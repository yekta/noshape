# noshape user guide

noshape works like Onshape. If you know Onshape, most things are where you expect them,
including the keyboard shortcuts. This page covers what is specific to a local app.

## Documents

- **Start window.** Opens when you launch noshape without a file. It shows recent
  documents and folders you add, with thumbnails. Ctrl/⌘+N asks for a name and creates
  a document in `~/Documents/noshape` or the selected folder.
- **One file per document.** A document is a single `.noshape` file holding every tab
  and the complete history. There is no Save: every change is stored as it happens.
  Undo/redo (Ctrl/⌘+Z, Ctrl+Y or ⇧⌘Z) work across sessions.
- **Versions and history** (Alt+H):
  - create named versions;
  - view any point in history read-only;
  - restore to it;
  - branch from it;
  - merge branches (Part Studios merge feature by feature);
  - compare any point with the current state (right-click a history entry).
- **Tabs:** Part Studios, Assemblies, Drawings, Variable Studios and Feature Studios
  (custom features). Add them with **+** in the tab bar.
- **Import and export.**
  - **Import** STEP, IGES, BREP, STL and OBJ into a Part Studio from the document menu.
    Meshes use the document's length unit.
  - **Export** parts as STEP, IGES, STL, OBJ or BREP, and drawings as PDF (all sheets),
    SVG or DXF.

## Part Studios

- **Feature dialogs** preview live.
  - Enter accepts, Shift+Enter accepts and repeats, Esc cancels.
  - **Final** previews the finished model instead of rolling back to the edited feature.
  - Some features have drag handles in the viewport: the extrude depth arrow and the
    revolve angle arc.
- **Feature list:**
  - drag the rollback bar, or use ↑/↓ while the list has focus;
  - drag features to reorder;
  - right-click for suppress, rename, roll to here, copy/paste and Add to new folder;
  - copied features can be pasted into another document.
- **Variables and configurations:** use the buttons at the top right of the viewport.
  Any numeric field accepts expressions such as `#width / 2 + 1/4 in`.
- **Derived:** brings parts from another Part Studio in the same document, or from
  another document pinned to a version. When a newer version exists, the dialog shows
  an **Update** button.
- **Sheet metal and frames:** the sheet metal panel shows the flat pattern, the bend
  table and DXF/SVG export. The cut list panel shows frame members. Both appear at
  the top right once the Part Studio has such parts.
- **Custom features:** written in a Feature Studio. See
  [custom-features.md](custom-features.md).

## Shortcuts

| Keys | Action |
|---|---|
| Alt+C | Search tools |
| Shift+? | Shortcut sheet |
| Shift+S / Shift+E / Shift+W / Shift+F | Sketch / Extrude / Revolve / Fillet |
| Shift+1 … Shift+7 | Front, Back, Left, Right, Top, Bottom, Isometric |
| N | View normal to selection |
| F | Zoom to fit |
| Z / Shift+Z | Zoom out / in |
| Arrows | Rotate 15° (Ctrl 5°, Shift 90°); Ctrl+Shift+arrows pan |
| O | Orthographic / perspective |
| P / Shift+P | Show/hide planes / construction |
| Y / Shift+Y / Shift+I / Shift+T | Hide / show all / isolate / transparent |
| Shift+X | Section view |
| Shift+V | Named views |
| Shift+H | Show/hide sketches |
| Shift+N | Rename the selected feature |
| Space / Delete | Clear selection / delete selection (selected parts get a Delete part feature) |
| S | Shortcut toolbar at the cursor |
| ` / Shift+` | Select other: faces and edges under the cursor, nearest first |
| W | Zoom to window (drag a box) |
| [ | Measure |
| Ctrl+M / K | Mate connector / show-hide mate connectors |
| Enter | Accept the dialog, or edit the selected feature |

**Mouse:** right-drag rotates, middle-drag or Ctrl+right-drag pans, the wheel
zooms at the cursor, and a double middle-click zooms to fit. Click the view cube's
faces, edges or corners for those views; its arrows rotate 15° (Shift 90°, Ctrl 5°)
and the menu under it has the camera and render options. In a dialog, the wheel
over a focused number field steps it by 0.1 (Ctrl 0.01, Shift 1).

**Feature list:** clicks add to the selection, Shift+click selects a range, and
dragging moves every selected feature (not above what it depends on). Type in the
filter box to search by name or type, or use `:errors`, `:suppressed`, `:type …`,
`:variable …`. Click the rollback bar, then use ↑/↓.

**In a sketch:**

| Keys | Tool |
|---|---|
| L | Line |
| A | 3-point arc |
| C | Circle |
| R / G | Center / corner rectangle |
| D | Dimension |
| M | Trim |
| X | Extend |
| O | Offset |
| U | Use (project) |
| Q | Construction |
| I | Coincident |
| E | Equal |
| H / V | Horizontal / vertical |

While drawing, type a number to enter an exact value. Tab switches fields.
