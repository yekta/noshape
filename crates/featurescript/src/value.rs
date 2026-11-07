//! FeatureScript values.
//!
//! Every value has one standard type (undefined, boolean, number, string,
//! array, map, box, builtin, function) and at most one type tag (an enum or
//! a custom type). Arrays and maps have value semantics: they are shared
//! (`Arc`) and copied on write (`Arc::make_mut`), so assignment and argument
//! passing are O(1). Boxes and builtins are shared references.
//!
//! Values have a total order (Onshape's "Equality and ordering"): first by
//! type tag (untagged values first, tags in declaration order), then by
//! standard type in the order listed above, then by value. Map keys use it.

use parking_lot::{Mutex, RwLock};
use std::any::Any;
use std::cmp::Ordering;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, LazyLock};

// ---------------------------------------------------------------- type tags

/// A type tag. `Tag::NONE` is "no tag"; others index the global registry in
/// declaration order, which is also their sort order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Tag(pub u32);

impl Tag {
    pub const NONE: Tag = Tag(0);
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug)]
pub struct TypeInfo {
    pub name: Arc<str>,
    /// Enum members in declaration order (empty for custom types).
    pub members: Vec<Arc<str>>,
    pub is_enum: bool,
    /// Key that identifies the declaration (module + name), so recompiling
    /// an unchanged module reuses its tags.
    pub key: String,
}

impl TypeInfo {
    pub fn ordinal(&self, member: &str) -> Option<u32> {
        self.members.iter().position(|m| &**m == member).map(|i| i as u32)
    }
}

struct Registry {
    types: Vec<Arc<TypeInfo>>,
    /// Every tag of a key: one per member list (an enum changed by an edit,
    /// or by a standard library version).
    by_key: std::collections::HashMap<String, Vec<Tag>>,
}

static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(|| RwLock::new(Registry { types: Vec::new(), by_key: Default::default() }));

/// Register (or find) a type declaration: the tag of `key` with these enum
/// members (a new tag when the members differ).
pub fn register_type(key: String, name: &str, is_enum: bool, members: Vec<Arc<str>>) -> Tag {
    let find = |r: &Registry| r.by_key.get(&key).and_then(|tags| tags.iter().copied().find(|t| r.types[t.0 as usize - 1].members == members));
    if let Some(t) = find(&REGISTRY.read()) {
        return t;
    }
    let mut r = REGISTRY.write();
    if let Some(t) = find(&r) {
        return t;
    }
    r.types.push(Arc::new(TypeInfo { name: name.into(), members, is_enum, key: key.clone() }));
    let tag = Tag(r.types.len() as u32);
    r.by_key.entry(key).or_default().push(tag);
    tag
}

/// `m[key]`; an enum key also finds the same member of its declaration in
/// another standard library version (`{ QueryType.UNION : … }[q.queryType]`
/// in std 1963 with a query the host made).
pub fn map_get<'a>(m: &'a Map, key: &Value) -> Option<&'a Value> {
    m.get(key).or_else(|| match key {
        Value::Enum(..) => m.iter().find(|(k, _)| matches!(k, Value::Enum(..)) && *k == key).map(|(_, v)| v),
        _ => None,
    })
}

/// Two tags of one type declaration (`onshape/std/query.fs#QueryType`)
/// from two standard library versions: an enum whose members changed gets
/// a tag per version, and values from either compare by member name.
pub fn same_declaration(a: Tag, b: Tag) -> bool {
    !a.is_none() && !b.is_none() && type_info(a).key == type_info(b).key
}

/// The tag of a type declaration (`onshape/std/units.fs#ValueWithUnits`) once a module declared it (not an enum's:
/// those get one per member list).
/// A ValueWithUnits' keys, made once (looked up and inserted per arithmetic operation).
pub static VALUE_KEY: LazyLock<Value> = LazyLock::new(|| Value::key("value"));
pub static UNIT_KEY: LazyLock<Value> = LazyLock::new(|| Value::key("unit"));

/// One shared allocation per string programs name (their literals and member keys, std's and the host's keys): map
/// lookups find such keys by address before comparing text ([`Map`]).
pub fn interned(s: &str) -> Arc<str> {
    static TABLE: LazyLock<Mutex<std::collections::HashSet<Arc<str>>>> = LazyLock::new(Default::default);
    let mut t = TABLE.lock();
    if let Some(a) = t.get(s) {
        return a.clone();
    }
    let a: Arc<str> = s.into();
    t.insert(a.clone());
    a
}

pub fn tag_of_key(key: &str) -> Option<Tag> {
    REGISTRY.read().by_key.get(key).and_then(|t| t.first().copied())
}

pub fn type_info(tag: Tag) -> Arc<TypeInfo> {
    REGISTRY.read().types[tag.0 as usize - 1].clone()
}

/// Whether `tag` is an enum's (kept per thread: `as` asks it per value, the registry's lock each time cost a call).
pub fn is_enum_tag(tag: Tag) -> bool {
    thread_local! {
        /// By tag: 0 not known yet, 1 an enum, 2 another type.
        static KNOWN: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    if tag.is_none() {
        return false;
    }
    let i = tag.0 as usize;
    if let Some(k) = KNOWN.with(|k| k.borrow().get(i).copied()).filter(|k| *k != 0) {
        return k == 1;
    }
    let is_enum = type_info(tag).is_enum;
    KNOWN.with(|k| {
        let mut k = k.borrow_mut();
        if k.len() <= i {
            k.resize(i + 1, 0);
        }
        k[i] = if is_enum { 1 } else { 2 };
    });
    is_enum
}

pub fn tag_name(tag: Tag) -> Arc<str> {
    if tag.is_none() { "".into() } else { type_info(tag).name.clone() }
}

// ---------------------------------------------------------------- values

/// An object provided by the runtime (Context, Sketch, ...).
pub trait Builtin: Any + Send + Sync + std::fmt::Debug {
    fn type_name(&self) -> &'static str;
    /// Creation order (sorting and identity).
    fn serial(&self) -> u64;
    fn as_any(&self) -> &dyn Any;
}

static SERIAL: AtomicU64 = AtomicU64::new(1);

/// Low bits of a serial: the number Onshape prints ("Box: 0", "BuiltIn: 3"),
/// counted per feature run and separately for boxes and builtins (live: the
/// Context is "BuiltIn: 0", a reference parameter's handles 1…12, the first
/// box "Box: 0"). The high bits keep serials unique and in creation order.
const SHOWN_BITS: u32 = 24;

thread_local! {
    /// Next printed number of a builtin and of a box on this thread.
    static SHOWN: std::cell::Cell<(u64, u64)> = const { std::cell::Cell::new((0, 0)) };
}

thread_local! {
    /// Map keys read through [`Value::get`] (`"*"`: the whole map through
    /// [`Value::as_map`]) by map address, while recording.
    static KEY_READS: std::cell::RefCell<Option<Vec<(usize, String)>>> = const { std::cell::RefCell::new(None) };
}

pub(crate) fn note_key_read(m: &Arc<Map>, key: &str) {
    KEY_READS.with(|r| {
        if let Some(reads) = r.borrow_mut().as_mut() {
            reads.push((Arc::as_ptr(m) as usize, key.to_string()));
        }
    });
}

/// Start recording the map keys natives read on this thread (the check that
/// every native reads the map std passes it, not a positional argument).
pub fn record_key_reads() {
    KEY_READS.with(|r| *r.borrow_mut() = Some(Vec::new()));
}

/// Stop recording: the keys read as (map address, key).
pub fn take_key_reads() -> Vec<(usize, String)> {
    KEY_READS.with(|r| r.borrow_mut().take()).unwrap_or_default()
}

/// Restart the printed numbers (at the start of each feature run).
pub fn reset_shown_numbers() {
    set_shown_counters((0, 0));
}

/// The next printed numbers (builtin, box) on this thread, to continue
/// them on the thread that runs the feature.
pub fn shown_counters() -> (u64, u64) {
    SHOWN.with(|c| c.get())
}

pub fn set_shown_counters(next: (u64, u64)) {
    SHOWN.with(|c| c.set(next));
}

fn make_serial(boxed: bool) -> u64 {
    let shown = SHOWN.with(|c| {
        let (b, x) = c.get();
        c.set(if boxed { (b, x + 1) } else { (b + 1, x) });
        if boxed { x } else { b }
    });
    (SERIAL.fetch_add(1, AtomicOrdering::Relaxed) << SHOWN_BITS) | (shown & ((1 << SHOWN_BITS) - 1))
}

/// A fresh creation serial for a builtin.
pub fn next_serial() -> u64 {
    make_serial(false)
}

/// A fresh creation serial for a box.
pub fn next_box_serial() -> u64 {
    make_serial(true)
}

/// The number a serial prints as.
pub fn shown_number(serial: u64) -> u64 {
    serial & ((1 << SHOWN_BITS) - 1)
}

/// A shared reference to a box's cell or a function. Map keys order these
/// by identity (creation serial, definition address and captures), never by
/// mutable contents; holding them as trait objects keeps their interior state
/// (the box's lock, lazily initialized globals) out of `Value`'s type.
pub struct Shared<T: Send + Sync + std::fmt::Debug + 'static>(Arc<dyn Opaque<T>>);

trait Opaque<T>: Send + Sync + std::fmt::Debug {
    fn inner(&self) -> &T;
}

impl<T: Send + Sync + std::fmt::Debug> Opaque<T> for T {
    fn inner(&self) -> &T {
        self
    }
}

impl<T: Send + Sync + std::fmt::Debug + 'static> Shared<T> {
    pub fn new(v: T) -> Self {
        Shared(Arc::new(v))
    }
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        std::ptr::addr_eq(Arc::as_ptr(&a.0), Arc::as_ptr(&b.0))
    }
}

impl<T: Send + Sync + std::fmt::Debug + 'static> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T: Send + Sync + std::fmt::Debug + 'static> std::ops::Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        (*self.0).inner()
    }
}

impl<T: Send + Sync + std::fmt::Debug + 'static> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (*self.0).fmt(f)
    }
}

#[derive(Debug)]
pub struct BoxCell {
    pub serial: u64,
    pub value: Mutex<Value>,
}

/// A function value.
pub enum Func {
    /// A lambda with its captured values.
    Closure { def: Arc<crate::ir::CompiledFn>, captures: Vec<Value> },
    /// A top-level function (overload set) used as a value.
    Overloads(Arc<crate::ir::OverloadSet>),
    /// A runtime builtin called with `bound` before the arguments (e.g.
    /// `PartStudioData.buildFunction`).
    Native { name: Arc<str>, f: crate::interp::NativeFn, bound: Vec<Value> },
}

impl std::fmt::Debug for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Func::Closure { def, .. } => write!(f, "Closure({}:{})", def.module, def.pos),
            Func::Overloads(s) => write!(f, "Overloads({})", s.name),
            Func::Native { name, .. } => write!(f, "Native({name})"),
        }
    }
}

impl Func {
    fn key(&self) -> usize {
        match self {
            Func::Closure { def, .. } => Arc::as_ptr(def) as *const u8 as usize,
            Func::Overloads(s) => Arc::as_ptr(s) as *const u8 as usize,
            Func::Native { f, .. } => *f as usize,
        }
    }
    fn captures(&self) -> &[Value] {
        match self {
            Func::Closure { captures, .. } => captures,
            Func::Overloads(_) => &[],
            Func::Native { bound, .. } => bound,
        }
    }
}

pub use crate::map::Map;

#[derive(Clone, Debug, Default)]
pub enum Value {
    #[default]
    Undefined,
    /// `undefined as SomeType` (e.g. the standard library's FailedParameter).
    TaggedUndefined(Tag),
    Bool(bool, Tag),
    Number(f64, Tag),
    Str(Arc<str>, Tag),
    /// An enum member: tag and ordinal (a string with the enum's tag).
    Enum(Tag, u32),
    Array(Arc<Vec<Value>>, Tag),
    Map(Arc<Map>, Tag),
    /// A box; std tags some (`new box(...) as Instantiator`).
    Box(Shared<BoxCell>, Tag),
    Builtin(Arc<dyn Builtin>, Tag),
    /// A function; a typecheck type may tag it (`function(t) { … } as Expression`: cf-parametric-curve).
    Function(Shared<Func>, Tag),
    /// A ValueWithUnits held in the value: `{ "unit" : unit(id), "value" : x } as ValueWithUnits` without its map
    /// (FS-COMPILED-INTERP stage 2; std's operators make them). Alike in every respect but its form.
    Quantity(f64, UnitId),
    /// A Vector of two or three numbers, or lengths of one unit, held in the value: `[…] as Vector` without its
    /// array.
    Vector(Compact),
}

/// A unit map of [`UNITS`] (0: none, a Vector's plain numbers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitId(pub u32);

/// A [`Value::Vector`]'s elements: `len` of `v`, plain numbers or quantities of `unit`.
#[derive(Clone, Copy, Debug)]
pub struct Compact {
    pub v: [f64; 3],
    pub unit: UnitId,
    pub len: VecLen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VecLen {
    Two = 2,
    Three = 3,
}

impl Compact {
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn is_empty(&self) -> bool {
        false
    }
    /// Element `i` as a value (a number or a quantity).
    pub fn at(&self, i: usize) -> Value {
        if self.unit.0 == 0 { Value::num(self.v[i]) } else { Value::Quantity(self.v[i], self.unit) }
    }
    pub fn elements(&self) -> impl ExactSizeIterator<Item = Value> + DoubleEndedIterator + '_ {
        (0..self.len()).map(|i| self.at(i))
    }
}

/// The unit maps quantities name: kept for good (a few per run: meter, radian, their powers).
static UNITS: LazyLock<RwLock<Vec<&'static Value>>> = LazyLock::new(Default::default);

/// The unit map `id` names (from this thread's copy of the table, which only grows: no lock per operation).
pub fn unit_value(id: UnitId) -> &'static Value {
    static NONE: Value = Value::Undefined;
    thread_local! {
        static SEEN: std::cell::RefCell<Vec<&'static Value>> = const { std::cell::RefCell::new(Vec::new()) };
    }
    let i = (id.0 as usize).wrapping_sub(1);
    SEEN.with(|seen| {
        if let Some(v) = seen.borrow().get(i) {
            return *v;
        }
        let all = UNITS.read();
        *seen.borrow_mut() = all.clone();
        all.get(i).copied().unwrap_or(&NONE)
    })
}

/// The id of unit map `unit` (made on first sight); `None` for anything but a map.
pub fn unit_id(unit: &Value) -> Option<UnitId> {
    let Value::Map(m, _) = unit else { return None };
    thread_local! {
        /// The last map asked (held: its address can't name another map meanwhile) and its id.
        static LAST: std::cell::RefCell<Option<(Arc<Map>, u32)>> = const { std::cell::RefCell::new(None) };
    }
    if let Some(id) = LAST.with(|l| l.borrow().as_ref().filter(|(a, _)| Arc::ptr_eq(a, m)).map(|(_, id)| *id)) {
        return Some(UnitId(id));
    }
    let found = UNITS.read().iter().position(|x| *x == unit);
    let id = match found {
        Some(i) => i as u32 + 1,
        None => {
            let mut u = UNITS.write();
            match u.iter().position(|x| *x == unit) {
                Some(i) => i as u32 + 1,
                None => {
                    u.push(Box::leak(Box::new(unit.clone())));
                    u.len() as u32
                }
            }
        }
    };
    LAST.with(|l| *l.borrow_mut() = Some((m.clone(), id)));
    Some(UnitId(id))
}

fn std_tag_of(key: &'static str, cell: &'static std::sync::OnceLock<Tag>) -> Option<Tag> {
    if let Some(t) = cell.get() {
        return Some(*t);
    }
    let t = tag_of_key(key)?;
    Some(*cell.get_or_init(|| t))
}

/// std's ValueWithUnits tag, once units.fs is loaded.
pub fn vwu_tag() -> Option<Tag> {
    static TAG: std::sync::OnceLock<Tag> = std::sync::OnceLock::new();
    std_tag_of("onshape/std/units.fs#ValueWithUnits", &TAG)
}

/// std's Vector tag, once vector.fs is loaded.
pub fn vector_tag() -> Option<Tag> {
    static TAG: std::sync::OnceLock<Tag> = std::sync::OnceLock::new();
    std_tag_of("onshape/std/vector.fs#Vector", &TAG)
}

/// Standard types, in sort order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StdType {
    Undefined,
    Boolean,
    Number,
    String,
    Array,
    Map,
    Box,
    Builtin,
    Function,
}

impl StdType {
    pub fn name(self) -> &'static str {
        match self {
            StdType::Undefined => "undefined",
            StdType::Boolean => "boolean",
            StdType::Number => "number",
            StdType::String => "string",
            StdType::Array => "array",
            StdType::Map => "map",
            StdType::Box => "box",
            StdType::Builtin => "builtin",
            StdType::Function => "function",
        }
    }

    pub fn from_name(s: &str) -> Option<StdType> {
        Some(match s {
            "undefined" => StdType::Undefined,
            "boolean" => StdType::Boolean,
            "number" => StdType::Number,
            "string" => StdType::String,
            "array" => StdType::Array,
            "map" => StdType::Map,
            "box" => StdType::Box,
            "builtin" => StdType::Builtin,
            "function" => StdType::Function,
            _ => return None,
        })
    }
}

impl Value {
    pub fn num(n: f64) -> Value {
        Value::Number(n, Tag::NONE)
    }
    pub fn bool(b: bool) -> Value {
        Value::Bool(b, Tag::NONE)
    }
    pub fn str(s: impl Into<Arc<str>>) -> Value {
        Value::Str(s.into(), Tag::NONE)
    }
    /// A string a program or the host names as a key ([`interned`]).
    pub fn key(s: &str) -> Value {
        Value::Str(interned(s), Tag::NONE)
    }
    /// The same string by address (an [`interned`] key): equal without comparing text.
    pub fn same_str(&self, other: &Value) -> bool {
        matches!((self, other), (Value::Str(a, ta), Value::Str(b, tb)) if ta == tb && Arc::ptr_eq(a, b))
    }
    pub fn array(v: Vec<Value>) -> Value {
        Value::Array(Arc::new(v), Tag::NONE)
    }
    pub fn map(m: Map) -> Value {
        Value::Map(Arc::new(m), Tag::NONE)
    }
    pub fn new_box(v: Value) -> Value {
        Value::Box(Shared::new(BoxCell { serial: next_box_serial(), value: Mutex::new(v) }), Tag::NONE)
    }
    /// A map from string keys.
    pub fn map_from<'a>(entries: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
        let mut m = Map::new();
        for (k, v) in entries {
            if !matches!(v, Value::Undefined) {
                m.insert(Value::str(k), v);
            }
        }
        Value::map(m)
    }

    pub fn std_type(&self) -> StdType {
        match self {
            Value::Undefined | Value::TaggedUndefined(_) => StdType::Undefined,
            Value::Bool(..) => StdType::Boolean,
            Value::Number(..) => StdType::Number,
            Value::Str(..) | Value::Enum(..) => StdType::String,
            Value::Array(..) | Value::Vector(..) => StdType::Array,
            Value::Map(..) | Value::Quantity(..) => StdType::Map,
            Value::Box(..) => StdType::Box,
            Value::Builtin(..) => StdType::Builtin,
            Value::Function(..) => StdType::Function,
        }
    }

    pub fn tag(&self) -> Tag {
        match self {
            Value::Bool(_, t) | Value::Number(_, t) | Value::Str(_, t) | Value::Array(_, t) | Value::Map(_, t) | Value::Box(_, t) | Value::Builtin(_, t) | Value::Function(_, t) | Value::Enum(t, _) | Value::TaggedUndefined(t) => *t,
            Value::Quantity(..) => vwu_tag().unwrap_or(Tag::NONE),
            Value::Vector(..) => vector_tag().unwrap_or(Tag::NONE),
            _ => Tag::NONE,
        }
    }

    /// A [`Value::Quantity`] or [`Value::Vector`]: a map or an array held in the value.
    pub fn is_compact(&self) -> bool {
        matches!(self, Value::Quantity(..) | Value::Vector(..))
    }

    /// The map or array a compact value stands for (one level: a Vector's elements stay quantities); others as they
    /// are.
    pub fn shallow(&self) -> std::borrow::Cow<'_, Value> {
        use std::borrow::Cow;
        match self {
            Value::Quantity(x, u) => Cow::Owned(Value::Map(Arc::new(Map::with_unit_value(unit_value(*u).clone(), Value::num(*x))), self.tag())),
            Value::Vector(c) => Cow::Owned(Value::Array(Arc::new(c.elements().collect()), self.tag())),
            _ => Cow::Borrowed(self),
        }
    }

    /// Whether this value holds a compact value anywhere (in its arrays, maps and boxes).
    pub fn has_compact(&self) -> bool {
        match self {
            Value::Quantity(..) | Value::Vector(..) => true,
            Value::Array(a, _) => a.iter().any(Value::has_compact),
            Value::Map(m, _) => m.iter().any(|(k, v)| k.has_compact() || v.has_compact()),
            Value::Box(b, _) => b.value.lock().has_compact(),
            _ => false,
        }
    }

    /// This value with every compact value made the map or array it stands for, all the way down (what the host
    /// sees: natives' arguments, results the host asks for). Unchanged, without a copy, when it holds none.
    pub fn materialized(self) -> Value {
        if !self.has_compact() {
            return self;
        }
        self.materialize()
    }

    fn materialize(self) -> Value {
        match self {
            Value::Quantity(..) | Value::Vector(..) => self.shallow().into_owned().materialize(),
            Value::Array(a, t) => Value::Array(Arc::new(Arc::unwrap_or_clone(a).into_iter().map(Value::materialized).collect()), t),
            Value::Map(m, t) => Value::Map(Arc::new(Arc::unwrap_or_clone(m).into_iter().map(|(k, v)| (k.materialized(), v.materialized())).collect()), t),
            Value::Box(b, t) => {
                // (Its holders all see the same contents, in their usual form.)
                let inner = std::mem::take(&mut *b.value.lock());
                *b.value.lock() = inner.materialized();
                Value::Box(b, t)
            }
            other => other,
        }
    }

    /// The same value with tag `tag` (`Tag::NONE` removes it). Enum members
    /// become plain strings when retagged.
    pub fn with_tag(self, tag: Tag) -> Value {
        match self {
            Value::Bool(b, _) => Value::Bool(b, tag),
            Value::Number(n, _) => Value::Number(n, tag),
            Value::Str(s, _) => Value::Str(s, tag),
            Value::Enum(t, o) => Value::Str(enum_name(t, o), tag),
            Value::Array(a, _) => Value::Array(a, tag),
            Value::Map(m, _) => Value::Map(m, tag),
            Value::Builtin(b, _) => Value::Builtin(b, tag),
            Value::Box(b, _) => Value::Box(b, tag),
            Value::Function(f, _) => Value::Function(f, tag),
            // (Its own tag keeps it compact; another makes it the map or array it stands for.)
            v @ (Value::Quantity(..) | Value::Vector(..)) if v.tag() == tag => v,
            v @ (Value::Quantity(..) | Value::Vector(..)) => v.shallow().into_owned().with_tag(tag),
            Value::Undefined | Value::TaggedUndefined(_) => {
                if tag.is_none() { Value::Undefined } else { Value::TaggedUndefined(tag) }
            }
        }
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n, _) => Some(*n),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b, _) => Some(*b),
            _ => None,
        }
    }
    /// The string of a string or enum member.
    pub fn as_str(&self) -> Option<Arc<str>> {
        match self {
            Value::Str(s, _) => Some(s.clone()),
            Value::Enum(t, o) => Some(enum_name(*t, *o)),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&Arc<Vec<Value>>> {
        match self {
            Value::Array(a, _) => Some(a),
            _ => None,
        }
    }
    pub fn as_map(&self) -> Option<&Arc<Map>> {
        match self {
            Value::Map(m, _) => {
                note_key_read(m, "*");
                Some(m)
            }
            _ => None,
        }
    }
    /// `m[key]` for a string key (undefined if absent or not a map).
    pub fn get(&self, key: &str) -> &Value {
        static UNDEF: Value = Value::Undefined;
        match self {
            Value::Map(m, _) => {
                note_key_read(m, key);
                m.get_str(key).unwrap_or(&UNDEF)
            }
            _ => &UNDEF,
        }
    }
    pub fn builtin<T: 'static>(&self) -> Option<&T> {
        match self {
            Value::Builtin(b, _) => b.as_any().downcast_ref::<T>(),
            _ => None,
        }
    }

    /// Standard type name plus tag, for messages: `"Vector"`, `"number"`.
    /// The type in runtime error messages: `number`, `ValueWithUnits (map)`.
    pub fn error_type(&self) -> String {
        let t = self.tag();
        let std = self.std_type().name();
        if t.is_none() { std.to_string() } else { format!("{} ({std})", tag_name(t)) }
    }

    pub fn type_description(&self) -> String {
        let t = self.tag();
        if t.is_none() { self.std_type().name().to_string() } else { tag_name(t).to_string() }
    }
}

pub fn enum_name(tag: Tag, ord: u32) -> Arc<str> {
    type_info(tag).members.get(ord as usize).cloned().unwrap_or_else(|| "?".into())
}

// ---------------------------------------------------------------- equality and order

fn std_rank(v: &Value) -> StdType {
    v.std_type()
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        let (ta, tb) = (self.tag(), other.tag());
        if ta != tb {
            return ta.cmp(&tb);
        }
        let (ra, rb) = (std_rank(self), std_rank(other));
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (self, other) {
            (Value::Undefined | Value::TaggedUndefined(_), Value::Undefined | Value::TaggedUndefined(_)) => Ordering::Equal,
            (Value::Bool(a, _), Value::Bool(b, _)) => a.cmp(b),
            (Value::Number(a, _), Value::Number(b, _)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            (Value::Enum(_, a), Value::Enum(_, b)) => a.cmp(b),
            (Value::Str(a, _), Value::Str(b, _)) => if Arc::ptr_eq(a, b) { Ordering::Equal } else { a.cmp(b) },
            (Value::Enum(t, o), Value::Str(b, _)) => (*enum_name(*t, *o)).cmp(&**b),
            (Value::Str(a, _), Value::Enum(t, o)) => (**a).cmp(&*enum_name(*t, *o)),
            (Value::Array(a, _), Value::Array(b, _)) => {
                if Arc::ptr_eq(a, b) {
                    return Ordering::Equal;
                }
                a.len().cmp(&b.len()).then_with(|| a.iter().cmp(b.iter()))
            }
            (Value::Map(a, _), Value::Map(b, _)) => {
                if Arc::ptr_eq(a, b) {
                    return Ordering::Equal;
                }
                // As arrays of [key, value] arrays sorted by key: by size first.
                a.len().cmp(&b.len()).then_with(|| a.iter().cmp(b.iter()))
            }
            (Value::Quantity(a, ua), Value::Quantity(b, ub)) => {
                // As maps: the unit first ("unit" < "value"), then the number.
                let units = if ua == ub { Ordering::Equal } else { unit_value(*ua).cmp(unit_value(*ub)) };
                units.then_with(|| a.partial_cmp(b).unwrap_or(Ordering::Equal))
            }
            (Value::Vector(a), Value::Vector(b)) => a.len.cmp(&b.len).then_with(|| a.elements().cmp(b.elements())),
            (a, b) if a.is_compact() || b.is_compact() => a.shallow().cmp(&b.shallow()),
            (Value::Box(a, _), Value::Box(b, _)) => a.serial.cmp(&b.serial),
            (Value::Builtin(a, _), Value::Builtin(b, _)) => a.serial().cmp(&b.serial()),
            (Value::Function(a, _), Value::Function(b, _)) => {
                if Shared::ptr_eq(a, b) {
                    return Ordering::Equal;
                }
                a.key().cmp(&b.key()).then_with(|| a.captures().iter().cmp(b.captures().iter()))
            }
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Number(a, ta), Value::Number(b, tb)) => a == b && ta == tb,
            (Value::Str(a, ta), Value::Str(b, tb)) => ta == tb && a == b,
            (Value::Enum(ta, a), Value::Enum(tb, b)) => (ta == tb && a == b) || (ta != tb && same_declaration(*ta, *tb) && enum_name(*ta, *a) == enum_name(*tb, *b)),
            _ => self.cmp(other) == Ordering::Equal,
        }
    }
}

impl Eq for Value {}

// ---------------------------------------------------------------- printing

/// Text of a number as Onshape prints it (live dom/fs-errors3.txt): the
/// shortest round-trip digits, plain for 1e-3 <= |x| < 1e7 (`0.05`, `12`),
/// otherwise exponent form (`1.23456789123e+8`, `1e-7`, `1e+21`); `-0`,
/// `inf`.
pub fn format_number(n: f64) -> String {
    if n.is_infinite() {
        return if n > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if n == 0.0 {
        return if n.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    if (1e-3..1e7).contains(&n.abs()) {
        return format!("{n}");
    }
    let s = format!("{n:e}");
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

/// A value as it appears inside a container (strings quoted): runtime
/// error texts use it ('Attempt to dereference non-container "abc"').
pub fn to_display_quoted(v: &Value) -> String {
    let mut s = String::new();
    write_display(&mut s, v, false);
    s
}

/// The runtime's string conversion used by `~` (Onshape's "internal
/// algorithm"): strings print as-is at the top level and quoted inside
/// containers; tagged containers print as `Tag(n) : value`.
pub fn to_display(v: &Value) -> String {
    let mut s = String::new();
    write_display(&mut s, v, true);
    s
}

fn write_display(out: &mut String, v: &Value, top: bool) {
    match v {
        Value::Quantity(..) | Value::Vector(..) => write_display(out, &v.shallow(), top),
        Value::Undefined => out.push_str("undefined"),
        Value::TaggedUndefined(t) => {
            let _ = write!(out, "{} : undefined", tag_name(*t));
        }
        Value::Bool(b, _) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            out.push_str(&format_number(*n))
        }
        Value::Str(s, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            if top && t.is_none() {
                out.push_str(s)
            } else {
                let _ = write!(out, "\"{s}\"");
            }
        }
        Value::Enum(t, o) => out.push_str(&enum_name(*t, *o)),
        Value::Array(a, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                out.push_str(if i == 0 { " " } else { " , " });
                write_display(out, x, false);
            }
            out.push_str(" ]");
        }
        Value::Map(m, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                out.push_str(if i == 0 { " " } else { " , " });
                write_display(out, k, false);
                out.push_str(" : ");
                write_display(out, x, false);
            }
            out.push_str(" }");
        }
        // Live: "Box: 0", "Context : BuiltIn: 0", "<function 6>" (numbered
        // per feature run, see `shown_number`).
        Value::Box(b, _) => {
            let _ = write!(out, "Box: {}", shown_number(b.serial));
        }
        Value::Builtin(b, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            let _ = write!(out, "BuiltIn: {}", shown_number(b.serial()));
        }
        Value::Function(f, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            let line = match &**f {
                Func::Closure { def, .. } => def.pos.line,
                Func::Overloads(s) => s.fns.first().map(|d| d.pos.line).unwrap_or(0),
                Func::Native { .. } => 0,
            };
            let _ = write!(out, "<function {line}>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordering_rules() {
        let t = register_type("test:Anything".into(), "Anything", false, vec![]);
        // 0 < (true as Anything) < (0 as Anything)
        let a = Value::num(0.0);
        let b = Value::bool(true).with_tag(t);
        let c = Value::num(0.0).with_tag(t);
        assert!(a < b && b < c);
        assert_ne!(a, c);
        assert_eq!(Value::num(-0.0), Value::num(0.0));
        // Arrays: by size first.
        assert!(Value::array(vec![Value::num(9.0)]) < Value::array(vec![Value::num(1.0), Value::num(1.0)]));
        // Enums: by ordinal.
        let e = register_type("test:E".into(), "E", true, vec!["Z".into(), "A".into()]);
        assert!(Value::Enum(e, 0) < Value::Enum(e, 1));
        assert_eq!(Value::Enum(e, 1).as_str().as_deref(), Some("A"));
    }

    #[test]
    fn display() {
        assert_eq!(format_number(2.0), "2");
        assert_eq!(format_number(0.25), "0.25");
        assert_eq!(format_number(-1.5e-9), "-1.5e-9");
        let m = Value::map_from([("value", Value::num(2.0))]);
        assert_eq!(to_display(&m), "{ \"value\" : 2 }");
        assert_eq!(to_display(&Value::array(vec![Value::num(1.0), Value::str("a")])), "[ 1 , \"a\" ]");
    }
}
