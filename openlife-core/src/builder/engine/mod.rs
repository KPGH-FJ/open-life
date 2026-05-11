pub mod generation;
pub mod step_runner;

#[cfg(test)]
mod tests;

use crate::builder::types::*;
use crate::life_model::LifeModel;

pub struct BuilderEngine<'a> {
    pub(crate) scheduler: &'a crate::scheduler::InferenceScheduler,
}

impl<'a> BuilderEngine<'a> {
    pub fn new(scheduler: &'a crate::scheduler::InferenceScheduler) -> Self {
        Self { scheduler }
    }

    pub async fn next_prompt(
        &self,
        session: &mut BuilderSession,
        user_reply: &str,
        current_model: &LifeModel,
    ) -> (String, Option<LifeModel>) {
        let result = match session.mode {
            BuilderMode::Quick => {
                self.quick_build_step(session, user_reply, current_model)
                    .await
            }
            BuilderMode::Incremental => {
                self.incremental_prompt(session, user_reply, current_model)
                    .await
            }
            BuilderMode::Socratic => self.socratic_step(session, user_reply, current_model).await,
        };
        session.current_prompt = result.0.clone();
        let draft_model = result.1.as_ref().unwrap_or(current_model);
        session.analysis = Some(BuilderAnalysis {
            completion: draft_model.calculate_4d_completion(),
            gaps: Self::detect_gaps(draft_model),
        });
        result
    }

    pub fn build_analysis(current_model: &LifeModel) -> BuilderAnalysis {
        BuilderAnalysis {
            completion: current_model.calculate_4d_completion(),
            gaps: Self::detect_gaps(current_model),
        }
    }
}
