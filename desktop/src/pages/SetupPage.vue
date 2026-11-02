<script setup lang="ts">
import { Alert, Button, LoadingText, toast } from 'frappe-ui'
import { computed, ref, watch } from 'vue'
import ConfirmActionDialog from '@/components/ConfirmActionDialog.vue'
import ErrorNotice from '@/components/ErrorNotice.vue'
import LogViewer from '@/components/LogViewer.vue'
import SetupConsentDialog from '@/components/SetupConsentDialog.vue'
import SetupSteps from '@/components/SetupSteps.vue'
import { useEnvironment } from '@/composables/useEnvironment'
import { useLogs } from '@/composables/useLogs'
import { useSetup } from '@/composables/useSetup'

const { progress, plan, error, loadPlan, start, cancel, moveAside } = useSetup()
const { report, choose, refresh } = useEnvironment()
const isConsentOpen = ref(false)
const isMoveOpen = ref(false)
const showDetails = ref(false)
const logBench = ref<string | null>('setup')
const { lines, query, isPaused, togglePause, copyText } = useLogs(logBench)

const isRunning = computed(() => progress.value.phase === 'running')
const hasPilot = computed(() => report.value?.pilotRoot != null)
const headline = computed(() => (hasPilot.value ? 'Finish setting up Pilot' : 'Set up Pilot'))

watch(
  () => progress.value.phase,
  async (phase) => {
    if (phase !== 'succeeded') return
    await refresh()
    toast.success('Pilot is ready')
  },
)

async function review() {
  await loadPlan()
  isConsentOpen.value = true
}

async function moveAndContinue() {
  const moved = await moveAside()
  if (!moved) return
  toast.success(`Your old Pilot folder is now at ${moved}`)
  await start()
}

async function copyDetails() {
  await navigator.clipboard.writeText(progress.value.failure?.details ?? '')
  toast.success('Details copied')
}
</script>

<template>
  <main class="mx-auto flex h-screen max-w-xl flex-col justify-center gap-6 px-6 py-10">
    <header class="flex flex-col gap-2">
      <span class="size-10 lucide-rocket text-ink-gray-8" aria-hidden="true" />
      <h1 class="text-2xl font-semibold text-ink-gray-9">{{ headline }}</h1>
      <p class="text-p-base text-ink-gray-6">
        Pilot Desktop installs and checks everything for you. You don't need to use Terminal.
      </p>
    </header>

    <ErrorNotice v-if="error" :error="error" />

    <template v-if="isRunning">
      <SetupSteps :steps="progress.steps" />
      <LoadingText :text="progress.activity || 'Working'" />
      <div class="flex items-center gap-2">
        <Button label="Cancel" @click="cancel" />
        <Button
          variant="ghost"
          :label="showDetails ? 'Hide details' : 'Show details'"
          @click="showDetails = !showDetails"
        />
      </div>
    </template>

    <template v-else>
      <Alert
        v-if="progress.phase === 'failed' && progress.failure"
        theme="red"
        :title="progress.failure.title"
        :description="progress.failure.message"
      />
      <Alert
        v-else-if="progress.phase === 'cancelled'"
        theme="amber"
        title="Setup was cancelled"
        description="Nothing was lost. Continue whenever you are ready."
      />
      <SetupSteps
        v-if="progress.steps.length && progress.phase !== 'idle'"
        :steps="progress.steps"
      />
      <div class="flex flex-wrap items-center gap-2">
        <Button
          v-if="progress.phase === 'failed' || progress.phase === 'cancelled'"
          variant="solid"
          :label="progress.phase === 'failed' ? 'Try again' : 'Continue setup'"
          @click="start"
        />
        <Button
          v-else
          variant="solid"
          label="Install Pilot"
          icon-left="lucide-download"
          @click="review"
        />
        <Button variant="ghost" label="I already have Pilot" @click="choose" />
        <Button
          v-if="progress.failure?.recovery === 'move_aside'"
          label="Move it aside and continue"
          icon-left="lucide-folder-input"
          @click="isMoveOpen = true"
        />
        <Button
          v-if="progress.failure"
          variant="ghost"
          label="Copy details"
          icon-left="lucide-copy"
          @click="copyDetails"
        />
        <Button
          variant="ghost"
          :label="showDetails ? 'Hide details' : 'Show details'"
          @click="showDetails = !showDetails"
        />
      </div>
    </template>

    <div v-if="showDetails" class="flex h-64 flex-col">
      <LogViewer
        v-model:query="query"
        :lines="lines"
        :is-paused="isPaused"
        :copy-text="copyText"
        @toggle-pause="togglePause"
      />
    </div>
    <ConfirmActionDialog
      v-model:open="isMoveOpen"
      title="Move your old Pilot folder aside?"
      message="The folder is renamed to 'pilot (previous)' in your home directory. Nothing is deleted. Pilot Desktop then installs a fresh copy."
      confirm-label="Move it aside"
      theme="blue"
      :on-confirm="moveAndContinue"
    />
    <SetupConsentDialog v-model:open="isConsentOpen" :plan="plan" :on-begin="start" />
  </main>
</template>
