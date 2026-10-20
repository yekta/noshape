//! The assembly toolbar as tool sets, and its right-click menu (live
//! assembly/331z): Customize toolbar… (`toolbar_customize`, shared with the
//! Part Studio) / Hide "Search tools". Saved in the config directory
//! (`assembly_toolbar.json`).

use super::*;
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
        let (order, removed) = (self.ui.tb_prefs.order.clone(), self.ui.tb_prefs.removed.clone());
        self.ui.tb_custom = Some(crate::toolbar_customize::Customize::new(TOOLSETS.len(), &order, &removed, window, cx));
        cx.notify();
    }

    /// The prefs the toolbar shows (the draft while customizing).
    pub(super) fn toolbar_prefs(&self) -> AsmToolbarPrefs {
        match &self.ui.tb_custom {
            Some(c) => AsmToolbarPrefs { order: c.order.clone(), removed: c.removed.clone(), hide_search: self.ui.tb_prefs.hide_search },
            None => self.ui.tb_prefs.clone(),
        }
    }

    /// One tool set; in customize mode its tools drag and it gets its sled
    /// (`toolbar_customize`).
    pub(super) fn render_toolset(&self, set: usize, tools: Vec<AnyElement>, ids: Vec<&'static str>, cx: &mut Context<Self>) -> AnyElement {
        if self.ui.tb_custom.is_none() {
            return h_flex().children(tools).into_any_element();
        }
        let mut row = h_flex();
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
