<script setup lang="ts">
import { FormControl } from 'frappe-ui'
import { computed } from 'vue'
import LogViewer from '@/components/LogViewer.vue'
import { useBenches } from '@/composables/useBenches'
import { useLogs } from '@/composables/useLogs'

const benchId = defineModel<string | null>('benchId', { required: true })
const { benches } = useBenches()
const { lines, query, isPaused, togglePause, copyText } = useLogs(benchId)
const options = computed(() =>
  benches.value.map((bench) => ({ label: bench.name, value: bench.id })),
)
</script>

<template>
  <section class="flex h-full min-h-0 flex-col gap-4">
    <header class="flex flex-wrap items-end gap-3">
      <div>
        <h2 class="text-xl font-semibold text-ink-gray-9">Logs</h2>
        <p class="text-p-sm text-ink-gray-6">
          Recent startup and launcher output. Secrets are removed before it reaches this screen.
        </p>
      </div>
      <div class="ml-auto w-56">
        <FormControl
          v-model="benchId"
          type="select"
          label="Bench"
          placeholder="Choose a bench"
          :options="options"
        />
      </div>
    </header>
    <p
      v-if="!benchId"
      class="rounded-lg border border-dashed border-outline-gray-3 p-8 text-p-sm text-ink-gray-6"
    >
      Choose a bench to see its logs.
    </p>
    <LogViewer
      v-else
      v-model:query="query"
      :lines="lines"
      :is-paused="isPaused"
      :copy-text="copyText"
      @toggle-pause="togglePause"
    />
  </section>
</template>
