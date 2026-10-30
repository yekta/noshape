//! Drawing templates (DWT) and Sheet properties › Select a DWT (live
//! behaviours "Sheet properties › Select a DWT", S13): picking a DWT gives
//! the sheet the template's size, border, zones and title block (property
//! fields stay live, edited cells keep their text); Format then shows the
//! file name and Size is hidden until Format › Custom detaches it. New
//! sheets get the template's continuation title block.
//!
//! noshape's DWT export carries its sheet formats (primary, continuation) as
//! a DWG custom property, so a template read back is exactly what was
//! exported.

pub use noshape_model_data::drawing::dwt::*;




/// The DWT bytes of `pages` (sheet geometry, formats only) with their
/// templates: the first sheet primary, the second (if any) continuation;
/// and the drawing's properties (without its own template name and lock).
pub fn to_dwt(pages: &[super::sheet::SheetGeometry], templates: &[SheetTemplate], props: Option<&super::DrawingProps>, insunits: u8, version: acadrust::DxfVersion) -> Result<Vec<u8>, String> {
    let mut doc = super::cad_export::cad_document_pages(pages, insunits, version)?;
    let json = serde_json::to_string(templates).map_err(|e| e.to_string())?;
    doc.summary_info.custom_properties.push((PROPERTY.to_string(), json));
    if let Some(p) = props {
        let p = super::DrawingProps { template: None, locked: false, ..p.clone() };
        doc.summary_info.custom_properties.push((PROPS_PROPERTY.to_string(), serde_json::to_string(&p).map_err(|e| e.to_string())?));
    }
    acadrust::DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
}

/// The drawing properties a noshape DWT carries (None: another DWT, or one
/// exported before they were written).
pub fn read_dwt_props(bytes: &[u8]) -> Option<super::DrawingProps> {
    let doc = acadrust::DwgReader::from_stream(std::io::Cursor::new(bytes.to_vec())).read().ok()?;
    let json = doc.summary_info.custom_properties.iter().find(|(k, _)| k == PROPS_PROPERTY).map(|(_, v)| v.clone())?;
    serde_json::from_str(&json).ok()
}

/// Create drawing › a DWT template (live request 1627): the new drawing's
/// first sheet takes the template's format, the continuation sheets its
/// second one, and the drawing its properties (when it carries them).
pub fn apply_dwt_template(d: &mut super::Drawing, name: &str, bytes: &[u8]) -> Result<(), String> {
    let templates = read_dwt(bytes)?;
    d.apply_dwt(0, name, &templates);
    if let Some(p) = read_dwt_props(bytes) {
        d.props = p;
    }
    Ok(())
}

/// The templates of a DWT exported by noshape (primary, continuation).
pub fn read_dwt(bytes: &[u8]) -> Result<Vec<SheetTemplate>, String> {
    let doc = acadrust::DwgReader::from_stream(std::io::Cursor::new(bytes.to_vec())).read().map_err(|e| e.to_string())?;
    let json = doc.summary_info.custom_properties.iter().find(|(k, _)| k == PROPERTY).map(|(_, v)| v.clone()).ok_or_else(|| "This template has no noshape sheet format".to_string())?;
    let t: Vec<SheetTemplate> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if t.is_empty() { Err("This template has no sheet".into()) } else { Ok(t) }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::*;

    #[test]
    fn dwt_round_trips_the_sheet_format() {
        let mut d = Drawing::default();
        d.sheets[0].size = SheetSize::A2;
        d.sheets[0].title_block.fields.push(("FINISH".into(), "NS EDIT".into()));
        let t = SheetTemplate::of(&d.sheets[0]);
        let (w, h) = d.sheets[0].size.size_mm();
        let mut g = super::super::sheet::SheetGeometry::new(w, h);
        g.add_border_and_title(&d.sheets[0]);
        let props = DrawingProps { decimal_comma: true, precision: Some(4), template: Some("mine.dwt".into()), locked: true, ..Default::default() };
        let bytes = to_dwt(&[g], std::slice::from_ref(&t), Some(&props), 4, acadrust::DxfVersion::AC1027).unwrap();
        let back = read_dwt(&bytes).unwrap();
        assert_eq!(back, vec![t.clone()]);
        // The properties come back, without the template name and lock.
        let p = read_dwt_props(&bytes).unwrap();
        assert_eq!((p.decimal_comma, p.precision, p.template.as_deref(), p.locked), (true, Some(4), None, false));
        let mut made = Drawing::default();
        apply_dwt_template(&mut made, "x.dwt", &bytes).unwrap();
        assert_eq!((made.sheets[0].size, made.props.precision), (SheetSize::A2, Some(4)));
        let mut fresh = Drawing::default();
        fresh.apply_dwt(0, "x.dwt", &back);
        assert_eq!(fresh.sheets[0].size, SheetSize::A2);
        assert!(fresh.sheets[0].title_block.fields.iter().any(|(k, v)| k == "FINISH" && v == "NS EDIT"));
        assert_eq!(fresh.sheets[0].dwt.as_deref(), Some("x.dwt"));
        fresh.detach_dwt(0);
        assert!(fresh.sheets[0].dwt.is_none());
    }
}
