//! Assembly tool search (Alt+C, "Search tools…" at the end of the
//! toolbar): Onshape's 600 px popover with the matching assembly tools on
//! the left and the selected tool's icon, name and description on the
//! right (same look as the Part Studio's).

use super::*;
use gpui_kit::component::input::InputEvent;

type Run = fn(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>);

/// (icon, name, shortcut, description, action)
struct AsmTool(&'static str, &'static str, Option<&'static str>, &'static str, Run);

const TOOLS: &[AsmTool] = &[
    AsmTool("update-all-button", "Update all references to latest versions", None, "Update all unpinned references to the latest versions or revisions.", |t, _, cx| t.open_reference_manager(cx)),
    AsmTool("insert", "Insert", Some("i"), "Insert parts, assemblies, sketches, or surfaces into this Assembly. Insert from this document, other documents, or standard content.", |t, w, cx| t.toggle_insert(w, cx)),
    AsmTool("mate-connector-assembly", "Mate connector", Some("ctrl+m"), "Create a mate connector on or between parts, surfaces, or sketches. Mate connectors are used to locate and orient entities within an assembly mate.", |t, w, cx| t.open_mc(None, w, cx)),
    AsmTool("fastened", "Fastened mate", Some("m"), "Remove all degrees of freedom between 2 entities.", |t, w, cx| t.start_mate(MateType::Fastened, w, cx)),
    AsmTool("revolute", "Revolute mate", Some("m"), "Allow rotation about the Z axis. The 1st selection serves as the rotational point, and the 2nd as the stationary point.", |t, w, cx| t.start_mate(MateType::Revolute, w, cx)),
    AsmTool("slider", "Slider mate", Some("m"), "Allow translation along the Z axis. The 1st selection serves as the sliding point, and the 2nd as the stationary point.", |t, w, cx| t.start_mate(MateType::Slider, w, cx)),
    AsmTool("planar", "Planar mate", Some("m"), "Allow translation along the X and Y axes, and rotation about the Z axis. The 1st selection serves as the movement point, and the 2nd as the stationary point.", |t, w, cx| t.start_mate(MateType::Planar, w, cx)),
    AsmTool("cylindrical", "Cylindrical mate", Some("m"), "Allow translation and rotation about the Z axis. The 1st selection serves as the translational and rotational point, and the 2nd as the stationary point.", |t, w, cx| t.start_mate(MateType::Cylindrical, w, cx)),
    AsmTool("pin-slot", "Pin slot mate", Some("m"), "Allow rotation about the Z axis and translation along the X axis. The 1st selection serves as the pin and the rotational movement point, and the 2nd selection serves as the translational movement.", |t, w, cx| t.start_mate(MateType::PinSlot, w, cx)),
    AsmTool("ball", "Ball mate", Some("m"), "Allow rotation about the X, Y, and Z axes. The 1st selection serves as the rotational movement point, and the 2nd selection serves as the stationary point.", |t, w, cx| t.start_mate(MateType::Ball, w, cx)),
    AsmTool("parallel-mate", "Parallel mate", Some("m"), "Allow 2 entities to rotate parallel to each other and translate freely.", |t, w, cx| t.start_mate(MateType::Parallel, w, cx)),
    AsmTool("group", "Group", None, "Lock the position of selected entities relative to one another.", |t, _, cx| t.create_group(cx)),
    AsmTool("snap-mode", "Snap mode", Some("shift+s"), "Drag an entity to snap to a Mate connector on another entity. Specify the mate type and orientation.", |t, w, cx| t.toggle_snap(w, cx)),
    AsmTool("show-mates", "Show mates mode", Some("h"), "Hover over or select an instance to show its mates.", |t, _, cx| t.toggle_show_mates_mode(cx)),
    AsmTool("replicate", "Replicate", None, "Using a seed instance as input, locate geometry identical to that which the seed is mated and replicate the seed on matching geometry.", |t, w, cx| t.open_pattern(PatternDialogKind::Replicate, w, cx)),
    AsmTool("replace-part", "Replace instance", None, "Replace instances with a selected part, surface, sketch, or subassembly.", |t, _, cx| t.replace_selected(cx)),
    AsmTool("linear-pattern", "Assembly linear pattern", None, "Create multiple instances of a part, surface, sketch, or subassembly. Specify up to 3 directions, distances, and number of instances.", |t, w, cx| t.open_pattern(PatternDialogKind::Linear, w, cx)),
    AsmTool("circular-pattern", "Assembly circular pattern", None, "Create multiple instances of a part, surface, sketch, or subassembly. Specify the axis, angle, and number of instances.", |t, w, cx| t.open_pattern(PatternDialogKind::Circular, w, cx)),
    AsmTool("mirror", "Assembly mirror", None, "Create mirrored instances of a part, surface or subassembly. Specify the mirror plane and select instances to mirror. Use mirror strategy table to control how instances are mirrored.", |t, w, cx| t.open_pattern(PatternDialogKind::Mirror, w, cx)),
    AsmTool("gear-relation", "Gear relation", None, "Create a constant ratio of angular rotation between 2 mates with rotational degrees of freedom.", |t, w, cx| t.open_relation(RelationType::Gear, None, w, cx)),
    AsmTool("rack-pinion", "Rack and pinion relation", None, "Relate a mate with a rotational degree of freedom to a mate with a linear degree of freedom.", |t, w, cx| t.open_relation(RelationType::RackAndPinion, None, w, cx)),
    AsmTool("screw-relation", "Screw relation", None, "Relate the rotational and translational degrees of freedom of a Cylindrical Mate.", |t, w, cx| t.open_relation(RelationType::Screw, None, w, cx)),
    AsmTool("linear-relation", "Linear relation", None, "Create a constant ratio of motion between 2 mates with linear degrees of freedom.", |t, w, cx| t.open_relation(RelationType::Linear, None, w, cx)),
    AsmTool("display-states", "Display states", None, "Assign a name to a specific display state of an assembly. Specify which parts, subassemblies, and mates are hidden or in view.", |t, w, cx| t.toggle_display_states(w, cx)),
    AsmTool("create-partstudio-incontext", "Create Part Studio in context", None, "Create a new Part Studio in the context of this assembly. The assembly appears in the Part Studio as context graphics that can be used for reference.", |t, _, cx| t.open_ps_in_context(cx)),
];

/// Onshape's tool search ranking (live dom/assembly-tool-search.txt): names
/// starting with the text, then names containing it, then names holding
/// its letters in order; shorter names first within each tier, then the
/// toolbar order; at most 10 results.
/// A toolbar tool's description (its tooltip's second line), by icon.
pub(super) fn tool_description(icon: &str) -> &'static str {
    TOOLS.iter().find(|t| t.0 == icon).map(|t| t.3).unwrap_or_else(|| match icon {
        "tangent-mate" => "Mate 2 entities tangentially, based on the selected faces, edges, or vertices.",
        "width-mate" => "Allow translation on a slot’s center plane and rotation about an axis perpendicular to that plane. The 1st selection defines the item to be centered. The 2nd selection defines the center plane.",
        _ => "",
    })
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
        (TOOLS[i].4)(self, window, cx);
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
                .child(div().text_size(px(14.)).line_height(px(21.)).text_color(pal.text).child(t.3))
        });
        let n = items.len();
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
                            .child(Input::new(&st).appearance(false).cleanable(true).text_size(px(14.)))
                            .when(self.ui.tool_query.is_empty(), |d| d.child(div().absolute().left(px(8.)).top_0().bottom_0().flex().items_center().text_size(px(14.)).italic().text_color(pal.text_faint).child("Search tools"))),
                    ),
                )
                .when(self.ui.tool_query.trim().is_empty(), |d| d.child(div().h(px(42.)).flex().items_center().justify_center().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(pal.text).child("Start typing to search tools")))
                .when(!self.ui.tool_query.trim().is_empty(), |d| {
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
