use crate::agent::evidence_store::{
    EvidenceRecord, EvidenceSourceType, EvidenceStatus, EvidenceType,
};
use chrono::{DateTime, Duration, Utc};
use ring::digest::{digest, SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const REPORT_KIND: &str = "evidence_graph_v1";
const TIMELINE_KIND: &str = "evidence_timeline_v1";
const CLUSTER_PREFIX: &str = "egc_";
const DECAY_GRACE_DAYS: i64 = 30;
const DECAY_HALF_LIFE_DAYS: f32 = 90.0;
const MIN_DECAY_FACTOR: f32 = 0.05;
const REJECTED_SIMILAR_COOLDOWN_DAYS: i64 = 14;

#[derive(Debug, Clone)]
pub struct EvidenceGraphInput {
    pub evidence_records: Vec<EvidenceRecord>,
    pub now: DateTime<Utc>,
}

impl EvidenceGraphInput {
    pub fn new(evidence_records: Vec<EvidenceRecord>, now: DateTime<Utc>) -> Self {
        Self {
            evidence_records,
            now,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceGraphLinkKind {
    Support,
    Opposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolarity {
    Supporting,
    Opposing,
    Corrective,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceGraphLink {
    pub kind: EvidenceGraphLinkKind,
    pub from_evidence_id: String,
    pub to_evidence_id: String,
    pub from_cluster_id: String,
    pub to_cluster_id: String,
    pub reason: String,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSourceWeightSummary {
    pub source_type: String,
    pub ref_count: usize,
    pub weight_total: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceConflictState {
    pub conflicted: bool,
    pub reasons: Vec<String>,
    pub opposing_evidence_ids: Vec<String>,
    pub contradicted: bool,
    pub rejected_opposition: bool,
    pub same_affected_path_cluster_opposition: bool,
}

impl EvidenceConflictState {
    fn clean() -> Self {
        Self {
            conflicted: false,
            reasons: Vec::new(),
            opposing_evidence_ids: Vec::new(),
            contradicted: false,
            rejected_opposition: false,
            same_affected_path_cluster_opposition: false,
        }
    }

    fn add_reason(&mut self, reason: &str) {
        push_unique(&mut self.reasons, reason.to_string());
        self.conflicted = true;
    }

    fn add_opposing_ids(&mut self, ids: impl IntoIterator<Item = String>) {
        for id in ids {
            if !id.trim().is_empty() {
                push_unique(&mut self.opposing_evidence_ids, id);
            }
        }
        if !self.opposing_evidence_ids.is_empty() {
            self.conflicted = true;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceDecayState {
    pub generated_at: DateTime<Utc>,
    pub last_observed_at: DateTime<Utc>,
    pub age_days: i64,
    pub grace_days: i64,
    pub half_life_days: f32,
    pub decay_factor: f32,
    pub effective_confidence: f32,
    pub decayed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCooldownState {
    pub active: bool,
    pub reason: Option<String>,
    pub similar_cluster_id: Option<String>,
    pub cooldown_days: i64,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub days_remaining: i64,
    pub rejected_evidence_ids: Vec<String>,
    pub rejected_proposal_ids: Vec<String>,
}

impl EvidenceCooldownState {
    fn inactive() -> Self {
        Self {
            active: false,
            reason: None,
            similar_cluster_id: None,
            cooldown_days: REJECTED_SIMILAR_COOLDOWN_DAYS,
            cooldown_until: None,
            days_remaining: 0,
            rejected_evidence_ids: Vec::new(),
            rejected_proposal_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceClusterSummary {
    pub cluster_id: String,
    pub cluster_hash: String,
    pub affected_path: String,
    pub evidence_types: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub supporting_evidence_ids: Vec<String>,
    pub opposing_evidence_ids: Vec<String>,
    pub dedupe_key_hashes: Vec<String>,
    pub record_count: usize,
    pub source_weights: Vec<EvidenceSourceWeightSummary>,
    pub source_weight_total: f32,
    pub average_confidence: f32,
    pub effective_confidence: f32,
    pub support_link_count: usize,
    pub opposition_link_count: usize,
    pub summary: String,
    pub conflict_state: EvidenceConflictState,
    pub cooldown_state: EvidenceCooldownState,
    pub newest_observed_at: DateTime<Utc>,
    pub oldest_observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceTimelineItem {
    pub evidence_id: String,
    pub evidence_type: String,
    pub affected_path: String,
    pub status: String,
    pub confidence: f32,
    pub risk_level: String,
    pub privacy_level: String,
    pub polarity: EvidencePolarity,
    pub source_ref_count: usize,
    pub support_link_count: usize,
    pub opposition_link_count: usize,
    pub linked_proposal_ids: Vec<String>,
    pub linked_agent_run_ids: Vec<String>,
    pub cluster_id: String,
    pub cluster_hash: String,
    pub conflict_state: EvidenceConflictState,
    pub decay_state: EvidenceDecayState,
    pub cooldown_state: EvidenceCooldownState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceTimelineReadModel {
    pub report_kind: String,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub item_count: usize,
    pub items: Vec<EvidenceTimelineItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceGraphReport {
    pub report_kind: String,
    pub graph_ready: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub generated_at: DateTime<Utc>,
    pub record_count: usize,
    pub cluster_count: usize,
    pub support_link_count: usize,
    pub opposition_link_count: usize,
    pub conflict_count: usize,
    pub decayed_count: usize,
    pub cooldown_count: usize,
    pub ran_runtime: bool,
    pub ran_model: bool,
    pub ran_tool: bool,
    pub wrote_life_model_count: u32,
    pub wrote_memory_count: u32,
    pub wrote_heuristic_count: u32,
    pub wrote_chat_message_count: u32,
    pub wrote_agent_run_count: u32,
    pub wrote_mcp_audit_count: u32,
    pub wrote_external_count: u32,
    pub clusters: Vec<EvidenceClusterSummary>,
    pub links: Vec<EvidenceGraphLink>,
    pub timeline: EvidenceTimelineReadModel,
}

#[derive(Debug, Clone)]
struct RecordGraphState {
    cluster_id: String,
    cluster_hash: String,
    polarity: EvidencePolarity,
    source_weight_total: f32,
    conflict_state: EvidenceConflictState,
    decay_state: EvidenceDecayState,
}

struct LinkDraft {
    kind: EvidenceGraphLinkKind,
    from_evidence_id: String,
    to_evidence_id: String,
    from_cluster_id: String,
    to_cluster_id: String,
    reason: &'static str,
    weight: f32,
}

pub fn build_evidence_timeline(input: EvidenceGraphInput) -> EvidenceTimelineReadModel {
    evaluate_evidence_graph(input).timeline
}

pub fn evaluate_evidence_graph(input: EvidenceGraphInput) -> EvidenceGraphReport {
    let now = input.now;
    let mut records = input.evidence_records;
    sort_records(&mut records);

    let mut states = Vec::with_capacity(records.len());
    let mut cluster_indices: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, record) in records.iter().enumerate() {
        let cluster_hash = cluster_hash_for_path(&record.affected_path);
        let cluster_id = format!("{CLUSTER_PREFIX}{}", short_hash(&cluster_hash));
        cluster_indices
            .entry(record.affected_path.clone())
            .or_default()
            .push(idx);

        let mut conflict_state = EvidenceConflictState::clean();
        if !record.opposing_refs.is_empty() {
            conflict_state.add_reason("opposing_refs_present");
            conflict_state.add_opposing_ids(record.opposing_refs.clone());
        }
        if record.status == EvidenceStatus::Contradicted {
            conflict_state.contradicted = true;
            conflict_state.add_reason("evidence_status_contradicted");
        }
        if is_rejected_opposition(record) {
            conflict_state.rejected_opposition = true;
            conflict_state.add_reason("rejected_proposal_outcome");
            conflict_state.add_opposing_ids(record.opposing_refs.clone());
        }

        states.push(RecordGraphState {
            cluster_id,
            cluster_hash,
            polarity: infer_polarity(record),
            source_weight_total: record_source_weight_total(record),
            conflict_state,
            decay_state: decay_state(record, now),
        });
    }

    let mut link_keys = BTreeSet::new();
    let mut links = Vec::new();
    for (idx, record) in records.iter().enumerate() {
        for target_id in metadata_source_evidence_ids(&record.run_metadata) {
            let kind = if is_rejected_opposition(record) {
                EvidenceGraphLinkKind::Opposition
            } else {
                EvidenceGraphLinkKind::Support
            };
            let target_cluster_id =
                cluster_id_for_evidence_id(&records, &states, target_id.as_str())
                    .unwrap_or_else(|| states[idx].cluster_id.clone());
            push_link(
                &mut links,
                &mut link_keys,
                LinkDraft {
                    kind,
                    from_evidence_id: record.id.clone(),
                    to_evidence_id: target_id,
                    from_cluster_id: states[idx].cluster_id.clone(),
                    to_cluster_id: target_cluster_id,
                    reason: if kind == EvidenceGraphLinkKind::Support {
                        "source_evidence_support"
                    } else {
                        "source_evidence_opposition"
                    },
                    weight: link_weight(record, &states[idx]),
                },
            );
        }
        for target_id in &record.opposing_refs {
            push_link(
                &mut links,
                &mut link_keys,
                LinkDraft {
                    kind: EvidenceGraphLinkKind::Opposition,
                    from_evidence_id: record.id.clone(),
                    to_evidence_id: target_id.clone(),
                    from_cluster_id: states[idx].cluster_id.clone(),
                    to_cluster_id: cluster_id_for_evidence_id(&records, &states, target_id)
                        .unwrap_or_else(|| states[idx].cluster_id.clone()),
                    reason: "opposing_refs",
                    weight: link_weight(record, &states[idx]),
                },
            );
        }
    }

    let mut cluster_cooldowns = BTreeMap::new();
    for indices in cluster_indices.values() {
        let supporting_ids = indices
            .iter()
            .filter(|idx| states[**idx].polarity == EvidencePolarity::Supporting)
            .map(|idx| records[*idx].id.clone())
            .collect::<Vec<_>>();
        let opposing_ids = indices
            .iter()
            .filter(|idx| states[**idx].polarity == EvidencePolarity::Opposing)
            .map(|idx| records[*idx].id.clone())
            .collect::<Vec<_>>();

        if supporting_ids.len() > 1 {
            let representative = supporting_ids[0].clone();
            for evidence_id in supporting_ids.iter().skip(1) {
                let from_idx = evidence_index(&records, evidence_id).unwrap_or(0);
                push_link(
                    &mut links,
                    &mut link_keys,
                    LinkDraft {
                        kind: EvidenceGraphLinkKind::Support,
                        from_evidence_id: evidence_id.clone(),
                        to_evidence_id: representative.clone(),
                        from_cluster_id: states[from_idx].cluster_id.clone(),
                        to_cluster_id: states[from_idx].cluster_id.clone(),
                        reason: "same_cluster_support",
                        weight: link_weight(&records[from_idx], &states[from_idx]),
                    },
                );
            }
        }

        if !supporting_ids.is_empty() && !opposing_ids.is_empty() {
            for idx in indices {
                states[*idx]
                    .conflict_state
                    .add_reason("same_affected_path_cluster_opposition");
                states[*idx]
                    .conflict_state
                    .same_affected_path_cluster_opposition = true;
                let opposite_ids = if states[*idx].polarity == EvidencePolarity::Opposing {
                    supporting_ids.clone()
                } else {
                    opposing_ids.clone()
                };
                states[*idx].conflict_state.add_opposing_ids(opposite_ids);
            }

            for opposing_id in &opposing_ids {
                let from_idx = evidence_index(&records, opposing_id).unwrap_or(0);
                for supporting_id in &supporting_ids {
                    push_link(
                        &mut links,
                        &mut link_keys,
                        LinkDraft {
                            kind: EvidenceGraphLinkKind::Opposition,
                            from_evidence_id: opposing_id.clone(),
                            to_evidence_id: supporting_id.clone(),
                            from_cluster_id: states[from_idx].cluster_id.clone(),
                            to_cluster_id: states[from_idx].cluster_id.clone(),
                            reason: "same_affected_path_cluster_opposition",
                            weight: link_weight(&records[from_idx], &states[from_idx]),
                        },
                    );
                }
            }
        }

        if let Some(first_idx) = indices.first() {
            let cooldown = cooldown_state_for_cluster(indices, &records, &states, now);
            cluster_cooldowns.insert(states[*first_idx].cluster_id.clone(), cooldown);
        }
    }

    sort_links(&mut links);
    let clusters = build_clusters(
        &records,
        &states,
        &cluster_indices,
        &links,
        &cluster_cooldowns,
    );
    let timeline = build_timeline(&records, &states, &links, &cluster_cooldowns, now);
    let support_link_count = links
        .iter()
        .filter(|link| link.kind == EvidenceGraphLinkKind::Support)
        .count();
    let opposition_link_count = links
        .iter()
        .filter(|link| link.kind == EvidenceGraphLinkKind::Opposition)
        .count();
    let conflict_count = timeline
        .items
        .iter()
        .filter(|item| item.conflict_state.conflicted)
        .count();
    let decayed_count = timeline
        .items
        .iter()
        .filter(|item| item.decay_state.decayed)
        .count();
    let cooldown_count = clusters
        .iter()
        .filter(|cluster| cluster.cooldown_state.active)
        .count();

    EvidenceGraphReport {
        report_kind: REPORT_KIND.to_string(),
        graph_ready: true,
        metadata_safe: true,
        contains_raw_content: false,
        generated_at: now,
        record_count: records.len(),
        cluster_count: clusters.len(),
        support_link_count,
        opposition_link_count,
        conflict_count,
        decayed_count,
        cooldown_count,
        ran_runtime: false,
        ran_model: false,
        ran_tool: false,
        wrote_life_model_count: 0,
        wrote_memory_count: 0,
        wrote_heuristic_count: 0,
        wrote_chat_message_count: 0,
        wrote_agent_run_count: 0,
        wrote_mcp_audit_count: 0,
        wrote_external_count: 0,
        clusters,
        links,
        timeline,
    }
}

fn build_clusters(
    records: &[EvidenceRecord],
    states: &[RecordGraphState],
    cluster_indices: &BTreeMap<String, Vec<usize>>,
    links: &[EvidenceGraphLink],
    cluster_cooldowns: &BTreeMap<String, EvidenceCooldownState>,
) -> Vec<EvidenceClusterSummary> {
    let mut clusters = Vec::new();
    for (affected_path, indices) in cluster_indices {
        let first_idx = indices[0];
        let cluster_id = states[first_idx].cluster_id.clone();
        let cluster_hash = states[first_idx].cluster_hash.clone();
        let mut evidence_types = Vec::new();
        let mut evidence_ids = Vec::new();
        let mut supporting_evidence_ids = Vec::new();
        let mut opposing_evidence_ids = Vec::new();
        let mut dedupe_key_hashes = Vec::new();
        let mut source_weights_by_type: BTreeMap<String, EvidenceSourceWeightSummary> =
            BTreeMap::new();
        let mut confidence_sum = 0.0;
        let mut effective_confidence_sum = 0.0;
        let mut source_weight_total = 0.0;
        let mut conflict_state = EvidenceConflictState::clean();
        let mut newest_observed_at = records[first_idx].last_observed_at;
        let mut oldest_observed_at = records[first_idx].last_observed_at;

        for idx in indices {
            let record = &records[*idx];
            push_unique(&mut evidence_types, record.evidence_type.to_string());
            push_unique(&mut evidence_ids, record.id.clone());
            match states[*idx].polarity {
                EvidencePolarity::Opposing => {
                    push_unique(&mut opposing_evidence_ids, record.id.clone())
                }
                _ => push_unique(&mut supporting_evidence_ids, record.id.clone()),
            }
            for hash in record_dedupe_key_hashes(record, affected_path) {
                push_unique(&mut dedupe_key_hashes, hash);
            }
            for source_ref in &record.source_refs {
                let source_type = source_ref.source_type.to_string();
                let entry = source_weights_by_type
                    .entry(source_type.clone())
                    .or_insert_with(|| EvidenceSourceWeightSummary {
                        source_type,
                        ref_count: 0,
                        weight_total: 0.0,
                    });
                entry.ref_count += 1;
                entry.weight_total =
                    round4(entry.weight_total + source_type_weight(source_ref.source_type));
            }
            source_weight_total = round4(source_weight_total + states[*idx].source_weight_total);
            confidence_sum += record.confidence;
            effective_confidence_sum += states[*idx].decay_state.effective_confidence;
            merge_conflict_state(&mut conflict_state, &states[*idx].conflict_state);
            if record.last_observed_at > newest_observed_at {
                newest_observed_at = record.last_observed_at;
            }
            if record.last_observed_at < oldest_observed_at {
                oldest_observed_at = record.last_observed_at;
            }
        }

        let support_link_count = links
            .iter()
            .filter(|link| {
                link.kind == EvidenceGraphLinkKind::Support && link.from_cluster_id == cluster_id
            })
            .count();
        let opposition_link_count = links
            .iter()
            .filter(|link| {
                link.kind == EvidenceGraphLinkKind::Opposition && link.from_cluster_id == cluster_id
            })
            .count();

        clusters.push(EvidenceClusterSummary {
            cluster_id: cluster_id.clone(),
            cluster_hash,
            affected_path: affected_path.clone(),
            evidence_types,
            evidence_ids,
            supporting_evidence_ids,
            opposing_evidence_ids,
            dedupe_key_hashes,
            record_count: indices.len(),
            source_weights: source_weights_by_type.into_values().collect(),
            source_weight_total,
            average_confidence: round4(confidence_sum / indices.len() as f32),
            effective_confidence: round4(effective_confidence_sum / indices.len() as f32),
            support_link_count,
            opposition_link_count,
            summary: format!("evidence cluster for {affected_path}"),
            conflict_state,
            cooldown_state: cluster_cooldowns
                .get(&cluster_id)
                .cloned()
                .unwrap_or_else(EvidenceCooldownState::inactive),
            newest_observed_at,
            oldest_observed_at,
        });
    }
    clusters
}

fn build_timeline(
    records: &[EvidenceRecord],
    states: &[RecordGraphState],
    links: &[EvidenceGraphLink],
    cluster_cooldowns: &BTreeMap<String, EvidenceCooldownState>,
    now: DateTime<Utc>,
) -> EvidenceTimelineReadModel {
    let mut items = records
        .iter()
        .enumerate()
        .map(|(idx, record)| EvidenceTimelineItem {
            evidence_id: record.id.clone(),
            evidence_type: record.evidence_type.to_string(),
            affected_path: record.affected_path.clone(),
            status: record.status.to_string(),
            confidence: round4(record.confidence),
            risk_level: record.risk_level.to_string(),
            privacy_level: record.privacy_level.to_string(),
            polarity: states[idx].polarity,
            source_ref_count: record.source_refs.len(),
            support_link_count: link_count_for_evidence(
                links,
                &record.id,
                EvidenceGraphLinkKind::Support,
            ),
            opposition_link_count: link_count_for_evidence(
                links,
                &record.id,
                EvidenceGraphLinkKind::Opposition,
            ),
            linked_proposal_ids: record.linked_proposal_ids.clone(),
            linked_agent_run_ids: record.linked_agent_run_ids.clone(),
            cluster_id: states[idx].cluster_id.clone(),
            cluster_hash: states[idx].cluster_hash.clone(),
            conflict_state: states[idx].conflict_state.clone(),
            decay_state: states[idx].decay_state.clone(),
            cooldown_state: cluster_cooldowns
                .get(&states[idx].cluster_id)
                .cloned()
                .unwrap_or_else(EvidenceCooldownState::inactive),
            created_at: record.created_at,
            updated_at: record.updated_at,
            last_observed_at: record.last_observed_at,
        })
        .collect::<Vec<_>>();

    items.sort_by(|a, b| {
        b.last_observed_at
            .cmp(&a.last_observed_at)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
            .then_with(|| a.evidence_id.cmp(&b.evidence_id))
    });

    EvidenceTimelineReadModel {
        report_kind: TIMELINE_KIND.to_string(),
        metadata_safe: true,
        contains_raw_content: false,
        generated_at: now,
        item_count: items.len(),
        items,
    }
}

fn cooldown_state_for_cluster(
    indices: &[usize],
    records: &[EvidenceRecord],
    states: &[RecordGraphState],
    now: DateTime<Utc>,
) -> EvidenceCooldownState {
    let mut state = EvidenceCooldownState::inactive();
    let mut cooldown_until = None;
    for idx in indices {
        let record = &records[*idx];
        if !is_rejected_opposition(record) {
            continue;
        }
        let until = record.last_observed_at + Duration::days(REJECTED_SIMILAR_COOLDOWN_DAYS);
        if until <= now {
            continue;
        }
        state.active = true;
        state.reason = Some("recent_rejected_similar_proposal_outcome".to_string());
        state.similar_cluster_id = Some(states[*idx].cluster_id.clone());
        state.cooldown_until = Some(match cooldown_until {
            Some(current) if current >= until => current,
            _ => until,
        });
        cooldown_until = state.cooldown_until;
        push_unique(&mut state.rejected_evidence_ids, record.id.clone());
        for proposal_id in rejected_proposal_ids(record) {
            push_unique(&mut state.rejected_proposal_ids, proposal_id);
        }
    }
    if let Some(until) = state.cooldown_until {
        state.days_remaining = (until - now).num_days().max(0);
    }
    state
}

fn decay_state(record: &EvidenceRecord, now: DateTime<Utc>) -> EvidenceDecayState {
    let age_days = (now - record.last_observed_at).num_days().max(0);
    let decay_factor = if age_days <= DECAY_GRACE_DAYS {
        1.0
    } else {
        let excess_days = (age_days - DECAY_GRACE_DAYS) as f32;
        0.5_f32
            .powf(excess_days / DECAY_HALF_LIFE_DAYS)
            .max(MIN_DECAY_FACTOR)
    };
    EvidenceDecayState {
        generated_at: now,
        last_observed_at: record.last_observed_at,
        age_days,
        grace_days: DECAY_GRACE_DAYS,
        half_life_days: DECAY_HALF_LIFE_DAYS,
        decay_factor: round4(decay_factor),
        effective_confidence: round4(record.confidence.clamp(0.0, 1.0) * decay_factor),
        decayed: decay_factor < 1.0,
    }
}

fn push_link(
    links: &mut Vec<EvidenceGraphLink>,
    link_keys: &mut BTreeSet<(EvidenceGraphLinkKind, String, String, String)>,
    draft: LinkDraft,
) {
    if draft.from_evidence_id.trim().is_empty()
        || draft.to_evidence_id.trim().is_empty()
        || draft.from_evidence_id == draft.to_evidence_id
    {
        return;
    }
    let key = (
        draft.kind,
        draft.from_evidence_id.clone(),
        draft.to_evidence_id.clone(),
        draft.reason.to_string(),
    );
    if !link_keys.insert(key) {
        return;
    }
    links.push(EvidenceGraphLink {
        kind: draft.kind,
        from_evidence_id: draft.from_evidence_id,
        to_evidence_id: draft.to_evidence_id,
        from_cluster_id: draft.from_cluster_id,
        to_cluster_id: draft.to_cluster_id,
        reason: draft.reason.to_string(),
        weight: round4(draft.weight),
    });
}

fn sort_records(records: &mut [EvidenceRecord]) {
    records.sort_by(|a, b| {
        a.affected_path
            .cmp(&b.affected_path)
            .then_with(|| a.created_at.cmp(&b.created_at))
            .then_with(|| a.id.cmp(&b.id))
    });
}

fn sort_links(links: &mut [EvidenceGraphLink]) {
    links.sort_by(|a, b| {
        a.from_evidence_id
            .cmp(&b.from_evidence_id)
            .then_with(|| a.to_evidence_id.cmp(&b.to_evidence_id))
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.reason.cmp(&b.reason))
    });
}

fn cluster_hash_for_path(affected_path: &str) -> String {
    sha256_hex(
        json!({
            "schema": REPORT_KIND,
            "clusterBy": "affected_path",
            "affectedPath": affected_path
        })
        .to_string()
        .as_bytes(),
    )
}

fn cluster_id_for_evidence_id(
    records: &[EvidenceRecord],
    states: &[RecordGraphState],
    evidence_id: &str,
) -> Option<String> {
    evidence_index(records, evidence_id).map(|idx| states[idx].cluster_id.clone())
}

fn evidence_index(records: &[EvidenceRecord], evidence_id: &str) -> Option<usize> {
    records.iter().position(|record| record.id == evidence_id)
}

fn link_count_for_evidence(
    links: &[EvidenceGraphLink],
    evidence_id: &str,
    kind: EvidenceGraphLinkKind,
) -> usize {
    links
        .iter()
        .filter(|link| {
            link.kind == kind
                && (link.from_evidence_id == evidence_id || link.to_evidence_id == evidence_id)
        })
        .count()
}

fn link_weight(record: &EvidenceRecord, state: &RecordGraphState) -> f32 {
    let source_weight = if state.source_weight_total > 0.0 {
        state.source_weight_total
    } else {
        0.25
    };
    (record.confidence.clamp(0.0, 1.0) * source_weight).clamp(0.0, 10.0)
}

fn record_source_weight_total(record: &EvidenceRecord) -> f32 {
    if record.source_refs.is_empty() {
        return 0.25;
    }
    round4(
        record
            .source_refs
            .iter()
            .map(|source_ref| source_type_weight(source_ref.source_type))
            .sum(),
    )
}

fn source_type_weight(source_type: EvidenceSourceType) -> f32 {
    match source_type {
        EvidenceSourceType::UserEdit => 1.0,
        EvidenceSourceType::Proposal => 0.9,
        EvidenceSourceType::AgentRun => 0.8,
        EvidenceSourceType::Feedback => 0.75,
        EvidenceSourceType::RunMetadata => 0.6,
        EvidenceSourceType::ChatMessage => 0.5,
        EvidenceSourceType::MemoryRecord => 0.45,
        EvidenceSourceType::VectorChunk => 0.35,
        EvidenceSourceType::Other => 0.25,
    }
}

fn infer_polarity(record: &EvidenceRecord) -> EvidencePolarity {
    if is_rejected_opposition(record) || record.evidence_type == EvidenceType::Contradiction {
        EvidencePolarity::Opposing
    } else {
        EvidencePolarity::Supporting
    }
}

fn is_rejected_opposition(record: &EvidenceRecord) -> bool {
    record.evidence_type == EvidenceType::ProposalOutcome
        && (metadata_str(&record.run_metadata, "outcome") == Some("rejected")
            || metadata_bool(&record.run_metadata, "negative")
            || metadata_bool(&record.run_metadata, "opposing"))
}

fn rejected_proposal_ids(record: &EvidenceRecord) -> Vec<String> {
    let mut ids = record.linked_proposal_ids.clone();
    if let Some(proposal_id) = metadata_str(&record.run_metadata, "proposalId") {
        push_unique(&mut ids, proposal_id.to_string());
    }
    ids
}

fn metadata_source_evidence_ids(value: &Value) -> Vec<String> {
    metadata_string_array(value, "sourceEvidenceIds")
}

fn metadata_string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|item| !item.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn metadata_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn metadata_bool(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn record_dedupe_key_hashes(record: &EvidenceRecord, affected_path: &str) -> Vec<String> {
    let mut hashes = Vec::new();
    for key in [
        "signalDedupeKeyDigest",
        "sourceEventDedupeKeyDigest",
        "sourceEventDigest",
        "proposalDigest",
    ] {
        if let Some(value) = record.run_metadata.get(key).and_then(Value::as_str) {
            push_unique(&mut hashes, value.to_string());
        }
    }
    if hashes.is_empty() {
        hashes.push(sha256_hex(
            json!({
                "affectedPath": affected_path,
                "evidenceType": record.evidence_type.to_string()
            })
            .to_string()
            .as_bytes(),
        ));
    }
    hashes
}

fn merge_conflict_state(target: &mut EvidenceConflictState, source: &EvidenceConflictState) {
    for reason in &source.reasons {
        target.add_reason(reason);
    }
    target.add_opposing_ids(source.opposing_evidence_ids.clone());
    target.contradicted |= source.contradicted;
    target.rejected_opposition |= source.rejected_opposition;
    target.same_affected_path_cluster_opposition |= source.same_affected_path_cluster_opposition;
    target.conflicted = target.conflicted
        || source.conflicted
        || target.contradicted
        || target.rejected_opposition
        || target.same_affected_path_cluster_opposition;
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn round4(value: f32) -> f32 {
    (value * 10_000.0).round() / 10_000.0
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = digest(&SHA256, bytes);
    digest
        .as_ref()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn short_hash(value: &str) -> String {
    sha256_hex(value.as_bytes()).chars().take(16).collect()
}
