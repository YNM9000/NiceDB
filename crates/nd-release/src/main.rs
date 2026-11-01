use std::path::Path;
use std::process::ExitCode;

use nd_release::{
    ARTEFACTS, asset_base, manifest, rfc3339, secret_key, sign, trusted_comment, unix_now,
};
use serde_json::Value;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nd-release: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dist, conf, key] = &args[..] else {
        return Err("usage: nd-release <dist> <tauri.conf.json> <secret.key>".into());
    };
    let dist = Path::new(dist);
    let conf: Value = serde_json::from_str(&read(Path::new(conf))?)
        .map_err(|e| format!("tauri.conf.json: {e}"))?;
    let version = conf["version"]
        .as_str()
        .ok_or("tauri.conf.json: no version")?;
    let endpoint = match std::env::var("NICEDB_UPDATE_URL") {
        Ok(url) => url,
        Err(_) => conf["plugins"]["updater"]["endpoints"][0]
            .as_str()
            .ok_or("tauri.conf.json: no updater endpoint")?
            .to_owned(),
    };
    let password = std::env::var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD").unwrap_or_default();
    let sk = secret_key(&read(Path::new(key))?, &password)?;
    let notes = match std::env::var("ND_RELEASE_NOTES") {
        Ok(path) => serde_json::from_str::<Value>(&read(Path::new(&path))?)
            .map_err(|e| format!("{path}: {e}"))?,
        Err(_) => Value::Null,
    };
    let note = |lang: &str| notes[lang].as_str().unwrap_or_default().to_owned();

    let out = dist.join("release");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let now = unix_now();
    let mut signed = Vec::new();
    for a in ARTEFACTS {
        let path = dist.join(a.file);
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let signature = sign(
            &sk,
            &data,
            &trusted_comment(now, a.file, version, a.platforms),
        )?;
        std::fs::copy(&path, out.join(a.file)).map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::write(
            out.join(format!("{}.sig", a.file)),
            format!("{signature}\n"),
        )
        .map_err(|e| e.to_string())?;
        println!("signed {} ({})", a.file, a.platforms.join(", "));
        signed.push((a.file, signature));
    }
    if signed.is_empty() {
        return Err(format!("nothing to sign in {}", dist.display()));
    }
    let list = std::env::var("ND_SECLAB_VERSIONS")
        .unwrap_or_else(|_| "crates/nd-core/src/tools/seclab/versions.json".to_owned());
    if let Ok(data) = std::fs::read(&list) {
        let file = "seclab-versions.json";
        let signature = sign(&sk, &data, &trusted_comment(now, file, version, &[]))?;
        std::fs::write(out.join(file), &data).map_err(|e| e.to_string())?;
        std::fs::write(out.join(format!("{file}.sig")), format!("{signature}\n"))
            .map_err(|e| e.to_string())?;
        println!("signed {file} from {list}");
    }
    if let Ok(sums) = std::fs::read(dist.join("SHA256SUMS")) {
        std::fs::write(out.join("SHA256SUMS"), sums).map_err(|e| e.to_string())?;
    }
    let base = asset_base(&endpoint, version)?;
    let m = manifest(
        version,
        (&note("it"), &note("en")),
        &rfc3339(now),
        &base,
        &signed,
    );
    let json = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
    std::fs::write(out.join("latest.json"), format!("{json}\n")).map_err(|e| e.to_string())?;
    println!(
        "{}/latest.json for {version}: files under {base}/, manifest at {endpoint}",
        out.display()
    );
    Ok(())
}
