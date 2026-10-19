//! File tabs (Onshape's imported or exported files kept in the document):
//! the file's bytes, name and upload time. A file tab shows a card with the
//! file's type, size and upload time, and can be downloaded, updated,
//! translated (CAD → Part Studio, DXF / DWG → Drawing) or deleted. Drawing
//! exports can be stored in a new file tab; Insert image / Insert DXF or
//! DWG / Select a DWT list them.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileTab {
    /// The file name with its extension ("plate.step").
    pub file_name: String,
    /// The bytes, base64 (shared: cloning a document copies no data).
    pub data_base64: Arc<str>,
    /// Size in bytes.
    pub size: u64,
    /// Upload (import / store) time, seconds since the Unix epoch.
    pub uploaded: i64,
}

/// What a file is, from its extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    /// STEP / IGES / BREP / STL / OBJ: translates to a Part Studio.
    Cad,
    /// DXF / DWG: translates to a Drawing.
    Drawing2d,
    /// DWT: a drawing template.
    Template,
    Image,
    Pdf,
    Csv,
    Other,
}

impl FileTab {
    pub fn new(file_name: &str, bytes: &[u8], uploaded: i64) -> Self {
        Self { file_name: file_name.to_string(), data_base64: crate::sketch_feature::base64_encode(bytes).into(), size: bytes.len() as u64, uploaded }
    }

    pub fn bytes(&self) -> Option<Vec<u8>> {
        crate::sketch_feature::base64_decode(&self.data_base64)
    }

    /// The extension in lower case ("step").
    pub fn ext(&self) -> String {
        std::path::Path::new(&self.file_name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
    }

    pub fn kind(&self) -> FileKind {
        match self.ext().as_str() {
            "step" | "stp" | "iges" | "igs" | "brep" | "stl" | "obj" => FileKind::Cad,
            "dxf" | "dwg" => FileKind::Drawing2d,
            "dwt" => FileKind::Template,
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "svg" => FileKind::Image,
            "pdf" => FileKind::Pdf,
            "csv" => FileKind::Csv,
            _ => FileKind::Other,
        }
    }

    /// The card's type line ("STEP file", "PNG image").
    pub fn type_label(&self) -> String {
        let e = self.ext().to_uppercase();
        let e = match e.as_str() {
            "STP" => "STEP".to_string(),
            "IGS" => "IGES".to_string(),
            "JPG" => "JPEG".to_string(),
            "TIF" => "TIFF".to_string(),
            "" => return "File".into(),
            _ => e,
        };
        match self.kind() {
            FileKind::Image => format!("{e} image"),
            FileKind::Pdf => "PDF document".into(),
            _ => format!("{e} file"),
        }
    }

    /// "49 KB", "1.2 MB", "312 bytes".
    pub fn size_label(&self) -> String {
        let s = self.size as f64;
        if s < 1024.0 {
            format!("{} bytes", self.size)
        } else if s < 1024.0 * 1024.0 {
            format!("{} KB", (s / 1024.0).round() as u64)
        } else {
            format!("{:.1} MB", s / 1024.0 / 1024.0)
        }
    }

    /// Whether Translate… applies (CAD → Part Studio, DXF / DWG → Drawing).
    pub fn translatable(&self) -> bool {
        matches!(self.kind(), FileKind::Cad | FileKind::Drawing2d)
    }
}

/// Seconds since the Unix epoch, now.
pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_tab_labels_and_round_trip() {
        let f = FileTab::new("noshape import test.step", &vec![7u8; 50 * 1024], 1_760_000_000);
        assert_eq!(f.type_label(), "STEP file");
        assert_eq!(f.size_label(), "50 KB");
        assert_eq!(f.kind(), FileKind::Cad);
        assert!(f.translatable());
        assert_eq!(f.bytes().unwrap().len(), 50 * 1024);
        let png = FileTab::new("logo.PNG", b"x", 0);
        assert_eq!((png.type_label().as_str(), png.kind()), ("PNG image", FileKind::Image));
        assert_eq!(FileTab::new("Drawing 1.pdf", &[], 0).type_label(), "PDF document");
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(serde_json::from_str::<FileTab>(&json).unwrap(), f);
        // Through the store's blobs, and ignored by tab_users.
        let mut doc = crate::Document::new();
        let id = doc.add_tab("noshape import test.step", crate::TabContent::File(f.clone()));
        let back = crate::Document::from_blobs(&doc.to_blobs()).unwrap();
        assert!(matches!(&back.tab(id).unwrap().content, crate::TabContent::File(g) if *g == f));
        assert!(doc.tab_users(doc.tabs[0].id).is_empty());
    }
}
