//! The shortcut map matrix (docs/parity/interaction-feel.md "Chrome"): every key of live's map (live2's
//! feel/chrome-shortcuts.json, the server's defaults for the `all`, `partStudio`, `sketchEdit` and `assembly` scopes, 11-13)
//! looked up in our keymap in that scope's context, with the Windows / Linux keys off a Mac and the Mac keys on one
//! (both checked on every OS through `key_bindings_for`). Live's Feedback / Report a bug and Open Debug Panel are
//! cloud and debugging rows, left out.

use super::*;
use gpui_kit::{KeyContext, Keymap, Keystroke};

/// (Scope, live's label, its Windows / Linux keys, its Mac keys, our action), keys `|`-separated as live writes them.
const LIVE: &[(&str, &str, &str, &str, &str)] = &[
    ("all", "Cancel", "Esc", "Esc", "Cancel"),
    ("all", "FeatureScript search", "Ctrl+Shift+F", "⇧ Shift+⌘ Cmd+F | ⌃ Control+⇧ Shift+F", "SearchFeatureStudios"),
    ("all", "Keyboard shortcuts", "Shift+/", "⇧ Shift+/", "ToggleShortcutHelp"),
    ("all", "Quick tab switching", "Ctrl+Space", "⌃ Control+Space", "SwitchRecentTab"),
    ("all", "Redo", "Ctrl+Y", "⌃ Control+Y | ⇧ Shift+⌘ Cmd+Z", "Redo"),
    ("all", "Search tools", "Alt+C", "⌥ Option+C", "SearchTools"),
    ("all", "Tab manager", "Alt+T", "⌥ Option+T", "TabManager"),
    ("all", "Undo", "Ctrl+Z", "⌘ Cmd+Z | ⌃ Control+Z", "Undo"),
    ("partStudio", "Back view", "Shift+2", "⇧ Shift+2", "ViewBack"),
    ("partStudio", "Bottom view", "Shift+6", "⇧ Shift+6", "ViewBottom"),
    ("partStudio", "Coarse rotate down", "Shift+Down", "⇧ Shift+Down", "RotateDown90"),
    ("partStudio", "Coarse rotate left", "Shift+Left", "⇧ Shift+Left", "RotateLeft90"),
    ("partStudio", "Coarse rotate right", "Shift+Right", "⇧ Shift+Right", "RotateRight90"),
    ("partStudio", "Coarse rotate up", "Shift+Up", "⇧ Shift+Up", "RotateUp90"),
    ("partStudio", "Front view", "Shift+1", "⇧ Shift+1", "ViewFront"),
    ("partStudio", "Hide all construction", "Shift+P", "⇧ Shift+P", "HideConstruction"),
    ("partStudio", "Hide selected parts", "Y", "Y", "HideSelected"),
    ("partStudio", "Hide/show planes", "P", "P", "TogglePlanes"),
    ("partStudio", "Isometric view", "Shift+7", "⇧ Shift+7", "ViewIsometric"),
    ("partStudio", "Left view", "Shift+3", "⇧ Shift+3", "ViewLeft"),
    ("partStudio", "Make isolate", "Shift+I", "⇧ Shift+I", "IsolateSelected"),
    ("partStudio", "Make transparent", "Shift+T", "⇧ Shift+T", "TransparentSelected"),
    ("partStudio", "Named view", "Shift+V", "⇧ Shift+V", "NamedViews"),
    ("partStudio", "Pan rotate down", "Ctrl+Shift+Down", "⌃ Control+⇧ Shift+Down", "PanDown"),
    ("partStudio", "Pan rotate left", "Ctrl+Shift+Left", "⌃ Control+⇧ Shift+Left", "PanLeft"),
    ("partStudio", "Pan rotate right", "Ctrl+Shift+Right", "⌃ Control+⇧ Shift+Right", "PanRight"),
    ("partStudio", "Pan rotate up", "Ctrl+Shift+Up", "⌃ Control+⇧ Shift+Up", "PanUp"),
    ("partStudio", "Precision rotate down", "Ctrl+Down", "⌃ Control+Down", "RotateDownFine"),
    ("partStudio", "Precision rotate left", "Ctrl+Left", "⌃ Control+Left", "RotateLeftFine"),
    ("partStudio", "Precision rotate right", "Ctrl+Right", "⌃ Control+Right", "RotateRightFine"),
    ("partStudio", "Precision rotate up", "Ctrl+Up", "⌃ Control+Up", "RotateUpFine"),
    ("partStudio", "Right view", "Shift+4", "⇧ Shift+4", "ViewRight"),
    ("partStudio", "Rotate down", "Down", "Down", "RotateDown"),
    ("partStudio", "Rotate left", "Left", "Left", "RotateLeft"),
    ("partStudio", "Rotate right", "Right", "Right", "RotateRight"),
    ("partStudio", "Rotate up", "Up", "Up", "RotateUp"),
    ("partStudio", "Section view", "Shift+X", "⇧ Shift+X", "SectionView"),
    ("partStudio", "Show selected parts", "Shift+Y", "⇧ Shift+Y", "ShowAll"),
    ("partStudio", "Top view", "Shift+5", "⇧ Shift+5", "ViewTop"),
    ("partStudio", "View in high quality", "Shift+R", "⇧ Shift+R", "ViewHighQuality"),
    ("partStudio", "View normal to", "N", "N", "ViewNormal"),
    ("partStudio", "Zoom in", "Shift+Z", "⇧ Shift+Z", "ZoomIn"),
    ("partStudio", "Zoom out", "Z", "Z", "ZoomOut"),
    ("partStudio", "Zoom to fit", "F", "F", "ZoomToFit"),
    ("partStudio", "Zoom to window", "W", "W", "ZoomWindow"),
    ("partStudio", "Clear selection", "Space", "Space", "ClearSelection"),
    ("partStudio", "Copy", "Ctrl+C", "⌘ Cmd+C | ⌃ Control+C", "Copy"),
    ("partStudio", "Curve/surface analysis", "Shift+C", "⇧ Shift+C", "CurveSurfaceAnalysis"),
    ("partStudio", "Delete selection", "Delete | Backspace", "Delete | Backspace", "DeleteSelection"),
    ("partStudio", "Dihedral analysis", "Shift+D", "⇧ Shift+D", "DihedralAnalysis"),
    ("partStudio", "Flip primary axis", "A", "A", "FlipPrimaryAxis"),
    ("partStudio", "Hide/show mate connectors", "K", "K", "ToggleMateConnectors"),
    ("partStudio", "Mate connector", "Ctrl+M", "⌃ Control+M", "MateConnectorTool"),
    ("partStudio", "Measure", "[", "[", "MeasureTool"),
    ("partStudio", "Rename selection", "Shift+N", "⇧ Shift+N", "RenameSelected"),
    ("partStudio", "Reorient secondary axis", "Q", "Q", "ReorientSecondaryAxis"),
    ("partStudio", "Select other", "`", "`", "SelectOther"),
    ("partStudio", "Shortcut toolbars", "S", "S", "ShortcutToolbar"),
    ("partStudio", "Extrude", "Shift+E", "⇧ Shift+E", "Extrude"),
    ("partStudio", "Fillet", "Shift+F", "⇧ Shift+F", "Fillet"),
    ("partStudio", "Hide/show sketches", "Shift+H", "⇧ Shift+H", "ToggleSketches"),
    ("partStudio", "Revolve", "Shift+W", "⇧ Shift+W", "Revolve"),
    ("partStudio", "Sketch", "Shift+S", "⇧ Shift+S", "NewSketch"),
    ("sketchEdit", "Back view", "Shift+2", "⇧ Shift+2", "ViewBack"),
    ("sketchEdit", "Bottom view", "Shift+6", "⇧ Shift+6", "ViewBottom"),
    ("sketchEdit", "Coarse rotate down", "Shift+Down", "⇧ Shift+Down", "RotateDown90"),
    ("sketchEdit", "Coarse rotate left", "Shift+Left", "⇧ Shift+Left", "RotateLeft90"),
    ("sketchEdit", "Coarse rotate right", "Shift+Right", "⇧ Shift+Right", "RotateRight90"),
    ("sketchEdit", "Coarse rotate up", "Shift+Up", "⇧ Shift+Up", "RotateUp90"),
    ("sketchEdit", "Front view", "Shift+1", "⇧ Shift+1", "ViewFront"),
    ("sketchEdit", "Hide all construction", "Shift+P", "⇧ Shift+P", "HideConstruction"),
    ("sketchEdit", "Hide selected parts", "Y", "Y", "HideSelected"),
    ("sketchEdit", "Hide/show planes", "P", "P", "TogglePlanes"),
    ("sketchEdit", "Isometric view", "Shift+7", "⇧ Shift+7", "ViewIsometric"),
    ("sketchEdit", "Left view", "Shift+3", "⇧ Shift+3", "ViewLeft"),
    ("sketchEdit", "Make isolate", "Shift+I", "⇧ Shift+I", "IsolateSelected"),
    ("sketchEdit", "Make transparent", "Shift+T", "⇧ Shift+T", "TransparentSelected"),
    ("sketchEdit", "Named view", "Shift+V", "⇧ Shift+V", "NamedViews"),
    ("sketchEdit", "Pan rotate down", "Ctrl+Shift+Down", "⌃ Control+⇧ Shift+Down", "PanDown"),
    ("sketchEdit", "Pan rotate left", "Ctrl+Shift+Left", "⌃ Control+⇧ Shift+Left", "PanLeft"),
    ("sketchEdit", "Pan rotate right", "Ctrl+Shift+Right", "⌃ Control+⇧ Shift+Right", "PanRight"),
    ("sketchEdit", "Pan rotate up", "Ctrl+Shift+Up", "⌃ Control+⇧ Shift+Up", "PanUp"),
    ("sketchEdit", "Precision rotate down", "Ctrl+Down", "⌃ Control+Down", "RotateDownFine"),
    ("sketchEdit", "Precision rotate left", "Ctrl+Left", "⌃ Control+Left", "RotateLeftFine"),
    ("sketchEdit", "Precision rotate right", "Ctrl+Right", "⌃ Control+Right", "RotateRightFine"),
    ("sketchEdit", "Precision rotate up", "Ctrl+Up", "⌃ Control+Up", "RotateUpFine"),
    ("sketchEdit", "Right view", "Shift+4", "⇧ Shift+4", "ViewRight"),
    ("sketchEdit", "Rotate down", "Down", "Down", "RotateDown"),
    ("sketchEdit", "Rotate left", "Left", "Left", "RotateLeft"),
    ("sketchEdit", "Rotate right", "Right", "Right", "RotateRight"),
    ("sketchEdit", "Rotate up", "Up", "Up", "RotateUp"),
    ("sketchEdit", "Section view", "Shift+X", "⇧ Shift+X", "SectionView"),
    ("sketchEdit", "Show selected parts", "Shift+Y", "⇧ Shift+Y", "ShowAll"),
    ("sketchEdit", "Top view", "Shift+5", "⇧ Shift+5", "ViewTop"),
    ("sketchEdit", "View in high quality", "Shift+R", "⇧ Shift+R", "ViewHighQuality"),
    ("sketchEdit", "View normal to", "N", "N", "ViewNormal"),
    ("sketchEdit", "Zoom in", "Shift+Z", "⇧ Shift+Z", "ZoomIn"),
    ("sketchEdit", "Zoom out", "Z", "Z", "ZoomOut"),
    ("sketchEdit", "Zoom to fit", "F", "F", "ZoomToFit"),
    ("sketchEdit", "Zoom to window", "W", "W", "ZoomWindow"),
    ("sketchEdit", "Clear selection", "Space", "Space", "ClearSelection"),
    ("sketchEdit", "Copy", "Ctrl+C", "⌘ Cmd+C | ⌃ Control+C", "Copy"),
    ("sketchEdit", "Curve/surface analysis", "Shift+C", "⇧ Shift+C", "CurveSurfaceAnalysis"),
    ("sketchEdit", "Delete selection", "Delete | Backspace", "Delete | Backspace", "DeleteSelection"),
    ("sketchEdit", "Dihedral analysis", "Shift+D", "⇧ Shift+D", "DihedralAnalysis"),
    ("sketchEdit", "Hide/show mate connectors", "K", "K", "ToggleMateConnectors"),
    ("sketchEdit", "Mate connector", "Ctrl+M", "⌃ Control+M", "MateConnectorTool"),
    ("sketchEdit", "Measure", "[", "[", "MeasureTool"),
    ("sketchEdit", "Paste", "Ctrl+V", "⌘ Cmd+V | ⌃ Control+V", "Paste"),
    ("sketchEdit", "Rename selection", "Shift+N", "⇧ Shift+N", "RenameSelected"),
    ("sketchEdit", "Select other", "`", "`", "SelectOther"),
    ("sketchEdit", "Shortcut toolbars", "S", "S", "ShortcutToolbar"),
    ("sketchEdit", "Extrude", "Shift+E", "⇧ Shift+E", "Extrude"),
    ("sketchEdit", "Hide/show sketches", "Shift+H", "⇧ Shift+H", "ToggleSketches"),
    ("sketchEdit", "Revolve", "Shift+W", "⇧ Shift+W", "Revolve"),
    ("sketchEdit", "3 point arc", "A", "A", "SketchArc3"),
    ("sketchEdit", "Center point circle", "C", "C", "SketchCircle"),
    ("sketchEdit", "Center point rectangle", "R", "R", "SketchCenterRectangle"),
    ("sketchEdit", "Coincident", "I", "I", "ConstraintCoincident"),
    ("sketchEdit", "Concentric", "Shift+O", "⇧ Shift+O", "ConstraintConcentric"),
    ("sketchEdit", "Corner rectangle", "G", "G", "SketchCornerRectangle"),
    ("sketchEdit", "Curvature", "Shift+U", "⇧ Shift+U", "ConstraintCurvature"),
    ("sketchEdit", "Dimension", "D", "D", "SketchDimension"),
    ("sketchEdit", "Equal", "E", "E", "ConstraintEqual"),
    ("sketchEdit", "Extend", "X", "X", "SketchExtend"),
    ("sketchEdit", "Fix", "Shift+J", "⇧ Shift+J", "ConstraintFix"),
    ("sketchEdit", "Horizontal", "H", "H", "ConstraintHorizontal"),
    ("sketchEdit", "Line", "L", "L", "SketchLine"),
    ("sketchEdit", "Line/tangent arc switch", "Shift+A", "⇧ Shift+A", "SketchTangentArcToggle"),
    ("sketchEdit", "Midpoint", "Shift+M", "⇧ Shift+M", "ConstraintMidpoint"),
    ("sketchEdit", "Normal", "Shift+K", "⇧ Shift+K", "ConstraintNormal"),
    ("sketchEdit", "Offset", "O", "O", "SketchOffset"),
    ("sketchEdit", "Parallel", "B", "B", "ConstraintParallel"),
    ("sketchEdit", "Perpendicular", "Shift+L", "⇧ Shift+L", "ConstraintPerpendicular"),
    ("sketchEdit", "Pierce", "Shift+G", "⇧ Shift+G", "ConstraintPierce"),
    ("sketchEdit", "Point", "Shift+S", "⇧ Shift+S", "SketchPoint"),
    ("sketchEdit", "Sketch fillet", "Shift+F", "⇧ Shift+F", "SketchFillet"),
    ("sketchEdit", "Symmetric", "Shift+Q", "⇧ Shift+Q", "ConstraintSymmetric"),
    ("sketchEdit", "Tangent", "T", "T", "ConstraintTangent"),
    ("sketchEdit", "Toggle construction", "Q", "Q", "SketchConstruction"),
    ("sketchEdit", "Trim", "M", "M", "SketchTrim"),
    ("sketchEdit", "Use", "U", "U", "SketchUse"),
    ("sketchEdit", "Vertical", "V", "V", "ConstraintVertical"),
    ("assembly", "Back view", "Shift+2", "⇧ Shift+2", "ViewBack"),
    ("assembly", "Bottom view", "Shift+6", "⇧ Shift+6", "ViewBottom"),
    ("assembly", "Coarse rotate down", "Shift+Down", "⇧ Shift+Down", "RotateDown90"),
    ("assembly", "Coarse rotate left", "Shift+Left", "⇧ Shift+Left", "RotateLeft90"),
    ("assembly", "Coarse rotate right", "Shift+Right", "⇧ Shift+Right", "RotateRight90"),
    ("assembly", "Coarse rotate up", "Shift+Up", "⇧ Shift+Up", "RotateUp90"),
    ("assembly", "Front view", "Shift+1", "⇧ Shift+1", "ViewFront"),
    ("assembly", "Hide all construction", "Shift+P", "⇧ Shift+P", "HideConstruction"),
    ("assembly", "Hide selected parts", "Y", "Y", "HideSelected"),
    ("assembly", "Isometric view", "Shift+7", "⇧ Shift+7", "ViewIsometric"),
    ("assembly", "Left view", "Shift+3", "⇧ Shift+3", "ViewLeft"),
    ("assembly", "Make isolate", "Shift+I", "⇧ Shift+I", "IsolateSelected"),
    ("assembly", "Make transparent", "Shift+T", "⇧ Shift+T", "TransparentSelected"),
    ("assembly", "Named view", "Shift+V", "⇧ Shift+V", "NamedViews"),
    ("assembly", "Pan rotate down", "Ctrl+Shift+Down", "⌃ Control+⇧ Shift+Down", "PanDown"),
    ("assembly", "Pan rotate left", "Ctrl+Shift+Left", "⌃ Control+⇧ Shift+Left", "PanLeft"),
    ("assembly", "Pan rotate right", "Ctrl+Shift+Right", "⌃ Control+⇧ Shift+Right", "PanRight"),
    ("assembly", "Pan rotate up", "Ctrl+Shift+Up", "⌃ Control+⇧ Shift+Up", "PanUp"),
    ("assembly", "Precision rotate down", "Ctrl+Down", "⌃ Control+Down", "RotateDownFine"),
    ("assembly", "Precision rotate left", "Ctrl+Left", "⌃ Control+Left", "RotateLeftFine"),
    ("assembly", "Precision rotate right", "Ctrl+Right", "⌃ Control+Right", "RotateRightFine"),
    ("assembly", "Precision rotate up", "Ctrl+Up", "⌃ Control+Up", "RotateUpFine"),
    ("assembly", "Right view", "Shift+4", "⇧ Shift+4", "ViewRight"),
    ("assembly", "Rotate down", "Down", "Down", "RotateDown"),
    ("assembly", "Rotate left", "Left", "Left", "RotateLeft"),
    ("assembly", "Rotate right", "Right", "Right", "RotateRight"),
    ("assembly", "Rotate up", "Up", "Up", "RotateUp"),
    ("assembly", "Section view", "Shift+X", "⇧ Shift+X", "SectionView"),
    ("assembly", "Show selected parts", "Shift+Y", "⇧ Shift+Y", "ShowAll"),
    ("assembly", "Top view", "Shift+5", "⇧ Shift+5", "ViewTop"),
    ("assembly", "View in high quality", "Shift+R", "⇧ Shift+R", "ViewHighQuality"),
    ("assembly", "View normal to", "N", "N", "ViewNormal"),
    ("assembly", "Zoom in", "Shift+Z", "⇧ Shift+Z", "ZoomIn"),
    ("assembly", "Zoom out", "Z", "Z", "ZoomOut"),
    ("assembly", "Zoom to fit", "F", "F", "ZoomToFit"),
    ("assembly", "Zoom to window", "W", "W", "ZoomWindow"),
    ("assembly", "Hide/show mates", "J", "J", "ToggleMates"),
    ("assembly", "Insert dialog", "I", "I", "InsertInstance"),
    ("assembly", "Mate", "M", "M", "FastenMate"),
    ("assembly", "Show mates mode", "H", "H", "ToggleShowMatesMode"),
    ("assembly", "Snap mode", "Shift+S", "⇧ Shift+S", "ToggleSnapMode"),
    ("assembly", "Clear selection", "Space", "Space", "ClearSelection"),
    ("assembly", "Copy", "Ctrl+C", "⌘ Cmd+C | ⌃ Control+C", "Copy"),
    ("assembly", "Delete selection", "Delete | Backspace", "Delete | Backspace", "DeleteSelection"),
    ("assembly", "Flip primary axis", "A", "A", "FlipPrimaryAxis"),
    ("assembly", "Hide/show mate connectors", "K", "K", "ToggleMateConnectors"),
    ("assembly", "Mate connector", "Ctrl+M", "⌃ Control+M", "MateConnectorTool"),
    ("assembly", "Measure", "[", "[", "MeasureTool"),
    ("assembly", "Paste", "Ctrl+V", "⌘ Cmd+V | ⌃ Control+V", "Paste"),
    ("assembly", "Rename selection", "Shift+N", "⇧ Shift+N", "RenameSelected"),
    ("assembly", "Reorient secondary axis", "Q", "Q", "ReorientSecondaryAxis"),
    ("assembly", "Select other", "`", "`", "SelectOther"),
    ("assembly", "Shortcut toolbars", "S", "S", "ShortcutToolbar"),
];

/// Rows ours doesn't match yet (scope, label): the test fails when one starts matching or another stops.
const GAPS: &[(&str, &str)] = &[];

/// Live's "Ctrl+Shift+F" / "⇧ Shift+⌘ Cmd+F" as a GPUI keystroke ("ctrl-shift-f", "shift-cmd-f").
fn keystroke(live: &str) -> String {
    live.split('+')
        .map(|t| {
            let t = t.trim_start_matches(['⌃', '⇧', '⌘', '⌥']).trim();
            match t {
                "Ctrl" | "Control" => "ctrl".to_string(),
                "Shift" => "shift".to_string(),
                "Cmd" => "cmd".to_string(),
                "Alt" | "Option" => "alt".to_string(),
                "Esc" => "escape".to_string(),
                _ => t.to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// The scope's context; a mate connector dialog's own keys (A, Q) in its dialog, where they act in live.
fn stack(scope: &str, action: &str) -> Vec<KeyContext> {
    let names: &[&str] = match scope {
        "sketchEdit" => &["Workspace", "PartStudio", "Sketch", "FeatureDialog"],
        "assembly" => &["Workspace", "Assembly"],
        _ if matches!(action, "FlipPrimaryAxis" | "ReorientSecondaryAxis") => &["Workspace", "PartStudio", "FeatureDialog"],
        _ => &["Workspace", "PartStudio"],
    };
    names.iter().map(|n| KeyContext::parse(n).unwrap()).collect()
}

#[test]
fn the_shortcut_map_matches_lives() {
    let maps = [(false, Keymap::new(key_bindings_for(false))), (true, Keymap::new(key_bindings_for(true)))];
    let mut report = Vec::new();
    let mut moved = Vec::new();
    for &(scope, label, win, mac, want) in LIVE {
        let mut why = Vec::new();
        for (is_mac, keymap) in &maps {
            for k in (if *is_mac { mac } else { win }).split(" | ") {
                let key = keystroke(k);
                let found = keymap.bindings_for_input(&[Keystroke::parse(&key).unwrap()], &stack(scope, want)).0;
                let got = found.first().map(|b| b.action().name().rsplit("::").next().unwrap_or_default().to_string());
                if got.as_deref() != Some(want) {
                    why.push(format!("{} {key}: {}", if *is_mac { "mac" } else { "win/linux" }, got.unwrap_or_else(|| "nothing".into())));
                }
            }
        }
        let ok = why.is_empty();
        report.push(format!("{} {scope} {label} ({win}; mac {mac}) -> {want}{}", if ok { "=" } else { "≠" }, if ok { String::new() } else { format!(": {}", why.join(", ")) }));
        if ok == GAPS.contains(&(scope, label)) {
            moved.push(format!("{scope} {label}"));
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    let misses: Vec<&String> = report.iter().filter(|l| l.starts_with('≠')).collect();
    eprintln!("shortcut map: {matched} / {} rows match live\n{}", report.len(), misses.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert!(moved.is_empty(), "rows whose status moved (update interaction-feel.md and GAPS): {moved:?}");
}

