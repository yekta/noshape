//! Live mode (agent API): this window's document is served to the CLI and
//! the MCP server (`noshape_ops::live`). Their calls run here, on the
//! window's own session, so an edit shows at once and lands on the same
//! undo stack as the user's edits.

use crate::doc_model::{DocEvent, DocModel};
use gpui_kit::*;
use noshape_ops::error::OpError;
use noshape_ops::live::Request;
use noshape_ops::registry::{self, OpOutput};
use noshape_ops::{Ctx, OpenDoc};
use serde_json::{Value, json};
use std::path::Path;

/// Serve `doc` while it is open.
pub fn serve(doc: &Entity<DocModel>, cx: &mut App) {
    let path = doc.read(cx).session.path.clone();
    let (tx, rx) = async_channel::unbounded::<Request>();
    let server = match noshape_ops::live::serve(&path, move |r| {
        let _ = tx.send_blocking(r);
    }) {
        Ok(s) => std::sync::Arc::new(parking_lot::Mutex::new(Some(s))),
        Err(e) => {
            log::warn!("live mode: can't serve {}: {e}", path.display());
            return;
        }
    };
    // Closing the document removes its discovery file at once.
    let held = server.clone();
    cx.observe_release(doc, move |_, _| {
        held.lock().take();
    })
    .detach();
    let weak = doc.downgrade();
    cx.spawn(async move |cx| {
        let mut ctx: Option<Ctx> = None;
        while let Ok(req) = rx.recv().await {
            let (tool, args) = (req.tool.clone(), req.args.clone());
            match weak.update(cx, |dm, cx| run(&mut ctx, &path, dm, &tool, &args, cx)) {
                Ok(r) => req.answer(r),
                Err(_) => {
                    req.answer(Err(OpError::new("live", "the app closed the document")));
                    break;
                }
            }
        }
        server.lock().take();
    })
    .detach();
}

/// The context the calls run in: the document adopted with a placeholder
/// session that is swapped with the window's for each call.
fn context(path: &Path) -> Result<Ctx, OpError> {
    let spare = noshape_ops::live::spare_path();
    let session = noshape_ops::session::Session::create(&spare, noshape_model::document::Document::new()).map_err(|e| OpError::new("io", format!("{e:#}")))?;
    let mut ctx = Ctx::new();
    ctx.serving = true;
    ctx.adopt(path, OpenDoc::new(session));
    Ok(ctx)
}

fn run(ctx: &mut Option<Ctx>, path: &Path, dm: &mut DocModel, tool: &str, args: &Value, cx: &mut Context<DocModel>) -> Result<OpOutput, OpError> {
    // The document is already open here.
    let (tool, args) = match tool {
        "document_open" => ("document_info", json!({})),
        "document_create" => return Err(OpError::invalid("the app has this document open")),
        t => (t, args.clone()),
    };
    let op = registry::by_tool_name(tool).ok_or_else(|| OpError::invalid(format!("unknown tool `{tool}`")))?;
    if ctx.is_none() {
        *ctx = Some(context(path)?);
    }
    let ctx = ctx.as_mut().expect("created");
    // An uncommitted edit (an assembly Insert session) commits first, as
    // before any other edit.
    dm.flush_pending(cx);
    let od = ctx.get_mut(path).expect("adopted");
    std::mem::swap(&mut od.session, &mut dm.session);
    let r = registry::run(ctx, op, &args);
    let od = ctx.get_mut(path).expect("adopted");
    std::mem::swap(&mut od.session, &mut dm.session);
    if op.mutates {
        cx.emit(DocEvent::Reloaded);
        cx.emit(DocEvent::HistoryChanged);
        cx.notify();
    }
    r
}
