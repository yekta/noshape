//! In-app updates (crates/update; docs/handoff/cliapp.md "In-app updates"): the release feed is checked at launch and
//! every 5 minutes (with its ETag: an unchanged answer costs nothing against GitHub's limit), this install's update is
//! downloaded and verified in the background, and a banner in a fixed slot (the navbar's middle; on the documents
//! page beside the account menu) says so: "Update available: v…" with the download's progress, then "Updated —
//! restart to finish" with Restart (the open documents reopen in the new version) and What's new (the release
//! notes). Preferences › Updates turns it off. A click here always answers at once (AGENTS.md).

use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_update::{FeedCache, Fetched, Kind, Release, Staged};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How often the feed is asked (at launch, then this often).
const INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Idle,
    Downloading,
    /// Downloaded, verified and unpacked: a restart applies it.
    Ready,
    /// This install can't apply it itself (a .deb without pkexec, a release without its signature): a link to
    /// download it.
    Manual,
    Failed(String),
}

struct Updates {
    phase: Phase,
    release: Option<Release>,
    staged: Option<Staged>,
    /// Bytes downloaded, of the total.
    progress: Arc<(AtomicU64, AtomicU64)>,
    /// Versions that failed this session (not tried again until the app restarts).
    failed: Vec<String>,
}

impl Global for Updates {}

/// What this build updates from and to.
struct Setup {
    /// The version compared with the feed's.
    current: String,
    feed: String,
    /// A dry-run build's update test: any other version is "newer".
    test: bool,
    kind: Kind,
}

/// A release build checks GitHub's latest release. A dev or dry-run build ("0.0.0-…") checks nothing, unless the
/// release workflow's update test points it at a test release (NOSHAPE_UPDATE_FEED, NOSHAPE_UPDATE_SELF_VERSION,
/// NOSHAPE_UPDATE_TEST_IDENTITY); a release never takes those.
fn setup() -> Option<Setup> {
    if std::env::var_os("NOSHAPE_NO_UPDATES").is_some() {
        return None;
    }
    let built = crate::VERSION;
    let release = noshape_update::is_release(built);
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let (feed, current) = if release { (noshape_update::LATEST.to_string(), built.to_string()) } else { (var("NOSHAPE_UPDATE_FEED")?, var("NOSHAPE_UPDATE_SELF_VERSION").unwrap_or_else(|| built.to_string())) };
    Some(Setup { current, feed, test: !release, kind: noshape_update::current_kind() })
}

/// The release workflow's update test reads what happened here (NOSHAPE_UPDATE_LOG).
pub fn note(line: &str) {
    log::info!("updates: {line}");
    if let Some(path) = std::env::var_os("NOSHAPE_UPDATE_LOG") {
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "app {}: {line}", std::process::id());
        }
    }
}

fn dir() -> std::path::PathBuf {
    noshape_app_core::paths::config_dir().join("updates")
}

fn feed_cache() -> FeedCache {
    std::fs::read(dir().join("feed.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_feed_cache(c: &FeedCache) {
    let _ = std::fs::create_dir_all(dir());
    if let Ok(t) = serde_json::to_vec(c) {
        let _ = std::fs::write(dir().join("feed.json"), t);
    }
}

pub fn init(cx: &mut App) {
    note(&format!("started {} {:?}", crate::VERSION, std::env::args().skip(1).collect::<Vec<_>>()));
    cx.set_global(Updates { phase: Phase::Idle, release: None, staged: None, progress: Arc::new((AtomicU64::new(0), AtomicU64::new(0))), failed: Vec::new() });
    let Some(setup) = setup() else { return };
    let setup = Arc::new(setup);
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(3)).await;
        loop {
            let wait = check(setup.clone(), cx).await;
            cx.background_executor().timer(wait).await;
        }
    })
    .detach();
}

/// Ask the feed; start downloading a newer release. How long until the next check.
async fn check(setup: Arc<Setup>, cx: &mut AsyncApp) -> Duration {
    let off = crate::preferences_window::ListPrefs::load().updates_off;
    let busy = cx.update(|cx| matches!(cx.global::<Updates>().phase, Phase::Downloading | Phase::Ready));
    if off || busy {
        return INTERVAL;
    }
    let url = setup.feed.clone();
    let ex = cx.background_executor().clone();
    let asked = ex.spawn(async move {
        let mut cache = feed_cache();
        let r = noshape_update::fetch(&url, &mut cache);
        save_feed_cache(&cache);
        r
    });
    let release = match asked.with_timeout(Duration::from_secs(90), &ex).await {
        Err(_) => {
            note("check: no answer");
            return INTERVAL;
        }
        Ok(Err(e)) => {
            note(&format!("check: {e}"));
            return INTERVAL;
        }
        Ok(Ok(Fetched::RateLimited(wait))) => {
            note(&format!("check: rate limited for {} s", wait.as_secs()));
            return wait;
        }
        Ok(Ok(Fetched::Release(r))) => r,
    };
    let known = cx.update(|cx| cx.global::<Updates>().failed.contains(&release.version().to_string()));
    let newer = if setup.test { release.version() != setup.current } else { noshape_update::is_newer(&setup.current, &release) };
    if !newer || known {
        return INTERVAL;
    }
    note(&format!("found {}", release.version()));
    download(setup, release, cx);
    INTERVAL
}

fn download(setup: Arc<Setup>, release: Release, cx: &mut AsyncApp) {
    // Signed by release.yml for its tag (see noshape_update::signer), else only a link to the release.
    let signer = noshape_update::signer(crate::VERSION, &release).ok().filter(|_| release.asset(noshape_update::SUMS_BUNDLE).is_some());
    let manual = !setup.kind.can_apply() || signer.is_none();
    let progress = cx.update(|cx| {
        let u = cx.global_mut::<Updates>();
        u.release = Some(release.clone());
        u.phase = if manual { Phase::Manual } else { Phase::Downloading };
        u.progress.0.store(0, Ordering::Relaxed);
        u.progress.1.store(0, Ordering::Relaxed);
        cx.refresh_windows();
        cx.global::<Updates>().progress.clone()
    });
    if manual {
        note(&format!("manual: {:?}", setup.kind));
        return;
    }
    let p = progress.clone();
    let work = cx.background_executor().spawn(async move {
        noshape_update::prepare(&release, &setup.kind, &dir(), &signer.unwrap_or_default(), &move |done, total| {
            p.0.store(done, Ordering::Relaxed);
            p.1.store(total, Ordering::Relaxed);
        })
    });
    cx.spawn(async move |cx| {
        // The banner's percentage, a few times a second while it downloads.
        let ticker = cx.spawn(async move |cx| loop {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            cx.update(|cx| cx.refresh_windows());
        });
        let r = work.await;
        drop(ticker);
        cx.update(|cx| {
            let u = cx.global_mut::<Updates>();
            let v = u.release.as_ref().map(|r| r.version().to_string()).unwrap_or_default();
            match r {
                Ok(staged) => {
                    note(&format!("verified {v}"));
                    u.staged = Some(staged);
                    u.phase = Phase::Ready;
                }
                Err(e) => {
                    note(&format!("refused {v}: {e}"));
                    u.failed.push(v);
                    u.phase = Phase::Failed(e);
                }
            }
            cx.refresh_windows();
            // The update test restarts on its own (a dry-run build only).
            if cx.global::<Updates>().phase == Phase::Ready && std::env::var_os("NOSHAPE_UPDATE_AUTORESTART").is_some() && !noshape_update::is_release(crate::VERSION) {
                restart(cx);
            }
        });
    })
    .detach();
}

/// Restart into the new version: the open documents (saved with every edit) are listed for it to reopen, the
/// helper waits for this process to end, swaps the install and starts it.
fn restart(cx: &mut App) {
    let Some(staged) = cx.global::<Updates>().staged.clone() else { return };
    let docs: Vec<String> = cx.try_global::<crate::files::OpenDocs>().map(|o| o.0.keys().map(|p| p.display().to_string()).collect()).unwrap_or_default();
    let list = dir().join("reopen.json");
    let written = std::fs::create_dir_all(dir()).and_then(|_| std::fs::write(&list, serde_json::to_vec(&docs).unwrap_or_default()));
    let r = written.map_err(|e| format!("{}: {e}", list.display())).and_then(|_| noshape_update::apply(&staged, &list, std::process::id()));
    match r {
        Ok(()) => {
            note(&format!("restarting into {} with {} document(s)", staged.version, docs.len()));
            cx.quit();
        }
        Err(e) => {
            note(&format!("restart failed: {e}"));
            cx.global_mut::<Updates>().phase = Phase::Failed(e);
            cx.refresh_windows();
        }
    }
}

/// The documents a restart for an update left open (`--reopen <list>`).
pub fn reopen_list(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let paths: Vec<String> = std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let _ = std::fs::remove_file(path);
    paths.into_iter().map(std::path::PathBuf::from).filter(|p| p.exists()).collect()
}

fn whats_new(window: &mut Window, cx: &mut App) {
    let Some(release) = cx.global::<Updates>().release.clone() else { return };
    window.open_dialog(cx, move |dialog, _, cx| {
        let pal = p();
        let notes = if release.notes.trim().is_empty() { "No notes for this release.".to_string() } else { release.notes.clone() };
        let page = release.page.clone();
        os::modal(dialog, format!("What's new in noshape {}", release.version()), 640.)
            .child(
                div()
                    .id("whats-new")
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .px(px(11.))
                    .pt(px(4.))
                    .pb(px(12.))
                    .text_size(px(14.))
                    .text_color(pal.text)
                    .child(v_flex().gap(px(4.)).children(notes.lines().map(|l| div().min_h(px(8.)).child(l.to_string())))),
            )
            .footer(
                os::modal_footer()
                    .when(!page.is_empty(), |d| d.child(div().id("whats-new-page").mr_auto().text_size(px(13.)).text_color(gpui_kit::rgb(0x1651b0)).cursor_pointer().child("Release page").on_click(move |_, _, cx| cx.open_url(&page))))
                    .child(os::modal_cancel(os::cancel_button("whats-new-close", "Close", cx))),
            )
    });
}

/// A small button inside the banner.
fn banner_button(id: &'static str, label: &'static str, primary: bool) -> Stateful<Div> {
    let d = div().id(id).flex_none().h(px(20.)).px(px(8.)).flex().items_center().rounded(px(3.)).text_size(px(12.)).cursor_pointer().child(label);
    if primary {
        d.bg(gpui_kit::rgb(0x1651b0)).text_color(gpui_kit::rgb_static(0xffffff)).hover(|d| d.bg(gpui_kit::rgb(0x0f3f8f)))
    } else {
        d.text_color(gpui_kit::rgb(0x1651b0)).hover(|d| d.underline())
    }
}

/// The banner, when there's something to say (None: the slot stays empty).
pub fn banner(cx: &App) -> Option<AnyElement> {
    let u = cx.try_global::<Updates>()?;
    let v = u.release.as_ref()?.version().to_string();
    let pal = p();
    let pill = || h_flex()
        .id("update-banner")
        .flex_none()
        .max_w(px(380.))
        .h(px(26.))
        .pl(px(10.))
        .pr(px(4.))
        .gap(px(8.))
        .rounded(px(3.))
        .border_1()
        .text_size(px(12.))
        .text_color(pal.text);
    let info = pill().bg(pal.info_fill).border_color(pal.info_outline);
    Some(match &u.phase {
        Phase::Idle => return None,
        Phase::Downloading => {
            let (done, total) = (u.progress.0.load(Ordering::Relaxed), u.progress.1.load(Ordering::Relaxed));
            let frac = if total > 0 { (done as f32 / total as f32).clamp(0., 1.) } else { 0. };
            info.pr(px(10.))
                .child(div().flex_none().child(format!("Update available: v{v} — downloading {}%", (frac * 100.).round() as u32)))
                .child(div().flex_none().w(px(60.)).h(px(4.)).rounded(px(2.)).bg(pal.info_outline).child(div().h_full().rounded(px(2.)).bg(gpui_kit::rgb(0x1651b0)).w(px(60. * frac))))
                .into_any_element()
        }
        Phase::Ready => info
            .child(div().flex_none().child("Updated — restart to finish"))
            .child(banner_button("update-restart", "Restart", true).on_click(|_, _, cx| restart(cx)))
            .child(banner_button("update-whats-new", "What's new", false).on_click(|_, window, cx| whats_new(window, cx)))
            .into_any_element(),
        Phase::Manual => {
            let page = u.release.as_ref().map(|r| r.page.clone()).unwrap_or_default();
            info.child(div().flex_none().child(format!("noshape v{v} is available")))
                .child(banner_button("update-download", "Download", true).on_click(move |_, _, cx| cx.open_url(&page)))
                .child(banner_button("update-whats-new", "What's new", false).on_click(|_, window, cx| whats_new(window, cx)))
                .into_any_element()
        }
        Phase::Failed(e) => {
            let e = e.clone();
            let shown = format!("Update v{v} not installed: {e}");
            pill().pr(px(10.))
                .bg(pal.error_fill)
                .border_color(pal.error_outline)
                .child(div().min_w_0().truncate().child(shown))
                .tooltip(crate::ui::tooltip(e))
                .into_any_element()
        }
    })
}

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
