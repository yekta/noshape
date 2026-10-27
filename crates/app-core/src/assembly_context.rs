//! Edit in context: the assembly shown around a part edited in its Part Studio (the Assembly's instance menu
//! creates it, the Part Studio shows it), and Onshape's in-context arrow.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::assembly::InstanceId;
use noshape_model::display::MeshBuffers;
use std::sync::Arc;

/// One instance of the assembly, in the Part Studio's coordinates.
#[derive(Clone)]
pub struct ContextOccurrence {
    pub top: InstanceId,
    pub name: String,
    pub mesh: Arc<MeshBuffers>,
    pub transform: glam::DMat4,
    pub color: [f32; 4],
}

/// The assembly shown around the edited part.
#[derive(Clone)]
pub struct AssemblyContext {
    pub assembly: noshape_model::document::TabId,
    pub assembly_name: String,
    /// The part being edited (selected on entry).
    pub part: Option<BodyId>,
    pub occurrences: Vec<ContextOccurrence>,
    /// Create Part Studio in context: where the new Part Studio's parts go
    /// in the assembly ("Insert and go to assembly").
    pub insert_at: Option<noshape_model::assembly::Placement>,
    /// The assembly's camera in the Part Studio's coordinates (Onshape
    /// keeps the view orientation and zoom).
    pub camera: Option<noshape_render::Camera>,
    /// The Part Studio's Assembly context (snapshot) shown: its bodies come
    /// from the regeneration (`StudioDisplay::context_bodies`), pickable
    /// for references; `None`: display-only `occurrences`.
    pub context: Option<u32>,
}

/// Onshape's in-context arrow ⇦ (20 px): `solid` = yellow fill (the active
/// context's features), else an outline; `dashed` = an instance that is not
/// the context's primary instance (live 505, 517, 520).
pub fn context_arrow_icon(solid: bool, dashed: bool) -> AnyElement {
    let ink = rgb(0x333333);
    div()
        .relative()
        .size(px(20.))
        .flex_none()
        .when(solid, |d| d.child(div().absolute().top_0().left_0().child(crate::ui::glyph("incontext-arrow-fill", 20., rgb(0xfab601)))))
        .child(div().absolute().top_0().left_0().child(crate::ui::glyph(if dashed { "incontext-arrow-dashed" } else { "incontext-arrow-outline" }, 20., ink)))
        .into_any_element()
}

/// Part Studio contexts (studio tab, context id) whose assembly has moved on
/// since their last update, as last seen by the assembly.
#[derive(Default)]
pub struct StaleContexts(pub std::collections::HashSet<(noshape_model::document::TabId, u32)>);

impl Global for StaleContexts {}
