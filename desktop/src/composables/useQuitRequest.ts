import { onScopeDispose, ref } from 'vue'
import { onQuitRequested } from '@/services/desktop'
import { createSubscription } from '@/services/subscription'

export function useQuitRequest() {
  const ownedBenches = ref<string[] | null>(null)
  onScopeDispose(createSubscription(() => onQuitRequested((owned) => (ownedBenches.value = owned))))
  return { ownedBenches, dismiss: () => (ownedBenches.value = null) }
}
