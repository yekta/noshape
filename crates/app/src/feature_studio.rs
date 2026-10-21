//! Feature Studio tab: a script editor for the document's custom features
//! (noshape's FeatureScript equivalent). As in Onshape, edits are saved as
//! you type but only Commit (ctrl+s; Commit all: shift+ctrl+s) applies them
//! to the document's Part Studios. Toolbar: undo/redo, the Commit split
//! button, Search tools…; above the code: Module outline ▾, back/forward
//! and the Search Feature Studios button (the search pane is the
//! workspace's, under the tab bar: `fs_search.rs`).

use crate::doc_model::{DocEvent, DocModel};
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, HighlightStyleResolver, Input, InputEvent, InputHighlighter, InputState, Position, TabSize};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::TabContent;
use noshape_model::custom::Severity;
use noshape_model::document::TabId;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Duration;

/// A popup anchor: the trigger's bounds from the last paint.
type BoundsCell = Rc<std::cell::Cell<Bounds<Pixels>>>;

actions!(feature_studio, [Commit, CommitAll, SearchStudios, ModuleOutline, BackReference, ForwardReference]);

/// Onshape's Feature Studio shortcuts (live dom/keyboard-shortcuts.txt):
/// Commit, Commit all, Top level symbols dropdown (the Module outline),
/// Back / Forward reference. The code editor's own ctrl+[ / ctrl+]
/// (outdent / indent) give way to the references, as in Onshape, so those
/// are bound again at the editor (registered after the kit's, they win).
pub fn bind_keys(cx: &mut App) {
    let c = Some("FeatureStudio");
    let code = Some("FeatureStudio > Input");
    cx.bind_keys([
        KeyBinding::new("secondary-s", Commit, c),
        KeyBinding::new("secondary-shift-s", CommitAll, c),
        KeyBinding::new("secondary-shift-f", SearchStudios, c),
        KeyBinding::new("secondary-shift-o", ModuleOutline, c),
        KeyBinding::new("secondary-[", BackReference, c),
        KeyBinding::new("secondary-]", ForwardReference, c),
        KeyBinding::new("secondary-shift-o", ModuleOutline, code),
        KeyBinding::new("secondary-[", BackReference, code),
        KeyBinding::new("secondary-]", ForwardReference, code),
    ]);
}

/// Pause after the last keystroke before the text is saved (not committed).
const SAVE_DELAY: Duration = Duration::from_millis(700);

/// Requests for the workspace.
pub enum FeatureStudioEvent {
    /// Open (or focus) the Search Feature Studios pane.
    OpenSearch,
    /// Right-click › Find usages / Rename in workspace: a symbol search for
    /// `name`, found at `at` ("Feature Studio 1 - Line 19, Column 13").
    FindSymbol { name: String, at: String, rename: bool },
    /// Compiling the saved text produced new notices (the notices panel and
    /// the navbar { } icon follow).
    NoticesChanged,
    /// Ctrl+click on a standard library symbol: open std's source there
    /// (module, 1-based line and column).
    OpenStd { module: String, line: usize, column: usize },
}

/// Search matches to highlight in each Feature Studio (set by the search
/// pane; read by the highlighter).
static MATCHES: Mutex<Vec<(TabId, Vec<Range<usize>>)>> = Mutex::new(Vec::new());

pub fn set_matches(tab: TabId, ranges: Vec<Range<usize>>) {
    let mut m = MATCHES.lock().unwrap_or_else(|e| e.into_inner());
    m.retain(|(t, _)| *t != tab);
    if !ranges.is_empty() {
        m.push((tab, ranges));
    }
}

fn matches_of(tab: TabId) -> Vec<Range<usize>> {
    MATCHES.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(t, _)| *t == tab).map(|(_, r)| r.clone()).unwrap_or_default()
}

/// A place in the back / forward reference history. Live
/// (dom/fs-nav-shortcuts.txt) the ▾ lists show "<tab>: <line>, <column>"
/// for caret positions, "<tab>: Symbol - <name>" for jump targets and
/// "<tab>: Use - <name>" for the ctrl+clicked use.
#[derive(Clone)]
struct NavEntry {
    pos: Position,
    label: String,
}

impl NavEntry {
    fn caret(pos: Position) -> Self {
        NavEntry { pos, label: format!("{}, {}", pos.line + 1, pos.character + 1) }
    }
}

/// A top-level symbol in the Module outline: name, parameter list, 1-based line.
#[derive(Clone)]
struct Symbol {
    name: String,
    params: String,
    line: usize,
    /// 0-based column of the name.
    column: usize,
}

/// [`outline`] of an editor's text, parsed again only when the text changes:
/// the toolbar shows the current symbol on every render (each keystroke and
/// caret blink), and parsing the module took ~0.7 ms. Keyed by a hash of the
/// text, so several Feature Studios share it.
fn outline_of(editor: &EditorState) -> Rc<Vec<Symbol>> {
    use std::hash::{Hash, Hasher};
    thread_local! {
        static CACHE: std::cell::RefCell<Vec<(u64, Rc<Vec<Symbol>>)>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    let text = editor.text();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for chunk in text.chunks() {
        chunk.hash(&mut h);
    }
    let key = h.finish();
    CACHE.with_borrow_mut(|cache| {
        if let Some((_, s)) = cache.iter().find(|(k, _)| *k == key) {
            return s.clone();
        }
        let symbols = Rc::new(outline(&text.to_string()));
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((key, symbols.clone()));
        symbols
    })
}

/// FeatureScript top-level symbols: `export const myFeature =
/// defineFeature(function(context is Context, …)` and `function f(a, b)`
/// (Onshape's outline shows "myFeature(context, i…").
fn outline(src: &str) -> Vec<Symbol> {
    let params_of = |rest: &str| -> String {
        let Some(open) = rest.find('(') else { return String::new() };
        let close = rest[open..].find(')').map(|c| open + c).unwrap_or(rest.len());
        rest[open + 1..close].split(',').map(|p| p.split(" is ").next().unwrap_or("").trim().to_string()).collect::<Vec<_>>().join(", ")
    };
    src.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let t = l.trim_start();
            let t = t.strip_prefix("export ").unwrap_or(t);
            if let Some(rest) = t.strip_prefix("const ") {
                let (name, value) = rest.split_once('=')?;
                let inner = value.split_once("function").map(|(_, r)| r).unwrap_or("");
                let name = name.trim().to_string();
                let column = column_of(l, &name);
                return Some(Symbol { name, params: params_of(inner), line: i + 1, column });
            }
            let rest = t.strip_prefix("function ").or_else(|| t.strip_prefix("predicate "))?;
            let open = rest.find('(')?;
            let name = rest[..open].trim().to_string();
            let column = column_of(l, &name);
            Some(Symbol { name, params: params_of(rest), line: i + 1, column })
        })
        .collect()
}

/// Onshape's snippet menus (live dom/feature-studio-toolbar-menus.txt):
/// tooltip, the button icon, and per item the FeatureScript
/// it inserts (the standard library's calls with example arguments).
type Snippets = &'static [(&'static str, &'static str)];
const SNIPPET_MENUS: [(&str, &str, Snippets); 6] = [
    (
        "Feature parameters",
        "fs-snip-p",
        &[
            ("Length parameter", "annotation { \"Name\" : \"My length\" }\nisLength(definition.myLength, LENGTH_BOUNDS);"),
            ("Angle parameter", "annotation { \"Name\" : \"My angle\" }\nisAngle(definition.myAngle, ANGLE_360_BOUNDS);"),
            ("Count parameter", "annotation { \"Name\" : \"My count\" }\nisInteger(definition.myCount, POSITIVE_COUNT_BOUNDS);"),
            ("Query parameter", "annotation { \"Name\" : \"My query\", \"Filter\" : EntityType.FACE, \"MaxNumberOfPicks\" : 1 }\ndefinition.myQuery is Query;"),
            ("Enum parameter", "annotation { \"Name\" : \"My enum\" }\ndefinition.myEnum is MyEnum;"),
            ("Boolean parameter", "annotation { \"Name\" : \"My boolean\" }\ndefinition.myBoolean is boolean;"),
            ("String parameter", "annotation { \"Name\" : \"My string\" }\ndefinition.myString is string;"),
            ("Array parameter", "annotation { \"Name\" : \"My array\", \"Item name\" : \"Item\", \"Item label template\" : \"#myParameter\" }\ndefinition.myArray is array;\nfor (var item in definition.myArray)\n{\n    annotation { \"Name\" : \"My parameter\" }\n    item.myParameter is boolean;\n}"),
            ("Selection-driven array parameter", "annotation { \"Name\" : \"My array\", \"Item name\" : \"Item\", \"Driven query\" : \"selection\", \"Item label template\" : \"#selection\" }\ndefinition.myArray is array;\nfor (var item in definition.myArray)\n{\n    annotation { \"Name\" : \"Selection\", \"Filter\" : EntityType.FACE, \"MaxNumberOfPicks\" : 1 }\n    item.selection is Query;\n}"),
            ("Part Studio reference parameter", "annotation { \"Name\" : \"My Part Studio\" }\ndefinition.myPartStudio is PartStudioData;"),
            ("Variable Studio reference parameter", "annotation { \"Name\" : \"My Variable Studio\" }\ndefinition.myVariableStudio is VariableStudioReference;"),
            ("Image reference parameter", "annotation { \"Name\" : \"My image\" }\ndefinition.myImage is ImageData;"),
            ("CSV reference parameter", "annotation { \"Name\" : \"My CSV\" }\ndefinition.myCsv is TableData;"),
            ("JSON reference parameter", "annotation { \"Name\" : \"My JSON\" }\ndefinition.myJson is JsonData;"),
            ("Parameter group", "annotation { \"Group Name\" : \"My group\", \"Collapsed By Default\" : false }\n{\n}"),
        ],
    ),
    (
        "Query functions",
        "fs-snip-q",
        &[
            ("qEverything", "qEverything(EntityType.BODY)"),
            ("qCreatedBy", "qCreatedBy(id + \"extrude1\", EntityType.FACE)"),
            ("qBodyType", "qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)"),
            ("qGeometry", "qGeometry(qEverything(EntityType.FACE), GeometryType.PLANE)"),
            ("qSketchRegion", "qSketchRegion(id + \"sketch1\")"),
            ("qCapEntity", "qCapEntity(id + \"extrude1\", CapType.START, EntityType.FACE)"),
            ("qContainsPoint", "qContainsPoint(qEverything(EntityType.BODY), vector(0, 0, 0) * meter)"),
            ("qIntersectsPlane", "qIntersectsPlane(qEverything(EntityType.EDGE), plane(vector(0, 0, 0) * meter, vector(0, 0, 1)))"),
            ("qAdjacent", "qAdjacent(query, AdjacencyType.EDGE, EntityType.FACE)"),
            ("qUnion", "qUnion([query1, query2])"),
            ("qIntersection", "qIntersection([query1, query2])"),
            ("qSubtraction", "qSubtraction(query1, query2)"),
            ("evaluateQuery", "evaluateQuery(context, query)"),
        ],
    ),
    (
        "Evaluate functions",
        "fs-snip-e",
        &[
            ("evVertexPoint", "evVertexPoint(context, { \"vertex\" : query })"),
            ("evLine", "evLine(context, { \"edge\" : query })"),
            ("evAxis", "evAxis(context, { \"axis\" : query })"),
            ("evPlane", "evPlane(context, { \"face\" : query })"),
            ("evOwnerSketchPlane", "evOwnerSketchPlane(context, { \"entity\" : query })"),
            ("evEdgeTangentLine", "evEdgeTangentLine(context, { \"edge\" : query, \"parameter\" : 0.5 })"),
            ("evFaceTangentPlane", "evFaceTangentPlane(context, { \"face\" : query, \"parameter\" : vector(0.5, 0.5) })"),
            ("evDistance", "evDistance(context, { \"side0\" : query1, \"side1\" : query2 })"),
            ("evLength", "evLength(context, { \"entities\" : query })"),
            ("evArea", "evArea(context, { \"entities\" : query })"),
            ("evVolume", "evVolume(context, { \"entities\" : query })"),
            ("evApproximateCentroid", "evApproximateCentroid(context, { \"entities\" : query })"),
            ("evApproximateMassProperties", "evApproximateMassProperties(context, { \"entities\" : query, \"density\" : 1 * kilogram / meter ^ 3 })"),
            ("evBox3d", "evBox3d(context, { \"topology\" : query })"),
        ],
    ),
    (
        "Math functions",
        "fs-snip-math",
        &[
            ("vector", "vector(0, 0, 0)"),
            ("line", "line(vector(0, 0, 0) * meter, vector(0, 0, 1))"),
            ("plane", "plane(vector(0, 0, 0) * meter, vector(0, 0, 1))"),
            ("coordSystem", "coordSystem(vector(0, 0, 0) * meter, vector(1, 0, 0), vector(0, 0, 1))"),
            ("transform", "transform(vector(0, 0, 0) * meter)"),
            ("rotationAround", "rotationAround(line(vector(0, 0, 0) * meter, vector(0, 0, 1)), 90 * degree)"),
            ("scaleUniformly", "scaleUniformly(2)"),
            ("fromWorld", "fromWorld(cSys)"),
            ("toWorld", "toWorld(cSys)"),
        ],
    ),
    (
        "Sketch functions",
        "fs-snip-sketch",
        &[
            ("Sketch", "var sketch1 = newSketch(context, id + \"sketch1\", {\n        \"sketchPlane\" : qCreatedBy(makeId(\"Top\"), EntityType.FACE)\n});\n// Create sketch entities here\nskSolve(sketch1);"),
            ("skLineSegment", "skLineSegment(sketch1, \"line1\", {\n        \"start\" : vector(0, 0) * inch,\n        \"end\" : vector(1, 1) * inch\n});"),
            ("skCircle", "skCircle(sketch1, \"circle1\", {\n        \"center\" : vector(0, 0) * inch,\n        \"radius\" : 1 * inch\n});"),
            ("skEllipse", "skEllipse(sketch1, \"ellipse1\", {\n        \"center\" : vector(0, 0) * inch,\n        \"majorRadius\" : 2 * inch,\n        \"minorRadius\" : 1 * inch\n});"),
            ("skArc", "skArc(sketch1, \"arc1\", {\n        \"start\" : vector(1, 0) * inch,\n        \"mid\" : vector(0, 1) * inch,\n        \"end\" : vector(-1, 0) * inch\n});"),
            ("skEllipticalArc", "skEllipticalArc(sketch1, \"ellipticalArc1\", {\n        \"center\" : vector(0, 0) * inch,\n        \"majorAxis\" : vector(1, 0),\n        \"majorRadius\" : 2 * inch,\n        \"minorRadius\" : 1 * inch,\n        \"startParameter\" : 0,\n        \"endParameter\" : 0.5\n});"),
            ("skFitSpline", "skFitSpline(sketch1, \"spline1\", {\n        \"points\" : [\n            vector(0, 0) * inch,\n            vector(1, 1) * inch,\n            vector(2, 0) * inch\n        ]\n});"),
            ("skRegularPolygon", "skRegularPolygon(sketch1, \"polygon1\", {\n        \"center\" : vector(0, 0) * inch,\n        \"firstVertex\" : vector(1, 0) * inch,\n        \"sides\" : 6\n});"),
            ("skPolyline", "skPolyline(sketch1, \"polyline1\", {\n        \"points\" : [\n            vector(0, 0) * inch,\n            vector(1, 0) * inch,\n            vector(1, 1) * inch\n        ]\n});"),
        ],
    ),
    (
        "Operation functions",
        "fs-snip-op",
        &[
            ("fCuboid", "fCuboid(context, id + \"cuboid1\", {\n        \"corner1\" : vector(0, 0, 0) * inch,\n        \"corner2\" : vector(1, 1, 1) * inch\n});"),
            ("fCylinder", "fCylinder(context, id + \"cylinder1\", {\n        \"topCenter\" : vector(0, 0, 1) * inch,\n        \"bottomCenter\" : vector(0, 0, 0) * inch,\n        \"radius\" : 0.5 * inch\n});"),
            ("fSphere", "fSphere(context, id + \"sphere1\", {\n        \"center\" : vector(0, 0, 0) * inch,\n        \"radius\" : 1 * inch\n});"),
            ("opExtrude", "opExtrude(context, id + \"extrude1\", {\n        \"entities\" : qSketchRegion(id + \"sketch1\"),\n        \"direction\" : evOwnerSketchPlane(context, { \"entity\" : qSketchRegion(id + \"sketch1\") }).normal,\n        \"endBound\" : BoundingType.BLIND,\n        \"endDepth\" : 1 * inch\n});"),
            ("opRevolve", "opRevolve(context, id + \"revolve1\", {\n        \"entities\" : qSketchRegion(id + \"sketch1\"),\n        \"axis\" : line(vector(0, 0, 0) * inch, vector(0, 0, 1)),\n        \"angleForward\" : 360 * degree\n});"),
            ("opThicken", "opThicken(context, id + \"thicken1\", {\n        \"entities\" : query,\n        \"thickness1\" : 0.1 * inch,\n        \"thickness2\" : 0 * inch\n});"),
            ("opTransform", "opTransform(context, id + \"transform1\", {\n        \"bodies\" : query,\n        \"transform\" : transform(vector(1, 0, 0) * inch)\n});"),
            ("opPattern", "opPattern(context, id + \"pattern1\", {\n        \"entities\" : query,\n        \"transforms\" : [transform(vector(1, 0, 0) * inch)],\n        \"instanceNames\" : [\"1\"]\n});"),
            ("opPlane", "opPlane(context, id + \"plane1\", {\n        \"plane\" : plane(vector(0, 0, 0) * inch, vector(0, 0, 1))\n});"),
            ("opDeleteBodies", "opDeleteBodies(context, id + \"deleteBodies1\", {\n        \"entities\" : query\n});"),
        ],
    ),
];

/// A new Feature Studio (live document/70).
pub const FS_TEMPLATE: &str = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n\n\n";

/// "New feature" (live document/72, dom/feature-studio-template-and-error.txt).
const NEW_FEATURE: &str = "annotation { \"Feature Type Name\" : \"My Feature\", \"Feature Type Description\" : \"\" }\nexport const myFeature = defineFeature(function(context is Context, id is Id, definition is map)\n    precondition\n    {\n        // Define the parameters of the feature type\n    }\n    {\n        // Define the function's action\n    });\n";

/// A notice of a Feature Studio (FeatureScript notices panel, editor squiggles).
#[derive(Clone, Debug)]
pub struct Notice {
    /// 1-based; 0: no location.
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub error: bool,
    /// An info notice (ⓘ, e.g. "Variable m set but not used").
    pub info: bool,
}

/// Characters Ace's squiggle covers from a notice's 0-based column: the
/// identifier or number starting there (live fs-squiggles-zoom8x: `f`, `x`,
/// `undefinedThing`), else one character.
fn squiggle_len(line: &str, column: usize) -> usize {
    let word = line.chars().skip(column).take_while(|c| c.is_alphanumeric() || *c == '_').count();
    word.max(1)
}

/// **Compile hook** (agent fs plugs the FeatureScript parser / checker in
/// here): the notices of a Feature Studio's source. Called on every save
/// for the editor and on demand by the notices panel.
pub fn studio_notices(tab: TabId, src: &str) -> Vec<Notice> {
    noshape_model::custom::compile(tab, src)
        .diagnostics
        .iter()
        .map(|d| Notice { line: d.line, column: d.column, message: d.message.clone(), error: d.severity == Severity::Error, info: d.severity == Severity::Info })
        .collect()
}

/// Format Feature Studio: re-indent by brace depth (4 spaces), trim line ends.
fn format_source(src: &str) -> String {
    let mut depth: i32 = 0;
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let t = line.trim();
        let opens = t.chars().filter(|c| matches!(c, '{' | '[' | '(')).count() as i32;
        let closes = t.chars().filter(|c| matches!(c, '}' | ']' | ')')).count() as i32;
        let leading_closes = t.chars().take_while(|c| matches!(c, '}' | ']' | ')')).count() as i32;
        let indent = (depth - leading_closes).max(0) as usize;
        if !t.is_empty() {
            out.push_str(&"    ".repeat(indent));
            out.push_str(t);
        }
        out.push('\n');
        depth = (depth + opens - closes).max(0);
    }
    out
}

/// Character column of `name` (as a whole word) in `line`.
fn column_of(line: &str, name: &str) -> usize {
    let is_id = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(i) = line[from..].find(name).map(|i| i + from) {
        let before = line[..i].chars().next_back();
        let after = line[i + name.len()..].chars().next();
        if !before.is_some_and(is_id) && !after.is_some_and(is_id) {
            return line[..i].chars().count();
        }
        from = i + name.len();
    }
    0
}

/// Where `name` is defined in a studio's text: a top-level or nested
/// `function` / `predicate` / `const` / `enum` / `type` / `operator`
/// declaration (the first one), at the name.
fn definition_of(src: &str, name: &str) -> Option<Position> {
    for (i, l) in src.lines().enumerate() {
        let t = l.trim_start();
        let t = t.strip_prefix("export ").unwrap_or(t);
        let decl = ["function ", "predicate ", "const ", "enum ", "type "].iter().find_map(|k| t.strip_prefix(k));
        let Some(rest) = decl else { continue };
        let declared: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if declared == name {
            return Some(Position { line: i as u32, character: column_of(l, name) as u32 });
        }
    }
    None
}

pub struct FeatureStudioView {
    tab: TabId,
    doc: Entity<DocModel>,
    editor: Entity<EditorState>,
    /// Text as last saved to / loaded from the document.
    synced: String,
    /// Notices of the saved text (`studio_notices`), shown as squiggles.
    notices: Vec<Notice>,
    save_task: Option<Task<()>>,
    compile_task: Option<Task<()>>,
    /// Module outline popover: its "Search symbols…" field.
    outline: Option<Entity<InputState>>,
    /// Places visited by jumps (back / forward), oldest first.
    nav: Vec<NavEntry>,
    /// The caret before the last click in the code (a ctrl+click records it).
    caret_before_click: Option<Position>,
    nav_at: usize,
    focus: FocusHandle,
    /// The toolbar's Import panel and the button's bounds (it hangs under it).
    import_panel: Option<(Entity<crate::fs_import::ImportPanel>, Subscription)>,
    import_anchor: BoundsCell,
    /// The toolbar's fit-to-width stage and its measurements (see
    /// `toolbar_stage`).
    tb: ToolbarFit,
    /// The Module outline button's bounds (its popover hangs under it).
    outline_anchor: BoundsCell,
    /// The pointer over the code (the diagnostic tooltip follows it).
    hover_at: Option<Point<Pixels>>,
    /// The toolbar as a cached view (see `render`).
    toolbar: Option<Entity<crate::cached_slice::Slice<FeatureStudioView>>>,
    _subs: Vec<Subscription>,
}

impl EventEmitter<FeatureStudioEvent> for FeatureStudioView {}

impl Focusable for FeatureStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl FeatureStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = text_of(&doc, tab, cx).unwrap_or_default();
        let editor = cx.new(|cx| {
            let mut st = EditorState::new(window, cx)
                .language("featurescript")
                .line_number(true)
                .soft_wrap(false)
                .tab_size(TabSize { tab_size: 4, hard_tabs: false })
                .default_value(source.clone());
            // Our Onshape menu replaces the kit's (see `symbol_menu`).
            st.set_context_menu_enabled(false);
            st.set_highlighter_factory(Rc::new(move |_| Some(Box::new(ScriptHighlighter { tab, ..Default::default() }) as Box<dyn InputHighlighter>)), cx);
            st
        });
        // Onshape opens a new studio with the caret on line 4, under the import.
        if source == FS_TEMPLATE {
            editor.update(cx, |e, cx| e.set_cursor_position(Position { line: 3, character: 0 }, window, cx));
        }
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&editor, window, |this: &mut Self, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::Change) {
                this.schedule_save(window, cx);
            } else if matches!(ev, InputEvent::Blur) {
                this.save(cx);
            }
        }));
        subs.push(cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            DocEvent::TabChanged(t) if *t == this.tab => this.sync_from_doc(window, cx),
            DocEvent::Reloaded | DocEvent::TabsChanged => this.sync_from_doc(window, cx),
            _ => {}
        }));
        let mut this = Self { tab, doc, editor, synced: source.clone(), notices: Vec::new(), save_task: None, compile_task: None, outline: None, nav: Vec::new(), caret_before_click: None, nav_at: 0, focus: cx.focus_handle(), import_panel: None, import_anchor: Default::default(), tb: ToolbarFit::default(), outline_anchor: Default::default(), hover_at: None, toolbar: None, _subs: subs };
        this.recompile(source, cx);
        this
    }

    pub fn editor_focus(&self, cx: &App) -> FocusHandle {
        self.editor.read(cx).focus_handle(cx)
    }

    fn read_only(&self, cx: &App) -> bool {
        self.doc.read(cx).session.read_only()
    }

    /// Undo/redo or history browsing replaced the text.
    fn sync_from_doc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(src) = text_of(&self.doc, self.tab, cx) else { return };
        if src == self.synced {
            cx.notify();
            return;
        }
        self.save_task = None;
        self.synced = src.clone();
        if self.editor.read(cx).value().as_ref() != src {
            self.editor.update(cx, |e, cx| e.set_value(src.clone(), window, cx));
        }
        self.recompile(src, cx);
    }

    /// Viewing a version (live std at 3095.0, dom/fs-nav-shortcuts.txt): the
    /// toolbar row holds only the view-only bar, "Versions are view only.
    /// Viewing 3095.0." and [Return to Main] (Part Studio's bar, without its
    /// custom features button).
    fn render_version_bar(&self, version: &str, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let branch = {
            let s = &self.doc.read(cx).session;
            s.store.branch(s.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into())
        };
        let doc = self.doc.clone();
        let back = div()
            .id("fs-vb-return")
            .h(px(24.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(3.))
            .bg(pal.accent)
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(rgb(0xffffff))
            .child(format!("Return to {branch}"))
            .on_click(move |_, _, cx| {
                doc.update(cx, |d, cx| {
                    if d.session.view_microversion(None).is_ok() {
                        d.reloaded(cx);
                    }
                })
            });
        let text = format!("Versions are view only. Viewing {version}.");
        crate::ui::toolbar()
            .id("fs-version-bar")
            .justify_center()
            .child(
                h_flex()
                    .h(px(32.))
                    .mt(px(-4.))
                    .px(px(20.))
                    .gap(px(10.))
                    .bg(rgb(0xffffff))
                    .rounded_b(px(10.))
                    .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(0x333333)).child(text))
                    .child(back),
            )
            .border_b_1()
            .border_color(rgb(0xd7d7d7))
            .into_any_element()
    }

    /// Ace's tooltip over a squiggle (live document/fs-gutter-tooltip): `#f5f5f5`,
    /// 1 px `#dadada`, the notice icon, the message in the code font.
    fn render_diagnostic_tooltip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use gpui_kit::base::input::DiagnosticSeverity;
        let at = self.hover_at?;
        let entry = self.editor.read(cx).hovered_diagnostic()?;
        let (error, info) = (entry.severity == DiagnosticSeverity::Error, matches!(entry.severity, DiagnosticSeverity::Info | DiagnosticSeverity::Hint));
        let tip = h_flex()
            .h(px(23.))
            .pl(px(18.))
            .pr(px(6.))
            .gap(px(7.))
            .bg(rgb(0xf5f5f5))
            .border_1()
            .border_color(rgb(0xdadada))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.15), offset: point(px(1.), px(1.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }])
            .text_size(px(13.))
            .font_family(cx.theme().mono_font_family.clone())
            .text_color(rgb(0x333333))
            .whitespace_nowrap()
            .child(crate::assets::icon(crate::fs_notices::notice_icon(error, info), 14., cx))
            .child(entry.message.clone());
        Some(deferred(anchored().position(at + point(px(6.), px(14.))).snap_to_window_with_margin(px(4.)).child(tip)).with_priority(2).into_any_element())
    }

    /// Import: open or close the panel (live document/81).
    fn toggle_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.import_panel.take().is_some() {
            cx.notify();
            return;
        }
        let panel = cx.new(|cx| crate::fs_import::ImportPanel::new(self.doc.clone(), self.tab, window, cx));
        let sub = cx.subscribe_in(&panel, window, |this: &mut Self, _, ev: &crate::fs_import::ImportEvent, window, cx| match ev {
            crate::fs_import::ImportEvent::Insert(line) => {
                if this.read_only(cx) {
                    return;
                }
                let src = this.editor.read(cx).value().to_string();
                let new = crate::fs_import::append_import(&src, line);
                this.editor.update(cx, |e, cx| e.set_value(new, window, cx));
                this.schedule_save(window, cx);
            }
            crate::fs_import::ImportEvent::Close => {
                this.import_panel = None;
                cx.notify();
            }
        });
        self.import_panel = Some((panel, sub));
        cx.notify();
    }

    fn schedule_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let _ = this.update(cx, |this: &mut Self, cx| this.save(cx));
        }));
        cx.notify();
    }

    /// Save the editor's text into the document (Onshape saves as you type;
    /// Part Studios keep using the committed source).
    fn save(&mut self, cx: &mut Context<Self>) {
        self.save_task = None;
        let src = self.editor.read(cx).value().to_string();
        if src == self.synced || self.read_only(cx) {
            return;
        }
        self.synced = src.clone();
        let tab = self.tab;
        let value = src.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, "Edit Feature Studio", cx, move |c| {
                if let TabContent::FeatureStudio(f) = c {
                    f.save(value);
                }
            });
        });
        self.recompile(src, cx);
    }

    /// Commit: apply this Feature Studio's text throughout the document.
    pub fn commit(&mut self, cx: &mut Context<Self>) {
        self.save(cx);
        let tab = self.tab;
        if !self.uncommitted(cx) {
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, "Commit Feature Studio", cx, |c| {
                if let TabContent::FeatureStudio(f) = c {
                    f.commit();
                }
            });
            let _ = doc.custom_features();
        });
        cx.notify();
    }

    /// Commit all: every Feature Studio with uncommitted changes.
    fn commit_all(&mut self, cx: &mut Context<Self>) {
        self.save(cx);
        commit_all(&self.doc, cx);
        cx.notify();
    }

    fn uncommitted(&self, cx: &App) -> bool {
        match self.doc.read(cx).session.doc.tab(self.tab).map(|t| &t.content) {
            Some(TabContent::FeatureStudio(f)) => f.has_uncommitted() || self.editor.read(cx).value().as_ref() != f.source,
            _ => false,
        }
    }

    /// Feature Studios with uncommitted changes ("Commit all (n)").
    fn uncommitted_count(&self, cx: &App) -> usize {
        let doc = &self.doc.read(cx).session.doc;
        doc.tabs.iter().filter(|t| matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted() || (t.id == self.tab && self.editor.read(cx).value().as_ref() != f.source))).count()
    }

    fn recompile(&mut self, src: String, cx: &mut Context<Self>) {
        let tab = self.tab;
        self.compile_task = Some(cx.spawn(async move |this, cx| {
            let notices = cx.background_spawn(async move { studio_notices(tab, &src) }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                // Squiggles under the reported token (hover shows the message).
                this.editor.update(cx, |e, cx| {
                    let text = e.text().clone();
                    let source = text.to_string();
                    let lines: Vec<&str> = source.split('\n').collect();
                    if let Some(set) = e.diagnostics_mut() {
                        use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
                        set.reset(&text);
                        for n in notices.iter().filter(|n| n.line > 0) {
                            let at = Position { line: (n.line - 1) as u32, character: n.column.saturating_sub(1) as u32 };
                            let len = squiggle_len(lines.get(n.line - 1).copied().unwrap_or(""), at.character as usize);
                            let end = Position { line: at.line, character: at.character + len as u32 };
                            set.push(Diagnostic::new(at..end, n.message.clone()).with_severity(if n.error {
                                DiagnosticSeverity::Error
                            } else if n.info {
                                DiagnosticSeverity::Info
                            } else {
                                DiagnosticSeverity::Warning
                            }));
                        }
                    }
                    cx.notify();
                });
                this.notices = notices;
                cx.emit(FeatureStudioEvent::NoticesChanged);
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// Jump to a 1-based line and column, remembering where we were.
    pub fn goto_line(&mut self, line: usize, column: usize, window: &mut Window, cx: &mut Context<Self>) {
        if line == 0 {
            return;
        }
        let to = Position { line: (line - 1) as u32, character: column.saturating_sub(1) as u32 };
        let here = NavEntry::caret(self.editor.read(cx).cursor_position());
        self.jump(here, NavEntry::caret(to), window, cx);
    }

    /// Jump from `from` to `to`, both added to the reference history.
    fn jump(&mut self, from: NavEntry, to: NavEntry, window: &mut Window, cx: &mut Context<Self>) {
        self.nav.truncate((self.nav_at + 1).min(self.nav.len()));
        if self.nav.last().is_none_or(|l| l.pos != from.pos || l.label != from.label) {
            self.nav.push(from);
        }
        let pos = to.pos;
        self.nav.push(to);
        self.nav_at = self.nav.len() - 1;
        self.move_cursor(pos, window, cx);
    }

    /// Ctrl+click (cmd+click on macOS) on a symbol: to its definition in this
    /// studio (live: the caret lands on the defined name).
    fn goto_definition(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((name, _)) = self.symbol_at_cursor(cx) else { return };
        let text = self.editor.read(cx).value().to_string();
        let Some(to) = definition_of(&text, &name) else {
            // Live: a std symbol opens the std document, read-only, at its export.
            if let Some((module, line, column)) = crate::std_source::std_definition(&name) {
                cx.emit(FeatureStudioEvent::OpenStd { module: module.to_string(), line, column });
            }
            return;
        };
        let here = self.editor.read(cx).cursor_position();
        // Live history: the caret before the click, the use, the definition.
        if let Some(before) = self.caret_before_click.take().filter(|b| *b != here) {
            self.nav.truncate((self.nav_at + 1).min(self.nav.len()));
            if self.nav.last().is_none_or(|l| l.pos != before) {
                self.nav.push(NavEntry::caret(before));
                self.nav_at = self.nav.len() - 1;
            }
        }
        let from = NavEntry { pos: here, label: format!("Use - {name}") };
        self.jump(from, NavEntry { pos: to, label: format!("Symbol - {name}") }, window, cx);
    }

    /// Back ▾ / Forward ▾: the history entries on that side, nearest first.
    fn open_history(&mut self, back: bool, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let items: Vec<(usize, String)> = if back {
            (0..self.nav_at.min(self.nav.len())).rev().map(|i| (i, format!("{tab}: {}", self.nav[i].label))).collect()
        } else {
            (self.nav_at + 1..self.nav.len()).map(|i| (i, format!("{tab}: {}", self.nav[i].label))).collect()
        };
        if items.is_empty() {
            return;
        }
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.dropdown();
            for (i, label) in items.iter().cloned() {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(label).on_click(move |_, w, cx| {
                    v.update(cx, |this, cx| {
                        this.nav_at = i;
                        let pos = this.nav[i].pos;
                        this.move_cursor(pos, w, cx);
                    })
                }));
            }
            m
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    fn move_cursor(&mut self, to: Position, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| {
            e.set_cursor_position(to, window, cx);
            e.focus(window, cx);
        });
        cx.notify();
    }

    fn navigate(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let at = self.nav_at as isize + delta;
        if at < 0 || at as usize >= self.nav.len() {
            return;
        }
        self.nav_at = at as usize;
        self.move_cursor(self.nav[self.nav_at].pos, window, cx);
    }

    fn toggle_outline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.outline.take().is_none() {
            let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search symbols..."));
            st.update(cx, |s, cx| s.focus(window, cx));
            self.outline = Some(st);
        }
        cx.notify();
    }

    /// Commit ▾ (help: "Commit" ctrl s, "Commit all (n)" shift ctrl s).
    fn open_commit_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.uncommitted_count(cx);
        let (v1, v2) = (cx.entity(), cx.entity());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            use crate::os_menu::OsMenuItem as I;
            let all = if n > 0 { format!("Commit all ({n})") } else { "Commit all".to_string() };
            m.dropdown()
                .item(I::new("Commit").keys(&["ctrl", "s"]).on_click(move |_, _, cx| v1.update(cx, |t, cx| t.commit(cx))))
                .item(I::new(all).keys(&["shift", "ctrl", "s"]).on_click(move |_, _, cx| v2.update(cx, |t, cx| t.commit_all(cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// The identifier under the cursor and where it is.
    fn symbol_at_cursor(&self, cx: &App) -> Option<(String, String)> {
        let e = self.editor.read(cx);
        let pos = e.cursor_position();
        let text = e.value();
        let line = text.split('\n').nth(pos.line as usize)?;
        let chars: Vec<char> = line.chars().collect();
        let at = (pos.character as usize).min(chars.len());
        let is_id = |c: &char| c.is_alphanumeric() || *c == '_';
        let start = chars[..at].iter().rposition(|c| !is_id(c)).map(|i| i + 1).unwrap_or(0);
        let end = chars[at..].iter().position(|c| !is_id(c)).map(|i| at + i).unwrap_or(chars.len());
        if start >= end || chars[start].is_ascii_digit() {
            return None;
        }
        let name: String = chars[start..end].iter().collect();
        let tab = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        Some((name, format!("{tab} - Line {}, Column {}", pos.line + 1, start + 1)))
    }

    /// Right-click in the code (help image fs-search-symbol-context): Copy,
    /// Paste, Cut, Find usages in workspace, Rename in workspace.
    fn symbol_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{Copy, Cut, Paste};
        let editor = self.editor.clone();
        let symbol = self.symbol_at_cursor(cx);
        let read_only = self.read_only(cx);
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            use crate::os_menu::OsMenuItem as I;
            let act = |a: Box<dyn Action>, editor: Entity<gpui_kit::component::input::EditorState>| {
                move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                    editor.update(cx, |e, cx| e.focus(window, cx));
                    window.dispatch_action(a.boxed_clone(), cx);
                }
            };
            let (s1, s2, v1, v2) = (symbol.clone(), symbol.clone(), view.clone(), view.clone());
            m.list_no_icons()
                .item(I::new("Copy").on_click(act(Box::new(Copy), editor.clone())))
                .item(I::new("Paste").disabled(read_only).on_click(act(Box::new(Paste), editor.clone())))
                .item(I::new("Cut").disabled(read_only).on_click(act(Box::new(Cut), editor.clone())))
                .item(I::new("Find usages in workspace").disabled(symbol.is_none()).on_click(move |_, _, cx| {
                    if let Some((name, at)) = s1.clone() {
                        v1.update(cx, |_, cx| cx.emit(FeatureStudioEvent::FindSymbol { name, at, rename: false }));
                    }
                }))
                .item(I::new("Rename in workspace").disabled(symbol.is_none() || read_only).on_click(move |_, _, cx| {
                    if let Some((name, at)) = s2.clone() {
                        v2.update(cx, |_, cx| cx.emit(FeatureStudioEvent::FindSymbol { name, at, rename: true }));
                    }
                }))
        });
        crate::os_menu::open(at, menu, window, cx);
    }

    /// Insert a snippet at the cursor.
    fn insert_snippet(&mut self, text: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only(cx) {
            return;
        }
        self.editor.update(cx, |e, cx| {
            e.focus(window, cx);
            e.insert(text, window, cx);
        });
    }

    fn format(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let src = self.editor.read(cx).value().to_string();
        let formatted = format_source(&src);
        if formatted != src && !self.read_only(cx) {
            self.editor.update(cx, |e, cx| e.set_value(formatted, window, cx));
            self.schedule_save(window, cx);
        }
    }

    fn open_snippets(&mut self, menu_ix: usize, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.snippet_list();
            for (label, text) in SNIPPET_MENUS[menu_ix].2.iter() {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(*label).on_click(move |_, w, cx| v.update(cx, |this, cx| this.insert_snippet(text, w, cx))));
            }
            m
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// Compact toolbar: the snippet ▾ is one flat list of all six menus'
    /// snippets (live 71-compact-snippet-menu); a pick inserts it and becomes
    /// the button's face.
    fn open_all_snippets(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.snippet_list();
            for (label, text) in SNIPPET_MENUS.iter().flat_map(|m| m.2.iter()) {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(*label).on_click(move |_, w, cx| {
                    set_snippet_face(label);
                    v.update(cx, |this, cx| this.insert_snippet(text, w, cx));
                }));
            }
            m
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// Compact toolbar: Undo ▾ = Undo / Redo with their shortcut chips (no
    /// history list); inert while there's nothing to undo or redo.
    fn open_undo_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let (can_undo, can_redo) = { let d = self.doc.read(cx); (d.session.can_undo(), d.session.can_redo()) };
        if !can_undo && !can_redo {
            return;
        }
        let (d1, d2) = (self.doc.clone(), self.doc.clone());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            m.dropdown()
                .item(crate::os_menu::OsMenuItem::new("Undo").icon_name("undo").keys(&["cmd", "z"]).disabled(!can_undo).on_click(move |_, _, cx| d1.update(cx, |d, cx| d.undo(cx))))
                .item(crate::os_menu::OsMenuItem::new("Redo").icon_name("redo").keys(&["ctrl", "y"]).disabled(!can_redo).on_click(move |_, _, cx| d2.update(cx, |d, cx| d.redo(cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// Compact toolbar: Import ▾ = Import | Format Studio.
    fn open_import_menu(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let (v1, v2) = (cx.entity(), cx.entity());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            m.dropdown()
                .item(crate::os_menu::OsMenuItem::new("Import").icon(os::glyph(os::GLYPH_FS_IMPORT, 20., p().text)).on_click(move |_, w, cx| v1.update(cx, |this, cx| this.toggle_import(w, cx))))
                .item(crate::os_menu::OsMenuItem::new("Format Studio").icon(os::glyph(os::GLYPH_BRUSH, 20., p().text)).on_click(move |_, w, cx| v2.update(cx, |this, cx| this.format(w, cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// Monitor ▾: Monitor / Profile for each Part Studio and Variable Studio.
    fn open_monitor(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let studios: Vec<(TabId, String)> = self.doc.read(cx).session.doc.ordered_tabs().iter().filter(|t| matches!(t.content, TabContent::PartStudio(_) | TabContent::VariableStudio(_))).map(|t| (t.id, t.name.clone())).collect();
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.dropdown();
            for (id, name) in studios {
                let v = view.clone();
                // Live 71: a pulse icon for Monitor, a stopwatch for Profile.
                m = m.item(crate::os_menu::OsMenuItem::new(format!("Monitor {name}")).icon_name("fs-monitor").on_click(move |_, _, cx| {
                    let doc = v.read(cx).doc.clone();
                    crate::fs_notices::set_monitored(&doc, id, cx);
                }));
                m = m.item(crate::os_menu::OsMenuItem::new(format!("Profile {name}")).icon_name("regen-timer").disabled(true));
            }
            m
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// The blue Commit split button.
    fn render_commit(&self, cx: &mut Context<Self>) -> AnyElement {
        // Onshape: pale (Bootstrap disabled look) while there is nothing to commit.
        let clean = !self.uncommitted(cx);
        let anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let probe = anchor.clone();
        h_flex()
            .relative()
            .flex_none()
            .mx(px(4.))
            .child(
                Button::new("fs-commit")
                    .primary()
                    .h(px(30.))
                    .px(px(10.5))
                    .rounded_l(px(5.25))
                    .rounded_r(px(0.))
                    .child(div().text_size(px(14.)).child("Commit"))
                    // Live document/70: #8aa8d7 = the primary blue at 50%.
                    .when(clean, |b| b.opacity(0.5))
                    .tooltip("Commit Feature Studio (cmd+s/ctrl+s)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.commit(cx))),
            )
            .child(div().w(px(1.)).h(px(30.)).bg(rgb(0x0e3f8c)).when(clean, |d| d.opacity(0.5)))
            .child(
                Button::new("fs-commit-menu")
                    .primary()
                    .h(px(30.))
                    .w(px(24.))
                    .p_0()
                    .rounded_r(px(5.25))
                    .rounded_l(px(0.))
                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 14., rgb(0xffffff)))
                    .when(clean, |b| b.opacity(0.5))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let b = anchor.get();
                        this.open_commit_menu(point(b.left(), b.bottom() + px(2.)), window, cx);
                    })),
            )
            .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .into_any_element()
    }

    /// Search tools… (alt+c): the Feature Studio's commands.
    fn open_tools(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let (v1, v2, v3, v4) = (cx.entity(), cx.entity(), cx.entity(), cx.entity());
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, _, _| {
            use crate::os_menu::OsMenuItem as I;
            m.dropdown()
                .item(I::new("Commit").keys(&["ctrl", "s"]).on_click(move |_, _, cx| v1.update(cx, |t, cx| t.commit(cx))))
                .item(I::new("Commit all").keys(&["shift", "ctrl", "s"]).on_click(move |_, _, cx| v2.update(cx, |t, cx| t.commit_all(cx))))
                .item(I::new("Search Feature Studios").keys(&["shift", "ctrl", "f"]).on_click(move |_, _, cx| v3.update(cx, |_, cx| cx.emit(FeatureStudioEvent::OpenSearch))))
                .item(I::new("Module outline").on_click(move |_, w, cx| v4.update(cx, |t, cx| t.toggle_outline(w, cx))))
        });
        crate::os_menu::open_anchored(at, Anchor::TopLeft, menu, window, cx);
    }

    /// Module outline ▾ popover: "Search symbols…", "n symbols", the list.
    fn render_outline(&self, at: Point<Pixels>, _window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let field = self.outline.clone()?;
        let pal = p();
        let q = field.read(cx).value().to_lowercase();
        let symbols: Vec<Symbol> = outline_of(self.editor.read(cx)).iter().filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q)).cloned().collect();
        let count = format!("{} symbol{}", symbols.len(), if symbols.len() == 1 { "" } else { "s" });
        let empty = symbols.is_empty().then(|| div().mt(px(22.)).text_size(px(11.)).line_height(px(16.)).text_color(pal.text_muted).child("No top-level symbols found"));
        let rows: Vec<AnyElement> = symbols
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let line = s.line;
                let label = format!("Symbol - {}({})", s.name, s.params);
                let column = s.column;
                h_flex()
                    .id(SharedString::from(format!("sym-{i}")))
                    .h(px(34.))
                    .px(px(8.))
                    .gap(px(8.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(0xdef1fb)))
                    .child(os::glyph(os::GLYPH_FUNCTION, 16., pal.text))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(format!("{}({})", s.name, s.params)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.outline = None;
                        let here = NavEntry::caret(this.editor.read(cx).cursor_position());
                        let to = Position { line: (line - 1) as u32, character: column as u32 };
                        this.jump(here, NavEntry { pos: to, label: label.clone() }, window, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            deferred(anchored().position(at).snap_to_window_with_margin(px(8.)).child(
                v_flex()
                    .id("fs-outline")
                    .occlude()
                    // Live 71 (empty): 218 px, #ccc, the 202 px search field 8 px in,
                    // 11 px "0 symbols" / "No top-level symbols found".
                    .min_w(px(218.))
                    .max_w(px(420.))
                    .max_h(px(420.))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xcccccc))
                    .rounded(px(4.))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                    .pt(px(8.))
                    .px(px(8.))
                    .pb(px(12.))
                    .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.outline = None;
                        cx.notify();
                    }))
                    .child(div().w(px(202.)).child(crate::os_ui::input_h(Input::new(&field), 31.).text_size(px(14.))))
                    .child(div().mt(px(10.)).text_size(px(11.)).line_height(px(16.)).text_color(pal.text_muted).child(count))
                    .children(empty)
                    .child(v_flex().id("fs-outline-list").mt(px(4.)).overflow_y_scroll().children(rows)),
            ))
            .with_priority(2)
            .into_any_element(),
        )
    }
}

/// Commit every Feature Studio of the document that has uncommitted changes
/// (each a change of its tab, so the Part Studios using it regenerate).
pub fn commit_all(doc: &Entity<DocModel>, cx: &mut App) {
    let tabs: Vec<TabId> = doc.read(cx).session.doc.tabs.iter().filter(|t| matches!(&t.content, TabContent::FeatureStudio(f) if f.has_uncommitted())).map(|t| t.id).collect();
    doc.update(cx, |doc, cx| {
        for tab in tabs {
            doc.edit_tab(tab, "Commit Feature Studio", cx, |c| {
                if let TabContent::FeatureStudio(f) = c {
                    f.commit();
                }
            });
        }
        let _ = doc.custom_features();
    });
}

/// Live (behaviours "Feature Studio compact toolbar"): the toolbar collapses
/// one group at a time to fit the window, in this order, and expands back the
/// same way: 3 = full; 2 = the six snippet menus and Import / Format become a
/// split button faced with the last-used snippet and Import ▾; 1 = New
/// feature loses its label; 0 = Undo / Redo become Undo ▾. Below stage 0 the
/// row is clipped at the window edge (never wraps or scrolls).
struct ToolbarFit {
    stage: u8,
    /// Probed this frame: the toolbar, its flexible spacer, the last item.
    toolbar: BoundsCell,
    spacer: BoundsCell,
    end: BoundsCell,
    /// Probed widths of the parts that change, kept from the stage that shows them.
    undo_full: BoundsCell,
    undo_compact: BoundsCell,
    new_label: BoundsCell,
    snip_full: BoundsCell,
    snip_compact: BoundsCell,
    widths: [Option<f32>; 5],
}

impl Default for ToolbarFit {
    fn default() -> Self {
        Self { stage: 3, toolbar: Default::default(), spacer: Default::default(), end: Default::default(), undo_full: Default::default(), undo_compact: Default::default(), new_label: Default::default(), snip_full: Default::default(), snip_compact: Default::default(), widths: [None; 5] }
    }
}

impl ToolbarFit {
    /// Re-picks the stage from last frame's layout; true when it changed.
    fn refit(&mut self) -> bool {
        let tb = self.toolbar.get();
        if tb.size.width <= px(0.) {
            return false;
        }
        let w = |c: &BoundsCell| { let b = c.get(); (b.size.width > px(0.)).then(|| f32::from(b.size.width)) };
        for (i, c) in [&self.undo_full, &self.undo_compact, &self.new_label, &self.snip_full, &self.snip_compact].into_iter().enumerate() {
            if let Some(v) = w(c) {
                self.widths[i] = Some(v);
            }
        }
        let avail = f32::from(tb.size.width);
        let overflow = (f32::from(self.end.get().right() - tb.right())).max(0.);
        let natural = avail - f32::from(self.spacer.get().size.width) + overflow;
        // Width each step adds (estimates until both sides were seen).
        let [uf, uc, nl, sf, sc] = self.widths;
        let d = [uf.zip(uc).map_or(16., |(a, b)| a - b), nl.map_or(77., |l| l + 2.), sf.zip(sc).map_or(260., |(a, b)| a - b)];
        let at = |s: u8| -> f32 {
            let cur = self.stage;
            let mut n = natural;
            for k in (cur + 1)..=s { n += d[k as usize - 1]; }
            for k in (s + 1)..=cur { n -= d[k as usize - 1]; }
            n
        };
        let best = (0..=3u8).rev().find(|s| at(*s) <= avail + 0.5).unwrap_or(0);
        for c in [&self.undo_full, &self.undo_compact, &self.new_label, &self.snip_full, &self.snip_compact] {
            c.set(Bounds::default());
        }
        let changed = best != self.stage;
        self.stage = best;
        changed
    }
}

/// The compact snippet button's face: the last-used snippet (an Onshape
/// preference that survives reloads), "Length parameter" at first.
fn snippet_face() -> String {
    std::fs::read_to_string(crate::files::config_dir().join("feature_studio_snippet.txt")).ok().map(|t| t.trim().to_string()).filter(|t| SNIPPET_MENUS.iter().any(|m| m.2.iter().any(|(l, _)| l == t))).unwrap_or_else(|| "Length parameter".into())
}

fn set_snippet_face(label: &str) {
    let p = crate::files::config_dir().join("feature_studio_snippet.txt");
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, label);
}

fn snippet_text(label: &str) -> Option<&'static str> {
    SNIPPET_MENUS.iter().flat_map(|m| m.2.iter()).find(|(l, _)| *l == label).map(|(_, t)| *t)
}

fn text_of(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<String> {
    match &doc.read(cx).session.doc.tab(tab)?.content {
        TabContent::FeatureStudio(f) => Some(f.text().to_string()),
        _ => None,
    }
}

impl FeatureStudioView {
    /// The toolbar (a cached view: see `render`).
    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let probe = |cell: &Rc<std::cell::Cell<Bounds<Pixels>>>| {
            let c = cell.clone();
            canvas(move |b, _, _| c.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
        };
        // Toolbar cells: 32 px, transparent, `#d7d7d7` hover (live toolbar tooltips list).
        let cell = |id: SharedString| h_flex().id(id).relative().flex_none().h(px(32.)).px(px(6.)).gap(px(2.)).rounded(px(3.)).cursor_pointer().hover(|d| d.bg(pal.border));
        let tools_anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        // Fit-to-width stage, re-picked from the last frame's layout.
        if self.tb.refit() {
            cx.notify();
        }
        let stage = self.tb.stage;
        let (can_undo, can_redo) = {
            let d = self.doc.read(cx);
            (d.session.can_undo(), d.session.can_redo())
        };
        let undo_area: AnyElement = if stage >= 1 {
            div().flex_none().relative().child(crate::ui::undo_redo(&self.doc, cx)).child(probe(&self.tb.undo_full)).into_any_element()
        } else {
            // Undo ▾ (Redo folded in; the ▾ is inert without history).
            let (anchor, d) = (BoundsCell::default(), self.doc.clone());
            let a2 = anchor.clone();
            crate::ui::tool_group(
                "fs-undo-group",
                crate::ui::split_tool_button("fs-undo", "undo", false, cx)
                    .opacity(if can_undo { 1. } else { 0.4 })
                    .when(!can_undo, |b| b.cursor(CursorStyle::OperationNotAllowed))
                    .tooltip("Undo (cmd+z/ctrl+z)")
                    .on_click(move |_, _, cx| {
                        if can_undo {
                            d.update(cx, |d, cx| d.undo(cx));
                        }
                    }),
                crate::ui::tool_caret("fs-undo-caret", cx).opacity(if can_undo || can_redo { 1. } else { 0.4 }).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    let b = a2.get();
                    this.open_undo_menu(point(b.left(), b.bottom() + px(2.)), window, cx);
                })),
            )
            .relative()
            .child(probe(&anchor))
            .child(probe(&self.tb.undo_compact))
            .into_any_element()
        };
        let snip_area: AnyElement = if stage >= 3 {
            let mut area = h_flex().flex_none().relative().gap(px(2.));
            for (i, (tip, letter, _)) in SNIPPET_MENUS.iter().enumerate() {
                let anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
                let a2 = anchor.clone();
                area = area.child(
                    cell(SharedString::from(format!("fs-snip-{i}")))
                        .w(px(50.))
                        .tooltip(crate::ui::tooltip(*tip))
                        // Live document/70: Onshape's 20 px menu icon (letter / glyph and
                        // the ▸≡ list), then the chevron.
                        .child(crate::assets::icon(letter, 20., cx))
                        .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text))
                        .child(probe(&anchor))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let b = a2.get();
                            this.open_snippets(i, point(b.left(), b.bottom() + px(2.)), window, cx);
                        })),
                );
            }
            area.child(crate::ui::tool_separator())
                .child(
                    cell("fs-import".into())
                        .w(px(32.))
                        .when(self.import_panel.is_some(), |d| d.bg(pal.border))
                        .tooltip(crate::ui::tooltip("Import"))
                        .child(os::glyph(os::GLYPH_FS_IMPORT, 20., pal.text))
                        .child(probe(&self.import_anchor))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_import(window, cx))),
                )
                .child(
                    cell("fs-format".into())
                        .w(px(32.))
                        .tooltip(crate::ui::tooltip("Format Feature Studio"))
                        .child(os::glyph(os::GLYPH_BRUSH, 20., pal.text))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.format(window, cx))),
                )
                .child(probe(&self.tb.snip_full))
                .into_any_element()
        } else {
            // The snippet menus as one split button faced with the last-used
            // snippet, and Import ▾ (live 71-compact-*).
            let face = snippet_face();
            let face_text = snippet_text(&face).unwrap_or_default();
            let (snip_anchor, import_caret_anchor) = (BoundsCell::default(), BoundsCell::default());
            let (sa, ia) = (snip_anchor.clone(), import_caret_anchor.clone());
            h_flex()
                .flex_none()
                .relative()
                .gap(px(2.))
                .child(
                    crate::ui::tool_group(
                        "fs-snip-group",
                        cell("fs-snip-face".into())
                            .tooltip(crate::ui::tooltip(face.clone()))
                            .child(div().text_size(px(14.)).text_color(pal.text).child(face))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.insert_snippet(face_text, window, cx))),
                        crate::ui::tool_caret("fs-snip-caret", cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let b = sa.get();
                            this.open_all_snippets(point(b.left(), b.bottom() + px(2.)), window, cx);
                        })),
                    )
                    .relative()
                    .child(probe(&snip_anchor)),
                )
                .child(crate::ui::tool_separator())
                .child(
                    crate::ui::tool_group(
                        "fs-import-group",
                        cell("fs-import".into())
                            .w(px(32.))
                            .when(self.import_panel.is_some(), |d| d.bg(pal.border))
                            .tooltip(crate::ui::tooltip("Import"))
                            .child(os::glyph(os::GLYPH_FS_IMPORT, 20., pal.text))
                            .child(probe(&self.import_anchor))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_import(window, cx))),
                        crate::ui::tool_caret("fs-import-caret", cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let b = ia.get();
                            this.open_import_menu(point(b.left(), b.bottom() + px(2.)), window, cx);
                        })),
                    )
                    .relative()
                    .child(probe(&import_caret_anchor)),
                )
                .child(probe(&self.tb.snip_compact))
                .into_any_element()
        };
        // A resize re-lays the toolbar out without rendering this view: when
        // its width changes, render again so the stage follows.
        let (rendered_w, tb_cell, view) = (self.tb.toolbar.get().size.width, self.tb.toolbar.clone(), cx.entity().downgrade());
        let watch = canvas(|_, _, _| {}, move |_, _, _, cx| {
            if tb_cell.get().size.width != rendered_w {
                let view = view.clone();
                cx.defer(move |cx| {
                    let _ = view.update(cx, |_, cx| cx.notify());
                });
            }
        })
        .absolute()
        .size(px(1.));
        let mut toolbar = crate::ui::toolbar()
            .relative()
            .child(probe(&self.tb.toolbar))
            .child(watch)
            .child(undo_area)
            .child(crate::ui::tool_separator())
            .child(
                cell("fs-new".into())
                    .tooltip(crate::ui::tooltip("New feature"))
                    .child(os::glyph(os::GLYPH_NEW_FEATURE, 20., pal.text))
                    .when(stage >= 2, |d| d.child(div().relative().text_size(px(14.)).text_color(pal.text).child("New feature").child(probe(&self.tb.new_label))))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.insert_snippet(NEW_FEATURE, window, cx))),
            )
            .child(crate::ui::tool_separator())
            .child(snip_area);
        toolbar = toolbar
            .child(crate::ui::tool_separator())
            .child(self.render_commit(cx))
            .child(
                div()
                    .relative()
                    .child(crate::ui::search_tools_toggle("fs-search-tools", false).on_click(cx.listener({
                        let a = tools_anchor.clone();
                        move |this, _: &ClickEvent, window, cx| {
                            let b = a.get();
                            this.open_tools(point(b.left(), b.bottom() + px(2.)), window, cx);
                        }
                    })))
                    .child(probe(&tools_anchor)),
            )
            .child(div().flex_1().relative().child(probe(&self.tb.spacer)));
        // Right side: Monitor ▾, Module outline ▾, back / forward, Line, Column.
        let d = &self.doc.read(cx).session.doc;
        let monitored = crate::fs_notices::monitored(&self.doc, cx);
        let monitor_name = monitored.and_then(|t| d.tab(t)).map(|t| format!("Monitor {}", t.name)).unwrap_or_else(|| "Monitor".into());
        let pos = self.editor.read(cx).cursor_position();
        let symbols = outline_of(self.editor.read(cx));
        let current = symbols.iter().rev().find(|s| s.line <= pos.line as usize + 1).map(|s| format!("{}({})", s.name, s.params));
        let (can_back, can_fwd) = (self.nav_at > 0, self.nav_at + 1 < self.nav.len());
        let mon_anchor: BoundsCell = Default::default();
        // Kept across renders: the outline popover is placed before this
        // frame paints the button (opened by ctrl+shift+O as well).
        let out_anchor = self.outline_anchor.clone();
        let ma = mon_anchor.clone();
        toolbar = toolbar
            .child(
                h_flex()
                    .id("fs-monitor")
                    .relative()
                    .flex_none()
                    .h(px(29.))
                    // Live dom feature-studio-editor: sized to the text ("Monitor
                    // Part Studio 1" 168 px + the caret cell); long names truncate.
                    .max_w(px(247.))
                    .pl(px(6.))
                    .gap(px(6.))
                    .border_1()
                    .border_color(rgb(0xaaaaaa))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip(monitor_name.clone()))
                    .child(os::glyph(os::GLYPH_PULSE, 18., pal.text))
                    .child(div().min_w_0().pr(px(4.)).truncate().text_size(px(14.)).text_color(pal.text).child(monitor_name))
                    .child(div().w(px(14.)).flex_none().h_full().flex().items_center().justify_center().border_l_1().border_color(rgb(0xaaaaaa)).child(os::glyph(os::GLYPH_CARET, 8., pal.text)))
                    .child(probe(&mon_anchor))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        let b = ma.get();
                        this.open_monitor(point(b.left(), b.bottom() + px(2.)), window, cx);
                    })),
            )
            .child(div().w(px(18.)))
            .child(
                cell("fs-outline-btn".into())
                    .w(px(149.))
                    .when(self.outline.is_some(), |d| d.bg(pal.border))
                    .tooltip(crate::ui::tooltip("Module outline (cmd+shift+O / ctrl+shift+O)"))
                    .child(os::glyph(os::GLYPH_OUTLINE, 20., pal.text))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(current.unwrap_or_else(|| "Module outline".into())))
                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 14., pal.text))
                    .child(probe(&out_anchor))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_outline(window, cx))),
            )
            .child(div().w(px(10.)))
            .child(
                cell("fs-back".into())
                    .opacity(if can_back { 1. } else { 0.4 })
                    .tooltip(crate::ui::tooltip("Back reference (ctrl+[/cmd+[)"))
                    .child(os::glyph(os::GLYPH_ARROW_LEFT, 20., pal.text_muted))
                    .child(
                        div().id("fs-back-list").child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text_muted)).on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_history(true, e.position() + point(px(-24.), px(14.)), window, cx);
                        })),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.navigate(-1, window, cx))),
            )
            .child(
                cell("fs-forward".into())
                    .opacity(if can_fwd { 1. } else { 0.4 })
                    .tooltip(crate::ui::tooltip("Forward reference (ctrl+]/cmd+])"))
                    .child(os::glyph(os::GLYPH_ARROW_RIGHT, 20., pal.text_muted))
                    .child(
                        div().id("fs-forward-list").child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text_muted)).on_click(cx.listener(|this, e: &ClickEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_history(false, e.position() + point(px(-24.), px(14.)), window, cx);
                        })),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.navigate(1, window, cx))),
            )
            .child(div().flex_none().relative().px(px(8.)).text_size(px(14.)).text_color(pal.text).child(format!("Line {} , Column {}", pos.line + 1, pos.character + 1)).child(probe(&self.tb.end)));
        toolbar.border_b_1().border_color(rgb(0xd7d7d7)).into_any_element()
    }
}

impl Render for FeatureStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let read_only = self.read_only(cx);
        let toolbar = self.toolbar.get_or_insert_with(|| {
            // A cached view (see `crate::cached_slice`): a caret blink in the
            // editor doesn't rebuild it. It shows the caret's line and
            // symbol, so it follows the editor's edits and cursor moves.
            let t = crate::cached_slice::Slice::new(&cx.entity(), |this, _, cx| this.render_toolbar(cx), cx);
            crate::cached_slice::Slice::observing(&t, &self.editor, cx);
            crate::cached_slice::Slice::observing(&t, &self.doc, cx);
            t
        });
        let toolbar = toolbar.clone().cached(StyleRefinement::default().w_full().h(px(crate::ui::TOOLBAR_H)).flex_none());
        let below = |cell: &Rc<std::cell::Cell<Bounds<Pixels>>>| {
            let b = cell.get();
            point(b.left(), b.bottom() + px(2.))
        };
        let out_anchor = self.outline_anchor.clone();
        let outline_at = below(&out_anchor);
        let outline = self.render_outline(outline_at, window, cx);
        v_flex()
            .size_full()
            .key_context("FeatureStudio")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Commit, _, cx| this.commit(cx)))
            .on_action(cx.listener(|this, _: &CommitAll, _, cx| this.commit_all(cx)))
            .on_action(cx.listener(|_, _: &SearchStudios, _, cx| cx.emit(FeatureStudioEvent::OpenSearch)))
            .on_action(cx.listener(|this, _: &ModuleOutline, window, cx| this.toggle_outline(window, cx)))
            .on_action(cx.listener(|this, _: &BackReference, window, cx| this.navigate(-1, window, cx)))
            .on_action(cx.listener(|this, _: &ForwardReference, window, cx| this.navigate(1, window, cx)))
            .bg(rgb(0xffffff))
            .child(match self.doc.read(cx).session.viewing_version() {
                Some(version) => self.render_version_bar(&version, cx),
                None => toolbar.into_any_element(),
            })
            .child(
                div()
                    .id("fs-code")
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
                    .child(
                        // Live document/70 (Ace): 13 px monospace on 16 px lines, the first
                        // line right under the toolbar. The kit editor keeps an 8 px top
                        // padding (its size's), clipped off here.
                        div().absolute().top(px(-8.)).left_0().right_0().bottom_0().child(Editor::new(&self.editor).readonly(read_only).bordered(false).h_full().text_size(px(13.)).line_height(px(16.))),
                    )
                    // The diagnostic tooltip follows the pointer (re-render only
                    // while one shows or just hid).
                    .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                        let shown = this.editor.read(cx).hovered_diagnostic().is_some();
                        let was = this.hover_at.is_some();
                        this.hover_at = shown.then_some(e.position);
                        if shown || was {
                            cx.notify();
                        }
                    }))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if !*hovered && this.hover_at.take().is_some() {
                            cx.notify();
                        }
                    }))
                    // Before the editor moves the caret: where it was.
                    .capture_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.caret_before_click = Some(this.editor.read(cx).cursor_position());
                    }))
                    // After the editor placed the caret at the click.
                    .on_mouse_down(MouseButton::Left, cx.listener(|_, e: &MouseDownEvent, window, cx| {
                        if e.modifiers.secondary() && !e.modifiers.alt && !e.modifiers.shift {
                            cx.defer_in(window, |this, window, cx| this.goto_definition(window, cx));
                        }
                    }))
                    .on_mouse_down(MouseButton::Right, cx.listener(|_, e: &MouseDownEvent, window, cx| {
                        let at = e.position;
                        cx.defer_in(window, move |this, window, cx| this.symbol_menu(at, window, cx));
                    })),
            )
            .children(outline)
            .children(self.render_diagnostic_tooltip(cx))
            .children(self.import_panel.as_ref().map(|(panel, _)| {
                let b = self.import_anchor.get();
                deferred(anchored().position(point(b.left(), b.bottom() + px(2.))).child(panel.clone())).with_priority(1).into_any_element()
            }))
    }
}

// ---------------------------------------------------------------- syntax highlighting

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Comment,
    String,
    Number,
    Keyword,
    /// Ace's `variable.language`: the `path` / `version` of `import(…)`.
    Variable,
    Plain,
}

/// A small lexer-based highlighter for FeatureScript (Onshape's Ace colours,
/// live dom/feature-studio-editor.txt).
struct ScriptHighlighter {
    tab: TabId,
    tokens: Vec<(Range<usize>, Tok)>,
    len: usize,
}

impl Default for ScriptHighlighter {
    fn default() -> Self {
        Self { tab: TabId::new_random(), tokens: Vec::new(), len: 0 }
    }
}

const KEYWORDS: &[&str] = &[
    "FeatureScript", "import", "export", "const", "function", "predicate", "operator", "annotation", "enum", "type", "typecheck", "typeconvert", "precondition",
    "if", "else", "for", "in", "while", "do", "return", "returns", "var", "break", "continue", "throw", "try", "catch", "switch", "is", "new", "true", "false",
    "undefined", "map", "array", "string", "boolean", "number", "builtin", "box",
];

fn lex(src: &str) -> Vec<(Range<usize>, Tok)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push((start..i, Tok::Comment));
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            out.push((start..i, Tok::Comment));
        } else if c == b'"' || c == b'`' || c == b'\'' {
            i += 1;
            while i < b.len() && b[i] != c {
                if b[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(b.len());
            out.push((start..i, Tok::String));
        } else if c.is_ascii_digit() {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_') {
                i += 1;
            }
            out.push((start..i, Tok::Number));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = &src[start..i];
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let tok = if KEYWORDS.contains(&word) {
                Tok::Keyword
            } else if (word == "path" || word == "version") && b.get(j) == Some(&b':') {
                Tok::Variable
            } else {
                Tok::Plain
            };
            if tok != Tok::Plain {
                out.push((start..i, tok));
            }
        } else {
            // Advance by one UTF-8 character.
            i += src[i..].chars().next().map(|ch| ch.len_utf8()).unwrap_or(1);
        }
    }
    out
}

fn tok_style(t: Tok) -> HighlightStyle {
    let color = |hex: u32| -> Hsla { rgb(hex).into() };
    let mut s = HighlightStyle::default();
    match t {
        Tok::Comment => {
            // Light editor palette (Onshape's FeatureScript editor is light).
            // Ace's default comment green.
            s.color = Some(color(0x4c886b));
            s.font_style = Some(FontStyle::Italic);
        }
        Tok::String => s.color = Some(color(0x036a07)),
        Tok::Number => s.color = Some(color(0x0000cd)),
        // Onshape's editor: keywords and numbers in plain blue.
        Tok::Keyword => s.color = Some(color(0x0000ff)),
        Tok::Variable => s.color = Some(color(0x318495)),
        Tok::Plain => {}
    }
    s
}

impl InputHighlighter for ScriptHighlighter {
    fn language(&self) -> SharedString {
        "featurescript".into()
    }

    fn update(&mut self, _edit: Option<gpui_kit::component::input::InputEdit>, text: &gpui_kit::component::input::Rope, _folding: bool, _window: &mut Window, _cx: &mut Context<EditorState>) {
        let src = text.to_string();
        self.len = src.len();
        self.tokens = lex(&src);
    }

    fn styles(&self, range: &Range<usize>, _resolver: &dyn HighlightStyleResolver) -> Vec<(Range<usize>, HighlightStyle)> {
        let mut out = Vec::new();
        let mut at = range.start;
        let first = self.tokens.partition_point(|(r, _)| r.end <= range.start);
        for (r, t) in &self.tokens[first..] {
            if r.start >= range.end {
                break;
            }
            let s = r.start.max(range.start);
            let e = r.end.min(range.end);
            if s > at {
                out.push((at..s, HighlightStyle::default()));
            }
            if e > s {
                out.push((s..e, tok_style(*t)));
                at = e;
            }
        }
        if at < range.end {
            out.push((at..range.end, HighlightStyle::default()));
        }
        // Search pane matches: Onshape's yellow mark over the token colors.
        let marks = matches_of(self.tab);
        if marks.is_empty() {
            return out;
        }
        let mut split = Vec::with_capacity(out.len());
        for (r, st) in out {
            let mut at = r.start;
            for m in marks.iter().filter(|m| m.start < r.end && m.end > r.start) {
                let (s, e) = (m.start.max(r.start), m.end.min(r.end));
                if s > at {
                    split.push((at..s, st));
                }
                let mut hl = st;
                hl.background_color = Some(rgb(0xfff1a8).into());
                split.push((s..e, hl));
                at = e;
            }
            if at < r.end {
                split.push((at..r.end, st));
            }
        }
        split
    }

    fn fold_ranges(&self, _text: &gpui_kit::component::input::Rope) -> Vec<gpui_kit::component::input::FoldRange> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Tok, lex};

    // Plain std test attribute (`gpui_kit::*` brings in gpui's `test` macro).
    #[::core::prelude::v1::test]
    fn lexes_featurescript() {
        let src = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n// hi\nexport const f = 1;";
        let toks = lex(src);
        let kinds: Vec<Tok> = toks.iter().map(|t| t.1).collect();
        assert!(kinds.contains(&Tok::Comment) && kinds.contains(&Tok::Keyword) && kinds.contains(&Tok::String) && kinds.contains(&Tok::Number));
        assert_eq!(kinds.iter().filter(|k| **k == Tok::Variable).count(), 2);
        let outline = super::outline(super::NEW_FEATURE);
        assert_eq!((outline[0].name.as_str(), outline[0].params.as_str()), ("myFeature", "context, id, definition"));
    }

    #[::core::prelude::v1::test]
    fn squiggles_cover_the_token() {
        let line = "function f() { x = 1; } y = undefinedThing + 1;";
        assert_eq!(super::squiggle_len(line, 9), 1);
        assert_eq!(super::squiggle_len(line, 15), 1);
        assert_eq!(super::squiggle_len(line, 28), "undefinedThing".len());
        assert_eq!(super::squiggle_len(line, 10), 1);
        assert_eq!(super::squiggle_len("", 3), 1);
    }
}
