import type { LifeModel } from "../types";
import type { SystemDiagnostics } from "../tauri";

export function isModelEmpty(model: LifeModel | null): boolean {
  if (!model) return true;
  const emptyOrDefault = (value: string | undefined, defaults: string[]) =>
    !value || defaults.includes(value);
  return (
    !model.identity.name.trim() &&
    !model.identity.birth_date &&
    model.identity.values.length === 0 &&
    model.identity.personality_traits.length === 0 &&
    !model.identity.life_philosophy.trim() &&
    !model.identity.mission_statement.trim() &&
    !model.identity.role_definition.primary_role.trim() &&
    model.identity.role_definition.secondary_roles.length === 0 &&
    model.identity.role_definition.responsibilities.length === 0 &&
    model.identity.role_definition.boundaries.length === 0 &&
    model.identity.voice_style.tone_descriptors.length === 0 &&
    !model.identity.voice_style.vocabulary_preference.trim() &&
    model.goals.short_term.length === 0 &&
    model.goals.medium_term.length === 0 &&
    model.goals.long_term.length === 0 &&
    model.goals.life_goals.length === 0 &&
    model.goals.daily.length === 0 &&
    model.capabilities.skills.length === 0 &&
    model.capabilities.resources.length === 0 &&
    model.capabilities.networks.length === 0 &&
    model.capabilities.tools.length === 0 &&
    model.capabilities.knowledge_domains.length === 0 &&
    emptyOrDefault(model.state.current_focus, ["构建人生模型"]) &&
    emptyOrDefault(model.state.health_status.physical, ["良好"]) &&
    emptyOrDefault(model.state.health_status.mental, ["积极"]) &&
    [0, 5, 7].includes(model.state.health_status.energy_level) &&
    emptyOrDefault(model.state.emotional_state.current_mood, ["期待"]) &&
    [0, 3].includes(model.state.emotional_state.stress_level) &&
    [0, 5, 6].includes(model.state.emotional_state.fulfillment_score) &&
    model.state.recent_reflections.length === 0 &&
    model.state.open_questions.length === 0 &&
    model.state.focus_areas.length === 0 &&
    model.state.recent_events.length === 0 &&
    model.state.habit_streaks.length === 0 &&
    model.state.custom_dimensions.length === 0 &&
    model.state.alerts.length === 0 &&
    (model.relationships?.inner_circle?.length ?? 0) === 0 &&
    (model.relationships?.mentors?.length ?? 0) === 0 &&
    (model.relationships?.collaborators?.length ?? 0) === 0 &&
    !model.preferences.work_hours.preferred_start.trim() &&
    !model.preferences.work_hours.preferred_end.trim() &&
    !model.preferences.work_hours.timezone.trim() &&
    !model.preferences.peak_energy_time.trim() &&
    !model.preferences.communication_style.trim() &&
    !model.preferences.learning_style.trim() &&
    !model.preferences.decision_making_style.trim()
  );
}

/**
 * Unified helper to check if the model is effectively empty.
 * Prioritizes backend diagnostics if available, falls back to local heuristic.
 */
export function getModelEmptyState(
  model: LifeModel | null,
  diagnostics: SystemDiagnostics | null | undefined
): boolean {
  if (diagnostics && typeof diagnostics.model_empty === "boolean") {
    return diagnostics.model_empty;
  }
  return isModelEmpty(model);
}
