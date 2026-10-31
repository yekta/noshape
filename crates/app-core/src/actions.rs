//! Application actions and default key bindings (Onshape's shortcut map).
//! `secondary` = Cmd on macOS, Ctrl elsewhere.

use gpui_kit::{KeyBinding, actions};

actions!(
    noshape,
    [
        // General
        Undo,
        Redo,
        Copy,
        Paste,
        DeleteSelection,
        ClearSelection,
        Accept,
        AcceptAndRepeat,
        // Tab / Shift+Tab in a feature dialog (Onshape's order; leaving it
        // past either end) and the navbar target of Tab past its end.
        DialogTab,
        DialogTabPrev,
        FocusNavbar,
        // Shift+Tab with the focus on the Part Studio itself (Onshape's
        // <body>): it stays there.
        StudioTabPrev,
        Cancel,
        RenameSelected,
        SearchTools,
        ShortcutToolbar,
        ToggleShortcutHelp,
        TabManager,
        SwitchRecentTab,
        MeasureTool,
        MateConnectorTool,
        ToggleMateConnectors,
        FlipPrimaryAxis,
        ReorientSecondaryAxis,
        SelectOther,
        SelectOtherPrevious,
        NewDocument,
        /// Open the Create Drawing dialog (request in `drawing_create`).
        CreateDrawing,
        /// Activate the tab set by `request_activate_tab` (below).
        ActivateRequestedTab,
        /// Open the Export modal of the item set by `workspace::request_export`.
        ExportRequestedItem,
        OpenDocument,
        SaveCopyAs,
        CloseWindow,
        Quit,
        ToggleVersionGraph,
        /// The Release drafts panel (left rail).
        ToggleReleaseDrafts,
        CreateVersion,
        // Part Studio
        NewSketch,
        Extrude,
        Revolve,
        Fillet,
        ToggleSketches,
        CurveSurfaceAnalysis,
        DihedralAnalysis,
        ViewHighQuality,
        RollbackUp,
        RollbackDown,
        // 3D view
        ViewFront,
        ViewBack,
        ViewLeft,
        ViewRight,
        ViewTop,
        ViewBottom,
        ViewIsometric,
        ViewNormal,
        ZoomToFit,
        ZoomIn,
        ZoomOut,
        ZoomWindow,
        TogglePlanes,
        HideConstruction,
        HideSelected,
        ShowAll,
        IsolateSelected,
        TransparentSelected,
        SectionView,
        NamedViews,
        RotateLeft,
        RotateRight,
        RotateUp,
        RotateDown,
        RotateLeftFine,
        RotateRightFine,
        RotateUpFine,
        RotateDownFine,
        RotateLeft90,
        RotateRight90,
        RotateUp90,
        RotateDown90,
        PanLeft,
        PanRight,
        PanUp,
        PanDown,
        ToggleProjection,
        // Sketch tools
        SketchLine,
        SketchArc3,
        SketchCircle,
        SketchCenterRectangle,
        SketchCornerRectangle,
        SketchPoint,
        SketchFillet,
        SketchOffset,
        SketchTrim,
        SketchExtend,
        SketchUse,
        SketchDimension,
        SketchConstruction,
        SketchTangentArcToggle,
        ConstraintCoincident,
        ConstraintConcentric,
        ConstraintCurvature,
        ConstraintEqual,
        ConstraintFix,
        ConstraintHorizontal,
        ConstraintVertical,
        ConstraintMidpoint,
        ConstraintNormal,
        ConstraintParallel,
        ConstraintPerpendicular,
        ConstraintPierce,
        ConstraintSymmetric,
        ConstraintTangent,
        // Assembly
        InsertInstance,
        FastenMate,
        ToggleMates,
        ToggleShowMatesMode,
        ToggleSnapMode,
        // Drawing
        DrawingUpdate,
    ]
);

/// Where Ctrl+Space goes back to the previous tab: anywhere in a document window but the Feature Studio's code
/// editor.
pub const RECENT_TAB_CONTEXT: &str = "Workspace && !(FeatureStudio > Input)";

/// Onshape's default key bindings (before user customizations; the app binds them with the user's customizations,
/// Preferences › Keyboard shortcuts, through its `shortcuts::init`).
pub fn default_key_bindings() -> Vec<KeyBinding> {
    // Bare-letter bindings must not fire while typing in an input.
    let ps = Some("PartStudio && !Input");
    let sk = Some("Sketch && !Input");
    let asm = Some("Assembly && !Input");
    let view = Some("Workspace && !Input");
    let doc_mod = Some("Workspace");
    let doc = Some("Workspace && !Input");
    let dlg = Some("FeatureDialog && !Input");
    vec![
        // General / document
        KeyBinding::new("secondary-z", Undo, doc_mod),
        KeyBinding::new("secondary-shift-z", Redo, doc_mod),
        KeyBinding::new("ctrl-y", Redo, doc_mod),
        KeyBinding::new("secondary-c", Copy, doc),
        KeyBinding::new("secondary-v", Paste, doc),
        KeyBinding::new("delete", DeleteSelection, doc),
        KeyBinding::new("backspace", DeleteSelection, doc),
        KeyBinding::new("space", ClearSelection, doc),
        KeyBinding::new("escape", Cancel, doc),
        KeyBinding::new("enter", Accept, dlg),
        KeyBinding::new("shift-enter", AcceptAndRepeat, dlg),
        KeyBinding::new("escape", Cancel, dlg),
        KeyBinding::new("enter", Accept, Some("CreateDrawing")),
        KeyBinding::new("escape", Cancel, Some("CreateDrawing")),
        // Esc while typing in a dialog field still cancels the dialog (Onshape).
        KeyBinding::new("escape", Cancel, Some("FeatureDialog > Input")),
        // And typing in the appearance editor's fields (live request 1663 (a)).
        KeyBinding::new("escape", Cancel, Some("AppearanceEditor > Input")),
        KeyBinding::new("tab", DialogTab, Some("FeatureDialog")),
        KeyBinding::new("shift-tab", DialogTabPrev, Some("FeatureDialog")),
        KeyBinding::new("shift-tab", StudioTabPrev, Some("PartStudio")),
        // Live (ui 10-22 b): Change configuration… leaves the focus on the
        // page, and the first Tab goes to the top bar (Share).
        KeyBinding::new("tab", FocusNavbar, asm),
        KeyBinding::new("shift-n", RenameSelected, doc),
        KeyBinding::new("alt-c", SearchTools, doc_mod),
        KeyBinding::new("s", ShortcutToolbar, doc),
        KeyBinding::new("shift-?", ToggleShortcutHelp, doc),
        KeyBinding::new("?", ToggleShortcutHelp, doc),
        KeyBinding::new("alt-t", TabManager, doc_mod),
        // Not from the Feature Studio's code editor, where Ctrl+Space is the editor's own (FS-EDITOR-COMPLETE).
        KeyBinding::new("ctrl-space", SwitchRecentTab, Some(RECENT_TAB_CONTEXT)),
        KeyBinding::new("[", MeasureTool, doc),
        KeyBinding::new("ctrl-m", MateConnectorTool, doc_mod),
        KeyBinding::new("k", ToggleMateConnectors, doc),
        KeyBinding::new("a", FlipPrimaryAxis, dlg),
        KeyBinding::new("q", ReorientSecondaryAxis, dlg),
        KeyBinding::new("`", SelectOther, doc),
        // Live (behaviours "spec: psui part-studio.md 2.16 / 2.17 … Select other"):
        // Shift+` ("~") does nothing; ↑ / ↓ move in the open list.
        KeyBinding::new("secondary-n", NewDocument, None),
        KeyBinding::new("secondary-o", OpenDocument, None),
        KeyBinding::new("secondary-shift-s", SaveCopyAs, doc_mod),
        KeyBinding::new("secondary-w", CloseWindow, None),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("alt-h", ToggleVersionGraph, doc_mod),
        KeyBinding::new("secondary-alt-v", CreateVersion, doc_mod),
        // Part Studio
        KeyBinding::new("shift-s", NewSketch, ps),
        // Enter edits the selected feature (or picks the Select other entry).
        KeyBinding::new("enter", Accept, ps),
        KeyBinding::new("shift-enter", AcceptAndRepeat, ps),
        KeyBinding::new("shift-e", Extrude, ps),
        KeyBinding::new("shift-w", Revolve, ps),
        KeyBinding::new("shift-f", Fillet, ps),
        KeyBinding::new("shift-h", ToggleSketches, ps),
        // Analysis tools and view in high quality.
        KeyBinding::new("shift-c", CurveSurfaceAnalysis, ps),
        KeyBinding::new("shift-d", DihedralAnalysis, ps),
        KeyBinding::new("shift-r", ViewHighQuality, ps),
        // A "3D view" shortcut: assemblies too.
        KeyBinding::new("shift-r", ViewHighQuality, asm),
        KeyBinding::new("up", RollbackUp, Some("FeatureList && !Input")),
        KeyBinding::new("down", RollbackDown, Some("FeatureList && !Input")),
        // 3D view
        KeyBinding::new("shift-1", ViewFront, view),
        KeyBinding::new("shift-2", ViewBack, view),
        KeyBinding::new("shift-3", ViewLeft, view),
        KeyBinding::new("shift-4", ViewRight, view),
        KeyBinding::new("shift-5", ViewTop, view),
        KeyBinding::new("shift-6", ViewBottom, view),
        KeyBinding::new("shift-7", ViewIsometric, view),
        // Shifted digits arrive as symbols on some platforms/layouts (US layout).
        KeyBinding::new("!", ViewFront, view),
        KeyBinding::new("@", ViewBack, view),
        KeyBinding::new("#", ViewLeft, view),
        KeyBinding::new("$", ViewRight, view),
        KeyBinding::new("%", ViewTop, view),
        KeyBinding::new("^", ViewBottom, view),
        KeyBinding::new("&", ViewIsometric, view),
        KeyBinding::new("n", ViewNormal, view),
        KeyBinding::new("f", ZoomToFit, view),
        KeyBinding::new("shift-z", ZoomIn, view),
        KeyBinding::new("z", ZoomOut, view),
        KeyBinding::new("w", ZoomWindow, view),
        KeyBinding::new("p", TogglePlanes, view),
        KeyBinding::new("shift-p", HideConstruction, view),
        KeyBinding::new("y", HideSelected, view),
        KeyBinding::new("shift-y", ShowAll, view),
        KeyBinding::new("shift-i", IsolateSelected, view),
        KeyBinding::new("shift-t", TransparentSelected, view),
        KeyBinding::new("shift-x", SectionView, view),
        KeyBinding::new("shift-v", NamedViews, view),
        KeyBinding::new("left", RotateLeft, view),
        KeyBinding::new("right", RotateRight, view),
        KeyBinding::new("up", RotateUp, view),
        KeyBinding::new("down", RotateDown, view),
        KeyBinding::new("ctrl-left", RotateLeftFine, view),
        KeyBinding::new("ctrl-right", RotateRightFine, view),
        KeyBinding::new("ctrl-up", RotateUpFine, view),
        KeyBinding::new("ctrl-down", RotateDownFine, view),
        KeyBinding::new("shift-left", RotateLeft90, view),
        KeyBinding::new("shift-right", RotateRight90, view),
        KeyBinding::new("shift-up", RotateUp90, view),
        KeyBinding::new("shift-down", RotateDown90, view),
        KeyBinding::new("ctrl-shift-left", PanLeft, view),
        KeyBinding::new("ctrl-shift-right", PanRight, view),
        KeyBinding::new("ctrl-shift-up", PanUp, view),
        KeyBinding::new("ctrl-shift-down", PanDown, view),
        // Perspective view has no key in Onshape (camera menu only; a stray
        // key must not flip the projection).
        // Sketch (each key toggles its tool)
        KeyBinding::new("l", SketchLine, sk),
        KeyBinding::new("a", SketchArc3, sk),
        KeyBinding::new("c", SketchCircle, sk),
        KeyBinding::new("r", SketchCenterRectangle, sk),
        KeyBinding::new("g", SketchCornerRectangle, sk),
        KeyBinding::new("shift-s", SketchPoint, sk),
        KeyBinding::new("shift-f", SketchFillet, sk),
        KeyBinding::new("o", SketchOffset, sk),
        KeyBinding::new("m", SketchTrim, sk),
        KeyBinding::new("x", SketchExtend, sk),
        KeyBinding::new("u", SketchUse, sk),
        KeyBinding::new("d", SketchDimension, sk),
        KeyBinding::new("q", SketchConstruction, sk),
        KeyBinding::new("shift-a", SketchTangentArcToggle, sk),
        KeyBinding::new("i", ConstraintCoincident, sk),
        KeyBinding::new("shift-o", ConstraintConcentric, sk),
        KeyBinding::new("shift-u", ConstraintCurvature, sk),
        KeyBinding::new("e", ConstraintEqual, sk),
        KeyBinding::new("shift-j", ConstraintFix, sk),
        KeyBinding::new("h", ConstraintHorizontal, sk),
        KeyBinding::new("v", ConstraintVertical, sk),
        KeyBinding::new("shift-m", ConstraintMidpoint, sk),
        KeyBinding::new("shift-k", ConstraintNormal, sk),
        KeyBinding::new("b", ConstraintParallel, sk),
        KeyBinding::new("shift-l", ConstraintPerpendicular, sk),
        KeyBinding::new("shift-g", ConstraintPierce, sk),
        KeyBinding::new("shift-q", ConstraintSymmetric, sk),
        KeyBinding::new("t", ConstraintTangent, sk),
        // Assembly
        KeyBinding::new("i", InsertInstance, asm),
        KeyBinding::new("m", FastenMate, asm),
        KeyBinding::new("j", ToggleMates, asm),
        KeyBinding::new("h", ToggleShowMatesMode, asm),
        KeyBinding::new("shift-s", ToggleSnapMode, asm),
        // Snap drag: a flips the dragged connector's primary axis, q reorients its secondary axis.
        KeyBinding::new("a", FlipPrimaryAxis, asm),
        KeyBinding::new("q", ReorientSecondaryAxis, asm),
        // Drawing
        KeyBinding::new("ctrl-q", DrawingUpdate, Some("Drawing")),
    ]
}

/// Tab to activate, set by views that are not subscribed to by the
/// workspace (e.g. a drawing view's "Switch to <Part Studio>").
static REQUESTED_TAB: parking_lot::Mutex<Option<noshape_model::document::TabId>> = parking_lot::Mutex::new(None);

/// Ask the workspace to switch to `tab` (it handles [`ActivateRequestedTab`]
/// by taking [`take_requested_tab`]).
pub fn request_activate_tab(tab: noshape_model::document::TabId, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App) {
    *REQUESTED_TAB.lock() = Some(tab);
    window.dispatch_action(Box::new(ActivateRequestedTab), cx);
}

/// The tab [`request_activate_tab`] asked for, once.
pub fn take_requested_tab() -> Option<noshape_model::document::TabId> {
    REQUESTED_TAB.lock().take()
}

#[cfg(test)]
mod recent_tab_tests {
    use super::RECENT_TAB_CONTEXT;
    use gpui_kit::{KeyBindingContextPredicate, KeyContext};

    #[test]
    fn ctrl_space_leaves_the_feature_studio_editor_alone() {
        let p = KeyBindingContextPredicate::parse(RECENT_TAB_CONTEXT).unwrap();
        let ctx = |names: &[&str]| names.iter().map(|n| KeyContext::parse(n).unwrap()).collect::<Vec<_>>();
        assert!(p.depth_of(&ctx(&["Workspace", "PartStudio"])).is_some());
        assert!(p.depth_of(&ctx(&["Workspace", "FeatureStudio"])).is_some(), "the Feature Studio outside its editor");
        assert!(p.depth_of(&ctx(&["Workspace", "PartStudio", "Input"])).is_some(), "a Part Studio field");
        assert!(p.depth_of(&ctx(&["Workspace", "FeatureStudio", "Input"])).is_none());
        assert!(p.depth_of(&ctx(&["Workspace", "FeatureStudio", "CodePane", "Input"])).is_none());
    }
}
