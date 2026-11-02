import type { UnlistenFn } from '@tauri-apps/api/event'

/** Holds a listener registration that may resolve after the caller has already disposed it. */
export function createSubscription(register: () => Promise<UnlistenFn>) {
  let disposed = false
  let unlisten: UnlistenFn | null = null
  register().then((stop) => {
    if (disposed) stop()
    else unlisten = stop
  })
  return () => {
    disposed = true
    unlisten?.()
    unlisten = null
  }
}
