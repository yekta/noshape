//! What an agent's call asks of a Part Studio window (live mode), done as
//! its menu items do it. A child module of `part_studio` (it reads the
//! view's state); called from `agent_window.rs`.

use super::PartStudioView;
use super::behaviors::IsolateMode;
use gpui_kit::*;
use noshape_model::reference::Reference;
use noshape_ops::window::DisplaySet;

impl PartStudioView {
    /// A display setting (view_display).
    pub(crate) fn agent_display(&mut self, set: DisplaySet, window: &mut Window, cx: &mut Context<Self>) {
        match set {
            DisplaySet::IsolateParts { transparent, parts } => {
                // As the part menu's Isolate… / Make transparent…: on these parts.
                self.selection = parts.into_iter().map(Reference::Body).collect();
                self.hover = None;
                self.isolate(if transparent { IsolateMode::Transparent } else { IsolateMode::Isolate }, window, cx);
            }
            DisplaySet::ExitIsolate => {
                self.exit_isolate(window, cx);
            }
            DisplaySet::HighlightBoundary(on) => {
                // As View › Highlight boundary edges.
                if self.opts.highlight_boundary != on {
                    self.opts.highlight_boundary = on;
                    let d = self.display.clone();
                    self.apply_display(d, window, cx);
                }
            }
            DisplaySet::OrientOnEdit(on) => {
                self.ui.orient_on_edit = on;
                cx.notify();
            }
            DisplaySet::ThicknessOff => self.thickness_off(cx),
            DisplaySet::IsolateInstances { .. } | DisplaySet::ShowMates(_) | DisplaySet::ShowMatesMode(_) | DisplaySet::SnapMode(_) => {}
        }
    }
}
