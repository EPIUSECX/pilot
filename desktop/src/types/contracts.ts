export type BenchStatus = 'stopped' | 'starting' | 'running' | 'stopping' | 'failed' | 'unknown'
export type Ownership = 'owned' | 'attached' | 'none'

export type ErrorCategory =
  | 'invalid_input'
  | 'not_configured'
  | 'environment_missing'
  | 'conflict'
  | 'busy'
  | 'unsafe_to_stop'
  | 'confirmation_required'
  | 'startup_failed'
  | 'timeout'
  | 'unauthorized'
  | 'not_found'
  | 'io'

export interface DesktopError {
  category: ErrorCategory
  message: string
  stage: string | null
  recovery: string | null
  details: string | null
}

export interface BenchSummary {
  id: string
  name: string
  path: string
  status: BenchStatus
  ownership: Ownership
  dashboardUrl: string | null
  frappeBranch: string | null
  isProduction: boolean
  isInitialized: boolean
  adminEnabled: boolean
  lastError: DesktopError | null
  notes: string[]
}

export interface OperationResult {
  ok: boolean
  message: string
  bench: BenchSummary | null
  notices: string[]
  generatedAdminPassword: string | null
}

export type CheckStatus = 'ok' | 'missing' | 'incompatible' | 'stopped' | 'unknown'

export interface DependencyCheck {
  id: string
  label: string
  status: CheckStatus
  required: boolean
  path: string | null
  version: string | null
  message: string
  nextAction: string | null
  details: string | null
}

export interface EnvironmentReport {
  checks: DependencyCheck[]
  pilotRoot: string | null
  pilotVersion: string | null
  isReady: boolean
  architecture: string
}

export interface InstallationPlan {
  sourceUrl: string
  steps: string[]
  notes: string[]
}

export interface LogLine {
  bench: string
  stream: 'stdout' | 'stderr' | 'launcher'
  timestampMs: number
  text: string
}

export type EditorChoice = 'cursor' | 'vscode' | 'zed'

export interface Preferences {
  schemaVersion: number
  pilotRoot: string | null
  editor: EditorChoice
  notifications: boolean
  ui: { lastPage: string | null; lastBench: string | null }
}

export type PreferenceUpdate = Pick<Preferences, 'editor' | 'notifications' | 'ui'>

export interface CreateBenchRequest {
  name: string
  frappeBranch: string
}

export interface RuntimeInfo {
  version: string
  architecture: string
  minimumMacos: string
}

export type ProjectTarget = 'editor' | 'finder'
