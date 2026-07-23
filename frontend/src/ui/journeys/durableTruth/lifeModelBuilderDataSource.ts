import {
  builderCreateProposals,
  builderListUnfinished,
  builderStart,
  builderStep,
  type BuilderSignalDecision,
  type BuilderTurnResponse,
  type UnfinishedBuilderSession,
} from "@/tauri";

export type BuilderProposalReceipt = Awaited<ReturnType<typeof builderCreateProposals>>;

export interface LifeModelBuilderDataSource {
  listUnfinished(): Promise<UnfinishedBuilderSession[]>;
  startQuick(sessionId: string): Promise<BuilderTurnResponse>;
  resume(session: UnfinishedBuilderSession): Promise<BuilderTurnResponse>;
  answer(sessionId: string, answer: string): Promise<BuilderTurnResponse>;
  createProposals(
    sessionId: string,
    decisions: BuilderSignalDecision[]
  ): Promise<BuilderProposalReceipt>;
}

function builderMode(mode: UnfinishedBuilderSession["mode"]): "quick" | "incremental" | "socratic" {
  if (mode === "Incremental") return "incremental";
  if (mode === "Socratic") return "socratic";
  return "quick";
}

export const tauriLifeModelBuilderDataSource: LifeModelBuilderDataSource = {
  listUnfinished: builderListUnfinished,
  startQuick: sessionId => builderStart("quick", sessionId),
  resume: session =>
    builderStart(
      builderMode(session.mode),
      session.session_id,
      session.target_dimension?.toLowerCase() as
        | "identity"
        | "goals"
        | "capabilities"
        | "state"
        | undefined
    ),
  answer: builderStep,
  createProposals: builderCreateProposals,
};
