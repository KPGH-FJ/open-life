import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import CalibrationPage from './CalibrationPage'
import { invoke } from '@tauri-apps/api/core'
import { mockInvoke } from '@/test/mocks/tauri'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

describe('CalibrationPage', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    vi.mocked(invoke).mockImplementation(mockInvoke)
    vi.spyOn(window, 'alert').mockImplementation(() => {})
  })

  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
  })

  it('renders calibration preview with before and after completion', async () => {
    render(
      <BrowserRouter>
        <CalibrationPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('周期校准')).toBeInTheDocument()
    })

    // Progress bar labels are now in Chinese
    expect(screen.getByText('Identity（身份认同）')).toBeInTheDocument()
    expect(screen.getByText('健康')).toBeInTheDocument()
  })

  it('applies selected calibration changes', async () => {
    render(
      <BrowserRouter>
        <CalibrationPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('直接应用')).toBeInTheDocument()
    })

    fireEvent.click(screen.getByRole('checkbox'))
    fireEvent.click(screen.getByText('直接应用'))

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        'apply_calibration',
        expect.objectContaining({
          changes: expect.any(Array),
        })
      )
    })
  })

  it('displays confidence badge and source breakdown for each change', async () => {
    render(
      <BrowserRouter>
        <CalibrationPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('周期校准')).toBeInTheDocument()
    })

    expect(screen.getAllByText('置信度 82%').length).toBeGreaterThanOrEqual(1)
    expect(screen.getByText('为什么推荐你校准这里')).toBeInTheDocument()
    // Source labels are now in expanded detail view
    // Click expand to see detailed sources
    const expandBtn = screen.getAllByTitle('查看详细信号来源')[0]
    fireEvent.click(expandBtn)
    await waitFor(() => {
      expect(screen.getByText('反馈信号')).toBeInTheDocument()
    })
  })

  it('shows signal summary cards with icons', async () => {
    render(
      <BrowserRouter>
        <CalibrationPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('周期校准')).toBeInTheDocument()
    })

    // Signal summary section headers
    expect(screen.getByText('融合信号概览')).toBeInTheDocument()
    expect(screen.getByText('最强反馈信号')).toBeInTheDocument()
    expect(screen.getByText('最强对话/行为信号')).toBeInTheDocument()
  })

  it('shows calibration governance guidance and linked follow-up actions', async () => {
    render(
      <BrowserRouter>
        <CalibrationPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('校准治理说明')).toBeInTheDocument()
    })

    expect(screen.getByText(/校准不是“让系统随便改你的人生模型”/)).toBeInTheDocument()
    expect(screen.getByText('先去检查记忆')).toBeInTheDocument()
    expect(screen.getByText('去版本控制看回滚路径')).toBeInTheDocument()
  })

  it('expands change item to show detailed source information', async () => {
    render(
      <BrowserRouter>
        <CalibrationPage />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('周期校准')).toBeInTheDocument()
    })

    // Click expand button
    const expandBtn = screen.getAllByTitle('查看详细信号来源')[0]
    fireEvent.click(expandBtn)

    // Should show detailed source breakdown
    await waitFor(() => {
      expect(screen.getByText('信号来源明细')).toBeInTheDocument()
    })

    // Source labels should be visible
    expect(screen.getByText('反馈信号')).toBeInTheDocument()
    expect(screen.getByText('行为记录')).toBeInTheDocument()
    expect(screen.getByText('对话推断')).toBeInTheDocument()
  })
})
