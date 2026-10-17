# Onshape CAD: Exploration Report for the "noshape" Clone

Compiled 2026-10-17. Scope: CAD only (sketching, part modeling, assemblies, drawings, documents/versioning, variables/configurations, import/export). Simulation, rendering, PDM/release, admin/enterprise, App Store, CAM, PCB and mobile are out of scope and get one line at most.

## 0. Status, method and source legend

### 0.1 Live-app access: FAILED (sign-up blocked by bot protection)
- OBSERVED: a disposable mail.tm inbox was created (`noshapea196d40e@maxxspace.com`). The Onshape sign-up form at https://www.onshape.com/en/sign-up was filled in with headed Chrome 155 on Xvfb, driven by Playwright over CDP.
- OBSERVED: the sign-up form has three fields: **First name\***, **Last name\*** and **Email Address\***, plus a **NEXT** button. The tagline reads "GET STARTED FREE · NO CREDIT CARD REQUIRED · INSTANT ACCESS". The page also has a cookie banner (Accept All / Reject All / Cookies Settings) and a "Create a student account" link.
- OBSERVED: clicking NEXT returned **"Invalid Submission: Sign up not successful for <email>. This submission has triggered our bot protection mechanism and cannot be processed. If you are not a bot: Do not use a temporary email address. Disable browser plug-ins. Use an up-to-date standard browser."** (screenshot `live-01-signup-blocked-bot-protection.png`).
- As instructed, I made no attempt to get around the bot challenge. I did not try other disposable domains or the student path, which would have meant misrepresenting the user. No Onshape account exists. The credentials file (`/root/onshape-research/credentials.txt`, chmod 600) holds the mail.tm inbox credentials and a status note.
- **As a result, nothing inside the CAD app (Part Studio, Sketch, Assembly, Drawing, version graph) was OBSERVED live.** Every fact below the sign-up section comes from public artifacts and carries a source tag.

### 0.2 Fallback sources (all public, downloaded locally)
| Tag | Source | Notes |
|---|---|---|
| **[DOCS]** | Official Onshape Help, build of 2026-10-09 (`https://cad.onshape.com/help/Content/...`). I enumerated the whole table of contents (521 pages) and downloaded the 394 in-scope pages. | Local copies: `/root/onshape-research/docs-raw/html/` (raw) and `/root/onshape-research/docs-raw/md/` (markdown). |
| **[DOCS-IMG]** | Official screenshots embedded in the help pages (toolbars, menus, panels), copied to `screenshots/docs-*.png`. | I read toolbar order from these images myself. |
| **[FS]** | Onshape FeatureScript standard library **v3095.0 (2026-10-10)**, via the public mirror `github.com/reframe-systems/os-std-mirror`. Every Part Studio feature dialog is generated from the `precondition` blocks in this library, so it is the **authoritative** source for dialog field order, labels, enum options, defaults and conditional visibility. | Parsed digest: `/root/onshape-research/docs-raw/fs-features.md` (86 features, 255 enums). Raw: `docs-raw/fs-std/`. |
| **[BUNDLE]** | The public web-client JS/CSS bundles that the sign-in page serves (`/js/woolsthorpe.*.js`, `/css/onshape-design-tokens.v1.0.53.min.css`). I only read them as text and never executed them. | Extracts: `docs-raw/bundle-command-actions.txt` (243 command/hotkey IDs), `docs-raw/bundle-appearance-constants.txt` (702 graphics constants), `docs-raw/bundle-i18n-strings.txt` (4,667 UI strings). |

**OBSERVED** is used only for the sign-up flow above.

### 0.3 Global layout at a glance [DOCS-IMG `docs-partstudio-annotated.png`, `docs-view-cube-menu.png`, `docs-document-toolbar.png`]
- **Top document bar:** Onshape logo, ☰ Document menu, document name, workspace/branch name ("Main"), link icon. On the right: notifications bell, App Store/integrations icon, Learning center icon, **Share** button, **?** Help dropdown (Help, Explore Onshape, FeatureScript documentation, **Keyboard shortcuts**, What's new, Forums, Report a bug, System check, About), user menu.
- **Feature/tool toolbar:** a single row across the top of the graphics area. Undo and Redo sit at the far left, **Search tools… (Alt/⌥ + C)** at the far right.
- **Left vertical icon rail (panel toggles), top to bottom:** Features list, Insert/Add (mate connectors?), Appearance, Comments, Properties/Notes, Configurations, Versions & history (clock), Search/Find, Bill of materials or list. The icon names are inferred from the images. See section A for the documented list.
- **Feature list panel:** header "Features (N)" with buttons (add folder, pause/freeze regeneration, regeneration timer); a "Filter by name or type" field with a filter funnel; "Default geometry" folder (Origin, Top, Front, Right); features in order; a horizontal **rollback bar**; then a separate **Parts (N)** list with its own scroller.
- **Right edge:** a vertical strip of panel toggles for the right-side panels (Appearances, Configurations, Properties/Custom tables, Variable table "(x)").
- **View cube:** top-right, with axis triad (X red, Y green, Z blue), rotation arrows around the cube and a small ▾ dropdown under it.
- **Bottom:** "+" (add tab/insert), a horizontal **tab bar** (Part Studios, Assemblies, Drawings, folders), and at the far left a search-tabs magnifier. Bottom-right: small mode buttons (section/measure/mass props quick access).
- **Hold the S key** in the graphics area to get a **shortcut toolbar**: a context-specific grid of 10 tools. For a Part Studio these are Sketch, Extrude, Revolve, Sweep, Loft, Fillet, Chamfer, Shell, Boolean, Transform [DOCS-IMG `docs-feature-shortcut-toolbar-S-key.png`].

### 0.4 Toolbar order read directly from official toolbar screenshots [DOCS-IMG]
Use this as the ground-truth visual order. Sections A, B, E and F add the tooltips and shortcuts. "▾" marks a dropdown group whose first item is the visible button. "|" marks a separator.

**Part Studio feature toolbar** (`docs-feature-toolbar.png`):
Undo · Redo | **Sketch** | Extrude · Revolve · Sweep · Loft · [Thicken ▾: Thicken, Enclose] | [Fillet (Shift+F) ▾: Fillet, Face blend] · Chamfer · [Draft ▾: Draft, Body draft] · Rib · Shell · Hole · External thread | [Linear pattern ▾: Linear pattern, Circular pattern, Curve pattern] · Mirror | Boolean · Split · [Transform ▾: Transform, Wrap, Decal] · Delete part | Modify fillet · Delete face · Move face · Replace face | Plane · [Offset surface ▾: Offset surface, Boundary surface, Fill, Move boundary, Ruled surface, Mutual trim, Constrained surface] · [Helix ▾: Helix, 3D fit spline, Projected curve, Bridging curve, Composite curve, Intersection curve, Trim curve, Isocline, Offset curve, Isoparametric curve, Edit curve, Routing curve] · Mate connector · Derived · [Variable ▾: Variable, Query variable] · Composite part · Tag | Frame · Frame trim · Gusset · End cap · Cut list | [Sheet metal model ▾: Sheet metal model, Flange, Hem, Tab, Bend, Form, Sheet metal loft, Make joint, Corner, Bend relief, Corner break, Finish sheet metal model] | Add custom feature (dashed ⊕) | Search tools… (Alt/⌥+C)

**Sketch toolbar** (`docs-sketch-toolbar-part1.png`, `-part2.png`):
Undo · Redo | (sketch-level: Extrude shortcut, Revolve shortcut) | [Line (L) ▾: Line, Midpoint line] · [Corner rectangle (G) ▾: Corner rectangle, Center point rectangle (R), Aligned rectangle] · [Center point circle (C) ▾: Center point circle, 3 point circle, Ellipse] · [3 point arc (A) ▾: 3 point arc, Tangent arc, Center point arc, Elliptical arc, Conic] · [Inscribed polygon ▾: Inscribed polygon, Circumscribed polygon] · [Spline ▾: Spline, Bezier, Spline control point] · Point · Text | [Use (U) ▾: Use, Intersection] · Construction (Q) | [Fillet (Shift+F) ▾: Fillet, Chamfer] · [Trim (M) ▾: Trim, Extend (X), Split] · [Offset (O) ▾: Offset, Slot] · Mirror · [Linear pattern ▾: Linear pattern, Circular pattern, Transform] · [Insert DXF or DWG ▾: Insert DXF or DWG, Insert image] | Dimension (D) | Coincident (I) · Concentric · Parallel (B) · Tangent (T) · Horizontal (H) · Vertical (V) · Perpendicular · Equal (E) · Midpoint · Normal · Pierce · Symmetric · Fix · Curvature | Search tools…

**Assembly toolbar** (`docs-assembly-toolbar.png`):
Undo · Redo | Update (refresh) | **Insert** | Mate connector | Fastened · Revolute · Slider · Planar · Cylindrical · Pin slot · Ball · Parallel · Tangent · Width | Group · Replicate | (instance tools: Replace instance, Create Part Studio in context, …) · Linear pattern · Circular pattern · Mirror | Gear relation · Rack and pinion relation · Screw relation · Linear relation | Display states / Exploded views · BOM | Search tools…

**Drawing toolbar** (`docs-drawing-toolbar.png`):
Undo · Redo | Update | Insert view · Projected view · Auxiliary view · Section view · Aligned section view · Detail view · Broken-out section · Break view · Crop view | [Dimension (D) ▾: Dimension, Chamfer dimension, Maximum or minimum dimension (Ctrl+M), 2 point linear, Point to line, Line to line, Line to line angular, 3 point angular, Arc length, Radial (Shift+R), Diameter (Shift+D), Foreshorten (Shift+H), Ordinate, Hole/thread callout] · Ordinate dimension · Hole/thread callout | Callout (balloon)? · Datum · Geometric tolerance · Surface finish · Weld symbol | Note (A) · Inspection? · Table · BOM table · Revision table · Hole table · Cut list / Custom table | Centerline / Centermark tools (point-to-point centerline, line-to-line centerline, bolt-circle, centermark, …) | Line · [Center point circle (C) ▾: Center point circle, 3 point circle] · Arc · [Corner rectangle (G) ▾: Corner rectangle, Center point rectangle (R)] · [Spline ▾: Spline, Spline point] · Point · Hatch · Trim | [Horizontal (H) ▾: Horizontal, Vertical (V), Coincident (I), Perpendicular (Shift+L), Parallel (B), Tangent (T)] | Insert DXF/DWG · Insert image · Format painter | Inspection/MBD tools. Section F has the documented list; the "?" items above are icons I could not read with confidence.

### 0.5 Visual-state constants from the client [BUNDLE]
Light theme (the default). Colors are resolved from the design tokens.

| Thing | Value |
|---|---|
| Graphics background | `#ffffff` (dark theme `#333333`) |
| Sketch: **under-constrained** geometry | **blue `#0000d0`** (dark theme `#3e94ff`); "show-through" (behind solids) `rgb(64,64,255)` |
| Sketch: **fully constrained** geometry | **black `#000000`** (dark `#ffffff`); show-through `rgb(64,64,64)` |
| Sketch: **over-constrained / conflicting** | **red `#cd0000`** (dark `#ff6b6b`); show-through `rgb(192,64,64)` |
| Inactive (not-being-edited) sketch | grey `#999999` |
| Sketch region (closed profile) fill | `#9aa5af` at 20% alpha; show-through `rgba(154,165,175,0.1)` |
| Construction geometry | dashed, stipple pattern `stipple(4, 6, 30, 6)` |
| Active sketch line width / point size | 2 px / 6 px (user-created points 8 px; spline handles 10 px) |
| Inactive sketch line width / point size | 1 px / 4 px |
| Hover **pre-highlight** | border `rgba(255,198,133,1)` (light orange), transparent fill |
| **Selected** entity highlight | `rgba(255,169,10,0.8)` fill, `rgba(255,169,10,1)` border (orange) |
| Selected part (whole body) | `rgba(255,192,76,1)` |
| Secondary highlight (related/"smart" selection, dialog's other field) | `rgba(242,255,128,0.4)` (pale yellow) |
| Error highlight (failed references) | `rgba(189,48,57,1)` (dark red) |
| Entities removed by a selection filter | `rgba(255,29,5,0.5)` |
| In-context references | magenta `rgba(212,3,249,0.3–0.6)` |
| Comment highlight | `rgba(66,139,202,0.5)` (blue) |
| Highlight line widths | min 5 px; hidden-edge highlight stipple `stipple(10,10,10,10)`, opacity 0.18 |
| Dimension colors | driving dimension text uses "DIMENSION_DRIVING_VALUE_TEXT_COLOR"; driven (reference) dimensions use a faded "DIMENSION_DRIVEN" color; bad/conflicting dimensions use the over-constrained red |
| Pre-highlight timing | pre-highlight is held back while the mouse moves fast (`PREHIGHLIGHT_SPEED_THRESHOLD=.15`); paused-mouse timeout max 250 ms |
| Profile inspector | errors in `rgba(189,48,57,1)`, focus in pale yellow |

Sketch constraint-status icons in the constraint manager [BUNDLE]: `constraint-status-solved` ("Solved"), `constraint-status-overconstrained` ("Errors"), plus a "Driven" state. When a new dimension would over-constrain the sketch, the client sets `setDrivenDimensionIfOverConstrained=true`, so the dimension is created as a **driven (reference) dimension** instead of failing [BUNDLE].

### 0.6 Static (non-remappable) input bindings [BUNDLE]
Rotate = arrow keys · Precision rotate = Ctrl+arrows · Coarse rotate = Shift+arrows · Pan = Ctrl+Shift+arrows · Select through transparent = Alt+Click · Copy annotation = Alt+LMB drag · Accept command = Enter · Accept & repeat command = Shift+Enter · Lock mate inference = hold Shift · Suppress sketch inferencing = hold Shift · Open in new window = Shift+Click · Open in new tab = Ctrl/Cmd+Click · Move rollback bar = select the bar, then Up/Down arrows.

