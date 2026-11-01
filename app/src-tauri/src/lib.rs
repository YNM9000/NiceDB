mod about;
mod commands;
mod error;
mod library;
mod org;
mod tools;
mod updater;

use std::sync::Arc;
use std::time::Duration;

use nd_core::Core;
use nd_net::KnownHosts;
use nd_store::Store;
use tauri::Manager;

pub use commands::builder;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let builder = builder();
    tauri::Builder::default()
        .plugin(navigation_guard())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
                && let Err(e) = webview.window().show()
            {
                tracing::warn!(error = %e, "main window not shown");
            }
        })
        .setup(move |app| {
            builder.mount_events(app);
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            keep_private(&dir)?;
            app.manage(Arc::new(updater::Updater::for_app(dir.clone())));
            let store = Store::open(&dir.join("nicedb.sqlite"))?;
            match (store.appearance(), app.get_webview_window("main")) {
                (Ok(a), Some(window)) => commands::apply_scale(&window, a.ui_scale),
                (Err(e), _) => tracing::warn!(error = %e, "appearance not read"),
                (Ok(_), None) => tracing::warn!("main window missing, interface size not applied"),
            }
            let mut files = Vec::new();
            if let Ok(home) = app.path().home_dir() {
                files.push(home.join(".ssh/known_hosts"));
            }
            files.push(dir.join("known_hosts"));
            let core = Arc::new(Core::new(
                store,
                Arc::new(nd_secrets::Keychain),
                KnownHosts { files },
            ));
            if let Some((json, sig)) = core.saved_seclab_versions() {
                let pubkey = updater::embedded_config().map(|c| c.1).unwrap_or_default();
                let kept = updater::verify_seclab(&pubkey, json.as_bytes(), &sig)
                    .map_err(|e| e.to_string())
                    .and_then(|()| {
                        core.install_seclab_versions(&json, &sig, false)
                            .map_err(|e| e.to_string())
                    });
                if let Err(e) = kept {
                    tracing::warn!(error = %e, "downloaded versions list not used");
                }
            }
            app.manage(core.clone());
            app.manage(commands::SessionWatch::default());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let events = core.upkeep().await;
                    if let Some(watch) = handle.try_state::<commands::SessionWatch>() {
                        for e in events {
                            watch.send(e);
                        }
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("tauri runtime failed to start")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit)
                && let Some(core) = app.try_state::<Arc<Core>>()
            {
                core.purge_all_trashed();
            }
        });
}

fn own_origin(url: &tauri::Url) -> bool {
    match (url.scheme(), url.host_str()) {
        ("tauri", Some("localhost")) | ("http" | "https", Some("tauri.localhost")) => true,
        ("http", Some("localhost" | "127.0.0.1")) => cfg!(debug_assertions),
        ("about", _) => url.as_str() == "about:blank",
        _ => false,
    }
}

fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_, url| {
            let ok = own_origin(url);
            if !ok {
                tracing::warn!(%url, "navigation refused");
            }
            ok
        })
        .build()
}

#[cfg(unix)]
fn keep_private(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    for name in ["updater.json", "known_hosts"] {
        let file = dir.join(name);
        if file.exists() {
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn keep_private(_dir: &std::path::Path) -> std::io::Result<()> {
    Ok(())
}
