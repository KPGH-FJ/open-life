export type Stage1DogfoodScenario = {
  id: string;
  scenarioType: "chat_e2e" | "seeded_task_control_e2e";
  prompt: string;
  expectedUiStates: string[];
  expectedFinalSections: string[];
  expectedBlocker?: string;
  selectedSkillId?: string;
};

export const STAGE1_DOGFOOD_SCENARIOS: Stage1DogfoodScenario[] = [
  s(
    "D01",
    "chat_e2e",
    "What is the difference between a task and a proposal in OpenLife?",
    ["answering", "completed"],
    ["completed_work", "next_action"]
  ),
  s(
    "D02",
    "chat_e2e",
    "Summarize `dogfood/project_brief.md`.",
    ["action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D03",
    "chat_e2e",
    "Find what we discussed about memory rollback.",
    ["action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D04",
    "chat_e2e",
    "Use my current working preferences to answer how I should plan tomorrow.",
    ["answering", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D05",
    "chat_e2e",
    "Search the fixture web source about the project policy and summarize it.",
    ["action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D06",
    "chat_e2e",
    "Use the selected review skill to critique this weekly plan.",
    ["planning", "completed"],
    ["completed_work", "observations_used"],
    undefined,
    "phase_e_review"
  ),
  s(
    "D07",
    "chat_e2e",
    "Use the right MCP read source to answer the workspace policy question.",
    ["planning", "action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D08",
    "chat_e2e",
    "Draft a weekly plan and break this goal into steps.",
    ["planning", "action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used", "next_action"]
  ),
  s(
    "D09",
    "seeded_task_control_e2e",
    "Skip unsupported plan step from seeded plan.",
    ["planning", "completed"],
    ["skipped_work", "next_action"]
  ),
  s(
    "D10",
    "chat_e2e",
    "Remember that I prefer morning deep work.",
    ["memory_candidate", "permission_needed"],
    ["proposals_created", "pending_user_action"]
  ),
  s(
    "D11",
    "seeded_task_control_e2e",
    "Accept seeded pending memory proposal.",
    ["memory_candidate", "completed"],
    ["durable_changes", "completed_work"]
  ),
  s(
    "D12",
    "seeded_task_control_e2e",
    "Roll back seeded accepted memory.",
    ["memory_candidate", "completed"],
    ["durable_changes", "completed_work"]
  ),
  s(
    "D13",
    "seeded_task_control_e2e",
    "Resume seeded blocked task after permission.",
    ["retry_available", "completed"],
    ["completed_work", "next_action"]
  ),
  s(
    "D14",
    "seeded_task_control_e2e",
    "Retry seeded failed read action.",
    ["retry_available", "observation_ready"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D15",
    "seeded_task_control_e2e",
    "Cancel seeded non-terminal task.",
    ["blocked", "completed"],
    ["blocked_work", "next_action"]
  ),
  s(
    "D16",
    "chat_e2e",
    "Publish the seeded `dogfood/policy_note.md` to the external destination named in the write-like action seed.",
    ["permission_needed", "blocked"],
    ["blocked_work", "pending_user_action"],
    "permission_required"
  ),
  s(
    "D17",
    "chat_e2e",
    "Use the seeded MCP read source to answer the workspace policy question, then explain why that tool was selected.",
    ["planning", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D18",
    "chat_e2e",
    "Use a skill that is not selected.",
    ["blocked", "completed"],
    ["blocked_work", "next_action"],
    "unselected_skill_not_injected"
  ),
  s(
    "D19",
    "seeded_task_control_e2e",
    "Inspect final delivery for seeded mixed-outcome task.",
    ["completed"],
    ["completed_work", "proposed_work", "blocked_work", "skipped_work", "next_action"]
  ),
  s(
    "D20",
    "seeded_task_control_e2e",
    "Reconnect and replay seeded task events.",
    ["replaying_events", "observation_ready", "completed"],
    ["completed_work", "next_action"]
  ),
  s(
    "D21",
    "chat_e2e",
    "Compare two memory facts that conflict.",
    ["memory_candidate", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D22",
    "chat_e2e",
    "Answer using two different read sources.",
    ["planning", "action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D23",
    "chat_e2e",
    "Use web while network policy blocks it.",
    ["blocked"],
    ["blocked_work", "next_action"],
    "web_network_policy_blocked"
  ),
  s(
    "D24",
    "chat_e2e",
    "Use MCP when no manifest exists.",
    ["blocked"],
    ["blocked_work", "next_action"],
    "mcp_missing_read_target"
  ),
  s(
    "D25",
    "chat_e2e",
    "Inspect loaded knowledge assets.",
    ["completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D26",
    "chat_e2e",
    "Propose an edit to USER.md for my planning preference.",
    ["memory_candidate", "permission_needed"],
    ["proposals_created", "pending_user_action"]
  ),
  s(
    "D27",
    "seeded_task_control_e2e",
    "Recover from stale resume context.",
    ["blocked", "retry_available"],
    ["blocked_work", "next_action"],
    "stale_context"
  ),
  s(
    "D28",
    "seeded_task_control_e2e",
    "Audit what changed in a terminal task.",
    ["completed"],
    [
      "completed_work",
      "proposed_work",
      "blocked_work",
      "skipped_work",
      "durable_changes",
      "next_action",
    ]
  ),
  s(
    "D29",
    "chat_e2e",
    "Ask a simple personal planning question with no required tool.",
    ["answering", "completed"],
    ["completed_work"]
  ),
  s(
    "D30",
    "chat_e2e",
    "Summarize a seeded note and create a memory proposal if useful.",
    ["action_running", "observation_ready", "memory_candidate", "completed"],
    ["completed_work", "proposals_created", "pending_user_action"]
  ),
  s(
    "D31",
    "chat_e2e",
    "Plan the seeded policy-note publication task, but ask me before any risky external publish step.",
    ["planning", "permission_needed", "blocked"],
    ["blocked_work", "pending_user_action"],
    "permission_required"
  ),
  s(
    "D32",
    "chat_e2e",
    "Use the selected skill plus a file read of `dogfood/planning_notes.md` to review the seed plan.",
    ["planning", "action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"],
    undefined,
    "planning_review"
  ),
  s(
    "D33",
    "chat_e2e",
    "Find prior session context, then answer using current memory.",
    ["action_running", "observation_ready", "completed"],
    ["completed_work", "observations_used"]
  ),
  s(
    "D34",
    "chat_e2e",
    "Create a proposal to change SOUL.md wording.",
    ["memory_candidate", "permission_needed"],
    ["proposals_created", "pending_user_action"]
  ),
  s(
    "D35",
    "seeded_task_control_e2e",
    "Deny seeded tool permission proposal.",
    ["blocked", "completed"],
    ["blocked_work", "next_action"]
  ),
  s(
    "D36",
    "seeded_task_control_e2e",
    "Defer seeded memory proposal.",
    ["memory_candidate", "completed"],
    ["proposed_work", "next_action"]
  ),
];

function s(
  id: string,
  scenarioType: Stage1DogfoodScenario["scenarioType"],
  prompt: string,
  expectedUiStates: string[],
  expectedFinalSections: string[],
  expectedBlocker?: string,
  selectedSkillId?: string
): Stage1DogfoodScenario {
  return {
    id,
    scenarioType,
    prompt,
    expectedUiStates,
    expectedFinalSections,
    expectedBlocker,
    selectedSkillId,
  };
}
