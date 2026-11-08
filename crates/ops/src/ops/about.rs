//! Help › About (and the macOS app menu's About noshape): the version, the copyright, Open CASCADE's notice and the
//! third-party notices the "Open source software components" link opens.

use crate::Ctx;
use crate::about::{COPYRIGHT, NOTICES, OCCT_NOTICE, notices_file};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![help_op(), system_check_op(), Op {
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

fn system_check_op() -> Op {
    Op {
        group: "app",
        verb: "system_check",
        summary: "What Help › System check shows: the system, its memory and the documents folder's disk, and the graphics adapter noshape draws with",
        description: "`os`, `arch`, `cpus`; `memory` (`total`, `available` bytes; available is null where the system doesn't say); `disk` (the documents folder's: `path`, `free`, `total` bytes); `graphics` (`adapter`, \"name (Backend)\", and `software`: true for a software renderer such as llvmpipe or WARP, which draws slowly). `problems` lists what the dialog marks: a software renderer, under 4 GB of memory, under 1 GB free on the disk.",
        fields: vec![],
        mutates: false,
        destructive: false,
        run: system_check,
    }
}

fn system_check(ctx: &mut Ctx, _: &Args) -> Result<OpOutput, OpError> {
    use noshape_scene::system_info as si;
    const GB: u64 = 1 << 30;
    let folder = ctx.documents.clone().unwrap_or_else(noshape_scene::documents_dir);
    let memory = si::memory();
    let disk = si::disk(&folder).or_else(|| folder.parent().and_then(si::disk));
    let adapter = noshape_scene::png::Offscreen::new().map(|o| o.renderer.adapter_name());
    let software = adapter.as_deref().is_ok_and(si::is_software_adapter);
    let mut problems = Vec::new();
    match &adapter {
        Err(e) => problems.push(format!("No graphics adapter: {e}")),
        Ok(_) if software => problems.push("Software renderer: drawing is slow. Install your graphics card's driver.".to_string()),
        Ok(_) => {}
    }
    if memory.is_some_and(|(total, _)| total < 4 * GB) {
        problems.push("Less than 4 GB of memory".into());
    }
    if disk.is_some_and(|(free, _)| free < GB) {
        problems.push("Less than 1 GB free on the documents folder's disk".into());
    }
    Ok(json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
        "memory": memory.map(|(total, available)| json!({ "total": total, "available": available })),
        "disk": disk.map(|(free, total)| json!({ "path": folder.to_string_lossy(), "free": free, "total": total })),
        "graphics": { "adapter": adapter.as_ref().ok(), "software": software },
        "problems": problems,
    })
    .into())
}

fn help_op() -> Op {
    Op {
        group: "app",
        verb: "help",
        summary: "noshape's user guide, as Help › Help shows it (Markdown)",
        description: "`guide`: the guide's Markdown. The FeatureScript standard library is featurestudio_std's, the keyboard shortcuts ui_shortcuts'.",
        fields: vec![],
        mutates: false,
        destructive: false,
        run: |_, _| Ok(json!({ "guide": crate::about::USER_GUIDE }).into()),
    }
}
