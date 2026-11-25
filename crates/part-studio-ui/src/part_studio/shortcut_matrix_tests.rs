//! Live shortcuts-1123 (Windows and macOS, every key pressed in a Part Studio, with a feature dialog open, in a sketch
//! and with a sketch tool on): each key of live's map resolves to our action in the same context, on each OS, and the
//! keys live leaves alone resolve to nothing.

use super::*;
use gpui_kit::{KeyContext, Keymap};

/// Live's key map in a Part Studio (its keyboard shortcuts, scopes "all" and "partStudio"): key → live's action. `cmd`
/// is ⌘ (Meta), which only a Mac takes; a Mac takes the `ctrl` keys too (⌃Z and ⌘Z undo, ⌃← stays ⌃).
const PART_STUDIO: &[(&str, &str)] = &[
    ("[", "showMeasure"), ("`", "selectOther"), ("a", "flipPrimaryAxis"), ("alt-c", "openCommandSearch"),
    ("alt-d", "openDebugPanel"), ("alt-t", "openTabManager"), ("backspace", "deleteSelection"),
    ("cmd-c", "copyOccurrences"), ("cmd-z", "undoChange"), ("ctrl-c", "copyOccurrences"),
    ("ctrl-d", "openDebugPanel"), ("ctrl-down", "precisionRotateDown"), ("ctrl-left", "precisionRotateLeft"),
    ("ctrl-m", "createFeatureMateConnector"), ("ctrl-right", "precisionRotateRight"),
    ("ctrl-shift-down", "panRotateDown"), ("ctrl-shift-f", "toggleFeatureScriptSearch"),
    ("ctrl-shift-left", "panRotateLeft"), ("ctrl-shift-right", "panRotateRight"), ("ctrl-shift-up", "panRotateUp"),
    ("ctrl-space", "openElementTabSwitcher"), ("ctrl-u", "openContactSupport"), ("ctrl-up", "precisionRotateUp"),
    ("ctrl-y", "redoChange"), ("ctrl-z", "undoChange"), ("delete", "deleteSelection"), ("down", "rotateDown"),
    ("escape", "cancel"), ("f", "zoomToFit"), ("k", "hideShowMateConnector"), ("left", "rotateLeft"),
    ("n", "viewNormalTo"), ("p", "hideShowPlanes"), ("q", "flipSecondaryAxis"), ("right", "rotateRight"),
    ("s", "openMiniToolbar"), ("shift-/", "openKeyboardShortcuts"), ("shift-1", "frontView"), ("shift-2", "backView"),
    ("shift-3", "leftView"), ("shift-4", "rightView"), ("shift-5", "topView"), ("shift-6", "bottomView"),
    ("shift-7", "isometricView"), ("shift-c", "curveSurfaceAnalysis"), ("shift-cmd-f", "toggleFeatureScriptSearch"),
    ("shift-cmd-z", "redoChange"), ("shift-d", "showDihedral"), ("shift-down", "coarseRotateDown"),
    ("shift-e", "createFeatureExtrude"), ("shift-f", "createFeatureFillet"), ("shift-h", "hideShowSketches"),
    ("shift-i", "makeIsolate"), ("shift-left", "coarseRotateLeft"), ("shift-n", "renameSelection"),
    ("shift-p", "hideAllConstruction"), ("shift-r", "viewInHighQuality"), ("shift-right", "coarseRotateRight"),
    ("shift-s", "createFeatureSketch"), ("shift-t", "makeTransparent"), ("shift-up", "coarseRotateUp"),
    ("shift-v", "namedView"), ("shift-w", "revolve"), ("shift-x", "sectionView"), ("shift-y", "showSelectedParts"),
    ("shift-z", "zoomIn"), ("space", "clearSelection"), ("up", "rotateUp"), ("w", "zoomToWindow"),
    ("y", "hideSelectedParts"), ("z", "zoomOut"),
];

/// The same in a sketch (scopes "all" and "sketchEdit").
const SKETCH: &[(&str, &str)] = &[
    ("[", "showMeasure"), ("`", "selectOther"), ("a", "threePointArc"), ("alt-c", "openCommandSearch"),
    ("alt-d", "openDebugPanel"), ("alt-t", "openTabManager"), ("b", "parallel"), ("backspace", "deleteSelection"),
    ("c", "centerPointCircle"), ("cmd-c", "copyOccurrences"), ("cmd-v", "pasteOccurrences"), ("cmd-z", "undoChange"),
    ("ctrl-c", "copyOccurrences"), ("ctrl-d", "openDebugPanel"), ("ctrl-down", "precisionRotateDown"),
    ("ctrl-left", "precisionRotateLeft"), ("ctrl-m", "createFeatureMateConnector"),
    ("ctrl-right", "precisionRotateRight"), ("ctrl-shift-down", "panRotateDown"),
    ("ctrl-shift-f", "toggleFeatureScriptSearch"), ("ctrl-shift-left", "panRotateLeft"),
    ("ctrl-shift-right", "panRotateRight"), ("ctrl-shift-up", "panRotateUp"),
    ("ctrl-space", "openElementTabSwitcher"), ("ctrl-u", "openContactSupport"), ("ctrl-up", "precisionRotateUp"),
    ("ctrl-v", "pasteOccurrences"), ("ctrl-y", "redoChange"), ("ctrl-z", "undoChange"), ("d", "dimension"),
    ("delete", "deleteSelection"), ("down", "rotateDown"), ("e", "equal"), ("escape", "cancel"), ("f", "zoomToFit"),
    ("g", "cornerRectangle"), ("h", "horizontal"), ("i", "coincident"), ("k", "hideShowMateConnector"), ("l", "line"),
    ("left", "rotateLeft"), ("m", "trim"), ("n", "viewNormalTo"), ("o", "offset"), ("p", "hideShowPlanes"),
    ("q", "toggleConstruction"), ("r", "centerPointRectangle"), ("right", "rotateRight"), ("s", "openMiniToolbar"),
    ("shift-/", "openKeyboardShortcuts"), ("shift-1", "frontView"), ("shift-2", "backView"), ("shift-3", "leftView"),
    ("shift-4", "rightView"), ("shift-5", "topView"), ("shift-6", "bottomView"), ("shift-7", "isometricView"),
    ("shift-a", "lineTangentArcSwitch"), ("shift-c", "curveSurfaceAnalysis"),
    ("shift-cmd-f", "toggleFeatureScriptSearch"), ("shift-cmd-z", "redoChange"), ("shift-d", "showDihedral"),
    ("shift-down", "coarseRotateDown"), ("shift-e", "createFeatureExtrude"), ("shift-f", "fillet"),
    ("shift-g", "pierce"), ("shift-h", "hideShowSketches"), ("shift-i", "makeIsolate"), ("shift-j", "fix"),
    ("shift-k", "normal"), ("shift-l", "perpendicular"), ("shift-left", "coarseRotateLeft"), ("shift-m", "midpoint"),
    ("shift-n", "renameSelection"), ("shift-o", "concentric"), ("shift-p", "hideAllConstruction"),
    ("shift-q", "symmetric"), ("shift-r", "viewInHighQuality"), ("shift-right", "coarseRotateRight"),
    ("shift-s", "point"), ("shift-t", "makeTransparent"), ("shift-u", "curvature"), ("shift-up", "coarseRotateUp"),
    ("shift-v", "namedView"), ("shift-w", "revolve"), ("shift-x", "sectionView"), ("shift-y", "showSelectedParts"),
    ("shift-z", "zoomIn"), ("space", "clearSelection"), ("t", "tangent"), ("u", "use"), ("up", "rotateUp"),
    ("v", "vertical"), ("w", "zoomToWindow"), ("x", "extend"), ("y", "hideSelectedParts"), ("z", "zoomOut"),
];

/// Every other key live pressed there: no action on either OS (the browser's own, or nothing).
const UNBOUND: &[&str] = &[
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "cmd--", "cmd-0", "cmd-1", "cmd-2", "cmd-3", "cmd-4", "cmd-5",
    "cmd-6", "cmd-7", "cmd-8", "cmd-9", "cmd-=", "cmd-a", "cmd-b", "cmd-d", "cmd-down", "cmd-e", "cmd-f", "cmd-g",
    "cmd-h", "cmd-i", "cmd-k", "cmd-l", "cmd-left", "cmd-m", "cmd-n", "cmd-o", "cmd-p", "cmd-r", "cmd-right", "cmd-s",
    "cmd-space", "cmd-t", "cmd-u", "cmd-up", "cmd-w", "cmd-x", "cmd-y", "ctrl--", "ctrl-0", "ctrl-1", "ctrl-2",
    "ctrl-3", "ctrl-4", "ctrl-5", "ctrl-6", "ctrl-7", "ctrl-8", "ctrl-9", "ctrl-=", "ctrl-a", "ctrl-b", "ctrl-e",
    "ctrl-f", "ctrl-g", "ctrl-h", "ctrl-i", "ctrl-k", "ctrl-l", "ctrl-n", "ctrl-o", "ctrl-p", "ctrl-r", "ctrl-s",
    "ctrl-shift-z", "ctrl-t", "ctrl-w", "ctrl-x", "end", "enter", "f1", "f10", "f11", "f12", "f2", "f3", "f4", "f5",
    "f6", "f7", "f8", "f9", "home", "j", "pagedown", "pageup", "shift-cmd-down", "shift-cmd-left", "shift-cmd-right",
    "shift-cmd-up", "shift-tab", "tab",
];

/// Our action for live's (None: a cloud panel left out, debug and contact support).
fn ours(live: &str) -> Option<&'static str> {
    Some(match live {
        "cancel" => "Cancel",
        "undoChange" => "Undo",
        "redoChange" => "Redo",
        "toggleFeatureScriptSearch" => "SearchFeatureStudios",
        "openCommandSearch" => "SearchTools",
        "openKeyboardShortcuts" => "ToggleShortcutHelp",
        "openTabManager" => "TabManager",
        "openElementTabSwitcher" => "SwitchRecentTab",
        "openMiniToolbar" => "ShortcutToolbar",
        "deleteSelection" => "DeleteSelection",
        "clearSelection" => "ClearSelection",
        "selectOther" => "SelectOther",
        "renameSelection" => "RenameSelected",
        "copyOccurrences" => "Copy",
        "pasteOccurrences" => "Paste",
        "frontView" => "ViewFront",
        "backView" => "ViewBack",
        "leftView" => "ViewLeft",
        "rightView" => "ViewRight",
        "topView" => "ViewTop",
        "bottomView" => "ViewBottom",
        "isometricView" => "ViewIsometric",
        "viewNormalTo" => "ViewNormal",
        "namedView" => "NamedViews",
        "zoomToFit" => "ZoomToFit",
        "zoomIn" => "ZoomIn",
        "zoomOut" => "ZoomOut",
        "zoomToWindow" => "ZoomWindow",
        "rotateLeft" => "RotateLeft",
        "rotateRight" => "RotateRight",
        "rotateUp" => "RotateUp",
        "rotateDown" => "RotateDown",
        "precisionRotateLeft" => "RotateLeftFine",
        "precisionRotateRight" => "RotateRightFine",
        "precisionRotateUp" => "RotateUpFine",
        "precisionRotateDown" => "RotateDownFine",
        "coarseRotateLeft" => "RotateLeft90",
        "coarseRotateRight" => "RotateRight90",
        "coarseRotateUp" => "RotateUp90",
        "coarseRotateDown" => "RotateDown90",
        "panRotateLeft" => "PanLeft",
        "panRotateRight" => "PanRight",
        "panRotateUp" => "PanUp",
        "panRotateDown" => "PanDown",
        "hideShowPlanes" => "TogglePlanes",
        "hideShowSketches" => "ToggleSketches",
        "hideShowMateConnector" => "ToggleMateConnectors",
        "hideAllConstruction" => "HideConstruction",
        "hideSelectedParts" => "HideSelected",
        "showSelectedParts" => "ShowAll",
        "makeIsolate" => "IsolateSelected",
        "makeTransparent" => "TransparentSelected",
        "sectionView" => "SectionView",
        "viewInHighQuality" => "ViewHighQuality",
        "showMeasure" => "MeasureTool",
        "showDihedral" => "DihedralAnalysis",
        "curveSurfaceAnalysis" => "CurveSurfaceAnalysis",
        "createFeatureSketch" => "NewSketch",
        "createFeatureExtrude" => "Extrude",
        "revolve" => "Revolve",
        "createFeatureFillet" => "Fillet",
        "createFeatureMateConnector" => "MateConnectorTool",
        "flipPrimaryAxis" => "FlipPrimaryAxis",
        "flipSecondaryAxis" => "ReorientSecondaryAxis",
        "line" => "SketchLine",
        "threePointArc" => "SketchArc3",
        "centerPointCircle" => "SketchCircle",
        "centerPointRectangle" => "SketchCenterRectangle",
        "cornerRectangle" => "SketchCornerRectangle",
        "point" => "SketchPoint",
        "fillet" => "SketchFillet",
        "offset" => "SketchOffset",
        "trim" => "SketchTrim",
        "extend" => "SketchExtend",
        "use" => "SketchUse",
        "dimension" => "SketchDimension",
        "toggleConstruction" => "SketchConstruction",
        "lineTangentArcSwitch" => "SketchTangentArcToggle",
        "coincident" => "ConstraintCoincident",
        "concentric" => "ConstraintConcentric",
        "curvature" => "ConstraintCurvature",
        "equal" => "ConstraintEqual",
        "fix" => "ConstraintFix",
        "horizontal" => "ConstraintHorizontal",
        "vertical" => "ConstraintVertical",
        "midpoint" => "ConstraintMidpoint",
        "normal" => "ConstraintNormal",
        "parallel" => "ConstraintParallel",
        "perpendicular" => "ConstraintPerpendicular",
        "pierce" => "ConstraintPierce",
        "symmetric" => "ConstraintSymmetric",
        "tangent" => "ConstraintTangent",
        "openDebugPanel" | "openContactSupport" => return None,
        other => panic!("no action of ours for live's {other}"),
    })
}

/// What a key may run beyond live's map: the desktop app's window keys (New, Open, Close…, the browser's own in live),
/// Paste in a Part Studio (live pastes a copied part from the paste event, not its key map), Tab and Shift+Tab walking
/// the focus (the browser's, through a dialog's fields) and Enter, which accepts a dialog and otherwise only commits
/// what is open (a sketch's step, a Section view, Select other: never the sketch, behaviours "Enter nor Esc").
fn also_ours(context: &str, key: &str, action: &str) -> bool {
    matches!(action, "NewDocument" | "OpenDocument" | "SaveCopyAs" | "CloseWindow" | "Quit")
        || (action == "Paste" && matches!(key, "ctrl-v" | "cmd-v"))
        || matches!((key, action), ("enter", "Accept") | ("tab", "StudioTab") | ("shift-tab", "StudioTabPrev"))
        || (context == "dialog" && matches!((key, action), ("tab", "DialogTab") | ("shift-tab", "DialogTabPrev")))
}

/// Actions the app's workspace handles (not in a Part Studio's window of its own).
const WORKSPACE: &[&str] = &["Undo", "Redo", "SearchTools", "SearchFeatureStudios", "TabManager", "SwitchRecentTab", "ToggleShortcutHelp"];

/// The window's key contexts where keys go (the canvas or the dialog focused) and the actions handled there.
fn contexts(cx: &mut TestAppContext, window: AnyWindowHandle) -> (Vec<KeyContext>, Vec<&'static str>) {
    draw(cx, window);
    cx.update_window(window, |_, w, cx| {
        let handled = crate::actions::key_bindings_for(false).iter().filter(|b| w.is_action_available(b.action(), cx)).map(|b| name(b)).collect();
        (w.context_stack(), handled)
    })
    .unwrap()
}

fn name(b: &gpui_kit::KeyBinding) -> &'static str {
    b.action().name().rsplit("::").next().unwrap_or_default()
}

#[gpui::test]
fn every_live_key_runs_our_action_in_the_part_studio_dialogs_and_sketches(cx: &mut TestAppContext) {
    let (window, view) = part_studio(cx, "keys-matrix");
    cx.update_window(window, |_, w, cx| {
        w.activate_window();
        let focus = view.read(cx).focus.clone();
        w.focus(&focus, cx);
    })
    .unwrap();
    let part_studio = contexts(cx, window);
    cx.update_window(window, |_, w, cx| view.update(cx, |v, cx| v.start_feature("extrude", w, cx))).unwrap();
    let dialog = contexts(cx, window);
    let (window, view) = sketch_with(cx, "keys-matrix-sketch", Tool::Select);
    let sketch = contexts(cx, window);
    sketch_in_tool(cx, window, view, Tool::Line);
    let tool = contexts(cx, window);
    let mut wrong = Vec::new();
    for mac in [false, true] {
        let keymap = Keymap::new(crate::actions::key_bindings_for(mac));
        for (place, (stack, handled), map, other) in [
            ("part studio", &part_studio, PART_STUDIO, SKETCH),
            ("dialog", &dialog, PART_STUDIO, SKETCH),
            ("sketch", &sketch, SKETCH, PART_STUDIO),
            ("sketch tool", &tool, SKETCH, PART_STUDIO),
        ] {
            let unbound = UNBOUND.iter().copied().chain(other.iter().map(|o| o.0).filter(|k| !map.iter().any(|m| m.0 == *k)));
            let keys = map.iter().map(|(k, live)| (*k, Some(*live))).chain(unbound.map(|k| (k, None)));
            for (key, live) in keys {
                let (bindings, _) = keymap.bindings_for_input(&[Keystroke::parse(key).unwrap()], stack);
                let got = bindings.first().map(name).filter(|a| *a != "NoAction");
                let meta_off_mac = !mac && key.split('-').any(|m| m == "cmd");
                // Live's axis flips act on a dialog's mate connector; with none open the key does nothing.
                let no_dialog = place == "part studio" && matches!(live, Some("flipPrimaryAxis" | "flipSecondaryAxis"));
                let want = live.filter(|_| !meta_off_mac && !no_dialog).and_then(ours);
                if got != want && !got.is_some_and(|a| want.is_none() && also_ours(place, key, a)) {
                    wrong.push(format!("{} {place} {key}: live {live:?} → {want:?}, ours {got:?}", if mac { "mac" } else { "win" }));
                } else if let Some(a) = want.filter(|a| !mac && !handled.contains(a) && !WORKSPACE.contains(a)) {
                    wrong.push(format!("{place} {key}: {a} bound but not handled"));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{} keys differ from live (contexts {:?} / {:?} / {:?} / {:?}):\n{}", wrong.len(), part_studio.0, dialog.0, sketch.0, tool.0, wrong.join("\n"));
}
