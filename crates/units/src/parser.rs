//! Recursive-descent parser.
//!
//! Grammar (lowest to highest precedence):
//!
//! ```text
//! expr     := or ( "?" expr ":" expr )?
//! or       := and ( ("||" | "or") and )*
//! and      := eq ( ("&&" | "and") eq )*
//! eq       := cmp ( ("==" | "!=") cmp )*
//! cmp      := add ( ("<" | "<=" | ">" | ">=") add )*
//! add      := mul ( ("+" | "-") mul )*
//! mul      := unary ( ("*" | "/" | "%") unary )*
//! unary    := ("-" | "+" | "!" | "not") unary | power
//! power    := postfix ( "^" unary )?
//! postfix  := number-literal | "(" expr ")" unit-suffix? | atom
//! ```
//!
//! Number literals absorb units and Onshape-style shorthands: `25 mm`,
//! `25mm`, `4 mm^2`, `1/4 in`, `1 1/2 in`, `1 ft 3 in`, `5' 3"`, `30°`.

use std::ops::Range;

use crate::expr::{BinaryOp, Constant, Expr, Func, UnaryOp};
use crate::lexer::{Spanned, Tok, tokenize};
use crate::{ParseError, ParseErrorKind, Unit};

/// Parses an expression.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Err(ParseError::new(ParseErrorKind::Empty, 0..src.len()));
    }
    let mut p = Parser {
        toks,
        pos: 0,
        src_len: src.len(),
    };
    let e = p.expr()?;
    if let Some((tok, span)) = p.toks.get(p.pos) {
        return Err(ParseError::new(
            ParseErrorKind::UnexpectedToken {
                found: tok.describe(),
                expected: "an operator or end of expression".into(),
            },
            span.clone(),
        ));
    }
    Ok(e)
}

struct Parser {
    toks: Vec<Spanned>,
    pos: usize,
    src_len: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(t, _)| t)
    }

    fn peek_at(&self, offset: usize) -> Option<&Tok> {
        self.toks.get(self.pos + offset).map(|(t, _)| t)
    }

    fn span(&self) -> Range<usize> {
        self.toks
            .get(self.pos)
            .map(|(_, s)| s.clone())
            .unwrap_or(self.src_len..self.src_len)
    }

    fn bump(&mut self) -> Option<Spanned> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn error_expected(&self, expected: &str) -> ParseError {
        match self.toks.get(self.pos) {
            Some((tok, span)) => ParseError::new(
                ParseErrorKind::UnexpectedToken {
                    found: tok.describe(),
                    expected: expected.into(),
                },
                span.clone(),
            ),
            None => ParseError::new(
                ParseErrorKind::UnexpectedEnd {
                    expected: expected.into(),
                },
                self.src_len..self.src_len,
            ),
        }
    }

    fn expect(&mut self, tok: Tok, what: &str) -> Result<Range<usize>, ParseError> {
        if self.peek() == Some(&tok) {
            Ok(self.bump().expect("peeked").1)
        } else {
            Err(self.error_expected(what))
        }
    }

    fn is_keyword(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == kw)
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        let cond = self.or()?;
        if self.peek() == Some(&Tok::Question) {
            self.bump();
            let then = self.expr()?;
            self.expect(Tok::Colon, "`:`")?;
            let otherwise = self.expr()?;
            return Ok(Expr::Ternary(
                Box::new(cond),
                Box::new(then),
                Box::new(otherwise),
            ));
        }
        Ok(cond)
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.and()?;
        while self.peek() == Some(&Tok::OrOr) || self.is_keyword("or") {
            self.bump();
            let r = self.and()?;
            l = Expr::Binary(BinaryOp::Or, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut l = self.eq()?;
        while self.peek() == Some(&Tok::AndAnd) || self.is_keyword("and") {
            self.bump();
            let r = self.eq()?;
            l = Expr::Binary(BinaryOp::And, Box::new(l), Box::new(r));
        }
        Ok(l)
    }

    fn binary_level(
        &mut self,
        next: fn(&mut Self) -> Result<Expr, ParseError>,
        ops: &[(Tok, BinaryOp)],
    ) -> Result<Expr, ParseError> {
        let mut l = next(self)?;
        'outer: loop {
            for (tok, op) in ops {
                if self.peek() == Some(tok) {
                    self.bump();
                    let r = next(self)?;
                    l = Expr::Binary(*op, Box::new(l), Box::new(r));
                    continue 'outer;
                }
            }
            return Ok(l);
        }
    }

    fn eq(&mut self) -> Result<Expr, ParseError> {
        self.binary_level(
            Self::cmp,
            &[(Tok::EqEq, BinaryOp::Eq), (Tok::Ne, BinaryOp::Ne)],
        )
    }

    fn cmp(&mut self) -> Result<Expr, ParseError> {
        self.binary_level(
            Self::add,
            &[
                (Tok::Le, BinaryOp::Le),
                (Tok::Lt, BinaryOp::Lt),
                (Tok::Ge, BinaryOp::Ge),
                (Tok::Gt, BinaryOp::Gt),
            ],
        )
    }

    fn add(&mut self) -> Result<Expr, ParseError> {
        self.binary_level(
            Self::mul,
            &[(Tok::Plus, BinaryOp::Add), (Tok::Minus, BinaryOp::Sub)],
        )
    }

    fn mul(&mut self) -> Result<Expr, ParseError> {
        self.binary_level(
            Self::unary,
            &[
                (Tok::Star, BinaryOp::Mul),
                (Tok::Slash, BinaryOp::Div),
                (Tok::Percent, BinaryOp::Rem),
            ],
        )
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.bump();
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(self.unary()?)))
            }
            Some(Tok::Plus) => {
                self.bump();
                self.unary()
            }
            Some(Tok::Bang) => {
                self.bump();
                Ok(Expr::Unary(UnaryOp::Not, Box::new(self.unary()?)))
            }
            Some(Tok::Ident(s)) if s == "not" => {
                self.bump();
                Ok(Expr::Unary(UnaryOp::Not, Box::new(self.unary()?)))
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let base = self.postfix()?;
        if self.peek() == Some(&Tok::Caret) {
            self.bump();
            let exp = self.unary()?;
            return Ok(Expr::Binary(BinaryOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    /// Unit at token offset `offset`, if any.
    fn unit_at(&self, offset: usize) -> Option<Unit> {
        match self.peek_at(offset)? {
            Tok::Ident(s) => Unit::from_symbol(s),
            Tok::DoubleQuote => Some(Unit::from_symbol("\"").expect("inch")),
            Tok::SingleQuote => Some(Unit::from_symbol("'").expect("foot")),
            Tok::Degree => Some(Unit::from_symbol("°").expect("degree")),
            _ => None,
        }
    }

    /// Consumes a unit (and an optional `^n`, `²`, `³` power) if one follows.
    fn unit_suffix(&mut self) -> Result<Option<(Unit, i8)>, ParseError> {
        let Some(unit) = self.unit_at(0) else {
            return Ok(None);
        };
        // A unit followed by `(` is not a suffix (`2 mm(…)` is an error anyway).
        self.bump();
        let mut power = 1i8;
        match self.peek() {
            Some(Tok::Sup2) => {
                self.bump();
                power = 2;
            }
            Some(Tok::Sup3) => {
                self.bump();
                power = 3;
            }
            Some(Tok::Caret) => {
                let (neg, num_off) = if self.peek_at(1) == Some(&Tok::Minus) {
                    (true, 2)
                } else {
                    (false, 1)
                };
                if let Some(Tok::Number(n)) = self.peek_at(num_off) {
                    let n = *n;
                    if n.fract() == 0.0 && (1.0..=64.0).contains(&n) {
                        for _ in 0..=num_off {
                            self.bump();
                        }
                        power = if neg { -(n as i8) } else { n as i8 };
                    }
                }
            }
            _ => {}
        }
        Ok(Some((unit, power)))
    }

    /// Number literal followed by `/ number unit` (fraction).
    fn starts_fraction(&self, offset: usize) -> bool {
        matches!(self.peek_at(offset), Some(Tok::Number(_)))
            && self.peek_at(offset + 1) == Some(&Tok::Slash)
            && matches!(self.peek_at(offset + 2), Some(Tok::Number(_)))
            && self.unit_at(offset + 3).is_some()
    }

    /// Parses a number literal with its optional unit and shorthands.
    /// The current token must be a number.
    fn number_literal(&mut self) -> Result<Expr, ParseError> {
        let mut inner = self.simple_number_or_fraction();
        // Mixed number `1 1/2 in`.
        if self.starts_fraction(0) {
            let frac = self.simple_number_or_fraction();
            inner = Expr::Binary(BinaryOp::Add, Box::new(inner), Box::new(frac));
        }
        let Some((unit, power)) = self.unit_suffix()? else {
            return Ok(inner);
        };
        let mut lit = Expr::WithUnit {
            expr: Box::new(inner),
            unit,
            power,
        };
        // Mixed units `1 ft 3 in`, `5' 3"`: a following number literal whose
        // unit has the same dimension continues the sum.
        if power == 1 {
            loop {
                let next_unit = if self.starts_fraction(0) {
                    self.unit_at(3)
                } else if matches!(self.peek(), Some(Tok::Number(_))) {
                    self.unit_at(1)
                } else {
                    None
                };
                let Some(next_unit) = next_unit else { break };
                if next_unit.dim() != unit.dim() {
                    break;
                }
                let inner = self.simple_number_or_fraction();
                let (u, p) = self.unit_suffix()?.expect("unit checked");
                let part = Expr::WithUnit {
                    expr: Box::new(inner),
                    unit: u,
                    power: p,
                };
                lit = Expr::Binary(BinaryOp::Add, Box::new(lit), Box::new(part));
                if p != 1 {
                    break;
                }
            }
        }
        Ok(lit)
    }

    fn simple_number_or_fraction(&mut self) -> Expr {
        let n = match self.bump() {
            Some((Tok::Number(n), _)) => n,
            _ => unreachable!("caller checked for a number"),
        };
        if self.starts_fraction_rest() {
            self.bump(); // '/'
            let d = match self.bump() {
                Some((Tok::Number(d), _)) => d,
                _ => unreachable!(),
            };
            return Expr::Binary(
                BinaryOp::Div,
                Box::new(Expr::Number(n)),
                Box::new(Expr::Number(d)),
            );
        }
        Expr::Number(n)
    }

    /// After a number: `/ number unit`.
    fn starts_fraction_rest(&self) -> bool {
        self.peek() == Some(&Tok::Slash)
            && matches!(self.peek_at(1), Some(Tok::Number(_)))
            && self.unit_at(2).is_some()
    }

    fn postfix(&mut self) -> Result<Expr, ParseError> {
        let span = self.span();
        match self.peek().cloned() {
            None => Err(self.error_expected("a value")),
            Some(Tok::Number(_)) => self.number_literal(),
            Some(Tok::LParen) => {
                self.bump();
                let e = self.expr()?;
                self.expect(Tok::RParen, "`)`")?;
                if let Some((unit, power)) = self.unit_suffix()? {
                    return Ok(Expr::WithUnit {
                        expr: Box::new(e),
                        unit,
                        power,
                    });
                }
                Ok(e)
            }
            Some(Tok::Var(name)) => {
                self.bump();
                Ok(Expr::Var(name))
            }
            Some(Tok::Pi) => {
                self.bump();
                Ok(Expr::Constant(Constant::Pi))
            }
            Some(Tok::Ident(name)) => {
                self.bump();
                if self.peek() == Some(&Tok::LParen) {
                    let Some(func) = Func::from_name(&name) else {
                        return Err(ParseError::new(ParseErrorKind::UnknownFunction(name), span));
                    };
                    self.bump();
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.expr()?);
                            if self.peek() == Some(&Tok::Comma) {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                    }
                    let close = self.expect(Tok::RParen, "`,` or `)`")?;
                    let (min, max) = func.arity();
                    if args.len() < min || max.is_some_and(|m| args.len() > m) {
                        return Err(ParseError::new(
                            ParseErrorKind::WrongArgCount {
                                func: func.name().into(),
                                expected: func.arity_text(),
                                found: args.len(),
                            },
                            span.start..close.end,
                        ));
                    }
                    return Ok(Expr::Call(func, args));
                }
                match name.as_str() {
                    "PI" | "pi" => return Ok(Expr::Constant(Constant::Pi)),
                    "true" => return Ok(Expr::Constant(Constant::True)),
                    "false" => return Ok(Expr::Constant(Constant::False)),
                    _ => {}
                }
                if let Some(unit) = Unit::from_symbol(&name) {
                    return Ok(Expr::Unit(unit));
                }
                if Func::from_name(&name).is_some() {
                    return Err(self.error_expected(&format!("`(` after `{name}`")));
                }
                Err(ParseError::new(
                    ParseErrorKind::UnknownIdentifier(name),
                    span,
                ))
            }
            Some(_) => Err(self.error_expected("a value")),
        }
    }
}
