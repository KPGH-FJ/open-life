use crate::agent::types::{AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::review_workflow::ProposalTerminalRelationStorageWriteProof;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactEffectState {
    Prepared,
    Staged,
    Confirmed,
    FailedBeforeEffect,
    Unknown,
}

impl ArtifactEffectState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Staged => "staged",
            Self::Confirmed => "confirmed",
            Self::FailedBeforeEffect => "failed_before_effect",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "prepared" => Ok(Self::Prepared),
            "staged" => Ok(Self::Staged),
            "confirmed" => Ok(Self::Confirmed),
            "failed_before_effect" => Ok(Self::FailedBeforeEffect),
            "unknown" => Ok(Self::Unknown),
            other => anyhow::bail!("unsupported artifact effect state: {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEffectRecord {
    pub proposal_id: String,
    pub dispatch_claim_id: String,
    pub proposal_snapshot_digest: String,
    pub target_reference_digest: String,
    pub content_digest: String,
    pub byte_size: u64,
    pub media_type: String,
    pub state: ArtifactEffectState,
    pub observed_content_digest: Option<String>,
    pub error_code: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOwnerOriginBinding {
    proposal_id: String,
    task_session_id: String,
    run_id: String,
    epoch_id: String,
    epoch_generation: u64,
    admission_id: String,
    canonical_user_message_ref: String,
    canonical_user_message_digest: String,
    canonical_store_identity: String,
}

impl TerminalOwnerOriginBinding {
    pub fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    pub fn task_session_id(&self) -> &str {
        &self.task_session_id
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn epoch_id(&self) -> &str {
        &self.epoch_id
    }

    pub fn epoch_generation(&self) -> u64 {
        self.epoch_generation
    }

    pub fn admission_id(&self) -> &str {
        &self.admission_id
    }

    pub fn canonical_user_message_ref(&self) -> &str {
        &self.canonical_user_message_ref
    }

    pub fn canonical_user_message_digest(&self) -> &str {
        &self.canonical_user_message_digest
    }

    pub fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }
}

/// Persisted lifecycle relationship between a Review item and its immutable
/// terminal-owner origin. Proposal type cannot stand in for this contract:
/// Memory review, effect approval, and action-resume permission have distinct
/// consequences for the originating turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalTerminalRelationKind {
    NonBlockingSuccessor,
    EffectBlockingPrerequisite,
    ActionResumePrerequisite,
    LegacyUnclassified,
}

impl ProposalTerminalRelationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NonBlockingSuccessor => "non_blocking_successor",
            Self::EffectBlockingPrerequisite => "effect_blocking_prerequisite",
            Self::ActionResumePrerequisite => "action_resume_prerequisite",
            Self::LegacyUnclassified => "legacy_unclassified",
        }
    }

    fn projection_target(self) -> Option<&'static str> {
        match self {
            Self::NonBlockingSuccessor => Some("agent_run_review_link.non_blocking"),
            Self::EffectBlockingPrerequisite => Some("agent_run_review_link.effect_blocking"),
            Self::ActionResumePrerequisite => Some("agent_run_review_link.action_resume"),
            Self::LegacyUnclassified => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProposalTerminalRelationProjectionAuthority {
    VerifiedByProposalStoreOutbox,
}

/// Opaque, metadata-only projection input loaded from ProposalStore after the
/// canonical Proposal origin, typed relation, event, and exact delivery target
/// have been cross-validated. It contains no Proposal or conversation body.
#[derive(Debug)]
pub struct ProposalTerminalRelationProjectionProof {
    proposal_id: String,
    task_session_id: String,
    run_id: String,
    epoch_id: String,
    epoch_generation: u64,
    admission_id: String,
    canonical_user_message_ref: String,
    canonical_user_message_digest: String,
    canonical_store_identity: String,
    relation_kind: ProposalTerminalRelationKind,
    relation_digest: String,
    target_binding_digest: String,
    agent_run_store_identity_digest: String,
    target_owner_revision: u64,
    target_status_at_issue: crate::agent::AgentRunStatus,
    source_outbox_event_id: String,
    projection_target: String,
    authority: ProposalTerminalRelationProjectionAuthority,
}

impl ProposalTerminalRelationProjectionProof {
    pub fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    pub fn task_session_id(&self) -> &str {
        &self.task_session_id
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn epoch_id(&self) -> &str {
        &self.epoch_id
    }

    pub fn epoch_generation(&self) -> u64 {
        self.epoch_generation
    }

    pub fn admission_id(&self) -> &str {
        &self.admission_id
    }

    pub fn canonical_user_message_ref(&self) -> &str {
        &self.canonical_user_message_ref
    }

    pub fn canonical_user_message_digest(&self) -> &str {
        &self.canonical_user_message_digest
    }

    pub fn canonical_store_identity(&self) -> &str {
        &self.canonical_store_identity
    }

    pub fn relation_kind(&self) -> ProposalTerminalRelationKind {
        self.relation_kind
    }

    pub fn relation_digest(&self) -> &str {
        &self.relation_digest
    }

    pub fn target_binding_digest(&self) -> &str {
        &self.target_binding_digest
    }

    pub fn agent_run_store_identity_digest(&self) -> &str {
        &self.agent_run_store_identity_digest
    }

    pub fn target_owner_revision(&self) -> u64 {
        self.target_owner_revision
    }

    pub fn target_status_at_issue(&self) -> crate::agent::AgentRunStatus {
        self.target_status_at_issue
    }

    pub fn source_outbox_event_id(&self) -> &str {
        &self.source_outbox_event_id
    }

    pub fn projection_target(&self) -> &str {
        &self.projection_target
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let target = ProposalTerminalRelationTargetContract {
            target_binding_digest: &self.target_binding_digest,
            agent_run_store_identity_digest: &self.agent_run_store_identity_digest,
            target_owner_revision: self.target_owner_revision,
            target_status_at_issue: self.target_status_at_issue,
        };
        target.validate_for(self.relation_kind)?;
        let expected_relation_digest = proposal_terminal_relation_digest(
            &self.proposal_id,
            self.relation_kind,
            &self.task_session_id,
            &self.run_id,
            &self.epoch_id,
            self.epoch_generation,
            &self.admission_id,
            &self.canonical_user_message_ref,
            &self.canonical_user_message_digest,
            &self.canonical_store_identity,
            Some(target),
        )?;
        if self.authority
            != ProposalTerminalRelationProjectionAuthority::VerifiedByProposalStoreOutbox
            || self.proposal_id.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.task_session_id.trim().is_empty()
            || self.epoch_id.trim().is_empty()
            || self.epoch_generation == 0
            || self.admission_id.trim().is_empty()
            || self.relation_digest != expected_relation_digest
            || self.source_outbox_event_id.trim().is_empty()
            || self.relation_kind.projection_target() != Some(self.projection_target.as_str())
        {
            anyhow::bail!("proposal_terminal_relation_projection_proof_invalid");
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn replace_source_outbox_event_id_for_test(&mut self, event_id: &str) {
        self.source_outbox_event_id = event_id.to_string();
    }
}

#[cfg(test)]
pub(super) fn proposal_terminal_relation_projection_fixture(
    origin: &crate::agent::TerminalOwnerReviewOriginProof,
    target: &crate::agent::store::AgentRunTerminalRelationTargetIntentAdmission,
    proposal_id: &str,
    relation_kind: ProposalTerminalRelationKind,
) -> ProposalTerminalRelationProjectionProof {
    let target_contract = ProposalTerminalRelationTargetContract {
        target_binding_digest: target.target_binding_digest(),
        agent_run_store_identity_digest: target.agent_run_store_identity_digest(),
        target_owner_revision: target.owner_revision(),
        target_status_at_issue: target.status_at_issue(),
    };
    let relation_digest = proposal_terminal_relation_digest(
        proposal_id,
        relation_kind,
        origin.task_session_id(),
        origin.run_id(),
        origin.epoch_id(),
        origin.epoch_generation(),
        origin.admission_id(),
        origin.canonical_user_message_ref(),
        origin.canonical_user_message_digest(),
        origin.canonical_store_identity(),
        Some(target_contract),
    )
    .expect("test proposal terminal relation digest");
    ProposalTerminalRelationProjectionProof {
        proposal_id: proposal_id.to_string(),
        task_session_id: origin.task_session_id().to_string(),
        run_id: origin.run_id().to_string(),
        epoch_id: origin.epoch_id().to_string(),
        epoch_generation: origin.epoch_generation(),
        admission_id: origin.admission_id().to_string(),
        canonical_user_message_ref: origin.canonical_user_message_ref().to_string(),
        canonical_user_message_digest: origin.canonical_user_message_digest().to_string(),
        canonical_store_identity: origin.canonical_store_identity().to_string(),
        relation_kind,
        relation_digest,
        target_binding_digest: target.target_binding_digest().to_string(),
        agent_run_store_identity_digest: target.agent_run_store_identity_digest().to_string(),
        target_owner_revision: target.owner_revision(),
        target_status_at_issue: target.status_at_issue(),
        source_outbox_event_id: format!("outbox:{}", uuid::Uuid::new_v4()),
        projection_target: relation_kind
            .projection_target()
            .expect("test typed relation target")
            .to_string(),
        authority: ProposalTerminalRelationProjectionAuthority::VerifiedByProposalStoreOutbox,
    }
}

#[derive(Debug, Clone, Copy)]
struct ProposalTerminalRelationTargetContract<'a> {
    target_binding_digest: &'a str,
    agent_run_store_identity_digest: &'a str,
    target_owner_revision: u64,
    target_status_at_issue: crate::agent::AgentRunStatus,
}

impl ProposalTerminalRelationTargetContract<'_> {
    fn validate_for(self, relation_kind: ProposalTerminalRelationKind) -> Result<()> {
        let is_sha256 = |value: &str| {
            value.strip_prefix("sha256:").is_some_and(|hex| {
                hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        };
        let status_is_compatible = match relation_kind {
            ProposalTerminalRelationKind::NonBlockingSuccessor => matches!(
                self.target_status_at_issue,
                crate::agent::AgentRunStatus::Running | crate::agent::AgentRunStatus::Completed
            ),
            ProposalTerminalRelationKind::EffectBlockingPrerequisite
            | ProposalTerminalRelationKind::ActionResumePrerequisite => matches!(
                self.target_status_at_issue,
                crate::agent::AgentRunStatus::Running
                    | crate::agent::AgentRunStatus::WaitingPermission
            ),
            ProposalTerminalRelationKind::LegacyUnclassified => false,
        };
        if !is_sha256(self.target_binding_digest)
            || !is_sha256(self.agent_run_store_identity_digest)
            || self.target_owner_revision == 0
            || !status_is_compatible
        {
            anyhow::bail!("proposal_terminal_relation_target_contract_invalid");
        }
        Ok(())
    }
}

/// Metadata-only canonical relation row. Origin identity remains owned by
/// `proposal_terminal_owner_origins`; this row must not duplicate task, run,
/// epoch, message, or user-authored content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProposalTerminalRelationRecord {
    pub(crate) proposal_id: String,
    pub(crate) relation_kind: ProposalTerminalRelationKind,
    pub(crate) relation_digest: String,
    pub(crate) target_binding_digest: Option<String>,
    pub(crate) agent_run_store_identity_digest: Option<String>,
    pub(crate) target_owner_revision: Option<u64>,
    pub(crate) target_status_at_issue: Option<crate::agent::AgentRunStatus>,
    pub(crate) link_outbox_event_id: Option<String>,
    pub(crate) created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
impl ProposalTerminalRelationRecord {
    pub(crate) fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    pub(crate) fn relation_kind(&self) -> ProposalTerminalRelationKind {
        self.relation_kind
    }

    pub(crate) fn relation_digest(&self) -> &str {
        &self.relation_digest
    }

    pub(crate) fn link_outbox_event_id(&self) -> Option<&str> {
        self.link_outbox_event_id.as_deref()
    }

    pub(crate) fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ProposalTerminalRelationStoreOutcome {
    CreatedOwned {
        proposal: AgentProposal,
        relation: ProposalTerminalRelationRecord,
    },
    ReplayedSameOrigin {
        proposal: AgentProposal,
        relation: ProposalTerminalRelationRecord,
    },
    ReusedForeignNonBlocking {
        proposal: AgentProposal,
    },
}

#[derive(Clone)]
pub struct ProposalStore {
    conn: Arc<Mutex<Connection>>,
}

impl ProposalStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open proposals db at {:?}", db_path))?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory proposals db")?;
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
            "proposal_store",
            &["proposals"],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS proposals (
                id TEXT PRIMARY KEY,
                run_id TEXT,
                proposal_type TEXT NOT NULL,
                source TEXT NOT NULL,
                source_detail TEXT,
                base_hash TEXT,
                affected_path TEXT NOT NULL,
                before_json TEXT,
                after_json TEXT NOT NULL,
                reason TEXT NOT NULL,
                confidence REAL NOT NULL,
                risk_level TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                expires_at TEXT,
                dispatch_claim_id TEXT,
                dispatch_claimed_at TEXT,
                dispatch_snapshot_digest TEXT,
                dispatch_state TEXT NOT NULL DEFAULT 'unclaimed',
                dispatch_error_code TEXT,
                review_idempotency_key TEXT
            )",
            [],
        )?;
        for (column, definition) in [
            ("run_id", "TEXT"),
            ("source", "TEXT NOT NULL DEFAULT 'manual'"),
            ("source_detail", "TEXT"),
            ("base_hash", "TEXT"),
            ("expires_at", "TEXT"),
            ("dispatch_claim_id", "TEXT"),
            ("dispatch_claimed_at", "TEXT"),
            ("dispatch_snapshot_digest", "TEXT"),
            ("dispatch_state", "TEXT NOT NULL DEFAULT 'unclaimed'"),
            ("dispatch_error_code", "TEXT"),
            ("review_idempotency_key", "TEXT"),
        ] {
            crate::sqlite_migration::ensure_column(&tx, "proposals", column, definition)?;
        }
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_proposals_status ON proposals(status, created_at DESC)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_proposals_expires ON proposals(expires_at) WHERE status = 'pending'",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_proposals_dispatch_reconciliation
             ON proposals(dispatch_state, dispatch_claimed_at, id)",
            [],
        )?;
        tx.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_proposals_active_review_idempotency
             ON proposals(review_idempotency_key)
             WHERE review_idempotency_key IS NOT NULL
               AND status IN ('pending', 'postponed', 'edited');",
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS artifact_effects (
                proposal_id TEXT PRIMARY KEY,
                dispatch_claim_id TEXT NOT NULL,
                proposal_snapshot_digest TEXT NOT NULL,
                target_reference_digest TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                byte_size INTEGER NOT NULL CHECK(byte_size >= 0),
                media_type TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN (
                    'prepared', 'staged', 'confirmed',
                    'failed_before_effect', 'unknown'
                )),
                observed_content_digest TEXT,
                error_code TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(proposal_id) REFERENCES proposals(id)
            )",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_artifact_effects_reconciliation
             ON artifact_effects(state, updated_at, proposal_id)",
            [],
        )?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS proposal_terminal_owner_origins (
                proposal_id TEXT PRIMARY KEY,
                task_session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                epoch_id TEXT NOT NULL,
                epoch_generation INTEGER NOT NULL CHECK(epoch_generation > 0),
                admission_id TEXT NOT NULL,
                canonical_user_message_ref TEXT NOT NULL,
                canonical_user_message_digest TEXT NOT NULL,
                canonical_store_identity TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(epoch_id, proposal_id),
                FOREIGN KEY(proposal_id) REFERENCES proposals(id)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_proposal_terminal_owner_origin_task
             ON proposal_terminal_owner_origins(task_session_id, run_id, epoch_generation);",
        )?;
        crate::sqlite_migration::ensure_column(
            &tx,
            "proposal_terminal_owner_origins",
            "canonical_store_identity",
            "TEXT",
        )?;
        tx.execute(
            "UPDATE proposal_terminal_owner_origins
             SET canonical_store_identity = 'legacy_unverified'
             WHERE canonical_store_identity IS NULL
                OR TRIM(canonical_store_identity) = ''",
            [],
        )?;
        crate::persistence_outbox::init_schema(&tx)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS proposal_terminal_owner_relations (
                proposal_id TEXT PRIMARY KEY,
                relation_kind TEXT NOT NULL CHECK(relation_kind IN (
                    'non_blocking_successor',
                    'effect_blocking_prerequisite',
                    'action_resume_prerequisite',
                    'legacy_unclassified'
                )),
                relation_digest TEXT NOT NULL,
                target_binding_digest TEXT,
                agent_run_store_identity_digest TEXT,
                target_owner_revision INTEGER,
                target_status_at_issue TEXT,
                link_outbox_event_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY(proposal_id) REFERENCES proposals(id),
                FOREIGN KEY(link_outbox_event_id) REFERENCES canonical_outbox_events(event_id),
                CHECK(
                    (relation_kind = 'legacy_unclassified'
                     AND link_outbox_event_id IS NULL
                     AND target_binding_digest IS NULL
                     AND agent_run_store_identity_digest IS NULL
                     AND target_owner_revision IS NULL
                     AND target_status_at_issue IS NULL)
                    OR
                    (relation_kind != 'legacy_unclassified'
                     AND link_outbox_event_id IS NOT NULL
                     AND target_binding_digest IS NOT NULL
                     AND agent_run_store_identity_digest IS NOT NULL
                     AND target_owner_revision > 0
                     AND target_status_at_issue IS NOT NULL)
                )
             ) WITHOUT ROWID;
             CREATE UNIQUE INDEX IF NOT EXISTS idx_proposal_terminal_relation_outbox
             ON proposal_terminal_owner_relations(link_outbox_event_id)
             WHERE link_outbox_event_id IS NOT NULL;",
        )?;
        for (column, definition) in [
            ("target_binding_digest", "TEXT"),
            ("agent_run_store_identity_digest", "TEXT"),
            ("target_owner_revision", "INTEGER"),
            ("target_status_at_issue", "TEXT"),
        ] {
            crate::sqlite_migration::ensure_column(
                &tx,
                "proposal_terminal_owner_relations",
                column,
                definition,
            )?;
        }
        validate_existing_terminal_relation_target_contract(&tx)?;
        backfill_legacy_terminal_relations(&tx)?;
        crate::sqlite_migration::record_schema_version(&tx, "proposal_store", 10)?;
        tx.commit()?;
        Ok(())
    }

    pub fn create_proposal(&self, proposal: &AgentProposal) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO proposals (id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.base_hash.as_ref(),
                proposal.affected_path,
                proposal.before.as_ref().map(|b| serde_json::to_string(b).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.status.to_string(),
                proposal.created_at.to_rfc3339(),
                proposal.resolved_at.map(|t| t.to_rfc3339()),
                proposal.expires_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// Atomically create one active ReviewWorkflow proposal or return the
    /// proposal that already owns the exact durable idempotency key.
    ///
    /// The key deliberately lives beside the canonical Proposal row. A
    /// scan-before-insert in ReviewWorkflow cannot protect concurrent callers,
    /// and reconstructing a caller-supplied key from Proposal payload after a
    /// restart is impossible. The immediate transaction plus partial unique
    /// index make this store the single admission authority.
    pub(crate) fn create_or_reuse_active_review_proposal(
        &self,
        proposal: &AgentProposal,
        review_idempotency_key: &str,
    ) -> Result<(AgentProposal, bool)> {
        let review_idempotency_key = review_idempotency_key.trim();
        if review_idempotency_key.is_empty() {
            anyhow::bail!("review workflow idempotency key is empty");
        }
        if review_idempotency_key.len() > 512 {
            anyhow::bail!("review workflow idempotency key is too large");
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                        affected_path, before_json, after_json, reason, confidence,
                        risk_level, status, created_at, resolved_at, expires_at
                 FROM proposals
                 WHERE review_idempotency_key = ?1
                   AND status IN ('pending', 'postponed', 'edited')
                 LIMIT 1",
                [review_idempotency_key],
                Self::row_to_proposal,
            )
            .optional()?;
        if let Some(existing) = existing {
            tx.commit()?;
            return Ok((existing, false));
        }

        tx.execute(
            "INSERT INTO proposals (
                id, run_id, proposal_type, source, source_detail, base_hash,
                affected_path, before_json, after_json, reason, confidence,
                risk_level, status, created_at, resolved_at, expires_at,
                review_idempotency_key
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17
             )",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.base_hash.as_ref(),
                proposal.affected_path,
                proposal
                    .before
                    .as_ref()
                    .map(|before| serde_json::to_string(before).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.status.to_string(),
                proposal.created_at.to_rfc3339(),
                proposal.resolved_at.map(|time| time.to_rfc3339()),
                proposal.expires_at.map(|time| time.to_rfc3339()),
                review_idempotency_key,
            ],
        )?;
        tx.commit()?;
        Ok((proposal.clone(), true))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub(super) fn create_or_reuse_active_review_proposal_with_terminal_relation(
        &self,
        write_proof: &ProposalTerminalRelationStorageWriteProof,
        proposal: &AgentProposal,
        review_idempotency_key: &str,
        task_session_id: &str,
        run_id: &str,
        epoch_id: &str,
        epoch_generation: u64,
        admission_id: &str,
        canonical_user_message_ref: &str,
        canonical_user_message_digest: &str,
        canonical_store_identity: &str,
        relation_kind: ProposalTerminalRelationKind,
        target_binding_digest: &str,
        agent_run_store_identity_digest: &str,
        target_owner_revision: u64,
        target_status_at_issue: crate::agent::AgentRunStatus,
    ) -> Result<ProposalTerminalRelationStoreOutcome> {
        let review_idempotency_key = review_idempotency_key.trim();
        let target = ProposalTerminalRelationTargetContract {
            target_binding_digest,
            agent_run_store_identity_digest,
            target_owner_revision,
            target_status_at_issue,
        };
        validate_terminal_relation_submission(
            review_idempotency_key,
            task_session_id,
            run_id,
            epoch_id,
            epoch_generation,
            admission_id,
            canonical_user_message_ref,
            canonical_user_message_digest,
            canonical_store_identity,
            relation_kind,
            target,
        )?;

        let storage_request_digest = proposal_terminal_relation_storage_request_digest(
            proposal,
            review_idempotency_key,
            task_session_id,
            run_id,
            epoch_id,
            epoch_generation,
            admission_id,
            canonical_user_message_ref,
            canonical_user_message_digest,
            canonical_store_identity,
            relation_kind,
            target.target_binding_digest,
            target.agent_run_store_identity_digest,
            target.target_owner_revision,
            target.target_status_at_issue,
        )?;
        write_proof.validate_for(&storage_request_digest)?;

        let mut canonical_proposal = proposal.clone();
        // The typed boundary never accepts caller ownership of a canonical
        // Proposal id. Exact replay returns the existing store-issued id.
        canonical_proposal.id = uuid::Uuid::new_v4().to_string();
        canonical_proposal.run_id = None;
        canonical_proposal.source_detail = None;

        let mut conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                        affected_path, before_json, after_json, reason, confidence,
                        risk_level, status, created_at, resolved_at, expires_at
                 FROM proposals
                 WHERE review_idempotency_key = ?1
                   AND status IN ('pending', 'postponed', 'edited')
                 LIMIT 1",
                [review_idempotency_key],
                Self::row_to_proposal,
            )
            .optional()?;

        if let Some(existing) = existing {
            if !terminal_relation_proposal_identity_matches(&existing, &canonical_proposal) {
                anyhow::bail!("proposal_terminal_relation_identity_drift");
            }
            let existing_origin = terminal_owner_origin_binding_from_conn(&tx, &existing.id)?;
            if existing_origin.as_ref().is_some_and(|origin| {
                terminal_owner_origin_matches(
                    origin,
                    task_session_id,
                    run_id,
                    epoch_id,
                    epoch_generation,
                    admission_id,
                    canonical_user_message_ref,
                    canonical_user_message_digest,
                    canonical_store_identity,
                )
            }) {
                let relation = terminal_owner_relation_from_conn(&tx, &existing.id)?
                    .context("proposal_terminal_relation_missing_for_owned_replay")?;
                let expected_digest = proposal_terminal_relation_digest(
                    &existing.id,
                    relation_kind,
                    task_session_id,
                    run_id,
                    epoch_id,
                    epoch_generation,
                    admission_id,
                    canonical_user_message_ref,
                    canonical_user_message_digest,
                    canonical_store_identity,
                    Some(target),
                )?;
                if relation.relation_kind != relation_kind
                    || relation.relation_digest != expected_digest
                    || relation.target_binding_digest.as_deref()
                        != Some(target.target_binding_digest)
                    || relation.agent_run_store_identity_digest.as_deref()
                        != Some(target.agent_run_store_identity_digest)
                    || relation.target_owner_revision != Some(target.target_owner_revision)
                    || relation.target_status_at_issue != Some(target.target_status_at_issue)
                {
                    anyhow::bail!("proposal_terminal_relation_kind_drift");
                }
                validate_relation_outbox_from_conn(&tx, &relation)?;
                tx.commit()?;
                return Ok(ProposalTerminalRelationStoreOutcome::ReplayedSameOrigin {
                    proposal: existing,
                    relation,
                });
            }

            // Repeating the exact same sensitive Memory fact may encounter a
            // pending effect-blocking review owned by an earlier turn. The
            // original typed relation remains the sole blocking owner; the
            // current turn only reuses the review item and must not acquire a
            // second relation or AgentRun projection.
            if existing.proposal_type == ProposalType::MemoryWrite
                && relation_kind == ProposalTerminalRelationKind::EffectBlockingPrerequisite
            {
                if let Some(relation) = terminal_owner_relation_from_conn(&tx, &existing.id)?
                    .filter(|relation| relation.relation_kind == relation_kind)
                {
                    validate_relation_outbox_from_conn(&tx, &relation)?;
                    tx.commit()?;
                    return Ok(
                        ProposalTerminalRelationStoreOutcome::ReusedForeignNonBlocking {
                            proposal: existing,
                        },
                    );
                }
            }

            // An active Proposal already belongs to another immutable origin
            // (or to an unowned legacy submission). Reuse may deduplicate the
            // review item, but it must never bind, block, or enqueue an
            // AgentRun projection for the current turn.
            if relation_kind != ProposalTerminalRelationKind::NonBlockingSuccessor {
                anyhow::bail!("proposal_terminal_relation_foreign_blocking_collision");
            }
            tx.commit()?;
            return Ok(
                ProposalTerminalRelationStoreOutcome::ReusedForeignNonBlocking {
                    proposal: existing,
                },
            );
        }

        tx.execute(
            "INSERT INTO proposals (
                id, run_id, proposal_type, source, source_detail, base_hash,
                affected_path, before_json, after_json, reason, confidence,
                risk_level, status, created_at, resolved_at, expires_at,
                review_idempotency_key
             ) VALUES (
                ?1, NULL, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15
             )",
            params![
                canonical_proposal.id,
                canonical_proposal.proposal_type.to_string(),
                canonical_proposal.source,
                canonical_proposal.base_hash.as_ref(),
                canonical_proposal.affected_path,
                canonical_proposal
                    .before
                    .as_ref()
                    .map(|before| serde_json::to_string(before).unwrap_or_default()),
                serde_json::to_string(&canonical_proposal.after).unwrap_or_default(),
                canonical_proposal.reason,
                canonical_proposal.confidence,
                canonical_proposal.risk_level.to_string(),
                canonical_proposal.status.to_string(),
                canonical_proposal.created_at.to_rfc3339(),
                canonical_proposal.resolved_at.map(|time| time.to_rfc3339()),
                canonical_proposal.expires_at.map(|time| time.to_rfc3339()),
                review_idempotency_key,
            ],
        )?;
        tx.execute(
            "INSERT INTO proposal_terminal_owner_origins (
                proposal_id, task_session_id, run_id, epoch_id, epoch_generation,
                admission_id, canonical_user_message_ref,
                canonical_user_message_digest, canonical_store_identity, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                canonical_proposal.id,
                task_session_id,
                run_id,
                epoch_id,
                i64::try_from(epoch_generation)?,
                admission_id,
                canonical_user_message_ref,
                canonical_user_message_digest,
                canonical_store_identity,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;

        let relation_digest = proposal_terminal_relation_digest(
            &canonical_proposal.id,
            relation_kind,
            task_session_id,
            run_id,
            epoch_id,
            epoch_generation,
            admission_id,
            canonical_user_message_ref,
            canonical_user_message_digest,
            canonical_store_identity,
            Some(target),
        )?;
        let projection_target = relation_kind
            .projection_target()
            .context("typed proposal terminal relation projection target missing")?;
        let outbox_receipt = crate::persistence_outbox::enqueue_mutation(
            &tx,
            "proposal_terminal_relation",
            &canonical_proposal.id,
            "linked",
            &relation_digest,
            &[projection_target],
        )?;
        tx.execute(
            "INSERT INTO proposal_terminal_owner_relations (
                proposal_id, relation_kind, relation_digest,
                target_binding_digest, agent_run_store_identity_digest,
                target_owner_revision, target_status_at_issue,
                link_outbox_event_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                canonical_proposal.id,
                relation_kind.as_str(),
                relation_digest,
                target.target_binding_digest,
                target.agent_run_store_identity_digest,
                i64::try_from(target.target_owner_revision)?,
                target.target_status_at_issue.to_string(),
                outbox_receipt.event_id,
                outbox_receipt.created_at.to_rfc3339(),
            ],
        )?;
        #[cfg(test)]
        if terminal_relation_commit_failpoints()
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal relation failpoint mutex: {error}"))?
            .remove(review_idempotency_key)
        {
            anyhow::bail!("proposal_terminal_relation_commit_failpoint");
        }
        let relation = ProposalTerminalRelationRecord {
            proposal_id: canonical_proposal.id.clone(),
            relation_kind,
            relation_digest,
            target_binding_digest: Some(target.target_binding_digest.to_string()),
            agent_run_store_identity_digest: Some(
                target.agent_run_store_identity_digest.to_string(),
            ),
            target_owner_revision: Some(target.target_owner_revision),
            target_status_at_issue: Some(target.target_status_at_issue),
            link_outbox_event_id: Some(outbox_receipt.event_id),
            created_at: outbox_receipt.created_at,
        };
        tx.commit()?;
        Ok(ProposalTerminalRelationStoreOutcome::CreatedOwned {
            proposal: canonical_proposal,
            relation,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub(crate) fn create_or_reuse_active_review_proposal_with_terminal_origin(
        &self,
        proposal: &AgentProposal,
        review_idempotency_key: &str,
        task_session_id: &str,
        run_id: &str,
        epoch_id: &str,
        epoch_generation: u64,
        admission_id: &str,
        canonical_user_message_ref: &str,
        canonical_user_message_digest: &str,
    ) -> Result<(AgentProposal, bool)> {
        let review_idempotency_key = review_idempotency_key.trim();
        if review_idempotency_key.is_empty()
            || review_idempotency_key.len() > 512
            || task_session_id.trim().is_empty()
            || run_id.trim().is_empty()
            || epoch_id.trim().is_empty()
            || epoch_generation == 0
            || admission_id.trim().is_empty()
            || canonical_user_message_ref.trim().is_empty()
            || canonical_user_message_digest.trim().is_empty()
        {
            anyhow::bail!("terminal owner review origin is invalid");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                        affected_path, before_json, after_json, reason, confidence,
                        risk_level, status, created_at, resolved_at, expires_at
                 FROM proposals
                 WHERE review_idempotency_key = ?1
                   AND status IN ('pending', 'postponed', 'edited')
                 LIMIT 1",
                [review_idempotency_key],
                Self::row_to_proposal,
            )
            .optional()?;
        if let Some(existing) = existing {
            let origin = terminal_owner_origin_binding_from_conn(&tx, &existing.id)?
                .context("terminal owner proposal lost its immutable origin")?;
            if origin.task_session_id != task_session_id
                || origin.run_id != run_id
                || origin.epoch_id != epoch_id
                || origin.epoch_generation != epoch_generation
                || origin.admission_id != admission_id
                || origin.canonical_user_message_ref != canonical_user_message_ref
                || origin.canonical_user_message_digest != canonical_user_message_digest
            {
                anyhow::bail!("terminal owner proposal origin replay mismatch");
            }
            tx.commit()?;
            return Ok((existing, false));
        }

        tx.execute(
            "INSERT INTO proposals (
                id, run_id, proposal_type, source, source_detail, base_hash,
                affected_path, before_json, after_json, reason, confidence,
                risk_level, status, created_at, resolved_at, expires_at,
                review_idempotency_key
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17
             )",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.base_hash.as_ref(),
                proposal.affected_path,
                proposal
                    .before
                    .as_ref()
                    .map(|before| serde_json::to_string(before).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.status.to_string(),
                proposal.created_at.to_rfc3339(),
                proposal.resolved_at.map(|time| time.to_rfc3339()),
                proposal.expires_at.map(|time| time.to_rfc3339()),
                review_idempotency_key,
            ],
        )?;
        tx.execute(
            "INSERT INTO proposal_terminal_owner_origins (
                proposal_id, task_session_id, run_id, epoch_id, epoch_generation,
                admission_id, canonical_user_message_ref,
                canonical_user_message_digest, canonical_store_identity, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'legacy_unverified', ?9)",
            params![
                proposal.id,
                task_session_id,
                run_id,
                epoch_id,
                i64::try_from(epoch_generation)?,
                admission_id,
                canonical_user_message_ref,
                canonical_user_message_digest,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        tx.commit()?;
        Ok((proposal.clone(), true))
    }

    pub fn terminal_owner_origin_binding(
        &self,
        proposal_id: &str,
    ) -> Result<Option<TerminalOwnerOriginBinding>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        terminal_owner_origin_binding_from_conn(&conn, proposal_id)
    }

    #[cfg(test)]
    pub(crate) fn terminal_owner_relation(
        &self,
        proposal_id: &str,
    ) -> Result<Option<ProposalTerminalRelationRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        terminal_owner_relation_from_conn(&conn, proposal_id)
    }

    /// Load one metadata-only projection proof from the canonical Proposal,
    /// origin, typed relation, and exact outbox delivery. This method does not
    /// copy Proposal/user content into the projection boundary.
    pub fn terminal_relation_projection_proof(
        &self,
        proposal_id: &str,
    ) -> Result<Option<ProposalTerminalRelationProjectionProof>> {
        let proposal_id = proposal_id.trim();
        if proposal_id.is_empty() {
            anyhow::bail!("proposal_terminal_relation_projection_id_empty");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let Some(origin) = terminal_owner_origin_binding_from_conn(&conn, proposal_id)? else {
            return Ok(None);
        };
        let Some(relation) = terminal_owner_relation_from_conn(&conn, proposal_id)? else {
            return Ok(None);
        };
        if relation.relation_kind == ProposalTerminalRelationKind::LegacyUnclassified {
            anyhow::bail!("proposal_terminal_relation_legacy_projection_forbidden");
        }
        validate_relation_outbox_from_conn(&conn, &relation)?;
        let projection_target = relation
            .relation_kind
            .projection_target()
            .context("proposal_terminal_relation_projection_target_missing")?;
        let proof = ProposalTerminalRelationProjectionProof {
            proposal_id: relation.proposal_id,
            task_session_id: origin.task_session_id().to_string(),
            run_id: origin.run_id().to_string(),
            epoch_id: origin.epoch_id().to_string(),
            epoch_generation: origin.epoch_generation(),
            admission_id: origin.admission_id().to_string(),
            canonical_user_message_ref: origin.canonical_user_message_ref().to_string(),
            canonical_user_message_digest: origin.canonical_user_message_digest().to_string(),
            canonical_store_identity: origin.canonical_store_identity().to_string(),
            relation_kind: relation.relation_kind,
            relation_digest: relation.relation_digest,
            target_binding_digest: relation
                .target_binding_digest
                .context("proposal_terminal_relation_projection_target_binding_missing")?,
            agent_run_store_identity_digest: relation
                .agent_run_store_identity_digest
                .context("proposal_terminal_relation_projection_store_binding_missing")?,
            target_owner_revision: relation
                .target_owner_revision
                .context("proposal_terminal_relation_projection_target_revision_missing")?,
            target_status_at_issue: relation
                .target_status_at_issue
                .context("proposal_terminal_relation_projection_target_status_missing")?,
            source_outbox_event_id: relation
                .link_outbox_event_id
                .context("proposal_terminal_relation_projection_outbox_missing")?,
            projection_target: projection_target.to_string(),
            authority: ProposalTerminalRelationProjectionAuthority::VerifiedByProposalStoreOutbox,
        };
        proof.validate()?;
        Ok(Some(proof))
    }

    /// Finalize the exact metadata-only relation delivery after the AgentRun
    /// projection has committed. Exact replay remains idempotent.
    pub fn mark_terminal_relation_projection_applied(
        &self,
        proof: &ProposalTerminalRelationProjectionProof,
    ) -> Result<()> {
        proof.validate()?;
        let conn = self
            .conn
            .lock()
            .map_err(|error| anyhow::anyhow!("mutex poison: {error}"))?;
        let relation = terminal_owner_relation_from_conn(&conn, proof.proposal_id())?
            .context("proposal_terminal_relation_projection_relation_missing")?;
        validate_relation_outbox_from_conn(&conn, &relation)?;
        if relation.relation_digest != proof.relation_digest()
            || relation.link_outbox_event_id.as_deref() != Some(proof.source_outbox_event_id())
            || relation.relation_kind.projection_target() != Some(proof.projection_target())
        {
            anyhow::bail!("proposal_terminal_relation_projection_finalize_mismatch");
        }
        crate::persistence_outbox::mark_delivery_applied(
            &conn,
            proof.source_outbox_event_id(),
            proof.projection_target(),
        )
    }

    /// Record a retryable derived-projection failure without changing the
    /// canonical Proposal or relation truth.
    pub fn mark_terminal_relation_projection_degraded(
        &self,
        proof: &ProposalTerminalRelationProjectionProof,
        error: &str,
    ) -> Result<()> {
        proof.validate()?;
        let conn = self
            .conn
            .lock()
            .map_err(|lock_error| anyhow::anyhow!("mutex poison: {lock_error}"))?;
        let relation = terminal_owner_relation_from_conn(&conn, proof.proposal_id())?
            .context("proposal_terminal_relation_projection_relation_missing")?;
        validate_relation_outbox_from_conn(&conn, &relation)?;
        if relation.relation_digest != proof.relation_digest()
            || relation.link_outbox_event_id.as_deref() != Some(proof.source_outbox_event_id())
            || relation.relation_kind.projection_target() != Some(proof.projection_target())
        {
            anyhow::bail!("proposal_terminal_relation_projection_degrade_mismatch");
        }
        crate::persistence_outbox::mark_delivery_degraded(
            &conn,
            proof.source_outbox_event_id(),
            proof.projection_target(),
            error,
        )
    }

    #[cfg(test)]
    pub(super) fn fail_next_terminal_relation_commit_for_test(
        &self,
        idempotency_key: &str,
    ) -> Result<()> {
        terminal_relation_commit_failpoints()
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal relation failpoint mutex: {error}"))?
            .insert(idempotency_key.to_string());
        Ok(())
    }

    pub(crate) fn update_active_review_proposal(
        &self,
        proposal: &AgentProposal,
        expected_proposal_id: &str,
        review_idempotency_key: &str,
    ) -> Result<bool> {
        let review_idempotency_key = review_idempotency_key.trim();
        if review_idempotency_key.is_empty() || review_idempotency_key.len() > 512 {
            anyhow::bail!("review workflow idempotency key is invalid");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute(
            "UPDATE proposals SET
                run_id = ?2,
                proposal_type = ?3,
                source = ?4,
                source_detail = ?5,
                base_hash = ?6,
                affected_path = ?7,
                before_json = ?8,
                after_json = ?9,
                reason = ?10,
                confidence = ?11,
                risk_level = ?12,
                status = 'pending',
                resolved_at = NULL,
                expires_at = ?13,
                review_idempotency_key = ?14
             WHERE id = ?1
               AND id = ?15
               AND status = 'pending'
               AND dispatch_state IN ('unclaimed', 'failed_before_effect')",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.base_hash.as_ref(),
                proposal.affected_path,
                proposal
                    .before
                    .as_ref()
                    .map(|before| serde_json::to_string(before).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.expires_at.map(|time| time.to_rfc3339()),
                review_idempotency_key,
                expected_proposal_id,
            ],
        )? == 1)
    }

    pub fn update_proposal(&self, proposal: &AgentProposal) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let changed = conn.execute(
            "UPDATE proposals SET
                run_id = ?2,
                proposal_type = ?3,
                source = ?4,
                source_detail = ?5,
                base_hash = ?6,
                affected_path = ?7,
                before_json = ?8,
                after_json = ?9,
                reason = ?10,
                confidence = ?11,
                risk_level = ?12,
                status = ?13,
                resolved_at = ?14,
                expires_at = ?15
            WHERE id = ?1",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.base_hash.as_ref(),
                proposal.affected_path,
                proposal
                    .before
                    .as_ref()
                    .map(|b| serde_json::to_string(b).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.status.to_string(),
                proposal.resolved_at.map(|t| t.to_rfc3339()),
                proposal.expires_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("proposal_status_projection_target_missing");
        }
        Ok(())
    }

    /// Applies a review-only mutation with a compare-and-swap guard. The mutation is
    /// rejected if another reviewer changed the Proposal or any effect dispatch started
    /// after the caller read it.
    pub fn update_review_before_dispatch(
        &self,
        proposal: &AgentProposal,
        expected_status: ProposalStatus,
    ) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute(
            "UPDATE proposals SET
                run_id = ?2,
                proposal_type = ?3,
                source = ?4,
                source_detail = ?5,
                base_hash = ?6,
                affected_path = ?7,
                before_json = ?8,
                after_json = ?9,
                reason = ?10,
                confidence = ?11,
                risk_level = ?12,
                status = ?13,
                resolved_at = ?14,
                expires_at = ?15
             WHERE id = ?1
               AND status = ?16
               AND dispatch_state IN ('unclaimed', 'failed_before_effect')",
            params![
                proposal.id,
                proposal.run_id.as_ref(),
                proposal.proposal_type.to_string(),
                proposal.source,
                proposal.source_detail.as_ref(),
                proposal.base_hash.as_ref(),
                proposal.affected_path,
                proposal
                    .before
                    .as_ref()
                    .map(|before| serde_json::to_string(before).unwrap_or_default()),
                serde_json::to_string(&proposal.after).unwrap_or_default(),
                proposal.reason,
                proposal.confidence,
                proposal.risk_level.to_string(),
                proposal.status.to_string(),
                proposal.resolved_at.map(|time| time.to_rfc3339()),
                proposal.expires_at.map(|time| time.to_rfc3339()),
                expected_status.to_string(),
            ],
        )? == 1)
    }

    pub fn claim_dispatch(&self, proposal_id: &str) -> Result<Option<String>> {
        let claim_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let proposal = tx
            .query_row(
                "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                        affected_path, before_json, after_json, reason, confidence,
                        risk_level, status, created_at, resolved_at, expires_at
                 FROM proposals
                 WHERE id = ?1
                   AND status IN ('pending', 'postponed', 'edited')
                   AND (
                        dispatch_claim_id IS NULL
                        OR dispatch_state IN ('unclaimed', 'failed_before_effect')
                   )",
                [proposal_id],
                Self::row_to_proposal,
            )
            .optional()?;
        let Some(proposal) = proposal else {
            tx.commit()?;
            return Ok(None);
        };
        let snapshot_digest =
            crate::agent::review_workflow::review_proposal_snapshot_digest(&proposal)?;
        let changed = tx.execute(
            "UPDATE proposals
             SET dispatch_claim_id = ?2,
                 dispatch_claimed_at = ?3,
                 dispatch_snapshot_digest = ?4,
                 dispatch_state = 'claimed',
                 dispatch_error_code = NULL
             WHERE id = ?1
               AND status IN ('pending', 'postponed', 'edited')
               AND (
                    dispatch_claim_id IS NULL
                    OR dispatch_state IN ('unclaimed', 'failed_before_effect')
               )",
            params![proposal_id, claim_id, now, snapshot_digest],
        )?;
        tx.commit()?;
        Ok((changed == 1).then_some(claim_id))
    }

    /// Records a mechanically proven failure before any external or canonical effect.
    /// Only this state is eligible for a later claim. Unknown/confirmed states are terminal
    /// for automatic retry because retrying them could duplicate an effect.
    pub fn mark_dispatch_failed_before_effect(
        &self,
        proposal_id: &str,
        claim_id: &str,
        error_code: &str,
    ) -> Result<bool> {
        self.transition_claimed_dispatch(
            proposal_id,
            claim_id,
            "failed_before_effect",
            Some(error_code),
        )
    }

    /// Records that local execution cannot prove whether an effect happened.
    /// This state is intentionally not claimable without explicit reconciliation.
    pub fn mark_dispatch_unknown(
        &self,
        proposal_id: &str,
        claim_id: &str,
        error_code: &str,
    ) -> Result<bool> {
        self.transition_claimed_dispatch(proposal_id, claim_id, "unknown", Some(error_code))
    }

    /// Persists effect truth before updating the Proposal read-model status. If that later
    /// projection update fails, this durable state is the reconciliation fact and prevents a
    /// second dispatch.
    pub fn mark_effect_confirmed_projection_pending(
        &self,
        proposal_id: &str,
        claim_id: &str,
    ) -> Result<bool> {
        self.transition_claimed_dispatch(
            proposal_id,
            claim_id,
            "confirmed_projection_pending",
            Some("proposal_status_projection_pending"),
        )
    }

    fn transition_claimed_dispatch(
        &self,
        proposal_id: &str,
        claim_id: &str,
        next_state: &str,
        error_code: Option<&str>,
    ) -> Result<bool> {
        debug_assert!(matches!(
            next_state,
            "failed_before_effect" | "unknown" | "confirmed_projection_pending"
        ));
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute(
            "UPDATE proposals
             SET dispatch_state = ?3, dispatch_error_code = ?4
             WHERE id = ?1 AND dispatch_claim_id = ?2 AND dispatch_state = 'claimed'",
            params![proposal_id, claim_id, next_state, error_code],
        )? == 1)
    }

    pub fn dispatch_state(&self, proposal_id: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT dispatch_state FROM proposals WHERE id = ?1",
            [proposal_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn dispatch_error_code(&self, proposal_id: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT dispatch_error_code FROM proposals WHERE id = ?1",
            [proposal_id],
            |row| row.get(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(Into::into)
    }

    /// Persist the exact artifact effect intent before any filesystem bytes are
    /// staged. The record intentionally contains digests and references only;
    /// artifact bodies remain owned by the Proposal snapshot and filesystem.
    pub fn prepare_artifact_effect(
        &self,
        proposal_id: &str,
        claim_id: &str,
        target_reference_digest: &str,
        content_digest: &str,
        byte_size: u64,
        media_type: &str,
    ) -> Result<ArtifactEffectRecord> {
        for (label, value) in [
            ("proposal_id", proposal_id),
            ("claim_id", claim_id),
            ("target_reference_digest", target_reference_digest),
            ("content_digest", content_digest),
            ("media_type", media_type),
        ] {
            if value.trim().is_empty() {
                anyhow::bail!("artifact effect {label} is empty");
            }
        }
        let byte_size = i64::try_from(byte_size).context("artifact byte size exceeds SQLite")?;
        let now = chrono::Utc::now();
        let now_text = now.to_rfc3339();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot_digest = tx
            .query_row(
                "SELECT dispatch_snapshot_digest
                 FROM proposals
                 WHERE id = ?1 AND dispatch_claim_id = ?2
                   AND dispatch_state = 'claimed'
                   AND dispatch_snapshot_digest IS NOT NULL",
                params![proposal_id, claim_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                anyhow::anyhow!("artifact effect requires the current dispatch claim")
            })?;

        let existing = tx
            .query_row(
                "SELECT proposal_id, dispatch_claim_id, proposal_snapshot_digest,
                        target_reference_digest, content_digest, byte_size, media_type,
                        state, observed_content_digest, error_code, created_at, updated_at
                 FROM artifact_effects WHERE proposal_id = ?1",
                [proposal_id],
                Self::row_to_artifact_effect,
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing.dispatch_claim_id == claim_id {
                if existing.proposal_snapshot_digest != snapshot_digest
                    || existing.target_reference_digest != target_reference_digest
                    || existing.content_digest != content_digest
                    || existing.byte_size != byte_size as u64
                    || existing.media_type != media_type
                {
                    anyhow::bail!(
                        "artifact effect replay payload does not match its dispatch claim"
                    );
                }
                tx.commit()?;
                return Ok(existing);
            }
            if existing.state != ArtifactEffectState::FailedBeforeEffect {
                anyhow::bail!(
                    "artifact effect has unresolved or completed bytes under another dispatch claim"
                );
            }
            let changed = tx.execute(
                "UPDATE artifact_effects
                 SET dispatch_claim_id = ?2,
                     proposal_snapshot_digest = ?3,
                     target_reference_digest = ?4,
                     content_digest = ?5,
                     byte_size = ?6,
                     media_type = ?7,
                     state = 'prepared',
                     observed_content_digest = NULL,
                     error_code = NULL,
                     created_at = ?8,
                     updated_at = ?8
                 WHERE proposal_id = ?1 AND state = 'failed_before_effect'",
                params![
                    proposal_id,
                    claim_id,
                    snapshot_digest,
                    target_reference_digest,
                    content_digest,
                    byte_size,
                    media_type,
                    now_text,
                ],
            )?;
            if changed != 1 {
                anyhow::bail!("artifact effect retry claim lost its compare-and-swap");
            }
        } else {
            tx.execute(
                "INSERT INTO artifact_effects (
                    proposal_id, dispatch_claim_id, proposal_snapshot_digest,
                    target_reference_digest, content_digest, byte_size, media_type,
                    state, observed_content_digest, error_code, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'prepared', NULL, NULL, ?8, ?8)",
                params![
                    proposal_id,
                    claim_id,
                    snapshot_digest,
                    target_reference_digest,
                    content_digest,
                    byte_size,
                    media_type,
                    now_text,
                ],
            )?;
        }
        tx.commit()?;
        Ok(ArtifactEffectRecord {
            proposal_id: proposal_id.to_string(),
            dispatch_claim_id: claim_id.to_string(),
            proposal_snapshot_digest: snapshot_digest,
            target_reference_digest: target_reference_digest.to_string(),
            content_digest: content_digest.to_string(),
            byte_size: byte_size as u64,
            media_type: media_type.to_string(),
            state: ArtifactEffectState::Prepared,
            observed_content_digest: None,
            error_code: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn mark_artifact_staged(&self, proposal_id: &str, claim_id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute(
            "UPDATE artifact_effects
             SET state = 'staged', updated_at = ?3
             WHERE proposal_id = ?1 AND dispatch_claim_id = ?2 AND state = 'prepared'",
            params![proposal_id, claim_id, chrono::Utc::now().to_rfc3339()],
        )? == 1)
    }

    /// Atomically records filesystem digest confirmation and advances the
    /// Proposal receipt to projection-pending. This closes the crash window
    /// where bytes were durable but the generic Proposal receipt was absent.
    pub fn finish_artifact_confirmed(
        &self,
        proposal_id: &str,
        claim_id: &str,
        observed_content_digest: &str,
    ) -> Result<bool> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact_changed = tx.execute(
            "UPDATE artifact_effects
             SET state = 'confirmed', observed_content_digest = ?3,
                 error_code = NULL, updated_at = ?4
             WHERE proposal_id = ?1 AND dispatch_claim_id = ?2
               AND content_digest = ?3
               AND state IN ('prepared', 'staged', 'unknown')",
            params![
                proposal_id,
                claim_id,
                observed_content_digest,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        let proposal_changed = tx.execute(
            "UPDATE proposals
             SET dispatch_state = 'confirmed_projection_pending',
                 dispatch_error_code = 'proposal_status_projection_pending'
             WHERE id = ?1 AND dispatch_claim_id = ?2
               AND dispatch_state IN ('claimed', 'unknown')",
            params![proposal_id, claim_id],
        )?;
        if artifact_changed == 1 && proposal_changed == 1 {
            tx.commit()?;
            return Ok(true);
        }
        let already_confirmed: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM artifact_effects AS artifact
                JOIN proposals AS proposal ON proposal.id = artifact.proposal_id
                WHERE artifact.proposal_id = ?1
                  AND artifact.dispatch_claim_id = ?2
                  AND artifact.state = 'confirmed'
                  AND artifact.content_digest = ?3
                  AND artifact.observed_content_digest = ?3
                  AND proposal.dispatch_claim_id = ?2
                  AND proposal.dispatch_state IN ('confirmed_projection_pending', 'confirmed')
             )",
            params![proposal_id, claim_id, observed_content_digest],
            |row| row.get(0),
        )?;
        if already_confirmed {
            tx.commit()?;
            Ok(true)
        } else {
            tx.rollback()?;
            Ok(false)
        }
    }

    pub fn finish_artifact_failed_before_effect(
        &self,
        proposal_id: &str,
        claim_id: &str,
        error_code: &str,
    ) -> Result<bool> {
        self.finish_artifact_nonconfirmed(
            proposal_id,
            claim_id,
            ArtifactEffectState::FailedBeforeEffect,
            error_code,
        )
    }

    pub fn finish_artifact_unknown(
        &self,
        proposal_id: &str,
        claim_id: &str,
        error_code: &str,
    ) -> Result<bool> {
        self.finish_artifact_nonconfirmed(
            proposal_id,
            claim_id,
            ArtifactEffectState::Unknown,
            error_code,
        )
    }

    fn finish_artifact_nonconfirmed(
        &self,
        proposal_id: &str,
        claim_id: &str,
        next_state: ArtifactEffectState,
        error_code: &str,
    ) -> Result<bool> {
        debug_assert!(matches!(
            next_state,
            ArtifactEffectState::FailedBeforeEffect | ArtifactEffectState::Unknown
        ));
        let next = next_state.as_str();
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let artifact_changed = tx.execute(
            "UPDATE artifact_effects
             SET state = ?3, error_code = ?4, updated_at = ?5
             WHERE proposal_id = ?1 AND dispatch_claim_id = ?2
               AND state IN ('prepared', 'staged', 'unknown')",
            params![
                proposal_id,
                claim_id,
                next,
                error_code,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        let proposal_changed = tx.execute(
            "UPDATE proposals SET dispatch_state = ?3, dispatch_error_code = ?4
             WHERE id = ?1 AND dispatch_claim_id = ?2
               AND dispatch_state IN ('claimed', 'unknown')",
            params![proposal_id, claim_id, next, error_code],
        )?;
        if artifact_changed == 1 && proposal_changed == 1 {
            tx.commit()?;
            Ok(true)
        } else {
            tx.rollback()?;
            Ok(false)
        }
    }

    pub fn artifact_effect(&self, proposal_id: &str) -> Result<Option<ArtifactEffectRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT proposal_id, dispatch_claim_id, proposal_snapshot_digest,
                    target_reference_digest, content_digest, byte_size, media_type,
                    state, observed_content_digest, error_code, created_at, updated_at
             FROM artifact_effects WHERE proposal_id = ?1",
            [proposal_id],
            Self::row_to_artifact_effect,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_artifact_effects_for_reconciliation(
        &self,
        limit: i64,
    ) -> Result<Vec<ArtifactEffectRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT proposal_id, dispatch_claim_id, proposal_snapshot_digest,
                    target_reference_digest, content_digest, byte_size, media_type,
                    state, observed_content_digest, error_code, created_at, updated_at
             FROM artifact_effects INDEXED BY idx_artifact_effects_reconciliation
             WHERE state IN ('prepared', 'staged', 'unknown')
             ORDER BY state ASC, updated_at ASC, proposal_id ASC
             LIMIT ?1",
        )?;
        let records = statement
            .query_map([limit.clamp(1, 200)], Self::row_to_artifact_effect)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into);
        records
    }

    fn row_to_artifact_effect(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactEffectRecord> {
        fn parse_time(
            value: String,
            column: usize,
        ) -> rusqlite::Result<chrono::DateTime<chrono::Utc>> {
            chrono::DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&chrono::Utc))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        column,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
        }
        let state_text: String = row.get(7)?;
        let state = ArtifactEffectState::parse(&state_text).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                7,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    error.to_string(),
                )),
            )
        })?;
        let byte_size: i64 = row.get(5)?;
        let byte_size = u64::try_from(byte_size).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                5,
                rusqlite::types::Type::Integer,
                Box::new(error),
            )
        })?;
        Ok(ArtifactEffectRecord {
            proposal_id: row.get(0)?,
            dispatch_claim_id: row.get(1)?,
            proposal_snapshot_digest: row.get(2)?,
            target_reference_digest: row.get(3)?,
            content_digest: row.get(4)?,
            byte_size,
            media_type: row.get(6)?,
            state,
            observed_content_digest: row.get(8)?,
            error_code: row.get(9)?,
            created_at: parse_time(row.get(10)?, 10)?,
            updated_at: parse_time(row.get(11)?, 11)?,
        })
    }

    pub fn confirmed_projection_claim_id(&self, proposal_id: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT dispatch_claim_id
             FROM proposals
             WHERE id = ?1
               AND dispatch_state = 'confirmed_projection_pending'",
            [proposal_id],
            |row| row.get(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(Into::into)
    }

    /// Returns a bounded, indexed batch whose effect is durably confirmed but whose
    /// Proposal read-model status was not projected yet. Reading this queue never claims
    /// or executes an effect.
    pub fn list_confirmed_projection_pending(
        &self,
        limit: i64,
    ) -> Result<Vec<(AgentProposal, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                    affected_path, before_json, after_json, reason, confidence,
                    risk_level, status, created_at, resolved_at, expires_at,
                    dispatch_claim_id
             FROM proposals INDEXED BY idx_proposals_dispatch_reconciliation
             WHERE dispatch_state = 'confirmed_projection_pending'
               AND dispatch_claim_id IS NOT NULL
             ORDER BY dispatch_claimed_at ASC, id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit.clamp(1, 200)], |row| {
            Ok((Self::row_to_proposal(row)?, row.get::<_, String>(16)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn dispatch_claim_id(&self, proposal_id: &str) -> Result<Option<String>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT dispatch_claim_id FROM proposals WHERE id = ?1",
            [proposal_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map(Option::flatten)
        .map_err(Into::into)
    }

    pub fn list_terminal_owner_reconciliation_candidates(
        &self,
        limit: usize,
    ) -> Result<Vec<(AgentProposal, String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT proposal.id, proposal.run_id, proposal.proposal_type,
                    proposal.source, proposal.source_detail, proposal.base_hash,
                    proposal.affected_path, proposal.before_json, proposal.after_json,
                    proposal.reason, proposal.confidence, proposal.risk_level,
                    proposal.status, proposal.created_at, proposal.resolved_at,
                    proposal.expires_at, proposal.dispatch_claim_id,
                    proposal.dispatch_state
             FROM proposals proposal
             INNER JOIN proposal_terminal_owner_origins origin
                ON origin.proposal_id = proposal.id
             WHERE proposal.dispatch_claim_id IS NOT NULL
               AND (
                    proposal.dispatch_state = 'confirmed_projection_pending'
                    OR (
                        proposal.dispatch_state = 'claimed'
                        AND proposal.proposal_type = 'memory_write'
                    )
               )
             ORDER BY proposal.dispatch_claimed_at ASC, proposal.id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::try_from(limit.clamp(1, 250))?], |row| {
            Ok((
                Self::row_to_proposal(row)?,
                row.get::<_, String>(16)?,
                row.get::<_, String>(17)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Startup-only recovery input for an ExternalWriteAction whose dispatch
    /// claim committed before ArtifactMaterializer persisted its prepared
    /// intent. Since the product path always writes the artifact intent before
    /// staging bytes, absence of that row proves the effect was not attempted.
    pub fn list_claimed_external_writes_without_artifact_intent(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT proposal.id, proposal.dispatch_claim_id
             FROM proposals proposal
             LEFT JOIN artifact_effects artifact
                ON artifact.proposal_id = proposal.id
             WHERE proposal.proposal_type = 'external_write_action'
               AND proposal.dispatch_state = 'claimed'
               AND proposal.dispatch_claim_id IS NOT NULL
               AND artifact.proposal_id IS NULL
             ORDER BY proposal.dispatch_claimed_at ASC, proposal.id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::try_from(limit.clamp(1, 250))?], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Reload the exact Proposal snapshot only while the supplied acceptance
    /// dispatch claim is the canonical owner. This is the mechanical bridge
    /// used by ReviewWorkflow to issue a non-serializable acceptance proof;
    /// callers cannot turn a Proposal id or a deserialized claim string into
    /// authorization.
    pub(crate) fn claimed_dispatch_proposal(
        &self,
        proposal_id: &str,
        claim_id: &str,
    ) -> Result<Option<(AgentProposal, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                    affected_path, before_json, after_json, reason, confidence,
                    risk_level, status, created_at, resolved_at, expires_at,
                    dispatch_snapshot_digest
             FROM proposals
             WHERE id = ?1 AND dispatch_claim_id = ?2
               AND dispatch_state IN ('claimed', 'confirmed_projection_pending')
               AND dispatch_snapshot_digest IS NOT NULL",
        )?;
        statement
            .query_row(params![proposal_id, claim_id], |row| {
                Ok((Self::row_to_proposal(row)?, row.get::<_, String>(16)?))
            })
            .optional()
            .map_err(Into::into)
    }

    /// Reload the durable ReviewWorkflow fact that created an effect and bind
    /// it to the exact snapshot and dispatch claim captured before execution.
    /// This is intentionally crate-private: serialized task rows cannot call it
    /// to manufacture cloud authority.
    pub(crate) fn materialized_dispatch_proposal(
        &self,
        proposal_id: &str,
    ) -> Result<Option<(AgentProposal, String, String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                    affected_path, before_json, after_json, reason, confidence,
                    risk_level, status, created_at, resolved_at, expires_at,
                    dispatch_claim_id, dispatch_snapshot_digest, dispatch_state
             FROM proposals
             WHERE id = ?1
               AND dispatch_claim_id IS NOT NULL
               AND dispatch_snapshot_digest IS NOT NULL
               AND dispatch_state IN ('confirmed_projection_pending', 'confirmed')",
        )?;
        statement
            .query_row([proposal_id], |row| {
                Ok((
                    Self::row_to_proposal(row)?,
                    row.get::<_, String>(16)?,
                    row.get::<_, String>(17)?,
                    row.get::<_, String>(18)?,
                ))
            })
            .optional()
            .map_err(Into::into)
    }

    /// Atomically projects an already-confirmed effect into the Proposal read model and
    /// closes its dispatch receipt. This method cannot claim or dispatch an effect and is
    /// therefore safe to retry after a crash or projection failure.
    pub fn project_confirmed_effect(
        &self,
        proposal: &AgentProposal,
        claim_id: &str,
    ) -> Result<bool> {
        if proposal.status != ProposalStatus::Accepted || proposal.resolved_at.is_none() {
            anyhow::bail!("confirmed effect projection requires an accepted Proposal");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute(
            "UPDATE proposals
             SET affected_path = ?3,
                 status = 'accepted',
                 resolved_at = ?4,
                 dispatch_state = 'confirmed',
                 dispatch_error_code = NULL
             WHERE id = ?1
               AND dispatch_claim_id = ?2
               AND dispatch_state = 'confirmed_projection_pending'",
            params![
                proposal.id,
                claim_id,
                proposal.affected_path,
                proposal.resolved_at.map(|time| time.to_rfc3339()),
            ],
        )? == 1)
    }

    pub fn get_proposal(&self, id: &str) -> Result<Option<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals WHERE id = ?1"
        )?;
        let row = stmt.query_row([id], Self::row_to_proposal);
        match row {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_pending_proposals(&self, limit: i64) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             WHERE status = 'pending'
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;
        let proposals = stmt.query_map([limit], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn list_all_proposals(&self, limit: i64, offset: i64) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2"
        )?;
        let proposals = stmt.query_map([limit, offset], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Load accepted typed prerequisites for one terminal owner. Callers must
    /// still validate the proposal subtype and its exact external authority;
    /// this query only prevents product resume from scanning unrelated review
    /// history or guessing by display text.
    pub fn list_accepted_action_resume_prerequisites_for_task(
        &self,
        task_session_id: &str,
        limit: usize,
    ) -> Result<Vec<AgentProposal>> {
        if task_session_id.trim().is_empty() {
            anyhow::bail!("proposal terminal owner task id is empty");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT proposal.id, proposal.run_id, proposal.proposal_type,
                    proposal.source, proposal.source_detail, proposal.base_hash,
                    proposal.affected_path, proposal.before_json, proposal.after_json,
                    proposal.reason, proposal.confidence, proposal.risk_level,
                    proposal.status, proposal.created_at, proposal.resolved_at,
                    proposal.expires_at
             FROM proposals proposal
             INNER JOIN proposal_terminal_owner_origins origin
                ON origin.proposal_id = proposal.id
             INNER JOIN proposal_terminal_owner_relations relation
                ON relation.proposal_id = proposal.id
             WHERE origin.task_session_id = ?1
               AND relation.relation_kind = 'action_resume_prerequisite'
               AND proposal.status = 'accepted'
               AND proposal.dispatch_state = 'confirmed'
             ORDER BY proposal.resolved_at DESC, proposal.id ASC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![task_session_id, i64::try_from(limit.clamp(1, 64))?],
            Self::row_to_proposal,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_proposals_filtered(
        &self,
        status: Option<ProposalStatus>,
        proposal_type: Option<ProposalType>,
        risk_level: Option<RiskLevel>,
        limit: i64,
    ) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;

        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            conditions.push("status = ?".to_string());
            params.push(Box::new(s.to_string()));
        }
        if let Some(t) = proposal_type {
            conditions.push("proposal_type = ?".to_string());
            params.push(Box::new(t.to_string()));
        }
        if let Some(r) = risk_level {
            conditions.push("risk_level = ?".to_string());
            params.push(Box::new(r.to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             {}
             ORDER BY created_at DESC
             LIMIT ?",
            where_clause
        );

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let proposals = stmt.query_map(
            rusqlite::params_from_iter(
                param_refs
                    .iter()
                    .copied()
                    .chain(std::iter::once(&limit as &dyn rusqlite::ToSql)),
            ),
            Self::row_to_proposal,
        )?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn count_by_status_and_risk(
        &self,
        status: ProposalStatus,
        min_risk: Option<RiskLevel>,
    ) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;

        let count: i64 = if let Some(risk) = min_risk {
            match risk {
                RiskLevel::Low => {
                    conn.query_row(
                        "SELECT COUNT(*) FROM proposals WHERE status = ?1 AND risk_level IN ('low', 'medium', 'high', 'critical')",
                        params![status.to_string()],
                        |row| row.get(0),
                    )?
                }
                RiskLevel::Medium => {
                    conn.query_row(
                        "SELECT COUNT(*) FROM proposals WHERE status = ?1 AND risk_level IN ('medium', 'high', 'critical')",
                        params![status.to_string()],
                        |row| row.get(0),
                    )?
                }
                RiskLevel::High => {
                    conn.query_row(
                        "SELECT COUNT(*) FROM proposals WHERE status = ?1 AND risk_level IN ('high', 'critical')",
                        params![status.to_string()],
                        |row| row.get(0),
                    )?
                }
                RiskLevel::Critical => {
                    conn.query_row(
                        "SELECT COUNT(*) FROM proposals WHERE status = ?1 AND risk_level = 'critical'",
                        params![status.to_string()],
                        |row| row.get(0),
                    )?
                }
            }
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM proposals WHERE status = ?1",
                params![status.to_string()],
                |row| row.get(0),
            )?
        };
        Ok(count)
    }

    pub fn pending_count(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM proposals WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_proposals_by_run_id(&self, run_id: &str) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             WHERE run_id = ?1
             ORDER BY created_at DESC"
        )?;
        let proposals = stmt.query_map([run_id], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    /// Cleanup expired proposals and return count
    pub fn cleanup_expired_proposals(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE proposals
             SET status = 'expired', resolved_at = ?1
             WHERE status = 'pending'
               AND dispatch_state IN ('unclaimed', 'failed_before_effect')
               AND expires_at < ?1",
            [&now],
        )?;
        Ok(rows)
    }

    /// List proposals expiring within given days
    pub fn list_expiring_soon(&self, days: i64) -> Result<Vec<AgentProposal>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let cutoff = (chrono::Utc::now() + chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, run_id, proposal_type, source, source_detail, base_hash, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at
             FROM proposals
             WHERE status = 'pending' AND expires_at < ?1
             ORDER BY expires_at ASC"
        )?;
        let proposals = stmt.query_map([&cutoff], Self::row_to_proposal)?;
        proposals
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    fn row_to_proposal(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentProposal> {
        let run_id: Option<String> = row.get(1)?;
        let type_str: String = row.get(2)?;
        let source: ProposalSource = row.get(3)?;
        let source_detail: Option<String> = row.get(4)?;
        let base_hash: Option<String> = row.get(5)?;
        let before_json: Option<String> = row.get(7)?;
        let after_json: String = row.get(8)?;
        let risk_str: String = row.get(11)?;
        let status_str: String = row.get(12)?;
        let created_at_str: String = row.get(13)?;
        let resolved_at_str: Option<String> = row.get(14)?;
        let expires_at_str: Option<String> = row.get(15)?;

        let proposal_type = match type_str.as_str() {
            "goal_update" => ProposalType::GoalUpdate,
            "state_update" => ProposalType::StateUpdate,
            "preference_update" => ProposalType::PreferenceUpdate,
            "capability_update" => ProposalType::CapabilityUpdate,
            "memory_write" => ProposalType::MemoryWrite,
            "memory_archive" => ProposalType::MemoryArchive,
            "tool_permission" => ProposalType::ToolPermission,
            "plugin_permission" => ProposalType::PluginPermission,
            "scheduled_task" => ProposalType::ScheduledTask,
            "external_write_action" => ProposalType::ExternalWriteAction,
            "model_policy_change" => ProposalType::ModelPolicyChange,
            "data_export" => ProposalType::DataExport,
            "schedule_checkin" => ProposalType::ScheduleCheckin,
            "unsupported" => ProposalType::Unsupported,
            "life_model_update" => ProposalType::LifeModelUpdate,
            _ => ProposalType::Unsupported,
        };

        let risk_level = match risk_str.as_str() {
            "low" => RiskLevel::Low,
            "medium" => RiskLevel::Medium,
            "high" => RiskLevel::High,
            "critical" => RiskLevel::Critical,
            _ => RiskLevel::Medium,
        };

        let status = match status_str.as_str() {
            "pending" => ProposalStatus::Pending,
            "accepted" => ProposalStatus::Accepted,
            "rejected" => ProposalStatus::Rejected,
            "edited" => ProposalStatus::Edited,
            "postponed" => ProposalStatus::Postponed,
            "expired" => ProposalStatus::Expired,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("unsupported proposal status: {other}"),
                    )),
                ));
            }
        };

        let before = before_json.and_then(|s| serde_json::from_str(&s).ok());
        let after = serde_json::from_str(&after_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    13,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&chrono::Utc);
        let resolved_at = resolved_at_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let expires_at = expires_at_str
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        Ok(AgentProposal {
            id: row.get(0)?,
            run_id,
            proposal_type,
            source,
            source_detail,
            base_hash,
            affected_path: row.get(6)?,
            before,
            after,
            reason: row.get(9)?,
            confidence: row.get(10)?,
            risk_level,
            status,
            created_at,
            resolved_at,
            expires_at,
        })
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn validate_terminal_relation_submission(
    review_idempotency_key: &str,
    task_session_id: &str,
    run_id: &str,
    epoch_id: &str,
    epoch_generation: u64,
    admission_id: &str,
    canonical_user_message_ref: &str,
    canonical_user_message_digest: &str,
    canonical_store_identity: &str,
    relation_kind: ProposalTerminalRelationKind,
    target: ProposalTerminalRelationTargetContract<'_>,
) -> Result<()> {
    if review_idempotency_key.is_empty()
        || review_idempotency_key.len() > 512
        || task_session_id.trim().is_empty()
        || run_id.trim().is_empty()
        || epoch_id.trim().is_empty()
        || epoch_generation == 0
        || admission_id.trim().is_empty()
        || canonical_user_message_ref.trim().is_empty()
        || canonical_user_message_digest.trim().is_empty()
        || canonical_store_identity.trim().is_empty()
    {
        anyhow::bail!("proposal_terminal_relation_submission_invalid");
    }
    if relation_kind == ProposalTerminalRelationKind::LegacyUnclassified {
        anyhow::bail!("legacy_unclassified_relation_requires_migration");
    }
    target.validate_for(relation_kind)?;
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn terminal_owner_origin_matches(
    origin: &TerminalOwnerOriginBinding,
    task_session_id: &str,
    run_id: &str,
    epoch_id: &str,
    epoch_generation: u64,
    admission_id: &str,
    canonical_user_message_ref: &str,
    canonical_user_message_digest: &str,
    canonical_store_identity: &str,
) -> bool {
    origin.task_session_id == task_session_id
        && origin.run_id == run_id
        && origin.epoch_id == epoch_id
        && origin.epoch_generation == epoch_generation
        && origin.admission_id == admission_id
        && origin.canonical_user_message_ref == canonical_user_message_ref
        && origin.canonical_user_message_digest == canonical_user_message_digest
        && origin.canonical_store_identity == canonical_store_identity
}

fn terminal_relation_proposal_identity_matches(
    existing: &AgentProposal,
    requested: &AgentProposal,
) -> bool {
    existing.proposal_type == requested.proposal_type
        && existing.source == requested.source
        && existing.affected_path == requested.affected_path
        && existing.base_hash == requested.base_hash
        && existing.before == requested.before
        && existing.risk_level == requested.risk_level
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
fn proposal_terminal_relation_digest(
    proposal_id: &str,
    relation_kind: ProposalTerminalRelationKind,
    task_session_id: &str,
    run_id: &str,
    epoch_id: &str,
    epoch_generation: u64,
    admission_id: &str,
    canonical_user_message_ref: &str,
    canonical_user_message_digest: &str,
    canonical_store_identity: &str,
    target: Option<ProposalTerminalRelationTargetContract<'_>>,
) -> Result<String> {
    let target = target.map(|target| {
        serde_json::json!({
            "targetBindingDigest": target.target_binding_digest,
            "agentRunStoreIdentityDigest": target.agent_run_store_identity_digest,
            "targetOwnerRevision": target.target_owner_revision,
            "targetStatusAtIssue": target.target_status_at_issue.to_string(),
        })
    });
    let material = serde_json::json!({
        "schema": "openlife.proposalTerminalRelation.v1",
        "proposalId": proposal_id,
        "relationKind": relation_kind.as_str(),
        "originBinding": {
            "taskSessionId": task_session_id,
            "runId": run_id,
            "epochId": epoch_id,
            "epochGeneration": epoch_generation,
            "admissionId": admission_id,
            "canonicalUserMessageRef": canonical_user_message_ref,
            "canonicalUserMessageDigest": canonical_user_message_digest,
            "canonicalStoreIdentity": canonical_store_identity,
        },
        "target": target,
    });
    let serialized = serde_json::to_string(&material)
        .context("proposal terminal relation digest serialization failed")?;
    Ok(crate::persistence_outbox::metadata_digest(&serialized))
}

#[expect(
    clippy::too_many_arguments,
    reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
)]
pub(super) fn proposal_terminal_relation_storage_request_digest(
    proposal: &AgentProposal,
    review_idempotency_key: &str,
    task_session_id: &str,
    run_id: &str,
    epoch_id: &str,
    epoch_generation: u64,
    admission_id: &str,
    canonical_user_message_ref: &str,
    canonical_user_message_digest: &str,
    canonical_store_identity: &str,
    relation_kind: ProposalTerminalRelationKind,
    target_binding_digest: &str,
    agent_run_store_identity_digest: &str,
    target_owner_revision: u64,
    target_status_at_issue: crate::agent::AgentRunStatus,
) -> Result<String> {
    let target = ProposalTerminalRelationTargetContract {
        target_binding_digest,
        agent_run_store_identity_digest,
        target_owner_revision,
        target_status_at_issue,
    };
    target.validate_for(relation_kind)?;
    let material = serde_json::json!({
        "schema": "openlife.proposalTerminalRelationStorageRequest.v1",
        "idempotencyKeyDigest": crate::persistence_outbox::metadata_digest(review_idempotency_key),
        "proposalPolicyIdentity": {
            "proposalType": proposal.proposal_type.to_string(),
            "source": proposal.source.to_string(),
            "sourceDetail": proposal.source_detail.as_deref(),
            "affectedPath": proposal.affected_path.as_str(),
            "baseHash": proposal.base_hash.as_deref(),
            "before": proposal.before.as_ref(),
            "after": &proposal.after,
            "reason": proposal.reason.as_str(),
            "confidence": proposal.confidence,
            "riskLevel": proposal.risk_level.to_string(),
            "status": proposal.status.to_string(),
            "createdAt": proposal.created_at.to_rfc3339(),
            "resolvedAt": proposal.resolved_at.as_ref().map(|value| value.to_rfc3339()),
            "expiresAt": proposal.expires_at.as_ref().map(|value| value.to_rfc3339()),
        },
        "relationKind": relation_kind.as_str(),
        "target": {
            "targetBindingDigest": target.target_binding_digest,
            "agentRunStoreIdentityDigest": target.agent_run_store_identity_digest,
            "targetOwnerRevision": target.target_owner_revision,
            "targetStatusAtIssue": target.target_status_at_issue.to_string(),
        },
        "origin": {
            "taskSessionId": task_session_id,
            "runId": run_id,
            "epochId": epoch_id,
            "epochGeneration": epoch_generation,
            "admissionId": admission_id,
            "canonicalUserMessageRef": canonical_user_message_ref,
            "canonicalUserMessageDigest": canonical_user_message_digest,
            "canonicalStoreIdentity": canonical_store_identity,
        },
    });
    Ok(crate::persistence_outbox::metadata_digest(
        &serde_json::to_string(&material)
            .context("proposal terminal relation storage request digest serialization failed")?,
    ))
}

fn terminal_owner_relation_from_conn(
    conn: &Connection,
    proposal_id: &str,
) -> Result<Option<ProposalTerminalRelationRecord>> {
    let row = conn
        .query_row(
            "SELECT proposal_id, relation_kind, relation_digest,
                    target_binding_digest, agent_run_store_identity_digest,
                    target_owner_revision, target_status_at_issue,
                    link_outbox_event_id, created_at
             FROM proposal_terminal_owner_relations WHERE proposal_id = ?1",
            [proposal_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        proposal_id,
        relation_kind,
        relation_digest,
        target_binding_digest,
        agent_run_store_identity_digest,
        target_owner_revision,
        target_status_at_issue,
        link_outbox_event_id,
        created_at,
    )) = row
    else {
        return Ok(None);
    };
    let relation_kind = match relation_kind.as_str() {
        "non_blocking_successor" => ProposalTerminalRelationKind::NonBlockingSuccessor,
        "effect_blocking_prerequisite" => ProposalTerminalRelationKind::EffectBlockingPrerequisite,
        "action_resume_prerequisite" => ProposalTerminalRelationKind::ActionResumePrerequisite,
        "legacy_unclassified" => ProposalTerminalRelationKind::LegacyUnclassified,
        other => anyhow::bail!("unsupported proposal terminal relation kind: {other}"),
    };
    let target_owner_revision = target_owner_revision
        .map(u64::try_from)
        .transpose()
        .context("proposal terminal relation target revision is invalid")?;
    let target_status_at_issue = target_status_at_issue
        .as_deref()
        .map(terminal_relation_status_at_issue)
        .transpose()?;
    let legacy_shape = link_outbox_event_id.is_none()
        && target_binding_digest.is_none()
        && agent_run_store_identity_digest.is_none()
        && target_owner_revision.is_none()
        && target_status_at_issue.is_none();
    let typed_shape = link_outbox_event_id.is_some()
        && target_binding_digest.is_some()
        && agent_run_store_identity_digest.is_some()
        && target_owner_revision.is_some_and(|revision| revision > 0)
        && target_status_at_issue.is_some();
    if (relation_kind == ProposalTerminalRelationKind::LegacyUnclassified && !legacy_shape)
        || (relation_kind != ProposalTerminalRelationKind::LegacyUnclassified && !typed_shape)
    {
        anyhow::bail!("proposal terminal relation outbox invariant violated");
    }
    if relation_kind != ProposalTerminalRelationKind::LegacyUnclassified {
        ProposalTerminalRelationTargetContract {
            target_binding_digest: target_binding_digest.as_deref().unwrap_or_default(),
            agent_run_store_identity_digest: agent_run_store_identity_digest
                .as_deref()
                .unwrap_or_default(),
            target_owner_revision: target_owner_revision.unwrap_or_default(),
            target_status_at_issue: target_status_at_issue
                .unwrap_or(crate::agent::AgentRunStatus::Failed),
        }
        .validate_for(relation_kind)?;
    }
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
        .context("proposal terminal relation created_at is invalid")?
        .with_timezone(&chrono::Utc);
    Ok(Some(ProposalTerminalRelationRecord {
        proposal_id,
        relation_kind,
        relation_digest,
        target_binding_digest,
        agent_run_store_identity_digest,
        target_owner_revision,
        target_status_at_issue,
        link_outbox_event_id,
        created_at,
    }))
}

fn terminal_relation_status_at_issue(value: &str) -> Result<crate::agent::AgentRunStatus> {
    match value {
        "running" => Ok(crate::agent::AgentRunStatus::Running),
        "waiting_permission" => Ok(crate::agent::AgentRunStatus::WaitingPermission),
        "completed" => Ok(crate::agent::AgentRunStatus::Completed),
        "failed" => Ok(crate::agent::AgentRunStatus::Failed),
        "remote_unknown" => Ok(crate::agent::AgentRunStatus::RemoteUnknown),
        "cancelled" => Ok(crate::agent::AgentRunStatus::Cancelled),
        _ => anyhow::bail!("proposal_terminal_relation_target_status_invalid"),
    }
}

fn validate_existing_terminal_relation_target_contract(
    tx: &rusqlite::Transaction<'_>,
) -> Result<()> {
    let legacy_invalid: i64 = tx.query_row(
        "SELECT COUNT(*) FROM proposal_terminal_owner_relations
         WHERE relation_kind = 'legacy_unclassified'
           AND (link_outbox_event_id IS NOT NULL
                OR target_binding_digest IS NOT NULL
                OR agent_run_store_identity_digest IS NOT NULL
                OR target_owner_revision IS NOT NULL
                OR target_status_at_issue IS NOT NULL)",
        [],
        |row| row.get(0),
    )?;
    if legacy_invalid != 0 {
        anyhow::bail!("proposal_terminal_relation_legacy_contract_invalid");
    }

    let typed = {
        let mut statement = tx.prepare(
            "SELECT proposal_id, relation_kind, target_binding_digest,
                    agent_run_store_identity_digest, target_owner_revision,
                    target_status_at_issue, link_outbox_event_id
             FROM proposal_terminal_owner_relations
             WHERE relation_kind != 'legacy_unclassified'
             ORDER BY proposal_id ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    for (
        _proposal_id,
        relation_kind,
        target_binding_digest,
        agent_run_store_identity_digest,
        target_owner_revision,
        target_status_at_issue,
        link_outbox_event_id,
    ) in typed
    {
        let relation_kind = match relation_kind.as_str() {
            "non_blocking_successor" => ProposalTerminalRelationKind::NonBlockingSuccessor,
            "effect_blocking_prerequisite" => {
                ProposalTerminalRelationKind::EffectBlockingPrerequisite
            }
            "action_resume_prerequisite" => ProposalTerminalRelationKind::ActionResumePrerequisite,
            _ => anyhow::bail!("proposal_terminal_relation_target_contract_missing"),
        };
        let target_binding_digest =
            target_binding_digest.context("proposal_terminal_relation_target_contract_missing")?;
        let agent_run_store_identity_digest = agent_run_store_identity_digest
            .context("proposal_terminal_relation_target_contract_missing")?;
        let target_owner_revision = target_owner_revision
            .and_then(|value| u64::try_from(value).ok())
            .context("proposal_terminal_relation_target_contract_missing")?;
        let target_status_at_issue = terminal_relation_status_at_issue(
            target_status_at_issue
                .as_deref()
                .context("proposal_terminal_relation_target_contract_missing")?,
        )?;
        let event_id =
            link_outbox_event_id.context("proposal_terminal_relation_target_contract_missing")?;
        ProposalTerminalRelationTargetContract {
            target_binding_digest: &target_binding_digest,
            agent_run_store_identity_digest: &agent_run_store_identity_digest,
            target_owner_revision,
            target_status_at_issue,
        }
        .validate_for(relation_kind)
        .map_err(|_| anyhow::anyhow!("proposal_terminal_relation_target_contract_missing"))?;
        let expected_target = relation_kind
            .projection_target()
            .context("proposal_terminal_relation_target_contract_missing")?;
        let delivery_counts = tx.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN projection_target = ?2 THEN 1 ELSE 0 END), 0)
             FROM canonical_outbox_deliveries WHERE event_id = ?1",
            params![event_id, expected_target],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?;
        if delivery_counts != (1, 1) {
            anyhow::bail!("proposal_terminal_relation_target_contract_missing");
        }
    }
    Ok(())
}

fn backfill_legacy_terminal_relations(tx: &rusqlite::Transaction<'_>) -> Result<()> {
    let missing = {
        let mut statement = tx.prepare(
            "SELECT origin.proposal_id, origin.task_session_id, origin.run_id,
                    origin.epoch_id, origin.epoch_generation, origin.admission_id,
                    origin.canonical_user_message_ref,
                    origin.canonical_user_message_digest,
                    origin.canonical_store_identity, origin.created_at
             FROM proposal_terminal_owner_origins origin
             LEFT JOIN proposal_terminal_owner_relations relation
               ON relation.proposal_id = origin.proposal_id
             WHERE relation.proposal_id IS NULL
             ORDER BY origin.proposal_id ASC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    for (
        proposal_id,
        task_session_id,
        run_id,
        epoch_id,
        epoch_generation,
        admission_id,
        canonical_user_message_ref,
        canonical_user_message_digest,
        canonical_store_identity,
        created_at,
    ) in missing
    {
        let epoch_generation = u64::try_from(epoch_generation)
            .context("legacy proposal terminal origin epoch generation is invalid")?;
        let relation_digest = proposal_terminal_relation_digest(
            &proposal_id,
            ProposalTerminalRelationKind::LegacyUnclassified,
            &task_session_id,
            &run_id,
            &epoch_id,
            epoch_generation,
            &admission_id,
            &canonical_user_message_ref,
            &canonical_user_message_digest,
            &canonical_store_identity,
            None,
        )?;
        tx.execute(
            "INSERT INTO proposal_terminal_owner_relations (
                proposal_id, relation_kind, relation_digest,
                link_outbox_event_id, created_at
             ) VALUES (?1, 'legacy_unclassified', ?2, NULL, ?3)",
            params![proposal_id, relation_digest, created_at],
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn terminal_relation_commit_failpoints(
) -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static FAILPOINTS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    FAILPOINTS.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn validate_relation_outbox_from_conn(
    conn: &Connection,
    relation: &ProposalTerminalRelationRecord,
) -> Result<()> {
    let event_id = relation
        .link_outbox_event_id
        .as_deref()
        .context("owned proposal terminal relation lost its outbox event")?;
    let event = crate::persistence_outbox::mutation_by_event_id(conn, event_id)?
        .context("proposal terminal relation outbox event missing")?;
    if event.aggregate_kind != "proposal_terminal_relation"
        || event.aggregate_id != relation.proposal_id
        || event.mutation_kind != "linked"
        || event.payload_digest != relation.relation_digest
    {
        anyhow::bail!("proposal_terminal_relation_outbox_mismatch");
    }
    let expected_target = relation
        .relation_kind
        .projection_target()
        .context("owned proposal terminal relation projection target missing")?;
    let target_counts = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN projection_target = ?2 THEN 1 ELSE 0 END), 0)
         FROM canonical_outbox_deliveries WHERE event_id = ?1",
        params![event_id, expected_target],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    if target_counts != (1, 1) {
        anyhow::bail!("proposal_terminal_relation_outbox_target_missing");
    }
    Ok(())
}

fn terminal_owner_origin_binding_from_conn(
    conn: &Connection,
    proposal_id: &str,
) -> Result<Option<TerminalOwnerOriginBinding>> {
    let row = conn
        .query_row(
            "SELECT proposal_id, task_session_id, run_id, epoch_id,
                    epoch_generation, admission_id, canonical_user_message_ref,
                    canonical_user_message_digest, canonical_store_identity
             FROM proposal_terminal_owner_origins WHERE proposal_id = ?1",
            [proposal_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        proposal_id,
        task_session_id,
        run_id,
        epoch_id,
        epoch_generation,
        admission_id,
        canonical_user_message_ref,
        canonical_user_message_digest,
        canonical_store_identity,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(TerminalOwnerOriginBinding {
        proposal_id,
        task_session_id,
        run_id,
        epoch_id,
        epoch_generation: u64::try_from(epoch_generation)
            .context("terminal owner epoch generation is negative")?,
        admission_id,
        canonical_user_message_ref,
        canonical_user_message_digest,
        canonical_store_identity,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal_relation_candidate(path: &str, content: &str) -> AgentProposal {
        AgentProposal::new(
            ProposalType::MemoryWrite,
            path,
            serde_json::json!({"content": content}),
            "Review this candidate.",
            0.8,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        )
    }

    fn submit_terminal_relation(
        store: &ProposalStore,
        proposal: &AgentProposal,
        idempotency_key: &str,
        origin_label: &str,
        kind: ProposalTerminalRelationKind,
    ) -> Result<super::super::review_workflow::ProposalTerminalRelationSubmitOutcome> {
        let origin =
            super::super::review_workflow::terminal_owner_review_origin_fixture(origin_label);
        let target = super::super::store::agent_run_terminal_relation_target_fixture(&origin);
        let request = super::super::review_workflow::DurableWriteRequest::from_agent_proposal(
            super::super::review_workflow::DurableWriteSource::MainChat,
            super::super::review_workflow::DurableWriteSubject::Memory,
            proposal.clone(),
            "Memory proposal is ready for Review Center approval.",
        )
        .with_idempotency_key(idempotency_key);
        super::super::review_workflow::ReviewWorkflow::new(store)
            .submit_with_terminal_owner_relation(
            request,
            &origin,
            kind,
            &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
            &target,
        )
    }

    #[test]
    fn proposal_relation_store_transaction_rolls_back_every_owned_row_together() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = terminal_relation_candidate(
            "memory.pending.atomic-rollback",
            "SENTINEL_BODY_MUST_NOT_SURVIVE_ROLLBACK",
        );
        store
            .fail_next_terminal_relation_commit_for_test("relation:atomic-rollback")
            .unwrap();

        let error = submit_terminal_relation(
            &store,
            &proposal,
            "relation:atomic-rollback",
            "atomic-rollback",
            ProposalTerminalRelationKind::EffectBlockingPrerequisite,
        )
        .expect_err("failpoint must interrupt before the IMMEDIATE transaction commit");
        assert_eq!(
            error.to_string(),
            "proposal_terminal_relation_commit_failpoint"
        );

        let conn = store.conn.lock().unwrap();
        for table in [
            "proposals",
            "proposal_terminal_owner_origins",
            "proposal_terminal_owner_relations",
            "canonical_outbox_events",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table} leaked a partial transaction row");
        }
        let delivery_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_deliveries delivery
                 JOIN canonical_outbox_events event ON event.event_id = delivery.event_id
                 WHERE event.aggregate_kind = 'proposal_terminal_relation'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(delivery_count, 0);
    }

    #[test]
    fn proposal_relation_store_reopen_replays_the_same_outbox_event() {
        let path = std::env::temp_dir().join(format!(
            "openlife-proposal-terminal-relation-reopen-{}.db",
            uuid::Uuid::new_v4()
        ));
        let proposal = terminal_relation_candidate("memory.pending.reopen", "stable fact");
        let first_event_id = {
            let store = ProposalStore::new(&path).unwrap();
            match submit_terminal_relation(
                &store,
                &proposal,
                "relation:reopen",
                "reopen",
                ProposalTerminalRelationKind::NonBlockingSuccessor,
            )
            .unwrap()
            {
                super::super::review_workflow::ProposalTerminalRelationSubmitOutcome::CreatedOwned { relation, .. } => {
                    relation.link_outbox_event_id().unwrap().to_string()
                }
                other => panic!("unexpected first outcome: {other:?}"),
            }
        };

        let store = ProposalStore::new(&path).unwrap();
        let replay = submit_terminal_relation(
            &store,
            &proposal,
            "relation:reopen",
            "reopen",
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap();
        match replay {
            super::super::review_workflow::ProposalTerminalRelationSubmitOutcome::ReplayedSameOrigin { relation, .. } => {
                assert_eq!(
                    relation.link_outbox_event_id(),
                    Some(first_event_id.as_str())
                );
            }
            other => panic!("unexpected replay outcome: {other:?}"),
        }
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn proposal_relation_store_migrates_existing_origin_to_legacy_without_outbox_credit() {
        let path = std::env::temp_dir().join(format!(
            "openlife-proposal-terminal-relation-legacy-{}.db",
            uuid::Uuid::new_v4()
        ));
        let proposal = terminal_relation_candidate("memory.pending.legacy", "legacy fact");
        {
            let store = ProposalStore::new(&path).unwrap();
            store.create_proposal(&proposal).unwrap();
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO proposal_terminal_owner_origins (
                    proposal_id, task_session_id, run_id, epoch_id, epoch_generation,
                    admission_id, canonical_user_message_ref,
                    canonical_user_message_digest, canonical_store_identity, created_at
                 ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, 'legacy_unverified', ?8)",
                params![
                    proposal.id,
                    "task:legacy",
                    "run:legacy",
                    "epoch:legacy",
                    "admission:legacy",
                    "message:legacy",
                    format!("sha256:{:0>64}", 6),
                    chrono::Utc::now().to_rfc3339(),
                ],
            )
            .unwrap();
        }

        let store = ProposalStore::new(&path).unwrap();
        let relation = store
            .terminal_owner_relation(&proposal.id)
            .unwrap()
            .expect("existing origin must be explicitly quarantined");
        assert_eq!(
            relation.relation_kind(),
            ProposalTerminalRelationKind::LegacyUnclassified
        );
        assert!(relation.link_outbox_event_id().is_none());
        assert!(relation.relation_digest().starts_with("sha256:"));
        let conn = store.conn.lock().unwrap();
        let outbox_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM canonical_outbox_events WHERE aggregate_id = ?1",
                [&proposal.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            outbox_count, 0,
            "legacy migration must not fake link credit"
        );
        drop(conn);
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn proposal_relation_store_open_fails_closed_for_unverifiable_old_typed_relation() {
        let path = std::env::temp_dir().join(format!(
            "openlife-proposal-terminal-relation-unverifiable-{}.db",
            uuid::Uuid::new_v4()
        ));
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE proposal_terminal_owner_relations (
                    proposal_id TEXT PRIMARY KEY,
                    relation_kind TEXT NOT NULL,
                    relation_digest TEXT NOT NULL,
                    link_outbox_event_id TEXT,
                    created_at TEXT NOT NULL
                 ) WITHOUT ROWID;
                 INSERT INTO proposal_terminal_owner_relations (
                    proposal_id, relation_kind, relation_digest,
                    link_outbox_event_id, created_at
                 ) VALUES (
                    'legacy-wip-proposal', 'effect_blocking_prerequisite',
                    'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                    'outbox:legacy-generic-target', '2026-01-01T00:00:00Z'
                 );",
            )
            .unwrap();
        }

        let error = match ProposalStore::new(&path) {
            Ok(_) => panic!("unverifiable typed relation must not be opened as current truth"),
            Err(error) => error,
        };
        assert_eq!(
            error.to_string(),
            "proposal_terminal_relation_target_contract_missing"
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn proposal_relation_store_outbox_contains_no_canonical_body_copy() {
        let store = ProposalStore::new_in_memory().unwrap();
        let sentinel = "SENTINEL_CANONICAL_PROPOSAL_BODY_42";
        let proposal = terminal_relation_candidate("memory.pending.body", sentinel);
        let relation = match submit_terminal_relation(
            &store,
            &proposal,
            "relation:body-absence",
            "body-absence",
            ProposalTerminalRelationKind::NonBlockingSuccessor,
        )
        .unwrap()
        {
            super::super::review_workflow::ProposalTerminalRelationSubmitOutcome::CreatedOwned { relation, .. } => relation,
            other => panic!("unexpected relation outcome: {other:?}"),
        };
        let event_id = relation.link_outbox_event_id().unwrap();
        let conn = store.conn.lock().unwrap();
        let metadata: String = conn
            .query_row(
                "SELECT relation.proposal_id || ':' || relation.relation_kind || ':' ||
                        relation.relation_digest || ':' || relation.link_outbox_event_id || ':' ||
                        event.aggregate_kind || ':' || event.aggregate_id || ':' ||
                        event.mutation_kind || ':' || event.payload_digest || ':' ||
                        delivery.projection_target
                 FROM proposal_terminal_owner_relations relation
                 JOIN canonical_outbox_events event
                   ON event.event_id = relation.link_outbox_event_id
                 JOIN canonical_outbox_deliveries delivery
                   ON delivery.event_id = event.event_id
                 WHERE relation.proposal_id = ?1 AND event.event_id = ?2",
                params![relation.proposal_id(), event_id],
                |row| row.get(0),
            )
            .unwrap();
        let outbox_aggregate_id: String = conn
            .query_row(
                "SELECT aggregate_id FROM canonical_outbox_events WHERE event_id = ?1",
                [event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!metadata.contains(sentinel));
        assert_eq!(outbox_aggregate_id, relation.proposal_id());
        assert_ne!(outbox_aggregate_id, proposal.id);
        assert!(uuid::Uuid::parse_str(&outbox_aggregate_id).is_ok());
        assert!(relation.relation_digest().starts_with("sha256:"));
    }

    #[test]
    fn proposal_relation_store_persists_exact_target_owner_metadata_and_kind_specific_outbox() {
        let cases = [
            (
                ProposalTerminalRelationKind::NonBlockingSuccessor,
                "agent_run_review_link.non_blocking",
            ),
            (
                ProposalTerminalRelationKind::EffectBlockingPrerequisite,
                "agent_run_review_link.effect_blocking",
            ),
            (
                ProposalTerminalRelationKind::ActionResumePrerequisite,
                "agent_run_review_link.action_resume",
            ),
        ];
        for (index, (kind, expected_target)) in cases.into_iter().enumerate() {
            let store = ProposalStore::new_in_memory().unwrap();
            let origin = super::super::review_workflow::terminal_owner_review_origin_fixture(
                &format!("target-owner-{index}"),
            );
            let target = super::super::store::agent_run_terminal_relation_target_fixture(&origin);
            let proposal = terminal_relation_candidate(
                &format!("memory.pending.target-owner-{index}"),
                "TARGET_OWNER_SENTINEL_BODY_MUST_NOT_BE_PERSISTED",
            );
            let request = super::super::review_workflow::DurableWriteRequest::from_agent_proposal(
                super::super::review_workflow::DurableWriteSource::MainChat,
                super::super::review_workflow::DurableWriteSubject::Memory,
                proposal,
                "Memory proposal is ready for Review Center approval.",
            )
            .with_idempotency_key(format!("relation:target-owner:{index}"));
            let outcome = super::super::review_workflow::ReviewWorkflow::new(&store)
                .submit_with_terminal_owner_relation(
                    request,
                    &origin,
                    kind,
                    &crate::agent::canonical_write_admission::DeterministicFixtureCanonicalWriteAdmission,
                    &target,
                )
                .unwrap();
            let relation = outcome.owned_relation().unwrap();
            let conn = store.conn.lock().unwrap();
            let persisted = conn
                .query_row(
                    "SELECT relation.target_binding_digest,
                            relation.agent_run_store_identity_digest,
                            relation.target_owner_revision,
                            relation.target_status_at_issue,
                            delivery.projection_target
                     FROM proposal_terminal_owner_relations relation
                     JOIN canonical_outbox_deliveries delivery
                       ON delivery.event_id = relation.link_outbox_event_id
                     WHERE relation.proposal_id = ?1",
                    [relation.proposal_id()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, u64>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(persisted.0, target.target_binding_digest());
            assert_eq!(persisted.1, target.agent_run_store_identity_digest());
            assert_eq!(persisted.2, target.owner_revision());
            assert_eq!(persisted.3, target.status_at_issue().to_string());
            assert_eq!(persisted.4, expected_target);
            assert!(!format!("{persisted:?}")
                .contains("TARGET_OWNER_SENTINEL_BODY_MUST_NOT_BE_PERSISTED"));
        }
    }

    #[test]
    fn test_create_and_get_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("New Name"),
            "Builder suggested new name",
            0.85,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&proposal).unwrap();

        let fetched = store.get_proposal(&proposal.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.id, proposal.id);
        assert_eq!(fetched.status, ProposalStatus::Pending);
        assert_eq!(fetched.source, ProposalSource::BuilderReview);
        assert!(fetched.expires_at.is_some());
    }

    #[test]
    fn test_create_and_get_chat_conversation_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory.candidates",
            serde_json::json!({ "content": "prefers concise replies" }),
            "Chat conversation suggested a memory candidate",
            0.72,
            RiskLevel::Medium,
            ProposalSource::ChatConversation,
        );
        store.create_proposal(&proposal).unwrap();

        let fetched = store.get_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(fetched.id, proposal.id);
        assert_eq!(fetched.proposal_type, ProposalType::MemoryWrite);
        assert_eq!(fetched.source, ProposalSource::ChatConversation);
        assert_eq!(fetched.after, proposal.after);
        assert!(fetched.expires_at.is_some());
    }

    #[test]
    fn test_accept_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::StateUpdate,
            "identity.name",
            serde_json::json!("New Name"),
            "Builder suggested new name",
            0.85,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&proposal).unwrap();

        proposal.accept();
        store.update_proposal(&proposal).unwrap();

        let fetched = store.get_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(fetched.status, ProposalStatus::Accepted);
    }

    #[test]
    fn concurrent_dispatch_claim_has_exactly_one_winner() {
        let store = std::sync::Arc::new(ProposalStore::new_in_memory().unwrap());
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "external.write",
            serde_json::json!({ "target": "test" }),
            "Concurrent dispatch claim test",
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );
        store.create_proposal(&proposal).unwrap();
        const CONTENDERS: usize = 100;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(CONTENDERS));
        let handles = (0..CONTENDERS)
            .map(|_| {
                let store = std::sync::Arc::clone(&store);
                let barrier = std::sync::Arc::clone(&barrier);
                let proposal_id = proposal.id.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    store.claim_dispatch(&proposal_id).unwrap().is_some()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok())
                .filter(|won| *won)
                .count(),
            1
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("claimed")
        );
    }

    #[test]
    fn definitely_before_effect_failure_can_be_reclaimed_but_unknown_and_confirmed_cannot() {
        fn proposal() -> AgentProposal {
            AgentProposal::new(
                ProposalType::ExternalWriteAction,
                "external.write",
                serde_json::json!({ "target": "test" }),
                "Dispatch retry safety test",
                0.9,
                RiskLevel::High,
                ProposalSource::Manual,
            )
        }

        let store = ProposalStore::new_in_memory().unwrap();

        let failed_before_effect = proposal();
        store.create_proposal(&failed_before_effect).unwrap();
        let failed_claim = store
            .claim_dispatch(&failed_before_effect.id)
            .unwrap()
            .unwrap();
        assert!(store
            .mark_dispatch_failed_before_effect(
                &failed_before_effect.id,
                &failed_claim,
                "validation_failed",
            )
            .unwrap());
        assert_eq!(
            store
                .dispatch_state(&failed_before_effect.id)
                .unwrap()
                .as_deref(),
            Some("failed_before_effect")
        );
        assert!(store
            .claim_dispatch(&failed_before_effect.id)
            .unwrap()
            .is_some());

        let unknown = proposal();
        store.create_proposal(&unknown).unwrap();
        let unknown_claim = store.claim_dispatch(&unknown.id).unwrap().unwrap();
        assert!(store
            .mark_dispatch_unknown(&unknown.id, &unknown_claim, "effect_state_unknown")
            .unwrap());
        assert_eq!(
            store.dispatch_state(&unknown.id).unwrap().as_deref(),
            Some("unknown")
        );
        assert!(store.claim_dispatch(&unknown.id).unwrap().is_none());

        let mut confirmed = proposal();
        store.create_proposal(&confirmed).unwrap();
        let confirmed_claim = store.claim_dispatch(&confirmed.id).unwrap().unwrap();
        assert!(store
            .mark_effect_confirmed_projection_pending(&confirmed.id, &confirmed_claim)
            .unwrap());
        confirmed.accept();
        assert!(store
            .project_confirmed_effect(&confirmed, &confirmed_claim)
            .unwrap());
        assert_eq!(
            store.dispatch_state(&confirmed.id).unwrap().as_deref(),
            Some("confirmed")
        );
        assert!(store.claim_dispatch(&confirmed.id).unwrap().is_none());
    }

    #[test]
    fn confirmed_effect_projection_and_receipt_close_atomically_and_are_retryable() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({"title": "projection recovery"}),
            "Atomic confirmed effect projection test",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&proposal).unwrap();
        let claim_id = store.claim_dispatch(&proposal.id).unwrap().unwrap();
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim_id)
            .unwrap());
        {
            let connection = store.conn.lock().unwrap();
            connection
                .execute_batch(
                    "CREATE TRIGGER fail_confirmed_projection_for_test
                     BEFORE UPDATE OF status ON proposals
                     WHEN NEW.status = 'accepted'
                     BEGIN
                       SELECT RAISE(FAIL, 'forced projection failure');
                     END;",
                )
                .unwrap();
        }
        let mut accepted = proposal.clone();
        accepted.accept();
        assert!(store
            .project_confirmed_effect(&accepted, &claim_id)
            .is_err());
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Pending
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed_projection_pending")
        );
        {
            let connection = store.conn.lock().unwrap();
            connection
                .execute_batch("DROP TRIGGER fail_confirmed_projection_for_test;")
                .unwrap();
        }
        assert!(store
            .project_confirmed_effect(&accepted, &claim_id)
            .unwrap());
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed")
        );
        assert!(store.claim_dispatch(&proposal.id).unwrap().is_none());
    }

    #[test]
    fn expiration_cleanup_cannot_overwrite_confirmed_projection_pending_truth() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({"title": "confirmed but projection pending"}),
            "Confirmed receipt must outrank expiry cleanup",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        proposal.expires_at = Some(chrono::Utc::now() - chrono::Duration::minutes(1));
        store.create_proposal(&proposal).unwrap();
        let claim_id = store.claim_dispatch(&proposal.id).unwrap().unwrap();
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim_id)
            .unwrap());

        assert_eq!(store.cleanup_expired_proposals().unwrap(), 0);
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Pending
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed_projection_pending")
        );
    }

    #[test]
    fn confirmed_receipt_repairs_legacy_rejected_or_expired_projection() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({"title": "repair stale projection"}),
            "Confirmed receipt is the higher-priority fact",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&proposal).unwrap();
        let claim_id = store.claim_dispatch(&proposal.id).unwrap().unwrap();
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim_id)
            .unwrap());
        {
            let connection = store.conn.lock().unwrap();
            connection
                .execute(
                    "UPDATE proposals SET status = 'rejected' WHERE id = ?1",
                    [&proposal.id],
                )
                .unwrap();
        }
        let pending = store.list_confirmed_projection_pending(200).unwrap();
        assert_eq!(pending.len(), 1);
        let mut accepted = pending[0].0.clone();
        accepted.accept();
        assert!(store
            .project_confirmed_effect(&accepted, &pending[0].1)
            .unwrap());
        assert_eq!(
            store.get_proposal(&proposal.id).unwrap().unwrap().status,
            ProposalStatus::Accepted
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed")
        );
    }

    #[test]
    fn confirmed_projection_reconciliation_query_uses_dispatch_index() {
        let store = ProposalStore::new_in_memory().unwrap();
        let connection = store.conn.lock().unwrap();
        let mut statement = connection
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT id, dispatch_claim_id
                 FROM proposals INDEXED BY idx_proposals_dispatch_reconciliation
                 WHERE dispatch_state = 'confirmed_projection_pending'
                   AND dispatch_claim_id IS NOT NULL
                 ORDER BY dispatch_claimed_at ASC, id ASC
                 LIMIT 200",
            )
            .unwrap();
        let details = statement
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        assert!(
            details.contains("idx_proposals_dispatch_reconciliation"),
            "{details}"
        );
    }

    #[test]
    fn review_mutation_cas_loses_to_concurrent_review_or_dispatch_claim() {
        let store = ProposalStore::new_in_memory().unwrap();
        let original = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({"title": "review CAS"}),
            "Review mutation concurrency test",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&original).unwrap();

        let mut rejected = original.clone();
        rejected.reject();
        assert!(store
            .update_review_before_dispatch(&rejected, ProposalStatus::Pending)
            .unwrap());
        let mut stale_edit = original.clone();
        stale_edit.edit(serde_json::json!({"title": "stale edit"}));
        assert!(!store
            .update_review_before_dispatch(&stale_edit, ProposalStatus::Pending)
            .unwrap());
        assert_eq!(
            store.get_proposal(&original.id).unwrap().unwrap().status,
            ProposalStatus::Rejected
        );

        let claimed = AgentProposal::new(
            ProposalType::ScheduledTask,
            "tasks.scheduled",
            serde_json::json!({"title": "dispatch wins"}),
            "Dispatch must block a late review mutation",
            0.9,
            RiskLevel::Medium,
            ProposalSource::BuilderReview,
        );
        store.create_proposal(&claimed).unwrap();
        assert!(store.claim_dispatch(&claimed.id).unwrap().is_some());
        let mut late_reject = claimed.clone();
        late_reject.reject();
        assert!(!store
            .update_review_before_dispatch(&late_reject, ProposalStatus::Pending)
            .unwrap());
        assert_eq!(
            store.get_proposal(&claimed.id).unwrap().unwrap().status,
            ProposalStatus::Pending
        );
        assert_eq!(
            store.dispatch_state(&claimed.id).unwrap().as_deref(),
            Some("claimed")
        );
    }

    #[test]
    fn test_list_pending_proposals() {
        let store = ProposalStore::new_in_memory().unwrap();
        for i in 0..3 {
            let proposal = AgentProposal::new(
                ProposalType::GoalUpdate,
                &format!("path.{}", i),
                serde_json::json!(format!("value{}", i)),
                "test",
                0.5,
                RiskLevel::Low,
                ProposalSource::Manual,
            );
            store.create_proposal(&proposal).unwrap();
        }

        let pending = store.list_pending_proposals(10).unwrap();
        assert_eq!(pending.len(), 3);

        let count = store.pending_count().unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_expired_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::GoalUpdate,
            "identity.name",
            serde_json::json!("New Name"),
            "Test",
            0.5,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        // Set expiration in the past
        proposal.expires_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
        store.create_proposal(&proposal).unwrap();

        let cleaned = store.cleanup_expired_proposals().unwrap();
        assert_eq!(cleaned, 1);

        let fetched = store.get_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(fetched.status, ProposalStatus::Expired);
        assert!(fetched.resolved_at.is_some());
        assert!(fetched.is_expired());
    }

    #[test]
    fn test_unknown_proposal_type_maps_to_unsupported() {
        let store = ProposalStore::new_in_memory().unwrap();
        // Insert a proposal with unknown type directly via SQL
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO proposals (id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                "unknown-type-test",
                Option::<String>::None,
                "unknown_future_type",
                "manual",
                Option::<String>::None,
                "test.path",
                Option::<String>::None,
                "\"value\"",
                "Test unknown type",
                0.5,
                "low",
                "pending",
                chrono::Utc::now().to_rfc3339(),
                Option::<String>::None,
                Option::<String>::None,
            ],
        ).unwrap();
        drop(conn);

        // Should not panic or masquerade as a LifeModel update.
        let fetched = store.get_proposal("unknown-type-test").unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.proposal_type, ProposalType::Unsupported);
        assert_eq!(fetched.status, ProposalStatus::Pending);
        assert_eq!(fetched.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_unknown_risk_level_fallback() {
        let store = ProposalStore::new_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO proposals (id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                "unknown-risk-test",
                Option::<String>::None,
                "goal_update",
                "manual",
                Option::<String>::None,
                "test.path",
                Option::<String>::None,
                "\"value\"",
                "Test unknown risk",
                0.5,
                "ultra_high",
                "pending",
                chrono::Utc::now().to_rfc3339(),
                Option::<String>::None,
                Option::<String>::None,
            ],
        ).unwrap();
        drop(conn);

        let fetched = store.get_proposal("unknown-risk-test").unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.risk_level, RiskLevel::Medium); // Fallback
    }

    #[test]
    fn unknown_persisted_status_fails_closed_instead_of_becoming_pending() {
        let store = ProposalStore::new_in_memory().unwrap();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO proposals (id, run_id, proposal_type, source, source_detail, affected_path, before_json, after_json, reason, confidence, risk_level, status, created_at, resolved_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                "unknown-status-test",
                Option::<String>::None,
                "goal_update",
                "manual",
                Option::<String>::None,
                "test.path",
                Option::<String>::None,
                "\"value\"",
                "Test unknown status",
                0.5,
                "low",
                "mystery_terminal_state",
                chrono::Utc::now().to_rfc3339(),
                Option::<String>::None,
                Option::<String>::None,
            ],
        )
        .unwrap();
        drop(conn);

        let error = store.get_proposal("unknown-status-test").unwrap_err();
        assert!(error.to_string().contains("unsupported proposal status"));
    }

    fn claimed_artifact_proposal(store: &ProposalStore) -> (AgentProposal, String) {
        let proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            "filesystem./tmp/roadshow-summary.md",
            serde_json::json!({
                "path": "/tmp/roadshow-summary.md",
                "content": "PRIVATE_ARTIFACT_BODY_MUST_NOT_BE_COPIED_TO_EFFECT_LEDGER"
            }),
            "Materialize a reviewed roadshow artifact.",
            1.0,
            RiskLevel::High,
            ProposalSource::ChatConversation,
        );
        store.create_proposal(&proposal).unwrap();
        let claim_id = store.claim_dispatch(&proposal.id).unwrap().unwrap();
        (proposal, claim_id)
    }

    #[test]
    fn artifact_effect_binds_exact_claim_and_advances_receipt_atomically() {
        let store = ProposalStore::new_in_memory().unwrap();
        let (proposal, claim_id) = claimed_artifact_proposal(&store);
        let record = store
            .prepare_artifact_effect(
                &proposal.id,
                &claim_id,
                "sha256:target",
                "sha256:content",
                42,
                "text/markdown",
            )
            .unwrap();
        assert_eq!(record.state, ArtifactEffectState::Prepared);
        assert!(store.mark_artifact_staged(&proposal.id, &claim_id).unwrap());
        assert!(!store
            .finish_artifact_confirmed(&proposal.id, &claim_id, "sha256:wrong")
            .unwrap());
        assert_eq!(
            store.artifact_effect(&proposal.id).unwrap().unwrap().state,
            ArtifactEffectState::Staged
        );
        assert!(store
            .finish_artifact_confirmed(&proposal.id, &claim_id, "sha256:content")
            .unwrap());
        let confirmed = store.artifact_effect(&proposal.id).unwrap().unwrap();
        assert_eq!(confirmed.state, ArtifactEffectState::Confirmed);
        assert_eq!(
            confirmed.observed_content_digest.as_deref(),
            Some("sha256:content")
        );
        assert_eq!(
            store.dispatch_state(&proposal.id).unwrap().as_deref(),
            Some("confirmed_projection_pending")
        );
        assert!(store
            .finish_artifact_confirmed(&proposal.id, &claim_id, "sha256:content")
            .unwrap());

        let connection = store.conn.lock().unwrap();
        let columns = connection
            .prepare("PRAGMA table_info(artifact_effects)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(!columns.iter().any(|column| {
            matches!(
                column.as_str(),
                "content" | "body" | "payload" | "after_json"
            )
        }));
    }

    #[test]
    fn artifact_retry_replaces_only_a_proven_failed_before_effect_claim() {
        let store = ProposalStore::new_in_memory().unwrap();
        let (proposal, first_claim) = claimed_artifact_proposal(&store);
        store
            .prepare_artifact_effect(
                &proposal.id,
                &first_claim,
                "sha256:target",
                "sha256:content",
                42,
                "text/markdown",
            )
            .unwrap();
        assert!(store
            .finish_artifact_failed_before_effect(
                &proposal.id,
                &first_claim,
                "artifact_stage_not_created",
            )
            .unwrap());
        let second_claim = store.claim_dispatch(&proposal.id).unwrap().unwrap();
        assert_ne!(first_claim, second_claim);
        let retried = store
            .prepare_artifact_effect(
                &proposal.id,
                &second_claim,
                "sha256:target",
                "sha256:content",
                42,
                "text/markdown",
            )
            .unwrap();
        assert_eq!(retried.dispatch_claim_id, second_claim);
        assert_eq!(retried.state, ArtifactEffectState::Prepared);
        assert!(store
            .mark_artifact_staged(&proposal.id, &second_claim)
            .unwrap());
        assert!(store
            .finish_artifact_unknown(
                &proposal.id,
                &second_claim,
                "artifact_rename_outcome_unknown",
            )
            .unwrap());
        assert!(store.claim_dispatch(&proposal.id).unwrap().is_none());
        assert!(store
            .prepare_artifact_effect(
                &proposal.id,
                "stale-claim",
                "sha256:target",
                "sha256:content",
                42,
                "text/markdown",
            )
            .is_err());
        let pending = store.list_artifact_effects_for_reconciliation(10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].state, ArtifactEffectState::Unknown);
    }

    #[test]
    fn artifact_and_proposal_receipt_cas_failure_rolls_back_both_sides() {
        let store = ProposalStore::new_in_memory().unwrap();
        let (proposal, claim_id) = claimed_artifact_proposal(&store);
        store
            .prepare_artifact_effect(
                &proposal.id,
                &claim_id,
                "sha256:target",
                "sha256:content",
                42,
                "text/markdown",
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE proposals SET dispatch_state = 'confirmed' WHERE id = ?1",
                [&proposal.id],
            )
            .unwrap();

        assert!(!store
            .finish_artifact_failed_before_effect(
                &proposal.id,
                &claim_id,
                "counterfactual_receipt_cas_mismatch",
            )
            .unwrap());
        let artifact = store.artifact_effect(&proposal.id).unwrap().unwrap();
        assert_eq!(artifact.state, ArtifactEffectState::Prepared);
        assert!(artifact.error_code.is_none());
    }
}
