//! PlanMode — governed read-only exploration that produces a structured AgentPlan.
//!
//! PlanMode enforces that the planner agent can only use read-only and network
//! tools. Write tools, external side effects, and bash/shell are blocked.
//! The planner generates an AgentPlan which is persisted via PlanStore and
//! recorded as a plan.created AgentRunEvent.

use crate::agent::event_store::AgentRunEventStore;
use crate::agent::plan_store::PlanStore;
use crate::agent::prompt_stack::PromptStack;
use crate::agent::types::{
    AgentEventActor, AgentPlan, AgentRunEvent, AgentRunEventType, RiskLevel,
};
use crate::mcp::McpRegistry;
use anyhow::Result;

/// PlanMode configuration: which tools the planner may use.
#[derive(Debug, Clone)]
pub struct PlanModeConfig {
    /// Names of tools the planner is allowed to call.
    pub allowed_tool_names: Vec<String>,
    /// Names of tools that exist but are blocked in plan mode.
    pub blocked_tool_names: Vec<String>,
}

impl PlanModeConfig {
    /// Build a PlanModeConfig by inspecting the McpRegistry.
    ///
    /// Allowed: tools with `action_type == "read"` or `action_type == "network"`.
    /// Blocked: tools with `action_type == "write"` or `action_type == "external_side_effect"`.
    /// Disabled/declarative-only tools are excluded from both lists.
    pub fn from_registry(registry: &McpRegistry) -> Self {
        let manifests = registry.list_manifests();
        let mut allowed = Vec::new();
        let mut blocked = Vec::new();

        for m in &manifests {
            if !m.enabled || m.declarative_only {
                continue;
            }
            if is_plan_mode_read_only(m) {
                allowed.push(m.name.clone());
            } else {
                blocked.push(m.name.clone());
            }
        }

        // Sort for deterministic output
        allowed.sort();
        blocked.sort();

        Self {
            allowed_tool_names: allowed,
            blocked_tool_names: blocked,
        }
    }

    /// Check whether a tool name is in the plan-mode allowlist.
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tool_names.iter().any(|n| n == tool_name)
    }

    /// Check whether a tool name is blocked in plan mode.
    pub fn is_blocked(&self, tool_name: &str) -> bool {
        self.blocked_tool_names.iter().any(|n| n == tool_name)
    }

    /// Produce a tools prompt containing only plan-mode allowed tools.
    pub fn read_only_tools_prompt(&self, registry: &McpRegistry) -> String {
        let manifests = registry.list_manifests();
        let mut lines: Vec<String> = Vec::new();
        for m in &manifests {
            if !m.enabled || m.declarative_only {
                continue;
            }
            if self.is_allowed(&m.name) {
                let params = m
                    .parameters
                    .as_object()
                    .map(|obj| {
                        let props = obj
                            .get("properties")
                            .and_then(|p| p.as_object())
                            .map(|props| {
                                props
                                    .keys()
                                    .map(|k| k.to_string())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        if props.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", props)
                        }
                    })
                    .unwrap_or_default();
                lines.push(format!(
                    "- {}: {}{}",
                    m.name, m.description, params
                ));
            }
        }
        if lines.is_empty() {
            return String::new();
        }
        format!("Available read-only tools for planning:\n{}", lines.join("\n"))
    }
}

/// Determine whether a tool is classified as read-only for PlanMode.
///
/// Read-only tools: `action_type == "read"` or `action_type == "network"`.
/// Write tools, external side effects, and proposal-generation tools are blocked
/// during exploration (they may appear in the plan but cannot be called).
pub fn is_plan_mode_read_only(manifest: &crate::tool_manifest::ToolManifest) -> bool {
    match manifest.action_type.as_str() {
        "read" => true,
        "network" => true,
        // "write" and "external_side_effect" tools are blocked during exploration
        _ => false,
    }
}

/// PlanMode runner — orchestrates read-only exploration and plan persistence.
pub struct PlanModeRunner {
    config: PlanModeConfig,
    plan_store: PlanStore,
    event_store: Option<AgentRunEventStore>,
}

impl PlanModeRunner {
    pub fn new(
        config: PlanModeConfig,
        plan_store: PlanStore,
        event_store: Option<AgentRunEventStore>,
    ) -> Self {
        Self {
            config,
            plan_store,
            event_store,
        }
    }

    /// Build a PromptStack suitable for PlanMode exploration.
    /// Includes the planning prompt block and the AgentPlan output schema.
    pub fn build_planning_prompt_stack(&self) -> PromptStack {
        PromptStack::plan_mode_stack()
    }

    /// Create a plan and persist it, recording a plan.created event if an
    /// event store is attached.
    pub fn create_plan(
        &self,
        goal: &str,
        run_id: Option<&str>,
        session_id: Option<&str>,
        risk_level: RiskLevel,
    ) -> Result<AgentPlan> {
        let mut plan = AgentPlan::new(goal, risk_level);
        if let Some(rid) = run_id {
            plan.run_id = Some(rid.to_string());
        }
        if let Some(sid) = session_id {
            plan.session_id = Some(sid.to_string());
        }

        self.plan_store.create_plan(&plan)?;

        if let Some(ref store) = self.event_store {
            let resolved_run_id = run_id.unwrap_or(&plan.id);
            let event = AgentRunEvent::new(
                resolved_run_id,
                AgentRunEventType::PlanCreated,
                AgentEventActor::Agent,
                format!("Plan created: {}", plan.goal),
                serde_json::json!({
                    "plan_id": plan.id,
                    "goal": plan.goal,
                    "risk_level": plan.risk_level.to_string(),
                    "requires_confirmation": plan.requires_confirmation,
                    "step_count": plan.steps.len(),
                    "tool_intent_count": plan.tool_intents.len(),
                }),
            );
            store.append_event(&event)?;
        }

        Ok(plan)
    }

    /// Persist a fully-populated plan (e.g. from model output) and emit the
    /// plan.created event.
    pub fn save_plan(&self, plan: &AgentPlan) -> Result<()> {
        self.plan_store.create_plan(plan)?;

        if let Some(ref store) = self.event_store {
            let run_id = plan.run_id.as_deref().unwrap_or(&plan.id);
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::PlanCreated,
                AgentEventActor::Agent,
                format!("Plan created: {}", plan.goal),
                serde_json::json!({
                    "plan_id": plan.id,
                    "goal": plan.goal,
                    "risk_level": plan.risk_level.to_string(),
                    "requires_confirmation": plan.requires_confirmation,
                }),
            );
            store.append_event(&event)?;
        }

        Ok(())
    }

    /// Return a reference to the configuration.
    pub fn config(&self) -> &PlanModeConfig {
        &self.config
    }

    /// Return a reference to the plan store.
    pub fn plan_store(&self) -> &PlanStore {
        &self.plan_store
    }

    /// Return a reference to the event store, if any.
    pub fn event_store(&self) -> Option<&AgentRunEventStore> {
        self.event_store.as_ref()
    }
}

/// Classification result for a tool in PlanMode context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanModeToolClass {
    /// Tool is allowed in plan mode (read-only, no mutations).
    Allowed,
    /// Tool is blocked — would mutate state or cause side effects.
    Blocked,
    /// Tool is unavailable (disabled or declarative-only).
    Unavailable,
}

impl PlanModeToolClass {
    /// Classify a tool by name against the given McpRegistry.
    pub fn classify(registry: &McpRegistry, tool_name: &str) -> Self {
        let manifests = registry.list_manifests();
        let manifest = manifests.iter().find(|m| m.name == tool_name);

        match manifest {
            None => PlanModeToolClass::Unavailable,
            Some(m) if !m.enabled || m.declarative_only => PlanModeToolClass::Unavailable,
            Some(m) if is_plan_mode_read_only(m) => PlanModeToolClass::Allowed,
            _ => PlanModeToolClass::Blocked,
        }
    }
}

// ── Plan Confirmation Protocol ─────────────────────────────────────────

/// Result of the plan confirmation check.
#[derive(Debug, Clone)]
pub struct PlanConfirmation {
    /// Whether user confirmation is required before execution.
    pub requires_confirmation: bool,
    /// Human-readable reasons for the confirmation decision.
    pub reasons: Vec<String>,
}

/// Determine whether a plan requires user confirmation before execution.
///
/// Rules (from ADR 0007):
/// - Confirmation required: risk medium/high/critical, any write intent,
///   any handoff sub-agent assignment, plan targets LifeModel or durable memory.
/// - Confirmation NOT required: purely read-only low-risk plan.
pub fn check_confirmation_required(plan: &AgentPlan) -> PlanConfirmation {
    let mut reasons = Vec::new();
    let mut requires = false;

    // Rule 1: risk level medium or higher
    if !matches!(plan.risk_level, RiskLevel::Low) {
        requires = true;
        reasons.push(format!(
            "Risk level is '{}' (medium or higher requires confirmation)",
            plan.risk_level
        ));
    }

    // Rule 2: any write/external side effect intent
    if plan.has_write_intents() {
        requires = true;
        reasons.push("Plan contains write tool intents".into());
    }

    // Rule 3: sub-agent handoff planned
    if plan.has_handoff_assignments() {
        requires = true;
        reasons.push("Plan contains sub-agent handoff assignments".into());
    }

    // Rule 4: plan targets LifeModel or durable memory
    let targets_lifemodel = plan.tool_intents.iter().any(|t| {
        t.tool_name.contains("life_model")
            || t.tool_name.contains("lifemodel")
            || t.tool_name.contains("goal")
    });
    if targets_lifemodel {
        requires = true;
        reasons.push("Plan targets LifeModel fields (goals, identity, etc.)".into());
    }

    let targets_durable_memory = plan
        .tool_intents
        .iter()
        .any(|t| t.tool_name.contains("memory") && t.is_write);
    if targets_durable_memory {
        requires = true;
        reasons.push("Plan targets durable memory writes".into());
    }

    // If no reason to require confirmation, it doesn't need it
    if !requires {
        reasons.push("Plan is low-risk and read-only; no confirmation required".into());
    }

    PlanConfirmation {
        requires_confirmation: requires,
        reasons,
    }
}

/// Record a plan.confirmation_requested event via the event store.
pub fn record_confirmation_requested(
    event_store: &AgentRunEventStore,
    run_id: &str,
    plan: &AgentPlan,
) -> Result<()> {
    let confirmation = check_confirmation_required(plan);
    let event = AgentRunEvent::new(
        run_id,
        AgentRunEventType::PlanConfirmationRequested,
        AgentEventActor::Agent,
        format!(
            "Plan '{}' confirmation {}required",
            plan.goal,
            if confirmation.requires_confirmation {
                ""
            } else {
                "not "
            }
        ),
        serde_json::json!({
            "plan_id": plan.id,
            "requires_confirmation": confirmation.requires_confirmation,
            "reasons": confirmation.reasons,
            "risk_level": plan.risk_level.to_string(),
        }),
    );
    event_store.append_event(&event)?;
    Ok(())
}

impl PlanModeRunner {
    /// Check whether a plan requires confirmation and optionally record
    /// a plan.confirmation_requested event.
    pub fn check_and_record_confirmation(&self, plan: &AgentPlan) -> Result<PlanConfirmation> {
        let confirmation = check_confirmation_required(plan);

        if let Some(ref store) = self.event_store {
            let run_id = plan.run_id.as_deref().unwrap_or(&plan.id);
            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::PlanConfirmationRequested,
                AgentEventActor::Agent,
                format!(
                    "Plan '{}' confirmation {}required",
                    plan.goal,
                    if confirmation.requires_confirmation {
                        ""
                    } else {
                        "not "
                    }
                ),
                serde_json::json!({
                    "plan_id": plan.id,
                    "requires_confirmation": confirmation.requires_confirmation,
                    "reasons": confirmation.reasons,
                    "risk_level": plan.risk_level.to_string(),
                }),
            );
            store.append_event(&event)?;
        }

        Ok(confirmation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::event_store::AgentRunEventStore;
    use crate::agent::plan_store::PlanStore;

    fn registry_with_defaults() -> McpRegistry {
        let mut registry = McpRegistry::new();
        registry.register_default_builtins();
        registry
    }

    #[test]
    fn test_read_only_tools_in_allowlist() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);

        // Core read-only tools must be in allowlist
        let must_be_allowed = [
            "life_model.read",
            "tool.list_available",
            "goal.read",
            "state.read",
            "memory.search",
            "proposal.list",
            "agent_run.lookup",
            "permission.check",
            "permission.request",
            "file.read",
            "calendar.read",
        ];
        for name in &must_be_allowed {
            assert!(
                config.is_allowed(name),
                "expected '{}' to be in plan-mode allowlist",
                name
            );
        }

        // Network tools (read-like) must be in allowlist
        let network_tools = ["web.search", "web.fetch"];
        for name in &network_tools {
            assert!(
                config.is_allowed(name),
                "expected network tool '{}' to be in plan-mode allowlist",
                name
            );
        }
    }

    #[test]
    fn test_write_tools_blocked_in_plan_mode() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);

        // Write tools must be blocked
        let must_be_blocked = [
            "life_model.propose_patch",
            "memory.propose_write",
            "memory.propose_archive",
            "file.write_proposal",
            "calendar.propose_event",
            "email.propose_draft",
            "task.create_proposal",
            "permission.replay_action",
        ];
        for name in &must_be_blocked {
            assert!(
                config.is_blocked(name),
                "expected write tool '{}' to be BLOCKED in plan mode",
                name
            );
            assert!(
                !config.is_allowed(name),
                "write tool '{}' must NOT be in plan-mode allowlist",
                name
            );
        }

        // External side effect tools must also be blocked
        let external_tools = ["a2a.call_agent", "mcp.call_tool"];
        for name in &external_tools {
            assert!(
                config.is_blocked(name),
                "expected external tool '{}' to be BLOCKED in plan mode",
                name
            );
        }
    }

    #[test]
    fn test_declarative_only_tools_excluded() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);

        // Declarative-only stubs should be neither allowed nor blocked
        // (they are filtered out entirely)
        assert!(!config.is_allowed("email.read"));
        assert!(!config.is_blocked("email.read"));
        assert!(!config.is_allowed("snapshot.create"));
        assert!(!config.is_blocked("snapshot.create"));
    }

    #[test]
    fn test_plan_mode_tool_classification() {
        let registry = registry_with_defaults();

        assert_eq!(
            PlanModeToolClass::classify(&registry, "life_model.read"),
            PlanModeToolClass::Allowed
        );
        assert_eq!(
            PlanModeToolClass::classify(&registry, "web.search"),
            PlanModeToolClass::Allowed
        );
        assert_eq!(
            PlanModeToolClass::classify(&registry, "file.write_proposal"),
            PlanModeToolClass::Blocked
        );
        assert_eq!(
            PlanModeToolClass::classify(&registry, "a2a.call_agent"),
            PlanModeToolClass::Blocked
        );
        // Declarative-only
        assert_eq!(
            PlanModeToolClass::classify(&registry, "email.read"),
            PlanModeToolClass::Unavailable
        );
    }

    #[test]
    fn test_read_only_tools_prompt() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let prompt = config.read_only_tools_prompt(&registry);

        // Prompt must include read-only tools
        assert!(prompt.contains("life_model.read"));
        assert!(prompt.contains("web.search"));
        assert!(prompt.contains("file.read"));

        // Prompt must NOT include write tools
        assert!(!prompt.contains("file.write_proposal"));
        assert!(!prompt.contains("a2a.call_agent"));
        assert!(!prompt.contains("permission.replay_action"));

        // Prompt must NOT include declarative-only tools
        assert!(!prompt.contains("email.read"));
        assert!(!prompt.contains("snapshot.create"));
    }

    #[test]
    fn test_plan_mode_config_is_deterministic() {
        let registry = registry_with_defaults();
        let config1 = PlanModeConfig::from_registry(&registry);
        let config2 = PlanModeConfig::from_registry(&registry);

        assert_eq!(config1.allowed_tool_names, config2.allowed_tool_names);
        assert_eq!(config1.blocked_tool_names, config2.blocked_tool_names);
    }

    #[test]
    fn test_runner_builds_planning_prompt_stack() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let plan_store = PlanStore::new_in_memory().unwrap();
        let runner = PlanModeRunner::new(config, plan_store, None);

        let mut stack = runner.build_planning_prompt_stack();
        assert_eq!(stack.blocks.len(), 1);
        assert_eq!(stack.blocks[0].id, "planning_prompt");
        assert!(stack.output_schema.is_some());

        let assembled = stack.assemble();
        assert!(assembled.contains("PlanMode"));
    }

    #[test]
    fn test_runner_create_plan_persists_and_records_event() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let plan_store = PlanStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runner = PlanModeRunner::new(
            config,
            plan_store,
            Some(event_store.clone()),
        );

        let run_id = "test-plan-create-001";
        let plan = runner
            .create_plan(
                "Analyze workspace structure",
                Some(run_id),
                Some("sess-001"),
                RiskLevel::Low,
            )
            .unwrap();

        // Plan is persisted
        let fetched = runner
            .plan_store()
            .get_plan(&plan.id)
            .unwrap()
            .unwrap();
        assert_eq!(fetched.goal, "Analyze workspace structure");
        assert_eq!(fetched.risk_level, RiskLevel::Low);
        assert_eq!(fetched.run_id.as_deref(), Some(run_id));
        assert_eq!(fetched.session_id.as_deref(), Some("sess-001"));

        // plan.created event is recorded
        let events = event_store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::PlanCreated);
        assert_eq!(events[0].actor, AgentEventActor::Agent);
        assert_eq!(
            events[0]
                .payload
                .get("plan_id")
                .unwrap()
                .as_str()
                .unwrap(),
            plan.id
        );
    }

    #[test]
    fn test_runner_save_plan_uses_plan_run_id() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let plan_store = PlanStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let runner = PlanModeRunner::new(config, plan_store, Some(event_store.clone()));

        let mut plan = AgentPlan::new("Test plan from model output", RiskLevel::Medium);
        plan.run_id = Some("external-run-001".to_string());
        plan.session_id = Some("sess-002".to_string());
        plan.publish();

        runner.save_plan(&plan).unwrap();

        // Plan was persisted
        let fetched = runner.plan_store().get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.goal, "Test plan from model output");
        assert!(fetched.requires_confirmation);

        // Event recorded against the plan's run_id
        let events = event_store
            .list_events_by_run("external-run-001")
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::PlanCreated);
    }

    #[test]
    fn test_no_event_recorded_without_event_store() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let plan_store = PlanStore::new_in_memory().unwrap();
        let runner = PlanModeRunner::new(config, plan_store, None);

        let plan = runner
            .create_plan("Quiet plan", None, None, RiskLevel::Low)
            .unwrap();

        // Plan is persisted
        assert!(runner.plan_store().get_plan(&plan.id).unwrap().is_some());

        // No event store → no error, just no event recorded
        assert!(runner.event_store().is_none());
    }

    #[test]
    fn test_plan_mode_blocks_lifemodel_memory_mutation_tools() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);

        // LifeModel mutation tools blocked
        assert!(config.is_blocked("life_model.propose_patch"));

        // Memory mutation tools blocked
        assert!(config.is_blocked("memory.propose_write"));
        assert!(config.is_blocked("memory.propose_archive"));

        // LifeModel/Memory read tools still allowed
        assert!(config.is_allowed("life_model.read"));
        assert!(config.is_allowed("memory.search"));

        // State read allowed, but no direct state write tool exists
        // (state changes are proposals)
        assert!(config.is_allowed("state.read"));
    }

    #[test]
    fn test_planning_prompt_stack_includes_output_schema() {
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let plan_store = PlanStore::new_in_memory().unwrap();
        let runner = PlanModeRunner::new(config, plan_store, None);

        let stack = runner.build_planning_prompt_stack();

        let schema = stack.output_schema.unwrap();
        let plan_obj = schema
            .get("properties")
            .and_then(|p| p.get("plan"))
            .expect("schema must have plan object");

        let required = plan_obj
            .get("required")
            .and_then(|r| r.as_array())
            .expect("plan must have required fields");
        let required_fields: Vec<&str> =
            required.iter().filter_map(|v| v.as_str()).collect();

        assert!(required_fields.contains(&"goal"));
        assert!(required_fields.contains(&"steps"));
        assert!(required_fields.contains(&"risk_level"));
    }

    // ── Confirmation protocol tests ───────────────────────────────────

    #[test]
    fn test_low_risk_read_only_plan_no_confirmation_required() {
        let plan = AgentPlan::new("Simple read query", RiskLevel::Low);
        let confirmation = check_confirmation_required(&plan);
        assert!(!confirmation.requires_confirmation);
        assert_eq!(confirmation.reasons.len(), 1);
        assert!(confirmation.reasons[0].contains("no confirmation required"));
    }

    #[test]
    fn test_medium_risk_plan_requires_confirmation() {
        let plan = AgentPlan::new("Risky operation", RiskLevel::Medium);
        let confirmation = check_confirmation_required(&plan);
        assert!(confirmation.requires_confirmation);
        assert!(confirmation
            .reasons
            .iter()
            .any(|r| r.contains("medium or higher")));
    }

    #[test]
    fn test_high_risk_plan_requires_confirmation() {
        let plan = AgentPlan::new("Very risky", RiskLevel::High);
        let confirmation = check_confirmation_required(&plan);
        assert!(confirmation.requires_confirmation);
    }

    #[test]
    fn test_critical_risk_plan_requires_confirmation() {
        let plan = AgentPlan::new("Critical operation", RiskLevel::Critical);
        let confirmation = check_confirmation_required(&plan);
        assert!(confirmation.requires_confirmation);
    }

    #[test]
    fn test_write_intent_triggers_confirmation() {
        let plan = AgentPlan {
            tool_intents: vec![crate::agent::types::ToolIntent {
                tool_name: "file.write_proposal".into(),
                purpose: "write output file".into(),
                risk_level: RiskLevel::High,
                is_write: true,
                parameters_summary: None,
            }],
            ..AgentPlan::new("Write config", RiskLevel::Low)
        };
        let confirmation = check_confirmation_required(&plan);
        assert!(confirmation.requires_confirmation);
        assert!(confirmation
            .reasons
            .iter()
            .any(|r| r.contains("write tool")));
    }

    #[test]
    fn test_handoff_assignment_triggers_confirmation() {
        let plan = AgentPlan {
            subagent_assignments: vec![crate::agent::types::SubAgentAssignment {
                agent_role: "reviewer".into(),
                task: "review code".into(),
                delegation_mode: "handoff".into(),
            }],
            ..AgentPlan::new("Handoff plan", RiskLevel::Low)
        };
        let confirmation = check_confirmation_required(&plan);
        assert!(confirmation.requires_confirmation);
        assert!(confirmation
            .reasons
            .iter()
            .any(|r| r.contains("handoff")));
    }

    #[test]
    fn test_lifemodel_target_triggers_confirmation() {
        let plan = AgentPlan {
            tool_intents: vec![crate::agent::types::ToolIntent {
                tool_name: "life_model.propose_patch".into(),
                purpose: "update identity".into(),
                risk_level: RiskLevel::Medium,
                is_write: true,
                parameters_summary: None,
            }],
            ..AgentPlan::new("LifeModel update", RiskLevel::Medium)
        };
        let confirmation = check_confirmation_required(&plan);
        assert!(confirmation.requires_confirmation);
        assert!(confirmation
            .reasons
            .iter()
            .any(|r| r.contains("LifeModel")));
    }

    #[test]
    fn test_confirmation_requested_event_recorded() {
        let plan_store = PlanStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let runner = PlanModeRunner::new(config, plan_store, Some(event_store.clone()));

        let run_id = "confirmation-test-run";
        let goal = "High-risk operation";
        let plan = runner
            .create_plan(goal, Some(run_id), None, RiskLevel::High)
            .unwrap();

        // Check and record confirmation
        let confirmation = runner.check_and_record_confirmation(&plan).unwrap();
        assert!(confirmation.requires_confirmation);

        // Verify event sequence: plan.created + plan.confirmation_requested
        let events = event_store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, AgentRunEventType::PlanCreated);
        assert_eq!(
            events[1].event_type,
            AgentRunEventType::PlanConfirmationRequested
        );
        assert!(events[1]
            .payload
            .get("requires_confirmation")
            .unwrap()
            .as_bool()
            .unwrap());
    }

    #[test]
    fn test_low_risk_plan_still_records_event() {
        let plan_store = PlanStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let registry = registry_with_defaults();
        let config = PlanModeConfig::from_registry(&registry);
        let runner = PlanModeRunner::new(config, plan_store, Some(event_store.clone()));

        let run_id = "lowrisk-confirm-run";
        let plan = runner
            .create_plan("Low risk plan", Some(run_id), None, RiskLevel::Low)
            .unwrap();

        // Even low-risk plans emit a confirmation event (not required)
        let confirmation = runner.check_and_record_confirmation(&plan).unwrap();
        assert!(!confirmation.requires_confirmation);

        let events = event_store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[1].event_type,
            AgentRunEventType::PlanConfirmationRequested
        );
        let payload_reasons: Vec<String> = events[1]
            .payload
            .get("reasons")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert!(payload_reasons
            .iter()
            .any(|r| r.contains("no confirmation required")));
    }

    #[test]
    fn test_record_confirmation_requested_standalone() {
        let event_store = AgentRunEventStore::new_in_memory().unwrap();
        let run_id = "standalone-confirm";

        let mut plan = AgentPlan::new("Standalone plan", RiskLevel::Medium);
        plan.run_id = Some(run_id.to_string());

        record_confirmation_requested(&event_store, run_id, &plan).unwrap();

        let events = event_store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_type,
            AgentRunEventType::PlanConfirmationRequested
        );
    }
}
