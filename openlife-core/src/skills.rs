use crate::agent::AgentExecutionBudget;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSourceKind {
    #[default]
    BuiltIn,
    Plugin,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillExecutionStatus {
    #[default]
    ExecutableBuiltIn,
    DisabledDeclarativeOnly,
    ModelOnlyNoTools,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub required_context: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub execution_budget: AgentExecutionBudget,
    pub input_schema: Value,
    pub output_schema: Value,
    pub proposal_policy: String,
    #[serde(default)]
    pub source_kind: SkillSourceKind,
    #[serde(default)]
    pub execution_status: SkillExecutionStatus,
    #[serde(default)]
    pub capability_flags: Vec<String>,
    #[serde(default)]
    pub plugin_id: Option<String>,
}

impl SkillManifest {
    pub fn as_plugin_declarative_only(mut self, plugin_id: &str) -> Self {
        self.source_kind = SkillSourceKind::Plugin;
        self.execution_status = SkillExecutionStatus::DisabledDeclarativeOnly;
        self.plugin_id = Some(plugin_id.to_string());
        self.allowed_tools.clear();
        self.execution_budget.allow_writes = false;
        if !self
            .capability_flags
            .iter()
            .any(|flag| flag == "plugin_declarative_only")
        {
            self.capability_flags.push("plugin_declarative_only".into());
        }
        self
    }
}

pub struct SkillRegistry {
    manifests: HashMap<String, SkillManifest>,
}

impl SkillRegistry {
    pub fn built_in() -> Self {
        let mut registry = Self {
            manifests: HashMap::new(),
        };
        registry.register(Self::weekly_review());
        registry.register(Self::goal_breakdown());
        registry.register(Self::memory_consolidation());
        registry.register(Self::evidence_review());
        registry
    }

    fn register(&mut self, manifest: SkillManifest) {
        self.manifests.insert(manifest.id.clone(), manifest);
    }

    pub fn list(&self) -> Vec<SkillManifest> {
        let mut skills: Vec<_> = self.manifests.values().cloned().collect();
        skills.sort_by(|a, b| a.id.cmp(&b.id));
        skills
    }

    pub fn get(&self, id: &str) -> Option<SkillManifest> {
        self.manifests.get(id).cloned()
    }

    pub fn remove_by_source_prefix(&mut self, prefix: &str) {
        self.manifests.retain(|id, _| !id.starts_with(prefix));
    }

    /// Build a bounded, read-only catalog preview for the existing Main Chat detail surface.
    ///
    /// The historical name is retained for its current consumer. This method is not an execution
    /// seam. Executable built-ins append their same packaged instruction so the detail preview
    /// and runtime context cannot drift; catalog-only skills emit no model instruction.
    pub fn build_system_prompt(&self, id: &str) -> anyhow::Result<String> {
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("unknown skill: {}", id))?;
        let execution_surface = if manifest
            .capability_flags
            .iter()
            .any(|flag| flag == "main_chat_turn_runtime_native")
        {
            "turn_runtime_native"
        } else {
            "catalog_only_unavailable"
        };
        let catalog = format!(
            "OpenLife skill catalog entry\nName: {}\nDescription: {}\nExecution surface: {}\nRequired bounded context IDs: {}",
            manifest.name,
            manifest.description,
            execution_surface,
            manifest.required_context.join(", ")
        );
        Ok(match Self::built_in_runtime_instruction(id) {
            Some(instruction) => format!("{catalog}\n\n{instruction}"),
            None => catalog,
        })
    }

    /// Return the bounded instruction owned by a packaged, executable skill.
    ///
    /// Keeping this instruction in the binary means the installed app does not
    /// depend on the repository working directory to offer or execute the skill.
    pub fn built_in_runtime_instruction(id: &str) -> Option<&'static str> {
        match id {
            "evidence_review" => Some(concat!(
                "Review only evidence supplied in the current bounded Main Chat context. ",
                "Summarize what the evidence proves, what it does not prove, and any blocker or retry state. ",
                "Treat tool availability, policy decisions, and execution receipts as facts only when OpenLife ",
                "provided them in the current turn. Never expose secret-like content and never claim that a ",
                "proposal, blocked action, or unavailable tool completed."
            )),
            _ => None,
        }
    }

    fn weekly_review() -> SkillManifest {
        SkillManifest {
            id: "weekly_review".into(),
            name: "Weekly Review".into(),
            description: "汇总近期 AgentRun、目标、状态和记忆，生成周复盘与改进建议。".into(),
            required_context: vec![
                "agent_runs".into(),
                "life_model.goals".into(),
                "life_model.state".into(),
                "memory".into(),
            ],
            allowed_tools: vec![],
            execution_budget: AgentExecutionBudget::default(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "用户的额外输入或要求"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string"},
                    "structured_output": {"type": "object"},
                    "proposal_candidates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "proposal_type": {"type": "string", "enum": ["state_update", "goal_update"]},
                                "affected_path": {"type": "string"},
                                "after": {"type": "object"},
                                "reason": {"type": "string"},
                                "confidence": {"type": "number"}
                            }
                        }
                    },
                    "warnings": {"type": "array", "items": {"type": "string"}}
                }
            }),
            proposal_policy: "review_required".into(),
            source_kind: SkillSourceKind::BuiltIn,
            execution_status: SkillExecutionStatus::Blocked,
            capability_flags: vec![
                "catalog_only".into(),
                "turn_runtime_contract_missing".into(),
            ],
            plugin_id: None,
        }
    }

    fn goal_breakdown() -> SkillManifest {
        SkillManifest {
            id: "goal_breakdown".into(),
            name: "Goal Breakdown".into(),
            description: "将长期目标拆解为里程碑、每日行动和风险提示。".into(),
            required_context: vec!["life_model.goals".into(), "life_model.state".into()],
            allowed_tools: vec![],
            execution_budget: AgentExecutionBudget::default(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "用户输入的目标描述"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string"},
                    "structured_output": {"type": "object"},
                    "proposal_candidates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "proposal_type": {"type": "string", "enum": ["goal_update"]},
                                "affected_path": {"type": "string"},
                                "after": {"type": "object"},
                                "reason": {"type": "string"},
                                "confidence": {"type": "number"}
                            }
                        }
                    },
                    "warnings": {"type": "array", "items": {"type": "string"}}
                }
            }),
            proposal_policy: "review_required".into(),
            source_kind: SkillSourceKind::BuiltIn,
            execution_status: SkillExecutionStatus::Blocked,
            capability_flags: vec![
                "catalog_only".into(),
                "turn_runtime_contract_missing".into(),
            ],
            plugin_id: None,
        }
    }

    fn memory_consolidation() -> SkillManifest {
        SkillManifest {
            id: "memory_consolidation".into(),
            name: "Memory Consolidation".into(),
            description: "从近期聊天和记忆中生成长期记忆候选与归档建议。".into(),
            required_context: vec!["memory".into(), "chat_history".into()],
            allowed_tools: vec![],
            execution_budget: AgentExecutionBudget::default(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "用户的额外要求"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "summary": {"type": "string"},
                    "structured_output": {"type": "object"},
                    "proposal_candidates": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "proposal_type": {"type": "string", "enum": ["memory_write", "memory_archive"]},
                                "affected_path": {"type": "string"},
                                "after": {"type": "object"},
                                "reason": {"type": "string"},
                                "confidence": {"type": "number"}
                            }
                        }
                    },
                    "warnings": {"type": "array", "items": {"type": "string"}}
                }
            }),
            proposal_policy: "review_required".into(),
            source_kind: SkillSourceKind::BuiltIn,
            execution_status: SkillExecutionStatus::Blocked,
            capability_flags: vec![
                "catalog_only".into(),
                "turn_runtime_contract_missing".into(),
            ],
            plugin_id: None,
        }
    }

    fn evidence_review() -> SkillManifest {
        SkillManifest {
            id: "evidence_review".into(),
            name: "Evidence Review".into(),
            description: "审查当前对话中有界证据，区分已证明事实、未证明边界、阻塞与重试状态。"
                .into(),
            required_context: vec!["current_turn_evidence".into()],
            allowed_tools: vec![],
            execution_budget: AgentExecutionBudget::default(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "需要审查的当前对话证据"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "proved": {"type": "array", "items": {"type": "string"}},
                    "not_proved": {"type": "array", "items": {"type": "string"}},
                    "blockers": {"type": "array", "items": {"type": "string"}}
                }
            }),
            proposal_policy: "no_writes".into(),
            source_kind: SkillSourceKind::BuiltIn,
            execution_status: SkillExecutionStatus::ExecutableBuiltIn,
            capability_flags: vec!["main_chat_turn_runtime_native".into(), "read_only".into()],
            plugin_id: None,
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_registered() {
        let registry = SkillRegistry::built_in();
        let skills = registry.list();
        assert_eq!(skills.len(), 4);
        assert!(registry.get("weekly_review").is_some());
        assert!(registry.get("goal_breakdown").is_some());
        assert!(registry.get("memory_consolidation").is_some());
        assert!(registry.get("evidence_review").is_some());
    }

    #[test]
    fn test_build_system_prompt_weekly_review() {
        let registry = SkillRegistry::built_in();
        let prompt = registry.build_system_prompt("weekly_review").unwrap();
        assert!(prompt.contains("Weekly Review"));
        assert!(prompt.contains("catalog_only_unavailable"));
        assert!(!prompt.contains("proposal_candidates"));
        assert!(!prompt.contains("你必须"));
    }

    #[test]
    fn test_build_system_prompt_goal_breakdown() {
        let registry = SkillRegistry::built_in();
        let prompt = registry.build_system_prompt("goal_breakdown").unwrap();
        assert!(prompt.contains("Goal Breakdown"));
    }

    #[test]
    fn test_build_system_prompt_unknown_skill() {
        let registry = SkillRegistry::built_in();
        let result = registry.build_system_prompt("unknown_skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_skill_manifest_fields() {
        let registry = SkillRegistry::built_in();
        let weekly = registry.get("weekly_review").unwrap();
        assert_eq!(weekly.id, "weekly_review");
        assert_eq!(weekly.name, "Weekly Review");
        assert!(!weekly.description.is_empty());
        assert_eq!(weekly.proposal_policy, "review_required");
        assert!(!weekly.input_schema.is_null());
        assert!(!weekly.output_schema.is_null());
    }

    #[test]
    fn built_in_catalog_does_not_claim_turn_runtime_execution() {
        let registry = SkillRegistry::built_in();

        for manifest in registry
            .list()
            .into_iter()
            .filter(|manifest| manifest.id != "evidence_review")
        {
            assert_eq!(manifest.source_kind, SkillSourceKind::BuiltIn);
            assert_eq!(manifest.execution_status, SkillExecutionStatus::Blocked);
            assert!(!manifest.execution_budget.allow_writes);
            assert!(manifest
                .capability_flags
                .iter()
                .any(|flag| flag == "catalog_only"));
            assert!(manifest
                .capability_flags
                .iter()
                .any(|flag| flag == "turn_runtime_contract_missing"));
            assert!(!manifest
                .capability_flags
                .iter()
                .any(|flag| flag == "main_chat_turn_runtime_native"));
        }
    }

    #[test]
    fn evidence_review_is_packaged_read_only_runtime_instruction() {
        let registry = SkillRegistry::built_in();
        let manifest = registry.get("evidence_review").expect("evidence review");
        assert_eq!(
            manifest.execution_status,
            SkillExecutionStatus::ExecutableBuiltIn
        );
        assert!(!manifest.execution_budget.allow_writes);
        assert!(manifest.allowed_tools.is_empty());
        assert!(manifest
            .capability_flags
            .iter()
            .any(|flag| flag == "main_chat_turn_runtime_native"));
        let prompt = registry
            .build_system_prompt("evidence_review")
            .expect("runtime prompt");
        assert!(prompt.contains("Review only evidence supplied"));
        assert!(prompt.contains("never claim"));
    }
}
