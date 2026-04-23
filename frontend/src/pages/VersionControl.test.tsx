import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { render, screen, waitFor, fireEvent } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'
import VersionControl from './VersionControl'
import { invoke } from '@tauri-apps/api/core'
import { mockInvoke } from '@/test/mocks/tauri'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

describe('VersionControl', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke)
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('shows diff summary after comparing versions', async () => {
    render(
      <BrowserRouter>
        <VersionControl />
      </BrowserRouter>
    )

    await waitFor(() => {
      expect(screen.getByText('历史版本')).toBeInTheDocument()
    })

    const checkboxes = screen.getAllByRole('checkbox')
    fireEvent.click(checkboxes[0])
    fireEvent.click(checkboxes[1])
    fireEvent.click(screen.getByText('对比选中版本'))

    await waitFor(() => {
      expect(screen.getByText('差异摘要')).toBeInTheDocument()
    })

    expect(screen.getByText(/身份 · \d+ 处/)).toBeInTheDocument()
    expect(screen.getByText(/目标 · \d+ 处/)).toBeInTheDocument()
    expect(screen.getByText('关键变化')).toBeInTheDocument()
  })
})
