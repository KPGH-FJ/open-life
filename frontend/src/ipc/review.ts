import type { AcceptProposalResult, ReviewCenterViewModel, ViewModelEnvelope } from "../tauri";
import { safeInvoke } from "./invoke";

export async function getReviewCenterViewModel(): Promise<
  ViewModelEnvelope<ReviewCenterViewModel>
> {
  return safeInvoke<ViewModelEnvelope<ReviewCenterViewModel>>("get_review_center_view_model");
}

export async function acceptProposal(proposalId: string): Promise<AcceptProposalResult> {
  return safeInvoke("accept_proposal", { proposalId });
}

export async function rejectProposal(proposalId: string): Promise<void> {
  return safeInvoke("reject_proposal", { proposalId });
}

export async function postponeProposal(proposalId: string): Promise<void> {
  return safeInvoke("postpone_proposal", { proposalId });
}
