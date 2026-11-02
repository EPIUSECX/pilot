<script setup lang="ts">
import { Dialog } from 'frappe-ui'
import { computed, ref } from 'vue'
import { DesktopCallError, desktop } from '@/services/desktop'
import type { DesktopError } from '@/types/contracts'
import ErrorNotice from './ErrorNotice.vue'

const props = defineProps<{ ownedBenches: string[] | null }>()
const emit = defineEmits<{ cancel: [] }>()

const error = ref<DesktopError | null>(null)
const open = computed({
  get: () => props.ownedBenches !== null,
  set: (value) => {
    if (!value) {
      error.value = null
      emit('cancel')
    }
  },
})

const actions = [
  {
    label: 'Stop benches and quit',
    variant: 'solid' as const,
    theme: 'red' as const,
    onClick: async () => {
      error.value = null
      try {
        await desktop.quit(true)
      } catch (caught) {
        if (caught instanceof DesktopCallError) error.value = caught.detail
      }
    },
  },
]
</script>

<template>
  <Dialog
    v-model:open="open"
    title="Quit Pilot Desktop?"
    message="Benches started by Pilot Desktop stop when it quits. Benches that were already running are left running."
    icon="lucide-power"
    theme="amber"
    size="md"
    :actions="actions"
  >
    <div class="flex flex-col gap-3">
      <ul class="list-disc pl-5 text-p-sm">
        <li v-for="name in ownedBenches ?? []" :key="name">{{ name }}</li>
      </ul>
      <ErrorNotice v-if="error" :error="error" />
    </div>
  </Dialog>
</template>
