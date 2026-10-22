//! Drawing view geometry: orientations and projection chains, hidden-line
//! projection of Part Studio bodies (via the kernel's HLR), section cuts,
//! detail crops, snap points and associative point references.
//!
//! Coordinates: every view has its own 2D *view coordinates* in meters
//! (x right, y up in the view plane, i.e. the HLR output frame). A view's
//! placement maps view coordinates to sheet millimeters (see
//! [`super::build::ViewPlacement`]); annotations store view coordinates.

use super::{BrokenOut, Crop, Drawing, PointRef, Projection, Sheet, Simplification, ViewId, ViewKind, ViewOrientation};
use crate::assembly::eval::{AssemblyEvaluator, AssemblyResult, SolveOptions};
use crate::configuration::ConfigValues;
use crate::display::BodyKind;
use crate::document::{Document, TabContent, TabId};
use crate::naming::Name;
use crate::partstudio::PartStudio;
use crate::reference::BodyId;
use crate::regen::context::{Body, Context};
use crate::regen::{RegenOptions, Regenerator};
use glam::{DQuat, DVec2, DVec3};
use noshape_kernel::{Curve, EdgeClass, Plane, Segment2d, Shape, ShapeType, Surface, Visibility};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// ------------------------------------------------------------ sources

/// What a base view shows: a Part Studio or an Assembly tab in a given
/// configuration (and, for assemblies, exploded view).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceKey {
    pub tab: TabId,
    pub config: ConfigValues,
    pub explode: Option<usize>,
    /// Pinned to a version (its id as text): the source comes from the
    /// document at that version; None: the live workspace.
    pub version: Option<String>,
    /// Assemblies: a named position (index) instead of the solved assembly.
    pub position: Option<usize>,
    /// Assemblies: a display state (by name); None: Show all.
    pub display: Option<String>,
}

impl SourceKey {
    pub fn studio(tab: TabId) -> Self {
        Self { tab, config: ConfigValues::default(), explode: None, version: None, position: None, display: None }
    }

    /// The source of a base view (and its part), with its pinned version.
    pub fn of_view(v: &super::DrawingView) -> Option<(Self, Option<BodyId>)> {
        match &v.kind {
            ViewKind::Base { source, config, part, .. } => {
                Some((Self { tab: *source, config: config.clone(), explode: v.explode, version: v.pinned.as_ref().map(|p| p.id.clone()), position: v.named_position, display: v.display_state.clone() }, part.clone()))
            }
            _ => None,
        }
    }
}

/// Properties of one part of a source (for property fields and the BOM).
#[derive(Clone, Debug, Default)]
pub struct PartInfo {
    pub body: Option<BodyId>,
    pub name: String,
    pub number: String,
    pub material: String,
    pub description: String,
    /// Density of the part's material (kg/m³), None without a material.
    pub density: Option<f64>,
    /// Properties › Revision and the other metadata (State, Vendor, …).
    pub revision: String,
    pub meta: crate::partstudio::ItemMeta,
    /// Mass properties › Override (kg).
    pub mass_override: Option<f64>,
    pub tessellation: Option<crate::partstudio::TessQuality>,
}

/// One bill-of-materials row of an assembly source.
#[derive(Clone, Debug)]
pub struct BomEntry {
    pub item: usize,
    pub name: String,
    pub number: String,
    pub material: String,
    pub quantity: usize,
    /// Bodies of the source context (one per occurrence) of this row.
    pub bodies: Vec<BodyId>,
}

/// A regenerated Part Studio, or an evaluated Assembly flattened into one
/// context of transformed bodies (one per part occurrence), referenced by
/// drawing views.
#[derive(Clone)]
pub struct SourceModel {
    /// Content hash of the source (+ configuration); keys the HLR cache.
    pub hash: u64,
    pub context: Arc<Context>,
    /// Tab name.
    pub name: String,
    pub assembly: bool,
    /// Per body of `context`.
    pub parts: Vec<PartInfo>,
    /// Assemblies only.
    pub bom: Vec<BomEntry>,
    /// Assemblies only: the Structured BOM (item "3.1", depth, name,
    /// part number, material, quantity).
    pub bom_structured: Vec<StructuredBomRow>,
    /// Part Studios: their MBD annotations (Model definitions datums).
    pub model_annotations: Vec<crate::annotations::Annotation>,
    /// Assemblies: the assembly's own Properties (name, part number, …),
    /// which an assembly drawing's part property fields show.
    pub own: Option<PartInfo>,
}

/// One row of a Structured BOM.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StructuredBomRow {
    pub item: String,
    pub depth: usize,
    pub name: String,
    pub number: String,
    pub material: String,
    pub quantity: usize,
}

/// The Structured BOM rows of an assembly tab.
fn structured_bom(doc: &Document, tab: crate::document::TabId, result: &AssemblyResult) -> Vec<StructuredBomRow> {
    use crate::assembly::bom::{BomCell, BomColumn, BomSettings, BomType, cell, lines};
    let settings = BomSettings { bom_type: BomType::Structured, ..Default::default() };
    let text = |c: BomCell| match c {
        BomCell::Text(t) => t,
        _ => String::new(),
    };
    lines(doc, tab, result, &settings)
        .iter()
        .filter(|l| !l.excluded && !l.top_level)
        .map(|l| StructuredBomRow {
            item: l.item.clone().unwrap_or_default(),
            depth: l.depth,
            name: l.name.clone(),
            number: text(cell(doc, result, l, BomColumn::PartNumber)),
            material: text(cell(doc, result, l, BomColumn::Material)),
            quantity: l.quantity,
        })
        .collect()
}

impl SourceModel {
    pub fn part(&self, body: Option<&BodyId>) -> Option<&PartInfo> {
        match body {
            Some(b) => self.parts.iter().find(|p| p.body.as_ref() == Some(b)),
            None => self.parts.first(),
        }
    }
    /// BOM item number of a body of the source context.
    pub fn bom_item(&self, body: &BodyId) -> Option<usize> {
        self.bom.iter().find(|r| r.bodies.contains(body)).map(|r| r.item)
    }
}

pub type Sources = HashMap<SourceKey, SourceModel>;

/// Content hash of a Part Studio definition and configuration.
pub fn studio_hash(studio: &PartStudio, config: &ConfigValues) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(studio).unwrap_or_default().hash(&mut h);
    config.hash(&mut h);
    h.finish()
}

/// Content hash of a source: the studio, or the assembly with every Part
/// Studio and sub-assembly of the document (assemblies reference them).
pub fn source_hash(doc: &Document, key: &SourceKey) -> Option<u64> {
    match &doc.tab(key.tab)?.content {
        TabContent::PartStudio(ps) => Some(studio_hash(ps, &key.config)),
        TabContent::Assembly(asm) => {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_vec(asm).unwrap_or_default().hash(&mut h);
            key.explode.hash(&mut h);
            key.position.hash(&mut h);
            key.display.hash(&mut h);
            for t in &doc.tabs {
                match &t.content {
                    TabContent::PartStudio(ps) => {
                        t.id.hash(&mut h);
                        serde_json::to_vec(ps).unwrap_or_default().hash(&mut h);
                    }
                    TabContent::Assembly(a) if t.id != key.tab => {
                        t.id.hash(&mut h);
                        serde_json::to_vec(a).unwrap_or_default().hash(&mut h);
                    }
                    _ => {}
                }
            }
            serde_json::to_vec(&doc.settings.units).unwrap_or_default().hash(&mut h);
            Some(h.finish())
        }
        _ => None,
    }
}

/// Every source (tab + configuration + explode) used by a drawing's base
/// views, in first-use order.
/// Version ids the document's drawings pin view references to.
pub fn pinned_versions(doc: &Document) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in &doc.tabs {
        if let crate::document::TabContent::Drawing(d) = &t.content {
            for id in d.sheets.iter().flat_map(|s| &s.views).filter_map(|v| v.pinned.as_ref().map(|p| &p.id)) {
                if !out.contains(id) {
                    out.push(id.clone());
                }
            }
        }
    }
    out
}

pub fn referenced_sources(drawing: &Drawing) -> Vec<SourceKey> {
    let mut out: Vec<SourceKey> = Vec::new();
    for sheet in &drawing.sheets {
        for v in &sheet.views {
            // Child views with their own assembly state need their own source.
            if let Some((k, _)) = view_source(sheet, v.id)
                && !out.contains(&k)
            {
                out.push(k);
            }
        }
    }
    // Custom tables tabulate a Part Studio the views need not show.
    for k in super::tables::custom_table_keys(drawing) {
        if !out.contains(&k.source) {
            out.push(k.source);
        }
    }
    out
}

/// Tabs referenced by a drawing (with the configuration of the first view
/// that uses each).
pub fn referenced_studios(drawing: &Drawing) -> Vec<(TabId, ConfigValues)> {
    let mut out: Vec<(TabId, ConfigValues)> = Vec::new();
    for k in referenced_sources(drawing) {
        if !out.iter().any(|(t, _)| *t == k.tab) {
            out.push((k.tab, k.config));
        }
    }
    out
}

fn studio_of(doc: &Document, tab: TabId) -> Option<&PartStudio> {
    match &doc.tab(tab)?.content {
        TabContent::PartStudio(ps) => Some(ps),
        _ => None,
    }
}

fn part_info(ps: &PartStudio, b: &Body) -> PartInfo {
    let props = ps.part_properties.get(&b.id);
    let get = |f: fn(&crate::partstudio::PartProperties) -> &Option<String>| props.and_then(|p| f(p).clone()).unwrap_or_default();
    PartInfo {
        body: Some(b.id.clone()),
        name: props.and_then(|p| p.name.clone()).unwrap_or_else(|| b.name.clone()),
        number: get(|p| &p.part_number),
        material: get(|p| &p.material),
        description: get(|p| &p.description),
        density: props.and_then(|p| p.material.as_ref().and(p.density)),
        revision: get(|p| &p.revision),
        meta: props.map(|p| p.meta.clone()).unwrap_or_default(),
        mass_override: props.and_then(|p| p.mass_override),
        tessellation: props.and_then(|p| p.tessellation),
    }
}

/// Regenerate / evaluate one source. `regen` is reused for Part Studios
/// (incremental), `assemblies` caches studio regenerations for assemblies.
pub fn regenerate_source(doc: &Document, key: &SourceKey, regen: &mut Regenerator, assemblies: &mut AssemblyEvaluator) -> Option<SourceModel> {
    let tab = doc.tab(key.tab)?;
    let hash = source_hash(doc, key)?;
    match &tab.content {
        TabContent::PartStudio(ps) => {
            let has_scripts = doc.tabs.iter().any(|t| matches!(t.content, TabContent::FeatureStudio(_)));
            let opts = RegenOptions { custom_features: has_scripts.then(|| crate::custom::DocumentScripts::from_document(doc) as Arc<dyn crate::custom::CustomFeatureProvider>), ..Default::default() };
            let r = regen.regenerate(ps, &key.config, &doc.settings.units, &opts);
            let parts = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| part_info(ps, b)).collect();
            Some(SourceModel { hash, context: r.context, name: tab.name.clone(), assembly: false, parts, bom: vec![], bom_structured: vec![], model_annotations: ps.annotations.clone(), own: None })
        }
        TabContent::Assembly(asm) => {
            // A named position: its placements and mate values; a display
            // state that no longer exists can't be resolved (the view fails).
            let opts = match key.position.and_then(|i| asm.named_positions.get(i)) {
                Some(np) => SolveOptions { start: Some(np.placements.iter().cloned().collect()), locks: np.mate_values.clone(), ..Default::default() },
                None => SolveOptions::default(),
            };
            let hidden = match &key.display {
                Some(name) => Some(asm.display_states.iter().find(|d| &d.name == name)?.hidden_instances.clone()),
                None => None,
            };
            let result = assemblies.evaluate(doc, asm, &opts, key.explode);
            let mut s = assembly_source(doc, &result, hash, tab.name.clone(), hidden.as_deref());
            s.bom_structured = structured_bom(doc, key.tab, &result);
            let p = &asm.properties;
            s.own = Some(PartInfo { name: tab.name.clone(), number: p.part_number.clone().unwrap_or_default(), description: p.description.clone().unwrap_or_default(), revision: p.revision.clone().unwrap_or_default(), meta: p.meta.clone(), mass_override: p.mass_override, ..Default::default() });
            Some(s)
        }
        _ => None,
    }
}

/// Flatten an evaluated assembly into a context of transformed bodies.
/// Occurrence `i` gets the body id `{feature: original feature, index:
/// (i + 1) << 16 | original index}` and keeps the original face, edge and
/// vertex names, so annotation references resolve like in Part Studios.
/// `hidden`: the display state's hidden instances (None: Show all, every
/// occurrence whatever the assembly currently hides).
pub fn assembly_source(doc: &Document, result: &AssemblyResult, hash: u64, name: String, hidden: Option<&[crate::assembly::InstanceId]>) -> SourceModel {
    let mut ctx = Context::default();
    let mut parts = Vec::new();
    let mut occ_body: Vec<(usize, BodyId)> = Vec::new();
    for (i, o) in result.occurrences.iter().enumerate() {
        if o.kind != BodyKind::Solid || hidden.is_some_and(|h| o.path.iter().any(|p| h.contains(p))) {
            continue;
        }
        let Some(b) = o.context.body(&o.body) else { continue };
        let Ok(shape) = noshape_kernel::transform(&b.shape, &noshape_kernel::Transform::Matrix(o.transform), false).map(|r| r.shape) else { continue };
        let id = BodyId { feature: b.id.feature, index: ((i as u32 + 1) << 16) | (b.id.index & 0xFFFF) };
        let nb = Body::new(id.clone(), o.instance_name.clone(), BodyKind::Solid, shape, b.faces.clone(), b.color, b.id.feature);
        let mut info = match studio_of(doc, o.studio) {
            Some(ps) => part_info(ps, b),
            None => PartInfo { name: o.body_name.clone(), ..Default::default() },
        };
        info.body = Some(id.clone());
        parts.push(info);
        occ_body.push((i, id));
        ctx.bodies.push(Arc::new(nb));
    }
    // Rows merge like the assembly's BOM: the same part in the same
    // configuration (values compared as numbers) is one row (qa5 D18).
    let canon = |studio, config: &ConfigValues| crate::assembly::bom::canonical_config(doc, studio, config);
    let mut rows: Vec<crate::assembly::analysis::BomRow> = Vec::new();
    for row in crate::assembly::analysis::bom(result) {
        let config = canon(row.studio, &row.config);
        if let Some(r) = rows.iter_mut().find(|r| r.studio == row.studio && r.body == row.body && r.config == config && r.mirrored == row.mirrored && r.external == row.external) {
            r.quantity += row.quantity;
            r.instances.extend(row.instances);
        } else {
            rows.push(crate::assembly::analysis::BomRow { item: rows.len() + 1, config, ..row });
        }
    }
    let mut bom = Vec::new();
    for row in &rows {
        let bodies: Vec<BodyId> = occ_body
            .iter()
            .filter(|(i, _)| {
                let o = &result.occurrences[*i];
                o.studio == row.studio && o.body == row.body && o.mirrored == row.mirrored && o.external == row.external && canon(o.studio, &o.config) == row.config
            })
            .map(|(_, id)| id.clone())
            .collect();
        let info = bodies.first().and_then(|b| parts.iter().find(|p: &&PartInfo| p.body.as_ref() == Some(b)));
        bom.push(BomEntry {
            item: row.item,
            name: info.map(|p| p.name.clone()).filter(|n| !n.is_empty()).unwrap_or(row.name.clone()),
            number: info.map(|p| p.number.clone()).unwrap_or_default(),
            material: info.map(|p| p.material.clone()).unwrap_or_default(),
            quantity: row.quantity,
            bodies,
        });
    }
    // Part names in BOM rows come from the Part Studio (not instance names).
    for (row, r) in bom.iter_mut().zip(&rows) {
        if let Some(ps) = studio_of(doc, r.studio)
            && let Some(n) = ps.part_properties.get(&r.body).and_then(|p| p.name.clone())
        {
            row.name = n;
        } else {
            row.name = r.name.clone();
        }
    }
    SourceModel { hash, context: Arc::new(ctx), name, assembly: true, parts, bom, bom_structured: Vec::new(), model_annotations: Vec::new(), own: None }
}

/// Regenerate every source a drawing references (headless helper for
/// tests, export and the demo; the app regenerates in the background).
pub fn regenerate_sources(doc: &Document, drawing: &Drawing) -> Sources {
    regenerate_sources_with(doc, drawing, &|_| None)
}

/// [`regenerate_sources`] with pinned sources taken from the document at
/// their version (`versions(id)`; sources whose version can't be loaded
/// are left out).
pub fn regenerate_sources_with(doc: &Document, drawing: &Drawing, versions: &dyn Fn(&str) -> Option<Document>) -> Sources {
    let mut out = Sources::new();
    let mut assemblies = AssemblyEvaluator::new();
    for key in referenced_sources(drawing) {
        let mut reg = Regenerator::new();
        let at = key.version.as_deref().map(versions);
        let d = match &at {
            Some(Some(d)) => d,
            Some(None) => continue,
            None => doc,
        };
        if let Some(src) = regenerate_source(d, &key, &mut reg, &mut assemblies) {
            out.insert(key, src);
        }
    }
    out
}

// ------------------------------------------------------------ frames

/// Orthonormal view frame: `dir` points from the eye into the scene.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewFrame {
    pub dir: DVec3,
    pub up: DVec3,
    pub right: DVec3,
}

impl ViewFrame {
    /// Same convention as the kernel HLR projector.
    pub fn new(dir: DVec3, up: DVec3) -> ViewFrame {
        let dir = dir.normalize();
        let z = -dir;
        let mut y = up - z * up.dot(z);
        if y.length_squared() < 1e-18 {
            y = z.any_orthonormal_vector();
        }
        let y = y.normalize();
        let x = y.cross(z);
        ViewFrame { dir, up: y, right: x }
    }

    pub fn project(&self, p: DVec3) -> [f64; 2] {
        [p.dot(self.right), p.dot(self.up)]
    }

    /// 3D direction of a 2D view-plane direction.
    pub fn lift(&self, v: [f64; 2]) -> DVec3 {
        self.right * v[0] + self.up * v[1]
    }

    fn key(&self, h: &mut impl Hasher) {
        for v in [self.dir, self.up] {
            for c in v.to_array() {
                ((c * 1e9).round() as i64).hash(h);
            }
        }
    }
}

pub fn orientation_frame(o: ViewOrientation) -> ViewFrame {
    let (dir, up) = match o {
        ViewOrientation::Front => (DVec3::Y, DVec3::Z),
        ViewOrientation::Back => (DVec3::NEG_Y, DVec3::Z),
        ViewOrientation::Top => (DVec3::NEG_Z, DVec3::Y),
        ViewOrientation::Bottom => (DVec3::Z, DVec3::NEG_Y),
        ViewOrientation::Left => (DVec3::X, DVec3::Z),
        ViewOrientation::Right => (DVec3::NEG_X, DVec3::Z),
        ViewOrientation::Isometric => (DVec3::new(-1.0, 1.0, -1.0), DVec3::Z),
    };
    ViewFrame::new(dir, up)
}

/// The view frame (viewing direction, up) of a Part Studio named view: its
/// camera's line of sight and up direction.
pub fn named_view_frame(nv: &crate::partstudio::NamedView) -> [[f64; 3]; 2] {
    let q = DQuat::from_array(nv.rotation).normalize();
    [(q * DVec3::NEG_Z).to_array(), (q * DVec3::Y).to_array()]
}

/// Snap a sheet direction to the nearest multiple of 45 degrees.
pub fn snap_direction(d: [f64; 2]) -> [f64; 2] {
    let a = d[1].atan2(d[0]);
    let k = (a / std::f64::consts::FRAC_PI_4).round();
    let a = k * std::f64::consts::FRAC_PI_4;
    let (s, c) = a.sin_cos();
    let r = |x: f64| if x.abs() < 1e-9 { 0.0 } else { x };
    [r(c), r(s)]
}

/// Frame of a view projected from `parent` and placed in sheet direction `m`.
/// Axis directions fold the view 90 degrees (third angle: the view shows the
/// side facing its placement; first angle: the opposite side); diagonal
/// directions give an isometric view.
pub fn fold(parent: &ViewFrame, m: [f64; 2], projection: Projection) -> ViewFrame {
    let l = (m[0] * m[0] + m[1] * m[1]).sqrt().max(1e-12);
    let m = [m[0] / l, m[1] / l];
    let sign = match projection {
        Projection::ThirdAngle => -1.0,
        Projection::FirstAngle => 1.0,
    };
    if m[0].abs() > 0.38 && m[1].abs() > 0.38 {
        let dir = parent.dir + (parent.right * m[0].signum() + parent.up * m[1].signum()) * sign;
        return ViewFrame::new(dir, parent.up);
    }
    fold_toward(parent, parent.lift(m) * sign)
}

/// Rotate the frame 90 degrees so that its viewing direction becomes `t`
/// (a unit vector in the parent's view plane).
fn fold_toward(parent: &ViewFrame, t: DVec3) -> ViewFrame {
    let t = t.normalize();
    let d = parent.dir;
    let rot = |v: DVec3| v - d * v.dot(d) - t * v.dot(t) + t * v.dot(d) - d * v.dot(t);
    ViewFrame::new(t, rot(parent.up))
}

fn view_center(sheet: &Sheet, id: ViewId) -> Option<[f64; 2]> {
    sheet.view(id).map(|v| v.position)
}

/// Rotate a 2D vector counter-clockwise by `a` radians.
pub fn rotate2(v: [f64; 2], a: f64) -> [f64; 2] {
    if a == 0.0 {
        return v;
    }
    let (s, c) = a.sin_cos();
    [v[0] * c - v[1] * s, v[0] * s + v[1] * c]
}

/// Rotation of a view on the sheet (radians). Detail views follow their
/// parent's rotation (they crop the parent's geometry).
pub fn view_rotation(sheet: &Sheet, id: ViewId) -> f64 {
    let mut cur = id;
    let mut a = 0.0;
    for _ in 0..64 {
        let Some(v) = sheet.view(cur) else { break };
        a += v.rotation;
        match &v.kind {
            ViewKind::Detail { parent, .. } => cur = *parent,
            _ => break,
        }
    }
    a
}

/// Sheet offset of `child` from `parent`, in the parent's (unrotated) view
/// frame.
fn offset_in_parent(sheet: &Sheet, child: ViewId, parent: ViewId) -> Option<[f64; 2]> {
    let (c, p) = (view_center(sheet, child)?, view_center(sheet, parent)?);
    Some(rotate2([c[0] - p[0], c[1] - p[1]], -view_rotation(sheet, parent)))
}

fn side_of(sheet: &Sheet, child: ViewId, parent: ViewId, n: [f64; 2]) -> f64 {
    match offset_in_parent(sheet, child, parent) {
        Some(d) if d[0] * n[0] + d[1] * n[1] < 0.0 => -1.0,
        _ => 1.0,
    }
}

/// An auxiliary view's side of its edge (parent view coordinates, unit):
/// the reference indicator sits there and looks back at the edge.
pub fn auxiliary_toward(sheet: &Sheet, id: ViewId) -> Option<[f64; 2]> {
    let v = sheet.view(id)?;
    let ViewKind::Auxiliary { parent, edge_dir } = &v.kind else { return None };
    let l = (edge_dir[0].powi(2) + edge_dir[1].powi(2)).sqrt().max(1e-12);
    let n = [-edge_dir[1] / l, edge_dir[0] / l];
    let s = side_of(sheet, id, *parent, n) * if v.flip { -1.0 } else { 1.0 };
    Some([n[0] * s, n[1] * s])
}

/// The direction from parent to child (in the parent's view frame) used
/// for projected views.
pub fn projected_direction(sheet: &Sheet, id: ViewId) -> Option<[f64; 2]> {
    let v = sheet.view(id)?;
    let ViewKind::Projected { parent, direction } = &v.kind else { return None };
    if let Some(d) = direction {
        return Some(snap_direction(*d));
    }
    let d = offset_in_parent(sheet, id, *parent)?;
    if d[0].abs() + d[1].abs() < 1e-9 {
        return Some([1.0, 0.0]);
    }
    Some(snap_direction(d))
}

/// Section cut data in the parent's view coordinates.
#[derive(Clone, Debug)]
pub struct SectionCut {
    /// The cutting line (2 points, more for a jogged line; `a`, center, `b`
    /// for an aligned section).
    pub points: Vec<[f64; 2]>,
    pub p0: [f64; 2],
    pub p1: [f64; 2],
    /// Unit view-plane direction (perpendicular to the first leg) pointing
    /// toward the section view (the removed side); the line of sight is the
    /// opposite direction.
    pub toward_view: [f64; 2],
    /// Partial section (see [`super::DrawingView::partial`]).
    pub partial: bool,
    /// "Show cut geometry only" and "Section depth" (Blind, meters, or Up
    /// to entity: the point, resolved with the model).
    pub cut_only: bool,
    pub depth: Option<f64>,
    pub depth_ref: Option<PointRef>,
    /// "Exclude from cut": parts left whole.
    pub exclude: Vec<BodyId>,
}

pub fn section_cut(sheet: &Sheet, id: ViewId) -> Option<SectionCut> {
    let v = sheet.view(id)?;
    let (parent, points) = match &v.kind {
        ViewKind::Section { parent, line, .. } => (*parent, line.clone()),
        ViewKind::AlignedSection { parent, center, a, b, .. } => (*parent, vec![*a, *center, *b]),
        _ => return None,
    };
    if points.len() < 2 {
        return None;
    }
    let (p0, p1) = (points[0], points[points.len() - 1]);
    let e = [points[1][0] - p0[0], points[1][1] - p0[1]];
    let l = (e[0] * e[0] + e[1] * e[1]).sqrt();
    if l < 1e-12 {
        return None;
    }
    let n = [-e[1] / l, e[0] / l];
    let s = side_of(sheet, id, parent, n) * if v.flip { -1.0 } else { 1.0 };
    let section = matches!(v.kind, ViewKind::Section { .. });
    Some(SectionCut { points, p0, p1, toward_view: [n[0] * s, n[1] * s], partial: v.partial && section, cut_only: v.cut_only, depth: v.section_depth.filter(|d| *d > 0.0), depth_ref: v.section_depth_ref.clone(), exclude: v.exclude_from_cut.clone() })
}

/// The frame of any view on a sheet (follows the parent chain).
/// The standard orthographic orientation a view looks along, if any
/// (Onshape names projected views "Right", "Top"... by it).
pub fn standard_orientation(drawing: &Drawing, sheet: &Sheet, id: ViewId) -> Option<ViewOrientation> {
    let f = view_frame(drawing, sheet, id)?;
    let dir = f.dir.normalize_or_zero();
    [ViewOrientation::Front, ViewOrientation::Back, ViewOrientation::Top, ViewOrientation::Bottom, ViewOrientation::Left, ViewOrientation::Right]
        .into_iter()
        .find(|o| orientation_frame(*o).dir.normalize().dot(dir) > 1.0 - 1e-9)
}

pub fn view_frame(drawing: &Drawing, sheet: &Sheet, id: ViewId) -> Option<ViewFrame> {
    view_frame_depth(drawing, sheet, id, 0)
}

fn view_frame_depth(drawing: &Drawing, sheet: &Sheet, id: ViewId, depth: usize) -> Option<ViewFrame> {
    if depth > 32 {
        return None;
    }
    let v = sheet.view(id)?;
    match &v.kind {
        ViewKind::Base { orientation, .. } => Some(match v.custom_frame {
            Some([d, u]) => ViewFrame::new(DVec3::from_array(d), DVec3::from_array(u)),
            None => orientation_frame(*orientation),
        }),
        ViewKind::Projected { parent, .. } => {
            let pf = view_frame_depth(drawing, sheet, *parent, depth + 1)?;
            Some(fold(&pf, projected_direction(sheet, id)?, drawing.projection))
        }
        ViewKind::Auxiliary { parent, edge_dir } => {
            let pf = view_frame_depth(drawing, sheet, *parent, depth + 1)?;
            let l = (edge_dir[0].powi(2) + edge_dir[1].powi(2)).sqrt().max(1e-12);
            let n = [-edge_dir[1] / l, edge_dir[0] / l];
            let s = side_of(sheet, id, *parent, n) * if sheet.view(id).is_some_and(|v| v.flip) { -1.0 } else { 1.0 };
            let sign = if drawing.projection == Projection::ThirdAngle { -1.0 } else { 1.0 };
            Some(fold_toward(&pf, pf.lift([n[0] * s, n[1] * s]) * sign))
        }
        ViewKind::Section { parent, .. } | ViewKind::AlignedSection { parent, .. } => {
            let pf = view_frame_depth(drawing, sheet, *parent, depth + 1)?;
            let cut = section_cut(sheet, id)?;
            // Line of sight is away from the view's placement side.
            Some(fold_toward(&pf, -pf.lift(cut.toward_view)))
        }
        ViewKind::Detail { parent, .. } => view_frame_depth(drawing, sheet, *parent, depth + 1),
    }
}

/// The root base view of a view (follows parents).
pub fn root_view(sheet: &Sheet, id: ViewId) -> Option<ViewId> {
    let mut cur = id;
    for _ in 0..64 {
        match sheet.parent_of(cur) {
            Some(p) => cur = p,
            None => return sheet.view(cur).map(|v| v.id),
        }
    }
    None
}

/// Source and part filter of a view.
pub fn view_source(sheet: &Sheet, id: ViewId) -> Option<(SourceKey, Option<BodyId>)> {
    let (mut key, part) = SourceKey::of_view(sheet.view(root_view(sheet, id)?)?)?;
    // Child views follow their parent's Explode/Position and Display state
    // unless they choose their own (the nearest choice up the chain wins).
    let (mut pos, mut disp) = (false, false);
    let mut cur = Some(id);
    for _ in 0..64 {
        let Some(v) = cur.and_then(|c| sheet.view(c)) else { break };
        if !pos && let Some(p) = v.own_position {
            (key.explode, key.position) = match p {
                super::AsmPosition::Default => (None, None),
                super::AsmPosition::Explode(e) => (Some(e), None),
                super::AsmPosition::Position(n) => (None, Some(n)),
            };
            pos = true;
        }
        if !disp && let Some(d) = &v.own_display {
            key.display = match d {
                super::DisplayChoice::ShowAll => None,
                super::DisplayChoice::State(s) => Some(s.clone()),
            };
            disp = true;
        }
        cur = sheet.parent_of(v.id);
    }
    Some((key, part))
}

/// Effective scale (paper length / model length) of a view.
pub fn view_scale(sheet: &Sheet, id: ViewId) -> f64 {
    let mut cur = id;
    for _ in 0..64 {
        let Some(v) = sheet.view(cur) else { break };
        if let Some(s) = v.scale
            && s > 0.0
        {
            return s;
        }
        match sheet.parent_of(cur) {
            Some(p) => cur = p,
            None => break,
        }
    }
    if sheet.scale > 0.0 { sheet.scale } else { 1.0 }
}

/// The bodies a view shows.
pub fn view_bodies<'a>(ctx: &'a Context, part: Option<&BodyId>) -> Vec<&'a Arc<Body>> {
    match part {
        // One part, whatever its kind (Insert view's Surfaces / Curves filters).
        Some(p) => ctx.bodies.iter().filter(|b| &b.id == p && b.kind != BodyKind::Point).collect(),
        // The whole Part Studio: its solids (an open composite part would
        // draw its members twice).
        None => ctx.bodies.iter().filter(|b| b.kind == BodyKind::Solid && !b.composite.as_ref().is_some_and(|c| !c.closed)).collect(),
    }
}

/// View coordinates of the center of a source's bodies (the point a base or
/// projected view of it places at the view position).
pub fn source_origin(sources: &Sources, key: &SourceKey, part: Option<&BodyId>, frame: &ViewFrame) -> Option<[f64; 2]> {
    let src = sources.get(key)?;
    let bodies = view_bodies(&src.context, part);
    (!bodies.is_empty()).then(|| frame.project(bodies_center(&bodies)))
}

// ------------------------------------------------------------ cutting lines

/// Move point `i` of a cutting line to `p`. With `keep_ortho` (jogged
/// section lines made of horizontal and vertical legs) the neighbors follow
/// so that every leg keeps its direction.
pub fn move_cut_point(points: &[[f64; 2]], i: usize, p: [f64; 2], keep_ortho: bool) -> Vec<[f64; 2]> {
    let mut out = points.to_vec();
    if i >= out.len() {
        return out;
    }
    let d = [p[0] - points[i][0], p[1] - points[i][1]];
    let tol = 1e-12;
    let ortho = keep_ortho && points.len() > 2 && points.windows(2).all(|w| (w[0][0] - w[1][0]).abs() < tol || (w[0][1] - w[1][1]).abs() < tol);
    out[i] = p;
    if ortho {
        for j in [i.wrapping_sub(1), i + 1] {
            let Some(q) = points.get(j) else { continue };
            if (q[1] - points[i][1]).abs() < tol {
                out[j][1] += d[1];
            } else {
                out[j][0] += d[0];
            }
        }
    }
    out
}

/// Closed uniform Catmull-Rom curve through `pts`, `per_segment` samples
/// per span (fewer than 3 points are returned as they are).
pub fn closed_spline(pts: &[[f64; 2]], per_segment: usize) -> Vec<[f64; 2]> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let k = per_segment.max(1);
    let mut out = Vec::with_capacity(n * k);
    for i in 0..n {
        let (p0, p1, p2, p3) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        for s in 0..k {
            let t = s as f64 / k as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |c: usize| 0.5 * (2.0 * p1[c] + (p2[c] - p0[c]) * t + (2.0 * p0[c] - 5.0 * p1[c] + 4.0 * p2[c] - p3[c]) * t2 + (3.0 * p1[c] - p0[c] - 3.0 * p2[c] + p3[c]) * t3);
            out.push([f(0), f(1)]);
        }
    }
    out
}

/// Outline polygon of a broken-out profile.
pub fn broken_out_outline(b: &BrokenOut) -> Vec<[f64; 2]> {
    if b.spline { closed_spline(&b.profile, 16) } else { b.profile.clone() }
}

/// Polygonal crop boundaries as a polygon (circles: None).
pub fn crop_polygon(c: &Crop) -> Option<Vec<[f64; 2]>> {
    match c {
        Crop::Circle { .. } => None,
        Crop::Polygon(p) => Some(p.clone()),
        Crop::Spline(p) => Some(closed_spline(p, 16)),
    }
}

// ------------------------------------------------------------ geometry

/// Classification of a model edge for dimensioning.
#[derive(Clone, Debug, PartialEq)]
pub enum EdgeGeom {
    /// Straight edge (projected endpoints).
    Line { a: [f64; 2], b: [f64; 2] },
    /// Circle or arc. `face_on` when the circle's axis is parallel to the
    /// viewing direction (it projects to a true circle); `bore` when it
    /// bounds a cylindrical face of the same radius and axis (a hole or
    /// boss rather than e.g. a fillet rim); `mid` is the projected middle
    /// point of the edge (on the arc).
    Circle { center: [f64; 2], radius: f64, face_on: bool, bore: bool, mid: [f64; 2] },
    Other,
}

/// A model edge that appears in a view.
#[derive(Clone, Debug)]
pub struct ModelEdge {
    pub body: BodyId,
    pub name: Name,
    pub geom: EdgeGeom,
    /// Endpoint references (start, end) with their projected positions.
    pub ends: [Option<(PointRef, [f64; 2])>; 2],
    /// Seam of a periodic face (both sides on the same face).
    pub seam: bool,
}

impl ModelEdge {
    pub fn center_ref(&self) -> Option<PointRef> {
        matches!(self.geom, EdgeGeom::Circle { .. }).then(|| PointRef::Center { body: self.body.clone(), name: (*self.name).clone() })
    }
}

#[derive(Clone, Debug)]
pub struct ViewEdge {
    pub points: Vec<[f64; 2]>,
    pub visible: bool,
    pub class: EdgeClass,
    /// Index into [`ViewGeometry::model_edges`].
    pub model: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Vertex,
    Center,
}

/// A point the dimension tools can snap to.
#[derive(Clone, Debug)]
pub struct Snap {
    pub p: [f64; 2],
    pub kind: SnapKind,
    pub point: Option<PointRef>,
}

/// Projected geometry of one view, in view coordinates (meters).
#[derive(Clone, Debug, Default)]
pub struct ViewGeometry {
    pub id: ViewId,
    pub frame: Option<ViewFrame>,
    /// View coordinates that sit at the view's sheet position.
    pub origin: [f64; 2],
    pub edges: Vec<ViewEdge>,
    pub model_edges: Vec<ModelEdge>,
    pub snaps: Vec<Snap>,
    /// Closed boundary loops (as segment soups) of section cut faces.
    pub section_segments: Vec<[[f64; 2]; 2]>,
    /// Detail crop circle (center, radius) in view coordinates.
    pub clip: Option<([f64; 2], f64)>,
    /// Detail crop polygon (rectangle / polygon / spline profiles), view
    /// coordinates; replaces the circle when set.
    pub clip_poly: Option<Vec<[f64; 2]>>,
    /// Center of mass (view coordinates) when shown.
    pub com: Option<[f64; 2]>,
    /// Flat pattern views: bend lines and form centermarks.
    pub bends: Vec<FlatBend>,
    /// Cosmetic threads of tapped holes seen end-on: center and major
    /// radius (view coordinates).
    pub threads: Vec<([f64; 2], f64)>,
    pub form_marks: Vec<[f64; 2]>,
    /// Crop boundary of a cropped view (view coordinates).
    pub crop: Option<Crop>,
    /// Axes of cylindrical faces seen from the side (for automatic
    /// centerlines), as projected segments.
    pub axes: Vec<[[f64; 2]; 2]>,
    pub min: [f64; 2],
    pub max: [f64; 2],
    /// Exact HLR (false = fast polygonal preview).
    pub exact: bool,
    pub error: Option<String>,
    /// Aligned sections: how model points map into the view (the revolved
    /// leg), for associative annotations.
    pub aligned: Option<AlignedMap>,
}

/// A bend line of a flat pattern view (view coordinates) with what its
/// bend note says.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatBend {
    pub a: [f64; 2],
    pub b: [f64; 2],
    /// Bends toward the viewer.
    pub up: bool,
    /// Bend angle (radians), inner radius and the bend region's width in
    /// the flat (m).
    pub angle: f64,
    pub radius: f64,
    pub width: f64,
}

/// Point mapping of an aligned section view: model points on the revolved
/// leg's side of the center are rotated into the viewing plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AlignedMap {
    pub parent: ViewFrame,
    /// Center and unit leg directions in the parent's view coordinates.
    pub center: [f64; 2],
    pub ua: [f64; 2],
    pub ub: [f64; 2],
    pub axis_origin: DVec3,
    pub axis: DVec3,
    pub angle: f64,
}

impl AlignedMap {
    /// Whether a model point lies on the revolved (`b`) leg's side.
    pub fn revolved(&self, p: DVec3) -> bool {
        let q = self.parent.project(p);
        let d = [q[0] - self.center[0], q[1] - self.center[1]];
        let tol = 1e-9;
        d[0] * self.ua[0] + d[1] * self.ua[1] < -tol && d[0] * self.ub[0] + d[1] * self.ub[1] >= -tol
    }
    pub fn rotate(&self, p: DVec3) -> DVec3 {
        self.axis_origin + DQuat::from_axis_angle(self.axis, self.angle) * (p - self.axis_origin)
    }
    /// Where a model point appears in the section (3D, before projection).
    pub fn map(&self, p: DVec3) -> DVec3 {
        if self.revolved(p) { self.rotate(p) } else { p }
    }
}

impl ViewGeometry {
    fn update_bounds(&mut self) {
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for e in &self.edges {
            for p in &e.points {
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
        }
        if let Some((c, r)) = self.clip {
            min = [c[0] - r, c[1] - r];
            max = [c[0] + r, c[1] + r];
        }
        if let Some(poly) = &self.clip_poly {
            min = [poly.iter().map(|p| p[0]).fold(f64::MAX, f64::min), poly.iter().map(|p| p[1]).fold(f64::MAX, f64::min)];
            max = [poly.iter().map(|p| p[0]).fold(f64::MIN, f64::max), poly.iter().map(|p| p[1]).fold(f64::MIN, f64::max)];
        }
        if let Some(crop) = &self.crop
            && min[0].is_finite()
        {
            let (lo, hi) = crop_bounds(crop);
            min = [min[0].max(lo[0]), min[1].max(lo[1])];
            max = [max[0].min(hi[0]), max[1].min(hi[1])];
            if min[0] > max[0] || min[1] > max[1] {
                (min, max) = (lo, hi);
            }
        }
        if !min[0].is_finite() {
            min = self.origin;
            max = self.origin;
        }
        self.min = min;
        self.max = max;
    }

    /// Edge whose polyline passes closest to `p` (view coordinates), within `tol`.
    pub fn edge_near(&self, p: [f64; 2], tol: f64, include_hidden: bool) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, e) in self.edges.iter().enumerate() {
            if !include_hidden && !e.visible {
                continue;
            }
            for w in e.points.windows(2) {
                let d = dist_to_segment(p, w[0], w[1]);
                if d <= tol && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
        best
    }

    pub fn snap_near(&self, p: [f64; 2], tol: f64) -> Option<&Snap> {
        self.snaps
            .iter()
            .map(|s| (s, ((s.p[0] - p[0]).powi(2) + (s.p[1] - p[1]).powi(2)).sqrt()))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(s, _)| s)
    }

    /// Model edges that appear as real lines (not only as seams).
    pub fn drawn_model_edges(&self) -> Vec<bool> {
        let mut out = vec![false; self.model_edges.len()];
        for e in &self.edges {
            if let Some(m) = e.model
                && e.class != EdgeClass::Seam
                && !self.model_edges.get(m as usize).is_some_and(|me| me.seam)
                && let Some(x) = out.get_mut(m as usize)
            {
                *x = true;
            }
        }
        out
    }

    /// Face-on circles (center, radius, reference) for radial dimensions
    /// and centermarks.
    pub fn circles(&self) -> Vec<([f64; 2], f64, Option<PointRef>)> {
        let mut out: Vec<([f64; 2], f64, Option<PointRef>)> = Vec::new();
        let drawn = self.drawn_model_edges();
        for (_, m) in self.model_edges.iter().enumerate().filter(|(i, _)| drawn[*i]) {
            if let EdgeGeom::Circle { center, radius, face_on: true, .. } = m.geom
                && !out.iter().any(|(c, r, _)| (c[0] - center[0]).abs() < 1e-9 && (c[1] - center[1]).abs() < 1e-9 && (r - radius).abs() < 1e-9)
            {
                out.push((center, radius, m.center_ref()));
            }
        }
        out
    }

    /// Face-on circular bores (holes and bosses): (center, radius, reference),
    /// one per center and radius.
    pub fn bores(&self) -> Vec<([f64; 2], f64, Option<PointRef>)> {
        let mut out: Vec<([f64; 2], f64, Option<PointRef>)> = Vec::new();
        let drawn = self.drawn_model_edges();
        for (_, m) in self.model_edges.iter().enumerate().filter(|(i, _)| drawn[*i]) {
            if let EdgeGeom::Circle { center, radius, face_on: true, bore: true, .. } = m.geom
                && !out.iter().any(|(c, r, _)| (c[0] - center[0]).abs() < 1e-9 && (c[1] - center[1]).abs() < 1e-9 && (r - radius).abs() < 1e-9)
            {
                out.push((center, radius, m.center_ref()));
            }
        }
        out
    }
}

/// Bounding box of a crop boundary.
pub fn crop_bounds(c: &Crop) -> ([f64; 2], [f64; 2]) {
    match c {
        Crop::Circle { center, radius } => ([center[0] - radius, center[1] - radius], [center[0] + radius, center[1] + radius]),
        Crop::Polygon(_) | Crop::Spline(_) => {
            let p = crop_polygon(c).unwrap_or_default();
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            for q in &p {
                for k in 0..2 {
                    lo[k] = lo[k].min(q[k]);
                    hi[k] = hi[k].max(q[k]);
                }
            }
            (lo, hi)
        }
    }
}

/// Parts of a polyline inside a crop boundary.
pub fn clip_to_crop(pts: &[[f64; 2]], crop: &Crop) -> Vec<Vec<[f64; 2]>> {
    match crop {
        Crop::Circle { center, radius } => clip_polyline_circle(pts, *center, *radius),
        Crop::Polygon(poly) => clip_polyline_polygon(pts, poly),
        Crop::Spline(p) => clip_polyline_polygon(pts, &closed_spline(p, 16)),
    }
}

/// Even-odd point-in-polygon test.
pub fn point_in_polygon(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Parts of a polyline inside a closed polygon (any shape, even-odd).
pub fn clip_polyline_polygon(pts: &[[f64; 2]], poly: &[[f64; 2]]) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    if poly.len() < 3 {
        return out;
    }
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let n = poly.len();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = [b[0] - a[0], b[1] - a[1]];
        let mut ts = vec![0.0, 1.0];
        for i in 0..n {
            let (c, e) = (poly[i], poly[(i + 1) % n]);
            let g = [e[0] - c[0], e[1] - c[1]];
            let den = d[0] * g[1] - d[1] * g[0];
            if den.abs() < 1e-300 {
                continue;
            }
            let t = ((c[0] - a[0]) * g[1] - (c[1] - a[1]) * g[0]) / den;
            let u = ((c[0] - a[0]) * d[1] - (c[1] - a[1]) * d[0]) / den;
            if t > 0.0 && t < 1.0 && (0.0..=1.0).contains(&u) {
                ts.push(t);
            }
        }
        ts.sort_by(f64::total_cmp);
        for k in 0..ts.len() - 1 {
            let (t0, t1) = (ts[k], ts[k + 1]);
            if t1 - t0 < 1e-12 {
                continue;
            }
            let tm = (t0 + t1) / 2.0;
            let at = |t: f64| [a[0] + d[0] * t, a[1] + d[1] * t];
            if point_in_polygon(at(tm), poly) {
                let p0 = at(t0);
                if cur.last().is_none_or(|l| (l[0] - p0[0]).abs() > 1e-15 || (l[1] - p0[1]).abs() > 1e-15) {
                    if cur.len() >= 2 {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(p0);
                }
                cur.push(at(t1));
            } else if cur.len() >= 2 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

/// Parts of a polyline with `axis` coordinate outside the open band (a, b).
pub fn clip_polyline_band(pts: &[[f64; 2]], axis: usize, a: f64, b: f64) -> Vec<Vec<[f64; 2]>> {
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let keep = |p: [f64; 2]| p[axis] <= a || p[axis] >= b;
    for w in pts.windows(2) {
        let (p, q) = (w[0], w[1]);
        let mut ts = vec![0.0, 1.0];
        let dp = q[axis] - p[axis];
        if dp.abs() > 1e-300 {
            for c in [a, b] {
                let t = (c - p[axis]) / dp;
                if t > 0.0 && t < 1.0 {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(f64::total_cmp);
        for k in 0..ts.len() - 1 {
            let (t0, t1) = (ts[k], ts[k + 1]);
            if t1 - t0 < 1e-12 {
                continue;
            }
            let at = |t: f64| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
            let m = at((t0 + t1) / 2.0);
            // Points exactly on the band edge count as kept.
            if keep(m) || (m[axis] - a).abs() < 1e-15 || (m[axis] - b).abs() < 1e-15 {
                let p0 = at(t0);
                if cur.last().is_none_or(|l| (l[0] - p0[0]).abs() > 1e-15 || (l[1] - p0[1]).abs() > 1e-15) {
                    if cur.len() >= 2 {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(p0);
                }
                cur.push(at(t1));
            } else if cur.len() >= 2 {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

pub fn dist_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let q = [a[0] + d[0] * t, a[1] + d[1] * t];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

/// Sample an edge into 3D points.
pub fn sample_edge(e: &Shape, segments: usize) -> Vec<DVec3> {
    let Ok(info) = e.edge_info() else { return vec![] };
    if info.degenerated {
        return vec![];
    }
    if matches!(info.curve, Curve::Line { .. }) {
        return vec![info.start, info.end];
    }
    let n = segments.max(2);
    (0..=n).filter_map(|i| e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64).ok()).collect()
}

/// Bounding box center of a set of bodies.
fn bodies_center(bodies: &[&Arc<Body>]) -> DVec3 {
    let mut acc: Option<(DVec3, DVec3)> = None;
    for b in bodies {
        if let Ok(bb) = b.shape.bounding_box(false) {
            acc = Some(match acc {
                None => (bb.min, bb.max),
                Some((a, c)) => (a.min(bb.min), c.max(bb.max)),
            });
        }
    }
    acc.map(|(a, b)| (a + b) * 0.5).unwrap_or(DVec3::ZERO)
}

fn bodies_diagonal(bodies: &[&Arc<Body>]) -> f64 {
    let mut acc: Option<(DVec3, DVec3)> = None;
    for b in bodies {
        if let Ok(bb) = b.shape.bounding_box(false) {
            acc = Some(match acc {
                None => (bb.min, bb.max),
                Some((a, c)) => (a.min(bb.min), c.max(bb.max)),
            });
        }
    }
    acc.map(|(a, b)| (b - a).length()).unwrap_or(0.1).max(1e-6)
}

/// Resolve a point reference: projected position and, for circle centers,
/// the radius.
pub fn resolve_point(ctx: &Context, frame: &ViewFrame, r: &PointRef) -> Option<([f64; 2], Option<f64>)> {
    if let PointRef::Intersection { .. } = r {
        return Some((intersection_in_view(ctx, frame, r)?, None));
    }
    if let PointRef::Quadrant { k, .. } = r {
        let (c, radius) = resolve_point3(ctx, r)?;
        return Some((quadrant_of(frame.project(c), radius?, *k), None));
    }
    match r {
        PointRef::Vertex { body, name } => {
            let b = ctx.body(body)?;
            let i = *b.find_vertices(name).first()?;
            let p = b.shape.sub_shape(ShapeType::Vertex, i).ok()?.vertex_point().ok()?;
            Some((frame.project(p), None))
        }
        PointRef::Center { body, name } => {
            let b = ctx.body(body)?;
            let i = *b.find_edges(name).first()?;
            match b.shape.sub_shape(ShapeType::Edge, i).ok()?.edge_curve().ok()? {
                Curve::Circle { center, radius, .. } => Some((frame.project(center), Some(radius))),
                _ => None,
            }
        }
        PointRef::Midpoint { .. } | PointRef::Quadrant { .. } | PointRef::Intersection { .. } => resolve_point3(ctx, r).map(|(p, _)| (frame.project(p), None)),
    }
}

/// Projected endpoints of a straight edge.
fn projected_line(ctx: &Context, frame: &ViewFrame, body: &BodyId, name: &crate::reference::TopoName) -> Option<([f64; 2], [f64; 2])> {
    let b = ctx.body(body)?;
    let i = *b.find_edges(name).first()?;
    let (p, q) = b.shape.sub_shape(ShapeType::Edge, i).ok()?.endpoints().ok()?;
    Some((frame.project(p), frame.project(q)))
}

/// Where two straight edges meet in a view (virtual sharp).
fn intersection_in_view(ctx: &Context, frame: &ViewFrame, r: &PointRef) -> Option<[f64; 2]> {
    let PointRef::Intersection { body, name, body2, name2 } = r else { return None };
    let (a1, b1) = projected_line(ctx, frame, body, name)?;
    let (a2, b2) = projected_line(ctx, frame, body2, name2)?;
    super::AnnotationKind::line_intersection(a1, b1, a2, b2)
}

/// A quadrant point of a projected circle (0 right, 1 top, 2 left, 3 bottom).
fn quadrant_of(c: [f64; 2], radius: f64, k: u8) -> [f64; 2] {
    let a = f64::from(k % 4) * std::f64::consts::FRAC_PI_2;
    [c[0] + radius * a.cos(), c[1] + radius * a.sin()]
}

/// Model position of a point reference (and, for circle centers, the radius).
pub fn resolve_point3(ctx: &Context, r: &PointRef) -> Option<(DVec3, Option<f64>)> {
    match r {
        PointRef::Vertex { body, name } => {
            let b = ctx.body(body)?;
            let i = *b.find_vertices(name).first()?;
            Some((b.shape.sub_shape(ShapeType::Vertex, i).ok()?.vertex_point().ok()?, None))
        }
        // Quadrants resolve to the circle (center, radius); views offset them.
        PointRef::Center { body, name } | PointRef::Quadrant { body, name, .. } => {
            let b = ctx.body(body)?;
            let i = *b.find_edges(name).first()?;
            match b.shape.sub_shape(ShapeType::Edge, i).ok()?.edge_curve().ok()? {
                Curve::Circle { center, radius, .. } => Some((center, Some(radius))),
                _ => None,
            }
        }
        // Intersections are resolved in a view (see `resolve_point`); in 3D
        // the middle of the closest points of the two lines.
        PointRef::Intersection { body, name, body2, name2 } => {
            let line = |body: &BodyId, name: &crate::reference::TopoName| -> Option<(DVec3, DVec3)> {
                let b = ctx.body(body)?;
                let i = *b.find_edges(name).first()?;
                b.shape.sub_shape(ShapeType::Edge, i).ok()?.endpoints().ok()
            };
            let ((p1, q1), (p2, q2)) = (line(body, name)?, line(body2, name2)?);
            let (d1, d2, w) = (q1 - p1, q2 - p2, p1 - p2);
            let (a, b, c, d, e) = (d1.dot(d1), d1.dot(d2), d2.dot(d2), d1.dot(w), d2.dot(w));
            let den = a * c - b * b;
            if den.abs() < 1e-18 {
                return None;
            }
            let (s, t) = ((b * e - c * d) / den, (a * e - b * d) / den);
            Some(((p1 + d1 * s + p2 + d2 * t) * 0.5, None))
        }
        PointRef::Midpoint { body, name } => {
            let b = ctx.body(body)?;
            let i = *b.find_edges(name).first()?;
            let e = b.shape.sub_shape(ShapeType::Edge, i).ok()?;
            let info = e.edge_info().ok()?;
            Some((e.point_at((info.first_param + info.last_param) * 0.5).ok()?, None))
        }
    }
}

/// Resolve a point reference in a computed view: like [`resolve_point`],
/// following the view's point mapping (aligned sections).
pub fn resolve_in_view(ctx: &Context, g: &ViewGeometry, r: &PointRef) -> Option<([f64; 2], Option<f64>)> {
    let frame = g.frame?;
    let (p, radius) = resolve_point3(ctx, r)?;
    let p = match &g.aligned {
        Some(m) => m.map(p),
        None => p,
    };
    if let PointRef::Quadrant { k, .. } = r {
        return Some((quadrant_of(frame.project(p), radius?, *k), None));
    }
    if let (PointRef::Intersection { .. }, None) = (r, &g.aligned) {
        return Some((intersection_in_view(ctx, &frame, r)?, None));
    }
    Some((frame.project(p), radius))
}

/// Reference to the vertex nearest to a 3D point (within `tol`).
pub fn vertex_ref_near(ctx: &Context, p: DVec3, tol: f64) -> Option<PointRef> {
    let mut best: Option<(f64, PointRef)> = None;
    for b in &ctx.bodies {
        for (i, v) in b.shape.vertices().iter().enumerate() {
            let Ok(q) = v.vertex_point() else { continue };
            let d = q.distance(p);
            if d <= tol && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, PointRef::Vertex { body: b.id.clone(), name: (*b.vertices[i]).clone() }));
            }
        }
    }
    best.map(|(_, r)| r)
}

/// Reference to the center of a circular edge whose center is near `p`
/// (the largest such circle when `largest`, else the smallest).
pub fn circle_ref_near(ctx: &Context, p: DVec3, tol: f64, largest: bool) -> Option<PointRef> {
    let mut best: Option<(f64, PointRef)> = None;
    for b in &ctx.bodies {
        for (i, e) in b.shape.edges().iter().enumerate() {
            if let Ok(Curve::Circle { center, radius, .. }) = e.edge_curve()
                && center.distance(p) <= tol
            {
                let better = match &best {
                    None => true,
                    Some((r, _)) => (largest && radius > *r) || (!largest && radius < *r),
                };
                if better {
                    best = Some((radius, PointRef::Center { body: b.id.clone(), name: (*b.edges[i]).clone() }));
                }
            }
        }
    }
    best.map(|(_, r)| r)
}

// ------------------------------------------------------------ computation

/// Cache of computed view geometry keyed by (source content, view
/// parameters, exactness); shared between sheets and rebuilds.
#[derive(Default)]
pub struct ViewCache {
    map: Mutex<HashMap<u64, Arc<ViewGeometry>>>,
}

impl ViewCache {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn get(&self, key: u64) -> Option<Arc<ViewGeometry>> {
        self.map.lock().get(&key).cloned()
    }
    fn put(&self, key: u64, g: Arc<ViewGeometry>) {
        let mut m = self.map.lock();
        if m.len() > 512 {
            m.clear();
        }
        m.insert(key, g);
    }
    pub fn len(&self) -> usize {
        self.map.lock().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Cache key of a view's geometry; `None` when the view cannot be computed
/// (missing parent or source).
pub fn view_key(drawing: &Drawing, sheet: &Sheet, id: ViewId, sources: &Sources, exact: bool) -> Option<u64> {
    // View render mode › Best quality: always exact.
    let exact = exact || sheet.view(id).is_some_and(|v| v.best_quality);
    let v = sheet.view(id)?;
    let frame = view_frame(drawing, sheet, id)?;
    let (key, part) = view_source(sheet, id)?;
    let src = sources.get(&key)?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    src.hash.hash(&mut h);
    part.hash(&mut h);
    frame.key(&mut h);
    exact.hash(&mut h);
    v.shown_sketches.hash(&mut h);
    v.hide_sketch_points.hash(&mut h);
    v.hidden_parts.hash(&mut h);
    v.show_com.hash(&mut h);
    v.flat.hash(&mut h);
    v.hide_threads.hash(&mut h);
    let fx = |x: f64, h: &mut std::collections::hash_map::DefaultHasher| ((x * 1e9).round() as i64).hash(h);
    match &v.kind {
        ViewKind::Section { parent, .. } | ViewKind::AlignedSection { parent, .. } => {
            (if matches!(v.kind, ViewKind::Section { .. }) { 1u8 } else { 3u8 }).hash(&mut h);
            let cut = section_cut(sheet, id)?;
            let pf = view_frame(drawing, sheet, *parent)?;
            pf.key(&mut h);
            for p in &cut.points {
                fx(p[0], &mut h);
                fx(p[1], &mut h);
            }
            fx(cut.toward_view[0], &mut h);
            fx(cut.toward_view[1], &mut h);
            if let Some(pre) = pre_cut(sheet, id) {
                view_key(drawing, sheet, pre, sources, exact)?.hash(&mut h);
            }
        }
        ViewKind::Detail { parent, center, radius, .. } => {
            2u8.hash(&mut h);
            view_key(drawing, sheet, *parent, sources, exact)?.hash(&mut h);
            for x in [center[0], center[1], *radius] {
                fx(x, &mut h);
            }
            for p in v.detail_profile.iter().flatten() {
                fx(p[0], &mut h);
                fx(p[1], &mut h);
            }
        }
        // Projected views carry their parent's broken-out sections, or with
        // Shift+click its section cut.
        ViewKind::Projected { parent, .. } if projects_section(sheet, id) || !broken_out_cuts(drawing, sheet, *parent).is_empty() => {
            4u8.hash(&mut h);
            v.cut_from_section.hash(&mut h);
            view_key(drawing, sheet, *parent, sources, exact)?.hash(&mut h);
        }
        _ => 0u8.hash(&mut h),
    }
    v.partial.hash(&mut h);
    v.cut_only.hash(&mut h);
    format!("{:?}", sketch_of(sheet, id)).hash(&mut h);
    v.section_depth.map(|d| (d * 1e9).round() as i64).hash(&mut h);
    format!("{:?}{:?}", v.section_depth_ref, v.exclude_from_cut).hash(&mut h);
    format!("{:?}", view_simplification(sheet, id)).hash(&mut h);
    for b in &v.broken_out {
        b.spline.hash(&mut h);
        fx(b.depth, &mut h);
        format!("{:?}", b.depth_ref).hash(&mut h);
        for p in &b.profile {
            fx(p[0], &mut h);
            fx(p[1], &mut h);
        }
    }
    match &v.crop {
        None => 0u8.hash(&mut h),
        Some(Crop::Circle { center, radius }) => {
            1u8.hash(&mut h);
            for x in [center[0], center[1], *radius] {
                fx(x, &mut h);
            }
        }
        Some(Crop::Polygon(pts)) | Some(Crop::Spline(pts)) => {
            (if matches!(v.crop, Some(Crop::Spline(_))) { 3u8 } else { 2u8 }).hash(&mut h);
            for p in pts {
                fx(p[0], &mut h);
                fx(p[1], &mut h);
            }
        }
    }
    Some(h.finish())
}

/// Compute (or fetch from the cache) a view's geometry.
pub fn compute_view(drawing: &Drawing, sheet: &Sheet, id: ViewId, sources: &Sources, cache: &ViewCache, exact: bool) -> Arc<ViewGeometry> {
    let exact = exact || sheet.view(id).is_some_and(|v| v.best_quality);
    let fail = |msg: &str| Arc::new(ViewGeometry { id, error: Some(msg.to_string()), ..Default::default() });
    let Some(key) = view_key(drawing, sheet, id, sources, exact) else {
        return fail("The view's source is not available");
    };
    if let Some(g) = cache.get(key) {
        return g;
    }
    let v = sheet.view(id).expect("view exists (key computed)");
    let frame = view_frame(drawing, sheet, id).expect("frame exists (key computed)");
    let (skey, part) = view_source(sheet, id).expect("source exists (key computed)");
    let simplification = view_simplification(sheet, id);
    let mut ctx = simplify_context(sources.get(&skey).expect("source exists (key computed)").context.clone(), part.as_ref(), simplification);
    if !v.hidden_parts.is_empty() {
        let mut c = (*ctx).clone();
        c.bodies.retain(|b| !v.hidden_parts.contains(&b.id));
        ctx = Arc::new(c);
    }
    let mut g = match &v.kind {
        ViewKind::Detail { parent, center, radius, .. } => {
            let pg = compute_view(drawing, sheet, *parent, sources, cache, exact);
            detail_geometry(id, &pg, *center, *radius, v.detail_profile.as_deref())
        }
        ViewKind::Section { parent, .. } => {
            let cut = section_cut(sheet, id).expect("cut exists (key computed)");
            let pf = view_frame(drawing, sheet, *parent).expect("parent frame");
            let pre = pre_cut(sheet, id).and_then(|s| Some((section_cut(sheet, s)?, view_frame(drawing, sheet, sheet.parent_of(s)?)?)));
            section_geometry(id, &ctx, part.as_ref(), &frame, &frame, &pf, &cut, pre.as_ref(), exact)
        }
        ViewKind::AlignedSection { parent, .. } => {
            let cut = section_cut(sheet, id).expect("cut exists (key computed)");
            let pf = view_frame(drawing, sheet, *parent).expect("parent frame");
            aligned_section_geometry(id, &ctx, part.as_ref(), &frame, &pf, &cut, exact)
        }
        ViewKind::Base { .. } if v.flat => flat_geometry(id, &ctx, part.as_ref(), &frame),
        // Sketch views (and views projected from them): the sketch's curves.
        ViewKind::Base { .. } | ViewKind::Projected { .. } | ViewKind::Auxiliary { .. } if sketch_of(sheet, id).is_some() => sketch_geometry(id, &ctx, &sketch_of(sheet, id).expect("checked"), &frame),
        // Shift+click: a projected view with the parent section's cut.
        ViewKind::Projected { parent, .. } if projects_section(sheet, id) => {
            let (Some(cut), Some(sf), Some(pf)) = (section_cut(sheet, *parent), view_frame(drawing, sheet, *parent), sheet.parent_of(*parent).and_then(|gp| view_frame(drawing, sheet, gp))) else {
                return fail("The parent section is not available");
            };
            section_geometry(id, &ctx, part.as_ref(), &frame, &sf, &pf, &cut, None, exact)
        }
        _ => {
            let cuts = broken_out_cuts(drawing, sheet, id);
            if cuts.is_empty() {
                projected_geometry(id, &ctx, part.as_ref(), &frame, exact)
            } else {
                broken_out_geometry(id, &ctx, part.as_ref(), &frame, &cuts, exact)
            }
        }
    };
    if !matches!(v.kind, ViewKind::Detail { .. }) {
        simplify_edges(&mut g, &ctx, part.as_ref(), simplification);
    }
    // Center of mass of the view's parts (parts with materials), a snap
    // point for dimensions.
    if v.show_com {
        let src = sources.get(&skey).expect("source exists (key computed)");
        let bodies: Vec<BodyId> = view_bodies(&ctx, part.as_ref()).iter().map(|b| b.id.clone()).collect();
        let density = |b: &BodyId| src.parts.iter().find(|p| p.body.as_ref() == Some(b)).and_then(|p| p.density);
        if !bodies.is_empty() && bodies.iter().all(|b| density(b).is_some())
            && let Some(m) = crate::regen::measure::mass_properties(&ctx, &bodies, &density)
        {
            let p = frame.project(m.center_of_mass);
            g.com = Some(p);
            g.snaps.push(Snap { p, kind: SnapKind::Vertex, point: None });
        }
    }
    // Cosmetic threads: tapped holes whose entrance (or exit, through)
    // faces the viewer; holes seen from the side are inside the material.
    if !v.hide_threads && !matches!(v.kind, ViewKind::Detail { .. }) {
        let boxes: Vec<_> = view_bodies(&ctx, part.as_ref()).iter().filter_map(|b| b.shape.bounding_box(false).ok()).collect();
        for hole in &ctx.holes {
            let Some(major) = hole.thread.as_deref().and_then(thread_major_diameter) else { continue };
            let along = hole.dir.normalize().dot(frame.dir);
            let inside = |p: DVec3| boxes.iter().any(|bb| p.cmpge(bb.min - DVec3::splat(1e-6)).all() && p.cmple(bb.max + DVec3::splat(1e-6)).all());
            if (along > 0.999 || (hole.through && along < -0.999)) && inside(hole.start) {
                g.threads.push((frame.project(hole.start), major / 2.0));
            }
        }
    }
    // Shown source sketches, drawn over the model; their free points as
    // small crosses (0.6 mm on the sheet) unless hidden.
    if !v.shown_sketches.is_empty() {
        let arm = 0.6 / (view_scale(sheet, id) * 1000.0).max(1e-9);
        for f in &v.shown_sketches {
            let sg = sketch_geometry(id, &ctx, f, &frame);
            g.edges.extend(sg.edges);
            g.snaps.extend(sg.snaps.iter().cloned());
            if !v.hide_sketch_points {
                for s in &sg.snaps {
                    let p = s.p;
                    g.edges.push(ViewEdge { points: vec![[p[0] - arm, p[1]], [p[0] + arm, p[1]]], visible: true, class: noshape_kernel::EdgeClass::Sharp, model: None });
                    g.edges.push(ViewEdge { points: vec![[p[0], p[1] - arm], [p[0], p[1] + arm]], visible: true, class: noshape_kernel::EdgeClass::Sharp, model: None });
                }
            }
        }
        g.update_bounds();
    }
    if let Some(crop) = &v.crop {
        // Spline crops are clipped as their sampled outline.
        let crop = match crop_polygon(crop) {
            Some(p) if matches!(crop, Crop::Spline(_)) => Crop::Polygon(p),
            _ => crop.clone(),
        };
        apply_crop(&mut g, &crop);
    }
    let g = Arc::new(g);
    cache.put(key, g.clone());
    g
}

/// A view's simplification: its own, else its parent's (Auto at the root).
pub fn view_simplification(sheet: &Sheet, id: ViewId) -> Simplification {
    let mut cur = id;
    for _ in 0..32 {
        match sheet.view(cur) {
            Some(v) if v.simplification.is_some() => return v.simplification.expect("checked"),
            Some(_) => match sheet.parent_of(cur) {
                Some(p) => cur = p,
                None => break,
            },
            None => break,
        }
    }
    Simplification::Auto
}

/// Auto: leave out parts below 0.1% of the source size (dust, tiny
/// fasteners in big assemblies); detail inside parts is kept.
const AUTO_PART_RATIO: f64 = 0.001;

fn body_diagonal(b: &Body) -> f64 {
    b.shape.bounding_box(false).map(|bb| bb.diagonal()).unwrap_or(0.0)
}

/// Leave out whole parts below the threshold (Absolute, Ratio to studio, Auto).
fn simplify_context(ctx: Arc<Context>, part: Option<&BodyId>, s: Simplification) -> Arc<Context> {
    let studio = bodies_diagonal(&view_bodies(&ctx, part));
    let min = match s {
        Simplification::Absolute(v) => v,
        Simplification::RatioToStudio(r) => r * studio,
        Simplification::Auto => AUTO_PART_RATIO * studio,
        Simplification::None | Simplification::RatioToPart(_) => return ctx,
    };
    if min <= 0.0 || ctx.bodies.iter().all(|b| b.kind != BodyKind::Solid || body_diagonal(b) >= min) {
        return ctx;
    }
    let mut c = (*ctx).clone();
    c.bodies.retain(|b| b.kind != BodyKind::Solid || body_diagonal(b) >= min);
    Arc::new(c)
}

/// Leave out edges (detail) shorter than the threshold: Absolute and Ratio
/// to studio use one length, Ratio to part a fraction of each part's size.
fn simplify_edges(g: &mut ViewGeometry, ctx: &Context, part: Option<&BodyId>, s: Simplification) {
    let studio = bodies_diagonal(&view_bodies(ctx, part));
    let per_part: HashMap<BodyId, f64> = match s {
        Simplification::RatioToPart(r) => ctx.bodies.iter().map(|b| (b.id.clone(), r * body_diagonal(b))).collect(),
        _ => HashMap::new(),
    };
    let fixed = match s {
        Simplification::Absolute(v) => v,
        Simplification::RatioToStudio(r) => r * studio,
        Simplification::RatioToPart(_) => 0.0,
        Simplification::None | Simplification::Auto => return,
    };
    let model_edges = &g.model_edges;
    let len = |pts: &[[f64; 2]]| pts.windows(2).map(|w| ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt()).sum::<f64>();
    g.edges.retain(|e| {
        let min = e.model.and_then(|m| model_edges.get(m as usize)).and_then(|me| per_part.get(&me.body).copied()).unwrap_or(fixed);
        min <= 0.0 || len(&e.points) >= min
    });
    g.update_bounds();
}

/// Restrict a view's geometry to a crop boundary.
fn apply_crop(g: &mut ViewGeometry, crop: &Crop) {
    let edges = std::mem::take(&mut g.edges);
    for e in edges {
        for piece in clip_to_crop(&e.points, crop) {
            g.edges.push(ViewEdge { points: piece, visible: e.visible, class: e.class, model: e.model });
        }
    }
    let poly = crop_polygon(crop);
    let inside = |p: [f64; 2]| match (crop, &poly) {
        (Crop::Circle { center, radius }, _) => (p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2) <= radius * radius * (1.0 + 1e-9),
        (_, Some(poly)) => point_in_polygon(p, poly),
        _ => true,
    };
    g.snaps.retain(|s| inside(s.p));
    // Section boundaries stay whole; hatch lines are cropped at build time.
    let axes = std::mem::take(&mut g.axes);
    for a in axes {
        for piece in clip_to_crop(&[a[0], a[1]], crop) {
            if piece.len() >= 2 {
                g.axes.push([piece[0], piece[piece.len() - 1]]);
            }
        }
    }
    g.crop = Some(crop.clone());
    g.update_bounds();
}

/// Compute every view of a sheet (parents first).
pub fn compute_sheet_views(drawing: &Drawing, sheet: &Sheet, sources: &Sources, cache: &ViewCache, exact: bool) -> HashMap<ViewId, Arc<ViewGeometry>> {
    sheet.views.iter().map(|v| (v.id, compute_view(drawing, sheet, v.id, sources, cache, exact))).collect()
}

/// Per-body topology used to describe model edges.
struct BodyTopo {
    /// Edge -> vertex indices.
    ev: Vec<Vec<usize>>,
    /// Edge -> face indices.
    ef: Vec<Vec<u32>>,
}

impl BodyTopo {
    fn of(shape: &Shape) -> Self {
        let ne = shape.count(ShapeType::Edge);
        let mut ev = vec![Vec::new(); ne];
        if let Ok(ve) = shape.vertex_edges() {
            for (vi, edges) in ve.iter().enumerate() {
                for &e in edges {
                    if let Some(slot) = ev.get_mut(e as usize) {
                        slot.push(vi);
                    }
                }
            }
        }
        BodyTopo { ev, ef: shape.edge_faces().unwrap_or_default() }
    }
}

/// One body as projected: the body (for names) and the shape to project
/// (the body itself or a section cut of it), with an output-edge -> body-edge map.
struct ProjBody<'a> {
    body: &'a Arc<Body>,
    shape: Shape,
    edge_map: Option<HashMap<usize, usize>>,
}

fn model_edge(body: &Arc<Body>, edge_index: usize, frame: &ViewFrame, topo: &BodyTopo) -> Option<ModelEdge> {
    let e = body.shape.sub_shape(ShapeType::Edge, edge_index).ok()?;
    let info = e.edge_info().ok()?;
    let geom = match info.curve {
        Curve::Line { .. } => EdgeGeom::Line { a: frame.project(info.start), b: frame.project(info.end) },
        Curve::Circle { center, axis, radius, .. } => {
            let axis = axis.normalize();
            let bore = topo.ef.get(edge_index).is_some_and(|faces| {
                faces.iter().any(|&f| {
                    matches!(body.shape.sub_shape(ShapeType::Face, f as usize).and_then(|f| f.face_surface()),
                        Ok(Surface::Cylinder { axis: a, radius: rc, .. }) if (rc - radius).abs() <= radius * 1e-6 + 1e-12 && a.normalize().dot(axis).abs() > 1.0 - 1e-6)
                })
            });
            let mid = e.point_at((info.first_param + info.last_param) / 2.0).map(|p| frame.project(p)).unwrap_or(frame.project(info.start));
            EdgeGeom::Circle { center: frame.project(center), radius, face_on: axis.dot(frame.dir).abs() > 1.0 - 1e-6, bore, mid }
        }
        _ => EdgeGeom::Other,
    };
    let mut ends: [Option<(PointRef, [f64; 2])>; 2] = [None, None];
    if let Some(vs) = topo.ev.get(edge_index) {
        for &vi in vs {
            let Ok(p) = body.shape.sub_shape(ShapeType::Vertex, vi).and_then(|v| v.vertex_point()) else { continue };
            let r = PointRef::Vertex { body: body.id.clone(), name: (*body.vertices[vi]).clone() };
            let slot = if p.distance(info.start) <= p.distance(info.end) { 0 } else { 1 };
            if ends[slot].is_none() {
                ends[slot] = Some((r, frame.project(p)));
            } else if ends[1 - slot].is_none() {
                ends[1 - slot] = Some((r, frame.project(p)));
            }
        }
    }
    let seam = topo.ef.get(edge_index).is_some_and(|f| f.len() == 1 || (f.len() == 2 && f[0] == f[1]));
    Some(ModelEdge { body: body.id.clone(), name: body.edges[edge_index].clone(), geom, ends, seam })
}

/// Axes of (at least half) cylindrical faces perpendicular to the view
/// direction, as projected segments covering the faces' extent.
fn cylinder_axes(shapes: &[Shape], frame: &ViewFrame) -> Vec<[[f64; 2]; 2]> {
    // (origin, axis, angular intervals, tmin, tmax)
    type Group = (DVec3, DVec3, Vec<(f64, f64)>, f64, f64);
    let mut groups: Vec<Group> = Vec::new();
    let tau = std::f64::consts::TAU;
    for shape in shapes {
        for f in shape.faces() {
            let Ok(Surface::Cylinder { origin, axis, radius, .. }) = f.face_surface() else { continue };
            let axis = axis.normalize();
            if axis.dot(frame.dir).abs() > 1e-6 || radius <= 0.0 {
                continue;
            }
            let Ok(uv) = f.uv_bounds() else { continue };
            let (u0, u1) = (uv[0].min(uv[1]), uv[0].max(uv[1]));
            let (mut t0, mut t1) = (f64::INFINITY, f64::NEG_INFINITY);
            for e in f.edges() {
                for p in sample_edge(&e, 8) {
                    let t = (p - origin).dot(axis);
                    t0 = t0.min(t);
                    t1 = t1.max(t);
                }
            }
            // Also skips NaN extents.
            if t1.partial_cmp(&t0) != Some(std::cmp::Ordering::Greater) {
                continue;
            }
            // Coaxial faces (split cylinders) join one group.
            let found = groups.iter_mut().find(|g| g.1.dot(axis).abs() > 1.0 - 1e-9 && ((origin - g.0) - g.1 * (origin - g.0).dot(g.1)).length() < radius * 1e-6 + 1e-12);
            match found {
                Some(g) => {
                    let s = if g.1.dot(axis) < 0.0 { -1.0 } else { 1.0 };
                    let off = (origin - g.0).dot(g.1);
                    let (a, b) = (off + s * t0, off + s * t1);
                    // Angles are only comparable for equally oriented frames;
                    // opposite axes count as a separate coverage estimate.
                    if s > 0.0 {
                        g.2.push((u0, u1));
                    } else {
                        g.2.push((tau - u1, tau - u0));
                    }
                    g.3 = g.3.min(a.min(b));
                    g.4 = g.4.max(a.max(b));
                }
                None => groups.push((origin, axis, vec![(u0, u1)], t0, t1)),
            }
        }
    }
    groups
        .into_iter()
        .filter(|g| angular_coverage(&g.2) >= std::f64::consts::PI - 1e-6)
        .map(|(o, a, _, t0, t1)| [frame.project(o + a * t0), frame.project(o + a * t1)])
        .collect()
}

/// Total angle covered by a set of angular intervals (radians, union,
/// modulo a full turn). Pieces of one face split along the axis share their
/// interval and count once.
fn angular_coverage(iv: &[(f64, f64)]) -> f64 {
    let tau = std::f64::consts::TAU;
    let mut parts: Vec<(f64, f64)> = Vec::new();
    for &(a, b) in iv {
        let span = (b - a).min(tau);
        if span >= tau - 1e-9 {
            return tau;
        }
        let a = a.rem_euclid(tau);
        if a + span <= tau {
            parts.push((a, a + span));
        } else {
            parts.push((a, tau));
            parts.push((0.0, a + span - tau));
        }
    }
    parts.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut total = 0.0;
    let mut cur: Option<(f64, f64)> = None;
    for (a, b) in parts {
        match cur {
            Some((ca, cb)) if a <= cb + 1e-9 => cur = Some((ca, cb.max(b))),
            Some((ca, cb)) => {
                total += cb - ca;
                cur = Some((a, b));
            }
            None => cur = Some((a, b)),
        }
    }
    if let Some((a, b)) = cur {
        total += b - a;
    }
    total
}

/// HLR projection of bodies (optionally pre-cut) into a view.
/// `keep(body index, point)` filters the vertex snap points.
fn project_bodies(id: ViewId, bodies: &[ProjBody], frame: &ViewFrame, exact: bool, keep: &dyn Fn(usize, DVec3) -> bool) -> ViewGeometry {
    let mut g = ViewGeometry { id, frame: Some(*frame), exact, ..Default::default() };
    if bodies.is_empty() {
        g.error = Some("No parts to show".into());
        g.update_bounds();
        return g;
    }
    let shapes: Vec<Shape> = bodies.iter().map(|b| b.shape.clone()).collect();
    let compound = if shapes.len() == 1 { shapes[0].clone() } else { Shape::compound(&shapes) };
    // Compound edge index -> (body index, body edge index).
    let mut cmap: HashMap<usize, (usize, usize)> = HashMap::new();
    for (bi, pb) in bodies.iter().enumerate() {
        for (i, e) in pb.shape.edges().iter().enumerate() {
            let src = match &pb.edge_map {
                Some(m) => match m.get(&i) {
                    Some(s) => *s,
                    None => continue,
                },
                None => i,
            };
            if let Some(ci) = compound.index_of(e, ShapeType::Edge) {
                cmap.insert(ci, (bi, src));
            }
        }
    }
    let diag = compound.bounding_box(false).map(|b| b.diagonal()).unwrap_or(0.1).max(1e-6);
    let defl = if exact { diag / 1500.0 } else { diag / 500.0 };
    let projected = match noshape_kernel::project_hlr_with(&compound, frame.dir, frame.up, exact, defl) {
        Ok(p) => p,
        Err(e) => {
            g.error = Some(format!("Hidden line removal failed: {e}"));
            g.update_bounds();
            return g;
        }
    };
    let mut topo_cache: HashMap<usize, BodyTopo> = HashMap::new();
    let mut me_index: HashMap<(usize, usize), u32> = HashMap::new();
    for pe in projected {
        if pe.polyline.len() < 2 {
            continue;
        }
        let model = pe.source_edge.and_then(|ci| cmap.get(&(ci as usize)).copied()).and_then(|(bi, ei)| {
            if let Some(ix) = me_index.get(&(bi, ei)) {
                return Some(*ix);
            }
            let body = bodies[bi].body;
            let topo = topo_cache.entry(bi).or_insert_with(|| BodyTopo::of(&body.shape));
            let me = model_edge(body, ei, frame, topo)?;
            g.model_edges.push(me);
            let ix = (g.model_edges.len() - 1) as u32;
            me_index.insert((bi, ei), ix);
            Some(ix)
        });
        g.edges.push(ViewEdge {
            points: pe.polyline.iter().map(|p| [p.x, p.y]).collect(),
            visible: pe.visibility == Visibility::Visible,
            class: pe.class,
            model,
        });
    }
    // Snap points: vertices (on the kept side of a section) and centers of
    // face-on circles.
    for (bi, pb) in bodies.iter().enumerate() {
        let b = pb.body;
        for (i, v) in b.shape.vertices().iter().enumerate() {
            let Ok(p) = v.vertex_point() else { continue };
            if !keep(bi, p) {
                continue;
            }
            g.snaps.push(Snap { p: frame.project(p), kind: SnapKind::Vertex, point: Some(PointRef::Vertex { body: b.id.clone(), name: (*b.vertices[i]).clone() }) });
        }
    }
    let centers: Vec<Snap> = g
        .model_edges
        .iter()
        .filter_map(|m| match m.geom {
            EdgeGeom::Circle { center, face_on: true, .. } => Some(Snap { p: center, kind: SnapKind::Center, point: m.center_ref() }),
            _ => None,
        })
        .collect();
    g.snaps.extend(centers);
    g.axes = cylinder_axes(&shapes, frame);
    g
}

fn projected_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame, exact: bool) -> ViewGeometry {
    let bodies = view_bodies(ctx, part);
    let pbs: Vec<ProjBody> = bodies.iter().map(|b| ProjBody { body: b, shape: b.shape.clone(), edge_map: None }).collect();
    let mut g = project_bodies(id, &pbs, frame, exact, &|_, _| true);
    g.origin = frame.project(bodies_center(&bodies));
    g.update_bounds();
    g
}

/// The plane coordinates of `Plane::new(o, -parent.dir, parent.right)` are
/// the parent's view coordinates (offset by the origin's projection).
fn view_plane(parent: &ViewFrame, depth: f64) -> Plane {
    Plane::new(parent.dir * depth, -parent.dir, parent.right)
}

/// Solid covering the removed side of a (possibly jogged) cutting line:
/// the polygon bounded by the line (extended past the model unless
/// `partial`) and a far offset toward the viewer side, extruded through the
/// model along the parent's view direction.
fn removal_tool(parent: &ViewFrame, points: &[[f64; 2]], toward: [f64; 2], center: DVec3, big: f64, partial: bool) -> noshape_kernel::Result<Shape> {
    let v2 = |p: [f64; 2]| DVec2::new(p[0], p[1]);
    let mut pts: Vec<DVec2> = points.iter().map(|p| v2(*p)).collect();
    let n = pts.len();
    if !partial {
        let d0 = (pts[0] - pts[1]).normalize_or_zero();
        let d1 = (pts[n - 1] - pts[n - 2]).normalize_or_zero();
        pts[0] += d0 * big;
        pts[n - 1] += d1 * big;
    }
    let t = v2(toward) * big * 2.0;
    let mut poly = pts.clone();
    poly.push(pts[n - 1] + t);
    poly.push(pts[0] + t);
    // Drop consecutive duplicates.
    poly.dedup_by(|a, b| a.distance(*b) < 1e-12);
    let segs: Vec<Segment2d> = (0..poly.len()).map(|i| Segment2d::Line { a: poly[i], b: poly[(i + 1) % poly.len()] }).collect();
    let depth = center.dot(parent.dir);
    let plane = view_plane(parent, depth - big);
    let face = noshape_kernel::make_face_2d(&plane, &segs, &[])?;
    Ok(noshape_kernel::extrude(&face, parent.dir * big * 2.0)?.shape)
}

/// A shape to cut: the body it shows (names, geometry for model edges), the
/// shape itself and its edge -> body edge map (None = the body's own edges).
struct CutPiece<'a> {
    body: &'a Arc<Body>,
    shape: Shape,
    map: Option<HashMap<usize, usize>>,
}

/// Cut bodies with a removal tool; returns projected bodies (with edge
/// maps) and the section faces (tool faces facing the viewer) as edges.
fn cut_bodies<'a>(bodies: &[&'a Arc<Body>], tool: &noshape_kernel::Result<Shape>, sight: DVec3) -> (Vec<ProjBody<'a>>, Vec<Shape>, Option<String>) {
    let pieces: Vec<CutPiece> = bodies.iter().map(|b| CutPiece { body: b, shape: b.shape.clone(), map: None }).collect();
    cut_pieces(pieces, tool, sight)
}

fn cut_pieces<'a>(pieces: Vec<CutPiece<'a>>, tool: &noshape_kernel::Result<Shape>, sight: DVec3) -> (Vec<ProjBody<'a>>, Vec<Shape>, Option<String>) {
    let mut pbs: Vec<ProjBody> = Vec::new();
    let mut faces: Vec<Shape> = Vec::new();
    let mut error = None;
    for pc in pieces {
        let r = match tool {
            Ok(t) => noshape_kernel::cut(&pc.shape, t),
            Err(e) => Err(e.clone()),
        };
        match r {
            Ok(r) => {
                if r.shape.count(ShapeType::Face) == 0 {
                    continue;
                }
                let mut map = HashMap::new();
                for i in 0..pc.shape.count(ShapeType::Edge) {
                    let src = match &pc.map {
                        Some(m) => match m.get(&i) {
                            Some(s) => *s,
                            None => continue,
                        },
                        None => i,
                    };
                    for o in r.history.modified(0, ShapeType::Edge, i as u32) {
                        map.entry(o as usize).or_insert(src);
                    }
                }
                for (fi, f) in r.shape.faces().iter().enumerate() {
                    let from_tool = r.history.of_output(ShapeType::Face, fi as u32).any(|h| h.arg == 1);
                    if from_tool && f.face_plane().ok().flatten().is_some_and(|p| p.normal.normalize().dot(sight).abs() > 1.0 - 1e-6) {
                        faces.push(f.clone());
                    }
                }
                pbs.push(ProjBody { body: pc.body, shape: r.shape, edge_map: Some(map) });
            }
            Err(e) => error = Some(format!("Section cut failed: {e}")),
        }
    }
    (pbs, faces, error)
}

fn section_segments_of(faces: &[Shape], frame: &ViewFrame) -> Vec<[[f64; 2]; 2]> {
    let mut out = Vec::new();
    for f in faces {
        for e in f.edges() {
            let pts: Vec<[f64; 2]> = sample_edge(&e, 48).into_iter().map(|p| frame.project(p)).collect();
            for w in pts.windows(2) {
                out.push([w[0], w[1]]);
            }
        }
    }
    out
}

/// "Cut from section": the section view whose cut a section of it applies
/// first (its parent; not for broken-out parents, which always cut the
/// base geometry).
pub fn pre_cut(sheet: &Sheet, id: ViewId) -> Option<ViewId> {
    let v = sheet.view(id)?;
    let ViewKind::Section { parent, .. } = v.kind else { return None };
    let p = sheet.view(parent)?;
    (v.cut_from_section && matches!(p.kind, ViewKind::Section { .. }) && p.broken_out.is_empty()).then_some(parent)
}

/// `frame` projects the result; `section` is the frame of the section view
/// whose cut this is (its line of sight is normal to the cut; a projected
/// view placed with Shift+click passes its parent section's).
#[allow(clippy::too_many_arguments)]
fn section_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame, section: &ViewFrame, parent: &ViewFrame, cut: &SectionCut, pre: Option<&(SectionCut, ViewFrame)>, _exact: bool) -> ViewGeometry {
    let partial = cut.partial;
    let all = view_bodies(ctx, part);
    // Exclude from cut: those parts are projected whole.
    let (bodies, kept): (Vec<&Arc<Body>>, Vec<&Arc<Body>>) = all.iter().copied().partition(|b| !cut.exclude.contains(&b.id));
    let diag = bodies_diagonal(&all);
    let center = bodies_center(&all);
    let sight = section.dir;
    let big = diag * 4.0 + 1.0;
    let mut tool = removal_tool(parent, &cut.points, cut.toward_view, center, big, partial);
    // Section depth: Blind, or Up to entity (the depth to that point,
    // measured along the line of sight from the cutting plane).
    let along0 = parent.lift([cut.points[1][0] - cut.p0[0], cut.points[1][1] - cut.p0[1]]).normalize();
    let plane0 = Plane::new(parent.lift(cut.p0) + parent.dir * center.dot(parent.dir), sight, along0);
    let depth = match &cut.depth_ref {
        Some(r) => resolve_point3(ctx, r).map(|(p, _)| plane0.signed_distance(p).abs()).filter(|d| *d > 1e-9),
        None => cut.depth,
    };
    if let Some(depth) = depth {
        let t = cut.toward_view;
        let far: Vec<[f64; 2]> = cut.points.iter().map(|p| [p[0] - t[0] * depth, p[1] - t[1] * depth]).collect();
        let back = removal_tool(parent, &far, [-t[0], -t[1]], center, big, false);
        tool = match (tool, back) {
            (Ok(a), Ok(b)) => noshape_kernel::fuse(&a, &b).map(|r| r.shape),
            (Err(e), _) | (_, Err(e)) => Err(e),
        };
    }
    let (mut pbs, mut faces, error) = match pre {
        // Cut from section: the parent section's cut first, then this one
        // (only this cut's faces are hatched).
        Some((pc, pf)) => {
            let pre_tool = removal_tool(pf, &pc.points, pc.toward_view, center, big, pc.partial);
            let (first, _, e0) = cut_bodies(&bodies, &pre_tool, sight);
            let pieces = first.into_iter().map(|pb| CutPiece { body: pb.body, shape: pb.shape, map: pb.edge_map }).collect();
            let (pbs, faces, e1) = cut_pieces(pieces, &tool, sight);
            (pbs, faces, e0.or(e1))
        }
        None => cut_bodies(&bodies, &tool, sight),
    };
    pbs.extend(kept.iter().map(|b| ProjBody { body: b, shape: b.shape.clone(), edge_map: None }));
    // Plane of the first leg (keeps snaps on the remaining side).
    let along = parent.lift([cut.points[1][0] - cut.p0[0], cut.points[1][1] - cut.p0[1]]).normalize();
    let p0 = parent.lift(cut.p0) + parent.dir * center.dot(parent.dir);
    let plane = Plane::new(p0, sight, along);
    let full = cut.points.len() == 2 && !partial;
    let tol = diag * 1e-7;
    if let Some(depth) = depth {
        // The faces left at the depth plane are not cut faces (no hatch).
        faces.retain(|f| !f.face_plane().ok().flatten().is_some_and(|p| (plane.signed_distance(p.origin).abs() - depth).abs() < diag * 1e-6 + 1e-9));
    }
    // Cut solids always use the exact HLR: OCCT's polygonal HLR
    // (HLRBRep_PolyAlgo::InsertOnOutLine) segfaults on some cuts, e.g.
    // through a cylinder axis. A cut is computed once; dragging only moves it.
    let mut g = project_bodies(id, &pbs, frame, true, &|_, p| !full || plane.signed_distance(p) >= -tol);
    if g.error.is_none() {
        g.error = error;
    }
    g.section_segments = section_segments_of(&faces, frame);
    if g.section_segments.is_empty() && full {
        // Fallback: planar section of the original bodies.
        for b in &bodies {
            if let Ok(c) = noshape_kernel::section_caps(&b.shape, &plane) {
                g.section_segments.extend(section_segments_of(&c.faces(), frame));
            }
        }
    }
    if cut.cut_only {
        // Show cut geometry only: the outlines of the cut faces.
        let segs = &g.section_segments;
        let near = diag * 1e-5 + 1e-9;
        g.edges.retain(|e| e.visible && e.points.iter().all(|p| segs.iter().any(|s| dist_to_segment(*p, s[0], s[1]) < near)));
    }
    g.origin = frame.project(center);
    g.update_bounds();
    g
}

/// Aligned section: the part on the `b` leg's side of the center is
/// revolved about the center axis into the plane of the `a` leg; both
/// halves are then cut along the straight line through the center. Edge
/// names are carried through, so dimensions stay associative (see
/// [`AlignedMap`]).
fn aligned_section_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame, parent: &ViewFrame, cut: &SectionCut, exact: bool) -> ViewGeometry {
    let bodies = view_bodies(ctx, part);
    let diag = bodies_diagonal(&bodies);
    let center3 = bodies_center(&bodies);
    let big = diag * 4.0 + 1.0;
    let [a, c, b] = [cut.points[0], cut.points[1], cut.points[2]];
    let v2 = |p: [f64; 2]| DVec2::new(p[0], p[1]);
    let ua = (v2(a) - v2(c)).normalize_or_zero();
    let ub = (v2(b) - v2(c)).normalize_or_zero();
    // Half-plane solids in the parent view: {x : (x - c) . u >= 0}.
    let half = |u: DVec2| -> noshape_kernel::Result<Shape> {
        let p = v2(c);
        let w = DVec2::new(-u.y, u.x);
        let poly = [p - w * big, p - w * big + u * big, p + w * big + u * big, p + w * big];
        let segs: Vec<Segment2d> = (0..4).map(|i| Segment2d::Line { a: poly[i], b: poly[(i + 1) % 4] }).collect();
        let plane = view_plane(parent, center3.dot(parent.dir) - big);
        let face = noshape_kernel::make_face_2d(&plane, &segs, &[])?;
        Ok(noshape_kernel::extrude(&face, parent.dir * big * 2.0)?.shape)
    };
    // Rotation (about the axis through c along the view direction) taking
    // ub onto -ua.
    let target = -ua;
    let ang = (ub.x * target.y - ub.y * target.x).atan2(ub.dot(target));
    let axis_origin = parent.lift(c);
    // Rotation is counter-clockwise in the parent view plane, i.e. about the
    // axis pointing toward the viewer.
    let rot = noshape_kernel::Transform::Rotate { origin: axis_origin, axis: -parent.dir, angle: ang };
    let map = AlignedMap { parent: *parent, center: c, ua: [ua.x, ua.y], ub: [ub.x, ub.y], axis_origin, axis: -parent.dir, angle: ang };
    // Rotated copies of the bodies keep the original names.
    let rotated: Vec<Option<Arc<Body>>> = bodies
        .iter()
        .map(|body| {
            let t = noshape_kernel::transform(&body.shape, &rot, false).ok()?;
            let mut rb = Body::new(body.id.clone(), body.name.clone(), body.kind, t.shape, vec![], body.color, body.id.feature);
            rb.faces = body.faces.clone();
            rb.edges = body.edges.clone();
            rb.vertices = body.vertices.clone();
            Some(Arc::new(rb))
        })
        .collect();
    let edge_map = |r: &noshape_kernel::OpResult, n: usize| -> HashMap<usize, usize> {
        let mut m = HashMap::new();
        for i in 0..n {
            for o in r.history.modified(0, ShapeType::Edge, i as u32) {
                m.entry(o as usize).or_insert(i);
            }
        }
        m
    };
    let (Ok(ha), Ok(hb)) = (half(ua), half(ub)) else {
        let mut g = ViewGeometry { id, frame: Some(*frame), exact, error: Some("Aligned section failed".into()), ..Default::default() };
        g.update_bounds();
        return g;
    };
    let mut pieces: Vec<CutPiece> = Vec::new();
    let mut error = None;
    for (body, rb) in bodies.iter().zip(&rotated) {
        let ne = body.shape.count(ShapeType::Edge);
        match noshape_kernel::common(&body.shape, &ha) {
            Ok(r) if r.shape.count(ShapeType::Face) > 0 => {
                let m = edge_map(&r, ne);
                pieces.push(CutPiece { body, shape: r.shape, map: Some(m) });
            }
            Ok(_) => {}
            Err(e) => error = Some(format!("Aligned section failed: {e}")),
        }
        let Some(rb) = rb else { continue };
        if let Ok(r) = noshape_kernel::common(&body.shape, &hb)
            && r.shape.count(ShapeType::Face) > 0
            && let Ok(t) = noshape_kernel::transform(&r.shape, &rot, false)
        {
            // A located copy keeps the sub-shape order of `r`.
            let m = edge_map(&r, ne);
            pieces.push(CutPiece { body: rb, shape: t.shape, map: Some(m) });
        }
    }
    // Straight cut through the center along the a leg.
    let line = [[c[0] + ua.x * big, c[1] + ua.y * big], [c[0] - ua.x * big, c[1] - ua.y * big]];
    let n = DVec2::new(-ua.y, ua.x);
    let s = if n.dot(v2(cut.toward_view)) < 0.0 { -1.0 } else { 1.0 };
    let tool = removal_tool(parent, &line, [n.x * s, n.y * s], center3, big, false);
    let (pbs, faces, err2) = cut_pieces(pieces, &tool, frame.dir);
    // Which projected bodies show the revolved leg.
    let revolved: Vec<bool> = pbs.iter().map(|pb| rotated.iter().flatten().any(|r| Arc::ptr_eq(r, pb.body))).collect();
    let tol = diag * 1e-7;
    let keep = |bi: usize, p: DVec3| {
        let q = parent.project(p);
        let d = DVec2::new(q[0] - c[0], q[1] - c[1]);
        let leg = if revolved.get(bi).copied().unwrap_or(false) { -ua } else { ua };
        d.dot(leg) >= -tol && d.dot(n * s) <= tol
    };
    // Exact HLR on cut solids (see `section_geometry`).
    let mut g = project_bodies(id, &pbs, frame, true, &keep);
    if g.error.is_none() {
        g.error = error.or(err2);
    }
    g.section_segments = section_segments_of(&faces, frame);
    g.origin = frame.project(center3);
    g.aligned = Some(map);
    g.update_bounds();
    g
}

/// Nearest and farthest depth (along `dir`) of a set of bodies' bounding boxes.
fn depth_range(bodies: &[&Arc<Body>], dir: DVec3) -> (f64, f64) {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in bodies {
        if let Ok(bb) = b.shape.bounding_box(true) {
            for k in 0..8 {
                let p = DVec3::new(if k & 1 == 0 { bb.min.x } else { bb.max.x }, if k & 2 == 0 { bb.min.y } else { bb.max.y }, if k & 4 == 0 { bb.min.z } else { bb.max.z });
                lo = lo.min(p.dot(dir));
                hi = hi.max(p.dot(dir));
            }
        }
    }
    if lo.is_finite() { (lo, hi) } else { (0.0, 0.0) }
}

/// Extent of the model along a view's line of sight (meters).
pub fn model_depth(ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame) -> f64 {
    let (lo, hi) = depth_range(&view_bodies(ctx, part), frame.dir);
    hi - lo
}

/// A projected view placed with Shift+click from a section view: it shows
/// the parent's cut (help H:projected_view).
pub fn projects_section(sheet: &Sheet, id: ViewId) -> bool {
    let Some(v) = sheet.view(id) else { return false };
    let ViewKind::Projected { parent, .. } = v.kind else { return false };
    v.cut_from_section && sheet.view(parent).is_some_and(|p| matches!(p.kind, ViewKind::Section { .. }))
}

/// The broken-out sections a view shows with the frame each was drawn in:
/// its own, and for projected views their parent's ("Projected views
/// created from broken-out views will also be broken", help
/// H:projected_view).
pub fn broken_out_cuts(drawing: &Drawing, sheet: &Sheet, id: ViewId) -> Vec<(BrokenOut, ViewFrame)> {
    let Some(v) = sheet.view(id) else { return Vec::new() };
    let mut out = match v.kind {
        ViewKind::Projected { parent, .. } if !projects_section(sheet, id) => broken_out_cuts(drawing, sheet, parent),
        _ => Vec::new(),
    };
    if !v.broken_out.is_empty()
        && let Some(f) = view_frame(drawing, sheet, id)
    {
        out.extend(v.broken_out.iter().map(|b| (b.clone(), f)));
    }
    out
}

/// Depth of a broken-out section below the model's nearest point.
pub fn broken_out_depth(ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame, b: &BrokenOut) -> f64 {
    match &b.depth_ref {
        Some(r) => match resolve_point3(ctx, r) {
            Some((p, _)) => p.dot(frame.dir) - depth_range(&view_bodies(ctx, part), frame.dir).0,
            None => b.depth,
        },
        None => b.depth,
    }
}

/// A view with broken-out sections: each profile, extruded from in front of
/// the model down to its depth, is cut away; the pocket floors are hatched.
fn broken_out_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, view: &ViewFrame, cuts: &[(BrokenOut, ViewFrame)], exact: bool) -> ViewGeometry {
    let bodies = view_bodies(ctx, part);
    let diag = bodies_diagonal(&bodies);
    let center = bodies_center(&bodies);
    let margin = diag * 0.05 + 1e-4;
    let mut tools: Vec<Shape> = Vec::new();
    let mut regions: Vec<(Vec<[f64; 2]>, f64, ViewFrame)> = Vec::new();
    let mut error = None;
    // Each profile is cut along the frame of the view it was drawn in (a
    // projected view's inherited cuts use its parent's).
    for (b, frame) in cuts {
        let (front, _) = depth_range(&bodies, frame.dir);
        let mut poly = broken_out_outline(b);
        poly.dedup_by(|p, q| (p[0] - q[0]).abs() < 1e-12 && (p[1] - q[1]).abs() < 1e-12);
        if poly.len() > 2 && poly.first() == poly.last() {
            poly.pop();
        }
        let depth = broken_out_depth(ctx, part, frame, b);
        if poly.len() < 3 || depth <= 0.0 {
            continue;
        }
        let segs: Vec<Segment2d> = (0..poly.len()).map(|i| Segment2d::Line { a: DVec2::from_array(poly[i]), b: DVec2::from_array(poly[(i + 1) % poly.len()]) }).collect();
        // Plane coordinates of `view_plane` are the view coordinates.
        let plane = view_plane(frame, front - margin);
        match noshape_kernel::make_face_2d(&plane, &segs, &[]).and_then(|f| noshape_kernel::extrude(&f, frame.dir * (depth + margin))) {
            Ok(t) => tools.push(t.shape),
            Err(e) => error = Some(format!("Broken-out section failed: {e}")),
        }
        regions.push((poly, front + depth, *frame));
    }
    let frame = view;
    if tools.is_empty() {
        let mut g = projected_geometry(id, ctx, part, frame, exact);
        if g.error.is_none() {
            g.error = error;
        }
        return g;
    }
    let tool = Ok(if tools.len() == 1 { tools.remove(0) } else { Shape::compound(&tools) });
    let (pbs, faces, err2) = cut_bodies(&bodies, &tool, frame.dir);
    // Vertices removed by a pocket are no snap points.
    let tol = diag * 1e-7;
    let keep = |_: usize, p: DVec3| !regions.iter().any(|(poly, floor, f)| p.dot(f.dir) < floor - tol && point_in_polygon(f.project(p), poly));
    // Exact HLR on cut solids (see `section_geometry`).
    let mut g = project_bodies(id, &pbs, frame, true, &keep);
    if g.error.is_none() {
        g.error = error.or(err2);
    }
    g.section_segments = section_segments_of(&faces, frame);
    g.origin = frame.project(center);
    g.update_bounds();
    g
}

/// The part seen at `p` (view coordinates) in a view with `frame`: the
/// front-most body the line of sight through `p` meets (callouts clicked on
/// a face or a silhouette, which have no model edge; qa5 D8). `tol`: how
/// far (view units) the line of sight may pass a body, as a click on a
/// silhouette lands beside the surface.
pub fn body_under(src: &SourceModel, frame: &ViewFrame, p: [f64; 2], tol: f64) -> Option<BodyId> {
    let at = frame.right * p[0] + frame.up * p[1];
    let reach = src.context.bodies.iter().filter_map(|b| b.shape.bounding_box(false).ok()).map(|b| b.diagonal()).fold(0.0, f64::max) * 4.0 + 1.0;
    let plane = Plane { origin: at, x_dir: frame.dir, normal: frame.right };
    let ray = noshape_kernel::make_edge_2d(&plane, &Segment2d::Line { a: DVec2::new(-reach, 0.0), b: DVec2::new(reach, 0.0) }).ok()?;
    let tol = tol.max(reach * 1e-7);
    src.context
        .bodies
        .iter()
        .filter(|b| b.kind == BodyKind::Solid)
        .filter_map(|b| {
            let d = noshape_kernel::distance(&ray, &b.shape).ok()?;
            (d.distance <= tol).then(|| (d.point_a.dot(frame.dir), b.id.clone()))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// Crop a parent view's geometry to a circle.
fn detail_geometry(id: ViewId, parent: &ViewGeometry, center: [f64; 2], radius: f64, poly: Option<&[[f64; 2]]>) -> ViewGeometry {
    let mut g = ViewGeometry { id, frame: parent.frame, origin: center, exact: parent.exact, error: parent.error.clone(), clip: Some((center, radius)), aligned: parent.aligned, ..Default::default() };
    // A polygon profile (rectangle, polygon, spline) replaces the circle.
    let poly = poly.filter(|p| p.len() >= 3);
    if let Some(p) = poly {
        g.clip = None;
        g.clip_poly = Some(p.to_vec());
    }
    let clip = |pts: &[[f64; 2]]| match poly {
        Some(p) => clip_polyline_polygon(pts, p),
        None => clip_polyline_circle(pts, center, radius),
    };
    let mut used: HashMap<u32, u32> = HashMap::new();
    for e in &parent.edges {
        for piece in clip(&e.points) {
            let model = e.model.map(|m| {
                *used.entry(m).or_insert_with(|| {
                    g.model_edges.push(parent.model_edges[m as usize].clone());
                    (g.model_edges.len() - 1) as u32
                })
            });
            g.edges.push(ViewEdge { points: piece, visible: e.visible, class: e.class, model });
        }
    }
    let inside = |p: [f64; 2]| match poly {
        Some(poly) => point_in_polygon(p, poly),
        None => (p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2) <= radius * radius * (1.0 + 1e-9),
    };
    g.snaps = parent.snaps.iter().filter(|s| inside(s.p)).cloned().collect();
    // Section boundaries stay whole (closed loops) so hatching works; the
    // hatch lines are clipped to the detail circle when the sheet is built.
    g.section_segments = parent.section_segments.clone();
    for a in &parent.axes {
        for piece in clip(&[a[0], a[1]]) {
            if piece.len() >= 2 {
                g.axes.push([piece[0], piece[piece.len() - 1]]);
            }
        }
    }
    g.update_bounds();
    g
}

/// Parts of a polyline inside a circle.
pub fn clip_polyline_circle(pts: &[[f64; 2]], c: [f64; 2], r: f64) -> Vec<Vec<[f64; 2]>> {
    let inside = |p: [f64; 2]| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) <= r * r;
    let mut out: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        // Parameter interval of the segment inside the circle.
        let d = [b[0] - a[0], b[1] - a[1]];
        let f = [a[0] - c[0], a[1] - c[1]];
        let qa = d[0] * d[0] + d[1] * d[1];
        let qb = 2.0 * (f[0] * d[0] + f[1] * d[1]);
        let qc = f[0] * f[0] + f[1] * f[1] - r * r;
        let disc = qb * qb - 4.0 * qa * qc;
        if qa < 1e-30 {
            continue;
        }
        if disc <= 0.0 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let sq = disc.sqrt();
        let t0 = ((-qb - sq) / (2.0 * qa)).max(0.0);
        let t1 = ((-qb + sq) / (2.0 * qa)).min(1.0);
        if t0 >= t1 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let pa = [a[0] + d[0] * t0, a[1] + d[1] * t0];
        let pb = [a[0] + d[0] * t1, a[1] + d[1] * t1];
        if cur.is_empty() || !inside(a) || t0 > 1e-12 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            cur.push(pa);
        }
        cur.push(pb);
        if t1 < 1.0 - 1e-12 {
            out.push(std::mem::take(&mut cur));
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out.retain(|p| p.len() >= 2);
    out
}



/// The sketch a view shows: its own (base views) or its root view's.
pub fn sketch_of(sheet: &Sheet, id: ViewId) -> Option<crate::FeatureId> {
    let root = root_view(sheet, id)?;
    sheet.view(root)?.sketch
}

/// View frame looking at a sketch's plane (from its normal side, its y up).
pub fn sketch_frame(ctx: &Context, feature: &crate::FeatureId) -> Option<[[f64; 3]; 2]> {
    let s = ctx.sketches.iter().find(|s| &s.feature == feature)?;
    Some([(-s.frame.normal).to_array(), s.frame.y.to_array()])
}

/// A sketch view: the sketch's (non-construction) curves projected into the
/// view; its points are snap points.
/// Major diameter (m) of a thread designation: metric "M6x1.0" / "M10",
/// unified "1/4-20" or numbered "#10-24".
pub fn thread_major_diameter(designation: &str) -> Option<f64> {
    let d = designation.trim();
    if let Some(m) = d.strip_prefix('M') {
        let n: String = m.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        return n.parse::<f64>().ok().map(|v| v * 1e-3);
    }
    let size = d.split('-').next()?.trim();
    let inch = if let Some(n) = size.strip_prefix('#') {
        // Numbered sizes: 0.060 + 0.013 × n.
        0.060 + 0.013 * n.parse::<f64>().ok()?
    } else if let Some((a, b)) = size.split_once('/') {
        // "1/4", "1 1/4".
        let (whole, num) = match a.trim().split_once(' ') {
            Some((w, n)) => (w.parse::<f64>().ok()?, n.parse::<f64>().ok()?),
            None => (0.0, a.trim().parse::<f64>().ok()?),
        };
        whole + num / b.trim().parse::<f64>().ok()?
    } else {
        size.parse::<f64>().ok()?
    };
    Some(inch * 0.0254)
}

/// A flat pattern view: the unfolded part's outline (cut edges), form
/// outlines and centermarks, and its bend lines (seen from the flat's
/// reference side, view X along the flat's X).
fn flat_geometry(id: ViewId, ctx: &Context, part: Option<&BodyId>, frame: &ViewFrame) -> ViewGeometry {
    let mut g = ViewGeometry { id, frame: Some(*frame), exact: true, ..Default::default() };
    let flat = match part.ok_or_else(|| "Select a sheet metal part".to_string()).and_then(|p| crate::sheetmetal::flat_pattern_for(ctx, p)) {
        Ok(f) => f,
        Err(e) => {
            g.error = Some(e);
            return g;
        }
    };
    let pt = |p: DVec2| [p.x, p.y];
    for l in flat.loops.iter().filter(|l| l.len() >= 2) {
        let mut pts: Vec<[f64; 2]> = l.iter().map(|p| pt(*p)).collect();
        pts.push(pts[0]);
        for w in pts.windows(2) {
            g.snaps.push(Snap { p: w[0], kind: SnapKind::Vertex, point: None });
        }
        g.edges.push(ViewEdge { points: pts, visible: true, class: noshape_kernel::EdgeClass::Outline, model: None });
    }
    for f in &flat.forms {
        for l in f.loops.iter().filter(|l| l.len() >= 2) {
            let mut pts: Vec<[f64; 2]> = l.iter().map(|p| pt(*p)).collect();
            pts.push(pts[0]);
            g.edges.push(ViewEdge { points: pts, visible: true, class: noshape_kernel::EdgeClass::Sharp, model: None });
        }
        g.form_marks.push(pt(f.center));
        g.snaps.push(Snap { p: pt(f.center), kind: SnapKind::Center, point: None });
    }
    for b in &flat.bend_lines {
        g.bends.push(FlatBend { a: pt(b.a), b: pt(b.b), up: b.up, angle: b.angle, radius: b.radius, width: b.allowance });
        g.snaps.push(Snap { p: pt(b.a), kind: SnapKind::Vertex, point: None });
        g.snaps.push(Snap { p: pt(b.b), kind: SnapKind::Vertex, point: None });
    }
    let (lo, hi) = flat.bounds();
    g.origin = [(lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0];
    g.update_bounds();
    g
}

fn sketch_geometry(id: ViewId, ctx: &Context, feature: &crate::FeatureId, frame: &ViewFrame) -> ViewGeometry {
    let mut g = ViewGeometry { id, frame: Some(*frame), exact: true, ..Default::default() };
    let Some(s) = ctx.sketches.iter().find(|s| &s.feature == feature) else {
        g.error = Some("The sketch was not found".into());
        return g;
    };
    let sk = &s.sketch;
    for e in sk.entities() {
        if e.construction {
            continue;
        }
        if let noshape_sketch::Geometry::Point { pos } = e.geometry {
            if sk.users_of_point(e.id).is_empty() {
                g.snaps.push(Snap { p: frame.project(s.frame.to_world(pos)), kind: SnapKind::Vertex, point: None });
            }
            continue;
        }
        let Some(c) = sk.curve(e.id) else { continue };
        let pts: Vec<[f64; 2]> = c.tessellate(1e-5).into_iter().map(|p| frame.project(s.frame.to_world(p))).collect();
        if pts.len() >= 2 {
            if let Some((a, b)) = e.endpoints().and_then(|(a, b)| Some((sk.point(a)?, sk.point(b)?))) {
                g.snaps.push(Snap { p: frame.project(s.frame.to_world(a)), kind: SnapKind::Vertex, point: None });
                g.snaps.push(Snap { p: frame.project(s.frame.to_world(b)), kind: SnapKind::Vertex, point: None });
            }
            g.edges.push(ViewEdge { points: pts, visible: true, class: noshape_kernel::EdgeClass::Sharp, model: None });
        }
    }
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in g.edges.iter().flat_map(|e| e.points.iter()) {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo[0] <= hi[0] {
        g.origin = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
    }
    g.update_bounds();
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_frames_follow_onshape_axes() {
        let f = orientation_frame(ViewOrientation::Front);
        assert!((f.right - DVec3::X).length() < 1e-12 && (f.up - DVec3::Z).length() < 1e-12);
        let t = orientation_frame(ViewOrientation::Top);
        assert!((t.right - DVec3::X).length() < 1e-12 && (t.up - DVec3::Y).length() < 1e-12);
        let r = orientation_frame(ViewOrientation::Right);
        assert!((r.right - DVec3::Y).length() < 1e-12 && (r.up - DVec3::Z).length() < 1e-12);
    }

    #[test]
    fn named_view_frame_is_the_camera() {
        // The Part Studio's Front camera (+90° about X) gives the Front frame.
        let q = DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2);
        let nv = crate::partstudio::NamedView { name: "N".into(), target: [0.0; 3], rotation: q.to_array(), distance: 1.0, perspective: false, section: Vec::new() };
        let [d, u] = named_view_frame(&nv);
        let f = ViewFrame::new(DVec3::from_array(d), DVec3::from_array(u));
        let front = orientation_frame(ViewOrientation::Front);
        assert!((f.dir - front.dir).length() < 1e-9 && (f.up - front.up).length() < 1e-9 && (f.right - front.right).length() < 1e-9, "{f:?}");
    }

    #[test]
    fn third_angle_folds() {
        let front = orientation_frame(ViewOrientation::Front);
        let top = orientation_frame(ViewOrientation::Top);
        let right = orientation_frame(ViewOrientation::Right);
        let up = fold(&front, [0.0, 1.0], Projection::ThirdAngle);
        assert!((up.dir - top.dir).length() < 1e-9 && (up.up - top.up).length() < 1e-9, "{up:?}");
        let rt = fold(&front, [1.0, 0.0], Projection::ThirdAngle);
        assert!((rt.dir - right.dir).length() < 1e-9 && (rt.up - right.up).length() < 1e-9);
        // First angle: the view to the right shows the left side.
        let lf = fold(&front, [1.0, 0.0], Projection::FirstAngle);
        assert!((lf.dir - orientation_frame(ViewOrientation::Left).dir).length() < 1e-9);
        // Diagonal: isometric.
        let iso = fold(&front, [1.0, 1.0], Projection::ThirdAngle);
        assert!((iso.dir - orientation_frame(ViewOrientation::Isometric).dir).length() < 1e-9);
    }

    #[test]
    fn circle_clipping() {
        let pieces = clip_polyline_circle(&[[-2.0, 0.0], [2.0, 0.0]], [0.0, 0.0], 1.0);
        assert_eq!(pieces.len(), 1);
        assert!((pieces[0][0][0] + 1.0).abs() < 1e-9 && (pieces[0][1][0] - 1.0).abs() < 1e-9);
        let pieces = clip_polyline_circle(&[[-2.0, 0.0], [0.0, 0.0], [0.0, 2.0], [2.0, 2.0]], [0.0, 0.0], 1.0);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].len(), 3);
        assert!(clip_polyline_circle(&[[-2.0, 3.0], [2.0, 3.0]], [0.0, 0.0], 1.0).is_empty());
    }
}
