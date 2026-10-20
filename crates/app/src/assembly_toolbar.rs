//! The assembly toolbar as tool sets, and Customize toolbar (live
//! assembly/331z, 340–343, part-studio/340–347, dom/customize-toolbar-*.txt):
//! right-click the toolbar → Customize toolbar… / Hide "Search tools". In
//! customize mode every tool set gets a 10 px `#cccccc` sled with "•••"
//! under it (drag to move the set), tools can be dragged onto the 640 px
//! "Tools" panel (red dashed drop zone) to remove them, and back onto the
//! toolbar; footer ☐ Show items already in toolbar · Save · Reset to default
//! · Cancel. Saved in the config directory (`assembly_toolbar.json`).

use super::*;
use gpui_kit::component::input::InputEvent;
use serde::{Deserialize, Serialize};

/// Onshape's assembly tool sets, in order (live dom/toolbar-assembly.txt).
pub(crate) const TOOLSETS: &[&[&str]] = &[
    &["tb-update-refs"],
    &["tb-insert"],
    &["tb-mc"],
    &["tb-fastened", "tb-revolute", "tb-slider", "tb-planar", "tb-cyl", "tb-pinslot", "tb-ball", "tb-parallel"],
    &["tb-tangent", "tb-width"],
    &["tb-group", "tb-snap", "tb-show-mates", "tb-replicate", "tb-replace", "tb-linpat", "tb-circpat", "tb-mirror"],
    &["tb-gear", "tb-rack", "tb-screw", "tb-linear"],
    &["tb-display-states", "tb-ps-in-context"],
];

/// Tools panel label and icon of a tool (live 343).
pub(crate) fn tool_info(id: &str) -> (&'static str, &'static str) {
    match id {
        "tb-update-refs" => ("Update all references to latest versions", "update-all-button"),
        "tb-insert" => ("Insert", "insert"),
        "tb-mc" => ("Mate connector", "mate-connector-assembly"),
        "tb-fastened" => ("Fastened mate", "fastened"),
        "tb-revolute" => ("Revolute mate", "revolute"),
        "tb-slider" => ("Slider mate", "slider"),
        "tb-planar" => ("Planar mate", "planar"),
        "tb-cyl" => ("Cylindrical mate", "cylindrical"),
        "tb-pinslot" => ("Pin slot mate", "pin-slot"),
        "tb-ball" => ("Ball mate", "ball"),
        "tb-parallel" => ("Parallel mate", "parallel-mate"),
        "tb-tangent" => ("Tangent mate", "tangent-mate"),
        "tb-width" => ("Width mate", "width-mate"),
        "tb-group" => ("Group", "group"),
        "tb-snap" => ("Snap mode", "snap-mode"),
        "tb-show-mates" => ("Show mates mode", "show-mates"),
        "tb-replicate" => ("Replicate", "replicate"),
        "tb-replace" => ("Replace instance", "replace-part"),
        "tb-linpat" => ("Assembly linear pattern", "linear-pattern"),
        "tb-circpat" => ("Assembly circular pattern", "circular-pattern"),
        "tb-mirror" => ("Assembly mirror", "mirror"),
        "tb-gear" => ("Gear relation", "gear-relation"),
        "tb-rack" => ("Rack and pinion relation", "rack-pinion"),
        "tb-screw" => ("Screw relation", "screw-relation"),
        "tb-linear" => ("Linear relation", "linear-relation"),
        "tb-display-states" => ("Display states", "display-states"),
        _ => ("Create Part Studio in context", "create-partstudio-incontext"),
    }
}

/// The user's assembly toolbar: tool set order, removed tools, Search tools hidden.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct AsmToolbarPrefs {
    #[serde(default)]
    pub order: Vec<usize>,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub hide_search: bool,
}

impl AsmToolbarPrefs {
    fn path() -> std::path::PathBuf {
        crate::files::config_dir().join("assembly_toolbar.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, t);
        }
    }

    /// Tool set order (every default set once, new sets appended).
    pub fn set_order(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self.order.iter().copied().filter(|i| *i < TOOLSETS.len()).collect();
        out.dedup();
        for i in 0..TOOLSETS.len() {
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out
    }

    /// Tool sets as shown: (default index, tools still in the toolbar).
    pub fn sets(&self) -> Vec<(usize, Vec<&'static str>)> {
        self.set_order().into_iter().map(|i| (i, TOOLSETS[i].iter().copied().filter(|t| !self.removed.iter().any(|r| r == t)).collect::<Vec<_>>())).filter(|(_, t)| !t.is_empty()).collect()
    }

    /// Move tool set `set` before tool set `before` (drag a sled).
    pub fn move_set(&mut self, set: usize, before: usize) {
        let mut order = self.set_order();
        order.retain(|i| *i != set);
        let at = order.iter().position(|i| *i == before).unwrap_or(order.len());
        order.insert(at, set);
        self.order = order;
    }
}

/// A toolbar tool being dragged in customize mode.
#[derive(Clone)]
pub(crate) struct ToolDrag {
    pub id: &'static str,
}

impl Render for ToolDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size(px(28.)).flex().items_center().justify_center().bg(rgb(0xffffff)).border_1().border_color(rgb(0xaaaaaa)).child(crate::assets::icon(tool_info(self.id).1, 20., cx))
    }
}

/// A tool set's sled being dragged.
#[derive(Clone)]
pub(crate) struct ToolsetDrag {
    pub set: usize,
}

impl Render for ToolsetDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(40.)).h(px(10.)).bg(rgb(0xcccccc)).flex().items_center().justify_center().text_size(px(10.)).font_weight(FontWeight::BOLD).child("•••")
    }
}

/// Customize mode: the edited prefs, Show items already in toolbar, search.
pub(crate) struct TbCustom {
    pub draft: AsmToolbarPrefs,
    pub show_all: bool,
    pub search: Entity<InputState>,
    _sub: Subscription,
}

impl AssemblyView {
    /// Right-click the toolbar (live 331z): Customize toolbar… · Hide / Show "Search tools".
    pub(super) fn toolbar_menu(&self, view: Entity<Self>) -> impl Fn(crate::os_menu::OsMenu, &mut Window, &mut Context<crate::os_menu::OsMenu>) -> crate::os_menu::OsMenu + 'static {
        let hidden = self.ui.tb_prefs.hide_search;
        move |m, _, _| {
            let (v1, v2) = (view.clone(), view.clone());
            m.list_no_icons()
                .item(crate::os_menu::OsMenuItem::new("Customize toolbar…").on_click(move |_, window, cx| v1.update(cx, |this, cx| this.start_toolbar_customize(window, cx))))
                .item(crate::os_menu::OsMenuItem::new(if hidden { "Show \"Search tools\"" } else { "Hide \"Search tools\"" }).on_click(move |_, _, cx| {
                    v2.update(cx, |this, cx| {
                        this.ui.tb_prefs.hide_search = !this.ui.tb_prefs.hide_search;
                        this.ui.tb_prefs.save();
                        cx.notify();
                    })
                }))
        }
    }

    pub(super) fn start_toolbar_customize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search tools"));
        let sub = cx.subscribe(&search, |_, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                cx.notify();
            }
        });
        self.ui.tb_custom = Some(TbCustom { draft: self.ui.tb_prefs.clone(), show_all: false, search, _sub: sub });
        cx.notify();
    }

    /// The prefs the toolbar shows (the draft while customizing).
    pub(super) fn toolbar_prefs(&self) -> AsmToolbarPrefs {
        self.ui.tb_custom.as_ref().map(|c| c.draft.clone()).unwrap_or_else(|| self.ui.tb_prefs.clone())
    }

    /// One tool set: its tools, and in customize mode the `#f5f5f5`
    /// background, draggable tools, the "•••" sled and the drop targets.
    pub(super) fn render_toolset(&self, set: usize, tools: Vec<AnyElement>, ids: Vec<&'static str>, cx: &mut Context<Self>) -> AnyElement {
        let Some(_) = &self.ui.tb_custom else { return h_flex().children(tools).into_any_element() };
        let mut row = h_flex();
        for (el, id) in tools.into_iter().zip(ids) {
            row = row.child(div().id(SharedString::from(format!("tbc-{id}"))).cursor(CursorStyle::Move).on_drag(ToolDrag { id }, |d, _, _, cx| cx.new(|_| d.clone())).child(el));
        }
        v_flex()
            .id(SharedString::from(format!("tbset-{set}")))
            .bg(rgb(0xf5f5f5))
            .child(row)
            .child(
                div()
                    .id(SharedString::from(format!("tbsled-{set}")))
                    .h(px(10.))
                    .w_full()
                    .bg(rgb(0xcccccc))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor(CursorStyle::Move)
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .line_height(px(10.))
                    .text_color(rgb(0x333333))
                    .child("•••")
                    .on_drag(ToolsetDrag { set }, |d, _, _, cx| cx.new(|_| d.clone())),
            )
            .drag_over::<ToolsetDrag>(|st, _, _, _| st.border_l_2().border_color(rgb(0x1651b0)))
            .on_drop(cx.listener(move |this, d: &ToolsetDrag, _, cx| {
                if let Some(c) = &mut this.ui.tb_custom
                    && d.set != set
                {
                    c.draft.move_set(d.set, set);
                    cx.notify();
                }
            }))
            .drag_over::<ToolDrag>(|st, _, _, _| st.bg(rgb(0xe0eaf5)))
            .on_drop(cx.listener(|this, d: &ToolDrag, _, cx| {
                // A tool dragged back from the Tools panel returns to its set.
                if let Some(c) = &mut this.ui.tb_custom {
                    c.draft.removed.retain(|r| r != d.id);
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    /// The 640 px "Tools" panel of customize mode.
    pub(super) fn render_tb_custom_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let c = self.ui.tb_custom.as_ref()?;
        let pal = p();
        let query = c.search.read(cx).value().trim().to_lowercase();
        let changed = c.draft != self.ui.tb_prefs;
        let customized = c.draft != AsmToolbarPrefs { hide_search: c.draft.hide_search, ..Default::default() };
        let all: Vec<&'static str> = TOOLSETS.iter().flat_map(|s| s.iter().copied()).collect();
        let removed: Vec<&'static str> = all.iter().copied().filter(|t| c.draft.removed.iter().any(|r| r == t)).collect();
        let listed: Vec<&'static str> = if c.show_all { all.clone() } else { removed.clone() }.into_iter().filter(|t| query.is_empty() || tool_info(t).0.to_lowercase().contains(&query)).collect();
        let mut grid = div().flex().flex_wrap().p(px(8.));
        for id in &listed {
            let (label, icon) = tool_info(id);
            let in_bar = !removed.contains(id);
            grid = grid.child(
                h_flex()
                    .id(SharedString::from(format!("tbtool-{id}")))
                    .w(px(192.))
                    .h(px(34.))
                    .px(px(8.))
                    .gap(px(6.))
                    .when(in_bar, |d| d.opacity(0.4))
                    .when(!in_bar, |d| d.cursor(CursorStyle::Move).on_drag(ToolDrag { id }, |d, _, _, cx| cx.new(|_| d.clone())))
                    .child(crate::assets::icon(icon, 20., cx))
                    .child(div().truncate().text_size(px(13.)).text_color(pal.text).child(label)),
            );
        }
        let empty_note = listed.is_empty().then(|| {
            v_flex()
                .p(px(12.))
                .items_center()
                .text_size(px(13.))
                .text_color(pal.text_muted)
                .child(div().font_weight(FontWeight::BOLD).child("All items are already in the toolbar"))
                .child("Drop items here to remove them from the toolbar")
        });
        let button = |id: &'static str, label: &'static str, primary: bool, enabled: bool| {
            div()
                .id(id)
                .h(px(30.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(3.5))
                .text_size(px(14.))
                .when(primary, |d| d.bg(rgb(0x1651b0)).text_color(rgb(0xffffff)))
                .when(!primary, |d| d.bg(rgb(0xffffff)).border_1().border_color(rgb(0xaaaaaa)).text_color(rgb(0x333333)))
                .when(!enabled, |d| d.opacity(0.25))
                .when(enabled, |d| d.cursor_pointer())
                .child(label)
        };
        let show_all = c.show_all;
        Some(
            v_flex()
                .id("tb-custom-panel")
                .occlude()
                .absolute()
                .top(px(2.))
                .left(px(0.))
                .w(px(600.))
                .bg(rgb(0xffffff))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .child(
                    h_flex()
                        .h(px(42.))
                        .px(px(8.))
                        .border_b_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::BOLD).child("Tools"))
                        .child(div().w(px(198.)).child(Input::new(&c.search).small().text_size(px(14.)).pl(px(5.)))),
                )
                .child(
                    v_flex()
                        .id("tb-custom-body")
                        .min_h(px(120.))
                        .max_h(px(330.))
                        .overflow_y_scroll()
                        .bg(rgb(0xf5f5f5))
                        .children(empty_note)
                        .child(grid)
                        // Dropping a toolbar tool here removes it (red dashed zone while dragging).
                        .drag_over::<ToolDrag>(|st, _, _, _| st.border_2().border_dashed().border_color(rgb(0xd0021b)))
                        .on_drop(cx.listener(|this, d: &ToolDrag, _, cx| {
                            if let Some(c) = &mut this.ui.tb_custom
                                && !c.draft.removed.iter().any(|r| r == d.id)
                            {
                                c.draft.removed.push(d.id.to_string());
                                cx.notify();
                            }
                        })),
                )
                .child(
                    h_flex()
                        .h(px(44.))
                        .px(px(8.))
                        .gap(px(6.))
                        .border_t_1()
                        .border_color(rgb(0xd7d7d7))
                        .child(crate::os_ui::checkbox("tb-show-all", "Show items already in toolbar", show_all).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(c) = &mut this.ui.tb_custom {
                                c.show_all = !c.show_all;
                            }
                            cx.notify();
                        })))
                        .child(div().flex_1())
                        .child(button("tb-save", "Save", true, changed).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if changed && let Some(c) = this.ui.tb_custom.take() {
                                this.ui.tb_prefs = c.draft;
                                this.ui.tb_prefs.save();
                            }
                            cx.notify();
                        })))
                        .child(button("tb-reset", "Reset to default", false, customized).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(c) = &mut this.ui.tb_custom
                                && customized
                            {
                                c.draft = AsmToolbarPrefs { hide_search: c.draft.hide_search, ..Default::default() };
                            }
                            cx.notify();
                        })))
                        .child(button("tb-cancel", "Cancel", false, true).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.tb_custom = None;
                            cx.notify();
                        }))),
                )
                .into_any_element(),
        )
    }
}
