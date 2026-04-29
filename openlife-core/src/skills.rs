use crate::agent::AgentExecutionBudget;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub required_context: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub execution_budget: AgentExecutionBudget,
    pub output_schema: Value,
    pub proposal_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRunResult {
    pub skill_id: String,
    pub summary: String,
    pub structured_output: Value,
    pub proposal_candidates: Vec<Value>,
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
        registry
    }

    pub fn register(&mut self, manifest: SkillManifest) {
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

    pub fn run_builtin(&self, id: &str, input: Value) -> anyhow::Result<SkillRunResult> {
        let manifest = self
            .manifests
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("unknown skill: {}", id))?;
        let text = input
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let summary = match id {
            "weekly_review" => "已生成周复盘草案和可审阅的状态/目标建议。",
            "goal_breakdown" => "已将目标拆解为里程碑、每日行动和风险提示。",
            "memory_consolidation" => "已生成长期记忆候选和归档建议。",
            _ => "已完成内置技能运行。",
        };
        Ok(SkillRunResult {
            skill_id: manifest.id.clone(),
            summary: summary.to_string(),
            structured_output: serde_json::json!({
                "skill_id": manifest.id,
                "input_text": text,
                "summary": summary,
                "direct_write": false
            }),
            proposal_candidates: vec![serde_json::json!({
                "title": format!("{} 生成的待审阅建议", manifest.name),
                "content": if text.is_empty() { summary.to_string() } else { text },
                "proposal_policy": manifest.proposal_policy,
            })],
        })
    }

    fn weekly_review() -> SkillManifest {
        SkillManifest {
            id: "weekly_review".into(),
            name: "Weekly Review".into(),
            description: "汇总近期 AgentRun、目标、状态和记忆，生成周复盘与改进建议。".into(),
            required_context: vec![
                "agent_runs".into(),
                "goals".into(),
                "state".into(),
                "memory".into(),
            ],
            allowed_tools: vec![],
            execution_budget: AgentExecutionBudget::default(),
            output_schema: serde_json::json!({"type": "object"}),
            proposal_policy: "review_required".into(),
        }
    }

    fn goal_breakdown() -> SkillManifest {
        SkillManifest {
            id: "goal_breakdown".into(),
            name: "Goal Breakdown".into(),
            description: "将长期目标拆解为里程碑、每日行动和风险提示。".into(),
            required_context: vec!["life_model.goals".into(), "state".into()],
            allowed_tools: vec![],
            execution_budget: AgentExecutionBudget::default(),
            output_schema: serde_json::json!({"type": "object"}),
            proposal_policy: "review_required".into(),
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
            output_schema: serde_json::json!({"type": "object"}),
            proposal_policy: "review_required".into(),
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}
