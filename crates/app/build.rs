//! Embeds the application icon into the Windows executable, and stamps the
//! build's commit (`NOSHAPE_COMMIT`, 12 hex digits) for the version shown
//! as Onshape shows its build: "(1.222.90730.c8d74e607a47)".

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icons/noshape.ico");
    commit();
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/icons/noshape.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the app icon: {e}");
        }
    }
}

/// The checked-out commit, when building from a git checkout (else none).
fn commit() {
    let git = std::path::Path::new("../../.git");
    // Rebuild when HEAD moves (a commit or a checkout).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(git.join("HEAD"))
        && let Some(r) = head.trim().strip_prefix("ref: ")
    {
        println!("cargo:rerun-if-changed=../../.git/{r}");
        println!("cargo:rerun-if-changed=../../.git/packed-refs");
    }
    let out = std::process::Command::new("git").args(["rev-parse", "--short=12", "HEAD"]).output();
    if let Ok(o) = out
        && o.status.success()
    {
        let sha = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !sha.is_empty() {
            println!("cargo:rustc-env=NOSHAPE_COMMIT={sha}");
        }
    }
}
