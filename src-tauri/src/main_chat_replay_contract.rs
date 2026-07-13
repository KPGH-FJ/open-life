use openlife_core::tool_manifest::ToolManifest;

pub(crate) const MAIN_CHAT_REPLAY_EXECUTION_ENVELOPE_VERSION: u32 = 2;

/// Durable, minimal execution identity used to decide whether a queued action
/// can be replayed. It contains no arguments or response body: only exact
/// identities and a digest/length binding owned by the original execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DurableMainChatReplayExecutionEnvelope {
    pub(crate) version: u32,
    pub(crate) task_session_id: String,
    pub(crate) run_id: String,
    pub(crate) queue_action_id: String,
    pub(crate) executor_action_id: String,
    pub(crate) queue_action_type: String,
    pub(crate) executor_action_type: String,
    pub(crate) requested_target: String,
    pub(crate) resolved_target: String,
    pub(crate) manifest_id: String,
    pub(crate) manifest_name: String,
    pub(crate) manifest_source: String,
    pub(crate) manifest_contract_digest: String,
    pub(crate) action_effect: openlife_core::tool_execution_receipt::ToolActionEffect,
    pub(crate) idempotency_contract: openlife_core::tool_manifest::ToolIdempotencyContract,
    pub(crate) input_hash: String,
    pub(crate) input_length_bytes: u64,
}

pub(crate) struct DurableMainChatReplayExecutionInput<'a> {
    pub(crate) task_session_id: &'a str,
    pub(crate) run_id: &'a str,
    pub(crate) queue_action_id: &'a str,
    pub(crate) executor_action_id: &'a str,
    pub(crate) queue_action_type: &'a str,
    pub(crate) executor_action_type: &'a str,
    pub(crate) requested_target: &'a str,
    pub(crate) resolved_target: &'a str,
    pub(crate) manifest: &'a ToolManifest,
    pub(crate) input: &'a serde_json::Value,
}

impl DurableMainChatReplayExecutionEnvelope {
    pub(crate) fn from_canonical_authority(
        authority: &openlife_core::agent::main_chat_agent_v1::CanonicalToolReplayAuthority,
    ) -> Self {
        Self {
            version: MAIN_CHAT_REPLAY_EXECUTION_ENVELOPE_VERSION,
            task_session_id: authority.task_session_id().to_string(),
            run_id: authority.run_id().to_string(),
            queue_action_id: authority.action_id().to_string(),
            executor_action_id: authority.executor_action_id().to_string(),
            queue_action_type: authority.queue_action_type().to_string(),
            executor_action_type: authority.executor_action_type().to_string(),
            requested_target: authority.requested_target().to_string(),
            resolved_target: authority.resolved_target().to_string(),
            manifest_id: authority.manifest_id().to_string(),
            manifest_name: authority.manifest_name().to_string(),
            manifest_source: authority.manifest_source().to_string(),
            manifest_contract_digest: authority.manifest_contract_digest().to_string(),
            action_effect: authority.action_effect(),
            idempotency_contract: authority.idempotency_contract(),
            input_hash: authority.input_hash().to_string(),
            input_length_bytes: authority.input_length_bytes(),
        }
    }

    pub(crate) fn new(input: DurableMainChatReplayExecutionInput<'_>) -> Result<Self, String> {
        let (input_length_bytes, input_hash) =
            openlife_core::agent::metadata_safe::metadata_safe_value_digest(input.input);
        let contract = openlife_core::agent::validate_manifest_execution_contract(input.manifest)?;
        let envelope = Self {
            version: MAIN_CHAT_REPLAY_EXECUTION_ENVELOPE_VERSION,
            task_session_id: input.task_session_id.to_string(),
            run_id: input.run_id.to_string(),
            queue_action_id: input.queue_action_id.to_string(),
            executor_action_id: input.executor_action_id.to_string(),
            queue_action_type: input.queue_action_type.to_string(),
            executor_action_type: input.executor_action_type.to_string(),
            requested_target: input.requested_target.to_string(),
            resolved_target: input.resolved_target.to_string(),
            manifest_id: input.manifest.id.clone(),
            manifest_name: input.manifest.name.clone(),
            manifest_source: input.manifest.source.to_string(),
            manifest_contract_digest: input.manifest.execution_contract_digest(),
            action_effect: contract.action_effect,
            idempotency_contract: contract.idempotency_contract,
            input_hash,
            input_length_bytes: input_length_bytes as u64,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub(crate) fn from_action_metadata(metadata: &serde_json::Value) -> Result<Self, String> {
        let value = metadata
            .get("replayExecutionEnvelope")
            .ok_or_else(|| "retry_replay_execution_envelope_missing".to_string())?;
        let envelope = serde_json::from_value::<Self>(value.clone())
            .map_err(|_| "retry_replay_execution_envelope_invalid".to_string())?;
        envelope.validate()?;
        Ok(envelope)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.version != MAIN_CHAT_REPLAY_EXECUTION_ENVELOPE_VERSION {
            return Err("retry_replay_execution_envelope_version_unsupported".into());
        }
        for (field, value) in [
            ("task_session_id", self.task_session_id.as_str()),
            ("run_id", self.run_id.as_str()),
            ("queue_action_id", self.queue_action_id.as_str()),
            ("executor_action_id", self.executor_action_id.as_str()),
            ("queue_action_type", self.queue_action_type.as_str()),
            ("executor_action_type", self.executor_action_type.as_str()),
            ("requested_target", self.requested_target.as_str()),
            ("resolved_target", self.resolved_target.as_str()),
            ("manifest_id", self.manifest_id.as_str()),
            ("manifest_name", self.manifest_name.as_str()),
            ("manifest_source", self.manifest_source.as_str()),
            (
                "manifest_contract_digest",
                self.manifest_contract_digest.as_str(),
            ),
            ("input_hash", self.input_hash.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("retry_replay_execution_envelope_missing_{field}"));
            }
        }
        for digest in [&self.manifest_contract_digest, &self.input_hash] {
            let Some(hex) = digest.strip_prefix("sha256:") else {
                return Err("retry_replay_execution_envelope_digest_invalid".into());
            };
            if hex.len() != 64
                || !hex
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err("retry_replay_execution_envelope_digest_invalid".into());
            }
        }
        if self.action_effect == openlife_core::tool_execution_receipt::ToolActionEffect::Unknown
            || self.idempotency_contract
                == openlife_core::tool_manifest::ToolIdempotencyContract::Unspecified
        {
            return Err("retry_replay_execution_envelope_effect_contract_invalid".into());
        }
        Ok(())
    }

    pub(crate) fn matches_current_execution(
        &self,
        task_session_id: &str,
        run_id: &str,
        queue_action_id: &str,
        queue_action_type: &str,
        executor_action_type: &str,
        requested_target: &str,
        resolved_target: &str,
        manifest: &ToolManifest,
        input: &serde_json::Value,
    ) -> bool {
        let (input_length_bytes, input_hash) =
            openlife_core::agent::metadata_safe::metadata_safe_value_digest(input);
        let Ok(contract) = openlife_core::agent::validate_manifest_execution_contract(manifest)
        else {
            return false;
        };
        self.task_session_id == task_session_id
            && self.run_id == run_id
            && self.queue_action_id == queue_action_id
            && self.queue_action_type == queue_action_type
            && self.executor_action_type == executor_action_type
            && self.requested_target == requested_target
            && self.resolved_target == resolved_target
            && self.manifest_id == manifest.id
            && self.manifest_name == manifest.name
            && self.manifest_source == manifest.source.to_string()
            && self.manifest_contract_digest == manifest.execution_contract_digest()
            && self.action_effect == contract.action_effect
            && self.idempotency_contract == contract.idempotency_contract
            && self.input_hash == input_hash
            && self.input_length_bytes == input_length_bytes as u64
    }

    pub(crate) fn attach_to_metadata(
        &self,
        metadata: &mut serde_json::Value,
    ) -> Result<(), String> {
        let object = metadata
            .as_object_mut()
            .ok_or_else(|| "retry_replay_observation_metadata_not_object".to_string())?;
        object.insert(
            "replayExecutionEnvelope".into(),
            serde_json::to_value(self)
                .map_err(|_| "retry_replay_execution_envelope_serialize_failed".to_string())?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::tool_manifest::{ToolIdempotencyContract, ToolSource};

    fn manifest() -> ToolManifest {
        let mut manifest = ToolManifest::new(
            "builtin_echo",
            "echo",
            serde_json::json!({"type":"object"}),
            "low",
            "1",
            ToolSource::BuiltIn,
        )
        .with_capabilities(vec!["read".into()])
        .with_idempotency_contract(ToolIdempotencyContract::Idempotent);
        manifest.action_type = "read".into();
        manifest
    }

    #[test]
    fn durable_envelope_rejects_target_or_manifest_drift() {
        let original_manifest = manifest();
        let input = serde_json::json!({"value":"exact"});
        let envelope =
            DurableMainChatReplayExecutionEnvelope::new(DurableMainChatReplayExecutionInput {
                task_session_id: "task-1",
                run_id: "run-1",
                queue_action_id: "action-1",
                executor_action_id: "executor-1",
                queue_action_type: "mcp.read_only",
                executor_action_type: "mcp_tool",
                requested_target: "mcp.call_tool",
                resolved_target: "builtin_echo",
                manifest: &original_manifest,
                input: &input,
            })
            .unwrap();
        assert!(envelope.matches_current_execution(
            "task-1",
            "run-1",
            "action-1",
            "mcp.read_only",
            "mcp_tool",
            "mcp.call_tool",
            "builtin_echo",
            &original_manifest,
            &input,
        ));
        assert!(!envelope.matches_current_execution(
            "task-1",
            "run-1",
            "action-1",
            "mcp.read_only",
            "mcp_tool",
            "different.request",
            "builtin_echo",
            &original_manifest,
            &input,
        ));
        let mut changed_manifest = manifest();
        changed_manifest.action_type = "write".into();
        assert!(!envelope.matches_current_execution(
            "task-1",
            "run-1",
            "action-1",
            "mcp.read_only",
            "mcp_tool",
            "mcp.call_tool",
            "builtin_echo",
            &changed_manifest,
            &input,
        ));
    }
}
