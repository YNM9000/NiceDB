use std::collections::BTreeMap;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};

const PREFS_FILE: &str = "updater.json";
const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const MAX_MANIFEST: u64 = 1 << 20;
const MAX_ARTIFACT: u64 = 1 << 30;
pub const URL_ENV: &str = "NICEDB_UPDATE_URL";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum UpdateFault {
    Network(String),
    Invalid(String),
    Signature,
    NoUpdate,
    NotDownloaded,
    Install(String),
    Restart(String),
    Link(String),
}

impl std::fmt::Display for UpdateFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type Res<T> = Result<T, UpdateFault>;

#[derive(Deserialize)]
struct ConfFile {
    plugins: ConfPlugins,
}
#[derive(Deserialize)]
struct ConfPlugins {
    updater: ConfUpdater,
}
#[derive(Deserialize)]
struct ConfUpdater {
    pubkey: String,
    endpoints: Vec<String>,
}

pub fn embedded_config() -> Res<(String, String)> {
    let c: ConfFile = serde_json::from_str(include_str!("../tauri.conf.json"))
        .map_err(|e| UpdateFault::Invalid(format!("tauri.conf.json: {e}")))?;
    let url = c
        .plugins
        .updater
        .endpoints
        .into_iter()
        .next()
        .unwrap_or_default();
    Ok((url, c.plugins.updater.pubkey))
}

pub fn platform_key() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        ("windows", "x86_64") => "windows-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("macos", "aarch64") => "darwin-aarch64",
        ("macos", "x86_64") => "darwin-x86_64",
        _ => "unsupported",
    }
}

fn appimage_path() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file())
}

pub fn app_exe() -> Option<PathBuf> {
    appimage_path().or_else(|| std::env::current_exe().ok())
}

pub fn running_platform(exe: &Path) -> String {
    if appimage_path().is_some() {
        return format!("{}-appimage", platform_key());
    }
    if cfg!(target_os = "linux") && exe.starts_with("/usr") {
        return format!("{}-deb", platform_key());
    }
    platform_key().to_owned()
}

pub fn artefact_for(platform: &str) -> String {
    if let Some(base) = platform.strip_suffix("-appimage") {
        format!("NiceDB-{base}.AppImage")
    } else if let Some(base) = platform.strip_suffix("-deb") {
        format!("NiceDB-{base}.deb")
    } else if platform.starts_with("darwin-") {
        "NiceDB-macos-universal.zip".to_owned()
    } else if platform.starts_with("windows-") {
        format!("NiceDB-{platform}.exe")
    } else {
        format!("NiceDB-{platform}")
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
pub struct Notes {
    #[serde(default)]
    pub it: String,
    #[serde(default)]
    pub en: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct PlatformAsset {
    pub signature: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub notes_i18n: Option<Notes>,
    #[serde(default)]
    pub pub_date: Option<String>,
    pub platforms: BTreeMap<String, PlatformAsset>,
}

impl Manifest {
    pub fn notes(&self) -> Notes {
        let mut n = self.notes_i18n.clone().unwrap_or_default();
        if n.en.is_empty() {
            n.en.clone_from(&self.notes);
        }
        if n.it.is_empty() {
            n.it.clone_from(&n.en);
        }
        n
    }
}

fn invalid(m: impl Into<String>) -> UpdateFault {
    UpdateFault::Invalid(m.into())
}

pub fn parse_manifest(bytes: &[u8]) -> Res<Manifest> {
    let m: Manifest =
        serde_json::from_slice(bytes).map_err(|e| invalid(format!("latest.json: {e}")))?;
    semver::Version::parse(&m.version).map_err(|_| invalid(format!("version: {}", m.version)))?;
    Ok(m)
}

pub fn is_newer(current: &str, candidate: &str) -> Res<bool> {
    let cur =
        semver::Version::parse(current).map_err(|_| invalid(format!("version: {current}")))?;
    let new =
        semver::Version::parse(candidate).map_err(|_| invalid(format!("version: {candidate}")))?;
    Ok(new > cur)
}

fn b64_text(s: &str) -> Option<String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .ok()?;
    String::from_utf8(raw).ok()
}

pub fn verify_signature(pubkey_b64: &str, data: &[u8], signature_b64: &str) -> Res<String> {
    let pk = b64_text(pubkey_b64)
        .and_then(|t| PublicKey::decode(&t).ok())
        .ok_or_else(|| invalid("public key"))?;
    let sig = b64_text(signature_b64)
        .and_then(|t| Signature::decode(&t).ok())
        .ok_or(UpdateFault::Signature)?;
    pk.verify(data, &sig, false)
        .map_err(|_| UpdateFault::Signature)?;
    Ok(sig.trusted_comment().to_owned())
}

pub const SECLAB_FILE: &str = "seclab-versions.json";
const MAX_SECLAB: u64 = 2 << 20;

pub fn verify_seclab(pubkey_b64: &str, json: &[u8], signature_b64: &str) -> Res<()> {
    let trusted = verify_signature(pubkey_b64, json, signature_b64)?;
    if signed_field(&trusted, "file:") == Some(SECLAB_FILE) {
        Ok(())
    } else {
        Err(UpdateFault::Signature)
    }
}

fn signed_field<'a>(trusted: &'a str, key: &str) -> Option<&'a str> {
    trusted.split('\t').find_map(|f| f.strip_prefix(key))
}

pub fn check_binding(trusted: &str, version: &str, current: &str, platform: &str) -> Res<()> {
    let signed = signed_field(trusted, "version:").ok_or(UpdateFault::Signature)?;
    let platform_ok =
        signed_field(trusted, "platform:").is_some_and(|p| p.split(',').any(|x| x == platform));
    let file_ok = signed_field(trusted, "file:") == Some(artefact_for(platform).as_str());
    if signed != version || !platform_ok || !file_ok || !is_newer(current, signed)? {
        return Err(UpdateFault::Signature);
    }
    Ok(())
}

fn origin(url: &str) -> Option<(String, String)> {
    let u: ureq::http::Uri = url.parse().ok()?;
    Some((
        u.scheme_str()?.to_ascii_lowercase(),
        u.authority()?.as_str().to_ascii_lowercase(),
    ))
}

fn is_loopback(host_port: &str) -> bool {
    let host = host_port.rsplit_once(':').map_or(host_port, |(h, _)| h);
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
}

pub fn check_manifest_url(url: &str) -> Res<()> {
    match origin(url) {
        Some((s, _)) if s == "https" => Ok(()),
        Some((s, a)) if s == "http" && is_loopback(&a) => Ok(()),
        _ => Err(invalid(format!("address not allowed: {url}"))),
    }
}

pub fn check_asset_url(manifest_url: &str, asset_url: &str) -> Res<()> {
    match (origin(manifest_url), origin(asset_url)) {
        (Some(a), Some(b)) if a == b => Ok(()),
        _ => Err(invalid(format!("file address not allowed: {asset_url}"))),
    }
}

fn network(e: impl std::fmt::Display) -> UpdateFault {
    UpdateFault::Network(e.to_string())
}

fn fetch(
    url: &str,
    max: u64,
    body_timeout: Duration,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Res<Vec<u8>> {
    let https = origin(url).is_some_and(|(s, _)| s == "https");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(https)
        .max_redirects(5)
        .user_agent(concat!("NiceDB/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_recv_body(Some(body_timeout))
        .build()
        .into();
    let mut resp = agent.get(url).call().map_err(network)?;
    let total = resp.body().content_length();
    if total.is_some_and(|t| t > max) {
        return Err(invalid("file too large"));
    }
    let mut reader = resp.body_mut().with_config().limit(max + 1).reader();
    let mut out = Vec::with_capacity(usize::try_from(total.unwrap_or(0).min(max)).unwrap_or(0));
    let mut buf = vec![0u8; 64 * 1024];
    let mut last = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(network)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        let done = out.len() as u64;
        if done > max {
            return Err(invalid("file too large"));
        }
        if done - last >= 256 * 1024 {
            last = done;
            progress(done, total);
        }
    }
    progress(out.len() as u64, total);
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    File(PathBuf),
    Bundle(PathBuf),
}

pub fn target_for(exe: &Path, zip: bool) -> Res<Target> {
    if !zip {
        return Ok(Target::File(exe.to_path_buf()));
    }
    let bundle = exe.parent().and_then(Path::parent).and_then(Path::parent);
    match bundle {
        Some(b)
            if b.extension().is_some_and(|e| e == "app")
                && exe.parent().is_some_and(|p| p.ends_with("Contents/MacOS")) =>
        {
            Ok(Target::Bundle(b.to_path_buf()))
        }
        _ => Err(invalid("the app is not inside a .app bundle")),
    }
}

impl Target {
    pub fn path(&self) -> &Path {
        match self {
            Target::File(p) | Target::Bundle(p) => p,
        }
    }
    fn writable(&self) -> bool {
        self.path().parent().is_some_and(dir_writable)
    }
}

fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".nicedb-write-{}", std::process::id()));
    let ok = std::fs::File::create(&probe).is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

pub fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

fn remove_any(p: &Path) {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => {
            let _ = std::fs::remove_dir_all(p);
        }
        Ok(_) => {
            let _ = std::fs::remove_file(p);
        }
        Err(_) => {}
    }
}

fn sync_dir(dir: &Path) {
    if cfg!(unix)
        && let Ok(d) = std::fs::File::open(dir)
    {
        let _ = d.sync_all();
    }
}

pub fn swap_in(target: &Path, staged: &Path) -> std::io::Result<()> {
    let old = sibling(target, ".old");
    remove_any(&old);
    if cfg!(unix)
        && std::fs::symlink_metadata(target).is_ok_and(|m| m.is_file())
        && std::fs::hard_link(target, &old).is_ok()
    {
        let r = std::fs::rename(staged, target);
        if r.is_err() {
            let _ = std::fs::remove_file(&old);
        }
        if let Some(d) = target.parent() {
            sync_dir(d);
        }
        return r;
    }
    std::fs::rename(target, &old)?;
    if let Err(e) = std::fs::rename(staged, target) {
        let _ = std::fs::rename(&old, target);
        return Err(e);
    }
    if let Some(d) = target.parent() {
        sync_dir(d);
    }
    Ok(())
}

pub fn replace_file(exe: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let staged = sibling(exe, ".new");
    remove_any(&staged);
    let res = (|| {
        let mut f = std::fs::File::create(&staged)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        executable(&staged, exe)?;
        swap_in(exe, &staged)
    })();
    if res.is_err() {
        remove_any(&staged);
    }
    res
}

#[cfg(unix)]
fn executable(path: &Path, like: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mode = std::fs::metadata(like).map_or(0o755, |m| m.permissions().mode()) | 0o755;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode & 0o7777))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn executable(_path: &Path, _like: &Path) -> std::io::Result<()> {
    Ok(())
}

pub fn extract_bundle(zip_bytes: &[u8], dest: &Path) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    let bad = |m: &str| Error::new(ErrorKind::InvalidData, m.to_owned());
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| bad(&e.to_string()))?;
    std::fs::create_dir_all(dest)?;
    let mut seen_app = false;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| bad(&e.to_string()))?;
        let name = entry
            .enclosed_name()
            .ok_or_else(|| bad("unsafe path in zip"))?;
        let mut parts = name.components();
        let top = parts.next().ok_or_else(|| bad("empty path in zip"))?;
        if Path::new(top.as_os_str())
            .extension()
            .is_none_or(|e| e != "app")
        {
            return Err(bad("zip does not contain an .app"));
        }
        seen_app = true;
        let rel: PathBuf = parts.collect();
        let mut at = dest.to_path_buf();
        for c in rel.parent().into_iter().flat_map(Path::components) {
            at.push(c);
            if std::fs::symlink_metadata(&at).is_ok_and(|m| !m.is_dir()) {
                return Err(bad("zip writes through a symlink"));
            }
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        if entry.is_symlink() {
            let mut link = String::new();
            entry.by_ref().take(4096).read_to_string(&mut link)?;
            let target = Path::new(&link);
            if link.is_empty()
                || !target
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_)))
            {
                return Err(bad("symlink leaves its folder"));
            }
            make_symlink(target, &out)?;
            continue;
        }
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&out)?;
        std::io::copy(&mut entry, &mut f)?;
        f.sync_all()?;
        set_mode(&out, entry.unix_mode())?;
    }
    if !seen_app {
        return Err(bad("empty zip"));
    }
    Ok(())
}

#[cfg(unix)]
fn make_symlink(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn make_symlink(_target: &Path, _at: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("symlinks in a bundle need macOS"))
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: Option<u32>) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    match mode {
        Some(m) => std::fs::set_permissions(path, std::fs::Permissions::from_mode(m & 0o7777)),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn set_mode(_path: &Path, _mode: Option<u32>) -> std::io::Result<()> {
    Ok(())
}

pub fn replace_bundle(bundle: &Path, zip_bytes: &[u8]) -> std::io::Result<()> {
    let staged = sibling(bundle, ".new");
    remove_any(&staged);
    let res = extract_bundle(zip_bytes, &staged).and_then(|()| swap_in(bundle, &staged));
    if res.is_err() {
        remove_any(&staged);
    }
    res
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Prefs {
    check_on_start: bool,
    last_check: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, specta::Type)]
pub struct UpdatePrefs {
    pub check_on_start: bool,
    #[specta(type = Option<f64>)]
    pub last_check: Option<u64>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UpdateStatus {
    Skipped,
    UpToDate {
        current: String,
    },
    Available {
        current: String,
        version: String,
        notes: Notes,
        pub_date: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, specta::Type)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum InstallOutcome {
    Replaced { path: String },
    Downloaded { path: String },
}

#[derive(Clone, Copy, Debug, Serialize, specta::Type)]
pub struct UpdateProgress {
    #[specta(type = f64)]
    pub downloaded: u64,
    #[specta(type = Option<f64>)]
    pub total: Option<u64>,
}

pub struct UpdaterConfig {
    pub manifest_url: String,
    pub pubkey: String,
    pub current_version: String,
    pub platform: String,
    pub exe: PathBuf,
    pub prefs_dir: PathBuf,
    pub downloads_dir: Option<PathBuf>,
    pub in_place: bool,
}

struct Staged {
    version: String,
    file_name: String,
    bytes: Vec<u8>,
}

pub struct Updater {
    cfg: UpdaterConfig,
    prefs: Mutex<Prefs>,
    available: Mutex<Option<(String, PlatformAsset)>>,
    staged: Mutex<Option<Staged>>,
    installed: Mutex<bool>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Updater {
    pub fn for_app(prefs_dir: PathBuf) -> Updater {
        let (url, pubkey) = embedded_config().unwrap_or_default();
        let exe = app_exe().unwrap_or_default();
        Updater::new(UpdaterConfig {
            manifest_url: std::env::var(URL_ENV)
                .ok()
                .filter(|_| cfg!(debug_assertions))
                .unwrap_or(url),
            pubkey,
            current_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: running_platform(&exe),
            exe,
            prefs_dir,
            downloads_dir: dirs::download_dir(),
            in_place: !cfg!(debug_assertions),
        })
    }

    pub fn seclab_list(&self) -> Res<(String, String)> {
        check_manifest_url(&self.cfg.manifest_url)?;
        let base = self
            .cfg
            .manifest_url
            .rsplit_once('/')
            .map(|(b, _)| b)
            .ok_or_else(|| invalid("manifest address"))?;
        let url = format!("{base}/{SECLAB_FILE}");
        let quiet = &mut |_: u64, _: Option<u64>| {};
        let json = fetch(&url, MAX_SECLAB, Duration::from_secs(30), quiet)?;
        let sig = fetch(&format!("{url}.sig"), 4096, Duration::from_secs(30), quiet)?;
        let sig = String::from_utf8(sig).map_err(|_| UpdateFault::Signature)?;
        verify_seclab(&self.cfg.pubkey, &json, sig.trim())?;
        let json = String::from_utf8(json).map_err(|_| invalid(SECLAB_FILE))?;
        Ok((json, sig.trim().to_owned()))
    }

    pub fn new(cfg: UpdaterConfig) -> Updater {
        let prefs = std::fs::read(cfg.prefs_dir.join(PREFS_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(Prefs {
                check_on_start: true,
                last_check: None,
            });
        Updater {
            cfg,
            prefs: Mutex::new(prefs),
            available: Mutex::new(None),
            staged: Mutex::new(None),
            installed: Mutex::new(false),
        }
    }

    pub fn manifest_url(&self) -> &str {
        &self.cfg.manifest_url
    }

    fn save_prefs(&self, p: &Prefs) {
        if let Ok(json) = serde_json::to_vec_pretty(p)
            && let Err(e) = std::fs::write(self.cfg.prefs_dir.join(PREFS_FILE), json)
        {
            tracing::warn!(error = %e, "update preferences not saved");
        }
    }

    pub fn prefs(&self) -> UpdatePrefs {
        let p = lock(&self.prefs);
        UpdatePrefs {
            check_on_start: p.check_on_start,
            last_check: p.last_check,
        }
    }

    pub fn set_check_on_start(&self, on: bool) -> UpdatePrefs {
        {
            let mut p = lock(&self.prefs);
            p.check_on_start = on;
            self.save_prefs(&p);
        }
        self.prefs()
    }

    pub fn check(&self, auto: bool, now_ms: u64) -> Res<UpdateStatus> {
        if auto {
            let p = lock(&self.prefs).clone();
            if !p.check_on_start
                || p.last_check
                    .is_some_and(|t| now_ms.saturating_sub(t) < DAY_MS)
            {
                return Ok(UpdateStatus::Skipped);
            }
        }
        check_manifest_url(&self.cfg.manifest_url)?;
        let bytes = fetch(
            &self.cfg.manifest_url,
            MAX_MANIFEST,
            Duration::from_secs(30),
            &mut |_, _| {},
        )?;
        let m = parse_manifest(&bytes)?;
        {
            let mut p = lock(&self.prefs);
            p.last_check = Some(now_ms);
            self.save_prefs(&p);
        }
        let current = self.cfg.current_version.clone();
        match m.platforms.get(&self.cfg.platform).cloned() {
            Some(a) if is_newer(&current, &m.version)? => {
                check_asset_url(&self.cfg.manifest_url, &a.url)?;
                *lock(&self.available) = Some((m.version.clone(), a));
                Ok(UpdateStatus::Available {
                    current,
                    notes: m.notes(),
                    version: m.version,
                    pub_date: m.pub_date,
                })
            }
            _ => {
                *lock(&self.available) = None;
                Ok(UpdateStatus::UpToDate { current })
            }
        }
    }

    pub fn download(&self, progress: &mut dyn FnMut(u64, Option<u64>)) -> Res<()> {
        let (version, asset) = lock(&self.available).clone().ok_or(UpdateFault::NoUpdate)?;
        let bytes = fetch(&asset.url, MAX_ARTIFACT, Duration::from_mins(30), progress)?;
        let trusted = verify_signature(&self.cfg.pubkey, &bytes, &asset.signature)?;
        check_binding(
            &trusted,
            &version,
            &self.cfg.current_version,
            &self.cfg.platform,
        )?;
        let file_name = artefact_for(&self.cfg.platform);
        *lock(&self.staged) = Some(Staged {
            version,
            file_name,
            bytes,
        });
        Ok(())
    }

    pub fn install(&self) -> Res<InstallOutcome> {
        let staged = lock(&self.staged)
            .take()
            .ok_or(UpdateFault::NotDownloaded)?;
        let ext = Path::new(&staged.file_name).extension();
        let package = ext.is_some_and(|e| e.eq_ignore_ascii_case("deb"));
        let target = target_for(
            &self.cfg.exe,
            ext.is_some_and(|e| e.eq_ignore_ascii_case("zip")),
        );
        if self.cfg.in_place
            && !package
            && let Ok(t) = &target
            && t.writable()
        {
            let r = match t {
                Target::File(p) => replace_file(p, &staged.bytes),
                Target::Bundle(p) => replace_bundle(p, &staged.bytes),
            };
            return match r {
                Ok(()) => {
                    *lock(&self.installed) = true;
                    Ok(InstallOutcome::Replaced {
                        path: t.path().to_string_lossy().into_owned(),
                    })
                }
                Err(e) => {
                    *lock(&self.staged) = Some(staged);
                    Err(UpdateFault::Install(e.to_string()))
                }
            };
        }
        self.save_to_downloads(&staged)
    }

    fn save_to_downloads(&self, s: &Staged) -> Res<InstallOutcome> {
        let dir = self
            .cfg
            .downloads_dir
            .clone()
            .ok_or_else(|| UpdateFault::Install("no Downloads folder".into()))?;
        let io = |e: std::io::Error| UpdateFault::Install(e.to_string());
        std::fs::create_dir_all(&dir).map_err(io)?;
        let name = match s.file_name.rsplit_once('.') {
            Some((stem, ext)) if !stem.is_empty() => format!("{stem}-{}.{ext}", s.version),
            _ => format!("{}-{}", s.file_name, s.version),
        };
        let dest = dir.join(name);
        let tmp = sibling(&dest, ".part");
        std::fs::write(&tmp, &s.bytes)
            .and_then(|()| std::fs::rename(&tmp, &dest))
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                io(e)
            })?;
        if !dest.extension().is_some_and(|e| e == "zip" || e == "deb") {
            executable(&dest, &dest).map_err(io)?;
        }
        Ok(InstallOutcome::Downloaded {
            path: dest.to_string_lossy().into_owned(),
        })
    }

    pub fn spawn_updated(&self) -> Res<()> {
        if !*lock(&self.installed) {
            return Err(UpdateFault::NotDownloaded);
        }
        std::process::Command::new(&self.cfg.exe)
            .spawn()
            .map(|_| ())
            .map_err(|e| UpdateFault::Restart(e.to_string()))
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
