//! Configurations panel › input ⋯ › Edit FeatureScript IDs… (bundle
//! `editFeatureScriptIdentifiersDialog.html` and its controller; Help
//! PartStudio__managing_configurations).

use super::PartStudioView;
use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::configuration::ConfigInputKind;

/// The cell error for an id (bundle `makeTableStringColumn`).
fn id_error(id: &str) -> Option<&'static str> {
    if id.is_empty() {
        return Some("ID cannot be empty");
    }
    let mut chars = id.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("ID must be a valid FeatureScript identifier");
    }
    noshape_model::configuration::FS_KEYWORDS.contains(&id).then_some("ID cannot be a reserved FeatureScript keyword")
}

type OnConfirm = std::rc::Rc<dyn Fn(String, Vec<(String, String)>, &mut App)>;

struct FsIdsDialog {
    studio_name: String,
    input_name: String,
    input_id: String,
    /// The input's default as shown ("Small"), for the id-change warning.
    default_display: String,
    /// The other inputs' ids (duplicates are refused).
    others: Vec<String>,
    id: Entity<InputState>,
    /// (old id, row name, editor) per list option.
    options: Vec<(String, String, Entity<InputState>)>,
    on_confirm: OnConfirm,
}

impl PartStudioView {
    pub(super) fn open_edit_fs_ids(&mut self, input: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx).cloned() else { return };
        let Some(inp) = studio.configuration.inputs.iter().find(|i| i.id == input).cloned() else { return };
        let default_display = match &inp.kind {
            ConfigInputKind::List { options, default } => options.iter().find(|o| &o.id == default).map(|o| o.name.clone()).unwrap_or_default(),
            ConfigInputKind::Checkbox { default } => default.to_string(),
            ConfigInputKind::Variable { default, .. } => default.text.clone(),
            ConfigInputKind::Text { default } => default.clone(),
        };
        let others = studio.configuration.inputs.iter().filter(|i| i.id != input).map(|i| i.id.clone()).collect();
        // Spaces are not accepted (live: typing "my size" leaves "size").
        let no_space = |s: &str, _: &mut App| !s.contains(char::is_whitespace);
        let id = cx.new(|cx| InputState::new(window, cx).validate(no_space).default_value(inp.id.clone()));
        let options = match &inp.kind {
            ConfigInputKind::List { options, .. } => options.iter().map(|o| (o.id.clone(), o.name.clone(), cx.new(|cx| InputState::new(window, cx).validate(no_space).default_value(o.id.clone())))).collect(),
            _ => Vec::new(),
        };
        let view = cx.entity().downgrade();
        let old = inp.id.clone();
        let on_confirm: OnConfirm = std::rc::Rc::new(move |new_id: String, renames: Vec<(String, String)>, cx: &mut App| {
            let old = old.clone();
            let _ = view.update(cx, |this, cx| this.change_config_ids(&old, &new_id, &renames, cx));
        });
        let studio_name = self.studio_name(cx);
        // The keyboard moves into the dialog (not a panel cell behind it).
        id.update(cx, |s, cx| s.focus(window, cx));
        let dialog = cx.new(|_| FsIdsDialog { studio_name, input_name: inp.name.clone(), input_id: inp.id.clone(), default_display, others, id, options, on_confirm });
        window.open_dialog(cx, move |d, _, _| d.w(px(560.)).margin_top(px(80.)).close_button(false).p_0().gap_0().rounded(px(4.)).child(dialog.clone()));
    }

    /// Apply new ids: the configuration and the chosen values follow.
    fn change_config_ids(&mut self, old: &str, new_id: &str, renames: &[(String, String)], cx: &mut Context<Self>) {
        let (o, n, r) = (old.to_string(), new_id.to_string(), renames.to_vec());
        self.edit_config("Edit FeatureScript IDs", cx, |ps| ps.configuration.change_ids(&o, &n, &r));
        if let Some(v) = self.config_values.0.remove(old) {
            let v = renames.iter().find(|(a, _)| *a == v).map(|(_, b)| b.clone()).unwrap_or(v);
            self.config_values.0.insert(new_id.to_string(), v);
        }
        self.config_dirty(cx);
    }
}

impl FsIdsDialog {
    fn new_id(&self, cx: &App) -> String {
        self.id.read(cx).value().to_string()
    }

    fn renames(&self, cx: &App) -> Vec<(String, String)> {
        self.options.iter().map(|(old, _, st)| (old.clone(), st.read(cx).value().to_string())).filter(|(a, b)| a != b).collect()
    }

    /// The first duplicate id (bundle `updateEditState`).
    fn duplicate(&self, cx: &App) -> Option<String> {
        let id = self.new_id(cx);
        if id != self.input_id && self.others.contains(&id) {
            return Some(id);
        }
        let mut seen = std::collections::HashSet::new();
        self.options.iter().map(|(_, _, st)| st.read(cx).value().to_string()).find(|v| !seen.insert(v.clone()))
    }
}

impl Render for FsIdsDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let grid = gpui_kit::rgb(0xaaaaaa);
        let multiple = !self.options.is_empty();
        let new_id = self.new_id(cx);
        let renames = self.renames(cx);
        let id_change = new_id != self.input_id;
        let option_change = !renames.is_empty();
        let errors = id_error(&new_id).is_some() || self.options.iter().any(|(_, _, st)| id_error(&st.read(cx).value()).is_some());
        // A duplicate only disables the button (live 10-20 n: no message).
        let can_confirm = self.duplicate(cx).is_none() && (id_change || option_change) && !errors;
        let cell = |w: f32| h_flex().w(px(w)).h(px(34.)).flex_none().px(px(4.)).border_r_1().border_b_1().border_color(grid).text_size(px(14.)).text_color(pal.text);
        let head = |w: f32, t: &str| cell(w).bg(gpui_kit::rgb(0xeaeaea)).justify_center().font_weight(FontWeight::BOLD).child(t.to_string());
        // An id cell: editable; while invalid only its tooltip says why (live
        // 10-20 n: no red text or border).
        let id_cell = |st: &Entity<InputState>, cx: &App| {
            let value = st.read(cx).value().to_string();
            let err = id_error(&value);
            cell(300.)
                .child(div().id(SharedString::from(format!("fsid-{}", st.entity_id()))).flex_1().when_some(err, |d, e| d.tooltip(crate::ui::tooltip(e))).child(Input::new(st).appearance(false).text_size(px(13.33))))
        };
        let table = |title: &str, name_head: &str, rows: Vec<(String, AnyElement)>| {
            let mut t = v_flex().border_l_1().border_t_1().border_color(grid).child(h_flex().child(head(200., name_head)).child(head(300., "Internal FeatureScript ID")));
            for (name, editor) in rows {
                t = t.child(h_flex().child(cell(200.).bg(gpui_kit::rgb(0xf5f5f5)).child(name)).child(editor));
            }
            v_flex().gap(px(4.)).when(multiple, |d| d.child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0x1651b0)).child(title.to_string()))).child(t)
        };
        let id_rows = vec![(self.input_name.clone(), id_cell(&self.id, cx).into_any_element())];
        let option_rows: Vec<(String, AnyElement)> = self.options.iter().map(|(_, name, st)| (name.clone(), id_cell(st, cx).into_any_element())).collect();
        let title = if multiple { "Edit FeatureScript IDs" } else { "Edit FeatureScript ID" };
        let info = format!(
            "{}This is typically only useful if you need to build {} inside a custom FeatureScript feature.",
            if multiple {
                format!("This dialog will change the internal FeatureScript identifiers of the input {}. ", self.input_name)
            } else {
                format!("This dialog will change the internal FeatureScript identifier of the input {}. ", self.input_name)
            },
            self.studio_name
        );
        let warning = |text: String| h_flex().gap(px(8.)).px(px(12.)).py(px(8.)).rounded(px(4.)).bg(pal.warn_fill).border_1().border_color(pal.warn_outline).text_size(px(13.)).text_color(pal.text).child(div().flex_1().min_w_0().child(text));
        let on_confirm = self.on_confirm.clone();
        v_flex()
            .w_full()
            .child(
                v_flex()
                    .px(px(16.))
                    .pt(px(16.))
                    .pb(px(12.))
                    .gap(px(8.))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::MEDIUM).text_color(gpui_kit::rgb(0x1651b0)).child(title))
                    .child(div().text_size(px(14.)).text_color(pal.text).child(info)),
            )
            .child(div().h(px(1.)).bg(gpui_kit::rgb(os::MODAL_DIVIDER)))
            .child(v_flex().p(px(16.)).gap(px(12.)).child(table("Input ID", "Name", id_rows)).when(multiple, |d| d.child(table("Row IDs", "Row name", option_rows))))
            .child(
                v_flex()
                    .px(px(16.))
                    .py(px(12.))
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(gpui_kit::rgb(os::MODAL_DIVIDER))
                    .when(can_confirm && id_change, |d| {
                        d.child(warning(format!(
                            "After this change, all assemblies, drawings, and other tabs which reference {0} will revert {0}'s {1} to the default value \"{2}\".",
                            self.studio_name, self.input_name, self.default_display
                        )))
                    })
                    .when(can_confirm && !id_change && option_change, |d| {
                        d.child(warning(format!("This change will break all references to {} in any assemblies, drawings, and other tabs which use a row of {} whose ID has changed.", self.studio_name, self.input_name)))
                    })
                    .child(
                        h_flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(os::primary_button("fsid-ok", if multiple { "Break references and change IDs" } else { "Break references and change ID" }).disabled(!can_confirm).on_click(move |_, window, cx| {
                                on_confirm(new_id.clone(), renames.clone(), cx);
                                window.close_dialog(cx);
                            }))
                            .child(os::secondary_button("fsid-cancel", "Cancel", cx).on_click(|_, window, cx| window.close_dialog(cx))),
                    ),
            )
    }
}
