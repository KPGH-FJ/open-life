use crate::mcp::McpArgumentInspection;
use crate::tool_manifest::ToolManifest;
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use serde_json::Value;

use super::helpers::{
    canonical_tool_source, configured_web_search_endpoint, filesystem_access_error,
    is_direct_external_write_tool, is_path_lexically_in_safe_paths, is_proposal_generation_tool,
    normalize_tool_name, should_mark_needs_confirmation, ToolCallInternalResult,
};
use super::ActionExecutionContext;
use super::ActionExecutionResult;
use super::ActionExecutionStatus;
use super::AgentActionRequest;
use crate::agent::metadata_safe::{metadata_safe_text_preview, metadata_safe_value_digest};
use crate::agent::types::{
    AgentAction, AgentObservation, BoundContentField, ContentReceiptBinding, ContentReceiptKind,
    ToolActionScope, ToolActionTraceEnvelope,
};
use crate::network_client::{
    resolve_network_policy_decision, NetworkPolicyDecision, NetworkPolicyDisposition,
};
use crate::tool_execution_receipt::ToolExecutionReceiptTracker;

struct NetworkPolicyBlockedInput<'a> {
    request: &'a AgentActionRequest,
    tool_name: &'a str,
    args: &'a Value,
    manifest: Option<&'a ToolManifest>,
    inspection: &'a McpArgumentInspection,
    decision: &'a NetworkPolicyDecision,
    execution_receipt: crate::tool_execution_receipt::ToolExecutionReceipt,
}

struct PendingNetworkAuthorization {
    permission_scope: String,
}

/// One adapter observation that may be consumed exactly once by the narrow
/// receipt issuer. Construction is private to this module, the value is not
/// Clone/serde, and consuming it moves the raw body out of ToolExecutor.
pub(crate) struct ObservedToolBodyAdmission {
    issuance_id: String,
    kind: ContentReceiptKind,
    body: String,
    owner_anchor_digest: String,
}

impl std::fmt::Debug for ObservedToolBodyAdmission {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ObservedToolBodyAdmission")
            .field("kind", &self.kind)
            .field("byte_count", &self.body.len())
            .field("body", &"[REDACTED]")
            .finish()
    }
}

impl ObservedToolBodyAdmission {
    fn from_adapter_observation(
        kind: ContentReceiptKind,
        body: &str,
        binding: &ContentReceiptBinding,
    ) -> Result<Self> {
        if body.len() > crate::agent::types::MAX_OBSERVED_CONTENT_RECEIPT_BYTES {
            return Err(
                crate::agent::types::ContentReceiptIssuanceError::ObservedBodyTooLarge {
                    observed_bytes: body.len(),
                    max_bytes: crate::agent::types::MAX_OBSERVED_CONTENT_RECEIPT_BYTES,
                }
                .into(),
            );
        }
        if binding.field() != BoundContentField::for_kind(kind) {
            return Err(crate::agent::types::ContentReceiptIssuanceError::FieldKindMismatch.into());
        }
        Ok(Self {
            issuance_id: uuid::Uuid::new_v4().to_string(),
            kind,
            body: body.to_string(),
            owner_anchor_digest: binding.owner_anchor_digest(),
        })
    }

    pub(crate) fn into_issue_evidence(self) -> ObservedToolBodyIssueEvidence {
        ObservedToolBodyIssueEvidence {
            issuance_id: self.issuance_id,
            kind: self.kind,
            body: self.body,
            owner_anchor_digest: self.owner_anchor_digest,
        }
    }
}

/// Store-facing evidence obtainable only by consuming an admission minted by
/// the ToolExecutor adapter boundary.
pub(crate) struct ObservedToolBodyIssueEvidence {
    issuance_id: String,
    kind: ContentReceiptKind,
    body: String,
    owner_anchor_digest: String,
}

impl ObservedToolBodyIssueEvidence {
    pub(crate) fn issuance_id(&self) -> &str {
        &self.issuance_id
    }

    pub(crate) fn kind(&self) -> ContentReceiptKind {
        self.kind
    }

    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    pub(crate) fn owner_anchor_digest(&self) -> &str {
        &self.owner_anchor_digest
    }
}

fn mint_observed_body_admission(
    observed_body: Option<(&str, ContentReceiptKind)>,
    request: &AgentActionRequest,
    action: &AgentAction,
    observation: &AgentObservation,
    require_run_identity: bool,
) -> Result<Option<ObservedToolBodyAdmission>> {
    let Some((body, kind)) = observed_body else {
        return Ok(None);
    };
    let Some(run_id) = request.source_run_id.as_deref() else {
        if require_run_identity {
            anyhow::bail!("internal_read_canonical_run_identity_missing");
        }
        return Ok(None);
    };
    let field = match kind {
        ContentReceiptKind::ToolOutput => BoundContentField::ActionOutputObservationContent,
        ContentReceiptKind::ToolError => BoundContentField::ActionErrorObservationContent,
    };
    let binding = ContentReceiptBinding::from_action_graph(run_id, action, observation, field)?;
    Ok(Some(ObservedToolBodyAdmission::from_adapter_observation(
        kind, body, &binding,
    )?))
}

impl super::ActionExecutor {
    /// Execute a tool action (MCP, builtin, or plugin).
    pub(crate) async fn execute_tool(
        &self,
        request: AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        receipt_tracker: ToolExecutionReceiptTracker,
    ) -> Result<ActionExecutionResult> {
        let normalized_target = normalize_tool_name(&request.target, ctx.registry);
        let tool_name = &normalized_target;
        let args = request
            .input
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| request.input.clone());

        // 1. Lookup manifest
        let manifest = ctx
            .registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == *tool_name || m.id == *tool_name);

        // 2. Inspect PII
        let inspection = ctx.registry.inspect_call_arguments(tool_name, &args);

        if manifest.is_none() {
            let forced_decision = ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "blocked".into(),
                reason: "tool manifest not found for governed execution".into(),
                policy_id: None,
            };
            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &forced_decision,
                None,
                &request,
            );
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::Blocked,
                stop_reason: Some("tool_manifest_not_found".into()),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission: None,
            });
        }

        if !self.config.allow_cloud
            && manifest.as_ref().is_some_and(|manifest| {
                manifest.action_type == "network"
                    || manifest
                        .capabilities
                        .iter()
                        .any(|capability| capability == "network")
            })
        {
            let forced_decision = ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "blocked".into(),
                reason: "network-capable tool blocked because allow_cloud=false".into(),
                policy_id: None,
            };
            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &forced_decision,
                manifest.as_ref(),
                &request,
            );
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::Blocked,
                stop_reason: Some("allow_cloud_false".into()),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission: None,
            });
        }

        // Network policy is an execution authorization, not a post-dispatch
        // transport error. Ordinary public Web defaults to allow; an explicit
        // `ask` policy pauses this attempt without minting a parallel Proposal.
        let mut authorized_network_policy = None;
        let mut pending_network_authorization = None;
        if matches!(tool_name.as_str(), "web.fetch" | "web.search") {
            let endpoint = if tool_name == "web.fetch" {
                args.get("url")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .ok_or_else(|| anyhow::anyhow!("Missing 'url' argument for web.fetch"))?
            } else {
                match configured_web_search_endpoint(&self.config.search_provider) {
                    Ok(endpoint) => endpoint,
                    Err(reason) => {
                        let forced_decision = ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: false,
                            decision: "blocked".into(),
                            reason: reason.into(),
                            policy_id: None,
                        };
                        let (action, observation) = self.build_blocked_action_observation(
                            tool_name,
                            &args,
                            &inspection,
                            &forced_decision,
                            manifest.as_ref(),
                            &request,
                        );
                        return Ok(ActionExecutionResult {
                            action,
                            observation,
                            status: ActionExecutionStatus::Blocked,
                            stop_reason: Some(reason.into()),
                            execution_receipt: receipt_tracker.snapshot(),
                            observed_body_admission: None,
                        });
                    }
                }
            };
            if let Some(policy) = ctx.network_policy {
                let network_decision = resolve_network_policy_decision(
                    policy, &endpoint, tool_name,
                )
                .map_err(|error| anyhow::anyhow!("network_policy_evaluation_failed: {error:#}"))?;
                match network_decision.disposition {
                    NetworkPolicyDisposition::Allow => {}
                    NetworkPolicyDisposition::Deny => {
                        return Ok(self.build_network_policy_blocked_result(
                            NetworkPolicyBlockedInput {
                                request: &request,
                                tool_name,
                                args: &args,
                                manifest: manifest.as_ref(),
                                inspection: &inspection,
                                decision: &network_decision,
                                execution_receipt: receipt_tracker.snapshot(),
                            },
                        ));
                    }
                    NetworkPolicyDisposition::Ask => {
                        let permission_scope =
                            network_permission_scope(&network_decision, &request, &args);
                        // Peek here so a one-shot grant is not consumed by a
                        // later generic permission/argument/path blocker. The
                        // CAS consumption happens immediately before dispatch.
                        let permission = ctx
                            .permission_store
                            .peek(
                                &permission_scope,
                                "network_policy",
                                "medium",
                                "network",
                                &["network".into(), "external_side_effect".into()],
                            )
                            .unwrap_or(ToolPermissionDecision {
                                allowed: false,
                                requires_confirmation: true,
                                decision: "ask_every_time".into(),
                                reason: "network consent lookup failed closed".into(),
                                policy_id: None,
                            });

                        if !permission.allowed {
                            if permission.decision == "deny" {
                                let mut denied = network_decision.clone();
                                denied.reason_code = "network_policy_permission_denied".into();
                                return Ok(self.build_network_policy_blocked_result(
                                    NetworkPolicyBlockedInput {
                                        request: &request,
                                        tool_name,
                                        args: &args,
                                        manifest: manifest.as_ref(),
                                        inspection: &inspection,
                                        decision: &denied,
                                        execution_receipt: receipt_tracker.snapshot(),
                                    },
                                ));
                            }
                            let forced_decision = ToolPermissionDecision {
                                allowed: false,
                                requires_confirmation: true,
                                decision: "network_policy_consent_required".into(),
                                reason: format!(
                                    "{}; durable consent store unavailable",
                                    network_decision.reason_code
                                ),
                                policy_id: None,
                            };
                            let (action, mut observation) = self.build_blocked_action_observation(
                                tool_name,
                                &args,
                                &inspection,
                                &forced_decision,
                                manifest.as_ref(),
                                &request,
                            );
                            if let Some(object) = observation
                                .structured_result
                                .as_mut()
                                .and_then(Value::as_object_mut)
                            {
                                object.insert(
                                    "networkPolicyDecisionId".into(),
                                    serde_json::json!(network_decision.decision_id),
                                );
                                object.insert(
                                    "networkPermissionScope".into(),
                                    serde_json::json!(permission_scope),
                                );
                                object.insert(
                                    "networkHost".into(),
                                    serde_json::json!(network_decision.host),
                                );
                                object.insert(
                                    "networkCapability".into(),
                                    serde_json::json!(network_decision.capability),
                                );
                                object.insert(
                                    "networkPolicyReasonCode".into(),
                                    serde_json::json!(network_decision.reason_code),
                                );
                                object.insert("directWritesExecuted".into(), Value::Bool(false));
                            }
                            return Ok(ActionExecutionResult {
                                action,
                                observation,
                                status: ActionExecutionStatus::NeedsConfirmation,
                                stop_reason: Some("network_policy_consent_required".into()),
                                execution_receipt: receipt_tracker.snapshot(),
                                observed_body_admission: None,
                            });
                        }
                        authorized_network_policy = Some(network_policy_after_exact_consent(
                            policy,
                            &network_decision,
                        ));
                        pending_network_authorization =
                            Some(PendingNetworkAuthorization { permission_scope });
                    }
                }
            }
        }

        // 3. Check permission with canonical decision order:
        //    unknown -> blocked
        //    disabled/declarative-only -> blocked
        //    explicit deny -> blocked
        //    allow_once -> execute (consume in step 5)
        //    allow_until_revoked -> execute
        //    high-risk without allow -> needs_confirmation
        //    low-risk read -> execute
        let decision = if let Some(ref manifest) = manifest {
            if !manifest.enabled || manifest.declarative_only {
                ToolPermissionDecision {
                    allowed: false,
                    requires_confirmation: false,
                    decision: "deny".into(),
                    reason: if manifest.declarative_only {
                        "tool is declarative-only (no executor available)"
                    } else {
                        "tool is disabled"
                    }
                    .into(),
                    policy_id: None,
                }
            } else {
                let source = canonical_tool_source(manifest);

                let perm_check = if let Some(authorization) = ctx.action_bound_tool_permission {
                    let derived_execution_binding =
                        crate::tool_permissions::ActionBoundToolExecutionBinding {
                            queue_action_type: request.action_type.clone(),
                            requested_target: request.target.clone(),
                        };
                    let execution_binding = authorization
                        .execution_binding
                        .as_ref()
                        .unwrap_or(&derived_execution_binding);
                    if self.config.consume_allow_once {
                        ctx.permission_store.consume_action_bound(
                            authorization,
                            execution_binding,
                            &manifest.name,
                            &source,
                            &manifest.risk_level,
                            &manifest.action_type,
                            &args,
                        )
                    } else if authorization.scope.matches_execution(
                        execution_binding,
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        &manifest.action_type,
                        &args,
                    ) {
                        Ok(ToolPermissionDecision {
                            allowed: true,
                            requires_confirmation: false,
                            decision: "action_bound_allow_once_peek".into(),
                            reason: "exact action-bound ToolPermission is available".into(),
                            policy_id: Some(authorization.permission_id.clone()),
                        })
                    } else {
                        Ok(ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: true,
                            decision: "action_bound_scope_mismatch".into(),
                            reason: "action-bound ToolPermission does not match execution".into(),
                            policy_id: Some(authorization.permission_id.clone()),
                        })
                    }
                } else if self.config.consume_allow_once {
                    ctx.permission_store.check(
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        &manifest.action_type,
                        &manifest.capabilities,
                    )
                } else {
                    ctx.permission_store.peek(
                        &manifest.name,
                        &source,
                        &manifest.risk_level,
                        &manifest.action_type,
                        &manifest.capabilities,
                    )
                };

                perm_check.unwrap_or(ToolPermissionDecision {
                    allowed: false,
                    requires_confirmation: true,
                    decision: "ask_every_time".into(),
                    reason: "permission check failed".into(),
                    policy_id: None,
                })
            }
        } else {
            // No manifest found
            ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "deny".into(),
                reason: "tool is not registered or disabled".into(),
                policy_id: None,
            }
        };

        if manifest.as_ref().is_some_and(|manifest| {
            manifest.declarative_only
                || is_proposal_generation_tool(&manifest.name)
                || is_direct_external_write_tool(manifest)
        }) {
            let forced_decision = ToolPermissionDecision {
                allowed: false,
                requires_confirmation: false,
                decision: "blocked".into(),
                reason: "canonical ToolGateway accepts read-only executable tools only".into(),
                policy_id: None,
            };
            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &forced_decision,
                manifest.as_ref(),
                &request,
            );
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::Blocked,
                stop_reason: Some("tool_write_not_supported".into()),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission: None,
            });
        }

        // 4. Determine if the exact read-only execution is blocked.
        let permission_blocks = decision.requires_confirmation || !decision.allowed;
        let inspection_blocks = inspection.requires_confirmation && inspection.pii_found;
        let blocked = manifest
            .as_ref()
            .is_none_or(|m| !m.enabled || m.declarative_only)
            || inspection_blocks
            || permission_blocks;

        if blocked {
            let needs_confirmation = should_mark_needs_confirmation(&decision, &inspection);

            let (action, observation) = self.build_blocked_action_observation(
                tool_name,
                &args,
                &inspection,
                &decision,
                manifest.as_ref(),
                &request,
            );
            let status = if needs_confirmation {
                ActionExecutionStatus::NeedsConfirmation
            } else {
                ActionExecutionStatus::Blocked
            };
            return Ok(ActionExecutionResult {
                action,
                observation,
                status,
                stop_reason: Some("blocked_by_policy".into()),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission: None,
            });
        }

        // 5. Safe Paths check for filesystem tools
        if let Some(ref m) = manifest {
            if m.capabilities.contains(&"filesystem".to_string()) {
                let path = args
                    .get("path")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("");
                if !is_path_lexically_in_safe_paths(path, ctx.safe_paths) {
                    let (action, observation) = self.build_blocked_action_observation(
                        tool_name,
                        &args,
                        &inspection,
                        &ToolPermissionDecision {
                            allowed: false,
                            requires_confirmation: false,
                            decision: "blocked".into(),
                            reason: filesystem_access_error(path, ctx.safe_paths),
                            policy_id: None,
                        },
                        manifest.as_ref(),
                        &request,
                    );
                    return Ok(ActionExecutionResult {
                        action,
                        observation,
                        status: ActionExecutionStatus::Blocked,
                        stop_reason: Some("path_not_in_safe_paths".into()),
                        execution_receipt: receipt_tracker.snapshot(),
                        observed_body_admission: None,
                    });
                }
            }
        }

        if self.config.consume_allow_once {
            if let Some(pending) = pending_network_authorization.as_ref() {
                let permission = ctx
                    .permission_store
                    .check(
                        &pending.permission_scope,
                        "network_policy",
                        "medium",
                        "network",
                        &["network".into(), "external_side_effect".into()],
                    )
                    .unwrap_or(ToolPermissionDecision {
                        allowed: false,
                        requires_confirmation: true,
                        decision: "ask_every_time".into(),
                        reason: "network consent consumption failed closed".into(),
                        policy_id: None,
                    });
                if !permission.allowed {
                    let forced_decision = ToolPermissionDecision {
                        allowed: false,
                        requires_confirmation: true,
                        decision: "network_policy_consent_required".into(),
                        reason: "single-use network consent was unavailable at dispatch".into(),
                        policy_id: permission.policy_id,
                    };
                    let (action, observation) = self.build_blocked_action_observation(
                        tool_name,
                        &args,
                        &inspection,
                        &forced_decision,
                        manifest.as_ref(),
                        &request,
                    );
                    return Ok(ActionExecutionResult {
                        action,
                        observation,
                        status: ActionExecutionStatus::NeedsConfirmation,
                        stop_reason: Some("network_policy_consent_required".into()),
                        execution_receipt: receipt_tracker.snapshot(),
                        observed_body_admission: None,
                    });
                }
            }
        }

        // 6. Execute
        let manifest_ref = manifest
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool manifest not found for '{}'", tool_name))?;
        let result = if manifest_ref.tags.contains(&"execution".to_string()) {
            let result = self
                .execute_execution_tool(
                    tool_name,
                    &args,
                    ctx,
                    &request,
                    manifest_ref,
                    receipt_tracker.clone(),
                    authorized_network_policy.as_ref(),
                )
                .await
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                });
            result
        } else {
            let admission = ctx
                .authorize_tool_dispatch(manifest_ref, &request, &args, &receipt_tracker)
                .await?;
            self.call_tool_internal(
                manifest_ref,
                args.clone(),
                ctx,
                inspection.pii_found,
                admission,
            )
            .await
        };

        let (mut action, mut observation, observed_body_admission) = self
            .build_success_action_observation(
                tool_name,
                &args,
                &result,
                manifest.as_ref(),
                &request,
            )?;
        if is_web_network_policy_blocker(tool_name, result.error.as_deref()) {
            action.status = "blocked".into();
            action.permission_decision = Some("network_policy_blocked".into());
            if let Some(structured) = observation.structured_result.as_mut() {
                if let Some(object) = structured.as_object_mut() {
                    object.insert("status".into(), serde_json::json!("blocked"));
                    object.insert("requires_confirmation".into(), serde_json::json!(false));
                    object.insert(
                        "permission_decision".into(),
                        serde_json::json!("network_policy_blocked"),
                    );
                    object.insert("network_policy_blocked".into(), serde_json::json!(true));
                }
            }
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::Blocked,
                stop_reason: Some("network_policy_blocked".into()),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission,
            });
        }
        let document_blocker = (tool_name == "document.read")
            .then(|| result.error.as_deref()?.split(':').next())
            .flatten()
            .filter(|reason| {
                matches!(
                    *reason,
                    "document_read_no_bound_content"
                        | "document_read_resource_store_unavailable"
                        | "document_read_bound_input_invalid"
                        | "document_read_selection_failed"
                )
            });
        if let Some(reason) = document_blocker {
            action.status = "blocked".into();
            action.permission_decision = Some(reason.into());
            if let Some(object) = observation
                .structured_result
                .as_mut()
                .and_then(Value::as_object_mut)
            {
                object.insert("status".into(), serde_json::json!("blocked"));
                object.insert("permission_decision".into(), serde_json::json!(reason));
            }
            return Ok(ActionExecutionResult {
                action,
                observation,
                status: ActionExecutionStatus::Blocked,
                stop_reason: Some(reason.into()),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission,
            });
        }

        let status = if result.success {
            ActionExecutionStatus::Succeeded
        } else {
            ActionExecutionStatus::Failed
        };
        let stop_reason = if !result.success && tool_name == "file.read" {
            Some("filesystem_read_failed".to_string())
        } else if !result.success && tool_name == "document.read" {
            result
                .error
                .as_deref()
                .and_then(|error| error.split(':').next())
                .map(str::to_string)
        } else {
            None
        };

        Ok(ActionExecutionResult {
            action,
            observation,
            status,
            stop_reason,
            execution_receipt: receipt_tracker.snapshot(),
            observed_body_admission,
        })
    }

    pub(crate) async fn call_tool_internal(
        &self,
        manifest: &ToolManifest,
        args: Value,
        ctx: &ActionExecutionContext<'_>,
        pii_found: bool,
        admission: super::ToolDispatchAdmission<'_>,
    ) -> ToolCallInternalResult {
        let (receipt_tracker, started_observer) = admission.into_remote_parts();
        receipt_tracker.mark_audit_persistence_pending();
        let result = match ctx
            .registry
            .execute_manifest_async_with_receipt_tracker(
                manifest,
                args.clone(),
                receipt_tracker.clone(),
                started_observer,
            )
            .await
        {
            Ok(output) => ToolCallInternalResult {
                success: true,
                output: Some(output),
                error: None,
            },
            Err(error) => ToolCallInternalResult {
                success: false,
                output: None,
                error: Some(error.to_string()),
            },
        };
        let audit_body = result
            .output
            .as_deref()
            .or(result.error.as_deref())
            .unwrap_or_default();
        match ctx.audit_store.insert_log(
            &manifest.name,
            &args,
            audit_body,
            result.success,
            pii_found,
        ) {
            Ok(_) => receipt_tracker.mark_audit_persistence_committed(),
            Err(_) => {
                receipt_tracker.mark_audit_persistence_failed();
                if let Some(observer) = ctx.tool_audit_persistence_observer {
                    observer.audit_persistence_failed(&receipt_tracker.snapshot());
                }
            }
        }
        result
    }

    pub fn build_blocked_action_observation(
        &self,
        tool_name: &str,
        args: &Value,
        inspection: &McpArgumentInspection,
        decision: &ToolPermissionDecision,
        manifest: Option<&ToolManifest>,
        request: &AgentActionRequest,
    ) -> (AgentAction, AgentObservation) {
        let now = chrono::Utc::now();
        let needs_confirmation = should_mark_needs_confirmation(decision, inspection);
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );

        let status = if needs_confirmation {
            "needs_confirmation"
        } else {
            "blocked"
        };

        let tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: canonical_tool_source(m),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: m.action_type.clone(),
            requires_confirmation: needs_confirmation,
            allowed: false,
        });
        let observation_id = format!(
            "observation-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let trace = self.build_tool_trace_envelope(
            &action_id,
            Some(&observation_id),
            tool_name,
            None,
            manifest,
            request,
            status,
            Some(decision.decision.clone()),
            Some(now),
            Some(now),
        );

        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(tool_name.to_string()),
            input: args.clone(),
            output: None,
            status: status.into(),
            permission_decision: Some(decision.decision.clone()),
            tool_scope,
            started_at: Some(now),
            finished_at: Some(now),
            error: if needs_confirmation {
                None
            } else {
                Some(decision.reason.clone())
            },
            timestamp: now,
            tool_trace: Some(trace.clone()),
            runtime_execution_receipt: None,
        };

        let observation = AgentObservation {
            id: observation_id,
            action_id: Some(action_id),
            content: if needs_confirmation {
                "Tool call requires permission confirmation".to_string()
            } else {
                decision.reason.clone()
            },
            source: manifest
                .map(canonical_tool_source)
                .unwrap_or_else(|| "builtin".to_string()),
            structured_result: Some(serde_json::json!({
                "success": false,
                "status": status,
                "requires_confirmation": needs_confirmation,
                "permission_decision": decision.decision,
                "directWritesExecuted": false,
            })),
            timestamp: now,
            tool_trace: Some(trace),
        };

        (action, observation)
    }

    fn build_success_action_observation(
        &self,
        tool_name: &str,
        args: &Value,
        result: &ToolCallInternalResult,
        manifest: Option<&ToolManifest>,
        request: &AgentActionRequest,
    ) -> Result<(
        AgentAction,
        AgentObservation,
        Option<ObservedToolBodyAdmission>,
    )> {
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );

        let status = if result.success {
            "succeeded"
        } else {
            "failed"
        };

        let tool_scope = manifest.map(|m| ToolActionScope {
            tool_name: m.name.clone(),
            tool_id: m.id.clone(),
            source: canonical_tool_source(m),
            risk_level: m.risk_level.clone(),
            capabilities: m.capabilities.clone(),
            action_type: m.action_type.clone(),
            requires_confirmation: false,
            allowed: result.success,
        });
        let observation_id = format!(
            "observation-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let trace = self.build_tool_trace_envelope(
            &action_id,
            Some(&observation_id),
            tool_name,
            result
                .output
                .as_deref()
                .map(|body| (body, ContentReceiptKind::ToolOutput))
                .or_else(|| {
                    result
                        .error
                        .as_deref()
                        .map(|body| (body, ContentReceiptKind::ToolError))
                }),
            manifest,
            request,
            status,
            None,
            Some(now),
            Some(now),
        );

        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(tool_name.to_string()),
            input: args.clone(),
            output: result
                .output
                .as_ref()
                .map(|output| serde_json::json!({ "text": output })),
            status: status.into(),
            permission_decision: None,
            tool_scope,
            started_at: Some(now),
            finished_at: Some(now),
            error: result.error.clone(),
            timestamp: now,
            tool_trace: Some(trace.clone()),
            runtime_execution_receipt: None,
        };

        let observation = AgentObservation {
            id: observation_id,
            action_id: Some(action_id),
            content: result
                .output
                .clone()
                .or_else(|| result.error.clone())
                .unwrap_or_else(|| "Tool call produced no output".to_string()),
            source: manifest
                .map(canonical_tool_source)
                .unwrap_or_else(|| "builtin".to_string()),
            structured_result: Some(serde_json::json!({
                "success": result.success,
                "status": status,
                "requires_confirmation": false,
                "permission_decision": null,
                "directWritesExecuted": false,
            })),
            timestamp: now,
            tool_trace: Some(trace),
        };

        let observed_body_admission = mint_observed_body_admission(
            manifest.and_then(|_| {
                result
                    .output
                    .as_deref()
                    .map(|body| (body, ContentReceiptKind::ToolOutput))
                    .or_else(|| {
                        result
                            .error
                            .as_deref()
                            .map(|body| (body, ContentReceiptKind::ToolError))
                    })
            }),
            request,
            &action,
            &observation,
            false,
        )?;

        Ok((action, observation, observed_body_admission))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn build_tool_trace_envelope(
        &self,
        action_id: &str,
        observation_id: Option<&str>,
        tool_name: &str,
        observed_body: Option<(&str, ContentReceiptKind)>,
        manifest: Option<&ToolManifest>,
        request: &AgentActionRequest,
        status: &str,
        permission_decision: Option<String>,
        started_at: Option<chrono::DateTime<chrono::Utc>>,
        finished_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> ToolActionTraceEnvelope {
        let output_preview = observed_body.map(|(text, _)| metadata_safe_text_preview(text));
        let output_item_count = observed_body
            .and_then(|(text, _)| serde_json::from_str::<Value>(text).ok())
            .map(|value| match value {
                Value::Array(items) => items.len(),
                Value::Object(map) => map.len(),
                Value::Null => 0,
                _ => 1,
            });
        let tool_source = manifest
            .map(canonical_tool_source)
            .unwrap_or_else(|| "unregistered".into());
        let action_category = manifest
            .map(|manifest| {
                if manifest.action_type.is_empty() {
                    "read".to_string()
                } else {
                    manifest.action_type.clone()
                }
            })
            .unwrap_or_else(|| "unknown".into());

        ToolActionTraceEnvelope {
            run_id: request.source_run_id.clone(),
            action_id: action_id.to_string(),
            step_index: request.step_index,
            tool_call_index: request.step_index,
            action_type: request.action_type.clone(),
            tool_id: manifest
                .map(|m| m.id.clone())
                .unwrap_or_else(|| tool_name.to_string()),
            tool_name: manifest
                .map(|m| m.name.clone())
                .unwrap_or_else(|| tool_name.to_string()),
            tool_source,
            action_category,
            risk_level: manifest
                .map(|m| m.risk_level.clone())
                .unwrap_or_else(|| "unknown".into()),
            permission_decision,
            status: status.to_string(),
            proposal_id: None,
            observation_id: observation_id.map(str::to_string),
            observation_status: Some(status.to_string()),
            output_preview,
            output_receipt: None,
            output_item_count,
            started_at,
            finished_at,
            metadata_safe: true,
        }
    }

    fn build_network_policy_blocked_result(
        &self,
        input: NetworkPolicyBlockedInput<'_>,
    ) -> ActionExecutionResult {
        let NetworkPolicyBlockedInput {
            request,
            tool_name,
            args,
            manifest,
            inspection,
            decision,
            execution_receipt,
        } = input;
        let forced_decision = ToolPermissionDecision {
            allowed: false,
            requires_confirmation: false,
            decision: decision.reason_code.clone(),
            reason: format!(
                "network policy denied '{}' for host '{}' (decision_id={})",
                decision.capability, decision.host, decision.decision_id
            ),
            policy_id: None,
        };
        let (mut action, mut observation) = self.build_blocked_action_observation(
            tool_name,
            args,
            inspection,
            &forced_decision,
            manifest,
            request,
        );
        action.permission_decision = Some(decision.reason_code.clone());
        if let Some(object) = observation
            .structured_result
            .as_mut()
            .and_then(Value::as_object_mut)
        {
            object.insert(
                "networkPolicyDecisionId".into(),
                serde_json::json!(decision.decision_id),
            );
            object.insert("networkHost".into(), serde_json::json!(decision.host));
            object.insert("networkPolicyBlocked".into(), serde_json::json!(true));
            object.insert("network_policy_blocked".into(), serde_json::json!(true));
            object.insert(
                "networkPolicyReasonCode".into(),
                serde_json::json!(decision.reason_code),
            );
            object.insert("directWritesExecuted".into(), serde_json::json!(false));
        }
        ActionExecutionResult {
            action,
            observation,
            status: ActionExecutionStatus::Blocked,
            stop_reason: Some(decision.reason_code.clone()),
            execution_receipt,
            observed_body_admission: None,
        }
    }
}

pub(super) fn record_effect_outcome(receipt_tracker: &ToolExecutionReceiptTracker, success: bool) {
    if success {
        receipt_tracker.mark_execution_succeeded();
        receipt_tracker.mark_effect_confirmed();
    } else {
        receipt_tracker.mark_execution_failed();
        receipt_tracker.mark_effect_unknown_if_dispatched();
    }
}

fn network_permission_scope(
    decision: &NetworkPolicyDecision,
    request: &AgentActionRequest,
    args: &Value,
) -> String {
    let (_, action_digest) = metadata_safe_value_digest(&serde_json::json!({
        "networkPolicyDecisionId": decision.decision_id,
        "sourceRunId": request.source_run_id,
        "stepIndex": request.step_index,
        "arguments": args,
    }));
    format!(
        "network-consent@{}#action:{}",
        decision.decision_id, action_digest
    )
}

fn network_policy_after_exact_consent(
    policy: &crate::config::NetworkPolicy,
    decision: &NetworkPolicyDecision,
) -> crate::config::NetworkPolicy {
    let mut authorized = policy.clone();
    // The grant is bound to this exact policy decision and host. Constrain the
    // transport policy so a redirect cannot inherit consent for a different
    // destination.
    authorized.default_decision = "deny".into();
    authorized.domain_allowlist = vec![decision.host.clone()];
    authorized
        .tool_overrides
        .insert(decision.capability.clone(), "allow".into());
    authorized
}

fn is_web_network_policy_blocker(tool_name: &str, error: Option<&str>) -> bool {
    if !matches!(tool_name, "web.fetch" | "web.search") {
        return false;
    }
    let Some(error) = error else {
        return false;
    };
    [
        "Network tools are disabled by policy",
        "denied by network policy override",
        "network denylist",
        "not in the network allowlist",
        "private/internal address",
        "Invalid URL scheme",
        "network_policy_blocked",
        "network_domain_denied",
        "network_domain_not_allowlisted",
        "network_private_or_reserved_address_blocked",
        "network_url_scheme_blocked",
        "network_redirect_limit_exceeded",
        "network_response_body_too_large",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}
