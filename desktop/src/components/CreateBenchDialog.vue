<script setup lang="ts">
import { Dialog, FormControl } from 'frappe-ui'
import { computed, ref, watch } from 'vue'
import type { CreateBenchRequest, DesktopError } from '@/types/contracts'
import ErrorNotice from './ErrorNotice.vue'

const NAME_PATTERN = /^[a-z0-9][a-z0-9_-]{0,47}$/
const BRANCHES = [
  { label: 'version-16 (stable)', value: 'version-16' },
  { label: 'develop', value: 'develop' },
]

const open = defineModel<boolean>('open', { required: true })
const props = defineProps<{
  existingNames: string[]
  error: DesktopError | null
  onCreate: (request: CreateBenchRequest) => Promise<void>
}>()

const name = ref('')
const frappeBranch = ref('version-16')
const isSubmitting = ref(false)

const nameError = computed(() => {
  if (!name.value) return undefined
  if (!NAME_PATTERN.test(name.value))
    return 'Use lowercase letters, digits, hyphens or underscores (up to 48).'
  if (props.existingNames.includes(name.value)) return 'A bench with this name already exists.'
  return undefined
})
const canSubmit = computed(() => name.value !== '' && !nameError.value && !isSubmitting.value)

watch(open, (isOpen) => {
  if (isOpen) {
    name.value = ''
    frappeBranch.value = 'version-16'
  }
})

function closeDialog() {
  open.value = false
}

async function submit(close: () => void) {
  if (!canSubmit.value) return
  isSubmitting.value = true
  try {
    await props.onCreate({ name: name.value, frappeBranch: frappeBranch.value })
    close()
  } catch {
    // The caller reports the failure through the error prop; the dialog stays open for a retry.
  } finally {
    isSubmitting.value = false
  }
}

const actions = computed(() => [
  {
    label: 'Create and start',
    variant: 'solid' as const,
    disabled: !canSubmit.value,
    onClick: ({ close }: { close: () => void }) => submit(close),
  },
])
</script>

<template>
  <Dialog
    v-model:open="open"
    title="Create a development bench"
    message="Pilot creates the bench in its benches folder, then Pilot Desktop starts it and opens Pilot's setup."
    icon="lucide-plus"
    size="md"
    :dismissible="!isSubmitting"
    :actions="actions"
  >
    <form class="flex flex-col gap-4" @submit.prevent="canSubmit && submit(closeDialog)">
      <FormControl
        v-model="name"
        label="Bench name"
        placeholder="my-bench"
        :error="nameError"
        required
      />
      <FormControl v-model="frappeBranch" type="select" label="Frappe branch" :options="BRANCHES" />
      <ErrorNotice v-if="error" :error="error" />
    </form>
  </Dialog>
</template>
