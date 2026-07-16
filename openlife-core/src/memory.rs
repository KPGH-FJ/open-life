use crate::agent::{AgentRun, AgentRunStore};
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::persistence_outbox::{
    self, CanonicalMutationReceipt, ProjectionDelivery, ProjectionSummary,
};
use crate::state_store::StateHistoryEntry;
use crate::vectors::{
    CanonicalVectorOwnerRef, MemoryChunk, VectorRebuildSourceSnapshot, VECTOR_REBUILD_BATCH_LIMIT,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
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
const LEGACY_STATE_HISTORY_MIGRATION_MAX_ROWS: usize = 50_000;

#[derive(Clone)]
pub struct MemoryStore {
    conn: Arc<Mutex<Connection>>,
    canonical_store_identity: Arc<str>,
}

const MEMORY_STORE_IDENTITY_PREFIX: &str = "memory_store:v1:";

fn is_canonical_memory_store_identity(value: &str) -> bool {
    value
        .strip_prefix(MEMORY_STORE_IDENTITY_PREFIX)
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
}

fn load_or_create_memory_store_identity(conn: &Connection) -> Result<String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_store_metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        ) WITHOUT ROWID",
        [],
    )?;
    let existing = conn
        .query_row(
            "SELECT value FROM memory_store_metadata WHERE key = 'canonical_store_identity'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        if !is_canonical_memory_store_identity(&existing) {
            anyhow::bail!("memory_store_canonical_identity_invalid");
        }
        return Ok(existing);
    }
    let identity = format!("{MEMORY_STORE_IDENTITY_PREFIX}{}", uuid::Uuid::new_v4());
    conn.execute(
        "INSERT INTO memory_store_metadata(key, value)
         VALUES ('canonical_store_identity', ?1)",
        [&identity],
    )?;
    Ok(identity)
}

fn load_existing_memory_store_identity(conn: &Connection) -> Result<String> {
    let identity = conn
        .query_row(
            "SELECT value FROM memory_store_metadata WHERE key = 'canonical_store_identity'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .context("memory_store_canonical_identity_missing")?;
    if !is_canonical_memory_store_identity(&identity) {
        anyhow::bail!("memory_store_canonical_identity_invalid");
    }
    Ok(identity)
}

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

fn conversation_search_terms(query: &str) -> Vec<String> {
    fn is_stopword(term: &str) -> bool {
        matches!(
            term,
            "a" | "an"
                | "the"
                | "about"
                | "did"
                | "do"
                | "find"
                | "i"
                | "me"
                | "please"
                | "prior"
                | "previous"
                | "search"
                | "session"
                | "sessions"
                | "tell"
                | "we"
                | "what"
                | "discuss"
                | "discussed"
                | "conversation"
                | "conversations"
                | "我们"
                | "之前"
                | "讨论"
                | "讨论过"
                | "请"
                | "查找"
                | "会话"
        )
    }

    let bounded = query
        .trim()
        .chars()
        .take(FTS_QUERY_MAX_CHARS)
        .collect::<String>()
        .to_lowercase();
    let mut terms = Vec::new();
    for segment in bounded.split(|character: char| !character.is_alphanumeric()) {
        if segment.is_empty() {
            continue;
        }
        let characters = segment.chars().collect::<Vec<_>>();
        let contains_cjk = characters
            .iter()
            .any(|character| ('\u{4e00}'..='\u{9fff}').contains(character));
        if contains_cjk && characters.len() > 2 {
            for pair in characters.windows(2) {
                let term = pair.iter().collect::<String>();
                if !is_stopword(&term) {
                    terms.push(term);
                }
            }
        } else if !is_stopword(segment) && (segment.chars().count() >= 2 || contains_cjk) {
            terms.push(segment.to_string());
        }
        if terms.len() >= FTS_QUERY_MAX_TOKENS {
            break;
        }
    }
    let mut seen = HashSet::new();
    terms.retain(|term| seen.insert(term.clone()));
    terms.truncate(FTS_QUERY_MAX_TOKENS);
    terms
}

fn ensure_conversation_write_allowed(conn: &Connection, session_id: &str) -> Result<()> {
    let session_id = session_id.trim();
    if session_id.is_empty() || session_id.len() > 256 {
        anyhow::bail!("invalid conversation session id");
    }
    if persistence_outbox::has_active_tombstone(conn, "conversation", session_id)? {
        anyhow::bail!("conversation_canonical_tombstoned");
    }
    Ok(())
}

fn update_length_delimited_digest(context: &mut DigestContext, bytes: &[u8]) {
    context.update(&(bytes.len() as u64).to_le_bytes());
    context.update(bytes);
}

fn conversation_content_digest(content: &str) -> String {
    let hash = digest(&SHA256, content.as_bytes());
    let hex = hash
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn ensure_message_operation_id(operation_id: &str) -> Result<()> {
    if operation_id.trim() != operation_id
        || operation_id.is_empty()
        || operation_id.len() > 256
        || !operation_id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'/' | b'.' | b'_' | b'-')
        })
    {
        anyhow::bail!("invalid canonical conversation message operation id");
    }
    Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalConversationMessageReceipt {
    pub message_id: i64,
    pub session_id: String,
    pub role: String,
    pub operation_id: String,
    pub canonical_ref: String,
    pub content_digest: String,
    pub content_length_bytes: usize,
    pub replayed: bool,
}

/// Non-serializable evidence that a canonical conversation row was observed
/// inside `MemoryStore`'s commit transaction.  The fields are deliberately
/// private: callers may carry this proof to another in-process store, but they
/// cannot manufacture one from an IPC/JSON receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalConversationMessageProof {
    message_id: i64,
    session_id: String,
    role: String,
    canonical_store_identity: Arc<str>,
    canonical_ref: String,
    content_digest: String,
}

impl CanonicalConversationMessageProof {
    pub(crate) fn message_id(&self) -> i64 {
        self.message_id
    }

    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    pub(crate) fn role(&self) -> &str {
        &self.role
    }

    pub(crate) fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }

    pub(crate) fn canonical_ref(&self) -> &str {
        &self.canonical_ref
    }

    pub(crate) fn content_digest(&self) -> &str {
        &self.content_digest
    }
}

/// In-process commit result used when a downstream canonical store must prove
/// ownership.  `receipt` remains safe to serialize separately; `proof` is an
/// opaque capability and has no serde implementation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalConversationMessageCommit {
    receipt: CanonicalConversationMessageReceipt,
    proof: CanonicalConversationMessageProof,
}

#[derive(Debug, Clone)]
pub struct CanonicalConversationMessageRecord {
    pub message: ChatMessage,
    pub receipt: CanonicalConversationMessageReceipt,
}

impl CanonicalConversationMessageCommit {
    pub fn receipt(&self) -> &CanonicalConversationMessageReceipt {
        &self.receipt
    }

    pub fn proof(&self) -> &CanonicalConversationMessageProof {
        &self.proof
    }

    pub fn into_receipt(self) -> CanonicalConversationMessageReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetrievalDisposition {
    Active,
    Archived,
}

impl MemoryRetrievalDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
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

impl MemoryStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open sqlite db at {:?}", db_path))?;
        configure_memory_store_connection(&conn, true)?;
        let canonical_store_identity = load_or_create_memory_store_identity(&conn)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            canonical_store_identity: Arc::from(canonical_store_identity),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory sqlite db")?;
        configure_memory_store_connection(&conn, false)?;
        let canonical_store_identity = load_or_create_memory_store_identity(&conn)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            canonical_store_identity: Arc::from(canonical_store_identity),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "memory_store",
            &["messages", "memories", "memory_store_metadata"],
        )?;
        let canonical_store_identity = load_existing_memory_store_identity(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            canonical_store_identity: Arc::from(canonical_store_identity),
        })
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        Ok(Self {
            conn: Arc::new(Mutex::new(
                crate::sqlite_migration::unavailable_read_only_sentinel("memory_store")?,
            )),
            canonical_store_identity: Arc::from("memory_store:unavailable"),
        })
    }

    pub(crate) fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                embedding_id INTEGER
            )",
            [],
        )?;
        Self::ensure_column_exists(&conn, "messages", "embedding_id", "INTEGER")?;
        Self::ensure_column_exists(&conn, "messages", "operation_id", "TEXT")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_messages_embedding_id ON messages(embedding_id)",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_operation_id
             ON messages(operation_id) WHERE operation_id IS NOT NULL",
            [],
        )?;
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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                life_model_yaml TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_snapshots_session ON snapshots(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS chat_sessions (
                session_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chat_sessions_updated ON chat_sessions(updated_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS state_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dimension_name TEXT NOT NULL,
                value REAL NOT NULL,
                unit TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                note TEXT,
                operation_id TEXT,
                operation_digest TEXT
            )",
            [],
        )?;
        Self::ensure_column_exists(&conn, "state_history", "operation_id", "TEXT")?;
        Self::ensure_column_exists(&conn, "state_history", "operation_digest", "TEXT")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_state_history_dimension ON state_history(dimension_name, recorded_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_state_history_operation_id
             ON state_history(operation_id) WHERE operation_id IS NOT NULL",
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
        Self::migrate_legacy_memory_index_operation_journal(&mut conn)?;
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_knowledge_note_operation_memory
             ON knowledge_note_operations(memory_id)",
            [],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_materialization_projections (
                event_id TEXT PRIMARY KEY,
                aggregate_kind TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                mutation_kind TEXT NOT NULL,
                memory_row_id INTEGER,
                applied_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_memory_materialization_aggregate
             ON memory_materialization_projections(aggregate_kind, aggregate_id);

             DELETE FROM memories
             WHERE EXISTS (
                 SELECT 1 FROM memory_materialization_projections projections
                 WHERE projections.aggregate_kind = 'memory_lifecycle'
                   AND projections.mutation_kind = 'deleted'
                   AND (
                       memories.source = 'memory_lifecycle:' || projections.aggregate_id
                       OR (
                           EXISTS (
                               SELECT 1 FROM json_each(
                                   CASE WHEN json_valid(memories.tags_json)
                                        THEN memories.tags_json ELSE '[]' END
                               ) owner_tag
                               WHERE owner_tag.value = 'canonical_owner:memory_lifecycle'
                           )
                           AND EXISTS (
                               SELECT 1 FROM json_each(
                                   CASE WHEN json_valid(memories.tags_json)
                                        THEN memories.tags_json ELSE '[]' END
                               ) memory_tag
                               WHERE memory_tag.value = 'memory_id:' || projections.aggregate_id
                           )
                       )
                   )
             );",
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
        // Conversation text has one canonical owner: `messages`. Older releases mirrored every
        // turn into long-term `memories`; remove that derived duplicate while preserving the
        // canonical conversation row and explicit/accepted memory assets.
        conn.execute(
            "DELETE FROM memories WHERE content_type = 'chat_message'",
            [],
        )?;
        crate::sqlite_migration::record_schema_version(&conn, "memory_store", 6)?;
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

    /// Retire the pre-KnowledgeNote command journal. Older releases stored a
    /// deterministic payload digest; the typed journal verifies replay by
    /// loading the canonical row and retains only a nonce-bound token.
    fn migrate_legacy_memory_index_operation_journal(conn: &mut Connection) -> Result<()> {
        let legacy_exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'memory_index_operations'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !legacy_exists {
            return Ok(());
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DROP INDEX IF EXISTS idx_memory_index_operation_memory", [])?;
        let legacy_rows = {
            let mut statement = tx.prepare(
                "SELECT operation_id, memory_id, outbox_event_id, created_at
                 FROM memory_index_operations
                 ORDER BY operation_id ASC",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        for (operation_id, memory_id, outbox_event_id, created_at) in legacy_rows {
            let operation_digest = persistence_outbox::metadata_digest(&format!(
                "knowledge_note_operation_migration:{operation_id}:{}",
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
                    outbox_event_id,
                    created_at
                ],
            )?;
        }
        tx.execute("DROP TABLE memory_index_operations", [])?;
        tx.commit()?;
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

    #[allow(clippy::too_many_arguments)]
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

    pub fn save_message(&self, session_id: &str, msg: &ChatMessage) -> Result<i64> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        ensure_conversation_write_allowed(&tx, session_id)?;
        let created_at = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, msg.role, msg.content, &created_at],
        )?;
        let message_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(message_id)
    }

    /// Idempotently commits one canonical conversation body. `operation_id`
    /// is a caller-owned execution fact, not a content-derived key: replaying
    /// the same operation returns the original row, while reusing it for a
    /// different session, role, or body fails closed.
    pub fn save_message_idempotent(
        &self,
        session_id: &str,
        msg: &ChatMessage,
        operation_id: &str,
    ) -> Result<CanonicalConversationMessageReceipt> {
        self.save_message_idempotent_internal(session_id, msg, operation_id)
            .map(CanonicalConversationMessageCommit::into_receipt)
    }

    /// Same canonical commit as `save_message_idempotent`, plus an opaque
    /// in-process proof for a downstream store that must bind its reference to
    /// the row that MemoryStore actually committed.
    pub fn save_message_idempotent_with_proof(
        &self,
        session_id: &str,
        msg: &ChatMessage,
        operation_id: &str,
    ) -> Result<CanonicalConversationMessageCommit> {
        if msg.role != "user" {
            anyhow::bail!("canonical_user_message_proof_requires_role_user");
        }
        self.save_message_idempotent_internal(session_id, msg, operation_id)
    }

    /// Revalidates a previously issued conversation proof and commits the
    /// referencing AgentRun while the canonical Memory connection remains
    /// fenced. The lock order is always MemoryStore -> AgentRunStore and the
    /// closure contains no external await, so a conversation delete cannot
    /// enter the proof-check/AgentRun-insert window.
    pub fn create_agent_run_from_active_conversation_message(
        &self,
        agent_run_store: &AgentRunStore,
        run: &AgentRun,
        proof: &CanonicalConversationMessageProof,
    ) -> Result<()> {
        self.create_agent_run_from_active_conversation_message_internal(
            agent_run_store,
            run,
            proof,
            || {},
        )
    }

    /// Production core seam for the non-interruptive, low-risk LifeEvent lane.
    /// Authorization is derived only from the exact active canonical user
    /// message, the deterministic candidate router, and the exact AgentRun
    /// execution owner. Caller strings are lookup keys, never authority.
    pub fn create_low_risk_life_event_from_active_user_message(
        &self,
        agent_run_store: &AgentRunStore,
        life_event_store: &crate::agent::LifeEventStore,
        message_proof: &CanonicalConversationMessageProof,
        candidate_id: &str,
        run_id: &str,
        operation_id: &str,
    ) -> Result<crate::agent::LifeEvent> {
        if message_proof.canonical_store_identity() != self.canonical_store_identity.as_ref()
            || message_proof.role() != "user"
        {
            anyhow::bail!("life_event_create_current_user_message_owner_mismatch");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if persistence_outbox::has_active_tombstone(
            &tx,
            "conversation",
            message_proof.session_id(),
        )? {
            anyhow::bail!("life_event_create_current_user_message_tombstoned");
        }
        let current = tx
            .query_row(
                "SELECT session_id, role, content FROM messages WHERE id = ?1",
                [message_proof.message_id()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .context("life_event_create_current_user_message_missing")?;
        let (session_id, role, content) = current;
        let canonical_ref = format!(
            "conversation://{session_id}/message/{}",
            message_proof.message_id()
        );
        if session_id != message_proof.session_id()
            || role != "user"
            || canonical_ref != message_proof.canonical_ref()
            || conversation_content_digest(&content) != message_proof.content_digest()
        {
            anyhow::bail!("life_event_create_current_user_message_proof_stale");
        }
        let policy_proof =
            crate::agent::main_chat_memory_candidate::issue_deterministic_life_event_policy_proof(
                message_proof,
                &content,
                candidate_id,
            )?;
        let event = agent_run_store.create_low_risk_life_event_from_authorities(
            life_event_store,
            message_proof,
            policy_proof,
            run_id,
            operation_id,
        )?;
        tx.commit()?;
        Ok(event)
    }

    /// Resolve a task/session reference back to its one canonical body. This
    /// is used only to hydrate transient runtime state after restart; callers
    /// receive no body when the conversation is tombstoned or the reference no
    /// longer names the exact row.
    pub fn load_active_conversation_message_by_ref(
        &self,
        canonical_ref: &str,
    ) -> Result<Option<ChatMessage>> {
        let Some(reference) = canonical_ref.strip_prefix("conversation://") else {
            anyhow::bail!("invalid_canonical_conversation_message_ref");
        };
        let Some((session_id, message_id)) = reference.rsplit_once("/message/") else {
            anyhow::bail!("invalid_canonical_conversation_message_ref");
        };
        let message_id = message_id
            .parse::<i64>()
            .context("invalid_canonical_conversation_message_id")?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        if persistence_outbox::has_active_tombstone(&conn, "conversation", session_id)? {
            return Ok(None);
        }
        conn.query_row(
            "SELECT role, content FROM messages
             WHERE id = ?1 AND session_id = ?2",
            params![message_id, session_id],
            |row| {
                Ok(ChatMessage {
                    role: row.get(0)?,
                    content: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// Resolves an idempotent conversation operation back to the canonical
    /// body owner. Execution/event stores may retain this operation and its
    /// receipt, but recovery must rehydrate the body only from Conversation.
    pub fn load_active_conversation_message_by_operation(
        &self,
        operation_id: &str,
    ) -> Result<Option<CanonicalConversationMessageRecord>> {
        ensure_message_operation_id(operation_id)?;
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let row = conn
            .query_row(
                "SELECT id, session_id, role, content
                 FROM messages WHERE operation_id = ?1",
                [operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((message_id, session_id, role, content)) = row else {
            return Ok(None);
        };
        if persistence_outbox::has_active_tombstone(&conn, "conversation", &session_id)? {
            return Ok(None);
        }
        let content_digest = conversation_content_digest(&content);
        Ok(Some(CanonicalConversationMessageRecord {
            message: ChatMessage {
                role: role.clone(),
                content: content.clone(),
            },
            receipt: CanonicalConversationMessageReceipt {
                message_id,
                session_id: session_id.clone(),
                role,
                operation_id: operation_id.to_string(),
                canonical_ref: format!("conversation://{session_id}/message/{message_id}"),
                content_digest,
                content_length_bytes: content.len(),
                replayed: true,
            },
        }))
    }

    fn create_agent_run_from_active_conversation_message_internal(
        &self,
        agent_run_store: &AgentRunStore,
        run: &AgentRun,
        proof: &CanonicalConversationMessageProof,
        before_agent_run_insert: impl FnOnce(),
    ) -> Result<()> {
        if proof.canonical_store_identity() != self.canonical_store_identity.as_ref()
            || proof.role() != "user"
        {
            anyhow::bail!("canonical_conversation_message_proof_store_mismatch");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_conversation_write_allowed(&tx, proof.session_id())?;
        let current = tx
            .query_row(
                "SELECT session_id, role, content
                 FROM messages WHERE id = ?1",
                [proof.message_id()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((session_id, role, content)) = current else {
            anyhow::bail!("canonical_conversation_message_proof_stale");
        };
        let canonical_ref = format!(
            "conversation://{}/message/{}",
            session_id,
            proof.message_id()
        );
        if session_id != proof.session_id()
            || role != proof.role()
            || canonical_ref != proof.canonical_ref()
            || conversation_content_digest(&content) != proof.content_digest()
        {
            anyhow::bail!("canonical_conversation_message_proof_stale");
        }

        // Test barriers may pause here to prove that a concurrent delete is
        // excluded until the AgentRun insert completes. Production passes a
        // no-op closure and performs no external work while holding the fence.
        before_agent_run_insert();
        agent_run_store.create_run_with_input_proof(run, proof)?;
        tx.commit()?;
        Ok(())
    }

    fn save_message_idempotent_internal(
        &self,
        session_id: &str,
        msg: &ChatMessage,
        operation_id: &str,
    ) -> Result<CanonicalConversationMessageCommit> {
        ensure_message_operation_id(operation_id)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        ensure_conversation_write_allowed(&tx, session_id)?;
        let created_at = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "INSERT OR IGNORE INTO messages
                (session_id, role, content, created_at, operation_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, msg.role, msg.content, created_at, operation_id],
        )?;
        let replayed = changed == 0;
        let message_id = if changed == 1 {
            tx.last_insert_rowid()
        } else {
            let existing = tx
                .query_row(
                    "SELECT id, session_id, role, content FROM messages WHERE operation_id = ?1",
                    [operation_id],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .optional()?;
            let Some((message_id, existing_session, existing_role, existing_content)) = existing
            else {
                anyhow::bail!(
                    "conversation message insert was ignored without a canonical operation owner"
                );
            };
            if existing_session != session_id
                || existing_role != msg.role
                || existing_content != msg.content
            {
                anyhow::bail!(
                    "conversation message operation id was reused with a different canonical payload"
                );
            }
            message_id
        };
        tx.commit()?;
        let content_digest = conversation_content_digest(&msg.content);
        let canonical_ref = format!("conversation://{session_id}/message/{message_id}");
        let receipt = CanonicalConversationMessageReceipt {
            message_id,
            session_id: session_id.to_string(),
            role: msg.role.clone(),
            operation_id: operation_id.to_string(),
            canonical_ref: canonical_ref.clone(),
            content_digest: content_digest.clone(),
            content_length_bytes: msg.content.len(),
            replayed,
        };
        Ok(CanonicalConversationMessageCommit {
            proof: CanonicalConversationMessageProof {
                message_id,
                session_id: session_id.to_string(),
                role: msg.role.clone(),
                canonical_store_identity: Arc::clone(&self.canonical_store_identity),
                canonical_ref,
                content_digest,
            },
            receipt,
        })
    }

    pub fn load_recent_messages(&self, session_id: &str, limit: usize) -> Result<Vec<ChatMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT role, content FROM messages
             WHERE session_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit as i64], |row| {
            Ok(ChatMessage {
                role: row.get(0)?,
                content: row.get(1)?,
            })
        })?;
        let mut messages: Vec<ChatMessage> = rows.collect::<Result<Vec<_>, _>>()?;
        messages.reverse();
        Ok(messages)
    }

    pub fn save_snapshot(&self, session_id: &str, model: &LifeModel) -> Result<i64> {
        let yaml = serde_yaml::to_string(model)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        ensure_conversation_write_allowed(&tx, session_id)?;
        tx.execute(
            "INSERT INTO snapshots (session_id, life_model_yaml, created_at) VALUES (?1, ?2, ?3)",
            params![session_id, yaml, Utc::now().to_rfc3339()],
        )?;
        let snapshot_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(snapshot_id)
    }

    pub fn load_latest_snapshot(&self, session_id: &str) -> Result<Option<LifeModel>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT life_model_yaml FROM snapshots WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![session_id])?;
        if let Some(row) = rows.next()? {
            let yaml: String = row.get(0)?;
            let model: LifeModel = serde_yaml::from_str(&yaml)?;
            Ok(Some(model))
        } else {
            Ok(None)
        }
    }

    pub fn list_sessions(&self, limit: usize) -> Result<Vec<(String, DateTime<Utc>)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, MAX(created_at) as last_at FROM messages GROUP BY session_id ORDER BY last_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            let sid: String = row.get(0)?;
            let last_at: String = row.get(1)?;
            let dt = DateTime::parse_from_rfc3339(&last_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok((sid, dt))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to collect sessions")
    }

    pub fn export_all_messages(&self) -> Result<Vec<ExportedMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, role, content, created_at FROM messages ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ExportedMessage {
                session_id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to export messages")
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
        let knowledge_owner = match knowledge_contract {
            Some((kind, aggregate_id, mutation_kind))
                if aggregate_id == memory_id.to_string()
                    && kind == "knowledge_note"
                    && mutation_kind == "created" =>
            {
                Some(CanonicalVectorOwnerRef::new(
                    "knowledge_note",
                    &aggregate_id,
                )?)
            }
            Some((kind, aggregate_id, mutation_kind))
                if aggregate_id == memory_id.to_string()
                    && kind == "memory_record"
                    && mutation_kind == "indexed" =>
            {
                Some(CanonicalVectorOwnerRef::new(
                    "memory_record",
                    &aggregate_id,
                )?)
            }
            Some(_) => anyhow::bail!("KnowledgeNote rebuild ownership proof is inconsistent"),
            None => None,
        };

        let mut lifecycle_statement = conn.prepare(
            "SELECT DISTINCT aggregate_id
             FROM memory_materialization_projections
             WHERE memory_row_id = ?1
               AND aggregate_kind = 'memory_lifecycle'
               AND mutation_kind = 'materialized'
             ORDER BY aggregate_id LIMIT 2",
        )?;
        let lifecycle_ids = lifecycle_statement
            .query_map([memory_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let lifecycle_owner = match lifecycle_ids.as_slice() {
            [] => None,
            [memory_id] => Some(CanonicalVectorOwnerRef::new("memory_lifecycle", memory_id)?),
            _ => anyhow::bail!("MemoryLifecycle rebuild ownership proof is ambiguous"),
        };
        match (knowledge_owner, lifecycle_owner) {
            (Some(_), Some(_)) => anyhow::bail!("vector rebuild row has multiple canonical owners"),
            (Some(owner), None) | (None, Some(owner)) => Ok(Some(owner)),
            (None, None) => Ok(None),
        }
    }

    fn validate_vector_rebuild_owner_contract(
        memory: &MemoryRecord,
        owner: Option<&CanonicalVectorOwnerRef>,
    ) -> Result<()> {
        let has_tag = |expected: &str| memory.tags.iter().any(|tag| tag == expected);
        match owner {
            Some(owner) if owner.kind() == "knowledge_note" => {
                if owner.id() != memory.id.to_string()
                    || memory.content_type != "knowledge_note"
                    || memory.privacy_level != "private"
                {
                    anyhow::bail!("KnowledgeNote rebuild row does not match its canonical proof");
                }
            }
            Some(owner) if owner.kind() == "memory_record" => {
                if owner.id() != memory.id.to_string()
                    || memory.content_type != "knowledge_note"
                    || memory.privacy_level != "private"
                {
                    anyhow::bail!("legacy Memory record rebuild proof is inconsistent");
                }
            }
            Some(owner) if owner.kind() == "memory_lifecycle" => {
                if memory.source != owner.source()
                    || memory.content_type != "lifecycle_memory_projection"
                    || memory.privacy_level != "private"
                    || !has_tag("canonical_owner:memory_lifecycle")
                    || !has_tag(&format!("memory_id:{}", owner.id()))
                {
                    anyhow::bail!("MemoryLifecycle rebuild row does not match its canonical proof");
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
        let memory_id = match owner.kind() {
            "knowledge_note" | "memory_record" => owner
                .id()
                .parse::<i64>()
                .context("KnowledgeNote owner id is not a canonical row id")?,
            "memory_lifecycle" => {
                let mut statement = conn.prepare(
                    "SELECT DISTINCT memory_row_id
                     FROM memory_materialization_projections
                     WHERE aggregate_kind = 'memory_lifecycle'
                       AND aggregate_id = ?1
                       AND mutation_kind = 'materialized'
                       AND memory_row_id IS NOT NULL
                     ORDER BY memory_row_id LIMIT 2",
                )?;
                let ids = statement
                    .query_map([owner.id()], |row| row.get::<_, i64>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                match ids.as_slice() {
                    [memory_id] => *memory_id,
                    [] => return Ok(None),
                    _ => anyhow::bail!("canonical Memory owner proof is ambiguous"),
                }
            }
            _ => anyhow::bail!("unsupported canonical Memory owner kind"),
        };
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

    pub fn clear_all_messages(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute("DELETE FROM messages", [])?;
        conn.execute(
            "DELETE FROM memories WHERE content_type = 'chat_message'",
            [],
        )?;
        Ok(())
    }

    pub fn import_messages(&self, messages: &[ExportedMessage]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        for session_id in messages
            .iter()
            .map(|message| message.session_id.as_str())
            .collect::<HashSet<_>>()
        {
            ensure_conversation_write_allowed(&tx, session_id)?;
        }
        for msg in messages {
            tx.execute(
                "INSERT INTO messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![msg.session_id, msg.role, msg.content, msg.created_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_all_messages(&self, messages: &[ExportedMessage]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        for session_id in messages
            .iter()
            .map(|message| message.session_id.as_str())
            .collect::<HashSet<_>>()
        {
            ensure_conversation_write_allowed(&tx, session_id)?;
        }
        tx.execute("DELETE FROM messages", [])?;
        tx.execute(
            "DELETE FROM memories WHERE content_type = 'chat_message'",
            [],
        )?;
        for msg in messages {
            tx.execute(
                "INSERT INTO messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![msg.session_id, msg.role, msg.content, msg.created_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn create_chat_session(&self, session_id: &str, title: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        ensure_conversation_write_allowed(&tx, session_id)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO chat_sessions (session_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, title, &now, &now],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_chat_sessions(&self, limit: usize) -> Result<Vec<ChatSession>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, title, created_at, updated_at FROM chat_sessions ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ChatSession {
                session_id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to list chat sessions")
    }

    pub fn rename_chat_session(&self, session_id: &str, title: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        ensure_conversation_write_allowed(&tx, session_id)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE chat_sessions SET title = ?1, updated_at = ?2 WHERE session_id = ?3",
            params![title, &now, session_id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_chat_session(&self, session_id: &str) -> Result<()> {
        self.delete_chat_session_with_tombstone(session_id, None)?;
        Ok(())
    }

    /// Delete the canonical conversation content and record its tombstone plus
    /// projection work in the same `memory.db` transaction. Other databases
    /// are reconciled from this durable metadata-only outbox; this method does
    /// not claim a cross-database transaction.
    pub fn delete_chat_session_with_tombstone(
        &self,
        session_id: &str,
        reason: Option<&str>,
    ) -> Result<CanonicalMutationReceipt> {
        let session_id = session_id.trim();
        if session_id.is_empty() || session_id.len() > 256 {
            anyhow::bail!("invalid conversation session id for deletion");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        // Long-term Memory rows have independent canonical owners. Only old
        // conversation mirrors, if any survived migration, belong to this
        // conversation tombstone.
        tx.execute(
            "DELETE FROM memories
             WHERE session_id = ?1 AND content_type = 'chat_message'",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM snapshots WHERE session_id = ?1",
            params![session_id],
        )?;
        tx.execute(
            "DELETE FROM chat_sessions WHERE session_id = ?1",
            params![session_id],
        )?;
        let receipt = persistence_outbox::enqueue_tombstone(
            &tx,
            "conversation",
            session_id,
            reason,
            &[
                "vector_store",
                "agent_run_store",
                "turn_event_store",
                "action_queue_store",
                "life_event_store",
            ],
        )?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Commit the retrieval visibility of one stable canonical Memory owner.
    ///
    /// Vector row ids, session ids and source prefixes are not accepted as
    /// authority. The state mutation and its ref-only projection event share
    /// the MemoryStore transaction; VectorStore may lag, but searches can
    /// always fail closed against this canonical state.
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

    pub fn touch_chat_session(&self, session_id: &str) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        ensure_conversation_write_allowed(&tx, session_id)?;
        let now = Utc::now().to_rfc3339();
        let updated = tx.execute(
            "UPDATE chat_sessions SET updated_at = ?1 WHERE session_id = ?2",
            params![&now, session_id],
        )?;
        if updated == 0 {
            tx.execute(
                "INSERT INTO chat_sessions (session_id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
                params![session_id, "新会话", &now, &now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    #[cfg(test)]
    fn record_state_entry(
        &self,
        dimension_name: &str,
        value: f64,
        unit: &str,
        note: Option<&str>,
    ) -> Result<i64> {
        Ok(self
            .record_state_entry_idempotent(
                &uuid::Uuid::new_v4().to_string(),
                dimension_name,
                value,
                unit,
                note,
                None,
                None,
                None,
            )?
            .state_entry_id)
    }

    #[cfg(test)]
    fn record_state_entry_idempotent(
        &self,
        operation_id: &str,
        dimension_name: &str,
        value: f64,
        unit: &str,
        note: Option<&str>,
        min_threshold: Option<f32>,
        max_threshold: Option<f32>,
        alert_days: Option<u32>,
    ) -> Result<CanonicalStateEntryWrite> {
        ensure_knowledge_note_operation_id(operation_id)
            .context("State entry operation id must be canonical lowercase UUIDv4")?;
        if dimension_name.trim().is_empty()
            || unit.trim().is_empty()
            || !value.is_finite()
            || min_threshold.map_or(false, |threshold| !threshold.is_finite())
            || max_threshold.map_or(false, |threshold| !threshold.is_finite())
        {
            anyhow::bail!("State entry payload is invalid");
        }
        let operation_digest = persistence_outbox::metadata_digest(&format!(
            "state_entry:{operation_id}:{}",
            serde_json::to_string(&serde_json::json!({
                "dimensionName": dimension_name,
                "value": value,
                "unit": unit,
                "note": note,
                "minThreshold": min_threshold,
                "maxThreshold": max_threshold,
                "alertDays": alert_days,
            }))?
        ));
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id, dimension_name, value, unit, note, operation_digest
                 FROM state_history WHERE operation_id = ?1",
                [operation_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?;
        if let Some((id, stored_dimension, stored_value, stored_unit, stored_note, stored_digest)) =
            existing
        {
            if stored_digest.as_deref() != Some(operation_digest.as_str())
                || stored_dimension != dimension_name
                || stored_value != value
                || stored_unit != unit
                || stored_note.as_deref() != note
            {
                anyhow::bail!("State entry operation id was reused with a different payload");
            }
            tx.commit()?;
            return Ok(CanonicalStateEntryWrite {
                operation_id: operation_id.to_string(),
                operation_digest,
                state_entry_id: id,
                replayed: true,
            });
        }
        tx.execute(
            "INSERT INTO state_history (
                dimension_name, value, unit, recorded_at, note,
                operation_id, operation_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                dimension_name,
                value,
                unit,
                Utc::now().to_rfc3339(),
                note,
                operation_id,
                operation_digest,
            ],
        )?;
        let state_entry_id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(CanonicalStateEntryWrite {
            operation_id: operation_id.to_string(),
            operation_digest,
            state_entry_id,
            replayed: false,
        })
    }

    pub fn get_state_history(
        &self,
        dimension_name: &str,
        limit: usize,
    ) -> Result<Vec<StateHistoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, dimension_name, value, unit, recorded_at, note FROM state_history WHERE dimension_name = ?1 ORDER BY recorded_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![dimension_name, limit as i64], |row| {
            Ok(StateHistoryEntry {
                id: row.get(0)?,
                dimension_name: row.get(1)?,
                value: row.get(2)?,
                unit: row.get(3)?,
                recorded_at: row.get(4)?,
                note: row.get(5)?,
            })
        })?;
        let mut entries: Vec<StateHistoryEntry> = rows.collect::<Result<Vec<_>, _>>()?;
        entries.reverse();
        Ok(entries)
    }

    pub fn get_latest_state_entries(&self, limit: usize) -> Result<Vec<StateHistoryEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, dimension_name, value, unit, recorded_at, note FROM state_history ORDER BY recorded_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(StateHistoryEntry {
                id: row.get(0)?,
                dimension_name: row.get(1)?,
                value: row.get(2)?,
                unit: row.get(3)?,
                recorded_at: row.get(4)?,
                note: row.get(5)?,
            })
        })?;
        let mut entries: Vec<StateHistoryEntry> = rows.collect::<Result<Vec<_>, _>>()?;
        entries.reverse();
        Ok(entries)
    }

    /// Complete, ordered source snapshot for the bounded StateStore migration
    /// seam. Product reads must not use this API after authority cutover.
    pub fn list_legacy_state_history_migration_source(
        &self,
    ) -> Result<LegacyStateHistoryMigrationSnapshot> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, dimension_name, value, unit, recorded_at, note,
                    operation_id, operation_digest
             FROM state_history
             ORDER BY id ASC
             LIMIT ?1",
        )?;
        let records = statement
            .query_map(
                [i64::try_from(
                    LEGACY_STATE_HISTORY_MIGRATION_MAX_ROWS.saturating_add(1),
                )?],
                |row| {
                    Ok(LegacyStateHistorySourceRecord {
                        id: row.get(0)?,
                        dimension_name: row.get(1)?,
                        value: row.get(2)?,
                        unit: row.get(3)?,
                        recorded_at: row.get(4)?,
                        note: row.get(5)?,
                        operation_id: row.get(6)?,
                        operation_digest: row.get(7)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(anyhow::Error::from)?;
        if records.len() > LEGACY_STATE_HISTORY_MIGRATION_MAX_ROWS {
            anyhow::bail!("legacy_state_history_migration_row_limit_exceeded");
        }
        Ok(LegacyStateHistoryMigrationSnapshot {
            source_store_identity: self.canonical_store_identity.to_string(),
            records,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn save_memory_record(
        &self,
        session_id: &str,
        content: &str,
        content_type: &str,
        source: &str,
        tags: &[String],
        privacy_level: &str,
        embedding_id: Option<i64>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let created_at = Utc::now().to_rfc3339();
        Self::insert_memory_row(
            &conn,
            session_id,
            content,
            content_type,
            source,
            None,
            &created_at,
            tags,
            privacy_level,
            embedding_id,
        )
    }

    /// Commit a canonical MemoryStore-owned note and its vector projection
    /// trigger in one local transaction. The outbox stores only the row id and
    /// an opaque digest; the vector materializer reloads the body from this
    /// canonical owner.
    #[allow(clippy::too_many_arguments)]
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
            // Countable, migration-only compatibility for operation receipts
            // written by the retired `index_memory_chunk` route. New product
            // writes below can never create this contract.
            let legacy_contract = canonical_mutation.aggregate_kind == "memory_record"
                && canonical_mutation.mutation_kind == "indexed";
            if (!current_contract && !legacy_contract)
                || canonical_mutation.aggregate_id != memory_id.to_string()
            {
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

    #[allow(clippy::too_many_arguments)]
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
        let expected_mutation = match owner.kind() {
            "knowledge_note" => "created",
            "memory_record" => "indexed",
            _ => anyhow::bail!("unsupported KnowledgeNote projection owner"),
        };
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
                params![
                    event_id,
                    memory_id,
                    owner.kind(),
                    owner.id(),
                    expected_mutation,
                ],
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
        if !matches!(owner.kind(), "knowledge_note" | "memory_record") {
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

    /// Replaceable compatibility materializer for Lifecycle-owned Memory. Raw
    /// content enters this legacy projection only here; the delivery and marker
    /// remain ref-only so D010 can later replace this with Lifecycle FTS without
    /// changing the outbox contract.
    #[allow(clippy::too_many_arguments)]
    pub fn project_lifecycle_memory(
        &self,
        event_id: &str,
        memory_id: &str,
        session_id: &str,
        content: &str,
        content_type: &str,
        tags: &[String],
        privacy_level: &str,
        embedding_id: Option<i64>,
    ) -> Result<Option<i64>> {
        validate_projection_ref("event_id", event_id)?;
        validate_projection_ref("memory_id", memory_id)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        if let Some(row_id) = projected_memory_row_id(&tx, event_id)? {
            tx.commit()?;
            return Ok(row_id);
        }
        // A deletion marker is a durable fence. It is checked in the same
        // projection-store transaction as the compatibility write, so a late
        // creation delivery can never unarchive content after its tombstone
        // has already been applied by another reconciler.
        if memory_lifecycle_projection_deleted(&tx, memory_id)? {
            insert_memory_projection_marker(
                &tx,
                event_id,
                "memory_lifecycle",
                memory_id,
                "materialized",
                None,
            )?;
            tx.commit()?;
            return Ok(None);
        }
        let lifecycle_source = format!("memory_lifecycle:{memory_id}");
        let existing = tx
            .query_row(
                "SELECT id, created_at FROM memories
                 WHERE source = ?1 ORDER BY id ASC LIMIT 1",
                [&lifecycle_source],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let row_id = if let Some((row_id, created_at)) = existing {
            let tags_json = serde_json::to_string(tags)?;
            let checksum = Self::checksum_for(content, session_id, &created_at);
            tx.execute(
                "UPDATE memories
                 SET session_id = ?2, content = ?3, content_type = ?4,
                     tags_json = ?5, privacy_level = ?6, embedding_id = ?7,
                     checksum = ?8, archived = 0, archived_at = NULL
                 WHERE id = ?1",
                params![
                    row_id,
                    session_id,
                    content,
                    content_type,
                    tags_json,
                    privacy_level,
                    embedding_id,
                    checksum
                ],
            )?;
            row_id
        } else {
            let created_at = Utc::now().to_rfc3339();
            Self::insert_memory_row(
                &tx,
                session_id,
                content,
                content_type,
                &lifecycle_source,
                None,
                &created_at,
                tags,
                privacy_level,
                embedding_id,
            )?
        };
        insert_memory_projection_marker(
            &tx,
            event_id,
            "memory_lifecycle",
            memory_id,
            "materialized",
            Some(row_id),
        )?;
        tx.commit()?;
        Ok(Some(row_id))
    }

    pub fn project_lifecycle_memory_tombstone(
        &self,
        event_id: &str,
        memory_id: &str,
    ) -> Result<usize> {
        validate_projection_ref("event_id", event_id)?;
        validate_projection_ref("memory_id", memory_id)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let already_applied = memory_projection_applied(&tx, event_id)?;
        let lifecycle_source = format!("memory_lifecycle:{memory_id}");
        // This row is a compatibility projection; the reversible canonical
        // body remains in MemoryLifecycleStore. A tombstone therefore removes
        // the duplicate raw body instead of retaining it as an "archive".
        let canonical_memory_tag = format!("memory_id:{memory_id}");
        let deleted = tx.execute(
            "DELETE FROM memories
             WHERE source = ?1
                OR (
                    EXISTS (
                        SELECT 1 FROM json_each(
                            CASE WHEN json_valid(memories.tags_json)
                                 THEN memories.tags_json ELSE '[]' END
                        ) owner_tag
                        WHERE owner_tag.value = 'canonical_owner:memory_lifecycle'
                    )
                    AND EXISTS (
                        SELECT 1 FROM json_each(
                            CASE WHEN json_valid(memories.tags_json)
                                 THEN memories.tags_json ELSE '[]' END
                        ) memory_tag
                        WHERE memory_tag.value = ?2
                    )
                )",
            params![lifecycle_source, canonical_memory_tag],
        )?;
        if !already_applied {
            insert_memory_projection_marker(
                &tx,
                event_id,
                "memory_lifecycle",
                memory_id,
                "deleted",
                None,
            )?;
        }
        tx.commit()?;
        Ok(deleted)
    }

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
                 WHERE memories_fts MATCH ?1 AND m.session_id = ?2
                   AND (
                       m.archived = 0 OR EXISTS (
                           SELECT 1 FROM memory_materialization_projections projection
                           WHERE projection.memory_row_id = m.id
                             AND projection.aggregate_kind = 'memory_lifecycle'
                             AND projection.mutation_kind = 'materialized'
                       )
                   )
                 ORDER BY rank ASC, m.created_at DESC
                 LIMIT ?3",
            ) {
                Ok(stmt) => stmt,
                Err(_) => {
                    let mut fallback =
                        self.search_text_memories_fallback(&conn, session_id, query, limit)?;
                    fallback.extend(self.search_session_messages(
                        &conn,
                        Some(session_id),
                        None,
                        query,
                        limit,
                    )?);
                    let mut seen = HashSet::new();
                    fallback.retain(|hit| {
                        seen.insert((hit.chunk.session_id.clone(), hit.chunk.content.clone()))
                    });
                    fallback.truncate(limit);
                    return Ok(fallback);
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
                   AND (
                       m.archived = 0 OR EXISTS (
                           SELECT 1 FROM memory_materialization_projections projection
                           WHERE projection.memory_row_id = m.id
                             AND projection.aggregate_kind = 'memory_lifecycle'
                             AND projection.mutation_kind = 'materialized'
                       )
                   )
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

        if let Some(session_id) = session_id {
            results.extend(self.search_session_messages(
                &conn,
                Some(session_id),
                None,
                query,
                limit,
            )?);
            let mut seen = HashSet::new();
            results.retain(|hit| {
                seen.insert((hit.chunk.session_id.clone(), hit.chunk.content.clone()))
            });
        }

        let now = Utc::now().to_rfc3339();
        let ids: Vec<i64> = results
            .iter()
            .map(|hit| hit.chunk.id)
            .filter(|id| *id > 0)
            .collect();
        if !ids.is_empty() {
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "UPDATE memories SET access_count = access_count + 1, last_accessed_at = ?1 WHERE archived = 0 AND id IN ({})",
                placeholders
            );
            let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&now];
            params_vec.extend(ids.iter().map(|id| id as &dyn rusqlite::ToSql));
            let _ = conn.execute(&sql, &*params_vec);
        }
        results.truncate(limit);
        Ok(results)
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
             WHERE (
                       archived = 0 OR EXISTS (
                           SELECT 1 FROM memory_materialization_projections projection
                           WHERE projection.memory_row_id = memories.id
                             AND projection.aggregate_kind = 'memory_lifecycle'
                             AND projection.mutation_kind = 'materialized'
                       )
                   )
               AND content LIKE ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        } else {
            "SELECT id, session_id, content, source, created_at, access_count, last_accessed_at
             FROM memories
             WHERE (
                       archived = 0 OR EXISTS (
                           SELECT 1 FROM memory_materialization_projections projection
                           WHERE projection.memory_row_id = memories.id
                             AND projection.aggregate_kind = 'memory_lifecycle'
                             AND projection.mutation_kind = 'materialized'
                       )
                   )
               AND session_id = ?1 AND content LIKE ?2
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
                        if owner.kind() == "memory_lifecycle" {
                            // This row is only a derived body projection. The
                            // caller must join it with MemoryLifecycleReader;
                            // residual MemoryStore archive rows are forbidden
                            // from overriding current lifecycle truth.
                            return Some(Ok(hit));
                        }
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

    /// Search the canonical conversation owner without copying message bodies
    /// into the long-term Memory index. `session_id` scopes an explicit
    /// session lookup; otherwise the search spans prior conversations while
    /// `exclude_session_id` keeps the current turn from satisfying its own
    /// "what did we discuss" query.
    pub fn search_conversation_messages(
        &self,
        session_id: Option<&str>,
        exclude_session_id: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let limit = limit.min(10);
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        self.search_session_messages(
            &conn,
            session_id,
            session_id.is_none().then_some(exclude_session_id).flatten(),
            query,
            limit,
        )
    }

    fn search_session_messages(
        &self,
        conn: &Connection,
        session_id: Option<&str>,
        exclude_session_id: Option<&str>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemorySearchHit>> {
        let bounded_query = query
            .trim()
            .chars()
            .take(FTS_QUERY_MAX_CHARS)
            .collect::<String>();
        let terms = conversation_search_terms(&bounded_query);
        if bounded_query.is_empty() || terms.is_empty() {
            return Ok(Vec::new());
        }
        let scan_limit = (limit.saturating_mul(40)).clamp(50, 400) as i64;
        let (sql, first_parameter) = if let Some(session_id) = session_id {
            (
                "SELECT id, session_id, content, role, created_at
                 FROM messages
                 WHERE session_id = ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?2",
                session_id,
            )
        } else {
            (
                "SELECT id, session_id, content, role, created_at
                 FROM messages
                 WHERE session_id != ?1
                 ORDER BY created_at DESC, id DESC
                 LIMIT ?2",
                exclude_session_id.unwrap_or_default(),
            )
        };
        let mut statement = conn.prepare(sql)?;
        let rows = statement
            .query_map(params![first_parameter, scan_limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?.max(1),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to scan bounded canonical conversation messages")?;
        let exact_query = bounded_query.to_lowercase();
        let mut hits = rows
            .into_iter()
            .filter_map(|(message_id, session_id, content, role, created_at)| {
                let normalized_content = content.to_lowercase();
                let matched_terms = terms
                    .iter()
                    .filter(|term| normalized_content.contains(term.as_str()))
                    .count();
                if matched_terms == 0 {
                    return None;
                }
                let exact_match = normalized_content.contains(exact_query.as_str());
                let relevance_score = if exact_match {
                    1.0
                } else {
                    0.55 + 0.4 * (matched_terms as f32 / terms.len() as f32)
                };
                Some(MemorySearchHit {
                    chunk: MemoryChunk {
                        id: -message_id,
                        session_id,
                        content,
                        source: format!("conversation:{role}"),
                        created_at,
                        tier: 3,
                        access_count: 0,
                        last_accessed_at: String::new(),
                        importance_score: 0.0,
                        archived: false,
                        archived_at: None,
                        summary: None,
                    },
                    relevance_score,
                    source_tier: "conversation".into(),
                })
            })
            .collect::<Vec<_>>();
        hits.sort_by(|left, right| {
            right
                .relevance_score
                .total_cmp(&left.relevance_score)
                .then_with(|| right.chunk.created_at.cmp(&left.chunk.created_at))
        });
        hits.truncate(limit);
        Ok(hits)
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

fn validate_projection_ref(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        anyhow::bail!("invalid Memory projection {label}");
    }
    Ok(())
}

fn projected_memory_row_id(conn: &Connection, event_id: &str) -> Result<Option<Option<i64>>> {
    conn.query_row(
        "SELECT memory_row_id FROM memory_materialization_projections WHERE event_id = ?1",
        [event_id],
        |row| row.get::<_, Option<i64>>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn memory_lifecycle_projection_deleted(conn: &Connection, memory_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM memory_materialization_projections
             WHERE aggregate_kind = 'memory_lifecycle'
               AND aggregate_id = ?1 AND mutation_kind = 'deleted'
             LIMIT 1",
            [memory_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn memory_projection_applied(conn: &Connection, event_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM memory_materialization_projections WHERE event_id = ?1",
            [event_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn insert_memory_projection_marker(
    conn: &Connection,
    event_id: &str,
    aggregate_kind: &str,
    aggregate_id: &str,
    mutation_kind: &str,
    memory_row_id: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO memory_materialization_projections (
            event_id, aggregate_kind, aggregate_id, mutation_kind,
            memory_row_id, applied_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event_id,
            aggregate_kind,
            aggregate_id,
            mutation_kind,
            memory_row_id,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedMessage {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatSession {
    pub session_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LegacyStateHistorySourceRecord {
    pub id: i64,
    pub dimension_name: String,
    pub value: f64,
    pub unit: String,
    pub recorded_at: String,
    pub note: Option<String>,
    pub operation_id: Option<String>,
    pub operation_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LegacyStateHistoryMigrationSnapshot {
    pub source_store_identity: String,
    pub records: Vec<LegacyStateHistorySourceRecord>,
}

impl LegacyStateHistoryMigrationSnapshot {
    pub fn validate_source_store_identity(&self) -> Result<()> {
        if !is_canonical_memory_store_identity(&self.source_store_identity) {
            anyhow::bail!("legacy_state_history_source_store_identity_invalid");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalStateEntryWrite {
    pub operation_id: String,
    pub operation_digest: String,
    pub state_entry_id: i64,
    pub replayed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatMessage;
    use rusqlite::Connection;

    #[test]
    fn memory_store_save_and_load_message() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let session_id = "s1";
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        };
        store.save_message(session_id, &msg).unwrap();
        let loaded = store.load_recent_messages(session_id, 10).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(loaded[0].content, "hello");
    }

    #[test]
    fn memory_store_read_only_degraded_open_checks_the_real_canonical_tables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory-read-only.db");
        {
            let store = MemoryStore::new(&path).unwrap();
            store
                .save_message(
                    "read-only-session",
                    &ChatMessage {
                        role: "user".into(),
                        content: "canonical read-only evidence".into(),
                    },
                )
                .unwrap();
        }

        let read_only = MemoryStore::open_read_only_existing(&path).unwrap();
        let loaded = read_only
            .load_recent_messages("read-only-session", 10)
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].content, "canonical read-only evidence");
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
    fn canonical_message_operation_replay_is_exact_and_payload_drift_fails_closed() {
        let store = MemoryStore::new_in_memory().unwrap();
        let message = ChatMessage {
            role: "assistant".into(),
            content: "Canonical scheduled delivery".into(),
        };
        let first = store
            .save_message_idempotent("scheduled:attempt-1", &message, "scheduled:attempt-1:final")
            .unwrap();
        let replay = store
            .save_message_idempotent("scheduled:attempt-1", &message, "scheduled:attempt-1:final")
            .unwrap();

        assert_eq!(first.message_id, replay.message_id);
        assert_eq!(first.canonical_ref, replay.canonical_ref);
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(
            first.canonical_ref,
            format!(
                "conversation://scheduled:attempt-1/message/{}",
                first.message_id
            )
        );
        let loaded = store
            .load_recent_messages("scheduled:attempt-1", 10)
            .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].role, message.role);
        assert_eq!(loaded[0].content, message.content);

        let drift = ChatMessage {
            role: "assistant".into(),
            content: "Different delivery".into(),
        };
        assert!(store
            .save_message_idempotent("scheduled:attempt-1", &drift, "scheduled:attempt-1:final")
            .is_err());
        assert!(store
            .save_message_idempotent("scheduled:attempt-2", &message, "scheduled:attempt-1:final")
            .is_err());
    }

    #[test]
    fn canonical_store_identity_is_stable_across_reopen_and_distinct_per_store() {
        let directory = tempfile::tempdir().unwrap();
        let first_path = directory.path().join("first-memory.db");
        let second_path = directory.path().join("second-memory.db");
        let first_identity = {
            let store = MemoryStore::new(&first_path).unwrap();
            store.canonical_store_identity().to_string()
        };
        let reopened_identity = MemoryStore::new(&first_path)
            .unwrap()
            .canonical_store_identity()
            .to_string();
        let second_identity = MemoryStore::new(&second_path)
            .unwrap()
            .canonical_store_identity()
            .to_string();

        assert_eq!(first_identity, reopened_identity);
        assert_ne!(first_identity, second_identity);
        assert!(is_canonical_memory_store_identity(&first_identity));
        assert!(is_canonical_memory_store_identity(&second_identity));
    }

    #[test]
    fn assistant_message_commit_cannot_issue_a_canonical_user_input_proof() {
        let store = MemoryStore::new_in_memory().unwrap();
        let assistant_message = ChatMessage {
            role: "assistant".into(),
            content: "This is output, not authenticated user input".into(),
        };

        let error = store
            .save_message_idempotent_with_proof(
                "proof-role-session",
                &assistant_message,
                "proof-role-operation",
            )
            .expect_err("assistant output cannot authorize an AgentRun input reference");
        assert!(error
            .to_string()
            .contains("canonical_user_message_proof_requires_role_user"));
        assert!(store
            .load_recent_messages("proof-role-session", 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn concurrent_message_replay_cannot_create_a_b_a_history() {
        let store = Arc::new(MemoryStore::new_in_memory().unwrap());
        let session_id = "concurrent-conversation";
        let message_a = ChatMessage {
            role: "user".into(),
            content: "A".into(),
        };
        store
            .save_message_idempotent(session_id, &message_a, "operation-a")
            .unwrap();

        let barrier = Arc::new(std::sync::Barrier::new(3));
        let replay_store = Arc::clone(&store);
        let replay_barrier = Arc::clone(&barrier);
        let replay_a = message_a.clone();
        let replay = std::thread::spawn(move || {
            replay_barrier.wait();
            replay_store
                .save_message_idempotent(session_id, &replay_a, "operation-a")
                .unwrap()
        });
        let write_store = Arc::clone(&store);
        let write_barrier = Arc::clone(&barrier);
        let write = std::thread::spawn(move || {
            let message_b = ChatMessage {
                role: "assistant".into(),
                content: "B".into(),
            };
            write_barrier.wait();
            write_store
                .save_message_idempotent(session_id, &message_b, "operation-b")
                .unwrap()
        });
        barrier.wait();
        assert!(replay.join().unwrap().replayed);
        assert!(!write.join().unwrap().replayed);

        let history = store.load_recent_messages(session_id, 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].content, "A");
        assert_eq!(history[1].content, "B");
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
            let receipt = crate::persistence_outbox::enqueue_mutation(
                &tx,
                "memory_retrieval",
                "legacy-lifecycle-owner",
                "archived",
                "sha256:legacy",
                &["vector_store"],
            )
            .unwrap();
            legacy_event_id = receipt.event_id.clone();
            tx.execute(
                "INSERT INTO memory_retrieval_states
                    (owner_kind, owner_id, disposition, revision, last_event_id,
                     reason_digest, changed_at)
                 VALUES ('memory_lifecycle', 'memory:owner', 'archived', 1, ?1,
                         'sha256:legacy', '2026-07-12T00:00:00Z')",
                [receipt.event_id],
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
    fn residual_lifecycle_archive_state_cannot_hide_a_reader_authorized_body() {
        const SENTINEL: &str = "LIFECYCLE_READER_IS_CANONICAL_SENTINEL";
        let store = MemoryStore::new_in_memory().unwrap();
        store
            .project_lifecycle_memory(
                "event-lifecycle-reader-canonical",
                "memory:reader-canonical",
                "lifecycle-reader-session",
                SENTINEL,
                "lifecycle_memory_projection",
                &[
                    "canonical_owner:memory_lifecycle".into(),
                    "memory_id:memory:reader-canonical".into(),
                ],
                "private",
                None,
            )
            .unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            conn.execute_batch(
                "DROP TRIGGER reject_memory_lifecycle_retrieval_insert;
                 DROP TRIGGER reject_memory_lifecycle_retrieval_update;",
            )
            .unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute(
                "UPDATE memories SET archived = 1, archived_at = '2026-07-12T00:00:00Z'
                 WHERE source = 'memory_lifecycle:memory:reader-canonical'",
                [],
            )
            .unwrap();
            let receipt = crate::persistence_outbox::enqueue_mutation(
                &tx,
                "memory_retrieval",
                "stale-memory-store-lifecycle-owner",
                "archived",
                "sha256:stale",
                &["vector_store"],
            )
            .unwrap();
            tx.execute(
                "INSERT INTO memory_retrieval_states
                    (owner_kind, owner_id, disposition, revision, last_event_id,
                     reason_digest, changed_at)
                 VALUES ('memory_lifecycle', 'memory:reader-canonical', 'archived', 1,
                         ?1, 'sha256:stale', '2026-07-12T00:00:00Z')",
                [receipt.event_id],
            )
            .unwrap();
            tx.commit().unwrap();
        }

        let hits = store.search_text_memories(None, SENTINEL, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(
            hits[0].chunk.source,
            "memory_lifecycle:memory:reader-canonical"
        );
    }

    #[test]
    fn memory_store_save_and_load_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let session_id = "s1";
        let model = LifeModel::default_model();
        store.save_snapshot(session_id, &model).unwrap();
        let loaded = store.load_latest_snapshot(session_id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().metadata.version, model.metadata.version);
    }

    #[test]
    fn memory_store_chat_session_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store.create_chat_session("sess1", "Test Session").unwrap();
        let sessions = store.list_chat_sessions(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "sess1");
        assert_eq!(sessions[0].title, "Test Session");

        store.rename_chat_session("sess1", "Renamed").unwrap();
        let sessions = store.list_chat_sessions(10).unwrap();
        assert_eq!(sessions[0].title, "Renamed");

        store.touch_chat_session("sess1").unwrap();
        store.delete_chat_session("sess1").unwrap();
        let sessions = store.list_chat_sessions(10).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn memory_store_state_history_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .record_state_entry("energy", 75.0, "%", Some("good"))
            .unwrap();
        store.record_state_entry("energy", 80.0, "%", None).unwrap();
        let history = store.get_state_history("energy", 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].value, 75.0);
        assert_eq!(history[1].value, 80.0);

        let latest = store.get_latest_state_entries(5).unwrap();
        assert_eq!(latest.len(), 2);
    }

    #[test]
    fn state_history_operation_is_payload_bound_and_replay_safe() {
        let store = MemoryStore::new_in_memory().unwrap();
        let operation_id = uuid::Uuid::new_v4().to_string();
        let first = store
            .record_state_entry_idempotent(
                &operation_id,
                "focus",
                8.0,
                "points",
                Some("afternoon"),
                Some(1.0),
                Some(10.0),
                Some(2),
            )
            .unwrap();
        let replay = store
            .record_state_entry_idempotent(
                &operation_id,
                "focus",
                8.0,
                "points",
                Some("afternoon"),
                Some(1.0),
                Some(10.0),
                Some(2),
            )
            .unwrap();

        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.state_entry_id, replay.state_entry_id);
        assert_eq!(first.operation_digest, replay.operation_digest);
        assert_eq!(store.get_state_history("focus", 10).unwrap().len(), 1);
        let migration_source = store.list_legacy_state_history_migration_source().unwrap();
        assert_eq!(
            migration_source.source_store_identity,
            store.canonical_store_identity.as_ref()
        );
        assert_eq!(migration_source.records.len(), 1);
        assert_eq!(migration_source.records[0].id, first.state_entry_id);
        assert_eq!(
            migration_source.records[0].operation_id.as_deref(),
            Some(operation_id.as_str())
        );
        assert_eq!(
            migration_source.records[0].operation_digest.as_deref(),
            Some(first.operation_digest.as_str())
        );
        assert_eq!(
            migration_source.records[0].note.as_deref(),
            Some("afternoon")
        );
        assert!(store
            .record_state_entry_idempotent(
                &operation_id,
                "focus",
                9.0,
                "points",
                Some("afternoon"),
                Some(1.0),
                Some(10.0),
                Some(2),
            )
            .unwrap_err()
            .to_string()
            .contains("different payload"));
        for (dimension_name, unit, note) in [
            ("energy", "points", Some("afternoon")),
            ("focus", "percent", Some("afternoon")),
            ("focus", "points", Some("evening")),
        ] {
            assert!(
                store
                    .record_state_entry_idempotent(
                        &operation_id,
                        dimension_name,
                        8.0,
                        unit,
                        note,
                        Some(1.0),
                        Some(10.0),
                        Some(2),
                    )
                    .unwrap_err()
                    .to_string()
                    .contains("different payload"),
                "dimension, unit, and note must each remain bound to the operation UUID"
            );
        }
        for (min_threshold, max_threshold, alert_days) in [
            (Some(2.0), Some(10.0), Some(2)),
            (Some(1.0), Some(9.0), Some(2)),
            (Some(1.0), Some(10.0), Some(3)),
        ] {
            assert!(
                store
                    .record_state_entry_idempotent(
                        &operation_id,
                        "focus",
                        8.0,
                        "points",
                        Some("afternoon"),
                        min_threshold,
                        max_threshold,
                        alert_days,
                    )
                    .unwrap_err()
                    .to_string()
                    .contains("different payload"),
                "each LifeModel projection field must remain bound to the operation UUID"
            );
        }
    }

    #[test]
    fn legacy_state_history_migration_source_fails_closed_above_bounded_limit() {
        let store = MemoryStore::new_in_memory().unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            {
                let mut statement = tx
                    .prepare(
                        "INSERT INTO state_history (
                            dimension_name, value, unit, recorded_at, note,
                            operation_id, operation_digest
                         ) VALUES ('focus', ?1, '/10', ?2, NULL, NULL, NULL)",
                    )
                    .unwrap();
                let recorded_at = Utc::now().to_rfc3339();
                for index in 0..=LEGACY_STATE_HISTORY_MIGRATION_MAX_ROWS {
                    statement
                        .execute(params![index as f64, &recorded_at])
                        .unwrap();
                }
            }
            tx.commit().unwrap();
        }

        let error = store
            .list_legacy_state_history_migration_source()
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("legacy_state_history_migration_row_limit_exceeded"));
    }

    #[test]
    fn fts_query_escape_handles_special_syntax_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .save_memory_record(
                "s1",
                "alpha quoted value with 中文 token",
                "note",
                "test",
                &[],
                "private",
                None,
            )
            .unwrap();

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
        store
            .save_memory_record(
                "s1",
                "deep work 深度工作 planning",
                "note",
                "test",
                &[],
                "private",
                None,
            )
            .unwrap();

        let english = store.search_text_memories(Some("s1"), "deep", 5).unwrap();
        assert_eq!(english.len(), 1);

        let chinese = store
            .search_text_memories(Some("s1"), "深度工作", 5)
            .unwrap();
        assert_eq!(chinese.len(), 1);
    }

    #[test]
    fn memory_store_export_import_messages() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let msg = ChatMessage {
            role: "user".into(),
            content: "hi".into(),
        };
        store.save_message("s1", &msg).unwrap();
        let exported = store.export_all_messages().unwrap();
        assert_eq!(exported.len(), 1);

        store.clear_all_messages().unwrap();
        let loaded = store.load_recent_messages("s1", 10).unwrap();
        assert!(loaded.is_empty());

        store.import_messages(&exported).unwrap();
        let loaded = store.load_recent_messages("s1", 10).unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn memory_store_keeps_chat_body_in_messages_only_but_session_search_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        let msg = ChatMessage {
            role: "assistant".into(),
            content: "Rust 异步任务需要注意取消和所有权".into(),
        };
        store.save_message("s1", &msg).unwrap();
        let hits = store.search_text_memories(Some("s1"), "Rust", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].chunk.content.contains("Rust"));
        assert_eq!(hits[0].source_tier, "conversation");

        let conn = store.conn.lock().unwrap();
        let message_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap();
        let mirrored_memory_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE content_type = 'chat_message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(message_count, 1);
        assert_eq!(mirrored_memory_count, 0);
    }

    #[test]
    fn conversation_search_spans_prior_sessions_and_excludes_current_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .save_message(
                "prior-session",
                &ChatMessage {
                    role: "user".into(),
                    content: "Agent memory needs bounded source citations".into(),
                },
            )
            .unwrap();
        store
            .save_message(
                "current-session",
                &ChatMessage {
                    role: "user".into(),
                    content: "What did we say about Agent memory?".into(),
                },
            )
            .unwrap();

        let hits = store
            .search_conversation_messages(
                None,
                Some("current-session"),
                "Find what we discussed about Agent memory.",
                10,
            )
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.session_id, "prior-session");
        assert_eq!(hits[0].source_tier, "conversation");

        let conn = store.conn.lock().unwrap();
        let mirrored_memory_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE content_type = 'chat_message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mirrored_memory_count, 0);
    }

    #[test]
    fn memory_store_restart_scrubs_legacy_chat_mirror_but_keeps_canonical_message() {
        const CHAT_BODY: &str = "RELATIONSHIP-NOTE-41952-BIRCH";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.db");
        let store = MemoryStore::new(&path).unwrap();
        store
            .save_message(
                "s1",
                &ChatMessage {
                    role: "user".into(),
                    content: CHAT_BODY.into(),
                },
            )
            .unwrap();
        store
            .save_memory_record(
                "s1",
                CHAT_BODY,
                "chat_message",
                "chat:user",
                &[],
                "private",
                None,
            )
            .unwrap();
        drop(store);

        let restarted = MemoryStore::new(&path).unwrap();
        let messages = restarted.load_recent_messages("s1", 10).unwrap();
        let hits = restarted
            .search_text_memories(Some("s1"), "RELATIONSHIP", 10)
            .unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, CHAT_BODY);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].source_tier, "conversation");
        let mirrored_count: i64 = restarted
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE content_type = 'chat_message'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mirrored_count, 0);
    }

    #[test]
    fn memory_store_can_save_manual_memory_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new(dir.path().join("memory.db")).unwrap();
        store
            .save_memory_record(
                "manual",
                "用户偏好在上午处理复杂任务",
                "note",
                "manual_index",
                &["preference".into(), "energy".into()],
                "private",
                None,
            )
            .unwrap();
        let hits = store.search_text_memories(None, "上午", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(matches!(hits[0].source_tier.as_str(), "fts" | "keyword"));
    }

    #[test]
    fn memory_store_migrates_legacy_messages_table_before_creating_embedding_index() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("legacy-memory.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        drop(conn);

        let store = MemoryStore::new(&db_path).unwrap();
        let msg = ChatMessage {
            role: "user".into(),
            content: "legacy schema still loads".into(),
        };
        store.save_message("legacy", &msg).unwrap();
        let loaded = store.load_recent_messages("legacy", 10).unwrap();
        assert_eq!(loaded.len(), 1);

        let conn = Connection::open(&db_path).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(messages)").unwrap();
        let column_names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(column_names.iter().any(|name| name == "embedding_id"));
    }

    #[test]
    fn conversation_delete_rolls_back_when_colocated_outbox_insert_fails() {
        let store = MemoryStore::new_in_memory().unwrap();
        store
            .create_chat_session("session-atomic", "Atomic")
            .unwrap();
        store
            .save_message(
                "session-atomic",
                &ChatMessage {
                    role: "user".into(),
                    content: "PRIVATE_DELETE_SENTINEL".into(),
                },
            )
            .unwrap();
        store.install_outbox_insert_failure_for_test().unwrap();

        let error = store
            .delete_chat_session_with_tombstone("session-atomic", Some("forget it"))
            .expect_err("canonical delete must roll back with its outbox");
        assert!(error.to_string().contains("injected canonical outbox"));
        assert_eq!(
            store
                .load_recent_messages("session-atomic", 10)
                .unwrap()
                .len(),
            1
        );
        assert!(store
            .list_replayable_projection_deliveries(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn conversation_delete_commits_metadata_only_tombstone_and_outbox() {
        let store = MemoryStore::new_in_memory().unwrap();
        store
            .create_chat_session("session-delete", "Delete")
            .unwrap();
        store
            .save_message(
                "session-delete",
                &ChatMessage {
                    role: "user".into(),
                    content: "PRIVATE_OUTBOX_SENTINEL".into(),
                },
            )
            .unwrap();
        let receipt = store
            .delete_chat_session_with_tombstone("session-delete", Some("PRIVATE_REASON_SENTINEL"))
            .unwrap();

        assert!(store
            .load_recent_messages("session-delete", 10)
            .unwrap()
            .is_empty());
        let deliveries = store.list_replayable_projection_deliveries(10).unwrap();
        assert_eq!(deliveries.len(), 5);
        let serialized = serde_json::to_string(&deliveries).unwrap();
        assert!(!serialized.contains("PRIVATE_OUTBOX_SENTINEL"));
        assert!(!serialized.contains("PRIVATE_REASON_SENTINEL"));
        assert_eq!(
            store.projection_summary(&receipt.event_id).unwrap().pending,
            5
        );
    }

    #[test]
    fn stale_conversation_proof_cannot_create_an_agent_run_after_delete() {
        let memory = MemoryStore::new_in_memory().unwrap();
        memory
            .create_chat_session("stale-proof-session", "Stale proof")
            .unwrap();
        let message = ChatMessage {
            role: "user".into(),
            content: "canonical body that will be deleted".into(),
        };
        let commit = memory
            .save_message_idempotent_with_proof(
                "stale-proof-session",
                &message,
                "stale-proof-operation",
            )
            .unwrap();
        let agent_runs = AgentRunStore::new_in_memory().unwrap();
        agent_runs.bind_canonical_memory_store(&memory).unwrap();
        let mut run = AgentRun::new_chat_run("stale-proof-session", &message.content);
        run.input_ref = Some(commit.receipt().canonical_ref.clone());
        memory
            .delete_chat_session_with_tombstone("stale-proof-session", Some("test_delete"))
            .unwrap();

        let error = memory
            .create_agent_run_from_active_conversation_message(&agent_runs, &run, commit.proof())
            .expect_err("a deleted canonical row must invalidate the old proof")
            .to_string();
        assert!(
            error.contains("canonical_tombstoned") || error.contains("proof_stale"),
            "{error}"
        );
        assert!(agent_runs.get_run(&run.id).unwrap().is_none());
    }

    #[test]
    fn memory_to_agent_run_fence_excludes_delete_without_deadlock_and_outbox_hides_run() {
        let memory = Arc::new(MemoryStore::new_in_memory().unwrap());
        memory
            .create_chat_session("fence-interleave-session", "Fence interleave")
            .unwrap();
        let message = ChatMessage {
            role: "user".into(),
            content: "body owned only by canonical conversation".into(),
        };
        let commit = memory
            .save_message_idempotent_with_proof(
                "fence-interleave-session",
                &message,
                "fence-interleave-operation",
            )
            .unwrap();
        let agent_runs = Arc::new(AgentRunStore::new_in_memory().unwrap());
        agent_runs.bind_canonical_memory_store(&memory).unwrap();
        let mut run = AgentRun::new_chat_run("fence-interleave-session", &message.content);
        run.input_ref = Some(commit.receipt().canonical_ref.clone());
        let run_id = run.id.clone();

        let entered = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let memory_for_insert = Arc::clone(&memory);
        let runs_for_insert = Arc::clone(&agent_runs);
        let entered_for_insert = Arc::clone(&entered);
        let release_for_insert = Arc::clone(&release);
        let insert = std::thread::spawn(move || {
            memory_for_insert.create_agent_run_from_active_conversation_message_internal(
                &runs_for_insert,
                &run,
                commit.proof(),
                || {
                    entered_for_insert.wait();
                    release_for_insert.wait();
                },
            )
        });
        entered.wait();

        let (delete_started_tx, delete_started_rx) = std::sync::mpsc::channel();
        let (delete_done_tx, delete_done_rx) = std::sync::mpsc::channel();
        let memory_for_delete = Arc::clone(&memory);
        let delete = std::thread::spawn(move || {
            delete_started_tx.send(()).unwrap();
            let result = memory_for_delete.delete_chat_session_with_tombstone(
                "fence-interleave-session",
                Some("concurrent_delete"),
            );
            delete_done_tx.send(result).unwrap();
        });
        delete_started_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        assert!(
            delete_done_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err(),
            "delete must not enter the proof-check/AgentRun-insert window"
        );
        release.wait();
        insert.join().unwrap().unwrap();
        let tombstone = delete_done_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("delete must complete after the fence releases")
            .unwrap();
        delete.join().unwrap();

        // Startup/foreground reconciliation consumes this same durable outbox
        // fact. Model it directly here and prove the final product state has no
        // live AgentRun pointing at the deleted canonical body.
        agent_runs
            .project_conversation_tombstone(&tombstone.event_id, "fence-interleave-session")
            .unwrap();
        assert!(agent_runs.get_run(&run_id).unwrap().is_none());
    }

    #[test]
    fn conversation_tombstone_fence_blocks_late_writes_and_preserves_long_term_memory() {
        let store = Arc::new(MemoryStore::new_in_memory().unwrap());
        store
            .create_chat_session("session-fenced", "Fenced")
            .unwrap();
        store
            .create_chat_session("unrelated-session", "Unrelated")
            .unwrap();
        store
            .save_message(
                "session-fenced",
                &ChatMessage {
                    role: "user".into(),
                    content: "CONVERSATION_BODY_MUST_BE_DELETED".into(),
                },
            )
            .unwrap();
        store
            .save_message(
                "unrelated-session",
                &ChatMessage {
                    role: "user".into(),
                    content: "UNRELATED_BODY_MUST_SURVIVE_FAILED_IMPORT".into(),
                },
            )
            .unwrap();
        store
            .save_memory_record(
                "session-fenced",
                "MANUAL_LONG_TERM_MEMORY_MUST_SURVIVE",
                "knowledge_note",
                "manual",
                &[],
                "private",
                None,
            )
            .unwrap();
        store
            .project_lifecycle_memory(
                "outbox:lifecycle-independent",
                "memory:independent",
                "session-fenced",
                "LIFECYCLE_MEMORY_MUST_SURVIVE",
                "lifecycle_memory_projection",
                &["memory_id:memory:independent".into()],
                "private",
                None,
            )
            .unwrap();

        let deletion_committed = Arc::new(std::sync::Barrier::new(2));
        let deleting_store = Arc::clone(&store);
        let deleting_barrier = Arc::clone(&deletion_committed);
        let delete = std::thread::spawn(move || {
            let receipt = deleting_store
                .delete_chat_session_with_tombstone("session-fenced", Some("user_confirmed_delete"))
                .unwrap();
            deleting_barrier.wait();
            receipt
        });

        // The barrier releases only after the canonical delete and its outbox
        // committed. Every late writer must observe the durable fence inside
        // its own transaction rather than recreating the aggregate.
        deletion_committed.wait();
        let late_message = ChatMessage {
            role: "assistant".into(),
            content: "LATE_MESSAGE_MUST_NOT_RESURRECT".into(),
        };
        for error in [
            store
                .save_message("session-fenced", &late_message)
                .unwrap_err(),
            store
                .create_chat_session("session-fenced", "Late create")
                .unwrap_err(),
            store
                .rename_chat_session("session-fenced", "Late rename")
                .unwrap_err(),
            store.touch_chat_session("session-fenced").unwrap_err(),
            store
                .save_snapshot("session-fenced", &LifeModel::default())
                .unwrap_err(),
            store
                .import_messages(&[ExportedMessage {
                    session_id: "session-fenced".into(),
                    role: "user".into(),
                    content: "LATE_IMPORT_MUST_NOT_RESURRECT".into(),
                    created_at: Utc::now().to_rfc3339(),
                }])
                .unwrap_err(),
            store
                .replace_all_messages(&[ExportedMessage {
                    session_id: "session-fenced".into(),
                    role: "user".into(),
                    content: "LATE_REPLACE_MUST_NOT_RESURRECT".into(),
                    created_at: Utc::now().to_rfc3339(),
                }])
                .unwrap_err(),
        ] {
            assert!(error
                .to_string()
                .contains("conversation_canonical_tombstoned"));
        }
        let receipt = delete.join().unwrap();

        assert!(store
            .load_recent_messages("session-fenced", 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            store.load_recent_messages("unrelated-session", 10).unwrap()[0].content,
            "UNRELATED_BODY_MUST_SURVIVE_FAILED_IMPORT"
        );
        assert_eq!(
            store
                .search_text_memories(None, "MANUAL_LONG_TERM_MEMORY_MUST_SURVIVE", 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .search_text_memories(None, "LIFECYCLE_MEMORY_MUST_SURVIVE", 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store.projection_summary(&receipt.event_id).unwrap().pending,
            5
        );
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
        let type_only_id = store
            .save_memory_record(
                "owner-proof-session",
                "type alone is not ownership",
                "knowledge_note",
                "manual",
                &[],
                "private",
                None,
            )
            .unwrap();
        let tag_only_id = store
            .save_memory_record(
                "owner-proof-session",
                "tag alone is not ownership",
                "untrusted_legacy",
                "manual",
                &["canonical_owner:knowledge_note".into()],
                "private",
                None,
            )
            .unwrap();

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
        assert!(store
            .load_verified_knowledge_note_projection(
                &write.canonical_mutation.event_id,
                &CanonicalVectorOwnerRef::new(
                    "memory_record",
                    &write.knowledge_note_id.to_string(),
                )
                .unwrap(),
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
    fn knowledge_note_v2_journal_migration_scrubs_enumerable_body_digest() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("memory-index-v2.db");
        let operation_id = uuid::Uuid::new_v4().to_string();
        let leaked_digest = "sha256:enumerable-low-entropy-body-digest";
        let first = {
            let store = MemoryStore::new(&db_path).unwrap();
            store
                .save_knowledge_note_idempotent_with_outbox(
                    &operation_id,
                    "manual-session",
                    "LOW_ENTROPY_PRIVATE_BODY",
                    "knowledge_note",
                    "manual",
                    &["manual".to_string()],
                    "private",
                )
                .unwrap()
        };
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "DROP INDEX idx_knowledge_note_operation_memory;
                 ALTER TABLE knowledge_note_operations
                 RENAME TO knowledge_note_operations_v4_source;
                 CREATE TABLE memory_index_operations (
                    operation_id TEXT PRIMARY KEY,
                    payload_digest TEXT NOT NULL,
                    memory_id INTEGER NOT NULL,
                    outbox_event_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(memory_id) REFERENCES memories(id),
                    FOREIGN KEY(outbox_event_id) REFERENCES canonical_outbox_events(event_id)
                 );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO memory_index_operations (
                    operation_id, payload_digest, memory_id, outbox_event_id, created_at
                 ) SELECT operation_id, ?1, memory_id, outbox_event_id, created_at
                   FROM knowledge_note_operations_v4_source",
                [leaked_digest],
            )
            .unwrap();
            conn.execute("DROP TABLE knowledge_note_operations_v4_source", [])
                .unwrap();
            conn.execute(
                "UPDATE canonical_outbox_events
                 SET aggregate_kind = 'memory_record', mutation_kind = 'indexed'
                 WHERE event_id = ?1",
                [&first.canonical_mutation.event_id],
            )
            .unwrap();
        }

        let reopened = MemoryStore::new(&db_path).unwrap();
        let replay = reopened
            .save_knowledge_note_idempotent_with_outbox(
                &operation_id,
                "manual-session",
                "LOW_ENTROPY_PRIVATE_BODY",
                "knowledge_note",
                "manual",
                &["manual".to_string()],
                "private",
            )
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.knowledge_note_id, first.knowledge_note_id);
        assert_eq!(
            replay.canonical_mutation.event_id,
            first.canonical_mutation.event_id
        );
        let snapshot = reopened.vector_rebuild_source_snapshot().unwrap();
        let rebuild_record = reopened
            .load_vector_rebuild_source_page(0, snapshot.through_memory_id, 10)
            .unwrap()
            .into_iter()
            .find(|record| record.memory.id == replay.knowledge_note_id)
            .unwrap();
        let legacy_owner = rebuild_record.canonical_owner.unwrap();
        assert_eq!(legacy_owner.kind(), "memory_record");
        assert_eq!(legacy_owner.id(), replay.knowledge_note_id.to_string());
        assert_eq!(
            reopened
                .load_verified_knowledge_note_projection(
                    &replay.canonical_mutation.event_id,
                    &legacy_owner,
                )
                .unwrap()
                .id,
            replay.knowledge_note_id
        );
        let conn = reopened.conn.lock().unwrap();
        let columns = {
            let mut statement = conn
                .prepare("PRAGMA table_info(knowledge_note_operations)")
                .unwrap();
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            columns
        };
        assert!(columns.iter().any(|column| column == "operation_digest"));
        assert!(!columns.iter().any(|column| column == "payload_digest"));
        let operation_digest: String = conn
            .query_row(
                "SELECT operation_digest FROM knowledge_note_operations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(operation_digest, leaked_digest);
        assert!(!operation_digest.contains("LOW_ENTROPY_PRIVATE_BODY"));
        let legacy_table_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'memory_index_operations'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_table_exists, 0);
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
            ),
            (
                "manual-session",
                "different body",
                "knowledge_note",
                "manual",
                vec!["manual".to_string()],
                "private",
            ),
            (
                "manual-session",
                "canonical body",
                "other-type",
                "manual",
                vec!["manual".to_string()],
                "private",
            ),
            (
                "manual-session",
                "canonical body",
                "knowledge_note",
                "other-source",
                vec!["manual".to_string()],
                "private",
            ),
            (
                "manual-session",
                "canonical body",
                "knowledge_note",
                "manual",
                vec!["changed".to_string()],
                "private",
            ),
            (
                "manual-session",
                "canonical body",
                "knowledge_note",
                "manual",
                vec!["manual".to_string()],
                "sensitive",
            ),
        ];

        for (session_id, content, content_type, source, tags, privacy_level) in variants {
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
            assert!(error.to_string().contains("different canonical payload"));
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
    fn lifecycle_compatibility_projection_is_event_idempotent_and_marker_is_ref_only() {
        let store = MemoryStore::new_in_memory().unwrap();
        let tags = vec!["memory_id:memory:test".to_string()];
        let first = store
            .project_lifecycle_memory(
                "outbox:test",
                "memory:test",
                "session-test",
                "PROJECTION_BODY_SENTINEL",
                "lifecycle_memory_projection",
                &tags,
                "private",
                None,
            )
            .unwrap();
        let second = store
            .project_lifecycle_memory(
                "outbox:test",
                "memory:test",
                "session-test",
                "PROJECTION_BODY_SENTINEL",
                "lifecycle_memory_projection",
                &tags,
                "private",
                None,
            )
            .unwrap();

        assert_eq!(first, second);
        let conn = store.conn.lock().unwrap();
        let projected_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM memories WHERE source = 'memory_lifecycle:memory:test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let marker: String = conn
            .query_row(
                "SELECT event_id || aggregate_kind || aggregate_id || mutation_kind
                 FROM memory_materialization_projections WHERE event_id = 'outbox:test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(projected_rows, 1);
        assert!(first.is_some());
        assert!(!marker.contains("PROJECTION_BODY_SENTINEL"));
    }

    #[test]
    fn lifecycle_deletion_marker_blocks_late_create_and_reasserts_delete() {
        let store = MemoryStore::new_in_memory().unwrap();
        let tags = vec![
            "canonical_owner:memory_lifecycle".to_string(),
            "memory_id:memory:deleted".to_string(),
        ];
        let created = store
            .project_lifecycle_memory(
                "outbox:create-before-delete",
                "memory:deleted",
                "session-test",
                "ORIGINAL_BODY",
                "lifecycle_memory_projection",
                &tags,
                "private",
                None,
            )
            .unwrap();
        assert!(created.is_some());
        store
            .save_memory_record(
                "session-test",
                "LEGACY_ALT_SOURCE_BODY_MUST_BE_SCRUBBED",
                "lifecycle_memory_projection",
                "legacy_projection_v0",
                &tags,
                "private",
                None,
            )
            .unwrap();
        assert_eq!(
            store
                .project_lifecycle_memory_tombstone("outbox:delete-after-create", "memory:deleted")
                .unwrap(),
            2
        );

        let late = store
            .project_lifecycle_memory(
                "outbox:late-create",
                "memory:deleted",
                "session-test",
                "LATE_BODY_MUST_NOT_RESURRECT",
                "lifecycle_memory_projection",
                &tags,
                "private",
                None,
            )
            .unwrap();
        assert_eq!(late, None);

        assert!(store
            .search_text_memories(None, "ORIGINAL_BODY", 10)
            .unwrap()
            .is_empty());

        // Re-applying an already-marked tombstone is deliberately
        // state-enforcing, not merely marker-idempotent. Simulate a stale row
        // written by an older binary and prove the same canonical delete event
        // scrubs the raw duplicate without a second delete request.
        store
            .save_memory_record(
                "session-test",
                "LEGACY_STALE_BODY_MUST_BE_SCRUBBED",
                "lifecycle_memory_projection",
                "legacy_projection_after_marker",
                &tags,
                "private",
                None,
            )
            .unwrap();
        assert_eq!(
            store
                .project_lifecycle_memory_tombstone("outbox:delete-after-create", "memory:deleted")
                .unwrap(),
            1
        );

        let conn = store.conn.lock().unwrap();
        let (derived_rows, raw_sentinels, late_marker): (i64, i64, i64) = conn
            .query_row(
                "SELECT
                    COUNT(*),
                    COALESCE(SUM(CASE WHEN content IN (
                        'ORIGINAL_BODY',
                        'LATE_BODY_MUST_NOT_RESURRECT',
                        'LEGACY_ALT_SOURCE_BODY_MUST_BE_SCRUBBED',
                        'LEGACY_STALE_BODY_MUST_BE_SCRUBBED'
                    ) THEN 1 ELSE 0 END), 0),
                    (SELECT COUNT(*) FROM memory_materialization_projections
                     WHERE event_id = 'outbox:late-create')
                 FROM memories
                 WHERE source = 'memory_lifecycle:memory:deleted'
                    OR content IN (
                        'LEGACY_ALT_SOURCE_BODY_MUST_BE_SCRUBBED',
                        'LEGACY_STALE_BODY_MUST_BE_SCRUBBED'
                    )",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(derived_rows, 0);
        assert_eq!(raw_sentinels, 0);
        assert_eq!(late_marker, 1);
    }

    #[test]
    fn vector_rebuild_source_uses_stable_bounded_memory_id_pages() {
        let store = MemoryStore::new_in_memory().unwrap();
        store
            .save_message(
                "conversation-only",
                &ChatMessage {
                    role: "user".into(),
                    content: "CONVERSATION_BODY_MUST_NOT_ENTER_REBUILD_PAGE".into(),
                },
            )
            .unwrap();
        for index in 0..130 {
            store
                .save_memory_record(
                    "memory-session",
                    &format!("memory-{index}"),
                    "preference",
                    &format!("memory_lifecycle:{index}"),
                    &[],
                    "private",
                    None,
                )
                .unwrap();
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
