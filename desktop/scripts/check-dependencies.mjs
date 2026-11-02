import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const EXACT_VERSION = /^\d+\.\d+\.\d+$/

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function parts(version) {
  return version.split('-')[0].split('.').map(Number)
}

function isAtLeast(version, minimum) {
  const [a, b, c] = parts(version)
  const [x, y, z] = parts(minimum)
  return a !== x ? a > x : b !== y ? b > y : c >= z
}

export function checkDependencies(root) {
  const problems = []
  const manifest = readJson(join(root, 'package.json'))
  const installed = (name) => {
    const path = join(root, 'node_modules', name, 'package.json')
    return existsSync(path) ? readJson(path) : null
  }

  for (const name of ['frappe-ui', 'tailwindcss', 'vue']) {
    const declared = manifest.dependencies?.[name] ?? manifest.devDependencies?.[name]
    if (!declared || !EXACT_VERSION.test(declared)) {
      problems.push(
        `${name} must be pinned to an exact version in package.json (found ${declared}).`,
      )
    }
  }
  if (!existsSync(join(root, 'package-lock.json'))) problems.push('package-lock.json is missing.')

  const frappeUi = installed('frappe-ui')
  const tailwind = installed('tailwindcss')
  const vue = installed('vue')
  if (!frappeUi) return [...problems, 'frappe-ui is not installed.']
  if (parts(frappeUi.version)[0] !== 1 || frappeUi.version.includes('-')) {
    problems.push(`frappe-ui must resolve to a stable 1.x release, got ${frappeUi.version}.`)
  }
  if (!tailwind || parts(tailwind.version)[0] !== 3 || !isAtLeast(tailwind.version, '3.4.2')) {
    problems.push(
      `tailwindcss must be 3.4.2 or later and below 4 (frappe-ui peer), got ${tailwind?.version}.`,
    )
  }
  if (!vue || !isAtLeast(vue.version, '3.5.0')) {
    problems.push(`vue must be 3.5.0 or later (frappe-ui peer), got ${vue?.version}.`)
  }
  return problems
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(fileURLToPath(import.meta.url), '..', '..')
  const problems = checkDependencies(root)
  if (problems.length) {
    console.error(problems.map((problem) => `dependency check: ${problem}`).join('\n'))
    process.exit(1)
  }
  console.log('dependency check: frappe-ui 1.x, Tailwind 3.4.x and Vue 3.5+ are compatible.')
}
