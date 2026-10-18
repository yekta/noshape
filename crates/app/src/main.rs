//! noshape: a fast, fully local, parametric CAD application.
// Release builds on Windows are GUI apps (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod actions;
mod assembly_mate_dialog;
mod assembly_scene;
mod assembly_view;
mod assets;
mod derived;
mod doc_model;
mod drawing_create;
mod drawing_insert;
mod drawing_extra;
mod drawing_more;
mod drawing_props;
mod drawing_scene;
mod drawing_tools;
mod drawing_ui;
mod drawing_view;
mod feature_dialog;
mod feature_studio;
mod files;
mod gpu;
mod history_panel;
mod os_menu;
mod os_ui;
mod part_studio;
mod scene_builder;
mod session;
mod sketch_mode;
mod start_library;
mod start_thumbnail;
mod start_window;
mod tab_manager;
mod theme;
mod ui;
mod var_complete;
mod variable_studio;
mod viewport;
mod workspace;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use std::path::PathBuf;

/// Onshape's UI font is Flama (commercial); Roboto (Apache 2.0) is the
/// closest open match. Bundled so every platform renders the same.
const UI_FONT: &str = "Roboto";

fn load_fonts(cx: &mut App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = vec![
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Regular.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Medium.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-SemiBold.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Bold.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Italic.ttf")),
    ];
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        log::warn!("could not load the UI font: {e:#}");
    }
}

/// Onshape's light theme (colors from `theme.rs`, which cites the tokens).
fn apply_theme(cx: &mut App) {
    load_fonts(cx);
    Theme::change(ThemeMode::Light, None, cx);
    let pal = theme::p();
    let white: Hsla = gpui_kit::rgb(0xffffff).into();
    Theme::update(cx, |t| {
        t.font_family = UI_FONT.into();
        t.font_size = px(14.);
        t.radius = px(4.);
        t.radius_lg = px(5.);
        t.shadow = false;
        t.background = pal.app_bg.into();
        t.foreground = pal.text.into();
        t.border = pal.border.into();
        t.window_border = pal.border_strong.into();
        t.primary = pal.accent.into();
        t.primary_hover = pal.accent_hover.into();
        t.primary_active = pal.accent.into();
        t.primary_foreground = white;
        t.button_primary = pal.accent.into();
        t.button_primary_hover = pal.accent_hover.into();
        t.button_primary_active = pal.accent.into();
        t.button_primary_foreground = white;
        t.secondary = pal.panel.into();
        t.secondary_hover = pal.panel_hover.into();
        t.secondary_active = pal.panel_active.into();
        t.secondary_foreground = pal.text.into();
        t.button = pal.panel.into();
        t.button_hover = pal.panel_hover.into();
        t.button_active = pal.panel_active.into();
        t.button_foreground = pal.text.into();
        t.popover = pal.panel_raised.into();
        t.popover_foreground = pal.text.into();
        t.muted = pal.panel_hover.into();
        t.muted_foreground = pal.text_muted.into();
        t.input = pal.field_outline.into();
        t.ring = pal.focus.into();
        t.caret = pal.text.into();
        t.title_bar = pal.panel_inactive.into();
        t.title_bar_border = pal.border.into();
        t.accent = pal.panel_hover.into();
        t.accent_foreground = pal.text.into();
        t.list_hover = pal.row_hover.into();
        t.list_active = pal.accent_soft.into();
        t.list_active_border = pal.accent_soft.into();
        t.tab_bar = pal.tab_bar.into();
        t.tab = pal.tab_inactive.into();
        t.tab_active = pal.panel.into();
        t.tab_foreground = pal.text_muted.into();
        t.tab_active_foreground = pal.text.into();
        t.danger = pal.error.into();
        t.success = pal.success.into();
        t.warning = pal.warning.into();
        t.info = pal.info.into();
        t.link = gpui_kit::rgb(0x094174).into();
        t.scrollbar_thumb = gpui_kit::rgb(0xc1c1c1).into();
        t.scrollbar_thumb_hover = gpui_kit::rgb(0xa8a8a8).into();
        t.selection = gpui_kit::rgb(0xb2ddf6).into();
        // Feature dialog preview slider: #777 track, white thumb (§1.6).
        t.slider_bar = gpui_kit::rgb(0x777777).into();
        t.slider_thumb = gpui_kit::rgb(0xffffff).into();
        // Modal backdrop: Bootstrap's 50% black, as Onshape's modals.
        t.overlay = gpui_kit::rgba(0x00000080).into();
    });
}

fn main() {
    noshape_render::perf::process_start();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,naga=warn,wgpu=warn,zbus=warn,tracing=warn")).init();
    // Initializes the GPU on a background thread; the first use waits for it.
    let gpu = gpu::Gpu::new().expect("could not start the GPU thread");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|a| a == "--demo");
    let path = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from);

    let app = gpui_kit::application().with_assets(assets::AppAssets);
    // macOS: clicking the dock icon with no windows open shows the start window.
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            files::show_start_window(cx);
        }
    });
    app.run(move |cx| {
        noshape_render::perf::milestone("GPUI app started");
        gpui_kit::init(cx);
        apply_theme(cx);
        actions::bind_default_keys(cx);
        cx.set_global(gpu::GpuGlobal(gpu.clone()));
        cx.on_action(|_: &actions::Quit, cx| cx.quit());

        files::init(cx);
        if path.is_some() || args.iter().any(|a| a.starts_with("--demo")) {
            let path = path.clone().unwrap_or_else(files::new_document_path);
            let initial = if args.iter().any(|a| a == "--demo-drawing-full") {
                Some(noshape_model::drawing::showcase::showcase_document())
            } else if args.iter().any(|a| a == "--demo-drawing") { Some(noshape_model::demo::demo_document_with_drawing()) } else if demo { Some(noshape_model::demo::demo_document()) } else { None };
            files::open_document_window(path, initial, gpu.clone(), cx).expect("could not open document");
        } else {
            files::show_start_window(cx);
        }
        let g2 = gpu.clone();
        cx.on_action(move |_: &actions::NewDocument, cx| {
            let _ = &g2;
            start_window::show_and_create(cx);
        });
        let g3 = gpu.clone();
        cx.on_action(move |_: &actions::OpenDocument, cx| files::prompt_open(g3.clone(), cx));
    });
}
