import { vi } from "vitest";
import type {
  LifeModel,
  ChatMessage,
  DailyGoal,
  StateHistoryEntry,
  StateAlert,
  LifeModelVersion,
  AgentRunEvent,
  AgentSpec,
} from "@/types";

export const mockLifeModel: LifeModel = {
  metadata: {
    version: "0.1.0",
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    author: "test",
  },
  identity: {
    name: "测试用户",
    values: [
      { name: "健康", weight: 0.8, description: "保持身体健康" },
      { name: "学习", weight: 0.7, description: "持续学习成长" },
    ],
    personality_traits: [
      { trait_name: "外向", score: 0.8 },
      { trait_name: "乐观", score: 0.9 },
    ],
    life_philosophy: "活在当下",
    mission_statement: "成为更好的自己",
    role_definition: {
      primary_role: "开发者",
      secondary_roles: ["家庭成员"],
      responsibilities: ["工作", "家庭"],
      boundaries: ["不加班"],
    },
    voice_style: {
      formality: "casual",
      tone_descriptors: ["友好"],
      vocabulary_preference: "简洁",
      emoji_usage: "often",
    },
  },
  goals: {
    short_term: [
      {
        name: "完成项目",
        priority: 1,
        status: "in_progress",
        milestones: [
          {
            name: "设计阶段",
            target_date: "2024-12-01",
            status: "completed",
            description: "完成设计",
          },
        ],
        description: "完成当前项目",
        progress: 0.5,
        related_memories: [],
      },
    ],
    medium_term: [],
    long_term: [],
    life_goals: [],
    daily: [{ name: "早起", done: false, time_block: { start: "07:00", end: "08:00" } }],
    progress: 0.5,
    related_memories: [],
  },
  capabilities: {
    skills: [
      { name: "编程", proficiency: 0.8, description: "软件开发" },
      { name: "写作", proficiency: 0.6, description: "技术写作" },
    ],
    resources: [
      { name: "MacBook", resource_type: "设备", description: "工作电脑", availability: "" },
    ],
    networks: [],
    tools: [],
    knowledge_domains: [{ domain: "AI", level: 7, description: "人工智能领域" }],
  },
  state: {
    current_focus: "工作",
    health_status: { physical: "良好", mental: "稳定", energy_level: 8 },
    emotional_state: { current_mood: "happy", stress_level: 2, fulfillment_score: 8 },
    recent_reflections: [],
    open_questions: [],
    focus_areas: ["工作", "学习"],
    recent_events: [],
    habit_streaks: [{ name: "阅读", streak_days: 5 }],
    custom_dimensions: [{ name: "专注度", unit: "%", current_value: 75, alert_days: 3 }],
    alerts: [],
  },
  relationships: { inner_circle: [], mentors: [], collaborators: [] },
  preferences: {
    work_hours: { preferred_start: "09:00", preferred_end: "18:00", timezone: "Asia/Shanghai" },
    peak_energy_time: "",
    communication_style: "",
    learning_style: "",
    decision_making_style: "",
  },
  evolution_rules: [],
};

export function createEmptyLifeModel(): LifeModel {
  return {
    metadata: {
      version: "0.1.0",
      created_at: "",
      updated_at: "",
      author: "",
    },
    identity: {
      name: "",
      birth_date: undefined,
      values: [],
      personality_traits: [],
      life_philosophy: "",
      mission_statement: "",
      role_definition: {
        primary_role: "",
        secondary_roles: [],
        responsibilities: [],
        boundaries: [],
      },
      voice_style: {
        formality: "neutral",
        tone_descriptors: [],
        vocabulary_preference: "",
        emoji_usage: "never",
      },
    },
    goals: {
      short_term: [],
      medium_term: [],
      long_term: [],
      life_goals: [],
      daily: [],
      progress: 0,
      related_memories: [],
    },
    capabilities: {
      skills: [],
      resources: [],
      networks: [],
      tools: [],
      knowledge_domains: [],
    },
    state: {
      current_focus: "构建人生模型",
      health_status: {
        physical: "良好",
        mental: "积极",
        energy_level: 7,
      },
      emotional_state: {
        current_mood: "期待",
        stress_level: 3,
        fulfillment_score: 6,
      },
      recent_reflections: [],
      open_questions: [],
      focus_areas: [],
      recent_events: [],
      habit_streaks: [],
      custom_dimensions: [],
      alerts: [],
    },
    relationships: {
      inner_circle: [],
      mentors: [],
      collaborators: [],
    },
    preferences: {
      work_hours: {
        preferred_start: "",
        preferred_end: "",
        timezone: "",
      },
      peak_energy_time: "",
      communication_style: "",
      learning_style: "",
      decision_making_style: "",
    },
    evolution_rules: [],
  };
}

export const mockDailyGoals: DailyGoal[] = [
  { name: "早起", done: false, time_block: { start: "07:00", end: "08:00" } },
  { name: "运动", done: true },
];

export const mockStateAlerts: StateAlert[] = [
  {
    dimension_name: "专注度",
    level: "warning",
    message: "专注度低于阈值",
    triggered_at: new Date().toISOString(),
  },
];

export const mockStateHistory: StateHistoryEntry[] = [
  {
    id: 1,
    dimension_name: "专注度",
    value: 70,
    unit: "%",
    note: "上午工作",
    recorded_at: new Date().toISOString(),
  },
  {
    id: 2,
    dimension_name: "专注度",
    value: 75,
    unit: "%",
    note: "下午工作",
    recorded_at: new Date(Date.now() - 86400000).toISOString(),
  },
];

export const mockChatSessions: Array<{ session_id: string; title: string; updated_at: string }> = [
  { session_id: "session-1", title: "会话 1", updated_at: new Date().toISOString() },
  {
    session_id: "session-2",
    title: "会话 2",
    updated_at: new Date(Date.now() - 3600000).toISOString(),
  },
];

export const mockChatMessages: ChatMessage[] = [
  { role: "user", content: "你好" },
  { role: "assistant", content: "你好！我是 OpenLife。" },
];

export const mockLifeModelVersions: LifeModelVersion[] = [
  {
    version: "0.1.0",
    timestamp: new Date().toISOString(),
    tag: "auto",
    note: "自动保存",
    yaml_content: "",
  },
  {
    version: "0.2.0",
    timestamp: new Date(Date.now() - 3600000).toISOString(),
    tag: "manual",
    note: "手动调整目标与状态",
    yaml_content: "",
  },
];

export const mockInvoke = vi.fn(<T>(cmd: string, _args?: Record<string, any>): Promise<T> => {
  switch (cmd) {
    case "get_config":
      return Promise.resolve({
        llm: {
          provider: "deepseek",
          openai_base: "https://api.deepseek.com",
          openai_key: "",
          embedding_model: "text-embedding-3-small",
          chat_model: "deepseek-chat",
          embedding_enabled: false,
        },
        prefer_local_model: false,
        local_model: "llama3",
      } as T);
    case "get_life_model":
      return Promise.resolve(mockLifeModel as T);
    case "get_daily_goals":
      return Promise.resolve(mockDailyGoals as T);
    case "get_state_alerts":
      return Promise.resolve(mockStateAlerts as T);
    case "get_state_history":
      return Promise.resolve(mockStateHistory as T);
    case "list_chat_sessions":
      return Promise.resolve(mockChatSessions as T);
    case "list_mcp_servers":
      return Promise.resolve([
        {
          name: "filesystem",
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
          tool_count: 2,
        },
      ] as T);
    case "list_mcp_audit_logs":
      return Promise.resolve([
        {
          id: 1,
          tool_name: "write_file",
          arguments: '{"path":"/tmp/demo.txt","content":"hello"}',
          result: "工具执行成功",
          success: true,
          pii_found: true,
          created_at: new Date(Date.now() - 60000).toISOString(),
        },
      ] as T);
    case "clear_mcp_audit_logs":
      return Promise.resolve(3 as T);
    case "list_mcp_tools":
      return Promise.resolve([
        { name: "read_file", description: "读取文件内容" },
        { name: "write_file", description: "写入文件内容" },
      ] as T);
    case "list_mcp_templates":
      return Promise.resolve([
        {
          id: "filesystem",
          name: "本地文件系统",
          description: "读取和写入本地文件",
          command: "npx",
          args: ["-y", "@modelcontextprotocol/server-filesystem", "{{rootPath}}"],
          required_args: ["rootPath"],
          arg_labels: { rootPath: "允许访问的根目录路径" },
          tags: ["file", "filesystem", "local"],
        },
      ] as T);
    case "recommend_mcp_manifests":
      return Promise.resolve([
        {
          name: "filesystem",
          description: "适合当前阶段进行本地文件读写",
          parameters: {},
          permission_level: "high",
          version: "1.0.0",
          source: { type: "Mcp", server_name: "filesystem" },
          tags: ["file", "filesystem", "local"],
        },
      ] as T);
    case "list_tool_manifests":
      return Promise.resolve([
        {
          id: "file.read",
          name: "file.read",
          description: "读取文件",
          parameters: {},
          permission_level: "low",
          risk_level: "low",
          version: "1.0.0",
          source: { type: "BuiltIn" },
          capabilities: ["read", "filesystem"],
          requires_confirmation: false,
          enabled: true,
          declarative_only: false,
          action_type: "read",
          tags: ["execution"],
        },
        {
          id: "web.search",
          name: "web.search",
          description: "搜索网页",
          parameters: {},
          permission_level: "low",
          risk_level: "low",
          version: "1.0.0",
          source: { type: "BuiltIn" },
          capabilities: ["network"],
          requires_confirmation: false,
          enabled: true,
          declarative_only: false,
          action_type: "read",
          tags: ["execution", "web"],
        },
      ] as T);
    case "get_chat_history":
      return Promise.resolve(mockChatMessages as T);
    case "list_snapshots":
      return Promise.resolve(mockLifeModelVersions as T);
    case "get_agent_run":
      return Promise.resolve(null as T);
    case "list_agent_runs":
      return Promise.resolve([] as T);
    case "list_agent_runs_for_session":
      return Promise.resolve([] as T);
    case "get_last_model_error":
      return Promise.resolve(null as T);
    case "get_pending_proposals":
      return Promise.resolve([] as T);
    case "list_proposals":
      return Promise.resolve([] as T);
    case "batch_accept_low_risk_proposals":
      return Promise.resolve(0 as T);
    case "accept_proposal":
    case "reject_proposal":
    case "edit_proposal":
    case "postpone_proposal":
      return Promise.resolve(undefined as T);
    case "list_tool_permissions":
      return Promise.resolve([
        {
          id: "perm-1",
          toolName: "builtin_echo",
          source: "builtin",
          riskLevel: "low",
          actionType: "mcp_tool_call",
          policy: "allow_until_revoked",
          createdAt: new Date().toISOString(),
        },
      ] as T);
    case "revoke_tool_permission":
      return Promise.resolve(true as T);
    case "check_tool_permission":
      return Promise.resolve({
        allowed: true,
        requiresConfirmation: false,
        decision: "allow",
        reason: "mock allow",
      } as T);
    case "list_skills":
      return Promise.resolve([
        {
          id: "weekly_review",
          name: "Weekly Review",
          description: "汇总近期 AgentRun、目标、状态和记忆。",
          requiredContext: ["agent_runs", "goals", "state", "memory"],
          allowedTools: [],
          executionBudget: {
            maxSteps: 5,
            maxToolCalls: 3,
            timeoutSeconds: 60,
            allowCloud: true,
            allowWrites: false,
          },
          outputSchema: {},
          proposalPolicy: "review_required",
        },
      ] as T);
    case "run_skill":
      return Promise.resolve({
        runId: "run-skill-1",
        status: "completed",
        summary: "Skill completed",
        generatedProposals: ["proposal-skill-1"],
      } as T);
    case "get_skill_run_status":
      return Promise.resolve(null as T);
    case "list_plugins":
    case "reload_plugins":
      return Promise.resolve([
        {
          manifest: {
            id: "local-demo",
            name: "Local Demo",
            version: "0.1.0",
            description: "本地插件示例",
            author: "OpenLife",
            tools: [],
            skills: [],
            permissions: ["read"],
            enabled: false,
            trustLevel: "local",
          },
          path: "/tmp/openlife/plugins/local-demo/plugin.json",
          enabled: false,
        },
      ] as T);
    case "enable_plugin":
    case "disable_plugin":
      return Promise.resolve(undefined as T);
    case "diff_snapshots":
      return Promise.resolve(
        [
          "-identity:",
          "-  mission_statement: 成为更好的自己",
          "+identity:",
          "+  mission_statement: 成为更稳定的自己",
          "-goals:",
          "+goals:",
          "+  daily:",
          "+    - name: 复盘",
          "-state:",
          "+state:",
          "+  current_focus: 深度工作",
        ].join("\n") as T
      );
    case "create_snapshot":
      return Promise.resolve(mockLifeModelVersions[0] as T);
    case "restore_snapshot":
      return Promise.resolve(mockLifeModel as T);
    case "goal_capability_gap_analysis":
      return Promise.resolve(["需要提升编程技能", "需要更多学习时间"] as T);
    case "goal_capability_gap_report":
      return Promise.resolve([
        {
          goal_name: "完成 AI 项目",
          skill_name: "编程",
          current_level: 4,
          target_level: 7,
          severity: "high",
          suggestion: "安排 2 周刻意练习，并补一个可验证里程碑",
        },
      ] as T);
    case "identity_goal_alignment_check":
      return Promise.resolve([] as T);
    case "identity_goal_alignment_report":
      return Promise.resolve([] as T);
    case "count_memory_chunks":
      return Promise.resolve(42 as T);
    case "rebuild_memory_index":
      return Promise.resolve({
        processed: 12,
        indexed: 10,
        skipped: 2,
      } as T);
    case "get_feedback_summary":
      return Promise.resolve({
        total_messages: 100,
        total_feedback_up: 80,
        total_feedback_down: 5,
        session_count: 10,
      } as T);
    case "should_show_calibration":
      return Promise.resolve({ weekly: false, monthly: false, today: "2026-04-17" } as T);
    case "check_ollama_status":
      return Promise.resolve(true as T);
    case "get_router_status":
      return Promise.resolve({
        onnx_available: false,
        onnx_disabled: false,
        active_backend: "regex",
        latency_threshold_us: 50000,
      } as T);
    case "get_model_router_status":
      return Promise.resolve({
        enabled: false,
        providers: [
          { name: "ollama", enabled: true, available: false, healthIsEstimated: true },
          { name: "openai", enabled: true, available: true, healthIsEstimated: true },
        ],
        lastCheckAt: new Date().toISOString(),
      } as T);
    case "replay_agent_action":
      return Promise.resolve({
        id: "action-replay-1",
        actionType: "mcp_tool_call",
        target: "test_tool",
        input: {},
        status: "succeeded",
        permissionDecision: "allow_once",
        toolScope: {
          toolId: "test_tool",
          toolName: "test_tool",
          source: "builtin",
          riskLevel: "low",
          capabilities: [],
          actionType: "mcp_tool_call",
        },
        startedAt: new Date().toISOString(),
        finishedAt: new Date().toISOString(),
        timestamp: new Date().toISOString(),
      } as T);
    case "get_system_diagnostics":
      return Promise.resolve({
        router: {
          onnx_available: false,
          onnx_disabled: false,
          active_backend: "regex",
          latency_threshold_us: 50000,
        },
        mcp_server_count: 1,
        mcp_tool_count: 2,
        mcp_recent_audit_count: 1,
        mcp_recent_pii_count: 1,
        memory_chunk_count: 42,
        vector_corrupt_embedding_count: 0,
        unfinished_builder_sessions: 0,
        pending_builder_review_sessions: 0,
        ollama_online: true,
        local_model: "llama3",
        resolved_local_model: "llama3:latest",
        prefer_local_model: true,
        cloud_api_configured: false,
        cloud_provider: "DeepSeek",
        cloud_api_validated: false,
        cloud_api_last_error: null,
        chat_ready: true,
        readiness_issues: [],
        data_dir: "/tmp/openlife-test",
        active_data_dir: "/tmp/openlife-test",
        legacy_data_dir: "/tmp/openlife-legacy",
        database_status: "ok",
        startup_warnings: [],
        snapshot_count: 2,
        life_model_ready: true,
        app_version: "0.1.0",
        model_empty: false,
        chat_session_count: 3,
        onboarding_completed: true,
        beta_ready: true,
        beta_readiness_issues: [],
        builder_completion: {
          identity: 80,
          goals: 75,
          capabilities: 70,
          state: 65,
          overall: 72.5,
          lowest_dimension: "state",
        },
        data_files: {
          messages_db_exists: true,
          messages_db_size_mb: 1.2,
          vectors_db_exists: true,
          vectors_db_size_mb: 0.8,
          mcp_audit_db_exists: true,
          mcp_audit_db_size_mb: 0.1,
          config_yaml_exists: true,
          life_model_yaml_exists: true,
        },
        ollama_models: [
          { name: "llama3", size_mb: 4500 },
          { name: "qwen2.5", size_mb: 3200 },
        ],
        config_source: "env+default",
      } as T);
    case "get_scheduler_config":
      return Promise.resolve({ localModel: "llama3", preferLocal: true } as T);
    case "save_chat_message":
      return Promise.resolve(undefined as T);
    case "register_mcp_server":
    case "unregister_mcp_server":
      return Promise.resolve(undefined as T);
    case "execute_tool_call":
      return Promise.resolve({
        name: _args?.name,
        arguments: _args?.arguments ?? {},
        sanitized_arguments: _args?.arguments ?? {},
        success: true,
        output: "工具执行成功",
        permission_level: "high",
        status: "success",
        requires_confirmation: false,
        pii_found: false,
        privacy_warnings: [],
      } as T);
    case "inspect_mcp_call":
      return Promise.resolve({
        permission_level: "medium",
        pii_found: true,
        findings: [{ path: "$.query", privacy_type: "Email", matched: "test@example.com" }],
        sanitized_arguments: { query: "帮我搜索 <EMAIL_0> 的公开信息" },
        requires_confirmation: true,
      } as T);
    case "get_model_4d_completion":
      return Promise.resolve({
        identity: 0.7,
        goals: 0.6,
        capabilities: 0.5,
        state: 0.8,
      } as T);
    case "builder_list_unfinished":
      return Promise.resolve([] as T);
    case "builder_start":
      return Promise.resolve({
        prompt: "请描述你的价值观",
        progress: { progress: 0.2, current_step_label: "价值观", step_index: 1, total_steps: 5 },
      } as T);
    case "builder_step":
      return Promise.resolve({
        prompt: "下一步问题",
        finished: false,
        progress: {
          progress: 0.4,
          current_step_label: "目标",
          step_index: 2,
          total_steps: 5,
          waiting_phase_confirmation: false,
        },
        mode: "Quick",
        pending_signals: [],
      } as T);
    case "builder_get_pending_signals":
      return Promise.resolve({
        session_id: "test-session",
        signals: [],
        summary: {
          identity_summary: "基于 0 个信号",
          goals_summary: "基于 0 个信号",
          capabilities_summary: "基于 0 个信号",
          state_summary: "基于 0 个信号",
          assumptions: ["用户通过快速构建流程提供"],
          unresolved_questions: [],
          recommended_next_steps: ["审阅并确认信号", "可选择进入渐进构建继续完善"],
        },
        finished: true,
      } as T);
    case "builder_apply_signals":
      return Promise.resolve({
        success: true,
        applied_fields: [],
        merged_fields: [],
        skipped_fields: [],
        edited_count: 0,
        rejected_count: 0,
        model: null,
      } as T);
    case "builder_create_proposals":
      return Promise.resolve({
        success: true,
        created_count: 1,
        rejected_count: 0,
        proposal_ids: ["proposal-1"],
        run_id: "run-1",
        warnings: [],
      } as T);
    case "add_daily_goal":
      return Promise.resolve(undefined as T);
    case "toggle_daily_goal":
      return Promise.resolve(true as T);
    case "delete_daily_goal":
    case "update_daily_goal":
      return Promise.resolve(undefined as T);
    case "record_state":
      return Promise.resolve(undefined as T);
    case "search_memory":
      return Promise.resolve([] as T);
    case "a2a_local_agent_card":
      return Promise.resolve({
        name: "OpenLife Local Agent",
        description: "本地 A2A 服务",
        version: "0.1.0",
        skills: [
          {
            id: "openlife.reasoning_bridge",
            name: "推理桥接",
            description: "桥接 OpenLife 和 A2A",
          },
        ],
      } as T);
    case "a2a_discover_agent":
      return Promise.resolve({
        name: "Remote Agent",
        description: "外部代理",
        version: "0.1.0",
        url: "http://127.0.0.1:8080",
        capabilities: { streaming: false },
        skills: [{ id: "demo", name: "Demo Skill", description: "测试技能" }],
      } as T);
    case "a2a_send_task":
    case "a2a_handle_task":
      return Promise.resolve('{"status":"ok"}' as T);
    case "a2a_bridge_local":
      return Promise.resolve({
        request: { method: _args?.method, params: { text: _args?.text } },
        a2a_request: { message: { parts: [{ type: "text", text: _args?.text }] } },
        response: { status: { state: "completed" } },
        reasoning_result: { text: "桥接成功" },
      } as T);
    case "a2a_restart_sidecar":
    case "a2a_stop_sidecar":
      return Promise.resolve(undefined as T);
    case "run_micro_evolution":
      return Promise.resolve({
        changes: [],
        applied: false,
        message: "近7天暂无足够信号来微调模型权重",
        snapshot_version: null,
      } as T);
    case "generate_micro_evolution_changes":
      return Promise.resolve({
        changes: [
          {
            dimension: "identity.values",
            target_name: "健康",
            old_value: 8,
            new_value: 8.03,
            reason: "近期正向行为信号增加",
            confidence: 0.82,
            sources: [
              { source: "feedback", score: 0.03, weight: 0.5 },
              { source: "behavior", score: 0.02, weight: 0.3 },
              { source: "inference", score: 0.01, weight: 0.2 },
            ],
          },
        ],
        applied: true,
        message: "已生成 1 项建议",
        before: { identity: 70, goals: 60, capabilities: 50, state: 80, overall: 65 },
        after: { identity: 71, goals: 60, capabilities: 50, state: 80, overall: 65 },
        requires_confirmation: true,
        signal_summary: {
          feedback_terms: 2,
          behavior_events: 1,
          inference_items: 1,
          top_feedback: [{ name: "健康", score: 0.03, source: "feedback" }],
          top_behavior: [{ name: "value_focus:健康", score: 0.02, source: "behavior" }],
          top_inference: [{ name: "identity.values:健康", score: 0.01, source: "inference" }],
        },
      } as T);
    case "apply_calibration":
      return Promise.resolve({
        success: true,
        snapshot_version: "v1-test",
        applied_count: 2,
        message: "已应用校准",
      } as T);
    case "calibration_create_proposals":
      return Promise.resolve({
        created_count: 2,
        created_ids: ["p1", "p2"],
        error_count: 0,
        errors: [],
        message: "已创建 2 个 Proposal",
      } as T);
    // Milestone D mocks
    case "get_hot_cache":
      return Promise.resolve({
        identity_summary: "你是测试用户，成为更好的自己。你的核心哲学是：活在当下。",
        top_values: ["健康 (保持身体健康)", "学习 (持续学习成长)"],
        current_goals: ["完成项目 (优先级: 1, 进度: 50%)", "○ 每日目标: 早起"],
        recent_state: "心情: happy，当前专注: 工作",
        last_refreshed: new Date().toISOString(),
        life_model_version: "",
      } as T);
    case "archive_low_access_memories":
      return Promise.resolve(0 as T);
    case "restore_archived_chunks":
      return Promise.resolve((_args?.chunk_ids ?? []).length as T);
    case "list_archived_chunks":
      return Promise.resolve([] as T);
    case "get_memory_tier_stats":
      return Promise.resolve({ total: 0, tier1: 0, tier2: 0, tier3: 0, archived: 0 } as T);
    case "export_mcp_audit_logs":
      return Promise.resolve({
        exported_at: new Date().toISOString(),
        entry_count: 0,
        days: _args?.days ?? 7,
        entries: [],
      } as T);
    case "cleanup_mcp_audit_logs":
      return Promise.resolve(0 as T);
    case "rotate_mcp_audit_key":
      return Promise.resolve(undefined as T);
    case "get_privacy_policy":
      return Promise.resolve({
        enabled: true,
        rules: [
          { ptype: "Phone", enabled: true, action: "Mask", custom_pattern: undefined },
          { ptype: "IdCard", enabled: true, action: "Block", custom_pattern: undefined },
          { ptype: "Email", enabled: true, action: "Mask", custom_pattern: undefined },
        ],
      } as T);
    case "export_all_data":
      return Promise.resolve({
        version: "2.0",
        app_version: "0.1.0",
        exported_at: new Date().toISOString(),
        life_model: {},
        messages: [],
        vectors: [],
      } as T);
    case "test_llm_connection":
      return Promise.resolve({
        ok: true,
        provider: _args?.config?.llm?.provider === "deepseek" ? "DeepSeek" : "OpenAI-compatible",
        message: "连接成功",
      } as T);
    case "set_privacy_policy":
      return Promise.resolve(undefined as T);
    case "has_completed_onboarding":
      return Promise.resolve(false as T);
    case "mark_onboarding_completed":
      return Promise.resolve(undefined as T);
    case "list_agent_run_events":
      return Promise.resolve(mockAgentRunEvents as T);
    case "get_agent_plan":
      return Promise.resolve(mockAgentPlan as T);
    case "list_agent_plans_for_run":
      return Promise.resolve([mockAgentPlan] as T);
    case "list_agent_plans_for_session":
      return Promise.resolve([mockAgentPlan] as T);
    case "confirm_agent_plan":
      return Promise.resolve({
        planId: "mock-plan-1",
        operation: "confirm",
        success: true,
        status: "confirmed",
        deviations: [],
      } as T);
    case "reject_agent_plan":
      return Promise.resolve({
        planId: "mock-plan-1",
        operation: "reject",
        success: true,
        status: "rejected",
        deviations: [],
      } as T);
    case "execute_agent_plan":
      return Promise.resolve({
        planId: "mock-plan-1",
        runId: "run-001",
        operation: "execute",
        success: true,
        status: "completed",
        stepsCompleted: 1,
        stepsFailed: 0,
        deviations: [],
        reviewVerdict: "approved",
      } as T);
    case "cancel_agent_plan":
      return Promise.resolve({
        planId: "mock-plan-1",
        operation: "cancel",
        success: true,
        status: "cancelled",
        deviations: [],
      } as T);
    case "retry_agent_plan":
      return Promise.resolve({
        planId: "mock-plan-1",
        operation: "retry",
        success: true,
        status: "executing",
        deviations: [],
      } as T);
    case "continue_agent_plan":
      return Promise.resolve({
        planId: "mock-plan-1",
        operation: "continue",
        success: true,
        status: "confirmed",
        deviations: [],
        message: "no blocked actions to continue",
      } as T);
    // ── AgentSpec mocks ──────────────────────────────────────────────
    case "get_agent_spec": {
      const id = (_args as any)?.specId || (_args as any)?.spec_id;
      if (id === "main.default") {
        return Promise.resolve(mockAgentSpec as T);
      }
      return Promise.resolve(null as T);
    }
    case "list_agent_specs":
      return Promise.resolve([mockAgentSpec] as T);
    case "get_default_agent_spec":
      return Promise.resolve(mockAgentSpec as T);
    case "update_agent_spec":
      return Promise.resolve(undefined as T);
    case "set_default_agent_spec":
      return Promise.resolve(undefined as T);
    default:
      return Promise.resolve({} as T);
  }
});

// ── AgentRunEvent mock data for timeline contract ─────────────────────

export const mockAgentRunEvents: AgentRunEvent[] = [
  {
    id: "evt-run-created-001",
    runId: "run-001",
    eventType: "run.created",
    actor: "runtime",
    summary: "Agent run created",
    payload: { session_id: "sess-1", kind: "conversation" },
    createdAt: "2026-05-06T10:00:00Z",
  },
  {
    id: "evt-context-001",
    runId: "run-001",
    eventType: "context.assembled",
    actor: "agent",
    summary: "Context assembled with 3 memory hits",
    payload: { memory_hit_count: 3, life_model_sections: ["identity", "goals"] },
    createdAt: "2026-05-06T10:00:01Z",
  },
  {
    id: "evt-model-started-001",
    runId: "run-001",
    eventType: "model.call_started",
    actor: "agent",
    phase: "reasoning",
    summary: "Calling deepseek-chat for step 1",
    payload: { provider: "deepseek", model: "deepseek-chat", step: 1 },
    createdAt: "2026-05-06T10:00:02Z",
  },
  {
    id: "evt-model-completed-001",
    runId: "run-001",
    eventType: "model.call_completed",
    actor: "agent",
    phase: "reasoning",
    summary: "Model call completed (1234ms)",
    payload: { latency_ms: 1234, reply_len: 256 },
    createdAt: "2026-05-06T10:00:03Z",
  },
  {
    id: "evt-tool-blocked-001",
    runId: "run-001",
    eventType: "tool.call_blocked",
    actor: "runtime",
    summary: "email.read blocked: declarative-only stub",
    payload: { tool: "email.read", reason: "declarative-only" },
    createdAt: "2026-05-06T10:00:04Z",
  },
  {
    id: "evt-plan-created-001",
    runId: "run-001",
    eventType: "plan.created",
    actor: "agent",
    summary: "Plan created: Analyze project structure",
    payload: {
      plan_id: "plan-001",
      goal: "Analyze project structure",
      risk_level: "low",
      requires_confirmation: false,
      step_count: 3,
    },
    createdAt: "2026-05-06T10:00:05Z",
  },
  {
    id: "evt-plan-confirm-001",
    runId: "run-001",
    eventType: "plan.confirmation_requested",
    actor: "agent",
    summary: "Plan confirmation requested",
    payload: {
      plan_id: "plan-001",
      requires_confirmation: false,
      reasons: ["Plan is low-risk and read-only; no confirmation required"],
    },
    createdAt: "2026-05-06T10:00:06Z",
  },
  {
    id: "evt-run-completed-001",
    runId: "run-001",
    eventType: "run.completed",
    actor: "runtime",
    summary: "Run completed: no_tools",
    payload: { stop_reason: "no_tools", step_count: 1, tool_call_count: 0 },
    createdAt: "2026-05-06T10:00:07Z",
  },
];

// ── AgentPlan mock data ────────────────────────────────────────────────

export const mockAgentPlan = {
  id: "mock-plan-1",
  runId: "run-001",
  sessionId: "session-1",
  goal: "Read user's life model",
  assumptions: ["Life model exists"],
  missingContext: [],
  steps: [
    {
      index: 0,
      description: "Read life model data",
      toolIntent: "life_model.read",
      expectedOutput: "life model fields",
      dependsOn: [],
    },
  ],
  toolIntents: [
    {
      toolName: "life_model.read",
      purpose: "read",
      riskLevel: "low" as const,
      isWrite: false,
      parametersSummary: undefined,
    },
  ],
  subagentAssignments: [],
  permissionRequirements: [],
  rollbackPlan: undefined,
  successCriteria: ["life model returned"],
  riskLevel: "low" as const,
  requiresConfirmation: false,
  status: "confirmed" as const,
  createdAt: "2026-05-06T10:00:00Z",
  updatedAt: "2026-05-06T10:00:00Z",
  confirmedAt: "2026-05-06T10:00:01Z",
  completedAt: undefined,
};

// ── AgentSpec mock data ──────────────────────────────────────────────────

export const mockAgentSpec: AgentSpec = {
  id: "main.default",
  role: "main",
  name: "OpenLife Main Agent",
  purpose: "LifeModel-governed personal agent",
  promptBlockIds: ["base_system", "tool_discipline", "privacy_rule"],
  allowedTools: [],
  deniedTools: [],
  canAccessLifemodel: true,
  canAccessMemoryEvidence: true,
  canGenerateProposals: true,
  maxSteps: 5,
  maxToolCalls: 3,
  timeoutSeconds: 60,
  outputSchemaId: undefined,
  readOnly: false,
  privacyPolicy: "local_only",
  active: true,
  createdAt: "2026-05-06T10:00:00Z",
  updatedAt: "2026-05-06T10:00:00Z",
};
