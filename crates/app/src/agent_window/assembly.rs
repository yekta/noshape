//! What an agent's call asks of an Assembly window (live mode), done as its
//! menu items and toolbar buttons do it. A child module of `assembly_view`
//! (it reads the view's state); called from `agent_window.rs`.

use super::AssemblyView;
use gpui_kit::*;
use noshape_ops::window::{DisplaySet, ListAct, Selected};

impl AssemblyView {
    /// A display setting (view_display).
    pub(crate) fn agent_display(&mut self, set: DisplaySet, window: &mut Window, cx: &mut Context<Self>) {
        match set {
            DisplaySet::IsolateInstances { transparent, instances } => self.isolate_cmd(transparent, instances, cx),
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
            DisplaySet::IsolateParts { .. } | DisplaySet::HighlightBoundary(_) | DisplaySet::OrientOnEdit(_) | DisplaySet::ThicknessOff | DisplaySet::ContextHidden(_) => {}
        }
    }

    /// The selection (ui_select): instances, or Clear selection.
    pub(crate) fn agent_select(&mut self, sel: Selected, cx: &mut Context<Self>) {
        self.clear_selection();
        if let Selected::Instances(ids) = sel {
            self.selected = ids;
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// The instance list and mate rows (ui_list), as their menus do it.
    pub(crate) fn agent_list(&mut self, act: ListAct, cx: &mut Context<Self>) {
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
            ListAct::Folders { .. } | ListAct::Filter(_) => {}
        }
    }
}
