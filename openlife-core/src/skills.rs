use crate::agent::AgentExecutionBudget;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
    pub execution_status: SkillExecutionStatus,
    #[serde(default)]
    pub capability_flags: Vec<String>,
}

pub struct SkillRegistry {
    manifests: HashMap<String, SkillManifest>,
}

impl SkillRegistry {
    pub fn built_in() -> Self {
        let mut registry = Self {
            manifests: HashMap::new(),
        };
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
            .any(|flag| flag == "canonical_chat_work_native")
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
            execution_status: SkillExecutionStatus::ExecutableBuiltIn,
            capability_flags: vec!["canonical_chat_work_native".into(), "read_only".into()],
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
        assert_eq!(skills.len(), 1);
        assert!(registry.get("evidence_review").is_some());
    }

    #[test]
    fn test_build_system_prompt_unknown_skill() {
        let registry = SkillRegistry::built_in();
        let result = registry.build_system_prompt("unknown_skill");
        assert!(result.is_err());
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
            .any(|flag| flag == "canonical_chat_work_native"));
        let prompt = registry
            .build_system_prompt("evidence_review")
            .expect("runtime prompt");
        assert!(prompt.contains("Review only evidence supplied"));
        assert!(prompt.contains("never claim"));
    }
}
