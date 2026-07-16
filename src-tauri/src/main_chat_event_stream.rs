use crate::main_chat_agent_state_payload::assemble_main_chat_agent_state_for_turn;
use crate::AppState;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use rand::RngCore;
use rusqlite::{
    params, types::Type, Connection, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
#[cfg(test)]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use openlife_core::agent::main_chat_runtime_contract::{
    MainChatAgentProductProposalStatus, MainChatAgentStateSnapshot,
};
use openlife_core::llm::{
    provider_lifecycle_evidence_digest, ProviderInvocationReceipt, ProviderInvocationStatus,
    ProviderPolicyReceiptEvidence,
};
use openlife_core::scheduler::ProviderInvocationDurabilityProof;
use openlife_core::tool_execution_receipt::{
    ToolActionEffect, ToolAuditPersistenceStatus, ToolDispatchKind, ToolEffectStatus,
    ToolExecutionOutcome, ToolExecutionReceipt, ToolTransportStatus,
};

const DURABLE_EVENT_PAYLOAD_VERSION: i64 = 7;
const UNRECOGNIZED_FIELDS_RECEIPT: &str = "unrecognizedFieldsReceipt";
const DURABLE_EVENT_IDENTITY_VERSION: i64 = 2;
const MAX_EVENT_IDENTITY_CHARS: usize = 256;
const MAX_EVENT_TYPE_CHARS: usize = 96;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MainChatAgentDurableEvent {
    pub event_id: String,
    pub task_session_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub object_type: String,
    pub object_id: String,
    pub created_at: DateTime<Utc>,
    pub source: String,
    pub payload_digest: String,
    pub payload: Value,
    pub backfilled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MainChatTurnLifecycleSnapshot {
    pub(crate) latest_sequence: u64,
    pub(crate) bound_run_id: Option<String>,
    pub(crate) lifecycle_event: Option<MainChatAgentDurableEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalOwnerSealState {
    Open,
    Sealing,
    Sealed,
}

impl TerminalOwnerSealState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Sealing => "sealing",
            Self::Sealed => "sealed",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "open" => Ok(Self::Open),
            "sealing" => Ok(Self::Sealing),
            "sealed" => Ok(Self::Sealed),
            _ => anyhow::bail!("terminal_owner_epoch_state_invalid"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TerminalOwnerEpoch {
    epoch_id: String,
    task_session_id: String,
    run_id: String,
    generation: u64,
    state: TerminalOwnerSealState,
    canonical_user_message_ref: String,
    canonical_user_message_digest: String,
    final_event_id: Option<String>,
    final_event_payload_digest: Option<String>,
    replayed: bool,
    review_origin: Option<openlife_core::agent::TerminalOwnerReviewOriginProof>,
}

impl TerminalOwnerEpoch {
    pub(crate) fn epoch_id(&self) -> &str {
        &self.epoch_id
    }

    pub(crate) fn task_session_id(&self) -> &str {
        &self.task_session_id
    }

    pub(crate) fn run_id(&self) -> &str {
        &self.run_id
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn state(&self) -> TerminalOwnerSealState {
        self.state
    }

    pub(crate) fn canonical_user_message_ref(&self) -> &str {
        &self.canonical_user_message_ref
    }

    pub(crate) fn canonical_user_message_digest(&self) -> &str {
        &self.canonical_user_message_digest
    }

    pub(crate) fn final_event_id(&self) -> Option<&str> {
        self.final_event_id.as_deref()
    }

    pub(crate) fn final_event_payload_digest(&self) -> Option<&str> {
        self.final_event_payload_digest.as_deref()
    }

    pub(crate) fn replayed(&self) -> bool {
        self.replayed
    }

    pub(crate) fn review_origin_proof(
        &self,
    ) -> Option<openlife_core::agent::TerminalOwnerReviewOriginProof> {
        self.review_origin.clone()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatTerminalFinalizationInput {
    pub(crate) task_session_id: String,
    pub(crate) run_id: String,
    pub(crate) epoch_generation: u64,
    pub(crate) delivery_id: String,
    pub(crate) expected_task_owner_revision: u64,
    pub(crate) expected_task_owner_digest: String,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatCancellationProjectionDelivery {
    pub(crate) cancellation_id: String,
    pub(crate) task_session_id: String,
    pub(crate) run_id: String,
    pub(crate) terminal_event_id: String,
    pub(crate) projection_target: String,
    pub(crate) state: String,
    pub(crate) attempts: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatEventIdentityQuarantineReceipt {
    pub(crate) quarantine_id: String,
    pub(crate) task_identity_digest: String,
    pub(crate) row_set_digest: String,
    pub(crate) event_count: u64,
    pub(crate) reason_code: String,
    pub(crate) quarantined_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct MainChatAgentEventDraft {
    task_session_id: String,
    run_id: String,
    event_type: String,
    object_type: String,
    object_id: String,
    created_at: DateTime<Utc>,
    source: String,
    payload: Value,
    backfilled: bool,
}

enum ProviderLifecycleAdmission<'a> {
    None,
    Runtime {
        scope: &'a crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
        proofs: &'a [ProviderInvocationDurabilityProof],
    },
    #[cfg(test)]
    SyntheticTest {
        scope: &'a crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
        proofs: &'a [ProviderInvocationDurabilityProof],
    },
}

#[derive(Clone, Copy)]
enum ToolLifecycleAdmission<'a> {
    None,
    LiveNotDispatched(&'a ToolExecutionReceipt),
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatAgentRuntimeEventInput {
    event_type: String,
    object_type: String,
    object_id: String,
    source: String,
    payload: Value,
    occurred_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub(crate) struct MainChatToolReceiptEventProjection {
    pub(crate) receipt_id: String,
    pub(crate) dispatch_event_at: DateTime<Utc>,
    pub(crate) terminal_at: DateTime<Utc>,
    pub(crate) terminal_event_type: &'static str,
    pub(crate) terminal_status: &'static str,
    pub(crate) common_payload: Value,
}

#[derive(Clone)]
pub(crate) struct MainChatToolLifecycleObserver {
    state: Arc<AppState>,
    task_session_id: String,
    run_id: String,
    replay_claim: Option<MainChatToolReplayClaimContext>,
}

#[derive(Clone)]
struct MainChatToolReplayClaimContext {
    action_id: String,
    claim_id: String,
    owner_generation: u64,
}

impl MainChatToolLifecycleObserver {
    pub(crate) fn new(
        state: Arc<AppState>,
        task_session_id: impl Into<String>,
        run_id: impl Into<String>,
    ) -> Self {
        Self {
            state,
            task_session_id: task_session_id.into(),
            run_id: run_id.into(),
            replay_claim: None,
        }
    }

    pub(crate) fn with_replay_claim(
        mut self,
        action_id: impl Into<String>,
        claim_id: impl Into<String>,
        owner_generation: u64,
    ) -> Result<Self, String> {
        let action_id = action_id.into();
        let claim_id = claim_id.into();
        if !is_bounded_event_reference(&action_id, 384)
            || uuid::Uuid::parse_str(&claim_id).is_err()
            || owner_generation == 0
        {
            return Err("tool_replay_claim_context_invalid".into());
        }
        self.replay_claim = Some(MainChatToolReplayClaimContext {
            action_id,
            claim_id,
            owner_generation,
        });
        Ok(self)
    }

    async fn persist_prepared_fact(
        &self,
        attempt: &openlife_core::agent::ToolDispatchAttempt,
    ) -> anyhow::Result<()> {
        if attempt.source_run_id.as_deref() != Some(self.run_id.as_str()) {
            anyhow::bail!("tool_dispatch_prepared_run_identity_mismatch");
        }
        let mut payload = json!({
            "receiptId": attempt.receipt_id,
            "requestId": attempt.receipt_id,
            "sourceRunId": self.run_id,
            "manifestId": attempt.manifest_id,
            "toolName": attempt.tool_name,
            "requestDigest": attempt.request_digest,
            "manifestContractDigest": attempt.manifest_contract_digest,
            "inputHash": attempt.input_hash,
            "inputLengthBytes": attempt.input_length_bytes,
            "actionEffect": attempt.action_effect.as_str(),
            "idempotencyContract": attempt.idempotency_contract.as_str(),
            "dispatchProcessRisk": attempt.process_risk.as_str(),
            "mayOutliveLocalProcess": attempt.process_risk.may_outlive_local_process(),
            "effectMaySurviveLocalProcess": attempt.effect_may_survive_local_process,
            "status": "prepared",
        });
        if let Some(replay_claim) = self.replay_claim.as_ref() {
            let authority_binding = {
                let queue_arc = self
                    .state
                    .main_chat_action_queue_store
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("main_chat_action_queue_store_unavailable"))?;
                let queue = queue_arc.lock().await;
                queue.issue_replay_prepared_tool_authority_binding(
                    &self.task_session_id,
                    &self.run_id,
                    &replay_claim.action_id,
                    &replay_claim.claim_id,
                    replay_claim.owner_generation,
                    attempt,
                )?
            };
            let object = payload
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("tool_dispatch_prepared_payload_invalid"))?;
            object.insert(
                "replayActionId".into(),
                json!(replay_claim.action_id.clone()),
            );
            object.insert("replayClaimId".into(), json!(replay_claim.claim_id.clone()));
            object.insert(
                "replayClaimOwnerGeneration".into(),
                json!(replay_claim.owner_generation),
            );
            object.insert("replayAuthorityBinding".into(), json!(authority_binding));
        }
        append_main_chat_agent_runtime_event(
            &self.state,
            self.task_session_id.clone(),
            self.run_id.clone(),
            "tool.dispatch_prepared",
            "tool_execution_receipt",
            attempt.receipt_id.clone(),
            "openlife_turn_runtime.tool_dispatch_prepared",
            payload,
        )
        .await
        .map(|_| ())
        .map_err(anyhow::Error::msg)
    }

    #[cfg(test)]
    pub(crate) async fn persist_prepared_fact_for_crash_fixture(
        &self,
        attempt: &openlife_core::agent::ToolDispatchAttempt,
    ) -> anyhow::Result<()> {
        self.persist_prepared_fact(attempt).await
    }
}

#[async_trait::async_trait]
impl openlife_core::agent::ToolDispatchObserver for MainChatToolLifecycleObserver {
    async fn before_dispatch(
        &self,
        attempt: &openlife_core::agent::ToolDispatchAttempt,
    ) -> anyhow::Result<()> {
        self.state
            .persistence_coordinator
            .require_effects_allowed()
            .map_err(anyhow::Error::msg)?;
        self.persist_prepared_fact(attempt).await
    }
}

#[async_trait::async_trait]
impl openlife_core::agent::ToolStartedTransitionObserver for MainChatToolLifecycleObserver {
    async fn after_dispatch(&self, receipt: &ToolExecutionReceipt) -> anyhow::Result<()> {
        let input = main_chat_tool_started_event_input(
            &self.run_id,
            receipt,
            "openlife_turn_runtime.tool_started",
        )
        .map_err(anyhow::Error::msg)?;
        append_main_chat_agent_runtime_event_batch(
            &self.state,
            self.task_session_id.clone(),
            self.run_id.clone(),
            vec![input],
        )
        .await
        .map(|_| ())
        .map_err(anyhow::Error::msg)
    }
}

impl MainChatAgentRuntimeEventInput {
    pub(crate) fn new(
        event_type: impl Into<String>,
        object_type: impl Into<String>,
        object_id: impl Into<String>,
        source: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            event_type: event_type.into(),
            object_type: object_type.into(),
            object_id: object_id.into(),
            source: source.into(),
            payload,
            occurred_at: None,
        }
    }

    pub(crate) fn with_occurred_at(mut self, occurred_at: DateTime<Utc>) -> Self {
        self.occurred_at = Some(occurred_at);
        self
    }

    pub(crate) fn occurred_at(&self) -> Option<DateTime<Utc>> {
        self.occurred_at
    }
}

#[derive(Clone)]
pub struct MainChatAgentEventStore {
    conn: Arc<Mutex<Connection>>,
    digest_key: Arc<MainChatEventDigestKey>,
}

#[cfg(test)]
fn terminal_final_seal_failpoints() -> &'static Mutex<std::collections::HashSet<String>> {
    static FAILPOINTS: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    FAILPOINTS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MainChatOrphanedToolReconciliationReport {
    pub(crate) examined: usize,
    pub(crate) remote_unknown: usize,
    pub(crate) effect_unknown: usize,
    pub(crate) local_aborted: usize,
    pub(crate) has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainChatToolQueueReconciliationDisposition {
    EffectNotAttempted,
    DispatchedUnknown,
}

impl MainChatToolQueueReconciliationDisposition {
    fn as_str(self) -> &'static str {
        match self {
            Self::EffectNotAttempted => "effect_not_attempted",
            Self::DispatchedUnknown => "dispatched_unknown",
        }
    }

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "effect_not_attempted" => Ok(Self::EffectNotAttempted),
            "dispatched_unknown" => Ok(Self::DispatchedUnknown),
            _ => anyhow::bail!("tool_reconciliation_outbox_disposition_invalid"),
        }
    }
}

fn core_tool_reconciliation_resolution(
    event_type: &str,
) -> Result<openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution> {
    match event_type {
        "tool.not_dispatched" => Ok(
            openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution::NotDispatched,
        ),
        "tool.dispatch_ambiguous" => Ok(
            openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution::DispatchAmbiguous,
        ),
        _ => anyhow::bail!("tool_reconciliation_resolution_event_type_invalid"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatToolQueueReconciliationProjection {
    pub(crate) outbox_id: String,
    pub(crate) prepared_event_id: String,
    pub(crate) prepared_payload_digest: String,
    pub(crate) resolution_event_id: String,
    pub(crate) resolution_payload_digest: String,
    pub(crate) resolution: openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution,
    pub(crate) task_session_id: String,
    pub(crate) run_id: String,
    pub(crate) receipt_id: String,
    pub(crate) replay_action_id: String,
    pub(crate) replay_claim_id: String,
    pub(crate) replay_claim_owner_generation: u64,
    pub(crate) manifest_id: String,
    pub(crate) tool_name: String,
    pub(crate) manifest_contract_digest: String,
    pub(crate) input_hash: String,
    pub(crate) input_length_bytes: u64,
    pub(crate) request_digest: String,
    pub(crate) action_effect: ToolActionEffect,
    pub(crate) idempotency_contract: openlife_core::tool_manifest::ToolIdempotencyContract,
    pub(crate) process_risk: openlife_core::agent::action_executor::ToolDispatchProcessRisk,
    pub(crate) effect_may_survive_local_process: bool,
    pub(crate) replay_authority_binding: String,
    pub(crate) disposition: MainChatToolQueueReconciliationDisposition,
    pub(crate) event_store_attestation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MainChatToolQueueReconciliationBatch {
    pub(crate) items: Vec<MainChatToolQueueReconciliationProjection>,
    pub(crate) has_more: bool,
}

/// Opaque redaction-key capability. Persistent callers must hydrate this from
/// a secret owner outside the event database. It deliberately has no serde,
/// debug payload, or getter for the underlying material.
pub(crate) struct MainChatEventDigestKey([u8; 32]);

impl MainChatEventDigestKey {
    pub(crate) fn from_key_material(material: &[u8]) -> Result<Self> {
        let key: [u8; 32] = material
            .try_into()
            .map_err(|_| anyhow::anyhow!("main_chat_event_digest_key_must_be_32_bytes"))?;
        if key.iter().all(|byte| *byte == 0) {
            anyhow::bail!("main_chat_event_digest_key_must_not_be_zero");
        }
        Ok(Self(key))
    }

    fn random() -> Result<Self> {
        let mut key = [0_u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        Self::from_key_material(&key)
    }

    fn reconciliation_attestation_key_pair(&self) -> Result<ring::signature::Ed25519KeyPair> {
        let mut material = b"openlife-main-chat-event-store-reconciliation-ed25519-v1\0".to_vec();
        material.extend_from_slice(&self.0);
        let digest = ring::digest::digest(&ring::digest::SHA256, &material);
        ring::signature::Ed25519KeyPair::from_seed_unchecked(digest.as_ref())
            .map_err(|_| anyhow::anyhow!("event_store_reconciliation_signing_key_invalid"))
    }

    fn reconciliation_attestation_public_key(&self) -> Result<[u8; 32]> {
        use ring::signature::KeyPair as _;
        self.reconciliation_attestation_key_pair()?
            .public_key()
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("event_store_reconciliation_public_key_invalid"))
    }

    fn sign_reconciliation_attestation(
        &self,
        envelope: &openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationEnvelope<'_>,
    ) -> Result<String> {
        let material = openlife_core::agent::main_chat_agent_v1::replay_prepared_tool_reconciliation_attestation_material(envelope);
        let signature = self.reconciliation_attestation_key_pair()?.sign(&material);
        Ok(format!(
            "ed25519:{}",
            STANDARD_NO_PAD.encode(signature.as_ref())
        ))
    }
}

#[derive(Debug)]
enum MainChatAgentEventStoreFault {
    ImmutableIdentityConflict {
        event_type: String,
        existing_payload_digest: String,
        incoming_payload_digest: String,
    },
    DuplicateImmutableIdentity {
        event_type: String,
    },
    ImmutableIdentityRegistryConflict,
    TaskRunIdentityConflict,
    ProviderLifecycleConflict {
        reason: String,
    },
    PersistedProviderLifecycleUnverified {
        reason: String,
    },
    ToolLifecycleConflict {
        reason: String,
    },
    PayloadSchemaConflict {
        event_type: String,
        reason: String,
    },
    CorruptRow {
        field: &'static str,
        reason: &'static str,
    },
    SequenceExhausted,
}

impl std::fmt::Display for MainChatAgentEventStoreFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImmutableIdentityConflict {
                event_type,
                existing_payload_digest,
                incoming_payload_digest,
            } => write!(
                formatter,
                "main_chat_agent_event_identity_conflict:{event_type}:{existing_payload_digest}:{incoming_payload_digest}"
            ),
            Self::DuplicateImmutableIdentity { event_type } => write!(
                formatter,
                "main_chat_agent_event_corrupt_identity:duplicate:{event_type}"
            ),
            Self::ImmutableIdentityRegistryConflict => formatter
                .write_str("main_chat_agent_event_immutable_identity_registry_conflict"),
            Self::TaskRunIdentityConflict => {
                formatter.write_str("main_chat_agent_event_task_run_identity_conflict")
            }
            Self::ProviderLifecycleConflict { reason } => {
                write!(formatter, "main_chat_provider_lifecycle_conflict:{reason}")
            }
            Self::PersistedProviderLifecycleUnverified { reason } => {
                write!(formatter, "main_chat_provider_lifecycle_unverified:{reason}")
            }
            Self::ToolLifecycleConflict { reason } => {
                write!(formatter, "main_chat_tool_lifecycle_conflict:{reason}")
            }
            Self::PayloadSchemaConflict { event_type, reason } => write!(
                formatter,
                "main_chat_agent_event_payload_schema_conflict:{event_type}:{reason}"
            ),
            Self::CorruptRow { field, reason } => write!(
                formatter,
                "main_chat_agent_event_corrupt_row:{field}:{reason}"
            ),
            Self::SequenceExhausted => {
                formatter.write_str("main_chat_agent_event_sequence_exhausted")
            }
        }
    }
}

impl std::error::Error for MainChatAgentEventStoreFault {}

impl MainChatAgentEventStore {
    pub(crate) fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        #[cfg(test)]
        {
            return Self::new_with_digest_key(
                db_path,
                MainChatEventDigestKey::from_key_material(&[0x5a; 32])?,
            );
        }
        #[cfg(not(test))]
        {
            let _ = db_path.into();
            anyhow::bail!("main_chat_event_persistent_digest_key_required")
        }
    }

    pub(crate) fn new_with_digest_key(
        db_path: impl Into<PathBuf>,
        digest_key: MainChatEventDigestKey,
    ) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self {
            conn: Arc::new(Mutex::new(Connection::open(&db_path).with_context(
                || format!("failed to open main chat agent event db at {:?}", db_path),
            )?)),
            digest_key: Arc::new(digest_key),
        };
        store.configure_connection()?;
        store.init_tables()?;
        Ok(store)
    }

    pub(crate) fn new_in_memory() -> Result<Self> {
        let store = Self {
            conn: Arc::new(Mutex::new(
                Connection::open_in_memory()
                    .context("failed to open in-memory main chat agent event db")?,
            )),
            digest_key: Arc::new(MainChatEventDigestKey::random()?),
        };
        store.configure_connection()?;
        store.init_tables()?;
        Ok(store)
    }

    fn configure_connection(&self) -> Result<()> {
        self.lock_conn()?.busy_timeout(Duration::from_secs(5))?;
        Ok(())
    }

    /// Prove the durable event database can acquire a write transaction before
    /// any provider or tool dispatch begins. The reserved sequence row is
    /// rolled back, so this check creates no product fact.
    pub(crate) fn preflight_writable(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        let transaction =
            rusqlite::Transaction::new_unchecked(&conn, TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO main_chat_agent_event_sequences(task_session_id, last_sequence)
             VALUES ('__openlife_event_store_preflight__', 0)
             ON CONFLICT(task_session_id) DO UPDATE SET last_sequence = last_sequence",
            [],
        )?;
        transaction.rollback()?;
        Ok(())
    }

    pub(crate) fn open_terminal_owner_epoch_from_admission(
        &self,
        admission: openlife_core::agent::main_chat_agent_v1::TerminalOwnerEpochAdmission,
    ) -> Result<TerminalOwnerEpoch> {
        admission.validate()?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = select_terminal_owner_epoch(&tx, admission.task_session_id())?;
        if let Some(mut existing) = existing {
            let stored_admission_id = tx.query_row(
                "SELECT admission_id FROM terminal_owner_epochs WHERE epoch_id = ?1",
                [&existing.epoch_id],
                |row| row.get::<_, String>(0),
            )?;
            if existing.run_id != admission.run_id()
                || stored_admission_id != admission.admission_id()
                || existing.canonical_user_message_ref != admission.canonical_user_message_ref()
                || existing.canonical_user_message_digest
                    != admission.canonical_user_message_digest()
            {
                anyhow::bail!("terminal_owner_epoch_admission_conflict");
            }
            existing.replayed = true;
            existing.review_origin = Some(
                openlife_core::agent::TerminalOwnerReviewOriginProof::from_epoch_admission(
                    &admission,
                    &existing.epoch_id,
                    existing.generation,
                )?,
            );
            tx.commit()?;
            return Ok(existing);
        }
        let epoch_id = format!("terminal-epoch:{}", uuid::Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO terminal_owner_epochs (
                epoch_id, task_session_id, run_id, generation, state, admission_id,
                canonical_user_message_ref, canonical_user_message_digest,
                canonical_store_identity, created_at, updated_at
             ) VALUES (?1, ?2, ?3, 1, 'open', ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                epoch_id,
                admission.task_session_id(),
                admission.run_id(),
                admission.admission_id(),
                admission.canonical_user_message_ref(),
                admission.canonical_user_message_digest(),
                admission.canonical_store_identity(),
                now,
            ],
        )?;
        let review_origin =
            openlife_core::agent::TerminalOwnerReviewOriginProof::from_epoch_admission(
                &admission, &epoch_id, 1,
            )?;
        tx.commit()?;
        Ok(TerminalOwnerEpoch {
            epoch_id,
            task_session_id: admission.task_session_id().to_string(),
            run_id: admission.run_id().to_string(),
            generation: 1,
            state: TerminalOwnerSealState::Open,
            canonical_user_message_ref: admission.canonical_user_message_ref().to_string(),
            canonical_user_message_digest: admission.canonical_user_message_digest().to_string(),
            final_event_id: None,
            final_event_payload_digest: None,
            replayed: false,
            review_origin: Some(review_origin),
        })
    }

    pub(crate) fn terminal_owner_epoch(
        &self,
        task_session_id: &str,
    ) -> Result<Option<TerminalOwnerEpoch>> {
        let conn = self.lock_conn()?;
        select_terminal_owner_epoch(&conn, task_session_id)
    }

    pub(crate) fn terminal_owner_final_event(
        &self,
        task_session_id: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        let conn = self.lock_conn()?;
        let Some(epoch) = select_terminal_owner_epoch(&conn, task_session_id)? else {
            return Ok(None);
        };
        let Some(final_event_id) = epoch.final_event_id else {
            return Ok(None);
        };
        select_event_by_id(&conn, &final_event_id)
    }

    pub(crate) fn terminal_owner_successor_head(
        &self,
        task_session_id: &str,
    ) -> Result<(u64, String)> {
        let conn = self.lock_conn()?;
        let epoch = select_terminal_owner_epoch(&conn, task_session_id)?
            .context("terminal_owner_epoch_missing")?;
        if epoch.state != TerminalOwnerSealState::Sealed {
            anyhow::bail!("terminal_owner_successor_requires_sealed_epoch");
        }
        let final_event_id = epoch
            .final_event_id
            .as_deref()
            .context("terminal_owner_sealed_final_missing")?;
        let final_event = select_event_by_id(&conn, final_event_id)?
            .context("terminal_owner_sealed_event_missing")?;
        terminal_owner_successor_head_from_conn(
            &conn,
            task_session_id,
            final_event_id,
            &final_event,
        )
    }

    pub(crate) fn open_terminal_owner_replay_epoch_from_admission(
        &self,
        admission: &crate::terminal_owner_write_gateway::TerminalOwnerReplayEpochAdmission,
    ) -> Result<TerminalOwnerEpoch> {
        admission.validate().map_err(anyhow::Error::msg)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut epoch = select_terminal_owner_epoch(&tx, admission.task_session_id())?
            .context("terminal_owner_replay_epoch_missing")?;
        if epoch.state != TerminalOwnerSealState::Sealed
            || epoch.epoch_id != admission.prior_epoch_id()
            || epoch.run_id != admission.run_id()
            || epoch.generation != admission.prior_epoch_generation()
            || epoch.final_event_id.as_deref() != Some(admission.prior_final_event_id())
            || epoch.canonical_user_message_ref != admission.canonical_user_message_ref()
            || epoch.canonical_user_message_digest != admission.canonical_user_message_digest()
        {
            anyhow::bail!("terminal_owner_replay_epoch_admission_conflict");
        }
        let next_generation = epoch
            .generation
            .checked_add(1)
            .context("terminal_owner_replay_epoch_generation_exhausted")?;
        let changed = tx.execute(
            "UPDATE terminal_owner_epochs
             SET generation = ?4, state = 'open', admission_id = ?5,
                 final_event_id = NULL, final_event_payload_digest = NULL,
                 expected_task_owner_revision = NULL,
                 expected_task_owner_digest = NULL, updated_at = ?6
             WHERE task_session_id = ?1 AND run_id = ?2
               AND generation = ?3 AND state = 'sealed'
               AND final_event_id = ?7",
            params![
                admission.task_session_id(),
                admission.run_id(),
                i64::try_from(admission.prior_epoch_generation())?,
                i64::try_from(next_generation)?,
                admission.admission_id(),
                Utc::now().to_rfc3339(),
                admission.prior_final_event_id(),
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("terminal_owner_replay_epoch_open_cas_lost");
        }
        tx.execute(
            "DELETE FROM terminal_owner_final_payloads WHERE epoch_id = ?1",
            [epoch.epoch_id.as_str()],
        )?;
        bind_task_run(&tx, admission.task_session_id(), admission.run_id())?;
        append_event_in_transaction(
            &tx,
            MainChatAgentEventDraft {
                task_session_id: admission.task_session_id().to_string(),
                run_id: admission.run_id().to_string(),
                event_type: "turn_started".into(),
                object_type: "turn".into(),
                object_id: format!("replay:{}", admission.admission_id()),
                created_at: Utc::now(),
                source: "openlife_turn_runtime.replay_epoch".into(),
                payload: json!({
                    "status": "started",
                    "requestId": admission.action_id(),
                    "policyRoute": "governed_replay_successor",
                    "selectedStrategy": "react_tool_execution",
                    "replayCause": admission.cause().as_str(),
                    "replayCauseRef": admission.cause_ref(),
                    "rawUserTextStored": false,
                }),
                backfilled: false,
            },
            &self.digest_key,
        )?;
        tx.commit()?;
        epoch.generation = next_generation;
        epoch.state = TerminalOwnerSealState::Open;
        epoch.final_event_id = None;
        epoch.final_event_payload_digest = None;
        epoch.replayed = true;
        epoch.review_origin = None;
        Ok(epoch)
    }

    pub(crate) fn begin_terminal_owner_seal(
        &self,
        task_session_id: &str,
        run_id: &str,
        generation: u64,
    ) -> Result<TerminalOwnerEpoch> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut epoch = select_terminal_owner_epoch(&tx, task_session_id)?
            .context("terminal_owner_epoch_missing")?;
        if epoch.run_id != run_id || epoch.generation != generation {
            anyhow::bail!("terminal_owner_epoch_identity_mismatch");
        }
        match epoch.state {
            TerminalOwnerSealState::Open => {
                let changed = tx.execute(
                    "UPDATE terminal_owner_epochs
                     SET state = 'sealing', updated_at = ?4
                     WHERE task_session_id = ?1 AND run_id = ?2
                       AND generation = ?3 AND state = 'open'",
                    params![
                        task_session_id,
                        run_id,
                        i64::try_from(generation)?,
                        Utc::now().to_rfc3339(),
                    ],
                )?;
                if changed != 1 {
                    anyhow::bail!("terminal_owner_epoch_seal_cas_lost");
                }
                epoch.state = TerminalOwnerSealState::Sealing;
            }
            TerminalOwnerSealState::Sealing | TerminalOwnerSealState::Sealed => {}
        }
        tx.commit()?;
        Ok(epoch)
    }

    pub(crate) fn stage_terminal_final_payload(
        &self,
        task_session_id: &str,
        run_id: &str,
        generation: u64,
        delivery_id: &str,
        payload: &Value,
    ) -> Result<()> {
        let payload_json = serde_json::to_string(payload)?;
        let payload_digest = metadata_safe_digest(&payload_json);
        let conn = self.lock_conn()?;
        let epoch = select_terminal_owner_epoch(&conn, task_session_id)?
            .context("terminal_owner_epoch_missing")?;
        if epoch.run_id != run_id
            || epoch.generation != generation
            || epoch.state != TerminalOwnerSealState::Sealing
        {
            anyhow::bail!("terminal_owner_final_payload_not_sealing");
        }
        let changed = conn.execute(
            "INSERT INTO terminal_owner_final_payloads (
                epoch_id, delivery_id, payload_json, payload_digest, staged_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(epoch_id) DO UPDATE SET
                delivery_id = excluded.delivery_id,
                payload_json = excluded.payload_json,
                payload_digest = excluded.payload_digest,
                staged_at = excluded.staged_at
             WHERE terminal_owner_final_payloads.delivery_id = excluded.delivery_id
               AND terminal_owner_final_payloads.payload_digest = excluded.payload_digest",
            params![
                epoch.epoch_id,
                delivery_id,
                payload_json,
                payload_digest,
                Utc::now().to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("terminal_owner_final_payload_identity_conflict");
        }
        Ok(())
    }

    pub(crate) fn terminal_owner_staged_final_delivery_id(
        &self,
        task_session_id: &str,
    ) -> Result<Option<String>> {
        let conn = self.lock_conn()?;
        let Some(epoch) = select_terminal_owner_epoch(&conn, task_session_id)? else {
            return Ok(None);
        };
        conn.query_row(
            "SELECT delivery_id FROM terminal_owner_final_payloads WHERE epoch_id = ?1",
            [epoch.epoch_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub(crate) fn append_terminal_final_and_seal(
        &self,
        input: MainChatTerminalFinalizationInput,
    ) -> Result<MainChatAgentDurableEvent> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_sequence_domain(&tx, &input.task_session_id)?;
        validate_task_run_binding(&tx, &input.task_session_id)?;
        bind_task_run(&tx, &input.task_session_id, &input.run_id)?;
        if event_scope_hidden(&tx, "task", &input.task_session_id)?
            || event_scope_hidden(&tx, "run", &input.run_id)?
        {
            anyhow::bail!("main_chat_event_canonical_source_tombstoned");
        }
        let epoch = select_terminal_owner_epoch(&tx, &input.task_session_id)?
            .context("terminal_owner_epoch_missing")?;
        if epoch.run_id != input.run_id || epoch.generation != input.epoch_generation {
            anyhow::bail!("terminal_owner_epoch_identity_mismatch");
        }
        if epoch.state == TerminalOwnerSealState::Sealed {
            let event_id = epoch
                .final_event_id
                .as_deref()
                .context("terminal_owner_sealed_final_missing")?;
            let event = select_event_by_id(&tx, event_id)?
                .context("terminal_owner_sealed_event_missing")?;
            tx.commit()?;
            return Ok(event);
        }
        if epoch.state != TerminalOwnerSealState::Sealing {
            anyhow::bail!("terminal_owner_epoch_not_sealing");
        }
        let staged_payload = tx
            .query_row(
                "SELECT delivery_id, payload_json FROM terminal_owner_final_payloads
                 WHERE epoch_id = ?1",
                [&epoch.epoch_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let mut payload = if let Some((delivery_id, payload_json)) = staged_payload {
            if delivery_id != input.delivery_id {
                anyhow::bail!("terminal_owner_final_payload_identity_mismatch");
            }
            serde_json::from_str::<Value>(&payload_json)?
        } else {
            json!({
                "deliveryId": input.delivery_id,
                "taskSessionId": input.task_session_id,
                "runId": input.run_id,
                "status": if input.status == "waiting_permission" {
                    "completed_with_pending_items"
                } else {
                    input.status.as_str()
                },
                "bodyStored": false,
                "runtimeOwner": "openlife_turn_runtime",
            })
        };
        let payload_object = payload
            .as_object_mut()
            .context("terminal_owner_final_payload_not_object")?;
        payload_object.insert(
            "taskOwnerRevision".into(),
            json!(input.expected_task_owner_revision),
        );
        payload_object.insert(
            "taskOwnerDigest".into(),
            json!(input.expected_task_owner_digest),
        );
        let event = append_event_in_transaction(
            &tx,
            MainChatAgentEventDraft {
                task_session_id: input.task_session_id.clone(),
                run_id: input.run_id.clone(),
                event_type: "final_delivery.created".into(),
                object_type: "final_delivery".into(),
                object_id: input.delivery_id,
                created_at: Utc::now(),
                source: "openlife_turn_runtime.final_delivery_owner".into(),
                payload,
                backfilled: false,
            },
            &self.digest_key,
        )?;
        #[cfg(test)]
        if terminal_final_seal_failpoints()
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal seal failpoint mutex: {error}"))?
            .contains(&input.task_session_id)
        {
            anyhow::bail!("injected_failure_after_final_insert_before_sealed_epoch_cas");
        }
        let changed = tx.execute(
            "UPDATE terminal_owner_epochs
             SET state = 'sealed', final_event_id = ?4,
                 final_event_payload_digest = ?5,
                 expected_task_owner_revision = ?6,
                 expected_task_owner_digest = ?7,
                 updated_at = ?8
             WHERE task_session_id = ?1 AND run_id = ?2
               AND generation = ?3 AND state = 'sealing'",
            params![
                input.task_session_id,
                input.run_id,
                i64::try_from(input.epoch_generation)?,
                event.event_id,
                event.payload_digest,
                i64::try_from(input.expected_task_owner_revision)?,
                input.expected_task_owner_digest,
                Utc::now().to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("terminal_owner_epoch_sealed_cas_lost");
        }
        tx.commit()?;
        Ok(event)
    }

    pub(crate) fn append_terminal_owner_successor(
        &self,
        task_session_id: &str,
        run_id: &str,
        proposal_id: &str,
        receipt: &openlife_core::agent::main_chat_agent_v1::VerifiedTerminalOwnerTransitionReceipt,
    ) -> Result<MainChatAgentDurableEvent> {
        if receipt.owner_kind() != "agent_task_session"
            || receipt.owner_id() != task_session_id
            || receipt.proposal_id() != proposal_id
        {
            anyhow::bail!("terminal_owner_successor_receipt_identity_mismatch");
        }
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_sequence_domain(&tx, task_session_id)?;
        validate_task_run_binding(&tx, task_session_id)?;
        bind_task_run(&tx, task_session_id, run_id)?;
        if event_scope_hidden(&tx, "task", task_session_id)?
            || event_scope_hidden(&tx, "run", run_id)?
        {
            anyhow::bail!("main_chat_event_canonical_source_tombstoned");
        }
        let epoch = select_terminal_owner_epoch(&tx, task_session_id)?
            .context("terminal_owner_epoch_missing")?;
        if epoch.run_id != run_id || epoch.state != TerminalOwnerSealState::Sealed {
            anyhow::bail!("terminal_owner_successor_requires_sealed_epoch");
        }
        let final_event_id = epoch
            .final_event_id
            .as_deref()
            .context("terminal_owner_sealed_final_missing")?;
        let final_event = select_event_by_id(&tx, final_event_id)?
            .context("terminal_owner_sealed_event_missing")?;
        let successor_object_id = format!("successor:{proposal_id}");
        let current_head = terminal_owner_successor_head_from_conn(
            &tx,
            task_session_id,
            final_event_id,
            &final_event,
        )?;
        if let Some(existing) = select_event_by_immutable_identity(
            &tx,
            task_session_id,
            "terminal_owner.successor_confirmed",
            &successor_object_id,
        )? {
            if existing.run_id != run_id
                || existing.payload.get("ownerKind").and_then(Value::as_str)
                    != Some("agent_task_session")
                || existing.payload.get("ownerId").and_then(Value::as_str) != Some(task_session_id)
                || existing.payload.get("causeRef").and_then(Value::as_str) != Some(proposal_id)
                || existing.payload.get("finalEventId").and_then(Value::as_str)
                    != Some(final_event_id)
                || existing
                    .payload
                    .get("beforeOwnerRevision")
                    .and_then(Value::as_u64)
                    != Some(receipt.before_revision())
                || existing
                    .payload
                    .get("afterOwnerRevision")
                    .and_then(Value::as_u64)
                    != Some(receipt.after_revision())
                || existing
                    .payload
                    .get("beforeOwnerDigest")
                    .and_then(Value::as_str)
                    != Some(receipt.before_digest())
                || existing
                    .payload
                    .get("afterOwnerDigest")
                    .and_then(Value::as_str)
                    != Some(receipt.after_digest())
                || existing
                    .payload
                    .get("localTransitionReceiptRef")
                    .and_then(Value::as_str)
                    != Some(receipt.receipt_ref())
                || existing
                    .payload
                    .get("localTransitionReceiptDigest")
                    .and_then(Value::as_str)
                    != Some(receipt.receipt_digest())
            {
                anyhow::bail!("terminal_owner_successor_replay_identity_mismatch");
            }
            tx.commit()?;
            return Ok(existing);
        }
        if current_head.0 != receipt.before_revision() || current_head.1 != receipt.before_digest()
        {
            anyhow::bail!("terminal_owner_successor_head_mismatch");
        }
        let event = append_event_in_transaction(
            &tx,
            MainChatAgentEventDraft {
                task_session_id: task_session_id.to_string(),
                run_id: run_id.to_string(),
                event_type: "terminal_owner.successor_confirmed".into(),
                object_type: "terminal_owner_successor".into(),
                object_id: successor_object_id,
                created_at: Utc::now(),
                source: "terminal_owner_write_gateway.review_successor".into(),
                payload: json!({
                    "causeKind": "proposal_review_acceptance",
                    "causeRef": proposal_id,
                    "finalEventId": final_event_id,
                    "ownerKind": receipt.owner_kind(),
                    "ownerId": receipt.owner_id(),
                    "beforeOwnerRevision": receipt.before_revision(),
                    "afterOwnerRevision": receipt.after_revision(),
                    "beforeOwnerDigest": receipt.before_digest(),
                    "afterOwnerDigest": receipt.after_digest(),
                    "localTransitionReceiptRef": receipt.receipt_ref(),
                    "localTransitionReceiptDigest": receipt.receipt_digest(),
                }),
                backfilled: false,
            },
            &self.digest_key,
        )?;
        tx.commit()?;
        Ok(event)
    }

    #[cfg(test)]
    pub(crate) fn install_fail_after_final_insert_before_sealed_epoch_cas_for_test(
        &self,
        task_session_id: &str,
    ) -> Result<()> {
        terminal_final_seal_failpoints()
            .lock()
            .map_err(|error| anyhow::anyhow!("terminal seal failpoint mutex: {error}"))?
            .insert(task_session_id.to_string());
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn clear_fail_after_final_insert_before_sealed_epoch_cas_for_test(
        &self,
        task_session_id: &str,
    ) {
        if let Ok(mut failpoints) = terminal_final_seal_failpoints().lock() {
            failpoints.remove(task_session_id);
        }
    }

    pub(crate) fn reconciliation_attestation_public_key(&self) -> Result<[u8; 32]> {
        self.digest_key.reconciliation_attestation_public_key()
    }

    /// Reconcile a previous process lifetime's unfinished tool attempts.
    ///
    /// `tool.dispatch_prepared` is a durable risk fence written before the
    /// adapter call. After process death it cannot prove either that dispatch
    /// happened or that it did not. The restart owner therefore emits an
    /// explicit ambiguous transition plus one conservative terminal. It never
    /// restores the attempt to not-attempted and never invents a dispatch
    /// timestamp or attempt count. A durable `tool.started` is stronger: it
    /// preserves its observed dispatch kind/count and receives only the
    /// missing conservative terminal.
    pub(crate) fn reconcile_orphaned_tool_attempts_after_restart(
        &self,
        limit: usize,
    ) -> Result<MainChatOrphanedToolReconciliationReport> {
        let bounded_limit = limit.clamp(1, 250);
        let query_limit = i64::try_from(bounded_limit.saturating_add(1))?;
        let now = Utc::now();
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut prepared = {
            let mut statement = tx.prepare(
                "SELECT p.event_id, p.task_session_id, p.run_id, p.sequence,
                        p.event_type, p.object_type, p.object_id, p.created_at,
                        p.source, p.payload_digest, p.payload_json, p.backfilled,
                        p.payload_minimized_version
                 FROM main_chat_agent_events p
                 WHERE p.event_type = 'tool.dispatch_prepared'
                   AND NOT EXISTS (
                       SELECT 1
                       FROM main_chat_agent_event_immutable_identities i
                       WHERE i.task_session_id = p.task_session_id
                         AND i.object_id = p.object_id
                         AND i.event_type IN (
                             'tool.not_dispatched', 'tool.dispatch_ambiguous', 'tool.started',
                             'tool.completed', 'tool.failed',
                             'tool.effect_unknown', 'tool.local_aborted',
                             'tool.remote_unknown'
                         )
                   )
                 ORDER BY p.created_at ASC, p.task_session_id ASC, p.sequence ASC
                 LIMIT ?1",
            )?;
            let rows = statement.query_map([query_limit], row_to_event)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let has_more = prepared.len() > bounded_limit;
        prepared.truncate(bounded_limit);
        let mut report = MainChatOrphanedToolReconciliationReport {
            has_more,
            ..MainChatOrphanedToolReconciliationReport::default()
        };

        for prepared in prepared {
            let may_outlive = prepared
                .payload
                .get("mayOutliveLocalProcess")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let effect_may_survive = prepared
                .payload
                .get("effectMaySurviveLocalProcess")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| {
                    prepared.payload.get("actionEffect").and_then(Value::as_str)
                        != Some("read_only")
                });
            let process_risk = prepared
                .payload
                .get("dispatchProcessRisk")
                .and_then(Value::as_str)
                .unwrap_or(if may_outlive {
                    "may_outlive_local_process"
                } else {
                    "process_bound"
                });
            let transport_status = if may_outlive {
                "remote_unknown"
            } else {
                "local_aborted"
            };
            let effect_status = if effect_may_survive {
                "unknown"
            } else {
                "not_attempted"
            };
            let terminal = if may_outlive {
                report.remote_unknown = report.remote_unknown.saturating_add(1);
                ("tool.remote_unknown", "remote_unknown")
            } else if effect_may_survive {
                report.effect_unknown = report.effect_unknown.saturating_add(1);
                ("tool.effect_unknown", "effect_unknown")
            } else {
                report.local_aborted = report.local_aborted.saturating_add(1);
                ("tool.local_aborted", "local_aborted")
            };
            let mut common = json!({
                "receiptId": prepared.object_id.clone(),
                "requestId": prepared.object_id.clone(),
                "sourceRunId": prepared.run_id.clone(),
                "manifestId": prepared.payload.get("manifestId").cloned().unwrap_or(Value::Null),
                "requestDigest": prepared.payload.get("requestDigest").cloned().unwrap_or(Value::Null),
                "actionEffect": prepared.payload.get("actionEffect").cloned().unwrap_or(Value::Null),
                "idempotencyContract": prepared.payload.get("idempotencyContract").cloned().unwrap_or(Value::Null),
                "dispatchKind": "unknown",
                "dispatchAttemptCount": 0,
                "transportStatus": transport_status,
                "effectStatus": effect_status,
                "executionOutcome": "unknown",
                "auditPersistenceStatus": "unknown",
                "startedAt": Value::Null,
                "dispatchedAt": Value::Null,
                "responseObservedAt": Value::Null,
                "finishedAt": Value::Null,
                "dispatchObserved": false,
                "reconciledAfterProcessRestart": true,
                "preparedEventId": prepared.event_id.clone(),
                "dispatchProcessRisk": process_risk,
                "mayOutliveLocalProcess": may_outlive,
                "effectMaySurviveLocalProcess": effect_may_survive,
            });
            if let (
                Some(replay_action_id),
                Some(replay_claim_id),
                Some(replay_owner_generation),
                Some(replay_authority_binding),
            ) = (
                prepared.payload.get("replayActionId").cloned(),
                prepared.payload.get("replayClaimId").cloned(),
                prepared.payload.get("replayClaimOwnerGeneration").cloned(),
                prepared.payload.get("replayAuthorityBinding").cloned(),
            ) {
                let object = common
                    .as_object_mut()
                    .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_common_payload_invalid"))?;
                object.insert("replayActionId".into(), replay_action_id);
                object.insert("replayClaimId".into(), replay_claim_id);
                object.insert("replayClaimOwnerGeneration".into(), replay_owner_generation);
                object.insert("replayAuthorityBinding".into(), replay_authority_binding);
            }
            let mut ambiguous_payload = common.clone();
            ambiguous_payload["status"] = json!("dispatch_ambiguous");
            let ambiguous = append_event_in_transaction(
                &tx,
                MainChatAgentEventDraft {
                    task_session_id: prepared.task_session_id.clone(),
                    run_id: prepared.run_id.clone(),
                    event_type: "tool.dispatch_ambiguous".into(),
                    object_type: "tool_execution_receipt".into(),
                    object_id: prepared.object_id.clone(),
                    created_at: now,
                    source: "bootstrap.prepared_tool_reconciliation".into(),
                    payload: ambiguous_payload,
                    backfilled: false,
                },
                &self.digest_key,
            )?;
            let mut terminal_payload = common;
            terminal_payload["status"] = json!(terminal.1);
            terminal_payload["finishedAt"] = json!(now);
            append_event_in_transaction(
                &tx,
                MainChatAgentEventDraft {
                    task_session_id: prepared.task_session_id.clone(),
                    run_id: prepared.run_id.clone(),
                    event_type: terminal.0.into(),
                    object_type: "tool_execution_receipt".into(),
                    object_id: prepared.object_id.clone(),
                    created_at: now,
                    source: "bootstrap.prepared_tool_reconciliation".into(),
                    payload: terminal_payload,
                    backfilled: false,
                },
                &self.digest_key,
            )?;
            enqueue_tool_queue_reconciliation_projection(
                &tx,
                &prepared,
                &ambiguous,
                if may_outlive || effect_may_survive {
                    MainChatToolQueueReconciliationDisposition::DispatchedUnknown
                } else {
                    MainChatToolQueueReconciliationDisposition::EffectNotAttempted
                },
                now,
                &self.digest_key,
            )?;
            debug_assert_eq!(ambiguous.event_type, "tool.dispatch_ambiguous");
            report.examined = report.examined.saturating_add(1);
        }

        let remaining = bounded_limit.saturating_sub(report.examined);
        if !report.has_more {
            let started_query_limit = i64::try_from(remaining.saturating_add(1))?;
            let mut started = {
                let mut statement = tx.prepare(
                    "SELECT s.event_id, s.task_session_id, s.run_id, s.sequence,
                            s.event_type, s.object_type, s.object_id, s.created_at,
                            s.source, s.payload_digest, s.payload_json, s.backfilled,
                            s.payload_minimized_version
                     FROM main_chat_agent_events s
                     WHERE s.event_type = 'tool.started'
                       AND NOT EXISTS (
                           SELECT 1
                           FROM main_chat_agent_event_immutable_identities i
                           WHERE i.task_session_id = s.task_session_id
                             AND i.object_id = s.object_id
                             AND i.event_type IN (
                                 'tool.not_dispatched',
                                 'tool.completed', 'tool.failed',
                                 'tool.effect_unknown', 'tool.local_aborted',
                                 'tool.remote_unknown'
                             )
                       )
                     ORDER BY s.created_at ASC, s.task_session_id ASC, s.sequence ASC
                     LIMIT ?1",
                )?;
                let rows = statement.query_map([started_query_limit], row_to_event)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()?
            };
            report.has_more = started.len() > remaining;
            started.truncate(remaining);
            for started in started {
                let dispatch_kind = started
                    .payload
                    .get("dispatchKind")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let may_outlive =
                    matches!(dispatch_kind, "network" | "mcp_stdio" | "a2a" | "unknown");
                let effect_may_survive =
                    started.payload.get("actionEffect").and_then(Value::as_str)
                        != Some("read_only");
                let effect_already_confirmed =
                    started.payload.get("effectStatus").and_then(Value::as_str)
                        == Some("confirmed");
                let effect_status = if effect_already_confirmed {
                    "confirmed"
                } else if effect_may_survive {
                    "unknown"
                } else {
                    "not_attempted"
                };
                let terminal = if may_outlive {
                    report.remote_unknown = report.remote_unknown.saturating_add(1);
                    ("tool.remote_unknown", "remote_unknown")
                } else if effect_may_survive && !effect_already_confirmed {
                    report.effect_unknown = report.effect_unknown.saturating_add(1);
                    ("tool.effect_unknown", "effect_unknown")
                } else {
                    report.local_aborted = report.local_aborted.saturating_add(1);
                    ("tool.local_aborted", "local_aborted")
                };
                let mut terminal_payload = started.payload.clone();
                terminal_payload["status"] = json!(terminal.1);
                terminal_payload["transportStatus"] = json!(if may_outlive {
                    "remote_unknown"
                } else {
                    "local_aborted"
                });
                terminal_payload["effectStatus"] = json!(effect_status);
                terminal_payload["executionOutcome"] = json!("unknown");
                terminal_payload["responseObservedAt"] = Value::Null;
                terminal_payload["finishedAt"] = json!(now);
                terminal_payload["dispatchObserved"] = json!(true);
                terminal_payload["reconciledAfterProcessRestart"] = json!(true);
                append_event_in_transaction(
                    &tx,
                    MainChatAgentEventDraft {
                        task_session_id: started.task_session_id,
                        run_id: started.run_id,
                        event_type: terminal.0.into(),
                        object_type: "tool_execution_receipt".into(),
                        object_id: started.object_id,
                        created_at: now,
                        source: "bootstrap.started_tool_reconciliation".into(),
                        payload: terminal_payload,
                        backfilled: false,
                    },
                    &self.digest_key,
                )?;
                report.examined = report.examined.saturating_add(1);
            }
        }
        tx.commit()?;
        Ok(report)
    }

    pub(crate) fn pending_tool_queue_reconciliation_projections(
        &self,
        limit: usize,
    ) -> Result<MainChatToolQueueReconciliationBatch> {
        let bounded_limit = limit.clamp(1, 250);
        let query_limit = i64::try_from(bounded_limit.saturating_add(1))?;
        let conn = self.lock_conn()?;
        let invalid_pending = conn
            .query_row(
                "SELECT 1
                 FROM main_chat_tool_queue_reconciliation_outbox o
                 LEFT JOIN main_chat_agent_events p ON p.event_id = o.prepared_event_id
                 LEFT JOIN main_chat_agent_events r ON r.event_id = o.resolution_event_id
                 WHERE o.state = 'pending'
                   AND (p.event_id IS NULL
                        OR p.event_type != 'tool.dispatch_prepared'
                        OR p.payload_digest != o.prepared_payload_digest
                        OR p.task_session_id != o.task_session_id
                        OR p.run_id != o.run_id
                        OR p.object_id != o.receipt_id
                        OR r.event_id IS NULL
                        OR r.payload_digest != o.resolution_payload_digest
                        OR r.task_session_id != o.task_session_id
                        OR r.run_id != o.run_id
                        OR r.object_id != o.receipt_id
                        OR r.event_type NOT IN (
                            'tool.not_dispatched', 'tool.dispatch_ambiguous'
                        ))
                 LIMIT 1",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if invalid_pending {
            anyhow::bail!("tool_reconciliation_outbox_prepared_event_authority_invalid");
        }
        let mut statement = conn.prepare(
            "SELECT o.outbox_id, o.prepared_event_id, o.prepared_payload_digest,
                    o.resolution_event_id, o.resolution_payload_digest,
                    o.task_session_id, o.run_id, o.receipt_id,
                    o.replay_action_id, o.replay_claim_id,
                    o.replay_claim_owner_generation, o.replay_authority_binding,
                    o.disposition, o.event_store_attestation,
                    p.task_session_id, p.run_id, p.object_id,
                    p.payload_digest, p.payload_json,
                    r.task_session_id, r.run_id, r.object_id,
                    r.payload_digest, r.event_type
             FROM main_chat_tool_queue_reconciliation_outbox o
             INNER JOIN main_chat_agent_events p ON p.event_id = o.prepared_event_id
             INNER JOIN main_chat_agent_events r ON r.event_id = o.resolution_event_id
             WHERE o.state = 'pending' AND p.event_type = 'tool.dispatch_prepared'
             ORDER BY o.created_at ASC, o.outbox_id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([query_limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, String>(13)?,
                row.get::<_, String>(14)?,
                row.get::<_, String>(15)?,
                row.get::<_, String>(16)?,
                row.get::<_, String>(17)?,
                row.get::<_, String>(18)?,
                row.get::<_, String>(19)?,
                row.get::<_, String>(20)?,
                row.get::<_, String>(21)?,
                row.get::<_, String>(22)?,
                row.get::<_, String>(23)?,
            ))
        })?;
        let mut raw = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = raw.len() > bounded_limit;
        raw.truncate(bounded_limit);
        let mut items = Vec::with_capacity(raw.len());
        for (
            outbox_id,
            prepared_event_id,
            prepared_payload_digest,
            resolution_event_id,
            resolution_payload_digest,
            task_session_id,
            run_id,
            receipt_id,
            replay_action_id,
            replay_claim_id,
            replay_claim_owner_generation,
            replay_authority_binding,
            disposition,
            event_store_attestation,
            event_task_session_id,
            event_run_id,
            event_receipt_id,
            event_payload_digest,
            event_payload_json,
            resolution_task_session_id,
            resolution_run_id,
            resolution_receipt_id,
            resolution_event_payload_digest,
            resolution_event_type,
        ) in raw
        {
            let expected_outbox_id = format!(
                "tool_queue_reconciliation:v2:{}",
                openlife_core::persistence_outbox::metadata_digest(&prepared_event_id)
            );
            if !is_bounded_event_reference(&outbox_id, 512)
                || outbox_id != expected_outbox_id
                || !is_bounded_event_reference(&prepared_event_id, 512)
                || !is_exact_metadata_digest(&prepared_payload_digest)
                || !is_bounded_event_reference(&resolution_event_id, 512)
                || !is_exact_metadata_digest(&resolution_payload_digest)
                || !is_bounded_event_reference(&task_session_id, 384)
                || !is_bounded_event_reference(&run_id, 384)
                || !is_bounded_event_reference(&receipt_id, 384)
                || !is_bounded_event_reference(&replay_action_id, 384)
                || uuid::Uuid::parse_str(&replay_claim_id).is_err()
                || replay_claim_owner_generation <= 0
                || !is_bounded_event_reference(&replay_authority_binding, 384)
                || prepared_payload_digest != event_payload_digest
                || task_session_id != event_task_session_id
                || run_id != event_run_id
                || receipt_id != event_receipt_id
                || resolution_payload_digest != resolution_event_payload_digest
                || task_session_id != resolution_task_session_id
                || run_id != resolution_run_id
                || receipt_id != resolution_receipt_id
            {
                anyhow::bail!("tool_reconciliation_outbox_identity_invalid");
            }
            let payload: Value = serde_json::from_str(&event_payload_json)
                .context("tool_reconciliation_prepared_payload_invalid")?;
            let payload_string = |key: &str| -> Result<String> {
                payload
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|value| is_bounded_event_reference(value, 384))
                    .map(str::to_string)
                    .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_prepared_{key}_invalid"))
            };
            if payload.get("replayActionId").and_then(Value::as_str)
                != Some(replay_action_id.as_str())
                || payload.get("replayClaimId").and_then(Value::as_str)
                    != Some(replay_claim_id.as_str())
                || payload
                    .get("replayClaimOwnerGeneration")
                    .and_then(Value::as_u64)
                    != u64::try_from(replay_claim_owner_generation).ok()
                || payload
                    .get("replayAuthorityBinding")
                    .and_then(Value::as_str)
                    != Some(replay_authority_binding.as_str())
            {
                anyhow::bail!("tool_reconciliation_prepared_outbox_binding_mismatch");
            }
            let action_effect: ToolActionEffect =
                serde_json::from_value(payload.get("actionEffect").cloned().ok_or_else(|| {
                    anyhow::anyhow!("tool_reconciliation_action_effect_missing")
                })?)?;
            let idempotency_contract =
                serde_json::from_value(payload.get("idempotencyContract").cloned().ok_or_else(
                    || anyhow::anyhow!("tool_reconciliation_idempotency_contract_missing"),
                )?)?;
            let process_risk = match payload
                .get("dispatchProcessRisk")
                .and_then(Value::as_str)
            {
                Some("process_bound") => {
                    openlife_core::agent::action_executor::ToolDispatchProcessRisk::ProcessBound
                }
                Some("may_outlive_local_process") => openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess,
                _ => anyhow::bail!("tool_reconciliation_process_risk_invalid"),
            };
            let disposition = MainChatToolQueueReconciliationDisposition::from_str(&disposition)?;
            let resolution = core_tool_reconciliation_resolution(&resolution_event_type)?;
            if !matches!(
                (disposition, resolution),
                (
                    MainChatToolQueueReconciliationDisposition::EffectNotAttempted,
                    openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution::NotDispatched
                        | openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution::DispatchAmbiguous
                ) | (
                    MainChatToolQueueReconciliationDisposition::DispatchedUnknown,
                    openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution::DispatchAmbiguous
                )
            ) {
                anyhow::bail!("tool_reconciliation_resolution_disposition_invalid");
            }
            let projection = MainChatToolQueueReconciliationProjection {
                outbox_id,
                prepared_event_id,
                prepared_payload_digest,
                resolution_event_id,
                resolution_payload_digest,
                resolution,
                task_session_id,
                run_id,
                receipt_id,
                replay_action_id,
                replay_claim_id,
                replay_claim_owner_generation: u64::try_from(replay_claim_owner_generation)?,
                manifest_id: payload_string("manifestId")?,
                tool_name: payload_string("toolName")?,
                manifest_contract_digest: payload_string("manifestContractDigest")?,
                input_hash: payload_string("inputHash")?,
                input_length_bytes: payload
                    .get("inputLengthBytes")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_input_length_invalid"))?,
                request_digest: payload_string("requestDigest")?,
                action_effect,
                idempotency_contract,
                process_risk,
                effect_may_survive_local_process: payload
                    .get("effectMaySurviveLocalProcess")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        anyhow::anyhow!("tool_reconciliation_effect_process_contract_invalid")
                    })?,
                replay_authority_binding,
                disposition,
                event_store_attestation,
            };
            let core_disposition = match projection.disposition {
                MainChatToolQueueReconciliationDisposition::EffectNotAttempted => openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationDisposition::EffectNotAttempted,
                MainChatToolQueueReconciliationDisposition::DispatchedUnknown => openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationDisposition::DispatchedUnknown,
            };
            let envelope = openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationEnvelope {
                outbox_id: &projection.outbox_id,
                prepared_event_id: &projection.prepared_event_id,
                prepared_payload_digest: &projection.prepared_payload_digest,
                resolution_event_id: &projection.resolution_event_id,
                resolution_payload_digest: &projection.resolution_payload_digest,
                resolution: projection.resolution,
                task_session_id: &projection.task_session_id,
                run_id: &projection.run_id,
                receipt_id: &projection.receipt_id,
                action_id: &projection.replay_action_id,
                replay_claim_id: &projection.replay_claim_id,
                replay_claim_owner_generation: projection.replay_claim_owner_generation,
                manifest_id: &projection.manifest_id,
                tool_name: &projection.tool_name,
                manifest_contract_digest: &projection.manifest_contract_digest,
                input_hash: &projection.input_hash,
                input_length_bytes: projection.input_length_bytes,
                request_digest: &projection.request_digest,
                action_effect: projection.action_effect,
                idempotency_contract: projection.idempotency_contract,
                process_risk: projection.process_risk,
                effect_may_survive_local_process: projection.effect_may_survive_local_process,
                replay_authority_binding: &projection.replay_authority_binding,
                disposition: core_disposition,
            };
            if self.digest_key.sign_reconciliation_attestation(&envelope)?
                != projection.event_store_attestation
            {
                anyhow::bail!("tool_reconciliation_event_store_attestation_invalid");
            }
            items.push(projection);
        }
        Ok(MainChatToolQueueReconciliationBatch { items, has_more })
    }

    pub(crate) fn mark_tool_queue_reconciliation_projection_applied(
        &self,
        projection: &MainChatToolQueueReconciliationProjection,
    ) -> Result<()> {
        let conn = self.lock_conn()?;
        let changed = conn.execute(
            "UPDATE main_chat_tool_queue_reconciliation_outbox
             SET state = 'applied', applied_at = ?2
             WHERE outbox_id = ?1
               AND prepared_event_id = ?3
               AND prepared_payload_digest = ?4
               AND resolution_event_id = ?5
               AND resolution_payload_digest = ?6
               AND task_session_id = ?7
               AND run_id = ?8
               AND receipt_id = ?9
               AND replay_action_id = ?10
               AND replay_claim_id = ?11
               AND replay_claim_owner_generation = ?12
               AND replay_authority_binding = ?13
               AND disposition = ?14
               AND event_store_attestation = ?15
               AND state = 'pending'",
            params![
                projection.outbox_id,
                Utc::now().to_rfc3339(),
                projection.prepared_event_id,
                projection.prepared_payload_digest,
                projection.resolution_event_id,
                projection.resolution_payload_digest,
                projection.task_session_id,
                projection.run_id,
                projection.receipt_id,
                projection.replay_action_id,
                projection.replay_claim_id,
                i64::try_from(projection.replay_claim_owner_generation)?,
                projection.replay_authority_binding,
                projection.disposition.as_str(),
                projection.event_store_attestation,
            ],
        )?;
        if changed == 1 {
            return Ok(());
        }
        let already_applied = conn
            .query_row(
                "SELECT 1 FROM main_chat_tool_queue_reconciliation_outbox
                 WHERE outbox_id = ?1
                   AND prepared_event_id = ?2
                   AND prepared_payload_digest = ?3
                   AND resolution_event_id = ?4
                   AND resolution_payload_digest = ?5
                   AND task_session_id = ?6
                   AND run_id = ?7
                   AND receipt_id = ?8
                   AND replay_action_id = ?9
                   AND replay_claim_id = ?10
                   AND replay_claim_owner_generation = ?11
                   AND replay_authority_binding = ?12
                   AND disposition = ?13
                   AND event_store_attestation = ?14
                   AND state = 'applied'",
                params![
                    projection.outbox_id,
                    projection.prepared_event_id,
                    projection.prepared_payload_digest,
                    projection.resolution_event_id,
                    projection.resolution_payload_digest,
                    projection.task_session_id,
                    projection.run_id,
                    projection.receipt_id,
                    projection.replay_action_id,
                    projection.replay_claim_id,
                    i64::try_from(projection.replay_claim_owner_generation)?,
                    projection.replay_authority_binding,
                    projection.disposition.as_str(),
                    projection.event_store_attestation,
                ],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !already_applied {
            anyhow::bail!("tool_reconciliation_outbox_apply_cas_failed");
        }
        Ok(())
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_event_sequences (
                task_session_id TEXT PRIMARY KEY,
                last_sequence INTEGER NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_event_task_runs (
                task_session_id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS terminal_owner_epochs (
                epoch_id TEXT PRIMARY KEY,
                task_session_id TEXT NOT NULL UNIQUE,
                run_id TEXT NOT NULL UNIQUE,
                generation INTEGER NOT NULL CHECK(generation > 0),
                state TEXT NOT NULL CHECK(state IN ('open', 'sealing', 'sealed')),
                admission_id TEXT NOT NULL UNIQUE,
                canonical_user_message_ref TEXT NOT NULL,
                canonical_user_message_digest TEXT NOT NULL,
                canonical_store_identity TEXT NOT NULL,
                final_event_id TEXT,
                final_event_payload_digest TEXT,
                expected_task_owner_revision INTEGER,
                expected_task_owner_digest TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS terminal_owner_final_payloads (
                epoch_id TEXT PRIMARY KEY,
                delivery_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                staged_at TEXT NOT NULL,
                FOREIGN KEY(epoch_id) REFERENCES terminal_owner_epochs(epoch_id)
             ) WITHOUT ROWID;",
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_events (
                event_id TEXT PRIMARY KEY,
                task_session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                object_type TEXT NOT NULL,
                object_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                source TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                payload_minimized_version INTEGER NOT NULL DEFAULT 0,
                backfilled INTEGER NOT NULL DEFAULT 0,
                UNIQUE(task_session_id, sequence),
                UNIQUE(task_session_id, event_type, object_id, payload_digest, backfilled)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_main_chat_agent_events_replay
             ON main_chat_agent_events(task_session_id, sequence)",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_event_immutable_identities (
                task_session_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                object_id TEXT NOT NULL,
                payload_digest TEXT NOT NULL,
                event_id TEXT NOT NULL,
                PRIMARY KEY(task_session_id, event_type, object_id),
                UNIQUE(event_id)
            )",
            [],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_event_tombstone_projections (
                canonical_tombstone_id TEXT NOT NULL,
                scope_kind TEXT NOT NULL CHECK(scope_kind IN ('task', 'run')),
                scope_id TEXT NOT NULL,
                active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0, 1)),
                applied_event_id TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                PRIMARY KEY(canonical_tombstone_id, scope_kind, scope_id)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_main_chat_event_active_tombstone_scope
             ON main_chat_agent_event_tombstone_projections(scope_kind, scope_id, active);
             CREATE TABLE IF NOT EXISTS main_chat_agent_run_projection_heads (
                run_id TEXT PRIMARY KEY,
                canonical_revision INTEGER NOT NULL,
                canonical_event_id TEXT NOT NULL,
                hidden INTEGER NOT NULL CHECK(hidden IN (0, 1)),
                canonical_tombstone_id TEXT,
                applied_at TEXT NOT NULL
             );",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS main_chat_cancellation_projection_deliveries (
                cancellation_id TEXT NOT NULL,
                task_session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                terminal_event_id TEXT NOT NULL,
                projection_target TEXT NOT NULL CHECK(projection_target IN (
                    'agent_run', 'task_session', 'action_queue'
                )),
                state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN (
                    'pending', 'applied', 'degraded'
                )),
                attempts INTEGER NOT NULL DEFAULT 0,
                error_digest TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(cancellation_id, projection_target)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_main_chat_cancellation_projection_replay
             ON main_chat_cancellation_projection_deliveries(state, updated_at);",
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS main_chat_tool_queue_reconciliation_outbox (
                outbox_id TEXT PRIMARY KEY,
                prepared_event_id TEXT NOT NULL UNIQUE,
                prepared_payload_digest TEXT NOT NULL,
                resolution_event_id TEXT NOT NULL UNIQUE,
                resolution_payload_digest TEXT NOT NULL,
                task_session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                receipt_id TEXT NOT NULL,
                replay_action_id TEXT NOT NULL,
                replay_claim_id TEXT NOT NULL,
                replay_claim_owner_generation INTEGER NOT NULL
                    CHECK(replay_claim_owner_generation > 0),
                replay_authority_binding TEXT NOT NULL,
                event_store_attestation TEXT NOT NULL,
                disposition TEXT NOT NULL CHECK(disposition IN (
                    'effect_not_attempted', 'dispatched_unknown'
                )),
                state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN (
                    'pending', 'applied'
                )),
                created_at TEXT NOT NULL,
                applied_at TEXT
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_main_chat_tool_queue_reconciliation_pending
             ON main_chat_tool_queue_reconciliation_outbox(state, created_at, outbox_id);",
        )?;
        openlife_core::sqlite_migration::ensure_column(
            &conn,
            "main_chat_tool_queue_reconciliation_outbox",
            "prepared_payload_digest",
            "TEXT NOT NULL DEFAULT 'legacy_unverified'",
        )?;
        openlife_core::sqlite_migration::ensure_column(
            &conn,
            "main_chat_tool_queue_reconciliation_outbox",
            "resolution_event_id",
            "TEXT NOT NULL DEFAULT 'legacy_unverified'",
        )?;
        openlife_core::sqlite_migration::ensure_column(
            &conn,
            "main_chat_tool_queue_reconciliation_outbox",
            "resolution_payload_digest",
            "TEXT NOT NULL DEFAULT 'legacy_unverified'",
        )?;
        openlife_core::sqlite_migration::ensure_column(
            &conn,
            "main_chat_tool_queue_reconciliation_outbox",
            "replay_claim_owner_generation",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        openlife_core::sqlite_migration::ensure_column(
            &conn,
            "main_chat_tool_queue_reconciliation_outbox",
            "replay_authority_binding",
            "TEXT NOT NULL DEFAULT 'legacy_unverified'",
        )?;
        openlife_core::sqlite_migration::ensure_column(
            &conn,
            "main_chat_tool_queue_reconciliation_outbox",
            "event_store_attestation",
            "TEXT NOT NULL DEFAULT 'legacy_unverified'",
        )?;
        conn.execute_batch(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_main_chat_tool_queue_reconciliation_resolution
             ON main_chat_tool_queue_reconciliation_outbox(resolution_event_id)
             WHERE resolution_event_id != 'legacy_unverified';",
        )?;
        conn.execute(
            "UPDATE main_chat_tool_queue_reconciliation_outbox
             SET state = 'applied', applied_at = COALESCE(applied_at, ?1)
             WHERE state = 'pending'
               AND (prepared_payload_digest = 'legacy_unverified'
                    OR resolution_event_id = 'legacy_unverified'
                    OR resolution_payload_digest = 'legacy_unverified'
                    OR replay_claim_owner_generation <= 0
                    OR replay_authority_binding = 'legacy_unverified'
                    OR event_store_attestation = 'legacy_unverified')",
            [Utc::now().to_rfc3339()],
        )?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS main_chat_agent_event_store_metadata (
                key TEXT PRIMARY KEY,
                value INTEGER NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS main_chat_agent_event_identity_quarantine (
                quarantine_id TEXT PRIMARY KEY,
                task_identity_digest TEXT NOT NULL,
                row_set_digest TEXT NOT NULL,
                event_count INTEGER NOT NULL,
                reason_code TEXT NOT NULL,
                quarantined_at TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS main_chat_agent_event_fact_quarantine (
                event_id TEXT PRIMARY KEY,
                task_session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                reason_code TEXT NOT NULL,
                quarantined_at TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS idx_main_chat_event_fact_quarantine_task
             ON main_chat_agent_event_fact_quarantine(task_session_id, run_id);",
        )?;
        bind_event_digest_key(&conn, &self.digest_key)?;
        ensure_event_payload_minimized_version_column(&conn)?;
        migrate_legacy_event_identities(&conn, &self.digest_key)?;
        validate_current_event_identity_domain(&conn)?;
        validate_immutable_identity_domain(&conn)?;
        migrate_legacy_event_payloads(&conn, &self.digest_key)?;
        validate_supported_payload_versions(&conn)?;
        validate_immutable_identity_domain(&conn)?;
        ensure_immutable_identity_constraint(&conn)?;
        validate_and_backfill_task_run_bindings(&conn)?;
        validate_all_task_sequence_domains(&conn)?;
        Ok(())
    }

    /// Read-only recovery surface for tasks removed from the live replay
    /// domain because their legacy identity or provider-lifecycle facts could
    /// not be safely upgraded. Receipts intentionally contain no original
    /// identifiers or payload bodies.
    pub(crate) fn list_identity_quarantine_receipts(
        &self,
        limit: usize,
    ) -> Result<Vec<MainChatEventIdentityQuarantineReceipt>> {
        let conn = self.lock_conn()?;
        let mut statement = conn.prepare(
            "SELECT quarantine_id, task_identity_digest, row_set_digest,
                    event_count, reason_code, quarantined_at
             FROM main_chat_agent_event_identity_quarantine
             ORDER BY quarantined_at ASC, quarantine_id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::try_from(limit.clamp(1, 250))?], |row| {
            let count = row.get::<_, i64>(3)?;
            let event_count = u64::try_from(count).map_err(|_| {
                row_decode_fault(3, Type::Integer, "quarantine_event_count", "negative")
            })?;
            let raw_timestamp = row.get::<_, String>(5)?;
            let quarantined_at = DateTime::parse_from_rfc3339(&raw_timestamp)
                .map(|timestamp| timestamp.with_timezone(&Utc))
                .map_err(|_| {
                    row_decode_fault(5, Type::Text, "quarantine_timestamp", "invalid_rfc3339")
                })?;
            Ok(MainChatEventIdentityQuarantineReceipt {
                quarantine_id: row.get(0)?,
                task_identity_digest: row.get(1)?,
                row_set_digest: row.get(2)?,
                event_count,
                reason_code: row.get(4)?,
                quarantined_at,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn append(&self, draft: MainChatAgentEventDraft) -> Result<MainChatAgentDurableEvent> {
        #[cfg(test)]
        if is_provider_lifecycle_draft(&draft) {
            return self.append_synthetic_provider_draft_for_test(draft);
        }
        let mut events =
            self.append_batch_internal(vec![draft], ProviderLifecycleAdmission::None)?;
        events
            .pop()
            .context("single event append returned no durable event")
    }

    /// Persist a zero-attempt terminal from the same process-local, sealed
    /// ToolGateway receipt. If a prepared dispatch fence exists, the terminal
    /// closes it and enqueues the ActionQueue reconciliation in the same
    /// transaction. A contract or policy rejection may have no prepared fence;
    /// its live receipt still owns a standalone `tool.not_dispatched` fact.
    /// Generic event inputs cannot construct either admission.
    fn append_live_not_dispatched_tool_receipt(
        &self,
        task_session_id: &str,
        run_id: &str,
        receipt: &ToolExecutionReceipt,
        source: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        if !receipt.proves_not_dispatched() {
            anyhow::bail!("tool_not_dispatched_live_receipt_proof_invalid");
        }
        if receipt.source_run_id.as_deref() != Some(run_id) {
            anyhow::bail!("tool_not_dispatched_run_identity_mismatch");
        }
        let manifest_id = receipt
            .manifest_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("tool_not_dispatched_manifest_id_missing"))?;
        let finished_at = receipt
            .finished_at
            .ok_or_else(|| anyhow::anyhow!("tool_not_dispatched_finished_at_missing"))?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_task_sequence_domain(&tx, task_session_id)?;
        validate_task_run_binding(&tx, task_session_id)?;
        let prepared = select_event_by_immutable_identity(
            &tx,
            task_session_id,
            "tool.dispatch_prepared",
            &receipt.receipt_id,
        )?;
        bind_task_run(&tx, task_session_id, run_id)?;
        if event_scope_hidden(&tx, "task", task_session_id)?
            || event_scope_hidden(&tx, "run", run_id)?
        {
            anyhow::bail!("main_chat_event_canonical_source_tombstoned");
        }
        let mut payload = json!({
            "receiptId": receipt.receipt_id,
            "requestId": receipt.receipt_id,
            "sourceRunId": run_id,
            "manifestId": manifest_id,
            "requestDigest": receipt.request_digest,
            "actionEffect": receipt.action_effect.as_str(),
            "idempotencyContract": receipt.idempotency_contract.as_str(),
            "dispatchKind": "not_attempted",
            "dispatchAttemptCount": 0,
            "dispatchObserved": false,
            "transportStatus": "not_attempted",
            "effectStatus": "not_attempted",
            "executionOutcome": receipt.execution_outcome.as_str(),
            "auditPersistenceStatus": receipt.audit_persistence_status.as_str(),
            "status": "not_dispatched",
            "startedAt": receipt.started_at,
            "dispatchedAt": Value::Null,
            "responseObservedAt": Value::Null,
            "finishedAt": finished_at,
            "reconciledAfterProcessRestart": false,
        });
        if let (Some(object), Some(prepared)) = (payload.as_object_mut(), prepared.as_ref()) {
            object.insert("preparedEventId".into(), json!(prepared.event_id.clone()));
        }
        let resolution = append_event_in_transaction_with_tool_admission(
            &tx,
            MainChatAgentEventDraft {
                task_session_id: task_session_id.to_string(),
                run_id: run_id.to_string(),
                event_type: "tool.not_dispatched".into(),
                object_type: "tool_execution_receipt".into(),
                object_id: receipt.receipt_id.clone(),
                created_at: finished_at,
                source: source.to_string(),
                payload,
                backfilled: false,
            },
            &self.digest_key,
            ToolLifecycleAdmission::LiveNotDispatched(receipt),
        )?;
        if let Some(prepared) = prepared.as_ref() {
            enqueue_tool_queue_reconciliation_projection(
                &tx,
                prepared,
                &resolution,
                MainChatToolQueueReconciliationDisposition::EffectNotAttempted,
                finished_at,
                &self.digest_key,
            )?;
        }
        tx.commit()?;
        Ok(Some(resolution))
    }

    #[cfg(test)]
    fn append_synthetic_provider_draft_for_test(
        &self,
        mut draft: MainChatAgentEventDraft,
    ) -> Result<MainChatAgentDurableEvent> {
        let request_id = draft.object_id.clone();
        let provider = draft
            .payload
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("provider-test")
            .to_string();
        let model = draft
            .payload
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("model-1")
            .to_string();
        let generation = draft
            .payload
            .get("providerConfigGeneration")
            .and_then(Value::as_str)
            .unwrap_or("test-provider-generation")
            .to_string();
        let evidence = synthetic_provider_policy_evidence_for_test(&request_id, &generation);
        append_provider_policy_evidence_payload(&mut draft.payload, &evidence)?;
        let started_at = if draft.event_type == "provider.started" {
            draft.created_at
        } else {
            let conn = self.lock_conn()?;
            select_event_by_immutable_identity(
                &conn,
                &draft.task_session_id,
                "provider.started",
                &request_id,
            )?
            .map(|event| event.created_at)
            .unwrap_or(draft.created_at)
        };
        draft.payload["startedAt"] = json!(started_at);
        if draft.event_type != "provider.started" {
            draft.payload["finishedAt"] = json!(draft.created_at);
        }
        let status = match draft.event_type.as_str() {
            "provider.completed" | "provider.started" => ProviderInvocationStatus::Completed,
            "provider.failed" => ProviderInvocationStatus::Failed,
            "provider.remote_unknown" => ProviderInvocationStatus::RemoteUnknown,
            _ => anyhow::bail!("unsupported synthetic provider lifecycle event"),
        };
        let error_digest = draft
            .payload
            .get("errorDigest")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                (status != ProviderInvocationStatus::Completed)
                    .then(|| format!("sha256:{}", "f".repeat(64)))
            });
        if draft.event_type != "provider.started" {
            draft.payload["errorDigest"] = json!(error_digest);
        }
        let receipt = ProviderInvocationReceipt {
            request_id,
            provider,
            model,
            status,
            started_at,
            finished_at: draft.created_at,
            error_digest,
            simulated: false,
            policy_evidence: Some(evidence),
        };
        let proof = ProviderInvocationDurabilityProof::synthetic_for_test(receipt)?;
        let scope = crate::main_chat_turn_runtime::MainChatProviderDurabilityScope::test_fixture(
            &draft.task_session_id,
            &draft.run_id,
        );
        let mut events = self.append_provider_lifecycle_for_test(vec![draft], &scope, &[proof])?;
        events
            .pop()
            .context("synthetic provider append returned no durable event")
    }

    #[cfg(test)]
    fn append_provider_lifecycle_for_test(
        &self,
        drafts: Vec<MainChatAgentEventDraft>,
        scope: &crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
        proofs: &[ProviderInvocationDurabilityProof],
    ) -> Result<Vec<MainChatAgentDurableEvent>> {
        self.append_batch_internal(
            drafts,
            ProviderLifecycleAdmission::SyntheticTest { scope, proofs },
        )
    }

    fn append_batch_with_provider_proofs(
        &self,
        drafts: Vec<MainChatAgentEventDraft>,
        scope: &crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
        proofs: &[ProviderInvocationDurabilityProof],
    ) -> Result<Vec<MainChatAgentDurableEvent>> {
        self.append_batch_internal(
            drafts,
            ProviderLifecycleAdmission::Runtime { scope, proofs },
        )
    }

    fn append_batch_internal(
        &self,
        drafts: Vec<MainChatAgentEventDraft>,
        provider_admission: ProviderLifecycleAdmission<'_>,
    ) -> Result<Vec<MainChatAgentDurableEvent>> {
        validate_provider_lifecycle_admission(&drafts, provider_admission)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut task_runs = std::collections::BTreeMap::<String, String>::new();
        for draft in &drafts {
            if event_scope_hidden(&tx, "task", &draft.task_session_id)?
                || event_scope_hidden(&tx, "run", &draft.run_id)?
            {
                anyhow::bail!("main_chat_event_canonical_source_tombstoned");
            }
            if task_runs
                .insert(draft.task_session_id.clone(), draft.run_id.clone())
                .is_some_and(|existing| existing != draft.run_id)
            {
                return Err(MainChatAgentEventStoreFault::TaskRunIdentityConflict.into());
            }
        }
        for (task_session_id, run_id) in task_runs {
            validate_task_sequence_domain(&tx, &task_session_id)?;
            validate_task_run_binding(&tx, &task_session_id)?;
            bind_task_run(&tx, &task_session_id, &run_id)?;
        }
        let events = drafts
            .into_iter()
            .map(|draft| append_event_in_transaction(&tx, draft, &self.digest_key))
            .collect::<Result<Vec<_>>>()?;
        enqueue_cancellation_projection_deliveries(&tx, &events)?;
        tx.commit()?;
        Ok(events)
    }

    fn append_batch(
        &self,
        drafts: Vec<MainChatAgentEventDraft>,
    ) -> Result<Vec<MainChatAgentDurableEvent>> {
        self.append_batch_internal(drafts, ProviderLifecycleAdmission::None)
    }

    pub(crate) fn list_cancellation_projection_deliveries(
        &self,
        task_session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MainChatCancellationProjectionDelivery>> {
        let conn = self.lock_conn()?;
        let mut statement = conn.prepare(
            "SELECT cancellation_id, task_session_id, run_id, terminal_event_id,
                    projection_target, state, attempts
             FROM main_chat_cancellation_projection_deliveries
             WHERE state IN ('pending', 'degraded')
               AND (?1 IS NULL OR task_session_id = ?1)
             ORDER BY updated_at ASC, cancellation_id ASC, projection_target ASC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![task_session_id, i64::try_from(limit.clamp(1, 250))?],
            |row| {
                let attempts: i64 = row.get(6)?;
                let attempts = u64::try_from(attempts).map_err(|_| {
                    row_decode_fault(
                        6,
                        Type::Integer,
                        "cancellation_projection_attempts",
                        "negative",
                    )
                })?;
                Ok(MainChatCancellationProjectionDelivery {
                    cancellation_id: row.get(0)?,
                    task_session_id: row.get(1)?,
                    run_id: row.get(2)?,
                    terminal_event_id: row.get(3)?,
                    projection_target: row.get(4)?,
                    state: row.get(5)?,
                    attempts,
                })
            },
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub(crate) fn mark_cancellation_projection_applied(
        &self,
        cancellation_id: &str,
        projection_target: &str,
    ) -> Result<()> {
        let changed = self.lock_conn()?.execute(
            "UPDATE main_chat_cancellation_projection_deliveries
             SET state = 'applied', attempts = attempts + 1,
                 error_digest = NULL, updated_at = ?3
             WHERE cancellation_id = ?1 AND projection_target = ?2",
            params![cancellation_id, projection_target, Utc::now().to_rfc3339()],
        )?;
        if changed != 1 {
            anyhow::bail!("cancellation_projection_delivery_missing");
        }
        Ok(())
    }

    pub(crate) fn mark_cancellation_projection_degraded(
        &self,
        cancellation_id: &str,
        projection_target: &str,
        error_digest: &str,
    ) -> Result<()> {
        let changed = self.lock_conn()?.execute(
            "UPDATE main_chat_cancellation_projection_deliveries
             SET state = 'degraded', attempts = attempts + 1,
                 error_digest = ?3, updated_at = ?4
             WHERE cancellation_id = ?1 AND projection_target = ?2",
            params![
                cancellation_id,
                projection_target,
                error_digest,
                Utc::now().to_rfc3339()
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("cancellation_projection_delivery_missing");
        }
        Ok(())
    }

    /// Isolate historical backfill projections that promoted AgentRun
    /// summaries into provider/context facts. The append-only rows remain for
    /// recovery, but product replay and point lookup no longer expose them.
    fn quarantine_run_derived_backfill_facts(
        &self,
        task_session_id: &str,
        run_id: Option<&str>,
    ) -> Result<usize> {
        if !is_bounded_event_reference(task_session_id, MAX_EVENT_IDENTITY_CHARS)
            || run_id
                .is_some_and(|run_id| !is_bounded_event_reference(run_id, MAX_EVENT_IDENTITY_CHARS))
        {
            anyhow::bail!("invalid_run_derived_backfill_quarantine_scope");
        }
        let mut conn = self.lock_conn()?;
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let candidates = {
            let mut statement = transaction.prepare(
                "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                        object_id, created_at, source, payload_digest, payload_json, backfilled,
                        payload_minimized_version
                 FROM main_chat_agent_events
                 WHERE task_session_id = ?1 AND (?2 IS NULL OR run_id = ?2) AND backfilled = 1
                   AND event_type IN ('provider.selected', 'context.selected')
                   AND NOT EXISTS (
                       SELECT 1 FROM main_chat_agent_event_fact_quarantine quarantine
                       WHERE quarantine.event_id = main_chat_agent_events.event_id
                   )
                 ORDER BY sequence ASC",
            )?;
            let rows = statement
                .query_map(params![task_session_id, run_id], row_to_event)?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let quarantined_at = Utc::now().to_rfc3339();
        let mut quarantined = 0usize;
        for event in candidates {
            let run_derived_provider = event.event_type == "provider.selected"
                && event.object_id == event.run_id
                && event.payload.get("evidenceId").and_then(Value::as_str)
                    == Some(event.run_id.as_str());
            let legacy_run_context = event.event_type == "context.selected"
                && event.payload.get("sourceKind").and_then(Value::as_str) == Some("run_context");
            if !run_derived_provider && !legacy_run_context {
                continue;
            }
            quarantined += transaction.execute(
                "INSERT INTO main_chat_agent_event_fact_quarantine (
                    event_id, task_session_id, run_id, reason_code, quarantined_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(event_id) DO NOTHING",
                params![
                    event.event_id,
                    task_session_id,
                    event.run_id,
                    if legacy_run_context {
                        "legacy_agent_run_context_unverified"
                    } else {
                        "agent_run_provider_projection_unverified"
                    },
                    quarantined_at,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(quarantined)
    }

    pub(crate) fn event_by_id(&self, event_id: &str) -> Result<Option<MainChatAgentDurableEvent>> {
        let scope = {
            let conn = self.lock_conn()?;
            conn.query_row(
                "SELECT task_session_id, run_id FROM main_chat_agent_events WHERE event_id = ?1",
                [event_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        };
        if let Some((task_session_id, run_id)) = scope {
            self.quarantine_run_derived_backfill_facts(&task_session_id, Some(&run_id))?;
        }
        let conn = self.lock_conn()?;
        let event = conn
            .query_row(
                "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
             FROM main_chat_agent_events
             WHERE event_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM main_chat_agent_event_fact_quarantine quarantine
                   WHERE quarantine.event_id = main_chat_agent_events.event_id
               )",
                [event_id],
                row_to_event,
            )
            .optional()?;
        if let Some(event) = event.as_ref() {
            validate_persisted_provider_lifecycles_for_task(&conn, &event.task_session_id)?;
        }
        Ok(event)
    }

    pub(crate) fn list(
        &self,
        task_session_id: &str,
        after_sequence: u64,
        limit: u64,
    ) -> Result<Vec<MainChatAgentDurableEvent>> {
        self.quarantine_run_derived_backfill_facts(task_session_id, None)?;
        let bounded_limit = limit.clamp(1, 250);
        let conn = self.lock_conn()?;
        if event_scope_hidden(&conn, "task", task_session_id)? {
            return Ok(Vec::new());
        }
        validate_task_sequence_domain(&conn, task_session_id)?;
        validate_task_run_binding(&conn, task_session_id)?;
        validate_persisted_provider_lifecycles_for_task(&conn, task_session_id)?;
        let after_sequence = i64::try_from(after_sequence)
            .map_err(|_| MainChatAgentEventStoreFault::SequenceExhausted)?;
        let mut stmt = conn.prepare(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
             FROM main_chat_agent_events
             WHERE task_session_id = ?1 AND sequence > ?2
               AND NOT EXISTS (
                   SELECT 1 FROM main_chat_agent_event_fact_quarantine quarantine
                   WHERE quarantine.event_id = main_chat_agent_events.event_id
               )
             ORDER BY sequence ASC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![task_session_id, after_sequence, bounded_limit as i64],
            row_to_event,
        )?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub(crate) fn get_immutable_event(
        &self,
        task_session_id: &str,
        event_type: &str,
        object_id: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        self.quarantine_run_derived_backfill_facts(task_session_id, None)?;
        let conn = self.lock_conn()?;
        if event_scope_hidden(&conn, "task", task_session_id)? {
            return Ok(None);
        }
        validate_task_sequence_domain(&conn, task_session_id)?;
        validate_task_run_binding(&conn, task_session_id)?;
        validate_persisted_provider_lifecycles_for_task(&conn, task_session_id)?;
        select_event_by_immutable_identity(&conn, task_session_id, event_type, object_id)
    }

    pub(crate) fn get_unique_tool_terminal_event(
        &self,
        task_session_id: &str,
        run_id: &str,
        receipt_id: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        self.quarantine_run_derived_backfill_facts(task_session_id, None)?;
        let conn = self.lock_conn()?;
        if event_scope_hidden(&conn, "task", task_session_id)?
            || event_scope_hidden(&conn, "run", run_id)?
        {
            return Ok(None);
        }
        validate_task_sequence_domain(&conn, task_session_id)?;
        validate_task_run_binding(&conn, task_session_id)?;
        validate_persisted_provider_lifecycles_for_task(&conn, task_session_id)?;
        let mut statement = conn.prepare(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
             FROM main_chat_agent_events
             WHERE task_session_id = ?1
               AND run_id = ?2
               AND object_type = 'tool_execution_receipt'
               AND object_id = ?3
               AND event_type IN (
                    'tool.completed', 'tool.failed', 'tool.effect_unknown',
                    'tool.local_aborted', 'tool.remote_unknown', 'tool.not_dispatched'
               )
               AND backfilled = 0
             ORDER BY sequence ASC
             LIMIT 2",
        )?;
        let mut events = statement
            .query_map(params![task_session_id, run_id, receipt_id], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        match events.len() {
            0 => Ok(None),
            1 => Ok(events.pop()),
            _ => anyhow::bail!("main_chat_tool_lifecycle_conflict:multiple_terminal_events"),
        }
    }

    /// Return the most recent durable adapter-edge provider fact across Main
    /// Chat tasks. This is intentionally metadata-only and read-only; callers
    /// must keep a failed or started-only cloud attempt `unknown` rather than
    /// inferring that a remote provider observed the request.
    pub(crate) fn latest_provider_event(&self) -> Result<Option<MainChatAgentDurableEvent>> {
        let conn = self.lock_conn()?;
        let event = conn
            .query_row(
                "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                        object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
                 FROM main_chat_agent_events
                 WHERE event_type IN ('provider.started', 'provider.completed', 'provider.failed', 'provider.remote_unknown')
                   AND backfilled = 0
                   AND NOT EXISTS (
                       SELECT 1 FROM main_chat_agent_event_tombstone_projections tombstone
                       WHERE tombstone.active = 1
                         AND ((tombstone.scope_kind = 'task'
                               AND tombstone.scope_id = main_chat_agent_events.task_session_id)
                           OR (tombstone.scope_kind = 'run'
                               AND tombstone.scope_id = main_chat_agent_events.run_id))
                   )
                 ORDER BY rowid DESC
                 LIMIT 1",
                [],
                row_to_event,
            )
            .optional()?;
        if let Some(event) = event.as_ref() {
            validate_task_sequence_domain(&conn, &event.task_session_id)?;
            validate_task_run_binding(&conn, &event.task_session_id)?;
            validate_persisted_provider_lifecycles_for_task(&conn, &event.task_session_id)?;
        }
        Ok(event)
    }

    pub(crate) fn latest_provider_event_for_run(
        &self,
        run_id: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        self.latest_provider_event_for_run_matching(run_id, None)
    }

    pub(crate) fn latest_completed_provider_event_for_run(
        &self,
        run_id: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        self.latest_provider_event_for_run_matching(run_id, Some("provider.completed"))
    }

    fn latest_provider_event_for_run_matching(
        &self,
        run_id: &str,
        exact_event_type: Option<&str>,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        let conn = self.lock_conn()?;
        let task_ids = {
            let mut statement = conn.prepare(
                "SELECT DISTINCT task_session_id
                 FROM main_chat_agent_events
                 WHERE run_id = ?1
                   AND event_type IN ('provider.started', 'provider.completed', 'provider.failed', 'provider.remote_unknown')
                   AND backfilled = 0",
            )?;
            let task_ids = statement
                .query_map([run_id], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            task_ids
        };
        for task_session_id in task_ids {
            validate_task_sequence_domain(&conn, &task_session_id)?;
            validate_task_run_binding(&conn, &task_session_id)?;
            validate_persisted_provider_lifecycles_for_task(&conn, &task_session_id)?;
        }
        let event = conn
            .query_row(
                "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                        object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
                 FROM main_chat_agent_events
                 WHERE run_id = ?1
                   AND event_type IN ('provider.started', 'provider.completed', 'provider.failed', 'provider.remote_unknown')
                   AND (?2 IS NULL OR event_type = ?2)
                   AND backfilled = 0
                   AND NOT EXISTS (
                       SELECT 1 FROM main_chat_agent_event_tombstone_projections tombstone
                       WHERE tombstone.active = 1
                         AND ((tombstone.scope_kind = 'task'
                               AND tombstone.scope_id = main_chat_agent_events.task_session_id)
                           OR (tombstone.scope_kind = 'run'
                               AND tombstone.scope_id = main_chat_agent_events.run_id))
                   )
                 ORDER BY sequence DESC, rowid DESC
                 LIMIT 1",
                params![run_id, exact_event_type],
                row_to_event,
            )
            .optional()?;
        if let Some(event) = event.as_ref() {
            validate_persisted_provider_lifecycles_for_task(&conn, &event.task_session_id)?;
        }
        Ok(event)
    }

    /// Return the latest durable turn lifecycle fact for one canonical task.
    /// `cancel_requested` is intentionally included: until `local_aborted` or a
    /// later terminal fact is durable, product projections must show pending
    /// cancellation rather than guessing a terminal state.
    pub(crate) fn latest_turn_lifecycle_event(
        &self,
        task_session_id: &str,
    ) -> Result<Option<MainChatAgentDurableEvent>> {
        Ok(self
            .turn_lifecycle_snapshot(task_session_id)?
            .lifecycle_event)
    }

    /// Read the event-stream sequence boundary and latest lifecycle receipt
    /// under one SQLite connection guard. Product projections compare two of
    /// these snapshots around their cross-store reads; a changed sequence
    /// means the assembled view was not a stable point-in-time observation.
    pub(crate) fn turn_lifecycle_snapshot(
        &self,
        task_session_id: &str,
    ) -> Result<MainChatTurnLifecycleSnapshot> {
        let conn = self.lock_conn()?;
        if event_scope_hidden(&conn, "task", task_session_id)? {
            return Ok(MainChatTurnLifecycleSnapshot {
                latest_sequence: 0,
                bound_run_id: None,
                lifecycle_event: None,
            });
        }
        validate_task_sequence_domain(&conn, task_session_id)?;
        validate_task_run_binding(&conn, task_session_id)?;
        validate_persisted_provider_lifecycles_for_task(&conn, task_session_id)?;
        let latest_sequence = conn
            .query_row(
                "SELECT sequence FROM main_chat_agent_events
                 WHERE task_session_id = ?1
                 ORDER BY sequence DESC
                 LIMIT 1",
                [task_session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        let latest_sequence = u64::try_from(latest_sequence).map_err(|_| {
            MainChatAgentEventStoreFault::CorruptRow {
                field: "sequence",
                reason: "negative",
            }
        })?;
        let bound_run_id = conn
            .query_row(
                "SELECT run_id FROM main_chat_agent_event_task_runs
                 WHERE task_session_id = ?1",
                [task_session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let lifecycle_event = conn
            .query_row(
                "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
             FROM main_chat_agent_events
             WHERE task_session_id = ?1
               AND backfilled = 0
               AND event_type IN (
                    'cancel_requested', 'local_aborted', 'interrupted', 'turn.interrupted',
                    'failed', 'final_delivery.created'
               )
             ORDER BY sequence DESC
             LIMIT 1",
                [task_session_id],
                row_to_event,
            )
            .optional()?;
        Ok(MainChatTurnLifecycleSnapshot {
            latest_sequence,
            bound_run_id,
            lifecycle_event,
        })
    }

    pub(crate) fn latest_run_id(&self, task_session_id: &str) -> Result<Option<String>> {
        self.quarantine_run_derived_backfill_facts(task_session_id, None)?;
        let conn = self.lock_conn()?;
        if event_scope_hidden(&conn, "task", task_session_id)? {
            return Ok(None);
        }
        validate_task_sequence_domain(&conn, task_session_id)?;
        validate_task_run_binding(&conn, task_session_id)?;
        validate_persisted_provider_lifecycles_for_task(&conn, task_session_id)?;
        let event = conn
            .query_row(
                "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                        object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
                 FROM main_chat_agent_events
                 WHERE task_session_id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM main_chat_agent_event_fact_quarantine quarantine
                       WHERE quarantine.event_id = main_chat_agent_events.event_id
                   )
                 ORDER BY sequence DESC
                 LIMIT 1",
                [task_session_id],
                row_to_event,
            )
            .optional()?;
        Ok(event.map(|event| event.run_id))
    }

    pub(crate) fn latest_sequence(&self, task_session_id: &str) -> Result<u64> {
        let conn = self.lock_conn()?;
        if event_scope_hidden(&conn, "task", task_session_id)? {
            return Ok(0);
        }
        validate_task_sequence_domain(&conn, task_session_id)?;
        validate_task_run_binding(&conn, task_session_id)?;
        validate_persisted_provider_lifecycles_for_task(&conn, task_session_id)?;
        let sequence = conn
            .query_row(
                "SELECT sequence FROM main_chat_agent_events
                 WHERE task_session_id = ?1
                 ORDER BY sequence DESC
                 LIMIT 1",
                [task_session_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        u64::try_from(sequence).map_err(|_| {
            MainChatAgentEventStoreFault::CorruptRow {
                field: "sequence",
                reason: "negative",
            }
            .into()
        })
    }

    /// Hide all durable event projections for tasks owned by a canonical
    /// conversation tombstone. Append-only event facts remain metadata-only
    /// and intact; product replay/read APIs become empty. Delivery is
    /// idempotent and never rewrites event sequence history.
    pub(crate) fn project_conversation_tombstone(
        &self,
        event_id: &str,
        tombstone_id: &str,
        task_ids: &[String],
    ) -> Result<usize> {
        self.project_event_visibility_tombstone(event_id, tombstone_id, "task", task_ids)
    }

    pub(crate) fn project_agent_run_canonical_head(
        &self,
        event_id: &str,
        canonical_revision: u64,
        run_id: &str,
        current_tombstone_id: Option<&str>,
        known_tombstone_ids: &[String],
    ) -> Result<usize> {
        if event_id.trim().is_empty()
            || run_id.trim().is_empty()
            || canonical_revision == 0
            || known_tombstone_ids
                .iter()
                .any(|tombstone_id| tombstone_id.trim().is_empty())
            || current_tombstone_id.is_some_and(|id| id.trim().is_empty())
            || current_tombstone_id
                .is_some_and(|id| !known_tombstone_ids.iter().any(|known| known == id))
        {
            anyhow::bail!("invalid AgentRun canonical projection head");
        }
        let revision = i64::try_from(canonical_revision)
            .context("AgentRun event projection revision overflow")?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current_head = tx
            .query_row(
                "SELECT canonical_revision, canonical_event_id, hidden,
                        canonical_tombstone_id
                 FROM main_chat_agent_run_projection_heads WHERE run_id = ?1",
                [run_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        if let Some((current_revision, current_event_id, hidden, tombstone_id)) = current_head {
            if current_revision > revision {
                anyhow::bail!(
                    "AgentRun event projection ahead of canonical source: target_revision={current_revision}, canonical_revision={revision}"
                );
            }
            if current_revision == revision
                && (current_event_id != event_id
                    || hidden != i64::from(current_tombstone_id.is_some())
                    || tombstone_id.as_deref() != current_tombstone_id)
            {
                anyhow::bail!("AgentRun event projection revision identity conflict");
            }
            if current_revision == revision {
                tx.commit()?;
                return Ok(0);
            }
        }
        let now = Utc::now().to_rfc3339();
        let mut changed = 0usize;
        for tombstone_id in known_tombstone_ids {
            changed += tx.execute(
                "INSERT INTO main_chat_agent_event_tombstone_projections (
                    canonical_tombstone_id, scope_kind, scope_id, active,
                    applied_event_id, applied_at
                 ) VALUES (?1, 'run', ?2, 0, ?3, ?4)
                 ON CONFLICT(canonical_tombstone_id, scope_kind, scope_id)
                 DO UPDATE SET active = 0,
                               applied_event_id = excluded.applied_event_id,
                               applied_at = excluded.applied_at",
                params![tombstone_id, run_id, event_id, now],
            )?;
        }
        if let Some(tombstone_id) = current_tombstone_id {
            changed += tx.execute(
                "UPDATE main_chat_agent_event_tombstone_projections
                 SET active = 1, applied_event_id = ?3, applied_at = ?4
                 WHERE canonical_tombstone_id = ?1
                   AND scope_kind = 'run' AND scope_id = ?2",
                params![tombstone_id, run_id, event_id, now],
            )?;
        }
        tx.execute(
            "INSERT INTO main_chat_agent_run_projection_heads (
                run_id, canonical_revision, canonical_event_id, hidden,
                canonical_tombstone_id, applied_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(run_id) DO UPDATE SET
                canonical_revision = excluded.canonical_revision,
                canonical_event_id = excluded.canonical_event_id,
                hidden = excluded.hidden,
                canonical_tombstone_id = excluded.canonical_tombstone_id,
                applied_at = excluded.applied_at
             WHERE excluded.canonical_revision >= canonical_revision",
            params![
                run_id,
                revision,
                event_id,
                i64::from(current_tombstone_id.is_some()),
                current_tombstone_id,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(changed)
    }

    #[cfg(test)]
    pub(crate) fn agent_run_projection_head_for_test(
        &self,
        run_id: &str,
    ) -> Result<Option<(u64, bool, Option<String>)>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT canonical_revision, hidden, canonical_tombstone_id
             FROM main_chat_agent_run_projection_heads WHERE run_id = ?1",
            [run_id],
            |row| {
                let revision: i64 = row.get(0)?;
                let revision = u64::try_from(revision).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(0, Type::Integer, Box::new(error))
                })?;
                Ok((revision, row.get::<_, i64>(1)? != 0, row.get(2)?))
            },
        )
        .optional()
        .map_err(Into::into)
    }

    fn project_event_visibility_tombstone(
        &self,
        event_id: &str,
        tombstone_id: &str,
        scope_kind: &str,
        scope_ids: &[String],
    ) -> Result<usize> {
        if event_id.trim().is_empty()
            || tombstone_id.trim().is_empty()
            || !matches!(scope_kind, "task" | "run")
        {
            anyhow::bail!("invalid event tombstone projection");
        }
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let mut applied = 0usize;
        for scope_id in scope_ids {
            if scope_id.trim().is_empty() {
                continue;
            }
            applied += tx.execute(
                "INSERT INTO main_chat_agent_event_tombstone_projections (
                    canonical_tombstone_id, scope_kind, scope_id, active,
                    applied_event_id, applied_at
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5)
                 ON CONFLICT(canonical_tombstone_id, scope_kind, scope_id)
                 DO NOTHING",
                params![tombstone_id, scope_kind, scope_id, event_id, now],
            )?;
        }
        tx.commit()?;
        Ok(applied)
    }

    #[cfg(test)]
    pub(crate) fn install_local_aborted_insert_failure_for_test(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch(
            "CREATE TRIGGER reject_local_aborted_event
             BEFORE INSERT ON main_chat_agent_events
             WHEN NEW.event_type = 'local_aborted'
             BEGIN
                 SELECT RAISE(ABORT, 'injected local_aborted event failure');
             END;",
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn install_failed_insert_failure_for_test(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch(
            "CREATE TRIGGER reject_failed_event
             BEFORE INSERT ON main_chat_agent_events
             WHEN NEW.event_type = 'failed'
             BEGIN
                 SELECT RAISE(ABORT, 'injected failed event failure');
             END;",
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn remove_failed_insert_failure_for_test(&self) -> Result<()> {
        self.lock_conn()?
            .execute_batch("DROP TRIGGER IF EXISTS reject_failed_event;")?;
        Ok(())
    }

    #[cfg(test)]
    fn disable_event_integrity_triggers_for_corruption_test(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        drop_event_integrity_triggers(&conn)
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|err| anyhow::anyhow!("mutex poison: {}", err))
    }
}

fn enqueue_cancellation_projection_deliveries(
    conn: &Connection,
    events: &[MainChatAgentDurableEvent],
) -> Result<()> {
    for terminal in events.iter().filter(|event| {
        matches!(event.event_type.as_str(), "local_aborted" | "interrupted")
            && event.object_type == "turn"
    }) {
        let Some(cancellation_id) = terminal
            .payload
            .get("cancellationId")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let cancel_request_present = events.iter().any(|event| {
            event.event_type == "cancel_requested"
                && event.object_type == "turn"
                && event.object_id == cancellation_id
                && event.task_session_id == terminal.task_session_id
                && event.run_id == terminal.run_id
                && event.payload.get("cancellationId").and_then(Value::as_str)
                    == Some(cancellation_id)
        });
        if !cancel_request_present {
            anyhow::bail!("cancellation_terminal_without_atomic_cancel_request");
        }
        for projection_target in ["agent_run", "task_session", "action_queue"] {
            conn.execute(
                "INSERT INTO main_chat_cancellation_projection_deliveries (
                    cancellation_id, task_session_id, run_id, terminal_event_id,
                    projection_target, state, attempts, error_digest, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 0, NULL, ?6)
                 ON CONFLICT(cancellation_id, projection_target) DO NOTHING",
                params![
                    cancellation_id,
                    terminal.task_session_id,
                    terminal.run_id,
                    terminal.event_id,
                    projection_target,
                    Utc::now().to_rfc3339(),
                ],
            )?;
        }
    }
    Ok(())
}

fn event_scope_hidden(conn: &Connection, scope_kind: &str, scope_id: &str) -> Result<bool> {
    if scope_kind == "task" {
        return Ok(conn
            .query_row(
                "SELECT 1
                 FROM main_chat_agent_event_tombstone_projections tombstone
                 WHERE tombstone.active = 1
                   AND ((tombstone.scope_kind = 'task' AND tombstone.scope_id = ?1)
                     OR (tombstone.scope_kind = 'run' AND tombstone.scope_id = (
                         SELECT run_id FROM main_chat_agent_event_task_runs
                         WHERE task_session_id = ?1
                     )))
                 LIMIT 1",
                [scope_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some());
    }
    Ok(conn
        .query_row(
            "SELECT 1 FROM main_chat_agent_event_tombstone_projections
             WHERE scope_kind = ?1 AND scope_id = ?2 AND active = 1 LIMIT 1",
            params![scope_kind, scope_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn ensure_event_payload_minimized_version_column(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("PRAGMA table_info(main_chat_agent_events)")?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !columns
        .iter()
        .any(|column| column == "payload_minimized_version")
    {
        conn.execute(
            "ALTER TABLE main_chat_agent_events
             ADD COLUMN payload_minimized_version INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

fn is_bounded_event_reference(value: &str, max_chars: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.chars().count() <= max_chars
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '-' | ':' | '/' | '+' | '@')
        })
}

fn is_typed_event_code(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= MAX_EVENT_TYPE_CHARS
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-' | ':')
        })
}

fn validate_event_draft_identity(draft: &MainChatAgentEventDraft) -> Result<()> {
    for (field, value) in [
        ("task_session_id", draft.task_session_id.as_str()),
        ("run_id", draft.run_id.as_str()),
        ("object_id", draft.object_id.as_str()),
    ] {
        if !is_bounded_event_reference(value, MAX_EVENT_IDENTITY_CHARS) {
            return Err(payload_schema_fault(
                &draft.event_type,
                &format!("invalid_identity:{field}"),
            ));
        }
    }
    for (field, value) in [
        ("event_type", draft.event_type.as_str()),
        ("object_type", draft.object_type.as_str()),
        ("source", draft.source.as_str()),
    ] {
        if !is_typed_event_code(value) {
            return Err(payload_schema_fault(
                &draft.event_type,
                &format!("invalid_typed_code:{field}"),
            ));
        }
    }
    Ok(())
}

fn migrate_legacy_event_identities(
    conn: &Connection,
    digest_key: &MainChatEventDigestKey,
) -> Result<()> {
    let current_version = conn
        .query_row(
            "SELECT value FROM main_chat_agent_event_store_metadata
             WHERE key = 'event_identity_version'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let mixed_or_bound_conflict = conn
        .query_row(
            "SELECT 1 FROM main_chat_agent_events
             GROUP BY task_session_id
             HAVING COUNT(DISTINCT run_id) != 1 OR MIN(run_id) = ''
             LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some()
        || conn
            .query_row(
                "SELECT 1
                 FROM main_chat_agent_events events
                 JOIN main_chat_agent_event_task_runs bindings
                   ON bindings.task_session_id = events.task_session_id
                 WHERE bindings.run_id != events.run_id LIMIT 1",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
    let immutable_conflict = conn
        .query_row(
            &format!(
                "SELECT 1 FROM main_chat_agent_events
                 WHERE event_type NOT IN ({})
                 GROUP BY task_session_id, event_type, object_id
                 HAVING COUNT(*) > 1 LIMIT 1",
                versioned_event_type_literals()
            ),
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if current_version == DURABLE_EVENT_IDENTITY_VERSION
        && !mixed_or_bound_conflict
        && !immutable_conflict
    {
        return Ok(());
    }
    if current_version < 0 || current_version > DURABLE_EVENT_IDENTITY_VERSION {
        return Err(MainChatAgentEventStoreFault::CorruptRow {
            field: "event_identity_version",
            reason: "unsupported",
        }
        .into());
    }

    let transaction = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    drop_event_integrity_triggers(&transaction)?;
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT event_id, task_session_id, run_id, sequence, event_type,
                    object_type, object_id, source, payload_digest
             FROM main_chat_agent_events
             ORDER BY task_session_id ASC, sequence ASC",
        )?;
        let mapped = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?;
        mapped.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let mut invalid_tasks = std::collections::BTreeMap::<String, (String, Vec<String>)>::new();
    let mut task_runs =
        std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    let mut task_row_receipts = std::collections::BTreeMap::<String, Vec<String>>::new();
    let mut immutable_identities =
        std::collections::BTreeMap::<(String, String, String), usize>::new();
    for (
        event_id,
        task_id,
        run_id,
        sequence,
        event_type,
        object_type,
        object_id,
        source,
        payload_digest,
    ) in &rows
    {
        let encoded_identity = serde_json::to_vec(&(
            event_id,
            task_id,
            run_id,
            sequence,
            event_type,
            object_type,
            object_id,
            source,
            payload_digest,
        ))?;
        let row_identity_receipt =
            hmac_sha256_digest(digest_key, "quarantine_row_identity", &encoded_identity);
        task_row_receipts
            .entry(task_id.clone())
            .or_default()
            .push(row_identity_receipt.clone());
        task_runs
            .entry(task_id.clone())
            .or_default()
            .insert(run_id.clone());
        if !VERSIONED_EVENT_TYPES.contains(&event_type.as_str()) {
            *immutable_identities
                .entry((task_id.clone(), event_type.clone(), object_id.clone()))
                .or_default() += 1;
        }
        let valid = *sequence > 0
            && is_bounded_event_reference(task_id, MAX_EVENT_IDENTITY_CHARS)
            && is_bounded_event_reference(run_id, MAX_EVENT_IDENTITY_CHARS)
            && is_bounded_event_reference(object_id, MAX_EVENT_IDENTITY_CHARS)
            && is_typed_event_code(event_type)
            && is_typed_event_code(object_type)
            && is_typed_event_code(source);
        if !valid {
            invalid_tasks
                .entry(task_id.clone())
                .or_insert_with(|| ("legacy_identity_invalid".into(), Vec::new()))
                .1
                .push(row_identity_receipt);
        }
    }
    for (task_id, runs) in task_runs.iter().filter(|(_, runs)| runs.len() != 1) {
        let entry = invalid_tasks
            .entry(task_id.clone())
            .or_insert_with(|| ("mixed_run_ownership".into(), Vec::new()));
        entry.0 = "mixed_run_ownership".into();
        entry.1.push(format!("task_run_count={}", runs.len()));
    }
    for ((task_id, event_type, object_id), count) in
        immutable_identities.iter().filter(|(_, count)| **count > 1)
    {
        let entry = invalid_tasks
            .entry(task_id.clone())
            .or_insert_with(|| ("immutable_identity_conflict".into(), Vec::new()));
        entry.0 = "immutable_identity_conflict".into();
        entry
            .1
            .push(format!("{event_type}\0{object_id}\0count={count}"));
    }
    let binding_conflicts = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT events.task_session_id
             FROM main_chat_agent_events events
             JOIN main_chat_agent_event_task_runs bindings
               ON bindings.task_session_id = events.task_session_id
             WHERE bindings.run_id != events.run_id",
        )?;
        let conflicts = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        conflicts
    };
    for task_id in binding_conflicts {
        invalid_tasks
            .entry(task_id)
            .or_insert_with(|| ("task_run_binding_conflict".into(), Vec::new()))
            .1
            .push("binding_conflict".into());
    }

    for (task_id, (reason_code, invalid_rows)) in &invalid_tasks {
        let task_runs = {
            let mut statement = transaction.prepare(
                "SELECT DISTINCT run_id FROM main_chat_agent_events
                 WHERE task_session_id = ?1",
            )?;
            let mapped = statement.query_map([task_id], |row| row.get::<_, String>(0))?;
            mapped.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let event_count = transaction.query_row(
            "SELECT COUNT(*) FROM main_chat_agent_events WHERE task_session_id = ?1",
            [task_id],
            |row| row.get::<_, i64>(0),
        )?;
        let task_identity_digest =
            hmac_sha256_digest(digest_key, "quarantine_task_identity", task_id.as_bytes());
        let mut row_receipts = task_row_receipts.get(task_id).cloned().unwrap_or_default();
        row_receipts.sort();
        let mut failure_receipts = invalid_rows.clone();
        failure_receipts.sort();
        let row_set_material = serde_json::to_vec(&(
            "main_chat_event_quarantine_rows_v2",
            reason_code,
            event_count,
            row_receipts,
            failure_receipts,
        ))?;
        let row_set_digest =
            hmac_sha256_digest(digest_key, "quarantine_row_set", &row_set_material);
        let quarantine_id = format!(
            "mainchat_event_quarantine:v2:{}",
            row_set_digest.trim_start_matches("hmac-sha256:")
        );
        transaction.execute(
            "INSERT OR IGNORE INTO main_chat_agent_event_identity_quarantine (
                quarantine_id, task_identity_digest, row_set_digest, event_count,
                reason_code, quarantined_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                quarantine_id,
                task_identity_digest,
                row_set_digest,
                event_count,
                reason_code,
                Utc::now().to_rfc3339(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM main_chat_agent_event_immutable_identities
             WHERE task_session_id = ?1",
            [task_id],
        )?;
        transaction.execute(
            "DELETE FROM main_chat_cancellation_projection_deliveries
             WHERE task_session_id = ?1",
            [task_id],
        )?;
        transaction.execute(
            "DELETE FROM main_chat_agent_event_tombstone_projections
             WHERE scope_kind = 'task' AND scope_id = ?1",
            [task_id],
        )?;
        for run_id in task_runs {
            transaction.execute(
                "DELETE FROM main_chat_agent_event_tombstone_projections
                 WHERE scope_kind = 'run' AND scope_id = ?1",
                [&run_id],
            )?;
            transaction.execute(
                "DELETE FROM main_chat_agent_run_projection_heads WHERE run_id = ?1",
                [&run_id],
            )?;
        }
        transaction.execute(
            "DELETE FROM main_chat_agent_events WHERE task_session_id = ?1",
            [task_id],
        )?;
        transaction.execute(
            "DELETE FROM main_chat_agent_event_sequences WHERE task_session_id = ?1",
            [task_id],
        )?;
        transaction.execute(
            "DELETE FROM main_chat_agent_event_task_runs WHERE task_session_id = ?1",
            [task_id],
        )?;
    }

    transaction.execute("DELETE FROM main_chat_agent_event_immutable_identities", [])?;
    let remaining = {
        let mut statement = transaction.prepare(
            "SELECT event_id, task_session_id, sequence, event_type, object_id
             FROM main_chat_agent_events ORDER BY task_session_id ASC, sequence ASC",
        )?;
        let mapped = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        mapped.collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut mappings = Vec::with_capacity(remaining.len());
    let mut new_ids = std::collections::HashSet::with_capacity(remaining.len());
    for (old_id, task_id, sequence_raw, event_type, object_id) in remaining {
        let sequence =
            u64::try_from(sequence_raw).map_err(|_| MainChatAgentEventStoreFault::CorruptRow {
                field: "sequence",
                reason: "identity_migration_invalid",
            })?;
        let new_id = stable_event_id(&task_id, sequence, &event_type, &object_id);
        if !new_ids.insert(new_id.clone()) {
            return Err(MainChatAgentEventStoreFault::CorruptRow {
                field: "event_id",
                reason: "identity_migration_collision",
            }
            .into());
        }
        mappings.push((old_id, new_id));
    }
    for (index, (old_id, new_id)) in mappings.iter().enumerate() {
        if old_id == new_id {
            continue;
        }
        let temporary_id = format!("mainchat_event:migrating:{index}:{}", &new_id[25..]);
        transaction.execute(
            "UPDATE main_chat_agent_events SET event_id = ?1 WHERE event_id = ?2",
            params![temporary_id, old_id],
        )?;
        for (table, column) in [
            (
                "main_chat_agent_event_tombstone_projections",
                "applied_event_id",
            ),
            ("main_chat_agent_run_projection_heads", "canonical_event_id"),
            (
                "main_chat_cancellation_projection_deliveries",
                "terminal_event_id",
            ),
        ] {
            transaction.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE {column} = ?2"),
                params![temporary_id, old_id],
            )?;
        }
    }
    for (index, (old_id, new_id)) in mappings.iter().enumerate() {
        if old_id == new_id {
            continue;
        }
        let temporary_id = format!("mainchat_event:migrating:{index}:{}", &new_id[25..]);
        transaction.execute(
            "UPDATE main_chat_agent_events SET event_id = ?1 WHERE event_id = ?2",
            params![new_id, temporary_id],
        )?;
        for (table, column) in [
            (
                "main_chat_agent_event_tombstone_projections",
                "applied_event_id",
            ),
            ("main_chat_agent_run_projection_heads", "canonical_event_id"),
            (
                "main_chat_cancellation_projection_deliveries",
                "terminal_event_id",
            ),
        ] {
            transaction.execute(
                &format!("UPDATE {table} SET {column} = ?1 WHERE {column} = ?2"),
                params![new_id, temporary_id],
            )?;
        }
    }
    transaction.execute(
        "INSERT INTO main_chat_agent_event_store_metadata(key, value)
         VALUES ('event_identity_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [DURABLE_EVENT_IDENTITY_VERSION],
    )?;
    validate_immutable_identity_domain(&transaction)?;
    validate_and_backfill_immutable_identity_registry(&transaction)?;
    install_event_integrity_triggers(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn validate_current_event_identity_domain(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare(
        "SELECT event_id, task_session_id, run_id, sequence, event_type,
                object_type, object_id, source
         FROM main_chat_agent_events",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let event_id = row.get::<_, String>(0)?;
        let task_session_id = row.get::<_, String>(1)?;
        let run_id = row.get::<_, String>(2)?;
        let sequence_raw = row.get::<_, i64>(3)?;
        let event_type = row.get::<_, String>(4)?;
        let object_type = row.get::<_, String>(5)?;
        let object_id = row.get::<_, String>(6)?;
        let source = row.get::<_, String>(7)?;
        let sequence =
            u64::try_from(sequence_raw).map_err(|_| MainChatAgentEventStoreFault::CorruptRow {
                field: "sequence",
                reason: "identity_invalid",
            })?;
        if sequence == 0
            || !is_bounded_event_reference(&task_session_id, MAX_EVENT_IDENTITY_CHARS)
            || !is_bounded_event_reference(&run_id, MAX_EVENT_IDENTITY_CHARS)
            || !is_bounded_event_reference(&object_id, MAX_EVENT_IDENTITY_CHARS)
            || !is_typed_event_code(&event_type)
            || !is_typed_event_code(&object_type)
            || !is_typed_event_code(&source)
            || stable_event_id(&task_session_id, sequence, &event_type, &object_id) != event_id
        {
            return Err(MainChatAgentEventStoreFault::CorruptRow {
                field: "event_id",
                reason: "identity_invalid",
            }
            .into());
        }
    }
    Ok(())
}

fn migrate_legacy_event_payloads(
    conn: &Connection,
    digest_key: &MainChatEventDigestKey,
) -> Result<()> {
    let transaction = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    let missing_policy_tasks = collect_provider_tasks_missing_policy_evidence(&transaction)?;
    let pre_quarantined = if missing_policy_tasks.is_empty() {
        0
    } else {
        drop_event_integrity_triggers(&transaction)?;
        quarantine_unverified_provider_lifecycle_tasks(
            &transaction,
            digest_key,
            missing_policy_tasks,
        )?
    };
    let rows = {
        let mut statement = transaction.prepare(
            "SELECT event_id, sequence, event_type, object_type, payload_json,
                    payload_minimized_version
             FROM main_chat_agent_events
             WHERE payload_minimized_version < ?1",
        )?;
        let rows = statement
            .query_map([DURABLE_EVENT_PAYLOAD_VERSION], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    let had_legacy_rows = !rows.is_empty();
    if had_legacy_rows {
        drop_event_integrity_triggers(&transaction)?;
        transaction.execute("DELETE FROM main_chat_agent_event_immutable_identities", [])?;
    }
    for (event_id, sequence_raw, event_type, object_type, payload_json, payload_version) in rows {
        if !(0..DURABLE_EVENT_PAYLOAD_VERSION).contains(&payload_version) {
            return Err(MainChatAgentEventStoreFault::CorruptRow {
                field: "payload_minimized_version",
                reason: "legacy_migration_invalid",
            }
            .into());
        }
        let sequence =
            u64::try_from(sequence_raw).map_err(|_| MainChatAgentEventStoreFault::CorruptRow {
                field: "sequence",
                reason: "legacy_migration_invalid",
            })?;
        if sequence == 0 {
            return Err(MainChatAgentEventStoreFault::CorruptRow {
                field: "sequence",
                reason: "legacy_migration_invalid",
            }
            .into());
        }
        let payload = serde_json::from_str::<Value>(&payload_json).map_err(|_| {
            MainChatAgentEventStoreFault::CorruptRow {
                field: "payload_json",
                reason: "legacy_invalid_json",
            }
        })?;
        let minimized = normalize_durable_event_payload_with_key(
            &event_type,
            &object_type,
            &payload,
            PayloadNormalizationOrigin::Legacy,
            digest_key,
        )?;
        let minimized_json = serde_json::to_string(&minimized)?;
        let payload_digest = stored_event_payload_digest(&event_type, sequence, &minimized_json);
        transaction.execute(
            "UPDATE main_chat_agent_events
             SET payload_json = ?1, payload_digest = ?2, payload_minimized_version = ?3
             WHERE event_id = ?4",
            params![
                minimized_json,
                payload_digest,
                DURABLE_EVENT_PAYLOAD_VERSION,
                event_id
            ],
        )?;
    }
    let invalid_provider_tasks = collect_unverified_provider_lifecycle_tasks(&transaction)?;
    if !had_legacy_rows && !invalid_provider_tasks.is_empty() {
        drop_event_integrity_triggers(&transaction)?;
    }
    let quarantined = quarantine_unverified_provider_lifecycle_tasks(
        &transaction,
        digest_key,
        invalid_provider_tasks,
    )?;
    if had_legacy_rows || pre_quarantined > 0 || quarantined > 0 {
        validate_immutable_identity_domain(&transaction)?;
        validate_and_backfill_immutable_identity_registry(&transaction)?;
        install_event_integrity_triggers(&transaction)?;
    }
    transaction.commit()?;
    Ok(())
}

fn collect_provider_tasks_missing_policy_evidence(
    conn: &Connection,
) -> Result<std::collections::BTreeMap<String, Vec<String>>> {
    const REQUIRED_POLICY_FIELDS: [&str; 17] = [
        "providerConfigGeneration",
        "policyDecisionId",
        "policyVersion",
        "policyAuthority",
        "effectiveDataRoute",
        "effectiveLocalRestriction",
        "subjectScopeDigest",
        "payloadPurpose",
        "unfilteredPayloadDigest",
        "contextManifestDigest",
        "preparedEnvelopeDigest",
        "networkPolicyDecisionDigest",
        "selectedContextRefs",
        "includedContextCategories",
        "declaredPayloadCategories",
        "policyProvenanceRefs",
        "policyEvidenceDigest",
    ];
    let mut statement = conn.prepare(
        "SELECT task_session_id, object_id, payload_json
         FROM main_chat_agent_events
         WHERE event_type IN (
             'provider.started', 'provider.completed', 'provider.failed',
             'provider.remote_unknown'
         )",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut invalid = std::collections::BTreeMap::<String, Vec<String>>::new();
    for row in rows {
        let (task_session_id, object_id, payload_json) = row?;
        let payload = serde_json::from_str::<Value>(&payload_json).ok();
        let object = payload.as_ref().and_then(Value::as_object);
        let missing = object.is_none_or(|object| {
            REQUIRED_POLICY_FIELDS
                .iter()
                .any(|field| !object.contains_key(*field))
                || object.get("rawLifeModelIncluded").and_then(Value::as_bool) != Some(false)
                || object
                    .get("rawUnboundedMemoryIncluded")
                    .and_then(Value::as_bool)
                    != Some(false)
        });
        if missing {
            invalid
                .entry(task_session_id)
                .or_default()
                .push(format!("policy_provenance_missing:{object_id}"));
        }
    }
    Ok(invalid)
}

fn collect_unverified_provider_lifecycle_tasks(
    conn: &Connection,
) -> Result<std::collections::BTreeMap<String, Vec<String>>> {
    let events = {
        let mut statement = conn.prepare(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
             FROM main_chat_agent_events
             WHERE event_type IN (
                'provider.started', 'provider.completed', 'provider.failed',
                'provider.remote_unknown'
             )
             ORDER BY task_session_id ASC, object_id ASC, sequence ASC",
        )?;
        let rows = statement
            .query_map([], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows
    };
    let mut groups =
        std::collections::BTreeMap::<(String, String), Vec<MainChatAgentDurableEvent>>::new();
    for event in events {
        groups
            .entry((event.task_session_id.clone(), event.object_id.clone()))
            .or_default()
            .push(event);
    }

    let mut invalid = std::collections::BTreeMap::<String, Vec<String>>::new();
    for ((task_session_id, object_id), events) in groups {
        let starts = events
            .iter()
            .filter(|event| event.event_type == "provider.started")
            .collect::<Vec<_>>();
        let terminals = events
            .iter()
            .filter(|event| {
                matches!(
                    event.event_type.as_str(),
                    "provider.completed" | "provider.failed" | "provider.remote_unknown"
                )
            })
            .collect::<Vec<_>>();
        let request_identity_valid = events.iter().all(|event| {
            event.payload.get("requestId").and_then(Value::as_str) == Some(object_id.as_str())
        });
        let provider_shape_valid = events
            .iter()
            .all(|event| validate_persisted_provider_event_shape(event).is_ok());
        let violation = if !provider_shape_valid {
            Some("policy_provenance_invalid")
        } else if starts.len() > 1 {
            Some("duplicate_start")
        } else if terminals.len() > 1 {
            Some("conflicting_terminal")
        } else if !request_identity_valid {
            Some("request_identity_mismatch")
        } else if terminals.is_empty() {
            None
        } else if starts.is_empty() {
            Some("terminal_without_start")
        } else {
            let start = starts[0];
            let terminal = terminals[0];
            let provider_identity_matches = [
                "provider",
                "model",
                "providerConfigGeneration",
                "policyEvidenceDigest",
            ]
            .into_iter()
            .all(|field| start.payload.get(field) == terminal.payload.get(field))
                && PROVIDER_POLICY_IDENTITY_FIELDS
                    .iter()
                    .all(|field| start.payload.get(*field) == terminal.payload.get(*field));
            if start.run_id != terminal.run_id {
                Some("run_identity_mismatch")
            } else if !provider_identity_matches {
                Some("adapter_identity_mismatch")
            } else {
                None
            }
        };
        if let Some(violation) = violation {
            invalid
                .entry(task_session_id)
                .or_default()
                .push(format!("{violation}:{object_id}"));
        }
    }
    Ok(invalid)
}

fn quarantine_unverified_provider_lifecycle_tasks(
    conn: &Connection,
    digest_key: &MainChatEventDigestKey,
    invalid_tasks: std::collections::BTreeMap<String, Vec<String>>,
) -> Result<usize> {
    let quarantined_count = invalid_tasks.len();
    for (task_session_id, mut violations) in invalid_tasks {
        let task_runs = {
            let mut statement = conn.prepare(
                "SELECT DISTINCT run_id FROM main_chat_agent_events
                 WHERE task_session_id = ?1 ORDER BY run_id ASC",
            )?;
            let rows = statement
                .query_map([&task_session_id], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let row_receipts = {
            let mut statement = conn.prepare(
                "SELECT sequence, event_type, object_type, object_id, source, payload_digest
                 FROM main_chat_agent_events
                 WHERE task_session_id = ?1 ORDER BY sequence ASC",
            )?;
            let rows = statement
                .query_map([&task_session_id], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        let event_count = i64::try_from(row_receipts.len())?;
        violations.sort();
        let row_set_material = serde_json::to_vec(&(
            "main_chat_provider_lifecycle_quarantine_v1",
            &row_receipts,
            &violations,
        ))?;
        let task_identity_digest = hmac_sha256_digest(
            digest_key,
            "quarantine_task_identity",
            task_session_id.as_bytes(),
        );
        let row_set_digest = hmac_sha256_digest(
            digest_key,
            "quarantine_provider_lifecycle_row_set",
            &row_set_material,
        );
        let quarantine_id = format!(
            "mainchat_event_quarantine:provider_lifecycle:v1:{}",
            row_set_digest.trim_start_matches("hmac-sha256:")
        );
        conn.execute(
            "INSERT OR IGNORE INTO main_chat_agent_event_identity_quarantine (
                quarantine_id, task_identity_digest, row_set_digest, event_count,
                reason_code, quarantined_at
             ) VALUES (?1, ?2, ?3, ?4, 'legacy_provider_lifecycle_unverified', ?5)",
            params![
                quarantine_id,
                task_identity_digest,
                row_set_digest,
                event_count,
                Utc::now().to_rfc3339(),
            ],
        )?;
        conn.execute(
            "DELETE FROM main_chat_agent_event_immutable_identities
             WHERE task_session_id = ?1",
            [&task_session_id],
        )?;
        conn.execute(
            "DELETE FROM main_chat_cancellation_projection_deliveries
             WHERE task_session_id = ?1",
            [&task_session_id],
        )?;
        conn.execute(
            "DELETE FROM main_chat_agent_event_tombstone_projections
             WHERE scope_kind = 'task' AND scope_id = ?1",
            [&task_session_id],
        )?;
        for run_id in task_runs {
            conn.execute(
                "DELETE FROM main_chat_agent_event_tombstone_projections
                 WHERE scope_kind = 'run' AND scope_id = ?1",
                [&run_id],
            )?;
            conn.execute(
                "DELETE FROM main_chat_agent_run_projection_heads WHERE run_id = ?1",
                [&run_id],
            )?;
        }
        conn.execute(
            "DELETE FROM main_chat_agent_events WHERE task_session_id = ?1",
            [&task_session_id],
        )?;
        conn.execute(
            "DELETE FROM main_chat_agent_event_sequences WHERE task_session_id = ?1",
            [&task_session_id],
        )?;
        conn.execute(
            "DELETE FROM main_chat_agent_event_task_runs WHERE task_session_id = ?1",
            [&task_session_id],
        )?;
    }
    Ok(quarantined_count)
}

fn validate_supported_payload_versions(conn: &Connection) -> Result<()> {
    let unsupported = conn
        .query_row(
            "SELECT 1 FROM main_chat_agent_events
             WHERE payload_minimized_version != ?1
             LIMIT 1",
            [DURABLE_EVENT_PAYLOAD_VERSION],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if unsupported.is_some() {
        return Err(MainChatAgentEventStoreFault::CorruptRow {
            field: "payload_minimized_version",
            reason: "unsupported",
        }
        .into());
    }
    Ok(())
}

const VERSIONED_EVENT_TYPES: [&str; 9] = [
    "task.updated",
    "plan.updated",
    "step.updated",
    "action.updated",
    "action.started",
    "action.completed",
    "action.failed",
    "proposal.updated",
    "memory.materialized",
];

const IMMUTABLE_IDENTITY_INDEX: &str = "idx_main_chat_agent_events_immutable_identity_v1";
const IMMUTABLE_INSERT_GUARD: &str = "guard_main_chat_immutable_event_insert_v1";
const IMMUTABLE_IDENTITY_RECORDER: &str = "record_main_chat_immutable_event_identity_v1";
const EVENT_UPDATE_GUARD: &str = "guard_main_chat_event_update_v1";
const EVENT_DELETE_GUARD: &str = "guard_main_chat_event_delete_v1";
const IDENTITY_INSERT_GUARD: &str = "guard_main_chat_event_identity_insert_v1";
const IDENTITY_UPDATE_GUARD: &str = "guard_main_chat_event_identity_update_v1";
const IDENTITY_DELETE_GUARD: &str = "guard_main_chat_event_identity_delete_v1";

fn validate_immutable_identity_domain(conn: &Connection) -> Result<()> {
    let placeholders = std::iter::repeat("?")
        .take(VERSIONED_EVENT_TYPES.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT event_type, MIN(payload_digest), MAX(payload_digest), COUNT(*)
         FROM main_chat_agent_events
         WHERE event_type NOT IN ({placeholders})
         GROUP BY task_session_id, event_type, object_id
         HAVING COUNT(*) > 1
         LIMIT 1"
    );
    let conflict = conn
        .query_row(
            &sql,
            rusqlite::params_from_iter(VERSIONED_EVENT_TYPES),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((event_type, first_digest, last_digest, _count)) = conflict else {
        return Ok(());
    };
    if first_digest == last_digest {
        return Err(MainChatAgentEventStoreFault::DuplicateImmutableIdentity { event_type }.into());
    }
    Err(MainChatAgentEventStoreFault::ImmutableIdentityConflict {
        event_type,
        existing_payload_digest: first_digest,
        incoming_payload_digest: last_digest,
    }
    .into())
}

fn ensure_immutable_identity_constraint(conn: &Connection) -> Result<()> {
    let transaction = rusqlite::Transaction::new_unchecked(conn, TransactionBehavior::Immediate)?;
    validate_immutable_identity_domain(&transaction)?;
    validate_and_backfill_immutable_identity_registry(&transaction)?;
    drop_event_integrity_triggers(&transaction)?;
    transaction.execute(
        &format!("DROP INDEX IF EXISTS {IMMUTABLE_IDENTITY_INDEX}"),
        [],
    )?;
    transaction
        .execute(
            &format!(
                "CREATE UNIQUE INDEX IF NOT EXISTS {IMMUTABLE_IDENTITY_INDEX}
             ON main_chat_agent_events(task_session_id, event_type, object_id)
             WHERE event_type NOT IN ({})",
                versioned_event_type_literals()
            ),
            [],
        )
        .context("failed to install immutable main chat event identity constraint")?;
    install_event_integrity_triggers(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn versioned_event_type_literals() -> String {
    VERSIONED_EVENT_TYPES
        .iter()
        .map(|event_type| format!("'{}'", event_type.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ")
}

fn validate_and_backfill_immutable_identity_registry(conn: &Connection) -> Result<()> {
    let versioned_event_types = versioned_event_type_literals();
    let stale_registry = conn
        .query_row(
            &format!(
                "SELECT 1
                 FROM main_chat_agent_event_immutable_identities identities
                 LEFT JOIN main_chat_agent_events events
                   ON events.task_session_id = identities.task_session_id
                  AND events.event_type = identities.event_type
                  AND events.object_id = identities.object_id
                  AND events.payload_digest = identities.payload_digest
                  AND events.event_id = identities.event_id
                 WHERE identities.event_type IN ({versioned_event_types})
                    OR events.event_id IS NULL
                 LIMIT 1"
            ),
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if stale_registry.is_some() {
        return Err(MainChatAgentEventStoreFault::ImmutableIdentityRegistryConflict.into());
    }
    conn.execute(
        &format!(
            "INSERT INTO main_chat_agent_event_immutable_identities (
                task_session_id, event_type, object_id, payload_digest, event_id
             )
             SELECT events.task_session_id, events.event_type, events.object_id,
                    events.payload_digest, events.event_id
             FROM main_chat_agent_events events
             WHERE events.event_type NOT IN ({versioned_event_types})
               AND NOT EXISTS (
                   SELECT 1
                   FROM main_chat_agent_event_immutable_identities identities
                   WHERE identities.task_session_id = events.task_session_id
                     AND identities.event_type = events.event_type
                     AND identities.object_id = events.object_id
               )"
        ),
        [],
    )?;
    let missing_registry = conn
        .query_row(
            &format!(
                "SELECT 1
                 FROM main_chat_agent_events events
                 LEFT JOIN main_chat_agent_event_immutable_identities identities
                   ON identities.task_session_id = events.task_session_id
                  AND identities.event_type = events.event_type
                  AND identities.object_id = events.object_id
                  AND identities.payload_digest = events.payload_digest
                  AND identities.event_id = events.event_id
                 WHERE events.event_type NOT IN ({versioned_event_types})
                   AND identities.event_id IS NULL
                 LIMIT 1"
            ),
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if missing_registry.is_some() {
        return Err(MainChatAgentEventStoreFault::ImmutableIdentityRegistryConflict.into());
    }
    Ok(())
}

fn drop_event_integrity_triggers(conn: &Connection) -> Result<()> {
    for trigger in [
        IMMUTABLE_INSERT_GUARD,
        IMMUTABLE_IDENTITY_RECORDER,
        EVENT_UPDATE_GUARD,
        EVENT_DELETE_GUARD,
        IDENTITY_INSERT_GUARD,
        IDENTITY_UPDATE_GUARD,
        IDENTITY_DELETE_GUARD,
    ] {
        conn.execute(&format!("DROP TRIGGER IF EXISTS {trigger}"), [])?;
    }
    Ok(())
}

fn install_event_integrity_triggers(conn: &Connection) -> Result<()> {
    let versioned_event_types = versioned_event_type_literals();
    conn.execute_batch(&format!(
        "CREATE TRIGGER {IMMUTABLE_INSERT_GUARD}
         BEFORE INSERT ON main_chat_agent_events
         WHEN NEW.event_type NOT IN ({versioned_event_types})
          AND EXISTS (
              SELECT 1
              FROM main_chat_agent_event_immutable_identities identities
              WHERE identities.task_session_id = NEW.task_session_id
                AND identities.event_type = NEW.event_type
                AND identities.object_id = NEW.object_id
          )
         BEGIN
             SELECT RAISE(ABORT, 'main_chat_agent_event_immutable_identity_exists');
         END;

         CREATE TRIGGER {IMMUTABLE_IDENTITY_RECORDER}
         AFTER INSERT ON main_chat_agent_events
         WHEN NEW.event_type NOT IN ({versioned_event_types})
         BEGIN
             INSERT INTO main_chat_agent_event_immutable_identities (
                 task_session_id, event_type, object_id, payload_digest, event_id
             ) VALUES (
                 NEW.task_session_id, NEW.event_type, NEW.object_id,
                 NEW.payload_digest, NEW.event_id
             );
         END;

         CREATE TRIGGER {EVENT_UPDATE_GUARD}
         BEFORE UPDATE ON main_chat_agent_events
         BEGIN
             SELECT RAISE(ABORT, 'main_chat_agent_events_append_only:update');
         END;

         CREATE TRIGGER {EVENT_DELETE_GUARD}
         BEFORE DELETE ON main_chat_agent_events
         BEGIN
             SELECT RAISE(ABORT, 'main_chat_agent_events_append_only:delete');
         END;

         CREATE TRIGGER {IDENTITY_INSERT_GUARD}
         BEFORE INSERT ON main_chat_agent_event_immutable_identities
         WHEN EXISTS (
             SELECT 1
             FROM main_chat_agent_event_immutable_identities identities
             WHERE identities.task_session_id = NEW.task_session_id
               AND identities.event_type = NEW.event_type
               AND identities.object_id = NEW.object_id
         )
         BEGIN
             SELECT RAISE(ABORT, 'main_chat_agent_event_identity_append_only:insert');
         END;

         CREATE TRIGGER {IDENTITY_UPDATE_GUARD}
         BEFORE UPDATE ON main_chat_agent_event_immutable_identities
         BEGIN
             SELECT RAISE(ABORT, 'main_chat_agent_event_identity_append_only:update');
         END;

         CREATE TRIGGER {IDENTITY_DELETE_GUARD}
         BEFORE DELETE ON main_chat_agent_event_immutable_identities
         BEGIN
             SELECT RAISE(ABORT, 'main_chat_agent_event_identity_append_only:delete');
         END;"
    ))?;
    Ok(())
}

fn validate_and_backfill_task_run_bindings(conn: &Connection) -> Result<()> {
    let mixed_run_task = conn
        .query_row(
            "SELECT 1
             FROM main_chat_agent_events
             GROUP BY task_session_id
             HAVING COUNT(DISTINCT run_id) != 1 OR MIN(run_id) = ''
             LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if mixed_run_task.is_some() {
        return Err(MainChatAgentEventStoreFault::TaskRunIdentityConflict.into());
    }
    let existing_binding_conflict = conn
        .query_row(
            "SELECT 1
             FROM main_chat_agent_events events
             JOIN main_chat_agent_event_task_runs bindings
               ON bindings.task_session_id = events.task_session_id
             WHERE bindings.run_id != events.run_id
             LIMIT 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if existing_binding_conflict.is_some() {
        return Err(MainChatAgentEventStoreFault::TaskRunIdentityConflict.into());
    }
    conn.execute(
        "INSERT INTO main_chat_agent_event_task_runs(task_session_id, run_id)
         SELECT task_session_id, MIN(run_id)
         FROM main_chat_agent_events
         GROUP BY task_session_id
         ON CONFLICT(task_session_id) DO NOTHING",
        [],
    )?;
    Ok(())
}

fn bind_task_run(conn: &Connection, task_session_id: &str, run_id: &str) -> Result<()> {
    if run_id.trim().is_empty() {
        return Err(MainChatAgentEventStoreFault::TaskRunIdentityConflict.into());
    }
    conn.execute(
        "INSERT INTO main_chat_agent_event_task_runs(task_session_id, run_id)
         VALUES (?1, ?2)
         ON CONFLICT(task_session_id) DO NOTHING",
        params![task_session_id, run_id],
    )?;
    let bound_run_id = conn.query_row(
        "SELECT run_id FROM main_chat_agent_event_task_runs WHERE task_session_id = ?1",
        [task_session_id],
        |row| row.get::<_, String>(0),
    )?;
    if bound_run_id != run_id {
        return Err(MainChatAgentEventStoreFault::TaskRunIdentityConflict.into());
    }
    Ok(())
}

fn validate_task_run_binding(conn: &Connection, task_session_id: &str) -> Result<()> {
    let event_run_domain = conn.query_row(
        "SELECT MIN(run_id), MAX(run_id), COUNT(*)
             FROM main_chat_agent_events
             WHERE task_session_id = ?1",
        [task_session_id],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    let binding = conn
        .query_row(
            "SELECT run_id FROM main_chat_agent_event_task_runs WHERE task_session_id = ?1",
            [task_session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match event_run_domain {
        (None, None, 0) if binding.is_none() => Ok(()),
        (Some(minimum), Some(maximum), count)
            if count > 0
                && minimum == maximum
                && !minimum.is_empty()
                && binding.as_deref() == Some(minimum.as_str()) =>
        {
            Ok(())
        }
        _ => Err(MainChatAgentEventStoreFault::TaskRunIdentityConflict.into()),
    }
}

fn validate_all_task_sequence_domains(conn: &Connection) -> Result<()> {
    let task_ids = {
        let mut statement = conn.prepare(
            "SELECT task_session_id FROM main_chat_agent_events
             UNION
             SELECT task_session_id FROM main_chat_agent_event_sequences
             UNION
             SELECT task_session_id FROM main_chat_agent_event_task_runs",
        )?;
        let task_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        task_ids
    };
    for task_session_id in task_ids {
        validate_task_sequence_domain(conn, &task_session_id)?;
        validate_task_run_binding(conn, &task_session_id)?;
    }
    Ok(())
}

fn is_provider_lifecycle_draft(draft: &MainChatAgentEventDraft) -> bool {
    matches!(
        draft.event_type.as_str(),
        "provider.started" | "provider.completed" | "provider.failed" | "provider.remote_unknown"
    )
}

fn provider_terminal_event_type(status: ProviderInvocationStatus) -> &'static str {
    match status {
        ProviderInvocationStatus::Completed => "provider.completed",
        ProviderInvocationStatus::Failed => "provider.failed",
        ProviderInvocationStatus::RemoteUnknown => "provider.remote_unknown",
    }
}

fn provider_policy_payload_matches_proof(
    draft: &MainChatAgentEventDraft,
    proof: &ProviderInvocationDurabilityProof,
) -> Result<()> {
    let mut expected = json!({
        "requestId": proof.request_id(),
        "provider": proof.provider(),
        "model": proof.model(),
    });
    append_provider_policy_evidence_payload(&mut expected, proof.policy_evidence())?;
    let expected = expected
        .as_object()
        .context("provider durability expected payload is not an object")?;
    let observed = draft
        .payload
        .as_object()
        .context("provider lifecycle payload is not an object")?;
    for (field, expected_value) in expected {
        if observed.get(field) != Some(expected_value) {
            anyhow::bail!(
                "provider_lifecycle_policy_evidence_mismatch:{}:{}",
                draft.object_id,
                field
            );
        }
    }
    Ok(())
}

fn validate_provider_lifecycle_admission(
    drafts: &[MainChatAgentEventDraft],
    admission: ProviderLifecycleAdmission<'_>,
) -> Result<()> {
    let lifecycle_drafts = drafts
        .iter()
        .filter(|draft| is_provider_lifecycle_draft(draft))
        .collect::<Vec<_>>();
    if lifecycle_drafts.is_empty() {
        return Ok(());
    }
    let (scope, proofs, synthetic_test) = match admission {
        ProviderLifecycleAdmission::None => {
            anyhow::bail!("provider_lifecycle_runtime_admission_missing")
        }
        ProviderLifecycleAdmission::Runtime { scope, proofs } => (scope, proofs, false),
        #[cfg(test)]
        ProviderLifecycleAdmission::SyntheticTest { scope, proofs } => (scope, proofs, true),
    };
    if lifecycle_drafts
        .iter()
        .any(|draft| !scope.validates(&draft.task_session_id, &draft.run_id))
    {
        anyhow::bail!("provider_lifecycle_task_run_scope_mismatch");
    }
    let mut proof_by_request = std::collections::BTreeMap::new();
    for proof in proofs {
        if proof_by_request.insert(proof.request_id(), proof).is_some() {
            anyhow::bail!("provider_lifecycle_duplicate_durability_proof");
        }
    }
    let observed_request_ids = lifecycle_drafts
        .iter()
        .map(|draft| draft.object_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    if observed_request_ids
        != proof_by_request
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
    {
        anyhow::bail!("provider_lifecycle_durability_proof_scope_mismatch");
    }

    for draft in lifecycle_drafts {
        let proof = proof_by_request
            .get(draft.object_id.as_str())
            .copied()
            .context("provider lifecycle durability proof missing")?;
        proof.policy_evidence().validate_minimal_truth()?;
        provider_policy_payload_matches_proof(draft, proof)?;
        let lifecycle_evidence_digest = draft
            .payload
            .get("policyEvidenceDigest")
            .and_then(Value::as_str)
            .context("provider lifecycle evidence digest missing")?;
        if lifecycle_evidence_digest != proof.lifecycle_evidence_digest() {
            anyhow::bail!(
                "provider_lifecycle_evidence_digest_mismatch:{}",
                draft.object_id
            );
        }

        if synthetic_test {
            #[cfg(test)]
            {
                if !proof.is_synthetic_test_fixture() {
                    anyhow::bail!("provider lifecycle test admission used a runtime proof");
                }
            }
        } else {
            proof.validate_runtime_start(
                &draft.object_id,
                draft
                    .payload
                    .get("provider")
                    .and_then(Value::as_str)
                    .context("provider lifecycle provider missing")?,
                draft
                    .payload
                    .get("model")
                    .and_then(Value::as_str)
                    .context("provider lifecycle model missing")?,
                proof.started_at(),
                proof.policy_evidence(),
                lifecycle_evidence_digest,
            )?;
        }

        if draft.event_type == "provider.started" {
            if draft.source != "provider_adapter" || draft.created_at != proof.started_at() {
                anyhow::bail!(
                    "provider_lifecycle_start_proof_mismatch:{}",
                    draft.object_id
                );
            }
            continue;
        }

        if draft.source == "provider_adapter" {
            let terminal_receipt = proof
                .terminal_receipt()
                .context("provider lifecycle terminal proof missing")?;
            if synthetic_test {
                #[cfg(test)]
                proof.validate_synthetic_test_receipt(terminal_receipt)?;
            } else {
                proof.validate_runtime_adapter_terminal(terminal_receipt)?;
            }
            if provider_terminal_event_type(terminal_receipt.status) != draft.event_type
                || terminal_receipt.finished_at != draft.created_at
                || draft.payload.get("startedAt") != Some(&json!(terminal_receipt.started_at))
                || draft.payload.get("finishedAt") != Some(&json!(terminal_receipt.finished_at))
                || draft.payload.get("errorDigest") != Some(&json!(terminal_receipt.error_digest))
            {
                anyhow::bail!(
                    "provider_lifecycle_terminal_proof_mismatch:{}",
                    draft.object_id
                );
            }
        } else if draft.event_type != "provider.remote_unknown"
            || draft.source != "openlife_turn_runtime"
        {
            anyhow::bail!(
                "provider_lifecycle_terminal_authority_invalid:{}",
                draft.object_id
            );
        } else if proof.terminal_receipt().is_some() {
            anyhow::bail!(
                "provider_lifecycle_runtime_unknown_conflicts_with_adapter_terminal:{}",
                draft.object_id
            );
        } else if draft.payload.get("startedAt") != Some(&json!(proof.started_at())) {
            anyhow::bail!(
                "provider_lifecycle_runtime_unknown_start_mismatch:{}",
                draft.object_id
            );
        }
    }
    Ok(())
}

fn enqueue_tool_queue_reconciliation_projection(
    transaction: &Transaction<'_>,
    prepared: &MainChatAgentDurableEvent,
    resolution: &MainChatAgentDurableEvent,
    disposition: MainChatToolQueueReconciliationDisposition,
    created_at: DateTime<Utc>,
    digest_key: &MainChatEventDigestKey,
) -> Result<()> {
    let replay_action_id = prepared
        .payload
        .get("replayActionId")
        .and_then(Value::as_str);
    let replay_claim_id = prepared
        .payload
        .get("replayClaimId")
        .and_then(Value::as_str);
    let replay_claim_owner_generation = prepared
        .payload
        .get("replayClaimOwnerGeneration")
        .and_then(Value::as_u64);
    let replay_authority_binding = prepared
        .payload
        .get("replayAuthorityBinding")
        .and_then(Value::as_str);
    let (
        Some(replay_action_id),
        Some(replay_claim_id),
        Some(replay_claim_owner_generation),
        Some(replay_authority_binding),
    ) = (
        replay_action_id,
        replay_claim_id,
        replay_claim_owner_generation,
        replay_authority_binding,
    )
    else {
        if replay_action_id.is_some()
            || replay_claim_id.is_some()
            || replay_claim_owner_generation.is_some()
            || replay_authority_binding.is_some()
        {
            anyhow::bail!("tool_reconciliation_replay_identity_incomplete");
        }
        return Ok(());
    };
    if !is_bounded_event_reference(replay_action_id, 384)
        || uuid::Uuid::parse_str(replay_claim_id).is_err()
        || replay_claim_owner_generation == 0
        || !is_bounded_event_reference(replay_authority_binding, 384)
    {
        anyhow::bail!("tool_reconciliation_replay_identity_invalid");
    }
    let outbox_id = format!(
        "tool_queue_reconciliation:v2:{}",
        openlife_core::persistence_outbox::metadata_digest(&prepared.event_id)
    );
    if resolution.task_session_id != prepared.task_session_id
        || resolution.run_id != prepared.run_id
        || resolution.object_id != prepared.object_id
        || !matches!(
            (disposition, resolution.event_type.as_str()),
            (
                MainChatToolQueueReconciliationDisposition::EffectNotAttempted,
                "tool.not_dispatched"
            ) | (
                MainChatToolQueueReconciliationDisposition::EffectNotAttempted,
                "tool.dispatch_ambiguous"
            ) | (
                MainChatToolQueueReconciliationDisposition::DispatchedUnknown,
                "tool.dispatch_ambiguous"
            )
        )
    {
        anyhow::bail!("tool_reconciliation_resolution_identity_invalid");
    }
    let payload_string = |key: &str| -> Result<&str> {
        prepared
            .payload
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_prepared_{key}_missing"))
    };
    let action_effect: ToolActionEffect = serde_json::from_value(
        prepared
            .payload
            .get("actionEffect")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_action_effect_missing"))?,
    )?;
    let idempotency_contract = serde_json::from_value(
        prepared
            .payload
            .get("idempotencyContract")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_idempotency_missing"))?,
    )?;
    let process_risk = match prepared
        .payload
        .get("dispatchProcessRisk")
        .and_then(Value::as_str)
    {
        Some("process_bound") => {
            openlife_core::agent::action_executor::ToolDispatchProcessRisk::ProcessBound
        }
        Some("may_outlive_local_process") => {
            openlife_core::agent::action_executor::ToolDispatchProcessRisk::MayOutliveLocalProcess
        }
        _ => anyhow::bail!("tool_reconciliation_process_risk_invalid"),
    };
    let core_disposition = match disposition {
        MainChatToolQueueReconciliationDisposition::EffectNotAttempted => openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationDisposition::EffectNotAttempted,
        MainChatToolQueueReconciliationDisposition::DispatchedUnknown => openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationDisposition::DispatchedUnknown,
    };
    let envelope =
        openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolReconciliationEnvelope {
            outbox_id: &outbox_id,
            prepared_event_id: &prepared.event_id,
            prepared_payload_digest: &prepared.payload_digest,
            resolution_event_id: &resolution.event_id,
            resolution_payload_digest: &resolution.payload_digest,
            resolution: core_tool_reconciliation_resolution(&resolution.event_type)?,
            task_session_id: &prepared.task_session_id,
            run_id: &prepared.run_id,
            receipt_id: &prepared.object_id,
            action_id: replay_action_id,
            replay_claim_id,
            replay_claim_owner_generation,
            manifest_id: payload_string("manifestId")?,
            tool_name: payload_string("toolName")?,
            manifest_contract_digest: payload_string("manifestContractDigest")?,
            input_hash: payload_string("inputHash")?,
            input_length_bytes: prepared
                .payload
                .get("inputLengthBytes")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_input_length_invalid"))?,
            request_digest: payload_string("requestDigest")?,
            action_effect,
            idempotency_contract,
            process_risk,
            effect_may_survive_local_process: prepared
                .payload
                .get("effectMaySurviveLocalProcess")
                .and_then(Value::as_bool)
                .ok_or_else(|| anyhow::anyhow!("tool_reconciliation_effect_process_invalid"))?,
            replay_authority_binding,
            disposition: core_disposition,
        };
    let event_store_attestation = digest_key.sign_reconciliation_attestation(&envelope)?;
    transaction.execute(
        "INSERT INTO main_chat_tool_queue_reconciliation_outbox (
            outbox_id, prepared_event_id, prepared_payload_digest,
            resolution_event_id, resolution_payload_digest,
            task_session_id, run_id, receipt_id, replay_action_id, replay_claim_id,
            replay_claim_owner_generation, replay_authority_binding,
            event_store_attestation, disposition, state, created_at, applied_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 'pending', ?15, NULL)
         ON CONFLICT(prepared_event_id) DO NOTHING",
        params![
            outbox_id,
            prepared.event_id,
            prepared.payload_digest,
            resolution.event_id,
            resolution.payload_digest,
            prepared.task_session_id,
            prepared.run_id,
            prepared.object_id,
            replay_action_id,
            replay_claim_id,
            i64::try_from(replay_claim_owner_generation)?,
            replay_authority_binding,
            event_store_attestation,
            disposition.as_str(),
            created_at.to_rfc3339(),
        ],
    )?;
    let exact = transaction
        .query_row(
            "SELECT 1 FROM main_chat_tool_queue_reconciliation_outbox
             WHERE outbox_id = ?1
               AND prepared_event_id = ?2
               AND prepared_payload_digest = ?3
               AND resolution_event_id = ?4
               AND resolution_payload_digest = ?5
               AND task_session_id = ?6
               AND run_id = ?7
               AND receipt_id = ?8
               AND replay_action_id = ?9
               AND replay_claim_id = ?10
               AND replay_claim_owner_generation = ?11
               AND replay_authority_binding = ?12
               AND event_store_attestation = ?13
               AND disposition = ?14",
            params![
                outbox_id,
                prepared.event_id,
                prepared.payload_digest,
                resolution.event_id,
                resolution.payload_digest,
                prepared.task_session_id,
                prepared.run_id,
                prepared.object_id,
                replay_action_id,
                replay_claim_id,
                i64::try_from(replay_claim_owner_generation)?,
                replay_authority_binding,
                event_store_attestation,
                disposition.as_str(),
            ],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exact {
        anyhow::bail!("tool_reconciliation_outbox_identity_conflict");
    }
    Ok(())
}

fn append_event_in_transaction(
    conn: &Connection,
    draft: MainChatAgentEventDraft,
    digest_key: &MainChatEventDigestKey,
) -> Result<MainChatAgentDurableEvent> {
    append_event_in_transaction_with_tool_admission(
        conn,
        draft,
        digest_key,
        ToolLifecycleAdmission::None,
    )
}

fn append_event_in_transaction_with_tool_admission(
    conn: &Connection,
    mut draft: MainChatAgentEventDraft,
    digest_key: &MainChatEventDigestKey,
    tool_admission: ToolLifecycleAdmission<'_>,
) -> Result<MainChatAgentDurableEvent> {
    validate_event_draft_identity(&draft)?;
    draft.payload = normalize_durable_event_payload_with_key(
        &draft.event_type,
        &draft.object_type,
        &draft.payload,
        PayloadNormalizationOrigin::New,
        digest_key,
    )?;
    validate_cancel_requested_transition(&draft)?;
    validate_provider_lifecycle_transition(conn, &draft)?;
    validate_tool_lifecycle_transition(conn, &draft, tool_admission)?;
    let payload_json = serde_json::to_string(&draft.payload)?;
    let content_digest = metadata_safe_digest(&payload_json);

    if is_immutable_event_type(&draft.event_type) {
        if let Some(existing) = select_event_by_exact_fact(
            conn,
            &draft.task_session_id,
            &draft.event_type,
            &draft.object_id,
            &content_digest,
        )? {
            return Ok(existing);
        }
        if let Some(existing) = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            &draft.event_type,
            &draft.object_id,
        )? {
            return Err(MainChatAgentEventStoreFault::ImmutableIdentityConflict {
                event_type: draft.event_type,
                existing_payload_digest: existing.payload_digest,
                incoming_payload_digest: content_digest,
            }
            .into());
        }
    } else if let Some(existing) = select_latest_event_by_identity(
        conn,
        &draft.task_session_id,
        &draft.event_type,
        &draft.object_id,
    )? {
        let existing_payload_json = serde_json::to_string(&existing.payload)?;
        if metadata_safe_digest(&existing_payload_json) == content_digest {
            return Ok(existing);
        }
    }

    let last_sequence_raw = conn
        .query_row(
            "SELECT last_sequence FROM main_chat_agent_event_sequences WHERE task_session_id = ?1",
            [&draft.task_session_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let last_sequence =
        u64::try_from(last_sequence_raw).map_err(|_| MainChatAgentEventStoreFault::CorruptRow {
            field: "last_sequence",
            reason: "negative",
        })?;
    let sequence = last_sequence
        .checked_add(1)
        .ok_or(MainChatAgentEventStoreFault::SequenceExhausted)?;
    let sequence_sql =
        i64::try_from(sequence).map_err(|_| MainChatAgentEventStoreFault::SequenceExhausted)?;
    let payload_digest = stored_event_payload_digest(&draft.event_type, sequence, &payload_json);
    let event_id = stable_event_id(
        &draft.task_session_id,
        sequence,
        &draft.event_type,
        &draft.object_id,
    );
    conn.execute(
        "INSERT INTO main_chat_agent_events (
            event_id, task_session_id, run_id, sequence, event_type, object_type,
            object_id, created_at, source, payload_digest, payload_json,
            payload_minimized_version, backfilled
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            event_id,
            draft.task_session_id,
            draft.run_id,
            sequence_sql,
            draft.event_type,
            draft.object_type,
            draft.object_id,
            draft.created_at.to_rfc3339(),
            draft.source,
            payload_digest,
            payload_json,
            DURABLE_EVENT_PAYLOAD_VERSION,
            if draft.backfilled { 1 } else { 0 },
        ],
    )?;
    conn.execute(
        "INSERT INTO main_chat_agent_event_sequences(task_session_id, last_sequence)
         VALUES (?1, ?2)
         ON CONFLICT(task_session_id) DO UPDATE SET last_sequence = excluded.last_sequence",
        params![draft.task_session_id, sequence_sql],
    )?;
    select_event_by_id(conn, &event_id)?.context("inserted event missing")
}

fn validate_cancel_requested_transition(draft: &MainChatAgentEventDraft) -> Result<()> {
    if draft.event_type != "cancel_requested" || draft.source != "openlife_turn_runtime" {
        return Ok(());
    }
    if draft
        .payload
        .get("localWaitAborted")
        .and_then(Value::as_bool)
        != Some(true)
        || draft
            .payload
            .get("remoteCancellationConfirmed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        anyhow::bail!("main_chat_cancel_requested_transport_truth_invalid");
    }
    Ok(())
}

fn validate_provider_lifecycle_transition(
    conn: &Connection,
    draft: &MainChatAgentEventDraft,
) -> Result<()> {
    const TERMINAL_TYPES: [&str; 3] = [
        "provider.completed",
        "provider.failed",
        "provider.remote_unknown",
    ];
    let is_start = draft.event_type == "provider.started";
    let is_terminal = TERMINAL_TYPES.contains(&draft.event_type.as_str());
    if !is_start && !is_terminal {
        return Ok(());
    }
    if draft.object_type != "provider_request" {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("invalid_object_type:{}", draft.event_type),
        }
        .into());
    }
    let payload_request_id = draft
        .payload
        .get("requestId")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("request_id_missing:{}", draft.event_type),
        })?;
    let provider = draft
        .payload
        .get("provider")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("provider_missing:{}", draft.object_id),
        })?;
    let model = draft
        .payload
        .get("model")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("model_missing:{}", draft.object_id),
        })?;
    let provider_config_generation = draft
        .payload
        .get("providerConfigGeneration")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("provider_config_generation_missing:{}", draft.object_id),
        })?;
    let policy_evidence_digest = draft
        .payload
        .get("policyEvidenceDigest")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("policy_evidence_digest_missing:{}", draft.object_id),
        })?;
    if payload_request_id != draft.object_id {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("request_id_mismatch:{}", draft.object_id),
        }
        .into());
    }

    if is_start {
        if draft.source != "provider_adapter" {
            return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
                reason: format!("start_source_unverified:{}", draft.object_id),
            }
            .into());
        }
        if let Some(existing_start) = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "provider.started",
            &draft.object_id,
        )? {
            let same_identity = existing_start.run_id == draft.run_id
                && existing_start.created_at == draft.created_at
                && existing_start
                    .payload
                    .get("provider")
                    .and_then(Value::as_str)
                    == Some(provider)
                && existing_start.payload.get("model").and_then(Value::as_str) == Some(model)
                && existing_start
                    .payload
                    .get("providerConfigGeneration")
                    .and_then(Value::as_str)
                    == Some(provider_config_generation)
                && existing_start
                    .payload
                    .get("policyEvidenceDigest")
                    .and_then(Value::as_str)
                    == Some(policy_evidence_digest);
            if same_identity {
                return Ok(());
            }
            return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
                reason: format!("start_identity_conflict:{}", draft.object_id),
            }
            .into());
        }
    }

    let mut existing_terminals = Vec::new();
    for event_type in TERMINAL_TYPES {
        if let Some(event) = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            event_type,
            &draft.object_id,
        )? {
            existing_terminals.push((event_type, event));
        }
    }
    if is_start {
        if !existing_terminals.is_empty() {
            return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
                reason: format!("start_after_terminal:{}", draft.object_id),
            }
            .into());
        }
        return Ok(());
    }

    let start = select_event_by_immutable_identity(
        conn,
        &draft.task_session_id,
        "provider.started",
        &draft.object_id,
    )?
    .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
        reason: format!("terminal_without_start:{}", draft.object_id),
    })?;
    if start.run_id != draft.run_id {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("run_identity_mismatch:{}", draft.object_id),
        }
        .into());
    }
    let start_provider = start.payload.get("provider").and_then(Value::as_str);
    let start_model = start.payload.get("model").and_then(Value::as_str);
    let start_provider_config_generation = start
        .payload
        .get("providerConfigGeneration")
        .and_then(Value::as_str);
    let start_policy_evidence_digest = start
        .payload
        .get("policyEvidenceDigest")
        .and_then(Value::as_str);
    if start_provider != Some(provider)
        || start_model != Some(model)
        || start_provider_config_generation != Some(provider_config_generation)
        || start_policy_evidence_digest != Some(policy_evidence_digest)
        || PROVIDER_POLICY_IDENTITY_FIELDS
            .iter()
            .any(|field| start.payload.get(*field) != draft.payload.get(*field))
    {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("adapter_identity_mismatch:{}", draft.object_id),
        }
        .into());
    }
    let terminal_candidate = MainChatAgentDurableEvent {
        event_id: String::new(),
        task_session_id: draft.task_session_id.clone(),
        run_id: draft.run_id.clone(),
        sequence: 0,
        event_type: draft.event_type.clone(),
        object_type: draft.object_type.clone(),
        object_id: draft.object_id.clone(),
        created_at: draft.created_at,
        source: draft.source.clone(),
        payload_digest: String::new(),
        payload: draft.payload.clone(),
        backfilled: draft.backfilled,
    };
    if persisted_provider_timestamp(&terminal_candidate, "startedAt")? != Some(start.created_at) {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("terminal_start_observation_mismatch:{}", draft.object_id),
        }
        .into());
    }
    let adapter_terminal = start.source == "provider_adapter" && draft.source == "provider_adapter";
    let runtime_cancel_terminal = start.source == "provider_adapter"
        && provider_remote_unknown_has_runtime_cancel_contract(&terminal_candidate);
    let runtime_kernel_failure_terminal = start.source == "provider_adapter"
        && provider_remote_unknown_has_runtime_kernel_failure_contract(&terminal_candidate);
    if !adapter_terminal && !runtime_cancel_terminal && !runtime_kernel_failure_terminal {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("source_mismatch:{}", draft.object_id),
        }
        .into());
    }
    if runtime_cancel_terminal {
        let cancellation_id = draft
            .payload
            .get("cancellationId")
            .and_then(Value::as_str)
            .expect("verified runtime cancellation draft has an id");
        let cancellation = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "cancel_requested",
            cancellation_id,
        )?
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("runtime_cancel_event_missing:{}", draft.object_id),
        })?;
        if cancellation.run_id != draft.run_id
            || cancellation.object_type != "turn"
            || cancellation.source != "openlife_turn_runtime"
            || cancellation
                .payload
                .get("cancellationId")
                .and_then(Value::as_str)
                != Some(cancellation_id)
        {
            return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
                reason: format!("runtime_cancel_event_mismatch:{}", draft.object_id),
            }
            .into());
        }
    }
    if runtime_kernel_failure_terminal {
        let failure_receipt_id = draft
            .payload
            .get("kernelFailureReceiptId")
            .and_then(Value::as_str)
            .expect("verified runtime kernel-failure draft has a receipt id");
        let failure = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "failed",
            failure_receipt_id,
        )?
        .ok_or_else(|| MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("runtime_kernel_failure_event_missing:{}", draft.object_id),
        })?;
        if failure.run_id != draft.run_id
            || failure.object_type != "turn"
            || failure.source != "openlife_turn_runtime.kernel_error_pre_commit"
            || failure.created_at != draft.created_at
            || failure.payload.get("status").and_then(Value::as_str) != Some("failed")
            || failure.payload.get("kind").and_then(Value::as_str) != Some("unknown_error")
            || failure
                .payload
                .get("durableCommitAllowedAfterFailure")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
                reason: format!("runtime_kernel_failure_event_mismatch:{}", draft.object_id),
            }
            .into());
        }
    }
    if existing_terminals
        .iter()
        .any(|(event_type, _)| *event_type != draft.event_type)
    {
        return Err(MainChatAgentEventStoreFault::ProviderLifecycleConflict {
            reason: format!("conflicting_terminal:{}", draft.object_id),
        }
        .into());
    }
    Ok(())
}

fn validate_tool_lifecycle_transition(
    conn: &Connection,
    draft: &MainChatAgentEventDraft,
    admission: ToolLifecycleAdmission<'_>,
) -> Result<()> {
    const TERMINAL_TYPES: [&str; 6] = [
        "tool.not_dispatched",
        "tool.completed",
        "tool.failed",
        "tool.effect_unknown",
        "tool.local_aborted",
        "tool.remote_unknown",
    ];
    let is_prepared = draft.event_type == "tool.dispatch_prepared";
    let is_ambiguous = draft.event_type == "tool.dispatch_ambiguous";
    let is_start = draft.event_type == "tool.started";
    let is_not_dispatched = draft.event_type == "tool.not_dispatched";
    let is_terminal = TERMINAL_TYPES.contains(&draft.event_type.as_str());
    if !is_prepared && !is_ambiguous && !is_start && !is_terminal {
        return Ok(());
    }
    if draft.object_type != "tool_execution_receipt" {
        return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("invalid_object_type:{}", draft.event_type),
        }
        .into());
    }
    if !draft.payload.is_object() {
        return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("payload_not_object:{}", draft.object_id),
        }
        .into());
    }
    if [
        "cancellationId",
        "remoteCancellationConfirmed",
        "localWaitAborted",
    ]
    .into_iter()
    .any(|field| draft.payload.get(field).is_some())
    {
        return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("cancellation_context_is_not_tool_fact:{}", draft.object_id),
        }
        .into());
    }
    let receipt_id = draft
        .payload
        .get("receiptId")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("receipt_id_missing:{}", draft.event_type),
        })?;
    let source_run_id = draft
        .payload
        .get("sourceRunId")
        .and_then(Value::as_str)
        .ok_or_else(|| MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("source_run_id_missing:{}", draft.object_id),
        })?;
    if receipt_id != draft.object_id || source_run_id != draft.run_id {
        return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("receipt_identity_mismatch:{}", draft.object_id),
        }
        .into());
    }
    if is_prepared {
        let process_risk = draft
            .payload
            .get("dispatchProcessRisk")
            .and_then(Value::as_str);
        let may_outlive = draft
            .payload
            .get("mayOutliveLocalProcess")
            .and_then(Value::as_bool);
        let process_contract_is_exact = matches!(
            (process_risk, may_outlive),
            (Some("process_bound"), Some(false)) | (Some("may_outlive_local_process"), Some(true))
        );
        let effect_may_survive = draft
            .payload
            .get("effectMaySurviveLocalProcess")
            .and_then(Value::as_bool);
        let action_effect = draft.payload.get("actionEffect").and_then(Value::as_str);
        let effect_contract_is_exact = match action_effect {
            Some("read_only") => effect_may_survive == Some(false),
            Some("local_mutation" | "external_mutation" | "proposal_only" | "unknown") => {
                effect_may_survive == Some(true)
            }
            _ => false,
        };
        if !process_contract_is_exact || !effect_contract_is_exact {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!(
                    "prepared_process_effect_contract_invalid:{}",
                    draft.object_id
                ),
            }
            .into());
        }
        match (
            draft.payload.get("replayActionId").and_then(Value::as_str),
            draft.payload.get("replayClaimId").and_then(Value::as_str),
            draft
                .payload
                .get("replayClaimOwnerGeneration")
                .and_then(Value::as_u64),
            draft
                .payload
                .get("replayAuthorityBinding")
                .and_then(Value::as_str),
        ) {
            (None, None, None, None) => {}
            (Some(action_id), Some(claim_id), Some(owner_generation), Some(binding))
                if is_bounded_event_reference(action_id, 384)
                    && uuid::Uuid::parse_str(claim_id).is_ok()
                    && owner_generation > 0
                    && is_bounded_event_reference(binding, 384) => {}
            _ => {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("prepared_replay_claim_identity_invalid:{}", draft.object_id),
                }
                .into())
            }
        }
        if let Some(existing) = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_prepared",
            &draft.object_id,
        )? {
            if existing.run_id == draft.run_id
                && existing.payload == draft.payload
                && existing.created_at == draft.created_at
            {
                return Ok(());
            }
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("prepared_identity_conflict:{}", draft.object_id),
            }
            .into());
        }
        let start_exists = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.started",
            &draft.object_id,
        )?
        .is_some();
        let ambiguous_exists = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_ambiguous",
            &draft.object_id,
        )?
        .is_some();
        let mut terminal_exists = false;
        for event_type in TERMINAL_TYPES {
            terminal_exists |= select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                event_type,
                &draft.object_id,
            )?
            .is_some();
        }
        if start_exists || ambiguous_exists || terminal_exists {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("prepared_after_dispatch:{}", draft.object_id),
            }
            .into());
        }
        return Ok(());
    }
    if is_not_dispatched {
        let ToolLifecycleAdmission::LiveNotDispatched(receipt) = admission else {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("not_dispatched_without_live_receipt:{}", draft.object_id),
            }
            .into());
        };
        if !receipt.proves_not_dispatched()
            || receipt.receipt_id != draft.object_id
            || receipt.source_run_id.as_deref() != Some(draft.run_id.as_str())
            || draft.source != "openlife_turn_runtime.tool_not_dispatched"
            || draft.created_at != receipt.finished_at.unwrap_or(draft.created_at)
            || draft.payload.get("status").and_then(Value::as_str) != Some("not_dispatched")
            || draft.payload.get("dispatchKind").and_then(Value::as_str) != Some("not_attempted")
            || draft
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64)
                != Some(0)
            || draft
                .payload
                .get("dispatchObserved")
                .and_then(Value::as_bool)
                != Some(false)
            || draft.payload.get("transportStatus").and_then(Value::as_str) != Some("not_attempted")
            || draft.payload.get("effectStatus").and_then(Value::as_str) != Some("not_attempted")
            || draft
                .payload
                .get("executionOutcome")
                .and_then(Value::as_str)
                != Some(receipt.execution_outcome.as_str())
            || draft.payload.get("manifestId").and_then(Value::as_str)
                != receipt.manifest_id.as_deref()
            || draft.payload.get("requestDigest").and_then(Value::as_str)
                != Some(receipt.request_digest.as_str())
            || draft.payload.get("actionEffect").and_then(Value::as_str)
                != Some(receipt.action_effect.as_str())
            || draft
                .payload
                .get("idempotencyContract")
                .and_then(Value::as_str)
                != Some(receipt.idempotency_contract.as_str())
            || draft.payload.get("startedAt") != Some(&json!(receipt.started_at))
            || draft.payload.get("finishedAt") != Some(&json!(receipt.finished_at))
            || !draft
                .payload
                .get("dispatchedAt")
                .is_some_and(Value::is_null)
            || !draft
                .payload
                .get("responseObservedAt")
                .is_some_and(Value::is_null)
            || draft
                .payload
                .get("reconciledAfterProcessRestart")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("not_dispatched_live_receipt_mismatch:{}", draft.object_id),
            }
            .into());
        }
        let prepared = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_prepared",
            &draft.object_id,
        )?;
        if let Some(prepared) = prepared.as_ref() {
            for key in [
                "receiptId",
                "sourceRunId",
                "manifestId",
                "requestDigest",
                "actionEffect",
                "idempotencyContract",
            ] {
                if prepared.payload.get(key) != draft.payload.get(key) {
                    return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                        reason: format!(
                            "prepared_not_dispatched_identity_conflict:{}:{key}",
                            draft.object_id
                        ),
                    }
                    .into());
                }
            }
            if draft.payload.get("preparedEventId").and_then(Value::as_str)
                != Some(prepared.event_id.as_str())
                || draft.created_at < prepared.created_at
            {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!(
                        "not_dispatched_prepared_binding_invalid:{}",
                        draft.object_id
                    ),
                }
                .into());
            }
        } else if draft.payload.get("preparedEventId").is_some() {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("not_dispatched_unexpected_prepared_ref:{}", draft.object_id),
            }
            .into());
        }
        if select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.started",
            &draft.object_id,
        )?
        .is_some()
            || select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                "tool.dispatch_ambiguous",
                &draft.object_id,
            )?
            .is_some()
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("not_dispatched_after_dispatch:{}", draft.object_id),
            }
            .into());
        }
        for terminal_type in TERMINAL_TYPES {
            if terminal_type != "tool.not_dispatched"
                && select_event_by_immutable_identity(
                    conn,
                    &draft.task_session_id,
                    terminal_type,
                    &draft.object_id,
                )?
                .is_some()
            {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("not_dispatched_after_terminal:{}", draft.object_id),
                }
                .into());
            }
        }
        return Ok(());
    }
    if is_ambiguous {
        let reconciled_after_restart = draft.source == "bootstrap.prepared_tool_reconciliation";
        if !reconciled_after_restart {
            let dispatch_kind = draft.payload.get("dispatchKind").and_then(Value::as_str);
            let dispatch_attempt_count = draft
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64);
            let may_outlive = matches!(dispatch_kind, Some("network" | "mcp_stdio" | "a2a"));
            let effect_may_survive =
                draft.payload.get("actionEffect").and_then(Value::as_str) != Some("read_only");
            let expected_process_risk = if may_outlive {
                "may_outlive_local_process"
            } else {
                "process_bound"
            };
            let expected_transport = if may_outlive {
                "remote_unknown"
            } else {
                "local_aborted"
            };
            let expected_effect = if effect_may_survive {
                "unknown"
            } else {
                "not_attempted"
            };
            if !draft.source.starts_with("openlife_turn_runtime.")
                || draft.payload.get("status").and_then(Value::as_str) != Some("dispatch_ambiguous")
                || draft
                    .payload
                    .get("dispatchObserved")
                    .and_then(Value::as_bool)
                    != Some(false)
                || draft
                    .payload
                    .get("reconciledAfterProcessRestart")
                    .and_then(Value::as_bool)
                    != Some(false)
                || !matches!(
                    dispatch_kind,
                    Some("local" | "network" | "mcp_stdio" | "a2a" | "simulated")
                )
                || !dispatch_attempt_count.is_some_and(|count| count > 0)
                || !draft
                    .payload
                    .get("dispatchedAt")
                    .is_some_and(Value::is_null)
                || draft
                    .payload
                    .get("dispatchProcessRisk")
                    .and_then(Value::as_str)
                    != Some(expected_process_risk)
                || draft
                    .payload
                    .get("mayOutliveLocalProcess")
                    .and_then(Value::as_bool)
                    != Some(may_outlive)
                || draft
                    .payload
                    .get("effectMaySurviveLocalProcess")
                    .and_then(Value::as_bool)
                    != Some(effect_may_survive)
                || draft.payload.get("transportStatus").and_then(Value::as_str)
                    != Some(expected_transport)
                || draft.payload.get("effectStatus").and_then(Value::as_str)
                    != Some(expected_effect)
                || draft
                    .payload
                    .get("executionOutcome")
                    .and_then(Value::as_str)
                    != Some("unknown")
            {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!(
                        "runtime_dispatch_ambiguous_shape_invalid:{}",
                        draft.object_id
                    ),
                }
                .into());
            }
            if let Some(existing) = select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                "tool.dispatch_ambiguous",
                &draft.object_id,
            )? {
                if existing.run_id == draft.run_id
                    && existing.source == draft.source
                    && existing.payload == draft.payload
                    && existing.created_at == draft.created_at
                {
                    return Ok(());
                }
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("dispatch_ambiguous_identity_conflict:{}", draft.object_id),
                }
                .into());
            }
            if let Some(prepared) = select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                "tool.dispatch_prepared",
                &draft.object_id,
            )? {
                for key in [
                    "receiptId",
                    "sourceRunId",
                    "manifestId",
                    "requestDigest",
                    "actionEffect",
                    "idempotencyContract",
                ] {
                    if prepared.payload.get(key) != draft.payload.get(key) {
                        return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                            reason: format!(
                                "prepared_ambiguous_identity_conflict:{}:{key}",
                                draft.object_id
                            ),
                        }
                        .into());
                    }
                }
                if draft.created_at < prepared.created_at {
                    return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                        reason: format!("dispatch_ambiguous_before_prepared:{}", draft.object_id),
                    }
                    .into());
                }
            }
            let start_exists = select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                "tool.started",
                &draft.object_id,
            )?
            .is_some();
            let mut terminal_exists = false;
            for event_type in TERMINAL_TYPES {
                terminal_exists |= select_event_by_immutable_identity(
                    conn,
                    &draft.task_session_id,
                    event_type,
                    &draft.object_id,
                )?
                .is_some();
            }
            if start_exists || terminal_exists {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("dispatch_ambiguous_after_lifecycle:{}", draft.object_id),
                }
                .into());
            }
            return Ok(());
        }
        if draft.source != "bootstrap.prepared_tool_reconciliation"
            || draft.payload.get("status").and_then(Value::as_str) != Some("dispatch_ambiguous")
            || draft
                .payload
                .get("dispatchObserved")
                .and_then(Value::as_bool)
                != Some(false)
            || draft
                .payload
                .get("reconciledAfterProcessRestart")
                .and_then(Value::as_bool)
                != Some(true)
            || draft.payload.get("dispatchKind").and_then(Value::as_str) != Some("unknown")
            || draft
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64)
                != Some(0)
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("dispatch_ambiguous_shape_invalid:{}", draft.object_id),
            }
            .into());
        }
        let prepared = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_prepared",
            &draft.object_id,
        )?
        .ok_or_else(|| MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("dispatch_ambiguous_without_prepared:{}", draft.object_id),
        })?;
        for key in [
            "receiptId",
            "sourceRunId",
            "manifestId",
            "requestDigest",
            "actionEffect",
            "idempotencyContract",
            "replayActionId",
            "replayClaimId",
            "replayClaimOwnerGeneration",
            "replayAuthorityBinding",
        ] {
            if prepared.payload.get(key) != draft.payload.get(key) {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!(
                        "prepared_ambiguous_identity_conflict:{}:{key}",
                        draft.object_id
                    ),
                }
                .into());
            }
        }
        let prepared_process_risk = prepared
            .payload
            .get("dispatchProcessRisk")
            .and_then(Value::as_str);
        let prepared_may_outlive = prepared
            .payload
            .get("mayOutliveLocalProcess")
            .and_then(Value::as_bool);
        let expected_process_contract = match (prepared_process_risk, prepared_may_outlive) {
            (Some("process_bound"), Some(false)) => ("process_bound", false),
            (Some("may_outlive_local_process"), Some(true)) => ("may_outlive_local_process", true),
            // Rows written before this contract existed contain neither field.
            // A restart cannot prove that their dispatch remained process-bound,
            // so reconciliation must conservatively retain remote uncertainty.
            (None, None) => ("may_outlive_local_process", true),
            _ => {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("prepared_process_contract_inconsistent:{}", draft.object_id),
                }
                .into())
            }
        };
        if draft
            .payload
            .get("dispatchProcessRisk")
            .and_then(Value::as_str)
            != Some(expected_process_contract.0)
            || draft
                .payload
                .get("mayOutliveLocalProcess")
                .and_then(Value::as_bool)
                != Some(expected_process_contract.1)
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!(
                    "prepared_ambiguous_process_contract_conflict:{}",
                    draft.object_id
                ),
            }
            .into());
        }
        let expected_effect_may_survive = match prepared
            .payload
            .get("effectMaySurviveLocalProcess")
            .and_then(Value::as_bool)
        {
            Some(value) => value,
            None => {
                prepared.payload.get("actionEffect").and_then(Value::as_str) != Some("read_only")
            }
        };
        if draft
            .payload
            .get("effectMaySurviveLocalProcess")
            .and_then(Value::as_bool)
            != Some(expected_effect_may_survive)
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!(
                    "prepared_ambiguous_effect_contract_conflict:{}",
                    draft.object_id
                ),
            }
            .into());
        }
        let mut observed_lifecycle_exists = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.started",
            &draft.object_id,
        )?
        .is_some();
        for event_type in TERMINAL_TYPES {
            observed_lifecycle_exists |= select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                event_type,
                &draft.object_id,
            )?
            .is_some();
        }
        if draft.created_at < prepared.created_at || observed_lifecycle_exists {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!(
                    "dispatch_ambiguous_after_observed_lifecycle:{}",
                    draft.object_id
                ),
            }
            .into());
        }
        return Ok(());
    }
    let ambiguous_predecessor = if is_terminal {
        select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_ambiguous",
            &draft.object_id,
        )?
    } else {
        None
    };
    if let Some(ambiguous) = ambiguous_predecessor.as_ref() {
        let restart_reconciliation = ambiguous.source == "bootstrap.prepared_tool_reconciliation";
        let shape_valid = if restart_reconciliation {
            draft.source == "bootstrap.prepared_tool_reconciliation"
                && draft.payload.get("dispatchKind").and_then(Value::as_str) == Some("unknown")
                && draft
                    .payload
                    .get("dispatchAttemptCount")
                    .and_then(Value::as_u64)
                    == Some(0)
                && draft
                    .payload
                    .get("reconciledAfterProcessRestart")
                    .and_then(Value::as_bool)
                    == Some(true)
        } else {
            (draft.source == "openlife_turn_runtime"
                || draft.source.starts_with("openlife_turn_runtime."))
                && matches!(
                    draft.payload.get("dispatchKind").and_then(Value::as_str),
                    Some("local" | "network" | "mcp_stdio" | "a2a" | "simulated")
                )
                && draft
                    .payload
                    .get("dispatchAttemptCount")
                    .and_then(Value::as_u64)
                    .is_some_and(|count| count > 0)
                && draft
                    .payload
                    .get("reconciledAfterProcessRestart")
                    .and_then(Value::as_bool)
                    == Some(false)
                && draft
                    .payload
                    .get("dispatchedAt")
                    .is_some_and(Value::is_null)
        };
        if !shape_valid
            || draft
                .payload
                .get("dispatchObserved")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("ambiguous_terminal_shape_invalid:{}", draft.object_id),
            }
            .into());
        }
    } else {
        draft
            .payload
            .get("dispatchKind")
            .and_then(Value::as_str)
            .filter(|value| {
                matches!(
                    *value,
                    "local" | "network" | "mcp_stdio" | "a2a" | "simulated"
                )
            })
            .ok_or_else(|| MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("dispatch_kind_missing:{}", draft.object_id),
            })?;
        draft
            .payload
            .get("dispatchAttemptCount")
            .and_then(Value::as_u64)
            .filter(|count| *count > 0)
            .ok_or_else(|| MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("dispatch_attempt_count_missing:{}", draft.object_id),
            })?;
        if draft
            .payload
            .get("dispatchObserved")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("concrete_dispatch_observation_missing:{}", draft.object_id),
            }
            .into());
        }
    }
    draft
        .payload
        .get("executionOutcome")
        .and_then(Value::as_str)
        .filter(|value| matches!(*value, "not_observed" | "succeeded" | "failed" | "unknown"))
        .ok_or_else(|| MainChatAgentEventStoreFault::ToolLifecycleConflict {
            reason: format!("execution_outcome_missing:{}", draft.object_id),
        })?;

    let identity_matches = |event: &MainChatAgentDurableEvent| {
        event.run_id == draft.run_id
            && [
                "receiptId",
                "sourceRunId",
                "manifestId",
                "requestDigest",
                "actionEffect",
                "idempotencyContract",
                "dispatchKind",
                "replayActionId",
                "replayClaimId",
                "replayClaimOwnerGeneration",
                "replayAuthorityBinding",
            ]
            .into_iter()
            .all(|key| event.payload.get(key) == draft.payload.get(key))
    };
    if is_start {
        if select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_ambiguous",
            &draft.object_id,
        )?
        .is_some()
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("start_after_dispatch_ambiguous:{}", draft.object_id),
            }
            .into());
        }
        if let Some(prepared) = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.dispatch_prepared",
            &draft.object_id,
        )? {
            for key in [
                "receiptId",
                "sourceRunId",
                "manifestId",
                "requestDigest",
                "actionEffect",
                "idempotencyContract",
            ] {
                if prepared.payload.get(key) != draft.payload.get(key) {
                    return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                        reason: format!("prepared_start_identity_conflict:{}", draft.object_id),
                    }
                    .into());
                }
            }
            if draft.created_at < prepared.created_at {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("start_before_prepared:{}", draft.object_id),
                }
                .into());
            }
        }
        if let Some(existing_start) = select_event_by_immutable_identity(
            conn,
            &draft.task_session_id,
            "tool.started",
            &draft.object_id,
        )? {
            if identity_matches(&existing_start) && existing_start.created_at == draft.created_at {
                return Ok(());
            }
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("start_identity_conflict:{}", draft.object_id),
            }
            .into());
        }
        for terminal_type in TERMINAL_TYPES {
            if select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                terminal_type,
                &draft.object_id,
            )?
            .is_some()
            {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("start_after_terminal:{}", draft.object_id),
                }
                .into());
            }
        }
        return Ok(());
    }

    let start = select_event_by_immutable_identity(
        conn,
        &draft.task_session_id,
        "tool.started",
        &draft.object_id,
    )?;
    match (start.as_ref(), ambiguous_predecessor.as_ref()) {
        (Some(_), Some(_)) => {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("terminal_has_two_dispatch_predecessors:{}", draft.object_id),
            }
            .into())
        }
        (Some(start), None) => {
            if !identity_matches(start) || draft.created_at < start.created_at {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("terminal_identity_conflict:{}", draft.object_id),
                }
                .into());
            }
            let started_attempts = start
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64)
                .unwrap_or(1);
            let terminal_attempts = draft
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if terminal_attempts < started_attempts {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("terminal_attempt_count_regressed:{}", draft.object_id),
                }
                .into());
            }
        }
        (None, Some(ambiguous)) => {
            if !identity_matches(ambiguous) || draft.created_at < ambiguous.created_at {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("ambiguous_terminal_identity_conflict:{}", draft.object_id),
                }
                .into());
            }
            let may_outlive = ambiguous
                .payload
                .get("mayOutliveLocalProcess")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let effect_may_survive = ambiguous
                .payload
                .get("effectMaySurviveLocalProcess")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let expected = if may_outlive {
                (
                    "tool.remote_unknown",
                    "remote_unknown",
                    if effect_may_survive {
                        "unknown"
                    } else {
                        "not_attempted"
                    },
                )
            } else if effect_may_survive {
                ("tool.effect_unknown", "local_aborted", "unknown")
            } else {
                ("tool.local_aborted", "local_aborted", "not_attempted")
            };
            if draft.event_type != expected.0
                || draft.payload.get("transportStatus").and_then(Value::as_str) != Some(expected.1)
                || draft.payload.get("effectStatus").and_then(Value::as_str) != Some(expected.2)
                || draft
                    .payload
                    .get("executionOutcome")
                    .and_then(Value::as_str)
                    != Some("unknown")
            {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!("ambiguous_terminal_certainty_invalid:{}", draft.object_id),
                }
                .into());
            }
            let ambiguous_attempts = ambiguous
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let terminal_attempts = draft
                .payload
                .get("dispatchAttemptCount")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if terminal_attempts < ambiguous_attempts {
                return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                    reason: format!(
                        "ambiguous_terminal_attempt_count_regressed:{}",
                        draft.object_id
                    ),
                }
                .into());
            }
        }
        (None, None) => {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!(
                    "terminal_without_start_or_ambiguous_predecessor:{}",
                    draft.object_id
                ),
            }
            .into())
        }
    }
    for terminal_type in TERMINAL_TYPES {
        if terminal_type != draft.event_type
            && select_event_by_immutable_identity(
                conn,
                &draft.task_session_id,
                terminal_type,
                &draft.object_id,
            )?
            .is_some()
        {
            return Err(MainChatAgentEventStoreFault::ToolLifecycleConflict {
                reason: format!("conflicting_terminal:{}", draft.object_id),
            }
            .into());
        }
    }
    Ok(())
}

const PERSISTED_PROVIDER_EVENT_TYPES: [&str; 4] = [
    "provider.started",
    "provider.completed",
    "provider.failed",
    "provider.remote_unknown",
];
const PERSISTED_PROVIDER_TERMINAL_TYPES: [&str; 3] = [
    "provider.completed",
    "provider.failed",
    "provider.remote_unknown",
];
const PROVIDER_POLICY_IDENTITY_FIELDS: [&str; 19] = [
    "providerConfigGeneration",
    "policyDecisionId",
    "policyVersion",
    "policyAuthority",
    "effectiveDataRoute",
    "effectiveLocalRestriction",
    "subjectScopeDigest",
    "payloadPurpose",
    "unfilteredPayloadDigest",
    "contextManifestDigest",
    "preparedEnvelopeDigest",
    "networkPolicyDecisionDigest",
    "selectedContextRefs",
    "includedContextCategories",
    "declaredPayloadCategories",
    "policyProvenanceRefs",
    "rawLifeModelIncluded",
    "rawUnboundedMemoryIncluded",
    "policyEvidenceDigest",
];
const MAX_PERSISTED_PROVIDER_LIFECYCLE_EVENTS_PER_TASK: usize =
    crate::main_chat_cancellation::MAX_PROVIDER_ATTEMPTS_PER_TURN * 2;

fn persisted_provider_lifecycle_unverified(reason: &str, object_id: &str) -> anyhow::Error {
    MainChatAgentEventStoreFault::PersistedProviderLifecycleUnverified {
        reason: format!("{reason}:{object_id}"),
    }
    .into()
}

fn persisted_provider_timestamp(
    event: &MainChatAgentDurableEvent,
    field: &str,
) -> Result<Option<DateTime<Utc>>> {
    let Some(value) = event.payload.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let raw = value.as_str().ok_or_else(|| {
        persisted_provider_lifecycle_unverified("timestamp_not_string", &event.object_id)
    })?;
    let timestamp = DateTime::parse_from_rfc3339(raw)
        .map_err(|_| {
            persisted_provider_lifecycle_unverified("timestamp_invalid", &event.object_id)
        })?
        .with_timezone(&Utc);
    Ok(Some(timestamp))
}

/// The sole verified cross-owner transition. The adapter observed dispatch,
/// while TurnRuntime later observed only local cancellation and an unknown
/// remote terminal. Every field is metadata-only and required so a generic
/// source rewrite cannot masquerade as this cancellation edge.
pub(crate) fn provider_remote_unknown_has_runtime_cancel_contract(
    event: &MainChatAgentDurableEvent,
) -> bool {
    if event.event_type != "provider.remote_unknown"
        || event.object_type != "provider_request"
        || event.source != "openlife_turn_runtime"
        || event.payload.get("status").and_then(Value::as_str) != Some("remote_unknown")
        || event
            .payload
            .get("cancellationId")
            .and_then(Value::as_str)
            .map_or(true, |value| value.trim().is_empty())
        || event
            .payload
            .get("localWaitAborted")
            .and_then(Value::as_bool)
            != Some(true)
        || event
            .payload
            .get("localKernelFutureDropped")
            .and_then(Value::as_bool)
            != Some(true)
        || event
            .payload
            .get("remoteCancellationConfirmed")
            .and_then(Value::as_bool)
            != Some(false)
        || event.payload.get("kernelFailureReceiptId").is_some()
        || event.payload.get("adapterTerminalObserved").is_some()
    {
        return false;
    }
    let Some(_started_at) = event
        .payload
        .get("startedAt")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
    else {
        return false;
    };
    let Some(observed_at) = event
        .payload
        .get("observedAt")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
    else {
        return false;
    };
    observed_at == event.created_at
}

/// The adapter observed a request start, but the owning kernel failed before
/// emitting an adapter terminal. This is deliberately distinct from local
/// cancellation: no cancellation id or remote-cancel claim is permitted, and
/// the unknown edge must reference a durable failed-turn receipt.
pub(crate) fn provider_remote_unknown_has_runtime_kernel_failure_contract(
    event: &MainChatAgentDurableEvent,
) -> bool {
    if event.event_type != "provider.remote_unknown"
        || event.object_type != "provider_request"
        || event.source != "openlife_turn_runtime"
        || event.payload.get("status").and_then(Value::as_str) != Some("remote_unknown")
        || event
            .payload
            .get("kernelFailureReceiptId")
            .and_then(Value::as_str)
            .map_or(true, |value| value.trim().is_empty())
        || event
            .payload
            .get("localKernelFutureDropped")
            .and_then(Value::as_bool)
            != Some(true)
        || event
            .payload
            .get("adapterTerminalObserved")
            .and_then(Value::as_bool)
            != Some(false)
        || event.payload.get("reasonCode").and_then(Value::as_str)
            != Some("kernel_failed_before_provider_terminal_observed")
        || event.payload.get("cancellationId").is_some()
        || event.payload.get("localWaitAborted").is_some()
        || event.payload.get("remoteCancellationConfirmed").is_some()
    {
        return false;
    }
    let Some(_started_at) = event
        .payload
        .get("startedAt")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
    else {
        return false;
    };
    let Some(observed_at) = event
        .payload
        .get("observedAt")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
    else {
        return false;
    };
    observed_at == event.created_at
}

fn provider_lifecycle_source_transition_is_verified(
    start: &MainChatAgentDurableEvent,
    terminal: &MainChatAgentDurableEvent,
) -> bool {
    start.source == "provider_adapter"
        && (terminal.source == "provider_adapter"
            || provider_remote_unknown_has_runtime_cancel_contract(terminal)
            || provider_remote_unknown_has_runtime_kernel_failure_contract(terminal))
}

fn validate_persisted_provider_event_shape(event: &MainChatAgentDurableEvent) -> Result<()> {
    if !PERSISTED_PROVIDER_EVENT_TYPES.contains(&event.event_type.as_str()) {
        return Err(persisted_provider_lifecycle_unverified(
            "unsupported_event_type",
            &event.object_id,
        ));
    }
    if event.object_type != "provider_request" {
        return Err(persisted_provider_lifecycle_unverified(
            "invalid_object_type",
            &event.object_id,
        ));
    }
    let source_verified = match event.event_type.as_str() {
        "provider.started" | "provider.completed" | "provider.failed" => {
            event.source == "provider_adapter"
        }
        "provider.remote_unknown" => {
            event.source == "provider_adapter"
                || provider_remote_unknown_has_runtime_cancel_contract(event)
                || provider_remote_unknown_has_runtime_kernel_failure_contract(event)
        }
        _ => false,
    };
    if !source_verified {
        return Err(persisted_provider_lifecycle_unverified(
            "source_unverified",
            &event.object_id,
        ));
    }
    let request_id = event
        .payload
        .get("requestId")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            persisted_provider_lifecycle_unverified("request_id_missing", &event.object_id)
        })?;
    if request_id != event.object_id {
        return Err(persisted_provider_lifecycle_unverified(
            "request_id_mismatch",
            &event.object_id,
        ));
    }
    for field in [
        "provider",
        "model",
        "providerConfigGeneration",
        "policyDecisionId",
        "policyVersion",
        "policyAuthority",
        "effectiveDataRoute",
        "subjectScopeDigest",
        "contextManifestDigest",
        "networkPolicyDecisionDigest",
        "policyEvidenceDigest",
    ] {
        if event
            .payload
            .get(field)
            .and_then(Value::as_str)
            .map_or(true, |value| value.trim().is_empty())
        {
            return Err(persisted_provider_lifecycle_unverified(
                &format!("{field}_missing"),
                &event.object_id,
            ));
        }
    }
    let closed_string = |field: &str, allowed: &[&str]| {
        event
            .payload
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| allowed.contains(&value))
    };
    if !closed_string(
        "policyAuthority",
        &[
            "main_chat_policy_router",
            "hs_policy_store",
            "scheduled_policy",
            "explicit_provider_probe_policy",
            "local_only_fail_closed",
        ],
    ) {
        return Err(persisted_provider_lifecycle_unverified(
            "policy_authority_invalid",
            &event.object_id,
        ));
    }
    if !closed_string("effectiveDataRoute", &["policy_allowed", "local_only"]) {
        return Err(persisted_provider_lifecycle_unverified(
            "effective_data_route_invalid",
            &event.object_id,
        ));
    }
    if !closed_string(
        "payloadPurpose",
        &[
            "main_chat_direct_answer",
            "main_chat_artifact_draft",
            "main_chat_react_ranking",
            "agent_loop_step",
            "agent_runtime_generation",
            "layered_reasoning_phase",
            "frozen_runtime_evaluation",
            "explicit_provider_probe",
        ],
    ) {
        return Err(persisted_provider_lifecycle_unverified(
            "payload_purpose_invalid",
            &event.object_id,
        ));
    }
    let local_restriction = event.payload.get("effectiveLocalRestriction");
    if !matches!(local_restriction, Some(Value::Null))
        && !local_restriction
            .and_then(Value::as_str)
            .is_some_and(|value| {
                [
                    "missing_canonical_policy",
                    "cloud_disabled",
                    "canonical_route_intersection",
                    "deserialized_capability_unavailable",
                    "test_fixture",
                ]
                .contains(&value)
            })
    {
        return Err(persisted_provider_lifecycle_unverified(
            "effective_local_restriction_invalid",
            &event.object_id,
        ));
    }
    if (event.payload["policyAuthority"] == "local_only_fail_closed"
        || !matches!(local_restriction, Some(Value::Null)))
        && event.payload["effectiveDataRoute"] != "local_only"
    {
        return Err(persisted_provider_lifecycle_unverified(
            "local_restriction_route_conflict",
            &event.object_id,
        ));
    }
    for field in ["unfilteredPayloadDigest", "preparedEnvelopeDigest"] {
        if !event
            .payload
            .get(field)
            .is_some_and(is_canonical_redacted_string)
        {
            return Err(persisted_provider_lifecycle_unverified(
                &format!("{field}_missing_exact_scope"),
                &event.object_id,
            ));
        }
    }
    let declared_payload_categories = event
        .payload
        .get("declaredPayloadCategories")
        .and_then(Value::as_array);
    if declared_payload_categories.is_none_or(|categories| {
        categories.is_empty()
            || categories.iter().any(|category| {
                category.as_str().is_none_or(|value| {
                    ![
                        "current_user_conversation",
                        "runtime_compiled_messages",
                        "frozen_evaluation_input",
                        "main_chat_react_candidate_ranking",
                        "a2a_authenticated_user_message",
                        "explicit_provider_probe",
                        "privacy_policy_masked",
                    ]
                    .contains(&value)
                })
            })
    }) {
        return Err(persisted_provider_lifecycle_unverified(
            "declared_payload_categories_invalid",
            &event.object_id,
        ));
    }
    if event
        .payload
        .get("rawLifeModelIncluded")
        .and_then(Value::as_bool)
        != Some(false)
        || event
            .payload
            .get("rawUnboundedMemoryIncluded")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(persisted_provider_lifecycle_unverified(
            "unsafe_raw_payload_claim",
            &event.object_id,
        ));
    }
    if event.event_type == "provider.started" {
        if let Some(started_at) = persisted_provider_timestamp(event, "startedAt")? {
            if started_at != event.created_at {
                return Err(persisted_provider_lifecycle_unverified(
                    "started_at_envelope_mismatch",
                    &event.object_id,
                ));
            }
        }
    }
    Ok(())
}

/// Validate the complete provider lifecycle for a task before returning any
/// partial task view. A page boundary, event id lookup, or latest cursor must
/// never turn a corrupt sibling event into apparently valid product truth.
fn validate_persisted_provider_lifecycles_for_task(
    conn: &Connection,
    task_session_id: &str,
) -> Result<()> {
    validate_task_sequence_domain(conn, task_session_id)?;
    validate_task_run_binding(conn, task_session_id)?;
    let events = {
        let row_limit = i64::try_from(MAX_PERSISTED_PROVIDER_LIFECYCLE_EVENTS_PER_TASK + 1)
            .expect("provider lifecycle row cap fits i64");
        let mut statement = conn.prepare(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
             FROM main_chat_agent_events
             WHERE task_session_id = ?1
               AND event_type IN ('provider.started', 'provider.completed', 'provider.failed', 'provider.remote_unknown')
             ORDER BY sequence ASC
             LIMIT ?2",
        )?;
        let events = statement
            .query_map(params![task_session_id, row_limit], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if events.len() > MAX_PERSISTED_PROVIDER_LIFECYCLE_EVENTS_PER_TASK {
            return Err(persisted_provider_lifecycle_unverified(
                "provider_lifecycle_limit_exceeded",
                task_session_id,
            ));
        }
        events
    };
    let mut lifecycles =
        std::collections::BTreeMap::<String, Vec<MainChatAgentDurableEvent>>::new();
    for event in events {
        validate_persisted_provider_event_shape(&event)?;
        lifecycles
            .entry(event.object_id.clone())
            .or_default()
            .push(event);
    }
    if lifecycles.len() > crate::main_chat_cancellation::MAX_PROVIDER_ATTEMPTS_PER_TURN {
        return Err(persisted_provider_lifecycle_unverified(
            "provider_lifecycle_limit_exceeded",
            task_session_id,
        ));
    }

    for (request_id, events) in lifecycles {
        let starts = events
            .iter()
            .filter(|event| event.event_type == "provider.started")
            .collect::<Vec<_>>();
        let terminals = events
            .iter()
            .filter(|event| PERSISTED_PROVIDER_TERMINAL_TYPES.contains(&event.event_type.as_str()))
            .collect::<Vec<_>>();
        if starts.len() > 1 {
            return Err(persisted_provider_lifecycle_unverified(
                "duplicate_start",
                &request_id,
            ));
        }
        if terminals.len() > 1 {
            return Err(persisted_provider_lifecycle_unverified(
                "conflicting_terminal",
                &request_id,
            ));
        }
        let Some(start) = starts.first().copied() else {
            return Err(persisted_provider_lifecycle_unverified(
                "terminal_without_start",
                &request_id,
            ));
        };
        let Some(terminal) = terminals.first().copied() else {
            continue;
        };
        if start.run_id != terminal.run_id {
            return Err(persisted_provider_lifecycle_unverified(
                "run_identity_mismatch",
                &request_id,
            ));
        }
        if [
            "provider",
            "model",
            "providerConfigGeneration",
            "policyEvidenceDigest",
        ]
        .into_iter()
        .any(|field| start.payload.get(field) != terminal.payload.get(field))
            || PROVIDER_POLICY_IDENTITY_FIELDS
                .iter()
                .any(|field| start.payload.get(*field) != terminal.payload.get(*field))
        {
            return Err(persisted_provider_lifecycle_unverified(
                "adapter_identity_mismatch",
                &request_id,
            ));
        }
        if !provider_lifecycle_source_transition_is_verified(start, terminal) {
            return Err(persisted_provider_lifecycle_unverified(
                "source_mismatch",
                &request_id,
            ));
        }
        if terminal.sequence <= start.sequence {
            return Err(persisted_provider_lifecycle_unverified(
                "terminal_before_start",
                &request_id,
            ));
        }
        if let Some(terminal_started_at) = persisted_provider_timestamp(terminal, "startedAt")? {
            if terminal_started_at != start.created_at {
                return Err(persisted_provider_lifecycle_unverified(
                    "terminal_started_at_mismatch",
                    &request_id,
                ));
            }
        }
        if let Some(finished_at) = persisted_provider_timestamp(terminal, "finishedAt")? {
            if finished_at != terminal.created_at {
                return Err(persisted_provider_lifecycle_unverified(
                    "finished_at_envelope_mismatch",
                    &request_id,
                ));
            }
        }
        if terminal.event_type == "provider.remote_unknown" {
            if let Some(observed_at) = persisted_provider_timestamp(terminal, "observedAt")? {
                if observed_at != terminal.created_at {
                    return Err(persisted_provider_lifecycle_unverified(
                        "observed_at_envelope_mismatch",
                        &request_id,
                    ));
                }
            }
        }
        if provider_remote_unknown_has_runtime_cancel_contract(terminal) {
            let cancellation_id = terminal
                .payload
                .get("cancellationId")
                .and_then(Value::as_str)
                .expect("verified runtime cancellation contract has an id");
            let cancellation = select_event_by_immutable_identity(
                conn,
                task_session_id,
                "cancel_requested",
                cancellation_id,
            )?
            .ok_or_else(|| {
                persisted_provider_lifecycle_unverified("runtime_cancel_event_missing", &request_id)
            })?;
            if cancellation.run_id != terminal.run_id
                || cancellation.object_type != "turn"
                || cancellation.source != "openlife_turn_runtime"
                || cancellation.sequence >= terminal.sequence
                || cancellation
                    .payload
                    .get("cancellationId")
                    .and_then(Value::as_str)
                    != Some(cancellation_id)
            {
                return Err(persisted_provider_lifecycle_unverified(
                    "runtime_cancel_event_mismatch",
                    &request_id,
                ));
            }
        } else if provider_remote_unknown_has_runtime_kernel_failure_contract(terminal) {
            let failure_receipt_id = terminal
                .payload
                .get("kernelFailureReceiptId")
                .and_then(Value::as_str)
                .expect("verified kernel-failure contract has a receipt id");
            let failure = select_event_by_immutable_identity(
                conn,
                task_session_id,
                "failed",
                failure_receipt_id,
            )?
            .ok_or_else(|| {
                persisted_provider_lifecycle_unverified(
                    "runtime_kernel_failure_event_missing",
                    &request_id,
                )
            })?;
            if failure.run_id != terminal.run_id
                || failure.object_type != "turn"
                || failure.source != "openlife_turn_runtime.kernel_error_pre_commit"
                || failure.sequence >= terminal.sequence
                || failure.created_at != terminal.created_at
                || failure.payload.get("status").and_then(Value::as_str) != Some("failed")
                || failure.payload.get("kind").and_then(Value::as_str) != Some("unknown_error")
                || failure
                    .payload
                    .get("durableCommitAllowedAfterFailure")
                    .and_then(Value::as_bool)
                    != Some(false)
            {
                return Err(persisted_provider_lifecycle_unverified(
                    "runtime_kernel_failure_event_mismatch",
                    &request_id,
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadNormalizationOrigin {
    New,
    Legacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadValueSchema {
    MetadataString,
    MetadataStringOrNull,
    PlanStatus,
    ToolActionEffect,
    ToolIdempotencyContract,
    ToolDispatchKind,
    ToolTransportStatus,
    ToolEffectStatus,
    ToolExecutionOutcome,
    ToolAuditPersistenceStatus,
    RedactedString,
    RedactedStringOrNull,
    ReasonCode,
    ReasonCodeOrNull,
    OpaqueDigest,
    RedactedDigestOrNull,
    TimestampOrNull,
    Bool,
    Count,
    MetadataStringArray,
    ContextReferenceArray,
    OpaqueDigestArray,
    MetadataStringArrayOrRedacted,
    ReadExecutionOrNull,
    ChildWorkflowProvenance,
}

#[derive(Debug, Clone, Copy)]
struct PayloadFieldSchema {
    key: &'static str,
    value_schema: PayloadValueSchema,
    required: bool,
}

impl PayloadFieldSchema {
    const fn required(key: &'static str, value_schema: PayloadValueSchema) -> Self {
        Self {
            key,
            value_schema,
            required: true,
        }
    }

    const fn optional(key: &'static str, value_schema: PayloadValueSchema) -> Self {
        Self {
            key,
            value_schema,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DurableEventPayloadSchema {
    object_type: &'static str,
    fields: &'static [PayloadFieldSchema],
    reject_unknown_fields: bool,
    expected_status: Option<&'static str>,
}

const MAX_METADATA_STRING_CHARS: usize = 256;
const MAX_REDACTED_STRING_CHARS: usize = 2_048;
const MAX_REDACTED_VALUE_BYTES: usize = 1024 * 1024;
const MAX_METADATA_ARRAY_ITEMS: usize = 256;
const MAX_EVENT_COUNT: u64 = 1_000_000;
const MAX_UNRECOGNIZED_FIELD_COUNT: usize = 64;

const TURN_STARTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::optional("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("requestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("policyRoute", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("selectedStrategy", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("rawUserTextStored", PayloadValueSchema::Bool),
];
const TURN_TERMINAL_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("kind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("errorDigest", PayloadValueSchema::RedactedDigestOrNull),
    PayloadFieldSchema::optional("cancellationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("observedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("reasonCode", PayloadValueSchema::ReasonCode),
    PayloadFieldSchema::optional("providerAttemptState", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("remoteProviderState", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("durableCommitAllowedAfterFailure", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("durableCommitAllowedAfterCancel", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
];
const CANCEL_REQUESTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("cancellationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("observedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("reasonCode", PayloadValueSchema::ReasonCode),
    PayloadFieldSchema::optional("durableCommitAllowedAfterCancel", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("localWaitAborted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("remoteCancellationConfirmed", PayloadValueSchema::Bool),
];
const PROVIDER_STARTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("requestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("provider", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("model", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required(
        "providerConfigGeneration",
        PayloadValueSchema::MetadataString,
    ),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("startedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("cancellationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyDecisionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyVersion", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyAuthority", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("effectiveDataRoute", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required(
        "effectiveLocalRestriction",
        PayloadValueSchema::MetadataStringOrNull,
    ),
    PayloadFieldSchema::required("subjectScopeDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("payloadPurpose", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::required(
        "unfilteredPayloadDigest",
        PayloadValueSchema::RedactedDigestOrNull,
    ),
    PayloadFieldSchema::required("contextManifestDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required(
        "preparedEnvelopeDigest",
        PayloadValueSchema::RedactedDigestOrNull,
    ),
    PayloadFieldSchema::required(
        "networkPolicyDecisionDigest",
        PayloadValueSchema::OpaqueDigest,
    ),
    PayloadFieldSchema::required(
        "selectedContextRefs",
        PayloadValueSchema::ContextReferenceArray,
    ),
    PayloadFieldSchema::required(
        "includedContextCategories",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required(
        "declaredPayloadCategories",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required(
        "policyProvenanceRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required("rawLifeModelIncluded", PayloadValueSchema::Bool),
    PayloadFieldSchema::required("rawUnboundedMemoryIncluded", PayloadValueSchema::Bool),
    PayloadFieldSchema::required("policyEvidenceDigest", PayloadValueSchema::OpaqueDigest),
];
const PROVIDER_COMPLETED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("requestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("provider", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("model", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required(
        "providerConfigGeneration",
        PayloadValueSchema::MetadataString,
    ),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("errorDigest", PayloadValueSchema::RedactedDigestOrNull),
    PayloadFieldSchema::optional("startedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("finishedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("cancellationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyDecisionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyVersion", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyAuthority", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("effectiveDataRoute", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required(
        "effectiveLocalRestriction",
        PayloadValueSchema::MetadataStringOrNull,
    ),
    PayloadFieldSchema::required("subjectScopeDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("payloadPurpose", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::required(
        "unfilteredPayloadDigest",
        PayloadValueSchema::RedactedDigestOrNull,
    ),
    PayloadFieldSchema::required("contextManifestDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required(
        "preparedEnvelopeDigest",
        PayloadValueSchema::RedactedDigestOrNull,
    ),
    PayloadFieldSchema::required(
        "networkPolicyDecisionDigest",
        PayloadValueSchema::OpaqueDigest,
    ),
    PayloadFieldSchema::required(
        "selectedContextRefs",
        PayloadValueSchema::ContextReferenceArray,
    ),
    PayloadFieldSchema::required(
        "includedContextCategories",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required(
        "declaredPayloadCategories",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required(
        "policyProvenanceRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required("rawLifeModelIncluded", PayloadValueSchema::Bool),
    PayloadFieldSchema::required("rawUnboundedMemoryIncluded", PayloadValueSchema::Bool),
    PayloadFieldSchema::required("policyEvidenceDigest", PayloadValueSchema::OpaqueDigest),
];
const PROVIDER_REMOTE_UNKNOWN_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("requestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("provider", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("model", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required(
        "providerConfigGeneration",
        PayloadValueSchema::MetadataString,
    ),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("errorDigest", PayloadValueSchema::RedactedDigestOrNull),
    PayloadFieldSchema::optional("startedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("finishedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("observedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::required("policyDecisionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyVersion", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("policyAuthority", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("effectiveDataRoute", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required(
        "effectiveLocalRestriction",
        PayloadValueSchema::MetadataStringOrNull,
    ),
    PayloadFieldSchema::required("subjectScopeDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("payloadPurpose", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::required(
        "unfilteredPayloadDigest",
        PayloadValueSchema::RedactedDigestOrNull,
    ),
    PayloadFieldSchema::required("contextManifestDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required(
        "preparedEnvelopeDigest",
        PayloadValueSchema::RedactedDigestOrNull,
    ),
    PayloadFieldSchema::required(
        "networkPolicyDecisionDigest",
        PayloadValueSchema::OpaqueDigest,
    ),
    PayloadFieldSchema::required(
        "selectedContextRefs",
        PayloadValueSchema::ContextReferenceArray,
    ),
    PayloadFieldSchema::required(
        "includedContextCategories",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required(
        "declaredPayloadCategories",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required(
        "policyProvenanceRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::required("rawLifeModelIncluded", PayloadValueSchema::Bool),
    PayloadFieldSchema::required("rawUnboundedMemoryIncluded", PayloadValueSchema::Bool),
    PayloadFieldSchema::required("policyEvidenceDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::optional("cancellationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("localWaitAborted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("localKernelFutureDropped", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("remoteCancellationConfirmed", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("adapterTerminalObserved", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("kernelFailureReceiptId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("reasonCode", PayloadValueSchema::ReasonCode),
];
const PROVIDER_ATTEMPT_FAILURE_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("providerAttemptState", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("reasonCode", PayloadValueSchema::ReasonCode),
    PayloadFieldSchema::required("observedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::required("remoteProviderState", PayloadValueSchema::MetadataString),
];
const TOOL_RECEIPT_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("receiptId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("requestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("sourceRunId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("manifestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("requestDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("actionEffect", PayloadValueSchema::ToolActionEffect),
    PayloadFieldSchema::required(
        "idempotencyContract",
        PayloadValueSchema::ToolIdempotencyContract,
    ),
    // These remain optional in the current decoder so pre-v6 lifecycle rows
    // can migrate without inventing dispatch facts.
    // `validate_tool_lifecycle_transition` requires both on every new write.
    PayloadFieldSchema::optional("dispatchKind", PayloadValueSchema::ToolDispatchKind),
    PayloadFieldSchema::optional("dispatchAttemptCount", PayloadValueSchema::Count),
    PayloadFieldSchema::required("transportStatus", PayloadValueSchema::ToolTransportStatus),
    PayloadFieldSchema::required("effectStatus", PayloadValueSchema::ToolEffectStatus),
    PayloadFieldSchema::optional("executionOutcome", PayloadValueSchema::ToolExecutionOutcome),
    PayloadFieldSchema::optional(
        "auditPersistenceStatus",
        PayloadValueSchema::ToolAuditPersistenceStatus,
    ),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("startedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("dispatchedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("responseObservedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("finishedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("cancellationId", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::optional("remoteCancellationConfirmed", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("localWaitAborted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("dispatchObserved", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("reconciledAfterProcessRestart", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("preparedEventId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("dispatchProcessRisk", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("mayOutliveLocalProcess", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("effectMaySurviveLocalProcess", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("replayActionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("replayClaimId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("replayClaimOwnerGeneration", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("replayAuthorityBinding", PayloadValueSchema::OpaqueDigest),
];
const TOOL_DISPATCH_PREPARED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("receiptId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("requestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("sourceRunId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("manifestId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("toolName", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("requestDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("manifestContractDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("inputHash", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("inputLengthBytes", PayloadValueSchema::Count),
    PayloadFieldSchema::required("actionEffect", PayloadValueSchema::ToolActionEffect),
    PayloadFieldSchema::required(
        "idempotencyContract",
        PayloadValueSchema::ToolIdempotencyContract,
    ),
    // Optional only for decoding pre-contract prepared rows. Every new write
    // is required to carry and validate these fields in
    // `validate_tool_lifecycle_transition`.
    PayloadFieldSchema::optional("dispatchProcessRisk", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("mayOutliveLocalProcess", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("effectMaySurviveLocalProcess", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("replayActionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("replayClaimId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("replayClaimOwnerGeneration", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("replayAuthorityBinding", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
];
const TASK_CREATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("runId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("strategy", PayloadValueSchema::MetadataString),
];
const ROUTE_SELECTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("runId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("strategy", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("reason", PayloadValueSchema::RedactedString),
];
const CONTEXT_SELECTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("contextId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("sourceKind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("sourceLabel", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("evidenceId", PayloadValueSchema::MetadataStringOrNull),
];
const PROVIDER_SELECTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("provider", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("model", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("routeType", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "providerConfigGeneration",
        PayloadValueSchema::MetadataStringOrNull,
    ),
    PayloadFieldSchema::required("evidenceId", PayloadValueSchema::MetadataString),
];
const PLAN_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("runId", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::optional("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("revisionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("goal", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("confirmedAt", PayloadValueSchema::TimestampOrNull),
    PayloadFieldSchema::optional("reviewId", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::optional("reviewSummaryPresent", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional(
        "sourceEvidenceIds",
        PayloadValueSchema::MetadataStringArrayOrRedacted,
    ),
    PayloadFieldSchema::optional(
        "supersededByPlanId",
        PayloadValueSchema::MetadataStringOrNull,
    ),
    PayloadFieldSchema::optional("stepIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directLifeModelWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("memoryWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("externalWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
    PayloadFieldSchema::optional("evidenceId", PayloadValueSchema::MetadataStringOrNull),
];
const PLAN_REVIEWED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("reviewId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("planStatus", PayloadValueSchema::PlanStatus),
    PayloadFieldSchema::optional("basePlanRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("completedStepCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("skippedStepCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("blockedStepCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("proposalCreatedCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("observationUsedCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("unresolvedCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("recommendedNextActionCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("completionClaimed", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directLifeModelWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("memoryWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("externalWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const STEP_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("stepId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("index", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("title", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("description", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("kind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("basePlanRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("linkedActionIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "linkedObservationIds",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional("linkedProposalIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("blockerIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "linkedFinalDeliveryIds",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional("skipReason", PayloadValueSchema::RedactedStringOrNull),
    PayloadFieldSchema::optional("skipReasonPresent", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("policyDecisionId", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::optional("reason", PayloadValueSchema::RedactedStringOrNull),
    PayloadFieldSchema::optional("reasonCode", PayloadValueSchema::ReasonCodeOrNull),
    PayloadFieldSchema::optional(
        "evidenceIds",
        PayloadValueSchema::MetadataStringArrayOrRedacted,
    ),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directLifeModelWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("memoryWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("externalWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const ACTION_QUEUED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("actionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("actionType", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("target", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("policyDecisionId", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::optional("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("stepId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const ACTION_STARTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("actionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("startedAt", PayloadValueSchema::TimestampOrNull),
];
const ACTION_COMPLETED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("actionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("observationIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("stepId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const ACTION_FAILED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("actionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("retryable", PayloadValueSchema::Bool),
];
const ACTION_UPDATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("actionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
];
const OBSERVATION_CREATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("observationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("actionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("sourceKind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("sourceLabel", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("readExecution", PayloadValueSchema::ReadExecutionOrNull),
    PayloadFieldSchema::optional("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("stepId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("preview", PayloadValueSchema::RedactedString),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const BLOCKER_CREATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("blockerId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("reasonCode", PayloadValueSchema::ReasonCode),
    PayloadFieldSchema::optional("affectedActionId", PayloadValueSchema::MetadataStringOrNull),
    PayloadFieldSchema::optional("recoverable", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("stepId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("expectedRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("baseRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directLifeModelWrites", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("externalWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const PROPOSAL_CREATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("proposalId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("proposalType", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "evidenceIds",
        PayloadValueSchema::MetadataStringArrayOrRedacted,
    ),
    PayloadFieldSchema::optional("actionIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("planId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("planSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("stepId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("revision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("metadataSafe", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "childWorkflowProvenance",
        PayloadValueSchema::ChildWorkflowProvenance,
    ),
];
const PROPOSAL_DECISION_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("proposalId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
];
const MEMORY_MATERIALIZED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("memoryId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("proposalId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("materializedViewVersion", PayloadValueSchema::Count),
];
const MEMORY_ROLLED_BACK_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("memoryId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("proposalId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "rolledBackByEventId",
        PayloadValueSchema::MetadataStringOrNull,
    ),
];
const EFFECT_COMMITTED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("receiptId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("operationId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("assetId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("assetVersion", PayloadValueSchema::Count),
    PayloadFieldSchema::required("mutationKind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("payloadDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("outboxEventId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("projectionStatus", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("replayed", PayloadValueSchema::Bool),
];
const TERMINAL_OWNER_SUCCESSOR_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("causeKind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("causeRef", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("finalEventId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("ownerKind", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("ownerId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("beforeOwnerRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::required("afterOwnerRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::required("beforeOwnerDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required("afterOwnerDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::required(
        "localTransitionReceiptRef",
        PayloadValueSchema::MetadataString,
    ),
    PayloadFieldSchema::required(
        "localTransitionReceiptDigest",
        PayloadValueSchema::MetadataString,
    ),
];
const FINAL_DELIVERY_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::optional("deliveryId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("runId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("reasonCode", PayloadValueSchema::ReasonCode),
    PayloadFieldSchema::optional("completedActionCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("observationCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("proposalCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("blockerCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("pendingUserActionCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("toolCallCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("transcriptCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("durableChangeCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("directWritesExecuted", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("assistantMessageRef", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("assistantMessageDigest", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional(
        "assistantMessageOperationId",
        PayloadValueSchema::MetadataString,
    ),
    PayloadFieldSchema::optional("bodyStored", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("runtimeOwner", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("taskOwnerStatus", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("taskOwnerDigestVersion", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("taskOwnerRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("taskOwnerDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::optional("runOwnerStatus", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("runOwnerRevision", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("runOwnerDigest", PayloadValueSchema::OpaqueDigest),
    PayloadFieldSchema::optional(
        "providerInvocationStatus",
        PayloadValueSchema::MetadataString,
    ),
    PayloadFieldSchema::optional("modelInvoked", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("toolInvoked", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("routePath", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("strategyLabel", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("routeReasonCode", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("blockerRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("proposalRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("actionQueueRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "actionQueueOwnerDigests",
        PayloadValueSchema::OpaqueDigestArray,
    ),
    PayloadFieldSchema::optional("toolReceiptRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "toolTerminalEventRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional(
        "toolTerminalEventDigests",
        PayloadValueSchema::OpaqueDigestArray,
    ),
    PayloadFieldSchema::optional("transcriptRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "transcriptOwnerDigests",
        PayloadValueSchema::OpaqueDigestArray,
    ),
    PayloadFieldSchema::optional(
        "completedActionRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional("observationRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "pendingUserActionRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional("durableChangeRefs", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional(
        "durableChangeTypes",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional(
        "durableChangeProvenanceRefs",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional(
        "durableChangeTimestamps",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional(
        "durableChangeRollbackStates",
        PayloadValueSchema::MetadataStringArray,
    ),
    PayloadFieldSchema::optional("kernelEventCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("durableEventCount", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("requiresProvider", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("requiresToolLoop", PayloadValueSchema::Bool),
    PayloadFieldSchema::optional("replayEpochGeneration", PayloadValueSchema::Count),
    PayloadFieldSchema::optional("replayExecutionRef", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("errorBodyStored", PayloadValueSchema::Bool),
];
const DIAGNOSTIC_CREATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("gapId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("gapCode", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("evidenceId", PayloadValueSchema::MetadataStringOrNull),
];
const TASK_UPDATED_FIELDS: &[PayloadFieldSchema] = &[
    PayloadFieldSchema::required("taskSessionId", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::required("status", PayloadValueSchema::MetadataString),
    PayloadFieldSchema::optional("controls", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("actionIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("observationIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("blockerIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("proposalIds", PayloadValueSchema::MetadataStringArray),
    PayloadFieldSchema::optional("finalDeliveryId", PayloadValueSchema::MetadataStringOrNull),
];
#[cfg(test)]
const TEST_IMMUTABLE_FIELDS: &[PayloadFieldSchema] = &[PayloadFieldSchema::required(
    "status",
    PayloadValueSchema::MetadataString,
)];

fn durable_event_payload_schema(
    event_type: &str,
    version: i64,
) -> Option<DurableEventPayloadSchema> {
    if version != DURABLE_EVENT_PAYLOAD_VERSION {
        return None;
    }
    let registered_object_type = DURABLE_EVENT_REGISTRY
        .iter()
        .find_map(|(registered, object_type)| (*registered == event_type).then_some(*object_type))
        .or_else(|| (cfg!(test) && event_type == "test.immutable_fact").then_some("test_fact"))?;
    let schema = match event_type {
        "turn_started" => DurableEventPayloadSchema {
            object_type: "turn",
            fields: TURN_STARTED_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("started"),
        },
        "cancel_requested" => DurableEventPayloadSchema {
            object_type: "turn",
            fields: CANCEL_REQUESTED_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("cancel_requested"),
        },
        "local_aborted" => DurableEventPayloadSchema {
            object_type: "turn",
            fields: TURN_TERMINAL_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("local_aborted"),
        },
        "interrupted" => DurableEventPayloadSchema {
            object_type: "turn",
            fields: TURN_TERMINAL_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("interrupted"),
        },
        "failed" => DurableEventPayloadSchema {
            object_type: "turn",
            fields: TURN_TERMINAL_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("failed"),
        },
        "provider.started" => DurableEventPayloadSchema {
            object_type: "provider_request",
            fields: PROVIDER_STARTED_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("started"),
        },
        "provider.completed" => DurableEventPayloadSchema {
            object_type: "provider_request",
            fields: PROVIDER_COMPLETED_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("completed"),
        },
        "provider.failed" => DurableEventPayloadSchema {
            object_type: "provider_request",
            fields: PROVIDER_COMPLETED_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("failed"),
        },
        "provider.remote_unknown" => DurableEventPayloadSchema {
            object_type: "provider_request",
            fields: PROVIDER_REMOTE_UNKNOWN_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("remote_unknown"),
        },
        "provider.receipt_state_failed" => DurableEventPayloadSchema {
            object_type: "provider_attempt_state",
            fields: PROVIDER_ATTEMPT_FAILURE_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("failed"),
        },
        "tool.dispatch_prepared" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_DISPATCH_PREPARED_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("prepared"),
        },
        "tool.not_dispatched" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("not_dispatched"),
        },
        "tool.dispatch_ambiguous" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("dispatch_ambiguous"),
        },
        "tool.started" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("started"),
        },
        "tool.completed" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("completed"),
        },
        "tool.failed" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("failed"),
        },
        "tool.effect_unknown" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("effect_unknown"),
        },
        "tool.local_aborted" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("local_aborted"),
        },
        "tool.remote_unknown" => DurableEventPayloadSchema {
            object_type: "tool_execution_receipt",
            fields: TOOL_RECEIPT_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("remote_unknown"),
        },
        "task.created" => DurableEventPayloadSchema {
            object_type: "task",
            fields: TASK_CREATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "route.selected" => DurableEventPayloadSchema {
            object_type: "route",
            fields: ROUTE_SELECTED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "context.selected" => DurableEventPayloadSchema {
            object_type: "context",
            fields: CONTEXT_SELECTED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "provider.selected" => DurableEventPayloadSchema {
            object_type: "provider",
            fields: PROVIDER_SELECTED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "plan.created" | "plan.updated" | "plan.confirmed" => DurableEventPayloadSchema {
            object_type: "plan",
            fields: PLAN_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "plan.reviewed" => DurableEventPayloadSchema {
            object_type: "plan_review",
            fields: PLAN_REVIEWED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "step.created" | "step.updated" | "step.cancelled" | "step.skipped" => {
            DurableEventPayloadSchema {
                object_type: "step",
                fields: STEP_FIELDS,
                reject_unknown_fields: false,
                expected_status: None,
            }
        }
        "action.queued" => DurableEventPayloadSchema {
            object_type: "action",
            fields: ACTION_QUEUED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "action.started" => DurableEventPayloadSchema {
            object_type: "action",
            fields: ACTION_STARTED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "action.completed" => DurableEventPayloadSchema {
            object_type: "action",
            fields: ACTION_COMPLETED_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("succeeded"),
        },
        "action.failed" => DurableEventPayloadSchema {
            object_type: "action",
            fields: ACTION_FAILED_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("failed"),
        },
        "action.updated" => DurableEventPayloadSchema {
            object_type: "action",
            fields: ACTION_UPDATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "observation.created" => DurableEventPayloadSchema {
            object_type: "observation",
            fields: OBSERVATION_CREATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "blocker.created" => DurableEventPayloadSchema {
            object_type: "blocker",
            fields: BLOCKER_CREATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "proposal.created" => DurableEventPayloadSchema {
            object_type: "proposal",
            fields: PROPOSAL_CREATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "proposal.accepted" => DurableEventPayloadSchema {
            object_type: "proposal",
            fields: PROPOSAL_DECISION_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("accepted"),
        },
        "proposal.rejected" => DurableEventPayloadSchema {
            object_type: "proposal",
            fields: PROPOSAL_DECISION_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("rejected"),
        },
        "proposal.deferred" => DurableEventPayloadSchema {
            object_type: "proposal",
            fields: PROPOSAL_DECISION_FIELDS,
            reject_unknown_fields: false,
            expected_status: Some("deferred"),
        },
        "proposal.updated" => DurableEventPayloadSchema {
            object_type: "proposal",
            fields: PROPOSAL_DECISION_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "memory.materialized" => DurableEventPayloadSchema {
            object_type: "memory",
            fields: MEMORY_MATERIALIZED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "memory.rolled_back" => DurableEventPayloadSchema {
            object_type: "memory",
            fields: MEMORY_ROLLED_BACK_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "effect_committed" => DurableEventPayloadSchema {
            object_type: "state_effect",
            fields: EFFECT_COMMITTED_FIELDS,
            reject_unknown_fields: true,
            expected_status: Some("committed"),
        },
        "terminal_owner.successor_confirmed" => DurableEventPayloadSchema {
            object_type: "terminal_owner_successor",
            fields: TERMINAL_OWNER_SUCCESSOR_FIELDS,
            reject_unknown_fields: true,
            expected_status: None,
        },
        "final_delivery.created" => DurableEventPayloadSchema {
            object_type: "final_delivery",
            fields: FINAL_DELIVERY_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "diagnostic.created" => DurableEventPayloadSchema {
            object_type: "diagnostic",
            fields: DIAGNOSTIC_CREATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        "task.updated" => DurableEventPayloadSchema {
            object_type: "task",
            fields: TASK_UPDATED_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        #[cfg(test)]
        "test.immutable_fact" => DurableEventPayloadSchema {
            object_type: "test_fact",
            fields: TEST_IMMUTABLE_FIELDS,
            reject_unknown_fields: false,
            expected_status: None,
        },
        _ => return None,
    };
    debug_assert_eq!(schema.object_type, registered_object_type);
    Some(schema)
}

#[cfg(test)]
fn normalize_durable_event_payload(
    event_type: &str,
    object_type: &str,
    payload: &Value,
    origin: PayloadNormalizationOrigin,
) -> Result<Value> {
    let key = MainChatEventDigestKey::from_key_material(&[0x5a; 32])?;
    normalize_durable_event_payload_with_key(event_type, object_type, payload, origin, &key)
}

fn normalize_durable_event_payload_with_key(
    event_type: &str,
    object_type: &str,
    payload: &Value,
    origin: PayloadNormalizationOrigin,
    digest_key: &MainChatEventDigestKey,
) -> Result<Value> {
    let schema = durable_event_payload_schema(event_type, DURABLE_EVENT_PAYLOAD_VERSION)
        .ok_or_else(|| payload_schema_fault(event_type, "unsupported_event_type"))?;
    if object_type != schema.object_type {
        return Err(payload_schema_fault(
            event_type,
            &format!("object_type_mismatch:{object_type}:{}", schema.object_type),
        ));
    }
    let object = payload
        .as_object()
        .ok_or_else(|| payload_schema_fault(event_type, "payload_not_object"))?;
    let mut normalized = serde_json::Map::new();
    let mut unrecognized = UnknownFieldAccumulator::default();
    for (key, value) in object {
        if key == UNRECOGNIZED_FIELDS_RECEIPT
            && origin == PayloadNormalizationOrigin::Legacy
            && (is_canonical_unrecognized_fields_receipt(value)
                || is_legacy_unrecognized_fields_receipt(value))
        {
            if !schema.reject_unknown_fields {
                unrecognized.observe_legacy_receipt(value, digest_key)?;
            }
            continue;
        }
        let Some(field) = schema.fields.iter().find(|field| field.key == key) else {
            if schema.reject_unknown_fields {
                if origin == PayloadNormalizationOrigin::Legacy {
                    // Older generic minimizers could leave a receipt or a
                    // schema-unrelated key behind. A versioned migration may
                    // discard that legacy residue, but a new lifecycle write
                    // must match the exact provider/tool contract.
                    continue;
                }
                return Err(payload_schema_fault(
                    event_type,
                    &format!("unexpected_payload_field:{key}"),
                ));
            }
            unrecognized.observe(key, value, digest_key)?;
            continue;
        };
        normalized.insert(
            key.clone(),
            normalize_schema_field_value(event_type, field, value, Some(digest_key))?,
        );
    }
    for field in schema.fields.iter().filter(|field| field.required) {
        if !normalized.contains_key(field.key) {
            return Err(payload_schema_fault(
                event_type,
                &format!("required_field_missing:{}", field.key),
            ));
        }
    }
    if let Some(expected_status) = schema.expected_status {
        if origin == PayloadNormalizationOrigin::Legacy
            && !normalized.contains_key("status")
            && schema.fields.iter().any(|field| field.key == "status")
        {
            // Pre-v6 rows could encode a fixed transition solely in the event
            // type. The migration may materialize that redundant typed status;
            // it never rewrites a contradictory value.
            normalized.insert("status".into(), Value::String(expected_status.into()));
        }
        if normalized.get("status").and_then(Value::as_str) != Some(expected_status) {
            return Err(payload_schema_fault(
                event_type,
                &format!("status_mismatch:{expected_status}"),
            ));
        }
    }
    normalize_event_status_contract(event_type, &mut normalized, origin)?;
    if let Some(receipt) = unrecognized.into_receipt(digest_key)? {
        normalized.insert(UNRECOGNIZED_FIELDS_RECEIPT.into(), receipt);
    }
    Ok(Value::Object(normalized))
}

fn validate_canonical_durable_event_payload(
    event_type: &str,
    object_type: &str,
    payload: &Value,
) -> Result<()> {
    let schema = durable_event_payload_schema(event_type, DURABLE_EVENT_PAYLOAD_VERSION)
        .ok_or_else(|| payload_schema_fault(event_type, "unsupported_event_type"))?;
    if object_type != schema.object_type {
        return Err(payload_schema_fault(event_type, "object_type_mismatch"));
    }
    let object = payload
        .as_object()
        .ok_or_else(|| payload_schema_fault(event_type, "payload_not_object"))?;
    for (key, value) in object {
        if key == UNRECOGNIZED_FIELDS_RECEIPT {
            if schema.reject_unknown_fields || !is_canonical_unrecognized_fields_receipt(value) {
                return Err(payload_schema_fault(
                    event_type,
                    "invalid_unrecognized_fields_receipt",
                ));
            }
            continue;
        }
        let field = schema
            .fields
            .iter()
            .find(|field| field.key == key)
            .ok_or_else(|| payload_schema_fault(event_type, "noncanonical_field"))?;
        validate_canonical_schema_field_value(event_type, field, value)?;
    }
    for field in schema.fields.iter().filter(|field| field.required) {
        if !object.contains_key(field.key) {
            return Err(payload_schema_fault(
                event_type,
                &format!("required_field_missing:{}", field.key),
            ));
        }
    }
    if let Some(expected_status) = schema.expected_status {
        if object.get("status").and_then(Value::as_str) != Some(expected_status) {
            return Err(payload_schema_fault(event_type, "status_mismatch"));
        }
    }
    validate_event_status_contract(event_type, object)?;
    Ok(())
}

fn event_status_contract(event_type: &str) -> Option<&'static [&'static str]> {
    match event_type {
        "plan.created" => Some(&["draft"]),
        "plan.confirmed" => Some(&["finalized"]),
        "plan.updated" => Some(&[
            "draft",
            "finalized",
            "in_progress",
            "completed",
            "cancelled",
        ]),
        "step.created" => Some(&["planned"]),
        "step.updated" => Some(&[
            "planned",
            "skipped",
            "blocked",
            "requiresproposal",
            "requiresconfirmation",
            "executed",
            "cancelled",
        ]),
        "step.cancelled" => Some(&["cancelled"]),
        "step.skipped" => Some(&["skipped"]),
        "action.queued" => Some(&["queued"]),
        "action.started" => Some(&["running"]),
        "action.updated" => Some(&[
            "queued",
            "running",
            "blocked",
            "needs_confirmation",
            "waiting_permission",
            "cancelled",
            "local_aborted",
            "remote_unknown",
            "unknown",
        ]),
        "proposal.updated" => Some(&[
            "draft",
            "pending_review",
            "accepted",
            "rejected",
            "deferred",
            "rolled_back",
            "stale",
        ]),
        "final_delivery.created" => Some(&[
            "completed",
            "completed_with_pending_items",
            "blocked",
            "failed",
            "cancelled",
            "interrupted",
        ]),
        "task.updated" => Some(&[
            "classifying",
            "answering",
            "planning",
            "waiting_for_user",
            "queued",
            "executing",
            "observing",
            "synthesizing",
            "proposal_pending",
            "blocked",
            "failed",
            "completed",
            "cancelled",
        ]),
        _ => None,
    }
}

fn normalize_event_status_contract(
    event_type: &str,
    payload: &mut serde_json::Map<String, Value>,
    origin: PayloadNormalizationOrigin,
) -> Result<()> {
    let Some(allowed) = event_status_contract(event_type) else {
        return Ok(());
    };
    if !payload.contains_key("status")
        && origin == PayloadNormalizationOrigin::Legacy
        && allowed.len() == 1
    {
        payload.insert("status".into(), Value::String(allowed[0].into()));
    }
    validate_event_status_contract(event_type, payload)
}

fn validate_event_status_contract(
    event_type: &str,
    payload: &serde_json::Map<String, Value>,
) -> Result<()> {
    let Some(allowed) = event_status_contract(event_type) else {
        return Ok(());
    };
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| payload_schema_fault(event_type, "status_missing"))?;
    if allowed.contains(&status) {
        Ok(())
    } else {
        Err(payload_schema_fault(event_type, "status_not_allowed"))
    }
}

fn normalize_schema_field_value(
    event_type: &str,
    field: &PayloadFieldSchema,
    value: &Value,
    digest_key: Option<&MainChatEventDigestKey>,
) -> Result<Value> {
    use PayloadValueSchema as Schema;
    let invalid = || {
        payload_schema_fault(
            event_type,
            &format!("invalid_field_type_or_bound:{}", field.key),
        )
    };
    match field.value_schema {
        Schema::MetadataString => bounded_metadata_string(value).ok_or_else(invalid),
        Schema::MetadataStringOrNull => {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                bounded_metadata_string(value).ok_or_else(invalid)
            }
        }
        Schema::PlanStatus => bounded_enum_string(
            value,
            &[
                "draft",
                "finalized",
                "in_progress",
                "completed",
                "cancelled",
            ],
        )
        .ok_or_else(invalid),
        Schema::ToolActionEffect => bounded_enum_string(
            value,
            &[
                "read_only",
                "local_mutation",
                "external_mutation",
                "proposal_only",
                "unknown",
            ],
        )
        .ok_or_else(invalid),
        Schema::ToolIdempotencyContract => {
            bounded_enum_string(value, &["unspecified", "non_idempotent", "idempotent"])
                .ok_or_else(invalid)
        }
        Schema::ToolDispatchKind => bounded_enum_string(
            value,
            &[
                "not_attempted",
                "local",
                "network",
                "mcp_stdio",
                "a2a",
                "simulated",
                "unknown",
            ],
        )
        .ok_or_else(invalid),
        Schema::ToolTransportStatus => bounded_enum_string(
            value,
            &[
                "not_attempted",
                "dispatched",
                "response_observed",
                "local_aborted",
                "remote_unknown",
            ],
        )
        .ok_or_else(invalid),
        Schema::ToolEffectStatus => {
            bounded_enum_string(value, &["not_attempted", "confirmed", "unknown"])
                .ok_or_else(invalid)
        }
        Schema::ToolExecutionOutcome => {
            bounded_enum_string(value, &["not_observed", "succeeded", "failed", "unknown"])
                .ok_or_else(invalid)
        }
        Schema::ToolAuditPersistenceStatus => bounded_enum_string(
            value,
            &["not_required", "pending", "committed", "failed", "unknown"],
        )
        .ok_or_else(invalid),
        Schema::RedactedString => bounded_redacted_string(value, digest_key).ok_or_else(invalid),
        Schema::RedactedStringOrNull => {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                bounded_redacted_string(value, digest_key).ok_or_else(invalid)
            }
        }
        Schema::ReasonCode => normalize_reason_code(value, digest_key).ok_or_else(invalid),
        Schema::ReasonCodeOrNull => {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                normalize_reason_code(value, digest_key).ok_or_else(invalid)
            }
        }
        Schema::OpaqueDigest => {
            let digest = value.as_str().ok_or_else(invalid)?;
            if digest.chars().count() > MAX_METADATA_STRING_CHARS {
                return Err(invalid());
            }
            if is_exact_metadata_digest(digest) {
                Ok(value.clone())
            } else {
                Err(invalid())
            }
        }
        Schema::RedactedDigestOrNull => {
            if value.is_null() {
                return Ok(Value::Null);
            }
            if is_canonical_redacted_string(value) {
                return Ok(value.clone());
            }
            if is_legacy_type_only_redaction(value, Some("string"))
                || is_legacy_unkeyed_redacted_event_value(value, Some("string"))
            {
                return digest_key
                    .map(|key| legacy_redacted_event_value(value, key))
                    .ok_or_else(invalid);
            }
            let digest = value.as_str().ok_or_else(invalid)?;
            if digest.chars().count() > MAX_METADATA_STRING_CHARS {
                return Err(invalid());
            }
            digest_key
                .map(|key| redacted_event_value(value, key))
                .ok_or_else(invalid)
        }
        Schema::TimestampOrNull => {
            if value.is_null() {
                return Ok(Value::Null);
            }
            let timestamp = value.as_str().ok_or_else(invalid)?;
            if timestamp.chars().count() > 64 || DateTime::parse_from_rfc3339(timestamp).is_err() {
                return Err(invalid());
            }
            Ok(value.clone())
        }
        Schema::Bool => value.as_bool().map(|_| value.clone()).ok_or_else(invalid),
        Schema::Count => value
            .as_u64()
            .filter(|count| *count <= MAX_EVENT_COUNT)
            .map(|_| value.clone())
            .ok_or_else(invalid),
        Schema::MetadataStringArray => normalize_metadata_string_array(value).ok_or_else(invalid),
        Schema::ContextReferenceArray => {
            normalize_context_reference_array(value).ok_or_else(invalid)
        }
        Schema::OpaqueDigestArray => {
            let values = value.as_array().ok_or_else(invalid)?;
            if values.len() > MAX_METADATA_ARRAY_ITEMS {
                return Err(invalid());
            }
            values
                .iter()
                .all(|value| {
                    value.as_str().is_some_and(|digest| {
                        digest.chars().count() <= MAX_METADATA_STRING_CHARS
                            && is_exact_metadata_digest(digest)
                    })
                })
                .then(|| value.clone())
                .ok_or_else(invalid)
        }
        Schema::MetadataStringArrayOrRedacted => {
            normalize_metadata_string_array_or_redacted(value, digest_key).ok_or_else(invalid)
        }
        Schema::ReadExecutionOrNull => {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                normalize_read_execution(value, digest_key).ok_or_else(invalid)
            }
        }
        Schema::ChildWorkflowProvenance => {
            normalize_child_workflow_provenance(value, digest_key).ok_or_else(invalid)
        }
    }
}

fn validate_canonical_schema_field_value(
    event_type: &str,
    field: &PayloadFieldSchema,
    value: &Value,
) -> Result<()> {
    let normalized = normalize_schema_field_value(event_type, field, value, None)?;
    if normalized == *value {
        Ok(())
    } else {
        Err(payload_schema_fault(event_type, "field_not_canonical"))
    }
}

fn bounded_metadata_string(value: &Value) -> Option<Value> {
    let value = value.as_str()?;
    (!value.is_empty()
        && value.chars().count() <= MAX_METADATA_STRING_CHARS
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '-' | ':' | '/' | '+' | '@')
        }))
    .then(|| Value::String(value.to_string()))
}

fn bounded_enum_string(value: &Value, allowed: &[&str]) -> Option<Value> {
    let value = value.as_str()?;
    allowed
        .contains(&value)
        .then(|| Value::String(value.to_string()))
}

fn bounded_redacted_string(
    value: &Value,
    digest_key: Option<&MainChatEventDigestKey>,
) -> Option<Value> {
    if is_canonical_redacted_string(value) {
        return Some(value.clone());
    }
    if is_legacy_type_only_redaction(value, Some("string"))
        || is_legacy_unkeyed_redacted_event_value(value, Some("string"))
    {
        return digest_key.map(|key| legacy_redacted_event_value(value, key));
    }
    let value = value.as_str()?;
    (value.chars().count() <= MAX_REDACTED_STRING_CHARS)
        .then(|| digest_key.map(|key| redacted_event_value(&Value::String(value.to_string()), key)))
        .flatten()
}

fn normalize_reason_code(
    value: &Value,
    digest_key: Option<&MainChatEventDigestKey>,
) -> Option<Value> {
    if is_canonical_redacted_string(value) {
        return Some(value.clone());
    }
    if is_legacy_type_only_redaction(value, Some("string"))
        || is_legacy_unkeyed_redacted_event_value(value, Some("string"))
    {
        return digest_key.map(|key| legacy_redacted_event_value(value, key));
    }
    let reason = value.as_str()?;
    if is_registered_event_reason_code(reason) {
        Some(Value::String(reason.to_string()))
    } else if reason.chars().count() <= MAX_REDACTED_STRING_CHARS {
        digest_key.map(|key| redacted_event_value(value, key))
    } else {
        None
    }
}

fn is_registered_event_reason_code(value: &str) -> bool {
    matches!(
        value,
        "cancel_without_canonical_effect"
            | "cancel_after_canonical_commit"
            | "cancel_with_canonical_commit_unknown"
            | "kernel_failed_before_provider_terminal_observed"
            | "provider_attempt_state_invalid"
            | "stale_plan_revision"
            | "invalid_plan_step"
            | "projection_summary_unavailable"
            | "canonical_outbox_reference_missing"
            | "projection_delivery_failed"
            | "policy_blocked"
            | "permission_required"
            | "replay_terminal_committed"
            | "replay_terminal_error"
            | "unknown"
    )
}

fn normalize_metadata_string_array(value: &Value) -> Option<Value> {
    let values = value.as_array()?;
    if values.len() > MAX_METADATA_ARRAY_ITEMS {
        return None;
    }
    values
        .iter()
        .map(bounded_metadata_string)
        .collect::<Option<Vec<_>>>()
        .map(Value::Array)
}

fn normalize_context_reference_array(value: &Value) -> Option<Value> {
    let values = value.as_array()?;
    if values.len() > MAX_METADATA_ARRAY_ITEMS {
        return None;
    }
    values
        .iter()
        .map(|value| {
            let raw = value.as_str()?;
            if raw.starts_with("websearch:") {
                openlife_core::web_search::is_canonical_web_search_context_ref(raw)
                    .then(|| Value::String(raw.to_string()))
            } else if raw.starts_with("resource:") {
                openlife_core::resource_selection::is_canonical_resource_context_ref(raw)
                    .then(|| Value::String(raw.to_string()))
            } else {
                bounded_metadata_string(value)
            }
        })
        .collect::<Option<Vec<_>>>()
        .map(Value::Array)
}

fn normalize_metadata_string_array_or_redacted(
    value: &Value,
    digest_key: Option<&MainChatEventDigestKey>,
) -> Option<Value> {
    if is_canonical_redacted_event_value(value)
        && value.get("valueType").and_then(Value::as_str) == Some("array")
    {
        return Some(value.clone());
    }
    if is_legacy_type_only_redaction(value, Some("array"))
        || is_legacy_unkeyed_redacted_event_value(value, Some("array"))
    {
        return digest_key.map(|key| legacy_redacted_event_value(value, key));
    }
    let values = value.as_array()?;
    if values.len() > MAX_METADATA_ARRAY_ITEMS
        || values.iter().any(|value| {
            value.as_str().map_or(true, |value| {
                value.chars().count() > MAX_REDACTED_STRING_CHARS
            })
        })
    {
        return None;
    }
    normalize_metadata_string_array(value)
        .or_else(|| digest_key.map(|key| redacted_event_value(value, key)))
}

fn normalize_read_execution(
    value: &Value,
    digest_key: Option<&MainChatEventDigestKey>,
) -> Option<Value> {
    const FIELDS: &[PayloadFieldSchema] = &[
        PayloadFieldSchema::required("kind", PayloadValueSchema::MetadataString),
        PayloadFieldSchema::required("sourceKind", PayloadValueSchema::MetadataString),
        PayloadFieldSchema::required("sourceLabel", PayloadValueSchema::RedactedString),
        PayloadFieldSchema::required("target", PayloadValueSchema::RedactedString),
        PayloadFieldSchema::required("realReadOnlyExecution", PayloadValueSchema::Bool),
        PayloadFieldSchema::required("fixtureBacked", PayloadValueSchema::Bool),
        PayloadFieldSchema::required("networkReadAttempted", PayloadValueSchema::Bool),
        PayloadFieldSchema::required("directWritesExecuted", PayloadValueSchema::Bool),
    ];
    normalize_nested_schema_object("readExecution", value, FIELDS, digest_key)
}

fn normalize_child_workflow_provenance(
    value: &Value,
    digest_key: Option<&MainChatEventDigestKey>,
) -> Option<Value> {
    const FIELDS: &[PayloadFieldSchema] = &[
        PayloadFieldSchema::required("kind", PayloadValueSchema::MetadataString),
        PayloadFieldSchema::required("id", PayloadValueSchema::MetadataString),
        PayloadFieldSchema::required("sourceRunId", PayloadValueSchema::MetadataString),
        PayloadFieldSchema::required("eventTaskBoundToSourceRun", PayloadValueSchema::Bool),
    ];
    normalize_nested_schema_object("childWorkflowProvenance", value, FIELDS, digest_key)
}

fn normalize_nested_schema_object(
    label: &str,
    value: &Value,
    fields: &[PayloadFieldSchema],
    digest_key: Option<&MainChatEventDigestKey>,
) -> Option<Value> {
    let object = value.as_object()?;
    let mut normalized = serde_json::Map::new();
    let mut unrecognized = UnknownFieldAccumulator::default();
    let mut existing_unrecognized_receipt = None;
    for (field_key, value) in object {
        if field_key == UNRECOGNIZED_FIELDS_RECEIPT
            && (is_canonical_unrecognized_fields_receipt(value)
                || is_legacy_unrecognized_fields_receipt(value))
        {
            if is_canonical_unrecognized_fields_receipt(value) {
                existing_unrecognized_receipt = Some(value.clone());
            } else {
                unrecognized
                    .observe_legacy_receipt(value, digest_key?)
                    .ok()?;
            }
            continue;
        }
        let Some(field) = fields.iter().find(|field| field.key == field_key) else {
            let event_digest_key = digest_key?;
            unrecognized
                .observe(field_key, value, event_digest_key)
                .ok()?;
            continue;
        };
        normalized.insert(
            field_key.clone(),
            normalize_schema_field_value(label, field, value, digest_key).ok()?,
        );
    }
    if fields
        .iter()
        .filter(|field| field.required)
        .any(|field| !normalized.contains_key(field.key))
    {
        return None;
    }
    if unrecognized.has_fields() {
        let receipt = unrecognized.into_receipt(digest_key?).ok()??;
        normalized.insert(UNRECOGNIZED_FIELDS_RECEIPT.into(), receipt);
    } else if let Some(receipt) = existing_unrecognized_receipt {
        normalized.insert(UNRECOGNIZED_FIELDS_RECEIPT.into(), receipt);
    }
    Some(Value::Object(normalized))
}

fn payload_schema_fault(event_type: &str, reason: &str) -> anyhow::Error {
    MainChatAgentEventStoreFault::PayloadSchemaConflict {
        event_type: event_type.to_string(),
        reason: reason.to_string(),
    }
    .into()
}

fn bind_event_digest_key(conn: &Connection, digest_key: &MainChatEventDigestKey) -> Result<()> {
    const METADATA_KEY: &str = "event_digest_key_verifier_v1";
    let expected = hmac_sha256_digest(
        digest_key,
        "event_store_key_verifier",
        b"openlife.main_chat_agent_event_store.v1",
    );
    let existing = conn
        .query_row(
            "SELECT value FROM main_chat_agent_event_store_metadata WHERE key = ?1",
            [METADATA_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    match existing {
        Some(existing) if existing != expected => {
            anyhow::bail!("main_chat_event_digest_key_mismatch")
        }
        Some(_) => Ok(()),
        None => {
            conn.execute(
                "INSERT INTO main_chat_agent_event_store_metadata(key, value) VALUES (?1, ?2)",
                params![METADATA_KEY, expected],
            )?;
            Ok(())
        }
    }
}

fn value_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn hmac_sha256_digest(key: &MainChatEventDigestKey, domain: &str, value: &[u8]) -> String {
    const BLOCK_BYTES: usize = 64;
    let mut key_block = [0_u8; BLOCK_BYTES];
    key_block[..key.0.len()].copy_from_slice(&key.0);
    let mut inner_pad = [0_u8; BLOCK_BYTES];
    let mut outer_pad = [0_u8; BLOCK_BYTES];
    for index in 0..BLOCK_BYTES {
        inner_pad[index] = key_block[index] ^ 0x36;
        outer_pad[index] = key_block[index] ^ 0x5c;
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(b"openlife.main_chat_event_hmac.v1\0");
    inner.update((domain.len() as u64).to_be_bytes());
    inner.update(domain.as_bytes());
    inner.update((value.len() as u64).to_be_bytes());
    inner.update(value);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    format!("hmac-sha256:{:x}", outer.finalize())
}

#[derive(Default)]
struct UnknownFieldAccumulator {
    field_count: usize,
    byte_count: usize,
    type_counts: std::collections::BTreeMap<&'static str, usize>,
    fact_digests: Vec<String>,
    legacy: bool,
}

impl UnknownFieldAccumulator {
    fn has_fields(&self) -> bool {
        self.field_count > 0
    }

    fn observe(
        &mut self,
        field_name: &str,
        value: &Value,
        key: &MainChatEventDigestKey,
    ) -> Result<()> {
        if self.field_count >= MAX_UNRECOGNIZED_FIELD_COUNT {
            anyhow::bail!("event_unknown_field_limit_exceeded");
        }
        let encoded = serde_json::to_vec(&(field_name, value))?;
        self.byte_count = self
            .byte_count
            .checked_add(encoded.len())
            .context("event_unknown_field_byte_count_overflow")?;
        if self.byte_count > MAX_REDACTED_VALUE_BYTES {
            anyhow::bail!("event_unknown_field_bytes_exceeded");
        }
        self.field_count += 1;
        *self.type_counts.entry(value_type(value)).or_default() += 1;
        self.fact_digests
            .push(hmac_sha256_digest(key, "unknown_field", &encoded));
        Ok(())
    }

    fn observe_legacy_receipt(
        &mut self,
        receipt: &Value,
        key: &MainChatEventDigestKey,
    ) -> Result<()> {
        let count = receipt
            .get("fieldCount")
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .filter(|count| *count > 0 && *count <= MAX_UNRECOGNIZED_FIELD_COUNT)
            .context("legacy_unknown_field_count_invalid")?;
        let encoded = serde_json::to_vec(receipt)?;
        if encoded.len() > MAX_REDACTED_VALUE_BYTES {
            anyhow::bail!("legacy_unknown_field_receipt_too_large");
        }
        self.field_count = count;
        self.byte_count = encoded.len();
        self.type_counts.insert("unknown", count);
        self.fact_digests.push(hmac_sha256_digest(
            key,
            "legacy_unknown_fields_receipt",
            &encoded,
        ));
        self.legacy = true;
        Ok(())
    }

    fn into_receipt(mut self, key: &MainChatEventDigestKey) -> Result<Option<Value>> {
        if !self.has_fields() {
            return Ok(None);
        }
        self.fact_digests.sort();
        let aggregate = serde_json::to_vec(&self.fact_digests)?;
        let type_counts = self
            .type_counts
            .into_iter()
            .map(|(kind, count)| (kind.to_string(), json!(count)))
            .collect::<serde_json::Map<_, _>>();
        Ok(Some(json!({
            "redacted": true,
            "valueType": "object",
            "fieldCount": self.field_count,
            "byteCount": self.byte_count,
            "digest": hmac_sha256_digest(key, "unknown_fields_aggregate", &aggregate),
            "digestScope": if self.legacy {
                "keyed_legacy_unknown_fields_v1"
            } else {
                "keyed_unknown_fields_v1"
            },
            "typeCounts": Value::Object(type_counts),
        })))
    }
}

fn is_canonical_redacted_event_value(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 5
        && object.get("redacted").and_then(Value::as_bool) == Some(true)
        && object
            .get("valueType")
            .and_then(Value::as_str)
            .is_some_and(|value_type| {
                matches!(
                    value_type,
                    "null" | "bool" | "number" | "string" | "array" | "object"
                )
            })
        && object
            .get("byteCount")
            .and_then(Value::as_u64)
            .is_some_and(|count| count <= MAX_REDACTED_VALUE_BYTES as u64)
        && object
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(is_exact_hmac_digest)
        && object
            .get("digestScope")
            .and_then(Value::as_str)
            .is_some_and(|scope| {
                matches!(scope, "keyed_input_value_v1" | "keyed_legacy_receipt_v1")
            })
}

fn is_legacy_unkeyed_redacted_event_value(value: &Value, expected_type: Option<&str>) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 5
        && object.get("redacted").and_then(Value::as_bool) == Some(true)
        && object
            .get("valueType")
            .and_then(Value::as_str)
            .is_some_and(|actual| expected_type.map_or(true, |expected| actual == expected))
        && object
            .get("byteCount")
            .and_then(Value::as_u64)
            .is_some_and(|count| count <= MAX_REDACTED_VALUE_BYTES as u64)
        && object
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(is_exact_metadata_digest)
        && object
            .get("digestScope")
            .and_then(Value::as_str)
            .is_some_and(|scope| matches!(scope, "input_value" | "legacy_receipt"))
}

fn is_legacy_type_only_redaction(value: &Value, expected_type: Option<&str>) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 2
        && object.get("redacted").and_then(Value::as_bool) == Some(true)
        && object
            .get("valueType")
            .and_then(Value::as_str)
            .is_some_and(|value_type| {
                expected_type.map_or_else(
                    || {
                        matches!(
                            value_type,
                            "null" | "bool" | "number" | "string" | "array" | "object"
                        )
                    },
                    |expected| value_type == expected,
                )
            })
}

fn is_canonical_redacted_string(value: &Value) -> bool {
    is_canonical_redacted_event_value(value)
        && value.get("valueType").and_then(Value::as_str) == Some("string")
}

fn is_canonical_unrecognized_fields_receipt(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let field_count = object.get("fieldCount").and_then(Value::as_u64);
    let type_count_sum = object
        .get("typeCounts")
        .and_then(Value::as_object)
        .filter(|counts| {
            counts.keys().all(|kind| {
                matches!(
                    kind.as_str(),
                    "null" | "bool" | "number" | "string" | "array" | "object" | "unknown"
                )
            })
        })
        .and_then(|counts| {
            counts
                .values()
                .try_fold(0_u64, |total, count| total.checked_add(count.as_u64()?))
        });
    object.len() == 7
        && object.get("redacted").and_then(Value::as_bool) == Some(true)
        && object.get("valueType").and_then(Value::as_str) == Some("object")
        && field_count
            .is_some_and(|count| count > 0 && count <= MAX_UNRECOGNIZED_FIELD_COUNT as u64)
        && type_count_sum == field_count
        && object
            .get("byteCount")
            .and_then(Value::as_u64)
            .is_some_and(|count| count <= MAX_REDACTED_VALUE_BYTES as u64)
        && object
            .get("digest")
            .and_then(Value::as_str)
            .is_some_and(is_exact_hmac_digest)
        && object
            .get("digestScope")
            .and_then(Value::as_str)
            .is_some_and(|scope| {
                matches!(
                    scope,
                    "keyed_unknown_fields_v1" | "keyed_legacy_unknown_fields_v1"
                )
            })
}

fn is_legacy_unrecognized_fields_receipt(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 3
        && object.get("redacted").and_then(Value::as_bool) == Some(true)
        && object.get("valueType").and_then(Value::as_str) == Some("object")
        && object
            .get("fieldCount")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0 && count <= MAX_UNRECOGNIZED_FIELD_COUNT as u64)
}

fn is_exact_hmac_digest(value: &str) -> bool {
    value.strip_prefix("hmac-sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    })
}

fn is_exact_metadata_digest(value: &str) -> bool {
    fn is_sha256_hex(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    }

    if let Some(hash) = value.strip_prefix("sha256:") {
        return is_sha256_hex(hash);
    }
    if let Some(tag) = value.strip_prefix("hmac-sha256:") {
        return is_sha256_hex(tag);
    }
    let Some((byte_count, hash)) = value
        .strip_prefix("bytes:")
        .and_then(|rest| rest.split_once(" hash:sha256:"))
    else {
        return false;
    };
    byte_count.parse::<usize>().ok().is_some_and(|parsed| {
        parsed <= MAX_REDACTED_VALUE_BYTES && parsed.to_string() == byte_count
    }) && is_sha256_hex(hash)
}

fn redacted_event_value(value: &Value, key: &MainChatEventDigestKey) -> Value {
    redacted_event_value_with_scope(value, "keyed_input_value_v1", key)
}

fn legacy_redacted_event_value(value: &Value, key: &MainChatEventDigestKey) -> Value {
    redacted_event_value_with_scope(value, "keyed_legacy_receipt_v1", key)
}

fn redacted_event_value_with_scope(
    value: &Value,
    digest_scope: &str,
    key: &MainChatEventDigestKey,
) -> Value {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    json!({
        "redacted": true,
        "valueType": value_type(value),
        "byteCount": bytes.len(),
        "digest": hmac_sha256_digest(key, digest_scope, &bytes),
        "digestScope": digest_scope,
    })
}

fn is_immutable_event_type(event_type: &str) -> bool {
    !VERSIONED_EVENT_TYPES.contains(&event_type)
}

fn select_event_by_exact_fact(
    conn: &Connection,
    task_session_id: &str,
    event_type: &str,
    object_id: &str,
    payload_digest: &str,
) -> Result<Option<MainChatAgentDurableEvent>> {
    let event = conn
        .query_row(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
             FROM main_chat_agent_events
             WHERE task_session_id = ?1 AND event_type = ?2 AND object_id = ?3
                   AND payload_digest = ?4
             ORDER BY sequence ASC
             LIMIT 1",
            params![task_session_id, event_type, object_id, payload_digest],
            row_to_event,
        )
        .optional()?;
    Ok(event)
}

fn select_event_by_immutable_identity(
    conn: &Connection,
    task_session_id: &str,
    event_type: &str,
    object_id: &str,
) -> Result<Option<MainChatAgentDurableEvent>> {
    let mut statement = conn.prepare(
        "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
         FROM main_chat_agent_events
         WHERE task_session_id = ?1 AND event_type = ?2 AND object_id = ?3
         ORDER BY sequence ASC
         LIMIT 2",
    )?;
    let events = statement
        .query_map(
            params![task_session_id, event_type, object_id],
            row_to_event,
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if events.len() > 1 {
        return Err(MainChatAgentEventStoreFault::DuplicateImmutableIdentity {
            event_type: event_type.to_string(),
        }
        .into());
    }
    Ok(events.into_iter().next())
}

fn select_latest_event_by_identity(
    conn: &Connection,
    task_session_id: &str,
    event_type: &str,
    object_id: &str,
) -> Result<Option<MainChatAgentDurableEvent>> {
    conn.query_row(
        "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
         FROM main_chat_agent_events
         WHERE task_session_id = ?1 AND event_type = ?2 AND object_id = ?3
         ORDER BY sequence DESC
         LIMIT 1",
        params![task_session_id, event_type, object_id],
        row_to_event,
    )
    .optional()
    .map_err(Into::into)
}

fn terminal_owner_successor_head_from_conn(
    conn: &Connection,
    task_session_id: &str,
    final_event_id: &str,
    final_event: &MainChatAgentDurableEvent,
) -> Result<(u64, String)> {
    let mut revision = final_event
        .payload
        .get("taskOwnerRevision")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_final_revision_missing"))?;
    let mut digest = final_event
        .payload
        .get("taskOwnerDigest")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_final_digest_missing"))?
        .to_string();
    let mut statement = conn.prepare(
        "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled,
                payload_minimized_version
         FROM main_chat_agent_events
         WHERE task_session_id = ?1
           AND event_type = 'terminal_owner.successor_confirmed'
           AND NOT EXISTS (
               SELECT 1 FROM main_chat_agent_event_fact_quarantine quarantine
               WHERE quarantine.event_id = main_chat_agent_events.event_id
           )
         ORDER BY sequence ASC",
    )?;
    let successors = statement
        .query_map([task_session_id], row_to_event)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for successor in successors {
        let cause_ref = successor
            .payload
            .get("causeRef")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_cause_missing"))?;
        let before_revision = successor
            .payload
            .get("beforeOwnerRevision")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_before_revision_missing"))?;
        let after_revision = successor
            .payload
            .get("afterOwnerRevision")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_after_revision_missing"))?;
        let before_digest = successor
            .payload
            .get("beforeOwnerDigest")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_before_digest_missing"))?;
        let after_digest = successor
            .payload
            .get("afterOwnerDigest")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_after_digest_missing"))?;
        let receipt_ref = successor
            .payload
            .get("localTransitionReceiptRef")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_receipt_ref_missing"))?;
        let receipt_digest = successor
            .payload
            .get("localTransitionReceiptDigest")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_receipt_digest_missing"))?;
        let expected_after_revision = revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("terminal_owner_successor_revision_exhausted"))?;
        if successor.run_id != final_event.run_id
            || successor.object_type != "terminal_owner_successor"
            || successor.object_id != format!("successor:{cause_ref}")
            || successor
                .payload
                .get("finalEventId")
                .and_then(Value::as_str)
                != Some(final_event_id)
            || successor.payload.get("ownerKind").and_then(Value::as_str)
                != Some("agent_task_session")
            || successor.payload.get("ownerId").and_then(Value::as_str) != Some(task_session_id)
            || before_revision != revision
            || before_digest != digest
            || after_revision != expected_after_revision
            || after_digest == before_digest
            || !receipt_ref.starts_with("terminal-transition:")
            || !receipt_digest.starts_with("hmac-sha256:")
        {
            anyhow::bail!("terminal_owner_successor_chain_invalid");
        }
        revision = after_revision;
        digest = after_digest.to_string();
    }
    Ok((revision, digest))
}

fn validate_task_sequence_domain(conn: &Connection, task_session_id: &str) -> Result<()> {
    let (minimum, maximum, count) = conn.query_row(
        "SELECT MIN(sequence), MAX(sequence), COUNT(*)
         FROM main_chat_agent_events WHERE task_session_id = ?1",
        [task_session_id],
        |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    let ledger = conn
        .query_row(
            "SELECT last_sequence FROM main_chat_agent_event_sequences
             WHERE task_session_id = ?1",
            [task_session_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let valid = match (minimum, maximum, count, ledger) {
        (None, None, 0, None) => true,
        (Some(1), Some(maximum), count, Some(ledger)) => {
            count > 0 && maximum == count && ledger == maximum
        }
        _ => false,
    };
    if !valid {
        return Err(MainChatAgentEventStoreFault::CorruptRow {
            field: "sequence",
            reason: "gap_or_ledger_mismatch",
        }
        .into());
    }
    Ok(())
}

fn select_event_by_id(
    conn: &Connection,
    event_id: &str,
) -> Result<Option<MainChatAgentDurableEvent>> {
    let event = conn
        .query_row(
            "SELECT event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
             FROM main_chat_agent_events
             WHERE event_id = ?1",
            [event_id],
            row_to_event,
        )
        .optional()?;
    Ok(event)
}

fn select_terminal_owner_epoch(
    conn: &Connection,
    task_session_id: &str,
) -> Result<Option<TerminalOwnerEpoch>> {
    let row = conn
        .query_row(
            "SELECT epoch_id, task_session_id, run_id, generation, state,
                    canonical_user_message_ref, canonical_user_message_digest,
                    final_event_id, final_event_payload_digest
             FROM terminal_owner_epochs WHERE task_session_id = ?1",
            [task_session_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        epoch_id,
        task_session_id,
        run_id,
        generation,
        state,
        canonical_user_message_ref,
        canonical_user_message_digest,
        final_event_id,
        final_event_payload_digest,
    )) = row
    else {
        return Ok(None);
    };
    Ok(Some(TerminalOwnerEpoch {
        epoch_id,
        task_session_id,
        run_id,
        generation: u64::try_from(generation).context("terminal owner epoch generation invalid")?,
        state: TerminalOwnerSealState::from_str(&state)?,
        canonical_user_message_ref,
        canonical_user_message_digest,
        final_event_id,
        final_event_payload_digest,
        replayed: false,
        review_origin: None,
    }))
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<MainChatAgentDurableEvent> {
    let event_id: String = row.get(0)?;
    let task_session_id: String = row.get(1)?;
    let run_id: String = row.get(2)?;
    let sequence_raw = row.get::<_, i64>(3)?;
    if sequence_raw <= 0 {
        return Err(row_decode_fault(
            3,
            Type::Integer,
            "sequence",
            "must_be_positive",
        ));
    }
    let sequence = u64::try_from(sequence_raw)
        .map_err(|_| row_decode_fault(3, Type::Integer, "sequence", "conversion_failed"))?;
    let event_type: String = row.get(4)?;
    let object_type: String = row.get(5)?;
    let object_id: String = row.get(6)?;
    let created_at_raw: String = row.get(7)?;
    let source: String = row.get(8)?;
    let payload_json: String = row.get(10)?;
    let payload_digest: String = row.get(9)?;
    let backfilled_raw = row.get::<_, i64>(11)?;
    let payload_version = row.get::<_, i64>(12)?;
    if payload_version != DURABLE_EVENT_PAYLOAD_VERSION {
        return Err(row_decode_fault(
            12,
            Type::Integer,
            "payload_minimized_version",
            "unsupported",
        ));
    }
    for (index, field, value) in [
        (1, "task_session_id", task_session_id.as_str()),
        (2, "run_id", run_id.as_str()),
        (6, "object_id", object_id.as_str()),
    ] {
        if !is_bounded_event_reference(value, MAX_EVENT_IDENTITY_CHARS) {
            return Err(row_decode_fault(
                index,
                Type::Text,
                field,
                "invalid_reference",
            ));
        }
    }
    for (index, field, value) in [
        (4, "event_type", event_type.as_str()),
        (5, "object_type", object_type.as_str()),
        (8, "source", source.as_str()),
    ] {
        if !is_typed_event_code(value) {
            return Err(row_decode_fault(
                index,
                Type::Text,
                field,
                "invalid_typed_code",
            ));
        }
    }
    if stable_event_id(&task_session_id, sequence, &event_type, &object_id) != event_id {
        return Err(row_decode_fault(
            0,
            Type::Text,
            "event_id",
            "does_not_match_logical_identity",
        ));
    }
    let backfilled = match backfilled_raw {
        0 => false,
        1 => true,
        _ => {
            return Err(row_decode_fault(
                11,
                Type::Integer,
                "backfilled",
                "must_be_zero_or_one",
            ))
        }
    };
    let created_at = DateTime::parse_from_rfc3339(&created_at_raw)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| row_decode_fault(7, Type::Text, "created_at", "invalid_rfc3339"))?;
    let payload = serde_json::from_str(&payload_json)
        .map_err(|_| row_decode_fault(10, Type::Text, "payload_json", "invalid_json"))?;
    validate_canonical_durable_event_payload(&event_type, &object_type, &payload).map_err(
        |_| {
            row_decode_fault(
                10,
                Type::Text,
                "payload_json",
                "schema_invalid_or_noncanonical",
            )
        },
    )?;
    if stored_event_payload_digest(&event_type, sequence, &payload_json) != payload_digest {
        return Err(row_decode_fault(
            9,
            Type::Text,
            "payload_digest",
            "does_not_match_payload",
        ));
    }
    Ok(MainChatAgentDurableEvent {
        event_id,
        task_session_id,
        run_id,
        sequence,
        event_type,
        object_type,
        object_id,
        created_at,
        source,
        payload_digest,
        payload,
        backfilled,
    })
}

fn row_decode_fault(
    column_index: usize,
    column_type: Type,
    field: &'static str,
    reason: &'static str,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column_index,
        column_type,
        Box::new(MainChatAgentEventStoreFault::CorruptRow { field, reason }),
    )
}

pub(crate) fn materialize_main_chat_agent_events_for_snapshot_in_store(
    store: &MainChatAgentEventStore,
    snapshot: &MainChatAgentStateSnapshot,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    materialize_main_chat_agent_events_for_snapshot_in_store_with_backfill(store, snapshot, false)
}

pub(crate) fn materialize_main_chat_agent_backfill_events_for_snapshot_in_store(
    store: &MainChatAgentEventStore,
    snapshot: &MainChatAgentStateSnapshot,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    materialize_main_chat_agent_events_for_snapshot_in_store_with_backfill(store, snapshot, true)
}

fn materialize_main_chat_agent_events_for_snapshot_in_store_with_backfill(
    store: &MainChatAgentEventStore,
    snapshot: &MainChatAgentStateSnapshot,
    backfilled: bool,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    let drafts = event_drafts_from_snapshot(snapshot, backfilled);
    store.append_batch(drafts)
}

pub(crate) fn append_main_chat_agent_runtime_event_batch_in_store(
    store: &MainChatAgentEventStore,
    task_session_id: &str,
    run_id: &str,
    inputs: Vec<MainChatAgentRuntimeEventInput>,
) -> Result<Vec<MainChatAgentDurableEvent>> {
    store.append_batch(runtime_event_drafts(task_session_id, run_id, inputs))
}

fn runtime_event_drafts(
    task_session_id: &str,
    run_id: &str,
    inputs: Vec<MainChatAgentRuntimeEventInput>,
) -> Vec<MainChatAgentEventDraft> {
    let created_at = Utc::now();
    inputs
        .into_iter()
        .map(|input| MainChatAgentEventDraft {
            task_session_id: task_session_id.to_string(),
            run_id: run_id.to_string(),
            event_type: input.event_type,
            object_type: input.object_type,
            object_id: input.object_id,
            created_at: input.occurred_at.unwrap_or(created_at),
            source: input.source,
            payload: input.payload,
            backfilled: false,
        })
        .collect()
}

pub(crate) async fn append_main_chat_agent_runtime_event_batch_with_provider_proofs(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: &str,
    durability_scope: &crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
    inputs: Vec<MainChatAgentRuntimeEventInput>,
    durability_proofs: &[ProviderInvocationDurabilityProof],
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    let inputs = omit_already_durable_tool_started_inputs(&store, task_session_id, run_id, inputs)?;
    store
        .append_batch_with_provider_proofs(
            runtime_event_drafts(task_session_id, run_id, inputs),
            durability_scope,
            durability_proofs,
        )
        .map_err(|error| error.to_string())
}

pub(crate) async fn materialize_main_chat_agent_events_for_snapshot(
    state: &Arc<AppState>,
    snapshot: &MainChatAgentStateSnapshot,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    materialize_main_chat_agent_events_for_snapshot_in_store(&store, snapshot)
        .map_err(|err| err.to_string())
}

pub(crate) async fn materialize_optional_main_chat_agent_events(
    state: &Arc<AppState>,
    snapshot: Option<&MainChatAgentStateSnapshot>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    if let Some(snapshot) = snapshot {
        materialize_main_chat_agent_events_for_snapshot(state, snapshot).await
    } else {
        Ok(Vec::new())
    }
}

/// Persist exact-request provider lifecycle facts before later AgentRun and
/// product-read-model projections. This seam is intentionally metadata-only;
/// response text and request bodies remain with their canonical owners.
pub(crate) async fn append_main_chat_provider_receipt_events(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: &str,
    durability_scope: &crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
    provider_receipts: &[ProviderInvocationReceipt],
    durability_proofs: &[ProviderInvocationDurabilityProof],
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    if provider_receipts.is_empty() {
        return Ok(Vec::new());
    }
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    store
        .append_batch_with_provider_proofs(
            provider_event_drafts(task_session_id, run_id, provider_receipts)
                .map_err(|error| error.to_string())?,
            durability_scope,
            durability_proofs,
        )
        .map_err(|error| error.to_string())
}

/// Canonical typed-receipt projection shared by ordinary completion and
/// cancellation terminalization. Arguments, outputs and error bodies remain
/// with their canonical owners.
pub(crate) fn project_main_chat_tool_receipt(
    run_id: &str,
    receipt: &ToolExecutionReceipt,
) -> std::result::Result<Option<MainChatToolReceiptEventProjection>, String> {
    receipt.mechanically_valid_terminal().map_err(|reason| {
        format!(
            "invalid_tool_execution_receipt:{}:{reason}",
            receipt.receipt_id
        )
    })?;
    if receipt.source_run_id.as_deref() != Some(run_id) {
        return Err(format!(
            "tool_receipt_run_identity_mismatch:{}",
            receipt.receipt_id
        ));
    }
    if receipt.transport_status == ToolTransportStatus::NotAttempted {
        return Ok(None);
    }
    let manifest_id = receipt
        .manifest_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("tool_receipt_manifest_id_missing:{}", receipt.receipt_id))?;
    let terminal_at = receipt
        .finished_at
        .ok_or_else(|| format!("tool_receipt_finished_at_missing:{}", receipt.receipt_id))?;
    let dispatch_event_at = receipt.dispatched_at.unwrap_or(terminal_at);
    let (dispatch_process_risk, may_outlive_local_process, effect_may_survive_local_process) =
        tool_receipt_process_facts(receipt);
    let (terminal_event_type, terminal_status) = match (
        receipt.transport_status,
        receipt.effect_status,
        receipt.execution_outcome,
    ) {
        (ToolTransportStatus::RemoteUnknown, _, _) => ("tool.remote_unknown", "remote_unknown"),
        (_, ToolEffectStatus::Unknown, _) => ("tool.effect_unknown", "effect_unknown"),
        (ToolTransportStatus::LocalAborted, _, _) => ("tool.local_aborted", "local_aborted"),
        (ToolTransportStatus::ResponseObserved, _, ToolExecutionOutcome::Succeeded) => {
            ("tool.completed", "completed")
        }
        (ToolTransportStatus::ResponseObserved, _, ToolExecutionOutcome::Failed) => {
            ("tool.failed", "failed")
        }
        (ToolTransportStatus::NotAttempted | ToolTransportStatus::Dispatched, _, _)
        | (ToolTransportStatus::ResponseObserved, _, _) => {
            return Err(format!(
                "tool_receipt_unprojectable_terminal:{}",
                receipt.receipt_id
            ));
        }
    };
    Ok(Some(MainChatToolReceiptEventProjection {
        receipt_id: receipt.receipt_id.clone(),
        dispatch_event_at,
        terminal_at,
        terminal_event_type,
        terminal_status,
        common_payload: json!({
            "receiptId": receipt.receipt_id,
            "requestId": receipt.receipt_id,
            "sourceRunId": run_id,
            "manifestId": manifest_id,
            "requestDigest": receipt.request_digest,
            "actionEffect": receipt.action_effect.as_str(),
            "idempotencyContract": receipt.idempotency_contract.as_str(),
            "dispatchKind": receipt.dispatch_kind.as_str(),
            "dispatchAttemptCount": receipt.dispatch_attempt_count,
            "dispatchObserved": receipt.dispatch_observed,
            "reconciledAfterProcessRestart": false,
            "dispatchProcessRisk": dispatch_process_risk,
            "mayOutliveLocalProcess": may_outlive_local_process,
            "effectMaySurviveLocalProcess": effect_may_survive_local_process,
            "transportStatus": receipt.transport_status.as_str(),
            "effectStatus": receipt.effect_status.as_str(),
            "executionOutcome": receipt.execution_outcome.as_str(),
            "auditPersistenceStatus": receipt.audit_persistence_status.as_str(),
            "startedAt": receipt.started_at,
            "dispatchedAt": receipt.dispatched_at,
            "responseObservedAt": receipt.response_observed_at,
            "finishedAt": receipt.finished_at,
        }),
    }))
}

pub(crate) fn main_chat_tool_started_event_input(
    run_id: &str,
    receipt: &ToolExecutionReceipt,
    source: &str,
) -> std::result::Result<MainChatAgentRuntimeEventInput, String> {
    if receipt.source_run_id.as_deref() != Some(run_id) {
        return Err(format!(
            "tool_receipt_run_identity_mismatch:{}",
            receipt.receipt_id
        ));
    }
    if receipt.transport_status != ToolTransportStatus::Dispatched
        || receipt.dispatch_attempt_count == 0
        || !receipt.dispatch_observed
    {
        return Err(format!(
            "tool_receipt_start_not_dispatched:{}",
            receipt.receipt_id
        ));
    }
    let manifest_id = receipt
        .manifest_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("tool_receipt_manifest_id_missing:{}", receipt.receipt_id))?;
    let dispatched_at = receipt
        .dispatched_at
        .ok_or_else(|| format!("tool_receipt_dispatched_at_missing:{}", receipt.receipt_id))?;
    Ok(MainChatAgentRuntimeEventInput::new(
        "tool.started",
        "tool_execution_receipt",
        &receipt.receipt_id,
        source,
        json!({
            "receiptId": receipt.receipt_id,
            "requestId": receipt.receipt_id,
            "sourceRunId": run_id,
            "manifestId": manifest_id,
            "requestDigest": receipt.request_digest,
            "actionEffect": receipt.action_effect.as_str(),
            "idempotencyContract": receipt.idempotency_contract.as_str(),
            "dispatchKind": receipt.dispatch_kind.as_str(),
            "dispatchAttemptCount": receipt.dispatch_attempt_count,
            "dispatchObserved": true,
            "transportStatus": "dispatched",
            "effectStatus": "not_attempted",
            "executionOutcome": "not_observed",
            "auditPersistenceStatus": receipt.audit_persistence_status.as_str(),
            "status": "started",
            "startedAt": receipt.started_at,
            "dispatchedAt": dispatched_at,
            "responseObservedAt": Value::Null,
            "finishedAt": Value::Null,
        }),
    )
    .with_occurred_at(dispatched_at))
}

fn tool_receipt_process_facts(receipt: &ToolExecutionReceipt) -> (&'static str, bool, bool) {
    let may_outlive_local_process = matches!(
        receipt.dispatch_kind,
        ToolDispatchKind::Network
            | ToolDispatchKind::McpStdio
            | ToolDispatchKind::A2a
            | ToolDispatchKind::Unknown
    );
    let effect_may_survive_local_process = receipt.action_effect != ToolActionEffect::ReadOnly;
    (
        if may_outlive_local_process {
            "may_outlive_local_process"
        } else {
            "process_bound"
        },
        may_outlive_local_process,
        effect_may_survive_local_process,
    )
}

pub(crate) fn main_chat_tool_dispatch_event_input(
    run_id: &str,
    receipt: &ToolExecutionReceipt,
    source: &str,
) -> std::result::Result<MainChatAgentRuntimeEventInput, String> {
    if receipt.dispatch_observed {
        let mut started_receipt = receipt.clone();
        started_receipt.transport_status = ToolTransportStatus::Dispatched;
        started_receipt.execution_outcome = ToolExecutionOutcome::NotObserved;
        started_receipt.effect_status = ToolEffectStatus::NotAttempted;
        if started_receipt.audit_persistence_status != ToolAuditPersistenceStatus::NotRequired {
            started_receipt.audit_persistence_status = ToolAuditPersistenceStatus::Pending;
        }
        started_receipt.response_observed_at = None;
        started_receipt.finished_at = None;
        return main_chat_tool_started_event_input(run_id, &started_receipt, source);
    }
    if receipt.source_run_id.as_deref() != Some(run_id) {
        return Err(format!(
            "tool_receipt_run_identity_mismatch:{}",
            receipt.receipt_id
        ));
    }
    if receipt.dispatch_attempt_count == 0 || receipt.dispatched_at.is_some() {
        return Err(format!(
            "tool_receipt_dispatch_ambiguity_invalid:{}",
            receipt.receipt_id
        ));
    }
    let manifest_id = receipt
        .manifest_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("tool_receipt_manifest_id_missing:{}", receipt.receipt_id))?;
    let (dispatch_process_risk, may_outlive_local_process, effect_may_survive_local_process) =
        tool_receipt_process_facts(receipt);
    let observed_at = receipt.finished_at.unwrap_or(receipt.started_at);
    Ok(MainChatAgentRuntimeEventInput::new(
        "tool.dispatch_ambiguous",
        "tool_execution_receipt",
        &receipt.receipt_id,
        source,
        json!({
            "receiptId": receipt.receipt_id,
            "requestId": receipt.receipt_id,
            "sourceRunId": run_id,
            "manifestId": manifest_id,
            "requestDigest": receipt.request_digest,
            "actionEffect": receipt.action_effect.as_str(),
            "idempotencyContract": receipt.idempotency_contract.as_str(),
            "dispatchKind": receipt.dispatch_kind.as_str(),
            "dispatchAttemptCount": receipt.dispatch_attempt_count,
            "dispatchObserved": false,
            "dispatchProcessRisk": dispatch_process_risk,
            "mayOutliveLocalProcess": may_outlive_local_process,
            "effectMaySurviveLocalProcess": effect_may_survive_local_process,
            "reconciledAfterProcessRestart": false,
            "transportStatus": receipt.transport_status.as_str(),
            "effectStatus": receipt.effect_status.as_str(),
            "executionOutcome": receipt.execution_outcome.as_str(),
            "auditPersistenceStatus": receipt.audit_persistence_status.as_str(),
            "status": "dispatch_ambiguous",
            "startedAt": receipt.started_at,
            "dispatchedAt": Value::Null,
            "responseObservedAt": Value::Null,
            "finishedAt": Value::Null,
        }),
    )
    .with_occurred_at(observed_at))
}

/// Convert regular (non-cancellation) ToolGateway receipts into the durable
/// lifecycle inputs owned by `MainChatAgentEventStore`.
pub(crate) fn main_chat_tool_receipt_event_inputs(
    run_id: &str,
    receipts: &[ToolExecutionReceipt],
    source: &str,
) -> std::result::Result<Vec<MainChatAgentRuntimeEventInput>, String> {
    let mut inputs = Vec::new();
    let mut seen_receipt_ids = std::collections::HashSet::new();
    for receipt in receipts {
        if !seen_receipt_ids.insert(receipt.receipt_id.as_str()) {
            return Err(format!(
                "duplicate_tool_execution_receipt:{}",
                receipt.receipt_id
            ));
        }
        let Some(projection) = project_main_chat_tool_receipt(run_id, receipt)? else {
            continue;
        };
        inputs.push(main_chat_tool_dispatch_event_input(
            run_id, receipt, source,
        )?);
        let mut terminal_payload = projection.common_payload;
        terminal_payload["status"] = json!(projection.terminal_status);
        inputs.push(
            MainChatAgentRuntimeEventInput::new(
                projection.terminal_event_type,
                "tool_execution_receipt",
                &projection.receipt_id,
                source,
                terminal_payload,
            )
            .with_occurred_at(projection.terminal_at),
        );
    }
    Ok(inputs)
}

fn omit_already_durable_tool_started_inputs(
    store: &MainChatAgentEventStore,
    task_session_id: &str,
    run_id: &str,
    inputs: Vec<MainChatAgentRuntimeEventInput>,
) -> std::result::Result<Vec<MainChatAgentRuntimeEventInput>, String> {
    const IMMUTABLE_START_FIELDS: [&str; 10] = [
        "receiptId",
        "requestId",
        "sourceRunId",
        "manifestId",
        "requestDigest",
        "actionEffect",
        "idempotencyContract",
        "dispatchKind",
        "startedAt",
        "dispatchedAt",
    ];
    let mut filtered = Vec::with_capacity(inputs.len());
    for input in inputs {
        if input.event_type != "tool.started" {
            filtered.push(input);
            continue;
        }
        let existing = store
            .get_immutable_event(task_session_id, "tool.started", &input.object_id)
            .map_err(|error| error.to_string())?;
        let Some(existing) = existing else {
            filtered.push(input);
            continue;
        };
        if existing.run_id != run_id
            || IMMUTABLE_START_FIELDS
                .iter()
                .any(|field| existing.payload.get(field) != input.payload.get(field))
        {
            return Err(format!(
                "durable_tool_started_identity_mismatch:{}",
                input.object_id
            ));
        }
        // MainChatToolLifecycleObserver owns the immutable adapter-edge start.
        // A later terminal receipt may have a larger retry count and must not
        // reconstruct or overwrite that earlier fact.
    }
    Ok(filtered)
}

fn append_main_chat_tool_receipt_event_batch_in_store(
    store: &MainChatAgentEventStore,
    task_session_id: &str,
    run_id: &str,
    inputs: Vec<MainChatAgentRuntimeEventInput>,
) -> std::result::Result<Vec<MainChatAgentDurableEvent>, String> {
    let inputs = omit_already_durable_tool_started_inputs(store, task_session_id, run_id, inputs)?;
    append_main_chat_agent_runtime_event_batch_in_store(store, task_session_id, run_id, inputs)
        .map_err(|error| error.to_string())
}

/// Persist live zero-attempt closures before any ActionQueue or product
/// projection can observe a failed replay. Receipts without a prepared fence
/// produce no event; there is nothing durable to resolve in that case.
pub(crate) async fn append_main_chat_live_not_dispatched_tool_receipts(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: &str,
    receipts: &[ToolExecutionReceipt],
) -> std::result::Result<Vec<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    let mut events = Vec::new();
    for receipt in receipts {
        if receipt.transport_status != ToolTransportStatus::NotAttempted {
            continue;
        }
        if let Some(event) = store
            .append_live_not_dispatched_tool_receipt(
                task_session_id,
                run_id,
                receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .map_err(|error| error.to_string())?
        {
            events.push(event);
        }
    }
    Ok(events)
}

pub(crate) async fn append_main_chat_tool_receipt_events(
    state: &Arc<AppState>,
    task_session_id: &str,
    run_id: &str,
    receipts: &[ToolExecutionReceipt],
    source: &str,
) -> std::result::Result<Vec<MainChatAgentDurableEvent>, String> {
    let mut durable = append_main_chat_live_not_dispatched_tool_receipts(
        state,
        task_session_id,
        run_id,
        receipts,
    )
    .await?;
    let inputs = main_chat_tool_receipt_event_inputs(run_id, receipts, source)?;
    if inputs.is_empty() {
        return Ok(durable);
    }
    let store_arc = state
        .main_chat_agent_event_store
        .as_ref()
        .ok_or_else(|| "main_chat_agent_event_store_unavailable".to_string())?;
    let store = store_arc.lock().await;
    durable.extend(append_main_chat_tool_receipt_event_batch_in_store(
        &store,
        task_session_id,
        run_id,
        inputs,
    )?);
    Ok(durable)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn append_main_chat_agent_runtime_event(
    state: &Arc<AppState>,
    task_session_id: impl Into<String>,
    run_id: impl Into<String>,
    event_type: impl Into<String>,
    object_type: impl Into<String>,
    object_id: impl Into<String>,
    source: impl Into<String>,
    payload: Value,
) -> Result<MainChatAgentDurableEvent, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    store
        .append(MainChatAgentEventDraft {
            task_session_id: task_session_id.into(),
            run_id: run_id.into(),
            event_type: event_type.into(),
            object_type: object_type.into(),
            object_id: object_id.into(),
            created_at: Utc::now(),
            source: source.into(),
            payload,
            backfilled: false,
        })
        .map_err(|err| err.to_string())
}

pub(crate) async fn append_main_chat_agent_runtime_event_batch(
    state: &Arc<AppState>,
    task_session_id: impl Into<String>,
    run_id: impl Into<String>,
    inputs: Vec<MainChatAgentRuntimeEventInput>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    let task_session_id = task_session_id.into();
    let run_id = run_id.into();
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    append_main_chat_agent_runtime_event_batch_in_store(&store, &task_session_id, &run_id, inputs)
        .map_err(|err| err.to_string())
}

pub(crate) async fn list_main_chat_agent_events_with_state(
    state: &Arc<AppState>,
    task_session_id: String,
    after_sequence: Option<u64>,
    limit: Option<u64>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Err("main_chat_agent_event_store_unavailable".into());
    };
    let store = store_arc.lock().await;
    store
        .list(
            &task_session_id,
            after_sequence.unwrap_or(0),
            limit.unwrap_or(100),
        )
        .map_err(|err| err.to_string())
}

pub(crate) async fn latest_main_chat_provider_event_with_state(
    state: &Arc<AppState>,
) -> Result<Option<MainChatAgentDurableEvent>, String> {
    let Some(store_arc) = state.main_chat_agent_event_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .latest_provider_event()
        .map_err(|error| error.to_string())
}

pub(crate) async fn get_main_chat_agent_state_snapshot_with_state(
    state: &Arc<AppState>,
    task_session_id: String,
) -> Result<MainChatAgentStateSnapshot, String> {
    let run_id = if let Some(store_arc) = state.main_chat_agent_event_store.as_ref() {
        store_arc
            .lock()
            .await
            .latest_run_id(&task_session_id)
            .map_err(|err| err.to_string())?
    } else {
        None
    };
    if let (Some(store_arc), Some(run_id)) = (
        state.main_chat_agent_event_store.as_ref(),
        run_id.as_deref(),
    ) {
        store_arc
            .lock()
            .await
            .quarantine_run_derived_backfill_facts(&task_session_id, Some(run_id))
            .map_err(|err| err.to_string())?;
    }
    let mut snapshot =
        assemble_main_chat_agent_state_for_turn(state, Some(&task_session_id), run_id.as_deref())
            .await
            .ok_or_else(|| "main_chat_agent_snapshot_unavailable".to_string())?;
    if let Some(store_arc) = state.main_chat_agent_event_store.as_ref() {
        let store = store_arc.lock().await;
        materialize_main_chat_agent_backfill_events_for_snapshot_in_store(&store, &snapshot)
            .map_err(|err| err.to_string())?;
        store
            .quarantine_run_derived_backfill_facts(
                &snapshot.task.task_id,
                Some(&snapshot.task.run_id),
            )
            .map_err(|err| err.to_string())?;
        snapshot.sequence = store
            .latest_sequence(&task_session_id)
            .map_err(|err| err.to_string())?;
    }
    Ok(snapshot)
}

#[tauri::command]
pub(crate) async fn list_main_chat_agent_events(
    task_session_id: String,
    after_sequence: Option<u64>,
    limit: Option<u64>,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<MainChatAgentDurableEvent>, String> {
    list_main_chat_agent_events_with_state(state.inner(), task_session_id, after_sequence, limit)
        .await
}

#[tauri::command]
pub(crate) async fn get_main_chat_agent_state_snapshot(
    task_session_id: String,
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<crate::product_agent_dto::ProductMainChatAgentStateSnapshot, String> {
    get_main_chat_agent_state_snapshot_with_state(state.inner(), task_session_id)
        .await
        .map(crate::product_agent_dto::ProductMainChatAgentStateSnapshot::from)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatEventGapRecoveryDecision {
    pub(crate) status: String,
    pub(crate) replay_after_sequence: u64,
    pub(crate) expected_sequence: u64,
    pub(crate) observed_sequence: u64,
    pub(crate) snapshot_required: bool,
}

pub(crate) fn evaluate_main_chat_event_gap_recovery(
    replayed_events: &[MainChatAgentDurableEvent],
    last_applied_sequence: u64,
    observed_sequence: u64,
) -> MainChatEventGapRecoveryDecision {
    let expected_sequence = last_applied_sequence + 1;
    let relevant_events = replayed_events
        .iter()
        .filter(|event| event.sequence > last_applied_sequence)
        .collect::<Vec<_>>();
    let replay_covers_gap = !relevant_events.is_empty()
        && relevant_events
            .first()
            .is_some_and(|event| event.sequence == expected_sequence)
        && relevant_events
            .windows(2)
            .all(|pair| pair[1].sequence == pair[0].sequence + 1)
        && relevant_events
            .last()
            .is_some_and(|event| event.sequence >= observed_sequence);
    MainChatEventGapRecoveryDecision {
        status: if replay_covers_gap {
            "replaying_events".into()
        } else {
            "snapshot_refresh_required".into()
        },
        replay_after_sequence: last_applied_sequence,
        expected_sequence,
        observed_sequence,
        snapshot_required: !replay_covers_gap,
    }
}

fn event_drafts_from_snapshot(
    snapshot: &MainChatAgentStateSnapshot,
    backfilled: bool,
) -> Vec<MainChatAgentEventDraft> {
    let mut drafts = Vec::new();
    let task_id = snapshot.task.task_id.clone();
    let run_id = snapshot.task.run_id.clone();
    drafts.push(draft(
        snapshot,
        "task.created",
        "task",
        &task_id,
        "agent_ingress",
        json!({
            "taskSessionId": task_id,
            "runId": run_id,
            "strategy": snapshot.task.strategy.as_str(),
        }),
    ));
    drafts.push(draft(
        snapshot,
        "route.selected",
        "route",
        snapshot.route.strategy.as_str(),
        "strategy_router",
        json!({
            "taskSessionId": snapshot.task.task_id,
            "runId": snapshot.task.run_id,
            "strategy": snapshot.route.strategy.as_str(),
            "reason": snapshot.route.reason,
        }),
    ));
    for context in &snapshot.context {
        drafts.push(draft(
            snapshot,
            "context.selected",
            "context",
            &context.context_id,
            "context_compiler",
            json!({
                "contextId": context.context_id,
                "sourceKind": context.source_kind,
                "sourceLabel": context.source_label,
                "evidenceId": context.evidence_id,
            }),
        ));
    }
    if let Some(provider) = &snapshot.provider {
        drafts.push(draft(
            snapshot,
            "provider.selected",
            "provider",
            &provider.evidence_id,
            "provider_lifecycle_projection",
            json!({
                "provider": provider.provider,
                "model": provider.model,
                "routeType": provider.route_type,
                "providerConfigGeneration": provider.provider_config_generation,
                "evidenceId": provider.evidence_id,
            }),
        ));
    }
    if let Some(plan) = &snapshot.plan {
        drafts.push(draft(
            snapshot,
            "plan.updated",
            "plan",
            &plan.plan_id,
            "plan_runtime",
            json!({
                "planId": plan.plan_id,
                "status": plan.status,
                "evidenceId": plan.evidence_id,
            }),
        ));
    }
    for action in &snapshot.actions {
        drafts.push(draft(
            snapshot,
            "action.queued",
            "action",
            &action.action_id,
            "action_queue",
            json!({
                "actionId": action.action_id,
                "actionType": action.action_type,
                "status": "queued",
                "target": action.target,
                "policyDecisionId": action.policy_decision_id,
            }),
        ));
        if action.started_at.is_some() {
            drafts.push(draft(
                snapshot,
                "action.started",
                "action",
                &action.action_id,
                "action_executor",
                json!({
                    "actionId": action.action_id,
                    "status": "running",
                    "startedAt": action.started_at,
                }),
            ));
        }
        match action.status.as_str() {
            "succeeded" => drafts.push(draft(
                snapshot,
                "action.completed",
                "action",
                &action.action_id,
                "action_executor",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                    "observationIds": action.observation_ids,
                }),
            )),
            "failed" => drafts.push(draft(
                snapshot,
                "action.failed",
                "action",
                &action.action_id,
                "action_executor",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                    "retryable": action.retryable,
                }),
            )),
            _ => drafts.push(draft(
                snapshot,
                "action.updated",
                "action",
                &action.action_id,
                "action_queue",
                json!({
                    "actionId": action.action_id,
                    "status": action.status,
                }),
            )),
        }
    }
    for observation in &snapshot.observations {
        drafts.push(draft(
            snapshot,
            "observation.created",
            "observation",
            &observation.observation_id,
            "action_executor",
            json!({
                "observationId": observation.observation_id,
                "actionId": observation.action_id,
                "sourceKind": observation.source_kind,
                "sourceLabel": observation.source_label,
                "readExecution": observation.read_execution,
            }),
        ));
    }
    for blocker in &snapshot.blockers {
        drafts.push(draft(
            snapshot,
            "blocker.created",
            "blocker",
            &blocker.blocker_id,
            "agent_loop",
            json!({
                "blockerId": blocker.blocker_id,
                "reasonCode": blocker.reason_code,
                "affectedActionId": blocker.affected_action_id,
                "recoverable": blocker.recoverable,
            }),
        ));
    }
    for proposal in &snapshot.proposals {
        drafts.push(draft(
            snapshot,
            "proposal.created",
            "proposal",
            &proposal.proposal_id,
            "proposal_store",
            json!({
                "proposalId": proposal.proposal_id,
                "proposalType": proposal.proposal_type,
                "evidenceIds": proposal.evidence_ids,
                "actionIds": proposal.action_ids,
            }),
        ));
        let status_event = match proposal.status {
            MainChatAgentProductProposalStatus::Accepted => Some("proposal.accepted"),
            MainChatAgentProductProposalStatus::Rejected => Some("proposal.rejected"),
            MainChatAgentProductProposalStatus::Deferred => Some("proposal.deferred"),
            _ => None,
        };
        if let Some(event_type) = status_event {
            drafts.push(draft(
                snapshot,
                event_type,
                "proposal",
                &proposal.proposal_id,
                "proposal_store",
                json!({
                    "proposalId": proposal.proposal_id,
                    "status": proposal.status.as_str(),
                }),
            ));
        }
        if let Some(record) = &proposal.memory_lifecycle {
            let status = format!("{:?}", record.status).to_ascii_lowercase();
            if status.contains("materialized") {
                drafts.push(draft(
                    snapshot,
                    "memory.materialized",
                    "memory",
                    &record.memory_id,
                    "proposal_store",
                    json!({
                        "memoryId": record.memory_id,
                        "proposalId": record.proposal_id,
                        "materializedViewVersion": record.materialized_view_version,
                    }),
                ));
            }
            if status.contains("rolledback") || status.contains("rolled_back") {
                drafts.push(draft(
                    snapshot,
                    "memory.rolled_back",
                    "memory",
                    &record.memory_id,
                    "proposal_store",
                    json!({
                        "memoryId": record.memory_id,
                        "proposalId": record.proposal_id,
                        "rolledBackByEventId": record.rolled_back_by_event_id,
                    }),
                ));
            }
        }
    }
    // FinalDelivery is an immutable TurnRuntime receipt, not a derived
    // snapshot event. Re-emitting it here created a second terminal owner with
    // a different delivery id and no canonical assistant-body binding.
    for diagnostic in &snapshot.diagnostics {
        drafts.push(draft(
            snapshot,
            "diagnostic.created",
            "diagnostic",
            &diagnostic.gap_id,
            "diagnostic",
            json!({
                "gapId": diagnostic.gap_id,
                "gapCode": diagnostic.gap_code,
                "evidenceId": diagnostic.evidence_id,
            }),
        ));
    }
    drafts.push(draft(
        snapshot,
        "task.updated",
        "task",
        &snapshot.task.task_id,
        "task_control",
        json!({
            "taskSessionId": snapshot.task.task_id,
            "status": snapshot.task.status.as_str(),
            "controls": snapshot.task.controls,
            "actionIds": snapshot.task.action_ids,
            "observationIds": snapshot.task.observation_ids,
            "blockerIds": snapshot.task.blocker_ids,
            "proposalIds": snapshot.task.proposal_ids,
            "finalDeliveryId": snapshot.task.final_delivery_id,
        }),
    ));
    if backfilled {
        for draft in &mut drafts {
            draft.backfilled = true;
            draft.source = "diagnostic".into();
        }
    }
    drafts
}

fn serialized_metadata_enum_label<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
fn synthetic_provider_policy_evidence_for_test(
    request_id: &str,
    provider_config_generation: &str,
) -> ProviderPolicyReceiptEvidence {
    use openlife_core::llm::{
        ProviderDataRoute, ProviderPayloadCategory, ProviderPayloadPurpose, ProviderPolicyAuthority,
    };

    ProviderPolicyReceiptEvidence {
        decision_id: format!("policy-{request_id}"),
        policy_version: "main_chat_policy_v2".into(),
        issuing_authority: ProviderPolicyAuthority::MainChatPolicyRouter,
        effective_data_route: ProviderDataRoute::PolicyAllowed,
        effective_local_restriction: None,
        subject_scope_digest: format!("sha256:{}", "b".repeat(64)),
        payload_purpose: Some(ProviderPayloadPurpose::MainChatDirectAnswer),
        unfiltered_payload_digest: Some(format!("sha256:{}", "c".repeat(64))),
        context_manifest_digest: format!("sha256:{}", "a".repeat(64)),
        prepared_envelope_digest: Some(format!("sha256:{}", "d".repeat(64))),
        provider_config_generation: provider_config_generation.to_string(),
        network_policy_decision_digest: format!("sha256:{}", "e".repeat(64)),
        selected_context_refs: Vec::new(),
        included_context_categories: Vec::new(),
        declared_payload_categories: vec![ProviderPayloadCategory::CurrentUserConversation],
        policy_provenance_refs: Vec::new(),
        raw_life_model_included: false,
        raw_unbounded_memory_included: false,
    }
}

pub(crate) fn append_provider_policy_evidence_payload(
    payload: &mut Value,
    evidence: &ProviderPolicyReceiptEvidence,
) -> Result<()> {
    let object = payload
        .as_object_mut()
        .context("provider policy evidence payload is not an object")?;
    let request_id = object
        .get("requestId")
        .and_then(Value::as_str)
        .context("provider policy evidence request id missing")?;
    let provider = object
        .get("provider")
        .and_then(Value::as_str)
        .context("provider policy evidence provider missing")?;
    let model = object
        .get("model")
        .and_then(Value::as_str)
        .context("provider policy evidence model missing")?;
    let lifecycle_evidence_digest =
        provider_lifecycle_evidence_digest(request_id, provider, model, evidence)?;
    object.insert("policyDecisionId".into(), json!(evidence.decision_id));
    object.insert("policyVersion".into(), json!(evidence.policy_version));
    object.insert(
        "policyAuthority".into(),
        json!(serialized_metadata_enum_label(evidence.issuing_authority)),
    );
    object.insert(
        "effectiveDataRoute".into(),
        json!(serialized_metadata_enum_label(
            evidence.effective_data_route
        )),
    );
    object.insert(
        "effectiveLocalRestriction".into(),
        evidence
            .effective_local_restriction
            .map(serialized_metadata_enum_label)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    object.insert(
        "subjectScopeDigest".into(),
        json!(evidence.subject_scope_digest),
    );
    object.insert(
        "payloadPurpose".into(),
        evidence
            .payload_purpose
            .map(serialized_metadata_enum_label)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    object.insert(
        "unfilteredPayloadDigest".into(),
        evidence
            .unfiltered_payload_digest
            .as_ref()
            .map(|value| Value::String(value.clone()))
            .unwrap_or(Value::Null),
    );
    object.insert(
        "contextManifestDigest".into(),
        json!(evidence.context_manifest_digest),
    );
    object.insert(
        "preparedEnvelopeDigest".into(),
        evidence
            .prepared_envelope_digest
            .as_ref()
            .map(|value| Value::String(value.clone()))
            .unwrap_or(Value::Null),
    );
    object.insert(
        "providerConfigGeneration".into(),
        json!(evidence.provider_config_generation),
    );
    object.insert(
        "networkPolicyDecisionDigest".into(),
        json!(evidence.network_policy_decision_digest),
    );
    object.insert(
        "selectedContextRefs".into(),
        json!(evidence.selected_context_refs),
    );
    object.insert(
        "includedContextCategories".into(),
        json!(evidence.included_context_categories),
    );
    object.insert(
        "declaredPayloadCategories".into(),
        json!(evidence
            .declared_payload_categories
            .iter()
            .copied()
            .map(serialized_metadata_enum_label)
            .collect::<Vec<_>>()),
    );
    object.insert(
        "policyProvenanceRefs".into(),
        json!(evidence
            .policy_provenance_refs
            .iter()
            .map(|reference| format!(
                "{}:{}:{}",
                serialized_metadata_enum_label(reference.kind()),
                reference.reference_id(),
                reference.digest()
            ))
            .collect::<Vec<_>>()),
    );
    object.insert(
        "rawLifeModelIncluded".into(),
        json!(evidence.raw_life_model_included),
    );
    object.insert(
        "rawUnboundedMemoryIncluded".into(),
        json!(evidence.raw_unbounded_memory_included),
    );
    object.insert(
        "policyEvidenceDigest".into(),
        json!(lifecycle_evidence_digest),
    );
    Ok(())
}

fn provider_receipt_event_payload(
    receipt: &ProviderInvocationReceipt,
    status: &str,
    include_error: bool,
) -> Result<Value> {
    let mut payload = json!({
        "requestId": receipt.request_id,
        "provider": receipt.provider,
        "model": receipt.model,
        "status": status,
        "startedAt": receipt.started_at,
    });
    if status != "started" {
        payload
            .as_object_mut()
            .expect("provider receipt payload is an object")
            .insert("finishedAt".into(), json!(receipt.finished_at));
    }
    if include_error {
        payload
            .as_object_mut()
            .expect("provider receipt payload is an object")
            .insert("errorDigest".into(), json!(receipt.error_digest));
    }
    if let Some(evidence) = receipt.policy_evidence.as_ref() {
        append_provider_policy_evidence_payload(&mut payload, evidence)?;
    }
    Ok(payload)
}

pub(crate) fn main_chat_provider_receipt_event_inputs(
    receipts: &[ProviderInvocationReceipt],
) -> Result<Vec<MainChatAgentRuntimeEventInput>> {
    let mut inputs = Vec::with_capacity(receipts.len() * 2);
    for receipt in receipts {
        inputs.push(
            MainChatAgentRuntimeEventInput::new(
                "provider.started",
                "provider_request",
                bounded_label(&receipt.request_id, 180),
                "provider_adapter",
                provider_receipt_event_payload(receipt, "started", false)?,
            )
            .with_occurred_at(receipt.started_at),
        );
        let (event_type, status) = match receipt.status {
            ProviderInvocationStatus::Completed => ("provider.completed", "completed"),
            ProviderInvocationStatus::Failed => ("provider.failed", "failed"),
            ProviderInvocationStatus::RemoteUnknown => {
                ("provider.remote_unknown", "remote_unknown")
            }
        };
        inputs.push(
            MainChatAgentRuntimeEventInput::new(
                event_type,
                "provider_request",
                bounded_label(&receipt.request_id, 180),
                "provider_adapter",
                provider_receipt_event_payload(receipt, status, true)?,
            )
            .with_occurred_at(receipt.finished_at),
        );
    }
    Ok(inputs)
}

fn provider_event_drafts(
    task_session_id: &str,
    run_id: &str,
    receipts: &[ProviderInvocationReceipt],
) -> Result<Vec<MainChatAgentEventDraft>> {
    Ok(main_chat_provider_receipt_event_inputs(receipts)?
        .into_iter()
        .map(|input| MainChatAgentEventDraft {
            task_session_id: task_session_id.to_string(),
            run_id: run_id.to_string(),
            event_type: input.event_type,
            object_type: input.object_type,
            object_id: input.object_id,
            created_at: input.occurred_at.unwrap_or_else(Utc::now),
            source: input.source,
            payload: input.payload,
            backfilled: false,
        })
        .collect())
}

fn draft(
    snapshot: &MainChatAgentStateSnapshot,
    event_type: &str,
    object_type: &str,
    object_id: &str,
    source: &str,
    payload: Value,
) -> MainChatAgentEventDraft {
    MainChatAgentEventDraft {
        task_session_id: snapshot.task.task_id.clone(),
        run_id: snapshot.task.run_id.clone(),
        event_type: event_type.into(),
        object_type: object_type.into(),
        object_id: bounded_label(object_id, 180),
        created_at: snapshot.emitted_at,
        source: source.into(),
        payload,
        backfilled: false,
    }
}

fn metadata_safe_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("bytes:{} hash:sha256:{:x}", value.len(), hasher.finalize())
}

fn sha256_identity_digest(value: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value);
    format!("sha256:{:x}", hasher.finalize())
}

fn stored_event_payload_digest(event_type: &str, sequence: u64, payload_json: &str) -> String {
    if is_immutable_event_type(event_type) {
        metadata_safe_digest(payload_json)
    } else {
        metadata_safe_digest(&format!(
            "transition-sequence:{sequence}\npayload:{payload_json}"
        ))
    }
}

fn stable_event_id(
    task_session_id: &str,
    sequence: u64,
    event_type: &str,
    object_id: &str,
) -> String {
    let mut identity = Vec::new();
    identity.extend_from_slice(b"openlife.main_chat_event_identity.v2\0");
    for component in [task_session_id, event_type, object_id] {
        identity.extend_from_slice(&(component.len() as u64).to_be_bytes());
        identity.extend_from_slice(component.as_bytes());
    }
    identity.extend_from_slice(&sequence.to_be_bytes());
    format!("mainchat_event:v2:{}", sha256_identity_digest(&identity))
}

fn bounded_label(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        if ch.is_control() {
            output.push(' ');
        } else {
            output.push(ch);
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Production registry shared by append-time schema lookup and coverage tests.
/// Adding an event requires one authority update; a test-only source map cannot
/// silently drift from the executable decoder.
const DURABLE_EVENT_REGISTRY: [(&str, &str); 50] = [
    ("turn_started", "turn"),
    ("cancel_requested", "turn"),
    ("local_aborted", "turn"),
    ("interrupted", "turn"),
    ("failed", "turn"),
    ("provider.started", "provider_request"),
    ("provider.completed", "provider_request"),
    ("provider.failed", "provider_request"),
    ("provider.remote_unknown", "provider_request"),
    ("provider.receipt_state_failed", "provider_attempt_state"),
    ("tool.dispatch_prepared", "tool_execution_receipt"),
    ("tool.not_dispatched", "tool_execution_receipt"),
    ("tool.dispatch_ambiguous", "tool_execution_receipt"),
    ("tool.started", "tool_execution_receipt"),
    ("tool.completed", "tool_execution_receipt"),
    ("tool.failed", "tool_execution_receipt"),
    ("tool.effect_unknown", "tool_execution_receipt"),
    ("tool.local_aborted", "tool_execution_receipt"),
    ("tool.remote_unknown", "tool_execution_receipt"),
    ("task.created", "task"),
    ("route.selected", "route"),
    ("context.selected", "context"),
    ("provider.selected", "provider"),
    ("plan.created", "plan"),
    ("plan.updated", "plan"),
    ("plan.confirmed", "plan"),
    ("plan.reviewed", "plan_review"),
    ("step.created", "step"),
    ("step.updated", "step"),
    ("step.cancelled", "step"),
    ("step.skipped", "step"),
    ("action.queued", "action"),
    ("action.started", "action"),
    ("action.completed", "action"),
    ("action.failed", "action"),
    ("action.updated", "action"),
    ("observation.created", "observation"),
    ("blocker.created", "blocker"),
    ("proposal.created", "proposal"),
    ("proposal.accepted", "proposal"),
    ("proposal.rejected", "proposal"),
    ("proposal.deferred", "proposal"),
    ("proposal.updated", "proposal"),
    ("memory.materialized", "memory"),
    ("memory.rolled_back", "memory"),
    ("effect_committed", "state_effect"),
    (
        "terminal_owner.successor_confirmed",
        "terminal_owner_successor",
    ),
    ("final_delivery.created", "final_delivery"),
    ("diagnostic.created", "diagnostic"),
    ("task.updated", "task"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v6_payload_schema_covers_the_shared_production_event_registry() {
        let mut unique = std::collections::BTreeSet::new();
        for (event_type, expected_object_type) in DURABLE_EVENT_REGISTRY {
            assert!(unique.insert(event_type), "duplicate source-map event");
            let schema = durable_event_payload_schema(event_type, DURABLE_EVENT_PAYLOAD_VERSION)
                .unwrap_or_else(|| panic!("missing v6 schema for {event_type}"));
            assert_eq!(schema.object_type, expected_object_type, "{event_type}");
        }
        assert_eq!(unique.len(), DURABLE_EVENT_REGISTRY.len());
        assert!(
            durable_event_payload_schema("unknown.event", DURABLE_EVENT_PAYLOAD_VERSION).is_none()
        );
        assert!(
            durable_event_payload_schema("task.updated", DURABLE_EVENT_PAYLOAD_VERSION - 1)
                .is_none()
        );
    }

    #[test]
    fn replay_final_receipt_metadata_remains_typed_and_readable() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-replay-final-schema".into(),
                run_id: "run-replay-final-schema".into(),
                event_type: "final_delivery.created".into(),
                object_type: "final_delivery".into(),
                object_id: "replay-delivery:task-replay-final-schema:2:execution-1".into(),
                created_at: Utc::now(),
                source: "openlife_turn_runtime.final_delivery_owner".into(),
                payload: json!({
                    "status": "failed",
                    "reasonCode": "replay_terminal_error",
                    "replayEpochGeneration": 2,
                    "replayExecutionRef": "execution-1",
                    "errorBodyStored": false,
                }),
                backfilled: false,
            })
            .unwrap();

        assert_eq!(event.payload["reasonCode"], "replay_terminal_error");
        assert_eq!(event.payload["replayEpochGeneration"], 2);
        assert_eq!(event.payload["replayExecutionRef"], "execution-1");
        assert_eq!(event.payload["errorBodyStored"], false);
        assert!(event.payload.get(UNRECOGNIZED_FIELDS_RECEIPT).is_none());
    }

    #[test]
    fn typed_error_digests_keep_counterfactual_event_facts_distinct() {
        let digest_a = format!("sha256:{}", "a".repeat(64));
        let digest_b = format!("sha256:{}", "b".repeat(64));
        let payload = |digest: &str| {
            let mut payload = json!({
                "requestId": "request-1",
                "provider": "openai",
                "model": "model-1",
                "status": "failed",
                "errorDigest": digest,
            });
            append_provider_policy_evidence_payload(
                &mut payload,
                &synthetic_provider_policy_evidence_for_test(
                    "request-1",
                    "test-provider-generation",
                ),
            )
            .unwrap();
            payload
        };
        let normalized_a = normalize_durable_event_payload(
            "provider.failed",
            "provider_request",
            &payload(&digest_a),
            PayloadNormalizationOrigin::New,
        )
        .unwrap();
        let normalized_b = normalize_durable_event_payload(
            "provider.failed",
            "provider_request",
            &payload(&digest_b),
            PayloadNormalizationOrigin::New,
        )
        .unwrap();
        assert_eq!(normalized_a["errorDigest"]["redacted"], true);
        assert_eq!(normalized_b["errorDigest"]["redacted"], true);
        assert_ne!(normalized_a["errorDigest"]["digest"], digest_a);
        assert_ne!(normalized_b["errorDigest"]["digest"], digest_b);
        assert_ne!(
            normalized_a["errorDigest"]["digest"],
            normalized_b["errorDigest"]["digest"]
        );
        assert_ne!(normalized_a, normalized_b);
    }

    #[test]
    fn event_status_contract_rejects_semantically_impossible_counterexamples() {
        for (event_type, object_type, payload) in [
            (
                "step.cancelled",
                "step",
                json!({"stepId": "step-1", "status": "completed"}),
            ),
            (
                "plan.confirmed",
                "plan",
                json!({"planId": "plan-1", "status": "draft"}),
            ),
            (
                "task.updated",
                "task",
                json!({"taskSessionId": "task-1", "status": "pin_74291"}),
            ),
        ] {
            assert!(normalize_durable_event_payload(
                event_type,
                object_type,
                &payload,
                PayloadNormalizationOrigin::New,
            )
            .is_err());
        }
    }

    #[test]
    fn action_updated_preserves_unobserved_unknown_without_accepting_arbitrary_status() {
        let normalized = normalize_durable_event_payload(
            "action.updated",
            "action",
            &json!({"actionId": "action-1", "status": "unknown"}),
            PayloadNormalizationOrigin::New,
        )
        .expect("unobserved action state remains explicitly unknown");
        assert_eq!(normalized["status"], "unknown");
        assert!(normalize_durable_event_payload(
            "action.updated",
            "action",
            &json!({"actionId": "action-1", "status": "adapter-secret-body"}),
            PayloadNormalizationOrigin::New,
        )
        .is_err());
    }

    #[test]
    fn event_identity_depends_on_logical_position_not_payload_digest() {
        let first_store = MainChatAgentEventStore::new_in_memory().unwrap();
        let second_store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event_with_payload = |gap_code: &str| MainChatAgentEventDraft {
            task_session_id: "task-1".into(),
            run_id: "run-1".into(),
            event_type: "diagnostic.created".into(),
            object_type: "diagnostic".into(),
            object_id: "gap-1".into(),
            created_at: Utc::now(),
            source: "diagnostic".into(),
            payload: json!({"gapId": "gap-1", "gapCode": gap_code}),
            backfilled: false,
        };
        let first_fact = first_store.append(event_with_payload("first_gap")).unwrap();
        let counterfactual_fact = second_store
            .append(event_with_payload("different_gap"))
            .unwrap();
        let first = stable_event_id("task-1", 7, "provider.completed", "request-1");
        let replay = stable_event_id("task-1", 7, "provider.completed", "request-1");
        let next = stable_event_id("task-1", 8, "provider.completed", "request-1");

        assert_ne!(
            first_fact.payload_digest,
            counterfactual_fact.payload_digest
        );
        assert_eq!(first_fact.event_id, counterfactual_fact.event_id);
        assert_eq!(first, replay);
        assert_ne!(first, next);
        assert!(first.starts_with("mainchat_event:v2:sha256:"));
        assert!(!first.contains("task-1"));
        assert!(!first.contains("request-1"));
        assert!(!first.contains("provider.completed"));
    }

    #[test]
    fn legacy_v4_payload_with_v1_identity_upgrades_atomically_without_raw_ascii_components() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy-v1-event-identity.db");
        let original = {
            let store = MainChatAgentEventStore::new(&path).unwrap();
            store
                .append(MainChatAgentEventDraft {
                    task_session_id: "task-legacy-v1-identity".into(),
                    run_id: "run-legacy-v1-identity".into(),
                    event_type: "diagnostic.created".into(),
                    object_type: "diagnostic".into(),
                    object_id: "gap-legacy-v1-identity".into(),
                    created_at: Utc::now(),
                    source: "diagnostic".into(),
                    payload: json!({
                        "gapId": "gap-legacy-v1-identity",
                        "gapCode": "legacy_v1_identity_fixture",
                    }),
                    backfilled: false,
                })
                .unwrap()
        };
        let legacy_event_id =
            "mainchat_event:task-legacy-v1-identity:1:diagnostic.created:gap-legacy-v1-identity";
        let legacy_private = "REAL_V4_PRIVATE_PAYLOAD_FIXTURE";
        let legacy_payload_json = serde_json::json!({
            "gapId": "gap-legacy-v1-identity",
            "gapCode": "legacy_v1_identity_fixture",
            "legacyPrivate": legacy_private,
        })
        .to_string();
        let legacy_payload_digest =
            stored_event_payload_digest("diagnostic.created", 1, &legacy_payload_json);
        let connection = Connection::open(&path).unwrap();
        drop_event_integrity_triggers(&connection).unwrap();
        connection
            .execute("DELETE FROM main_chat_agent_event_immutable_identities", [])
            .unwrap();
        connection
            .execute(
                "UPDATE main_chat_agent_events
                 SET event_id = ?1, payload_json = ?2, payload_digest = ?3,
                     payload_minimized_version = 4
                 WHERE event_id = ?4",
                params![
                    legacy_event_id,
                    legacy_payload_json,
                    legacy_payload_digest,
                    original.event_id
                ],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM main_chat_agent_event_store_metadata
                 WHERE key = 'event_identity_version'",
                [],
            )
            .unwrap();
        drop(connection);

        let reopened = MainChatAgentEventStore::new(&path).unwrap();
        let events = reopened.list("task-legacy-v1-identity", 0, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, original.event_id);
        assert!(!events[0].event_id.contains("task-legacy-v1-identity"));
        assert!(!events[0].event_id.contains("gap-legacy-v1-identity"));
        assert!(!serde_json::to_string(&events[0].payload)
            .unwrap()
            .contains(legacy_private));
        assert!(reopened
            .list_identity_quarantine_receipts(10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn invalid_legacy_identity_is_quarantined_as_a_read_only_receipt() {
        const PRIVATE_SOURCE: &str = "private API key copied as source";
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy-invalid-event-identity.db");
        let original = {
            let store = MainChatAgentEventStore::new(&path).unwrap();
            store
                .append(MainChatAgentEventDraft {
                    task_session_id: "task-legacy-invalid-identity".into(),
                    run_id: "run-legacy-invalid-identity".into(),
                    event_type: "diagnostic.created".into(),
                    object_type: "diagnostic".into(),
                    object_id: "gap-legacy-invalid-identity".into(),
                    created_at: Utc::now(),
                    source: "diagnostic".into(),
                    payload: json!({
                        "gapId": "gap-legacy-invalid-identity",
                        "gapCode": "legacy_invalid_identity_fixture",
                    }),
                    backfilled: false,
                })
                .unwrap()
        };
        let connection = Connection::open(&path).unwrap();
        drop_event_integrity_triggers(&connection).unwrap();
        connection
            .execute("DELETE FROM main_chat_agent_event_immutable_identities", [])
            .unwrap();
        connection
            .execute(
                "UPDATE main_chat_agent_events
                 SET event_id = 'legacy-invalid-identity-event', source = ?1
                 WHERE event_id = ?2",
                params![PRIVATE_SOURCE, original.event_id],
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM main_chat_agent_event_store_metadata
                 WHERE key = 'event_identity_version'",
                [],
            )
            .unwrap();
        drop(connection);

        let reopened = MainChatAgentEventStore::new(&path).unwrap();
        assert!(reopened
            .list("task-legacy-invalid-identity", 0, 10)
            .unwrap()
            .is_empty());
        let receipts = reopened.list_identity_quarantine_receipts(10).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].event_count, 1);
        assert_eq!(receipts[0].reason_code, "legacy_identity_invalid");
        assert!(is_exact_hmac_digest(&receipts[0].task_identity_digest));
        assert!(is_exact_hmac_digest(&receipts[0].row_set_digest));
        assert!(receipts[0]
            .quarantine_id
            .starts_with("mainchat_event_quarantine:v2:"));
        let serialized = format!("{receipts:?}");
        assert!(!serialized.contains(PRIVATE_SOURCE));
        assert!(!serialized.contains("task-legacy-invalid-identity"));
    }

    #[test]
    fn quarantine_row_set_receipt_changes_when_the_corrupt_row_changes() {
        fn quarantine_for_source(path: &std::path::Path, source: &str) -> String {
            let original = {
                let store = MainChatAgentEventStore::new(path).unwrap();
                store
                    .append(MainChatAgentEventDraft {
                        task_session_id: "task-quarantine-counterfactual".into(),
                        run_id: "run-quarantine-counterfactual".into(),
                        event_type: "diagnostic.created".into(),
                        object_type: "diagnostic".into(),
                        object_id: "gap-quarantine-counterfactual".into(),
                        created_at: DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                            .unwrap()
                            .with_timezone(&Utc),
                        source: "diagnostic".into(),
                        payload: json!({
                            "gapId": "gap-quarantine-counterfactual",
                            "gapCode": "quarantine_counterfactual",
                        }),
                        backfilled: false,
                    })
                    .unwrap()
            };
            let connection = Connection::open(path).unwrap();
            drop_event_integrity_triggers(&connection).unwrap();
            connection
                .execute("DELETE FROM main_chat_agent_event_immutable_identities", [])
                .unwrap();
            connection
                .execute(
                    "UPDATE main_chat_agent_events SET event_id = 'legacy-invalid-event', source = ?1
                     WHERE event_id = ?2",
                    params![source, original.event_id],
                )
                .unwrap();
            connection
                .execute(
                    "DELETE FROM main_chat_agent_event_store_metadata
                     WHERE key = 'event_identity_version'",
                    [],
                )
                .unwrap();
            drop(connection);

            MainChatAgentEventStore::new(path)
                .unwrap()
                .list_identity_quarantine_receipts(1)
                .unwrap()
                .remove(0)
                .row_set_digest
        }

        let directory = tempfile::tempdir().unwrap();
        let first = quarantine_for_source(&directory.path().join("first.db"), "private source A");
        let second = quarantine_for_source(&directory.path().join("second.db"), "private source B");
        assert!(is_exact_hmac_digest(&first));
        assert!(is_exact_hmac_digest(&second));
        assert_ne!(first, second);
    }

    #[test]
    fn provider_and_tool_lifecycle_schemas_reject_cross_event_payload_fields() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let provider_error = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-schema-cross-provider".into(),
                run_id: "run-schema-cross-provider".into(),
                event_type: "provider.started".into(),
                object_type: "provider_request".into(),
                object_id: "request-schema-cross-provider".into(),
                created_at: Utc::now(),
                source: "provider_adapter".into(),
                payload: json!({
                    "status": "started",
                    "requestId": "request-schema-cross-provider",
                    "provider": "provider-a",
                    "model": "model-a",
                    "receiptId": "tool-receipt-must-not-fit-provider",
                }),
                backfilled: false,
            })
            .expect_err("a tool receipt field cannot be persisted as a provider fact");
        assert!(provider_error
            .to_string()
            .contains("unexpected_payload_field:receiptId"));

        let mut tool_payload = tool_receipt_payload(
            "receipt-schema-cross-tool",
            "run-schema-cross-tool",
            "started",
        );
        tool_payload["provider"] = json!("provider-must-not-fit-tool");
        let tool_error = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-schema-cross-tool".into(),
                run_id: "run-schema-cross-tool".into(),
                event_type: "tool.started".into(),
                object_type: "tool_execution_receipt".into(),
                object_id: "receipt-schema-cross-tool".into(),
                created_at: Utc::now(),
                source: "openlife_turn_runtime".into(),
                payload: tool_payload,
                backfilled: false,
            })
            .expect_err("a provider field cannot be persisted as a tool receipt");
        assert!(tool_error
            .to_string()
            .contains("unexpected_payload_field:provider"));
        assert!(store
            .list("task-schema-cross-provider", 0, 10)
            .unwrap()
            .is_empty());
        assert!(store
            .list("task-schema-cross-tool", 0, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn event_type_and_status_cannot_contradict_each_other() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let failed_as_interrupted = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-status-contradiction".into(),
                run_id: "run-status-contradiction".into(),
                event_type: "failed".into(),
                object_type: "turn".into(),
                object_id: "turn-status-contradiction".into(),
                created_at: Utc::now(),
                source: "openlife_turn_runtime".into(),
                payload: json!({"status": "interrupted"}),
                backfilled: false,
            })
            .expect_err("failed event cannot carry interrupted status");
        assert!(failed_as_interrupted
            .to_string()
            .contains("status_mismatch:failed"));

        let accepted_as_rejected = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-proposal-status-contradiction".into(),
                run_id: "run-proposal-status-contradiction".into(),
                event_type: "proposal.accepted".into(),
                object_type: "proposal".into(),
                object_id: "proposal-status-contradiction".into(),
                created_at: Utc::now(),
                source: "proposal_store".into(),
                payload: json!({
                    "proposalId": "proposal-status-contradiction",
                    "status": "rejected",
                }),
                backfilled: false,
            })
            .expect_err("accepted event cannot carry rejected status");
        assert!(accepted_as_rejected
            .to_string()
            .contains("status_mismatch:accepted"));
    }

    #[test]
    fn event_schema_rejects_wrong_types_unbounded_arrays_and_unknown_event_types() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let wrong_type = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-schema-wrong-type".into(),
                run_id: "run-schema-wrong-type".into(),
                event_type: "task.updated".into(),
                object_type: "task".into(),
                object_id: "task-schema-wrong-type".into(),
                created_at: Utc::now(),
                source: "task_control".into(),
                payload: json!({
                    "taskSessionId": "task-schema-wrong-type",
                    "status": false,
                }),
                backfilled: false,
            })
            .expect_err("status must match the event-specific string schema");
        assert!(wrong_type
            .to_string()
            .contains("invalid_field_type_or_bound:status"));

        let too_many_controls = (0..=MAX_METADATA_ARRAY_ITEMS)
            .map(|index| format!("control-{index}"))
            .collect::<Vec<_>>();
        let unbounded = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-schema-unbounded".into(),
                run_id: "run-schema-unbounded".into(),
                event_type: "task.updated".into(),
                object_type: "task".into(),
                object_id: "task-schema-unbounded".into(),
                created_at: Utc::now(),
                source: "task_control".into(),
                payload: json!({
                    "taskSessionId": "task-schema-unbounded",
                    "status": "answering",
                    "controls": too_many_controls,
                }),
                backfilled: false,
            })
            .expect_err("event arrays have an explicit item bound");
        assert!(unbounded
            .to_string()
            .contains("invalid_field_type_or_bound:controls"));

        let unknown = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-schema-unknown".into(),
                run_id: "run-schema-unknown".into(),
                event_type: "future.unversioned".into(),
                object_type: "future".into(),
                object_id: "future-object".into(),
                created_at: Utc::now(),
                source: "future".into(),
                payload: json!({"status": "completed"}),
                backfilled: false,
            })
            .expect_err("new event types require an explicit versioned schema");
        assert!(unknown.to_string().contains("unsupported_event_type"));
    }

    #[test]
    fn evidence_reference_array_with_source_detail_is_redacted_without_failing_the_turn() {
        const SOURCE_DETAIL: &str = "--- private.txt\n+++ private.txt\n@@\n+diagnosis details";
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-redacted-evidence-array".into(),
                run_id: "run-redacted-evidence-array".into(),
                event_type: "proposal.created".into(),
                object_type: "proposal".into(),
                object_id: "proposal-redacted-evidence-array".into(),
                created_at: Utc::now(),
                source: "proposal_store".into(),
                payload: json!({
                    "proposalId": "proposal-redacted-evidence-array",
                    "proposalType": "memory_write",
                    "evidenceIds": [SOURCE_DETAIL],
                    "actionIds": [],
                }),
                backfilled: false,
            })
            .expect("invalidly labeled source detail must become unknown, not break the turn");
        assert_eq!(event.payload["evidenceIds"]["redacted"], true);
        assert_eq!(event.payload["evidenceIds"]["valueType"], "array");
        assert_eq!(
            event.payload["evidenceIds"]["digestScope"],
            "keyed_input_value_v1"
        );
        assert!(event.payload["evidenceIds"]["digest"]
            .as_str()
            .is_some_and(is_exact_hmac_digest));
        assert!(!serde_json::to_string(&event.payload)
            .unwrap()
            .contains(SOURCE_DETAIL));
    }

    fn cancellation_event_inputs(cancellation_id: &str) -> Vec<MainChatAgentRuntimeEventInput> {
        vec![
            MainChatAgentRuntimeEventInput::new(
                "cancel_requested",
                "turn",
                cancellation_id,
                "openlife_turn_runtime",
                json!({
                    "status": "cancel_requested",
                    "cancellationId": cancellation_id,
                    "durableCommitAllowedAfterCancel": false,
                    "localWaitAborted": true,
                    "remoteCancellationConfirmed": false,
                }),
            ),
            MainChatAgentRuntimeEventInput::new(
                "local_aborted",
                "turn",
                cancellation_id,
                "openlife_turn_runtime",
                json!({
                    "status": "local_aborted",
                    "cancellationId": cancellation_id,
                    "durableCommitAllowedAfterCancel": false,
                }),
            ),
        ]
    }

    fn tool_receipt_payload(receipt_id: &str, run_id: &str, status: &str) -> Value {
        json!({
            "status": status,
            "receiptId": receipt_id,
            "requestId": receipt_id,
            "sourceRunId": run_id,
            "manifestId": "mcp:typed-tool",
            "requestDigest": format!("sha256:{}", "c".repeat(64)),
            "actionEffect": "external_mutation",
            "idempotencyContract": "non_idempotent",
            "dispatchKind": "mcp_stdio",
            "dispatchAttemptCount": 1,
            "dispatchObserved": true,
            "transportStatus": if status == "started" { "dispatched" } else { "local_aborted" },
            "effectStatus": if status == "started" { "not_attempted" } else { "unknown" },
            "executionOutcome": if status == "started" { "not_observed" } else { "unknown" },
            "auditPersistenceStatus": if status == "started" { "pending" } else { "unknown" },
        })
    }

    #[tokio::test]
    async fn d067_durable_tool_lifecycle_preserves_failed_audit_disposition() {
        let run_id = uuid::Uuid::new_v4().to_string();
        let mut registry = openlife_core::mcp::McpRegistry::new();
        registry.register_builtin(
            openlife_core::tool_manifest::ToolManifest {
                id: "d067.event.read".into(),
                name: "d067.event.read".into(),
                description: "D067 durable event fixture.".into(),
                parameters: json!({"type": "object"}),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: openlife_core::tool_manifest::ToolSource::BuiltIn,
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                idempotency_contract:
                    openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
                tags: vec![],
            },
            Box::new(|_| Ok(json!({"ok": true}).to_string())),
        );
        let permission_store =
            openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap();
        let audit_store = openlife_core::mcp_audit::McpAuditStore::unavailable_sentinel(
            "d067_event_audit_failure",
        );
        let privacy_engine = openlife_core::privacy::PrivacyEngine::new();
        let owner_store = openlife_core::agent::AgentRunStore::new_in_memory().unwrap();
        let mut owner_run =
            openlife_core::agent::AgentRun::new_tool_execution_run("d067.event.read");
        owner_run.id = run_id.clone();
        owner_store.create_run(&owner_run).unwrap();
        let context = openlife_core::agent::ActionExecutionContext::new(
            &registry,
            &permission_store,
            &audit_store,
            &privacy_engine,
            &[],
        )
        .with_agent_run_store(&owner_store);
        let result = openlife_core::agent::ToolGateway::from_executor_config(Default::default())
            .execute(
                openlife_core::agent::AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "d067.event.read".into(),
                    input: json!({}),
                    source_run_id: Some(run_id.clone()),
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("tool result remains available after audit failure");
        assert_eq!(
            result.execution_receipt.audit_persistence_status,
            ToolAuditPersistenceStatus::Failed
        );

        let inputs = main_chat_tool_receipt_event_inputs(
            &run_id,
            &[result.execution_receipt],
            "openlife_turn_runtime",
        )
        .expect("project typed lifecycle");
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].event_type, "tool.started");
        assert_eq!(inputs[0].payload["auditPersistenceStatus"], "pending");
        assert_eq!(inputs[1].event_type, "tool.completed");
        assert_eq!(inputs[1].payload["auditPersistenceStatus"], "failed");

        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let durable =
            append_main_chat_tool_receipt_event_batch_in_store(&store, &run_id, &run_id, inputs)
                .expect("persist typed lifecycle atomically");
        assert_eq!(durable.len(), 2);
        assert_eq!(durable[0].payload["auditPersistenceStatus"], "pending");
        assert_eq!(durable[1].payload["auditPersistenceStatus"], "failed");
    }

    #[test]
    fn plan_and_tool_fact_enums_reject_unregistered_states() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let plan_error = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-plan-closed-status".into(),
                run_id: "run-plan-closed-status".into(),
                event_type: "plan.reviewed".into(),
                object_type: "plan_review".into(),
                object_id: "review-plan-closed-status".into(),
                created_at: Utc::now(),
                source: "plan_runtime".into(),
                payload: json!({
                    "reviewId": "review-plan-closed-status",
                    "planId": "plan-closed-status",
                    "planSessionId": "plan-session-closed-status",
                    "planStatus": "looks_done",
                }),
                backfilled: false,
            })
            .expect_err("planStatus is a closed durable fact enum");
        assert!(plan_error
            .to_string()
            .contains("invalid_field_type_or_bound:planStatus"));

        for (index, field, invalid_value) in [
            (0, "actionEffect", "unsafe_magic"),
            (1, "idempotencyContract", "probably"),
            (2, "dispatchKind", "teleport"),
            (3, "transportStatus", "maybe"),
            (4, "effectStatus", "eventually"),
            (5, "executionOutcome", "claimed_success"),
        ] {
            let receipt_id = format!("receipt-closed-tool-enum-{index}");
            let run_id = format!("run-closed-tool-enum-{index}");
            let mut payload = tool_receipt_payload(&receipt_id, &run_id, "started");
            payload[field] = json!(invalid_value);
            let error = store
                .append(MainChatAgentEventDraft {
                    task_session_id: format!("task-closed-tool-enum-{index}"),
                    run_id,
                    event_type: "tool.started".into(),
                    object_type: "tool_execution_receipt".into(),
                    object_id: receipt_id,
                    created_at: Utc::now(),
                    source: "tool_gateway".into(),
                    payload,
                    backfilled: false,
                })
                .expect_err("tool receipt semantics are closed durable fact enums");
            assert!(
                error
                    .to_string()
                    .contains(&format!("invalid_field_type_or_bound:{field}")),
                "{field}: {error}"
            );
        }
    }

    #[test]
    fn tool_receipt_lifecycle_requires_one_start_and_one_matching_terminal_schema() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-tool-lifecycle";
        let run_id = "run-tool-lifecycle";
        let receipt_id = "receipt-tool-lifecycle";
        let terminal_without_start = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.remote_unknown",
                "tool_execution_receipt",
                receipt_id,
                "openlife_turn_runtime",
                {
                    let mut payload = tool_receipt_payload(receipt_id, run_id, "remote_unknown");
                    payload["transportStatus"] = json!("remote_unknown");
                    payload
                },
            )],
        );
        assert!(terminal_without_start
            .unwrap_err()
            .to_string()
            .contains("terminal_without_start"));
        assert!(store.list(task_id, 0, 10).unwrap().is_empty());

        let mut payload_with_arguments = tool_receipt_payload(receipt_id, run_id, "started");
        payload_with_arguments["arguments"] = json!({"must": "be rejected"});
        let payload_copy = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.started",
                "tool_execution_receipt",
                receipt_id,
                "openlife_turn_runtime",
                payload_with_arguments,
            )],
        );
        assert!(payload_copy
            .unwrap_err()
            .to_string()
            .contains("unexpected_payload_field:arguments"));
        assert!(store.list(task_id, 0, 10).unwrap().is_empty());

        let started_at = Utc::now();
        let events = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![
                MainChatAgentRuntimeEventInput::new(
                    "tool.started",
                    "tool_execution_receipt",
                    receipt_id,
                    "openlife_turn_runtime",
                    tool_receipt_payload(receipt_id, run_id, "started"),
                )
                .with_occurred_at(started_at),
                MainChatAgentRuntimeEventInput::new(
                    "tool.remote_unknown",
                    "tool_execution_receipt",
                    receipt_id,
                    "openlife_turn_runtime",
                    tool_receipt_payload(receipt_id, run_id, "remote_unknown"),
                )
                .with_occurred_at(started_at + chrono::Duration::milliseconds(1)),
            ],
        )
        .expect("persist typed tool lifecycle");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].payload["receiptId"], receipt_id);
        assert_eq!(events[1].payload["sourceRunId"], run_id);
        assert_eq!(events[1].payload["actionEffect"], "external_mutation");
        assert_eq!(events[1].payload["idempotencyContract"], "non_idempotent");
        assert_eq!(
            events[1].payload["requestDigest"],
            format!("sha256:{}", "c".repeat(64))
        );
        assert_eq!(events[1].payload["effectStatus"], "unknown");
        assert!(events[1].payload.get("arguments").is_none());
        assert!(events[1].payload.get(UNRECOGNIZED_FIELDS_RECEIPT).is_none());

        let conflict = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.completed",
                "tool_execution_receipt",
                receipt_id,
                "openlife_turn_runtime",
                tool_receipt_payload(receipt_id, run_id, "completed"),
            )
            .with_occurred_at(started_at + chrono::Duration::milliseconds(2))],
        );
        assert!(conflict
            .unwrap_err()
            .to_string()
            .contains("conflicting_terminal"));
        assert_eq!(store.list(task_id, 0, 10).unwrap().len(), 2);
    }

    #[test]
    fn regular_non_cancelled_remote_unknown_receipt_materializes_one_durable_lifecycle() {
        let receipt =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_remote_unknown(
                Some("run-regular-tool-timeout".into()),
                Some("web.fetch".into()),
                "regular-tool-timeout".into(),
                openlife_core::tool_execution_receipt::ToolActionEffect::ReadOnly,
                openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
            );
        let inputs = main_chat_tool_receipt_event_inputs(
            "run-regular-tool-timeout",
            &[receipt.clone()],
            "openlife_turn_runtime.regular_tool_receipt",
        )
        .expect("materialize regular tool receipt inputs");

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].event_type, "tool.started");
        assert_eq!(inputs[1].event_type, "tool.remote_unknown");
        assert_eq!(inputs[0].payload["receiptId"], receipt.receipt_id);
        assert_eq!(inputs[0].payload["dispatchKind"], "network");
        assert_eq!(inputs[0].payload["dispatchAttemptCount"], 1);
        assert_eq!(inputs[1].payload["status"], "remote_unknown");

        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let events = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-regular-tool-timeout",
            "run-regular-tool-timeout",
            inputs,
        )
        .expect("persist regular non-cancelled tool lifecycle");
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].event_type, "tool.remote_unknown");
    }

    #[test]
    fn regular_attempt_without_concrete_dispatch_never_materializes_tool_started() {
        let receipt =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_ambiguous_network_attempt(
                Some("run-regular-ambiguous-attempt".into()),
                Some("web.fetch".into()),
                "regular-ambiguous-attempt".into(),
                openlife_core::tool_execution_receipt::ToolActionEffect::ReadOnly,
                openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
            );
        assert_eq!(receipt.dispatch_attempt_count, 1);
        assert!(!receipt.dispatch_observed);
        assert!(receipt.dispatched_at.is_none());

        let inputs = main_chat_tool_receipt_event_inputs(
            "run-regular-ambiguous-attempt",
            &[receipt.clone()],
            "openlife_turn_runtime.regular_tool_receipt",
        )
        .expect("materialize ambiguous regular receipt inputs");

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].event_type, "tool.dispatch_ambiguous");
        assert_eq!(inputs[0].payload["dispatchObserved"], false);
        assert_eq!(inputs[0].payload["dispatchAttemptCount"], 1);
        assert!(inputs[0].payload["dispatchedAt"].is_null());
        assert_eq!(inputs[1].event_type, "tool.remote_unknown");
        assert!(inputs
            .iter()
            .all(|input| input.event_type != "tool.started"));

        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let events = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-regular-ambiguous-attempt",
            "run-regular-ambiguous-attempt",
            inputs,
        )
        .expect("persist ambiguous attempt plus unknown terminal");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "tool.dispatch_ambiguous");
        assert_eq!(events[1].event_type, "tool.remote_unknown");
    }

    #[test]
    fn regular_response_observed_receipt_materializes_completed_lifecycle() {
        let receipt =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
                Some("run-regular-tool-completed".into()),
                Some("file.read".into()),
                "regular-tool-completed".into(),
                true,
            );

        let inputs = main_chat_tool_receipt_event_inputs(
            "run-regular-tool-completed",
            &[receipt],
            "openlife_turn_runtime.regular_tool_receipt",
        )
        .expect("materialize completed regular tool lifecycle");

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].event_type, "tool.started");
        assert_eq!(inputs[1].event_type, "tool.completed");
        assert_eq!(inputs[0].payload["dispatchKind"], "local");
        assert_eq!(inputs[1].payload["status"], "completed");
    }

    #[test]
    fn terminal_projection_keeps_the_first_durable_start_across_http_retries() {
        let task_id = "task-tool-retry-start-owner";
        let run_id = "run-tool-retry-start-owner";
        let mut terminal =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
                Some(run_id.into()),
                Some("web.search".into()),
                "retry-start-owner".into(),
                true,
            );
        terminal.dispatch_attempt_count = 2;

        let mut first_start = terminal.clone();
        first_start.dispatch_attempt_count = 1;
        first_start.transport_status = ToolTransportStatus::Dispatched;
        first_start.effect_status = ToolEffectStatus::NotAttempted;
        first_start.execution_outcome = ToolExecutionOutcome::NotObserved;
        first_start.response_observed_at = None;
        first_start.finished_at = None;

        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![main_chat_tool_started_event_input(
                run_id,
                &first_start,
                "openlife_turn_runtime.tool_started",
            )
            .unwrap()],
        )
        .unwrap();

        let projected = main_chat_tool_receipt_event_inputs(
            run_id,
            std::slice::from_ref(&terminal),
            "openlife_turn_runtime.regular_tool_receipt",
        )
        .unwrap();
        assert_eq!(projected[0].event_type, "tool.started");
        assert_eq!(projected[0].payload["dispatchAttemptCount"], 2);
        let appended =
            append_main_chat_tool_receipt_event_batch_in_store(&store, task_id, run_id, projected)
                .unwrap();
        assert_eq!(appended.len(), 1);
        assert_eq!(appended[0].event_type, "tool.completed");
        let events = store.list(task_id, 0, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "tool.started");
        assert_eq!(events[0].payload["dispatchAttemptCount"], 1);
        assert_eq!(events[1].event_type, "tool.completed");
        assert_eq!(events[1].payload["dispatchAttemptCount"], 2);
    }

    #[test]
    fn response_failure_and_effect_uncertainty_are_not_projected_as_completion() {
        let read_failure =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
                Some("run-read-failed".into()),
                Some("file.read".into()),
                "read-failed".into(),
                false,
            );
        let failed = project_main_chat_tool_receipt("run-read-failed", &read_failure)
            .unwrap()
            .unwrap();
        assert_eq!(failed.terminal_event_type, "tool.failed");
        assert_eq!(failed.terminal_status, "failed");

        let effect_unknown = openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_mutation_failure(
            Some("run-effect-unknown".into()),
            Some("file.write".into()),
            "effect-unknown".into(),
        );
        let uncertain = project_main_chat_tool_receipt("run-effect-unknown", &effect_unknown)
            .unwrap()
            .unwrap();
        assert_eq!(uncertain.terminal_event_type, "tool.effect_unknown");
        assert_ne!(uncertain.terminal_event_type, "tool.remote_unknown");
    }

    #[test]
    fn explicitly_observed_prepared_and_started_events_do_not_invent_a_terminal() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let receipt =
            openlife_core::tool_execution_receipt::ToolExecutionReceipt::test_observed_local_read(
                Some("run-crash-after-dispatch".into()),
                Some("file.read".into()),
                "crash-after-dispatch".into(),
                true,
            );
        let mut started = receipt.clone();
        started.transport_status = ToolTransportStatus::Dispatched;
        started.execution_outcome = ToolExecutionOutcome::NotObserved;
        started.effect_status = ToolEffectStatus::NotAttempted;
        started.dispatch_attempt_count = 1;
        started.response_observed_at = None;
        started.finished_at = None;
        let prepared = MainChatAgentRuntimeEventInput::new(
            "tool.dispatch_prepared",
            "tool_execution_receipt",
            &receipt.receipt_id,
            "test.write_ahead",
            json!({
                "receiptId": receipt.receipt_id,
                "requestId": receipt.receipt_id,
                "sourceRunId": "run-crash-after-dispatch",
                "manifestId": "file.read",
                "toolName": "file.read",
                "requestDigest": receipt.request_digest,
                "manifestContractDigest": format!("sha256:{}", "d".repeat(64)),
                "inputHash": format!("sha256:{}", "e".repeat(64)),
                "inputLengthBytes": 16,
                "actionEffect": "read_only",
                "idempotencyContract": "idempotent",
                "dispatchProcessRisk": "process_bound",
                "mayOutliveLocalProcess": false,
                "effectMaySurviveLocalProcess": false,
                "status": "prepared",
            }),
        )
        .with_occurred_at(receipt.started_at);
        let start = main_chat_tool_started_event_input(
            "run-crash-after-dispatch",
            &started,
            "test.adapter_boundary",
        )
        .unwrap();
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-crash-after-dispatch",
            "run-crash-after-dispatch",
            vec![prepared, start],
        )
        .unwrap();
        let durable = store.list("task-crash-after-dispatch", 0, 10).unwrap();
        assert_eq!(durable.len(), 2);
        assert_eq!(durable[0].event_type, "tool.dispatch_prepared");
        assert_eq!(durable[1].event_type, "tool.started");
        assert!(durable.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "tool.completed" | "tool.failed" | "tool.effect_unknown" | "tool.remote_unknown"
        )));

        let reconciled = store
            .reconcile_orphaned_tool_attempts_after_restart(10)
            .unwrap();
        assert_eq!(reconciled.examined, 1);
        assert_eq!(reconciled.local_aborted, 1);
        let after_restart = store.list("task-crash-after-dispatch", 0, 10).unwrap();
        assert_eq!(after_restart.len(), 3);
        assert_eq!(after_restart[2].event_type, "tool.local_aborted");
        assert_eq!(after_restart[2].payload["dispatchObserved"], true);
        assert_eq!(after_restart[2].payload["dispatchKind"], "local");
    }

    #[test]
    fn prepared_only_restart_reconciliation_is_conservative_bounded_and_idempotent() {
        let cases = [
            (
                "remote-read",
                "read_only",
                "may_outlive_local_process",
                true,
                false,
                "tool.remote_unknown",
                "remote_unknown",
                "not_attempted",
            ),
            (
                "local-effect",
                "local_mutation",
                "process_bound",
                false,
                true,
                "tool.effect_unknown",
                "local_aborted",
                "unknown",
            ),
            (
                "local-read",
                "read_only",
                "process_bound",
                false,
                false,
                "tool.local_aborted",
                "local_aborted",
                "not_attempted",
            ),
        ];

        for (
            label,
            action_effect,
            process_risk,
            may_outlive,
            effect_may_survive,
            expected_terminal,
            expected_transport,
            expected_effect,
        ) in cases
        {
            let store = MainChatAgentEventStore::new_in_memory().unwrap();
            let task_id = format!("task-prepared-only-{label}");
            let run_id = format!("run-prepared-only-{label}");
            let receipt_id = format!("receipt-prepared-only-{label}");
            append_main_chat_agent_runtime_event_batch_in_store(
                &store,
                &task_id,
                &run_id,
                vec![MainChatAgentRuntimeEventInput::new(
                    "tool.dispatch_prepared",
                    "tool_execution_receipt",
                    &receipt_id,
                    "test.prepared_fence",
                    json!({
                        "receiptId": receipt_id,
                        "requestId": receipt_id,
                        "sourceRunId": run_id,
                        "manifestId": format!("manifest-{label}"),
                        "toolName": format!("tool-{label}"),
                        "requestDigest": format!("sha256:{}", "a".repeat(64)),
                        "manifestContractDigest": format!("sha256:{}", "b".repeat(64)),
                        "inputHash": format!("sha256:{}", "c".repeat(64)),
                        "inputLengthBytes": 8,
                        "actionEffect": action_effect,
                        "idempotencyContract": "idempotent",
                        "dispatchProcessRisk": process_risk,
                        "mayOutliveLocalProcess": may_outlive,
                        "effectMaySurviveLocalProcess": effect_may_survive,
                        "status": "prepared",
                    }),
                )],
            )
            .unwrap();

            let first = store
                .reconcile_orphaned_tool_attempts_after_restart(10)
                .unwrap();
            assert_eq!(first.examined, 1, "{label}");
            assert!(!first.has_more, "{label}");
            let events = store.list(&task_id, 0, 10).unwrap();
            assert_eq!(events.len(), 3, "{label}");
            assert_eq!(events[1].event_type, "tool.dispatch_ambiguous", "{label}");
            assert_eq!(events[1].payload["dispatchObserved"], false, "{label}");
            assert_eq!(events[1].payload["dispatchAttemptCount"], 0, "{label}");
            assert!(events[1].payload["dispatchedAt"].is_null(), "{label}");
            assert_eq!(events[2].event_type, expected_terminal, "{label}");
            assert_eq!(
                events[2].payload["transportStatus"], expected_transport,
                "{label}"
            );
            assert_eq!(
                events[2].payload["effectStatus"], expected_effect,
                "{label}"
            );
            assert_eq!(events[2].payload["executionOutcome"], "unknown", "{label}");
            assert!(
                store
                    .pending_tool_queue_reconciliation_projections(10)
                    .unwrap()
                    .items
                    .is_empty(),
                "ordinary tool attempts must not create replay queue projections: {label}"
            );

            let second = store
                .reconcile_orphaned_tool_attempts_after_restart(10)
                .unwrap();
            assert_eq!(second.examined, 0, "{label}");
            assert_eq!(store.list(&task_id, 0, 10).unwrap().len(), 3, "{label}");
        }
    }

    #[test]
    fn live_not_dispatched_closure_excludes_prepared_restart_and_rejects_unsealed_receipts() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-live-not-dispatched";
        let run_id = "run-live-not-dispatched";
        let manifest_id = "mcp:live-not-dispatched";
        let registration = openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration::test_never_dispatched_read(
            Some(run_id.into()),
            Some(manifest_id.into()),
            "request-live-not-dispatched".into(),
        );
        let prepared_receipt = registration.snapshot();
        let replay_action_id = uuid::Uuid::new_v4().to_string();
        let replay_claim_id = uuid::Uuid::new_v4().to_string();
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.dispatch_prepared",
                "tool_execution_receipt",
                &prepared_receipt.receipt_id,
                "test.live_not_dispatched_prepared",
                json!({
                    "receiptId": prepared_receipt.receipt_id,
                    "requestId": prepared_receipt.receipt_id,
                    "sourceRunId": run_id,
                    "manifestId": manifest_id,
                    "toolName": "live_not_dispatched",
                    "requestDigest": prepared_receipt.request_digest,
                    "manifestContractDigest": format!("sha256:{}", "a".repeat(64)),
                    "inputHash": format!("sha256:{}", "b".repeat(64)),
                    "inputLengthBytes": 7,
                    "actionEffect": "read_only",
                    "idempotencyContract": "idempotent",
                    "dispatchProcessRisk": "process_bound",
                    "mayOutliveLocalProcess": false,
                    "effectMaySurviveLocalProcess": false,
                    "replayActionId": replay_action_id,
                    "replayClaimId": replay_claim_id,
                    "replayClaimOwnerGeneration": 1,
                    "replayAuthorityBinding": format!("hmac-sha256:{}", "c".repeat(64)),
                    "status": "prepared",
                }),
            )],
        )
        .unwrap();
        let receipt = registration.settle_after_runtime_failure();

        let closure = store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .unwrap()
            .expect("a matching prepared fact is explicitly resolved");
        assert_eq!(closure.event_type, "tool.not_dispatched");
        assert_eq!(closure.payload["dispatchAttemptCount"], 0);
        assert_eq!(closure.payload["dispatchObserved"], false);
        assert_eq!(closure.payload["transportStatus"], "not_attempted");
        assert_eq!(closure.payload["effectStatus"], "not_attempted");
        let forged_generic = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.not_dispatched",
                "tool_execution_receipt",
                &receipt.receipt_id,
                "openlife_turn_runtime.tool_not_dispatched",
                closure.payload.clone(),
            )
            .with_occurred_at(closure.created_at)],
        )
        .unwrap_err();
        assert!(forged_generic
            .to_string()
            .contains("not_dispatched_without_live_receipt"));
        assert!(store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .unwrap()
            .is_some());

        let restart = store
            .reconcile_orphaned_tool_attempts_after_restart(10)
            .unwrap();
        assert_eq!(restart.examined, 0);
        let events = store.list(task_id, 0, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "tool.dispatch_prepared");
        assert_eq!(events[1].event_type, "tool.not_dispatched");
        assert!(events.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "tool.started" | "tool.dispatch_ambiguous" | "tool.remote_unknown"
        )));
        let outbox = store
            .pending_tool_queue_reconciliation_projections(10)
            .unwrap();
        assert_eq!(outbox.items.len(), 1);
        assert_eq!(
            outbox.items[0].disposition,
            MainChatToolQueueReconciliationDisposition::EffectNotAttempted
        );

        let standalone_store = MainChatAgentEventStore::new_in_memory().unwrap();
        let standalone_task_id = "task-live-gateway-rejection";
        let standalone_run_id = "run-live-gateway-rejection";
        let standalone_registration = openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration::test_never_dispatched_read(
            Some(standalone_run_id.into()),
            Some("mcp:gateway-rejection".into()),
            "request-live-gateway-rejection".into(),
        );
        let standalone_receipt = standalone_registration.settle_after_runtime_failure();
        let standalone = standalone_store
            .append_live_not_dispatched_tool_receipt(
                standalone_task_id,
                standalone_run_id,
                &standalone_receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .unwrap()
            .expect("a sealed ToolGateway rejection is a standalone zero-dispatch terminal");
        assert_eq!(standalone.event_type, "tool.not_dispatched");
        assert!(standalone.payload.get("preparedEventId").is_none());
        assert_eq!(
            standalone_store
                .list(standalone_task_id, 0, 10)
                .unwrap()
                .len(),
            1
        );
        assert!(standalone_store
            .pending_tool_queue_reconciliation_projections(10)
            .unwrap()
            .items
            .is_empty());

        let restored: ToolExecutionReceipt =
            serde_json::from_value(serde_json::to_value(&receipt).unwrap()).unwrap();
        assert!(store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &restored,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .unwrap_err()
            .to_string()
            .contains("live_receipt_proof_invalid"));
        let attempted = ToolExecutionReceipt::test_ambiguous_network_attempt(
            Some(run_id.into()),
            Some(manifest_id.into()),
            "request-attempted".into(),
            ToolActionEffect::ReadOnly,
            openlife_core::tool_manifest::ToolIdempotencyContract::Idempotent,
        );
        assert!(store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &attempted,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .unwrap_err()
            .to_string()
            .contains("live_receipt_proof_invalid"));
    }

    #[test]
    fn live_not_dispatched_external_mutation_keeps_effect_not_attempted_despite_nominal_risk() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-live-not-dispatched-external-mutation";
        let run_id = "run-live-not-dispatched-external-mutation";
        let manifest_id = "mcp:live-not-dispatched-external-mutation";
        let registration = openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration::test_never_dispatched_external_mutation(
            Some(run_id.into()),
            Some(manifest_id.into()),
            "request-live-not-dispatched-external-mutation".into(),
        );
        let prepared_receipt = registration.snapshot();
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.dispatch_prepared",
                "tool_execution_receipt",
                &prepared_receipt.receipt_id,
                "test.live_not_dispatched_external_mutation_prepared",
                json!({
                    "receiptId": prepared_receipt.receipt_id,
                    "requestId": prepared_receipt.receipt_id,
                    "sourceRunId": run_id,
                    "manifestId": manifest_id,
                    "toolName": "live_not_dispatched_external_mutation",
                    "requestDigest": prepared_receipt.request_digest,
                    "manifestContractDigest": format!("sha256:{}", "a".repeat(64)),
                    "inputHash": format!("sha256:{}", "b".repeat(64)),
                    "inputLengthBytes": 7,
                    "actionEffect": "external_mutation",
                    "idempotencyContract": "non_idempotent",
                    "dispatchProcessRisk": "may_outlive_local_process",
                    "mayOutliveLocalProcess": true,
                    "effectMaySurviveLocalProcess": true,
                    "replayActionId": uuid::Uuid::new_v4().to_string(),
                    "replayClaimId": uuid::Uuid::new_v4().to_string(),
                    "replayClaimOwnerGeneration": 1,
                    "replayAuthorityBinding": format!("hmac-sha256:{}", "c".repeat(64)),
                    "status": "prepared",
                }),
            )],
        )
        .unwrap();
        let receipt = registration.settle_after_runtime_failure();
        let closure = store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .unwrap()
            .expect("live zero-attempt mutation closes its exact prepared fence");
        assert_eq!(closure.event_type, "tool.not_dispatched");
        assert_eq!(closure.payload["effectStatus"], "not_attempted");
        let outbox = store
            .pending_tool_queue_reconciliation_projections(10)
            .unwrap();
        assert_eq!(outbox.items.len(), 1);
        assert_eq!(
            outbox.items[0].resolution,
            openlife_core::agent::main_chat_agent_v1::ReplayPreparedToolResolution::NotDispatched
        );
        assert_eq!(
            outbox.items[0].disposition,
            MainChatToolQueueReconciliationDisposition::EffectNotAttempted
        );
    }

    #[test]
    fn failed_not_dispatched_persistence_leaves_prepared_for_conservative_restart() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-failed-not-dispatched-persistence";
        let run_id = "run-failed-not-dispatched-persistence";
        let registration = openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration::test_never_dispatched_read(
            Some(run_id.into()),
            Some("mcp:failed-not-dispatched-persistence".into()),
            "request-failed-not-dispatched-persistence".into(),
        );
        let prepared_receipt = registration.snapshot();
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.dispatch_prepared",
                "tool_execution_receipt",
                &prepared_receipt.receipt_id,
                "test.failed_not_dispatched_prepared",
                json!({
                    "receiptId": prepared_receipt.receipt_id,
                    "requestId": prepared_receipt.receipt_id,
                    "sourceRunId": run_id,
                    "manifestId": "mcp:failed-not-dispatched-persistence",
                    "toolName": "failed_not_dispatched_persistence",
                    "requestDigest": prepared_receipt.request_digest,
                    "manifestContractDigest": format!("sha256:{}", "d".repeat(64)),
                    "inputHash": format!("sha256:{}", "e".repeat(64)),
                    "inputLengthBytes": 9,
                    "actionEffect": "read_only",
                    "idempotencyContract": "idempotent",
                    "dispatchProcessRisk": "process_bound",
                    "mayOutliveLocalProcess": false,
                    "effectMaySurviveLocalProcess": false,
                    "status": "prepared",
                }),
            )],
        )
        .unwrap();
        let receipt = registration.settle_after_runtime_failure();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_test_not_dispatched_insert
                 BEFORE INSERT ON main_chat_agent_events
                 WHEN NEW.event_type = 'tool.not_dispatched'
                 BEGIN
                    SELECT RAISE(ABORT, 'injected tool.not_dispatched persistence failure');
                 END;",
            )
            .unwrap();
        }
        assert!(store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .is_err());
        assert_eq!(store.list(task_id, 0, 10).unwrap().len(), 1);
        {
            let conn = store.lock_conn().unwrap();
            conn.execute("DROP TRIGGER reject_test_not_dispatched_insert", [])
                .unwrap();
        }
        let restart = store
            .reconcile_orphaned_tool_attempts_after_restart(10)
            .unwrap();
        assert_eq!(restart.examined, 1);
        let events = store.list(task_id, 0, 10).unwrap();
        assert_eq!(events[1].event_type, "tool.dispatch_ambiguous");
        assert_eq!(events[2].event_type, "tool.local_aborted");
    }

    #[test]
    fn failed_not_dispatched_outbox_insert_rolls_back_the_resolution_event() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-failed-not-dispatched-outbox";
        let run_id = "run-failed-not-dispatched-outbox";
        let manifest_id = "mcp:failed-not-dispatched-outbox";
        let registration = openlife_core::tool_execution_receipt::ToolExecutionReceiptRegistration::test_never_dispatched_read(
            Some(run_id.into()),
            Some(manifest_id.into()),
            "request-failed-not-dispatched-outbox".into(),
        );
        let prepared_receipt = registration.snapshot();
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "tool.dispatch_prepared",
                "tool_execution_receipt",
                &prepared_receipt.receipt_id,
                "test.failed_not_dispatched_outbox_prepared",
                json!({
                    "receiptId": prepared_receipt.receipt_id,
                    "requestId": prepared_receipt.receipt_id,
                    "sourceRunId": run_id,
                    "manifestId": manifest_id,
                    "toolName": "failed_not_dispatched_outbox",
                    "requestDigest": prepared_receipt.request_digest,
                    "manifestContractDigest": format!("sha256:{}", "a".repeat(64)),
                    "inputHash": format!("sha256:{}", "b".repeat(64)),
                    "inputLengthBytes": 7,
                    "actionEffect": "read_only",
                    "idempotencyContract": "idempotent",
                    "dispatchProcessRisk": "process_bound",
                    "mayOutliveLocalProcess": false,
                    "effectMaySurviveLocalProcess": false,
                    "replayActionId": uuid::Uuid::new_v4().to_string(),
                    "replayClaimId": uuid::Uuid::new_v4().to_string(),
                    "replayClaimOwnerGeneration": 1,
                    "replayAuthorityBinding": format!("hmac-sha256:{}", "c".repeat(64)),
                    "status": "prepared",
                }),
            )],
        )
        .unwrap();
        let receipt = registration.settle_after_runtime_failure();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_test_tool_queue_outbox_insert
                 BEFORE INSERT ON main_chat_tool_queue_reconciliation_outbox
                 BEGIN
                    SELECT RAISE(ABORT, 'injected tool queue outbox persistence failure');
                 END;",
            )
            .unwrap();
        }
        assert!(store
            .append_live_not_dispatched_tool_receipt(
                task_id,
                run_id,
                &receipt,
                "openlife_turn_runtime.tool_not_dispatched",
            )
            .is_err());
        let before_restart = store.list(task_id, 0, 10).unwrap();
        assert_eq!(before_restart.len(), 1);
        assert_eq!(before_restart[0].event_type, "tool.dispatch_prepared");
        assert!(store
            .pending_tool_queue_reconciliation_projections(10)
            .unwrap()
            .items
            .is_empty());
        {
            let conn = store.lock_conn().unwrap();
            conn.execute("DROP TRIGGER reject_test_tool_queue_outbox_insert", [])
                .unwrap();
        }
        let restart = store
            .reconcile_orphaned_tool_attempts_after_restart(10)
            .unwrap();
        assert_eq!(restart.examined, 1);
        let events = store.list(task_id, 0, 10).unwrap();
        assert_eq!(events[1].event_type, "tool.dispatch_ambiguous");
        assert_eq!(events[1].payload["executionOutcome"], "unknown");
        assert_eq!(events[2].event_type, "tool.local_aborted");
    }

    #[test]
    fn replay_prepared_restart_reconciliation_enqueues_exact_durable_queue_projection() {
        let cases = [
            (
                "remote-mutation",
                "external_mutation",
                "may_outlive_local_process",
                true,
                true,
                MainChatToolQueueReconciliationDisposition::DispatchedUnknown,
            ),
            (
                "local-read",
                "read_only",
                "process_bound",
                false,
                false,
                MainChatToolQueueReconciliationDisposition::EffectNotAttempted,
            ),
        ];

        for (
            label,
            action_effect,
            process_risk,
            may_outlive,
            effect_may_survive,
            expected_disposition,
        ) in cases
        {
            let store = MainChatAgentEventStore::new_in_memory().unwrap();
            let task_id = format!("task-replay-prepared-{label}");
            let run_id = format!("run-replay-prepared-{label}");
            let receipt_id = format!("receipt-replay-prepared-{label}");
            let action_id = format!("action-replay-prepared-{label}");
            let claim_id = uuid::Uuid::new_v4().to_string();
            let claim_owner_generation = 7_u64;
            let replay_authority_binding = format!("hmac-sha256:{}", "d".repeat(64));
            let prepared = append_main_chat_agent_runtime_event_batch_in_store(
                &store,
                &task_id,
                &run_id,
                vec![MainChatAgentRuntimeEventInput::new(
                    "tool.dispatch_prepared",
                    "tool_execution_receipt",
                    &receipt_id,
                    "test.replay_prepared_fence",
                    json!({
                        "receiptId": receipt_id,
                        "requestId": receipt_id,
                        "sourceRunId": run_id,
                        "manifestId": format!("manifest-{label}"),
                        "toolName": format!("tool-{label}"),
                        "requestDigest": format!("sha256:{}", "a".repeat(64)),
                        "manifestContractDigest": format!("sha256:{}", "b".repeat(64)),
                        "inputHash": format!("sha256:{}", "c".repeat(64)),
                        "inputLengthBytes": 8,
                        "actionEffect": action_effect,
                        "idempotencyContract": "idempotent",
                        "dispatchProcessRisk": process_risk,
                        "mayOutliveLocalProcess": may_outlive,
                        "effectMaySurviveLocalProcess": effect_may_survive,
                        "replayActionId": action_id,
                        "replayClaimId": claim_id,
                        "replayClaimOwnerGeneration": claim_owner_generation,
                        "replayAuthorityBinding": replay_authority_binding.clone(),
                        "status": "prepared",
                    }),
                )],
            )
            .unwrap()
            .remove(0);

            let report = store
                .reconcile_orphaned_tool_attempts_after_restart(10)
                .unwrap();
            assert_eq!(report.examined, 1, "{label}");
            let batch = store
                .pending_tool_queue_reconciliation_projections(10)
                .unwrap();
            assert!(!batch.has_more, "{label}");
            assert_eq!(batch.items.len(), 1, "{label}");
            let projection = &batch.items[0];
            assert_eq!(projection.prepared_event_id, prepared.event_id, "{label}");
            assert!(
                is_exact_metadata_digest(&projection.prepared_payload_digest),
                "stored prepared payload digest must retain its exact typed digest form: {label}"
            );
            assert_eq!(projection.task_session_id, task_id, "{label}");
            assert_eq!(projection.run_id, run_id, "{label}");
            assert_eq!(projection.receipt_id, receipt_id, "{label}");
            assert_eq!(projection.replay_action_id, action_id, "{label}");
            assert_eq!(projection.replay_claim_id, claim_id, "{label}");
            assert_eq!(
                projection.replay_claim_owner_generation, claim_owner_generation,
                "{label}"
            );
            assert_eq!(
                projection.replay_authority_binding, replay_authority_binding,
                "{label}"
            );
            assert_eq!(projection.disposition, expected_disposition, "{label}");

            store
                .mark_tool_queue_reconciliation_projection_applied(projection)
                .unwrap();
            store
                .mark_tool_queue_reconciliation_projection_applied(projection)
                .expect("an exact already-applied projection is idempotent");
            assert!(store
                .pending_tool_queue_reconciliation_projections(10)
                .unwrap()
                .items
                .is_empty());

            let mut forged = projection.clone();
            forged.run_id.push_str("-forged");
            assert!(store
                .mark_tool_queue_reconciliation_projection_applied(&forged)
                .is_err());
        }
    }

    #[test]
    fn legacy_prepared_without_process_contract_reconciles_remote_unknown() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-legacy-prepared-process-contract";
        let run_id = "run-legacy-prepared-process-contract";
        let receipt_id = "receipt-legacy-prepared-process-contract";
        let created_at = Utc::now();
        let current = store
            .append(MainChatAgentEventDraft {
                task_session_id: task_id.into(),
                run_id: run_id.into(),
                event_type: "tool.dispatch_prepared".into(),
                object_type: "tool_execution_receipt".into(),
                object_id: receipt_id.into(),
                created_at,
                source: "openlife_turn_runtime.tool_dispatch_prepared".into(),
                payload: json!({
                    "receiptId": receipt_id,
                    "requestId": receipt_id,
                    "sourceRunId": run_id,
                    "manifestId": "legacy-local-read",
                    "toolName": "legacy-local-read",
                    "requestDigest": format!("sha256:{}", "a".repeat(64)),
                    "manifestContractDigest": format!("sha256:{}", "b".repeat(64)),
                    "inputHash": format!("sha256:{}", "c".repeat(64)),
                    "inputLengthBytes": 8,
                    "actionEffect": "read_only",
                    "idempotencyContract": "idempotent",
                    "dispatchProcessRisk": "process_bound",
                    "mayOutliveLocalProcess": false,
                    "effectMaySurviveLocalProcess": false,
                    "status": "prepared",
                }),
                backfilled: false,
            })
            .unwrap();

        {
            let conn = store.lock_conn().unwrap();
            drop_event_integrity_triggers(&conn).unwrap();
            let mut legacy_payload = current.payload.clone();
            let object = legacy_payload.as_object_mut().unwrap();
            object.remove("dispatchProcessRisk");
            object.remove("mayOutliveLocalProcess");
            object.remove("effectMaySurviveLocalProcess");
            let legacy_json = serde_json::to_string(&legacy_payload).unwrap();
            let legacy_digest =
                stored_event_payload_digest("tool.dispatch_prepared", 1, &legacy_json);
            conn.execute(
                "UPDATE main_chat_agent_events
                 SET payload_json = ?1, payload_digest = ?2, payload_minimized_version = 6
                 WHERE event_id = ?3",
                params![legacy_json, legacy_digest, current.event_id],
            )
            .unwrap();
            migrate_legacy_event_payloads(&conn, &store.digest_key).unwrap();
        }

        let report = store
            .reconcile_orphaned_tool_attempts_after_restart(10)
            .unwrap();
        assert_eq!(report.examined, 1);
        assert_eq!(report.remote_unknown, 1);
        assert_eq!(report.effect_unknown, 0);
        assert_eq!(report.local_aborted, 0);
        let events = store.list(task_id, 0, 10).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[1].event_type, "tool.dispatch_ambiguous");
        assert_eq!(events[1].payload["dispatchObserved"], false);
        assert_eq!(events[1].payload["dispatchAttemptCount"], 0);
        assert_eq!(
            events[1].payload["dispatchProcessRisk"],
            "may_outlive_local_process"
        );
        assert_eq!(events[1].payload["mayOutliveLocalProcess"], true);
        assert_eq!(events[1].payload["effectMaySurviveLocalProcess"], false);
        assert_eq!(events[2].event_type, "tool.remote_unknown");
        assert_eq!(events[2].payload["transportStatus"], "remote_unknown");
        assert_eq!(events[2].payload["effectStatus"], "not_attempted");
    }

    #[test]
    fn lifecycle_snapshot_captures_sequence_and_terminal_receipt_atomically() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-lifecycle-snapshot";
        let run_id = "run-lifecycle-snapshot";
        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![MainChatAgentRuntimeEventInput::new(
                "task.created",
                "task",
                task_id,
                "agent_ingress",
                json!({
                    "taskSessionId": task_id,
                    "runId": run_id,
                    "strategy": "direct_answer",
                }),
            )],
        )
        .unwrap();
        let before = store.turn_lifecycle_snapshot(task_id).unwrap();
        assert_eq!(before.latest_sequence, 1);
        assert_eq!(before.bound_run_id.as_deref(), Some(run_id));
        assert!(before.lifecycle_event.is_none());

        append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            cancellation_event_inputs("cancel-lifecycle-snapshot"),
        )
        .unwrap();
        let after = store.turn_lifecycle_snapshot(task_id).unwrap();
        assert_eq!(after.latest_sequence, 3);
        assert_eq!(after.bound_run_id.as_deref(), Some(run_id));
        let receipt = after.lifecycle_event.expect("latest lifecycle receipt");
        assert_eq!(receipt.sequence, 3);
        assert_eq!(receipt.event_type, "local_aborted");
        assert_eq!(receipt.run_id, run_id);
    }

    fn minimal_snapshot(task_id: &str, run_id: &str) -> MainChatAgentStateSnapshot {
        serde_json::from_value(json!({
            "task": {
                "taskId": task_id,
                "runId": run_id,
                "conversationId": "conversation-1",
                "userMessageId": "message-1",
                "title": "Minimal test task",
                "strategy": "direct_answer",
                "status": "answering",
                "createdAt": Utc::now(),
                "updatedAt": Utc::now(),
                "traceAvailable": true,
                "controls": [],
                "actionIds": [],
                "observationIds": [],
                "blockerIds": [],
                "proposalIds": []
            },
            "route": {
                "strategy": "direct_answer",
                "reason": "policy_selected_direct_answer"
            },
            "context": [],
            "actions": [],
            "observations": [],
            "blockers": [],
            "proposals": [],
            "diagnostics": [],
            "sequence": 0,
            "emittedAt": Utc::now(),
            "events": []
        }))
        .expect("minimal snapshot should match the durable contract")
    }

    #[test]
    fn provider_lifecycle_schema_rejects_payload_copies_instead_of_sanitizing_them() {
        const SECRET: &str = "EVENT_STORE_RAW_PRIVATE_CONTENT_SENTINEL";
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        store
            .append(provider_started_draft(
                "task-1",
                "run-1",
                "request-1",
                "provider-a",
            ))
            .unwrap();
        let error = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-1".into(),
                run_id: "run-1".into(),
                event_type: "provider.completed".into(),
                object_type: "provider_request".into(),
                object_id: "request-1".into(),
                created_at: Utc::now(),
                source: "provider_adapter".into(),
                payload: json!({
                    "status": "completed",
                    "requestId": "request-1",
                    "provider": "provider-a",
                    "model": "model-1",
                    "content": SECRET,
                    "note": SECRET,
                    "aliases": [SECRET, { "label": SECRET }],
                    "nested": { "arguments": { "private": SECRET } },
                }),
                backfilled: false,
            })
            .expect_err("provider lifecycle facts have an exact metadata-only schema");

        assert!(error
            .to_string()
            .contains("main_chat_agent_event_payload_schema_conflict:provider.completed"));
        assert_eq!(store.list("task-1", 0, 100).unwrap().len(), 1);
    }

    #[test]
    fn provider_selected_context_refs_accept_only_canonical_bounded_source_refs() {
        let mut valid = provider_started_draft(
            "task-web-context-ref",
            "run-web-context-ref",
            "request-web-context-ref",
            "provider-a",
        )
        .payload;
        valid["selectedContextRefs"] = json!([
            "websearch://550e8400-e29b-41d4-a716-446655440000/0?citation=webref_0123456789abcdef01234567",
            "resource://4b014569-cd91-4a9f-8bba-e4b605c9a412/chunk/0?citation=cite_c6857bb9f404f647ccae812c"
        ]);
        normalize_durable_event_payload(
            "provider.started",
            "provider_request",
            &valid,
            PayloadNormalizationOrigin::New,
        )
        .expect("a canonical current-run Web reference is bounded provider evidence");

        for invalid_ref in [
            "websearch://550e8400-e29b-41d4-a716-446655440000/0",
            "websearch://550e8400-e29b-41d4-a716-446655440000/00?citation=webref_0123456789abcdef01234567",
            "websearch://550e8400-e29b-41d4-a716-446655440000/0?citation=webref_NOT_A_DIGEST____________",
            "resource://4b014569-cd91-4a9f-8bba-e4b605c9a412/chunk/00?citation=cite_c6857bb9f404f647ccae812c",
            "resource://4b014569-cd91-4a9f-8bba-e4b605c9a412/chunk/0?citation=cite_c6857bb9f404f647ccae812c&filename=secret.md",
            "https://example.com/path?private=user-derived-content",
        ] {
            let mut invalid = valid.clone();
            invalid["selectedContextRefs"] = json!([invalid_ref]);
            let error = normalize_durable_event_payload(
                "provider.started",
                "provider_request",
                &invalid,
                PayloadNormalizationOrigin::New,
            )
            .expect_err("untyped, malformed, or raw Web references must fail closed");
            assert!(
                error
                    .to_string()
                    .contains("invalid_field_type_or_bound:selectedContextRefs"),
                "{invalid_ref}: {error}"
            );
        }
    }

    #[test]
    fn provider_web_context_reference_survives_restart_without_copying_web_content() {
        const WEB_CONTEXT_REF: &str =
            "websearch://550e8400-e29b-41d4-a716-446655440000/0?citation=webref_0123456789abcdef01234567";
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("web-provider-events.sqlite");
        let task_id = "task-web-provider-restart";
        let run_id = "run-web-provider-restart";
        let request_id = "request-web-provider-restart";
        let mut evidence =
            synthetic_provider_policy_evidence_for_test(request_id, "test-provider-generation");
        evidence.selected_context_refs = vec![WEB_CONTEXT_REF.into()];
        evidence.included_context_categories = vec!["web_search_untrusted".into()];
        let started_at = Utc::now();
        let receipt = ProviderInvocationReceipt {
            request_id: request_id.into(),
            provider: "provider-a".into(),
            model: "model-1".into(),
            status: ProviderInvocationStatus::Completed,
            started_at,
            finished_at: started_at + chrono::Duration::milliseconds(5),
            error_digest: None,
            simulated: false,
            policy_evidence: Some(evidence),
        };
        let proof = ProviderInvocationDurabilityProof::synthetic_for_test(receipt.clone()).unwrap();
        let drafts = provider_event_drafts(task_id, run_id, &[receipt]).unwrap();
        let scope = crate::main_chat_turn_runtime::MainChatProviderDurabilityScope::test_fixture(
            task_id, run_id,
        );

        {
            let store = MainChatAgentEventStore::new(&path).unwrap();
            store
                .append_provider_lifecycle_for_test(drafts, &scope, &[proof])
                .unwrap();
        }

        let reopened = MainChatAgentEventStore::new(&path).unwrap();
        let events = reopened.list(task_id, 0, 10).unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            events.iter().all(|event| {
                event
                    .payload
                    .get("selectedContextRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|refs| refs == &vec![json!(WEB_CONTEXT_REF)])
            }),
            "{events:#?}"
        );
        for event in &events {
            let payload = event.payload.as_object().unwrap();
            for forbidden_field in ["messages", "content", "body", "snippet", "query", "url"] {
                assert!(!payload.contains_key(forbidden_field), "{forbidden_field}");
            }
        }
    }

    #[test]
    fn durable_payload_does_not_retain_unrecognized_user_derived_object_keys() {
        const SENSITIVE_KEY: &str = "diagnosis_HIV_positive_for_Alice";
        const NESTED_SENSITIVE_KEY: &str = "private_case_number_74291";
        const NORMALIZATION_BYPASS_KEY: &str = "病例status";
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let mut payload = json!({
            "observationId": "observation-safe-metadata",
            "actionId": "action-safe-metadata",
            "sourceKind": "web",
            "sourceLabel": "https://example.com/source",
            "provider": "provider-must-not-fit-observation",
            "requestId": "request-must-not-fit-observation",
            "readExecution": {
                "kind": "web_search_network",
                "sourceKind": "web",
                "sourceLabel": "https://example.com/source",
                "target": "https://example.com/query",
                "realReadOnlyExecution": true,
                "fixtureBacked": false,
                "networkReadAttempted": true,
                "directWritesExecuted": false,
            },
        });
        payload
            .as_object_mut()
            .unwrap()
            .insert(SENSITIVE_KEY.into(), json!({ NESTED_SENSITIVE_KEY: true }));
        payload
            .as_object_mut()
            .unwrap()
            .insert(NORMALIZATION_BYPASS_KEY.into(), json!(true));
        let event = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-user-derived-key".into(),
                run_id: "run-user-derived-key".into(),
                event_type: "observation.created".into(),
                object_type: "observation".into(),
                object_id: "observation-safe-metadata".into(),
                created_at: Utc::now(),
                source: "action_executor".into(),
                payload,
                backfilled: false,
            })
            .unwrap();

        let serialized = serde_json::to_string(&event.payload).unwrap();
        assert!(!serialized.contains(SENSITIVE_KEY));
        assert!(!serialized.contains(NESTED_SENSITIVE_KEY));
        assert!(!serialized.contains(NORMALIZATION_BYPASS_KEY));
        assert!(!serialized.contains("provider-must-not-fit-observation"));
        assert!(!serialized.contains("request-must-not-fit-observation"));
        assert!(event.payload.get("provider").is_none());
        assert!(event.payload.get("requestId").is_none());
        assert_eq!(event.payload["observationId"], "observation-safe-metadata");
        assert_eq!(
            event.payload["readExecution"]["realReadOnlyExecution"],
            true
        );
        assert_eq!(event.payload["readExecution"]["fixtureBacked"], false);
        assert_eq!(event.payload["readExecution"]["networkReadAttempted"], true);
        assert_eq!(
            event.payload["readExecution"]["directWritesExecuted"],
            false
        );
        assert_eq!(event.payload["unrecognizedFieldsReceipt"]["fieldCount"], 4);
        assert_eq!(event.payload["unrecognizedFieldsReceipt"]["redacted"], true);
        assert_eq!(
            event.payload["unrecognizedFieldsReceipt"]["digestScope"],
            "keyed_unknown_fields_v1"
        );
        assert!(event.payload["unrecognizedFieldsReceipt"]["digest"]
            .as_str()
            .is_some_and(is_exact_hmac_digest));
        assert_eq!(
            event.payload["unrecognizedFieldsReceipt"]["typeCounts"]["string"],
            2
        );
        assert_eq!(
            event.payload["unrecognizedFieldsReceipt"]["typeCounts"]["object"],
            1
        );
        assert_eq!(
            event.payload["unrecognizedFieldsReceipt"]["typeCounts"]["bool"],
            1
        );
    }

    #[test]
    fn redacted_sensitive_values_use_keyed_receipts_and_do_not_collapse() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let completion = |content: &str| MainChatAgentEventDraft {
            task_session_id: "task-redacted-digest".into(),
            run_id: "run-redacted-digest".into(),
            event_type: "plan.created".into(),
            object_type: "plan".into(),
            object_id: "plan-redacted-digest".into(),
            created_at: Utc::now(),
            source: "plan_runtime".into(),
            payload: json!({
                "planId": "plan-redacted-digest",
                "status": "draft",
                "goal": content,
            }),
            backfilled: false,
        };

        let first = store
            .append(completion("short-enumerable-secret-a"))
            .unwrap();
        let conflict = store
            .append(completion("short-enumerable-secret-b"))
            .expect_err("different hidden facts must not collapse into one durable identity");
        let serialized = serde_json::to_string(&first.payload).unwrap();
        let keyed_digest = first.payload["goal"]["digest"].as_str().unwrap();
        let enumerable_digest = format!(
            "sha256:{:x}",
            Sha256::digest("short-enumerable-secret-a".as_bytes())
        );

        assert!(conflict
            .to_string()
            .contains("main_chat_agent_event_identity_conflict"));
        assert!(!serialized.contains("short-enumerable-secret-a"));
        assert!(!serialized.contains("short-enumerable-secret-b"));
        assert!(is_exact_hmac_digest(keyed_digest));
        assert_ne!(keyed_digest, enumerable_digest);
        assert_eq!(first.payload["goal"]["digestScope"], "keyed_input_value_v1");
    }

    #[test]
    fn keyed_redaction_is_stable_only_within_the_same_key_domain() {
        let first_key = MainChatEventDigestKey::from_key_material(&[0x11; 32]).unwrap();
        let second_key = MainChatEventDigestKey::from_key_material(&[0x22; 32]).unwrap();
        let value = json!("low-entropy-private-value");
        let first = redacted_event_value(&value, &first_key);
        let repeated = redacted_event_value(&value, &first_key);
        let second_domain = redacted_event_value(&value, &second_key);

        assert_eq!(first, repeated);
        assert_ne!(first["digest"], second_domain["digest"]);
        assert!(first["digest"].as_str().is_some_and(is_exact_hmac_digest));
        assert!(!serde_json::to_string(&first)
            .unwrap()
            .contains("low-entropy-private-value"));
    }

    #[test]
    fn persistent_event_store_binds_the_injected_digest_key() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("key-bound-events.db");
        {
            let store = MainChatAgentEventStore::new_with_digest_key(
                &path,
                MainChatEventDigestKey::from_key_material(&[0x31; 32]).unwrap(),
            )
            .unwrap();
            store
                .append(MainChatAgentEventDraft {
                    task_session_id: "task-key-bound".into(),
                    run_id: "run-key-bound".into(),
                    event_type: "plan.created".into(),
                    object_type: "plan".into(),
                    object_id: "plan-key-bound".into(),
                    created_at: Utc::now(),
                    source: "plan_runtime".into(),
                    payload: json!({
                        "planId": "plan-key-bound",
                        "status": "draft",
                        "goal": "private plan goal",
                    }),
                    backfilled: false,
                })
                .unwrap();
        }

        MainChatAgentEventStore::new_with_digest_key(
            &path,
            MainChatEventDigestKey::from_key_material(&[0x31; 32]).unwrap(),
        )
        .expect("the same durable key must reopen the store");
        let mismatch = match MainChatAgentEventStore::new_with_digest_key(
            &path,
            MainChatEventDigestKey::from_key_material(&[0x32; 32]).unwrap(),
        ) {
            Ok(_) => panic!("a different key would make receipt comparisons dishonest"),
            Err(error) => error,
        };
        assert!(mismatch
            .to_string()
            .contains("main_chat_event_digest_key_mismatch"));
    }

    #[test]
    fn typed_error_digest_is_keyed_and_counterfactual_conflicts_instead_of_collapsing() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let started = provider_started_draft(
            "task-low-entropy-error",
            "run-low-entropy-error",
            "request-low-entropy-error",
            "provider-a",
        );
        let terminal_at = started.created_at + chrono::Duration::milliseconds(1);
        store.append(started).unwrap();
        let candidate_digest = |candidate: &str| format!("sha256:{:x}", Sha256::digest(candidate));
        let terminal = |candidate: &str| MainChatAgentEventDraft {
            task_session_id: "task-low-entropy-error".into(),
            run_id: "run-low-entropy-error".into(),
            event_type: "provider.failed".into(),
            object_type: "provider_request".into(),
            object_id: "request-low-entropy-error".into(),
            created_at: terminal_at,
            source: "provider_adapter".into(),
            payload: json!({
                "status": "failed",
                "requestId": "request-low-entropy-error",
                "provider": "provider-a",
                "model": "model-1",
                "errorDigest": candidate_digest(candidate),
            }),
            backfilled: false,
        };

        let first = store.append(terminal("timeout")).unwrap();
        let conflict = store.append(terminal("unauthorized")).unwrap_err();
        let serialized = serde_json::to_string(&first.payload).unwrap();
        assert!(conflict
            .to_string()
            .contains("main_chat_agent_event_identity_conflict"));
        assert!(first.payload["errorDigest"]["digest"]
            .as_str()
            .is_some_and(is_exact_hmac_digest));
        assert_eq!(
            first.payload["errorDigest"]["digestScope"],
            "keyed_input_value_v1"
        );
        assert!(!serialized.contains(&candidate_digest("timeout")));
        assert!(!serialized.contains(&candidate_digest("unauthorized")));
        assert!(!serialized.contains("hash:sha256:"));
    }

    #[test]
    fn snapshot_event_materialization_rolls_back_the_whole_batch_on_later_failure() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute_batch(
                "CREATE TRIGGER reject_route_event
                 BEFORE INSERT ON main_chat_agent_events
                 WHEN NEW.event_type = 'route.selected'
                 BEGIN
                     SELECT RAISE(ABORT, 'injected route event failure');
                 END;",
            )
            .unwrap();
        }

        let result = materialize_main_chat_agent_events_for_snapshot_in_store(
            &store,
            &minimal_snapshot("task-atomic", "run-atomic"),
        );

        assert!(
            result.is_err(),
            "the injected second-event failure must surface"
        );
        assert!(
            store.list("task-atomic", 0, 100).unwrap().is_empty(),
            "a failed event batch must not leave the earlier task.created event committed"
        );
        assert_eq!(store.latest_sequence("task-atomic").unwrap(), 0);
    }

    #[test]
    fn cancellation_runtime_event_batch_rolls_back_every_event_when_second_draft_fails() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        store
            .install_local_aborted_insert_failure_for_test()
            .unwrap();

        let result = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-cancel-batch-failure",
            "run-cancel-batch-failure",
            cancellation_event_inputs("cancellation:batch-failure"),
        );

        assert!(
            result.is_err(),
            "the injected second-draft failure must surface"
        );
        assert!(
            store
                .list("task-cancel-batch-failure", 0, 100)
                .unwrap()
                .is_empty(),
            "one cancellation batch must leave zero rows when any draft fails"
        );
        assert_eq!(
            store.latest_sequence("task-cancel-batch-failure").unwrap(),
            0
        );
    }

    #[test]
    fn cancellation_runtime_event_batch_is_idempotent_for_one_cancellation_id() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let first = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-cancel-idempotent",
            "run-cancel-idempotent",
            cancellation_event_inputs("cancellation:idempotent"),
        )
        .expect("first cancellation batch commits");
        let repeated = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-cancel-idempotent",
            "run-cancel-idempotent",
            cancellation_event_inputs("cancellation:idempotent"),
        )
        .expect("repeated cancellation batch reuses the committed facts");

        assert_eq!(first.len(), 2);
        assert_eq!(repeated, first);
        assert!(first.iter().all(|event| {
            event.object_id == "cancellation:idempotent"
                && event.payload.get("cancellationId").and_then(Value::as_str)
                    == Some("cancellation:idempotent")
        }));
        assert_eq!(
            store.list("task-cancel-idempotent", 0, 100).unwrap(),
            first,
            "the repeated cancellation id must not append duplicate rows"
        );
        assert_eq!(store.latest_sequence("task-cancel-idempotent").unwrap(), 2);
    }

    #[test]
    fn runtime_cancel_requested_rejects_contradictory_transport_truth() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let error = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            "task-cancel-transport-truth",
            "run-cancel-transport-truth",
            vec![MainChatAgentRuntimeEventInput::new(
                "cancel_requested",
                "turn",
                "cancellation:transport-truth",
                "openlife_turn_runtime",
                json!({
                    "status": "cancel_requested",
                    "cancellationId": "cancellation:transport-truth",
                    "localWaitAborted": false,
                    "remoteCancellationConfirmed": true,
                    "durableCommitAllowedAfterCancel": false,
                }),
            )],
        )
        .expect_err("runtime cancellation cannot claim contradictory transport facts");
        assert!(error
            .to_string()
            .contains("main_chat_cancel_requested_transport_truth_invalid"));
        assert!(store
            .list("task-cancel-transport-truth", 0, 10)
            .unwrap()
            .is_empty());
    }

    fn provider_started_draft(
        task_session_id: &str,
        run_id: &str,
        request_id: &str,
        provider: &str,
    ) -> MainChatAgentEventDraft {
        let mut draft = MainChatAgentEventDraft {
            task_session_id: task_session_id.into(),
            run_id: run_id.into(),
            event_type: "provider.started".into(),
            object_type: "provider_request".into(),
            object_id: request_id.into(),
            created_at: Utc::now(),
            source: "provider_adapter".into(),
            payload: json!({
                "status": "started",
                "requestId": request_id,
                "provider": provider,
                "model": "model-1",
            }),
            backfilled: false,
        };
        append_provider_policy_evidence_payload(
            &mut draft.payload,
            &synthetic_provider_policy_evidence_for_test(request_id, "test-provider-generation"),
        )
        .unwrap();
        draft
    }

    fn provider_terminal_draft(
        task_session_id: &str,
        run_id: &str,
        request_id: &str,
        provider: &str,
        event_type: &str,
        created_at: DateTime<Utc>,
    ) -> MainChatAgentEventDraft {
        let mut draft = MainChatAgentEventDraft {
            task_session_id: task_session_id.into(),
            run_id: run_id.into(),
            event_type: event_type.into(),
            object_type: "provider_request".into(),
            object_id: request_id.into(),
            created_at,
            source: "provider_adapter".into(),
            payload: json!({
                "status": event_type.strip_prefix("provider.").unwrap_or("unknown"),
                "requestId": request_id,
                "provider": provider,
                "model": "model-1",
                "errorDigest": if event_type == "provider.failed" {
                    Value::String("sha256:test-failure".into())
                } else {
                    Value::Null
                },
            }),
            backfilled: false,
        };
        append_provider_policy_evidence_payload(
            &mut draft.payload,
            &synthetic_provider_policy_evidence_for_test(request_id, "test-provider-generation"),
        )
        .unwrap();
        draft
    }

    fn synthetic_provider_pair(
        task_session_id: &str,
        run_id: &str,
        request_id: &str,
        provider: &str,
        model: &str,
        status: ProviderInvocationStatus,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
    ) -> (
        Vec<MainChatAgentEventDraft>,
        crate::main_chat_turn_runtime::MainChatProviderDurabilityScope,
        ProviderInvocationDurabilityProof,
    ) {
        let evidence =
            synthetic_provider_policy_evidence_for_test(request_id, "test-provider-generation");
        let receipt = ProviderInvocationReceipt {
            request_id: request_id.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            status,
            started_at,
            finished_at,
            error_digest: (status != ProviderInvocationStatus::Completed)
                .then(|| format!("sha256:{}", "f".repeat(64))),
            simulated: false,
            policy_evidence: Some(evidence),
        };
        let proof = ProviderInvocationDurabilityProof::synthetic_for_test(receipt.clone()).unwrap();
        let drafts = provider_event_drafts(task_session_id, run_id, &[receipt]).unwrap();
        let scope = crate::main_chat_turn_runtime::MainChatProviderDurabilityScope::test_fixture(
            task_session_id,
            run_id,
        );
        (drafts, scope, proof)
    }

    fn synthetic_provider_start_only_proof(
        request_id: &str,
        provider: &str,
        model: &str,
        started_at: DateTime<Utc>,
    ) -> (
        openlife_core::llm::ProviderPolicyReceiptEvidence,
        ProviderInvocationDurabilityProof,
    ) {
        let evidence =
            synthetic_provider_policy_evidence_for_test(request_id, "test-provider-generation");
        let proof = ProviderInvocationDurabilityProof::synthetic_start_for_test(
            request_id.to_string(),
            provider.to_string(),
            model.to_string(),
            started_at,
            evidence.clone(),
        )
        .expect("synthetic start-only provider proof");
        (evidence, proof)
    }

    fn immutable_test_draft(
        task_session_id: &str,
        run_id: &str,
        object_id: &str,
        status: &str,
    ) -> MainChatAgentEventDraft {
        MainChatAgentEventDraft {
            task_session_id: task_session_id.into(),
            run_id: run_id.into(),
            event_type: "test.immutable_fact".into(),
            object_type: "test_fact".into(),
            object_id: object_id.into(),
            created_at: Utc::now(),
            source: "test".into(),
            payload: json!({"status": status}),
            backfilled: false,
        }
    }

    #[test]
    fn provider_lifecycle_rejects_orphan_identity_mismatch_and_conflicting_terminal() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let orphan = store
            .append(provider_terminal_draft(
                "task-provider-lifecycle",
                "run-provider-lifecycle",
                "request-orphan",
                "provider-a",
                "provider.completed",
                Utc::now(),
            ))
            .unwrap_err();
        assert!(orphan
            .to_string()
            .contains("main_chat_provider_lifecycle_conflict:terminal_without_start"));

        let start = provider_started_draft(
            "task-provider-lifecycle",
            "run-provider-lifecycle",
            "request-bound",
            "provider-a",
        );
        let started_at = start.created_at;
        store.append(start).unwrap();
        let mismatch = store
            .append(provider_terminal_draft(
                "task-provider-lifecycle",
                "run-provider-lifecycle",
                "request-bound",
                "provider-b",
                "provider.completed",
                started_at,
            ))
            .unwrap_err();
        assert!(mismatch
            .to_string()
            .contains("main_chat_provider_lifecycle_conflict:adapter_identity_mismatch"));

        store
            .append(provider_terminal_draft(
                "task-provider-lifecycle",
                "run-provider-lifecycle",
                "request-bound",
                "provider-a",
                "provider.completed",
                started_at,
            ))
            .unwrap();
        let conflict = store
            .append(provider_terminal_draft(
                "task-provider-lifecycle",
                "run-provider-lifecycle",
                "request-bound",
                "provider-a",
                "provider.failed",
                started_at,
            ))
            .unwrap_err();
        assert!(conflict
            .to_string()
            .contains("main_chat_provider_lifecycle_conflict:conflicting_terminal"));
    }

    #[test]
    fn provider_lifecycle_missing_policy_provenance_rolls_back_zero_rows() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let (mut drafts, scope, proof) = synthetic_provider_pair(
            "task-provider-policy-missing",
            "run-provider-policy-missing",
            "request-provider-policy-missing",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            Utc::now(),
            Utc::now(),
        );
        for draft in &mut drafts {
            for field in PROVIDER_POLICY_IDENTITY_FIELDS {
                draft.payload.as_object_mut().unwrap().remove(field);
            }
        }

        let error = store
            .append_provider_lifecycle_for_test(drafts, &scope, &[proof])
            .expect_err("missing policy provenance must fail before any row commits");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_policy_evidence_mismatch"));
        assert!(store
            .list("task-provider-policy-missing", 0, 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            store
                .latest_sequence("task-provider-policy-missing")
                .unwrap(),
            0
        );
    }

    #[test]
    fn provider_lifecycle_policy_a_to_b_drift_rolls_back_the_atomic_pair() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let now = Utc::now();
        let (mut drafts, scope, proof) = synthetic_provider_pair(
            "task-provider-policy-drift",
            "run-provider-policy-drift",
            "request-provider-policy-drift",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            now,
            now - chrono::Duration::seconds(1),
        );
        let terminal = drafts.last_mut().unwrap();
        terminal.payload["policyDecisionId"] = json!("policy-B");
        terminal.payload["effectiveDataRoute"] = json!("local_only");
        terminal.payload["policyEvidenceDigest"] = json!(format!("sha256:{}", "1".repeat(64)));

        let error = store
            .append_provider_lifecycle_for_test(drafts, &scope, &[proof])
            .expect_err("terminal policy drift must roll back the start and terminal");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_policy_evidence_mismatch"));
        assert!(store
            .list("task-provider-policy-drift", 0, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn provider_lifecycle_source_string_without_runtime_proof_is_not_authority() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let now = Utc::now();
        let (drafts, _, _) = synthetic_provider_pair(
            "task-provider-forged-source",
            "run-provider-forged-source",
            "request-provider-forged-source",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            now,
            now,
        );

        let error = store
            .append_batch(drafts)
            .expect_err("provider_adapter source text must not mint durability authority");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_runtime_admission_missing"));
        assert!(store
            .list("task-provider-forged-source", 0, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn provider_durability_proof_cannot_be_transplanted_to_another_task_or_run() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let now = Utc::now();
        let (drafts, _, proof) = synthetic_provider_pair(
            "task-provider-target",
            "run-provider-target",
            "request-provider-transplant",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            now,
            now,
        );
        let wrong_scope =
            crate::main_chat_turn_runtime::MainChatProviderDurabilityScope::test_fixture(
                "task-provider-source",
                "run-provider-source",
            );

        let error = store
            .append_provider_lifecycle_for_test(drafts, &wrong_scope, &[proof])
            .expect_err("a real proof must remain bound to its canonical task and run");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_task_run_scope_mismatch"));
        assert!(store
            .list("task-provider-target", 0, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn terminal_proof_cannot_join_a_reused_request_id_to_another_start_observation() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let first_started_at = Utc::now();
        let (mut first, first_scope, first_proof) = synthetic_provider_pair(
            "task-provider-attempt-transplant",
            "run-provider-attempt-transplant",
            "request-provider-attempt-transplant",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            first_started_at,
            first_started_at,
        );
        first.truncate(1);
        store
            .append_provider_lifecycle_for_test(first, &first_scope, &[first_proof])
            .expect("persist the first proved start only");

        let second_started_at = first_started_at + chrono::Duration::seconds(1);
        let (second, second_scope, second_proof) = synthetic_provider_pair(
            "task-provider-attempt-transplant",
            "run-provider-attempt-transplant",
            "request-provider-attempt-transplant",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            second_started_at,
            second_started_at,
        );
        let second_terminal = second.into_iter().skip(1).collect::<Vec<_>>();
        let error = store
            .append_provider_lifecycle_for_test(second_terminal, &second_scope, &[second_proof])
            .expect_err("terminal B cannot join the already durable start A");
        assert!(error
            .to_string()
            .contains("terminal_start_observation_mismatch"));
        let durable = store
            .list("task-provider-attempt-transplant", 0, 10)
            .unwrap();
        assert_eq!(durable.len(), 1);
        assert_eq!(durable[0].event_type, "provider.started");
    }

    #[test]
    fn runtime_unknown_cannot_replace_an_observed_adapter_terminal() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let now = Utc::now();
        let (mut drafts, scope, proof) = synthetic_provider_pair(
            "task-provider-observed-terminal",
            "run-provider-observed-terminal",
            "request-provider-observed-terminal",
            "openai",
            "gpt-test",
            ProviderInvocationStatus::Completed,
            now,
            now,
        );
        let terminal = drafts.last_mut().unwrap();
        terminal.event_type = "provider.remote_unknown".into();
        terminal.source = "openlife_turn_runtime".into();
        terminal.payload["status"] = json!("remote_unknown");

        let error = store
            .append_provider_lifecycle_for_test(drafts, &scope, &[proof])
            .expect_err("runtime unknown cannot erase a proved adapter terminal");
        assert!(error
            .to_string()
            .contains("runtime_unknown_conflicts_with_adapter_terminal"));
        assert!(store
            .list("task-provider-observed-terminal", 0, 10)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn latest_provider_fact_uses_durable_sequence_across_wall_clock_rollback() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let base = DateTime::parse_from_rfc3339("2026-07-12T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let (first, first_scope, first_proof) = synthetic_provider_pair(
            "task-provider-sequence",
            "run-provider-sequence",
            "request-provider-a",
            "openai",
            "model-a",
            ProviderInvocationStatus::Completed,
            base + chrono::Duration::seconds(100),
            base + chrono::Duration::seconds(101),
        );
        store
            .append_provider_lifecycle_for_test(first, &first_scope, &[first_proof])
            .unwrap();
        let (second, second_scope, second_proof) = synthetic_provider_pair(
            "task-provider-sequence",
            "run-provider-sequence",
            "request-provider-b",
            "openai",
            "model-b",
            ProviderInvocationStatus::RemoteUnknown,
            base + chrono::Duration::seconds(50),
            base + chrono::Duration::seconds(49),
        );
        store
            .append_provider_lifecycle_for_test(second, &second_scope, &[second_proof])
            .unwrap();

        let latest_for_run = store
            .latest_provider_event_for_run("run-provider-sequence")
            .unwrap()
            .unwrap();
        assert_eq!(latest_for_run.object_id, "request-provider-b");
        assert_eq!(latest_for_run.event_type, "provider.remote_unknown");
        assert_eq!(latest_for_run.sequence, 4);
        let latest_global = store.latest_provider_event().unwrap().unwrap();
        assert_eq!(latest_global.object_id, "request-provider-b");
    }

    #[test]
    fn provider_lifecycle_uses_sequence_when_wall_clock_rolls_back_within_attempt() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let start = provider_started_draft(
            "task-provider-time",
            "run-provider-time",
            "request-time",
            "provider-a",
        );
        let started_at = start.created_at;
        store.append(start).unwrap();

        let terminal = store
            .append(provider_terminal_draft(
                "task-provider-time",
                "run-provider-time",
                "request-time",
                "provider-a",
                "provider.completed",
                started_at - chrono::Duration::milliseconds(1),
            ))
            .expect("later durable sequence remains authoritative across clock rollback");

        assert_eq!(terminal.sequence, 2);
        assert!(terminal.created_at < started_at);
        assert_eq!(
            store
                .latest_provider_event_for_run("run-provider-time")
                .unwrap()
                .unwrap()
                .event_type,
            "provider.completed"
        );
    }

    #[test]
    fn immutable_identity_rejects_a_different_payload_without_advancing_sequence() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let first = store
            .append(immutable_test_draft(
                "task-conflict",
                "run-conflict",
                "request-conflict",
                "first",
            ))
            .unwrap();

        let error = store
            .append(immutable_test_draft(
                "task-conflict",
                "run-conflict",
                "request-conflict",
                "second",
            ))
            .expect_err("one immutable event identity cannot claim two payloads");

        assert!(error
            .to_string()
            .contains("main_chat_agent_event_identity_conflict"));
        assert_eq!(store.list("task-conflict", 0, 100).unwrap(), vec![first]);
        assert_eq!(store.latest_sequence("task-conflict").unwrap(), 1);
    }

    #[test]
    fn database_constraint_rejects_external_immutable_identity_conflict_after_open() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("event-external-writer-conflict.db");
        let store = MainChatAgentEventStore::new(&path).unwrap();
        let original = store
            .append(provider_started_draft(
                "task-external-writer",
                "run-external-writer",
                "request-external-writer",
                "provider-a",
            ))
            .unwrap();

        let external = Connection::open(&path).unwrap();
        let mut conflicting_fact = original.payload.clone();
        conflicting_fact["provider"] = json!("provider-b");
        let conflicting_payload = serde_json::to_string(&conflicting_fact).unwrap();
        let error = external
            .execute(
                "INSERT INTO main_chat_agent_events (
                    event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json,
                    payload_minimized_version, backfilled
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)",
                params![
                    "external-conflicting-event",
                    "task-external-writer",
                    "run-external-writer",
                    2_i64,
                    "provider.started",
                    "provider_request",
                    "request-external-writer",
                    Utc::now().to_rfc3339(),
                    "legacy_external_writer",
                    metadata_safe_digest(&conflicting_payload),
                    conflicting_payload,
                    DURABLE_EVENT_PAYLOAD_VERSION,
                ],
            )
            .expect_err("the database must reject a second immutable logical identity");

        assert!(error
            .to_string()
            .contains("main_chat_agent_event_immutable_identity_exists"));

        let replace_error = external
            .execute(
                "INSERT OR REPLACE INTO main_chat_agent_events (
                    event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json,
                    payload_minimized_version, backfilled
                 ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0)",
                params![
                    "external-replacement-event",
                    "task-external-writer",
                    "run-external-writer",
                    "provider.started",
                    "provider_request",
                    "request-external-writer",
                    Utc::now().to_rfc3339(),
                    "legacy_external_writer",
                    metadata_safe_digest(&conflicting_payload),
                    conflicting_payload,
                    DURABLE_EVENT_PAYLOAD_VERSION,
                ],
            )
            .expect_err("INSERT OR REPLACE must not rewrite an immutable fact");
        assert!(replace_error
            .to_string()
            .contains("main_chat_agent_event_immutable_identity_exists"));

        let update_error = external
            .execute(
                "UPDATE main_chat_agent_events
                 SET payload_json = ?1, payload_digest = ?2
                 WHERE event_id = ?3",
                params![
                    conflicting_payload,
                    metadata_safe_digest(&conflicting_payload),
                    original.event_id,
                ],
            )
            .expect_err("coherent payload and digest updates must not rewrite durable facts");
        assert!(update_error
            .to_string()
            .contains("main_chat_agent_events_append_only:update"));

        let delete_error = external
            .execute(
                "DELETE FROM main_chat_agent_events WHERE event_id = ?1",
                [&original.event_id],
            )
            .expect_err("external writers must not delete durable facts");
        assert!(delete_error
            .to_string()
            .contains("main_chat_agent_events_append_only:delete"));
        assert_eq!(
            store.list("task-external-writer", 0, 100).unwrap(),
            vec![original],
            "an external writer conflict must not pollute replay truth"
        );
        assert_eq!(store.latest_sequence("task-external-writer").unwrap(), 1);
    }

    #[test]
    fn immutable_identity_conflict_rolls_back_the_entire_later_batch() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let original = store
            .append(immutable_test_draft(
                "task-conflict-batch",
                "run-conflict-batch",
                "request-original",
                "first",
            ))
            .unwrap();

        let error = store
            .append_batch(vec![
                immutable_test_draft(
                    "task-conflict-batch",
                    "run-conflict-batch",
                    "request-new",
                    "first",
                ),
                immutable_test_draft(
                    "task-conflict-batch",
                    "run-conflict-batch",
                    "request-original",
                    "second",
                ),
            ])
            .expect_err("the conflicting second draft must roll back the first draft");

        assert!(error
            .to_string()
            .contains("main_chat_agent_event_identity_conflict"));
        assert_eq!(
            store.list("task-conflict-batch", 0, 100).unwrap(),
            vec![original]
        );
        assert_eq!(store.latest_sequence("task-conflict-batch").unwrap(), 1);
    }

    #[test]
    fn concurrent_connections_cannot_commit_two_versions_of_one_immutable_fact() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("event-identity-race.db");
        let first_store = MainChatAgentEventStore::new(&path).unwrap();
        let second_store = MainChatAgentEventStore::new(&path).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(2));

        let first_barrier = Arc::clone(&barrier);
        let first = std::thread::spawn(move || {
            first_barrier.wait();
            first_store.append(immutable_test_draft(
                "task-race",
                "run-race",
                "request-race",
                "first",
            ))
        });
        let second_barrier = Arc::clone(&barrier);
        let second = std::thread::spawn(move || {
            second_barrier.wait();
            second_store.append(immutable_test_draft(
                "task-race",
                "run-race",
                "request-race",
                "second",
            ))
        });

        let outcomes = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        let error = outcomes
            .iter()
            .find_map(|outcome| outcome.as_ref().err())
            .expect("one conflicting claimant must fail");
        assert!(error
            .to_string()
            .contains("main_chat_agent_event_identity_conflict"));

        let verifier = MainChatAgentEventStore::new(&path).unwrap();
        let events = verifier.list("task-race", 0, 100).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(verifier.latest_sequence("task-race").unwrap(), 1);
    }

    #[test]
    fn same_fact_cannot_silently_switch_to_a_different_run() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let draft = provider_started_draft(
            "task-run-binding",
            "run-binding-a",
            "request-run-binding",
            "provider-a",
        );
        let first = store.append(draft.clone()).unwrap();
        let mut wrong_run = draft;
        wrong_run.run_id = "run-binding-b".into();

        let error = store
            .append(wrong_run)
            .expect_err("same-digest idempotency must not hide a different run owner");

        assert!(error
            .to_string()
            .contains("main_chat_agent_event_task_run_identity_conflict"));
        assert_eq!(store.list("task-run-binding", 0, 100).unwrap(), vec![first]);
    }

    #[test]
    fn positive_sequence_gap_blocks_reads_and_later_appends() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let first = store
            .append(provider_started_draft(
                "task-sequence-gap",
                "run-sequence-gap",
                "request-sequence-gap-1",
                "provider-a",
            ))
            .unwrap();
        store
            .append(provider_started_draft(
                "task-sequence-gap",
                "run-sequence-gap",
                "request-sequence-gap-2",
                "provider-a",
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "DELETE FROM main_chat_agent_events WHERE event_id = ?1",
                [&first.event_id],
            )
            .unwrap();
        }

        let read_error = store.list("task-sequence-gap", 0, 100).unwrap_err();
        assert!(read_error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:sequence"));
        let append_error = store
            .append(provider_started_draft(
                "task-sequence-gap",
                "run-sequence-gap",
                "request-sequence-gap-3",
                "provider-a",
            ))
            .unwrap_err();
        assert!(append_error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:sequence"));
        let conn = store.lock_conn().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM main_chat_agent_events WHERE task_session_id = 'task-sequence-gap'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "corruption must not be extended by a later append"
        );
    }

    #[test]
    fn sequence_ledger_ahead_of_facts_blocks_append() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        store
            .append(provider_started_draft(
                "task-ledger-ahead",
                "run-ledger-ahead",
                "request-ledger-ahead-1",
                "provider-a",
            ))
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "UPDATE main_chat_agent_event_sequences SET last_sequence = 9
                 WHERE task_session_id = 'task-ledger-ahead'",
                [],
            )
            .unwrap();
        }

        let error = store
            .append(provider_started_draft(
                "task-ledger-ahead",
                "run-ledger-ahead",
                "request-ledger-ahead-2",
                "provider-a",
            ))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:sequence"));
        let conn = store.lock_conn().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM main_chat_agent_events WHERE task_session_id = 'task-ledger-ahead'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn reopening_quarantines_preexisting_immutable_conflicts_without_blocking_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy-event-identity-conflict.db");
        let store = MainChatAgentEventStore::new(&path).unwrap();
        let original = store
            .append(provider_started_draft(
                "task-legacy-conflict",
                "run-legacy-conflict",
                "request-legacy-conflict",
                "provider-a",
            ))
            .unwrap();
        drop(store);

        let mut conflicting_fact = original.payload;
        conflicting_fact["provider"] = json!("provider-b");
        let conflicting_payload = serde_json::to_string(&conflicting_fact).unwrap();
        let conflicting_digest = metadata_safe_digest(&conflicting_payload);
        let connection = Connection::open(&path).unwrap();
        drop_event_integrity_triggers(&connection).unwrap();
        connection
            .execute(
                "DELETE FROM main_chat_agent_event_store_metadata
                 WHERE key = 'event_identity_version'",
                [],
            )
            .unwrap();
        connection
            .execute("DELETE FROM main_chat_agent_event_immutable_identities", [])
            .unwrap();
        connection
            .execute(&format!("DROP INDEX {IMMUTABLE_IDENTITY_INDEX}"), [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO main_chat_agent_events (
                    event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json,
                    payload_minimized_version, backfilled
                 ) VALUES (?1, ?2, ?3, 2, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 0)",
                params![
                    "legacy-conflicting-event-2",
                    "task-legacy-conflict",
                    "run-legacy-conflict",
                    "provider.started",
                    "provider_request",
                    "request-legacy-conflict",
                    Utc::now().to_rfc3339(),
                    "legacy_provider_adapter",
                    conflicting_digest,
                    conflicting_payload,
                ],
            )
            .unwrap();
        drop(connection);

        let reopened = MainChatAgentEventStore::new(&path).unwrap();
        assert!(reopened
            .list("task-legacy-conflict", 0, 10)
            .unwrap()
            .is_empty());
        let receipts = reopened.list_identity_quarantine_receipts(10).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].reason_code, "immutable_identity_conflict");
    }

    #[test]
    fn reopening_quarantines_mixed_run_ownership_without_blocking_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy-event-run-conflict.db");
        let store = MainChatAgentEventStore::new(&path).unwrap();
        store
            .append(provider_started_draft(
                "task-legacy-run-conflict",
                "run-legacy-a",
                "request-legacy-run-conflict",
                "provider-a",
            ))
            .unwrap();
        drop(store);

        let payload_json = serde_json::to_string(&json!({
            "taskSessionId": "task-legacy-run-conflict",
            "status": "answering"
        }))
        .unwrap();
        let payload_digest = stored_event_payload_digest("task.updated", 2, &payload_json);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "DELETE FROM main_chat_agent_event_store_metadata
                 WHERE key = 'event_identity_version'",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO main_chat_agent_events (
                    event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json,
                    payload_minimized_version, backfilled
                 ) VALUES (?1, ?2, ?3, 2, 'task.updated', 'task', ?2, ?4, 'legacy', ?5, ?6, 2, 0)",
                params![
                    "legacy-mixed-run-event-2",
                    "task-legacy-run-conflict",
                    "run-legacy-b",
                    Utc::now().to_rfc3339(),
                    payload_digest,
                    payload_json,
                ],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE main_chat_agent_event_sequences SET last_sequence = 2
                 WHERE task_session_id = 'task-legacy-run-conflict'",
                [],
            )
            .unwrap();
        drop(connection);

        let reopened = MainChatAgentEventStore::new(&path).unwrap();
        assert!(reopened
            .list("task-legacy-run-conflict", 0, 10)
            .unwrap()
            .is_empty());
        let receipts = reopened.list_identity_quarantine_receipts(10).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].reason_code, "mixed_run_ownership");
    }

    #[test]
    fn explicit_versioned_event_type_can_append_distinct_state_facts() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let mut first = provider_started_draft(
            "task-versioned",
            "run-versioned",
            "task-versioned",
            "provider-a",
        );
        first.event_type = "task.updated".into();
        first.object_type = "task".into();
        first.payload = json!({
            "taskSessionId": "task-versioned",
            "status": "answering"
        });
        let mut second = first.clone();
        second.payload = json!({
            "taskSessionId": "task-versioned",
            "status": "completed"
        });
        let third = first.clone();

        store
            .append_batch(vec![first, second, third.clone()])
            .unwrap();
        store
            .append(third)
            .expect("repeating the latest state is idempotent");

        let events = store.list("task-versioned", 0, 100).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[1].sequence, 2);
        assert_eq!(events[2].sequence, 3);
        assert_eq!(events[0].payload, events[2].payload);
        assert_ne!(events[0].payload_digest, events[1].payload_digest);
        assert_ne!(events[0].payload_digest, events[2].payload_digest);
    }

    #[test]
    fn snapshot_action_status_changes_do_not_rewrite_queued_or_started_facts() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let mut queued = minimal_snapshot("task-action-snapshot", "run-action-snapshot");
        queued.actions = vec![serde_json::from_value(json!({
            "actionId": "action-snapshot-1",
            "actionType": "file.read",
            "target": "workspace:file",
            "label": "Read one file",
            "status": "queued",
            "riskLevel": "safe_read",
            "policyDecisionId": "policy-action-1",
            "startedAt": null,
            "finishedAt": null,
            "observationIds": [],
            "retryable": false
        }))
        .unwrap()];
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &queued).unwrap();

        let mut pending = queued.clone();
        pending.actions[0].status = "blocked".into();
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &pending).unwrap();
        assert!(store
            .list("task-action-snapshot", 0, 100)
            .unwrap()
            .iter()
            .all(|event| event.event_type != "action.started"));

        let mut executing = pending.clone();
        executing.actions[0].status = "running".into();
        executing.actions[0].started_at = Some(Utc::now());
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &executing).unwrap();

        let mut retrying = executing.clone();
        retrying.actions[0].started_at = Some(Utc::now() + chrono::Duration::seconds(1));
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &retrying)
            .expect("a later execution attempt needs a distinct versioned start fact");

        let mut completed = retrying;
        completed.actions[0].status = "succeeded".into();
        completed.actions[0].finished_at = Some(Utc::now());
        completed.actions[0].observation_ids = vec!["observation-action-1".into()];
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &completed)
            .expect("later action state must append transitions without rewriting creation facts");

        let events = store.list("task-action-snapshot", 0, 100).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "action.queued")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "action.started")
                .count(),
            2
        );
        assert!(events
            .iter()
            .any(|event| event.event_type == "action.completed"));
    }

    #[test]
    fn legacy_run_derived_backfill_facts_are_quarantined_without_hiding_canonical_context() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-legacy-backfill-quarantine";
        let run_id = "run-legacy-backfill-quarantine";
        let mut snapshot = minimal_snapshot(task_id, run_id);
        snapshot.context = vec![
            serde_json::from_value(json!({
                "contextId": "canonical-session-context",
                "sourceKind": "context_snapshot",
                "sourceLabel": "canonical-session-context",
                "evidenceId": "canonical-session-context"
            }))
            .unwrap(),
            serde_json::from_value(json!({
                "contextId": "legacy-run-context",
                "sourceKind": "run_context",
                "sourceLabel": "legacy run payload",
                "evidenceId": "legacy-run-context"
            }))
            .unwrap(),
        ];
        snapshot.provider = Some(
            serde_json::from_value(json!({
                "provider": "legacy-provider",
                "model": "legacy-model",
                "routeType": "cloud",
                "reason": "legacy AgentRun model_route",
                "evidenceId": run_id
            }))
            .unwrap(),
        );
        let materialized =
            materialize_main_chat_agent_backfill_events_for_snapshot_in_store(&store, &snapshot)
                .unwrap();
        let legacy_event_ids = materialized
            .iter()
            .filter(|event| {
                event.event_type == "provider.selected"
                    || (event.event_type == "context.selected"
                        && event.payload.get("sourceKind").and_then(Value::as_str)
                            == Some("run_context"))
            })
            .map(|event| event.event_id.clone())
            .collect::<Vec<_>>();
        assert_eq!(legacy_event_ids.len(), 2);

        let (visible, point_lookup) = std::thread::scope(|scope| {
            let list = scope.spawn(|| store.list(task_id, 0, 100).unwrap());
            let point = scope.spawn(|| store.event_by_id(&legacy_event_ids[0]).unwrap());
            (list.join().unwrap(), point.join().unwrap())
        });
        assert!(point_lookup.is_none());
        assert_eq!(
            store
                .quarantine_run_derived_backfill_facts(task_id, Some(run_id))
                .unwrap(),
            0,
            "concurrent first-read quarantine must be idempotent"
        );
        assert!(visible.iter().any(|event| {
            event.event_type == "context.selected"
                && event.payload.get("sourceKind").and_then(Value::as_str)
                    == Some("context_snapshot")
        }));
        assert!(!visible.iter().any(|event| {
            event.event_type == "provider.selected"
                || event.payload.get("sourceKind").and_then(Value::as_str) == Some("run_context")
        }));
        for event_id in legacy_event_ids {
            assert!(store.event_by_id(&event_id).unwrap().is_none());
        }
    }

    #[test]
    fn run_derived_backfill_quarantine_is_first_read_safe_and_restart_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("run-derived-backfill.db");
        let task_id = "task-run-derived-reopen";
        let run_id = "run-run-derived-reopen";
        let legacy_event_ids = {
            let store = MainChatAgentEventStore::new(&path).unwrap();
            let mut snapshot = minimal_snapshot(task_id, run_id);
            snapshot.context = vec![serde_json::from_value(json!({
                "contextId": "legacy-run-context-reopen",
                "sourceKind": "run_context",
                "sourceLabel": "legacy run context",
                "evidenceId": "legacy-run-context-reopen"
            }))
            .unwrap()];
            snapshot.provider = Some(
                serde_json::from_value(json!({
                    "provider": "legacy-provider",
                    "model": "legacy-model",
                    "routeType": "cloud",
                    "reason": "legacy AgentRun model_route",
                    "evidenceId": run_id
                }))
                .unwrap(),
            );
            materialize_main_chat_agent_backfill_events_for_snapshot_in_store(&store, &snapshot)
                .unwrap()
                .into_iter()
                .filter(|event| {
                    event.event_type == "provider.selected"
                        || event.payload.get("sourceKind").and_then(Value::as_str)
                            == Some("run_context")
                })
                .map(|event| event.event_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(legacy_event_ids.len(), 2);

        let reopened = MainChatAgentEventStore::new(&path).unwrap();
        assert!(reopened
            .event_by_id(&legacy_event_ids[0])
            .unwrap()
            .is_none());
        assert!(reopened.list(task_id, 0, 100).unwrap().iter().all(|event| {
            event.event_type != "provider.selected"
                && event.payload.get("sourceKind").and_then(Value::as_str) != Some("run_context")
        }));
        drop(reopened);

        let reopened_again = MainChatAgentEventStore::new(&path).unwrap();
        for event_id in legacy_event_ids {
            assert!(reopened_again.event_by_id(&event_id).unwrap().is_none());
        }
    }

    #[test]
    fn snapshot_proposal_decision_does_not_rewrite_proposal_created_fact() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let mut pending = minimal_snapshot("task-proposal-snapshot", "run-proposal-snapshot");
        pending.proposals = vec![serde_json::from_value(json!({
            "proposalId": "proposal-snapshot-1",
            "proposalType": "memory_write",
            "status": "pending_review",
            "title": "Remember one preference",
            "summary": "A bounded proposal summary",
            "evidenceIds": ["evidence-proposal-1"],
            "actionIds": [],
            "controls": ["accept_proposal", "reject_proposal"],
            "memoryLifecycle": null
        }))
        .unwrap()];
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &pending).unwrap();

        let mut accepted = pending.clone();
        accepted.proposals[0].status = MainChatAgentProductProposalStatus::Accepted;
        materialize_main_chat_agent_events_for_snapshot_in_store(&store, &accepted)
            .expect("proposal decision must append a transition without rewriting creation");

        let events = store.list("task-proposal-snapshot", 0, 100).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "proposal.created")
                .count(),
            1
        );
        assert!(events
            .iter()
            .any(|event| event.event_type == "proposal.accepted"));
    }

    #[test]
    fn corrupted_sequence_is_an_error_instead_of_a_synthetic_zero() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        store
            .append(provider_started_draft(
                "deleted-task",
                "deleted-run",
                "request-delete-test",
                "provider-a",
            ))
            .unwrap();
        let event = store
            .append(provider_started_draft(
                "task-corrupt-sequence",
                "run-corrupt-sequence",
                "request-corrupt-sequence",
                "provider-a",
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "UPDATE main_chat_agent_events SET sequence = -1 WHERE event_id = ?1",
                [&event.event_id],
            )
            .unwrap();
        }

        let error = select_event_by_id(&store.lock_conn().unwrap(), &event.event_id)
            .expect_err("negative durable sequence must be explicit corruption");
        assert!(error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:sequence"));
        assert!(store.latest_sequence("task-corrupt-sequence").is_err());
    }

    #[test]
    fn corrupted_timestamp_is_an_error_instead_of_the_current_time() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event = store
            .append(provider_started_draft(
                "task-corrupt-time",
                "run-corrupt-time",
                "request-corrupt-time",
                "provider-a",
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "UPDATE main_chat_agent_events SET created_at = 'not-a-timestamp' WHERE event_id = ?1",
                [&event.event_id],
            )
            .unwrap();
        }

        let error = select_event_by_id(&store.lock_conn().unwrap(), &event.event_id)
            .expect_err("malformed durable timestamp must be explicit corruption");
        assert!(error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:created_at"));
    }

    #[test]
    fn corrupted_payload_is_an_error_instead_of_synthetic_null() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event = store
            .append(provider_started_draft(
                "task-corrupt-payload",
                "run-corrupt-payload",
                "request-corrupt-payload",
                "provider-a",
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "UPDATE main_chat_agent_events SET payload_json = '{broken-json' WHERE event_id = ?1",
                [&event.event_id],
            )
            .unwrap();
        }

        let error = select_event_by_id(&store.lock_conn().unwrap(), &event.event_id)
            .expect_err("malformed durable payload must be explicit corruption");
        assert!(error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:payload_json"));
    }

    #[test]
    fn current_version_row_with_a_cross_event_field_fails_closed_on_decode() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event = store
            .append(MainChatAgentEventDraft {
                task_session_id: "task-corrupt-schema".into(),
                run_id: "run-corrupt-schema".into(),
                event_type: "diagnostic.created".into(),
                object_type: "diagnostic".into(),
                object_id: "gap-corrupt-schema".into(),
                created_at: Utc::now(),
                source: "diagnostic".into(),
                payload: json!({
                    "gapId": "gap-corrupt-schema",
                    "gapCode": "schema_corruption",
                }),
                backfilled: false,
            })
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        let corrupt_payload = json!({
            "gapId": "gap-corrupt-schema",
            "gapCode": "schema_corruption",
            "provider": "provider-field-cannot-live-under-diagnostic",
        })
        .to_string();
        let corrupt_digest =
            stored_event_payload_digest("diagnostic.created", event.sequence, &corrupt_payload);
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "UPDATE main_chat_agent_events
                 SET payload_json = ?1, payload_digest = ?2 WHERE event_id = ?3",
                params![corrupt_payload, corrupt_digest, event.event_id],
            )
            .unwrap();
        }

        let error = select_event_by_id(&store.lock_conn().unwrap(), &event.event_id)
            .expect_err("current-version rows must already match their event schema");
        assert!(error
            .to_string()
            .contains("payload_json:schema_invalid_or_noncanonical"));
    }

    #[test]
    fn unsupported_future_payload_version_is_not_decoded_as_current_truth() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let event = store
            .append(provider_started_draft(
                "task-future-payload-version",
                "run-future-payload-version",
                "request-future-payload-version",
                "provider-a",
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "UPDATE main_chat_agent_events SET payload_minimized_version = ?1
                 WHERE event_id = ?2",
                params![DURABLE_EVENT_PAYLOAD_VERSION + 1, event.event_id],
            )
            .unwrap();
        }

        let error = select_event_by_id(&store.lock_conn().unwrap(), &event.event_id)
            .expect_err("newer durable payload semantics require an explicit migration");
        assert!(error
            .to_string()
            .contains("main_chat_agent_event_corrupt_row:payload_minimized_version"));
    }

    #[test]
    fn preexisting_matching_provider_pair_without_policy_provenance_is_quarantined() {
        // This is a row-level migration counterexample using the durable
        // `payload_minimized_version` contract. It deliberately makes no
        // claim about historical table DDL.
        const LEGACY_BODY: &str = "THERAPY-CASE-74291-ORCHID";
        const LEGACY_SENSITIVE_KEY: &str = "legacy_private_case_74291";
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let conn = store.lock_conn().unwrap();
        let started_at = Utc::now();
        let start_payload = json!({
            "status": "started",
            "requestId": "request-legacy",
            "provider": "provider-legacy",
            "model": "model-legacy",
        });
        let start_event_id =
            stable_event_id("task-legacy", 1, "provider.started", "request-legacy");
        conn.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled,
                payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, 'provider.started', 'provider_request', ?4, ?5,
                       'provider_adapter', 'legacy-start-payload-digest', ?6, 0, 2)",
            params![
                start_event_id,
                "task-legacy",
                "run-legacy",
                "request-legacy",
                started_at.to_rfc3339(),
                start_payload.to_string(),
            ],
        )
        .unwrap();
        let mut legacy_payload = json!({
            "status": "completed",
            "requestId": "request-legacy",
            "provider": "provider-legacy",
            "model": "model-legacy",
            "errorDigest": format!("sha256:{}", "a".repeat(64)),
            "note": LEGACY_BODY
        });
        legacy_payload
            .as_object_mut()
            .unwrap()
            .insert(LEGACY_SENSITIVE_KEY.into(), json!(true));
        let event_id = stable_event_id("task-legacy", 2, "provider.completed", "request-legacy");
        conn.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled, payload_minimized_version
             ) VALUES (?1, ?2, ?3, 2, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 2)",
            params![
                event_id,
                "task-legacy",
                "run-legacy",
                "provider.completed",
                "provider_request",
                "request-legacy",
                (started_at + chrono::Duration::milliseconds(1)).to_rfc3339(),
                "provider_adapter",
                "legacy-payload-digest",
                legacy_payload.to_string(),
            ],
        )
        .unwrap();

        migrate_legacy_event_payloads(&conn, &store.digest_key).unwrap();
        migrate_legacy_event_payloads(&conn, &store.digest_key).unwrap();

        assert!(select_event_by_id(&conn, &start_event_id)
            .unwrap()
            .is_none());
        assert!(select_event_by_id(&conn, &event_id).unwrap().is_none());
        drop(conn);
        let receipts = store.list_identity_quarantine_receipts(10).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].event_count, 2);
        assert_eq!(
            receipts[0].reason_code,
            "legacy_provider_lifecycle_unverified"
        );
    }

    #[test]
    fn matching_legacy_provider_pair_without_provenance_stays_hidden_after_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let db_path = directory
            .path()
            .join("provider-policy-provenance-missing.db");
        let store = MainChatAgentEventStore::new(&db_path).unwrap();
        let task_id = "task-provider-policy-legacy-reopen";
        let run_id = "run-provider-policy-legacy-reopen";
        let request_id = "request-provider-policy-legacy-reopen";
        let started_at = Utc::now();
        let start_id = stable_event_id(task_id, 1, "provider.started", request_id);
        let terminal_id = stable_event_id(task_id, 2, "provider.completed", request_id);
        {
            let conn = store.lock_conn().unwrap();
            for (event_id, sequence, event_type, created_at, status) in [
                (&start_id, 1_i64, "provider.started", started_at, "started"),
                (
                    &terminal_id,
                    2_i64,
                    "provider.completed",
                    started_at + chrono::Duration::milliseconds(1),
                    "completed",
                ),
            ] {
                let payload = json!({
                    "status": status,
                    "requestId": request_id,
                    "provider": "provider-legacy",
                    "model": "model-legacy"
                });
                conn.execute(
                    "INSERT INTO main_chat_agent_events (
                        event_id, task_session_id, run_id, sequence, event_type, object_type,
                        object_id, created_at, source, payload_digest, payload_json, backfilled,
                        payload_minimized_version
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 'provider_request', ?6, ?7,
                               'provider_adapter', ?8, ?9, 0, 2)",
                    params![
                        event_id,
                        task_id,
                        run_id,
                        sequence,
                        event_type,
                        request_id,
                        created_at.to_rfc3339(),
                        format!("legacy-digest-{sequence}"),
                        payload.to_string(),
                    ],
                )
                .unwrap();
            }
        }
        drop(store);

        let reopened = MainChatAgentEventStore::new(&db_path).unwrap();
        assert!(reopened.list(task_id, 0, 10).unwrap().is_empty());
        assert!(reopened.event_by_id(&start_id).unwrap().is_none());
        assert!(reopened.event_by_id(&terminal_id).unwrap().is_none());
        assert_eq!(
            reopened
                .list_identity_quarantine_receipts(10)
                .unwrap()
                .len(),
            1
        );
        drop(reopened);

        let reopened_again = MainChatAgentEventStore::new(&db_path).unwrap();
        assert!(reopened_again.list(task_id, 0, 10).unwrap().is_empty());
        assert_eq!(
            reopened_again
                .list_identity_quarantine_receipts(10)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn preexisting_provider_terminal_without_start_is_quarantined_not_upgraded_to_v7_truth() {
        // As above, this is a preexisting payload-version row in the current
        // table contract, not an invented historical schema fixture.
        let directory = tempfile::tempdir().unwrap();
        let db_path = directory.path().join("orphan-provider-lifecycle.db");
        let store = MainChatAgentEventStore::new(&db_path).unwrap();
        let payload = json!({
            "status": "completed",
            "requestId": "request-orphan-legacy",
            "provider": "provider-legacy",
            "model": "model-legacy",
        });
        let event_id = stable_event_id(
            "task-orphan-legacy",
            1,
            "provider.completed",
            "request-orphan-legacy",
        );
        {
            let conn = store.lock_conn().unwrap();
            conn.execute(
                "INSERT INTO main_chat_agent_events (
                    event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
                 ) VALUES (?1, 'task-orphan-legacy', 'run-orphan-legacy', 1,
                           'provider.completed', 'provider_request', 'request-orphan-legacy', ?2,
                           'provider_adapter', 'legacy-orphan-payload-digest', ?3, 0, 2)",
                params![event_id, Utc::now().to_rfc3339(), payload.to_string()],
            )
            .unwrap();
        }
        drop(store);

        let reopened = MainChatAgentEventStore::new(&db_path).unwrap();
        assert!(reopened
            .list("task-orphan-legacy", 0, 10)
            .unwrap()
            .is_empty());
        assert!(reopened.event_by_id(&event_id).unwrap().is_none());
        let receipts = reopened.list_identity_quarantine_receipts(10).unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].event_count, 1);
        assert_eq!(
            receipts[0].reason_code,
            "legacy_provider_lifecycle_unverified"
        );
        drop(reopened);

        let reopened_again = MainChatAgentEventStore::new(&db_path).unwrap();
        assert_eq!(
            reopened_again
                .list_identity_quarantine_receipts(10)
                .unwrap()
                .len(),
            1,
            "quarantine migration must be restart-idempotent"
        );
    }

    #[test]
    fn ordinary_replay_list_rejects_provider_terminal_whose_start_is_missing() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-provider-read-corruption";
        let run_id = "run-provider-read-corruption";
        let request_id = "request-provider-read-corruption";
        let started = store
            .append(provider_started_draft(
                task_id,
                run_id,
                request_id,
                "provider-a",
            ))
            .unwrap();
        let terminal = store
            .append(provider_terminal_draft(
                task_id,
                run_id,
                request_id,
                "provider-a",
                "provider.completed",
                started.created_at + chrono::Duration::milliseconds(1),
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        {
            let conn = store.lock_conn().unwrap();
            let payload_json = serde_json::to_string(&terminal.payload).unwrap();
            let rewritten_event_id = stable_event_id(task_id, 1, "provider.completed", request_id);
            let rewritten_digest =
                stored_event_payload_digest("provider.completed", 1, &payload_json);
            conn.execute(
                "DELETE FROM main_chat_agent_event_immutable_identities
                 WHERE task_session_id = ?1",
                [task_id],
            )
            .unwrap();
            conn.execute(
                "DELETE FROM main_chat_agent_events WHERE event_id = ?1",
                [&started.event_id],
            )
            .unwrap();
            conn.execute(
                "UPDATE main_chat_agent_events
                 SET event_id = ?1, sequence = 1, payload_digest = ?2
                 WHERE event_id = ?3",
                params![rewritten_event_id, rewritten_digest, terminal.event_id],
            )
            .unwrap();
            conn.execute(
                "UPDATE main_chat_agent_event_sequences SET last_sequence = 1
                 WHERE task_session_id = ?1",
                [task_id],
            )
            .unwrap();
        }

        let error = store
            .list(task_id, 0, 10)
            .expect_err("gap replay must not receive an orphan provider terminal as current truth");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_unverified:terminal_without_start"));
    }

    #[test]
    fn digest_consistent_provider_request_mismatch_fails_every_task_read_surface() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-provider-request-mismatch";
        let run_id = "run-provider-request-mismatch";
        let event = store
            .append(provider_started_draft(
                task_id,
                run_id,
                "request-provider-request-mismatch",
                "provider-a",
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        let mut mismatched_payload = event.payload.clone();
        mismatched_payload["requestId"] = json!("different-request-id");
        let payload_json = serde_json::to_string(&mismatched_payload).unwrap();
        let payload_digest =
            stored_event_payload_digest(&event.event_type, event.sequence, &payload_json);
        store
            .lock_conn()
            .unwrap()
            .execute(
                "UPDATE main_chat_agent_events
                 SET payload_json = ?1, payload_digest = ?2
                 WHERE event_id = ?3",
                params![payload_json, payload_digest, event.event_id],
            )
            .unwrap();

        for error in [
            store.event_by_id(&event.event_id).unwrap_err(),
            store.list(task_id, 0, 10).unwrap_err(),
            store
                .list(task_id, event.sequence, 10)
                .expect_err("an empty pagination window must still validate task truth"),
            store.latest_run_id(task_id).unwrap_err(),
            store.latest_sequence(task_id).unwrap_err(),
            store.latest_provider_event().unwrap_err(),
        ] {
            assert!(
                error
                    .to_string()
                    .contains("provider_lifecycle_unverified:request_id_mismatch"),
                "unexpected read result: {error}"
            );
        }
    }

    #[test]
    fn persisted_provider_lifecycle_rejects_source_drift() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-provider-source-drift";
        let run_id = "run-provider-source-drift";
        let request_id = "request-provider-source-drift";
        let start = store
            .append(provider_started_draft(
                task_id,
                run_id,
                request_id,
                "provider-a",
            ))
            .unwrap();
        let terminal = store
            .append(provider_terminal_draft(
                task_id,
                run_id,
                request_id,
                "provider-a",
                "provider.completed",
                start.created_at + chrono::Duration::milliseconds(1),
            ))
            .unwrap();
        store
            .disable_event_integrity_triggers_for_corruption_test()
            .unwrap();
        store
            .lock_conn()
            .unwrap()
            .execute(
                "UPDATE main_chat_agent_events SET source = 'openlife_turn_runtime'
                 WHERE event_id = ?1",
                [&terminal.event_id],
            )
            .unwrap();

        let error = store
            .event_by_id(&start.event_id)
            .expect_err("either member of a drifted lifecycle must fail closed");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_unverified:source_unverified"));
        assert!(store.list(task_id, 0, 10).is_err());
    }

    #[test]
    fn runtime_completed_without_adapter_observation_is_rejected() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-runtime-provider-completed-unobserved";
        let run_id = "run-runtime-provider-completed-unobserved";
        let request_id = "request-runtime-provider-completed-unobserved";
        let started_at = Utc::now();
        let error = append_main_chat_agent_runtime_event_batch_in_store(
            &store,
            task_id,
            run_id,
            vec![
                MainChatAgentRuntimeEventInput::new(
                    "provider.started",
                    "provider_request",
                    request_id,
                    "openlife_turn_runtime",
                    json!({
                        "status": "started",
                        "requestId": request_id,
                        "provider": "provider-a",
                        "model": "model-a",
                    }),
                )
                .with_occurred_at(started_at),
                MainChatAgentRuntimeEventInput::new(
                    "provider.completed",
                    "provider_request",
                    request_id,
                    "openlife_turn_runtime",
                    json!({
                        "status": "completed",
                        "requestId": request_id,
                        "provider": "provider-a",
                        "model": "model-a",
                        "errorDigest": Value::Null,
                    }),
                )
                .with_occurred_at(started_at + chrono::Duration::milliseconds(1)),
            ],
        )
        .expect_err("runtime cannot invent an adapter start and completed terminal");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_runtime_admission_missing"));
        assert!(store.list(task_id, 0, 10).unwrap().is_empty());
    }

    #[test]
    fn kernel_failure_provider_batch_rolls_back_when_a_later_closure_draft_is_invalid() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-kernel-provider-batch-rollback";
        let run_id = "run-kernel-provider-batch-rollback";
        let request_id = "request-kernel-provider-batch-rollback";
        let failure_id = "terminal:run-kernel-provider-batch-rollback:unknown_error";
        let started_at = Utc::now();
        let observed_at = started_at + chrono::Duration::milliseconds(1);
        let (mut provider_drafts, scope, proof) = synthetic_provider_pair(
            task_id,
            run_id,
            request_id,
            "provider-a",
            "model-a",
            ProviderInvocationStatus::RemoteUnknown,
            started_at,
            observed_at,
        );
        let start = provider_drafts.remove(0);
        let mut runtime_unknown = provider_drafts.remove(0);
        runtime_unknown.source = "openlife_turn_runtime".into();
        runtime_unknown.payload["startedAt"] = json!(started_at);
        runtime_unknown.payload["observedAt"] = json!(observed_at);
        runtime_unknown.payload["localKernelFutureDropped"] = json!(true);
        runtime_unknown.payload["adapterTerminalObserved"] = json!(true);
        runtime_unknown.payload["kernelFailureReceiptId"] = json!(failure_id);
        runtime_unknown.payload["reasonCode"] =
            json!("kernel_failed_before_provider_terminal_observed");
        let failure = MainChatAgentEventDraft {
            task_session_id: task_id.into(),
            run_id: run_id.into(),
            event_type: "failed".into(),
            object_type: "turn".into(),
            object_id: failure_id.into(),
            created_at: observed_at,
            source: "openlife_turn_runtime.kernel_error_pre_commit".into(),
            payload: json!({
                "status": "failed",
                "kind": "unknown_error",
                "errorDigest": format!("sha256:{}", "f".repeat(64)),
                "durableCommitAllowedAfterFailure": false,
            }),
            backfilled: false,
        };
        let error = store
            .append_provider_lifecycle_for_test(
                vec![start, failure, runtime_unknown],
                &scope,
                &[proof],
            )
            .expect_err("a bad later closure draft must roll the whole batch back");
        assert!(error
            .to_string()
            .contains("runtime_unknown_conflicts_with_adapter_terminal"));
        assert!(
            store.list(task_id, 0, 10).unwrap().is_empty(),
            "the adapter start and failed-turn receipt must not survive a later draft failure"
        );
    }

    #[test]
    fn persisted_provider_lifecycle_accepts_only_the_complete_runtime_cancel_transition() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-provider-runtime-cancel";
        let run_id = "run-provider-runtime-cancel";
        let request_id = "request-provider-runtime-cancel";
        let cancellation_id = "cancellation:task-provider-runtime-cancel";
        let start = store
            .append(provider_started_draft(
                task_id,
                run_id,
                request_id,
                "provider-a",
            ))
            .unwrap();
        let observed_at = start.created_at + chrono::Duration::milliseconds(1);
        store
            .append(MainChatAgentEventDraft {
                task_session_id: task_id.into(),
                run_id: run_id.into(),
                event_type: "cancel_requested".into(),
                object_type: "turn".into(),
                object_id: cancellation_id.into(),
                created_at: observed_at,
                source: "openlife_turn_runtime".into(),
                payload: json!({
                    "status": "cancel_requested",
                    "cancellationId": cancellation_id,
                    "observedAt": observed_at,
                    "durableCommitAllowedAfterCancel": false,
                    "localWaitAborted": true,
                    "remoteCancellationConfirmed": false,
                }),
                backfilled: false,
            })
            .unwrap();
        let (policy_evidence, start_only_proof) = synthetic_provider_start_only_proof(
            request_id,
            "provider-a",
            "model-1",
            start.created_at,
        );
        let mut runtime_cancel = MainChatAgentEventDraft {
            task_session_id: task_id.into(),
            run_id: run_id.into(),
            event_type: "provider.remote_unknown".into(),
            object_type: "provider_request".into(),
            object_id: request_id.into(),
            created_at: observed_at,
            source: "openlife_turn_runtime".into(),
            payload: json!({
                "status": "remote_unknown",
                "requestId": request_id,
                "provider": "provider-a",
                "model": "model-1",
                "startedAt": start.created_at,
                "observedAt": observed_at,
                "cancellationId": cancellation_id,
                "localWaitAborted": true,
                "localKernelFutureDropped": true,
                "remoteCancellationConfirmed": false,
            }),
            backfilled: false,
        };
        append_provider_policy_evidence_payload(&mut runtime_cancel.payload, &policy_evidence)
            .unwrap();
        let scope = crate::main_chat_turn_runtime::MainChatProviderDurabilityScope::test_fixture(
            task_id, run_id,
        );
        store
            .append_provider_lifecycle_for_test(vec![runtime_cancel], &scope, &[start_only_proof])
            .unwrap();

        assert_eq!(store.list(task_id, 0, 10).unwrap().len(), 3);

        let incomplete_store = MainChatAgentEventStore::new_in_memory().unwrap();
        let incomplete_start = incomplete_store
            .append(provider_started_draft(
                "task-provider-runtime-cancel-incomplete",
                "run-provider-runtime-cancel-incomplete",
                "request-provider-runtime-cancel-incomplete",
                "provider-a",
            ))
            .unwrap();
        let incomplete_observed_at =
            incomplete_start.created_at + chrono::Duration::milliseconds(1);
        let (incomplete_policy_evidence, incomplete_start_only_proof) =
            synthetic_provider_start_only_proof(
                "request-provider-runtime-cancel-incomplete",
                "provider-a",
                "model-1",
                incomplete_start.created_at,
            );
        let mut incomplete_runtime_cancel = MainChatAgentEventDraft {
            task_session_id: "task-provider-runtime-cancel-incomplete".into(),
            run_id: "run-provider-runtime-cancel-incomplete".into(),
            event_type: "provider.remote_unknown".into(),
            object_type: "provider_request".into(),
            object_id: "request-provider-runtime-cancel-incomplete".into(),
            created_at: incomplete_observed_at,
            source: "openlife_turn_runtime".into(),
            payload: json!({
                "status": "remote_unknown",
                "requestId": "request-provider-runtime-cancel-incomplete",
                "provider": "provider-a",
                "model": "model-1",
                "startedAt": incomplete_start.created_at,
                "observedAt": incomplete_observed_at,
                "cancellationId": "cancellation:incomplete",
                "localWaitAborted": false,
                "localKernelFutureDropped": true,
                "remoteCancellationConfirmed": false,
            }),
            backfilled: false,
        };
        append_provider_policy_evidence_payload(
            &mut incomplete_runtime_cancel.payload,
            &incomplete_policy_evidence,
        )
        .unwrap();
        let incomplete_scope =
            crate::main_chat_turn_runtime::MainChatProviderDurabilityScope::test_fixture(
                "task-provider-runtime-cancel-incomplete",
                "run-provider-runtime-cancel-incomplete",
            );
        let error = incomplete_store
            .append_provider_lifecycle_for_test(
                vec![incomplete_runtime_cancel],
                &incomplete_scope,
                &[incomplete_start_only_proof],
            )
            .expect_err("an incomplete runtime cancel payload must not authorize source drift");
        assert!(error.to_string().contains("source_mismatch"));
        assert_eq!(
            incomplete_store
                .list("task-provider-runtime-cancel-incomplete", 0, 10)
                .unwrap()
                .len(),
            1,
            "the independently committed adapter start remains truthful"
        );
    }

    #[test]
    fn persisted_provider_lifecycle_fails_closed_above_the_per_turn_attempt_cap() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let task_id = "task-provider-lifecycle-cap";
        let run_id = "run-provider-lifecycle-cap";
        for index in 0..crate::main_chat_cancellation::MAX_PROVIDER_ATTEMPTS_PER_TURN {
            let request_id = format!("request-provider-cap-{index}");
            let start = store
                .append(provider_started_draft(
                    task_id,
                    run_id,
                    &request_id,
                    "provider-a",
                ))
                .unwrap();
            store
                .append(provider_terminal_draft(
                    task_id,
                    run_id,
                    &request_id,
                    "provider-a",
                    "provider.completed",
                    start.created_at,
                ))
                .unwrap();
        }
        store
            .append(provider_started_draft(
                task_id,
                run_id,
                "request-provider-cap-overflow",
                "provider-a",
            ))
            .unwrap();

        let error = store
            .list(task_id, 0, 250)
            .expect_err("cap plus one must fail before allocating an unbounded task history");
        assert!(error
            .to_string()
            .contains("provider_lifecycle_unverified:provider_lifecycle_limit_exceeded"));
    }

    #[test]
    fn legacy_v3_event_payload_migrates_through_the_current_event_schema() {
        const LEGACY_PRIVATE_VALUE: &str = "LEGACY-V3-PRIVATE-VALUE-74291";
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let conn = store.lock_conn().unwrap();
        let legacy_payload = json!({
            "gapId": "gap-legacy-v3",
            "gapCode": "legacy_schema_gap",
            "evidenceId": "evidence-legacy-v3",
            "provider": "must-not-survive-under-diagnostic-event",
            "privateNote": LEGACY_PRIVATE_VALUE,
        });
        let event_id = stable_event_id("task-legacy-v3", 1, "diagnostic.created", "gap-legacy-v3");
        conn.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled,
                payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 3)",
            params![
                event_id,
                "task-legacy-v3",
                "run-legacy-v3",
                "diagnostic.created",
                "diagnostic",
                "gap-legacy-v3",
                Utc::now().to_rfc3339(),
                "diagnostic",
                "legacy-v3-digest",
                legacy_payload.to_string(),
            ],
        )
        .unwrap();

        migrate_legacy_event_payloads(&conn, &store.digest_key).unwrap();
        let (payload_json, version): (String, i64) = conn
            .query_row(
                "SELECT payload_json, payload_minimized_version
                 FROM main_chat_agent_events WHERE event_id = ?1",
                [&event_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let payload: Value = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(payload["gapId"], "gap-legacy-v3");
        assert_eq!(payload["gapCode"], "legacy_schema_gap");
        assert!(payload.get("provider").is_none());
        assert!(!payload_json.contains(LEGACY_PRIVATE_VALUE));
        assert_eq!(payload[UNRECOGNIZED_FIELDS_RECEIPT]["fieldCount"], 2);
        assert_eq!(version, DURABLE_EVENT_PAYLOAD_VERSION);
    }

    #[test]
    fn legacy_v6_unknown_field_receipt_is_rekeyed_without_inventing_field_values() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let conn = store.lock_conn().unwrap();
        let event_id = stable_event_id(
            "task-legacy-v6-receipt",
            1,
            "diagnostic.created",
            "gap-legacy-v6-receipt",
        );
        let legacy_payload = json!({
            "gapId": "gap-legacy-v6-receipt",
            "gapCode": "legacy_unknown_fields",
            "unrecognizedFieldsReceipt": {
                "redacted": true,
                "valueType": "object",
                "fieldCount": 2,
            },
        });
        conn.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled,
                payload_minimized_version
             ) VALUES (?1, 'task-legacy-v6-receipt', 'run-legacy-v6-receipt', 1,
                       'diagnostic.created', 'diagnostic', 'gap-legacy-v6-receipt', ?2,
                       'diagnostic', 'legacy-v6-digest', ?3, 0, 6)",
            params![
                event_id,
                Utc::now().to_rfc3339(),
                legacy_payload.to_string()
            ],
        )
        .unwrap();

        migrate_legacy_event_payloads(&conn, &store.digest_key).unwrap();
        let migrated_json: String = conn
            .query_row(
                "SELECT payload_json FROM main_chat_agent_events WHERE event_id = ?1",
                [&event_id],
                |row| row.get(0),
            )
            .unwrap();
        let migrated: Value = serde_json::from_str(&migrated_json).unwrap();
        let receipt = &migrated[UNRECOGNIZED_FIELDS_RECEIPT];
        assert_eq!(receipt["fieldCount"], 2);
        assert_eq!(receipt["typeCounts"]["unknown"], 2);
        assert_eq!(receipt["digestScope"], "keyed_legacy_unknown_fields_v1");
        assert!(receipt["digest"].as_str().is_some_and(is_exact_hmac_digest));
        assert!(!migrated_json.contains("private"));
    }

    #[test]
    fn legacy_v4_fixed_transition_materializes_exact_status_during_migration() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let conn = store.lock_conn().unwrap();
        let event_id = stable_event_id(
            "task-legacy-v4-action-status",
            1,
            "action.completed",
            "action-legacy-v4-status",
        );
        let payload = json!({
            "actionId": "action-legacy-v4-status",
            "observationIds": [],
        })
        .to_string();
        conn.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled,
                payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, 'action.completed', 'action', ?4, ?5,
                       'action_executor', ?6, ?7, 0, 4)",
            params![
                event_id,
                "task-legacy-v4-action-status",
                "run-legacy-v4-action-status",
                "action-legacy-v4-status",
                Utc::now().to_rfc3339(),
                stored_event_payload_digest("action.completed", 1, &payload),
                payload,
            ],
        )
        .unwrap();

        migrate_legacy_event_payloads(&conn, &store.digest_key).unwrap();
        let migrated: String = conn
            .query_row(
                "SELECT payload_json FROM main_chat_agent_events WHERE event_id = ?1",
                [&event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&migrated).unwrap()["status"],
            "succeeded"
        );
    }

    #[test]
    fn legacy_v3_migration_fails_closed_on_a_wrong_typed_recognized_field() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let conn = store.lock_conn().unwrap();
        let legacy_payload = json!({
            "taskSessionId": "task-legacy-v3-invalid",
            "status": false,
        });
        let event_id = stable_event_id(
            "task-legacy-v3-invalid",
            1,
            "task.updated",
            "task-legacy-v3-invalid",
        );
        conn.execute(
            "INSERT INTO main_chat_agent_events (
                event_id, task_session_id, run_id, sequence, event_type, object_type,
                object_id, created_at, source, payload_digest, payload_json, backfilled,
                payload_minimized_version
             ) VALUES (?1, ?2, ?3, 1, 'task.updated', 'task', ?2, ?4, 'legacy',
                       'legacy-v3-invalid-digest', ?5, 0, 3)",
            params![
                event_id,
                "task-legacy-v3-invalid",
                "run-legacy-v3-invalid",
                Utc::now().to_rfc3339(),
                legacy_payload.to_string(),
            ],
        )
        .unwrap();

        let error = migrate_legacy_event_payloads(&conn, &store.digest_key)
            .expect_err("a legacy recognized field with the wrong type is store corruption");
        assert!(error
            .to_string()
            .contains("invalid_field_type_or_bound:status"));
        let version: i64 = conn
            .query_row(
                "SELECT payload_minimized_version FROM main_chat_agent_events
                 WHERE event_id = ?1",
                [&event_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 3, "failed migration must roll back atomically");
    }

    #[test]
    fn legacy_event_payload_migration_is_atomic_when_a_later_nonempty_payload_is_malformed() {
        const FIRST_PRIVATE_BODY: &str = "LEGACY_EVENT_ATOMIC_PRIVATE_BODY";
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let conn = store.lock_conn().unwrap();
        let valid_legacy_payload = json!({
            "gapId": "gap-legacy-atomic-valid",
            "gapCode": "legacy_atomic_valid",
            "privateNote": FIRST_PRIVATE_BODY,
        })
        .to_string();
        for (_legacy_event_id, task_id, run_id, object_id, payload_json) in [
            (
                "legacy-atomic-valid-event",
                "task-legacy-atomic-valid",
                "run-legacy-atomic-valid",
                "gap-legacy-atomic-valid",
                valid_legacy_payload.as_str(),
            ),
            (
                "legacy-atomic-malformed-event",
                "task-legacy-atomic-malformed",
                "run-legacy-atomic-malformed",
                "gap-legacy-atomic-malformed",
                "{broken-json",
            ),
        ] {
            let event_id = stable_event_id(task_id, 1, "diagnostic.created", object_id);
            conn.execute(
                "INSERT INTO main_chat_agent_events (
                    event_id, task_session_id, run_id, sequence, event_type, object_type,
                    object_id, created_at, source, payload_digest, payload_json, backfilled,
                    payload_minimized_version
                 ) VALUES (?1, ?2, ?3, 1, 'diagnostic.created', 'diagnostic', ?4, ?5,
                           'legacy', 'legacy-atomic-digest', ?6, 0, 3)",
                params![
                    event_id,
                    task_id,
                    run_id,
                    object_id,
                    Utc::now().to_rfc3339(),
                    payload_json,
                ],
            )
            .unwrap();
        }

        let error = migrate_legacy_event_payloads(&conn, &store.digest_key)
            .expect_err("a later malformed durable payload must roll back the whole migration")
            .to_string();
        assert!(error.contains("legacy_invalid_json"), "{error}");

        let rows = conn
            .prepare(
                "SELECT event_id, payload_json, payload_minimized_version
                 FROM main_chat_agent_events
                 WHERE task_session_id LIKE 'task-legacy-atomic-%'
                 ORDER BY event_id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|(_, _, version)| *version == 3));
        assert!(rows
            .iter()
            .any(|(_, payload, _)| payload.contains(FIRST_PRIVATE_BODY)));
    }

    #[test]
    fn canonical_tombstone_hides_event_replay_without_mutating_append_only_facts() {
        let store = MainChatAgentEventStore::new_in_memory().unwrap();
        let started = store
            .append(provider_started_draft(
                "deleted-task",
                "deleted-run",
                "request-delete-test",
                "provider-a",
            ))
            .unwrap();
        let event = store
            .append(provider_terminal_draft(
                "deleted-task",
                "deleted-run",
                "request-delete-test",
                "provider-a",
                "provider.completed",
                started.created_at + chrono::Duration::milliseconds(1),
            ))
            .unwrap();
        assert_eq!(store.list("deleted-task", 0, 10).unwrap().len(), 2);

        assert_eq!(
            store
                .project_agent_run_canonical_head(
                    "delete-event",
                    1,
                    "deleted-run",
                    Some("delete-tombstone"),
                    &["delete-tombstone".into()],
                )
                .unwrap(),
            2
        );
        assert_eq!(
            store
                .project_agent_run_canonical_head(
                    "delete-event",
                    1,
                    "deleted-run",
                    Some("delete-tombstone"),
                    &["delete-tombstone".into()],
                )
                .unwrap(),
            0
        );
        assert!(store.list("deleted-task", 0, 10).unwrap().is_empty());
        assert!(store.latest_provider_event().unwrap().is_none());
        assert!(
            select_event_by_id(&store.lock_conn().unwrap(), &event.event_id)
                .unwrap()
                .is_some()
        );
        assert!(store
            .append(MainChatAgentEventDraft {
                task_session_id: "deleted-task".into(),
                run_id: "deleted-run".into(),
                event_type: "failed".into(),
                object_type: "turn".into(),
                object_id: "late-event".into(),
                created_at: Utc::now(),
                source: "turn_runtime".into(),
                payload: json!({"status": "failed"}),
                backfilled: false,
            })
            .is_err());

        store
            .project_agent_run_canonical_head(
                "restore-event",
                2,
                "deleted-run",
                None,
                &["delete-tombstone".into()],
            )
            .unwrap();
        assert_eq!(store.list("deleted-task", 0, 10).unwrap().len(), 2);
        assert!(store
            .project_agent_run_canonical_head(
                "late-delete-event",
                1,
                "deleted-run",
                Some("delete-tombstone"),
                &["delete-tombstone".into()],
            )
            .unwrap_err()
            .to_string()
            .contains("ahead of canonical source"));
        assert_eq!(store.list("deleted-task", 0, 10).unwrap().len(), 2);
    }
}
