use std::io::Write as _;
use std::path::{Path, PathBuf};

const PREFIX: &str = "nicedb-private-";

pub struct PrivateDir(PathBuf);

impl PrivateDir {
    pub fn new() -> std::io::Result<Self> {
        static SWEEP: std::sync::Once = std::sync::Once::new();
        SWEEP.call_once(sweep);
        let base = std::env::temp_dir();
        let pid = std::process::id();
        for attempt in 0..16u32 {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let path = base.join(format!("{PREFIX}{pid}-{nanos:x}-{attempt}"));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no free private folder",
        ))
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, name: &str, parts: &[&[u8]]) -> std::io::Result<PathBuf> {
        let path = self.0.join(name);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&path)?;
        for p in parts {
            file.write_all(p)?;
        }
        Ok(path)
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sweep() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let pid = name
            .strip_prefix(PREFIX)
            .or_else(|| name.strip_prefix("nicedb-mongo-"))
            .and_then(|rest| rest.split('-').next())
            .and_then(|p| p.parse::<u32>().ok());
        let Some(pid) = pid else { continue };
        if pid == std::process::id() || !gone(pid) {
            continue;
        }
        let path = entry.path();
        let _ = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
    }
}

#[cfg(target_os = "linux")]
fn gone(pid: u32) -> bool {
    !Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
fn gone(_pid: u32) -> bool {
    false
}
