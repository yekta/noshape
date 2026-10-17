# GPUI Kit 0.7.3 cheat sheet (for the noshape CAD UI)

Checked against the **published** crates in `~/.cargo/registry` (`gpui-kit 0.7.3`,
`gpui-component 0.7.3`, `gpui-base 0.7.3`, `gpui-kit-assets 0.7.3`, `gpui-pre 0.3.9`).
The git clone at `/tmp/gpui-kit` is **ahead of 0.7.3**. APIs that exist only in the
clone are marked *(not in 0.7.3)*. The snippets were adapted from
`crates/story/src/stories/*` and `examples/*`. A file containing nearly all of them
compiles with `cargo check` against 0.7.3 (scratch crate `/tmp/gk-check`).

## 0. Imports: read this first

```rust
use gpui_kit::*;                              // all of GPUI, plus the kit `actions!` macro
use gpui_kit::prelude::FluentBuilder as _;    // .when / .when_some / .map. NOT included in `gpui_kit::*`
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, StyledExt as _,
    WindowExt as _, Icon, IconName, IndexPath, Theme, ThemeMode, TitleBar, h_flex, v_flex,
    button::{Button, ButtonVariants as _},
};
```
- **Don't** write `use gpui_kit::component::*;` next to `use gpui_kit::*;`. Both globs
  export `Size` and `Edges` as different types, so using either name fails with E0659.
- Capability methods come from traits: `.primary()/.ghost()` need `ButtonVariants`,
  `.small()/.xsmall()/.large()/.with_size()` need `Sizable`, `.disabled()` needs
  `Disableable` (some components also have an inherent `disabled`), `.selected()` needs
  `Selectable`, `cx.theme()` needs `ActiveTheme`, and `window.open_dialog/push_notification`
  need `WindowExt`.
- `#[derive(Action)]` expands to `gpui::…` paths. These resolve through a hidden
  `gpui_kit::gpui` alias, which `use gpui_kit::*;` brings into scope. Keep that glob in
  every module that derives `Action`, `IntoElement` or `Render`.

---

## 1. App and window bootstrap

`gpui_kit::application() -> Application` re-exports `gpui_platform::application`.
`Application::with_assets(self, impl AssetSource) -> Self`, then `.run(|cx: &mut App| …)`.
`gpui_kit::init(cx: &mut App)` initializes theme, Root, inputs, menus and so on. Call it first.
```rust
pub fn open_window<V: Render>(options: WindowOptions, cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Result<(AnyWindowHandle, Entity<V>)>
```
`open_window` wraps your view in `gpui_kit::component::Root` (= `gpui_base::Root`). Root
draws the dialog, sheet, notification, tooltip and menu layers, sets
`window.set_rem_size(theme.font_size)`, applies the theme font/background, and wraps
everything in `WindowBorder` (Linux client-side shadow and resize edges). Return your
view, **not** another Root. Inside async code, call it from within `cx.update(|cx| …)`.

`WindowOptions` fields that matter (gpui `platform.rs`): `window_bounds: Option<WindowBounds>`,
`titlebar: Option<TitlebarOptions>`, `window_min_size: Option<Size<Pixels>>`,
`window_decorations: Option<WindowDecorations /* Server | Client */>`, `kind: WindowKind`,
`is_resizable`, `is_minimizable`, `focus`, `show`, `app_id: Option<String>`,
`window_background: WindowBackgroundAppearance`, `app_owns_titlebar_drag: bool`,
`icon: Option<Arc<image::RgbaImage>>`.
`TitlebarOptions { title: Option<SharedString>, appears_transparent: bool, traffic_light_position: Option<Point<Pixels>> }`.
`Bounds::centered(display: Option<DisplayId>, size: Size<Pixels>, cx: &App)`, and
`WindowBounds::centered(size, cx)` gives `Windowed(...)`.

```rust
fn main() {
    gpui_kit::application().with_assets(AppAssets).run(|cx| {
        gpui_kit::init(cx);
        bind_keys(cx);                                   // section 3
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(1440.), px(900.)), cx))),
            window_min_size: Some(size(px(800.), px(500.))),
            #[cfg(target_os = "linux")]
            window_decorations: Some(WindowDecorations::Client),
            ..TitleBar::window_options()                 // transparent titlebar + app-owned drag
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            let ws = cx.new(|cx| Workspace::new(window, cx));
            let fh = ws.read(cx).focus_handle.clone();
            window.defer(cx, move |window, cx| fh.focus(window, cx)); // so actions have a target
            ws
        }).expect("open window");
    });
}
```
Title at runtime: `window.set_window_title(&str)`. Quit: `cx.quit()`.

### TitleBar (`gpui_kit::component::TitleBar`)
- `TitleBar::new()`. It implements `ParentElement + Styled`. Height `TITLE_BAR_HEIGHT = px(34.)`.
  Left padding is 80px on macOS (traffic lights) and 12px elsewhere. It draws the Windows
  and Linux min/max/close controls itself.
- `TitleBar::window_options() -> WindowOptions` gives `titlebar: Some(title_bar_options())`
  and `app_owns_titlebar_drag: true`. `TitleBar::title_bar_options() -> TitlebarOptions`
  gives `appears_transparent: true` and traffic lights at (9, 9).
- `.on_close_window(impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)`. Linux only.
  Without it, X calls `window.remove_window()`.
- Put the TitleBar as the first child of your root `v_flex()`. It only looks right when
  the window uses `TitleBar::window_options()`.
```rust
v_flex().size_full()
    .child(TitleBar::new().child(h_flex().w_full().pr_2().justify_between()
        .child("noshape — Part1").child(Button::new("share").ghost().small().label("Share"))))
    .child(body)
```
Window decorations: `gpui_kit::component::{window_border, WindowBorder, window_paddings}`.
Root already applies `window_border()`, so you rarely need these.

---

## 2. Theme

Read it with `cx.theme() -> &Theme` (trait `ActiveTheme`, implemented for `App`, so it
also works on `Context<T>`). `Theme: Deref<Target = ThemeColor>`, so color fields are
read directly, e.g. `cx.theme().border`.

**Color fields (`Hsla`) on `ThemeColor`:**
`background foreground border ring selection caret overlay window_border transparent`(Theme) ·
`primary primary_foreground primary_hover primary_active` · `secondary(_foreground|_hover|_active)` ·
`accent accent_foreground` · `muted muted_foreground` · `popover popover_foreground` ·
`danger(_foreground|_hover|_active)` `warning(…)` `success(…)` `info(…)` ·
`input` (input border) · `link link_hover link_active` ·
`button button_foreground button_hover button_active` plus `button_{primary,secondary,danger,warning,success,info}{,_foreground,_hover,_active}` ·
`list list_active list_active_border list_even list_head list_hover` ·
`table table_active table_active_border table_even table_head table_head_foreground table_foot table_foot_foreground table_hover table_row_border` ·
`tab tab_active tab_active_foreground tab_bar tab_bar_segmented tab_foreground` ·
`sidebar sidebar_accent sidebar_accent_foreground sidebar_border sidebar_foreground sidebar_primary sidebar_primary_foreground` ·
`title_bar title_bar_border status_bar status_bar_border` · `scrollbar scrollbar_thumb scrollbar_thumb_hover` ·
`slider_bar slider_thumb switch switch_thumb progress_bar skeleton accordion group_box group_box_foreground description_list_label description_list_label_foreground` ·
`drag_border drop_target` · `chart_1..chart_5 chart_bullish chart_bearish chart_grid` ·
`red red_light green green_light blue blue_light yellow yellow_light magenta magenta_light cyan cyan_light`.
There are **no** `surface` or `destructive` fields, although `skills/…/usage.md` says there
are. Use `popover`/`danger`, or `cx.theme().color_tokens().surface` / `.destructive`.

Other `Theme` fields: `mode: ThemeMode {Light, Dark}`, `font_family`, `font_size: Pixels`
(default 16; Root uses it as rem), `mono_font_family`, `mono_font_size` (13), `radius`
(6), `radius_lg` (8), `shadow: bool`, `focus_ring: bool`, `scrollbar_mode`, `list`, `sheet`,
`tokens`, `light_theme/dark_theme: Rc<ThemeConfig>`. Helpers: `is_dark()`, `radius_full()`,
`theme_name()`.

```rust
// Dark mode: loads the registered default dark ThemeConfig. This REPLACES colors.
Theme::change(ThemeMode::Dark, None, cx);                    // (mode, Option<&mut Window>, &mut App)
Theme::sync_system_appearance(Some(window), cx);             // follow the OS
// Edit fields. Use `update`, not `global_mut`: it re-syncs tokens and the base projection
// and refreshes windows.
Theme::update(cx, |t| {
    t.font_size = px(13.);           // compact CAD density; all rem-based text_* sizes shrink
    t.radius = px(4.);
    t.font_family = "Inter".into();
    t.primary = rgb(0x3b82f6).into();
});
```
Order matters: switching mode reloads colors. Set colors in a separate `Theme::update`
**after** the mode switch. `font_size`, `radius` and fonts survive the switch unless the
config sets them.

**Custom theme as JSON.** Use the same format as `/tmp/gpui-kit/themes/*.json`, whose keys
look like `"background"`, `"primary.background"`, `"primary.foreground"`,
`"primary.hover.background"`, `"list.active.background"`, `"tab_bar.background"`,
`"title_bar.background"`, `"input.border"`, `"base.blue"` and so on. Top-level keys per
theme are `name`, `mode`, `font.size`, `font.family`, `mono_font.family`,
`mono_font.size`, `radius`, `radius.lg`, `shadow` and `colors{…}`.
```rust
let reg = ThemeRegistry::global_mut(cx);
reg.load_themes_from_str(include_str!("../themes/noshape.json"))?;   // {"name":..,"themes":[{..}]}
let cfg: Rc<ThemeConfig> = ThemeRegistry::global(cx).themes()["Noshape Dark"].clone();
Theme::update(cx, |t| t.apply_config(&cfg));    // installs it as light/dark_theme for its mode
// Live-reloading a directory: ThemeRegistry::watch_dir(PathBuf, cx, |cx| {...})
```
**Font size scale.** GPUI's rem-based helpers: `text_xs` 0.75, `text_sm` 0.875,
`text_base` 1, `text_lg` 1.125, `text_xl` 1.25, `text_2xl` 1.5, `text_3xl` 1.875 rem.
Rem = `theme.font_size`. Use `.text_size(px(11.))` for absolute sizes. Component sizes
are `Size::{XSmall, Small, Medium(default), Large, Size(Pixels)}`.

---

## 3. Actions, keybindings and focus (GPUI)

```rust
actions!(noshape, [Extrude, Undo, Redo, DeleteSelection]);   // kit macro: unit structs (Clone, PartialEq, Default, Debug, Action)

#[derive(Clone, PartialEq, serde::Deserialize, Action)]       // action with data
#[action(namespace = noshape, no_json)]                        // no_json: no JsonSchema needed
pub struct SelectTool(pub usize);

fn bind_keys(cx: &mut App) {
    cx.bind_keys([                       // fn bind_keys(&mut self, impl IntoIterator<Item = KeyBinding>)
        KeyBinding::new("e", Extrude, Some("Workspace && !Input")),   // panics on a parse error
        KeyBinding::new("secondary-z", Undo, Some("Workspace")),      // cmd on macOS, ctrl elsewhere
        KeyBinding::new("secondary-shift-z", Redo, Some("Workspace")),
        KeyBinding::new("delete", DeleteSelection, Some("Workspace && !Input")),
        KeyBinding::new("l", SelectTool(1), Some("Workspace && !Input")),
    ]);
    cx.on_action(|_: &Undo, cx: &mut App| { /* global fallback handler */ });
}
```
- `KeyBinding::new<A: Action>(keystrokes: &str, action: A, context: Option<&str>)`.
- Keystroke grammar: `[secondary-][ctrl-][alt-][shift-][cmd-|super-|win-][fn-]key`.
  Separate chords with spaces (`"g g"`). Key names include `escape enter tab space backspace delete up down left right home end pageup pagedown f1…`.
  `secondary-` means cmd on macOS and ctrl elsewhere. Prefer it to `#[cfg]` pairs (the
  stories use `#[cfg(target_os = "macos")]` with `cmd-c`/`ctrl-c`).
- Context predicates: `Ident`, `a && b`, `a || b`, `!a`, `Parent > Child`, `key == value`.
  `!Input` is true when no ancestor context in the focused path is `Input`. Every text
  input uses key context `"Input"`. **A bare letter binding steals typing from inputs**,
  so guard letter shortcuts with `&& !Input`.
- Elements: `.key_context("Workspace")`, `.track_focus(&focus_handle)`,
  `.on_action(cx.listener(Self::on_extrude))`, `.capture_action(...)` and
  `.on_key_down(|ev: &KeyDownEvent, window, cx| …)`. `on_action` takes
  `impl Fn(&A, &mut Window, &mut App)`, which is what `cx.listener` returns. Actions
  dispatch from the **focused** element up through its ancestors. Nothing focused means
  only `cx.on_action` global handlers run, so focus your root on startup (see section 1).
- Focus: `cx.focus_handle() -> FocusHandle`, `handle.focus(window, cx)` (note: takes cx),
  `handle.is_focused(window)`, `handle.contains_focused(window, cx)`,
  `.tab_index(isize)`, `.tab_stop(bool)`. Implement `Focusable { fn focus_handle(&self, cx: &App) -> FocusHandle }`.
- Dispatch from code: `window.dispatch_action(Box::new(Extrude), cx)`. It is deferred and
  goes to the focused node.
- Show the binding: `Kbd::binding_for_action(&Extrude, Some("Workspace"), window) -> Option<Kbd>`.

```rust
impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full()
            .key_context("Workspace").track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_extrude))            // fn on_extrude(&mut self, _: &Extrude, &mut Window, &mut Context<Self>)
            .on_action(cx.listener(|this, a: &SelectTool, _, cx| { this.tool = a.0; cx.notify(); }))
    }
}
```

---

## 4. Button, ButtonGroup, Toggle (`gpui_kit::component::button::*`)

`Button::new(id: impl Into<ElementId>)`. Builders:
`.label(impl Into<SharedString>)`, `.icon(impl Into<ButtonIcon>)` (takes `IconName`, `Icon`
or any `IconNamed`; **icon without a label gives an icon-only square button**),
`.outline()` (inherent), `.compact()`, `.loading(bool)`, `.dropdown_caret(bool)`,
`.rounded(impl Into<ButtonRounded>)`, `.tooltip(impl Into<SharedString>)`,
`.tooltip_with_action(text, &dyn Action, Option<&str /*context*/>)` (shows the shortcut),
`.tooltip_placement(Placement)`, `.tooltip_show_delay(Duration)` (default 500 ms),
`.on_click(impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)`,
`.on_hover(impl Fn(&bool, &mut Window, &mut App))`, `.toggled(bool)` (accessibility only),
`.tab_stop(bool)`, `.child(..)`.
Variants (`ButtonVariants`): `.primary() .secondary() .danger() .warning() .success() .info() .ghost() .link() .text() .custom(ButtonCustomVariant)`.
Traits: `.selected(bool)` (Selectable: the pressed look, use it for tool toggles),
`.disabled(bool)`, `.xsmall() .small() .large() .with_size(px(..))`.
```rust
Button::new("tool-line").icon(IconName::Minus).ghost().small()
    .selected(self.tool == Tool::Line)
    .tooltip_with_action("Line", &SelectTool(1), Some("Workspace"))
    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| { this.tool = Tool::Line; cx.notify(); }))
Button::new("ok").primary().label("OK");   Button::new("x").outline().label("Cancel").disabled(true)
```
**ButtonGroup**: `ButtonGroup::new(id).child(Button).children(..)`, `.multiple(bool)`,
`.layout(Axis)`, `.compact()`, `.outline()`, plus variants, sizes and `.disabled`.
`.on_click(impl Fn(&Vec<usize>, &mut Window, &mut App))` receives the selected indices.
Each child's `.selected()` drives its own look.
```rust
ButtonGroup::new("display").outline().compact()
    .child(Button::new("shaded").label("Shaded").selected(self.mode == 0))
    .child(Button::new("wire").label("Wire").selected(self.mode == 1))
    .on_click(cx.listener(|this, sel: &Vec<usize>, _, cx| { this.mode = sel[0]; cx.notify(); }))
```
**Toggle**: `Toggle::new(id).label(..).icon(..).checked(bool).tooltip(..)`,
`.on_click(impl Fn(&bool /*new*/, &mut Window, &mut App))`. `ToggleVariants`:
`.ghost()` (default) and `.outline()`.
**ToggleGroup**: `ToggleGroup::new(id).child(Toggle).segmented()`,
`.on_click(impl Fn(&Vec<bool>, ..))`.
**Toolbar** (`component::toolbar::{Toolbar, ToolbarGroup}`):
`Toolbar::new(id).child(impl Sizable + IntoElement)` (default size Small; buttons become
quiet), `ToolbarGroup::new(id).label("Sketch").child(..)`, `.content(any element)`.

---

## 5. Icons

`gpui_kit::component::{Icon, IconName, IconNamed}`. `Icon::new(impl Into<Icon>)`,
`Icon::empty()`, `.path("icons/foo.svg")` (asset path), `.data(&[u8])` (raw SVG bytes),
`.rotate(radians)`, `.transform(..)`, `.text_color(..)` (sets the stroke via
`currentColor`), sizes `.xsmall()`=12px `.small()`=14px default=16px `.large()`=24px
`.with_size(px(n))`, plus any `Styled` method. `IconName` is `IntoElement`, so
`.child(IconName::Search)` works. `IconName::view(cx) -> Entity<Icon>`.

**Component `IconName` (exactly 106 variants, all in the default `Assets` bundle):**
ALargeSmall ArrowDown ArrowLeft ArrowRight ArrowUp Asterisk Ban BatteryCharging BatteryFull
BatteryLow BatteryMedium BatteryWarning Battery Bell BookOpen Bot Building2 Calendar
CaseSensitive ChartPie Check ChevronDown ChevronLeft ChevronRight ChevronUp ChevronsUpDown
CircleAlert CircleCheck CircleUser CircleX Close Copy Cpu Dash Delete EllipsisVertical
Ellipsis ExternalLink EyeOff Eye FileText File FolderClosed FolderOpen Folder Frame
GalleryVerticalEnd Github Globe HardDrive HeartOff Heart Inbox Info Inspector
LayoutDashboard LoaderCircle Loader Map Maximize MemoryStick Menu Mic Minimize Minus Moon
Network Palette PanelBottomOpen PanelBottom PanelLeftClose PanelLeftOpen PanelLeft
PanelRightClose PanelRightOpen PanelRight Pause Play Plus Redo2 Redo RefreshCw Replace
ResizeCorner RotateCw Search Settings2 Settings SortAscending SortDescending SquareTerminal
Square StarFill StarOff Star Sun ThumbsDown ThumbsUp TriangleAlert Undo2 Undo User
WindowClose WindowMaximize WindowMinimize WindowRestore

**Full Lucide catalog (1830 icons):** `gpui_kit::assets::IconName`. Variants are the
PascalCase file names, e.g. `Box Move3d Rotate3d Scale3d Axis3d Ruler Grid3x3 Magnet
Layers Move Hand PencilRuler Circle Cylinder Cone Torus Spline PenTool Lock LockOpen Trash
Save Download Upload ZoomIn ZoomOut Crosshair Hexagon Triangle`. It works anywhere an icon
is accepted (`Icon::new(gpui_kit::assets::IconName::Box)`, `Button::icon(..)`), **but**
only the 106 defaults are embedded in `gpui_kit::assets::Assets`. For others, register
`gpui_kit::assets::AllAssets` (embeds all 1830), or embed just the ones you use:
`gpui_kit::assets::icon_assets!(CadIcons, [Box, Move3d, Rotate3d]);`. That macro gives an
`AssetSource` that returns `Ok(None)` for other paths. Chain it in front of `Assets`.
List all variants at runtime with `gpui_kit::assets::IconName::ALL`.

**No asset source means no icons.** If `.with_assets(..)` isn't called, every icon renders
blank. The current `crates/app/src/main.rs` doesn't call it.

**Your own SVGs plus the kit icons.** `Assets::load` returns **Err** for unknown paths, so
try yours first (`/tmp/gk-check/src/main.rs`; needs `rust-embed = "8"`):
```rust
#[derive(rust_embed::RustEmbed)]
#[folder = "assets"] #[include = "icons/**/*.svg"]
struct AppIcons;
pub struct AppAssets;
impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(f) = AppIcons::get(path) { return Ok(Some(f.data)); }
        gpui_kit::assets::Assets.load(path)            // or AllAssets
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut v: Vec<SharedString> = AppIcons::iter().filter(|p| p.starts_with(path)).map(|p| p.to_string().into()).collect();
        v.extend(gpui_kit::assets::Assets.list(path)?);
        Ok(v)
    }
}
// usage
Icon::empty().path("icons/extrude.svg").small()
Button::new("extrude").icon(Icon::empty().path("icons/extrude.svg")).ghost()
svg().path("icons/extrude.svg").size_4().text_color(cx.theme().foreground)   // raw GPUI: set a size yourself
```
SVGs are monochrome masks: draw with `stroke="currentColor"`/`fill="currentColor"`.
`text_color` provides the color.

**Typed enum for your icons:** `gpui_kit::component::icon_named!(CadIcon, "assets/icons");`
(path relative to `CARGO_MANIFEST_DIR`). Each SVG becomes a variant and `path()` returns
`"icons/<file>"`. It derives `IntoElement`, so **you must** add
`impl RenderOnce for CadIcon { fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement { Icon::new(self) } }`.
`IconNamed` and `SharedString` must be in scope where the macro is invoked.

---

## 6. Input (`gpui_kit::component::input::*`)

State: `InputState` (= `InputBaseState<InputMode>`, single line). Create it once (not in
`render`) with `cx.new(|cx| InputState::new(window, cx) …)`.
Builders (by value): `.placeholder(impl Into<SharedString>)`, `.default_value(impl Into<SharedString>)`,
`.masked(bool)`, `.pattern(regex::Regex)`, `.validate(impl Fn(&str, &mut App) -> bool)`,
`.mask_pattern(MaskPattern)`, and numeric `.step(impl Into<NumberStep>)`
(`f64` or `NumberStep::by_value(..)`), `.step_by(Fn(f64, StepAction, &mut App) -> f64)`,
`.min(f64)`, `.max(f64)`.
Methods on `&mut self`: `set_value(impl Into<InputContent> /* &str, String, SharedString */, window, cx)`,
`value(&self) -> SharedString`, `text(&self) -> &Rope`, `set_placeholder(.., window, cx)`,
`set_disabled(bool, cx)`, `set_masked(bool, window, cx)`, `focus(&self, window, cx)`,
`select_all(window, cx)`, `clean(window, cx)`, `set_step(impl Into<Option<NumberStep>>, window, cx)`.
**`set_value` does not emit `InputEvent::Change`** and it clears undo history, so there
is no feedback loop when you sync from the model.

Events: `InputEvent::{Change, PressEnter { secondary: bool, shift: bool }, Focus, Blur}`.
Keep the `Subscription`, because dropping it unsubscribes:
```rust
let depth = cx.new(|cx| InputState::new(window, cx).placeholder("Depth").default_value("10"));
self._subs.push(cx.subscribe_in(&depth, window, |this, state, ev: &InputEvent, window, cx| match ev {
    InputEvent::Change => this.preview = state.read(cx).value(),
    InputEvent::PressEnter { .. } | InputEvent::Blur => this.commit_depth(window, cx),
    InputEvent::Focus => {}
}));
```
Element: `Input::new(&Entity<InputState>)` with `.prefix(el)`, `.suffix(el)`,
`.cleanable(bool)`, `.mask_toggle()`, `.appearance(bool)` (false removes border and
background), `.bordered(bool)`, `.focus_bordered(bool)`, `.disabled(bool)`, `.readonly(bool)`,
`.tab_index(isize)`, `.h(..)`, `.id(..)`, sizes and `Styled`.
**Compact single-line field:** `Input::new(&s).xsmall().w(px(72.))`. For an inline cell,
add `.appearance(false)`.

**NumberInput:** `NumberInput::new(&Entity<InputState>)` with `.placeholder`, `.prefix`,
`.suffix`, `.appearance`, `.disabled`, sizes. It reuses `InputState`. By default the
state steps internally (`NumberStep::Fixed(1.)`, clamped to min/max) and you watch
`InputEvent::Change`. Call `state.set_step(None, window, cx)` to get
`NumberInputEvent::Step(StepAction::{Increment, Decrement})` and step it yourself:
```rust
let dist = cx.new(|cx| InputState::new(window, cx).default_value("5").step(0.5).min(0.));
NumberInput::new(&dist).small().suffix("mm")
```
Also available: `Textarea`/`TextareaState`, and `OtpInput`.

---

## 7. Select, Checkbox, Switch, Radio, Slider

**Select** (`component::select::{Select, SelectState, SelectEvent, SelectItem, SearchableVec, SelectGroup}`):
```rust
// SelectState::new(delegate: D, selected_index: Option<IndexPath>, window, cx)
// D = Vec<T> or SearchableVec<T>, where T: SelectItem (&'static str, String and SharedString are built in)
let unit = cx.new(|cx| SelectState::new(vec!["mm".into(), "cm".into(), "in".into()] as Vec<SharedString>,
                                         Some(IndexPath::new(0)), window, cx));
self._subs.push(cx.subscribe_in(&unit, window, |this, _, ev: &SelectEvent<Vec<SharedString>>, _, cx| {
    let SelectEvent::Confirm(value) = ev;           // Option<T::Value>
    this.unit = value.clone(); cx.notify();
}));
Select::new(&unit).small().placeholder("Unit").menu_width(px(160.))
```
State: `.searchable(bool)` (builder), `set_selected_index(Option<IndexPath>, window, cx)`,
`set_selected_value(&Value, window, cx)`, `set_items(D, window, cx)`,
`selected_index(&self, cx)`, `selected_value(&self) -> Option<&Value>`. It also emits
`DismissEvent`. Element: `.placeholder .icon .title_prefix .cleanable .menu_width
.menu_max_h .search_placeholder .appearance(bool) .disabled(bool) .empty(..)`.
Custom items: `impl SelectItem for T { type Value; fn title(&self) -> SharedString; fn value(&self) -> &Self::Value; /* opt: display_title, render, matches, disabled */ }`.
`IndexPath::new(row)` and `IndexPath::default().row(r).section(s)`. Fields are
`section`, `row` and `column`.

**Checkbox / Switch / Radio** are stateless and controlled. Each takes `::new(id)`,
`.label(..)`, `.checked(bool)`, `.on_change(impl Fn(&bool /*requested*/, &mut Window, &mut App))`
(`on_click` is an alias; the last call wins), `.tooltip(..)`, `.disabled(bool)`, sizes.
Switch also has `.color(Hsla)`.
```rust
Checkbox::new("show-grid").label("Grid").checked(self.grid)
    .on_change(cx.listener(|this, v: &bool, _, cx| { this.grid = *v; cx.notify(); }))
Switch::new("snap").checked(self.snap).on_change(cx.listener(|this, v, _, cx| { this.snap = *v; cx.notify(); }))
```
`RadioGroup::new(id)` / `::horizontal(id)` / `::vertical(id)`, `.children(["A", "B"])`,
`.child(Radio)`, `.selected_index(Option<usize>)`, `.layout(Axis)`, `.disabled(bool)`,
`.on_change(impl Fn(&usize, ..))`.

**Slider** (`component::slider::{Slider, SliderState, SliderEvent, SliderValue, SliderScale}`):
```rust
let opacity = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.01).default_value(0.5));
self._subs.push(cx.subscribe(&opacity, |this, _, ev: &SliderEvent, cx| match ev {
    SliderEvent::Change(v) => { this.opacity = v.start(); cx.notify(); }   // while dragging
    SliderEvent::Release(v) => this.commit(v.start()),                      // on mouse up
}));
Slider::new(&opacity)            // .horizontal() .vertical() .reverse() .disabled(bool)
```
`SliderValue::{Single(f32), Range(f32, f32)}`. `set_value(impl Into<SliderValue>, window, cx)`,
`value()`, `.scale(SliderScale::Logarithmic)`. `Slider::accessibility_label` is *(not in 0.7.3)*.

---

## 8. Tooltip, Popover, context menus, dropdown menus

**Tooltip.** Buttons, Checkbox, Switch, Radio and Toggle have `.tooltip("…")`. Any
`div()` needs an `.id(..)` and then
`.tooltip(|window, cx| Tooltip::new("Fillet").action(&Fillet, Some("Workspace")).build(window, cx))`.
The GPUI signature is `fn tooltip(self, impl Fn(&mut Window, &mut App) -> AnyView + 'static)`.
`Tooltip::element(|window, cx| el)` builds a custom body. App-wide delay:
`component::tooltip::TooltipDefaults`.

**Popover** (`component::popover::Popover`): `Popover::new(id)`,
`.trigger(T: Selectable + IntoElement)` (e.g. a Button), `.content(|state: &mut PopoverState, window, cx| el)`
or `.child(..)`, `.anchor(Anchor::TopLeft|TopRight|BottomLeft|BottomRight)`, `.offset(px)`,
`.arrow(bool)`, `.mouse_button(MouseButton)`, `.default_open(bool)`, controlled
`.open(bool)` + `.on_open_change(Fn(&bool, &mut Window, &mut App))`,
`.overlay_closable(bool)`, `.appearance(bool)`, `.track_focus(&FocusHandle)`, `Styled`.
```rust
Popover::new("snap-opts").anchor(Anchor::TopRight)
    .trigger(Button::new("snap-btn").icon(IconName::Settings2).ghost().small())
    .w(px(240.)).p_2().child(v_flex().gap_2().child(Checkbox::new("s1").label("Endpoints").checked(true)))
```

**Context menu** (`component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem}`). Any
`InteractiveElement + ParentElement + Styled`:
`.context_menu(impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static) -> ContextMenu<Self>`.
Give the element a stable `.id()`. **Call it last**, because it wraps the element in
`ContextMenu<E>` and further `Div` methods are gone. `ContextMenu` is `IntoElement` but
not `Into<AnyElement>`, so add `.into_any_element()` when a parent wants `Into<AnyElement>`
(e.g. `h_resizable().child(..)`).
```rust
div().id("viewport").size_full().child(canvas)
    .context_menu(move |menu, window, cx| {
        menu.menu("Extrude", Box::new(Extrude))
            .menu_with_icon("Undo", IconName::Undo, Box::new(Undo))
            .menu_with_check("Snap to grid", snap, Box::new(ToggleSnap))
            .menu_with_disabled("Paste", Box::new(Paste), !can_paste)
            .separator().label("View")
            .submenu("Standard views", window, cx, |m, _, _| m.menu("Top", Box::new(ViewTop)).menu("Iso", Box::new(ViewIso)))
            .item(PopupMenuItem::new("Custom…").icon(IconName::Palette).on_click(|_ev, _w, _cx| {}))
    })
```
Other `PopupMenu` builders: `link(label, href)`, `menu_element(action, |w, cx| el)`,
`menu_with_enable`, `min_w(px)`, `max_h(px)`, `scrollable(bool)`, `check_side(Side)`.
`PopupMenuItem::{new(label), element(f), separator(), label(l), submenu(l, Entity<PopupMenu>), link(..)}`
with `.icon .action(Box<dyn Action>) .disabled .checked .on_click(Fn(&ClickEvent, &mut Window, &mut App))`.
**Action routing:** a chosen item calls `window.dispatch_action` from the current focus,
so your `.on_action` handlers must sit on an ancestor of the menu trigger. Otherwise call
`.action_context(focus_handle)` on the menu, which focuses that handle first. Shortcuts
appear next to items automatically when bound.
`Tree::context_menu(|ix, &TreeEntry, PopupMenu, window, cx| menu)` and
`Input::context_menu(..)` are the per-component variants.

**Dropdown menu on a button** (`menu::DropdownMenu` trait, implemented for `Button`):
```rust
Button::new("file").label("File").ghost().small().dropdown_caret(true)
    .dropdown_menu(|menu, window, cx| menu.menu("New part", Box::new(NewPart)).separator().menu("Export STEP…", Box::new(ExportStep)))
// .dropdown_menu_with_anchor(Anchor::TopRight, f); the result has .anchor(..) and .on_open_change(Fn(&bool, ..))
DropdownButton::new("extrude-dd").button(Button::new("extrude").label("Extrude"))   // split button
    .dropdown_menu(|m, _, _| m.menu("Revolve", Box::new(Revolve)))
```
Native app menus: `cx.set_menus(vec![Menu { name: "Edit".into(), items: vec![MenuItem::action("Undo", Undo)], disabled: false }])`
(GPUI). The kit also has `menu::AppMenuBar`.

---

## 9. Panels, sidebar, tabs, trees, lists, scrolling

**Resizable** (`component::resizable::*`, also re-exported at the `component` root):
`h_resizable(id)` / `v_resizable(id) -> ResizablePanelGroup` with `.child(impl Into<ResizablePanel>)`
(any `Into<AnyElement>` or a nested group), `.children(..)`, `.with_state(&Entity<ResizableState>)`,
`.on_resize(Fn(&Entity<ResizableState>, &mut Window, &mut App))`.
`resizable_panel()` with `.size(px)` (initial), `.size_range(px(a)..px(b))`, `.visible(bool)`,
`.child(..)`. Plain children flex. `ResizableState::default()`, `.sizes() -> &Vec<Pixels>`.
`ResizablePanel::fixed()` and component-level `resize_handle` are *(not in 0.7.3)*.
```rust
h_resizable("workspace").with_state(&self.split)
    .child(resizable_panel().size(px(260.)).size_range(px(180.)..px(480.)).child(feature_tree))
    .child(viewport_el)                                             // grows
    .child(resizable_panel().size(px(300.)).child(properties_panel))
```
**Sidebar** (`component::sidebar::*`): `Sidebar::new(id)` with `.side(Side)`,
`.collapsible(SidebarCollapsible::{Icon, Offcanvas, None} | bool)`, `.collapsed(bool)`,
`.header(el)`, `.footer(el)`, `.child(SidebarGroup|SidebarMenu)`, `.w(px(240.))`.
`SidebarGroup::new("Label").child(SidebarMenu::new().children([SidebarMenuItem::new("Parts").icon(IconName::Folder).active(true).on_click(|ev, w, cx| {})]))`.
`SidebarMenuItem` also has `.children([..])`, `.default_open`, `.suffix(..)`,
`.context_menu(..)`. Use `SidebarToggleButton::new().collapsed(b).on_click(..)` to toggle.

**Tabs** (`component::tab::{Tab, TabBar}`): `TabBar::new(id)`, `.child(impl Into<Tab>)`
(`&'static str`, `String`, `SharedString`, `Icon`, `IconName` or `Tab`),
`.selected_index(usize)`, `.on_click(Fn(&usize, &mut Window, &mut App))`,
`.underline() .pill() .outline() .segmented()`, `.prefix/.suffix(el)`, `.menu(bool)`, sizes.
**`Tab::new()` takes no id**: `Tab::new().label("Drawing").icon(IconName::File)`. Tab bars
are stateless, so keep the index yourself. `.folder()` is *(not in 0.7.3)*.
```rust
TabBar::new("doc-tabs").underline().small().selected_index(self.tab)
    .on_click(cx.listener(|this, ix: &usize, _, cx| { this.tab = *ix; cx.notify(); }))
    .child("Part1").child("Assembly1").child(Tab::new().label("Drawing1"))
```
**Tree** (`component::tree::{tree, Tree, TreeState, TreeItem, TreeEntry, TreeEvent}`)
suits the feature tree:
```rust
let ft = cx.new(|cx| TreeState::new(cx).items(vec![
    TreeItem::new("origin", "Origin").expanded(true).child(TreeItem::new("xy", "XY Plane")),
    TreeItem::new("sketch1", "Sketch1"),
]));
tree(&self.ft, move |ix, entry: &TreeEntry, selected, _window, cx| {
    ListItem::new(ix).selected(selected).pl(px(12.) * entry.depth() + px(8.))
        .child(h_flex().gap_2().child(if entry.is_folder() { IconName::Folder } else { IconName::File })
                       .child(entry.item().label.clone()))
}).context_menu(|_ix, entry, menu, _, _| menu.menu("Rename", Box::new(Rename)).menu("Delete", Box::new(DeleteSelection)))
```
`TreeItem { pub id, pub label, pub children }` with `::new(id, label)`, `.child`,
`.children`, `.expanded(bool)`, `.disabled(bool)`. `TreeState`: `set_items(items, cx)`,
`selected_index()`, `set_selected_index(Option<usize>, cx)`, `selected_item()`,
`selected_entry()`, `set_selected_item(Option<&TreeItem>, cx)`, `index_of(&id)`,
`scroll_to_item(ix, ScrollStrategy)`, `focus(window, cx)`. `TreeEvent::{Expanded(id), Collapsed(id)}`.
There is **no selection-changed event**: use `ListItem::on_click` inside `render_item` or
read the state after an action. `render_item` gets `&mut App`, so to update your view use
`view.update(cx, |this, cx| …)` with `let view = cx.entity();` captured.

**List** (`component::list::{List, ListState, ListDelegate, ListItem, ListEvent}`) is
virtualized and optionally searchable. Rows must all be the same height.
```rust
impl ListDelegate for Parts {
    type Item = ListItem;
    fn items_count(&self, _section: usize, _: &App) -> usize { self.rows.len() }
    fn render_item(&mut self, ix: IndexPath, _: &mut Window, _: &mut Context<ListState<Self>>) -> Option<ListItem> {
        Some(ListItem::new(ix.row).selected(self.sel == Some(ix)).child(self.rows[ix.row].clone()))
    }
    fn set_selected_index(&mut self, ix: Option<IndexPath>, _: &mut Window, cx: &mut Context<ListState<Self>>) { self.sel = ix; cx.notify(); }
    // optional: sections_count, perform_search(&mut self, &str, ..) -> Task<()>, confirm(secondary, ..), cancel, render_empty, …
}
let parts = cx.new(|cx| ListState::new(Parts::new(), window, cx).searchable(true));
cx.subscribe(&parts, |this, _, ev: &ListEvent, cx| if let ListEvent::Confirm(ix) = ev { /* … */ });  // Select(ix) | Confirm(ix) | Cancel
List::new(&parts).h(px(300.))
```
`ListItem::new(id)` has `.selected .disabled .on_click(Fn(&ClickEvent, ..)) .suffix(|w, cx| el) .check_icon(..)`.

**Virtual lists:**
- Kit: `v_virtual_list(view: Entity<V>, id, item_sizes: Rc<Vec<Size<Pixels>>>, |this: &mut V, range: Range<usize>, window, cx: &mut Context<V>| -> Vec<impl IntoElement>)`.
  Rows may vary in size. `.track_scroll(&VirtualListScrollHandle)`. `h_virtual_list` is the
  horizontal version.
- GPUI: `uniform_list(id, count, |range, window, cx: &mut App| Vec<_>)` with
  `.track_scroll(&UniformListScrollHandle)`. `handle.scroll_to_item(ix, ScrollStrategy::Center)`.
```rust
div().relative().size_full()
    .child(v_virtual_list(cx.entity(), "rows", self.item_sizes.clone(), |this, range, _, _| {
        range.map(|ix| div().h(px(24.)).child(this.rows[ix].clone())).collect()
    }).track_scroll(&self.vscroll))
    .vertical_scrollbar(&self.vscroll)              // scroll::ScrollableElement
```
**Scrolling:** GPUI `.overflow_y_scroll()` / `.overflow_scroll()` need an `.id(..)`
(stateful element) and show no scrollbar. The kit adds
`use gpui_kit::component::scroll::ScrollableElement as _;`, giving `.overflow_y_scrollbar()`,
`.overflow_x_scrollbar()`, `.overflow_scrollbar()`, and `.scrollbar(&handle, ScrollbarAxis)` /
`.vertical_scrollbar(&handle)` for an element that tracks a `ScrollHandle` or
`VirtualListScrollHandle`.
```rust
v_flex().id("props").size_full().gap_2().p_2().overflow_y_scrollbar().children(rows)
```

---

## 10. Dialogs, sheets, notifications, Kbd (`component::WindowExt`)

```rust
window.open_dialog(cx, |dialog, _window, _cx| {          // F: Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static (it may re-run, so keep it cheap)
    dialog.title("Fillet").w(px(360.)).child(Input::new(&radius_input))
        .footer(DialogFooter::new().gap_2()
            .child(DialogClose::new().child(Button::new("cancel").label("Cancel").outline()))
            .child(DialogAction::new().child(Button::new("ok").label("Apply").primary())))
        .on_ok(|_ev, _w, _cx| true)                         // return false to keep it open
});
window.open_alert_dialog(cx, |a, _, _| a.title("Delete Sketch1?").description("Dependent features will fail.")
    .show_cancel(true).ok_text("Delete").ok_variant(ButtonVariant::Danger).on_ok(|_, _, _| true));
window.open_sheet_at(Placement::Right, cx, |sheet, _, _| sheet.title("Measure").size(px(320.)).child(content));
window.push_notification("Saved", cx);                    // impl Into<Notification>: &str / String / SharedString
window.push_notification(Notification::error("Rebuild failed").title("Sketch1"), cx);
```
Dialog imports: `component::dialog::{Dialog, DialogFooter, DialogClose, DialogAction, AlertDialog}`
and `button::ButtonVariant`. Dialog also has `.on_cancel`, `.on_close`,
`.close_button(bool)`, `.overlay(bool)`, `.overlay_closable(bool)`, `.keyboard(bool)`,
`.width()`, `.max_w()`. Window methods: `close_dialog(cx)`, `close_all_dialogs(cx)`,
`has_active_dialog(cx)`, `close_sheet(cx)`, `remove_notification::<T>(cx)`,
`clear_notifications(cx)`, `has_focused_input(cx)`.
`Notification::new()` **takes no args**. Type constructors are associated functions:
`Notification::info(msg)` / `success` / `warning` / `error`. Chain `.title .message
.icon .autohide(bool) .placement(Anchor) .on_click .action(..) .id::<T>()` (dedupe key).
**Kbd** (`component::kbd::Kbd`): `Kbd::new(Keystroke::parse("secondary-shift-p").unwrap())`,
`Kbd::binding_for_action(&action, Some("ctx"), window) -> Option<Kbd>`,
`Kbd::binding_for_action_in(&action, &focus_handle, window)`, `.outline()`, `.appearance(bool)`.

---

## 11. Layout and GPUI essentials

- `h_flex()` = `div().h_flex()`: flex row with `items_center`. `v_flex()`: flex column
  that stretches across. Both live in `gpui_kit::component` (and `gpui_kit::base`).
  **StyledExt** (`component::StyledExt`) adds `.h_flex() .v_flex() .paddings(Edges) .margins(Edges) .corner_radii(Corners) .debug_red()…`.
  `ThemeStyled` adds `.popover_style(cx)` (popup surface: bg, shadow, radius),
  `.focus_ring_style(window, cx)` and `.rounded_full_style(cx)`.
- Common GPUI style methods: `size_full w_full h_full w(px) h(px) min_w max_w flex_1 flex_none flex_shrink_0 gap_1/2/3 p_2 px_2 py_1 m_* items_center justify_between justify_center relative absolute inset_0 top/left(px) overflow_hidden bg(color) text_color border_1 border_b_1 border_color rounded(px) rounded_md shadow_md cursor_pointer cursor(CursorStyle) opacity hover(|s| s.bg(..)) active(|s| ..) text_xs/sm font_weight(FontWeight::MEDIUM) truncate whitespace_nowrap`.
- Interaction (needs `.id(...)` for click/drag/scroll/tooltip):
  `div().id("x").on_click(|ev: &ClickEvent, window, cx| …)`, `.on_mouse_down(MouseButton::Left, |ev: &MouseDownEvent, w, cx| …)`,
  `.on_mouse_move(|ev: &MouseMoveEvent, w, cx| …)`, `.on_mouse_up(..)`, `.on_scroll_wheel(|ev: &ScrollWheelEvent, w, cx| …)`,
  `.on_key_down(..)`, and `cx.stop_propagation()` inside a handler.
- Conditional builders: `use gpui_kit::prelude::FluentBuilder as _;` gives
  `.when(bool, |el| el)`, `.when_some(opt, |el, v| el)`, `.when_none(&opt, ..)`,
  `.when_else(b, then, else)`, `.map(|el| ..)`.
- Overlays and floating panels: `deferred(child)` paints after the rest of the tree
  (above it), with `.with_priority(n)`. `anchored()` positions out of flow with
  `.position(point(px, px))`, `.anchor(Anchor::TopLeft)`, `.offset(point)`,
  `.position_mode(AnchoredPositionMode::Window|Local)`, `.snap_to_window()`.
  Combine them for a floating HUD over the viewport:
  `deferred(anchored().position(pos).snap_to_window().child(panel)).with_priority(1)`.
  For a fixed overlay inside the viewport, `div().relative()` with
  `div().absolute().top_2().right_2()` children is simpler.
- Entities: `cx.new(|cx: &mut Context<T>| T) -> Entity<T>`, `entity.read(cx) -> &T`,
  `entity.update(cx, |this, cx| …) -> R`, `entity.downgrade() -> WeakEntity<T>`,
  `weak.update(cx, …) -> Result<R>`, `weak.upgrade() -> Option<Entity<T>>`,
  `cx.entity()`, `cx.weak_entity()`.
- Rendering: `impl Render for V { fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement }`.
  For stateless parts use `#[derive(IntoElement)] struct X; impl RenderOnce for X { fn render(self, window, cx: &mut App) -> impl IntoElement }`.
  An `Entity<V: Render>` is itself a child element: `.child(self.viewport.clone())`.
- `cx.notify()` re-renders this entity. `cx.listener(|this, ev: &E, window, cx| …)` turns
  a method into `Fn(&E, &mut Window, &mut App)`. `window.listener_for(&entity, f)` does
  the same for another entity. `cx.processor(..)` handles by-value args.
- Events: `impl EventEmitter<MyEvent> for Model {}`, then `cx.emit(MyEvent::Changed)`.
  - `cx.subscribe(&e, |this, e: Entity<E>, ev: &Evt, cx| ..) -> Subscription`
  - `cx.subscribe_in(&e, window, |this, e: &Entity<E>, ev, window, cx| ..)`
  - `cx.observe(&e, |this, e, cx| ..)` fires on `notify`
  - `cx.observe_in(&e, window, ..)`, `cx.on_release(..)`

  Store the `Subscription`s in a `Vec<Subscription>` field, or `.detach()` them.
  Dropping one unsubscribes.
- Async:
  - `cx.spawn(async move |this: WeakEntity<T>, cx: &mut AsyncApp| { … this.update(cx, |this, cx| …) })`
  - `cx.spawn_in(window, async move |this, cx: &mut AsyncWindowContext| { this.update_in(cx, |this, window, cx| …) })`
  - Background thread: `cx.background_spawn(fut)` (trait `AppContext`, in scope via the
    glob) or `cx.background_executor().spawn(fut)`. Both need a `Send` future and return
    `Task<R>`.
  - Timer: `cx.background_executor().timer(Duration::from_millis(16)).await`.

  `Task` cancels when dropped. Store it (e.g. `_rebuild: Option<Task<()>>` for
  debouncing) or call `.detach()`.
```rust
self._rebuild = Some(cx.spawn_in(window, async move |this, cx| {
    cx.background_executor().timer(Duration::from_millis(150)).await;      // debounce
    let mesh = cx.background_spawn(async move { kernel_rebuild(doc) }).await;
    this.update_in(cx, |this, _window, cx| { this.mesh = Some(mesh); cx.notify(); }).ok();
}));
```
- Globals: `impl Global for S {}`, then `cx.set_global(S)`, `cx.global::<S>()`,
  `cx.global_mut::<S>()`, `cx.observe_global::<S>(..)`. Deferred work:
  `window.defer(cx, |window, cx| …)` and `cx.defer(|cx| …)`.

---

## 12. Drag and drop (reordering rows)

Signatures (GPUI `elements/div.rs`):
- `on_drag<T: 'static, W: Render>(value: T, constructor: impl Fn(&T, Point<Pixels> /*offset*/, &mut Window, &mut App) -> Entity<W>)`.
  On `StatefulInteractiveElement`, so it needs `.id()`. `W` is the drag preview view.
- `drag_over::<T>(|style: StyleRefinement, &T, &mut Window, &mut App| -> StyleRefinement)`
  is the hover highlight while a `T` is dragged over.
- `on_drop::<T>(impl Fn(&T, &mut Window, &mut App))`, and
  `can_drop(|any: &dyn Any, window, cx| -> bool)`. Also `on_drag_move::<T>(..)` and
  `group_drag_over::<T>(group, f)`.
```rust
#[derive(Clone)]
struct DragFeature { ix: usize, label: SharedString }
impl Render for DragFeature {            // the drag preview
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().px_2().py_1().popover_style(cx).child(self.label.clone())      // ThemeStyled
    }
}
v_flex().children(self.features.iter().enumerate().map(|(ix, f)| {
    div().id(SharedString::from(format!("feat-{}", f.id)))              // stable domain id, not ix
        .child(f.label.clone())
        .on_drag(DragFeature { ix, label: f.label.clone() }, |drag, _offset, _, cx| cx.new(|_| drag.clone()))
        .drag_over::<DragFeature>(|s, _, _, cx| s.border_t_2().border_color(cx.theme().drag_border))
        .on_drop(cx.listener(move |this, drag: &DragFeature, _, cx| {
            let item = this.features.remove(drag.ix);
            this.features.insert(ix.min(this.features.len()), item);
            cx.notify();
        }))
}))
```
Inside the constructor, `cx.stop_propagation()` stops parent drags (the kit's
`tab_panel.rs` does this). The kit's own drag colors are `cx.theme().drag_border` and
`cx.theme().drop_target`.

---

## Gotchas (0.7.3)
1. Docs in `skills/gpui-kit/references/usage.md` are wrong in places: `Tab::new()` takes no
   id, `Notification::new()` takes no args (use `Notification::info(msg)`), and
   `theme.surface`/`theme.destructive` don't exist.
2. `use gpui_kit::*` doesn't include `FluentBuilder`. Import `gpui_kit::prelude::FluentBuilder`.
3. Globbing both `gpui_kit::*` and `gpui_kit::component::*` makes `Size` and `Edges` ambiguous.
4. Without `.with_assets(..)` icons are blank. `Assets` holds only 106 icons and
   **errors** on unknown paths, so put app assets first in a combined source.
5. `icon_named!` needs a manual `impl RenderOnce`.
6. Bare-letter shortcuts eat keystrokes in inputs. Use `"Workspace && !Input"`.
   Actions need a focused element, so focus the root after opening the window.
7. `.context_menu(..)` must come last in a chain and needs `.into_any_element()` for
   `h_resizable().child`. Menu actions bubble from the focused element unless you set
   `.action_context(handle)`.
8. `InputState::set_value` emits no `Change` and resets undo. NumberInput only emits
   `Step` events after `set_step(None, ..)`.
9. `Theme::change`/mode switches overwrite colors. Re-apply custom colors afterwards, or
   ship a JSON ThemeConfig.
10. The git clone (main) is ahead of 0.7.3. `scroll_bounce`, `Slider::accessibility_label`,
    `ResizablePanel::fixed`, `TabBar::folder` and component `resize_handle` are missing in 0.7.3.
