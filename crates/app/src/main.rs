//! noshape: a fast, fully local, parametric CAD application.
// Release builds on Windows are GUI apps (no console window).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// `cargo test`/`clippy --all-targets` hit "recursion limit reached while expanding
// `#[test]`" at the default of 128.
#![recursion_limit = "256"]

// The app's base layer (docs/CRATES.md), under its old paths.
use noshape_app_core::{actions, assets, cached_slice, camera_menu, color_mixer, cursor, custom_tables, derived, doc_model, export_naming, gpu, graphics_prefs, graphics_prefs_dialog, i18n, isolate_ui, named_views_dialog, nav_trace, navigation, os_menu, os_select, os_ui, overlay_labels, quantity_field, scene_builder, section_dialog, session, start_thumbnail, theme, tool_synonyms, tool_text, toolbar_customize, ui, viewport};
// The Drawing studio UI (docs/CRATES.md), under its old paths; what it needs of the app: `register_drawing_hooks`.
use noshape_drawing_ui::{drawing_create, drawing_keys, drawing_sketch, drawing_view};
mod fs_profile;
mod live_server;
mod std_source;
mod assembly_mate_dialog;
mod assembly_scene;
mod assembly_view;
mod config_visibility;
mod material_libraries;
mod numbering_prefs;
mod release_conditions;
mod release_prefs;
mod release_settings;
mod revision_template;
mod revision_tools;
mod company_settings;
mod material_prefs;
mod release_thumbs;
mod doc_dialogs;
mod shortcuts_page;
mod file_tab_view;
mod feature_dialog;
mod feature_studio;
mod fs_notices;
mod fs_reference;
mod select_item;
mod fs_import;
mod fs_search;
mod import_dialog;
mod material_library_view;
mod files;
mod history_panel;
mod compare_graphics;
mod compare_mode;
mod workspace_protection;
mod repair_mode;
mod merge_dialog;
mod version_properties;
mod units_prefs;
mod doc_panels;
mod mouse_prefs;
mod drawing_prefs;
mod part_studio;
mod preferences_window;
mod properties_dialog;
mod release_dialog;
mod sketch_mode;
mod sketch_fonts;
mod prefs_general;
mod prefs_export_rules;
mod measure_panel;
mod start_library;
mod start_search;
mod start_trash;
mod shortcuts;
mod smoke;
mod start_window;
mod tab_manager;
mod tab_thumbs;
mod var_complete;
mod variable_import;
mod variable_insert;
mod variable_studio;
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
        // Rows edited while regeneration is paused (live: bold italic).
        std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-BoldItalic.ttf")),
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
    let white: Hsla = gpui_kit::rgb_static(0xffffff).into();
    // Live (sketch/13): text selected in a focused field is Chrome's
    // `#3367d1` with white text; code editors keep `t.selection` below
    // (patched gpui-base, vendor/gpui-base).
    cx.set_global(gpui_kit::base::input::PlainSelectionColors { background: gpui_kit::rgb(0x3367d1).into(), foreground: white });
    // Onshape (live dom/caret-inputs.json): `input::placeholder { font-style:
    // italic; font-weight: 300 }` in `rgb(102, 102, 102)`.
    // Preferences › Language and Theming, as saved.
    {
        let prefs = crate::preferences_window::ListPrefs::load();
        crate::i18n::set_current(prefs.language.as_deref().unwrap_or(crate::i18n::DEFAULT));
        crate::theme::set_dark(prefs.dark_mode);
        // Company settings: drawings' revision tables follow its template.
        crate::release_settings::release_settings();
    }
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
    apply_kit_theme(cx);
}

/// GPUI Kit's theme from the palette in effect: at startup, and again when
/// Preferences › Theming changes (live applies the theme without a reload).
pub(crate) fn apply_kit_theme(cx: &mut App) {
    let pal = theme::p();
    let white: Hsla = gpui_kit::rgb_static(0xffffff).into();
    let dark = gpui_kit::hex_theme_active();
    // Feature Studio's current-line gutter cell: light #dcdcdc; dark (Ace
    // twilight, live dom/dark-drawing-fs-vs.json) white at 3%.
    let active_cell = if dark { gpui_kit::rgba_static(0xffffff08) } else { gpui_kit::rgb(0xdcdcdc) };
    cx.set_global(gpui_kit::base::input::CodeGutterMetrics { min_digits: 1, number_inset: px(15.), right_margin: px(0.), fold_width: px(14.), text_gap: px(4.), active_background: Some(active_cell.into()) });
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
        // Primary buttons stay #1651b0 in dark (live dk-910/920: Create,
        // Share, Try Professional).
        let button_blue: Hsla = gpui_kit::rgb_static(0x1651b0).into();
        t.primary = button_blue;
        t.primary_hover = pal.accent_hover.into();
        t.primary_active = button_blue;
        t.primary_foreground = white;
        t.button_primary = button_blue;
        t.button_primary_hover = pal.accent_hover.into();
        t.button_primary_active = button_blue;
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
        // #f0f0f0 gutter, dark line numbers, #ededed current line. Dark: Ace
        // twilight on #333 (live dom/dark-drawing-fs-vs.json): #232323 gutter,
        // #e2e2e2 numbers, the current line white at 3%.
        let (gutter, active, number) = if dark { (0x232323ff, 0xffffff08, 0xe2e2e2ff) } else { (0xf0f0f0ff, 0xedededff, 0x333333ff) };
        let mut code = (*t.highlight_theme).clone();
        code.style.editor_gutter_background = Some(gpui_kit::rgba_static(gutter).into());
        code.style.editor_active_line = Some(gpui_kit::rgba_static(active).into());
        code.style.editor_line_number = Some(gpui_kit::rgba_static(number).into());
        code.style.editor_active_line_number = Some(gpui_kit::rgba_static(number).into());
        t.highlight_theme = std::sync::Arc::new(code);
    });
}

/// What the Drawing studio (noshape-drawing-ui) needs of the app: the workspace, other documents' windows, file tabs,
/// the release log, the documents library, Create version, the export rules link, a preference.
fn register_drawing_hooks() {
    noshape_drawing_ui::drawing_hooks::register(noshape_drawing_ui::drawing_hooks::Hooks {
        import: workspace::request_import,
        open_document_window: |path, gpu, cx| files::open_document_window(path, None, gpu, cx),
        pick_file: |doc, title, kinds, exts, window, cx, on_pick| file_tab_view::pick_file(doc, title, kinds, exts, window, cx, on_pick),
        release_log: release_dialog::release_log,
        watermark_text: release_dialog::drawing_watermark_text,
        library_docs: || start_library::Library::default().scan(&start_library::ScanInput::current()).docs,
        create_version: history_panel::open_create_version_in,
        export_rules_link: preferences_window::export_rules_link,
        light_model_space: || preferences_window::ListPrefs::load().drawing_light_model_space,
    });
}

fn main() {
    noshape_render::perf::process_start();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,naga=warn,wgpu=warn,zbus=warn,tracing=warn")).init();
    // Initializes the GPU on a background thread; the first use waits for it.
    let gpu = gpu::Gpu::new().expect("could not start the GPU thread");
    noshape_render::perf::milestone("GPU thread started");
    // Regeneration evaluates custom features and the features std runs through the FeatureScript host.
    noshape_model::custom::register();
    // Feature Studio imports of other documents / versions read their snapshots.
    noshape_model::custom::set_external_tab_loader(derived::external_tab);
    register_drawing_hooks();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|a| a == "--demo");
    let path = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from);

    let app = gpui_kit::application().with_assets(assets::AppAssets);
    noshape_render::perf::milestone("GPUI application created");
    // macOS: clicking the dock icon with no windows open shows the start window.
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            files::show_start_window(cx);
        }
    });
    app.run(move |cx| {
        noshape_render::perf::milestone("GPUI app started");
        gpui_kit::init(cx);
        noshape_render::perf::milestone("gpui-kit initialized");
        // Onshape's text (Chrome) is grayscale anti-aliased, without the
        // colour fringes of subpixel rendering.
        cx.set_text_rendering_mode(gpui_kit::TextRenderingMode::Grayscale);
        apply_theme(cx);
        noshape_render::perf::milestone("theme applied");
        graphics_prefs::init(cx);
        // The default keys with the user's customizations (Preferences › Keyboard shortcuts).
        shortcuts::init(cx, actions::default_key_bindings());
        feature_studio::bind_keys(cx);
        // Mouse controls (also hands "Use pen input as mouse" to the platform).
        navigation::MouseControls::current();
        // Preferences › Decimal format (comma decimals in expressions and values).
        units_prefs::apply();
        cx.set_global(gpu::GpuGlobal(gpu.clone()));
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        noshape_render::perf::milestone("preferences and keys set");

        files::init(cx);
        noshape_render::perf::milestone("app initialized");
        if path.is_some() || args.iter().any(|a| a.starts_with("--demo")) {
            let path = path.clone().unwrap_or_else(files::new_document_path);
            let initial = if args.iter().any(|a| a == "--demo-drawing-full") {
                Some(noshape_model::drawing::showcase::showcase_document())
            } else if args.iter().any(|a| a == "--demo-fab") {
                Some(noshape_model::demo_fab::fabrication_document())
            } else if args.iter().any(|a| a == "--demo-tables") {
                Some(noshape_model::demo_fab::custom_tables_document())
            } else if args.iter().any(|a| a == "--demo-drawing") { Some(noshape_model::demo::demo_document_with_drawing()) } else if demo { Some(noshape_model::demo::demo_document()) } else { None };
            // A file that can't be opened (damaged, not a noshape document,
            // stale side files) lands on the Documents page with an error,
            // never a panic.
            if let Err(e) = files::open_document_window(path, initial, gpu.clone(), cx) {
                log::error!("could not open document: {e:#}");
                start_window::show_with_error(format!("Could not open document: {e:#}"), cx);
            }
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
