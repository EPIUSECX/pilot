<script setup lang="ts">
import { Button, FormControl, Switch, toast } from 'frappe-ui'
import { nextTick, ref, watch } from 'vue'
import { formatLine } from '@/services/logs'
import type { LogLine } from '@/types/contracts'

const props = defineProps<{ lines: LogLine[]; isPaused: boolean; copyText: () => string }>()
const query = defineModel<string>('query', { required: true })
const emit = defineEmits<{ togglePause: [] }>()
const scroller = ref<HTMLElement | null>(null)

watch(
  () => props.lines.length,
  async () => {
    if (props.isPaused) return
    await nextTick()
    scroller.value?.scrollTo({ top: scroller.value.scrollHeight })
  },
)

async function copy() {
  await navigator.clipboard.writeText(props.copyText())
  toast.success('Visible log lines copied')
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <div class="min-w-48 flex-1">
        <FormControl v-model="query" type="search" placeholder="Search logs" />
      </div>
      <Switch :model-value="isPaused" label="Pause" @update:model-value="emit('togglePause')" />
      <Button label="Copy" icon-left="lucide-copy" @click="copy" />
    </div>
    <div
      ref="scroller"
      class="min-h-0 flex-1 overflow-auto rounded-lg border border-outline-gray-2 bg-surface-gray-1 p-3 font-mono text-xs leading-5 text-ink-gray-8"
    >
      <p v-if="lines.length === 0" class="text-ink-gray-5">
        No log lines yet. Output appears here after redaction.
      </p>
      <div
        v-for="(line, index) in lines"
        :key="index"
        class="whitespace-pre-wrap break-words"
        :class="{ 'text-ink-red-4': line.stream === 'stderr' }"
      >
        {{ formatLine(line) }}
      </div>
    </div>
  </div>
</template>
