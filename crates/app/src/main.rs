//! noshape: a fast, fully local, parametric CAD application.

mod actions;
mod assets;
mod doc_model;
mod feature_dialog;
mod gpu;
mod history_panel;
mod part_studio;
mod scene_builder;
mod session;
mod sketch_mode;
mod theme;
mod viewport;
mod workspace;

use gpui_kit::component::{Theme, ThemeMode, TitleBar};
use gpui_kit::*;
use std::path::PathBuf;

/// Where new documents go by default.
fn documents_dir() -> PathBuf {
    let base = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("noshape");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn new_document_path() -> PathBuf {
    let dir = documents_dir();
    for n in 1.. {
        let p = dir.join(format!("Untitled {n}.noshape"));
        if !p.exists() {
            return p;
        }
    }
    unreachable!()
}

fn apply_theme(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let pal = theme::p();
    Theme::update(cx, |t| {
        t.font_size = px(13.);
        t.radius = px(5.);
        t.background = pal.app_bg.into();
        t.foreground = pal.text.into();
        t.border = pal.border.into();
        t.primary = pal.accent.into();
        t.primary_hover = pal.accent_hover.into();
        t.popover = pal.panel_raised.into();
        t.popover_foreground = pal.text.into();
        t.muted = pal.panel_hover.into();
        t.muted_foreground = pal.text_muted.into();
        t.input = pal.border_strong.into();
        t.ring = pal.accent.into();
        t.title_bar = pal.panel.into();
        t.title_bar_border = pal.border.into();
        t.accent = pal.panel_hover.into();
        t.accent_foreground = pal.text.into();
        t.list_hover = pal.panel_hover.into();
        t.list_active = pal.accent_soft.into();
        t.selection = theme::with_alpha(pal.accent, 0.35).into();
    });
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,naga=warn,wgpu=warn,zbus=warn,tracing=warn")).init();
    let gpu = gpu::Gpu::new().expect("noshape needs a GPU (Vulkan, Metal or DirectX 12)");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|a| a == "--demo");
    let path = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from);

    gpui_kit::application().with_assets(assets::AppAssets).run(move |cx| {
        gpui_kit::init(cx);
        apply_theme(cx);
        actions::bind_default_keys(cx);
        cx.on_action(|_: &actions::Quit, cx| cx.quit());

        let session = match &path {
            Some(p) if p.exists() => session::Session::open(p),
            Some(p) => session::Session::create(p, if demo { noshape_model::demo::demo_document() } else { noshape_model::Document::new() }),
            None => session::Session::create(new_document_path(), if demo { noshape_model::demo::demo_document() } else { noshape_model::Document::new() }),
        }
        .expect("could not open document");

        let gpu = gpu.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
            window_min_size: Some(size(px(900.), px(560.))),
            #[cfg(target_os = "linux")]
            window_decorations: Some(WindowDecorations::Client),
            app_id: Some("dev.noshape.noshape".into()),
            ..TitleBar::window_options()
        };
        gpui_kit::open_window(options, cx, move |window, cx| {
            let doc = cx.new(|_| doc_model::DocModel::new(session));
            let ws = cx.new(|cx| workspace::Workspace::new(doc, gpu, window, cx));
            let fh = ws.read(cx).focus_handle(cx);
            window.defer(cx, move |window, cx| fh.focus(window, cx));
            ws
        })
        .expect("failed to open window");
    });
}
