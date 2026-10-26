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
/// templates: the first sheet primary, the second (if any) continuation.
pub fn to_dwt(pages: &[super::sheet::SheetGeometry], templates: &[SheetTemplate], insunits: u8, version: acadrust::DxfVersion) -> Result<Vec<u8>, String> {
    let mut doc = super::cad_export::cad_document_pages(pages, insunits, version)?;
    let json = serde_json::to_string(templates).map_err(|e| e.to_string())?;
    doc.summary_info.custom_properties.push((PROPERTY.to_string(), json));
    acadrust::DwgWriter::write_to_vec(&doc).map_err(|e| e.to_string())
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
        let bytes = to_dwt(&[g], std::slice::from_ref(&t), 4, acadrust::DxfVersion::AC1027).unwrap();
        let back = read_dwt(&bytes).unwrap();
        assert_eq!(back, vec![t.clone()]);
        let mut fresh = Drawing::default();
        fresh.apply_dwt(0, "x.dwt", &back);
        assert_eq!(fresh.sheets[0].size, SheetSize::A2);
        assert!(fresh.sheets[0].title_block.fields.iter().any(|(k, v)| k == "FINISH" && v == "NS EDIT"));
        assert_eq!(fresh.sheets[0].dwt.as_deref(), Some("x.dwt"));
        fresh.detach_dwt(0);
        assert!(fresh.sheets[0].dwt.is_none());
    }
}
