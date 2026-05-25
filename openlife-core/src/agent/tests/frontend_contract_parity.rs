//! Backend ↔ Frontend Typed Event Contract Parity
//!
//! # Purpose
//!
//! Ensures that Rust backend typed payload builder output,
//! frontend `typedContract.ts` parser, frontend `fixtures/agentRunEvents.ts`,
//! and the trace contract matrix document remain consistent.
//!
//! This is a **cross-end contract parity audit** — a CI-enforceable test
//! that scans frontend source files at test time and verifies every backend
//! typed builder event has a corresponding frontend event type string,
//! required field tokens, parser tokens, fixture tokens, and test tokens
//! present in the expected frontend files.
//!
//! # Backend builder source
//!
//! The backend builder name list is **not** hand-written here.  It is
//! derived from `super::trace_contract_audit::typed_payload_builder_refs()`
//! which calls the real `payload_builder_contract_manifest()` in the same
//! test module — the same manifest used by `payload_builder_contract`
//! tests.  The frontend parity module has zero duplicated builder lists.
//!
//! # What it does NOT do
//!
//! - Does NOT parse TypeScript AST — uses simple substring scanning
//! - Does NOT change any frontend parser behavior
//! - Does NOT change any Rust payload builder output
//! - Does NOT introduce external dependencies
//! - Does NOT modify UI components
//!
//! # How it works
//!
//! 1. A `FrontendTypedEventContractEntry` manifest defines the expected
//!    cross-end contract for each backend typed builder event.
//! 2. A pure validator function scans frontend source files for required
//!    tokens and checks that every backend builder has a frontend entry.
//! 3. Positive tests call the validator with real data.
//! 4. Negative tests construct bad manifests or tamper with source strings,
//!    then assert the validator returns Err.
//!
//! All tests are prefixed `frontend_typed_contract_`.

use std::collections::HashSet;
use std::path::Path;

// ── Re-exported from trace_contract_audit.rs ──
use super::trace_contract_audit::typed_payload_builder_refs;

// ════════════════════════════════════════════════════════════════════
// Parity manifest entry
// ════════════════════════════════════════════════════════════════════

/// One entry in the frontend typed contract parity manifest.
///
/// Each entry defines:
/// - Which backend builder maps to which frontend event type string
/// - What payload fields the backend builder outputs (snake_case)
/// - What tokens MUST exist in `typedContract.ts` (parser tokens)
/// - What tokens MUST exist in `agentRunEvents.ts` (fixture tokens)
/// - What tokens MUST exist in `typedContract.test.ts` (test tokens)
/// - Whether this event is parser pass-through (structurally parsed vs
///   metadata-only/generic pass-through)
struct FrontendTypedEventContractEntry {
    /// Short event name matching `event_contract_manifest()` entry.
    event: &'static str,
    /// Builder function name (e.g. `build_tool_call_blocked_payload`).
    backend_builder: &'static str,
    /// Frontend event type string (e.g. `agent_spec.selected`).
    frontend_event_type: &'static str,
    /// Required snake_case payload fields the backend builder outputs.
    required_payload_fields: &'static [&'static str],
    /// Optional snake_case payload fields — informational.
    #[allow(dead_code)]
    optional_payload_fields: &'static [&'static str],
    /// Substring tokens that MUST appear in `typedContract.ts`.
    frontend_parser_tokens: &'static [&'static str],
    /// Substring tokens that MUST appear in `fixtures/agentRunEvents.ts`.
    fixture_tokens: &'static [&'static str],
    /// Substring tokens that MUST appear in `typedContract.test.ts`.
    test_tokens: &'static [&'static str],
    /// False for governance events (structurally parsed by frontend).
    /// True for metadata-only or generic events (pass-through as kind: "unknown").
    ///
    /// Governance events: required fields must appear in typedContract.ts
    /// AND in fixtures/tests.
    /// Parser pass-through events: required fields must appear in fixtures/tests
    /// (typedContract.ts only needs the event type token).
    is_parser_pass_through: bool,
}

impl FrontendTypedEventContractEntry {
    #[allow(clippy::too_many_arguments)]
    fn new(
        event: &'static str,
        backend_builder: &'static str,
        frontend_event_type: &'static str,
        required_payload_fields: &'static [&'static str],
        optional_payload_fields: &'static [&'static str],
        frontend_parser_tokens: &'static [&'static str],
        fixture_tokens: &'static [&'static str],
        test_tokens: &'static [&'static str],
        is_parser_pass_through: bool,
    ) -> Self {
        Self {
            event,
            backend_builder,
            frontend_event_type,
            required_payload_fields,
            optional_payload_fields,
            frontend_parser_tokens,
            fixture_tokens,
            test_tokens,
            is_parser_pass_through,
        }
    }
}

// ════════════════════════════════════════════════════════════════════
// Parity manifest — single source of truth for cross-end contract
// ════════════════════════════════════════════════════════════════════

#[rustfmt::skip]
fn frontend_parity_manifest() -> Vec<FrontendTypedEventContractEntry> {
    vec![
        // ── ProductionAudited (governance events) ─────────────
        FrontendTypedEventContractEntry::new(
            "AgentSpecSelected",
            "build_agent_spec_selected_payload",
            "agent_spec.selected",
            &["agent_spec_id", "privacy_policy"],
            &["role"],
            &["agent_spec.selected", "agent_spec_id", "privacy_policy"],
            &["agent_spec.selected", "agent_spec_id", "role", "privacy_policy"],
            &["agent_spec.selected", "AgentRunEvent"],
            false,
        ),
        FrontendTypedEventContractEntry::new(
            "PromptStackAssembled",
            "build_prompt_stack_assembled_payload",
            "prompt_stack.assembled",
            &["agent_spec_id", "prompt_blocks"],
            &[],
            &["prompt_stack.assembled", "agent_spec_id", "prompt_blocks"],
            &["prompt_stack.assembled", "agent_spec_id", "prompt_blocks"],
            &["prompt_stack.assembled"],
            false,
        ),
        FrontendTypedEventContractEntry::new(
            "ContextGovernanceApplied",
            "build_context_governance_applied_payload",
            "context_governance.applied",
            &["agent_spec_id"],
            &["context_included", "context_excluded", "privacy_policy", "agent_spec_privacy_policy", "effective_privacy_policy"],
            &["context_governance.applied"],
            &["context_governance.applied", "agent_spec_id", "context_included", "context_excluded", "privacy_policy"],
            &["context_governance.applied"],
            false,
        ),
        FrontendTypedEventContractEntry::new(
            "ToolCallBlocked",
            "build_tool_call_blocked_payload",
            "tool.call_blocked",
            &["status", "tool_name", "source", "agent_spec_id"],
            &["block_reason", "proposal_reason", "failure_kind", "proposal_id", "target_tool_name", "wrapper_tool_name", "human_message", "target_source"],
            &["tool.call_blocked", "block_reason", "proposal_reason"],
            &["tool.call_blocked", "status", "tool_name", "source", "block_reason", "agent_spec_id"],
            &["tool.call_blocked", "BlockReason"],
            false,
        ),
        FrontendTypedEventContractEntry::new(
            "ReplayStarted",
            "build_replay_started_payload",
            "replay.started",
            &["status", "run_id", "action_id", "replay_of_action_id", "agent_spec_id", "tool_name", "source"],
            &[],
            &["replay.started", "run_id", "action_id", "replay_of_action_id"],
            &[],
            &["replay.started", "run_id", "action_id", "replay_of_action_id", "agent_spec_id", "tool_name", "source"],
            false,
        ),
        FrontendTypedEventContractEntry::new(
            "ReplayCompleted",
            "build_replay_completed_payload",
            "replay.completed",
            &["status", "run_id", "action_id", "replay_of_action_id", "agent_spec_id", "tool_name", "source"],
            &["block_reason", "proposal_reason", "failure_kind"],
            &["replay.completed", "run_id", "action_id", "replay_of_action_id"],
            &[],
            &["replay.completed", "run_id", "action_id", "replay_of_action_id", "agent_spec_id", "tool_name", "source"],
            false,
        ),
        FrontendTypedEventContractEntry::new(
            "ReplayFailed",
            "build_replay_failed_payload",
            "replay.failed",
            &["status", "run_id", "action_id", "replay_of_action_id", "human_message"],
            &["block_reason", "failure_kind", "tool_name", "source", "agent_spec_id"],
            &["replay.failed", "block_reason", "failure_kind"],
            &["replay.failed", "status", "run_id", "action_id", "replay_of_action_id", "block_reason"],
            &["replay.failed"],
            false,
        ),
        // ── IntentionallyExcluded (parser pass-through events) ───
        // These events are not structurally parsed by typedContract.ts
        // (they pass through as kind: "unknown"). Their required fields
        // are checked against fixtures/tests only (not typedContract.ts).
        // The frontend_event_type must appear in typedContract.ts for
        // labels, explanations, or generic failure detection.
        FrontendTypedEventContractEntry::new(
            "ProposalCreated",
            "build_proposal_created_payload",
            "proposal.created",
            &["proposal_id", "source", "proposal_type", "affected_path", "risk_level", "status", "source_detail"],
            &[],
            &["proposal.created"],
            &["proposal.created", "proposal_id", "source", "proposal_type", "affected_path", "risk_level", "status", "source_detail"],
            &["proposal.created", "proposal_id", "source", "proposal_type", "affected_path", "risk_level", "status", "source_detail"],
            true,
        ),
        FrontendTypedEventContractEntry::new(
            "ModelFailed",
            "build_model_failed_payload",
            "model.failed",
            &["agent_spec_id", "error"],
            &[],
            &["model.failed"],
            &[],
            &["model.failed", "error"],
            true,
        ),
        FrontendTypedEventContractEntry::new(
            "RunFailed",
            "build_run_failed_payload",
            "run.failed",
            &["error"],
            &[],
            &["run.failed"],
            &[],
            &["run.failed", "error"],
            true,
        ),
        FrontendTypedEventContractEntry::new(
            "ToolCallFailed",
            "build_tool_call_failed_payload",
            "tool.call_failed",
            &["tool", "error"],
            &[],
            &["tool.call_failed"],
            &[],
            &["tool.call_failed", "error"],
            true,
        ),
        FrontendTypedEventContractEntry::new(
            "ModelCallFailed",
            "build_model_call_failed_payload",
            "model.call_failed",
            &["provider", "model", "error"],
            &[],
            &["model.call_failed"],
            &[],
            &["model.call_failed", "provider", "model", "error"],
            true,
        ),
    ]
}

// ════════════════════════════════════════════════════════════════════
// File reading helpers
// ════════════════════════════════════════════════════════════════════

fn read_frontend_file(filename: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let full = root.join("frontend").join(filename);
    std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", full.display(), e))
}

fn typed_contract_source() -> String {
    read_frontend_file("src/utils/typedContract.ts")
}

fn typed_contract_test_source() -> String {
    read_frontend_file("src/utils/typedContract.test.ts")
}

fn fixtures_source() -> String {
    read_frontend_file("src/test/fixtures/agentRunEvents.ts")
}

// ════════════════════════════════════════════════════════════════════
// Core validator
// ════════════════════════════════════════════════════════════════════

/// Validate backend ↔ frontend typed event contract parity.
///
/// Returns `Ok(())` if all checks pass, or `Err(Vec<String>)` with
/// one or more human-readable error messages.
fn validate_frontend_typed_contract_parity(
    parity_manifest: &[FrontendTypedEventContractEntry],
    builder_refs: &[(&str, &str)],
    typed_contract_source: &str,
    typed_contract_test_source: &str,
    fixtures_source: &str,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // ── 1. Cross-manifest coverage ─────────────────────────────────

    let backend_builder_set: HashSet<&str> = builder_refs.iter().map(|(_, b)| *b).collect();
    let frontend_builder_set: HashSet<&str> =
        parity_manifest.iter().map(|e| e.backend_builder).collect();
    let _frontend_event_set: HashSet<&str> = parity_manifest.iter().map(|e| e.event).collect();
    let _frontend_type_set: HashSet<&str> = parity_manifest
        .iter()
        .map(|e| e.frontend_event_type)
        .collect();

    // 1a. Every backend builder must have a frontend parity entry.
    let missing: Vec<&&str> = backend_builder_set
        .difference(&frontend_builder_set)
        .collect();
    if !missing.is_empty() {
        errors.push(format!(
            "{} backend builder(s) missing from frontend parity manifest: {:?}",
            missing.len(),
            missing,
        ));
    }

    // 1b. No frontend parity entry references a non-existent backend builder.
    let extraneous: Vec<&&str> = frontend_builder_set
        .difference(&backend_builder_set)
        .collect();
    if !extraneous.is_empty() {
        errors.push(format!(
            "{} frontend parity builder(s) not in backend builder manifest: {:?}",
            extraneous.len(),
            extraneous,
        ));
    }

    // 1c. No duplicate event names in the frontend parity manifest.
    let mut seen_events = HashSet::new();
    let mut dup_events = Vec::new();
    for entry in parity_manifest {
        if !seen_events.insert(entry.event) {
            dup_events.push(entry.event);
        }
    }
    if !dup_events.is_empty() {
        errors.push(format!(
            "duplicate event name(s) in frontend parity manifest: {:?}",
            dup_events,
        ));
    }

    // 1d. No duplicate frontend_event_type in the frontend parity manifest.
    let mut seen_types = Vec::new();
    let mut dup_types = Vec::new();
    for entry in parity_manifest {
        if seen_types.contains(&entry.frontend_event_type) {
            dup_types.push(entry.frontend_event_type);
        }
        seen_types.push(entry.frontend_event_type);
    }
    if !dup_types.is_empty() {
        errors.push(format!(
            "duplicate frontend_event_type(s) in frontend parity manifest: {:?}",
            dup_types.iter().collect::<HashSet<_>>(),
        ));
    }

    // ── 2. Per-entry token checks ───────────────────────────────────

    for entry in parity_manifest {
        let ft = entry.frontend_event_type;

        // 2a. frontend_event_type must appear in typedContract.ts.
        if !typed_contract_source.contains(ft) {
            errors.push(format!(
                "[{}] frontend_event_type '{}' not found in typedContract.ts",
                entry.event, ft,
            ));
        }

        if entry.is_parser_pass_through {
            // Parser pass-through: required fields must appear in fixtures OR
            // test file (the typedContract.ts parser does not structurally parse
            // these events).
            for field in entry.required_payload_fields {
                let in_fixtures = fixtures_source.contains(field);
                let in_tests = typed_contract_test_source.contains(field);
                if !in_fixtures && !in_tests {
                    errors.push(format!(
                        "[{}] required field '{}' not found in agentRunEvents.ts or typedContract.test.ts",
                        entry.event, field,
                    ));
                }
            }
        } else {
            // Governance event: required fields must appear in
            // typedContract.ts AND in fixtures/tests (at least one).
            for field in entry.required_payload_fields {
                if !typed_contract_source.contains(field) {
                    errors.push(format!(
                        "[{}] required field '{}' not found in typedContract.ts",
                        entry.event, field,
                    ));
                }
            }
            for field in entry.required_payload_fields {
                let in_fixtures = fixtures_source.contains(field);
                let in_tests = typed_contract_test_source.contains(field);
                if !in_fixtures && !in_tests {
                    errors.push(format!(
                        "[{}] required field '{}' not found in agentRunEvents.ts or typedContract.test.ts",
                        entry.event, field,
                    ));
                }
            }
        }

        // 2d. Each frontend_parser_tokens must appear in typedContract.ts.
        for token in entry.frontend_parser_tokens {
            if !typed_contract_source.contains(token) {
                errors.push(format!(
                    "[{}] parser token '{}' not found in typedContract.ts",
                    entry.event, token,
                ));
            }
        }

        // 2e. Each fixture_tokens must appear in agentRunEvents.ts.
        for token in entry.fixture_tokens {
            if !fixtures_source.contains(token) {
                errors.push(format!(
                    "[{}] fixture token '{}' not found in agentRunEvents.ts",
                    entry.event, token,
                ));
            }
        }

        // 2f. Each test_tokens must appear in typedContract.test.ts.
        for token in entry.test_tokens {
            if !typed_contract_test_source.contains(token) {
                errors.push(format!(
                    "[{}] test token '{}' not found in typedContract.test.ts",
                    entry.event, token,
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ════════════════════════════════════════════════════════════════════
// Positive tests — call validator on real data
// ════════════════════════════════════════════════════════════════════

#[test]
fn frontend_typed_contract_all_backend_builders_have_frontend_entry() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let ts_source = typed_contract_source();
    let test_source = typed_contract_test_source();
    let fix_source = fixtures_source();

    let result = validate_frontend_typed_contract_parity(
        &parity,
        &builder_refs,
        &ts_source,
        &test_source,
        &fix_source,
    );
    if let Err(ref errs) = result {
        assert!(
            result.is_ok(),
            "all_backend_builders check failed:\n{}",
            errs.join("\n"),
        );
    }
    assert!(
        result.is_ok(),
        "unexpected errors: {:?}",
        result.unwrap_err()
    );
}

#[test]
fn frontend_typed_contract_frontend_tokens_exist() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let ts_source = typed_contract_source();
    let test_source = typed_contract_test_source();
    let fix_source = fixtures_source();

    let result = validate_frontend_typed_contract_parity(
        &parity,
        &builder_refs,
        &ts_source,
        &test_source,
        &fix_source,
    );
    assert!(
        result.is_ok(),
        "frontend_typed_contract_frontend_tokens_exist failed:\n{}",
        result.unwrap_err().join("\n"),
    );
}

#[test]
fn frontend_typed_contract_required_fields_exist() {
    let parity = frontend_parity_manifest();
    let ts_source = typed_contract_source();
    let test_source = typed_contract_test_source();
    let fix_source = fixtures_source();

    let mut errors = Vec::new();
    for entry in &parity {
        if entry.is_parser_pass_through {
            for field in entry.required_payload_fields {
                if !fix_source.contains(field) && !test_source.contains(field) {
                    errors.push(format!(
                        "[{}] required field '{}' missing from agentRunEvents.ts and typedContract.test.ts",
                        entry.event, field,
                    ));
                }
            }
        } else {
            for field in entry.required_payload_fields {
                if !ts_source.contains(field) {
                    errors.push(format!(
                        "[{}] required field '{}' missing from typedContract.ts",
                        entry.event, field,
                    ));
                }
            }
        }
    }
    assert!(
        errors.is_empty(),
        "required_fields check failed:\n{}",
        errors.join("\n"),
    );
}

#[test]
fn frontend_typed_contract_fixture_tokens_exist() {
    let parity = frontend_parity_manifest();
    let fix_source = fixtures_source();

    let mut errors = Vec::new();
    for entry in &parity {
        for token in entry.fixture_tokens {
            if !fix_source.contains(token) {
                errors.push(format!(
                    "[{}] fixture token '{}' missing from agentRunEvents.ts",
                    entry.event, token,
                ));
            }
        }
    }
    assert!(
        errors.is_empty(),
        "fixture_tokens check failed:\n{}",
        errors.join("\n"),
    );
}

#[test]
fn frontend_typed_contract_test_tokens_exist() {
    let parity = frontend_parity_manifest();
    let test_source = typed_contract_test_source();

    let mut errors = Vec::new();
    for entry in &parity {
        for token in entry.test_tokens {
            if !test_source.contains(token) {
                errors.push(format!(
                    "[{}] test token '{}' missing from typedContract.test.ts",
                    entry.event, token,
                ));
            }
        }
    }
    assert!(
        errors.is_empty(),
        "test_tokens check failed:\n{}",
        errors.join("\n"),
    );
}

// ════════════════════════════════════════════════════════════════════
// Negative tests — construct bad manifests, assert Err
// ════════════════════════════════════════════════════════════════════

#[test]
fn frontend_typed_contract_missing_frontend_entry_fails() {
    let builder_refs = typed_payload_builder_refs();
    let mut parity = frontend_parity_manifest();
    parity.retain(|e| e.event != "ToolCallBlocked");

    let ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let fix = fixtures_source();

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when a backend builder has no frontend parity entry",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("build_tool_call_blocked_payload"),
        "error should name the missing builder: {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_unknown_builder_fails() {
    let builder_refs = typed_payload_builder_refs();
    let mut parity = frontend_parity_manifest();
    parity.push(FrontendTypedEventContractEntry::new(
        "SyntheticFake",
        "build_nonexistent_payload",
        "synthetic.fake",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
    ));

    let ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let fix = fixtures_source();

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when parity manifest references a non-existent backend builder",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("build_nonexistent_payload"),
        "error should name the extraneous builder: {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_duplicate_event_type_fails() {
    let builder_refs = typed_payload_builder_refs();
    let mut parity = frontend_parity_manifest();
    parity.push(FrontendTypedEventContractEntry::new(
        "ToolCallBlockedDup",
        "build_tool_call_blocked_payload",
        "tool.call_blocked",
        &[],
        &[],
        &[],
        &[],
        &[],
        false,
    ));

    let ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let fix = fixtures_source();

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when two entries share the same frontend_event_type",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("duplicate frontend_event_type"),
        "error should mention 'duplicate frontend_event_type': {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_missing_parser_token_fails() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let mut ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let fix = fixtures_source();

    let needle = "tool.call_blocked";
    ts = ts.replace(needle, &"T".repeat(needle.len()));

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when parser token is removed from typedContract.ts",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("tool.call_blocked") || err.contains("tool_call_blocked"),
        "error should mention the missing token: {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_missing_required_field_fails() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let mut ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let fix = fixtures_source();

    ts = ts.replace("agent_spec_id", &"X".repeat(13));

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when required field is removed from typedContract.ts",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("agent_spec_id"),
        "error should mention the missing required field: {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_missing_fixture_token_fails() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let mut fix = fixtures_source();

    fix = fix.replace("block_reason", &"X".repeat(12));

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when fixture token is removed from agentRunEvents.ts",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("block_reason"),
        "error should mention the missing fixture token: {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_missing_test_token_fails() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let ts = typed_contract_source();
    let mut test_src = typed_contract_test_source();
    let fix = fixtures_source();

    test_src = test_src.replace("replay.failed", &"X".repeat(12));

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when test token is removed from typedContract.test.ts",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("replay.failed"),
        "error should mention the missing test token: {}",
        err,
    );
}

// ════════════════════════════════════════════════════════════════════
// NEW: Backend builder source drift test
// ════════════════════════════════════════════════════════════════════

/// Proves that when a new backend builder is added to the builder
/// manifest but the frontend parity manifest does NOT have a
/// corresponding entry, the validator fails.
///
/// The backend builder refs come from `typed_payload_builder_refs()`
/// which is derived from the real `payload_builder_contract_manifest()`.
/// There is no hand-written 11-builder list in this module.
#[test]
fn frontend_typed_contract_backend_builder_source_drift_fails() {
    let mut builder_refs = typed_payload_builder_refs();
    // Inject a synthetic new builder that has no frontend parity entry.
    builder_refs.push(("SyntheticDrift", "build_synthetic_new_payload"));

    let parity = frontend_parity_manifest();
    let ts = typed_contract_source();
    let test_src = typed_contract_test_source();
    let fix = fixtures_source();

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when a new backend builder appears without a frontend parity entry",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("build_synthetic_new_payload"),
        "error should name the drifted builder: {}",
        err,
    );
}

#[test]
fn frontend_typed_contract_generic_failure_required_field_missing_fails() {
    let parity = frontend_parity_manifest();
    let builder_refs = typed_payload_builder_refs();
    let ts = typed_contract_source();
    let mut test_src = typed_contract_test_source();
    let fix = fixtures_source();

    // Remove "provider" — a required field for ModelCallFailed.
    test_src = test_src.replace("provider", &"X".repeat(8));

    let result =
        validate_frontend_typed_contract_parity(&parity, &builder_refs, &ts, &test_src, &fix);
    assert!(
        result.is_err(),
        "should fail when generic failure required field 'provider' is removed from test file",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("provider"),
        "error should mention the missing generic failure field 'provider': {}",
        err,
    );
}
