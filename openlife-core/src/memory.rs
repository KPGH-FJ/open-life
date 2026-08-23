use crate::persistence_outbox::{
    self, CanonicalMutationReceipt, ProjectionDelivery, ProjectionSummary,
};
use crate::vectors::{
    CanonicalVectorOwnerRef, MemoryChunk, VectorRebuildSourceSnapshot, VECTOR_REBUILD_BATCH_LIMIT,
};
use anyhow::{Context, Result};
use chrono::Utc;
use ring::digest::{digest, Context as DigestContext, SHA256};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const FTS_QUERY_MAX_CHARS: usize = 256;
const FTS_QUERY_MAX_TOKENS: usize = 8;
const KNOWLEDGE_NOTE_MAX_CONTENT_BYTES: usize = 256 * 1024;
const KNOWLEDGE_NOTE_MAX_SOURCE_BYTES: usize = 128;
const KNOWLEDGE_NOTE_MAX_TAGS: usize = 32;
const KNOWLEDGE_NOTE_MAX_TAG_BYTES: usize = 128;
/// Canonical store for user-authored knowledge notes plus rebuildable
/// projections of lifecycle-owned Agent Memory.
///
/// Conversation messages belong to `ConversationStore`; long-term Memory
/// bodies belong to `MemoryLifecycleStore`. The on-disk database and outbox
/// owner retain the historical `memory_store` identity for crash recovery,
/// but this Rust type deliberately names the narrower product responsibility.
#[derive(Clone)]
pub struct KnowledgeNoteProjectionStore {
    conn: Arc<Mutex<Connection>>,
}

#[cfg(test)]
type MemoryStore = KnowledgeNoteProjectionStore;

fn normalize_fts_query(query: &str) -> Option<String> {
    let limited = query
        .trim()
        .chars()
        .take(FTS_QUERY_MAX_CHARS)
        .collect::<String>();
    let tokens = limited
        .split_whitespace()
        .filter_map(|part| {
            let trimmed = part.trim_matches(|c: char| {
                !c.is_alphanumeric() && !('\u{4e00}'..='\u{9fff}').contains(&c)
            });
            if trimmed.is_empty() {
                None
            } else {
                Some(format!("\"{}\"", trimmed.replace('"', "\"\"")))
            }
        })
        .take(FTS_QUERY_MAX_TOKENS)
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

fn update_length_delimited_digest(context: &mut DigestContext, bytes: &[u8]) {
    context.update(&(bytes.len() as u64).to_le_bytes());
    context.update(bytes);
}

fn ensure_knowledge_note_operation_id(operation_id: &str) -> Result<()> {
    let parsed =
        uuid::Uuid::parse_str(operation_id).context("KnowledgeNote operation id must be a UUID")?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.hyphenated().to_string() != operation_id
    {
        anyhow::bail!("KnowledgeNote operation id must be canonical lowercase UUIDv4");
    }
    Ok(())
}

fn ensure_knowledge_note_payload(
    session_id: &str,
    content: &str,
    content_type: &str,
    source: &str,
    tags: &[String],
    privacy_level: &str,
) -> Result<()> {
    let invalid_control = |value: &str| {
        value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    };
    let invalid_label = |value: &str| {
        value.chars().any(|character| {
            !character.is_alphanumeric()
                && !matches!(character, ' ' | '_' | '-' | '.' | ':' | '/' | '@')
        })
    };
    if session_id.trim() != session_id || session_id.is_empty() || session_id.len() > 256 {
        anyhow::bail!("KnowledgeNote session id is invalid");
    }
    if content.trim().is_empty()
        || content.len() > KNOWLEDGE_NOTE_MAX_CONTENT_BYTES
        || invalid_control(content)
    {
        anyhow::bail!("KnowledgeNote content is invalid or exceeds the bounded limit");
    }
    if source.trim() != source
        || source.is_empty()
        || source.len() > KNOWLEDGE_NOTE_MAX_SOURCE_BYTES
        || invalid_control(source)
        || invalid_label(source)
    {
        anyhow::bail!("KnowledgeNote source is invalid or exceeds the bounded limit");
    }
    if content_type != "knowledge_note" || privacy_level != "private" {
        anyhow::bail!("KnowledgeNote admission requires the private typed-note contract");
    }
    if tags.len() > KNOWLEDGE_NOTE_MAX_TAGS
        || tags.iter().any(|tag| {
            tag.trim() != tag
                || tag.is_empty()
                || tag.len() > KNOWLEDGE_NOTE_MAX_TAG_BYTES
                || invalid_control(tag)
                || invalid_label(tag)
        })
    {
        anyhow::bail!("KnowledgeNote tags are invalid or exceed the bounded limit");
    }
    Ok(())
}

fn ensure_memory_retrieval_reason_code(reason_code: &str) -> Result<()> {
    if reason_code.is_empty()
        || reason_code.len() > 96
        || !reason_code.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        anyhow::bail!("invalid canonical Memory retrieval reason code");
    }
    Ok(())
}

fn memory_retrieval_aggregate_id(owner: &CanonicalVectorOwnerRef) -> String {
    persistence_outbox::metadata_digest(&format!("memory_retrieval_owner:{}", owner.source()))
}

fn configure_memory_store_connection(conn: &Connection, file_backed: bool) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    if file_backed {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub content_type: String,
    pub source: String,
    pub role: Option<String>,
    pub created_at: String,
    pub importance_score: f32,
    pub access_count: i64,
    pub last_accessed_at: Option<String>,
    pub tags: Vec<String>,
    pub privacy_level: String,
    pub embedding_id: Option<i64>,
    pub checksum: String,
}

#[derive(Debug, Clone)]
pub struct VectorRebuildSourceRecord {
    pub memory: MemoryRecord,
    pub canonical_owner: Option<CanonicalVectorOwnerRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalKnowledgeNoteWrite {
    pub operation_id: String,
    pub knowledge_note_id: i64,
    /// Opaque nonce-bound journal token. It is deliberately not a
    /// deterministic digest of the Memory body.
    pub operation_digest: String,
    pub replayed: bool,
    pub canonical_mutation: CanonicalMutationReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchHit {
    pub chunk: MemoryChunk,
    pub relevance_score: f32,
    pub source_tier: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetrievalDisposition {
    Active,
    Paused,
    Archived,
}

impl MemoryRetrievalDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Archived => "archived",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "archived" => Ok(Self::Archived),
            other => anyhow::bail!("unsupported canonical Memory retrieval disposition: {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMemoryRetrievalState {
    pub owner_kind: String,
    pub owner_id: String,
    pub disposition: MemoryRetrievalDisposition,
    pub revision: u64,
    pub last_event_id: String,
    pub changed_at: String,
}

impl CanonicalMemoryRetrievalState {
    pub fn owner(&self) -> Result<CanonicalVectorOwnerRef> {
        CanonicalVectorOwnerRef::new(&self.owner_kind, &self.owner_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMemoryRetrievalMutation {
    pub changed: bool,
    pub state: Option<CanonicalMemoryRetrievalState>,
    pub canonical_mutation: Option<CanonicalMutationReceipt>,
}

impl KnowledgeNoteProjectionStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open sqlite db at {:?}", db_path))?;
        configure_memory_store_connection(&conn, true)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory sqlite db")?;
        configure_memory_store_connection(&conn, false)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "memory_store",
            &["memories"],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        Ok(Self {
            conn: Arc::new(Mutex::new(
                crate::sqlite_migration::unavailable_read_only_sentinel("memory_store")?,
            )),
        })
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                content_type TEXT NOT NULL,
                source TEXT NOT NULL,
                role TEXT,
                created_at TEXT NOT NULL,
                importance_score REAL NOT NULL DEFAULT 0.5,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed_at TEXT,
                tags_json TEXT NOT NULL DEFAULT '[]',
                privacy_level TEXT NOT NULL DEFAULT 'private',
                embedding_id INTEGER,
                archived INTEGER NOT NULL DEFAULT 0,
                archived_at TEXT,
                checksum TEXT NOT NULL
            )",
            [],
        )?;
        Self::ensure_column_exists(&conn, "memories", "embedding_id", "INTEGER")?;
        Self::ensure_column_exists(&conn, "memories", "archived", "INTEGER NOT NULL DEFAULT 0")?;
        Self::ensure_column_exists(&conn, "memories", "archived_at", "TEXT")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_session_created ON memories(session_id, created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_content_type ON memories(content_type)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_embedding_id ON memories(embedding_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_memories_archived ON memories(archived)",
            [],
        )?;
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                content,
                tags,
                content='memories',
                content_rowid='id'
            )",
            [],
        )?;
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags_json);
            END",
            [],
        )?;
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, tags) VALUES('delete', old.id, old.content, old.tags_json);
            END",
            [],
        )?;
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, tags) VALUES('delete', old.id, old.content, old.tags_json);
                INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags_json);
            END",
            [],
        )?;
        persistence_outbox::init_schema(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS knowledge_note_operations (
                operation_id TEXT PRIMARY KEY,
                operation_digest TEXT NOT NULL,
                memory_id INTEGER NOT NULL,
                outbox_event_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(memory_id) REFERENCES memories(id),
                FOREIGN KEY(outbox_event_id) REFERENCES canonical_outbox_events(event_id)
             );",
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_note_operation_memory
             ON knowledge_note_operations(memory_id)",
            [],
        )?;
        conn.execute_batch(
            "DELETE FROM memories
             WHERE source LIKE 'memory_lifecycle:%'
                OR EXISTS (
                    SELECT 1 FROM json_each(
                        CASE WHEN json_valid(memories.tags_json)
                             THEN memories.tags_json ELSE '[]' END
                    ) owner_tag
                    WHERE owner_tag.value = 'canonical_owner:memory_lifecycle'
                );
             DROP TABLE IF EXISTS memory_materialization_projections;",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_retrieval_states (
                owner_kind TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                disposition TEXT NOT NULL CHECK(disposition IN ('active', 'archived')),
                revision INTEGER NOT NULL CHECK(revision > 0),
                last_event_id TEXT NOT NULL,
                reason_digest TEXT NOT NULL,
                changed_at TEXT NOT NULL,
                PRIMARY KEY(owner_kind, owner_id),
                FOREIGN KEY(last_event_id) REFERENCES canonical_outbox_events(event_id)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_memory_retrieval_disposition
             ON memory_retrieval_states(disposition, changed_at DESC);

             -- MemoryLifecycleStore is the only canonical retrieval owner for
             -- lifecycle assets. Remove intermediate dual-owner rows from old
             -- builds, then mechanically reject their reintroduction even
             -- through raw SQL inside this store.
             UPDATE canonical_outbox_deliveries
             SET terminal_disposition = 'compensated',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE terminal_disposition IS NULL
               AND event_id IN (
                   SELECT last_event_id FROM memory_retrieval_states
                   WHERE owner_kind = 'memory_lifecycle'
               );
             DELETE FROM memory_retrieval_states
             WHERE owner_kind = 'memory_lifecycle';
             CREATE TRIGGER IF NOT EXISTS reject_memory_lifecycle_retrieval_insert
             BEFORE INSERT ON memory_retrieval_states
             WHEN NEW.owner_kind = 'memory_lifecycle'
             BEGIN
                 SELECT RAISE(ABORT, 'memory_lifecycle retrieval belongs to MemoryLifecycleStore');
             END;
             CREATE TRIGGER IF NOT EXISTS reject_memory_lifecycle_retrieval_update
             BEFORE UPDATE OF owner_kind ON memory_retrieval_states
             WHEN NEW.owner_kind = 'memory_lifecycle'
             BEGIN
                 SELECT RAISE(ABORT, 'memory_lifecycle retrieval belongs to MemoryLifecycleStore');
             END;",
        )?;
        crate::sqlite_migration::record_schema_version(&conn, "memory_store", 9)?;
        Ok(())
    }

    fn ensure_column_exists(
        conn: &Connection,
        table_name: &str,
        column_name: &str,
        column_definition: &str,
    ) -> Result<()> {
        let pragma = format!("PRAGMA table_info({table_name})");
        let mut stmt = conn.prepare(&pragma)?;
        let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let has_column = columns
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .any(|name| name == column_name);
        if !has_column {
            let sql =
                format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}");
            conn.execute(&sql, [])?;
        }
        Ok(())
    }

    fn checksum_for(content: &str, session_id: &str, created_at: &str) -> String {
        let data = format!("{}:{}:{}", session_id, created_at, content);
        let hash = digest(&SHA256, data.as_bytes());
        let bytes = hash.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push_str(&format!("{:02x}", b));
        }
        out
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn insert_memory_row(
        conn: &Connection,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        role: Option<&str>,
        created_at: &str,
        tags: &[String],
        privacy_level: &str,
        embedding_id: Option<i64>,
    ) -> Result<i64> {
        let checksum = Self::checksum_for(content, session_id, created_at);
        let tags_json = serde_json::to_string(tags)?;
        conn.execute(
            "INSERT INTO memories (
                session_id, content, content_type, source, role, created_at,
                importance_score, access_count, last_accessed_at, tags_json,
                privacy_level, embedding_id, archived, archived_at, checksum
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                session_id,
                content,
                content_type,
                source,
                role,
                created_at,
                0.5_f32,
                0_i64,
                Option::<String>::None,
                tags_json,
                privacy_level,
                embedding_id,
                0_i64,
                Option::<String>::None,
                checksum
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn export_active_memory_records(&self) -> Result<Vec<MemoryRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, content, content_type, source, role, created_at,
                    importance_score, access_count, last_accessed_at, tags_json,
                    privacy_level, embedding_id, checksum
             FROM memories
             WHERE archived = 0
             ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([], |row| {
            let tags_json: String = row.get(10)?;
            Ok(MemoryRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                content: row.get(2)?,
                content_type: row.get(3)?,
                source: row.get(4)?,
                role: row.get(5)?,
                created_at: row.get(6)?,
                importance_score: row.get(7)?,
                access_count: row.get(8)?,
                last_accessed_at: row.get(9)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                privacy_level: row.get(11)?,
                embedding_id: row.get(12)?,
                checksum: row.get(13)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to export active memory records")
    }

    /// Captures the stable canonical-id boundary and a metadata-only digest for
    /// a vector rebuild. The digest intentionally excludes memory body text;
    /// content integrity is represented by the canonical checksum.
    fn canonical_vector_owner_for_memory(
        conn: &Connection,
        memory_id: i64,
    ) -> Result<Option<CanonicalVectorOwnerRef>> {
        let knowledge_contract = conn
            .query_row(
                "SELECT events.aggregate_kind, events.aggregate_id, events.mutation_kind
                 FROM knowledge_note_operations operations
                 JOIN canonical_outbox_events events
                   ON events.event_id = operations.outbox_event_id
                 WHERE operations.memory_id = ?1",
                [memory_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        match knowledge_contract {
            Some((kind, aggregate_id, mutation_kind))
                if aggregate_id == memory_id.to_string()
                    && kind == "knowledge_note"
                    && mutation_kind == "created" =>
            {
                Ok(Some(CanonicalVectorOwnerRef::new(
                    "knowledge_note",
                    &aggregate_id,
                )?))
            }
            Some(_) => anyhow::bail!("KnowledgeNote rebuild ownership proof is inconsistent"),
            None => Ok(None),
        }
    }

    fn validate_vector_rebuild_owner_contract(
        memory: &MemoryRecord,
        owner: Option<&CanonicalVectorOwnerRef>,
    ) -> Result<()> {
        match owner {
            Some(owner) if owner.kind() == "knowledge_note" => {
                if owner.id() != memory.id.to_string()
                    || memory.content_type != "knowledge_note"
                    || memory.privacy_level != "private"
                {
                    anyhow::bail!("KnowledgeNote rebuild row does not match its canonical proof");
                }
            }
            Some(_) => anyhow::bail!("unsupported vector rebuild owner contract"),
            None => {}
        }
        Ok(())
    }

    fn verified_canonical_memory_record_for_owner(
        conn: &Connection,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<Option<MemoryRecord>> {
        if owner.kind() != "knowledge_note" {
            anyhow::bail!("unsupported canonical Memory owner kind");
        }
        let memory_id = owner
            .id()
            .parse::<i64>()
            .context("KnowledgeNote owner id is not a canonical row id")?;
        let record = conn
            .query_row(
                "SELECT id, session_id, content, content_type, source, role, created_at,
                        importance_score, access_count, last_accessed_at, tags_json,
                        privacy_level, embedding_id, checksum
                 FROM memories WHERE id = ?1 AND archived = 0",
                [memory_id],
                row_to_memory_record,
            )
            .optional()?;
        let Some(record) = record else {
            return Ok(None);
        };
        let proven_owner = Self::canonical_vector_owner_for_memory(conn, memory_id)?;
        if proven_owner.as_ref() != Some(owner) {
            return Ok(None);
        }
        Self::validate_vector_rebuild_owner_contract(&record, Some(owner))?;
        Ok(Some(record))
    }

    fn memory_retrieval_state_from_conn(
        conn: &Connection,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<Option<(MemoryRetrievalDisposition, u64, String)>> {
        if owner.kind() == "memory_lifecycle" {
            anyhow::bail!("MemoryLifecycle retrieval disposition is owned by MemoryLifecycleStore");
        }
        conn.query_row(
            "SELECT disposition, revision, last_event_id
             FROM memory_retrieval_states
             WHERE owner_kind = ?1 AND owner_id = ?2",
            params![owner.kind(), owner.id()],
            |row| {
                let disposition_raw = row.get::<_, String>(0)?;
                let disposition =
                    MemoryRetrievalDisposition::parse(&disposition_raw).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                error.to_string(),
                            )),
                        )
                    })?;
                let revision_raw = row.get::<_, i64>(1)?;
                let revision = u64::try_from(revision_raw).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?;
                Ok((disposition, revision, row.get(2)?))
            },
        )
        .optional()
        .map_err(Into::into)
    }

    fn memory_retrieval_is_active_from_conn(
        conn: &Connection,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<bool> {
        Ok(Self::memory_retrieval_state_from_conn(conn, owner)?
            .is_none_or(|(disposition, _, _)| disposition == MemoryRetrievalDisposition::Active))
    }

    pub fn is_verified_canonical_memory_owner(
        &self,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(Self::verified_canonical_memory_record_for_owner(&conn, owner)?.is_some())
    }

    pub fn vector_rebuild_source_snapshot(&self) -> Result<VectorRebuildSourceSnapshot> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let through_memory_id: i64 = conn.query_row(
            "SELECT COALESCE(MAX(id), 0) FROM memories WHERE archived = 0",
            [],
            |row| row.get(0),
        )?;
        Self::vector_rebuild_source_snapshot_from_conn(&conn, through_memory_id)
    }

    pub fn vector_rebuild_source_snapshot_through(
        &self,
        through_memory_id: i64,
    ) -> Result<VectorRebuildSourceSnapshot> {
        if through_memory_id < 0 {
            anyhow::bail!("vector rebuild source cursor cannot be negative");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Self::vector_rebuild_source_snapshot_from_conn(&conn, through_memory_id)
    }

    fn vector_rebuild_source_snapshot_from_conn(
        conn: &Connection,
        through_memory_id: i64,
    ) -> Result<VectorRebuildSourceSnapshot> {
        let mut statement = conn.prepare(
            "SELECT id, session_id, checksum, content_type, source, created_at,
                    importance_score, access_count, COALESCE(last_accessed_at, ''),
                    tags_json, privacy_level
             FROM memories
             WHERE archived = 0 AND id <= ?1
             ORDER BY id",
        )?;
        let rows = statement.query_map([through_memory_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, f32>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?;
        let mut digest_context = DigestContext::new(&SHA256);
        digest_context.update(b"openlife:vector-rebuild-source:v3\0");
        let mut total_count = 0_usize;
        for row in rows {
            let (
                id,
                session_id,
                checksum,
                content_type,
                source,
                created_at,
                importance_score,
                access_count,
                last_accessed_at,
                tags_json,
                privacy_level,
            ) = row?;
            let owner = Self::canonical_vector_owner_for_memory(conn, id)?;
            total_count = total_count.saturating_add(1);
            digest_context.update(&id.to_le_bytes());
            update_length_delimited_digest(&mut digest_context, session_id.as_bytes());
            update_length_delimited_digest(&mut digest_context, checksum.as_bytes());
            update_length_delimited_digest(&mut digest_context, content_type.as_bytes());
            update_length_delimited_digest(&mut digest_context, source.as_bytes());
            update_length_delimited_digest(&mut digest_context, created_at.as_bytes());
            digest_context.update(&importance_score.to_bits().to_le_bytes());
            digest_context.update(&access_count.to_le_bytes());
            update_length_delimited_digest(&mut digest_context, last_accessed_at.as_bytes());
            update_length_delimited_digest(&mut digest_context, tags_json.as_bytes());
            update_length_delimited_digest(&mut digest_context, privacy_level.as_bytes());
            if let Some(owner) = owner {
                digest_context.update(&[1]);
                update_length_delimited_digest(&mut digest_context, owner.kind().as_bytes());
                update_length_delimited_digest(&mut digest_context, owner.id().as_bytes());
                match if owner.kind() == "memory_lifecycle" {
                    None
                } else {
                    Self::memory_retrieval_state_from_conn(conn, &owner)?
                } {
                    Some((disposition, revision, event_id)) => {
                        digest_context.update(&[1]);
                        update_length_delimited_digest(
                            &mut digest_context,
                            disposition.as_str().as_bytes(),
                        );
                        digest_context.update(&revision.to_le_bytes());
                        update_length_delimited_digest(&mut digest_context, event_id.as_bytes());
                    }
                    None => digest_context.update(&[0]),
                }
            } else {
                digest_context.update(&[0]);
            }
        }
        let digest = digest_context.finish();
        let metadata_digest = format!(
            "sha256:{}",
            digest
                .as_ref()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        Ok(VectorRebuildSourceSnapshot {
            through_memory_id,
            total_count,
            metadata_digest,
        })
    }

    /// Reads only one canonical-id page. The hard cap prevents callers from
    /// turning a rebuild back into an unbounded full-history allocation.
    pub fn load_vector_rebuild_source_page(
        &self,
        after_memory_id: i64,
        through_memory_id: i64,
        limit: usize,
    ) -> Result<Vec<VectorRebuildSourceRecord>> {
        if after_memory_id < 0 || through_memory_id < 0 {
            anyhow::bail!("vector rebuild source cursor cannot be negative");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, session_id, content, content_type, source, role, created_at,
                    importance_score, access_count, last_accessed_at, tags_json,
                    privacy_level, embedding_id, checksum
             FROM memories
             WHERE archived = 0 AND id > ?1 AND id <= ?2
             ORDER BY id
             LIMIT ?3",
        )?;
        let rows = statement.query_map(
            params![
                after_memory_id,
                through_memory_id,
                limit.clamp(1, VECTOR_REBUILD_BATCH_LIMIT) as i64
            ],
            |row| {
                let tags_json: String = row.get(10)?;
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    content: row.get(2)?,
                    content_type: row.get(3)?,
                    source: row.get(4)?,
                    role: row.get(5)?,
                    created_at: row.get(6)?,
                    importance_score: row.get(7)?,
                    access_count: row.get(8)?,
                    last_accessed_at: row.get(9)?,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    privacy_level: row.get(11)?,
                    embedding_id: row.get(12)?,
                    checksum: row.get(13)?,
                })
            },
        )?;
        let memories = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to load vector rebuild source page")?;
        memories
            .into_iter()
            .map(|memory| {
                let mut canonical_owner =
                    Self::canonical_vector_owner_for_memory(&conn, memory.id)?;
                Self::validate_vector_rebuild_owner_contract(&memory, canonical_owner.as_ref())?;
                if let Some(owner) = canonical_owner.as_ref() {
                    if !Self::memory_retrieval_is_active_from_conn(&conn, owner)? {
                        canonical_owner = None;
                    }
                }
                Ok(VectorRebuildSourceRecord {
                    memory,
                    canonical_owner,
                })
            })
            .collect()
    }

    pub fn set_memory_retrieval_disposition(
        &self,
        owner: &CanonicalVectorOwnerRef,
        disposition: MemoryRetrievalDisposition,
        reason_code: &str,
    ) -> Result<CanonicalMemoryRetrievalMutation> {
        self.set_memory_retrieval_dispositions(
            std::slice::from_ref(owner),
            disposition,
            reason_code,
        )?
        .into_iter()
        .next()
        .context("canonical Memory retrieval single mutation result is missing")
    }

    /// Atomically mutate one or more proven canonical owners. Every owner is
    /// validated before the first outbox row is enqueued; duplicate or forged
    /// owners roll back the entire batch.
    pub fn set_memory_retrieval_dispositions(
        &self,
        owners: &[CanonicalVectorOwnerRef],
        disposition: MemoryRetrievalDisposition,
        reason_code: &str,
    ) -> Result<Vec<CanonicalMemoryRetrievalMutation>> {
        ensure_memory_retrieval_reason_code(reason_code)?;
        if owners.is_empty() || owners.len() > 200 {
            anyhow::bail!("canonical Memory retrieval batch must contain 1..=200 owners");
        }
        if owners
            .iter()
            .any(|owner| owner.kind() == "memory_lifecycle")
        {
            anyhow::bail!("MemoryLifecycle retrieval disposition is owned by MemoryLifecycleStore");
        }
        let unique = owners
            .iter()
            .map(CanonicalVectorOwnerRef::source)
            .collect::<HashSet<_>>();
        if unique.len() != owners.len() {
            anyhow::bail!("canonical Memory retrieval batch contains duplicate owners");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for owner in owners {
            if Self::verified_canonical_memory_record_for_owner(&tx, owner)?.is_none() {
                anyhow::bail!("canonical Memory retrieval owner proof is missing or inconsistent");
            }
        }
        let mutations = owners
            .iter()
            .map(|owner| {
                Self::set_memory_retrieval_disposition_in_transaction(
                    &tx,
                    owner,
                    disposition,
                    reason_code,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(mutations)
    }

    fn set_memory_retrieval_disposition_in_transaction(
        conn: &Transaction<'_>,
        owner: &CanonicalVectorOwnerRef,
        disposition: MemoryRetrievalDisposition,
        reason_code: &str,
    ) -> Result<CanonicalMemoryRetrievalMutation> {
        let existing = conn
            .query_row(
                "SELECT disposition, revision, last_event_id, changed_at
                 FROM memory_retrieval_states
                 WHERE owner_kind = ?1 AND owner_id = ?2",
                params![owner.kind(), owner.id()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        if let Some((stored, revision, last_event_id, changed_at)) = existing.as_ref() {
            let stored_disposition = MemoryRetrievalDisposition::parse(stored)?;
            if stored_disposition == disposition {
                let revision = u64::try_from(*revision)
                    .context("canonical Memory retrieval revision is invalid")?;
                let canonical_mutation =
                    persistence_outbox::mutation_by_event_id(conn, last_event_id)?
                        .context("canonical Memory retrieval state lost its outbox event")?;
                let state = CanonicalMemoryRetrievalState {
                    owner_kind: owner.kind().to_string(),
                    owner_id: owner.id().to_string(),
                    disposition,
                    revision,
                    last_event_id: last_event_id.clone(),
                    changed_at: changed_at.clone(),
                };
                return Ok(CanonicalMemoryRetrievalMutation {
                    changed: false,
                    state: Some(state),
                    canonical_mutation: Some(canonical_mutation),
                });
            }
        } else if disposition == MemoryRetrievalDisposition::Active {
            return Ok(CanonicalMemoryRetrievalMutation {
                changed: false,
                state: None,
                canonical_mutation: None,
            });
        }

        let reason_digest = persistence_outbox::metadata_digest(reason_code);
        let payload_digest = persistence_outbox::metadata_digest(&format!(
            "memory_retrieval:{}:{}:{}",
            owner.source(),
            disposition.as_str(),
            uuid::Uuid::new_v4()
        ));
        let canonical_mutation = persistence_outbox::enqueue_mutation(
            conn,
            "memory_retrieval",
            &memory_retrieval_aggregate_id(owner),
            disposition.as_str(),
            &payload_digest,
            &["vector_store"],
        )?;
        let revision_raw = i64::try_from(canonical_mutation.aggregate_revision)
            .context("canonical Memory retrieval revision exceeds SQLite range")?;
        let changed_at = canonical_mutation.created_at.to_rfc3339();
        conn.execute(
            "INSERT INTO memory_retrieval_states (
                owner_kind, owner_id, disposition, revision, last_event_id,
                reason_digest, changed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(owner_kind, owner_id) DO UPDATE SET
                disposition = excluded.disposition,
                revision = excluded.revision,
                last_event_id = excluded.last_event_id,
                reason_digest = excluded.reason_digest,
                changed_at = excluded.changed_at",
            params![
                owner.kind(),
                owner.id(),
                disposition.as_str(),
                revision_raw,
                canonical_mutation.event_id,
                reason_digest,
                changed_at,
            ],
        )?;
        let state = CanonicalMemoryRetrievalState {
            owner_kind: owner.kind().to_string(),
            owner_id: owner.id().to_string(),
            disposition,
            revision: canonical_mutation.aggregate_revision,
            last_event_id: canonical_mutation.event_id.clone(),
            changed_at,
        };
        Ok(CanonicalMemoryRetrievalMutation {
            changed: true,
            state: Some(state),
            canonical_mutation: Some(canonical_mutation),
        })
    }

    pub fn memory_retrieval_state(
        &self,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<Option<CanonicalMemoryRetrievalState>> {
        if owner.kind() == "memory_lifecycle" {
            anyhow::bail!("MemoryLifecycle retrieval disposition is owned by MemoryLifecycleStore");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT disposition, revision, last_event_id, changed_at
             FROM memory_retrieval_states
             WHERE owner_kind = ?1 AND owner_id = ?2",
            params![owner.kind(), owner.id()],
            |row| {
                let disposition_raw = row.get::<_, String>(0)?;
                let disposition =
                    MemoryRetrievalDisposition::parse(&disposition_raw).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                error.to_string(),
                            )),
                        )
                    })?;
                let revision_raw = row.get::<_, i64>(1)?;
                let revision = u64::try_from(revision_raw).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Integer,
                        Box::new(error),
                    )
                })?;
                Ok(CanonicalMemoryRetrievalState {
                    owner_kind: owner.kind().to_string(),
                    owner_id: owner.id().to_string(),
                    disposition,
                    revision,
                    last_event_id: row.get(2)?,
                    changed_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// Reload the exact canonical head that authorizes a retrieval-state
    /// projection. A stale event cannot reapply an older archive/restore state.
    pub fn load_memory_retrieval_state_for_projection(
        &self,
        event_id: &str,
    ) -> Result<CanonicalMemoryRetrievalState> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let (state, event_aggregate_id, event_mutation_kind, event_revision) = conn
            .query_row(
                "SELECT states.owner_kind, states.owner_id, states.disposition,
                        states.revision, states.last_event_id, states.changed_at,
                        events.aggregate_id, events.mutation_kind,
                        events.aggregate_revision
                 FROM memory_retrieval_states states
                 JOIN canonical_outbox_events events
                   ON events.event_id = states.last_event_id
                 WHERE states.last_event_id = ?1
                   AND events.aggregate_kind = 'memory_retrieval'",
                [event_id],
                |row| {
                    let disposition_raw = row.get::<_, String>(2)?;
                    let disposition =
                        MemoryRetrievalDisposition::parse(&disposition_raw).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    error.to_string(),
                                )),
                            )
                        })?;
                    let revision_raw = row.get::<_, i64>(3)?;
                    let revision = u64::try_from(revision_raw).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?;
                    let event_revision_raw = row.get::<_, i64>(8)?;
                    let event_revision = u64::try_from(event_revision_raw).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?;
                    Ok((
                        CanonicalMemoryRetrievalState {
                            owner_kind: row.get(0)?,
                            owner_id: row.get(1)?,
                            disposition,
                            revision,
                            last_event_id: row.get(4)?,
                            changed_at: row.get(5)?,
                        },
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        event_revision,
                    ))
                },
            )
            .optional()?
            .context("canonical Memory retrieval projection is stale or missing")?;
        let owner = state.owner()?;
        if event_aggregate_id != memory_retrieval_aggregate_id(&owner)
            || event_mutation_kind != state.disposition.as_str()
            || event_revision != state.revision
        {
            anyhow::bail!("canonical Memory retrieval projection identity is inconsistent");
        }
        Ok(state)
    }

    /// Resolve the current canonical retrieval head for an event on the same
    /// opaque owner aggregate. This is used only to compensate a stale
    /// projection delivery; it never authorizes replaying the stale mutation.
    pub fn load_memory_retrieval_head_for_event(
        &self,
        event_id: &str,
    ) -> Result<CanonicalMemoryRetrievalState> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let state = conn
            .query_row(
                "SELECT states.owner_kind, states.owner_id, states.disposition,
                        states.revision, states.last_event_id, states.changed_at
                 FROM canonical_outbox_events stale_events
                 JOIN canonical_outbox_events head_events
                   ON head_events.aggregate_kind = stale_events.aggregate_kind
                  AND head_events.aggregate_id = stale_events.aggregate_id
                 JOIN memory_retrieval_states states
                   ON states.last_event_id = head_events.event_id
                 WHERE stale_events.event_id = ?1
                   AND stale_events.aggregate_kind = 'memory_retrieval'
                   AND head_events.aggregate_kind = 'memory_retrieval'",
                [event_id],
                |row| {
                    let disposition_raw = row.get::<_, String>(2)?;
                    let disposition =
                        MemoryRetrievalDisposition::parse(&disposition_raw).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                2,
                                rusqlite::types::Type::Text,
                                Box::new(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    error.to_string(),
                                )),
                            )
                        })?;
                    let revision_raw = row.get::<_, i64>(3)?;
                    let revision = u64::try_from(revision_raw).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Integer,
                            Box::new(error),
                        )
                    })?;
                    Ok(CanonicalMemoryRetrievalState {
                        owner_kind: row.get(0)?,
                        owner_id: row.get(1)?,
                        disposition,
                        revision,
                        last_event_id: row.get(4)?,
                        changed_at: row.get(5)?,
                    })
                },
            )
            .optional()?
            .context("canonical Memory retrieval event has no current head")?;
        let owner = state.owner()?;
        let head = Self::memory_retrieval_state_from_conn(&conn, &owner)?
            .context("canonical Memory retrieval head is missing")?;
        if head.0 != state.disposition || head.1 != state.revision || head.2 != state.last_event_id
        {
            anyhow::bail!("canonical Memory retrieval head changed while loading");
        }
        Ok(state)
    }

    pub fn mark_memory_retrieval_projection_applied_if_head(
        &self,
        event_id: &str,
        revision: u64,
        projection_target: &str,
    ) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_applied_if_canonical_head(
            &mut conn,
            event_id,
            revision,
            projection_target,
        )
    }

    pub fn mark_memory_retrieval_projection_compensated_to_head(
        &self,
        stale_event_id: &str,
        head_event_id: &str,
        head_revision: u64,
        projection_target: &str,
    ) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_compensated_to_head(
            &mut conn,
            stale_event_id,
            head_event_id,
            head_revision,
            projection_target,
        )
    }

    pub fn is_memory_retrieval_active(&self, owner: &CanonicalVectorOwnerRef) -> Result<bool> {
        if owner.kind() == "memory_lifecycle" {
            anyhow::bail!("MemoryLifecycle retrieval disposition is owned by MemoryLifecycleStore");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        if Self::verified_canonical_memory_record_for_owner(&conn, owner)?.is_none() {
            return Ok(false);
        }
        Self::memory_retrieval_is_active_from_conn(&conn, owner)
    }

    pub fn list_archived_memory_retrieval_states(
        &self,
        limit: usize,
    ) -> Result<Vec<CanonicalMemoryRetrievalState>> {
        self.list_archived_memory_retrieval_states_page(limit, 0)
    }

    pub fn list_archived_memory_retrieval_states_page(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<CanonicalMemoryRetrievalState>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT owner_kind, owner_id, disposition, revision, last_event_id, changed_at
             FROM memory_retrieval_states
             WHERE disposition = 'archived'
               AND owner_kind != 'memory_lifecycle'
             ORDER BY changed_at DESC, owner_kind, owner_id",
        )?;
        let rows = statement.query_map([], |row| {
            let disposition_raw = row.get::<_, String>(2)?;
            let disposition =
                MemoryRetrievalDisposition::parse(&disposition_raw).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            error.to_string(),
                        )),
                    )
                })?;
            let revision_raw = row.get::<_, i64>(3)?;
            let revision = u64::try_from(revision_raw).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Integer,
                    Box::new(error),
                )
            })?;
            Ok(CanonicalMemoryRetrievalState {
                owner_kind: row.get(0)?,
                owner_id: row.get(1)?,
                disposition,
                revision,
                last_event_id: row.get(4)?,
                changed_at: row.get(5)?,
            })
        })?;
        let mut valid_seen = 0_usize;
        let mut page = Vec::with_capacity(limit.min(500));
        for row in rows {
            let state = row?;
            let owner = state.owner()?;
            if Self::verified_canonical_memory_record_for_owner(&conn, &owner)?.is_none() {
                continue;
            }
            if valid_seen < offset {
                valid_seen = valid_seen.saturating_add(1);
                continue;
            }
            page.push(state);
            if page.len() >= limit {
                break;
            }
        }
        Ok(page)
    }

    pub fn count_archived_memory_retrieval_states(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT owner_kind, owner_id
             FROM memory_retrieval_states
             WHERE disposition = 'archived'
               AND owner_kind != 'memory_lifecycle'",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut count = 0_usize;
        for owner in rows {
            let (owner_kind, owner_id) = owner?;
            let owner = CanonicalVectorOwnerRef::new(&owner_kind, &owner_id)?;
            if Self::verified_canonical_memory_record_for_owner(&conn, &owner)?.is_some() {
                count = count.saturating_add(1);
            }
        }
        Ok(count)
    }

    pub fn list_replayable_projection_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<ProjectionDelivery>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::list_replayable_deliveries(&conn, limit)
    }

    pub fn list_replayable_projection_deliveries_for_event(
        &self,
        event_id: &str,
    ) -> Result<Vec<ProjectionDelivery>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::list_replayable_deliveries_for_event(&conn, event_id)
    }

    pub fn mark_projection_applied(&self, event_id: &str, projection_target: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_applied(&conn, event_id, projection_target)
    }

    pub fn mark_projection_degraded(
        &self,
        event_id: &str,
        projection_target: &str,
        error: &str,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::mark_delivery_degraded(&conn, event_id, projection_target, error)
    }

    pub fn projection_summary(&self, event_id: &str) -> Result<ProjectionSummary> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        persistence_outbox::projection_summary(&conn, event_id)
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_outbox_insert_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "CREATE TRIGGER fail_canonical_outbox_insert_for_test
             BEFORE INSERT ON canonical_outbox_events
             BEGIN
                 SELECT RAISE(ABORT, 'injected canonical outbox insert failure');
             END;",
        )?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn remove_canonical_memory_row_for_corruption_test(&self, memory_id: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.pragma_update(None, "foreign_keys", false)?;
        let deletion = conn.execute("DELETE FROM memories WHERE id = ?1", [memory_id]);
        let foreign_keys = conn.pragma_update(None, "foreign_keys", true);
        let deleted = deletion?;
        foreign_keys?;
        if deleted != 1 {
            anyhow::bail!("canonical Memory corruption fixture row was not found");
        }
        Ok(())
    }

    /// Commit a canonical KnowledgeNote and its vector projection
    /// trigger in one local transaction. The outbox stores only the row id and
    /// an opaque digest; the vector materializer reloads the body from this
    /// canonical owner.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn save_knowledge_note_idempotent_with_outbox(
        &self,
        operation_id: &str,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        tags: &[String],
        privacy_level: &str,
    ) -> Result<CanonicalKnowledgeNoteWrite> {
        ensure_knowledge_note_operation_id(operation_id)?;
        ensure_knowledge_note_payload(
            session_id,
            content,
            content_type,
            source,
            tags,
            privacy_level,
        )?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT operation_digest, memory_id, outbox_event_id
                 FROM knowledge_note_operations WHERE operation_id = ?1",
                [operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((operation_digest, memory_id, event_id)) = existing {
            if !Self::knowledge_note_payload_matches(
                &tx,
                memory_id,
                session_id,
                content,
                content_type,
                source,
                tags,
                privacy_level,
            )? {
                anyhow::bail!(
                    "KnowledgeNote operation id was reused with a different canonical payload"
                );
            }
            let canonical_mutation = persistence_outbox::mutation_by_event_id(&tx, &event_id)?
                .context("KnowledgeNote operation lost its canonical outbox receipt")?;
            let current_contract = canonical_mutation.aggregate_kind == "knowledge_note"
                && canonical_mutation.mutation_kind == "created";
            if !current_contract || canonical_mutation.aggregate_id != memory_id.to_string() {
                anyhow::bail!("KnowledgeNote operation has inconsistent canonical refs");
            }
            tx.commit()?;
            return Ok(CanonicalKnowledgeNoteWrite {
                operation_id: operation_id.to_string(),
                knowledge_note_id: memory_id,
                operation_digest,
                replayed: true,
                canonical_mutation,
            });
        }
        let created_at = Utc::now().to_rfc3339();
        let memory_id = Self::insert_memory_row(
            &tx,
            session_id,
            content,
            content_type,
            source,
            None,
            &created_at,
            tags,
            privacy_level,
            None,
        )?;
        // A one-time nonce makes the persisted token non-enumerable while the
        // aggregate ref remains sufficient for deterministic replay.
        let outbox_payload_digest = persistence_outbox::metadata_digest(&format!(
            "knowledge_note:{memory_id}:{}",
            uuid::Uuid::new_v4()
        ));
        let canonical_mutation = persistence_outbox::enqueue_mutation(
            &tx,
            "knowledge_note",
            &memory_id.to_string(),
            "created",
            &outbox_payload_digest,
            &["vector_store"],
        )?;
        let operation_digest = persistence_outbox::metadata_digest(&format!(
            "knowledge_note_operation:{operation_id}:{}",
            uuid::Uuid::new_v4()
        ));
        tx.execute(
            "INSERT INTO knowledge_note_operations (
                operation_id, operation_digest, memory_id, outbox_event_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                operation_id,
                operation_digest,
                memory_id,
                canonical_mutation.event_id,
                created_at,
            ],
        )?;
        tx.commit()?;
        Ok(CanonicalKnowledgeNoteWrite {
            operation_id: operation_id.to_string(),
            knowledge_note_id: memory_id,
            operation_digest,
            replayed: false,
            canonical_mutation,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn knowledge_note_payload_matches(
        conn: &Connection,
        memory_id: i64,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        tags: &[String],
        privacy_level: &str,
    ) -> Result<bool> {
        let existing = conn
            .query_row(
                "SELECT session_id, content, content_type, source, tags_json, privacy_level
                 FROM memories WHERE id = ?1",
                [memory_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()?
            .context("KnowledgeNote operation lost its canonical row")?;
        let mut existing_tags = serde_json::from_str::<Vec<String>>(&existing.4)
            .context("KnowledgeNote canonical tags are invalid")?;
        let mut requested_tags = tags.to_vec();
        existing_tags.sort();
        requested_tags.sort();
        Ok(existing.0 == session_id
            && existing.1 == content
            && existing.2 == content_type
            && existing.3 == source
            && existing_tags == requested_tags
            && existing.5 == privacy_level)
    }

    pub fn get_active_memory_record(&self, memory_id: i64) -> Result<Option<MemoryRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT id, session_id, content, content_type, source, role, created_at,
                    importance_score, access_count, last_accessed_at, tags_json,
                    privacy_level, embedding_id, checksum
             FROM memories WHERE id = ?1 AND archived = 0",
            [memory_id],
            row_to_memory_record,
        )
        .optional()
        .map_err(Into::into)
    }

    /// Load a KnowledgeNote body only when the operation journal, canonical
    /// outbox event, aggregate identity, mutation contract, and active row all
    /// prove the same owner. Projection code must not recover a body by numeric
    /// row id alone.
    pub fn load_verified_knowledge_note_projection(
        &self,
        event_id: &str,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<MemoryRecord> {
        if owner.kind() != "knowledge_note" {
            anyhow::bail!("unsupported KnowledgeNote projection owner");
        }
        let memory_id = owner
            .id()
            .parse::<i64>()
            .context("KnowledgeNote owner id is not a canonical row id")?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let record = conn
            .query_row(
                "SELECT memories.id, memories.session_id, memories.content,
                        memories.content_type, memories.source, memories.role,
                        memories.created_at, memories.importance_score,
                        memories.access_count, memories.last_accessed_at,
                        memories.tags_json, memories.privacy_level,
                        memories.embedding_id, memories.checksum
                 FROM knowledge_note_operations operations
                 JOIN canonical_outbox_events events
                   ON events.event_id = operations.outbox_event_id
                 JOIN memories ON memories.id = operations.memory_id
                 WHERE operations.outbox_event_id = ?1
                   AND operations.memory_id = ?2
                   AND events.aggregate_kind = ?3
                   AND events.aggregate_id = ?4
                   AND events.mutation_kind = ?5
                   AND memories.archived = 0",
                params![event_id, memory_id, owner.kind(), owner.id(), "created",],
                row_to_memory_record,
            )
            .optional()?
            .context("canonical KnowledgeNote projection proof is missing or inconsistent")?;
        Self::validate_vector_rebuild_owner_contract(&record, Some(owner))?;
        Ok(record)
    }

    pub fn is_verified_knowledge_note_owner_active(
        &self,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<bool> {
        if owner.kind() != "knowledge_note" {
            anyhow::bail!("unsupported KnowledgeNote read owner");
        }
        let memory_id = owner
            .id()
            .parse::<i64>()
            .context("KnowledgeNote owner id is not a canonical row id")?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let proven_owner = Self::canonical_vector_owner_for_memory(&conn, memory_id)?;
        if proven_owner.as_ref() != Some(owner) {
            return Ok(false);
        }
        let record = conn
            .query_row(
                "SELECT id, session_id, content, content_type, source, role, created_at,
                        importance_score, access_count, last_accessed_at, tags_json,
                        privacy_level, embedding_id, checksum
                 FROM memories WHERE id = ?1 AND archived = 0",
                [memory_id],
                row_to_memory_record,
            )
            .optional()?;
        let Some(record) = record else {
            return Ok(false);
        };
        Ok(Self::validate_vector_rebuild_owner_contract(&record, Some(owner)).is_ok())
    }

    /// Pure text lookup. Access telemetry is a separate explicit mutation via
    /// [`Self::record_text_search_access_telemetry`].
    pub fn search_text_memories(
        &self,
        session_id: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>> {
        if query.trim().is_empty() {
            return Ok(vec![]);
        }

        let Some(normalized_query) = normalize_fts_query(query) else {
            return Ok(vec![]);
        };

        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut results = if let Some(session_id) = session_id {
            let mut stmt = match conn.prepare(
                "SELECT m.id, m.session_id, m.content, m.source, m.created_at, m.access_count, m.last_accessed_at, bm25(memories_fts) AS rank
                 FROM memories_fts
                 JOIN memories m ON m.id = memories_fts.rowid
                 WHERE memories_fts MATCH ?1
                   AND m.session_id = ?2
                   AND m.archived = 0
                 ORDER BY rank ASC, m.created_at DESC
                 LIMIT ?3",
            ) {
                Ok(stmt) => stmt,
                Err(_) => {
                    return self.search_text_memories_fallback(
                        &conn,
                        session_id,
                        query,
                        limit,
                    );
                }
            };
            let rows =
                stmt.query_map(params![normalized_query, session_id, limit as i64], |row| {
                    let rank: f32 = row.get(7)?;
                    Ok(Self::row_to_search_hit(row, rank, "fts"))
                })?;
            rows.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = match conn.prepare(
                "SELECT m.id, m.session_id, m.content, m.source, m.created_at, m.access_count, m.last_accessed_at, bm25(memories_fts) AS rank
                 FROM memories_fts
                 JOIN memories m ON m.id = memories_fts.rowid
                 WHERE memories_fts MATCH ?1
                   AND m.archived = 0
                 ORDER BY rank ASC, m.created_at DESC
                 LIMIT ?2",
            ) {
                Ok(stmt) => stmt,
                Err(_) => return self.search_text_memories_fallback(&conn, "", query, limit),
            };
            let rows = stmt.query_map(params![normalized_query, limit as i64], |row| {
                let rank: f32 = row.get(7)?;
                Ok(Self::row_to_search_hit(row, rank, "fts"))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        if results.is_empty() {
            results =
                self.search_text_memories_fallback(&conn, session_id.unwrap_or(""), query, limit)?;
        }
        results = Self::filter_retrieval_active_search_hits(&conn, results)?;

        results.truncate(limit);
        Ok(results)
    }

    /// Commit optional access telemetry for results returned by a prior pure
    /// text search. Callers must place this mutation behind their canonical
    /// owner admission fence; a failed telemetry commit must never invalidate
    /// the already-observed search result.
    pub fn record_text_search_access_telemetry(&self, memory_ids: &[i64]) -> Result<usize> {
        let mut memory_ids = memory_ids
            .iter()
            .copied()
            .filter(|memory_id| *memory_id > 0)
            .collect::<Vec<_>>();
        memory_ids.sort_unstable();
        memory_ids.dedup();
        if memory_ids.is_empty() {
            return Ok(0);
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let mut updated = 0usize;
        for memory_id in memory_ids {
            updated += tx.execute(
                "UPDATE memories
                 SET access_count = access_count + 1, last_accessed_at = ?1
                 WHERE id = ?2 AND archived = 0",
                params![&now, memory_id],
            )?;
        }
        tx.commit()?;
        Ok(updated)
    }

    fn search_text_memories_fallback(
        &self,
        conn: &Connection,
        session_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>> {
        let like_query = format!("%{}%", query.trim());
        let sql = if session_id.is_empty() {
            "SELECT id, session_id, content, source, created_at, access_count, last_accessed_at
             FROM memories
             WHERE archived = 0 AND content LIKE ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        } else {
            "SELECT id, session_id, content, source, created_at, access_count, last_accessed_at
             FROM memories
             WHERE archived = 0 AND session_id = ?1 AND content LIKE ?2
             ORDER BY created_at DESC
             LIMIT ?3"
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = if session_id.is_empty() {
            stmt.query_map(params![like_query, limit as i64], |row| {
                Ok(Self::row_to_search_hit_fallback(row, query, "keyword"))
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![session_id, like_query, limit as i64], |row| {
                Ok(Self::row_to_search_hit_fallback(row, query, "keyword"))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };
        Self::filter_retrieval_active_search_hits(conn, rows)
    }

    fn filter_retrieval_active_search_hits(
        conn: &Connection,
        hits: Vec<MemorySearchHit>,
    ) -> Result<Vec<MemorySearchHit>> {
        hits.into_iter()
            .filter_map(|hit| {
                if hit.chunk.id <= 0 {
                    return Some(Ok(hit));
                }
                match Self::canonical_vector_owner_for_memory(conn, hit.chunk.id) {
                    Ok(Some(owner)) => {
                        match Self::memory_retrieval_is_active_from_conn(conn, &owner) {
                            Ok(true) => Some(Ok(hit)),
                            Ok(false) => None,
                            Err(error) => Some(Err(error)),
                        }
                    }
                    Ok(None) => Some(Ok(hit)),
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    fn row_to_search_hit(row: &rusqlite::Row, rank: f32, source_tier: &str) -> MemorySearchHit {
        MemorySearchHit {
            chunk: MemoryChunk {
                id: row.get(0).unwrap_or_default(),
                session_id: row.get(1).unwrap_or_default(),
                content: row.get(2).unwrap_or_default(),
                source: row.get(3).unwrap_or_else(|_| "memory".to_string()),
                created_at: row.get(4).unwrap_or_default(),
                tier: 2,
                access_count: row.get(5).unwrap_or_default(),
                last_accessed_at: row
                    .get::<_, Option<String>>(6)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                importance_score: 0.0,
                archived: false,
                archived_at: None,
                summary: None,
            },
            relevance_score: 1.0 / (1.0 + rank.max(0.0)),
            source_tier: source_tier.to_string(),
        }
    }

    fn row_to_search_hit_fallback(
        row: &rusqlite::Row,
        query: &str,
        source_tier: &str,
    ) -> MemorySearchHit {
        let content: String = row.get(2).unwrap_or_default();
        let score = if content.contains(query) { 0.72 } else { 0.45 };
        MemorySearchHit {
            chunk: MemoryChunk {
                id: row.get(0).unwrap_or_default(),
                session_id: row.get(1).unwrap_or_default(),
                content,
                source: row.get(3).unwrap_or_else(|_| "memory".to_string()),
                created_at: row.get(4).unwrap_or_default(),
                tier: 2,
                access_count: row.get(5).unwrap_or_default(),
                last_accessed_at: row
                    .get::<_, Option<String>>(6)
                    .unwrap_or_default()
                    .unwrap_or_default(),
                importance_score: 0.0,
                archived: false,
                archived_at: None,
                summary: None,
            },
            relevance_score: score,
            source_tier: source_tier.to_string(),
        }
    }
}

fn row_to_memory_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRecord> {
    let tags_json: String = row.get(10)?;
    Ok(MemoryRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        content: row.get(2)?,
        content_type: row.get(3)?,
        source: row.get(4)?,
        role: row.get(5)?,
        created_at: row.get(6)?,
        importance_score: row.get(7)?,
        access_count: row.get(8)?,
        last_accessed_at: row.get(9)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        privacy_level: row.get(11)?,
        embedding_id: row.get(12)?,
        checksum: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn save_test_knowledge_note(
        store: &MemoryStore,
        session_id: &str,
        content: &str,
    ) -> CanonicalKnowledgeNoteWrite {
        store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                session_id,
                content,
                "knowledge_note",
                "test",
                &[],
                "private",
            )
            .unwrap()
    }

    fn insert_unowned_test_row(
        store: &MemoryStore,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        tags: &[String],
    ) -> i64 {
        let conn = store.conn.lock().unwrap();
        MemoryStore::insert_memory_row(
            &conn,
            session_id,
            content,
            content_type,
            source,
            None,
            &Utc::now().to_rfc3339(),
            tags,
            "private",
            None,
        )
        .unwrap()
    }

    #[test]
    fn canonical_memory_retrieval_state_is_stable_idempotent_and_outbox_backed() {
        let store = MemoryStore::new_in_memory().unwrap();
        let note = store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "retrieval-state-session",
                "canonical retrieval state body",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner =
            CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                .unwrap();

        let archived = store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        assert!(archived.changed);
        let archived_state = archived.state.as_ref().unwrap();
        assert_eq!(
            archived_state.disposition,
            MemoryRetrievalDisposition::Archived
        );
        assert_eq!(archived_state.revision, 1);
        assert!(!store.is_memory_retrieval_active(&owner).unwrap());
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 1);
        let archived_event = archived.canonical_mutation.as_ref().unwrap();
        assert_eq!(archived_event.aggregate_kind, "memory_retrieval");
        assert_eq!(archived_event.mutation_kind, "archived");
        assert_eq!(
            store
                .projection_summary(&archived_event.event_id)
                .unwrap()
                .pending,
            1
        );

        let replay = store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        assert!(!replay.changed);
        assert_eq!(
            replay.canonical_mutation.as_ref().unwrap().event_id,
            archived_event.event_id
        );

        let restored = store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Active,
                "user_reviewed_restore",
            )
            .unwrap();
        assert!(restored.changed);
        assert_eq!(restored.state.as_ref().unwrap().revision, 2);
        assert!(store.is_memory_retrieval_active(&owner).unwrap());
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 0);
        assert!(store
            .load_memory_retrieval_state_for_projection(&archived_event.event_id)
            .is_err());
        assert_eq!(
            store
                .load_memory_retrieval_state_for_projection(
                    &restored.canonical_mutation.as_ref().unwrap().event_id,
                )
                .unwrap()
                .disposition,
            MemoryRetrievalDisposition::Active
        );
        assert_eq!(
            store.list_archived_memory_retrieval_states(10).unwrap(),
            Vec::<CanonicalMemoryRetrievalState>::new()
        );
    }

    #[test]
    fn canonical_memory_retrieval_state_rolls_back_when_outbox_fails() {
        let store = MemoryStore::new_in_memory().unwrap();
        let note = store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "retrieval-failure-session",
                "canonical retrieval failure body",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner =
            CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                .unwrap();
        store.install_outbox_insert_failure_for_test().unwrap();

        let error = store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .expect_err("state must not commit without its canonical outbox event");

        assert!(error.to_string().contains("injected canonical outbox"));
        assert!(store.memory_retrieval_state(&owner).unwrap().is_none());
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 0);
    }

    #[test]
    fn canonical_memory_retrieval_reason_cannot_copy_user_authored_text() {
        let store = MemoryStore::new_in_memory().unwrap();
        let note = store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "retrieval-reason-session",
                "canonical retrieval reason body",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner =
            CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                .unwrap();

        let error = store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "archive this private sentence: SECRET-123",
            )
            .expect_err("canonical reason must be a bounded code, not user content");

        assert!(error.to_string().contains("reason code"));
        assert!(store.memory_retrieval_state(&owner).unwrap().is_none());
    }

    #[test]
    fn canonical_memory_retrieval_rejects_forged_stable_owner() {
        let store = MemoryStore::new_in_memory().unwrap();
        let forged = CanonicalVectorOwnerRef::new("knowledge_note", "424242").unwrap();

        let error = store
            .set_memory_retrieval_disposition(
                &forged,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .expect_err("a syntactically valid owner is not canonical proof");

        assert!(error.to_string().contains("owner proof"));
        assert!(!store.is_verified_canonical_memory_owner(&forged).unwrap());
        assert!(store.memory_retrieval_state(&forged).unwrap().is_none());
    }

    #[test]
    fn canonical_memory_retrieval_batch_is_all_or_nothing() {
        let store = MemoryStore::new_in_memory().unwrap();
        let mut owners = Vec::new();
        for body in ["batch owner one", "batch owner two"] {
            let note = store
                .save_knowledge_note_idempotent_with_outbox(
                    &uuid::Uuid::new_v4().to_string(),
                    "retrieval-batch-session",
                    body,
                    "knowledge_note",
                    "manual",
                    &[],
                    "private",
                )
                .unwrap();
            owners.push(
                CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                    .unwrap(),
            );
        }
        let forged = CanonicalVectorOwnerRef::new("knowledge_note", "999999").unwrap();
        let mut invalid_batch = owners.clone();
        invalid_batch.push(forged);

        assert!(store
            .set_memory_retrieval_dispositions(
                &invalid_batch,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .is_err());
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 0);
        assert!(owners
            .iter()
            .all(|owner| store.memory_retrieval_state(owner).unwrap().is_none()));

        let second_aggregate = memory_retrieval_aggregate_id(&owners[1]);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch(&format!(
                "CREATE TRIGGER fail_second_retrieval_event_for_test
                 BEFORE INSERT ON canonical_outbox_events
                 WHEN NEW.aggregate_kind = 'memory_retrieval'
                  AND NEW.aggregate_id = '{second_aggregate}'
                 BEGIN
                    SELECT RAISE(ABORT, 'injected second retrieval event failure');
                 END;"
            ))
            .unwrap();
        }
        assert!(store
            .set_memory_retrieval_dispositions(
                &owners,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .is_err());
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 0);
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_second_retrieval_event_for_test;")
            .unwrap();

        let mutations = store
            .set_memory_retrieval_dispositions(
                &owners,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        assert_eq!(mutations.len(), 2);
        assert!(mutations.iter().all(|mutation| mutation.changed));
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 2);
    }

    #[test]
    fn archived_memory_count_and_pages_exceed_legacy_five_hundred_boundary() {
        let store = MemoryStore::new_in_memory().unwrap();
        let mut owners = Vec::with_capacity(501);
        for index in 0..501 {
            let note = store
                .save_knowledge_note_idempotent_with_outbox(
                    &uuid::Uuid::new_v4().to_string(),
                    "retrieval-large-page-session",
                    &format!("canonical archived note {index}"),
                    "knowledge_note",
                    "manual",
                    &[],
                    "private",
                )
                .unwrap();
            owners.push(
                CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                    .unwrap(),
            );
        }
        for batch in owners.chunks(200) {
            store
                .set_memory_retrieval_dispositions(
                    batch,
                    MemoryRetrievalDisposition::Archived,
                    "user_reviewed_archive",
                )
                .unwrap();
        }

        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 501);
        assert_eq!(
            store
                .list_archived_memory_retrieval_states_page(500, 0)
                .unwrap()
                .len(),
            500
        );
        assert_eq!(
            store
                .list_archived_memory_retrieval_states_page(10, 500)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn archived_product_list_excludes_owner_whose_canonical_asset_is_gone() {
        let store = MemoryStore::new_in_memory().unwrap();
        let note = store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "retrieval-current-owner-session",
                "current owner proof body",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner =
            CanonicalVectorOwnerRef::new("knowledge_note", &note.knowledge_note_id.to_string())
                .unwrap();
        store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .unwrap();
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 1);

        store
            .remove_canonical_memory_row_for_corruption_test(note.knowledge_note_id)
            .unwrap();
        assert_eq!(store.count_archived_memory_retrieval_states().unwrap(), 0);
        assert!(store
            .list_archived_memory_retrieval_states(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn memory_store_rejects_and_migrates_lifecycle_retrieval_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-lifecycle-owner-boundary.db");
        let store = MemoryStore::new(&path).unwrap();
        let owner = CanonicalVectorOwnerRef::new("memory_lifecycle", "memory:owner").unwrap();
        assert!(store
            .set_memory_retrieval_disposition(
                &owner,
                MemoryRetrievalDisposition::Archived,
                "user_reviewed_archive",
            )
            .is_err());
        assert!(store.memory_retrieval_state(&owner).is_err());

        let legacy_event_id;
        {
            let mut conn = store.conn.lock().unwrap();
            conn.execute_batch(
                "DROP TRIGGER reject_memory_lifecycle_retrieval_insert;
                 DROP TRIGGER reject_memory_lifecycle_retrieval_update;",
            )
            .unwrap();
            let tx = conn.transaction().unwrap();
            let legacy_reason_digest =
                crate::persistence_outbox::metadata_digest("legacy_lifecycle_archive");
            let receipt = crate::persistence_outbox::enqueue_mutation(
                &tx,
                "memory_retrieval",
                "legacy-lifecycle-owner",
                "archived",
                &legacy_reason_digest,
                &["vector_store"],
            )
            .unwrap();
            legacy_event_id = receipt.event_id.clone();
            tx.execute(
                "INSERT INTO memory_retrieval_states
                    (owner_kind, owner_id, disposition, revision, last_event_id,
                     reason_digest, changed_at)
                 VALUES ('memory_lifecycle', 'memory:owner', 'archived', 1, ?1,
                         ?2, '2026-07-12T00:00:00Z')",
                params![receipt.event_id, legacy_reason_digest],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        drop(store);

        let migrated = MemoryStore::new(&path).unwrap();
        let conn = migrated.conn.lock().unwrap();
        let residual: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memory_retrieval_states
                 WHERE owner_kind = 'memory_lifecycle'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(residual, 0);
        let terminal: String = conn
            .query_row(
                "SELECT terminal_disposition FROM canonical_outbox_deliveries
                 WHERE event_id = ?1 AND projection_target = 'vector_store'",
                [&legacy_event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(terminal, "compensated");
        assert!(conn
            .execute(
                "INSERT INTO memory_retrieval_states
                    (owner_kind, owner_id, disposition, revision, last_event_id,
                     reason_digest, changed_at)
                 VALUES ('memory_lifecycle', 'memory:new', 'archived', 1,
                         'missing', 'sha256:new', '2026-07-12T00:00:00Z')",
                [],
            )
            .is_err());
    }

    #[test]
    fn fts_query_escape_handles_special_syntax_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        save_test_knowledge_note(&store, "s1", "alpha quoted value with 中文 token");

        let long_query = "alpha ".repeat(100);
        for query in [
            "\"quoted\"",
            "alpha OR NEAR * ()",
            "foo\"bar",
            long_query.as_str(),
        ] {
            store.search_text_memories(Some("s1"), query, 5).unwrap();
        }
    }

    #[test]
    fn fts_query_escape_preserves_normal_search() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        save_test_knowledge_note(&store, "s1", "deep work 深度工作 planning");

        let english = store.search_text_memories(Some("s1"), "deep", 5).unwrap();
        assert_eq!(english.len(), 1);

        let chinese = store
            .search_text_memories(Some("s1"), "深度工作", 5)
            .unwrap();
        assert_eq!(chinese.len(), 1);
    }

    #[test]
    fn text_search_is_a_pure_read_of_memory_owner_state() {
        let store = MemoryStore::new_in_memory().unwrap();
        save_test_knowledge_note(&store, "pure-search-session", "PURE_TEXT_SEARCH_RESULT");
        let before_records = store.export_active_memory_records().unwrap();
        let before_rebuild_source = store.vector_rebuild_source_snapshot().unwrap();

        let hits = store
            .search_text_memories(None, "PURE_TEXT_SEARCH_RESULT", 5)
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(
            serde_json::to_value(store.export_active_memory_records().unwrap()).unwrap(),
            serde_json::to_value(&before_records).unwrap()
        );
        assert_eq!(
            store.vector_rebuild_source_snapshot().unwrap(),
            before_rebuild_source,
            "a read must not drift the digest used to bind a vector rebuild source"
        );

        assert_eq!(
            store
                .record_text_search_access_telemetry(&[hits[0].chunk.id, hits[0].chunk.id, -1,])
                .unwrap(),
            1,
            "duplicate and conversation-only hit ids must not overcount telemetry"
        );
        let after_records = store.export_active_memory_records().unwrap();
        assert_eq!(after_records[0].access_count, 1);
        assert!(after_records[0].last_accessed_at.is_some());
        assert_ne!(
            store
                .vector_rebuild_source_snapshot()
                .unwrap()
                .metadata_digest,
            before_rebuild_source.metadata_digest
        );
    }

    #[test]
    fn memory_store_can_save_knowledge_notes() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        save_test_knowledge_note(&store, "manual", "用户偏好在上午处理复杂任务");
        let hits = store.search_text_memories(None, "上午", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(matches!(hits[0].source_tier.as_str(), "fts" | "keyword"));
    }

    #[test]
    fn knowledge_note_and_vector_outbox_are_one_atomic_commit() {
        let store = MemoryStore::new_in_memory().unwrap();
        store.install_outbox_insert_failure_for_test().unwrap();

        store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "manual-session",
                "MANUAL_CANONICAL_SENTINEL",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .expect_err("canonical note must roll back when its outbox insert fails");

        let canonical_rows: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(canonical_rows, 0);
        let operation_rows: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM knowledge_note_operations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(operation_rows, 0);
        assert!(store
            .list_replayable_projection_deliveries(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn knowledge_note_exact_replay_returns_original_canonical_receipt() {
        let store = MemoryStore::new_in_memory().unwrap();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let write = || {
            store.save_knowledge_note_idempotent_with_outbox(
                &operation_id,
                "manual-session",
                "MANUAL_OPERATION_BODY_MUST_NOT_ENTER_JOURNAL",
                "knowledge_note",
                "manual",
                &["source:manual".to_string(), "manual".to_string()],
                "private",
            )
        };

        let first = write().unwrap();
        let replay = write().unwrap();
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.operation_id, operation_id);
        assert_eq!(first.knowledge_note_id, replay.knowledge_note_id);
        assert_eq!(first.operation_digest, replay.operation_digest);
        assert_eq!(
            first.canonical_mutation.event_id,
            replay.canonical_mutation.event_id
        );
        assert_eq!(first.canonical_mutation, replay.canonical_mutation);

        let conn = store.conn.lock().unwrap();
        for table in [
            "memories",
            "knowledge_note_operations",
            "canonical_outbox_events",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 1, "unexpected row count in {table}");
        }
        let journal_text: String = conn
            .query_row(
                "SELECT operation_id || operation_digest || memory_id || outbox_event_id || created_at
                 FROM knowledge_note_operations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!journal_text.contains("MANUAL_OPERATION_BODY_MUST_NOT_ENTER_JOURNAL"));
        assert_ne!(
            first.operation_digest,
            persistence_outbox::metadata_digest("MANUAL_OPERATION_BODY_MUST_NOT_ENTER_JOURNAL"),
            "the operation journal must not persist an enumerable body digest"
        );
        assert_eq!(first.canonical_mutation.aggregate_kind, "knowledge_note");
        assert_eq!(first.canonical_mutation.mutation_kind, "created");
    }

    #[test]
    fn vector_rebuild_owner_requires_durable_relational_proof() {
        let store = MemoryStore::new_in_memory().unwrap();
        let canonical = store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "owner-proof-session",
                "canonical KnowledgeNote",
                "knowledge_note",
                "manual",
                &[
                    "canonical_owner:knowledge_note".into(),
                    "source:manual".into(),
                ],
                "private",
            )
            .unwrap();
        let type_only_id = insert_unowned_test_row(
            &store,
            "owner-proof-session",
            "type alone is not ownership",
            "knowledge_note",
            "manual",
            &[],
        );
        let tag_only_id = insert_unowned_test_row(
            &store,
            "owner-proof-session",
            "tag alone is not ownership",
            "untrusted_legacy",
            "manual",
            &["canonical_owner:knowledge_note".into()],
        );

        let snapshot = store.vector_rebuild_source_snapshot().unwrap();
        let page = store
            .load_vector_rebuild_source_page(0, snapshot.through_memory_id, 10)
            .unwrap();
        let owner_for = |memory_id| {
            page.iter()
                .find(|record| record.memory.id == memory_id)
                .unwrap()
                .canonical_owner
                .as_ref()
        };
        let owner = owner_for(canonical.knowledge_note_id).unwrap();
        assert_eq!(owner.kind(), "knowledge_note");
        assert_eq!(owner.id(), canonical.knowledge_note_id.to_string());
        assert!(owner_for(type_only_id).is_none());
        assert!(owner_for(tag_only_id).is_none());
    }

    #[test]
    fn knowledge_note_projection_loader_binds_event_owner_and_row_contract() {
        let store = MemoryStore::new_in_memory().unwrap();
        let write = store
            .save_knowledge_note_idempotent_with_outbox(
                &uuid::Uuid::new_v4().to_string(),
                "projection-proof-session",
                "verified canonical KnowledgeNote",
                "knowledge_note",
                "manual",
                &[],
                "private",
            )
            .unwrap();
        let owner =
            CanonicalVectorOwnerRef::new("knowledge_note", &write.knowledge_note_id.to_string())
                .unwrap();
        let record = store
            .load_verified_knowledge_note_projection(&write.canonical_mutation.event_id, &owner)
            .unwrap();
        assert_eq!(record.id, write.knowledge_note_id);
        assert!(store
            .load_verified_knowledge_note_projection("wrong-event", &owner)
            .is_err());
        assert!(CanonicalVectorOwnerRef::new(
            "memory_record",
            &write.knowledge_note_id.to_string(),
        )
        .is_err());

        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE memories SET privacy_level = 'public' WHERE id = ?1",
                [write.knowledge_note_id],
            )
            .unwrap();
        assert!(store
            .load_verified_knowledge_note_projection(&write.canonical_mutation.event_id, &owner,)
            .is_err());
    }

    #[test]
    fn knowledge_note_operation_id_reuse_with_payload_drift_fails_closed() {
        let variants = [
            (
                "other-session",
                "canonical body",
                "knowledge_note",
                "manual",
                vec!["manual".to_string()],
                "private",
                "KnowledgeNote operation id was reused with a different canonical payload",
            ),
            (
                "manual-session",
                "different body",
                "knowledge_note",
                "manual",
                vec!["manual".to_string()],
                "private",
                "KnowledgeNote operation id was reused with a different canonical payload",
            ),
            (
                "manual-session",
                "canonical body",
                "other-type",
                "manual",
                vec!["manual".to_string()],
                "private",
                "KnowledgeNote admission requires the private typed-note contract",
            ),
            (
                "manual-session",
                "canonical body",
                "knowledge_note",
                "other-source",
                vec!["manual".to_string()],
                "private",
                "KnowledgeNote operation id was reused with a different canonical payload",
            ),
            (
                "manual-session",
                "canonical body",
                "knowledge_note",
                "manual",
                vec!["changed".to_string()],
                "private",
                "KnowledgeNote operation id was reused with a different canonical payload",
            ),
            (
                "manual-session",
                "canonical body",
                "knowledge_note",
                "manual",
                vec!["manual".to_string()],
                "sensitive",
                "KnowledgeNote admission requires the private typed-note contract",
            ),
        ];

        for (session_id, content, content_type, source, tags, privacy_level, expected_error) in
            variants
        {
            let store = MemoryStore::new_in_memory().unwrap();
            let operation_id = uuid::Uuid::new_v4().to_string();
            store
                .save_knowledge_note_idempotent_with_outbox(
                    &operation_id,
                    "manual-session",
                    "canonical body",
                    "knowledge_note",
                    "manual",
                    &["manual".to_string()],
                    "private",
                )
                .unwrap();
            let error = store
                .save_knowledge_note_idempotent_with_outbox(
                    &operation_id,
                    session_id,
                    content,
                    content_type,
                    source,
                    &tags,
                    privacy_level,
                )
                .expect_err("operation id reuse must bind the complete canonical payload");
            assert_eq!(error.to_string(), expected_error);
        }
    }

    #[test]
    fn knowledge_note_requires_uuid_v4_operation_id() {
        let store = MemoryStore::new_in_memory().unwrap();
        let uppercase_v4 = uuid::Uuid::new_v4().to_string().to_uppercase();
        for invalid in [
            "not-a-uuid",
            "00000000-0000-0000-0000-000000000000",
            uppercase_v4.as_str(),
        ] {
            let error = store
                .save_knowledge_note_idempotent_with_outbox(
                    invalid,
                    "manual-session",
                    "canonical body",
                    "knowledge_note",
                    "manual",
                    &[],
                    "private",
                )
                .expect_err("non-v4 operation ids must be rejected");
            assert!(error.to_string().contains("operation id"));
        }
    }

    #[test]
    fn knowledge_note_rejects_unbounded_or_untyped_metadata_before_writing() {
        let store = MemoryStore::new_in_memory().unwrap();
        let cases = [
            (
                "session",
                "body".to_string(),
                "knowledge_note",
                "source\0label".to_string(),
                vec![],
                "private",
            ),
            (
                "session",
                "x".repeat(KNOWLEDGE_NOTE_MAX_CONTENT_BYTES + 1),
                "knowledge_note",
                "manual".to_string(),
                vec![],
                "private",
            ),
            (
                "session",
                "body".to_string(),
                "knowledge_note",
                "source\u{202e}label".to_string(),
                vec![],
                "private",
            ),
            (
                "session",
                "body".to_string(),
                "arbitrary_type",
                "manual".to_string(),
                vec![],
                "private",
            ),
            (
                "session",
                "body".to_string(),
                "knowledge_note",
                "manual".to_string(),
                vec!["tag".to_string(); KNOWLEDGE_NOTE_MAX_TAGS + 1],
                "private",
            ),
            (
                "session",
                "body".to_string(),
                "knowledge_note",
                "manual".to_string(),
                vec![],
                "public",
            ),
        ];

        for (session_id, content, content_type, source, tags, privacy_level) in cases {
            store
                .save_knowledge_note_idempotent_with_outbox(
                    &uuid::Uuid::new_v4().to_string(),
                    session_id,
                    &content,
                    content_type,
                    &source,
                    &tags,
                    privacy_level,
                )
                .expect_err("invalid manual index metadata must fail before canonical mutation");
        }

        let conn = store.conn.lock().unwrap();
        for table in [
            "memories",
            "knowledge_note_operations",
            "canonical_outbox_events",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "invalid payload mutated {table}");
        }
    }

    #[test]
    fn concurrent_knowledge_note_retries_commit_one_canonical_write() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("concurrent-memory-index.db");
        let stores = (0..8)
            .map(|_| MemoryStore::new(&db_path).unwrap())
            .collect::<Vec<_>>();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(stores.len()));
        let handles = stores
            .into_iter()
            .map(|store| {
                let barrier = std::sync::Arc::clone(&barrier);
                let operation_id = operation_id.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store
                        .save_knowledge_note_idempotent_with_outbox(
                            &operation_id,
                            "manual-session",
                            "CONCURRENT_CANONICAL_BODY",
                            "knowledge_note",
                            "manual",
                            &["manual".to_string()],
                            "private",
                        )
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let writes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(writes.iter().filter(|write| !write.replayed).count(), 1);
        assert!(writes
            .iter()
            .all(|write| write.knowledge_note_id == writes[0].knowledge_note_id));
        assert!(writes.iter().all(|write| {
            write.canonical_mutation.event_id == writes[0].canonical_mutation.event_id
        }));

        let conn = Connection::open(&db_path).unwrap();
        for table in [
            "memories",
            "knowledge_note_operations",
            "canonical_outbox_events",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 1, "concurrent retries duplicated {table}");
        }
    }

    #[test]
    fn vector_rebuild_source_uses_stable_bounded_memory_id_pages() {
        let store = MemoryStore::new_in_memory().unwrap();
        for index in 0..130 {
            save_test_knowledge_note(&store, "memory-session", &format!("memory-{index}"));
        }

        let snapshot = store.vector_rebuild_source_snapshot().unwrap();
        assert_eq!(snapshot.total_count, 130);
        let first = store
            .load_vector_rebuild_source_page(0, snapshot.through_memory_id, 50)
            .unwrap();
        let second = store
            .load_vector_rebuild_source_page(
                first.last().unwrap().memory.id,
                snapshot.through_memory_id,
                50,
            )
            .unwrap();
        let final_page = store
            .load_vector_rebuild_source_page(
                second.last().unwrap().memory.id,
                snapshot.through_memory_id,
                usize::MAX,
            )
            .unwrap();

        assert_eq!(first.len(), 50);
        assert_eq!(second.len(), 50);
        assert_eq!(final_page.len(), 30);
        assert!(first
            .windows(2)
            .all(|pair| pair[0].memory.id < pair[1].memory.id));
        assert!(first
            .iter()
            .chain(second.iter())
            .chain(final_page.iter())
            .all(|record| !record
                .memory
                .content
                .contains("CONVERSATION_BODY_MUST_NOT_ENTER_REBUILD_PAGE")));
        assert_eq!(
            store
                .vector_rebuild_source_snapshot_through(snapshot.through_memory_id)
                .unwrap(),
            snapshot
        );
    }
}
