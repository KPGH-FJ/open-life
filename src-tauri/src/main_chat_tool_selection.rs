const MAIN_CHAT_CONTRACT_SAFE_LABEL_MAX_LEN: usize = 96;
const MAIN_CHAT_WRITE_LIKE_CONTAINS_TERMS: &[&str] = &[
    "write",
    "send",
    "delete",
    "remove",
    "update",
    "create",
    "modify",
    "mutate",
    "externalwrite",
    "externalsideeffect",
    "realwrite",
    "emailsend",
    "calendarsend",
    "calendarwrite",
    "providerwrite",
    "shellexec",
    "execute",
];

#[derive(Clone)]
pub(crate) struct MainChatGovernedToolCandidate {
    pub(crate) candidate_id: String,
    pub(crate) target: String,
    pub(crate) manifest_source: String,
    pub(crate) capabilities: Vec<String>,
    pub(crate) match_reason: String,
}

impl MainChatGovernedToolCandidate {
    pub(crate) fn capabilities_digest_label(&self) -> String {
        let capabilities_digest = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::json!(self.capabilities),
        );
        format!(
            "bytes:{} hash:{}",
            capabilities_digest.0, capabilities_digest.1
        )
    }

    pub(crate) fn capability_labels_label(&self) -> String {
        let mut labels = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for capability in &self.capabilities {
            if !main_chat_contract_safe_label(capability, false)
                || main_chat_surface_contains_write_like_term(capability)
                || !seen.insert(capability.as_str())
            {
                continue;
            }
            let next_label = if labels.is_empty() {
                capability.clone()
            } else {
                format!("{}/{}", labels.join("/"), capability)
            };
            if next_label.len() > MAIN_CHAT_CONTRACT_SAFE_LABEL_MAX_LEN {
                break;
            }
            labels.push(capability.clone());
        }
        if labels.is_empty() {
            "none".into()
        } else {
            labels.join("/")
        }
    }

    pub(crate) fn manifest_source_label(&self) -> String {
        main_chat_contract_label_or(&self.manifest_source, true, "contract_unsafe_source")
    }

    pub(crate) fn match_reason_label(&self) -> String {
        main_chat_contract_label_or(&self.match_reason, false, "contract_unsafe")
    }
}

/// Produce the exact governed input used for canonical MCP read dispatch.
/// Deterministic built-in defaults remain runtime-owned rather than copied
/// into provider output or a second execution plan.
pub(crate) fn normalize_main_chat_mcp_read_arguments(
    manifest: &openlife_core::tool_manifest::ToolManifest,
    supplied_arguments: serde_json::Value,
) -> serde_json::Value {
    if manifest.name == "builtin_echo"
        && supplied_arguments
            .as_object()
            .is_some_and(serde_json::Map::is_empty)
    {
        serde_json::json!({ "text": "kernel registered MCP read" })
    } else {
        supplied_arguments
    }
}

pub(crate) fn main_chat_governed_mcp_read_tool_candidates(
    registry: &openlife_core::mcp::McpRegistry,
    selection_query: &str,
    limit: usize,
) -> Vec<MainChatGovernedToolCandidate> {
    let mut manifests = registry
        .list_manifests()
        .into_iter()
        .filter(main_chat_manifest_is_governed_read_candidate)
        .collect::<Vec<_>>();
    let selection_terms = main_chat_selection_terms(selection_query);
    manifests.sort_by(|left, right| {
        let left_score = main_chat_manifest_selection_score(left, &selection_terms);
        let right_score = main_chat_manifest_selection_score(right, &selection_terms);
        right_score
            .cmp(&left_score)
            .then_with(|| left.name.cmp(&right.name))
    });
    let mut seen_targets = std::collections::HashSet::new();
    manifests
        .into_iter()
        .filter(|manifest| seen_targets.insert(manifest.name.clone()))
        .take(limit)
        .map(|manifest| {
            let selection_score = main_chat_manifest_selection_score(&manifest, &selection_terms);
            let capabilities = main_chat_manifest_read_candidate_capabilities(&manifest);
            MainChatGovernedToolCandidate {
                candidate_id: manifest.name.clone(),
                target: manifest.name,
                manifest_source: manifest.source.to_string(),
                capabilities,
                match_reason: if selection_score > 0 {
                    "capability_or_name_match".into()
                } else {
                    "manifest_default_order".into()
                },
            }
        })
        .collect()
}

fn main_chat_manifest_read_candidate_capabilities(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> Vec<String> {
    let mut capabilities = manifest.capabilities.clone();
    if manifest.action_type.eq_ignore_ascii_case("read")
        && !capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"))
    {
        capabilities.insert(0, "read".into());
    }
    capabilities
}

fn main_chat_manifest_selection_score(
    manifest: &openlife_core::tool_manifest::ToolManifest,
    selection_terms: &[String],
) -> usize {
    if selection_terms.is_empty() {
        return 0;
    }
    let searchable = std::iter::once(manifest.name.as_str())
        .chain(manifest.capabilities.iter().map(String::as_str))
        .chain(manifest.tags.iter().map(String::as_str))
        .map(normalize_main_chat_selection_text)
        .collect::<Vec<_>>();
    selection_terms
        .iter()
        .filter(|term| searchable.iter().any(|value| value.contains(term.as_str())))
        .count()
}

fn main_chat_selection_terms(selection_query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for raw_token in selection_query.split_whitespace() {
        let term = normalize_main_chat_selection_text(raw_token);
        if term.len() < 3 || main_chat_generic_selection_term(&term) {
            continue;
        }
        if !terms.iter().any(|existing| existing == &term) {
            terms.push(term);
        }
    }
    terms
}

fn normalize_main_chat_selection_text(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn main_chat_generic_selection_term(term: &str) -> bool {
    matches!(
        term,
        "mcp"
            | "read"
            | "readonly"
            | "tool"
            | "tools"
            | "utility"
            | "use"
            | "now"
            | "please"
            | "with"
            | "for"
            | "and"
            | "the"
    )
}

pub(crate) fn main_chat_manifest_is_governed_read_candidate(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    if openlife_core::agent::validate_manifest_execution_contract(manifest).is_err() {
        return false;
    }
    if manifest.name == "mcp.call_tool" || !manifest.enabled || manifest.declarative_only {
        return false;
    }
    if !main_chat_manifest_has_contract_safe_name(manifest)
        || !main_chat_manifest_has_contract_safe_source(manifest)
    {
        return false;
    }
    if manifest.requires_confirmation
        || matches!(
            manifest.risk_level.to_ascii_lowercase().as_str(),
            "high" | "critical"
        )
        || matches!(
            manifest.permission_level.to_ascii_lowercase().as_str(),
            "high" | "critical"
        )
        || matches!(
            manifest.action_type.to_ascii_lowercase().as_str(),
            "write" | "external_side_effect"
        )
        || manifest.capabilities.iter().any(|capability| {
            matches!(
                capability.to_ascii_lowercase().as_str(),
                "write" | "external_side_effect"
            )
        })
        || main_chat_manifest_has_write_like_surface(manifest)
    {
        return false;
    }
    manifest.action_type.eq_ignore_ascii_case("read")
        || manifest
            .capabilities
            .iter()
            .any(|capability| capability.eq_ignore_ascii_case("read"))
}

fn main_chat_manifest_has_contract_safe_name(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    main_chat_contract_safe_label(&manifest.name, false)
}

fn main_chat_manifest_has_contract_safe_source(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    main_chat_contract_safe_label(&manifest.source.to_string(), true)
}

fn main_chat_contract_safe_label(value: &str, allow_colon: bool) -> bool {
    !value.is_empty()
        && value.len() <= MAIN_CHAT_CONTRACT_SAFE_LABEL_MAX_LEN
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, '.' | '_' | '-' | '/')
                || (allow_colon && ch == ':')
        })
}

fn main_chat_contract_label_or(value: &str, allow_colon: bool, fallback: &str) -> String {
    if main_chat_contract_safe_label(value, allow_colon) {
        value.to_string()
    } else {
        fallback.to_string()
    }
}

pub(crate) fn main_chat_manifest_has_write_like_surface(
    manifest: &openlife_core::tool_manifest::ToolManifest,
) -> bool {
    std::iter::once(manifest.id.as_str())
        .chain(std::iter::once(manifest.name.as_str()))
        .chain(std::iter::once(manifest.action_type.as_str()))
        .chain(manifest.capabilities.iter().map(String::as_str))
        .chain(manifest.tags.iter().map(String::as_str))
        .any(main_chat_surface_contains_write_like_term)
}

pub(crate) fn main_chat_surface_contains_write_like_term(value: &str) -> bool {
    let surface = normalize_main_chat_selection_text(value);
    matches!(
        surface.as_str(),
        "write"
            | "send"
            | "delete"
            | "remove"
            | "update"
            | "create"
            | "modify"
            | "mutate"
            | "externalwrite"
            | "externalsideeffect"
            | "realwrite"
            | "emailsend"
            | "calendarsend"
            | "calendarwrite"
            | "providerwrite"
            | "shellexec"
            | "execute"
            | "exec"
    ) || MAIN_CHAT_WRITE_LIKE_CONTAINS_TERMS
        .iter()
        .any(|term| surface.contains(term))
        || surface.ends_with("write")
        || surface.ends_with("send")
        || surface.ends_with("delete")
}
