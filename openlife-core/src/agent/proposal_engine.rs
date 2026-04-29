use crate::agent::types::{AgentProposal, AgentRun, ProposalSource};
use crate::life_model::LifeModel;
use anyhow::Result;

/// Trait for generating proposals from agent runs.
pub trait ProposalGenerator: Send + Sync {
    fn name(&self) -> &'static str;
    fn source(&self) -> ProposalSource;

    /// Generate proposals from an agent run output.
    ///
    /// # Arguments
    /// * `run` - The agent run that produced the output
    /// * `output` - The output text to analyze
    /// * `life_model` - Current life model for context
    fn generate(
        &self,
        run: &AgentRun,
        output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>>;
}

/// Engine that manages multiple proposal generators.
pub struct ProposalEngine {
    generators: Vec<Box<dyn ProposalGenerator>>,
}

impl Default for ProposalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ProposalEngine {
    pub fn new() -> Self {
        Self {
            generators: Vec::new(),
        }
    }

    /// Register a proposal generator.
    pub fn register(&mut self, generator: Box<dyn ProposalGenerator>) {
        self.generators.push(generator);
    }

    /// Generate proposals from all registered generators.
    pub fn generate_from_run(
        &self,
        run: &AgentRun,
        output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let mut all_proposals = Vec::new();

        for generator in &self.generators {
            match generator.generate(run, output, life_model) {
                Ok(proposals) => {
                    for mut proposal in proposals {
                        // Set run_id and source
                        proposal.run_id = Some(run.id.clone());
                        proposal.source = generator.source();
                        all_proposals.push(proposal);
                    }
                }
                Err(e) => {
                    log::warn!("Proposal generator '{}' failed: {}", generator.name(), e);
                }
            }
        }

        Ok(all_proposals)
    }
}

/// Builder proposal generator.
pub struct BuilderProposalGenerator;

impl ProposalGenerator for BuilderProposalGenerator {
    fn name(&self) -> &'static str {
        "builder"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::BuilderReview
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // Builder proposals are created directly in builder.rs
        // This generator is a placeholder for future automatic proposal generation
        Ok(Vec::new())
    }
}

/// Calibration proposal generator.
pub struct CalibrationProposalGenerator;

impl ProposalGenerator for CalibrationProposalGenerator {
    fn name(&self) -> &'static str {
        "calibration"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::CalibrationRun
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // Calibration proposals are created directly in calibration.rs
        Ok(Vec::new())
    }
}

/// Feedback proposal generator.
pub struct FeedbackProposalGenerator;

impl ProposalGenerator for FeedbackProposalGenerator {
    fn name(&self) -> &'static str {
        "feedback"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::FeedbackEvolution
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // TODO: Implement feedback-based proposal generation
        Ok(Vec::new())
    }
}

/// Memory governance proposal generator.
pub struct MemoryProposalGenerator;

impl ProposalGenerator for MemoryProposalGenerator {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::MemoryGovernance
    }

    fn generate(
        &self,
        _run: &AgentRun,
        _output: &str,
        _life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        // TODO: Implement memory-based proposal generation
        Ok(Vec::new())
    }
}

/// Chat proposal generator adapter that wraps proposal_generators::ChatProposalGenerator.
pub struct ChatProposalGeneratorAdapter {
    inner: crate::agent::proposal_generators::ChatProposalGenerator,
}

impl Default for ChatProposalGeneratorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatProposalGeneratorAdapter {
    pub fn new() -> Self {
        Self {
            inner: crate::agent::proposal_generators::ChatProposalGenerator::default(),
        }
    }
}

impl ProposalGenerator for ChatProposalGeneratorAdapter {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn source(&self) -> ProposalSource {
        ProposalSource::ProactiveAgent
    }

    fn generate(
        &self,
        run: &AgentRun,
        _output: &str,
        life_model: &LifeModel,
    ) -> Result<Vec<AgentProposal>> {
        let session_id = run.session_id.as_deref().unwrap_or("unknown");
        let user_input = run.user_input.as_deref().unwrap_or("");
        self.inner
            .generate_proposals(session_id, user_input, life_model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proposal_engine_registration() {
        let mut engine = ProposalEngine::new();
        engine.register(Box::new(BuilderProposalGenerator));
        engine.register(Box::new(CalibrationProposalGenerator));
        engine.register(Box::new(ChatProposalGeneratorAdapter::new()));

        assert_eq!(engine.generators.len(), 3);
    }

    #[test]
    fn test_builder_generator_source() {
        let gen = BuilderProposalGenerator;
        assert_eq!(gen.name(), "builder");
        assert_eq!(gen.source(), ProposalSource::BuilderReview);
    }

    #[test]
    fn test_chat_adapter_name() {
        let gen = ChatProposalGeneratorAdapter::new();
        assert_eq!(gen.name(), "chat");
        assert_eq!(gen.source(), ProposalSource::ProactiveAgent);
    }
}
