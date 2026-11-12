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

## Left for batch 2

- **Full cycles for the other 44 tools:** surfaces (Boundary, Fill, Move boundary, Ruled, Mutual trim, Constrained),
  curves (Fit spline, Projected, Bridging, Composite, Intersection, Trim, Isocline, Offset, Isoparametric, Edit,
  Routing), Frame / Frame trim / Gusset / End cap / Cut list / Tag, the rest of sheet metal (Tab, Bend, Jog, Form,
  Sheet metal loft, Make joint, Corner, Bend relief, Finish), Hole (from a sketch point), Rib, Wrap, Decal, Derived,
  Replace face, Face blend, Body draft, Enclose, Query variable, Mate connector.
- **The analysis submenu's eight tools; Create Drawing of… and Export… from the graphics.**
- **New state keys, both recorders:** `camera` and `measure`, so view and Measure scenarios compare what they show.
