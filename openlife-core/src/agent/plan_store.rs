use crate::agent::types::{AgentPlan, PlanStatus, RiskLevel};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct PlanStore {
    conn: Mutex<Connection>,
}

impl PlanStore {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path: PathBuf = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open plans db at {:?}", db_path))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("failed to open in-memory plans db")?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS agent_plans (
                id TEXT PRIMARY KEY,
                run_id TEXT,
                session_id TEXT,
                agent_spec_id TEXT,
                goal TEXT NOT NULL,
                assumptions_json TEXT NOT NULL DEFAULT '[]',
                missing_context_json TEXT NOT NULL DEFAULT '[]',
                steps_json TEXT NOT NULL DEFAULT '[]',
                tool_intents_json TEXT NOT NULL DEFAULT '[]',
                subagent_assignments_json TEXT NOT NULL DEFAULT '[]',
                permission_requirements_json TEXT NOT NULL DEFAULT '[]',
                rollback_plan TEXT,
                success_criteria_json TEXT NOT NULL DEFAULT '[]',
                risk_level TEXT NOT NULL,
                requires_confirmation INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                confirmed_at TEXT,
                completed_at TEXT
            )",
            [],
        )?;
        // Migration: add agent_spec_id column if missing (pre-existing DBs).
        // Only ignore "duplicate column name" errors; propagate other failures.
        if let Err(e) = conn.execute(
            "ALTER TABLE agent_plans ADD COLUMN agent_spec_id TEXT",
            [],
        ) {
            if !e.to_string().contains("duplicate column name") {
                return Err(anyhow::anyhow!(
                    "failed to migrate agent_plans table: {}",
                    e
                ));
            }
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_plans_status ON agent_plans(status, created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_plans_run ON agent_plans(run_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_agent_plans_session ON agent_plans(session_id, created_at DESC)",
            [],
        )?;
        Ok(())
    }

    pub fn create_plan(&self, plan: &AgentPlan) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "INSERT INTO agent_plans (
                id, run_id, session_id, agent_spec_id, goal, assumptions_json,
                missing_context_json, steps_json, tool_intents_json,
                subagent_assignments_json, permission_requirements_json,
                rollback_plan, success_criteria_json, risk_level,
                requires_confirmation, status, created_at, updated_at,
                confirmed_at, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                plan.id,
                plan.run_id.as_ref(),
                plan.session_id.as_ref(),
                plan.agent_spec_id.as_ref(),
                plan.goal,
                serde_json::to_string(&plan.assumptions).unwrap_or_default(),
                serde_json::to_string(&plan.missing_context).unwrap_or_default(),
                serde_json::to_string(&plan.steps).unwrap_or_default(),
                serde_json::to_string(&plan.tool_intents).unwrap_or_default(),
                serde_json::to_string(&plan.subagent_assignments).unwrap_or_default(),
                serde_json::to_string(&plan.permission_requirements).unwrap_or_default(),
                plan.rollback_plan.as_ref(),
                serde_json::to_string(&plan.success_criteria).unwrap_or_default(),
                plan.risk_level.to_string(),
                plan.requires_confirmation as i32,
                plan.status.to_string(),
                plan.created_at.to_rfc3339(),
                plan.updated_at.to_rfc3339(),
                plan.confirmed_at.map(|t| t.to_rfc3339()),
                plan.completed_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn get_plan(&self, plan_id: &str) -> Result<Option<AgentPlan>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, session_id, agent_spec_id, goal, assumptions_json,
                    missing_context_json, steps_json, tool_intents_json,
                    subagent_assignments_json, permission_requirements_json,
                    rollback_plan, success_criteria_json, risk_level,
                    requires_confirmation, status, created_at, updated_at,
                    confirmed_at, completed_at
             FROM agent_plans WHERE id = ?1",
        )?;
        let row = stmt.query_row([plan_id], |row| {
            let assumptions_json: String = row.get(5)?;
            let missing_context_json: String = row.get(6)?;
            let steps_json: String = row.get(7)?;
            let tool_intents_json: String = row.get(8)?;
            let subagent_assignments_json: String = row.get(9)?;
            let permission_requirements_json: String = row.get(10)?;
            let success_criteria_json: String = row.get(12)?;
            let risk_level_str: String = row.get(13)?;
            let requires_confirmation_int: i32 = row.get(14)?;
            let status_str: String = row.get(15)?;
            let created_at_str: String = row.get(16)?;
            let updated_at_str: String = row.get(17)?;
            let confirmed_at_str: Option<String> = row.get(18)?;
            let completed_at_str: Option<String> = row.get(19)?;

            let risk_level = match risk_level_str.as_str() {
                "low" => RiskLevel::Low,
                "medium" => RiskLevel::Medium,
                "high" => RiskLevel::High,
                "critical" => RiskLevel::Critical,
                _ => RiskLevel::Low,
            };

            let status = match status_str.as_str() {
                "draft" => PlanStatus::Draft,
                "published" => PlanStatus::Published,
                "confirmed" => PlanStatus::Confirmed,
                "executing" => PlanStatus::Executing,
                "completed" => PlanStatus::Completed,
                "rejected" => PlanStatus::Rejected,
                "cancelled" => PlanStatus::Cancelled,
                "failed" => PlanStatus::Failed,
                "failed_review" => PlanStatus::FailedReview,
                _ => PlanStatus::Draft,
            };

            let parse_dt = |s: &str| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            15,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })
            };

            Ok(AgentPlan {
                id: row.get(0)?,
                run_id: row.get(1)?,
                session_id: row.get(2)?,
                agent_spec_id: row.get(3)?,
                goal: row.get(4)?,
                assumptions: serde_json::from_str(&assumptions_json).unwrap_or_default(),
                missing_context: serde_json::from_str(&missing_context_json).unwrap_or_default(),
                steps: serde_json::from_str(&steps_json).unwrap_or_default(),
                tool_intents: serde_json::from_str(&tool_intents_json).unwrap_or_default(),
                subagent_assignments: serde_json::from_str(&subagent_assignments_json)
                    .unwrap_or_default(),
                permission_requirements: serde_json::from_str(&permission_requirements_json)
                    .unwrap_or_default(),
                rollback_plan: row.get(11)?,
                success_criteria: serde_json::from_str(&success_criteria_json).unwrap_or_default(),
                risk_level,
                requires_confirmation: requires_confirmation_int != 0,
                status,
                created_at: parse_dt(&created_at_str)?,
                updated_at: parse_dt(&updated_at_str)?,
                confirmed_at: confirmed_at_str.as_deref().map(parse_dt).transpose()?,
                completed_at: completed_at_str.as_deref().map(parse_dt).transpose()?,
            })
        });
        match row {
            Ok(plan) => Ok(Some(plan)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_plans(&self, limit: i64, offset: i64) -> Result<Vec<AgentPlan>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, session_id, agent_spec_id, goal, assumptions_json,
                    missing_context_json, steps_json, tool_intents_json,
                    subagent_assignments_json, permission_requirements_json,
                    rollback_plan, success_criteria_json, risk_level,
                    requires_confirmation, status, created_at, updated_at,
                    confirmed_at, completed_at
             FROM agent_plans
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], Self::row_to_plan)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn list_plans_by_run(&self, run_id: &str) -> Result<Vec<AgentPlan>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, session_id, agent_spec_id, goal, assumptions_json,
                    missing_context_json, steps_json, tool_intents_json,
                    subagent_assignments_json, permission_requirements_json,
                    rollback_plan, success_criteria_json, risk_level,
                    requires_confirmation, status, created_at, updated_at,
                    confirmed_at, completed_at
             FROM agent_plans
             WHERE run_id = ?1
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([run_id], Self::row_to_plan)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn list_plans_by_session(&self, session_id: &str, limit: i64) -> Result<Vec<AgentPlan>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, session_id, agent_spec_id, goal, assumptions_json,
                    missing_context_json, steps_json, tool_intents_json,
                    subagent_assignments_json, permission_requirements_json,
                    rollback_plan, success_criteria_json, risk_level,
                    requires_confirmation, status, created_at, updated_at,
                    confirmed_at, completed_at
             FROM agent_plans
             WHERE session_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session_id, limit], Self::row_to_plan)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn update_plan(&self, plan: &AgentPlan) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        conn.execute(
            "UPDATE agent_plans SET
                run_id = ?2, session_id = ?3, agent_spec_id = ?4, goal = ?5, assumptions_json = ?6,
                missing_context_json = ?7, steps_json = ?8, tool_intents_json = ?9,
                subagent_assignments_json = ?10, permission_requirements_json = ?11,
                rollback_plan = ?12, success_criteria_json = ?13, risk_level = ?14,
                requires_confirmation = ?15, status = ?16, updated_at = ?17,
                confirmed_at = ?18, completed_at = ?19
             WHERE id = ?1",
            params![
                plan.id,
                plan.run_id.as_ref(),
                plan.session_id.as_ref(),
                plan.agent_spec_id.as_ref(),
                plan.goal,
                serde_json::to_string(&plan.assumptions).unwrap_or_default(),
                serde_json::to_string(&plan.missing_context).unwrap_or_default(),
                serde_json::to_string(&plan.steps).unwrap_or_default(),
                serde_json::to_string(&plan.tool_intents).unwrap_or_default(),
                serde_json::to_string(&plan.subagent_assignments).unwrap_or_default(),
                serde_json::to_string(&plan.permission_requirements).unwrap_or_default(),
                plan.rollback_plan.as_ref(),
                serde_json::to_string(&plan.success_criteria).unwrap_or_default(),
                plan.risk_level.to_string(),
                plan.requires_confirmation as i32,
                plan.status.to_string(),
                plan.updated_at.to_rfc3339(),
                plan.confirmed_at.map(|t| t.to_rfc3339()),
                plan.completed_at.map(|t| t.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn plan_count(&self) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poison: {}", e))?;
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM agent_plans", [], |row| row.get(0))?;
        Ok(count)
    }

    fn row_to_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentPlan> {
        let assumptions_json: String = row.get(5)?;
        let missing_context_json: String = row.get(6)?;
        let steps_json: String = row.get(7)?;
        let tool_intents_json: String = row.get(8)?;
        let subagent_assignments_json: String = row.get(9)?;
        let permission_requirements_json: String = row.get(10)?;
        let success_criteria_json: String = row.get(12)?;
        let risk_level_str: String = row.get(13)?;
        let requires_confirmation_int: i32 = row.get(14)?;
        let status_str: String = row.get(15)?;
        let created_at_str: String = row.get(16)?;
        let updated_at_str: String = row.get(17)?;
        let confirmed_at_str: Option<String> = row.get(18)?;
        let completed_at_str: Option<String> = row.get(19)?;

        let risk_level = match risk_level_str.as_str() {
            "low" => RiskLevel::Low,
            "medium" => RiskLevel::Medium,
            "high" => RiskLevel::High,
            "critical" => RiskLevel::Critical,
            _ => RiskLevel::Low,
        };

        let status = match status_str.as_str() {
            "draft" => PlanStatus::Draft,
            "published" => PlanStatus::Published,
            "confirmed" => PlanStatus::Confirmed,
            "executing" => PlanStatus::Executing,
            "completed" => PlanStatus::Completed,
            "rejected" => PlanStatus::Rejected,
            "cancelled" => PlanStatus::Cancelled,
            "failed" => PlanStatus::Failed,
            "failed_review" => PlanStatus::FailedReview,
            _ => PlanStatus::Draft,
        };

        let parse_dt = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        15,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
        };

        Ok(AgentPlan {
            id: row.get(0)?,
            run_id: row.get(1)?,
            session_id: row.get(2)?,
            agent_spec_id: row.get(3)?,
            goal: row.get(4)?,
            assumptions: serde_json::from_str(&assumptions_json).unwrap_or_default(),
            missing_context: serde_json::from_str(&missing_context_json).unwrap_or_default(),
            steps: serde_json::from_str(&steps_json).unwrap_or_default(),
            tool_intents: serde_json::from_str(&tool_intents_json).unwrap_or_default(),
            subagent_assignments: serde_json::from_str(&subagent_assignments_json)
                .unwrap_or_default(),
            permission_requirements: serde_json::from_str(&permission_requirements_json)
                .unwrap_or_default(),
            rollback_plan: row.get(11)?,
            success_criteria: serde_json::from_str(&success_criteria_json).unwrap_or_default(),
            risk_level,
            requires_confirmation: requires_confirmation_int != 0,
            status,
            created_at: parse_dt(&created_at_str)?,
            updated_at: parse_dt(&updated_at_str)?,
            confirmed_at: confirmed_at_str.as_deref().map(parse_dt).transpose()?,
            completed_at: completed_at_str.as_deref().map(parse_dt).transpose()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{
        AgentEventActor, AgentRunEvent, AgentRunEventType, PlanStep, RiskLevel, ToolIntent,
    };

    fn create_test_plan(goal: &str, risk_level: RiskLevel) -> AgentPlan {
        let mut plan = AgentPlan::new(goal, risk_level);
        plan.assumptions = vec!["User has valid API key".into()];
        plan.steps = vec![
            PlanStep {
                index: 0,
                description: "Read current config".into(),
                tool_intent: Some("file.read".into()),
                expected_output: Some("config content".into()),
                depends_on: vec![],
            },
            PlanStep {
                index: 1,
                description: "Analyze and propose changes".into(),
                tool_intent: None,
                expected_output: Some("analysis result".into()),
                depends_on: vec![0],
            },
        ];
        plan.tool_intents = vec![ToolIntent {
            tool_name: "file.read".into(),
            purpose: "Read current configuration".into(),
            risk_level: RiskLevel::Low,
            is_write: false,
            parameters_summary: Some("path: config.yaml".into()),
        }];
        plan.success_criteria = vec!["Config is analyzed correctly".into()];
        plan
    }

    #[test]
    fn test_create_and_get_plan() {
        let store = PlanStore::new_in_memory().unwrap();
        let plan = create_test_plan("Analyze project configuration", RiskLevel::Low);
        store.create_plan(&plan).unwrap();

        let fetched = store.get_plan(&plan.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.goal, "Analyze project configuration");
        assert_eq!(fetched.risk_level, RiskLevel::Low);
        assert!(!fetched.requires_confirmation);
        assert_eq!(fetched.status, PlanStatus::Draft);
        assert_eq!(fetched.assumptions.len(), 1);
        assert_eq!(fetched.steps.len(), 2);
        assert_eq!(fetched.tool_intents.len(), 1);
        assert_eq!(fetched.success_criteria.len(), 1);
    }

    #[test]
    fn test_plan_requires_confirmation_for_non_low_risk() {
        // Low risk should NOT require confirmation
        let low_plan = AgentPlan::new("Low risk task", RiskLevel::Low);
        assert!(!low_plan.requires_confirmation);

        // Medium risk should require confirmation
        let medium_plan = AgentPlan::new("Medium risk task", RiskLevel::Medium);
        assert!(medium_plan.requires_confirmation);

        // High risk should require confirmation
        let high_plan = AgentPlan::new("High risk task", RiskLevel::High);
        assert!(high_plan.requires_confirmation);

        // Critical risk should require confirmation
        let critical_plan = AgentPlan::new("Critical risk task", RiskLevel::Critical);
        assert!(critical_plan.requires_confirmation);
    }

    #[test]
    fn test_plan_round_trip_all_fields() {
        let store = PlanStore::new_in_memory().unwrap();
        let mut plan = AgentPlan::new("Full plan", RiskLevel::Medium);

        plan.run_id = Some("run-001".into());
        plan.session_id = Some("session-001".into());
        plan.assumptions = vec!["Assumption A".into(), "Assumption B".into()];
        plan.missing_context = vec!["Missing: user config path".into()];
        plan.steps = vec![PlanStep {
            index: 0,
            description: "Step 1".into(),
            tool_intent: Some("tool.x".into()),
            expected_output: None,
            depends_on: vec![],
        }];
        plan.tool_intents = vec![ToolIntent {
            tool_name: "tool.x".into(),
            purpose: "do something".into(),
            risk_level: RiskLevel::Medium,
            is_write: true,
            parameters_summary: None,
        }];
        plan.permission_requirements = vec![];
        plan.rollback_plan = Some("Undo changes by...".into());
        plan.success_criteria = vec!["Task completed".into()];
        plan.publish();
        assert_eq!(plan.status, PlanStatus::Published);

        store.create_plan(&plan).unwrap();

        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.id, plan.id);
        assert_eq!(fetched.run_id, Some("run-001".into()));
        assert_eq!(fetched.session_id, Some("session-001".into()));
        assert_eq!(fetched.goal, "Full plan");
        assert_eq!(fetched.assumptions.len(), 2);
        assert_eq!(fetched.missing_context.len(), 1);
        assert_eq!(fetched.steps.len(), 1);
        assert_eq!(fetched.tool_intents.len(), 1);
        assert!(fetched.tool_intents[0].is_write);
        assert_eq!(fetched.rollback_plan, Some("Undo changes by...".into()));
        assert_eq!(fetched.success_criteria.len(), 1);
        assert_eq!(fetched.risk_level, RiskLevel::Medium);
        assert!(fetched.requires_confirmation);
        assert_eq!(fetched.status, PlanStatus::Published);
    }

    #[test]
    fn test_list_plans() {
        let store = PlanStore::new_in_memory().unwrap();
        for i in 0..5 {
            let plan = create_test_plan(&format!("Plan {}", i), RiskLevel::Low);
            store.create_plan(&plan).unwrap();
        }

        let plans = store.list_plans(10, 0).unwrap();
        assert_eq!(plans.len(), 5);
        // Should be in descending created_at order
        assert!(plans[0].created_at >= plans[4].created_at);
    }

    #[test]
    fn test_list_plans_by_run() {
        let store = PlanStore::new_in_memory().unwrap();

        let mut plan_a = create_test_plan("Plan A", RiskLevel::Low);
        plan_a.run_id = Some("run-001".into());
        store.create_plan(&plan_a).unwrap();

        let mut plan_b = create_test_plan("Plan B", RiskLevel::Medium);
        plan_b.run_id = Some("run-001".into());
        store.create_plan(&plan_b).unwrap();

        let mut plan_c = create_test_plan("Plan C", RiskLevel::Low);
        plan_c.run_id = Some("run-002".into());
        store.create_plan(&plan_c).unwrap();

        let by_run1 = store.list_plans_by_run("run-001").unwrap();
        assert_eq!(by_run1.len(), 2);

        let by_run2 = store.list_plans_by_run("run-002").unwrap();
        assert_eq!(by_run2.len(), 1);

        let by_none = store.list_plans_by_run("run-nonexistent").unwrap();
        assert_eq!(by_none.len(), 0);
    }

    #[test]
    fn test_list_plans_by_session() {
        let store = PlanStore::new_in_memory().unwrap();

        let mut plan_a = create_test_plan("Plan A", RiskLevel::Low);
        plan_a.session_id = Some("sess-001".into());
        store.create_plan(&plan_a).unwrap();

        let mut plan_b = create_test_plan("Plan B", RiskLevel::Low);
        plan_b.session_id = Some("sess-002".into());
        store.create_plan(&plan_b).unwrap();

        let by_sess1 = store.list_plans_by_session("sess-001", 10).unwrap();
        assert_eq!(by_sess1.len(), 1);
        assert_eq!(by_sess1[0].goal, "Plan A");
    }

    #[test]
    fn test_plan_count() {
        let store = PlanStore::new_in_memory().unwrap();
        assert_eq!(store.plan_count().unwrap(), 0);

        let plan = create_test_plan("Test plan", RiskLevel::Low);
        store.create_plan(&plan).unwrap();
        assert_eq!(store.plan_count().unwrap(), 1);
    }

    #[test]
    fn test_update_plan() {
        let store = PlanStore::new_in_memory().unwrap();
        let mut plan = create_test_plan("Original goal", RiskLevel::Low);
        store.create_plan(&plan).unwrap();

        plan.goal = "Updated goal".into();
        plan.risk_level = RiskLevel::Medium;
        plan.requires_confirmation = true;
        plan.confirm();
        assert_eq!(plan.status, PlanStatus::Confirmed);
        store.update_plan(&plan).unwrap();

        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.goal, "Updated goal");
        assert_eq!(fetched.risk_level, RiskLevel::Medium);
        assert!(fetched.requires_confirmation);
        assert_eq!(fetched.status, PlanStatus::Confirmed);
        assert!(fetched.confirmed_at.is_some());
    }

    #[test]
    fn test_plan_lifecycle_transitions() {
        let store = PlanStore::new_in_memory().unwrap();
        let mut plan = create_test_plan("Lifecycle test", RiskLevel::High);
        store.create_plan(&plan).unwrap();

        // Draft -> Published
        plan.publish();
        store.update_plan(&plan).unwrap();
        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Published);

        // Published -> Confirmed
        plan.confirm();
        store.update_plan(&plan).unwrap();
        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Confirmed);
        assert!(fetched.confirmed_at.is_some());

        // Confirmed -> Executing
        plan.start_execution();
        store.update_plan(&plan).unwrap();
        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Executing);

        // Executing -> Completed
        plan.complete();
        store.update_plan(&plan).unwrap();
        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Completed);
        assert!(fetched.completed_at.is_some());
    }

    #[test]
    fn test_plan_rejection() {
        let store = PlanStore::new_in_memory().unwrap();
        let mut plan = create_test_plan("Rejectable plan", RiskLevel::Low);
        plan.publish();
        store.create_plan(&plan).unwrap();

        plan.reject();
        store.update_plan(&plan).unwrap();

        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.status, PlanStatus::Rejected);
    }

    #[test]
    fn test_has_write_intents() {
        let plan = AgentPlan {
            tool_intents: vec![
                ToolIntent {
                    tool_name: "web.search".into(),
                    purpose: "search".into(),
                    risk_level: RiskLevel::Low,
                    is_write: false,
                    parameters_summary: None,
                },
                ToolIntent {
                    tool_name: "file.write_proposal".into(),
                    purpose: "write file".into(),
                    risk_level: RiskLevel::High,
                    is_write: true,
                    parameters_summary: None,
                },
            ],
            ..create_test_plan("test", RiskLevel::Medium)
        };

        assert!(plan.has_write_intents());
    }

    #[test]
    fn test_has_handoff_assignments() {
        let plan = AgentPlan {
            subagent_assignments: vec![crate::agent::types::SubAgentAssignment {
                agent_role: "reviewer".into(),
                task: "review output".into(),
                delegation_mode: "handoff".into(),
            }],
            ..create_test_plan("test", RiskLevel::Medium)
        };

        assert!(plan.has_handoff_assignments());
    }

    // ── Event integration tests ──────────────────────────────────────────

    /// When a plan is created, a plan.created event is recorded via the AgentRunEventStore.
    #[test]
    fn test_plan_created_event_recorded() {
        use crate::agent::event_store::AgentRunEventStore;

        let plan_store = PlanStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();

        let run_id = "test-plan-event-run-001";
        let mut plan = create_test_plan("Plan with event", RiskLevel::Medium);
        plan.run_id = Some(run_id.to_string());

        // Persist the plan
        plan_store.create_plan(&plan).unwrap();

        // Record the plan.created event
        let event = AgentRunEvent::new(
            run_id,
            AgentRunEventType::PlanCreated,
            AgentEventActor::Agent,
            format!("Plan created: {}", plan.goal),
            serde_json::json!({
                "plan_id": plan.id,
                "goal": plan.goal,
                "risk_level": plan.risk_level.to_string(),
                "requires_confirmation": plan.requires_confirmation,
                "step_count": plan.steps.len(),
                "tool_intent_count": plan.tool_intents.len()
            }),
        );

        event_store.append_event(&event).unwrap();

        // Verify the event was stored
        let events = event_store.list_events_by_run(run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, AgentRunEventType::PlanCreated);
        assert_eq!(events[0].actor, AgentEventActor::Agent);
        assert_eq!(
            events[0].payload.get("plan_id").unwrap().as_str().unwrap(),
            plan.id
        );
        assert_eq!(
            events[0]
                .payload
                .get("risk_level")
                .unwrap()
                .as_str()
                .unwrap(),
            "medium"
        );

        // Verify the plan is also accessible from the plan store
        let fetched = plan_store.get_plan(&plan.id).unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().goal, "Plan with event");
    }

    /// Event isolation: plan.created events from different runs are independent.
    #[test]
    fn test_plan_created_events_isolated_by_run() {
        use crate::agent::event_store::AgentRunEventStore;

        let plan_store = PlanStore::new_in_memory().unwrap();
        let event_store = AgentRunEventStore::new_in_memory().unwrap();

        // Create two plans under different runs
        for (i, run_id) in ["run-a", "run-b"].iter().enumerate() {
            let mut plan = create_test_plan(&format!("Plan {}", i), RiskLevel::Low);
            plan.run_id = Some(run_id.to_string());
            plan_store.create_plan(&plan).unwrap();

            let event = AgentRunEvent::new(
                run_id,
                AgentRunEventType::PlanCreated,
                AgentEventActor::Agent,
                format!("Plan {} created", i),
                serde_json::json!({"plan_id": plan.id}),
            );
            event_store.append_event(&event).unwrap();
        }

        assert_eq!(event_store.list_events_by_run("run-a").unwrap().len(), 1);
        assert_eq!(event_store.list_events_by_run("run-b").unwrap().len(), 1);
        assert_eq!(
            event_store
                .list_events_by_run("run-nonexistent")
                .unwrap()
                .len(),
            0
        );
    }

    // ── P7 stabilization: plan-bound AgentSpec round-trip ────────────

    #[test]
    fn test_agent_plan_agent_spec_id_round_trips_through_serde() {
        let plan = create_test_plan("plan with spec", RiskLevel::Low)
            .clone();
        let mut plan = plan;
        plan.agent_spec_id = Some("main.alt".to_string());

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("agentSpecId"));
        assert!(json.contains("main.alt"));

        let deserialized: AgentPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_spec_id, Some("main.alt".to_string()));
    }

    #[test]
    fn test_agent_plan_agent_spec_id_round_trips_through_store() {
        let store = PlanStore::new_in_memory().unwrap();
        let plan = AgentPlan::new("plan with spec", RiskLevel::Low)
            .with_agent_spec("main.custom");
        store.create_plan(&plan).unwrap();

        let fetched = store.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(fetched.agent_spec_id, Some("main.custom".to_string()));
    }

    #[test]
    fn test_agent_plan_without_spec_id_deserializes_as_none() {
        let plan = create_test_plan("no spec plan", RiskLevel::Low);
        let json = serde_json::to_string(&plan).unwrap();
        let deserialized: AgentPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_spec_id, None);
    }
}
