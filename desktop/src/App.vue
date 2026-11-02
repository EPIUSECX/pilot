<script setup lang="ts">
import { Button, FrappeUIProvider } from 'frappe-ui'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import QuitDialog from '@/components/QuitDialog.vue'
import { retainBenchSubscription } from '@/composables/useBenches'
import { useQuitRequest } from '@/composables/useQuitRequest'
import BenchesPage from '@/pages/BenchesPage.vue'
import LogsPage from '@/pages/LogsPage.vue'
import PreferencesPage from '@/pages/PreferencesPage.vue'
import WelcomePage from '@/pages/WelcomePage.vue'

type Page = 'benches' | 'environment' | 'logs' | 'preferences'

const NAVIGATION: { page: Page; label: string; icon: string }[] = [
  { page: 'benches', label: 'Benches', icon: 'lucide-layers' },
  { page: 'environment', label: 'Environment', icon: 'lucide-shield-check' },
  { page: 'logs', label: 'Logs', icon: 'lucide-scroll-text' },
  { page: 'preferences', label: 'Preferences', icon: 'lucide-settings' },
]

const page = ref<Page>('benches')
const logBench = ref<string | null>(null)
const { ownedBenches, dismiss } = useQuitRequest()
const current = computed(() => NAVIGATION.find((item) => item.page === page.value))
let release: (() => void) | null = null

onMounted(() => (release = retainBenchSubscription()))
onUnmounted(() => release?.())

function showLogs(benchId: string) {
  logBench.value = benchId
  page.value = 'logs'
}
</script>

<template>
  <FrappeUIProvider>
    <div class="flex h-screen bg-surface-white text-ink-gray-8">
      <nav
        class="flex w-48 shrink-0 flex-col gap-1 border-r border-outline-gray-2 bg-surface-gray-1 p-3"
        aria-label="Sections"
      >
        <h1 class="px-2 pb-3 pt-1 text-lg font-semibold text-ink-gray-9">Pilot Desktop</h1>
        <Button
          v-for="item in NAVIGATION"
          :key="item.page"
          class="w-full !justify-start"
          :variant="page === item.page ? 'subtle' : 'ghost'"
          :label="item.label"
          :icon-left="item.icon"
          :aria-current="page === item.page ? 'page' : undefined"
          @click="page = item.page"
        />
      </nav>
      <main class="min-w-0 flex-1 overflow-auto p-6" :aria-label="current?.label">
        <BenchesPage v-if="page === 'benches'" @goto="page = $event" @show-logs="showLogs" />
        <WelcomePage v-else-if="page === 'environment'" @continue="page = 'benches'" />
        <LogsPage v-else-if="page === 'logs'" v-model:bench-id="logBench" />
        <PreferencesPage v-else />
      </main>
    </div>
    <QuitDialog :owned-benches="ownedBenches" @cancel="dismiss" />
  </FrappeUIProvider>
</template>
