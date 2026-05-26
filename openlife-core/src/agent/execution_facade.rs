use crate::agent::action_executor::{ActionContext, ActionExecutor, ActionExecutorConfig};
use crate::agent::agent_loop::{AgentLoop, AgentLoopConfig, AgentLoopResult, StreamingCallback};
use crate::agent::event_store::AgentRunEventStore;
use crate::agent::runtime::AgentRuntime;
use crate::agent::types::{AgentRun, AgentRunStatus, AgentTask};
use crate::config::AppConfig;
use crate::life_model::LifeModel;
use crate::privacy::PrivacyEngine;
use crate::scheduler::InferenceScheduler;
use anyhow::Result;
use std::sync::Arc;

/// Execution mode governs runtime strategy.
#[derive(Clone)]
pub enum AgentExecutionMode {
    Chat,
    StreamChat {
        callback: Arc<dyn StreamingCallback>,
    },
    Scheduled,
    Proactive,
}

/// Dependencies that live across the entire execution.
pub struct AgentExecutionDeps {
    pub life_model: LifeModel,
    pub tools_prompt: String,
    pub privacy_engine: PrivacyEngine,
    pub privacy_policy: crate::agent::types::PrivacyPolicy,
    pub agent_spec: crate::agent::types::AgentSpec,
    pub prompt_registry: crate::agent::prompt_stack::PromptBlockRegistry,
    pub scheduler: InferenceScheduler,
    pub app_config: AppConfig,
    pub agent_loop_config: AgentLoopConfig,
    pub event_store: Option<AgentRunEventStore>,
}

/// Unified outcome of any agent execution.
#[derive(Debug, Clone)]
pub struct AgentExecutionOutcome {
    pub reply: String,
    pub run: AgentRun,
    pub mode: AgentExecutionModeKind,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
    pub warnings: Vec<String>,
    pub reasoning_trace: crate::agent::ReasoningTrace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentExecutionModeKind {
    Chat,
    StreamChat,
    Scheduled,
    Proactive,
    Fallback,
}

impl std::fmt::Display for AgentExecutionModeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentExecutionModeKind::Chat => write!(f, "chat"),
            AgentExecutionModeKind::StreamChat => write!(f, "stream_chat"),
            AgentExecutionModeKind::Scheduled => write!(f, "scheduled"),
            AgentExecutionModeKind::Proactive => write!(f, "proactive"),
            AgentExecutionModeKind::Fallback => write!(f, "fallback"),
        }
    }
}

/// Main execution facade ensuring convergent runtime semantics.
pub struct AgentExecutionFacade;

impl AgentExecutionFacade {
    /// Run an agent task with consistent runtime semantics.
    ///
    /// `action_ctx` is assembled by the Tauri layer from live stores.
    /// The facade ensures:
    /// - Every execution creates or attaches to an AgentRun.
    /// - Fallback is traceable and attaches to the same AgentRun.
    /// - Streaming and non-streaming share core semantics.
    pub async fn run(
        task: AgentTask,
        mode: AgentExecutionMode,
        deps: AgentExecutionDeps,
        action_ctx: &ActionContext,
    ) -> Result<AgentExecutionOutcome> {
        match mode {
            AgentExecutionMode::Chat => Self::run_chat(task, deps, action_ctx).await,
            AgentExecutionMode::StreamChat { callback } => {
                Self::run_stream_chat(task, deps, action_ctx, callback).await
            }
            AgentExecutionMode::Scheduled => Self::run_scheduled(task, deps, action_ctx).await,
            AgentExecutionMode::Proactive => Self::run_proactive(task, deps, action_ctx).await,
        }
    }

    async fn run_chat(
        task: AgentTask,
        deps: AgentExecutionDeps,
        action_ctx: &ActionContext,
    ) -> Result<AgentExecutionOutcome> {
        let agent_loop = Self::build_agent_loop(&deps);

        match agent_loop
            .run(
                &task,
                &deps.life_model,
                &deps.tools_prompt,
                None,
                deps.privacy_engine.clone(),
                deps.privacy_policy,
                &deps.agent_spec,
                &deps.prompt_registry,
                action_ctx,
            )
            .await
        {
            Ok(result) => Ok(Self::build_outcome(
                result,
                AgentExecutionModeKind::Chat,
                false,
                None,
            )),
            Err(e) => Self::handle_fallback(task, deps, &e.to_string()).await,
        }
    }

    async fn run_stream_chat(
        task: AgentTask,
        deps: AgentExecutionDeps,
        action_ctx: &ActionContext,
        callback: Arc<dyn StreamingCallback>,
    ) -> Result<AgentExecutionOutcome> {
        let agent_loop = Self::build_agent_loop(&deps);

        match agent_loop
            .run_streaming(
                &task,
                &deps.life_model,
                &deps.tools_prompt,
                None,
                deps.privacy_engine.clone(),
                deps.privacy_policy,
                &deps.agent_spec,
                &deps.prompt_registry,
                action_ctx,
                callback,
            )
            .await
        {
            Ok(result) => Ok(Self::build_outcome(
                result,
                AgentExecutionModeKind::StreamChat,
                false,
                None,
            )),
            Err(e) => Self::handle_fallback(task, deps, &e.to_string()).await,
        }
    }

    async fn run_scheduled(
        task: AgentTask,
        deps: AgentExecutionDeps,
        action_ctx: &ActionContext,
    ) -> Result<AgentExecutionOutcome> {
        let agent_loop = Self::build_agent_loop(&deps);

        let result = agent_loop
            .run(
                &task,
                &deps.life_model,
                &deps.tools_prompt,
                None,
                deps.privacy_engine.clone(),
                deps.privacy_policy,
                &deps.agent_spec,
                &deps.prompt_registry,
                action_ctx,
            )
            .await
            .map_err(|e| anyhow::anyhow!("scheduled execution failed: {}", e))?;

        Ok(Self::build_outcome(
            result,
            AgentExecutionModeKind::Scheduled,
            false,
            None,
        ))
    }

    async fn run_proactive(
        task: AgentTask,
        deps: AgentExecutionDeps,
        action_ctx: &ActionContext,
    ) -> Result<AgentExecutionOutcome> {
        let agent_loop = Self::build_agent_loop(&deps);

        let result = agent_loop
            .run(
                &task,
                &deps.life_model,
                &deps.tools_prompt,
                None,
                deps.privacy_engine.clone(),
                deps.privacy_policy,
                &deps.agent_spec,
                &deps.prompt_registry,
                action_ctx,
            )
            .await
            .map_err(|e| anyhow::anyhow!("proactive execution failed: {}", e))?;

        Ok(Self::build_outcome(
            result,
            AgentExecutionModeKind::Proactive,
            false,
            None,
        ))
    }

    async fn handle_fallback(
        task: AgentTask,
        deps: AgentExecutionDeps,
        error_msg: &str,
    ) -> Result<AgentExecutionOutcome> {
        let mut prompt_stack =
            AgentRuntime::prompt_stack_for_spec(&deps.agent_spec, &deps.prompt_registry)
                .map_err(|e| anyhow::anyhow!("fallback prompt stack error: {}", e))?;
        let mut fallback_messages = task.messages.clone();
        let assembled_prompt = prompt_stack.assemble();
        if !assembled_prompt.trim().is_empty() {
            fallback_messages.insert(
                0,
                crate::llm::ChatMessage {
                    role: "system".to_string(),
                    content: assembled_prompt,
                },
            );
        }

        let fallback_reply = deps
            .scheduler
            .generate_governed(
                fallback_messages,
                &deps.life_model,
                if deps.tools_prompt.trim().is_empty() {
                    None
                } else {
                    Some(&deps.tools_prompt)
                },
                deps.privacy_policy,
            )
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "AgentLoop failed: {}. Fallback also failed: {}",
                    error_msg,
                    e
                )
            })?;

        let mut run = AgentRun::new_chat_run(&task.session_id, &task.user_text);
        run.status = AgentRunStatus::Completed;
        run.output_preview = Some(fallback_reply.chars().take(200).collect());
        run.warnings.push(format!("fallback: {}", error_msg));
        run.finished_at = Some(chrono::Utc::now());

        Ok(AgentExecutionOutcome {
            reply: fallback_reply,
            run,
            mode: AgentExecutionModeKind::Fallback,
            fallback_used: true,
            fallback_reason: Some(error_msg.to_string()),
            warnings: vec![format!("AgentLoop failed, used fallback: {}", error_msg)],
            reasoning_trace: crate::agent::ReasoningTrace::default(),
        })
    }

    fn build_agent_loop(deps: &AgentExecutionDeps) -> AgentLoop {
        let runtime = AgentRuntime::new(
            deps.life_model.clone(),
            deps.scheduler.clone(),
            &deps.app_config,
        );
        let action_executor = ActionExecutor::new(ActionExecutorConfig::default());
        let mut agent_loop = AgentLoop::new(
            runtime,
            action_executor,
            deps.scheduler.clone(),
            deps.agent_loop_config.clone(),
        );
        if let Some(ref store) = deps.event_store {
            agent_loop = agent_loop.with_event_store(store.clone());
        }
        agent_loop
    }

    fn build_outcome(
        result: AgentLoopResult,
        mode: AgentExecutionModeKind,
        fallback_used: bool,
        fallback_reason: Option<String>,
    ) -> AgentExecutionOutcome {
        let reasoning_trace = result.run.reasoning_trace.clone().unwrap_or_default();
        AgentExecutionOutcome {
            reply: result.final_response,
            run: result.run,
            mode,
            fallback_used,
            fallback_reason,
            warnings: Vec::new(),
            reasoning_trace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_builder() {
        let run = AgentRun::new_chat_run("test-session", "hello");
        let outcome = AgentExecutionOutcome {
            reply: "Hi!".into(),
            run,
            mode: AgentExecutionModeKind::Chat,
            fallback_used: false,
            fallback_reason: None,
            warnings: vec![],
            reasoning_trace: crate::agent::ReasoningTrace::default(),
        };
        assert_eq!(outcome.mode.to_string(), "chat");
        assert!(!outcome.fallback_used);
    }

    #[test]
    fn test_fallback_outcome_flags() {
        let run = AgentRun::new_chat_run("test-session", "hello");
        let outcome = AgentExecutionOutcome {
            reply: "Fallback reply".into(),
            run,
            mode: AgentExecutionModeKind::Fallback,
            fallback_used: true,
            fallback_reason: Some("AgentLoop timeout".into()),
            warnings: vec!["fallback".into()],
            reasoning_trace: crate::agent::ReasoningTrace::default(),
        };
        assert!(outcome.fallback_used);
        assert_eq!(
            outcome.fallback_reason.as_deref(),
            Some("AgentLoop timeout")
        );
        assert_eq!(outcome.mode.to_string(), "fallback");
    }

    #[test]
    fn core_execution_facade_fallback_uses_governed_generation_boundary() {
        let source = include_str!("execution_facade.rs");
        let start = source
            .find("async fn handle_fallback")
            .expect("core fallback helper should exist");
        let end = source[start..]
            .find("fn build_agent_loop")
            .map(|offset| start + offset)
            .expect("build_agent_loop should follow fallback helper");
        let fallback_source = &source[start..end];

        assert!(
            fallback_source.contains(".generate_governed("),
            "core ExecutionFacade fallback must preserve AgentSpec privacy governance"
        );
        assert!(
            !fallback_source.contains(".generate("),
            "core ExecutionFacade fallback must not call legacy scheduler generation"
        );
    }

    #[test]
    fn test_mode_kinds_display() {
        assert_eq!(AgentExecutionModeKind::Chat.to_string(), "chat");
        assert_eq!(
            AgentExecutionModeKind::StreamChat.to_string(),
            "stream_chat"
        );
        assert_eq!(AgentExecutionModeKind::Scheduled.to_string(), "scheduled");
        assert_eq!(AgentExecutionModeKind::Proactive.to_string(), "proactive");
        assert_eq!(AgentExecutionModeKind::Fallback.to_string(), "fallback");
    }

    #[test]
    fn test_fallback_builds_run_with_warnings() {
        let mut run = AgentRun::new_chat_run("fb-1", "test");
        run.status = AgentRunStatus::Completed;
        run.warnings.push("fallback: test error".to_string());
        let outcome = AgentExecutionOutcome {
            reply: "fb reply".into(),
            run,
            mode: AgentExecutionModeKind::Fallback,
            fallback_used: true,
            fallback_reason: Some("test error".into()),
            warnings: vec!["fallback warning".into()],
            reasoning_trace: crate::agent::ReasoningTrace::default(),
        };
        assert_eq!(outcome.reply, "fb reply");
        assert!(!outcome.warnings.is_empty());
    }
}
