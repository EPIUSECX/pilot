<script setup lang="ts">
import { Dialog } from 'frappe-ui'
import { computed } from 'vue'

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{
  title: string
  message: string
  confirmLabel: string
  theme?: 'amber' | 'red' | 'blue'
  onConfirm: () => Promise<void> | void
}>()

const actions = computed(() => [
  {
    label: props.confirmLabel,
    variant: 'solid' as const,
    theme: props.theme === 'blue' ? ('blue' as const) : ('red' as const),
    onClick: async ({ close }: { close: () => void }) => {
      await props.onConfirm()
      close()
    },
  },
])
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="title"
    :message="message"
    :theme="theme ?? 'amber'"
    icon="lucide-triangle-alert"
    size="md"
    :actions="actions"
  >
    <slot />
  </Dialog>
</template>
