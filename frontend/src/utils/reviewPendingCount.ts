import type { AgentProposal } from "../tauri";

export const REVIEW_PENDING_PROPOSAL_LIMIT = 100;

export function isPendingReviewProposal(proposal: AgentProposal): boolean {
  return proposal.status === "pending";
}

export function countPendingReviewProposals(proposals: AgentProposal[]): number {
  return proposals.filter(isPendingReviewProposal).length;
}
