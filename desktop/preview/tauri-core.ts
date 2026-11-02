/** PREVIEW ONLY: stands in for @tauri-apps/api/core when PILOT_DESKTOP_PREVIEW=1. Never bundled in app builds. */
const error = (category: string, message: string) => ({ category, message, stage: 'preview', recovery: 'Preview data only.', details: null })
const bench = (name: string, status: string, ownership: string, extra = {}) => ({
  id: name, name, path: `/Users/dev/Pilot Root/benches/${name}`, status, ownership,
  dashboardUrl: status === 'running' ? 'http://127.0.0.1:7000' : null, frappeBranch: 'version-16',
  isProduction: false, isInitialized: true, adminEnabled: true, lastError: null, notes: [], ...extra,
})
const benches = [
  bench('main', 'running', 'owned'),
  bench('shared-qa', 'running', 'attached'),
  bench('scratch', 'stopped', 'none'),
  bench('broken', 'failed', 'none', { lastError: { ...error('conflict', 'Port 7003 is already used by another process (node server.js).'), stage: 'identity', recovery: 'Stop the other program or change this bench\'s admin port, then try again.' } }),
]
const check = (id: string, label: string, status: string, message: string, extra = {}) => ({ id, label, status, required: true, path: null, version: null, message, nextAction: null, details: null, ...extra })
const report = {
  isReady: false, architecture: 'aarch64', pilotRoot: '/Users/dev/Pilot Root', pilotVersion: 'dev',
  checks: [
    check('pilot', 'Pilot installation', 'ok', 'Pilot is installed.', { path: '/Users/dev/Pilot Root' }),
    check('python', 'Python for the Pilot command (host)', 'ok', 'Runs the pilot command. Each bench gets its own Python separately.', { version: '3.12.13', path: '/Users/dev/.local/share/uv/python/bin/python3.12' }),
    check('node', 'Node.js', 'incompatible', 'Found Node.js 22.23.1; Pilot needs 24 or later to build benches.', { version: '22.23.1', nextAction: 'Install Node.js 24 with `brew install node@24`, then choose Refresh.' }),
    check('redis', 'Redis', 'missing', 'redis-server was not found.', { nextAction: 'Install Redis with `brew install redis`.' }),
    check('mariadb', 'MariaDB', 'stopped', 'Installed. Pilot starts its own database server when a bench is initialized.', { required: false, version: '13.0.2' }),
    check('editor', 'Cursor (editor shortcut)', 'ok', 'Open in editor is available.', { required: false }),
  ],
}
const handlers: Record<string, unknown> = {
  detect_environment: report, choose_pilot_installation: null,
  plan_installation: { sourceUrl: 'https://raw.githubusercontent.com/frappe/pilot/develop/install.sh', steps: ['Download install.sh and show its SHA-256.', 'Install Homebrew only if `brew` is missing.'], notes: ['Nothing runs until you type yes in Terminal.'] },
  get_runtime_info: { version: '0.1.0', architecture: 'aarch64', minimumMacos: '14.0' },
  list_benches: benches,
  get_preferences: { schemaVersion: 1, pilotRoot: null, editor: 'cursor', notifications: true, ui: { lastPage: null, lastBench: null } },
  read_logs: [
    { bench: 'main', stream: 'launcher', timestampMs: Date.now(), text: 'Started Pilot (pid 4242).' },
    { bench: 'main', stream: 'stdout', timestampMs: Date.now(), text: '  Open http://localhost:7000/?sid=[redacted] in your browser' },
    { bench: 'main', stream: 'stderr', timestampMs: Date.now(), text: '[web] warning: example stderr line' },
  ],
}
export async function invoke<T>(command: string): Promise<T> {
  if (command in handlers) return handlers[command] as T
  throw error('io', `${command} is not available in the browser preview.`)
}
