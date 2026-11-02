<script setup lang="ts">
import { Button, FormControl, Switch, toast } from 'frappe-ui'
import { computed, onMounted, ref, watch } from 'vue'
import ErrorNotice from '@/components/ErrorNotice.vue'
import { useEnvironment } from '@/composables/useEnvironment'
import { usePreferences } from '@/composables/usePreferences'
import { DesktopCallError, desktop } from '@/services/desktop'
import type { DesktopError, EditorChoice } from '@/types/contracts'

const EDITORS = [
  { label: 'Cursor', value: 'cursor' },
  { label: 'Visual Studio Code', value: 'vscode' },
  { label: 'Zed', value: 'zed' },
]

const { preferences, load, save } = usePreferences()
const { choose } = useEnvironment()
const editor = ref<EditorChoice>('cursor')
const notifications = ref(true)
const error = ref<DesktopError | null>(null)
const runtime = ref<{ version: string; architecture: string; minimumMacos: string } | null>(null)

const isDirty = computed(
  () =>
    preferences.value !== null &&
    (editor.value !== preferences.value.editor ||
      notifications.value !== preferences.value.notifications),
)

watch(preferences, (value) => {
  if (!value) return
  editor.value = value.editor
  notifications.value = value.notifications
})

onMounted(async () => {
  await load()
  runtime.value = await desktop.runtimeInfo()
})

async function submit() {
  if (!preferences.value) return
  error.value = null
  try {
    await save({
      editor: editor.value,
      notifications: notifications.value,
      ui: preferences.value.ui,
    })
    toast.success('Preferences saved')
  } catch (caught) {
    error.value = caught instanceof DesktopCallError ? caught.detail : null
  }
}

async function chooseFolder() {
  await choose()
  await load()
}
</script>

<template>
  <section class="flex max-w-xl flex-col gap-5">
    <header>
      <h2 class="text-xl font-semibold text-ink-gray-9">Preferences</h2>
      <p class="text-p-sm text-ink-gray-6">Stored on this Mac without credentials.</p>
    </header>
    <ErrorNotice v-if="error" :error="error" />
    <div class="flex flex-col gap-1">
      <span class="text-p-sm font-medium text-ink-gray-8">Pilot installation</span>
      <div class="flex items-center gap-2">
        <code
          class="min-w-0 flex-1 truncate rounded bg-surface-gray-2 px-3 py-2 text-sm"
          :title="preferences?.pilotRoot ?? ''"
          >{{ preferences?.pilotRoot ?? 'Detected automatically (~/pilot)' }}</code
        >
        <Button label="Choose…" icon-left="lucide-folder-search" @click="chooseFolder" />
      </div>
    </div>
    <FormControl
      v-model="editor"
      type="select"
      label="Editor for Open in editor"
      :options="EDITORS"
    />
    <Switch v-model="notifications" label="Notify me when a bench stops unexpectedly" />
    <div><Button variant="solid" label="Save" :disabled="!isDirty" @click="submit" /></div>
    <p v-if="runtime" class="text-p-sm text-ink-gray-5">
      Pilot Desktop {{ runtime.version }} · {{ runtime.architecture }} · macOS
      {{ runtime.minimumMacos }}
      or later
    </p>
  </section>
</template>
