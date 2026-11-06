// @ts-nocheck

import { invoke as __TAURI_INVOKE, Channel } from "@tauri-apps/api/core";

export const commands = {
	appearanceGet: () => typedError<Appearance_Serialize, CoreError_Serialize>(__TAURI_INVOKE("appearance_get")),
	appearanceSet: (value: Appearance_Deserialize) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("appearance_set", { value })),
	erLayoutGet: (connectionId: string, database: string) => typedError<ErLayout | null, CoreError_Serialize>(__TAURI_INVOKE("er_layout_get", { connectionId, database })),
	erLayoutSet: (connectionId: string, database: string, layout: ErLayout) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("er_layout_set", { connectionId, database, layout })),
	connectionsList: () => typedError<ConnectionProfile_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("connections_list")),
	connectionSave: (profile: ConnectionProfile_Deserialize, secrets: Secrets_Deserialize, confirm: string | null) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("connection_save", { profile, secrets, confirm })),
	connectionDelete: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("connection_delete", { id })),
	connectionTrash: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("connection_trash", { id })),
	connectionRestore: (profile: ConnectionProfile_Deserialize, confirm: string | null) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("connection_restore", { profile, confirm })),
	connectionPurge: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("connection_purge", { id })),
	connectionTest: (profile: ConnectionProfile_Deserialize, secrets: Secrets_Deserialize) => typedError<ServerInfo, CoreError_Serialize>(__TAURI_INVOKE("connection_test", { profile, secrets })),
	varsPreview: (connectionId: string | null, folder: string | null, texts: string[]) => typedError<VarPreview[], CoreError_Serialize>(__TAURI_INVOKE("vars_preview", { connectionId, folder, texts })),
	hostKeyTrust: (host: string, port: number, key: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("host_key_trust", { host, port, key })),
	sessionOpen: (connectionId: string, database: string | null) => typedError<SessionInfo_Serialize, CoreError_Serialize>(__TAURI_INVOKE("session_open", { connectionId, database })),
	sessionSwitch: (sessionId: string, database: string) => typedError<SessionInfo_Serialize, CoreError_Serialize>(__TAURI_INVOKE("session_switch", { sessionId, database })),
	sessionReconnect: (sessionId: string) => typedError<SessionInfo_Serialize, CoreError_Serialize>(__TAURI_INVOKE("session_reconnect", { sessionId })),
	sessionWatch: (events: Channel<SessionEvent_Deserialize>) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("session_watch", { events })),
	visibilityGet: (connectionId: string) => typedError<Visibility_Serialize, CoreError_Serialize>(__TAURI_INVOKE("visibility_get", { connectionId })),
	visibilitySet: (connectionId: string, visibility: Visibility_Deserialize) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("visibility_set", { connectionId, visibility })),
	sessionClose: (sessionId: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("session_close", { sessionId })),
	schemaLoad: (sessionId: string) => typedError<PackedSchema_Serialize, CoreError_Serialize>(__TAURI_INVOKE("schema_load", { sessionId })),
	schemaOpen: (sessionId: string) => typedError<SchemaStart_Serialize, CoreError_Serialize>(__TAURI_INVOKE("schema_open", { sessionId })),
	schemaNamespace: (sessionId: string, namespace: string) => typedError<PackedSchema_Serialize, CoreError_Serialize>(__TAURI_INVOKE("schema_namespace", { sessionId, namespace })),
	searchPath: (sessionId: string) => typedError<string[] | null, CoreError_Serialize>(__TAURI_INVOKE("search_path", { sessionId })),
	objectDdl: (sessionId: string, object: ObjectRef_Deserialize) => typedError<string, CoreError_Serialize>(__TAURI_INVOKE("object_ddl", { sessionId, object })),
	objectDependencies: (sessionId: string, object: ObjectRef_Deserialize) => typedError<Dependency_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("object_dependencies", { sessionId, object })),
	sqlGenerate: (dialect: Dialect, request: GenRequest_Deserialize) => __TAURI_INVOKE<string | null>("sql_generate", { dialect, request }),
	run: (request: RunRequest_Deserialize, events: Channel<RunEvent_Deserialize>) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("run", { request, events })),
	explain: (request: ExplainRequest_Deserialize) => typedError<PlanReport_Serialize, CoreError_Serialize>(__TAURI_INVOKE("explain", { request })),
	paramsFound: (text: string, dialect: Dialect, scope: RunScope) => typedError<ParamsFound, CoreError_Serialize>(__TAURI_INVOKE("params_found", { text, dialect, scope })),
	runCancel: (runId: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("run_cancel", { runId })),
	historyList: (connectionId: string | null, query: string, limit: number) => typedError<HistoryEntry_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("history_list", { connectionId, query, limit })),
	exportSave: (name: string, data: string) => typedError<boolean, CoreError_Serialize>(__TAURI_INVOKE("export_save", { name, data })),
	fileLoad: () => typedError<LoadedFile | null, CoreError_Serialize>(__TAURI_INVOKE("file_load")),
	reportPdf: (name: string, report: PdfReport_Deserialize) => typedError<boolean, CoreError_Serialize>(__TAURI_INVOKE("report_pdf", { name, report })),
	dataBrowse: (sessionId: string, browse: Browse_Deserialize) => typedError<Page_Serialize, CoreError_Serialize>(__TAURI_INVOKE("data_browse", { sessionId, browse })),
	dataPick: (sessionId: string, browse: Browse_Deserialize, jobId: string) => typedError<Page_Serialize, CoreError_Serialize>(__TAURI_INVOKE("data_pick", { sessionId, browse, jobId })),
	tableIdentity: (sessionId: string, namespace: string, name: string) => typedError<IdentityColumn[], CoreError_Serialize>(__TAURI_INVOKE("table_identity", { sessionId, namespace, name })),
	dataRelated: (sessionId: string, matches: KeyMatch_Deserialize[]) => typedError<(number | null)[], CoreError_Serialize>(__TAURI_INVOKE("data_related", { sessionId, matches })),
	dataSummarize: (sessionId: string, source: SummarySource_Deserialize, summary: Summary_Deserialize) => typedError<((string | null)[])[], CoreError_Serialize>(__TAURI_INVOKE("data_summarize", { sessionId, source, summary })),
	databaseCounts: (sessionId: string) => typedError<([string, number | null])[], CoreError_Serialize>(__TAURI_INVOKE("database_counts", { sessionId })),
	dataPreview: (sessionId: string, changes: ChangeSet_Deserialize) => typedError<ChangePreview, CoreError_Serialize>(__TAURI_INVOKE("data_preview", { sessionId, changes })),
	dataEstimate: (sessionId: string, changes: ChangeSet_Deserialize) => typedError<(number | null)[], CoreError_Serialize>(__TAURI_INVOKE("data_estimate", { sessionId, changes })),
	dataApply: (sessionId: string, changes: ChangeSet_Deserialize, confirm: string | null) => typedError<ApplyOutcome_Serialize, CoreError_Serialize>(__TAURI_INVOKE("data_apply", { sessionId, changes, confirm })),
	documentsWrite: (sessionId: string, write: DocumentWrite, confirm: string | null) => typedError<Outcome_Serialize, CoreError_Serialize>(__TAURI_INVOKE("documents_write", { sessionId, write, confirm })),
	keysScan: (sessionId: string, scan: KeyScan_Deserialize) => typedError<KeyPage_Serialize, CoreError_Serialize>(__TAURI_INVOKE("keys_scan", { sessionId, scan })),
	keyRead: (sessionId: string, read: KeyRead) => typedError<KeyValue_Serialize, CoreError_Serialize>(__TAURI_INVOKE("key_read", { sessionId, read })),
	keyWrite: (sessionId: string, write: KeyWrite_Deserialize, confirm: string | null) => typedError<Outcome_Serialize, CoreError_Serialize>(__TAURI_INVOKE("key_write", { sessionId, write, confirm })),
	serverReport: (sessionId: string) => typedError<Report, CoreError_Serialize>(__TAURI_INVOKE("server_report", { sessionId })),
	writeAccess: (sessionId: string) => typedError<WriteAccess_Serialize, CoreError_Serialize>(__TAURI_INVOKE("write_access", { sessionId })),
	writesUnlock: (sessionId: string) => typedError<WriteAccess_Serialize, CoreError_Serialize>(__TAURI_INVOKE("writes_unlock", { sessionId })),
	writesLock: (sessionId: string) => typedError<WriteAccess_Serialize, CoreError_Serialize>(__TAURI_INVOKE("writes_lock", { sessionId })),
	txState: (sessionId: string) => typedError<TxState_Serialize, CoreError_Serialize>(__TAURI_INVOKE("tx_state", { sessionId })),
	txModeSet: (sessionId: string, mode: CommitMode) => typedError<TxState_Serialize, CoreError_Serialize>(__TAURI_INVOKE("tx_mode_set", { sessionId, mode })),
	txCommit: (sessionId: string, confirm: string | null) => typedError<TxState_Serialize, CoreError_Serialize>(__TAURI_INVOKE("tx_commit", { sessionId, confirm })),
	txRollback: (sessionId: string) => typedError<TxState_Serialize, CoreError_Serialize>(__TAURI_INVOKE("tx_rollback", { sessionId })),
	auditList: (connectionId: string, limit: number) => typedError<AuditEntry_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("audit_list", { connectionId, limit })),
	scriptsList: () => typedError<Script_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("scripts_list")),
	scriptSave: (script: Script_Deserialize) => typedError<Script_Serialize, CoreError_Serialize>(__TAURI_INVOKE("script_save", { script })),
	scriptDelete: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("script_delete", { id })),
	snippetsList: () => typedError<Snippet_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("snippets_list")),
	snippetSave: (snippet: Snippet_Deserialize) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("snippet_save", { snippet })),
	snippetDelete: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("snippet_delete", { id })),
	notesList: () => typedError<SavedNote[], CoreError_Serialize>(__TAURI_INVOKE("notes_list")),
	noteSet: (scope: NoteScope, scopeId: string, body: string) => typedError<SavedNote | null, CoreError_Serialize>(__TAURI_INVOKE("note_set", { scope, scopeId, body })),
	preferencesGet: () => typedError<Preferences_Serialize, CoreError_Serialize>(__TAURI_INVOKE("preferences_get")),
	preferencesSet: (value: Preferences_Deserialize, confirm: string | null) => typedError<Preferences_Serialize, CoreError_Serialize>(__TAURI_INVOKE("preferences_set", { value, confirm })),
	marksGet: () => typedError<Marks_Serialize, CoreError_Serialize>(__TAURI_INVOKE("marks_get")),
	marksSet: (value: Marks_Deserialize) => typedError<Marks_Serialize, CoreError_Serialize>(__TAURI_INVOKE("marks_set", { value })),
	genProfilesGet: () => typedError<GenProfiles_Serialize, CoreError_Serialize>(__TAURI_INVOKE("gen_profiles_get")),
	genProfileSave: (profile: GenSaved_Deserialize) => typedError<GenProfiles_Serialize, CoreError_Serialize>(__TAURI_INVOKE("gen_profile_save", { profile })),
	genProfileDelete: (id: string) => typedError<GenProfiles_Serialize, CoreError_Serialize>(__TAURI_INVOKE("gen_profile_delete", { id })),
	foldersList: () => typedError<Folder_Serialize[], CoreError_Serialize>(__TAURI_INVOKE("folders_list")),
	folderSave: (folder: Folder_Deserialize) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("folder_save", { folder })),
	folderDelete: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("folder_delete", { id })),
	nodeMove: (node: TreeNode, parent: string | null, index: number) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("node_move", { node, parent, index })),
	workspacesGet: () => typedError<Workspaces_Serialize, CoreError_Serialize>(__TAURI_INVOKE("workspaces_get")),
	workspaceSave: (workspace: Workspace_Deserialize) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("workspace_save", { workspace })),
	workspaceDelete: (id: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("workspace_delete", { id })),
	workspaceSessionSave: (id: string | null, session: WorkspaceSession_Deserialize) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("workspace_session_save", { id, session })),
	workspaceActivate: (id: string | null) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("workspace_activate", { id })),
	packageExport: (path: string, selection: ExportSelection, format: ExportFormat) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("package_export", { path, selection, format })),
	packagePreview: (path: string, passphrase: string | null) => typedError<ImportPreview_Serialize, CoreError_Serialize>(__TAURI_INVOKE("package_preview", { path, passphrase })),
	packageImport: (path: string, passphrase: string | null, choices: ImportChoice[]) => typedError<ImportOutcome, CoreError_Serialize>(__TAURI_INVOKE("package_import", { path, passphrase, choices })),
	dataExport: (request: DataExport_Deserialize, events: Channel<Progress_Deserialize>) => typedError<DataExported, CoreError_Serialize>(__TAURI_INVOKE("data_export", { request, events })),
	dataImportSample: (sessionId: string, source: DataImportSource_Deserialize) => typedError<DataImportSample, CoreError_Serialize>(__TAURI_INVOKE("data_import_sample", { sessionId, source })),
	dataImport: (request: DataImport_Deserialize, events: Channel<Progress_Deserialize>) => typedError<DataImported, CoreError_Serialize>(__TAURI_INVOKE("data_import", { request, events })),
	jobCancel: (jobId: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("job_cancel", { jobId })),
	schemaCompare: (request: CompareRequest) => typedError<Comparison_Serialize, CoreError_Serialize>(__TAURI_INVOKE("schema_compare", { request })),
	tableDesign: (sessionId: string, namespace: string, name: string) => typedError<TableDesign_Serialize, CoreError_Serialize>(__TAURI_INVOKE("table_design", { sessionId, namespace, name })),
	tableDesignScript: (engine: DesignEngine, before: TableDesign_Deserialize | null, after: TableDesign_Deserialize) => __TAURI_INVOKE<DesignScript_Serialize>("table_design_script", { engine, before, after }),
	rowProbe: (source: RowSource) => typedError<RowProbe_Serialize, CoreError_Serialize>(__TAURI_INVOKE("row_probe", { source })),
	rowCompare: (request: RowCompareRequest, events: Channel<RowCompareEvent_Deserialize>) => typedError<RowCompared_Serialize, CoreError_Serialize>(__TAURI_INVOKE("row_compare", { request, events })),
	rowScript: (compareId: string, namespace: string, name: string) => typedError<string[], CoreError_Serialize>(__TAURI_INVOKE("row_script", { compareId, namespace, name })),
	rowForget: (compareId: string) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("row_forget", { compareId })),
	backupTools: (sessionId: string) => typedError<ToolCheck_Serialize, CoreError_Serialize>(__TAURI_INVOKE("backup_tools", { sessionId })),
	toolDirSet: (family: ToolFamily, dir: string | null) => typedError<null, CoreError_Serialize>(__TAURI_INVOKE("tool_dir_set", { family, dir })),
	backupRun: (request: BackupRequest, events: Channel<Progress_Deserialize>) => typedError<Backed_Serialize, CoreError_Serialize>(__TAURI_INVOKE("backup_run", { request, events })),
	backupRestore: (request: RestoreRequest_Deserialize, events: Channel<Progress_Deserialize>) => typedError<Backed_Serialize, CoreError_Serialize>(__TAURI_INVOKE("backup_restore", { request, events })),
	serverActivity: (sessionId: string) => typedError<Activity_Serialize, CoreError_Serialize>(__TAURI_INVOKE("server_activity", { sessionId })),
	serverSizes: (sessionId: string) => typedError<Sizes_Serialize, CoreError_Serialize>(__TAURI_INVOKE("server_sizes", { sessionId })),
	serverKill: (sessionId: string, target: string, mode: KillMode, confirm: string | null) => typedError<boolean, CoreError_Serialize>(__TAURI_INVOKE("server_kill", { sessionId, target, mode, confirm })),
	serverMonitor: (sessionId: string) => typedError<Monitor_Serialize, CoreError_Serialize>(__TAURI_INVOKE("server_monitor", { sessionId })),
	serverSlowQueries: (sessionId: string) => typedError<SlowQueries_Serialize, CoreError_Serialize>(__TAURI_INVOKE("server_slow_queries", { sessionId })),
	serverMaintain: (request: MaintenanceRequest_Deserialize) => typedError<MaintenanceDone, CoreError_Serialize>(__TAURI_INVOKE("server_maintain", { request })),
	principalsList: (sessionId: string) => typedError<Principals_Serialize, CoreError_Serialize>(__TAURI_INVOKE("principals_list", { sessionId })),
	clickhouseAccess: (sessionId: string) => typedError<ClickhouseAccess_Serialize, CoreError_Serialize>(__TAURI_INVOKE("clickhouse_access", { sessionId })),
	securityCheck: (sessionId: string) => typedError<SecurityReport_Serialize, CoreError_Serialize>(__TAURI_INVOKE("security_check", { sessionId })),
	valueSearchPlan: (sessionId: string, spec: SearchSpec, tables: SearchTable[]) => typedError<SearchPlanned_Serialize, CoreError_Serialize>(__TAURI_INVOKE("value_search_plan", { sessionId, spec, tables })),
	valueSearch: (request: ValueSearch, events: Channel<SearchEvent>) => typedError<SearchDone, CoreError_Serialize>(__TAURI_INVOKE("value_search", { request, events })),
	genProfile: (sessionId: string, namespace: string, name: string) => typedError<GenProfile_Serialize, CoreError_Serialize>(__TAURI_INVOKE("gen_profile", { sessionId, namespace, name })),
	genPreview: (request: FakeRequest_Deserialize) => typedError<GenPreview_Serialize, CoreError_Serialize>(__TAURI_INVOKE("gen_preview", { request })),
	genRun: (request: FakeRequest_Deserialize, events: Channel<Progress_Deserialize>) => typedError<GenDone_Serialize, CoreError_Serialize>(__TAURI_INVOKE("gen_run", { request, events })),
	appInfo: () => __TAURI_INVOKE<AppInfo>("app_info"),
	openLink: (url: string) => typedError<null, UpdateFault>(__TAURI_INVOKE("open_link", { url })),
	updateCheck: (auto: boolean) => typedError<UpdateStatus_Serialize, UpdateFault>(__TAURI_INVOKE("update_check", { auto })),
	seclabVersionsUpdate: () => typedError<string, UpdateFault>(__TAURI_INVOKE("seclab_versions_update")),
	updateDownload: (progress: Channel<UpdateProgress_Deserialize>) => typedError<null, UpdateFault>(__TAURI_INVOKE("update_download", { progress })),
	updateInstall: () => typedError<InstallOutcome, UpdateFault>(__TAURI_INVOKE("update_install")),
	updateRestart: () => typedError<null, UpdateFault>(__TAURI_INVOKE("update_restart")),
	updatePrefsGet: () => __TAURI_INVOKE<UpdatePrefs_Serialize>("update_prefs_get"),
	updatePrefsSet: (checkOnStart: boolean) => __TAURI_INVOKE<UpdatePrefs_Serialize>("update_prefs_set", { checkOnStart }),
};

export type Activity = Activity_Serialize | Activity_Deserialize;

export type Activity_Deserialize = {
	sessions: ServerSession_Deserialize[],
	locks: LockWait_Deserialize[],
	partial: boolean,
	can_cancel: boolean,
};

export type Activity_Serialize = {
	sessions: ServerSession_Serialize[],
	locks: LockWait_Serialize[],
	partial: boolean,
	can_cancel: boolean,
};

export type AppInfo = {
	version: string,
	commit: string,
	built: string,
	platform: string,
	update_url: string,
};

export type Appearance = Appearance_Serialize | Appearance_Deserialize;

export type Appearance_Deserialize = {
	style?: Style,
	palette?: Palette,
	theme?: ThemeMode,
	locale?: Locale,
	ui_scale?: UiScale,
	sounds?: boolean,
	sound_volume?: Volume,
	sound_detail?: SoundDetail,
	sound_timbre?: SoundTimbre,
	sound_gestures?: boolean,
	jingle?: boolean,
	setup_done?: boolean,
};

export type Appearance_Serialize = {
	style: Style,
	palette: Palette,
	theme: ThemeMode,
	locale: Locale,
	ui_scale: UiScale,
	sounds: boolean,
	sound_volume: Volume,
	sound_detail: SoundDetail,
	sound_timbre: SoundTimbre,
	sound_gestures: boolean,
	jingle: boolean,
	setup_done: boolean,
};

export type ApplyOutcome = ApplyOutcome_Serialize | ApplyOutcome_Deserialize;

export type ApplyOutcome_Deserialize = { kind: "applied"; rows: number | null; 
held: boolean } | 
{ kind: "conflict"; index: number } | { kind: "failed"; index: number; error: DriverError_Deserialize };

export type ApplyOutcome_Serialize = { kind: "applied"; rows: number | null; 
held: boolean } | 
{ kind: "conflict"; index: number } | { kind: "failed"; index: number; error: DriverError_Serialize };

export type AuditEntry = AuditEntry_Serialize | AuditEntry_Deserialize;

export type AuditEntry_Deserialize = {
	id: number | null,
	connection_id: string,
	database?: string,
	at: number | null,
	who: string,
	sql: string,
	rows?: number | null,
	error?: string | null,
};

export type AuditEntry_Serialize = {
	id: number | null,
	connection_id: string,
	database: string,
	at: number | null,
	who: string,
	sql: string,
	rows: number | null,
	error: string | null,
};

export type AutoKind = 
"identity" | 
"computed";

export type Backed = Backed_Serialize | Backed_Deserialize;

export type Backed_Deserialize = {
	bytes: number | null,
	elapsed_ms: number | null,
	cancelled: boolean,
	notes?: string | null,
};

export type Backed_Serialize = {
	bytes: number | null,
	elapsed_ms: number | null,
	cancelled: boolean,
	notes: string | null,
};

export type BackgroundMerge = BackgroundMerge_Serialize | BackgroundMerge_Deserialize;

export type BackgroundMerge_Deserialize = {
	namespace: string,
	table: string,
	elapsed_ms?: number | null,
	progress?: number | null,
	parts?: number | null,
	bytes?: number | null,
	mutation: boolean,
};

export type BackgroundMerge_Serialize = {
	namespace: string,
	table: string,
	elapsed_ms: number | null,
	progress: number | null,
	parts: number | null,
	bytes: number | null,
	mutation: boolean,
};

export type BackupFormat = 
"sql" | 
"archive" | 
"file" | 
"folder" | 
"bak" | 
"zip";

export type BackupRequest = {
	job_id: string,
	session_id: string,
	path: string,
	format: BackupFormat,
	schema_only: boolean,
};

export type BackupTool = BackupTool_Serialize | BackupTool_Deserialize;

export type BackupTool_Deserialize = {
	name: string,
	path?: string | null,
	version?: string | null,
};

export type BackupTool_Serialize = {
	name: string,
	path: string | null,
	version: string | null,
};

export type Browse = Browse_Serialize | Browse_Deserialize;

export type Browse_Deserialize = {
	table: TableRef,
	columns: string[],
	key: string[],
	filter?: string | null,
	quick: QuickFilter_Deserialize[],
	order: Order[],
	limit: number,
	offset: number | null,
	after?: (string | null)[] | null,
	search?: string | null,
};

export type Browse_Serialize = {
	table: TableRef,
	columns: string[],
	key: string[],
	filter: string | null,
	quick: QuickFilter_Serialize[],
	order: Order[],
	limit: number,
	offset: number | null,
	after: (string | null)[] | null,
	search: string | null,
};

export type Buffers = {
	hit: number | null,
	read: number | null,
	written: number | null,
	temp_read: number | null,
	temp_written: number | null,
};

export type Cancel = 
"server" | 
"between_fetches" | 
"none";

export type Capabilities = {
	family: Family,
	language: Language,
	schema: SchemaKind,
	results: ResultShape,
	tree: TreeShape,
	words: Vocabulary,
	transactions: boolean,
	editable: boolean,
	explain: boolean,
	er: boolean,
	ddl: boolean,
	sort: boolean,
	params: boolean,
	tools: Tool[],
	cancel: Cancel,
};

export type CassandraTarget = CassandraTarget_Serialize | CassandraTarget_Deserialize;

export type CassandraTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	consistency?: Consistency,
	datacenter?: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
};

export type CassandraTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	consistency: Consistency,
	datacenter: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
};

export type CellChange = CellChange_Serialize | CellChange_Deserialize;

export type CellChange_Deserialize = {
	column: number,
	was?: string | null,
};

export type CellChange_Serialize = {
	column: number,
	was: string | null,
};

export type Change = Change_Serialize | Change_Deserialize;

export type ChangePreview = {
	begin: string,
	statements: string[],
	commit: string,
};

export type ChangeSet = ChangeSet_Serialize | ChangeSet_Deserialize;

export type ChangeSet_Deserialize = {
	table: TableRef,
	columns: ColumnType[],
	changes: Change_Deserialize[],
};

export type ChangeSet_Serialize = {
	table: TableRef,
	columns: ColumnType[],
	changes: Change_Serialize[],
};

export type Change_Deserialize = { kind: "update"; key: Field_Deserialize[]; original: Field_Deserialize[]; set: Field_Deserialize[] } | { kind: "insert"; values: Field_Deserialize[] } | { kind: "delete"; key: Field_Deserialize[]; original: Field_Deserialize[] };

export type Change_Serialize = { kind: "update"; key: Field_Serialize[]; original: Field_Serialize[]; set: Field_Serialize[] } | { kind: "insert"; values: Field_Serialize[] } | { kind: "delete"; key: Field_Serialize[]; original: Field_Serialize[] };

export type Check = Check_Serialize | Check_Deserialize;

export type Check_Deserialize = { kind: "values"; values: string[] } | { kind: "range"; min?: number | null; max?: number | null; 
min_open: boolean; max_open: boolean };

export type Check_Serialize = { kind: "values"; values: string[] } | { kind: "range"; min: number | null; max: number | null; 
min_open: boolean; max_open: boolean };

export type ClickhouseAccess = ClickhouseAccess_Serialize | ClickhouseAccess_Deserialize;

export type ClickhouseAccess_Deserialize = {
	profiles: SettingsProfile_Deserialize[],
	policies: RowPolicy_Deserialize[],
};

export type ClickhouseAccess_Serialize = {
	profiles: SettingsProfile_Serialize[],
	policies: RowPolicy_Serialize[],
};

export type ClickhouseTarget = ClickhouseTarget_Serialize | ClickhouseTarget_Deserialize;

export type ClickhouseTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
};

export type ClickhouseTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
};

export type ClientCert = {
	cert: string,
	key: string,
};

export type Column = Column_Serialize | Column_Deserialize;

export type ColumnGen = ColumnGen_Serialize | ColumnGen_Deserialize;

export type ColumnGen_Deserialize = {
	name: string,
	generator: Gen_Deserialize,
	null_ratio: number | null,
	max_len?: number | null,
	locale?: DataLocale | null,
	dist?: Dist,
};

export type ColumnGen_Serialize = {
	name: string,
	generator: Gen_Serialize,
	null_ratio: number | null,
	max_len: number | null,
	locale: DataLocale | null,
	dist: Dist,
};

export type ColumnHits = {
	column: string,
	count: number | null,
};

export type ColumnInfo = {
	name: string,
	type_name: string,
	nullable: boolean,
	primary_key: boolean,
};

export type ColumnMap = {
	source: number,
	target: string,
};

export type ColumnPair = ColumnPair_Serialize | ColumnPair_Deserialize;

export type ColumnPair_Deserialize = {
	name: string,
	b_name: string,
	a_type?: string | null,
	b_type?: string | null,
	type_differs: boolean,
};

export type ColumnPair_Serialize = {
	name: string,
	b_name: string,
	a_type: string | null,
	b_type: string | null,
	type_differs: boolean,
};

export type ColumnPlan = ColumnPlan_Serialize | ColumnPlan_Deserialize;

export type ColumnPlan_Deserialize = {
	common: ColumnPair_Deserialize[],
	only_a: Column_Deserialize[],
	only_b: Column_Deserialize[],
};

export type ColumnPlan_Serialize = {
	common: ColumnPair_Serialize[],
	only_a: Column_Serialize[],
	only_b: Column_Serialize[],
};

export type ColumnType = {
	name: string,
	type_name: string,
};

export type Column_Deserialize = {
	name: string,
	type_name?: string | null,
};

export type Column_Serialize = {
	name: string,
	type_name: string | null,
};

export type CommitMode = 
"auto" | 
"manual" | 
"smart";

export type CompareRequest = {
	source_session: string,
	source_namespace: string,
	target_session: string,
	target_namespace: string,
};

export type Comparison = Comparison_Serialize | Comparison_Deserialize;

export type Comparison_Deserialize = {
	items: DiffItem_Deserialize[],
	statements: string[],
	source_dialect: Dialect,
	target_dialect: Dialect,
};

export type Comparison_Serialize = {
	items: DiffItem_Serialize[],
	statements: string[],
	source_dialect: Dialect,
	target_dialect: Dialect,
};

export type ConfirmMode = "typed" | "modal" | "none";

export type Conflict = { reason: "same_id"; existing: string } | { reason: "same_name"; existing: string } | 
{ reason: "replaces" };

export type ConnectionCount = ConnectionCount_Serialize | ConnectionCount_Deserialize;

export type ConnectionCount_Deserialize = {
	total?: number | null,
	active?: number | null,
	idle_in_transaction?: number | null,
	waiting?: number | null,
	max?: number | null,
};

export type ConnectionCount_Serialize = {
	total: number | null,
	active: number | null,
	idle_in_transaction: number | null,
	waiting: number | null,
	max: number | null,
};

export type ConnectionProfile = ConnectionProfile_Serialize | ConnectionProfile_Deserialize;

export type ConnectionProfile_Deserialize = {
	id: string,
	name: string,
	folder?: string | null,
	environment: Environment,
	color?: string | null,
	target: Target_Deserialize,
	session?: SessionOptions_Deserialize,
	template?: Template,
	raw_fields?: { [key in string]: string },
};

export type ConnectionProfile_Serialize = {
	id: string,
	name: string,
	folder: string | null,
	environment: Environment,
	color: string | null,
	target: Target_Serialize,
	session: SessionOptions_Serialize,
	template: Template,
	raw_fields: { [key in string]: string },
};

export type Consistency = "any" | "one" | "two" | "three" | "quorum" | "all" | "local_quorum" | "each_quorum" | "local_one";

export type CoreError = CoreError_Serialize | CoreError_Deserialize;

export type CoreError_Deserialize = { code: "driver"; detail: DriverError_Deserialize } | { code: "tunnel"; detail: TunnelError } | { code: "secrets"; detail: string } | { code: "no_keyring" } | { code: "store"; detail: string } | { code: "not_found"; detail: string } | { code: "single_database" } | { code: "busy" } | { code: "read_only" } | { code: "needs_confirmation" } | { code: "name_mismatch" } | { code: "risky"; detail: RiskFound_Deserialize[] } | { code: "transaction_open" } | { code: "transaction_failed" } | { code: "not_dry_runnable"; detail: {
	index: number,
	sql: string,
} } | { code: "no_key" } | { code: "invalid_value"; detail: string } | { code: "plan_unavailable" } | { code: "unsortable"; detail: Unsortable } | { code: "package"; detail: PackageFault } | { code: "io"; detail: string } | { code: "tool"; detail: ToolFault } | { code: "init"; detail: InitFailure_Deserialize } | { code: "param"; detail: ParamError } | { code: "variable"; detail: VarError } | { code: "model" } | { code: "not_sql" } | { code: "poisoned" };

export type CoreError_Serialize = { code: "driver"; detail: DriverError_Serialize } | { code: "tunnel"; detail: TunnelError } | { code: "secrets"; detail: string } | { code: "no_keyring" } | { code: "store"; detail: string } | { code: "not_found"; detail: string } | { code: "single_database" } | { code: "busy" } | { code: "read_only" } | { code: "needs_confirmation" } | { code: "name_mismatch" } | { code: "risky"; detail: RiskFound_Serialize[] } | { code: "transaction_open" } | { code: "transaction_failed" } | { code: "not_dry_runnable"; detail: {
	index: number,
	sql: string,
} } | { code: "no_key" } | { code: "invalid_value"; detail: string } | { code: "plan_unavailable" } | { code: "unsortable"; detail: Unsortable } | { code: "package"; detail: PackageFault } | { code: "io"; detail: string } | { code: "tool"; detail: ToolFault } | { code: "init"; detail: InitFailure_Serialize } | { code: "param"; detail: ParamError } | { code: "variable"; detail: VarError } | { code: "model" } | { code: "not_sql" } | { code: "poisoned" };

export type DataExport = DataExport_Serialize | DataExport_Deserialize;

export type DataExportSource = { kind: "table"; namespace: string; name: string } | { kind: "query"; sql: string };

export type DataExport_Deserialize = {
	job_id: string,
	session_id: string,
	source: DataExportSource,
	path: string,
	format: FileFormat,
	header: boolean,
	expected_rows?: number | null,
};

export type DataExport_Serialize = {
	job_id: string,
	session_id: string,
	source: DataExportSource,
	path: string,
	format: FileFormat,
	header: boolean,
	expected_rows: number | null,
};

export type DataExported = {
	rows: number | null,
	bytes: number | null,
	elapsed_ms: number | null,
	truncated: boolean,
	cancelled: boolean,
};

export type DataImport = DataImport_Serialize | DataImport_Deserialize;

export type DataImportSample = {
	columns: SampleColumn[],
	rows: ((string | null)[])[],
	sheets: string[],
	more: boolean,
};

export type DataImportSource = DataImportSource_Serialize | DataImportSource_Deserialize;

export type DataImportSource_Deserialize = {
	path: string,
	format: FileFormat,
	header: boolean,
	sheet?: string | null,
};

export type DataImportSource_Serialize = {
	path: string,
	format: FileFormat,
	header: boolean,
	sheet: string | null,
};

export type DataImportTarget = { kind: "existing"; namespace: string; name: string } | { kind: "create"; namespace: string; name: string; columns: NewColumn[] };

export type DataImport_Deserialize = {
	job_id: string,
	session_id: string,
	source: DataImportSource_Deserialize,
	target: DataImportTarget,
	mapping: ColumnMap[],
	empty_is_null: boolean,
	on_error: OnRowError,
	confirm?: string | null,
};

export type DataImport_Serialize = {
	job_id: string,
	session_id: string,
	source: DataImportSource_Serialize,
	target: DataImportTarget,
	mapping: ColumnMap[],
	empty_is_null: boolean,
	on_error: OnRowError,
	confirm: string | null,
};

export type DataImported = {
	inserted: number | null,
	skipped: number | null,
	faults: RowFault[],
	faults_cut: boolean,
	committed: boolean,
	cancelled: boolean,
	elapsed_ms: number | null,
};

export type DataLocale = "it" | "uk" | "us";

export type DatabaseSize = DatabaseSize_Serialize | DatabaseSize_Deserialize;

export type DatabaseSize_Deserialize = {
	name: string,
	bytes?: number | null,
};

export type DatabaseSize_Serialize = {
	name: string,
	bytes: number | null,
};

export type DateDisplay = "raw" | "local";

export type Db2Target = Db2Target_Serialize | Db2Target_Deserialize;

export type Db2Target_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
};

export type Db2Target_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
};

export type DdlKind = "table" | "view" | 
"routine" | "trigger" | "index" | "sequence";

export type DepDirection = 
"uses" | 
"used_by";

export type DepKind = "table" | "view" | "routine" | "trigger" | "sequence";

export type DepVia = 
"definition" | "foreign_key" | "trigger" | 
"owned";

export type Dependency = Dependency_Serialize | Dependency_Deserialize;

export type Dependency_Deserialize = {
	direction: DepDirection,
	via: DepVia,
	kind: DepKind,
	namespace: string,
	name: string,
	detail?: string | null,
};

export type Dependency_Serialize = {
	direction: DepDirection,
	via: DepVia,
	kind: DepKind,
	namespace: string,
	name: string,
	detail: string | null,
};

export type DesignCheck = DesignCheck_Serialize | DesignCheck_Deserialize;

export type DesignCheck_Deserialize = {
	origin?: string | null,
	name: string,
	expression: string,
};

export type DesignCheck_Serialize = {
	origin: string | null,
	name: string,
	expression: string,
};

export type DesignColumn = DesignColumn_Serialize | DesignColumn_Deserialize;

export type DesignColumn_Deserialize = {
	origin?: string | null,
	name: string,
	type_name: string,
	nullable: boolean,
	default?: string | null,
	identity: boolean,
	comment?: string | null,
	default_name?: string | null,
};

export type DesignColumn_Serialize = {
	origin: string | null,
	name: string,
	type_name: string,
	nullable: boolean,
	default: string | null,
	identity: boolean,
	comment: string | null,
	default_name: string | null,
};

export type DesignEngine = "postgres" | "cockroach" | "mysql" | "mssql" | "sqlite" | "duckdb" | "firebird";

export type DesignForeignKey = DesignForeignKey_Serialize | DesignForeignKey_Deserialize;

export type DesignForeignKey_Deserialize = {
	origin?: string | null,
	name: string,
	columns: string[],
	ref_namespace: string,
	ref_table: string,
	ref_columns: string[],
	on_delete: FkAction,
	on_update: FkAction,
};

export type DesignForeignKey_Serialize = {
	origin: string | null,
	name: string,
	columns: string[],
	ref_namespace: string,
	ref_table: string,
	ref_columns: string[],
	on_delete: FkAction,
	on_update: FkAction,
};

export type DesignIndex = DesignIndex_Serialize | DesignIndex_Deserialize;

export type DesignIndex_Deserialize = {
	origin?: string | null,
	name: string,
	columns: string[],
	unique: boolean,
	descending?: boolean[],
	expressions?: boolean[],
	predicate?: string | null,
};

export type DesignIndex_Serialize = {
	origin: string | null,
	name: string,
	columns: string[],
	unique: boolean,
	descending: boolean[],
	expressions: boolean[],
	predicate: string | null,
};

export type DesignNote = DesignNote_Serialize | DesignNote_Deserialize;

export type DesignNoteCode = 
"drops_data" | 
"rebuild" | 
"reorder_rewrites" | 
"reorder_unsupported" | 
"default_constraint" | 
"identity_unsupported" | 
"comment_unsupported" | 
"not_null_needs_value" | 
"type_change" | 
"primary_key_required" | 
"fk_action_unsupported" | 
"invalid" | 
"trigger_recreated" | 
"trigger_blocks" | 
"partial_unsupported" | 
"rename_via_temp" | 
"rename_table_unsupported";

export type DesignNote_Deserialize = {
	code: DesignNoteCode,
	subject?: string | null,
	detail?: string | null,
};

export type DesignNote_Serialize = {
	code: DesignNoteCode,
	subject: string | null,
	detail: string | null,
};

export type DesignScript = DesignScript_Serialize | DesignScript_Deserialize;

export type DesignScript_Deserialize = {
	statements: string[],
	notes: DesignNote_Deserialize[],
	rebuild: boolean,
};

export type DesignScript_Serialize = {
	statements: string[],
	notes: DesignNote_Serialize[],
	rebuild: boolean,
};

export type DesignTrigger = {
	name: string,
	sql: string,
};

export type Detail = {
	key: string,
	value: string,
};

export type Dialect = "postgres" | "sqlite" | "mysql" | "mssql" | "duckdb" | "clickhouse" | "firebird" | "hana" | 
"db2" | 
"ansi" | 
"ansi_backtick" | "oracle" | 
"cql" | 
"partiql" | 
"opensearch" | 
"elasticsearch";

export type DiffItem = DiffItem_Serialize | DiffItem_Deserialize;

export type DiffItem_Deserialize = {
	kind: ObjectKind,
	table?: string | null,
	name: string,
	state: DiffState,
	source?: string | null,
	target?: string | null,
	notes: Note[],
};

export type DiffItem_Serialize = {
	kind: ObjectKind,
	table: string | null,
	name: string,
	state: DiffState,
	source: string | null,
	target: string | null,
	notes: Note[],
};

export type DiffState = "added" | "removed" | "changed" | "same";

export type Dist = { kind: "uniform" } | 
{ kind: "normal" } | 
{ kind: "low" } | 
{ kind: "high" } | 
{ kind: "sequential"; step: number | null };

export type DocumentWrite = { kind: "insert"; database: string; collection: string; documents: string[] } | { kind: "replace"; database: string; collection: string; id: string; document: string } | { kind: "delete"; database: string; collection: string; ids: string[] };

export type DriverError = DriverError_Serialize | DriverError_Deserialize;

export type DriverError_Deserialize = { kind: "connect"; message: string } | { kind: "auth"; message: string } | { kind: "tls"; message: string } | { kind: "query"; message: string; code?: string | null; detail?: string | null; hint?: string | null; 
offset?: number | null } | { kind: "cancelled" } | { kind: "timeout"; seconds: number } | { kind: "io"; message: string };

export type DriverError_Serialize = { kind: "connect"; message: string } | { kind: "auth"; message: string } | { kind: "tls"; message: string } | { kind: "query"; message: string; code: string | null; detail: string | null; hint: string | null; 
offset: number | null } | { kind: "cancelled" } | { kind: "timeout"; seconds: number } | { kind: "io"; message: string };

export type DriverPrefs = DriverPrefs_Serialize | DriverPrefs_Deserialize;

export type DriverPrefs_Deserialize = {
	db2_clidriver?: string,
};

export type DriverPrefs_Serialize = {
	db2_clidriver: string,
};

export type DuckdbTarget = {
	path: string,
	read_only: boolean,
};

export type DynamodbTarget = DynamodbTarget_Serialize | DynamodbTarget_Deserialize;

export type DynamodbTarget_Deserialize = {
	region: string,
	endpoint?: string,
	profile?: string,
	user?: string,
};

export type DynamodbTarget_Serialize = {
	region: string,
	endpoint: string,
	profile: string,
	user: string,
};

export type Edition = Edition_Serialize | Edition_Deserialize;

export type Edition_Deserialize = {
	flavor: Flavor,
	version?: string | null,
	facts: Fact[],
	limits: Limit[],
};

export type Edition_Serialize = {
	flavor: Flavor,
	version: string | null,
	facts: Fact[],
	limits: Limit[],
};

export type EditorPrefs = EditorPrefs_Serialize | EditorPrefs_Deserialize;

export type EditorPrefs_Deserialize = {
	font_size?: number,
	tab_width?: number,
	autocomplete?: boolean,
	uppercase_keywords?: boolean,
	inspections?: boolean,
	continue_on_error?: boolean,
};

export type EditorPrefs_Serialize = {
	font_size: number,
	tab_width: number,
	autocomplete: boolean,
	uppercase_keywords: boolean,
	inspections: boolean,
	continue_on_error: boolean,
};

export type ElasticTarget = ElasticTarget_Serialize | ElasticTarget_Deserialize;

export type ElasticTarget_Deserialize = {
	host: string,
	port: number,
	database?: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
};

export type ElasticTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
};

export type Engine = "postgres" | "mysql" | "mariadb" | "sqlite" | "mssql" | "duckdb" | "cockroach" | "clickhouse" | "mongodb" | "cassandra" | "elasticsearch" | "tidb" | "oracle" | "firebird" | "hana" | "db2";

export type Environment = "local" | "development" | "staging" | "production";

export type ErLayout = {
	tables: TablePosition[],
};

export type EstimateReason = "requested" | 
"production_write" | 
"no_safe_rollback" | 
"engine_cannot_measure" | 
"no_savepoint" | 
"no_privilege";

export type Execution = { kind: "measured" } | 
{ kind: "measured_rolled_back" } | { kind: "estimated"; reason: EstimateReason };

export type ExplainMode = 
"analyze" | 
"estimate";

export type ExplainRequest = ExplainRequest_Serialize | ExplainRequest_Deserialize;

export type ExplainRequest_Deserialize = {
	run_id: string,
	session_id: string,
	text: string,
	scope: RunScope,
	mode: ExplainMode,
	params?: ParamValue[] | null,
};

export type ExplainRequest_Serialize = {
	run_id: string,
	session_id: string,
	text: string,
	scope: RunScope,
	mode: ExplainMode,
	params: ParamValue[] | null,
};

export type ExportFormat = { kind: "sealed"; passphrase: string } | { kind: "plain" };

export type ExportSelection = {
	folders: string[],
	connections: string[],
	workspaces: string[],
	history: boolean,
	settings: boolean,
	secrets: boolean,
	library: boolean,
};

export type Fact = 
{ kind: "base"; value: string } | 
{ kind: "endpoint"; value: string } | 
{ kind: "branch"; value: string } | 
{ kind: "project"; value: string } | { kind: "region"; value: string } | 
{ kind: "pooled"; value: Pooler } | 
{ kind: "nodes"; value: number } | 
{ kind: "store"; value: string } | 
{ kind: "extension"; value: {
	name: string,
	version: string,
} };

export type FakeColumn = FakeColumn_Serialize | FakeColumn_Deserialize;

export type FakeColumn_Deserialize = {
	name: string,
	type_name: string,
	nullable: boolean,
	primary_key: boolean,
	unique: boolean,
	auto?: AutoKind | null,
	default?: string | null,
	check?: Check_Deserialize | null,
	parent?: GenParentRef | null,
	shape: Shape_Deserialize,
	suggested: ColumnGen_Deserialize,
};

export type FakeColumn_Serialize = {
	name: string,
	type_name: string,
	nullable: boolean,
	primary_key: boolean,
	unique: boolean,
	auto: AutoKind | null,
	default: string | null,
	check: Check_Serialize | null,
	parent: GenParentRef | null,
	shape: Shape_Serialize,
	suggested: ColumnGen_Serialize,
};

export type FakeRequest = FakeRequest_Serialize | FakeRequest_Deserialize;

export type FakeRequest_Deserialize = {
	job_id: string,
	session_id: string,
	namespace: string,
	name: string,
	columns: ColumnGen_Deserialize[],
	rows: number,
	seed: number,
	parents?: number | null,
	locale?: DataLocale,
};

export type FakeRequest_Serialize = {
	job_id: string,
	session_id: string,
	namespace: string,
	name: string,
	columns: ColumnGen_Serialize[],
	rows: number,
	seed: number,
	parents: number | null,
	locale: DataLocale,
};

export type Family = "relational" | "document" | "key_value" | "wide_column" | "search";

export type Field = Field_Serialize | Field_Deserialize;

export type Field_Deserialize = {
	column: string,
	value?: string | null,
};

export type Field_Serialize = {
	column: string,
	value: string | null,
};

export type FileFormat = "csv" | "tsv" | "json" | "xlsx" | "parquet";

export type FilterOp = "eq" | "ne" | "lt" | "le" | "gt" | "ge" | "contains" | "is_null" | "not_null";

export type Finding = Finding_Serialize | Finding_Deserialize;

export type Finding_Deserialize = {
	rule: Rule,
	severity: Severity,
	node?: number | null,
	table?: string | null,
	column?: string | null,
	wrapper?: string | null,
	rows?: number | null,
	kept?: number | null,
	factor?: number | null,
	share?: number | null,
	kb?: number | null,
	suggestion?: string | null,
};

export type Finding_Serialize = {
	rule: Rule,
	severity: Severity,
	node: number | null,
	table: string | null,
	column: string | null,
	wrapper: string | null,
	rows: number | null,
	kept: number | null,
	factor: number | null,
	share: number | null,
	kb: number | null,
	suggestion: string | null,
};

export type FirebirdTarget = FirebirdTarget_Serialize | FirebirdTarget_Deserialize;

export type FirebirdTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	role?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
};

export type FirebirdTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	role: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
};

export type FkAction = "no_action" | "restrict" | "cascade" | "set_null" | "set_default";

export type Flavor = "timescale" | "neon" | "supabase" | "yugabyte" | "tidb" | "planetscale";

export type Folder = Folder_Serialize | Folder_Deserialize;

export type FolderMark = "star" | "briefcase" | "building" | "flask" | "server" | "shield";

export type FolderTint = "ochre" | "red" | "green" | "blue" | "violet" | "slate";

export type Folder_Deserialize = {
	id: string,
	parent?: string | null,
	name: string,
	tint?: FolderTint | null,
	mark?: FolderMark | null,
	collapsed: boolean,
};

export type Folder_Serialize = {
	id: string,
	parent: string | null,
	name: string,
	tint: FolderTint | null,
	mark: FolderMark | null,
	collapsed: boolean,
};

export type ForeignKey = {
	name: string,
	columns: string[],
	ref_namespace: string,
	ref_table: string,
	ref_columns: string[],
	on_delete: FkAction,
	on_update: FkAction,
};

export type Function = {
	name: string,
	signature: string,
};

export type GapReason = 
{ kind: "file_database" } | 
{ kind: "not_exposed" } | 
{ kind: "permission"; need: string } | 
{ kind: "extension_missing"; name: string } | 
{ kind: "extension_not_loaded"; name: string } | 
{ kind: "feature_off"; name: string } | 
{ kind: "failed"; message: string };

export type Gen = Gen_Serialize | Gen_Deserialize;

export type GenColumn = GenColumn_Serialize | GenColumn_Deserialize;

export type GenColumn_Deserialize = {
	name: string,
	primary_key: boolean,
	type_name?: string,
	not_null?: boolean,
	identity?: Identity | null,
};

export type GenColumn_Serialize = {
	name: string,
	primary_key: boolean,
	type_name: string,
	not_null: boolean,
	identity: Identity | null,
};

export type GenDone = GenDone_Serialize | GenDone_Deserialize;

export type GenDone_Deserialize = {
	tables: GenWritten[],
	inserted: number | null,
	committed: boolean,
	kept: boolean,
	cancelled: boolean,
	fault?: GenFault | null,
	elapsed_ms: number | null,
};

export type GenDone_Serialize = {
	tables: GenWritten[],
	inserted: number | null,
	committed: boolean,
	kept: boolean,
	cancelled: boolean,
	fault: GenFault | null,
	elapsed_ms: number | null,
};

export type GenFault = 
{ kind: "empty_parent"; table: string } | 
{ kind: "no_unique"; columns: string[] } | 
{ kind: "batch"; table: string; from: number | null; to: number | null; message: string } | 
{ kind: "row"; table: string; row: number | null; message: string } | 
{ kind: "expr"; column: string; message: string } | 
{ kind: "check"; expr: string } | { kind: "server"; message: string };

export type GenForeignKey = GenForeignKey_Serialize | GenForeignKey_Deserialize;

export type GenForeignKey_Deserialize = {
	namespace: string,
	table: string,
	columns: string[],
	ref_columns: string[],
	parent_rows?: number | null,
	own: boolean,
};

export type GenForeignKey_Serialize = {
	namespace: string,
	table: string,
	columns: string[],
	ref_columns: string[],
	parent_rows: number | null,
	own: boolean,
};

export type GenParentRef = {
	fk: number,
	namespace: string,
	table: string,
	column: string,
};

export type GenPreview = GenPreview_Serialize | GenPreview_Deserialize;

export type GenPreview_Deserialize = {
	columns: string[],
	rows: ((string | null)[])[],
	waiting: string[],
	fault?: GenFault | null,
};

export type GenPreview_Serialize = {
	columns: string[],
	rows: ((string | null)[])[],
	waiting: string[],
	fault: GenFault | null,
};

export type GenProfile = GenProfile_Serialize | GenProfile_Deserialize;

export type GenProfile_Deserialize = {
	namespace: string,
	name: string,
	columns: FakeColumn_Deserialize[],
	foreign_keys: GenForeignKey_Deserialize[],
	unique: string[][],
	ancestors: TableRef[],
	rules: GenRule[],
	estimated_rows?: number | null,
};

export type GenProfile_Serialize = {
	namespace: string,
	name: string,
	columns: FakeColumn_Serialize[],
	foreign_keys: GenForeignKey_Serialize[],
	unique: string[][],
	ancestors: TableRef[],
	rules: GenRule[],
	estimated_rows: number | null,
};

export type GenProfiles = GenProfiles_Serialize | GenProfiles_Deserialize;

export type GenProfiles_Deserialize = {
	profiles?: GenSaved_Deserialize[],
};

export type GenProfiles_Serialize = {
	profiles: GenSaved_Serialize[],
};

export type GenRequest = GenRequest_Serialize | GenRequest_Deserialize;

export type GenRequest_Deserialize = {
	table: TableRef,
	view: boolean,
	columns: GenColumn_Deserialize[],
	generator: Generator,
	copy_name: string,
};

export type GenRequest_Serialize = {
	table: TableRef,
	view: boolean,
	columns: GenColumn_Serialize[],
	generator: Generator,
	copy_name: string,
};

export type GenRule = {
	expr: string,
	kind: RuleKind,
};

export type GenSaved = GenSaved_Serialize | GenSaved_Deserialize;

export type GenSaved_Deserialize = {
	id: string,
	name: string,
	connection_id: string,
	database?: string | null,
	namespace: string,
	table: string,
	locale: DataLocale,
	seed: number,
	rows: number,
	parents?: number | null,
	columns: ColumnGen_Deserialize[],
	saved_ms: number | null,
};

export type GenSaved_Serialize = {
	id: string,
	name: string,
	connection_id: string,
	database: string | null,
	namespace: string,
	table: string,
	locale: DataLocale,
	seed: number,
	rows: number,
	parents: number | null,
	columns: ColumnGen_Serialize[],
	saved_ms: number | null,
};

export type GenWritten = {
	namespace: string,
	name: string,
	rows: number | null,
};

export type Gen_Deserialize = 
{ kind: "auto" } | { kind: "null" } | { kind: "fixed"; value: string } | { kind: "first_name" } | { kind: "last_name" } | { kind: "full_name" } | { kind: "company" } | { kind: "email" } | { kind: "phone" } | { kind: "street" } | { kind: "city" } | 
{ kind: "province" } | { kind: "postal_code" } | 
{ kind: "birth_date" } | { kind: "codice_fiscale" } | { kind: "partita_iva" } | { kind: "integer"; min: number | null; max: number | null } | 
{ kind: "sequence"; start: number | null } | { kind: "decimal"; min: number | null; max: number | null; scale: number } | { kind: "boolean" } | 
{ kind: "date"; from: string; to: string } | { kind: "date_time"; from: string; to: string } | { kind: "time" } | { kind: "lorem"; min_words: number; max_words: number } | { kind: "code"; len: number } | { kind: "uuid" } | { kind: "json" } | 
{ kind: "pick"; values: string[]; weights?: (number | null)[] } | 
{ kind: "parent"; fk: number; column: number } | 
{ kind: "pattern"; pattern: string } | 
{ kind: "expr"; expr: string; scale: number };

export type Gen_Serialize = 
{ kind: "auto" } | { kind: "null" } | { kind: "fixed"; value: string } | { kind: "first_name" } | { kind: "last_name" } | { kind: "full_name" } | { kind: "company" } | { kind: "email" } | { kind: "phone" } | { kind: "street" } | { kind: "city" } | 
{ kind: "province" } | { kind: "postal_code" } | 
{ kind: "birth_date" } | { kind: "codice_fiscale" } | { kind: "partita_iva" } | { kind: "integer"; min: number | null; max: number | null } | 
{ kind: "sequence"; start: number | null } | { kind: "decimal"; min: number | null; max: number | null; scale: number } | { kind: "boolean" } | 
{ kind: "date"; from: string; to: string } | { kind: "date_time"; from: string; to: string } | { kind: "time" } | { kind: "lorem"; min_words: number; max_words: number } | { kind: "code"; len: number } | { kind: "uuid" } | { kind: "json" } | 
{ kind: "pick"; values: string[]; weights: (number | null)[] } | 
{ kind: "parent"; fk: number; column: number } | 
{ kind: "pattern"; pattern: string } | 
{ kind: "expr"; expr: string; scale: number };

export type Generator = "select" | "insert" | "update" | "delete" | 
"upsert" | 
"copy" | "drop";

export type Grant = Grant_Serialize | Grant_Deserialize;

export type Grant_Deserialize = {
	grantee: string,
	kind: string,
	schema?: string | null,
	object?: string | null,
	privilege: string,
	grantable: boolean,
};

export type Grant_Serialize = {
	grantee: string,
	kind: string,
	schema: string | null,
	object: string | null,
	privilege: string,
	grantable: boolean,
};

export type Group = "tables" | "views" | "functions" | "sequences" | "types" | "triggers" | "materialized_views" | "dictionaries" | "packages" | "synonyms" | "aliases" | "data_streams";

export type GroupFn = "sum" | "count" | "avg" | "min" | "max";

export type GroupValue = GroupValue_Serialize | GroupValue_Deserialize;

export type GroupValue_Deserialize = {
	column?: string | null,
	func: GroupFn,
};

export type GroupValue_Serialize = {
	column: string | null,
	func: GroupFn,
};

export type HanaTarget = HanaTarget_Serialize | HanaTarget_Deserialize;

export type HanaTarget_Deserialize = {
	host: string,
	port: number,
	database?: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
};

export type HanaTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
};

export type HistoryEntry = HistoryEntry_Serialize | HistoryEntry_Deserialize;

export type HistoryEntry_Deserialize = {
	id: number | null,
	connection_id: string,
	sql: string,
	started_at: number | null,
	elapsed_ms: number | null,
	rows?: number | null,
	error?: string | null,
};

export type HistoryEntry_Serialize = {
	id: number | null,
	connection_id: string,
	sql: string,
	started_at: number | null,
	elapsed_ms: number | null,
	rows: number | null,
	error: string | null,
};

export type HypertableSize = HypertableSize_Serialize | HypertableSize_Deserialize;

export type HypertableSize_Deserialize = {
	namespace: string,
	name: string,
	chunks?: number | null,
	compressed_chunks?: number | null,
	before_compression?: number | null,
	after_compression?: number | null,
	total_bytes?: number | null,
};

export type HypertableSize_Serialize = {
	namespace: string,
	name: string,
	chunks: number | null,
	compressed_chunks: number | null,
	before_compression: number | null,
	after_compression: number | null,
	total_bytes: number | null,
};

export type Identity = 
"always" | 
"by_default";

export type IdentityColumn = {
	name: string,
	identity: Identity,
};

export type ImportChoice = {
	kind: ItemKind,
	id: string,
	resolution: Resolution,
};

export type ImportItem = ImportItem_Serialize | ImportItem_Deserialize;

export type ImportItem_Deserialize = {
	kind: ItemKind,
	id: string,
	name: string,
	path: string[],
	conflict?: Conflict | null,
	secret: boolean,
};

export type ImportItem_Serialize = {
	kind: ItemKind,
	id: string,
	name: string,
	path: string[],
	conflict: Conflict | null,
	secret: boolean,
};

export type ImportOutcome = {
	added: number,
	replaced: number,
	skipped: number,
	history: number,
	settings: boolean,
	scripts: number,
	snippets: number,
	notes: number,
	params: number,
	profiles: number,
	marks: number,
};

export type ImportPreview = ImportPreview_Serialize | ImportPreview_Deserialize;

export type ImportPreview_Deserialize = {
	sealed: boolean,
	created_at: number | null,
	items: ImportItem_Deserialize[],
	history: number,
	scripts: number,
	snippets: number,
	notes: number,
};

export type ImportPreview_Serialize = {
	sealed: boolean,
	created_at: number | null,
	items: ImportItem_Serialize[],
	history: number,
	scripts: number,
	snippets: number,
	notes: number,
};

export type Index = {
	name: string,
	columns: string[],
	unique: boolean,
};

export type IndexUsage = IndexUsage_Serialize | IndexUsage_Deserialize;

export type IndexUsage_Deserialize = {
	namespace: string,
	table: string,
	name: string,
	bytes?: number | null,
	scans?: number | null,
	unique: boolean,
	unused: boolean,
};

export type IndexUsage_Serialize = {
	namespace: string,
	table: string,
	name: string,
	bytes: number | null,
	scans: number | null,
	unique: boolean,
	unused: boolean,
};

export type Inferred = "integer" | "big_integer" | "decimal" | "float" | "boolean" | "date" | "timestamp" | "timestamp_zone" | "text";

export type InitFailure = InitFailure_Serialize | InitFailure_Deserialize;

export type InitFailure_Deserialize = {
	index: number,
	sql: string,
	error: DriverError_Deserialize,
};

export type InitFailure_Serialize = {
	index: number,
	sql: string,
	error: DriverError_Serialize,
};

export type InstallOutcome = 
{ mode: "replaced"; path: string } | 
{ mode: "downloaded"; path: string };

export type ItemKind = "folder" | "connection" | "workspace" | "settings";

export type KeyData = 
{ shape: "text"; text: string; binary: boolean; truncated: boolean } | { shape: "entries"; entries: KeyEntry[] } | { shape: "missing" };

export type KeyEntry = {
	name: string,
	value: string,
};

export type KeyInfo = KeyInfo_Serialize | KeyInfo_Deserialize;

export type KeyInfo_Deserialize = {
	key: string,
	kind: string,
	ttl_ms?: number | null,
	bytes?: number | null,
};

export type KeyInfo_Serialize = {
	key: string,
	kind: string,
	ttl_ms: number | null,
	bytes: number | null,
};

export type KeyMatch = KeyMatch_Serialize | KeyMatch_Deserialize;

export type KeyMatch_Deserialize = {
	table: TableRef,
	types: ColumnType[],
	key: Field_Deserialize[],
};

export type KeyMatch_Serialize = {
	table: TableRef,
	types: ColumnType[],
	key: Field_Serialize[],
};

export type KeyPage = KeyPage_Serialize | KeyPage_Deserialize;

export type KeyPage_Deserialize = {
	keys: KeyInfo_Deserialize[],
	cursor: string,
	total?: number | null,
};

export type KeyPage_Serialize = {
	keys: KeyInfo_Serialize[],
	cursor: string,
	total: number | null,
};

export type KeyRead = {
	key: string,
	cursor: string,
	count: number,
};

export type KeyScan = KeyScan_Serialize | KeyScan_Deserialize;

export type KeyScan_Deserialize = {
	pattern: string,
	cursor: string,
	count: number,
	kind?: string | null,
};

export type KeyScan_Serialize = {
	pattern: string,
	cursor: string,
	count: number,
	kind: string | null,
};

export type KeyValue = KeyValue_Serialize | KeyValue_Deserialize;

export type KeyValue_Deserialize = {
	key: string,
	kind: string,
	ttl_ms?: number | null,
	length?: number | null,
	data: KeyData,
	next?: string | null,
};

export type KeyValue_Serialize = {
	key: string,
	kind: string,
	ttl_ms: number | null,
	length: number | null,
	data: KeyData,
	next: string | null,
};

export type KeyWrite = KeyWrite_Serialize | KeyWrite_Deserialize;

export type KeyWrite_Deserialize = 
{ op: "set_string"; key: string; value: string } | { op: "hash_set"; key: string; field: string; value: string } | { op: "hash_delete"; key: string; fields: string[] } | { op: "list_set"; key: string; index: number | null; value: string } | { op: "list_push"; key: string; values: string[]; head: boolean } | 
{ op: "list_remove"; key: string; index: number | null; value: string } | { op: "set_add"; key: string; members: string[] } | { op: "set_remove"; key: string; members: string[] } | { op: "zset_add"; key: string; member: string; score: number | null } | { op: "zset_remove"; key: string; members: string[] } | { op: "stream_add"; key: string; fields: KeyEntry[] } | { op: "stream_delete"; key: string; ids: string[] } | { op: "json_set"; key: string; value: string } | 
{ op: "expire"; key: string; ttl_ms?: number | null } | 
{ op: "rename"; key: string; to: string } | { op: "delete"; keys: string[] } | 
{ op: "create"; key: string; kind: string; first: KeyEntry; ttl_ms?: number | null };

export type KeyWrite_Serialize = 
{ op: "set_string"; key: string; value: string } | { op: "hash_set"; key: string; field: string; value: string } | { op: "hash_delete"; key: string; fields: string[] } | { op: "list_set"; key: string; index: number | null; value: string } | { op: "list_push"; key: string; values: string[]; head: boolean } | 
{ op: "list_remove"; key: string; index: number | null; value: string } | { op: "set_add"; key: string; members: string[] } | { op: "set_remove"; key: string; members: string[] } | { op: "zset_add"; key: string; member: string; score: number | null } | { op: "zset_remove"; key: string; members: string[] } | { op: "stream_add"; key: string; fields: KeyEntry[] } | { op: "stream_delete"; key: string; ids: string[] } | { op: "json_set"; key: string; value: string } | 
{ op: "expire"; key: string; ttl_ms: number | null } | 
{ op: "rename"; key: string; to: string } | { op: "delete"; keys: string[] } | 
{ op: "create"; key: string; kind: string; first: KeyEntry; ttl_ms: number | null };

export type KillMode = 
"cancel" | 
"terminate";

export type Kinds = {
	text: boolean,
	number: boolean,
	other: boolean,
};

export type Language = { kind: "sql"; dialect: Dialect } | 
{ kind: "mql" } | 
{ kind: "redis" } | 
{ kind: "cql" } | 
{ kind: "partiql" } | 
{ kind: "search" };

export type LibsqlTarget = LibsqlTarget_Serialize | LibsqlTarget_Deserialize;

export type LibsqlTarget_Deserialize = { location: "file"; path: string; read_only: boolean } | { location: "remote"; url: string; proxy?: ProxyConfig_Deserialize | null };

export type LibsqlTarget_Serialize = { location: "file"; path: string; read_only: boolean } | { location: "remote"; url: string; proxy: ProxyConfig_Serialize | null };

export type Limit = 
"pooled_session" | 
"scale_to_zero" | 
"managed_schemas" | 
"foreign_keys_optional" | 
"deploy_requests" | 
"no_routines" | 
"distributed_locks" | 
"own_dump_tool" | 
"hypertable_dump" | 
"dashboard_users" | 
"own_plans";

export type LoadedFile = {
	name: string,
	data: string,
};

export type Locale = "it" | "en" | "zh-CN";

export type LockWait = LockWait_Serialize | LockWait_Deserialize;

export type LockWait_Deserialize = {
	waiting: string,
	blocking: string,
	object?: string | null,
	mode?: string | null,
	waited_ms?: number | null,
};

export type LockWait_Serialize = {
	waiting: string,
	blocking: string,
	object: string | null,
	mode: string | null,
	waited_ms: number | null,
};

export type LongOp = LongOp_Serialize | LongOp_Deserialize;

export type LongOp_Deserialize = {
	session: string,
	operation: string,
	target?: string | null,
	progress?: number | null,
	elapsed_ms?: number | null,
	remaining_ms?: number | null,
};

export type LongOp_Serialize = {
	session: string,
	operation: string,
	target: string | null,
	progress: number | null,
	elapsed_ms: number | null,
	remaining_ms: number | null,
};

export type Maintenance = 
"vacuum" | 
"analyze" | 
"optimize";

export type MaintenanceDone = {
	sql: string,
	messages: string[],
	elapsed_ms: number | null,
};

export type MaintenanceRequest = MaintenanceRequest_Serialize | MaintenanceRequest_Deserialize;

export type MaintenanceRequest_Deserialize = {
	session_id: string,
	action: Maintenance,
	table?: MaintenanceTable | null,
	confirm?: string | null,
};

export type MaintenanceRequest_Serialize = {
	session_id: string,
	action: Maintenance,
	table: MaintenanceTable | null,
	confirm: string | null,
};

export type MaintenanceTable = {
	namespace: string,
	name: string,
};

export type Mark = Mark_Serialize | Mark_Deserialize;

export type MarkTarget = MarkTarget_Serialize | MarkTarget_Deserialize;

export type MarkTarget_Deserialize = { kind: "table"; connection_id: string; database: string; namespace: string; name: string; view: boolean } | { kind: "query"; connection_id: string; database?: string | null; sql: string } | 
{ kind: "item"; key: string };

export type MarkTarget_Serialize = { kind: "table"; connection_id: string; database: string; namespace: string; name: string; view: boolean } | { kind: "query"; connection_id: string; database: string | null; sql: string } | 
{ kind: "item"; key: string };

export type Mark_Deserialize = {
	id: string,
	label: string,
	pinned: boolean,
	target: MarkTarget_Deserialize,
};

export type Mark_Serialize = {
	id: string,
	label: string,
	pinned: boolean,
	target: MarkTarget_Serialize,
};

export type Marks = Marks_Serialize | Marks_Deserialize;

export type Marks_Deserialize = {
	marks?: Mark_Deserialize[],
	recent?: string[],
};

export type Marks_Serialize = {
	marks: Mark_Serialize[],
	recent: string[],
};

export type Match = "exact" | "contains";

export type MongoTarget = MongoTarget_Serialize | MongoTarget_Deserialize;

export type MongoTarget_Deserialize = {
	host: string,
	port: number,
	srv?: boolean,
	database: string,
	user: string,
	auth_source?: string,
	options?: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
};

export type MongoTarget_Serialize = {
	host: string,
	port: number,
	srv: boolean,
	database: string,
	user: string,
	auth_source: string,
	options: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
};

export type Monitor = Monitor_Serialize | Monitor_Deserialize;

export type MonitorGap = {
	part: MonitorPart,
	reason: GapReason,
};

export type MonitorPart = "connections" | "transactions" | "databases" | "cache_hit" | "slow_queries";

export type Monitor_Deserialize = {
	connections?: ConnectionCount_Deserialize | null,
	transactions: OpenTransaction_Deserialize[],
	databases: DatabaseSize_Deserialize[],
	cache_hit?: number | null,
	merges?: BackgroundMerge_Deserialize[],
	long_ops?: LongOp_Deserialize[],
	gaps: MonitorGap[],
};

export type Monitor_Serialize = {
	connections: ConnectionCount_Serialize | null,
	transactions: OpenTransaction_Serialize[],
	databases: DatabaseSize_Serialize[],
	cache_hit: number | null,
	merges?: BackgroundMerge_Serialize[],
	long_ops?: LongOp_Serialize[],
	gaps: MonitorGap[],
};

export type MssqlTarget = MssqlTarget_Serialize | MssqlTarget_Deserialize;

export type MssqlTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
};

export type MssqlTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
};

export type MysqlTarget = MysqlTarget_Serialize | MysqlTarget_Deserialize;

export type MysqlTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
	flavor?: Flavor | null,
};

export type MysqlTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
	flavor?: Flavor,
};

export type Namespace = Namespace_Serialize | Namespace_Deserialize;

export type NamespaceLevel = "always" | 
"multiple";

export type NamespaceSize = {
	name: string,
	relations: number | null,
};

export type Namespace_Deserialize = {
	name: string,
	tables: Relation_Deserialize[],
	views: Relation_Deserialize[],
	functions: Function[],
	sequences: Object_Deserialize[],
	types: Object_Deserialize[],
	triggers: Object_Deserialize[],
	materialized_views?: Relation_Deserialize[],
	dictionaries?: Relation_Deserialize[],
	packages?: Object_Deserialize[],
	synonyms?: Object_Deserialize[],
	aliases?: Relation_Deserialize[],
	data_streams?: Relation_Deserialize[],
	managed?: Flavor | null,
	tags?: { [key in string]: RelationTag[] },
};

export type Namespace_Serialize = {
	name: string,
	tables: Relation_Serialize[],
	views: Relation_Serialize[],
	functions: Function[],
	sequences: Object_Serialize[],
	types: Object_Serialize[],
	triggers: Object_Serialize[],
	materialized_views?: Relation_Serialize[],
	dictionaries?: Relation_Serialize[],
	packages?: Object_Serialize[],
	synonyms?: Object_Serialize[],
	aliases?: Relation_Serialize[],
	data_streams?: Relation_Serialize[],
	managed?: Flavor,
	tags?: { [key in string]: RelationTag[] },
};

export type NewColumn = {
	name: string,
	type_name: string,
	nullable: boolean,
};

export type Node = Node_Serialize | Node_Deserialize;

export type NodeKind = "full_scan" | "index_scan" | "index_only_scan" | "bitmap_scan" | "bitmap_index" | "hash_join" | "nested_loop" | "merge_join" | "hash" | "aggregate" | "sort" | "limit" | "gather" | "materialize" | "subquery" | "append" | "window" | "unique" | "modify" | "result" | "other";

export type Node_Deserialize = {
	id: number,
	parent?: number | null,
	depth: number,
	children: number[],
	op: string,
	kind: NodeKind,
	relation?: string | null,
	alias?: string | null,
	index?: string | null,
	condition?: string | null,
	filter?: string | null,
	est_rows?: number | null,
	rows?: number | null,
	loops?: number | null,
	rows_removed?: number | null,
	est_examined?: number | null,
	startup_cost?: number | null,
	total_cost?: number | null,
	time_ms?: number | null,
	self_ms?: number | null,
	width?: number | null,
	buffers?: Buffers | null,
	spill?: Spill_Deserialize | null,
	parallel: boolean,
	details: Detail[],
};

export type Node_Serialize = {
	id: number,
	parent: number | null,
	depth: number,
	children: number[],
	op: string,
	kind: NodeKind,
	relation: string | null,
	alias: string | null,
	index: string | null,
	condition: string | null,
	filter: string | null,
	est_rows: number | null,
	rows: number | null,
	loops: number | null,
	rows_removed: number | null,
	est_examined: number | null,
	startup_cost: number | null,
	total_cost: number | null,
	time_ms: number | null,
	self_ms: number | null,
	width: number | null,
	buffers: Buffers | null,
	spill: Spill_Serialize | null,
	parallel: boolean,
	details: Detail[],
};

export type Note = 
"destructive" | 
"rebuild" | 
"no_definition";

export type NoteScope = "connection" | "workspace";

export type Notes = Notes_Serialize | Notes_Deserialize;

export type Notes_Deserialize = {
	it?: string,
	en?: string,
};

export type Notes_Serialize = {
	it: string,
	en: string,
};

export type Noun = "table" | "collection" | "key" | "index" | "column" | "field";

export type NullDisplay = "null" | "dash" | "empty";

export type NumberDisplay = "raw" | "grouped";

export type Object = Object_Serialize | Object_Deserialize;

export type ObjectKind = "table" | "column" | "primary_key" | "index" | "foreign_key" | "view" | "function";

export type ObjectRef = ObjectRef_Serialize | ObjectRef_Deserialize;

export type ObjectRef_Deserialize = {
	kind: DdlKind,
	namespace: string,
	name: string,
	table?: string | null,
	signature?: string | null,
};

export type ObjectRef_Serialize = {
	kind: DdlKind,
	namespace: string,
	name: string,
	table: string | null,
	signature: string | null,
};

export type Object_Deserialize = {
	name: string,
	detail?: string | null,
};

export type Object_Serialize = {
	name: string,
	detail: string | null,
};

export type OdbcTarget = OdbcTarget_Serialize | OdbcTarget_Deserialize;

export type OdbcTarget_Deserialize = {
	dsn?: string,
	driver_path?: string,
	connect_string?: string,
	user?: string,
};

export type OdbcTarget_Serialize = {
	dsn: string,
	driver_path: string,
	connect_string: string,
	user: string,
};

export type OnRowError = 
"stop" | 
"skip";

export type OpenTransaction = OpenTransaction_Serialize | OpenTransaction_Deserialize;

export type OpenTransaction_Deserialize = {
	id: string,
	user?: string | null,
	database?: string | null,
	age_ms?: number | null,
	state?: string | null,
	query?: string | null,
};

export type OpenTransaction_Serialize = {
	id: string,
	user: string | null,
	database: string | null,
	age_ms: number | null,
	state: string | null,
	query: string | null,
};

export type OracleRole = "normal" | "sysdba" | "sysoper";

export type OracleTarget = OracleTarget_Serialize | OracleTarget_Deserialize;

export type OracleTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	sid?: boolean,
	connect_string?: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	role?: OracleRole,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
};

export type OracleTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	sid: boolean,
	connect_string: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	role: OracleRole,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
};

export type Order = {
	column: string,
	descending: boolean,
};

export type Outcome = Outcome_Serialize | Outcome_Deserialize;

export type Outcome_Deserialize = {
	command?: string | null,
	rows_affected?: number | null,
};

export type Outcome_Serialize = {
	command: string | null,
	rows_affected: number | null,
};

export type PackageFault = "needs_passphrase" | "wrong_passphrase" | "malformed" | "unsupported" | "wrong_extension";

export type PackedNamespace = PackedNamespace_Serialize | PackedNamespace_Deserialize;

export type PackedNamespace_Deserialize = {
	name: string,
	tables: PackedRelation_Deserialize[],
	views: PackedRelation_Deserialize[],
	functions: Function[],
	sequences: Object_Deserialize[],
	types: Object_Deserialize[],
	triggers: Object_Deserialize[],
	materialized_views?: PackedRelation_Deserialize[],
	dictionaries?: PackedRelation_Deserialize[],
	packages?: Object_Deserialize[],
	synonyms?: Object_Deserialize[],
	aliases?: PackedRelation_Deserialize[],
	data_streams?: PackedRelation_Deserialize[],
	details?: { [key in string]: string },
	managed?: Flavor | null,
	tags?: { [key in string]: RelationTag[] },
};

export type PackedNamespace_Serialize = {
	name: string,
	tables: PackedRelation_Serialize[],
	views: PackedRelation_Serialize[],
	functions: Function[],
	sequences: Object_Serialize[],
	types: Object_Serialize[],
	triggers: Object_Serialize[],
	materialized_views?: PackedRelation_Serialize[],
	dictionaries?: PackedRelation_Serialize[],
	packages?: Object_Serialize[],
	synonyms?: Object_Serialize[],
	aliases?: PackedRelation_Serialize[],
	data_streams?: PackedRelation_Serialize[],
	details?: { [key in string]: string },
	managed?: Flavor,
	tags?: { [key in string]: RelationTag[] },
};

export type PackedRelation = PackedRelation_Serialize | PackedRelation_Deserialize;

export type PackedRelation_Deserialize = [string, string, string, string, number | null];

export type PackedRelation_Serialize = [string, string, string, string, number | null];

export type PackedSchema = PackedSchema_Serialize | PackedSchema_Deserialize;

export type PackedSchema_Deserialize = {
	databases: string[],
	current_database: string,
	namespaces: PackedNamespace_Deserialize[],
};

export type PackedSchema_Serialize = {
	databases: string[],
	current_database: string,
	namespaces: PackedNamespace_Serialize[],
};

export type Page = Page_Serialize | Page_Deserialize;

export type Page_Deserialize = {
	columns: Column_Deserialize[],
	rows: ((string | null)[])[],
	has_more: boolean,
	next?: (string | null)[] | null,
	total?: number | null,
	estimated: boolean,
	sql: string,
	elapsed_ms: number | null,
};

export type Page_Serialize = {
	columns: Column_Serialize[],
	rows: ((string | null)[])[],
	has_more: boolean,
	next: (string | null)[] | null,
	total: number | null,
	estimated: boolean,
	sql: string,
	elapsed_ms: number | null,
};

export type Palette = "1" | "2" | "3";

export type ParamError = { kind: "missing"; name: string } | { kind: "invalid"; name: string; as_kind: ParamKind };

export type ParamKind = "text" | "number" | "date" | "boolean" | "null" | 
"raw";

export type ParamSlot = {
	name: string,
	label: string,
	spots: ParamSpot[],
};

export type ParamSpot = {
	from: number,
	to: number,
};

export type ParamUse = {
	name: string,
	kind: ParamKind,
	value: string,
	used_at: number | null,
	here: boolean,
};

export type ParamValue = {
	name: string,
	kind: ParamKind,
	value: string,
};

export type ParamsFound = {
	slots: ParamSlot[],
	history: ParamUse[],
};

export type PartitionSize = PartitionSize_Serialize | PartitionSize_Deserialize;

export type PartitionSize_Deserialize = {
	namespace: string,
	table: string,
	partition: string,
	parts?: number | null,
	rows?: number | null,
	bytes?: number | null,
	compressed?: boolean | null,
};

export type PartitionSize_Serialize = {
	namespace: string,
	table: string,
	partition: string,
	parts: number | null,
	rows: number | null,
	bytes: number | null,
	compressed?: boolean,
};

export type PdfBlock = PdfBlock_Serialize | PdfBlock_Deserialize;

export type PdfBlock_Deserialize = { kind: "heading"; text: string; count?: string | null; tone: PdfTone } | { kind: "paragraph"; text: string; tone: PdfTone } | { kind: "list"; items: string[]; tone: PdfTone } | { kind: "finding"; severity: string; tone: PdfTone; category: string; title: string; what: string; subjects_label: string; subjects: string[]; fix_label: string; fix?: string | null };

export type PdfBlock_Serialize = { kind: "heading"; text: string; count: string | null; tone: PdfTone } | { kind: "paragraph"; text: string; tone: PdfTone } | { kind: "list"; items: string[]; tone: PdfTone } | { kind: "finding"; severity: string; tone: PdfTone; category: string; title: string; what: string; subjects_label: string; subjects: string[]; fix_label: string; fix: string | null };

export type PdfReport = PdfReport_Serialize | PdfReport_Deserialize;

export type PdfReport_Deserialize = {
	style: PdfStyle,
	language: string,
	title: string,
	subtitle: string,
	lead?: string | null,
	tally: PdfTally[],
	blocks: PdfBlock_Deserialize[],
	footer: string,
	pages: string,
};

export type PdfReport_Serialize = {
	style: PdfStyle,
	language: string,
	title: string,
	subtitle: string,
	lead: string | null,
	tally: PdfTally[],
	blocks: PdfBlock_Serialize[],
	footer: string,
	pages: string,
};

export type PdfStyle = "registro" | "tavola" | "banco" | "listino";

export type PdfTally = {
	value: string,
	label: string,
	tone: PdfTone,
};

export type PdfTone = "plain" | "muted" | "high" | "medium" | "low" | "ok";

export type Plan = Plan_Serialize | Plan_Deserialize;

export type PlanReport = PlanReport_Serialize | PlanReport_Deserialize;

export type PlanReport_Deserialize = {
	plan: Plan_Deserialize,
	findings: Finding_Deserialize[],
	execution: Execution,
	unreadable?: string | null,
	sql: string,
	from: number,
	to: number,
	dialect?: Dialect | null,
	elapsed_ms: number | null,
	at: number | null,
};

export type PlanReport_Serialize = {
	plan: Plan_Serialize,
	findings: Finding_Serialize[],
	execution: Execution,
	unreadable: string | null,
	sql: string,
	from: number,
	to: number,
	dialect: Dialect | null,
	elapsed_ms: number | null,
	at: number | null,
};

export type Plan_Deserialize = {
	engine: Engine,
	analyzed: boolean,
	nodes: Node_Deserialize[],
	total_ms?: number | null,
	planning_ms?: number | null,
	slowest?: number | null,
	worst_estimate?: number | null,
	raw: string,
};

export type Plan_Serialize = {
	engine: Engine,
	analyzed: boolean,
	nodes: Node_Serialize[],
	total_ms: number | null,
	planning_ms: number | null,
	slowest: number | null,
	worst_estimate: number | null,
	raw: string,
};

export type Pooler = 
"transaction" | 
"session";

export type Position = {
	line: number,
	column: number,
};

export type PostgresTarget = PostgresTarget_Serialize | PostgresTarget_Deserialize;

export type PostgresTarget_Deserialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
	flavor?: Flavor | null,
};

export type PostgresTarget_Serialize = {
	host: string,
	port: number,
	database: string,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
	flavor?: Flavor,
};

export type Preferences = Preferences_Serialize | Preferences_Deserialize;

export type Preferences_Deserialize = {
	editor?: EditorPrefs_Deserialize,
	results?: ResultsPrefs_Deserialize,
	safety?: SafetyPrefs_Deserialize,
	keys?: { [key in string]: string },
	drivers?: DriverPrefs_Deserialize,
};

export type Preferences_Serialize = {
	editor: EditorPrefs_Serialize,
	results: ResultsPrefs_Serialize,
	safety: SafetyPrefs_Serialize,
	keys: { [key in string]: string },
	drivers: DriverPrefs_Serialize,
};

export type PreviewKind = "update" | "delete";

export type Principal = Principal_Serialize | Principal_Deserialize;

export type Principal_Deserialize = {
	name: string,
	host?: string | null,
	attributes: RoleAttribute[],
	member_of: string[],
};

export type Principal_Serialize = {
	name: string,
	host: string | null,
	attributes: RoleAttribute[],
	member_of: string[],
};

export type Principals = Principals_Serialize | Principals_Deserialize;

export type Principals_Deserialize = {
	principals: Principal_Deserialize[],
	grants: Grant_Deserialize[],
	partial: boolean,
	quotas: Quota[],
};

export type Principals_Serialize = {
	principals: Principal_Serialize[],
	grants: Grant_Serialize[],
	partial: boolean,
	quotas: Quota[],
};

export type ProfileSetting = ProfileSetting_Serialize | ProfileSetting_Deserialize;

export type ProfileSetting_Deserialize = {
	name: string,
	value?: string | null,
	min?: string | null,
	max?: string | null,
	writability?: string | null,
};

export type ProfileSetting_Serialize = {
	name: string,
	value: string | null,
	min: string | null,
	max: string | null,
	writability: string | null,
};

export type Progress = Progress_Serialize | Progress_Deserialize;

export type Progress_Deserialize = {
	done: number | null,
	total?: number | null,
	unit: Unit,
	rows: number | null,
	bytes: number | null,
	stage?: string | null,
};

export type Progress_Serialize = {
	done: number | null,
	total: number | null,
	unit: Unit,
	rows: number | null,
	bytes: number | null,
	stage: string | null,
};

export type ProxyConfig = ProxyConfig_Serialize | ProxyConfig_Deserialize;

export type ProxyConfig_Deserialize = {
	kind: ProxyKind,
	host: string,
	port: number,
	user?: string | null,
};

export type ProxyConfig_Serialize = {
	kind: ProxyKind,
	host: string,
	port: number,
	user: string | null,
};

export type ProxyKind = "socks5" | "http";

export type QueryProgress = QueryProgress_Serialize | QueryProgress_Deserialize;

export type QueryProgress_Deserialize = {
	read_rows: number | null,
	read_bytes: number | null,
	total_rows?: number | null,
};

export type QueryProgress_Serialize = {
	read_rows: number | null,
	read_bytes: number | null,
	total_rows: number | null,
};

export type QuickFilter = QuickFilter_Serialize | QuickFilter_Deserialize;

export type QuickFilter_Deserialize = {
	column: string,
	op: FilterOp,
	value?: string | null,
};

export type QuickFilter_Serialize = {
	column: string,
	op: FilterOp,
	value: string | null,
};

export type Quota = {
	name: string,
	all: boolean,
	applies_to: string[],
	except: string[],
	limits: QuotaLimit[],
};

export type QuotaCap = {
	what: string,
	max: number | null,
};

export type QuotaLimit = {
	seconds: number | null,
	caps: QuotaCap[],
};

export type RedisMode = { kind: "single" } | { kind: "cluster" } | 
{ kind: "sentinel"; master: string };

export type RedisTarget = RedisTarget_Serialize | RedisTarget_Deserialize;

export type RedisTarget_Deserialize = {
	host: string,
	port: number,
	database: number,
	user?: string,
	tls: TlsMode,
	ca_file?: string | null,
	ssh?: SshConfig_Deserialize | null,
	proxy?: ProxyConfig_Deserialize | null,
	client_cert?: ClientCert | null,
	mode?: RedisMode,
};

export type RedisTarget_Serialize = {
	host: string,
	port: number,
	database: number,
	user: string,
	tls: TlsMode,
	ca_file: string | null,
	ssh: SshConfig_Serialize | null,
	proxy: ProxyConfig_Serialize | null,
	client_cert: ClientCert | null,
	mode: RedisMode,
};

export type Relation = Relation_Serialize | Relation_Deserialize;

export type RelationTag = { kind: "hypertable"; chunks: number; compressed: number } | { kind: "continuous_aggregate" } | 
{ kind: "rls"; policies: number };

export type Relation_Deserialize = {
	name: string,
	columns: ColumnInfo[],
	indexes: Index[],
	foreign_keys: ForeignKey[],
	estimated_rows?: number | null,
	detail?: string | null,
};

export type Relation_Serialize = {
	name: string,
	columns: ColumnInfo[],
	indexes: Index[],
	foreign_keys: ForeignKey[],
	estimated_rows: number | null,
	detail?: string,
};

export type Report = {
	sections: ReportSection[],
};

export type ReportGroup = 
"info" | "clients" | "slowlog" | "memory" | 
"grid";

export type ReportSection = {
	id: string,
	group: ReportGroup,
	columns: string[],
	rows: string[][],
};

export type Resolution = "keep_both" | "replace" | "skip";

export type RestoreRequest = RestoreRequest_Serialize | RestoreRequest_Deserialize;

export type RestoreRequest_Deserialize = {
	job_id: string,
	session_id: string,
	path: string,
	confirm?: string | null,
};

export type RestoreRequest_Serialize = {
	job_id: string,
	session_id: string,
	path: string,
	confirm: string | null,
};

export type ResultShape = "rows" | "documents" | "keys";

export type ResultsPrefs = ResultsPrefs_Serialize | ResultsPrefs_Deserialize;

export type ResultsPrefs_Deserialize = {
	row_cap?: number,
	null_display?: NullDisplay,
	dates?: DateDisplay,
	numbers?: NumberDisplay,
	monitor_ms?: number,
};

export type ResultsPrefs_Serialize = {
	row_cap: number,
	null_display: NullDisplay,
	dates: DateDisplay,
	numbers: NumberDisplay,
	monitor_ms: number,
};

export type Risk = "update_without_where" | "delete_without_where" | "drop" | "truncate" | "alter" | 
"flush" | 
"server" | 
"full_scan";

export type RiskFound = RiskFound_Serialize | RiskFound_Deserialize;

export type RiskFound_Deserialize = {
	index: number,
	from: number,
	to: number,
	sql: string,
	risk: Risk,
	rows?: number | null,
};

export type RiskFound_Serialize = {
	index: number,
	from: number,
	to: number,
	sql: string,
	risk: Risk,
	rows: number | null,
};

export type RoleAttribute = "login" | "superuser" | "create_db" | "create_role" | 
"locked";

export type RowCompareEvent = RowCompareEvent_Serialize | RowCompareEvent_Deserialize;

export type RowCompareEvent_Deserialize = { kind: "plan"; plan: ColumnPlan_Deserialize } | { kind: "rows"; rows: RowDiff_Deserialize[] } | { kind: "progress"; a_rows: number | null; b_rows: number | null; waiting: number | null };

export type RowCompareEvent_Serialize = { kind: "plan"; plan: ColumnPlan_Serialize } | { kind: "rows"; rows: RowDiff_Serialize[] } | { kind: "progress"; a_rows: number | null; b_rows: number | null; waiting: number | null };

export type RowCompareRequest = {
	job_id: string,
	a: RowSource,
	b: RowSource,
	key: string[],
};

export type RowCompared = RowCompared_Serialize | RowCompared_Deserialize;

export type RowCompared_Deserialize = {
	plan: ColumnPlan_Deserialize,
	counts: RowCounts,
	a_rows: number | null,
	b_rows: number | null,
	same_cut: boolean,
	cancelled: boolean,
	elapsed_ms: number | null,
	a_dialect: Dialect,
	b_dialect: Dialect,
};

export type RowCompared_Serialize = {
	plan: ColumnPlan_Serialize,
	counts: RowCounts,
	a_rows: number | null,
	b_rows: number | null,
	same_cut: boolean,
	cancelled: boolean,
	elapsed_ms: number | null,
	a_dialect: Dialect,
	b_dialect: Dialect,
};

export type RowCounts = {
	added: number | null,
	removed: number | null,
	changed: number | null,
	same: number | null,
};

export type RowDiff = RowDiff_Serialize | RowDiff_Deserialize;

export type RowDiff_Deserialize = {
	state: RowState,
	cells: (string | null)[],
	changes: CellChange_Deserialize[],
};

export type RowDiff_Serialize = {
	state: RowState,
	cells: (string | null)[],
	changes: CellChange_Serialize[],
};

export type RowFault = {
	line: number | null,
	message: string,
};

export type RowPolicy = RowPolicy_Serialize | RowPolicy_Deserialize;

export type RowPolicy_Deserialize = {
	name: string,
	database: string,
	table: string,
	filter?: string | null,
	restrictive: boolean,
	all: boolean,
	applies_to: string[],
	except: string[],
};

export type RowPolicy_Serialize = {
	name: string,
	database: string,
	table: string,
	filter: string | null,
	restrictive: boolean,
	all: boolean,
	applies_to: string[],
	except: string[],
};

export type RowPreview = RowPreview_Serialize | RowPreview_Deserialize;

export type RowPreview_Deserialize = {
	kind: PreviewKind,
	columns: Column_Deserialize[],
	before: ((string | null)[])[],
	after: ((string | null)[])[],
	changed: number[],
	truncated: boolean,
	measured: boolean,
};

export type RowPreview_Serialize = {
	kind: PreviewKind,
	columns: Column_Serialize[],
	before: ((string | null)[])[],
	after: ((string | null)[])[],
	changed: number[],
	truncated: boolean,
	measured: boolean,
};

export type RowProbe = RowProbe_Serialize | RowProbe_Deserialize;

export type RowProbe_Deserialize = {
	columns: Column_Deserialize[],
	dialect: Dialect,
};

export type RowProbe_Serialize = {
	columns: Column_Serialize[],
	dialect: Dialect,
};

export type RowSide = "a" | "b";

export type RowSource = { kind: "query"; session_id: string; sql: string } | { kind: "table"; session_id: string; namespace: string; name: string };

export type RowState = "added" | "removed" | "changed" | "same";

export type Rule = "seq_scan" | "fk_without_index" | "row_estimate" | "spill" | "nested_loop" | "function_on_indexed_column" | "implicit_cast" | "select_star_sort" | "missing_limit" | 
"docs_examined" | 
"in_memory_sort";

export type RuleKind = 
"column" | 
"row" | 
"server";

export type RunConfirm = RunConfirm_Serialize | RunConfirm_Deserialize;

export type RunConfirm_Deserialize = {
	risks: boolean,
	name?: string | null,
};

export type RunConfirm_Serialize = {
	risks: boolean,
	name: string | null,
};

export type RunEvent = RunEvent_Serialize | RunEvent_Deserialize;

export type RunEvent_Deserialize = { type: "statement"; index: number; from: number; to: number; sql: string; unsortable?: Unsortable | null } | { type: "columns"; index: number; columns: Column_Deserialize[] } | { type: "rows"; index: number; rows: ((string | null)[])[] } | 
{ type: "progress"; index: number; progress: QueryProgress_Deserialize } | 
{ type: "documents"; index: number; documents: string[] } | { type: "done"; index: number; elapsed_ms: number | null; row_count: number | null; rows_affected?: number | null; command?: string | null; truncated: boolean; 
row_limit?: number | null } | { type: "failed"; index: number; elapsed_ms: number | null; error: DriverError_Deserialize; position?: Position | null; at?: number | null } | 
{ type: "relocked" } | 
{ type: "reconnected"; index: number; write: boolean; transaction_lost: boolean } | 
{ type: "lost"; index: number; write: boolean; error: CoreError_Deserialize } | 
{ type: "preview"; index: number; preview: RowPreview_Deserialize } | 
{ type: "implicit_commit"; index: number } | 
{ type: "transaction_lost" } | 
{ type: "transaction"; state: TxState_Deserialize } | { type: "finished"; elapsed_ms: number | null; cancelled: boolean };

export type RunEvent_Serialize = { type: "statement"; index: number; from: number; to: number; sql: string; unsortable: Unsortable | null } | { type: "columns"; index: number; columns: Column_Serialize[] } | { type: "rows"; index: number; rows: ((string | null)[])[] } | 
{ type: "progress"; index: number; progress: QueryProgress_Serialize } | 
{ type: "documents"; index: number; documents: string[] } | { type: "done"; index: number; elapsed_ms: number | null; row_count: number | null; rows_affected: number | null; command: string | null; truncated: boolean; 
row_limit: number | null } | { type: "failed"; index: number; elapsed_ms: number | null; error: DriverError_Serialize; position: Position | null; at: number | null } | 
{ type: "relocked" } | 
{ type: "reconnected"; index: number; write: boolean; transaction_lost: boolean } | 
{ type: "lost"; index: number; write: boolean; error: CoreError_Serialize } | 
{ type: "preview"; index: number; preview: RowPreview_Serialize } | 
{ type: "implicit_commit"; index: number } | 
{ type: "transaction_lost" } | 
{ type: "transaction"; state: TxState_Serialize } | { type: "finished"; elapsed_ms: number | null; cancelled: boolean };

export type RunRequest = RunRequest_Serialize | RunRequest_Deserialize;

export type RunRequest_Deserialize = {
	run_id: string,
	session_id: string,
	text: string,
	scope: RunScope,
	confirm?: RunConfirm_Deserialize,
	sort?: SortKey[],
	all_rows?: boolean,
	dry_run?: boolean,
	params?: ParamValue[] | null,
	continue_on_error?: boolean,
};

export type RunRequest_Serialize = {
	run_id: string,
	session_id: string,
	text: string,
	scope: RunScope,
	confirm: RunConfirm_Serialize,
	sort: SortKey[],
	all_rows: boolean,
	dry_run: boolean,
	params: ParamValue[] | null,
	continue_on_error: boolean,
};

export type RunScope = { kind: "script" } | { kind: "selection"; from: number; to: number } | { kind: "statement"; cursor: number };

export type SafetyPrefs = SafetyPrefs_Serialize | SafetyPrefs_Deserialize;

export type SafetyPrefs_Deserialize = {
	timeout_local?: number,
	timeout_development?: number,
	timeout_staging?: number,
	timeout_production?: number,
	unlock_minutes?: number,
	unlock_mode?: UnlockMode,
	confirm_mode?: ConfirmMode,
};

export type SafetyPrefs_Serialize = {
	timeout_local: number,
	timeout_development: number,
	timeout_staging: number,
	timeout_production: number,
	unlock_minutes: number,
	unlock_mode: UnlockMode,
	confirm_mode: ConfirmMode,
};

export type SampleColumn = {
	name: string,
	inferred: Inferred,
	type_name: string,
	nullable: boolean,
};

export type SavedDesign = SavedDesign_Serialize | SavedDesign_Deserialize;

export type SavedDesign_Deserialize = {
	namespace: string,
	engine: string,
	origin?: string | null,
	draft?: string | null,
};

export type SavedDesign_Serialize = {
	namespace: string,
	engine: string,
	origin: string | null,
	draft: string | null,
};

export type SavedNote = {
	scope: NoteScope,
	scope_id: string,
	body: string,
	updated_at: number | null,
};

export type SavedTab = SavedTab_Serialize | SavedTab_Deserialize;

export type SavedTab_Deserialize = {
	connection_id: string,
	title: string,
	text?: string,
	view?: TabView,
	table?: SavedTable | null,
	database?: string | null,
	params?: ParamValue[],
	design?: SavedDesign_Deserialize | null,
	trail?: string | null,
};

export type SavedTab_Serialize = {
	connection_id: string,
	title: string,
	text: string,
	view: TabView,
	table: SavedTable | null,
	database: string | null,
	params?: ParamValue[],
	design?: SavedDesign_Serialize,
	trail?: string,
};

export type SavedTable = {
	namespace: string,
	name: string,
};

export type Schema = Schema_Serialize | Schema_Deserialize;

export type SchemaKind = 
"declared" | 
"inferred" | "none";

export type SchemaStart = SchemaStart_Serialize | SchemaStart_Deserialize;

export type SchemaStart_Deserialize = {
	schema: PackedSchema_Deserialize,
	pending: NamespaceSize[],
};

export type SchemaStart_Serialize = {
	schema: PackedSchema_Serialize,
	pending: NamespaceSize[],
};

export type Schema_Deserialize = {
	databases: string[],
	current_database: string,
	namespaces: Namespace_Deserialize[],
};

export type Schema_Serialize = {
	databases: string[],
	current_database: string,
	namespaces: Namespace_Serialize[],
};

export type Script = Script_Serialize | Script_Deserialize;

export type Script_Deserialize = {
	id: string,
	name: string,
	folder: string,
	tags: string[],
	connection_id?: string | null,
	sql: string,
	params?: ParamValue[],
	created_at: number | null,
	updated_at: number | null,
};

export type Script_Serialize = {
	id: string,
	name: string,
	folder: string,
	tags: string[],
	connection_id: string | null,
	sql: string,
	params: ParamValue[],
	created_at: number | null,
	updated_at: number | null,
};

export type SearchDone = {
	searched: number,
	of: number,
	with_hits: number,
	rows: number | null,
	failed: number,
	cancelled: boolean,
	out_of_time: boolean,
	elapsed_ms: number | null,
};

export type SearchEvent = 
{ kind: "table"; index: number; of: number; name: string } | { kind: "hits"; hits: TableHits } | { kind: "failed"; namespace: string; name: string; message: string };

export type SearchPlanned = SearchPlanned_Serialize | SearchPlanned_Deserialize;

export type SearchPlanned_Deserialize = {
	columns: string[][],
	sample?: string | null,
};

export type SearchPlanned_Serialize = {
	columns: string[][],
	sample: string | null,
};

export type SearchSpec = {
	value: string,
	mode: Match,
	case_sensitive: boolean,
	kinds: Kinds,
};

export type SearchTable = {
	table: TableRef,
	columns: ColumnType[],
	key: string[],
};

export type SecCategory = "accounts" | "network" | "version" | "data" | "hygiene";

export type SecFinding = SecFinding_Serialize | SecFinding_Deserialize;

export type SecFinding_Deserialize = {
	id: string,
	category: SecCategory,
	verdict: SecVerdict,
	severity?: SecSeverity | null,
	reason?: SecSkip | null,
	subjects: SecSubject_Deserialize[],
	fix?: string | null,
	note?: string | null,
};

export type SecFinding_Serialize = {
	id: string,
	category: SecCategory,
	verdict: SecVerdict,
	severity: SecSeverity | null,
	reason: SecSkip | null,
	subjects: SecSubject_Serialize[],
	fix: string | null,
	note: string | null,
};

export type SecSeverity = "high" | "medium" | "low";

export type SecSkip = 
"engine" | 
"privilege" | 
"not_detectable";

export type SecSubject = SecSubject_Serialize | SecSubject_Deserialize;

export type SecSubject_Deserialize = {
	name: string,
	detail?: string | null,
};

export type SecSubject_Serialize = {
	name: string,
	detail: string | null,
};

export type SecVerdict = "found" | "passed" | "skipped";

export type Secrets = Secrets_Serialize | Secrets_Deserialize;

export type Secrets_Deserialize = {
	password?: string | null,
	ssh?: string | null,
	proxy?: string | null,
	tls_key?: string | null,
	jumps?: (string | null)[],
};

export type Secrets_Serialize = {
	password: string | null,
	ssh: string | null,
	proxy: string | null,
	tls_key: string | null,
	jumps: (string | null)[],
};

export type SecurityReport = SecurityReport_Serialize | SecurityReport_Deserialize;

export type SecurityReport_Deserialize = {
	product: string,
	version: string,
	database: string,
	checks: SecFinding_Deserialize[],
	versions_snapshot: string,
	versions_sources: string[],
};

export type SecurityReport_Serialize = {
	product: string,
	version: string,
	database: string,
	checks: SecFinding_Serialize[],
	versions_snapshot: string,
	versions_sources: string[],
};

export type ServerInfo = {
	product: string,
	version: string,
	database: string,
};

export type ServerSession = ServerSession_Serialize | ServerSession_Deserialize;

export type ServerSession_Deserialize = {
	id: string,
	user?: string | null,
	database?: string | null,
	client?: string | null,
	application?: string | null,
	state?: string | null,
	query?: string | null,
	duration_ms?: number | null,
	wait?: string | null,
	blocked_by: string[],
};

export type ServerSession_Serialize = {
	id: string,
	user: string | null,
	database: string | null,
	client: string | null,
	application: string | null,
	state: string | null,
	query: string | null,
	duration_ms: number | null,
	wait: string | null,
	blocked_by: string[],
};

export type SessionEvent = SessionEvent_Serialize | SessionEvent_Deserialize;

export type SessionEvent_Deserialize = 
{ type: "lost"; session_id: string; error: CoreError_Deserialize } | { type: "reconnected"; session_id: string; transaction_lost: boolean; 
server: ServerInfo } | { type: "idle_closed"; session_id: string; transaction_lost: boolean };

export type SessionEvent_Serialize = 
{ type: "lost"; session_id: string; error: CoreError_Serialize } | { type: "reconnected"; session_id: string; transaction_lost: boolean; 
server: ServerInfo } | { type: "idle_closed"; session_id: string; transaction_lost: boolean };

export type SessionInfo = SessionInfo_Serialize | SessionInfo_Deserialize;

export type SessionInfo_Deserialize = {
	session_id: string,
	connection_id: string,
	dialect?: Dialect | null,
	caps: Capabilities,
	server: ServerInfo,
	environment: Environment,
	transaction_lost: boolean,
	edition?: Edition_Deserialize | null,
};

export type SessionInfo_Serialize = {
	session_id: string,
	connection_id: string,
	dialect: Dialect | null,
	caps: Capabilities,
	server: ServerInfo,
	environment: Environment,
	transaction_lost: boolean,
	edition: Edition_Serialize | null,
};

export type SessionOptions = SessionOptions_Serialize | SessionOptions_Deserialize;

export type SessionOptions_Deserialize = {
	init?: string,
	keepalive_secs?: number,
	reconnect?: boolean,
	idle_minutes?: number,
	row_limit?: number | null,
};

export type SessionOptions_Serialize = {
	init: string,
	keepalive_secs: number,
	reconnect: boolean,
	idle_minutes: number,
	row_limit: number | null,
};

export type SettingsProfile = SettingsProfile_Serialize | SettingsProfile_Deserialize;

export type SettingsProfile_Deserialize = {
	name: string,
	all: boolean,
	applies_to: string[],
	except: string[],
	settings: ProfileSetting_Deserialize[],
	inherits: string[],
};

export type SettingsProfile_Serialize = {
	name: string,
	all: boolean,
	applies_to: string[],
	except: string[],
	settings: ProfileSetting_Serialize[],
	inherits: string[],
};

export type Severity = "high" | "medium" | "low";

export type Shape = Shape_Serialize | Shape_Deserialize;

export type Shape_Deserialize = { kind: "int"; min: number | null; max: number | null } | { kind: "decimal"; precision: number; scale: number } | { kind: "float" } | { kind: "bool" } | { kind: "date" } | { kind: "date_time" } | { kind: "time" } | { kind: "uuid" } | { kind: "text"; max?: number | null } | { kind: "json" } | { kind: "binary" } | { kind: "enum"; values: string[] } | { kind: "other" };

export type Shape_Serialize = { kind: "int"; min: number | null; max: number | null } | { kind: "decimal"; precision: number; scale: number } | { kind: "float" } | { kind: "bool" } | { kind: "date" } | { kind: "date_time" } | { kind: "time" } | { kind: "uuid" } | { kind: "text"; max: number | null } | { kind: "json" } | { kind: "binary" } | { kind: "enum"; values: string[] } | { kind: "other" };

export type Sizes = Sizes_Serialize | Sizes_Deserialize;

export type Sizes_Deserialize = {
	tables: TableSize_Deserialize[],
	indexes: IndexUsage_Deserialize[],
	usage_known: boolean,
	partitions?: PartitionSize_Deserialize[],
	hypertables?: HypertableSize_Deserialize[],
};

export type Sizes_Serialize = {
	tables: TableSize_Serialize[],
	indexes: IndexUsage_Serialize[],
	usage_known: boolean,
	partitions?: PartitionSize_Serialize[],
	hypertables?: HypertableSize_Serialize[],
};

export type SlowQueries = SlowQueries_Serialize | SlowQueries_Deserialize;

export type SlowQueries_Deserialize = {
	source?: SlowSource | null,
	gap?: GapReason | null,
	queries: SlowQuery_Deserialize[],
};

export type SlowQueries_Serialize = {
	source: SlowSource | null,
	gap: GapReason | null,
	queries: SlowQuery_Serialize[],
};

export type SlowQuery = SlowQuery_Serialize | SlowQuery_Deserialize;

export type SlowQuery_Deserialize = {
	text: string,
	sample?: string | null,
	calls?: number | null,
	total_ms?: number | null,
	mean_ms?: number | null,
	rows?: number | null,
};

export type SlowQuery_Serialize = {
	text: string,
	sample: string | null,
	calls: number | null,
	total_ms: number | null,
	mean_ms: number | null,
	rows: number | null,
};

export type SlowSource = "pg_stat_statements" | "performance_schema" | "query_store" | "plan_cache" | "statement_statistics" | "query_log" | 
"statements_summary";

export type Snippet = Snippet_Serialize | Snippet_Deserialize;

export type Snippet_Deserialize = {
	id: string,
	name: string,
	trigger: string,
	description: string,
	body: string,
	dialect?: string | null,
};

export type Snippet_Serialize = {
	id: string,
	name: string,
	trigger: string,
	description: string,
	body: string,
	dialect: string | null,
};

export type SortKey = {
	column: number,
	descending: boolean,
};

export type SoundDetail = "all" | "reduced";

export type SoundTimbre = "marimba" | "ciotola";

export type Spill = Spill_Serialize | Spill_Deserialize;

export type SpillKind = "sort" | "hash" | "aggregate";

export type Spill_Deserialize = {
	kind: SpillKind,
	kb?: number | null,
};

export type Spill_Serialize = {
	kind: SpillKind,
	kb: number | null,
};

export type SqliteTarget = {
	path: string,
	read_only: boolean,
};

export type SshAuth = { method: "password" } | { method: "key"; path: string } | { method: "agent" };

export type SshConfig = SshConfig_Serialize | SshConfig_Deserialize;

export type SshConfig_Deserialize = {
	host: string,
	port: number,
	user: string,
	auth: SshAuth,
	jumps?: SshHop[],
};

export type SshConfig_Serialize = {
	host: string,
	port: number,
	user: string,
	auth: SshAuth,
	jumps: SshHop[],
};

export type SshHop = {
	host: string,
	port: number,
	user: string,
	auth: SshAuth,
};

export type Style = "registro" | "tavola" | "banco" | "listino";

export type Summary = Summary_Serialize | Summary_Deserialize;

export type SummaryColumn = {
	name: string,
	numeric: boolean,
};

export type SummarySource = SummarySource_Serialize | SummarySource_Deserialize;

export type SummarySource_Deserialize = { kind: "query"; sql: string } | { kind: "table"; browse: Browse_Deserialize };

export type SummarySource_Serialize = { kind: "query"; sql: string } | { kind: "table"; browse: Browse_Serialize };

export type Summary_Deserialize = 
{ kind: "stats"; columns: SummaryColumn[] } | 
{ kind: "group"; x: string; values: GroupValue_Deserialize[]; limit: number };

export type Summary_Serialize = 
{ kind: "stats"; columns: SummaryColumn[] } | 
{ kind: "group"; x: string; values: GroupValue_Serialize[]; limit: number };

export type TabView = "query" | "data" | "schema";

export type TableDesign = TableDesign_Serialize | TableDesign_Deserialize;

export type TableDesign_Deserialize = {
	namespace: string,
	name: string,
	comment?: string | null,
	columns: DesignColumn_Deserialize[],
	primary_key: string[],
	pk_name?: string | null,
	indexes: DesignIndex_Deserialize[],
	foreign_keys: DesignForeignKey_Deserialize[],
	checks: DesignCheck_Deserialize[],
	triggers?: DesignTrigger[],
};

export type TableDesign_Serialize = {
	namespace: string,
	name: string,
	comment: string | null,
	columns: DesignColumn_Serialize[],
	primary_key: string[],
	pk_name: string | null,
	indexes: DesignIndex_Serialize[],
	foreign_keys: DesignForeignKey_Serialize[],
	checks: DesignCheck_Serialize[],
	triggers: DesignTrigger[],
};

export type TableHits = {
	namespace: string,
	name: string,
	total: number | null,
	columns: ColumnHits[],
	shown: string[],
	key: string[],
	rows: ((string | null)[])[],
	elapsed_ms: number | null,
};

export type TablePosition = {
	namespace: string,
	table: string,
	x: number | null,
	y: number | null,
};

export type TableRef = {
	namespace: string,
	name: string,
};

export type TableSize = TableSize_Serialize | TableSize_Deserialize;

export type TableSize_Deserialize = {
	namespace: string,
	name: string,
	rows?: number | null,
	data_bytes?: number | null,
	index_bytes?: number | null,
	total_bytes?: number | null,
};

export type TableSize_Serialize = {
	namespace: string,
	name: string,
	rows: number | null,
	data_bytes: number | null,
	index_bytes: number | null,
	total_bytes: number | null,
};

export type Target = Target_Serialize | Target_Deserialize;

export type Target_Deserialize = {
	driver: "postgres",
} & PostgresTarget_Deserialize | {
	driver: "sqlite",
} & SqliteTarget | {
	driver: "mysql",
} & MysqlTarget_Deserialize | {
	driver: "sqlserver",
} & MssqlTarget_Deserialize | 
{
	driver: "cockroach",
} & PostgresTarget_Deserialize | {
	driver: "duckdb",
} & DuckdbTarget | {
	driver: "libsql",
} & LibsqlTarget_Deserialize | {
	driver: "mongodb",
} & MongoTarget_Deserialize | {
	driver: "clickhouse",
} & ClickhouseTarget_Deserialize | 
{
	driver: "redis",
} & RedisTarget_Deserialize | 
{
	driver: "cassandra",
} & CassandraTarget_Deserialize | {
	driver: "dynamodb",
} & DynamodbTarget_Deserialize | 
{
	driver: "elasticsearch",
} & ElasticTarget_Deserialize | {
	driver: "firebird",
} & FirebirdTarget_Deserialize | {
	driver: "oracle",
} & OracleTarget_Deserialize | 
{
	driver: "db2",
} & Db2Target_Deserialize | 
{
	driver: "hana",
} & HanaTarget_Deserialize | 
{
	driver: "odbc",
} & OdbcTarget_Deserialize;

export type Target_Serialize = {
	driver: "postgres",
} & PostgresTarget_Serialize | {
	driver: "sqlite",
} & SqliteTarget | {
	driver: "mysql",
} & MysqlTarget_Serialize | {
	driver: "sqlserver",
} & MssqlTarget_Serialize | 
{
	driver: "cockroach",
} & PostgresTarget_Serialize | {
	driver: "duckdb",
} & DuckdbTarget | {
	driver: "libsql",
} & LibsqlTarget_Serialize | {
	driver: "mongodb",
} & MongoTarget_Serialize | {
	driver: "clickhouse",
} & ClickhouseTarget_Serialize | 
{
	driver: "redis",
} & RedisTarget_Serialize | 
{
	driver: "cassandra",
} & CassandraTarget_Serialize | {
	driver: "dynamodb",
} & DynamodbTarget_Serialize | 
{
	driver: "elasticsearch",
} & ElasticTarget_Serialize | {
	driver: "firebird",
} & FirebirdTarget_Serialize | {
	driver: "oracle",
} & OracleTarget_Serialize | 
{
	driver: "db2",
} & Db2Target_Serialize | 
{
	driver: "hana",
} & HanaTarget_Serialize | 
{
	driver: "odbc",
} & OdbcTarget_Serialize;

export type Template = { kind: "none" } | { kind: "model" } | 
{ kind: "follows"; id: string; overrides: string[] };

export type ThemeMode = "system" | "light" | "dark";

export type TlsMode = "disable" | "verify" | "insecure";

export type Tool = "activity" | "monitor" | "space" | "users" | "compare" | "rows" | "backup" | "transfer" | "seclab" | "search" | "generate" | 
"server" | 
"cluster";

export type ToolCheck = ToolCheck_Serialize | ToolCheck_Deserialize;

export type ToolCheck_Deserialize = {
	dump?: BackupTool_Deserialize | null,
	restore: BackupTool_Deserialize[],
	formats: BackupFormat[],
	builtin: boolean,
	supported: boolean,
	server_side: boolean,
	can_restore: boolean,
	schema_only: boolean,
};

export type ToolCheck_Serialize = {
	dump: BackupTool_Serialize | null,
	restore: BackupTool_Serialize[],
	formats: BackupFormat[],
	builtin: boolean,
	supported: boolean,
	server_side: boolean,
	can_restore: boolean,
	schema_only: boolean,
};

export type ToolFamily = "postgres" | "mysql";

export type ToolFault = 
{ kind: "unsupported" } | 
{ kind: "missing"; tool: string } | 
{ kind: "failed"; tool: string; message: string } | 
{ kind: "not_read" } | 
{ kind: "wrong_extension" } | 
{ kind: "file"; message: string } | 
{ kind: "empty" } | 
{ kind: "key_missing"; column: string } | 
{ kind: "duplicate_key"; side: RowSide; key: (string | null)[] } | 
{ kind: "production" } | 
{ kind: "too_large"; rows: number | null; limit_mb: number };

export type TreeNode = { kind: "folder"; id: string } | { kind: "connection"; id: string };

export type TreeShape = {
	databases: boolean,
	namespaces: NamespaceLevel,
	groups: Group[],
	server: boolean,
};

export type TunnelError = { kind: "host_key_unknown"; host: string; port: number; fingerprint: string; key: string } | { kind: "host_key_changed"; host: string } | { kind: "auth"; message: string } | { kind: "connect"; message: string } | { kind: "proxy"; message: string } | { kind: "proxy_auth"; message: string };

export type TxState = TxState_Serialize | TxState_Deserialize;

export type TxState_Deserialize = {
	mode: CommitMode,
	open: boolean,
	writes: number,
	rows: number | null,
	open_ms?: number | null,
	failed: boolean,
	dry_run: boolean,
};

export type TxState_Serialize = {
	mode: CommitMode,
	open: boolean,
	writes: number,
	rows: number | null,
	open_ms: number | null,
	failed: boolean,
	dry_run: boolean,
};

export type UiScale = number;

export type Unit = "rows" | "bytes" | "tables" | "pages";

export type UnlockMode = "timed" | "session" | "open";

export type Unsortable = "not_select" | "writes" | "multiple";

export type UpdateFault = { code: "network"; detail: string } | { code: "invalid"; detail: string } | { code: "signature" } | { code: "no_update" } | { code: "not_downloaded" } | { code: "install"; detail: string } | { code: "restart"; detail: string } | { code: "link"; detail: string };

export type UpdatePrefs = UpdatePrefs_Serialize | UpdatePrefs_Deserialize;

export type UpdatePrefs_Deserialize = {
	check_on_start: boolean,
	last_check?: number | null,
};

export type UpdatePrefs_Serialize = {
	check_on_start: boolean,
	last_check: number | null,
};

export type UpdateProgress = UpdateProgress_Serialize | UpdateProgress_Deserialize;

export type UpdateProgress_Deserialize = {
	downloaded: number | null,
	total?: number | null,
};

export type UpdateProgress_Serialize = {
	downloaded: number | null,
	total: number | null,
};

export type UpdateStatus = UpdateStatus_Serialize | UpdateStatus_Deserialize;

export type UpdateStatus_Deserialize = 
{ status: "skipped" } | { status: "up_to_date"; current: string } | { status: "available"; current: string; version: string; notes: Notes_Deserialize; pub_date?: string | null };

export type UpdateStatus_Serialize = 
{ status: "skipped" } | { status: "up_to_date"; current: string } | { status: "available"; current: string; version: string; notes: Notes_Serialize; pub_date: string | null };

export type ValueSearch = {
	job_id: string,
	session_id: string,
	spec: SearchSpec,
	tables: SearchTable[],
	row_limit: number,
	budget_secs: number,
};

export type VarError = { kind: "undefined"; name: string; field: string } | { kind: "secret"; name: string; field: string } | { kind: "syntax"; field: string } | { kind: "bad_number"; field: string; value: string };

export type VarPreview = { kind: "value"; text: string } | { kind: "problem"; error: VarError };

export type Visibility = Visibility_Serialize | Visibility_Deserialize;

export type Visibility_Deserialize = {
	databases?: { [key in string]: boolean },
	schemas?: { [key in string]: { [key in string]: boolean } },
};

export type Visibility_Serialize = {
	databases: { [key in string]: boolean },
	schemas: { [key in string]: { [key in string]: boolean } },
};

export type Vocabulary = {
	table: Noun,
	column: Noun,
};

export type Volume = number;

export type Workspace = Workspace_Serialize | Workspace_Deserialize;

export type WorkspaceSession = WorkspaceSession_Serialize | WorkspaceSession_Deserialize;

export type WorkspaceSession_Deserialize = {
	tabs?: SavedTab_Deserialize[],
	active?: number | null,
	split?: number | null,
	history_open?: boolean,
	widths?: { [key in string]: number },
};

export type WorkspaceSession_Serialize = {
	tabs: SavedTab_Serialize[],
	active: number | null,
	split: number | null,
	history_open: boolean,
	widths: { [key in string]: number },
};

export type Workspace_Deserialize = {
	id: string,
	name: string,
	folders: string[],
	connections: string[],
	session: WorkspaceSession_Deserialize,
	vars?: { [key in string]: string },
};

export type Workspace_Serialize = {
	id: string,
	name: string,
	folders: string[],
	connections: string[],
	session: WorkspaceSession_Serialize,
	vars: { [key in string]: string },
};

export type Workspaces = Workspaces_Serialize | Workspaces_Deserialize;

export type Workspaces_Deserialize = {
	active?: string | null,
	all: WorkspaceSession_Deserialize,
	list: Workspace_Deserialize[],
};

export type Workspaces_Serialize = {
	active: string | null,
	all: WorkspaceSession_Serialize,
	list: Workspace_Serialize[],
};

export type WriteAccess = WriteAccess_Serialize | WriteAccess_Deserialize;

export type WriteAccess_Deserialize = {
	guarded: boolean,
	open: boolean,
	mode: UnlockMode,
	open_ms?: number | null,
};

export type WriteAccess_Serialize = {
	guarded: boolean,
	open: boolean,
	mode: UnlockMode,
	open_ms: number | null,
};

async function typedError<T, E>(result: Promise<T>): Promise<{ status: "ok"; data: T } | { status: "error"; error: E }> {
    try {
        return { status: "ok", data: await result };
    } catch (e) {
        if (e instanceof Error) throw e;
        return { status: "error", error: e as any };
    }
}

