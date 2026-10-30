//! noshape's Drawing studio UI (docs/CRATES.md): the drawing editor, its tools, panels and dialogs, on
//! `noshape-app-core`. What it needs of the app above it (the workspace, the release log, the documents library,
//! file tabs, preferences) it reaches through [`drawing_hooks`], which the `noshape` binary registers at startup.

// The app's base layer under the paths these files used inside the app (`crate::ui::…`).
#[allow(unused_imports)]
use noshape_app_core::*;

pub mod drawing_aux;
pub mod drawing_break;
pub mod drawing_callout;
pub mod drawing_cell_panel;
pub mod drawing_cells;
pub mod drawing_config;
pub mod drawing_hole_table;
pub mod drawing_centermark;
pub mod drawing_chooser;
pub mod drawing_chrome;
pub mod drawing_color;
pub mod drawing_create;
pub mod drawing_crop;
pub mod drawing_custom_table;
pub mod drawing_datum;
pub mod drawing_detail;
pub mod drawing_drawprops;
pub mod drawing_export;
pub mod drawing_extra;
pub mod drawing_gdt;
pub mod drawing_hatch;
pub mod drawing_hide_edges;
pub mod drawing_hide_parts;
pub mod drawing_hooks;
pub mod drawing_images;
pub mod drawing_inplace;
pub mod drawing_insert;
pub mod drawing_inspection;
pub mod drawing_jog;
pub mod drawing_keys;
pub mod drawing_label;
pub mod drawing_launcher;
pub mod drawing_leaders;
pub mod drawing_linestyle;
pub mod drawing_mbd;
pub mod drawing_more;
pub mod drawing_move_sheet;
pub mod drawing_note;
pub mod drawing_note_color;
pub mod drawing_painter;
pub mod drawing_palette;
pub mod drawing_panels;
pub mod drawing_props;
pub mod drawing_refs;
pub mod drawing_related;
pub mod drawing_replicate;
pub mod drawing_revision;
pub mod drawing_ruler;
pub mod drawing_scene;
pub mod drawing_aligned;
pub mod drawing_sf;
pub mod drawing_section;
pub mod drawing_section_edit;
pub mod drawing_shaded;
pub mod drawing_sheetprops;
pub mod drawing_sketch;
pub mod drawing_sketch_dim;
pub mod drawing_smart;
pub mod drawing_styles;
pub mod drawing_table_dlg;
pub mod drawing_table_tool;
pub mod drawing_toolbar;
pub mod drawing_tools;
pub mod drawing_ui;
pub mod drawing_view;
pub mod drawing_view_sketches;
pub mod drawing_watermark;
pub mod drawing_weld;
pub mod drawing_wt;
