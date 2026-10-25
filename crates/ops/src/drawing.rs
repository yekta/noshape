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
use noshape_model::drawing::tb_lang::TitleLanguage;
use noshape_model::drawing::{AsmPosition, DimUnits, Drawing, Projection, SheetFormat, SheetSize, Simplification, TitleBlockStyle, ViewId, ZoneStart};
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

/// Sheet sizes of the Custom template tab per standard.
/// (sheet setup name, size); the size label is the name's last word.
/// Live drawing/800–825: ANSI A (default), A Portrait, B, C, D; ISO and
/// JIS A4 (default), A4 Portrait, A3, A2, A1, A0.
pub fn custom_sizes(standard: &str) -> Vec<(&'static str, SheetSize)> {
    let names: &[&'static str] = match standard {
        "ISO" => &["ISO A4", "ISO A4 Portrait", "ISO A3", "ISO A2", "ISO A1", "ISO A0"],
        "JIS" => &["JIS A4", "JIS A4 Portrait", "JIS A3", "JIS A2", "JIS A1", "JIS A0"],
        _ => &["ANSI A", "ANSI A Portrait", "ANSI B", "ANSI C", "ANSI D"],
    };
    names.iter().copied().zip(SheetSize::standard_list(standard == "ANSI").iter().copied()).collect()
}

/// The Custom template's default zones per size (live: ANSI A and A Portrait
/// 2 x 2, B and C 4 x 4, D 8 x 4; ISO / JIS A4 0 x 0, A3 6 x 4, A2 8 x 6, A1
/// 12 x 8, A0 16 x 12).
pub fn custom_zones(size: SheetSize) -> [u32; 2] {
    match size {
        SheetSize::AnsiB | SheetSize::AnsiC => [4, 4],
        s => s.standard_zones(),
    }
}

/// Custom template › Units (help H:custom_drawing_templates; "Feet and
/// inches" exists only in drawings).
pub const CUSTOM_UNITS: [(DimUnits, &str); 7] = [
    (DimUnits::Millimeter, "Millimeters"),
    (DimUnits::Centimeter, "Centimeter"),
    (DimUnits::Meter, "Meter"),
    (DimUnits::Inch, "Inches"),
    (DimUnits::FeetInches, "Feet and inches"),
    (DimUnits::Foot, "Foot"),
    (DimUnits::Yard, "Yard"),
];


/// Zone counts offered by Horizontal / Vertical zones: even numbers 0–20 (live).
pub const ZONE_COUNTS: [u32; 11] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20];

/// The Custom template tab's choices (live drawing/02).
#[derive(Clone, Copy, Debug)]
pub struct CustomTemplate {
    pub standard: &'static str,
    pub language: usize,
    pub size: usize,
    pub units: DimUnits,
    pub comma: bool,
    pub first_angle: bool,
    pub border: bool,
    pub zones: [u32; 2],
    pub top_left: bool,
    pub title_block: bool,
}

impl CustomTemplate {
    pub fn new(metric: bool) -> Self {
        let mut t = Self { standard: "ANSI", language: 2, size: 0, units: DimUnits::Inch, comma: false, first_angle: false, border: true, zones: [2, 2], top_left: false, title_block: true };
        t.set_standard(if metric { "ISO" } else { "ANSI" });
        t
    }

    /// Choosing a standard resets the size (A / A4) and its zones, the
    /// units, the decimal separator and the projection (live: ANSI Inches,
    /// Period, Third angle; ISO Millimeters, Comma, First angle; JIS
    /// Millimeters, Comma, Third angle). Border, start zones and title block
    /// stay.
    pub fn set_standard(&mut self, standard: &'static str) {
        self.standard = standard;
        let ansi = standard == "ANSI";
        self.units = if ansi { DimUnits::Inch } else { DimUnits::Millimeter };
        self.comma = !ansi;
        self.first_angle = standard == "ISO";
        self.set_size(0);
    }

    pub fn set_size(&mut self, i: usize) {
        self.size = i;
        if let Some((_, size)) = custom_sizes(self.standard).get(i) {
            self.zones = custom_zones(*size);
        }
    }

    pub fn template(&self) -> Option<DrawingTemplate> {
        let (name, size) = *custom_sizes(self.standard).get(self.size)?;
        let style = if !self.title_block {
            TitleBlockStyle::None
        } else if self.standard == "ANSI" {
            TitleBlockStyle::Ansi
        } else {
            TitleBlockStyle::Iso
        };
        let projection = if self.first_angle { Projection::FirstAngle } else { Projection::ThirdAngle };
        let mut t = DrawingTemplate::standard(name, size, style, self.units.imperial(), projection);
        if !matches!(self.units, DimUnits::Millimeter | DimUnits::Inch) {
            t.units = Some(self.units);
        }
        t.decimal_comma = self.comma;
        t.language = TitleLanguage::ALL.get(self.language).copied().unwrap_or_default();
        let zones = (self.zones != size.standard_zones()).then_some(self.zones);
        t.format = SheetFormat { border: self.border, zones, start: if self.top_left { ZoneStart::TopLeft } else { ZoneStart::BottomRight } };
        Some(t)
    }
}

// ------------------------------------------------------------ Insert view panel

/// The five View simplification choices, with Onshape-like default values.
pub fn simplification_choices(cur: Simplification) -> [Simplification; 5] {
    let keep = |d: Simplification| if std::mem::discriminant(&d) == std::mem::discriminant(&cur) { cur } else { d };
    [Simplification::None, Simplification::Auto, keep(Simplification::Absolute(0.001)), keep(Simplification::RatioToStudio(0.01)), keep(Simplification::RatioToPart(0.01))]
}

/// The value field text of a simplification ("1 mm", "1 %").
pub fn simplification_text(s: Simplification, mm: bool) -> String {
    let trim = |v: f64| {
        let t = format!("{v:.4}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    match s {
        Simplification::Absolute(m) if mm => format!("{} mm", trim(m * 1000.0)),
        Simplification::Absolute(m) => format!("{} in", trim(m / 0.0254)),
        Simplification::RatioToStudio(r) | Simplification::RatioToPart(r) => format!("{} %", trim(r * 100.0)),
        _ => String::new(),
    }
}

/// Parse the value field into `kind` (lengths in the drawing units by default).
pub fn parse_simplification(kind: Simplification, text: &str, mm: bool) -> Option<Simplification> {
    match kind {
        Simplification::Absolute(_) => {
            let prefs = noshape_units::UnitPrefs { length: if mm { noshape_units::LengthUnit::Millimeter } else { noshape_units::LengthUnit::Inch }, ..Default::default() };
            let q = noshape_units::evaluate_as(text.trim(), noshape_units::Dim::LENGTH, &prefs, &noshape_units::EmptyScope).ok()?;
            (q.value > 0.0).then_some(Simplification::Absolute(q.value))
        }
        Simplification::RatioToStudio(_) | Simplification::RatioToPart(_) => {
            let v: f64 = text.trim().trim_end_matches('%').trim().parse().ok()?;
            let r = (v / 100.0).max(0.0);
            Some(if matches!(kind, Simplification::RatioToStudio(_)) { Simplification::RatioToStudio(r) } else { Simplification::RatioToPart(r) })
        }
        other => Some(other),
    }
}

/// An assembly's Explode/Position choices in Onshape's order: Default, the
/// exploded views, then the named positions.
pub fn asm_positions(asm: &noshape_model::assembly::Assembly) -> Vec<(String, AsmPosition)> {
    let mut out = vec![("Default".to_string(), AsmPosition::Default)];
    out.extend(asm.exploded_views.iter().enumerate().map(|(i, e)| (e.name.clone(), AsmPosition::Explode(i))));
    out.extend(asm.named_positions.iter().enumerate().map(|(i, n)| (n.name.clone(), AsmPosition::Position(i))));
    out
}

/// The name shown for an Explode/Position choice.
pub fn asm_position_name(asm: &noshape_model::assembly::Assembly, p: AsmPosition) -> String {
    match p {
        AsmPosition::Default => "Default".into(),
        AsmPosition::Explode(i) => asm.exploded_views.get(i).map(|e| e.name.clone()).unwrap_or_default(),
        AsmPosition::Position(i) => asm.named_positions.get(i).map(|n| n.name.clone()).unwrap_or_default(),
    }
}

/// The named views a source offers in View orientation (Part Studios).
pub fn named_views(doc: &noshape_model::document::Document, source: TabId) -> &[noshape_model::partstudio::NamedView] {
    match doc.tab(source).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => &ps.named_views,
        _ => &[],
    }
}

/// The frame of the source's named view `name`, if it still exists.
pub fn named_view_frame(doc: &noshape_model::document::Document, source: TabId, name: &str) -> Option<[[f64; 3]; 2]> {
    named_views(doc, source).iter().find(|v| v.name == name).map(noshape_model::drawing::views::named_view_frame)
}

// ------------------------------------------------------------ View properties

/// A typed scale: "1:2", "2/1" or a number.
pub fn parse_scale(s: &str) -> Option<f64> {
    let s = s.trim();
    let v = if let Some((a, b)) = s.split_once([':', '/']) {
        a.trim().parse::<f64>().ok()? / b.trim().parse::<f64>().ok()?
    } else {
        s.parse::<f64>().ok()?
    };
    (v.is_finite() && v > 0.0).then_some(v)
}


/// The View properties dialog's values for one view (live drawing/177).
#[derive(Clone, Debug, PartialEq)]
pub struct ViewPropValues {
    /// Base views: View orientation (a standard one) …
    pub orientation: Option<noshape_model::drawing::ViewOrientation>,
    /// … or a named view of the source.
    pub named: Option<String>,
    /// The view's own scale (None: inherited).
    pub scale: Option<f64>,
    /// Inherited from the sheet ("(Sheet)") rather than the parent.
    pub sheet_scale: bool,
    /// Rotation angle, degrees counter-clockwise (None: unchanged).
    pub rotation_deg: Option<f64>,
    pub tangent: noshape_scene::drawing_tools::view_edit::TangentMode,
    pub best_quality: bool,
    pub asm_position: Option<AsmPosition>,
    pub display: Option<noshape_model::drawing::DisplayChoice>,
    pub unaligned: bool,
    pub cut_from_section: bool,
    pub sketch_appearances: bool,
    pub model_annotations: bool,
    pub simplification: Option<Simplification>,
    pub name: String,
    pub view_label: bool,
    pub label: String,
    pub scale_label: bool,
}

impl ViewPropValues {
    /// The dialog as it opens on view `id` of `sheet`.
    pub fn of(d: &Drawing, sheet: usize, id: ViewId) -> Option<Self> {
        use noshape_model::drawing::{DisplayChoice, ViewKind};
        let s = d.sheets.get(sheet)?;
        let v = s.view(id)?;
        let base = matches!(v.kind, ViewKind::Base { .. });
        let auto_label = matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. });
        Some(ViewPropValues {
            orientation: match &v.kind {
                ViewKind::Base { orientation, .. } => Some(*orientation),
                _ => None,
            },
            named: v.named_view.clone(),
            scale: v.scale,
            sheet_scale: v.sheet_scale,
            rotation_deg: None,
            tangent: noshape_scene::drawing_tools::view_edit::TangentMode::of(v),
            best_quality: v.best_quality,
            asm_position: if base { Some(v.asm_position()) } else { v.own_position },
            display: if base { Some(v.display_state.clone().map_or(DisplayChoice::ShowAll, DisplayChoice::State)) } else { v.own_display.clone() },
            unaligned: v.unaligned,
            cut_from_section: v.cut_from_section,
            sketch_appearances: v.sketch_appearances,
            model_annotations: v.model_annotations,
            simplification: v.simplification,
            name: v.name.clone().unwrap_or_else(|| noshape_scene::drawing_tools::view_edit::default_view_name(d, s, v)),
            view_label: auto_label || v.label.as_ref().is_some_and(|l| !l.is_empty()),
            label: match &v.kind {
                ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } | ViewKind::Detail { label, .. } => label.clone(),
                _ => v.label.clone().unwrap_or_default(),
            },
            scale_label: v.scale_label.unwrap_or(v.label.is_some() && v.scale.is_some()),
        })
    }
}

/// View properties ✓ on one view (the sheet move is the caller's).
pub fn apply_view_props(doc: &Document, d: &mut Drawing, sheet: usize, id: ViewId, p: &ViewPropValues) {
    use noshape_model::drawing::{DisplayChoice, ViewKind};
    use noshape_scene::drawing_tools::view_edit::{default_view_name, set_standard_orientation};
    let Some(old) = d.sheets.get(sheet).and_then(|s| s.view(id)).cloned() else { return };
    let default_name = default_view_name(d, &d.sheets[sheet], &old);
    let Some(v) = d.sheets[sheet].view_mut(id) else { return };
    v.scale = p.scale;
    v.sheet_scale = p.sheet_scale && p.scale.is_none();
    if let Some(r) = p.rotation_deg {
        v.rotation = r.to_radians();
    }
    p.tangent.apply(v);
    v.best_quality = p.best_quality;
    if matches!(v.kind, ViewKind::Base { .. }) {
        v.set_asm_position(p.asm_position.unwrap_or(AsmPosition::Default));
        v.display_state = match &p.display {
            Some(DisplayChoice::State(n)) => Some(n.clone()),
            _ => None,
        };
    } else if !matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. } | ViewKind::Detail { .. }) {
        v.own_position = p.asm_position;
        v.own_display = p.display.clone();
    }
    v.unaligned = p.unaligned;
    v.cut_from_section = p.cut_from_section;
    v.sketch_appearances = p.sketch_appearances;
    v.model_annotations = p.model_annotations;
    v.simplification = p.simplification;
    let name = p.name.trim();
    v.name = (!name.is_empty() && name != default_name).then(|| name.to_string());
    v.scale_label = Some(p.scale_label);
    let label = p.label.trim().to_string();
    match &mut v.kind {
        ViewKind::Section { label: l, .. } | ViewKind::AlignedSection { label: l, .. } | ViewKind::Detail { label: l, .. } => {
            if !label.is_empty() {
                *l = label;
            }
        }
        _ => v.label = (p.view_label && !label.is_empty()).then_some(label).or_else(|| p.view_label.then(|| v.name.clone().unwrap_or(default_name.clone()).to_uppercase())),
    }
    if let (ViewKind::Base { orientation: o, source, .. }, Some(n)) = (&mut v.kind, p.orientation) {
        let source = *source;
        if p.named != old.named_view {
            match p.named.as_ref().and_then(|name| named_view_frame(doc, source, name)) {
                Some(frame) => {
                    v.custom_frame = Some(frame);
                    v.named_view = p.named.clone();
                }
                None => set_standard_orientation(v, n),
            }
        } else if *o != n {
            set_standard_orientation(v, n);
        }
    }
}

// ------------------------------------------------------------ dimension palette

/// The palette's precision list (label, decimals; None: the drawing's).
pub const PRECISIONS: [(&str, Option<u8>); 10] = [("0", Some(0)), ("0.1", Some(1)), ("0.12", Some(2)), ("0.123", Some(3)), ("(Drawing)", None), ("0.1234", Some(4)), ("0.12345", Some(5)), ("0.123456", Some(6)), ("0.1234567", Some(7)), ("0.12345678", Some(8))];
/// Tolerance lists (D35; hole callouts have no Basic).
pub const DIM_TOLERANCES: [&str; 10] = ["None", "Symmetrical", "Deviation", "Limits", "Basic", "MIN", "MAX", "Fit", "Fit with tolerance", "Fit (Tolerance only)"];
pub const HOLE_TOLERANCES: [&str; 9] = ["None", "Symmetrical", "Deviation", "Limits", "MIN", "MAX", "Fit", "Fit with tolerance", "Fit (Tolerance only)"];

/// The tolerance list entry of an annotation's format.
pub fn tolerance_name(a: &noshape_model::drawing::Annotation) -> &'static str {
    use noshape_model::drawing::Tolerance;
    match &a.format.tolerance {
        Tolerance::None => "None",
        Tolerance::Symmetric { .. } => "Symmetrical",
        Tolerance::Deviation { .. } => "Deviation",
        Tolerance::Limits { .. } => "Limits",
        Tolerance::Basic => "Basic",
        Tolerance::Min => "MIN",
        Tolerance::Max => "MAX",
        Tolerance::Fit { .. } => a.format.fit.mode.name(),
    }
}

/// The palette's tolerance entry `name` with its two value fields (meters:
/// Symmetrical `a`; Deviation / Limits upper `a`, lower `b`). Fits keep
/// their classes (H7 for a new one).
pub fn set_tolerance(f: &mut noshape_model::drawing::DimFormat, name: &str, a: f64, b: f64) {
    use noshape_model::drawing::{FitMode, Tolerance};
    let classes = match &f.tolerance {
        Tolerance::Fit { hole, shaft } => (hole.clone(), shaft.clone()),
        _ => ("H7".into(), String::new()),
    };
    f.tolerance = match name {
        "Symmetrical" => Tolerance::Symmetric { value: a },
        "Deviation" => Tolerance::Deviation { upper: a, lower: b },
        "Limits" => Tolerance::Limits { upper: a, lower: b },
        "Basic" => Tolerance::Basic,
        "MIN" => Tolerance::Min,
        "MAX" => Tolerance::Max,
        "Fit" | "Fit with tolerance" | "Fit (Tolerance only)" => {
            f.fit.mode = match name {
                "Fit with tolerance" => FitMode::WithTolerance,
                "Fit (Tolerance only)" => FitMode::ToleranceOnly,
                _ => FitMode::Fit,
            };
            Tolerance::Fit { hole: classes.0, shaft: classes.1 }
        }
        _ => Tolerance::None,
    };
}

