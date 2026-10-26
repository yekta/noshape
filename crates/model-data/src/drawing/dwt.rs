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
use super::{Drawing, Sheet, SheetFormat, SheetSize, TitleBlock, TitleBlockStyle};
use serde::{Deserialize, Serialize};

/// The custom document property holding the formats.
pub const PROPERTY: &str = "noshape.sheet_formats";

/// A sheet's format: what a template defines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SheetTemplate {
    pub size: SheetSize,
    pub format: SheetFormat,
    pub title_block: TitleBlock,
    pub title_style: TitleBlockStyle,
    #[serde(default)]
    pub title_language: super::tb_lang::TitleLanguage,
}

impl SheetTemplate {
    pub fn of(s: &Sheet) -> Self {
        Self { size: s.size, format: s.format, title_block: s.title_block.clone(), title_style: s.title_style, title_language: s.title_language }
    }

    fn apply(&self, s: &mut Sheet) {
        s.size = self.size;
        s.format = self.format;
        s.title_block = self.title_block.clone();
        s.title_style = self.title_style;
        s.title_language = self.title_language;
    }
}

impl Drawing {
    /// Sheet properties › Select a DWT on sheet `si` (named `name`).
    pub fn apply_dwt(&mut self, si: usize, name: &str, templates: &[SheetTemplate]) {
        let Some(primary) = templates.first() else { return };
        if let Some(s) = self.sheets.get_mut(si) {
            primary.apply(s);
            s.dwt = Some(name.to_string());
        }
        self.continuation = Some(Box::new(templates.get(1).unwrap_or(primary).clone()));
    }

    /// Sheet properties › Format › Custom: the sheet keeps its format but is
    /// no longer the template's (Size and the zone rows come back).
    pub fn detach_dwt(&mut self, si: usize) {
        if let Some(s) = self.sheets.get_mut(si) {
            s.dwt = None;
        }
    }

    /// A new sheet after `after`: a picked template's continuation format.
    pub fn new_sheet_template(&self) -> Option<&SheetTemplate> {
        self.continuation.as_deref()
    }
}

impl Sheet {
    /// Apply a template's format (new sheets: the continuation).
    pub fn apply_template_format(&mut self, t: &SheetTemplate) {
        t.apply(self);
    }
}
