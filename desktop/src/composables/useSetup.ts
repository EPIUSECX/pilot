import { onScopeDispose, ref } from 'vue'
import { DesktopCallError, desktop, onSetupProgress } from '@/services/desktop'
import { createSubscription } from '@/services/subscription'
import type { DesktopError, SetupPlan, SetupProgress } from '@/types/contracts'

const idle: SetupProgress = { phase: 'idle', steps: [], activity: '', failure: null }
const progress = ref<SetupProgress>(idle)
const plan = ref<SetupPlan | null>(null)
const error = ref<DesktopError | null>(null)

async function attempt(action: () => Promise<unknown>) {
  try {
    await action()
    error.value = null
  } catch (caught) {
    error.value = caught instanceof DesktopCallError ? caught.detail : null
  }
}

export function useSetup() {
  const dispose = createSubscription(() => onSetupProgress((next) => (progress.value = next)))
  onScopeDispose(dispose)
  desktop.getSetupProgress().then((current) => (progress.value = current))

  return {
    progress,
    plan,
    error,
    loadPlan: () => attempt(async () => (plan.value = await desktop.planSetup())),
    start: () => attempt(desktop.startSetup),
    cancel: () => attempt(desktop.cancelSetup),
    async moveAside() {
      let moved: string | null = null
      await attempt(async () => (moved = await desktop.movePilotAside()))
      return moved
    },
  }
}
