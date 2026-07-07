use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ========================================
// A2A Protocol Types (based on Google A2A draft)
// ========================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub provider: Option<AgentProvider>,
    pub version: String,
    pub documentation_url: Option<String>,
    pub capabilities: AgentCapabilities,
    pub authentication: Option<AgentAuthentication>,
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
    pub skills: Vec<AgentSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProvider {
    pub organization: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    pub streaming: bool,
    pub push_notifications: bool,
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuthentication {
    pub schemes: Vec<String>,
    pub credentials: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub examples: Option<Vec<String>>,
    pub input_modes: Option<Vec<String>>,
    pub output_modes: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub status: Option<TaskStatus>,
    pub history: Option<Vec<Message>>,
    pub artifacts: Option<Vec<Artifact>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatus {
    pub state: TaskState,
    pub message: Option<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Cancelled,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String, // "user" | "agent"
    pub parts: Vec<Part>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Part {
    Text { text: String },
    File { file: FileData },
    Data { data: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileData {
    pub name: Option<String>,
    pub mime_type: Option<String>,
    pub bytes: Option<String>, // base64
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parts: Vec<Part>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub index: Option<u32>,
    pub append: Option<bool>,
    pub last_chunk: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTaskRequest {
    pub id: String,
    pub session_id: Option<String>,
    pub message: Message,
    pub accepted_output_modes: Option<Vec<String>>,
    pub push_notification: Option<PushNotificationConfig>,
    pub history_length: Option<i32>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationConfig {
    pub url: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendTaskResponse {
    pub id: String,
    pub status: TaskStatus,
    pub artifacts: Option<Vec<Artifact>>,
    pub history: Option<Vec<Message>>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2AErrorResponse {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// ========================================
// A2A Client
// ========================================

pub struct A2AClient {
    http: reqwest::Client,
}

impl A2AClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn discover_agent_card(&self, base_url: &str) -> anyhow::Result<AgentCard> {
        let url = format!("{}/.well-known/agent.json", base_url.trim_end_matches('/'));
        let res = self.http.get(&url).send().await?;
        if !res.status().is_success() {
            anyhow::bail!("Failed to fetch agent card: {}", res.status());
        }
        let card: AgentCard = res.json().await?;
        Ok(card)
    }

    pub async fn send_task(
        &self,
        base_url: &str,
        req: &SendTaskRequest,
    ) -> anyhow::Result<SendTaskResponse> {
        let url = format!("{}/tasks/send", base_url.trim_end_matches('/'));
        let res = self.http.post(&url).json(req).send().await?;
        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("A2A task failed: {}", text);
        }
        let resp: SendTaskResponse = res.json().await?;
        Ok(resp)
    }

    /// Convenience: build a text-only task request
    pub fn build_text_task(session_id: Option<String>, text: &str) -> SendTaskRequest {
        SendTaskRequest {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            message: Message {
                role: "user".into(),
                parts: vec![Part::Text { text: text.into() }],
                metadata: None,
            },
            accepted_output_modes: Some(vec!["text".into()]),
            push_notification: None,
            history_length: None,
            metadata: None,
        }
    }
}

impl Default for A2AClient {
    fn default() -> Self {
        Self::new()
    }
}

// ========================================
// A2A Server (in-process handlers, no HTTP server yet)
// ========================================

use crate::agent::context_assembler::AssembleOutput;
use crate::agent::types::{AgentTaskKind, ContextSummary, RedactionLevel};
use crate::agent::{LayeredReasoner, ReasoningInput, ReasoningStrategy, ReasoningTrace};
use crate::life_model::LifeModel;
use crate::llm::ChatMessage;
use crate::privacy::PrivacyEngine;

pub struct A2AServerHandler {
    pub life_model: LifeModel,
    pub privacy_engine: PrivacyEngine,
}

impl A2AServerHandler {
    pub fn default_agent_card(port: u16, model: &LifeModel) -> AgentCard {
        let value_names: Vec<String> = model
            .identity
            .values
            .iter()
            .map(|v| v.name.clone())
            .collect();
        let goal_count = model.goals.short_term.len()
            + model.goals.medium_term.len()
            + model.goals.long_term.len()
            + model.goals.life_goals.len();
        let skill_names: Vec<String> = model
            .capabilities
            .skills
            .iter()
            .map(|s| s.name.clone())
            .collect();

        AgentCard {
            name: "OpenLife".into(),
            description: format!(
                "Your lifelong growth partner. Current model: {} values, {} goals, {} skills.",
                value_names.len(),
                goal_count,
                skill_names.len()
            ),
            url: format!("http://127.0.0.1:{}", port),
            provider: Some(AgentProvider {
                organization: "OpenLife".into(),
                url: "https://openlife.app".into(),
            }),
            version: "0.1.0".into(),
            documentation_url: None,
            capabilities: AgentCapabilities {
                streaming: false,
                push_notifications: false,
                state_transition_history: false,
            },
            authentication: None,
            default_input_modes: vec!["text".into()],
            default_output_modes: vec!["text".into()],
            skills: vec![
                AgentSkill {
                    id: "openlife.query_life_model".into(),
                    name: "Query Life Model".into(),
                    description: format!(
                        "Returns a sanitized JSON view of the user's Life Model (values: {:?}, goals: {}, skills: {:?})",
                        value_names, goal_count, skill_names
                    ),
                    tags: vec!["lifemodel".into()],
                    examples: None,
                    input_modes: None,
                    output_modes: None,
                },
                AgentSkill {
                    id: "openlife.assess_values".into(),
                    name: "Assess Values".into(),
                    description: format!(
                        "Assesses alignment with the user's core values: {:?}",
                        value_names
                    ),
                    tags: vec!["values".into()],
                    examples: None,
                    input_modes: None,
                    output_modes: None,
                },
                AgentSkill {
                    id: "openlife.reasoning_bridge".into(),
                    name: "Reasoning Bridge".into(),
                    description: "Runs the Layered Reasoning Meaning→Strategy→Generation pipeline and returns a structured trace".into(),
                    tags: vec!["reasoning".into(), "decision".into()],
                    examples: None,
                    input_modes: None,
                    output_modes: None,
                },
            ],
        }
    }

    pub fn handle_task(&self, req: SendTaskRequest) -> SendTaskResponse {
        let skill_hint = req
            .metadata
            .as_ref()
            .and_then(|m| m.get("skill"))
            .and_then(|v| v.as_str())
            .unwrap_or("openlife.query_life_model");

        let result = match skill_hint {
            "openlife.assess_values" => self.assess_values(&req),
            "openlife.reasoning_bridge" => self.reasoning_bridge(&req),
            _ => self.query_life_model(),
        };

        SendTaskResponse {
            id: req.id,
            status: TaskStatus {
                state: TaskState::Completed,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text {
                        text: result.clone(),
                    }],
                    metadata: None,
                }),
            },
            artifacts: Some(vec![Artifact {
                name: Some("result".into()),
                description: None,
                parts: vec![Part::Text { text: result }],
                metadata: None,
                index: None,
                append: None,
                last_chunk: Some(true),
            }]),
            history: None,
            metadata: None,
        }
    }

    fn query_life_model(&self) -> String {
        let summary = serde_json::json!({
            "identity": {
                "name": self.life_model.identity.name,
                "values": self.life_model.identity.values.iter().map(|v| serde_json::json!({
                    "name": v.name,
                    "weight": v.weight,
                })).collect::<Vec<_>>(),
                "personality_traits": self.life_model.identity.personality_traits.iter().map(|t| &t.trait_name).collect::<Vec<_>>(),
                "life_philosophy": self.life_model.identity.life_philosophy,
            },
            "goals": {
                "short_term": self.life_model.goals.short_term.iter().map(|g| &g.name).collect::<Vec<_>>(),
                "medium_term": self.life_model.goals.medium_term.iter().map(|g| &g.name).collect::<Vec<_>>(),
                "long_term": self.life_model.goals.long_term.iter().map(|g| &g.name).collect::<Vec<_>>(),
                "life_goals": self.life_model.goals.life_goals.iter().map(|g| &g.name).collect::<Vec<_>>(),
            },
            "capabilities": {
                "skills": self.life_model.capabilities.skills.iter().map(|s| &s.name).collect::<Vec<_>>(),
                "resources": self.life_model.capabilities.resources.iter().map(|r| &r.name).collect::<Vec<_>>(),
                "networks": self.life_model.capabilities.networks,
            },
            "state": {
                "current_focus": self.life_model.state.current_focus,
                "emotional_state": {
                    "current_mood": self.life_model.state.emotional_state.current_mood,
                    "stress_level": self.life_model.state.emotional_state.stress_level,
                    "fulfillment_score": self.life_model.state.emotional_state.fulfillment_score,
                },
                "health_status": {
                    "physical": self.life_model.state.health_status.physical,
                    "mental": self.life_model.state.health_status.mental,
                    "energy_level": self.life_model.state.health_status.energy_level,
                },
            },
        });
        let json_str = serde_json::to_string_pretty(&summary).unwrap_or_default();
        let (sanitized, _) = self.privacy_engine.desensitize(&json_str);
        sanitized
    }

    fn assess_values(&self, req: &SendTaskRequest) -> String {
        let user_text = extract_text_from_message(&req.message);
        let issues = self.privacy_engine.detect(&user_text);
        let filtered = self.privacy_engine.desensitize(&user_text);
        let passes = crate::core_value_signal_extractor::contains_core_value_signal(&filtered.0);

        // Desensitize values and goal names so external agents never see raw PII
        let raw_values = self
            .life_model
            .identity
            .values
            .iter()
            .map(|v| v.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let (desensitized_values, _) = self.privacy_engine.desensitize(&raw_values);
        let raw_goals = self
            .life_model
            .goals
            .short_term
            .iter()
            .chain(self.life_model.goals.medium_term.iter())
            .chain(self.life_model.goals.long_term.iter())
            .chain(self.life_model.goals.life_goals.iter())
            .map(|g| g.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let (desensitized_goals, _) = self.privacy_engine.desensitize(&raw_goals);

        serde_json::to_string_pretty(&serde_json::json!({
            "core_values": desensitized_values,
            "goals_summary": desensitized_goals,
            "privacy_issues_detected": issues.len(),
            "privacy_issue_types": issues.iter().map(|(t, _)| format!("{:?}", t)).collect::<Vec<_>>(),
            "values_alignment_passed": passes,
            "desensitized_input": filtered.0,
        })).unwrap_or_default()
    }

    fn reasoning_bridge(&self, req: &SendTaskRequest) -> String {
        let user_text = extract_text_from_message(&req.message);
        let session_id = req
            .session_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let reasoning_input = ReasoningInput {
            task_kind: AgentTaskKind::Conversation,
            user_text: user_text.clone(),
            session_id: session_id.clone(),
        };

        let assemble_output = AssembleOutput {
            life_model: std::sync::Arc::new(self.life_model.clone()),
            tools_prompt: String::new(),
            privacy_map: HashMap::new(),
            desensitized_messages: std::sync::Arc::new(vec![ChatMessage {
                role: "user".to_string(),
                content: user_text,
            }]),
            memory_context: String::new(),
            context_summary: ContextSummary {
                life_model_empty: self.life_model.is_effectively_empty(),
                included_life_model_sections: vec![],
                memory_hit_count: 0,
                memory_sources: vec![],
                used_tools_prompt: false,
                redaction_applied: false,
                redaction_level: RedactionLevel::None,
            },
            embed_error: None,
        };

        let run_id = uuid::Uuid::new_v4().to_string();
        let life_model_clone = self.life_model.clone();

        let rt = tokio::runtime::Handle::try_current();
        let trace: ReasoningTrace = match rt {
            Ok(handle) => handle.block_on(async move {
                let scheduler = crate::scheduler::InferenceScheduler::default();
                let reasoner = LayeredReasoner::new(scheduler, life_model_clone);
                match reasoner
                    .reason(&reasoning_input, &assemble_output, &run_id)
                    .await
                {
                    Ok(output) => output.trace,
                    Err(e) => {
                        let mut trace = ReasoningTrace::default();
                        trace.errors.push(e.to_string());
                        trace
                    }
                }
            }),
            Err(_) => {
                let new_rt = tokio::runtime::Runtime::new().unwrap();
                new_rt.block_on(async move {
                    let scheduler = crate::scheduler::InferenceScheduler::default();
                    let reasoner = LayeredReasoner::new(scheduler, life_model_clone);
                    match reasoner
                        .reason(&reasoning_input, &assemble_output, &run_id)
                        .await
                    {
                        Ok(output) => output.trace,
                        Err(e) => {
                            let mut trace = ReasoningTrace::default();
                            trace.errors.push(e.to_string());
                            trace
                        }
                    }
                })
            }
        };
        serde_json::to_string_pretty(&trace).unwrap_or_default()
    }
}

fn extract_text_from_message(msg: &Message) -> String {
    msg.parts
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ========================================
// Reasoning <-> A2A Bridge helpers
// ========================================

pub fn reasoning_input_to_a2a_task(
    req: &ReasoningInput,
    skill: Option<&str>,
    tool_calls: Option<&serde_json::Value>,
) -> SendTaskRequest {
    let text = &req.user_text;
    let mut task = A2AClient::build_text_task(Some(req.session_id.clone()), text);
    if let Some(skill) = skill {
        task.metadata = Some({
            let mut m = HashMap::new();
            m.insert(
                "skill".to_string(),
                serde_json::Value::String(skill.to_string()),
            );
            m
        });
    }
    if let Some(tools) = tool_calls {
        task.metadata
            .get_or_insert_with(HashMap::new)
            .insert("tool_calls".to_string(), tools.clone());
    }
    task
}

pub fn a2a_response_to_reasoning_result(
    resp: &SendTaskResponse,
) -> Result<serde_json::Value, String> {
    // Aggregate text from artifacts (primary) and status message (fallback)
    let artifact_text: String = resp
        .artifacts
        .as_ref()
        .map(|arts| {
            arts.iter()
                .flat_map(|art| &art.parts)
                .filter_map(|part| match part {
                    Part::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();

    let status_text = resp
        .status
        .message
        .as_ref()
        .and_then(|m| m.parts.first())
        .and_then(|part| match part {
            Part::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let text = if artifact_text.is_empty() {
        status_text
    } else {
        artifact_text
    };

    // Extract any structured metadata that the A2A handler may have returned
    let metadata = resp.metadata.clone().unwrap_or_default();

    // Build a reasoning-compatible result object
    let result = serde_json::json!({
        "text": text,
        "state": resp.status.state,
        "status": {
            "state": resp.status.state,
            "message": resp.status.message.as_ref().and_then(|m| m.parts.first()).and_then(|part| match part {
                Part::Text { text } => Some(text.clone()),
                _ => None,
            }),
        },
        "metadata": metadata,
    });

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentTaskKind, ReasoningInput};

    #[test]
    fn build_text_task_basic() {
        let task = A2AClient::build_text_task(Some("sid-123".into()), "hello");
        assert_eq!(task.session_id, Some("sid-123".into()));
        assert_eq!(task.message.role, "user");
        let text = extract_text_from_message(&task.message);
        assert_eq!(text, "hello");
    }

    #[test]
    fn reasoning_input_to_a2a_task_maps_text_and_metadata() {
        let req = ReasoningInput {
            task_kind: AgentTaskKind::Conversation,
            user_text: "do something".to_string(),
            session_id: "sess-42".to_string(),
        };
        let task = reasoning_input_to_a2a_task(
            &req,
            Some("coding"),
            Some(&serde_json::json!([{"name": "tool1"}])),
        );
        assert_eq!(task.session_id, Some("sess-42".into()));
        let meta = task.metadata.as_ref().unwrap();
        assert_eq!(meta.get("skill").unwrap().as_str().unwrap(), "coding");
        assert!(meta.get("tool_calls").is_some());
    }

    #[test]
    fn a2a_response_to_reasoning_result_prefers_artifacts() {
        let resp = SendTaskResponse {
            id: "task-1".into(),
            status: TaskStatus {
                state: TaskState::Completed,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text {
                        text: "status text".into(),
                    }],
                    metadata: None,
                }),
            },
            artifacts: Some(vec![Artifact {
                name: Some("result".into()),
                description: None,
                parts: vec![Part::Text {
                    text: "artifact text".into(),
                }],
                metadata: None,
                index: None,
                append: None,
                last_chunk: None,
            }]),
            history: None,
            metadata: Some({
                let mut m = HashMap::new();
                m.insert("key".into(), serde_json::Value::String("value".into()));
                m
            }),
        };
        let result = a2a_response_to_reasoning_result(&resp).unwrap();
        assert_eq!(result["text"].as_str().unwrap(), "artifact text");
        assert_eq!(result["metadata"]["key"].as_str().unwrap(), "value");
    }

    #[test]
    fn a2a_response_to_reasoning_result_fallback_to_status() {
        let resp = SendTaskResponse {
            id: "task-2".into(),
            status: TaskStatus {
                state: TaskState::Working,
                message: Some(Message {
                    role: "agent".into(),
                    parts: vec![Part::Text {
                        text: "working on it".into(),
                    }],
                    metadata: None,
                }),
            },
            artifacts: None,
            history: None,
            metadata: None,
        };
        let result = a2a_response_to_reasoning_result(&resp).unwrap();
        assert_eq!(result["text"].as_str().unwrap(), "working on it");
    }

    #[test]
    fn extract_text_from_message_joins_parts() {
        let msg = Message {
            role: "user".into(),
            parts: vec![
                Part::Text {
                    text: "hello".into(),
                },
                Part::Text {
                    text: "world".into(),
                },
            ],
            metadata: None,
        };
        assert_eq!(extract_text_from_message(&msg), "hello world");
    }
}
