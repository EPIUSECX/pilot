<script setup lang="ts">
import { Alert, Button, LoadingText, toast } from 'frappe-ui'
import { onMounted, ref } from 'vue'
import BenchRow from '@/components/BenchRow.vue'
import ConfirmActionDialog from '@/components/ConfirmActionDialog.vue'
import CreateBenchDialog from '@/components/CreateBenchDialog.vue'
import ErrorNotice from '@/components/ErrorNotice.vue'
import GeneratedPasswordDialog from '@/components/GeneratedPasswordDialog.vue'
import { useBenches } from '@/composables/useBenches'
import { DesktopCallError, desktop, toDesktopError } from '@/services/desktop'
import type { BenchSummary, CreateBenchRequest, DesktopError } from '@/types/contracts'

const emit = defineEmits<{ goto: [page: 'environment']; showLogs: [benchId: string] }>()
const { benches, isLoading, loadError, busy, refresh, start, stop, restart, cancelStart } =
  useBenches()

const isCreateOpen = ref(false)
const createError = ref<DesktopError | null>(null)
const password = ref<{ bench: string; value: string } | null>(null)
const isPasswordOpen = ref(false)
const pending = ref<{ bench: BenchSummary; action: 'stop' | 'restart' } | null>(null)
const isConfirmOpen = ref(false)

onMounted(refresh)

async function run(action: () => Promise<void>) {
  try {
    await action()
  } catch (error) {
    toast.error(toDesktopError(error).message)
  }
}

async function startAndOpen(id: string) {
  const outcome = await start(id)
  if (outcome.ok) await run(() => desktop.openDashboard(id))
}

function askBeforeAffecting(bench: BenchSummary, action: 'stop' | 'restart') {
  pending.value = { bench, action }
  isConfirmOpen.value = true
}

async function onStop(bench: BenchSummary) {
  if (bench.ownership === 'attached') return askBeforeAffecting(bench, 'stop')
  await stop(bench.id, false)
}

async function onRestart(bench: BenchSummary) {
  if (bench.ownership === 'attached') return askBeforeAffecting(bench, 'restart')
  await restart(bench.id, false)
}

async function confirmPending() {
  const current = pending.value
  if (!current) return
  await (current.action === 'stop' ? stop(current.bench.id, true) : restart(current.bench.id, true))
}

async function create(request: CreateBenchRequest) {
  createError.value = null
  try {
    const result = await desktop.createBench(request)
    await refresh()
    if (result.generatedAdminPassword) {
      password.value = { bench: request.name, value: result.generatedAdminPassword }
      isPasswordOpen.value = true
    }
    toast.success(result.message)
    void startAndOpen(request.name)
  } catch (error) {
    createError.value = error instanceof DesktopCallError ? error.detail : toDesktopError(error)
    throw error
  }
}
</script>

<template>
  <section class="flex flex-col gap-4">
    <header class="flex flex-wrap items-center gap-2">
      <div>
        <h2 class="text-xl font-semibold text-ink-gray-9">Benches</h2>
        <p class="text-p-sm text-ink-gray-6">
          Development benches managed by your Pilot installation.
        </p>
      </div>
      <div class="ml-auto flex gap-2">
        <Button
          label="Refresh"
          icon-left="lucide-refresh-cw"
          :loading="isLoading"
          @click="refresh"
        />
        <Button
          label="Create bench"
          variant="solid"
          icon-left="lucide-plus"
          :disabled="loadError?.category === 'not_configured'"
          @click="isCreateOpen = true"
        />
      </div>
    </header>

    <Alert
      v-if="loadError?.category === 'not_configured'"
      theme="amber"
      title="Pilot is not set up yet"
      :description="loadError.recovery ?? loadError.message"
    >
      <template #actions
        ><Button label="Open Environment" @click="emit('goto', 'environment')" /></template
      >
    </Alert>
    <ErrorNotice v-else-if="loadError" :error="loadError" />
    <LoadingText v-else-if="isLoading && benches.length === 0" text="Looking for benches" />
    <div
      v-else-if="benches.length === 0"
      class="flex flex-col items-start gap-3 rounded-lg border border-dashed border-outline-gray-3 p-8"
    >
      <p class="font-medium text-ink-gray-9">No benches yet</p>
      <p class="text-p-sm text-ink-gray-6">
        Pilot found no benches in its benches folder. Create one to get started.
      </p>
      <Button
        label="Create bench"
        variant="solid"
        icon-left="lucide-plus"
        @click="isCreateOpen = true"
      />
    </div>

    <div class="flex flex-col gap-3">
      <BenchRow
        v-for="bench in benches"
        :key="bench.id"
        :bench="bench"
        :busy-label="busy[bench.id]"
        @start="startAndOpen(bench.id)"
        @cancel="cancelStart(bench.id)"
        @open="run(() => desktop.openDashboard(bench.id))"
        @stop="onStop(bench)"
        @restart="onRestart(bench)"
        @editor="run(() => desktop.openProject(bench.id, 'editor'))"
        @finder="run(() => desktop.openProject(bench.id, 'finder'))"
        @logs="emit('showLogs', bench.id)"
        @reset-session="run(() => desktop.resetDashboardSession(bench.id))"
      />
    </div>

    <CreateBenchDialog
      v-model:open="isCreateOpen"
      :existing-names="benches.map((bench) => bench.name)"
      :error="createError"
      :on-create="create"
    />
    <GeneratedPasswordDialog
      v-if="password"
      v-model:open="isPasswordOpen"
      :bench-name="password.bench"
      :password="password.value"
    />
    <ConfirmActionDialog
      v-if="pending"
      v-model:open="isConfirmOpen"
      :title="
        pending.action === 'stop' ? `Stop ${pending.bench.name}?` : `Restart ${pending.bench.name}?`
      "
      message="This bench was already running before Pilot Desktop opened. Its web, worker and scheduler processes will stop for everything using it, including terminals and other tools."
      :confirm-label="pending.action === 'stop' ? 'Stop bench' : 'Stop and restart'"
      :on-confirm="confirmPending"
    />
  </section>
</template>
