<script setup lang="ts">
import { Button, Dialog, toast } from 'frappe-ui'

const open = defineModel<boolean>('open', { required: true })
defineProps<{ benchName: string; password: string }>()

async function copy(password: string) {
  await navigator.clipboard.writeText(password)
  toast.success('Password copied')
}
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="`Admin password for ${benchName}`"
    message="Pilot generated this password. It is shown once and is not stored by Pilot Desktop. Change it later with pilot set-admin-password."
    icon="lucide-key-round"
    size="md"
    :actions="[{ label: 'I saved it', variant: 'solid', onClick: ({ close }) => close() }]"
  >
    <div class="flex items-center gap-2">
      <code class="flex-1 select-all rounded bg-surface-gray-2 p-3 font-mono text-sm"
        >{{ password }}</code
      >
      <Button label="Copy" icon-left="lucide-copy" @click="copy(password)" />
    </div>
  </Dialog>
</template>
