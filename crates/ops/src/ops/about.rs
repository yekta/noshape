//! Help › About (and the macOS app menu's About noshape): the version, the copyright, Open CASCADE's notice and the
//! third-party notices the "Open source software components" link opens.

use crate::Ctx;
use crate::about::{COPYRIGHT, NOTICES, OCCT_NOTICE, notices_file};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "app",
        verb: "about",
        summary: "What Help › About shows: the version, the copyright and the open source software components' notices",
        description: "The About dialog's text: `version` (the build), `copyright`, `occt` (Open CASCADE Technology's LGPL notice) and `notices_file`, the third-party notices shipped with noshape (OCCT's LGPL 2.1 with its exception, the fonts' and every Rust package's licence) written where the system can open them, as the dialog's \"Open source software components\" link does. `notices` true adds their whole text (`notices_text`).",
        fields: vec![opt("notices", json!({ "type": "boolean" }), "Include the notices' whole text (about 1.2 MB). Default false.")],
        mutates: false,
        destructive: false,
        run: about,
    }]
}

fn about(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let path = notices_file().map_err(|e| OpError::new("io", format!("Could not write the notices file: {e}")))?;
    let mut v = json!({
        "name": "noshape",
        "version": noshape_kernel::VERSION,
        "copyright": COPYRIGHT,
        "occt": OCCT_NOTICE,
        "notices_file": path.to_string_lossy(),
    });
    if a.bool_or("notices", false)? {
        v["notices_text"] = json!(NOTICES);
    }
    Ok(v.into())
}
