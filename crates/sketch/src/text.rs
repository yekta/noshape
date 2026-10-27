//! Sketch text: glyph outlines of the bundled Liberation fonts (SIL Open
//! Font License 1.1, see `fonts/LICENSE-Liberation-OFL.txt`) laid out in a
//! text box and converted to sketch curves.
//!
//! A text box (Onshape `skText`) is a construction rectangle: its lower-left
//! corner is the start of the first baseline and its height is the distance
//! from the baseline to the ascender of the first line. The box keeps the
//! aspect ratio of the laid-out text through a [`ConstraintKind::LengthRatio`]
//! constraint, so dragging or dimensioning a corner scales the text. The
//! glyph curves themselves are derived geometry: they are regenerated from
//! the solved box whenever the sketch is evaluated (see
//! [`Sketch::add_text_outlines`]).

use crate::constraints::ConstraintKind;
use crate::entities::Geometry;
use crate::ids::EntityId;
use crate::nurbs::clamped_uniform_knots;
use crate::shapes::ShapeIds;
use crate::sketch::{Result, Sketch, SketchError};
use glam::DVec2;

/// Font families available for sketch text.
pub const FONTS: [&str; 3] = ["Liberation Sans", "Liberation Serif", "Liberation Mono"];

/// Default font family of the app's own glyph outlines (labels, cursors);
/// sketch text uses [`SKETCH_DEFAULT_FONT`]. The Liberation families stay
/// loadable for documents that used them.
pub const DEFAULT_FONT: &str = "Liberation Sans";

/// One family of the Text tool's font list (live dom/sketch-text-fonts: 32
/// Google fonts in this order; `bold` / `italic`: whether the B / I buttons
/// show for it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SketchFont {
    pub name: &'static str,
    pub bold: bool,
    pub italic: bool,
}

/// The Text tool's fonts (SIL OFL 1.1 / Apache 2.0, see `fonts/sketch/`).
pub const SKETCH_FONTS: [SketchFont; 32] = [
    SketchFont { name: "Allerta", bold: false, italic: false },
    SketchFont { name: "Allerta Stencil", bold: false, italic: false },
    SketchFont { name: "Arimo", bold: true, italic: true },
    SketchFont { name: "Balthazar", bold: false, italic: false },
    SketchFont { name: "Baumans", bold: false, italic: false },
    SketchFont { name: "Bebas Neue", bold: false, italic: false },
    SketchFont { name: "Comic Neue", bold: true, italic: true },
    SketchFont { name: "Courier Prime", bold: true, italic: true },
    SketchFont { name: "Didact Gothic", bold: false, italic: false },
    SketchFont { name: "Droid Sans Mono", bold: false, italic: false },
    SketchFont { name: "Inconsolata", bold: true, italic: false },
    SketchFont { name: "Inter", bold: true, italic: false },
    SketchFont { name: "Michroma", bold: false, italic: false },
    SketchFont { name: "MPLUSRounded1c", bold: true, italic: false },
    SketchFont { name: "Noto Sans", bold: true, italic: true },
    SketchFont { name: "Noto Sans Chinese Simplified", bold: true, italic: false },
    SketchFont { name: "Noto Sans Chinese Traditional", bold: true, italic: false },
    SketchFont { name: "Noto Sans Japanese", bold: true, italic: false },
    SketchFont { name: "Noto Sans Korean", bold: true, italic: false },
    SketchFont { name: "Noto Serif", bold: true, italic: true },
    SketchFont { name: "Open Sans", bold: true, italic: true },
    SketchFont { name: "Orbitron", bold: true, italic: false },
    SketchFont { name: "Oswald", bold: true, italic: false },
    SketchFont { name: "Poppins", bold: true, italic: true },
    SketchFont { name: "PTSans", bold: true, italic: true },
    SketchFont { name: "Rajdhani", bold: true, italic: false },
    SketchFont { name: "Roboto", bold: true, italic: true },
    SketchFont { name: "Roboto Slab", bold: true, italic: false },
    SketchFont { name: "Ropa Sans", bold: false, italic: true },
    SketchFont { name: "Sofia Sans", bold: true, italic: true },
    SketchFont { name: "Source Sans Pro", bold: true, italic: true },
    SketchFont { name: "Tinos", bold: true, italic: true },
];

/// The Text tool's default font (live).
pub const SKETCH_DEFAULT_FONT: &str = "Open Sans";

/// The Text tool's font files, zstd-compressed (unpacked on first use).
static SKETCH_FILES: [&[u8]; 55] = [
    include_bytes!("../fonts/sketch/Allerta-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/AllertaStencil-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/Arimo-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Arimo-Italic-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Balthazar-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/Baumans-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/BebasNeue-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/ComicNeue-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/ComicNeue-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/ComicNeue-Italic.ttf.zst"),
    include_bytes!("../fonts/sketch/ComicNeue-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/sketch/CourierPrime-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/CourierPrime-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/CourierPrime-Italic.ttf.zst"),
    include_bytes!("../fonts/sketch/CourierPrime-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/sketch/DidactGothic-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/DroidSansMono.ttf.zst"),
    include_bytes!("../fonts/sketch/Inconsolata-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Inter-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Michroma-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/MPLUSRounded1c-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/MPLUSRounded1c-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/NotoSansSC-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/NotoSansTC-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/NotoSansJP-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/NotoSansKR-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/NotoSerif-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/NotoSerif-Italic-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/OpenSans-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/OpenSans-Italic-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Orbitron-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Oswald-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Poppins-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/Poppins-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/Poppins-Italic.ttf.zst"),
    include_bytes!("../fonts/sketch/Poppins-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/sketch/PT_Sans-Web-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/PT_Sans-Web-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/PT_Sans-Web-Italic.ttf.zst"),
    include_bytes!("../fonts/sketch/PT_Sans-Web-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/sketch/Rajdhani-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/Rajdhani-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/Roboto-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Roboto-Italic-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/RobotoSlab-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/RopaSans-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/RopaSans-Italic.ttf.zst"),
    include_bytes!("../fonts/sketch/SofiaSans-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/SofiaSans-Italic-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/SourceSans3-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/SourceSans3-Italic-VF.ttf.zst"),
    include_bytes!("../fonts/sketch/Tinos-Regular.ttf.zst"),
    include_bytes!("../fonts/sketch/Tinos-Bold.ttf.zst"),
    include_bytes!("../fonts/sketch/Tinos-Italic.ttf.zst"),
    include_bytes!("../fonts/sketch/Tinos-BoldItalic.ttf.zst"),
];

static SKETCH_UNPACKED: [std::sync::OnceLock<&'static [u8]>; 55] = [const { std::sync::OnceLock::new() }; 55];

/// The drawing Font list's files: the releases Onshape's drawings use
/// (live asb 10-24 "drawing font files", verified against its PDF exports):
/// DejaVu 2.34 Sans / Sans Mono / Serif (Bitstream Vera license), Arimo /
/// Tinos 1.23, Open Sans 1.10, Noto Sans 1.05, Noto Sans UI 1.04, Noto
/// Serif 1.02 (Apache 2.0), each regular / bold / italic / bold italic;
/// Noto Sans SC / TC (Regular, Thin), JP, KR 1.004 (SIL OFL 1.1). Licenses
/// in `fonts/drawing/LICENSE-*.txt`; zstd-compressed, unpacked on first use.
static DRAWING_FILES: [&[u8]; 42] = [
    include_bytes!("../fonts/drawing/DejaVuSans.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSans-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSans-Oblique.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSans-BoldOblique.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSansMono.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSansMono-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSansMono-Oblique.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSansMono-BoldOblique.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSerif.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSerif-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSerif-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/DejaVuSerif-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/Arimo-Regular.ttf.zst"),
    include_bytes!("../fonts/drawing/Arimo-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/Arimo-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/Arimo-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/Tinos-Regular.ttf.zst"),
    include_bytes!("../fonts/drawing/Tinos-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/Tinos-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/Tinos-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/OpenSans-Regular.ttf.zst"),
    include_bytes!("../fonts/drawing/OpenSans-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/OpenSans-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/OpenSans-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSans-Regular.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSans-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSans-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSans-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSansUI-Regular.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSansUI-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSansUI-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSansUI-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSerif-Regular.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSerif-Bold.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSerif-Italic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSerif-BoldItalic.ttf.zst"),
    include_bytes!("../fonts/drawing/NotoSansSC-Regular.otf.zst"),
    include_bytes!("../fonts/drawing/NotoSansSC-Thin.otf.zst"),
    include_bytes!("../fonts/drawing/NotoSansTC-Regular.otf.zst"),
    include_bytes!("../fonts/drawing/NotoSansTC-Thin.otf.zst"),
    include_bytes!("../fonts/drawing/NotoSansJP-Regular.otf.zst"),
    include_bytes!("../fonts/drawing/NotoSansKR-Regular.otf.zst"),
];

static DRAWING_UNPACKED: [std::sync::OnceLock<&'static [u8]>; 42] = [const { std::sync::OnceLock::new() }; 42];

fn drawing_file(i: usize) -> &'static [u8] {
    DRAWING_UNPACKED[i].get_or_init(|| Box::leak(zstd::decode_all(DRAWING_FILES[i]).unwrap_or_default().into_boxed_slice()))
}

/// A drawing font's face: its file, and whether bold / italic are
/// synthesized (live: the CJK families have one face; Onshape emboldens
/// and slants it).
struct DrawingFace {
    data: &'static [u8],
    synth_bold: bool,
    synth_italic: bool,
}

/// The face of a drawing Font list family (live's 22). "Light" CJK is the
/// Thin face (live: NotoSansSC-Thin). The AR* / APMONO shape fonts draw in a
/// free stand-in until their own glyphs exist (parity S28).
fn drawing_face(font: &str, bold: bool, italic: bool) -> Option<DrawingFace> {
    let style = usize::from(bold) + 2 * usize::from(italic);
    let four = |base: usize| Some(DrawingFace { data: drawing_file(base + style), synth_bold: false, synth_italic: false });
    let one = |i: usize| Some(DrawingFace { data: drawing_file(i), synth_bold: bold, synth_italic: italic });
    match font {
        "DejaVu Sans" => four(0),
        "DejaVu Sans Mono" => four(4),
        "DejaVu Serif" => four(8),
        "Arimo" => four(12),
        "Tinos" => four(16),
        "Open Sans" => four(20),
        "Noto Sans" => four(24),
        "Noto Sans UI" => four(28),
        "Noto Serif" => four(32),
        "Noto Sans Chinese Simplified" => one(36),
        "Noto Sans Chinese Simplified Light" => one(37),
        "Noto Sans Chinese Traditional" => one(38),
        "Noto Sans Chinese Traditional Light" => one(39),
        "Noto Sans Japanese" => one(40),
        "Noto Sans Korean" => one(41),
        "APMONO" | "ARMono" => drawing_face("DejaVu Sans Mono", bold, italic),
        "ARComp" | "ARIsoP1" | "ARSimp" | "ARTxt" => drawing_face("DejaVu Sans", bold, italic),
        "ARItal" => drawing_face("DejaVu Sans", bold, true),
        _ => None,
    }
}

fn sketch_file(i: usize) -> &'static [u8] {
    SKETCH_UNPACKED[i].get_or_init(|| Box::leak(zstd::decode_all(SKETCH_FILES[i]).unwrap_or_default().into_boxed_slice()))
}

/// A face of a Text tool font: its file and, for a variable font, the weight.
enum SketchFace {
    File(usize, Option<f32>),
    /// The bundled static Noto Sans.
    Noto,
}

/// The face of `font` (a [`SKETCH_FONTS`] name) in a style it has; a style it
/// lacks falls back to the closest one it has.
fn sketch_face(font: &str, bold: bool, italic: bool) -> Option<SketchFace> {
    let some = |f: Option<(usize, Option<f32>)>| f.map(|(i, w)| SketchFace::File(i, w));
    let faces: [Option<SketchFace>; 4] = match font {
        "Allerta" => [some(Some((0, None))), some(None), some(None), some(None)],
        "Allerta Stencil" => [some(Some((1, None))), some(None), some(None), some(None)],
        "Arimo" => [some(Some((2, None))), some(Some((2, Some(700.0)))), some(Some((3, None))), some(Some((3, Some(700.0))))],
        "Balthazar" => [some(Some((4, None))), some(None), some(None), some(None)],
        "Baumans" => [some(Some((5, None))), some(None), some(None), some(None)],
        "Bebas Neue" => [some(Some((6, None))), some(None), some(None), some(None)],
        "Comic Neue" => [some(Some((7, None))), some(Some((8, None))), some(Some((9, None))), some(Some((10, None)))],
        "Courier Prime" => [some(Some((11, None))), some(Some((12, None))), some(Some((13, None))), some(Some((14, None)))],
        "Didact Gothic" => [some(Some((15, None))), some(None), some(None), some(None)],
        "Droid Sans Mono" => [some(Some((16, None))), some(None), some(None), some(None)],
        "Inconsolata" => [some(Some((17, None))), some(Some((17, Some(700.0)))), some(None), some(None)],
        "Inter" => [some(Some((18, None))), some(Some((18, Some(700.0)))), some(None), some(None)],
        "Michroma" => [some(Some((19, None))), some(None), some(None), some(None)],
        "MPLUSRounded1c" => [some(Some((20, None))), some(Some((21, None))), some(None), some(None)],
        "Noto Sans" => [Some(SketchFace::Noto), Some(SketchFace::Noto), Some(SketchFace::Noto), Some(SketchFace::Noto)],
        "Noto Sans Chinese Simplified" => [some(Some((22, None))), some(Some((22, Some(700.0)))), some(None), some(None)],
        "Noto Sans Chinese Traditional" => [some(Some((23, None))), some(Some((23, Some(700.0)))), some(None), some(None)],
        "Noto Sans Japanese" => [some(Some((24, None))), some(Some((24, Some(700.0)))), some(None), some(None)],
        "Noto Sans Korean" => [some(Some((25, None))), some(Some((25, Some(700.0)))), some(None), some(None)],
        "Noto Serif" => [some(Some((26, None))), some(Some((26, Some(700.0)))), some(Some((27, None))), some(Some((27, Some(700.0))))],
        "Open Sans" => [some(Some((28, None))), some(Some((28, Some(700.0)))), some(Some((29, None))), some(Some((29, Some(700.0))))],
        "Orbitron" => [some(Some((30, None))), some(Some((30, Some(700.0)))), some(None), some(None)],
        "Oswald" => [some(Some((31, None))), some(Some((31, Some(700.0)))), some(None), some(None)],
        "Poppins" => [some(Some((32, None))), some(Some((33, None))), some(Some((34, None))), some(Some((35, None)))],
        "PTSans" => [some(Some((36, None))), some(Some((37, None))), some(Some((38, None))), some(Some((39, None)))],
        "Rajdhani" => [some(Some((40, None))), some(Some((41, None))), some(None), some(None)],
        "Roboto" => [some(Some((42, None))), some(Some((42, Some(700.0)))), some(Some((43, None))), some(Some((43, Some(700.0))))],
        "Roboto Slab" => [some(Some((44, None))), some(Some((44, Some(700.0)))), some(None), some(None)],
        "Ropa Sans" => [some(Some((45, None))), some(None), some(Some((46, None))), some(None)],
        "Sofia Sans" => [some(Some((47, None))), some(Some((47, Some(700.0)))), some(Some((48, None))), some(Some((48, Some(700.0))))],
        "Source Sans Pro" => [some(Some((49, None))), some(Some((49, Some(700.0)))), some(Some((50, None))), some(Some((50, Some(700.0))))],
        "Tinos" => [some(Some((51, None))), some(Some((52, None))), some(Some((53, None))), some(Some((54, None)))],
        _ => return None,
    };
    let [r, b, i, bi] = faces;
    match (bold, italic) {
        (true, true) => bi.or(b).or(i).or(r),
        (true, false) => b.or(r),
        (false, true) => i.or(r),
        (false, false) => r,
    }
}

/// The Text tool family of an Onshape font file name ("OpenSans-BoldItalic.ttf",
/// "RobotoSlab-Regular.ttf"), compared without spaces and case.
pub fn sketch_font_of_file(name: &str) -> Option<&'static str> {
    let stem = name.split(['-', '.']).next().unwrap_or(name).to_ascii_lowercase().replace([' ', '_'], "");
    SKETCH_FONTS.iter().map(|f| f.name).find(|n| n.to_ascii_lowercase().replace(' ', "") == stem)
}


/// Maximum number of characters in a text box (Onshape limit).
pub const MAX_CHARS: usize = 250;

/// Ids of glyph outline entities derived from text box `n` start at
/// `GLYPH_ID_BASE + n * GLYPH_ID_STRIDE`.
pub const GLYPH_ID_BASE: u32 = 1 << 30;
pub const GLYPH_ID_STRIDE: u32 = 1 << 16;

/// First glyph entity id of a text box.
pub fn glyph_id_base(text_id: u32) -> u32 {
    GLYPH_ID_BASE + (text_id % (1 << 13)) * GLYPH_ID_STRIDE
}

/// Text box id a derived glyph entity belongs to.
pub fn glyph_text_id(entity: EntityId) -> Option<u32> {
    (entity.0 >= GLYPH_ID_BASE).then(|| (entity.0 - GLYPH_ID_BASE) / GLYPH_ID_STRIDE)
}

static SANS: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationSans-Regular.ttf"),
    include_bytes!("../fonts/LiberationSans-Bold.ttf"),
    include_bytes!("../fonts/LiberationSans-Italic.ttf"),
    include_bytes!("../fonts/LiberationSans-BoldItalic.ttf"),
];
static SERIF: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("../fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("../fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("../fonts/LiberationSerif-BoldItalic.ttf"),
];
/// Noto Sans (SIL OFL 1.1): Onshape's default drawing font.
static NOTO_SANS: [&[u8]; 4] = [
    include_bytes!("../fonts/NotoSans-Regular.ttf"),
    include_bytes!("../fonts/NotoSans-Bold.ttf"),
    include_bytes!("../fonts/NotoSans-Italic.ttf"),
    include_bytes!("../fonts/NotoSans-BoldItalic.ttf"),
];
static MONO: [&[u8]; 4] = [
    include_bytes!("../fonts/LiberationMono-Regular.ttf"),
    include_bytes!("../fonts/LiberationMono-Bold.ttf"),
    include_bytes!("../fonts/LiberationMono-Italic.ttf"),
    include_bytes!("../fonts/LiberationMono-BoldItalic.ttf"),
];

/// Every bundled font file (Noto Sans and the Liberation families), e.g. for
/// registering them with the UI so text edited in place uses the same font.
pub fn font_files() -> impl Iterator<Item = &'static [u8]> {
    NOTO_SANS.iter().chain(SANS.iter()).chain(SERIF.iter()).chain(MONO.iter()).copied()
}

fn font_data(font: &str, bold: bool, italic: bool) -> &'static [u8] {
    let f = font.to_ascii_lowercase();
    let family = if f.contains("noto") {
        &NOTO_SANS
    } else if f.contains("serif") && !f.contains("sans") {
        &SERIF
    } else if f.contains("mono") {
        &MONO
    } else {
        &SANS
    };
    family[usize::from(bold) + 2 * usize::from(italic)]
}

/// The font files of a Text tool family, unpacked (each once), e.g. for the
/// Text dialog to draw its preview in the font.
pub fn sketch_font_files(font: &str) -> Vec<&'static [u8]> {
    let mut out: Vec<&'static [u8]> = Vec::new();
    for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
        let data = match sketch_face(font, bold, italic) {
            Some(SketchFace::File(i, _)) => sketch_file(i),
            Some(SketchFace::Noto) => font_data("Noto Sans", bold, italic),
            None => continue,
        };
        if !out.iter().any(|d| std::ptr::eq(*d, data)) {
            out.push(data);
        }
    }
    out
}

/// The family name a font file declares (typographic family, else family):
/// what a text system knows it by ("PT Sans" for the "PTSans" entry).
pub fn font_family_name(data: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let names = face.names();
    [ttf_parser::name_id::TYPOGRAPHIC_FAMILY, ttf_parser::name_id::FAMILY]
        .into_iter()
        .find_map(|id| names.into_iter().find(|n| n.name_id == id && n.is_unicode()).and_then(|n| n.to_string()))
}

/// One piece of a glyph contour, in box units.
#[derive(Clone, Debug, PartialEq)]
pub enum TextSegment {
    Line(DVec2, DVec2),
    /// Quadratic or cubic Bézier control polygon.
    Bezier(Vec<DVec2>),
}

impl TextSegment {
    fn start(&self) -> DVec2 {
        match self {
            TextSegment::Line(a, _) => *a,
            TextSegment::Bezier(c) => c[0],
        }
    }
    fn end(&self) -> DVec2 {
        match self {
            TextSegment::Line(_, b) => *b,
            TextSegment::Bezier(c) => *c.last().unwrap_or(&DVec2::ZERO),
        }
    }
}

/// Laid-out text in box units: the first baseline starts at the origin and
/// the first line's ascender is at `y = 1`. Later lines go below.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLayout {
    /// Closed glyph contours.
    pub contours: Vec<Vec<TextSegment>>,
    /// Advance width of the longest line.
    pub width: f64,
    pub lines: usize,
}

impl TextLayout {
    /// Polylines approximating the contours (previews).
    pub fn polylines(&self, steps: usize) -> Vec<Vec<DVec2>> {
        let steps = steps.max(1);
        self.contours
            .iter()
            .map(|c| {
                let mut pts = Vec::new();
                for s in c {
                    match s {
                        TextSegment::Line(a, b) => {
                            if pts.is_empty() {
                                pts.push(*a);
                            }
                            pts.push(*b);
                        }
                        TextSegment::Bezier(ctrl) => {
                            if pts.is_empty() {
                                pts.push(ctrl[0]);
                            }
                            pts.extend((1..=steps).map(|i| de_casteljau(ctrl, i as f64 / steps as f64)));
                        }
                    }
                }
                pts
            })
            .collect()
    }
}

fn de_casteljau(pts: &[DVec2], t: f64) -> DVec2 {
    let mut v = pts.to_vec();
    while v.len() > 1 {
        for i in 0..v.len() - 1 {
            v[i] = v[i].lerp(v[i + 1], t);
        }
        v.pop();
    }
    v[0]
}

struct Outline {
    scale: f64,
    origin: DVec2,
    contours: Vec<Vec<TextSegment>>,
    cur: Vec<TextSegment>,
    last: DVec2,
    start: DVec2,
}

impl Outline {
    fn p(&self, x: f32, y: f32) -> DVec2 {
        self.origin + DVec2::new(x as f64, y as f64) * self.scale
    }
    fn flush(&mut self) {
        if (self.last - self.start).length() > 1e-9 && !self.cur.is_empty() {
            self.cur.push(TextSegment::Line(self.last, self.start));
        }
        if !self.cur.is_empty() {
            self.contours.push(std::mem::take(&mut self.cur));
        }
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush();
        self.start = self.p(x, y);
        self.last = self.start;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let q = self.p(x, y);
        if (q - self.last).length() > 1e-9 {
            self.cur.push(TextSegment::Line(self.last, q));
        }
        self.last = q;
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, q) = (self.p(x1, y1), self.p(x, y));
        if (q - self.last).length() > 1e-9 {
            self.cur.push(TextSegment::Bezier(vec![self.last, c, q]));
        }
        self.last = q;
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, q) = (self.p(x1, y1), self.p(x2, y2), self.p(x, y));
        if (q - self.last).length() > 1e-9 {
            self.cur.push(TextSegment::Bezier(vec![self.last, c1, c2, q]));
        }
        self.last = q;
    }
    fn close(&mut self) {
        self.flush();
    }
}

/// Lays out `text` (newlines start new lines) in box units.
pub fn layout_text(text: &str, font: &str, bold: bool, italic: bool) -> TextLayout {
    let (data, wght) = match sketch_face(font, bold, italic) {
        Some(SketchFace::File(i, w)) => (sketch_file(i), w),
        Some(SketchFace::Noto) => (font_data("Noto Sans", bold, italic), None),
        None => (font_data(font, bold, italic), None),
    };
    layout_face(text, data, wght, false, false)
}

/// Lays out drawing text (live's 22 families in the files Onshape's
/// drawings use, see [`drawing_face`]); other names as [`layout_text`].
pub fn layout_drawing_text(text: &str, font: &str, bold: bool, italic: bool) -> TextLayout {
    match drawing_face(font, bold, italic) {
        Some(f) => layout_face(text, f.data, None, f.synth_bold, f.synth_italic),
        None => layout_text(text, font, bold, italic),
    }
}

/// A drawing font face as a PDF embeds it (Text: Selectable): the file,
/// the synthesized styles, and the metrics [`layout_drawing_text`] lays out
/// with (`em` and `line_h` in its units, where the ascender is 1).
pub struct EmbedFace {
    pub data: &'static [u8],
    pub synth_bold: bool,
    pub synth_italic: bool,
    pub units_per_em: u16,
    pub em: f64,
    pub line_h: f64,
    /// In font units: ascender, descender, cap height, bbox.
    pub ascender: i16,
    pub descender: i16,
    pub cap_height: i16,
    pub bbox: [i16; 4],
    pub postscript_name: String,
    pub italic_angle: f32,
    /// The Font list family ("Noto Sans") and whether bold (the PDF font
    /// descriptor's FontFamily and FontWeight).
    pub family: String,
    pub bold: bool,
}

/// [`EmbedFace`] of a drawing Font list family (None for the sketch fonts
/// and files that don't parse).
pub fn drawing_embed_face(font: &str, bold: bool, italic: bool) -> Option<EmbedFace> {
    let f = drawing_face(font, bold, italic)?;
    let face = ttf_parser::Face::parse(f.data, 0).ok()?;
    let asc = (face.ascender() as f64).max(1.0);
    let r = face.global_bounding_box();
    // As the file names it (live's PDFs: "AAAAAA+NotoSans", "…+DejaVuSans").
    let postscript_name = face.names().into_iter().find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME).and_then(|n| n.to_string()).unwrap_or_else(|| font.replace(' ', ""));
    Some(EmbedFace {
        data: f.data,
        synth_bold: f.synth_bold,
        synth_italic: f.synth_italic,
        units_per_em: face.units_per_em(),
        em: face.units_per_em() as f64 / asc,
        line_h: (face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) / asc,
        ascender: face.ascender(),
        descender: face.descender(),
        cap_height: face.capital_height().unwrap_or(face.ascender()),
        bbox: [r.x_min, r.y_min, r.x_max, r.y_max],
        postscript_name,
        italic_angle: face.italic_angle(),
        family: font.to_string(),
        bold,
    })
}

/// The glyphs [`layout_drawing_text`] draws, per line: (glyph id, the
/// character, advance in font units). Missing characters are '?' as there.
pub fn drawing_glyphs(text: &str, data: &[u8]) -> Vec<Vec<(u16, char, u16)>> {
    let Ok(face) = ttf_parser::Face::parse(data, 0) else { return Vec::new() };
    text.chars()
        .take(MAX_CHARS)
        .collect::<String>()
        .lines()
        .map(|line| {
            line.chars()
                .filter_map(|ch| {
                    let gid = face.glyph_index(ch).or_else(|| face.glyph_index('?'))?;
                    Some((gid.0, ch, face.glyph_hor_advance(gid).unwrap_or(0)))
                })
                .collect()
        })
        .collect()
}

/// Synthesized italic: the slant (x per y, live asb 10-24: 0.27–0.30).
pub const SYNTH_SLANT: f64 = 0.28;
/// Synthesized bold: the extra advance per glyph (live: about +30/1000 em)
/// and the outline drawn again that far right.
pub const SYNTH_BOLD: f64 = 0.03;

/// [`layout_text`] of one face (`wght`: a variable font's weight).
fn layout_face(text: &str, data: &[u8], wght: Option<f32>, synth_bold: bool, synth_italic: bool) -> TextLayout {
    let Ok(mut face) = ttf_parser::Face::parse(data, 0) else { return TextLayout::default() };
    // A variable font's weight (Bold = 700; otherwise its default instance).
    if let Some(w) = wght {
        face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), w);
    }
    let asc = (face.ascender() as f64).max(1.0);
    let scale = 1.0 / asc;
    let em = face.units_per_em() as f64 * scale;
    let line_h = (face.ascender() as f64 - face.descender() as f64 + face.line_gap() as f64) * scale;
    let mut out = TextLayout::default();
    for (li, line) in text.chars().take(MAX_CHARS).collect::<String>().lines().enumerate() {
        let mut pen = DVec2::new(0.0, -line_h * li as f64);
        for ch in line.chars() {
            let Some(gid) = face.glyph_index(ch).or_else(|| face.glyph_index('?')) else { continue };
            let mut ol = Outline { scale, origin: pen, contours: Vec::new(), cur: Vec::new(), last: pen, start: pen };
            face.outline_glyph(gid, &mut ol);
            ol.flush();
            let mut glyph: Vec<Vec<TextSegment>> = ol.contours.into_iter().filter(|c| c.len() >= 2 || matches!(c.first(), Some(TextSegment::Bezier(_)))).collect();
            // Synthesized styles (fonts with one face): slanted about the
            // baseline; emboldened by drawing the outline again to the right.
            if synth_italic {
                let base = pen.y;
                glyph = glyph.iter().map(|c| map_contour(c, &|p| DVec2::new(p.x + (p.y - base) * SYNTH_SLANT, p.y))).collect();
            }
            if synth_bold {
                let dx = SYNTH_BOLD * em;
                let shifted: Vec<Vec<TextSegment>> = glyph.iter().map(|c| map_contour(c, &|p| DVec2::new(p.x + dx, p.y))).collect();
                glyph.extend(shifted);
            }
            out.contours.extend(glyph);
            pen.x += face.glyph_hor_advance(gid).unwrap_or(0) as f64 * scale + if synth_bold { SYNTH_BOLD * em } else { 0.0 };
        }
        out.width = out.width.max(pen.x);
        out.lines = li + 1;
    }
    out
}

/// A contour with every point moved by `f`.
fn map_contour(c: &[TextSegment], f: &dyn Fn(DVec2) -> DVec2) -> Vec<TextSegment> {
    c.iter()
        .map(|s| match s {
            TextSegment::Line(a, b) => TextSegment::Line(f(*a), f(*b)),
            TextSegment::Bezier(pts) => TextSegment::Bezier(pts.iter().map(|p| f(*p)).collect()),
        })
        .collect()
}

/// Placement of a text layout in the sketch: box corner, axis directions and
/// height (from the solved text box), plus Onshape's flip options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPlacement {
    /// Lower-left corner of the box (start of the first baseline).
    pub origin: DVec2,
    /// Unit vector along the baseline.
    pub x: DVec2,
    /// Unit vector from the baseline toward the ascender.
    pub y: DVec2,
    pub height: f64,
    /// Flip about the box's horizontal center line.
    pub mirror_horizontal: bool,
    /// Flip about the box's vertical center line.
    pub mirror_vertical: bool,
}

impl TextPlacement {
    /// Maps a layout point (box units) into sketch coordinates.
    pub fn map(&self, layout_width: f64, p: DVec2) -> DVec2 {
        let u = if self.mirror_vertical { layout_width - p.x } else { p.x };
        let v = if self.mirror_horizontal { 1.0 - p.y } else { p.y };
        self.origin + (self.x * u + self.y * v) * self.height
    }
}

/// Points and constraints of a text box created by [`Sketch::add_text_box`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextBoxIds {
    /// Corners: lower-left, lower-right, upper-right, upper-left.
    pub points: [EntityId; 4],
    /// Construction lines: bottom, right, top, left.
    pub lines: [EntityId; 4],
    /// The aspect ratio constraint (bottom length / left length).
    pub ratio: Option<crate::ids::ConstraintId>,
    pub shape: ShapeIds,
}

impl Sketch {
    /// Creates a text box: a construction rectangle with its lower-left corner
    /// at `origin`, the given `height` and `aspect` (width / height), a
    /// horizontal bottom edge and an aspect-ratio constraint.
    pub fn add_text_box(&mut self, origin: DVec2, height: f64, aspect: f64) -> Result<TextBoxIds> {
        if !(height > 0.0 && height.is_finite()) {
            return Err(SketchError::InvalidGeometry("text height must be positive".into()));
        }
        let aspect = if aspect.is_finite() && aspect > 1e-6 { aspect } else { 0.5 };
        let width = height * aspect;
        let shape = self.three_point_rectangle(origin, origin + DVec2::X * width, origin + DVec2::Y * height)?;
        for l in &shape.curves {
            self.set_construction(*l, true)?;
        }
        let pts = [shape.points[0], shape.points[1], shape.points[2], shape.points[3]];
        let lines = [shape.curves[0], shape.curves[1], shape.curves[2], shape.curves[3]];
        let mut out = TextBoxIds { points: pts, lines, ratio: None, shape };
        if let Ok(c) = self.add_constraint(ConstraintKind::Horizontal { a: lines[0], b: None }) {
            out.shape.constraints.push(c);
        }
        // Left edge runs upper-left -> lower-left; its length is the height.
        let ratio = self.add_constraint(ConstraintKind::LengthRatio { a: lines[0], b: lines[3], ratio: aspect })?;
        out.shape.constraints.push(ratio);
        out.ratio = Some(ratio);
        Ok(out)
    }

    /// Placement of a text from its box corners (lower-left, lower-right,
    /// upper-right, upper-left).
    pub fn text_placement(&self, corners: &[EntityId], mirror_horizontal: bool, mirror_vertical: bool) -> Option<TextPlacement> {
        let p0 = self.point(*corners.first()?)?;
        let p1 = self.point(*corners.get(1)?)?;
        let p3 = self.point(*corners.get(3)?)?;
        let x = (p1 - p0).try_normalize()?;
        let up = p3 - p0;
        let height = up.length();
        // Keep a right-handed frame even if the box was flipped by dragging.
        let y = up.try_normalize().unwrap_or(x.perp());
        (height > 1e-12).then_some(TextPlacement { origin: p0, x, y, height, mirror_horizontal, mirror_vertical })
    }

    /// Adds the outlines of a laid-out text as fixed (external) curves.
    /// With `id_base`, entities get consecutive ids starting there (stable
    /// derived geometry, see [`Sketch::add_entity_with_id`]); otherwise ids
    /// are allocated normally. Returns the created curves.
    pub fn add_text_outlines(&mut self, layout: &TextLayout, place: &TextPlacement, id_base: Option<u32>) -> Vec<EntityId> {
        let mut next = id_base;
        let limit = id_base.map(|b| b.saturating_add(GLYPH_ID_STRIDE - 1));
        let mut add = |sk: &mut Sketch, g: Geometry| -> Option<EntityId> {
            match next.as_mut() {
                Some(n) => {
                    if limit.is_some_and(|l| *n > l) {
                        return None;
                    }
                    let id = EntityId(*n);
                    *n += 1;
                    sk.add_entity_with_id(id, g, false, true).ok()
                }
                None => sk.add_entity(g, false, true).ok(),
            }
        };
        let map = |p: DVec2| place.map(layout.width, p);
        let mut curves = Vec::new();
        for contour in &layout.contours {
            let Some(first) = contour.first() else { continue };
            let Some(start) = add(self, Geometry::Point { pos: map(first.start()) }) else { break };
            let mut prev = start;
            for (i, seg) in contour.iter().enumerate() {
                let last = i + 1 == contour.len();
                let end = if last && (seg.end() - first.start()).length() < 1e-9 {
                    start
                } else {
                    match add(self, Geometry::Point { pos: map(seg.end()) }) {
                        Some(p) => p,
                        None => break,
                    }
                };
                let g = match seg {
                    TextSegment::Line(..) => {
                        if end == prev {
                            continue;
                        }
                        Geometry::Line { start: prev, end }
                    }
                    TextSegment::Bezier(ctrl) => {
                        let mut control = vec![prev];
                        for q in &ctrl[1..ctrl.len() - 1] {
                            match add(self, Geometry::Point { pos: map(*q) }) {
                                Some(p) => control.push(p),
                                None => break,
                            }
                        }
                        control.push(end);
                        let degree = ctrl.len() - 1;
                        if control.len() != ctrl.len() {
                            break;
                        }
                        Geometry::BSpline { control, degree, knots: clamped_uniform_knots(ctrl.len(), degree), weights: None }
                    }
                };
                if let Some(c) = add(self, g) {
                    curves.push(c);
                }
                prev = end;
            }
        }
        curves
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_metrics() {
        let l = layout_text("Hi", DEFAULT_FONT, false, false);
        assert_eq!(l.lines, 1);
        assert!(l.width > 0.5 && l.width < 2.0, "width {}", l.width);
        // H: one contour, i: two contours.
        assert_eq!(l.contours.len(), 3);
        let bold = layout_text("Hi", DEFAULT_FONT, true, false);
        assert!(bold.width > l.width);
        let two = layout_text("A\nB", "Liberation Mono", false, true);
        assert_eq!(two.lines, 2);
        let ys: Vec<f64> = two.polylines(4).iter().flatten().map(|p| p.y).collect();
        assert!(ys.iter().cloned().fold(f64::INFINITY, f64::min) < -0.5, "second line below the first");
    }
}

#[cfg(test)]
mod drawing_font_tests {
    use super::*;

    /// Live's 22 drawing families all lay out real glyphs; DejaVu is its own
    /// face, and the CJK "Light" ones are thinner than their regular.
    #[test]
    fn drawing_font_families_have_faces() {
        let families = ["APMONO", "ARComp", "ARIsoP1", "ARItal", "ARMono", "ARSimp", "ARTxt", "Arimo", "DejaVu Sans", "DejaVu Sans Mono", "DejaVu Serif", "Noto Sans", "Noto Sans Chinese Simplified", "Noto Sans Chinese Simplified Light", "Noto Sans Chinese Traditional", "Noto Sans Chinese Traditional Light", "Noto Sans Japanese", "Noto Sans Korean", "Noto Sans UI", "Noto Serif", "Open Sans", "Tinos"];
        for f in families {
            let l = layout_drawing_text("Hag", f, false, false);
            assert!(l.width > 0.0 && !l.contours.is_empty(), "{f}");
        }
        assert_ne!(layout_drawing_text("Hag", "DejaVu Sans", false, false).width, layout_drawing_text("Hag", "Noto Sans", false, false).width);
        let ink = |f: &str| -> usize { layout_drawing_text("永", f, false, false).contours.iter().map(|c| c.len()).sum() };
        assert!(ink("Noto Sans Chinese Simplified Light") > 0 && ink("Noto Sans Chinese Simplified") > 0);
        let wide = |f: &str, b: bool| layout_drawing_text("Hag", f, b, false).width;
        assert!(wide("DejaVu Sans", true) > wide("DejaVu Sans", false));
        // CJK: one face; bold is synthesized (wider, the outline twice), italic slanted.
        let w = |b: bool| layout_drawing_text("永永", "Noto Sans Japanese", b, false);
        assert!(w(true).width > w(false).width && w(true).contours.len() == 2 * w(false).contours.len());
        let right = |l: &TextLayout| l.polylines(2).iter().flatten().map(|p| p.x).fold(f64::MIN, f64::max);
        assert!(right(&layout_drawing_text("l", "Noto Sans Korean", false, true)) > right(&layout_drawing_text("l", "Noto Sans Korean", false, false)));
    }
}
