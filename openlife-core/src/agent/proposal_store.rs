use crate::agent::types::{AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel};
use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
        crate::sqlite_migration::record_schema_version(&tx, "proposal_store", 11)?;
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

    /// Removes content-bearing fields from one accepted Memory proposal after
    /// the user has confirmed irreversible privacy erasure of the canonical
    /// Memory asset. Stable ids, decision state, timestamps, and existing
    /// digests remain available as body-free audit metadata.
    pub fn redact_accepted_memory_proposal_content(
        &self,
        proposal_id: &str,
        memory_id: &str,
    ) -> Result<bool> {
        let proposal_id = proposal_id.trim();
        let memory_id = memory_id.trim();
        if proposal_id.is_empty() || memory_id.is_empty() {
            anyhow::bail!("Memory proposal privacy redaction requires exact ids");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let proposal = conn
            .query_row(
                "SELECT id, run_id, proposal_type, source, source_detail, base_hash,
                        affected_path, before_json, after_json, reason, confidence,
                        risk_level, status, created_at, resolved_at, expires_at
                 FROM proposals WHERE id = ?1",
                [proposal_id],
                Self::row_to_proposal,
            )
            .optional()?;
        let Some(proposal) = proposal else {
            return Ok(false);
        };
        if proposal.status != ProposalStatus::Accepted {
            anyhow::bail!("Memory proposal privacy redaction requires an accepted proposal");
        }
        if !matches!(
            proposal.proposal_type,
            ProposalType::MemoryWrite | ProposalType::MemoryArchive
        ) {
            anyhow::bail!("proposal privacy redaction target is not a Memory proposal");
        }
        let redacted_after = serde_json::to_string(&serde_json::json!({
            "memoryId": memory_id,
            "privacyErased": true,
        }))?;
        let changed = conn.execute(
            "UPDATE proposals
             SET source_detail = 'privacy_erased',
                 base_hash = NULL,
                 before_json = NULL,
                 after_json = ?2,
                 reason = 'Memory proposal content privacy-erased by user request.'
             WHERE id = ?1
               AND status = 'accepted'
               AND proposal_type IN ('memory_write', 'memory_archive')",
            params![proposal_id, redacted_after],
        )?;
        if changed != 1 {
            anyhow::bail!("Memory proposal privacy redaction lost its exact target");
        }
        Ok(true)
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

    pub fn list_claimed_external_writes_without_artifact_intent(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut statement = conn.prepare(
            "SELECT id, dispatch_claim_id
             FROM proposals
             WHERE proposal_type = 'external_write_action'
               AND dispatch_state = 'claimed'
               AND dispatch_claim_id IS NOT NULL
             ORDER BY dispatch_claimed_at ASC, id ASC
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
            "memory_write" => ProposalType::MemoryWrite,
            "memory_archive" => ProposalType::MemoryArchive,
            "tool_permission" => ProposalType::ToolPermission,
            "external_write_action" => ProposalType::ExternalWriteAction,
            "life_model_update" => ProposalType::LifeModelUpdate,
            _ => return Err(rusqlite::Error::InvalidQuery),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
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
    fn accepted_memory_proposal_privacy_redaction_removes_content_but_keeps_audit_identity() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::MemoryWrite,
            "memory:privacy-target",
            serde_json::json!({
                "content": "PROPOSAL_PRIVATE_MEMORY_BODY",
                "scope": "project"
            }),
            "PROPOSAL_PRIVATE_MEMORY_REASON",
            1.0,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.id = "proposal:privacy-target".into();
        proposal.before = Some(serde_json::json!({
            "content": "PROPOSAL_PRIVATE_MEMORY_BEFORE"
        }));
        store.create_proposal(&proposal).unwrap();
        proposal.status = ProposalStatus::Accepted;
        proposal.resolved_at = Some(chrono::Utc::now());
        store.update_proposal(&proposal).unwrap();

        assert!(store
            .redact_accepted_memory_proposal_content(&proposal.id, "memory:privacy-target")
            .unwrap());
        let redacted = store.get_proposal(&proposal.id).unwrap().unwrap();
        let rendered = serde_json::to_string(&redacted).unwrap();
        assert_eq!(redacted.id, proposal.id);
        assert_eq!(redacted.status, ProposalStatus::Accepted);
        assert_eq!(redacted.after["memoryId"], "memory:privacy-target");
        assert_eq!(redacted.after["privacyErased"], true);
        assert!(redacted.before.is_none());
        assert!(!rendered.contains("PROPOSAL_PRIVATE_MEMORY_BODY"));
        assert!(!rendered.contains("PROPOSAL_PRIVATE_MEMORY_BEFORE"));
        assert!(!rendered.contains("PROPOSAL_PRIVATE_MEMORY_REASON"));
    }

    #[test]
    fn test_accept_proposal() {
        let store = ProposalStore::new_in_memory().unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
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
            ProposalType::ExternalWriteAction,
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
            ProposalType::ExternalWriteAction,
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
            ProposalType::ExternalWriteAction,
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
            ProposalType::ExternalWriteAction,
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
            ProposalType::ExternalWriteAction,
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
                ProposalType::LifeModelUpdate,
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
            ProposalType::LifeModelUpdate,
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
    fn test_unknown_proposal_type_is_rejected() {
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

        assert!(store.get_proposal("unknown-type-test").is_err());
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
                "life_model_update",
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
                "life_model_update",
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
}
