use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

/// A memory chunk with embedding stored in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub source: &'a str,
}

pub struct VectorStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIntegrityReport {
    pub total_chunks: i64,
    pub corrupt_embedding_count: i64,
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
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().context("failed to open in-memory vector sqlite db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS vectors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding_json TEXT NOT NULL,
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
        // Migrate old tables that lack the new columns
        let _ = conn.execute(
            "ALTER TABLE vectors ADD COLUMN tier INTEGER NOT NULL DEFAULT 2",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE vectors ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE vectors ADD COLUMN last_accessed_at TEXT", []);
        let _ = conn.execute(
            "ALTER TABLE vectors ADD COLUMN importance_score REAL NOT NULL DEFAULT 0.5",
            [],
        );
        let _ = conn.execute(
            "ALTER TABLE vectors ADD COLUMN archived INTEGER NOT NULL DEFAULT 0",
            [],
        );
        let _ = conn.execute("ALTER TABLE vectors ADD COLUMN archived_at TEXT", []);
        let _ = conn.execute("ALTER TABLE vectors ADD COLUMN summary TEXT", []);
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_session ON vectors(session_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_tier ON vectors(tier)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_archived ON vectors(archived)",
            [],
        )?;
        Ok(())
    }

    pub fn insert(
        &self,
        session_id: &str,
        content: &str,
        embedding: &[f32],
        source: &str,
    ) -> Result<i64> {
        let json = serde_json::to_string(embedding)?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO vectors (session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![session_id, content, json, source, chrono::Utc::now().to_rfc3339(), 2, 0, Option::<String>::None, 0.5_f32, 0, Option::<String>::None, Option::<String>::None],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn insert_batch(&self, items: &[VectorInsertItem<'_>]) -> Result<usize> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        for item in items {
            let json = serde_json::to_string(item.embedding)?;
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![item.session_id, item.content, json, item.source, &now, 2, 0, Option::<String>::None, 0.5_f32, 0, Option::<String>::None, Option::<String>::None],
            )?;
        }
        tx.commit()?;
        Ok(items.len())
    }

    fn row_to_chunk(row: &rusqlite::Row) -> rusqlite::Result<(MemoryChunk, Vec<f32>)> {
        let embedding_json: String = row.get(3)?;
        let embedding: Vec<f32> = serde_json::from_str(&embedding_json).unwrap_or_default();
        Ok((
            MemoryChunk {
                id: row.get(0)?,
                session_id: row.get(1)?,
                content: row.get(2)?,
                source: row.get(4)?,
                created_at: row.get(5)?,
                tier: row.get(6)?,
                access_count: row.get(7)?,
                last_accessed_at: row.get::<_, Option<String>>(8)?.unwrap_or_default(),
                importance_score: row.get::<_, Option<f32>>(9)?.unwrap_or(0.5),
                archived: row.get::<_, Option<i64>>(10)?.unwrap_or(0) != 0,
                archived_at: row.get::<_, Option<String>>(11)?,
                summary: row.get::<_, Option<String>>(12)?,
            },
            embedding,
        ))
    }

    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Result<Vec<(MemoryChunk, f32)>> {
        let top: Vec<(MemoryChunk, f32)> = {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
            // Limit scan to the most recent 2000 non-archived chunks
            let mut stmt = conn.prepare(
                "SELECT id, session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary FROM vectors WHERE archived = 0 ORDER BY created_at DESC LIMIT 2000",
            )?;
            let rows = stmt.query_map([], Self::row_to_chunk)?;

            let mut results: Vec<(MemoryChunk, f32)> = Vec::new();
            for row in rows {
                let (chunk, emb) = row?;
                if emb.len() == query_embedding.len() && !emb.is_empty() {
                    let score = cosine_similarity(query_embedding, &emb);
                    results.push((chunk, score));
                }
            }

            // Sort by composite score: similarity + tier bonus
            results.sort_by(|a, b| {
                let comp_a = Self::composite_score(a.1, a.0.tier);
                let comp_b = Self::composite_score(b.1, b.0.tier);
                comp_b
                    .partial_cmp(&comp_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.into_iter().take(top_k).collect()
        };
        // Bump access stats for returned results (must be outside conn lock)
        self.bump_access_for_chunks(&top)?;
        Ok(top)
    }

    /// Search scoped to a specific session_id, with a hard limit on scan size.
    pub fn search_by_session(
        &self,
        session_id: &str,
        query_embedding: &[f32],
        top_k: usize,
        limit: usize,
    ) -> Result<Vec<(MemoryChunk, f32)>> {
        let top: Vec<(MemoryChunk, f32)> = {
            let conn = self
                .conn
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
            let mut stmt = conn.prepare(
                "SELECT id, session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary FROM vectors WHERE session_id = ?1 AND archived = 0 ORDER BY created_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![session_id, limit as i64], Self::row_to_chunk)?;

            let mut results: Vec<(MemoryChunk, f32)> = Vec::new();
            for row in rows {
                let (chunk, emb) = row?;
                if emb.len() == query_embedding.len() && !emb.is_empty() {
                    let score = cosine_similarity(query_embedding, &emb);
                    results.push((chunk, score));
                }
            }

            results.sort_by(|a, b| {
                let comp_a = Self::composite_score(a.1, a.0.tier);
                let comp_b = Self::composite_score(b.1, b.0.tier);
                comp_b
                    .partial_cmp(&comp_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.into_iter().take(top_k).collect()
        };
        self.bump_access_for_chunks(&top)?;
        Ok(top)
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
        Ok((promoted as usize, demoted as usize))
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
        let mut stmt = conn.prepare("SELECT embedding_json FROM vectors")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut total = 0_i64;
        let mut corrupt = 0_i64;
        for row in rows {
            total += 1;
            let embedding_json = row?;
            match serde_json::from_str::<Vec<f32>>(&embedding_json) {
                Ok(embedding) if !embedding.is_empty() => {}
                _ => corrupt += 1,
            }
        }
        Ok(VectorIntegrityReport {
            total_chunks: total,
            corrupt_embedding_count: corrupt,
        })
    }

    pub fn export_all_chunks(&self) -> Result<Vec<ExportedVectorChunk>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary FROM vectors ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            let embedding_json: String = row.get(2)?;
            let embedding: Vec<f32> = serde_json::from_str(&embedding_json).unwrap_or_default();
            Ok(ExportedVectorChunk {
                session_id: row.get(0)?,
                content: row.get(1)?,
                embedding,
                source: row.get(3)?,
                created_at: row.get(4)?,
                tier: row.get(5)?,
                access_count: row.get(6)?,
                last_accessed_at: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
                importance_score: row.get::<_, Option<f32>>(8)?.unwrap_or(0.5),
                archived: row.get::<_, Option<i64>>(9)?.unwrap_or(0) != 0,
                archived_at: row.get::<_, Option<String>>(10)?,
                summary: row.get::<_, Option<String>>(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to export vectors")
    }

    pub fn clear_all_chunks(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute("DELETE FROM vectors", [])?;
        Ok(())
    }

    pub fn import_chunks(&self, chunks: &[ExportedVectorChunk]) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        for chunk in chunks {
            let embedding_json = serde_json::to_string(&chunk.embedding)?;
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![chunk.session_id, chunk.content, embedding_json, chunk.source, chunk.created_at, chunk.tier, chunk.access_count, &chunk.last_accessed_at, chunk.importance_score, chunk.archived as i64, &chunk.archived_at, &chunk.summary],
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
            let embedding_json = serde_json::to_string(&chunk.embedding)?;
            tx.execute(
                "INSERT INTO vectors (session_id, content, embedding_json, source, created_at, tier, access_count, last_accessed_at, importance_score, archived, archived_at, summary) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![chunk.session_id, chunk.content, embedding_json, chunk.source, chunk.created_at, chunk.tier, chunk.access_count, &chunk.last_accessed_at, chunk.importance_score, chunk.archived as i64, &chunk.archived_at, &chunk.summary],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Archive low-access memories: mark as archived, store summary, retain original for recovery.
    /// Criteria: tier >= 3, not accessed in 90+ days, access_count <= 2, importance_score < 0.3.
    pub fn archive_low_access_memories(&self) -> Result<usize> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        let now = chrono::Utc::now().to_rfc3339();
        let archived = tx.execute(
            "UPDATE vectors SET archived = 1, archived_at = ?1, summary = substr(content, 1, 200) WHERE archived = 0 AND tier >= 3 AND (last_accessed_at IS NULL OR last_accessed_at < ?2) AND access_count <= 2 AND importance_score < 0.3",
            params![&now, &cutoff],
        )?;
        tx.commit()?;
        Ok(archived as usize)
    }

    /// Restore archived memories back to active state.
    pub fn restore_archived(&self, chunk_ids: &[i64]) -> Result<usize> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let placeholders: Vec<String> = chunk_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "UPDATE vectors SET archived = 0, archived_at = NULL, tier = 2 WHERE archived = 1 AND id IN ({})",
            placeholders.join(",")
        );
        let restored = tx.execute(&sql, rusqlite::params_from_iter(chunk_ids.iter()))?;
        tx.commit()?;
        Ok(restored as usize)
    }

    /// List archived chunks with summary (no embedding to save memory).
    pub fn list_archived(&self, limit: usize) -> Result<Vec<ArchivedChunkSummary>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, content, source, created_at, archived_at, summary, access_count, importance_score FROM vectors WHERE archived = 1 ORDER BY archived_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok(ArchivedChunkSummary {
                id: row.get(0)?,
                session_id: row.get(1)?,
                content: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
                archived_at: row.get(5)?,
                summary: row.get(6)?,
                access_count: row.get(7)?,
                importance_score: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .context("failed to list archived")
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArchivedChunkSummary {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub source: String,
    pub created_at: String,
    pub archived_at: String,
    pub summary: Option<String>,
    pub access_count: i64,
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

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Embedding with automatic fallback:
/// 1. OpenRouter (text-embedding-3-small, 1536-dim)
/// 2. Local Ollama (nomic-embed-text or similar)
/// 3. Deterministic hash-based fallback (384-dim)
pub async fn embed_text(
    text: &str,
    openai_base: &str,
    openai_key: &str,
    embedding_model: &str,
) -> Result<Vec<f32>> {
    embed_text_with_config(
        text,
        "openrouter",
        openai_base,
        openai_key,
        embedding_model,
        true,
    )
    .await
}

pub async fn embed_text_with_config(
    text: &str,
    provider: &str,
    openai_base: &str,
    openai_key: &str,
    embedding_model: &str,
    embedding_enabled: bool,
) -> Result<Vec<f32>> {
    let api_key = crate::llm::effective_api_key(provider, openai_key);
    if embedding_enabled && !api_key.is_empty() {
        let client = reqwest::Client::new();
        let model_name = if embedding_model.is_empty() {
            "openai/text-embedding-3-small"
        } else {
            embedding_model
        };
        let body = serde_json::json!({
            "model": model_name,
            "input": text,
        });
        let url = if openai_base.is_empty() {
            format!(
                "{}/embeddings",
                crate::llm::default_base_for_provider(provider)
            )
        } else {
            format!("{}/embeddings", openai_base.trim_end_matches('/'))
        };
        if let Ok(res) = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            let status = res.status();
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if status.is_success() {
                    if let Some(arr) = json["data"][0]["embedding"].as_array() {
                        let embedding = arr
                            .iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect::<Vec<f32>>();
                        if !embedding.is_empty() {
                            return Ok(embedding);
                        }
                    }
                }
            }
        }
    }

    // Fallback 1: Ollama local embedding
    if crate::ollama::is_ollama_available("nomic-embed-text").await {
        if let Ok(emb) = crate::ollama::ollama_embed(text, "nomic-embed-text").await {
            if !emb.is_empty() {
                return Ok(emb);
            }
        }
    }

    // Fallback 2: deterministic hash-based embedding
    Ok(crate::ollama::fallback_embed(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_embedding(len: usize) -> Vec<f32> {
        (0..len).map(|i| i as f32 * 0.01).collect()
    }

    #[test]
    fn vector_store_insert_and_count() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb = dummy_embedding(4);
        let id = store.insert("s1", "hello", &emb, "chat").unwrap();
        assert!(id > 0);
        assert_eq!(store.count_all_chunks().unwrap(), 1);
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
        let index_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_vectors_archived'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(archived_count, 1);
        assert_eq!(index_count, 1);
    }

    #[test]
    fn vector_store_insert_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb_a = dummy_embedding(4);
        let emb_b = dummy_embedding(4);
        let items = vec![
            VectorInsertItem {
                session_id: "s1",
                content: "a",
                embedding: &emb_a,
                source: "chat",
            },
            VectorInsertItem {
                session_id: "s1",
                content: "b",
                embedding: &emb_b,
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
        store.insert("s1", "hello world", &emb, "chat").unwrap();
        store.insert("s2", "other session", &emb, "chat").unwrap();
        let results = store.search_by_session("s1", &emb, 5, 100).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.session_id, "s1");
    }

    #[test]
    fn vector_store_search_global() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let emb_a = vec![1.0f32, 0.0, 0.0, 0.0];
        let emb_b = vec![0.0f32, 1.0, 0.0, 0.0];
        store.insert("s1", "alpha", &emb_a, "chat").unwrap();
        store.insert("s1", "beta", &emb_b, "chat").unwrap();
        let results = store.search(&emb_a, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0.content, "alpha");
    }

    #[test]
    fn vector_store_tier_maintenance() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        store
            .insert("s1", "old", &dummy_embedding(4), "chat")
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
    fn vector_store_archive_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        store
            .insert("s1", "forgettable", &dummy_embedding(4), "chat")
            .unwrap();
        // Set old access date and low importance
        {
            let conn = store.conn.lock().unwrap();
            let old = (chrono::Utc::now() - chrono::Duration::days(120)).to_rfc3339();
            conn.execute("UPDATE vectors SET last_accessed_at = ?1, access_count = 1, importance_score = 0.1, tier = 3", params![&old]).unwrap();
        }
        let archived = store.archive_low_access_memories().unwrap();
        assert_eq!(archived, 1);

        // Search should not find archived
        let results = store.search(&dummy_embedding(4), 5).unwrap();
        assert_eq!(results.len(), 0);

        // List archived
        let list = store.list_archived(10).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].summary.is_some());

        // Restore
        let restored = store.restore_archived(&[list[0].id]).unwrap();
        assert_eq!(restored, 1);

        let results = store.search(&dummy_embedding(4), 5).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn vector_store_importance_scoring() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        let id = store
            .insert("s1", "important", &dummy_embedding(4), "chat")
            .unwrap();
        store.set_importance(id, 0.95).unwrap();
        let stats = store.tier_stats().unwrap();
        assert_eq!(stats.total, 1);
    }

    #[test]
    fn archived_high_importance_chunk_is_not_promoted() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        store
            .insert("s1", "archived but important", &dummy_embedding(4), "chat")
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
        store.insert("s1", "content", &emb, "note").unwrap();
        let exported = store.export_all_chunks().unwrap();
        assert_eq!(exported.len(), 1);

        let dir2 = tempfile::tempdir().unwrap();
        let store2 = VectorStore::new(dir2.path().join("vectors2.db")).unwrap();
        store2.import_chunks(&exported).unwrap();
        assert_eq!(store2.count_all_chunks().unwrap(), 1);
        let found = store2.search_by_session("s1", &emb, 5, 100).unwrap();
        assert_eq!(found[0].0.content, "content");
    }

    #[test]
    fn vector_store_clear_all_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        store
            .insert("s1", "x", &dummy_embedding(4), "chat")
            .unwrap();
        assert_eq!(store.count_all_chunks().unwrap(), 1);
        store.clear_all_chunks().unwrap();
        assert_eq!(store.count_all_chunks().unwrap(), 0);
    }

    #[test]
    fn vector_integrity_report_counts_corrupt_embeddings() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().join("vectors.db")).unwrap();
        store
            .insert("s1", "healthy", &dummy_embedding(4), "note")
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
}
