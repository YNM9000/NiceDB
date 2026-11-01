import { Channel } from '@tauri-apps/api/core';
import {
  commands,
  type DocumentWrite,
  type KeyRead,
  type KeyScan_Deserialize,
  type KeyWrite_Deserialize,
  type ApplyOutcome_Serialize,
  type DesignEngine,
  type TableDesign_Deserialize,
  type AuditEntry_Serialize,
  type Browse_Deserialize,
  type KeyMatch_Deserialize,
  type Summary_Deserialize,
  type SummarySource_Deserialize,
  type ChangePreview,
  type ChangeSet_Deserialize,
  type Page_Serialize,
  type RiskFound,
  type WriteAccess_Serialize,
  type Appearance_Serialize,
  type ConnectionProfile_Serialize,
  type CoreError_Serialize,
  type ErLayout,
  type ExplainRequest,
  type PlanReport_Serialize,
  type HistoryEntry_Serialize,
  type RunEvent_Serialize,
  type RunRequest,
  type Schema_Serialize,
  type Visibility_Serialize,
  type ServerInfo,
  type SessionInfo,
  type SessionEvent_Serialize,
  type ExportFormat,
  type ExportSelection,
  type Folder_Serialize,
  type ImportChoice,
  type ImportOutcome,
  type ImportPreview_Serialize,
  type TreeNode,
  type WorkspaceSession_Serialize,
  type Workspace_Serialize,
  type Workspaces_Serialize,
  type Activity_Serialize,
  type Backed_Serialize,
  type BackupRequest,
  type CompareRequest,
  type RowCompareEvent_Serialize,
  type RowCompareRequest,
  type RowCompared_Serialize,
  type RowProbe_Serialize,
  type RowSource,
  type Comparison_Serialize,
  type DataExport_Deserialize,
  type DataExported,
  type DataImport_Deserialize,
  type DataImportSample,
  type DataImportSource_Deserialize,
  type DataImported,
  type KillMode,
  type Principals_Serialize,
  type SecurityReport_Serialize,
  type Progress_Serialize,
  type RestoreRequest_Deserialize,
  type Sizes_Serialize,
  type Monitor_Serialize,
  type SlowQueries_Serialize,
  type MaintenanceRequest_Deserialize,
  type MaintenanceDone,
  type ToolCheck_Serialize,
  type ToolFamily,
  type NoteScope,
  type Preferences_Serialize,
  type SavedNote,
  type Script_Serialize,
  type Snippet_Serialize,
  type ObjectRef_Deserialize,
  type CommitMode,
  type RowPreview_Serialize,
  type TxState_Serialize,
  type Marks_Serialize,
  type Mark_Serialize,
  type MarkTarget_Serialize,
  type Dialect,
  type ParamError,
  type ParamKind,
  type RunScope,
  type SearchDone,
  type SearchEvent,
  type SearchPlanned_Serialize,
  type SearchSpec,
  type SearchTable,
  type ValueSearch,
  type FakeRequest_Deserialize,
  type GenDone_Serialize,
  type GenPreview_Serialize,
  type GenProfile_Serialize,
  type GenProfiles_Serialize,
  type GenSaved_Serialize,
} from './bindings';
import { m } from '../paraglide/messages';
import type { DriverError_Serialize, GenRequest, PdfReport_Deserialize as PdfReport } from './bindings';
import { fromBase64, toBase64 } from './bytes';
import { backend as mock } from './mock';
import { initFailure } from './session';
import { varFault } from './vars';
import { unpackSchema } from './schema-wire';
import type { NamespaceSize } from './bindings';

export interface SchemaOpened {
  schema: Schema;
  pending: NamespaceSize[];
}
import { aboutReal } from './about-api';

export type Appearance = Appearance_Serialize;
export type ConnectionProfile = ConnectionProfile_Serialize;
export type CoreError = CoreError_Serialize;
export type HistoryEntry = HistoryEntry_Serialize;
export type RunEvent = RunEvent_Serialize;
export type SessionEvent = SessionEvent_Serialize;
export type Schema = Schema_Serialize;
export type Visibility = Visibility_Serialize;
export type ApplyOutcome = ApplyOutcome_Serialize;
export type AuditEntry = AuditEntry_Serialize;
export type Browse = Browse_Deserialize;
export type ChangeSet = ChangeSet_Deserialize;
export type KeyMatch = KeyMatch_Deserialize;
export type Page = Page_Serialize;
export type WriteAccess = WriteAccess_Serialize;
export type TxState = TxState_Serialize;
export type RowPreview = RowPreview_Serialize;
export type { CommitMode };
export type { ChangePreview, RiskFound };
export type Secrets = { password: string | null; ssh: string | null; proxy?: string | null; tls_key?: string | null; jumps?: (string | null)[] };
export type { ErLayout, ExplainRequest, RunRequest, ServerInfo, SessionInfo };
export type PlanReport = PlanReport_Serialize;
export type Folder = Folder_Serialize;
export type Workspace = Workspace_Serialize;
export type WorkspaceSession = WorkspaceSession_Serialize;
export type Workspaces = Workspaces_Serialize;
export type ImportPreview = ImportPreview_Serialize;
export type { ExportFormat, ExportSelection, ImportChoice, ImportOutcome, TreeNode };
export type Preferences = Preferences_Serialize;
export type Marks = Marks_Serialize;
export type Mark = Mark_Serialize;
export type MarkTarget = MarkTarget_Serialize;
export type Script = Script_Serialize;
export type Snippet = Snippet_Serialize;
export type { NoteScope, SavedNote };
export type Progress = Progress_Serialize;
export type Activity = Activity_Serialize;
export type Sizes = Sizes_Serialize;
export type Monitor = Monitor_Serialize;
export type SlowQueries = SlowQueries_Serialize;
export type Principals = Principals_Serialize;
export type SecurityReport = SecurityReport_Serialize;
export type Comparison = Comparison_Serialize;
export type RowCompareEvent = RowCompareEvent_Serialize;
export type RowCompared = RowCompared_Serialize;
export type RowProbe = RowProbe_Serialize;
export type { RowCompareRequest, RowSource };
export type ToolCheck = ToolCheck_Serialize;
export type SearchPlanned = SearchPlanned_Serialize;
export type { SearchDone, SearchEvent, SearchSpec, SearchTable, ValueSearch };
export type FakeRequest = FakeRequest_Deserialize;
export type GenProfile = GenProfile_Serialize;
export type GenProfiles = GenProfiles_Serialize;
export type GenSaved = GenSaved_Serialize;
export type GenPreview = GenPreview_Serialize;
export type GenDone = GenDone_Serialize;
export type Backed = Backed_Serialize;
export type DataExport = DataExport_Deserialize;
export type DataImport = DataImport_Deserialize;
export type DataImportSource = DataImportSource_Deserialize;
export type RestoreRequest = RestoreRequest_Deserialize;
export type { BackupRequest, CompareRequest, DataExported, DataImportSample, DataImported, KillMode, ToolFamily };

function progressChannel(onProgress: (p: Progress) => void) {
  const channel = new Channel<Progress>();
  channel.onmessage = onProgress;
  return channel as never;
}

export const defaultAppearance: Appearance = { style: 'listino', palette: '2', theme: 'dark', locale: 'it', ui_scale: 100, sounds: true, sound_volume: 30, sound_detail: 'all', sound_timbre: 'marimba', sound_gestures: false, jingle: true, setup_done: false };

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export class ApiError extends Error {
  constructor(readonly error: CoreError) {
    super(describe(error));
  }
}

export function driverMessage(e: DriverError_Serialize): string {
  if (e.kind === 'cancelled') return m.results_cancelled();
  if (e.kind === 'timeout') return m.err_timeout({ seconds: e.seconds });
  return e.message;
}

const FAULT = {
  needs_passphrase: m.org_fault_needs_passphrase,
  wrong_passphrase: m.org_fault_wrong_passphrase,
  malformed: m.org_fault_malformed,
  unsupported: m.org_fault_unsupported,
  wrong_extension: m.org_fault_wrong_extension,
};

function toolFault(f: Extract<CoreError, { code: 'tool' }>['detail']): string {
  switch (f.kind) {
    case 'unsupported':
      return m.tool_fault_unsupported();
    case 'missing':
      return m.tool_fault_missing({ tool: f.tool });
    case 'failed':
      return m.tool_fault_failed({ tool: f.tool, message: f.message });
    case 'not_read':
      return m.tool_fault_not_read();
    case 'wrong_extension':
      return m.tool_fault_extension();
    case 'file':
      return m.tool_fault_file({ message: f.message });
    case 'empty':
      return m.tool_fault_empty();
    case 'production':
      return m.tool_fault_production();
    case 'key_missing':
      return m.rows_fault_key_missing({ column: f.column });
    case 'duplicate_key':
      return m.rows_fault_duplicate({ side: (f.side === 'a' ? m.tool_cmp_target() : m.tool_cmp_source()).toLowerCase(), key: f.key.map((v) => v ?? 'NULL').join(', ') });
    case 'too_large':
      return m.rows_fault_too_large({ rows: new Intl.NumberFormat().format(f.rows ?? 0), limit: f.limit_mb });
  }
}

export function paramFault(e: ParamError, label?: string): string {
  const name = label ?? (/^\d+$/.test(e.name) ? `?${e.name}` : `:${e.name}`);
  if (e.kind === 'missing') return m.params_err_missing({ name });
  return e.as_kind === 'raw' ? m.params_err_raw({ name }) : m.params_err_invalid({ name, kind: paramKindLabel(e.as_kind).toLowerCase() });
}

export function paramKindLabel(kind: ParamKind): string {
  return { text: m.params_kind_text, number: m.params_kind_number, date: m.params_kind_date, boolean: m.params_kind_boolean, null: m.params_kind_null, raw: m.params_kind_raw }[kind]();
}

export function describe(e: CoreError): string {
  switch (e.code) {
    case 'driver':
      return e.detail.kind === 'cancelled' ? 'cancelled' : driverMessage(e.detail);
    case 'read_only':
      return m.err_read_only();
    case 'needs_confirmation':
      return m.err_needs_confirmation();
    case 'name_mismatch':
      return m.err_name_mismatch();
    case 'no_key':
      return m.err_no_key();
    case 'invalid_value':
      return m.data_bad_hex({ column: e.detail });
    case 'transaction_open':
      return m.err_transaction_open();
    case 'transaction_failed':
      return m.tx_err_failed();
    case 'not_dry_runnable':
      return m.tx_err_not_dry({ index: e.detail.index + 1, sql: e.detail.sql.length > 80 ? `${e.detail.sql.slice(0, 80)}…` : e.detail.sql });
    case 'single_database':
      return m.err_single_database();
    case 'busy':
      return m.err_busy();
    case 'risky':
      return m.err_risky();
    case 'plan_unavailable':
      return m.err_plan_unavailable();
    case 'tunnel':
      return 'message' in e.detail ? e.detail.message : e.detail.kind;
    case 'package':
      return FAULT[e.detail]();
    case 'tool':
      return toolFault(e.detail);
    case 'init':
      return initFailure(e.detail);
    case 'param':
      return paramFault(e.detail);
    case 'variable':
      return varFault(e.detail);
    case 'model':
      return m.err_model();
    case 'io':
    case 'secrets':
    case 'store':
    case 'not_found':
      return e.detail;
    default:
      return e.code;
  }
}

type Result<T> = { status: 'ok'; data: T } | { status: 'error'; error: CoreError };

async function unwrap<T>(r: Promise<Result<T>>): Promise<T> {
  const res = await r;
  if (res.status === 'error') throw new ApiError(res.error);
  return res.data;
}

const real = {
  appearanceGet: () => unwrap(commands.appearanceGet()),
  appearanceSet: async (v: Appearance): Promise<void> => {
    await unwrap(commands.appearanceSet(v));
  },
  connectionsList: () => unwrap(commands.connectionsList()),
  connectionSave: async (p: ConnectionProfile, s: Secrets, confirm: string | null = null): Promise<void> => {
    await unwrap(commands.connectionSave(p, s, confirm));
  },
  connectionDelete: async (id: string): Promise<void> => {
    await unwrap(commands.connectionDelete(id));
  },
  connectionTrash: async (id: string): Promise<void> => {
    await unwrap(commands.connectionTrash(id));
  },
  connectionRestore: async (p: ConnectionProfile, confirm: string | null = null): Promise<void> => {
    await unwrap(commands.connectionRestore(p, confirm));
  },
  connectionPurge: async (id: string): Promise<void> => {
    await unwrap(commands.connectionPurge(id));
  },
  connectionTest: (p: ConnectionProfile, s: Secrets) => unwrap(commands.connectionTest(p, s)),
  varsPreview: (connectionId: string | null, folder: string | null, texts: string[]) => unwrap(commands.varsPreview(connectionId, folder, texts)),
  hostKeyTrust: async (host: string, port: number, key: string): Promise<void> => {
    await unwrap(commands.hostKeyTrust(host, port, key));
  },
  sessionOpen: (id: string, database: string | null = null) => unwrap(commands.sessionOpen(id, database)),
  sessionSwitch: (sessionId: string, database: string) => unwrap(commands.sessionSwitch(sessionId, database)),
  sessionReconnect: (sessionId: string) => unwrap(commands.sessionReconnect(sessionId)),
  sessionWatch: async (onEvent: (e: SessionEvent) => void): Promise<void> => {
    const channel = new Channel<SessionEvent>();
    channel.onmessage = onEvent;
    await unwrap(commands.sessionWatch(channel as never));
  },
  visibilityGet: (connectionId: string) => unwrap(commands.visibilityGet(connectionId)),
  visibilitySet: async (connectionId: string, visibility: Visibility): Promise<void> => {
    await unwrap(commands.visibilitySet(connectionId, visibility));
  },
  sessionClose: async (id: string): Promise<void> => {
    await unwrap(commands.sessionClose(id));
  },
  schemaLoad: async (id: string) => unpackSchema(await unwrap(commands.schemaLoad(id))),
  schemaOpen: async (id: string): Promise<SchemaOpened> => {
    const start = await unwrap(commands.schemaOpen(id));
    return { schema: unpackSchema(start.schema), pending: start.pending };
  },
  schemaNamespace: async (id: string, namespace: string) => unpackSchema(await unwrap(commands.schemaNamespace(id, namespace))),
  searchPath: (sessionId: string) => unwrap(commands.searchPath(sessionId)),
  objectDdl: (sessionId: string, object: ObjectRef_Deserialize) => unwrap(commands.objectDdl(sessionId, object)),
  objectDependencies: (sessionId: string, object: ObjectRef_Deserialize) => unwrap(commands.objectDependencies(sessionId, object)),
  sqlGenerate: (dialect: Dialect, request: GenRequest) => commands.sqlGenerate(dialect, request),
  run: (request: RunRequest, onEvent: (e: RunEvent) => void): Promise<void> => {
    const channel = new Channel<RunEvent>();
    channel.onmessage = onEvent;
    return unwrap(commands.run(request, channel as never)).then(() => {});
  },
  runCancel: async (id: string): Promise<void> => {
    await unwrap(commands.runCancel(id));
  },
  explain: (request: ExplainRequest) => unwrap(commands.explain(request)),
  paramsFound: (text: string, dialect: Dialect, scope: RunScope) => unwrap(commands.paramsFound(text, dialect, scope)),
  historyList: (connectionId: string | null, query: string, limit: number) => unwrap(commands.historyList(connectionId, query, limit)),
  erLayoutGet: (connectionId: string, database: string) => unwrap(commands.erLayoutGet(connectionId, database)),
  erLayoutSet: async (connectionId: string, database: string, layout: ErLayout): Promise<void> => {
    await unwrap(commands.erLayoutSet(connectionId, database, layout));
  },
  exportSave: (name: string, data: Uint8Array): Promise<boolean> => unwrap(commands.exportSave(name, toBase64(data))),
  fileLoad: async (): Promise<{ name: string; bytes: Uint8Array } | null> => {
    const file = await unwrap(commands.fileLoad());
    return file && { name: file.name, bytes: fromBase64(file.data) };
  },
  reportPdf: (name: string, report: PdfReport): Promise<boolean> => unwrap(commands.reportPdf(name, report)),
  dataBrowse: (sessionId: string, browse: Browse) => unwrap(commands.dataBrowse(sessionId, browse)),
  dataPick: (sessionId: string, browse: Browse, jobId: string) => unwrap(commands.dataPick(sessionId, browse, jobId)),
  tableIdentity: (sessionId: string, namespace: string, name: string) => unwrap(commands.tableIdentity(sessionId, namespace, name)),
  dataRelated: (sessionId: string, matches: KeyMatch[]) => unwrap(commands.dataRelated(sessionId, matches)),
  databaseCounts: (sessionId: string) => unwrap(commands.databaseCounts(sessionId)),
  dataSummarize: (sessionId: string, source: SummarySource_Deserialize, summary: Summary_Deserialize) => unwrap(commands.dataSummarize(sessionId, source, summary)),
  dataPreview: (sessionId: string, changes: ChangeSet) => unwrap(commands.dataPreview(sessionId, changes)),
  dataEstimate: (sessionId: string, changes: ChangeSet) => unwrap(commands.dataEstimate(sessionId, changes)),
  dataApply: (sessionId: string, changes: ChangeSet, confirm: string | null) => unwrap(commands.dataApply(sessionId, changes, confirm)),
  documentsWrite: (sessionId: string, write: DocumentWrite, confirm: string | null) => unwrap(commands.documentsWrite(sessionId, write, confirm)),
  keysScan: (sessionId: string, scan: KeyScan_Deserialize) => unwrap(commands.keysScan(sessionId, scan)),
  keyRead: (sessionId: string, read: KeyRead) => unwrap(commands.keyRead(sessionId, read)),
  keyWrite: (sessionId: string, write: KeyWrite_Deserialize, confirm: string | null) => unwrap(commands.keyWrite(sessionId, write, confirm)),
  serverReport: (sessionId: string) => unwrap(commands.serverReport(sessionId)),
  writeAccess: (sessionId: string) => unwrap(commands.writeAccess(sessionId)),
  writesUnlock: (sessionId: string) => unwrap(commands.writesUnlock(sessionId)),
  writesLock: (sessionId: string) => unwrap(commands.writesLock(sessionId)),
  auditList: (connectionId: string, limit: number) => unwrap(commands.auditList(connectionId, limit)),
  txState: (sessionId: string) => unwrap(commands.txState(sessionId)),
  txModeSet: (sessionId: string, mode: CommitMode) => unwrap(commands.txModeSet(sessionId, mode)),
  txCommit: (sessionId: string, confirm: string | null) => unwrap(commands.txCommit(sessionId, confirm)),
  txRollback: (sessionId: string) => unwrap(commands.txRollback(sessionId)),
  scriptsList: () => unwrap(commands.scriptsList()),
  scriptSave: (script: Script) => unwrap(commands.scriptSave(script)),
  scriptDelete: async (id: string): Promise<void> => {
    await unwrap(commands.scriptDelete(id));
  },
  snippetsList: () => unwrap(commands.snippetsList()),
  snippetSave: async (snippet: Snippet): Promise<void> => {
    await unwrap(commands.snippetSave(snippet));
  },
  snippetDelete: async (id: string): Promise<void> => {
    await unwrap(commands.snippetDelete(id));
  },
  notesList: () => unwrap(commands.notesList()),
  noteSet: (scope: NoteScope, scopeId: string, body: string) => unwrap(commands.noteSet(scope, scopeId, body)),
  preferencesGet: () => unwrap(commands.preferencesGet()),
  preferencesSet: (value: Preferences, confirm: string | null = null) => unwrap(commands.preferencesSet(value, confirm)),
  marksGet: () => unwrap(commands.marksGet()),
  marksSet: (value: Marks) => unwrap(commands.marksSet(value)),
  foldersList: () => unwrap(commands.foldersList()),
  folderSave: async (f: Folder): Promise<void> => {
    await unwrap(commands.folderSave(f));
  },
  folderDelete: async (id: string): Promise<void> => {
    await unwrap(commands.folderDelete(id));
  },
  nodeMove: async (node: TreeNode, parent: string | null, index: number): Promise<void> => {
    await unwrap(commands.nodeMove(node, parent, index));
  },
  workspacesGet: () => unwrap(commands.workspacesGet()),
  workspaceSave: async (w: Workspace): Promise<void> => {
    await unwrap(commands.workspaceSave(w));
  },
  workspaceDelete: async (id: string): Promise<void> => {
    await unwrap(commands.workspaceDelete(id));
  },
  workspaceSessionSave: async (id: string | null, session: WorkspaceSession): Promise<void> => {
    await unwrap(commands.workspaceSessionSave(id, session));
  },
  workspaceActivate: async (id: string | null): Promise<void> => {
    await unwrap(commands.workspaceActivate(id));
  },
  packageExport: async (name: string, selection: ExportSelection, format: ExportFormat): Promise<boolean> => {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const ext = format.kind === 'sealed' ? 'nicedb' : 'json';
    const path = await save({ defaultPath: `${name}.${ext}`, filters: [{ name: 'NiceDB', extensions: [ext] }] });
    if (!path) return false;
    await unwrap(commands.packageExport(path, selection, format));
    return true;
  },
  packagePick: async (): Promise<string | null> => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({ multiple: false, directory: false, filters: [{ name: 'NiceDB', extensions: ['nicedb', 'json'] }] });
    return typeof path === 'string' ? path : null;
  },
  packagePreview: (path: string, passphrase: string | null) => unwrap(commands.packagePreview(path, passphrase)),
  packageImport: (path: string, passphrase: string | null, choices: ImportChoice[]) => unwrap(commands.packageImport(path, passphrase, choices)),
  pickSave: async (name: string, extensions: string[], label: string): Promise<string | null> => {
    const { save } = await import('@tauri-apps/plugin-dialog');
    return (await save({ defaultPath: name, filters: [{ name: label, extensions }] })) ?? null;
  },
  pickOpen: async (extensions: string[], label: string): Promise<string | null> => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({ multiple: false, directory: false, filters: [{ name: label, extensions }] });
    return typeof path === 'string' ? path : null;
  },
  pickFolder: async (_purpose: 'clidriver' | 'tools' = 'tools'): Promise<string | null> => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const path = await open({ multiple: false, directory: true });
    return typeof path === 'string' ? path : null;
  },
  dataExport: (request: DataExport, onProgress: (p: Progress) => void) => unwrap(commands.dataExport(request, progressChannel(onProgress))),
  dataImportSample: (sessionId: string, source: DataImportSource) => unwrap(commands.dataImportSample(sessionId, source)),
  dataImport: (request: DataImport, onProgress: (p: Progress) => void) => unwrap(commands.dataImport(request, progressChannel(onProgress))),
  jobCancel: async (id: string): Promise<void> => {
    await unwrap(commands.jobCancel(id));
  },
  schemaCompare: (request: CompareRequest) => unwrap(commands.schemaCompare(request)),
  tableDesign: (sessionId: string, namespace: string, name: string) => unwrap(commands.tableDesign(sessionId, namespace, name)),
  tableDesignScript: (engine: DesignEngine, before: TableDesign_Deserialize | null, after: TableDesign_Deserialize) => commands.tableDesignScript(engine, before, after),
  rowProbe: (source: RowSource) => unwrap(commands.rowProbe(source)),
  rowCompare: (request: RowCompareRequest, onEvent: (e: RowCompareEvent) => void) => {
    const channel = new Channel<RowCompareEvent>();
    channel.onmessage = onEvent;
    return unwrap(commands.rowCompare(request, channel as never));
  },
  rowScript: (compareId: string, namespace: string, name: string) => unwrap(commands.rowScript(compareId, namespace, name)),
  rowForget: async (compareId: string): Promise<void> => {
    await unwrap(commands.rowForget(compareId));
  },
  backupTools: (sessionId: string) => unwrap(commands.backupTools(sessionId)),
  toolDirSet: async (family: ToolFamily, dir: string | null): Promise<void> => {
    await unwrap(commands.toolDirSet(family, dir));
  },
  backupRun: (request: BackupRequest, onProgress: (p: Progress) => void) => unwrap(commands.backupRun(request, progressChannel(onProgress))),
  backupRestore: (request: RestoreRequest, onProgress: (p: Progress) => void) => unwrap(commands.backupRestore(request, progressChannel(onProgress))),
  serverActivity: (sessionId: string) => unwrap(commands.serverActivity(sessionId)),
  serverSizes: (sessionId: string) => unwrap(commands.serverSizes(sessionId)),
  serverMonitor: (sessionId: string) => unwrap(commands.serverMonitor(sessionId)),
  serverSlowQueries: (sessionId: string) => unwrap(commands.serverSlowQueries(sessionId)),
  serverMaintain: (request: MaintenanceRequest_Deserialize): Promise<MaintenanceDone> => unwrap(commands.serverMaintain(request)),
  serverKill: (sessionId: string, target: string, mode: KillMode, confirm: string | null) => unwrap(commands.serverKill(sessionId, target, mode, confirm)),
  clickhouseAccess: (sessionId: string) => unwrap(commands.clickhouseAccess(sessionId)),
  principalsList: (sessionId: string) => unwrap(commands.principalsList(sessionId)),
  securityCheck: (sessionId: string) => unwrap(commands.securityCheck(sessionId)),
  valueSearchPlan: (sessionId: string, spec: SearchSpec, tables: SearchTable[]): Promise<SearchPlanned> => unwrap(commands.valueSearchPlan(sessionId, spec, tables)),
  valueSearch: (request: ValueSearch, onEvent: (e: SearchEvent) => void): Promise<SearchDone> => {
    const channel = new Channel<SearchEvent>();
    channel.onmessage = onEvent;
    return unwrap(commands.valueSearch(request, channel as never));
  },
  genProfile: (sessionId: string, namespace: string, name: string): Promise<GenProfile> => unwrap(commands.genProfile(sessionId, namespace, name)),
  genPreview: (request: FakeRequest): Promise<GenPreview> => unwrap(commands.genPreview(request)),
  genRun: (request: FakeRequest, onProgress: (p: Progress) => void): Promise<GenDone> => unwrap(commands.genRun(request, progressChannel(onProgress))),
  genProfilesGet: (): Promise<GenProfiles> => unwrap(commands.genProfilesGet()),
  genProfileSave: (profile: GenSaved): Promise<GenProfiles> => unwrap(commands.genProfileSave(profile)),
  genProfileDelete: (id: string): Promise<GenProfiles> => unwrap(commands.genProfileDelete(id)),
  ...aboutReal,
};

export type Api = typeof real;

export const api: Api = inTauri ? real : mock;
