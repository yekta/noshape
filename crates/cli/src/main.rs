//! `noshape-cli`: every noshape operation as a command, JSON in and out.
//!
//! ```text
//! noshape-cli <group> <verb> [--<field> <value>]... [--json '<object>']
//! noshape-cli call <tool> ['<json object>']     (arguments from stdin when omitted)
//! noshape-cli batch [file]                      (JSON lines {"tool": ..., "args": {...}}; one session)
//! noshape-cli mcp                               (MCP server on stdio)
//! ```
//!
//! Field values are parsed as JSON when they parse (numbers, arrays,
//! objects, booleans) and taken as strings otherwise. Results print as JSON;
//! failures print `{"error": ..., "message": ..., "param": ...}` and exit 2.

// The system heap is slow for our many small allocations on Windows (WINDOWS-RUST-SLOW).
#[cfg(any(windows, all(target_os = "macos", bool_mimalloc)))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    std::process::exit(noshape_cli::run(std::env::args_os().collect()));
}
