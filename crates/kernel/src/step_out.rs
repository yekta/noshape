//! STEP files laid out like Onshape's export (live G47): OCCT writes each
//! part's geometry, then this module rebuilds the file the way Onshape's
//! writer does: one independent product per part (no assembly, last part
//! first), each with its own representation context, the part's name on the
//! product, its shape representation and its solid, a colour per part,
//! entities grouped by type and numbered from #10, Onshape's header and line
//! wrapping.

use std::collections::HashMap;
use std::rc::Rc;

use crate::error::{KernelError, Result};

/// A value in an entity's parameter list.
#[derive(Clone, Debug, PartialEq)]
enum V {
    /// An entity reference: OCCT's number while parsing, an output index after.
    Ref(u32),
    /// String contents, quotes doubled as in the file.
    Str(String),
    Enum(String),
    Int(String),
    Real(f64),
    Typed(String, Vec<V>),
    List(Vec<V>),
    Dollar,
    Star,
}

/// An entity: one record, or several for a complex entity.
#[derive(Clone, Debug)]
struct Ent {
    recs: Vec<(String, Vec<V>)>,
    complex: bool,
}

impl Ent {
    fn simple(t: &str, args: Vec<V>) -> Ent {
        Ent { recs: vec![(t.to_string(), args)], complex: false }
    }
    fn ty(&self) -> &str {
        &self.recs[0].0
    }
    fn args(&self) -> &[V] {
        &self.recs[0].1
    }
    fn rec(&self, t: &str) -> Option<&[V]> {
        self.recs.iter().find(|r| r.0 == t).map(|r| r.1.as_slice())
    }
    /// The sort key of its type: the record name, or the complex entity's
    /// record names joined.
    fn key(&self) -> String {
        if self.complex { self.recs.iter().map(|r| r.0.as_str()).collect::<Vec<_>>().join(" ") } else { self.recs[0].0.clone() }
    }
}

fn bad(what: &str) -> KernelError {
    KernelError::Io(format!("could not read OCCT's STEP output: {what}"))
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        loop {
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"/*") {
                match self.s[self.i + 2..].windows(2).position(|w| w == b"*/") {
                    Some(p) => self.i += p + 4,
                    None => self.i = self.s.len(),
                }
            } else {
                return;
            }
        }
    }
    fn peek(&mut self) -> u8 {
        self.ws();
        self.s.get(self.i).copied().unwrap_or(0)
    }
    fn eat(&mut self, c: u8) -> Result<()> {
        if self.peek() != c {
            return Err(bad(&format!("expected '{}' at {}", c as char, self.i)));
        }
        self.i += 1;
        Ok(())
    }
    fn word(&mut self) -> String {
        self.ws();
        let st = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_') {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[st..self.i]).into_owned()
    }
    fn num(&mut self) -> u32 {
        let st = self.i;
        while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[st..self.i]).ok().and_then(|t| t.parse().ok()).unwrap_or(0)
    }
    fn args(&mut self) -> Result<Vec<V>> {
        self.eat(b'(')?;
        let mut out = Vec::new();
        if self.peek() == b')' {
            self.i += 1;
            return Ok(out);
        }
        loop {
            out.push(self.value()?);
            match self.peek() {
                b',' => self.i += 1,
                b')' => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => return Err(bad(&format!("bad list at {}", self.i))),
            }
        }
    }
    fn value(&mut self) -> Result<V> {
        let c = self.peek();
        Ok(match c {
            b'#' => {
                self.i += 1;
                V::Ref(self.num())
            }
            b'\'' => {
                self.i += 1;
                let st = self.i;
                loop {
                    match self.s.get(self.i) {
                        None => return Err(bad("unterminated string")),
                        Some(b'\'') if self.s.get(self.i + 1) == Some(&b'\'') => self.i += 2,
                        Some(b'\'') => break,
                        _ => self.i += 1,
                    }
                }
                let t = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                self.i += 1;
                V::Str(t)
            }
            b'.' if self.s.get(self.i + 1).is_some_and(|b| b.is_ascii_alphabetic()) => {
                self.i += 1;
                let w = self.word();
                self.eat(b'.')?;
                V::Enum(w)
            }
            b'$' => {
                self.i += 1;
                V::Dollar
            }
            b'*' => {
                self.i += 1;
                V::Star
            }
            b'(' => V::List(self.args()?),
            c if c.is_ascii_alphabetic() => {
                let w = self.word();
                V::Typed(w, self.args()?)
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'E' | b'e') {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.s[st..self.i]).map_err(|_| bad("number"))?;
                if t.is_empty() {
                    return Err(bad(&format!("unexpected '{}' at {}", c as char, st)));
                }
                if t.contains(['.', 'E', 'e']) { V::Real(t.parse().map_err(|_| bad(t))?) } else { V::Int(t.to_string()) }
            }
        })
    }
}

/// The DATA section's entities by number.
fn parse(text: &str) -> Result<HashMap<u32, Ent>> {
    let start = text.find("DATA;").ok_or_else(|| bad("no DATA section"))? + 5;
    let mut p = Parser { s: text.as_bytes(), i: start };
    let mut out = HashMap::new();
    loop {
        match p.peek() {
            b'#' => {
                p.i += 1;
                let id = p.num();
                p.eat(b'=')?;
                let ent = if p.peek() == b'(' {
                    p.i += 1;
                    let mut recs = Vec::new();
                    while p.peek() != b')' {
                        let w = p.word();
                        if w.is_empty() {
                            return Err(bad("complex entity"));
                        }
                        recs.push((w, p.args()?));
                    }
                    p.i += 1;
                    Ent { recs, complex: true }
                } else {
                    let w = p.word();
                    Ent { recs: vec![(w, p.args()?)], complex: false }
                };
                p.eat(b';')?;
                out.insert(id, ent);
            }
            _ => return Ok(out),
        }
    }
}

fn refs(v: &[V], out: &mut Vec<u32>) {
    for x in v {
        match x {
            V::Ref(r) => out.push(*r),
            V::List(l) | V::Typed(_, l) => refs(l, out),
            _ => {}
        }
    }
}

fn map_refs(v: &mut [V], f: &impl Fn(u32) -> u32) {
    for x in v {
        match x {
            V::Ref(r) => *r = f(*r),
            V::List(l) | V::Typed(_, l) => map_refs(l, f),
            _ => {}
        }
    }
}

/// Onshape's real format: 15 significant digits, "0.", "1.E-8".
fn real(x: f64) -> String {
    if x == 0.0 {
        return "0.".into();
    }
    let sci = format!("{:.14E}", x);
    let (mant, exp) = sci.split_once('E').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let trim = |m: &str| -> String {
        let m = m.trim_end_matches('0');
        m.to_string()
    };
    if !(-4..15).contains(&exp) {
        format!("{}E{}", trim(mant), exp)
    } else {
        let dec = (14 - exp).max(0) as usize;
        let f = format!("{:.*}", dec, x);
        let f = if f.contains('.') { trim(&f) } else { format!("{f}.") };
        if f == "-0." { "0.".into() } else { f }
    }
}

/// Onshape's line layout: references, strings and enumerations move to a
/// new line when they would pass column 72 (enumerations indented by one
/// space); after a separator past column 72 the line breaks.
struct Out {
    text: String,
    col: usize,
}

impl Out {
    fn raw(&mut self, s: &str) {
        self.text.push_str(s);
        self.col += s.len();
    }
    fn nl(&mut self) {
        self.text.push('\n');
        self.col = 0;
    }
    fn fits(&mut self, s: &str, indent: &str) {
        if self.col > 0 && self.col + s.len() > 72 {
            self.nl();
            self.raw(indent);
        }
        self.raw(s);
    }
    /// A string: on a new line when it would pass column 72, and broken
    /// wherever it reaches column 70 with characters left (Onshape's
    /// 72-character application context of AP203 is split "…assemblie" / "s'").
    fn string(&mut self, t: &str) {
        if self.col > 0 && self.col + t.len() > 72 {
            self.nl();
        }
        for ch in t.chars() {
            if self.col >= 70 {
                self.nl();
            }
            self.text.push(ch);
            self.col += 1;
        }
    }
    fn sep(&mut self) {
        self.raw(",");
        if self.col > 72 {
            self.nl();
        }
    }
    fn values(&mut self, v: &[V]) {
        self.raw("(");
        for (i, x) in v.iter().enumerate() {
            if i > 0 {
                self.sep();
            }
            self.value(x);
        }
        self.raw(")");
    }
    fn value(&mut self, x: &V) {
        match x {
            V::Ref(r) => self.fits(&format!("#{r}"), ""),
            V::Str(s) => self.string(&format!("'{s}'")),
            V::Enum(e) => self.fits(&format!(".{e}."), " "),
            V::Int(i) => self.raw(i),
            V::Real(r) => self.raw(&real(*r)),
            V::Typed(t, a) => {
                self.raw(t);
                self.values(a);
            }
            V::List(l) => self.values(l),
            V::Dollar => self.raw("$"),
            V::Star => self.raw("*"),
        }
    }
    fn entity(&mut self, id: usize, e: &Ent) {
        self.raw(&format!("#{id}="));
        if e.complex {
            self.raw("(");
            self.nl();
            for (t, a) in &e.recs {
                self.raw(t);
                self.values(a);
                self.nl();
            }
            self.raw(");");
        } else {
            self.raw(e.ty());
            self.values(e.args());
            self.raw(";");
        }
        self.nl();
    }
}

/// A STEP file OCCT wrote, parsed: one part or many (one product each).
pub(crate) struct Occt(HashMap<u32, Ent>);

impl Occt {
    pub fn parse(text: &str) -> Result<Rc<Occt>> {
        Ok(Rc::new(Occt(parse(text)?)))
    }
    /// The (first) shape definition representation: a single part's.
    pub fn sdr(&self) -> Result<u32> {
        self.0.iter().filter(|(_, e)| e.ty() == "SHAPE_DEFINITION_REPRESENTATION").map(|(id, _)| *id).min().ok_or_else(|| bad("no shape representation"))
    }
    /// Each product's shape definition representation by the product's name.
    pub fn sdrs_by_product(&self) -> HashMap<String, u32> {
        let arg = |id: &V, k: usize| match id {
            V::Ref(r) => self.0.get(r).and_then(|e| e.args().get(k)),
            _ => None,
        };
        let mut out = HashMap::new();
        for (id, e) in &self.0 {
            if e.ty() != "SHAPE_DEFINITION_REPRESENTATION" {
                continue;
            }
            // SDR -> PRODUCT_DEFINITION_SHAPE -> PRODUCT_DEFINITION -> FORMATION -> PRODUCT.
            let product = e.args().first().and_then(|pds| arg(pds, 2)).and_then(|pd| arg(pd, 2)).and_then(|pdf| arg(pdf, 2)).and_then(|prod| arg(prod, 0));
            if let Some(V::Str(n)) = product {
                out.insert(n.clone(), *id);
            }
        }
        out
    }
}

/// One part to write: OCCT's text holding it (its shape definition
/// representation `sdr`) and Onshape's part data.
pub(crate) struct Part {
    pub name: String,
    /// sRGB 0..1.
    pub color: Option<[f32; 3]>,
    pub occt: Rc<Occt>,
    pub sdr: u32,
    /// Its model definitions (AP242 only).
    pub pmi: Option<crate::io::PartPmi>,
}

/// The application protocol, with Onshape's header and context strings for it
/// (live G47 / G48).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Schema {
    Ap242,
    Ap214,
    Ap203,
}

impl Schema {
    fn description(self) -> &'static str {
        match self {
            Schema::Ap242 => "('STEP AP242 Edition 2',\n'CAx-IF Rec.Pracs.---Representation and Presentation of Product Manufa\ncturing Information (PMI)---4.0---2014-10-13',\n'CAx-IF Rec.Pracs.---3D Tessellated Geometry---0.4---2014-09-14','2;1',\n'CAx-IF Rec.Pracs.---User Defined Attributes---1.5---2016-08-15')",
            Schema::Ap214 => "('STEP AP214',\n'CAx-IF Rec.Pracs.---User Defined Attributes---1.5---2016-08-15')",
            Schema::Ap203 => "('STEP AP203')",
        }
    }
    fn file_schema(self) -> &'static str {
        match self {
            Schema::Ap242 => "AP242_MANAGED_MODEL_BASED_3D_ENGINEERING_MIM_LF { 1 0 10303 442 3 1 4 }",
            Schema::Ap214 => "AUTOMOTIVE_DESIGN {1 0 10303 214 3 1 1}",
            Schema::Ap203 => "CONFIG_CONTROL_DESIGN",
        }
    }
    /// Application context, protocol name and year.
    fn application(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Schema::Ap242 => ("managed model based 3d engineering", "ap242_managed_model_based_3d_engineering", "2020"),
            Schema::Ap214 => ("core data for automotive mechanical design processes", "automotive_design", "2010"),
            Schema::Ap203 => ("configuration controlled 3D designs of mechanical parts and assemblies", "config_control_design", "2010"),
        }
    }
}

/// Entities in the order Onshape's writer creates them (the index).
#[derive(Default)]
struct Builder {
    ents: Vec<Ent>,
    /// Entities numbered after the grouped ones, in creation order (MBD,
    /// as Onshape appends it).
    tail: Vec<bool>,
}

impl Builder {
    fn add(&mut self, e: Ent) -> u32 {
        self.ents.push(e);
        self.tail.push(false);
        (self.ents.len() - 1) as u32
    }
    fn add_tail(&mut self, e: Ent) -> u32 {
        self.ents.push(e);
        self.tail.push(true);
        (self.ents.len() - 1) as u32
    }
    fn reserve(&mut self) -> u32 {
        self.add(Ent::simple("", vec![]))
    }
    fn set(&mut self, i: u32, e: Ent) {
        self.ents[i as usize] = e;
    }
}

fn s(t: &str) -> V {
    V::Str(t.replace('\'', "''"))
}

fn list(ids: &[u32]) -> V {
    V::List(ids.iter().map(|&i| V::Ref(i)).collect())
}

/// OCCT's entities of one part copied in Onshape's creation order: solids,
/// shells, faces, bounds and loops before what they hold, a face's surface
/// before its bounds, everything else after what it references (an edge
/// after its curve and then its vertices).
struct Copier<'a> {
    src: &'a HashMap<u32, Ent>,
    map: HashMap<u32, u32>,
}

impl Copier<'_> {
    fn top_down(t: &str) -> bool {
        matches!(
            t,
            "MANIFOLD_SOLID_BREP" | "BREP_WITH_VOIDS" | "CLOSED_SHELL" | "OPEN_SHELL" | "ORIENTED_CLOSED_SHELL" | "ORIENTED_OPEN_SHELL" | "SHELL_BASED_SURFACE_MODEL" | "ADVANCED_FACE" | "FACE_SURFACE" | "FACE_BOUND" | "FACE_OUTER_BOUND" | "EDGE_LOOP" | "VERTEX_LOOP" | "GEOMETRIC_CURVE_SET"
        )
    }
    fn children(e: &Ent) -> Vec<u32> {
        let mut out = Vec::new();
        let a = e.args();
        match e.ty() {
            "ADVANCED_FACE" | "FACE_SURFACE" if !e.complex && a.len() >= 3 => {
                refs(&a[2..3], &mut out);
                refs(&a[1..2], &mut out);
                refs(&a[3..], &mut out);
            }
            "EDGE_CURVE" if !e.complex && a.len() >= 4 => {
                refs(&a[3..4], &mut out);
                refs(&a[1..3], &mut out);
                refs(&a[4..], &mut out);
            }
            _ => {
                for rec in &e.recs {
                    refs(&rec.1, &mut out);
                }
            }
        }
        out
    }
    fn get(&self, id: u32) -> Result<&Ent> {
        self.src.get(&id).ok_or_else(|| bad(&format!("missing #{id}")))
    }
    /// Copies `id` and what it references; `made` runs right after the copy
    /// of `id` itself is created.
    fn visit(&mut self, b: &mut Builder, id: u32, made: &mut dyn FnMut(&mut Builder, u32)) -> Result<u32> {
        if let Some(&i) = self.map.get(&id) {
            return Ok(i);
        }
        let e = self.get(id)?.clone();
        let td = Self::top_down(e.ty());
        if td {
            let i = b.reserve();
            self.map.insert(id, i);
            made(b, i);
        }
        for c in Self::children(&e) {
            self.visit(b, c, &mut |_, _| {})?;
        }
        if !td {
            let i = b.reserve();
            self.map.insert(id, i);
            made(b, i);
        }
        let mut e = e;
        for rec in &mut e.recs {
            map_refs(&mut rec.1, &|r| self.map[&r]);
        }
        let i = self.map[&id];
        b.set(i, e);
        Ok(i)
    }
}


/// The uncertainty OCCT wrote in a representation context: its shape's
/// greatest tolerance (the writer's Greatest precision mode), in the file's
/// unit.
fn occt_uncertainty(src: &HashMap<u32, Ent>, ctx: u32) -> Option<f64> {
    let Some(V::List(l)) = src.get(&ctx)?.rec("GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT")?.first() else { return None };
    let value = |e: &Ent| match e.args().first() {
        Some(V::Typed(_, a)) => match a.first() {
            Some(V::Real(x)) => Some(*x),
            _ => None,
        },
        Some(V::Real(x)) => Some(*x),
        _ => None,
    };
    l.iter().filter_map(|v| if let V::Ref(r) = v { src.get(r) } else { None }).filter_map(value).reduce(f64::max)
}

/// A part's uncertainty: Onshape's 1e-8 m, or the part's greatest tolerance
/// when it is over 1 µm ("Maximum Tolerance applied to model"; modelled
/// parts stay at OCCT's 1e-7 m, imported healed ones go higher). A healed
/// imported part (NIST's cap screw) declared at 1e-8 m made readers heal it
/// again at that precision: 20 s to read one screw, 0.5 s at its own
/// tolerance.
fn uncertainty(src: &HashMap<u32, Ent>, ctx: u32, metres: f64) -> f64 {
    let onshape = 1e-8 / metres;
    match occt_uncertainty(src, ctx) {
        Some(u) if u * metres > 1e-6 => u,
        _ => onshape,
    }
}

/// The whole file in Onshape's layout.
pub(crate) fn write(parts: &[Part], file_name: &str, timestamp: &str, schema: Schema) -> Result<String> {
    let mut b = Builder::default();
    let (app_context, protocol, year) = schema.application();
    let mut last = None;
    // Onshape writes the last part first.
    for part in parts.iter().rev() {
        let src = &part.occt.0;
        let get = |id: u32| src.get(&id).ok_or_else(|| bad(&format!("missing #{id}")));
        // OCCT's representation of the part and its context.
        let sdr = get(part.sdr)?;
        let Some(V::Ref(rep_id)) = sdr.args().get(1) else { return Err(bad("shape representation")) };
        let rep = get(*rep_id)?;
        let (Some(V::List(items)), Some(V::Ref(ctx_id))) = (rep.args().get(1), rep.args().get(2)) else { return Err(bad("representation items")) };
        let items: Vec<u32> = items.iter().filter_map(|v| if let V::Ref(r) = v { Some(*r) } else { None }).filter(|r| src.get(r).is_some_and(|e| e.ty() != "AXIS2_PLACEMENT_3D")).collect();
        let units: Vec<u32> = match get(*ctx_id)?.rec("GLOBAL_UNIT_ASSIGNED_CONTEXT").and_then(|a| a.first()) {
            Some(V::List(l)) => l.iter().filter_map(|v| if let V::Ref(r) = v { Some(*r) } else { None }).collect(),
            _ => return Err(bad("units")),
        };
        let name = s(&part.name);
        let mut cp = Copier { src, map: HashMap::new() };

        // Product data.
        let ac = b.add(Ent::simple("APPLICATION_CONTEXT", vec![s(app_context)]));
        b.add(Ent::simple("APPLICATION_PROTOCOL_DEFINITION", vec![s("international standard"), s(protocol), V::Int(year.into()), V::Ref(ac)]));
        let pc = b.add(Ent::simple(if schema == Schema::Ap203 { "MECHANICAL_CONTEXT" } else { "PRODUCT_CONTEXT" }, vec![s(""), V::Ref(ac), s("mechanical")]));
        b.add(Ent::simple("PRODUCT_CATEGORY", vec![s(""), s("")]));
        let prod = b.add(Ent::simple("PRODUCT", vec![name.clone(), name.clone(), name.clone(), list(&[pc])]));
        b.add(Ent::simple("PRODUCT_RELATED_PRODUCT_CATEGORY", vec![s(""), s(""), list(&[prod])]));
        let pdf = b.add(Ent::simple("PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE", vec![s(""), s(""), V::Ref(prod), V::Enum("NOT_KNOWN".into())]));
        let pdc = b.add(Ent::simple(if schema == Schema::Ap203 { "DESIGN_CONTEXT" } else { "PRODUCT_DEFINITION_CONTEXT" }, vec![s(""), V::Ref(ac), s("design")]));
        let pd = b.add(Ent::simple("PRODUCT_DEFINITION", vec![s(""), s(""), V::Ref(pdf), V::Ref(pdc)]));
        let pds = b.add(Ent::simple("PRODUCT_DEFINITION_SHAPE", vec![s(""), s(""), V::Ref(pd)]));

        // Units (OCCT's), uncertainty (1e-8 m in the file's unit) and context.
        let unit_ids: Vec<u32> = units.iter().map(|&u| cp.visit(&mut b, u, &mut |_, _| {})).collect::<Result<_>>()?;
        let len_unit = *unit_ids.first().ok_or_else(|| bad("length unit"))?;
        let metres = {
            let e = &b.ents[len_unit as usize];
            match (e.rec("SI_UNIT"), e.rec("CONVERSION_BASED_UNIT")) {
                (Some([V::Enum(p), _]), _) => match p.as_str() {
                    "MILLI" => 1e-3,
                    "CENTI" => 1e-2,
                    "KILO" => 1e3,
                    _ => 1.0,
                },
                (_, Some([V::Str(n), ..])) => match n.as_str() {
                    "INCH" => 0.0254,
                    "FOOT" => 0.3048,
                    "YARD" => 0.9144,
                    _ => 1.0,
                },
                _ => 1.0,
            }
        };
        let unc = b.add(Ent::simple("UNCERTAINTY_MEASURE_WITH_UNIT", vec![V::Typed("LENGTH_MEASURE".into(), vec![V::Real(uncertainty(src, *ctx_id, metres))]), V::Ref(len_unit), s("DISTANCE_ACCURACY_VALUE"), s("Maximum Tolerance applied to model")]));
        let ctx = b.add(Ent {
            recs: vec![
                ("GEOMETRIC_REPRESENTATION_CONTEXT".into(), vec![V::Int("3".into())]),
                ("GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT".into(), vec![list(&[unc])]),
                ("GLOBAL_UNIT_ASSIGNED_CONTEXT".into(), vec![list(&unit_ids)]),
                ("REPRESENTATION_CONTEXT".into(), vec![name.clone(), s("TOP_LEVEL_ASSEMBLY_PART")]),
            ],
            complex: true,
        });
        let mdgpr = part.color.map(|_| b.reserve());

        // The shape representation and its placement.
        let p0 = b.add(Ent::simple("CARTESIAN_POINT", vec![s(""), V::List(vec![V::Real(0.0), V::Real(0.0), V::Real(0.0)])]));
        let dz = b.add(Ent::simple("DIRECTION", vec![s(""), V::List(vec![V::Real(0.0), V::Real(0.0), V::Real(1.0)])]));
        let dx = b.add(Ent::simple("DIRECTION", vec![s(""), V::List(vec![V::Real(1.0), V::Real(0.0), V::Real(0.0)])]));
        let ax = b.add(Ent::simple("AXIS2_PLACEMENT_3D", vec![s(""), V::Ref(p0), V::Ref(dz), V::Ref(dx)]));
        let sr = b.add(Ent::simple("SHAPE_REPRESENTATION", vec![name.clone(), list(&[ax]), V::Ref(ctx)]));
        b.add(Ent::simple("SHAPE_DEFINITION_REPRESENTATION", vec![V::Ref(pds), V::Ref(sr)]));

        // The solid (or sheet, wire) with its colour, then its topology.
        let rgb: Option<Vec<V>> = part.color.map(|c| std::iter::once(s("")).chain(c.iter().map(|&x| V::Real((f64::from(x.clamp(0.0, 1.0)) * 255.0).round() / 255.0))).collect());
        let mut styled = Vec::new();
        let mut solids = Vec::new();
        for &it in &items {
            let i = cp.visit(&mut b, it, &mut |b, item| {
                let Some(rgb) = &rgb else { return };
                let col = b.add(Ent::simple("COLOUR_RGB", rgb.clone()));
                let fasc = b.add(Ent::simple("FILL_AREA_STYLE_COLOUR", vec![s(""), V::Ref(col)]));
                let fas = b.add(Ent::simple("FILL_AREA_STYLE", vec![s(""), list(&[fasc])]));
                let ssfa = b.add(Ent::simple("SURFACE_STYLE_FILL_AREA", vec![V::Ref(fas)]));
                let sss = b.add(Ent::simple("SURFACE_SIDE_STYLE", vec![s(""), list(&[ssfa])]));
                let ssu = b.add(Ent::simple("SURFACE_STYLE_USAGE", vec![V::Enum("BOTH".into()), V::Ref(sss)]));
                let psa = b.add(Ent::simple("PRESENTATION_STYLE_ASSIGNMENT", vec![list(&[ssu])]));
                styled.push(b.add(Ent::simple("STYLED_ITEM", vec![s(""), list(&[psa]), V::Ref(item)])));
            })?;
            if let Some(V::Str(n)) = b.ents[i as usize].recs[0].1.first_mut() {
                *n = part.name.replace('\'', "''");
            }
            solids.push(i);
        }
        let mut rep_o = rep.clone();
        rep_o.recs[0].1 = vec![s(""), list(&solids), V::Ref(ctx)];
        let absr = b.add(rep_o);
        b.add(Ent::simple("SHAPE_REPRESENTATION_RELATIONSHIP", vec![s(""), s(""), V::Ref(sr), V::Ref(absr)]));
        if let Some(m) = mdgpr {
            b.set(m, Ent::simple("MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION", vec![s(""), list(&styled), V::Ref(ctx)]));
        }
        // AP242 MBD (live 1746): semantic PMI on this product's faces, with
        // its tessellated presentation.
        if schema == Schema::Ap242
            && let Some(pmi) = &part.pmi
        {
            let faces: Vec<u32> = items.iter().flat_map(|&it| face_order(src, it)).filter_map(|f| cp.map.get(&f).copied()).collect();
            write_pmi(&mut b, pmi, &Pmi { pds, absr, ctx, solids: &solids, faces: &faces, metres });
        }
        last = Some((pds, ctx));
    }
    // AP214 / AP203: Onshape's PMI validation properties, on the part written last.
    if schema != Schema::Ap242
        && let Some((pds, ctx)) = last
    {
        for what in ["number of annotations", "number of views"] {
            let vri = b.add(Ent::simple("VALUE_REPRESENTATION_ITEM", vec![s(what), V::Typed("COUNT_MEASURE".into(), vec![V::Real(0.0)])]));
            let def = b.add(Ent::simple("PROPERTY_DEFINITION", vec![s("pmi validation property"), s(""), V::Ref(pds)]));
            let rep = b.add(Ent::simple("REPRESENTATION", vec![s(""), list(&[vri]), V::Ref(ctx)]));
            b.add(Ent::simple("PROPERTY_DEFINITION_REPRESENTATION", vec![V::Ref(def), V::Ref(rep)]));
        }
    }

    Ok(numbered(&b, file_name, timestamp, schema))
}

/// A solid's faces in OCCT's order (its shells', the outer first): the
/// order of `TopExp::MapShapes`, which PMI face indices use.
fn face_order(src: &HashMap<u32, Ent>, item: u32) -> Vec<u32> {
    let refs_of = |v: Option<&V>| -> Vec<u32> {
        match v {
            Some(V::List(l)) => l.iter().filter_map(|x| if let V::Ref(r) = x { Some(*r) } else { None }).collect(),
            Some(V::Ref(r)) => vec![*r],
            _ => Vec::new(),
        }
    };
    let shell_faces = |id: u32| -> Vec<u32> {
        let Some(e) = src.get(&id) else { return Vec::new() };
        match e.ty() {
            "ORIENTED_CLOSED_SHELL" | "ORIENTED_OPEN_SHELL" => refs_of(e.args().get(2)).into_iter().flat_map(|sh| src.get(&sh).map(|x| refs_of(x.args().get(1))).unwrap_or_default()).collect(),
            _ => refs_of(e.args().get(1)),
        }
    };
    let Some(e) = src.get(&item) else { return Vec::new() };
    match e.ty() {
        "MANIFOLD_SOLID_BREP" => refs_of(e.args().get(1)).into_iter().flat_map(shell_faces).collect(),
        "BREP_WITH_VOIDS" => refs_of(e.args().get(1)).into_iter().chain(refs_of(e.args().get(2))).flat_map(shell_faces).collect(),
        "SHELL_BASED_SURFACE_MODEL" => refs_of(e.args().get(1)).into_iter().flat_map(shell_faces).collect(),
        _ => Vec::new(),
    }
}

/// Where a part's MBD goes: its product definition shape, B-rep
/// representation and context, solids, faces (by 0-based index) and the
/// file's metres per unit.
struct Pmi<'a> {
    pds: u32,
    absr: u32,
    ctx: u32,
    solids: &'a [u32],
    faces: &'a [u32],
    metres: f64,
}

/// A part's MBD in Onshape's layout (live 1746, CAx-IF recommended practices
/// for PMI representation and presentation): per datum DATUM, DATUM_FEATURE
/// and its face; per dimension SHAPE_ASPECTs on its faces, the
/// DIMENSIONAL_LOCATION / _SIZE or ANGULAR_LOCATION with its value (mm or
/// degrees, with its precision) and PLUS_MINUS_TOLERANCE; each drawn as a
/// DRAUGHTING_CALLOUT of tessellated text, arrowheads and lines in its
/// ANNOTATION_PLANE, all in one DRAUGHTING_MODEL.
fn write_pmi(b: &mut Builder, pmi: &crate::io::PartPmi, at: &Pmi) {
    use crate::io::PmiKind;
    let mut model_items: Vec<u32> = Vec::new();
    let mut model: Option<u32> = None;
    // An aspect on faces (1-based indices), each face by its own usage.
    let aspect = |b: &mut Builder, name: &str, faces: &[u32]| -> u32 {
        let a = b.add_tail(Ent::simple("SHAPE_ASPECT", vec![s(name), V::Dollar, V::Ref(at.pds), V::Enum("T".into())]));
        for f in faces.iter().filter_map(|&f| at.faces.get((f as usize).wrapping_sub(1))) {
            b.add_tail(Ent::simple("GEOMETRIC_ITEM_SPECIFIC_USAGE", vec![s(""), s(""), V::Ref(a), V::Ref(at.absr), V::Ref(*f)]));
        }
        a
    };
    let mm = |b: &mut Builder| b.add_tail(Ent { recs: vec![("LENGTH_UNIT".into(), vec![]), ("NAMED_UNIT".into(), vec![V::Star]), ("SI_UNIT".into(), vec![V::Enum("MILLI".into()), V::Enum("METRE".into())])], complex: true });
    let degree = |b: &mut Builder| {
        let rad = b.add_tail(Ent { recs: vec![("NAMED_UNIT".into(), vec![V::Star]), ("PLANE_ANGLE_UNIT".into(), vec![]), ("SI_UNIT".into(), vec![V::Dollar, V::Enum("RADIAN".into())])], complex: true });
        let factor = b.add_tail(Ent::simple("LENGTH_MEASURE_WITH_UNIT", vec![V::Typed("LENGTH_MEASURE".into(), vec![V::Real(180.0 / std::f64::consts::PI)]), V::Ref(rad)]));
        b.add_tail(Ent { recs: vec![("CONVERSION_BASED_UNIT".into(), vec![s("degree"), V::Ref(factor)]), ("NAMED_UNIT".into(), vec![V::Dollar]), ("PLANE_ANGLE_UNIT".into(), vec![])], complex: true })
    };
    for d in &pmi.datums {
        let name = s(&format!("Datum {}", d.letter));
        let datum = b.add_tail(Ent::simple("DATUM", vec![name.clone(), name, V::Ref(at.pds), V::Enum("F".into()), s(&d.letter)]));
        let fname = s(&format!("Datum_feature {}", d.letter));
        let feature = b.add_tail(Ent::simple("DATUM_FEATURE", vec![fname.clone(), fname, V::Ref(at.pds), V::Enum("T".into())]));
        b.add_tail(Ent::simple("SHAPE_ASPECT_RELATIONSHIP", vec![s(""), s(""), V::Ref(feature), V::Ref(datum)]));
        for f in d.faces.iter().filter_map(|&f| at.faces.get((f as usize).wrapping_sub(1))) {
            b.add_tail(Ent::simple("GEOMETRIC_ITEM_SPECIFIC_USAGE", vec![s(""), s(""), V::Ref(feature), V::Ref(at.absr), V::Ref(*f)]));
        }
        if let Some(p) = &d.presentation {
            let callout = present(b, at, p, &mut model_items, &mut model);
            b.add_tail(Ent::simple("DRAUGHTING_MODEL_ITEM_ASSOCIATION", vec![s(""), s(""), V::Ref(feature), V::Ref(model.unwrap_or(0)), V::Ref(callout)]));
        }
    }
    for d in &pmi.dims {
        let angle = d.kind == PmiKind::Angle;
        let (dim, aspects) = match d.kind {
            PmiKind::Radius | PmiKind::Diameter => {
                let a = aspect(b, &d.name, &d.a);
                let what = if d.kind == PmiKind::Radius { "radius" } else { "diameter" };
                (b.add_tail(Ent::simple("DIMENSIONAL_SIZE", vec![V::Ref(a), s(what)])), vec![a])
            }
            _ => {
                let a = aspect(b, &d.name, &d.a);
                let c = aspect(b, &d.name, &d.b);
                let e = match d.kind {
                    PmiKind::Angle => Ent::simple("ANGULAR_LOCATION", vec![s("angle"), s(&d.name), V::Ref(a), V::Ref(c), V::Enum("EQUAL".into())]),
                    PmiKind::AxisDistance => Ent::simple("DIMENSIONAL_LOCATION", vec![s("linear distance centre"), s(&d.name), V::Ref(a), V::Ref(c)]),
                    _ => Ent::simple("DIMENSIONAL_LOCATION", vec![s("linear distance"), s(&d.name), V::Ref(a), V::Ref(c)]),
                };
                (b.add_tail(e), vec![a, c])
            }
        };
        // Values in mm or degrees.
        let shown = |v: f64| if angle { v.to_degrees() } else { v * 1000.0 };
        if let Some((lower, upper)) = d.tolerance {
            let (u, ty, measure) = if angle { (degree(b), "PLANE_ANGLE_MEASURE_WITH_UNIT", "PLANE_ANGLE_MEASURE") } else { (mm(b), "MEASURE_WITH_UNIT", "LENGTH_MEASURE") };
            let l = b.add_tail(Ent::simple(ty, vec![V::Typed(measure.into(), vec![V::Real(shown(lower))]), V::Ref(u)]));
            let h = b.add_tail(Ent::simple(ty, vec![V::Typed(measure.into(), vec![V::Real(shown(upper))]), V::Ref(u)]));
            let tv = b.add_tail(Ent::simple("TOLERANCE_VALUE", vec![V::Ref(l), V::Ref(h)]));
            b.add_tail(Ent::simple("PLUS_MINUS_TOLERANCE", vec![V::Ref(tv), V::Ref(dim)]));
        }
        let u = if angle { degree(b) } else { mm(b) };
        let q = b.add_tail(Ent::simple("PRECISION_QUALIFIER", vec![V::Int(d.precision.to_string())]));
        let value = if angle {
            Ent { recs: vec![("MEASURE_REPRESENTATION_ITEM".into(), vec![]), ("MEASURE_WITH_UNIT".into(), vec![V::Typed("PLANE_ANGLE_MEASURE".into(), vec![V::Real(shown(d.value))]), V::Ref(u)]), ("PLANE_ANGLE_MEASURE_WITH_UNIT".into(), vec![]), ("QUALIFIED_REPRESENTATION_ITEM".into(), vec![list(&[q])]), ("REPRESENTATION_ITEM".into(), vec![s("nominal value")])], complex: true }
        } else {
            Ent { recs: vec![("LENGTH_MEASURE_WITH_UNIT".into(), vec![]), ("MEASURE_REPRESENTATION_ITEM".into(), vec![]), ("MEASURE_WITH_UNIT".into(), vec![V::Typed("POSITIVE_LENGTH_MEASURE".into(), vec![V::Real(shown(d.value))]), V::Ref(u)]), ("QUALIFIED_REPRESENTATION_ITEM".into(), vec![list(&[q])]), ("REPRESENTATION_ITEM".into(), vec![s("nominal value")])], complex: true }
        };
        let value = b.add_tail(value);
        let sdr = b.add_tail(Ent::simple("SHAPE_DIMENSION_REPRESENTATION", vec![s(""), list(&[value]), V::Ref(at.ctx)]));
        b.add_tail(Ent::simple("DIMENSIONAL_CHARACTERISTIC_REPRESENTATION", vec![V::Ref(dim), V::Ref(sdr)]));
        if let Some(p) = &d.presentation {
            let callout = present(b, at, p, &mut model_items, &mut model);
            for item in aspects.iter().chain(std::iter::once(&dim)) {
                b.add_tail(Ent::simple("DRAUGHTING_MODEL_ITEM_ASSOCIATION", vec![s(""), s(""), V::Ref(*item), V::Ref(model.unwrap_or(0)), V::Ref(callout)]));
            }
        }
    }
    if let Some(m) = model {
        b.set(m, Ent::simple("DRAUGHTING_MODEL", vec![s(""), list(&model_items), V::Ref(at.ctx)]));
    }
}

/// An annotation's callout and its plane (live 1746's layout); its items go
/// into the part's DRAUGHTING_MODEL (`model`, made at the first).
fn present(b: &mut Builder, at: &Pmi, p: &crate::io::PmiPresentation, model_items: &mut Vec<u32>, model: &mut Option<u32>) -> u32 {
    let null_style = |b: &mut Builder| b.add_tail(Ent::simple("PRESENTATION_STYLE_ASSIGNMENT", vec![V::List(vec![V::Typed("NULL_STYLE".into(), vec![V::Enum("NULL".into())])])]));
    let point = |p: [f64; 3], scale: f64| V::List(p.iter().map(|x| V::Real(x / scale)).collect());
    let style = null_style(b);
    let o = b.add_tail(Ent::simple("CARTESIAN_POINT", vec![s(""), point(p.origin, at.metres)]));
    let z = b.add_tail(Ent::simple("DIRECTION", vec![s(""), point(p.normal, 1.0)]));
    let x = b.add_tail(Ent::simple("DIRECTION", vec![s(""), point(p.x, 1.0)]));
    let ax = b.add_tail(Ent::simple("AXIS2_PLACEMENT_3D", vec![s(""), V::Ref(o), V::Ref(z), V::Ref(x)]));
    let plane = b.add_tail(Ent::simple("PLANE", vec![s(""), V::Ref(ax)]));
    let ap = b.add_tail(Ent::simple("", vec![]));
    let identity = |b: &mut Builder| {
        let o = b.add_tail(Ent::simple("CARTESIAN_POINT", vec![s(""), V::List(vec![V::Real(0.0); 3])]));
        let z = b.add_tail(Ent::simple("DIRECTION", vec![s(""), V::List(vec![V::Real(0.0), V::Real(0.0), V::Real(1.0)])]));
        let x = b.add_tail(Ent::simple("DIRECTION", vec![s(""), V::List(vec![V::Real(1.0), V::Real(0.0), V::Real(0.0)])]));
        b.add_tail(Ent::simple("AXIS2_PLACEMENT_3D", vec![s(""), V::Ref(o), V::Ref(z), V::Ref(x)]))
    };
    let (ax1, ax2) = (identity(b), identity(b));
    let items: Vec<V> = std::iter::once(V::Ref(ax2)).chain(at.solids.iter().map(|&i| V::Ref(i))).collect();
    let sr = b.add_tail(Ent::simple("SHAPE_REPRESENTATION", vec![s(""), V::List(items), V::Ref(at.ctx)]));
    let map = b.add_tail(Ent::simple("REPRESENTATION_MAP", vec![V::Ref(ax1), V::Ref(sr)]));
    let mapped = b.add_tail(Ent::simple("MAPPED_ITEM", vec![s(""), V::Ref(map), V::Ref(ax1)]));
    let null = null_style(b);
    let styled = b.add_tail(Ent::simple("STYLED_ITEM", vec![s(""), list(&[null]), V::Ref(mapped)]));
    model_items.extend([ap, ax1, mapped, styled]);
    if model.is_none() {
        *model = Some(b.add_tail(Ent::simple("", vec![])));
    }
    let font = b.add_tail(Ent::simple("DRAUGHTING_PRE_DEFINED_CURVE_FONT", vec![s("continuous")]));
    let black = b.add_tail(Ent::simple("COLOUR_RGB", vec![s(""), V::Real(0.0), V::Real(0.0), V::Real(0.0)]));
    let curve = b.add_tail(Ent::simple("CURVE_STYLE", vec![s(""), V::Ref(font), V::Typed("POSITIVE_LENGTH_MEASURE".into(), vec![V::Real(0.02)]), V::Ref(black)]));
    let line_style = b.add_tail(Ent::simple("PRESENTATION_STYLE_ASSIGNMENT", vec![list(&[curve])]));
    let coords = |b: &mut Builder, pts: &[[f64; 3]]| b.add_tail(Ent::simple("COORDINATES_LIST", vec![s("coordinates"), V::Int(pts.len().to_string()), V::List(pts.iter().map(|q| point(*q, at.metres)).collect())]));
    let mut occurrences = Vec::new();
    if p.triangles.len() >= 3 {
        let c = coords(b, &p.triangles);
        let n = p.triangles.len() / 3;
        let tris = V::List((0..n).map(|k| V::List((1..=3).map(|j| V::Int((3 * k + j).to_string())).collect())).collect());
        let set = b.add_tail(Ent::simple("COMPLEX_TRIANGULATED_SURFACE_SET", vec![s("surface set"), V::Ref(c), V::Int(p.triangles.len().to_string()), V::List(vec![]), V::List(vec![]), V::List(vec![]), tris]));
        occurrences.push(b.add_tail(Ent::simple("TESSELLATED_GEOMETRIC_SET", vec![s(&p.label), list(&[set])])));
    }
    if p.lines.len() >= 2 {
        let c = coords(b, &p.lines);
        let segs = V::List((0..p.lines.len() / 2).map(|k| V::List(vec![V::Int((2 * k + 1).to_string()), V::Int((2 * k + 2).to_string())])).collect());
        let set = b.add_tail(Ent::simple("TESSELLATED_CURVE_SET", vec![s("curve set"), V::Ref(c), segs]));
        occurrences.push(b.add_tail(Ent::simple("TESSELLATED_GEOMETRIC_SET", vec![s(&p.label), list(&[set])])));
    }
    let occurrences: Vec<u32> = occurrences.into_iter().map(|g| b.add_tail(Ent::simple("TESSELLATED_ANNOTATION_OCCURRENCE", vec![s(&p.label), list(&[line_style]), V::Ref(g)]))).collect();
    let callout = b.add_tail(Ent::simple("DRAUGHTING_CALLOUT", vec![s(&p.label), list(&occurrences)]));
    b.set(ap, Ent::simple("ANNOTATION_PLANE", vec![s(""), list(&[style]), V::Ref(plane), list(&[callout])]));
    callout
}

/// One placed part of an assembly: the index of its part and its placement
/// (rows of a 3 x 4 matrix, translation in metres).
pub(crate) struct AsmInstance {
    pub part: usize,
    pub transform: [[f64; 4]; 3],
}

/// An assembly in Onshape's layout (live qa4 r4 s5, an Assembly exported as
/// STEP): one application context and unit set; the root product, whose
/// shape representation lists the identity placement and one placement per
/// instance; each part once, as a component product in its own coordinates;
/// per instance a NEXT_ASSEMBLY_USAGE_OCCURRENCE ("NAUO1"…, named after its
/// part) with its transformation and context dependent shape representation;
/// the colours in one presentation on the assembly's context.
pub(crate) fn write_assembly(root: &str, parts: &[Part], instances: &[AsmInstance], file_name: &str, timestamp: &str, schema: Schema) -> Result<String> {
    let mut b = Builder::default();
    let (app_context, protocol, year) = schema.application();
    let first_part = parts.first().ok_or_else(|| bad("no parts"))?;
    let first = &first_part.occt.0;
    let ac = b.add(Ent::simple("APPLICATION_CONTEXT", vec![s(app_context)]));
    b.add(Ent::simple("APPLICATION_PROTOCOL_DEFINITION", vec![s("international standard"), s(protocol), V::Int(year.into()), V::Ref(ac)]));
    // Product data: the root, then each part in the order instances use them,
    // each NAUO's shape right after its part's.
    let product = |b: &mut Builder, name: &str| -> (u32, u32) {
        let pc = b.add(Ent::simple(if schema == Schema::Ap203 { "MECHANICAL_CONTEXT" } else { "PRODUCT_CONTEXT" }, vec![s(""), V::Ref(ac), s("mechanical")]));
        b.add(Ent::simple("PRODUCT_CATEGORY", vec![s(""), s("")]));
        let prod = b.add(Ent::simple("PRODUCT", vec![s(name), s(name), s(name), list(&[pc])]));
        b.add(Ent::simple("PRODUCT_RELATED_PRODUCT_CATEGORY", vec![s(""), s(""), list(&[prod])]));
        let pdf = b.add(Ent::simple("PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE", vec![s(""), s(""), V::Ref(prod), V::Enum("NOT_KNOWN".into())]));
        let pdc = b.add(Ent::simple(if schema == Schema::Ap203 { "DESIGN_CONTEXT" } else { "PRODUCT_DEFINITION_CONTEXT" }, vec![s(""), V::Ref(ac), s("design")]));
        let pd = b.add(Ent::simple("PRODUCT_DEFINITION", vec![s(""), s(""), V::Ref(pdf), V::Ref(pdc)]));
        let pds = b.add(Ent::simple("PRODUCT_DEFINITION_SHAPE", vec![s(""), s(""), V::Ref(pd)]));
        (pd, pds)
    };
    let (root_pd, root_pds) = product(&mut b, root);
    let mut order: Vec<usize> = Vec::new();
    for i in instances {
        if i.part >= parts.len() {
            return Err(bad("instance part"));
        }
        if !order.contains(&i.part) {
            order.push(i.part);
        }
    }
    let mut part_pd: HashMap<usize, (u32, u32)> = HashMap::new();
    let mut nauo_pds: Vec<u32> = Vec::new();
    for (k, i) in instances.iter().enumerate() {
        if let std::collections::hash_map::Entry::Vacant(e) = part_pd.entry(i.part) {
            e.insert(product(&mut b, &parts[i.part].name));
        }
        // Its NAUO is made below; the shape refers to it.
        nauo_pds.push(b.add(Ent::simple("PRODUCT_DEFINITION_SHAPE", vec![s(" "), s("NAUO PRDDFN"), V::Int(format!("__nauo{k}"))])));
    }

    // Units (OCCT's, from the first part), one uncertainty and context per product.
    // A part's representation (its id) and its solids.
    let rep_items = |src: &HashMap<u32, Ent>, sdr: u32| -> Result<(Vec<u32>, u32)> {
        let sdr = src.get(&sdr).ok_or_else(|| bad("no shape representation"))?;
        let Some(V::Ref(rep_id)) = sdr.args().get(1) else { return Err(bad("shape representation")) };
        let rep = src.get(rep_id).ok_or_else(|| bad("representation"))?;
        let Some(V::List(items)) = rep.args().get(1) else { return Err(bad("representation items")) };
        let items: Vec<u32> = items.iter().filter_map(|v| if let V::Ref(r) = v { Some(*r) } else { None }).filter(|r| src.get(r).is_some_and(|e| e.ty() != "AXIS2_PLACEMENT_3D")).collect();
        Ok((items, *rep_id))
    };
    let units: Vec<u32> = {
        let sdr = first.get(&first_part.sdr).ok_or_else(|| bad("no shape representation"))?;
        let Some(V::Ref(rep_id)) = sdr.args().get(1) else { return Err(bad("shape representation")) };
        let Some(V::Ref(ctx_id)) = first.get(rep_id).and_then(|r| r.args().get(2)) else { return Err(bad("context")) };
        match first.get(ctx_id).and_then(|c| c.rec("GLOBAL_UNIT_ASSIGNED_CONTEXT")).and_then(|a| a.first()) {
            Some(V::List(l)) => l.iter().filter_map(|v| if let V::Ref(r) = v { Some(*r) } else { None }).collect(),
            _ => return Err(bad("units")),
        }
    };
    let mut cp0 = Copier { src: first, map: HashMap::new() };
    let unit_ids: Vec<u32> = units.iter().map(|&u| cp0.visit(&mut b, u, &mut |_, _| {})).collect::<Result<_>>()?;
    let len_unit = *unit_ids.first().ok_or_else(|| bad("length unit"))?;
    let metres = {
        let e = &b.ents[len_unit as usize];
        match (e.rec("SI_UNIT"), e.rec("CONVERSION_BASED_UNIT")) {
            (Some([V::Enum(p), _]), _) => match p.as_str() {
                "MILLI" => 1e-3,
                "CENTI" => 1e-2,
                "KILO" => 1e3,
                _ => 1.0,
            },
            (_, Some([V::Str(n), ..])) => match n.as_str() {
                "INCH" => 0.0254,
                "FOOT" => 0.3048,
                "YARD" => 0.9144,
                _ => 1.0,
            },
            _ => 1.0,
        }
    };
    // Each part's uncertainty (its own representation context's); the root's, the greatest.
    let part_unc: HashMap<usize, f64> = order
        .iter()
        .map(|&p| {
            let src = &parts[p].occt.0;
            let ctx = src.get(&parts[p].sdr).and_then(|sdr| match sdr.args().get(1) {
                Some(V::Ref(rep)) => src.get(rep),
                _ => None,
            });
            let ctx = ctx.and_then(|r| match r.args().get(2) {
                Some(V::Ref(c)) => Some(*c),
                _ => None,
            });
            (p, ctx.map_or(1e-8 / metres, |c| uncertainty(src, c, metres)))
        })
        .collect();
    let root_unc = part_unc.values().copied().fold(1e-8 / metres, f64::max);
    let context = |b: &mut Builder, name: &str, kind: &str, unc: f64| -> u32 {
        let unc = b.add(Ent::simple("UNCERTAINTY_MEASURE_WITH_UNIT", vec![V::Typed("LENGTH_MEASURE".into(), vec![V::Real(unc)]), V::Ref(len_unit), s("DISTANCE_ACCURACY_VALUE"), s("Maximum Tolerance applied to model")]));
        b.add(Ent {
            recs: vec![
                ("GEOMETRIC_REPRESENTATION_CONTEXT".into(), vec![V::Int("3".into())]),
                ("GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT".into(), vec![list(&[unc])]),
                ("GLOBAL_UNIT_ASSIGNED_CONTEXT".into(), vec![list(&unit_ids)]),
                ("REPRESENTATION_CONTEXT".into(), vec![s(name), s(kind)]),
            ],
            complex: true,
        })
    };
    let root_ctx = context(&mut b, root, "TOP_LEVEL_ASSEMBLY_PART", root_unc);
    let part_ctx: HashMap<usize, u32> = order.iter().map(|&p| (p, context(&mut b, &parts[p].name, "COMPONENT_PART", part_unc[&p]))).collect();
    let mdgpr = b.reserve();

    // The identity placement every representation shares, the root's
    // representation (its placements filled in below) and the parts'.
    let p0 = b.add(Ent::simple("CARTESIAN_POINT", vec![s(""), V::List(vec![V::Real(0.0), V::Real(0.0), V::Real(0.0)])]));
    let dz = b.add(Ent::simple("DIRECTION", vec![s(""), V::List(vec![V::Real(0.0), V::Real(0.0), V::Real(1.0)])]));
    let dx = b.add(Ent::simple("DIRECTION", vec![s(""), V::List(vec![V::Real(1.0), V::Real(0.0), V::Real(0.0)])]));
    let ax = b.add(Ent::simple("AXIS2_PLACEMENT_3D", vec![s(""), V::Ref(p0), V::Ref(dz), V::Ref(dx)]));
    let root_sr = b.reserve();
    b.add(Ent::simple("SHAPE_DEFINITION_REPRESENTATION", vec![V::Ref(root_pds), V::Ref(root_sr)]));
    let mut part_sr: HashMap<usize, u32> = HashMap::new();
    for &p in &order {
        let sr = b.add(Ent::simple("SHAPE_REPRESENTATION", vec![s(&parts[p].name), list(&[ax]), V::Ref(part_ctx[&p])]));
        b.add(Ent::simple("SHAPE_DEFINITION_REPRESENTATION", vec![V::Ref(part_pd[&p].1), V::Ref(sr)]));
        part_sr.insert(p, sr);
    }

    // Each part's solids with their colour, then each of its instances' placement.
    let mut styled = Vec::new();
    let mut placements: Vec<u32> = vec![0; instances.len()];
    for &p in &order {
        let src = &parts[p].occt.0;
        let (items, rep_id) = rep_items(src, parts[p].sdr)?;
        let rep = src.get(&rep_id).ok_or_else(|| bad("representation"))?.clone();
        let mut cp = if p == 0 { Copier { src, map: cp0.map.clone() } } else { Copier { src, map: HashMap::new() } };
        let rgb: Option<Vec<V>> = parts[p].color.map(|c| std::iter::once(s("")).chain(c.iter().map(|&x| V::Real((f64::from(x.clamp(0.0, 1.0)) * 255.0).round() / 255.0))).collect());
        let mut solids = Vec::new();
        for &it in &items {
            let i = cp.visit(&mut b, it, &mut |b, item| {
                let Some(rgb) = &rgb else { return };
                let col = b.add(Ent::simple("COLOUR_RGB", rgb.clone()));
                let fasc = b.add(Ent::simple("FILL_AREA_STYLE_COLOUR", vec![s(""), V::Ref(col)]));
                let fas = b.add(Ent::simple("FILL_AREA_STYLE", vec![s(""), list(&[fasc])]));
                let ssfa = b.add(Ent::simple("SURFACE_STYLE_FILL_AREA", vec![V::Ref(fas)]));
                let sss = b.add(Ent::simple("SURFACE_SIDE_STYLE", vec![s(""), list(&[ssfa])]));
                let ssu = b.add(Ent::simple("SURFACE_STYLE_USAGE", vec![V::Enum("BOTH".into()), V::Ref(sss)]));
                let psa = b.add(Ent::simple("PRESENTATION_STYLE_ASSIGNMENT", vec![list(&[ssu])]));
                styled.push(b.add(Ent::simple("STYLED_ITEM", vec![s(""), list(&[psa]), V::Ref(item)])));
            })?;
            if let Some(V::Str(n)) = b.ents[i as usize].recs[0].1.first_mut() {
                *n = parts[p].name.replace('\'', "''");
            }
            solids.push(i);
        }
        for (k, inst) in instances.iter().enumerate().filter(|(_, i)| i.part == p) {
            let t = &inst.transform;
            let at = b.add(Ent::simple("CARTESIAN_POINT", vec![s(""), V::List((0..3).map(|r| V::Real(t[r][3] / metres)).collect())]));
            let z = b.add(Ent::simple("DIRECTION", vec![s(""), V::List((0..3).map(|r| V::Real(t[r][2])).collect())]));
            let x = b.add(Ent::simple("DIRECTION", vec![s(""), V::List((0..3).map(|r| V::Real(t[r][0])).collect())]));
            placements[k] = b.add(Ent::simple("AXIS2_PLACEMENT_3D", vec![s(""), V::Ref(at), V::Ref(z), V::Ref(x)]));
        }
        let mut rep_o = rep;
        rep_o.recs[0].1 = vec![s(""), list(&solids), V::Ref(part_ctx[&p])];
        let absr = b.add(rep_o);
        b.add(Ent::simple("SHAPE_REPRESENTATION_RELATIONSHIP", vec![s(""), s(""), V::Ref(part_sr[&p]), V::Ref(absr)]));
    }
    let mut root_items = vec![ax];
    root_items.extend(&placements);
    b.set(root_sr, Ent::simple("SHAPE_REPRESENTATION", vec![s(root), list(&root_items), V::Ref(root_ctx)]));

    // The instances: transformation, relationship, usage, its shape.
    for (k, inst) in instances.iter().enumerate() {
        let idt = b.add(Ent::simple("ITEM_DEFINED_TRANSFORMATION", vec![s(" "), s(" "), V::Ref(ax), V::Ref(placements[k])]));
        let rr = b.add(Ent {
            recs: vec![
                ("REPRESENTATION_RELATIONSHIP".into(), vec![s(" "), s(" "), V::Ref(part_sr[&inst.part]), V::Ref(root_sr)]),
                ("REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION".into(), vec![V::Ref(idt)]),
                ("SHAPE_REPRESENTATION_RELATIONSHIP".into(), vec![]),
            ],
            complex: true,
        });
        let name = &parts[inst.part].name;
        let nauo = b.add(Ent::simple("NEXT_ASSEMBLY_USAGE_OCCURRENCE", vec![s(&format!("NAUO{}", k + 1)), s(name), s(name), V::Ref(root_pd), V::Ref(part_pd[&inst.part].0), s("")]));
        b.ents[nauo_pds[k] as usize].recs[0].1[2] = V::Ref(nauo);
        b.add(Ent::simple("CONTEXT_DEPENDENT_SHAPE_REPRESENTATION", vec![V::Ref(rr), V::Ref(nauo_pds[k])]));
    }
    b.set(mdgpr, Ent::simple("MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION", vec![s(""), list(&styled), V::Ref(root_ctx)]));
    // AP214 / AP203: Onshape's PMI validation properties, on the root.
    if schema != Schema::Ap242 {
        for what in ["number of annotations", "number of views"] {
            let vri = b.add(Ent::simple("VALUE_REPRESENTATION_ITEM", vec![s(what), V::Typed("COUNT_MEASURE".into(), vec![V::Real(0.0)])]));
            let def = b.add(Ent::simple("PROPERTY_DEFINITION", vec![s("pmi validation property"), s(""), V::Ref(root_pds)]));
            let rep = b.add(Ent::simple("REPRESENTATION", vec![s(""), list(&[vri]), V::Ref(root_ctx)]));
            b.add(Ent::simple("PROPERTY_DEFINITION_REPRESENTATION", vec![V::Ref(def), V::Ref(rep)]));
        }
    }
    Ok(numbered(&b, file_name, timestamp, schema))
}

/// Onshape's numbering (ST-Developer's renumber hook): entities grouped by
/// type, the type created last first, each group in creation order, from
/// #10; then the file text.
fn numbered(b: &Builder, file_name: &str, timestamp: &str, schema: Schema) -> String {
    let keys: Vec<String> = b.ents.iter().map(Ent::key).collect();
    let mut first: HashMap<&str, usize> = HashMap::new();
    for (i, k) in keys.iter().enumerate() {
        first.entry(k.as_str()).or_insert(i);
    }
    let mut idx: Vec<usize> = (0..b.ents.len()).filter(|&i| !b.tail[i]).collect();
    idx.sort_by_key(|&i| (std::cmp::Reverse(first[keys[i].as_str()]), i));
    idx.extend((0..b.ents.len()).filter(|&i| b.tail[i]));
    let mut num = vec![0u32; b.ents.len()];
    for (n, &i) in idx.iter().enumerate() {
        num[i] = 10 + n as u32;
    }
    let mut out = Out { text: String::new(), col: 0 };
    out.text.push_str(&header(file_name, timestamp, schema));
    for &i in &idx {
        let mut e = b.ents[i].clone();
        for rec in &mut e.recs {
            map_refs(&mut rec.1, &|r| num[r as usize]);
        }
        out.entity(num[i] as usize, &e);
    }
    out.text.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    out.text
}

/// A header line ending in a string, split like [`Out::string`].
fn header_str(prefix: &str, value: &str, suffix: &str) -> String {
    let mut o = Out { text: String::new(), col: 0 };
    o.raw(prefix);
    o.string(&format!("'{}'", value.replace('\'', "''")));
    o.raw(suffix);
    o.text
}

fn header(file_name: &str, timestamp: &str, schema: Schema) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(\n/* description */ {},\n/* implementation_level */ '2;1');\n\nFILE_NAME(\n{}\n/* time_stamp */ '{timestamp}',\n/* author */ (''),\n/* organization */ (''),\n/* preprocessor_version */ 'NOSHAPE',\n/* originating_system */ 'NOSHAPE {}',\n/* authorisation */ '  ');\n\nFILE_SCHEMA (('{}'));\nENDSEC;\n\nDATA;\n",
        schema.description(),
        header_str("/* name */ ", file_name, ","),
        crate::VERSION,
        schema.file_schema(),
    )
}

/// UTC "YYYY-MM-DDTHH:MM:SSZ" of a Unix time.
pub(crate) fn timestamp(unix: u64) -> String {
    let days = (unix / 86400) as i64;
    let secs = unix % 86400;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", secs / 3600, secs / 60 % 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reals_like_onshape() {
        assert_eq!(real(0.0), "0.");
        assert_eq!(real(-0.0), "0.");
        assert_eq!(real(1.0), "1.");
        assert_eq!(real(-1.0), "-1.");
        assert_eq!(real(0.01), "0.01");
        assert_eq!(real(1e-8), "1.E-8");
        assert_eq!(real(2e-5), "2.E-5");
        assert_eq!(real(59.0 / 255.0), "0.231372549019608");
        assert_eq!(real(9.0 * std::f64::consts::FRAC_PI_4), "7.06858347057703");
        assert_eq!(real(-1.30104260698261e-18), "-1.30104260698261E-18");
        assert_eq!(real(0.0001), "0.0001");
        assert_eq!(real(2020.5), "2020.5");
    }

    #[test]
    fn wrapping_like_onshape() {
        let mut o = Out { text: String::new(), col: 0 };
        let e = Ent::simple("MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION", vec![s(""), V::List(vec![V::Ref(295)]), V::Ref(511)]);
        o.entity(508, &e);
        assert_eq!(o.text, "#508=MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',(#295),\n#511);\n");
        let mut o = Out { text: String::new(), col: 0 };
        let e = Ent::simple("PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE", vec![s(""), s(""), V::Ref(541), V::Enum("NOT_KNOWN".into())]);
        o.entity(535, &e);
        assert_eq!(o.text, "#535=PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE('','',#541,\n .NOT_KNOWN.);\n");
        let mut o = Out { text: String::new(), col: 0 };
        let e = Ent::simple("UNCERTAINTY_MEASURE_WITH_UNIT", vec![V::Typed("LENGTH_MEASURE".into(), vec![V::Real(1e-8)]), V::Ref(523), s("DISTANCE_ACCURACY_VALUE"), s("Maximum Tolerance applied to model")]);
        o.entity(514, &e);
        assert_eq!(o.text, "#514=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-8),#523,\n'DISTANCE_ACCURACY_VALUE','Maximum Tolerance applied to model');\n");
    }

    #[test]
    fn utc_timestamp() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(timestamp(1_792_586_367), "2026-10-21T12:39:27Z");
    }
}
