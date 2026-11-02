import { expect, test } from 'vitest'
import type { LogLine } from '@/types/contracts'
import { appendBounded, filterLines } from './logs'

const line = (text: string): LogLine => ({ bench: 'b', stream: 'stdout', timestampMs: 0, text })

test('the buffer never grows past its bound and keeps the newest lines', () => {
  const many = Array.from({ length: 2100 }, (_, index) => line(`line ${index}`))
  const bounded = appendBounded([], many)
  expect(bounded).toHaveLength(2000)
  expect(bounded[0].text).toBe('line 100')
})

test('search is case-insensitive and ignores blank queries', () => {
  const lines = [line('Starting WEB'), line('redis ready')]
  expect(filterLines(lines, 'web')).toHaveLength(1)
  expect(filterLines(lines, '  ')).toHaveLength(2)
})
