//! The FeatureScript evaluator.
//!
//! An [`Interp`] runs compiled code ([`crate::ir`]) on a value stack (one
//! frame of slots per call). Exceptions are Rust errors carrying the thrown
//! FeatureScript value; every raised exception outside `try silent` is also
//! recorded as a notice, like Onshape's notices flyout.

use crate::ast::{BinOp, FnKind, Name};
use crate::ir::*;
use crate::lexer::Pos;
use crate::value::{Func, Map, Shared, Tag, Value, to_display, type_info};
use parking_lot::{ReentrantMutex, RwLock};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Instant;

#[path = "interp_closure.rs"]
pub(crate) mod closure;
pub use closure::{compact_default as compact_values, set_compact_default, set_tree_walker_default};

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

/// Host natives' fields read with compact-aware helpers ([`register_compact_fields`]).
static COMPACT_FIELDS: LazyLock<RwLock<HashMap<&'static str, &'static [&'static str]>>> = LazyLock::new(Default::default);

/// Host natives whose definition's `fields` (their map argument's) take compact values as they are: the native
/// reads them with compact-aware helpers. Their other arguments and fields are maps and arrays as ever.
pub fn register_compact_fields(list: &[(&'static str, &'static [&'static str])]) {
    let mut m = COMPACT_FIELDS.write();
    for (n, f) in list {
        m.insert(n, f);
    }
}

/// `v` without compact values but in `keep`'s fields (a map's).
fn materialized_but(v: Value, keep: &[&str]) -> Value {
    match v {
        Value::Map(m, t) if m.iter().any(|(k, x)| !k.as_str().is_some_and(|k| keep.contains(&&*k)) && x.has_compact()) => {
            let m = Arc::unwrap_or_clone(m).into_iter().map(|(k, x)| if k.as_str().is_some_and(|s| keep.contains(&&*s)) { (k, x) } else { (k, x.materialized()) }).collect();
            Value::Map(Arc::new(m), t)
        }
        Value::Map(..) => v,
        other => other.materialized(),
    }
}

/// This crate's natives that move elements without reading them (the host's own of these names excepted).
fn structural_native(name: &str) -> bool {
    const MOVERS: [&str; 11] = ["size", "resize", "subArray", "concatenateArrays", "indexOf", "keys", "values", "mergeMaps", "intersectMaps", "reverse", "range"];
    MOVERS.contains(&name) && !HOST_NATIVES.read().contains_key(name)
}

/// How a native takes its arguments ([`Interp::call_native`]): settled by the natives registered before compiling.
#[derive(Clone, Copy)]
pub(crate) enum NativeArgs {
    /// This crate's @size: a compact value's length without its array.
    Size,
    /// A host native's fields read compact ([`register_compact_fields`]).
    Keep(&'static [&'static str]),
    /// This crate's natives that move elements: the top level without compact values.
    Structural,
    /// Without compact values all the way down.
    Plain,
}

pub(crate) fn native_args(name: &str) -> NativeArgs {
    if name == "size" && structural_native(name) {
        return NativeArgs::Size;
    }
    if let Some(keep) = COMPACT_FIELDS.read().get(name).copied() {
        return NativeArgs::Keep(keep);
    }
    if structural_native(name) { NativeArgs::Structural } else { NativeArgs::Plain }
}

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
    Some(Value::Function(Shared::new(Func::Native { name: name.into(), f, bound }), Tag::NONE))
}

pub fn native(name: &str) -> Option<NativeFn> {
    if let Some(f) = HOST_NATIVES.read().get(name) {
        return Some(*f);
    }
    crate::natives::core(name)
}

/// Natives whose results older standard libraries expect in another form
/// (std 1963 converted raw maps with `planeFromBuiltin` & co.): name →
/// (first version with the current form, the old form).
static LEGACY_NATIVES: LazyLock<RwLock<HashMap<&'static str, (u32, NativeFn)>>> = LazyLock::new(Default::default);

/// Register natives as modules older than `since` call them (Onshape's
/// builtins answer each library version in its own form).
pub fn register_legacy_natives(since: u32, list: &[(&'static str, NativeFn)]) {
    let mut m = LEGACY_NATIVES.write();
    for (n, f) in list {
        m.insert(n, (since, *f));
    }
}

/// The native `name` as a module with `FeatureScript version;` calls it.
pub fn native_at(name: &str, version: Option<u32>) -> Option<NativeFn> {
    if let Some(v) = version
        && let Some((since, f)) = LEGACY_NATIVES.read().get(name)
        && v < *since
    {
        return Some(*f);
    }
    native(name)
}

// ---------------------------------------------------------------- interpreter

pub(crate) enum Flow {
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
    /// The running function's module's `FeatureScript <version>;` ([`Interp::module_version`]).
    module_version: Option<u32>,
    /// Position of the expression being evaluated (natives read it).
    pub pos: Pos,
    steps: u64,
    deadline: Option<Instant>,
    /// The regeneration's cancel (a newer edit made it stale, or the user's cancel): checked with the deadline.
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
    silent: u32,
    pub notices: Vec<Notice>,
    /// `print` / `println` output.
    pub output: String,
    /// Host state for natives (the modeling session).
    pub host: Option<Box<dyn std::any::Any>>,
    /// Overload sets merged with the loaded modules' exported overloads,
    /// by set address: (registry generation, candidates).
    merged: HashMap<usize, (u64, Candidates)>,
    /// Calls resolved by overload set and argument types ([`Interp::resolve_cached`]), and the operators' overload
    /// sets: both found again when the registries change ([`crate::compile::overloads_generation`]).
    resolved: HashMap<ResolveKey, Resolved, ahash::RandomState>,
    operator_sets: [Option<Option<Arc<OverloadSet>>>; OPERATORS.len()],
    overloads_generation: u64,
    /// The `@native` being called (its errors read "@evPlane: ...").
    pub native_name: Name,
    /// The predicate statement that evaluated to false: (position, text).
    failed_check: Option<(Pos, Arc<str>)>,
    /// Per-line timing (Feature Studio "Profile"), when enabled.
    pub profile: Option<LineProfile>,
    /// Cancelled or timed out: the frames' values left as the calls unwind ([`Interp::take_stopped_frames`]).
    stopped: Option<Vec<Vec<Value>>>,
    /// Run function bodies with the tree walker, not their closures ([`closure`]; NOSHAPE_FS_TREE=1: the reference
    /// of the differential test).
    pub tree_walker: bool,
    /// std's operators answer compact values (`Value::Quantity`, `Value::Vector`; NOSHAPE_FS_PLAIN=1: never, the
    /// reference of the differential test).
    pub compact: bool,
}

/// Time spent running each source line (module, 1-based line), including
/// what its statements call; a line is timed once while it is active
/// (recursion is not counted twice).
#[derive(Debug, Default)]
pub struct LineProfile {
    pub times: HashMap<(Arc<str>, u32), std::time::Duration>,
    active: HashMap<(Arc<str>, u32), u32>,
    /// Functions (at their declaration's line) and natives ("@name") timed instead of lines: the closures run.
    pub functions: bool,
}

impl LineProfile {
    pub fn functions() -> Self {
        Self { functions: true, ..Default::default() }
    }
}

/// The overloads a call chooses from.
type Candidates = Arc<Vec<Arc<FnDecl>>>;

/// The operators with overloads, as [`Interp::operator_set`] indexes them.
const OPERATORS: [&str; 7] = ["operator+", "operator-", "operator*", "operator/", "operator%", "operator^", "operator<"];

/// A call's overload set (its address), whether it is an operator's try ([`Interp::try_overload`]), and its
/// arguments' types ([`signature`]).
type ResolveKey = (usize, bool, [u32; 6], u8);

/// A compiled call site's last std operator: what [`Interp::resolved`] answered for these argument types in this
/// generation of the overload registries (the closures check both before using it).
pub(crate) struct Site {
    pub generation: u64,
    pub sig: ([u32; 6], u8),
    pub decl: Arc<FnDecl>,
    /// The std operator it is (none: a function to run).
    pub op: Option<crate::std_ops::Op>,
    /// Its arithmetic, a units.fs or vector.fs operator's.
    pub arith: Option<crate::std_ops::Arith>,
    /// Its arguments' [`quick_sig`] when it decides `sig` (else 0).
    pub kinds: u16,
}

/// Arguments' kinds when they are untagged numbers or compact values (else 0, as for none or over 8): their
/// signature is their kinds'.
pub(crate) fn quick_sig<'a>(args: impl IntoIterator<Item = &'a Value>) -> u16 {
    let mut sig = 0u16;
    for (n, v) in args.into_iter().enumerate() {
        let kind = match v {
            Value::Number(_, Tag::NONE) => 1,
            Value::Quantity(..) => 2,
            Value::Vector(..) => 3,
            _ => return 0,
        };
        if n == 8 {
            return 0;
        }
        sig = sig << 2 | kind;
    }
    sig
}

/// A resolved call: the overload (none: an operator's overloads don't apply) and the std operator it is. The set is
/// kept so its address isn't reused while it keys this.
struct Resolved {
    _set: Arc<OverloadSet>,
    decl: Option<Arc<FnDecl>>,
    op: Option<crate::std_ops::Op>,
}

/// What overload resolution reads of the arguments ([`TypeRef::matches`]): each one's standard type, tag and whether
/// it is an enum member; `None` past six arguments or tags.
pub(crate) fn signature(args: &[Value]) -> Option<([u32; 6], u8)> {
    signature_of(args.iter())
}

/// [`signature`] of values held elsewhere.
pub(crate) fn signature_of<'a>(args: impl ExactSizeIterator<Item = &'a Value>) -> Option<([u32; 6], u8)> {
    let n = args.len();
    if n > 6 {
        return None;
    }
    let mut sig = [0u32; 6];
    for (s, a) in sig.iter_mut().zip(args) {
        let t = a.tag().0;
        if t >= 1 << 27 {
            return None;
        }
        *s = u32::from(matches!(a, Value::Enum(..))) << 31 | (a.std_type() as u32) << 27 | t;
    }
    Some((sig, n as u8))
}

impl Default for Interp {
    fn default() -> Self {
        Self::new()
    }
}

impl Interp {
    /// The `FeatureScript <version>;` of the module whose function is running (a native's caller).
    pub fn module_version(&self) -> Option<u32> {
        self.module_version
    }

    pub fn new() -> Self {
        Interp {
            stack: Vec::with_capacity(1024),
            base: 0,
            frames: Vec::new(),
            module: "".into(),
            module_version: None,
            pos: Pos::default(),
            steps: 0,
            deadline: None,
            cancel: None,
            silent: 0,
            notices: Vec::new(),
            output: String::new(),
            host: None,
            merged: HashMap::new(),
            resolved: HashMap::default(),
            operator_sets: Default::default(),
            overloads_generation: crate::compile::overloads_generation(),
            native_name: "".into(),
            failed_check: None,
            profile: None,
            stopped: None,
            tree_walker: closure::tree_walker_default(),
            compact: closure::compact_default(),
        }
    }

    /// Abort evaluation with an error after `deadline`.
    pub fn set_deadline(&mut self, deadline: Option<Instant>) {
        self.deadline = deadline;
    }

    /// Abort evaluation ("Regeneration cancelled") once `cancel` says so: looked at as often as the deadline
    /// (every 16k steps, a few ms of script; natives in long loops call [`Interp::check_deadline`]).
    pub fn set_cancel(&mut self, cancel: Option<Box<dyn Fn() -> bool + Send>>) {
        self.cancel = cancel;
    }

    /// A native's error after the cancel (a kernel operation it stopped): the run is stopping, even where the script
    /// catches the error.
    fn native_stopped(&mut self, r: &R<Value>) {
        if r.is_err() && self.stopped.is_none() && self.cancel.as_ref().is_some_and(|c| c()) {
            self.stopped = Some(Vec::new());
        }
    }

    fn cancelled(&mut self) -> R<()> {
        if self.cancel.as_ref().is_some_and(|c| c()) {
            self.cancel = None;
            self.stopped.get_or_insert_default();
            return Err(self.error("Regeneration cancelled"));
        }
        Ok(())
    }

    /// A cancel or the deadline is unwinding the run (also a native's error after the cancel: a kernel operation
    /// stopped by it): no `try` catches it, else the feature would go on down its fallback and finish.
    fn stopping(&mut self) -> bool {
        if self.stopped.is_none() && self.cancel.as_ref().is_some_and(|c| c()) {
            self.cancel = None;
            self.stopped = Some(Vec::new());
        }
        self.stopped.is_some()
    }

    /// The values of the frames a cancel or the deadline unwound, for the caller to free off its thread (a
    /// displacement map's million-point arrays took 0.3-0.8 s to free after the cancel).
    pub fn take_stopped_frames(&mut self) -> Vec<Vec<Value>> {
        self.stopped.take().unwrap_or_default()
    }

    /// A returning frame's values dropped, a big array or map only it held freed on a thread of its own (a displacement
    /// map's million-point table: 0.5 s at the feature's end, or after a cancel the run waited for).
    fn drop_frame(&mut self, base: usize) {
        const BIG: usize = 4096;
        static DROPPER: LazyLock<Option<std::sync::mpsc::Sender<Value>>> = LazyLock::new(|| {
            let (tx, rx) = std::sync::mpsc::channel::<Value>();
            std::thread::Builder::new().name("fs-free".into()).spawn(move || rx.into_iter().for_each(drop)).ok().map(|_| tx)
        });
        // (Read in place: a frame's numbers aren't moved to be dropped.)
        for v in &mut self.stack[base..] {
            let big = match v {
                Value::Array(a, _) => a.len() >= BIG && Arc::strong_count(a) == 1,
                Value::Map(m, _) => m.len() >= BIG && Arc::strong_count(m) == 1,
                _ => false,
            };
            if big && let Some(tx) = DROPPER.as_ref() {
                // (Sent, or dropped here should the thread be gone.)
                let _ = tx.send(std::mem::take(v));
            }
        }
        self.stack.truncate(base);
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
        // (A thrown value reaches the host: in its usual form.)
        let value = value.materialized();
        let e = Box::new(Exception { value, module: self.module.clone(), pos: self.pos, trace: self.frames.clone(), internal: false });
        self.note_exception(&e);
        e
    }

    /// Raise `value` with `notice` as the notice text.
    pub fn raise_noted(&mut self, value: Value, notice: String) -> Box<Exception> {
        let value = value.materialized();
        let e = Box::new(Exception { value, module: self.module.clone(), pos: self.pos, trace: self.frames.clone(), internal: false });
        if self.silent == 0 && self.notices.len() < MAX_NOTICES {
            self.notices.push(Notice { kind: NoticeKind::Error, message: notice, module: e.module.clone(), pos: e.pos, trace: e.trace.clone() });
        }
        e
    }

    /// `throw v`: the notice reads "throw custom", "throw my message"
    /// (a regenError's custom message), "throw { "a" : 1 }" (live).
    pub fn throw(&mut self, value: Value) -> Box<Exception> {
        let value = value.materialized();
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

    /// The deadline's error once it has passed: for native functions that
    /// loop long between the interpreter's own checks (std's collision
    /// detection over many faces, qa MIRROR-HANG-402).
    pub fn check_deadline(&mut self) -> R<()> {
        self.cancelled()?;
        if let Some(d) = self.deadline
            && Instant::now() > d
        {
            self.deadline = None;
            self.stopped.get_or_insert_default();
            return Err(self.error("Regeneration timed out"));
        }
        Ok(())
    }

    fn tick(&mut self) -> R<()> {
        self.steps += 1;
        if self.steps & 0x3FFF == 0 {
            self.cancelled()?;
        }
        if self.steps & 0xFFFF == 0
            && let Some(d) = self.deadline
            && Instant::now() > d
        {
            self.deadline = None;
            self.stopped.get_or_insert_default();
            return Err(self.error("Regeneration timed out"));
        }
        Ok(())
    }

    // ------------------------------------------------------------ globals

    /// Value of a top-level constant (initialized on first use).
    pub(crate) fn global_in(&mut self, cell: &Arc<GlobalCell>) -> R<Value> {
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
        let mut r = self.eval_in(ex);
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
    pub(crate) fn call_value_in(&mut self, f: &Value, args: Vec<Value>) -> R<Value> {
        match f {
            Value::Function(func, _) => match &**func {
                Func::Closure { def, captures } => {
                    let (def, captures) = (def.clone(), captures.clone());
                    self.invoke_in(&def, args, &captures, true)
                }
                Func::Overloads(set) => {
                    let set = set.clone();
                    self.call_set_in(&set, args)
                }
                Func::Native { name, f, bound } => {
                    let (f, mut all, name) = (*f, bound.clone(), name.clone());
                    all.extend(args);
                    self.call_native(f, &name, all)
                }
            },
            other => Err(self.error(format!("Called value must be function, value is {}", other.error_type()))),
        }
    }

    /// Call the best overload of `set` for `args`.
    pub(crate) fn call_set_in(&mut self, set: &Arc<OverloadSet>, mut args: Vec<Value>) -> R<Value> {
        let (decl, op) = self.resolve_cached(set, &args)?;
        if let Some(op) = op
            && let Some(v) = crate::std_ops::apply(op, &decl, &mut args, self.compact)
        {
            return Ok(v);
        }
        let Some(c) = decl.compiled.get() else {
            return Err(self.error(format!("Function {} is not compiled", set.name)));
        };
        self.invoke_in(c, args, &[], false)
    }

    /// Call the best overload of `set` for the arguments on the stack from `base`.
    fn call_set_at(&mut self, set: &Arc<OverloadSet>, base: usize) -> R<Value> {
        // (A std function answered from the cache's own entry: no copy of its declaration.)
        self.check_generation();
        if let Some((sig, n)) = signature(&self.stack[base..])
            && let Some(r) = self.resolved.get(&(Arc::as_ptr(set) as usize, false, sig, n))
            && let (Some(d), Some(op)) = (&r.decl, r.op)
            && let Some(v) = crate::std_ops::apply(op, d, &mut self.stack[base..], self.compact)
        {
            self.stack.truncate(base);
            return Ok(v);
        }
        let (decl, op) = match self.resolve_cached_at(set, base) {
            Ok(found) => found,
            Err(e) => {
                self.discard_from(base);
                return Err(e);
            }
        };
        if let Some(op) = op
            && let Some(v) = crate::std_ops::apply(op, &decl, &mut self.stack[base..], self.compact)
        {
            self.stack.truncate(base);
            return Ok(v);
        }
        let Some(c) = decl.compiled.get() else {
            self.discard_from(base);
            return Err(self.error(format!("Function {} is not compiled", set.name)));
        };
        self.invoke_at(c, base)
    }

    /// [`Interp::resolve_cached`] for the arguments on the stack from `base` (left there).
    fn resolve_cached_at(&mut self, set: &Arc<OverloadSet>, base: usize) -> R<(Arc<FnDecl>, Option<crate::std_ops::Op>)> {
        self.check_generation();
        let key = signature(&self.stack[base..]).map(|(sig, n)| (Arc::as_ptr(set) as usize, false, sig, n));
        if let Some(r) = key.and_then(|k| self.resolved.get(&k))
            && let Some(d) = &r.decl
        {
            return Ok((d.clone(), r.op));
        }
        let args = self.stack.split_off(base);
        let r = self.resolve_cached(set, &args);
        self.stack.extend(args);
        r
    }

    /// `x = append(x, v)`, `x = concatenateArrays(x, y)` (std's) and `x = @resize(x, n, v)` on an array grow x in place:
    /// amortized O(1) an element as in Onshape, where a copy per call made array building quadratic (copied only when
    /// another value shares the array). Any other call runs as written. x is read after the other arguments: no
    /// expression changes a local. None: not such a statement (nothing evaluated).
    fn assign_grown(&mut self, slot: u32, callee: &Callee, args: &[Ex], pos: Pos) -> Option<R<Flow>> {
        let is_x = |e: &Ex| matches!(e.k, ExK::Local(s) if s == slot);
        let grows = match callee {
            Callee::Native(_, name) => &**name == "resize",
            Callee::Overloads(set) => matches!(&*set.name, "append" | "concatenateArrays"),
            _ => false,
        };
        if !grows || !args.first().is_some_and(is_x) {
            return None;
        }
        Some(self.call_grown(slot, callee, &args[1..], pos))
    }

    /// `x = append(x, y)` or `x = concatenateArrays(x, y)` with y evaluated: x grown in place when std's (the tree
    /// walker's and the closures' alike).
    fn grow_one(&mut self, slot: u32, set: &Arc<OverloadSet>, y: Value) -> R<Flow> {
        let idx = self.base + slot as usize;
        if self.stack[idx].is_compact() {
            self.stack[idx] = self.stack[idx].shallow().into_owned();
        }
        let pair = [self.stack[idx].clone(), y];
        let (d, _) = self.resolve_cached(set, &pair)?;
        if matches!(pair[0], Value::Array(..)) && &*d.module == "onshape/std/containers.fs" {
            let [x, y] = pair;
            // (x's copy gone first: the local's array is then the only one, grown in place.)
            drop(x);
            self.grow_slot(idx, &*d.name == "append", y);
            return Ok(Flow::Normal);
        }
        let Some(c) = d.compiled.get() else {
            return Err(self.error(format!("Function {} is not compiled", d.name)));
        };
        self.stack[idx] = self.invoke_in(c, pair.into(), &[], false)?;
        Ok(Flow::Normal)
    }

    /// std's append (`append`) or concatenateArrays on the array in stack slot `idx`, in place.
    pub(crate) fn grow_slot(&mut self, idx: usize, append: bool, y: Value) {
        if let Value::Array(arr, tag) = &mut self.stack[idx] {
            let v = Arc::make_mut(arr);
            if append {
                v.push(y);
            } else {
                if let Value::Array(b, _) = y.shallow().into_owned() {
                    v.extend(Arc::unwrap_or_clone(b));
                }
                // @concatenateArrays answers an untagged array.
                *tag = Tag::NONE;
            }
        }
    }

    /// Whether `set` resolved to std's append (`Some(true)`) or concatenateArrays (`Some(false)`) for arguments of
    /// `sig` ([`Interp::resolved`]), for a call site to keep.
    pub(crate) fn std_grow_site(&self, set: &Arc<OverloadSet>, sig: ([u32; 6], u8)) -> Option<bool> {
        let d = self.resolved.get(&(Arc::as_ptr(set) as usize, false, sig.0, sig.1))?.decl.as_ref()?;
        (&*d.module == "onshape/std/containers.fs").then(|| &*d.name == "append")
    }

    fn call_grown(&mut self, slot: u32, callee: &Callee, rest: &[Ex], pos: Pos) -> R<Flow> {
        enum Grow {
            Push,
            Extend,
            Resize(usize),
        }
        // `x = append(x, v)`, `x = concatenateArrays(x, y)`: the two arguments without a vector of them.
        if let ([y], Callee::Overloads(set)) = (rest, callee) {
            self.pos = pos;
            let y = self.eval_in(y)?;
            self.pos = pos;
            return self.grow_one(slot, set, y);
        }
        self.pos = pos;
        let mut a = Vec::with_capacity(rest.len() + 1);
        a.push(Value::Undefined);
        for x in rest {
            a.push(self.eval_in(x)?);
        }
        self.pos = pos;
        let idx = self.base + slot as usize;
        if self.stack[idx].is_compact() {
            self.stack[idx] = self.stack[idx].shallow().into_owned();
        }
        a[0] = self.stack[idx].clone();
        let decl = match callee {
            Callee::Overloads(set) => Some(self.resolve_cached(set, &a)?.0),
            _ => None,
        };
        let array = matches!(a[0], Value::Array(..));
        let grow = match &decl {
            Some(d) if array && a.len() == 2 && &*d.module == "onshape/std/containers.fs" => match &*d.name {
                "append" => Some(Grow::Push),
                _ => Some(Grow::Extend),
            },
            Some(_) => None,
            None => match a.get(1) {
                Some(Value::Number(n, _)) if array && *n >= 0.0 && n.fract() == 0.0 => Some(Grow::Resize(*n as usize)),
                _ => None,
            },
        };
        if let Some(grow) = grow {
            a[0] = Value::Undefined;
            if let Value::Array(arr, tag) = &mut self.stack[idx] {
                let v = Arc::make_mut(arr);
                match grow {
                    Grow::Push => v.push(a.pop().unwrap_or_default()),
                    Grow::Extend => {
                        if let Some(Value::Array(b, _)) = a.pop().map(|y| y.shallow().into_owned()) {
                            v.extend(Arc::unwrap_or_clone(b));
                        }
                        // @concatenateArrays answers an untagged array.
                        *tag = Tag::NONE;
                    }
                    Grow::Resize(n) => v.resize(n, a.get(2).cloned().unwrap_or_default()),
                }
            }
            return Ok(Flow::Normal);
        }
        let r = match (decl, callee) {
            (Some(d), _) => {
                let Some(c) = d.compiled.get() else {
                    return Err(self.error(format!("Function {} is not compiled", d.name)));
                };
                self.invoke_in(c, a, &[], false)?
            }
            (None, Callee::Native(n, name)) => self.call_native(*n, name, a)?,
            (None, _) => return Err(self.error("Not a call that grows an array")),
        };
        self.stack[idx] = r;
        Ok(Flow::Normal)
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

    /// The overload registries changed: what was resolved among them is found again.
    fn check_generation(&mut self) {
        let g = crate::compile::overloads_generation();
        if g != self.overloads_generation {
            self.overloads_generation = g;
            self.resolved.clear();
            self.operator_sets = Default::default();
        }
    }

    /// [`Interp::resolve`], remembered for the arguments' types (all it reads of them), with the std operator the
    /// overload is.
    fn resolve_cached(&mut self, set: &Arc<OverloadSet>, args: &[Value]) -> R<(Arc<FnDecl>, Option<crate::std_ops::Op>)> {
        self.check_generation();
        let key = signature(args).map(|(sig, n)| (Arc::as_ptr(set) as usize, false, sig, n));
        if let Some(r) = key.and_then(|k| self.resolved.get(&k))
            && let Some(d) = &r.decl
        {
            return Ok((d.clone(), r.op));
        }
        let decl = self.resolve(set, args)?;
        let op = crate::std_ops::classify(&decl);
        if let Some(k) = key {
            self.resolved.insert(k, Resolved { _set: set.clone(), decl: Some(decl.clone()), op });
        }
        Ok((decl, op))
    }

    /// The std operator the cache resolved for operator `i` (`call`: a function's call set instead) on arguments of
    /// `sig`, for a call site to keep ([`Site`]).
    pub(crate) fn std_site(&mut self, operator: Option<usize>, call: Option<&Arc<OverloadSet>>, sig: ([u32; 6], u8)) -> Option<Site> {
        let (ptr, try_) = match (operator, call) {
            (Some(i), _) => (Arc::as_ptr(self.operator_sets.get(i)?.as_ref()?.as_ref()?) as usize, true),
            (None, Some(set)) => (Arc::as_ptr(set) as usize, false),
            _ => return None,
        };
        let r = self.resolved.get(&(ptr, try_, sig.0, sig.1))?;
        // (An operator site keeps std's operators only: the others' bodies run through binop's rules.)
        if operator.is_some() && r.op.is_none() {
            return None;
        }
        let arith = match r.op {
            Some(crate::std_ops::Op::Units | crate::std_ops::Op::Vectors) => r.decl.as_ref().and_then(|d| crate::std_ops::arith(&d.name)),
            _ => None,
        };
        Some(Site { generation: self.overloads_generation, sig, decl: r.decl.clone()?, op: r.op, arith, kinds: 0 })
    }

    /// Every loaded overload of operator `OPERATORS[i]`.
    fn operator_set(&mut self, i: usize) -> Option<Arc<OverloadSet>> {
        self.check_generation();
        self.operator_sets[i].get_or_insert_with(|| crate::compile::operator_overloads(OPERATORS[i])).clone()
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
    pub(crate) fn invoke_in(&mut self, f: &CompiledFn, args: Vec<Value>, captures: &[Value], check_types: bool) -> R<Value> {
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
        let base = self.stack.len();
        self.stack.extend(args);
        self.enter(f, base, captures)
    }

    /// Run `f` on the arguments on the stack from `base` (a call's, evaluated there: no vector of them).
    pub(crate) fn invoke_at(&mut self, f: &CompiledFn, base: usize) -> R<Value> {
        if self.frames.len() >= MAX_DEPTH {
            self.discard_from(base);
            return Err(self.error("Maximum call stack depth exceeded."));
        }
        if let Err(e) = self.tick() {
            self.discard_from(base);
            return Err(e);
        }
        self.enter(f, base, &[])
    }

    /// The stack's values from `base` dropped (a call that didn't happen); once stopped, kept for the caller to free.
    fn discard_from(&mut self, base: usize) {
        match &mut self.stopped {
            Some(dead) => dead.push(self.stack.split_off(base)),
            None => self.stack.truncate(base),
        }
    }

    /// `f`'s frame from `base`, its arguments there.
    fn enter(&mut self, f: &CompiledFn, base: usize, captures: &[Value]) -> R<Value> {
        let call_pos = self.pos;
        let size = base + f.frame_size as usize;
        if self.stack.len() < size {
            self.stack.resize(size, Value::Undefined);
        }
        for (slot, v) in f.capture_slots.iter().zip(captures) {
            self.stack[base + *slot as usize] = v.clone();
        }
        let saved_base = std::mem::replace(&mut self.base, base);
        let saved_module = std::mem::replace(&mut self.module, f.module.clone());
        let saved_version = std::mem::replace(&mut self.module_version, f.version);
        // (The caller's module kept in its frame, taken back from it: each call pops its own.)
        self.frames.push(TraceFrame { function: f.name.clone(), module: saved_module, pos: call_pos });
        let r = match &self.profile {
            Some(p) if p.functions => self.timed((f.module.clone(), f.pos.line), |i| i.run_body(f)),
            _ => self.run_body(f),
        };
        let saved_module = self.frames.pop().map(|fr| fr.module).unwrap_or_default();
        // (Once stopped, every frame's values to the caller, returned or not: a script that caught a kernel
        // operation's cancel ran on and freed the displacement map's points table here, 0.5-0.9 s.)
        if r.is_err() && self.stopped.is_none() && self.cancel.as_ref().is_some_and(|c| c()) {
            self.stopped = Some(Vec::new());
        }
        match &mut self.stopped {
            Some(dead) => dead.push(self.stack.split_off(base)),
            None => self.drop_frame(base),
        }
        self.base = saved_base;
        self.module = saved_module;
        self.module_version = saved_version;
        self.pos = call_pos;
        r
    }

    fn run_body(&mut self, f: &CompiledFn) -> R<Value> {
        // The body's closures ([`closure`]), made on its first run; the tree walker when asked or profiling.
        let compiled = (!self.tree_walker && self.profile.as_ref().is_none_or(|p| p.functions)).then(|| f.closures.get_or_init(|| closure::compile_fn(f)));
        if let Some(p) = &f.precondition {
            let flow = match compiled.and_then(|c| c.precondition.as_ref()) {
                Some(cp) => cp(self)?,
                None => self.exec(p)?,
            };
            match flow {
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
        let flow = match (compiled, &f.body) {
            (Some(c), _) => match &c.body {
                closure::CBody::Expr(e) => Flow::Return(e(self)?),
                closure::CBody::Block(stmts) => closure::run_block(self, stmts)?,
            },
            (None, FnBody::Expr(e)) => Flow::Return(self.eval_in(e)?),
            (None, FnBody::Block(stmts)) => self.exec_block(stmts)?,
        };
        let v = match flow {
            Flow::Normal => Value::bool(predicate).or_undefined(!predicate),
            Flow::Return(v) => v,
            Flow::False => Value::bool(false),
            Flow::Break | Flow::Continue => return Err(self.error("break or continue outside of a loop")),
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
        let v = self.eval_in(e)?;
        self.cond_value(v, what, at)
    }

    /// A condition's value as a boolean (`at` locates its error).
    fn cond_value(&mut self, v: Value, what: &str, at: Pos) -> R<bool> {
        match v {
            Value::Bool(b, _) => Ok(b),
            other => {
                self.pos = at;
                Err(self.error(format!("{what} must be boolean, value is {}", other.error_type())))
            }
        }
    }

    fn exec(&mut self, s: &St) -> R<Flow> {
        if self.profile.as_ref().is_none_or(|p| p.functions) {
            return self.exec_statement(s);
        }
        self.timed((self.module.clone(), s.pos.line), |i| i.exec_statement(s))
    }

    /// `run`, its time added to `key`'s in the profile (once while `key` is active).
    fn timed<T>(&mut self, key: (Arc<str>, u32), run: impl FnOnce(&mut Self) -> T) -> T {
        let Some(p) = self.profile.as_mut() else { return run(self) };
        let depth = p.active.entry(key.clone()).or_insert(0);
        *depth += 1;
        let start = (*depth == 1).then(Instant::now);
        let r = run(self);
        if let Some(p) = self.profile.as_mut() {
            if let Some(d) = p.active.get_mut(&key) {
                *d -= 1;
            }
            if let Some(t) = start {
                *p.times.entry(key).or_default() += t.elapsed();
            }
        }
        r
    }

    fn exec_statement(&mut self, s: &St) -> R<Flow> {
        self.tick()?;
        match &s.k {
            StK::Let { slot, ty, init } => {
                let v = match init {
                    Some(e) => self.eval_in(e)?,
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
                if path.is_empty()
                    && op.is_none()
                    && let ExK::Call(callee, args) = &value.k
                    && let Some(r) = self.assign_grown(*slot, callee, args, value.pos)
                {
                    return r;
                }
                // A local itself (`x = v`, `x += v`): its slot directly.
                if path.is_empty() {
                    let rhs = self.eval_in(value)?;
                    self.pos = s.pos;
                    let idx = self.base + *slot as usize;
                    self.stack[idx] = match op {
                        None => rhs,
                        Some((o, ov)) => {
                            let old = std::mem::take(&mut self.stack[idx]);
                            self.binop_in(*o, old, rhs, ov.as_ref())?
                        }
                    };
                    return Ok(Flow::Normal);
                }
                let mut keys = Vec::with_capacity(path.len());
                for a in path {
                    keys.push(match a {
                        Acc::Member(k) => AccVal::Key(k.clone()),
                        Acc::Index(e) => AccVal::Key(self.eval_in(e)?),
                        Acc::Box => AccVal::Box,
                    });
                }
                let rhs = self.eval_in(value)?;
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
                            me.binop_in(*o, old, rhs, ov.as_ref())?
                        }
                    };
                    Ok(())
                });
                self.stack[idx] = root;
                r.map(|_| Flow::Normal)
            }
            StK::Expr(e) => {
                self.eval_in(e)?;
                Ok(Flow::Normal)
            }
            StK::Check(e, text) => match self.eval_in(e)? {
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
                let container = self.eval_in(iter)?;
                let container = if container.is_compact() { container.shallow().into_owned() } else { container };
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
                    Some(e) => self.eval_in(e)?,
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
                    Some(e) => self.eval_in(e)?,
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
                let (module, version) = (self.module.clone(), self.module_version);
                let r = self.exec_block(body);
                if *silent {
                    self.silent -= 1;
                }
                match r {
                    Ok(flow) => Ok(flow),
                    Err(e) if self.stopping() => Err(e),
                    Err(e) => {
                        // Unwind to this frame.
                        self.frames.truncate(depth);
                        self.base = base;
                        self.module = module;
                        self.module_version = version;
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
        // (Into a compact value: as the map or array it stands for.)
        if target.is_compact() {
            *target = target.shallow().into_owned();
        }
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

    pub(crate) fn eval_in(&mut self, e: &Ex) -> R<Value> {
        match &e.k {
            ExK::Lit(v) => Ok(v.clone()),
            ExK::Local(s) => Ok(self.stack[self.base + *s as usize].clone()),
            ExK::Global(c) => {
                self.pos = e.pos;
                self.global_in(c)
            }
            ExK::FnValue(set) => Ok(Value::Function(Shared::new(Func::Overloads(set.clone())), Tag::NONE)),
            ExK::Array(items) => {
                let mut v = Vec::with_capacity(items.len());
                for x in items {
                    v.push(self.eval_in(x)?);
                }
                Ok(Value::array(v))
            }
            ExK::Map(entries) => {
                let mut m = Map::new();
                for (k, x) in entries {
                    let k = self.eval_in(k)?;
                    let x = self.eval_in(x)?;
                    if x.is_undefined() {
                        m.remove(&k);
                    } else {
                        m.insert(k, x);
                    }
                }
                Ok(Value::map(m))
            }
            ExK::Neg(x, _) => {
                let v = self.eval_in(x)?;
                self.pos = e.pos;
                self.negated(v)
            }
            ExK::Not(x) => match self.eval_in(x)? {
                Value::Bool(b, _) => Ok(Value::bool(!b)),
                _ => {
                    self.pos = e.pos;
                    Err(self.error("Operand for '!' was not a boolean."))
                }
            },
            ExK::Bin(op, a, b, ov) => {
                let va = self.eval_in(a)?;
                let vb = self.eval_in(b)?;
                self.pos = e.pos;
                self.binop_in(*op, va, vb, ov.as_ref())
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
                let v = self.eval_in(a)?;
                if v.is_undefined() { self.eval_in(b) } else { Ok(v) }
            }
            ExK::Cond(c, a, b) => {
                if self.cond(c, "First operand of logical or conditional operator", c.pos)? {
                    self.eval_in(a)
                } else {
                    self.eval_in(b)
                }
            }
            ExK::Is(x, t) => {
                let v = self.eval_in(x)?;
                Ok(Value::bool(t.matches(&v)))
            }
            ExK::As(x, t) => {
                let v = self.eval_in(x)?;
                self.pos = e.pos;
                self.as_type_in(v, *t)
            }
            ExK::Call(callee, args) => {
                // Onshape locates errors in a call's arguments that have no
                // position of their own (undefined functions) at the call.
                if !matches!(callee, Callee::Missing(..)) {
                    self.pos = e.pos;
                }
                // A function's arguments onto the stack, where its frame starts.
                if let Callee::Overloads(set) = callee {
                    let base = self.stack.len();
                    for x in args {
                        match self.eval_in(x) {
                            Ok(v) => self.stack.push(v),
                            Err(err) => {
                                self.discard_from(base);
                                return Err(err);
                            }
                        }
                    }
                    self.pos = e.pos;
                    return self.call_set_at(set, base);
                }
                let f = match callee {
                    Callee::Value(fx) => Some(self.eval_in(fx)?),
                    _ => None,
                };
                let mut a = Vec::with_capacity(args.len());
                for x in args {
                    a.push(self.eval_in(x)?);
                }
                if !matches!(callee, Callee::Missing(..)) {
                    self.pos = e.pos;
                }
                match callee {
                    Callee::Overloads(set) => self.call_set_in(set, a),
                    Callee::Value(_) => self.call_value_in(&f.unwrap_or_default(), a),
                    Callee::Native(n, name) => self.call_native(*n, name, a),
                    Callee::MissingNative(name) => Err(self.error(format!("@{name} is not supported"))),
                    Callee::Missing(name, n) => Err(self.error(format!("Function {name} with {n} argument(s) not found"))),
                }
            }
            ExK::Index(b, i, safe) => {
                // (A local array's element read in its slot: no copy of the array. No expression changes a local.)
                if let ExK::Local(slot) = b.k
                    && matches!(self.stack[self.base + slot as usize], Value::Array(..) | Value::Vector(..))
                {
                    let idx = self.eval_in(i)?;
                    self.pos = e.pos;
                    let at = self.base + slot as usize;
                    if let (Value::Array(a, _), Value::Number(n, _)) = (&self.stack[at], &idx)
                        && *n >= 0.0
                        && n.fract() == 0.0
                        && (*n as usize) < a.len()
                    {
                        return Ok(a[*n as usize].clone());
                    }
                    let base = self.stack[at].clone();
                    return self.index_in(base, &idx);
                }
                let base = self.eval_in(b)?;
                if *safe && base.is_undefined() {
                    return Ok(Value::Undefined);
                }
                let idx = self.eval_in(i)?;
                self.pos = e.pos;
                self.index_in(base, &idx)
            }
            ExK::Member(b, k, safe) => {
                // (A local's member read in its slot: no copy of the map.)
                if let ExK::Local(slot) = b.k {
                    match &self.stack[self.base + slot as usize] {
                        Value::Map(m, _) => return Ok(m.get(k).cloned().unwrap_or_default()),
                        Value::Quantity(x, u) => return Ok(quantity_member(*x, *u, k)),
                        _ => {}
                    }
                }
                let base = self.eval_in(b)?;
                match base {
                    Value::Map(m, _) => Ok(m.get(k).cloned().unwrap_or_default()),
                    Value::Quantity(x, u) => Ok(quantity_member(x, u, k)),
                    Value::Undefined if *safe => Ok(Value::Undefined),
                    other => {
                        self.pos = e.pos;
                        Err(self.error(format!("Attempt to dereference non-container {}", crate::value::to_display_quoted(&other))))
                    }
                }
            }
            ExK::BoxGet(b, safe) => match self.eval_in(b)? {
                Value::Box(bx, _) => Ok(bx.value.lock().clone()),
                Value::Undefined if *safe => Ok(Value::Undefined),
                other => {
                    self.pos = e.pos;
                    Err(self.error(format!("Box-style access requires box, value is {}", other.error_type())))
                }
            },
            ExK::Lambda(def, outer) => {
                let captures = outer.iter().map(|s| self.stack[self.base + *s as usize].clone()).collect();
                Ok(Value::Function(Shared::new(Func::Closure { def: def.clone(), captures }), Tag::NONE))
            }
            ExK::NewBox(x) => {
                let v = self.eval_in(x)?;
                Ok(Value::new_box(v))
            }
            ExK::Try(silent, x) => {
                if *silent {
                    self.silent += 1;
                }
                let depth = self.frames.len();
                let (base, module, version) = (self.base, self.module.clone(), self.module_version);
                let r = self.eval_in(x);
                if *silent {
                    self.silent -= 1;
                }
                match r {
                    Ok(v) => Ok(v),
                    Err(e) if self.stopping() => Err(e),
                    Err(_) => {
                        self.frames.truncate(depth);
                        self.base = base;
                        self.module = module;
                        self.module_version = version;
                        Ok(Value::Undefined)
                    }
                }
            }
            ExK::Switch(subject, entries) => {
                let s = self.eval_in(subject)?;
                for (k, v) in entries {
                    if self.eval_in(k)? == s {
                        return self.eval_in(v);
                    }
                }
                Ok(Value::Undefined)
            }
        }
    }

    /// `-v` (unary minus): a number's, else an operator overload's.
    fn negated(&mut self, v: Value) -> R<Value> {
        if let Value::Number(n, Tag::NONE) = v {
            return Ok(Value::num(-n));
        }
        let mut one = [v];
        if !one[0].tag().is_none()
            && let Some(set) = self.operator_set(1)
            && let Some(r) = self.try_overload(&set, &mut one)?
        {
            return Ok(r);
        }
        let [v] = one;
        match v {
            Value::Number(n, _) => Ok(Value::num(-n)),
            _ => Err(self.error("Operand for '-' was not a number.")),
        }
    }

    /// A native's call with its arguments (its errors read "@name: …").
    fn call_native(&mut self, n: NativeFn, name: &Name, a: Vec<Value>) -> R<Value> {
        self.call_native_as(n, name, a, native_args(name))
    }

    pub(crate) fn call_native_as(&mut self, n: NativeFn, name: &Name, mut a: Vec<Value>, kind: NativeArgs) -> R<Value> {
        // A cancel stops a script between its natives too (cf-part-draft's Part Draft 3: 1,242 face areas, ~0.6 ms each,
        // in fewer steps than the tick's 16k).
        self.cancelled()?;
        // The host reads maps and arrays: its natives' arguments without compact values all the way down; this
        // crate's natives that only move elements need their top level so (no scan of a big array per call).
        // (@size of a compact value: its length, without making its array; std's preconditions ask per element.)
        if let (NativeArgs::Size, [v @ (Value::Vector(..) | Value::Quantity(..))]) = (kind, a.as_slice()) {
            return Ok(Value::num(match v {
                Value::Vector(c) => c.len(),
                _ => 2,
            } as f64));
        }
        if let NativeArgs::Keep(keep) = kind {
            a = a.into_iter().map(|v| materialized_but(v, keep)).collect();
        } else if matches!(kind, NativeArgs::Size | NativeArgs::Structural) {
            for v in a.iter_mut().filter(|v| v.is_compact()) {
                *v = v.shallow().into_owned();
            }
            // (@concatenateArrays takes an array of arrays: its compact Vectors too.)
            if &**name == "concatenateArrays"
                && let Some(Value::Array(arrays, _)) = a.first_mut()
                && arrays.iter().any(Value::is_compact)
            {
                for v in Arc::make_mut(arrays).iter_mut().filter(|v| v.is_compact()) {
                    *v = v.shallow().into_owned();
                }
            }
        } else if a.iter().any(Value::has_compact) {
            a = a.into_iter().map(Value::materialized).collect();
        }
        let saved = std::mem::replace(&mut self.native_name, name.clone());
        let r = match &self.profile {
            Some(p) if p.functions => self.timed((format!("@{name}").into(), 0), |i| n(i, a)),
            _ => n(self, a),
        };
        self.native_name = saved;
        self.native_stopped(&r);
        r
    }

    pub(crate) fn index_in(&mut self, base: Value, idx: &Value) -> R<Value> {
        if let Value::Vector(c) = &base {
            let i = self.array_index(idx, c.len())?;
            return Ok(c.at(i));
        }
        let base = if base.is_compact() { base.shallow().into_owned() } else { base };
        match base {
            Value::Array(a, _) => {
                let i = self.array_index(idx, a.len())?;
                Ok(a[i].clone())
            }
            Value::Map(m, _) => Ok(crate::value::map_get(&m, idx).cloned().unwrap_or_default()),
            other => Err(self.error(format!("Attempt to dereference non-container {}", crate::value::to_display_quoted(&other)))),
        }
    }

    /// `v as t`.
    pub(crate) fn as_type_in(&mut self, v: Value, t: TypeRef) -> R<Value> {
        match t {
            TypeRef::Std(st) => {
                if v.std_type() != st {
                    return Err(self.error(format!("Cannot convert a value of type {} to {}", v.type_description(), st.name())));
                }
                Ok(v.with_tag(Tag::NONE))
            }
            TypeRef::Tag(tag) => {
                if !crate::value::is_enum_tag(tag) {
                    // std tags boxes (`new box(...) as Instantiator`); a studio its functions
                    // (`function(t) { … } as Expression`, cf-parametric-curve).
                    return Ok(v.with_tag(tag));
                }
                // Live: 'Type error: value "FOO" cannot be enum Shade'.
                let info = type_info(tag);
                let Some(s) = v.as_str() else {
                    return Err(self.error(format!("Type error: value {} cannot be enum {}", crate::value::to_display_quoted(&v), info.name)));
                };
                match info.ordinal(&s) {
                    Some(o) => Ok(Value::Enum(tag, o)),
                    None => Err(self.error(format!("Type error: value {} cannot be enum {}", crate::value::to_display_quoted(&v), info.name))),
                }
            }
        }
    }

    /// Call the best overload of an operator if one matches; `None` when no
    /// overload accepts the operands (the built-in operator applies).
    /// `args` are taken only when an overload applies.
    fn try_overload(&mut self, set: &Arc<OverloadSet>, args: &mut [Value]) -> R<Option<Value>> {
        self.check_generation();
        let key = signature(args).map(|(sig, n)| (Arc::as_ptr(set) as usize, true, sig, n));
        // (A std operator answered from the cache's own entry: no copy of its declaration.)
        if let Some(r) = key.and_then(|k| self.resolved.get(&k)) {
            match (&r.decl, r.op) {
                (None, _) => return Ok(None),
                (Some(d), Some(op)) => {
                    if let Some(v) = crate::std_ops::apply(op, d, args, self.compact) {
                        return Ok(Some(v));
                    }
                }
                _ => {}
            }
        }
        let found = match key.and_then(|k| self.resolved.get(&k)) {
            Some(r) => r.decl.clone().map(|d| (d, None)),
            None => {
                let found = match set.fns.iter().any(|f| f.arity() == args.len() && Self::satisfies(&f.param_types, args)) {
                    true => {
                        let d = self.resolve(set, args)?;
                        let op = crate::std_ops::classify(&d);
                        Some((d, op))
                    }
                    false => None,
                };
                if let Some(k) = key {
                    self.resolved.insert(k, Resolved { _set: set.clone(), decl: found.as_ref().map(|f| f.0.clone()), op: found.as_ref().and_then(|f| f.1) });
                }
                found
            }
        };
        let Some((decl, op)) = found else { return Ok(None) };
        if let Some(op) = op
            && let Some(v) = crate::std_ops::apply(op, &decl, args, self.compact)
        {
            return Ok(Some(v));
        }
        let Some(c) = decl.compiled.get() else {
            return Err(self.error(format!("Function {} is not compiled", set.name)));
        };
        let args = args.iter_mut().map(std::mem::take).collect();
        self.invoke_in(c, args, &[], false).map(Some)
    }

    /// A binary operator, with operator overloads for tagged operands.
    pub(crate) fn binop_in(&mut self, op: BinOp, a: Value, b: Value, _ov: Option<&Arc<OverloadSet>>) -> R<Value> {
        if let (Value::Number(x, Tag::NONE), Value::Number(y, Tag::NONE)) = (&a, &b) {
            return self.num_op(op, *x, *y);
        }
        // (OPERATORS' index.)
        let operator = match op {
            BinOp::Add => Some(0),
            BinOp::Sub => Some(1),
            BinOp::Mul => Some(2),
            BinOp::Div => Some(3),
            BinOp::Mod => Some(4),
            BinOp::Pow => Some(5),
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => Some(6),
            _ => None,
        };
        let global = if !a.tag().is_none() || !b.tag().is_none() { operator.and_then(|i| self.operator_set(i)) } else { None };
        // (The operands tried in place: taken only by an overload that applies.)
        let mut pair = [a, b];
        if let Some(set) = global.as_ref() {
            // operator< answers > and <= with the operands swapped, >= and <= negated.
            let swapped = matches!(op, BinOp::Gt | BinOp::Le);
            if swapped {
                pair.swap(0, 1);
            }
            let r = self.try_overload(set, &mut pair)?;
            if swapped {
                pair.swap(0, 1);
            }
            if let Some(r) = r {
                return match (op, r) {
                    (BinOp::Le | BinOp::Ge, Value::Bool(x, _)) => Ok(Value::bool(!x)),
                    (BinOp::Le | BinOp::Ge, other) => Err(self.error(format!("operator< must return a boolean, returned {}", other.type_description()))),
                    (_, r) => Ok(r),
                };
            }
        }
        let [a, b] = pair;
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

    // ------------------------------------------------------------ the host's entry points
    // What these answer holds no compact values (`Value::materialized`): the host reads maps and arrays.

    pub fn eval(&mut self, e: &Ex) -> R<Value> {
        self.eval_in(e).map(Value::materialized)
    }
    pub fn call_set(&mut self, set: &Arc<OverloadSet>, args: Vec<Value>) -> R<Value> {
        self.call_set_in(set, args).map(Value::materialized)
    }
    pub fn call_value(&mut self, f: &Value, args: Vec<Value>) -> R<Value> {
        self.call_value_in(f, args).map(Value::materialized)
    }
    pub fn invoke(&mut self, f: &CompiledFn, args: Vec<Value>, captures: &[Value], check_types: bool) -> R<Value> {
        self.invoke_in(f, args, captures, check_types).map(Value::materialized)
    }
    pub fn global(&mut self, cell: &Arc<GlobalCell>) -> R<Value> {
        self.global_in(cell).map(Value::materialized)
    }
    pub fn index(&mut self, base: Value, idx: &Value) -> R<Value> {
        self.index_in(base, idx).map(Value::materialized)
    }
    pub fn as_type(&mut self, v: Value, t: TypeRef) -> R<Value> {
        self.as_type_in(v, t).map(Value::materialized)
    }
    pub fn binop(&mut self, op: BinOp, a: Value, b: Value, ov: Option<&Arc<OverloadSet>>) -> R<Value> {
        self.binop_in(op, a, b, ov).map(Value::materialized)
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

/// `q.key` of a compact ValueWithUnits: its value, its unit, else undefined (a map's member).
pub(crate) fn quantity_member(x: f64, u: crate::value::UnitId, key: &Value) -> Value {
    match key {
        Value::Str(s, Tag::NONE) if &**s == "value" => Value::num(x),
        Value::Str(s, Tag::NONE) if &**s == "unit" => crate::value::unit_value(u).clone(),
        _ => Value::Undefined,
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
