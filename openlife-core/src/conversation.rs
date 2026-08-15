//! Canonical Conversation, Turn, and Item persistence.
//!
//! Conversation is the product owner for Chat history. A Chat Turn is not a
//! Task or Run. The caller-owned UUID is its idempotency identity, and the
//! terminal assistant Item is committed atomically with the Turn terminal.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub workspace_root: Option<String>,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    Active,
    Archived,
}

impl ConversationStatus {
    fn from_db(value: &str) -> Result<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => anyhow::bail!("conversation_status_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

impl TurnStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    fn from_db(value: &str) -> Result<Self> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => anyhow::bail!("conversation_turn_status_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationItemKind {
    UserMessage,
    UserSteering,
    AssistantMessage,
    SystemNotice,
}

impl ConversationItemKind {
    fn from_db(value: &str) -> Result<Self> {
        match value {
            "user_message" => Ok(Self::UserMessage),
            "user_steering" => Ok(Self::UserSteering),
            "assistant_message" => Ok(Self::AssistantMessage),
            "system_notice" => Ok(Self::SystemNotice),
            _ => anyhow::bail!("conversation_item_kind_invalid:{value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRecord {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub selected_skill_id: Option<String>,
    pub status: ConversationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderBinding {
    pub profile_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub endpoint_class: String,
    pub config_generation: String,
}

impl ProviderBinding {
    fn validate(&self) -> Result<()> {
        validate_label("provider_profile_id", &self.profile_id, 256)?;
        validate_label("provider_id", &self.provider_id, 128)?;
        validate_label("model_id", &self.model_id, 256)?;
        validate_label("endpoint_class", &self.endpoint_class, 128)?;
        validate_label("config_generation", &self.config_generation, 256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRecord {
    pub id: String,
    pub conversation_id: String,
    pub status: TurnStatus,
    pub request_digest: String,
    pub provider: ProviderBinding,
    pub error_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationItemRecord {
    pub id: String,
    pub conversation_id: String,
    pub turn_id: String,
    pub sequence: u64,
    pub kind: ConversationItemKind,
    pub content: String,
    pub content_digest: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSnapshot {
    pub turn: TurnRecord,
    pub items: Vec<ConversationItemRecord>,
}

/// Opaque proof that the Conversation owner committed the exact current user
/// Item for this Turn. Private fields prevent a transport/model payload from
/// constructing provider authority from identifiers alone.
#[derive(Debug, Clone)]
pub struct ConversationUserMessageProof {
    conversation_id: String,
    turn_id: String,
    item_id: String,
    content_digest: String,
    content_length_bytes: usize,
    issuance_id: uuid::Uuid,
}

impl ConversationUserMessageProof {
    pub fn conversation_id(&self) -> &str {
        &self.conversation_id
    }

    pub fn turn_id(&self) -> &str {
        &self.turn_id
    }

    pub fn item_ref(&self) -> String {
        format!(
            "conversation://{}/turn/{}/item/{}",
            self.conversation_id, self.turn_id, self.item_id
        )
    }

    pub fn content_digest(&self) -> &str {
        &self.content_digest
    }

    pub fn content_length_bytes(&self) -> usize {
        self.content_length_bytes
    }

    pub fn is_live(&self) -> bool {
        !self.issuance_id.is_nil()
    }
}

#[derive(Debug, Clone)]
pub struct BegunChatTurn {
    pub snapshot: TurnSnapshot,
    pub user_message_proof: ConversationUserMessageProof,
}

pub struct BeginChatTurn<'a> {
    pub turn_id: &'a str,
    pub conversation_id: &'a str,
    pub user_message: &'a str,
    pub provider: &'a ProviderBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendedWorkSteering {
    pub item: ConversationItemRecord,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct ConversationStore {
    conn: Arc<Mutex<Connection>>,
    db_path: Option<PathBuf>,
}

impl ConversationStore {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = Self {
            conn: Arc::new(Mutex::new(Connection::open(&path)?)),
            db_path: Some(path),
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let store = Self {
            conn: Arc::new(Mutex::new(Connection::open_in_memory()?)),
            db_path: None,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn open_read_only_existing(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let conn = crate::sqlite_migration::open_existing_read_only(
            path,
            "conversation_store",
            &[
                "projects",
                "conversations",
                "conversation_turns",
                "conversation_items",
            ],
        )?;
        Self::validate_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: Some(path.to_path_buf()),
        })
    }

    pub fn db_path(&self) -> Option<&Path> {
        self.db_path.as_deref()
    }

    fn lock_conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("conversation_store_mutex_poison:{error}"))
    }

    fn initialize(&self) -> Result<()> {
        let mut conn = self.lock_conn()?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS conversation_store_metadata (
                key TEXT PRIMARY KEY, value TEXT NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                workspace_root TEXT,
                revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                project_id TEXT,
                selected_skill_id TEXT,
                status TEXT NOT NULL CHECK(status IN ('active','archived')),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE RESTRICT
             );
             CREATE TABLE IF NOT EXISTS conversation_turns (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN (
                    'running','completed','failed','cancelled','interrupted'
                )),
                request_digest TEXT NOT NULL,
                provider_profile_id TEXT NOT NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                endpoint_class TEXT NOT NULL,
                config_generation TEXT NOT NULL,
                error_code TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                finished_at TEXT,
                FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_conversation_one_running_turn
                ON conversation_turns(conversation_id) WHERE status='running';
             CREATE INDEX IF NOT EXISTS idx_conversation_turn_history
                ON conversation_turns(conversation_id, created_at, id);
             CREATE TABLE IF NOT EXISTS conversation_items (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                turn_id TEXT NOT NULL,
                sequence INTEGER NOT NULL CHECK(sequence > 0),
                kind TEXT NOT NULL CHECK(kind IN (
                    'user_message','user_steering','assistant_message','system_notice'
                )),
                content TEXT NOT NULL,
                content_digest TEXT NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(conversation_id, sequence),
                FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                FOREIGN KEY(turn_id) REFERENCES conversation_turns(id) ON DELETE CASCADE
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_conversation_turn_user_message
                ON conversation_items(turn_id) WHERE kind='user_message';
             CREATE UNIQUE INDEX IF NOT EXISTS idx_conversation_turn_assistant_message
                ON conversation_items(turn_id) WHERE kind='assistant_message';
             CREATE INDEX IF NOT EXISTS idx_conversation_items_turn
                ON conversation_items(turn_id, sequence);
             INSERT INTO conversation_store_metadata(key,value)
             VALUES('schema_version','2') ON CONFLICT(key) DO NOTHING;",
        )?;
        if Self::schema_version(&conn)? == 1 {
            Self::migrate_v1_to_v2(&mut conn)?;
        }
        Self::validate_schema(&conn)
    }

    fn schema_version(conn: &Connection) -> Result<i64> {
        conn.query_row(
            "SELECT value FROM conversation_store_metadata WHERE key='schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .context("conversation_store_schema_version_missing")?
        .parse::<i64>()
        .context("conversation_store_schema_version_invalid")
    }

    fn migrate_v1_to_v2(conn: &mut Connection) -> Result<()> {
        conn.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA legacy_alter_table=ON;")?;
        let migration = (|| -> Result<()> {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS projects (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    workspace_root TEXT,
                    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 ALTER TABLE conversations ADD COLUMN project_id TEXT
                    REFERENCES projects(id) ON DELETE RESTRICT;
                 DROP INDEX IF EXISTS idx_conversation_turn_user_message;
                 DROP INDEX IF EXISTS idx_conversation_turn_assistant_message;
                 DROP INDEX IF EXISTS idx_conversation_items_turn;
                 ALTER TABLE conversation_items RENAME TO conversation_items_v1;
                 CREATE TABLE conversation_items (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    turn_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    kind TEXT NOT NULL CHECK(kind IN (
                        'user_message','user_steering','assistant_message','system_notice'
                    )),
                    content TEXT NOT NULL,
                    content_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    UNIQUE(conversation_id, sequence),
                    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                    FOREIGN KEY(turn_id) REFERENCES conversation_turns(id) ON DELETE CASCADE
                 );
                 INSERT INTO conversation_items SELECT * FROM conversation_items_v1;
                 DROP TABLE conversation_items_v1;
                 CREATE UNIQUE INDEX idx_conversation_turn_user_message
                    ON conversation_items(turn_id) WHERE kind='user_message';
                 CREATE UNIQUE INDEX idx_conversation_turn_assistant_message
                    ON conversation_items(turn_id) WHERE kind='assistant_message';
                 CREATE INDEX idx_conversation_items_turn
                    ON conversation_items(turn_id, sequence);",
            )?;
            let changed = tx.execute(
                "UPDATE conversation_store_metadata SET value='2'
                 WHERE key='schema_version' AND value='1'",
                [],
            )?;
            if changed != 1 {
                anyhow::bail!("conversation_store_v1_migration_version_conflict");
            }
            tx.commit()?;
            Ok(())
        })();
        let restore = conn.execute_batch("PRAGMA legacy_alter_table=OFF; PRAGMA foreign_keys=ON;");
        migration?;
        restore?;
        let violation = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_row([], |_| Ok(()))
            .optional()?;
        if violation.is_some() {
            anyhow::bail!("conversation_store_v1_migration_foreign_key_violation");
        }
        Ok(())
    }

    fn validate_schema(conn: &Connection) -> Result<()> {
        let version = Self::schema_version(conn)?;
        if version != SCHEMA_VERSION {
            anyhow::bail!("conversation_store_schema_version_unsupported:{version}");
        }
        Ok(())
    }

    pub fn create_conversation(&self, id: &str, title: &str) -> Result<ConversationRecord> {
        validate_uuid("conversation_id", id)?;
        validate_label("conversation_title", title, 512)?;
        let now = Utc::now();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO conversations(id,title,status,created_at,updated_at)
             VALUES(?1,?2,'active',?3,?3)",
            params![id, title.trim(), now.to_rfc3339()],
        )?;
        drop(conn);
        self.get_conversation(id)?
            .context("conversation_create_missing")
    }

    pub fn create_project(
        &self,
        id: &str,
        name: &str,
        workspace_root: Option<&str>,
    ) -> Result<ProjectRecord> {
        validate_uuid("project_id", id)?;
        validate_label("project_name", name, 512)?;
        if let Some(root) = workspace_root {
            validate_label("project_workspace_root", root, 4096)?;
        }
        let now = Utc::now().to_rfc3339();
        self.lock_conn()?.execute(
            "INSERT INTO projects(id,name,workspace_root,revision,created_at,updated_at)
             VALUES(?1,?2,?3,1,?4,?4)",
            params![id, name.trim(), workspace_root, now],
        )?;
        self.get_project(id)?.context("project_create_missing")
    }

    pub fn get_project(&self, id: &str) -> Result<Option<ProjectRecord>> {
        self.lock_conn()?
            .query_row(
                "SELECT id,name,workspace_root,revision,created_at,updated_at
                 FROM projects WHERE id=?1",
                [id],
                project_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn project_scope_digest(project: &ProjectRecord) -> String {
        content_digest(&format!(
            "{}\0{}\0{}\0{}",
            project.id,
            project.name,
            project.workspace_root.as_deref().unwrap_or(""),
            project.revision
        ))
    }

    pub fn list_projects(&self, limit: usize) -> Result<Vec<ProjectRecord>> {
        let conn = self.lock_conn()?;
        let mut statement = conn.prepare(
            "SELECT id,name,workspace_root,revision,created_at,updated_at
             FROM projects ORDER BY updated_at DESC,id DESC LIMIT ?1",
        )?;
        let projects = statement
            .query_map([limit.clamp(1, 500) as i64], project_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(anyhow::Error::from)?;
        Ok(projects)
    }

    pub fn assign_conversation_project(
        &self,
        conversation_id: &str,
        project_id: Option<&str>,
    ) -> Result<()> {
        validate_uuid("conversation_id", conversation_id)?;
        if let Some(project_id) = project_id {
            validate_uuid("project_id", project_id)?;
        }
        let changed = self.lock_conn()?.execute(
            "UPDATE conversations SET project_id=?2,updated_at=?3
             WHERE id=?1 AND status='active'
               AND NOT EXISTS (
                   SELECT 1 FROM conversation_turns turn
                   WHERE turn.conversation_id=?1 AND turn.status='running'
               )",
            params![conversation_id, project_id, Utc::now().to_rfc3339()],
        )?;
        require_one(changed, "conversation_project_assignment_unavailable")
    }

    pub fn update_project_scope(
        &self,
        project_id: &str,
        name: &str,
        workspace_root: Option<&str>,
        expected_revision: u64,
    ) -> Result<ProjectRecord> {
        validate_uuid("project_id", project_id)?;
        validate_label("project_name", name, 512)?;
        if let Some(root) = workspace_root {
            validate_label("project_workspace_root", root, 4096)?;
        }
        if expected_revision == 0 {
            anyhow::bail!("project_revision_invalid");
        }
        let changed = self.lock_conn()?.execute(
            "UPDATE projects SET name=?2,workspace_root=?3,revision=revision+1,updated_at=?4
             WHERE id=?1 AND revision=?5",
            params![
                project_id,
                name.trim(),
                workspace_root,
                Utc::now().to_rfc3339(),
                i64::try_from(expected_revision)?
            ],
        )?;
        require_one(changed, "project_scope_revision_conflict")?;
        self.get_project(project_id)?
            .context("project_update_missing")
    }

    pub fn list_conversations(
        &self,
        include_archived: bool,
        limit: usize,
    ) -> Result<Vec<ConversationRecord>> {
        let limit = limit.clamp(1, 500) as i64;
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id,title,project_id,selected_skill_id,status,created_at,updated_at
             FROM conversations WHERE (?1=1 OR status='active')
             ORDER BY updated_at DESC,id DESC LIMIT ?2",
        )?;
        let conversations = stmt
            .query_map(params![include_archived, limit], conversation_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("list conversations")?;
        Ok(conversations)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Option<ConversationRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id,title,project_id,selected_skill_id,status,created_at,updated_at
             FROM conversations WHERE id=?1",
            [id],
            conversation_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> Result<()> {
        validate_label("conversation_title", title, 512)?;
        let changed = self.lock_conn()?.execute(
            "UPDATE conversations SET title=?2,updated_at=?3 WHERE id=?1 AND status='active'",
            params![id, title.trim(), Utc::now().to_rfc3339()],
        )?;
        require_one(changed, "conversation_not_found")
    }

    pub fn set_selected_skill(&self, id: &str, skill_id: Option<&str>) -> Result<()> {
        if let Some(skill_id) = skill_id {
            validate_label("selected_skill_id", skill_id, 256)?;
        }
        let changed = self.lock_conn()?.execute(
            "UPDATE conversations SET selected_skill_id=?2,updated_at=?3
             WHERE id=?1 AND status='active'",
            params![id, skill_id, Utc::now().to_rfc3339()],
        )?;
        require_one(changed, "conversation_not_found")
    }

    pub fn archive_conversation(&self, id: &str) -> Result<()> {
        let changed = self.lock_conn()?.execute(
            "UPDATE conversations SET status='archived',updated_at=?2
             WHERE id=?1 AND status='active'",
            params![id, Utc::now().to_rfc3339()],
        )?;
        require_one(changed, "conversation_not_found_or_archived")
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        let changed = self
            .lock_conn()?
            .execute("DELETE FROM conversations WHERE id=?1", [id])?;
        require_one(changed, "conversation_not_found")
    }

    pub fn begin_chat_turn(&self, input: BeginChatTurn<'_>) -> Result<TurnSnapshot> {
        self.begin_chat_turn_with_proof(input)
            .map(|begun| begun.snapshot)
    }

    pub fn begin_chat_turn_with_proof(&self, input: BeginChatTurn<'_>) -> Result<BegunChatTurn> {
        validate_uuid("turn_id", input.turn_id)?;
        validate_uuid("conversation_id", input.conversation_id)?;
        validate_content("user_message", input.user_message)?;
        input.provider.validate()?;
        let request_digest = content_digest(input.user_message);
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = load_turn_tx(&tx, input.turn_id)? {
            if existing.conversation_id != input.conversation_id
                || existing.request_digest != request_digest
                || existing.provider != *input.provider
            {
                anyhow::bail!("conversation_turn_idempotency_payload_drift");
            }
            let items = load_turn_items_tx(&tx, input.turn_id)?;
            tx.commit()?;
            let proof = user_proof_from_snapshot(&existing, &items)?;
            return Ok(BegunChatTurn {
                snapshot: TurnSnapshot {
                    turn: existing,
                    items,
                },
                user_message_proof: proof,
            });
        }
        let active: Option<String> = tx
            .query_row(
                "SELECT id FROM conversation_turns
                 WHERE conversation_id=?1 AND status='running'",
                [input.conversation_id],
                |row| row.get(0),
            )
            .optional()?;
        if active.is_some() {
            anyhow::bail!("conversation_already_has_running_turn");
        }
        let status: Option<String> = tx
            .query_row(
                "SELECT status FROM conversations WHERE id=?1",
                [input.conversation_id],
                |row| row.get(0),
            )
            .optional()?;
        if status.as_deref() != Some("active") {
            anyhow::bail!("conversation_missing_or_inactive");
        }
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence),0)+1 FROM conversation_items WHERE conversation_id=?1",
            [input.conversation_id],
            |row| row.get(0),
        )?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO conversation_turns(
                id,conversation_id,status,request_digest,provider_profile_id,
                provider_id,model_id,endpoint_class,config_generation,created_at,updated_at
             ) VALUES(?1,?2,'running',?3,?4,?5,?6,?7,?8,?9,?9)",
            params![
                input.turn_id,
                input.conversation_id,
                request_digest,
                input.provider.profile_id,
                input.provider.provider_id,
                input.provider.model_id,
                input.provider.endpoint_class,
                input.provider.config_generation,
                now,
            ],
        )?;
        tx.execute(
            "INSERT INTO conversation_items(
                id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
             ) VALUES(?1,?2,?3,?4,'user_message',?5,?6,?7)",
            params![
                stable_id("item", &[input.turn_id, "user"]),
                input.conversation_id,
                input.turn_id,
                sequence,
                input.user_message,
                request_digest,
                now,
            ],
        )?;
        tx.execute(
            "UPDATE conversations SET updated_at=?2 WHERE id=?1",
            params![input.conversation_id, now],
        )?;
        let turn = load_turn_tx(&tx, input.turn_id)?.context("begun_turn_missing")?;
        let items = load_turn_items_tx(&tx, input.turn_id)?;
        tx.commit()?;
        let proof = user_proof_from_snapshot(&turn, &items)?;
        Ok(BegunChatTurn {
            snapshot: TurnSnapshot { turn, items },
            user_message_proof: proof,
        })
    }

    pub fn append_work_steering(
        &self,
        steering_id: &str,
        conversation_id: &str,
        turn_id: &str,
        content: &str,
    ) -> Result<AppendedWorkSteering> {
        validate_uuid("steering_id", steering_id)?;
        validate_uuid("conversation_id", conversation_id)?;
        validate_uuid("turn_id", turn_id)?;
        validate_content("steering_content", content)?;
        let item_id = stable_id("item", &[turn_id, "steering", steering_id]);
        let steering_digest = content_digest(content);
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = tx
            .query_row(
                "SELECT id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
                 FROM conversation_items WHERE id=?1",
                [&item_id],
                item_from_row,
            )
            .optional()?
        {
            if existing.conversation_id != conversation_id
                || existing.turn_id != turn_id
                || existing.kind != ConversationItemKind::UserSteering
                || existing.content != content
                || existing.content_digest != steering_digest
            {
                anyhow::bail!("conversation_steering_idempotency_payload_drift");
            }
            tx.commit()?;
            return Ok(AppendedWorkSteering {
                item: existing,
                replayed: true,
            });
        }
        let target: Option<(String, String)> = tx
            .query_row(
                "SELECT conversation_id,status FROM conversation_turns WHERE id=?1",
                [turn_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if target
            .as_ref()
            .map(|(conversation, status)| (conversation.as_str(), status.as_str()))
            != Some((conversation_id, "running"))
        {
            anyhow::bail!("conversation_steering_target_not_running");
        }
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence),0)+1 FROM conversation_items WHERE conversation_id=?1",
            [conversation_id],
            |row| row.get(0),
        )?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO conversation_items(
                id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
             ) VALUES(?1,?2,?3,?4,'user_steering',?5,?6,?7)",
            params![
                item_id,
                conversation_id,
                turn_id,
                sequence,
                content,
                steering_digest,
                now
            ],
        )?;
        let item = tx.query_row(
            "SELECT id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
             FROM conversation_items WHERE id=?1",
            [&item_id],
            item_from_row,
        )?;
        tx.execute(
            "UPDATE conversations SET updated_at=?2 WHERE id=?1",
            params![conversation_id, now],
        )?;
        tx.commit()?;
        Ok(AppendedWorkSteering {
            item,
            replayed: false,
        })
    }

    pub fn complete_chat_turn(
        &self,
        turn_id: &str,
        assistant_message: &str,
    ) -> Result<TurnSnapshot> {
        validate_content("assistant_message", assistant_message)?;
        let assistant_digest = content_digest(assistant_message);
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let turn = load_turn_tx(&tx, turn_id)?.context("conversation_turn_missing")?;
        if turn.status == TurnStatus::Completed {
            let items = load_turn_items_tx(&tx, turn_id)?;
            if items.iter().any(|item| {
                item.kind == ConversationItemKind::AssistantMessage
                    && item.content_digest == assistant_digest
                    && item.content == assistant_message
            }) {
                tx.commit()?;
                return Ok(TurnSnapshot { turn, items });
            }
            anyhow::bail!("conversation_turn_terminal_payload_drift");
        }
        if turn.status != TurnStatus::Running {
            anyhow::bail!("conversation_turn_not_running");
        }
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence),0)+1 FROM conversation_items WHERE conversation_id=?1",
            [&turn.conversation_id],
            |row| row.get(0),
        )?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO conversation_items(
                id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
             ) VALUES(?1,?2,?3,?4,'assistant_message',?5,?6,?7)",
            params![
                stable_id("item", &[turn_id, "assistant"]),
                turn.conversation_id,
                turn_id,
                sequence,
                assistant_message,
                assistant_digest,
                now,
            ],
        )?;
        tx.execute(
            "UPDATE conversation_turns SET status='completed',updated_at=?2,finished_at=?2
             WHERE id=?1 AND status='running'",
            params![turn_id, now],
        )?;
        tx.execute(
            "UPDATE conversations SET updated_at=?2 WHERE id=?1",
            params![turn.conversation_id, now],
        )?;
        let turn = load_turn_tx(&tx, turn_id)?.context("completed_turn_missing")?;
        let items = load_turn_items_tx(&tx, turn_id)?;
        tx.commit()?;
        Ok(TurnSnapshot { turn, items })
    }

    /// Work uses the same canonical Conversation/Turn/Item transcript as Chat.
    /// The distinct method name keeps product intent explicit while preserving
    /// one atomic assistant Item + Turn completion owner.
    pub fn complete_work_turn(
        &self,
        turn_id: &str,
        assistant_message: &str,
    ) -> Result<TurnSnapshot> {
        self.complete_chat_turn(turn_id, assistant_message)
    }

    pub fn fail_chat_turn(&self, turn_id: &str, error_code: &str) -> Result<TurnRecord> {
        validate_label("turn_error_code", error_code, 256)?;
        self.terminalize_without_assistant(turn_id, TurnStatus::Failed, Some(error_code))
    }

    pub fn cancel_chat_turn(&self, turn_id: &str) -> Result<TurnRecord> {
        self.terminalize_without_assistant(turn_id, TurnStatus::Cancelled, None)
    }

    pub fn interrupt_incomplete_turns(&self) -> Result<usize> {
        let now = Utc::now().to_rfc3339();
        self.lock_conn()?
            .execute(
                "UPDATE conversation_turns
                 SET status='interrupted',error_code='process_restarted',updated_at=?1,finished_at=?1
                 WHERE status='running'",
                [now],
            )
            .map_err(Into::into)
    }

    fn terminalize_without_assistant(
        &self,
        turn_id: &str,
        status: TurnStatus,
        error_code: Option<&str>,
    ) -> Result<TurnRecord> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = load_turn_tx(&tx, turn_id)?.context("conversation_turn_missing")?;
        if existing.status == status && existing.error_code.as_deref() == error_code {
            tx.commit()?;
            return Ok(existing);
        }
        if existing.status != TurnStatus::Running {
            anyhow::bail!("conversation_turn_not_running");
        }
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE conversation_turns
             SET status=?2,error_code=?3,updated_at=?4,finished_at=?4
             WHERE id=?1 AND status='running'",
            params![turn_id, status.as_str(), error_code, now],
        )?;
        let record = load_turn_tx(&tx, turn_id)?.context("terminal_turn_missing")?;
        tx.commit()?;
        Ok(record)
    }

    pub fn get_turn(&self, turn_id: &str) -> Result<Option<TurnSnapshot>> {
        let conn = self.lock_conn()?;
        let Some(turn) = load_turn_tx(&conn, turn_id)? else {
            return Ok(None);
        };
        Ok(Some(TurnSnapshot {
            items: load_turn_items_tx(&conn, turn_id)?,
            turn,
        }))
    }

    pub fn get_item(&self, item_id: &str) -> Result<Option<ConversationItemRecord>> {
        validate_label("conversation_item_id", item_id, 512)?;
        self.lock_conn()?
            .query_row(
                "SELECT id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
                 FROM conversation_items WHERE id=?1",
                [item_id],
                item_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn latest_turn(&self, conversation_id: &str) -> Result<Option<TurnRecord>> {
        let conn = self.lock_conn()?;
        conn.query_row(
            "SELECT id,conversation_id,status,request_digest,provider_profile_id,
                    provider_id,model_id,endpoint_class,config_generation,error_code,
                    created_at,updated_at,finished_at
             FROM conversation_turns WHERE conversation_id=?1
             ORDER BY created_at DESC,id DESC LIMIT 1",
            [conversation_id],
            turn_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_items(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationItemRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
             FROM conversation_items WHERE conversation_id=?1
             ORDER BY sequence DESC LIMIT ?2",
        )?;
        let mut items = stmt
            .query_map(
                params![conversation_id, limit.clamp(1, 1000) as i64],
                item_from_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        items.reverse();
        Ok(items)
    }

    /// Returns the bounded transcript that may be sent to a model for the
    /// current Turn. Only completed prior Turns and the exact current running
    /// Turn are eligible. User Items left behind by failed, cancelled, or
    /// interrupted Turns remain visible in product history but never become
    /// implicit instructions for a later model call.
    pub fn list_model_context_items(
        &self,
        conversation_id: &str,
        current_turn_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationItemRecord>> {
        validate_uuid("conversation_id", conversation_id)?;
        validate_uuid("current_turn_id", current_turn_id)?;
        let conn = self.lock_conn()?;
        let current: Option<(String, String)> = conn
            .query_row(
                "SELECT conversation_id,status FROM conversation_turns WHERE id=?1",
                [current_turn_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if current
            .as_ref()
            .map(|(owner, status)| (owner.as_str(), status.as_str()))
            != Some((conversation_id, "running"))
        {
            anyhow::bail!("conversation_model_context_turn_not_running");
        }
        let mut stmt = conn.prepare(
            "SELECT i.id,i.conversation_id,i.turn_id,i.sequence,i.kind,i.content,
                    i.content_digest,i.created_at
             FROM conversation_items i
             INNER JOIN conversation_turns t ON t.id=i.turn_id
             WHERE i.conversation_id=?1
               AND (t.status='completed' OR t.id=?2)
             ORDER BY i.sequence DESC LIMIT ?3",
        )?;
        let mut items = stmt
            .query_map(
                params![
                    conversation_id,
                    current_turn_id,
                    limit.clamp(1, 1000) as i64
                ],
                item_from_row,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        items.reverse();
        Ok(items)
    }
}

fn user_proof_from_snapshot(
    turn: &TurnRecord,
    items: &[ConversationItemRecord],
) -> Result<ConversationUserMessageProof> {
    let user = items
        .iter()
        .find(|item| item.kind == ConversationItemKind::UserMessage)
        .context("conversation_turn_user_item_missing")?;
    if user.conversation_id != turn.conversation_id
        || user.turn_id != turn.id
        || user.content_digest != turn.request_digest
    {
        anyhow::bail!("conversation_turn_user_item_owner_mismatch");
    }
    Ok(ConversationUserMessageProof {
        conversation_id: user.conversation_id.clone(),
        turn_id: user.turn_id.clone(),
        item_id: user.id.clone(),
        content_digest: user.content_digest.clone(),
        content_length_bytes: user.content.len(),
        issuance_id: uuid::Uuid::new_v4(),
    })
}

fn require_one(changed: usize, code: &str) -> Result<()> {
    if changed == 1 {
        Ok(())
    } else {
        anyhow::bail!("{code}")
    }
}

fn validate_uuid(field: &str, value: &str) -> Result<()> {
    let parsed = uuid::Uuid::parse_str(value).with_context(|| format!("{field}_invalid"))?;
    if parsed.get_version() != Some(uuid::Version::Random)
        || parsed.hyphenated().to_string() != value
    {
        anyhow::bail!("{field}_invalid");
    }
    Ok(())
}

fn validate_label(field: &str, value: &str, max: usize) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max || trimmed.chars().any(char::is_control) {
        anyhow::bail!("{field}_invalid");
    }
    Ok(())
}

fn validate_content(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 2_000_000 {
        anyhow::bail!("{field}_invalid");
    }
    Ok(())
}

fn content_digest(value: &str) -> String {
    format!(
        "sha256:{}",
        encode_hex(digest(&SHA256, value.as_bytes()).as_ref())
    )
}

fn stable_id(kind: &str, parts: &[&str]) -> String {
    let mut material = kind.as_bytes().to_vec();
    for part in parts {
        material.push(0);
        material.extend_from_slice(part.as_bytes());
    }
    format!("{kind}:{}", encode_hex(digest(&SHA256, &material).as_ref()))
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_time(value: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationRecord> {
    let status: String = row.get(4)?;
    Ok(ConversationRecord {
        id: row.get(0)?,
        title: row.get(1)?,
        project_id: row.get(2)?,
        selected_skill_id: row.get(3)?,
        status: ConversationStatus::from_db(&status).map_err(to_sql_error)?,
        created_at: parse_time(row.get(5)?)?,
        updated_at: parse_time(row.get(6)?)?,
    })
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRecord> {
    let revision = u64::try_from(row.get::<_, i64>(3)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Integer, error.into())
    })?;
    Ok(ProjectRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        workspace_root: row.get(2)?,
        revision,
        created_at: parse_time(row.get(4)?)?,
        updated_at: parse_time(row.get(5)?)?,
    })
}

fn turn_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnRecord> {
    let status: String = row.get(2)?;
    Ok(TurnRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        status: TurnStatus::from_db(&status).map_err(to_sql_error)?,
        request_digest: row.get(3)?,
        provider: ProviderBinding {
            profile_id: row.get(4)?,
            provider_id: row.get(5)?,
            model_id: row.get(6)?,
            endpoint_class: row.get(7)?,
            config_generation: row.get(8)?,
        },
        error_code: row.get(9)?,
        created_at: parse_time(row.get(10)?)?,
        updated_at: parse_time(row.get(11)?)?,
        finished_at: row
            .get::<_, Option<String>>(12)?
            .map(parse_time)
            .transpose()?,
    })
}

fn item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ConversationItemRecord> {
    let kind: String = row.get(4)?;
    Ok(ConversationItemRecord {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        turn_id: row.get(2)?,
        sequence: row.get::<_, i64>(3)? as u64,
        kind: ConversationItemKind::from_db(&kind).map_err(to_sql_error)?,
        content: row.get(5)?,
        content_digest: row.get(6)?,
        created_at: parse_time(row.get(7)?)?,
    })
}

fn load_turn_tx(conn: &Connection, turn_id: &str) -> Result<Option<TurnRecord>> {
    conn.query_row(
        "SELECT id,conversation_id,status,request_digest,provider_profile_id,
                provider_id,model_id,endpoint_class,config_generation,error_code,
                created_at,updated_at,finished_at
         FROM conversation_turns WHERE id=?1",
        [turn_id],
        turn_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn load_turn_items_tx(conn: &Connection, turn_id: &str) -> Result<Vec<ConversationItemRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id,conversation_id,turn_id,sequence,kind,content,content_digest,created_at
         FROM conversation_items WHERE turn_id=?1 ORDER BY sequence",
    )?;
    let items = stmt
        .query_map([turn_id], item_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(items)
}

fn to_sql_error(error: anyhow::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, error.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> String {
        uuid::Uuid::new_v4().hyphenated().to_string()
    }

    fn provider() -> ProviderBinding {
        ProviderBinding {
            profile_id: "profile-default".into(),
            provider_id: "openai".into(),
            model_id: "gpt-test".into(),
            endpoint_class: "cloud".into(),
            config_generation: "generation-1".into(),
        }
    }

    #[test]
    fn v1_store_migrates_projects_and_repeated_steering_without_losing_history() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("conversation-v1.db");
        let conversation_id = id();
        let turn_id = id();
        let item_id = id();
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "PRAGMA foreign_keys=ON;
                 CREATE TABLE conversation_store_metadata (
                    key TEXT PRIMARY KEY, value TEXT NOT NULL
                 ) WITHOUT ROWID;
                 INSERT INTO conversation_store_metadata VALUES('schema_version','1');
                 CREATE TABLE conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    selected_skill_id TEXT,
                    status TEXT NOT NULL CHECK(status IN ('active','archived')),
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 CREATE TABLE conversation_turns (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    status TEXT NOT NULL CHECK(status IN (
                        'running','completed','failed','cancelled','interrupted'
                    )),
                    request_digest TEXT NOT NULL,
                    provider_profile_id TEXT NOT NULL,
                    provider_id TEXT NOT NULL,
                    model_id TEXT NOT NULL,
                    endpoint_class TEXT NOT NULL,
                    config_generation TEXT NOT NULL,
                    error_code TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    finished_at TEXT,
                    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                 );
                 CREATE TABLE conversation_items (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    turn_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL CHECK(sequence > 0),
                    kind TEXT NOT NULL CHECK(kind IN (
                        'user_message','assistant_message','system_notice'
                    )),
                    content TEXT NOT NULL,
                    content_digest TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    UNIQUE(conversation_id, sequence),
                    UNIQUE(turn_id, kind),
                    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                    FOREIGN KEY(turn_id) REFERENCES conversation_turns(id) ON DELETE CASCADE
                 );",
            )
            .unwrap();
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO conversations VALUES(?1,'Migrated',NULL,'active',?2,?2)",
                params![conversation_id, now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO conversation_turns VALUES(
                    ?1,?2,'running','digest','profile-default','openai','gpt-test',
                    'cloud','generation-1',NULL,?3,?3,NULL
                 )",
                params![turn_id, conversation_id, now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO conversation_items VALUES(
                    ?1,?2,?3,1,'user_message','hello','digest',?4
                 )",
                params![item_id, conversation_id, turn_id, now],
            )
            .unwrap();
        }

        let store = ConversationStore::new(&path).unwrap();
        assert_eq!(
            store
                .get_conversation(&conversation_id)
                .unwrap()
                .unwrap()
                .project_id,
            None
        );
        assert_eq!(store.get_turn(&turn_id).unwrap().unwrap().items.len(), 1);
        store
            .append_work_steering(&id(), &conversation_id, &turn_id, "first adjustment")
            .unwrap();
        store
            .append_work_steering(&id(), &conversation_id, &turn_id, "second adjustment")
            .unwrap();
        assert_eq!(
            store
                .get_turn(&turn_id)
                .unwrap()
                .unwrap()
                .items
                .iter()
                .filter(|item| item.kind == ConversationItemKind::UserSteering)
                .count(),
            2
        );
    }

    #[test]
    fn chat_turn_commits_ordered_items_and_terminal_atomically() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        let turn_id = id();
        store
            .create_conversation(&conversation_id, "First chat")
            .unwrap();
        let begun = store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "hello",
                provider: &provider(),
            })
            .unwrap();
        assert_eq!(begun.turn.status, TurnStatus::Running);
        assert_eq!(begun.items.len(), 1);
        let completed = store.complete_chat_turn(&turn_id, "world").unwrap();
        assert_eq!(completed.turn.status, TurnStatus::Completed);
        assert_eq!(completed.items.len(), 2);
        assert_eq!(completed.items[0].kind, ConversationItemKind::UserMessage);
        assert_eq!(
            completed.items[1].kind,
            ConversationItemKind::AssistantMessage
        );
    }

    #[test]
    fn model_context_excludes_non_successful_prior_turns() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        store
            .create_conversation(&conversation_id, "Clean context")
            .unwrap();

        let completed_turn = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &completed_turn,
                conversation_id: &conversation_id,
                user_message: "completed user",
                provider: &provider(),
            })
            .unwrap();
        store
            .complete_chat_turn(&completed_turn, "completed assistant")
            .unwrap();

        let failed_turn = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &failed_turn,
                conversation_id: &conversation_id,
                user_message: "failed user must not leak",
                provider: &provider(),
            })
            .unwrap();
        store.fail_chat_turn(&failed_turn, "test_failure").unwrap();

        let cancelled_turn = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &cancelled_turn,
                conversation_id: &conversation_id,
                user_message: "cancelled user must not leak",
                provider: &provider(),
            })
            .unwrap();
        store.cancel_chat_turn(&cancelled_turn).unwrap();

        let interrupted_turn = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &interrupted_turn,
                conversation_id: &conversation_id,
                user_message: "interrupted user must not leak",
                provider: &provider(),
            })
            .unwrap();
        assert_eq!(store.interrupt_incomplete_turns().unwrap(), 1);

        let current_turn = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &current_turn,
                conversation_id: &conversation_id,
                user_message: "current user",
                provider: &provider(),
            })
            .unwrap();

        let context = store
            .list_model_context_items(&conversation_id, &current_turn, 100)
            .unwrap();
        assert_eq!(
            context
                .iter()
                .map(|item| item.content.as_str())
                .collect::<Vec<_>>(),
            vec!["completed user", "completed assistant", "current user"]
        );
        assert_eq!(store.list_items(&conversation_id, 100).unwrap().len(), 6);
    }

    #[test]
    fn turn_replay_is_exact_and_payload_drift_fails_closed() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        let turn_id = id();
        let provider = provider();
        store
            .create_conversation(&conversation_id, "Replay")
            .unwrap();
        let input = || BeginChatTurn {
            turn_id: &turn_id,
            conversation_id: &conversation_id,
            user_message: "same",
            provider: &provider,
        };
        store.begin_chat_turn(input()).unwrap();
        store.begin_chat_turn(input()).unwrap();
        let error = store
            .begin_chat_turn(BeginChatTurn {
                user_message: "different",
                ..input()
            })
            .unwrap_err();
        assert!(error.to_string().contains("idempotency_payload_drift"));
    }

    #[test]
    fn one_conversation_has_at_most_one_running_turn() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        store
            .create_conversation(&conversation_id, "Serial")
            .unwrap();
        let first = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &first,
                conversation_id: &conversation_id,
                user_message: "first",
                provider: &provider(),
            })
            .unwrap();
        let error = store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &id(),
                conversation_id: &conversation_id,
                user_message: "second",
                provider: &provider(),
            })
            .unwrap_err();
        assert!(error.to_string().contains("already_has_running_turn"));
        store.cancel_chat_turn(&first).unwrap();
    }

    #[test]
    fn restart_marks_only_incomplete_turns_interrupted() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("conversation.db");
        let store = ConversationStore::new(&path).unwrap();
        let conversation_id = id();
        store
            .create_conversation(&conversation_id, "Restart")
            .unwrap();
        let turn_id = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "in flight",
                provider: &provider(),
            })
            .unwrap();
        drop(store);
        let restarted = ConversationStore::new(&path).unwrap();
        assert_eq!(restarted.interrupt_incomplete_turns().unwrap(), 1);
        assert_eq!(
            restarted.get_turn(&turn_id).unwrap().unwrap().turn.status,
            TurnStatus::Interrupted
        );
    }

    #[test]
    fn deleting_conversation_cascades_turns_and_items() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        let turn_id = id();
        store
            .create_conversation(&conversation_id, "Delete")
            .unwrap();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "private",
                provider: &provider(),
            })
            .unwrap();
        store.cancel_chat_turn(&turn_id).unwrap();
        store.delete_conversation(&conversation_id).unwrap();
        assert!(store.get_turn(&turn_id).unwrap().is_none());
        assert!(store.list_items(&conversation_id, 100).unwrap().is_empty());
    }

    #[test]
    fn project_scope_is_revisioned_and_cannot_change_during_a_running_turn() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        let project_id = id();
        store
            .create_conversation(&conversation_id, "Scoped Work")
            .unwrap();
        let project = store
            .create_project(&project_id, "Research", Some("/tmp/research"))
            .unwrap();
        store
            .assign_conversation_project(&conversation_id, Some(&project_id))
            .unwrap();
        assert_eq!(
            store
                .get_conversation(&conversation_id)
                .unwrap()
                .unwrap()
                .project_id
                .as_deref(),
            Some(project_id.as_str())
        );

        let turn_id = id();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "work inside this scope",
                provider: &provider(),
            })
            .unwrap();
        assert!(store
            .assign_conversation_project(&conversation_id, None)
            .unwrap_err()
            .to_string()
            .contains("conversation_project_assignment_unavailable"));
        store.cancel_chat_turn(&turn_id).unwrap();
        let updated = store
            .update_project_scope(
                &project_id,
                "Research v2",
                Some("/tmp/research-v2"),
                project.revision,
            )
            .unwrap();
        assert_eq!(updated.revision, project.revision + 1);
        assert_ne!(
            ConversationStore::project_scope_digest(&project),
            ConversationStore::project_scope_digest(&updated)
        );
    }

    #[test]
    fn work_steering_is_an_ordered_conversation_item_and_replays_exactly() {
        let store = ConversationStore::new_in_memory().unwrap();
        let conversation_id = id();
        let turn_id = id();
        let steering_id = id();
        store
            .create_conversation(&conversation_id, "Steering")
            .unwrap();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &turn_id,
                conversation_id: &conversation_id,
                user_message: "prepare the report",
                provider: &provider(),
            })
            .unwrap();
        let first = store
            .append_work_steering(&steering_id, &conversation_id, &turn_id, "put risks first")
            .unwrap();
        let replay = store
            .append_work_steering(&steering_id, &conversation_id, &turn_id, "put risks first")
            .unwrap();
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(first.item.kind, ConversationItemKind::UserSteering);
        assert_eq!(first.item.sequence, 2);
        assert!(store
            .append_work_steering(&steering_id, &conversation_id, &turn_id, "different",)
            .unwrap_err()
            .to_string()
            .contains("conversation_steering_idempotency_payload_drift"));
    }
}
