//! FeatureScript compiled to closures (FS-COMPILED-INTERP stage 1): a function body's nodes become closures that call
//! the tree walker's own helpers, so both answer alike; nodes not worth compiling run the tree walker on a copy of
//! their subtree. NOSHAPE_FS_TREE=1 runs the tree walker everywhere (the reference of the differential test).

use super::*;

pub(crate) type CEx = Box<dyn Fn(&mut Interp) -> R<Value> + Send + Sync>;
pub(crate) type CSt = Box<dyn Fn(&mut Interp) -> R<Flow> + Send + Sync>;

/// A function's precondition and body as closures.
pub struct Body {
    pub(crate) precondition: Option<CSt>,
    pub(crate) body: CBody,
}

pub(crate) enum CBody {
    Block(Vec<CSt>),
    Expr(CEx),
}

/// New interpreters' mode: 0 from NOSHAPE_FS_TREE, 1 closures, 2 the tree walker ([`set_tree_walker_default`]).
static MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Whether new interpreters walk the tree instead of running closures (NOSHAPE_FS_TREE set, or as set).
pub fn tree_walker_default() -> bool {
    static ENV: LazyLock<bool> = LazyLock::new(|| std::env::var_os("NOSHAPE_FS_TREE").is_some());
    match MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => *ENV,
    }
}

/// New interpreters walk the tree (`true`) or run closures (the differential test runs a corpus both ways).
pub fn set_tree_walker_default(tree: bool) {
    MODE.store(if tree { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// New interpreters' compact values: 0 from NOSHAPE_FS_PLAIN, 1 on, 2 off ([`set_compact_default`]).
static COMPACT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Whether std's operators answer compact values in new interpreters (not with NOSHAPE_FS_PLAIN, or as set).
pub fn compact_default() -> bool {
    static ENV: LazyLock<bool> = LazyLock::new(|| std::env::var_os("NOSHAPE_FS_PLAIN").is_none());
    match COMPACT.load(std::sync::atomic::Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => *ENV,
    }
}

/// New interpreters answer compact values (`true`) or maps and arrays (the differential test's reference).
pub fn set_compact_default(on: bool) {
    COMPACT.store(if on { 1 } else { 2 }, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn compile_fn(f: &CompiledFn) -> Body {
    Body {
        precondition: f.precondition.as_ref().map(statement),
        body: match &f.body {
            FnBody::Block(stmts) => CBody::Block(stmts.iter().map(statement).collect()),
            FnBody::Expr(e) => CBody::Expr(expression(e)),
        },
    }
}

/// A block's statements in order (the tree walker's `exec_block`).
pub(crate) fn run_block(i: &mut Interp, stmts: &[CSt]) -> R<Flow> {
    for s in stmts {
        match s(i)? {
            Flow::Normal => {}
            other => return Ok(other),
        }
    }
    Ok(Flow::Normal)
}

/// The tree walker on a copy of `s` (statements not compiled).
fn walked_statement(s: &St) -> CSt {
    let s = s.clone();
    Box::new(move |i| i.exec(&s))
}

/// The tree walker on a copy of `e`.
fn walked(e: &Ex) -> CEx {
    let e = e.clone();
    Box::new(move |i| i.eval_in(&e))
}

/// @size of an array, a map or a compact value (a ValueWithUnits: its unit and value).
fn size_of(v: &Value) -> Option<usize> {
    match v {
        Value::Array(a, _) => Some(a.len()),
        Value::Map(m, _) => Some(m.len()),
        Value::Vector(c) => Some(c.len()),
        Value::Quantity(..) => Some(2),
        _ => None,
    }
}

/// An append site's resolution: its generation, signature, append (else concatenateArrays), and the array's tag and
/// element's kind standing for the signature.
type GrowSite = (u64, ([u32; 6], u8), bool, Option<(Tag, u16)>);

/// A binary operator's call site: its std operator kept for its operands' types (+ - * /).
struct BinSite {
    op: BinOp,
    operator: Option<usize>,
    ov: Option<Arc<OverloadSet>>,
    site: std::sync::OnceLock<Site>,
}

impl BinSite {
    fn new(op: BinOp, ov: Option<Arc<OverloadSet>>) -> Self {
        // (+ - * /: the site's std operator kept for its operands' types: no resolution per operation.)
        let operator = match op {
            BinOp::Add => Some(0),
            BinOp::Sub => Some(1),
            BinOp::Mul => Some(2),
            BinOp::Div => Some(3),
            _ => None,
        };
        Self { op, operator, ov, site: std::sync::OnceLock::new() }
    }

    fn apply(&self, i: &mut Interp, va: Value, vb: Value) -> R<Value> {
        let Some(operator) = self.operator else { return i.binop_in(self.op, va, vb, self.ov.as_ref()) };
        // (Numbers and compact values: their kinds stand for their signature; std's arithmetic on them as f64s.)
        let kinds = quick_sig([&va, &vb]);
        let mut pair = [va, vb];
        if kinds != 0
            && i.compact
            && let Some(s) = self.site.get()
            && s.kinds == kinds
            && s.generation == crate::compile::overloads_generation()
            && let (Some(op), Some(arith)) = (s.op, s.arith)
            && let Some(v) = crate::std_ops::compact_as(op, arith, &pair)
        {
            return Ok(v);
        }
        let sig = signature(&pair);
        if let (Some(s), Some(sig)) = (self.site.get(), sig)
            && s.sig == sig
            && s.generation == crate::compile::overloads_generation()
            && let Some(op) = s.op
            && let Some(v) = crate::std_ops::apply(op, &s.decl, &mut pair, i.compact)
        {
            return Ok(v);
        }
        let [va, vb] = pair;
        let r = i.binop_in(self.op, va, vb, self.ov.as_ref());
        if self.site.get().is_none()
            && let Some(sig) = sig
            && let Some(mut s) = i.std_site(Some(operator), None, sig)
        {
            s.kinds = kinds;
            let _ = self.site.set(s);
        }
        r
    }
}

/// A loop body's flow: Some to leave the loop with (`Break`: normally).
fn after_body(flow: Flow) -> Option<Flow> {
    match flow {
        Flow::Normal | Flow::Continue => None,
        Flow::Break => Some(Flow::Normal),
        other => Some(other),
    }
}

pub(crate) fn statement(s: &St) -> CSt {
    let pos = s.pos;
    match &s.k {
        StK::Let { slot, ty, init } => {
            let (slot, ty) = (*slot as usize, *ty);
            let init = init.as_ref().map(expression);
            Box::new(move |i| {
                i.tick()?;
                let v = match &init {
                    Some(c) => c(i)?,
                    None => Value::Undefined,
                };
                if let Some(t) = ty
                    && !t.matches(&v)
                {
                    i.pos = pos;
                    return Err(i.error(format!("Value assigned to variable should be {}, was {}", t.name(), v.type_description())));
                }
                let b = i.base;
                i.stack[b + slot] = v;
                Ok(Flow::Normal)
            })
        }
        StK::Assign { slot, path, op, value } if path.is_empty() => {
            let slot = *slot;
            // `x = append(x, v)` (the tree walker's assign_grown): x grown in place.
            if op.is_none()
                && let ExK::Call(Callee::Overloads(set), args) = &value.k
                && matches!(&*set.name, "append" | "concatenateArrays")
                && let [x, y] = args.as_slice()
                && matches!(x.k, ExK::Local(s) if s == slot)
            {
                let (set, y, vpos) = (set.clone(), expression(y), value.pos);
                // (The site's std append / concatenateArrays kept for its arguments' types: grown without a
                // resolution per element.)
                // (An array of its tag and a number or compact element: their kinds stand for their signature.)
                let site: std::sync::OnceLock<GrowSite> = std::sync::OnceLock::new();
                return Box::new(move |i| {
                    i.tick()?;
                    i.pos = vpos;
                    let y = y(i)?;
                    i.pos = vpos;
                    let idx = i.base + slot as usize;
                    let kinds = match &i.stack[idx] {
                        Value::Array(_, t) => Some((*t, quick_sig([&y]))).filter(|k| k.1 != 0),
                        _ => None,
                    };
                    if let Some((generation, _, append, Some(k))) = site.get()
                        && kinds == Some(*k)
                        && *generation == crate::compile::overloads_generation()
                    {
                        i.grow_slot(idx, *append, y);
                        return Ok(Flow::Normal);
                    }
                    let sig = signature_of([&i.stack[idx], &y].into_iter());
                    if let (Some((generation, s, append, _)), Some(sig)) = (site.get(), sig)
                        && *s == sig
                        && *generation == crate::compile::overloads_generation()
                        && matches!(i.stack[idx], Value::Array(..))
                    {
                        i.grow_slot(idx, *append, y);
                        return Ok(Flow::Normal);
                    }
                    let r = i.grow_one(slot, &set, y);
                    if site.get().is_none()
                        && let Some(sig) = sig
                        && let Some(append) = i.std_grow_site(&set, sig)
                    {
                        let _ = site.set((crate::compile::overloads_generation(), sig, append, kinds));
                    }
                    r
                });
            }
            if op.is_none()
                && let ExK::Call(callee, args) = &value.k
                && i_grows(slot, callee, args)
            {
                return walked_statement(s);
            }
            let (v, op) = (expression(value), op.as_ref().map(|(o, ov)| BinSite::new(*o, ov.clone())));
            let idx = slot as usize;
            Box::new(move |i| {
                i.tick()?;
                let rhs = v(i)?;
                i.pos = pos;
                let at = i.base + idx;
                i.stack[at] = match &op {
                    None => rhs,
                    Some(site) => {
                        let old = std::mem::take(&mut i.stack[at]);
                        match (&old, &rhs) {
                            (Value::Number(x, Tag::NONE), Value::Number(y, Tag::NONE)) => i.num_op(site.op, *x, *y)?,
                            _ => site.apply(i, old, rhs)?,
                        }
                    }
                };
                Ok(Flow::Normal)
            })
        }
        StK::Expr(e) => {
            let e = expression(e);
            Box::new(move |i| {
                i.tick()?;
                e(i)?;
                Ok(Flow::Normal)
            })
        }
        StK::Check(e, text) => {
            let (e, text) = (expression(e), text.clone());
            Box::new(move |i| {
                i.tick()?;
                match e(i)? {
                    Value::Bool(true, _) => Ok(Flow::Normal),
                    Value::Bool(false, _) => {
                        i.failed_check = Some((pos, text.clone()));
                        Ok(Flow::False)
                    }
                    other => {
                        i.pos = pos;
                        Err(i.error(format!("Precondition or predicate statement must evaluate to boolean, value is {}", other.error_type())))
                    }
                }
            })
        }
        StK::Block(b) => {
            let b: Vec<CSt> = b.iter().map(statement).collect();
            Box::new(move |i| {
                i.tick()?;
                run_block(i, &b)
            })
        }
        StK::If(c, t, o) => {
            let (c, t, o) = (expression(c), statement(t), o.as_deref().map(statement));
            Box::new(move |i| {
                i.tick()?;
                let v = c(i)?;
                if i.cond_value(v, "If statement condition", pos)? {
                    t(i)
                } else if let Some(o) = &o {
                    o(i)
                } else {
                    Ok(Flow::Normal)
                }
            })
        }
        StK::While(c, body) => {
            let (c, body) = (expression(c), statement(body));
            Box::new(move |i| {
                i.tick()?;
                loop {
                    let v = c(i)?;
                    if !i.cond_value(v, "While loop condition", pos)? {
                        break;
                    }
                    if let Some(out) = after_body(body(i)?) {
                        return Ok(out);
                    }
                }
                Ok(Flow::Normal)
            })
        }
        StK::For { init, cond, step, body } => {
            let (init, cond, step, body) = (init.as_deref().map(statement), cond.as_ref().map(expression), step.as_deref().map(statement), statement(body));
            Box::new(move |i| {
                i.tick()?;
                if let Some(s) = &init {
                    s(i)?;
                }
                loop {
                    if let Some(c) = &cond {
                        let v = c(i)?;
                        if !i.cond_value(v, "For loop condition", pos)? {
                            break;
                        }
                    }
                    if let Some(out) = after_body(body(i)?) {
                        return Ok(out);
                    }
                    if let Some(s) = &step {
                        s(i)?;
                    }
                }
                Ok(Flow::Normal)
            })
        }
        StK::ForIn { key, val, iter, body } => {
            let (key, val, iter, body) = (*key, *val as usize, expression(iter), statement(body));
            Box::new(move |i| {
                i.tick()?;
                let container = iter(i)?;
                let container = if container.is_compact() { container.shallow().into_owned() } else { container };
                let b = i.base;
                match container {
                    Value::Array(a, _) => {
                        for (n, x) in a.iter().enumerate() {
                            if let Some(k) = key {
                                i.stack[b + k as usize] = Value::num(n as f64);
                            }
                            i.stack[b + val] = x.clone();
                            if let Some(out) = after_body(body(i)?) {
                                return Ok(out);
                            }
                        }
                    }
                    Value::Map(m, _) => {
                        for (k, x) in m.iter() {
                            match key {
                                Some(ks) => {
                                    i.stack[b + ks as usize] = k.clone();
                                    i.stack[b + val] = x.clone();
                                }
                                None => {
                                    let mut e = Map::new();
                                    e.insert(Value::str("key"), k.clone());
                                    e.insert(Value::str("value"), x.clone());
                                    i.stack[b + val] = Value::map(e);
                                }
                            }
                            if let Some(out) = after_body(body(i)?) {
                                return Ok(out);
                            }
                        }
                    }
                    other => {
                        i.pos = pos;
                        return Err(i.error(format!("Iterator loop requires map or array, value is {}", other.error_type())));
                    }
                }
                Ok(Flow::Normal)
            })
        }
        StK::Return(e) => {
            let e = e.as_ref().map(expression);
            Box::new(move |i| {
                i.tick()?;
                let v = match &e {
                    Some(e) => e(i)?,
                    None => Value::Undefined,
                };
                // A wrong return type is reported at the return statement.
                i.pos = pos;
                Ok(Flow::Return(v))
            })
        }
        StK::Break => Box::new(|i| {
            i.tick()?;
            Ok(Flow::Break)
        }),
        StK::Continue => Box::new(|i| {
            i.tick()?;
            Ok(Flow::Continue)
        }),
        StK::Nop => Box::new(|i| {
            i.tick()?;
            Ok(Flow::Normal)
        }),
        // Assignments into a member or element, checks, throws, try: the tree walker.
        _ => walked_statement(s),
    }
}

/// The tree walker's assign_grown test: `x = append(x, …)`, `x = concatenateArrays(x, …)`, `x = @resize(x, …)`.
fn i_grows(slot: u32, callee: &Callee, args: &[Ex]) -> bool {
    let grows = match callee {
        Callee::Native(_, name) => &**name == "resize",
        Callee::Overloads(set) => matches!(&*set.name, "append" | "concatenateArrays"),
        _ => false,
    };
    grows && args.first().is_some_and(|e| matches!(e.k, ExK::Local(s) if s == slot))
}

pub(crate) fn expression(e: &Ex) -> CEx {
    let pos = e.pos;
    match &e.k {
        ExK::Lit(v) => {
            let v = v.clone();
            Box::new(move |_| Ok(v.clone()))
        }
        ExK::Local(s) => {
            let s = *s as usize;
            Box::new(move |i| Ok(i.stack[i.base + s].clone()))
        }
        ExK::Global(c) => {
            let c = c.clone();
            // (A length constant, std's `meter`, read compact once its value is known: no map shared per read.)
            let compact: std::sync::OnceLock<Option<Value>> = std::sync::OnceLock::new();
            Box::new(move |i| {
                if i.compact
                    && let Some(Some(v)) = compact.get()
                {
                    return Ok(v.clone());
                }
                i.pos = pos;
                let v = i.global_in(&c)?;
                if i.compact && compact.get().is_none() {
                    let _ = compact.set(crate::std_ops::quantity_of(&v));
                }
                Ok(v)
            })
        }
        ExK::Array(items) => {
            let items: Vec<CEx> = items.iter().map(expression).collect();
            Box::new(move |i| {
                let mut v = Vec::with_capacity(items.len());
                for x in &items {
                    v.push(x(i)?);
                }
                Ok(Value::array(v))
            })
        }
        ExK::Map(entries) => {
            let entries: Vec<(CEx, CEx)> = entries.iter().map(|(k, x)| (expression(k), expression(x))).collect();
            Box::new(move |i| {
                let mut m = Map::new();
                for (k, x) in &entries {
                    let k = k(i)?;
                    let x = x(i)?;
                    if x.is_undefined() {
                        m.remove(&k);
                    } else {
                        m.insert(k, x);
                    }
                }
                Ok(Value::map(m))
            })
        }
        ExK::Not(x) => {
            let x = expression(x);
            Box::new(move |i| match x(i)? {
                Value::Bool(b, _) => Ok(Value::bool(!b)),
                _ => {
                    i.pos = pos;
                    Err(i.error("Operand for '!' was not a boolean."))
                }
            })
        }
        ExK::Bin(op, a, b, ov) => {
            let (op, ov) = (*op, ov.clone());
            // Two locals or a local and a number: read in their slots (no copies when both are numbers).
            let number = |x: &Ex| match &x.k {
                ExK::Lit(Value::Number(n, Tag::NONE)) => Some(*n),
                _ => None,
            };
            let local = |x: &Ex| match x.k {
                ExK::Local(s) => Some(s as usize),
                _ => None,
            };
            let site = BinSite::new(op, ov);
            if let (Some(sa), Some(sb)) = (local(a), local(b)) {
                return Box::new(move |i| {
                    if let (Value::Number(x, Tag::NONE), Value::Number(y, Tag::NONE)) = (&i.stack[i.base + sa], &i.stack[i.base + sb]) {
                        let (x, y) = (*x, *y);
                        i.pos = pos;
                        return i.num_op(op, x, y);
                    }
                    let (va, vb) = (i.stack[i.base + sa].clone(), i.stack[i.base + sb].clone());
                    i.pos = pos;
                    site.apply(i, va, vb)
                });
            }
            if let (Some(sa), Some(y)) = (local(a), number(b)) {
                return Box::new(move |i| {
                    if let Value::Number(x, Tag::NONE) = &i.stack[i.base + sa] {
                        let x = *x;
                        i.pos = pos;
                        return i.num_op(op, x, y);
                    }
                    let va = i.stack[i.base + sa].clone();
                    i.pos = pos;
                    site.apply(i, va, Value::num(y))
                });
            }
            let (a, b) = (expression(a), expression(b));
            Box::new(move |i| {
                let va = a(i)?;
                let vb = b(i)?;
                i.pos = pos;
                if let (Value::Number(x, Tag::NONE), Value::Number(y, Tag::NONE)) = (&va, &vb) {
                    return i.num_op(op, *x, *y);
                }
                site.apply(i, va, vb)
            })
        }
        ExK::And(a, b) => {
            let (pa, pb) = (a.pos, b.pos);
            let (a, b) = (expression(a), expression(b));
            Box::new(move |i| {
                let v = a(i)?;
                if !i.cond_value(v, "Operand of &&", pa)? {
                    return Ok(Value::bool(false));
                }
                let v = b(i)?;
                Ok(Value::bool(i.cond_value(v, "Operand of &&", pb)?))
            })
        }
        ExK::Or(a, b) => {
            let (pa, pb) = (a.pos, b.pos);
            let (a, b) = (expression(a), expression(b));
            Box::new(move |i| {
                let v = a(i)?;
                if i.cond_value(v, "Operand of ||", pa)? {
                    return Ok(Value::bool(true));
                }
                let v = b(i)?;
                Ok(Value::bool(i.cond_value(v, "Operand of ||", pb)?))
            })
        }
        ExK::Coalesce(a, b) => {
            let (a, b) = (expression(a), expression(b));
            Box::new(move |i| {
                let v = a(i)?;
                if v.is_undefined() { b(i) } else { Ok(v) }
            })
        }
        ExK::Cond(c, a, b) => {
            let pc = c.pos;
            let (c, a, b) = (expression(c), expression(a), expression(b));
            Box::new(move |i| {
                let v = c(i)?;
                if i.cond_value(v, "First operand of logical or conditional operator", pc)? { a(i) } else { b(i) }
            })
        }
        ExK::Is(x, t) => {
            let (x, t) = (expression(x), *t);
            Box::new(move |i| {
                let v = x(i)?;
                Ok(Value::bool(t.matches(&v)))
            })
        }
        ExK::As(x, t) => {
            let (x, t) = (expression(x), *t);
            Box::new(move |i| {
                let v = x(i)?;
                i.pos = pos;
                i.as_type_in(v, t)
            })
        }
        // A function's arguments onto the stack, where its frame starts.
        ExK::Call(Callee::Overloads(set), args) => {
            let set = set.clone();
            let args: Vec<CEx> = args.iter().map(expression).collect();
            // (The site's std function kept for its arguments' types: no resolution per call.)
            let site: std::sync::OnceLock<Site> = std::sync::OnceLock::new();
            Box::new(move |i| {
                i.pos = pos;
                let base = i.stack.len();
                for x in &args {
                    match x(i) {
                        Ok(v) => i.stack.push(v),
                        Err(err) => {
                            i.discard_from(base);
                            return Err(err);
                        }
                    }
                }
                i.pos = pos;
                // (Numbers and compact values: their kinds stand for their signature.)
                let kinds = quick_sig(&i.stack[base..]);
                let sig = if kinds != 0 && site.get().is_some_and(|s| s.kinds == kinds) { site.get().map(|s| s.sig) } else { signature(&i.stack[base..]) };
                if let (Some(s), Some(sig)) = (site.get(), sig)
                    && s.sig == sig
                    && s.generation == crate::compile::overloads_generation()
                {
                    let compact = i.compact;
                    match s.op {
                        Some(op) => {
                            if let Some(v) = crate::std_ops::apply(op, &s.decl, &mut i.stack[base..], compact) {
                                i.stack.truncate(base);
                                return Ok(v);
                            }
                        }
                        // (A function: run on its arguments where they are.)
                        None => {
                            if let Some(c) = s.decl.compiled.get() {
                                return i.invoke_at(c, base);
                            }
                        }
                    }
                }
                let r = i.call_set_at(&set, base);
                if site.get().is_none()
                    && let Some(sig) = sig
                    && let Some(mut s) = i.std_site(None, Some(&set), sig)
                {
                    s.kinds = kinds;
                    let _ = site.set(s);
                }
                r
            })
        }
        ExK::Member(b, k, safe) => {
            let (k, safe) = (k.clone(), *safe);
            // (A local's member read in its slot: no copy of the map.)
            let slot = match b.k {
                ExK::Local(s) => Some(s as usize),
                _ => None,
            };
            let b = expression(b);
            Box::new(move |i| {
                if let Some(s) = slot {
                    match &i.stack[i.base + s] {
                        Value::Map(m, _) => return Ok(m.get(&k).cloned().unwrap_or_default()),
                        Value::Quantity(x, u) => return Ok(quantity_member(*x, *u, &k)),
                        _ => {}
                    }
                }
                match b(i)? {
                    Value::Map(m, _) => Ok(m.get(&k).cloned().unwrap_or_default()),
                    Value::Quantity(x, u) => Ok(quantity_member(x, u, &k)),
                    Value::Undefined if safe => Ok(Value::Undefined),
                    other => {
                        i.pos = pos;
                        Err(i.error(format!("Attempt to dereference non-container {}", crate::value::to_display_quoted(&other))))
                    }
                }
            })
        }
        ExK::Index(b, x, safe) => {
            let safe = *safe;
            let slot = match b.k {
                ExK::Local(s) => Some(s as usize),
                _ => None,
            };
            let (b, x) = (expression(b), expression(x));
            Box::new(move |i| {
                // (A local array's element read in its slot: no copy of the array. No expression changes a local.)
                if let Some(s) = slot
                    && matches!(i.stack[i.base + s], Value::Array(..) | Value::Vector(..))
                {
                    let idx = x(i)?;
                    i.pos = pos;
                    let at = i.base + s;
                    if let (Value::Array(a, _), Value::Number(n, _)) = (&i.stack[at], &idx)
                        && *n >= 0.0
                        && n.fract() == 0.0
                        && (*n as usize) < a.len()
                    {
                        return Ok(a[*n as usize].clone());
                    }
                    let base = i.stack[at].clone();
                    return i.index_in(base, &idx);
                }
                let base = b(i)?;
                if safe && base.is_undefined() {
                    return Ok(Value::Undefined);
                }
                let idx = x(i)?;
                i.pos = pos;
                i.index_in(base, &idx)
            })
        }
        ExK::Neg(x, _) => {
            let x = expression(x);
            Box::new(move |i| {
                let v = x(i)?;
                i.pos = pos;
                i.negated(v)
            })
        }
        ExK::Call(Callee::Native(n, name), args) => {
            let (n, name, kind) = (*n, name.clone(), native_args(name));
            let mut args: Vec<CEx> = args.iter().map(expression).collect();
            // (@size of an array, a map or a compact value: its length, without a vector of arguments.)
            if matches!(kind, NativeArgs::Size)
                && args.len() == 1
                && let Some(x) = args.pop()
            {
                return Box::new(move |i| {
                    i.pos = pos;
                    let v = x(i)?;
                    if let Some(len) = size_of(&v) {
                        return Ok(Value::num(len as f64));
                    }
                    i.pos = pos;
                    i.call_native_as(n, &name, vec![v], kind)
                });
            }
            Box::new(move |i| {
                i.pos = pos;
                let mut a = Vec::with_capacity(args.len());
                for x in &args {
                    a.push(x(i)?);
                }
                i.pos = pos;
                i.call_native_as(n, &name, a, kind)
            })
        }
        ExK::Call(Callee::Value(fx), args) => {
            let fx = expression(fx);
            let args: Vec<CEx> = args.iter().map(expression).collect();
            Box::new(move |i| {
                i.pos = pos;
                let f = fx(i)?;
                let mut a = Vec::with_capacity(args.len());
                for x in &args {
                    a.push(x(i)?);
                }
                i.pos = pos;
                i.call_value_in(&f, a)
            })
        }
        // Missing functions, function values, boxes, lambdas, try, switch: the tree walker.
        _ => walked(e),
    }
}
