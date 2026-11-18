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
/// From the pointer resting on a tool to its title tooltip (live2 toolbar-help-1118: 0.45–0.69 s, median 0.47 s). Ours
/// read later (qa DRW-TOOLBAR-TOOLTIP-DELAY) by the kit's fade starting at 0 opacity, one more frame, now gone.
pub const TIP_DELAY: Duration = Duration::from_millis(450);
const WIDTH: f32 = 400.;

/// A tool's help: its first paragraph, then numbered steps.
pub struct ToolHelp {
    pub intro: &'static str,
    pub steps: &'static [&'static str],
}

/// Tools by their tooltip without the shortcut ("Projected view"): live's titles, step counts and facts in our own
/// words (DECISIONS 11-18: help popover texts).
static HELP: &[(&str, ToolHelp)] = &[
    ("Update from this workspace", ToolHelp {
        intro: "This button lights up when the design behind the drawing has changed and the drawing can catch up. A click brings every view and property on every sheet up to date, dependent views such as section and detail views included. To go back to the drawing as it was, restore the previous entry in the version history.",
        steps: &[],
    }),
    ("Insert view", ToolHelp {
        intro: "Places a view of a Part Studio or Assembly.",
        steps: &[
            "Pick a part, sheet metal flat pattern, sketch or assembly from this workspace, from a version or from another document.",
            "Or take a part from what is already selected.",
            "Pick a view of the chosen reference.",
            "Pick the view's scale.",
            "Click on the sheet to place the view.",
        ],
    }),
    ("Projected view", ToolHelp {
        intro: "Places a projected view of a view already on the sheet.",
        steps: &[
            "Select the existing view.",
            "Drag the new view into place.",
            "The projection follows the drawing's standard: third angle projection in ANSI, first angle projection in ISO.",
        ],
    }),
    ("Auxiliary view", ToolHelp {
        intro: "Makes a view normal to, in line with or aligned to an axis along a selected edge.",
        steps: &[
            "Select a straight edge, an isometric circle or fillet edge, a centerline or a sketch line.",
            "Drag the view into place.",
        ],
    }),
    ("Section view", ToolHelp {
        intro: "Makes a horizontal, vertical or angular section of a view on the sheet.",
        steps: &[
            "In the section dialog, pick horizontal, vertical or angular.",
            "Click points to set where the cutting line runs.",
            "An angular section needs two points for its cutting line.",
            "Set a view depth if you want one.",
            "Move the pointer out of the view to see a preview.",
            "Click to place the section view.",
            "A cutting line can have jogs.",
        ],
    }),
    ("Aligned section view", ToolHelp {
        intro: "Makes an aligned section of a view on the sheet.",
        steps: &[
            "Select the center point of the cutting line.",
            "Select a point that places the cutting line's part on the viewing plane.",
            "Select a point that places the cutting line's part on the revolved plane.",
            "Move the pointer out of the view to see a preview.",
            "Click to place the section view.",
        ],
    }),
    ("Broken-out section", ToolHelp {
        intro: "Cuts a closed profile out of a view on the sheet, down to a set depth.",
        steps: &[
            "Sketch a closed profile, or select one.",
            "Pick an end condition.",
            "Set the depth.",
        ],
    }),
    ("Detail view", ToolHelp {
        intro: "Makes a detail view of one region of a view on the sheet.",
        steps: &[
            "Pick the profile's shape: circle, rectangle, closed spline or closed polygon.",
            "Draw the profile inside the view you want to detail.",
            "Click on the sheet to place the detail view.",
            "When the model's geometry changes, the detail view follows.",
        ],
    }),
    ("Break view", ToolHelp {
        intro: "Shortens a view on the sheet by taking out a part of it with a regular break.",
        steps: &[
            "Pick a horizontal or a vertical break.",
            "Pick the break line style and the gap.",
            "Place the break lines in the view.",
            "When the model's geometry changes, the broken view follows.",
        ],
    }),
    ("Crop view", ToolHelp {
        intro: "Crops a view on the sheet to a closed profile.",
        steps: &[
            "Sketch a closed profile, or select one.",
        ],
    }),
    ("Dimension", ToolHelp {
        intro: "Dimensions any kind of curve.",
        steps: &[
            "Select two points or edges, or an arc or circle.",
            "Move the pointer and click where the dimension goes.",
        ],
    }),
    ("Ordinate dimension", ToolHelp {
        intro: "Places a group of ordinate dimensions on a drawing view.",
        steps: &[
            "Select the origin point: it gets the zero dimension.",
            "Drag, then click where the origin's text goes.",
            "Select more points to add them to the group.",
            "Press Enter to stop placing dimensions.",
            "To change a group later, right-click any of its ordinates and pick Add to ordinate.",
            "Deleting the origin deletes the whole group.",
        ],
    }),
    ("Hole/thread callout", ToolHelp {
        intro: "Labels a hole or thread with a dimensional callout.",
        steps: &[
            "Select a circle that belongs to a hole or thread feature.",
            "Drag the callout into place.",
        ],
    }),
    ("Datum", ToolHelp {
        intro: "Places a datum attached to an entity.",
        steps: &[
            "Pick datum or datum target.",
            "Select the entity the datum attaches to.",
            "Click to place the datum.",
            "For a datum target, set its origin, its X and Y directions and its distance.",
        ],
    }),
    ("Geometric tolerance", ToolHelp {
        intro: "Places a feature control frame for a geometric tolerance.",
        steps: &[
            "Fill in the control frames in the dialog.",
            "Select a point on a view's edge to start a leader, or a spot away from edges to place the frame on its own.",
        ],
    }),
    ("Surface finish symbol", ToolHelp {
        intro: "Places a surface finish symbol.",
        steps: &[
            "Fill in the surface finish in the dialog.",
            "Select a straight edge, a point on a curved edge or a spot on the sheet for the symbol to belong to.",
            "Click to place the symbol.",
        ],
    }),
    ("Weld symbol", ToolHelp {
        intro: "Places a weld symbol.",
        steps: &[
            "Fill in the weld symbol in the dialog.",
            "Select the entity the symbol belongs to.",
            "Click to place the symbol.",
        ],
    }),
    ("Edge condition symbol", ToolHelp {
        intro: "Places an edge condition symbol (ISO 13715).",
        steps: &[
            "Fill in the edge condition in the dialog.",
            "Select an edge, a corner, a sketch entity or a spot on the sheet.",
            "Click to place the symbol.",
        ],
    }),
    ("Note", ToolHelp {
        intro: "Places a note, with a leader or without.",
        steps: &[
            "For a note without a leader, click an empty spot.",
            "For a leader, select an edge or other geometry to anchor it.",
            "Click where the text goes.",
            "Set the text's properties from the menu.",
            "Click the check to keep the note.",
            "To change its text later, double-click the note.",
        ],
    }),
    ("Callout", ToolHelp {
        intro: "Places a callout on its own or attached to geometry.",
        steps: &[
            "Change the value in the dialog if needed.",
            "Select geometry, or a spot on the sheet.",
            "Click where the callout goes.",
            "To change its note later, double-click the callout.",
        ],
    }),
    ("Table", ToolHelp {
        intro: "Places a table whose cells hold notes.",
        steps: &[
            "In the table dialog set the rows and columns, and a title row and header row if you want them.",
            "Click to place the table.",
            "Type and style text in the cell that is active.",
            "Pick the fixed corner.",
            "Move from cell to cell with Tab and the other navigation keys.",
            "Select one or more cells to change their properties.",
            "Double-click a cell to change its note.",
        ],
    }),
    ("BOM table", ToolHelp {
        intro: "Places a Bill of Materials table, from an Assembly, from a BOM app tab or from a BOM data tab.",
        steps: &[
            "Select the BOM to insert.",
            "If you want, set a Template, Type, Order or Fixed corner.",
            "Click on the sheet to place the BOM.",
        ],
    }),
    ("Cut list table", ToolHelp {
        intro: "Places the cut list table of a composite part.",
        steps: &[
            "Select the cut list to insert.",
            "Pick the order rows are shown in.",
            "Pick the fixed corner.",
            "Click on the sheet to place the cut list.",
        ],
    }),
    ("Hole table", ToolHelp {
        intro: "Places a hole table for one view or several.",
        steps: &[
            "Select the views the table covers and, if you want, an origin within a view.",
            "Click on the sheet to place the hole table.",
        ],
    }),
    ("Custom table", ToolHelp {
        intro: "Places a custom table.",
        steps: &[
            "Select a Part Studio.",
            "Select the custom table to insert.",
            "Pick the order rows are shown in.",
            "Pick the fixed corner.",
            "Click on the sheet to place the custom table.",
        ],
    }),
    ("2 point centerline", ToolHelp {
        intro: "Draws a centerline between two points.",
        steps: &[
            "Select the two points.",
            "Drag the grips at its ends to change the centerline's length.",
        ],
    }),
    ("Edge to edge centerline", ToolHelp {
        intro: "Draws a centerline from two edges.",
        steps: &[
            "Select two lines, or two concentric arcs.",
            "Drag the grips at its ends to change the centerline's length.",
        ],
    }),
    ("3 point circle centerline", ToolHelp {
        intro: "Draws a circular centerline through a bolt circle.",
        steps: &[
            "Select the centers of three circle or arc edges.",
        ],
    }),
    ("2 point circle centerline", ToolHelp {
        intro: "Draws a circular centerline from two points.",
        steps: &[
            "Select the center point.",
            "Select a point on the circle.",
        ],
    }),
    ("Centermark", ToolHelp {
        intro: "Places a centermark, or a pattern of them, at the center of an arc or circle.",
        steps: &[
            "Select arcs or circles, one at a time or as a pattern.",
        ],
    }),
    ("Virtual sharp", ToolHelp {
        intro: "Places a virtual sharp where two straight edges would meet.",
        steps: &[
            "Select the two straight edges.",
        ],
    }),
    ("Line", ToolHelp {
        intro: "Draws a line from two clicks, or a chain of lines.",
        steps: &[
            "Click the start point.",
            "Click the end point.",
            "Keep clicking to chain more lines.",
            "Double-click to finish at a point.",
            "Press Escape to stop.",
        ],
    }),
    ("Center point circle", ToolHelp {
        intro: "Draws a circle from two clicks.",
        steps: &[
            "Click the center point.",
            "Click to set the circle's radius.",
        ],
    }),
    ("Tangent arc", ToolHelp {
        intro: "Draws an arc tangent to a selected line or arc.",
        steps: &[],
    }),
    ("Corner rectangle", ToolHelp {
        intro: "Draws a rectangle from two corner clicks.",
        steps: &[
            "Click one corner.",
            "Click the opposite corner.",
        ],
    }),
    ("Spline", ToolHelp {
        intro: "Draws a spline through several points.",
        steps: &[
            "Select the start point.",
            "Select more points for the spline to pass through.",
            "Double-click to finish the spline.",
            "Drag its handle points to reshape the spline.",
        ],
    }),
    ("Sketch point", ToolHelp {
        intro: "Places a sketch point.",
        steps: &[
            "Pick the Sketch point tool.",
            "Click where the point goes.",
            "Drag the point to move it.",
        ],
    }),
    ("Hatch region", ToolHelp {
        intro: "Fills closed areas with a hatch.",
        steps: &[
            "Sketch a closed profile, or select one.",
            "Pick a hatch pattern.",
            "Set the pattern's scale, angle and color.",
        ],
    }),
    ("Trim", ToolHelp {
        intro: "Cuts a line or curve back to where it first meets another entity or a boundary. With nothing to meet, the whole entity is deleted.",
        steps: &[],
    }),
    ("Horizontal", ToolHelp {
        intro: "Makes one or more lines, or a set of points, horizontal.",
        steps: &[],
    }),
    ("Insert DXF or DWG", ToolHelp {
        intro: "Brings a DXF or DWG file's contents into the drawing as drawing entities.",
        steps: &[
            "Import the DXF or DWG file into a document.",
            "Pick the Insert DXF or DWG tool.",
            "In the dialog, select the file to bring in.",
        ],
    }),
    ("Insert image", ToolHelp {
        intro: "Places an image in the drawing.",
        steps: &[
            "Select an image from this workspace, or browse documents for another.",
            "Click and drag to set the image's size and position.",
            "Drag its handles to resize the image, or its middle to move it.",
        ],
    }),
    ("Format painter", ToolHelp {
        intro: "Select an annotation to take its style, tolerance and precision, and content, then select the annotations to give them to.",
        steps: &[],
    }),
    ("Inspection item", ToolHelp {
        intro: "Inspection items are made by hand or automatically. By hand: hover an annotation to preview its inspection item, then click to place it. Automatically: pick Automatic in the dialog, click Scope and add inspection items, a sheet or one view, check the preview and click to place them.",
        steps: &[],
    }),
    ("Hide inspection items", ToolHelp {
        intro: "Hides the drawing's inspection items.",
        steps: &[],
    }),
];

/// The help of the tool whose tooltip is `tip` (its shortcut dropped), if it has one.
pub fn help(tip: &str) -> Option<&'static ToolHelp> {
    let name = tip.split(" (").next().unwrap_or(tip);
    HELP.iter().find(|(t, _)| *t == name).map(|(_, h)| h)
}

/// The toolbar tool under the pointer and whether its help shows yet.
pub struct HelpHover {
    pub tip: &'static str,
    /// Its title tooltip shows ([`TIP_DELAY`]).
    pub tip_shown: bool,
    /// Its help popover shows ([`HELP_DELAY`]).
    pub shown: bool,
    _timer: Task<()>,
}

impl DrawingEditor {
    /// The pointer arrived on (`on`) or left a toolbar tool.
    pub fn tool_hover(&mut self, tip: &'static str, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let same = self.help_hover.as_ref().is_some_and(|h| h.tip == tip);
        if on == same {
            return;
        }
        if self.help_hover.take().is_some_and(|h| h.shown) {
            cx.notify();
        }
        if !on {
            return;
        }
        let timer = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(TIP_DELAY).await;
            let _ = this.update(cx, |this, _| {
                if let Some(h) = this.help_hover.as_mut().filter(|h| h.tip == tip) {
                    h.tip_shown = true;
                }
            });
            if help(tip).is_none() {
                return;
            }
            cx.background_executor().timer(HELP_DELAY - TIP_DELAY).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if let Some(h) = this.help_hover.as_mut().filter(|h| h.tip == tip) {
                    h.shown = true;
                    // The popover takes the title tooltip's place (live: it covers it).
                    gpui_kit::component::tooltip::hide_tooltip(window, cx);
                    cx.notify();
                }
            });
        });
        self.help_hover = Some(HelpHover { tip, tip_shown: false, shown: false, _timer: timer });
    }

    /// The hovered tool's tooltip as live's recorder reads it: its title, and with a help, "title / title help" (the
    /// help's text is in the tooltip's element from the start, shown or not).
    /// (Called while drawing and when the runner reads the state: the toolbar's tooltip is the kit's, not noted.)
    pub(crate) fn note_tool_tip(&self) {
        let Some(h) = self.help_hover.as_ref().filter(|h| h.tip_shown || h.shown) else { return };
        match help(h.tip) {
            Some(help) => crate::ui_script::note_tooltip(format!("{0} / {0} {1} {2}", h.tip, help.intro, help.steps.join(" ")).trim()),
            None => crate::ui_script::note_tooltip(h.tip),
        }
    }

    /// The shown help popover, placed under the toolbar at the editor's left edge.
    pub fn render_tool_help(&self) -> Option<AnyElement> {
        self.note_tool_tip();
        let tip = self.help_hover.as_ref().filter(|h| h.shown)?.tip;
        let h = help(tip)?;
        let name = tip.split(" (").next().unwrap_or(tip);
        let border = gpui_kit::rgb(0xc9cccf);
        let title = div().h(px(25.)).px(px(8.)).flex().items_center().bg(gpui_kit::rgb(0xcfdbe5)).border_b_1().border_color(gpui_kit::rgb(0xebebeb)).font_weight(FontWeight::BOLD).child(name);
        let steps = h.steps.iter().enumerate().map(|(i, s)| h_flex().w_full().items_start().child(div().w(px(24.)).flex_none().flex().justify_end().pr(px(4.)).child(format!("{}.", i + 1))).child(div().flex_1().min_w_0().whitespace_normal().child(*s)));
        let body = v_flex().w_full().px(px(8.)).py(px(6.)).line_height(px(17.)).whitespace_normal().when(!h.intro.is_empty(), |d| d.child(h.intro)).children(steps);
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
