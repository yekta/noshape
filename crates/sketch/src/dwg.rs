//! DWG reader for "Insert DXF or DWG", built on the clean-room `cadkit-dwg`
//! crate (R13/R14 through 2018: `AC1012` … `AC1032`). The document goes
//! through the same converter as DXF ([`crate::cad`]): model space, block
//! references expanded, curves mapped exactly onto the drawing plane.

use crate::dxf::DxfDrawing;
use crate::sketch::{Result, SketchError};
use cadkit_core::ReadOptions;

pub use crate::cad::parse_cad;

/// Parses a binary DWG file (R13/R14 – 2018) into model-space sketch geometry.
pub fn parse_dwg(bytes: &[u8]) -> Result<DxfDrawing> {
    if !cadkit_dwg::sniff(bytes) {
        let magic = String::from_utf8_lossy(bytes.get(..6).unwrap_or(bytes)).into_owned();
        return Err(SketchError::OperationFailed(format!("unsupported DWG version {magic:?} (R13 to 2018 are supported)")));
    }
    let doc = cadkit_dwg::read(bytes, &ReadOptions::default()).map_err(|e| SketchError::OperationFailed(format!("cannot read the DWG file: {e}")))?;
    crate::cad::from_document(&doc, "DWG")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_format() {
        use crate::dxf::DxfEntity;
        use glam::DVec2;
        assert!(parse_cad(b"AC1009 old").is_err());
        assert!(parse_cad(b"AC1015").is_err());
        assert!(parse_cad(b"hello").is_err());
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nPOINT\n10\n1\n20\n2\n0\nENDSEC\n0\nEOF\n";
        let d = parse_cad(format!("\u{feff}{dxf}").as_bytes()).unwrap();
        assert_eq!(d.entities, vec![DxfEntity::Point(DVec2::new(1.0, 2.0))]);
    }
}
