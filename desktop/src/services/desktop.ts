import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  BenchSummary,
  CreateBenchRequest,
  DesktopError,
  EnvironmentReport,
  InstallationPlan,
  LogLine,
  OperationResult,
  Preferences,
  PreferenceUpdate,
  ProjectTarget,
  RuntimeInfo,
} from '@/types/contracts'

export class DesktopCallError extends Error {
  constructor(readonly detail: DesktopError) {
    super(detail.message)
  }
}

function isDesktopError(value: unknown): value is DesktopError {
  return typeof value === 'object' && value !== null && 'category' in value && 'message' in value
}

export function toDesktopError(error: unknown): DesktopError {
  if (error instanceof DesktopCallError) return error.detail
  if (isDesktopError(error)) return error
  const message = error instanceof Error ? error.message : String(error)
  return { category: 'io', message, stage: null, recovery: null, details: null }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args)
  } catch (error) {
    throw new DesktopCallError(toDesktopError(error))
  }
}

export const desktop = {
  detectEnvironment: () => call<EnvironmentReport>('detect_environment'),
  chooseInstallation: () => call<EnvironmentReport | null>('choose_pilot_installation'),
  planInstallation: () => call<InstallationPlan>('plan_installation'),
  beginInstallation: () => call<void>('begin_installation'),
  runtimeInfo: () => call<RuntimeInfo>('get_runtime_info'),
  listBenches: () => call<BenchSummary[]>('list_benches'),
  createBench: (request: CreateBenchRequest) => call<OperationResult>('create_bench', { request }),
  startBench: (benchId: string) => call<OperationResult>('start_bench', { benchId }),
  cancelStart: (benchId: string) => call<void>('cancel_start', { benchId }),
  stopBench: (benchId: string, confirmAttached: boolean) =>
    call<OperationResult>('stop_bench', { benchId, confirmAttached }),
  restartBench: (benchId: string, confirmAttached: boolean) =>
    call<OperationResult>('restart_bench', { benchId, confirmAttached }),
  openDashboard: (benchId: string) => call<void>('open_dashboard', { benchId }),
  resetDashboardSession: (benchId: string) => call<void>('reset_dashboard_session', { benchId }),
  openProject: (benchId: string, target: ProjectTarget) =>
    call<void>('open_project', { benchId, target }),
  readLogs: (benchId: string) => call<LogLine[]>('read_logs', { benchId }),
  getPreferences: () => call<Preferences>('get_preferences'),
  savePreferences: (update: PreferenceUpdate) => call<Preferences>('save_preferences', { update }),
  quit: (stopOwned: boolean) => call<string[]>('quit_app', { stopOwned }),
}

export type Desktop = typeof desktop

export function onBenchesChanged(handler: (benches: BenchSummary[]) => void): Promise<UnlistenFn> {
  return listen<BenchSummary[]>('benches://changed', (event) => handler(event.payload))
}

export function onLogLine(handler: (line: LogLine) => void): Promise<UnlistenFn> {
  return listen<LogLine>('diagnostics://log', (event) => handler(event.payload))
}

export function onQuitRequested(handler: (ownedBenches: string[]) => void): Promise<UnlistenFn> {
  return listen<string[]>('app://quit-requested', (event) => handler(event.payload))
}
