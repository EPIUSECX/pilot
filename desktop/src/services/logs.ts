import type { LogLine } from '@/types/contracts'

const MAXIMUM_LINES = 2000

export function appendBounded(lines: LogLine[], incoming: LogLine[]): LogLine[] {
  const merged = lines.concat(incoming)
  return merged.length > MAXIMUM_LINES ? merged.slice(merged.length - MAXIMUM_LINES) : merged
}

export function filterLines(lines: LogLine[], query: string): LogLine[] {
  const needle = query.trim().toLowerCase()
  return needle ? lines.filter((line) => line.text.toLowerCase().includes(needle)) : lines
}

export function formatLine(line: LogLine): string {
  const time = new Date(line.timestampMs).toLocaleTimeString()
  return `${time} [${line.stream}] ${line.text}`
}
