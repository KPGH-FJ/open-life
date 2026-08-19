use crate::errors::AppError;
use rfd::{AsyncMessageDialog, MessageButtons, MessageDialogResult, MessageLevel};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::Runtime;
use uuid::Uuid;

pub(crate) const DANGER_ACTION_POLICY_VERSION: &str = "danger-action-native-v1";
const CHALLENGE_TTL_MILLIS: i64 = 5 * 60 * 1_000;
const GRANT_TTL_MILLIS: i64 = 60 * 1_000;
const MAX_OUTSTANDING_CHALLENGES: usize = 256;
const SINGLETON_TARGET: &str = "__danger_action_singleton_scope__";

pub(crate) struct NativeDangerActionRequest<'a> {
    pub action_type: &'a str,
    pub target_ids_for_new_challenge: &'a [String],
    pub requested_target: Option<&'a str>,
    pub affected_count: usize,
    pub arguments: &'a serde_json::Value,
    pub arguments_summary: &'a str,
    pub scope_summary: &'a str,
    pub challenge_id: Option<&'a str>,
}

struct GrantConsumptionRequest<'a> {
    challenge_id: &'a str,
    action_type: &'a str,
    requested_target: Option<&'a str>,
    expected_affected_count: usize,
    arguments_digest: &'a str,
    local_session_binding: &'a str,
    now_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChallengeScope {
    action_type: String,
    target_ids: Vec<String>,
    affected_count: usize,
    local_session_binding: String,
    policy_version: String,
}

impl ChallengeScope {
    fn target_keys(&self) -> Vec<String> {
        if self.target_ids.is_empty() {
            vec![SINGLETON_TARGET.to_string()]
        } else {
            self.target_ids.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GrantRecord {
    grant_id: String,
    target_key: String,
    scope_digest: String,
    arguments_digest: String,
    policy_version: String,
    expires_at_millis: i64,
    consumed_at_millis: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ChallengeStatus {
    Pending,
    Confirming {
        ticket_id: String,
        arguments_digest: String,
    },
    Confirmed {
        arguments_digest: String,
        grants: BTreeMap<String, GrantRecord>,
    },
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChallengeRecord {
    scope: ChallengeScope,
    expires_at_millis: i64,
    status: ChallengeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativePromptTicket {
    challenge_id: String,
    ticket_id: String,
    requested_target: String,
    action_type: String,
    affected_count: usize,
    target_count: usize,
    arguments_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AuthorizationStep {
    NativePromptRequired(NativePromptTicket),
    GrantConsumed,
}

#[derive(Debug)]
struct DangerActionGrantAuthority {
    app_session_nonce: String,
    challenges: Mutex<HashMap<String, ChallengeRecord>>,
}

impl Default for DangerActionGrantAuthority {
    fn default() -> Self {
        Self {
            app_session_nonce: Uuid::new_v4().to_string(),
            challenges: Mutex::new(HashMap::new()),
        }
    }
}

impl DangerActionGrantAuthority {
    fn local_session_binding(&self, window_label: &str) -> Result<String, AppError> {
        // The release capability exposes shipped commands only to the app-owned main
        // WebView. The per-process nonce prevents a challenge from carrying across an
        // application restart even when the window label is unchanged.
        if window_label != "main" {
            return Err(AppError::permission(
                "danger action confirmation requires the authenticated main app session",
            ));
        }
        Ok(digest_serializable(&(
            "openlife-local-session",
            &self.app_session_nonce,
            window_label,
        )))
    }

    fn create_challenge(
        &self,
        action_type: &str,
        target_ids: &[String],
        affected_count: usize,
        local_session_binding: &str,
        now_millis: i64,
    ) -> Result<String, AppError> {
        let target_ids = canonical_target_ids(target_ids)?;
        let challenge_id = format!("danger-challenge:{}", Uuid::new_v4());
        let record = ChallengeRecord {
            scope: ChallengeScope {
                action_type: action_type.to_string(),
                target_ids,
                affected_count,
                local_session_binding: local_session_binding.to_string(),
                policy_version: DANGER_ACTION_POLICY_VERSION.to_string(),
            },
            expires_at_millis: now_millis.saturating_add(CHALLENGE_TTL_MILLIS),
            status: ChallengeStatus::Pending,
        };
        let mut challenges = self
            .challenges
            .lock()
            .map_err(|_| AppError::internal("danger action confirmation authority poisoned"))?;
        challenges.retain(|_, existing| existing.expires_at_millis >= now_millis);
        if challenges.len() >= MAX_OUTSTANDING_CHALLENGES {
            return Err(AppError::permission(
                "too many pending danger action confirmations; wait for existing challenges to expire",
            ));
        }
        challenges.insert(challenge_id.clone(), record);
        Ok(challenge_id)
    }

    fn begin_or_consume(
        &self,
        request: GrantConsumptionRequest<'_>,
    ) -> Result<AuthorizationStep, AppError> {
        let mut challenges = self
            .challenges
            .lock()
            .map_err(|_| AppError::internal("danger action confirmation authority poisoned"))?;
        let record = challenges.get_mut(request.challenge_id).ok_or_else(|| {
            AppError::permission(
                "danger action requires a fresh server-issued native confirmation challenge",
            )
        })?;

        ensure_challenge_scope(
            record,
            request.action_type,
            request.requested_target,
            request.expected_affected_count,
            request.local_session_binding,
            request.now_millis,
        )?;
        let requested_target = request
            .requested_target
            .unwrap_or(SINGLETON_TARGET)
            .to_string();

        match &mut record.status {
            ChallengeStatus::Pending => {
                let ticket = NativePromptTicket {
                    challenge_id: request.challenge_id.to_string(),
                    ticket_id: Uuid::new_v4().to_string(),
                    requested_target,
                    action_type: record.scope.action_type.clone(),
                    affected_count: record.scope.affected_count,
                    target_count: record.scope.target_ids.len(),
                    arguments_digest: request.arguments_digest.to_string(),
                };
                record.status = ChallengeStatus::Confirming {
                    ticket_id: ticket.ticket_id.clone(),
                    arguments_digest: request.arguments_digest.to_string(),
                };
                Ok(AuthorizationStep::NativePromptRequired(ticket))
            }
            ChallengeStatus::Confirming { .. } => Err(AppError::permission(
                "native confirmation is already in progress for this danger action",
            )),
            ChallengeStatus::Rejected => Err(AppError::permission(
                "native confirmation was rejected; start a fresh danger action preflight",
            )),
            ChallengeStatus::Confirmed {
                arguments_digest: confirmed_arguments_digest,
                grants,
            } => {
                if confirmed_arguments_digest != request.arguments_digest {
                    return Err(AppError::permission(
                        "danger action arguments changed after native confirmation",
                    ));
                }
                consume_grant(
                    grants,
                    &requested_target,
                    request.arguments_digest,
                    request.now_millis,
                )?;
                Ok(AuthorizationStep::GrantConsumed)
            }
        }
    }

    fn confirm_and_consume(
        &self,
        ticket: &NativePromptTicket,
        local_session_binding: &str,
        now_millis: i64,
    ) -> Result<(), AppError> {
        let mut challenges = self
            .challenges
            .lock()
            .map_err(|_| AppError::internal("danger action confirmation authority poisoned"))?;
        let record = challenges.get_mut(&ticket.challenge_id).ok_or_else(|| {
            AppError::permission("native confirmation challenge is no longer available")
        })?;
        ensure_challenge_scope(
            record,
            &ticket.action_type,
            if ticket.requested_target == SINGLETON_TARGET {
                None
            } else {
                Some(ticket.requested_target.as_str())
            },
            ticket.affected_count,
            local_session_binding,
            now_millis,
        )?;
        match &record.status {
            ChallengeStatus::Confirming {
                ticket_id,
                arguments_digest,
            } if ticket_id == &ticket.ticket_id && arguments_digest == &ticket.arguments_digest => {
            }
            _ => {
                return Err(AppError::permission(
                    "native confirmation ticket does not match the pending challenge",
                ));
            }
        }

        let expires_at_millis = now_millis.saturating_add(GRANT_TTL_MILLIS);
        let mut grants = BTreeMap::new();
        for target_key in record.scope.target_keys() {
            let grant_id = format!("danger-grant:{}", Uuid::new_v4());
            let scope_digest = digest_serializable(&(
                &record.scope.action_type,
                &target_key,
                &ticket.arguments_digest,
                &record.scope.local_session_binding,
                &record.scope.policy_version,
            ));
            grants.insert(
                target_key.clone(),
                GrantRecord {
                    grant_id,
                    target_key,
                    scope_digest,
                    arguments_digest: ticket.arguments_digest.clone(),
                    policy_version: record.scope.policy_version.clone(),
                    expires_at_millis,
                    consumed_at_millis: None,
                },
            );
        }
        consume_grant(
            &mut grants,
            &ticket.requested_target,
            &ticket.arguments_digest,
            now_millis,
        )?;
        record.status = ChallengeStatus::Confirmed {
            arguments_digest: ticket.arguments_digest.clone(),
            grants,
        };
        Ok(())
    }

    fn reject(&self, ticket: &NativePromptTicket) -> Result<(), AppError> {
        let mut challenges = self
            .challenges
            .lock()
            .map_err(|_| AppError::internal("danger action confirmation authority poisoned"))?;
        let record = challenges.get_mut(&ticket.challenge_id).ok_or_else(|| {
            AppError::permission("native confirmation challenge is no longer available")
        })?;
        match &record.status {
            ChallengeStatus::Confirming { ticket_id, .. } if ticket_id == &ticket.ticket_id => {
                record.status = ChallengeStatus::Rejected;
                Ok(())
            }
            _ => Err(AppError::permission(
                "native confirmation ticket does not match the pending challenge",
            )),
        }
    }
}

fn ensure_challenge_scope(
    record: &ChallengeRecord,
    action_type: &str,
    requested_target: Option<&str>,
    expected_affected_count: usize,
    local_session_binding: &str,
    now_millis: i64,
) -> Result<(), AppError> {
    if now_millis > record.expires_at_millis {
        return Err(AppError::permission(
            "danger action confirmation challenge expired",
        ));
    }
    if record.scope.action_type != action_type
        || record.scope.affected_count != expected_affected_count
        || record.scope.local_session_binding != local_session_binding
        || record.scope.policy_version != DANGER_ACTION_POLICY_VERSION
    {
        return Err(AppError::permission(
            "danger action confirmation scope does not match the requested action",
        ));
    }
    match requested_target {
        Some(target) if !record.scope.target_ids.iter().any(|id| id == target) => {
            Err(AppError::permission(
                "danger action target is outside the server-issued confirmation scope",
            ))
        }
        None if !record.scope.target_ids.is_empty() => Err(AppError::permission(
            "danger action target scope is missing from the final action",
        )),
        _ => Ok(()),
    }
}

fn consume_grant(
    grants: &mut BTreeMap<String, GrantRecord>,
    target_key: &str,
    arguments_digest: &str,
    now_millis: i64,
) -> Result<(), AppError> {
    let grant = grants.get_mut(target_key).ok_or_else(|| {
        AppError::permission("no native confirmation grant exists for this danger action target")
    })?;
    if grant.target_key != target_key
        || !grant.grant_id.starts_with("danger-grant:")
        || grant.scope_digest.is_empty()
        || grant.policy_version != DANGER_ACTION_POLICY_VERSION
        || grant.arguments_digest != arguments_digest
    {
        return Err(AppError::permission(
            "native confirmation grant scope does not match the final action",
        ));
    }
    if now_millis > grant.expires_at_millis {
        return Err(AppError::permission(
            "native confirmation grant expired before it could be consumed",
        ));
    }
    if grant.consumed_at_millis.is_some() {
        return Err(AppError::permission(
            "native confirmation grant was already consumed",
        ));
    }
    // This compare-and-set occurs while the authority mutex is held, so concurrent
    // final-action calls have exactly one winner for a target-specific grant.
    grant.consumed_at_millis = Some(now_millis);
    Ok(())
}

fn canonical_target_ids(target_ids: &[String]) -> Result<Vec<String>, AppError> {
    let mut canonical = Vec::with_capacity(target_ids.len());
    for target_id in target_ids {
        if target_id.is_empty()
            || target_id.len() > 128
            || target_id.trim() != target_id
            || target_id.chars().any(char::is_control)
        {
            return Err(AppError::permission(
                "danger action confirmation target is not metadata-safe",
            ));
        }
        canonical.push(target_id.clone());
    }
    canonical.sort();
    canonical.dedup();
    if canonical.len() > 100 {
        return Err(AppError::permission(
            "danger action confirmation target scope is too large",
        ));
    }
    Ok(canonical)
}

fn digest_serializable<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value)
        .expect("danger action confirmation only hashes infallibly serializable values");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn arguments_digest(arguments: &serde_json::Value) -> String {
    digest_serializable(&("danger-action-arguments", arguments))
}

fn authority() -> &'static DangerActionGrantAuthority {
    static AUTHORITY: OnceLock<DangerActionGrantAuthority> = OnceLock::new();
    AUTHORITY.get_or_init(DangerActionGrantAuthority::default)
}

fn monotonic_now_millis() -> i64 {
    static PROCESS_START: OnceLock<Instant> = OnceLock::new();
    PROCESS_START
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn bounded_prompt_summary(summary: &str) -> String {
    let sanitized = summary
        .chars()
        .filter(|character| !character.is_control())
        .take(240)
        .collect::<String>();
    if sanitized.trim().is_empty() {
        "参数已由后端绑定到本次确认。".to_string()
    } else {
        sanitized
    }
}

fn native_prompt_scope_count_labels(ticket: &NativePromptTicket) -> (String, String) {
    if ticket.action_type == "data_import_overwrite"
        && ticket.affected_count == 0
        && ticket.target_count == 0
    {
        // Import preflight intentionally runs before the user-selected file is
        // parsed. Zero would falsely claim an empty overwrite; the final
        // native prompt is still bound to the validated payload digest and
        // governed request through `arguments_digest`.
        return (
            "未在预检阶段枚举（以已校验备份内容为准）".into(),
            "未在预检阶段枚举（最多四类 canonical owner）".into(),
        );
    }
    (
        ticket.affected_count.to_string(),
        ticket.target_count.to_string(),
    )
}

async fn show_native_confirmation<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    ticket: &NativePromptTicket,
    scope_summary: &str,
    arguments_summary: &str,
) -> Result<bool, AppError> {
    let (affected_count, target_count) = native_prompt_scope_count_labels(ticket);
    let message = format!(
        "OpenLife 请求执行高风险动作。\n\n动作：{}\n范围：{}\n影响数量：{}\n目标数量：{}\n参数：{}\n\n只有此系统对话框中的确认会授权动作；网页文字和确认短语不会授权。",
        ticket.action_type,
        bounded_prompt_summary(scope_summary),
        affected_count,
        target_count,
        bounded_prompt_summary(arguments_summary),
    );
    let result = tokio::time::timeout(
        Duration::from_millis(CHALLENGE_TTL_MILLIS as u64),
        AsyncMessageDialog::new()
            .set_parent(window)
            .set_title("OpenLife 高风险动作确认")
            .set_description(message)
            .set_level(MessageLevel::Warning)
            .set_buttons(MessageButtons::OkCancelCustom(
                "确认执行".to_string(),
                "取消".to_string(),
            ))
            .show(),
    )
    .await
    .map_err(|_| AppError::timeout("native confirmation dialog timed out"))?;
    Ok(matches!(
        result,
        MessageDialogResult::Custom(ref label) if label == "确认执行"
    ))
}

pub(crate) fn issue_danger_action_challenge(
    window_label: &str,
    action_type: &str,
    target_ids: &[String],
    affected_count: usize,
) -> Result<String, AppError> {
    let authority = authority();
    let local_session_binding = authority.local_session_binding(window_label)?;
    authority.create_challenge(
        action_type,
        target_ids,
        affected_count,
        &local_session_binding,
        monotonic_now_millis(),
    )
}

pub(crate) async fn require_native_danger_action_confirmation<R: Runtime>(
    window: &tauri::WebviewWindow<R>,
    request: NativeDangerActionRequest<'_>,
) -> Result<(), AppError> {
    let authority = authority();
    let local_session_binding = authority.local_session_binding(window.label())?;
    let now_millis = monotonic_now_millis();
    let owned_challenge_id = match request.challenge_id {
        Some(challenge_id) if !challenge_id.trim().is_empty() => challenge_id.to_string(),
        _ => authority.create_challenge(
            request.action_type,
            request.target_ids_for_new_challenge,
            request.affected_count,
            &local_session_binding,
            now_millis,
        )?,
    };
    let arguments_digest = arguments_digest(request.arguments);
    match authority.begin_or_consume(GrantConsumptionRequest {
        challenge_id: &owned_challenge_id,
        action_type: request.action_type,
        requested_target: request.requested_target,
        expected_affected_count: request.affected_count,
        arguments_digest: &arguments_digest,
        local_session_binding: &local_session_binding,
        now_millis,
    })? {
        AuthorizationStep::GrantConsumed => Ok(()),
        AuthorizationStep::NativePromptRequired(ticket) => {
            let confirmed = match show_native_confirmation(
                window,
                &ticket,
                request.scope_summary,
                request.arguments_summary,
            )
            .await
            {
                Ok(confirmed) => confirmed,
                Err(error) => {
                    let _ = authority.reject(&ticket);
                    return Err(error);
                }
            };
            if !confirmed {
                authority.reject(&ticket)?;
                return Err(AppError::permission(
                    "danger action was not confirmed in the native system dialog",
                ));
            }
            authority.confirm_and_consume(&ticket, &local_session_binding, monotonic_now_millis())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn test_authority() -> DangerActionGrantAuthority {
        DangerActionGrantAuthority {
            app_session_nonce: "test-app-session-nonce".to_string(),
            challenges: Mutex::new(HashMap::new()),
        }
    }

    fn issue_pending(
        authority: &DangerActionGrantAuthority,
        targets: &[&str],
        affected_count: usize,
        now_millis: i64,
    ) -> (String, String) {
        let session = authority.local_session_binding("main").unwrap();
        let target_ids = targets
            .iter()
            .map(|target| target.to_string())
            .collect::<Vec<_>>();
        let challenge = authority
            .create_challenge(
                "mcp_audit_cleanup",
                &target_ids,
                affected_count,
                &session,
                now_millis,
            )
            .unwrap();
        (challenge, session)
    }

    fn consumption_request<'a>(
        challenge_id: &'a str,
        requested_target: Option<&'a str>,
        expected_affected_count: usize,
        arguments_digest: &'a str,
        local_session_binding: &'a str,
        now_millis: i64,
    ) -> GrantConsumptionRequest<'a> {
        GrantConsumptionRequest {
            challenge_id,
            action_type: "mcp_audit_cleanup",
            requested_target,
            expected_affected_count,
            arguments_digest,
            local_session_binding,
            now_millis,
        }
    }

    fn native_confirm_first_target(
        authority: &DangerActionGrantAuthority,
        challenge: &str,
        session: &str,
        target: &str,
        affected_count: usize,
        arguments: &str,
        now_millis: i64,
    ) {
        let step = authority
            .begin_or_consume(consumption_request(
                challenge,
                Some(target),
                affected_count,
                arguments,
                session,
                now_millis,
            ))
            .unwrap();
        let AuthorizationStep::NativePromptRequired(ticket) = step else {
            panic!("first consumption must require native confirmation");
        };
        authority
            .confirm_and_consume(&ticket, session, now_millis + 1)
            .unwrap();
    }

    #[test]
    fn random_native_grant_is_scope_bound_and_single_use() {
        let authority = test_authority();
        let (challenge, session) = issue_pending(&authority, &["run-1"], 1, 1_000);
        native_confirm_first_target(
            &authority, &challenge, &session, "run-1", 1, "args-a", 1_001,
        );

        let replay = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-1"),
                1,
                "args-a",
                &session,
                1_002,
            ))
            .unwrap_err();
        assert!(replay.message().contains("already consumed"));

        let records = authority.challenges.lock().unwrap();
        let record = records.get(&challenge).unwrap();
        let ChallengeStatus::Confirmed { grants, .. } = &record.status else {
            panic!("native callback must produce a confirmed grant set");
        };
        let grant = grants.get("run-1").unwrap();
        assert!(grant.grant_id.starts_with("danger-grant:"));
        assert_ne!(grant.grant_id, challenge);
        assert!(!grant.scope_digest.contains("run-1"));
        assert_eq!(grant.policy_version, DANGER_ACTION_POLICY_VERSION);
    }

    #[test]
    fn target_argument_session_policy_and_expiry_changes_fail_closed() {
        let authority = test_authority();
        let (challenge, session) = issue_pending(&authority, &["run-1", "run-2"], 2, 2_000);
        native_confirm_first_target(
            &authority, &challenge, &session, "run-1", 2, "args-a", 2_001,
        );

        let target_change = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-3"),
                2,
                "args-a",
                &session,
                2_002,
            ))
            .unwrap_err();
        assert!(target_change.message().contains("outside"));

        let args_change = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-2"),
                2,
                "args-b",
                &session,
                2_002,
            ))
            .unwrap_err();
        assert!(args_change.message().contains("arguments changed"));

        let other_session = digest_serializable(&("other", "session"));
        let session_change = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-2"),
                2,
                "args-a",
                &other_session,
                2_002,
            ))
            .unwrap_err();
        assert!(session_change.message().contains("scope does not match"));

        let expired = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-2"),
                2,
                "args-a",
                &session,
                2_001 + GRANT_TTL_MILLIS + 2,
            ))
            .unwrap_err();
        assert!(expired.message().contains("grant expired"));

        let mut records = authority.challenges.lock().unwrap();
        records.get_mut(&challenge).unwrap().scope.policy_version = "stale-policy".to_string();
        drop(records);
        let policy_change = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-2"),
                2,
                "args-a",
                &session,
                2_003,
            ))
            .unwrap_err();
        assert!(policy_change.message().contains("scope does not match"));
    }

    #[test]
    fn concurrent_consumers_have_exactly_one_cas_winner() {
        let authority = Arc::new(test_authority());
        let (challenge, session) = issue_pending(&authority, &["run-1", "run-2"], 2, 3_000);
        native_confirm_first_target(
            &authority, &challenge, &session, "run-1", 2, "args-a", 3_001,
        );

        let mut handles = Vec::new();
        for _ in 0..8 {
            let authority = Arc::clone(&authority);
            let challenge = challenge.clone();
            let session = session.clone();
            handles.push(thread::spawn(move || {
                authority.begin_or_consume(consumption_request(
                    &challenge,
                    Some("run-2"),
                    2,
                    "args-a",
                    &session,
                    3_002,
                ))
            }));
        }
        let winners = handles
            .into_iter()
            .map(|handle| matches!(handle.join().unwrap(), Ok(AuthorizationStep::GrantConsumed)))
            .filter(|won| *won)
            .count();
        assert_eq!(winners, 1);
    }

    #[test]
    fn advertised_phrase_or_forged_identifier_never_creates_authority() {
        let authority = test_authority();
        let session = authority.local_session_binding("main").unwrap();
        let forged = authority
            .begin_or_consume(consumption_request(
                "danger-preflight:sha256:client-forged",
                Some("run-1"),
                1,
                "DELETE RUNS",
                &session,
                4_000,
            ))
            .unwrap_err();
        assert!(forged.message().contains("server-issued"));
    }

    #[test]
    fn challenges_are_random_restart_local_and_expire_before_confirmation() {
        let authority = test_authority();
        let (first, session) = issue_pending(&authority, &["run-1"], 1, 10_000);
        let (second, _) = issue_pending(&authority, &["run-1"], 1, 10_000);
        assert_ne!(first, second);
        assert!(first.starts_with("danger-challenge:"));
        assert!(!first.contains("run-1"));

        let expired = authority
            .begin_or_consume(consumption_request(
                &first,
                Some("run-1"),
                1,
                "args-a",
                &session,
                10_000 + CHALLENGE_TTL_MILLIS + 1,
            ))
            .unwrap_err();
        assert!(expired.message().contains("expired"));
    }

    #[test]
    fn rejected_native_prompt_cannot_be_reused() {
        let authority = test_authority();
        let (challenge, session) = issue_pending(&authority, &["run-1"], 1, 5_000);
        let step = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-1"),
                1,
                "args-a",
                &session,
                5_001,
            ))
            .unwrap();
        let AuthorizationStep::NativePromptRequired(ticket) = step else {
            panic!("pending challenge must request native confirmation");
        };
        authority.reject(&ticket).unwrap();
        let rejected = authority
            .begin_or_consume(consumption_request(
                &challenge,
                Some("run-1"),
                1,
                "args-a",
                &session,
                5_002,
            ))
            .unwrap_err();
        assert!(rejected.message().contains("rejected"));
    }

    #[test]
    fn shipped_privileged_commands_do_not_accept_client_phrase_as_authority() {
        let settings = include_str!("commands/settings.rs");
        let memory = include_str!("commands/memory.rs");
        let proposal = include_str!("commands/proposal.rs");
        let lib = include_str!("lib.rs");
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();

        assert!(!settings.contains("pub struct DangerActionConfirmationEvidence"));
        assert!(!settings.contains("evidence.confirmation_phrase"));
        assert!(!settings.contains("fn danger_action_confirmation_phrase"));
        assert!(!settings.contains("danger_action_preflight_id("));
        assert!(settings.contains("issue_danger_action_challenge("));
        assert!(settings.contains("require_native_danger_action_confirmation("));
        assert!(memory.contains("require_danger_action_confirmation("));
        assert!(proposal.contains("require_native_danger_action_confirmation("));
        assert!(proposal.contains("#[cfg(test)]\npub(crate) async fn accept_proposal_with_state"));
        assert!(proposal.contains("!proposal_requires_native_confirmation"));
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        for shipped_command in [
            "accept_proposal,",
            "export_mcp_audit_logs,",
            "rebuild_memory_index,",
            "cleanup_mcp_audit_logs,",
            "rotate_mcp_audit_key,",
        ] {
            assert!(
                lib.contains(shipped_command),
                "expected privileged command {shipped_command} to remain shipped behind the Rust authority"
            );
        }
    }

    #[test]
    fn native_confirmation_is_parented_to_the_authenticated_window() {
        let source = include_str!("danger_action_confirmation.rs");
        let show_native_confirmation = source
            .split("async fn show_native_confirmation")
            .nth(1)
            .and_then(|tail| {
                tail.split("pub(crate) fn issue_danger_action_challenge")
                    .next()
            })
            .expect("native confirmation implementation must remain present");

        assert!(show_native_confirmation.contains("AsyncMessageDialog::new()"));
        assert!(show_native_confirmation.contains(".set_parent(window)"));
        assert!(show_native_confirmation.contains(".show(),"));
    }
}
