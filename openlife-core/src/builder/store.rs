use crate::builder::types::*;
use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

const LEGACY_AI_GENERATED_REVIEW_PROMPT: &str =
    "快速构建问题已完成！接下来请审阅 AI 生成的模型建议。";
const TRUTHFUL_REVIEW_PROMPT: &str = "快速构建问题已完成！接下来请审阅根据你回答生成的待确认建议。";
const BUILDER_RETENTION_SCHEMA_VERSION: u8 = 1;
const BUILDER_ACTIVE_RETENTION_DAYS: i64 = 90;
const BUILDER_RECOVERY_WINDOW_DAYS: i64 = 30;
const BUILDER_STORE_MAX_BYTES: u64 = 64 * 1024 * 1024;
const BUILDER_STORE_MAX_SESSIONS: usize = 512;
const BUILDER_SESSION_MAX_BYTES: usize = 16 * 1024 * 1024;
const BUILDER_SESSION_MAX_DRAFT_BYTES: usize = 8 * 1024 * 1024;
const BUILDER_SESSION_MAX_PROMPT_BYTES: usize = 256 * 1024;
const BUILDER_SESSION_MAX_SIGNALS: usize = 512;
const BUILDER_SIGNAL_MAX_BYTES: usize = 256 * 1024;
const BUILDER_SIGNAL_TOTAL_MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuilderSessionStoreData {
    pub sessions: HashMap<String, BuilderSession>,
}

pub struct BuilderSessionStore {
    path: PathBuf,
    clock: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

impl BuilderSessionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            clock: Arc::new(Utc::now),
        }
    }

    #[cfg(test)]
    fn new_at(path: impl Into<PathBuf>, now: DateTime<Utc>) -> Self {
        Self {
            path: path.into(),
            clock: Arc::new(move || now),
        }
    }

    fn now(&self) -> DateTime<Utc> {
        (self.clock)()
    }

    fn initialize_retention(session: &mut BuilderSession, now: DateTime<Utc>) {
        session.retention = BuilderSessionRetention {
            schema_version: BUILDER_RETENTION_SCHEMA_VERSION,
            created_at: Some(now),
            last_activity_at: Some(now),
            expires_at: Some(now + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS)),
            purge_after: Some(
                now + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS + BUILDER_RECOVERY_WINDOW_DAYS),
            ),
        };
    }

    fn validate_or_migrate_retention(
        session: &mut BuilderSession,
        now: DateTime<Utc>,
    ) -> Result<bool> {
        match session.retention.schema_version {
            0 => {
                Self::initialize_retention(session, now);
                Ok(true)
            }
            BUILDER_RETENTION_SCHEMA_VERSION => {
                if session.retention.created_at.is_none()
                    || session.retention.last_activity_at.is_none()
                    || session.retention.expires_at.is_none()
                    || session.retention.purge_after.is_none()
                {
                    bail!("builder_session_retention_v1_is_incomplete");
                }
                Ok(false)
            }
            version => bail!("unsupported_builder_session_retention_schema:{version}"),
        }
    }

    fn touch_retention(session: &mut BuilderSession, now: DateTime<Utc>) -> Result<()> {
        Self::validate_or_migrate_retention(session, now)?;
        session.retention.last_activity_at = Some(now);
        session.retention.expires_at = Some(now + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS));
        session.retention.purge_after = Some(
            now + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS + BUILDER_RECOVERY_WINDOW_DAYS),
        );
        Ok(())
    }

    fn ensure_owner_only_parent(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(())
    }

    fn validate_session_bounds(session: &BuilderSession) -> Result<()> {
        if session.session_id.trim() != session.session_id
            || session.session_id.is_empty()
            || session.session_id.len() > 256
            || session.session_id.chars().any(char::is_control)
        {
            bail!("builder_session_id_is_invalid");
        }
        if session.draft_yaml.len() > BUILDER_SESSION_MAX_DRAFT_BYTES {
            bail!("builder_session_draft_exceeds_bounded_limit");
        }
        if session.current_prompt.len() > BUILDER_SESSION_MAX_PROMPT_BYTES {
            bail!("builder_session_prompt_exceeds_bounded_limit");
        }
        if session.pending_signals.len() > BUILDER_SESSION_MAX_SIGNALS
            || session.confirmed_signals.len() > BUILDER_SESSION_MAX_SIGNALS
        {
            bail!("builder_session_signal_count_exceeds_bounded_limit");
        }
        let mut signal_bytes = 0usize;
        for signal in session
            .pending_signals
            .iter()
            .chain(session.confirmed_signals.iter())
        {
            let encoded_len = serde_json::to_vec(signal)?.len();
            if encoded_len > BUILDER_SIGNAL_MAX_BYTES {
                bail!("builder_session_signal_exceeds_bounded_limit");
            }
            signal_bytes = signal_bytes
                .checked_add(encoded_len)
                .filter(|total| *total <= BUILDER_SIGNAL_TOTAL_MAX_BYTES)
                .ok_or_else(|| anyhow::anyhow!("builder_session_signals_exceed_bounded_limit"))?;
        }
        if serde_json::to_vec(session)?.len() > BUILDER_SESSION_MAX_BYTES {
            bail!("builder_session_exceeds_bounded_limit");
        }
        Ok(())
    }

    fn validate_data_bounds(data: &BuilderSessionStoreData) -> Result<()> {
        if data.sessions.len() > BUILDER_STORE_MAX_SESSIONS {
            bail!("builder_session_store_exceeds_session_limit");
        }
        for session in data.sessions.values() {
            Self::validate_session_bounds(session)?;
        }
        Ok(())
    }

    fn write_data(&self, data: &BuilderSessionStoreData) -> Result<()> {
        Self::validate_data_bounds(data)?;
        self.ensure_owner_only_parent()?;
        let text = serde_json::to_string_pretty(data)?;
        if text.len() as u64 > BUILDER_STORE_MAX_BYTES {
            bail!("builder_session_store_exceeds_file_limit");
        }
        crate::atomic_file::write_atomic(&self.path, text.as_bytes())
    }

    pub fn load(&self) -> Result<BuilderSessionStoreData> {
        if !self.path.exists() {
            return Ok(BuilderSessionStoreData::default());
        }
        self.ensure_owner_only_parent()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))?;
        }
        let metadata = std::fs::metadata(&self.path)?;
        if metadata.len() > BUILDER_STORE_MAX_BYTES {
            bail!("builder_session_store_exceeds_file_limit");
        }
        let mut bytes = Vec::with_capacity(metadata.len().min(BUILDER_STORE_MAX_BYTES) as usize);
        std::fs::File::open(&self.path)?
            .take(BUILDER_STORE_MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > BUILDER_STORE_MAX_BYTES {
            bail!("builder_session_store_exceeds_file_limit");
        }
        let text = String::from_utf8(bytes)?;
        let mut data: BuilderSessionStoreData = serde_json::from_str(&text)?;
        Self::validate_data_bounds(&data)?;
        let now = self.now();
        let mut changed = false;
        for session in data.sessions.values_mut() {
            if session.current_prompt == LEGACY_AI_GENERATED_REVIEW_PROMPT {
                session.current_prompt = TRUTHFUL_REVIEW_PROMPT.into();
                changed = true;
            }
            changed |= Self::validate_or_migrate_retention(session, now)?;
        }
        let session_count = data.sessions.len();
        data.sessions
            .retain(|_, session| session.retention.status_at(now).is_some());
        changed |= session_count != data.sessions.len();
        if changed {
            self.write_data(&data)?;
        }
        Ok(data)
    }

    pub fn save(&self, data: &BuilderSessionStoreData) -> Result<()> {
        let mut normalized = data.clone();
        Self::validate_data_bounds(&normalized)?;
        let now = self.now();
        for session in normalized.sessions.values_mut() {
            Self::validate_or_migrate_retention(session, now)?;
        }
        self.write_data(&normalized)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<BuilderSession>> {
        let data = self.load()?;
        Ok(data.sessions.get(session_id).cloned())
    }

    pub fn get_active_session(&self, session_id: &str) -> Result<Option<BuilderSession>> {
        let now = self.now();
        Ok(self.get_session(session_id)?.filter(|session| {
            session.retention.status_at(now) == Some(BuilderSessionRetentionStatus::Active)
        }))
    }

    /// Explicitly resumes a recoverable draft and renews its active retention
    /// window. A claimed review is returned without renewal so a blocked or
    /// crashed staging operation cannot be silently converted into a new edit
    /// epoch.
    pub fn resume_session(&self, session_id: &str) -> Result<Option<BuilderSession>> {
        let mut data = self.load()?;
        let Some(current) = data.sessions.get_mut(session_id) else {
            return Ok(None);
        };
        if current.review_claim_id.is_some() {
            return Ok(Some(current.clone()));
        }
        let now = self.now();
        Self::touch_retention(current, now)?;
        current.revision = current.revision.saturating_add(1);
        let resumed = current.clone();
        self.save(&data)?;
        Ok(Some(resumed))
    }

    pub fn save_session(&self, session: &BuilderSession) -> Result<()> {
        let mut data = self.load()?;
        let mut next = session.clone();
        Self::touch_retention(&mut next, self.now())?;
        data.sessions.insert(next.session_id.clone(), next);
        self.save(&data)
    }

    pub fn create_session_if_absent(&self, session: &BuilderSession) -> Result<BuilderSession> {
        let mut data = self.load()?;
        if let Some(current) = data.sessions.get(&session.session_id) {
            return Ok(current.clone());
        }
        let mut next = session.clone();
        Self::touch_retention(&mut next, self.now())?;
        data.sessions.insert(next.session_id.clone(), next.clone());
        self.save(&data)?;
        Ok(next)
    }

    pub fn save_session_if_revision(
        &self,
        session: &BuilderSession,
        expected_revision: u64,
    ) -> Result<Option<BuilderSession>> {
        let mut data = self.load()?;
        let Some(current) = data.sessions.get(&session.session_id) else {
            return Ok(None);
        };
        if current.revision != expected_revision
            || current.review_claim_id.is_some()
            || current.retention.status_at(self.now())
                != Some(BuilderSessionRetentionStatus::Active)
        {
            return Ok(None);
        }
        let mut next = session.clone();
        next.revision = expected_revision.saturating_add(1);
        next.review_claim_id = None;
        Self::touch_retention(&mut next, self.now())?;
        data.sessions.insert(next.session_id.clone(), next.clone());
        self.save(&data)?;
        Ok(Some(next))
    }

    pub fn claim_review_session(
        &self,
        session_id: &str,
        expected_revision: u64,
    ) -> Result<Option<BuilderSession>> {
        let mut data = self.load()?;
        let Some(current) = data.sessions.get_mut(session_id) else {
            return Ok(None);
        };
        if current.revision != expected_revision
            || current.review_claim_id.is_some()
            || !current.finished
            || current.pending_signals.is_empty()
            || current.retention.status_at(self.now())
                != Some(BuilderSessionRetentionStatus::Active)
        {
            return Ok(None);
        }
        current.revision = current.revision.saturating_add(1);
        current.review_claim_id = Some(uuid::Uuid::new_v4().to_string());
        Self::touch_retention(current, self.now())?;
        let claimed = current.clone();
        self.save(&data)?;
        Ok(Some(claimed))
    }

    pub fn release_review_claim(&self, session_id: &str, claim_id: &str) -> Result<bool> {
        let mut data = self.load()?;
        let Some(current) = data.sessions.get_mut(session_id) else {
            return Ok(false);
        };
        if current.review_claim_id.as_deref() != Some(claim_id) {
            return Ok(false);
        }
        current.review_claim_id = None;
        current.revision = current.revision.saturating_add(1);
        Self::touch_retention(current, self.now())?;
        self.save(&data)?;
        Ok(true)
    }

    pub fn remove_claimed_session(&self, session_id: &str, claim_id: &str) -> Result<bool> {
        let mut data = self.load()?;
        let matches = data
            .sessions
            .get(session_id)
            .is_some_and(|session| session.review_claim_id.as_deref() == Some(claim_id));
        if !matches {
            return Ok(false);
        }
        data.sessions.remove(session_id);
        self.save(&data)?;
        Ok(true)
    }

    pub fn remove_session(&self, session_id: &str) -> Result<()> {
        let mut data = self.load()?;
        data.sessions.remove(session_id);
        self.save(&data)
    }

    pub fn remove_session_if_unclaimed(&self, session_id: &str) -> Result<bool> {
        let mut data = self.load()?;
        let Some(current) = data.sessions.get(session_id) else {
            return Ok(true);
        };
        if current.review_claim_id.is_some() {
            return Ok(false);
        }
        data.sessions.remove(session_id);
        self.save(&data)?;
        Ok(true)
    }

    pub fn list_unfinished_sessions(&self) -> Result<Vec<BuilderSession>> {
        let data = self.load()?;
        Ok(data
            .sessions
            .values()
            .filter(|s| !s.finished || !s.pending_signals.is_empty())
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 0, 0, 0).single().unwrap()
    }

    #[test]
    fn builder_session_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = BuilderSessionStore::new(dir.path().join("store.json"));
        let mut session = BuilderSession::new("s4", BuilderMode::Socratic);
        session.step_index = 3;
        session.current_prompt = "当前问题".into();
        store.save_session(&session).unwrap();

        let loaded = store.get_session("s4").unwrap().unwrap();
        assert_eq!(loaded.session_id, "s4");
        assert_eq!(loaded.step_index, 3);
        assert_eq!(loaded.mode, BuilderMode::Socratic);
        assert_eq!(loaded.current_prompt, "当前问题");

        let list = store.list_unfinished_sessions().unwrap();
        assert_eq!(list.len(), 1);

        store.remove_session("s4").unwrap();
        assert!(store.get_session("s4").unwrap().is_none());
    }

    #[test]
    fn builder_session_store_includes_finished_review_sessions_in_resume_list() {
        let dir = tempfile::tempdir().unwrap();
        let store = BuilderSessionStore::new(dir.path().join("store.json"));
        let mut session = BuilderSession::new("review-1", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("fujing".into()),
            confidence: 0.9,
            reason: "测试".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        store.save_session(&session).unwrap();

        let list = store.list_unfinished_sessions().unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].finished);
        assert_eq!(list[0].pending_signals.len(), 1);
    }

    #[test]
    fn builder_session_store_rejects_stale_writer_and_claims_review_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = BuilderSessionStore::new(dir.path().join("store.json"));
        let mut session = BuilderSession::new("cas-review", BuilderMode::Quick);
        store.save_session(&session).unwrap();

        session.current_prompt = "first update".into();
        let saved = store
            .save_session_if_revision(&session, 0)
            .unwrap()
            .expect("first revision wins");
        assert_eq!(saved.revision, 1);
        session.current_prompt = "stale update".into();
        assert!(store
            .save_session_if_revision(&session, 0)
            .unwrap()
            .is_none());

        let mut review = saved;
        review.finished = true;
        review.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("Alex"),
            confidence: 0.9,
            reason: "explicit".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        let review = store
            .save_session_if_revision(&review, 1)
            .unwrap()
            .expect("review revision");
        let claimed = store
            .claim_review_session("cas-review", review.revision)
            .unwrap()
            .expect("first review claim wins");
        assert!(claimed.review_claim_id.is_some());
        assert!(store
            .claim_review_session("cas-review", review.revision)
            .unwrap()
            .is_none());
        let claim_id = claimed.review_claim_id.as_deref().unwrap();
        assert!(store
            .remove_claimed_session("cas-review", claim_id)
            .unwrap());
        assert!(store.get_session("cas-review").unwrap().is_none());
    }

    #[test]
    fn builder_session_store_create_and_delete_respect_the_canonical_claim() {
        let dir = tempfile::tempdir().unwrap();
        let store = BuilderSessionStore::new(dir.path().join("store.json"));
        let mut first = BuilderSession::new("canonical", BuilderMode::Quick);
        first.current_prompt = "first".into();
        assert_eq!(
            store
                .create_session_if_absent(&first)
                .unwrap()
                .current_prompt,
            "first"
        );

        let mut stale_creator = first.clone();
        stale_creator.current_prompt = "must-not-overwrite".into();
        assert_eq!(
            store
                .create_session_if_absent(&stale_creator)
                .unwrap()
                .current_prompt,
            "first"
        );

        first.finished = true;
        first.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("Alex"),
            confidence: 0.9,
            reason: "explicit".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        let review = store
            .save_session_if_revision(&first, 0)
            .unwrap()
            .expect("review revision");
        let claimed = store
            .claim_review_session("canonical", review.revision)
            .unwrap()
            .expect("review claim");
        assert!(!store.remove_session_if_unclaimed("canonical").unwrap());
        assert!(store
            .remove_claimed_session("canonical", claimed.review_claim_id.as_deref().unwrap())
            .unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn builder_session_store_is_owner_only_on_disk() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let private_dir = root.path().join("private-builder-state");
        let path = private_dir.join("sessions.json");
        let store = BuilderSessionStore::new(&path);
        store
            .save_session(&BuilderSession::new("private", BuilderMode::Socratic))
            .unwrap();

        assert_eq!(
            std::fs::metadata(&private_dir)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn legacy_builder_retention_migrates_once_to_stable_explicit_deadlines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.json");
        let mut legacy = serde_json::to_value(BuilderSession::new(
            "legacy-resumable",
            BuilderMode::Socratic,
        ))
        .unwrap();
        legacy.as_object_mut().unwrap().remove("retention");
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "sessions": {"legacy-resumable": legacy}
            }))
            .unwrap(),
        )
        .unwrap();

        let migration_time = fixed_time();
        let migrated = BuilderSessionStore::new_at(&path, migration_time)
            .get_session("legacy-resumable")
            .unwrap()
            .unwrap();
        assert_eq!(migrated.retention.schema_version, 1);
        assert_eq!(migrated.retention.created_at, Some(migration_time));
        assert_eq!(
            migrated.retention.expires_at,
            Some(migration_time + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS))
        );
        assert_eq!(
            migrated.retention.purge_after,
            Some(
                migration_time
                    + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS + BUILDER_RECOVERY_WINDOW_DAYS,)
            )
        );

        // A later read must use the persisted deadline. Re-reading the file or
        // changing its filesystem timestamps cannot silently renew retention.
        let later = BuilderSessionStore::new_at(&path, migration_time + Duration::days(10))
            .get_session("legacy-resumable")
            .unwrap()
            .unwrap();
        assert_eq!(later.retention, migrated.retention);
    }

    #[test]
    fn expired_builder_draft_is_recoverable_until_explicit_purge_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.json");
        let started_at = fixed_time();
        let store = BuilderSessionStore::new_at(&path, started_at);
        let mut resumable = BuilderSession::new("resumable", BuilderMode::Socratic);
        resumable.draft_yaml = "RECOVERABLE_PRIVATE_DRAFT".into();
        store.save_session(&resumable).unwrap();
        let mut purgeable = BuilderSession::new("purgeable", BuilderMode::Quick);
        purgeable.draft_yaml = "PURGED_PRIVATE_DRAFT".into();
        store.save_session(&purgeable).unwrap();

        let expired_at = started_at + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS + 1);
        let expired_store = BuilderSessionStore::new_at(&path, expired_at);
        assert!(expired_store
            .get_active_session("resumable")
            .unwrap()
            .is_none());
        let listed = expired_store.list_unfinished_sessions().unwrap();
        let expired = listed
            .iter()
            .find(|session| session.session_id == "resumable")
            .expect("expired draft remains visible as a narrow recoverable summary source");
        assert_eq!(
            expired.retention.status_at(expired_at),
            Some(BuilderSessionRetentionStatus::ExpiredRecoverable)
        );

        let resumed = expired_store
            .resume_session("resumable")
            .unwrap()
            .expect("explicit resume renews the draft");
        assert_eq!(resumed.draft_yaml, "RECOVERABLE_PRIVATE_DRAFT");
        assert_eq!(
            resumed.retention.status_at(expired_at),
            Some(BuilderSessionRetentionStatus::Active)
        );
        assert_eq!(resumed.revision, 1);

        let after_original_purge = BuilderSessionStore::new_at(
            &path,
            started_at
                + Duration::days(BUILDER_ACTIVE_RETENTION_DAYS + BUILDER_RECOVERY_WINDOW_DAYS + 1),
        );
        assert!(after_original_purge
            .get_session("purgeable")
            .unwrap()
            .is_none());
        assert!(after_original_purge
            .get_active_session("resumable")
            .unwrap()
            .is_some());
        assert!(!std::fs::read_to_string(&path)
            .unwrap()
            .contains("PURGED_PRIVATE_DRAFT"));
    }

    #[test]
    fn unknown_builder_retention_schema_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions.json");
        let now = fixed_time();
        let mut session = BuilderSession::new("future-schema", BuilderMode::Quick);
        session.retention = BuilderSessionRetention {
            schema_version: 99,
            created_at: Some(now),
            last_activity_at: Some(now),
            expires_at: Some(now + Duration::days(90)),
            purge_after: Some(now + Duration::days(120)),
        };
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "sessions": {"future-schema": session}
            }))
            .unwrap(),
        )
        .unwrap();

        let error = BuilderSessionStore::new_at(&path, now)
            .load()
            .expect_err("unknown retention schema cannot be guessed or overwritten");
        assert!(error
            .to_string()
            .contains("unsupported_builder_session_retention_schema:99"));
    }

    #[test]
    fn oversized_builder_store_is_rejected_before_deserialization() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized-builder-store.json");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(BUILDER_STORE_MAX_BYTES + 1).unwrap();

        let error = BuilderSessionStore::new(&path)
            .load()
            .expect_err("oversized canonical Builder file must fail closed");
        assert!(error
            .to_string()
            .contains("builder_session_store_exceeds_file_limit"));
    }

    #[test]
    fn oversized_builder_session_fails_without_persisting_private_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bounded-builder-store.json");
        let store = BuilderSessionStore::new(&path);
        let mut session = BuilderSession::new("oversized-private-draft", BuilderMode::Socratic);
        session.draft_yaml = "x".repeat(BUILDER_SESSION_MAX_DRAFT_BYTES + 1);

        let error = store
            .save_session(&session)
            .expect_err("oversized Builder draft must be rejected before persistence");
        assert!(error
            .to_string()
            .contains("builder_session_draft_exceeds_bounded_limit"));
        assert!(!path.exists());
    }

    #[test]
    fn excessive_builder_session_count_fails_instead_of_silently_dropping_drafts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("too-many-builder-sessions.json");
        let store = BuilderSessionStore::new(&path);
        let data = BuilderSessionStoreData {
            sessions: (0..=BUILDER_STORE_MAX_SESSIONS)
                .map(|index| {
                    let session_id = format!("bounded-session-{index}");
                    (
                        session_id.clone(),
                        BuilderSession::new(session_id, BuilderMode::Quick),
                    )
                })
                .collect(),
        };

        let error = store
            .save(&data)
            .expect_err("session overflow must be explicit and non-destructive");
        assert!(error
            .to_string()
            .contains("builder_session_store_exceeds_session_limit"));
        assert!(!path.exists());
    }
}
