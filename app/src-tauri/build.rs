use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn main() {
    let commit = std::env::var("ND_COMMIT")
        .ok()
        .or_else(|| git(&["rev-parse", "--short=10", "HEAD"]))
        .unwrap_or_default();
    let built = std::env::var("ND_BUILT")
        .ok()
        .or_else(|| git(&["log", "-1", "--format=%cs"]))
        .unwrap_or_default();
    println!("cargo:rustc-env=ND_COMMIT={commit}");
    println!("cargo:rustc-env=ND_BUILT={built}");
    println!("cargo:rerun-if-env-changed=ND_COMMIT");
    println!("cargo:rerun-if-env-changed=ND_BUILT");
    let mut watched = vec!["HEAD".to_owned(), "packed-refs".to_owned()];
    watched.extend(git(&["symbolic-ref", "-q", "HEAD"]));
    for path in watched
        .iter()
        .filter_map(|r| git(&["rev-parse", "--git-path", r]))
    {
        println!("cargo:rerun-if-changed={path}");
    }
    tauri_build::build();
}
