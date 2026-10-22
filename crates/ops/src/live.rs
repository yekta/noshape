//! Live mode: when the app has a document open, the CLI and the MCP server
//! run their operations in the app (the edits show at once and land on the
//! app's undo stack) instead of on the file.
//!
//! The app serves each open document on a localhost TCP port and writes a
//! discovery file `<temp>/noshape-live/<hash of the path>.json` with the
//! port, a random token and its process id. A call is one line of JSON
//! (`{"token", "tool", "args"}`) answered by one line
//! (`{"value", "images"}` or `{"error"}`). Plain TCP on 127.0.0.1 works the
//! same on Linux, macOS and Windows.

use crate::error::OpError;
use crate::registry::{Image, OpOutput};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where the discovery files live.
fn dir() -> PathBuf {
    std::env::temp_dir().join("noshape-live")
}

/// FNV-1a of the canonical path: the discovery file's name.
fn key(path: &Path) -> String {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut h: u64 = 0xcbf29ce484222325;
    for b in p.to_string_lossy().as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn info_path(path: &Path) -> PathBuf {
    dir().join(format!("{}.json", key(path)))
}

/// A random token (std only: the hasher's random keys, the time, the pid).
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

// ------------------------------------------------------------ base64

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                s.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| B64.iter().position(|x| *x == c).map(|v| v as u32);
    let bytes: Vec<u8> = s.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for c in bytes.chunks(4) {
        let mut n = 0u32;
        for (k, b) in c.iter().enumerate() {
            n |= val(*b)? << (18 - 6 * k);
        }
        for k in 0..c.len().saturating_sub(1) {
            out.push((n >> (16 - 8 * k) & 255) as u8);
        }
    }
    Some(out)
}

// ------------------------------------------------------------ client

/// An error the app sent back (its `to_json`).
fn error_from_json(e: &Value) -> OpError {
    const CODES: [&str; 11] = ["invalid_argument", "not_found", "regeneration_failed", "read_only", "protected", "io", "internal", "live", "export_failed", "render_failed", "unsupported"];
    let code = e.get("error").and_then(Value::as_str).and_then(|c| CODES.iter().find(|x| **x == c)).copied().unwrap_or("internal");
    let mut err = OpError::new(code, e.get("message").and_then(Value::as_str).unwrap_or(""));
    if let Some(p) = e.get("param").and_then(Value::as_str) {
        err = err.with_param(p);
    }
    if let Some(d) = e.get("detail") {
        err = err.with_detail(d.clone());
    }
    err
}

/// The app serving `path`, if one is running: its port and token.
pub fn server_for(path: &Path) -> Option<(u16, String)> {
    let info: Value = serde_json::from_slice(&std::fs::read(info_path(path)).ok()?).ok()?;
    let port = info.get("port")?.as_u64()? as u16;
    let token = info.get("token")?.as_str()?.to_string();
    // A stale file (the app quit without removing it) doesn't connect.
    TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)).ok()?;
    Some((port, token))
}

/// Run `tool` in the app serving `path`.
pub fn call(port: u16, token: &str, tool: &str, args: &Value) -> Result<OpOutput, OpError> {
    let io = |e: std::io::Error| OpError::new("live", format!("the app's live connection failed: {e}"));
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(io)?;
    let line = serde_json::to_string(&json!({ "token": token, "tool": tool, "args": args })).expect("json");
    s.write_all(line.as_bytes()).and_then(|_| s.write_all(b"\n")).map_err(io)?;
    let mut reply = String::new();
    BufReader::new(s).read_line(&mut reply).map_err(io)?;
    let r: Value = serde_json::from_str(&reply).map_err(|e| OpError::new("live", format!("bad reply from the app: {e}")))?;
    if let Some(e) = r.get("error") {
        return Err(error_from_json(e));
    }
    let images = r.get("images").and_then(Value::as_array).map(|a| a.iter().filter_map(|i| Some(Image { png: b64_decode(i.as_str()?)? })).collect()).unwrap_or_default();
    Ok(OpOutput { value: r.get("value").cloned().unwrap_or(Value::Null), images })
}

// ------------------------------------------------------------ server

/// A document served to the CLI / MCP: dropping it removes the discovery file.
pub struct Server {
    info: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.info);
    }
}

/// One call received: `reply` sends the answer back.
pub struct Request {
    pub tool: String,
    pub args: Value,
    reply: std::sync::mpsc::Sender<Result<OpOutput, OpError>>,
}

impl Request {
    pub fn answer(self, r: Result<OpOutput, OpError>) {
        let _ = self.reply.send(r);
    }
}

/// A file for the serving app's spare session (see `crates/app/src/live_server.rs`).
pub fn spare_path() -> PathBuf {
    dir().join(format!("{}-{}.noshape", std::process::id(), &token()[..12]))
}

/// Remove what apps that are gone left behind (killed, so no destructor
/// ran): discovery files that don't connect, and those apps' spare sessions.
fn clean_stale() {
    let Ok(entries) = std::fs::read_dir(dir()) else { return };
    let files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    let mut alive = vec![std::process::id()];
    for f in files.iter().filter(|f| f.extension().is_some_and(|e| e == "json")) {
        let info: Option<Value> = std::fs::read(f).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let port = info.as_ref().and_then(|i| i.get("port")?.as_u64());
        let up = port.is_some_and(|p| TcpStream::connect_timeout(&([127, 0, 0, 1], p as u16).into(), Duration::from_millis(200)).is_ok());
        match info.as_ref().and_then(|i| i.get("pid")?.as_u64()) {
            Some(pid) if up => alive.push(pid as u32),
            _ => {
                let _ = std::fs::remove_file(f);
            }
        }
    }
    for f in &files {
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let pid = name.split_once('-').and_then(|(p, _)| p.parse::<u32>().ok());
        if name.contains(".noshape") && pid.is_some_and(|p| !alive.contains(&p)) {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// Serve document `path`: each call arrives on `requests` (to run on the
/// app's thread) and is answered with [`Request::answer`].
pub fn serve(path: &Path, requests: impl Fn(Request) + Send + 'static) -> std::io::Result<Server> {
    clean_stale();
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let tok = token();
    std::fs::create_dir_all(dir())?;
    let info = info_path(path);
    std::fs::write(&info, serde_json::to_vec(&json!({ "port": port, "token": tok, "pid": std::process::id(), "path": path.to_string_lossy() })).expect("json"))?;
    std::thread::Builder::new().name("noshape-live".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            let Ok(mut s) = stream.try_clone() else { continue };
            if BufReader::new(stream).read_line(&mut line).is_err() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(req) if req.get("token").and_then(Value::as_str) == Some(tok.as_str()) => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    requests(Request { tool: req.get("tool").and_then(Value::as_str).unwrap_or("").to_string(), args: req.get("args").cloned().unwrap_or(Value::Null), reply: tx });
                    match rx.recv() {
                        Ok(Ok(out)) => json!({ "value": out.value, "images": out.images.iter().map(|i| b64_encode(&i.png)).collect::<Vec<_>>() }),
                        Ok(Err(e)) => json!({ "error": e.to_json() }),
                        Err(_) => json!({ "error": OpError::new("live", "the app closed the document").to_json() }),
                    }
                }
                _ => json!({ "error": OpError::new("live", "not authorized").to_json() }),
            };
            let _ = s.write_all(format!("{reply}\n").as_bytes());
        }
    })?;
    Ok(Server { info })
}

// ------------------------------------------------------------ forwarding

/// Run the call in the app when the document it targets is open there
/// (`None`: run it here, on the file). The app's own context never forwards.
pub(crate) fn forward(ctx: &mut crate::Ctx, op: &crate::registry::Op, args: &Value) -> Option<Result<OpOutput, OpError>> {
    if ctx.serving {
        return None;
    }
    let name = op.tool_name();
    let named = |k: &str| args.get(k).and_then(Value::as_str).map(PathBuf::from);
    let target = match name.as_str() {
        "document_create" => return None,
        "document_open" => named("path")?,
        _ => named("doc").or_else(|| ctx.current_path().map(Path::to_path_buf))?,
    };
    let key = std::fs::canonicalize(&target).unwrap_or(target);
    let (port, token) = match ctx.live_server(&key) {
        Some(s) => s,
        // A document this context opened from its file stays on the file.
        None if ctx.open_paths().contains(&key.as_path()) => return None,
        None => {
            let s = server_for(&key)?;
            ctx.set_live(&key, s.clone());
            s
        }
    };
    if name == "document_close" {
        ctx.unset_live(&key);
        return Some(Ok(json!({ "closed": key.display().to_string(), "live": true }).into()));
    }
    match call(port, &token, &name, args) {
        // The app quit: back to the file.
        Err(e) if e.code == "live" => {
            ctx.unset_live(&key);
            None
        }
        r => Some(r),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for n in 0..10 {
            let d: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&d)).unwrap(), d);
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
    }
}
