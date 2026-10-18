//! Versions and history panel: the document's version graph (every edit is
//! a microversion), named versions, branches, restore, merge and read-only
//! viewing of any point in history — Onshape's version graph, fully local.

use crate::doc_model::{DocEvent, DocModel};
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_store::{BranchId, MicroversionId, VersionGraph};

/// Branch lane colors (Onshape `--os-version-graph-branch-0..5`).
const LANES: [u32; 6] = [0x1651b0, 0xfab601, 0x3ad4a7, 0xb689be, 0x3e94ff, 0xb88a00];

enum Prompt {
    None,
    CreateVersion(Entity<InputState>),
    Branch(Entity<InputState>),
}

pub struct HistoryPanel {
    doc: Entity<DocModel>,
    graph: Option<VersionGraph>,
    prompt: Prompt,
    /// Result of "Compare with current": (description of the older point, diff).
    compare: Option<(String, Vec<noshape_model::compare::TabDiff>)>,
    _subs: Vec<Subscription>,
}

impl HistoryPanel {
    pub fn new(doc: Entity<DocModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subs = vec![cx.subscribe_in(&doc, window, |this: &mut Self, _, _ev: &DocEvent, _, cx| {
            this.graph = None;
            cx.notify();
        })];
        Self { doc, graph: None, prompt: Prompt::None, compare: None, _subs: subs }
    }

    fn graph(&mut self, cx: &App) -> VersionGraph {
        if self.graph.is_none() {
            self.graph = self.doc.read(cx).session.store.graph().ok();
        }
        self.graph.clone().unwrap_or(VersionGraph { microversions: vec![], branches: vec![], versions: vec![] })
    }

    fn after_change(&mut self, cx: &mut Context<Self>) {
        self.graph = None;
        self.prompt = Prompt::None;
        self.doc.update(cx, |d, cx| d.reloaded(cx));
        cx.notify();
    }

    fn error(&mut self, e: anyhow::Error, cx: &mut Context<Self>) {
        self.doc.update(cx, |_, cx| cx.emit(DocEvent::Error(format!("{e:#}"))));
    }

    fn view_mv(&mut self, mv: Option<MicroversionId>, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.view_microversion(mv));
        match r {
            Ok(()) => self.after_change(cx),
            Err(e) => self.error(e, cx),
        }
    }

    /// Compare a point in history with the current branch head.
    fn compare_with_current(&mut self, mv: MicroversionId, label: String, cx: &mut Context<Self>) {
        let r = (|| -> anyhow::Result<Vec<noshape_model::compare::TabDiff>> {
            let s = &self.doc.read(cx).session;
            let old = noshape_model::document::Document::from_blobs(&s.store.read_all(mv)?)?;
            let new = noshape_model::document::Document::from_blobs(&s.store.read_all(s.head()?)?)?;
            Ok(noshape_model::compare::compare_documents(&old, &new))
        })();
        match r {
            Ok(d) => self.compare = Some((label, d)),
            Err(e) => self.error(e, cx),
        }
        cx.notify();
    }

    fn render_compare(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        use noshape_model::compare::ChangeKind;
        let (label, diffs) = self.compare.as_ref()?;
        let pal = p();
        let color = |k: &ChangeKind| match k {
            ChangeKind::Added => pal.success,
            ChangeKind::Removed => pal.error,
            ChangeKind::Modified => pal.warning,
        };
        let sign = |k: &ChangeKind| match k {
            ChangeKind::Added => "+",
            ChangeKind::Removed => "−",
            ChangeKind::Modified => "~",
        };
        let mut body: Vec<AnyElement> = Vec::new();
        if diffs.is_empty() {
            body.push(div().text_xs().text_color(pal.text_faint).child("No differences.").into_any_element());
        }
        for d in diffs {
            body.push(
                h_flex()
                    .gap_1p5()
                    .pt_1()
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(color(&d.status)).child(sign(&d.status)))
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(d.tab.clone()))
                    .child(div().text_xs().text_color(pal.text_faint).child(d.kind_name))
                    .into_any_element(),
            );
            for c in &d.changes {
                body.push(
                    h_flex()
                        .pl_3()
                        .gap_1p5()
                        .items_start()
                        .child(div().text_xs().text_color(color(&c.kind)).child(sign(&c.kind)))
                        .child(div().text_xs().text_color(pal.text_muted).child(c.text.clone()))
                        .into_any_element(),
                );
            }
        }
        Some(
            v_flex()
                .max_h(px(320.))
                .border_b_1()
                .border_color(pal.border)
                .child(
                    h_flex()
                        .px_2()
                        .pt_2()
                        .justify_between()
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("COMPARE WITH CURRENT"))
                        .child(Button::new("cmp-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.compare = None;
                            cx.notify();
                        }))),
                )
                .child(div().px_2().text_xs().text_color(pal.text_faint).truncate().child(format!("From: {label}")))
                .child(v_flex().id("cmp-body").px_2().pb_2().overflow_y_scroll().children(body))
                .into_any_element(),
        )
    }

    fn restore(&mut self, mv: MicroversionId, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.restore(mv));
        match r {
            Ok(()) => self.after_change(cx),
            Err(e) => self.error(e, cx),
        }
    }

    fn switch_branch(&mut self, b: BranchId, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.switch_branch(b));
        match r {
            Ok(()) => self.after_change(cx),
            Err(e) => self.error(e, cx),
        }
    }

    fn merge(&mut self, from: BranchId, cx: &mut Context<Self>) {
        let r = self.doc.update(cx, |d, _| d.session.merge_from(from));
        match r {
            Ok(outcome) => {
                let msg = if outcome.up_to_date {
                    "Already up to date".to_string()
                } else if outcome.conflicts.is_empty() {
                    format!("Merged {} tab(s)", outcome.taken_from_source.len())
                } else {
                    format!("Merged; {} tab(s) changed on both branches were merged feature by feature", outcome.conflicts.len())
                };
                self.after_change(cx);
                self.doc.update(cx, |_, cx| cx.emit(DocEvent::Info(msg)));
            }
            Err(e) => self.error(e, cx),
        }
    }

    fn start_prompt_version(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.graph(cx).versions.len() + 1;
        let st = cx.new(|cx| InputState::new(window, cx).default_value(format!("V{n}")));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let name = st.read(cx).value().to_string();
                let r = this.doc.update(cx, |d, _| d.session.create_version(name.trim(), ""));
                match r {
                    Ok(_) => this.after_change(cx),
                    Err(e) => this.error(e, cx),
                }
            }
        });
        self._subs.push(sub);
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.prompt = Prompt::CreateVersion(st);
        cx.notify();
    }

    fn start_prompt_branch(&mut self, from: MicroversionId, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.graph(cx).branches.len();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(format!("Branch {n}")));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                let name = st.read(cx).value().to_string();
                let r = this.doc.update(cx, |d, _| d.session.branch_from(from, name.trim()));
                match r {
                    Ok(_) => this.after_change(cx),
                    Err(e) => this.error(e, cx),
                }
            }
        });
        self._subs.push(sub);
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.prompt = Prompt::Branch(st);
        cx.notify();
    }
}

fn relative_time(ms: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(ms);
    let s = ((now - ms) / 1000).max(0);
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86399 => format!("{} h ago", s / 3600),
        _ => format!("{} d ago", s / 86400),
    }
}

impl Render for HistoryPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let graph = self.graph(cx);
        let session = &self.doc.read(cx).session;
        let current_branch = session.branch;
        let viewing = session.viewing;
        let head = session.head().ok();
        let branches: Vec<_> = graph.branches.iter().filter(|b| !b.deleted).cloned().collect();
        let lane_of = |b: BranchId| branches.iter().position(|x| x.id == b).unwrap_or(0) % LANES.len();
        let current_name = branches.iter().find(|b| b.id == current_branch).map(|b| b.name.clone()).unwrap_or_else(|| "Main".into());
        let _ = window;

        let view = cx.entity();
        let branch_menu = {
            let branches = branches.clone();
            let view = view.clone();
            Button::new("branch-select").label(current_name.clone()).outline().xsmall().dropdown_caret(true).dropdown_menu(move |mut menu, _, _| {
                menu = menu.label("Switch to branch");
                for b in &branches {
                    let v = view.clone();
                    let id = b.id;
                    menu = menu.item(PopupMenuItem::new(b.name.clone()).checked(b.id == current_branch).on_click(move |_, _, cx| v.update(cx, |t, cx| t.switch_branch(id, cx))));
                }
                menu
            })
        };
        let merge_menu = {
            let others: Vec<_> = branches.iter().filter(|b| b.id != current_branch).cloned().collect();
            let view = view.clone();
            Button::new("merge").label("Merge").outline().xsmall().dropdown_caret(true).dropdown_menu(move |mut menu, _, _| {
                if others.is_empty() {
                    menu = menu.label("No other branches");
                }
                for b in &others {
                    let v = view.clone();
                    let id = b.id;
                    menu = menu.item(PopupMenuItem::new(format!("Merge {} into current", b.name)).on_click(move |_, _, cx| v.update(cx, |t, cx| t.merge(id, cx))));
                }
                menu
            })
        };

        let mut rows: Vec<AnyElement> = Vec::new();
        for mv in graph.microversions.iter().rev().take(400) {
            let lane = lane_of(mv.branch);
            let color = rgb(LANES[lane]);
            let versions: Vec<String> = graph.versions.iter().filter(|v| v.microversion == mv.id).map(|v| v.name.clone()).collect();
            let is_head = Some(mv.id) == head;
            let is_viewing = viewing == Some(mv.id);
            let id = mv.id;
            let view2 = view.clone();
            let label = mv.description.clone();
            let row = h_flex()
                .id(SharedString::from(format!("mv-{}", mv.id.short())))
                .w_full()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_sm()
                .cursor_pointer()
                .when(is_viewing, |d| d.bg(pal.accent_soft))
                .hover(|d| d.bg(pal.panel_hover))
                .child(div().w(px(6. + lane as f32 * 10.)).flex_none().h_full())
                .child(div().size(px(9.)).flex_none().rounded_full().bg(color).when(mv.parents.len() > 1, |d| d.border_2().border_color(pal.text)))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_xs().text_color(pal.text).truncate().child(mv.description.clone()))
                        .child(div().text_xs().text_color(pal.text_faint).child(format!("{} · {}", mv.author, relative_time(mv.created)))),
                )
                .children(versions.into_iter().map(|n| div().px_1p5().rounded_sm().bg(with_alpha(pal.success, 0.18)).text_xs().text_color(pal.success).child(n)))
                .when(is_head, |d| d.child(div().text_xs().text_color(pal.accent).child("current")))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.view_mv(if is_head { None } else { Some(id) }, cx)))
                .context_menu(move |menu, _, _| {
                    let v1 = view2.clone();
                    let v2 = view2.clone();
                    let v3 = view2.clone();
                    let v4 = view2.clone();
                    let label = label.clone();
                    menu.item(PopupMenuItem::new("View this point (read-only)").on_click(move |_, _, cx| v1.update(cx, |t, cx| t.view_mv(Some(id), cx))))
                        .item(PopupMenuItem::new("Restore to here").on_click(move |_, _, cx| v2.update(cx, |t, cx| t.restore(id, cx))))
                        .item(PopupMenuItem::new("Branch from here…").on_click(move |_, window, cx| v3.update(cx, |t, cx| t.start_prompt_branch(id, window, cx))))
                        .item(PopupMenuItem::new("Compare with current").on_click(move |_, _, cx| v4.update(cx, |t, cx| t.compare_with_current(id, label.clone(), cx))))
                });
            rows.push(row.into_any_element());
        }

        let prompt: Option<AnyElement> = match &self.prompt {
            Prompt::None => None,
            Prompt::CreateVersion(st) => Some(
                v_flex()
                    .gap_1()
                    .p_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_xs().text_color(pal.text_muted).child("New version name (Enter to create)"))
                    .child(Input::new(st).xsmall())
                    .into_any_element(),
            ),
            Prompt::Branch(st) => Some(
                v_flex()
                    .gap_1()
                    .p_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_xs().text_color(pal.text_muted).child("New branch name (Enter to create)"))
                    .child(Input::new(st).xsmall())
                    .into_any_element(),
            ),
        };

        v_flex()
            .size_full()
            .bg(pal.panel)
            .border_l_1()
            .border_color(pal.border)
            .child(
                h_flex()
                    .h(px(32.))
                    .px_3()
                    .justify_between()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("VERSIONS & HISTORY")),
            )
            .child(
                h_flex()
                    .p_2()
                    .gap_1()
                    .flex_wrap()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(branch_menu)
                    .child(merge_menu)
                    .child(Button::new("create-version").label("Version").icon(IconName::Plus).primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.start_prompt_version(window, cx))))
                    .when(viewing.is_some(), |d| {
                        d.child(Button::new("return").label("Return to workspace").outline().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.view_mv(None, cx))))
                    }),
            )
            .children(prompt)
            .children(self.render_compare(cx))
            .child(v_flex().id("history-rows").flex_1().p_1().gap_px().overflow_y_scroll().children(rows))
    }
}
