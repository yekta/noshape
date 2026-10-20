//! Feature list operations: copy/paste of features (through the system
//! clipboard, so features move between documents) and feature folders.

use super::PartStudioView;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use noshape_model::partstudio::Feature;
use noshape_model::reference::FeatureId;

/// Clipboard payload key so arbitrary text is never pasted as features.
const CLIP_KEY: &str = "noshape_features";

impl PartStudioView {
    pub(super) fn copy_selected_features(&mut self, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        // Keep history order regardless of selection order.
        let features: Vec<Feature> = studio.features.iter().filter(|f| self.selected_features.contains(&f.id)).cloned().collect();
        if features.is_empty() {
            return;
        }
        let n = features.len();
        if let Ok(text) = serde_json::to_string(&serde_json::json!({ CLIP_KEY: features })) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.flash_info(format!("Copied {n} feature{}", if n == 1 { "" } else { "s" }), cx);
        }
    }

    /// Paste features from the clipboard at the rollback bar.
    pub(super) fn paste_features(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(Ok(features)) = v.get_mut(CLIP_KEY).map(|f| serde_json::from_value::<Vec<Feature>>(f.take())) else { return };
        if features.is_empty() {
            return;
        }
        let n = features.len();
        let ids = self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Paste {n} feature{}", if n == 1 { "" } else { "s" }), cx, |ps| {
                let at = ps.rollback_index();
                ps.paste_features(&features, at)
            })
        });
        if let Some(ids) = ids {
            self.selected_features = ids;
        }
        cx.notify();
    }

    pub(super) fn dissolve_folder(&mut self, folder: u32, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Remove folder", cx, |ps| ps.dissolve_folder(folder));
        });
    }

    /// Suppress or unsuppress every feature of a folder.
    pub(super) fn suppress_folder(&mut self, folder: u32, suppressed: bool, cx: &mut Context<Self>) {
        let ids: Vec<FeatureId> = self.studio(cx).map(|s| s.features.iter().filter(|f| f.folder == Some(folder)).map(|f| f.id).collect()).unwrap_or_default();
        self.set_suppressed(ids, suppressed, cx);
    }

    /// Folder menu "Expand all" / "Collapse all".
    pub(super) fn set_all_folders_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if collapsed { "Collapse all folders" } else { "Expand all folders" }, cx, |ps| {
                for f in &mut ps.folders {
                    f.collapsed = collapsed;
                }
            });
        });
    }

    pub(super) fn toggle_folder(&mut self, folder: u32, cx: &mut Context<Self>) {
        // Collapse state is view state but lives in the document like Onshape's.
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Toggle folder", cx, |ps| {
                if let Some(f) = ps.folder_mut(folder) {
                    f.collapsed = !f.collapsed;
                }
            });
        });
    }

    pub(super) fn start_folder_rename(&mut self, folder: u32, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.studio(cx).and_then(|s| s.folder(folder)).map(|f| f.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().trim().to_string();
                this.folder_renaming = None;
                if !v.is_empty() {
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(this.tab, &format!("Rename folder : {v}"), cx, |ps| {
                            if let Some(f) = ps.folder_mut(folder) {
                                f.name = v;
                            }
                        });
                    });
                }
                cx.notify();
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.folder_renaming = Some((folder, st, sub));
        cx.notify();
    }
}
