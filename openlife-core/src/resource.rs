//! Canonical ownership for user-imported resources.
//!
//! Resource bytes, parser output, provenance, message bindings, tombstones, and
//! outbox facts live in one SQLite owner. Parsing happens before this store is
//! entered; callers must submit a complete bounded batch, so a parser failure
//! cannot leave partially imported context behind.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

pub const MAX_RESOURCES_PER_IMPORT: usize = 5;
pub const MAX_RESOURCE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_IMPORT_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_CHUNKS_PER_RESOURCE: usize = 256;
pub const MAX_CHUNK_CHARS: usize = 64 * 1024;

const RESOURCE_AGGREGATE_KIND: &str = "imported_resource";
const RESOURCE_PROJECTION_TARGET: &str = "resource_projection";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceFormat {
    Text,
    Markdown,
    Json,
    Source,
    Pdf,
    Docx,
    Csv,
    Xlsx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceProvenance {
    Text {
        start_line: u32,
        end_line: u32,
    },
    Pdf {
        page: u32,
    },
    Docx {
        paragraph_start: u32,
        paragraph_end: u32,
    },
    Csv {
        range: String,
    },
    Xlsx {
        sheet: String,
        range: String,
    },
}

impl ResourceProvenance {
    fn validate_for(&self, format: ResourceFormat) -> Result<()> {
        let valid = matches!(
            (format, self),
            (
                ResourceFormat::Text
                    | ResourceFormat::Markdown
                    | ResourceFormat::Json
                    | ResourceFormat::Source,
                Self::Text { .. }
            ) | (ResourceFormat::Pdf, Self::Pdf { .. })
                | (ResourceFormat::Docx, Self::Docx { .. })
                | (ResourceFormat::Csv, Self::Csv { .. })
                | (ResourceFormat::Xlsx, Self::Xlsx { .. })
        );
        if !valid {
            anyhow::bail!("resource_provenance_format_mismatch");
        }
        match self {
            Self::Text {
                start_line,
                end_line,
            } if *start_line == 0 || end_line < start_line => {
                anyhow::bail!("resource_text_provenance_invalid")
            }
            Self::Pdf { page } if *page == 0 => {
                anyhow::bail!("resource_pdf_provenance_invalid")
            }
            Self::Docx {
                paragraph_start,
                paragraph_end,
            } if *paragraph_start == 0 || paragraph_end < paragraph_start => {
                anyhow::bail!("resource_docx_provenance_invalid")
            }
            Self::Csv { range } if !valid_cell_range(range) => {
                anyhow::bail!("resource_csv_provenance_invalid")
            }
            Self::Xlsx { sheet, range }
                if sheet.trim().is_empty() || sheet.len() > 128 || !valid_cell_range(range) =>
            {
                anyhow::bail!("resource_xlsx_provenance_invalid")
            }
            _ => Ok(()),
        }
    }
}

fn valid_cell_range(value: &str) -> bool {
    let Some((start, end)) = value.split_once(':') else {
        return false;
    };
    [start, end].into_iter().all(|cell| {
        let letters = cell.bytes().take_while(u8::is_ascii_alphabetic).count();
        letters > 0 && letters <= 3 && cell[letters..].parse::<u32>().is_ok_and(|row| row > 0)
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceChunkDraft {
    pub content: String,
    pub provenance: ResourceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceImportCandidate {
    pub resource_id: String,
    pub filename: String,
    pub declared_mime: String,
    pub detected_mime: String,
    pub format: ResourceFormat,
    pub bytes: Vec<u8>,
    pub chunks: Vec<ResourceChunkDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceImportBatch {
    pub operation_id: String,
    pub message_id: String,
    pub resources: Vec<ResourceImportCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedResourceReceipt {
    pub resource_id: String,
    pub binding_id: String,
    pub filename: String,
    pub digest: String,
    pub byte_count: u64,
    pub chunk_count: u32,
    pub reused_existing: bool,
    pub event_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceImportReceipt {
    pub operation_id: String,
    pub message_id: String,
    pub resources: Vec<ImportedResourceReceipt>,
    pub committed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredResource {
    pub resource_id: String,
    pub filename: String,
    pub declared_mime: String,
    pub detected_mime: String,
    pub format: ResourceFormat,
    pub digest: String,
    pub byte_count: u64,
    pub chunk_count: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredResourceChunk {
    pub resource_id: String,
    pub ordinal: u32,
    pub content: String,
    pub content_digest: String,
    pub provenance: ResourceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceContextChunk {
    pub resource: StoredResource,
    pub chunk: StoredResourceChunk,
}

#[derive(Clone)]
pub struct ResourceStore {
    db_path: Option<PathBuf>,
    conn: Arc<Mutex<Connection>>,
}

impl ResourceStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create ResourceStore directory {}", parent.display()))?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("open canonical ResourceStore {}", db_path.display()))?;
        configure_connection(&conn)?;
        let store = Self {
            db_path: Some(db_path),
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory ResourceStore")?;
        configure_connection(&conn)?;
        let store = Self {
            db_path: None,
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.lock_connection()?;
        crate::persistence_outbox::init_schema(&conn)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS resource_blobs (
                digest TEXT PRIMARY KEY,
                byte_count INTEGER NOT NULL CHECK(byte_count > 0),
                content_bytes BLOB NOT NULL,
                created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS imported_resources (
                resource_id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                declared_mime TEXT NOT NULL,
                detected_mime TEXT NOT NULL,
                format TEXT NOT NULL,
                digest TEXT NOT NULL UNIQUE,
                byte_count INTEGER NOT NULL CHECK(byte_count > 0),
                chunk_count INTEGER NOT NULL CHECK(chunk_count > 0),
                created_at TEXT NOT NULL,
                deleted_at TEXT
             );
             CREATE TABLE IF NOT EXISTS resource_chunks (
                resource_id TEXT NOT NULL,
                ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
                content TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                provenance_json TEXT NOT NULL,
                PRIMARY KEY(resource_id, ordinal),
                FOREIGN KEY(resource_id) REFERENCES imported_resources(resource_id) ON DELETE CASCADE
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS resource_message_bindings (
                binding_id TEXT PRIMARY KEY,
                operation_id TEXT NOT NULL,
                message_id TEXT NOT NULL,
                resource_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(operation_id, resource_id),
                FOREIGN KEY(resource_id) REFERENCES imported_resources(resource_id)
             );
             CREATE INDEX IF NOT EXISTS idx_resource_bindings_message
             ON resource_message_bindings(message_id, created_at, resource_id);
             CREATE TABLE IF NOT EXISTS resource_import_operations (
                operation_id TEXT PRIMARY KEY,
                payload_digest TEXT NOT NULL,
                receipt_json TEXT NOT NULL,
                created_at TEXT NOT NULL
             );",
        )?;
        Ok(())
    }

    pub fn commit_import_batch(&self, batch: ResourceImportBatch) -> Result<ResourceImportReceipt> {
        self.commit_import_batch_guarded(batch, || Result::<()>::Ok(()))
    }

    /// Commit a complete import while holding a caller-provided linearization
    /// guard only across the final durable commit. Expensive parsing and SQL
    /// preparation must happen before that guard so cancellation remains fast.
    pub fn commit_import_batch_guarded<G, F>(
        &self,
        batch: ResourceImportBatch,
        acquire_commit_guard: F,
    ) -> Result<ResourceImportReceipt>
    where
        F: FnOnce() -> Result<G>,
    {
        let prepared = PreparedImportBatch::validate(batch)?;
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut acquire_commit_guard = Some(acquire_commit_guard);

        if let Some((existing_digest, receipt_json)) = tx
            .query_row(
                "SELECT payload_digest, receipt_json FROM resource_import_operations
                 WHERE operation_id = ?1",
                [&prepared.operation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if existing_digest != prepared.payload_digest {
                anyhow::bail!("resource_import_operation_payload_drift");
            }
            let receipt: ResourceImportReceipt = serde_json::from_str(&receipt_json)
                .context("decode canonical ResourceStore replay receipt")?;
            for resource in &receipt.resources {
                let active = tx
                    .query_row(
                        "SELECT deleted_at IS NULL FROM imported_resources WHERE resource_id = ?1",
                        [&resource.resource_id],
                        |row| row.get::<_, bool>(0),
                    )
                    .optional()?;
                if active != Some(true) {
                    anyhow::bail!("resource_import_replay_tombstoned");
                }
            }
            let _commit_guard = acquire_commit_guard
                .take()
                .ok_or_else(|| anyhow::anyhow!("resource_import_commit_guard_missing"))?(
            )?;
            tx.rollback()?;
            return Ok(receipt);
        }

        let now = Utc::now();
        let mut receipts = Vec::with_capacity(prepared.resources.len());
        for candidate in prepared.resources {
            let existing = load_resource_by_digest(&tx, &candidate.digest)?;
            let (resource_id, chunk_count, reused_existing, event_id) = match existing {
                Some((_, true, _)) => anyhow::bail!("resource_digest_tombstoned"),
                Some((resource_id, false, existing_chunk_count)) => {
                    (resource_id, existing_chunk_count, true, None)
                }
                None => {
                    tx.execute(
                        "INSERT INTO resource_blobs (digest, byte_count, content_bytes, created_at)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            candidate.digest,
                            candidate.bytes.len() as i64,
                            candidate.bytes,
                            now.to_rfc3339(),
                        ],
                    )?;
                    tx.execute(
                        "INSERT INTO imported_resources (
                            resource_id, filename, declared_mime, detected_mime, format,
                            digest, byte_count, chunk_count, created_at, deleted_at
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL)",
                        params![
                            candidate.resource_id,
                            candidate.filename,
                            candidate.declared_mime,
                            candidate.detected_mime,
                            format_label(candidate.format),
                            candidate.digest,
                            candidate.bytes.len() as i64,
                            candidate.chunks.len() as i64,
                            now.to_rfc3339(),
                        ],
                    )?;
                    for (ordinal, chunk) in candidate.chunks.iter().enumerate() {
                        tx.execute(
                            "INSERT INTO resource_chunks (
                                resource_id, ordinal, content, content_digest, provenance_json
                             ) VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![
                                candidate.resource_id,
                                ordinal as i64,
                                chunk.content,
                                content_digest(chunk.content.as_bytes()),
                                serde_json::to_string(&chunk.provenance)?,
                            ],
                        )?;
                    }
                    let event = crate::persistence_outbox::enqueue_mutation(
                        &tx,
                        RESOURCE_AGGREGATE_KIND,
                        &candidate.resource_id,
                        "imported",
                        &candidate.digest,
                        &[RESOURCE_PROJECTION_TARGET],
                    )?;
                    (
                        candidate.resource_id.clone(),
                        candidate.chunks.len() as u32,
                        false,
                        Some(event.event_id),
                    )
                }
            };
            let binding_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO resource_message_bindings (
                    binding_id, operation_id, message_id, resource_id, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    binding_id,
                    prepared.operation_id,
                    prepared.message_id,
                    resource_id,
                    now.to_rfc3339(),
                ],
            )?;
            receipts.push(ImportedResourceReceipt {
                resource_id,
                binding_id,
                filename: candidate.filename,
                digest: candidate.digest,
                byte_count: candidate.bytes.len() as u64,
                chunk_count,
                reused_existing,
                event_id,
            });
        }

        let receipt = ResourceImportReceipt {
            operation_id: prepared.operation_id.clone(),
            message_id: prepared.message_id,
            resources: receipts,
            committed_at: now,
        };
        tx.execute(
            "INSERT INTO resource_import_operations (
                operation_id, payload_digest, receipt_json, created_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                prepared.operation_id,
                prepared.payload_digest,
                serde_json::to_string(&receipt)?,
                now.to_rfc3339(),
            ],
        )?;
        let _commit_guard = acquire_commit_guard
            .take()
            .ok_or_else(|| anyhow::anyhow!("resource_import_commit_guard_missing"))?(
        )?;
        tx.commit()?;
        Ok(receipt)
    }

    pub fn get_resource(&self, resource_id: &str) -> Result<Option<StoredResource>> {
        validate_uuid_v4("resource_id", resource_id)?;
        let conn = self.lock_connection()?;
        conn.query_row(
            "SELECT resource_id, filename, declared_mime, detected_mime, format,
                    digest, byte_count, chunk_count, created_at
             FROM imported_resources
             WHERE resource_id = ?1 AND deleted_at IS NULL",
            [resource_id],
            stored_resource_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn load_bytes(&self, resource_id: &str) -> Result<Option<Vec<u8>>> {
        validate_uuid_v4("resource_id", resource_id)?;
        let conn = self.lock_connection()?;
        conn.query_row(
            "SELECT blobs.content_bytes
             FROM imported_resources resources
             JOIN resource_blobs blobs ON blobs.digest = resources.digest
             WHERE resources.resource_id = ?1 AND resources.deleted_at IS NULL",
            [resource_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_chunks(&self, resource_id: &str) -> Result<Vec<StoredResourceChunk>> {
        validate_uuid_v4("resource_id", resource_id)?;
        let conn = self.lock_connection()?;
        let mut statement = conn.prepare(
            "SELECT chunks.ordinal, chunks.content, chunks.content_digest, chunks.provenance_json
             FROM resource_chunks chunks
             JOIN imported_resources resources ON resources.resource_id = chunks.resource_id
             WHERE chunks.resource_id = ?1 AND resources.deleted_at IS NULL
             ORDER BY chunks.ordinal ASC",
        )?;
        let chunks = statement
            .query_map([resource_id], |row| {
                let provenance_json: String = row.get(3)?;
                let provenance = serde_json::from_str(&provenance_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        provenance_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                let ordinal_raw: i64 = row.get(0)?;
                Ok(StoredResourceChunk {
                    resource_id: resource_id.to_string(),
                    ordinal: ordinal_raw as u32,
                    content: row.get(1)?,
                    content_digest: row.get(2)?,
                    provenance,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(anyhow::Error::from)?;
        Ok(chunks)
    }

    pub fn list_context_chunks_for_message(
        &self,
        message_id: &str,
    ) -> Result<Vec<ResourceContextChunk>> {
        if message_id.trim().is_empty() || message_id.len() > 256 {
            anyhow::bail!("resource_context_message_id_invalid");
        }
        let conn = self.lock_connection()?;
        let mut statement = conn.prepare(
            "SELECT resources.resource_id, resources.filename,
                    resources.declared_mime, resources.detected_mime,
                    resources.format, resources.digest, resources.byte_count,
                    resources.chunk_count, resources.created_at,
                    chunks.ordinal, chunks.content, chunks.content_digest,
                    chunks.provenance_json
             FROM resource_message_bindings bindings
             JOIN imported_resources resources
               ON resources.resource_id = bindings.resource_id
             JOIN resource_chunks chunks
               ON chunks.resource_id = resources.resource_id
             WHERE bindings.message_id = ?1 AND resources.deleted_at IS NULL
             GROUP BY resources.resource_id, chunks.ordinal
             ORDER BY resources.resource_id ASC, chunks.ordinal ASC",
        )?;
        let rows = statement.query_map([message_id], |row| {
            let format_text: String = row.get(4)?;
            let created_at_text: String = row.get(8)?;
            let resource = StoredResource {
                resource_id: row.get(0)?,
                filename: row.get(1)?,
                declared_mime: row.get(2)?,
                detected_mime: row.get(3)?,
                format: parse_format(&format_text).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        format_text.len(),
                        rusqlite::types::Type::Text,
                        error.into(),
                    )
                })?,
                digest: row.get(5)?,
                byte_count: row.get::<_, i64>(6)? as u64,
                chunk_count: row.get::<_, i64>(7)? as u32,
                created_at: DateTime::parse_from_rfc3339(&created_at_text)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            created_at_text.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?
                    .with_timezone(&Utc),
            };
            let provenance_json: String = row.get(12)?;
            let provenance = serde_json::from_str(&provenance_json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    provenance_json.len(),
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(ResourceContextChunk {
                chunk: StoredResourceChunk {
                    resource_id: resource.resource_id.clone(),
                    ordinal: row.get::<_, i64>(9)? as u32,
                    content: row.get(10)?,
                    content_digest: row.get(11)?,
                    provenance,
                },
                resource,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete_resource(
        &self,
        resource_id: &str,
        reason: Option<&str>,
    ) -> Result<crate::persistence_outbox::CanonicalMutationReceipt> {
        validate_uuid_v4("resource_id", resource_id)?;
        let mut conn = self.lock_connection()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let digest: Option<String> = tx
            .query_row(
                "SELECT digest FROM imported_resources WHERE resource_id = ?1",
                [resource_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(digest) = digest else {
            anyhow::bail!("resource_not_found");
        };
        let receipt = crate::persistence_outbox::enqueue_tombstone(
            &tx,
            RESOURCE_AGGREGATE_KIND,
            resource_id,
            reason,
            &[RESOURCE_PROJECTION_TARGET],
        )?;
        tx.execute(
            "UPDATE imported_resources
             SET deleted_at = COALESCE(deleted_at, ?2)
             WHERE resource_id = ?1",
            params![resource_id, Utc::now().to_rfc3339()],
        )?;
        tx.execute(
            "DELETE FROM resource_chunks WHERE resource_id = ?1",
            [resource_id],
        )?;
        tx.execute(
            "DELETE FROM resource_message_bindings WHERE resource_id = ?1",
            [resource_id],
        )?;
        tx.execute("DELETE FROM resource_blobs WHERE digest = ?1", [digest])?;
        tx.commit()?;
        Ok(receipt)
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("ResourceStore mutex poisoned: {error}"))
    }
}

struct PreparedImportBatch {
    operation_id: String,
    message_id: String,
    payload_digest: String,
    resources: Vec<PreparedImportCandidate>,
}

struct PreparedImportCandidate {
    resource_id: String,
    filename: String,
    declared_mime: String,
    detected_mime: String,
    format: ResourceFormat,
    digest: String,
    bytes: Vec<u8>,
    chunks: Vec<ResourceChunkDraft>,
}

impl PreparedImportBatch {
    fn validate(batch: ResourceImportBatch) -> Result<Self> {
        validate_uuid_v4("operation_id", &batch.operation_id)?;
        if batch.message_id.trim().is_empty() || batch.message_id.len() > 256 {
            anyhow::bail!("resource_import_message_id_invalid");
        }
        if batch.resources.is_empty() || batch.resources.len() > MAX_RESOURCES_PER_IMPORT {
            anyhow::bail!("resource_import_file_count_exceeded");
        }
        let total_bytes = batch
            .resources
            .iter()
            .try_fold(0usize, |total, resource| {
                total.checked_add(resource.bytes.len())
            })
            .ok_or_else(|| anyhow::anyhow!("resource_import_total_bytes_overflow"))?;
        if total_bytes > MAX_IMPORT_BYTES {
            anyhow::bail!("resource_import_total_bytes_exceeded");
        }

        let mut ids = std::collections::BTreeSet::new();
        let mut digests = std::collections::BTreeSet::new();
        let mut resources = Vec::with_capacity(batch.resources.len());
        for candidate in batch.resources {
            validate_uuid_v4("resource_id", &candidate.resource_id)?;
            if !ids.insert(candidate.resource_id.clone()) {
                anyhow::bail!("resource_import_duplicate_resource_id");
            }
            let filename = candidate.filename.trim();
            if filename.is_empty() || filename.len() > 255 || filename.contains(['/', '\\', '\0']) {
                anyhow::bail!("resource_filename_invalid");
            }
            if candidate.declared_mime.trim().is_empty()
                || candidate.declared_mime.len() > 128
                || candidate.detected_mime.trim().is_empty()
                || candidate.detected_mime.len() > 128
            {
                anyhow::bail!("resource_mime_invalid");
            }
            if candidate.bytes.is_empty() || candidate.bytes.len() > MAX_RESOURCE_BYTES {
                anyhow::bail!("resource_file_bytes_exceeded");
            }
            if candidate.chunks.is_empty() || candidate.chunks.len() > MAX_CHUNKS_PER_RESOURCE {
                anyhow::bail!("resource_chunk_count_exceeded");
            }
            for chunk in &candidate.chunks {
                if chunk.content.trim().is_empty()
                    || chunk.content.chars().count() > MAX_CHUNK_CHARS
                {
                    anyhow::bail!("resource_chunk_content_invalid");
                }
                chunk.provenance.validate_for(candidate.format)?;
            }
            let digest = content_digest(&candidate.bytes);
            if !digests.insert(digest.clone()) {
                anyhow::bail!("resource_import_duplicate_content");
            }
            resources.push(PreparedImportCandidate {
                resource_id: candidate.resource_id,
                filename: filename.to_string(),
                declared_mime: candidate.declared_mime,
                detected_mime: candidate.detected_mime,
                format: candidate.format,
                digest,
                bytes: candidate.bytes,
                chunks: candidate.chunks,
            });
        }
        let payload_digest = operation_payload_digest(&batch.message_id, &resources)?;
        Ok(Self {
            operation_id: batch.operation_id,
            message_id: batch.message_id,
            payload_digest,
            resources,
        })
    }
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    if !conn.is_autocommit() {
        anyhow::bail!("ResourceStore connection unexpectedly opened in a transaction");
    }
    if conn.path().is_some() {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
    }
    Ok(())
}

fn load_resource_by_digest(conn: &Connection, digest: &str) -> Result<Option<(String, bool, u32)>> {
    conn.query_row(
        "SELECT resource_id, deleted_at IS NOT NULL, chunk_count
         FROM imported_resources WHERE digest = ?1",
        [digest],
        |row| {
            let chunk_count: i64 = row.get(2)?;
            Ok((row.get(0)?, row.get(1)?, chunk_count as u32))
        },
    )
    .optional()
    .map_err(Into::into)
}

fn stored_resource_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredResource> {
    let format: String = row.get(4)?;
    let created_at: String = row.get(8)?;
    Ok(StoredResource {
        resource_id: row.get(0)?,
        filename: row.get(1)?,
        declared_mime: row.get(2)?,
        detected_mime: row.get(3)?,
        format: parse_format(&format).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                format.len(),
                rusqlite::types::Type::Text,
                error.into(),
            )
        })?,
        digest: row.get(5)?,
        byte_count: row.get::<_, i64>(6)? as u64,
        chunk_count: row.get::<_, i64>(7)? as u32,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    created_at.len(),
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?
            .with_timezone(&Utc),
    })
}

fn format_label(format: ResourceFormat) -> &'static str {
    match format {
        ResourceFormat::Text => "text",
        ResourceFormat::Markdown => "markdown",
        ResourceFormat::Json => "json",
        ResourceFormat::Source => "source",
        ResourceFormat::Pdf => "pdf",
        ResourceFormat::Docx => "docx",
        ResourceFormat::Csv => "csv",
        ResourceFormat::Xlsx => "xlsx",
    }
}

fn parse_format(value: &str) -> Result<ResourceFormat> {
    match value {
        "text" => Ok(ResourceFormat::Text),
        "markdown" => Ok(ResourceFormat::Markdown),
        "json" => Ok(ResourceFormat::Json),
        "source" => Ok(ResourceFormat::Source),
        "pdf" => Ok(ResourceFormat::Pdf),
        "docx" => Ok(ResourceFormat::Docx),
        "csv" => Ok(ResourceFormat::Csv),
        "xlsx" => Ok(ResourceFormat::Xlsx),
        _ => anyhow::bail!("unsupported canonical resource format: {value}"),
    }
}

fn content_digest(bytes: &[u8]) -> String {
    let value = digest(&SHA256, bytes);
    let mut encoded = String::with_capacity(64);
    for byte in value.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("sha256:{encoded}")
}

fn operation_payload_digest(
    message_id: &str,
    resources: &[PreparedImportCandidate],
) -> Result<String> {
    let payload = resources
        .iter()
        .map(|resource| {
            serde_json::json!({
                "filename": resource.filename,
                "declaredMime": resource.declared_mime,
                "detectedMime": resource.detected_mime,
                "format": format_label(resource.format),
                "digest": resource.digest,
                "chunks": resource.chunks.iter().map(|chunk| serde_json::json!({
                    "contentDigest": content_digest(chunk.content.as_bytes()),
                    "provenance": chunk.provenance,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(content_digest(
        serde_json::to_string(&(message_id, payload))?.as_bytes(),
    ))
}

fn validate_uuid_v4(label: &str, value: &str) -> Result<()> {
    let parsed = Uuid::parse_str(value).with_context(|| format!("{label}_invalid"))?;
    if parsed.get_version_num() != 4 || parsed.to_string() != value.to_ascii_lowercase() {
        anyhow::bail!("{label}_must_be_uuid_v4");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_candidate(bytes: &[u8]) -> ResourceImportCandidate {
        ResourceImportCandidate {
            resource_id: Uuid::new_v4().to_string(),
            filename: "roadshow.md".to_string(),
            declared_mime: "text/markdown".to_string(),
            detected_mime: "text/markdown".to_string(),
            format: ResourceFormat::Markdown,
            bytes: bytes.to_vec(),
            chunks: vec![ResourceChunkDraft {
                content: String::from_utf8(bytes.to_vec()).unwrap(),
                provenance: ResourceProvenance::Text {
                    start_line: 1,
                    end_line: 1,
                },
            }],
        }
    }

    fn batch(
        operation_id: String,
        message_id: &str,
        resources: Vec<ResourceImportCandidate>,
    ) -> ResourceImportBatch {
        ResourceImportBatch {
            operation_id,
            message_id: message_id.to_string(),
            resources,
        }
    }

    #[test]
    fn canonical_import_is_atomic_replay_safe_and_deduplicated() {
        let store = ResourceStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().to_string();
        let first = text_candidate(b"roadshow evidence");
        let first_resource_id = first.resource_id.clone();
        let request = batch(operation_id.clone(), "message-1", vec![first.clone()]);

        let receipt = store.commit_import_batch(request).unwrap();
        let replay = store
            .commit_import_batch(batch(
                operation_id,
                "message-1",
                vec![ResourceImportCandidate {
                    resource_id: Uuid::new_v4().to_string(),
                    ..first
                }],
            ))
            .unwrap();
        assert_eq!(receipt, replay);
        assert_eq!(receipt.resources[0].resource_id, first_resource_id);
        assert!(!receipt.resources[0].reused_existing);

        let second = text_candidate(b"roadshow evidence");
        let reused = store
            .commit_import_batch(batch(Uuid::new_v4().to_string(), "message-2", vec![second]))
            .unwrap();
        assert!(reused.resources[0].reused_existing);
        assert_eq!(reused.resources[0].resource_id, first_resource_id);

        let conn = store.lock_connection().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM resource_blobs", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM imported_resources", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM resource_message_bindings",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn operation_payload_drift_fails_closed_without_partial_writes() {
        let store = ResourceStore::new_in_memory().unwrap();
        let operation_id = Uuid::new_v4().to_string();
        store
            .commit_import_batch(batch(
                operation_id.clone(),
                "message-1",
                vec![text_candidate(b"first")],
            ))
            .unwrap();
        let error = store
            .commit_import_batch(batch(
                operation_id,
                "message-1",
                vec![text_candidate(b"drift")],
            ))
            .unwrap_err();
        assert!(error.to_string().contains("payload_drift"));

        let invalid = ResourceImportCandidate {
            chunks: Vec::new(),
            ..text_candidate(b"invalid")
        };
        assert!(store
            .commit_import_batch(batch(
                Uuid::new_v4().to_string(),
                "message-2",
                vec![text_candidate(b"would-partially-write"), invalid],
            ))
            .is_err());
        let conn = store.lock_connection().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM resource_blobs", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn tombstone_deletes_sensitive_content_and_prevents_restart_resurrection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("resources.db");
        let candidate = text_candidate(b"sensitive attachment");
        let resource_id = candidate.resource_id.clone();
        let digest = content_digest(&candidate.bytes);
        let import_operation_id = Uuid::new_v4().to_string();
        {
            let store = ResourceStore::new(&path).unwrap();
            store
                .commit_import_batch(batch(
                    import_operation_id.clone(),
                    "message-1",
                    vec![candidate.clone()],
                ))
                .unwrap();
            let first_delete = store
                .delete_resource(&resource_id, Some("user deleted attachment"))
                .unwrap();
            let replay_delete = store
                .delete_resource(&resource_id, Some("user deleted attachment"))
                .unwrap();
            assert_eq!(first_delete.event_id, replay_delete.event_id);
            assert!(store.get_resource(&resource_id).unwrap().is_none());
            assert!(store.load_bytes(&resource_id).unwrap().is_none());
            let replay_error = store
                .commit_import_batch(batch(import_operation_id, "message-1", vec![candidate]))
                .unwrap_err();
            assert!(replay_error.to_string().contains("replay_tombstoned"));
        }

        let restarted = ResourceStore::new(&path).unwrap();
        assert!(restarted.get_resource(&resource_id).unwrap().is_none());
        let retry = text_candidate(b"sensitive attachment");
        let error = restarted
            .commit_import_batch(batch(Uuid::new_v4().to_string(), "message-2", vec![retry]))
            .unwrap_err();
        assert!(error.to_string().contains("tombstoned"));
        let conn = restarted.lock_connection().unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM resource_blobs", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM imported_resources WHERE digest = ?1 AND deleted_at IS NOT NULL",
                [digest],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
    }
}
