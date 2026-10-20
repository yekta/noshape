//! FeatureScript parser (recursive descent with precedence climbing).
//!
//! Grammar, from Onshape's language reference and the standard library:
//!
//! ```text
//! module    := ('FeatureScript' NUMBER ';')? (import | ';')* (topLevel | ';')* EOF
//! import    := (ID '::')? 'export'? 'import' '(' 'path' ':' STRING ',' 'version' ':' STRING ')' ';'
//! topLevel  := annotation? 'export'? (function | predicate | operator | const | enum | type)
//! ```
//!
//! Errors use the wording of Onshape's (ANTLR) parser: "extraneous input",
//! "mismatched input", "missing ... at ...", "no viable alternative".

use super::ast::*;
use super::lexer::{Kw, LexError, Op, Pos, Tok, Token, lex};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub pos: Pos,
    pub message: String,
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError { pos: e.pos, message: e.message }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.pos, self.message)
    }
}

type PResult<T> = Result<T, ParseError>;

/// What may start a top-level construct after the imports (Onshape's
/// "expecting {...}" list).
const TOP_EXPECT: &str = "{<EOF>, 'annotation', 'export', 'enum', 'type', 'function', 'predicate', 'operator', 'typeconvert', 'const', TOP_SEMI}";
const TOP_EXPECT_IMPORTS: &str = "{<EOF>, 'annotation', 'export', 'enum', 'type', 'function', 'predicate', 'operator', 'typeconvert', 'const', 'import', ID, TOP_SEMI}";

pub fn parse(src: &str) -> PResult<Module> {
    let toks = lex(src)?;
    let mut p = Parser { toks, i: 0, src };
    p.module()
}

/// Parse a single expression (used for tests and tooling).
pub fn parse_expr(src: &str) -> PResult<Expr> {
    let toks = lex(src)?;
    let mut p = Parser { toks, i: 0, src };
    let e = p.expr()?;
    p.expect_eof()?;
    Ok(e)
}

struct Parser<'a> {
    toks: Vec<Token>,
    i: usize,
    src: &'a str,
}

impl<'a> Parser<'a> {
    // ------------------------------------------------------------ helpers

    fn peek(&self) -> &Tok {
        &self.toks[self.i].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.i + n).min(self.toks.len() - 1)].tok
    }
    fn pos(&self) -> Pos {
        self.toks[self.i].pos
    }
    fn bump(&mut self) -> Token {
        let t = self.toks[self.i].clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }
    fn is_op(&self, op: Op) -> bool {
        matches!(self.peek(), Tok::Op(o) if *o == op)
    }
    fn is_kw(&self, kw: Kw) -> bool {
        matches!(self.peek(), Tok::Kw(k) if *k == kw)
    }
    fn eat_op(&mut self, op: Op) -> bool {
        if self.is_op(op) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, kw: Kw) -> bool {
        if self.is_kw(kw) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn is_ident(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Ident(x) if &**x == s)
    }

    fn err<T>(&self, message: impl Into<String>) -> PResult<T> {
        Err(ParseError { pos: self.pos(), message: message.into() })
    }

    /// Error for an unexpected token where `expected` (already quoted) was
    /// required, worded like ANTLR's single-token recovery.
    fn unexpected<T>(&self, expected: &str, expected_tok: Option<&Tok>) -> PResult<T> {
        let cur = self.peek().describe();
        if let Some(e) = expected_tok {
            if self.peek_at(1) == e {
                return self.err(format!("extraneous input {cur} expecting {expected}"));
            }
            // A missing single token: the current one could follow it.
            if matches!(e, Tok::Op(Op::Semi | Op::RParen | Op::RBracket | Op::RBrace | Op::Comma | Op::Colon)) {
                return self.err(format!("missing {expected} at {cur}"));
            }
        }
        self.err(format!("mismatched input {cur} expecting {expected}"))
    }

    fn expect_op(&mut self, op: Op) -> PResult<Token> {
        if self.is_op(op) {
            return Ok(self.bump());
        }
        self.unexpected(&format!("'{}'", op.as_str()), Some(&Tok::Op(op)))
    }
    fn expect_kw(&mut self, kw: Kw) -> PResult<Token> {
        if self.is_kw(kw) {
            return Ok(self.bump());
        }
        self.unexpected(&format!("'{}'", kw.as_str()), Some(&Tok::Kw(kw)))
    }
    fn expect_ident(&mut self) -> PResult<Name> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.bump();
                Ok(s)
            }
            _ => self.unexpected("ID", None),
        }
    }
    fn expect_str(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.bump();
                Ok(s.to_string())
            }
            _ => self.unexpected("STRING", None),
        }
    }
    /// The `;` ending a top-level declaration (Onshape's grammar calls it
    /// TOP_SEMI in its messages).
    fn expect_top_semi(&mut self) -> PResult<()> {
        if self.eat_op(Op::Semi) {
            return Ok(());
        }
        self.unexpected("TOP_SEMI", Some(&Tok::Op(Op::Semi)))
    }

    /// Source text from token `from` through the current token.
    fn text_through(&self, from: usize) -> &'a str {
        let a = self.toks[from].start;
        let b = self.toks[self.i].end.max(a);
        &self.src[a..b]
    }

    fn expect_eof(&self) -> PResult<()> {
        if matches!(self.peek(), Tok::Eof) { Ok(()) } else { self.err(format!("extraneous input {} expecting <EOF>", self.peek().describe())) }
    }

    // ------------------------------------------------------------ module

    fn module(&mut self) -> PResult<Module> {
        let mut version = None;
        if self.is_ident("FeatureScript") {
            self.bump();
            match self.peek().clone() {
                Tok::Number(n) => {
                    self.bump();
                    version = Some(n as u32);
                }
                _ => return self.unexpected("NUMBER", None),
            }
            self.expect_op(Op::Semi)?;
        }
        let mut items = Vec::new();
        let mut imports_done = false;
        loop {
            if self.eat_op(Op::Semi) {
                continue;
            }
            if matches!(self.peek(), Tok::Eof) {
                break;
            }
            let pos = self.pos();
            // Imports: `import(...)`, `export import(...)`, `NS::import(...)`.
            let ns_import = matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Op(Op::Scope)) && (matches!(self.peek_at(2), Tok::Kw(Kw::Import)) || (matches!(self.peek_at(2), Tok::Kw(Kw::Export)) && matches!(self.peek_at(3), Tok::Kw(Kw::Import))));
            let export_import = self.is_kw(Kw::Export) && matches!(self.peek_at(1), Tok::Kw(Kw::Import));
            if ns_import || export_import || self.is_kw(Kw::Import) {
                if imports_done {
                    return self.err(format!("extraneous input {} expecting {TOP_EXPECT}", self.peek().describe()));
                }
                let namespace = if ns_import {
                    let n = self.expect_ident()?;
                    self.bump();
                    Some(n)
                } else {
                    None
                };
                let export = self.eat_kw(Kw::Export);
                self.expect_kw(Kw::Import)?;
                self.expect_op(Op::LParen)?;
                if !self.is_ident("path") {
                    return self.unexpected("'path'", None);
                }
                self.bump();
                self.expect_op(Op::Colon)?;
                let path = self.expect_str()?;
                self.expect_op(Op::Comma)?;
                if !self.is_ident("version") {
                    return self.unexpected("'version'", None);
                }
                self.bump();
                self.expect_op(Op::Colon)?;
                let version = self.expect_str()?;
                self.expect_op(Op::RParen)?;
                self.expect_top_semi()?;
                items.push(TopLevel { pos, name_pos: pos, export, annotation: None, kind: TopKind::Import { namespace, path, version } });
                continue;
            }
            let expect = if imports_done { TOP_EXPECT } else { TOP_EXPECT_IMPORTS };
            imports_done = true;
            let annotation = if self.is_kw(Kw::Annotation) { Some(self.annotation()?) } else { None };
            let pos = if annotation.is_some() { self.pos() } else { pos };
            let export = self.eat_kw(Kw::Export);
            let name_pos = self.toks[(self.i + 1).min(self.toks.len() - 1)].pos;
            let kind = match self.peek() {
                Tok::Kw(Kw::Function) => {
                    self.bump();
                    TopKind::Function(self.fn_def(FnKind::Function, pos)?)
                }
                Tok::Kw(Kw::Predicate) => {
                    self.bump();
                    TopKind::Function(self.fn_def(FnKind::Predicate, pos)?)
                }
                Tok::Kw(Kw::Operator) => {
                    self.bump();
                    let op = match self.peek() {
                        Tok::Op(Op::Plus) => FnKind::Operator(BinOp::Add),
                        Tok::Op(Op::Minus) => FnKind::Operator(BinOp::Sub),
                        Tok::Op(Op::Star) => FnKind::Operator(BinOp::Mul),
                        Tok::Op(Op::Slash) => FnKind::Operator(BinOp::Div),
                        Tok::Op(Op::Percent) => FnKind::Operator(BinOp::Mod),
                        Tok::Op(Op::Caret) => FnKind::Operator(BinOp::Pow),
                        Tok::Op(Op::Lt) => FnKind::Operator(BinOp::Lt),
                        _ => return self.unexpected("{'+', '-', '*', '/', '%', '^', '<'}", None),
                    };
                    self.bump();
                    let mut f = self.fn_def_after_name(op, "".into(), pos)?;
                    if op == FnKind::Operator(BinOp::Sub) && f.params.len() == 1 {
                        Arc::get_mut(&mut f).expect("fresh").kind = FnKind::Negate;
                    }
                    TopKind::Function(f)
                }
                Tok::Kw(Kw::Typeconvert) => {
                    self.bump();
                    TopKind::TypeConvert(self.fn_def(FnKind::Function, pos)?)
                }
                Tok::Kw(Kw::Const) => {
                    self.bump();
                    let name = self.expect_ident()?;
                    let ty = if self.eat_kw(Kw::Is) { Some(self.type_name()?) } else { None };
                    self.expect_op(Op::Assign)?;
                    if self.is_op(Op::Semi) {
                        return Err(ParseError { pos: name_pos, message: "Missing initializer expression".into() });
                    }
                    let init = self.expr()?;
                    self.expect_top_semi()?;
                    TopKind::Const { name, ty, init }
                }
                Tok::Kw(Kw::Enum) => {
                    self.bump();
                    let name = self.expect_ident()?;
                    self.expect_op(Op::LBrace)?;
                    let mut values = Vec::new();
                    if !self.is_op(Op::RBrace) {
                        loop {
                            let ann = if self.is_kw(Kw::Annotation) { Some(self.annotation()?) } else { None };
                            let p = self.pos();
                            let v = self.expect_ident()?;
                            values.push((ann, v, p));
                            if !self.eat_op(Op::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect_op(Op::RBrace)?;
                    TopKind::Enum { name, values }
                }
                Tok::Kw(Kw::Type) => {
                    self.bump();
                    let name = self.expect_ident()?;
                    self.expect_kw(Kw::Typecheck)?;
                    let typecheck = self.qual_name()?;
                    self.expect_op(Op::Semi)?;
                    TopKind::Type { name, typecheck }
                }
                _ => {
                    if annotation.is_some() || export {
                        return self.unexpected("{'enum', 'type', 'function', 'predicate', 'operator', 'typeconvert', 'const'}", None);
                    }
                    return self.err(format!("extraneous input {} expecting {expect}", self.peek().describe()));
                }
            };
            items.push(TopLevel { pos, name_pos, export, annotation, kind });
        }
        Ok(Module { version, items })
    }

    fn annotation(&mut self) -> PResult<Expr> {
        self.expect_kw(Kw::Annotation)?;
        if !self.is_op(Op::LBrace) {
            return self.unexpected("'{'", Some(&Tok::Op(Op::LBrace)));
        }
        self.primary()
    }

    fn fn_def(&mut self, kind: FnKind, pos: Pos) -> PResult<Arc<FnDef>> {
        let name = self.expect_ident()?;
        self.fn_def_after_name(kind, name, pos)
    }

    fn fn_def_after_name(&mut self, kind: FnKind, name: Name, pos: Pos) -> PResult<Arc<FnDef>> {
        self.expect_op(Op::LParen)?;
        let params = self.params()?;
        let returns = if self.eat_kw(Kw::Returns) { Some(self.type_name()?) } else { None };
        let precondition = if self.eat_kw(Kw::Precondition) { Some(self.stmt()?) } else { None };
        if !self.is_op(Op::LBrace) {
            return self.unexpected("'{'", Some(&Tok::Op(Op::LBrace)));
        }
        let body_pos = self.pos();
        let (body, end) = self.block()?;
        Ok(Arc::new(FnDef { kind, name, params, returns, precondition, body: Body::Block(body), pos, body_pos, end }))
    }

    /// Parameters after `(`, consuming `)`.
    fn params(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();
        if self.eat_op(Op::RParen) {
            return Ok(params);
        }
        loop {
            let pos = self.pos();
            let name = self.expect_ident()?;
            let ty = if self.eat_kw(Kw::Is) { Some(self.type_name()?) } else { None };
            params.push(Param { name, ty, pos });
            if self.eat_op(Op::Comma) {
                continue;
            }
            self.expect_op(Op::RParen)?;
            return Ok(params);
        }
    }

    fn qual_name(&mut self) -> PResult<QualName> {
        let first = self.expect_ident()?;
        if self.is_op(Op::Scope) {
            self.bump();
            let name = self.expect_ident()?;
            return Ok(QualName { ns: Some(first), name });
        }
        Ok(QualName { ns: None, name: first })
    }

    /// A type after `is` / `as` / `returns`: a (namespaced) identifier, or
    /// the keywords `function` and `undefined`.
    fn type_name(&mut self) -> PResult<QualName> {
        match self.peek() {
            Tok::Kw(Kw::Function) => {
                self.bump();
                Ok(QualName { ns: None, name: "function".into() })
            }
            Tok::Kw(Kw::Undefined) => {
                self.bump();
                Ok(QualName { ns: None, name: "undefined".into() })
            }
            Tok::Ident(_) => self.qual_name(),
            _ => self.unexpected("ID", None),
        }
    }

    // ------------------------------------------------------------ statements

    /// `{ stmts }`; returns the statements and the closing brace position.
    fn block(&mut self) -> PResult<(Vec<Stmt>, Pos)> {
        self.expect_op(Op::LBrace)?;
        let mut out = Vec::new();
        while !self.is_op(Op::RBrace) {
            if matches!(self.peek(), Tok::Eof) {
                return self.unexpected("'}'", Some(&Tok::Op(Op::RBrace)));
            }
            out.push(self.stmt()?);
        }
        let end = self.pos();
        self.bump();
        Ok((out, end))
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        let pos = self.pos();
        let mut text = None;
        let kind = match self.peek().clone() {
            Tok::Op(Op::LBrace) => StmtKind::Block(self.block()?.0),
            Tok::Op(Op::Semi) => {
                self.bump();
                StmtKind::Empty
            }
            Tok::Kw(Kw::Annotation) => {
                let annotation = self.annotation()?;
                let stmt = Box::new(self.stmt()?);
                StmtKind::Annotated { annotation, stmt }
            }
            Tok::Kw(Kw::Var) | Tok::Kw(Kw::Const) => {
                let s = self.var_decl()?;
                self.expect_op(Op::Semi)?;
                s
            }
            Tok::Kw(Kw::If) => {
                self.bump();
                self.expect_op(Op::LParen)?;
                let cond = self.expr()?;
                self.expect_op(Op::RParen)?;
                let then = Box::new(self.stmt()?);
                let otherwise = if self.eat_kw(Kw::Else) { Some(Box::new(self.stmt()?)) } else { None };
                StmtKind::If { cond, then, otherwise }
            }
            Tok::Kw(Kw::While) => {
                self.bump();
                self.expect_op(Op::LParen)?;
                let cond = self.expr()?;
                self.expect_op(Op::RParen)?;
                StmtKind::While { cond, body: Box::new(self.stmt()?) }
            }
            Tok::Kw(Kw::For) => self.for_stmt()?,
            Tok::Kw(Kw::Return) => {
                self.bump();
                let e = if self.is_op(Op::Semi) { None } else { Some(self.expr()?) };
                self.expect_op(Op::Semi)?;
                StmtKind::Return(e)
            }
            Tok::Kw(Kw::Break) => {
                self.bump();
                self.expect_op(Op::Semi)?;
                StmtKind::Break
            }
            Tok::Kw(Kw::Continue) => {
                self.bump();
                self.expect_op(Op::Semi)?;
                StmtKind::Continue
            }
            Tok::Kw(Kw::Throw) => {
                self.bump();
                let e = if self.is_op(Op::Semi) { None } else { Some(self.expr()?) };
                self.expect_op(Op::Semi)?;
                StmtKind::Throw(e)
            }
            Tok::Kw(Kw::Try) if self.try_is_statement() => {
                self.bump();
                let silent = self.is_ident("silent");
                if silent {
                    self.bump();
                }
                let body = self.block()?.0;
                let catch = if self.eat_kw(Kw::Catch) {
                    let name = if self.eat_op(Op::LParen) {
                        let n = self.expect_ident()?;
                        self.expect_op(Op::RParen)?;
                        Some(n)
                    } else {
                        None
                    };
                    Some((name, self.block()?.0))
                } else {
                    None
                };
                StmtKind::Try { silent, body, catch }
            }
            Tok::Kw(k @ (Kw::Assert | Kw::Case | Kw::Default | Kw::Do)) => {
                return self.err(format!("'{}' is reserved", k.as_str()));
            }
            _ => {
                let start = self.i;
                let s = self.simple_stmt()?;
                if matches!(s, StmtKind::Expr(_)) {
                    let (a, b) = (self.toks[start].start, self.toks[self.i.saturating_sub(1)].end);
                    text = Some(Arc::from(self.src[a..b.max(a)].trim()));
                }
                self.expect_op(Op::Semi)?;
                s
            }
        };
        Ok(Stmt { pos, kind, text })
    }

    /// `try {` or `try silent {` (a statement) vs `try (expr)` (an expression).
    fn try_is_statement(&self) -> bool {
        match self.peek_at(1) {
            Tok::Op(Op::LBrace) => true,
            Tok::Ident(s) if &**s == "silent" => matches!(self.peek_at(2), Tok::Op(Op::LBrace)),
            _ => false,
        }
    }

    fn var_decl(&mut self) -> PResult<StmtKind> {
        let constant = self.is_kw(Kw::Const);
        self.bump();
        let name_pos = self.pos();
        let name = self.expect_ident()?;
        let ty = if self.eat_kw(Kw::Is) { Some(self.type_name()?) } else { None };
        let init = if self.eat_op(Op::Assign) {
            if self.is_op(Op::Semi) {
                return Err(ParseError { pos: name_pos, message: "Missing initializer expression".into() });
            }
            Some(self.expr()?)
        } else {
            if constant {
                return self.unexpected("'='", Some(&Tok::Op(Op::Assign)));
            }
            if ty.is_some() {
                return self.err(format!("variable {name} with a type must be initialized"));
            }
            None
        };
        Ok(StmtKind::Var { constant, name, ty, init })
    }

    /// An assignment or an expression statement (without the `;`).
    fn simple_stmt(&mut self) -> PResult<StmtKind> {
        let start = self.i;
        let e = self.expr()?;
        let op = match self.peek() {
            Tok::Op(Op::Assign) => Some(None),
            Tok::Op(Op::AddAssign) => Some(Some(BinOp::Add)),
            Tok::Op(Op::SubAssign) => Some(Some(BinOp::Sub)),
            Tok::Op(Op::MulAssign) => Some(Some(BinOp::Mul)),
            Tok::Op(Op::DivAssign) => Some(Some(BinOp::Div)),
            Tok::Op(Op::PowAssign) => Some(Some(BinOp::Pow)),
            Tok::Op(Op::ModAssign) => Some(Some(BinOp::Mod)),
            Tok::Op(Op::CatAssign) => Some(Some(BinOp::Cat)),
            Tok::Op(Op::AndAssign) => Some(Some(BinOp::And)),
            Tok::Op(Op::OrAssign) => Some(Some(BinOp::Or)),
            Tok::Op(Op::CoalesceAssign) => Some(Some(BinOp::Coalesce)),
            _ => None,
        };
        let Some(op) = op else { return Ok(StmtKind::Expr(e)) };
        if !is_lvalue(&e) || self.toks[start].tok == Tok::Op(Op::LParen) {
            return self.err(format!("no viable alternative at input '{}'", self.text_through(start)));
        }
        self.bump();
        let value = self.expr()?;
        Ok(StmtKind::Assign { target: e, op, value })
    }

    fn for_stmt(&mut self) -> PResult<StmtKind> {
        self.bump();
        self.expect_op(Op::LParen)?;
        // for-in: `for (var a in x)`, `for (var k, v in x)`, `for (a in x)`.
        let declare = self.is_kw(Kw::Var);
        let o = declare as usize;
        let is_in = matches!(self.peek_at(o), Tok::Ident(_))
            && (matches!(self.peek_at(o + 1), Tok::Kw(Kw::In)) || (matches!(self.peek_at(o + 1), Tok::Op(Op::Comma)) && matches!(self.peek_at(o + 2), Tok::Ident(_)) && matches!(self.peek_at(o + 3), Tok::Kw(Kw::In))));
        if is_in {
            if declare {
                self.bump();
            }
            let first = self.expect_ident()?;
            let (key, value) = if self.eat_op(Op::Comma) { (Some(first), self.expect_ident()?) } else { (None, first) };
            self.expect_kw(Kw::In)?;
            let iter = self.expr()?;
            self.expect_op(Op::RParen)?;
            let body = Box::new(self.stmt()?);
            return Ok(StmtKind::ForIn { declare, key, value, iter, body });
        }
        let init = if self.is_op(Op::Semi) {
            None
        } else {
            let pos = self.pos();
            let kind = if self.is_kw(Kw::Var) || self.is_kw(Kw::Const) { self.var_decl()? } else { self.simple_stmt()? };
            Some(Box::new(Stmt { pos, kind, text: None }))
        };
        self.expect_op(Op::Semi)?;
        let cond = if self.is_op(Op::Semi) { None } else { Some(self.expr()?) };
        self.expect_op(Op::Semi)?;
        let step = if self.is_op(Op::RParen) {
            None
        } else {
            let pos = self.pos();
            Some(Box::new(Stmt { pos, kind: self.simple_stmt()?, text: None }))
        };
        self.expect_op(Op::RParen)?;
        let body = Box::new(self.stmt()?);
        Ok(StmtKind::For { init, cond, step, body })
    }

    // ------------------------------------------------------------ expressions

    fn expr(&mut self) -> PResult<Expr> {
        self.ternary()
    }

    /// `?:` is left-associative in Onshape (live, behaviours.md "?:"):
    /// `a ? x : b ? y : z` is `(a ? x : b) ? y : z`.
    fn ternary(&mut self) -> PResult<Expr> {
        let mut cond = self.binary(0)?;
        while self.is_op(Op::Question) {
            self.bump();
            let a = self.ternary()?;
            self.expect_op(Op::Colon)?;
            let b = self.binary(0)?;
            let pos = cond.pos;
            cond = Expr::new(pos, ExprKind::Cond(cond, a, b));
        }
        Ok(cond)
    }

    /// Binary operators by level: 0 `??`, 1 `||`, 2 `&&`, 3 `== !=`,
    /// 4 `< > <= >=`, 5 `+ - ~`, 6 `* / %`, 7 `is as` (postfix: `p as
    /// Vector * meter` is `(p as Vector) * meter`), then unary.
    fn binary(&mut self, level: u8) -> PResult<Expr> {
        if level == 8 {
            return self.unary();
        }
        let mut lhs = self.binary(level + 1)?;
        loop {
            let pos = lhs.pos;
            if level == 7 {
                if self.is_kw(Kw::Is) || self.is_kw(Kw::As) {
                    let is = self.is_kw(Kw::Is);
                    self.bump();
                    let ty = self.type_name()?;
                    lhs = Expr::new(pos, if is { ExprKind::Is(lhs, ty) } else { ExprKind::As(lhs, ty) });
                    continue;
                }
                return Ok(lhs);
            }
            let op = match (level, self.peek()) {
                (0, Tok::Op(Op::Coalesce)) => BinOp::Coalesce,
                (1, Tok::Op(Op::Or)) => BinOp::Or,
                (2, Tok::Op(Op::And)) => BinOp::And,
                (3, Tok::Op(Op::Eq)) => BinOp::Eq,
                (3, Tok::Op(Op::Ne)) => BinOp::Ne,
                (4, Tok::Op(Op::Lt)) => BinOp::Lt,
                (4, Tok::Op(Op::Gt)) => BinOp::Gt,
                (4, Tok::Op(Op::Le)) => BinOp::Le,
                (4, Tok::Op(Op::Ge)) => BinOp::Ge,
                (5, Tok::Op(Op::Plus)) => BinOp::Add,
                (5, Tok::Op(Op::Minus)) => BinOp::Sub,
                (5, Tok::Op(Op::Tilde)) => BinOp::Cat,
                (6, Tok::Op(Op::Star)) => BinOp::Mul,
                (6, Tok::Op(Op::Slash)) => BinOp::Div,
                (6, Tok::Op(Op::Percent)) => BinOp::Mod,
                _ => return Ok(lhs),
            };
            self.bump();
            let rhs = self.binary(level + 1)?;
            // Located at the left operand's start, like Onshape's notices.
            lhs = Expr::new(pos, ExprKind::Binary(op, lhs, rhs));
        }
    }

    fn unary(&mut self) -> PResult<Expr> {
        let pos = self.pos();
        if self.eat_op(Op::Minus) {
            let e = self.unary()?;
            // Negated constants are constants.
            if let ExprKind::Number(n) = &*e.kind {
                return Ok(Expr::new(pos, ExprKind::Number(-n)));
            }
            return Ok(Expr::new(pos, ExprKind::Unary(UnOp::Neg, e)));
        }
        if self.eat_op(Op::Not) {
            let e = self.unary()?;
            return Ok(Expr::new(pos, ExprKind::Unary(UnOp::Not, e)));
        }
        self.power()
    }

    /// `^` is right associative and binds tighter than unary minus.
    fn power(&mut self) -> PResult<Expr> {
        let base = self.postfix()?;
        if self.is_op(Op::Caret) {
            self.bump();
            let exp = self.unary()?;
            return Ok(Expr::new(base.pos, ExprKind::Binary(BinOp::Pow, base, exp)));
        }
        Ok(base)
    }

    fn args(&mut self) -> PResult<Vec<Expr>> {
        // After `(`.
        let mut args = Vec::new();
        if self.eat_op(Op::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.expr()?);
            if self.eat_op(Op::Comma) {
                continue;
            }
            self.expect_op(Op::RParen)?;
            return Ok(args);
        }
    }

    fn postfix(&mut self) -> PResult<Expr> {
        let mut e = self.primary()?;
        loop {
            // Like Onshape's notices, a postfix expression is located at
            // its start (`a[3]` at `a`).
            let pos = e.pos;
            match self.peek() {
                Tok::Op(Op::LParen) => {
                    self.bump();
                    let args = self.args()?;
                    e = Expr::new(pos, ExprKind::Call(e, args));
                }
                Tok::Op(Op::LBracket) | Tok::Op(Op::SafeIndex) => {
                    let safe = self.is_op(Op::SafeIndex);
                    self.bump();
                    if self.eat_op(Op::RBracket) {
                        e = Expr::new(pos, ExprKind::BoxGet(e, safe));
                    } else {
                        let idx = self.expr()?;
                        self.expect_op(Op::RBracket)?;
                        e = Expr::new(pos, ExprKind::Index(e, idx, safe));
                    }
                }
                Tok::Op(Op::Dot) | Tok::Op(Op::SafeDot) => {
                    let safe = self.is_op(Op::SafeDot);
                    self.bump();
                    let name = self.expect_ident()?;
                    e = Expr::new(pos, ExprKind::Member(e, name, safe));
                }
                Tok::Op(Op::Arrow) => {
                    self.bump();
                    let f = self.qual_name()?;
                    self.expect_op(Op::LParen)?;
                    let args = self.args()?;
                    e = Expr::new(pos, ExprKind::Arrow(e, f, args));
                }
                _ => return Ok(e),
            }
        }
    }

    /// Index of the token closing the parenthesis at `self.i + offset`.
    fn matching_paren(&self, offset: usize) -> Option<usize> {
        let mut depth = 0usize;
        for j in self.i + offset..self.toks.len() {
            match &self.toks[j].tok {
                Tok::Op(Op::LParen) | Tok::Op(Op::LBracket) | Tok::Op(Op::SafeIndex) | Tok::Op(Op::LBrace) => depth += 1,
                Tok::Op(Op::RParen) | Tok::Op(Op::RBracket) | Tok::Op(Op::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                Tok::Eof => return None,
                _ => {}
            }
        }
        None
    }

    fn lambda_body(&mut self, params: Vec<Param>, returns: Option<QualName>, pos: Pos) -> PResult<Expr> {
        let body_pos = self.pos();
        let (body, end) = if self.is_op(Op::LBrace) && !self.brace_starts_map() {
            let (b, end) = self.block()?;
            (Body::Block(b), end)
        } else {
            let e = self.expr()?;
            let end = self.toks[self.i.saturating_sub(1)].pos;
            (Body::Expr(e), end)
        };
        let def = FnDef { kind: FnKind::Lambda, name: "".into(), params, returns, precondition: None, body, pos, body_pos, end };
        Ok(Expr::new(pos, ExprKind::Lambda(Arc::new(def))))
    }

    /// After `=>`: whether `{` opens a map literal (`{ "k" : v }`,
    /// `{ k : v }`, `{ (k) : v }`) rather than a function body.
    fn brace_starts_map(&self) -> bool {
        match self.peek_at(1) {
            Tok::Str(_) | Tok::Ident(_) | Tok::Number(_) => matches!(self.peek_at(2), Tok::Op(Op::Colon)),
            Tok::Op(Op::LParen) => self.matching_paren(1).is_some_and(|j| matches!(self.toks.get(j + 1).map(|t| &t.tok), Some(Tok::Op(Op::Colon)))),
            _ => false,
        }
    }

    fn map_entries(&mut self, close: Op) -> PResult<Vec<(MapKey, Expr)>> {
        let mut entries = Vec::new();
        loop {
            if self.eat_op(close) {
                return Ok(entries);
            }
            let key = match self.peek().clone() {
                Tok::Ident(s) if matches!(self.peek_at(1), Tok::Op(Op::Colon)) => {
                    self.bump();
                    MapKey::Str(s)
                }
                Tok::Str(s) if matches!(self.peek_at(1), Tok::Op(Op::Colon)) => {
                    self.bump();
                    MapKey::Str(s)
                }
                _ => MapKey::Expr(self.expr()?),
            };
            self.expect_op(Op::Colon)?;
            let value = self.expr()?;
            entries.push((key, value));
            if !self.eat_op(Op::Comma) {
                self.expect_op(close)?;
                return Ok(entries);
            }
        }
    }

    fn primary(&mut self) -> PResult<Expr> {
        let pos = self.pos();
        let start = self.i;
        let kind = match self.peek().clone() {
            Tok::Number(n) => {
                self.bump();
                ExprKind::Number(n)
            }
            Tok::Str(s) => {
                self.bump();
                ExprKind::Str(s)
            }
            Tok::Kw(Kw::True) => {
                self.bump();
                ExprKind::Bool(true)
            }
            Tok::Kw(Kw::False) => {
                self.bump();
                ExprKind::Bool(false)
            }
            Tok::Kw(Kw::Undefined) => {
                self.bump();
                ExprKind::Undefined
            }
            Tok::Kw(Kw::Inf) => {
                self.bump();
                ExprKind::Number(f64::INFINITY)
            }
            Tok::Ident(name) => {
                // `x => ...`
                if matches!(self.peek_at(1), Tok::Op(Op::FatArrow)) {
                    self.bump();
                    self.bump();
                    return self.lambda_body(vec![Param { name, ty: None, pos }], None, pos);
                }
                let q = self.qual_name()?;
                ExprKind::Ident(q)
            }
            Tok::Builtin(name) => {
                self.bump();
                if !self.is_op(Op::LParen) {
                    return self.unexpected("'('", Some(&Tok::Op(Op::LParen)));
                }
                ExprKind::Builtin(name)
            }
            Tok::Op(Op::LBracket) => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    if self.eat_op(Op::RBracket) {
                        break;
                    }
                    items.push(self.expr()?);
                    if !self.eat_op(Op::Comma) {
                        self.expect_op(Op::RBracket)?;
                        break;
                    }
                }
                ExprKind::Array(items)
            }
            Tok::Op(Op::LBrace) => {
                self.bump();
                ExprKind::Map(self.map_entries(Op::RBrace)?)
            }
            Tok::Op(Op::LParen) => {
                // `(a, b) => ...` / `(a is T) returns R => ...`
                if let Some(close) = self.matching_paren(0)
                    && matches!(self.toks.get(close + 1).map(|t| &t.tok), Some(Tok::Op(Op::FatArrow)) | Some(Tok::Kw(Kw::Returns)))
                {
                    self.bump();
                    let params = self.params()?;
                    let returns = if self.eat_kw(Kw::Returns) { Some(self.type_name()?) } else { None };
                    self.expect_op(Op::FatArrow)?;
                    return self.lambda_body(params, returns, pos);
                }
                self.bump();
                let e = self.expr()?;
                self.expect_op(Op::RParen)?;
                return Ok(e);
            }
            Tok::Kw(Kw::Function) => {
                self.bump();
                self.expect_op(Op::LParen)?;
                let params = self.params()?;
                let returns = if self.eat_kw(Kw::Returns) { Some(self.type_name()?) } else { None };
                let precondition = if self.eat_kw(Kw::Precondition) { Some(self.stmt()?) } else { None };
                if !self.is_op(Op::LBrace) {
                    return self.unexpected("'{'", Some(&Tok::Op(Op::LBrace)));
                }
                let body_pos = self.pos();
                let (body, end) = self.block()?;
                let def = FnDef { kind: FnKind::Lambda, name: "".into(), params, returns, precondition, body: Body::Block(body), pos, body_pos, end };
                ExprKind::Lambda(Arc::new(def))
            }
            Tok::Kw(Kw::New) => {
                self.bump();
                if !self.is_ident("box") {
                    return self.unexpected("'box'", None);
                }
                self.bump();
                self.expect_op(Op::LParen)?;
                let e = self.expr()?;
                self.expect_op(Op::RParen)?;
                ExprKind::NewBox(e)
            }
            Tok::Kw(Kw::Try) => {
                self.bump();
                let silent = self.is_ident("silent");
                if silent {
                    self.bump();
                }
                self.expect_op(Op::LParen)?;
                let e = self.expr()?;
                self.expect_op(Op::RParen)?;
                ExprKind::Try(silent, e)
            }
            Tok::Kw(Kw::Switch) => {
                self.bump();
                self.expect_op(Op::LParen)?;
                let subject = self.expr()?;
                self.expect_op(Op::RParen)?;
                self.expect_op(Op::LBrace)?;
                ExprKind::Switch(subject, self.map_entries(Op::RBrace)?)
            }
            Tok::Eof => return self.err("mismatched input <EOF> expecting an expression".to_string()),
            _ => {
                // Like ANTLR's prediction, the input quoted starts at the
                // binary operator whose operand is missing (`'+ ;'`).
                let from = if start > 0 && is_binary_op(&self.toks[start - 1].tok) { start - 1 } else { start };
                return Err(ParseError { pos, message: format!("no viable alternative at input '{}'", self.text_through(from)) });
            }
        };
        Ok(Expr::new(pos, kind))
    }
}

fn is_binary_op(t: &Tok) -> bool {
    use Op::*;
    matches!(t, Tok::Op(Coalesce | Or | And | Eq | Ne | Lt | Gt | Le | Ge | Plus | Minus | Tilde | Star | Slash | Percent | Caret))
}

fn is_lvalue(e: &Expr) -> bool {
    match &*e.kind {
        ExprKind::Ident(q) => q.ns.is_none(),
        ExprKind::Member(b, _, false) | ExprKind::Index(b, _, false) | ExprKind::BoxGet(b, false) => is_lvalue(b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses() {
        let src = r#"FeatureScript 3095;
import(path : "onshape/std/common.fs", version : "3095.0");

annotation { "Feature Type Name" : "My Feature", "Feature Type Description" : "" }
export const myFeature = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        // Define the parameters of the feature type
    }
    {
        // Define the function's action
    });
"#;
        let m = parse(src).unwrap();
        assert_eq!(m.version, Some(3095));
        assert_eq!(m.items.len(), 2);
        assert!(matches!(&m.items[1].kind, TopKind::Const { name, .. } if &**name == "myFeature"));
    }

    #[test]
    fn onshape_error_message_for_stray_word() {
        let src = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\nconst x = 1;\nthis\n";
        let e = parse(src).unwrap_err();
        assert_eq!(e.pos, Pos { line: 4, col: 1 });
        assert_eq!(e.message, "extraneous input 'this' expecting {<EOF>, 'annotation', 'export', 'enum', 'type', 'function', 'predicate', 'operator', 'typeconvert', 'const', TOP_SEMI}");
    }

    #[test]
    fn expressions() {
        let e = parse_expr("a ?? b || c && d == e < f + g * -h ^ 2 is number").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Binary(BinOp::Coalesce, _, _)));
        let e = parse_expr("x->f(1)->g()").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Arrow(_, q, a) if &*q.name == "g" && a.is_empty()));
        let e = parse_expr("(a, b is number) => a * b").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Lambda(f) if f.params.len() == 2));
        let e = parse_expr("{ a : 1, (b) : 2, 'c' : 3, }").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Map(m) if m.len() == 3));
        let e = parse_expr("switch (t) { E.A : 1, E.B : 2 }").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Switch(_, m) if m.len() == 2));
        let e = parse_expr("-3").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Number(n) if *n == -3.0));
        let e = parse_expr("b?.c?[0]?[]").unwrap();
        assert!(matches!(&*e.kind, ExprKind::BoxGet(_, true)));
        let e = parse_expr("element as map == element").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Binary(BinOp::Eq, _, _)));
        let e = parse_expr("p as Vector * meter").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Binary(BinOp::Mul, l, _) if matches!(&*l.kind, ExprKind::As(..))));
        let e = parse_expr("x => { 'k' : x }").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Lambda(f) if matches!(f.body, Body::Expr(_))));
        let e = parse_expr("try silent(f(x))").unwrap();
        assert!(matches!(&*e.kind, ExprKind::Try(true, _)));
    }

    #[test]
    fn statements() {
        let src = r#"
function f(x is number) returns number
precondition x >= 0;
{
    var a = [1, 2];
    a[0] += 1;
    for (var i = 0; i < 3; i += 1) { a = append(a, i); }
    for (var k, v in { a : 1 }) println(k ~ v);
    for (e in a) { continue; }
    try silent { g(); } catch (e) { throw e; }
    try { g(); }
    var b = new box(0);
    b[] = 2;
    if (x > 1) return 1; else if (x > 0) return 2; else { return 3; }
}
export operator+(a is Foo, b is Foo) returns Foo { return a; }
operator-(a is Foo) { return a; }
export enum E { annotation { "Name" : "A" } A, B }
export type T typecheck canBeT;
"#;
        let m = parse(src).unwrap();
        assert_eq!(m.items.len(), 5);
        assert!(matches!(&m.items[2].kind, TopKind::Function(f) if f.kind == FnKind::Negate));
    }

    #[test]
    fn bad_assignment_target() {
        let e = parse("function f() { (x).y = 1; }").unwrap_err();
        assert!(e.message.starts_with("no viable alternative"), "{}", e.message);
    }

    #[test]
    fn missing_semicolon() {
        let e = parse("function f() { var x = 1 }").unwrap_err();
        assert_eq!(e.message, "missing ';' at '}'");
    }
}
