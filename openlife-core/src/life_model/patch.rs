use crate::agent::types::RiskLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

pub const LIFEMODEL_PATCH_BATCH_SCHEMA_V1: &str = "lifemodel_patch_batch_v1";
pub const LIFEMODEL_PATCH_BATCH_PATH: &str = "$lifemodel_batch";
/// Public contract bounds used by every producer before it acquires a durable
/// review claim. Keeping these values private previously let Builder admit a
/// much larger request and discover the real limit only after mutating its
/// canonical session.
pub const MAX_LIFEMODEL_PATCH_BATCH_OPERATIONS: usize = 64;
pub const MAX_LIFEMODEL_PATCH_BATCH_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelPatchBatchOperationV1 {
    pub candidate_id: String,
    pub path: String,
    pub candidate: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelPatchBatchV1 {
    pub schema_version: String,
    pub operations: Vec<LifeModelPatchBatchOperationV1>,
}

impl LifeModelPatchBatchV1 {
    pub fn new(operations: Vec<LifeModelPatchBatchOperationV1>) -> Result<Self, String> {
        let batch = Self {
            schema_version: LIFEMODEL_PATCH_BATCH_SCHEMA_V1.to_string(),
            operations,
        };
        batch.validate()?;
        Ok(batch)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != LIFEMODEL_PATCH_BATCH_SCHEMA_V1 {
            return Err("unsupported_lifemodel_patch_batch_schema".into());
        }
        if self.operations.is_empty()
            || self.operations.len() > MAX_LIFEMODEL_PATCH_BATCH_OPERATIONS
        {
            return Err("lifemodel_patch_batch_operation_count_out_of_bounds".into());
        }
        if serde_json::to_vec(self)
            .map_err(|_| "lifemodel_patch_batch_serialization_failed")?
            .len()
            > MAX_LIFEMODEL_PATCH_BATCH_BYTES
        {
            return Err("lifemodel_patch_batch_payload_too_large".into());
        }

        let mut paths = BTreeSet::new();
        let mut candidate_ids = BTreeSet::new();
        for operation in &self.operations {
            let candidate_id = operation.candidate_id.trim();
            if candidate_id != operation.candidate_id
                || candidate_id.is_empty()
                || candidate_id.len() > 160
            {
                return Err("invalid_lifemodel_patch_batch_candidate_id".into());
            }
            if !candidate_ids.insert(candidate_id) {
                return Err("duplicate_lifemodel_patch_batch_candidate_id".into());
            }
            let path = operation.path.trim();
            if path != operation.path
                || path.is_empty()
                || path.len() > 160
                || path.starts_with('.')
                || path.ends_with('.')
                || path.split('.').any(|segment| {
                    segment.is_empty()
                        || !segment
                            .chars()
                            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                })
            {
                return Err("invalid_lifemodel_patch_batch_path".into());
            }
            if operation.candidate.is_null() {
                return Err("lifemodel_patch_batch_candidate_must_not_be_null".into());
            }
            if !paths.insert(path) {
                return Err("duplicate_lifemodel_patch_batch_path".into());
            }
        }
        Ok(())
    }
}

/// A single patch operation on a LifeModel path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchOp {
    /// Replace the value at the path entirely
    Replace,
    /// Shallow merge for objects (non-destructive update)
    Merge,
    /// Append to an array (adds to the end)
    Append,
    /// Insert at a specific array index
    Insert,
    /// Delete the element at the path
    Delete,
}

impl std::fmt::Display for PatchOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchOp::Replace => write!(f, "replace"),
            PatchOp::Merge => write!(f, "merge"),
            PatchOp::Append => write!(f, "append"),
            PatchOp::Insert => write!(f, "insert"),
            PatchOp::Delete => write!(f, "delete"),
        }
    }
}

/// Status of a patch in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchStatus {
    Pending,
    Applied,
    Rejected,
    Superseded,
}

impl std::fmt::Display for PatchStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchStatus::Pending => write!(f, "pending"),
            PatchStatus::Applied => write!(f, "applied"),
            PatchStatus::Rejected => write!(f, "rejected"),
            PatchStatus::Superseded => write!(f, "superseded"),
        }
    }
}

/// Source of the patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchSource {
    BuilderReview,
    Calibration,
    Feedback,
    Manual,
    Evolution,
    ChatConversation,
    ProactiveAgent,
    SkillRuntime,
    Plugin,
    MemoryGovernance,
    PlanningSession,
}

impl std::fmt::Display for PatchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchSource::BuilderReview => write!(f, "builder_review"),
            PatchSource::Calibration => write!(f, "calibration"),
            PatchSource::Feedback => write!(f, "feedback"),
            PatchSource::Manual => write!(f, "manual"),
            PatchSource::Evolution => write!(f, "evolution"),
            PatchSource::ChatConversation => write!(f, "chat_conversation"),
            PatchSource::ProactiveAgent => write!(f, "proactive_agent"),
            PatchSource::SkillRuntime => write!(f, "skill_runtime"),
            PatchSource::Plugin => write!(f, "plugin"),
            PatchSource::MemoryGovernance => write!(f, "memory_governance"),
            PatchSource::PlanningSession => write!(f, "planning_session"),
        }
    }
}

/// A single patch to a LifeModel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelPatch {
    pub id: String,
    pub proposal_id: Option<String>,
    pub path_pointer: String,
    pub path_display: String,
    pub operation: PatchOp,
    pub before: Option<serde_json::Value>,
    pub after: serde_json::Value,
    pub source: PatchSource,
    pub reason: String,
    pub confidence: f32,
    pub risk_level: RiskLevel,
    pub status: PatchStatus,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

impl LifeModelPatch {
    pub fn new(
        path_pointer: &str,
        path_display: &str,
        operation: PatchOp,
        after: serde_json::Value,
        reason: &str,
        risk_level: RiskLevel,
        source: PatchSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            proposal_id: None,
            path_pointer: path_pointer.to_string(),
            path_display: path_display.to_string(),
            operation,
            before: None,
            after,
            source,
            reason: reason.to_string(),
            confidence: 0.8,
            risk_level,
            status: PatchStatus::Pending,
            created_at: Utc::now(),
            applied_at: None,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    pub fn from_proposal(
        proposal_id: &str,
        path_pointer: &str,
        path_display: &str,
        operation: PatchOp,
        before: Option<serde_json::Value>,
        after: serde_json::Value,
        reason: &str,
        confidence: f32,
        risk_level: RiskLevel,
        source: PatchSource,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            proposal_id: Some(proposal_id.to_string()),
            path_pointer: path_pointer.to_string(),
            path_display: path_display.to_string(),
            operation,
            before,
            after,
            source,
            reason: reason.to_string(),
            confidence,
            risk_level,
            status: PatchStatus::Pending,
            created_at: Utc::now(),
            applied_at: None,
        }
    }

    pub fn mark_applied(&mut self) {
        self.status = PatchStatus::Applied;
        self.applied_at = Some(Utc::now());
    }

    pub fn mark_rejected(&mut self) {
        self.status = PatchStatus::Rejected;
    }

    pub fn mark_superseded(&mut self) {
        self.status = PatchStatus::Superseded;
    }
}

/// Result of applying a single patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchApplyResult {
    pub patch_id: String,
    pub success: bool,
    pub path: String,
    pub operation: String,
    pub error: Option<String>,
}

/// Error when applying a patch.
#[derive(Debug, Clone)]
pub enum PatchError {
    InvalidPath(String),
    BeforeMismatch {
        expected: serde_json::Value,
        actual: serde_json::Value,
    },
    InvalidOperation {
        op: PatchOp,
        reason: String,
    },
    Serialization(String),
    Validation(String),
    IndexOutOfBounds {
        index: usize,
        len: usize,
    },
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::InvalidPath(p) => write!(f, "Invalid path: {}", p),
            PatchError::BeforeMismatch { expected, actual } => {
                write!(
                    f,
                    "Before mismatch: expected {:?}, got {:?}",
                    expected, actual
                )
            }
            PatchError::InvalidOperation { op, reason } => {
                write!(f, "Invalid operation {:?}: {}", op, reason)
            }
            PatchError::Serialization(e) => write!(f, "Serialization error: {}", e),
            PatchError::Validation(e) => write!(f, "Validation error: {}", e),
            PatchError::IndexOutOfBounds { index, len } => {
                write!(f, "Index {} out of bounds (len: {})", index, len)
            }
        }
    }
}

impl std::error::Error for PatchError {}

/// Types of conflicts between patches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// Two patches target the exact same path
    SamePath,
    /// One patch targets a parent, the other a child
    ParentChild,
    /// Two patches affect adjacent array indices (order-sensitive)
    ArrayIndex,
}

/// A detected conflict between two patches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchConflict {
    pub patch_id_1: String,
    pub patch_id_2: String,
    pub conflict_type: ConflictType,
    pub resolution: Option<ConflictResolution>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Resolution strategy for a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Keep the first patch, reject the second
    KeepFirst,
    /// Keep the second patch, reject the first
    KeepSecond,
    /// Keep both (if semantically valid)
    KeepBoth,
    /// Manual resolution required
    Manual,
}

/// Policy for batch patch application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureMode {
    /// All or nothing: rollback on any failure
    Atomic,
    /// Skip failures, keep successes
    Partial,
    /// Critical failures trigger rollback, others continue
    Adaptive,
}

/// Configuration for batch patch application.
#[derive(Debug, Clone)]
pub struct PatchBatchPolicy {
    pub failure_mode: FailureMode,
    pub detect_dependencies: bool,
    pub auto_resolve_low_risk: bool,
}

impl Default for PatchBatchPolicy {
    fn default() -> Self {
        Self {
            failure_mode: FailureMode::Adaptive,
            detect_dependencies: true,
            auto_resolve_low_risk: true,
        }
    }
}

/// Result of applying a batch of patches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchApplyResult {
    pub applied: Vec<PatchApplyResult>,
    pub skipped: Vec<String>,
    pub pending_review: Vec<PatchConflict>,
    pub rolled_back: bool,
    pub error: Option<String>,
}

/// Convert a dot-separated path (used in proposals) to JSON Pointer.
/// Example: "identity.values.0.weight" → "/identity/values/0/weight"
pub fn dot_to_pointer(dot_path: &str) -> String {
    let parts: Vec<&str> = dot_path.split('.').collect();
    format!("/{}", parts.join("/"))
}

/// Convert a JSON Pointer to a display-friendly string.
/// Example: "/identity/values/0/weight" → "Identity > Values > [0] > Weight"
pub fn pointer_to_display(pointer: &str, model: &crate::life_model::LifeModel) -> String {
    let parts: Vec<String> = pointer
        .split('/')
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(idx, s)| {
            // Check if it's an array index
            if let Ok(index) = s.parse::<usize>() {
                // Try to get a meaningful name for array items
                let parent_path: Vec<&str> = pointer.split('/').filter(|p| !p.is_empty()).collect();
                if idx > 0 && idx <= parent_path.len() {
                    let parent = parent_path[idx - 1];
                    return get_array_item_name(parent, index, model);
                }
                format!("[{}]", s)
            } else {
                // Map known field names to human-readable labels
                let label = get_field_label(s);
                if idx == 0 {
                    // Top-level section
                    label.to_string()
                } else {
                    label
                }
            }
        })
        .collect();

    if parts.is_empty() {
        "Root".to_string()
    } else {
        parts.join(" > ")
    }
}

/// Map field names to human-readable labels.
fn get_field_label(field: &str) -> String {
    let labels = [
        ("identity", "Identity (身份)"),
        ("goals", "Goals (目标)"),
        ("capabilities", "Capabilities (能力)"),
        ("state", "State (状态)"),
        ("values", "Values (价值观)"),
        ("personality_traits", "Personality Traits (个性特征)"),
        ("life_philosophy", "Life Philosophy (人生哲学)"),
        ("mission_statement", "Mission Statement (使命宣言)"),
        ("role_definition", "Role Definition (角色定义)"),
        ("short_term", "Short-term Goals (短期目标)"),
        ("medium_term", "Medium-term Goals (中期目标)"),
        ("long_term", "Long-term Goals (长期目标)"),
        ("daily", "Daily Goals (每日目标)"),
        ("current_focus", "Current Focus (当前焦点)"),
        ("health_status", "Health Status (健康状况)"),
        ("emotional_state", "Emotional State (情绪状态)"),
        ("habit_streaks", "Habit Streaks (习惯 streak)"),
        ("custom_dimensions", "Custom Dimensions (自定义维度)"),
        ("alerts", "Alerts (警报)"),
        ("name", "Name (名称)"),
        ("description", "Description (描述)"),
        ("priority", "Priority (优先级)"),
        ("progress", "Progress (进度)"),
        ("deadline", "Deadline (截止日期)"),
        ("milestones", "Milestones (里程碑)"),
        ("skills", "Skills (技能)"),
        ("resources", "Resources (资源)"),
        ("networks", "Networks (网络)"),
        ("tools", "Tools (工具)"),
        ("knowledge_domains", "Knowledge Domains (知识领域)"),
        ("proficiency", "Proficiency (熟练度)"),
        ("source", "Source (来源)"),
        ("content", "Content (内容)"),
        ("tags", "Tags (标签)"),
    ];

    for (key, label) in &labels {
        if *key == field {
            return label.to_string();
        }
    }

    // Fallback: capitalize first letter
    let mut chars = field.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => field.to_string(),
    }
}

/// Get a meaningful name for array items based on parent field.
fn get_array_item_name(parent: &str, index: usize, model: &crate::life_model::LifeModel) -> String {
    match parent {
        "values" => {
            if let Some(value) = model.identity.values.get(index) {
                return format!("[{}] {}", index, value.name);
            }
        }
        "personality_traits" => {
            if let Some(trait_item) = model.identity.personality_traits.get(index) {
                return format!("[{}] {}", index, trait_item.trait_name);
            }
        }
        "short_term" => {
            if let Some(goal) = model.goals.short_term.get(index) {
                return format!("[{}] {}", index, goal.name);
            }
        }
        "medium_term" => {
            if let Some(goal) = model.goals.medium_term.get(index) {
                return format!("[{}] {}", index, goal.name);
            }
        }
        "long_term" => {
            if let Some(goal) = model.goals.long_term.get(index) {
                return format!("[{}] {}", index, goal.name);
            }
        }
        "daily" => {
            if let Some(goal) = model.goals.daily.get(index) {
                return format!("[{}] {}", index, goal.name);
            }
        }
        "skills" => {
            if let Some(skill) = model.capabilities.skills.get(index) {
                return format!("[{}] {}", index, skill.name);
            }
        }
        "resources" => {
            if let Some(resource) = model.capabilities.resources.get(index) {
                return format!("[{}] {}", index, resource.name);
            }
        }
        "habit_streaks" => {
            if let Some(streak) = model.state.habit_streaks.get(index) {
                return format!("[{}] {}", index, streak.name);
            }
        }
        "custom_dimensions" => {
            if let Some(dim) = model.state.custom_dimensions.get(index) {
                return format!("[{}] {}", index, dim.name);
            }
        }
        _ => {}
    }
    format!("[{}]", index)
}

/// Detect conflicts between patches.
/// Returns a list of conflicts that need resolution.
pub fn detect_conflicts(patches: &[LifeModelPatch]) -> Vec<PatchConflict> {
    let mut conflicts = Vec::new();

    for i in 0..patches.len() {
        for j in (i + 1)..patches.len() {
            let p1 = &patches[i];
            let p2 = &patches[j];

            // Check for same path conflict
            if p1.path_pointer == p2.path_pointer {
                // Append + Append on same array is not a conflict
                if p1.operation == PatchOp::Append && p2.operation == PatchOp::Append {
                    continue;
                }
                conflicts.push(PatchConflict {
                    patch_id_1: p1.id.clone(),
                    patch_id_2: p2.id.clone(),
                    conflict_type: ConflictType::SamePath,
                    resolution: None,
                    resolved_at: None,
                });
                continue;
            }

            // Check for parent-child conflict
            if p1
                .path_pointer
                .starts_with(&format!("{}/", p2.path_pointer))
                || p2
                    .path_pointer
                    .starts_with(&format!("{}/", p1.path_pointer))
            {
                conflicts.push(PatchConflict {
                    patch_id_1: p1.id.clone(),
                    patch_id_2: p2.id.clone(),
                    conflict_type: ConflictType::ParentChild,
                    resolution: None,
                    resolved_at: None,
                });
                continue;
            }

            // Check for array index conflict
            if let Some(conflict_type) =
                detect_array_index_conflict(&p1.path_pointer, &p2.path_pointer)
            {
                conflicts.push(PatchConflict {
                    patch_id_1: p1.id.clone(),
                    patch_id_2: p2.id.clone(),
                    conflict_type,
                    resolution: None,
                    resolved_at: None,
                });
            }
        }
    }

    conflicts
}

/// Detect if two paths affect adjacent array indices.
fn detect_array_index_conflict(path1: &str, path2: &str) -> Option<ConflictType> {
    let parts1: Vec<&str> = path1.split('/').filter(|s| !s.is_empty()).collect();
    let parts2: Vec<&str> = path2.split('/').filter(|s| !s.is_empty()).collect();

    if parts1.len() != parts2.len() {
        return None;
    }

    let mut index_diff: Option<(usize, usize)> = None;
    let mut diff_count = 0;

    for (p1, p2) in parts1.iter().zip(parts2.iter()) {
        if p1 != p2 {
            if let (Ok(i1), Ok(i2)) = (p1.parse::<usize>(), p2.parse::<usize>()) {
                if i1.abs_diff(i2) == 1 {
                    index_diff = Some((i1, i2));
                    diff_count += 1;
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    }

    if diff_count == 1 && index_diff.is_some() {
        Some(ConflictType::ArrayIndex)
    } else {
        None
    }
}

/// Resolve conflicts automatically based on risk level.
/// Returns (accepted_patch_ids, rejected_patch_ids, manual_review_conflicts)
pub fn auto_resolve_conflicts(
    patches: &[LifeModelPatch],
    conflicts: &[PatchConflict],
) -> (Vec<String>, Vec<String>, Vec<PatchConflict>) {
    let mut accepted: Vec<String> = patches.iter().map(|p| p.id.clone()).collect();
    let mut rejected: Vec<String> = Vec::new();
    let mut manual: Vec<PatchConflict> = Vec::new();

    for conflict in conflicts {
        let p1 = patches.iter().find(|p| p.id == conflict.patch_id_1);
        let p2 = patches.iter().find(|p| p.id == conflict.patch_id_2);

        if let (Some(patch1), Some(patch2)) = (p1, p2) {
            // Low risk: auto-resolve by timestamp (keep latest)
            if patch1.risk_level == RiskLevel::Low && patch2.risk_level == RiskLevel::Low {
                let keep = if patch1.created_at >= patch2.created_at {
                    patch1.id.clone()
                } else {
                    patch2.id.clone()
                };
                let reject = if keep == patch1.id {
                    patch2.id.clone()
                } else {
                    patch1.id.clone()
                };

                accepted.retain(|id| id != &reject);
                if !rejected.contains(&reject) {
                    rejected.push(reject);
                }
            } else {
                // Medium/High/Critical: manual review
                manual.push(conflict.clone());
            }
        }
    }

    (accepted, rejected, manual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_candidate_batch_is_minimal_and_rejects_canonical_before_after_copies() {
        let batch = LifeModelPatchBatchV1::new(vec![LifeModelPatchBatchOperationV1 {
            candidate_id: "sig_goal".into(),
            path: "goals.short_term".into(),
            candidate: serde_json::json!([{
                "name": "new candidate",
                "priority": 0,
                "status": "pending",
                "milestones": [],
                "description": "",
                "progress": 0.0
            }]),
        }])
        .unwrap();
        let value = serde_json::to_value(&batch).unwrap();
        let operation = &value["operations"][0];
        assert!(operation.get("before").is_none());
        assert!(operation.get("after").is_none());
        assert_eq!(operation["candidateId"], "sig_goal");

        let legacy_copy = serde_json::json!({
            "schemaVersion": LIFEMODEL_PATCH_BATCH_SCHEMA_V1,
            "operations": [{
                "candidateId": "sig_goal",
                "path": "goals.short_term",
                "candidate": [],
                "before": ["PRIVATE_EXISTING_CANONICAL_VALUE"],
                "after": []
            }]
        });
        assert!(serde_json::from_value::<LifeModelPatchBatchV1>(legacy_copy).is_err());
    }

    #[test]
    fn test_dot_to_pointer() {
        assert_eq!(
            dot_to_pointer("identity.values.0.weight"),
            "/identity/values/0/weight"
        );
        assert_eq!(dot_to_pointer("goals.short_term"), "/goals/short_term");
    }

    #[test]
    fn test_pointer_to_display() {
        use crate::life_model::LifeModel;
        let model = LifeModel::default();
        assert_eq!(
            pointer_to_display("/identity/values/0/weight", &model),
            "Identity (身份) > Values (价值观) > [0] > Weight"
        );
    }

    #[test]
    fn test_detect_same_path_conflict() {
        let p1 = LifeModelPatch::new(
            "/identity/values/0/weight",
            "Identity > Values > [0] > Weight",
            PatchOp::Replace,
            serde_json::json!(80),
            "Test",
            RiskLevel::Medium,
            PatchSource::Manual,
        );
        let p2 = LifeModelPatch::new(
            "/identity/values/0/weight",
            "Identity > Values > [0] > Weight",
            PatchOp::Replace,
            serde_json::json!(90),
            "Test 2",
            RiskLevel::Medium,
            PatchSource::Manual,
        );

        let conflicts = detect_conflicts(&[p1, p2]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::SamePath);
    }

    #[test]
    fn test_no_conflict_for_append_append() {
        let p1 = LifeModelPatch::new(
            "/goals/short_term",
            "Goals > Short Term",
            PatchOp::Append,
            serde_json::json!({"name": "Goal 1"}),
            "Test",
            RiskLevel::Low,
            PatchSource::Manual,
        );
        let p2 = LifeModelPatch::new(
            "/goals/short_term",
            "Goals > Short Term",
            PatchOp::Append,
            serde_json::json!({"name": "Goal 2"}),
            "Test 2",
            RiskLevel::Low,
            PatchSource::Manual,
        );

        let conflicts = detect_conflicts(&[p1, p2]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_auto_resolve_low_risk() {
        let p1 = LifeModelPatch::new(
            "/identity/values/0/weight",
            "Test",
            PatchOp::Replace,
            serde_json::json!(80),
            "Test",
            RiskLevel::Low,
            PatchSource::Manual,
        );
        let p2 = LifeModelPatch::new(
            "/identity/values/0/weight",
            "Test",
            PatchOp::Replace,
            serde_json::json!(90),
            "Test 2",
            RiskLevel::Low,
            PatchSource::Manual,
        );

        let conflicts = detect_conflicts(&[p1.clone(), p2.clone()]);
        let (accepted, rejected, manual) = auto_resolve_conflicts(&[p1, p2], &conflicts);

        assert_eq!(accepted.len(), 1);
        assert_eq!(rejected.len(), 1);
        assert!(manual.is_empty());
    }
}
