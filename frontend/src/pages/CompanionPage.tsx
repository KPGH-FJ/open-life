import { useState } from "react";
import AgentStage, { type AgentStageState } from "../components/AgentStage";
import ChatPage from "./ChatPage";

export default function CompanionPage() {
  const [stageState, setStageState] = useState<AgentStageState>("idle");

  return (
    <section
      data-testid="companion-page"
      aria-label="陪伴主界面"
      className="h-full min-h-0 bg-[#f5f6f2] px-3 py-3 sm:px-4"
    >
      <div className="mx-auto grid h-full min-h-0 max-w-[1500px] grid-rows-[auto_minmax(0,1fr)] gap-3 lg:grid-cols-[340px_minmax(0,1fr)] lg:grid-rows-1">
        <aside className="min-h-0">
          <div className="lg:sticky lg:top-0">
            <AgentStage state={stageState} compact />
          </div>
        </aside>

        <div className="min-h-0 overflow-hidden rounded-lg border border-stone-200 bg-white shadow-sm">
          <ChatPage companionMode onCompanionStageChange={setStageState} />
        </div>
      </div>
    </section>
  );
}
