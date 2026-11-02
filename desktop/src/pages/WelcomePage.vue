<script setup lang="ts">
import { Alert, Button, LoadingText, toast } from 'frappe-ui'
import { onMounted, ref } from 'vue'
import CheckRow from '@/components/CheckRow.vue'
import ErrorNotice from '@/components/ErrorNotice.vue'
import InstallPlanDialog from '@/components/InstallPlanDialog.vue'
import { useEnvironment } from '@/composables/useEnvironment'
import { desktop } from '@/services/desktop'
import type { InstallationPlan } from '@/types/contracts'

const emit = defineEmits<{ continue: [] }>()
const { report, isChecking, error, refresh, choose } = useEnvironment()
const plan = ref<InstallationPlan | null>(null)
const isPlanOpen = ref(false)

onMounted(refresh)

async function showPlan() {
  plan.value = await desktop.planInstallation()
  isPlanOpen.value = true
}

async function begin() {
  await desktop.beginInstallation()
  toast.info('Terminal opened. Come back and choose Refresh when the installer finishes.')
}
</script>

<template>
  <section class="flex flex-col gap-4">
    <header class="flex flex-wrap items-center gap-2">
      <div>
        <h2 class="text-xl font-semibold text-ink-gray-9">Environment</h2>
        <p class="text-p-sm text-ink-gray-6">
          What Pilot needs on this Mac to create and run benches.
        </p>
      </div>
      <div class="ml-auto flex gap-2">
        <Button label="Choose Pilot folder" icon-left="lucide-folder-search" @click="choose" />
        <Button label="Install Pilot" icon-left="lucide-download" @click="showPlan" />
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
        Pilot's Python and each bench's Python are separate environments. Building Pilot Desktop
        from source also needs Rust and the Xcode command line tools, which an installed app does
        not. Architecture: {{ report.architecture }}.
      </p>
    </template>
    <InstallPlanDialog v-model:open="isPlanOpen" :plan="plan" :on-begin="begin" />
  </section>
</template>
