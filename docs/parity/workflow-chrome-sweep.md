# Workflow: Part Studio chrome sweep (ui, from 2026-10-22)

The Part Studio chrome compared with the live captures in /root/onshape-research/live, area by area. Areas: the view
cube and its menu, the right-side panels (Appearances, Mass properties, Measure, Section view, Named views, analysis
tools), the toolbar overflow, the top bar menus, and the tab bar menus. One row per difference; fixes land in small
commits. Status: ✅ fixed (commit), ✗ open, ? asked live (REQUESTS.md).

Method: our screenshot (Xvfb :101, 1919×1079) next to the live screenshot of the same state; positions, colours and
sizes are measured from pixels at 1×, texts from the dom/ captures.

## 1. View cube and Camera and render options menu (live part-studio/21, 50–54; dom/menu-camera-render.txt)

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| VC1 | Items, groups, ✓ marks, separators, icons of the camera menu | 15 rows in 7 groups (dom) | the same | ✅ matches |
| VC2 | Widget position | cube's dark edge y 140; ▾ button box y 218–246; menu border top 246, left 1681, right 1904 | everything 1 px lower; menu 1 px further left | ✅ widget top 14 px (was 15); menu anchored 1 px further right |
| VC3 | ▾ caret next to the cube icon | 8×4 CSS caret `#333` (dark core 6 px wide) | 9 px `ui-caret-down` glyph (core 3 px) | ✅ `drawing-tool-caret` 12 px `#333` |
| VC4 | ▾ button while its menu is open | `#e5e5e5` (229) | stays at the hover `#eaeaea` | ✅ pressed `#e5e5e5` while the menu is open (`os_menu::is_open_menu`) |
| VC5 | Camera menu border | 1 px `#d7d7d7` (215) | `#ccc` (204), the right-click menu colour | ✅ `#d7d7d7` for the camera menu and its submenus |
| VC6 | Submenus (Shaded…, Hidden edges…, Tangent edges…) | ✓ at +14, text at +29 from the border, like the main menu | ✓ +17, text +36 (right-click menu insets) | ✅ submenus inherit the camera menu's insets |
| VC7 | Menu font | Flama 12 px `#555` | our UI font 12 px | ✗ app-wide font (Open Sans / Flama not bundled; see theme-layout.md) |
| VC8 | Assembly camera menu | (not compared yet) | a shorter dropdown (`assembly_view::render_camera_menu`, chevron caret) | ✗ to compare (assembly) |

## Feature-row tooltips (lead 10-22; live behaviours "Feature-row hover tooltips: Fillet / Chamfer / Shell")

| # | What | Live | Ours | Status |
|---|---|---|---|---|
| RT1 | Parents after "<-" | only feature-level inputs: sketches; a Derived's source studio. Fillet, Chamfer, Shell, Boolean: the name alone | parents through part references too ("Boolean 1 <- Extrude 1", "Fillet 1 <- Extrude 1") | ✅ sketches only, plus "Derived 1 <- <source studio>" |
| RT2 | Sketch row | "Sketch 1 (Hidden) is not fully defined" (hidden + under-defined) | "Sketch 1" | ✅ name + " (Hidden)" + " is not fully defined"; ? the other combinations (REQUESTS "ui (10-22 d)") |
