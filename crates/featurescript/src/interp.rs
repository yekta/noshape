//! The FeatureScript evaluator.
//!
//! An [`Interp`] runs compiled code ([`crate::ir`]) on a value stack (one
//! frame of slots per call). Exceptions are Rust errors carrying the thrown
//! FeatureScript value; every raised exception outside `try silent` is also
//! recorded as a notice, like Onshape's notices flyout.

use crate::ast::{BinOp, FnKind, Name};
use crate::ir::*;
use crate::lexer::Pos;
use crate::value::{Func, Map, Shared, StdType, Tag, Value, to_display, type_info};
use parking_lot::{ReentrantMutex, RwLock};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

/// A runtime builtin (`@name`).
pub type NativeFn = fn(&mut Interp, Vec<Value>) -> R<Value>;

pub type R<T> = Result<T, Box<Exception>>;

#[derive(Clone, Debug)]
pub struct TraceFrame {
    pub function: Name,
    pub module: Arc<str>,
    /// Where the function was called from.
    pub pos: Pos,
}

#[derive(Clone, Debug)]
pub struct Exception {
    pub value: Value,
    pub module: Arc<str>,
    pub pos: Pos,
    pub trace: Vec<TraceFrame>,
    /// A language error (not a `throw`): `catch` sees "Execution error".
    pub internal: bool,
}

impl Exception {
    /// Text of the thrown value (`message` of library error maps).
    pub fn message(&self) -> String {
        error_text(&self.value)
    }
}

/// Readable text of a thrown value: strings as-is, library error maps by
/// their `message`.
pub fn error_text(v: &Value) -> String {
    match v {
        Value::Map(..) => match v.get("message") {
            Value::Undefined => to_display(v),
            m => error_text(m),
        },
        Value::Str(s, _) => s.to_string(),
        other => to_display(other),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub kind: NoticeKind,
    pub message: String,
    pub module: Arc<str>,
    pub pos: Pos,
    pub trace: Vec<TraceFrame>,
}

// ---------------------------------------------------------------- natives

static HOST_NATIVES: LazyLock<RwLock<HashMap<&'static str, NativeFn>>> = LazyLock::new(Default::default);

/// Values the host substitutes for top-level constants, by (module path,
/// name): std features the host evaluates itself.
type Overrides = HashMap<(Arc<str>, Arc<str>), Value>;
static GLOBAL_OVERRIDES: LazyLock<RwLock<Overrides>> = LazyLock::new(Default::default);

/// Make constant `name` of `module` (e.g. "onshape/std/wrap.fs") evaluate
/// to `value` instead of its initializer. Must happen before it is first read.
pub fn override_global(module: &str, name: &str, value: Value) {
    GLOBAL_OVERRIDES.write().insert((module.into(), name.into()), value);
}

/// Register runtime builtins provided by the host (modeling operations).
/// Must happen before modules using them are compiled.
pub fn register_natives(list: &[(&'static str, NativeFn)]) {
    let mut m = HOST_NATIVES.write();
    for (n, f) in list {
        m.insert(n, *f);
    }
}

/// A function value calling the native `name` with `bound` first.
pub fn native_function(name: &str, bound: Vec<Value>) -> Option<Value> {
    let f = native(name)?;
    Some(Value::Function(Shared::new(Func::Native { name: name.into(), f, bound })))
}

pub fn native(name: &str) -> Option<NativeFn> {
    if let Some(f) = HOST_NATIVES.read().get(name) {
        return Some(*f);
    }
    crate::natives::core(name)
}

// ---------------------------------------------------------------- interpreter

enum Flow {
    Normal,
    Break,
    Continue,
    Return(Value),
    /// A predicate expression statement was false.
    False,
}

/// Serializes the initialization of top-level constants across threads.
static INIT_LOCK: LazyLock<ReentrantMutex<()>> = LazyLock::new(|| ReentrantMutex::new(()));

thread_local! {
    static INITIALIZING: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

/// Onshape: "Maximum call stack depth exceeded." after ~1020 nested calls.
const MAX_DEPTH: usize = 1024;
const MAX_NOTICES: usize = 500;

pub struct Interp {
    stack: Vec<Value>,
    base: usize,
    frames: Vec<TraceFrame>,
    module: Arc<str>,
    /// Position of the expression being evaluated (natives read it).
    pub pos: Pos,
    steps: u64,
    deadline: Option<Instant>,
    silent: u32,
    pub notices: Vec<Notice>,
    /// `print` / `println` output.
    pub output: String,
    /// Host state for natives (the modeling session).
    pub host: Option<Box<dyn std::any::Any>>,
    /// Overload sets merged with the loaded modules' exported overloads,
    /// by set address: (registry generation, candidates).
    merged: HashMap<usize, (u64, Candidates)>,
    /// The `@native` being called (its errors read "@evPlane: ...").
    pub native_name: Name,
    /// The predicate statement that evaluated to false: (position, text).
    failed_check: Option<(Pos, Arc<str>)>,
}

/// The overloads a call chooses from.
type Candidates = Arc<Vec<Arc<FnDecl>>>;

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

impl Interp {
    pub fn new() -> Self {
        Interp {
            stack: Vec::with_capacity(1024),
            base: 0,
            frames: Vec::new(),
            module: "".into(),
            pos: Pos::default(),
            steps: 0,
            deadline: None,
            silent: 0,
            notices: Vec::new(),
            output: String::new(),
            host: None,
            merged: HashMap::new(),
            native_name: "".into(),
            failed_check: None,
        }
    }

    /// Abort evaluation with an error after `deadline`.
    pub fn set_deadline(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
    }

    pub fn module(&self) -> &Arc<str> {
        &self.module
    }

    pub fn trace(&self) -> &[TraceFrame] {
        &self.frames
    }

    // ------------------------------------------------------------ errors

    /// Raise `value` at the current position.
    pub fn raise(&mut self, value: Value) -> Box<Exception> {
        let e = Box::new(Exception { value, module: self.module.clone(), pos: self.pos, trace: self.frames.clone(), internal: false });
        self.note_exception(&e);
        e
    }

    /// Raise `value` with `notice` as the notice text.
    pub fn raise_noted(&mut self, value: Value, notice: String) -> Box<Exception> {
        let e = Box::new(Exception { value, module: self.module.clone(), pos: self.pos, trace: self.frames.clone(), internal: false });
        if self.silent == 0 && self.notices.len() < MAX_NOTICES {
            self.notices.push(Notice { kind: NoticeKind::Error, message: notice, module: e.module.clone(), pos: e.pos, trace: e.trace.clone() });
        }
        e
    }

    /// `throw v`: the notice reads "throw custom", "throw my message"
    /// (a regenError's custom message), "throw { "a" : 1 }" (live).
    pub fn throw(&mut self, value: Value) -> Box<Exception> {
        let text = match &value {
            Value::Str(s, _) => s.to_string(),
            Value::Map(..) if !value.get("customMessage").is_undefined() => error_text(value.get("customMessage")),
            Value::Map(..) if !value.get("message").is_undefined() => error_text(value.get("message")),
            other => crate::value::to_display_quoted(other),
        };
        self.raise_noted(value, format!("throw {text}"))
    }

    /// Raise a language error (its message is the notice; `catch` sees
    /// "Execution error", like Onshape).
    pub fn error(&mut self, message: impl Into<String>) -> Box<Exception> {
        let e = Box::new(Exception { value: Value::str(message.into()), module: self.module.clone(), pos: self.pos, trace: self.frames.clone(), internal: true });
        self.note_exception(&e);
        e
    }

    fn note_exception(&mut self, e: &Exception) {
        if self.silent == 0 && self.notices.len() < MAX_NOTICES {
            self.notices.push(Notice { kind: NoticeKind::Error, message: e.message(), module: e.module.clone(), pos: e.pos, trace: e.trace.clone() });
        }
    }

    pub fn notice(&mut self, kind: NoticeKind, message: String) {
        if self.notices.len() < MAX_NOTICES {
            self.notices.push(Notice { kind, message, module: self.module.clone(), pos: self.pos, trace: self.frames.clone() });
        }
    }

    fn tick(&mut self) -> R<()> {
        self.steps += 1;
        if self.steps & 0xFFFF == 0
            && let Some(d) = self.deadline
            && Instant::now() > d
        {
            self.deadline = None;
            return Err(self.error("Regeneration timed out"));
        }
        Ok(())
    }

    // ------------------------------------------------------------ globals

    /// Value of a top-level constant (initialized on first use).
    pub fn global(&mut self, cell: &Arc<GlobalCell>) -> R<Value> {
        if let Some(v) = cell.value.get() {
            return v.clone().map_err(|e| self.raise(e));
        }
        let _guard = INIT_LOCK.lock();
        if let Some(v) = cell.value.get() {
            return v.clone().map_err(|e| self.raise(e));
        }
        let key = Arc::as_ptr(cell) as usize;
        if INITIALIZING.with(|i| i.borrow().contains(&key)) {
            return Err(self.error(format!("Constant {} is defined in terms of itself", cell.name)));
        }
        if let Some(v) = GLOBAL_OVERRIDES.read().get(&(cell.module.clone(), cell.name.clone())).cloned() {
            let _ = cell.value.set(Ok(v.clone()));
            return Ok(v);
        }
        let Some((ex, frame)) = cell.init.get() else {
            return Err(self.error(format!("Constant {} is not initialized", cell.name)));
        };
        INITIALIZING.with(|i| i.borrow_mut().push(key));
        let saved = (self.base, std::mem::replace(&mut self.module, cell.module.clone()), self.pos, self.silent);
        // Constants are initialized once for everyone: their exceptions are
        // not the current run's notices.
        self.silent += 1;
        let base = self.stack.len();
        self.stack.resize(base + *frame as usize, Value::Undefined);
        self.base = base;
        let mut r = self.eval(ex);
        if let (Ok(v), Some(Some(t))) = (&r, cell.ty.get())
            && !t.matches(v)
        {
            r = Err(self.error(format!("Value of constant {} should be {}, was {}", cell.name, t.name(), v.type_description())));
        }
        self.stack.truncate(base);
        self.base = saved.0;
        self.module = saved.1;
        self.pos = saved.2;
        self.silent = saved.3;
        INITIALIZING.with(|i| i.borrow_mut().pop());
        let stored = r.map_err(|e| e.value);
        let _ = cell.value.set(stored.clone());
        stored.map_err(|e| self.raise(e))
    }

    // ------------------------------------------------------------ calls

    /// Call a function value with arguments.
    pub fn call_value(&mut self, f: &Value, args: Vec<Value>) -> R<Value> {
        match f {
            Value::Function(func) => match &**func {
                Func::Closure { def, captures } => {
                    let (def, captures) = (def.clone(), captures.clone());
                    self.invoke(&def, args, &captures, true)
                }
                Func::Overloads(set) => {
                    let set = set.clone();
                    self.call_set(&set, args)
                }
                Func::Native { name, f, bound } => {
                    let (f, mut all) = (*f, bound.clone());
                    all.extend(args);
                    let saved = std::mem::replace(&mut self.native_name, name.clone());
                    let r = f(self, all);
                    self.native_name = saved;
                    r
                }
            },
            other => Err(self.error(format!("Called value must be function, value is {}", other.error_type()))),
        }
    }

    /// Call the best overload of `set` for `args`.
    pub fn call_set(&mut self, set: &Arc<OverloadSet>, args: Vec<Value>) -> R<Value> {
        let decl = self.resolve(set, &args)?;
        let Some(c) = decl.compiled.get().cloned() else {
            return Err(self.error(format!("Function {} is not compiled", set.name)));
        };
        self.invoke(&c, args, &[], false)
    }

    fn satisfies(types: &[Option<TypeRef>], args: &[Value]) -> bool {
        types.iter().zip(args).all(|(t, a)| t.is_none_or(|t| t.matches(a)))
    }

    fn more_specific(a: &[Option<TypeRef>], b: &[Option<TypeRef>]) -> bool {
        let spec = |t: &Option<TypeRef>| t.map(|t| t.specificity()).unwrap_or(0);
        let mut strictly = false;
        for (x, y) in a.iter().zip(b) {
            let (sx, sy) = (spec(x), spec(y));
            if sx < sy {
                return false;
            }
            if sx > sy {
                strictly = true;
            }
        }
        strictly
    }

    /// The most specific satisfying overload.
    /// The overloads a call considers: the set visible at the call plus the
    /// same-named exported functions of every loaded module.
    fn candidates(&mut self, set: &OverloadSet) -> Candidates {
        let key = set as *const OverloadSet as usize;
        let (generation, global) = crate::compile::exported_overloads(&set.name);
        if let Some((g, c)) = self.merged.get(&key)
            && *g == generation
        {
            return c.clone();
        }
        let mut all = set.fns.clone();
        for f in global.iter().flat_map(|g| g.iter()) {
            if !all.iter().any(|x| Arc::ptr_eq(x, f)) {
                all.push(f.clone());
            }
        }
        let all = Arc::new(all);
        self.merged.insert(key, (generation, all.clone()));
        all
    }

    pub fn resolve(&mut self, set: &OverloadSet, args: &[Value]) -> R<Arc<FnDecl>> {
        let candidates = self.candidates(set);
        let mut found: smallvec_lite::Small<Arc<FnDecl>> = smallvec_lite::Small::new();
        for f in candidates.iter() {
            if f.arity() == args.len() && Self::satisfies(&f.param_types, args) {
                found.push(f.clone());
            }
        }
        match found.len() {
            1 => return Ok(found.take(0)),
            0 => {
                let name = display_fn_name(&set.name);
                let types: Vec<String> = args.iter().map(|a| a.type_description()).collect();
                let arity_ok = set.fns.iter().any(|f| f.arity() == args.len());
                return Err(self.error(if arity_ok {
                    format!("Call {name}({}) does not match {name}({})", types.join(", "), set.fns.iter().filter(|f| f.arity() == args.len()).map(|f| f.param_types.iter().map(|t| t.map(|t| t.name()).unwrap_or_else(|| "any".into())).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join(") or ("))
                } else {
                    format!("Function {name} with {} argument(s) not found", args.len())
                }));
            }
            _ => {}
        }
        let all = found.as_slice();
        let best: Vec<usize> = (0..all.len()).filter(|&i| (0..all.len()).all(|j| j == i || !Self::more_specific(&all[j].param_types, &all[i].param_types))).collect();
        if best.len() == 1 {
            return Ok(all[best[0]].clone());
        }
        // Equally specific: the overload visible where the call is wins.
        let visible: Vec<usize> = best.iter().copied().filter(|&i| set.fns.iter().any(|f| Arc::ptr_eq(f, &all[i]))).collect();
        if visible.len() == 1 {
            return Ok(all[visible[0]].clone());
        }
        let name = display_fn_name(&set.name);
        Err(self.error(format!("Ambiguous call to {name}: more than one overload matches")))
    }

    /// Run a compiled function.
    pub fn invoke(&mut self, f: &CompiledFn, args: Vec<Value>, captures: &[Value], check_types: bool) -> R<Value> {
        if self.frames.len() >= MAX_DEPTH {
            return Err(self.error("Maximum call stack depth exceeded."));
        }
        self.tick()?;
        if check_types {
            if args.len() != f.param_types.len() {
                return Err(self.error(format!("Function called with {} argument(s), expected {}", args.len(), f.param_types.len())));
            }
            for (i, (t, a)) in f.param_types.iter().zip(&args).enumerate() {
                if let Some(t) = t
                    && !t.matches(a)
                {
                    return Err(self.error(format!("Argument {} ({}) should be {}, was {}", i + 1, f.param_names[i], t.name(), a.type_description())));
                }
            }
        }
        let call_pos = self.pos;
        let base = self.stack.len();
        self.stack.resize(base + f.frame_size as usize, Value::Undefined);
        for (i, a) in args.into_iter().enumerate() {
            self.stack[base + i] = a;
        }
        for (slot, v) in f.capture_slots.iter().zip(captures) {
            self.stack[base + *slot as usize] = v.clone();
        }
        let saved_base = std::mem::replace(&mut self.base, base);
        let saved_module = std::mem::replace(&mut self.module, f.module.clone());
        self.frames.push(TraceFrame { function: f.name.clone(), module: saved_module.clone(), pos: call_pos });
        let r = self.run_body(f);
        self.frames.pop();
        self.stack.truncate(base);
        self.base = saved_base;
        self.module = saved_module;
        self.pos = call_pos;
        r
    }

    fn run_body(&mut self, f: &CompiledFn) -> R<Value> {
        if let Some(p) = &f.precondition {
            match self.exec(p)? {
                Flow::False => {
                    // Live: "Precondition of guarded failed (x > 0)", at the
                    // failing statement; the text is the statement's source or
                    // its annotation's "Message".
                    let (pos, text) = self.failed_check.take().unwrap_or((p.pos, "".into()));
                    self.pos = pos;
                    let name = if f.name.is_empty() { "function".to_string() } else { display_fn_name(&f.name) };
                    return Err(self.error(format!("Precondition of {name} failed ({text})")));
                }
                Flow::Normal => {}
                _ => return Err(self.error("Invalid statement in precondition")),
            }
        }
        let predicate = f.kind == FnKind::Predicate;
        let v = match &f.body {
            FnBody::Expr(e) => self.eval(e)?,
            FnBody::Block(stmts) => match self.exec_block(stmts)? {
                Flow::Normal => Value::bool(predicate).or_undefined(!predicate),
                Flow::Return(v) => v,
                Flow::False => Value::bool(false),
                Flow::Break | Flow::Continue => return Err(self.error("break or continue outside of a loop")),
            },
        };
        if let Some(t) = f.returns
            && !t.matches(&v)
        {
            return Err(self.error(format!("Return value should be {}, was {}", t.name(), v.type_description())));
        }
        Ok(v)
    }

    // ------------------------------------------------------------ statements

    fn exec_block(&mut self, stmts: &[St]) -> R<Flow> {
        for s in stmts {
            match self.exec(s)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    /// A condition; `at` locates its error (Onshape: the statement).
    fn cond(&mut self, e: &Ex, what: &str, at: Pos) -> R<bool> {
        match self.eval(e)? {
            Value::Bool(b, _) => Ok(b),
            other => {
                self.pos = at;
                Err(self.error(format!("{what} must be boolean, value is {}", other.error_type())))
            }
        }
    }

    fn exec(&mut self, s: &St) -> R<Flow> {
        self.tick()?;
        match &s.k {
            StK::Let { slot, ty, init } => {
                let v = match init {
                    Some(e) => self.eval(e)?,
                    None => Value::Undefined,
                };
                if let Some(t) = ty
                    && !t.matches(&v)
                {
                    self.pos = s.pos;
                    return Err(self.error(format!("Value assigned to variable should be {}, was {}", t.name(), v.type_description())));
                }
                let b = self.base;
                self.stack[b + *slot as usize] = v;
                Ok(Flow::Normal)
            }
            StK::Assign { slot, path, op, value } => {
                let mut keys = Vec::with_capacity(path.len());
                for a in path {
                    keys.push(match a {
                        Acc::Member(k) => AccVal::Key(k.clone()),
                        Acc::Index(e) => AccVal::Key(self.eval(e)?),
                        Acc::Box => AccVal::Box,
                    });
                }
                let rhs = self.eval(value)?;
                self.pos = s.pos;
                let idx = self.base + *slot as usize;
                let mut root = std::mem::take(&mut self.stack[idx]);
                let mut rhs = Some(rhs);
                let r = self.store(&mut root, &keys, &mut |me: &mut Interp, target: &mut Value| {
                    let rhs = rhs.take().unwrap_or_default();
                    *target = match op {
                        None => rhs,
                        Some((o, ov)) => {
                            let old = std::mem::take(target);
                            me.binop(*o, old, rhs, ov.as_ref())?
                        }
                    };
                    Ok(())
                });
                self.stack[idx] = root;
                r.map(|_| Flow::Normal)
            }
            StK::Expr(e) => {
                self.eval(e)?;
                Ok(Flow::Normal)
            }
            StK::Check(e, text) => match self.eval(e)? {
                Value::Bool(true, _) => Ok(Flow::Normal),
                Value::Bool(false, _) => {
                    self.failed_check = Some((s.pos, text.clone()));
                    Ok(Flow::False)
                }
                other => {
                    self.pos = s.pos;
                    Err(self.error(format!("Precondition or predicate statement must evaluate to boolean, value is {}", other.error_type())))
                }
            },
            StK::Block(b) => self.exec_block(b),
            StK::If(c, t, o) => {
                if self.cond(c, "If statement condition", s.pos)? {
                    self.exec(t)
                } else if let Some(o) = o {
                    self.exec(o)
                } else {
                    Ok(Flow::Normal)
                }
            }
            StK::While(c, body) => {
                while self.cond(c, "While loop condition", s.pos)? {
                    match self.exec(body)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            StK::For { init, cond, step, body } => {
                if let Some(i) = init {
                    self.exec(i)?;
                }
                loop {
                    if let Some(c) = cond
                        && !self.cond(c, "For loop condition", s.pos)?
                    {
                        break;
                    }
                    match self.exec(body)? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other),
                    }
                    if let Some(st) = step {
                        self.exec(st)?;
                    }
                }
                Ok(Flow::Normal)
            }
            StK::ForIn { key, val, iter, body } => {
                let container = self.eval(iter)?;
                let b = self.base;
                match container {
                    Value::Array(a, _) => {
                        for (i, x) in a.iter().enumerate() {
                            if let Some(k) = key {
                                self.stack[b + *k as usize] = Value::num(i as f64);
                            }
                            self.stack[b + *val as usize] = x.clone();
                            match self.exec(body)? {
                                Flow::Normal | Flow::Continue => {}
                                Flow::Break => break,
                                other => return Ok(other),
                            }
                        }
                    }
                    Value::Map(m, _) => {
                        for (k, x) in m.iter() {
                            match key {
                                Some(ks) => {
                                    self.stack[b + *ks as usize] = k.clone();
                                    self.stack[b + *val as usize] = x.clone();
                                }
                                None => {
                                    let mut e = Map::new();
                                    e.insert(Value::str("key"), k.clone());
                                    e.insert(Value::str("value"), x.clone());
                                    self.stack[b + *val as usize] = Value::map(e);
                                }
                            }
                            match self.exec(body)? {
                                Flow::Normal | Flow::Continue => {}
                                Flow::Break => break,
                                other => return Ok(other),
                            }
                        }
                    }
                    other => {
                        self.pos = s.pos;
                        return Err(self.error(format!("Iterator loop requires map or array, value is {}", other.error_type())));
                    }
                }
                Ok(Flow::Normal)
            }
            StK::Return(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => Value::Undefined,
                };
                // A wrong return type is reported at the return statement.
                self.pos = s.pos;
                Ok(Flow::Return(v))
            }
            StK::Break => Ok(Flow::Break),
            StK::Continue => Ok(Flow::Continue),
            StK::Throw(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => Value::Undefined,
                };
                self.pos = s.pos;
                Err(self.throw(v))
            }
            StK::Try { silent, body, catch } => {
                if *silent {
                    self.silent += 1;
                }
                let depth = self.frames.len();
                let base = self.base;
                let module = self.module.clone();
                let r = self.exec_block(body);
                if *silent {
                    self.silent -= 1;
                }
                match r {
                    Ok(flow) => Ok(flow),
                    Err(e) => {
                        // Unwind to this frame.
                        self.frames.truncate(depth);
                        self.base = base;
                        self.module = module;
                        match catch {
                            Some((slot, stmts)) => {
                                if let Some(sl) = slot {
                                    self.stack[base + *sl as usize] = if e.internal { Value::str("Execution error") } else { e.value };
                                }
                                self.exec_block(stmts)
                            }
                            None => Ok(Flow::Normal),
                        }
                    }
                }
            }
            StK::Nop => Ok(Flow::Normal),
        }
    }

    /// Store into `target` along `path` (copy on write), calling `leaf` on
    /// the addressed value. Storing undefined in a map removes the key.
    fn store(&mut self, target: &mut Value, path: &[AccVal], leaf: &mut dyn FnMut(&mut Interp, &mut Value) -> R<()>) -> R<()> {
        let Some((first, rest)) = path.split_first() else {
            return leaf(self, target);
        };
        match first {
            AccVal::Key(k) => match target {
                Value::Map(m, _) => {
                    let mm = Arc::make_mut(m);
                    let mut child = mm.remove(k).unwrap_or_default();
                    let r = self.store(&mut child, rest, leaf);
                    if !child.is_undefined() {
                        mm.insert(k.clone(), child);
                    }
                    r
                }
                Value::Array(a, _) => {
                    let len = a.len();
                    let i = self.array_index(k, len)?;
                    let av = Arc::make_mut(a);
                    let mut child = std::mem::take(&mut av[i]);
                    let r = self.store(&mut child, rest, leaf);
                    av[i] = child;
                    r
                }
                other => Err(self.error(format!("Attempt to dereference non-container {}", crate::value::to_display_quoted(other)))),
            },
            AccVal::Box => match target {
                Value::Box(b, _) => {
                    let cell = b.clone();
                    let mut inner = std::mem::take(&mut *cell.value.lock());
                    let r = self.store(&mut inner, rest, leaf);
                    *cell.value.lock() = inner;
                    r
                }
                other => Err(self.error(format!("Box-style access requires box, value is {}", other.error_type()))),
            },
        }
    }

    fn array_index(&mut self, k: &Value, len: usize) -> R<usize> {
        match k {
            Value::Number(n, _) if *n >= 0.0 && n.fract() == 0.0 && (*n as usize) < len => Ok(*n as usize),
            Value::Number(n, _) if n.fract() == 0.0 => Err(self.error(format!("Array index {} out of bounds.", crate::value::format_number(*n)))),
            Value::Number(..) => Err(self.error("Array index is not an integer.")),
            other => Err(self.error(format!("Array index must be number, value is {}", other.error_type()))),
        }
    }

    // ------------------------------------------------------------ expressions

    pub fn eval(&mut self, e: &Ex) -> R<Value> {
        match &e.k {
            ExK::Lit(v) => Ok(v.clone()),
            ExK::Local(s) => Ok(self.stack[self.base + *s as usize].clone()),
            ExK::Global(c) => {
                self.pos = e.pos;
                self.global(c)
            }
            ExK::FnValue(set) => Ok(Value::Function(Shared::new(Func::Overloads(set.clone())))),
            ExK::Array(items) => {
                let mut v = Vec::with_capacity(items.len());
                for x in items {
                    v.push(self.eval(x)?);
                }
                Ok(Value::array(v))
            }
            ExK::Map(entries) => {
                let mut m = Map::new();
                for (k, x) in entries {
                    let k = self.eval(k)?;
                    let x = self.eval(x)?;
                    if x.is_undefined() {
                        m.remove(&k);
                    } else {
                        m.insert(k, x);
                    }
                }
                Ok(Value::map(m))
            }
            ExK::Neg(x, ov) => {
                let v = self.eval(x)?;
                self.pos = e.pos;
                if let Value::Number(n, Tag::NONE) = v {
                    return Ok(Value::num(-n));
                }
                let _ = ov;
                if !v.tag().is_none()
                    && let Some(set) = crate::compile::operator_overloads("operator-")
                    && let Some(r) = self.try_overload(&set, vec![v.clone()])?
                {
                    return Ok(r);
                }
                match v {
                    Value::Number(n, _) => Ok(Value::num(-n)),
                    _ => Err(self.error("Operand for '-' was not a number.")),
                }
            }
            ExK::Not(x) => match self.eval(x)? {
                Value::Bool(b, _) => Ok(Value::bool(!b)),
                _ => {
                    self.pos = e.pos;
                    Err(self.error("Operand for '!' was not a boolean."))
                }
            },
            ExK::Bin(op, a, b, ov) => {
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                self.pos = e.pos;
                self.binop(*op, va, vb, ov.as_ref())
            }
            ExK::And(a, b) => {
                if !self.cond(a, "Operand of &&", a.pos)? {
                    return Ok(Value::bool(false));
                }
                Ok(Value::bool(self.cond(b, "Operand of &&", b.pos)?))
            }
            ExK::Or(a, b) => {
                if self.cond(a, "Operand of ||", a.pos)? {
                    return Ok(Value::bool(true));
                }
                Ok(Value::bool(self.cond(b, "Operand of ||", b.pos)?))
            }
            ExK::Coalesce(a, b) => {
                let v = self.eval(a)?;
                if v.is_undefined() { self.eval(b) } else { Ok(v) }
            }
            ExK::Cond(c, a, b) => {
                if self.cond(c, "First operand of logical or conditional operator", c.pos)? {
                    self.eval(a)
                } else {
                    self.eval(b)
                }
            }
            ExK::Is(x, t) => {
                let v = self.eval(x)?;
                Ok(Value::bool(t.matches(&v)))
            }
            ExK::As(x, t) => {
                let v = self.eval(x)?;
                self.pos = e.pos;
                self.as_type(v, *t)
            }
            ExK::Call(callee, args) => {
                // Onshape locates errors in a call's arguments that have no
                // position of their own (undefined functions) at the call.
                if !matches!(callee, Callee::Missing(..)) {
                    self.pos = e.pos;
                }
                let f = match callee {
                    Callee::Value(fx) => Some(self.eval(fx)?),
                    _ => None,
                };
                let mut a = Vec::with_capacity(args.len());
                for x in args {
                    a.push(self.eval(x)?);
                }
                if !matches!(callee, Callee::Missing(..)) {
                    self.pos = e.pos;
                }
                match callee {
                    Callee::Overloads(set) => self.call_set(set, a),
                    Callee::Value(_) => self.call_value(&f.unwrap_or_default(), a),
                    Callee::Native(n, name) => {
                        let saved = std::mem::replace(&mut self.native_name, name.clone());
                        let r = n(self, a);
                        self.native_name = saved;
                        r
                    }
                    Callee::MissingNative(name) => Err(self.error(format!("@{name} is not supported"))),
                    Callee::Missing(name, n) => Err(self.error(format!("Function {name} with {n} argument(s) not found"))),
                }
            }
            ExK::Index(b, i, safe) => {
                let base = self.eval(b)?;
                if *safe && base.is_undefined() {
                    return Ok(Value::Undefined);
                }
                let idx = self.eval(i)?;
                self.pos = e.pos;
                self.index(base, &idx)
            }
            ExK::Member(b, k, safe) => {
                let base = self.eval(b)?;
                match base {
                    Value::Map(m, _) => Ok(m.get(k).cloned().unwrap_or_default()),
                    Value::Undefined if *safe => Ok(Value::Undefined),
                    other => {
                        self.pos = e.pos;
                        Err(self.error(format!("Attempt to dereference non-container {}", crate::value::to_display_quoted(&other))))
                    }
                }
            }
            ExK::BoxGet(b, safe) => match self.eval(b)? {
                Value::Box(bx, _) => Ok(bx.value.lock().clone()),
                Value::Undefined if *safe => Ok(Value::Undefined),
                other => {
                    self.pos = e.pos;
                    Err(self.error(format!("Box-style access requires box, value is {}", other.error_type())))
                }
            },
            ExK::Lambda(def, outer) => {
                let captures = outer.iter().map(|s| self.stack[self.base + *s as usize].clone()).collect();
                Ok(Value::Function(Shared::new(Func::Closure { def: def.clone(), captures })))
            }
            ExK::NewBox(x) => {
                let v = self.eval(x)?;
                Ok(Value::new_box(v))
            }
            ExK::Try(silent, x) => {
                if *silent {
                    self.silent += 1;
                }
                let depth = self.frames.len();
                let (base, module) = (self.base, self.module.clone());
                let r = self.eval(x);
                if *silent {
                    self.silent -= 1;
                }
                match r {
                    Ok(v) => Ok(v),
                    Err(_) => {
                        self.frames.truncate(depth);
                        self.base = base;
                        self.module = module;
                        Ok(Value::Undefined)
                    }
                }
            }
            ExK::Switch(subject, entries) => {
                let s = self.eval(subject)?;
                for (k, v) in entries {
                    if self.eval(k)? == s {
                        return self.eval(v);
                    }
                }
                Ok(Value::Undefined)
            }
        }
    }

    pub fn index(&mut self, base: Value, idx: &Value) -> R<Value> {
        match base {
            Value::Array(a, _) => {
                let i = self.array_index(idx, a.len())?;
                Ok(a[i].clone())
            }
            Value::Map(m, _) => Ok(m.get(idx).cloned().unwrap_or_default()),
            other => Err(self.error(format!("Attempt to dereference non-container {}", crate::value::to_display_quoted(&other)))),
        }
    }

    /// `v as t`.
    pub fn as_type(&mut self, v: Value, t: TypeRef) -> R<Value> {
        match t {
            TypeRef::Std(st) => {
                if v.std_type() != st {
                    return Err(self.error(format!("Cannot convert a value of type {} to {}", v.type_description(), st.name())));
                }
                Ok(v.with_tag(Tag::NONE))
            }
            TypeRef::Tag(tag) => {
                let info = type_info(tag);
                if info.is_enum {
                    // Live: 'Type error: value "FOO" cannot be enum Shade'.
                    let Some(s) = v.as_str() else {
                        return Err(self.error(format!("Type error: value {} cannot be enum {}", crate::value::to_display_quoted(&v), info.name)));
                    };
                    return match info.ordinal(&s) {
                        Some(o) => Ok(Value::Enum(tag, o)),
                        None => Err(self.error(format!("Type error: value {} cannot be enum {}", crate::value::to_display_quoted(&v), info.name))),
                    };
                }
                match v.std_type() {
                    // std tags boxes (`new box(...) as Instantiator`).
                    StdType::Function => Err(self.error(format!("Cannot convert a value of type {} to {}", v.type_description(), info.name))),
                    _ => Ok(v.with_tag(tag)),
                }
            }
        }
    }

    /// Call the best overload of an operator if one matches; `None` when no
    /// overload accepts the operands (the built-in operator applies).
    fn try_overload(&mut self, set: &Arc<OverloadSet>, args: Vec<Value>) -> R<Option<Value>> {
        if !set.fns.iter().any(|f| f.arity() == args.len() && Self::satisfies(&f.param_types, &args)) {
            return Ok(None);
        }
        self.call_set(set, args).map(Some)
    }

    /// A binary operator, with operator overloads for tagged operands.
    pub fn binop(&mut self, op: BinOp, a: Value, b: Value, _ov: Option<&Arc<OverloadSet>>) -> R<Value> {
        if let (Value::Number(x, Tag::NONE), Value::Number(y, Tag::NONE)) = (&a, &b) {
            return self.num_op(op, *x, *y);
        }
        let name = match op {
            BinOp::Add => Some("operator+"),
            BinOp::Sub => Some("operator-"),
            BinOp::Mul => Some("operator*"),
            BinOp::Div => Some("operator/"),
            BinOp::Mod => Some("operator%"),
            BinOp::Pow => Some("operator^"),
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => Some("operator<"),
            _ => None,
        };
        let global = if !a.tag().is_none() || !b.tag().is_none() { name.and_then(crate::compile::operator_overloads) } else { None };
        if let Some(set) = global.as_ref() {
            match op {
                BinOp::Lt => {
                    if let Some(r) = self.try_overload(set, vec![a.clone(), b.clone()])? {
                        return Ok(r);
                    }
                }
                BinOp::Gt => {
                    if let Some(r) = self.try_overload(set, vec![b.clone(), a.clone()])? {
                        return Ok(r);
                    }
                }
                BinOp::Le | BinOp::Ge => {
                    let args = if op == BinOp::Le { vec![b.clone(), a.clone()] } else { vec![a.clone(), b.clone()] };
                    if let Some(r) = self.try_overload(set, args)? {
                        return match r {
                            Value::Bool(x, _) => Ok(Value::bool(!x)),
                            other => Err(self.error(format!("operator< must return a boolean, returned {}", other.type_description()))),
                        };
                    }
                }
                _ => {
                    if let Some(r) = self.try_overload(set, vec![a.clone(), b.clone()])? {
                        return Ok(r);
                    }
                }
            }
        }
        match op {
            BinOp::Eq => return Ok(Value::bool(a == b)),
            BinOp::Ne => return Ok(Value::bool(a != b)),
            BinOp::Cat => {
                let mut s = to_display(&a);
                s.push_str(&to_display(&b));
                return Ok(Value::str(s));
            }
            _ => {}
        }
        match (&a, &b) {
            (Value::Number(x, _), Value::Number(y, _)) => self.num_op(op, *x, *y),
            (Value::Enum(ta, x), Value::Enum(tb, y)) if ta == tb && matches!(op, BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge) => Ok(Value::bool(match op {
                BinOp::Lt => x < y,
                BinOp::Gt => x > y,
                BinOp::Le => x <= y,
                _ => x >= y,
            })),
            _ => Err(self.error(format!("Can not {} {} and {}.", op.verb(), a.error_type(), b.error_type()))),
        }
    }

    fn num_op(&mut self, op: BinOp, x: f64, y: f64) -> R<Value> {
        let r = match op {
            BinOp::Add => x + y,
            BinOp::Sub => x - y,
            BinOp::Mul => x * y,
            BinOp::Div => x / y,
            BinOp::Mod => {
                // The sign of the result is the sign of the second operand.
                let m = x % y;
                if m != 0.0 && (m < 0.0) != (y < 0.0) { m + y } else { m }
            }
            BinOp::Pow => x.powf(y),
            BinOp::Lt => return Ok(Value::bool(x < y)),
            BinOp::Gt => return Ok(Value::bool(x > y)),
            BinOp::Le => return Ok(Value::bool(x <= y)),
            BinOp::Ge => return Ok(Value::bool(x >= y)),
            BinOp::Eq => return Ok(Value::bool(x == y)),
            BinOp::Ne => return Ok(Value::bool(x != y)),
            BinOp::Cat => return Ok(Value::str(format!("{}{}", crate::value::format_number(x), crate::value::format_number(y)))),
            BinOp::And | BinOp::Or | BinOp::Coalesce => unreachable!("short-circuit operators are not numeric"),
        };
        if r.is_nan() {
            return Err(self.error("Invalid number (NaN)"));
        }
        Ok(Value::num(r))
    }
}

enum AccVal {
    Key(Value),
    Box,
}

trait OrUndefined {
    fn or_undefined(self, undefined: bool) -> Value;
}

impl OrUndefined for Value {
    fn or_undefined(self, undefined: bool) -> Value {
        if undefined { Value::Undefined } else { self }
    }
}

/// `operator+` -> `operator +`; plain names unchanged.
fn display_fn_name(name: &str) -> String {
    name.to_string()
}

/// A tiny inline vector for overload candidates.
mod smallvec_lite {
    pub struct Small<T> {
        items: Vec<T>,
    }
    impl<T> Small<T> {
        pub fn new() -> Self {
            Small { items: Vec::new() }
        }
        pub fn push(&mut self, t: T) {
            self.items.push(t)
        }
        pub fn len(&self) -> usize {
            self.items.len()
        }
        pub fn take(mut self, i: usize) -> T {
            self.items.swap_remove(i)
        }
        pub fn as_slice(&self) -> &[T] {
            &self.items
        }
    }
}
