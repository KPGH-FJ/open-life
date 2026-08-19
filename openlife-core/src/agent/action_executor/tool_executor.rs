use crate::mcp::McpArgumentInspection;
use crate::tool_manifest::{ToolManifest, ToolSource};
use crate::tool_permissions::ToolPermissionDecision;
use anyhow::Result;
use serde_json::Value;

use super::helpers::{
    canonical_tool_source, configured_web_search_endpoint, ensure_external_write_content_size,
    external_write_content_preview, filesystem_access_error, is_direct_external_write_tool,
    is_path_lexically_in_safe_paths, is_proposal_generation_tool,
    minimized_external_write_arguments, normalize_tool_name,
    policy_requires_external_write_proposal, should_mark_needs_confirmation,
    ToolCallInternalResult,
};
use super::ActionExecutionContext;
use super::ActionExecutionResult;
use super::ActionExecutionStatus;
use super::AgentActionRequest;
use crate::agent::metadata_safe::{metadata_safe_text_preview, metadata_safe_value_digest};
use crate::agent::policy_store::BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST;
use crate::agent::review_workflow::{DurableWriteRequest, DurableWriteSource, DurableWriteSubject};
use crate::agent::tool_gateway::ToolGatewayContractEvidence;
use crate::agent::types::{
    AgentAction, AgentObservation, AgentProposal, BoundContentField, ContentReceiptBinding,
    ContentReceiptKind, ProposalSource, ProposalType, RiskLevel, ToolActionScope,
    ToolActionTraceEnvelope,
};
use crate::agent::{ExternalWriteGovernanceInput, LifeModelGovernor, MemoryWriteGovernanceInput};
use crate::network_client::{
    resolve_network_policy_decision, NetworkPolicyDecision, NetworkPolicyDisposition,
};
use crate::tool_execution_receipt::{ToolActionEffect, ToolExecutionReceiptTracker};
use ring::digest::{digest, SHA256};

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
    decision: NetworkPolicyDecision,
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
                governance_report: None,
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
                governance_report: None,
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission: None,
            });
        }

        // Network policy is an execution authorization, not a post-dispatch
        // transport error. Resolve it before generic tool permission handling,
        // dispatch observers, and receipt mutation. An `ask` decision must
        // materialize a durable ReviewWorkflow item or remain fail-closed.
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
                            governance_report: None,
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
                            if let Some(result) = self.create_network_policy_consent_proposal(
                                &request,
                                ctx,
                                tool_name,
                                &args,
                                &network_decision,
                                &receipt_tracker,
                            ) {
                                return result;
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
                                governance_report: None,
                                execution_receipt: receipt_tracker.snapshot(),
                                observed_body_admission: None,
                            });
                        }
                        authorized_network_policy = Some(network_policy_after_exact_consent(
                            policy,
                            &network_decision,
                        ));
                        pending_network_authorization = Some(PendingNetworkAuthorization {
                            permission_scope,
                            decision: network_decision,
                        });
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

        if let Some(ref m) = manifest {
            if !self.config.allow_writes
                && is_direct_external_write_tool(m)
                && !is_proposal_generation_tool(&m.name)
            {
                let forced_decision = ToolPermissionDecision {
                    allowed: false,
                    requires_confirmation: false,
                    decision: "blocked".into(),
                    reason: "write-like tool blocked because allow_writes=false".into(),
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
                    stop_reason: Some("allow_writes_false".into()),
                    governance_report: None,
                    execution_receipt: receipt_tracker.snapshot(),
                    observed_body_admission: None,
                });
            }
        }

        // 4. Determine if blocked.
        // Proposal-generation tools (file.write_proposal, memory.propose_write, etc.)
        // only create proposals; they don't execute side effects directly.
        // They are exempt from permission-confirmation blocking so the agent can
        // always reach the handler that creates the proposal for user review.
        let is_proposal_tool = manifest
            .as_ref()
            .is_none_or(|m| is_proposal_generation_tool(&m.name));

        if let Some(m) = manifest.as_ref().filter(|m| {
            policy_requires_external_write_proposal(ctx) && is_direct_external_write_tool(m)
        }) {
            if let Some(result) =
                self.create_external_write_action_proposal(&request, ctx, tool_name, &args, m)
            {
                return result;
            }

            let forced_decision = ToolPermissionDecision {
                allowed: false,
                requires_confirmation: true,
                decision: "proposal_required".into(),
                reason: "HS proposal-first policy requires an ExternalWriteAction proposal before direct external write".into(),
                policy_id: Some(BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST.into()),
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
                stop_reason: Some("hs_external_write_proposal_first".into()),
                governance_report: Some(
                    LifeModelGovernor
                        .govern_external_write(ExternalWriteGovernanceInput {
                            tool_name: tool_name.clone(),
                            risk_level: manifest_risk_level(m),
                            source_run_id: request.source_run_id.clone(),
                            proposal_already_created: false,
                        })
                        .to_report(),
                ),
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission: None,
            });
        }

        let permission_blocks =
            !is_proposal_tool && (decision.requires_confirmation || !decision.allowed);
        let inspection_blocks =
            !is_proposal_tool && inspection.requires_confirmation && inspection.pii_found;
        let blocked = manifest
            .as_ref()
            .is_none_or(|m| !m.enabled || m.declarative_only)
            || inspection_blocks
            || permission_blocks;

        if blocked {
            // Special handling for declarative stubs that should create proposals
            if let Some(ref m) = manifest {
                if m.declarative_only {
                    match tool_name.as_str() {
                        "calendar.propose_event" => {
                            if let Some(result) = self.create_declarative_stub_proposal(
                                &request,
                                ctx,
                                tool_name,
                                &args,
                                ProposalType::ScheduledTask,
                                "calendar",
                                "Agent proposed calendar event",
                            ) {
                                return result;
                            }
                        }
                        "email.propose_draft" => {
                            if let Some(result) = self.create_declarative_stub_proposal(
                                &request,
                                ctx,
                                tool_name,
                                &args,
                                ProposalType::DataExport,
                                "email",
                                "Agent proposed email draft",
                            ) {
                                return result;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let needs_confirmation = should_mark_needs_confirmation(&decision, &inspection);

            // PolicyStore proposal-first decisions convert blocked direct writes into
            // user-reviewable ExternalWriteAction proposals.
            if needs_confirmation {
                if let Some(m) = manifest.as_ref().filter(|m| {
                    policy_requires_external_write_proposal(ctx) && is_direct_external_write_tool(m)
                }) {
                    if let Some(result) = self
                        .create_external_write_action_proposal(&request, ctx, tool_name, &args, m)
                    {
                        return result;
                    }
                }
            }

            // Auto-generate ToolPermission Proposal when blocked by policy
            // so the user can grant permission and continue in the Review Center.
            if needs_confirmation
                && manifest
                    .as_ref()
                    .is_some_and(|m| !m.declarative_only && !is_proposal_generation_tool(&m.name))
            {
                if let Some(result) = self.create_tool_permission_proposal(
                    &request,
                    ctx,
                    tool_name,
                    &args,
                    manifest.as_ref(),
                    &decision,
                ) {
                    return result;
                }
                // Fall-through: if proposal creation fails, return NeedsConfirmation status
            }

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
                governance_report: None,
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
                        governance_report: None,
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
                    if let Some(result) = self.create_network_policy_consent_proposal(
                        &request,
                        ctx,
                        tool_name,
                        &args,
                        &pending.decision,
                        &receipt_tracker,
                    ) {
                        return result;
                    }
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
                        governance_report: None,
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
        let result = if manifest_ref.tags.contains(&"core_os".to_string()) {
            let result = self
                .execute_core_os_tool(
                    tool_name,
                    &args,
                    &request,
                    ctx,
                    manifest_ref,
                    receipt_tracker.clone(),
                )
                .await
                .unwrap_or_else(|e| ToolCallInternalResult {
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                });
            result
        } else if manifest_ref.tags.contains(&"execution".to_string()) {
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
                governance_report: None,
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
                governance_report: None,
                execution_receipt: receipt_tracker.snapshot(),
                observed_body_admission,
            });
        }

        // For mcp.call_tool: override tool_scope with target manifest and handle
        // target tool permission failures as NeedsConfirmation instead of Failed
        if tool_name == "mcp.call_tool" {
            let mut target_permission_manifest: Option<ToolManifest> = None;
            if let Some(target_name) = args.get("tool_name").and_then(|v: &Value| v.as_str()) {
                let target_manifest = ctx
                    .registry
                    .list_manifests()
                    .into_iter()
                    .find(|m| m.name == target_name || m.id == target_name);
                if let Some(target_manifest) = target_manifest {
                    let target_source = canonical_tool_source(&target_manifest);
                    observation.source = target_source.clone();
                    action.tool_scope = Some(ToolActionScope {
                        tool_name: target_manifest.name.clone(),
                        tool_id: target_manifest.id.clone(),
                        source: target_source.clone(),
                        risk_level: target_manifest.risk_level.clone(),
                        capabilities: target_manifest.capabilities.clone(),
                        action_type: target_manifest.action_type.clone(),
                        requires_confirmation: false,
                        allowed: result.success,
                    });
                    for trace in action
                        .tool_trace
                        .iter_mut()
                        .chain(observation.tool_trace.iter_mut())
                    {
                        trace.tool_name = target_manifest.name.clone();
                        trace.tool_id = target_manifest.id.clone();
                        trace.tool_source = target_source.clone();
                        trace.risk_level = target_manifest.risk_level.clone();
                        trace.action_category = target_manifest.action_type.clone();
                    }
                    target_permission_manifest = Some(target_manifest);
                } else {
                    action.status = "blocked".to_string();
                    action.permission_decision = Some("mcp_read_tool_not_registered".into());
                    if let Some(structured) = observation.structured_result.as_mut() {
                        if let Some(object) = structured.as_object_mut() {
                            object.insert("status".into(), serde_json::json!("blocked"));
                            object.insert("requires_confirmation".into(), serde_json::json!(false));
                            object.insert(
                                "permission_decision".into(),
                                serde_json::json!("mcp_read_tool_not_registered"),
                            );
                            object.insert(
                                "blockerReason".into(),
                                serde_json::json!("mcp_read_tool_not_registered"),
                            );
                            object.insert("directWritesExecuted".into(), serde_json::json!(false));
                        }
                    }
                    return Ok(ActionExecutionResult {
                        action,
                        observation,
                        status: ActionExecutionStatus::Blocked,
                        stop_reason: Some("mcp_read_tool_not_registered".into()),
                        governance_report: None,
                        execution_receipt: receipt_tracker.snapshot(),
                        observed_body_admission,
                    });
                }
            }
            // If target tool permission was denied, treat as NeedsConfirmation
            if !result.success {
                if let Some(ref error) = result.error {
                    if error.contains("hs_external_write_proposal_first") {
                        action.status = "needs_confirmation".to_string();
                        action.permission_decision = Some("proposal_required".into());
                        if let Some(structured) = observation.structured_result.as_mut() {
                            if let Some(object) = structured.as_object_mut() {
                                object.insert(
                                    "status".into(),
                                    serde_json::json!("needs_confirmation"),
                                );
                                object.insert(
                                    "requires_confirmation".into(),
                                    serde_json::json!(true),
                                );
                                object.insert(
                                    "permission_decision".into(),
                                    serde_json::json!("proposal_required"),
                                );
                                object.insert("proposal_required".into(), serde_json::json!(true));
                            }
                        }
                        return Ok(ActionExecutionResult {
                            action,
                            observation,
                            status: ActionExecutionStatus::NeedsConfirmation,
                            stop_reason: Some("hs_external_write_proposal_first".into()),
                            governance_report: None,
                            execution_receipt: receipt_tracker.snapshot(),
                            observed_body_admission,
                        });
                    }
                    if error.contains("blocked") || error.contains("ask_every_time") {
                        if let Some(target_manifest) = target_permission_manifest.as_ref() {
                            let target_args = args
                                .get("arguments")
                                .cloned()
                                .unwrap_or_else(|| serde_json::json!({}));
                            let forced_decision = ToolPermissionDecision {
                                allowed: false,
                                requires_confirmation: true,
                                decision: "ask_every_time".into(),
                                reason: "target tool requires permission".into(),
                                policy_id: None,
                            };
                            if let Some(result) = self.create_tool_permission_proposal(
                                &request,
                                ctx,
                                &target_manifest.name,
                                &target_args,
                                Some(target_manifest),
                                &forced_decision,
                            ) {
                                return result;
                            }
                        }
                        action.status = "needs_confirmation".to_string();
                        return Ok(ActionExecutionResult {
                            action,
                            observation,
                            status: ActionExecutionStatus::NeedsConfirmation,
                            stop_reason: Some("target_tool_needs_confirmation".into()),
                            governance_report: None,
                            execution_receipt: receipt_tracker.snapshot(),
                            observed_body_admission,
                        });
                    }
                }
            }
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
            governance_report: None,
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

    pub(super) fn build_internal_read_action_observation(
        &self,
        request: &AgentActionRequest,
        contract: &ToolGatewayContractEvidence,
        succeeded: bool,
        observation_content: String,
        structured_result: Value,
        failure_code: Option<String>,
    ) -> Result<(
        AgentAction,
        AgentObservation,
        Option<ObservedToolBodyAdmission>,
    )> {
        if contract.source != "tool_gateway_internal"
            || contract.action_effect != ToolActionEffect::ReadOnly
            || contract.idempotency_contract
                != crate::tool_manifest::ToolIdempotencyContract::Idempotent
            || contract.action_type != "read"
            || contract.capabilities != ["read"]
            || contract.manifest_id != request.action_type
            || contract.tool_name != request.target
        {
            anyhow::bail!("tool_gateway_internal_read_contract_binding_mismatch");
        }
        if succeeded && failure_code.is_some() {
            anyhow::bail!("tool_gateway_internal_read_success_error_conflict");
        }
        if !succeeded && failure_code.is_none() {
            anyhow::bail!("tool_gateway_internal_read_failure_code_missing");
        }

        let now = chrono::Utc::now();
        let action_id = uuid::Uuid::new_v4().to_string();
        let observation_id = uuid::Uuid::new_v4().to_string();
        let status = if succeeded { "succeeded" } else { "failed" };
        let permission_decision = "read_only_memory_search".to_string();
        let observed_body =
            succeeded.then_some((observation_content.as_str(), ContentReceiptKind::ToolOutput));
        let trace = ToolActionTraceEnvelope {
            run_id: request.source_run_id.clone(),
            action_id: action_id.clone(),
            step_index: request.step_index,
            tool_call_index: request.step_index,
            action_type: contract.tool_name.clone(),
            tool_id: contract.manifest_id.clone(),
            tool_name: contract.tool_name.clone(),
            tool_source: contract.source.clone(),
            action_category: contract.action_type.clone(),
            risk_level: contract.risk_level.clone(),
            permission_decision: Some(permission_decision.clone()),
            status: status.into(),
            proposal_id: None,
            observation_id: Some(observation_id.clone()),
            observation_status: Some(status.into()),
            output_preview: observed_body.map(|(body, _)| metadata_safe_text_preview(body)),
            output_receipt: None,
            output_item_count: structured_result
                .get("hitCount")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok()),
            started_at: Some(now),
            finished_at: Some(now),
            metadata_safe: true,
        };
        let action = AgentAction {
            id: action_id.clone(),
            action_type: contract.tool_name.clone(),
            target: Some(contract.tool_name.clone()),
            input: request.input.clone(),
            output: succeeded.then(|| serde_json::json!({"text": observation_content.clone()})),
            status: status.into(),
            error: failure_code,
            permission_decision: Some(permission_decision),
            started_at: Some(now),
            finished_at: Some(now),
            timestamp: now,
            tool_scope: Some(ToolActionScope {
                tool_id: contract.manifest_id.clone(),
                tool_name: contract.tool_name.clone(),
                source: contract.source.clone(),
                risk_level: contract.risk_level.clone(),
                capabilities: contract.capabilities.clone(),
                action_type: contract.action_type.clone(),
                requires_confirmation: false,
                // The gateway authorized this read capability before
                // execution. A failed adapter/result does not retroactively
                // turn an allowed read into a policy denial; terminal status
                // and stop_reason retain the execution failure.
                allowed: true,
            }),
            tool_trace: Some(trace.clone()),
            runtime_execution_receipt: None,
        };
        let observation = AgentObservation {
            id: observation_id,
            action_id: Some(action_id),
            content: observation_content.clone(),
            source: request.action_type.clone(),
            structured_result: Some(structured_result),
            timestamp: now,
            tool_trace: Some(trace),
        };
        let admission =
            mint_observed_body_admission(observed_body, request, &action, &observation, true)?;
        Ok((action, observation, admission))
    }

    pub fn build_proposal_required_action(
        &self,
        request: AgentActionRequest,
        reason: &str,
    ) -> ActionExecutionResult {
        let execution_receipt = super::receipt_tracker_for_request(
            &request,
            None,
            ToolActionEffect::ProposalOnly,
            crate::tool_manifest::ToolIdempotencyContract::NonIdempotent,
        )
        .snapshot();
        let now = chrono::Utc::now();
        let action_id = format!(
            "action-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let status = if self.config.allow_writes {
            "needs_confirmation"
        } else {
            "blocked"
        };
        let observation_id = format!(
            "observation-{}-{}",
            request.step_index,
            now.timestamp_nanos_opt().unwrap_or_default()
        );
        let trace = self.build_tool_trace_envelope(
            &action_id,
            Some(&observation_id),
            &request.target,
            None,
            None,
            &request,
            status,
            Some("proposal_required".into()),
            Some(now),
            Some(now),
        );
        let action = AgentAction {
            id: action_id.clone(),
            action_type: request.action_type.clone(),
            target: Some(request.target.clone()),
            input: request.input.clone(),
            output: None,
            status: status.into(),
            permission_decision: Some("proposal_required".into()),
            tool_scope: None,
            started_at: Some(now),
            finished_at: Some(now),
            error: Some(reason.to_string()),
            timestamp: now,
            tool_trace: Some(trace.clone()),
            runtime_execution_receipt: None,
        };
        let observation = AgentObservation {
            id: observation_id,
            action_id: Some(action_id),
            content: reason.to_string(),
            source: "action_executor".into(),
            structured_result: Some(serde_json::json!({
                "success": false,
                "status": status,
                "requires_confirmation": self.config.allow_writes,
                "permission_decision": "proposal_required",
                "proposal_required": true,
            })),
            timestamp: now,
            tool_trace: Some(trace),
        };
        let governance_report = match request.action_type.as_str() {
            "memory_write" | "memory_archive" => Some(
                LifeModelGovernor
                    .govern_memory_write(MemoryWriteGovernanceInput {
                        risk_level: RiskLevel::Medium,
                        source_run_id: request.source_run_id.clone(),
                        proposal_already_created: false,
                    })
                    .to_report(),
            ),
            _ => None,
        };

        ActionExecutionResult {
            action,
            observation,
            status: if self.config.allow_writes {
                ActionExecutionStatus::NeedsConfirmation
            } else {
                ActionExecutionStatus::Blocked
            },
            stop_reason: Some("proposal_required".into()),
            governance_report,
            execution_receipt,
            observed_body_admission: None,
        }
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
        let proposal_id = observed_body.and_then(|(text, _)| extract_proposal_id_from_text(text));
        let tool_source = manifest
            .map(canonical_tool_source)
            .unwrap_or_else(|| "unregistered".into());
        let action_category = manifest
            .map(|m| {
                if proposal_id.is_some() || is_proposal_generation_tool(&m.name) {
                    "proposal".to_string()
                } else if m.action_type.is_empty() {
                    "read".to_string()
                } else {
                    m.action_type.clone()
                }
            })
            .unwrap_or_else(|| {
                if request.action_type.contains("proposal") {
                    "proposal".into()
                } else {
                    "unknown".into()
                }
            });

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
            proposal_id,
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

    /// Auto-create a ToolPermission Proposal when a tool is blocked by policy.
    /// The proposal records the blocked action so it can be replayed after the
    /// user grants permission in the Review Center.
    fn create_tool_permission_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        manifest: Option<&ToolManifest>,
        decision: &ToolPermissionDecision,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        ctx.proposal_store?;
        let source = manifest
            .map(canonical_tool_source)
            .unwrap_or_else(|| "builtin".to_string());
        let risk_level = manifest
            .map(|m| m.risk_level.clone())
            .unwrap_or_else(|| "medium".to_string());
        let action_type = manifest
            .map(|m| m.action_type.clone())
            .unwrap_or_else(|| request.action_type.clone());
        let capabilities = manifest.map(|m| m.capabilities.clone()).unwrap_or_default();
        let (input_length_bytes, input_hash) = metadata_safe_value_digest(args);
        let after = serde_json::json!({
            "permission_action": "grant",
            "permission_scope_kind": "action_bound",
            "permission": "allow_once",
            "tool_name": tool_name,
            "source": source.clone(),
            "risk_level": risk_level.clone(),
            "policy": "allow_once",
            "canonical_scope": {
                "tool_name": tool_name,
                "source": source.clone(),
                "risk_level": risk_level.clone(),
                "action_type": action_type,
                "capabilities": capabilities,
                "blocked_run_id": request.source_run_id,
                "blocked_step_index": request.step_index,
                "input_hash": input_hash,
                "input_length_bytes": input_length_bytes,
            },
            "blocked_action": {
                "action_type": request.action_type,
                "target": request.target,
                "resolved_target": tool_name,
                "source_run_id": request.source_run_id,
                "step_index": request.step_index,
                "input_hash": input_hash,
                "input_length_bytes": input_length_bytes,
            },
            "reason": decision.reason,
            "auto_generated": true,
            "directWritesExecuted": false,
        });

        let affected_path = format!("tool_permission.{}.{}", source, tool_name);
        let mut proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            &affected_path,
            after,
            &format!(
                "[Auto] 工具 '{}' ({}，风险等级：{}) 需要权限确认。原因：{}",
                tool_name, source, risk_level, decision.reason
            ),
            0.7,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );

        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }

        let outcome = match ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::ToolPermission,
            DurableWriteSubject::ToolPermission,
            proposal,
            "Tool permission proposal is pending Review Center approval.",
        )) {
            Ok(outcome) => outcome,
            Err(e) => {
                eprintln!(
                    "[warn] Failed to create ToolPermission Proposal for {}: {}",
                    tool_name, e
                );
                return Some(Err(e));
            }
        };

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: 已创建 ToolPermission 提案 (id: {})，请前往 Review Center 审批",
                tool_name,
                outcome.proposal_id()
            ),
        );
        result.status = ActionExecutionStatus::NeedsConfirmation;
        result.stop_reason = Some("tool_permission_required".into());
        result.action.status = "needs_confirmation".into();
        result.action.permission_decision = Some("tool_permission_required".into());
        if let Some(structured) = result.observation.structured_result.as_mut() {
            if let Some(object) = structured.as_object_mut() {
                object.insert("status".into(), serde_json::json!("needs_confirmation"));
                object.insert("requires_confirmation".into(), serde_json::json!(true));
                object.insert(
                    "permission_decision".into(),
                    serde_json::json!("tool_permission_required"),
                );
                object.insert(
                    "proposalId".into(),
                    serde_json::json!(outcome.proposal_id()),
                );
                object.insert("directWritesExecuted".into(), serde_json::json!(false));
            }
        }
        if let Some(trace) = result.action.tool_trace.as_mut() {
            trace.proposal_id = Some(outcome.proposal_id().to_string());
            trace.status = "needs_confirmation".into();
            trace.permission_decision = Some("tool_permission_required".into());
            trace.action_category = "proposal".into();
        }
        if let Some(trace) = result.observation.tool_trace.as_mut() {
            trace.proposal_id = Some(outcome.proposal_id().to_string());
            trace.status = "needs_confirmation".into();
            trace.observation_status = Some("needs_confirmation".into());
            trace.permission_decision = Some("tool_permission_required".into());
            trace.action_category = "proposal".into();
        }

        Some(Ok(result))
    }

    fn create_network_policy_consent_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        decision: &NetworkPolicyDecision,
        receipt_tracker: &ToolExecutionReceiptTracker,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        ctx.proposal_store?;
        let permission_scope = network_permission_scope(decision, request, args);
        let (input_length_bytes, input_digest) = metadata_safe_value_digest(args);
        let after = serde_json::json!({
            "permission_action": "grant",
            "permission_scope_kind": "network_policy",
            "permission": "allow_once",
            "tool_name": permission_scope,
            "source": "network_policy",
            "risk_level": "medium",
            "action_type": "network",
            "canonical_scope": {
                "tool_name": permission_scope,
                "source": "network_policy",
                "risk_level": "medium",
                "action_type": "network",
                "capabilities": ["network", "external_side_effect"],
                "network_policy_decision_id": decision.decision_id,
                "network_capability": decision.capability,
                "host": decision.host,
                "blocked_run_id": request.source_run_id,
                "blocked_step_index": request.step_index,
                "input_digest": input_digest,
                "input_length_bytes": input_length_bytes,
            },
            "blocked_action": {
                "action_type": request.action_type,
                "target": request.target,
                "source_run_id": request.source_run_id,
                "step_index": request.step_index,
                "network_policy_decision_id": decision.decision_id,
            },
            "reason": decision.reason_code,
            "auto_generated": true,
            "directWritesExecuted": false,
        });
        let mut proposal = AgentProposal::new(
            ProposalType::ToolPermission,
            &format!("tool_permission.network_policy.{}", decision.decision_id),
            after,
            &format!(
                "Allow one '{}' network request to '{}' after explicit review.",
                tool_name, decision.host
            ),
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        proposal.source_detail = Some(format!("network_policy_consent:{}", decision.decision_id));
        proposal.run_id.clone_from(&request.source_run_id);

        let workflow_request = DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::ToolPermission,
            DurableWriteSubject::ToolPermission,
            proposal,
            "Network consent is pending Review Center approval.",
        )
        .with_idempotency_key(format!("network_policy_consent:{permission_scope}"))
        .with_evidence_refs(vec![format!(
            "network_policy_decision:{}",
            decision.decision_id
        )]);
        let outcome = match ctx.submit_review_proposal(workflow_request) {
            Ok(outcome) => outcome,
            Err(error) => return Some(Err(error)),
        };

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: network consent is pending Review Center approval (proposal id: {})",
                tool_name,
                outcome.proposal_id()
            ),
        );
        result.execution_receipt = receipt_tracker.snapshot();
        result.status = ActionExecutionStatus::NeedsConfirmation;
        result.stop_reason = Some("network_policy_consent_required".into());
        result.action.status = "needs_confirmation".into();
        result.action.permission_decision = Some("network_policy_consent_required".into());
        if let Some(structured) = result.observation.structured_result.as_mut() {
            if let Some(object) = structured.as_object_mut() {
                object.insert("status".into(), serde_json::json!("needs_confirmation"));
                object.insert("requires_confirmation".into(), serde_json::json!(true));
                object.insert(
                    "permission_decision".into(),
                    serde_json::json!("network_policy_consent_required"),
                );
                object.insert(
                    "networkPolicyDecisionId".into(),
                    serde_json::json!(decision.decision_id),
                );
                object.insert("networkHost".into(), serde_json::json!(decision.host));
                object.insert(
                    "proposalId".into(),
                    serde_json::json!(outcome.proposal_id()),
                );
                object.insert("directWritesExecuted".into(), serde_json::json!(false));
            }
        }
        for trace in [
            result.action.tool_trace.as_mut(),
            result.observation.tool_trace.as_mut(),
        ]
        .into_iter()
        .flatten()
        {
            trace.proposal_id = Some(outcome.proposal_id().to_string());
            trace.status = "needs_confirmation".into();
            trace.permission_decision = Some("network_policy_consent_required".into());
            trace.action_category = "proposal".into();
        }
        if let Some(trace) = result.observation.tool_trace.as_mut() {
            trace.observation_status = Some("needs_confirmation".into());
        }

        Some(Ok(result))
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
            governance_report: None,
            execution_receipt,
            observed_body_admission: None,
        }
    }

    pub(crate) fn create_external_write_action_proposal(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        manifest: &ToolManifest,
    ) -> Option<anyhow::Result<ActionExecutionResult>> {
        let proposal_id = match self
            .create_external_write_action_proposal_record(request, ctx, tool_name, args, manifest)
        {
            Some(Ok(proposal_id)) => proposal_id,
            Some(Err(e)) => return Some(Err(e)),
            None => return None,
        };

        let mut result = self.build_proposal_required_action(
            request.clone(),
            &format!(
                "{}: created ExternalWriteAction proposal (id: {}) for HS proposal-first policy",
                tool_name, proposal_id
            ),
        );
        result.stop_reason = Some("hs_external_write_proposal_first".into());
        result.governance_report = Some(
            LifeModelGovernor
                .govern_external_write(ExternalWriteGovernanceInput {
                    tool_name: tool_name.to_string(),
                    risk_level: manifest_risk_level(manifest),
                    source_run_id: request.source_run_id.clone(),
                    proposal_already_created: false,
                })
                .to_report(),
        );
        if let Some(trace) = result.action.tool_trace.as_mut() {
            trace.proposal_id = Some(proposal_id.clone());
            trace.action_category = "proposal".into();
        }
        if let Some(trace) = result.observation.tool_trace.as_mut() {
            trace.proposal_id = Some(proposal_id);
            trace.action_category = "proposal".into();
        }

        Some(Ok(result))
    }

    pub(crate) fn create_external_write_action_proposal_record(
        &self,
        request: &AgentActionRequest,
        ctx: &ActionExecutionContext<'_>,
        tool_name: &str,
        args: &Value,
        manifest: &ToolManifest,
    ) -> Option<anyhow::Result<String>> {
        ctx.proposal_store?;
        let source = canonical_tool_source(manifest);
        let server = match &manifest.source {
            ToolSource::Mcp { server_name } => Some(server_name.clone()),
            _ => None,
        };
        let risk_level = manifest.risk_level.clone();
        let action_type = manifest.action_type.clone();
        let capabilities = manifest.capabilities.clone();
        let path = args
            .get("path")
            .or_else(|| args.get("file_path"))
            .or_else(|| args.get("destination"))
            .and_then(Value::as_str)
            .unwrap_or(tool_name);
        let content_value = args
            .get("content")
            .or_else(|| args.get("body"))
            .or_else(|| args.get("data"))
            .cloned()
            .unwrap_or(Value::Null);
        let content_text = content_value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| content_value.to_string());
        let hash = digest(&SHA256, content_text.as_bytes());
        let content_hash: String = hash.as_ref().iter().map(|b| format!("{:02x}", b)).collect();
        let size_bytes = content_text.len();
        if let Err(e) = ensure_external_write_content_size(&content_text) {
            return Some(Err(e));
        }
        let content_preview = external_write_content_preview(&content_text);
        let minimized_arguments =
            minimized_external_write_arguments(args, &content_hash, size_bytes, &content_preview);
        let operation = if !path.is_empty() && std::path::Path::new(path).exists() {
            "overwrite"
        } else {
            "create"
        };

        let mut proposal = AgentProposal::new(
            ProposalType::ExternalWriteAction,
            &format!("{}.{}", source, path),
            serde_json::json!({
                "tool_name": tool_name,
                "tool_id": manifest.id,
                "source": source,
                "server": server,
                "arguments": minimized_arguments,
                "path": path,
                "content": content_text,
                "content_preview": content_preview,
                "content_hash": content_hash,
                "size_bytes": size_bytes,
                "operation": operation,
                "risk_level": risk_level,
                "action_type": action_type,
                "capabilities": capabilities,
                "requires_confirmation": true,
                "hs_policy_id": BUILTIN_POLICY_EXTERNAL_WRITES_PROPOSAL_FIRST,
            }),
            &format!(
                "Agent proposed external write via '{}' ({})",
                tool_name, operation
            ),
            0.9,
            RiskLevel::High,
            ProposalSource::Manual,
        );

        if let Some(ref run_id) = request.source_run_id {
            proposal.run_id = Some(run_id.clone());
        }
        let outcome = match ctx.submit_review_proposal(DurableWriteRequest::from_agent_proposal(
            DurableWriteSource::ToolPermission,
            DurableWriteSubject::ExternalWrite,
            proposal,
            "External write proposal is pending Review Center approval.",
        )) {
            Ok(outcome) => outcome,
            Err(e) => {
                eprintln!(
                    "[warn] Failed to create ExternalWriteAction Proposal for {}: {}",
                    tool_name, e
                );
                return Some(Err(e));
            }
        };

        Some(Ok(outcome.proposal_id().to_string()))
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

fn extract_proposal_id_from_text(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text).ok().and_then(|value| {
        value
            .get("proposal_id")
            .or_else(|| value.get("proposalId"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn manifest_risk_level(manifest: &ToolManifest) -> RiskLevel {
    match manifest.risk_level.trim().to_ascii_lowercase().as_str() {
        "critical" => RiskLevel::Critical,
        "high" => RiskLevel::High,
        "medium" => RiskLevel::Medium,
        _ => RiskLevel::Low,
    }
}
