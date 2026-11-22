//! SIGTERM (logout, shutdown, `kill`) quits the app as Quit does (open documents commit their pending edit:
//! src/quit_tests.rs), not by the signal's default action.

#[cfg(unix)]
#[test]
#[ignore = "T2: starts the app's window (needs DISPLAY)"]
fn sigterm_quits_the_app_as_quit_does() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("noshape-quit-signal-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let mut child = Command::new(env!("CARGO_BIN_EXE_noshape"))
        .env("NOSHAPE_CONFIG_DIR", dir.join("config"))
        .env("NOSHAPE_DOCUMENTS_DIR", dir.join("docs"))
        .env("NOSHAPE_NO_NATIVE_DIALOGS", "1")
        .env("RUST_LOG", "info")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start noshape");
    let (tx, rx) = std::sync::mpsc::channel();
    let stderr = child.stderr.take().expect("stderr");
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let wait_for = |what: &str, child: &mut std::process::Child| loop {
        match rx.recv_timeout(std::time::Duration::from_secs(60)) {
            Ok(line) if line.contains(what) => return,
            Ok(_) => {}
            Err(_) => {
                let _ = child.kill();
                panic!("no \"{what}\" from the app");
            }
        }
    };
    wait_for("quit signals:", &mut child);
    let pid = child.id().to_string();
    assert!(Command::new("kill").args(["-TERM", &pid]).status().expect("kill").success());
    wait_for("signal 15: quitting", &mut child);
    let status = child.wait().expect("wait");
    assert_eq!((status.code(), status.signal()), (Some(0), None), "the app ended by SIGTERM's default action, not by quitting");
    let _ = std::fs::remove_dir_all(&dir);
}
