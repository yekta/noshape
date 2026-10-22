//! Headless drawings: a Drawing tab's sheet built the way the Drawing
//! editor builds it (view sources regenerated, view geometry computed
//! exactly, the sheet assembled), so the shared drawing tools
//! (`noshape_scene::drawing_tools`) can pick, preview and place on it, and
//! the edit commits as the editor's does.

use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::drawing::build::{BuiltSheet, SheetInfo, assemble_sheet};
use noshape_model::drawing::create::DrawingSource;
use noshape_model::drawing::templates::DrawingTemplate;
use noshape_model::drawing::views::{self, SourceKey, Sources, ViewCache, ViewGeometry};
use noshape_model::drawing::{Drawing, Projection, SheetSize, TitleBlockStyle, ViewId};
use noshape_scene::drawing_tools::{Sel, SheetPick, Tool};
use std::collections::HashMap;
use std::sync::Arc;

/// Store meta key of the document's release log (the app's release dialog).
pub const RELEASES_META_KEY: &str = "releases";

/// The pixels per sheet millimeter the API's clicks are made at: snap
/// radii (8 px) are then 0.4 mm, picking (5 px) 0.25 mm.
pub const API_ZOOM: f64 = 20.0;

/// A Drawing tab's sheet, built.
pub struct SheetModel {
    pub tab: TabId,
    pub drawing: Drawing,
    pub sheet: usize,
    pub sources: Sources,
    pub geoms: HashMap<ViewId, Arc<ViewGeometry>>,
    pub built: Option<Arc<BuiltSheet>>,
    pub info: SheetInfo,
    /// A view's source changed since the views were made: they keep that
    /// geometry until Update (the yellow Update button; qa5 D17).
    pub out_of_date: bool,
}

/// The document state the drawing's views were last made from, when a
/// source changed since (the views keep their geometry until Update): that
/// document and the changed sources. Pinned sources never go out of date.
pub fn old_state(session: &crate::session::Session, drawing: &Drawing) -> Option<(Document, Vec<SourceKey>)> {
    let mv: noshape_store::MicroversionId = drawing.sources_at.as_ref()?.parse().ok()?;
    let head = session.head().ok()?;
    if head == mv {
        return None;
    }
    // Both states as stored, so a document never written back from memory
    // (a new or imported one) compares like with like.
    let old = Document::from_blobs(&session.store.read_all(mv).ok()?).ok()?;
    let current = Document::from_blobs(&session.store.read_all(head).ok()?).ok()?;
    let keys: Vec<SourceKey> = views::referenced_sources(drawing)
        .into_iter()
        .filter(|k| k.version.is_none() && old.tab(k.tab).is_some() && views::source_hash(&old, k).is_some_and(|h| views::source_hash(&current, k) != Some(h)))
        .collect();
    (!keys.is_empty()).then_some((old, keys))
}

/// Before a drawing edit is committed: up-to-date views were made from the
/// current document state (an out-of-date drawing keeps its `sources_at`).
pub fn stamp_sources(session: &crate::session::Session, drawing: &mut Drawing) {
    if old_state(session, drawing).is_none() {
        drawing.sources_at = session.head().ok().map(|m| m.to_string());
    }
}

impl SheetModel {
    /// Build sheet `sheet` of drawing `drawing` (the tab's, or an edited
    /// copy) in `doc`: its sources (plus `extra`, e.g. what Insert view
    /// places), exact view geometry and the assembled sheet.
    pub fn build(session: &crate::session::Session, tab: TabId, drawing: Drawing, sheet: usize, extra: &[SourceKey]) -> Self {
        let doc = &session.doc;
        let versions = |id: &str| session.document_at_version(id).ok().flatten();
        let mut sources = views::regenerate_sources_with(doc, &drawing, &versions);
        let mut assemblies = crate::derived::assembly_evaluator(Default::default());
        // Views not updated since a change: their sources as they were made.
        let old = old_state(session, &drawing);
        let out_of_date = old.is_some();
        if let Some((old_doc, keys)) = old {
            for k in keys {
                if let Some(src) = views::regenerate_source(&old_doc, &k, &mut noshape_model::regen::Regenerator::new(), &mut assemblies) {
                    sources.insert(k, src);
                }
            }
        }
        let missing: Vec<&SourceKey> = extra.iter().filter(|k| !sources.contains_key(*k)).collect();
        for k in missing {
            if let Some(src) = views::regenerate_source(doc, k, &mut noshape_model::regen::Regenerator::new(), &mut assemblies) {
                sources.insert(k.clone(), src);
            }
        }
        let sheet = sheet.min(drawing.sheets.len().saturating_sub(1));
        let cache = ViewCache::new();
        let mut geoms = HashMap::new();
        if let Some(s) = drawing.sheets.get(sheet) {
            views::compute_sheet_views(&drawing, s, &sources, &cache, true);
            for v in &s.views {
                if let Some(g) = views::view_key(&drawing, s, v.id, &sources, true).and_then(|k| cache.get(k)) {
                    geoms.insert(v.id, g);
                }
            }
        }
        let mut info = sheet_info(session, tab);
        // Custom tables (the editor evaluates them in the background).
        let fs_at = |v: &str| crate::derived::ExternalRef::parse(v).and_then(|e| crate::derived::external_snapshot(&e.path, e.microversion).ok()).map(|d| (*d).clone());
        info.custom_tables = noshape_model::drawing::tables::evaluate_custom_tables(doc, &drawing, &sources, &fs_at);
        let built = (!drawing.sheets.is_empty()).then(|| Arc::new(assemble_sheet(&drawing, sheet, &geoms, &sources, &info)));
        SheetModel { tab, drawing, sheet, sources, geoms, built, info, out_of_date }
    }

    /// The shared tools' view of this sheet with `tool` active and the
    /// pointer at `cursor` (sheet mm), hovering what lies there.
    pub fn pick<'a>(&'a self, tool: &'a Tool, selection: &'a [Sel], cursor: [f64; 2]) -> SheetPick<'a> {
        let mut pk = SheetPick {
            drawing: &self.drawing,
            display: &self.drawing,
            sheet: self.sheet,
            built: self.built.as_deref(),
            geoms: &self.geoms,
            tool,
            hover: None,
            cursor,
            zoom: API_ZOOM,
            snap_mid_quad: false,
            selection,
            sources: &self.sources,
        };
        pk.hover = pk.hover_at();
        pk
    }
}

/// The title block's document facts (as the editor's `update_info`).
pub fn sheet_info(session: &crate::session::Session, tab: TabId) -> SheetInfo {
    let log: noshape_model::release::ReleaseLog = session.store.get_meta(RELEASES_META_KEY).ok().flatten().map(|b| noshape_model::release::ReleaseLog::from_json(&b)).unwrap_or_default();
    SheetInfo {
        doc_name: session.name(),
        user_name: noshape_store::local_user_name(),
        drawing_name: session.doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default(),
        revisions: noshape_model::drawing::tables::revision_rows(&log, tab),
        custom_tables: Default::default(),
    }
}

/// What every drawing edit does before it is committed (the editor's
/// `commit`): the sheet shown goes with the change, sketch dimensions stay
/// on their geometry, views linked across sheets follow.
pub fn finish_edit(d: &mut Drawing, sheet: usize) {
    d.active_sheet = sheet;
    if let Some(s) = d.sheets.get_mut(sheet) {
        s.sync_sketch_dimensions();
    }
    d.sync_links();
}

/// The drawing of a tab.
pub fn drawing_of(doc: &Document, tab: TabId) -> Option<&Drawing> {
    match doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::Drawing(d)) => Some(d),
        _ => None,
    }
}

/// A new drawing as Create Drawing makes it: the template's sheet, and
/// with `suggested` the source's suggested views (scaled to its bounds,
/// sketch sources facing the sketch plane) and its MBD datums.
pub fn new_drawing(doc: &Document, template: &DrawingTemplate, source: Option<&DrawingSource>, suggested: bool) -> Drawing {
    let bounds = if suggested { source.and_then(|s| noshape_model::drawing::create::source_bounds(doc, s)) } else { None };
    let frame = source.and_then(|s| noshape_model::drawing::create::source_frame(doc, s));
    let mut d = noshape_model::drawing::create::create_drawing(template, source, bounds, suggested, frame);
    // DRAWN: who created it, saved with the drawing.
    d.drawn_by = Some(noshape_store::local_user_name()).filter(|n| !n.is_empty());
    if suggested {
        noshape_model::drawing::mbd::place_model_datums(&mut d, doc);
    }
    d
}

/// One row of Create Drawing's Existing templates list: Onshape's file
/// name, its document and the sheet setup.
#[derive(Clone, Copy, Debug)]
pub struct TemplateRow {
    pub file: &'static str,
    pub document: &'static str,
    pub standard: &'static str,
    pub template: DrawingTemplate,
}

/// Onshape's standard templates (ISO in millimeters, first angle; ANSI in
/// inches or millimeters, third angle): (file, sheet setup name, size, inches).
const ANSI: [(&str, &str, SheetSize, bool); 12] = [
    ("ANSI_A_INCH.dwt", "ANSI A", SheetSize::AnsiA, true),
    ("ANSI_A_MM.dwt", "ANSI A", SheetSize::AnsiA, false),
    ("ANSI_A_Portrait_INCH.dwt", "ANSI A Portrait", SheetSize::AnsiAPortrait, true),
    ("ANSI_A_Portrait_MM.dwt", "ANSI A Portrait", SheetSize::AnsiAPortrait, false),
    ("ANSI_B_INCH.dwt", "ANSI B", SheetSize::AnsiB, true),
    ("ANSI_B_MM.dwt", "ANSI B", SheetSize::AnsiB, false),
    ("ANSI_C_INCH.dwt", "ANSI C", SheetSize::AnsiC, true),
    ("ANSI_C_MM.dwt", "ANSI C", SheetSize::AnsiC, false),
    ("ANSI_D_INCH.dwt", "ANSI D", SheetSize::AnsiD, true),
    ("ANSI_D_MM.dwt", "ANSI D", SheetSize::AnsiD, false),
    ("ANSI_E_INCH.dwt", "ANSI E", SheetSize::AnsiE, true),
    ("ANSI_E_MM.dwt", "ANSI E", SheetSize::AnsiE, false),
];
const ISO: [(&str, &str, SheetSize); 5] = [("ISO_A0.dwt", "ISO A0", SheetSize::A0), ("ISO_A1.dwt", "ISO A1", SheetSize::A1), ("ISO_A2.dwt", "ISO A2", SheetSize::A2), ("ISO_A3.dwt", "ISO A3", SheetSize::A3), ("ISO_A4.dwt", "ISO A4", SheetSize::A4)];

pub fn template_rows() -> Vec<TemplateRow> {
    let ansi = ANSI.iter().map(|&(file, name, size, inches)| TemplateRow {
        file,
        document: "noshape ANSI Drawing Templates",
        standard: "ANSI",
        template: DrawingTemplate::standard(name, size, TitleBlockStyle::Ansi, inches, Projection::ThirdAngle),
    });
    let iso = ISO.iter().map(|&(file, name, size)| TemplateRow {
        file,
        document: "noshape ISO Drawing Templates",
        standard: "ISO",
        template: DrawingTemplate::standard(name, size, TitleBlockStyle::Iso, false, Projection::FirstAngle),
    });
    ansi.chain(iso).collect()
}
