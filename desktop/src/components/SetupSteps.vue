<script setup lang="ts">
import type { SetupStep, SetupStepStatus } from '@/types/contracts'

defineProps<{ steps: SetupStep[] }>()

const icons: Record<SetupStepStatus, string> = {
  pending: 'lucide-circle',
  running: 'lucide-loader-circle animate-spin',
  done: 'lucide-circle-check text-ink-green-3',
  failed: 'lucide-circle-alert text-ink-red-4',
}
const tones: Record<SetupStepStatus, string> = {
  pending: 'text-ink-gray-4',
  running: 'text-ink-gray-9 font-medium',
  done: 'text-ink-gray-7',
  failed: 'text-ink-gray-9 font-medium',
}
</script>

<template>
  <ol class="flex flex-col gap-3" aria-label="Setup steps">
    <li
      v-for="step in steps"
      :key="step.id"
      class="flex items-center gap-3 text-base"
      :class="tones[step.status]"
    >
      <span class="size-5 shrink-0" :class="icons[step.status]" aria-hidden="true" />
      <span>{{ step.label }}</span>
      <span class="sr-only">{{ step.status }}</span>
    </li>
  </ol>
</template>
