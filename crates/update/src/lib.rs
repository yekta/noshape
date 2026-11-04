//! In-app updates (docs/handoff/cliapp.md "In-app updates"): the release feed (GitHub's `releases/latest`, polled with
//! an ETag so an unchanged answer costs nothing against the 60-an-hour limit), the download of this install's asset,
//! its verification (the release's SHA256SUMS signed keyless with Sigstore by release.yml run for that release's tag,
//! checked by [`sigstore`], then the file's line in it; on macOS also the new app's Developer ID signature: anything
//! else is refused), unpacking it, and the helper that swaps the install once the app has quit and opens the new
//! version with the same documents. No GUI here: the app (crates/app `updates`) drives it and draws the banner.

pub mod sigstore;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Who signs a release: release.yml run for a `v…` tag of this repository (GitHub's OIDC identity in the Fulcio
/// certificate; the tag is appended).
pub const RELEASE_SIGNER: &str = "https://github.com/yekta/noshape/.github/workflows/release.yml@refs/tags/";

/// The Sigstore bundle over SHA256SUMS that every release carries.
pub const SUMS_BUNDLE: &str = "SHA256SUMS.sigstore.json";

/// The Apple team whose Developer ID must have signed a macOS update.
pub const APPLE_TEAM: &str = "44R7KF7645";

/// The identity `release`'s SHA256SUMS must be signed by: release.yml for exactly its tag (`v` and a release
/// version). A build that isn't a release ("0.0.0-…", a dry run) may be given another in NOSHAPE_UPDATE_TEST_IDENTITY
/// (release.yml's update test, signed by its run on a branch); a release never takes one.
pub fn signer(current: &str, release: &Release) -> Result<String, String> {
    if !is_release(current)
        && let Ok(id) = std::env::var("NOSHAPE_UPDATE_TEST_IDENTITY")
        && !id.trim().is_empty()
    {
        return Ok(id.trim().to_string());
    }
    match release.tag.strip_prefix('v') {
        Some(v) if is_release(v) => Ok(format!("{RELEASE_SIGNER}{}", release.tag)),
        _ => Err(format!("{} isn't a release tag", release.tag)),
    }
}

/// The newest release (prereleases and drafts aren't "latest").
pub const LATEST: &str = "https://api.github.com/repos/yekta/noshape/releases/latest";

// ------------------------------------------------------------------ the feed

/// A release as the app needs it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub tag: String,
    /// The release notes (Markdown).
    pub notes: String,
    /// The release's page.
    pub page: String,
    pub assets: Vec<Asset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
}

impl Release {
    /// From GitHub's release JSON.
    pub fn from_github(v: &Value) -> Result<Release, String> {
        let tag = v["tag_name"].as_str().ok_or("the release has no tag")?.to_string();
        let assets = v["assets"].as_array().map(|a| a.iter().filter_map(|a| Some(Asset { name: a["name"].as_str()?.into(), url: a["browser_download_url"].as_str()?.into(), size: a["size"].as_u64().unwrap_or(0) })).collect()).unwrap_or_default();
        Ok(Release { tag, notes: v["body"].as_str().unwrap_or("").into(), page: v["html_url"].as_str().unwrap_or("").into(), assets })
    }

    /// "0.4.21" for tag "v0.4.21".
    pub fn version(&self) -> &str {
        self.tag.trim_start_matches('v')
    }

    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

/// Whether `version` is a release's ("0.4.21"), not a dev or dry-run build's ("0.0.0-dev", "0.0.0-1a2b3c4").
pub fn is_release(version: &str) -> bool {
    semver::Version::parse(version).is_ok_and(|v| v.pre.is_empty())
}

/// Whether `release` is newer than `current`. A dev or prerelease build ("0.0.0-dev", "0.0.0-1a2b3c4") never
/// updates: it isn't a release.
pub fn is_newer(current: &str, release: &Release) -> bool {
    match (semver::Version::parse(current), semver::Version::parse(release.version())) {
        (Ok(c), Ok(r)) => c.pre.is_empty() && r.pre.is_empty() && r > c,
        _ => false,
    }
}

/// What the feed said last (kept on disk, so a restart asks with the ETag too).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FeedCache {
    pub url: String,
    pub etag: Option<String>,
    pub release: Option<Release>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Fetched {
    Release(Release),
    /// GitHub's limit for this address: ask again after this long.
    RateLimited(Duration),
}

fn agent(read_timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .timeout_per_read(Some(read_timeout))
        .user_agent(format!("noshape-updater/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Ask the feed `url` for its release, with the ETag of the last answer: unchanged ("304 Not Modified", which
/// GitHub doesn't count against the limit) gives the cached release.
pub fn fetch(url: &str, cache: &mut FeedCache) -> Result<Fetched, String> {
    if cache.url != url {
        *cache = FeedCache { url: url.into(), ..Default::default() };
    }
    let mut req = agent(Duration::from_secs(30)).get(url).header("Accept", "application/vnd.github+json").header("X-GitHub-Api-Version", "2022-11-28");
    if let (Some(etag), Some(_)) = (&cache.etag, &cache.release) {
        req = req.header("If-None-Match", etag);
    }
    let mut resp = req.call().map_err(|e| format!("{url}: {e}"))?;
    let status = resp.status().as_u16();
    let header = |k: &str| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    match status {
        304 => cache.release.clone().map(Fetched::Release).ok_or_else(|| "304 without a cached release".into()),
        200 => {
            let etag = header("etag");
            let text = resp.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| format!("{url}: not JSON: {e}"))?;
            let release = Release::from_github(&v)?;
            cache.etag = etag;
            cache.release = Some(release.clone());
            Ok(Fetched::Release(release))
        }
        403 | 429 => {
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let wait = header("retry-after").and_then(|s| s.parse::<u64>().ok()).or_else(|| header("x-ratelimit-reset").and_then(|s| s.parse::<u64>().ok()).map(|t| t.saturating_sub(now))).unwrap_or(3600);
            Ok(Fetched::RateLimited(Duration::from_secs(wait.clamp(60, 3600))))
        }
        s => Err(format!("{url}: HTTP {s}")),
    }
}

// --------------------------------------------------------- this install

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    pub fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        }
    }
}

/// How this copy of noshape was installed, which says which asset updates it and how.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// noshape.app (the dmg): the bundle is swapped.
    MacBundle { bundle: PathBuf },
    /// The installer's (Program Files): the new installer runs silently.
    WindowsInstalled { exe: PathBuf },
    /// The portable zip: the exe (and noshape.com) are swapped in place.
    WindowsPortable { exe: PathBuf },
    /// The tar.gz's install.sh: the binary is swapped.
    LinuxTar { bin: PathBuf },
    /// The .deb: installed with pkexec (or downloaded by hand when there is none).
    LinuxDeb { bin: PathBuf },
    /// No update path (the reason).
    Unsupported(String),
}

/// The kind of the install whose executable is `exe` (links resolved).
pub fn kind_of(exe: &Path, os: Os) -> Kind {
    let s = exe.to_string_lossy();
    match os {
        Os::Mac => match exe.ancestors().nth(3) {
            Some(b) if s.contains(".app/Contents/MacOS/") && b.extension().is_some_and(|e| e == "app") => {
                if cfg!(target_arch = "aarch64") { Kind::MacBundle { bundle: b.to_path_buf() } } else { Kind::Unsupported("releases are for Apple silicon".into()) }
            }
            _ => Kind::Unsupported("not an app bundle (a build from source)".into()),
        },
        Os::Windows => {
            let dir = exe.parent().unwrap_or(Path::new(""));
            if dir.join("unins000.exe").is_file() { Kind::WindowsInstalled { exe: exe.into() } } else { Kind::WindowsPortable { exe: exe.into() } }
        }
        Os::Linux => {
            if exe == Path::new("/usr/bin/noshape") {
                Kind::LinuxDeb { bin: exe.into() }
            } else if exe.parent().and_then(Path::file_name).is_some_and(|d| d == "debug" || d == "release") {
                // cargo's target/debug or target/release (any target dir).
                Kind::Unsupported("a build from source".into())
            } else {
                Kind::LinuxTar { bin: exe.into() }
            }
        }
    }
}

pub fn current_kind() -> Kind {
    match std::env::current_exe().and_then(|p| p.canonicalize()) {
        Ok(exe) => kind_of(&plain_path(exe), Os::current()),
        Err(e) => Kind::Unsupported(format!("couldn't tell where this program is: {e}")),
    }
}

impl Kind {
    /// The release asset that updates this install.
    pub fn asset_name(&self, version: &str) -> Option<String> {
        Some(match self {
            Kind::MacBundle { .. } => format!("noshape-{version}-macos-arm64.app.zip"),
            Kind::WindowsInstalled { .. } => format!("noshape-{version}-windows-x86_64-setup.exe"),
            Kind::WindowsPortable { .. } => format!("noshape-{version}-windows-x86_64.zip"),
            Kind::LinuxTar { .. } => format!("noshape-{version}-linux-x86_64.tar.gz"),
            Kind::LinuxDeb { .. } => format!("noshape-{version}-linux-x86_64.deb"),
            Kind::Unsupported(_) => return None,
        })
    }

    /// Whether the app can apply the update itself (else: a link to download it).
    pub fn can_apply(&self) -> bool {
        match self {
            Kind::LinuxDeb { .. } => on_path("pkexec"),
            Kind::Unsupported(_) => false,
            _ => true,
        }
    }
}

/// Windows' canonical paths are verbatim (`\\?\D:\…`), which Windows PowerShell's file commands don't take: the
/// plain `D:\…` form (a network path `\\?\UNC\…` is left as it is).
pub fn plain_path(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC\\") && rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

// ------------------------------------------------- download and verify

/// The lines of a SHA256SUMS file: (sha256 hex, file name).
pub fn parse_sums(text: &str) -> Vec<(String, String)> {
    text.lines().filter_map(|l| {
        let (h, n) = l.split_once(char::is_whitespace)?;
        Some((h.to_ascii_lowercase(), n.trim_start().trim_start_matches('*').to_string()))
    }).collect()
}

/// The SHA-256 (hex) of the file at `path`.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut sha, mut buf) = (Sha256::new(), vec![0u8; 1 << 16]);
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            return Ok(sha.finalize().iter().map(|b| format!("{b:02x}")).collect());
        }
        sha.update(&buf[..n]);
    }
}

/// SHA256SUMS (`sums`) and its Sigstore bundle verified for `signer`: the checksum it gives `name`.
pub fn signed_checksum(sums: &str, bundle: &[u8], signer: &str, name: &str) -> Result<String, String> {
    sigstore::verify_bundle(bundle, sums.as_bytes(), signer).map_err(|e| format!("SHA256SUMS: {e}"))?;
    parse_sums(sums).into_iter().find(|(_, n)| n == name).map(|(h, _)| h).ok_or_else(|| format!("SHA256SUMS has no {name}"))
}

fn get_text(url: &str) -> Result<String, String> {
    let mut resp = agent(Duration::from_secs(60)).get(url).call().map_err(|e| format!("{url}: {e}"))?;
    if resp.status().as_u16() != 200 {
        return Err(format!("{url}: HTTP {}", resp.status().as_u16()));
    }
    resp.body_mut().read_to_string().map_err(|e| format!("{url}: {e}"))
}

/// Download `asset` into `dest`, calling `progress(done, total)` as it goes.
pub fn download(asset: &Asset, dest: &Path, progress: &dyn Fn(u64, u64)) -> Result<(), String> {
    let mut resp = agent(Duration::from_secs(60)).get(&asset.url).call().map_err(|e| format!("{}: {e}", asset.name))?;
    if resp.status().as_u16() != 200 {
        return Err(format!("{}: HTTP {}", asset.name, resp.status().as_u16()));
    }
    let total = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok()).unwrap_or(asset.size);
    let mut reader = resp.body_mut().with_config().limit(u64::MAX).reader();
    let tmp = dest.with_extension("part");
    let mut out = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let (mut buf, mut done) = (vec![0u8; 1 << 16], 0u64);
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("{}: {e}", asset.name))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| format!("{}: {e}", tmp.display()))?;
        done += n as u64;
        progress(done, total);
    }
    drop(out);
    std::fs::rename(&tmp, dest).map_err(|e| format!("{}: {e}", dest.display()))
}

/// An update downloaded, verified and unpacked, ready to apply.
#[derive(Clone, Debug)]
pub struct Staged {
    pub version: String,
    pub kind: Kind,
    /// The new app (macOS: noshape.app; Windows portable / Linux: the folder with the executable) or installer.
    pub payload: PathBuf,
}

/// Download this install's asset of `release` into `dir`, check it against SHA256SUMS signed by `signer` (see
/// [`signer`]), and unpack it (a macOS app must carry noshape's Developer ID signature). Nothing is applied.
pub fn prepare(release: &Release, kind: &Kind, dir: &Path, signer: &str, progress: &dyn Fn(u64, u64)) -> Result<Staged, String> {
    let v = release.version().to_string();
    let name = kind.asset_name(&v).ok_or_else(|| match kind {
        Kind::Unsupported(why) => why.clone(),
        _ => "no update for this install".into(),
    })?;
    let need = |n: &str| release.asset(n).ok_or_else(|| format!("the release has no {n}"));
    let (asset, sums, bundle) = (need(&name)?, need("SHA256SUMS")?, need(SUMS_BUNDLE)?);
    // SHA256SUMS first: signed for this release, it names the file's checksum.
    let sha = signed_checksum(&get_text(&sums.url)?, get_text(&bundle.url)?.as_bytes(), signer, &name)?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(&name);
    download(asset, &file, progress)?;
    let got = sha256_file(&file)?;
    if got != sha {
        let _ = std::fs::remove_file(&file);
        return Err(format!("{name}: checksum {got}, SHA256SUMS says {sha}"));
    }
    let payload = unpack(&file, kind, &dir.join(format!("staged-{v}")))?;
    if matches!(kind, Kind::MacBundle { .. }) {
        developer_id(&payload)?;
    }
    Ok(Staged { version: v, kind: kind.clone(), payload })
}

/// The app at `app` is sealed by a Developer ID Application certificate of [`APPLE_TEAM`] (Apple's designated
/// requirement for Developer ID, the team pinned).
fn developer_id(app: &Path) -> Result<(), String> {
    let req = format!("=anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = \"{APPLE_TEAM}\"");
    let mut c = quiet("/usr/bin/codesign");
    c.args(["--verify", "--deep", "--strict", "-R"]).arg(req).arg(app);
    run(c, "the new app's signature isn't noshape's Developer ID")
}

/// A system program by its full path (never one found first on PATH), with no console window on Windows.
fn quiet(program: &str) -> Command {
    #[cfg(windows)]
    let program = format!(r"{}\System32\{program}", std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()));
    let mut c = Command::new(program);
    c.stdin(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000);
    c
}

fn run(mut c: Command, what: &str) -> Result<(), String> {
    let out = c.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(format!("{what}: {}", String::from_utf8_lossy(&out.stderr).trim())) }
}

/// The verified download unpacked (installers and .debs are used as they are).
fn unpack(file: &Path, kind: &Kind, to: &Path) -> Result<PathBuf, String> {
    if matches!(kind, Kind::WindowsInstalled { .. } | Kind::LinuxDeb { .. }) {
        return Ok(file.into());
    }
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    // ditto keeps a macOS app's signature; elsewhere the system's tar (Windows' bsdtar reads zips too).
    let mut c = if matches!(kind, Kind::MacBundle { .. }) {
        let mut c = quiet("/usr/bin/ditto");
        c.arg("-x").arg("-k");
        c
    } else {
        let mut c = quiet(if cfg!(windows) { "tar.exe" } else if Path::new("/usr/bin/tar").is_file() { "/usr/bin/tar" } else { "/bin/tar" });
        c.arg("-xf");
        c
    };
    c.arg(file);
    if !matches!(kind, Kind::MacBundle { .. }) {
        c.arg("-C");
    }
    c.arg(to);
    run(c, "unpacking the update")?;
    let want = match kind {
        Kind::MacBundle { .. } => "noshape.app",
        Kind::WindowsPortable { .. } => "noshape.exe",
        _ => "noshape",
    };
    find(to, want, 2).ok_or_else(|| format!("the update has no {want}"))
}

/// `name` in `dir` or its subfolders, `depth` levels down: the folder holding it (the bundle itself for an .app).
fn find(dir: &Path, name: &str, depth: usize) -> Option<PathBuf> {
    let direct = dir.join(name);
    if name.ends_with(".app") && direct.is_dir() {
        return Some(direct);
    }
    if !name.ends_with(".app") && direct.is_file() {
        return Some(dir.into());
    }
    if depth == 0 {
        return None;
    }
    std::fs::read_dir(dir).ok()?.flatten().filter(|e| e.path().is_dir()).find_map(|e| find(&e.path(), name, depth - 1))
}

// ------------------------------------------------------------------ apply

const MAC_HELPER: &str = r#"#!/bin/sh
# noshape's update helper (crates/update): once the app has quit, swap the bundle for the new one, then open it with
# the documents that were open.
pid="$1" bundle="$2" new="$3" list="$4"
log() { [ -n "$NOSHAPE_UPDATE_LOG" ] && echo "helper: $*" >> "$NOSHAPE_UPDATE_LOG"; }
# Until it has ended (a zombie its parent hasn't reaped yet counts as ended).
while kill -0 "$pid" 2>/dev/null && ! ps -o stat= -p "$pid" | grep -q Z; do sleep 0.2; done
old="$bundle.old.$$"
if mv "$bundle" "$old" 2>/dev/null && mv "$new" "$bundle" 2>/dev/null; then
  rm -rf "$old"
else
  [ -d "$bundle" ] || mv "$old" "$bundle"
  # Applications not writable by this user: the system's password prompt.
  /usr/bin/osascript -e "do shell script \"rm -rf '$old' && mv '$bundle' '$old' && mv '$new' '$bundle' && rm -rf '$old'\" with administrator privileges" || { log "swap failed"; open -n "$bundle" --args --update-failed "The update was not installed: noshape could not be replaced in its folder (the password prompt was declined or failed). Restart from the update card to try again." --reopen "$list"; exit 1; }
fi
log "swapped $bundle"
if [ -n "$NOSHAPE_UPDATE_LOG" ]; then
  open -n "$bundle" --env "NOSHAPE_UPDATE_LOG=$NOSHAPE_UPDATE_LOG" --args --reopen "$list"
else
  open -n "$bundle" --args --reopen "$list"
fi
"#;

const LINUX_HELPER: &str = r#"#!/bin/sh
# noshape's update helper (crates/update): once the app has quit, put the new binary in place (or install the new
# .deb), then start it with the documents that were open.
pid="$1" mode="$2" bin="$3" new="$4" list="$5"
log() { [ -n "$NOSHAPE_UPDATE_LOG" ] && echo "helper: $*" >> "$NOSHAPE_UPDATE_LOG"; }
# Until it has ended (a zombie its parent hasn't reaped yet counts as ended).
while kill -0 "$pid" 2>/dev/null && ! ps -o stat= -p "$pid" | grep -q Z; do sleep 0.2; done
if [ "$mode" = deb ]; then
  pkexec apt-get install -y "$new" || { log "pkexec apt-get failed"; env -u NOSHAPE_UPDATE_AUTORESTART setsid "$bin" --update-failed "The update was not installed: the package install was declined or failed. Restart from the update card to try again." --reopen "$list" > /dev/null 2>&1 < /dev/null & exit 1; }
elif ! { cp "$new/noshape" "$bin.new" 2>/dev/null && chmod 755 "$bin.new" && mv -f "$bin.new" "$bin"; }; then
  rm -f "$bin.new"
  pkexec install -m 755 "$new/noshape" "$bin" || { log "install failed"; env -u NOSHAPE_UPDATE_AUTORESTART setsid "$bin" --update-failed "The update was not installed: noshape could not be replaced (the password prompt was declined or failed). Restart from the update card to try again." --reopen "$list" > /dev/null 2>&1 < /dev/null & exit 1; }
fi
log "swapped $bin"
env -u NOSHAPE_UPDATE_AUTORESTART setsid "$bin" --reopen "$list" > /dev/null 2>&1 < /dev/null &
"#;

const WINDOWS_HELPER: &str = r#"# noshape's update helper (crates/update): once the app has quit, run the new installer silently (the install
# folder, its PATH entry and the agents' connections stay) or swap the portable exe, then start noshape with the
# documents that were open. If the update can't be installed, the old noshape starts again and says why.
param([int]$AppPid, [string]$Mode, [string]$Exe, [string]$Payload, [string]$List)
$HelperLog = Join-Path (Split-Path $Payload) 'update-helper.log'
function Log($m) {
  Add-Content -Path $HelperLog -Value "$(Get-Date -Format o) helper: $m" -ErrorAction SilentlyContinue
  if ($env:NOSHAPE_UPDATE_LOG) { Add-Content -Path $env:NOSHAPE_UPDATE_LOG -Value "helper: $m" }
}
function Failed($why) {
  Log "failed: $why"
  Remove-Item Env:NOSHAPE_UPDATE_AUTORESTART -ErrorAction SilentlyContinue
  Start-Process -FilePath $Exe -ArgumentList '--update-failed', ('"' + $why + '"'), '--reopen', ('"' + $List + '"')
  exit 1
}
while (Get-Process -Id $AppPid -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 200 }
# Other copies from the install folder (a coding agent's `noshape mcp`, a terminal's `noshape`) hold the files Windows
# won't replace; the agents start them again when they need them.
$dir = Split-Path $Exe
Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and (Split-Path $_.Path) -eq $dir } | ForEach-Object { Log "closing $($_.Path) ($($_.Id))"; Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
if ($Mode -eq 'installer') {
  try {
    $p = Start-Process -FilePath $Payload -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOCANCEL', '/FORCECLOSEAPPLICATIONS' -Verb RunAs -Wait -PassThru -ErrorAction Stop
  } catch { Failed "The update was not installed: Windows' permission prompt was declined or failed. Restart from the update card to try again." }
  if ($p.ExitCode -ne 0) { Failed "The update was not installed: its installer stopped (code $($p.ExitCode)). Close noshape everywhere it runs, then restart from the update card. Details: $HelperLog" }
} else {
  try {
    Copy-Item (Join-Path $Payload 'noshape.exe') $Exe -Force -ErrorAction Stop
    if (Test-Path (Join-Path $Payload 'noshape.com')) { Copy-Item (Join-Path $Payload 'noshape.com') (Join-Path $dir 'noshape.com') -Force -ErrorAction Stop }
  } catch { Failed "The update was not installed: $_" }
}
Log "swapped $Exe"
Remove-Item Env:NOSHAPE_UPDATE_AUTORESTART -ErrorAction SilentlyContinue
Start-Process -FilePath $Exe -ArgumentList '--reopen', ('"' + $List + '"')
"#;

/// Start the helper that applies `staged` once process `pid` (the app) has quit, then starts the new version with
/// `--reopen <list>` (a JSON list of the documents that were open). The helper runs on its own: quit the app next.
pub fn apply(staged: &Staged, list: &Path, pid: u32) -> Result<(), String> {
    let dir = staged.payload.parent().unwrap_or(Path::new(".")).to_path_buf();
    let write = |name: &str, text: &str| -> Result<PathBuf, String> {
        let p = dir.join(name);
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
        Ok(p)
    };
    let pid = pid.to_string();
    let mut c = match &staged.kind {
        Kind::MacBundle { bundle } => {
            let mut c = quiet("/bin/sh");
            c.arg(write("update-helper.sh", MAC_HELPER)?).arg(&pid).arg(bundle).arg(&staged.payload).arg(list);
            c
        }
        Kind::LinuxTar { bin } | Kind::LinuxDeb { bin } => {
            let mut c = quiet("/bin/sh");
            let mode = if matches!(staged.kind, Kind::LinuxDeb { .. }) { "deb" } else { "tar" };
            c.arg(write("update-helper.sh", LINUX_HELPER)?).arg(&pid).arg(mode).arg(bin).arg(&staged.payload).arg(list);
            c
        }
        Kind::WindowsInstalled { exe } | Kind::WindowsPortable { exe } => {
            let mut c = quiet(r"WindowsPowerShell\v1.0\powershell.exe");
            let mode = if matches!(staged.kind, Kind::WindowsInstalled { .. }) { "installer" } else { "portable" };
            c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"]).arg(write("update-helper.ps1", WINDOWS_HELPER)?).arg(&pid).arg(mode).arg(exe).arg(&staged.payload).arg(list);
            c
        }
        Kind::Unsupported(why) => return Err(why.clone()),
    };
    c.stdout(Stdio::null()).stderr(Stdio::null());
    // Its own process group: the helper outlives the app.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut c, 0);
    c.spawn().map(|_| ()).map_err(|e| format!("starting the update helper: {e}"))
}
