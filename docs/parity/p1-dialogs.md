# P1 feature dialogs against live Onshape

Agent ui (10-26), landed by psui (10-27).

Every P1 feature dialog compared with live at 1×:
fields, defaults, order, chevrons, tooltips, error states, preview.

Method:
- **Specs.** Live's `/featurespecs` (REST from the page) against ours
  (`noshape_model::features::get`), parameter by parameter: id, label,
  kind, default (metric and imperial), enum options and names, UI hints,
  whether a visibility condition exists, description (the field tooltip).
  The live dump stays outside the repo (`onshape-research/live/p1audit`).
- **Empty dialogs.** Each dialog opened empty in the same Part Studio (a
  40 × 20 × 10 mm block), cropped at the same place in both apps.
- **Title errors.** The red title's hover text with nothing selected.
- **States.** Tabs, dropdown options and checkboxes clicked in live; the
  same click points replayed in ours (the layouts coincide to the pixel),
  both cropped and compared.

Status: ✅ matches · 🔧 differed, fixed · ⏳ open.

Live remembers some values per user (`REMEMBER_PREVIOUS_VALUE`: Fillet
and Chamfer distances, body type, Hole style and standard …), so an empty
live dialog can open on a remembered state (Revolve on Thin, Hole on
Metric / Tapped M6, Fillet 1 mm). The specs carry the same hints, and the
defaults are compared on the specs.

## Specs

| Feature | Fields, order, labels, defaults, enum options, tooltips | Notes |
|---|---|---|
| Sketch | ✅ (empty dialog) | Sketch is a client-side feature (no spec row) |
| Extrude | ✅ | `bodyType`, `startOffset` carry live's sheet-metal (flat) condition; ours shows them always (no flat view yet) |
| Revolve | ✅ | |
| Sweep | ✅ | |
| Loft | ✅ | our `endConditionsGroup` is the "End conditions" group row live draws from `groups` |
| Fillet | ✅ | |
| Chamfer | ✅ | |
| Draft | ✅ | |
| Shell | ✅ | |
| Hole | ⏳ (structure differs: ours a lookup cascade; compared on screen) | |
| Linear pattern | ✅ | |
| Circular pattern | ✅ | |
| Curve pattern | 🔧 | "Path to pattern along" allowed reordering its picks (`ALLOW_QUERY_ORDER`); live doesn't |
| Mirror | ✅ | |
| Boolean | ✅ | |
| Split | ✅ | our `edgeProjectionOptionsGroup` is a group row |
| Transform | ✅ | |
| Plane | ✅ | |

Live's `PREVENT_CREATING_NEW_MATE_CONNECTORS` is our
`NO_MATE_CONNECTOR_BUTTON` (Direction, Plane's Entities: no mate connector
icon in either).

## Empty dialogs and title errors

All 18 dialogs open with the same rows, order, chevrons, checkboxes and
defaults as live (live's remembered values aside), and the red title's
hover text matches word for word ("Extrude 2 did not regenerate properly:
Select face or sketch region to extrude.", … "Plane 1 did not regenerate
properly: Offset plane requires a plane, circle, ellipse, or arc to offset
from."). Revolve's text was compared on Thin in live, see States.

## States

68 states (tabs, end types, dropdown options, checkboxes) clicked in
live and replayed in ours from the same points; side-by-side crops in
`ui-scratch/audit/pairs-*.png`.

| Area | Status | Notes |
|---|---|---|
| Revolve (Solid, Surface, Thin, one direction) | ✅ | |
| Extrude (Surface, Thin, every end type, Direction, Starting offset, Symmetric, Draft, Second end position) | ✅ | |
| Sweep (Surface, Thin, Twist, Scale, each Profile control) | 🔧 | Lock profile direction with nothing picked flagged "Sweep path"; sweep.fs checks the direction first ("Direction to lock" pink), then a thin wall, then the path |
| Loft (Surface, Thin, Guides, Path, Connections, end conditions) | 🔧 | Ticking Guides / Path / Connections makes that field the active one (FeatureScript `FOCUS_ON_VISIBLE`); Profiles, empty, turns pink |
| Fillet (Full round, Measurement, Control, Asymmetric, Partial, Variable, Smooth corners) | ✅ | Vertices / Points on edges: no reorder (`PREVENT_ARRAY_REORDER`) |
| Chamfer (types, Tangent) | ✅ | |
| Draft (Parting line), Shell (Hollow) | ✅ | |
| Hole (Inch / Metric, Simple / Counterbore / Countersink) | ✅ | live's 11.25 mm counterbore was its remembered Metric M6 value |
| Linear / Circular / Curve pattern (Feature, Face, Second direction, Skip instances) | 🔧 | Skip instances: "Instances to skip" is one field taking clicks on the instance points (`ALLOW_ARRAY_FOCUS`, `TOGGLE_POINTS`): 11 px white discs ringed #606a72, hovered #134e96, skipped #a2b9df and the instance gone, the seed a 4 px white dot; items "(1, 0) ×", a CLEAR button ("Clear all items"); no "Add instance" button |
| Mirror, Boolean, Split, Transform, Plane | ✅ | |

✓ (all dialogs): live's `canSubmit` enables it once the dialog is dirty
(an edit, or a regeneration without an error) and no applied value fails
validation; regeneration errors and empty selections don't disable it.
Ours disabled ✓ while a selection field was in error. 🔧

## Open

- ⏳ Long checkbox labels: live's text is wider than ours at the same
  13.33 px, so "Use sketch plane direction" (Split, Face) ends in an
  ellipsis in live and fits in ours. The rows clip at the same place (26 px
  from the dialog's right edge); the difference is the font's advance
  widths.
- ⏳ Dialog x: live dumps show the dialog at x 246 (10-18) and 248 (10-26);
  ours is at 246.
- ✅ Draft `INITIAL_FOCUS_ON_EDIT` (live behaviours "psui (10-26) Draft edit focus"): on edit the faces / parting
  edges field is the active one, the plane / pull direction field a normal white field; a new Draft opens on Neutral
  plane. Ours does the same (`focus_for_edit`).
- ✅ Dialog x: 246 is right; live's own dialog on the shared account had been dragged to x 40.
