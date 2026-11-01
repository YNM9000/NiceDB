use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minisign::{SecretKey, SecretKeyBox};
use serde_json::{Value, json};

pub struct Artefact {
    pub file: &'static str,
    pub platforms: &'static [&'static str],
    pub served: bool,
}

pub const ARTEFACTS: &[Artefact] = &[
    Artefact {
        file: "NiceDB-linux-x86_64",
        platforms: &["linux-x86_64"],
        served: true,
    },
    Artefact {
        file: "NiceDB-linux-x86_64.AppImage",
        platforms: &["linux-x86_64-appimage"],
        served: true,
    },
    Artefact {
        file: "NiceDB-linux-x86_64.deb",
        platforms: &["linux-x86_64-deb"],
        served: true,
    },
    Artefact {
        file: "NiceDB-linux-x86_64.tar.gz",
        platforms: &["linux-x86_64"],
        served: false,
    },
    Artefact {
        file: "NiceDB-windows-x86_64.exe",
        platforms: &["windows-x86_64"],
        served: true,
    },
    Artefact {
        file: "NiceDB-windows-x86_64.zip",
        platforms: &["windows-x86_64"],
        served: false,
    },
    Artefact {
        file: "NiceDB-windows-x86_64-setup.exe",
        platforms: &["windows-x86_64-nsis"],
        served: false,
    },
    Artefact {
        file: "NiceDB-macos-universal.zip",
        platforms: &["darwin-aarch64", "darwin-x86_64"],
        served: true,
    },
];

pub fn served_file(platform: &str) -> Option<&'static str> {
    ARTEFACTS
        .iter()
        .find(|a| a.served && a.platforms.contains(&platform))
        .map(|a| a.file)
}

pub fn secret_key(key_file: &str, password: &str) -> Result<SecretKey, String> {
    let text = STANDARD
        .decode(key_file.trim())
        .map_err(|_| "key: not base64".to_owned())?;
    let text = String::from_utf8(text).map_err(|_| "key: not utf-8".to_owned())?;
    let boxed = SecretKeyBox::from_string(&text).map_err(|e| format!("key: {e}"))?;
    SecretKey::from_box(boxed, Some(password.to_owned())).map_err(|e| format!("key: {e}"))
}

pub fn trusted_comment(timestamp: u64, file: &str, version: &str, platforms: &[&str]) -> String {
    format!(
        "timestamp:{timestamp}\tfile:{file}\tversion:{version}\tplatform:{}",
        platforms.join(",")
    )
}

pub fn sign(sk: &SecretKey, data: &[u8], comment: &str) -> Result<String, String> {
    let sig = minisign::sign(
        None,
        sk,
        data,
        Some(comment),
        Some("signature from tauri secret key"),
    )
    .map_err(|e| format!("sign: {e}"))?;
    Ok(STANDARD.encode(sig.to_string()))
}

pub fn asset_base(endpoint: &str, version: &str) -> Result<String, String> {
    if let Some(rest) = endpoint.strip_prefix("https://github.com/")
        && let Some(repo) = rest.strip_suffix("/releases/latest/download/latest.json")
        && repo.split('/').count() == 2
    {
        return Ok(format!(
            "https://github.com/{repo}/releases/download/v{version}"
        ));
    }
    match endpoint.rsplit_once('/') {
        Some((dir, "latest.json"))
            if endpoint.starts_with("https://") || endpoint.starts_with("http://127.0.0.1") =>
        {
            Ok(format!("{dir}/v{version}"))
        }
        _ => Err(format!(
            "the manifest URL must end in latest.json: {endpoint}"
        )),
    }
}

pub fn manifest(
    version: &str,
    notes: (&str, &str),
    pub_date: &str,
    base: &str,
    signed: &[(&str, String)],
) -> Value {
    let mut platforms = serde_json::Map::new();
    for (file, signature) in signed {
        for a in ARTEFACTS.iter().filter(|a| a.served && a.file == *file) {
            for p in a.platforms {
                platforms.insert(
                    (*p).to_owned(),
                    json!({ "signature": signature, "url": format!("{base}/{file}") }),
                );
            }
        }
    }
    json!({
        "version": version,
        "notes": notes.1,
        "notes_i18n": { "it": notes.0, "en": notes.1 },
        "pub_date": pub_date,
        "platforms": platforms,
    })
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn rfc3339(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}
