import { describe, expect, it, vi } from 'vitest'
import type { SetupProgress } from '@/types/contracts'

const running: SetupProgress = {
  phase: 'running',
  steps: [{ id: 'node', label: 'Node.js 24', status: 'running' }],
  activity: 'Installing Node.js',
  failure: null,
}
let emit: (progress: SetupProgress) => void = () => {}
const startSetup = vi.fn()

vi.mock('@/services/desktop', () => ({
  DesktopCallError: class extends Error {
    constructor(readonly detail: unknown) {
      super('failed')
    }
  },
  desktop: {
    getSetupProgress: () =>
      Promise.resolve({ phase: 'idle', steps: [], activity: '', failure: null }),
    startSetup,
    planSetup: vi.fn(),
    cancelSetup: vi.fn(),
  },
  onSetupProgress: (handler: typeof emit) => {
    emit = handler
    return Promise.resolve(() => {})
  },
}))

describe('useSetup', () => {
  it('shows the progress the app reports while setup runs', async () => {
    const { useSetup } = await import('./useSetup')
    const { progress } = useSetup()
    await Promise.resolve()
    emit(running)
    expect(progress.value.activity).toBe('Installing Node.js')
  })

  it('keeps the failure from a refused start so the screen can show it', async () => {
    const { DesktopCallError } = await import('@/services/desktop')
    const detail = { category: 'busy', message: 'Setup is already running.' }
    startSetup.mockRejectedValueOnce(new DesktopCallError(detail as never))
    const { useSetup } = await import('./useSetup')
    const { start, error } = useSetup()
    await start()
    expect(error.value).toEqual(detail)
  })
})
