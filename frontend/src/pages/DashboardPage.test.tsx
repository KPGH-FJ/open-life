import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import DashboardPage from './DashboardPage'
import { invoke } from '@tauri-apps/api/core'
import { mockInvoke } from '@/test/mocks/tauri'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

describe('DashboardPage', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    vi.mocked(invoke).mockImplementation(mockInvoke)
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it('renders dashboard with correct title', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('仪表盘')).toBeInTheDocument()
    })
  })

  it('displays daily goals from tauri', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('今日目标')).toBeInTheDocument()
    })

    expect(screen.getByText('早起')).toBeInTheDocument()
    expect(screen.getByText('运动')).toBeInTheDocument()
  })

  it('displays gap analysis results', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('完成 AI 项目')).toBeInTheDocument()
    })
    expect(screen.getByText(/关键能力：编程/)).toBeInTheDocument()
    expect(screen.getByText('安排 2 周刻意练习，并补一个可验证里程碑')).toBeInTheDocument()
  })

  it('shows version info when snapshot exists', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getAllByText((content) => content.includes('0.1.0')).length).toBeGreaterThan(0)
    })
  })

  it('displays skill stats from life model', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('技能')).toBeInTheDocument()
    })
    const skillCard = screen.getByText('技能').parentElement
    expect(skillCard).toHaveTextContent('2')
  })

  it('shows memory count stats', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('记忆')).toBeInTheDocument()
    })
    expect(screen.getByText('42')).toBeInTheDocument()
  })

  it('shows state trend explanation for selected dimension', async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('趋势解释')).toBeInTheDocument()
    })

    expect(screen.getByText(/专注度最近有下降趋势/)).toBeInTheDocument()
    expect(screen.getByText(/预警原因：专注度低于阈值/)).toBeInTheDocument()
    expect(screen.getByText('最近备注')).toBeInTheDocument()
  })
})
