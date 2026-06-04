#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SupportedMaturationDomain {
    PlanningPreference,
    EnergyPattern,
    WorkStyle,
    CommunicationPreference,
}

impl SupportedMaturationDomain {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            SupportedMaturationDomain::PlanningPreference => "planning_preference",
            SupportedMaturationDomain::EnergyPattern => "energy_pattern",
            SupportedMaturationDomain::WorkStyle => "work_style",
            SupportedMaturationDomain::CommunicationPreference => "communication_preference",
        }
    }
}

pub(crate) const MATURATION_SOURCE_DETAIL_PREFIX: &str = "maturation:";

pub(crate) fn is_maturation_source_detail(source_detail: &str) -> bool {
    maturation_source_event_type(source_detail).is_some()
}

pub(crate) fn maturation_source_event_type(source_detail: &str) -> Option<&str> {
    source_detail
        .trim()
        .strip_prefix(MATURATION_SOURCE_DETAIL_PREFIX)
        .map(str::trim)
        .filter(|event_type| !event_type.is_empty())
}

pub(crate) fn classify_supported_maturation_domain(
    affected_path: &str,
    source_detail: Option<&str>,
) -> Option<SupportedMaturationDomain> {
    if high_risk_maturation_path_or_source_detail(affected_path, source_detail) {
        return None;
    }

    if let Some(event_type) = source_detail.and_then(maturation_source_event_type) {
        return supported_domain_from_text(event_type);
    }

    supported_domain_from_text(affected_path)
}

pub(crate) fn high_risk_maturation_path_or_source_detail(
    affected_path: &str,
    source_detail: Option<&str>,
) -> bool {
    high_risk_maturation_text(affected_path)
        || source_detail
            .map(high_risk_maturation_text)
            .unwrap_or(false)
}

pub(crate) fn high_risk_maturation_text(value: &str) -> bool {
    contains_any(
        &value.trim().to_ascii_lowercase(),
        &[
            "identity",
            "values",
            "value/",
            "mission",
            "relationships",
            "relationship",
            "health",
            "finance",
            "financial",
            "privacy",
            "long_term",
            "long-term",
            "longterm",
            "life_direction",
            "direction",
        ],
    )
}

fn supported_domain_from_text(value: &str) -> Option<SupportedMaturationDomain> {
    let value = value.trim().to_ascii_lowercase();
    if contains_any(
        &value,
        &[
            "preference.planning",
            "planning_preference",
            "planning-preference",
            "/preferences/planning",
            "preferences.planning",
            "low_energy_planning",
            "low-pressure-planning",
            "low_pressure_planning",
            "planning",
        ],
    ) {
        return Some(SupportedMaturationDomain::PlanningPreference);
    }
    if contains_any(
        &value,
        &[
            "energy_pattern",
            "energy-pattern",
            "energy.pattern",
            "/state/energy",
            "/preferences/energy",
            "preferences.energy",
            "preferences.peak_energy_time",
            "peak_energy",
            "energy",
        ],
    ) {
        return Some(SupportedMaturationDomain::EnergyPattern);
    }
    if contains_any(
        &value,
        &[
            "work_style",
            "work-style",
            "work.style",
            "workflow",
            "deep_work",
            "deep-work",
            "/preferences/work",
            "preferences.work",
        ],
    ) {
        return Some(SupportedMaturationDomain::WorkStyle);
    }
    if contains_any(
        &value,
        &[
            "preference.communication",
            "communication_preference",
            "communication-preference",
            "communication",
            "communication_style",
            "communication-style",
            "/preferences/comm",
            "preferences.communication",
        ],
    ) {
        return Some(SupportedMaturationDomain::CommunicationPreference);
    }
    None
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
