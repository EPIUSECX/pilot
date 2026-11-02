<script setup lang="ts">
import { Dialog } from 'frappe-ui'
import { computed } from 'vue'
import type { InstallationPlan } from '@/types/contracts'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ plan: InstallationPlan | null; onBegin: () => Promise<void> }>()

const actions = computed(() => [
  {
    label: 'Open Terminal to continue',
    variant: 'solid' as const,
    onClick: async ({ close }: { close: () => void }) => {
      await props.onBegin()
      close()
    },
  },
])
</script>

<template>
  <Dialog
    v-model:open="open"
    title="Install Pilot"
    message="Review what the installer will do. Nothing changes until you confirm in Terminal."
    icon="lucide-download"
    size="lg"
    :actions="actions"
  >
    <div v-if="plan" class="flex flex-col gap-4 text-p-sm text-ink-gray-8">
      <ol class="list-decimal space-y-1 pl-5">
        <li v-for="step in plan.steps" :key="step">{{ step }}</li>
      </ol>
      <ul class="list-disc space-y-1 pl-5 text-ink-gray-6">
        <li v-for="note in plan.notes" :key="note">{{ note }}</li>
      </ul>
      <p class="break-all text-xs text-ink-gray-5">Source: {{ plan.sourceUrl }}</p>
    </div>
  </Dialog>
</template>
