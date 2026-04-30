use crate::builder::types::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuilderSessionStoreData {
    pub sessions: HashMap<String, BuilderSession>,
}

pub struct BuilderSessionStore {
    path: PathBuf,
}

impl BuilderSessionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self) -> Result<BuilderSessionStoreData> {
        if !self.path.exists() {
            return Ok(BuilderSessionStoreData::default());
        }
        let text = std::fs::read_to_string(&self.path)?;
        let data: BuilderSessionStoreData = serde_json::from_str(&text)?;
        Ok(data)
    }

    pub fn save(&self, data: &BuilderSessionStoreData) -> Result<()> {
        let text = serde_json::to_string_pretty(data)?;
        std::fs::write(&self.path, text)?;
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<BuilderSession>> {
        let data = self.load()?;
        Ok(data.sessions.get(session_id).cloned())
    }

    pub fn save_session(&self, session: &BuilderSession) -> Result<()> {
        let mut data = self.load()?;
        data.sessions
            .insert(session.session_id.clone(), session.clone());
        self.save(&data)
    }

    pub fn remove_session(&self, session_id: &str) -> Result<()> {
        let mut data = self.load()?;
        data.sessions.remove(session_id);
        self.save(&data)
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
}

