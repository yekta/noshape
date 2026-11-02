//! In-app updates against real Sigstore fixtures (tests/fixtures: SHA256SUMS signed keyless by
//! .github/workflows/update-test-fixtures.yml's run on GitHub, whose identity is identity.txt) and a local HTTP server
//! standing in for GitHub: what verifies is applied; anything changed, signed by another workflow or not logged is
//! refused.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use noshape_update::{Asset, FeedCache, Fetched, Kind, Os, RELEASE_SIGNER, Release, SUMS_BUNDLE, fetch, is_newer, kind_of, prepare, signed_checksum, signer, sigstore};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const TAR: &str = "noshape-0.0.2-linux-x86_64.tar.gz";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

/// The workflow that signed the fixtures.
fn identity() -> String {
    read("identity.txt").trim().to_string()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("noshape-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn bundle() -> Value {
    serde_json::from_str(&read(SUMS_BUNDLE)).unwrap()
}

fn verify(b: &Value, sums: &str, id: &str) -> Result<(), String> {
    sigstore::verify_bundle(b.to_string().as_bytes(), sums.as_bytes(), id)
}

/// Base64 field `ptr` of `b` with one bit of its decoded bytes flipped at `at` (from the end when negative).
fn flip(b: &mut Value, ptr: &str, at: isize) {
    let f = b.pointer_mut(ptr).unwrap_or_else(|| panic!("{ptr}"));
    let mut bytes = B64.decode(f.as_str().unwrap()).unwrap();
    let i = if at < 0 { bytes.len() - (-at) as usize } else { at as usize };
    bytes[i] ^= 1;
    *f = Value::String(B64.encode(bytes));
}

#[test]
fn signed_sums_verify_and_anything_else_is_refused() {
    let (sums, id) = (read("SHA256SUMS"), identity());
    verify(&bundle(), &sums, &id).unwrap();
    assert_eq!(signed_checksum(&sums, read(SUMS_BUNDLE).as_bytes(), &id, TAR).unwrap(), noshape_update::sha256_file(&fixtures().join(TAR)).unwrap());
    assert!(signed_checksum(&sums, read(SUMS_BUNDLE).as_bytes(), &id, "noshape-0.0.2-linux-x86_64.deb").unwrap_err().contains("has no"));

    // SHA256SUMS changed (another checksum for the file).
    let e = verify(&bundle(), &sums.replacen('5', "6", 1), &id).unwrap_err();
    assert!(e.contains("digest"), "{e}");
    // Signed by another workflow, ref or repository: a release's identity, a fork's, a prefix of the real one.
    for other in [format!("{RELEASE_SIGNER}v0.0.2"), id.replace("yekta/noshape", "someone/noshape"), id[..id.len() - 1].to_string()] {
        let e = verify(&bundle(), &sums, &other).unwrap_err();
        assert!(e.contains("signed by"), "{other}: {e}");
    }
    // The signature changed, or the bundle's digest made to match another file.
    let mut b = bundle();
    flip(&mut b, "/messageSignature/signature", -3);
    assert!(verify(&b, &sums, &id).is_err());
    let mut b = bundle();
    flip(&mut b, "/messageSignature/messageDigest/digest", 0);
    assert!(verify(&b, &sums, &id).unwrap_err().contains("digest"));
    // Rekor's promise changed, or its entry moved in time or the log.
    let mut b = bundle();
    flip(&mut b, "/verificationMaterial/tlogEntries/0/inclusionPromise/signedEntryTimestamp", -3);
    assert!(verify(&b, &sums, &id).unwrap_err().contains("signed entry timestamp"));
    let mut b = bundle();
    let t: u64 = b["verificationMaterial"]["tlogEntries"][0]["integratedTime"].as_str().unwrap().parse().unwrap();
    b["verificationMaterial"]["tlogEntries"][0]["integratedTime"] = Value::String((t + 60).to_string());
    assert!(verify(&b, &sums, &id).unwrap_err().contains("signed entry timestamp"));
    let mut b = bundle();
    b["verificationMaterial"]["tlogEntries"] = Value::Array(Vec::new());
    assert!(verify(&b, &sums, &id).unwrap_err().contains("no transparency log entry"));
    // Another certificate (a key not Fulcio's): its body no longer matches Rekor's entry.
    let mut b = bundle();
    flip(&mut b, "/verificationMaterial/certificate/rawBytes", -10);
    assert!(verify(&b, &sums, &id).is_err());
    // A trust root without Sigstore's Fulcio CAs, or without its Rekor log.
    let root: Value = serde_json::from_str(include_str!("../src/trusted_root.json")).unwrap();
    let mut no_ca = root.clone();
    no_ca["certificateAuthorities"] = Value::Array(Vec::new());
    let e = sigstore::verify_with_root(read(SUMS_BUNDLE).as_bytes(), sums.as_bytes(), &id, &no_ca.to_string()).unwrap_err();
    assert!(e.contains("Fulcio"), "{e}");
    let mut no_log = root;
    no_log["tlogs"] = Value::Array(Vec::new());
    let e = sigstore::verify_with_root(read(SUMS_BUNDLE).as_bytes(), sums.as_bytes(), &id, &no_log.to_string()).unwrap_err();
    assert!(e.contains("transparency log"), "{e}");
    // Not a bundle at all.
    assert!(sigstore::verify_bundle(b"{}", sums.as_bytes(), &id).is_err());
}

#[test]
fn a_release_is_signed_by_release_yml_for_its_own_tag() {
    let r = |tag: &str| Release { tag: tag.into(), notes: String::new(), page: String::new(), assets: Vec::new() };
    assert_eq!(signer("0.4.20", &r("v0.4.21")).unwrap(), "https://github.com/yekta/noshape/.github/workflows/release.yml@refs/tags/v0.4.21");
    assert!(signer("0.4.20", &r("0.4.21")).is_err());
    assert!(signer("0.4.20", &r("v0.4.21-rc1")).is_err());
    assert!(signer("0.4.20", &r("update-test")).is_err());
}

#[test]
fn only_a_newer_release_updates_a_release() {
    let r = |tag: &str| Release { tag: tag.into(), notes: String::new(), page: String::new(), assets: Vec::new() };
    assert!(is_newer("0.4.20", &r("v0.4.21")));
    assert!(is_newer("0.4.20", &r("v0.5.0")));
    assert!(!is_newer("0.4.21", &r("v0.4.21")));
    assert!(!is_newer("0.4.22", &r("v0.4.21")));
    // A dev build or a prerelease isn't a release: it never updates, and isn't offered.
    assert!(!is_newer("0.0.0-dev", &r("v0.4.21")));
    assert!(!is_newer("0.4.20", &r("v0.4.21-rc1")));
    assert!(!is_newer("0.4.20", &r("update-test")));
}

#[test]
fn each_install_takes_its_own_asset() {
    let mac = kind_of(Path::new("/Applications/noshape.app/Contents/MacOS/noshape"), Os::Mac);
    if cfg!(target_arch = "aarch64") {
        assert_eq!(mac, Kind::MacBundle { bundle: "/Applications/noshape.app".into() });
        assert_eq!(mac.asset_name("0.4.21").as_deref(), Some("noshape-0.4.21-macos-arm64.app.zip"));
    }
    assert!(matches!(kind_of(Path::new("/Users/me/src/noshape/target/release/noshape"), Os::Mac), Kind::Unsupported(_)));
    assert_eq!(kind_of(Path::new("/usr/bin/noshape"), Os::Linux).asset_name("0.4.21").as_deref(), Some("noshape-0.4.21-linux-x86_64.deb"));
    assert_eq!(kind_of(Path::new("/home/me/.local/bin/noshape"), Os::Linux).asset_name("0.4.21").as_deref(), Some("noshape-0.4.21-linux-x86_64.tar.gz"));
    assert!(matches!(kind_of(Path::new("/root/target-x/release/noshape"), Os::Linux), Kind::Unsupported(_)));
    // Windows: the installer's folder has its uninstaller; a portable copy doesn't.
    let dir = scratch("win-kind");
    std::fs::write(dir.join("unins000.exe"), "").unwrap();
    assert_eq!(kind_of(&dir.join("noshape.exe"), Os::Windows).asset_name("0.4.21").as_deref(), Some("noshape-0.4.21-windows-x86_64-setup.exe"));
    std::fs::remove_file(dir.join("unins000.exe")).unwrap();
    assert_eq!(kind_of(&dir.join("noshape.exe"), Os::Windows).asset_name("0.4.21").as_deref(), Some("noshape-0.4.21-windows-x86_64.zip"));
    let _ = std::fs::remove_dir_all(dir);
    // Windows' verbatim paths (what canonicalize gives) are made plain for the helper; others are left alone.
    use noshape_update::plain_path;
    assert_eq!(plain_path(r"\\?\D:\a\noshape\noshape.exe".into()), PathBuf::from(r"D:\a\noshape\noshape.exe"));
    assert_eq!(plain_path(r"\\?\UNC\server\share\noshape.exe".into()), PathBuf::from(r"\\?\UNC\server\share\noshape.exe"));
    assert_eq!(plain_path("/usr/bin/noshape".into()), PathBuf::from("/usr/bin/noshape"));
}

// ------------------------------------------------------------- a fake GitHub

/// A path's answer: status, headers, body.
type Route = (&'static str, u16, Vec<(&'static str, String)>, Vec<u8>);
/// Requests seen: (path, If-None-Match).
type Seen = Arc<Mutex<Vec<(String, Option<String>)>>>;

struct Server {
    base: String,
    seen: Seen,
}

/// Serves `routes` (path -> status, headers, body); a request with the If-None-Match of a route's ETag gets 304.
fn serve(routes: Vec<Route>) -> Server {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut line = String::new();
            if r.read_line(&mut line).is_err() {
                continue;
            }
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            let mut inm = None;
            loop {
                let mut h = String::new();
                if r.read_line(&mut h).is_err() || h.trim().is_empty() {
                    break;
                }
                if let Some((k, v)) = h.split_once(':')
                    && k.eq_ignore_ascii_case("if-none-match")
                {
                    inm = Some(v.trim().to_string());
                }
            }
            log.lock().unwrap().push((path.clone(), inm.clone()));
            let mut s = s;
            let Some((_, status, headers, body)) = routes.iter().find(|(p, ..)| *p == path) else {
                let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
                continue;
            };
            let etag = headers.iter().find(|(k, _)| *k == "etag").map(|(_, v)| v.clone());
            let (status, body): (u16, &[u8]) = if inm.is_some() && inm == etag { (304, b"") } else { (*status, body) };
            let mut head = format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n", body.len());
            for (k, v) in headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = s.write_all(head.as_bytes()).and_then(|_| s.write_all(body));
        }
    });
    Server { base, seen }
}

fn github_release(base: &str, tag: &str, assets: &[&str]) -> Vec<u8> {
    let assets: Vec<serde_json::Value> = assets.iter().map(|n| serde_json::json!({ "name": n, "browser_download_url": format!("{base}/dl/{n}"), "size": std::fs::metadata(fixtures().join(n)).map(|m| m.len()).unwrap_or(0) })).collect();
    serde_json::json!({ "tag_name": tag, "body": "What's new: the test release.", "html_url": format!("{base}/release"), "assets": assets }).to_string().into_bytes()
}

fn file_route(name: &'static str) -> Route {
    let path: &'static str = Box::leak(format!("/dl/{name}").into_boxed_str());
    (path, 200, Vec::new(), std::fs::read(fixtures().join(name)).unwrap())
}

#[test]
fn the_feed_asks_with_its_etag_and_backs_off_when_limited() {
    // The release JSON needs the server's address: serve twice (the first only to learn it is not needed: URLs are
    // the feed's own business here).
    let body = github_release("http://x", "v0.0.2", &[TAR]);
    let s = serve(vec![
        ("/latest", 200, vec![("etag", "\"abc\"".into())], body),
        ("/limited", 403, vec![("x-ratelimit-remaining", "0".into()), ("retry-after", "120".into())], b"{}".to_vec()),
    ]);
    let mut cache = FeedCache::default();
    let url = format!("{}/latest", s.base);
    let first = fetch(&url, &mut cache).unwrap();
    assert!(matches!(&first, Fetched::Release(r) if r.tag == "v0.0.2" && r.notes.starts_with("What's new")), "{first:?}");
    assert_eq!(cache.etag.as_deref(), Some("\"abc\""));
    // Unchanged: a 304 (free against the limit) gives the cached release.
    let second = fetch(&url, &mut cache).unwrap();
    assert_eq!(second, first);
    let seen = s.seen.lock().unwrap().clone();
    assert_eq!(seen[0].1, None);
    assert_eq!(seen[1].1.as_deref(), Some("\"abc\""));
    // Limited: wait as long as GitHub says.
    let mut other = FeedCache::default();
    assert_eq!(fetch(&format!("{}/limited", s.base), &mut other).unwrap(), Fetched::RateLimited(std::time::Duration::from_secs(120)));
}

fn release_server(tar_bytes: Option<Vec<u8>>) -> (Server, Release) {
    // Two passes: the asset URLs carry the server's address.
    let mut routes = vec![file_route(TAR), file_route("SHA256SUMS"), file_route(SUMS_BUNDLE)];
    if let Some(b) = tar_bytes {
        routes[0].3 = b;
    }
    let s = serve(routes);
    let release = Release::from_github(&serde_json::from_slice(&github_release(&s.base, "v0.0.2", &[TAR, "SHA256SUMS", SUMS_BUNDLE])).unwrap()).unwrap();
    (s, release)
}

#[test]
fn prepare_downloads_verifies_and_unpacks_or_refuses() {
    let (_s, release) = release_server(None);
    let kind = Kind::LinuxTar { bin: "/home/me/.local/bin/noshape".into() };
    let dir = scratch("prepare");
    let seen = Arc::new(Mutex::new((0u64, 0u64)));
    let p = seen.clone();
    let staged = prepare(&release, &kind, &dir, &identity(), &move |d, t| *p.lock().unwrap() = (d, t)).unwrap();
    assert_eq!(staged.version, "0.0.2");
    assert!(staged.payload.join("noshape").is_file(), "{}", staged.payload.display());
    let (done, total) = *seen.lock().unwrap();
    assert!(done > 0 && done == total, "progress {done}/{total}");

    // The same release with its download changed on the way: refused, nothing unpacked.
    let mut bytes = std::fs::read(fixtures().join(TAR)).unwrap();
    let n = bytes.len();
    bytes[n - 5] ^= 0xff;
    let (_t, tampered) = release_server(Some(bytes));
    let dir2 = scratch("prepare-tampered");
    let e = prepare(&tampered, &kind, &dir2, &identity(), &|_, _| {}).unwrap_err();
    assert!(e.contains(TAR) && e.contains("checksum"), "{e}");
    assert!(!dir2.join("staged-0.0.2").exists() && !dir2.join(TAR).exists());
    // Signed by another workflow (a release's identity on a test signature): refused before downloading.
    let e = prepare(&release, &kind, &scratch("prepare-other"), &format!("{RELEASE_SIGNER}v0.0.2"), &|_, _| {}).unwrap_err();
    assert!(e.contains("signed by"), "{e}");
    // A release without its signature: refused.
    let unsigned = Release { assets: release.assets.iter().filter(|a| a.name != SUMS_BUNDLE).cloned().collect::<Vec<Asset>>(), ..release.clone() };
    assert!(prepare(&unsigned, &kind, &scratch("prepare-unsigned"), &identity(), &|_, _| {}).unwrap_err().contains(SUMS_BUNDLE));
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(dir2);
}

/// The Linux helper: once the app (here: a process that has ended, not yet reaped) is gone, the new binary takes the old one's
/// place and starts with --reopen and the list of documents.
#[cfg(target_os = "linux")]
#[test]
fn the_linux_helper_swaps_the_binary_and_starts_it() {
    let (_s, release) = release_server(None);
    let home = scratch("helper");
    let bin = home.join("bin/noshape");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, "old").unwrap();
    let kind = Kind::LinuxTar { bin: bin.clone() };
    let staged = prepare(&release, &kind, &home.join("cache"), &identity(), &|_, _| {}).unwrap();
    let list = home.join("reopen.json");
    std::fs::write(&list, "[]").unwrap();
    // The app has ended but its parent hasn't reaped it yet (a zombie): the helper goes on all the same.
    let mut gone = std::process::Command::new("true").spawn().unwrap();
    let pid = gone.id();
    std::thread::sleep(std::time::Duration::from_millis(200));
    noshape_update::apply(&staged, &list, pid).unwrap();
    let t = std::time::Instant::now();
    while std::fs::read_to_string(&bin).unwrap_or_default() == "old" && t.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(std::fs::read_to_string(&bin).unwrap().contains("noshape 0.0.2 (test)"), "the binary wasn't swapped");
    gone.wait().unwrap();
    let _ = std::fs::remove_dir_all(home);
}
