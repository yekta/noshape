//! FeatureScript tokens.
//!
//! Lexical rules follow Onshape's language reference ("Lexical conventions"):
//! C and C++ comments, single or double quoted strings with `\b \t \n \f \r`
//! and `\uXXXX` escapes, decimal floating point numbers, identifiers, `@name`
//! runtime builtins and the C-like operator set (no `++` / `--`).

use std::sync::Arc;

/// 1-based line and column (columns count characters, like Ace's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

impl std::fmt::Display for Pos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(Arc<str>),
    /// `@name`: a function provided by the runtime.
    Builtin(Arc<str>),
    Number(f64),
    Str(Arc<str>),
    Kw(Kw),
    Op(Op),
    Eof,
}

macro_rules! keywords {
    ($($v:ident = $s:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Kw { $($v),* }
        impl Kw {
            pub fn lookup(s: &str) -> Option<Kw> {
                match s { $($s => Some(Kw::$v),)* _ => None }
            }
            pub fn as_str(self) -> &'static str {
                match self { $(Kw::$v => $s),* }
            }
        }
    };
}

keywords! {
    Annotation = "annotation", Enum = "enum", Export = "export", Function = "function", Import = "import",
    Operator = "operator", Precondition = "precondition", Predicate = "predicate", Returns = "returns",
    Type = "type", Typecheck = "typecheck", Typeconvert = "typeconvert", As = "as", Is = "is", New = "new",
    Break = "break", Const = "const", Continue = "continue", For = "for", In = "in", Return = "return",
    Var = "var", While = "while", If = "if", Else = "else", False = "false", Inf = "inf", True = "true",
    Undefined = "undefined", Catch = "catch", Throw = "throw", Try = "try", Switch = "switch",
    Assert = "assert", Case = "case", Default = "default", Do = "do",
    // Reserved (live: `const box = ...` is "mismatched input 'box' expecting ID").
    Box = "box",
}

macro_rules! operators {
    ($($v:ident = $s:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Op { $($v),* }
        impl Op {
            pub fn as_str(self) -> &'static str {
                match self { $(Op::$v => $s),* }
            }
        }
        /// Every operator, longest first (for maximal munch).
        const OPS: &[(&str, Op)] = &[$(($s, Op::$v)),*];
    };
}

// Longest first: maximal munch takes the first match.
operators! {
    CoalesceAssign = "??=", AndAssign = "&&=", OrAssign = "||=",
    Scope = "::", Arrow = "->", FatArrow = "=>", SafeDot = "?.", SafeIndex = "?[", Coalesce = "??",
    Eq = "==", Ne = "!=", Le = "<=", Ge = ">=", And = "&&", Or = "||",
    AddAssign = "+=", SubAssign = "-=", MulAssign = "*=", DivAssign = "/=", PowAssign = "^=", ModAssign = "%=",
    CatAssign = "~=",
    Plus = "+", Minus = "-", Star = "*", Slash = "/", Percent = "%", Caret = "^", Tilde = "~",
    Lt = "<", Gt = ">", Assign = "=", Not = "!", Question = "?", Colon = ":", Semi = ";", Comma = ",",
    Dot = ".", LParen = "(", RParen = ")", LBracket = "[", RBracket = "]", LBrace = "{", RBrace = "}",
}

impl Tok {
    /// How ANTLR-style messages quote a token: `'if'`, `'('`, `<EOF>`.
    pub fn describe(&self) -> String {
        match self {
            Tok::Ident(s) => format!("'{s}'"),
            Tok::Builtin(s) => format!("'@{s}'"),
            Tok::Number(_) | Tok::Str(_) => "a literal".to_string(),
            Tok::Kw(k) => format!("'{}'", k.as_str()),
            Tok::Op(o) => format!("'{}'", o.as_str()),
            Tok::Eof => "<EOF>".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
    /// Byte range in the source (for error messages quoting the input).
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LexError {
    pub pos: Pos,
    pub message: String,
}

/// Split `src` into tokens (ending with [`Tok::Eof`]).
pub fn lex(src: &str) -> Result<Vec<Token>, LexError> {
    let bytes = src.as_bytes();
    let mut out = Vec::with_capacity(src.len() / 4);
    let mut i = 0usize;
    let mut line = 1u32;
    let mut line_start = 0usize;
    // Column of byte offset `at` (characters since the line start, 1-based).
    let col_of = |line_start: usize, at: usize| -> u32 {
        let s = &src[line_start..at];
        if s.is_ascii() { s.len() as u32 + 1 } else { s.chars().count() as u32 + 1 }
    };
    while i < bytes.len() {
        let c = bytes[i];
        // Whitespace.
        if c == b'\n' {
            i += 1;
            line += 1;
            line_start = i;
            continue;
        }
        if c == b' ' || c == b'\t' || c == b'\r' || c == 0x0c {
            i += 1;
            continue;
        }
        let pos = Pos { line, col: col_of(line_start, i) };
        // Comments.
        if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            loop {
                if i >= bytes.len() {
                    return Err(LexError { pos, message: "unterminated comment".into() });
                }
                if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    i += 2;
                    break;
                }
                if bytes[i] == b'\n' {
                    line += 1;
                    line_start = i + 1;
                }
                i += 1;
            }
            continue;
        }
        let start = i;
        // Identifiers, keywords, builtins.
        if c.is_ascii_alphabetic() || c == b'_' || (c == b'@' && bytes.get(i + 1).is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')) {
            let builtin = c == b'@';
            if builtin {
                i += 1;
            }
            let s0 = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &src[s0..i];
            let tok = if builtin {
                Tok::Builtin(word.into())
            } else if let Some(k) = Kw::lookup(word) {
                Tok::Kw(k)
            } else {
                Tok::Ident(word.into())
            };
            out.push(Token { tok, pos, start, end: i });
            continue;
        }
        // Numbers: 12, 1.5, .5, 3e9, 1.44E-3.
        if c.is_ascii_digit() || (c == b'.' && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit())) {
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'.' && bytes.get(i + 1).is_none_or(|b| b.is_ascii_digit() || !(b.is_ascii_alphabetic() || *b == b'_' || *b == b'.')) {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                let mut j = i + 1;
                if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j].is_ascii_digit() {
                    i = j;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let text = &src[start..i];
            let v: f64 = text.parse().map_err(|_| LexError { pos, message: format!("token recognition error at: '{text}'") })?;
            out.push(Token { tok: Tok::Number(v), pos, start, end: i });
            continue;
        }
        // Strings.
        if c == b'"' || c == b'\'' {
            let quote = c;
            i += 1;
            let mut s = String::new();
            loop {
                if i >= bytes.len() || bytes[i] == b'\n' {
                    return Err(LexError { pos, message: format!("token recognition error at: '{}'", &src[start..i.min(bytes.len())]) });
                }
                let b = bytes[i];
                if b == quote {
                    i += 1;
                    break;
                }
                if b == b'\\' {
                    let Some(&e) = bytes.get(i + 1) else { continue };
                    i += 2;
                    match e {
                        b'b' => s.push('\u{8}'),
                        b't' => s.push('\t'),
                        b'n' => s.push('\n'),
                        b'f' => s.push('\u{c}'),
                        b'r' => s.push('\r'),
                        b'u' => {
                            let hex = src.get(i..i + 4).unwrap_or("");
                            match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                                Some(ch) if hex.len() == 4 => {
                                    s.push(ch);
                                    i += 4;
                                }
                                _ => return Err(LexError { pos, message: "invalid \\u escape in string".into() }),
                            }
                        }
                        _ => {
                            // `\\`, `\'`, `\"` and anything else: the character itself.
                            let ch = src[i - 1..].chars().next().unwrap_or('\\');
                            s.push(ch);
                            i += ch.len_utf8() - 1;
                        }
                    }
                    continue;
                }
                let ch = src[i..].chars().next().unwrap_or('?');
                s.push(ch);
                i += ch.len_utf8();
            }
            out.push(Token { tok: Tok::Str(s.into()), pos, start, end: i });
            continue;
        }
        // Operators.
        let rest = &src[i..];
        if let Some((s, op)) = OPS.iter().find(|(s, _)| rest.starts_with(s)) {
            i += s.len();
            out.push(Token { tok: Tok::Op(*op), pos, start, end: i });
            continue;
        }
        let ch = rest.chars().next().unwrap_or('?');
        return Err(LexError { pos, message: format!("token recognition error at: '{ch}'") });
    }
    let pos = Pos { line, col: col_of(line_start, bytes.len()) };
    out.push(Token { tok: Tok::Eof, pos, start: bytes.len(), end: bytes.len() });
    Ok(out)
}



/// Byte ranges of the references to the symbol `name` in `src`: identifier
/// tokens (not text in strings or comments), excluding field names after
/// `.` / `?.` and map-literal keys (`{ name : ... }`). Empty if `src` does
/// not lex.
pub fn symbol_occurrences(src: &str, name: &str) -> Vec<std::ops::Range<usize>> {
    let Ok(toks) = lex(src) else { return Vec::new() };
    let mut out = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let Tok::Ident(s) = &t.tok else { continue };
        if &**s != name {
            continue;
        }
        let prev = i.checked_sub(1).map(|p| &toks[p].tok);
        if matches!(prev, Some(Tok::Op(Op::Dot | Op::SafeDot))) {
            continue;
        }
        let key = matches!(toks.get(i + 1).map(|t| &t.tok), Some(Tok::Op(Op::Colon))) && matches!(prev, Some(Tok::Op(Op::LBrace | Op::Comma | Op::LParen)));
        if !key {
            out.push(t.start..t.end);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn basics() {
        let t = toks("FeatureScript 3095; x ??= @size(a) ~ 'q\\'s' // c\n /* d */ 1.5e3 .5 inf");
        assert_eq!(t[0], Tok::Ident("FeatureScript".into()));
        assert_eq!(t[1], Tok::Number(3095.0));
        assert_eq!(t[4], Tok::Op(Op::CoalesceAssign));
        assert_eq!(t[5], Tok::Builtin("size".into()));
        assert_eq!(t[10], Tok::Str("q's".into()));
        assert_eq!(t[11], Tok::Number(1500.0));
        assert_eq!(t[12], Tok::Number(0.5));
        assert_eq!(t[13], Tok::Kw(Kw::Inf));
        assert_eq!(t[14], Tok::Eof);
    }

    #[test]
    fn positions_and_escapes() {
        let t = lex("a\n  \"\\u263A\\n\" b").unwrap();
        assert_eq!(t[1].pos, Pos { line: 2, col: 3 });
        assert_eq!(t[1].tok, Tok::Str("\u{263A}\n".into()));
        assert_eq!(t[2].pos, Pos { line: 2, col: 14 });
    }

    #[test]
    fn member_after_number_like() {
        // `a.b` and `x[0].y` keep the dot as an operator.
        assert_eq!(toks("1..")[0], Tok::Number(1.0));
        assert_eq!(toks("a.b")[1], Tok::Op(Op::Dot));
    }
}
