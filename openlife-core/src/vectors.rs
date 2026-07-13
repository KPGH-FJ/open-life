use crate::embedding::{
    validate_embedding_vector, EmbeddingProfile, MAX_EMBEDDING_DIMENSION,
    UNKNOWN_EMBEDDING_PROFILE_ID,
};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MAX_VECTOR_SEARCH_CANDIDATES: usize = 2_000;
pub const VECTOR_REBUILD_BATCH_LIMIT: usize = 64;
const CANONICAL_VECTOR_OWNER_KINDS: [&str; 3] =
    ["memory_lifecycle", "memory_record", "knowledge_note"];

/// Stable, validated identity for a canonical asset materialized in the
/// derived vector store. Provenance strings and session ids are deliberately
/// not accepted as ownership evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalVectorOwnerRef {
    kind: String,
    id: String,
}

impl CanonicalVectorOwnerRef {
    pub fn new(kind: &str, id: &str) -> Result<Self> {
        validate_projection_ref("aggregate_kind", kind)?;
        validate_projection_ref("aggregate_id", id)?;
        if !CANONICAL_VECTOR_OWNER_KINDS.contains(&kind) {
            anyhow::bail!("unsupported canonical vector owner");
        }
        Ok(Self {
            kind: kind.to_string(),
            id: id.to_string(),
        })
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source(&self) -> String {
        format!("{}:{}", self.kind, self.id)
    }
}

/// A memory chunk with embedding stored in SQLite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryChunk {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub source: String,
    pub created_at: String,
    pub tier: i64,
    pub access_count: i64,
    pub last_accessed_at: String,
    pub importance_score: f32,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub summary: Option<String>,
}

/// Item used for batch insert into the vector store.
#[derive(Debug, Clone)]
pub struct VectorInsertItem<'a> {
    pub session_id: &'a str,
    pub content: &'a str,
    pub embedding: &'a [f32],
    pub profile: &'a EmbeddingProfile,
    pub source: &'a str,
}

#[derive(Clone)]
pub struct VectorStore {
    conn: Arc<Mutex<Connection>>,
    rebuild_execution: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIntegrityReport {
    pub total_chunks: i64,
    pub corrupt_embedding_count: i64,
    pub unknown_profile_count: i64,
    pub profile_dimension_mismatch_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorRebuildEvidence {
    pub expected_profile_id: String,
    pub expected_dimension: usize,
    pub incompatible_profiles: Vec<String>,
    pub unknown_profile_count: usize,
    pub profile_mismatch_count: usize,
    pub dimension_mismatch_count: usize,
    pub corrupt_embedding_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VectorSearchOutcome {
    Matches {
        matches: Vec<(MemoryChunk, f32)>,
        /// Incompatible rows are excluded from similarity calculation but remain
        /// visible so callers can schedule a rebuild without discarding valid hits.
        rebuild: Option<VectorRebuildEvidence>,
    },
    RebuildRequired(VectorRebuildEvidence),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorRebuildSourceSnapshot {
    pub through_memory_id: i64,
    pub total_count: usize,
    pub metadata_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorRebuildJobStatus {
    Prepared,
    Running,
    Paused,
    CancelRequested,
    Cancelled,
    Completed,
    Failed,
}

impl VectorRebuildJobStatus {
    fn from_stored(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "cancel_requested" => Ok(Self::CancelRequested),
            "cancelled" => Ok(Self::Cancelled),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            _ => anyhow::bail!("unknown vector rebuild status"),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Completed | Self::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorRebuildJob {
    pub job_id: String,
    pub status: VectorRebuildJobStatus,
    pub source_snapshot: VectorRebuildSourceSnapshot,
    pub last_processed_memory_id: i64,
    pub processed: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub embedding_profile_id: Option<String>,
    pub embedding_dimension: Option<usize>,
    pub provider_invocations: usize,
    pub cache_hits: usize,
    pub remote_unknown_provider_attempts: usize,
    pub last_error_digest: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct VectorRebuildBatchItem {
    pub memory_id: i64,
    pub chunk: Option<ExportedVectorChunk>,
    pub canonical_owner: Option<CanonicalVectorOwnerRef>,
    pub provider_dispatch_count: usize,
    pub cache_hit: bool,
}

impl VectorStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open vector sqlite db at {:?}", db_path))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            rebuild_execution: Arc::new(tokio::sync::Mutex::new(())),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory vector sqlite db")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            rebuild_execution: Arc::new(tokio::sync::Mutex::new(())),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "vector_store",
            &["vectors"],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            rebuild_execution: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        Ok(Self {
            conn: Arc::new(Mutex::new(
                crate::sqlite_migration::unavailable_read_only_sentinel("vector_store")?,
            )),
            rebuild_execution: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Serializes rebuild drivers only while allowing status/cancel reads to
    /// use the durable SQLite state. The guard is never the vector connection
    /// guard, so provider awaits cannot block search or cancellation.
    pub async fn acquire_rebuild_execution(&self) -> tokio::sync::OwnedMutexGuard<()> {
        self.rebuild_execution.clone().lock_owned().await
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS vectors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding_json TEXT NOT NULL,
                embedding_blob BLOB,
                embedding_profile_id TEXT NOT NULL DEFAULT 'unknown',
                embedding_dimension INTEGER NOT NULL DEFAULT 0,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL,
                tier INTEGER NOT NULL DEFAULT 2,
                access_count INTEGER NOT NULL DEFAULT 0,
                last_accessed_at TEXT,
                importance_score REAL NOT NULL DEFAULT 0.5,
                archived INTEGER NOT NULL DEFAULT 0,
                archived_at TEXT,
                summary TEXT
            )",
            [],
        )?;
        for (column, definition) in [
            ("tier", "INTEGER NOT NULL DEFAULT 2"),
            ("access_count", "INTEGER NOT NULL DEFAULT 0"),
            ("last_accessed_at", "TEXT"),
            ("importance_score", "REAL NOT NULL DEFAULT 0.5"),
            ("archived", "INTEGER NOT NULL DEFAULT 0"),
            ("archived_at", "TEXT"),
            ("summary", "TEXT"),
            ("embedding_blob", "BLOB"),
            ("embedding_profile_id", "TEXT NOT NULL DEFAULT 'unknown'"),
            ("embedding_dimension", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            crate::sqlite_migration::ensure_column(&tx, "vectors", column, definition)?;
        }
        let legacy_embeddings = {
            let mut statement = tx.prepare(
                "SELECT id, embedding_json FROM vectors
                 WHERE embedding_blob IS NULL OR length(embedding_blob) = 0",
            )?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        for (id, legacy_json) in legacy_embeddings {
            if let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&legacy_json) {
                if !embedding.is_empty() {
                    tx.execute(
                        "UPDATE vectors SET embedding_blob = ?2, embedding_json = '[]' WHERE id = ?1",
                        params![id, encode_embedding_blob(&embedding)],
                    )?;
                }
            }
        }
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_session ON vectors(session_id, created_at)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_tier ON vectors(tier)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_archived ON vectors(archived)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_profile_search ON vectors(archived, embedding_profile_id, embedding_dimension, created_at DESC)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_session_profile_search ON vectors(session_id, archived, embedding_profile_id, embedding_dimension, created_at DESC)",
            [],
        )?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_vectors_canonical_owner_source
             ON vectors(source)
             WHERE source GLOB 'memory_lifecycle:*'
                OR source GLOB 'memory_record:*'
                OR source GLOB 'knowledge_note:*'",
            [],
        )?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS vector_profile_stats (
                scope_kind TEXT NOT NULL CHECK(scope_kind IN ('global', 'session')),
                scope_id TEXT NOT NULL,
                archived INTEGER NOT NULL CHECK(archived IN (0, 1)),
                embedding_profile_id TEXT NOT NULL,
                embedding_dimension INTEGER NOT NULL,
                row_count INTEGER NOT NULL CHECK(row_count >= 0),
                malformed_blob_count INTEGER NOT NULL CHECK(malformed_blob_count >= 0),
                PRIMARY KEY (
                    scope_kind, scope_id, archived,
                    embedding_profile_id, embedding_dimension
                )
            ) WITHOUT ROWID;
            CREATE TABLE IF NOT EXISTS vector_tombstone_projections (
                tombstone_id TEXT PRIMARY KEY,
                aggregate_kind TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_vector_tombstone_aggregate
            ON vector_tombstone_projections(aggregate_kind, aggregate_id);
            CREATE TABLE IF NOT EXISTS vector_materialization_projections (
                event_id TEXT PRIMARY KEY,
                aggregate_kind TEXT NOT NULL,
                aggregate_id TEXT NOT NULL,
                mutation_kind TEXT NOT NULL,
                vector_id INTEGER,
                applied_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_vector_materialization_aggregate
            ON vector_materialization_projections(aggregate_kind, aggregate_id);
            CREATE TABLE IF NOT EXISTS vector_memory_retrieval_projections (
                owner_kind TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                disposition TEXT NOT NULL CHECK(disposition IN ('active', 'archived')),
                revision INTEGER NOT NULL CHECK(revision > 0),
                event_id TEXT NOT NULL UNIQUE,
                applied_at TEXT NOT NULL,
                PRIMARY KEY(owner_kind, owner_id)
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS idx_vector_memory_retrieval_disposition
            ON vector_memory_retrieval_projections(disposition, applied_at DESC);
            DROP TRIGGER IF EXISTS vector_materialization_owner_insert;
            DROP TRIGGER IF EXISTS vector_materialization_owner_update;
            CREATE TRIGGER vector_materialization_owner_insert
            BEFORE INSERT ON vector_materialization_projections
            WHEN NEW.vector_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM vectors
                WHERE id = NEW.vector_id
                  AND source = NEW.aggregate_kind || ':' || NEW.aggregate_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'vector materialization owner mismatch');
            END;
            CREATE TRIGGER vector_materialization_owner_update
            BEFORE UPDATE OF vector_id, aggregate_kind, aggregate_id
            ON vector_materialization_projections
            WHEN NEW.vector_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM vectors
                WHERE id = NEW.vector_id
                  AND source = NEW.aggregate_kind || ':' || NEW.aggregate_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'vector materialization owner mismatch');
            END;
            DELETE FROM vectors
            WHERE EXISTS (
                SELECT 1 FROM vector_materialization_projections projections
                WHERE projections.aggregate_kind = 'memory_lifecycle'
                  AND projections.mutation_kind = 'deleted'
                  AND vectors.source = 'memory_lifecycle:' || projections.aggregate_id
            );
            CREATE TABLE IF NOT EXISTS vector_rebuild_jobs (
                job_id TEXT PRIMARY KEY,
                status TEXT NOT NULL CHECK(status IN (
                    'prepared', 'running', 'paused', 'cancel_requested',
                    'cancelled', 'completed', 'failed'
                )),
                source_through_memory_id INTEGER NOT NULL,
                source_total_count INTEGER NOT NULL,
                source_metadata_digest TEXT NOT NULL,
                last_processed_memory_id INTEGER NOT NULL DEFAULT 0,
                processed_count INTEGER NOT NULL DEFAULT 0,
                indexed_count INTEGER NOT NULL DEFAULT 0,
                skipped_count INTEGER NOT NULL DEFAULT 0,
                embedding_profile_id TEXT,
                embedding_dimension INTEGER,
                provider_invocations INTEGER NOT NULL DEFAULT 0,
                cache_hits INTEGER NOT NULL DEFAULT 0,
                remote_unknown_provider_attempts INTEGER NOT NULL DEFAULT 0,
                last_error_digest TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_vector_rebuild_jobs_updated
            ON vector_rebuild_jobs(updated_at DESC);
            CREATE TABLE IF NOT EXISTS vector_rebuild_items (
                job_id TEXT NOT NULL,
                memory_id INTEGER NOT NULL,
                outcome TEXT NOT NULL CHECK(outcome IN ('indexed', 'skipped')),
                session_id TEXT,
                content TEXT,
                embedding_blob BLOB,
                embedding_profile_id TEXT,
                embedding_dimension INTEGER,
                source TEXT,
                created_at TEXT,
                tier INTEGER,
                access_count INTEGER,
                last_accessed_at TEXT,
                importance_score REAL,
                archived INTEGER,
                archived_at TEXT,
                summary TEXT,
                PRIMARY KEY(job_id, memory_id)
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS idx_vector_rebuild_items_job
            ON vector_rebuild_items(job_id, memory_id);
            DROP TRIGGER IF EXISTS vectors_profile_stats_after_insert;
            DROP TRIGGER IF EXISTS vectors_profile_stats_after_delete;
            DROP TRIGGER IF EXISTS vectors_profile_stats_after_update;
            CREATE TRIGGER vectors_profile_stats_after_insert
            AFTER INSERT ON vectors
            BEGIN
                INSERT INTO vector_profile_stats (
                    scope_kind, scope_id, archived, embedding_profile_id,
                    embedding_dimension, row_count, malformed_blob_count
                ) VALUES (
                    'global', '', NEW.archived, NEW.embedding_profile_id,
                    NEW.embedding_dimension, 1,
                    CASE WHEN NEW.embedding_blob IS NOT NULL AND (
                        length(NEW.embedding_blob) = 0
                        OR length(NEW.embedding_blob) % 4 != 0
                        OR (NEW.embedding_dimension > 0 AND length(NEW.embedding_blob) != NEW.embedding_dimension * 4)
                    ) THEN 1 ELSE 0 END
                ) ON CONFLICT DO UPDATE SET
                    row_count = row_count + 1,
                    malformed_blob_count = malformed_blob_count + excluded.malformed_blob_count;
                INSERT INTO vector_profile_stats (
                    scope_kind, scope_id, archived, embedding_profile_id,
                    embedding_dimension, row_count, malformed_blob_count
                ) VALUES (
                    'session', NEW.session_id, NEW.archived, NEW.embedding_profile_id,
                    NEW.embedding_dimension, 1,
                    CASE WHEN NEW.embedding_blob IS NOT NULL AND (
                        length(NEW.embedding_blob) = 0
                        OR length(NEW.embedding_blob) % 4 != 0
                        OR (NEW.embedding_dimension > 0 AND length(NEW.embedding_blob) != NEW.embedding_dimension * 4)
                    ) THEN 1 ELSE 0 END
                ) ON CONFLICT DO UPDATE SET
                    row_count = row_count + 1,
                    malformed_blob_count = malformed_blob_count + excluded.malformed_blob_count;
            END;
            CREATE TRIGGER vectors_profile_stats_after_delete
            AFTER DELETE ON vectors
            BEGIN
                UPDATE vector_profile_stats SET
                    row_count = row_count - 1,
                    malformed_blob_count = malformed_blob_count -
                        CASE WHEN OLD.embedding_blob IS NOT NULL AND (
                            length(OLD.embedding_blob) = 0
                            OR length(OLD.embedding_blob) % 4 != 0
                            OR (OLD.embedding_dimension > 0 AND length(OLD.embedding_blob) != OLD.embedding_dimension * 4)
                        ) THEN 1 ELSE 0 END
                WHERE scope_kind = 'global' AND scope_id = ''
                  AND archived = OLD.archived
                  AND embedding_profile_id = OLD.embedding_profile_id
                  AND embedding_dimension = OLD.embedding_dimension;
                UPDATE vector_profile_stats SET
                    row_count = row_count - 1,
                    malformed_blob_count = malformed_blob_count -
                        CASE WHEN OLD.embedding_blob IS NOT NULL AND (
                            length(OLD.embedding_blob) = 0
                            OR length(OLD.embedding_blob) % 4 != 0
                            OR (OLD.embedding_dimension > 0 AND length(OLD.embedding_blob) != OLD.embedding_dimension * 4)
                        ) THEN 1 ELSE 0 END
                WHERE scope_kind = 'session' AND scope_id = OLD.session_id
                  AND archived = OLD.archived
                  AND embedding_profile_id = OLD.embedding_profile_id
                  AND embedding_dimension = OLD.embedding_dimension;
                DELETE FROM vector_profile_stats WHERE row_count = 0;
            END;
            CREATE TRIGGER vectors_profile_stats_after_update
            AFTER UPDATE OF session_id, archived, embedding_profile_id, embedding_dimension, embedding_blob ON vectors
            BEGIN
                UPDATE vector_profile_stats SET
                    row_count = row_count - 1,
                    malformed_blob_count = malformed_blob_count -
                        CASE WHEN OLD.embedding_blob IS NOT NULL AND (
                            length(OLD.embedding_blob) = 0
                            OR length(OLD.embedding_blob) % 4 != 0
                            OR (OLD.embedding_dimension > 0 AND length(OLD.embedding_blob) != OLD.embedding_dimension * 4)
                        ) THEN 1 ELSE 0 END
                WHERE scope_kind = 'global' AND scope_id = ''
                  AND archived = OLD.archived
                  AND embedding_profile_id = OLD.embedding_profile_id
                  AND embedding_dimension = OLD.embedding_dimension;
                UPDATE vector_profile_stats SET
                    row_count = row_count - 1,
                    malformed_blob_count = malformed_blob_count -
                        CASE WHEN OLD.embedding_blob IS NOT NULL AND (
                            length(OLD.embedding_blob) = 0
                            OR length(OLD.embedding_blob) % 4 != 0
                            OR (OLD.embedding_dimension > 0 AND length(OLD.embedding_blob) != OLD.embedding_dimension * 4)
                        ) THEN 1 ELSE 0 END
                WHERE scope_kind = 'session' AND scope_id = OLD.session_id
                  AND archived = OLD.archived
                  AND embedding_profile_id = OLD.embedding_profile_id
                  AND embedding_dimension = OLD.embedding_dimension;
                DELETE FROM vector_profile_stats WHERE row_count = 0;
                INSERT INTO vector_profile_stats (
                    scope_kind, scope_id, archived, embedding_profile_id,
                    embedding_dimension, row_count, malformed_blob_count
                ) VALUES (
                    'global', '', NEW.archived, NEW.embedding_profile_id,
                    NEW.embedding_dimension, 1,
                    CASE WHEN NEW.embedding_blob IS NOT NULL AND (
                        length(NEW.embedding_blob) = 0
                        OR length(NEW.embedding_blob) % 4 != 0
                        OR (NEW.embedding_dimension > 0 AND length(NEW.embedding_blob) != NEW.embedding_dimension * 4)
                    ) THEN 1 ELSE 0 END
                ) ON CONFLICT DO UPDATE SET
                    row_count = row_count + 1,
                    malformed_blob_count = malformed_blob_count + excluded.malformed_blob_count;
                INSERT INTO vector_profile_stats (
                    scope_kind, scope_id, archived, embedding_profile_id,
                    embedding_dimension, row_count, malformed_blob_count
                ) VALUES (
                    'session', NEW.session_id, NEW.archived, NEW.embedding_profile_id,
                    NEW.embedding_dimension, 1,
                    CASE WHEN NEW.embedding_blob IS NOT NULL AND (
                        length(NEW.embedding_blob) = 0
                        OR length(NEW.embedding_blob) % 4 != 0
                        OR (NEW.embedding_dimension > 0 AND length(NEW.embedding_blob) != NEW.embedding_dimension * 4)
                    ) THEN 1 ELSE 0 END
                ) ON CONFLICT DO UPDATE SET
                    row_count = row_count + 1,
                    malformed_blob_count = malformed_blob_count + excluded.malformed_blob_count;
            END;",
        )?;
        // Older releases copied every ordinary chat turn into the vector store. Conversation
        // text is now owned by MemoryStore.messages and searched there by reference, so these
        // derived content copies must not survive migration.
        tx.execute(
            "DELETE FROM vectors WHERE source IN ('user_message', 'assistant_reply')",
            [],
        )?;
        // The profile summary is a transactionally maintained projection. Rebuild
        // it once during migration/startup so search latency depends on the number
        // of distinct profiles, never on the number of vector rows.
        tx.execute("DELETE FROM vector_profile_stats", [])?;
        tx.execute(
            "INSERT INTO vector_profile_stats (
                scope_kind, scope_id, archived, embedding_profile_id,
                embedding_dimension, row_count, malformed_blob_count
             )
             SELECT 'global', '', archived, embedding_profile_id,
                    embedding_dimension, COUNT(*),
                    SUM(CASE WHEN embedding_blob IS NOT NULL AND (
                        length(embedding_blob) = 0
                        OR length(embedding_blob) % 4 != 0
                        OR (embedding_dimension > 0 AND length(embedding_blob) != embedding_dimension * 4)
                    ) THEN 1 ELSE 0 END)
             FROM vectors
             GROUP BY archived, embedding_profile_id, embedding_dimension",
            [],
        )?;
        tx.execute(
            "INSERT INTO vector_profile_stats (
                scope_kind, scope_id, archived, embedding_profile_id,
                embedding_dimension, row_count, malformed_blob_count
             )
             SELECT 'session', session_id, archived, embedding_profile_id,
                    embedding_dimension, COUNT(*),
                    SUM(CASE WHEN embedding_blob IS NOT NULL AND (
                        length(embedding_blob) = 0
                        OR length(embedding_blob) % 4 != 0
                        OR (embedding_dimension > 0 AND length(embedding_blob) != embedding_dimension * 4)
                    ) THEN 1 ELSE 0 END)
             FROM vectors
             GROUP BY session_id, archived, embedding_profile_id, embedding_dimension",
            [],
        )?;
        crate::sqlite_migration::record_schema_version(&tx, "vector_store", 10)?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert(
        &self,
        session_id: &str,
        content: &str,
        embedding: &[f32],
        profile: &EmbeddingProfile,
        source: &str,
    ) -> Result<i64> {
        validate_embedding_profile(embedding, profile)?;
        if is_reserved_canonical_vector_source(source) {
            anyhow::bail!("generic vector insert cannot claim a canonical owner");
        }
        let blob = encode_embedding_blob(embedding);
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        if !source.starts_with("memory_lifecycle:") && vector_session_tombstoned(&conn, session_id)?
        {
            anyhow::bail!("vector_session_canonical_source_tombstoned");
        }
        conn.execute(
            "INSERT INTO vectors (session_id, content, embedding_json, embedding_blob, embedding_profile_id, embedding_dimension, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![session_id, content, blob, profile.id, profile.dimension as i64, source, chrono::Utc::now().to_rfc3339(), 2, 0, Option::<String>::None, 0.5_f32, 0, Option::<String>::None, Option::<String>::None],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn projected_materialization_vector_id(
        &self,
        event_id: &str,
        owner: &CanonicalVectorOwnerRef,
    ) -> Result<Option<i64>> {
        validate_projection_ref("event_id", event_id)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(projected_vector_id(&conn, event_id, owner)?.flatten())
    }

    /// Idempotently materialize one canonical Memory reference. The ref-only
    /// marker and vector row commit together; raw content is supplied by the
    /// replaceable materializer after loading the canonical owner and is never
    /// copied into the outbox or marker.
    #[allow(clippy::too_many_arguments)]
    pub fn project_memory_embedding(
        &self,
        event_id: &str,
        owner: &CanonicalVectorOwnerRef,
        session_id: &str,
        content: &str,
        embedding: &[f32],
        profile: &EmbeddingProfile,
    ) -> Result<Option<i64>> {
        validate_projection_ref("event_id", event_id)?;
        validate_embedding_profile(embedding, profile)?;
        let source = owner.source();
        let blob = encode_embedding_blob(embedding);
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        if let Some(vector_id) = projected_vector_id(&tx, event_id, owner)? {
            tx.commit()?;
            return Ok(vector_id);
        }
        // Lifecycle deletion is a durable projection fence. Checking it under
        // the VectorStore transaction prevents a creation delivery that spent
        // time awaiting an embedding provider from resurrecting a row after a
        // concurrent rollback already applied its tombstone.
        if owner.kind() == "memory_lifecycle"
            && memory_lifecycle_vector_projection_deleted(&tx, owner.id())?
        {
            insert_vector_projection_marker(
                &tx,
                event_id,
                owner.kind(),
                owner.id(),
                "materialized",
                None,
            )?;
            tx.commit()?;
            return Ok(None);
        }
        let existing_id = tx
            .query_row(
                "SELECT id FROM vectors WHERE source = ?1 ORDER BY id ASC LIMIT 1",
                [&source],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let vector_id = if let Some(vector_id) = existing_id {
            tx.execute(
                "UPDATE vectors
                 SET session_id = ?2, content = ?3, embedding_json = '[]',
                     embedding_blob = ?4, embedding_profile_id = ?5,
                     embedding_dimension = ?6, archived = 0, archived_at = NULL,
                     summary = NULL
                 WHERE id = ?1",
                params![
                    vector_id,
                    session_id,
                    content,
                    blob,
                    profile.id,
                    profile.dimension as i64
                ],
            )?;
            vector_id
        } else {
            tx.execute(
                "INSERT INTO vectors (
                    session_id, content, embedding_json, embedding_blob,
                    embedding_profile_id, embedding_dimension, source,
                    created_at, tier, access_count, last_accessed_at,
                    importance_score, archived, archived_at, summary
                 ) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7, 2, 0, NULL, 0.5, 0, NULL, NULL)",
                params![
                    session_id,
                    content,
                    blob,
                    profile.id,
                    profile.dimension as i64,
                    source,
                    chrono::Utc::now().to_rfc3339()
                ],
            )?;
            tx.last_insert_rowid()
        };
        insert_vector_projection_marker(
            &tx,
            event_id,
            owner.kind(),
            owner.id(),
            "materialized",
            Some(vector_id),
        )?;
        apply_memory_retrieval_fence_to_owner(&tx, owner)?;
        tx.commit()?;
        Ok(Some(vector_id))
    }

    /// Project the canonical Memory retrieval head into the derived vector
    /// index. Stable owner identity and monotonic revision are mandatory;
    /// vector row ids are never accepted as authority.
    pub fn project_memory_retrieval_state(
        &self,
        event_id: &str,
        owner: &CanonicalVectorOwnerRef,
        archived: bool,
        revision: u64,
    ) -> Result<usize> {
        validate_projection_ref("event_id", event_id)?;
        if revision == 0 {
            anyhow::bail!("canonical Memory retrieval revision must be positive");
        }
        let revision_raw = i64::try_from(revision)
            .context("canonical Memory retrieval revision exceeds SQLite range")?;
        let disposition = if archived { "archived" } else { "active" };
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let existing = tx
            .query_row(
                "SELECT disposition, revision, event_id
                 FROM vector_memory_retrieval_projections
                 WHERE owner_kind = ?1 AND owner_id = ?2",
                params![owner.kind(), owner.id()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((stored_disposition, stored_revision, stored_event_id)) = existing {
            if stored_revision > revision_raw {
                anyhow::bail!("stale canonical Memory retrieval projection");
            }
            if stored_revision == revision_raw {
                if stored_disposition == disposition && stored_event_id == event_id {
                    let changed = apply_memory_retrieval_fence_to_owner(&tx, owner)?;
                    tx.commit()?;
                    return Ok(changed);
                }
                anyhow::bail!("canonical Memory retrieval projection revision drift");
            }
        }
        tx.execute(
            "INSERT INTO vector_memory_retrieval_projections (
                owner_kind, owner_id, disposition, revision, event_id, applied_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(owner_kind, owner_id) DO UPDATE SET
                disposition = excluded.disposition,
                revision = excluded.revision,
                event_id = excluded.event_id,
                applied_at = excluded.applied_at",
            params![
                owner.kind(),
                owner.id(),
                disposition,
                revision_raw,
                event_id,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        let changed = apply_memory_retrieval_fence_to_owner(&tx, owner)?;
        tx.commit()?;
        Ok(changed)
    }

    pub fn project_memory_lifecycle_tombstone(
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
        let already_applied = vector_projection_applied(&tx, event_id)?;
        let source = format!("memory_lifecycle:{memory_id}");
        // VectorStore is derived. Keep only the metadata fence and remove the
        // duplicate body, embedding, and summary on canonical rollback.
        let deleted = tx.execute("DELETE FROM vectors WHERE source = ?1", [source])?;
        if !already_applied {
            insert_vector_projection_marker(
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

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_memory_projection_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "CREATE TRIGGER fail_vector_memory_projection_for_test
             BEFORE INSERT ON vector_materialization_projections
             BEGIN
                 SELECT RAISE(ABORT, 'injected vector memory projection failure');
             END;",
        )?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn remove_memory_projection_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch("DROP TRIGGER IF EXISTS fail_vector_memory_projection_for_test;")?;
        Ok(())
    }

    pub fn insert_batch(&self, items: &[VectorInsertItem<'_>]) -> Result<usize> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        for item in items {
            validate_embedding_profile(item.embedding, item.profile)?;
            if is_reserved_canonical_vector_source(item.source) {
                anyhow::bail!("generic vector batch cannot claim a canonical owner");
            }
            if !item.source.starts_with("memory_lifecycle:")
                && vector_session_tombstoned(&tx, item.session_id)?
            {
                anyhow::bail!("vector_session_canonical_source_tombstoned");
            }
            let blob = encode_embedding_blob(item.embedding);
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, embedding_blob, embedding_profile_id, embedding_dimension, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![item.session_id, item.content, blob, item.profile.id, item.profile.dimension as i64, item.source, &now, 2, 0, Option::<String>::None, 0.5_f32, 0, Option::<String>::None, Option::<String>::None],
            )?;
        }
        tx.commit()?;
        Ok(items.len())
    }

    fn row_to_chunk(
        row: &rusqlite::Row,
    ) -> rusqlite::Result<(MemoryChunk, Vec<f32>, String, usize)> {
        let embedding_json: String = row.get(3)?;
        let embedding_blob: Option<Vec<u8>> = row.get(4)?;
        let embedding = decode_embedding(embedding_blob.as_deref(), &embedding_json);
        Ok((
            MemoryChunk {
                id: row.get(0)?,
                session_id: row.get(1)?,
                content: row.get(2)?,
                source: row.get(5)?,
                created_at: row.get(6)?,
                tier: row.get(7)?,
                access_count: row.get(8)?,
                last_accessed_at: row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                importance_score: row.get::<_, Option<f32>>(10)?.unwrap_or(0.5),
                archived: row.get::<_, Option<i64>>(11)?.unwrap_or(0) != 0,
                archived_at: row.get::<_, Option<String>>(12)?,
                summary: row.get::<_, Option<String>>(13)?,
            },
            embedding,
            row.get(14)?,
            row.get::<_, i64>(15)?.max(0) as usize,
        ))
    }

    pub fn search(
        &self,
        query_embedding: &[f32],
        profile: &EmbeddingProfile,
        top_k: usize,
    ) -> Result<VectorSearchOutcome> {
        validate_embedding_profile(query_embedding, profile)?;
        let (candidates, rebuild) = {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
            let rebuild = profile_compatibility_evidence(&conn, profile, None)?;
            // Only matching profile metadata can enter similarity calculation. Newer
            // incompatible rows therefore cannot crowd older compatible rows out of the cap.
            let mut stmt = conn.prepare(
                "SELECT id, session_id, content, embedding_json, embedding_blob, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary, embedding_profile_id, embedding_dimension FROM vectors WHERE archived = 0 AND embedding_profile_id = ?1 AND embedding_dimension = ?2 ORDER BY created_at DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(
                params![
                    &profile.id,
                    profile.dimension as i64,
                    MAX_VECTOR_SEARCH_CANDIDATES as i64
                ],
                Self::row_to_chunk,
            )?;
            (rows.collect::<std::result::Result<Vec<_>, _>>()?, rebuild)
        };
        self.score_matching_candidates(query_embedding, profile, candidates, rebuild, top_k)
    }

    /// Search scoped to a specific session_id, with a hard limit on scan size.
    pub fn search_by_session(
        &self,
        session_id: &str,
        query_embedding: &[f32],
        profile: &EmbeddingProfile,
        top_k: usize,
        limit: usize,
    ) -> Result<VectorSearchOutcome> {
        validate_embedding_profile(query_embedding, profile)?;
        let (candidates, rebuild) = {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
            let rebuild = profile_compatibility_evidence(&conn, profile, Some(session_id))?;
            let mut stmt = conn.prepare(
                "SELECT id, session_id, content, embedding_json, embedding_blob, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary, embedding_profile_id, embedding_dimension FROM vectors WHERE session_id = ?1 AND archived = 0 AND embedding_profile_id = ?2 AND embedding_dimension = ?3 ORDER BY created_at DESC LIMIT ?4",
            )?;
            let rows = stmt.query_map(
                params![
                    session_id,
                    &profile.id,
                    profile.dimension as i64,
                    limit.min(MAX_VECTOR_SEARCH_CANDIDATES) as i64
                ],
                Self::row_to_chunk,
            )?;
            (rows.collect::<std::result::Result<Vec<_>, _>>()?, rebuild)
        };
        self.score_matching_candidates(query_embedding, profile, candidates, rebuild, top_k)
    }

    fn score_matching_candidates(
        &self,
        query_embedding: &[f32],
        profile: &EmbeddingProfile,
        candidates: Vec<(MemoryChunk, Vec<f32>, String, usize)>,
        mut rebuild: Option<VectorRebuildEvidence>,
        top_k: usize,
    ) -> Result<VectorSearchOutcome> {
        let mut corrupt_matching = 0usize;
        let mut results = candidates
            .into_iter()
            .filter_map(
                |(chunk, embedding, stored_profile_id, declared_dimension)| {
                    if stored_profile_id != profile.id
                        || declared_dimension != profile.dimension
                        || validate_embedding_vector(&embedding, Some(declared_dimension)).is_err()
                    {
                        corrupt_matching += 1;
                        return None;
                    }
                    let score = cosine_similarity(query_embedding, &embedding);
                    Some((chunk, score))
                },
            )
            .collect::<Vec<_>>();
        if corrupt_matching > 0 {
            let evidence = rebuild.get_or_insert_with(|| empty_rebuild_evidence(profile));
            // The compatibility scan may already have counted malformed blobs.
            // Candidate validation is authoritative for rows considered for
            // scoring, so merge counts without double-counting the same row.
            evidence.corrupt_embedding_count =
                evidence.corrupt_embedding_count.max(corrupt_matching);
            push_bounded_profile(
                &mut evidence.incompatible_profiles,
                format!("{}:dim:{}", profile.id, profile.dimension),
            );
        }
        let has_valid_match = !results.is_empty();
        results.sort_by(|a, b| {
            let comp_a = Self::composite_score(a.1, a.0.tier);
            let comp_b = Self::composite_score(b.1, b.0.tier);
            comp_b
                .partial_cmp(&comp_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top = results.into_iter().take(top_k).collect::<Vec<_>>();
        self.bump_access_for_chunks(&top)?;
        if !has_valid_match {
            if let Some(rebuild) = rebuild {
                return Ok(VectorSearchOutcome::RebuildRequired(rebuild));
            }
        }
        Ok(VectorSearchOutcome::Matches {
            matches: top,
            rebuild,
        })
    }

    fn composite_score(similarity: f32, tier: i64) -> f32 {
        let tier_bonus = match tier {
            1 => 0.15,
            2 => 0.05,
            _ => 0.0,
        };
        similarity + tier_bonus
    }

    fn bump_access_for_chunks(&self, chunks: &[(MemoryChunk, f32)]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        for (chunk, _) in chunks {
            tx.execute(
                "UPDATE vectors SET access_count = access_count + 1, last_accessed_at = ?1 WHERE id = ?2",
                params![&now, chunk.id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Run promotion/demotion heuristics considering access_count, last_accessed_at and importance_score.
    /// - Promote to Hot (tier 1) if: access_count >= 5 AND last_accessed within 7 days OR importance_score >= 0.8.
    /// - Demote to Retrieval (tier 3) if: not accessed in 30 days AND importance_score < 0.4.
    /// - Only considers non-archived chunks.
    pub fn run_tier_maintenance(&self) -> Result<(usize, usize)> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now();
        let hot_cutoff = (now - chrono::Duration::days(7)).to_rfc3339();
        let promoted = conn.execute(
            "UPDATE vectors SET tier = 1 WHERE archived = 0 AND tier > 1 AND ((access_count >= 5 AND (last_accessed_at >= ?1)) OR (importance_score >= 0.8))",
            params![&hot_cutoff],
        )?;
        let retrieval_cutoff = (now - chrono::Duration::days(30)).to_rfc3339();
        let demoted = conn.execute(
            "UPDATE vectors SET tier = 3 WHERE archived = 0 AND tier < 3 AND (last_accessed_at IS NULL OR last_accessed_at < ?1) AND importance_score < 0.4",
            params![&retrieval_cutoff],
        )?;
        Ok((promoted, demoted))
    }

    pub fn count_all_chunks(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM vectors", [], |row| row.get(0))?;
        Ok(count)
    }

    pub fn integrity_report(&self) -> Result<VectorIntegrityReport> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT embedding_json, embedding_blob, embedding_profile_id, embedding_dimension FROM vectors",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?.max(0) as usize,
            ))
        })?;
        let mut total = 0_i64;
        let mut corrupt = 0_i64;
        let mut unknown_profile = 0_i64;
        let mut profile_dimension_mismatch = 0_i64;
        for row in rows {
            total += 1;
            let (embedding_json, embedding_blob, profile_id, dimension) = row?;
            let malformed_blob = embedding_blob.as_deref().is_some_and(|blob| {
                blob.is_empty()
                    || blob.len() % std::mem::size_of::<f32>() != 0
                    || (dimension > 0
                        && blob.len() != dimension.saturating_mul(std::mem::size_of::<f32>()))
            });
            let embedding = decode_embedding(embedding_blob.as_deref(), &embedding_json);
            if malformed_blob
                || validate_embedding_vector(&embedding, (dimension > 0).then_some(dimension))
                    .is_err()
            {
                corrupt += 1;
            }
            if profile_id == UNKNOWN_EMBEDDING_PROFILE_ID || dimension == 0 {
                unknown_profile += 1;
            } else if dimension != embedding.len() {
                profile_dimension_mismatch += 1;
            }
        }
        Ok(VectorIntegrityReport {
            total_chunks: total,
            corrupt_embedding_count: corrupt,
            unknown_profile_count: unknown_profile,
            profile_dimension_mismatch_count: profile_dimension_mismatch,
        })
    }

    pub fn export_all_chunks(&self) -> Result<Vec<ExportedVectorChunk>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, content, embedding_json, embedding_blob, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary, embedding_profile_id, embedding_dimension FROM vectors ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let embedding_json: String = row.get(2)?;
            let embedding_blob: Option<Vec<u8>> = row.get(3)?;
            let embedding = decode_embedding(embedding_blob.as_deref(), &embedding_json);
            Ok(ExportedVectorChunk {
                session_id: row.get(0)?,
                content: row.get(1)?,
                embedding,
                source: row.get(4)?,
                created_at: row.get(5)?,
                tier: row.get(6)?,
                access_count: row.get(7)?,
                last_accessed_at: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                importance_score: row.get::<_, Option<f32>>(9)?.unwrap_or(0.5),
                archived: row.get::<_, Option<i64>>(10)?.unwrap_or(0) != 0,
                archived_at: row.get::<_, Option<String>>(11)?,
                summary: row.get::<_, Option<String>>(12)?,
                embedding_profile_id: row.get(13)?,
                embedding_dimension: row.get::<_, i64>(14)?.max(0) as usize,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to export vectors")
    }

    /// Export only vector rows that have no canonical owner elsewhere.
    /// Canonical Memory and legacy chat rows are derived indexes and must be
    /// recreated from their owner instead of becoming generic import truth.
    pub fn export_portable_chunks(&self) -> Result<Vec<ExportedVectorChunk>> {
        Ok(self
            .export_all_chunks()?
            .into_iter()
            .filter(|chunk| {
                matches!(
                    portable_vector_disposition(&chunk.source),
                    PortableVectorDisposition::Portable
                )
            })
            .collect())
    }

    /// Validate the exact replacement payload without mutating the store.
    /// The same validation is repeated inside `replace_portable_chunks`' SQLite
    /// transaction so a tombstone committed after this preflight still wins.
    pub fn validate_portable_replacement(
        &self,
        chunks: &[ExportedVectorChunk],
    ) -> Result<PortableVectorImportReport> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        validate_portable_vector_chunks(&conn, chunks)
    }

    pub fn clear_all_chunks(&self) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM vectors", [])?;
        rebind_and_validate_materialization_markers(&tx)?;
        tx.commit()?;
        Ok(())
    }

    pub fn import_chunks(&self, chunks: &[ExportedVectorChunk]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        for chunk in chunks {
            if vector_session_tombstoned(&tx, &chunk.session_id)? {
                anyhow::bail!("vector import targets a tombstoned conversation session");
            }
            if is_legacy_chat_vector_source(&chunk.source) {
                continue;
            }
            if is_reserved_canonical_vector_source(&chunk.source) {
                anyhow::bail!("generic vector import cannot claim a canonical owner");
            }
            let embedding_blob = encode_embedding_blob(&chunk.embedding);
            validate_exported_chunk_profile(chunk)?;
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, embedding_blob, embedding_profile_id, embedding_dimension, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![chunk.session_id, chunk.content, embedding_blob, chunk.embedding_profile_id, chunk.embedding_dimension as i64, chunk.source, chunk.created_at, chunk.tier, chunk.access_count, &chunk.last_accessed_at, chunk.importance_score, chunk.archived as i64, &chunk.archived_at, &chunk.summary],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn replace_all_chunks(&self, chunks: &[ExportedVectorChunk]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM vectors", [])?;
        for chunk in chunks {
            if vector_session_tombstoned(&tx, &chunk.session_id)? {
                anyhow::bail!("vector replacement targets a tombstoned conversation session");
            }
            if is_legacy_chat_vector_source(&chunk.source) {
                continue;
            }
            if is_reserved_canonical_vector_source(&chunk.source) {
                anyhow::bail!("generic vector replacement cannot claim a canonical owner");
            }
            let embedding_blob = encode_embedding_blob(&chunk.embedding);
            validate_exported_chunk_profile(chunk)?;
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, embedding_blob, embedding_profile_id, embedding_dimension, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![chunk.session_id, chunk.content, embedding_blob, chunk.embedding_profile_id, chunk.embedding_dimension as i64, chunk.source, chunk.created_at, chunk.tier, chunk.access_count, &chunk.last_accessed_at, chunk.importance_score, chunk.archived as i64, &chunk.archived_at, &chunk.summary],
            )?;
        }
        rebind_and_validate_materialization_markers(&tx)?;
        tx.commit()?;
        Ok(())
    }

    /// Replace only the portable, generic portion of the vector store.
    ///
    /// Canonical projection rows and their materialization markers remain in
    /// place. Canonical or legacy chat rows supplied by old archives are
    /// counted as skipped rather than replayed. All validation and writes occur
    /// in one SQLite transaction, so a failed replacement preserves the prior
    /// portable rows.
    pub fn replace_portable_chunks(
        &self,
        chunks: &[ExportedVectorChunk],
    ) -> Result<PortableVectorImportReport> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let report = validate_portable_vector_chunks(&tx, chunks)?;
        tx.execute(
            "DELETE FROM vectors
             WHERE source NOT GLOB 'memory_lifecycle:*'
               AND source NOT GLOB 'memory_record:*'
               AND source NOT GLOB 'knowledge_note:*'",
            [],
        )?;
        for chunk in chunks {
            if !matches!(
                portable_vector_disposition(&chunk.source),
                PortableVectorDisposition::Portable
            ) {
                continue;
            }
            let embedding_blob = encode_embedding_blob(&chunk.embedding);
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, embedding_blob, embedding_profile_id, embedding_dimension, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![chunk.session_id, chunk.content, embedding_blob, chunk.embedding_profile_id, chunk.embedding_dimension as i64, chunk.source, chunk.created_at, chunk.tier, chunk.access_count, &chunk.last_accessed_at, chunk.importance_score, chunk.archived as i64, &chunk.archived_at, &chunk.summary],
            )?;
        }
        rebind_and_validate_materialization_markers(&tx)?;
        tx.commit()?;
        Ok(report)
    }

    pub fn start_or_resume_rebuild(
        &self,
        source_snapshot: &VectorRebuildSourceSnapshot,
    ) -> Result<VectorRebuildJob> {
        validate_rebuild_source_snapshot(source_snapshot)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let active_job_id = tx
            .query_row(
                "SELECT job_id FROM vector_rebuild_jobs
                 WHERE status IN ('prepared', 'running', 'paused', 'cancel_requested')
                 ORDER BY created_at DESC, job_id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let job_id = if let Some(job_id) = active_job_id {
            let status = read_rebuild_job(&tx, &job_id)?.status;
            if status != VectorRebuildJobStatus::CancelRequested {
                tx.execute(
                    "UPDATE vector_rebuild_jobs
                     SET status = 'running', last_error_digest = NULL, updated_at = ?2
                     WHERE job_id = ?1",
                    params![&job_id, chrono::Utc::now().to_rfc3339()],
                )?;
            }
            job_id
        } else {
            let job_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO vector_rebuild_jobs (
                    job_id, status, source_through_memory_id, source_total_count,
                    source_metadata_digest, created_at, updated_at
                 ) VALUES (?1, 'running', ?2, ?3, ?4, ?5, ?5)",
                params![
                    &job_id,
                    source_snapshot.through_memory_id,
                    source_snapshot.total_count as i64,
                    &source_snapshot.metadata_digest,
                    &now
                ],
            )?;
            job_id
        };
        let job = read_rebuild_job(&tx, &job_id)?;
        tx.commit()?;
        Ok(job)
    }

    pub fn latest_rebuild_job(&self) -> Result<Option<VectorRebuildJob>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let job_id = conn
            .query_row(
                "SELECT job_id FROM vector_rebuild_jobs
                 ORDER BY created_at DESC, job_id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        job_id
            .map(|job_id| read_rebuild_job(&conn, &job_id))
            .transpose()
    }

    pub fn rebuild_job(&self, job_id: &str) -> Result<Option<VectorRebuildJob>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let exists = conn
            .query_row(
                "SELECT 1 FROM vector_rebuild_jobs WHERE job_id = ?1",
                [job_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        exists.then(|| read_rebuild_job(&conn, job_id)).transpose()
    }

    pub fn stage_rebuild_batch(
        &self,
        job_id: &str,
        items: &[VectorRebuildBatchItem],
    ) -> Result<VectorRebuildJob> {
        if items.is_empty() {
            return self
                .rebuild_job(job_id)?
                .ok_or_else(|| anyhow::anyhow!("vector rebuild job not found"));
        }
        if items.len() > VECTOR_REBUILD_BATCH_LIMIT {
            anyhow::bail!("vector rebuild batch exceeds bounded limit");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let job = read_rebuild_job(&tx, job_id)?;
        match job.status {
            VectorRebuildJobStatus::Prepared
            | VectorRebuildJobStatus::Running
            | VectorRebuildJobStatus::Paused => {}
            VectorRebuildJobStatus::CancelRequested => {
                anyhow::bail!("vector rebuild cancellation requested")
            }
            _ => anyhow::bail!("vector rebuild job is terminal"),
        }

        let mut previous_id = job.last_processed_memory_id;
        let mut indexed_delta = 0_usize;
        let mut skipped_delta = 0_usize;
        let mut provider_delta = 0_usize;
        let mut cache_delta = 0_usize;
        let mut profile_id = job.embedding_profile_id.clone();
        let mut profile_dimension = job.embedding_dimension;
        for item in items {
            if item.memory_id <= previous_id
                || item.memory_id > job.source_snapshot.through_memory_id
            {
                anyhow::bail!("vector rebuild batch cursor is not strictly monotonic");
            }
            previous_id = item.memory_id;
            provider_delta = provider_delta.saturating_add(item.provider_dispatch_count);
            cache_delta += usize::from(item.cache_hit);
            if let Some(chunk) = item.chunk.as_ref() {
                let owner = item
                    .canonical_owner
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("vector rebuild item lacks canonical owner"))?;
                if chunk.source != owner.source() {
                    anyhow::bail!("vector rebuild source does not match canonical owner");
                }
                if is_legacy_chat_vector_source(&chunk.source) {
                    anyhow::bail!("legacy chat body cannot enter vector rebuild");
                }
                validate_exported_chunk_profile(chunk)?;
                if let Some(expected) = profile_id.as_ref() {
                    if expected != &chunk.embedding_profile_id
                        || profile_dimension != Some(chunk.embedding_dimension)
                    {
                        anyhow::bail!("embedding profile changed during vector rebuild");
                    }
                } else {
                    profile_id = Some(chunk.embedding_profile_id.clone());
                    profile_dimension = Some(chunk.embedding_dimension);
                }
                tx.execute(
                    "INSERT INTO vector_rebuild_items (
                        job_id, memory_id, outcome, session_id, content, embedding_blob,
                        embedding_profile_id, embedding_dimension, source, created_at,
                        tier, access_count, last_accessed_at, importance_score,
                        archived, archived_at, summary
                     ) VALUES (
                        ?1, ?2, 'indexed', ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                        ?10, ?11, ?12, ?13, ?14, ?15, ?16
                     )",
                    params![
                        job_id,
                        item.memory_id,
                        &chunk.session_id,
                        &chunk.content,
                        encode_embedding_blob(&chunk.embedding),
                        &chunk.embedding_profile_id,
                        chunk.embedding_dimension as i64,
                        &chunk.source,
                        &chunk.created_at,
                        chunk.tier,
                        chunk.access_count,
                        &chunk.last_accessed_at,
                        chunk.importance_score,
                        chunk.archived as i64,
                        &chunk.archived_at,
                        &chunk.summary,
                    ],
                )?;
                indexed_delta += 1;
            } else {
                if item.canonical_owner.is_some() {
                    anyhow::bail!("skipped vector rebuild item cannot claim canonical owner");
                }
                tx.execute(
                    "INSERT INTO vector_rebuild_items (job_id, memory_id, outcome)
                     VALUES (?1, ?2, 'skipped')",
                    params![job_id, item.memory_id],
                )?;
                skipped_delta += 1;
            }
        }
        let processed = job.processed.saturating_add(items.len());
        if processed > job.source_snapshot.total_count {
            anyhow::bail!("vector rebuild processed count exceeds source snapshot");
        }
        tx.execute(
            "UPDATE vector_rebuild_jobs
             SET status = 'running', last_processed_memory_id = ?2,
                 processed_count = ?3, indexed_count = indexed_count + ?4,
                 skipped_count = skipped_count + ?5,
                 embedding_profile_id = ?6, embedding_dimension = ?7,
                 provider_invocations = provider_invocations + ?8,
                 cache_hits = cache_hits + ?9,
                 last_error_digest = NULL, updated_at = ?10
             WHERE job_id = ?1",
            params![
                job_id,
                previous_id,
                processed as i64,
                indexed_delta as i64,
                skipped_delta as i64,
                &profile_id,
                profile_dimension.map(|value| value as i64),
                provider_delta as i64,
                cache_delta as i64,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        let updated = read_rebuild_job(&tx, job_id)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn request_rebuild_cancel(&self, job_id: &str) -> Result<VectorRebuildJob> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let current = read_rebuild_job(&tx, job_id)?;
        if !current.status.is_terminal() {
            tx.execute(
                "UPDATE vector_rebuild_jobs
                 SET status = 'cancel_requested', updated_at = ?2
                 WHERE job_id = ?1",
                params![job_id, chrono::Utc::now().to_rfc3339()],
            )?;
        }
        let updated = read_rebuild_job(&tx, job_id)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn rebuild_cancel_requested(&self, job_id: &str) -> Result<bool> {
        Ok(self
            .rebuild_job(job_id)?
            .is_some_and(|job| job.status == VectorRebuildJobStatus::CancelRequested))
    }

    pub fn settle_rebuild_cancel(&self, job_id: &str) -> Result<VectorRebuildJob> {
        self.settle_rebuild_cancel_with_remote_unknown(job_id, false)
    }

    pub fn settle_rebuild_cancel_with_remote_unknown(
        &self,
        job_id: &str,
        interrupted_provider_attempt: bool,
    ) -> Result<VectorRebuildJob> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let current = read_rebuild_job(&tx, job_id)?;
        if current.status == VectorRebuildJobStatus::Cancelled {
            tx.commit()?;
            return Ok(current);
        }
        if current.status != VectorRebuildJobStatus::CancelRequested {
            anyhow::bail!("vector rebuild cancellation was not requested");
        }
        tx.execute(
            "DELETE FROM vector_rebuild_items WHERE job_id = ?1",
            [job_id],
        )?;
        tx.execute(
            "UPDATE vector_rebuild_jobs
             SET status = 'cancelled',
                 remote_unknown_provider_attempts =
                    remote_unknown_provider_attempts + ?2,
                 updated_at = ?3 WHERE job_id = ?1",
            params![
                job_id,
                i64::from(interrupted_provider_attempt),
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        let updated = read_rebuild_job(&tx, job_id)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn pause_rebuild(&self, job_id: &str, error_digest: &str) -> Result<VectorRebuildJob> {
        validate_rebuild_error_digest(error_digest)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE vector_rebuild_jobs
             SET status = CASE WHEN status = 'cancel_requested' THEN status ELSE 'paused' END,
                 last_error_digest = ?2, updated_at = ?3
             WHERE job_id = ?1 AND status IN ('prepared', 'running', 'paused', 'cancel_requested')",
            params![job_id, error_digest, chrono::Utc::now().to_rfc3339()],
        )?;
        read_rebuild_job(&conn, job_id)
    }

    pub fn fail_rebuild(&self, job_id: &str, error_digest: &str) -> Result<VectorRebuildJob> {
        validate_rebuild_error_digest(error_digest)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let current = read_rebuild_job(&tx, job_id)?;
        if current.status.is_terminal() {
            tx.commit()?;
            return Ok(current);
        }
        tx.execute(
            "DELETE FROM vector_rebuild_items WHERE job_id = ?1",
            [job_id],
        )?;
        tx.execute(
            "UPDATE vector_rebuild_jobs
             SET status = 'failed', last_error_digest = ?2, updated_at = ?3
             WHERE job_id = ?1",
            params![job_id, error_digest, chrono::Utc::now().to_rfc3339()],
        )?;
        let updated = read_rebuild_job(&tx, job_id)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn finalize_rebuild(
        &self,
        job_id: &str,
        observed_source: &VectorRebuildSourceSnapshot,
    ) -> Result<VectorRebuildJob> {
        validate_rebuild_source_snapshot(observed_source)?;
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let job = read_rebuild_job(&tx, job_id)?;
        if job.status == VectorRebuildJobStatus::CancelRequested {
            anyhow::bail!("vector rebuild cancellation requested");
        }
        if job.status.is_terminal() {
            anyhow::bail!("vector rebuild job is terminal");
        }
        if &job.source_snapshot != observed_source {
            anyhow::bail!("vector rebuild source snapshot changed");
        }
        if job.processed != job.source_snapshot.total_count
            || job.indexed.saturating_add(job.skipped) != job.processed
        {
            anyhow::bail!("vector rebuild source scan is incomplete");
        }
        let staged_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM vector_rebuild_items WHERE job_id = ?1",
            [job_id],
            |row| row.get(0),
        )?;
        if staged_count.max(0) as usize != job.processed {
            anyhow::bail!("vector rebuild checkpoint and staging rows diverged");
        }

        // This is the only promotion boundary. Embeddings were prepared in
        // bounded shadow batches; the active projection changes atomically or
        // not at all if the process/database fails.
        tx.execute("DELETE FROM vectors", [])?;
        tx.execute(
            "INSERT INTO vectors (
                session_id, content, embedding_json, embedding_blob,
                embedding_profile_id, embedding_dimension, source, created_at,
                tier, access_count, last_accessed_at, importance_score,
                archived, archived_at, summary
             )
             SELECT session_id, content, '[]', embedding_blob,
                    embedding_profile_id, embedding_dimension, source, created_at,
                    tier, access_count, last_accessed_at, importance_score,
                    archived, archived_at, summary
             FROM vector_rebuild_items
             WHERE job_id = ?1 AND outcome = 'indexed'
             ORDER BY memory_id",
            [job_id],
        )?;
        // Canonical retrieval state is independent of rebuild staging. Reapply
        // every owner fence before promotion so a rebuild cannot resurrect an
        // archived owner even when the source snapshot was prepared earlier.
        apply_all_memory_retrieval_fences(&tx)?;
        // SQLite row ids change across promotion. Rebind and validate every
        // marker before this transaction can make the rebuilt rows visible.
        rebind_and_validate_materialization_markers(&tx)?;
        tx.execute(
            "DELETE FROM vector_rebuild_items WHERE job_id = ?1",
            [job_id],
        )?;
        tx.execute(
            "UPDATE vector_rebuild_jobs
             SET status = 'completed', last_error_digest = NULL, updated_at = ?2
             WHERE job_id = ?1",
            params![job_id, chrono::Utc::now().to_rfc3339()],
        )?;
        let completed = read_rebuild_job(&tx, job_id)?;
        tx.commit()?;
        Ok(completed)
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn rebuild_staged_item_count(&self, job_id: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vector_rebuild_items WHERE job_id = ?1",
            [job_id],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Return ref-only candidates from derived access telemetry. This method
    /// never mutates retrieval truth; callers must re-prove each owner in its
    /// canonical store before creating a review action.
    pub fn low_access_canonical_memory_candidates(
        &self,
        limit: usize,
    ) -> Result<Vec<CanonicalMemoryRetrievalCandidate>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        let mut statement = conn.prepare(
            "SELECT projections.aggregate_kind, projections.aggregate_id,
                    vectors.tier, vectors.access_count, vectors.last_accessed_at,
                    vectors.importance_score
             FROM vectors
             JOIN vector_materialization_projections projections
               ON projections.vector_id = vectors.id
              AND projections.mutation_kind = 'materialized'
              AND vectors.source =
                  projections.aggregate_kind || ':' || projections.aggregate_id
             WHERE vectors.archived = 0
               AND vectors.tier >= 3
               AND (vectors.last_accessed_at IS NULL OR vectors.last_accessed_at < ?1)
               AND vectors.access_count <= 2
               AND vectors.importance_score < 0.3
               AND projections.aggregate_kind IN (
                   'memory_lifecycle', 'memory_record', 'knowledge_note'
               )
             ORDER BY vectors.importance_score ASC,
                      COALESCE(vectors.last_accessed_at, '') ASC,
                      projections.aggregate_kind,
                      projections.aggregate_id
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![&cutoff, i64::try_from(limit.clamp(1, 500)).unwrap_or(500)],
            |row| {
                Ok(CanonicalMemoryRetrievalCandidate {
                    owner_kind: row.get(0)?,
                    owner_id: row.get(1)?,
                    tier: row.get(2)?,
                    access_count: row.get(3)?,
                    last_accessed_at: row.get(4)?,
                    importance_score: row.get(5)?,
                })
            },
        )?;
        let candidates = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        for candidate in &candidates {
            CanonicalVectorOwnerRef::new(&candidate.owner_kind, &candidate.owner_id)?;
        }
        Ok(candidates)
    }

    /// Apply a canonical conversation tombstone to derived vector content.
    /// Vectors with a mechanically matching materialization marker have an
    /// independent canonical owner and therefore survive conversation deletion.
    /// A source prefix alone is never ownership evidence. Every vector owned
    /// only by the deleted conversation is removed so it cannot remain searchable.
    pub fn project_conversation_tombstone(
        &self,
        tombstone_id: &str,
        session_id: &str,
    ) -> Result<usize> {
        if tombstone_id.trim().is_empty() || session_id.trim().is_empty() {
            anyhow::bail!("invalid vector conversation tombstone projection");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let already_applied = tx
            .query_row(
                "SELECT 1 FROM vector_tombstone_projections WHERE tombstone_id = ?1",
                [tombstone_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        let deleted = tx.execute(
            "DELETE FROM vectors
             WHERE session_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM vector_materialization_projections projections
                   WHERE projections.vector_id = vectors.id
                     AND projections.mutation_kind = 'materialized'
                     AND projections.aggregate_kind IN (
                         'memory_lifecycle', 'memory_record', 'knowledge_note'
                     )
                     AND vectors.source =
                         projections.aggregate_kind || ':' || projections.aggregate_id
               )",
            [session_id],
        )?;
        if !already_applied {
            tx.execute(
                "INSERT INTO vector_tombstone_projections (
                    tombstone_id, aggregate_kind, aggregate_id, applied_at
                 ) VALUES (?1, 'conversation', ?2, ?3)",
                params![tombstone_id, session_id, chrono::Utc::now().to_rfc3339()],
            )?;
        }
        tx.commit()?;
        Ok(deleted)
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn install_tombstone_projection_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch(
            "CREATE TRIGGER fail_vector_tombstone_projection_for_test
             BEFORE DELETE ON vectors
             BEGIN
                 SELECT RAISE(ABORT, 'injected vector tombstone projection failure');
             END;",
        )?;
        Ok(())
    }

    #[cfg(any(test, feature = "test-utils"))]
    #[doc(hidden)]
    pub fn remove_tombstone_projection_failure_for_test(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute_batch("DROP TRIGGER IF EXISTS fail_vector_tombstone_projection_for_test;")?;
        Ok(())
    }

    /// Update importance score for a chunk (e.g. after user feedback).
    pub fn set_importance(&self, chunk_id: i64, score: f32) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE vectors SET importance_score = ?1 WHERE id = ?2",
            params![score.clamp(0.0, 1.0), chunk_id],
        )?;
        Ok(())
    }

    /// Get tier statistics for diagnostics.
    pub fn tier_stats(&self) -> Result<TierStats> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE archived = 0",
            [],
            |row| row.get(0),
        )?;
        let tier1: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE archived = 0 AND tier = 1",
            [],
            |row| row.get(0),
        )?;
        let tier2: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE archived = 0 AND tier = 2",
            [],
            |row| row.get(0),
        )?;
        let tier3: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE archived = 0 AND tier = 3",
            [],
            |row| row.get(0),
        )?;
        let archived: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vectors WHERE archived = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(TierStats {
            total,
            tier1,
            tier2,
            tier3,
            archived,
        })
    }
}

fn read_rebuild_job(conn: &Connection, job_id: &str) -> Result<VectorRebuildJob> {
    conn.query_row(
        "SELECT job_id, status, source_through_memory_id, source_total_count,
                source_metadata_digest, last_processed_memory_id, processed_count,
                indexed_count, skipped_count, embedding_profile_id,
                embedding_dimension, provider_invocations, cache_hits,
                remote_unknown_provider_attempts, last_error_digest,
                created_at, updated_at
         FROM vector_rebuild_jobs WHERE job_id = ?1",
        [job_id],
        |row| {
            let stored_status: String = row.get(1)?;
            let status = VectorRebuildJobStatus::from_stored(&stored_status).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    error.into(),
                )
            })?;
            Ok(VectorRebuildJob {
                job_id: row.get(0)?,
                status,
                source_snapshot: VectorRebuildSourceSnapshot {
                    through_memory_id: row.get(2)?,
                    total_count: row.get::<_, i64>(3)?.max(0) as usize,
                    metadata_digest: row.get(4)?,
                },
                last_processed_memory_id: row.get(5)?,
                processed: row.get::<_, i64>(6)?.max(0) as usize,
                indexed: row.get::<_, i64>(7)?.max(0) as usize,
                skipped: row.get::<_, i64>(8)?.max(0) as usize,
                embedding_profile_id: row.get(9)?,
                embedding_dimension: row
                    .get::<_, Option<i64>>(10)?
                    .map(|value| value.max(0) as usize),
                provider_invocations: row.get::<_, i64>(11)?.max(0) as usize,
                cache_hits: row.get::<_, i64>(12)?.max(0) as usize,
                remote_unknown_provider_attempts: row.get::<_, i64>(13)?.max(0) as usize,
                last_error_digest: row.get(14)?,
                created_at: row.get(15)?,
                updated_at: row.get(16)?,
            })
        },
    )
    .context("failed to load vector rebuild job")
}

fn validate_rebuild_source_snapshot(snapshot: &VectorRebuildSourceSnapshot) -> Result<()> {
    if snapshot.through_memory_id < 0
        || snapshot.metadata_digest.trim().is_empty()
        || snapshot.metadata_digest.len() > 128
    {
        anyhow::bail!("invalid vector rebuild source snapshot");
    }
    Ok(())
}

fn validate_rebuild_error_digest(error_digest: &str) -> Result<()> {
    if !error_digest.starts_with("sha256:") || error_digest.len() != 71 {
        anyhow::bail!("vector rebuild errors must be metadata-only SHA-256 digests");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalMemoryRetrievalCandidate {
    pub owner_kind: String,
    pub owner_id: String,
    pub tier: i64,
    pub access_count: i64,
    pub last_accessed_at: Option<String>,
    pub importance_score: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TierStats {
    pub total: i64,
    pub tier1: i64,
    pub tier2: i64,
    pub tier3: i64,
    pub archived: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportedVectorChunk {
    pub session_id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    #[serde(default = "unknown_embedding_profile_id")]
    pub embedding_profile_id: String,
    #[serde(default)]
    pub embedding_dimension: usize,
    pub source: String,
    pub created_at: String,
    pub tier: i64,
    pub access_count: i64,
    pub last_accessed_at: String,
    pub importance_score: f32,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub summary: Option<String>,
}

/// Mechanical accounting for the portable subset of a vector archive.
///
/// Canonical Memory vectors and conversation-token vectors are projections of
/// other canonical owners. They may appear in older backups, but replaying
/// them through the generic vector import path would create a second truth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableVectorImportReport {
    pub supplied: usize,
    pub applied: usize,
    pub skipped_canonical_projection: usize,
    pub skipped_legacy_chat_projection: usize,
}

impl PortableVectorImportReport {
    pub fn skipped(self) -> usize {
        self.skipped_canonical_projection
            .saturating_add(self.skipped_legacy_chat_projection)
    }
}

fn unknown_embedding_profile_id() -> String {
    UNKNOWN_EMBEDDING_PROFILE_ID.into()
}

fn validate_exported_chunk_profile(chunk: &ExportedVectorChunk) -> Result<()> {
    if chunk.embedding_profile_id == UNKNOWN_EMBEDDING_PROFILE_ID {
        if chunk.embedding_dimension != 0 {
            anyhow::bail!("legacy_unknown_embedding_profile_dimension_must_be_zero");
        }
        return Ok(());
    }
    if chunk.embedding_dimension == 0
        || chunk.embedding_dimension > MAX_EMBEDDING_DIMENSION
        || chunk.embedding_dimension != chunk.embedding.len()
    {
        anyhow::bail!("exported_embedding_profile_dimension_mismatch");
    }
    validate_embedding_vector(&chunk.embedding, Some(chunk.embedding_dimension))?;
    Ok(())
}

fn vector_session_tombstoned(conn: &Connection, session_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM vector_tombstone_projections
             WHERE aggregate_kind = 'conversation' AND aggregate_id = ?1 LIMIT 1",
            [session_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn validate_projection_ref(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > 256
        || value.chars().any(char::is_control)
    {
        anyhow::bail!("invalid vector projection {label}");
    }
    Ok(())
}

fn is_reserved_canonical_vector_source(source: &str) -> bool {
    CANONICAL_VECTOR_OWNER_KINDS
        .iter()
        .any(|kind| source.starts_with(&format!("{kind}:")))
}

fn canonical_owner_from_source(source: &str) -> Result<Option<CanonicalVectorOwnerRef>> {
    for kind in CANONICAL_VECTOR_OWNER_KINDS {
        let prefix = format!("{kind}:");
        if let Some(id) = source.strip_prefix(&prefix) {
            return CanonicalVectorOwnerRef::new(kind, id).map(Some);
        }
    }
    Ok(None)
}

fn rebind_and_validate_materialization_markers(conn: &Connection) -> Result<()> {
    let duplicate_source = conn
        .query_row(
            "SELECT source FROM vectors
             WHERE source GLOB 'memory_lifecycle:*'
                OR source GLOB 'memory_record:*'
                OR source GLOB 'knowledge_note:*'
             GROUP BY source HAVING COUNT(*) > 1 LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if duplicate_source.is_some() {
        anyhow::bail!("duplicate canonical vector owner source");
    }

    let mut sources = conn.prepare(
        "SELECT source FROM vectors
         WHERE source GLOB 'memory_lifecycle:*'
            OR source GLOB 'memory_record:*'
            OR source GLOB 'knowledge_note:*'",
    )?;
    for source in sources.query_map([], |row| row.get::<_, String>(0))? {
        let source = source?;
        canonical_owner_from_source(&source)?
            .ok_or_else(|| anyhow::anyhow!("invalid canonical vector owner source"))?;
    }
    drop(sources);

    conn.execute(
        "UPDATE vector_materialization_projections
         SET vector_id = (
             SELECT vectors.id
             FROM vectors
             WHERE vectors.source =
                   vector_materialization_projections.aggregate_kind || ':' ||
                   vector_materialization_projections.aggregate_id
         )
         WHERE mutation_kind = 'materialized'",
        [],
    )?;

    let mut markers = conn.prepare(
        "SELECT projections.event_id, projections.aggregate_kind,
                projections.aggregate_id, projections.vector_id, vectors.source
         FROM vector_materialization_projections projections
         LEFT JOIN vectors ON vectors.id = projections.vector_id
         WHERE projections.mutation_kind = 'materialized'",
    )?;
    let rows = markers.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        let (event_id, kind, id, vector_id, source) = row?;
        let owner = CanonicalVectorOwnerRef::new(&kind, &id)?;
        match (vector_id, source) {
            (Some(_), Some(source)) if source == owner.source() => {}
            (Some(_), Some(_)) => anyhow::bail!(
                "vector materialization marker source mismatch after replacement: {event_id}"
            ),
            (Some(_), None) => anyhow::bail!(
                "vector materialization marker dangling after replacement: {event_id}"
            ),
            (None, None)
                if owner.kind() == "memory_lifecycle"
                    && memory_lifecycle_vector_projection_deleted(conn, owner.id())? => {}
            (None, _) => {
                anyhow::bail!("canonical vector missing after replacement for marker: {event_id}")
            }
        }
    }
    Ok(())
}

fn projected_vector_id(
    conn: &Connection,
    event_id: &str,
    expected_owner: &CanonicalVectorOwnerRef,
) -> Result<Option<Option<i64>>> {
    let marker = conn
        .query_row(
            "SELECT projections.aggregate_kind, projections.aggregate_id,
                    projections.mutation_kind, projections.vector_id, vectors.source
             FROM vector_materialization_projections projections
             LEFT JOIN vectors ON vectors.id = projections.vector_id
             WHERE projections.event_id = ?1",
            [event_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((kind, id, mutation_kind, vector_id, source)) = marker else {
        return Ok(None);
    };
    if kind != expected_owner.kind() || id != expected_owner.id() || mutation_kind != "materialized"
    {
        anyhow::bail!("vector projection marker owner identity mismatch");
    }
    match (vector_id, source) {
        (Some(vector_id), Some(source)) if source == expected_owner.source() => {
            Ok(Some(Some(vector_id)))
        }
        (Some(_), Some(_)) => anyhow::bail!("vector projection marker source mismatch"),
        (Some(_), None) => anyhow::bail!("vector projection marker is dangling"),
        (None, None)
            if expected_owner.kind() == "memory_lifecycle"
                && memory_lifecycle_vector_projection_deleted(conn, expected_owner.id())? =>
        {
            Ok(Some(None))
        }
        (None, _) => anyhow::bail!("materialized vector projection has no vector"),
    }
}

fn memory_lifecycle_vector_projection_deleted(conn: &Connection, memory_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM vector_materialization_projections
             WHERE aggregate_kind = 'memory_lifecycle'
               AND aggregate_id = ?1 AND mutation_kind = 'deleted'
             LIMIT 1",
            [memory_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn vector_projection_applied(conn: &Connection, event_id: &str) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM vector_materialization_projections WHERE event_id = ?1",
            [event_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn insert_vector_projection_marker(
    conn: &Connection,
    event_id: &str,
    aggregate_kind: &str,
    aggregate_id: &str,
    mutation_kind: &str,
    vector_id: Option<i64>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO vector_materialization_projections (
            event_id, aggregate_kind, aggregate_id, mutation_kind,
            vector_id, applied_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event_id,
            aggregate_kind,
            aggregate_id,
            mutation_kind,
            vector_id,
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

fn apply_memory_retrieval_fence_to_owner(
    conn: &Connection,
    owner: &CanonicalVectorOwnerRef,
) -> Result<usize> {
    let disposition = conn
        .query_row(
            "SELECT disposition
             FROM vector_memory_retrieval_projections
             WHERE owner_kind = ?1 AND owner_id = ?2",
            params![owner.kind(), owner.id()],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match disposition.as_deref() {
        Some("archived") => conn
            .execute(
                "UPDATE vectors
                 SET archived = 1,
                     archived_at = COALESCE(archived_at, ?2),
                     summary = NULL
                 WHERE source = ?1",
                params![owner.source(), chrono::Utc::now().to_rfc3339()],
            )
            .map_err(Into::into),
        Some("active") => conn
            .execute(
                "UPDATE vectors
                 SET archived = 0, archived_at = NULL, summary = NULL
                 WHERE source = ?1",
                [owner.source()],
            )
            .map_err(Into::into),
        Some(_) => anyhow::bail!("unsupported vector Memory retrieval disposition"),
        None => Ok(0),
    }
}

fn apply_all_memory_retrieval_fences(conn: &Connection) -> Result<()> {
    let owners = {
        let mut statement = conn.prepare(
            "SELECT owner_kind, owner_id
             FROM vector_memory_retrieval_projections
             ORDER BY owner_kind, owner_id",
        )?;
        let owners = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        owners
    };
    for (kind, id) in owners {
        let owner = CanonicalVectorOwnerRef::new(&kind, &id)?;
        apply_memory_retrieval_fence_to_owner(conn, &owner)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortableVectorDisposition {
    Portable,
    CanonicalProjection,
    LegacyChatProjection,
}

fn portable_vector_disposition(source: &str) -> PortableVectorDisposition {
    if is_reserved_canonical_vector_source(source) {
        PortableVectorDisposition::CanonicalProjection
    } else if is_legacy_chat_vector_source(source) {
        PortableVectorDisposition::LegacyChatProjection
    } else {
        PortableVectorDisposition::Portable
    }
}

fn validate_portable_vector_chunks(
    conn: &Connection,
    chunks: &[ExportedVectorChunk],
) -> Result<PortableVectorImportReport> {
    let mut report = PortableVectorImportReport {
        supplied: chunks.len(),
        ..PortableVectorImportReport::default()
    };
    for chunk in chunks {
        match portable_vector_disposition(&chunk.source) {
            PortableVectorDisposition::CanonicalProjection => {
                report.skipped_canonical_projection =
                    report.skipped_canonical_projection.saturating_add(1);
            }
            PortableVectorDisposition::LegacyChatProjection => {
                report.skipped_legacy_chat_projection =
                    report.skipped_legacy_chat_projection.saturating_add(1);
            }
            PortableVectorDisposition::Portable => {
                validate_exported_chunk_profile(chunk)?;
                if vector_session_tombstoned(conn, &chunk.session_id)? {
                    anyhow::bail!("vector replacement targets a tombstoned conversation session");
                }
                report.applied = report.applied.saturating_add(1);
            }
        }
    }
    debug_assert_eq!(report.supplied, report.applied + report.skipped());
    Ok(report)
}

fn is_legacy_chat_vector_source(source: &str) -> bool {
    matches!(source, "user_message" | "assistant_reply")
}

/// Cosine similarity with manual 4-wide vectorization.
/// Processes 4 f32 values per iteration for better cache and instruction throughput.
fn encode_embedding_blob(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode_embedding(blob: Option<&[u8]>, legacy_json: &str) -> Vec<f32> {
    if let Some(blob) = blob {
        if blob.is_empty() || blob.len() % 4 != 0 {
            // A present-but-malformed canonical blob is corruption. Falling back
            // to stale compatibility JSON would hide the damaged canonical value.
            return Vec::new();
        }
        return blob
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
    }
    serde_json::from_str(legacy_json).unwrap_or_default()
}

fn validate_embedding_profile(embedding: &[f32], profile: &EmbeddingProfile) -> Result<()> {
    profile.validate_known_identity()?;
    validate_embedding_vector(embedding, Some(profile.dimension)).with_context(|| {
        format!(
            "embedding_profile_vector_invalid profile={} expected={} actual={}",
            profile.id,
            profile.dimension,
            embedding.len()
        )
    })?;
    Ok(())
}

const MAX_INCOMPATIBLE_PROFILE_EVIDENCE: usize = 32;

fn profile_compatibility_evidence(
    conn: &Connection,
    expected: &EmbeddingProfile,
    session_id: Option<&str>,
) -> Result<Option<VectorRebuildEvidence>> {
    let mut evidence = empty_rebuild_evidence(expected);
    if let Some(session_id) = session_id {
        let mut statement = conn.prepare(
            "SELECT embedding_profile_id, embedding_dimension, row_count,
                    malformed_blob_count
             FROM vector_profile_stats
             WHERE scope_kind = 'session' AND scope_id = ?1 AND archived = 0",
        )?;
        let rows = statement.query_map([session_id], profile_group_row)?;
        accumulate_profile_evidence(rows, expected, &mut evidence)?;
    } else {
        let mut statement = conn.prepare(
            "SELECT embedding_profile_id, embedding_dimension, row_count,
                    malformed_blob_count
             FROM vector_profile_stats
             WHERE scope_kind = 'global' AND scope_id = '' AND archived = 0",
        )?;
        let rows = statement.query_map([], profile_group_row)?;
        accumulate_profile_evidence(rows, expected, &mut evidence)?;
    }
    let required = evidence.unknown_profile_count > 0
        || evidence.profile_mismatch_count > 0
        || evidence.dimension_mismatch_count > 0
        || evidence.corrupt_embedding_count > 0;
    Ok(required.then_some(evidence))
}

fn profile_group_row(row: &rusqlite::Row) -> rusqlite::Result<(String, usize, usize, usize)> {
    Ok((
        row.get(0)?,
        row.get::<_, i64>(1)?.max(0) as usize,
        row.get::<_, i64>(2)?.max(0) as usize,
        row.get::<_, i64>(3)?.max(0) as usize,
    ))
}

fn accumulate_profile_evidence<I>(
    rows: I,
    expected: &EmbeddingProfile,
    evidence: &mut VectorRebuildEvidence,
) -> Result<()>
where
    I: IntoIterator<Item = rusqlite::Result<(String, usize, usize, usize)>>,
{
    for row in rows {
        let (profile_id, dimension, count, malformed_blob_count) = row?;
        let unknown = profile_id == UNKNOWN_EMBEDDING_PROFILE_ID || dimension == 0;
        let profile_mismatch = !unknown && profile_id != expected.id;
        let dimension_mismatch = !unknown && dimension != expected.dimension;
        if unknown {
            evidence.unknown_profile_count = evidence.unknown_profile_count.saturating_add(count);
        }
        if profile_mismatch {
            evidence.profile_mismatch_count = evidence.profile_mismatch_count.saturating_add(count);
        }
        if dimension_mismatch {
            evidence.dimension_mismatch_count =
                evidence.dimension_mismatch_count.saturating_add(count);
        }
        // Matching rows are decoded below, where valid legacy JSON remains usable
        // and malformed/non-finite values are counted without double counting.
        if profile_mismatch || dimension_mismatch || unknown {
            evidence.corrupt_embedding_count = evidence
                .corrupt_embedding_count
                .saturating_add(malformed_blob_count);
        }
        if unknown || profile_mismatch || dimension_mismatch || malformed_blob_count > 0 {
            push_bounded_profile(
                &mut evidence.incompatible_profiles,
                format!("{profile_id}:dim:{dimension}"),
            );
        }
    }
    Ok(())
}

fn empty_rebuild_evidence(expected: &EmbeddingProfile) -> VectorRebuildEvidence {
    VectorRebuildEvidence {
        expected_profile_id: expected.id.clone(),
        expected_dimension: expected.dimension,
        incompatible_profiles: Vec::new(),
        unknown_profile_count: 0,
        profile_mismatch_count: 0,
        dimension_mismatch_count: 0,
        corrupt_embedding_count: 0,
    }
}

fn push_bounded_profile(profiles: &mut Vec<String>, value: String) {
    if profiles.len() < MAX_INCOMPATIBLE_PROFILE_EVIDENCE && !profiles.contains(&value) {
        profiles.push(value);
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty()
        || a.len() != b.len()
        || a.iter().any(|value| !value.is_finite())
        || b.iter().any(|value| !value.is_finite())
    {
        return 0.0;
    }
    let len = a.len();
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;

    // Process 4 elements at a time
    let chunks = len / 4;
    for i in 0..chunks {
        let idx = i * 4;
        let ax = [
            f64::from(a[idx]),
            f64::from(a[idx + 1]),
            f64::from(a[idx + 2]),
            f64::from(a[idx + 3]),
        ];
        let bx = [
            f64::from(b[idx]),
            f64::from(b[idx + 1]),
            f64::from(b[idx + 2]),
            f64::from(b[idx + 3]),
        ];
        dot += ax[0] * bx[0] + ax[1] * bx[1] + ax[2] * bx[2] + ax[3] * bx[3];
        norm_a += ax[0] * ax[0] + ax[1] * ax[1] + ax[2] * ax[2] + ax[3] * ax[3];
        norm_b += bx[0] * bx[0] + bx[1] * bx[1] + bx[2] * bx[2] + bx[3] * bx[3];
    }

    // Process remaining elements
    for i in chunks * 4..len {
        let x = f64::from(a[i]);
        let y = f64::from(b[i]);
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    let score = dot / (norm_a.sqrt() * norm_b.sqrt());
    if score.is_finite() {
        score.clamp(-1.0, 1.0) as f32
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingSensitiveTopic {
    Health,
    Finance,
    Identity,
    Relationship,
    PrivateFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingPrivacyPlan {
    pub embedding_text: String,
    pub cloud_allowed: bool,
    pub hs_local_only: bool,
    pub detected_privacy_types: Vec<String>,
    pub sensitive_topic: Option<EmbeddingSensitiveTopic>,
    pub blocking_reasons: Vec<String>,
}

pub fn plan_embedding_privacy(
    text: &str,
    privacy_engine: &crate::privacy::PrivacyEngine,
    hs_local_only: bool,
) -> EmbeddingPrivacyPlan {
    let findings = privacy_engine.detect(text);
    let (embedding_text, _) = privacy_engine.desensitize(text);
    let mut detected_privacy_types: Vec<String> = findings
        .iter()
        .map(|(ptype, _)| ptype.placeholder_prefix().to_string())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    detected_privacy_types.sort();

    let sensitive_topic = classify_embedding_sensitive_topic(text, &findings);
    let mut blocking_reasons = Vec::new();
    if hs_local_only {
        blocking_reasons.push("hs_local_only".to_string());
    }
    if !findings.is_empty() {
        blocking_reasons.push("pii_detected".to_string());
    }
    if let Some(topic) = sensitive_topic {
        blocking_reasons.push(format!("sensitive_topic:{:?}", topic).to_lowercase());
    }

    EmbeddingPrivacyPlan {
        embedding_text,
        cloud_allowed: blocking_reasons.is_empty(),
        hs_local_only,
        detected_privacy_types,
        sensitive_topic,
        blocking_reasons,
    }
}

fn classify_embedding_sensitive_topic(
    text: &str,
    findings: &[(crate::privacy::PrivacyType, String)],
) -> Option<EmbeddingSensitiveTopic> {
    if findings
        .iter()
        .any(|(ptype, _)| matches!(ptype, crate::privacy::PrivacyType::IdCard))
    {
        return Some(EmbeddingSensitiveTopic::Identity);
    }
    if findings
        .iter()
        .any(|(ptype, _)| matches!(ptype, crate::privacy::PrivacyType::BankCard))
    {
        return Some(EmbeddingSensitiveTopic::Finance);
    }
    if findings.iter().any(|(ptype, _)| {
        matches!(
            ptype,
            crate::privacy::PrivacyType::Email
                | crate::privacy::PrivacyType::Phone
                | crate::privacy::PrivacyType::Address
                | crate::privacy::PrivacyType::Name
                | crate::privacy::PrivacyType::Generic
        )
    }) {
        return Some(EmbeddingSensitiveTopic::PrivateFile);
    }

    let lower = text.to_lowercase();
    if contains_any(
        &lower,
        &[
            "health",
            "medical",
            "medicine",
            "medication",
            "prescription",
            "doctor",
            "therapy",
            "mental",
            "illness",
            "diagnosis",
            "anxiety",
            "depression",
            "drug",
            "药",
            "用药",
            "处方",
            "病",
            "医院",
            "健康",
            "心理",
            "焦虑",
            "抑郁",
            "诊断",
            "治疗",
        ],
    ) {
        Some(EmbeddingSensitiveTopic::Health)
    } else if contains_any(
        &lower,
        &[
            "finance",
            "bank",
            "salary",
            "income",
            "insurance",
            "debt",
            "loan",
            "tax",
            "credit",
            "mortgage",
            "投资",
            "银行",
            "工资",
            "收入",
            "保险",
            "债务",
            "负债",
            "贷款",
            "税",
            "信用卡",
        ],
    ) {
        Some(EmbeddingSensitiveTopic::Finance)
    } else if contains_any(
        &lower,
        &[
            "identity",
            "identity card",
            "id card",
            "passport",
            "ssn",
            "values",
            "mission",
            "身份",
            "身份证",
            "护照",
            "证件",
            "价值观",
            "使命",
        ],
    ) {
        Some(EmbeddingSensitiveTopic::Identity)
    } else if contains_any(
        &lower,
        &[
            "relationship",
            "intimate relationship",
            "partner",
            "family",
            "breakup",
            "break up",
            "divorce",
            "family conflict",
            "关系",
            "亲密关系",
            "伴侣",
            "家人",
            "分手",
            "家庭矛盾",
            "家庭冲突",
            "婚姻",
            "离婚",
            "恋爱",
        ],
    ) {
        Some(EmbeddingSensitiveTopic::Relationship)
    } else if contains_any(
        &lower,
        &[
            "private file",
            "privacy",
            "private",
            "secret",
            "confidential",
            "contract",
            "resume",
            "cv",
            "私人文件",
            "隐私",
            "机密",
            "合同",
            "简历",
        ],
    ) {
        Some(EmbeddingSensitiveTopic::PrivateFile)
    } else {
        None
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::EmbeddingRouteKind;

    fn dummy_embedding(len: usize) -> Vec<f32> {
        (0..len).map(|i| i as f32 * 0.01).collect()
    }

    fn test_profile(len: usize) -> EmbeddingProfile {
        EmbeddingProfile::new(
            EmbeddingRouteKind::DeterministicHash,
            "openlife-test",
            "deterministic-test-v1",
            "builtin:test",
            "deterministic-test-artifact-v1",
            len,
        )
        .unwrap()
    }

    fn test_owner(kind: &str, id: &str) -> CanonicalVectorOwnerRef {
        CanonicalVectorOwnerRef::new(kind, id).unwrap()
    }

    fn expect_matches(outcome: VectorSearchOutcome) -> Vec<(MemoryChunk, f32)> {
        match outcome {
            VectorSearchOutcome::Matches { matches, .. } => matches,
            VectorSearchOutcome::RebuildRequired(_) => {
                panic!("test vectors unexpectedly require rebuild")
            }
        }
    }

    #[test]
    fn vector_store_insert_and_count() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb = dummy_embedding(4);
        let profile = test_profile(emb.len());
        let id = store.insert("s1", "hello", &emb, &profile, "chat").unwrap();
        assert!(id > 0);
        assert_eq!(store.count_all_chunks().unwrap(), 1);
        let conn = store.conn.lock().unwrap();
        let (json, blob_len): (String, i64) = conn
            .query_row(
                "SELECT embedding_json, length(embedding_blob) FROM vectors WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(json, "[]");
        assert_eq!(blob_len, (emb.len() * std::mem::size_of::<f32>()) as i64);
    }

    #[test]
    fn vector_store_migrates_legacy_schema_before_indexing() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("vectors.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "CREATE TABLE vectors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    content TEXT NOT NULL,
                    embedding_json TEXT NOT NULL,
                    source TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )",
                [],
            )
            .unwrap();
        }

        let store = VectorStore::new(&db_path).unwrap();
        let conn = store.conn.lock().unwrap();
        let archived_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vectors') WHERE name = 'archived'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let embedding_blob_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('vectors') WHERE name = 'embedding_blob'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let index_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_vectors_archived'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(archived_count, 1);
        assert_eq!(embedding_blob_count, 1);
        assert_eq!(index_count, 1);
    }

    #[test]
    fn vector_profile_searches_use_composite_metadata_indexes() {
        let store = VectorStore::new_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();
        let global_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT id, session_id, content, embedding_json, embedding_blob, source, created_at,
                        tier, access_count, last_accessed_at, importance_score, archived, archived_at,
                        summary, embedding_profile_id, embedding_dimension
                 FROM vectors
                 WHERE archived = 0 AND embedding_profile_id = 'profile' AND embedding_dimension = 4
                 ORDER BY created_at DESC LIMIT 2000",
                [],
                |row| row.get(3),
            )
            .unwrap();
        let session_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT id, session_id, content, embedding_json, embedding_blob, source, created_at,
                        tier, access_count, last_accessed_at, importance_score, archived, archived_at,
                        summary, embedding_profile_id, embedding_dimension
                 FROM vectors
                 WHERE session_id = 's1' AND archived = 0 AND embedding_profile_id = 'profile'
                       AND embedding_dimension = 4
                 ORDER BY created_at DESC LIMIT 1000",
                [],
                |row| row.get(3),
            )
            .unwrap();

        assert!(
            global_plan.contains("idx_vectors_profile_search"),
            "{global_plan}"
        );
        assert!(
            session_plan.contains("idx_vectors_session_profile_search"),
            "{session_plan}"
        );
    }

    #[test]
    fn compatibility_lookup_uses_profile_summary_instead_of_scanning_vectors() {
        let store = VectorStore::new_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();
        let global_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT embedding_profile_id, embedding_dimension, row_count,
                        malformed_blob_count
                 FROM vector_profile_stats
                 WHERE scope_kind = 'global' AND scope_id = '' AND archived = 0",
                [],
                |row| row.get(3),
            )
            .unwrap();
        let session_plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN
                 SELECT embedding_profile_id, embedding_dimension, row_count,
                        malformed_blob_count
                 FROM vector_profile_stats
                 WHERE scope_kind = 'session' AND scope_id = 's1' AND archived = 0",
                [],
                |row| row.get(3),
            )
            .unwrap();

        assert!(
            global_plan.contains("vector_profile_stats"),
            "{global_plan}"
        );
        assert!(
            session_plan.contains("vector_profile_stats"),
            "{session_plan}"
        );
        assert!(!global_plan.contains("SCAN vectors"), "{global_plan}");
        assert!(!session_plan.contains("SCAN vectors"), "{session_plan}");
    }

    #[test]
    fn vector_profile_summary_tracks_insert_archive_and_delete_transactionally() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let first = store
            .insert("s1", "first", &[1.0, 0.0, 0.0, 0.0], &profile, "note")
            .unwrap();
        store
            .insert("s2", "second", &[0.0, 1.0, 0.0, 0.0], &profile, "note")
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            let global_active: i64 = conn
                .query_row(
                    "SELECT row_count FROM vector_profile_stats
                     WHERE scope_kind = 'global' AND scope_id = '' AND archived = 0
                       AND embedding_profile_id = ?1 AND embedding_dimension = 4",
                    [&profile.id],
                    |row| row.get(0),
                )
                .unwrap();
            let session_active: i64 = conn
                .query_row(
                    "SELECT row_count FROM vector_profile_stats
                     WHERE scope_kind = 'session' AND scope_id = 's1' AND archived = 0
                       AND embedding_profile_id = ?1 AND embedding_dimension = 4",
                    [&profile.id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(global_active, 2);
            assert_eq!(session_active, 1);
        }

        assert_eq!(
            store
                .conn
                .lock()
                .unwrap()
                .execute("UPDATE vectors SET archived = 1 WHERE id = ?1", [first])
                .unwrap(),
            1
        );
        {
            let conn = store.conn.lock().unwrap();
            let (active, archived): (i64, i64) = conn
                .query_row(
                    "SELECT
                        COALESCE(SUM(CASE WHEN archived = 0 THEN row_count ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN archived = 1 THEN row_count ELSE 0 END), 0)
                     FROM vector_profile_stats
                     WHERE scope_kind = 'global' AND scope_id = ''
                       AND embedding_profile_id = ?1 AND embedding_dimension = 4",
                    [&profile.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!((active, archived), (1, 1));
        }

        store.clear_all_chunks().unwrap();
        let conn = store.conn.lock().unwrap();
        let summary_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM vector_profile_stats", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(summary_rows, 0);
    }

    #[test]
    fn session_search_candidate_limit_is_hard_capped() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let embedding = [1.0, 0.0, 0.0, 0.0];
        let contents = (0..(MAX_VECTOR_SEARCH_CANDIDATES + 25))
            .map(|index| format!("item-{index}"))
            .collect::<Vec<_>>();
        let items = contents
            .iter()
            .map(|content| VectorInsertItem {
                session_id: "bounded-session",
                content,
                embedding: &embedding,
                profile: &profile,
                source: "note",
            })
            .collect::<Vec<_>>();
        store.insert_batch(&items).unwrap();

        let matches = expect_matches(
            store
                .search_by_session(
                    "bounded-session",
                    &embedding,
                    &profile,
                    usize::MAX,
                    usize::MAX,
                )
                .unwrap(),
        );
        assert_eq!(matches.len(), MAX_VECTOR_SEARCH_CANDIDATES);
    }

    #[test]
    fn twenty_thousand_vector_search_meets_the_bounded_candidate_gate() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let embedding = [1.0, 0.0, 0.0, 0.0];
        let contents = (0..20_000)
            .map(|index| format!("benchmark-item-{index}"))
            .collect::<Vec<_>>();
        let items = contents
            .iter()
            .map(|content| VectorInsertItem {
                session_id: "benchmark-session",
                content,
                embedding: &embedding,
                profile: &profile,
                source: "benchmark",
            })
            .collect::<Vec<_>>();
        store.insert_batch(&items).unwrap();

        let started = std::time::Instant::now();
        let matches = expect_matches(store.search(&embedding, &profile, usize::MAX).unwrap());
        let elapsed = started.elapsed();

        assert_eq!(matches.len(), MAX_VECTOR_SEARCH_CANDIDATES);
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "20,000-vector bounded search exceeded the frozen 5s local debug gate: {elapsed:?}"
        );
        let conn = store.conn.lock().unwrap();
        let summary_count: i64 = conn
            .query_row(
                "SELECT row_count FROM vector_profile_stats
                 WHERE scope_kind = 'global' AND scope_id = '' AND archived = 0
                   AND embedding_profile_id = ?1 AND embedding_dimension = 4",
                [&profile.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(summary_count, 20_000);
    }

    #[test]
    fn vector_store_migration_removes_legacy_chat_body_copies_only() {
        const CHAT_BODY: &str = "THERAPY-CASE-74291-ORCHID";
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("legacy-chat-vectors.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE vectors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    content TEXT NOT NULL,
                    embedding_json TEXT NOT NULL,
                    source TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at)
                 VALUES ('s1', ?1, '[0.1,0.2]', 'user_message', ?2)",
                params![CHAT_BODY, chrono::Utc::now().to_rfc3339()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at)
                 VALUES ('s1', 'accepted preference', '[0.1,0.2]', 'memory_lifecycle:m1', ?1)",
                [chrono::Utc::now().to_rfc3339()],
            )
            .unwrap();
        }

        let store = VectorStore::new(&db_path).unwrap();
        let exported = store.export_all_chunks().unwrap();

        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].source, "memory_lifecycle:m1");
        assert!(!serde_json::to_string(&exported)
            .unwrap()
            .contains(CHAT_BODY));
    }

    #[test]
    fn vector_store_insert_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb_a = dummy_embedding(4);
        let emb_b = dummy_embedding(4);
        let profile = test_profile(4);
        let items = vec![
            VectorInsertItem {
                session_id: "s1",
                content: "a",
                embedding: &emb_a,
                profile: &profile,
                source: "chat",
            },
            VectorInsertItem {
                session_id: "s1",
                content: "b",
                embedding: &emb_b,
                profile: &profile,
                source: "chat",
            },
        ];
        assert_eq!(store.insert_batch(&items).unwrap(), 2);
        assert_eq!(store.count_all_chunks().unwrap(), 2);
    }

    #[test]
    fn vector_store_search_by_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb = dummy_embedding(4);
        let profile = test_profile(emb.len());
        store
            .insert("s1", "hello world", &emb, &profile, "chat")
            .unwrap();
        store
            .insert("s2", "other session", &emb, &profile, "chat")
            .unwrap();
        let results = expect_matches(
            store
                .search_by_session("s1", &emb, &profile, 5, 100)
                .unwrap(),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.session_id, "s1");
    }

    #[test]
    fn vector_store_search_global() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb_a = vec![1.0f32, 0.0, 0.0, 0.0];
        let emb_b = vec![0.0f32, 1.0, 0.0, 0.0];
        let profile = test_profile(emb_a.len());
        store
            .insert("s1", "alpha", &emb_a, &profile, "chat")
            .unwrap();
        store
            .insert("s1", "beta", &emb_b, &profile, "chat")
            .unwrap();
        let results = expect_matches(store.search(&emb_a, &profile, 2).unwrap());
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.content, "alpha");
    }

    #[test]
    fn vector_store_tier_maintenance() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let profile = test_profile(4);
        store
            .insert("s1", "old", &dummy_embedding(4), &profile, "chat")
            .unwrap();
        // Manually set last_accessed_at far in the past and low importance so demotion triggers
        {
            let conn = store.conn.lock().unwrap();
            let old = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
            conn.execute(
                "UPDATE vectors SET last_accessed_at = ?1, importance_score = 0.1",
                params![&old],
            )
            .unwrap();
        }
        let (promoted, demoted) = store.run_tier_maintenance().unwrap();
        assert_eq!(promoted, 0);
        assert_eq!(demoted, 1);
    }

    #[test]
    fn low_access_metrics_are_candidates_and_canonical_projection_controls_retrieval() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let profile = test_profile(4);
        let owner = test_owner("knowledge_note", "low-access-owner");
        store
            .project_memory_embedding(
                "outbox:low-access-owner",
                &owner,
                "s1",
                "forgettable",
                &dummy_embedding(4),
                &profile,
            )
            .unwrap();
        // Set old access date and low importance
        {
            let conn = store.conn.lock().unwrap();
            let old = (chrono::Utc::now() - chrono::Duration::days(120)).to_rfc3339();
            conn.execute("UPDATE vectors SET last_accessed_at = ?1, access_count = 1, importance_score = 0.1, tier = 3", params![&old]).unwrap();
        }
        let candidates = store.low_access_canonical_memory_candidates(10).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].owner_kind, owner.kind());
        assert_eq!(candidates[0].owner_id, owner.id());
        assert_eq!(
            expect_matches(store.search(&dummy_embedding(4), &profile, 5).unwrap()).len(),
            1
        );

        assert_eq!(
            store
                .project_memory_retrieval_state("retrieval:archive", &owner, true, 1)
                .unwrap(),
            1
        );
        assert!(expect_matches(store.search(&dummy_embedding(4), &profile, 5).unwrap()).is_empty());

        assert_eq!(
            store
                .project_memory_retrieval_state("retrieval:restore", &owner, false, 2)
                .unwrap(),
            1
        );

        let results = expect_matches(store.search(&dummy_embedding(4), &profile, 5).unwrap());
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn vector_store_importance_scoring() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let profile = test_profile(4);
        let id = store
            .insert("s1", "important", &dummy_embedding(4), &profile, "chat")
            .unwrap();
        store.set_importance(id, 0.95).unwrap();
        let stats = store.tier_stats().unwrap();
        assert_eq!(stats.total, 1);
    }

    #[test]
    fn archived_high_importance_chunk_is_not_promoted() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let profile = test_profile(4);
        store
            .insert(
                "s1",
                "archived but important",
                &dummy_embedding(4),
                &profile,
                "chat",
            )
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE vectors SET archived = 1, tier = 3, importance_score = 0.95",
                [],
            )
            .unwrap();
        }

        let (promoted, _) = store.run_tier_maintenance().unwrap();
        assert_eq!(promoted, 0);
        {
            let conn = store.conn.lock().unwrap();
            let tier: i64 = conn
                .query_row("SELECT tier FROM vectors WHERE archived = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(tier, 3);
        }
    }

    #[test]
    fn vector_store_export_import_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb = dummy_embedding(4);
        let profile = test_profile(emb.len());
        store
            .insert("s1", "content", &emb, &profile, "note")
            .unwrap();
        let exported = store.export_all_chunks().unwrap();
        assert_eq!(exported.len(), 1);

        let dir2 = tempfile::tempdir().unwrap();
        let store2 = VectorStore::new(dir2.path().join("vectors2.db")).unwrap();
        store2.import_chunks(&exported).unwrap();
        assert_eq!(store2.count_all_chunks().unwrap(), 1);
        let found = expect_matches(
            store2
                .search_by_session("s1", &emb, &profile, 5, 100)
                .unwrap(),
        );
        assert_eq!(found[0].0.content, "content");
    }

    #[test]
    fn portable_vector_archive_excludes_derived_rows_and_reports_exact_replacement_counts() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let owner = test_owner("knowledge_note", "portable-archive-owner");
        store
            .project_memory_embedding(
                "outbox:portable-archive-owner",
                &owner,
                "canonical-session",
                "CANONICAL_CONTENT_MUST_NOT_BE_IMPORTED",
                &[0.1, 0.2, 0.3, 0.4],
                &profile,
            )
            .unwrap();
        store
            .insert(
                "portable-session",
                "portable",
                &[0.4, 0.3, 0.2, 0.1],
                &profile,
                "manual_note",
            )
            .unwrap();
        store
            .insert(
                "legacy-chat-session",
                "LEGACY_CHAT_DERIVED",
                &[0.3, 0.4, 0.1, 0.2],
                &profile,
                "user_message",
            )
            .unwrap();

        let portable = store.export_portable_chunks().unwrap();
        assert_eq!(portable.len(), 1);
        assert_eq!(portable[0].source, "manual_note");

        let mut old_archive = store.export_all_chunks().unwrap();
        old_archive
            .iter_mut()
            .find(|chunk| chunk.source == owner.source())
            .unwrap()
            .content = "SPOOFED_CANONICAL_IMPORT".into();
        let report = store.replace_portable_chunks(&old_archive).unwrap();
        assert_eq!(report.supplied, 3);
        assert_eq!(report.applied, 1);
        assert_eq!(report.skipped_canonical_projection, 1);
        assert_eq!(report.skipped_legacy_chat_projection, 1);
        assert_eq!(report.skipped(), 2);

        let active = store.export_all_chunks().unwrap();
        assert!(active
            .iter()
            .any(|chunk| chunk.content == "CANONICAL_CONTENT_MUST_NOT_BE_IMPORTED"));
        assert!(!active
            .iter()
            .any(|chunk| chunk.content == "SPOOFED_CANONICAL_IMPORT"));
        assert!(!active
            .iter()
            .any(|chunk| chunk.content == "LEGACY_CHAT_DERIVED"));
        assert!(store
            .projected_materialization_vector_id("outbox:portable-archive-owner", &owner)
            .unwrap()
            .is_some());
    }

    #[test]
    fn vector_store_clear_all_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let profile = test_profile(4);
        store
            .insert("s1", "x", &dummy_embedding(4), &profile, "chat")
            .unwrap();
        assert_eq!(store.count_all_chunks().unwrap(), 1);
        store.clear_all_chunks().unwrap();
        assert_eq!(store.count_all_chunks().unwrap(), 0);
    }

    #[test]
    fn vector_integrity_report_counts_corrupt_embeddings() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let profile = test_profile(4);
        store
            .insert("s1", "healthy", &dummy_embedding(4), &profile, "note")
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "s1",
                    "broken",
                    "not-json",
                    "note",
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .unwrap();
        }

        let report = store.integrity_report().unwrap();
        assert_eq!(report.total_chunks, 2);
        assert_eq!(report.corrupt_embedding_count, 1);
        assert_eq!(report.unknown_profile_count, 1);
        assert_eq!(report.profile_dimension_mismatch_count, 0);
    }

    #[test]
    fn legacy_unknown_profile_is_visible_without_being_called_blob_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("legacy-profile.db");
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE vectors (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    content TEXT NOT NULL,
                    embedding_json TEXT NOT NULL,
                    source TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at)
                 VALUES ('s1', 'legacy valid vector', '[0.1,0.2,0.3,0.4]', 'note', ?1)",
                [chrono::Utc::now().to_rfc3339()],
            )
            .unwrap();
        }

        let store = VectorStore::new(&db_path).unwrap();
        let report = store.integrity_report().unwrap();
        assert_eq!(report.total_chunks, 1);
        assert_eq!(report.corrupt_embedding_count, 0);
        assert_eq!(report.unknown_profile_count, 1);
        let profile = test_profile(4);
        let outcome = store.search(&dummy_embedding(4), &profile, 5).unwrap();
        assert!(matches!(
            outcome,
            VectorSearchOutcome::RebuildRequired(VectorRebuildEvidence {
                unknown_profile_count: 1,
                corrupt_embedding_count: 0,
                ..
            })
        ));
    }

    #[test]
    fn same_dimension_profile_change_requires_rebuild() {
        let store = VectorStore::new_in_memory().unwrap();
        let stored = EmbeddingProfile::new(
            EmbeddingRouteKind::Cloud,
            "openai",
            "model-a",
            "endpoint:sha256:test",
            "artifact-a",
            4,
        )
        .unwrap();
        let query = EmbeddingProfile::new(
            EmbeddingRouteKind::Cloud,
            "openai",
            "model-b",
            "endpoint:sha256:test",
            "artifact-b",
            4,
        )
        .unwrap();
        store
            .insert("s1", "profile-a", &dummy_embedding(4), &stored, "note")
            .unwrap();

        let outcome = store.search(&dummy_embedding(4), &query, 5).unwrap();
        assert!(matches!(
            outcome,
            VectorSearchOutcome::RebuildRequired(VectorRebuildEvidence {
                profile_mismatch_count: 1,
                dimension_mismatch_count: 0,
                ..
            })
        ));
    }

    #[test]
    fn matching_profile_with_wrong_blob_dimension_is_corrupt_not_scored() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let id = store
            .insert(
                "s1",
                "wrong blob dimension",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
                "note",
            )
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE vectors SET embedding_blob = ?1 WHERE id = ?2",
                params![encode_embedding_blob(&[1.0, 0.0, 0.0]), id],
            )
            .unwrap();
        }

        let outcome = store.search(&[1.0, 0.0, 0.0, 0.0], &profile, 5).unwrap();
        assert!(matches!(
            outcome,
            VectorSearchOutcome::RebuildRequired(VectorRebuildEvidence {
                corrupt_embedding_count: 1,
                ..
            })
        ));
    }

    #[test]
    fn malformed_blob_never_falls_back_to_stale_legacy_json_for_scoring() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let id = store
            .insert(
                "s1",
                "malformed canonical blob",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
                "note",
            )
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE vectors SET embedding_blob = ?1, embedding_json = '[1.0,0.0,0.0,0.0]' WHERE id = ?2",
                params![vec![1_u8, 2, 3], id],
            )
            .unwrap();
        }

        let outcome = store.search(&[1.0, 0.0, 0.0, 0.0], &profile, 5).unwrap();
        assert!(matches!(
            outcome,
            VectorSearchOutcome::RebuildRequired(VectorRebuildEvidence {
                corrupt_embedding_count: 1,
                ..
            })
        ));
    }

    #[test]
    fn unknown_profile_cannot_be_written_or_searched() {
        let store = VectorStore::new_in_memory().unwrap();
        let unknown = EmbeddingProfile::unknown();

        assert!(store
            .insert("s1", "must not mix", &[1.0], &unknown, "note")
            .is_err());
        assert!(store.search(&[1.0], &unknown, 5).is_err());
        assert_eq!(store.count_all_chunks().unwrap(), 0);
    }

    #[test]
    fn tampered_profile_fields_cannot_reuse_a_valid_profile_id() {
        let store = VectorStore::new_in_memory().unwrap();
        let mut tampered = test_profile(4);
        tampered.deployment_identity = "endpoint:sha256:tampered".into();

        assert!(store
            .insert(
                "s1",
                "tampered identity",
                &[1.0, 0.0, 0.0, 0.0],
                &tampered,
                "note"
            )
            .is_err());
        assert!(store.search(&[1.0, 0.0, 0.0, 0.0], &tampered, 5).is_err());
        assert_eq!(store.count_all_chunks().unwrap(), 0);
    }

    #[test]
    fn cosine_similarity_rejects_mismatched_lengths_instead_of_truncating() {
        assert_eq!(super::cosine_similarity(&[1.0, 0.0], &[1.0]), 0.0);
    }

    #[test]
    fn incompatible_rows_do_not_enter_cosine_or_crowd_compatible_rows() {
        let store = VectorStore::new_in_memory().unwrap();
        let compatible = EmbeddingProfile::new(
            EmbeddingRouteKind::Cloud,
            "openai",
            "model-compatible",
            "endpoint:sha256:test",
            "artifact-compatible",
            4,
        )
        .unwrap();
        let incompatible = EmbeddingProfile::new(
            EmbeddingRouteKind::Cloud,
            "openai",
            "model-incompatible",
            "endpoint:sha256:test",
            "artifact-incompatible",
            4,
        )
        .unwrap();
        let compatible_embedding = vec![1.0, 0.0, 0.0, 0.0];
        store
            .insert(
                "s1",
                "older compatible row",
                &compatible_embedding,
                &compatible,
                "note",
            )
            .unwrap();
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            let blob = encode_embedding_blob(&[0.0, 1.0, 0.0, 0.0]);
            for index in 0..2_050 {
                tx.execute(
                    "INSERT INTO vectors (session_id, content, embedding_json, embedding_blob, embedding_profile_id, embedding_dimension, source, created_at)
                     VALUES ('s1', ?1, '[]', ?2, ?3, 4, 'note', ?4)",
                    params![
                        format!("new incompatible row {index}"),
                        &blob,
                        &incompatible.id,
                        chrono::Utc::now().to_rfc3339(),
                    ],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }

        let outcome = store
            .search_by_session("s1", &compatible_embedding, &compatible, 5, 2_000)
            .unwrap();
        match outcome {
            VectorSearchOutcome::Matches {
                matches,
                rebuild: Some(rebuild),
            } => {
                assert_eq!(matches.len(), 1);
                assert_eq!(matches[0].0.content, "older compatible row");
                assert_eq!(rebuild.profile_mismatch_count, 2_050);
            }
            other => panic!("expected compatible matches with rebuild evidence, got {other:?}"),
        }
    }

    #[test]
    fn cosine_similarity_basic() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        let c = vec![0.0f32, 1.0, 0.0];
        assert!((super::cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
        assert!(super::cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_zero_vector() {
        let a = vec![0.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0, 0.0];
        assert_eq!(super::cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn cosine_similarity_is_finite_for_large_finite_components() {
        let a = vec![f32::MAX; 4];
        let b = vec![f32::MAX; 4];
        let score = super::cosine_similarity(&a, &b);

        assert!(score.is_finite());
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_norm_embedding_cannot_be_written() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);

        assert!(store
            .insert("s1", "zero vector", &[0.0, 0.0, 0.0, 0.0], &profile, "note")
            .is_err());
        assert_eq!(store.count_all_chunks().unwrap(), 0);
    }

    #[test]
    fn embedding_privacy_plan_masks_pii_and_blocks_cloud() {
        let engine = crate::privacy::PrivacyEngine::new();
        let plan = super::plan_embedding_privacy(
            "我的身份证是11010519491231002X，邮箱 test@example.com，最近在看用药记录",
            &engine,
            false,
        );

        assert!(!plan.cloud_allowed);
        assert!(plan.embedding_text.contains("<BLOCKED_IDCARD_0>"));
        assert!(plan.embedding_text.contains("<EMAIL_0>"));
        assert!(!plan.embedding_text.contains("11010519491231002X"));
        assert!(!plan.embedding_text.contains("test@example.com"));
        assert!(plan
            .blocking_reasons
            .iter()
            .any(|reason| reason == "pii_detected"));
    }

    #[test]
    fn embedding_privacy_plan_blocks_sensitive_topics_and_hs_local_only() {
        let engine = crate::privacy::PrivacyEngine::new();
        let health = super::plan_embedding_privacy("帮我分析一下最近的用药和诊断", &engine, false);
        assert!(!health.cloud_allowed);
        assert_eq!(
            health.sensitive_topic,
            Some(super::EmbeddingSensitiveTopic::Health)
        );

        let local_only = super::plan_embedding_privacy("普通任务安排", &engine, true);
        assert!(!local_only.cloud_allowed);
        assert!(local_only
            .blocking_reasons
            .iter()
            .any(|reason| reason == "hs_local_only"));
    }

    #[test]
    fn vector_search_dimension_mismatch_requires_rebuild_instead_of_silent_filtering() {
        let store = VectorStore::new_in_memory().unwrap();
        let stored_profile = test_profile(4);
        let query_profile = test_profile(3);
        store
            .insert(
                "profile-session",
                "profiled content",
                &[0.1, 0.2, 0.3, 0.4],
                &stored_profile,
                "note",
            )
            .unwrap();

        let outcome = store.search(&[0.1, 0.2, 0.3], &query_profile, 5).unwrap();
        assert!(matches!(
            outcome,
            VectorSearchOutcome::RebuildRequired(VectorRebuildEvidence {
                dimension_mismatch_count: 1,
                ..
            })
        ));
    }

    #[test]
    fn conversation_tombstone_deletes_searchable_session_projection_idempotently() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        store
            .insert(
                "deleted-session",
                "PRIVATE_VECTOR_SENTINEL",
                &[0.1, 0.2, 0.3, 0.4],
                &profile,
                "manual_index",
            )
            .unwrap();
        store
            .project_memory_embedding(
                "outbox:accepted-memory",
                &test_owner("memory_lifecycle", "memory-1"),
                "deleted-session",
                "accepted memory remains",
                &[0.4, 0.3, 0.2, 0.1],
                &profile,
            )
            .unwrap();
        store
            .project_memory_embedding(
                "outbox:manual-memory-record",
                &test_owner("memory_record", "42"),
                "deleted-session",
                "manual canonical memory remains",
                &[0.2, 0.3, 0.4, 0.1],
                &profile,
            )
            .unwrap();
        store
            .project_memory_embedding(
                "outbox:knowledge-note",
                &test_owner("knowledge_note", "7"),
                "deleted-session",
                "canonical KnowledgeNote remains",
                &[0.3, 0.2, 0.1, 0.4],
                &profile,
            )
            .unwrap();

        assert_eq!(
            store
                .project_conversation_tombstone("tombstone-1", "deleted-session")
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .project_conversation_tombstone("tombstone-1", "deleted-session")
                .unwrap(),
            0
        );
        let chunks = store.export_all_chunks().unwrap();
        assert!(!chunks
            .iter()
            .any(|chunk| chunk.content == "PRIVATE_VECTOR_SENTINEL"));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.source == "memory_lifecycle:memory-1"));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.source == "memory_record:42"));
        assert!(chunks
            .iter()
            .any(|chunk| chunk.source == "knowledge_note:7"));
        assert!(store
            .insert(
                "deleted-session",
                "late stale projection",
                &[0.1, 0.2, 0.3, 0.4],
                &profile,
                "manual_index",
            )
            .is_err());
    }

    #[test]
    fn vector_import_and_replace_cannot_bypass_conversation_tombstone() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        store
            .insert(
                "safe-session",
                "SAFE_VECTOR_MUST_SURVIVE_REJECTED_REPLACE",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
                "manual",
            )
            .unwrap();
        store
            .project_conversation_tombstone("tombstone-import", "deleted-session")
            .unwrap();
        let mut forbidden = store.export_all_chunks().unwrap()[0].clone();
        forbidden.session_id = "deleted-session".into();
        forbidden.content = "LATE_IMPORTED_VECTOR_MUST_NOT_APPEAR".into();

        assert!(store.import_chunks(&[forbidden.clone()]).is_err());
        assert!(store.replace_all_chunks(&[forbidden]).is_err());
        assert!(store
            .validate_portable_replacement(&[store.export_all_chunks().unwrap()[0].clone()])
            .is_ok());
        let mut forbidden_portable = store.export_all_chunks().unwrap()[0].clone();
        forbidden_portable.session_id = "deleted-session".into();
        forbidden_portable.content = "PORTABLE_REPLACE_MUST_NOT_BYPASS_TOMBSTONE".into();
        assert!(store
            .validate_portable_replacement(&[forbidden_portable.clone()])
            .is_err());
        assert!(store
            .replace_portable_chunks(&[forbidden_portable])
            .is_err());
        let chunks = store.export_all_chunks().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].content,
            "SAFE_VECTOR_MUST_SURVIVE_REJECTED_REPLACE"
        );
    }

    fn staged_rebuild_item(
        memory_id: i64,
        content: &str,
        profile: &EmbeddingProfile,
    ) -> VectorRebuildBatchItem {
        VectorRebuildBatchItem {
            memory_id,
            chunk: Some(ExportedVectorChunk {
                session_id: "rebuild-session".into(),
                content: content.into(),
                embedding: vec![1.0, 0.0, 0.0, 0.0],
                embedding_profile_id: profile.id.clone(),
                embedding_dimension: profile.dimension,
                source: format!("memory_lifecycle:{memory_id}"),
                created_at: chrono::Utc::now().to_rfc3339(),
                tier: 2,
                access_count: 0,
                last_accessed_at: String::new(),
                importance_score: 0.5,
                archived: false,
                archived_at: None,
                summary: None,
            }),
            canonical_owner: Some(
                CanonicalVectorOwnerRef::new("memory_lifecycle", &memory_id.to_string()).unwrap(),
            ),
            provider_dispatch_count: 0,
            cache_hit: false,
        }
    }

    #[test]
    fn rebuild_checkpoint_survives_restart_and_promotes_only_after_complete_scan() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vectors-rebuild.db");
        let profile = test_profile(4);
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 2,
            total_count: 2,
            metadata_digest: "sha256:stable-snapshot".into(),
        };

        let job_id = {
            let store = VectorStore::new(&path).unwrap();
            store
                .insert(
                    "old-session",
                    "OLD_ACTIVE_VECTOR",
                    &[1.0, 0.0, 0.0, 0.0],
                    &profile,
                    "unowned:old",
                )
                .unwrap();
            let job = store.start_or_resume_rebuild(&snapshot).unwrap();
            let progress = store
                .stage_rebuild_batch(&job.job_id, &[staged_rebuild_item(1, "new-one", &profile)])
                .unwrap();
            assert_eq!(progress.processed, 1);
            assert_eq!(progress.last_processed_memory_id, 1);
            assert_eq!(
                store.export_all_chunks().unwrap()[0].content,
                "OLD_ACTIVE_VECTOR"
            );
            job.job_id
        };

        let reopened = VectorStore::new(&path).unwrap();
        let resumed = reopened.start_or_resume_rebuild(&snapshot).unwrap();
        assert_eq!(resumed.job_id, job_id);
        assert_eq!(resumed.processed, 1);
        reopened
            .stage_rebuild_batch(&job_id, &[staged_rebuild_item(2, "new-two", &profile)])
            .unwrap();
        let completed = reopened.finalize_rebuild(&job_id, &snapshot).unwrap();

        assert_eq!(completed.status, VectorRebuildJobStatus::Completed);
        assert_eq!(completed.processed, 2);
        assert_eq!(completed.indexed, 2);
        let chunks = reopened.export_all_chunks().unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().any(|chunk| chunk.content == "new-one"));
        assert!(chunks.iter().any(|chunk| chunk.content == "new-two"));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.content != "OLD_ACTIVE_VECTOR"));
    }

    #[test]
    fn cancelled_rebuild_discards_shadow_projection_and_preserves_active_vectors() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        store
            .insert(
                "old-session",
                "ACTIVE_BEFORE_CANCEL",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
                "unowned:old",
            )
            .unwrap();
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 1,
            total_count: 1,
            metadata_digest: "sha256:cancel-snapshot".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        store
            .stage_rebuild_batch(
                &job.job_id,
                &[staged_rebuild_item(1, "SHADOW_MUST_DISAPPEAR", &profile)],
            )
            .unwrap();

        let requested = store.request_rebuild_cancel(&job.job_id).unwrap();
        assert_eq!(requested.status, VectorRebuildJobStatus::CancelRequested);
        let cancelled = store
            .settle_rebuild_cancel_with_remote_unknown(&job.job_id, true)
            .unwrap();
        assert_eq!(cancelled.status, VectorRebuildJobStatus::Cancelled);
        assert_eq!(cancelled.remote_unknown_provider_attempts, 1);
        assert_eq!(store.rebuild_staged_item_count(&job.job_id).unwrap(), 0);
        let active = store.export_all_chunks().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].content, "ACTIVE_BEFORE_CANCEL");
    }

    #[test]
    fn rebuild_rejects_changed_source_snapshot_without_partial_projection() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        store
            .insert(
                "old-session",
                "ACTIVE_BEFORE_STALE_REBUILD",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
                "unowned:old",
            )
            .unwrap();
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 1,
            total_count: 1,
            metadata_digest: "sha256:before".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        store
            .stage_rebuild_batch(&job.job_id, &[staged_rebuild_item(1, "staged", &profile)])
            .unwrap();

        let changed = VectorRebuildSourceSnapshot {
            metadata_digest: "sha256:after".into(),
            ..snapshot
        };
        assert!(store.finalize_rebuild(&job.job_id, &changed).is_err());
        assert_eq!(
            store.export_all_chunks().unwrap()[0].content,
            "ACTIVE_BEFORE_STALE_REBUILD"
        );
        let failed = store
            .fail_rebuild(
                &job.job_id,
                &crate::persistence_outbox::metadata_digest("source_snapshot_changed"),
            )
            .unwrap();
        assert_eq!(failed.status, VectorRebuildJobStatus::Failed);
        assert_eq!(store.rebuild_staged_item_count(&job.job_id).unwrap(), 0);
    }

    #[test]
    fn paused_rebuild_resumes_the_same_checkpoint_instead_of_starting_over() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 2,
            total_count: 2,
            metadata_digest: "sha256:pause-snapshot".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        let checkpoint = store
            .stage_rebuild_batch(&job.job_id, &[staged_rebuild_item(1, "first", &profile)])
            .unwrap();
        let paused = store
            .pause_rebuild(
                &job.job_id,
                &crate::persistence_outbox::metadata_digest("provider_temporarily_unavailable"),
            )
            .unwrap();
        assert_eq!(paused.status, VectorRebuildJobStatus::Paused);

        let resumed = store.start_or_resume_rebuild(&snapshot).unwrap();
        assert_eq!(resumed.job_id, job.job_id);
        assert_eq!(resumed.status, VectorRebuildJobStatus::Running);
        assert_eq!(
            resumed.last_processed_memory_id,
            checkpoint.last_processed_memory_id
        );
        assert_eq!(resumed.processed, 1);
    }

    #[test]
    fn memory_projection_is_idempotent_and_marker_never_copies_body() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let first = store
            .project_memory_embedding(
                "outbox:vector-test",
                &test_owner("memory_lifecycle", "memory:test"),
                "session-test",
                "VECTOR_PROJECTION_BODY_SENTINEL",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();
        let second = store
            .project_memory_embedding(
                "outbox:vector-test",
                &test_owner("memory_lifecycle", "memory:test"),
                "session-test",
                "VECTOR_PROJECTION_BODY_SENTINEL",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();

        assert_eq!(first, second);
        assert!(first.is_some());
        assert_eq!(store.count_all_chunks().unwrap(), 1);
        let conn = store.conn.lock().unwrap();
        let marker: String = conn
            .query_row(
                "SELECT event_id || aggregate_kind || aggregate_id || mutation_kind
                 FROM vector_materialization_projections
                 WHERE event_id = 'outbox:vector-test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!marker.contains("VECTOR_PROJECTION_BODY_SENTINEL"));
    }

    #[test]
    fn knowledge_note_projection_uses_its_canonical_owner_identity() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);

        let vector_id = store
            .project_memory_embedding(
                "outbox:knowledge-note",
                &test_owner("knowledge_note", "42"),
                "session-test",
                "KNOWLEDGE_NOTE_PROJECTION_SENTINEL",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap()
            .expect("an active canonical KnowledgeNote must be materialized");

        assert_eq!(store.count_all_chunks().unwrap(), 1);
        let conn = store.conn.lock().unwrap();
        let (source, marker_vector_id): (String, i64) = conn
            .query_row(
                "SELECT v.source, p.vector_id
                 FROM vector_materialization_projections p
                 JOIN vectors v ON v.id = p.vector_id
                 WHERE p.event_id = 'outbox:knowledge-note'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(source, "knowledge_note:42");
        assert_eq!(marker_vector_id, vector_id);
    }

    #[test]
    fn canonical_retrieval_projection_rejects_stale_head_and_fences_late_materialization() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let owner = test_owner("knowledge_note", "retrieval-head-owner");

        assert_eq!(
            store
                .project_memory_retrieval_state("retrieval:archive-head", &owner, true, 1)
                .unwrap(),
            0
        );
        let vector_id = store
            .project_memory_embedding(
                "outbox:retrieval-head-owner",
                &owner,
                "retrieval-session",
                "LATE_MATERIALIZATION_MUST_STAY_ARCHIVED",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap()
            .unwrap();
        assert!(
            store
                .export_all_chunks()
                .unwrap()
                .into_iter()
                .find(|chunk| chunk.source == owner.source())
                .unwrap()
                .archived
        );

        assert!(store
            .project_memory_retrieval_state("retrieval:revision-drift", &owner, false, 1)
            .is_err());
        assert_eq!(
            store
                .project_memory_retrieval_state("retrieval:restore-head", &owner, false, 2)
                .unwrap(),
            1
        );
        assert!(store
            .project_memory_retrieval_state("retrieval:stale-archive", &owner, true, 1)
            .is_err());
        let chunk = store
            .export_all_chunks()
            .unwrap()
            .into_iter()
            .find(|chunk| chunk.source == owner.source())
            .unwrap();
        assert!(!chunk.archived);
        assert!(vector_id > 0);
    }

    #[test]
    fn archived_canonical_owner_remains_unsearchable_after_rebuild_promotion() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let owner = test_owner("knowledge_note", "84");
        let event_id = "outbox:archived-rebuild-owner";
        store
            .project_memory_embedding(
                event_id,
                &owner,
                "archived-rebuild-session",
                "ARCHIVED_REBUILD_BODY",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();
        store
            .project_memory_retrieval_state("retrieval:archive-before-rebuild", &owner, true, 1)
            .unwrap();
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 84,
            total_count: 1,
            metadata_digest: "sha256:archived-rebuild-owner".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        let mut rebuilt = staged_rebuild_item(84, "ARCHIVED_REBUILD_BODY", &profile);
        rebuilt.chunk.as_mut().unwrap().source = owner.source();
        rebuilt.canonical_owner = Some(owner.clone());
        store.stage_rebuild_batch(&job.job_id, &[rebuilt]).unwrap();
        store.finalize_rebuild(&job.job_id, &snapshot).unwrap();

        let chunk = store.export_all_chunks().unwrap().pop().unwrap();
        assert!(chunk.archived);
        assert!(
            expect_matches(store.search(&[1.0, 0.0, 0.0, 0.0], &profile, 5).unwrap()).is_empty()
        );
    }

    #[test]
    fn generic_vector_writes_cannot_spoof_canonical_owner_prefixes() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        for source in [
            "knowledge_note:spoof",
            "memory_lifecycle:spoof",
            "memory_record:spoof",
        ] {
            assert!(store
                .insert(
                    "session-test",
                    "spoofed canonical owner",
                    &[1.0, 0.0, 0.0, 0.0],
                    &profile,
                    source,
                )
                .is_err());
        }
        assert_eq!(store.count_all_chunks().unwrap(), 0);
    }

    #[test]
    fn near_prefix_provenance_is_not_misclassified_as_canonical_owner() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        for source in ["knowledge-note:local", "memoryXlifecycle:local"] {
            store
                .insert(
                    "session-test",
                    "ordinary provenance",
                    &[1.0, 0.0, 0.0, 0.0],
                    &profile,
                    source,
                )
                .unwrap();
        }
        let exported = store.export_all_chunks().unwrap();
        store.replace_all_chunks(&exported).unwrap();
        assert_eq!(store.count_all_chunks().unwrap(), 2);
    }

    #[test]
    fn rebuild_rebinds_owner_markers_and_corruption_is_never_reported_applied() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let knowledge_event = "outbox:knowledge-before-rebuild";
        store
            .project_memory_embedding(
                knowledge_event,
                &test_owner("knowledge_note", "42"),
                "knowledge-session",
                "knowledge before rebuild",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();
        store
            .project_memory_embedding(
                "outbox:memory-record-before-rebuild",
                &test_owner("memory_record", "7"),
                "memory-session",
                "memory record before rebuild",
                &[0.0, 1.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();

        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 42,
            total_count: 2,
            metadata_digest: "sha256:marker-rebind-snapshot".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        let mut memory_record = staged_rebuild_item(7, "rebuilt memory record", &profile);
        memory_record.chunk.as_mut().unwrap().source = "memory_record:7".into();
        memory_record.canonical_owner = Some(test_owner("memory_record", "7"));
        let mut knowledge_note = staged_rebuild_item(42, "rebuilt knowledge note", &profile);
        knowledge_note.chunk.as_mut().unwrap().source = "knowledge_note:42".into();
        knowledge_note.canonical_owner = Some(test_owner("knowledge_note", "42"));
        store
            .stage_rebuild_batch(&job.job_id, &[memory_record, knowledge_note])
            .unwrap();
        store.finalize_rebuild(&job.job_id, &snapshot).unwrap();

        let rebound_id = store
            .projected_materialization_vector_id(
                knowledge_event,
                &test_owner("knowledge_note", "42"),
            )
            .unwrap()
            .expect("rebuild must rebind the KnowledgeNote marker");
        let conn = store.conn.lock().unwrap();
        let rebound_source: String = conn
            .query_row(
                "SELECT source FROM vectors WHERE id = ?1",
                [rebound_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rebound_source, "knowledge_note:42");
        let other_id: i64 = conn
            .query_row(
                "SELECT id FROM vectors WHERE source = 'memory_record:7'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute_batch("DROP TRIGGER vector_materialization_owner_update;")
            .unwrap();
        conn.execute(
            "UPDATE vector_materialization_projections SET vector_id = ?1
             WHERE event_id = ?2",
            params![other_id, knowledge_event],
        )
        .unwrap();
        drop(conn);
        assert!(store
            .projected_materialization_vector_id(
                knowledge_event,
                &test_owner("knowledge_note", "42"),
            )
            .is_err());

        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE vector_materialization_projections SET vector_id = ?1
             WHERE event_id = ?2",
            params![rebound_id, knowledge_event],
        )
        .unwrap();
        conn.execute("DELETE FROM vectors WHERE id = ?1", [rebound_id])
            .unwrap();
        drop(conn);

        assert!(store
            .projected_materialization_vector_id(
                knowledge_event,
                &test_owner("knowledge_note", "42"),
            )
            .is_err());
        assert!(store
            .project_memory_embedding(
                knowledge_event,
                &test_owner("knowledge_note", "42"),
                "knowledge-session",
                "canonical reload cannot hide a dangling marker",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .is_err());
    }

    #[test]
    fn rebuild_marker_rebind_failure_rolls_back_vectors_and_marker() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let owner = test_owner("knowledge_note", "42");
        let event_id = "outbox:knowledge-rebind-rollback";
        let original_vector_id = store
            .project_memory_embedding(
                event_id,
                &owner,
                "knowledge-session",
                "ORIGINAL_VECTOR_MUST_SURVIVE",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap()
            .unwrap();
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 42,
            total_count: 1,
            metadata_digest: "sha256:marker-rebind-rollback".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        let mut rebuilt = staged_rebuild_item(42, "REBUILT_MUST_ROLL_BACK", &profile);
        rebuilt.chunk.as_mut().unwrap().source = owner.source();
        rebuilt.canonical_owner = Some(owner.clone());
        store.stage_rebuild_batch(&job.job_id, &[rebuilt]).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER fail_marker_rebind_for_test
                 BEFORE UPDATE OF vector_id ON vector_materialization_projections
                 BEGIN
                     SELECT RAISE(ABORT, 'injected marker rebind failure');
                 END;",
            )
            .unwrap();
        }

        assert!(store.finalize_rebuild(&job.job_id, &snapshot).is_err());
        assert_eq!(
            store
                .projected_materialization_vector_id(event_id, &owner)
                .unwrap(),
            Some(original_vector_id)
        );
        let chunks = store.export_all_chunks().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "ORIGINAL_VECTOR_MUST_SURVIVE");
        assert_eq!(store.rebuild_staged_item_count(&job.job_id).unwrap(), 1);
        assert!(!store
            .rebuild_job(&job.job_id)
            .unwrap()
            .unwrap()
            .status
            .is_terminal());
    }

    #[test]
    fn lifecycle_tombstone_and_late_replay_do_not_unarchive_applied_projection() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let event_id = "outbox:create-before-delete";
        store
            .project_memory_embedding(
                event_id,
                &test_owner("memory_lifecycle", "memory:deleted"),
                "session-test",
                "must stay archived",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();
        assert_eq!(
            store
                .project_memory_lifecycle_tombstone("outbox:delete-after-create", "memory:deleted")
                .unwrap(),
            1
        );
        assert_eq!(store.count_all_chunks().unwrap(), 0);
        // A different late create event must be fenced by the durable delete
        // marker, not merely deduplicated by the original create event id.
        let late = store
            .project_memory_embedding(
                "outbox:late-create-after-delete",
                &test_owner("memory_lifecycle", "memory:deleted"),
                "session-test",
                "LATE_BODY_MUST_NOT_RESURRECT",
                &[1.0, 0.0, 0.0, 0.0],
                &profile,
            )
            .unwrap();
        assert_eq!(late, None);

        // Simulate an archived stale row from an older binary. Generic restore
        // must honor the durable lifecycle deletion fence, and re-applying the
        // same tombstone must scrub all derived raw content.
        let stale_id = {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO vectors (
                    session_id, content, embedding_json, embedding_blob,
                    embedding_profile_id, embedding_dimension, source, created_at
                 ) VALUES (?1, ?2, '[]', ?3, ?4, ?5, ?6, ?7)",
                params![
                    "session-test",
                    "LEGACY_STALE_BODY_MUST_BE_SCRUBBED",
                    encode_embedding_blob(&[1.0, 0.0, 0.0, 0.0]),
                    &profile.id,
                    profile.dimension as i64,
                    "memory_lifecycle:memory:deleted",
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();
            conn.last_insert_rowid()
        };
        assert_eq!(
            store
                .conn
                .lock()
                .unwrap()
                .execute("UPDATE vectors SET archived = 1 WHERE id = ?1", [stale_id])
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .project_memory_lifecycle_tombstone("outbox:delete-after-create", "memory:deleted")
                .unwrap(),
            1
        );
        let chunks = store.export_all_chunks().unwrap();
        assert!(chunks.is_empty());
        let serialized = serde_json::to_string(&chunks).unwrap();
        assert!(!serialized.contains("LATE_BODY_MUST_NOT_RESURRECT"));
        assert!(!serialized.contains("LEGACY_STALE_BODY_MUST_BE_SCRUBBED"));
    }

    #[test]
    fn twenty_thousand_item_rebuild_stages_in_bounded_batches_and_promotes_under_gate() {
        let store = VectorStore::new_in_memory().unwrap();
        let profile = test_profile(4);
        let snapshot = VectorRebuildSourceSnapshot {
            through_memory_id: 20_000,
            total_count: 20_000,
            metadata_digest: "sha256:20k-snapshot".into(),
        };
        let job = store.start_or_resume_rebuild(&snapshot).unwrap();
        let started = std::time::Instant::now();
        for first in (1_i64..=20_000).step_by(VECTOR_REBUILD_BATCH_LIMIT) {
            let last = (first + VECTOR_REBUILD_BATCH_LIMIT as i64 - 1).min(20_000);
            let contents = (first..=last)
                .map(|id| format!("rebuild-{id}"))
                .collect::<Vec<_>>();
            let items = (first..=last)
                .zip(contents.iter())
                .map(|(id, content)| staged_rebuild_item(id, content, &profile))
                .collect::<Vec<_>>();
            assert!(items.len() <= VECTOR_REBUILD_BATCH_LIMIT);
            store.stage_rebuild_batch(&job.job_id, &items).unwrap();
        }
        let completed = store.finalize_rebuild(&job.job_id, &snapshot).unwrap();
        let elapsed = started.elapsed();

        assert_eq!(completed.indexed, 20_000);
        assert_eq!(store.count_all_chunks().unwrap(), 20_000);
        assert!(
            elapsed < std::time::Duration::from_secs(8),
            "20,000-item staged rebuild exceeded the frozen 8s local debug gate: {elapsed:?}"
        );
    }

    #[test]
    fn embedding_cloud_adapter_has_no_direct_reqwest_or_unbounded_response_reader() {
        let source = include_str!("vectors.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        let direct_client = ["reqwest", "::Client"].concat();
        assert!(!production.contains(&direct_client));
        assert!(!production.contains(".json::<serde_json::Value>().await"));
    }
}
