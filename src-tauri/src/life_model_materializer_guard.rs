#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeModelMaterializerCallerKind {
    OrdinaryChatAutoCheckinSourceData,
    GovernedManualOverride,
    SourceDataCompatibilityMaterialization,
    AcceptedProposalApply,
    GovernedRestoreImportOperation,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeModelMaterializerCallerPurpose {
    SourceDataCompatibilityNotAcceptedTruth,
    GovernedManualOverride,
    AcceptedProposalApplySourceSpecificPatchMappingComplete,
    GovernedRestoreImportOperation,
    Unclassified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeModelMaterializerCallerContext {
    pub(crate) stable_id: String,
    pub(crate) kind: LifeModelMaterializerCallerKind,
    pub(crate) purpose: LifeModelMaterializerCallerPurpose,
}

impl LifeModelMaterializerCallerContext {
    pub(crate) fn new(
        stable_id: impl Into<String>,
        kind: LifeModelMaterializerCallerKind,
        purpose: LifeModelMaterializerCallerPurpose,
    ) -> Self {
        Self {
            stable_id: stable_id.into(),
            kind,
            purpose,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeModelMaterializerCallerRestrictionReport {
    pub(crate) stable_id: String,
    pub(crate) write_entrypoint: String,
    pub(crate) allowed: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

pub(crate) fn ensure_lifemodel_materializer_caller_restriction(
    context: &LifeModelMaterializerCallerContext,
    write_entrypoint: &str,
) -> Result<LifeModelMaterializerCallerRestrictionReport, String> {
    let report = evaluate_lifemodel_materializer_caller_restriction(context, write_entrypoint);
    if report.allowed {
        Ok(report)
    } else {
        Err(format!(
            "LifeModel materializer caller restriction blocked for {} via {}: {}",
            context.stable_id,
            write_entrypoint,
            report.blocking_reasons.join(",")
        ))
    }
}

fn evaluate_lifemodel_materializer_caller_restriction(
    context: &LifeModelMaterializerCallerContext,
    write_entrypoint: &str,
) -> LifeModelMaterializerCallerRestrictionReport {
    let mut blocking_reasons = Vec::new();
    if context.stable_id.trim().is_empty() {
        blocking_reasons.push("materializer_caller_context_missing_stable_id".into());
    }
    if context.kind == LifeModelMaterializerCallerKind::Unclassified
        || context.purpose == LifeModelMaterializerCallerPurpose::Unclassified
    {
        blocking_reasons.push("materializer_caller_context_unclassified".into());
    }
    if !caller_pair_allowed(context) {
        blocking_reasons.push("materializer_caller_context_not_allowed".into());
    }

    blocking_reasons.sort();
    blocking_reasons.dedup();

    LifeModelMaterializerCallerRestrictionReport {
        stable_id: context.stable_id.clone(),
        write_entrypoint: write_entrypoint.to_string(),
        allowed: blocking_reasons.is_empty(),
        blocking_reasons,
    }
}

fn caller_pair_allowed(context: &LifeModelMaterializerCallerContext) -> bool {
    matches!(
        (context.kind, context.purpose),
        (
            LifeModelMaterializerCallerKind::GovernedManualOverride,
            LifeModelMaterializerCallerPurpose::GovernedManualOverride,
        ) | (
            LifeModelMaterializerCallerKind::GovernedRestoreImportOperation,
            LifeModelMaterializerCallerPurpose::GovernedRestoreImportOperation,
        ) | (
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization
                | LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
            LifeModelMaterializerCallerPurpose::SourceDataCompatibilityNotAcceptedTruth,
        ) | (
            LifeModelMaterializerCallerKind::AcceptedProposalApply,
            LifeModelMaterializerCallerPurpose::AcceptedProposalApplySourceSpecificPatchMappingComplete,
        )
    )
}
