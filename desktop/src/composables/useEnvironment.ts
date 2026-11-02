import { ref } from 'vue'
import { DesktopCallError, desktop } from '@/services/desktop'
import type { DesktopError, EnvironmentReport } from '@/types/contracts'

const report = ref<EnvironmentReport | null>(null)
const isChecking = ref(false)
const error = ref<DesktopError | null>(null)

async function run(source: () => Promise<EnvironmentReport | null>) {
  isChecking.value = true
  try {
    const next = await source()
    if (next) report.value = next
    error.value = null
  } catch (caught) {
    error.value = caught instanceof DesktopCallError ? caught.detail : null
  } finally {
    isChecking.value = false
  }
}

export function useEnvironment() {
  return {
    report,
    isChecking,
    error,
    refresh: () => run(desktop.detectEnvironment),
    choose: () => run(desktop.chooseInstallation),
  }
}
