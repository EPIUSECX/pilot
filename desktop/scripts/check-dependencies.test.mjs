import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, test } from 'vitest'
import { checkDependencies } from './check-dependencies.mjs'

function project({ frappeUi = '1.1.0', tailwind = '3.4.19', vue = '3.5.44', declared = {} } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'dependency check '))
  const dependencies = { 'frappe-ui': '1.1.0', vue: '3.5.44', tailwindcss: '3.4.19', ...declared }
  writeFileSync(join(root, 'package.json'), JSON.stringify({ dependencies }))
  writeFileSync(join(root, 'package-lock.json'), '{}')
  for (const [name, version] of Object.entries({
    'frappe-ui': frappeUi,
    tailwindcss: tailwind,
    vue,
  })) {
    mkdirSync(join(root, 'node_modules', name), { recursive: true })
    writeFileSync(join(root, 'node_modules', name, 'package.json'), JSON.stringify({ version }))
  }
  return root
}

test('accepts frappe-ui 1.x with Tailwind 3.4 and Vue 3.5', () => {
  expect(checkDependencies(project())).toEqual([])
})

test.each(['0.1.40', '2.0.0', '1.0.0-rc.2'])('rejects frappe-ui %s', (version) => {
  expect(checkDependencies(project({ frappeUi: version })).join()).toMatch(/frappe-ui must resolve/)
})

test.each(['4.3.3', '3.3.0'])('rejects Tailwind %s', (version) => {
  expect(checkDependencies(project({ tailwind: version })).join()).toMatch(/tailwindcss must be/)
})

test('rejects Vue below the peer range', () => {
  expect(checkDependencies(project({ vue: '3.4.0' })).join()).toMatch(/vue must be/)
})

test('rejects unpinned ranges', () => {
  expect(checkDependencies(project({ declared: { 'frappe-ui': '^1.1.0' } })).join()).toMatch(
    /exact version/,
  )
})
