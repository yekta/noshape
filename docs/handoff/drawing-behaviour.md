# Handoff: Drawing behaviour parity (agent "asb")

Checklist (kept current): `docs/parity/drawing.md` — 365 Onshape drawing behaviours from the help
pages, each audited against the code (first audit 2026-10-18: 28 ✅, 142 🔧, 188 ❌, 7 n/a).
Agent "ui2" restyles drawing visuals in the same files: keep visual changes surgical.

## Plan (priority order)
1. Create drawing flow: "Create drawing of <name>" (Part Studio parts list, assembly instances /
   top-level), Create drawing dialog (name, template by standard, Suggested views / No views),
   "+" › Create Drawing with the same dialog.
2. Insert view dialog (source, orientation, scale, simplification, explode / display state) and
   View properties dialog (double-click a view).
3. View context menu in Onshape's order and labels; Show/hide submenu.
4. Tool behaviour: pressing a tool's shortcut / icon again exits it; dimension dropdown names and
   the missing dimension types; smart Dimension (d) picks; w, s, [, Shift+N, Ctrl+S, Ctrl+C/V.
5. Section / detail / auxiliary dialogs and editing; sheets panel and sheet properties.
6. Dimension palette and editing (grips, jog, break), note editor, property picker.
7. Sketch tools, hatch region, tables (revision, custom), export dialog.

## State
- Nothing implemented yet beyond the checklist.

## How to test
- `--demo-drawing-full` opens the showcase drawing; model tests `cargo test -p noshape-model --lib drawing::`.
