# Part Studio UI coverage audit (psui, 2026-11-12)

The lead asked (11-12) which Part Studio UI no ps-* scenario touches. Breadth was already complete
(tools/ui-parity/coverage.py: 76/76 toolbar tools, 119/121 menu items, 86/89 shortcuts), but at a median of one
scenario per item, and most tools only opened and cancelled. This audit counts depth.
Scripts: /root/psui-scratch/cov/ (scan.py, menus.py, depth.py).

## Before (594 Part Studio scenarios)

- **Feature dialogs** (75 tools; per tool: new, fields, ✓, ✗, edit, ✓ after the edit, re-edit). Only Extrude had a
  re-edit. Sketch, Extrude, Fillet, Shell and Hole had a ✓ on a new feature. 58 tools were only opened and closed with
  Esc. No ✗ after an edit kept the old values anywhere.
- **Right-click menus** (items live shows that no scenario clicks):
  - edge, with the edge selected: Fillet…, Chamfer…;
  - face / edge / part: Edit ‹feature›…, Edit ‹sketch›…, Show dependencies of…, Delete ‹feature›, Assign material…,
    Copy ‹part›, Create Drawing of…, Export…;
  - the Select ▸ submenu: Tangent connected faces, Fillet faces, Loop/chain connected edges, Protrusion / Pocket /
    Hole / Bounded faces;
  - the analysis submenu: Deviation, Dihedral, Draft, Flatten surfaces, Interference, Reflection, Zebra stripes,
    Connection;
  - a plane right-clicked in the graphics (never recorded);
  - a configuration row's Delete row and Switch to; the Appearance panel's Move down.
- **Panels:**
  - Mass properties: one scenario (one part, Part tab);
  - Section view: one (Front, ✓), never edited or turned off;
  - Named views: opened and closed, never a view added;
  - Appearance editor: the face dialog's R field only, never a part's hex / RGB popup;
  - View cube: Top and Isometric only.
- **Not measurable yet:** the state has no camera, no Measure readout, no mass values and no preview geometry. View
  cube, named view, Measure and Mass scenarios compare only selection and dialogs.

## Batch 1 (50 scenarios, psui 11-12)

- **Full cycles** (`<tool>-new-edit-reedit`): new → picks → a field → ✓ → double-click → change → ✓ → re-edit → Esc.
  Covered tools: Revolve, Sweep, Loft, Thicken, Fillet, Chamfer, Draft, Shell, Linear / Circular pattern, Mirror,
  Boolean, Split, Transform, Delete part, Move face, Delete face, Modify fillet, Offset surface, Plane, Helix,
  Composite part, Variable, Flange, Hem, Corner break, External thread.
- **Edits of start features:** `sheet-metal-model-edit-reedit`, `fillet-edit-cancel-keeps`.
- **Menus:** `edge-rclick-fillet`, `edge-rclick-chamfer`, `face-rclick-edit-feature`, `edge-rclick-edit-sketch`,
  `face-rclick-show-dependencies`, `face-rclick-delete-feature`, `face-rclick-assign-material`,
  `face-rclick-select-tangent-connected`, `edge-rclick-select-loop`, `face-rclick-select-fillet-faces`,
  `plane-graphics-rclick-menu`, `part-row-rclick-copy-paste`.
- **Panels:** `mass-properties-two-parts`, `mass-properties-face-tab`, `section-view-edit`, `named-views-add`,
  `appearance-editor-hex`, `appearance-editor-rgb`, `view-cube-front-then-iso`, `dialog-enter-commits-new`,
  `parts-row-dblclick-rename-escape`.

After batch 1, 29 of 75 tools have a re-edit; 44 are still only opened and closed.

## Batch 2 (41 scenarios, psui 11-12)

New starts: `pin`, `hole-sketch`, `surfaces`, `curves`, `frame-l`, `sm-box` and `bracket` (tests/ui-parity/starts.json;
each `about` gives the coordinates the picks use).

- **Full cycles:**
  - surfaces: Boundary surface, Fill, Move boundary, Ruled surface, Mutual trim, Replace face;
  - Face blend, Body draft;
  - curves: Intersection, 3D fit spline, Composite, Bridging, Trim, Edit, Projected, Routing, Offset, Isocline,
    Isoparametric;
  - Hole from a sketch point; Rib;
  - sheet metal: Tab, Bend, Jog, Corner, Bend relief, Make joint, Finish sheet metal model;
  - frames: End cap, Gusset, Cut list.
- **Edits:** `frame-edit-reedit`; `mate-connector-new-edit-cancel`.
- **Analysis tools** (Show analysis tools ›): Deviation, Connection, Dihedral, Interference, Zebra, Reflection, Draft,
  Flatten surfaces.

All 41 run on ours without a failed step. Features that end in error on ours, to check against live:
- curves: Bridging, Trim (Sketch 2's line isn't taken as a curve to adjust), Edit, Routing, Offset, Isocline,
  Isoparametric;
- Rib (with Parts filled);
- sheet metal: Bend, Jog, Corner, Make joint;
- Replace face.

After both batches, 61 of 75 tools have a re-edit.

## Still left

- Wrap, Decal, Derived, Form, Flex PCB / Finish flex PCB: they need an image, another document or a library.
- Tag, Frame trim (with a second frame), Sheet metal loft, Constrained surface, Enclose, Query variable's edit.
- Create Drawing of… and Export… from the graphics.
