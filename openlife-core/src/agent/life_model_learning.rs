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
    PassiveInference,
}

impl LifeModelLearningExplicitness {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExplicitUserRequest => "explicit_user_request",
            Self::PassiveInference => "passive_inference",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "explicit_user_request" => Ok(Self::ExplicitUserRequest),
            "passive_inference" => Ok(Self::PassiveInference),
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
    Reviewable,
    Conflicted,
    Proposed,
    Rejected,
    Materialized,
    Expired,
}

impl LifeModelLearningCandidateStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accumulating => "accumulating",
            Self::Reviewable => "reviewable",
            Self::Conflicted => "conflicted",
            Self::Proposed => "proposed",
            Self::Rejected => "rejected",
            Self::Materialized => "materialized",
            Self::Expired => "expired",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "accumulating" => Ok(Self::Accumulating),
            "reviewable" => Ok(Self::Reviewable),
            "conflicted" => Ok(Self::Conflicted),
            "proposed" => Ok(Self::Proposed),
            "rejected" => Ok(Self::Rejected),
            "materialized" => Ok(Self::Materialized),
            "expired" => Ok(Self::Expired),
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
    /// A task or session boundary used to count independent passive support.
    pub independence_ref: String,
    pub summary: String,
    pub section: LifeModelSectionV2,
    pub value: LifeModelUserValueV2,
    /// A narrow semantic target. Different values for the same target conflict.
    pub target_key: String,
    /// User-controllable suggestion family, such as `stable_preferences`.
    pub suggestion_class: String,
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
    pub independence_ref: String,
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
    pub target_key: String,
    pub suggestion_class: String,
    pub support_count: usize,
    pub independent_support_count: usize,
    pub status: LifeModelLearningCandidateStatus,
    pub explicitness: LifeModelLearningExplicitness,
    pub sensitivity: LifeModelLearningSensitivity,
    pub observation_ids: Vec<String>,
    pub source_refs: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelLearningSuppressionKind {
    ExactCandidate,
    SuggestionClass,
}

impl LifeModelLearningSuppressionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactCandidate => "exact_candidate",
            Self::SuggestionClass => "suggestion_class",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelLearningDecisionReceipt {
    pub candidate_id: String,
    pub changed: bool,
    pub status: LifeModelLearningCandidateStatus,
    pub suppression_kind: Option<LifeModelLearningSuppressionKind>,
    pub content_scrubbed: bool,
    pub proposal_changed: bool,
    pub canonical_life_model_changed: bool,
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
                "life_model_learning_candidate_observations",
                "life_model_learning_suppressions",
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
                    ON life_model_learning_candidates(workspace_ref, status, updated_at DESC);
                 CREATE TABLE IF NOT EXISTS life_model_learning_candidate_observations (
                    candidate_id TEXT NOT NULL,
                    observation_id TEXT NOT NULL UNIQUE,
                    PRIMARY KEY(candidate_id, observation_id),
                    FOREIGN KEY(candidate_id) REFERENCES life_model_learning_candidates(id)
                        ON DELETE CASCADE,
                    FOREIGN KEY(observation_id) REFERENCES life_model_learning_observations(id)
                        ON DELETE CASCADE
                 ) WITHOUT ROWID;
                 CREATE TABLE IF NOT EXISTS life_model_learning_suppressions (
                    id TEXT PRIMARY KEY,
                    workspace_ref TEXT NOT NULL,
                    suppression_kind TEXT NOT NULL,
                    suppression_digest TEXT NOT NULL,
                    suggestion_class TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    UNIQUE(workspace_ref, suppression_kind, suppression_digest)
                 );
                 CREATE INDEX IF NOT EXISTS idx_lifemodel_learning_suppression_workspace
                    ON life_model_learning_suppressions(workspace_ref, suppression_kind);",
            )
            .context("initialize_lifemodel_learning_store")?;
        crate::sqlite_migration::ensure_column(
            &connection,
            "life_model_learning_observations",
            "independence_ref",
            "TEXT NOT NULL DEFAULT 'legacy:unknown'",
        )?;
        crate::sqlite_migration::ensure_column(
            &connection,
            "life_model_learning_observations",
            "body_scrubbed",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        for (column, definition) in [
            ("target_key", "TEXT NOT NULL DEFAULT 'legacy.unspecified'"),
            ("suggestion_class", "TEXT NOT NULL DEFAULT 'legacy'"),
            ("semantic_digest", "TEXT NOT NULL DEFAULT ''"),
            ("support_count", "INTEGER NOT NULL DEFAULT 1"),
            ("independent_support_count", "INTEGER NOT NULL DEFAULT 1"),
            ("body_scrubbed", "INTEGER NOT NULL DEFAULT 0"),
        ] {
            crate::sqlite_migration::ensure_column(
                &connection,
                "life_model_learning_candidates",
                column,
                definition,
            )?;
        }
        migrate_5_3a_rows(&connection)?;
        let store = Self {
            conn: Mutex::new(connection),
        };
        store.reconcile_expired(&Utc::now().to_rfc3339())?;
        Ok(store)
    }

    pub fn capture_explicit_candidate(
        &self,
        capture: LifeModelLearningCapture,
    ) -> Result<LifeModelLearningCaptureReceipt> {
        validate_capture(&capture)?;
        let value_json =
            serde_json::to_string(&capture.value).context("serialize_lifemodel_learning_value")?;
        let semantic_digest = semantic_digest(&capture, &value_json);
        let identity_digest = sha256_prefixed(
            format!(
                "{}\0{}\0{}\0{}\0{}\0{}",
                capture.workspace_ref,
                capture.source_ref,
                capture.source_digest,
                capture.independence_ref,
                section_name(capture.section),
                value_json
            )
            .as_bytes(),
        );
        let observation_id = format!("lmo_{}", &identity_digest[7..31]);
        let proposed_candidate_id = format!("lmc_{}", &semantic_digest[7..31]);

        let mut connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        let transaction = connection
            .transaction()
            .context("begin_lifemodel_learning_capture")?;
        if let Some((observation, candidate_id)) = transaction
            .query_row(
                &format!(
                    "SELECT {} FROM life_model_learning_observations WHERE identity_digest = ?1",
                    observation_columns()
                ),
                [&identity_digest],
                |row| {
                    let observation = observation_from_row(row)?;
                    Ok((observation, String::new()))
                },
            )
            .optional()
            .context("read_replayed_lifemodel_learning_observation")?
            .map(|(observation, _)| {
                let candidate_id = transaction
                    .query_row(
                        "SELECT candidate_id FROM life_model_learning_candidate_observations
                         WHERE observation_id = ?1",
                        [&observation.id],
                        |row| row.get::<_, String>(0),
                    )
                    .or_else(|_| {
                        transaction.query_row(
                            "SELECT id FROM life_model_learning_candidates WHERE observation_id = ?1",
                            [&observation.id],
                            |row| row.get::<_, String>(0),
                        )
                    })?;
                Ok::<_, rusqlite::Error>((observation, candidate_id))
            })
            .transpose()
            .context("resolve_replayed_lifemodel_learning_candidate")?
        {
            transaction
                .commit()
                .context("commit_replayed_lifemodel_learning_capture")?;
            drop(connection);
            let candidate = self
                .get_candidate(&candidate_id)?
                .ok_or_else(|| anyhow!("replayed_lifemodel_learning_candidate_missing"))?;
            return Ok(LifeModelLearningCaptureReceipt {
                observation,
                candidate,
                replayed: true,
                proposal_created: false,
                canonical_life_model_changed: false,
            });
        }

        let exact_suppressed = transaction
            .query_row(
                "SELECT 1 FROM life_model_learning_suppressions
                 WHERE workspace_ref = ?1 AND suppression_kind = 'exact_candidate'
                   AND suppression_digest = ?2 LIMIT 1",
                params![capture.workspace_ref, semantic_digest],
                |_| Ok(()),
            )
            .optional()
            .context("read_exact_lifemodel_learning_suppression")?
            .is_some();
        let class_digest = sha256_prefixed(capture.suggestion_class.as_bytes());
        let class_suppressed = transaction
            .query_row(
                "SELECT 1 FROM life_model_learning_suppressions
                 WHERE workspace_ref = ?1 AND suppression_kind = 'suggestion_class'
                   AND suppression_digest = ?2 LIMIT 1",
                params![capture.workspace_ref, class_digest],
                |_| Ok(()),
            )
            .optional()
            .context("read_class_lifemodel_learning_suppression")?
            .is_some();
        if exact_suppressed || class_suppressed {
            transaction
                .commit()
                .context("commit_suppressed_lifemodel_learning_capture")?;
            bail!("lifemodel_learning_candidate_suppressed");
        }

        transaction
            .execute(
                "INSERT INTO life_model_learning_observations (
                    id, identity_digest, workspace_ref, source_ref, source_digest,
                    independence_ref, summary,
                    section, value_json, explicitness, sensitivity, observed_at, expires_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    observation_id,
                    identity_digest,
                    capture.workspace_ref,
                    capture.source_ref,
                    capture.source_digest,
                    capture.independence_ref,
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
        let existing_candidate_id = transaction
            .query_row(
                "SELECT id FROM life_model_learning_candidates
                 WHERE workspace_ref = ?1 AND semantic_digest = ?2
                   AND status IN ('accumulating', 'reviewable', 'conflicted')
                 LIMIT 1",
                params![capture.workspace_ref, semantic_digest],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("find_lifemodel_learning_semantic_candidate")?;
        let candidate_id = existing_candidate_id.unwrap_or(proposed_candidate_id);
        if transaction
            .query_row(
                "SELECT 1 FROM life_model_learning_candidates WHERE id = ?1",
                [&candidate_id],
                |_| Ok(()),
            )
            .optional()?
            .is_none()
        {
            let status = match capture.explicitness {
                LifeModelLearningExplicitness::ExplicitUserRequest => {
                    LifeModelLearningCandidateStatus::Reviewable
                }
                LifeModelLearningExplicitness::PassiveInference => {
                    LifeModelLearningCandidateStatus::Accumulating
                }
            };
            transaction
                .execute(
                    "INSERT INTO life_model_learning_candidates (
                        id, observation_id, workspace_ref, summary, section, value_json, status,
                        explicitness, sensitivity, source_ref, created_at, updated_at, expires_at,
                        target_key, suggestion_class, semantic_digest, support_count,
                        independent_support_count, body_scrubbed
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, ?12,
                               ?13, ?14, ?15, 1, 1, 0)",
                    params![
                        candidate_id,
                        observation_id,
                        capture.workspace_ref,
                        capture.summary,
                        section_name(capture.section),
                        serde_json::to_string(&capture.value)?,
                        status.as_str(),
                        capture.explicitness.as_str(),
                        capture.sensitivity.as_str(),
                        capture.source_ref,
                        capture.observed_at,
                        capture.candidate_expires_at,
                        capture.target_key,
                        capture.suggestion_class,
                        semantic_digest,
                    ],
                )
                .context("insert_lifemodel_learning_candidate")?;
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO life_model_learning_candidate_observations
                    (candidate_id, observation_id) VALUES (?1, ?2)",
                params![candidate_id, observation_id],
            )
            .context("link_lifemodel_learning_candidate_observation")?;
        let (support_count, independent_support_count) = transaction
            .query_row(
                "SELECT COUNT(*), COUNT(DISTINCT o.independence_ref)
                 FROM life_model_learning_candidate_observations co
                 JOIN life_model_learning_observations o ON o.id = co.observation_id
                 WHERE co.candidate_id = ?1",
                [&candidate_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .context("count_lifemodel_learning_candidate_support")?;
        let current_status = transaction.query_row(
            "SELECT status FROM life_model_learning_candidates WHERE id = ?1",
            [&candidate_id],
            |row| row.get::<_, String>(0),
        )?;
        let next_status = if current_status == LifeModelLearningCandidateStatus::Conflicted.as_str()
        {
            LifeModelLearningCandidateStatus::Conflicted
        } else if capture.explicitness == LifeModelLearningExplicitness::ExplicitUserRequest
            || independent_support_count >= 2
        {
            LifeModelLearningCandidateStatus::Reviewable
        } else {
            LifeModelLearningCandidateStatus::Accumulating
        };
        transaction.execute(
            "UPDATE life_model_learning_candidates
             SET status = ?2, support_count = ?3, independent_support_count = ?4,
                 updated_at = ?5,
                 explicitness = CASE WHEN ?7 = 'explicit_user_request'
                                     THEN 'explicit_user_request' ELSE explicitness END,
                 expires_at = CASE WHEN expires_at < ?6 THEN ?6 ELSE expires_at END
             WHERE id = ?1",
            params![
                candidate_id,
                next_status.as_str(),
                support_count,
                independent_support_count,
                capture.observed_at,
                capture.candidate_expires_at,
                capture.explicitness.as_str(),
            ],
        )?;

        let conflicting_ids = {
            let mut statement = transaction.prepare(
                "SELECT id FROM life_model_learning_candidates
                 WHERE workspace_ref = ?1 AND target_key = ?2 AND semantic_digest <> ?3
                   AND status IN ('accumulating', 'reviewable', 'conflicted')",
            )?;
            let ids = statement
                .query_map(
                    params![capture.workspace_ref, capture.target_key, semantic_digest],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            ids
        };
        if !conflicting_ids.is_empty() {
            transaction.execute(
                "UPDATE life_model_learning_candidates SET status = 'conflicted', updated_at = ?2
                 WHERE id = ?1",
                params![candidate_id, capture.observed_at],
            )?;
            for conflicting_id in conflicting_ids {
                transaction.execute(
                    "UPDATE life_model_learning_candidates SET status = 'conflicted', updated_at = ?2
                     WHERE id = ?1",
                    params![conflicting_id, capture.observed_at],
                )?;
            }
        }
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
        let now = Utc::now().to_rfc3339();
        let mut statement = connection
            .prepare(
                "SELECT id FROM life_model_learning_candidates
                 WHERE workspace_ref = ?1
                   AND status IN ('accumulating', 'reviewable', 'conflicted')
                   AND body_scrubbed = 0
                   AND expires_at > ?2
                 ORDER BY updated_at DESC, id ASC LIMIT ?3",
            )
            .context("prepare_lifemodel_learning_candidate_list")?;
        let candidate_ids = statement
            .query_map(params![workspace_ref, now, limit as i64], |row| {
                row.get::<_, String>(0)
            })
            .context("query_lifemodel_learning_candidate_list")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("collect_lifemodel_learning_candidate_list")?;
        candidate_ids
            .iter()
            .map(|id| load_candidate(&connection, id))
            .collect()
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
        let target_key = transaction
            .query_row(
                "SELECT target_key FROM life_model_learning_candidates
                 WHERE id = ?1 AND workspace_ref = ?2",
                params![candidate_id, workspace_ref],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("read_lifemodel_learning_candidate_delete_owner")?;
        let Some(target_key) = target_key else {
            transaction
                .commit()
                .context("commit_missing_lifemodel_learning_candidate_delete")?;
            return Ok(false);
        };
        let observation_ids = {
            let mut statement = transaction.prepare(
                "SELECT observation_id FROM life_model_learning_candidate_observations
                 WHERE candidate_id = ?1",
            )?;
            let values = statement
                .query_map([candidate_id], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            values
        };
        transaction
            .execute(
                "DELETE FROM life_model_learning_candidates WHERE id = ?1 AND workspace_ref = ?2",
                params![candidate_id, workspace_ref],
            )
            .context("delete_lifemodel_learning_candidate")?;
        for observation_id in observation_ids {
            transaction
                .execute(
                    "DELETE FROM life_model_learning_observations WHERE id = ?1 AND workspace_ref = ?2",
                    params![observation_id, workspace_ref],
                )
                .context("delete_lifemodel_learning_observation")?;
        }
        recompute_target_status(
            &transaction,
            workspace_ref,
            &target_key,
            &Utc::now().to_rfc3339(),
        )?;
        transaction
            .commit()
            .context("commit_lifemodel_learning_candidate_delete")?;
        Ok(true)
    }

    pub fn reject_and_suppress_candidate(
        &self,
        workspace_ref: &str,
        candidate_id: &str,
        now: &str,
    ) -> Result<LifeModelLearningDecisionReceipt> {
        self.scrub_and_suppress(
            workspace_ref,
            candidate_id,
            now,
            LifeModelLearningCandidateStatus::Rejected,
            LifeModelLearningSuppressionKind::ExactCandidate,
        )
    }

    pub fn pause_suggestion_class(
        &self,
        workspace_ref: &str,
        candidate_id: &str,
        now: &str,
    ) -> Result<LifeModelLearningDecisionReceipt> {
        self.scrub_and_suppress(
            workspace_ref,
            candidate_id,
            now,
            LifeModelLearningCandidateStatus::Rejected,
            LifeModelLearningSuppressionKind::SuggestionClass,
        )
    }

    pub fn reconcile_expired(&self, now: &str) -> Result<usize> {
        let now = parse_time(now, "invalid_lifemodel_learning_reconcile_time")?.to_rfc3339();
        let mut connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE life_model_learning_observations
             SET summary = '', value_json = '{\"statement\":\"\"}', body_scrubbed = 1
             WHERE body_scrubbed = 0 AND expires_at <= ?1",
            [&now],
        )?;
        let expired_candidates = {
            let mut statement = transaction.prepare(
                "SELECT id, workspace_ref, semantic_digest, suggestion_class, target_key
                 FROM life_model_learning_candidates
                 WHERE body_scrubbed = 0 AND expires_at <= ?1
                   AND status IN ('accumulating', 'reviewable', 'conflicted')",
            )?;
            let values = statement
                .query_map([&now], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            values
        };
        for (candidate_id, workspace_ref, semantic_digest, suggestion_class, _) in
            &expired_candidates
        {
            let suppression_identity = sha256_prefixed(
                format!("{workspace_ref}\0exact_candidate\0{semantic_digest}").as_bytes(),
            );
            transaction.execute(
                "INSERT OR IGNORE INTO life_model_learning_suppressions
                    (id, workspace_ref, suppression_kind, suppression_digest, suggestion_class, created_at)
                 VALUES (?1, ?2, 'exact_candidate', ?3, ?4, ?5)",
                params![
                    format!("lms_{}", &suppression_identity[7..31]),
                    workspace_ref,
                    semantic_digest,
                    suggestion_class,
                    now,
                ],
            )?;
            scrub_candidate_body(
                &transaction,
                candidate_id,
                LifeModelLearningCandidateStatus::Expired,
                &now,
            )?;
        }
        for (_, workspace_ref, _, _, target_key) in &expired_candidates {
            recompute_target_status(&transaction, workspace_ref, target_key, &now)?;
        }
        transaction.commit()?;
        Ok(expired_candidates.len())
    }

    fn scrub_and_suppress(
        &self,
        workspace_ref: &str,
        candidate_id: &str,
        now: &str,
        status: LifeModelLearningCandidateStatus,
        kind: LifeModelLearningSuppressionKind,
    ) -> Result<LifeModelLearningDecisionReceipt> {
        validate_ref(workspace_ref, "invalid_lifemodel_learning_workspace_ref")?;
        validate_ref(candidate_id, "invalid_lifemodel_learning_candidate_id")?;
        let now = parse_time(now, "invalid_lifemodel_learning_decision_time")?.to_rfc3339();
        let mut connection = self
            .conn
            .lock()
            .map_err(|error| anyhow!("lifemodel_learning_store_lock_poisoned:{error}"))?;
        let transaction = connection.transaction()?;
        let owner = transaction
            .query_row(
                "SELECT semantic_digest, suggestion_class, target_key
                 FROM life_model_learning_candidates
                 WHERE id = ?1 AND workspace_ref = ?2 AND body_scrubbed = 0",
                params![candidate_id, workspace_ref],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((semantic_digest, suggestion_class, target_key)) = owner else {
            transaction.commit()?;
            return Ok(LifeModelLearningDecisionReceipt {
                candidate_id: candidate_id.into(),
                changed: false,
                status,
                suppression_kind: None,
                content_scrubbed: false,
                proposal_changed: false,
                canonical_life_model_changed: false,
            });
        };
        let suppression_digest = match kind {
            LifeModelLearningSuppressionKind::ExactCandidate => semantic_digest,
            LifeModelLearningSuppressionKind::SuggestionClass => {
                sha256_prefixed(suggestion_class.as_bytes())
            }
        };
        let suppression_identity = sha256_prefixed(
            format!("{workspace_ref}\0{}\0{suppression_digest}", kind.as_str()).as_bytes(),
        );
        let suppression_id = format!("lms_{}", &suppression_identity[7..31]);
        transaction.execute(
            "INSERT OR IGNORE INTO life_model_learning_suppressions
                (id, workspace_ref, suppression_kind, suppression_digest, suggestion_class, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                suppression_id,
                workspace_ref,
                kind.as_str(),
                suppression_digest,
                suggestion_class,
                now,
            ],
        )?;
        let affected = if kind == LifeModelLearningSuppressionKind::SuggestionClass {
            let mut statement = transaction.prepare(
                "SELECT id, target_key FROM life_model_learning_candidates
                 WHERE workspace_ref = ?1 AND suggestion_class = ?2 AND body_scrubbed = 0
                   AND status IN ('accumulating', 'reviewable', 'conflicted')",
            )?;
            let values = statement
                .query_map(params![workspace_ref, suggestion_class], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            values
        } else {
            vec![(candidate_id.to_string(), target_key)]
        };
        for (affected_id, _) in &affected {
            scrub_candidate_body(&transaction, affected_id, status, &now)?;
        }
        for (_, affected_target) in &affected {
            recompute_target_status(&transaction, workspace_ref, affected_target, &now)?;
        }
        transaction.commit()?;
        Ok(LifeModelLearningDecisionReceipt {
            candidate_id: candidate_id.into(),
            changed: true,
            status,
            suppression_kind: Some(kind),
            content_scrubbed: true,
            proposal_changed: false,
            canonical_life_model_changed: false,
        })
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
        let exists = connection
            .query_row(
                "SELECT 1 FROM life_model_learning_candidates WHERE id = ?1 AND body_scrubbed = 0",
                [id],
                |_| Ok(()),
            )
            .optional()?;
        exists
            .map(|_| load_candidate(&connection, id))
            .transpose()
            .context("read_lifemodel_learning_candidate")
    }
}

fn validate_capture(capture: &LifeModelLearningCapture) -> Result<()> {
    validate_ref(
        &capture.workspace_ref,
        "invalid_lifemodel_learning_workspace_ref",
    )?;
    validate_ref(&capture.source_ref, "invalid_lifemodel_learning_source_ref")?;
    validate_ref(
        &capture.independence_ref,
        "invalid_lifemodel_learning_independence_ref",
    )?;
    validate_ref(&capture.target_key, "invalid_lifemodel_learning_target_key")?;
    validate_ref(
        &capture.suggestion_class,
        "invalid_lifemodel_learning_suggestion_class",
    )?;
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
    "id, workspace_ref, source_ref, source_digest, independence_ref, summary, section, value_json,
     explicitness, sensitivity, observed_at, expires_at"
}

fn candidate_columns() -> &'static str {
    "id, observation_id, workspace_ref, summary, section, value_json, status,
     explicitness, sensitivity, source_ref, created_at, updated_at, expires_at,
     target_key, suggestion_class, support_count, independent_support_count"
}

fn observation_from_row(row: &Row<'_>) -> rusqlite::Result<LifeModelLearningObservation> {
    Ok(LifeModelLearningObservation {
        id: row.get(0)?,
        workspace_ref: row.get(1)?,
        source_ref: row.get(2)?,
        source_digest: row.get(3)?,
        independence_ref: row.get(4)?,
        summary: row.get(5)?,
        section: parse_db_value(row.get::<_, String>(6)?, parse_section, 6)?,
        value: parse_json_value(row.get::<_, String>(7)?, 7)?,
        explicitness: parse_db_value(
            row.get::<_, String>(8)?,
            LifeModelLearningExplicitness::parse,
            8,
        )?,
        sensitivity: parse_db_value(
            row.get::<_, String>(9)?,
            LifeModelLearningSensitivity::parse,
            9,
        )?,
        observed_at: row.get(10)?,
        expires_at: row.get(11)?,
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
        target_key: row.get(13)?,
        suggestion_class: row.get(14)?,
        support_count: row.get::<_, i64>(15)?.max(0) as usize,
        independent_support_count: row.get::<_, i64>(16)?.max(0) as usize,
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

fn load_candidate(connection: &Connection, id: &str) -> Result<LifeModelLearningCandidate> {
    let mut candidate = connection
        .query_row(
            &format!(
                "SELECT {} FROM life_model_learning_candidates WHERE id = ?1 AND body_scrubbed = 0",
                candidate_columns()
            ),
            [id],
            candidate_from_row,
        )
        .context("load_lifemodel_learning_candidate")?;
    let mut statement = connection.prepare(
        "SELECT o.id, o.source_ref
         FROM life_model_learning_candidate_observations co
         JOIN life_model_learning_observations o ON o.id = co.observation_id
         WHERE co.candidate_id = ?1
         ORDER BY o.observed_at ASC, o.id ASC",
    )?;
    let pairs = statement
        .query_map([id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !pairs.is_empty() {
        candidate.observation_ids = pairs.iter().map(|pair| pair.0.clone()).collect();
        candidate.source_refs = pairs.into_iter().map(|pair| pair.1).collect();
    }
    Ok(candidate)
}

fn scrub_candidate_body(
    transaction: &rusqlite::Transaction<'_>,
    candidate_id: &str,
    status: LifeModelLearningCandidateStatus,
    now: &str,
) -> Result<()> {
    transaction.execute(
        "UPDATE life_model_learning_candidates
         SET summary = '', value_json = '{\"statement\":\"\"}', status = ?2,
             body_scrubbed = 1, updated_at = ?3
         WHERE id = ?1",
        params![candidate_id, status.as_str(), now],
    )?;
    transaction.execute(
        "UPDATE life_model_learning_observations
         SET summary = '', value_json = '{\"statement\":\"\"}', body_scrubbed = 1
         WHERE id IN (
            SELECT observation_id FROM life_model_learning_candidate_observations
            WHERE candidate_id = ?1
         )",
        [candidate_id],
    )?;
    Ok(())
}

fn recompute_target_status(
    transaction: &rusqlite::Transaction<'_>,
    workspace_ref: &str,
    target_key: &str,
    now: &str,
) -> Result<()> {
    let active = {
        let mut statement = transaction.prepare(
            "SELECT id, explicitness, independent_support_count, semantic_digest
             FROM life_model_learning_candidates
             WHERE workspace_ref = ?1 AND target_key = ?2 AND body_scrubbed = 0
               AND status IN ('accumulating', 'reviewable', 'conflicted')",
        )?;
        let values = statement
            .query_map(params![workspace_ref, target_key], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        values
    };
    let semantic_count = active
        .iter()
        .map(|(_, _, _, digest)| digest)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    for (candidate_id, explicitness, independent_support_count, _) in active {
        let status = if semantic_count > 1 {
            LifeModelLearningCandidateStatus::Conflicted
        } else if explicitness == LifeModelLearningExplicitness::ExplicitUserRequest.as_str()
            || independent_support_count >= 2
        {
            LifeModelLearningCandidateStatus::Reviewable
        } else {
            LifeModelLearningCandidateStatus::Accumulating
        };
        transaction.execute(
            "UPDATE life_model_learning_candidates SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![candidate_id, status.as_str(), now],
        )?;
    }
    Ok(())
}

fn semantic_digest(capture: &LifeModelLearningCapture, value_json: &str) -> String {
    sha256_prefixed(
        format!(
            "{}\0{}\0{}\0{}",
            capture.workspace_ref,
            capture.target_key,
            section_name(capture.section),
            normalize_semantic_value(value_json)
        )
        .as_bytes(),
    )
}

fn normalize_semantic_value(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn migrate_5_3a_rows(connection: &Connection) -> Result<()> {
    connection.execute(
        "INSERT OR IGNORE INTO life_model_learning_candidate_observations
            (candidate_id, observation_id)
         SELECT id, observation_id FROM life_model_learning_candidates",
        [],
    )?;
    let legacy_rows = {
        let mut statement = connection.prepare(
            "SELECT id, workspace_ref, section, value_json
             FROM life_model_learning_candidates
             WHERE semantic_digest = '' OR target_key = 'legacy.unspecified'",
        )?;
        let values = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        values
    };
    for (id, workspace_ref, section, value_json) in legacy_rows {
        let suggestion_class = section.clone();
        let target_key = if section == "collaboration_preferences" {
            "collaboration_preferences.communication_style".to_string()
        } else {
            let digest = sha256_prefixed(normalize_semantic_value(&value_json).as_bytes());
            format!("stable_preferences.claim:{}", &digest[7..23])
        };
        let semantic_digest = sha256_prefixed(
            format!(
                "{workspace_ref}\0{target_key}\0{section}\0{}",
                normalize_semantic_value(&value_json)
            )
            .as_bytes(),
        );
        connection.execute(
            "UPDATE life_model_learning_candidates
             SET target_key = ?2, suggestion_class = ?3, semantic_digest = ?4,
                 status = CASE WHEN explicitness = 'explicit_user_request' AND status = 'accumulating'
                               THEN 'reviewable' ELSE status END
             WHERE id = ?1",
            params![id, target_key, suggestion_class, semantic_digest],
        )?;
    }
    connection.execute(
        "UPDATE life_model_learning_observations
         SET independence_ref = source_ref WHERE independence_ref = 'legacy:unknown'",
        [],
    )?;
    Ok(())
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
            independence_ref: source_ref.into(),
            summary: "Prefer concise progress updates.".into(),
            section: LifeModelSectionV2::CollaborationPreferences,
            value: LifeModelUserValueV2::Statement {
                statement: "Prefer concise progress updates.".into(),
            },
            target_key: "collaboration_preferences.communication_style".into(),
            suggestion_class: "collaboration_preferences".into(),
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
    fn existing_5_3a_rows_migrate_without_losing_candidate_body_or_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("life-model-learning.db");
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "PRAGMA foreign_keys=ON;
                     CREATE TABLE life_model_learning_observations (
                        id TEXT PRIMARY KEY, identity_digest TEXT NOT NULL UNIQUE,
                        workspace_ref TEXT NOT NULL, source_ref TEXT NOT NULL,
                        source_digest TEXT NOT NULL, summary TEXT NOT NULL,
                        section TEXT NOT NULL, value_json TEXT NOT NULL,
                        explicitness TEXT NOT NULL, sensitivity TEXT NOT NULL,
                        observed_at TEXT NOT NULL, expires_at TEXT NOT NULL
                     );
                     CREATE TABLE life_model_learning_candidates (
                        id TEXT PRIMARY KEY, observation_id TEXT NOT NULL UNIQUE,
                        workspace_ref TEXT NOT NULL, summary TEXT NOT NULL,
                        section TEXT NOT NULL, value_json TEXT NOT NULL,
                        status TEXT NOT NULL, explicitness TEXT NOT NULL,
                        sensitivity TEXT NOT NULL, source_ref TEXT NOT NULL,
                        created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                        expires_at TEXT NOT NULL,
                        FOREIGN KEY(observation_id) REFERENCES life_model_learning_observations(id)
                     );
                     INSERT INTO life_model_learning_observations VALUES (
                        'lmo_old', 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                        'workspace:one', 'message:old',
                        'sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
                        'Prefer concise updates.', 'collaboration_preferences',
                        '{\"kind\":\"statement\",\"value\":{\"statement\":\"Prefer concise updates.\"}}',
                        'explicit_user_request', 'internal', '2026-08-08T08:00:00Z',
                        '2026-09-07T08:00:00Z'
                     );
                     INSERT INTO life_model_learning_candidates VALUES (
                        'lmc_old', 'lmo_old', 'workspace:one', 'Prefer concise updates.',
                        'collaboration_preferences',
                        '{\"kind\":\"statement\",\"value\":{\"statement\":\"Prefer concise updates.\"}}',
                        'accumulating', 'explicit_user_request', 'internal', 'message:old',
                        '2026-08-08T08:00:00Z', '2026-08-08T08:00:00Z',
                        '2026-11-06T08:00:00Z'
                     );",
                )
                .unwrap();
        }

        let store = LifeModelLearningStore::new(&path).unwrap();
        let candidates = store.list_active_candidates("workspace:one", 20).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "lmc_old");
        assert_eq!(
            candidates[0].status,
            LifeModelLearningCandidateStatus::Reviewable
        );
        assert_eq!(candidates[0].source_refs, vec!["message:old"]);
        assert_eq!(
            candidates[0].target_key,
            "collaboration_preferences.communication_style"
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
    fn repeated_observations_merge_and_passive_support_requires_independent_boundaries() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let mut first = capture("workspace:one", "message:one");
        first.explicitness = LifeModelLearningExplicitness::PassiveInference;
        first.independence_ref = "session:one".into();
        let first_receipt = store.capture_explicit_candidate(first.clone()).unwrap();
        assert_eq!(
            first_receipt.candidate.status,
            LifeModelLearningCandidateStatus::Accumulating
        );

        let mut same_session = first.clone();
        same_session.source_ref = "message:two".into();
        same_session.source_digest = format!("sha256:{}", "b".repeat(64));
        let same_session_receipt = store
            .capture_explicit_candidate(same_session.clone())
            .unwrap();
        assert_eq!(same_session_receipt.candidate.support_count, 2);
        assert_eq!(same_session_receipt.candidate.independent_support_count, 1);
        assert_eq!(
            same_session_receipt.candidate.status,
            LifeModelLearningCandidateStatus::Accumulating
        );

        same_session.source_ref = "message:three".into();
        same_session.source_digest = format!("sha256:{}", "c".repeat(64));
        same_session.independence_ref = "session:two".into();
        let independent = store.capture_explicit_candidate(same_session).unwrap();
        assert_eq!(independent.candidate.support_count, 3);
        assert_eq!(independent.candidate.independent_support_count, 2);
        assert_eq!(
            independent.candidate.status,
            LifeModelLearningCandidateStatus::Reviewable
        );
        assert_eq!(independent.candidate.observation_ids.len(), 3);
    }

    #[test]
    fn conflicting_values_for_same_narrow_target_block_review() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let first = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();
        let mut contrary = capture("workspace:one", "message:two");
        contrary.source_digest = format!("sha256:{}", "b".repeat(64));
        contrary.independence_ref = "message:two".into();
        contrary.summary = "Prefer detailed progress updates.".into();
        contrary.value = LifeModelUserValueV2::Statement {
            statement: "Prefer detailed progress updates.".into(),
        };
        let second = store.capture_explicit_candidate(contrary).unwrap();

        assert_ne!(first.candidate.id, second.candidate.id);
        let active = store.list_active_candidates("workspace:one", 20).unwrap();
        assert_eq!(active.len(), 2);
        assert!(active
            .iter()
            .all(|candidate| { candidate.status == LifeModelLearningCandidateStatus::Conflicted }));
        store
            .reject_and_suppress_candidate(
                "workspace:one",
                &second.candidate.id,
                "2026-08-09T08:00:00Z",
            )
            .unwrap();
        let resolved = store.list_active_candidates("workspace:one", 20).unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, first.candidate.id);
        assert_eq!(
            resolved[0].status,
            LifeModelLearningCandidateStatus::Reviewable
        );
    }

    #[test]
    fn rejection_scrubs_bodies_and_suppresses_semantic_recurrence() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let original_capture = capture("workspace:one", "message:one");
        let receipt = store
            .capture_explicit_candidate(original_capture.clone())
            .unwrap();
        let decision = store
            .reject_and_suppress_candidate(
                "workspace:one",
                &receipt.candidate.id,
                "2026-08-09T08:00:00Z",
            )
            .unwrap();
        assert!(decision.changed);
        assert!(decision.content_scrubbed);
        assert_eq!(
            decision.suppression_kind,
            Some(LifeModelLearningSuppressionKind::ExactCandidate)
        );
        assert!(store
            .list_active_candidates("workspace:one", 20)
            .unwrap()
            .is_empty());
        let connection = store.conn.lock().unwrap();
        let (candidate_summary, candidate_value, observation_summary): (String, String, String) =
            connection
                .query_row(
                    "SELECT c.summary, c.value_json, o.summary
                     FROM life_model_learning_candidates c
                     JOIN life_model_learning_candidate_observations co ON co.candidate_id = c.id
                     JOIN life_model_learning_observations o ON o.id = co.observation_id
                     WHERE c.id = ?1",
                    [&receipt.candidate.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .unwrap();
        assert!(candidate_summary.is_empty());
        assert_eq!(candidate_value, "{\"statement\":\"\"}");
        assert!(observation_summary.is_empty());
        drop(connection);

        let mut recurrence = original_capture;
        recurrence.source_ref = "message:later".into();
        recurrence.source_digest = format!("sha256:{}", "d".repeat(64));
        recurrence.independence_ref = "message:later".into();
        assert!(store.capture_explicit_candidate(recurrence).is_err());
    }

    #[test]
    fn pausing_a_suggestion_class_is_workspace_scoped() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let receipt = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();
        let mut existing_other_value = capture("workspace:one", "message:existing-two");
        existing_other_value.source_digest = format!("sha256:{}", "f".repeat(64));
        existing_other_value.independence_ref = "message:existing-two".into();
        existing_other_value.summary = "Prefer detailed progress updates.".into();
        existing_other_value.value = LifeModelUserValueV2::Statement {
            statement: existing_other_value.summary.clone(),
        };
        store
            .capture_explicit_candidate(existing_other_value)
            .unwrap();
        store
            .pause_suggestion_class(
                "workspace:one",
                &receipt.candidate.id,
                "2026-08-09T08:00:00Z",
            )
            .unwrap();
        assert!(store
            .list_active_candidates("workspace:one", 20)
            .unwrap()
            .is_empty());

        let mut another_value = capture("workspace:one", "message:two");
        another_value.source_digest = format!("sha256:{}", "e".repeat(64));
        another_value.summary = "Prefer detailed progress updates.".into();
        another_value.value = LifeModelUserValueV2::Statement {
            statement: another_value.summary.clone(),
        };
        assert!(store
            .capture_explicit_candidate(another_value.clone())
            .is_err());
        another_value.workspace_ref = "workspace:two".into();
        assert!(store.capture_explicit_candidate(another_value).is_ok());
    }

    #[test]
    fn expiry_scrubs_candidate_and_observation_bodies() {
        let store = LifeModelLearningStore::new_in_memory().unwrap();
        let receipt = store
            .capture_explicit_candidate(capture("workspace:one", "message:one"))
            .unwrap();
        assert_eq!(store.reconcile_expired("2027-01-01T00:00:00Z").unwrap(), 1);
        assert!(store
            .list_active_candidates("workspace:one", 20)
            .unwrap()
            .is_empty());
        let connection = store.conn.lock().unwrap();
        let (status, body_scrubbed, summary): (String, i64, String) = connection
            .query_row(
                "SELECT status, body_scrubbed, summary FROM life_model_learning_candidates WHERE id = ?1",
                [&receipt.candidate.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(status, "expired");
        assert_eq!(body_scrubbed, 1);
        assert!(summary.is_empty());
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
