# OpenLife vNext Architecture Diagrams

Date: 2026-05-06

This document provides development-oriented diagrams for the Agent Framework upgrade. These diagrams are meant to guide implementation and review, not to serve as marketing diagrams.

## 1. Overall Framework Architecture

```mermaid
flowchart TD
    U["User Intent / Proactive Trigger / Scheduled Task"] --> T["AgentTask"]
    T --> R["AgentRun Created"]
    R --> PS["PromptStack"]
    R --> CA["ContextAssembler"]
    CA --> LM["LifeModel Summary"]
    CA --> MEM["Memory Context / Evidence"]
    CA --> WS["Workspace Context"]
    PS --> MR["ModelRouter + PrivacyPolicy"]
    CA --> MR
    MR --> PL["Planner"]
    PL --> AP["AgentPlan"]
    AP --> AR["AgentRuntime / AgentLoop"]
    AR --> TR["ToolRuntime"]
    AR --> SR["SubAgentRuntime"]
    AR --> PE["ProposalEngine"]
    TR --> OBS["ToolObservation"]
    SR --> OBS
    OBS --> EV["AgentRunEvent Trace"]
    PE --> PROP["AgentProposal"]
    PROP --> REVIEW["User Review Center"]
    REVIEW --> APPLY["Apply / Replay / Rollback"]
    APPLY --> LM
    APPLY --> MEM
    APPLY --> AUD["Audit / Snapshot / PatchStore"]
    AR --> OUT["Final Response"]
```

## 2. Single AgentRun Sequence

```mermaid
sequenceDiagram
    participant User
    participant Runtime as AgentRuntime
    participant Prompt as PromptStack
    participant Context as ContextAssembler
    participant Router as ModelRouter
    participant Planner
    participant Tools as ToolRuntime
    participant Proposal as ProposalEngine
    participant Review as ReviewCenter
    participant Stores as Stores/Audit

    User->>Runtime: request
    Runtime->>Stores: create AgentRun
    Runtime->>Prompt: assemble prompt blocks
    Runtime->>Context: assemble LifeModel/Memory/Workspace context
    Context-->>Runtime: context package
    Runtime->>Router: choose model route with privacy policy
    Router-->>Runtime: provider/model/redaction trace
    Runtime->>Planner: produce AgentPlan when needed
    Planner-->>Runtime: structured AgentPlan
    Runtime->>Tools: execute allowed read/tool actions
    Tools-->>Runtime: observations or blocked events
    Runtime->>Proposal: create proposals for side effects
    Proposal-->>Review: pending proposals
    Runtime->>Stores: append AgentRunEvents
    Runtime-->>User: final response + pending proposal summary
    User->>Review: accept/edit/reject
    Review->>Stores: patch/snapshot/audit/replay result
```

## 3. Tool Permission and Execution Model

```mermaid
flowchart TD
    AS["AgentSpec"] --> AT["Allowed Tools"]
    AT --> TC["ToolCall"]
    TC --> TP["ToolPolicy"]
    TP --> P1{"Executable?"}
    P1 -- "No / declarative-only" --> BLK["Blocked + AgentRunEvent"]
    P1 -- "Yes" --> RISK{"Risk Level"}
    RISK --> PERM["PermissionGate"]
    PERM -- "Allowed" --> SAN["ExecutionSandbox"]
    PERM -- "Needs confirmation" --> PR["ToolPermission Proposal"]
    PERM -- "Denied" --> BLK
    SAN --> EX["Executor"]
    EX --> OBS["ToolObservation"]
    OBS --> AUD["AuditEvent"]
    OBS --> EVT["AgentRunEvent"]
```

## 4. PromptStack Assembly

```mermaid
flowchart TD
    SPEC["AgentSpec"] --> BASE["BaseSystemPrompt"]
    SPEC --> SUB["SubAgentPrompt, if applicable"]
    TASK["AgentTask"] --> TP["TaskPrompt"]
    PLAN["Mode"] --> PP["PlanningPrompt / ExecutePrompt"]
    CTX["Context Package"] --> LP["LifeModelPrompt"]
    CTX --> MP["MemoryEvidencePrompt"]
    TOOLS["ToolRegistry"] --> TOOLP["ToolPrompt"]
    POLICY["Policy"] --> PRIV["PrivacyPrompt"]
    POLICY --> PROP["ProposalPrompt"]
    BASE --> STACK["PromptStack"]
    SUB --> STACK
    TP --> STACK
    PP --> STACK
    LP --> STACK
    MP --> STACK
    TOOLP --> STACK
    PRIV --> STACK
    PROP --> STACK
    STACK --> TRACE["PromptBlock IDs/versions in AgentRunEvent"]
    STACK --> MODEL["Model Call"]
```

## 5. Memory to LifeModel Evolution

```mermaid
flowchart TD
    M1["Accepted Memories"] --> EA["Evidence Aggregator"]
    M2["VectorStore Hits"] --> EA
    M3["Feedback Signals"] --> EA
    M4["Rejected Proposal History"] --> EA
    EA --> PAT["Pattern / Trend Detection"]
    PAT --> CONTRA["Contradiction Detection"]
    CONTRA --> IMP["LifeModel Impact Analysis"]
    IMP --> RISK["Field Risk Classifier"]
    RISK --> EP["Evolution Proposal"]
    EP --> REVIEW["User Review"]
    REVIEW -- "Accept/Edit" --> PATCH["LifeModel Patch"]
    REVIEW -- "Reject" --> LEARN["Evolution Feedback"]
    PATCH --> SNAP["Snapshot + Audit"]
    SNAP --> LM["Updated LifeModel"]
    LM --> FUTURE["Future AgentRun Context"]
```

## 6. Sub-Agent Orchestration

```mermaid
flowchart TD
    MAIN["Main Agent"] --> DECIDE["Delegation Decision"]
    DECIDE --> MODE{"Delegation Mode"}
    MODE -- "call_as_tool" --> SPEC1["Specialist AgentSpec"]
    MODE -- "handoff" --> SPEC2["Handoff AgentSpec"]
    MODE -- "parallel" --> SPEC3["Worker AgentSpecs"]
    MODE -- "review" --> SPEC4["Reviewer AgentSpec"]
    SPEC1 --> CTX1["Isolated Context Policy"]
    SPEC2 --> CTX2["Isolated Context Policy"]
    SPEC3 --> CTX3["Isolated Context Policy"]
    SPEC4 --> CTX4["Isolated Context Policy"]
    CTX1 --> TOOLS["Role-Specific ToolPolicy"]
    CTX2 --> TOOLS
    CTX3 --> TOOLS
    CTX4 --> TOOLS
    TOOLS --> RUN["Child AgentRun / Event Link"]
    RUN --> RESULT["DelegationResult"]
    RESULT --> MAIN
```

## 7. Development Dependency Order

```mermaid
flowchart LR
    A["Runtime Audit"] --> B["Architecture Baseline"]
    B --> C["Execution Path Convergence"]
    C --> D["AgentRunEvent"]
    D --> E["ToolRuntime Hardening"]
    E --> F["PromptStack"]
    F --> G["MemoryEvidence + Evolution"]
    G --> H["AgentSpec + PlanMode"]
    H --> I["SubAgentRuntime"]
    I --> J["Compaction"]
    J --> K["Bash / Sandbox"]
    K --> L["Frontend Agent Workspace"]
```
