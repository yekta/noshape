//! Bill of materials of an Assembly (the right strip's "BOM Table" panel,
//! live assembly/181–187): BOM type Flattened / Structured, the default
//! columns Item | Quantity | Part number | Description plus the "Add column"
//! properties, the ⋯ options (Show excluded/suppressed, Show top level
//! assembly row), Copy table and Export to CSV. Cell edits write the
//! part's (or subassembly's) Properties, like Onshape.

use super::eval::AssemblyResult;
use super::standard::StandardSpec;
use super::eval::InstancePath;
use super::{Assembly, BomBehavior, InstanceSource};
use crate::configuration::ConfigValues;
use crate::document::{Document, TabContent, TabId};
use crate::partstudio::{PartProperties, RELEASE_STATES};
use crate::reference::BodyId;
use serde::{Deserialize, Serialize};

/// "BOM type" (native select: Flattened, Structured).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BomType {
    #[default]
    Flattened,
    Structured,
}

impl BomType {
    pub const ALL: [BomType; 2] = [BomType::Flattened, BomType::Structured];
    pub fn label(self) -> &'static str {
        match self {
            BomType::Flattened => "Flattened",
            BomType::Structured => "Structured",
        }
    }
}

/// A BOM table column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BomColumn {
    Item,
    Quantity,
    PartNumber,
    Description,
    Appearance,
    Name,
    Revision,
    State,
    Vendor,
    Project,
    ProductLine,
    Material,
    Title1,
    Title2,
    Title3,
    NotRevisionManaged,
    UnitOfMeasure,
    Mass,
    SubassemblyBomBehavior,
    TessellationQuality,
}

/// How a column's cells are edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellEdit {
    /// Computed or locked (Item, Quantity, State, Material, Mass, …).
    ReadOnly,
    Text,
    Check,
    /// A list of choices (`BomColumn::choices`).
    Choice,
}

/// Onshape's "Unit of measure" choices (Properties; "Each" for a new part).
pub const UNITS_OF_MEASURE: [&str; 18] = ["", "Centimeter", "Foot", "Inch", "Meter", "Millimeter", "Yard", "Gram", "Kilogram", "Ounce", "Pound", "Liter", "Gallon", "Each", "Fluid ounce", "Milliliter", "Centiliter", "Package"];
/// Cell texts (live 374–375).
pub const NO_VALUE: &str = "No value";
pub const NOT_APPLICABLE: &str = "N/A";
pub const NO_MATERIAL: &str = "- None -";

/// Default unit of measure of a part.
pub const DEFAULT_UNIT_OF_MEASURE: &str = "Each";

impl BomColumn {
    /// The default table (live 181).
    pub const DEFAULT: [BomColumn; 4] = [BomColumn::Item, BomColumn::Quantity, BomColumn::PartNumber, BomColumn::Description];
    /// "Add column ▾" list, in Onshape's order (live 183).
    pub const ADDABLE: [BomColumn; 16] = [
        BomColumn::Appearance,
        BomColumn::Name,
        BomColumn::Revision,
        BomColumn::State,
        BomColumn::Vendor,
        BomColumn::Project,
        BomColumn::ProductLine,
        BomColumn::Material,
        BomColumn::Title1,
        BomColumn::Title2,
        BomColumn::Title3,
        BomColumn::NotRevisionManaged,
        BomColumn::UnitOfMeasure,
        BomColumn::Mass,
        BomColumn::SubassemblyBomBehavior,
        BomColumn::TessellationQuality,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BomColumn::Item => "Item",
            BomColumn::Quantity => "Quantity",
            BomColumn::PartNumber => "Part number",
            BomColumn::Description => "Description",
            BomColumn::Appearance => "Appearance",
            BomColumn::Name => "Name",
            BomColumn::Revision => "Revision",
            BomColumn::State => "State",
            BomColumn::Vendor => "Vendor",
            BomColumn::Project => "Project",
            BomColumn::ProductLine => "Product line",
            BomColumn::Material => "Material",
            BomColumn::Title1 => "Title 1",
            BomColumn::Title2 => "Title 2",
            BomColumn::Title3 => "Title 3",
            BomColumn::NotRevisionManaged => "Not revision managed",
            BomColumn::UnitOfMeasure => "Unit of measure",
            BomColumn::Mass => "Mass",
            BomColumn::SubassemblyBomBehavior => "Subassembly BOM behavior",
            BomColumn::TessellationQuality => "Tessellation quality",
        }
    }

    /// Item is the frozen first column (live: `os-colgroup-frozen`).
    pub fn frozen(self) -> bool {
        self == BomColumn::Item
    }

    pub fn edit(self) -> CellEdit {
        match self {
            BomColumn::PartNumber
            | BomColumn::Description
            | BomColumn::Name
            | BomColumn::Revision
            | BomColumn::Vendor
            | BomColumn::Project
            | BomColumn::ProductLine
            | BomColumn::Title1
            | BomColumn::Title2
            | BomColumn::Title3 => CellEdit::Text,
            BomColumn::NotRevisionManaged => CellEdit::Check,
            BomColumn::UnitOfMeasure | BomColumn::SubassemblyBomBehavior | BomColumn::TessellationQuality => CellEdit::Choice,
            // State and Material are locked like in Properties (live 131–134).
            BomColumn::Item | BomColumn::Quantity | BomColumn::Appearance | BomColumn::State | BomColumn::Material | BomColumn::Mass => CellEdit::ReadOnly,
        }
    }

    /// Choices of a `CellEdit::Choice` column.
    pub fn choices(self) -> Vec<&'static str> {
        match self {
            BomColumn::UnitOfMeasure => UNITS_OF_MEASURE.to_vec(),
            BomColumn::SubassemblyBomBehavior => BomBehavior::ALL.iter().map(|b| b.label()).collect(),
            BomColumn::TessellationQuality => crate::partstudio::TessQuality::ALL.iter().map(|q| q.label()).collect(),
            _ => Vec::new(),
        }
    }
}

/// The BOM panel's settings, stored with the Assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BomSettings {
    #[serde(default)]
    pub bom_type: BomType,
    pub columns: Vec<BomColumn>,
    /// ⋯ › Show excluded/suppressed.
    #[serde(default)]
    pub show_excluded: bool,
    /// ⋯ › Show top level assembly row.
    #[serde(default)]
    pub show_top_level: bool,
}

impl Default for BomSettings {
    fn default() -> Self {
        BomSettings { bom_type: BomType::Flattened, columns: BomColumn::DEFAULT.to_vec(), show_excluded: false, show_top_level: false }
    }
}

impl BomSettings {
    pub fn is_default(&self) -> bool {
        *self == BomSettings::default()
    }

    /// Add column: appended at the right.
    pub fn add_column(&mut self, c: BomColumn) {
        if !self.columns.contains(&c) {
            self.columns.push(c);
        }
    }

    /// Header › Remove column (Item stays).
    pub fn remove_column(&mut self, c: BomColumn) {
        if !c.frozen() {
            self.columns.retain(|x| *x != c);
        }
    }

    /// Header › Move left (-1) / Move right (+1); Item stays first.
    pub fn move_column(&mut self, c: BomColumn, delta: i32) {
        let Some(i) = self.columns.iter().position(|x| *x == c) else { return };
        let j = i as i32 + delta;
        if c.frozen() || j < 0 || j as usize >= self.columns.len() || self.columns[j as usize].frozen() {
            return;
        }
        self.columns.swap(i, j as usize);
    }

    /// Whether Move left / Move right are possible for a column.
    pub fn can_move(&self, c: BomColumn, delta: i32) -> bool {
        let Some(i) = self.columns.iter().position(|x| *x == c) else { return false };
        let j = i as i32 + delta;
        !c.frozen() && j >= 0 && (j as usize) < self.columns.len() && !self.columns[j as usize].frozen()
    }
}

/// What a BOM row stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum BomSource {
    /// A part of a Part Studio of this document.
    Part { studio: TabId, body: BodyId, config: ConfigValues, mirrored: bool },
    /// A part of another document (read-only properties).
    External { key: String, body: Option<BodyId>, config: ConfigValues },
    /// Standard content.
    Standard(StandardSpec),
    /// A subassembly (Assembly tab of this document).
    Assembly(TabId),
    /// An Assembly of another document (read-only properties).
    ExternalAssembly { key: String, config: ConfigValues },
}

impl BomSource {
    /// A subassembly row (this document's or another's).
    pub fn is_assembly(&self) -> bool {
        matches!(self, BomSource::Assembly(_) | BomSource::ExternalAssembly { .. })
    }
}

/// One BOM row.
#[derive(Clone, Debug)]
pub struct BomLine {
    /// Item number ("1", "2", "3.1"); "--" for excluded rows and the
    /// assembly name for the top level assembly row (live).
    pub item: Option<String>,
    /// Nesting level (Structured).
    pub depth: usize,
    pub source: BomSource,
    pub name: String,
    pub quantity: usize,
    /// The instances the row counts.
    pub instances: Vec<InstancePath>,
    /// Excluded from BOMs (only listed with Show excluded/suppressed, at
    /// the end, Item "--").
    pub excluded: bool,
    /// The top level assembly row.
    pub top_level: bool,
    /// Has child rows (Structured subassembly).
    pub has_children: bool,
}

/// One part / subassembly use found while walking the assembly tree.
struct Use {
    source: BomSource,
    name: String,
    path: InstancePath,
    excluded: bool,
    /// Subassembly children (Structured).
    children: Vec<Use>,
    behavior: BomBehavior,
}

fn part_props<'a>(doc: &'a Document, studio: TabId, body: &BodyId) -> Option<&'a PartProperties> {
    match doc.tab(studio).map(|t| &t.content) {
        Some(TabContent::PartStudio(ps)) => ps.part_properties.get(body),
        _ => None,
    }
}

fn assembly_of(doc: &Document, tab: TabId) -> Option<&Assembly> {
    match doc.tab(tab).map(|t| &t.content) {
        Some(TabContent::Assembly(a)) => Some(a),
        _ => None,
    }
}

/// Part name of an occurrence of `path` with `body` (solved result), else
/// the part's Properties name.
fn part_name(doc: &Document, result: &AssemblyResult, path: &InstancePath, studio: Option<TabId>, body: &BodyId) -> String {
    if let Some(o) = result.occurrences.iter().find(|o| &o.path == path && &o.body == body) {
        return o.body_name.clone();
    }
    if let Some(o) = result.occurrences.iter().find(|o| Some(o.studio) == studio && &o.body == body) {
        return o.body_name.clone();
    }
    studio.and_then(|s| part_props(doc, s, body)).and_then(|p| p.name.clone()).unwrap_or_else(|| "Part".into())
}

fn walk(doc: &Document, asm: &Assembly, prefix: &InstancePath, result: &AssemblyResult, suppressed_above: bool, depth: usize, out: &mut Vec<Use>) {
    if depth > 16 {
        return;
    }
    for inst in &asm.instances {
        let mut path = prefix.clone();
        path.push(inst.id);
        // A suppressed instance is not in the BOM at all, even with Show
        // excluded/suppressed (live).
        if suppressed_above || inst.suppressed || result.node(&path).is_some_and(|n| n.suppressed) {
            continue;
        }
        let suppressed = false;
        match &inst.source {
            InstanceSource::Part { studio, body, config } | InstanceSource::MirroredPart { studio, body, config, .. } => {
                let excluded = suppressed || part_props(doc, *studio, body).is_some_and(|p| p.meta.exclude_from_bom);
                let mirrored = matches!(inst.source, InstanceSource::MirroredPart { .. });
                let mut name = part_name(doc, result, &path, Some(*studio), body);
                if mirrored {
                    name = format!("{name} (mirrored)");
                }
                out.push(Use { source: BomSource::Part { studio: *studio, body: body.clone(), config: config.clone(), mirrored }, name, path, excluded, children: Vec::new(), behavior: BomBehavior::default() });
            }
            InstanceSource::PartStudio { studio, config } => {
                // Every solid / surface part of the rigid Part Studio instance.
                for o in result.occurrences.iter().filter(|o| o.path == path) {
                    let excluded = suppressed || part_props(doc, *studio, &o.body).is_some_and(|p| p.meta.exclude_from_bom);
                    out.push(Use { source: BomSource::Part { studio: *studio, body: o.body.clone(), config: config.clone(), mirrored: false }, name: o.body_name.clone(), path: path.clone(), excluded, children: Vec::new(), behavior: BomBehavior::default() });
                }
            }
            InstanceSource::External { key, body: None, config } if result.node(&path).is_some_and(|n| n.linked.is_some()) => {
                let Some((name, sub)) = result.node(&path).and_then(|n| n.linked.clone()) else { continue };
                let mut children = Vec::new();
                walk(doc, &sub, &path, result, suppressed, depth + 1, &mut children);
                let excluded = suppressed || sub.properties.meta.exclude_from_bom;
                out.push(Use { source: BomSource::ExternalAssembly { key: key.clone(), config: config.clone() }, name, path, excluded, children, behavior: sub.properties.bom_behavior });
            }
            InstanceSource::External { key, body, config } => match body {
                Some(b) => {
                    let name = part_name(doc, result, &path, None, b);
                    out.push(Use { source: BomSource::External { key: key.clone(), body: Some(b.clone()), config: config.clone() }, name, path, excluded: suppressed, children: Vec::new(), behavior: BomBehavior::default() });
                }
                None => {
                    for o in result.occurrences.iter().filter(|o| o.path == path) {
                        out.push(Use { source: BomSource::External { key: key.clone(), body: Some(o.body.clone()), config: config.clone() }, name: o.body_name.clone(), path: path.clone(), excluded: suppressed, children: Vec::new(), behavior: BomBehavior::default() });
                    }
                }
            },
            InstanceSource::Standard { spec } => {
                let name = result.occurrences.iter().find(|o| o.path == path).map(|o| o.body_name.clone()).unwrap_or_else(|| spec.component.clone());
                out.push(Use { source: BomSource::Standard(spec.clone()), name, path, excluded: suppressed, children: Vec::new(), behavior: BomBehavior::default() });
            }
            // Sketches are not BOM items.
            InstanceSource::Sketch { .. } => {}
            InstanceSource::Assembly { assembly, .. } => {
                let Some(sub) = assembly_of(doc, *assembly) else { continue };
                let mut children = Vec::new();
                walk(doc, sub, &path, result, suppressed, depth + 1, &mut children);
                let name = doc.tab(*assembly).map(|t| t.name.clone()).unwrap_or_default();
                let excluded = suppressed || sub.properties.meta.exclude_from_bom;
                out.push(Use { source: BomSource::Assembly(*assembly), name, path, excluded, children, behavior: sub.properties.bom_behavior });
            }
        }
    }
}

/// Group uses with the same source (and exclusion) into rows, in
/// first-appearance order.
fn group(uses: Vec<Use>) -> Vec<(Use, usize, Vec<InstancePath>)> {
    let mut rows: Vec<(Use, usize, Vec<InstancePath>)> = Vec::new();
    for u in uses {
        if let Some(r) = rows.iter_mut().find(|r| r.0.source == u.source && r.0.excluded == u.excluded) {
            r.1 += 1;
            r.2.push(u.path);
        } else {
            let p = u.path.clone();
            rows.push((u, 1, vec![p]));
        }
    }
    rows
}

/// Flattened: parts at every level; a subassembly set to "Show Assembly
/// only" counts as one item instead of its parts.
fn flatten(uses: Vec<Use>, out: &mut Vec<Use>) {
    for u in uses {
        if u.source.is_assembly() {
            if u.behavior == BomBehavior::AssemblyOnly {
                out.push(Use { children: Vec::new(), ..u });
            } else {
                let excluded = u.excluded;
                let mut kids = Vec::new();
                flatten(u.children, &mut kids);
                for mut k in kids {
                    k.excluded |= excluded;
                    out.push(k);
                }
            }
        } else {
            out.push(u);
        }
    }
}

/// Structured: subassembly rows with their own rows indented below
/// ("Show components only" lifts the components a level up, "Show
/// Assembly only" hides them).
fn structure(uses: Vec<Use>) -> Vec<Use> {
    let mut out = Vec::new();
    for mut u in uses {
        if u.source.is_assembly() {
            let kids = structure(std::mem::take(&mut u.children));
            match u.behavior {
                BomBehavior::ComponentsOnly => {
                    let excluded = u.excluded;
                    out.extend(kids.into_iter().map(|mut k| {
                        k.excluded |= excluded;
                        k
                    }));
                }
                BomBehavior::AssemblyOnly => out.push(u),
                BomBehavior::AssemblyAndComponents => {
                    u.children = kids;
                    out.push(u);
                }
            }
        } else {
            out.push(u);
        }
    }
    out
}

fn emit(uses: Vec<Use>, prefix: &str, depth: usize, settings: &BomSettings, out: &mut Vec<BomLine>) {
    let rows = group(uses);
    let mut n = 0;
    let mut excluded_rows = Vec::new();
    for (u, qty, instances) in rows {
        if u.excluded {
            excluded_rows.push((u, qty, instances));
            continue;
        }
        n += 1;
        let item = if prefix.is_empty() { n.to_string() } else { format!("{prefix}.{n}") };
        let has_children = !u.children.is_empty();
        out.push(BomLine { item: Some(item.clone()), depth, source: u.source, name: u.name, quantity: qty, instances, excluded: false, top_level: false, has_children });
        if has_children {
            emit(u.children, &item, depth + 1, settings, out);
        }
    }
    if settings.show_excluded {
        for (u, qty, instances) in excluded_rows {
            out.push(BomLine { item: Some("--".into()), depth, source: u.source, name: u.name, quantity: qty, instances, excluded: true, top_level: false, has_children: false });
        }
    }
}

/// The BOM rows of Assembly tab `tab` (solved as `result`).
pub fn lines(doc: &Document, tab: TabId, result: &AssemblyResult, settings: &BomSettings) -> Vec<BomLine> {
    let Some(asm) = assembly_of(doc, tab) else { return Vec::new() };
    let mut uses = Vec::new();
    walk(doc, asm, &Vec::new(), result, false, 0, &mut uses);
    let uses = match settings.bom_type {
        BomType::Flattened => {
            let mut flat = Vec::new();
            flatten(uses, &mut flat);
            flat
        }
        BomType::Structured => structure(uses),
    };
    let mut out = Vec::new();
    emit(uses, "", 0, settings, &mut out);
    // The top level assembly row goes at the bottom, its name as the Item.
    if settings.show_top_level {
        let name = doc.tab(tab).map(|t| t.name.clone()).unwrap_or_default();
        out.push(BomLine { item: Some(name.clone()), depth: 0, source: BomSource::Assembly(tab), name, quantity: 1, instances: Vec::new(), excluded: false, top_level: true, has_children: false });
    }
    out
}

/// A cell's content.
#[derive(Clone, Debug, PartialEq)]
pub enum BomCell {
    Text(String),
    Check(bool),
    /// Appearance swatch (linear RGBA).
    Color([f32; 4]),
}

impl BomCell {
    pub fn text(&self) -> String {
        match self {
            BomCell::Text(t) => t.clone(),
            BomCell::Check(b) => b.to_string(),
            BomCell::Color(c) => {
                let s = |v: f32| {
                    let v = v.clamp(0.0, 1.0);
                    let srgb = if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
                    (srgb * 255.0).round() as u8
                };
                format!("#{:02x}{:02x}{:02x}", s(c[0]), s(c[1]), s(c[2]))
            }
        }
    }
}

/// Mass of one part of a row ("No material" without a material).
fn mass_text(result: &AssemblyResult, line: &BomLine, props: Option<&PartProperties>) -> String {
    if let Some(m) = props.and_then(|p| p.mass_override) {
        return format!("{m:.3} kg");
    }
    let Some(density) = props.and_then(|p| p.material.as_ref().and(p.density)) else { return NO_VALUE.into() };
    let Some(path) = line.instances.first() else { return String::new() };
    let body = match &line.source {
        BomSource::Part { body, .. } => body,
        _ => return String::new(),
    };
    let Some(o) = result.occurrences.iter().find(|o| &o.path == path && &o.body == body) else { return String::new() };
    let d = |_: &BodyId| Some(density);
    match crate::regen::measure::mass_properties(&o.context, std::slice::from_ref(&o.body), &d).and_then(|m| m.mass) {
        Some(m) => format!("{m:.3} kg"),
        None => String::new(),
    }
}

/// The content of `col` for `line`.
pub fn cell(doc: &Document, result: &AssemblyResult, line: &BomLine, col: BomColumn) -> BomCell {
    let t = |s: Option<&String>| BomCell::Text(s.cloned().unwrap_or_default());
    match col {
        BomColumn::Item => return BomCell::Text(line.item.clone().unwrap_or_default()),
        BomColumn::Quantity if line.top_level => return BomCell::Text(String::new()),
        BomColumn::Quantity => return BomCell::Text(line.quantity.to_string()),
        BomColumn::Name => return BomCell::Text(line.name.clone()),
        // The top level row (live): N/A where an assembly has no value.
        BomColumn::Appearance | BomColumn::Material | BomColumn::TessellationQuality if line.top_level => return BomCell::Text(NOT_APPLICABLE.into()),
        BomColumn::Mass if line.top_level => return BomCell::Text(NO_VALUE.into()),
        _ => {}
    }
    match &line.source {
        BomSource::Part { studio, body, .. } => {
            let props = part_props(doc, *studio, body);
            let meta = props.map(|p| &p.meta);
            match col {
                BomColumn::PartNumber => t(props.and_then(|p| p.part_number.as_ref())),
                BomColumn::Description => t(props.and_then(|p| p.description.as_ref())),
                BomColumn::Revision => t(props.and_then(|p| p.revision.as_ref())),
                BomColumn::State => BomCell::Text(meta.map_or(RELEASE_STATES[0], |m| m.state()).into()),
                BomColumn::Vendor => t(meta.and_then(|m| m.vendor.as_ref())),
                BomColumn::Project => t(meta.and_then(|m| m.project.as_ref())),
                BomColumn::ProductLine => t(meta.and_then(|m| m.product_line.as_ref())),
                BomColumn::Title1 => t(meta.and_then(|m| m.title1.as_ref())),
                BomColumn::Title2 => t(meta.and_then(|m| m.title2.as_ref())),
                BomColumn::Title3 => t(meta.and_then(|m| m.title3.as_ref())),
                BomColumn::NotRevisionManaged => BomCell::Check(meta.is_some_and(|m| m.not_revision_managed)),
                BomColumn::UnitOfMeasure => BomCell::Text(meta.and_then(|m| m.unit_of_measure.clone()).unwrap_or_else(|| DEFAULT_UNIT_OF_MEASURE.into())),
                BomColumn::Material => BomCell::Text(props.and_then(|p| p.material.clone()).unwrap_or_else(|| NO_MATERIAL.into())),
                // "Default" until the part gets an appearance (live 374).
                BomColumn::Appearance => match props.and_then(|p| p.appearance.as_ref()).map(|a| a.color) {
                    Some(c) => BomCell::Color(c),
                    None => BomCell::Text("Default".into()),
                },
                BomColumn::Mass => BomCell::Text(mass_text(result, line, props)),
                BomColumn::TessellationQuality => BomCell::Text(props.and_then(|p| p.tessellation).unwrap_or(crate::partstudio::TessQuality::Auto).label().into()),
                BomColumn::SubassemblyBomBehavior => BomCell::Text(String::new()),
                BomColumn::Item | BomColumn::Quantity | BomColumn::Name => unreachable!(),
            }
        }
        BomSource::Assembly(_) | BomSource::ExternalAssembly { .. } => {
            // Another document's Assembly: its own properties, read-only.
            let linked = line.instances.first().and_then(|p| result.node(p)).and_then(|n| n.linked.clone());
            let p = match &line.source {
                BomSource::Assembly(tab) => assembly_of(doc, *tab).map(|a| (*a.properties).clone()),
                _ => linked.map(|(_, a)| (*a.properties).clone()),
            };
            let Some(p) = p.as_ref() else { return BomCell::Text(String::new()) };
            match col {
                BomColumn::PartNumber => t(p.part_number.as_ref()),
                BomColumn::Description => t(p.description.as_ref()),
                BomColumn::Revision => t(p.revision.as_ref()),
                BomColumn::State => BomCell::Text(p.meta.state().into()),
                BomColumn::Vendor => t(p.meta.vendor.as_ref()),
                BomColumn::Project => t(p.meta.project.as_ref()),
                BomColumn::ProductLine => t(p.meta.product_line.as_ref()),
                BomColumn::Title1 => t(p.meta.title1.as_ref()),
                BomColumn::Title2 => t(p.meta.title2.as_ref()),
                BomColumn::Title3 => t(p.meta.title3.as_ref()),
                BomColumn::NotRevisionManaged => BomCell::Check(p.meta.not_revision_managed),
                BomColumn::UnitOfMeasure => BomCell::Text(p.meta.unit_of_measure.clone().unwrap_or_else(|| DEFAULT_UNIT_OF_MEASURE.into())),
                BomColumn::SubassemblyBomBehavior => BomCell::Text(p.bom_behavior.label().into()),
                _ => BomCell::Text(String::new()),
            }
        }
        BomSource::Standard(spec) => match col {
            BomColumn::Description => BomCell::Text(spec.part_name()),
            BomColumn::State => BomCell::Text(RELEASE_STATES[0].into()),
            BomColumn::UnitOfMeasure => BomCell::Text(DEFAULT_UNIT_OF_MEASURE.into()),
            _ => BomCell::Text(String::new()),
        },
        BomSource::External { .. } => match col {
            BomColumn::State => BomCell::Text(RELEASE_STATES[0].into()),
            BomColumn::UnitOfMeasure => BomCell::Text(DEFAULT_UNIT_OF_MEASURE.into()),
            _ => BomCell::Text(String::new()),
        },
    }
}

/// Whether a row's cell can be edited (this document's parts and
/// subassemblies only).
pub fn editable(line: &BomLine, col: BomColumn) -> bool {
    if line.top_level && col == BomColumn::SubassemblyBomBehavior {
        return false;
    }
    match &line.source {
        BomSource::Part { .. } => col.edit() != CellEdit::ReadOnly && col != BomColumn::SubassemblyBomBehavior,
        BomSource::Assembly(_) => col.edit() != CellEdit::ReadOnly && col != BomColumn::TessellationQuality,
        BomSource::Standard(_) | BomSource::External { .. } | BomSource::ExternalAssembly { .. } => false,
    }
}

fn opt(v: &str) -> Option<String> {
    (!v.is_empty()).then(|| v.to_string())
}

/// Write an edited cell into the part's (or subassembly's) Properties.
/// Returns false when nothing changed.
pub fn set_cell(doc: &mut Document, line: &BomLine, col: BomColumn, value: &str) -> bool {
    if !editable(line, col) {
        return false;
    }
    match &line.source {
        BomSource::Part { studio, body, .. } => {
            let Some(TabContent::PartStudio(ps)) = doc.tab_mut(*studio).map(|t| &mut t.content) else { return false };
            let p = ps.part_properties.entry(body.clone()).or_default();
            let before = p.clone();
            match col {
                BomColumn::PartNumber => p.part_number = opt(value),
                BomColumn::Description => p.description = opt(value),
                BomColumn::Name => p.name = opt(value),
                BomColumn::Revision => p.revision = opt(value),
                BomColumn::Vendor => p.meta.vendor = opt(value),
                BomColumn::Project => p.meta.project = opt(value),
                BomColumn::ProductLine => p.meta.product_line = opt(value),
                BomColumn::Title1 => p.meta.title1 = opt(value),
                BomColumn::Title2 => p.meta.title2 = opt(value),
                BomColumn::Title3 => p.meta.title3 = opt(value),
                BomColumn::NotRevisionManaged => p.meta.not_revision_managed = value == "true",
                BomColumn::UnitOfMeasure => p.meta.unit_of_measure = (value != DEFAULT_UNIT_OF_MEASURE).then(|| value.to_string()),
                BomColumn::TessellationQuality => {
                    p.tessellation = crate::partstudio::TessQuality::ALL.iter().copied().find(|q| q.label() == value).filter(|q| *q != crate::partstudio::TessQuality::Auto)
                }
                _ => return false,
            }
            *p != before
        }
        BomSource::Assembly(tab) => {
            if col == BomColumn::Name {
                let Some(t) = doc.tab_mut(*tab) else { return false };
                if value.trim().is_empty() || t.name == value {
                    return false;
                }
                t.name = value.trim().to_string();
                return true;
            }
            let Some(TabContent::Assembly(a)) = doc.tab_mut(*tab).map(|t| &mut t.content) else { return false };
            let p = &mut a.properties;
            let before = (**p).clone();
            match col {
                BomColumn::PartNumber => p.part_number = opt(value),
                BomColumn::Description => p.description = opt(value),
                BomColumn::Revision => p.revision = opt(value),
                BomColumn::Vendor => p.meta.vendor = opt(value),
                BomColumn::Project => p.meta.project = opt(value),
                BomColumn::ProductLine => p.meta.product_line = opt(value),
                BomColumn::Title1 => p.meta.title1 = opt(value),
                BomColumn::Title2 => p.meta.title2 = opt(value),
                BomColumn::Title3 => p.meta.title3 = opt(value),
                BomColumn::NotRevisionManaged => p.meta.not_revision_managed = value == "true",
                BomColumn::UnitOfMeasure => p.meta.unit_of_measure = (value != DEFAULT_UNIT_OF_MEASURE).then(|| value.to_string()),
                BomColumn::SubassemblyBomBehavior => match BomBehavior::ALL.iter().copied().find(|b| b.label() == value) {
                    Some(b) => p.bom_behavior = b,
                    None => return false,
                },
                _ => return false,
            }
            **p != before
        }
        _ => false,
    }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Export to CSV's file name.
pub fn csv_file_name(assembly: &str) -> String {
    format!("BOM of {assembly}.csv")
}

/// Whether a cell shows Onshape's pink "No value" (Mass without a material).
pub fn is_missing_value(c: &BomCell, col: BomColumn) -> bool {
    col == BomColumn::Mass && *c == BomCell::Text(NO_VALUE.into())
}

/// The table as text: a header row of column names, then one row per BOM
/// line. `csv`: comma separated with quoting (Export to CSV), else tab
/// separated (Copy table, pastes into spreadsheets).
pub fn table_text(doc: &Document, result: &AssemblyResult, lines: &[BomLine], settings: &BomSettings, csv: bool) -> String {
    let sep = if csv { "," } else { "\t" };
    let field = |s: &str| if csv { csv_field(s) } else { s.replace(['\t', '\n'], " ") };
    let mut out = settings.columns.iter().map(|c| field(c.label())).collect::<Vec<_>>().join(sep);
    out.push('\n');
    for l in lines {
        let row: Vec<String> = settings
            .columns
            .iter()
            .map(|c| {
                let text = cell(doc, result, l, *c).text();
                // Onshape writes the top level row's Item without spaces ("Assembly1").
                if l.top_level && *c == BomColumn::Item { field(&text.replace(' ', "")) } else { field(&text) }
            })
            .collect();
        out.push_str(&row.join(sep));
        out.push('\n');
    }
    out
}
