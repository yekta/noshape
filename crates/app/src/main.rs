//! noshape: a fast, fully local, parametric CAD application.
// Release builds on Windows are GUI apps (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// `cargo test`/`clippy --all-targets` hit "recursion limit reached while expanding
// `#[test]`" at the default of 128.
#![recursion_limit = "256"]

mod actions;
mod fs_profile;
mod std_source;
mod assembly_mate_dialog;
mod assembly_scene;
mod assembly_view;
mod assets;
mod cached_slice;
mod color_mixer;
mod config_visibility;
mod material_libraries;
mod numbering_prefs;
mod tool_synonyms;
mod release_prefs;
mod release_thumbs;
mod custom_tables;
mod derived;
mod doc_dialogs;
mod doc_model;
mod drawing_centermark;
mod drawing_create;
mod drawing_drawprops;
mod drawing_insert;
mod drawing_keys;
mod drawing_sketch_dim;
mod drawing_label;
mod drawing_mbd;
mod drawing_extra;
mod drawing_more;
mod drawing_painter;
mod drawing_inspection;
mod drawing_panels;
mod drawing_export;
mod export_naming;
mod drawing_smart;
mod drawing_sketch;
mod drawing_hatch;
mod drawing_images;
mod drawing_break;
mod drawing_aux;
mod drawing_chooser;
mod drawing_crop;
mod drawing_detail;
mod drawing_revision;
mod drawing_view_sketches;
mod drawing_hide_parts;
mod drawing_note;
mod drawing_cell_panel;
mod drawing_cells;
mod drawing_note_color;
mod drawing_refs;
mod drawing_color;
mod drawing_callout;
mod drawing_gdt;
mod drawing_ruler;
mod drawing_table_dlg;
mod drawing_custom_table;
mod drawing_chrome;
mod drawing_jog;
mod drawing_related;
mod drawing_move_sheet;
mod drawing_styles;
mod drawing_hide_edges;
mod drawing_inplace;
mod drawing_linestyle;
mod drawing_section_edit;
mod drawing_replicate;
mod drawing_toolbar;
mod drawing_palette;
mod drawing_datum;
mod drawing_table_tool;
mod drawing_weld;
mod file_tab_view;
mod drawing_props;
mod drawing_scene;
mod drawing_section;
mod drawing_shaded;
mod drawing_sheetprops;
mod drawing_tools;
mod drawing_ui;
mod drawing_view;
mod feature_dialog;
mod feature_studio;
mod fs_notices;
mod fs_reference;
mod select_item;
mod fs_import;
mod fs_search;
mod import_dialog;
mod isolate_ui;
mod material_library_view;
mod files;
mod gpu;
mod graphics_prefs;
mod graphics_prefs_dialog;
mod history_panel;
mod compare_graphics;
mod compare_mode;
mod workspace_protection;
mod repair_mode;
mod merge_dialog;
mod version_properties;
mod units_prefs;
mod doc_panels;
mod cursor;
mod mouse_prefs;
mod drawing_prefs;
mod navigation;
mod os_menu;
mod overlay_labels;
mod os_select;
mod os_ui;
mod part_studio;
mod preferences_window;
mod properties_dialog;
mod quantity_field;
mod release_dialog;
mod scene_builder;
mod session;
mod sketch_mode;
mod start_library;
mod start_search;
mod start_thumbnail;
mod start_trash;
mod shortcuts;
mod smoke;
mod start_window;
mod tab_manager;
mod tab_thumbs;
mod theme;
mod tool_text;
mod toolbar_customize;
mod touch;
mod ui;
mod var_complete;
mod variable_import;
mod variable_insert;
mod variable_studio;
mod viewport;
mod workspace;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use std::path::PathBuf;

/// Onshape's UI font is Flama (commercial); Roboto (Apache 2.0) is the
/// closest open match. Bundled so every platform renders the same.
const UI_FONT: &str = "Roboto";

/// The app's version: the release workflow stamps the tag's (`NOSHAPE_VERSION`,
/// "0.4.3"; "0.0.0-<sha>" for untagged runs), else the crate's.
pub(crate) const VERSION: &str = match option_env!("NOSHAPE_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// The build as the documents page shows it, Onshape's "version.commit"
/// ("0.4.4.c8d74e607a47"; the version alone outside a git checkout).
pub(crate) fn build_label() -> String {
    match option_env!("NOSHAPE_COMMIT") {
        Some(c) => format!("{VERSION}.{c}"),
        None => VERSION.to_string(),
    }
}

fn load_fonts(cx: &mut App) {
    let fonts: Vec<std::borrow::Cow<'static, [u8]>> = vec![
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Regular.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Medium.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-SemiBold.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Bold.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Italic.ttf")),
        // Placeholders (Onshape: `input::placeholder { font-style: italic; font-weight: 300 }`).
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-LightItalic.ttf")),
        // Measure values (Onshape's `.measurement-value`: Inconsolata 14 px / 600).
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Inconsolata-Regular.ttf")),
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Inconsolata-Bold.ttf")),
    ];
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        log::warn!("could not load the UI font: {e:#}");
    }
    // The drawing fonts, for notes and table cells edited in place.
    let drawing: Vec<std::borrow::Cow<'static, [u8]>> = noshape_sketch::text::font_files().map(std::borrow::Cow::Borrowed).collect();
    if let Err(e) = cx.text_system().add_fonts(drawing) {
        log::warn!("could not load the drawing fonts: {e:#}");
    }
}

/// Onshape's light theme (colors from `theme.rs`, which cites the tokens).
fn apply_theme(cx: &mut App) {
    load_fonts(cx);
    Theme::change(ThemeMode::Light, None, cx);
    let pal = theme::p();
    let white: Hsla = gpui_kit::rgb(0xffffff).into();
    // Live (sketch/13): text selected in a focused field is Chrome's
    // `#3367d1` with white text; code editors keep `t.selection` below
    // (patched gpui-base, vendor/gpui-base).
    cx.set_global(gpui_kit::base::input::PlainSelectionColors { background: gpui_kit::rgb(0x3367d1).into(), foreground: white });
    // Onshape (live dom/caret-inputs.json): `input::placeholder { font-style:
    // italic; font-weight: 300 }` in `rgb(102, 102, 102)`.
    cx.set_global(gpui_kit::base::input::PlaceholderStyle { italic: true, weight: Some(FontWeight::LIGHT), color: Some(gpui_kit::rgb(0x666666).into()) });
    // Live document/70 (Ace): the Feature Studio gutter is 42 px for one-digit
    // line counts, numbers 21 px in, a 14 px fold column, the code 4 px after it,
    // the current line's cell #dcdcdc (patched gpui-base).
    // Feature Studio squiggles show Ace's tooltip, drawn by feature_studio.rs.
    gpui_kit::base::input::set_app_diagnostic_hover(true);
    // Squiggles (live document/fs-squiggles-zoom8x): errors #df5528, warnings
    // #fcc81a (the kit's theme status colors are a stub without tree-sitter).
    cx.set_global(gpui_kit::base::input::DiagnosticColors { error: gpui_kit::rgb(0xdf5528).into(), warning: gpui_kit::rgb(0xfcc81a).into(), info: gpui_kit::rgb(0x3d8fd1).into(), hint: gpui_kit::rgb(0x3d8fd1).into() });
    // Ace's gutter markers (live document/fs-gutter-*): the notice icons, 13 px
    // in the 16 px cell 2 px into the gutter, with Ace's tooltip.
    cx.set_global(gpui_kit::base::input::GutterMarkerRenderer(std::rc::Rc::new(feature_studio::gutter_marker)));
    cx.set_global(gpui_kit::base::input::CodeGutterMetrics { min_digits: 1, number_inset: px(15.), right_margin: px(0.), fold_width: px(14.), text_gap: px(4.), active_background: Some(gpui_kit::rgb(0xdcdcdc).into()) });
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
        // Feature Studio code editor (Onshape's Ace editor, help images):
        // #f0f0f0 gutter, dark line numbers, #ededed current line.
        let mut code = (*t.highlight_theme).clone();
        code.style.editor_gutter_background = Some(gpui_kit::rgb(0xf0f0f0).into());
        code.style.editor_active_line = Some(gpui_kit::rgb(0xededed).into());
        code.style.editor_line_number = Some(gpui_kit::rgb(0x333333).into());
        code.style.editor_active_line_number = Some(gpui_kit::rgb(0x333333).into());
        t.highlight_theme = std::sync::Arc::new(code);
    });
}

fn main() {
    noshape_render::perf::process_start();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,naga=warn,wgpu=warn,zbus=warn,tracing=warn")).init();
    // Initializes the GPU on a background thread; the first use waits for it.
    let gpu = gpu::Gpu::new().expect("could not start the GPU thread");
    // Feature Studio imports of other documents / versions read their snapshots.
    noshape_model::custom::set_external_tab_loader(derived::external_tab);
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
        // Onshape's text (Chrome) is grayscale anti-aliased, without the
        // colour fringes of subpixel rendering.
        cx.set_text_rendering_mode(gpui_kit::TextRenderingMode::Grayscale);
        apply_theme(cx);
        graphics_prefs::init(cx);
        actions::bind_default_keys(cx);
        feature_studio::bind_keys(cx);
        // Mouse controls (also hands "Use pen input as mouse" to the platform).
        navigation::MouseControls::current();
        // Preferences › Decimal format (comma decimals in expressions and values).
        units_prefs::apply();
        cx.set_global(gpu::GpuGlobal(gpu.clone()));
        cx.on_action(|_: &actions::Quit, cx| cx.quit());

        files::init(cx);
        if path.is_some() || args.iter().any(|a| a.starts_with("--demo")) {
            let path = path.clone().unwrap_or_else(files::new_document_path);
            let initial = if args.iter().any(|a| a == "--demo-drawing-full") {
                Some(noshape_model::drawing::showcase::showcase_document())
            } else if args.iter().any(|a| a == "--demo-fab") {
                Some(noshape_model::demo_fab::fabrication_document())
            } else if args.iter().any(|a| a == "--demo-tables") {
                Some(noshape_model::demo_fab::custom_tables_document())
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
