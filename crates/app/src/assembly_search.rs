//! Assembly tool search (Alt+C, "Search tools…" at the end of the
//! toolbar): Onshape's 600 px popover with the matching assembly tools on
//! the left and the selected tool's icon, name and description on the
//! right (same look as the Part Studio's).

use super::*;
use gpui_kit::component::input::InputEvent;

type Run = fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>);

/// (icon, name, shortcut, action); the description is live's, by name
/// (`tool_text::ASSEMBLY`).
struct AsmTool(&'static str, &'static str, Option<&'static str>, Run);

const TOOLS: &[AsmTool] = &[
    AsmTool("update-all-button", "Update all references to latest versions", None, |t, _, cx| t.open_reference_manager(None, cx)),
    AsmTool("insert", "Insert", Some("i"), |t, w, cx| t.toggle_insert(w, cx)),
    AsmTool("mate-connector-assembly", "Mate connector", Some("ctrl+m"), |t, w, cx| t.open_mc(None, w, cx)),
    AsmTool("fastened", "Fastened mate", Some("m"), |t, w, cx| t.start_mate(MateType::Fastened, w, cx)),
    AsmTool("revolute", "Revolute mate", Some("m"), |t, w, cx| t.start_mate(MateType::Revolute, w, cx)),
    AsmTool("slider", "Slider mate", Some("m"), |t, w, cx| t.start_mate(MateType::Slider, w, cx)),
    AsmTool("planar", "Planar mate", Some("m"), |t, w, cx| t.start_mate(MateType::Planar, w, cx)),
    AsmTool("cylindrical", "Cylindrical mate", Some("m"), |t, w, cx| t.start_mate(MateType::Cylindrical, w, cx)),
    AsmTool("pin-slot", "Pin slot mate", Some("m"), |t, w, cx| t.start_mate(MateType::PinSlot, w, cx)),
    AsmTool("ball", "Ball mate", Some("m"), |t, w, cx| t.start_mate(MateType::Ball, w, cx)),
    AsmTool("parallel-mate", "Parallel mate", Some("m"), |t, w, cx| t.start_mate(MateType::Parallel, w, cx)),
    AsmTool("group", "Group", None, |t, _, cx| t.create_group(cx)),
    AsmTool("snap-mode", "Snap mode", Some("shift+s"), |t, w, cx| t.toggle_snap(w, cx)),
    AsmTool("show-mates", "Show mates mode", Some("h"), |t, _, cx| t.toggle_show_mates_mode(cx)),
    AsmTool("replicate", "Replicate", None, |t, w, cx| t.open_pattern(PatternDialogKind::Replicate, w, cx)),
    AsmTool("replace-part", "Replace instance", None, |t, _, cx| t.replace_selected(cx)),
    AsmTool("linear-pattern", "Assembly linear pattern", None, |t, w, cx| t.open_pattern(PatternDialogKind::Linear, w, cx)),
    AsmTool("circular-pattern", "Assembly circular pattern", None, |t, w, cx| t.open_pattern(PatternDialogKind::Circular, w, cx)),
    AsmTool("mirror", "Assembly mirror", None, |t, w, cx| t.open_pattern(PatternDialogKind::Mirror, w, cx)),
    AsmTool("gear-relation", "Gear relation", None, |t, w, cx| t.open_relation(RelationType::Gear, None, w, cx)),
    AsmTool("rack-pinion", "Rack and pinion relation", None, |t, w, cx| t.open_relation(RelationType::RackAndPinion, None, w, cx)),
    AsmTool("screw-relation", "Screw relation", None, |t, w, cx| t.open_relation(RelationType::Screw, None, w, cx)),
    AsmTool("linear-relation", "Linear relation", None, |t, w, cx| t.open_relation(RelationType::Linear, None, w, cx)),
    AsmTool("display-states", "Display states", None, |t, w, cx| t.toggle_display_states(w, cx)),
    AsmTool("create-partstudio-incontext", "Create Part Studio in context", None, |t, _, cx| t.open_ps_in_context(cx)),
];

/// Onshape's tool search ranking (live dom/assembly-tool-search.txt): names
/// starting with the text, then names containing it, then names holding
/// its letters in order; shorter names first within each tier, then the
/// toolbar order; at most 10 results.
/// A toolbar tool's description (its tooltip's second line), by icon.
pub(super) fn tool_description(icon: &str) -> &'static str {
    let title = TOOLS.iter().find(|t| t.0 == icon).map(|t| t.1).unwrap_or(match icon {
        "tangent-mate" => "Tangent mate",
        "width-mate" => "Width mate",
        _ => "",
    });
    crate::tool_text::assembly_description(title).unwrap_or_default()
}

fn matching(query: &str) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let subsequence = |n: &str| {
        let mut it = n.chars();
        q.chars().filter(|c| !c.is_whitespace()).all(|c| it.any(|x| x == c))
    };
    let mut hits: Vec<(u8, usize, usize)> = TOOLS
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let n = t.1.to_lowercase();
            let tier = if n.starts_with(&q) {
                0
            } else if n.contains(&q) {
                1
            } else if subsequence(&n) {
                2
            } else {
                return None;
            };
            Some((tier, n.chars().count(), i))
        })
        .collect();
    hits.sort();
    hits.into_iter().take(10).map(|(_, _, i)| i).collect()
}

impl AssemblyView {
    pub(super) fn open_tool_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.tool_search.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                this.ui.tool_query = st.read(cx).value().to_string();
                this.ui.tool_sel = 0;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                let m = matching(&this.ui.tool_query);
                if let Some(&i) = m.get(this.ui.tool_sel).or(m.first()) {
                    this.run_tool(i, window, cx);
                }
            }
            _ => {}
        });
        // Focus once the field exists (focusing before its first paint is lost).
        let st2 = st.clone();
        window.defer(cx, move |window, cx| st2.update(cx, |s, cx| s.focus(window, cx)));
        self.ui.tool_query.clear();
        self.ui.tool_sel = 0;
        self.ui.tool_search = Some((st, sub));
        cx.notify();
    }

    pub(super) fn close_tool_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.tool_search = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// The icon of the tool the search has selected, for the yellow toolbar
    /// highlight ("Highlight tool in toolbar").
    pub(super) fn search_found_icon(&self) -> Option<&'static str> {
        if self.ui.tool_search.is_none() || self.ui.tool_no_highlight {
            return None;
        }
        let m = matching(&self.ui.tool_query);
        m.get(self.ui.tool_sel).or(m.first()).map(|&i| TOOLS[i].0)
    }

    fn run_tool(&mut self, i: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tool_search(window, cx);
        (TOOLS[i].3)(self, window, cx);
    }

    pub(super) fn render_tool_search(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (st, _) = self.ui.tool_search.as_ref()?;
        let st = st.clone();
        let pal = p();
        let items = matching(&self.ui.tool_query);
        let sel = self.ui.tool_sel.min(items.len().saturating_sub(1));
        let chip = |t: &str| div().px(px(3.)).h(px(18.)).flex().items_center().border_1().border_color(rgb(0xaaaaaa)).rounded(px(2.)).bg(rgb(0xf5f5f5)).text_size(px(11.)).text_color(pal.text).child(t.to_string());
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(row, &i)| {
                let t = &TOOLS[i];
                h_flex()
                    .id(SharedString::from(format!("atool-{row}")))
                    .h(px(34.))
                    .flex_none()
                    .px(px(4.))
                    .gap(px(6.))
                    .rounded(px(2.))
                    .cursor_pointer()
                    .when(row == sel, |d| d.bg(pal.accent_soft))
                    .when(row != sel, |d| d.hover(|d| d.bg(pal.row_hover)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if *h && this.ui.tool_sel != row {
                            this.ui.tool_sel = row;
                            cx.notify();
                        }
                    }))
                    .child(crate::assets::icon(&icon_path(t.0), 20., cx))
                    .child(div().flex_1().min_w_0().truncate().text_size(px(14.)).text_color(pal.text).child(t.1))
                    .children(t.2.map(|s| h_flex().children(s.split('+').map(chip))))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| this.run_tool(i, window, cx)))
                    .into_any_element()
            })
            .collect();
        let detail = items.get(sel).map(|&i| {
            let t = &TOOLS[i];
            v_flex()
                .flex_1()
                .min_w_0()
                .pl(px(12.))
                .pt(px(4.))
                .gap(px(10.))
                .child(h_flex().gap(px(6.)).child(crate::assets::icon(&icon_path(t.0), 28., cx)).child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(t.1)))
                .child(div().text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child(crate::ui::tool_description(crate::tool_text::assembly_description(t.1).unwrap_or_default())))
        });
        let n = items.len();
        // "Did you mean:" + a pill per tool the search stands for (as in the
        // Part Studio, bundle CommandSuggestions); a pill searches for it.
        let suggested = crate::tool_synonyms::suggestions(crate::tool_synonyms::ASSEMBLY, &self.ui.tool_query, n > 0);
        let st3 = st.clone();
        let suggestions = (!suggested.is_empty() && !self.ui.tool_query.trim().is_empty()).then(|| {
            h_flex()
                .px(px(14.))
                .pt(px(10.))
                // Live menus/tool-search-export.png: 24 px down to the results.
                .pb(px(24.))
                .gap(px(10.))
                .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Did you mean:"))
                .children(suggested.into_iter().enumerate().map(|(i, name)| {
                    let st = st3.clone();
                    div()
                        .id(SharedString::from(format!("asm-suggest-{i}")))
                        .px(px(10.))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x999999))
                        .bg(rgb(0xffffff))
                        .cursor_pointer()
                        .hover(|d| d.bg(rgb(0xeaeaea)))
                        .text_size(px(12.))
                        .text_color(pal.text)
                        .child(name)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            st.update(cx, |s, cx| s.set_value(name, window, cx));
                            this.ui.tool_query = name.to_string();
                            this.ui.tool_sel = 0;
                            cx.notify();
                        }))
                }))
        });
        Some(
            div()
                .id("asm-tool-search")
                .absolute()
                .top(px(10.))
                .right(px(0.))
                .w(px(600.))
                .occlude()
                .bg(rgb(0xf5f5f5))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| this.close_tool_search(window, cx)))
                .capture_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
                    match ev.keystroke.key.as_str() {
                        "escape" => this.close_tool_search(window, cx),
                        "down" if n > 0 => this.ui.tool_sel = (this.ui.tool_sel + 1).min(n - 1),
                        "up" => this.ui.tool_sel = this.ui.tool_sel.saturating_sub(1),
                        _ => return,
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
                .child(
                    div().bg(rgb(0xffffff)).p(px(6.)).child(
                        div()
                            .relative()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(pal.focus)
                            .child(crate::ui::input_with_placeholder(&st, "Search tools", cx).appearance(false).cleanable(true).text_size(px(14.)).pl(px(5.))),
                    ),
                )
                .when(self.ui.tool_query.trim().is_empty(), |d| d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Start typing to search tools")))
                .children(suggestions)
                .when(!self.ui.tool_query.trim().is_empty() && n == 0, |d| {
                    d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).text_color(pal.text).child("No items match your search"))
                })
                .when(!self.ui.tool_query.trim().is_empty() && n > 0, |d| {
                    d.child(
                        h_flex()
                            .items_start()
                            .px(px(10.))
                            .py(px(14.))
                            .child(v_flex().w(px(198.)).flex_none().gap(px(2.)).children(rows))
                            .child(div().w(px(1.)).ml(px(12.)).h(px(142.)).bg(rgb(0xcccccc)))
                            .children(detail),
                    )
                })
                .child(
                    h_flex()
                        .h(px(34.))
                        .px(px(5.))
                        .gap(px(2.))
                        .bg(rgb(0xeeeeee))
                        .text_size(px(12.))
                        .text_color(pal.text_muted)
                        .child("Return to this dialog at any time by pressing")
                        .child(div().w(px(3.)))
                        .child(chip("alt/⌥"))
                        .child(chip("c"))
                        .child(div().flex_1())
                        .child(crate::ui::os_checkbox("atool-highlight", "Highlight tool in toolbar", !self.ui.tool_no_highlight).text_size(px(12.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.tool_no_highlight = !this.ui.tool_no_highlight;
                            cx.notify();
                        }))),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    /// Every assembly tool shows live's description.
    #[test]
    fn every_tool_has_its_live_description() {
        for t in super::TOOLS {
            assert!(crate::tool_text::assembly_description(t.1).is_some(), "{}", t.1);
        }
        assert!(!super::tool_description("tangent-mate").is_empty() && !super::tool_description("width-mate").is_empty());
    }

    use super::matching;

    #[test]
    fn tool_search_ranks_like_onshape() {
        let names = |q: &str| matching(q).into_iter().map(|i| super::TOOLS[i].1).collect::<Vec<_>>();
        assert!(names("").is_empty());
        // Live order (dom/assembly-tool-search.txt).
        assert_eq!(names("mate"), vec!["Mate connector", "Ball mate", "Slider mate", "Planar mate", "Fastened mate", "Revolute mate", "Pin slot mate", "Parallel mate", "Show mates mode", "Cylindrical mate"]);
        assert_eq!(names("pattern"), vec!["Assembly linear pattern", "Assembly circular pattern", "Update all references to latest versions"]);
        assert_eq!(names("gear"), vec!["Gear relation"]);
        assert_eq!(names("insert"), vec!["Insert"]);
        assert_eq!(names("group"), vec!["Group"]);
        assert_eq!(names("context"), vec!["Create Part Studio in context"]);
        assert_eq!(names("display"), vec!["Display states"]);
    }
}

