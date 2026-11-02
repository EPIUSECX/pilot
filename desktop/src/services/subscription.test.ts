import { expect, test, vi } from 'vitest'
import { createSubscription } from './subscription'

test('disposing after registration stops the listener once', async () => {
  const stop = vi.fn()
  const dispose = createSubscription(() => Promise.resolve(stop))
  await Promise.resolve()
  dispose()
  dispose()
  expect(stop).toHaveBeenCalledTimes(1)
})

test('disposing before registration resolves still stops the listener', async () => {
  const stop = vi.fn()
  const dispose = createSubscription(() => Promise.resolve(stop))
  dispose()
  await Promise.resolve()
  await Promise.resolve()
  expect(stop).toHaveBeenCalledTimes(1)
})
