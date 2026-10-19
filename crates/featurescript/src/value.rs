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
use std::collections::BTreeMap;
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
    by_key: std::collections::HashMap<String, Tag>,
}

static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(|| RwLock::new(Registry { types: Vec::new(), by_key: Default::default() }));

/// Register (or find) a type declaration; enum members must not change for
/// a given key.
pub fn register_type(key: String, name: &str, is_enum: bool, members: Vec<Arc<str>>) -> Tag {
    if let Some(t) = REGISTRY.read().by_key.get(&key)
        && type_info(*t).members == members
    {
        return *t;
    }
    let mut r = REGISTRY.write();
    if let Some(t) = r.by_key.get(&key)
        && r.types[t.0 as usize - 1].members == members
    {
        return *t;
    }
    r.types.push(Arc::new(TypeInfo { name: name.into(), members, is_enum, key: key.clone() }));
    let tag = Tag(r.types.len() as u32);
    r.by_key.insert(key, tag);
    tag
}

pub fn type_info(tag: Tag) -> Arc<TypeInfo> {
    REGISTRY.read().types[tag.0 as usize - 1].clone()
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

/// Restart the printed numbers (at the start of each feature run).
pub fn reset_shown_numbers() {
    SHOWN.with(|c| c.set((0, 0)));
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
}

impl std::fmt::Debug for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Func::Closure { def, .. } => write!(f, "Closure({}:{})", def.module, def.pos),
            Func::Overloads(s) => write!(f, "Overloads({})", s.name),
        }
    }
}

impl Func {
    fn key(&self) -> usize {
        match self {
            Func::Closure { def, .. } => Arc::as_ptr(def) as *const u8 as usize,
            Func::Overloads(s) => Arc::as_ptr(s) as *const u8 as usize,
        }
    }
    fn captures(&self) -> &[Value] {
        match self {
            Func::Closure { captures, .. } => captures,
            Func::Overloads(_) => &[],
        }
    }
}

pub type Map = BTreeMap<Value, Value>;

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
    Box(Shared<BoxCell>),
    Builtin(Arc<dyn Builtin>, Tag),
    Function(Shared<Func>),
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
    pub fn array(v: Vec<Value>) -> Value {
        Value::Array(Arc::new(v), Tag::NONE)
    }
    pub fn map(m: Map) -> Value {
        Value::Map(Arc::new(m), Tag::NONE)
    }
    pub fn new_box(v: Value) -> Value {
        Value::Box(Shared::new(BoxCell { serial: next_box_serial(), value: Mutex::new(v) }))
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
            Value::Array(..) => StdType::Array,
            Value::Map(..) => StdType::Map,
            Value::Box(..) => StdType::Box,
            Value::Builtin(..) => StdType::Builtin,
            Value::Function(..) => StdType::Function,
        }
    }

    pub fn tag(&self) -> Tag {
        match self {
            Value::Bool(_, t) | Value::Number(_, t) | Value::Str(_, t) | Value::Array(_, t) | Value::Map(_, t) | Value::Builtin(_, t) | Value::Enum(t, _) | Value::TaggedUndefined(t) => *t,
            _ => Tag::NONE,
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
            Value::Undefined | Value::TaggedUndefined(_) => {
                if tag.is_none() { Value::Undefined } else { Value::TaggedUndefined(tag) }
            }
            other => other,
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
            Value::Map(m, _) => Some(m),
            _ => None,
        }
    }
    /// `m[key]` for a string key (undefined if absent or not a map).
    pub fn get(&self, key: &str) -> &Value {
        static UNDEF: Value = Value::Undefined;
        match self {
            Value::Map(m, _) => m.get(&Value::Str(key.into(), Tag::NONE)).unwrap_or(&UNDEF),
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
            (Value::Str(a, _), Value::Str(b, _)) => a.cmp(b),
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
            (Value::Box(a), Value::Box(b)) => a.serial.cmp(&b.serial),
            (Value::Builtin(a, _), Value::Builtin(b, _)) => a.serial().cmp(&b.serial()),
            (Value::Function(a), Value::Function(b)) => {
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
            (Value::Enum(ta, a), Value::Enum(tb, b)) => ta == tb && a == b,
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
        Value::Box(b) => {
            let _ = write!(out, "Box: {}", shown_number(b.serial));
        }
        Value::Builtin(b, t) => {
            if !t.is_none() {
                let _ = write!(out, "{} : ", tag_name(*t));
            }
            let _ = write!(out, "BuiltIn: {}", shown_number(b.serial()));
        }
        Value::Function(f) => {
            let line = match &**f {
                Func::Closure { def, .. } => def.pos.line,
                Func::Overloads(s) => s.fns.first().map(|d| d.pos.line).unwrap_or(0),
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
