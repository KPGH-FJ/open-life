import AgentStage from "../components/AgentStage";
import ChatPage from "./ChatPage";

export default function CompanionPage() {
  return (
    <section
      data-testid="companion-page"
      aria-label="陪伴主界面"
      className="h-full min-h-0 overflow-hidden bg-[#f4f3ee] px-4 py-5 sm:px-7"
    >
      <div className="mx-auto grid h-full min-h-0 w-full max-w-[1500px] grid-rows-[auto_minmax(0,1fr)] gap-5 lg:grid-cols-[minmax(360px,42%)_minmax(560px,58%)] lg:grid-rows-1">
        <aside className="min-h-0">
          <AgentStage state="idle" compact />
        </aside>

        <div className="min-h-0 overflow-hidden rounded-xl border border-stone-200 bg-[#fffefa] shadow-sm">
          <ChatPage companionMode />
        </div>
      </div>
    </section>
  );
}
