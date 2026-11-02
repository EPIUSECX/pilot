import { toast } from 'frappe-ui'
import { reactive, ref } from 'vue'
import { DesktopCallError, desktop, onBenchesChanged } from '@/services/desktop'
import { createSubscription } from '@/services/subscription'
import type { BenchSummary, DesktopError, OperationResult } from '@/types/contracts'

const benches = ref<BenchSummary[]>([])
const isLoading = ref(false)
const loadError = ref<DesktopError | null>(null)
const busy = reactive<Record<string, string>>({})
let subscribers = 0
let dispose: (() => void) | null = null

export type Outcome = { ok: true; result: OperationResult } | { ok: false; error: DesktopError }

async function refresh() {
  isLoading.value = true
  try {
    benches.value = await desktop.listBenches()
    loadError.value = null
  } catch (error) {
    loadError.value = error instanceof DesktopCallError ? error.detail : null
    benches.value = []
  } finally {
    isLoading.value = false
  }
}

async function perform(
  benchId: string,
  label: string,
  action: () => Promise<OperationResult>,
): Promise<Outcome> {
  if (busy[benchId]) {
    return { ok: false, error: busyError() }
  }
  busy[benchId] = label
  try {
    const result = await action()
    for (const notice of result.notices) toast.warning(notice)
    await refresh()
    return { ok: true, result }
  } catch (error) {
    await refresh()
    const detail = error instanceof DesktopCallError ? error.detail : fallbackError(error)
    if (detail.category !== 'confirmation_required') toast.error(detail.message)
    return { ok: false, error: detail }
  } finally {
    delete busy[benchId]
  }
}

function busyError(): DesktopError {
  return {
    category: 'busy',
    message: 'Another operation is already running for this bench.',
    stage: null,
    recovery: null,
    details: null,
  }
}

function fallbackError(error: unknown): DesktopError {
  const message = error instanceof Error ? error.message : String(error)
  return { category: 'io', message, stage: null, recovery: null, details: null }
}

export function useBenches() {
  return {
    benches,
    isLoading,
    loadError,
    busy,
    refresh,
    start: (id: string) => perform(id, 'Starting', () => desktop.startBench(id)),
    stop: (id: string, confirmAttached: boolean) =>
      perform(id, 'Stopping', () => desktop.stopBench(id, confirmAttached)),
    restart: (id: string, confirmAttached: boolean) =>
      perform(id, 'Restarting', () => desktop.restartBench(id, confirmAttached)),
    cancelStart: (id: string) => desktop.cancelStart(id),
  }
}

/** Keeps one live subscription for the whole app, however many components ask for it. */
export function retainBenchSubscription() {
  if (subscribers++ === 0) {
    dispose = createSubscription(() => onBenchesChanged((next) => (benches.value = next)))
  }
  return () => {
    if (--subscribers === 0) {
      dispose?.()
      dispose = null
    }
  }
}
