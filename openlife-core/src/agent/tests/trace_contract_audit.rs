//! Trace Contract Drift Audit
//!
//! # Purpose
//!
//! This module scans production source files to guarantee that every typed
//! governance/explainability `AgentRunEventType` emission uses the
//! corresponding `trace_payloads::build_*_payload` builder — preventing
//! hand-written `serde_json::json!(...)` payloads from bypassing the
//! centralised contract.
//!
//! # What it does NOT do
//!
//! - Does **not** replace runtime contract tests (event_store.rs, etc.)
//! - Does **not** scan test code — `#[cfg(test)]` regions are excluded
//! - Does **not** flag generic events (RunCreated, RunFailed, etc.)
//! - Does **not** block normal `serde_json::json!` usage elsewhere
//!
//! # How it works
//!
//! 1. For each `AuditRule`, the production portion of the target file is
//!    loaded (everything before the last `#[cfg(test)]` marker).
//! 2. The production source is **sanitised**: Rust line comments (`//`),
//!    block comments (`/* … */`), regular string literals (`"…"`), and raw
//!    string literals (`r"…"`, `r#"…"#`, …) are masked with spaces
//!    (newlines preserved).  Only the sanitised (non-comment, non-string)
//!    portion is used for token detection.
//! 3. Every occurrence of the event enum token is located in the sanitised
//!    source.
//! 4. A symmetric window (±window_chars chars) of the sanitised source is
//!    scanned for **any** of the required builder function names.
//! 5. If no builder is found in the window → **violation**.
//! 6. If `expected_emissions` is set and the count does not match exactly →
//!    **violation** (catches removals AND unregistered additions).
//! 7. If `expected_emissions` is `None` and the count is below
//!    `min_emissions` → **warning**.
//!
//! Violations report line numbers and snippets from the *original*
//! (unsanitised) source for developer readability.
//!
//! # Multi-builder events
//!
//! Each event type maps to a specific builder.  There are no multi-builder
//! events — `ReplayFailed` requires `build_replay_failed_payload` and
//! `ReplayCompleted` requires `build_replay_completed_payload`.
//! `required_builders` is still a slice so the mechanism supports future
//! legitimate multi-builder cases without redesign.
//!
//! # Negative / unit tests
//!
//! The module also includes unit tests on short synthetic snippets that
//! verify the scan logic correctly distinguishes compliant from
//! non-compliant code — including tests that prove tokens hidden inside
//! comments or string literals cannot cause false passes or false
//! positives.

use std::path::Path;

// ────────────────────────────────────────────────────────────────────
// Audit rule definition
// ────────────────────────────────────────────────────────────────────

/// A single audit assertion: in a given production file, every emission of
/// `event_token` must reference at least one builder from `required_builders`
/// within `window_chars` characters (sanitised source).
struct AuditRule {
    /// Path relative to the workspace root.
    file_rel_path: &'static str,
    /// The `AgentRunEventType` token to match.
    event_token: &'static str,
    /// Builder function names that are acceptable for this event.
    required_builders: &'static [&'static str],
    /// Exact number of production-code event occurrences expected.
    /// When `Some(n)`, the audit fails if count ≠ n — catching removals
    /// and unregistered additions equally.  When `None`, falls back to
    /// `min_emissions` (for files whose emission count is legitimately
    /// variable, such as those not yet stabilised).
    expected_emissions: Option<usize>,
    /// Minimum number of production-code event occurrences expected.
    /// Only used when `expected_emissions` is `None`.
    min_emissions: usize,
    /// Bidirectional search window (characters) around the event token
    /// in the sanitised source.
    window_chars: usize,
}

impl AuditRule {
    fn new(
        file_rel_path: &'static str,
        event_token: &'static str,
        required_builders: &'static [&'static str],
        expected_emissions: Option<usize>,
        min_emissions: usize,
        window_chars: usize,
    ) -> Self {
        Self {
            file_rel_path,
            event_token,
            required_builders,
            expected_emissions,
            min_emissions,
            window_chars,
        }
    }
}

// ────────────────────────────────────────────────────────────────────
// Audit rule matrix — the single source of truth for which events must
// use which builders in which production files.
// ────────────────────────────────────────────────────────────────────
// Window constraints (May 2026 revision):
//  - Default window: ±900 chars (previously ±1500).  In practice builder
//    calls are 1–10 lines from the event enum token; 900 is conservative.
//  - Larger windows are set only where justified by file complexity (e.g.
//    tool_executor.rs at 2050+ lines with deeply-nested helper closures).
// ────────────────────────────────────────────────────────────────────

fn audit_rules() -> Vec<AuditRule> {
    vec![
        // ── ToolCallBlocked ──
        // tool_executor.rs: 6 production emissions (verified 2026-05-18)
        //   Lines: 166, 427, 474, 802, 1011, 1623 — all before #[cfg(test)] at L2210.
        AuditRule::new(
            "openlife-core/src/agent/action_executor/tool_executor.rs",
            "AgentRunEventType::ToolCallBlocked",
            &["build_tool_call_blocked_payload"],
            Some(6),
            5,
            1200, // larger window: deep closures, ~5.4k-line file
        ),
        // tools.rs: 1 production emission (verified 2026-05-18)
        //   Line 58 — no #[cfg(test)] in file.
        AuditRule::new(
            "openlife-core/src/agent/agent_loop/tools.rs",
            "AgentRunEventType::ToolCallBlocked",
            &["build_tool_call_blocked_payload"],
            Some(1),
            1,
            900,
        ),
        // plan_executor.rs: 1 production emission (verified 2026-05-18)
        //   Line 287 — before #[cfg(test)] at L658.
        AuditRule::new(
            "openlife-core/src/agent/plan_executor.rs",
            "AgentRunEventType::ToolCallBlocked",
            &["build_tool_call_blocked_payload"],
            Some(1),
            1,
            900,
        ),
        // ── Replay events (src-tauri) ──
        // agent.rs: 2 ReplayFailed production emissions (verified 2026-05-18)
        //   Lines 135 (early-failure closure), 426 (outcome path).
        //   Wider window: at L426 the event token is in a `let event_type = if …`
        //   expression; the builder is ~21 lines later inside `if let Some(ref
        //   event_store)` block — ~1365 chars apart.
        AuditRule::new(
            "src-tauri/src/commands/agent.rs",
            "AgentRunEventType::ReplayFailed",
            &["build_replay_failed_payload"],
            Some(2),
            1,
            1500,
        ),
        // agent.rs: 1 ReplayStarted production emission (verified 2026-05-18)
        //   Line 299. Builder at L302 — 3 lines / ~90 chars.
        AuditRule::new(
            "src-tauri/src/commands/agent.rs",
            "AgentRunEventType::ReplayStarted",
            &["build_replay_started_payload"],
            Some(1),
            1,
            900,
        ),
        // agent.rs: 1 ReplayCompleted production emission (verified 2026-05-18)
        //   Line 428. Event token is in `let event_type = if …` expression at
        //   L425-429; the builder is at L457 inside a deeply nested payload
        //   branch — ~29 lines / ~1740 chars apart.
        AuditRule::new(
            "src-tauri/src/commands/agent.rs",
            "AgentRunEventType::ReplayCompleted",
            &["build_replay_completed_payload"],
            Some(1),
            1,
            1800,
        ),
        // ── Governance events: streaming / execution path ──
        // streaming.rs: 1 AgentSpecSelected (L979) — no #[cfg(test)].
        AuditRule::new(
            "src-tauri/src/streaming.rs",
            "AgentRunEventType::AgentSpecSelected",
            &["build_agent_spec_selected_payload"],
            Some(1),
            1,
            900,
        ),
        // streaming.rs: 1 PromptStackAssembled (L1042).
        AuditRule::new(
            "src-tauri/src/streaming.rs",
            "AgentRunEventType::PromptStackAssembled",
            &["build_prompt_stack_assembled_payload"],
            Some(1),
            1,
            900,
        ),
        // streaming.rs: 1 ContextGovernanceApplied (L1061).
        AuditRule::new(
            "src-tauri/src/streaming.rs",
            "AgentRunEventType::ContextGovernanceApplied",
            &["build_context_governance_applied_payload"],
            Some(1),
            1,
            900,
        ),
        // execution.rs: 1 AgentSpecSelected (L180) — before #[cfg(test)] at L552.
        AuditRule::new(
            "src-tauri/src/commands/execution.rs",
            "AgentRunEventType::AgentSpecSelected",
            &["build_agent_spec_selected_payload"],
            Some(1),
            1,
            900,
        ),
        // execution.rs: 1 PromptStackAssembled (L385).
        AuditRule::new(
            "src-tauri/src/commands/execution.rs",
            "AgentRunEventType::PromptStackAssembled",
            &["build_prompt_stack_assembled_payload"],
            Some(1),
            1,
            900,
        ),
        // execution.rs: 1 ContextGovernanceApplied (L400).
        AuditRule::new(
            "src-tauri/src/commands/execution.rs",
            "AgentRunEventType::ContextGovernanceApplied",
            &["build_context_governance_applied_payload"],
            Some(1),
            1,
            900,
        ),
        // ── Governance events: orchestrator / AgentLoop path ──
        // orchestrator.rs: 1 PromptStackAssembled (L92) — no #[cfg(test)].
        AuditRule::new(
            "openlife-core/src/agent/agent_loop/orchestrator.rs",
            "AgentRunEventType::PromptStackAssembled",
            &["build_prompt_stack_assembled_payload"],
            Some(1),
            1,
            900,
        ),
        // orchestrator.rs: 1 ContextGovernanceApplied (L106).
        AuditRule::new(
            "openlife-core/src/agent/agent_loop/orchestrator.rs",
            "AgentRunEventType::ContextGovernanceApplied",
            &["build_context_governance_applied_payload"],
            Some(1),
            1,
            900,
        ),
        // orchestrator.rs: 1 AgentSpecSelected (L177).
        AuditRule::new(
            "openlife-core/src/agent/agent_loop/orchestrator.rs",
            "AgentRunEventType::AgentSpecSelected",
            &["build_agent_spec_selected_payload"],
            Some(1),
            1,
            900,
        ),
    ]
}

// ────────────────────────────────────────────────────────────────────
// Source sanitisation — mask comments & strings while preserving length
// ────────────────────────────────────────────────────────────────────

/// Produce a copy of `src` in which Rust comments and string literals are
/// replaced with spaces (newlines inside multi-line constructs are kept).
///
/// This ensures that event tokens or builder names appearing *only* inside
/// comments or string literals cannot fool the audit into a false pass or
/// a false-positive emission count.
///
/// Handled constructs:
///  - Line comments: `// …` (everything until end-of-line, including the `//`)
///  - Block comments: `/* … */` (non-nested; unterminated block comments are
///    masked through end-of-input)
///  - Regular string literals: `"…"` with `\\` / `\"` / `\n` etc. escape
///    sequences
///  - Raw string literals: `r"…"`, `r#"…"#`, `r##"…"##`, etc.
fn sanitize_source(src: &str) -> String {
    let bytes = src.as_bytes();
    let len = bytes.len();
    let mut out = Vec::with_capacity(len);
    let mut i = 0usize;

    while i < len {
        // ── line comment `//` ──
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            out.push(b' ');
            out.push(b' ');
            i += 2;
            while i < len && bytes[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
            if i < len && bytes[i] == b'\n' {
                out.push(b'\n');
                i += 1;
            }
            continue;
        }

        // ── block comment `/* … */` ──
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            out.push(b' ');
            out.push(b' ');
            i += 2;
            while i + 1 < len {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                    break;
                }
                if bytes[i] == b'\n' {
                    out.push(b'\n');
                } else {
                    out.push(b' ');
                }
                i += 1;
            }
            // Unterminated block comment — mask rest of input.
            if i < len && !(i >= 2 && bytes[i - 2] == b'*' && bytes[i - 1] == b'/') {
                while i < len {
                    out.push(if bytes[i] == b'\n' { b'\n' } else { b' ' });
                    i += 1;
                }
            }
            continue;
        }

        // ── regular string literal `"…"` ──
        if bytes[i] == b'"' {
            out.push(b' ');
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    // escape sequence — consume two characters
                    out.push(b' ');
                    out.push(b' ');
                    i += 2;
                } else if bytes[i] == b'"' {
                    out.push(b' ');
                    i += 1;
                    break;
                } else if bytes[i] == b'\n' {
                    // Unterminated string (or multi-line literal) — keep
                    // newline so line numbering is preserved.
                    out.push(b'\n');
                    i += 1;
                } else {
                    out.push(b' ');
                    i += 1;
                }
            }
            continue;
        }

        // ── raw string literal `r"…"`, `r#"…"#`, `r##"…"##`, … ──
        // Check for `r` then optional `#`-run then `"`.
        if bytes[i] == b'r' {
            let mut hash_count = 0usize;
            let mut j = i + 1;
            while j < len && bytes[j] == b'#' {
                hash_count += 1;
                j += 1;
            }
            if j < len && bytes[j] == b'"' {
                // Confirmed: raw string start.
                out.push(b' '); // r
                i += 1;
                for _ in 0..hash_count {
                    out.push(b' '); // hashes
                    i += 1;
                }
                out.push(b' '); // opening "
                i += 1;

                // Scan for closing delimiter.
                let closing_len = 1 + hash_count; // " + hashes
                while i < len {
                    if bytes[i] == b'"' {
                        if hash_count == 0 {
                            // r"…" — first " is the closing delimiter.
                            out.push(b' ');
                            i += 1;
                            break;
                        }
                        // r#"…"#, etc. — check for trailing hashes.
                        if i + hash_count < len {
                            let mut matches = true;
                            for k in 0..hash_count {
                                if bytes[i + 1 + k] != b'#' {
                                    matches = false;
                                    break;
                                }
                            }
                            if matches {
                                out.push(b' '); // closing "
                                for _ in 0..hash_count {
                                    out.push(b' '); // closing hashes
                                }
                                i += closing_len;
                                break;
                            }
                        }
                    }
                    if bytes[i] == b'\n' {
                        out.push(b'\n');
                    } else {
                        out.push(b' ');
                    }
                    i += 1;
                }
                continue;
            }
        }

        // ── ordinary character ──
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| {
        // Non-UTF-8 bytes should never happen with Rust source; fall back
        // to an all-spaces string of the same length.
        " ".repeat(len)
    })
}

// ────────────────────────────────────────────────────────────────────
// Core scan logic
// ────────────────────────────────────────────────────────────────────

/// Find the byte offset where the last `#[cfg(test)]` block starts.
/// Uses the **original** (unsanitised) source to avoid false positives
/// from `#[cfg(test)]` appearing inside comments or strings.
///
/// Limitation: uses the last `#[cfg(test)]` on a line.  If a future file
/// has an inline `#[cfg(test)]` attribute on a test helper function
/// followed by more production code, the simple cut-at-last-marker
/// approach would exclude those trailing production lines.  Mitigation:
/// Rust convention puts `#[cfg(test)] mod tests { … }` at end of file.
fn find_test_region_start(content: &str) -> Option<usize> {
    let mut last_test_byte = None;
    let mut byte_offset = 0usize;

    for line in content.lines() {
        if line.trim().starts_with("#[cfg(test)]") {
            last_test_byte = Some(byte_offset);
        }
        byte_offset += line.len() + 1; // +1 for '\n'
    }
    last_test_byte
}

/// Build a human-readable snippet from the **original** source.
fn snippet_around(content: &str, pos: usize, context_chars: usize) -> String {
    let start = pos.saturating_sub(context_chars);
    let end = (pos + context_chars).min(content.len());
    content[start..end].replace('\n', "↵")
}

/// Audit a single source file against a rule.
///
/// Returns a list of human-readable violation messages.  An empty list
/// means the file passes the audit for this rule.
fn audit_file(rule: &AuditRule, content: &str) -> Vec<String> {
    // ── 1. Split off test region (on original source) ──
    let test_start = find_test_region_start(content);
    let original_production = if let Some(ts) = test_start {
        &content[..ts]
    } else {
        content
    };

    // ── 2. Sanitise production source (mask comments & strings) ──
    let sanitised = sanitize_source(original_production);

    // ── 3. Count production emissions (on sanitised source) ──
    let production_emissions = sanitised.matches(rule.event_token).count();

    // ── 4. Walk every occurrence of the event token in sanitised source ──
    let mut violations = Vec::new();
    let mut search_pos = 0usize;

    while let Some(rel_pos) = sanitised[search_pos..].find(rule.event_token) {
        let s_abs = search_pos + rel_pos;

        // Build a bidirectional window around the event token.
        let win_start = s_abs.saturating_sub(rule.window_chars);
        let win_end = (s_abs + rule.event_token.len() + rule.window_chars).min(sanitised.len());
        let window = &sanitised[win_start..win_end];

        let any_builder_found = rule
            .required_builders
            .iter()
            .any(|builder| window.contains(builder));

        if !any_builder_found {
            // Resolve the line number from the *original* source for
            // readable diagnostics.  Because sanitised has the same byte
            // length and newline positions, we use s_abs directly.
            let line = original_production[..s_abs.min(original_production.len())]
                .matches('\n')
                .count()
                + 1;
            let snippet = snippet_around(original_production, s_abs, 120);
            violations.push(format!(
                "VIOLATION in {}:{} — `{}` emitted but none of {:?} found within \
                 ±{} chars (sanitised source).\n  near: {}",
                rule.file_rel_path,
                line,
                rule.event_token,
                rule.required_builders,
                rule.window_chars,
                snippet,
            ));
        }

        search_pos = s_abs + rule.event_token.len();
    }

    // ── 5. Check emission count ──
    match rule.expected_emissions {
        Some(expected) => {
            if production_emissions != expected {
                violations.push(format!(
                    "COUNT MISMATCH in {} — expected exactly {} production emissions of `{}`, \
                     found {}. The audit rule may be stale or emissions were added/removed \
                     without updating the rule.",
                    rule.file_rel_path, expected, rule.event_token, production_emissions,
                ));
            }
        }
        None => {
            if production_emissions < rule.min_emissions {
                violations.push(format!(
                    "WARNING: {} — expected at least {} production emissions of `{}`, \
                     found {}. The audit rule may be stale or emissions were accidentally removed.",
                    rule.file_rel_path, rule.min_emissions, rule.event_token, production_emissions,
                ));
            }
        }
    }

    violations
}

// ────────────────────────────────────────────────────────────────────
// Shared helpers
// ────────────────────────────────────────────────────────────────────

fn read_workspace_file(rel_path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let full = root.join(rel_path);
    std::fs::read_to_string(&full)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", full.display(), e))
}

// ────────────────────────────────────────────────────────────────────
// Integration test — scans all real production files
// ────────────────────────────────────────────────────────────────────

#[test]
fn all_production_files_use_required_builders() {
    let rules = audit_rules();
    let mut all_violations = Vec::new();

    for rule in &rules {
        let content = read_workspace_file(rule.file_rel_path);
        let file_violations = audit_file(rule, &content);
        all_violations.extend(file_violations);
    }

    assert!(
        all_violations.is_empty(),
        "{} trace contract drift violation(s):\n\n{}",
        all_violations.len(),
        all_violations.join("\n\n"),
    );
}

// ────────────────────────────────────────────────────────────────────
// Unit / synthetic tests
// ────────────────────────────────────────────────────────────────────

fn audit_snippet(
    event_token: &'static str,
    required_builders: &'static [&'static str],
    snippet: &str,
    window_chars: usize,
) -> Vec<String> {
    let rule = AuditRule::new(
        "synthetic",
        event_token,
        required_builders,
        None, // no exact count for synthetic snippets
        0,    // min_emissions=0 disables the count guard in synthetic tests
        window_chars,
    );
    audit_file(&rule, snippet)
}

// ────────────────────────────────────────────────────────────────────
// Existing positive / negative tests (updated window params)
// ────────────────────────────────────────────────────────────────────

#[test]
fn negative_tool_call_blocked_without_builder_fails() {
    let code = r#"
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "web.search",
                "source": "builtin",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "expected violation for event without required builder, but none reported"
    );
}

#[test]
fn positive_tool_call_blocked_with_builder_passes() {
    let code = r#"
        let payload = trace_payloads::build_tool_call_blocked_payload(
            "blocked",
            "web.search",
            "builtin",
            Some("main.default"),
            Some("agent_spec_denied"),
            None::<&str>,
            None::<&str>,
            None,
        );
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload,
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        violations.is_empty(),
        "no violation expected when builder is present in window: {:?}",
        violations,
    );
}

#[test]
fn negative_replay_failed_without_builder_fails() {
    let code = r#"
        let evt = AgentRunEvent {
            event_type: AgentRunEventType::ReplayFailed,
            payload: json!({"status": "failed", "run_id": "r1"}),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ReplayFailed",
        &["build_replay_failed_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "expected violation for ReplayFailed without builder"
    );
}

#[test]
fn negative_replay_failed_hand_written_serialized_payload_fails() {
    let code = r#"
        let evt = AgentRunEvent {
            event_type: AgentRunEventType::ReplayFailed,
            payload: serde_json::json!({
                "status": "failed",
                "run_id": "r1",
                "action_id": "a1",
                "block_reason": "replay_spec_missing",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ReplayFailed",
        &["build_replay_failed_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "hand-written json! payload without builder should be a violation"
    );
}

#[test]
fn positive_replay_completed_with_builder_passes() {
    let code = r#"
        let p = trace_payloads::build_replay_completed_payload(
            "completed", "r1", "a1", "orig", "spec", "tool", "src",
            None::<&str>, None::<&str>, None::<&str>,
        );
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ReplayCompleted,
            payload: p,
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ReplayCompleted",
        &["build_replay_completed_payload"],
        code,
        900,
    );
    assert!(
        violations.is_empty(),
        "expected no violation: {:?}",
        violations
    );
}

#[test]
fn negative_replay_failed_using_completed_builder_is_violation() {
    let code = r#"
        let p = trace_payloads::build_replay_completed_payload(
            "failed", "r1", "a1", "orig", "spec", "tool", "src",
            Some("agent_spec_denied"), None::<&str>, None::<&str>,
        );
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ReplayFailed,
            payload: p,
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ReplayFailed",
        &["build_replay_failed_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "ReplayFailed using build_replay_completed_payload must be a violation"
    );
}

#[test]
fn positive_replay_failed_with_correct_builder_passes() {
    let code = r#"
        let p = trace_payloads::build_replay_failed_payload(
            "r1", "a1", "orig", "Run not found",
            Some("replay_spec_missing"),
            None::<&str>,
            None,
        );
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ReplayFailed,
            payload: p,
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ReplayFailed",
        &["build_replay_failed_payload"],
        code,
        900,
    );
    assert!(
        violations.is_empty(),
        "ReplayFailed using build_replay_failed_payload should pass: {:?}",
        violations,
    );
}

#[test]
fn negative_agent_spec_selected_without_builder_fails() {
    let code = r#"
        let evt = AgentRunEvent {
            event_type: AgentRunEventType::AgentSpecSelected,
            payload: json!({"agent_spec_id": "x", "role": "y", "privacy_policy": "z"}),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::AgentSpecSelected",
        &["build_agent_spec_selected_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "expected violation for AgentSpecSelected without builder"
    );
}

#[test]
fn positive_context_governance_applied_with_builder_passes() {
    let code = r#"
        let p = trace_payloads::build_context_governance_applied_payload(
            "spec", vec!["a".into()], vec!["b".into()], "local",
            ContextGovernanceEmitter::StreamingExecution,
        );
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ContextGovernanceApplied,
            payload: p,
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ContextGovernanceApplied",
        &["build_context_governance_applied_payload"],
        code,
        900,
    );
    assert!(
        violations.is_empty(),
        "expected no violation: {:?}",
        violations
    );
}

#[test]
fn builder_too_far_outside_window_is_violation() {
    let padding = "x".repeat(400);
    let code = format!(
        "let p = trace_payloads::build_agent_spec_selected_payload(\"a\", \"b\", \"c\");\n{}\nlet event = AgentRunEventType::AgentSpecSelected;",
        padding
    );

    let violations = audit_snippet(
        "AgentRunEventType::AgentSpecSelected",
        &["build_agent_spec_selected_payload"],
        &code,
        200,
    );
    assert!(
        !violations.is_empty(),
        "expected violation when builder is outside the {}-char window, got none",
        200,
    );
}

#[test]
fn test_region_content_is_excluded() {
    let code = r#"
        let p = trace_payloads::build_tool_call_blocked_payload("b", "t", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
        #[cfg(test)]
        mod tests {
            let _ = AgentRunEventType::ToolCallBlocked;
        }
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        Some(1), // exact: 1 production emission
        1,
        900,
    );
    let violations = audit_file(&rule, code);
    assert!(
        violations.is_empty(),
        "test-region event should be excluded from audit: {:?}",
        violations,
    );
}

#[test]
fn min_emissions_warning_when_too_few_production_emissions() {
    let code = r#"
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "t", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let event = AgentRunEventType::ToolCallBlocked;
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        None, // no exact count → fall back to min_emissions
        3,    // expect at least 3 — but only 1 exists
        900,
    );
    let violations = audit_file(&rule, code);
    assert_eq!(violations.len(), 1, "expected 1 min-emissions warning");
    assert!(
        violations[0].contains("expected at least 3"),
        "warning should mention min_emissions: {}",
        violations[0],
    );
}

#[test]
fn audit_does_not_flag_unrelated_serde_json_invocation() {
    let code = r#"
        let some_config = serde_json::json!({"key": "value"});
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "x", "y", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let event = AgentRunEventType::ToolCallBlocked;
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        violations.is_empty(),
        "unrelated json! should not cause false positives: {:?}",
        violations,
    );
}

#[test]
fn expected_emissions_fails_on_count_mismatch() {
    let code = r#"
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "t1", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
        let p2 = trace_payloads::build_tool_call_blocked_payload("blocked", "t2", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        Some(3), // expect exactly 3 — but only 2 exist
        1,
        900,
    );
    let violations = audit_file(&rule, code);
    assert_eq!(violations.len(), 1, "expected 1 count-mismatch violation");
    assert!(
        violations[0].contains("expected exactly 3"),
        "violation should mention exact count: {}",
        violations[0],
    );
}

// ────────────────────────────────────────────────────────────────────
// NEW negative tests — source sanitisation (mask comments & strings)
// ────────────────────────────────────────────────────────────────────

/// Event token inside a line comment must NOT contribute to the emission
/// count and must NOT trigger a builder-missing violation.
#[test]
fn event_token_in_line_comment_is_ignored() {
    let code = r#"
        // This comment mentions AgentRunEventType::ToolCallBlocked for docs
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "t", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        Some(1), // exactly 1 non-comment emission
        1,
        900,
    );
    let violations = audit_file(&rule, code);
    assert!(
        violations.is_empty(),
        "event token in line comment must be ignored: {:?}",
        violations,
    );
}

/// Event token inside a string literal must NOT be counted.
#[test]
fn event_token_in_string_literal_is_ignored() {
    let code = r#"
        let desc = "Event is AgentRunEventType::ToolCallBlocked";
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "t", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        Some(1), // exactly 1 real emission
        1,
        900,
    );
    let violations = audit_file(&rule, code);
    assert!(
        violations.is_empty(),
        "event token in string literal must be ignored: {:?}",
        violations,
    );
}

/// Builder name inside a line comment alongside a real event that uses
/// json! (no builder) must NOT create a false pass.
#[test]
fn builder_in_line_comment_does_not_pass_real_event() {
    let code = r#"
        // See build_tool_call_blocked_payload in trace_payloads.rs for docs
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "builder only in comment should not pass audit: violations={:?}",
        violations,
    );
}

/// Builder name inside a string literal (e.g. a diagnostic message)
/// alongside a real event that uses json! must NOT create a false pass.
#[test]
fn builder_in_string_literal_does_not_pass_real_event() {
    let code = r#"
        println!("call build_tool_call_blocked_payload");
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "builder only in string literal should not pass audit: violations={:?}",
        violations,
    );
}

/// Builder name inside a raw string literal must NOT create a false pass.
#[test]
fn builder_in_raw_string_does_not_pass_real_event() {
    let code = r##"
        let s = r#"use build_tool_call_blocked_payload to construct the payload"#;
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "##;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "builder only in raw string should not pass audit: violations={:?}",
        violations,
    );
}

/// Builder name inside a block comment must NOT create a false pass.
#[test]
fn builder_in_block_comment_does_not_pass_real_event() {
    let code = r#"
        /* For blocked events, always call build_tool_call_blocked_payload */
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "builder only in block comment should not pass audit: violations={:?}",
        violations,
    );
}

/// Event token inside a block comment must NOT trigger a builder-missing
/// violation or affect the emission count.
#[test]
fn event_token_in_block_comment_is_ignored() {
    let code = r#"
        /* AgentRunEventType::ToolCallBlocked means the tool was blocked. */
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "t", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        Some(1),
        1,
        900,
    );
    let violations = audit_file(&rule, code);
    assert!(
        violations.is_empty(),
        "event token in block comment must be ignored: {:?}",
        violations,
    );
}

/// Narrow window prevents adjacent-event builder from masking a missing
/// builder on a different event type.
#[test]
fn adjacent_event_with_different_builder_not_masked() {
    // Event A (ToolCallBlocked) uses json! (no builder).
    // Event B (AgentSpecSelected) uses build_agent_spec_selected_payload.
    // The audit for ToolCallBlocked only accepts build_tool_call_blocked_payload
    // — the AgentSpecSelected builder in the window must NOT pass the
    // ToolCallBlocked audit.
    let code = r#"
        let p = trace_payloads::build_agent_spec_selected_payload("spec", "role", "local");
        let _ = AgentRunEventType::AgentSpecSelected;
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "adjacent event's builder must not mask missing builder on a different event: {:?}",
        violations,
    );
}

/// Raw string with double-hash delimiters (r##"…"##) containing a builder
/// must be correctly masked.
#[test]
fn builder_in_raw_string_double_hash_does_not_pass() {
    let code = r###"
        let s = r##"call build_tool_call_blocked_payload"##;
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "###;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "builder only in r## raw string should not pass: {:?}",
        violations,
    );
}

/// Regular string containing escape sequence for a quote (`\"`)
/// must NOT leak the following characters into non-string territory.
#[test]
fn string_with_escaped_quote_is_fully_masked() {
    // The string `"foo \" bar"` includes an escaped quote.  The
    // sanitised output must mask the entire string including the
    // backslash and the escaped quote.
    let code = r#"
        let msg = "ignored \" AgentRunEventType::ToolCallBlocked";
        let event = AgentRunEvent {
            event_type: AgentRunEventType::ToolCallBlocked,
            payload: serde_json::json!({
                "status": "blocked",
                "tool_name": "x",
                "source": "builtin",
            }),
        };
    "#;

    let violations = audit_snippet(
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        code,
        900,
    );
    assert!(
        !violations.is_empty(),
        "string with escaped quote should not leak event token to non-string: {:?}",
        violations,
    );
}

/// Block comment containing a `#[cfg(test)]`-like string must NOT be
/// mistaken for a real test-region split.
#[test]
fn test_region_detection_ignores_cfg_test_in_comment() {
    // A comment mentioning `#[cfg(test)]` must not trigger the test-region
    // split.  The real `#[cfg(test)]` is later.
    let code = r#"
        // Always put tests inside #[cfg(test)] mod
        let p = trace_payloads::build_tool_call_blocked_payload("blocked", "t", "s", None::<&str>, None::<&str>, None::<&str>, None::<&str>, None);
        let _ = AgentRunEventType::ToolCallBlocked;
        #[cfg(test)]
        mod tests {
            // test-only hand-written json! — must be excluded
            let _ = AgentRunEventType::ToolCallBlocked;
        }
    "#;

    let rule = AuditRule::new(
        "synthetic",
        "AgentRunEventType::ToolCallBlocked",
        &["build_tool_call_blocked_payload"],
        Some(1),
        1,
        900,
    );
    let violations = audit_file(&rule, code);
    assert!(
        violations.is_empty(),
        "cfg(test) in comment must not falsely trigger test-region split: {:?}",
        violations,
    );
}

// ════════════════════════════════════════════════════════════════════
// Event Contract Coverage Manifest
// ════════════════════════════════════════════════════════════════════
//
// Every AgentRunEventType variant MUST appear in the manifest below
// with a classification and a reason.  If a future developer adds a new
// enum variant without updating this manifest, the coverage tests will
// fail.
// ════════════════════════════════════════════════════════════════════

/// Classification of an `AgentRunEventType` variant relative to the
/// trace contract audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventContractStatus {
    /// Event has a typed payload builder in `trace_payloads.rs` AND at
    /// least one `AuditRule` in `audit_rules()`.
    ProductionAudited,
    /// Event exists and is emitted in production, but intentionally
    /// excluded from the audit for a documented reason.
    IntentionallyExcluded,
    /// Runtime lifecycle / infrastructure event that does not carry
    /// typed governance payloads.
    LegacyInternalOnly,
    /// Enum variant never emitted directly by production code.
    TypeOnlyNoDirectEmission,
}

/// One entry in the event contract manifest.
struct EventContractEntry {
    event: &'static str,
    status: EventContractStatus,
    reason: &'static str,
    /// For `ProductionAudited` entries only — event tokens in `audit_rules()`.
    production_rule_tokens: &'static [&'static str],
}

/// The single source of truth for which `AgentRunEventType` variant
/// belongs to which contract tier.
#[rustfmt::skip]
fn event_contract_manifest() -> Vec<EventContractEntry> {
    vec![
        // ── Production Audited (typed builder + AuditRule) ────────
        EventContractEntry { event: "ToolCallBlocked", status: EventContractStatus::ProductionAudited, reason: "governance event: tool execution blocked or needs confirmation; requires typed block_reason/proposal_reason", production_rule_tokens: &["AgentRunEventType::ToolCallBlocked"] },
        EventContractEntry { event: "ReplayFailed", status: EventContractStatus::ProductionAudited, reason: "governance event: replay action failed; requires typed block_reason/failure_kind", production_rule_tokens: &["AgentRunEventType::ReplayFailed"] },
        EventContractEntry { event: "ReplayStarted", status: EventContractStatus::ProductionAudited, reason: "governance event: replay action lifecycle; requires typed run_id/action_id/agent_spec_id", production_rule_tokens: &["AgentRunEventType::ReplayStarted"] },
        EventContractEntry { event: "ReplayCompleted", status: EventContractStatus::ProductionAudited, reason: "governance event: replay action lifecycle; requires typed outcome_status and optional block_reason/proposal_reason/failure_kind", production_rule_tokens: &["AgentRunEventType::ReplayCompleted"] },
        EventContractEntry { event: "AgentSpecSelected", status: EventContractStatus::ProductionAudited, reason: "governance event: records which AgentSpec was resolved for this run; required for explainability metadata", production_rule_tokens: &["AgentRunEventType::AgentSpecSelected"] },
        EventContractEntry { event: "PromptStackAssembled", status: EventContractStatus::ProductionAudited, reason: "governance event: records prompt block trace for explainability; required for prompt auditability", production_rule_tokens: &["AgentRunEventType::PromptStackAssembled"] },
        EventContractEntry { event: "ContextGovernanceApplied", status: EventContractStatus::ProductionAudited, reason: "governance event: records context governance decision (included/excluded blocks, privacy policy); required for context auditability", production_rule_tokens: &["AgentRunEventType::ContextGovernanceApplied"] },
        // ── Intentionally Excluded (typed builder, no audit) ──────
        EventContractEntry { event: "ModelFailed", status: EventContractStatus::IntentionallyExcluded, reason: "generic model-level error; has typed builder (build_model_failed_payload) but carries error info, not governance decisions", production_rule_tokens: &[] },
        EventContractEntry { event: "RunFailed", status: EventContractStatus::IntentionallyExcluded, reason: "generic run-level error; has typed builder (build_run_failed_payload) but carries error info, not governance decisions", production_rule_tokens: &[] },
        EventContractEntry { event: "ToolCallFailed", status: EventContractStatus::IntentionallyExcluded, reason: "generic tool error; has typed builder (build_tool_call_failed_payload) but carries error info, not governance decisions", production_rule_tokens: &[] },
        EventContractEntry { event: "ModelCallFailed", status: EventContractStatus::IntentionallyExcluded, reason: "generic model call error; has typed builder (build_model_call_failed_payload) but carries error info, not governance decisions", production_rule_tokens: &[] },
        // ── Legacy / Internal Only (no typed governance payload) ──
        EventContractEntry { event: "RunCreated", status: EventContractStatus::LegacyInternalOnly, reason: "runtime lifecycle event; carries session/run metadata, not typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "ContextAssembled", status: EventContractStatus::LegacyInternalOnly, reason: "internal assembly trace; carries block selection metadata, not typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "ModelRouteSelected", status: EventContractStatus::LegacyInternalOnly, reason: "model routing trace; carries provider/model selection, not typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "ModelCallStarted", status: EventContractStatus::LegacyInternalOnly, reason: "model call lifecycle event; carries timing/token metadata", production_rule_tokens: &[] },
        EventContractEntry { event: "ModelCallCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "model call lifecycle event; carries timing/token metadata", production_rule_tokens: &[] },
        EventContractEntry { event: "ToolCallStarted", status: EventContractStatus::LegacyInternalOnly, reason: "tool execution lifecycle event; carries tool name and parameters", production_rule_tokens: &[] },
        EventContractEntry { event: "ToolCallCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "tool execution lifecycle event; carries observation/result metadata", production_rule_tokens: &[] },
        EventContractEntry { event: "ObservationCreated", status: EventContractStatus::LegacyInternalOnly, reason: "internal ReAct observation trace; carries tool observation, not governance", production_rule_tokens: &[] },
        EventContractEntry { event: "ProposalCreated", status: EventContractStatus::LegacyInternalOnly, reason: "internal proposal tracking; carries proposal metadata, not governance", production_rule_tokens: &[] },
        EventContractEntry { event: "FallbackStarted", status: EventContractStatus::LegacyInternalOnly, reason: "fallback execution trace; carries fallback reason, not typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "FallbackCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "fallback execution trace; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "JsonRepairStarted", status: EventContractStatus::LegacyInternalOnly, reason: "JSON self-repair trace; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "JsonRepairCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "JSON self-repair trace; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "RunCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "runtime lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "CompactionCreated", status: EventContractStatus::LegacyInternalOnly, reason: "context compaction trace; carries compaction metadata, not typed governance payload", production_rule_tokens: &[] },
        // ── Plan lifecycle events ──
        EventContractEntry { event: "PlanCreated", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; carries plan metadata, not typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanConfirmationRequested", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanConfirmationResolved", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanExecutionStarted", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanStepStarted", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanStepCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanStepFailed", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanDeviationRecorded", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanExecutionCompleted", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanExecutionFailed", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanCancelRequested", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanCancelled", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanRetryRequested", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanRetryStarted", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanContinuationRequested", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanActionReplayed", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        EventContractEntry { event: "PlanActionReplayRequested", status: EventContractStatus::LegacyInternalOnly, reason: "plan lifecycle event; no typed governance payload", production_rule_tokens: &[] },
        // ── Type-only / never directly emitted ──
        EventContractEntry { event: "Unknown", status: EventContractStatus::TypeOnlyNoDirectEmission, reason: "forward-compat catch-all; never directly emitted by production code; deserialised by older builds reading newer traces", production_rule_tokens: &[] },
    ]
}

// ────────────────────────────────────────────────────────────────────
// Enum variant extraction from source
// ────────────────────────────────────────────────────────────────────

/// Extract `AgentRunEventType` variant names from `types/mod.rs`
/// using sanitised source scanning.
fn parse_agent_run_event_type_variants() -> Vec<String> {
    let content = read_workspace_file("openlife-core/src/agent/types/mod.rs");
    let sanitised = sanitize_source(&content);

    let enum_start = sanitised
        .find("pub enum AgentRunEventType")
        .expect("AgentRunEventType enum not found in types/mod.rs");
    let body_start = sanitised[enum_start..]
        .find('{')
        .map(|p| enum_start + p + 1)
        .expect("opening brace not found for AgentRunEventType enum");

    let mut depth = 1i32;
    let mut body_end = body_start;
    for (i, ch) in sanitised[body_start..].char_indices() {
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                body_end = body_start + i;
                break;
            }
        }
    }

    let body = &sanitised[body_start..body_end];
    let mut variants = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("///") {
            continue;
        }
        if let Some(rest) = trimmed.strip_suffix(',') {
            let ident = rest.trim();
            let name = if let Some(paren) = ident.find('(') {
                &ident[..paren]
            } else {
                ident
            };
            let name = name.trim();
            if !name.is_empty()
                && name
                    .chars()
                    .next()
                    .map_or(false, |c| c.is_ascii_uppercase())
            {
                variants.push(name.to_string());
            }
        }
    }
    variants
}

// ════════════════════════════════════════════════════════════════════
// Validator functions — pure, reusable, callable from both positive
// and negative tests.
// ════════════════════════════════════════════════════════════════════

/// Validate that every enum variant has a manifest entry, and no
/// manifest entry is stale.
///
/// Returns `Ok(())` on success, or `Err(errors)` with one or more
/// human-readable messages.
fn validate_manifest_against_enum(
    variants: &[String],
    manifest: &[EventContractEntry],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let manifest_names: std::collections::HashSet<&str> =
        manifest.iter().map(|e| e.event).collect();
    let variant_set: std::collections::HashSet<&str> =
        variants.iter().map(|s| s.as_str()).collect();

    // Missing: variant not in manifest.
    let mut missing = Vec::new();
    for v in variants {
        if !manifest_names.contains(v.as_str()) {
            missing.push(v.clone());
        }
    }
    if !missing.is_empty() {
        errors.push(format!(
            "{} enum variant(s) missing from manifest: {}",
            missing.len(),
            missing.join(", "),
        ));
    }

    // Stale: manifest entry not in enum.
    let mut stale = Vec::new();
    for e in manifest {
        if !variant_set.contains(e.event) {
            stale.push(e.event.to_string());
        }
    }
    if !stale.is_empty() {
        errors.push(format!(
            "{} manifest entry(s) stale (variant removed from enum): {}",
            stale.len(),
            stale.join(", "),
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate that every `ProductionAudited` manifest entry has a
/// matching `AuditRule`, and that every audit rule maps to a
/// `ProductionAudited` manifest entry.
fn validate_manifest_against_audit_rules(
    manifest: &[EventContractEntry],
    rules: &[AuditRule],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let prefix = "AgentRunEventType::";

    // Build lookup: event_token → true
    let rule_tokens: std::collections::HashSet<&str> =
        rules.iter().map(|r| r.event_token).collect();

    // Build set of short names from ProductionAudited entries.
    let manifest_pa: std::collections::HashSet<&str> = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::ProductionAudited)
        .map(|e| e.event)
        .collect();

    // ProductionAudited entries without matching rule.
    for entry in manifest {
        if entry.status != EventContractStatus::ProductionAudited {
            continue;
        }
        if entry.production_rule_tokens.is_empty() {
            errors.push(format!(
                "ProductionAudited entry '{}' has empty production_rule_tokens",
                entry.event,
            ));
            continue;
        }
        for token in entry.production_rule_tokens {
            if !rule_tokens.contains(token) {
                errors.push(format!(
                    "ProductionAudited entry '{}' references rule token '{}' that has no AuditRule",
                    entry.event, token,
                ));
            }
        }
    }

    // Audit rules without ProductionAudited manifest entry.
    for rule in rules {
        let short = rule
            .event_token
            .strip_prefix(prefix)
            .unwrap_or(rule.event_token);
        if !manifest_pa.contains(short) {
            errors.push(format!(
                "AuditRule for '{}' has no ProductionAudited manifest entry",
                rule.event_token,
            ));
        }
    }

    // ProductionAudited count vs unique audited tokens.
    let pa_count = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::ProductionAudited)
        .count();
    let unique_tokens = rule_tokens.len();
    if pa_count != unique_tokens {
        errors.push(format!(
            "ProductionAudited manifest entries ({}) != unique audited event tokens ({})",
            pa_count, unique_tokens,
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Check that every `IntentionallyExcluded` entry has a non-empty reason.
fn validate_intentionally_excluded_reasons(
    manifest: &[EventContractEntry],
) -> Result<(), Vec<String>> {
    let empty: Vec<&str> = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::IntentionallyExcluded && e.reason.is_empty())
        .map(|e| e.event)
        .collect();
    if empty.is_empty() {
        Ok(())
    } else {
        Err(vec![format!(
            "{} IntentionallyExcluded entry(s) have empty reason: {}",
            empty.len(),
            empty.join(", "),
        )])
    }
}

/// Check no duplicate event names in the manifest.
fn validate_no_duplicate_events(manifest: &[EventContractEntry]) -> Result<(), Vec<String>> {
    let mut seen = std::collections::HashSet::new();
    let mut dupes = Vec::new();
    for e in manifest {
        if !seen.insert(e.event) {
            dupes.push(e.event);
        }
    }
    if dupes.is_empty() {
        Ok(())
    } else {
        Err(vec![format!(
            "duplicate event name(s) in manifest: {:?}",
            dupes
        )])
    }
}

// ════════════════════════════════════════════════════════════════════
// Document consistency validation
// ════════════════════════════════════════════════════════════════════

/// Parsed row from the classification summary table in Section 9.5.3.
struct DocTierRow {
    tier_name: String,
    count: usize,
    events: Vec<String>,
}

/// Result of parsing the classification summary table.
struct DocTable {
    rows: Vec<DocTierRow>,
    /// The explicit count from the `| **Total** | **N** | |` row, if present.
    explicit_total: Option<usize>,
}

/// Parse the classification summary table from `plans/openlife_trace_contract_matrix.md`.
fn parse_document_tier_table(doc: &str) -> Option<DocTable> {
    let section = doc.find("## 9.5 AgentRunEvent Contract Coverage")?;
    let section = &doc[section..];

    let table_start = section.find("### 9.5.3 Current Event Classification Summary")?;
    let table_section = &section[table_start..];

    let mut rows = Vec::new();
    let mut explicit_total: Option<usize> = None;

    for line in table_section.lines().skip(1) {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if !rows.is_empty() {
                break;
            }
            continue;
        }
        if trimmed.starts_with("|---") || trimmed.starts_with("| Tier") {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|s| s.trim())
            .collect();
        if cells.len() < 3 {
            continue;
        }
        let name = cells[0].to_string();
        let mut count_str = cells[1].to_string();
        // Strip markdown bold markers (**…**) from cell content.
        count_str.retain(|c| c != '*');
        let count: usize = count_str.parse().unwrap_or(0);

        if name == "**Total**" || name == "Total" {
            explicit_total = Some(count);
            continue;
        }

        let events: Vec<String> = cells[2]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        rows.push(DocTierRow {
            tier_name: name,
            count,
            events,
        });
    }

    if rows.is_empty() {
        return None;
    }
    Some(DocTable {
        rows,
        explicit_total,
    })
}

/// Validate the document against the manifest and enum variants.
fn validate_document_against_manifest(
    doc: &str,
    manifest: &[EventContractEntry],
    variants: &[String],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // 1. Section heading must exist.
    if !doc.contains("## 9.5 AgentRunEvent Contract Coverage") {
        errors.push(
            "## 9.5 AgentRunEvent Contract Coverage section heading not found in document".into(),
        );
    }

    // 2. Check for forbidden stale strings.
    for forbidden in &["all 45 AgentRunEventType variants", "Total | **45**"] {
        if doc.contains(forbidden) {
            errors.push(format!(
                "document contains forbidden stale string: '{}'",
                forbidden,
            ));
        }
    }

    // 3. Parse the summary table — must succeed.
    let table = match parse_document_tier_table(doc) {
        Some(t) => t,
        None => {
            errors.push(
                "document is missing the classification summary table \
                 (### 9.5.3 Current Event Classification Summary)"
                    .into(),
            );
            return Err(errors);
        }
    };

    // 4. Total row must be present.
    let doc_explicit_total = match table.explicit_total {
        Some(t) => t,
        None => {
            errors.push("document classification table is missing the Total row".into());
            // Continue checking tier rows even if Total is missing.
            0
        }
    };

    // 5. Build manifest sets per tier.
    let manifest_pa: std::collections::HashSet<&str> = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::ProductionAudited)
        .map(|e| e.event)
        .collect();
    let manifest_ie: std::collections::HashSet<&str> = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::IntentionallyExcluded)
        .map(|e| e.event)
        .collect();
    let manifest_li: std::collections::HashSet<&str> = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::LegacyInternalOnly)
        .map(|e| e.event)
        .collect();
    let manifest_to: std::collections::HashSet<&str> = manifest
        .iter()
        .filter(|e| e.status == EventContractStatus::TypeOnlyNoDirectEmission)
        .map(|e| e.event)
        .collect();

    for row in &table.rows {
        let doc_set: std::collections::HashSet<&str> =
            row.events.iter().map(|s| s.as_str()).collect();

        if doc_set.len() != row.events.len() {
            let mut seen = std::collections::HashSet::new();
            let mut dupes = Vec::new();
            for ev in &row.events {
                if !seen.insert(ev.as_str()) {
                    dupes.push(ev.clone());
                }
            }
            errors.push(format!(
                "document tier '{}' has duplicate event(s): {}",
                row.tier_name,
                dupes.join(", "),
            ));
        }

        let expected_set: &std::collections::HashSet<&str> = match row.tier_name.as_str() {
            "ProductionAudited" => &manifest_pa,
            "IntentionallyExcluded" => &manifest_ie,
            "LegacyInternalOnly" => &manifest_li,
            "TypeOnlyNoDirectEmission" => &manifest_to,
            _ => continue,
        };

        if row.count != expected_set.len() {
            errors.push(format!(
                "document tier '{}' count {} but manifest has {} events",
                row.tier_name,
                row.count,
                expected_set.len(),
            ));
        }

        let doc_extra: Vec<&String> = row
            .events
            .iter()
            .filter(|e| !expected_set.contains(e.as_str()))
            .collect();
        if !doc_extra.is_empty() {
            errors.push(format!(
                "document tier '{}' has {} event(s) not in manifest: {}",
                row.tier_name,
                doc_extra.len(),
                doc_extra
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        let manifest_extra: Vec<String> = expected_set
            .iter()
            .filter(|e| !doc_set.contains(**e))
            .map(|e| e.to_string())
            .collect();
        if !manifest_extra.is_empty() {
            errors.push(format!(
                "manifest tier '{}' has {} event(s) missing from document: {}",
                row.tier_name,
                manifest_extra.len(),
                manifest_extra.join(", "),
            ));
        }
    }

    // 6. Cross-validate totals from multiple sources.
    let tier_sum: usize = table.rows.iter().map(|r| r.count).sum();

    if doc_explicit_total > 0 && doc_explicit_total != tier_sum {
        errors.push(format!(
            "document explicit total {} != sum of tier counts {}",
            doc_explicit_total, tier_sum,
        ));
    }
    if doc_explicit_total > 0 && doc_explicit_total != variants.len() {
        errors.push(format!(
            "document explicit total {} != enum variant count {}",
            doc_explicit_total,
            variants.len(),
        ));
    }
    if doc_explicit_total > 0 && doc_explicit_total != manifest.len() {
        errors.push(format!(
            "document explicit total {} != manifest entry count {}",
            doc_explicit_total,
            manifest.len(),
        ));
    }
    // Tier-sum checks (run even if Total row is missing).
    if tier_sum != variants.len() {
        errors.push(format!(
            "document tier sum {} != enum variant count {}",
            tier_sum,
            variants.len(),
        ));
    }
    if tier_sum != manifest.len() {
        errors.push(format!(
            "document tier sum {} != manifest entry count {}",
            tier_sum,
            manifest.len(),
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ════════════════════════════════════════════════════════════════════
// Positive (real-data) coverage tests — named with event_contract_ *
// so `cargo test -p openlife-core event_contract` runs them.
// ════════════════════════════════════════════════════════════════════

#[test]
fn event_contract_all_enum_variants_have_manifest_entry() {
    let variants = parse_agent_run_event_type_variants();
    let manifest = event_contract_manifest();
    let result = validate_manifest_against_enum(&variants, &manifest);
    assert!(
        result.is_ok(),
        "validate_manifest_against_enum failed: {:?}",
        result.unwrap_err(),
    );
}

#[test]
fn event_contract_production_audited_events_have_audit_rules() {
    let manifest = event_contract_manifest();
    let rules = audit_rules();
    let result = validate_manifest_against_audit_rules(&manifest, &rules);
    assert!(
        result.is_ok(),
        "validate_manifest_against_audit_rules failed: {:?}",
        result.unwrap_err(),
    );
}

#[test]
fn event_contract_intentionally_excluded_have_reason() {
    let manifest = event_contract_manifest();
    let result = validate_intentionally_excluded_reasons(&manifest);
    assert!(
        result.is_ok(),
        "validate_intentionally_excluded_reasons failed: {:?}",
        result.unwrap_err(),
    );
}

#[test]
fn event_contract_no_duplicate_events() {
    let manifest = event_contract_manifest();
    let result = validate_no_duplicate_events(&manifest);
    assert!(
        result.is_ok(),
        "validate_no_duplicate_events failed: {:?}",
        result.unwrap_err(),
    );
}

#[test]
fn event_contract_document_matches_manifest() {
    let doc = read_workspace_file("plans/openlife_trace_contract_matrix.md");
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(&doc, &manifest, &variants);
    assert!(
        result.is_ok(),
        "validate_document_against_manifest failed:\n{}",
        result.unwrap_err().join("\n"),
    );
}

#[test]
fn event_contract_parse_enum_finds_44_variants() {
    let variants = parse_agent_run_event_type_variants();
    // Exact count — will fail if variants are added/removed, forcing
    // manifest and document updates.
    assert_eq!(
        variants.len(),
        44,
        "AgentRunEventType has {} variants; expected exactly 44.  \
         If this changed, update the manifest and document.",
        variants.len(),
    );
    assert!(
        variants.iter().any(|v| v == "Unknown"),
        "must find Unknown variant",
    );
}

#[test]
fn event_contract_parse_enum_sanitised_no_paren_in_names() {
    let variants = parse_agent_run_event_type_variants();
    let bad: Vec<_> = variants.iter().filter(|v| v.contains('(')).collect();
    assert!(
        bad.is_empty(),
        "parsed variants must not contain parenthesised type data: {:?}",
        bad,
    );
}

// ════════════════════════════════════════════════════════════════════
// Negative tests — construct bad inputs, call validators, assert fail.
// ════════════════════════════════════════════════════════════════════

#[test]
fn event_contract_missing_manifest_entry_fails() {
    let mut variants = parse_agent_run_event_type_variants();
    variants.push("SyntheticNewEvent".into());
    let manifest = event_contract_manifest();
    let result = validate_manifest_against_enum(&variants, &manifest);
    assert!(
        result.is_err(),
        "should fail when variant has no manifest entry"
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("missing from manifest"),
        "error should mention 'missing from manifest': {}",
        err,
    );
    assert!(
        err.contains("SyntheticNewEvent"),
        "error should name the missing variant: {}",
        err,
    );
}

#[test]
fn event_contract_stale_manifest_entry_fails() {
    let variants = parse_agent_run_event_type_variants();
    let mut manifest = event_contract_manifest();
    manifest.push(EventContractEntry {
        event: "SyntheticRemovedEvent",
        status: EventContractStatus::LegacyInternalOnly,
        reason: "test-only stale entry",
        production_rule_tokens: &[],
    });
    let result = validate_manifest_against_enum(&variants, &manifest);
    assert!(result.is_err(), "should fail when manifest has stale entry");
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("stale"),
        "error should mention 'stale': {}",
        err,
    );
    assert!(
        err.contains("SyntheticRemovedEvent"),
        "error should name the stale entry: {}",
        err,
    );
}

#[test]
fn event_contract_production_audited_without_rule_fails() {
    let mut manifest = event_contract_manifest();
    manifest.push(EventContractEntry {
        event: "SyntheticAudited",
        status: EventContractStatus::ProductionAudited,
        reason: "test",
        production_rule_tokens: &["AgentRunEventType::NoSuchRule"],
    });
    let rules = audit_rules();
    let result = validate_manifest_against_audit_rules(&manifest, &rules);
    assert!(
        result.is_err(),
        "should fail when ProductionAudited has no matching rule",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("NoSuchRule") || err.contains("SyntheticAudited"),
        "error should reference the bad entry: {}",
        err,
    );
}

#[test]
fn event_contract_audit_rule_without_production_manifest_fails() {
    // Build a manifest with no ProductionAudited entries, and a rule
    // for an event that isn't in the manifest as ProductionAudited.
    let mut manifest = event_contract_manifest();
    // Demote ReplayFailed from ProductionAudited to LegacyInternalOnly.
    for entry in &mut manifest {
        if entry.event == "ReplayFailed" {
            entry.status = EventContractStatus::LegacyInternalOnly;
        }
    }
    let rules = audit_rules();
    let result = validate_manifest_against_audit_rules(&manifest, &rules);
    assert!(
        result.is_err(),
        "should fail when an AuditRule has no ProductionAudited manifest entry",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("ReplayFailed") || err.contains("AuditRule"),
        "error should reference the orphan rule: {}",
        err,
    );
}

#[test]
fn event_contract_intentionally_excluded_empty_reason_fails() {
    let mut manifest = event_contract_manifest();
    // Wipe reason on ModelFailed.
    for entry in &mut manifest {
        if entry.event == "ModelFailed" {
            entry.reason = "";
        }
    }
    let result = validate_intentionally_excluded_reasons(&manifest);
    assert!(
        result.is_err(),
        "should fail when IntentionallyExcluded has empty reason",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("empty reason"),
        "error should mention 'empty reason': {}",
        err,
    );
}

#[test]
fn event_contract_document_missing_summary_table_fails() {
    let doc = "## 9.5 AgentRunEvent Contract Coverage\n\
                Some text but no 9.5.3 table.\n";
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(doc, &manifest, &variants);
    assert!(
        result.is_err(),
        "should fail when classification summary table is missing",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("classification summary"),
        "error should mention 'classification summary': {}",
        err,
    );
}

#[test]
fn event_contract_document_missing_total_row_fails() {
    let doc = "## 9.5 AgentRunEvent Contract Coverage\n\
                ### 9.5.3 Current Event Classification Summary\n\
                | Tier | Count | Events |\n\
                |---|---|---|\n\
                | ProductionAudited | 7 | ToolCallBlocked, ReplayFailed, ReplayStarted, ReplayCompleted, AgentSpecSelected, PromptStackAssembled, ContextGovernanceApplied |\n";
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(doc, &manifest, &variants);
    assert!(result.is_err(), "should fail when Total row is missing",);
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("Total row"),
        "error should mention 'Total row': {}",
        err,
    );
}

#[test]
fn event_contract_document_total_mismatch_fails() {
    // All four tier rows are correct, but the Total row has the wrong count.
    let doc = "## 9.5 AgentRunEvent Contract Coverage\n\
                ### 9.5.3 Current Event Classification Summary\n\
                | Tier | Count | Events |\n\
                |---|---|---|\n\
                | ProductionAudited | 7 | ToolCallBlocked, ReplayFailed, ReplayStarted, ReplayCompleted, AgentSpecSelected, PromptStackAssembled, ContextGovernanceApplied |\n\
                | IntentionallyExcluded | 4 | ModelFailed, RunFailed, ToolCallFailed, ModelCallFailed |\n\
                | LegacyInternalOnly | 32 | RunCreated, ContextAssembled, ModelRouteSelected, ModelCallStarted, ModelCallCompleted, ToolCallStarted, ToolCallCompleted, ObservationCreated, ProposalCreated, FallbackStarted, FallbackCompleted, JsonRepairStarted, JsonRepairCompleted, RunCompleted, CompactionCreated, PlanCreated, PlanConfirmationRequested, PlanConfirmationResolved, PlanExecutionStarted, PlanStepStarted, PlanStepCompleted, PlanStepFailed, PlanDeviationRecorded, PlanExecutionCompleted, PlanExecutionFailed, PlanCancelRequested, PlanCancelled, PlanRetryRequested, PlanRetryStarted, PlanContinuationRequested, PlanActionReplayed, PlanActionReplayRequested |\n\
                | TypeOnlyNoDirectEmission | 1 | Unknown |\n\
                | **Total** | **99** | |\n";
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(doc, &manifest, &variants);
    assert!(
        result.is_err(),
        "should fail when Total row count is wrong even though tier rows are correct",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("99"),
        "error should mention the wrong total '99': {}",
        err,
    );
}

#[test]
fn event_contract_document_duplicate_event_fails() {
    let doc = "## 9.5 AgentRunEvent Contract Coverage\n\
                ### 9.5.3 Current Event Classification Summary\n\
                | Tier | Count | Events |\n\
                |---|---|---|\n\
                | LegacyInternalOnly | 2 | Dup, Dup |\n";
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(doc, &manifest, &variants);
    assert!(
        result.is_err(),
        "should fail when document has duplicate events in a tier",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("duplicate"),
        "error should mention 'duplicate': {}",
        err,
    );
}

#[test]
fn event_contract_document_production_list_mismatch_fails() {
    let doc = "## 9.5 AgentRunEvent Contract Coverage\n\
                ### 9.5.3 Current Event Classification Summary\n\
                | Tier | Count | Events |\n\
                |---|---|---|\n\
                | ProductionAudited | 99 | ToolCallBlocked |\n";
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(doc, &manifest, &variants);
    assert!(
        result.is_err(),
        "should fail when ProductionAudited count in doc mismatches manifest",
    );
}

#[test]
fn event_contract_document_forbidden_string_fails() {
    let doc = "## 9.5 AgentRunEvent Contract Coverage\n\
                all 45 AgentRunEventType variants\n";
    let manifest = event_contract_manifest();
    let variants = parse_agent_run_event_type_variants();
    let result = validate_document_against_manifest(doc, &manifest, &variants);
    assert!(
        result.is_err(),
        "should fail when document contains forbidden stale string",
    );
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("forbidden"),
        "error should mention 'forbidden': {}",
        err,
    );
}

#[test]
fn event_contract_duplicate_in_manifest_fails() {
    let mut manifest = event_contract_manifest();
    manifest.push(EventContractEntry {
        event: "ToolCallBlocked",
        status: EventContractStatus::LegacyInternalOnly,
        reason: "duplicate",
        production_rule_tokens: &[],
    });
    let result = validate_no_duplicate_events(&manifest);
    assert!(result.is_err(), "should fail on duplicate event names");
    let err = result.unwrap_err().join("|");
    assert!(
        err.contains("duplicate"),
        "error should mention 'duplicate': {}",
        err,
    );
}
