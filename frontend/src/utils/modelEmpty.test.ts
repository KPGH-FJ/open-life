import { describe, expect, it } from "vitest";
import { isModelEmpty } from "./modelEmpty";
import { mockLifeModel } from "../test/mocks/tauri";

describe("isModelEmpty", () => {
  it("treats the default life model skeleton as empty", () => {
    const model = structuredClone(mockLifeModel);
    model.identity.name = "";
    model.identity.values = [];
    model.identity.personality_traits = [];
    model.identity.life_philosophy = "";
    model.identity.mission_statement = "";
    model.identity.role_definition.primary_role = "";
    model.identity.role_definition.secondary_roles = [];
    model.identity.role_definition.responsibilities = [];
    model.identity.role_definition.boundaries = [];
    model.identity.voice_style.tone_descriptors = [];
    model.identity.voice_style.vocabulary_preference = "";
    model.goals.short_term = [];
    model.goals.medium_term = [];
    model.goals.long_term = [];
    model.goals.life_goals = [];
    model.goals.daily = [];
    model.capabilities.skills = [];
    model.capabilities.resources = [];
    model.capabilities.networks = [];
    model.capabilities.tools = [];
    model.capabilities.knowledge_domains = [];
    model.state.current_focus = "构建人生模型";
    model.state.health_status = { physical: "良好", mental: "积极", energy_level: 7 };
    model.state.emotional_state = { current_mood: "期待", stress_level: 3, fulfillment_score: 6 };
    model.state.focus_areas = [];
    model.state.habit_streaks = [];
    model.state.custom_dimensions = [];
    model.state.alerts = [];
    model.relationships = { inner_circle: [], mentors: [], collaborators: [] };
    model.preferences.work_hours.preferred_start = "";
    model.preferences.work_hours.preferred_end = "";
    model.preferences.work_hours.timezone = "";
    model.preferences.communication_style = "";
    model.preferences.learning_style = "";
    model.preferences.decision_making_style = "";

    expect(isModelEmpty(model)).toBe(true);
  });

  it("treats builder-populated content as non-empty", () => {
    const model = structuredClone(mockLifeModel);
    expect(isModelEmpty(model)).toBe(false);
  });
});
