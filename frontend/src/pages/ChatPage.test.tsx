import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import ChatPage from './ChatPage'
import { invoke } from '@tauri-apps/api/core'
import { mockInvoke } from '@/test/mocks/tauri'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}))

describe('ChatPage', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    vi.mocked(invoke).mockImplementation(mockInvoke)
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it('renders chat page with session list', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('会话 1')).toBeInTheDocument()
    })

    expect(screen.getByText('会话 2')).toBeInTheDocument()
  })

  it('shows quick command guide by default', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText(/快捷指令/)).toBeInTheDocument()
    })
  })

  it('allows typing a message', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByPlaceholderText(/输入消息/)).toBeInTheDocument()
    })
    await screen.findByText('会话 1')
    await screen.findByText('你好！我是 OpenLife。')

    const textarea = screen.getByPlaceholderText(/输入消息/)
    fireEvent.change(textarea, { target: { value: '测试消息' } })
    expect(textarea).toHaveValue('测试消息')
  })

  it('renders readiness bar with local model status', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    expect(await screen.findByText('聊天就绪')).toBeInTheDocument()
    expect(screen.getAllByText(/llama3:latest/).length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText(/云端 API：未配置/)).toBeInTheDocument()
  })

  it('shows companion cockpit with life model pulse', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    expect(await screen.findByText('陪跑现场')).toBeInTheDocument()
    expect(screen.getByText('使命')).toBeInTheDocument()
    expect(screen.getByText('成为更好的自己')).toBeInTheDocument()
    expect(screen.getByText('当前重心')).toBeInTheDocument()
    expect(screen.getByText('工作')).toBeInTheDocument()
  })

  it('fills prompt from companion mode card', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    await screen.findByText('陪跑现场')
    fireEvent.click(screen.getAllByText('目标拆解')[0])

    expect((textarea as HTMLTextAreaElement).value).toContain('请帮我拆解一个当前目标')
  })

  it('does not call model stream when chat is not ready but keeps slash commands usable', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === 'get_system_diagnostics') {
        return Promise.resolve({
          router: { onnx_available: false, onnx_disabled: false, active_backend: 'regex', latency_threshold_us: 50000 },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 0,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 0,
          unfinished_builder_sessions: 0,
          ollama_online: false,
          local_model: 'llama3',
          resolved_local_model: null,
          prefer_local_model: true,
          cloud_api_configured: false,
          chat_ready: false,
          readiness_issues: ['聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。'],
          data_dir: '/tmp/openlife-test',
          snapshot_count: 0,
          life_model_ready: true,
          app_version: '0.1.0',
          model_empty: false,
          chat_session_count: 0,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
        } as any)
      }
      return mockInvoke(cmd, args)
    })

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    await screen.findByText('需要配置')

    fireEvent.change(textarea, { target: { value: '普通消息' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    await waitFor(() => {
      expect(screen.getByText(/普通对话暂不可用/)).toBeInTheDocument()
    })
    expect(invoke).not.toHaveBeenCalledWith('start_stream_message', expect.anything())

    fireEvent.change(textarea, { target: { value: '/goal' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    await waitFor(() => {
      const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'save_chat_message')
      expect(saveCalls.length).toBeGreaterThanOrEqual(2)
    })
  })

  it('shows DeepSeek API key guidance when cloud stream fails', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === 'start_stream_message') {
        return Promise.reject(new Error('DeepSeek error 401: invalid API Key'))
      }
      return mockInvoke(cmd, args)
    })

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    await screen.findByText('聊天就绪')
    fireEvent.change(textarea, { target: { value: '帮我规划今天' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    expect(await screen.findByText(/DeepSeek 鉴权失败/)).toBeInTheDocument()
    expect(screen.getByText(/去设置页查看“试用就绪检查”/)).toBeInTheDocument()
  })

  it('does not hide DeepSeek runtime errors behind non-blocking readiness warnings', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === 'get_system_diagnostics') {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: 'regex',
            latency_threshold_us: 50000,
          },
          mcp_server_count: 0,
          mcp_tool_count: 0,
          mcp_recent_audit_count: 0,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 0,
          unfinished_builder_sessions: 0,
          ollama_online: false,
          local_model: 'llama3',
          resolved_local_model: null,
          prefer_local_model: true,
          cloud_api_configured: true,
          cloud_provider: 'DeepSeek',
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: ['当前设置为优先本地模型，但未找到可用模型：llama3。'],
          data_dir: '/tmp/openlife-test',
          active_data_dir: '/tmp/openlife-test',
          legacy_data_dir: '/tmp/openlife-legacy',
          database_status: 'ok',
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: '0.1.0',
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: 'state',
          },
        } as any)
      }
      if (cmd === 'start_stream_message') {
        return Promise.reject(new Error('DeepSeek error 401: invalid API Key'))
      }
      return mockInvoke(cmd, args)
    })

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    await screen.findByText('聊天就绪')
    fireEvent.change(textarea, { target: { value: '测试 DeepSeek' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    expect(await screen.findByText(/DeepSeek 鉴权失败/)).toBeInTheDocument()
    expect(screen.queryByText(/暂时无法发送普通对话/)).not.toBeInTheDocument()
  })

  it('persists normal user messages before starting the stream', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    await screen.findByText('聊天就绪')
    fireEvent.change(textarea, { target: { value: '今天怎么安排？' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('save_chat_message', {
        sessionId: 'session-1',
        session_id: 'session-1',
        message: { role: 'user', content: '今天怎么安排？' },
      })
    })
    expect(invoke).toHaveBeenCalledWith('start_stream_message', expect.objectContaining({
      sessionId: 'session-1',
      session_id: 'session-1',
      args: expect.objectContaining({ sessionId: 'session-1', session_id: 'session-1' }),
    }))
  })

  it('persists slash command messages to chat history', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    fireEvent.change(textarea, { target: { value: '/goal' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    await waitFor(() => {
      const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'save_chat_message')
      expect(saveCalls).toHaveLength(2)
    })

    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'save_chat_message')
    expect(saveCalls[0][1]).toMatchObject({
      sessionId: 'session-1',
      session_id: 'session-1',
      message: { role: 'user', content: '/goal' },
    })
    expect(saveCalls[1][1]).toMatchObject({
      sessionId: 'session-1',
      session_id: 'session-1',
      message: { role: 'assistant' },
    })
  })

  it('supports adding a daily goal from slash command', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    fireEvent.change(textarea, { target: { value: '/goal add 阅读30分钟' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('add_daily_goal', { name: '阅读30分钟', time_block: undefined })
    })
    expect(await screen.findByText(/已添加今日目标：阅读30分钟/)).toBeInTheDocument()
  })

  it('supports completing a daily goal from slash command', async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    )

    const textarea = await screen.findByPlaceholderText(/输入消息/)
    fireEvent.change(textarea, { target: { value: '/goal done 早起' } })
    fireEvent.keyDown(textarea, { key: 'Enter', code: 'Enter' })

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('toggle_daily_goal', { index: 0 })
    })
    expect(await screen.findByText(/已完成今日目标：早起/)).toBeInTheDocument()
  })

})
