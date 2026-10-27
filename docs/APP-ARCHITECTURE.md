# The app's shape (lead, 2026-10-27)

The target structure of noshape's GPUI app, modelled on Zed (zed-industries/zed, read 10-27: 251 crates, a ~22k-line
binary). Rule of thumb from it: **every user-facing area is a crate with an `init(cx)`; the workspace knows traits and
registries, never features; crates talk through shared action types, not through each other.** docs/CRATES.md has the
model side and the progress log; this file is the app side.

## How Zed does it (the parts we copy)

- **`workspace` crate**: the window, panes, tabs, docks, modals, notifications. It defines the traits features
  implement: `Item` (a tab: title, icon, tooltip, dirty, save/save-as/reload, split, navigation),
  `SerializableItem` (persist and restore across restarts), `ProjectItem` (opened from a path),
  `Panel` (docked panel: position, size, icon, toggle action), `ModalView`. It keeps registries
  (`register_project_item::<T>(cx)`, `register_serializable_item::<T>(cx)`) and an `AppState` with the app-wide handles.
  It depends on no feature crate.
- **Feature crates** (`editor`, `terminal_view`, `project_panel`, `git_ui`, `debugger_ui`, …): each has
  `pub fn init(cx: &mut App)` that registers its actions, item kinds, panels and settings.
- **`zed_actions`**: a tiny crate of action types several crates need ("open settings", "open recent", …). A crate
  dispatches one; the crate that owns the behaviour handles it (registered in its `init`). No upward dependency, no
  global callbacks.
- **`settings`**: each crate registers its typed settings; one store, one file.
- **`ui` / `theme`**: the component library and theming, below everything.
- **The binary** (`crates/zed`): `main.rs` builds `AppState`, calls every crate's `init` in order, opens windows;
  `zed.rs` adds panels to a new workspace and registers workspace-level actions; `app_menus.rs` builds the menu bar.

## noshape's target crates

| Layer | Crate | Contents | Today |
|---|---|---|---|
| ui | `noshape-app-core` (later: `noshape-ui` + `noshape-viewport`) | theme, assets/icons, i18n, base widgets, menus, inputs, 3D viewport + navigation + view cube, shared dialogs | exists (17k) |
| ui | **`noshape-actions`** | action types several crates dispatch: OpenDocument, OpenVersion, ActivateTab, OpenPreferences(page), ShowReleaseDialog(items), CreateVersion, Import, Export, ShowHistory, … | new |
| shell | **`noshape-workspace`** | the document window: Onshape's tab bar, tab manager, panels/docks framework, the **`Studio` trait** (one per tab kind: create the view for a tab, title/icon/thumbnail, toolbar, context menu, undo/save integration, restore) + `register_studio::<T>(cx)`; `PreferencesPage` registry; app-wide `AppState` (documents, DocModel handles, preferences store) | in the binary (workspace.rs, tab_manager.rs, doc_panels.rs, doc_dialogs.rs, files.rs) |
| studios | `noshape-part-studio-ui` (incl. sketch UI, feature dialog), `noshape-assembly-ui`, `noshape-drawing-ui`, `noshape-variable-studio-ui`, `noshape-feature-studio-ui`, material library and file tabs | each implements `Studio` for its tab kind and registers in `init(cx)` | drawing-ui, assembly-ui exist; Part Studio + the small studios being split (10-27) |
| document features | `noshape-history-ui` (versions, compare, merge), `noshape-release-ui` (release management, revisions), `noshape-start-ui` (start window, documents library), `noshape-preferences-ui` (the window; pages registered by the crates that own them) | each with `init(cx)`, handling its actions | being split (10-27) |
| binary | `noshape` | `main.rs`: AppState, every `init(cx)` in order, open the start window or documents; app menus; smoke mode. Target ~5–10k lines | 71k today |

## Rules

1. A crate never depends on a crate above it. Studios don't depend on each other except through `noshape-actions`
   (e.g. "open the drawing creator for this part" is an action the drawing studio handles).
2. Cross-crate calls are **actions** (`noshape-actions`) handled by the owning crate's `init`, or **registries** in
   `noshape-workspace` (studios, panels, preference pages). The OnceLock hooks of drawing-ui / assembly-ui / the
   10-27 splits are the stopgap: new code uses actions; hooks migrate when their crate is next touched.
3. Each crate has `pub fn init(cx: &mut App)`; `main.rs` calls them. Nothing registers itself implicitly.
4. A crate over ~25k lines gets split along its natural seams (Part Studio: dialogs / feature list / sketch UI /
   viewport tools) before it tangles.
5. Edit-loop target (measured in docs/CRATES.md): an edit in a studio rebuilds that studio crate plus links the binary
   in well under 30 s warm.

## Steps

1. (10-27, running) Part Studio UI out of the binary (psui); Variable Studio, Feature Studio, history, release, start,
   preferences out of the binary (lead subagent). Binary → ~10k.
2. `noshape-actions` + `noshape-workspace` with the `Studio` trait and registries; every studio registers in `init`;
   the workspace stops naming studios; hooks become actions.
3. Split app-core into `noshape-ui` and `noshape-viewport` if it keeps growing; split the Part Studio crate along its
   seams if it passes ~25k.
