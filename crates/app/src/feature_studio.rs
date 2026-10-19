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
use gpui_kit::component::{h_flex, v_flex};
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

actions!(feature_studio, [Commit, CommitAll, SearchStudios]);

/// Onshape's Feature Studio shortcuts (live dom/keyboard-shortcuts.txt).
pub fn bind_keys(cx: &mut App) {
    let c = Some("FeatureStudio");
    cx.bind_keys([
        KeyBinding::new("secondary-s", Commit, c),
        KeyBinding::new("secondary-shift-s", CommitAll, c),
        KeyBinding::new("secondary-shift-f", SearchStudios, c),
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

/// A top-level symbol in the Module outline: name, parameter list, 1-based line.
#[derive(Clone)]
struct Symbol {
    name: String,
    params: String,
    line: usize,
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
                return Some(Symbol { name: name.trim().to_string(), params: params_of(inner), line: i + 1 });
            }
            let rest = t.strip_prefix("function ").or_else(|| t.strip_prefix("predicate "))?;
            let open = rest.find('(')?;
            Some(Symbol { name: rest[..open].trim().to_string(), params: params_of(rest), line: i + 1 })
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
    /// Cursor positions visited by jumps (back / forward).
    nav: Vec<Position>,
    nav_at: usize,
    focus: FocusHandle,
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
        let mut this = Self { tab, doc, editor, synced: source.clone(), notices: Vec::new(), save_task: None, compile_task: None, outline: None, nav: Vec::new(), nav_at: 0, focus: cx.focus_handle(), _subs: subs };
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
                    if let Some(set) = e.diagnostics_mut() {
                        use gpui_kit::base::input::{Diagnostic, DiagnosticSeverity};
                        set.reset(&text);
                        for n in notices.iter().filter(|n| n.line > 0) {
                            let at = Position { line: (n.line - 1) as u32, character: n.column.saturating_sub(1) as u32 };
                            let end = Position { line: at.line, character: at.character + 4 };
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
        let here = self.editor.read(cx).cursor_position();
        self.nav.truncate(self.nav_at);
        self.nav.push(here);
        let to = Position { line: (line - 1) as u32, character: column.saturating_sub(1) as u32 };
        self.nav.push(to);
        self.nav_at = self.nav.len() - 1;
        self.move_cursor(to, window, cx);
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
        self.move_cursor(self.nav[self.nav_at], window, cx);
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

    /// Monitor ▾: Monitor / Profile for each Part Studio and Variable Studio.
    fn open_monitor(&mut self, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let studios: Vec<(TabId, String)> = self.doc.read(cx).session.doc.ordered_tabs().iter().filter(|t| matches!(t.content, TabContent::PartStudio(_) | TabContent::VariableStudio(_))).map(|t| (t.id, t.name.clone())).collect();
        let view = cx.entity();
        let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
            m = m.dropdown();
            for (id, name) in studios {
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(format!("Monitor {name}")).on_click(move |_, _, cx| {
                    let doc = v.read(cx).doc.clone();
                    crate::fs_notices::set_monitored(&doc, id, cx);
                }));
                m = m.item(crate::os_menu::OsMenuItem::new(format!("Profile {name}")).disabled(true));
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
        let symbols: Vec<Symbol> = outline(self.editor.read(cx).value().as_ref()).into_iter().filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q)).collect();
        let count = format!("{} symbol{}", symbols.len(), if symbols.len() == 1 { "" } else { "s" });
        let empty = symbols.is_empty().then(|| div().text_size(px(14.)).text_color(pal.text_muted).child("No top-level symbols found"));
        let rows: Vec<AnyElement> = symbols
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                let line = s.line;
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
                        this.goto_line(line, 1, window, cx);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            deferred(anchored().position(at).snap_to_window_with_margin(px(8.)).child(
                v_flex()
                    .id("fs-outline")
                    .occlude()
                    .w(px(254.))
                    .max_h(px(420.))
                    .bg(rgb(0xffffff))
                    .border_1()
                    .border_color(rgb(0xd7d7d7))
                    .rounded(px(5.25))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                    .p(px(8.))
                    .gap(px(6.))
                    .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        this.outline = None;
                        cx.notify();
                    }))
                    .child(Input::new(&field).h(px(30.)).text_size(px(14.)))
                    .child(div().text_size(px(12.)).text_color(pal.text_muted).child(count))
                    .children(empty)
                    .child(v_flex().id("fs-outline-list").overflow_y_scroll().children(rows)),
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

fn text_of(doc: &Entity<DocModel>, tab: TabId, cx: &App) -> Option<String> {
    match &doc.read(cx).session.doc.tab(tab)?.content {
        TabContent::FeatureStudio(f) => Some(f.text().to_string()),
        _ => None,
    }
}

impl Render for FeatureStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let read_only = self.read_only(cx);
        let probe = |cell: &Rc<std::cell::Cell<Bounds<Pixels>>>| {
            let c = cell.clone();
            canvas(move |b, _, _| c.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()
        };
        let below = |cell: &Rc<std::cell::Cell<Bounds<Pixels>>>| {
            let b = cell.get();
            point(b.left(), b.bottom() + px(2.))
        };
        // Toolbar cells: 32 px, transparent, `#d7d7d7` hover (live toolbar tooltips list).
        let cell = |id: SharedString| h_flex().id(id).relative().flex_none().h(px(32.)).px(px(6.)).gap(px(2.)).rounded(px(3.)).cursor_pointer().hover(|d| d.bg(pal.border));
        let tools_anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let mut toolbar = crate::ui::toolbar()
            .child(crate::ui::undo_redo(&self.doc, cx))
            .child(crate::ui::tool_separator())
            .child(
                cell("fs-new".into())
                    .tooltip(crate::ui::tooltip("New feature"))
                    .child(os::glyph(os::GLYPH_NEW_FEATURE, 20., pal.text))
                    .child(div().text_size(px(14.)).text_color(pal.text).child("New feature"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.insert_snippet(NEW_FEATURE, window, cx))),
            )
            .child(crate::ui::tool_separator());
        for (i, (tip, letter, _)) in SNIPPET_MENUS.iter().enumerate() {
            let anchor: Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let a2 = anchor.clone();
            toolbar = toolbar.child(
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
        toolbar = toolbar
            .child(crate::ui::tool_separator())
            .child(cell("fs-import".into()).w(px(32.)).opacity(0.4).tooltip(crate::ui::tooltip("Import")).child(os::glyph(os::GLYPH_FS_IMPORT, 20., pal.text)))
            .child(
                cell("fs-format".into())
                    .w(px(32.))
                    .tooltip(crate::ui::tooltip("Format Feature Studio"))
                    .child(os::glyph(os::GLYPH_BRUSH, 20., pal.text))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.format(window, cx))),
            )
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
            .child(div().flex_1());
        // Right side: Monitor ▾, Module outline ▾, back / forward, Line, Column.
        let d = &self.doc.read(cx).session.doc;
        let monitored = crate::fs_notices::monitored(&self.doc, cx);
        let monitor_name = monitored.and_then(|t| d.tab(t)).map(|t| format!("Monitor {}", t.name)).unwrap_or_else(|| "Monitor".into());
        let pos = self.editor.read(cx).cursor_position();
        let symbols = outline(self.editor.read(cx).value().as_ref());
        let current = symbols.iter().rev().find(|s| s.line <= pos.line as usize + 1).map(|s| format!("{}({})", s.name, s.params));
        let (can_back, can_fwd) = (self.nav_at > 0, self.nav_at + 1 < self.nav.len());
        let (mon_anchor, out_anchor): (BoundsCell, BoundsCell) = Default::default();
        let ma = mon_anchor.clone();
        toolbar = toolbar
            .child(
                h_flex()
                    .id("fs-monitor")
                    .relative()
                    .flex_none()
                    .h(px(29.))
                    .max_w(px(220.))
                    .pl(px(6.))
                    .gap(px(6.))
                    .border_1()
                    .border_color(rgb(0xaaaaaa))
                    .rounded(px(3.))
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip(monitor_name.clone()))
                    .child(os::glyph(os::GLYPH_PULSE, 18., pal.text))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(monitor_name))
                    .child(div().w(px(14.)).h_full().flex().items_center().justify_center().border_l_1().border_color(rgb(0xaaaaaa)).child(os::glyph(os::GLYPH_CARET, 8., pal.text)))
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
                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text_muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.navigate(-1, window, cx))),
            )
            .child(
                cell("fs-forward".into())
                    .opacity(if can_fwd { 1. } else { 0.4 })
                    .tooltip(crate::ui::tooltip("Forward reference (ctrl+]/cmd+])"))
                    .child(os::glyph(os::GLYPH_ARROW_RIGHT, 20., pal.text_muted))
                    .child(os::glyph(os::GLYPH_CHEVRON_DOWN, 12., pal.text_muted))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.navigate(1, window, cx))),
            )
            .child(div().flex_none().px(px(8.)).text_size(px(14.)).text_color(pal.text).child(format!("Line {} , Column {}", pos.line + 1, pos.character + 1)));
        let outline_at = below(&out_anchor);
        let outline = self.render_outline(outline_at, window, cx);
        v_flex()
            .size_full()
            .key_context("FeatureStudio")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Commit, _, cx| this.commit(cx)))
            .on_action(cx.listener(|this, _: &CommitAll, _, cx| this.commit_all(cx)))
            .on_action(cx.listener(|_, _: &SearchStudios, _, cx| cx.emit(FeatureStudioEvent::OpenSearch)))
            .bg(rgb(0xffffff))
            .child(toolbar.border_b_1().border_color(rgb(0xd7d7d7)))
            .child(
                div()
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
                    // After the editor placed the caret at the click.
                    .on_mouse_down(MouseButton::Right, cx.listener(|_, e: &MouseDownEvent, window, cx| {
                        let at = e.position;
                        cx.defer_in(window, move |this, window, cx| this.symbol_menu(at, window, cx));
                    })),
            )
            .children(outline)
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
}
