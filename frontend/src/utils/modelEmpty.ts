import type { LifeModel } from "../types";

export function isModelEmpty(model: LifeModel | null): boolean {
  if (!model) return true;
  const hasValues =
    model.identity.values.length > 0 ||
    model.identity.personality_traits.length > 0 ||
    model.identity.mission_statement.length > 0;
  const hasGoals =
    model.goals.short_term.length > 0 ||
    model.goals.medium_term.length > 0 ||
    model.goals.long_term.length > 0 ||
    model.goals.life_goals.length > 0 ||
    model.goals.daily.length > 0;
  const hasSkills =
    model.capabilities.skills.length > 0 ||
    model.capabilities.resources.length > 0 ||
    model.capabilities.networks.length > 0 ||
    model.capabilities.tools.length > 0 ||
    model.capabilities.knowledge_domains.length > 0;
  const hasState =
    model.state.emotional_state.current_mood.length > 0 ||
    model.state.current_focus.length > 0;
  const hasRelationships =
    (model.relationships?.inner_circle?.length ?? 0) > 0 ||
    (model.relationships?.mentors?.length ?? 0) > 0 ||
    (model.relationships?.collaborators?.length ?? 0) > 0;
  return !hasValues && !hasGoals && !hasSkills && !hasState && !hasRelationships;
}
