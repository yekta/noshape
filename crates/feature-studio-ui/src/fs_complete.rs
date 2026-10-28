//! Code completion in the Feature Studio editor, as live's Ace list (behaviours "qa (2026-10-27) FS-EDITOR-COMPLETE"):
//! the list opens on every identifier keystroke from the first letter, ranks prefix matches first and fuzzy
//! (subsequence) matches after, and shows std's exported functions, predicates, enums and enum values with their
//! kind on the right. Enter or a click inserts a function's whole call (Onshape's `@autocomplete` / `@eg` values,
//! one `"key" : value` line per required definition field); Tab doesn't accept; Esc only closes the list. After
//! `Enum.` the list holds the enum's values in declaration order; after any other `x.` there is none.

use std::sync::LazyLock;

/// One row of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    /// The right column: "function", "enum", or an enum value's enum.
    pub kind: String,
    /// What Enter inserts: a function's call, else the name.
    pub snippet: String,
    /// The same as an Ace snippet: a function's values as placeholders.
    pub template: String,
    /// The doc tooltip (Markdown): the call, the description, the parameters.
    pub doc: String,
    /// Ace's completer score: functions over predicates over enums (live "opE": the functions' fuzzy matches
    /// before the predicate's, the enums last).
    rank: i32,
}

/// std's completions, in the order a studio's `common.fs` import brings the modules in.
pub fn index() -> &'static [Entry] {
    static INDEX: LazyLock<Vec<Entry>> = LazyLock::new(|| modules().into_iter().flat_map(|m| parse_module(source(m))).collect());
    &INDEX
}

fn source(module: &str) -> &'static str {
    noshape_model::custom::STD_SOURCES.iter().find(|(n, _)| *n == module).map(|(_, s)| *s).unwrap_or("")
}

/// The modules `common.fs` imports, depth first, each once.
fn modules() -> Vec<&'static str> {
    fn visit(m: &'static str, out: &mut Vec<&'static str>) {
        if out.contains(&m) {
            return;
        }
        let Some(name) = noshape_model::custom::STD_SOURCES.iter().map(|(n, _)| *n).find(|n| *n == m) else { return };
        out.push(name);
        let src = source(name);
        let mut rest = src;
        while let Some(i) = rest.find("import(path") {
            rest = &rest[i..];
            let Some(start) = rest.find("\"onshape/std/") else { break };
            let after = &rest[start + 13..];
            let Some(end) = after.find('"') else { break };
            let module = &after[..end];
            if let Some(found) = noshape_model::custom::STD_SOURCES.iter().map(|(n, _)| *n).find(|n| *n == module) {
                visit(found, out);
            }
            rest = &after[end..];
        }
    }
    let mut out = Vec::new();
    visit("common.fs", &mut out);
    out
}

/// A doc comment's parts (Onshape's doc tags).
#[derive(Default, Debug)]
struct Doc {
    internal: bool,
    description: String,
    params: Vec<DocParam>,
    values: Vec<(String, String)>,
}

#[derive(Default, Debug, Clone)]
struct DocParam {
    name: String,
    ty: Option<String>,
    text: String,
    fields: Vec<DocParam>,
}

impl DocParam {
    fn optional(&self) -> bool {
        self.text.contains("@optional") || self.text.contains("@requiredif")
    }
    /// The value a call puts in: `@autocomplete`, else the first `@eg`, else the name.
    fn value(&self) -> String {
        for tag in ["@autocomplete", "@eg"] {
            if let Some(i) = self.text.find(tag) {
                let rest = self.text[i + tag.len()..].trim_start();
                if let Some(code) = rest.strip_prefix('`').and_then(|r| r.split_once('`')).map(|(c, _)| c) {
                    return code.to_string();
                }
            }
        }
        self.name.clone()
    }
    /// The description without the tags.
    fn plain(&self) -> String {
        let mut s = self.text.clone();
        for tag in ["@autocomplete", "@eg"] {
            while let Some(i) = s.find(tag) {
                let rest = &s[i + tag.len()..];
                let end = rest.trim_start().strip_prefix('`').and_then(|r| r.find('`').map(|j| rest.len() - r.len() + j + 1)).unwrap_or(0);
                s.replace_range(i..i + tag.len() + end, "");
            }
        }
        let s = s.replace("@optional", "");
        let s = match s.find("@requiredif") {
            Some(i) => {
                let rest = &s[i..];
                let end = rest.find('}').map_or(rest.len(), |j| j + 1);
                format!("{}{}", &s[..i], &rest[end..])
            }
            None => s,
        };
        s.split_whitespace().collect::<Vec<_>>().join(" ").trim_start_matches(['.', ' ']).to_string()
    }
}

fn parse_doc(lines: &[&str]) -> Doc {
    let mut doc = Doc::default();
    let mut desc: Vec<String> = Vec::new();
    // 0: description, 1: a @param, 2: a definition field, 3: a @value, 4: another tag.
    let mut state = 0;
    for raw in lines {
        let line = raw.trim().trim_start_matches("/**").trim_end_matches("*/").trim();
        let line = line.strip_prefix('*').map(str::trim).unwrap_or(line);
        if line.starts_with("@internal") {
            doc.internal = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("@param ") {
            let (head, text) = rest.split_once(':').unwrap_or((rest, ""));
            let mut parts = head.split_whitespace();
            let name = parts.next().unwrap_or("").to_string();
            let ty = parts.next().map(|t| t.trim_matches(['{', '}']).to_string()).filter(|t| !t.is_empty());
            let map = head.contains("{{");
            doc.params.push(DocParam { name, ty: if map { Some("map".into()) } else { ty }, text: text.trim().to_string(), fields: Vec::new() });
            state = if map { 2 } else { 1 };
            continue;
        }
        if state == 2 {
            if line.starts_with("}}") {
                state = 4;
                continue;
            }
            if let Some(rest) = line.strip_prefix("@field ") {
                let (head, text) = rest.split_once(':').unwrap_or((rest, ""));
                let mut parts = head.split_whitespace();
                let name = parts.next().unwrap_or("").to_string();
                let ty = parts.next().map(|t| t.trim_matches(['{', '}']).to_string()).filter(|t| !t.is_empty());
                if let Some(p) = doc.params.last_mut() {
                    p.fields.push(DocParam { name, ty, text: text.trim().to_string(), fields: Vec::new() });
                }
            } else if let Some(f) = doc.params.last_mut().and_then(|p| p.fields.last_mut()) {
                f.text.push(' ');
                f.text.push_str(line);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("@default ").map(str::trim).unwrap_or(line).strip_prefix("@value ") {
            let (name, text) = rest.split_once(':').unwrap_or((rest, ""));
            doc.values.push((name.trim().to_string(), text.trim().to_string()));
            state = 3;
            continue;
        }
        if line.starts_with('@') {
            state = 4;
            continue;
        }
        match state {
            0 => desc.push(line.to_string()),
            1 => {
                if let Some(p) = doc.params.last_mut() {
                    p.text.push(' ');
                    p.text.push_str(line);
                }
            }
            3 => {
                if let Some(v) = doc.values.last_mut() {
                    v.1.push(' ');
                    v.1.push_str(line);
                }
            }
            _ => {}
        }
    }
    // Paragraphs: blank lines split them, other lines join.
    doc.description = desc.split(|l| l.is_empty()).map(|p| p.join(" ")).filter(|p| !p.trim().is_empty()).collect::<Vec<_>>().join("\n\n");
    doc
}

/// `(name, type)` of each parameter of a parameter list.
fn params_of(list: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in list.chars().chain(std::iter::once(',')) {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let p = cur.trim();
                if !p.is_empty() {
                    let (n, t) = p.split_once(" is ").unwrap_or((p, ""));
                    out.push((n.trim().to_string(), t.trim().to_string()));
                }
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out
}

/// A function's call (live "FS-EDITOR-COMPLETE follow-up": `opExtractWires(context, id + "opExtractWires1", {` /
/// `        "edges" : edges` / `});`, every call ending in `;`); with `stops`, each value an Ace placeholder
/// (`${1:context}`), in order (Enter selects the first, Tab the next).
fn call_snippet(name: &str, params: &[(String, String)], doc: &Doc, stops: bool) -> String {
    let mut n = 0;
    let mut stop = |v: String| {
        if !stops {
            return v;
        }
        n += 1;
        format!("${{{n}:{}}}", v.replace('\\', "\\\\").replace('$', "\\$").replace('}', "\\}"))
    };
    let mut args = Vec::new();
    for (p, ty) in params {
        let d = doc.params.iter().find(|d| &d.name == p);
        if ty == "map"
            && let Some(d) = d.filter(|d| !d.fields.is_empty())
        {
            let mut fields = Vec::new();
            for f in d.fields.iter().filter(|f| !f.optional()) {
                fields.push(format!("        \"{}\" : {}", f.name, stop(f.value())));
            }
            args.push(if fields.is_empty() { "{}".to_string() } else { format!("{{\n{}\n}}", fields.join(",\n")) });
            continue;
        }
        args.push(stop(d.map_or_else(|| p.clone(), DocParam::value)));
    }
    format!("{name}({});", args.join(", "))
}

fn function_doc(snippet: &str, params: &[(String, String)], doc: &Doc) -> String {
    let mut s = format!("```\n{snippet}\n```\n\n{}", doc.description);
    let documented: Vec<&DocParam> = params.iter().filter_map(|(p, _)| doc.params.iter().find(|d| &d.name == p)).filter(|d| !d.plain().is_empty() || !d.fields.is_empty()).collect();
    if !documented.is_empty() {
        s.push_str("\n\nPARAMETER INFO");
        for d in documented {
            let ty = params.iter().find(|(p, _)| p == &d.name).map(|(_, t)| t.clone()).filter(|t| !t.is_empty()).or_else(|| d.ty.clone()).unwrap_or_default();
            s.push_str(&format!("\n\n**{}** is {ty}", d.name));
            if !d.plain().is_empty() {
                s.push_str(&format!("\n\n{}", d.plain()));
            }
            for f in &d.fields {
                s.push_str(&format!("\n- **{}** is {}", f.name, f.ty.clone().unwrap_or_default()));
                if !f.plain().is_empty() {
                    s.push_str(&format!("\n\n  {}", f.plain()));
                }
            }
        }
    }
    s
}

/// The completions a module exports (documented `@internal` ones left out, as live's list).
fn parse_module(src: &str) -> Vec<Entry> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::new();
    let mut doc_start: Option<usize> = None;
    let mut doc_lines: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.starts_with("/**") {
            doc_start = Some(i);
        }
        if let Some(s) = doc_start
            && t.ends_with("*/")
        {
            doc_lines = Some((s, i));
            doc_start = None;
            i += 1;
            continue;
        }
        if doc_start.is_some() {
            i += 1;
            continue;
        }
        let Some(rest) = t.strip_prefix("export ") else {
            // Annotations between a doc comment and its export keep the comment.
            if !t.is_empty() && !t.starts_with("annotation") && !t.starts_with('"') && !t.starts_with('}') && !t.starts_with('{') && !t.starts_with("/*") && !t.starts_with("//") {
                doc_lines = None;
            }
            i += 1;
            continue;
        };
        let doc = doc_lines.take().map(|(a, b)| parse_doc(&lines[a..=b])).unwrap_or_default();
        if doc.internal {
            i += 1;
            continue;
        }
        let ident = |s: &str| s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
        // The parameter list from the first `(` after `at`, over as many lines as it takes.
        let list_from = |at: &str| -> Option<String> {
            let mut text = at.to_string();
            let mut j = i;
            loop {
                if let Some(open) = text.find('(') {
                    let mut depth = 0;
                    for (k, c) in text[open..].char_indices() {
                        match c {
                            '(' => depth += 1,
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    return Some(text[open + 1..open + k].to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
                j += 1;
                text.push(' ');
                text.push_str(lines.get(j)?.trim());
                if j > i + 20 {
                    return None;
                }
            }
        };
        let function = |name: String, list: Option<String>, rank: i32, out: &mut Vec<Entry>| {
            let params = params_of(&list.unwrap_or_default());
            let snippet = call_snippet(&name, &params, &doc, false);
            let template = call_snippet(&name, &params, &doc, true);
            let doc = function_doc(&snippet, &params, &doc);
            // Live: a predicate's column reads "predicate".
            let kind = if rank == 2 { "predicate" } else { "function" };
            out.push(Entry { name, kind: kind.into(), snippet, template, doc, rank });
        };
        if let Some(r) = rest.strip_prefix("function ") {
            function(ident(r), list_from(r), 3, &mut out);
        } else if let Some(r) = rest.strip_prefix("predicate ") {
            function(ident(r), list_from(r), 2, &mut out);
        } else if let Some(r) = rest.strip_prefix("const ") {
            let name = ident(r);
            let value = r.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
            if let Some(f) = value.strip_prefix("function").or_else(|| value.strip_prefix("defineFeature(function")) {
                function(name, list_from(f), 3, &mut out);
            }
        } else if let Some(r) = rest.strip_prefix("enum ") {
            let name = ident(r);
            out.push(Entry { name: name.clone(), kind: "enum".into(), snippet: name.clone(), template: name.clone(), doc: doc.description.clone(), rank: 1 });
            // The values, in declaration order, up to the closing brace.
            let mut j = i + 1;
            while j < lines.len() && !lines[j].trim().starts_with('}') {
                let v = lines[j].trim();
                if !v.starts_with("annotation") && !v.starts_with('{') && !v.starts_with("//") && !v.is_empty() {
                    for part in v.split(',') {
                        let value = ident(part.trim());
                        if !value.is_empty() {
                            let text = doc.values.iter().find(|(n, _)| n == &value).map(|(_, t)| t.clone()).unwrap_or_default();
                            out.push(Entry { name: value.clone(), kind: name.clone(), snippet: value.clone(), template: value.clone(), doc: if text.is_empty() { value.clone() } else { format!("{value}\n\n{text}") }, rank: 0 });
                        }
                    }
                }
                j += 1;
            }
            i = j;
        }
        i += 1;
    }
    out
}

/// A row matched against the typed word. It matches when the word can be cut into pieces that each run on from
/// the start of one of the name's humps, in order (case-insensitive): "opE" in opSplitEdges (op + E) and in
/// lastModifyingOperationId (Ope), not in qLoopEdges or setProperty, whose "ope" starts inside a hump, and "opExt"
/// in neither opMergeContexts nor ToleranceTypeExtended (live "opE" / "opExt"). Its cost is Ace's: the word's
/// position when the name contains it, else each gap of its letters' first matches in order, plus 10 when the
/// first isn't the name's first. `None`: no match. The second value is the matched letters' byte ranges.
fn score(name: &str, needle: &str) -> Option<(i32, Vec<std::ops::Range<usize>>)> {
    if needle.is_empty() {
        return Some((0, Vec::new()));
    }
    let chars: Vec<(usize, char)> = name.char_indices().collect();
    let starts = hump_starts(&chars);
    let lower = |c: char| c.to_lowercase().next().unwrap_or(c);
    let want: Vec<char> = needle.chars().map(lower).collect();
    // From name char `i` (inside a piece) or only at hump starts, does `want[k..]` match?
    fn fits(chars: &[(usize, char)], starts: &[bool], want: &[char], k: usize, i: usize, inside: bool, lower: &dyn Fn(char) -> char) -> bool {
        if k == want.len() {
            return true;
        }
        if inside && i < chars.len() && lower(chars[i].1) == want[k] && fits(chars, starts, want, k + 1, i + 1, true, lower) {
            return true;
        }
        (i..chars.len()).any(|j| starts[j] && lower(chars[j].1) == want[k] && fits(chars, starts, want, k + 1, j + 1, true, lower))
    }
    if !fits(&chars, &starts, &want, 0, 0, false, &lower) {
        return None;
    }
    let name_lower = name.to_lowercase();
    if name_lower.len() == name.len()
        && let Some(at) = name_lower.find(&needle.to_lowercase())
    {
        return Some((at as i32, std::iter::once(at..at + needle.len()).collect()));
    }
    let mut last: isize = -1;
    let mut penalty = 0;
    let mut hits = Vec::new();
    for c in needle.chars() {
        let from = (last + 1) as usize;
        let lo = lower(c);
        let up = c.to_uppercase().next().unwrap_or(c);
        let i1 = chars.iter().skip(from).position(|(_, x)| *x == lo).map(|p| p + from);
        let i2 = chars.iter().skip(from).position(|(_, x)| *x == up).map(|p| p + from);
        let index = match (i1, i2) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => return None,
        } as isize;
        let distance = index - last - 1;
        if distance > 0 {
            if last == -1 {
                penalty += 10;
            }
            penalty += distance as i32;
        }
        let (b, ch) = chars[index as usize];
        hits.push(b..b + ch.len_utf8());
        last = index;
    }
    Some((penalty, hits))
}

/// Where a name's humps start: its first letter, after `_`, a capital after a small letter or digit, and the last
/// capital of a run before a small letter (opCreateBSplineCurve: op Create B Spline Curve).
fn hump_starts(chars: &[(usize, char)]) -> Vec<bool> {
    (0..chars.len())
        .map(|i| {
            let c = chars[i].1;
            let prev = i.checked_sub(1).map(|j| chars[j].1);
            let next = chars.get(i + 1).map(|x| x.1);
            match prev {
                None => true,
                Some('_') => c != '_',
                Some(p) => c.is_uppercase() && (p.is_lowercase() || p.is_ascii_digit() || (p.is_uppercase() && next.is_some_and(char::is_lowercase))),
            }
        })
        .collect()
}

/// A row of the list with the letters to highlight.
pub struct Match<'a> {
    pub entry: &'a Entry,
    pub hits: Vec<std::ops::Range<usize>>,
}

/// The rows for the word `needle`, Ace's order: exact (no-gap) matches first, then by score, std's order within.
pub fn matches<'a>(entries: &'a [Entry], needle: &str) -> Vec<Match<'a>> {
    let mut rows: Vec<(bool, i32, Match<'a>)> = entries
        .iter()
        .filter_map(|e| {
            let (penalty, hits) = score(&e.name, needle)?;
            Some((penalty == 0, e.rank * 1000 - penalty, Match { entry: e, hits }))
        })
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    rows.into_iter().map(|(_, _, m)| m).collect()
}

/// What the caret is completing in `text` at byte `offset`: the word's start and the rows. Inside a word: every
/// matching entry (no list for a word nothing matches, or right after a non-identifier). After `Enum.`: the
/// enum's values only; after any other `x.`: nothing.
pub fn complete_at(text: &str, offset: usize) -> Option<(usize, Vec<Match<'static>>)> {
    let before = &text[..offset.min(text.len())];
    let word_len: usize = before.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').map(char::len_utf8).sum();
    let start = offset - word_len;
    let word = &before[start..];
    if word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    // Not in comments or strings (the line up to the word).
    let line = &before[before[..start].rfind('\n').map_or(0, |i| i + 1)..start];
    if line.contains("//") || line.matches('"').count() % 2 == 1 {
        return None;
    }
    let entries = index();
    if let Some(owner) = before[..start].strip_suffix('.') {
        let owner_len: usize = owner.chars().rev().take_while(|c| c.is_alphanumeric() || *c == '_').map(char::len_utf8).sum();
        let owner = &owner[owner.len() - owner_len..];
        let values: Vec<Match> = matches(entries, word).into_iter().filter(|m| m.entry.kind == owner && m.entry.rank == 0).collect();
        // Declaration order with nothing typed (live `EntityType.`).
        let mut values = values;
        if word.is_empty() {
            values.sort_by_key(|m| entries.iter().position(|e| std::ptr::eq(e, m.entry)));
        }
        return (!values.is_empty()).then_some((start, values));
    }
    if word.is_empty() {
        return None;
    }
    // Enum values only after their enum's dot (live "opE": no OPEN).
    let rows: Vec<Match> = matches(entries, word).into_iter().filter(|m| m.entry.rank > 0).collect();
    (!rows.is_empty()).then_some((start, rows))
}

/// `snippet` as inserted on a line indented by `indent`: each new line starts with it (Ace's snippet insertion).
pub fn indented(snippet: &str, indent: &str) -> String {
    snippet.replace('\n', &format!("\n{indent}"))
}

/// The editor's completion provider: [`complete_at`] on the caret's line.
pub struct Completions;

impl gpui_kit::component::input::CompletionProvider for Completions {
    fn completions(
        &self,
        text: &gpui_kit::component::input::Rope,
        offset: usize,
        _trigger: gpui_kit::component::input::lsp_types::CompletionContext,
        _window: &mut gpui_kit::Window,
        _cx: &mut gpui_kit::App,
    ) -> gpui_kit::Task<anyhow::Result<gpui_kit::component::input::lsp_types::CompletionResponse>> {
        use gpui_kit::component::input::RopeExt as _;
        use gpui_kit::component::input::lsp_types::{CompletionItem, CompletionResponse, CompletionTextEdit, Documentation, InsertTextFormat, LSPAny, MarkupContent, MarkupKind, Range, TextEdit};
        let row = text.offset_to_position(offset).line as usize;
        let line_start = text.line_start_offset(row);
        let line = text.slice(line_start..offset).to_string();
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let items = complete_at(&line, line.len())
            .map(|(start, rows)| {
                let range = Range { start: text.offset_to_position(line_start + start), end: text.offset_to_position(offset) };
                rows.into_iter()
                    .map(|m| CompletionItem {
                        label: m.entry.name.clone(),
                        detail: Some(m.entry.kind.clone()),
                        // A function's call as an Ace snippet (its values selected in turn by Tab).
                        text_edit: Some(CompletionTextEdit::Edit(TextEdit { range, new_text: indented(&m.entry.template, &indent) })),
                        insert_text_format: (m.entry.template != m.entry.snippet).then_some(InsertTextFormat::SNIPPET),
                        documentation: (!m.entry.doc.is_empty()).then(|| Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: m.entry.doc.clone() })),
                        // The letters the list highlights (the menu's `ace` look reads them).
                        data: Some(LSPAny::from(m.hits.iter().map(|r| LSPAny::from(vec![r.start as u64, r.end as u64])).collect::<Vec<_>>())),
                        ..Default::default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        gpui_kit::Task::ready(Ok(CompletionResponse::Array(items)))
    }

    /// Every identifier keystroke (live: the list opens on the first letter), and the dot of `Enum.`.
    fn is_completion_trigger(&self, _offset: usize, new_text: &str, _cx: &mut gpui_kit::App) -> bool {
        !new_text.is_empty() && new_text.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(rows: &[Match]) -> Vec<String> {
        rows.iter().map(|m| m.entry.name.clone()).collect()
    }

    /// Live (A): after "opE" the prefix matches in std's order, then the fuzzy ones: the functions, the predicate,
    /// the enums.
    #[test]
    fn ope_lists_like_live() {
        let (_, rows) = complete_at("opE", 3).unwrap();
        let got = names(&rows);
        let want = ["opEnclose", "opExtractWires", "opExtrude", "opSplineThroughEdges", "opSplitEdges", "lastModifyingOperationId", "surfaceOperationTypePredicate", "SplitOperationKeepType", "NewBodyOperationType"];
        let pos: Vec<usize> = want.iter().map(|w| got.iter().position(|g| g == w).unwrap_or_else(|| panic!("{w} missing: {got:?}"))).collect();
        assert!(pos.windows(2).all(|w| w[0] < w[1]), "{:?}", &got[..20.min(got.len())]);
        assert_eq!(&got[..3], &want[..3]);
        // @internal functions are left out (live "opExt": only opExtractWires and opExtrude).
        assert_eq!(names(&complete_at("opExt", 5).unwrap().1), ["opExtractWires", "opExtrude"]);
    }

    /// Live (A): "op" starts with std's op functions in their declaration order.
    #[test]
    fn op_keeps_std_order() {
        let got = names(&complete_at("op", 2).unwrap().1);
        assert_eq!(&got[..4], ["opMoveCurveBoundary", "opBodyDraft", "opBoolean", "opBoundarySurface"]);
    }

    /// Live (A): Enter inserts the whole call, the id `"<name>1"` and one line per required field.
    #[test]
    fn call_snippets_like_live() {
        let find = |n: &str| index().iter().find(|e| e.name == n).unwrap();
        assert_eq!(indented(&find("opExtractWires").snippet, "    "), "opExtractWires(context, id + \"opExtractWires1\", {\n            \"edges\" : edges\n    });");
        let b = &find("opBoolean").snippet;
        assert!(b.contains("\"tools\" : tools,\n        \"operationType\" : BooleanOperationType.UNION\n}"), "{b}");
        assert!(!b.contains("targets"), "{b}");
        let d = &find("opExtractWires").doc;
        assert!(d.contains("Generates wire bodies from the supplied edges.") && d.contains("PARAMETER INFO") && d.contains("**edges** is Query"), "{d}");
        assert_eq!(find("opExtractWires").kind, "function");
        // Live follow-up (a), (c), (e): the placeholders and the `;`.
        let rows: Vec<&Entry> = index().iter().filter(|e| e.name == "qCreatedBy").collect();
        assert_eq!(rows[0].template, "qCreatedBy(${1:id + \"extrude1\"}, ${2:EntityType.BODY});");
        assert_eq!(rows[1].template, "qCreatedBy(${1:featureId});");
        assert_eq!(find("isQueryEmpty").snippet, "isQueryEmpty(context, query);");
        let p = find("surfaceOperationTypePredicate");
        assert_eq!((p.kind.as_str(), p.snippet.as_str()), ("predicate", "surfaceOperationTypePredicate(definition);"));
    }

    /// Live (C): `EntityType.` lists its values in declaration order with the enum as their kind; `definition.`
    /// lists nothing; one row per overload.
    #[test]
    fn members_and_overloads() {
        let text = "x = EntityType.";
        let (start, rows) = complete_at(text, text.len()).unwrap();
        assert_eq!(start, text.len());
        assert_eq!(names(&rows), ["VERTEX", "EDGE", "FACE", "BODY"]);
        assert_eq!(rows[0].entry.kind, "EntityType");
        assert_eq!(rows[0].entry.doc, "VERTEX\n\nA zero-dimensional point or vertex");
        assert!(complete_at("definition.", 11).is_none());
        let n = index().iter().filter(|e| e.name == "qCreatedBy").count();
        assert!(n >= 2, "{n}");
        assert!(complete_at("// op", 5).is_none());
    }
}
