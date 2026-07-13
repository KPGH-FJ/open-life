use crate::agent::action_executor::tool_executor::{
    build_simulated_observed_read_contract_fixture, SimulatedObservedReadContractFixture,
};
use crate::agent::main_chat_agent_v1::{AgentIngress, PolicyDecision};
use crate::agent::structured_memory_evidence::{
    admit_structured_memory_evidence, CanonicalObservationEvidence, FinalProviderEvidence,
    MemoryEvidenceAdmissionRequest, MemoryEvidenceAdmissionStatus, MemoryEvidenceRequestSource,
    StructuredEvidenceCredit, StructuredMemoryEvidenceEnvelope,
};
use crate::agent::{AgentTaskKind, MemoryDestination};
use crate::config::NetworkPolicy;
use crate::llm::{
    BoundedContextBlock, ChatMessage, ContextManifest, PreparedProviderRequest,
    ProviderInvocationReceipt, ProviderInvocationStatus, ProviderPayloadCategory,
    ProviderPayloadPurpose, ProviderPolicyAuthorization,
};
use crate::scheduler::InferenceScheduler;

const USER_PROMPT: &str = "Read file `src-tauri/test-fixtures/d051_useful_memory.md` and create a memory proposal only if the observation contains a useful supported personal fact.";
const CANDIDATE_TEXT: &str = "The user works in UTC.";

fn sha256(value: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, value);
    let hex = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn observation_ref(graph: &SimulatedObservedReadContractFixture) -> String {
    let action = graph.run.actions.first().expect("one canonical action");
    let observation = graph
        .run
        .observations
        .first()
        .expect("one canonical observation");
    format!(
        "agent-run://{}/action/{}/observation/{}",
        graph.run.id, action.id, observation.id
    )
}

fn exact_slice(body: &str, candidate: &str) -> (usize, usize, String) {
    let start = body
        .find(candidate)
        .unwrap_or_else(|| panic!("candidate is absent from observation: {candidate:?}"));
    let end = start + candidate.len();
    (start, end, sha256(&body.as_bytes()[start..end]))
}

fn final_response(observation_ref: &str, observation_body: &str, candidate: &str) -> String {
    let (start, end, digest) = exact_slice(observation_body, candidate);
    serde_json::json!({
        "final": "The governed read completed.",
        "actions": [],
        "thought_summary": "The observation is sufficient.",
        "warnings": [],
        "memory_evidence_schema": "openlife.memory_evidence.v1",
        "memory_evidence": [{
            "candidate_text": candidate,
            "subject": "current_user",
            "assertion": "asserted_fact",
            "modality": "asserted",
            "confidence": 0.93,
            "evidence": {
                "observation_ref": observation_ref,
                "start_byte": start,
                "end_byte": end,
                "sha256": digest,
            }
        }]
    })
    .to_string()
}

async fn prepared_final_request(
    decision: &crate::agent::main_chat_agent_v1::AgentIngressDecision,
    observation_ref: &str,
    observation_body: &str,
) -> PreparedProviderRequest {
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: "Return the final AgentLoop JSON envelope.".into(),
        },
        ChatMessage {
            role: "user".into(),
            content: USER_PROMPT.into(),
        },
    ];
    let context_blocks = vec![BoundedContextBlock {
        source_ref: observation_ref.to_string(),
        category: "untrusted_tool_observation".into(),
        content: observation_body.to_string(),
    }];
    let authorization = ProviderPolicyAuthorization::from_main_chat_ingress(decision)
        .and_then(|authorization| {
            authorization.authorize_derived_payload(
                ProviderPayloadPurpose::AgentLoopStep,
                USER_PROMPT,
                &messages,
                &context_blocks,
            )
        })
        .expect("live PolicyRouter provider authorization");
    let request_id = uuid::Uuid::new_v4().to_string();
    let privacy_decision_id = authorization.decision_id().to_string();
    let scheduler = InferenceScheduler::new(
        "unused-local".into(),
        false,
        "openai".into(),
        "https://api.openai.com/v1".into(),
        "test-key".into(),
        "gpt-d051-contract".into(),
        "unused-embedding".into(),
        false,
    );
    scheduler
        .prepare_chat_request_with_authorization(
            messages,
            context_blocks,
            ContextManifest {
                request_id,
                privacy_decision_id,
                selected_context_refs: vec![observation_ref.to_string()],
                included_context_categories: vec!["untrusted_tool_observation".into()],
                declared_payload_categories: vec![ProviderPayloadCategory::RuntimeCompiledMessages],
                policy_provenance_refs: Vec::new(),
                raw_life_model_included: false,
                raw_unbounded_memory_included: false,
            },
            authorization,
            NetworkPolicy {
                enabled: true,
                default_decision: "allow".into(),
                ..NetworkPolicy::default()
            },
            true,
        )
        .await
        .expect("prepare exact final provider request")
}

struct AdmissionFixture {
    graph: SimulatedObservedReadContractFixture,
    decision: crate::agent::main_chat_agent_v1::AgentIngressDecision,
    prepared: PreparedProviderRequest,
    provider_receipt: ProviderInvocationReceipt,
    response_body: String,
    envelope: StructuredMemoryEvidenceEnvelope,
    operation_id: String,
    execution_epoch_id: String,
}

impl AdmissionFixture {
    async fn with_observation(observed_body: &str, candidate: &str) -> Self {
        let graph = build_simulated_observed_read_contract_fixture(observed_body);
        let observation_ref = observation_ref(&graph);
        let decision = AgentIngress::default().decide(
            "d051-typed-admission",
            USER_PROMPT,
            None,
            AgentTaskKind::Conversation,
        );
        let prepared = prepared_final_request(&decision, &observation_ref, observed_body).await;
        let now = chrono::Utc::now();
        let provider_receipt = ProviderInvocationReceipt {
            request_id: prepared.context_manifest.request_id.clone(),
            provider: prepared.provider_target.clone(),
            model: prepared.model_target.clone(),
            status: ProviderInvocationStatus::Completed,
            started_at: now - chrono::Duration::milliseconds(5),
            finished_at: now,
            error_digest: None,
            simulated: false,
            policy_evidence: Some(prepared.policy_receipt_evidence()),
        };
        let response_body = final_response(&observation_ref, observed_body, candidate);
        let envelope = StructuredMemoryEvidenceEnvelope::parse_final_response(&response_body)
            .expect("parse typed D051 final envelope");
        Self {
            operation_id: graph.run.id.clone(),
            execution_epoch_id: uuid::Uuid::new_v4().to_string(),
            graph,
            decision,
            prepared,
            provider_receipt,
            response_body,
            envelope,
        }
    }

    async fn positive() -> Self {
        Self::with_observation(
            "D051_RAW_OBSERVATION_SENTINEL_START\nThe user works in UTC.\nD051_RAW_OBSERVATION_SENTINEL_END\n",
            CANDIDATE_TEXT,
        )
        .await
    }

    fn observation_evidence(&self) -> CanonicalObservationEvidence<'_> {
        CanonicalObservationEvidence::verify_runtime_graph(
            &self.graph.store,
            &self.graph.run,
            &self.graph.tool_receipt,
            &self.graph.observed_body,
        )
        .expect("verify typed simulated canonical observation graph")
    }

    fn provider_evidence(&self) -> FinalProviderEvidence<'_> {
        FinalProviderEvidence::verify(
            &self.prepared,
            &self.provider_receipt,
            &sha256(self.response_body.as_bytes()),
            &self.response_body,
            1,
            1,
            StructuredEvidenceCredit::TypedCoreContract,
        )
        .expect("verify exact typed final-provider fixture")
    }

    fn request<'a>(
        &'a self,
        observation: CanonicalObservationEvidence<'a>,
        provider: FinalProviderEvidence<'a>,
    ) -> MemoryEvidenceAdmissionRequest<'a> {
        MemoryEvidenceAdmissionRequest {
            operation_id: &self.operation_id,
            execution_epoch_id: &self.execution_epoch_id,
            current_user_message_id: &self.decision.policy_decision.authorized_user_message_id,
            current_user_message_digest: &self
                .decision
                .policy_decision
                .authorized_user_message_digest,
            current_request_is_explicit: true,
            current_request_source: MemoryEvidenceRequestSource::CurrentAuthenticatedUserMessage,
            policy_decision: &self.decision.policy_decision,
            observation,
            final_provider: provider,
            envelope: &self.envelope,
        }
    }

    fn admit(&self) -> crate::agent::structured_memory_evidence::MemoryEvidenceAdmission {
        let observation = self.observation_evidence();
        let provider = self.provider_evidence();
        admit_structured_memory_evidence(self.request(observation, provider))
            .expect("typed D051 admission outcome")
    }
}

#[test]
fn d051_product_behavior_not_symbol_names_removes_legacy_implicit_authority() {
    let routed = crate::agent::extract_main_chat_memory_candidates(CANDIDATE_TEXT);
    assert!(
        routed
            .iter()
            .all(|candidate| candidate.destination != MemoryDestination::MemoryProposal),
        "plain untrusted observation prose must have no proposal authority before typed provider evidence"
    );
}

#[tokio::test]
async fn d051_same_existing_final_provider_receipt_and_exact_observation_are_required() {
    let fixture = AdmissionFixture::positive().await;
    let admitted = fixture.admit();

    assert_eq!(
        admitted.status,
        MemoryEvidenceAdmissionStatus::CandidateAdmitted
    );
    assert_eq!(
        admitted.reason_code,
        "same_final_provider_evidence_admitted"
    );
    assert_eq!(admitted.candidate_body.as_deref(), Some(CANDIDATE_TEXT));
    assert_eq!(admitted.credit, StructuredEvidenceCredit::TypedCoreContract);
    assert!(
        !admitted.external_live_credit,
        "typed Core fixture evidence is never product or external-live credit"
    );
}

#[tokio::test]
async fn d051_provider_request_response_receipt_and_manifest_counterfactuals_fail_closed() {
    let mut simulated = AdmissionFixture::positive().await;
    simulated.provider_receipt.simulated = true;
    assert!(FinalProviderEvidence::verify(
        &simulated.prepared,
        &simulated.provider_receipt,
        &sha256(simulated.response_body.as_bytes()),
        &simulated.response_body,
        1,
        1,
        StructuredEvidenceCredit::TypedCoreContract,
    )
    .is_err());

    let mut failed = AdmissionFixture::positive().await;
    failed.provider_receipt.status = ProviderInvocationStatus::Failed;
    assert!(FinalProviderEvidence::verify(
        &failed.prepared,
        &failed.provider_receipt,
        &sha256(failed.response_body.as_bytes()),
        &failed.response_body,
        1,
        1,
        StructuredEvidenceCredit::TypedCoreContract,
    )
    .is_err());

    let mut wrong_request = AdmissionFixture::positive().await;
    wrong_request.provider_receipt.request_id = uuid::Uuid::new_v4().to_string();
    assert!(FinalProviderEvidence::verify(
        &wrong_request.prepared,
        &wrong_request.provider_receipt,
        &sha256(wrong_request.response_body.as_bytes()),
        &wrong_request.response_body,
        1,
        1,
        StructuredEvidenceCredit::TypedCoreContract,
    )
    .is_err());

    let wrong_response = AdmissionFixture::positive().await;
    assert!(FinalProviderEvidence::verify(
        &wrong_response.prepared,
        &wrong_response.provider_receipt,
        &sha256(b"different final response"),
        &wrong_response.response_body,
        1,
        1,
        StructuredEvidenceCredit::TypedCoreContract,
    )
    .is_err());

    let mut no_manifest_match = AdmissionFixture::positive().await;
    no_manifest_match.prepared.context_blocks.clear();
    no_manifest_match
        .prepared
        .context_manifest
        .selected_context_refs
        .clear();
    no_manifest_match
        .prepared
        .context_manifest
        .included_context_categories
        .clear();
    assert!(FinalProviderEvidence::verify(
        &no_manifest_match.prepared,
        &no_manifest_match.provider_receipt,
        &sha256(no_manifest_match.response_body.as_bytes()),
        &no_manifest_match.response_body,
        1,
        1,
        StructuredEvidenceCredit::TypedCoreContract,
    )
    .is_err());

    let mut two_manifest_matches = AdmissionFixture::positive().await;
    two_manifest_matches
        .prepared
        .context_blocks
        .push(two_manifest_matches.prepared.context_blocks[0].clone());
    assert!(FinalProviderEvidence::verify(
        &two_manifest_matches.prepared,
        &two_manifest_matches.provider_receipt,
        &sha256(two_manifest_matches.response_body.as_bytes()),
        &two_manifest_matches.response_body,
        1,
        1,
        StructuredEvidenceCredit::TypedCoreContract,
    )
    .is_err());
}

#[tokio::test]
async fn d051_canonical_observation_receipt_and_owner_counterfactuals_fail_closed() {
    let mut missing_receipt = AdmissionFixture::positive().await;
    missing_receipt.graph.run.actions[0]
        .react_trace
        .as_mut()
        .expect("action trace")
        .output_receipt = None;
    assert!(CanonicalObservationEvidence::verify_runtime_graph(
        &missing_receipt.graph.store,
        &missing_receipt.graph.run,
        &missing_receipt.graph.tool_receipt,
        &missing_receipt.graph.observed_body,
    )
    .is_err());

    let transplanted_graph =
        build_simulated_observed_read_contract_fixture(&missing_receipt.graph.observed_body);
    assert!(CanonicalObservationEvidence::verify_runtime_graph(
        &missing_receipt.graph.store,
        &transplanted_graph.run,
        &transplanted_graph.tool_receipt,
        &transplanted_graph.observed_body,
    )
    .is_err());

    for (field, forged) in [
        (
            "observation_ref",
            serde_json::json!("agent-run://forged/action/forged/observation/forged"),
        ),
        ("sha256", serde_json::json!(sha256(b"forged slice"))),
    ] {
        let mut fixture = AdmissionFixture::positive().await;
        let mut response: serde_json::Value =
            serde_json::from_str(&fixture.response_body).expect("D051 response JSON");
        response["memory_evidence"][0]["evidence"][field] = forged;
        fixture.response_body = response.to_string();
        fixture.envelope =
            StructuredMemoryEvidenceEnvelope::parse_final_response(&fixture.response_body)
                .expect("parse forged evidence envelope without granting it");
        let outcome = fixture.admit();
        assert_eq!(
            outcome.status,
            MemoryEvidenceAdmissionStatus::Rejected,
            "{field}"
        );
        assert!(!outcome.reason_code.is_empty(), "{field}");
    }
}

#[tokio::test]
async fn d051_current_authenticated_explicit_low_medium_review_lane_is_required() {
    let fixture = AdmissionFixture::positive().await;
    let observation = fixture.observation_evidence();
    let provider = fixture.provider_evidence();
    let mut request = fixture.request(observation, provider);
    request.current_request_is_explicit = false;
    let not_explicit = admit_structured_memory_evidence(request).expect("typed rejection");
    assert_eq!(not_explicit.status, MemoryEvidenceAdmissionStatus::Rejected);
    assert_eq!(not_explicit.reason_code, "current_request_not_explicit");

    let fixture = AdmissionFixture::positive().await;
    let observation = fixture.observation_evidence();
    let provider = fixture.provider_evidence();
    let mut request = fixture.request(observation, provider);
    request.current_request_source = MemoryEvidenceRequestSource::UntrustedContent;
    let untrusted = admit_structured_memory_evidence(request).expect("typed rejection");
    assert_eq!(untrusted.status, MemoryEvidenceAdmissionStatus::Rejected);
    assert_eq!(
        untrusted.reason_code,
        "current_request_source_not_authenticated_user"
    );

    let mut forged_policy = AdmissionFixture::positive().await;
    forged_policy.decision.policy_decision = PolicyDecision::default();
    let rejected = forged_policy.admit();
    assert_eq!(rejected.status, MemoryEvidenceAdmissionStatus::Rejected);

    let fixture = AdmissionFixture::positive().await;
    let observation = fixture.observation_evidence();
    let provider = fixture.provider_evidence();
    let mut request = fixture.request(observation, provider);
    request.current_user_message_id = "conversation://forged/message/1";
    let wrong_user_owner =
        admit_structured_memory_evidence(request).expect("typed user-owner rejection");
    assert_eq!(
        wrong_user_owner.status,
        MemoryEvidenceAdmissionStatus::Rejected
    );

    let fixture = AdmissionFixture::positive().await;
    let observation = fixture.observation_evidence();
    let provider = fixture.provider_evidence();
    let mut request = fixture.request(observation, provider);
    request.operation_id = "forged-operation";
    let wrong_operation =
        admit_structured_memory_evidence(request).expect("typed operation-owner rejection");
    assert_eq!(
        wrong_operation.status,
        MemoryEvidenceAdmissionStatus::Rejected
    );
}

#[tokio::test]
async fn d051_model_subject_assertion_modality_and_confidence_can_only_reject() {
    for (field, value, reason) in [
        (
            "subject",
            serde_json::json!("other"),
            "draft_subject_not_current_user",
        ),
        (
            "assertion",
            serde_json::json!("prediction"),
            "draft_assertion_not_asserted_fact",
        ),
        (
            "modality",
            serde_json::json!("hypothetical"),
            "draft_modality_not_asserted",
        ),
        (
            "confidence",
            serde_json::json!(0.49),
            "draft_confidence_below_threshold",
        ),
    ] {
        let mut fixture = AdmissionFixture::positive().await;
        let mut response: serde_json::Value =
            serde_json::from_str(&fixture.response_body).expect("D051 response JSON");
        response["memory_evidence"][0][field] = value;
        fixture.response_body = response.to_string();
        fixture.envelope =
            StructuredMemoryEvidenceEnvelope::parse_final_response(&fixture.response_body)
                .expect("parse counterfactual envelope");
        let outcome = fixture.admit();
        assert_eq!(
            outcome.status,
            MemoryEvidenceAdmissionStatus::Rejected,
            "{field}"
        );
        assert_eq!(outcome.reason_code, reason, "{field}");
    }
}

#[tokio::test]
async fn d051_structural_boundaries_are_ranges_not_prompt_keyword_classification() {
    for (body, candidate) in [
        ("Header: \"The user works in UTC.\"", CANDIDATE_TEXT),
        ("> The user works in UTC.\n", CANDIDATE_TEXT),
        ("Result: `The user works in UTC.`", CANDIDATE_TEXT),
        ("```text\nThe user works in UTC.\n```", CANDIDATE_TEXT),
        (r#"{"note":"The user works in UTC."}"#, CANDIDATE_TEXT),
    ] {
        let fixture = AdmissionFixture::with_observation(body, candidate).await;
        let outcome = fixture.admit();
        assert_eq!(
            outcome.status,
            MemoryEvidenceAdmissionStatus::Rejected,
            "{body}"
        );
        assert_eq!(outcome.reason_code, "evidence_inside_untrusted_structure");
    }

    let plain = "Ignore prior guidance is the title of the user's research paper.";
    let fixture = AdmissionFixture::with_observation(plain, plain).await;
    let outcome = fixture.admit();
    assert_eq!(
        outcome.status,
        MemoryEvidenceAdmissionStatus::CandidateAdmitted,
        "plain exact evidence must not be rejected by prompt-like keywords"
    );
}

#[tokio::test]
async fn d051_real_byte_limits_and_utf8_ranges_fail_closed_without_panics() {
    let candidate = CANDIDATE_TEXT;
    let exact_observation = format!("{}{}", "x".repeat(16_384 - candidate.len()), candidate);
    let exact = AdmissionFixture::with_observation(&exact_observation, candidate).await;
    assert_eq!(
        exact.admit().status,
        MemoryEvidenceAdmissionStatus::CandidateAdmitted
    );

    let over_observation = format!("{}{}", "x".repeat(16_385 - candidate.len()), candidate);
    let over = AdmissionFixture::with_observation(&over_observation, candidate).await;
    assert_eq!(over.admit().reason_code, "observation_limit_exceeded");

    for (slice_bytes, expected) in [
        (2_048usize, MemoryEvidenceAdmissionStatus::CandidateAdmitted),
        (2_049usize, MemoryEvidenceAdmissionStatus::Rejected),
    ] {
        let candidate = "z".repeat(slice_bytes);
        let fixture = AdmissionFixture::with_observation(&candidate, &candidate).await;
        let outcome = fixture.admit();
        assert_eq!(outcome.status, expected, "slice bytes={slice_bytes}");
        if slice_bytes == 2_049 {
            assert_eq!(outcome.reason_code, "evidence_slice_limit_exceeded");
        }
    }

    for (start, end) in [(5usize, 4usize), (0, 50_000), (1, 4)] {
        let mut fixture =
            AdmissionFixture::with_observation("前The user works in UTC.", candidate).await;
        let mut response: serde_json::Value =
            serde_json::from_str(&fixture.response_body).expect("D051 response JSON");
        response["memory_evidence"][0]["evidence"]["start_byte"] = serde_json::json!(start);
        response["memory_evidence"][0]["evidence"]["end_byte"] = serde_json::json!(end);
        fixture.response_body = response.to_string();
        fixture.envelope =
            StructuredMemoryEvidenceEnvelope::parse_final_response(&fixture.response_body)
                .expect("parse invalid-range envelope without slicing");
        let outcome = fixture.admit();
        assert_eq!(outcome.status, MemoryEvidenceAdmissionStatus::Rejected);
        assert_eq!(outcome.reason_code, "evidence_range_invalid");
    }
}

#[tokio::test]
async fn d051_candidate_cardinality_and_extractor_unavailability_are_typed() {
    let mut empty = AdmissionFixture::positive().await;
    let mut response: serde_json::Value =
        serde_json::from_str(&empty.response_body).expect("D051 response JSON");
    response["memory_evidence"] = serde_json::json!([]);
    empty.response_body = response.to_string();
    empty.envelope = StructuredMemoryEvidenceEnvelope::parse_final_response(&empty.response_body)
        .expect("parse empty evidence array");
    let empty_outcome = empty.admit();
    assert_eq!(
        empty_outcome.status,
        MemoryEvidenceAdmissionStatus::NoCandidate
    );
    assert_eq!(empty_outcome.reason_code, "provider_returned_no_candidate");

    let mut multiple = AdmissionFixture::positive().await;
    let mut response: serde_json::Value =
        serde_json::from_str(&multiple.response_body).expect("D051 response JSON");
    let draft = response["memory_evidence"][0].clone();
    response["memory_evidence"] = serde_json::json!([draft.clone(), draft]);
    multiple.response_body = response.to_string();
    multiple.envelope =
        StructuredMemoryEvidenceEnvelope::parse_final_response(&multiple.response_body)
            .expect("parse multiple evidence drafts");
    let multiple_outcome = multiple.admit();
    assert_eq!(
        multiple_outcome.status,
        MemoryEvidenceAdmissionStatus::Rejected
    );
    assert_eq!(
        multiple_outcome.reason_code,
        "ambiguous_multiple_candidates"
    );

    let mut over_limit = AdmissionFixture::positive().await;
    let mut response: serde_json::Value =
        serde_json::from_str(&over_limit.response_body).expect("D051 response JSON");
    let draft = response["memory_evidence"][0].clone();
    response["memory_evidence"] = serde_json::Value::Array(vec![draft; 5]);
    over_limit.response_body = response.to_string();
    over_limit.envelope =
        StructuredMemoryEvidenceEnvelope::parse_final_response(&over_limit.response_body)
            .expect("parse over-limit evidence envelope without admitting it");
    let over_limit_outcome = over_limit.admit();
    assert_eq!(
        over_limit_outcome.status,
        MemoryEvidenceAdmissionStatus::Rejected
    );
    assert_eq!(over_limit_outcome.reason_code, "draft_limit_exceeded");

    for response_body in [
        "not-json".to_string(),
        serde_json::json!({"final":"answer","actions":[]}).to_string(),
        serde_json::json!({
            "final":"answer",
            "actions":[],
            "memory_evidence_schema":"openlife.memory_evidence.v1",
            "memory_evidence":"not-an-array"
        })
        .to_string(),
    ] {
        let mut fixture = AdmissionFixture::positive().await;
        fixture.response_body = response_body;
        fixture.envelope =
            StructuredMemoryEvidenceEnvelope::parse_final_response(&fixture.response_body)
                .expect("extractor unavailability is a typed envelope state");
        let outcome = fixture.admit();
        assert_eq!(outcome.status, MemoryEvidenceAdmissionStatus::Unavailable);
    }
}
