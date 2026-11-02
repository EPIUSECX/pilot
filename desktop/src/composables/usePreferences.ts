import { ref } from 'vue'
import { desktop } from '@/services/desktop'
import type { Preferences, PreferenceUpdate } from '@/types/contracts'

const preferences = ref<Preferences | null>(null)

async function load() {
  preferences.value = await desktop.getPreferences()
}

async function save(update: PreferenceUpdate) {
  preferences.value = await desktop.savePreferences(update)
}

export function usePreferences() {
  return { preferences, load, save }
}
