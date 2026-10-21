//! Appearances (Onshape "Customizing appearances"): the Appearance editor
//! for parts (color, transparency, tessellation quality), faces, features
//! and sketches, and the Appearance panel listing every custom appearance.
//! Face and feature colors reach the renderer as per-vertex colors.

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::partstudio::{Appearance, AppearanceTarget, FaceAppearance, TessQuality};
use noshape_model::reference::{FeatureId, Reference};
use std::collections::HashMap;
use std::sync::Arc;

pub use crate::scene_builder::VertexColors;

/// The Appearance editor's Palette tab (live dom/dialog-appearance-editor.txt):
/// a top row of black / blue / red, then the 8 default part colours; under
/// them a 3-column ramp of greys (7 rows) and an 8-column colour grid (7 rows).
pub const PALETTE_TOP_A: [u32; 3] = [0x000000, 0x0000ff, 0xff0000];
pub const PALETTE_TOP_B: [u32; 8] = [0x9dcfed, 0xa5a5a5, 0x3b61b4, 0xeaeaea, 0xc4e2f3, 0xf88701, 0x7f7f7f, 0xfab601];
pub const PALETTE_GREYS: [u32; 21] = [
    0xe6e6e6, 0xe6e0cf, 0xcfdbe5, 0xcccccc, 0xccc6b6, 0xb6c2cc, 0xb3b3b3, 0xb3ae9f, 0x9faab3, 0x999999, 0x999487, 0x879199, 0x808080, 0x807c70, 0x707980, 0x666666, 0x666359, 0x596066, 0x4d4d4d,
    0x4d4a43, 0x43484d,
];
pub const PALETTE_GRID: [u32; 56] = [
    0xe3b2a3, 0xebcca7, 0xf0dfad, 0xcbe7be, 0xbcefea, 0xc2d6ec, 0xccbfe9, 0xd9c1dd, 0xe1927a, 0xeab97d, 0xf5d578, 0xa7d293, 0x89dad3, 0x8aaed6, 0xa795d2, 0xc38bcd, 0xdf7251, 0xe9a554, 0xfacb43,
    0x83bc68, 0x57c6bd, 0x5387c0, 0x836abb, 0xae55bc, 0xdd5228, 0xe8922a, 0xffc10e, 0x5fa73d, 0x24b1a6, 0x1651b0, 0x5e40a4, 0x981fac, 0xba401b, 0xc1761c, 0xd29f09, 0x488c29, 0x18948b, 0x165294,
    0x4b2e8f, 0x7d168e, 0x982d0d, 0x995a0e, 0xa67c05, 0x327014, 0x0c786f, 0x10447d, 0x371b7a, 0x610c6f, 0x751b00, 0x723e00, 0x795a00, 0x1b5500, 0x005b54, 0x0b3767, 0x240965, 0x460351,
];

/// The editor field a colour was typed in (it keeps its own text).
#[derive(Clone, Copy, PartialEq)]
enum ColorField {
    Hex,
    Channel(usize),
}

/// What the open Appearance editor edits.
#[derive(Clone, Debug, PartialEq)]
pub enum EditTarget {
    Parts(Vec<BodyId>),
    /// Faces (and the index of the appearance being edited, if any).
    Faces(Vec<Reference>, Option<usize>),
    /// A feature's faces or a sketch's curves.
    Feature(FeatureId, Option<usize>),
    /// Configured properties: a part's Appearance cell (part, row).
    ConfiguredCell(BodyId, usize),
}

pub struct AppearanceEdit {
    pub target: EditTarget,
    /// sRGB.
    pub rgb: [u8; 3],
    /// 0 = opaque, 1 = invisible (Onshape "Part transparency").
    pub transparency: f32,
    pub tessellation: TessQuality,
    /// The Mixer tab is showing (else Palette).
    pub mixer: bool,
    pub mix: crate::color_mixer::MixerState,
    /// The A slider is being dragged.
    alpha_drag: bool,
    /// Colour, transparency and quality at opening: ✓ stays at 20 % until
    /// one changes (live).
    initial: ([u8; 3], f32, TessQuality),
    hex: Entity<InputState>,
    /// R, G, B.
    channels: [Entity<InputState>; 3],
    alpha: Entity<InputState>,
    _subs: Vec<Subscription>,
}

/// sRGB 0..255 to linear 0..1.
pub fn srgb_to_lin(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Linear 0..1 to sRGB 0..255.
pub fn lin_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

/// Parse `#rrggbb` / `rrggbb` / `rgb`.
pub fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let t = text.trim().trim_start_matches('#');
    let full: String = match t.len() {
        3 => t.chars().flat_map(|c| [c, c]).collect(),
        6 => t.to_string(),
        _ => return None,
    };
    let v = u32::from_str_radix(&full, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// The editor's # field (live "3B61B4": upper case, the # is its label).
fn hex_field(rgb: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// Per-vertex sRGB colors of a body mesh: faces with a custom appearance
/// get its color, others alpha 0 (the part color).
pub fn face_colors(body: &noshape_model::display::BodyDisplay, appearances: &[FaceAppearance]) -> Option<Vec<[u8; 4]>> {
    let mut colors: Option<Vec<[u8; 4]>> = None;
    // Colours the features gave faces (an import's) first.
    for (fi, c) in body.face_colors.iter().enumerate() {
        let (Some(c), Some(&(first, count))) = (c, body.mesh.face_ranges.get(fi)) else { continue };
        let c = [lin_to_srgb(c[0]), lin_to_srgb(c[1]), lin_to_srgb(c[2]), 255];
        let v = colors.get_or_insert_with(|| vec![[0; 4]; body.mesh.positions.len()]);
        for &i in &body.mesh.indices[first as usize..(first + count) as usize] {
            if let Some(slot) = v.get_mut(i as usize) {
                *slot = c;
            }
        }
    }
    for a in appearances {
        let c = [lin_to_srgb(a.color[0]), lin_to_srgb(a.color[1]), lin_to_srgb(a.color[2]), 255];
        for (fi, r) in body.face_refs.iter().enumerate() {
            let hit = match &a.target {
                AppearanceTarget::Faces(faces) => faces.contains(r),
                AppearanceTarget::Feature(f) => r.depends_on() == Some(*f),
            };
            if !hit {
                continue;
            }
            let Some(&(first, count)) = body.mesh.face_ranges.get(fi) else { continue };
            let v = colors.get_or_insert_with(|| vec![[0; 4]; body.mesh.positions.len()]);
            for &i in &body.mesh.indices[first as usize..(first + count) as usize] {
                if let Some(slot) = v.get_mut(i as usize) {
                    *slot = c;
                }
            }
        }
    }
    colors
}

impl PartStudioView {
    /// Per-vertex colors for the scene: an active analysis map, otherwise
    /// face/feature appearances. Cached per display and inputs.
    pub(super) fn vertex_colors(&mut self, cx: &App) -> Option<VertexColors> {
        let appearances = self.studio(cx).map(|s| s.appearances.clone()).unwrap_or_default();
        let curvature = self.ui.curvature_map.as_ref().map(|c| (c.kind, c.bar.palette, c.bar.bounds));
        let key = (Arc::as_ptr(&self.display) as usize, format!("{:?}{:?}{:?}", appearances, self.ui.analysis, curvature));
        if let Some((k, v)) = &self.ui.vcolor_cache
            && *k == key
        {
            return v.clone();
        }
        let mut map: HashMap<BodyId, Arc<Vec<[u8; 4]>>> = HashMap::new();
        if matches!(self.ui.analysis, Some(super::analysis::Analysis::Curvature)) {
            let m = self.doc.read(cx).session.doc.settings.units.length.meters();
            let (colors, range) = super::analysis::curvature_colors(&self.display.bodies, self.ui.curvature_map.as_ref(), m);
            if let Some(c) = &mut self.ui.curvature_map {
                c.range = range;
            }
            map = colors;
            let v = (!map.is_empty()).then(|| Arc::new(map));
            self.ui.vcolor_cache = Some((key, v.clone()));
            return v;
        }
        for b in self.display.bodies.iter().filter(|b| !b.hidden) {
            let colors = match &self.ui.analysis {
                Some(a) => Some(super::analysis::analysis_colors(b, a)),
                None => face_colors(b, &appearances),
            };
            if let Some(c) = colors {
                map.insert(b.id.clone(), Arc::new(c));
            }
        }
        let v = (!map.is_empty()).then(|| Arc::new(map));
        self.ui.vcolor_cache = Some((key, v.clone()));
        v
    }

    /// Open the Appearance editor.
    pub(super) fn edit_appearance(&mut self, target: EditTarget, window: &mut Window, cx: &mut Context<Self>) {
        let studio = self.studio(cx).cloned();
        let (color, tess) = match (&target, &studio) {
            (EditTarget::Parts(b), Some(s)) => {
                let first = b.first();
                let props = first.and_then(|b| s.part_properties.get(b));
                let c = props.and_then(|p| p.appearance).map(|a| a.color).or_else(|| first.and_then(|b| self.display.body(b)).map(|b| b.color)).unwrap_or([0.7, 0.7, 0.7, 1.0]);
                (c, props.and_then(|p| p.tessellation).unwrap_or(TessQuality::Auto))
            }
            (EditTarget::Faces(_, Some(i)) | EditTarget::Feature(_, Some(i)), Some(s)) => (s.appearances.get(*i).map(|a| a.color).unwrap_or([0.8, 0.3, 0.2, 1.0]), TessQuality::Auto),
            // The cell's color, else the part's current one.
            (EditTarget::ConfiguredCell(b, row), Some(s)) => {
                let cell = noshape_model::configured_properties::table(s, b).and_then(|t| t.rows.get(*row)).and_then(|r| r.values.get("Appearance")).and_then(|v| noshape_model::configured_properties::parse_color(v));
                (cell.or_else(|| self.display.body(b).map(|d| d.color)).unwrap_or([0.7, 0.7, 0.7, 1.0]), TessQuality::Auto)
            }
            _ => ([0.831, 0.44, 0.007, 1.0], TessQuality::Auto),
        };
        let rgb = [lin_to_srgb(color[0]), lin_to_srgb(color[1]), lin_to_srgb(color[2])];
        let transparency = 1.0 - color[3].clamp(0.0, 1.0);
        let hex = cx.new(|cx| InputState::new(window, cx).default_value(hex_field(rgb)));
        let channels = [0, 1, 2].map(|i| cx.new(|cx| InputState::new(window, cx).default_value(rgb[i].to_string())));
        let alpha = cx.new(|cx| InputState::new(window, cx).default_value(noshape_units::to_fixed(f64::from(1.0 - transparency), 2)));
        let mut subs = vec![cx.subscribe_in(&hex, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::Change | InputEvent::PressEnter { .. })
                && let Some(rgb) = parse_hex(&st.read(cx).value())
            {
                this.set_edit_color_from(rgb, Some(ColorField::Hex), window, cx);
            }
        })];
        for (i, ch) in channels.iter().enumerate() {
            subs.push(cx.subscribe_in(ch, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change | InputEvent::PressEnter { .. })
                    && let Ok(v) = st.read(cx).value().trim().parse::<u8>()
                    && let Some(mut rgb) = this.ui.appearance_edit.as_ref().map(|e| e.rgb)
                {
                    rgb[i] = v;
                    this.set_edit_color_from(rgb, Some(ColorField::Channel(i)), window, cx);
                }
            }));
        }
        subs.push(cx.subscribe_in(&alpha, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::Change | InputEvent::PressEnter { .. })
                && let Ok(v) = st.read(cx).value().trim().parse::<f32>()
                && let Some(e) = &mut this.ui.appearance_edit
            {
                e.transparency = 1.0 - v.clamp(0.0, 1.0);
                cx.notify();
            }
        }));
        // Live 173: the hex field has the focus, its text selected.
        hex.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.ui.appearance_edit = Some(AppearanceEdit { target, rgb, transparency, tessellation: tess, mixer: false, mix: crate::color_mixer::MixerState::from_rgb(rgb), alpha_drag: false, initial: (rgb, transparency, tess), hex, channels, alpha, _subs: subs });
        cx.notify();
    }

    fn set_edit_color(&mut self, rgb: [u8; 3], window: &mut Window, cx: &mut Context<Self>) {
        self.set_edit_color_from(rgb, None, window, cx);
    }

    /// The editor's colour, from a swatch or the mixer (`from` None) or one
    /// of its fields, which keeps its own text.
    fn set_edit_color_from(&mut self, rgb: [u8; 3], from: Option<ColorField>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &mut self.ui.appearance_edit else { return };
        e.rgb = rgb;
        if from.is_some() {
            let used = e.mix.hue_used;
            e.mix = crate::color_mixer::MixerState::from_rgb(rgb);
            e.mix.hue_used = used;
        }
        let (hex, channels) = (e.hex.clone(), e.channels.clone());
        if from != Some(ColorField::Hex) {
            hex.update(cx, |s, cx| s.set_value(hex_field(rgb), window, cx));
        }
        for (i, ch) in channels.iter().enumerate() {
            if from != Some(ColorField::Channel(i)) {
                ch.update(cx, |s, cx| s.set_value(rgb[i].to_string(), window, cx));
            }
        }
        cx.notify();
    }

    /// The A slider and field (`alpha` 0–1).
    fn set_edit_alpha(&mut self, alpha: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &mut self.ui.appearance_edit else { return };
        e.transparency = 1.0 - alpha.clamp(0.0, 1.0);
        let field = e.alpha.clone();
        field.update(cx, |s, cx| s.set_value(noshape_units::to_fixed(f64::from(alpha), 2), window, cx));
        cx.notify();
    }

    /// Apply the editor (✓).
    fn accept_appearance(&mut self, cx: &mut Context<Self>) {
        let Some(e) = self.ui.appearance_edit.take() else { return };
        let color = [srgb_to_lin(e.rgb[0]), srgb_to_lin(e.rgb[1]), srgb_to_lin(e.rgb[2]), 1.0 - e.transparency];
        let tess = e.tessellation;
        if let EditTarget::ConfiguredCell(part, row) = &e.target {
            let value = noshape_model::configured_properties::color_text(e.rgb, 1.0 - e.transparency);
            let (part, row) = (part.clone(), *row);
            self.edit_config("Edit configured properties", cx, |ps| noshape_model::configured_properties::set_cell(ps, &part, row, "Appearance", &value));
            self.config_dirty(cx);
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Edit appearance", cx, |ps| match &e.target {
                EditTarget::Parts(bodies) => {
                    for b in bodies {
                        let p = ps.part_properties.entry(b.clone()).or_default();
                        p.appearance = Some(Appearance { color });
                        p.tessellation = (tess != TessQuality::Auto).then_some(tess);
                    }
                }
                EditTarget::Faces(faces, index) => {
                    let a = FaceAppearance { target: AppearanceTarget::Faces(faces.clone()), color };
                    match index.and_then(|i| ps.appearances.get_mut(i)) {
                        Some(slot) => *slot = a,
                        None => ps.appearances.push(a),
                    }
                }
                EditTarget::ConfiguredCell(..) => {}
                EditTarget::Feature(f, index) => {
                    let a = FaceAppearance { target: AppearanceTarget::Feature(*f), color };
                    match index.or_else(|| ps.appearances.iter().position(|x| x.target == a.target)).and_then(|i| ps.appearances.get_mut(i)) {
                        Some(slot) => *slot = a,
                        None => ps.appearances.push(a),
                    }
                }
            });
        });
        cx.notify();
    }

    fn remove_appearance(&mut self, index: usize, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Remove appearance", cx, |ps| {
                if index < ps.appearances.len() {
                    ps.appearances.remove(index);
                }
            });
        });
    }

    fn reset_part_appearance(&mut self, b: BodyId, cx: &mut Context<Self>) {
        self.edit_part_appearance(&b, cx, |p| {
            p.appearance = None;
            p.tessellation = None;
        });
    }

    fn edit_part_appearance(&mut self, b: &BodyId, cx: &mut Context<Self>, f: impl FnOnce(&mut noshape_model::partstudio::PartProperties)) {
        let b = b.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Edit appearance", cx, |ps| f(ps.part_properties.entry(b).or_default()));
        });
    }

    /// Faces of the selection (for "Add appearance").
    pub(super) fn selected_faces(&self) -> Vec<Reference> {
        self.selection.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect()
    }

    /// The floating Appearance editor (live dom/dialog-appearance-editor.txt,
    /// part-studio/173–174): 220 x 400; header (title, ✓ ✕); tabs Palette |
    /// Mixer; the swatches or the mixer (162 px); the current colour, the A
    /// slider and field; # R G B; Custom colors; Tessellation quality; ?.
    pub(super) fn render_appearance_editor(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let e = self.ui.appearance_edit.as_ref()?;
        let pal = p();
        let title = match &e.target {
            EditTarget::Faces(..) => "Face appearance",
            _ => "Appearance",
        };
        let is_part = matches!(e.target, EditTarget::Parts(..));
        // Live 175: a face appearance starts with its "Faces or features" field.
        let faces = match &e.target {
            EditTarget::Faces(f, _) => Some(f.len()),
            _ => None,
        };
        let changed = e.initial != (e.rgb, e.transparency, e.tessellation);
        let rgb = e.rgb;
        let alpha = 1.0 - e.transparency;
        let (mixer, mix) = (e.mixer, e.mix);
        let color = |c: [u8; 3], a: f32| gpui_kit::Rgba { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a };
        // Live: an 18 px white swatch with a 16 px colour; the current one
        // has a 2 px `#1651b0` glow and a white ✓.
        let swatch = |hex: u32, cx: &mut Context<Self>| {
            let c = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
            let on = c == rgb;
            div()
                .id(SharedString::from(format!("ap-sw-{hex:06x}")))
                .size(px(18.))
                .p(px(1.))
                .bg(gpui_kit::rgb(0xffffff))
                .cursor_pointer()
                .child(
                    div()
                        .size(px(16.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb_u32(hex))
                        .when(on, |d| d.shadow(vec![BoxShadow { color: gpui_kit::rgb(0x1651b0).into(), offset: point(px(0.), px(0.)), blur_radius: px(1.), spread_radius: px(0.), inset: false }]).child(crate::ui::glyph("check", 12., gpui_kit::rgb(0xffffff)))),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.set_edit_color(c, w, cx)))
                .into_any_element()
        };
        let grid = |colors: &[u32], cols: usize, cx: &mut Context<Self>| {
            let mut col = v_flex();
            for row in colors.chunks(cols) {
                col = col.child(h_flex().children(row.iter().map(|h| swatch(*h, cx))));
            }
            col
        };
        let tab = |id: &'static str, label: &'static str, on: bool, to_mixer: bool, w: f32, cx: &mut Context<Self>| {
            div()
                .id(id)
                .w(px(w))
                .h(px(26.))
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(on, |d| d.text_color(gpui_kit::rgb(0x1651b0)).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(gpui_kit::rgb(0x1651b0))))
                .when(!on, |d| d.text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(e) = &mut this.ui.appearance_edit {
                        e.mixer = to_mixer;
                    }
                    cx.notify();
                }))
        };
        let body: AnyElement = if mixer {
            div()
                .h(px(162.))
                .w(px(218.))
                .p(px(5.))
                .rounded(px(5.))
                .bg(gpui_kit::rgb(0xffffff))
                .child(crate::color_mixer::mixer(&mix, |v: &mut Self| v.ui.appearance_edit.as_mut().map(|e| &mut e.mix), |v: &mut Self, c, w, cx| v.set_edit_color(c, w, cx), cx))
                .into_any_element()
        } else {
            v_flex()
                .h(px(162.))
                .child(h_flex().h(px(27.)).child(h_flex().w(px(64.)).pt(px(6.)).px(px(5.)).children(PALETTE_TOP_A.iter().map(|h| swatch(*h, cx)))).child(h_flex().pt(px(6.)).px(px(5.)).children(PALETTE_TOP_B.iter().map(|h| swatch(*h, cx)))))
                .child(h_flex().items_start().child(div().w(px(65.)).pt(px(6.)).px(px(5.)).child(grid(&PALETTE_GREYS, 3, cx))).child(div().pt(px(6.)).px(px(5.)).child(grid(&PALETTE_GRID, 8, cx))))
                .into_any_element()
        };
        // Live: a 12 px `#666` label 5 px in, a 13 px black value over a
        // 1 px `#d7d7d7` rule (`#1651b0` while focused).
        let field = |label: &'static str, input: &Entity<InputState>, w: f32, focused: bool| {
            h_flex()
                .h(px(27.))
                .pr(px(4.))
                .items_start()
                // Live: the label box is 12 px (its 5 px padding overlaps the text).
                .child(div().w(px(12.)).flex_none().pt(px(6.)).pl(px(2.)).h(px(21.)).text_size(px(12.)).line_height(px(16.)).text_color(gpui_kit::rgb(0x666666)).child(label))
                .child(
                    div()
                        .w(px(w))
                        .h(px(27.))
                        .border_b_1()
                        .border_color(if focused { gpui_kit::rgb(0x1651b0) } else { gpui_kit::rgb(0xd7d7d7) })
                        .child(Input::new(input).appearance(false).h(px(26.)).text_size(px(13.)).text_color(gpui_kit::rgb(0x000000)).pl(px(3.)).pr(px(2.))),
                )
        };
        let (hex, ch, a_in) = (e.hex.clone(), e.channels.clone(), e.alpha.clone());
        let on = |st: &Entity<InputState>| st.read(cx).focus_handle(cx).is_focused(window);
        let (f_hex, f_ch, f_a) = (on(&hex), [on(&ch[0]), on(&ch[1]), on(&ch[2])], on(&a_in));
        // The A slider (live noUiSlider, 84 x 10, "Change part transparency"):
        // a 2 px line, an 11 px knob; only a press on it drags it.
        let track: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
        let (t1, t2, t3) = (track.clone(), track.clone(), track);
        let frac = |x: Pixels, b: Bounds<Pixels>| (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
        let slider = div()
            .id("ap-alpha")
            .relative()
            .ml(px(7.))
            .w(px(84.))
            .h(px(20.))
            .cursor_pointer()
            .tooltip(crate::ui::tooltip("Change part transparency"))
            .child(canvas(move |b, _, _| t1.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .child(div().absolute().left_0().right_0().top(px(9.)).h(px(2.)).bg(gpui_kit::rgb(0x777777)))
            .child(
                div()
                    .absolute()
                    .top(px(5.))
                    .left(relative(alpha))
                    .ml(px(-6.))
                    .size(px(11.))
                    .rounded_full()
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_1()
                    .border_color(gpui_kit::rgb(0x888888))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }]),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                if let Some(e) = &mut this.ui.appearance_edit {
                    e.alpha_drag = true;
                }
                this.set_edit_alpha(frac(ev.position.x, t2.get()), w, cx);
            }))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, w, cx| {
                let dragging = this.ui.appearance_edit.as_ref().is_some_and(|e| e.alpha_drag);
                if ev.pressed_button != Some(MouseButton::Left) {
                    if let Some(e) = &mut this.ui.appearance_edit {
                        e.alpha_drag = false;
                    }
                } else if dragging {
                    this.set_edit_alpha(frac(ev.position.x, t3.get()), w, cx);
                }
            }));
        let tess = e.tessellation;
        let view = cx.entity();
        let tess_select = crate::os_ui::select_field("ap-tess", TessQuality::ALL.iter().map(|q| q.label().to_string()).collect(), TessQuality::ALL.iter().position(|q| *q == tess).unwrap_or(0), move |i, _, cx| {
            view.update(cx, |this, cx| {
                if let Some(e) = &mut this.ui.appearance_edit {
                    e.tessellation = TessQuality::ALL[i];
                }
                cx.notify();
            })
        })
        .h(px(26.))
        .w(px(97.))
        .pl(px(6.))
        .rounded(px(0.))
        .border_0()
        .border_b_1()
        .border_color(gpui_kit::rgb(0xd7d7d7))
        .text_size(px(14.));
        let parts = match &e.target {
            EditTarget::Parts(b) => b.len(),
            _ => 0,
        };
        let tess_note = format!("{parts} {} part{}", tess.label().to_lowercase().replace("auto", "fine"), if parts == 1 { "" } else { "s" });
        Some(
            v_flex()
                .id("appearance-editor")
                .occlude()
                .child(crate::ui::panel_probe("ps-appearance"))
                .absolute()
                // Live: beside an open right panel at the top (part-studio/173),
                // else at the graphics area's bottom left like Material
                // (dom/cursor-rows.json: from the part menu, x 241 y 647).
                .map(|d| if self.right_panel.is_some() { d.top(px(3.)).right(px(4.)) } else { d.left_0().bottom_0() })
                .w(px(220.))
                .h(px(if faces.is_some() { 431. } else { 400. }))
                .bg(with_alpha(gpui_kit::rgb(0xffffff), 0.95))
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(
                    h_flex()
                        .h(px(29.))
                        .flex_none()
                        .bg(gpui_kit::rgb(0xffffff))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd7d7d7))
                        .child(crate::ui::panel_title("ps-appearance", div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title)))
                        .child(div().id("ap-ok").size(px(28.)).flex().items_center().justify_center().bg(if changed { pal.accept } else { gpui_kit::rgb(0xcce9cc) }).cursor(CursorStyle::Arrow).child(crate::assets::icon("dialog-ok", 20., cx)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if changed {
                                this.accept_appearance(cx);
                            }
                        })))
                        .child(
                            div()
                                .id("ap-cancel")
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor(CursorStyle::Arrow)
                                .child(crate::assets::icon("dialog-cancel", 18., cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.ui.appearance_edit = None;
                                    cx.notify();
                                })),
                        ),
                )
                .when_some(faces, |d, n| {
                    d.child(
                        div().px(px(3.)).pt(px(5.)).child(
                            v_flex()
                                .id("ap-faces")
                                .min_h(px(26.))
                                .px(px(6.))
                                .pb(px(4.))
                                .rounded(px(2.))
                                .border_1()
                                .border_color(pal.accent)
                                .bg(pal.row_hover)
                                .cursor_pointer()
                                .child(div().pt(px(7.)).text_size(px(10.)).line_height(px(9.)).text_color(pal.text_muted).child("Faces or features"))
                                .child(div().text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).child(format!("{n} face{}", if n == 1 { "" } else { "s" })))
                                // A click takes the selected faces.
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    let sel = this.selected_faces();
                                    if let Some(EditTarget::Faces(f, _)) = this.ui.appearance_edit.as_mut().map(|e| &mut e.target)
                                        && !sel.is_empty()
                                    {
                                        *f = sel;
                                    }
                                    cx.notify();
                                })),
                        ),
                    )
                })
                .child(h_flex().h(px(26.)).flex_none().border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).child(tab("ap-tab-palette", "Palette", !mixer, false, 115., cx)).child(tab("ap-tab-mixer", "Mixer", mixer, true, 105., cx)))
                .child(body)
                .child(
                    h_flex()
                        .h(px(27.))
                        .flex_none()
                        .child(div().ml(px(6.)).w(px(66.)).h(px(22.)).bg(gpui_kit::rgb(0xffffff)).child(div().size_full().bg(color(rgb, alpha))))
                        .child(slider)
                        .child(div().flex_1())
                        .child(field("A", &a_in, 30., f_a)),
                )
                .child(h_flex().h(px(27.)).flex_none().child(field("#", &hex, 60., f_hex)).child(field("R", &ch[0], 30., f_ch[0])).child(field("G", &ch[1], 30., f_ch[1])).child(field("B", &ch[2], 30., f_ch[2])))
                .child(div().h(px(26.)).flex_none().pt(px(5.)).px(px(5.)).text_size(px(12.)).line_height(px(16.)).text_color(gpui_kit::rgb(0x666666)).child("Custom colors"))
                .child(h_flex().h(px(26.)).flex_none().pl(px(6.)).child(div().size(px(19.)).flex().items_center().justify_center().border_1().border_color(gpui_kit::rgb(0x999999)).text_size(px(14.)).text_color(gpui_kit::rgb(0x999999)).child("+")))
                .when(is_part, |d| {
                    d.child(h_flex().h(px(32.)).flex_none().px(px(5.)).justify_between().child(div().text_size(px(12.)).child("Tessellation quality")).child(tess_select))
                        .child(div().h(px(19.)).flex_none().px(px(5.)).pt(px(3.)).text_size(px(12.)).line_height(px(13.2)).child(tess_note))
                })
                .child(div().flex_1())
                .child(h_flex().h(px(26.)).flex_none().justify_end().pr(px(4.)).child(div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")))
                .into_any_element(),
        )
    }

    /// Appearance panel (right side): every custom appearance.
    /// Onshape's Appearances panel (live, dom/panel-appearance.txt): tabs
    /// "Parts and surfaces | Sketches and curves", then one row per part or
    /// sketch: the name in bold 14 px on `#eaeaea` beside its swatch bar
    /// (half the width each, 29 px, 1 px `#aaa`). Face and feature
    /// appearances follow their part. Double-click a row to edit it.
    pub(super) fn render_appearance_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let names = self.labeler(cx);
        let sketches_tab = self.ui.appearance_sketches;
        let grid = rgb_u32(0xaaaaaa);
        // Live (part-studio/173): a selected part's row has a 2 px `#1651b0`
        // outline and `#def1fb` cells; the cells keep their height inside it.
        let swatch = |color: [f32; 4], selected: bool| {
            let c = gpui_kit::Rgba { r: lin_to_srgb(color[0]) as f32 / 255.0, g: lin_to_srgb(color[1]) as f32 / 255.0, b: lin_to_srgb(color[2]) as f32 / 255.0, a: color[3].clamp(0.0, 1.0) };
            div()
                .flex_1()
                .h(px(if selected { 25. } else { 29. }))
                .p(px(if selected { 2. } else { 3. }))
                .bg(rgb_u32(if selected { 0xdef1fb } else { 0xeaeaea }))
                .when(!selected, |d| d.border_t_1().border_r_1().border_b_1())
                .border_color(grid)
                .child(
                    // Translucent colors show over a checkerboard (live `checkerboard-background`).
                    div().size_full().rounded(px(4.)).border_1().border_color(grid).bg(rgb_u32(0xffffff)).child(div().size_full().rounded(px(3.)).bg(c)).when(c.a < 0.99, |d| {
                        d.child(
                            canvas(
                                |_, _, _| {},
                                |b, _, window, _| {
                                    let cell = px(5.);
                                    let (cols, rows) = ((b.size.width / cell).ceil() as i32, (b.size.height / cell).ceil() as i32);
                                    for r in 0..rows {
                                        for c in (r % 2..cols).step_by(2) {
                                            let o = point(b.origin.x + cell * c as f32, b.origin.y + cell * r as f32);
                                            window.paint_quad(fill(Bounds::new(o, size(cell, cell)).intersect(&b), rgb_u32(0xd7d7d7)));
                                        }
                                    }
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                    }),
                )
        };
        let row = |id: SharedString, color: [f32; 4], label: String, indent: bool, selected: bool| {
            h_flex()
                .id(id)
                .w_full()
                .cursor_pointer()
                .when(selected, |d| d.border_2().border_color(rgb_u32(0x1651b0)))
                .child(
                    div()
                        .flex_1()
                        .h(px(if selected { 25. } else { 29. }))
                        .flex()
                        .items_center()
                        .pl(px(if indent { 18. } else { 4. }) - px(if selected { 2. } else { 0. }))
                        .bg(rgb_u32(if selected { 0xdef1fb } else { 0xeaeaea }))
                        .map(|d| if selected { d.border_r_1() } else { d.border_1() })
                        .border_color(grid)
                        .text_size(px(14.))
                        .when(!indent, |d| d.font_weight(FontWeight::BOLD))
                        .text_color(pal.text)
                        .truncate()
                        .child(label),
                )
                .child(swatch(color, selected))
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        if !sketches_tab {
            for body in self.display.bodies.iter().filter(|b| b.kind != noshape_model::display::BodyKind::Wire) {
                let props = studio.part_properties.get(&body.id);
                let color = props.and_then(|p| p.appearance).map(|a| a.color).unwrap_or(body.color);
                let (b1, b2) = (body.id.clone(), body.id.clone());
                let custom = props.is_some_and(|p| p.appearance.is_some());
                let view = cx.entity();
                rows.push(
                    row(SharedString::from(format!("ap-part-{}", body.id)), color, body.name.clone(), false, self.selection.contains(&Reference::Body(body.id.clone())))
                        // Live: a click selects the part (row and part
                        // highlighted), a double click edits its appearance.
                        .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                            if ev.click_count() >= 2 {
                                this.edit_appearance(EditTarget::Parts(vec![b1.clone()]), w, cx);
                            } else {
                                this.pick_body(b1.clone(), cx);
                            }
                        }))
                        .when(custom, |d| {
                            d.os_context_menu(move |m, _, _| {
                                let v = view.clone();
                                let b = b2.clone();
                                m.item(crate::os_menu::OsMenuItem::new("Remove appearance").on_click(move |_, _, cx| v.update(cx, |this, cx| this.reset_part_appearance(b.clone(), cx))))
                            })
                        })
                        .into_any_element(),
                );
            }
        }
        for (i, a) in studio.appearances.iter().enumerate() {
            let is_sketch = matches!(&a.target, AppearanceTarget::Feature(f) if self.display.sketches.iter().any(|s| s.feature == *f));
            if is_sketch != sketches_tab {
                continue;
            }
            let (label, target) = match &a.target {
                AppearanceTarget::Faces(f) => (format!("{} face{}", f.len(), if f.len() == 1 { "" } else { "s" }), EditTarget::Faces(f.clone(), Some(i))),
                AppearanceTarget::Feature(f) => (names(&Reference::Feature(*f)), EditTarget::Feature(*f, Some(i))),
            };
            let faces = match &a.target {
                AppearanceTarget::Faces(f) => f.clone(),
                AppearanceTarget::Feature(_) => Vec::new(),
            };
            let view = cx.entity();
            rows.push(
                row(SharedString::from(format!("ap-face-{i}")), a.color, label, !is_sketch, false)
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        // Hovering a face appearance highlights its faces.
                        this.ui.appearance_hover = if *h { faces.clone() } else { Vec::new() };
                        this.refresh_scene(false, cx);
                    }))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                        if ev.click_count() >= 2 {
                            this.edit_appearance(target.clone(), w, cx);
                        }
                    }))
                    .os_context_menu(move |m, _, _| {
                        let v = view.clone();
                        m.item(crate::os_menu::OsMenuItem::new("Remove appearance").on_click(move |_, _, cx| v.update(cx, |this, cx| this.remove_appearance(i, cx))))
                    })
                    .into_any_element(),
            );
        }
        if sketches_tab {
            // Sketches without their own appearance, in the inactive sketch color.
            let with_own: Vec<FeatureId> = studio.appearances.iter().filter_map(|a| if let AppearanceTarget::Feature(f) = a.target { Some(f) } else { None }).collect();
            for sk in self.display.sketches.iter().filter(|s| !with_own.contains(&s.feature)) {
                let f = sk.feature;
                let color = sk.color.unwrap_or_else(|| crate::theme::lin(crate::theme::viewport::INACTIVE));
                rows.push(
                    row(SharedString::from(format!("ap-sk-{f}")), color, sk.name.clone(), false, false)
                        .on_click(cx.listener(move |this, ev: &ClickEvent, w, cx| {
                            if ev.click_count() >= 2 {
                                this.edit_appearance(EditTarget::Feature(f, None), w, cx);
                            }
                        }))
                        .into_any_element(),
                );
            }
        }
        let tab = |id: &'static str, label: &'static str, on: bool, sketches: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.33))
                .cursor_pointer()
                .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(pal.border))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.appearance_sketches = sketches;
                    cx.notify();
                }))
        };
        v_flex()
            .child(h_flex().w_full().mt(px(-5.)).mx(px(-5.)).child(tab("ap-tab-parts", "Parts and surfaces", !sketches_tab, false, cx)).child(tab("ap-tab-sketches", "Sketches and curves", sketches_tab, true, cx)))
            .child(v_flex().pt(px(10.)).children(rows))
            .into_any_element()
    }

    /// The "Add face appearance" button at the right of the Appearances title.
    pub(super) fn render_add_face_appearance(&mut self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("ap-add-face")
            .size(px(26.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .cursor_pointer()
            .hover(|d| d.bg(p().panel_hover))
            .child(crate::assets::icon("add-appearance", 20., cx))
            .tooltip(crate::ui::tooltip("Add face appearance"))
            .on_click(cx.listener(|this, _: &ClickEvent, w, cx| {
                let faces = this.selected_faces();
                this.edit_appearance(EditTarget::Faces(faces, None), w, cx);
            }))
            .into_any_element()
    }
}

fn rgb_u32(hex: u32) -> gpui_kit::Rgba {
    gpui_kit::rgb(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn hex_and_srgb_round_trip() {
        assert_eq!(parse_hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_hex("f80"), Some([255, 136, 0]));
        assert_eq!(parse_hex("nope"), None);
        assert_eq!(hex_field([1, 2, 255]), "0102FF");
        for c in [0u8, 17, 128, 200, 255] {
            assert_eq!(lin_to_srgb(srgb_to_lin(c)), c);
        }
    }
}
