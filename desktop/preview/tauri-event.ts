/** PREVIEW ONLY: no events in the browser preview. */
export async function listen(): Promise<() => void> {
  return () => {}
}
export type UnlistenFn = () => void
