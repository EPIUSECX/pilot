import { computed, onScopeDispose, type Ref, ref, watch } from 'vue'
import { desktop, onLogLine } from '@/services/desktop'
import { appendBounded, filterLines, formatLine } from '@/services/logs'
import { createSubscription } from '@/services/subscription'
import type { LogLine } from '@/types/contracts'

export function useLogs(benchId: Ref<string | null>) {
  const live = ref<LogLine[]>([])
  const frozen = ref<LogLine[] | null>(null)
  const query = ref('')
  const isPaused = computed(() => frozen.value !== null)
  const lines = computed(() => filterLines(frozen.value ?? live.value, query.value))

  const dispose = createSubscription(() =>
    onLogLine((line) => {
      if (line.bench === benchId.value) live.value = appendBounded(live.value, [line])
    }),
  )
  onScopeDispose(dispose)

  watch(
    benchId,
    async (id) => {
      frozen.value = null
      live.value = id ? await desktop.readLogs(id) : []
    },
    { immediate: true },
  )

  return {
    lines,
    query,
    isPaused,
    togglePause: () => (frozen.value = isPaused.value ? null : [...live.value]),
    copyText: () => lines.value.map(formatLine).join('\n'),
  }
}
