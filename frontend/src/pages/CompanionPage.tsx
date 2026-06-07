import { useState } from "react";
import { Link } from "react-router-dom";
import { CalendarDays, HeartHandshake, Inbox } from "lucide-react";
import AgentStage, { type AgentStageState } from "../components/AgentStage";
import ChatPage from "./ChatPage";

export default function CompanionPage() {
  const [stageState, setStageState] = useState<AgentStageState>("idle");

  return (
    <section
      data-testid="companion-page"
      aria-label="陪伴主界面"
      className="h-full min-h-0 overflow-hidden bg-[#f5f6f2] px-3 py-3 sm:px-4"
    >
      <div className="mx-auto flex h-full min-h-0 w-full max-w-[1500px] flex-col gap-3">
        <header className="flex shrink-0 flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <HeartHandshake size={20} aria-hidden="true" className="text-stone-700" />
              <h1 className="text-xl font-semibold tracking-normal text-stone-950">陪伴</h1>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <span className="inline-flex h-7 items-center rounded-md border border-stone-200 bg-white px-2.5 text-xs font-medium text-stone-700">
                普通 Chat
              </span>
              <span className="inline-flex h-7 items-center rounded-md border border-stone-200 bg-white px-2.5 text-xs font-medium text-stone-700">
                legacy_stream
              </span>
              <span className="inline-flex h-7 items-center rounded-md border border-stone-200 bg-white px-2.5 text-xs font-medium text-stone-700">
                Proposal-first
              </span>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Link
              to="/today"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20"
            >
              今日
              <CalendarDays size={15} aria-hidden="true" />
            </Link>
            <Link
              to="/mailbox"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20"
            >
              邮箱
              <Inbox size={15} aria-hidden="true" />
            </Link>
          </div>
        </header>

        <div className="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)] gap-3 lg:grid-cols-[340px_minmax(0,1fr)] lg:grid-rows-1">
          <aside className="min-h-0">
            <div className="lg:sticky lg:top-0">
              <AgentStage state={stageState} compact />
            </div>
          </aside>

          <div className="min-h-0 overflow-hidden rounded-lg border border-stone-200 bg-white shadow-sm">
            <ChatPage companionMode onCompanionStageChange={setStageState} />
          </div>
        </div>
      </div>
    </section>
  );
}
