use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt as _;

use crate::updater::{
    InstallOutcome, UpdateFault, UpdatePrefs, UpdateProgress, UpdateStatus, Updater, now_ms,
};

type St<'a> = State<'a, Arc<Updater>>;
type Res<T> = Result<T, UpdateFault>;

pub const LINKS: &[&str] = &[
    "https://github.com/maxib0n",
    "https://github.com/maxib0n/NiceDB",
    "https://www.gnu.org/licenses/gpl-3.0.html",
];

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct AppInfo {
    pub version: String,
    pub commit: String,
    pub built: String,
    pub platform: String,
    pub update_url: String,
}

#[tauri::command]
#[specta::specta]
pub fn app_info(st: St<'_>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        commit: env!("ND_COMMIT").to_owned(),
        built: env!("ND_BUILT").to_owned(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        update_url: st.manifest_url().to_owned(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn open_link(app: AppHandle, url: String) -> Res<()> {
    if !LINKS.contains(&url.as_str()) {
        return Err(UpdateFault::Link(format!("not allowed: {url}")));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| UpdateFault::Link(e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn seclab_versions_update(
    st: St<'_>,
    core: State<'_, Arc<nd_core::Core>>,
) -> Res<String> {
    let (json, sig) = blocking(&st, Updater::seclab_list).await?;
    core.install_seclab_versions(&json, &sig, true)
        .map_err(|e| UpdateFault::Invalid(e.to_string()))
}

async fn blocking<T: Send + 'static>(
    st: &St<'_>,
    f: impl FnOnce(&Updater) -> Res<T> + Send + 'static,
) -> Res<T> {
    let u = st.inner().clone();
    tauri::async_runtime::spawn_blocking(move || f(&u))
        .await
        .map_err(|e| UpdateFault::Network(e.to_string()))?
}

#[tauri::command]
#[specta::specta]
pub async fn update_check(st: St<'_>, auto: bool) -> Res<UpdateStatus> {
    if auto && cfg!(debug_assertions) && std::env::var_os(crate::updater::URL_ENV).is_none() {
        return Ok(UpdateStatus::Skipped);
    }
    blocking(&st, move |u| u.check(auto, now_ms())).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_download(st: St<'_>, progress: Channel<UpdateProgress>) -> Res<()> {
    blocking(&st, move |u| {
        u.download(&mut |downloaded, total| {
            let _ = progress.send(UpdateProgress { downloaded, total });
        })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_install(st: St<'_>) -> Res<InstallOutcome> {
    blocking(&st, Updater::install).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_restart(app: AppHandle, st: St<'_>) -> Res<()> {
    blocking(&st, Updater::spawn_updated).await?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn update_prefs_get(st: St<'_>) -> UpdatePrefs {
    st.prefs()
}

#[tauri::command]
#[specta::specta]
pub fn update_prefs_set(st: St<'_>, check_on_start: bool) -> UpdatePrefs {
    st.set_check_on_start(check_on_start)
}
