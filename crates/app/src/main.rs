//! noshape: a fast, fully local, parametric CAD application.

mod actions;
mod assembly_mate_dialog;
mod assembly_scene;
mod assembly_view;
mod assets;
mod doc_model;
mod drawing_scene;
mod drawing_tools;
mod drawing_ui;
mod drawing_view;
mod feature_dialog;
mod files;
mod gpu;
mod history_panel;
mod part_studio;
mod scene_builder;
mod session;
mod sketch_mode;
mod theme;
mod variable_studio;
mod viewport;
mod workspace;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use std::path::PathBuf;

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
        cx.set_global(gpu::GpuGlobal(gpu.clone()));
        cx.on_action(|_: &actions::Quit, cx| cx.quit());

        let path = path.clone().unwrap_or_else(files::new_document_path);
        let initial = if args.iter().any(|a| a == "--demo-drawing") { Some(noshape_model::demo::demo_document_with_drawing()) } else if demo { Some(noshape_model::demo::demo_document()) } else { None };
        files::open_document_window(path, initial, gpu.clone(), cx).expect("could not open document");
        let g2 = gpu.clone();
        cx.on_action(move |_: &actions::NewDocument, cx| {
            if let Err(e) = files::open_document_window(files::new_document_path(), None, g2.clone(), cx) {
                log::error!("new document failed: {e:#}");
            }
        });
        let g3 = gpu.clone();
        cx.on_action(move |_: &actions::OpenDocument, cx| files::prompt_open(g3.clone(), cx));
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
}
