use std::path::PathBuf;
use std::sync::Arc;

use nd_core::{Core, ExportFormat};
use nd_store::{
    ExportSelection, Folder, ImportChoice, ImportOutcome, ImportPreview, TreeNode, Workspace,
    WorkspaceSession, Workspaces,
};
use tauri::State;

use crate::error::CommandError;

type Ctx<'a> = State<'a, Arc<Core>>;
type Res<T> = Result<T, CommandError>;

#[tauri::command]
#[specta::specta]
pub fn folders_list(core: Ctx<'_>) -> Res<Vec<Folder>> {
    core.store(nd_store::Store::folders)
}

#[tauri::command]
#[specta::specta]
pub fn folder_save(core: Ctx<'_>, folder: Folder) -> Res<()> {
    core.store(|s| s.save_folder(&folder))
}

#[tauri::command]
#[specta::specta]
pub fn folder_delete(core: Ctx<'_>, id: String) -> Res<()> {
    core.store(|s| s.delete_folder(&id)).map(|_| ())
}

#[tauri::command]
#[specta::specta]
pub fn node_move(core: Ctx<'_>, node: TreeNode, parent: Option<String>, index: u32) -> Res<()> {
    core.store(|s| s.move_node(&node, parent.as_deref(), index as usize))
}

#[tauri::command]
#[specta::specta]
pub fn workspaces_get(core: Ctx<'_>) -> Res<Workspaces> {
    core.store(nd_store::Store::workspaces)
}

#[tauri::command]
#[specta::specta]
pub fn workspace_save(core: Ctx<'_>, workspace: Workspace) -> Res<()> {
    if let Some(name) = workspace
        .vars
        .keys()
        .find(|n| !nd_store::valid_name(n) || nd_store::secret_like(n))
    {
        let field = "workspace".to_owned();
        let name = name.clone();
        return Err(nd_core::CoreError::Variable(
            if nd_store::valid_name(&name) {
                nd_store::VarError::Secret { name, field }
            } else {
                nd_store::VarError::Syntax { field }
            },
        ));
    }
    core.store(|s| s.save_workspace(&workspace))
}

#[tauri::command]
#[specta::specta]
pub fn workspace_delete(core: Ctx<'_>, id: String) -> Res<()> {
    core.store(|s| s.delete_workspace(&id)).map(|_| ())
}

#[tauri::command]
#[specta::specta]
pub fn workspace_session_save(
    core: Ctx<'_>,
    id: Option<String>,
    session: WorkspaceSession,
) -> Res<()> {
    core.store(|s| s.save_session(id.as_deref(), &session))
}

#[tauri::command]
#[specta::specta]
pub fn workspace_activate(core: Ctx<'_>, id: Option<String>) -> Res<()> {
    core.store(|s| s.set_active_workspace(id.as_deref()))
}

#[tauri::command]
#[specta::specta]
pub async fn package_export(
    core: Ctx<'_>,
    path: String,
    selection: ExportSelection,
    format: ExportFormat,
) -> Res<()> {
    core.export_package(PathBuf::from(path), selection, format)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn package_preview(
    core: Ctx<'_>,
    path: String,
    passphrase: Option<String>,
) -> Res<ImportPreview> {
    core.import_preview(PathBuf::from(path), passphrase).await
}

#[tauri::command]
#[specta::specta]
pub async fn package_import(
    core: Ctx<'_>,
    path: String,
    passphrase: Option<String>,
    choices: Vec<ImportChoice>,
) -> Res<ImportOutcome> {
    core.import_apply(PathBuf::from(path), passphrase, choices)
        .await
}
