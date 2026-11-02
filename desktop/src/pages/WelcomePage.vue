<script setup lang="ts">
import { Alert, Button, LoadingText } from 'frappe-ui'
import { onMounted } from 'vue'
import CheckRow from '@/components/CheckRow.vue'
import ErrorNotice from '@/components/ErrorNotice.vue'
import { useEnvironment } from '@/composables/useEnvironment'

const emit = defineEmits<{ continue: [] }>()
const { report, isChecking, error, refresh, choose } = useEnvironment()

onMounted(refresh)
</script>

<template>
  <section class="flex flex-col gap-4">
    <header class="flex flex-wrap items-center gap-2">
      <div>
        <h2 class="text-xl font-semibold text-ink-gray-9">Diagnostics</h2>
        <p class="text-p-sm text-ink-gray-6">
          What Pilot needs on this Mac to create and run benches.
        </p>
      </div>
      <div class="ml-auto flex gap-2">
        <Button label="Choose Pilot folder" icon-left="lucide-folder-search" @click="choose" />
        <Button
          label="Refresh"
          icon-left="lucide-refresh-cw"
          :loading="isChecking"
          @click="refresh"
        />
      </div>
    </header>
    <ErrorNotice v-if="error" :error="error" />
    <LoadingText v-if="isChecking && !report" text="Checking this Mac" />
    <template v-if="report">
      <Alert
        :theme="report.isReady ? 'green' : 'amber'"
        :title="report.isReady ? 'Ready to run benches' : 'Some requirements need attention'"
        :description="
          report.pilotVersion
            ? `Pilot ${report.pilotVersion} at ${report.pilotRoot}`
            : (report.pilotRoot ?? 'No Pilot installation selected.')
        "
      >
        <template v-if="report.isReady" #actions>
          <Button variant="solid" label="Go to benches" @click="emit('continue')" />
        </template>
      </Alert>
      <ul class="rounded-lg border border-outline-gray-2 bg-surface-white px-4">
        <CheckRow v-for="check in report.checks" :key="check.id" :check="check" />
      </ul>
      <p class="text-p-sm text-ink-gray-5">
        Pilot's Python and each bench's Python are separate environments. Architecture:
        {{ report.architecture }}.
      </p>
    </template>
  </section>
</template>
