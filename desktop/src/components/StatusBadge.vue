<script setup lang="ts">
import { Badge } from 'frappe-ui'
import type { BenchStatus } from '@/types/contracts'

defineProps<{ status: BenchStatus }>()

const themes = {
  running: 'green',
  starting: 'amber',
  stopping: 'amber',
  failed: 'red',
  stopped: 'gray',
  unknown: 'gray',
} as const

const labels: Record<BenchStatus, string> = {
  running: 'Running',
  starting: 'Starting',
  stopping: 'Stopping',
  failed: 'Failed',
  stopped: 'Stopped',
  unknown: 'Unknown',
}
</script>

<template>
  <Badge :theme="themes[status]" :label="labels[status]" />
</template>
