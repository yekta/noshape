//! What an agent's call asks of an Assembly window (live mode), done as its
//! menu items and toolbar buttons do it. A child module of `assembly_view`
//! (it reads the view's state); called from `agent_window.rs`.

use super::AssemblyView;
use gpui_kit::*;
use noshape_ops::window::DisplaySet;

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
            DisplaySet::IsolateParts { .. } | DisplaySet::HighlightBoundary(_) | DisplaySet::OrientOnEdit(_) | DisplaySet::ThicknessOff => {}
        }
    }
}
