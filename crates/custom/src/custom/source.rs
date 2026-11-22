//! Feature Studio source edits the editor's commands make: Format Studio
//! and the Import panel's line.

/// Format Feature Studio: re-indent by brace depth (4 spaces), trim line ends.
pub fn format_source(src: &str) -> String {
    let mut depth: i32 = 0;
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        let t = line.trim();
        let opens = t.chars().filter(|c| matches!(c, '{' | '[' | '(')).count() as i32;
        let closes = t.chars().filter(|c| matches!(c, '}' | ']' | ')')).count() as i32;
        let leading_closes = t.chars().take_while(|c| matches!(c, '}' | ']' | ')')).count() as i32;
        let indent = (depth - leading_closes).max(0) as usize;
        if !t.is_empty() {
            out.push_str(&"    ".repeat(indent));
            out.push_str(t);
        }
        out.push('\n');
        depth = (depth + opens - closes).max(0);
    }
    out
}

/// `src` with `line` after its last `import(...)` line (live: not at the
/// cursor), or after the `FeatureScript` header when it has none.
pub fn append_import(src: &str, line: &str) -> String {
    let lines: Vec<&str> = src.split('\n').collect();
    let is_import = |l: &str| {
        let t = l.trim_start();
        t.starts_with("import(") || t.starts_with("export import(") || (t.contains("::import(") && !t.starts_with("//"))
    };
    let at = lines.iter().rposition(|l| is_import(l)).or_else(|| lines.iter().position(|l| l.trim_start().starts_with("FeatureScript"))).map(|i| i + 1).unwrap_or(0);
    let mut out: Vec<&str> = lines[..at].to_vec();
    out.push(line);
    out.extend_from_slice(&lines[at..]);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_go_after_the_last_import() {
        let src = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n\nexport const x = 1;";
        let out = append_import(src, "import(path : \"a\", version : \"b\");");
        assert_eq!(out, "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\nimport(path : \"a\", version : \"b\");\n\nexport const x = 1;");
        let out = append_import("FeatureScript 3095;\n", "import(path : \"a\", version : \"b\");");
        assert_eq!(out, "FeatureScript 3095;\nimport(path : \"a\", version : \"b\");\n");
        let out = append_import("FeatureScript 3095;\nCsv::import(path : \"c\", version : \"d\");\nx", "import(path : \"a\", version : \"b\");");
        assert_eq!(out, "FeatureScript 3095;\nCsv::import(path : \"c\", version : \"d\");\nimport(path : \"a\", version : \"b\");\nx");
    }
}
