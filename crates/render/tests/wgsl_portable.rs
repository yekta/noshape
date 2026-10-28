//! WGSL that Vulkan and Metal accept but D3D12's DXC rejects. Only the Windows CI runner compiles shaders through DXC,
//! so check the patterns here, on every platform.

use std::fs;

/// DXC: "control may reach end of non-void function" when a function's last statement is `discard` (bf131a31's
/// hover outline failed on Windows only, 10-28). End in a return: `if (!hit) { discard; } return color;`.
#[test]
fn no_shader_function_ends_in_discard() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut bad = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "wgsl") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let mut last = "";
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap().trim();
            if code == "}" && !line.starts_with(char::is_whitespace) && last == "discard;" {
                bad.push(format!("{}:{}", path.file_name().unwrap().to_string_lossy(), i + 1));
            }
            if !code.is_empty() {
                last = code;
            }
        }
    }
    assert!(bad.is_empty(), "functions ending in `discard` (DXC rejects them; end in a return): {bad:?}");
}
