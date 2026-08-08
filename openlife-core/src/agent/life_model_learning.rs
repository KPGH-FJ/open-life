use crate::life_model::v2::{LifeModelSectionV2, LifeModelUserValueV2};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_SUMMARY_CHARS: usize = 240;
const MAX_VALUE_CHARS: usize = 512;
const MAX_REF_CHARS: usize = 256;
const MAX_LIST_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelLearningExplicitness {
    ExplicitUserRequest,
}

impl LifeModelLearningExplicitness {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitUserRequest => "explicit_user_request",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "explicit_user_request" => Ok(Self::ExplicitUserRequest),
            _ => bail!("invalid_lifemodel_learning_explicitness"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelLearningSensitivity {
    Internal,
}

impl LifeModelLearningSensitivity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "internal" => Ok(Self::Internal),
            _ => bail!("invalid_lifemodel_learning_sensitivity"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelLearningCandidateStatus {
    Accumulating,
}

impl LifeModelLearningCandidateStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accumulating => "accumulating",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "accumulating" => Ok(Self::Accumulating),
            _ => bail!("invalid_lifemodel_learning_candidate_status"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLearningCapture {
    pub workspace_ref: String,
    pub source_ref: String,
    pub source_digest: String,
    pub summary: String,
    pub section: LifeModelSectionV2,
    pub value: LifeModelUserValueV2,
    pub explicitness: LifeModelLearningExplicitness,
    pub sensitivity: LifeModelLearningSensitivity,
    pub observed_at: String,
    pub observation_expires_at: String,
    pub candidate_expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLearningObservation {
    pub id: String,
    pub workspace_ref: String,
    pub source_ref: String,
    pub source_digest: String,
    pub summary: String,
    pub section: LifeModelSectionV2,
    pub value: LifeModelUserValueV2,
    pub explicitness: LifeModelLearningExplicitness,
    pub sensitivity: LifeModelLearningSensitivity,
    pub observed_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLearningCandidate {
    pub id: String,
    pub workspace_ref: String,
    pub summary: String,
    pub section: LifeModelSectionV2,
    pub value: LifeModelUserValueV2,
    pub status: LifeModelLearningCandidateStatus,
    pub explicitness: LifeModelLearningExplicitness,
    pub sensitivity: LifeModelLearningSensitivity,
    pub observation_ids: Vec<String>,
    pub source_refs: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLearningCaptureReceipt {
    pub observation: LifeModelLearningObservation,
    pub candidate: LifeModelLearningCandidate,
    pub replayed: bool,
    pub proposal_created: bool,
    pub canonical_life_model_changed: bool,
}

pub struct LifeModelLearningStore {
    conn: Mutex<Connection>,
}

impl LifeModelLearningStore {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create_lifemodel_learning_parent:{parent:?}"))?;
        }
        let connection = Connection::open(&path)
            .with_context(|| format!("open_lifemodel_learning_store:{path:?}"))?;
        Self::from_connection(connection)
    }

    pub fn new_in_memory() -> Result<Self> {
        Self::from_connection(
            Connection::open_in_memory().context("open_in_memory_lifemodel_learning_store")?,
        )
    }

    pub fn open_read_only_existing(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let connection = crate::sqlite_migration::open_existing_read_only(
            &path,
            "life_model_learning_store",
            &[
                "life_model_learning_observations",
                "life_model_learning_candidates",
            ],
        )?;
        Ok(Self {
            conn: Mutex::new(connection),
        })
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .context("configure_lifemodel_learning_busy_timeout")?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS life_model_learning_observations (
                    id TEXT PRIMARY KEY,
                    identity_digest TEXT NOT NULL UNIQUE,
                    workspace_ref TEXT NOT NULL,
                    source_ref TEXT NOT NULL,
                    source_digest TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    section TEXT NOT NULL,
                    value_json TEXT NOT NULL,
                    explicitness TEXT NOT NULL,
                    sensitivity TEXT NOT NULL,
                    observed_at TEXT NOT NULL,
                    expires_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS life_model_learning_candidates (
                    id TEXT PRIMARY KEY,
                    observation_id TEXT NOT NULL UNIQUE,
                    workspace_ref TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    section TEXT NOT NULL,
                    value_json TEXT NOT NULL,
                    status TEXT NOT NULL,
                    explicitness TEXT NOT NULL,
                    sensitivity TEXT NOT NULL,
                    source_ref TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    expires_at TEXT NOT NULL,
                    FOREIGN KEY(observation_id) REFERENCES life_model_learning_observations(id)
                        ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_lifemodel_learning_candidate_workspace_status
                    ON life_model_learning_candidates(workspace_ref, status, updated_at DESC);",
            )
            .context("initialize_lifemodel_learning_store")?;
        Ok(Self {
            conn: Mutex::new(connection),
        })
    }

    pub fn capture_explicit_candidate(
        &self,
        capture: LifeModelLearningCapture,
    ) -> Result<LifeModelLearningCaptureReceipt> {
        validate_capture(&capture)?;
        let value_json =
            serde_json::to_string(&capture.value).context("serialize_lifemodel_learning_value")?;
        let identity_digest = sha256_prefixed(
            format!(
                "{}\0{}\0{}\0{}\0{}",
                capture.workspace_ref,
                capture.source_ref,
                capture.source_digest,
                section_name(capture.section),
                value_json
            )
            .as_bytes(),
        );
        let observation_id = format!("lmo_{}", &identity_digest[7..31]);
        let candidate_id = format!("lmc_{}", &identity_digest[31..55]);

        let mut connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        let transaction = connection
            .transaction()
            .context("begin_lifemodel_learning_capture")?;
        if let Some(observation) = transaction
            .query_row(
                &format!(
                    "SELECT {} FROM life_model_learning_observations WHERE identity_digest = ?1",
                    observation_columns()
                ),
                [&identity_digest],
                observation_from_row,
            )
            .optional()
            .context("read_replayed_lifemodel_learning_observation")?
        {
            let candidate = transaction
                .query_row(
                    &format!(
                        "SELECT {} FROM life_model_learning_candidates WHERE observation_id = ?1",
                        candidate_columns()
                    ),
                    [&observation.id],
                    candidate_from_row,
                )
                .context("read_replayed_lifemodel_learning_candidate")?;
            transaction
                .commit()
                .context("commit_replayed_lifemodel_learning_capture")?;
            return Ok(LifeModelLearningCaptureReceipt {
                observation,
                candidate,
                replayed: true,
                proposal_created: false,
                canonical_life_model_changed: false,
            });
        }

        transaction
            .execute(
                "INSERT INTO life_model_learning_observations (
                    id, identity_digest, workspace_ref, source_ref, source_digest, summary,
                    section, value_json, explicitness, sensitivity, observed_at, expires_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    observation_id,
                    identity_digest,
                    capture.workspace_ref,
                    capture.source_ref,
                    capture.source_digest,
                    capture.summary,
                    section_name(capture.section),
                    value_json,
                    capture.explicitness.as_str(),
                    capture.sensitivity.as_str(),
                    capture.observed_at,
                    capture.observation_expires_at,
                ],
            )
            .context("insert_lifemodel_learning_observation")?;
        transaction
            .execute(
                "INSERT INTO life_model_learning_candidates (
                    id, observation_id, workspace_ref, summary, section, value_json, status,
                    explicitness, sensitivity, source_ref, created_at, updated_at, expires_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12)",
                params![
                    candidate_id,
                    observation_id,
                    capture.workspace_ref,
                    capture.summary,
                    section_name(capture.section),
                    serde_json::to_string(&capture.value)?,
                    LifeModelLearningCandidateStatus::Accumulating.as_str(),
                    capture.explicitness.as_str(),
                    capture.sensitivity.as_str(),
                    capture.source_ref,
                    capture.observed_at,
                    capture.candidate_expires_at,
                ],
            )
            .context("insert_lifemodel_learning_candidate")?;
        transaction
            .commit()
            .context("commit_lifemodel_learning_capture")?;
        drop(connection);

        let observation = self
            .get_observation(&observation_id)?
            .ok_or_else(|| anyhow!("lifemodel_learning_observation_missing_after_commit"))?;
        let candidate = self
            .get_candidate(&candidate_id)?
            .ok_or_else(|| anyhow!("lifemodel_learning_candidate_missing_after_commit"))?;
        Ok(LifeModelLearningCaptureReceipt {
            observation,
            candidate,
            replayed: false,
            proposal_created: false,
            canonical_life_model_changed: false,
        })
    }

    pub fn list_active_candidates(
        &self,
        workspace_ref: &str,
        limit: usize,
    ) -> Result<Vec<LifeModelLearningCandidate>> {
        validate_ref(workspace_ref, "invalid_lifemodel_learning_workspace_ref")?;
        if limit == 0 || limit > MAX_LIST_LIMIT {
            bail!("invalid_lifemodel_learning_candidate_limit");
        }
        let connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        let mut statement = connection
            .prepare(&format!(
                "SELECT {} FROM life_model_learning_candidates
                 WHERE workspace_ref = ?1 AND status = ?2
                 ORDER BY updated_at DESC, id ASC LIMIT ?3",
                candidate_columns()
            ))
            .context("prepare_lifemodel_learning_candidate_list")?;
        let candidates = statement
            .query_map(
                params![
                    workspace_ref,
                    LifeModelLearningCandidateStatus::Accumulating.as_str(),
                    limit as i64
                ],
                candidate_from_row,
            )
            .context("query_lifemodel_learning_candidate_list")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("collect_lifemodel_learning_candidate_list")?;
        Ok(candidates)
    }

    pub fn delete_candidate(&self, workspace_ref: &str, candidate_id: &str) -> Result<bool> {
        validate_ref(workspace_ref, "invalid_lifemodel_learning_workspace_ref")?;
        validate_ref(candidate_id, "invalid_lifemodel_learning_candidate_id")?;
        let mut connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        let transaction = connection
            .transaction()
            .context("begin_lifemodel_learning_candidate_delete")?;
        let observation_id = transaction
            .query_row(
                "SELECT observation_id FROM life_model_learning_candidates
                 WHERE id = ?1 AND workspace_ref = ?2",
                params![candidate_id, workspace_ref],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("read_lifemodel_learning_candidate_delete_owner")?;
        let Some(observation_id) = observation_id else {
            transaction
                .commit()
                .context("commit_missing_lifemodel_learning_candidate_delete")?;
            return Ok(false);
        };
        transaction
            .execute(
                "DELETE FROM life_model_learning_candidates WHERE id = ?1 AND workspace_ref = ?2",
                params![candidate_id, workspace_ref],
            )
            .context("delete_lifemodel_learning_candidate")?;
        transaction
            .execute(
                "DELETE FROM life_model_learning_observations WHERE id = ?1 AND workspace_ref = ?2",
                params![observation_id, workspace_ref],
            )
            .context("delete_lifemodel_learning_observation")?;
        transaction
            .commit()
            .context("commit_lifemodel_learning_candidate_delete")?;
        Ok(true)
    }

    fn get_observation(&self, id: &str) -> Result<Option<LifeModelLearningObservation>> {
        let connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        connection
            .query_row(
                &format!(
                    "SELECT {} FROM life_model_learning_observations WHERE id = ?1",
                    observation_columns()
                ),
                [id],
                observation_from_row,
            )
            .optional()
            .context("read_lifemodel_learning_observation")
    }

    fn get_candidate(&self, id: &str) -> Result<Option<LifeModelLearningCandidate>> {
        let connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        connection
            .query_row(
                &format!(
                    "SELECT {} FROM life_model_learning_candidates WHERE id = ?1",
                    candidate_columns()
                ),
                [id],
                candidate_from_row,
            )
            .optional()
            .context("read_lifemodel_learning_candidate")
    }
}

fn validate_capture(capture: &LifeModelLearningCapture) -> Result<()> {
    validate_ref(
        &capture.workspace_ref,
        "invalid_lifemodel_learning_workspace_ref",
    )?;
    validate_ref(&capture.source_ref, "invalid_lifemodel_learning_source_ref")?;
    validate_digest(&capture.source_digest)?;
    let summary = capture.summary.trim();
    if summary.is_empty() || summary.chars().count() > MAX_SUMMARY_CHARS {
        bail!("invalid_lifemodel_learning_summary");
    }
    if !matches!(
        capture.section,
        LifeModelSectionV2::StablePreferences | LifeModelSectionV2::CollaborationPreferences
    ) {
        bail!("unsupported_lifemodel_learning_section_5_3a");
    }
    match &capture.value {
        LifeModelUserValueV2::Statement { statement }
            if !statement.trim().is_empty() && statement.chars().count() <= MAX_VALUE_CHARS => {}
        _ => bail!("unsupported_lifemodel_learning_value_5_3a"),
    }
    let observed_at = parse_time(
        &capture.observed_at,
        "invalid_lifemodel_learning_observed_at",
    )?;
    let observation_expires_at = parse_time(
        &capture.observation_expires_at,
        "invalid_lifemodel_learning_observation_expires_at",
    )?;
    let candidate_expires_at = parse_time(
        &capture.candidate_expires_at,
        "invalid_lifemodel_learning_candidate_expires_at",
    )?;
    if observation_expires_at <= observed_at || candidate_expires_at <= observation_expires_at {
        bail!("invalid_lifemodel_learning_retention_window");
    }
    Ok(())
}

fn validate_ref(value: &str, error: &str) -> Result<()> {
    if value.trim().is_empty() || value.chars().count() > MAX_REF_CHARS {
        bail!(error.to_string());
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<()> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        bail!("invalid_lifemodel_learning_source_digest");
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid_lifemodel_learning_source_digest");
    }
    Ok(())
}

fn parse_time(value: &str, error: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| anyhow!(error.to_string()))
}

fn section_name(section: LifeModelSectionV2) -> &'static str {
    match section {
        LifeModelSectionV2::StablePreferences => "stable_preferences",
        LifeModelSectionV2::CollaborationPreferences => "collaboration_preferences",
        _ => "unsupported",
    }
}

fn parse_section(value: &str) -> Result<LifeModelSectionV2> {
    match value {
        "stable_preferences" => Ok(LifeModelSectionV2::StablePreferences),
        "collaboration_preferences" => Ok(LifeModelSectionV2::CollaborationPreferences),
        _ => bail!("invalid_lifemodel_learning_section"),
    }
}

fn observation_columns() -> &'static str {
    "id, workspace_ref, source_ref, source_digest, summary, section, value_json,
     explicitness, sensitivity, observed_at, expires_at"
}

fn candidate_columns() -> &'static str {
    "id, observation_id, workspace_ref, summary, section, value_json, status,
     explicitness, sensitivity, source_ref, created_at, updated_at, expires_at"
}

fn observation_from_row(row: &Row<'_>) -> rusqlite::Result<LifeModelLearningObservation> {
    Ok(LifeModelLearningObservation {
        id: row.get(0)?,
        workspace_ref: row.get(1)?,
        source_ref: row.get(2)?,
        source_digest: row.get(3)?,
        summary: row.get(4)?,
        section: parse_db_value(row.get::<_, String>(5)?, parse_section, 5)?,
        value: parse_json_value(row.get::<_, String>(6)?, 6)?,
        explicitness: parse_db_value(
            row.get::<_, String>(7)?,
            LifeModelLearningExplicitness::parse,
            7,
        )?,
        sensitivity: parse_db_value(
            row.get::<_, String>(8)?,
            LifeModelLearningSensitivity::parse,
            8,
        )?,
        observed_at: row.get(9)?,
        expires_at: row.get(10)?,
    })
}

fn candidate_from_row(row: &Row<'_>) -> rusqlite::Result<LifeModelLearningCandidate> {
    let observation_id = row.get::<_, String>(1)?;
    let source_ref = row.get::<_, String>(9)?;
    Ok(LifeModelLearningCandidate {
        id: row.get(0)?,
        workspace_ref: row.get(2)?,
        summary: row.get(3)?,
        section: parse_db_value(row.get::<_, String>(4)?, parse_section, 4)?,
        value: parse_json_value(row.get::<_, String>(5)?, 5)?,
        status: parse_db_value(
            row.get::<_, String>(6)?,
            LifeModelLearningCandidateStatus::parse,
            6,
        )?,
        explicitness: parse_db_value(
            row.get::<_, String>(7)?,
            LifeModelLearningExplicitness::parse,
            7,
        )?,
        sensitivity: parse_db_value(
            row.get::<_, String>(8)?,
            LifeModelLearningSensitivity::parse,
            8,
        )?,
        observation_ids: vec![observation_id],
        source_refs: vec![source_ref],
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        expires_at: row.get(12)?,
    })
}

fn parse_db_value<T>(
    value: String,
    parser: impl FnOnce(&str) -> Result<T>,
    column: usize,
) -> rusqlite::Result<T> {
    parser(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })
}

fn parse_json_value(value: String, column: usize) -> rusqlite::Result<LifeModelUserValueV2> {
    serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let hash = digest(&SHA256, bytes);
    let mut value = String::from("sha256:");
    for byte in hash.as_ref() {
        value.push_str(&format!("{byte:02x}"));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(workspace_ref: &str, source_ref: &str) -> LifeModelLearningCapture {
        LifeModelLearningCapture {
            workspace_ref: workspace_ref.into(),
            source_ref: source_ref.into(),
            source_digest: format!("sha256:{}", "a".repeat(64)),
            summary: "Prefer concise progress updates.".into(),
            section: LifeModelSectionV2::CollaborationPreferences,
            value: LifeModelUserValueV2::Statement {
                statement: "Prefer concise progress updates.".into(),
            },
            explicitness: LifeModelLearningExplicitness::ExplicitUserRequest,
            sensitivity: LifeModelLearningSensitivity::Internal,
            observed_at: "2026-08-08T08:00:00Z".into(),
            observation_expires_at: "2026-09-07T08:00:00Z".into(),
            candidate_expires_at: "2026-11-06T08:00:00Z".into(),
        }
    }

    #[test]
    fn explicit_capture_persists_candidate_without_proposal_or_lifemodel_credit() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let receipt = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();

        assert!(!receipt.replayed);
        assert!(!receipt.proposal_created);
        assert!(!receipt.canonical_life_model_changed);
        assert_eq!(
            store.list_active_candidates("workspace:one", 20).unwrap(),
            vec![receipt.candidate]
        );
    }

    #[test]
    fn exact_source_replay_does_not_duplicate_observation_or_candidate() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let first = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();
        let replay = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();

        assert!(replay.replayed);
        assert_eq!(first.observation.id, replay.observation.id);
        assert_eq!(first.candidate.id, replay.candidate.id);
        assert_eq!(
            store
                .list_active_candidates("workspace:one", 20)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn candidate_survives_store_reopen_without_gaining_proposal_or_lifemodel_credit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("life-model-learning.db");
        let candidate = {
            let store = LifeModelLearningStore::new(&path).unwrap();
            let receipt = store
                .capture_explicit_candidate(capture("workspace:one", "message:one"))
                .unwrap();
            assert!(!receipt.proposal_created);
            assert!(!receipt.canonical_life_model_changed);
            receipt.candidate
        };

        let reopened = LifeModelLearningStore::new(&path).unwrap();
        assert_eq!(
            reopened
                .list_active_candidates("workspace:one", 20)
                .unwrap(),
            vec![candidate.clone()]
        );
        drop(reopened);

        let read_only = LifeModelLearningStore::open_read_only_existing(&path).unwrap();
        assert_eq!(
            read_only
                .list_active_candidates("workspace:one", 20)
                .unwrap(),
            vec![candidate]
        );
    }

    #[test]
    fn workspace_reads_and_deletes_are_isolated() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let one = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();
        let two = store
            .capture_explicit_candidate(capture("workspace:two", "message:two"))
            .unwrap();

        assert_eq!(
            store.list_active_candidates("workspace:one", 20).unwrap(),
            vec![one.candidate.clone()]
        );
        assert!(!store
            .delete_candidate("workspace:two", &one.candidate.id)
            .unwrap());
        assert!(store
            .delete_candidate("workspace:one", &one.candidate.id)
            .unwrap());
        assert!(store
            .list_active_candidates("workspace:one", 20)
            .unwrap()
            .is_empty());
        assert_eq!(
            store.list_active_candidates("workspace:two", 20).unwrap(),
            vec![two.candidate]
        );
    }

    #[test]
    fn sensitive_or_unsupported_content_is_rejected_without_writes() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let mut unsupported = capture("workspace:one", "message:one");
        unsupported.section = LifeModelSectionV2::ImportantRelationships;
        unsupported.value = LifeModelUserValueV2::Relationship {
            person_label: "Private person".into(),
            relationship: "friend".into(),
            significance: "private".into(),
        };

        assert!(store.capture_explicit_candidate(unsupported).is_err());
        assert!(store
            .list_active_candidates("workspace:one", 20)
            .unwrap()
            .is_empty());
    }
}
