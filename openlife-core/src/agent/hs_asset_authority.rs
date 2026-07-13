use crate::agent::heuristic_store::{HeuristicLifecycleStatus, HeuristicQuery, HeuristicStore};
use crate::life_model::{
    collaboration_guidance_digest_from_records, collaboration_guidance_digest_from_view,
    collaboration_guidance_summaries, extract_hs_compatibility_view_from_yaml, LifeModel,
};
use anyhow::{Context, Result};
use chrono::Utc;
use ring::digest::{digest, SHA256};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use uuid::Uuid;

const AUTHORITY_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HSAssetCategory {
    Identity,
    Goals,
    Capabilities,
    State,
    Relationships,
    Preferences,
    CollaborationGuidance,
}

impl HSAssetCategory {
    pub const ALL: [Self; 7] = [
        Self::Identity,
        Self::Goals,
        Self::Capabilities,
        Self::State,
        Self::Relationships,
        Self::Preferences,
        Self::CollaborationGuidance,
    ];
}

impl std::fmt::Display for HSAssetCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Identity => "identity",
            Self::Goals => "goals",
            Self::Capabilities => "capabilities",
            Self::State => "state",
            Self::Relationships => "relationships",
            Self::Preferences => "preferences",
            Self::CollaborationGuidance => "collaboration_guidance",
        };
        write!(f, "{value}")
    }
}

impl FromStr for HSAssetCategory {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "identity" => Ok(Self::Identity),
            "goals" => Ok(Self::Goals),
            "capabilities" => Ok(Self::Capabilities),
            "state" => Ok(Self::State),
            "relationships" => Ok(Self::Relationships),
            "preferences" => Ok(Self::Preferences),
            "collaboration_guidance" => Ok(Self::CollaborationGuidance),
            other => Err(anyhow::anyhow!("unknown HS asset category: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HSAssetOwner {
    LifeModelYaml,
    AcceptedHsStore,
}

impl std::fmt::Display for HSAssetOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LifeModelYaml => write!(f, "lifemodel_yaml"),
            Self::AcceptedHsStore => write!(f, "accepted_hs_store"),
        }
    }
}

impl FromStr for HSAssetOwner {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "lifemodel_yaml" => Ok(Self::LifeModelYaml),
            "accepted_hs_store" => Ok(Self::AcceptedHsStore),
            other => Err(anyhow::anyhow!("unknown HS asset owner: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HSAssetAuthorityRecord {
    pub category: HSAssetCategory,
    pub owner: HSAssetOwner,
    pub revision: i64,
    pub previous_owner: Option<HSAssetOwner>,
    pub last_evidence_digest: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HSAssetWriteKind {
    ProductMutation,
    DerivedCompatibilityProjection,
}

#[derive(Debug, Clone, Copy)]
pub struct HSAssetWriteRequest {
    pub category: HSAssetCategory,
    pub source_owner: HSAssetOwner,
    pub target_owner: HSAssetOwner,
    pub kind: HSAssetWriteKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowParityReceipt {
    pub evidence_id: String,
    pub category: HSAssetCategory,
    pub authority_revision: i64,
    pub canonical_digest: String,
    pub compatibility_digest: String,
    pub repeated_materialization_digest: String,
    pub deterministic: bool,
    pub parity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackRehearsalReceipt {
    pub evidence_id: String,
    pub category: HSAssetCategory,
    pub authority_revision: i64,
    pub succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductScenarioReceipt {
    pub evidence_id: String,
    pub category: HSAssetCategory,
    pub authority_revision: i64,
    pub scenario_ref: String,
    pub observed_owner: HSAssetOwner,
    pub output_digest: String,
    pub selected_asset_count: usize,
    pub succeeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollaborationGuidanceProjection {
    pub yaml: String,
    pub canonical_digest: String,
    pub compatibility_digest: String,
    pub repeated_materialization_digest: String,
    pub asset_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollaborationGuidanceCutoverStatus {
    ShadowEvidencePending,
    Promoted,
    AlreadyPromoted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollaborationGuidanceCutoverReport {
    pub status: CollaborationGuidanceCutoverStatus,
    pub authority: HSAssetAuthorityRecord,
    pub projection: CollaborationGuidanceProjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredEvidence {
    evidence_id: String,
    category: HSAssetCategory,
    authority_revision: i64,
    evidence_kind: String,
    canonical_digest: Option<String>,
    compatibility_digest: Option<String>,
    repeated_digest: Option<String>,
    scenario_ref: Option<String>,
    observed_owner: Option<HSAssetOwner>,
    output_digest: Option<String>,
    selected_asset_count: usize,
    succeeded: bool,
}

pub struct HSAssetAuthorityRegistry {
    conn: Mutex<Connection>,
}

impl HSAssetAuthorityRegistry {
    pub fn new(db_path: impl Into<PathBuf>) -> Result<Self> {
        let db_path = db_path.into();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("failed to open HS authority registry at {db_path:?}"))?;
        let registry = Self {
            conn: Mutex::new(conn),
        };
        registry.init_tables()?;
        Ok(registry)
    }

    pub fn new_in_memory() -> Result<Self> {
        let registry = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
        };
        registry.init_tables()?;
        Ok(registry)
    }

    fn init_tables(&self) -> Result<()> {
        let mut conn = self.lock_conn()?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS hs_asset_authority_meta (
                 schema_version INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS hs_asset_authority (
                 category TEXT PRIMARY KEY,
                 owner TEXT NOT NULL,
                 revision INTEGER NOT NULL,
                 previous_owner TEXT,
                 last_evidence_digest TEXT,
                 updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS hs_asset_transition_evidence (
                 evidence_id TEXT PRIMARY KEY,
                 category TEXT NOT NULL,
                 authority_revision INTEGER NOT NULL,
                 evidence_kind TEXT NOT NULL,
                 canonical_digest TEXT,
                 compatibility_digest TEXT,
                 repeated_digest TEXT,
                 scenario_ref TEXT,
                 observed_owner TEXT,
                 output_digest TEXT,
                 selected_asset_count INTEGER NOT NULL DEFAULT 0,
                 succeeded INTEGER NOT NULL,
                 created_at TEXT NOT NULL,
                 FOREIGN KEY(category) REFERENCES hs_asset_authority(category)
             );
             CREATE INDEX IF NOT EXISTS idx_hs_transition_evidence_category_revision
             ON hs_asset_transition_evidence(category, authority_revision, evidence_kind);",
        )?;
        let version = conn
            .query_row(
                "SELECT schema_version FROM hs_asset_authority_meta LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        match version {
            Some(version) if version != AUTHORITY_SCHEMA_VERSION => {
                return Err(anyhow::anyhow!(
                    "unsupported HS authority schema version: {version}"
                ));
            }
            Some(_) => {}
            None => {
                conn.execute(
                    "INSERT INTO hs_asset_authority_meta(schema_version) VALUES (?1)",
                    [AUTHORITY_SCHEMA_VERSION],
                )?;
            }
        }
        let tx = conn.transaction()?;
        let now = Utc::now().to_rfc3339();
        for category in HSAssetCategory::ALL {
            tx.execute(
                "INSERT OR IGNORE INTO hs_asset_authority (
                     category, owner, revision, previous_owner, last_evidence_digest, updated_at
                 ) VALUES (?1, ?2, 1, NULL, NULL, ?3)",
                params![
                    category.to_string(),
                    HSAssetOwner::LifeModelYaml.to_string(),
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn authority(&self, category: HSAssetCategory) -> Result<HSAssetAuthorityRecord> {
        let conn = self.lock_conn()?;
        load_authority(&*conn, category)
    }

    pub fn list_authorities(&self) -> Result<Vec<HSAssetAuthorityRecord>> {
        HSAssetCategory::ALL
            .iter()
            .map(|category| self.authority(*category))
            .collect()
    }

    pub fn require_read_owner(
        &self,
        category: HSAssetCategory,
        expected_owner: HSAssetOwner,
    ) -> Result<HSAssetAuthorityRecord> {
        let record = self.authority(category)?;
        if record.owner != expected_owner {
            return Err(anyhow::anyhow!(
                "HS asset read authority mismatch for {category}: expected {expected_owner}, current {}",
                record.owner
            ));
        }
        Ok(record)
    }

    pub fn authorize_write(&self, request: HSAssetWriteRequest) -> Result<()> {
        let record = self.authority(request.category)?;
        let allowed = match request.kind {
            HSAssetWriteKind::ProductMutation => {
                request.source_owner == record.owner && request.target_owner == record.owner
            }
            HSAssetWriteKind::DerivedCompatibilityProjection => {
                record.owner == HSAssetOwner::AcceptedHsStore
                    && request.source_owner == HSAssetOwner::AcceptedHsStore
                    && request.target_owner == HSAssetOwner::LifeModelYaml
            }
        };
        if !allowed {
            return Err(anyhow::anyhow!(
                "HS asset write authority denied for {}: owner={}, source={}, target={}, kind={:?}",
                request.category,
                record.owner,
                request.source_owner,
                request.target_owner,
                request.kind
            ));
        }
        Ok(())
    }

    pub fn record_shadow_parity(
        &self,
        category: HSAssetCategory,
        expected_revision: i64,
        canonical_digest: impl Into<String>,
        compatibility_digest: impl Into<String>,
        repeated_materialization_digest: impl Into<String>,
    ) -> Result<ShadowParityReceipt> {
        let canonical_digest = canonical_digest.into();
        let compatibility_digest = compatibility_digest.into();
        let repeated_materialization_digest = repeated_materialization_digest.into();
        require_digest(&canonical_digest)?;
        require_digest(&compatibility_digest)?;
        require_digest(&repeated_materialization_digest)?;
        let authority = self.authority(category)?;
        require_revision(&authority, expected_revision)?;
        if authority.owner != HSAssetOwner::LifeModelYaml {
            return Err(anyhow::anyhow!(
                "shadow parity is only valid before cutover from lifemodel_yaml"
            ));
        }
        let deterministic = canonical_digest == repeated_materialization_digest;
        let parity = canonical_digest == compatibility_digest;
        let evidence_id = evidence_id("shadow_parity");
        self.insert_evidence(StoredEvidence {
            evidence_id: evidence_id.clone(),
            category,
            authority_revision: expected_revision,
            evidence_kind: "shadow_parity".into(),
            canonical_digest: Some(canonical_digest.clone()),
            compatibility_digest: Some(compatibility_digest.clone()),
            repeated_digest: Some(repeated_materialization_digest.clone()),
            scenario_ref: None,
            observed_owner: None,
            output_digest: None,
            selected_asset_count: 0,
            succeeded: deterministic && parity,
        })?;
        Ok(ShadowParityReceipt {
            evidence_id,
            category,
            authority_revision: expected_revision,
            canonical_digest,
            compatibility_digest,
            repeated_materialization_digest,
            deterministic,
            parity,
        })
    }

    pub fn rehearse_rollback(
        &self,
        category: HSAssetCategory,
        expected_revision: i64,
    ) -> Result<RollbackRehearsalReceipt> {
        let mut conn = self.lock_conn()?;
        let authority = load_authority(&*conn, category)?;
        require_revision(&authority, expected_revision)?;
        if authority.owner != HSAssetOwner::LifeModelYaml {
            return Err(anyhow::anyhow!(
                "rollback rehearsal must run before the first category cutover"
            ));
        }

        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE hs_asset_authority SET owner = ?2 WHERE category = ?1",
            params![
                category.to_string(),
                HSAssetOwner::AcceptedHsStore.to_string()
            ],
        )?;
        let promoted_owner = load_owner_tx(&tx, category)?;
        tx.execute(
            "UPDATE hs_asset_authority SET owner = ?2 WHERE category = ?1",
            params![
                category.to_string(),
                HSAssetOwner::LifeModelYaml.to_string()
            ],
        )?;
        let restored_owner = load_owner_tx(&tx, category)?;
        let succeeded = promoted_owner == HSAssetOwner::AcceptedHsStore
            && restored_owner == HSAssetOwner::LifeModelYaml;
        tx.rollback()?;
        drop(conn);

        let evidence_id = evidence_id("rollback_rehearsal");
        self.insert_evidence(StoredEvidence {
            evidence_id: evidence_id.clone(),
            category,
            authority_revision: expected_revision,
            evidence_kind: "rollback_rehearsal".into(),
            canonical_digest: None,
            compatibility_digest: None,
            repeated_digest: None,
            scenario_ref: None,
            observed_owner: None,
            output_digest: Some(digest_string(&format!(
                "{category}:{expected_revision}:{promoted_owner}:{restored_owner}"
            ))),
            selected_asset_count: 0,
            succeeded,
        })?;
        Ok(RollbackRehearsalReceipt {
            evidence_id,
            category,
            authority_revision: expected_revision,
            succeeded,
        })
    }

    pub fn record_product_scenario(
        &self,
        category: HSAssetCategory,
        expected_revision: i64,
        scenario_ref: impl Into<String>,
        observed_owner: HSAssetOwner,
        selected_asset_ids: &[String],
        output_digest: impl Into<String>,
    ) -> Result<ProductScenarioReceipt> {
        let scenario_ref = scenario_ref.into();
        let output_digest = output_digest.into();
        let authority = self.authority(category)?;
        require_revision(&authority, expected_revision)?;
        if authority.owner != HSAssetOwner::LifeModelYaml {
            return Err(anyhow::anyhow!(
                "pre-cutover product scenario evidence requires lifemodel_yaml authority"
            ));
        }
        if scenario_ref.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "product scenario evidence reference is required"
            ));
        }
        require_digest(&output_digest)?;
        let selected_asset_count = selected_asset_ids
            .iter()
            .filter(|asset_id| !asset_id.trim().is_empty())
            .count();
        let succeeded = observed_owner == HSAssetOwner::AcceptedHsStore && selected_asset_count > 0;
        let evidence_id = evidence_id("product_scenario");
        self.insert_evidence(StoredEvidence {
            evidence_id: evidence_id.clone(),
            category,
            authority_revision: expected_revision,
            evidence_kind: "product_scenario".into(),
            canonical_digest: None,
            compatibility_digest: None,
            repeated_digest: None,
            scenario_ref: Some(scenario_ref.clone()),
            observed_owner: Some(observed_owner),
            output_digest: Some(output_digest.clone()),
            selected_asset_count,
            succeeded,
        })?;
        Ok(ProductScenarioReceipt {
            evidence_id,
            category,
            authority_revision: expected_revision,
            scenario_ref,
            observed_owner,
            output_digest,
            selected_asset_count,
            succeeded,
        })
    }

    /// Return only durable, successful runtime evidence for the current
    /// authority revision. Callers cannot promote from a summary string held
    /// in memory; the receipt must already exist in this registry.
    pub fn latest_successful_product_scenario(
        &self,
        category: HSAssetCategory,
        authority_revision: i64,
    ) -> Result<Option<ProductScenarioReceipt>> {
        let conn = self.lock_conn()?;
        let evidence_id = conn
            .query_row(
                "SELECT evidence_id
                 FROM hs_asset_transition_evidence
                 WHERE category = ?1
                   AND authority_revision = ?2
                   AND evidence_kind = 'product_scenario'
                   AND succeeded = 1
                 ORDER BY created_at DESC, evidence_id DESC
                 LIMIT 1",
                params![category.to_string(), authority_revision],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(evidence_id) = evidence_id else {
            return Ok(None);
        };
        let evidence = load_evidence(&conn, &evidence_id)?;
        Ok(Some(ProductScenarioReceipt {
            evidence_id: evidence.evidence_id,
            category: evidence.category,
            authority_revision: evidence.authority_revision,
            scenario_ref: evidence.scenario_ref.unwrap_or_default(),
            observed_owner: evidence
                .observed_owner
                .unwrap_or(HSAssetOwner::LifeModelYaml),
            output_digest: evidence.output_digest.unwrap_or_default(),
            selected_asset_count: evidence.selected_asset_count,
            succeeded: evidence.succeeded,
        }))
    }

    pub fn promote_to_accepted_hs(
        &self,
        category: HSAssetCategory,
        expected_revision: i64,
        parity: &ShadowParityReceipt,
        rollback: &RollbackRehearsalReceipt,
        scenario: &ProductScenarioReceipt,
    ) -> Result<HSAssetAuthorityRecord> {
        for (proof_category, proof_revision) in [
            (parity.category, parity.authority_revision),
            (rollback.category, rollback.authority_revision),
            (scenario.category, scenario.authority_revision),
        ] {
            if proof_category != category || proof_revision != expected_revision {
                return Err(anyhow::anyhow!(
                    "promotion evidence category or revision does not match requested CAS"
                ));
            }
        }

        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let authority = load_authority(&tx, category)?;
        require_revision(&authority, expected_revision)?;
        if authority.owner != HSAssetOwner::LifeModelYaml {
            return Err(anyhow::anyhow!(
                "category {category} is not owned by lifemodel_yaml at revision {expected_revision}"
            ));
        }
        let stored_parity = load_evidence(&tx, &parity.evidence_id)?;
        let stored_rollback = load_evidence(&tx, &rollback.evidence_id)?;
        let stored_scenario = load_evidence(&tx, &scenario.evidence_id)?;
        verify_promotion_evidence(
            category,
            expected_revision,
            &stored_parity,
            &stored_rollback,
            &stored_scenario,
        )?;
        let evidence_digest = digest_string(&format!(
            "{}:{}:{}",
            parity.evidence_id, rollback.evidence_id, scenario.evidence_id
        ));
        let changed = tx.execute(
            "UPDATE hs_asset_authority
             SET owner = ?3,
                 previous_owner = ?4,
                 revision = revision + 1,
                 last_evidence_digest = ?5,
                 updated_at = ?6
             WHERE category = ?1 AND revision = ?2 AND owner = ?4",
            params![
                category.to_string(),
                expected_revision,
                HSAssetOwner::AcceptedHsStore.to_string(),
                HSAssetOwner::LifeModelYaml.to_string(),
                evidence_digest,
                Utc::now().to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            return Err(anyhow::anyhow!(
                "HS asset authority promotion CAS conflict for {category} at revision {expected_revision}"
            ));
        }
        let promoted = load_authority(&tx, category)?;
        tx.commit()?;
        Ok(promoted)
    }

    pub fn invalidate_shadow_evidence(
        &self,
        category: HSAssetCategory,
        expected_revision: i64,
        reason_digest: &str,
    ) -> Result<HSAssetAuthorityRecord> {
        require_digest(reason_digest)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let authority = load_authority(&tx, category)?;
        require_revision(&authority, expected_revision)?;
        let changed = tx.execute(
            "UPDATE hs_asset_authority
             SET revision = revision + 1,
                 last_evidence_digest = ?3,
                 updated_at = ?4
             WHERE category = ?1 AND revision = ?2",
            params![
                category.to_string(),
                expected_revision,
                reason_digest,
                Utc::now().to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            return Err(anyhow::anyhow!(
                "HS asset authority invalidation CAS conflict"
            ));
        }
        let updated = load_authority(&tx, category)?;
        tx.commit()?;
        Ok(updated)
    }

    pub fn rollback_to_yaml(
        &self,
        category: HSAssetCategory,
        expected_revision: i64,
        rollback: &RollbackRehearsalReceipt,
        reason_digest: &str,
    ) -> Result<HSAssetAuthorityRecord> {
        require_digest(reason_digest)?;
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;
        let authority = load_authority(&tx, category)?;
        require_revision(&authority, expected_revision)?;
        if authority.owner != HSAssetOwner::AcceptedHsStore
            || authority.previous_owner != Some(HSAssetOwner::LifeModelYaml)
        {
            return Err(anyhow::anyhow!(
                "rollback is only available for a category promoted from lifemodel_yaml"
            ));
        }
        let rehearsal = load_evidence(&tx, &rollback.evidence_id)?;
        if rehearsal.category != category
            || rehearsal.evidence_kind != "rollback_rehearsal"
            || !rehearsal.succeeded
        {
            return Err(anyhow::anyhow!(
                "validated rollback rehearsal evidence is required"
            ));
        }
        let changed = tx.execute(
            "UPDATE hs_asset_authority
             SET owner = ?3,
                 previous_owner = ?4,
                 revision = revision + 1,
                 last_evidence_digest = ?5,
                 updated_at = ?6
             WHERE category = ?1 AND revision = ?2 AND owner = ?4",
            params![
                category.to_string(),
                expected_revision,
                HSAssetOwner::LifeModelYaml.to_string(),
                HSAssetOwner::AcceptedHsStore.to_string(),
                reason_digest,
                Utc::now().to_rfc3339(),
            ],
        )?;
        if changed != 1 {
            return Err(anyhow::anyhow!("HS asset rollback CAS conflict"));
        }
        let restored = load_authority(&tx, category)?;
        tx.commit()?;
        Ok(restored)
    }

    fn insert_evidence(&self, evidence: StoredEvidence) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO hs_asset_transition_evidence (
                 evidence_id, category, authority_revision, evidence_kind,
                 canonical_digest, compatibility_digest, repeated_digest,
                 scenario_ref, observed_owner, output_digest, selected_asset_count,
                 succeeded, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                evidence.evidence_id,
                evidence.category.to_string(),
                evidence.authority_revision,
                evidence.evidence_kind,
                evidence.canonical_digest,
                evidence.compatibility_digest,
                evidence.repeated_digest,
                evidence.scenario_ref,
                evidence.observed_owner.map(|owner| owner.to_string()),
                evidence.output_digest,
                evidence.selected_asset_count as i64,
                i64::from(evidence.succeeded),
                Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|error| anyhow::anyhow!("HS authority mutex poisoned: {error}"))
    }
}

pub fn build_collaboration_guidance_projection(
    model: &LifeModel,
    heuristic_store: &HeuristicStore,
) -> Result<CollaborationGuidanceProjection> {
    let mut records = heuristic_store.query(HeuristicQuery::default())?;
    records.retain(|record| {
        matches!(
            record.status,
            HeuristicLifecycleStatus::Active | HeuristicLifecycleStatus::Trial
        )
    });
    records.sort_by(|left, right| left.id.cmp(&right.id));
    if records.is_empty() {
        return Err(anyhow::anyhow!(
            "collaboration guidance cutover requires at least one runtime-readable canonical asset"
        ));
    }

    let canonical_digest = collaboration_guidance_digest_from_records(&records);
    let summaries = collaboration_guidance_summaries(&records);
    let yaml = model.materialize_yaml_compatibility_view(summaries.clone(), &[], &records)?;
    let view = extract_hs_compatibility_view_from_yaml(&yaml)?;
    let compatibility_digest = collaboration_guidance_digest_from_view(&view);
    let repeated_yaml = model.materialize_yaml_compatibility_view(summaries, &[], &records)?;
    let repeated_view = extract_hs_compatibility_view_from_yaml(&repeated_yaml)?;
    let repeated_materialization_digest = collaboration_guidance_digest_from_view(&repeated_view);
    let asset_ids = records.iter().map(|record| record.id.clone()).collect();

    Ok(CollaborationGuidanceProjection {
        yaml,
        canonical_digest,
        compatibility_digest,
        repeated_materialization_digest,
        asset_ids,
    })
}

/// Reconcile the category on startup. A fresh install records only LM-B
/// digest parity and rollback evidence. LM-C promotion occurs only after a
/// successful product-runtime receipt has already been durably recorded for
/// the same authority revision.
pub fn reconcile_collaboration_guidance_authority(
    registry: &HSAssetAuthorityRegistry,
    model: &LifeModel,
    heuristic_store: &HeuristicStore,
) -> Result<CollaborationGuidanceCutoverReport> {
    let projection = build_collaboration_guidance_projection(model, heuristic_store)?;
    validate_collaboration_guidance_projection(&projection)?;

    let authority = registry.authority(HSAssetCategory::CollaborationGuidance)?;
    if authority.owner == HSAssetOwner::AcceptedHsStore {
        registry.authorize_write(HSAssetWriteRequest {
            category: HSAssetCategory::CollaborationGuidance,
            source_owner: HSAssetOwner::AcceptedHsStore,
            target_owner: HSAssetOwner::LifeModelYaml,
            kind: HSAssetWriteKind::DerivedCompatibilityProjection,
        })?;
        return Ok(CollaborationGuidanceCutoverReport {
            status: CollaborationGuidanceCutoverStatus::AlreadyPromoted,
            authority,
            projection,
        });
    }

    let parity = registry.record_shadow_parity(
        HSAssetCategory::CollaborationGuidance,
        authority.revision,
        projection.canonical_digest.clone(),
        projection.compatibility_digest.clone(),
        projection.repeated_materialization_digest.clone(),
    )?;
    let rollback =
        registry.rehearse_rollback(HSAssetCategory::CollaborationGuidance, authority.revision)?;
    let Some(scenario) = registry.latest_successful_product_scenario(
        HSAssetCategory::CollaborationGuidance,
        authority.revision,
    )?
    else {
        return Ok(CollaborationGuidanceCutoverReport {
            status: CollaborationGuidanceCutoverStatus::ShadowEvidencePending,
            authority,
            projection,
        });
    };
    let promoted = registry.promote_to_accepted_hs(
        HSAssetCategory::CollaborationGuidance,
        authority.revision,
        &parity,
        &rollback,
        &scenario,
    )?;
    registry.authorize_write(HSAssetWriteRequest {
        category: HSAssetCategory::CollaborationGuidance,
        source_owner: HSAssetOwner::AcceptedHsStore,
        target_owner: HSAssetOwner::LifeModelYaml,
        kind: HSAssetWriteKind::DerivedCompatibilityProjection,
    })?;
    Ok(CollaborationGuidanceCutoverReport {
        status: CollaborationGuidanceCutoverStatus::Promoted,
        authority: promoted,
        projection,
    })
}

/// Explicit completion API used only after the caller has obtained a durable
/// product-runtime scenario receipt. This API never manufactures scenario
/// evidence from a selector fixture.
pub fn complete_collaboration_guidance_cutover(
    registry: &HSAssetAuthorityRegistry,
    model: &LifeModel,
    heuristic_store: &HeuristicStore,
    scenario: &ProductScenarioReceipt,
) -> Result<CollaborationGuidanceCutoverReport> {
    let projection = build_collaboration_guidance_projection(model, heuristic_store)?;
    validate_collaboration_guidance_projection(&projection)?;
    let authority = registry.authority(HSAssetCategory::CollaborationGuidance)?;
    if authority.owner == HSAssetOwner::AcceptedHsStore {
        return Ok(CollaborationGuidanceCutoverReport {
            status: CollaborationGuidanceCutoverStatus::AlreadyPromoted,
            authority,
            projection,
        });
    }
    let parity = registry.record_shadow_parity(
        HSAssetCategory::CollaborationGuidance,
        authority.revision,
        projection.canonical_digest.clone(),
        projection.compatibility_digest.clone(),
        projection.repeated_materialization_digest.clone(),
    )?;
    let rollback =
        registry.rehearse_rollback(HSAssetCategory::CollaborationGuidance, authority.revision)?;
    let promoted = registry.promote_to_accepted_hs(
        HSAssetCategory::CollaborationGuidance,
        authority.revision,
        &parity,
        &rollback,
        scenario,
    )?;
    Ok(CollaborationGuidanceCutoverReport {
        status: CollaborationGuidanceCutoverStatus::Promoted,
        authority: promoted,
        projection,
    })
}

fn validate_collaboration_guidance_projection(
    projection: &CollaborationGuidanceProjection,
) -> Result<()> {
    if projection.canonical_digest != projection.compatibility_digest
        || projection.canonical_digest != projection.repeated_materialization_digest
    {
        return Err(anyhow::anyhow!(
            "collaboration guidance materialization is not deterministic or digest-equivalent"
        ));
    }
    Ok(())
}

pub fn digest_string(value: &str) -> String {
    let hash = digest(&SHA256, value.as_bytes());
    let mut output = String::with_capacity(71);
    output.push_str("sha256:");
    for byte in hash.as_ref() {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn load_authority(
    conn: &impl AuthorityQuery,
    category: HSAssetCategory,
) -> Result<HSAssetAuthorityRecord> {
    conn.query_authority(category)
}

trait AuthorityQuery {
    fn query_authority(&self, category: HSAssetCategory) -> Result<HSAssetAuthorityRecord>;
}

impl AuthorityQuery for Connection {
    fn query_authority(&self, category: HSAssetCategory) -> Result<HSAssetAuthorityRecord> {
        query_authority_row(self, category)
    }
}

impl<'a> AuthorityQuery for Transaction<'a> {
    fn query_authority(&self, category: HSAssetCategory) -> Result<HSAssetAuthorityRecord> {
        query_authority_row(self, category)
    }
}

fn query_authority_row(
    conn: &Connection,
    category: HSAssetCategory,
) -> Result<HSAssetAuthorityRecord> {
    conn.query_row(
        "SELECT category, owner, revision, previous_owner, last_evidence_digest, updated_at
         FROM hs_asset_authority WHERE category = ?1",
        [category.to_string()],
        |row| {
            let category_raw: String = row.get(0)?;
            let owner_raw: String = row.get(1)?;
            let previous_owner_raw: Option<String> = row.get(3)?;
            Ok((
                category_raw,
                owner_raw,
                row.get::<_, i64>(2)?,
                previous_owner_raw,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    )
    .map_err(Into::into)
    .and_then(
        |(category_raw, owner_raw, revision, previous_owner_raw, digest, updated_at)| {
            Ok(HSAssetAuthorityRecord {
                category: category_raw.parse()?,
                owner: owner_raw.parse()?,
                revision,
                previous_owner: previous_owner_raw.map(|owner| owner.parse()).transpose()?,
                last_evidence_digest: digest,
                updated_at,
            })
        },
    )
}

fn load_owner_tx(tx: &Transaction<'_>, category: HSAssetCategory) -> Result<HSAssetOwner> {
    let owner: String = tx.query_row(
        "SELECT owner FROM hs_asset_authority WHERE category = ?1",
        [category.to_string()],
        |row| row.get(0),
    )?;
    owner.parse()
}

fn load_evidence(conn: &Connection, evidence_id: &str) -> Result<StoredEvidence> {
    conn.query_row(
        "SELECT evidence_id, category, authority_revision, evidence_kind,
                canonical_digest, compatibility_digest, repeated_digest,
                scenario_ref, observed_owner, output_digest, selected_asset_count, succeeded
         FROM hs_asset_transition_evidence WHERE evidence_id = ?1",
        [evidence_id],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
            ))
        },
    )
    .map_err(Into::into)
    .and_then(
        |(
            evidence_id,
            category,
            authority_revision,
            evidence_kind,
            canonical_digest,
            compatibility_digest,
            repeated_digest,
            scenario_ref,
            observed_owner,
            output_digest,
            selected_asset_count,
            succeeded,
        )| {
            Ok(StoredEvidence {
                evidence_id,
                category: category.parse()?,
                authority_revision,
                evidence_kind,
                canonical_digest,
                compatibility_digest,
                repeated_digest,
                scenario_ref,
                observed_owner: observed_owner.map(|owner| owner.parse()).transpose()?,
                output_digest,
                selected_asset_count: selected_asset_count.max(0) as usize,
                succeeded: succeeded != 0,
            })
        },
    )
}

fn verify_promotion_evidence(
    category: HSAssetCategory,
    revision: i64,
    parity: &StoredEvidence,
    rollback: &StoredEvidence,
    scenario: &StoredEvidence,
) -> Result<()> {
    for evidence in [parity, rollback, scenario] {
        if evidence.category != category || evidence.authority_revision != revision {
            return Err(anyhow::anyhow!(
                "promotion evidence is stale or belongs to another category"
            ));
        }
    }
    if parity.evidence_kind != "shadow_parity"
        || !parity.succeeded
        || parity.canonical_digest != parity.compatibility_digest
        || parity.canonical_digest != parity.repeated_digest
    {
        return Err(anyhow::anyhow!(
            "deterministic shadow digest parity is required before promotion"
        ));
    }
    if rollback.evidence_kind != "rollback_rehearsal" || !rollback.succeeded {
        return Err(anyhow::anyhow!(
            "successful rollback rehearsal is required before promotion"
        ));
    }
    if scenario.evidence_kind != "product_scenario"
        || !scenario.succeeded
        || scenario.observed_owner != Some(HSAssetOwner::AcceptedHsStore)
        || scenario.selected_asset_count == 0
        || scenario
            .scenario_ref
            .as_ref()
            .is_none_or(|reference| reference.trim().is_empty())
        || scenario
            .output_digest
            .as_ref()
            .is_none_or(|digest| require_digest(digest).is_err())
    {
        return Err(anyhow::anyhow!(
            "successful product scenario evidence is required before promotion"
        ));
    }
    Ok(())
}

fn require_revision(record: &HSAssetAuthorityRecord, expected_revision: i64) -> Result<()> {
    if record.revision != expected_revision {
        return Err(anyhow::anyhow!(
            "stale HS asset authority revision: expected {expected_revision}, current {}",
            record.revision
        ));
    }
    Ok(())
}

fn require_digest(value: &str) -> Result<()> {
    let valid = value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit());
    if !valid {
        return Err(anyhow::anyhow!("expected a sha256 digest reference"));
    }
    Ok(())
}

fn evidence_id(kind: &str) -> String {
    format!("hs_{kind}_{}", Uuid::new_v4().simple())
}
