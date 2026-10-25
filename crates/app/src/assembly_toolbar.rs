//! The assembly toolbar as tool sets, and its right-click menu (live
//! assembly/331z): Customize toolbar… (`toolbar_customize`, shared with the
//! Part Studio) / Hide "Search tools". Saved in the config directory
//! (`assembly_toolbar.json`).

use super::*;
use gpui_kit::component::menu::DropdownMenu as _;
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
    /// S shortcut toolbar tools (toolbar tool ids; None = the defaults;
    /// Preferences › Shortcut toolbars › Assembly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut_tools: Option<Vec<String>>,
}

/// The S shortcut toolbar's default tools (live behaviours "Assembly":
/// 11 ticked of 30).
pub(crate) const DEFAULT_SHORTCUT_TOOLS: [&str; 11] = ["tb-insert", "tb-mc", "tb-fastened", "tb-revolute", "tb-slider", "tb-planar", "tb-group", "tb-replicate", "tb-replace", "tb-linpat", "tb-circpat"];

/// Tools the S toolbar offers besides the toolbar's (live: Section view,
/// Mass and section properties, Measure at the end of the list).
const SHORTCUT_EXTRAS: [(&str, &str, &str); 3] = [("tb-section", "Section view", "section-view-3d"), ("tb-mass", "Mass and section properties", "mass-properties"), ("tb-measure", "Measure", "measure")];

/// Every tool the S toolbar can hold (Preferences › Shortcut toolbars ›
/// Assembly, live's 30): (id, name, icon), in list order; the toolbar shows
/// the ticked ones in this order.
pub(crate) fn shortcut_catalog() -> Vec<(&'static str, &'static str, &'static str)> {
    TOOLSETS
        .iter()
        .flat_map(|s| s.iter())
        .map(|id| {
            let (name, icon) = tool_info(id);
            (*id, name, icon)
        })
        .chain(SHORTCUT_EXTRAS)
        .collect()
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

    pub fn shortcut_tools(&self) -> Vec<String> {
        self.shortcut_tools.clone().unwrap_or_else(|| DEFAULT_SHORTCUT_TOOLS.iter().map(|s| s.to_string()).collect())
    }

    pub fn toggle_shortcut(&mut self, id: &str) {
        let mut tools = self.shortcut_tools();
        match tools.iter().position(|t| t == id) {
            Some(i) => {
                tools.remove(i);
            }
            None => tools.push(id.to_string()),
        }
        self.shortcut_tools = Some(tools);
    }

    /// Tool sets as shown: (default index, tools still in the toolbar).
    pub fn sets(&self) -> Vec<(usize, Vec<&'static str>)> {
        self.set_order().into_iter().map(|i| (i, TOOLSETS[i].iter().copied().filter(|t| !self.removed.iter().any(|r| r == t)).collect::<Vec<_>>())).filter(|(_, t)| !t.is_empty()).collect()
    }
}

/// Live's toolbar fit (client `Toolbar` component; see the Part Studio's
/// `toolbar_collapse_from`): the longest run of tool sets from the left that
/// fits stays expanded. Onshape's widths: a tool 34 px, "Insert" 78, 4 px
/// per set; a collapsed set 56 (38 for one tool); Undo / Redo 70, "Search
/// tools…" 173, 7 at the end; room = the window minus 40. Live (bundle
/// model): everything fits from 1284 px; at 1280 Display states collapses.
pub(super) fn collapse_from(sets: &[(usize, Vec<&'static str>)], search_hidden: bool, viewport_w: f32) -> usize {
    let widths: Vec<(f32, f32)> = sets
        .iter()
        .map(|(_, ids)| (4. + ids.iter().map(|id| if *id == "tb-insert" { 78. } else { 34. }).sum::<f32>(), 4. + if ids.len() > 1 { 52. } else { 34. }))
        .collect();
    let fixed = 70. + if search_hidden { 0. } else { 173. } + 7.;
    (0..=widths.len())
        .rev()
        .find(|&k| fixed + widths[..k].iter().map(|w| w.0).sum::<f32>() + widths[k..].iter().map(|w| w.1).sum::<f32>() <= viewport_w - 40.)
        .unwrap_or(0)
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

    /// Runs a toolbar tool by id (the S shortcut toolbar).
    pub(super) fn run_toolbar_tool(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "tb-update-refs" => {
                if self.has_newer_references(cx) {
                    self.open_reference_manager(None, cx);
                }
            }
            "tb-insert" => self.toggle_insert(window, cx),
            "tb-mc" => self.open_mc(None, window, cx),
            "tb-fastened" => self.start_mate(MateType::Fastened, window, cx),
            "tb-revolute" => self.start_mate(MateType::Revolute, window, cx),
            "tb-slider" => self.start_mate(MateType::Slider, window, cx),
            "tb-planar" => self.start_mate(MateType::Planar, window, cx),
            "tb-cyl" => self.start_mate(MateType::Cylindrical, window, cx),
            "tb-pinslot" => self.start_mate(MateType::PinSlot, window, cx),
            "tb-ball" => self.start_mate(MateType::Ball, window, cx),
            "tb-parallel" => self.start_mate(MateType::Parallel, window, cx),
            "tb-tangent" => self.start_mate(MateType::Tangent, window, cx),
            "tb-width" => self.start_mate(MateType::Width, window, cx),
            "tb-group" => self.create_group(cx),
            "tb-snap" => self.toggle_snap(window, cx),
            "tb-show-mates" => self.toggle_show_mates_mode(cx),
            "tb-replicate" => self.open_pattern(PatternDialogKind::Replicate, window, cx),
            "tb-replace" => self.replace_selected(cx),
            "tb-linpat" => self.open_pattern(PatternDialogKind::Linear, window, cx),
            "tb-circpat" => self.open_pattern(PatternDialogKind::Circular, window, cx),
            "tb-mirror" => self.open_pattern(PatternDialogKind::Mirror, window, cx),
            "tb-gear" => self.open_relation(RelationType::Gear, None, window, cx),
            "tb-rack" => self.open_relation(RelationType::RackAndPinion, None, window, cx),
            "tb-screw" => self.open_relation(RelationType::Screw, None, window, cx),
            "tb-linear" => self.open_relation(RelationType::Linear, None, window, cx),
            "tb-display-states" => self.toggle_display_states(window, cx),
            "tb-ps-in-context" => self.open_ps_in_context(false, cx),
            "tb-section" => self.toggle_section(window, cx),
            "tb-mass" => self.toggle_mass(window, cx),
            "tb-measure" => self.toggle_measure(cx),
            _ => {}
        }
    }

    pub(super) fn start_toolbar_customize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (order, removed) = (self.ui.tb_prefs.order.clone(), self.ui.tb_prefs.removed.clone());
        self.ui.tb_custom = Some(crate::toolbar_customize::Customize::new(TOOLSETS.len(), &order, &removed, window, cx));
        cx.notify();
    }

    /// The prefs the toolbar shows (the draft while customizing).
    pub(super) fn toolbar_prefs(&self) -> AsmToolbarPrefs {
        match &self.ui.tb_custom {
            Some(c) => AsmToolbarPrefs { order: c.order.clone(), removed: c.removed.clone(), hide_search: self.ui.tb_prefs.hide_search, shortcut_tools: self.ui.tb_prefs.shortcut_tools.clone() },
            None => self.ui.tb_prefs.clone(),
        }
    }

    /// One tool set; in customize mode its tools drag and it gets its sled
    /// (`toolbar_customize`).
    pub(super) fn render_toolset(&self, set: usize, tools: Vec<AnyElement>, ids: Vec<&'static str>, collapsed: bool, cx: &mut Context<Self>) -> AnyElement {
        // A collapsed set (narrow window): its first tool with a ▾ listing
        // every tool of the set (as the Part Studio's); a single tool stays
        // (Insert loses its label in Onshape; ours keeps it).
        if collapsed && tools.len() > 1 && self.ui.tb_custom.is_none() {
            let view = cx.entity();
            let update_enabled = self.has_newer_references(cx);
            let face = tools.into_iter().next();
            let caret = crate::ui::tool_caret(SharedString::from(format!("asm-tbm-{set}")), cx).dropdown_menu(move |mut menu, window, _| {
                menu = crate::ui::fit_tool_menu(menu.min_w(px(180.)), window);
                for id in &ids {
                    let (name, icon) = tool_info(id);
                    let (v, id) = (view.clone(), *id);
                    menu = menu.item(crate::ui::menu_tool_item(name, icon, None).disabled(id == "tb-update-refs" && !update_enabled).on_click(move |_, window, cx| v.update(cx, |this, cx| this.run_toolbar_tool(id, window, cx))));
                }
                menu
            });
            return crate::ui::tool_group(SharedString::from(format!("asm-tbg-{set}")), div().children(face), caret).into_any_element();
        }
        // Live 1.222: 32 px buttons 2 px apart (34 px pitch, as the Part
        // Studio's toolbar).
        if self.ui.tb_custom.is_none() {
            return h_flex().gap(px(2.)).children(tools).into_any_element();
        }
        let mut row = h_flex().gap(px(2.));
        for (el, id) in tools.into_iter().zip(ids) {
            let Some(one) = TOOLSETS[set].iter().find(|t| **t == id) else { continue };
            row = row.child(crate::toolbar_customize::tool(el, std::slice::from_ref(one), crate::assets::icon_path(tool_info(id).1), cx));
        }
        crate::toolbar_customize::toolset(set, row, true, |v: &mut Self| v.ui.tb_custom.as_mut(), cx)
    }

    /// Customize toolbar's "Tools" panel, 10 px under the toolbar (the
    /// sleds hang there) at the left (live 340).
    pub(super) fn render_tb_custom_panel(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        static TOOLS: std::sync::OnceLock<Vec<crate::toolbar_customize::PanelTool>> = std::sync::OnceLock::new();
        let tools = TOOLS.get_or_init(|| {
            TOOLSETS
                .iter()
                .flat_map(|s| s.iter())
                .map(|id| {
                    let (label, icon) = tool_info(id);
                    crate::toolbar_customize::PanelTool { ids: std::slice::from_ref(id), label: label.into(), icon: crate::assets::icon_path(icon) }
                })
                .collect()
        });
        let panel = crate::toolbar_customize::panel(
            self,
            tools,
            |v: &mut Self| v.ui.tb_custom.as_mut(),
            |v: &mut Self, saved, cx| {
                if let Some((order, removed)) = saved {
                    v.ui.tb_prefs.order = order;
                    v.ui.tb_prefs.removed = removed;
                    v.ui.tb_prefs.save();
                }
                v.ui.tb_custom = None;
                cx.notify();
            },
            window,
            cx,
        )?;
        Some(div().absolute().top(px(10.)).left_0().child(panel).into_any_element())
    }
}
