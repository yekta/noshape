//! Live mode: with a document served (as the app serves its open
//! documents), calls for it run in the server's context, on its session.

use noshape_ops::{Ctx, registry};
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Result<Value, noshape_ops::OpError> {
    registry::run(ctx, registry::by_tool_name(tool).expect("tool"), &args).map(|o| o.value)
}

#[test]
fn calls_for_a_served_document_run_in_the_server() {
    let dir = std::env::temp_dir().join(format!("noshape-live-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("live.noshape");
    let _ = std::fs::remove_file(&path);
    // The "app": its own context, serving the document from a thread.
    let mut app = Ctx::new();
    app.serving = true;
    call(&mut app, "document_create", json!({ "path": path.to_str().unwrap() })).unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<noshape_ops::live::Request>();
    let tx = std::sync::Mutex::new(tx);
    let server = noshape_ops::live::serve(&path, move |r| {
        let _ = tx.lock().unwrap().send(r);
    })
    .unwrap();
    let app_thread = std::thread::spawn(move || {
        let mut n = 0;
        while let Ok(req) = rx.recv() {
            let tool = if req.tool == "document_open" { "document_info".to_string() } else { req.tool.clone() };
            let args = if req.tool == "document_open" { json!({}) } else { req.args.clone() };
            let r = registry::run(&mut app, registry::by_tool_name(&tool).unwrap(), &args);
            req.answer(r);
            n += 1;
            if n == 4 {
                break;
            }
        }
        app
    });
    // The "CLI": opens the path; everything runs in the app.
    let mut cli = Ctx::new();
    let info = call(&mut cli, "document_open", json!({ "path": path.to_str().unwrap() })).unwrap();
    assert_eq!(info["name"], "live", "{info}");
    assert!(cli.open_paths().is_empty(), "the CLI didn't open the file itself");
    let sk = call(&mut cli, "sketch_create", json!({ "plane": "plane:top" })).unwrap();
    assert!(sk["sketch"].is_string(), "{sk}");
    // Errors come back with their parameter.
    let e = call(&mut cli, "sketch_draw", json!({ "sketch": sk["sketch"], "tool": "nope", "points": [[0, 0]] })).unwrap_err();
    assert_eq!(e.param.as_deref(), Some("tool"), "{e:?}");
    let h = call(&mut cli, "history_list", json!({ "limit": 1 })).unwrap();
    assert!(h["history"][0]["description"].as_str().unwrap().contains("Sketch 1"), "{h}");
    let mut app = app_thread.join().unwrap();
    // The edit is in the app's session (its undo stack).
    let undo = call(&mut app, "document_info", json!({})).unwrap();
    assert_eq!(undo["can_undo"], true, "{undo}");
    // Closing the server removes the discovery file: calls go to the file again.
    drop(server);
    assert!(noshape_ops::live::server_for(&path).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
