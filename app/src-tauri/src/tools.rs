use std::sync::Arc;

use nd_core::{
    Activity, Backed, BackupRequest, CompareRequest, Comparison, Core, DataExport, DataExported,
    DataImport, DataImportSample, DataImportSource, DataImported, DesignEngine, DesignScript,
    FakeRequest, GenDone, GenPreview, GenProfile, KillMode, MaintenanceDone, MaintenanceRequest,
    Monitor, Principals, Progress, RestoreRequest, RowCompareEvent, RowCompareRequest, RowCompared,
    RowProbe, RowSource, SearchDone, SearchEvent, SearchPlanned, SearchTable, SecurityReport,
    Sizes, SlowQueries, TableDesign, ToolCheck, ToolFamily, ValueSearch,
};
use tauri::State;
use tauri::ipc::Channel;

use crate::error::CommandError;

type Ctx<'a> = State<'a, Arc<Core>>;
type Res<T> = Result<T, CommandError>;

fn relay(events: Channel<Progress>) -> impl Fn(Progress) + Send + Sync {
    move |p| {
        if let Err(e) = events.send(p) {
            tracing::debug!(error = %e, "progress event dropped");
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn data_export(
    core: Ctx<'_>,
    request: DataExport,
    events: Channel<Progress>,
) -> Res<DataExported> {
    core.export_data(request, relay(events)).await
}

#[tauri::command]
#[specta::specta]
pub async fn data_import_sample(
    core: Ctx<'_>,
    session_id: String,
    source: DataImportSource,
) -> Res<DataImportSample> {
    core.import_sample(&session_id, source).await
}

#[tauri::command]
#[specta::specta]
pub async fn data_import(
    core: Ctx<'_>,
    request: DataImport,
    events: Channel<Progress>,
) -> Res<DataImported> {
    core.import_data(request, relay(events)).await
}

#[tauri::command]
#[specta::specta]
pub async fn job_cancel(core: Ctx<'_>, job_id: String) -> Res<()> {
    core.cancel_job(&job_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn schema_compare(core: Ctx<'_>, request: CompareRequest) -> Res<Comparison> {
    core.compare_schemas(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn table_design(
    core: Ctx<'_>,
    session_id: String,
    namespace: String,
    name: String,
) -> Res<TableDesign> {
    core.table_design(&session_id, &namespace, &name).await
}

#[tauri::command]
#[specta::specta]
pub fn table_design_script(
    engine: DesignEngine,
    before: Option<TableDesign>,
    after: TableDesign,
) -> DesignScript {
    nd_core::design_script(before.as_ref(), &after, engine)
}

#[tauri::command]
#[specta::specta]
pub async fn row_probe(core: Ctx<'_>, source: RowSource) -> Res<RowProbe> {
    core.row_probe(source).await
}

#[tauri::command]
#[specta::specta]
pub async fn row_compare(
    core: Ctx<'_>,
    request: RowCompareRequest,
    events: Channel<RowCompareEvent>,
) -> Res<RowCompared> {
    core.compare_rows(request, move |e| {
        if let Err(err) = events.send(e) {
            tracing::debug!(error = %err, "comparison event dropped");
        }
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub fn row_script(
    core: Ctx<'_>,
    compare_id: String,
    namespace: String,
    name: String,
) -> Res<Vec<String>> {
    core.row_script(&compare_id, &namespace, &name)
}

#[tauri::command]
#[specta::specta]
pub fn row_forget(core: Ctx<'_>, compare_id: String) -> Res<()> {
    core.row_forget(&compare_id)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_tools(core: Ctx<'_>, session_id: String) -> Res<ToolCheck> {
    core.backup_tools(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub fn tool_dir_set(core: Ctx<'_>, family: ToolFamily, dir: Option<String>) -> Res<()> {
    core.set_tool_dir(family, dir)
}

#[tauri::command]
#[specta::specta]
pub async fn backup_run(
    core: Ctx<'_>,
    request: BackupRequest,
    events: Channel<Progress>,
) -> Res<Backed> {
    core.backup(request, relay(events)).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    core: Ctx<'_>,
    request: RestoreRequest,
    events: Channel<Progress>,
) -> Res<Backed> {
    core.restore(request, relay(events)).await
}

#[tauri::command]
#[specta::specta]
pub async fn server_activity(core: Ctx<'_>, session_id: String) -> Res<Activity> {
    core.activity(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn server_sizes(core: Ctx<'_>, session_id: String) -> Res<Sizes> {
    core.sizes(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn server_monitor(core: Ctx<'_>, session_id: String) -> Res<Monitor> {
    core.monitor(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn server_slow_queries(core: Ctx<'_>, session_id: String) -> Res<SlowQueries> {
    core.slow_queries(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn server_maintain(core: Ctx<'_>, request: MaintenanceRequest) -> Res<MaintenanceDone> {
    core.maintain(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn server_kill(
    core: Ctx<'_>,
    session_id: String,
    target: String,
    mode: KillMode,
    confirm: Option<String>,
) -> Res<bool> {
    core.kill_session(&session_id, &target, mode, confirm.as_deref())
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn principals_list(core: Ctx<'_>, session_id: String) -> Res<Principals> {
    core.principals(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn clickhouse_access(
    core: Ctx<'_>,
    session_id: String,
) -> Res<nd_core::ClickhouseAccess> {
    core.clickhouse_access(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn security_check(core: Ctx<'_>, session_id: String) -> Res<SecurityReport> {
    core.security_check(&session_id).await
}

#[tauri::command]
#[specta::specta]
pub fn value_search_plan(
    core: Ctx<'_>,
    session_id: String,
    spec: nd_sql::SearchSpec,
    tables: Vec<SearchTable>,
) -> Res<SearchPlanned> {
    core.search_plan(&session_id, &spec, &tables)
}

#[tauri::command]
#[specta::specta]
pub async fn value_search(
    core: Ctx<'_>,
    request: ValueSearch,
    events: Channel<SearchEvent>,
) -> Res<SearchDone> {
    core.search_value(request, move |e| {
        if let Err(err) = events.send(e) {
            tracing::debug!(error = %err, "search event dropped");
        }
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn gen_profile(
    core: Ctx<'_>,
    session_id: String,
    namespace: String,
    name: String,
) -> Res<GenProfile> {
    core.gen_profile(&session_id, &namespace, &name).await
}

#[tauri::command]
#[specta::specta]
pub async fn gen_preview(core: Ctx<'_>, request: FakeRequest) -> Res<GenPreview> {
    core.gen_preview(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn gen_run(
    core: Ctx<'_>,
    request: FakeRequest,
    events: Channel<Progress>,
) -> Res<GenDone> {
    core.gen_run(request, relay(events)).await
}
