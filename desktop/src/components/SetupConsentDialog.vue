<script setup lang="ts">
import { Dialog } from 'frappe-ui'
import { computed } from 'vue'
import type { SetupPlan } from '@/types/contracts'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{ plan: SetupPlan | null; onBegin: () => Promise<void> }>()

const actions = computed(() => [
  {
    label: 'Install',
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
    icon="lucide-download"
    size="lg"
    :actions="actions"
  >
    <div v-if="plan" class="flex flex-col gap-4 text-p-base text-ink-gray-8">
      <p>Pilot Desktop will add the following to your Mac. You can keep working while it runs.</p>
      <ul class="flex flex-col gap-2">
        <li v-for="step in plan.steps" :key="step.id">
          <span class="font-medium text-ink-gray-9">{{ step.label }}</span>
          <span class="text-ink-gray-6"> · {{ step.detail }}</span>
        </li>
      </ul>
      <p v-if="plan.needsPassword" class="text-ink-gray-7">
        macOS will ask for your Mac password once, in its own window.
      </p>
      <p class="text-ink-gray-6">{{ plan.estimate }}</p>
    </div>
  </Dialog>
</template>
