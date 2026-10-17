//! Tokenizer for the expression language.

use std::ops::Range;

use crate::{ParseError, ParseErrorKind};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tok {
    Number(f64),
    Ident(String),
    Var(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
    Question,
    Colon,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Bang,
    /// `"` (inch)
    DoubleQuote,
    /// `'` (foot)
    SingleQuote,
    /// `°`
    Degree,
    /// `²`
    Sup2,
    /// `³`
    Sup3,
    /// `π`
    Pi,
}

impl Tok {
    pub(crate) fn describe(&self) -> String {
        match self {
            Tok::Number(n) => n.to_string(),
            Tok::Ident(s) => s.clone(),
            Tok::Var(s) => format!("#{s}"),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Star => "*".into(),
            Tok::Slash => "/".into(),
            Tok::Percent => "%".into(),
            Tok::Caret => "^".into(),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::Comma => ",".into(),
            Tok::Question => "?".into(),
            Tok::Colon => ":".into(),
            Tok::Lt => "<".into(),
            Tok::Le => "<=".into(),
            Tok::Gt => ">".into(),
            Tok::Ge => ">=".into(),
            Tok::EqEq => "==".into(),
            Tok::Ne => "!=".into(),
            Tok::AndAnd => "&&".into(),
            Tok::OrOr => "||".into(),
            Tok::Bang => "!".into(),
            Tok::DoubleQuote => "\"".into(),
            Tok::SingleQuote => "'".into(),
            Tok::Degree => "°".into(),
            Tok::Sup2 => "²".into(),
            Tok::Sup3 => "³".into(),
            Tok::Pi => "π".into(),
        }
    }
}

pub(crate) type Spanned = (Tok, Range<usize>);

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    (c.is_alphanumeric() || c == '_') && !matches!(c, '²' | '³' | 'π')
}

pub(crate) fn tokenize(src: &str) -> Result<Vec<Spanned>, ParseError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < src.len() {
        let c = src[i..].chars().next().expect("in bounds");
        let start = i;
        let clen = c.len_utf8();
        if c.is_whitespace() {
            i += clen;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'.' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
            }
            if j < bytes.len() && (bytes[j] == b'e' || bytes[j] == b'E') {
                let mut k = j + 1;
                if k < bytes.len() && (bytes[k] == b'+' || bytes[k] == b'-') {
                    k += 1;
                }
                if k < bytes.len() && bytes[k].is_ascii_digit() {
                    while k < bytes.len() && bytes[k].is_ascii_digit() {
                        k += 1;
                    }
                    j = k;
                }
            }
            let text = &src[i..j];
            let v: f64 = text.parse().map_err(|_| {
                ParseError::new(ParseErrorKind::InvalidNumber(text.to_string()), i..j)
            })?;
            if !v.is_finite() {
                return Err(ParseError::new(
                    ParseErrorKind::InvalidNumber(text.to_string()),
                    i..j,
                ));
            }
            out.push((Tok::Number(v), i..j));
            i = j;
            continue;
        }
        if c == '#' {
            let mut j = i + 1;
            let first = src[j..].chars().next();
            if !first.is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
                return Err(ParseError::new(ParseErrorKind::InvalidVariable, i..i + 1));
            }
            while let Some(c) = src[j..].chars().next() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    j += 1;
                } else {
                    break;
                }
            }
            out.push((Tok::Var(src[i + 1..j].to_string()), i..j));
            i = j;
            continue;
        }
        if is_ident_start(c) && c != 'π' {
            let mut j = i;
            while let Some(c) = src[j..].chars().next() {
                if is_ident_continue(c) {
                    j += c.len_utf8();
                } else {
                    break;
                }
            }
            out.push((Tok::Ident(src[i..j].to_string()), i..j));
            i = j;
            continue;
        }
        let next = bytes.get(i + 1).copied();
        let (tok, len) = match c {
            '+' => (Tok::Plus, 1),
            '-' | '−' => (Tok::Minus, clen),
            '*' | '×' | '·' => (Tok::Star, clen),
            '/' | '÷' => (Tok::Slash, clen),
            '%' => (Tok::Percent, 1),
            '^' => (Tok::Caret, 1),
            '(' => (Tok::LParen, 1),
            ')' => (Tok::RParen, 1),
            ',' => (Tok::Comma, 1),
            '?' => (Tok::Question, 1),
            ':' => (Tok::Colon, 1),
            '<' if next == Some(b'=') => (Tok::Le, 2),
            '<' => (Tok::Lt, 1),
            '>' if next == Some(b'=') => (Tok::Ge, 2),
            '>' => (Tok::Gt, 1),
            '=' if next == Some(b'=') => (Tok::EqEq, 2),
            '!' if next == Some(b'=') => (Tok::Ne, 2),
            '!' => (Tok::Bang, 1),
            '&' if next == Some(b'&') => (Tok::AndAnd, 2),
            '|' if next == Some(b'|') => (Tok::OrOr, 2),
            '"' | '″' => (Tok::DoubleQuote, clen),
            '\'' | '′' => (Tok::SingleQuote, clen),
            '°' => (Tok::Degree, clen),
            '²' => (Tok::Sup2, clen),
            '³' => (Tok::Sup3, clen),
            'π' => (Tok::Pi, clen),
            _ => {
                return Err(ParseError::new(
                    ParseErrorKind::UnexpectedChar(c),
                    start..start + clen,
                ));
            }
        };
        out.push((tok, start..start + len));
        i += len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        tokenize(s).unwrap().into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(toks("1e-3"), vec![Tok::Number(1e-3)]);
        assert_eq!(toks(".5"), vec![Tok::Number(0.5)]);
        assert_eq!(toks("2."), vec![Tok::Number(2.0)]);
        assert_eq!(toks("1E+2"), vec![Tok::Number(100.0)]);
        assert_eq!(
            toks("25mm"),
            vec![Tok::Number(25.0), Tok::Ident("mm".into())]
        );
        assert_eq!(toks("2em"), vec![Tok::Number(2.0), Tok::Ident("em".into())]);
        assert_eq!(
            toks("1e3mm"),
            vec![Tok::Number(1000.0), Tok::Ident("mm".into())]
        );
    }

    #[test]
    fn operators_and_spans() {
        let t = tokenize("#a<=b && !c").unwrap();
        assert_eq!(t[0], (Tok::Var("a".into()), 0..2));
        assert_eq!(t[1], (Tok::Le, 2..4));
        assert_eq!(t[3], (Tok::AndAnd, 6..8));
        assert_eq!(t[4], (Tok::Bang, 9..10));
        assert_eq!(
            toks("5' 3\""),
            vec![
                Tok::Number(5.0),
                Tok::SingleQuote,
                Tok::Number(3.0),
                Tok::DoubleQuote
            ]
        );
        assert_eq!(toks("30°"), vec![Tok::Number(30.0), Tok::Degree]);
        assert_eq!(toks("2µm"), vec![Tok::Number(2.0), Tok::Ident("µm".into())]);
        assert_eq!(toks("2π"), vec![Tok::Number(2.0), Tok::Pi]);
    }

    #[test]
    fn errors() {
        let e = tokenize("1 $ 2").unwrap_err();
        assert_eq!(e.kind, ParseErrorKind::UnexpectedChar('$'));
        assert_eq!(e.span, 2..3);
        let e = tokenize("# x").unwrap_err();
        assert_eq!(e.kind, ParseErrorKind::InvalidVariable);
        assert!(tokenize("1 = 2").is_err());
        assert!(tokenize("a & b").is_err());
    }
}
