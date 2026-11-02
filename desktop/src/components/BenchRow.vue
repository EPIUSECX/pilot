<script setup lang="ts">
import { Badge, Button, Dropdown, Spinner } from 'frappe-ui'
import { computed } from 'vue'
import type { BenchSummary } from '@/types/contracts'
import ErrorNotice from './ErrorNotice.vue'
import StatusBadge from './StatusBadge.vue'

const props = defineProps<{ bench: BenchSummary; busyLabel?: string }>()
const emit = defineEmits<{
  start: []
  cancel: []
  open: []
  stop: []
  restart: []
  editor: []
  finder: []
  logs: []
  resetSession: []
}>()

const isRunning = computed(() => props.bench.status === 'running')
const isStarting = computed(
  () => props.bench.status === 'starting' && props.bench.ownership === 'owned',
)
const canStart = computed(
  () => props.bench.adminEnabled && !props.bench.isProduction && props.bench.notes.length === 0,
)

const menu = computed(() => [
  {
    label: 'Restart',
    icon: 'lucide-rotate-cw',
    condition: () => isRunning.value,
    onClick: () => emit('restart'),
  },
  { label: 'Open in editor', icon: 'lucide-code-xml', onClick: () => emit('editor') },
  { label: 'Reveal in Finder', icon: 'lucide-folder-open', onClick: () => emit('finder') },
  { label: 'Logs', icon: 'lucide-scroll-text', onClick: () => emit('logs') },
  {
    label: 'Reset dashboard session',
    icon: 'lucide-eraser',
    condition: () => isRunning.value,
    onClick: () => emit('resetSession'),
  },
])
</script>

<template>
  <article class="flex flex-col gap-3 rounded-lg border border-outline-gray-2 bg-surface-white p-4">
    <div class="flex flex-wrap items-center gap-2">
      <h3 class="text-lg font-semibold text-ink-gray-9">{{ bench.name }}</h3>
      <StatusBadge :status="bench.status" />
      <Badge v-if="bench.ownership === 'owned'" theme="blue" label="Started here" />
      <Badge v-else-if="bench.ownership === 'attached'" theme="gray" label="Attached" />
      <Badge v-if="bench.frappeBranch" variant="outline" :label="bench.frappeBranch" />
      <div class="ml-auto flex items-center gap-2">
        <span v-if="busyLabel" class="flex items-center gap-1 text-p-sm text-ink-gray-6"
          ><Spinner size="sm" />{{ busyLabel }}…</span
        >
        <Button v-if="isStarting" label="Cancel" @click="emit('cancel')" />
        <Button
          v-else-if="isRunning"
          variant="solid"
          label="Open Pilot"
          icon-left="lucide-external-link"
          :disabled="!!busyLabel"
          @click="emit('open')"
        />
        <Button
          v-else
          label="Start"
          variant="solid"
          icon-left="lucide-play"
          :disabled="!canStart || !!busyLabel"
          @click="emit('start')"
        />
        <Button
          v-if="isRunning"
          label="Stop"
          icon-left="lucide-square"
          :disabled="!!busyLabel"
          @click="emit('stop')"
        />
        <Dropdown :options="menu" align="end">
          <template #default="{ open }">
            <Button
              icon="lucide-ellipsis"
              tooltip="More actions"
              :active="open"
              :disabled="!!busyLabel"
            />
          </template>
        </Dropdown>
      </div>
    </div>
    <p class="truncate text-p-sm text-ink-gray-5" :title="bench.path">{{ bench.path }}</p>
    <p v-if="bench.ownership === 'attached'" class="text-p-sm text-ink-gray-6">
      Already running before Pilot Desktop opened. It keeps running when you quit.
    </p>
    <p v-for="note in bench.notes" :key="note" class="text-p-sm text-ink-gray-6">{{ note }}</p>
    <ErrorNotice v-if="bench.lastError" :error="bench.lastError" />
  </article>
</template>
