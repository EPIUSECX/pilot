<script setup lang="ts">
import { Alert, Button } from 'frappe-ui'
import { computed, ref } from 'vue'
import type { DesktopError } from '@/types/contracts'

const props = defineProps<{ error: DesktopError }>()
const showDetails = ref(false)
const description = computed(() =>
  [props.error.stage ? `Stage: ${props.error.stage}.` : '', props.error.recovery ?? '']
    .filter(Boolean)
    .join(' '),
)
const theme = computed(() => (props.error.category === 'unsafe_to_stop' ? 'amber' : 'red'))
</script>

<template>
  <div class="flex flex-col gap-2">
    <Alert :theme="theme" :title="error.message" :description="description || undefined" />
    <div v-if="error.details" class="flex flex-col gap-2">
      <Button
        class="self-start"
        variant="ghost"
        size="sm"
        :label="showDetails ? 'Hide details' : 'Show details'"
        @click="showDetails = !showDetails"
      />
      <pre
        v-if="showDetails"
        class="max-h-48 overflow-auto rounded bg-surface-gray-2 p-3 text-xs text-ink-gray-7"
      >{{ error.details }}</pre>
    </div>
  </div>
</template>
