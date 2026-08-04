use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionPolicy {
    Allow,
    Deny,
    AskEveryTime,
    AllowOnce,
    AllowUntilRevoked,
}

impl std::fmt::Display for ToolPermissionPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Deny => write!(f, "deny"),
            Self::AskEveryTime => write!(f, "ask_every_time"),
            Self::AllowOnce => write!(f, "allow_once"),
            Self::AllowUntilRevoked => write!(f, "allow_until_revoked"),
        }
    }
}

impl std::str::FromStr for ToolPermissionPolicy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            "ask_every_time" => Ok(Self::AskEveryTime),
            "allow_once" => Ok(Self::AllowOnce),
            "allow_until_revoked" => Ok(Self::AllowUntilRevoked),
            other => Err(anyhow::anyhow!("unknown tool permission policy: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionRecord {
    pub id: String,
    pub tool_name: String,
    pub source: String,
    pub risk_level: String,
    pub action_type: String,
    pub policy: ToolPermissionPolicy,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPermissionDecision {
    pub allowed: bool,
    pub requires_confirmation: bool,
    pub decision: String,
    pub reason: String,
    pub policy_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewedNetworkPermissionAuthority {
    ConsumedByCanonicalStore,
}

/// Non-serializable proof that the canonical ToolPermissionStore consumed the
/// exact ReviewWorkflow-linked network AllowOnce with a successful CAS.
#[derive(Debug)]
pub struct ConsumedReviewedNetworkPermission {
    permission_id: String,
    proposal_id: String,
    tool_name: String,
    source: String,
    risk_level: String,
    action_type: String,
    authority: ReviewedNetworkPermissionAuthority,
}

impl ConsumedReviewedNetworkPermission {
    pub fn permission_id(&self) -> &str {
        &self.permission_id
    }

    fn validate_scope(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<()> {
        if self.authority != ReviewedNetworkPermissionAuthority::ConsumedByCanonicalStore
            || self.permission_id.trim().is_empty()
            || self.proposal_id.trim().is_empty()
            || self.tool_name != tool_name
            || self.source != source
            || self.risk_level != risk_level
            || self.action_type != action_type
        {
            anyhow::bail!("consumed reviewed network permission scope mismatch");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBoundToolPermissionScope {
    pub tool_name: String,
    pub source: String,
    pub risk_level: String,
    pub manifest_action_type: String,
    pub queue_action_type: String,
    pub requested_target: String,
    pub resolved_target: String,
    pub input_hash: String,
    pub input_length_bytes: u64,
}

/// The product action identity that selected the concrete tool execution.
///
/// The manifest identity alone is insufficient: two queue actions may resolve
/// to the same tool, and a reviewed request target must not authorize a
/// different product action that happens to produce identical arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBoundToolExecutionBinding {
    pub queue_action_type: String,
    pub requested_target: String,
}

impl ActionBoundToolExecutionBinding {
    pub fn validate(&self) -> Result<()> {
        if self.queue_action_type.trim().is_empty() {
            anyhow::bail!(
                "action-bound ToolPermission execution binding missing queue_action_type"
            );
        }
        if self.requested_target.trim().is_empty() {
            anyhow::bail!("action-bound ToolPermission execution binding missing requested_target");
        }
        Ok(())
    }
}

impl ActionBoundToolPermissionScope {
    pub fn from_proposal_after(after: &serde_json::Value) -> Result<Self> {
        let canonical_scope = after
            .get("canonical_scope")
            .or_else(|| after.get("canonicalScope"));
        let top = |aliases: &[&str]| {
            aliases
                .iter()
                .find_map(|key| after.get(key).and_then(serde_json::Value::as_str))
                .or_else(|| {
                    canonical_scope.and_then(|scope| {
                        aliases
                            .iter()
                            .find_map(|key| scope.get(key).and_then(serde_json::Value::as_str))
                    })
                })
                .filter(|value| !value.trim().is_empty())
        };
        let blocked = after
            .get("blocked_action")
            .or_else(|| after.get("blockedAction"))
            .context("action-bound ToolPermission missing blocked_action")?;
        let blocked_string = |aliases: &[&str]| {
            aliases
                .iter()
                .find_map(|key| blocked.get(key).and_then(serde_json::Value::as_str))
                .or_else(|| {
                    canonical_scope.and_then(|scope| {
                        aliases
                            .iter()
                            .find_map(|key| scope.get(key).and_then(serde_json::Value::as_str))
                    })
                })
                .filter(|value| !value.trim().is_empty())
        };
        let input_length_bytes = ["input_length_bytes", "inputLengthBytes"]
            .iter()
            .find_map(|key| blocked.get(key).and_then(serde_json::Value::as_u64))
            .or_else(|| {
                canonical_scope.and_then(|scope| {
                    ["input_length_bytes", "inputLengthBytes"]
                        .iter()
                        .find_map(|key| scope.get(key).and_then(serde_json::Value::as_u64))
                })
            })
            .context("action-bound ToolPermission missing input_length_bytes")?;
        let scope = Self {
            tool_name: top(&["tool_name", "toolName", "name"])
                .context("action-bound ToolPermission missing tool_name")?
                .to_string(),
            source: top(&["source"])
                .context("action-bound ToolPermission missing source")?
                .to_string(),
            risk_level: top(&["risk_level", "riskLevel"])
                .context("action-bound ToolPermission missing risk_level")?
                .to_string(),
            manifest_action_type: top(&["action_type", "actionType"])
                .context("action-bound ToolPermission missing manifest action_type")?
                .to_string(),
            queue_action_type: blocked_string(&["action_type", "actionType"])
                .context("action-bound ToolPermission missing queue action_type")?
                .to_string(),
            requested_target: blocked_string(&["target"])
                .context("action-bound ToolPermission missing requested target")?
                .to_string(),
            resolved_target: blocked_string(&["resolved_target", "resolvedTarget"])
                .or_else(|| top(&["tool_name", "toolName", "name"]))
                .context("action-bound ToolPermission missing resolved target")?
                .to_string(),
            input_hash: blocked_string(&["input_hash", "inputHash"])
                .context("action-bound ToolPermission missing input_hash")?
                .to_string(),
            input_length_bytes,
        };
        scope.validate()?;
        Ok(scope)
    }

    pub fn validate(&self) -> Result<()> {
        for (field, value) in [
            ("tool_name", self.tool_name.as_str()),
            ("source", self.source.as_str()),
            ("risk_level", self.risk_level.as_str()),
            ("manifest_action_type", self.manifest_action_type.as_str()),
            ("queue_action_type", self.queue_action_type.as_str()),
            ("requested_target", self.requested_target.as_str()),
            ("resolved_target", self.resolved_target.as_str()),
            ("input_hash", self.input_hash.as_str()),
        ] {
            if value.trim().is_empty() {
                anyhow::bail!("action-bound ToolPermission scope missing {field}");
            }
        }
        let Some(input_hash_hex) = self.input_hash.strip_prefix("sha256:") else {
            anyhow::bail!("action-bound ToolPermission input_hash is not a sha256 digest");
        };
        if input_hash_hex.len() != 64
            || !input_hash_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            anyhow::bail!(
                "action-bound ToolPermission input_hash is not a canonical sha256 digest"
            );
        }
        Ok(())
    }

    pub fn binding_digest(&self) -> String {
        crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!([
            "action_bound_tool_permission_v1",
            self.tool_name,
            self.source,
            self.risk_level,
            self.manifest_action_type,
            self.queue_action_type,
            self.requested_target,
            self.resolved_target,
            self.input_hash,
            self.input_length_bytes,
        ]))
        .1
    }

    pub fn matches_execution(
        &self,
        execution_binding: &ActionBoundToolExecutionBinding,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        manifest_action_type: &str,
        input: &serde_json::Value,
    ) -> bool {
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(input);
        execution_binding.validate().is_ok()
            && self.queue_action_type == execution_binding.queue_action_type
            && self.requested_target == execution_binding.requested_target
            && self.tool_name == tool_name
            && self.source == source
            && self.risk_level == risk_level
            && self.manifest_action_type == manifest_action_type
            && self.resolved_target == tool_name
            && self.input_hash == input_hash
            && self.input_length_bytes == input_length_bytes as u64
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBoundToolPermissionAuthorization {
    pub permission_id: String,
    pub proposal_id: String,
    pub scope_digest: String,
    pub scope: ActionBoundToolPermissionScope,
    pub execution_binding: Option<ActionBoundToolExecutionBinding>,
}

impl ActionBoundToolPermissionAuthorization {
    pub fn bind_execution(mut self, binding: ActionBoundToolExecutionBinding) -> Result<Self> {
        binding.validate()?;
        self.execution_binding = Some(binding);
        Ok(self)
    }
}

#[derive(Clone)]
pub struct ToolPermissionStore {
    conn: Arc<Mutex<Connection>>,
    explicit_provider_probe_verifier: crate::network_client::ExplicitProviderProbeVerifier,
    explicit_provider_probe_issuer: crate::network_client::ExplicitProviderProbeIssuer,
}

impl ToolPermissionStore {
    fn from_connection(conn: Connection) -> Self {
        let (explicit_provider_probe_verifier, explicit_provider_probe_issuer) =
            crate::network_client::create_explicit_provider_probe_authority();
        Self {
            conn: Arc::new(Mutex::new(conn)),
            explicit_provider_probe_verifier,
            explicit_provider_probe_issuer,
        }
    }

    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self::from_connection(Connection::open(&db_path)?);
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let store = Self::from_connection(Connection::open_in_memory()?);
        store.init_tables()?;
        Ok(store)
    }

    pub fn open_read_only_existing(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        let conn = crate::sqlite_migration::open_existing_read_only(
            &db_path,
            "tool_permission_store",
            &["tool_permissions"],
        )?;
        Ok(Self::from_connection(conn))
    }

    pub fn unavailable_sentinel() -> Result<Self> {
        Ok(Self::from_connection(
            crate::sqlite_migration::unavailable_read_only_sentinel("tool_permission_store")?,
        ))
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tool_permissions (
                id TEXT PRIMARY KEY,
                tool_name TEXT NOT NULL,
                source TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                action_type TEXT NOT NULL,
                policy TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                consumed_at TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS action_bound_tool_permissions (
                id TEXT PRIMARY KEY,
                proposal_id TEXT NOT NULL UNIQUE,
                scope_digest TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                source TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                manifest_action_type TEXT NOT NULL,
                queue_action_type TEXT NOT NULL,
                requested_target TEXT NOT NULL,
                resolved_target TEXT NOT NULL,
                input_hash TEXT NOT NULL,
                input_length_bytes INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                consumed_at TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS reviewed_network_permissions (
                permission_id TEXT PRIMARY KEY,
                proposal_id TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                FOREIGN KEY(permission_id) REFERENCES tool_permissions(id)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tool_permissions_lookup
             ON tool_permissions(tool_name, source, risk_level, action_type)",
            [],
        )?;
        Ok(())
    }

    /// Bind a scheduler generation to this canonical store's verifier. The
    /// scheduler never receives the paired issuer and cannot self-authorize.
    pub fn bind_explicit_provider_probe_scheduler(
        &self,
        scheduler: crate::scheduler::InferenceScheduler,
    ) -> crate::scheduler::InferenceScheduler {
        scheduler
            .with_explicit_provider_probe_verifier(self.explicit_provider_probe_verifier.clone())
    }

    /// Issue a provider-probe capability only from either a direct canonical
    /// network allow or a consumed, ReviewWorkflow-created AllowOnce record.
    /// Caller strings are never accepted as consent proof.
    pub fn issue_explicit_provider_probe_grant(
        &self,
        challenge: crate::network_client::ExplicitProviderProbeChallenge,
        effective_network_policy: crate::config::NetworkPolicy,
        original_decision: &crate::network_client::NetworkPolicyDecision,
        effective_decision: crate::network_client::NetworkPolicyDecision,
        reviewed_permission: Option<ConsumedReviewedNetworkPermission>,
    ) -> Result<crate::network_client::ExplicitProviderProbeGrant> {
        use crate::network_client::NetworkPolicyDisposition;

        let capability = format!("provider.{}", challenge.provider_target());
        if original_decision.capability != capability || effective_decision.capability != capability
        {
            anyhow::bail!("explicit_provider_probe_capability_mismatch");
        }

        let consent_reference = if let Some(reviewed_permission) = reviewed_permission {
            if original_decision.disposition != NetworkPolicyDisposition::Ask
                || effective_decision.disposition != NetworkPolicyDisposition::Allow
            {
                anyhow::bail!("explicit_provider_probe_review_decision_mismatch");
            }
            let endpoint_digest =
                crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                    "endpoint": challenge.endpoint(),
                }))
                .1;
            let expected_scope = format!(
                "{}@{}#endpoint:{}",
                capability, original_decision.decision_id, endpoint_digest
            );
            reviewed_permission.validate_scope(&expected_scope, "provider", "high", "network")?;
            let permission_id = reviewed_permission.permission_id.as_str();
            let (record, proposal_id) = {
                let conn = self
                    .conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
                conn.query_row(
                    "SELECT p.id, p.tool_name, p.source, p.risk_level, p.action_type,
                            p.policy, p.created_at, p.expires_at, p.consumed_at,
                            r.proposal_id
                     FROM tool_permissions p
                     INNER JOIN reviewed_network_permissions r ON r.permission_id = p.id
                     WHERE p.id = ?1",
                    [permission_id],
                    |row| Ok((row_to_record(row)?, row.get::<_, String>(9)?)),
                )
                .optional()?
                .ok_or_else(|| {
                    anyhow::anyhow!("explicit_provider_probe_reviewed_permission_missing")
                })?
            };
            if record.tool_name != expected_scope
                || record.source != "provider"
                || record.risk_level != "high"
                || record.action_type != "network"
                || record.policy != ToolPermissionPolicy::AllowOnce
                || record.consumed_at.is_none()
                || record
                    .expires_at
                    .is_some_and(|expires| expires < Utc::now())
                || proposal_id != reviewed_permission.proposal_id
            {
                anyhow::bail!("explicit_provider_probe_reviewed_permission_scope_mismatch");
            }
            format!("review:{proposal_id}:permission:{}", record.id)
        } else {
            if original_decision != &effective_decision
                || original_decision.disposition != NetworkPolicyDisposition::Allow
            {
                anyhow::bail!("explicit_provider_probe_direct_allow_missing");
            }
            format!("policy:{}", original_decision.decision_id)
        };

        self.explicit_provider_probe_issuer
            .issue_governed_probe_grant(
                challenge,
                effective_network_policy,
                effective_decision,
                consent_reference,
            )
    }

    /// Atomically consume the exact ReviewWorkflow-linked network AllowOnce.
    /// A generic permission row, a serialized id, or a losing concurrent
    /// caller cannot produce the returned capability.
    pub fn consume_reviewed_network_once(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<Option<ConsumedReviewedNetworkPermission>> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let candidate = tx
            .query_row(
                "SELECT p.id, r.proposal_id
                 FROM tool_permissions p
                 INNER JOIN reviewed_network_permissions r ON r.permission_id = p.id
                 WHERE p.tool_name = ?1 AND p.source = ?2 AND p.risk_level = ?3
                   AND p.action_type = ?4 AND p.policy = 'allow_once'
                   AND p.consumed_at IS NULL
                   AND (p.expires_at IS NULL OR p.expires_at >= ?5)
                 ORDER BY p.created_at DESC
                 LIMIT 1",
                params![tool_name, source, risk_level, action_type, now],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((permission_id, proposal_id)) = candidate else {
            tx.commit()?;
            return Ok(None);
        };
        let changed = tx.execute(
            "UPDATE tool_permissions SET consumed_at = ?2
             WHERE id = ?1 AND consumed_at IS NULL",
            params![permission_id, now],
        )?;
        if changed != 1 {
            tx.commit()?;
            return Ok(None);
        }
        tx.commit()?;
        Ok(Some(ConsumedReviewedNetworkPermission {
            permission_id,
            proposal_id,
            tool_name: tool_name.to_string(),
            source: source.to_string(),
            risk_level: risk_level.to_string(),
            action_type: action_type.to_string(),
            authority: ReviewedNetworkPermissionAuthority::ConsumedByCanonicalStore,
        }))
    }

    /// Atomically consume the ReviewWorkflow-linked AllowOnce issued by the
    /// exact accepted Proposal. Continuations must use this entrypoint: a
    /// merely scope-equivalent grant belongs to another review decision and
    /// cannot authorize this replay generation.
    pub fn consume_reviewed_network_once_for_proposal(
        &self,
        proposal_id: &str,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<Option<ConsumedReviewedNetworkPermission>> {
        if proposal_id.trim().is_empty() {
            anyhow::bail!("reviewed network permission proposal_id is empty");
        }
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        let candidate = tx
            .query_row(
                "SELECT p.id
                 FROM tool_permissions p
                 INNER JOIN reviewed_network_permissions r ON r.permission_id = p.id
                 WHERE r.proposal_id = ?1
                   AND p.tool_name = ?2 AND p.source = ?3 AND p.risk_level = ?4
                   AND p.action_type = ?5 AND p.policy = 'allow_once'
                   AND p.consumed_at IS NULL
                   AND (p.expires_at IS NULL OR p.expires_at >= ?6)
                 LIMIT 1",
                params![proposal_id, tool_name, source, risk_level, action_type, now],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(permission_id) = candidate else {
            tx.commit()?;
            return Ok(None);
        };
        let changed = tx.execute(
            "UPDATE tool_permissions SET consumed_at = ?2
             WHERE id = ?1 AND consumed_at IS NULL",
            params![permission_id, now],
        )?;
        if changed != 1 {
            tx.commit()?;
            return Ok(None);
        }
        tx.commit()?;
        Ok(Some(ConsumedReviewedNetworkPermission {
            permission_id,
            proposal_id: proposal_id.to_string(),
            tool_name: tool_name.to_string(),
            source: source.to_string(),
            risk_level: risk_level.to_string(),
            action_type: action_type.to_string(),
            authority: ReviewedNetworkPermissionAuthority::ConsumedByCanonicalStore,
        }))
    }

    /// Read-only preflight used before a continuation epoch is opened. The
    /// actual authority is still the atomic exact-Proposal consume above.
    pub fn reviewed_network_once_available_for_proposal(
        &self,
        proposal_id: &str,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<bool> {
        if proposal_id.trim().is_empty() {
            anyhow::bail!("reviewed network permission proposal_id is empty");
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let now = Utc::now().to_rfc3339();
        let count = conn.query_row(
            "SELECT COUNT(*)
             FROM tool_permissions p
             INNER JOIN reviewed_network_permissions r ON r.permission_id = p.id
             WHERE r.proposal_id = ?1
               AND p.tool_name = ?2 AND p.source = ?3 AND p.risk_level = ?4
               AND p.action_type = ?5 AND p.policy = 'allow_once'
               AND p.consumed_at IS NULL
               AND (p.expires_at IS NULL OR p.expires_at >= ?6)",
            params![proposal_id, tool_name, source, risk_level, action_type, now],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count == 1)
    }

    /// Create the exact generic AllowOnce row plus an immutable proposal link
    /// in one SQLite transaction. Only accepted network-policy proposals use
    /// this path; generic grants cannot later masquerade as reviewed consent.
    pub fn grant_reviewed_network_once(
        &self,
        review_acceptance: &crate::agent::review_workflow::ClaimedReviewAcceptanceSnapshot,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<ToolPermissionRecord> {
        review_acceptance.validate()?;
        let proposal = review_acceptance.proposal();
        let after = &proposal.after;
        let canonical_scope = after
            .get("canonical_scope")
            .or_else(|| after.get("canonicalScope"));
        let scope_field = |aliases: &[&str]| {
            aliases
                .iter()
                .find_map(|key| after.get(key).and_then(serde_json::Value::as_str))
                .or_else(|| {
                    canonical_scope.and_then(|scope| {
                        aliases
                            .iter()
                            .find_map(|key| scope.get(key).and_then(serde_json::Value::as_str))
                    })
                })
        };
        let scope_kind = after
            .get("permission_scope_kind")
            .or_else(|| after.get("permissionScopeKind"))
            .and_then(serde_json::Value::as_str);
        let permission = after
            .get("permission")
            .or_else(|| after.get("policy"))
            .and_then(serde_json::Value::as_str);
        let network_decision_id = canonical_scope
            .and_then(|scope| {
                scope
                    .get("network_policy_decision_id")
                    .or_else(|| scope.get("networkPolicyDecisionId"))
            })
            .and_then(serde_json::Value::as_str);
        let endpoint_digest = canonical_scope
            .and_then(|scope| {
                scope
                    .get("endpoint_digest")
                    .or_else(|| scope.get("endpointDigest"))
            })
            .and_then(serde_json::Value::as_str);
        let canonical_scope_suffix =
            network_decision_id
                .zip(endpoint_digest)
                .map(|(decision_id, endpoint_digest)| {
                    format!("@{decision_id}#endpoint:{endpoint_digest}")
                });
        let expected_proposal_risk = match risk_level {
            "medium" => Some(crate::agent::RiskLevel::Medium),
            "high" => Some(crate::agent::RiskLevel::High),
            _ => None,
        };
        if proposal.proposal_type != crate::agent::ProposalType::ToolPermission
            || expected_proposal_risk != Some(proposal.risk_level)
            || scope_kind != Some("network_policy")
            || permission != Some("allow_once")
            || scope_field(&["tool_name", "toolName", "name"]) != Some(tool_name)
            || scope_field(&["source"]) != Some(source)
            || scope_field(&["risk_level", "riskLevel"]) != Some(risk_level)
            || scope_field(&["action_type", "actionType"]) != Some(action_type)
            || canonical_scope_suffix
                .as_deref()
                .is_none_or(|suffix| !tool_name.ends_with(suffix))
            || endpoint_digest.is_none_or(|digest| {
                !digest.strip_prefix("sha256:").is_some_and(|hex| {
                    hex.len() == 64
                        && hex
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
            })
            || tool_name.trim().is_empty()
            || source.trim().is_empty()
            || action_type != "network"
        {
            anyhow::bail!("reviewed network permission scope is invalid");
        }
        let proposal_id = proposal.id.as_str();

        let mut conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let tx = conn.transaction()?;
        if let Some(existing) = tx
            .query_row(
                "SELECT p.id, p.tool_name, p.source, p.risk_level, p.action_type,
                        p.policy, p.created_at, p.expires_at, p.consumed_at
                 FROM tool_permissions p
                 INNER JOIN reviewed_network_permissions r ON r.permission_id = p.id
                 WHERE r.proposal_id = ?1",
                [proposal_id],
                row_to_record,
            )
            .optional()?
        {
            if existing.tool_name != tool_name
                || existing.source != source
                || existing.risk_level != risk_level
                || existing.action_type != action_type
                || existing.policy != ToolPermissionPolicy::AllowOnce
            {
                anyhow::bail!("reviewed network permission proposal scope conflict");
            }
            return Ok(existing);
        }

        let record = ToolPermissionRecord {
            id: Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            source: source.to_string(),
            risk_level: risk_level.to_string(),
            action_type: action_type.to_string(),
            policy: ToolPermissionPolicy::AllowOnce,
            created_at: Utc::now(),
            expires_at: None,
            consumed_at: None,
        };
        tx.execute(
            "INSERT INTO tool_permissions
             (id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL)",
            params![
                record.id,
                record.tool_name,
                record.source,
                record.risk_level,
                record.action_type,
                record.policy.to_string(),
                record.created_at.to_rfc3339(),
            ],
        )?;
        tx.execute(
            "INSERT INTO reviewed_network_permissions (permission_id, proposal_id, created_at)
             VALUES (?1, ?2, ?3)",
            params![record.id, proposal_id, record.created_at.to_rfc3339()],
        )?;
        tx.commit()?;
        Ok(record)
    }

    pub fn grant_action_bound(
        &self,
        proposal_id: &str,
        scope: &ActionBoundToolPermissionScope,
    ) -> Result<ActionBoundToolPermissionAuthorization> {
        scope.validate()?;
        if proposal_id.trim().is_empty() {
            anyhow::bail!("action-bound ToolPermission proposal_id is empty");
        }
        let scope_digest = scope.binding_digest();
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let existing = conn
            .query_row(
                "SELECT id, scope_digest, consumed_at
                 FROM action_bound_tool_permissions
                 WHERE proposal_id = ?1",
                [proposal_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((permission_id, existing_digest, _consumed_at)) = existing {
            if existing_digest != scope_digest {
                anyhow::bail!(
                    "action-bound ToolPermission proposal scope conflicts with existing grant"
                );
            }
            return Ok(ActionBoundToolPermissionAuthorization {
                permission_id,
                proposal_id: proposal_id.to_string(),
                scope_digest,
                scope: scope.clone(),
                execution_binding: None,
            });
        }

        let permission_id = Uuid::new_v4().to_string();
        let input_length_bytes = i64::try_from(scope.input_length_bytes)
            .context("action-bound ToolPermission input length exceeds SQLite INTEGER")?;
        let inserted = conn.execute(
            "INSERT INTO action_bound_tool_permissions (
                id, proposal_id, scope_digest, tool_name, source, risk_level,
                manifest_action_type, queue_action_type, requested_target,
                resolved_target, input_hash, input_length_bytes, created_at, consumed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL)
             ON CONFLICT(proposal_id) DO NOTHING",
            params![
                permission_id,
                proposal_id,
                scope_digest,
                scope.tool_name,
                scope.source,
                scope.risk_level,
                scope.manifest_action_type,
                scope.queue_action_type,
                scope.requested_target,
                scope.resolved_target,
                scope.input_hash,
                input_length_bytes,
                Utc::now().to_rfc3339(),
            ],
        )?;
        if inserted == 0 {
            let (existing_id, existing_digest) = conn.query_row(
                "SELECT id, scope_digest
                 FROM action_bound_tool_permissions
                 WHERE proposal_id = ?1",
                [proposal_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            if existing_digest != scope_digest {
                anyhow::bail!(
                    "action-bound ToolPermission proposal scope conflicts with concurrent grant"
                );
            }
            return Ok(ActionBoundToolPermissionAuthorization {
                permission_id: existing_id,
                proposal_id: proposal_id.to_string(),
                scope_digest,
                scope: scope.clone(),
                execution_binding: None,
            });
        }
        Ok(ActionBoundToolPermissionAuthorization {
            permission_id,
            proposal_id: proposal_id.to_string(),
            scope_digest,
            scope: scope.clone(),
            execution_binding: None,
        })
    }

    pub fn peek_action_bound(
        &self,
        proposal_id: &str,
        scope: &ActionBoundToolPermissionScope,
    ) -> Result<Option<ActionBoundToolPermissionAuthorization>> {
        scope.validate()?;
        let scope_digest = scope.binding_digest();
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT id, consumed_at
             FROM action_bound_tool_permissions
             WHERE proposal_id = ?1 AND scope_digest = ?2",
            params![proposal_id, scope_digest],
            |row| {
                let permission_id = row.get::<_, String>(0)?;
                let consumed_at = row.get::<_, Option<String>>(1)?;
                Ok(consumed_at
                    .is_none()
                    .then(|| ActionBoundToolPermissionAuthorization {
                        permission_id,
                        proposal_id: proposal_id.to_string(),
                        scope_digest: scope_digest.clone(),
                        scope: scope.clone(),
                        execution_binding: None,
                    }))
            },
        )
        .optional()
        .map(Option::flatten)
        .map_err(Into::into)
    }

    // Permission consumption binds the grant to the complete action and tool identity.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn consume_action_bound(
        &self,
        authorization: &ActionBoundToolPermissionAuthorization,
        execution_binding: &ActionBoundToolExecutionBinding,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        manifest_action_type: &str,
        input: &serde_json::Value,
    ) -> Result<ToolPermissionDecision> {
        if authorization.scope.binding_digest() != authorization.scope_digest
            || !authorization.scope.matches_execution(
                execution_binding,
                tool_name,
                source,
                risk_level,
                manifest_action_type,
                input,
            )
        {
            return Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "action_bound_scope_mismatch".to_string(),
                reason: "action-bound ToolPermission does not match the exact execution"
                    .to_string(),
                policy_id: Some(authorization.permission_id.clone()),
            });
        }
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let changed = conn.execute(
            "UPDATE action_bound_tool_permissions
             SET consumed_at = ?4
             WHERE id = ?1 AND proposal_id = ?2 AND scope_digest = ?3
               AND consumed_at IS NULL",
            params![
                authorization.permission_id,
                authorization.proposal_id,
                authorization.scope_digest,
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(if changed == 1 {
            ToolPermissionDecision {
                allowed: true,
                requires_confirmation: false,
                decision: "action_bound_allow_once".to_string(),
                reason: "exact action-bound ToolPermission consumed".to_string(),
                policy_id: Some(authorization.permission_id.clone()),
            }
        } else {
            ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "action_bound_allow_once_already_consumed".to_string(),
                reason: "exact action-bound ToolPermission was consumed by another execution"
                    .to_string(),
                policy_id: Some(authorization.permission_id.clone()),
            }
        })
    }

    pub fn grant(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
        policy: ToolPermissionPolicy,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<ToolPermissionRecord> {
        let record = ToolPermissionRecord {
            id: Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            source: source.to_string(),
            risk_level: risk_level.to_string(),
            action_type: action_type.to_string(),
            policy,
            created_at: Utc::now(),
            expires_at,
            consumed_at: None,
        };
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO tool_permissions
             (id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.tool_name,
                record.source,
                record.risk_level,
                record.action_type,
                record.policy.to_string(),
                record.created_at.to_rfc3339(),
                record.expires_at.map(|t| t.to_rfc3339()),
                record.consumed_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(record)
    }

    pub fn list(&self) -> Result<Vec<ToolPermissionRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at
             FROM tool_permissions ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_record)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn action_bound_permission_count(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count = conn.query_row(
            "SELECT COUNT(*) FROM action_bound_tool_permissions",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        usize::try_from(count).context("negative action-bound ToolPermission count")
    }

    pub fn revoke(&self, id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        Ok(conn.execute("DELETE FROM tool_permissions WHERE id = ?1", [id])? > 0)
    }

    pub fn check(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
        capabilities: &[String],
    ) -> Result<ToolPermissionDecision> {
        let record = self.find_best(tool_name, source, risk_level, action_type)?;
        let Some(record) = record else {
            let asks = risk_level == "high"
                || capabilities.iter().any(|c| {
                    matches!(
                        c.as_str(),
                        "write" | "memory" | "lifemodel" | "filesystem" | "external_side_effect"
                    )
                });
            return Ok(ToolPermissionDecision {
                allowed: !asks,
                requires_confirmation: asks,
                decision: if asks { "ask_every_time" } else { "allow" }.to_string(),
                reason: if asks {
                    "no policy for high-risk/write action".to_string()
                } else {
                    "low-risk read action allowed by default".to_string()
                },
                policy_id: None,
            });
        };

        if record
            .expires_at
            .is_some_and(|expires| expires < Utc::now())
        {
            return Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "expired".to_string(),
                reason: "matching policy expired".to_string(),
                policy_id: Some(record.id),
            });
        }

        match record.policy {
            ToolPermissionPolicy::Allow | ToolPermissionPolicy::AllowUntilRevoked => {
                Ok(ToolPermissionDecision {
                    allowed: true,
                    requires_confirmation: false,
                    decision: record.policy.to_string(),
                    reason: "matching allow policy".to_string(),
                    policy_id: Some(record.id),
                })
            }
            ToolPermissionPolicy::Deny => Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "deny".to_string(),
                reason: "matching deny policy".to_string(),
                policy_id: Some(record.id),
            }),
            ToolPermissionPolicy::AskEveryTime => Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "ask_every_time".to_string(),
                reason: "matching ask-every-time policy".to_string(),
                policy_id: Some(record.id),
            }),
            ToolPermissionPolicy::AllowOnce => {
                if self.consume_if_available(&record.id)? {
                    Ok(ToolPermissionDecision {
                        allowed: true,
                        requires_confirmation: false,
                        decision: "allow_once".to_string(),
                        reason: "matching allow-once policy consumed".to_string(),
                        policy_id: Some(record.id),
                    })
                } else {
                    Ok(ToolPermissionDecision {
                        allowed: false,
                        requires_confirmation: true,
                        decision: "allow_once_already_consumed".to_string(),
                        reason: "matching allow-once policy was consumed by another request"
                            .to_string(),
                        policy_id: Some(record.id),
                    })
                }
            }
        }
    }

    /// Peek at the permission decision without consuming `AllowOnce` policies.
    /// Used for replay pre-checks: the permission is not consumed until `check()` is called.
    pub fn peek(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
        capabilities: &[String],
    ) -> Result<ToolPermissionDecision> {
        let record = self.find_best(tool_name, source, risk_level, action_type)?;
        let Some(record) = record else {
            let asks = risk_level == "high"
                || capabilities.iter().any(|c| {
                    matches!(
                        c.as_str(),
                        "write" | "memory" | "lifemodel" | "filesystem" | "external_side_effect"
                    )
                });
            return Ok(ToolPermissionDecision {
                allowed: !asks,
                requires_confirmation: asks,
                decision: if asks { "ask_every_time" } else { "allow" }.to_string(),
                reason: if asks {
                    "no policy for high-risk/write action".to_string()
                } else {
                    "low-risk read action allowed by default".to_string()
                },
                policy_id: None,
            });
        };

        if record
            .expires_at
            .is_some_and(|expires| expires < Utc::now())
        {
            return Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "expired".to_string(),
                reason: "matching policy expired".to_string(),
                policy_id: Some(record.id),
            });
        }

        if record.policy == ToolPermissionPolicy::AllowOnce && record.consumed_at.is_some() {
            return Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "allow_once_already_consumed".to_string(),
                reason: "matching allow-once policy was consumed by another request".to_string(),
                policy_id: Some(record.id),
            });
        }

        match record.policy {
            ToolPermissionPolicy::Allow | ToolPermissionPolicy::AllowUntilRevoked => {
                Ok(ToolPermissionDecision {
                    allowed: true,
                    requires_confirmation: false,
                    decision: record.policy.to_string(),
                    reason: "matching allow policy".to_string(),
                    policy_id: Some(record.id),
                })
            }
            ToolPermissionPolicy::Deny => Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "deny".to_string(),
                reason: "matching deny policy".to_string(),
                policy_id: Some(record.id),
            }),
            ToolPermissionPolicy::AskEveryTime => Ok(ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "ask_every_time".to_string(),
                reason: "matching ask-every-time policy".to_string(),
                policy_id: Some(record.id),
            }),
            ToolPermissionPolicy::AllowOnce => Ok(ToolPermissionDecision {
                allowed: true,
                requires_confirmation: false,
                decision: "allow_once".to_string(),
                reason: "matching allow-once policy (not consumed yet)".to_string(),
                policy_id: Some(record.id),
            }),
        }
    }

    fn consume_if_available(&self, id: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let changed = conn.execute(
            "UPDATE tool_permissions
             SET consumed_at = ?2
             WHERE id = ?1 AND consumed_at IS NULL",
            params![id, Utc::now().to_rfc3339()],
        )?;
        Ok(changed == 1)
    }

    fn find_best(
        &self,
        tool_name: &str,
        source: &str,
        risk_level: &str,
        action_type: &str,
    ) -> Result<Option<ToolPermissionRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.query_row(
            "SELECT id, tool_name, source, risk_level, action_type, policy, created_at, expires_at, consumed_at
             FROM tool_permissions
             WHERE (tool_name = ?1 OR tool_name = '*')
               AND (source = ?2 OR source = '*')
               AND (risk_level = ?3 OR risk_level = '*')
               AND (action_type = ?4 OR action_type = '*')
             ORDER BY
               CASE WHEN tool_name = ?1 THEN 0 ELSE 1 END,
               CASE WHEN source = ?2 THEN 0 ELSE 1 END,
               CASE WHEN risk_level = ?3 THEN 0 ELSE 1 END,
               CASE WHEN action_type = ?4 THEN 0 ELSE 1 END,
               created_at DESC
             LIMIT 1",
            params![tool_name, source, risk_level, action_type],
            row_to_record,
        )
        .optional()
        .context("failed to query tool permission")
    }
}

fn row_to_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<ToolPermissionRecord> {
    let policy: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let expires_at: Option<String> = row.get(7)?;
    let consumed_at: Option<String> = row.get(8)?;
    Ok(ToolPermissionRecord {
        id: row.get(0)?,
        tool_name: row.get(1)?,
        source: row.get(2)?,
        risk_level: row.get(3)?,
        action_type: row.get(4)?,
        policy: policy.parse().map_err(|_| rusqlite::Error::InvalidQuery)?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .with_timezone(&Utc),
        expires_at: expires_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .map(|dt| dt.with_timezone(&Utc)),
        consumed_at: consumed_at
            .as_deref()
            .map(DateTime::parse_from_rfc3339)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?
            .map(|dt| dt.with_timezone(&Utc)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_list_revoke_permission() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let record = store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(store.revoke(&record.id).unwrap());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn high_risk_without_policy_requires_confirmation() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let decision = store
            .check(
                "write_file",
                "mcp",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!decision.allowed);
        assert!(decision.requires_confirmation);
    }

    #[test]
    fn allow_once_consumes_policy_and_second_check_asks() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        store
            .grant(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();
        // First check: allowed, policy consumed
        let first = store
            .check(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(first.allowed);
        assert_eq!(first.decision, "allow_once");
        // Second check: the consumed record remains authoritative, so execution
        // cannot fall through to a weaker default or an older broader grant.
        let second = store
            .check(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!second.allowed);
        assert!(second.requires_confirmation);
        assert_eq!(second.decision, "allow_once_already_consumed");
        assert!(second.policy_id.is_some());
    }

    #[test]
    fn consumed_low_risk_allow_once_never_falls_through_to_default_allow() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let record = store
            .grant(
                "builtin_echo",
                "builtin",
                "low",
                "read",
                ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();
        let first = store
            .check("builtin_echo", "builtin", "low", "read", &["read".into()])
            .unwrap();
        assert!(first.allowed);
        assert_eq!(first.policy_id.as_deref(), Some(record.id.as_str()));

        let second = store
            .check("builtin_echo", "builtin", "low", "read", &["read".into()])
            .unwrap();
        assert!(!second.allowed);
        assert!(second.requires_confirmation);
        assert_eq!(second.decision, "allow_once_already_consumed");
        assert_eq!(second.policy_id.as_deref(), Some(record.id.as_str()));

        let peek = store
            .peek("builtin_echo", "builtin", "low", "read", &["read".into()])
            .unwrap();
        assert!(!peek.allowed);
        assert_eq!(peek.decision, "allow_once_already_consumed");
    }

    #[test]
    fn action_bound_allow_once_is_consumed_only_by_the_exact_input() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let exact_input = serde_json::json!({"value":"exact"});
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&exact_input);
        let scope = ActionBoundToolPermissionScope {
            tool_name: "write_file".into(),
            source: "builtin".into(),
            risk_level: "high".into(),
            manifest_action_type: "write".into(),
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
            resolved_target: "write_file".into(),
            input_hash,
            input_length_bytes: input_length_bytes as u64,
        };
        let execution_binding = ActionBoundToolExecutionBinding {
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
        };
        store.grant_action_bound("proposal-exact", &scope).unwrap();
        let authorization = store
            .peek_action_bound("proposal-exact", &scope)
            .unwrap()
            .expect("exact grant available");

        let wrong = store
            .consume_action_bound(
                &authorization,
                &execution_binding,
                "write_file",
                "builtin",
                "high",
                "write",
                &serde_json::json!({"value":"different"}),
            )
            .unwrap();
        assert!(!wrong.allowed);
        assert_eq!(wrong.decision, "action_bound_scope_mismatch");
        assert!(store
            .peek_action_bound("proposal-exact", &scope)
            .unwrap()
            .is_some());

        let exact = store
            .consume_action_bound(
                &authorization,
                &execution_binding,
                "write_file",
                "builtin",
                "high",
                "write",
                &exact_input,
            )
            .unwrap();
        assert!(exact.allowed);
        assert_eq!(exact.decision, "action_bound_allow_once");
        let second = store
            .consume_action_bound(
                &authorization,
                &execution_binding,
                "write_file",
                "builtin",
                "high",
                "write",
                &exact_input,
            )
            .unwrap();
        assert!(!second.allowed);
        assert_eq!(second.decision, "action_bound_allow_once_already_consumed");
    }

    #[test]
    fn manifest_permission_lookup_cannot_consume_action_bound_grant() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let exact_input = serde_json::json!({"value":"exact"});
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&exact_input);
        let scope = ActionBoundToolPermissionScope {
            tool_name: "write_file".into(),
            source: "builtin".into(),
            risk_level: "high".into(),
            manifest_action_type: "write".into(),
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
            resolved_target: "write_file".into(),
            input_hash,
            input_length_bytes: input_length_bytes as u64,
        };
        store
            .grant_action_bound("proposal-isolated", &scope)
            .unwrap();

        let unrelated = store
            .check("write_file", "builtin", "high", "write", &["write".into()])
            .unwrap();
        assert!(!unrelated.allowed);
        assert!(store
            .peek_action_bound("proposal-isolated", &scope)
            .unwrap()
            .is_some());
    }

    #[test]
    fn action_bound_allow_once_compare_and_swap_has_one_concurrent_consumer() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let input = serde_json::json!({"value":"exact"});
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&input);
        let scope = ActionBoundToolPermissionScope {
            tool_name: "write_file".into(),
            source: "builtin".into(),
            risk_level: "high".into(),
            manifest_action_type: "write".into(),
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
            resolved_target: "write_file".into(),
            input_hash,
            input_length_bytes: input_length_bytes as u64,
        };
        let authorization = store
            .grant_action_bound("proposal-concurrent", &scope)
            .unwrap();
        let execution_binding = ActionBoundToolExecutionBinding {
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
        };
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let store = store.clone();
            let authorization = authorization.clone();
            let input = input.clone();
            let execution_binding = execution_binding.clone();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                store
                    .consume_action_bound(
                        &authorization,
                        &execution_binding,
                        "write_file",
                        "builtin",
                        "high",
                        "write",
                        &input,
                    )
                    .unwrap()
                    .allowed
            }));
        }
        let winners = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|allowed| *allowed)
            .count();
        assert_eq!(winners, 1);
    }

    #[test]
    fn action_bound_allow_once_rejects_queue_action_or_requested_target_drift() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let input = serde_json::json!({"value":"exact"});
        let (input_length_bytes, input_hash) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&input);
        let scope = ActionBoundToolPermissionScope {
            tool_name: "write_file".into(),
            source: "builtin".into(),
            risk_level: "high".into(),
            manifest_action_type: "write".into(),
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
            resolved_target: "write_file".into(),
            input_hash,
            input_length_bytes: input_length_bytes as u64,
        };
        let authorization = store
            .grant_action_bound("proposal-product-action", &scope)
            .unwrap();

        for binding in [
            ActionBoundToolExecutionBinding {
                queue_action_type: "memory.write".into(),
                requested_target: "write_file".into(),
            },
            ActionBoundToolExecutionBinding {
                queue_action_type: "file.write".into(),
                requested_target: "other.request".into(),
            },
        ] {
            let denied = store
                .consume_action_bound(
                    &authorization,
                    &binding,
                    "write_file",
                    "builtin",
                    "high",
                    "write",
                    &input,
                )
                .unwrap();
            assert!(!denied.allowed);
            assert_eq!(denied.decision, "action_bound_scope_mismatch");
        }
        assert!(store
            .peek_action_bound("proposal-product-action", &scope)
            .unwrap()
            .is_some());

        let exact = store
            .consume_action_bound(
                &authorization,
                &ActionBoundToolExecutionBinding {
                    queue_action_type: "file.write".into(),
                    requested_target: "write_file".into(),
                },
                "write_file",
                "builtin",
                "high",
                "write",
                &input,
            )
            .unwrap();
        assert!(exact.allowed);
    }

    #[test]
    fn action_bound_scope_rejects_noncanonical_sha256_labels() {
        let mut scope = ActionBoundToolPermissionScope {
            tool_name: "write_file".into(),
            source: "builtin".into(),
            risk_level: "high".into(),
            manifest_action_type: "write".into(),
            queue_action_type: "file.write".into(),
            requested_target: "write_file".into(),
            resolved_target: "write_file".into(),
            input_hash: "sha256:not-a-digest".into(),
            input_length_bytes: 1,
        };
        assert!(scope.validate().is_err());
        scope.input_hash = format!("sha256:{}", "A".repeat(64));
        assert!(scope.validate().is_err());
        scope.input_hash = format!("sha256:{}", "0".repeat(64));
        assert!(scope.validate().is_ok());
    }

    #[test]
    fn replay_uses_same_source_canonical_format_as_normal() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        // Grant with canonical source format "mcp:filesystem"
        store
            .grant(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        // Normal execution check with canonical format
        let normal = store
            .check(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(normal.allowed);
        assert_eq!(normal.decision, "allow_until_revoked");
        // Replay uses the same source format from tool_scope
        let replay = store
            .check(
                "write_file",
                "mcp:filesystem",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(replay.allowed);
        assert_eq!(replay.decision, "allow_until_revoked");
        // Mismatched source format should not match
        let mismatched = store
            .check(
                "write_file",
                "mcp",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!mismatched.allowed);
        assert!(mismatched.requires_confirmation);
    }

    #[test]
    fn source_canonical_format_builtin_vs_mcp() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        // Grant builtin tool with canonical format (high risk to test mismatch blocking)
        store
            .grant(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                ToolPermissionPolicy::AllowUntilRevoked,
                None,
            )
            .unwrap();
        // Builtin check passes
        let builtin_check = store
            .check(
                "write_file",
                "builtin",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(builtin_check.allowed);
        // MCP check with same tool name but different source does not match builtin grant
        // Since no policy matches for mcp:memory, high-risk + write capability requires confirmation
        let mcp_check = store
            .check(
                "write_file",
                "mcp:memory",
                "high",
                "mcp_tool_call",
                &["write".to_string()],
            )
            .unwrap();
        assert!(!mcp_check.allowed);
        assert!(mcp_check.requires_confirmation);
    }

    #[test]
    fn peek_allow_once_does_not_consume() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        store
            .grant(
                "dangerous_write",
                "mcp:filesystem",
                "high",
                "write",
                ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();

        // First peek should show allowed but not consume
        let peek1 = store
            .peek(
                "dangerous_write",
                "mcp:filesystem",
                "high",
                "write",
                &["write".into()],
            )
            .unwrap();
        assert!(peek1.allowed);
        assert_eq!(peek1.decision, "allow_once");

        // Second peek should still show allowed (not consumed)
        let peek2 = store
            .peek(
                "dangerous_write",
                "mcp:filesystem",
                "high",
                "write",
                &["write".into()],
            )
            .unwrap();
        assert!(peek2.allowed);
        assert_eq!(peek2.decision, "allow_once");

        // check() should consume the policy
        let check1 = store
            .check(
                "dangerous_write",
                "mcp:filesystem",
                "high",
                "write",
                &["write".into()],
            )
            .unwrap();
        assert!(check1.allowed);
        assert_eq!(check1.decision, "allow_once");

        // Second check should require confirmation (already consumed)
        let check2 = store
            .check(
                "dangerous_write",
                "mcp:filesystem",
                "high",
                "write",
                &["write".into()],
            )
            .unwrap();
        assert!(!check2.allowed);
        assert!(check2.requires_confirmation);
    }

    #[test]
    fn peek_and_check_other_policies_behave_same() {
        let store = ToolPermissionStore::new_in_memory().unwrap();

        // Allow policy
        store
            .grant(
                "read_file",
                "builtin",
                "low",
                "read",
                ToolPermissionPolicy::Allow,
                None,
            )
            .unwrap();
        let peek_allow = store
            .peek("read_file", "builtin", "low", "read", &["read".into()])
            .unwrap();
        let check_allow = store
            .check("read_file", "builtin", "low", "read", &["read".into()])
            .unwrap();
        assert_eq!(peek_allow.allowed, check_allow.allowed);
        assert_eq!(peek_allow.decision, check_allow.decision);

        // Deny policy
        store
            .grant(
                "delete_all",
                "builtin",
                "high",
                "write",
                ToolPermissionPolicy::Deny,
                None,
            )
            .unwrap();
        let peek_deny = store
            .peek("delete_all", "builtin", "high", "write", &["write".into()])
            .unwrap();
        let check_deny = store
            .check("delete_all", "builtin", "high", "write", &["write".into()])
            .unwrap();
        assert_eq!(peek_deny.allowed, check_deny.allowed);
        assert_eq!(peek_deny.decision, check_deny.decision);

        // AskEveryTime policy
        store
            .grant(
                "network_call",
                "builtin",
                "medium",
                "network",
                ToolPermissionPolicy::AskEveryTime,
                None,
            )
            .unwrap();
        let peek_ask = store
            .peek(
                "network_call",
                "builtin",
                "medium",
                "network",
                &["network".into()],
            )
            .unwrap();
        let check_ask = store
            .check(
                "network_call",
                "builtin",
                "medium",
                "network",
                &["network".into()],
            )
            .unwrap();
        assert_eq!(peek_ask.allowed, check_ask.allowed);
        assert_eq!(peek_ask.decision, check_ask.decision);
    }

    #[test]
    fn allow_once_compare_and_swap_allows_exactly_one_concurrent_consumer() {
        let path = std::env::temp_dir().join(format!(
            "openlife-tool-permission-cas-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let seed = ToolPermissionStore::new(&path).unwrap();
        seed.grant(
            "concurrent_write",
            "builtin",
            "high",
            "write",
            ToolPermissionPolicy::AllowOnce,
            None,
        )
        .unwrap();
        drop(seed);

        const CONTENDERS: usize = 100;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(CONTENDERS));
        let handles = (0..CONTENDERS)
            .map(|_| {
                let barrier = std::sync::Arc::clone(&barrier);
                let path = path.clone();
                std::thread::spawn(move || {
                    let store = ToolPermissionStore::new(path).unwrap();
                    barrier.wait();
                    store
                        .check(
                            "concurrent_write",
                            "builtin",
                            "high",
                            "write",
                            &["write".into()],
                        )
                        .unwrap()
                        .allowed
                })
            })
            .collect::<Vec<_>>();
        let allowed = handles
            .into_iter()
            .filter_map(|handle| handle.join().ok())
            .filter(|won| *won)
            .count();
        assert_eq!(allowed, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn arbitrary_allow_once_cannot_mint_provider_probe_authority() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let scheduler = store.bind_explicit_provider_probe_scheduler(
            crate::scheduler::InferenceScheduler::new(
                "unused-local".into(),
                false,
                "openai".into(),
                "https://api.openai.com/v1".into(),
                "sk-test".into(),
                "gpt-4o-mini".into(),
                "text-embedding-3-small".into(),
                false,
            ),
        );
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let original_policy = crate::config::NetworkPolicy {
            default_decision: "ask".into(),
            ..Default::default()
        };
        let original_decision = crate::network_client::resolve_network_policy_decision(
            &original_policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let endpoint_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "endpoint": endpoint.clone(),
            }))
            .1;
        let scope = format!(
            "provider.openai@{}#endpoint:{}",
            original_decision.decision_id, endpoint_digest
        );
        let arbitrary = store
            .grant(
                &scope,
                "provider",
                "high",
                "network",
                ToolPermissionPolicy::AllowOnce,
                None,
            )
            .unwrap();
        assert!(store
            .consume_reviewed_network_once(&scope, "provider", "high", "network")
            .unwrap()
            .is_none());
        let mut effective_policy = original_policy;
        effective_policy
            .tool_overrides
            .insert("provider.openai".into(), "allow".into());
        let effective_decision = crate::network_client::resolve_network_policy_decision(
            &effective_policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();

        let error = store
            .issue_explicit_provider_probe_grant(
                scheduler.explicit_provider_probe_challenge().unwrap(),
                effective_policy,
                &original_decision,
                effective_decision,
                None,
            )
            .unwrap_err();
        assert!(error.to_string().contains("direct_allow_missing"));
        assert!(!arbitrary.id.is_empty());
    }

    #[test]
    fn reviewed_web_network_permission_accepts_exact_medium_scope() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let endpoint_digest =
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let scope = format!("web.fetch@network-policy:web#endpoint:{endpoint_digest}");
        let proposal = crate::agent::AgentProposal::new(
            crate::agent::ProposalType::ToolPermission,
            "tool_permission.network_policy.web.fetch",
            serde_json::json!({
                "permission_scope_kind": "network_policy",
                "permission": "allow_once",
                "tool_name": scope,
                "source": "network_policy",
                "risk_level": "medium",
                "action_type": "network",
                "canonical_scope": {
                    "tool_name": scope,
                    "source": "network_policy",
                    "risk_level": "medium",
                    "action_type": "network",
                    "network_policy_decision_id": "network-policy:web",
                    "endpoint_digest": endpoint_digest,
                },
            }),
            "review exact web endpoint",
            1.0,
            crate::agent::RiskLevel::Medium,
            crate::agent::ProposalSource::ChatConversation,
        );
        proposal_store.create_proposal(&proposal).unwrap();
        let claim_id = proposal_store
            .claim_dispatch(&proposal.id)
            .unwrap()
            .unwrap();
        let acceptance = crate::agent::ReviewWorkflow::new(&proposal_store)
            .claimed_acceptance_snapshot(&proposal.id, &claim_id)
            .unwrap();

        store
            .grant_reviewed_network_once(&acceptance, &scope, "network_policy", "medium", "network")
            .unwrap();
        assert!(store
            .reviewed_network_once_available_for_proposal(
                &proposal.id,
                &scope,
                "network_policy",
                "medium",
                "network",
            )
            .unwrap());
        let consumed = store
            .consume_reviewed_network_once_for_proposal(
                &proposal.id,
                &scope,
                "network_policy",
                "medium",
                "network",
            )
            .unwrap()
            .expect("exact reviewed web network permission remains consumable");
        assert_eq!(consumed.proposal_id, proposal.id);
    }

    #[test]
    fn claimed_review_acceptance_and_consumed_allow_once_can_mint_exact_probe() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let scheduler = store.bind_explicit_provider_probe_scheduler(
            crate::scheduler::InferenceScheduler::new(
                "unused-local".into(),
                false,
                "openai".into(),
                "https://api.openai.com/v1".into(),
                "sk-test".into(),
                "gpt-4o-mini".into(),
                "text-embedding-3-small".into(),
                false,
            ),
        );
        let endpoint = crate::llm::chat_completions_url("openai", &scheduler.openai_base);
        let original_policy = crate::config::NetworkPolicy {
            default_decision: "ask".into(),
            ..Default::default()
        };
        let original_decision = crate::network_client::resolve_network_policy_decision(
            &original_policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let endpoint_digest =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "endpoint": endpoint.clone(),
            }))
            .1;
        let scope = format!(
            "provider.openai@{}#endpoint:{}",
            original_decision.decision_id, endpoint_digest
        );
        let proposal = crate::agent::AgentProposal::new(
            crate::agent::ProposalType::ToolPermission,
            "tool_permission.provider.openai",
            serde_json::json!({
                "permission_scope_kind": "network_policy",
                "permission": "allow_once",
                "tool_name": scope.clone(),
                "source": "provider",
                "risk_level": "high",
                "action_type": "network",
                "canonical_scope": {
                    "tool_name": scope.clone(),
                    "source": "provider",
                    "risk_level": "high",
                    "action_type": "network",
                    "network_policy_decision_id": original_decision.decision_id.clone(),
                    "endpoint_digest": endpoint_digest.clone(),
                },
            }),
            "review exact provider dispatch",
            1.0,
            crate::agent::RiskLevel::High,
            crate::agent::ProposalSource::NetworkConsent,
        );
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        proposal_store.create_proposal(&proposal).unwrap();
        let claim_id = proposal_store
            .claim_dispatch(&proposal.id)
            .unwrap()
            .unwrap();
        let acceptance = crate::agent::ReviewWorkflow::new(&proposal_store)
            .claimed_acceptance_snapshot(&proposal.id, &claim_id)
            .unwrap();
        let permission = store
            .grant_reviewed_network_once(&acceptance, &scope, "provider", "high", "network")
            .unwrap();
        let reviewed_permission = store
            .consume_reviewed_network_once(&scope, "provider", "high", "network")
            .unwrap()
            .expect("reviewed AllowOnce must produce one opaque consumed proof");
        let mut effective_policy = original_policy;
        effective_policy
            .tool_overrides
            .insert("provider.openai".into(), "allow".into());
        let effective_decision = crate::network_client::resolve_network_policy_decision(
            &effective_policy,
            &endpoint,
            "provider.openai",
        )
        .unwrap();
        let grant = store
            .issue_explicit_provider_probe_grant(
                scheduler.explicit_provider_probe_challenge().unwrap(),
                effective_policy,
                &original_decision,
                effective_decision,
                Some(reviewed_permission),
            )
            .unwrap();

        assert!(!permission.id.is_empty());
        scheduler.prepare_explicit_provider_probe(grant).unwrap();
    }

    #[test]
    fn provider_continuation_consumes_only_its_exact_reviewed_proposal() {
        let store = ToolPermissionStore::new_in_memory().unwrap();
        let proposal_store = crate::agent::ProposalStore::new_in_memory().unwrap();
        let scope = "provider.openai@decision#endpoint:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let create_reviewed = |suffix: &str| {
            let affected_path = format!("tool_permission.provider.openai.{suffix}");
            let proposal = crate::agent::AgentProposal::new(
                crate::agent::ProposalType::ToolPermission,
                &affected_path,
                serde_json::json!({
                    "permission_scope_kind": "network_policy",
                    "permission": "allow_once",
                    "tool_name": scope,
                    "source": "provider",
                    "risk_level": "high",
                    "action_type": "network",
                    "canonical_scope": {
                        "tool_name": scope,
                        "source": "provider",
                        "risk_level": "high",
                        "action_type": "network",
                        "network_policy_decision_id": "decision",
                        "endpoint_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    },
                }),
                "review exact provider dispatch",
                1.0,
                crate::agent::RiskLevel::High,
                crate::agent::ProposalSource::NetworkConsent,
            );
            proposal_store.create_proposal(&proposal).unwrap();
            let claim_id = proposal_store
                .claim_dispatch(&proposal.id)
                .unwrap()
                .unwrap();
            let acceptance = crate::agent::ReviewWorkflow::new(&proposal_store)
                .claimed_acceptance_snapshot(&proposal.id, &claim_id)
                .unwrap();
            store
                .grant_reviewed_network_once(&acceptance, scope, "provider", "high", "network")
                .unwrap();
            proposal.id
        };
        let first = create_reviewed("first");
        let second = create_reviewed("second");

        assert!(store
            .reviewed_network_once_available_for_proposal(
                &first, scope, "provider", "high", "network",
            )
            .unwrap());

        let consumed_first = store
            .consume_reviewed_network_once_for_proposal(
                &first, "provider.openai@decision#endpoint:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "provider", "high", "network",
            )
            .unwrap()
            .expect("the exact first Proposal grant must remain independently consumable");
        assert_eq!(consumed_first.proposal_id, first);
        assert!(!store
            .reviewed_network_once_available_for_proposal(
                &first, scope, "provider", "high", "network",
            )
            .unwrap());
        assert!(store
            .consume_reviewed_network_once_for_proposal(
                &first, scope, "provider", "high", "network",
            )
            .unwrap()
            .is_none());
        let consumed_second = store
            .consume_reviewed_network_once_for_proposal(
                &second, scope, "provider", "high", "network",
            )
            .unwrap()
            .expect("consuming the first Proposal must not consume the second");
        assert_eq!(consumed_second.proposal_id, second);
    }
}
