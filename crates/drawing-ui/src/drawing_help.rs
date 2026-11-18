//! A drawing toolbar tool's help popover (live2 toolbar-help-1118): about 3.1 s after the pointer rests on a tool (its
//! title tooltip shows at about 0.47 s), a 400 px card at the drawing frame's left edge just under the toolbar, over the
//! tooltip: the tool's name in a blue-grey title bar, then its help, steps numbered. It stays while the pointer rests.
//! Undo and Redo have none.

use crate::drawing_view::DrawingEditor;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use gpui_kit::component::{h_flex, v_flex};
use std::time::Duration;

/// From the pointer resting on a tool to its help (live 3.05–3.24 s).
pub const HELP_DELAY: Duration = Duration::from_millis(3100);
const WIDTH: f32 = 400.;

/// A tool's help: its first paragraph, then numbered steps.
pub struct ToolHelp {
    pub intro: &'static str,
    pub steps: &'static [&'static str],
}

/// Tools by their tooltip without the shortcut ("Projected view").
#[cfg(not(test))]
static HELP: &[(&str, ToolHelp)] = &[];
#[cfg(test)]
static HELP: &[(&str, ToolHelp)] = &[("Projected view", ToolHelp { intro: "Test help.", steps: &["First step.", "Second step."] })];

/// The help of the tool whose tooltip is `tip` (its shortcut dropped), if it has one.
pub fn help(tip: &str) -> Option<&'static ToolHelp> {
    let name = tip.split(" (").next().unwrap_or(tip);
    HELP.iter().find(|(t, _)| *t == name).map(|(_, h)| h)
}

/// The toolbar tool under the pointer and whether its help shows yet.
pub struct HelpHover {
    pub tip: &'static str,
    pub shown: bool,
    _timer: Task<()>,
}

impl DrawingEditor {
    /// The pointer arrived on (`on`) or left a toolbar tool.
    pub fn tool_hover(&mut self, tip: &'static str, on: bool, cx: &mut Context<Self>) {
        let same = self.help_hover.as_ref().is_some_and(|h| h.tip == tip);
        if on == same {
            return;
        }
        if !on || help(tip).is_none() {
            if self.help_hover.take().is_some_and(|h| h.shown) {
                cx.notify();
            }
            return;
        }
        let shown = self.help_hover.take().is_some_and(|h| h.shown);
        let timer = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HELP_DELAY).await;
            let _ = this.update(cx, |this, cx| {
                if let Some(h) = &mut this.help_hover
                    && h.tip == tip
                {
                    h.shown = true;
                    cx.notify();
                }
            });
        });
        self.help_hover = Some(HelpHover { tip, shown: false, _timer: timer });
        if shown {
            cx.notify();
        }
    }

    /// The shown help popover, placed under the toolbar at the editor's left edge.
    pub fn render_tool_help(&self) -> Option<AnyElement> {
        let tip = self.help_hover.as_ref().filter(|h| h.shown)?.tip;
        let h = help(tip)?;
        let name = tip.split(" (").next().unwrap_or(tip);
        crate::ui_script::note_tooltip(format!("{name} {} {}", h.intro, h.steps.join(" ")).trim());
        let border = gpui_kit::rgb(0xc9cccf);
        let title = div().h(px(25.)).px(px(8.)).flex().items_center().bg(gpui_kit::rgb(0xcfdbe5)).border_b_1().border_color(gpui_kit::rgb(0xebebeb)).font_weight(FontWeight::BOLD).child(name);
        let steps = h.steps.iter().enumerate().map(|(i, s)| h_flex().items_start().child(div().w(px(24.)).flex_none().flex().justify_end().pr(px(4.)).child(format!("{}.", i + 1))).child(div().flex_1().child(*s)));
        let body = v_flex().px(px(8.)).py(px(6.)).line_height(px(17.)).when(!h.intro.is_empty(), |d| d.child(h.intro)).children(steps);
        Some(
            div()
                .absolute()
                .top(px(crate::ui::TOOLBAR_H + 1.))
                .left_0()
                .w(px(WIDTH))
                .debug_selector(|| "tool-help".into())
                .bg(gpui_kit::rgb(0xffffff))
                .border_1()
                .border_color(border)
                .rounded(px(4.))
                .overflow_hidden()
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_size(px(12.))
                .text_color(gpui_kit::rgb(0x333333))
                .child(title)
                .child(body)
                .into_any_element(),
        )
    }
}
