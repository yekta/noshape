//! What an agent's call asks of an Assembly window (live mode), done as its
//! menu items and toolbar buttons do it. A child module of `assembly_view`
//! (it reads the view's state); called from `agent_window.rs`.

use super::AssemblyView;
use gpui_kit::*;
use noshape_ops::window::{DisplaySet, ListAct, Selected};

impl AssemblyView {
    /// view_camera's named view: as picking it in the Named views dialog.
    pub fn agent_named_view(&mut self, name: &str, cx: &mut Context<Self>) {
        self.apply_named_view(name, cx);
    }

    /// A display setting (view_display).
    pub fn agent_display(&mut self, set: DisplaySet, window: &mut Window, cx: &mut Context<Self>) {
        match set {
            DisplaySet::IsolateInstances { transparent, instances } => self.isolate_occurrences(transparent, instances, cx),
            DisplaySet::ExitIsolate => {
                self.exit_isolate(cx);
            }
            DisplaySet::ShowMates(on) => {
                // As Hide/show mates (ToggleMates).
                if self.show_mates != on {
                    self.show_mates = on;
                    self.refresh_scene(false, cx);
                    cx.notify();
                }
            }
            DisplaySet::ShowMatesMode(on) => {
                if self.show_mates_mode != on {
                    self.toggle_show_mates_mode(cx);
                }
            }
            DisplaySet::SnapMode(on) => {
                if self.snap.is_some() != on {
                    self.toggle_snap(window, cx);
                }
            }
            DisplaySet::IsolateControls { select_transparent, transparency, expand } => {
                // As the Isolate / Make transparent dialog's controls.
                if let Some(on) = select_transparent {
                    self.ui.select_transparent = on;
                }
                if let Some(t) = transparency {
                    self.ui.transparency = Some(crate::isolate_ui::t_alpha(t));
                }
                match expand {
                    Some(e) => self.set_iso_expand(Some(e.connectivity), e.steps as i32, cx),
                    None => {
                        self.rebuild_scene(true, cx);
                        cx.notify();
                    }
                }
            }
            // As the exploded view row's Activate <view> and the Done button.
            DisplaySet::Explode(Some(i)) => self.activate_explode_from_menu(i, cx),
            DisplaySet::Explode(None) => self.activate_explode(None, cx),
            // As dropping the view's rollback bar under this many steps.
            DisplaySet::ExplodeProgress(steps) => {
                self.set_explode_progress(steps as f64, cx);
                cx.notify();
            }
            DisplaySet::IsolateParts { .. } | DisplaySet::HighlightBoundary(_) | DisplaySet::OrientOnEdit(_) | DisplaySet::ThicknessOff | DisplaySet::ContextHidden(_) | DisplaySet::ContextControls { .. } => {}
        }
    }

    /// The selected instances (live mode's window snapshot).
    pub fn selected_instances(&self) -> &[noshape_model::assembly::InstanceId] {
        &self.selected
    }

    /// The selection (ui_select): instances, or Clear selection.
    pub fn agent_select(&mut self, sel: Selected, cx: &mut Context<Self>) {
        self.clear_selection();
        if let Selected::Instances(ids) = sel {
            self.selected = ids;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The instance list and mate rows (ui_list), as their menus do it.
    pub fn agent_list(&mut self, act: ListAct, cx: &mut Context<Self>) {
        match act {
            ListAct::All(expand) => self.expand_all(expand, cx),
            ListAct::Instances { tops, expand } => {
                for top in tops {
                    if expand {
                        self.expanded.insert(vec![top]);
                    } else {
                        self.expanded.retain(|p| p[0] != top);
                    }
                }
                cx.notify();
            }
            ListAct::Mates { mates, expand } => {
                for m in mates {
                    if expand {
                        self.ui.expanded_mates.insert(m);
                    } else {
                        self.ui.expanded_mates.remove(&m);
                    }
                }
                cx.notify();
            }
            ListAct::Find(top) => {
                // Find in instance list: selected, its row scrolled to.
                self.selected = vec![top];
                self.ui.scroll_to_selection = true;
                self.refresh_scene(false, cx);
            }
            ListAct::Folders { .. } | ListAct::Filter(_) | ListAct::Tables => {}
        }
    }

    /// The user's toolbar settings changed (ui_toolbar): as after Customize toolbar….
    pub fn agent_reload_toolbar(&mut self, cx: &mut Context<Self>) {
        self.ui.tb_prefs = super::toolbar::AsmToolbarPrefs::load();
        cx.notify();
    }
}
