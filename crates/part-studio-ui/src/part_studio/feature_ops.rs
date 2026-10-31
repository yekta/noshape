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
        // No toast, as live's Copy sketch / Copy Part (core-workflow 8.1.3, 8.6.6).
        if let Ok(text) = serde_json::to_string(&serde_json::json!({ CLIP_KEY: features })) {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
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
            doc.edit_studio(self.tab, noshape_ops::studio::UNPACK_FOLDER_DESCRIPTION, cx, |ps| ps.dissolve_folder(folder));
        });
    }

    /// Every feature in a folder, nested folders included.
    pub(super) fn folder_features(&self, folder: u32, cx: &App) -> Vec<FeatureId> {
        self.studio(cx).map(|s| s.folder_features(folder)).unwrap_or_default()
    }

    /// Suppress or unsuppress every feature of a folder (nested ones too:
    /// live qa4 r5 s8 (6)).
    pub(super) fn suppress_folder(&mut self, folder: u32, suppressed: bool, cx: &mut Context<Self>) {
        let ids = self.folder_features(folder, cx);
        self.set_suppressed(ids, suppressed, cx);
    }

    /// A folder counts as suppressed when everything in it is, by hand or
    /// by an expression (the regenerated state; its menu then offers
    /// Unsuppress; an empty folder never is).
    pub(super) fn folder_suppressed(&self, folder: u32, cx: &App) -> bool {
        let Some(s) = self.studio(cx) else { return false };
        let ids = s.folder_features(folder);
        let off = |id: &FeatureId| s.feature(*id).is_some_and(|f| f.suppressed) || self.display.feature_state(*id).is_some_and(|st| st.status == noshape_model::display::FeatureStatus::Suppressed);
        !ids.is_empty() && ids.iter().all(off)
    }

    /// The folder's Edit dynamic suppression ▸ Remove expression: off every
    /// feature in it (one undo step).
    pub(super) fn clear_folder_dynamic_suppression(&mut self, folder: u32, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::suppression_expr_description(None), cx, |ps| ps.set_folder_suppression_expr(folder, None));
        });
    }

    /// Whether any feature in a folder has a dynamic suppression expression
    /// (its menu then offers Edit dynamic suppression ▸ Remove expression).
    pub(super) fn folder_has_expr(&self, folder: u32, cx: &App) -> bool {
        self.studio(cx).is_some_and(|s| s.folder_features(folder).iter().any(|id| s.feature(*id).is_some_and(|f| f.suppression_expr.is_some())))
    }

    /// A feature's Edit dynamic suppression ▸ Remove expression.
    pub(super) fn remove_dynamic_suppression(&mut self, id: FeatureId, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::suppression_expr_description(None), cx, |ps| noshape_ops::studio::set_suppression_expr(ps, id, None));
        });
    }

    /// The folder row's Delete: the folder with everything in it.
    pub(super) fn delete_folder(&mut self, folder: u32, cx: &mut Context<Self>) {
        let ids = self.folder_features(folder, cx);
        let desc = self.studio(cx).map(|s| noshape_ops::studio::delete_description(s, &ids)).unwrap_or_else(|| noshape_ops::studio::delete_description(&Default::default(), &ids));
        self.ui.selected_folder = None;
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &desc, cx, |ps| ps.delete_folder_tree(folder));
        });
    }

    /// The folder row's Roll to here: the bar after its last feature.
    /// A folder's Roll to here: to just before its first child (live "psui (2026-10-28, P3) FOLDER-MENU-ROLL-TO-HERE":
    /// the bar goes to the folder's top, its children rolled back).
    pub(super) fn roll_to_folder(&mut self, folder: u32, cx: &mut Context<Self>) {
        let Some(first) = self.studio(cx).and_then(|s| s.folder_features(folder).iter().filter_map(|id| s.index_of(*id)).min()) else { return };
        self.set_rollback(Some(first), cx);
        self.ui.rollback_folder_top = Some((folder, first));
    }

    /// Expand ▸ / Collapse ▸ "… selected": the selected folder.
    pub(super) fn set_folder_collapsed(&mut self, folder: u32, collapsed: bool, cx: &mut Context<Self>) {
        self.ui.folder_open.insert(folder, !collapsed);
        cx.notify();
    }

    /// Folder menu "Expand all" / "Collapse all".
    pub(super) fn set_all_folders_collapsed(&mut self, collapsed: bool, cx: &mut Context<Self>) {
        let ids: Vec<u32> = self.studio(cx).map(|s| s.folders.iter().map(|f| f.id).collect()).unwrap_or_default();
        for id in ids {
            self.ui.folder_open.insert(id, !collapsed);
        }
        cx.notify();
    }

    /// Whether a folder is closed in this view (see `UiState::folder_open`).
    pub(super) fn folder_collapsed(&self, fd: &noshape_model::partstudio::Folder) -> bool {
        self.ui.folder_open.get(&fd.id).map_or(fd.collapsed, |open| !open)
    }

    /// Opening or closing a folder is view state: no undo step (live qa4
    /// r5 s8 (7): the undo steps are the user's model actions).
    pub(super) fn toggle_folder(&mut self, folder: u32, cx: &mut Context<Self>) {
        let Some(collapsed) = self.studio(cx).and_then(|s| s.folder(folder)).map(|fd| self.folder_collapsed(fd)) else { return };
        self.ui.folder_open.insert(folder, collapsed);
        cx.notify();
    }

    pub(super) fn start_folder_rename(&mut self, folder: u32, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.studio(cx).and_then(|s| s.folder(folder)).map(|f| f.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                // Once, and only a change (as the default geometry's rename: the late blur after Esc recorded the
                // unchanged name and ate the next menu click; core-workflow 8.5.2).
                if this.folder_renaming.take().is_none() {
                    return;
                }
                let v = st.read(cx).value().trim().to_string();
                let old = this.studio(cx).and_then(|s| s.folder(folder)).map(|f| f.name.clone()).unwrap_or_default();
                if !v.is_empty() && v != old {
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(this.tab, &noshape_ops::studio::rename_folder_description(&v), cx, |ps| noshape_ops::studio::rename_folder(ps, folder, &v));
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
