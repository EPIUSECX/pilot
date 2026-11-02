<script setup lang="ts">
import { Badge } from 'frappe-ui'
import { computed } from 'vue'
import type { CheckStatus, DependencyCheck } from '@/types/contracts'

const props = defineProps<{ check: DependencyCheck }>()

const themes = {
  ok: 'green',
  missing: 'red',
  incompatible: 'amber',
  stopped: 'amber',
  unknown: 'gray',
} as const
const labels: Record<CheckStatus, string> = {
  ok: 'Ready',
  missing: 'Missing',
  incompatible: 'Incompatible',
  stopped: 'Not running',
  unknown: 'Unknown',
}
const theme = computed(() =>
  props.check.status === 'missing' && !props.check.required ? 'gray' : themes[props.check.status],
)
</script>

<template>
  <li class="flex flex-col gap-1 border-b border-outline-gray-1 py-3 last:border-b-0">
    <div class="flex flex-wrap items-center gap-2">
      <span class="font-medium text-ink-gray-9">{{ check.label }}</span>
      <Badge :theme="theme" :label="labels[check.status]" />
      <Badge v-if="!check.required" variant="outline" label="Optional" />
      <span v-if="check.version" class="text-p-sm text-ink-gray-5">{{ check.version }}</span>
    </div>
    <p class="text-p-sm text-ink-gray-7">{{ check.message }}</p>
    <p v-if="check.nextAction && check.status !== 'ok'" class="text-p-sm text-ink-gray-9">
      <span class="font-medium">Next:</span> {{ check.nextAction }}
    </p>
    <p v-if="check.path" class="truncate text-xs text-ink-gray-5" :title="check.path">
      {{ check.path }}
    </p>
  </li>
</template>
