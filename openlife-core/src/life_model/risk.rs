use crate::agent::types::RiskLevel;

/// Classify a LifeModel field path into a risk level.
///
/// Paths use dot-notation, e.g. "identity.values", "goals.long_term".
/// Unknown or unmapped paths default to Medium risk.
pub fn classify_field_risk(path: &str) -> RiskLevel {
    let normalized = path.trim().to_lowercase();

    // ── High Risk: identity-defining, never auto-apply ─────────────────
    if normalized.starts_with("identity.values")
        || normalized.starts_with("identity.life_philosophy")
        || normalized.starts_with("identity.mission_statement")
        || normalized.starts_with("identity.role_definition")
        || normalized.starts_with("goals.long_term")
        || normalized.starts_with("goals.life_goals")
        || normalized.starts_with("relationships.key_people")
    {
        return RiskLevel::High;
    }

    // ── Medium Risk: proposal-first, reviewable ───────────────────────
    if normalized.starts_with("identity") // identity.name, personality_traits, voice_style, etc.
        || normalized.starts_with("goals.short_term")
        || normalized.starts_with("goals.medium_term")
        || normalized.starts_with("capabilities")
        || normalized.starts_with("preferences.work_style")
        || normalized.starts_with("preferences.learning_style")
        || normalized.starts_with("preferences.communication")
        || normalized.starts_with("preferences.routines_and_habits")
        || normalized.starts_with("preferences.life_rhythm")
        || normalized.starts_with("relationships.networks")
    {
        return RiskLevel::Medium;
    }

    // ── Low Risk: traceable, lightweight confirmation eligible ───────
    if normalized.starts_with("state")
        || normalized.starts_with("goals.daily")
        || normalized.starts_with("metadata")
        || normalized.starts_with("preferences.ui")
    {
        return RiskLevel::Low;
    }

    // Default: medium (conservative)
    RiskLevel::Medium
}

/// Returns true if the field path should never be auto-applied under any policy.
pub fn requires_explicit_review(path: &str) -> bool {
    matches!(
        classify_field_risk(path),
        RiskLevel::High | RiskLevel::Critical
    )
}

/// Returns true if the field can be updated via lightweight policy (post-approval).
pub fn eligible_for_lightweight_update(path: &str) -> bool {
    matches!(classify_field_risk(path), RiskLevel::Low)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_values_high() {
        assert_eq!(classify_field_risk("identity.values"), RiskLevel::High);
    }

    #[test]
    fn test_identity_life_philosophy_high() {
        assert_eq!(
            classify_field_risk("identity.life_philosophy"),
            RiskLevel::High
        );
    }

    #[test]
    fn test_identity_mission_statement_high() {
        assert_eq!(
            classify_field_risk("identity.mission_statement"),
            RiskLevel::High
        );
    }

    #[test]
    fn test_identity_role_definition_high() {
        assert_eq!(
            classify_field_risk("identity.role_definition"),
            RiskLevel::High
        );
    }

    #[test]
    fn test_goals_long_term_high() {
        assert_eq!(classify_field_risk("goals.long_term"), RiskLevel::High);
        assert_eq!(classify_field_risk("goals.life_goals"), RiskLevel::High);
    }

    #[test]
    fn test_relationships_key_people_high() {
        assert_eq!(
            classify_field_risk("relationships.key_people"),
            RiskLevel::High
        );
    }

    #[test]
    fn test_identity_name_medium() {
        assert_eq!(classify_field_risk("identity.name"), RiskLevel::Medium);
        assert_eq!(
            classify_field_risk("identity.personality_traits"),
            RiskLevel::Medium
        );
        assert_eq!(
            classify_field_risk("identity.voice_style"),
            RiskLevel::Medium
        );
    }

    #[test]
    fn test_goals_short_medium() {
        assert_eq!(classify_field_risk("goals.short_term"), RiskLevel::Medium);
        assert_eq!(classify_field_risk("goals.medium_term"), RiskLevel::Medium);
    }

    #[test]
    fn test_capabilities_medium() {
        assert_eq!(
            classify_field_risk("capabilities.skills"),
            RiskLevel::Medium
        );
        assert_eq!(
            classify_field_risk("capabilities.resources"),
            RiskLevel::Medium
        );
        assert_eq!(classify_field_risk("capabilities.tools"), RiskLevel::Medium);
    }

    #[test]
    fn test_preferences_medium() {
        assert_eq!(
            classify_field_risk("preferences.work_style"),
            RiskLevel::Medium
        );
        assert_eq!(
            classify_field_risk("preferences.communication"),
            RiskLevel::Medium
        );
        assert_eq!(
            classify_field_risk("preferences.routines_and_habits"),
            RiskLevel::Medium
        );
    }

    #[test]
    fn test_state_fields_low() {
        assert_eq!(classify_field_risk("state.current_focus"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.health_status"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.emotional_state"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.feeling_score"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.energy"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.notes"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.last_updated"), RiskLevel::Low);
        assert_eq!(classify_field_risk("state.habit_streaks"), RiskLevel::Low);
        assert_eq!(
            classify_field_risk("state.custom_dimensions"),
            RiskLevel::Low
        );
    }

    #[test]
    fn test_daily_goals_low() {
        assert_eq!(classify_field_risk("goals.daily"), RiskLevel::Low);
    }

    #[test]
    fn test_metadata_low() {
        assert_eq!(classify_field_risk("metadata.version"), RiskLevel::Low);
    }

    #[test]
    fn test_unknown_path_medium() {
        assert_eq!(classify_field_risk("unknown.field"), RiskLevel::Medium);
        assert_eq!(classify_field_risk(""), RiskLevel::Medium);
    }

    #[test]
    fn test_requires_explicit_review() {
        assert!(requires_explicit_review("identity.values"));
        assert!(requires_explicit_review("goals.long_term"));
        assert!(!requires_explicit_review("goals.short_term"));
        assert!(!requires_explicit_review("state.current_focus"));
    }

    #[test]
    fn test_eligible_for_lightweight_update() {
        assert!(eligible_for_lightweight_update("state.current_focus"));
        assert!(eligible_for_lightweight_update("state.feeling_score"));
        assert!(!eligible_for_lightweight_update("identity.values"));
        assert!(!eligible_for_lightweight_update("capabilities.skills"));
    }

    #[test]
    fn test_trimmed_input_works() {
        assert_eq!(classify_field_risk("  identity.values  "), RiskLevel::High);
    }
}
