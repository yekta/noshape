//! Sheet metal flat view › "Export DXF/DWG of Flat pattern of <part>…": the
//! "Export as DXF/DWG" modal (live behaviours "(10) Export DXF/DWG of Flat
//! pattern", dom/qa5-sm-112-flat-export.html). File name "<studio> - Flat
//! pattern of <part>"; Format DXF | DWG (DXF starts at Release 14, DWG at
//! 2000); Version; Scope (the part, its model's parts, the Part Studio's);
//! ☑ Use workspace units (Units grayed while on); Options; the flat's eight
//! checkboxes with live's defaults. The format, version, scope, units and
//! bend centerlines change the file; the other checkboxes have nothing to
//! write yet (our flats have no splines, tangent, counterbore, form or
//! sketch geometry).

use super::PartStudioView;
use super::export::{modal_checkbox, store_export, store_written};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{WindowExt as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::drawing::cad_export::{CAD_VERSIONS, DEFAULT_DWG, DEFAULT_DXF};
use noshape_model::sheetmetal::flat_dxf::{FlatExport, flats_dwg, flats_dxf};
use std::cell::RefCell;
use std::rc::Rc;

const FORMATS: [&str; 2] = ["DXF", "DWG"];
const SCOPES: [&str; 3] = ["Single flat pattern part only", "All flat pattern parts in the current model", "All flat pattern parts in the Part Studio"];
/// The Units list and each unit's DXF $INSUNITS.
const UNITS: [(&str, u8); 6] = [("Centimeter", 5), ("Foot", 2), ("Inch", 1), ("Meter", 6), ("Millimeter", 4), ("Yard", 10)];
const OPTIONS: [&str; 3] = ["Download", "Download and store file in a new tab", "Store file in a new tab"];
/// The checkboxes (id, label, live's default).
const CHECKS: [(&str, &str, bool); 8] = [
    ("fx-splines", "Export splines as polylines", false),
    ("fx-z", "Set z-height to zero and normals to positive", true),
    ("fx-centerlines", "Include bend centerlines", true),
    ("fx-tangents", "Include bend tangent lines", false),
    ("fx-cbore", "Include counterbore and countersink lines", false),
    ("fx-form-outlines", "Include form feature outlines", false),
    ("fx-form-marks", "Include form feature centermarks", false),
    ("fx-sketches", "Include visible sketches", true),
];
const CENTERLINES: usize = 2;

#[derive(Clone)]
struct Settings {
    dwg: bool,
    version: usize,
    scope: usize,
    workspace_units: bool,
    units: usize,
    options: usize,
    checks: [bool; 8],
}

impl PartStudioView {
    /// The "Export as DXF/DWG" modal for `body`'s flat pattern.
    pub(super) fn open_flat_export(&mut self, body: BodyId, window: &mut Window, cx: &mut Context<Self>) {
        let part = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_else(|| "Part".into());
        let base = format!("{} - Flat pattern of {part}", self.studio_name(cx));
        // The workspace's length unit, in Units' order.
        let workspace = match self.doc.read(cx).session.doc.settings.units.length {
            noshape_units::LengthUnit::Centimeter => 0,
            noshape_units::LengthUnit::Foot => 1,
            noshape_units::LengthUnit::Inch => 2,
            noshape_units::LengthUnit::Meter => 3,
            noshape_units::LengthUnit::Yard => 5,
            _ => 4,
        };
        let file = cx.new(|cx| InputState::new(window, cx).default_value(base));
        let st = Rc::new(RefCell::new(Settings { dwg: false, version: DEFAULT_DXF, scope: 0, workspace_units: true, units: workspace, options: 0, checks: CHECKS.map(|c| c.2) }));
        let (view, part_id) = (cx.entity(), body);
        window.open_dialog(cx, move |dialog, _, cx| {
            let s = st.borrow().clone();
            let select = |id: &'static str, label: &'static str, opts: Vec<String>, cur: usize, disabled: bool, set: fn(&mut Settings, usize)| {
                let st = st.clone();
                div().mt(px(4.)).child(crate::os_ui::field_label(label)).child(
                    div()
                        .mt(px(5.))
                        .relative()
                        .child(crate::os_ui::select_field(id, opts, cur, move |i, w, _| {
                            set(&mut st.borrow_mut(), i);
                            w.refresh();
                        }))
                        // Live: Units is grayed while "Use workspace units" is on.
                        .when(disabled, |d| d.child(div().absolute().inset_0().rounded(px(4.)).bg(gpui_kit::rgba(0x00000011)).occlude())),
                )
            };
            let check = |k: usize, on: bool, set: fn(&mut Settings, bool)| {
                let st = st.clone();
                modal_checkbox(CHECKS[k].0, CHECKS[k].1, on).on_click(move |_, w, _| {
                    set(&mut st.borrow_mut(), !on);
                    w.refresh();
                })
            };
            let strs = |a: &[&str]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>();
            let mut body = crate::os_ui::modal_body()
                .gap(px(0.))
                .child(h_flex().gap(px(4.)).items_end().child(crate::os_ui::field_label("File name")).child(crate::part_studio_hooks::export_rules_link("fx-rules")))
                .child(div().mt(px(2.)).child(Input::new(&file).h(px(34.)).text_size(px(14.))))
                .child(select("fx-format", "Format", strs(&FORMATS), s.dwg as usize, false, |s, i| {
                    s.dwg = i == 1;
                    s.version = if s.dwg { DEFAULT_DWG } else { DEFAULT_DXF };
                }))
                .child(select("fx-version", "Version", CAD_VERSIONS.iter().map(|v| v.0.to_string()).collect(), s.version, false, |s, i| s.version = i))
                .child(select("fx-scope", "Scope", strs(&SCOPES), s.scope, false, |s, i| s.scope = i));
            let units_check = {
                let st = st.clone();
                let on = s.workspace_units;
                modal_checkbox("fx-workspace-units", "Use workspace units", on).on_click(move |_, w, _| {
                    st.borrow_mut().workspace_units = !on;
                    w.refresh();
                })
            };
            body = body
                .child(div().mt(px(9.)).child(units_check))
                .child(select("fx-units", "Units", UNITS.iter().map(|u| u.0.to_string()).collect(), s.units, s.workspace_units, |s, i| s.units = i))
                .child(select("fx-options", "Options", strs(&OPTIONS), s.options, false, |s, i| s.options = i));
            let setters: [fn(&mut Settings, bool); 8] = [|s, v| s.checks[0] = v, |s, v| s.checks[1] = v, |s, v| s.checks[2] = v, |s, v| s.checks[3] = v, |s, v| s.checks[4] = v, |s, v| s.checks[5] = v, |s, v| s.checks[6] = v, |s, v| s.checks[7] = v];
            for (k, set) in setters.into_iter().enumerate() {
                body = body.child(div().mt(px(if k == 0 { 9. } else { 12. })).child(check(k, s.checks[k], set)));
            }
            let (st2, file2, view2) = (st.clone(), file.clone(), view.clone());
            let body_id = part_id.clone();
            crate::os_ui::modal(dialog, "Export as DXF/DWG", 600.)
                .child(body)
                .footer(crate::os_ui::modal_footer().child(crate::os_ui::modal_ok(crate::os_ui::primary_button("ok", "Export"))).child(crate::os_ui::modal_cancel(crate::os_ui::cancel_button("cancel", "Cancel", cx))))
                .on_ok(move |_, window, cx| {
                    let s = st2.borrow().clone();
                    let name = file2.read(cx).value().trim().to_string();
                    let (view, part) = (view2.clone(), body_id.clone());
                    window.defer(cx, move |window, cx| view.update(cx, |this, cx| this.run_flat_export(s, name, part, window, cx)));
                    true
                })
        });
    }

    /// Write the flats the scope takes, as the modal set them.
    fn run_flat_export(&mut self, s: Settings, name: String, body: BodyId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let model = ctx.sheet_metal.get(&body).map(|d| d.model);
        let parts: Vec<BodyId> = match s.scope {
            0 => vec![body],
            1 => ctx.sheet_metal.iter().filter(|(_, d)| Some(d.model) == model).map(|(b, _)| b.clone()).collect(),
            _ => ctx.sheet_metal.keys().cloned().collect(),
        };
        let mut flats = Vec::new();
        for b in &parts {
            match noshape_model::sheetmetal::flat_pattern_for(&ctx, b) {
                Ok(f) => flats.push(f),
                Err(e) => return self.flash_info(format!("Export failed: {e}"), cx),
            }
        }
        let units = if s.workspace_units { self.workspace_insunits(cx) } else { UNITS[s.units.min(5)].1 };
        let o = FlatExport { insunits: units, version: CAD_VERSIONS[s.version.min(CAD_VERSIONS.len() - 1)].1, bend_centerlines: s.checks[CENTERLINES] };
        let bytes = match if s.dwg { flats_dwg(&flats, &o) } else { flats_dxf(&flats, &o) } {
            Ok(b) => b,
            Err(e) => return self.flash_info(format!("Export failed: {e}"), cx),
        };
        let name = if name.is_empty() { "Flat pattern".to_string() } else { name };
        let ext = if s.dwg { "dwg" } else { "dxf" };
        let file = format!("{name}.{ext}");
        // Options › Store file in a new tab: no download.
        if s.options == 2 {
            let msg = match store_export(&self.doc, self.tab, &file, |p| {
                std::fs::write(p, &bytes)?;
                Ok(p.to_path_buf())
            }, cx) {
                Ok(f) => format!("Stored {f}"),
                Err(e) => format!("Export failed: {e:#}"),
            };
            return self.flash_info(msg, cx);
        }
        let view = cx.entity();
        crate::file_dialogs::request_save_path("Export as DXF/DWG", file, ext, window, cx, move |path, cx| {
            view.update(cx, |this, cx| {
                let msg = match std::fs::write(&path, &bytes) {
                    Ok(()) => {
                        if s.options == 1 {
                            store_written(&this.doc, this.tab, &path, cx);
                        }
                        format!("Exported {}", path.display())
                    }
                    Err(e) => format!("Export failed: {e}"),
                };
                this.flash_info(msg, cx);
            });
        });
    }

    /// The workspace length unit's DXF $INSUNITS.
    fn workspace_insunits(&self, cx: &App) -> u8 {
        match self.doc.read(cx).session.doc.settings.units.length {
            noshape_units::LengthUnit::Centimeter => 5,
            noshape_units::LengthUnit::Foot => 2,
            noshape_units::LengthUnit::Inch => 1,
            noshape_units::LengthUnit::Meter => 6,
            noshape_units::LengthUnit::Yard => 10,
            noshape_units::LengthUnit::Micrometer => 13,
            _ => 4,
        }
    }
}
