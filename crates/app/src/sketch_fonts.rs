//! The Text tool's fonts in the UI: registered with the text system on first
//! use, so the Text dialog's preview and edit box draw in the picked font
//! (live sketch/132: "Default text" in Open Sans).

use gpui_kit::*;
use std::collections::HashMap;
use std::sync::Mutex;

static FAMILIES: Mutex<Option<HashMap<String, Option<SharedString>>>> = Mutex::new(None);

/// The UI family name of Text tool font `font`, registering its files once
/// (None: not a Text tool font, or the files could not be read).
pub fn ui_family(font: &str, cx: &App) -> Option<SharedString> {
    let mut lock = FAMILIES.lock().unwrap_or_else(|e| e.into_inner());
    let map = lock.get_or_insert_with(HashMap::new);
    if let Some(f) = map.get(font) {
        return f.clone();
    }
    let files = noshape_sketch::text::sketch_font_files(font);
    let family = files
        .first()
        .and_then(|f| noshape_sketch::text::font_family_name(f))
        .filter(|_| cx.text_system().add_fonts(files.iter().map(|f| std::borrow::Cow::Borrowed(*f)).collect()).is_ok())
        .map(SharedString::from);
    map.insert(font.to_string(), family.clone());
    family
}
