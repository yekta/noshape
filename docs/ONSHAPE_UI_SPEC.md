# Onshape UI implementation spec (visual reference for noshape)

Status: complete. Sections: 1 palette, 2 typography, 3 layout, 4 components, 5 viewport glyphs, 6 icon guide and inventory (353 rows), 7 gap list. Reference library index: `/root/onshape-research/ui-ref/index.md`.

## 0. Sources and conventions

All values come from public material. No Onshape account was used.

| Tag | Source | Local copy (outside the repo) |
|---|---|---|
| **[TOK]** | Onshape design tokens, `https://cad.onshape.com/css/onshape-design-tokens.v1.0.53.min.css`, linked from the public sign-in page. These are the `--os-*` CSS variables: 145 primitive colors, 277 light-theme semantic tokens and about 640 component tokens. | `/root/onshape-research/ui-ref/assets/onshape-design-tokens.v1.0.53.min.css` |
| **[CSS]** | Web client stylesheet `https://cad.onshape.com/css/woolsthorpe.1e32446014473a935264.css` (1.4 MB), plus the Angular component styles embedded in `woolsthorpe.*.js`. This is the source for sizes, paddings, radii and states. | `ui-ref/assets/woolsthorpe.1e32446014473a935264.css`; Angular styles extracted to `ui-ref/assets/ng-component-styles.css` |
| **[GFX]** | WebGL appearance constants in the client bundle (viewport colors, line widths, view cube geometry). | `/root/onshape-research/docs-raw/bundle-appearance-constants.txt` |
| **[ICON]** | The web client's SVG icon sprite, `https://cad.onshape.com/images/icons.v1.4.428.min.svg`, with 1,565 symbols. These are the real toolbar, feature-list and panel icons. | `ui-ref/assets/icons.v1.4.428.min.svg`, split into `ui-ref/icons-svg/*.svg`, rendered as 4x PNGs in `ui-ref/icons-sprite/*.png` |
| **[SHOT]** | Official help-center screenshots: 5,148 images from 394 help pages. | `ui-ref/help-images/raw/...`, mapped in `ui-ref/index.md` and `ui-ref/help-images/manifest.json` |
| **[HELP]** | The help page "Colors" (`/help/Content/Home/colors.htm`), which lists the hex values Onshape uses for selection, sketches and alerts. | `docs-raw/md/Home__colors.md` |

Conventions:
- All sizes are CSS pixels at 100% browser zoom, which equal GPUI logical pixels. Most help screenshots were captured at 125% (a feature-list row pitch of 32.5 px corresponds to the 26 px CSS row). When a value comes from a screenshot, it has been divided by the capture scale.
- `pt` sizes in Onshape's CSS convert to px as follows: 9pt = 12 px, 10pt = 13.33 px, 11pt = 14.67 px, 12pt = 16 px. The root `html { font-size: 14px }`.
- Onshape's token names sometimes swap "horizontal" and "vertical". For example, `--os-button-normal-padding-vertical: 12px` is applied as the left/right padding. This spec gives the padding as it actually renders.

---

## 1. Color palette (light theme; Onshape's default)

### 1.1 Core surfaces

| Role | Token | Value | Notes |
|---|---|---|---|
| App / viewport background | `--os-background-primary` | `#ffffff` | The WebGL `BACKGROUND_COLOR` is this token, so the viewport is **plain white with no gradient** [GFX]. Confirmed by sampling `docs-partstudio-annotated.png`. |
| Document top bar (navbar) | `--os-navbar-background` (= `--os-fill-senary`) | `#eaeaea` | Sampled at exactly `#eaeaea` in `docs-document-toolbar.png`. Height 40 px. |
| Navbar item hover | `--os-navbar-item--hover` | `#d7d7d7` | |
| Navbar subtitle ("Main" workspace name) | `--os-navbar-subtitle-text` | `#999999` | |
| Feature toolbar | `--os-toolbar-background` | `#ffffff` | The toolbar sits on the white viewport, with no band and no bottom border. |
| Toolbar group separator | `--os-toolbar-divider` | `#d7d7d7` | 2 px wide. |
| Left panel (feature list) body | `--os-left-panel-body-fill` | `#ffffff` | |
| Left panel chrome / panel-select rail | `--os-left-panel-background` | `#fafafa` | Sampled `#fafafa` on the left icon rail. |
| Panel outline / splitter | `--os-left-panel-outline` = `--os-outline-tertiary` | `#d7d7d7` | 1 px. |
| Inactive panel | `--os-panel-background--inactive` | `#eaeaea` | |
| Panel footer | `--os-panel-footer-background` | `#eeeeee` | |
| Panel outline (strong) | `--os-panel-outline` | `#cccccc` | |
| Right panel-select rail | `--os-vertical-tab-background` | `#fafafa` | Outline `#d7d7d7`, hover `#eaeaea`, selected indicator `#1651b0`. |
| Bottom tab bar background | `--os-tab-bar-background` | `#d7d7d7` | Sampled `#d7d7d7`; it is darker (`#cecece`) right under the tab shadows. |
| Tab, inactive | `--os-tab-bar-tab--inactive` | `#eaeaea` | |
| Tab, active | `--os-tab-bar-tab--active` | `#ffffff` | |
| Tab, active indicator | `--os-tab-bar-selected-indicator` | `#1651b0` | A 2 px bar at the bottom edge of the tab. |
| Tab text active / inactive | `--os-tab-bar-text--active` / `--inactive` | `#333333` / `#666666` | |
| Tab-bar button hover (+, tab manager) | `--os-color-noreaster-300` | `#bababa` | Used by the CSS for `#os-tab-bar .os-tab-bar-controls svg:hover`. The token `--os-tab-bar-button--hover` is `#eaeaea`. |
| Feature dialog body | `--os-feature-dialog-background` | `#fffffff2` (white at 95%) | Translucent over the viewport. |
| Feature dialog header | `--os-feature-dialog-header-background` | `#ffffff` | Bottom border `--os-feature-dialog-divider` `#d7d7d7`. |
| Feature dialog footer | `--os-feature-dialog-footer-fill` | `#eeeeee` | Border `#d7d7d7`. |
| Modal dialog | `--os-modal-background` | `#ffffff` | Border `#d7d7d7`, radius 4. |
| Tooltip | `--os-tooltip-fill` | `#d0dbe5` | A blue-gray fill, not dark. Text `#333333`, weight 500, 12 px. |
| Expanded (rich) tooltip | `--os-tooltip-expanded-fill` | `#ffffff` | |
| Context menu / dropdown | `--os-context-menu-background--idle` | `#ffffff` | Outline `#cccccc`. |
| Context menu hover | `--os-context-menu-background--hover` | `#def1fb` | |
| Dropdown selected item | `--os-dropdown-menu-fill--selected` | `#b2ddf6` | |
| Dropdown menu text | `--os-dropdown-menu-text` | `#555555` | `#333333` when selected. |
| Tool search ("Search tools…") field | `--os-tool-search-button-fill--idle` | `#eeeeee` | Hover `#d7d7d7`, radius 3. Sampled `#eeeeee`. |
| Tool search popup container | `--os-tool-search-container-fill` | `#f5f5f5` | Results: idle `#f5f5f5`, hover `#def1fb`, selected `#b2ddf6`; footer `#eeeeee`; divider `#cccccc`. |
| Mode bar (sketch / in-context banner) | `--os-mode-bar-fill-primary` | `#d0dbe5` | Secondary (warn) `#fab601`, tertiary `#90cef1`. |
| Regeneration flyout | `--os-regeneration-flyout-fill` | `#fff8e6` | |

### 1.2 Text

| Role | Token | Value |
|---|---|---|
| Primary text | `--os-text-primary` | `#333333` |
| Secondary text (labels, inactive tabs, dialog field labels) | `--os-text-secondary` | `#666666` |
| Tertiary / disabled / suppressed | `--os-text-tertiary--static` | `#999999` |
| Quaternary (dropdown items, disabled menu items) | `--os-text-quaternary` | `#555555` |
| Strongest text (input text in FS dropdowns) | `--os-text-quinary` | `#1c1c1c` |
| Emphasis / selected enum / links in titles | `--os-text-emphasis` | `#1651b0` |
| Link | `--os-link` | `#094174` |
| Invalid / error text (feature list, dialog title) | `--os-text-invalid` | `#aa252d` |
| Text on accent fill | `--os-text-accent` | `#ffffff` |
| Rolled-back / not-computed feature rows | `--os-color-noreaster-200` | `#cccccc`, italic |
| Hidden part / feature rows | `--os-color-noreaster-400` | `#aaaaaa` |

### 1.3 Accent, selection and hover (UI)

| Role | Token | Value |
|---|---|---|
| Accent (Onshape blue) | `--os-accent-primary` = `--os-active` | `#1651b0` |
| Accent hover | `--os-emphasis-hover--static` | `#769fcc` (primary-button hover) |
| Accent darker | `--os-color-charles-600` / `-800` | `#134e96` / `#094174` |
| Focus ring | `--os-focus` | `#3e94ff` |
| List row hover (feature list, menus, tables) | `--os-hover-primary` | `#def1fb` |
| List row selected | `--os-select-primary` | `#b2ddf6` |
| List row "related" highlight (inferred) | `--os-tree-item-background--inferred` | `#ecf0f4`, with a 1 px bottom border `#90cef1` |
| Generic button hover (gray) | `--os-hover-secondary` | `#eaeaea` |
| Icon button hover | `--os-icon-button-fill--hover` | `#eaeaea` |
| Icon button hover inside a split group | `--os-icon-button-fill--hover-group` | `#d7d7d7` |
| Icon button selected / toggled on | `--os-icon-button-fill--selected` | `#eaeaea` plus inset shadow `0 1px 1px rgba(0,0,0,.25)` |
| Toolbar tool active (command running) | `--os-select-primary` | `#b2ddf6` plus inset shadow `0 1px 1px rgba(0,0,0,.2)` |
| Toolbar tool hover | `--os-color-constitution-rgba5` | `rgba(0,0,0,0.05)` over white, which is about `#f2f2f2` |
| Highlight (configured parameter, "changed") | `--os-highlight-fill` / `--os-highlight-outline` | `#fcda7f` / `#fab601` |
| Parameter highlight outline (jump-to) | `--os-color-seaport-500` | `#90cef1`, 2 px |
| Overlay (emphasis) | `--os-overlay-emphasis` | `#1651b019` |
| Overlay (disabled) | `--os-overlay-disabled` | `#33333359` |

### 1.4 Status colors

| Role | Fill | Outline | Text | Background | Close icon |
|---|---|---|---|---|---|
| Success | `#009400` (`--os-success`) | `#a2db7d` | `#333333` | `#ddf6cc` | `#028502` |
| Warning | `#fab601` (`--os-warn`) | `#fcda7f` | `#333333` | `#fff8e6` | `#987107` |
| Error | `#bd3039` (`--os-error`) | `#dd979b` | `#333333` (invalid text `#aa252d`) | `#f9ebec` | `#aa252d` |
| Info | `#90cef1` (`--os-info`) | `#b2ddf6` | `#094174` | `#def1fb` | `#467bb6` |

Alerts (toasts at the top of the viewport) use the background, outline and text from the same row, with a 1 px border, radius 4 px, padding 10 px horizontal and 6 px vertical, and a 7 px gap [TOK `--os-alert-*`].

### 1.5 Buttons

| Variant | Idle | Hover | Disabled | Text |
|---|---|---|---|---|
| Primary (e.g. **Share**, OK in modals) | `#1651b0` | `#769fcc` | `#a2b9df` | `#ffffff` |
| Secondary | `#ffffff`, border `#aaaaaa` | `#eaeaea` | `#ffffff`, text `#999999` | `#333333` |
| Cancel (gray) | `#d7d7d7` | `#eeeeee` | `#cccccc`, text `#555555` | `#333333` |
| Success | `#028502` | `#74ce3a` | `#a2db7d` | `#ffffff` |
| Error / destructive | `#bd3039` | `#d66e74` | `#dd979b` | `#ffffff` |
| Feature-dialog accept (green check) | `#009400` | `#74ce3a` | `#a2db7d` (the CSS applies opacity .2 when disabled) | white check glyph |
| Feature-dialog cancel (red X) | transparent | `#eaeaea` | | X glyph `#bd3039` (`--os-icon-accent-cancel`) |
| Pressed (any) | (same fill) | | | inset shadow `0 3px 5px rgba(0,0,0,.12)` (`--osx-pressed`) or `0 1px 1px rgba(0,0,0,.25)` (`--osx-pressed-secondary`) |

Button shadows: primary `#0000001f`, secondary `#0000003f` (`--os-effect-shadow-button-*`).

### 1.6 Inputs, checkboxes, toggles

| Role | Value |
|---|---|
| Form-field fill | `#ffffff`; disabled `#eeeeee` |
| Form-field outline idle / focused / error / disabled | `#aaaaaa` 1 px / `#3e94ff` **2 px** / `#bd3039` / `#aaaaaa` |
| Form-field text / disabled text | `#333333` / `#666666` |
| Text box (in-panel, e.g. rename) outline idle / edit / error | `#cccccc` / `#1651b0` / `#bd3039` |
| Search bar outline idle / focused | `#aaaaaa` / `#3e94ff`; filter variant idle `#cccccc` |
| Search bar text active / placeholder | `#333333` / `#666666` (placeholder is italic in the feature filter) |
| **Feature-dialog value field** (underline style) | No box. 1 px bottom border `#d7d7d7`; hover `#1651b0`; focused `#1651b0` 1 px border plus 1 px inset shadow (2 px total); invalid fill `#f9ebec` with a `#bd3039` underline. |
| Feature-dialog field label | `#666666`, `#333333` on hover |
| Selection box (query list, QLV) | border 1 px `#d7d7d7`, radius 2, text `#666666`; **focused**: fill `#def1fb`, border `#1651b0`, text `#555555`; error: fill `#f9ebec`, border `#bd3039`; missing-reference text `#aa252d` |
| Selection row zebra | every second row `#ffffff59` |
| Checkbox | 12×12, radius 3, border 1 px `#888888` (hover `#333333`), fill `#ffffff`; checked: fill `#1651b0` with a white check; focus: border `#3e94ff` plus 1 px ring; disabled fill `#999999` |
| Checkbox label | `#666666`; hover `#333333` |
| Toggle switch | body active `#1651b0`, inactive `#eaeaea`; handle `#ffffff`; outline `#999999` |
| Radio | active `#1651b0`, inactive `#333333` |
| Enum underline-tabs (e.g. Solid / Surface / Thin) | text `#666666`; selected **weight 600** with a 2 px underline `#4b5761` (`--os-color-revolution`); idle underline 1 px `#f5f5f5` |
| Dropdown (enum select) | border `#d7d7d7` underline; hover `#def1fb`; selected item fill `#1651b0` with white text |
| Slider | bar `#777777` (disabled `#d7d7d7`); handle `#ffffff` with a `#888888` outline; dragging `#d7d7d7` |
| Collapsible section header (accordion) | caret active `#666666`, inactive `#d7d7d7`, error `#bd3039`; leader line `#cccccc` |

### 1.7 Tables, scrollbars, misc

| Role | Value |
|---|---|
| Table header idle / hover | `#eaeaea` / `#d7d7d7` |
| Table cell idle / hover | `#ffffff` / `#def1fb` |
| Read-only cell idle / hover | `#f5f5f5` / `#eeeeee` |
| Table border idle / focused | `#aaaaaa` 1 px / `#1651b0` 2 px |
| Scrollbars | The client uses the native browser scrollbar. Sampled in Chrome: track `#f1f1f1`, thumb `#c1c1c1`, about 15 px wide (12 px at 100% zoom in the parts list). In noshape, use a thin overlay scrollbar with thumb `#c1c1c1` (hover `#a8a8a8`) and track `#f1f1f1`. |
| Drop shadows | primary `0 5px 10px rgba(0,0,0,.20)` (dialogs, menus, dropdowns); secondary `0 1px 2px rgba(0,0,0,.20)` (tooltips); tertiary `0 3px 6px rgba(0,0,0,.20)` |
| Spinner | ring `#d7d7d7`, indicator `#1651b0` |
| Version graph branches 0..8 | `#1651b0`, `#fab601`, `#3ad4a7`, `#b689be`, `#3e94ff`, `#b88a00`, `#219773`, `#672f8f`, `#e98e72` (faded variants: `#d0dbe5`, `#fff8e6`, `#c3f2e4`, `#e9dbeb`, `#def1fb`, `#e9dbb2`, `#c3e2d9`, `#d1c0dd`, `#f8ddd4`) |
| Version graph node idle / hover / selected / referenced | `#fafafa` / `#def1fb` / `#b2ddf6` / `#d0dbe5` |
| Collaborator colors | `#3ad4a7`, `#B689BE`, `#008BF5`, `#B88A00`, `#219773` |

### 1.8 Viewport (3D graphics)

| Element | Value | Source |
|---|---|---|
| Background | `#ffffff`, flat | [GFX] `BACKGROUND_COLOR` |
| Default part colors | Parts get colors from an **8-color cycle** that repeats from the 9th part [HELP]. The lit-face colors sampled from the official swatch image (`help-images/raw/Images/concepts/Capture16.png`) are, in order: (1) light steel blue, lit about `#8bb6d0`, shadow `#4e6776`; (2) mid gray, lit about `#9d9d9d`, shadow `#525252`; (3) blue, lit about `#5071b8`, shadow `#1d305a`; (4) white, lit `#fcfcfc`-`#f3f3f3`, shadow `#757575`; (5) pale cyan, lit about `#e5feff`, shadow `#627179`; (6) orange, lit about `#ffa330`, shadow `#7c4300`; (7) dark gray, lit about `#969696`, shadow `#3f3f3f`; (8) gold, lit about `#f1b61a`, shadow `#7d5b00`. The base material colors are sent by the server and are not in the public bundle, so these values come from rendered pixels. For a single default color, use part 1 (base about `#a3c4dc` before lighting). Many help screenshots show parts in color 4 (near-white). | [HELP], [SHOT] |
| Part edges (shaded-with-edges) | `#000000` (`LINE_POINT_COLOR`), 1 px | [GFX] |
| Edges in show-through / hidden lines | `rgb(64,64,64)` | [GFX] `SHOW_THROUGH_LINE_POINT_COLOR` |
| Tangent (smooth) edges | `rgb(35,35,35)`, 0.5 px, stipple 30-4-8-4 | [GFX] |
| Transparent-part edge factor | 0.4 | [TOK] |
| Unshaded (hidden-line / wireframe) edges | `#000000` | [TOK] |
| Mesh facet edges | `rgba(70,70,70,1)`, 0.3 px | [GFX] |
| Origin | `#000000`; size 10 px (point font 3) | [GFX] |
| Point bodies | `#000000`, 8 px | [GFX] |
| Construction (default) planes | fill `#cedbe5` at **opacity 0.2**; outline `#1d60aa` at 35% (`#1d60aa59`), **2 px**; label `#1651b0`, 20 px Flama, top-left inside the plane; default plane aspect 16:9 (1.77777), plane height = 0.7 × viewport height | [GFX], [TOK] |
| Sketch plane (active sketch) | line and text `#90cef1` | [TOK] |
| Axes / triad | X `#cd0000`, Y `#009400`, Z `#0000d0` | [TOK] |
| Pre-highlight (hover) | border `rgba(255,198,133,1)` = `#ffc685`; interior has no fill (alpha 0). The help page names sketch-edge hover as `#f3bf8e`. | [GFX], [HELP] |
| Selection | interior `rgba(255,169,10,0.8)`, border `#ffa90a`. The help page names it "vivid yellow `#ffc107`". Selected parts use `PART_HIGHLIGHT_COLOR` `rgb(255,192,76)`. | [GFX], [HELP] |
| Secondary highlight (related entities, smart selection) | `rgba(242,255,128,0.4)`, border `rgba(242,255,128,0.8)` | [GFX] |
| Feature highlight (hovering a feature in the list) | `rgba(255,169,10,0.8)` | [GFX] |
| Error highlight | `rgb(189,48,57)` = `#bd3039` | [GFX] |
| In-context references | `rgba(212,3,249,0.3)` (magenta) | [GFX] |
| Comment highlight | `rgba(66,139,202,0.5)` | [GFX] |
| Box select, left-to-right (window) | outline `#789ad1`, fill `#d0dcef`; the token `--os-box-select-color` is `#1651b0` | [HELP] |
| Box select, right-to-left (crossing) | outline `#ffdc74`, fill `#fff3cf` | [HELP] |
| Interior highlight overlay opacity | default .5, preview .25 | [GFX] |
| Mate connector | fill `#ffffffcc`, stroke `#002847` | [TOK] |
| Manipulators (drag arrows) | line `#ffffff`, outline `#606a72`; hover line `#a2b9df` / outline `#134e96`; drag line `#134e96` | [TOK] |

### 1.9 Sketch colors

| Element | Value | Source |
|---|---|---|
| Under-defined geometry (active sketch) | `#0000d0` (`--os-sketch-underconstrained-line-point-color`). The help page names it "pure blue `#0000ff`", but the token is `#0000d0`. | [TOK] |
| Under-defined, occluded (show-through) | `#a2b9df` (token); GFX fallback `rgb(64,64,255)` | [TOK] |
| Fully defined | `#000000` | [TOK] |
| Over-defined / error | `#cd0000` (token `--os-sketch-overconstrained-line-point-color`). The help page names it `#bd3039`. | [TOK] |
| Over-defined, occluded | `#dd979b` | [TOK] |
| Inactive sketch | `#999999` (token); the help page names it `#949ba0` | [TOK] |
| Closed-region fill | `#9aa5af` at 20% (`#9aa5af33`), which renders about `#ebedef` on white; the help page names it `#E3E6E9`. Show-through fill is at 10%. | [TOK] |
| Construction geometry | same color as its constraint state, dashed with stipple (4, 6, 30, 6): long dash 30, gap 6 | [GFX] |
| Sketch preview (rubber-band) | `rgba(68,156,205,1)` = `#449ccd` | [GFX] |
| Active sketch line width / point size | 2 px / 6 px (user points 8 px, spline handles 10 px) | [GFX] |
| Inactive sketch line width / point size | 1 px / 4 px | [GFX] |
| Drag-trim path | `#555555` | [TOK] |
| Inference snap | point box 8 px; snap lines 1 px | [GFX] |
| External (used / projected) entities | `#672f8f` (`--os-accent-nonary`) | [TOK], noshape convention |

**Dimensions and constraints**

| Element | Value |
|---|---|
| Dimension line, arrow and text | `#000000`, line 1 px (2 px when selected) |
| Dimension value background | none at rest; hover `#bf9460` at **60% opacity**, radius 4, padding 5; selected `#bf6900` at 60%. The help page names hover `#d9bfa0` and selected `#d9a566`, which are the same colors after blending. |
| Selected dimension line | `#6a4f06` |
| Hovered driving-dimension line | `#987107` |
| Driven (reference) dimension | `#666666` |
| Occluded / inactive dimension | `#666666` |
| Expression-edit box | white fill, 2 px border `#90cef1`, radius 4, padding 6; with variables, fill `#def1fb` (hover `#b2ddf6`, selected `#90cef1`); bad expression `#dd979b` |
| Constraint glyph box (OK) | fill `#ffffff`; hover `#fcda7f`; selected `#fab601` |
| Constraint glyph box (OK, external) | `#def1fb` / `#b2ddf6` / `#90cef1` |
| Constraint glyph box (bad) | `#d66e74` / `#aa252d` / `#bd3039` |
| Constraint leader line | `#9aa5af` |

### 1.10 View cube

| Element | Value |
|---|---|
| On-screen size | 150 px square (`VIEW_CUBE_SCREEN_SIZE_PX`); margins 15 px from the top and right |
| Front face fill | `#ffffff` |
| Back / side shading and rotate arrows | `#cedbe5` at 60% (`#cedbe599`) |
| Face hover | `#90cef1` at 60% |
| Face labels | `#000000`, 12 px Flama (Front, Top, Right, ...); face text is drawn rotated onto the face plane |
| Triad | X red `#cd0000`, Y green `#009400`, Z blue `#0000d0`, 1 px lines, 12 px labels; hidden axes stippled at 30% opacity |
| Geometry ratios | front size .75, back .97, corner .2, arrow width .10, arrow height .13 |
| Dropdown caret (view menu) | a small `▾` button under the cube at its bottom-right, next to the "cube" view-mode icon (`docs-partstudio-annotated.png`) |

### 1.11 Icon colors (for icon artists; section 6 has the full guide)

Icons use these token fallbacks inside the SVGs:
`--os-icon-outline-primary` `#333333` (outlines; 735 uses), `--os-icon-fill-primary` `#ffffff` (lit faces), `--os-icon-fill-secondary` `#999999` (shaded faces / the result of the operation), `--os-icon-fill-tertiary` `#666666`, `--os-icon-fill-quaternary` `#d7d7d7`, `--os-icon-fill-quinary` `#eeeeee`, `--os-icon-accent-primary` `#1651b0` (blue accent: sketch, direction, "new" geometry), `--os-icon-accent-secondary` `#90cef1`, `--os-icon-accent-success` `#009400`, `--os-icon-accent-cancel` `#bd3039`, `--os-icon-accent-warn` `#fab601`, `--os-icon-outline-tertiary` `#888888`.

---

## 2. Typography

### 2.1 Font family

- Onshape uses **Flama** (Feliciano Type Foundry) throughout the UI: `body { font-family: Flama, Open Sans, Helvetica Neue, Helvetica, Arial, sans-serif }` [CSS]. The weights served are 200 Thin, 300 Light, 400 Basic, 500 Medium, 600 Semibold and 700 Bold, from `/fonts/Flama-*.woff2`. The viewport also renders Flama: `VIEW_CUBE_TEXT_FONT="Flama"` and `PLANE_TEXT_FONT="Flama"`.
- **Flama is a commercial font and must not be redistributed.** Use **Roboto**, which noshape already bundles (Regular, Medium, Bold, Italic). Roboto is the closest freely licensed match: a DIN-influenced grotesque with a single-storey `g`, a similar x-height, similar widths and the same "technical" tone. Bundle **Roboto Medium (500)** and add **Roboto SemiBold (600)** (Roboto Flex / Roboto v3 has a 600 weight), because Onshape uses 600 for the document name, edited feature rows and selected enum tabs. If 600 is unavailable, use Bold (700). Do not fake 600 with 500.
- Antialiasing: grayscale (`-webkit-font-smoothing: antialiased`).
- Monospace (FeatureScript editor): `Monaco, Menlo, Ubuntu Mono, Consolas, source-code-pro, monospace`, 12 px. In noshape, use the system monospace.

### 2.2 Type scale (px at 100% zoom)

| UI element | Size | Weight | Color | Line height / notes |
|---|---|---|---|---|
| Base / root | 14 | 400 | `#333333` | `html { font-size: 14px }` |
| Document name (navbar) | 22 | 600 | `#333333` | padding 2 px 4 px |
| Workspace / version name after the doc name ("Main") | 16 | 400 | `#999999` | margin-left 4 px |
| Navbar menu / buttons (Share) | 14 | 500 | white on `#1651b0` for Share | |
| Toolbar text button ("Sketch" label next to its icon) | 14 | 400 | `#333333` | `.os-tool-command-name` padding 5 px 8 px, 0 px on the left after the icon |
| Toolbar dropdown menu items | 14 | 400 | `#333333` | 30 px rows (icon cell 30 px) |
| Tool search placeholder ("Search tools…") | 14 | 400 | `#333333` | The shortcut hint chips (`alt/⌥` `c`) are about 11 px, with a 1 px `#aaaaaa` border, radius 2 |
| Feature list / parts list / assembly tree rows | **13.33 (10pt)** | 400 | `#333333` | 26 px rows |
| Feature list header "Features (11)" | 14 | 600 | `#333333` | Renders semibold in screenshots; no specific CSS rule was found |
| "Parts (97)" heading | 13.33 | 400 | `#333333` | Same row style as the feature-list rows, with a caret |
| Feature row being edited | 13.33 | **600** | `#333333` | row filled `#b2ddf6` |
| Rolled-back / not-computed feature | 13.33 | 400 italic | `#cccccc` | |
| Suppressed feature | 13.33 | 400, line-through | `#999999` | icon at 50% |
| Error feature | 13.33 | 400 (600 in the constraint list) | `#aa252d` | |
| Filter box placeholder "Filter by name or type" | 13.33 | 400 **italic** | `#666666` | |
| Feature dialog title | **13** | **500** | `#333333`; `#aa252d` on error | line-height 28 px, padding-left 6 px |
| Feature dialog body (base) | 12 (9pt) | 400 | | line-height 1.8em |
| Dialog field label ("Depth", "Faces and sketch regions…") | 13.33 (10pt) | 400 | `#666666` | padding 7 px 5 px 0 |
| Dialog value input | 13.33 | 400 | `#333333` | right-aligned for numbers; padding 6 px 3 px 2 px |
| Selection box placeholder label | 12 (9pt) | 400 | `#666666` | shrinks to 85% scale (about 10 px) at top-left once items are added |
| Selection box items (chips / rows) | 13.33 | 400 (active item 700) | `#666666`, `#555555` when focused | 24 px rows |
| Enum underline tabs (Solid / Surface / Thin) | 13.33 | 400; selected 600 | `#666666` | padding 4 px 2 px |
| Icon-label tabs (New / Add / Remove / Intersect) | 12 (9pt) | 400; active 500 | `#666666` | 14 px icon plus 2 px gap |
| Checkbox label | 13.33 | 400 | `#666666`, `#333333` on hover | line-height 1.3, padding 5 px 0 5 px 4 px |
| Collapsible section header | 13.33 | 400 | `#666666` | padding 5 px 4 px |
| Context menu | **12** | 400 | `#333333` | line-height 18 px; disabled `#555555` |
| Tooltip | **12 (9pt)** | **500** | `#333333` on `#d0dbe5` | padding 4 px top, 3 px bottom, 7 px horizontal (0.5rem), radius 0, shadow `0 1px 2px rgba(0,0,0,.2)`, max-width 200 px |
| Bottom tab label | 14 | 400 | `#333333` active / `#666666` inactive | Folder tabs show the name in UPPERCASE in screenshots ("PART STUDIOS", "ASSEMBLIES"), because those are user-named folders. Element tabs use the user's casing. |
| Modal dialog title | 16 (12pt) | 500 | `#333333` | The same 12pt/500 is used for table titles |
| Help menu / dropdown items (navbar) | 14 | 400 | `#555555` | about 27 px rows (`docs-document-toolbar.png`) |
| Version graph / history entries | 13.33 | 400 | `#333333` | |
| Drawing / sketch dimension text (viewport) | 12 px screen-space by default (`DEFAULT_POINT_FONT` size) | 400 | `#000000` | |
| View cube labels | 12 | 400 | `#000000` | |
| Plane labels in the viewport | 20 | 400 | `#1651b0` | |

---

## 3. Layout and measurements (100% zoom)

Reference frame: `ui-ref/part-studio/docs-partstudio-annotated.png`, captured at 125%. Every value below is in CSS px, cross-checked against the client CSS.

### 3.0 Window map (Part Studio)

```
┌──────────────────────────────────────────────────────────────────────────────────────────────┐
│ NAVBAR 40px  #eaeaea  [logo][☰][Document name 22/600][Main 16 #999][🔗]  ...  [🔔][▦][⚙][Share][?▾][user] │
├────┬─────────────────────────────────────────────────────────────────────────────────────────┤
│RAIL│ TOOLBAR 36px, white, on top of the viewport: [↶][↷] | [✎ Sketch] | [tools…] | … [Search tools… alt/⌥ c] │
│40px├───────────────┬─────────────────────────────────────────────────────────────┬──────┬────┤
│    │ FEATURES PANEL│                                                       [view cube 150px]│RAIL│
│#fa │ ~205px (resiz.)│               VIEWPORT  #ffffff                             │      │32px│
│fafa│ Features (11)…│                                                             │      │(rt)│
│    │ [⊽][Filter…]  │                                                             │      │    │
│    │ ▾ Default geo.│                                                             │      │    │
│    │   Origin…     │                                                             │      │    │
│    │ ████ rollback │                                                             │      │    │
│    │───────────────│                                                             │      │    │
│    │ ▾ Parts (97)  │                                                   [⌖][◠][⚖] (bottom-right) │
├────┴───────────────┴─────────────────────────────────────────────────────────────┴──────┴────┤
│ TAB BAR 30px #d7d7d7  [🔍][+] [tab][tab][ACTIVE TAB (white, 2px blue underline)][tab]…          │
└──────────────────────────────────────────────────────────────────────────────────────────────┘
```

- The **left document-panel rail** (40 px wide) spans from below the navbar to the tab bar. The tab bar's first 40 px cell holds the tab-search magnifier, so the rail column continues through the tab bar.
- The **toolbar** starts right of the rail and spans the full width above the feature panel and viewport. The feature panel and viewport both start below it (y = 40 + 36).
- There is no status bar. Transient messages appear as alerts at the top-center of the viewport.

### 3.1 Navbar (document top bar)

| Item | Spec |
|---|---|
| Height | **40 px** (`nav.navbar { min-height: 40px }`; items `height: 40px; line-height: 40px; padding: 0 6px`) |
| Background | `#eaeaea`, with no bottom border (the white toolbar below provides the edge) |
| Logo | The Onshape wordmark, 14 px from the left (`.navbar-logo-documents { padding-left:14px; padding-right:5px }`). noshape uses its own mark here. |
| ☰ document menu | `hamburger-button` icon, 20 px, `#333`, with a 6 px pad; hover `#d7d7d7` fills the full 40 px height |
| Document name | 22 px / 600 / `#333`, padding 2 px 4 px, ellipsis. Click to rename: the inline input is the same size. |
| Workspace / version name | 16 px / 400 / `#999`, margin-left 4 px, max-width 80 px |
| Link icon | `link-icon`, 16-20 px, `#333` |
| Right cluster | notifications bell, App Store (`navbar-app-store`), Learning center (`navbar-learning-center`), **Share** (primary button: `#1651b0` fill, white 14/500 text, radius 4, about 30×52 px, margin 2.5 px 4 px), `?`▾ help, then the user avatar with name. In noshape, keep only **Help ▾** and a **settings/menu** button and drop the cloud items (Share, notifications, App Store, user). |
| Help dropdown | white, 1 px `#ccc` border, radius 4, shadow `0 5px 10px rgba(0,0,0,.2)`; items 14 px `#555`, about 27 px rows, padding about 18 px left; separators 1 px `#d7d7d7` |

### 3.2 Feature toolbar

| Item | Spec |
|---|---|
| Height | **36 px** (screenshot: 45 px at 125%). Background white with no border; the feature-panel top border (1 px `#d7d7d7`) sits right below it. |
| Tool button | `.os-tool-button`: icon cell **30×30 px** holding a **20×20 px** SVG; radius 3 px. Horizontal pitch between plain tools is **34 px** (2 px margin each side). |
| Text tool ("Sketch") | 20 px pencil icon, then the label at 14 px / 400 `#333` with padding 5 px 8 px and 0 px left padding after the icon. The button is about 80 px wide. |
| Split (dropdown) tool | a 30 px icon cell plus a **15 px caret cell** (`.os-caret { width:15px; font-size:10px }`). The caret is the `dropdown-arrow` glyph, a small chevron about 8×5 px, drawn `#333` in screenshots. The icon part and the caret part hover separately: hovering one part fills it `rgba(0,0,0,.05)` and the other part `#eaeaea`/`#d7d7d7`. When open, the caret is filled `#b2ddf6` with inset shadow `0 1px 1px rgba(0,0,0,.2)`. |
| Hover | background `rgba(0,0,0,0.05)` (`--os-color-constitution-rgba5`); some toolsets use `#eaeaea` |
| Active (tool's dialog is open) | background `#b2ddf6`, inset shadow `0 1px 1px rgba(0,0,0,.2)` |
| Disabled | opacity 0.4 (inspection toolbar) / 0.2 (custom toolbar) |
| Group separator | **2 px wide, `#d7d7d7`**, inset 5 px top and bottom, so about 26 px tall, centered. Measured 2 px × 27 px at 1:1. |
| Order | Undo · Redo │ Sketch │ … (`research/sections/A-partstudio-ui.md`; `ui-ref/toolbars/docs-feature-toolbar.png`) |
| Dropdown menu | below the caret (`top:100%`); white; border 1 px `#ccc`; radius 3; shadow `0 5px 10px rgba(0,0,0,.2)`; **min-width 180 px**; rows are 30 px (30 px icon cell + label 14 px `#333`, padding 5 px 8 px); a shortcut chip floats at the right (`shift` `f`); hover `#def1fb` |
| Overflow | When the window is too narrow, trailing tools collapse into a `»`/overflow dropdown at the end of the toolbar. |
| **Search tools…** | At the far right, 10 px after the last tool. Fill `#eeeeee` (hover `#d7d7d7`, pressed inset `0 3px 5px rgba(0,0,0,.12)`); radius 3; padding 5 px 10 px; about **32 px tall × 160 px wide**; text "Search tools…" 14 px `#333`; then the shortcut chips `alt/⌥` and `c` (about 11 px text, 1 px `#aaa` border, radius 2, white fill, 2 px gap). Clicking opens a popover below it: fill `#f5f5f5`, shadow `0 5px 10px rgba(0,0,0,.25)`, a white search field, a result list 200 px wide (rows hover `#def1fb`, selected `#b2ddf6`) and an info pane to the right (border-left 1 px `#ccc`, 400 px wide). |
| Custom feature button | dashed-outline ⊕ (`add-toolbar-feature` icon) placed just before Search tools |

### 3.3 Left document-panel rail

| Item | Spec |
|---|---|
| Width | **40 px** including a 1 px right border `#d7d7d7` (`.os-panel-select-bar { width:40px; border-right:1px solid; border-top:1px solid }`) |
| Background | `#fafafa` |
| Buttons | full width × **31 px**, padding 5 px, icon 20 px `#333`; 4 px gap between buttons (35 px pitch, measured 44 px at 125%) |
| Hover | `#eaeaea` |
| Active | icon tinted `#1651b0` and a **4 px `#1651b0` bar on the left edge** (`:after { border-left:4px solid }`) |
| Icons, top to bottom (Part Studio) | Versions & history (`versions-history-panel`), Create version (`create-version-button`), Release drafts (`create-release-button`), Comments (`comments-panel`), Document notes (`notes-panel`), Where used (`where-used`), Performance (`performance-panel`), Explore/AI advisor (`onshape-ai-advisor-button`); then a 1 px `#d7d7d7` separator (`.os-panel-select-global-buttons { border-top }`) and Action items (`action-items`). In noshape, keep Versions & history, Create version, Comments/Notes (local notes), Where used and Performance, and drop the cloud-only items. |
| Bottom | The tab-search button (`tab-search-button`) sits in the tab bar's left cell under the rail. |

### 3.4 Feature list panel

| Item | Spec |
|---|---|
| Width | about **205 px** default (256 px at 125%); resizable with a splitter on the right edge (1 px `#d7d7d7`). In the panel screenshot it collapses to 0 with a small tab (`treelist-button` icon, 32×32, white, 1 px `#d7d7d7` border, attached to the viewport's left edge at mid-height). |
| Background | `#ffffff` |
| Header row | about 30 px. "**Features (11)**" 14 px / 600 `#333` at the left, 4 px padding. At the right (padding-right 10 px): **Add folder** (`new-folder-button`), **Pause regeneration** (`pause-regen-button`, 2 bars), **Regeneration timer** (stopwatch, `performance-panel`). Each is 20 px in a button with padding 0 5 px. A toggled button is filled `#eaeaea` with inset `0 1px 1px rgba(0,0,0,.25)`. |
| Filter row | about 32 px: funnel icon (`filter-results-button`, 20 px) at the left, then the search field: white, **1 px `#aaa` border** (`#3e94ff` when focused), radius 2-3, height **28 px**, padding 4 px 22 px 4 px 5 px, placeholder "*Filter by name or type*" (italic, `#666`, 13.33 px). The field fills the remaining width with a 5 px right margin. |
| Tree rows | **26 px** row (`.os-list-item-inner { height: 26px }`); 13.33 px text. Each row: 5 px left margin, then a **20 px icon cell** (the SVG is drawn at **17×17 px, opacity 0.8**, offset 2 px/1 px), 3 px gap, then the name (ellipsis, padding-right 3 px). Hover and selection fill an inset rect (1 px top and bottom inset): hover `#def1fb`, selected `#b2ddf6`. The eye toggle (show/hide) appears at the right edge on hover only (`.os-list-item-columns`, 20 px icon columns, 10 px right margin). |
| Folder rows ("Default geometry") | a caret (`expanded` ▾ / `collapsed` ▸, about 12 px, `#333`) in a 16 px column, then the label, with no icon. Children are indented **22 px**, and a thin **vertical guide line** (1 px `#d7d7d7`) runs down from the caret through the children. |
| Default geometry children | "Origin" (`origin-button`: small circle with a dot) and "Top", "Front", "Right" (`default-plane`: a plane parallelogram). All are text `#999`-ish, because default geometry renders like hidden items (`#aaa`) until shown. |
| Feature rows | the feature type icon (e.g. `extrude-button`) and the name. **Edited** (dialog open): weight 600, row `#b2ddf6`. **Rolled back** (below the bar): italic `#ccc`. **Suppressed**: line-through `#999`, icon at 50%. **Error**: text `#aa252d` with an error badge (`list-error-overlay`, 12×12 at icon offset 10, 9). **Under-defined sketch**: a badge overlay (`list-underdefined-overlay`). |
| Rollback bar | a **7 px tall** horizontal bar, margins 5 px left and right, fill `#cccccc` with `filter: brightness(.9)` (`#b8b8b8`; sampled `#c2c2c2`), inside a row of about 14 px. Hovering it turns it `#def1fb`; when selected or dragged it is `#b2ddf6`. It is dragged vertically between rows. |
| Panel split | The feature list and parts list share the panel, separated by a 1 px `#d7d7d7` line and a vertical splitter. Each scrolls independently with the browser scrollbar style (§1.7). |
| Parts list header | "▾ Parts (97)" uses the same 26 px row style, 13.33 px / 400 with a caret. |
| Part rows | indented 22 px under the header; icon `default-part` (a solid block outline, 17 px); 26 px rows. Composite parts use `default-composite-part-button`; surfaces `default-surface`; curves `default-curve`; meshes `default-mesh`. |
| Assembly instance list | the same row metrics (`#assembly-tree` 10pt, 26 px rows), with mate-feature icons such as `fastened`, `revolute` in the "Mates" folder |

### 3.5 Bottom tab bar

| Item | Spec |
|---|---|
| Height | **30 px** (`#os-tab-bar { height:30px }`) |
| Background | `#d7d7d7` |
| Left controls | 40 px tab-search cell (magnifier icon `tab-search-button`, `#555`), then the **+** button (`plus-button`, 20 px in a 34×30 px cell, padding 5 px 7 px, color `#555`, hover `#bababa`). The CSS places the tab-manager button (`tab-manager`) in the same control group. |
| Tab | `flex: 1 0 auto`, **min-width 60 px, max-width 200 px**; height **27 px** (active **28 px**); margin-right 2 px; padding 3 px 10 px 0; **border-radius 0 0 5 px 5 px** (tabs hang from the top edge, with rounded bottom corners); shadow `0 1px 2px rgba(0,0,0,.12)` |
| Inactive tab | fill `#eaeaea`, text `#666` 14 px |
| Active tab | fill `#ffffff`, text `#333`, and a **2 px `#1651b0` bar along the bottom** (`:after { height:2px; bottom:0 }`) |
| Tab content | element icon **20×20** (margin-right 3 px), then the name (ellipsis). Icons: Part Studio `partstudio`, Assembly `default-assembly`, Drawing `drawing`, Variable Studio `variable-studio-element`, Feature Studio `feature-studio-element`, folder `folder`. |
| Folder breadcrumbs | When you are inside a tab folder, the breadcrumb chevrons sit left of the tabs: arrow-shaped (8 px point) chips in `#eaeaea`, with the last one white. |
| Drag | The dragged tab gets shadow `0 5px 10px rgba(0,0,0,.2)`. A drop target folder is filled `#b2ddf6`. |
| Context menu | the standard context menu (§4.6), opening upward from the tab (`ui-ref/context-menus/docs-partstudio-context-menu.png`) |

### 3.6 Right side panel rail (Part Studio)

| Item | Spec |
|---|---|
| Position | a vertical strip of square buttons flush with the right window edge, vertically **centered** on the viewport (not at the top). In the 125% screenshot it spans y≈380-545. |
| Buttons | about **32×32 px** each (40 px at 125%), stacked with 1 px `#d7d7d7` separators; white fill; 1 px `#d7d7d7` border on the left, top and bottom; icon 20-24 px. Hover `#eaeaea`. Active: `#1651b0` indicator bar (`--os-vertical-tab-selected-indicator`) and the panel slides open to the left. |
| Part Studio buttons, top to bottom | **Appearance** (`appearances-button`), **Configurations** (`configuration-table-button`), **Custom tables** (`featurescript-table-button`), **Variable table (x)** (`variable-table-button`). Sheet-metal Part Studios add **Sheet metal table / flat view** (`sheet-metal-table-button`) when a sheet-metal model is active. |
| Assembly buttons | BOM (`bom-table-button`), Configurations, Exploded views (`explode-button`), Named positions (`named-position-button`), Display states (`display-states-button`), Variable table |
| Drawing | Sheets (left), Styles (`style-panel`), Properties |
| Opened right panel | about 300-350 px wide, white, 1 px `#d7d7d7` left border, header 14 px / 500 with a close ×, between the toolbar and the tab bar |

### 3.7 View cube

| Item | Spec |
|---|---|
| Placement | top-right of the viewport, **15 px** from the top and right edges (`VIEW_CUBE_TOP/RIGHT_MARGIN_PX = 15`); the widget is **150 × 150 px**; bottom margin 40 px |
| Cube | an isometric cube whose front-facing faces are white (`#ffffff`) with large **rounded/chamfered** edges and corners, each a separate clickable hotspot drawn in `#cedbe5` at 60%. The face labels "Front", "Top", "Right" (12 px Flama, black) are drawn in perspective on the faces. |
| Arrows | four triangular nudge arrows (up, down, left, right) about 12 px, outside the cube on each side, plus two **curved rotate arrows** above the cube (counter-clockwise left, clockwise right), all `#cedbe5` at 60%; hover `#90cef1` at 60% |
| Triad | from the cube's back-bottom-left corner: X red `#cd0000`, Y green `#009400`, Z blue `#0000d0`, 1 px lines with axis letters (12 px) at the tips. Hidden axes are stippled at 30% opacity. |
| View menu button | below the cube at its bottom-right: a small 3D cube icon (`view-cube-button`, 20 px, gray shading) with a **▾** caret (`#333`, about 6 px), together about 36×22 px. Click opens the view menu. |
| View menu | `ui-ref/view-cube/docs-view-cube-menu.png`. A standard dropdown, about **225 px** wide, 12-13 px text `#333`, about 22 px rows. A left check column (about 20 px) shows `✓` for toggles. Submenus show a right-pointing filled triangle `►` 7 px from the right. Separators are 1 px `#d7d7d7`. Items: Isometric, Dimetric, Trimetric │ ⚙ Graphics preferences… │ Named views…, Previous view │ Zoom to fit, Zoom to window │ Perspective view, Orient normal to sketch on edit │ ✓ Shaded with edges ►, ✓ Hidden edges removed ►, ✓ Tangent edges visible ► │ View in high quality, Highlight boundary edges │ Section view… |

### 3.8 Viewport bottom-right quick tools

At the bottom-right of the viewport, about 8 px above the tab bar and 12 px from the right: three 20 px glyph buttons with no background (`docs-partstudio-annotated.png`, x≈1818-1906 at 125%): **Measure** (`measure-button`, a tape measure), **Analysis tools** (`sketch-analysis-tools`/`analysis-tools`, a curvature comb) and **Mass properties** (`mass-properties`, a balance scale). Hover fills `#eaeaea`, radius 3. Clicking opens a small flyout panel above them.

### 3.9 Feature dialog (floating parameter panel)

The reference is `ui-ref/dialogs/feature-tools__extrudedialog1-03.png`, captured at exactly 1:1 (220 px wide).

| Item | Spec |
|---|---|
| Position | floats over the viewport **at its top-left**, overlapping the feature list's right edge. The default CSS position is `left:161px; top:85px` relative to the document area; in practice it sits a few px right of the feature panel, just under the toolbar. Draggable by its header. |
| Width | **220 px** fixed (`--osx-feature-dialog-width`); some dialogs are wider (Sketch text 404 px, Display states 265 px, "wider" dialogs 440 px) |
| Frame | background `rgba(255,255,255,.95)`; **no border**; shadow `0 5px 10px rgba(0,0,0,.2)`; radius 0 |
| Header | **28 px** tall, white, 1 px `#d7d7d7` bottom border. Title at padding-left 6 px, **13 px / 500 / `#333`**, line-height 28 px, ellipsis. The title is red (`#aa252d`) while the feature has an error or lacks required input, and italic at 50% when suppressed. At the right are two **28×28 px** square buttons with no gap: **Accept**, a green fill `#009400` with a white ✓ (`green-check` glyph, about 18 px) (hover `#74ce3a`; disabled: the same green at 20% opacity, which shows as `#cce9cc`), then **Cancel**, transparent with a red **✕** `#bd3039` about 14 px (hover `#eaeaea`). Clicking the title renames inline. |
| Body | padding about 0 2 px; base font 12 px with line-height 1.8em |
| Enum underline tabs (row 1: Solid / Surface / Thin) | full-width row, **26 px** tall, equal-width segments; text 13.33 px `#666`; the selected segment has text `#1651b0`, weight 500/600 and a **2 px `#1651b0` underline** under that segment. 1 px `#e8e8e8` divider under the row. |
| Operation tabs (row 2: New / Add / Remove / Intersect) | the same style, 26 px. New/Add/Remove/Intersect are text tabs, not icon toggles. |
| Selection box (query list) | 2 px side margins; **min height 26 px**, grows up to 7.5 em then scrolls; 1 px border `#d7d7d7`, radius 2. Placeholder "Faces and sketch regions to extrude" at 12 px `#666`, padding 6 px. **When active (focused for picking):** fill `#def1fb` (sampled `#dbf1fa`), border `#1651b0`. Selected entities are 24 px rows ("Face of Part 1", "Sketch 1 region") at 13.33 px `#555`, padding 0 20 px 0 6 px, with an ✕ remove glyph at the right (`#666`, 125% size, visible on hover) and every second row slightly tinted. Once items exist, the label shrinks to 85% and moves to the top-left. Error state: fill `#f9ebec`, border `#bd3039`. |
| Dropdown (end type "Blind") | a full-width row of about 30 px; the value at 13.33 px `#333`, left-aligned with padding 7 px 0 0 6 px; a small black ▾ caret at the right; 1 px `#d7d7d7` underline (hover `#1651b0`, focused 2 px `#1651b0` inset). A **flip-direction button** (`flip-direction` icon, 26×26, with the same hover/underline behavior) sits at the far right of the same row. Menu: white, shadow, items 13.33 px with 16 px icons when present; hover `#def1fb`; the selected item is `#1651b0` with white text. |
| Value field ("Depth  1 in") | a 26 px row. Label at the left (13.33 px `#666`, padding 7 px 5 px 0, min-width 30 px). The input fills the rest, **right-aligned**, 13.33 px `#333`, padding 6 px 3 px 2 px, with only a **1 px `#d7d7d7` bottom border**: hover `#1651b0`; focused 1 px `#1651b0` plus 1 px inset (2 px look); invalid fill `#f9ebec` with a red underline. The input ends 26-30 px before the right edge, leaving room for a trailing icon button such as the tolerance `±` (`os-param-icon-button` 26×26). |
| Checkbox row ("Symmetric", "Draft") | a 26 px row; the 12×12 checkbox at margin 8 px 4 px 6 px; label 13.33 px `#666` (`#333` on hover), padding 5 px 0 5 px 4 px |
| Collapsible checkbox row ("▸ ☐ Direction", "▸ ☐ Starting offset", "▸ ☐ Second end position") | a 16 px chevron column (`›`, `#666` active, `#d7d7d7` when disabled), then the checkbox and label. Expanded contents are indented **20 px** (`.os-param-group-indent`). |
| Section header (non-checkbox groups) | 13.33 px `#666`, padding 5 px 4 px, with a chevron; the `--os-parameter-section-fill` `#eeeeee` band is used for grouped sections |
| Footer | the current client shows a **preview slider** (a horizontal track `#777`, a 12 px round white handle with a `#888` outline) at the bottom-left and a round **?** help button (`help-button`, `#888`, hover `#666`, about 16 px) at the bottom-right, about 28 px tall. The legacy "Final" checkbox footer (`#eeeeee` fill, 1 px `#d7d7d7` top border) is still used by some dialogs. |
| Parameter highlight | when jumping from the viewport, the parameter gets a 2 px `#90cef1` outline; a changed (configured) parameter gets a `#fab601` border |
| Spacing | parameter rows are **26 px** min-height (enum rows 28 px); there is no extra vertical gap between rows (gap 5 px inside rows) |

### 3.10 Sketch mode

| Item | Spec |
|---|---|
| Toolbar | The **sketch toolbar replaces the feature toolbar** in place, with the same 36 px row and button metrics. Order: Undo · Redo │ Extrude · Revolve (shortcuts) │ Line▾ · Corner rectangle▾ · Center point circle▾ · 3 point arc▾ · Inscribed polygon▾ · Spline▾ · Point · Text │ Use▾ · Construction │ Fillet▾ · Trim▾ · Offset▾ · Mirror · Linear pattern▾ · Insert DXF▾ │ Dimension │ 14 constraints … │ Search tools (`ui-ref/toolbars/docs-sketch-toolbar-part1.png`). Dropdown items show the single-letter shortcut chip at the right (`l`, `g`, `r`, `c`, `a`), 1 px `#aaa` border, radius 2, about 14×14 px. |
| Sketch dialog | the same 220 px feature-dialog frame: header "Sketch 1" with ✓/✕; a sketch-plane selection box (26 px, active-blue) with a 20 px "view normal" clock icon button to its right; checkboxes "Disable imprinting", "Show constraints", "Show expressions", "Show errors" (checked) in 26 px rows; footer with the sketch-analysis icon (`sketch-analysis-tools`, 20 px, boxed 1 px `#aaa`, radius 3) and the **?** (`ui-ref/sketch/docs-sketch-dialog.png`) |
| Viewport | The sketch plane outline and name are `#90cef1`. The camera does not move automatically unless "Orient normal to sketch on edit" is set. |
| Geometry | colors from §1.9; active lines 2 px; points 6 px filled squares/dots in the constraint-state color; endpoints of under-defined geometry are blue |
| Hold **S** shortcut toolbar | a 5×2 grid of 30 px tool buttons in a translucent white box (`--os-shortcut-menu-fill` `#ffffff59`, 1 px `#ccc` border, radius 4), centered on the cursor |

### 3.11 Assembly and Drawing deltas

- **Assembly**: the same navbar, rail, toolbar and tab bar. The left panel is the **Instances list** ("Instances (n)" header with filter, instances, then a "Mates" folder with mate rows), using the same 26 px rows. The toolbar order is in `research/sections/E-assembly.md` and `ui-ref/toolbars/docs-assembly-toolbar.png`. Mate dialogs use the 220 px feature-dialog frame. When a mate is created, a small floating **mate HUD** of mate-type buttons appears (bordered 1 px `#ccc`, radius 4, white 95%, light shadow).
- **Drawing** (`ui-ref/drawing/drawings-tools__drawings-sample.png`, 1:1): the sheet is a white page (`#ffffff`) with a 1 px black frame (zone letters and numbers in the border, a title block at the bottom-right) on a **flat gray surround `#dcdcdc`** (sampled), with no drop shadow on the page. The left panel is the **Sheets** panel: a header band (`#fafafa`) with "Sheets (1)" (18 px / 500, **`#1651b0`**, centered) with an add-sheet button at the right; underline tabs "Sort by sheet | Sort by reference" (13 px, active `#1651b0` with a 2 px underline); then a tree: "Sheet1 (1)" (a selected row with a light-blue fill and a 3 px `#1651b0` left bar), the part reference with an "In progress" status pill (`#90cef1`, radius 2, 12 px), and views (Front, Right, Top, Isometric) with view icons. The watermark "In progress" is drawn diagonally in light gray across an unreleased sheet. The toolbar is longer (`ui-ref/toolbars/docs-drawing-toolbar.png`): views │ dimensions▾ │ annotations │ tables │ centerlines │ sketch tools │ constraints │ insert │ format painter │ inspection. The right panel holds Styles/Properties. Drawing dialogs use the same 220 px frame.

---

## 4. Component visual specs

Values come from [CSS]/[TOK] unless noted. Radii: Onshape barely rounds anything. Tool buttons and icon buttons use 3 px; buttons, inputs, menus and modals use 4 px; tabs use 5 px (bottom corners only); feature dialogs, tooltips and the feature list use 0.

### 4.1 Buttons

| Variant | Size | Radius | Fill (idle / hover / pressed / disabled) | Border | Text |
|---|---|---|---|---|---|
| Primary | height 30-32 px; padding 0 12 px; 6 px icon gap | 4 | `#1651b0` / `#769fcc` / `#1651b0` + inset `0 3px 5px rgba(0,0,0,.12)` / `#a2b9df` | none (1 px transparent) | 14 px / 400-500, white |
| Secondary | same | 4 | `#ffffff` / `#eaeaea` / `#eaeaea` + inset / `#ffffff` (text `#999`) | 1 px `#aaaaaa` | 14 px `#333` |
| Cancel | same | 4 | `#d7d7d7` / `#eeeeee` / inset / `#cccccc` | none | `#333` |
| Small | padding 0 10 px, height about 24 px | 4 | as above | | 12-13 px |
| Icon button (panels, dialog side buttons) | 26×26 (dialog), 30×30 (toolbar), 31 px tall (rail); padding 7 px default, 1 px for "fill" style | 3 | transparent / `#eaeaea` / `#eaeaea` + inset `0 1px 1px rgba(0,0,0,.25)` / opacity .4 | none | icon 20 px |
| Dialog side icon button (flip, ±) | 26×26, padding 3 | 0 | transparent / `#eaeaea`, plus a **1 px `#1651b0` bottom border on hover** / the same plus a 1 px inset underline | none | |
| Pill icon button | | 20 | `#ecf0f4` / `#eaeaea` / selected `#d0dbe5` | | |
| Highlighted icon button (attention) | | 3 | `#fcda7f` | 2 px `#fab601` | |
| Modal dialog buttons | min-width about 80 px, 14 px gap between buttons | 4 | white / `#eaeaea` / selected `#b2ddf6` | **2 px** `#cccccc` (selected `#aaaaaa`) | |

Focus ring: `0 0 0 2px #3e94ff` (keyboard focus only).

### 4.2 Text inputs

| Kind | Spec |
|---|---|
| Boxed field (modals, filter, rename, settings) | white; 1 px `#aaaaaa` (search/filter: `#aaa`; textbox: `#ccc`); radius 4 (filter about 2-3); height **28 px**; padding 4 px 5 px (plus 22 px right padding when there is a clear-× button); 13.33-14 px text `#333`; placeholder `#666` (italic in the feature filter). **Focused: border `#3e94ff`**, 2 px for form fields (`--os-form-field-border-size--focused`) and 1 px for search fields. Error: `#bd3039`. Disabled: fill `#eeeeee`, text `#666`. |
| Underline field (feature dialogs, tables) | no box; 26 px tall; 1 px `#d7d7d7` bottom border; hover border `#1651b0`; focus 1 px `#1651b0` plus inset `0 -1px 0 #1651b0`; invalid fill `#f9ebec` plus red underline; warning `#fab601` underline. Numbers are **right-aligned**. While editing, an expression shows a blue-tinted chip (`#def1fb`) when it references variables. |
| Inline rename (tab, feature) | white fill `#f5f5f5` (`--os-tab-bar-textbox-fill`); 1 px `#1651b0` outline; padding 3 px |

### 4.3 Checkbox, radio, toggle

- **Checkbox**: 12×12 px; radius 3; 1 px `#888` border (hover `#333`); white. Checked: fill `#1651b0`, same border, with a white check (path `M6.564.75l-3.59 3.612-1.538-1.55L0 4.26 2.974 7.25 8 2.193z` in a `-1 -1 10 10` viewBox). Focus: `#3e94ff` border plus a 1 px ring. Disabled: `#999` fill and border. The label sits 4 px right of the box.
- **Radio**: a 12 px circle; active dot `#1651b0`; inactive ring `#333`; label `#333`, subtitle `#999`; 5 px gap.
- **Toggle switch**: track active `#1651b0` / inactive `#eaeaea` with a 1 px `#999` outline; white handle; disabled track `#d0dbe5` (on) / `#f5f5f5` (off).

### 4.4 Selection box (query list) and chips

See §3.9. Additional states:
- **Idle with items**: white; 1 px `#d7d7d7`; entries are 24 px rows of plain text, not rounded chips, with every second row tinted `#ffffff59`.
- **Focused**: `#def1fb` fill, `#1651b0` border. Only one selection box is active at a time, and viewport picks go to it.
- **Hovering a row** highlights the matching entity in the viewport (pre-highlight color) and shows the row's ✕.
- **Selection pills** (used elsewhere, e.g. filters and the version picker): radius 4; 1 px `#d7d7d7`; white; padding 2 px 3 px 2 px 5 px; 3 px gap; selected `#1651b0` with white text.

### 4.5 Dropdowns (select)

- **Field**: as in §3.9 (underline style in dialogs) or boxed (1 px `#aaa`, radius 4, height 28 px) in modals. The caret is a small solid **black ▾** about 7×4 px, 8 px from the right.
- **Menu**: white; 1 px `#d7d7d7` (`--os-dropdown-menu-outline`); radius 4 (condensed) or 5 (wide); shadow `0 5px 10px rgba(0,0,0,.2)` (`0 -4px 8px rgba(0,0,0,.25)` when it opens upward); rows of about 26 px at 13.33 px; text `#555` (selected `#333`); hover `#def1fb`; selected `#b2ddf6` (enum params: `#1651b0` with white text); padding 1 px (condensed) or 5 px (wide); icons 16 px with a 6 px gap.

### 4.6 Context menus

`ui-ref/context-menus/docs-partstudio-context-menu.png`, `docs-assembly-context-menu.png`.

| Item | Spec |
|---|---|
| Container | white; **1 px `#cccccc`**; **radius 4**; shadow `0 5px 10px rgba(0,0,0,.2)`; padding 1 px; min-width 120 px, max-width about 250-300 px |
| Font | **12 px**, Flama (Roboto), 400, `#333` |
| Row | line-height 18 px plus 2 px top and bottom padding = **22 px rows**; padding-left **16 px** (35 px when the menu has icons), right 8 px |
| Icons | 17×17 px, absolutely positioned 10 px from the left (only in menus with icons; the Delete row shows a ⊗ icon `#888`) |
| Hover | `#def1fb` (no text color change) |
| Disabled | text `#555`, no hover |
| Separator | 1 px `#d7d7d7` (the jQuery default is `#ddd`) running the full width, with no margin |
| Submenu | the row has right padding 16 px and a `►` glyph 7 px from the right; view menus use a 5 px CSS triangle `#ccc`. Submenus open to the right and overlap the parent by 5 px. |
| Shortcut hints | right-aligned, 11-12 px `#999` (where present) |
| Section header (e.g. a part name) | some Part Studio menus start with a bold 12 px title row |

### 4.7 Tooltips

| Item | Spec |
|---|---|
| Simple tooltip | fill **`#d0dbe5`**; text `#333` **12 px / 500**; padding 4 px top, 3 px bottom, about 7 px horizontal; **radius 0**; shadow `0 1px 2px rgba(0,0,0,.2)`; max-width 200 px; no arrow; appears below the toolbar button, offset about 4 px |
| Toolbar tooltip content | "Tool name (shortcut)", then a one-line description on hover-hold. Onshape's toolbar tooltips are **expanded**: a white card (`#ffffff`, padding 4 px 3 px, max-width 390 px, 9pt text) with the title in bold, the description, and often an animated GIF preview with a "Learn more" link. The simple blue-gray tooltip is used everywhere else (tabs, panel icons, list rows). |
| Delay | about 500 ms |

### 4.8 Modal dialogs (Preferences, Export, Version, etc.)

White; 1 px `#d7d7d7` border; radius 4; shadow primary. The header has a 16 px / 500 title, padding 10 px 16 px, and a close × (`close-button`, `#888`, hover `#666`) at the right. The body has padding 16 px vertical and 10 px horizontal, with a 10 px gap between controls. The footer is right-aligned: the primary action is blue (e.g. **OK**, **Export**) and Cancel is secondary, with a 14 px gap. A backdrop dims the app with `rgba(0,0,0,.3)`-ish.

### 4.9 Alerts / toasts

These appear at the top-center of the viewport below the toolbar. They have a 1 px border, radius 4, padding 6 px 10 px, a 7 px icon-text gap, 13 px text, and a close × at the right. Colors per kind are in §1.4: info `#def1fb`/`#b2ddf6`/`#094174`, warn `#fff8e6`/`#fcda7f`, error `#f9ebec`/`#dd979b`, success `#ddf6cc`/`#a2db7d`. **Banners** (full-width, e.g. view-only) use the fill `#f5f5f5`, padding 20 px × 8 px and a gap of 20 px.

### 4.10 Panels, splitters, scrollbars

- Panel header text 14 px / 500 `#333`; header height about 30 px; 1 px `#d7d7d7` separators.
- Splitters: a 1 px visible `#d7d7d7` line with a 4-6 px invisible hit area; resize cursor.
- Scrollbars: see §1.7 (thumb `#c1c1c1`, track `#f1f1f1`).
- Empty states: a centered gray illustration (`*-empty` sprite icons) with 13 px `#666` text.

### 4.11 Collapsible tree caret

The caret is a chevron ▸/▾ (sprite `collapsed`/`expanded`), about 10-12 px, `#333`, in a 16 px column. In dialogs a lighter `›` is used (`#666`; `#d7d7d7` when disabled).

---

## 5. Viewport glyphs: dimensions, constraints, selection

### 5.1 Sketch dimensions (`ui-ref/sketch/`, `help-images/raw/Images/concepts/color-sk-*.png`)

- **Lines**: 1 px black, crisp. Extension lines run from the geometry to about 2-3 px past the dimension line. The dimension line is broken around the value text.
- **Arrowheads**: filled black, narrow, about **9-10 px long × 4 px wide**, pointing outward to the extension lines. When the text does not fit, the arrows flip outside.
- **Value text**: about **14 px** Flama (Roboto), `#000`/`#333`, horizontal (screen-aligned, not rotated with the line), centered in the gap. The value carries no units unless they differ from the document units (`2`, `1.5`, `R1.988`, `140°`).
- **Angular**: an arc with arrowheads and the value outside the arc.
- **Radial / diameter**: a leader with one arrow and the prefix `R` / `Ø`.
- **Hover**: the text box gets a `#bf9460` 60% rounded rect (radius 4, padding 5), which renders about `#d9bfa0`, and the line turns `#987107`.
- **Selected**: the box becomes `#bf6900` at 60% (about `#d9a566`), the line `#6a4f06`, 2 px.
- **Driven (reference) dimensions**: `#666`, with the value in parentheses.
- **Editing**: an inline input replaces the value: a white box with a 2 px `#90cef1` border and radius 4.

### 5.2 Sketch constraint glyphs

- **Shown** when "Show constraints" is on, or **on hover** of an entity (`ui-ref/sketch/sketch-tools__constrainticonsonhover.png`).
- **Glyph**: the same artwork as the constraint toolbar icon, simplified and monochrome, drawn at about **12-14 px** inside a small box with a **white fill** (`--os-constraint-ok-color`) and no visible border, placed near the midpoint of the constrained entity (offset perpendicular, about 8-10 px). Several constraints on one entity form a horizontal row with 2 px gaps.
- **States**: hover box `#fcda7f`, selected `#fab601`. Constraints to external references use `#def1fb`/`#b2ddf6`/`#90cef1`. Conflicting constraints use red boxes `#d66e74` (hover `#aa252d`, selected `#bd3039`). A thin leader `#9aa5af` connects the glyph to its entity when they are apart.
- **Coincident at a point**: no box; the point itself is drawn as a filled dot. Midpoint and other point constraints show their glyph near the point.

### 5.3 Selection and hover in the viewport

| State | Faces | Edges / curves | Vertices |
|---|---|---|---|
| Pre-highlight (hover) | no interior tint; only the boundary loop is outlined in `#ffc685`, 2-3 px | entity drawn `#ffc685` (sketch edge hover `#f3bf8e`), thickened to about 3 px | point drawn `#ffc685`, slightly larger |
| Selected | interior tinted `rgba(255,169,10,0.8)` blended (**about `#ffc107` look**), boundary `#ffa90a` | `#ffa90a` (help: `#ffc107`), about 3 px | `#ffa90a` |
| Selected whole part | `rgb(255,192,76)` | | |
| Related / secondary | `rgba(242,255,128,.4)` | | |
| Error | `#bd3039` | `#bd3039` | |

Highlights draw on top with a small depth offset (z-offset constants in [GFX]: plane 23, sketch faces 21, part edges 15, sketch edges 12, points 3-9).

### 5.4 Default planes, origin, triad

- **Planes**: a translucent fill (`#cedbe5` at 20%) with a 2 px outline (`#1d60aa` at 35%), plus the name label "Top" / "Front" / "Right" in **20 px `#1651b0`** at the plane's top-left corner, inside. The plane is sized to 70% of the viewport height, with an aspect of 16:9. On hover the outline turns `#ffc685`; when selected the fill tints yellow.
- **Origin**: a small black point with axis stubs. Hovering shows the three axes.
- **Mate connectors**: a white disc at 80% with a navy `#002847` outline and a triad.

---

## 6. Icon style guide

Source: the client sprite `icons.v1.4.428.min.svg` [ICON]. Every symbol is split into `/root/onshape-research/ui-ref/icons-svg/<symbol>.svg` (colors resolved) and rendered at 4x in `ui-ref/icons-sprite/<symbol>.png`. The inventory icons are copied, named by command, to `ui-ref/icons/<area>-<name>.png/.svg`. Contact sheets per area are `ui-ref/icons/_sheet-<area>.png`. **These are Onshape's copyrighted artwork: use them as visual references only and redraw our own; do not copy paths into the repo.**

### 6.1 Grid and sizes

- **Design grid: 20×20 px** (1,200 of 1,565 symbols use `viewBox="0 0 20 20"`). Artwork usually fills 1-19 (a 1 px margin) and often touches the edges.
- **Rendered sizes**: toolbar 20 px inside a 30 px button; toolbar dropdown rows 20 px; feature/parts list **17 px at opacity 0.8**; tab icons 20 px; left rail 20 px; dialog icon buttons 20 px inside 26 px; enum dropdown items 16 px; icon-label tabs 14 px; context menu 17 px; S-key shortcut toolbar 20 px. Mobile and large UI use 40 px.
- **Recommendation for noshape**: switch to a **20×20 viewBox** to match Onshape's pixel grid. The current icons are 24×24 drawn at 1.5 px strokes, which become 1.25 px at 20 px and look heavier and softer than Onshape's. If the 24 grid stays, scale every measurement below by 1.2.

### 6.2 Construction

- Onshape icons are **filled shapes, not strokes**. Only 30 symbols use `stroke`. Outlines are produced by a dark silhouette path with lighter face paths inset on top of it. The resulting **outline weight is about 1.0-1.35 px on the 20 px grid** (for example `extrude-button`: outer silhouette `M1 1v15l3 3h15V4l-3-3H1z`, faces inset about 1-1.35 px).
- **3D solids use an oblique (cabinet-style) projection, not isometric**: a front face drawn square-on, with depth receding **diagonally at 45° up-left or down-right, about 3 px long** (extrude, shell, hole, chamfer, fillet, rib, split, delete part). Cylinders and cones (sweep, loft, thicken, external thread) are drawn front-on with elliptical caps. Avoid 30° isometric for feature icons; noshape's current icons use it. Isometric appears only in a few view and assembly icons (`view-cube-button`, `composite-part`, `default-assembly`).
- **Three-tone fill model** (tokens with fallbacks):
  - **Outline / dark** `#333333` (`--os-icon-outline-primary`): silhouettes, lines, glyph strokes.
  - **Lit faces** `#FFFFFF` (`--os-icon-fill-primary`): the unaffected body.
  - **Affected face / the feature's result** `#999999` (`--os-icon-fill-secondary`): the extruded boss, the fillet's round face, the chamfer face, the shell cavity, the swept tube, the draft face. Occasionally `#666666` (darker shadow side) and `#D7D7D7`/`#EEEEEE` (planes, light fills; `default-plane-button` uses `#D2D2D2`).
- **Accent blue `#1651B0`** (`--os-icon-accent-primary`) marks *sketch geometry and inputs*: sketch endpoints, construction or "used" edges, the underline under the Sketch pencil, sketch-tool control points, pattern instances, coordinate-system (+) marks, "add/new" plus signs (Create version, Add spline point). Lighter blue `#90CEF1` is used for highlights and good-state mates; `#769FCC` for subtle info.
- **Status colors**: success `#009400` (green check), cancel/error `#BD3039` (red ×, error badges), warning `#FAB601` (warning triangle). Delete-type features overlay a **dark `#333` disc with a white ×** at the bottom-right (`delete-bodies-button`, `delete-face-button`, `frame-trim-button`).
- **Sketch tools** (`sketch-*-button`) are 2D line art: curves are 1.0-1.3 px `#333` filled paths; **control points / endpoints are hollow blue rings**, 4 px outer diameter (r=2), 0.5 px ring, white center. Center points are a blue ring or a small `+`. Dashed segments indicate construction or ghost geometry.
- **Constraint icons** are bold glyphs: horizontal/vertical bars 2 px `#333`; parallel and perpendicular lines 1.3-2 px; a blue 1 px secondary element (e.g. the second line of Parallel, the circle of Tangent, the second arc of Concentric). Coincident is a `#333` dot with two lines, one blue and one dark.
- **Mate icons**: each mate type has its own glyph (fastened = a cylinder with a T-cap, revolute = a cylinder with a rotation arrow, and so on), with variants `_Error` (red), `_Suppressed` (gray), `_External` (blue-tinted) and `_External_Suppressed` used in the instance list.
- **Drawing tools**: mostly line art with sheet or view frames (`drawing-insert-view`, `drawing-section-view`, ...). Dimension tools show arrows and extension lines with a blue highlight on the measured entity.
- **Element (tab) icons**: Part Studio is a stack of blocks (`partstudio`); Assembly is two interlocking blocks (`default-assembly`); Drawing is a sheet with a view (`drawing`); Variable Studio and Feature Studio have their own document-style glyphs. All are 20 px, mostly `#333`/`#999`, with a blue detail.
- **Overlays / badges** (12×12 at offset 10, 9 in list rows): error (red disc with a white `!` or ×), warning (yellow triangle), under-defined (a small blue "-" badge), external link (chain), configured (a small table grid).

### 6.3 Rendering requirement in GPUI (important)

GPUI's `svg()` / `Icon::path` draw **monochrome alpha masks** tinted with one color. Onshape's icons depend on three or four colors (white faces inside dark outlines, gray affected faces, blue accents). Under a mask, white faces become the tint color and the icon collapses into a silhouette.

- To match Onshape, **render toolbar, feature-list, tab and dialog icons as full-color SVG images** (`gpui::img("icons/x.svg")`, which rasterizes SVGs in color through resvg). Alternatively, ship each icon as two or three mask layers (`x.outline.svg`, `x.fill.svg`, `x.accent.svg`) and stack them, tinted `#333` / `#999` / `#1651b0`, with the white faces as a fourth layer.
- Keep mask rendering only for single-color glyphs (chevrons, ×, checkmarks, the ▾ caret).
- **Disabled** icons: opacity 0.4. **Hidden or suppressed list items**: grayscale at 50% opacity (`filter: grayscale(1)` with `opacity .5`).

### 6.4 How each feature is depicted (key examples; see `ui-ref/icons/_sheet-part-studio.png`)

| Feature | Depiction |
|---|---|
| Extrude | a white box (front face plus 3 px oblique depth) with a smaller **gray box** protruding from its front-bottom-right |
| Revolve | a gray "C"/pac-man disc with a dashed axis line through its center and an open notch |
| Sweep | a gray S-shaped tube with an elliptical end cap |
| Loft | a gray truncated cone (narrow top ellipse, flared base) |
| Thicken | a white box with a gray top surface and a **down arrow** through it |
| Enclose | a gray cup inside dashed corner brackets |
| Fillet | a white box whose top-left edge is a **gray rounded quarter-round** face |
| Chamfer | a white box with a **gray flat bevel** on its top-left edge |
| Draft | a gray tapered block (trapezoid prism) |
| Shell | a white box with a **gray recessed cavity** (inner wall) |
| Hole | a white box with a gray counterbored circular hole on its front face |
| External thread | a gray cylinder with horizontal thread bands |
| Linear pattern | a 2×2 grid of small boxes, the top-left one white and the others gray |
| Circular pattern | four small cylinders arranged on a dashed circular arc |
| Mirror | two L-blocks mirrored about a vertical dashed plane, one white and one gray |
| Boolean | two overlapping circles with a gray intersection lens |
| Split | a white box cut by a curved gray sheet |
| Transform | a box with two curved arrows and a diamond (rotated copy) |
| Delete part | a gray box with a dark × badge |
| Plane | a gray parallelogram plane with an inner corner line |
| Mate connector | a circle with a quarter "pie" (coordinate-system glyph) |
| Sketch | a pencil (`#333` outline, white body) over a **blue underline** |
| Variable | a bold italic **(𝑥)** glyph in `#333` |
| Sheet metal | bent L/U plate shapes in white with gray bend zones |

### 6.5 Icon inventory

Every toolbar command in Part Studio, Sketch, Assembly and Drawing, plus document, view, panel, navbar, tab and dialog icons, mapped to Onshape's sprite symbol and noshape's current file. The reference column points to the 4x render (80×80 px for 20 px icons; larger symbols at 2x or 1x) and, next to it, the `.svg` with resolved colors. Both are under `/root/onshape-research/` and are **not** to be copied into the repo. Data: `ui-ref/icons/inventory.json`; generator: `/root/onshape-research/scripts/build_inventory.py`. The Onshape name to symbol mapping comes from the official **Icon Glossary** help page (`/help/Content/Home/icon_glossary.htm`, parsed into `ui-ref/help-images/icon-glossary.json`), extended by hand for chrome icons.

| # | Area | Onshape name | Onshape sprite symbol | noshape icon (`crates/app/assets/icons/`) | Reference (under `/root/onshape-research/`) |
|---|---|---|---|---|---|
| 1 | Toolbar (all) | Undo | `undo-button` | `undo.svg` | `ui-ref/icons/toolbar-all-undo.png` |
| 2 | Toolbar (all) | Redo | `redo-button` | `redo.svg` | `ui-ref/icons/toolbar-all-redo.png` |
| 3 | Toolbar (all) | Dropdown caret | `dropdown-arrow` | `chevron-down.svg` | `ui-ref/icons/toolbar-all-dropdown-caret.png` |
| 4 | Toolbar (all) | Search tools | `search` | `search.svg` | `ui-ref/icons/toolbar-all-search-tools.png` |
| 5 | Toolbar (all) | Add custom feature | `add-toolbar-feature` | `custom-feature.svg` | `ui-ref/icons/toolbar-all-add-custom-feature.png` |
| 6 | Part Studio | Extrude | `extrude-button` | `extrude.svg` | `ui-ref/icons/part-studio-extrude.png` |
| 7 | Part Studio | Revolve | `revolve-button` | `revolve.svg` | `ui-ref/icons/part-studio-revolve.png` |
| 8 | Part Studio | Sweep | `sweep-button` | `sweep.svg` | `ui-ref/icons/part-studio-sweep.png` |
| 9 | Part Studio | Loft | `loft-button` | `loft.svg` | `ui-ref/icons/part-studio-loft.png` |
| 10 | Part Studio | Thicken | `thicken-button` | `thicken.svg` | `ui-ref/icons/part-studio-thicken.png` |
| 11 | Part Studio | Enclose | `enclose-button` | `enclose.svg` | `ui-ref/icons/part-studio-enclose.png` |
| 12 | Part Studio | Fillet | `fillet-button` | `fillet.svg` | `ui-ref/icons/part-studio-fillet.png` |
| 13 | Part Studio | Face blend | `face-blend-button` | `face-blend.svg` | `ui-ref/icons/part-studio-face-blend.png` |
| 14 | Part Studio | Chamfer | `chamfer-button` | `chamfer.svg` | `ui-ref/icons/part-studio-chamfer.png` |
| 15 | Part Studio | Draft | `draft-button` | `draft.svg` | `ui-ref/icons/part-studio-draft.png` |
| 16 | Part Studio | Body draft | `body-draft-button` | `body-draft.svg` | `ui-ref/icons/part-studio-body-draft.png` |
| 17 | Part Studio | Rib | `rib-button` | `rib.svg` | `ui-ref/icons/part-studio-rib.png` |
| 18 | Part Studio | Shell | `shell-button` | `shell.svg` | `ui-ref/icons/part-studio-shell.png` |
| 19 | Part Studio | Hole | `hole-button` | `hole.svg` | `ui-ref/icons/part-studio-hole.png` |
| 20 | Part Studio | External thread | `external-thread-button` | `external-thread.svg` | `ui-ref/icons/part-studio-external-thread.png` |
| 21 | Part Studio | Linear pattern | `linear-pattern-button` | `linear-pattern.svg` | `ui-ref/icons/part-studio-linear-pattern.png` |
| 22 | Part Studio | Circular pattern | `circular-pattern-button` | `circular-pattern.svg` | `ui-ref/icons/part-studio-circular-pattern.png` |
| 23 | Part Studio | Curve pattern | `curve-pattern-button` | `curve-pattern.svg` | `ui-ref/icons/part-studio-curve-pattern.png` |
| 24 | Part Studio | Mirror | `mirror-button` | `mirror.svg` | `ui-ref/icons/part-studio-mirror.png` |
| 25 | Part Studio | Boolean | `boolean-bodies-button` | `boolean.svg` | `ui-ref/icons/part-studio-boolean.png` |
| 26 | Part Studio | Split | `split-part-button` | `split.svg` | `ui-ref/icons/part-studio-split.png` |
| 27 | Part Studio | Transform | `transform-button` | `transform.svg` | `ui-ref/icons/part-studio-transform.png` |
| 28 | Part Studio | Wrap | `wrap-button` | `wrap.svg` | `ui-ref/icons/part-studio-wrap.png` |
| 29 | Part Studio | Decal | `decal-button` | `decal.svg` | `ui-ref/icons/part-studio-decal.png` |
| 30 | Part Studio | Delete part | `delete-bodies-button` | `delete-part.svg` | `ui-ref/icons/part-studio-delete-part.png` |
| 31 | Part Studio | Modify fillet | `modify-fillet-button` | `modify-fillet.svg` | `ui-ref/icons/part-studio-modify-fillet.png` |
| 32 | Part Studio | Delete face | `delete-face-button` | `delete-face.svg` | `ui-ref/icons/part-studio-delete-face.png` |
| 33 | Part Studio | Move face | `move-face-button` | `move-face.svg` | `ui-ref/icons/part-studio-move-face.png` |
| 34 | Part Studio | Replace face | `replace-face-button` | `replace-face.svg` | `ui-ref/icons/part-studio-replace-face.png` |
| 35 | Part Studio | Plane | `default-plane-button` | `plane.svg` | `ui-ref/icons/part-studio-plane.png` |
| 36 | Part Studio | Offset surface | `offset-surface-button` | `offset-surface.svg` | `ui-ref/icons/part-studio-offset-surface.png` |
| 37 | Part Studio | Boundary surface | `boundary-surface-button` | `boundary-surface.svg` | `ui-ref/icons/part-studio-boundary-surface.png` |
| 38 | Part Studio | Fill | `fill-button` | `fill.svg` | `ui-ref/icons/part-studio-fill.png` |
| 39 | Part Studio | Move boundary | `extend-surface-button` | `move-boundary.svg` | `ui-ref/icons/part-studio-move-boundary.png` |
| 40 | Part Studio | Ruled surface | `ruled-surface-button` | `ruled-surface.svg` | `ui-ref/icons/part-studio-ruled-surface.png` |
| 41 | Part Studio | Mutual trim | `mutual-trim-button` | `mutual-trim.svg` | `ui-ref/icons/part-studio-mutual-trim.png` |
| 42 | Part Studio | Constrained surface | `constrained-surface-button` | `constrained-surface.svg` | `ui-ref/icons/part-studio-constrained-surface.png` |
| 43 | Part Studio | Helix | `helix-button` | `helix.svg` | `ui-ref/icons/part-studio-helix.png` |
| 44 | Part Studio | 3D fit spline | `fit-spline-button` | `fit-spline.svg` | `ui-ref/icons/part-studio-3d-fit-spline.png` |
| 45 | Part Studio | Projected curve | `project-curves-button` | `projected-curve.svg` | `ui-ref/icons/part-studio-projected-curve.png` |
| 46 | Part Studio | Bridging curve | `bridging-curve-button` | `bridging-curve.svg` | `ui-ref/icons/part-studio-bridging-curve.png` |
| 47 | Part Studio | Composite curve | `composite-curve-button` | `composite-curve.svg` | `ui-ref/icons/part-studio-composite-curve.png` |
| 48 | Part Studio | Intersection curve | `intersection-curve-button` | `intersection-curve.svg` | `ui-ref/icons/part-studio-intersection-curve.png` |
| 49 | Part Studio | Trim curve | `trim-curve-button` | `trim-curve.svg` | `ui-ref/icons/part-studio-trim-curve.png` |
| 50 | Part Studio | Isocline | `isocline-button` | `isocline.svg` | `ui-ref/icons/part-studio-isocline.png` |
| 51 | Part Studio | Offset curve | `(help image)` | `offset-curve.svg` | `ui-ref/icons/part-studio-offset-curve.png` |
| 52 | Part Studio | Isoparametric curve | `isoparametric-curve-button` | `isoparametric.svg` | `ui-ref/icons/part-studio-isoparametric-curve.png` |
| 53 | Part Studio | Edit curve | `edit-curve-button` | `edit-curve.svg` | `ui-ref/icons/part-studio-edit-curve.png` |
| 54 | Part Studio | Routing curve | `routing-curve-button` | `routing-curve.svg` | `ui-ref/icons/part-studio-routing-curve.png` |
| 55 | Part Studio | Mate connector | `mateConnector` | `mate-connector.svg` | `ui-ref/icons/part-studio-mate-connector.png` |
| 56 | Part Studio | Derived | `import-derived-button` | `derived.svg` | `ui-ref/icons/part-studio-derived.png` |
| 57 | Part Studio | Variable | `variablestudio` | `variable.svg` | `ui-ref/icons/part-studio-variable.png` |
| 58 | Part Studio | Query variable | `query-variable-button` | `query-variable.svg` | `ui-ref/icons/part-studio-query-variable.png` |
| 59 | Part Studio | Composite part | `composite-part-button` | `composite-part.svg` | `ui-ref/icons/part-studio-composite-part.png` |
| 60 | Part Studio | Tag | `tag-button` | `tag.svg` | `ui-ref/icons/part-studio-tag.png` |
| 61 | Part Studio | Frame | `frame-button` | `frame.svg` | `ui-ref/icons/part-studio-frame.png` |
| 62 | Part Studio | Frame trim | `frame-trim-button` | `frame-trim.svg` | `ui-ref/icons/part-studio-frame-trim.png` |
| 63 | Part Studio | Gusset | `gusset-button` | `gusset.svg` | `ui-ref/icons/part-studio-gusset.png` |
| 64 | Part Studio | End cap | `endcap-button` | `end-cap.svg` | `ui-ref/icons/part-studio-end-cap.png` |
| 65 | Part Studio | Cut list | `cutlist-button` | `cut-list.svg` | `ui-ref/icons/part-studio-cut-list.png` |
| 66 | Part Studio | Sheet metal model | `sheet-metal-start-button` | `sheet-metal.svg` | `ui-ref/icons/part-studio-sheet-metal-model.png` |
| 67 | Part Studio | Flange | `sheet-metal-flange-button` | `flange.svg` | `ui-ref/icons/part-studio-flange.png` |
| 68 | Part Studio | Hem | `sheet-metal-hem-button` | `hem.svg` | `ui-ref/icons/part-studio-hem.png` |
| 69 | Part Studio | Tab | `sheet-metal-tab-button` | `tab.svg` | `ui-ref/icons/part-studio-tab.png` |
| 70 | Part Studio | Bend | `sheet-metal-bend-button` | `sheet-metal-bend.svg` | `ui-ref/icons/part-studio-bend.png` |
| 71 | Part Studio | Form | `(help image)` | `sheet-metal-form.svg` | `ui-ref/icons/part-studio-form.png` |
| 72 | Part Studio | Sheet metal loft | `sheet-metal-loft-button` | `sheet-metal-loft.svg` | `ui-ref/icons/part-studio-sheet-metal-loft.png` |
| 73 | Part Studio | Make joint | `sheet-metal-make-joint-button` | `sheet-metal-make-joint.svg` | `ui-ref/icons/part-studio-make-joint.png` |
| 74 | Part Studio | Corner | `sheet-metal-corner-button` | `sheet-metal-corner.svg` | `ui-ref/icons/part-studio-corner.png` |
| 75 | Part Studio | Bend relief | `sheet-metal-bend-relief-button` | `bend-relief.svg` | `ui-ref/icons/part-studio-bend-relief.png` |
| 76 | Part Studio | Corner break | `sheet-metal-corner-break-button` | `corner-break.svg` | `ui-ref/icons/part-studio-corner-break.png` |
| 77 | Part Studio | Finish sheet metal model | `sheet-metal-end-button` | `sheet-metal-finish.svg` | `ui-ref/icons/part-studio-finish-sheet-metal-model.png` |
| 78 | Part Studio | Flex PCB model | `pcb-start-button` | **missing** | `ui-ref/icons/part-studio-flex-pcb-model.png` |
| 79 | Part Studio | Finish flex PCB model | `pcb-end-button` | **missing** | `ui-ref/icons/part-studio-finish-flex-pcb-model.png` |
| 80 | Part Studio | Sketch | `new-sketch-button` | `sketch.svg` | `ui-ref/icons/part-studio-sketch.png` |
| 81 | Part Studio | Mate connector (toolbar) | `mate-connector-button` | `mate-connector.svg` | `ui-ref/icons/part-studio-mate-connector-toolbar.png` |
| 82 | Sketch | Extrude | `extrude-button` | `extrude.svg` | `ui-ref/icons/sketch-extrude.png` |
| 83 | Sketch | Revolve | `revolve-button` | `revolve.svg` | `ui-ref/icons/sketch-revolve.png` |
| 84 | Sketch | Line | `sketch-line-segment-button` | `line.svg` | `ui-ref/icons/sketch-line.png` |
| 85 | Sketch | Midpoint line | `sketch-midpoint-line-button` | `midpoint-line.svg` | `ui-ref/icons/sketch-midpoint-line.png` |
| 86 | Sketch | Corner rectangle | `sketch-rectangle-button` | `corner-rectangle.svg` | `ui-ref/icons/sketch-corner-rectangle.png` |
| 87 | Sketch | Center point rectangle | `sketch-center-rectangle-button` | `center-rectangle.svg` | `ui-ref/icons/sketch-center-point-rectangle.png` |
| 88 | Sketch | Aligned rectangle | `sketch-aligned-rectangle-button` | `aligned-rectangle.svg` | `ui-ref/icons/sketch-aligned-rectangle.png` |
| 89 | Sketch | Center point circle | `sketch-circle-button` | `circle.svg` | `ui-ref/icons/sketch-center-point-circle.png` |
| 90 | Sketch | 3 point circle | `sketch-perimeter-circle-button` | `three-point-circle.svg` | `ui-ref/icons/sketch-3-point-circle.png` |
| 91 | Sketch | Ellipse | `sketch-ellipse-button` | `ellipse.svg` | `ui-ref/icons/sketch-ellipse.png` |
| 92 | Sketch | 3 point arc | `sketch-arc-button` | `three-point-arc.svg` | `ui-ref/icons/sketch-3-point-arc.png` |
| 93 | Sketch | Tangent arc | `sketch-tangent-arc-button` | `tangent-arc.svg` | `ui-ref/icons/sketch-tangent-arc.png` |
| 94 | Sketch | Center point arc | `sketch-center-arc-button` | `center-point-arc.svg` | `ui-ref/icons/sketch-center-point-arc.png` |
| 95 | Sketch | Elliptical arc | `elliptical-arc-button` | `elliptical-arc.svg` | `ui-ref/icons/sketch-elliptical-arc.png` |
| 96 | Sketch | Conic | `sketch-conic-button` | `conic.svg` | `ui-ref/icons/sketch-conic.png` |
| 97 | Sketch | Inscribed polygon | `sketch-inscribed-polygon-button` | `polygon-inscribed.svg` | `ui-ref/icons/sketch-inscribed-polygon.png` |
| 98 | Sketch | Circumscribed polygon | `sketch-circumscribed-polygon-button` | `polygon-circumscribed.svg` | `ui-ref/icons/sketch-circumscribed-polygon.png` |
| 99 | Sketch | Spline | `sketch-spline-button` | `spline.svg` | `ui-ref/icons/sketch-spline.png` |
| 100 | Sketch | Bezier | `sketch-bezier-button` | `bezier.svg` | `ui-ref/icons/sketch-bezier.png` |
| 101 | Sketch | Spline control point | `add-spline-point` | `spline.svg` | `ui-ref/icons/sketch-spline-control-point.png` |
| 102 | Sketch | Point | `sketch-point-button` | `point-sketch.svg` | `ui-ref/icons/sketch-point.png` |
| 103 | Sketch | Text | `sketch-text-rectangle-button` | `text.svg` | `ui-ref/icons/sketch-text.png` |
| 104 | Sketch | Use | `sketch-use-button` | `use.svg` | `ui-ref/icons/sketch-use.png` |
| 105 | Sketch | Intersection | `sketch-intersection-button` | `intersection.svg` | `ui-ref/icons/sketch-intersection.png` |
| 106 | Sketch | Construction | `sketch-construction-button` | `construction.svg` | `ui-ref/icons/sketch-construction.png` |
| 107 | Sketch | Fillet | `sketch-fillet-button` | `sketch-fillet.svg` | `ui-ref/icons/sketch-fillet.png` |
| 108 | Sketch | Chamfer | `sketch-chamfer-button` | `sketch-chamfer.svg` | `ui-ref/icons/sketch-chamfer.png` |
| 109 | Sketch | Trim | `sketch-trim-button` | `trim.svg` | `ui-ref/icons/sketch-trim.png` |
| 110 | Sketch | Extend | `sketch-extend-button` | `extend.svg` | `ui-ref/icons/sketch-extend.png` |
| 111 | Sketch | Split | `sketch_split_icon` | `split-sketch.svg` | `ui-ref/icons/sketch-split.png` |
| 112 | Sketch | Offset | `sketch-offset-button` | `offset.svg` | `ui-ref/icons/sketch-offset.png` |
| 113 | Sketch | Slot | `sketch-slot-button` | `slot.svg` | `ui-ref/icons/sketch-slot.png` |
| 114 | Sketch | Mirror | `sketch-mirror-button` | `sketch-mirror.svg` | `ui-ref/icons/sketch-mirror.png` |
| 115 | Sketch | Linear pattern | `sketch-lpattern-button` | `sketch-linear-pattern.svg` | `ui-ref/icons/sketch-linear-pattern.png` |
| 116 | Sketch | Circular pattern | `sketch-cpattern-button` | `sketch-circular-pattern.svg` | `ui-ref/icons/sketch-circular-pattern.png` |
| 117 | Sketch | Transform | `sketch-transform-button` | `transform-sketch.svg` | `ui-ref/icons/sketch-transform.png` |
| 118 | Sketch | Insert DXF or DWG | `150715_Sketch_Icons_30_InsertDXFDWG` | `import.svg` | `ui-ref/icons/sketch-insert-dxf-or-dwg.png` |
| 119 | Sketch | Insert image | `sketch-image-rectangle-button` | `image.svg` | `ui-ref/icons/sketch-insert-image.png` |
| 120 | Sketch | Dimension | `sketch-dimension-button` | `dimension.svg` | `ui-ref/icons/sketch-dimension.png` |
| 121 | Sketch | Coincident | `sketch-coincident-button` | `coincident.svg` | `ui-ref/icons/sketch-coincident.png` |
| 122 | Sketch | Concentric | `sketch-concentric-button` | `concentric.svg` | `ui-ref/icons/sketch-concentric.png` |
| 123 | Sketch | Parallel | `sketch-parallel-button` | `parallel.svg` | `ui-ref/icons/sketch-parallel.png` |
| 124 | Sketch | Tangent | `sketch-tangent-button` | `tangent.svg` | `ui-ref/icons/sketch-tangent.png` |
| 125 | Sketch | Horizontal | `sketch-horizontal-button` | `horizontal.svg` | `ui-ref/icons/sketch-horizontal.png` |
| 126 | Sketch | Vertical | `sketch-vertical-button` | `vertical.svg` | `ui-ref/icons/sketch-vertical.png` |
| 127 | Sketch | Perpendicular | `sketch-perpendicular-button` | `perpendicular.svg` | `ui-ref/icons/sketch-perpendicular.png` |
| 128 | Sketch | Equal | `sketch-equal-button` | `equal.svg` | `ui-ref/icons/sketch-equal.png` |
| 129 | Sketch | Midpoint | `sketch-midpoint-button` | `midpoint.svg` | `ui-ref/icons/sketch-midpoint.png` |
| 130 | Sketch | Normal | `sketch-normal-button` | `normal.svg` | `ui-ref/icons/sketch-normal.png` |
| 131 | Sketch | Pierce | `sketch-pierce-button` | `pierce.svg` | `ui-ref/icons/sketch-pierce.png` |
| 132 | Sketch | Symmetric | `sketch-symmetric-button` | `symmetric.svg` | `ui-ref/icons/sketch-symmetric.png` |
| 133 | Sketch | Fix | `sketch-fix-button` | `fix.svg` | `ui-ref/icons/sketch-fix.png` |
| 134 | Sketch | Curvature | `equal-curvature` | `curvature.svg` | `ui-ref/icons/sketch-curvature.png` |
| 135 | Assembly | Insert | `assembly-insert-button` | `insert.svg` | `ui-ref/icons/assembly-insert.png` |
| 136 | Assembly | Mate connector | `mateConnector` | `mate-connector-assembly.svg` | `ui-ref/icons/assembly-mate-connector.png` |
| 137 | Assembly | Fastened mate | `fastened` | `fastened.svg` | `ui-ref/icons/assembly-fastened-mate.png` |
| 138 | Assembly | Revolute mate | `revolute` | `revolute.svg` | `ui-ref/icons/assembly-revolute-mate.png` |
| 139 | Assembly | Slider mate | `slider` | `slider.svg` | `ui-ref/icons/assembly-slider-mate.png` |
| 140 | Assembly | Planar mate | `planar` | `planar.svg` | `ui-ref/icons/assembly-planar-mate.png` |
| 141 | Assembly | Cylindrical mate | `cylindrical` | `cylindrical.svg` | `ui-ref/icons/assembly-cylindrical-mate.png` |
| 142 | Assembly | Pin slot mate | `pin_slot` | `pin-slot.svg` | `ui-ref/icons/assembly-pin-slot-mate.png` |
| 143 | Assembly | Ball mate | `ball` | `ball.svg` | `ui-ref/icons/assembly-ball-mate.png` |
| 144 | Assembly | Parallel mate | `parallel` | `parallel-mate.svg` | `ui-ref/icons/assembly-parallel-mate.png` |
| 145 | Assembly | Tangent mate | `tangent` | `tangent-mate.svg` | `ui-ref/icons/assembly-tangent-mate.png` |
| 146 | Assembly | Width mate | `width` | `width-mate.svg` | `ui-ref/icons/assembly-width-mate.png` |
| 147 | Assembly | Group | `(help image)` | `group.svg` | `ui-ref/icons/assembly-group.png` |
| 148 | Assembly | Snap mode | `SnapMate` | `snap-mode.svg` | `ui-ref/icons/assembly-snap-mode.png` |
| 149 | Assembly | Show mates mode | `(help image)` | `show-mates.svg` | `ui-ref/icons/assembly-show-mates-mode.png` |
| 150 | Assembly | Replicate | `AssemblyReplicate` | `replicate.svg` | `ui-ref/icons/assembly-replicate.png` |
| 151 | Assembly | Replace instance | `replace-part` | `replace-part.svg` | `ui-ref/icons/assembly-replace-instance.png` |
| 152 | Assembly | Assembly linear pattern | `linear-pattern-button` | `linear-pattern.svg` | `ui-ref/icons/assembly-assembly-linear-pattern.png` |
| 153 | Assembly | Assembly circular pattern | `circular-pattern-button` | `circular-pattern.svg` | `ui-ref/icons/assembly-assembly-circular-pattern.png` |
| 154 | Assembly | Assembly mirror | `mirror-button` | `mirror.svg` | `ui-ref/icons/assembly-assembly-mirror.png` |
| 155 | Assembly | Gear relation | `gear` | `gear-relation.svg` | `ui-ref/icons/assembly-gear-relation.png` |
| 156 | Assembly | Rack and pinion relation | `rack_and_pinion` | `rack-pinion.svg` | `ui-ref/icons/assembly-rack-and-pinion-relation.png` |
| 157 | Assembly | Screw relation | `screw` | `screw-relation.svg` | `ui-ref/icons/assembly-screw-relation.png` |
| 158 | Assembly | Linear relation | `linear` | `linear-relation.svg` | `ui-ref/icons/assembly-linear-relation.png` |
| 159 | Assembly | Display states | `display-states-button` | `display-states.svg` | `ui-ref/icons/assembly-display-states.png` |
| 160 | Assembly | Create Part Studio in context | `create-partstudio-incontext` | `part-studio.svg` | `ui-ref/icons/assembly-create-part-studio-in-context.png` |
| 161 | Assembly | BOM Table | `bom-table-button` | `bom.svg` | `ui-ref/icons/assembly-bom-table.png` |
| 162 | Assembly | Configuration panel | `configuration-table-button` | `configuration.svg` | `ui-ref/icons/assembly-configuration-panel.png` |
| 163 | Assembly | Exploded views | `explode-button` | `explode.svg` | `ui-ref/icons/assembly-exploded-views.png` |
| 164 | Assembly | Named positions | `named-position-button` | `named-position.svg` | `ui-ref/icons/assembly-named-positions.png` |
| 165 | Assembly | Simulation | `simulation-panel-button` | **missing** | `ui-ref/icons/assembly-simulation.png` |
| 166 | Assembly | Variable table | `variable-table-button` | `variables-panel.svg` | `ui-ref/icons/assembly-variable-table.png` |
| 167 | Assembly | Update (regenerate) | `update-all-button` | **missing** | `ui-ref/icons/assembly-update-regenerate.png` |
| 168 | Drawing | Insert view | `drawing-insert-view` | `base-view.svg` | `ui-ref/icons/drawing-insert-view.png` |
| 169 | Drawing | Projected view | `drawing-projected-view` | `projected-view.svg` | `ui-ref/icons/drawing-projected-view.png` |
| 170 | Drawing | Auxiliary view | `drawing-auxiliary-view` | `auxiliary-view.svg` | `ui-ref/icons/drawing-auxiliary-view.png` |
| 171 | Drawing | Section view | `drawing-section-view` | `section-view.svg` | `ui-ref/icons/drawing-section-view.png` |
| 172 | Drawing | Aligned section view | `drawing-aligned-section` | `section-view.svg` | `ui-ref/icons/drawing-aligned-section-view.png` |
| 173 | Drawing | Broken-out section | `BrokenOutSection` | `broken-out-section.svg` | `ui-ref/icons/drawing-broken-out-section.png` |
| 174 | Drawing | Detail view | `drawing-detail-view` | `detail-view.svg` | `ui-ref/icons/drawing-detail-view.png` |
| 175 | Drawing | Break view | `drawing-break-view` | `broken-view.svg` | `ui-ref/icons/drawing-break-view.png` |
| 176 | Drawing | Crop view | `CropView` | `crop-view.svg` | `ui-ref/icons/drawing-crop-view.png` |
| 177 | Drawing | Dimension | `drawing-smart-dim` | `linear-dim.svg` | `ui-ref/icons/drawing-dimension.png` |
| 178 | Drawing | Chamfer dimension | `drawing-chamfer-dim` | `chamfer-dim.svg` | `ui-ref/icons/drawing-chamfer-dimension.png` |
| 179 | Drawing | Maximum or minimum dimension | `drawing-linear-dim-circles-min-max` | `linear-dim.svg` | `ui-ref/icons/drawing-maximum-or-minimum-dimension.png` |
| 180 | Drawing | 2 point linear dimension | `drawing-linear-dim-point-to-point` | `linear-dim.svg` | `ui-ref/icons/drawing-2-point-linear-dimension.png` |
| 181 | Drawing | Point to line dimension | `drawing-linear-dim-point-to-line` | `linear-dim.svg` | `ui-ref/icons/drawing-point-to-line-dimension.png` |
| 182 | Drawing | Line to line dimension | `drawing-linear-dim-line-to-line` | `linear-dim.svg` | `ui-ref/icons/drawing-line-to-line-dimension.png` |
| 183 | Drawing | Line to line angular dimension | `drawing-angular-dim-line-to-line` | `angular-dim.svg` | `ui-ref/icons/drawing-line-to-line-angular-dimension.png` |
| 184 | Drawing | 3 point angular dimension | `drawing-angular-dim-3pt` | `angular-dim.svg` | `ui-ref/icons/drawing-3-point-angular-dimension.png` |
| 185 | Drawing | Arc length dimension | `arcLength` | `arc-length-dim.svg` | `ui-ref/icons/drawing-arc-length-dimension.png` |
| 186 | Drawing | Radial dimension | `drawing-radial-dim` | `radial-dim.svg` | `ui-ref/icons/drawing-radial-dimension.png` |
| 187 | Drawing | Diameter dimension | `drawing-diameter-dim` | `diameter-dim.svg` | `ui-ref/icons/drawing-diameter-dimension.png` |
| 188 | Drawing | Foreshorten dimension | `drawing-half-dim` | `radial-dim.svg` | `ui-ref/icons/drawing-foreshorten-dimension.png` |
| 189 | Drawing | Ordinate dimension | `drawing-ordinate-dim` | `ordinate-dim.svg` | `ui-ref/icons/drawing-ordinate-dimension.png` |
| 190 | Drawing | Hole/thread callout | `drawing-hole-callout` | `hole-callout.svg` | `ui-ref/icons/drawing-hole-thread-callout.png` |
| 191 | Drawing | Datum | `drawing-datum` | `datum.svg` | `ui-ref/icons/drawing-datum.png` |
| 192 | Drawing | Geometric tolerance | `(help image)` | `gdt.svg` | `ui-ref/icons/drawing-geometric-tolerance.png` |
| 193 | Drawing | Surface finish symbol | `drawing-surface-finish-symbol` | `surface-finish.svg` | `ui-ref/icons/drawing-surface-finish-symbol.png` |
| 194 | Drawing | Weld symbol | `drawing-weld-symbol` | **missing** | `ui-ref/icons/drawing-weld-symbol.png` |
| 195 | Drawing | Note | `drawing-note` | `note.svg` | `ui-ref/icons/drawing-note.png` |
| 196 | Drawing | Callout | `drawing-callout` | `callout.svg` | `ui-ref/icons/drawing-callout.png` |
| 197 | Drawing | Table | `drawing-table` | `table.svg` | `ui-ref/icons/drawing-table.png` |
| 198 | Drawing | BOM table | `drawing-bom` | `bom.svg` | `ui-ref/icons/drawing-bom-table.png` |
| 199 | Drawing | Cut list table | `drawing-cutlist-table` | `cut-list.svg` | `ui-ref/icons/drawing-cut-list-table.png` |
| 200 | Drawing | Hole table | `drawing-hole-table` | `hole-table.svg` | `ui-ref/icons/drawing-hole-table.png` |
| 201 | Drawing | Custom table | `drawing-custom-table` | `table.svg` | `ui-ref/icons/drawing-custom-table.png` |
| 202 | Drawing | 2 point centerline | `drawing-two-point-linear-centerline` | `centerline.svg` | `ui-ref/icons/drawing-2-point-centerline.png` |
| 203 | Drawing | Edge to edge centerline | `drawing-two-edge-linear-centerline` | `centerline.svg` | `ui-ref/icons/drawing-edge-to-edge-centerline.png` |
| 204 | Drawing | 3 point circle centerline | `drawing-three-point-circular-centerline` | `centerline.svg` | `ui-ref/icons/drawing-3-point-circle-centerline.png` |
| 205 | Drawing | 2 point circle centerline | `drawing-two-point-circular-centerline` | `centerline.svg` | `ui-ref/icons/drawing-2-point-circle-centerline.png` |
| 206 | Drawing | Centermark | `drawing-centermark` | `centermark.svg` | `ui-ref/icons/drawing-centermark.png` |
| 207 | Drawing | Virtual sharp | `drawing-virtual-sharp` | **missing** | `ui-ref/icons/drawing-virtual-sharp.png` |
| 208 | Drawing | Line | `drawing-two-point-line` | `line.svg` | `ui-ref/icons/drawing-line.png` |
| 209 | Drawing | Center point circle | `sketch-circle-button` | `circle.svg` | `ui-ref/icons/drawing-center-point-circle.png` |
| 210 | Drawing | 3 point circle | `sketch-perimeter-circle-button` | `three-point-circle.svg` | `ui-ref/icons/drawing-3-point-circle.png` |
| 211 | Drawing | Tangent arc | `sketch-tangent-arc-button` | `tangent-arc.svg` | `ui-ref/icons/drawing-tangent-arc.png` |
| 212 | Drawing | Corner rectangle | `sketch-rectangle-button` | `corner-rectangle.svg` | `ui-ref/icons/drawing-corner-rectangle.png` |
| 213 | Drawing | Center point rectangle | `sketch-center-rectangle-button` | `center-rectangle.svg` | `ui-ref/icons/drawing-center-point-rectangle.png` |
| 214 | Drawing | Spline | `drawing-spline` | `spline.svg` | `ui-ref/icons/drawing-spline.png` |
| 215 | Drawing | Spline point | `drawing-spline-point` | `spline.svg` | `ui-ref/icons/drawing-spline-point.png` |
| 216 | Drawing | Sketch point | `sketch-point-button` | `point-sketch.svg` | `ui-ref/icons/drawing-sketch-point.png` |
| 217 | Drawing | Hatch region | `hatch-button` | **missing** | `ui-ref/icons/drawing-hatch-region.png` |
| 218 | Drawing | Trim | `sketch-trim-button` | `trim.svg` | `ui-ref/icons/drawing-trim.png` |
| 219 | Drawing | Horizontal | `sketch-horizontal-button` | `horizontal.svg` | `ui-ref/icons/drawing-horizontal.png` |
| 220 | Drawing | Vertical | `sketch-vertical-button` | `vertical.svg` | `ui-ref/icons/drawing-vertical.png` |
| 221 | Drawing | Coincident | `sketch-coincident-button` | `coincident.svg` | `ui-ref/icons/drawing-coincident.png` |
| 222 | Drawing | Perpendicular | `sketch-perpendicular-button` | `perpendicular.svg` | `ui-ref/icons/drawing-perpendicular.png` |
| 223 | Drawing | Tangent | `sketch-tangent-button` | `tangent.svg` | `ui-ref/icons/drawing-tangent.png` |
| 224 | Drawing | Parallel | `sketch-parallel-button` | `parallel.svg` | `ui-ref/icons/drawing-parallel.png` |
| 225 | Drawing | Insert DXF or DWG | `drawing-insert-dwg-dxf` | `import.svg` | `ui-ref/icons/drawing-insert-dxf-or-dwg.png` |
| 226 | Drawing | Insert image | `drawing-insert-image` | `image.svg` | `ui-ref/icons/drawing-insert-image.png` |
| 227 | Drawing | Format painter | `format-painter` | **missing** | `ui-ref/icons/drawing-format-painter.png` |
| 228 | Drawing | Inspection item | `inspection-items` | **missing** | `ui-ref/icons/drawing-inspection-item.png` |
| 229 | Drawing | Hide inspection items | `inspection-items-hide` | **missing** | `ui-ref/icons/drawing-hide-inspection-items.png` |
| 230 | Drawing | Sheets | `drawing-sheets` | `sheet.svg` | `ui-ref/icons/drawing-sheets.png` |
| 231 | Drawing | Inspection table | `inspection-table` | **missing** | `ui-ref/icons/drawing-inspection-table.png` |
| 232 | Drawing | Styles | `style-panel` | **missing** | `ui-ref/icons/drawing-styles.png` |
| 233 | Drawing | Drawing properties | `(help image)` | `settings.svg` | `ui-ref/icons/drawing-drawing-properties.png` |
| 234 | Drawing | MBD status | `model-based-definition` | **missing** | `ui-ref/icons/drawing-mbd-status.png` |
| 235 | Drawing | Measure | `measure-button` | `measure.svg` | `ui-ref/icons/drawing-measure.png` |
| 236 | Drawing | Update drawing | `drawing-update` | `drawing-update.svg` | `ui-ref/icons/drawing-update-drawing.png` |
| 237 | Part Studio panels | Appearance panel | `appearances-button` | `appearance.svg` | `ui-ref/icons/part-studio-panels-appearance-panel.png` |
| 238 | Part Studio panels | Configuration panel | `configuration-table-button` | `configuration.svg` | `ui-ref/icons/part-studio-panels-configuration-panel.png` |
| 239 | Part Studio panels | Custom tables | `featurescript-table-button` | **missing** | `ui-ref/icons/part-studio-panels-custom-tables.png` |
| 240 | Part Studio panels | Inspection table | `model-based-definition` | **missing** | `ui-ref/icons/part-studio-panels-inspection-table.png` |
| 241 | Part Studio panels | Sheet metal table and flat view | `sheet-metal-table-button` | `sheet-metal-table.svg` | `ui-ref/icons/part-studio-panels-sheet-metal-table-and-flat-view.png` |
| 242 | Part Studio panels | Variable table | `variable-table-button` | `variables-panel.svg` | `ui-ref/icons/part-studio-panels-variable-table.png` |
| 243 | Feature list | Features list toggle | `treelist-button` | **missing** | `ui-ref/icons/feature-list-features-list-toggle.png` |
| 244 | Feature list | Add folder | `new-folder-button` | `feature-folder.svg` | `ui-ref/icons/feature-list-add-folder.png` |
| 245 | Feature list | Pause regeneration | `pause-regen-button` | **missing** | `ui-ref/icons/feature-list-pause-regeneration.png` |
| 246 | Feature list | Regeneration time (stopwatch) | `performance-panel` | **missing** | `ui-ref/icons/feature-list-regeneration-time-stopwatch.png` |
| 247 | Feature list | Filter | `filter-results-button` | `filter.svg` | `ui-ref/icons/feature-list-filter.png` |
| 248 | Feature list | Collapsed caret | `collapsed` | `chevron-right.svg` | `ui-ref/icons/feature-list-collapsed-caret.png` |
| 249 | Feature list | Expanded caret | `expanded` | `chevron-down.svg` | `ui-ref/icons/feature-list-expanded-caret.png` |
| 250 | Feature list | Origin | `origin-button` | `origin.svg` | `ui-ref/icons/feature-list-origin.png` |
| 251 | Feature list | Default plane | `default-plane` | `plane.svg` | `ui-ref/icons/feature-list-default-plane.png` |
| 252 | Feature list | Sketch (list item) | `default-sketch` | `sketch.svg` | `ui-ref/icons/feature-list-sketch-list-item.png` |
| 253 | Feature list | Part (list item) | `default-part` | `part.svg` | `ui-ref/icons/feature-list-part-list-item.png` |
| 254 | Feature list | Surface (list item) | `default-surface` | `surface-body.svg` | `ui-ref/icons/feature-list-surface-list-item.png` |
| 255 | Feature list | Curve (list item) | `default-curve` | `wire-body.svg` | `ui-ref/icons/feature-list-curve-list-item.png` |
| 256 | Feature list | Composite part (list item) | `default-composite-part-button` | `composite-part.svg` | `ui-ref/icons/feature-list-composite-part-list-item.png` |
| 257 | Feature list | Mesh (list item) | `default-mesh` | **missing** | `ui-ref/icons/feature-list-mesh-list-item.png` |
| 258 | Feature list | Show (eye open) | `eye_open` | `eye.svg` | `ui-ref/icons/feature-list-show-eye-open.png` |
| 259 | Feature list | Hide (eye closed) | `eye_closed` | `eye-off.svg` | `ui-ref/icons/feature-list-hide-eye-closed.png` |
| 260 | Feature list | Error badge | `list-error-overlay` | `error.svg` | `ui-ref/icons/feature-list-error-badge.png` |
| 261 | Feature list | Warning badge | `warning-overlay` | `warning.svg` | `ui-ref/icons/feature-list-warning-badge.png` |
| 262 | Feature list | Under-defined sketch badge | `list-underdefined-overlay` | `under-defined.svg` | `ui-ref/icons/feature-list-under-defined-sketch-badge.png` |
| 263 | Dialog | Accept (green check) | `green-check` | `check.svg` | `ui-ref/icons/dialog-accept-green-check.png` |
| 264 | Dialog | Cancel (X) | `close-x` | `close.svg` | `ui-ref/icons/dialog-cancel-x.png` |
| 265 | Dialog | Flip direction | `flip-direction` | `flip.svg` | `ui-ref/icons/dialog-flip-direction.png` |
| 266 | Dialog | Flip direction (opposite) | `flip-direction-opposite` | **missing** | `ui-ref/icons/dialog-flip-direction-opposite.png` |
| 267 | Dialog | Help (?) | `help-button` | `info.svg` | `ui-ref/icons/dialog-help.png` |
| 268 | Dialog | Pause dialog | `pause-dialog` | **missing** | `ui-ref/icons/dialog-pause-dialog.png` |
| 269 | Dialog | Remove selection | `remove-button-x` | `close.svg` | `ui-ref/icons/dialog-remove-selection.png` |
| 270 | Navbar | Onshape menu (hamburger) | `hamburger-button` | `menu.svg` | `ui-ref/icons/navbar-onshape-menu-hamburger.png` |
| 271 | Navbar | Copy link | `link-icon` | `link.svg` | `ui-ref/icons/navbar-copy-link.png` |
| 272 | Navbar | Notifications | `navbar-notifications` | **missing** | `ui-ref/icons/navbar-notifications.png` |
| 273 | Navbar | App Store | `navbar-app-store` | **missing** | `ui-ref/icons/navbar-app-store.png` |
| 274 | Navbar | Learning Center | `navbar-learning-center` | **missing** | `ui-ref/icons/navbar-learning-center.png` |
| 275 | Navbar | Help | `navbar-help` | `info.svg` | `ui-ref/icons/navbar-help.png` |
| 276 | Navbar | Comments | `navbar-comments` | **missing** | `ui-ref/icons/navbar-comments.png` |
| 277 | Navbar | Action items | `navbar-action-items` | **missing** | `ui-ref/icons/navbar-action-items.png` |
| 278 | Tab bar | Insert new tab (+) | `plus-button` | `plus.svg` | `ui-ref/icons/tab-bar-insert-new-tab.png` |
| 279 | Tab bar | Tab manager | `tab-manager` | **missing** | `ui-ref/icons/tab-bar-tab-manager.png` |
| 280 | Tab bar | Search tabs | `tab-search-button` | `search.svg` | `ui-ref/icons/tab-bar-search-tabs.png` |
| 281 | Tab bar | Part Studio tab | `partstudio` | `part-studio.svg` | `ui-ref/icons/tab-bar-part-studio-tab.png` |
| 282 | Tab bar | Assembly tab | `default-assembly` | `assembly.svg` | `ui-ref/icons/tab-bar-assembly-tab.png` |
| 283 | Tab bar | Drawing tab | `drawing` | `drawing.svg` | `ui-ref/icons/tab-bar-drawing-tab.png` |
| 284 | Tab bar | Variable Studio tab | `variable-studio-element` | `variable-studio.svg` | `ui-ref/icons/tab-bar-variable-studio-tab.png` |
| 285 | Tab bar | Feature Studio tab | `feature-studio-element` | `feature-studio.svg` | `ui-ref/icons/tab-bar-feature-studio-tab.png` |
| 286 | Tab bar | Folder tab | `folder` | `folder.svg` | `ui-ref/icons/tab-bar-folder-tab.png` |
| 287 | Tab bar | Image tab | `image-element` | `image.svg` | `ui-ref/icons/tab-bar-image-tab.png` |
| 288 | Tab bar | PDF tab | `pdf-element` | `document.svg` | `ui-ref/icons/tab-bar-pdf-tab.png` |
| 289 | Document panel | Versions and history | `versions-history-panel` | `history.svg` | `ui-ref/icons/document-panel-versions-and-history.png` |
| 290 | Document panel | Compare history entries | `(help image)` | **missing** | `ui-ref/icons/document-panel-compare-history-entries.png` |
| 291 | Document panel | Repair | `repair` | **missing** | `ui-ref/icons/document-panel-repair.png` |
| 292 | Document panel | Hide/show merges | `mergelines` | `merge.svg` | `ui-ref/icons/document-panel-hide-show-merges.png` |
| 293 | Document panel | Hide/show automatic versions | `auto-version` | `version.svg` | `ui-ref/icons/document-panel-hide-show-automatic-versions.png` |
| 294 | Document panel | Create version | `create-version-button` | `version.svg` | `ui-ref/icons/document-panel-create-version.png` |
| 295 | Document panel | Release drafts | `create-release-button` | **missing** | `ui-ref/icons/document-panel-release-drafts.png` |
| 296 | Document panel | Comments | `navbar-comments` | **missing** | `ui-ref/icons/document-panel-comments.png` |
| 297 | Document panel | Filter | `workspace` | `filter.svg` | `ui-ref/icons/document-panel-filter.png` |
| 298 | Document panel | Tag entity | `tag-entity-button` | **missing** | `ui-ref/icons/document-panel-tag-entity.png` |
| 299 | Document panel | Markup | `markup-button` | **missing** | `ui-ref/icons/document-panel-markup.png` |
| 300 | Document panel | Browse for attachment | `image-element` | `image.svg` | `ui-ref/icons/document-panel-browse-for-attachment.png` |
| 301 | Document panel | Document notes | `notes-panel` | `note.svg` | `ui-ref/icons/document-panel-document-notes.png` |
| 302 | Document panel | Refresh notes | `refresh-button` | **missing** | `ui-ref/icons/document-panel-refresh-notes.png` |
| 303 | Document panel | Edit notes | `edit-button` | `edit.svg` | `ui-ref/icons/document-panel-edit-notes.png` |
| 304 | Document panel | Where used | `where-used` | **missing** | `ui-ref/icons/document-panel-where-used.png` |
| 305 | Document panel | Performance | `performance-panel` | **missing** | `ui-ref/icons/document-panel-performance.png` |
| 306 | Document panel | AI Advisor | `onshape-ai-advisor-button` | **missing** | `ui-ref/icons/document-panel-ai-advisor.png` |
| 307 | Document panel | Explore Onshape | `explore-onshape-circle` | **missing** | `ui-ref/icons/document-panel-explore-onshape.png` |
| 308 | Document panel | Action items | `navbar-action-items` | **missing** | `ui-ref/icons/document-panel-action-items.png` |
| 309 | Document panel | Tab manager | `tab-manager` | **missing** | `ui-ref/icons/document-panel-tab-manager.png` |
| 310 | Document panel | Properties | `properties-panel` | `properties.svg` | `ui-ref/icons/document-panel-properties.png` |
| 311 | Document panel | Version graph | `version-history-graph-button` | `version-graph.svg` | `ui-ref/icons/document-panel-version-graph.png` |
| 312 | Document panel | Branch | `branch-button` | `branch.svg` | `ui-ref/icons/document-panel-branch.png` |
| 313 | Document panel | Compare | `compare-versions-button` | **missing** | `ui-ref/icons/document-panel-compare.png` |
| 314 | View | Fit all | `(help image)` | `zoom-fit.svg` | `ui-ref/icons/view-fit-all.png` |
| 315 | View | Isometric | `(help image)` | `iso-view.svg` | `ui-ref/icons/view-isometric.png` |
| 316 | View | Front | `(help image)` | `front-view.svg` | `ui-ref/icons/view-front.png` |
| 317 | View | Back | `(help image)` | `back-view.svg` | `ui-ref/icons/view-back.png` |
| 318 | View | Top | `(help image)` | `top-view.svg` | `ui-ref/icons/view-top.png` |
| 319 | View | Bottom | `(help image)` | `bottom-view.svg` | `ui-ref/icons/view-bottom.png` |
| 320 | View | Left | `(help image)` | `left-view.svg` | `ui-ref/icons/view-left.png` |
| 321 | View | Right | `(help image)` | `right-view.svg` | `ui-ref/icons/view-right.png` |
| 322 | View | Toggle transparency | `(help image)` | `transparency.svg` | `ui-ref/icons/view-toggle-transparency.png` |
| 323 | View | Appearance | `render-appearances-panel` | `appearance.svg` | `ui-ref/icons/view-appearance.png` |
| 324 | View | View cube menu button | `view-cube-button` | `view-cube.svg` | `ui-ref/icons/view-view-cube-menu-button.png` |
| 325 | View | View cube rotate counter-clockwise | `viewcube-rotate-anti` | **missing** | `ui-ref/icons/view-view-cube-rotate-counter-clockwise.png` |
| 326 | View | View cube rotate clockwise | `viewcube-rotate-clock` | **missing** | `ui-ref/icons/view-view-cube-rotate-clockwise.png` |
| 327 | View | View cube up arrow | `viewcube-up-arrow` | **missing** | `ui-ref/icons/view-view-cube-up-arrow.png` |
| 328 | View | View cube down arrow | `viewcube-down-arrow` | **missing** | `ui-ref/icons/view-view-cube-down-arrow.png` |
| 329 | View | Section view | `lui-section-view` | `section-plane.svg` | `ui-ref/icons/view-section-view.png` |
| 330 | View | Zoom to fit | `lui-zoom-to-fit` | `zoom-fit.svg` | `ui-ref/icons/view-zoom-to-fit.png` |
| 331 | View | Zoom window | `lui-zoom-window` | `zoom-window.svg` | `ui-ref/icons/view-zoom-window.png` |
| 332 | View | Named views | `named-view-button` | **missing** | `ui-ref/icons/view-named-views.png` |
| 333 | View | Isolate | `isolate-button` | `isolate.svg` | `ui-ref/icons/view-isolate.png` |
| 334 | View | Mass properties | `mass-properties` | `mass-properties.svg` | `ui-ref/icons/view-mass-properties.png` |
| 335 | View | Analysis tools | `sketch-analysis-tools` | **missing** | `ui-ref/icons/view-analysis-tools.png` |
| 336 | Context menu | Edit | `edit-button` | `edit.svg` | `ui-ref/icons/context-menu-edit.png` |
| 337 | Context menu | Delete | `delete-button` | `delete.svg` | `ui-ref/icons/context-menu-delete.png` |
| 338 | Context menu | Copy | `copy-button` | `copy.svg` | `ui-ref/icons/context-menu-copy.png` |
| 339 | Context menu | Paste | `paste-command` | `paste.svg` | `ui-ref/icons/context-menu-paste.png` |
| 340 | Context menu | Export | `export-button` | `export.svg` | `ui-ref/icons/context-menu-export.png` |
| 341 | Context menu | Rename | `edit-button` | `rename.svg` | `ui-ref/icons/context-menu-rename.png` |
| 342 | Context menu | Settings | `settings` | `settings.svg` | `ui-ref/icons/context-menu-settings.png` |
| 343 | General | Copy link | `copy-document-URL` | `link.svg` | `ui-ref/icons/general-copy-link.png` |
| 344 | General | FeatureScript notices | `feature-studio-element` | `feature-studio.svg` | `ui-ref/icons/general-featurescript-notices.png` |
| 345 | General | Notifications | `navbar-notifications` | **missing** | `ui-ref/icons/general-notifications.png` |
| 346 | General | Action items | `action-items` | **missing** | `ui-ref/icons/general-action-items.png` |
| 347 | General | App Store | `global-sidebar-app-store` | **missing** | `ui-ref/icons/general-app-store.png` |
| 348 | General | Learning Center | `navbar-learning-center` | **missing** | `ui-ref/icons/general-learning-center.png` |
| 349 | General | Help Menu | `navbar-help` | **missing** | `ui-ref/icons/general-help-menu.png` |
| 350 | General | Measure details | `measure-button` | `measure.svg` | `ui-ref/icons/general-measure-details.png` |
| 351 | General | Analysis tools | `(help image)` | **missing** | `ui-ref/icons/general-analysis-tools.png` |
| 352 | General | Mass and section properties | `(help image)` | `mass-properties.svg` | `ui-ref/icons/general-mass-and-section-properties.png` |
| 353 | General | Add | `add-feature-type` | `plus.svg` | `ui-ref/icons/general-add.png` |

Rows: 353. Rows with a noshape icon: 300. Rows marked **missing**: 53. Several Onshape commands share one noshape file (for example, every linear dimension maps to `linear-dim.svg`); Onshape gives each its own icon, so these also need separate drawings.

noshape icons that no row maps to (check whether they are still needed, or map them by hand): `animate`, `balloon`, `camera`, `cancel`, `check-repeat`, `collinear`, `corner-relief`, `cube`, `drawing-bom`, `driven-dimension`, `export-step`, `export-stl`, `f-sphere`, `fix-instance`, `flip-circular`, `form`, `hidden-lines`, `hide`, `home`, `instance`, `interference`, `jog`, `joint`, `lock`, `material`, `minus`, `more`, `new-document`, `open-document`, `orthographic`, `perspective`, `point`, `polyline`, `recent`, `refold`, `rollback`, `save`, `select-box`, `shaded`, `shaded-edges`, `sheet-metal-rip`, `show`, `sub-assembly`, `suppress`, `three-point-rectangle`, `title-block`, `unfold`, `unlink`, `unlock`, `unsuppress`, `wireframe`

---

## 7. Gap list: noshape vs Onshape, by visual impact

Compared against `/tmp/ui/before/*.png` (a build of the committed tree, 07:33 today), `/tmp/lead/ui/*.png`, `/tmp/sk2`, `/tmp/drw3`, `/tmp/feat` and `/tmp/fs`. All of these show the **dark, purple-accent theme**. The working-tree `crates/app/src/theme.rs` already carries the Onshape light-token palette, so items 1-2 may be partly done by the time you read this; verify them against a fresh screenshot. noshape measurements below were taken from the 1440×900 window in those screenshots, at 1:1.

| # | Area | noshape now | Onshape target (spec section) |
|---|---|---|---|
| 1 | **Theme / palette** | dark chrome (panels and toolbar `#15181d`, title bar `#14161b`), purple accent `#8b7cff`, viewport gradient `#2a2e38` to `#181c22` (sampled) | white chrome, **`#1651b0`** accent, `#333` text, `#eaeaea` navbar, `#d7d7d7` tab bar, `#fafafa` rails (§1.1-1.3). Remove every purple. |
| 2 | **Viewport background and planes** | dark navy gradient; planes as saturated purple fill and outline with small purple labels | **flat `#ffffff`**; planes `#cedbe5` at 20% fill with a **2 px `#1d60aa` at 35%** outline; labels **20 px `#1651b0`** inside the top-left corner; 16:9, sized to 70% of the viewport height (§1.8, §5.4) |
| 3 | **Icons render as one-color masks** | toolbar and list icons are light-gray silhouettes (GPUI `svg()` masks); 24 px grid, 30° isometric, about 16 px on screen | **full-color** icons (`#333` outline, white faces, `#999` result faces, `#1651b0` accents), **20 px** in 30 px buttons, oblique projection; render through `img()` or layered masks (§6.2-6.3). Every row of the inventory (§6.5) needs a redraw against its reference. |
| 4 | **Document top bar** | 33 px title bar with a purple app logo, "noshape ▾", a small doc name, and undo/redo/flag at the far right | **40 px `#eaeaea` navbar**: ☰ menu, **document name 22 px / 600**, workspace name "Main" 16 px `#999`, link icon; a Help ▾ at the right. **Undo/Redo move to the start of the toolbar.** No flag icon (§3.1). |
| 5 | **Left document-panel rail missing** | none | a **40 px `#fafafa` rail** (1 px `#d7d7d7` right border) from the navbar to the bottom: Versions & history, Create version, Comments/Notes, Where used, Performance; 31 px buttons; active = 4 px blue left bar (§3.3) |
| 6 | **Feature toolbar** | 40 px tall, dark, icons about 16 px with tight 8-10 px gaps, carets as tiny gray "⌄", 1 px separators; no Search tools; first tool is a bare pencil | **36 px**, white; 30×30 buttons with 20 px icons at a 34 px pitch; **2 px `#d7d7d7` separators** 26 px tall; caret cell 15 px with a `#333` chevron; **"✎ Sketch" text button**; **Search tools… box** (`#eee`, 32 px tall, about 160 px, `alt/⌥ c` chips) at the right; overflow chevron when narrow (§3.2) |
| 7 | **Feature list header and filter** | "FEATURES (3)" in small uppercase gray letter-spaced text; no buttons; no filter | "**Features (3)**" 14 px / 600 `#333`, plus Add-folder, Pause and Stopwatch buttons at the right; a **filter row** (funnel plus a 28 px "*Filter by name or type*" box) (§3.4) |
| 8 | **Default geometry** | Origin/Top/Front/Right listed flat at the top, then a gap | a collapsible "**▾ Default geometry**" folder with children indented 22 px and a 1 px guide line; plane and origin icons (§3.4) |
| 9 | **Feature rows and selection** | 25-27 px rows; selected/edited = purple 1 px outline box with a purple dot; Sketch rows in blue text | **26 px rows**, 17 px icons at 0.8 opacity; hover `#def1fb`; selected `#b2ddf6` (fill, no outline); edited = `#b2ddf6` + **600 weight**; text always `#333` (no blue rows); rolled back = italic `#ccc` (§3.4, §1.3) |
| 10 | **Rollback bar** | a 2-3 px **purple** line (`#8b7cff`) | a **7 px gray bar** (`#b8b8b8`) with 5 px side margins; drag target `#b2ddf6` (§3.4) |
| 11 | **Panel widths** | feature panel 249 px | about **205 px** (resizable) (§3.4) |
| 12 | **Parts list** | "PARTS (1)" uppercase heading, a square color swatch per part | "**▾ Parts (1)**" row (caret, 13.33 px), part rows with the `default-part` outline icon, indented 22 px (§3.4) |
| 13 | **Bottom tab bar** | 32 px, dark, a pill-shaped active tab with a purple tint, small 12 px icons, "+" at the left | **30 px `#d7d7d7`** bar; tabs `#eaeaea`, 27 px tall, **radius 0 0 5 5**, 60-200 px wide, 2 px gaps, light shadow; **active tab white, 28 px, with a 2 px `#1651b0` bottom bar**; 20 px element icons; tab-search cell (40 px, under the rail) and a "+" cell (34 px) at the left (§3.5) |
| 14 | **Feature dialog frame** | 316 px wide, dark, radius about 4, a 1 px border; the header has a small ✓ and × as plain glyphs | **220 px**, white at 95%, **no border, no radius**, shadow `0 5px 10px rgba(0,0,0,.2)`; **28 px header**: title 13 px / 500 (red while invalid), a **28×28 solid green `#009400` ✓ block** and a red **✕** (§3.9) |
| 15 | **Dialog tabs** | segmented pills (a purple filled "Solid" / "New" chip) | **underline tabs**: text `#666`, selected `#1651b0` with a **2 px blue underline**; 26 px rows separated by 1 px `#e8e8e8` (§3.9) |
| 16 | **Dialog value fields** | boxed dark inputs with a left label column (about 100 px) | **underline fields**: label `#666` 13.33 px, right-aligned value, 1 px `#d7d7d7` underline (blue on focus), 26 px rows, trailing 26 px icon button (±, flip) (§3.9, §4.2) |
| 17 | **Selection box** | a purple-outlined dark box with a chip "Sketch 1 region ×" and the label above the box | the label is the **placeholder inside the box** (12 px `#666`); **focused box filled `#def1fb` with a `#1651b0` border**; entries are 24 px text rows with a hover ×, not chips (§3.9, §4.4) |
| 18 | **Dialog checkboxes and sections** | rows separated by divider lines; no disclosure chevrons | **no dividers**; collapsible "▸ ☐ Direction" rows with a 16 px chevron and 20 px child indent; 12 px blue checkboxes (§3.9, §4.3) |
| 19 | **Dialog footer** | a "☐ Final" row | a **preview slider + `?` help** footer (the legacy Final footer is `#eee` with a top border) (§3.9) |
| 20 | **Dropdown in dialog** | a boxed select with a chevron and a separate small flip glyph | an underline select with a black ▾, plus a 26×26 **flip-direction** icon button in the same row (§3.9) |
| 21 | **View cube missing** | a column of 8 tiny icons at the top-right plus a 2-icon chip | a **150 px view cube** 15 px from the top-right: white cube with translucent `#cedbe5` edges, 4 nudge arrows, 2 rotate arrows, XYZ triad, and the **view-mode cube icon + ▾ menu** below it (§3.7). Move the current render/visibility toggles into that menu. |
| 22 | **Right panel rail** | none (render controls at the top-right) | a **vertically centered strip** of 32 px buttons at the right edge: Appearance, Configurations, Custom tables, Variable table (§3.6) |
| 23 | **Bottom-right quick tools** | none | Measure, Analysis, Mass properties glyph buttons above the tab bar at the right (§3.8) |
| 24 | **Selection colors in the viewport** | selected faces filled salmon/peach (sampled `#e6a889`); hover similar | hover = **outline only** `#ffc685`; selected = **`rgba(255,169,10,.8)`** (reads as `#ffc107` yellow-orange) with a `#ffa90a` boundary; whole part `rgb(255,192,76)` (§5.3) |
| 25 | **Default part color** | light lavender-gray | Onshape's 8-color cycle; for a single default use near-white/gray (`#f3f3f3` lit) or light steel blue (`#a3c4dc` base); black 1 px edges (§1.8) |
| 26 | **Sketch mode chrome** | a purple "✓ Finish sketch" button at the right of the sketch toolbar; floating "Edit text" panel styled like the app | **no Finish button in the toolbar**: the sketch closes from its dialog's green ✓. The sketch toolbar has the same metrics as the feature toolbar (with Search tools at the right). Option panels use the dialog frame (§3.10). |
| 27 | **Sketch colors and glyphs** | blue text outline with orange dotted handles; constraint glyphs in dark-gray filled boxes | under-defined **`#0000d0`**, fully defined black, 2 px lines, 6 px points; constraint glyphs in **white boxes** (hover `#fcda7f`, selected `#fab601`); dimension text 14 px black with solid narrow arrowheads (§1.9, §5.1-5.2) |
| 28 | **Toasts / hints** | dark pill with blue text ("That entity can't be used…") at the bottom-center | **alert** at the top-center: info `#def1fb` fill, `#b2ddf6` 1 px border, `#094174` text, radius 4, close × (§4.9) |
| 29 | **Typography** | Roboto (good), but many labels at 11-12 px and uppercase section labels | the type scale in §2.2: 13.33 px rows, 13 px / 500 dialog titles, 12 px menus and tooltips, 14 px toolbar text; **no uppercase** except user-named folders |
| 30 | **Context menus and tooltips** | dark popovers | white menus (1 px `#ccc`, radius 4, 12 px text, 22 px rows, hover `#def1fb`); tooltips **`#d0dbe5`** 12 px / 500, square corners (§4.6-4.7) |
| 31 | **Drawing workspace** | the sheet on a dark gray surround; sheet tabs ("Sheet1 · Assembly · Details") and a status line ("Sheet1 · A3 · 1:1 x y") at the bottom; "Insert view" and "Export" text buttons | **`#dcdcdc` surround**, white sheet with no shadow; a **Sheets panel on the left** (blue "Sheets (1)" header, sort tabs, sheet/view tree); no status line; an icon toolbar with the same metrics as the Part Studio (§3.11) |
| 32 | **Assembly** | (same chrome issues as above) | "Instances (n)" list with a Mates folder, the mate HUD, the assembly toolbar order (§3.11) |
| 33 | **Start window** | a dark document browser | out of scope for Onshape parity (cloud documents page), but it should use the same light palette and a document-list style (rows hover `#def1fb`, selected `#b2ddf6`, 1 px `#d7d7d7` dividers) |

### 7.1 Coverage gaps in this spec

- **Live client not observed.** Hover timings, animation, exact dropdown widths and some icon positions were inferred from CSS and from screenshots captured at 110-125% (scale-corrected). The CSS rules are authoritative where they exist.
- **Default part colors** are approximated from rendered pixels (the base material values come from the server).
- **Navbar right-cluster sizes** (Share, help, avatar) are estimated; noshape drops most of these anyway.
- **Right panel rail** button size (about 32 px) and the opened panel width (about 300-350 px) are screenshot estimates.
- **Toolbar overflow behavior** and the exact tool-search popover layout are only partly specified by the CSS.
- The **view cube** geometry ratios come from [GFX]; the arrow shapes are described from screenshots.

### 7.2 Corrections to the working-tree `crates/app/src/theme.rs` (as of 07:25)

The palette is mostly right. Change these values:

- `accent_hover: #467bb6`: Onshape's primary-button hover is **`#769fcc`** (`--os-button-fill-primary--hover`). `#467bb6` is `--os-hover-senary--static`, which is used only for the demo-card arrows.
- `panel_active: #d7d7d7`: a selected/toggled icon button is **`#eaeaea` plus inset shadow `0 1px 1px rgba(0,0,0,.25)`**; a running toolbar tool is **`#b2ddf6`** with inset `0 1px 1px rgba(0,0,0,.2)`. `#d7d7d7` is only the hover color inside a split group.
- `panel_hover: #eaeaea`: correct for icon buttons, but **toolbar** tools hover at `rgba(0,0,0,.05)` (about `#f2f2f2`).
- `info: #1651b0`: alerts use info **`#def1fb` fill / `#b2ddf6` border / `#094174` text**.
- `t.input = #cccccc`: dialog fields are underline-style (`#d7d7d7`); boxed form fields are `#aaaaaa`; the focus ring is `#3e94ff`, not the accent.
- `viewport::PART: #b5b5b5`: see §1.8 (8-color cycle; a near-white or light steel blue default).
- `viewport::SKETCH_EXTERNAL: #672f8f`: this is a noshape convention. Onshape has no distinct "used" color in the tokens; used/projected entities take the normal constraint-state colors, and in-context references are magenta `rgba(212,3,249,.3)`.
- Missing tokens worth adding: `tooltip_text #333`, `rail_bg #fafafa`, `navbar_bg #eaeaea`, `toolbar_divider #d7d7d7`, `focus #3e94ff`, `qlv_active_fill #def1fb`, `rollback #b8b8b8`, `tab_shadow rgba(0,0,0,.12)`, `dialog_bg rgba(255,255,255,.95)`, `shadow_primary 0 5px 10px rgba(0,0,0,.2)`.
