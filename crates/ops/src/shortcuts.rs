//! Onshape's keyboard shortcuts registry (My account › Preferences ›
//! Keyboard shortcuts; the Shift+/ sheet): every customizable action of the
//! six tabs with Onshape's default key, and the Onshape-controlled (🔒)
//! rows. Shared by the app (crates/app/src/shortcuts.rs: customizing,
//! keymap, the sheet) and the agent API (ui_shortcuts).

/// The tabs of the shortcut lists. The customize page has all but Feature
/// Studio (whose shortcuts are all Onshape controlled).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Tab {
    General,
    PartStudio,
    Assembly,
    View3d,
    Sketch,
    FeatureStudio,
    Drawing,
}

impl Tab {
    /// Tabs of the Preferences page, in order.
    pub const CUSTOMIZE: [Tab; 6] = [Tab::General, Tab::PartStudio, Tab::Assembly, Tab::View3d, Tab::Sketch, Tab::Drawing];
    /// Tabs of the Shift+/ sheet, in order.
    pub const SHEET: [Tab; 7] = [Tab::General, Tab::PartStudio, Tab::Assembly, Tab::View3d, Tab::Sketch, Tab::FeatureStudio, Tab::Drawing];

    pub fn label(self) -> &'static str {
        match self {
            Tab::General => "General",
            Tab::PartStudio => "Part Studio",
            Tab::Assembly => "Assembly",
            Tab::View3d => "3D view",
            Tab::Sketch => "Sketch",
            Tab::FeatureStudio => "Feature Studio",
            Tab::Drawing => "Drawing",
        }
    }

    /// Key context of keys run through the app's `RunShortcut`.
    pub fn run_context(self) -> &'static str {
        match self {
            Tab::General | Tab::View3d | Tab::FeatureStudio => "Workspace && !Input",
            Tab::PartStudio => "PartStudio && !Input",
            Tab::Assembly => "Assembly && !Input",
            Tab::Sketch => "Sketch && !Input",
            Tab::Drawing => "Drawing && !Input",
        }
    }
}

/// A customizable action.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    /// Stable id ("sketch:Line*"), used in the preferences file.
    pub id: &'static str,
    pub tab: Tab,
    /// Onshape's name ("*": also applies in drawing sketches).
    pub name: &'static str,
    /// Onshape's default key (GPUI keystroke syntax), `None` = Unassigned.
    pub default: Option<&'static str>,
    /// The GPUI action `actions::bind_default_keys` binds for it (its
    /// short name); `None`: run through the app's `RunShortcut`.
    pub action: Option<&'static str>,
}

impl Entry {
    /// Applies in drawing sketches too (Onshape's `*`).
    pub fn in_drawing_sketch(&self) -> bool {
        self.tab == Tab::Sketch && self.name.ends_with('*')
    }

    /// The name without the drawing-sketch star.
    pub fn plain_name(&self) -> &'static str {
        self.name.trim_end_matches('*')
    }
}

macro_rules! entries {
    ($( $tab:ident $prefix:literal [ $( ($name:literal, $key:expr, $action:expr) ),* $(,)? ] )*) => {
        &[ $( $( Entry { id: concat!($prefix, ":", $name), tab: Tab::$tab, name: $name, default: $key, action: $action }, )* )* ]
    };
}

const N: Option<&str> = None;

/// Every customizable action, per tab in Onshape's (alphabetical) order.
pub const ENTRIES: &[Entry] = entries! {
    General "general" [
        ("Collapse all in list", N, N),
        ("Connection analysis", N, N),
        ("Create version", N, Some("CreateVersion")),
        ("Curvature color map", N, N),
        ("Curve/surface analysis", Some("shift-c"), Some("CurveSurfaceAnalysis")),
        ("Deviation analysis", N, N),
        ("Dihedral analysis", Some("shift-d"), Some("DihedralAnalysis")),
        ("Draft analysis", N, N),
        ("Expand all in list", N, N),
        ("Flatten surfaces", N, N),
        ("Flip primary axis", Some("a"), Some("FlipPrimaryAxis")),
        ("Hide/show mate connectors", Some("k"), Some("ToggleMateConnectors")),
        ("Interference detection", N, N),
        ("Keyboard shortcuts", Some("shift-/"), Some("ToggleShortcutHelp")),
        ("Mass and section properties", N, N),
        ("Mate connector", Some("ctrl-m"), Some("MateConnectorTool")),
        ("Measure", Some("["), Some("MeasureTool")),
        ("Reflection analysis", N, N),
        ("Rename selection", Some("shift-n"), Some("RenameSelected")),
        ("Reorient secondary axis", Some("q"), Some("ReorientSecondaryAxis")),
        ("Search tools", Some("alt-c"), Some("SearchTools")),
        ("Select other", Some("`"), Some("SelectOther")),
        ("Tab manager", Some("alt-t"), Some("TabManager")),
        ("Zebra stripes", N, N),
    ]
    PartStudio "partStudio" [
        ("3D fit spline", N, N),
        ("Add custom features", N, N),
        ("Bend", N, N),
        ("Bend relief", N, N),
        ("Body draft", N, N),
        ("Boolean", N, N),
        ("Boundary surface", N, N),
        ("Bridging curve", N, N),
        ("Chamfer", N, N),
        ("Circular pattern", N, N),
        ("Composite curve", N, N),
        ("Composite part", N, N),
        ("Constrained surface", N, N),
        ("Corner", N, N),
        ("Corner break", N, N),
        ("Curve pattern", N, N),
        ("Cut list", N, N),
        ("Decal", N, N),
        ("Delete face", N, N),
        ("Delete part", N, N),
        ("Derived", N, N),
        ("Draft", N, N),
        ("Edit curve", N, N),
        ("Enclose", N, N),
        ("End cap", N, N),
        ("External thread", N, N),
        ("Extrude", Some("shift-e"), Some("Extrude")),
        ("Face blend", N, N),
        ("Fill", N, N),
        ("Fillet", Some("shift-f"), Some("Fillet")),
        ("Finish flex PCB model", N, N),
        ("Finish sheet metal model", N, N),
        ("Flange", N, N),
        ("Flex PCB model", N, N),
        ("Form", N, N),
        ("Frame", N, N),
        ("Frame trim", N, N),
        ("Gusset", N, N),
        ("Helix", N, N),
        ("Hem", N, N),
        ("Hide/show sketches", Some("shift-h"), Some("ToggleSketches")),
        ("Hole", N, N),
        ("Intersection curve", N, N),
        ("Isocline", N, N),
        ("Isoparameteric curve", N, N),
        ("Jog", N, N),
        ("Linear pattern", N, N),
        ("Loft", N, N),
        ("Make joint", N, N),
        ("Mirror", N, N),
        ("Modify fillet", N, N),
        ("Move boundary", N, N),
        ("Move face", N, N),
        ("Mutual trim", N, N),
        ("Offset curve", N, N),
        ("Offset surface", N, N),
        ("Plane", N, N),
        ("Projected curve", N, N),
        ("Query variable", N, N),
        ("Replace face", N, N),
        ("Revolve", Some("shift-w"), Some("Revolve")),
        ("Rib", N, N),
        ("Routing curve", N, N),
        ("Ruled surface", N, N),
        ("Sheet metal loft", N, N),
        ("Sheet metal model", N, N),
        ("Shell", N, N),
        ("Sketch", Some("shift-s"), Some("NewSketch")),
        ("Split", N, N),
        ("Sweep", N, N),
        ("Tab", N, N),
        ("Tag", N, N),
        ("Thicken", N, N),
        ("Transform", N, N),
        ("Trim curve", N, N),
        ("Variable", N, N),
        ("Wrap", N, N),
    ]
    Assembly "assembly" [
        ("Assembly circular pattern", N, N),
        ("Assembly linear pattern", N, N),
        ("Assembly mirror", N, N),
        ("Create Part Studio in context", N, N),
        ("Display states", N, N),
        ("Group", N, N),
        ("Hide/show mates", Some("j"), Some("ToggleMates")),
        ("Insert dialog", Some("i"), Some("InsertInstance")),
        ("Mate", Some("m"), Some("FastenMate")),
        ("Mate relation", N, N),
        ("Replace instance", N, N),
        ("Replicate", N, N),
        ("Show mates mode", Some("h"), Some("ToggleShowMatesMode")),
        ("Snap mode", Some("shift-s"), Some("ToggleSnapMode")),
        ("Tangent mate", N, N),
        ("Width mate", N, N),
    ]
    View3d "view3d" [
        ("Back view", Some("shift-2"), Some("ViewBack")),
        ("Bottom view", Some("shift-6"), Some("ViewBottom")),
        ("Dimetric", N, N),
        ("Front view", Some("shift-1"), Some("ViewFront")),
        ("Graphics preferences", N, N),
        ("Hide all construction", Some("shift-p"), Some("HideConstruction")),
        ("Hide selected parts", Some("y"), Some("HideSelected")),
        ("Hide/show hidden edges", N, N),
        ("Hide/show planes", Some("p"), Some("TogglePlanes")),
        ("Highlight boundary edges", N, N),
        ("Isometric view", Some("shift-7"), Some("ViewIsometric")),
        ("Left view", Some("shift-3"), Some("ViewLeft")),
        ("Make isolate", Some("shift-i"), Some("IsolateSelected")),
        ("Make transparent", Some("shift-t"), Some("TransparentSelected")),
        ("Named view", Some("shift-v"), Some("NamedViews")),
        // No key in Onshape: only a customized key runs it (RunShortcut).
        ("Perspective view", N, N),
        ("Previous view", N, N),
        ("Right view", Some("shift-4"), Some("ViewRight")),
        ("Section view", Some("shift-x"), Some("SectionView")),
        ("Shaded with edges", N, N),
        ("Shaded without edges", N, N),
        ("Show selected parts", Some("shift-y"), Some("ShowAll")),
        ("Tangent edges hidden", N, N),
        ("Tangent edges phantom", N, N),
        ("Tangent edges visible", N, N),
        ("Top view", Some("shift-5"), Some("ViewTop")),
        ("Translucent", N, N),
        ("Trimetric", N, N),
        ("Unshaded", N, N),
        ("View in high quality", Some("shift-r"), Some("ViewHighQuality")),
        ("View normal to", Some("n"), Some("ViewNormal")),
        ("Zoom in", Some("shift-z"), Some("ZoomIn")),
        ("Zoom out", Some("z"), Some("ZoomOut")),
        ("Zoom to fit", Some("f"), Some("ZoomToFit")),
        ("Zoom to window", Some("w"), Some("ZoomWindow")),
    ]
    Sketch "sketch" [
        ("3 point arc", Some("a"), Some("SketchArc3")),
        ("3 point circle*", N, N),
        ("Aligned rectangle", N, N),
        ("Bezier", N, N),
        ("Center point arc", N, N),
        ("Center point circle*", Some("c"), Some("SketchCircle")),
        ("Center point rectangle*", Some("r"), Some("SketchCenterRectangle")),
        ("Circular pattern", N, N),
        ("Circumscribed polygon", N, N),
        ("Coincident*", Some("i"), Some("ConstraintCoincident")),
        ("Concentric", Some("shift-o"), Some("ConstraintConcentric")),
        ("Conic", N, N),
        ("Constraint manager", N, N),
        ("Corner rectangle*", Some("g"), Some("SketchCornerRectangle")),
        ("Curvature", Some("shift-u"), Some("ConstraintCurvature")),
        ("Dimension*", Some("d"), Some("SketchDimension")),
        ("Ellipse", N, N),
        ("Elliptical arc", N, N),
        ("Equal", Some("e"), Some("ConstraintEqual")),
        ("Extend", Some("x"), Some("SketchExtend")),
        ("Fix", Some("shift-j"), Some("ConstraintFix")),
        ("Horizontal*", Some("h"), Some("ConstraintHorizontal")),
        ("Inscribed polygon", N, N),
        ("Insert DXF or DWG", N, N),
        ("Insert image", N, N),
        ("Intersection", N, N),
        ("Line*", Some("l"), Some("SketchLine")),
        ("Line/tangent arc switch", Some("shift-a"), Some("SketchTangentArcToggle")),
        ("Linear pattern", N, N),
        ("Midpoint", Some("shift-m"), Some("ConstraintMidpoint")),
        ("Midpoint line", N, N),
        ("Mirror sketch", N, N),
        ("Normal", Some("shift-k"), Some("ConstraintNormal")),
        ("Offset", Some("o"), Some("SketchOffset")),
        ("Parallel*", Some("b"), Some("ConstraintParallel")),
        ("Perpendicular*", Some("shift-l"), Some("ConstraintPerpendicular")),
        ("Pierce", Some("shift-g"), Some("ConstraintPierce")),
        ("Point", Some("shift-s"), Some("SketchPoint")),
        ("Profile inspector", N, N),
        ("Sketch chamfer", N, N),
        ("Sketch fillet", Some("shift-f"), Some("SketchFillet")),
        ("Slot", N, N),
        ("Spline", N, N),
        ("Spline control point", N, N),
        ("Split", N, N),
        ("Symmetric", Some("shift-q"), Some("ConstraintSymmetric")),
        ("Tangent*", Some("t"), Some("ConstraintTangent")),
        ("Tangent arc*", N, N),
        ("Text", N, N),
        ("Toggle construction", Some("q"), Some("SketchConstruction")),
        ("Transform", N, N),
        ("Trim*", Some("m"), Some("SketchTrim")),
        ("Use", Some("u"), Some("SketchUse")),
        ("Vertical*", Some("v"), Some("ConstraintVertical")),
    ]
    Drawing "drawing" [
        ("2-point centerline", N, N),
        ("2-point circle centerline", N, N),
        ("2-point linear dimension", N, N),
        ("3-point angular dimension", N, N),
        ("3-point circle centerline", N, N),
        ("Aligned section view", N, N),
        ("Arc length dimension", N, N),
        ("Auxiliary view", N, N),
        ("BOM table", N, N),
        ("Break view", N, N),
        ("Broken-out section", N, N),
        ("Callout", N, N),
        ("Centermark", N, N),
        ("Chamfer dimension", N, N),
        ("Crop view", N, N),
        ("Custom table", N, N),
        ("Cut list table", N, N),
        ("Datum", N, N),
        ("Detail view", N, N),
        ("Diameter dimension", Some("shift-d"), N),
        ("Display sheet menu", Some("ctrl-s"), N),
        ("Drawing properties panel", N, N),
        ("Edge to edge centerline", N, N),
        ("First sheet", Some("home"), N),
        ("Foreshorten dimension", Some("shift-h"), N),
        ("Format painter", N, N),
        ("Geometric tolerance", N, N),
        ("Hatch region", N, N),
        ("Hide inspection items", N, N),
        ("Hole table", N, N),
        ("Hole/thread callout", N, N),
        ("Insert DXF or DWG", N, N),
        ("Insert image", N, N),
        ("Insert view", N, N),
        ("Inspection item", N, N),
        ("Last sheet", Some("end"), N),
        ("Line to line angular dimension", N, N),
        ("Line to line dimension", N, N),
        ("Max/min dimension", Some("ctrl-m"), N),
        ("Next sheet", Some("pagedown"), N),
        ("Note", Some("n"), N),
        ("Ordinate dimension", N, N),
        ("Point to line dimension", N, N),
        ("Previous sheet", Some("pageup"), N),
        ("Projected view", Some("p"), N),
        ("Quad & mid snap point", Some("shift-q"), N),
        ("Radial dimension", Some("shift-r"), N),
        ("Revision table", N, N),
        ("Section view", N, N),
        ("Sketch point", N, N),
        ("Spline", N, N),
        ("Spline point", N, N),
        ("Surface finish symbol", N, N),
        ("Table", N, N),
        ("Update drawing", Some("ctrl-q"), N),
        ("Virtual sharp", N, N),
        ("Weld symbol", N, N),
    ]
};

/// Onshape-controlled (🔒) shortcuts of the Shift+/ sheet: tab, keys as
/// Onshape prints them (space-separated chips, " / " between
/// alternatives), action.
pub const LOCKED: &[(Tab, &str, &str)] = &[
    (Tab::General, "shift enter", "Accept & repeat command"),
    (Tab::General, "enter", "Accept command"),
    (Tab::General, "escape", "Cancel"),
    (Tab::General, "space", "Clear selection"),
    (Tab::General, "ctrl/cmd c", "Copy"),
    (Tab::General, "delete / backspace", "Delete selection"),
    (Tab::General, "ctrl/cmd shift f", "FeatureScript search"),
    (Tab::General, "ctrl u", "Feedback/Report a bug"),
    (Tab::General, "shift", "Lock mate inference"),
    (Tab::General, "ctrl/cmd click", "Open in new tab"),
    (Tab::General, "shift click", "Open in new window"),
    (Tab::General, "ctrl/cmd v", "Paste"),
    (Tab::General, "ctrl space", "Quick tab switching"),
    (Tab::General, "ctrl y / shift cmd z", "Redo"),
    (Tab::General, "s", "Shortcut toolbars"),
    (Tab::General, "ctrl/cmd z", "Undo"),
    (Tab::PartStudio, "select rollback bar ↑ ↓", "Move rollback bar"),
    (Tab::View3d, "shift ← → ↑ ↓", "Coarse rotate"),
    (Tab::View3d, "ctrl shift ← → ↑ ↓", "Pan"),
    (Tab::View3d, "ctrl ← → ↑ ↓", "Precision rotate"),
    (Tab::View3d, "← → ↑ ↓", "Rotate"),
    (Tab::View3d, "alt/⌥ click", "Select through transparent"),
    (Tab::Sketch, "shift", "Suppress inferencing"),
    (Tab::FeatureStudio, "ctrl/cmd [", "Back reference"),
    (Tab::FeatureStudio, "ctrl/cmd s", "Commit"),
    (Tab::FeatureStudio, "ctrl/cmd shift s", "Commit all"),
    (Tab::FeatureStudio, "escape", "Dismiss autocomplete"),
    (Tab::FeatureStudio, "ctrl/cmd ]", "Forward reference"),
    (Tab::FeatureStudio, "ctrl/cmd shift o", "Top level symbols dropdown"),
    (Tab::Drawing, "alt/⌥ lmb drag", "Copy annotation"),
];

pub fn entry(id: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.id == id)
}

pub fn entries(tab: Tab) -> impl Iterator<Item = &'static Entry> {
    ENTRIES.iter().filter(move |e| e.tab == tab)
}
