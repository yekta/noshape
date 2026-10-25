//! The operation registry: every operation the agent API offers, with its
//! JSON input schema and handler. The CLI's subcommands and the MCP server's
//! tools are generated from this one list, so they can't drift.

use crate::Ctx;
use crate::error::OpError;
use serde_json::{Value, json};

/// One input field of an operation.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: &'static str,
    /// JSON schema of the value.
    pub schema: Value,
    pub required: bool,
    pub description: &'static str,
}

/// A rendered image returned by an operation (MCP image content).
pub struct Image {
    pub png: Vec<u8>,
}

/// What an operation returns: a JSON value, plus images.
pub struct OpOutput {
    pub value: Value,
    pub images: Vec<Image>,
}

impl From<Value> for OpOutput {
    fn from(value: Value) -> Self {
        OpOutput { value, images: Vec::new() }
    }
}

pub type Handler = fn(&mut Ctx, &Args) -> Result<OpOutput, OpError>;

/// An operation. `group` + `verb` give the CLI command (`noshape-cli
/// feature add`) and the MCP tool name (`feature_add`).
pub struct Op {
    pub group: &'static str,
    pub verb: &'static str,
    /// One line, imperative ("Add a feature to a Part Studio").
    pub summary: &'static str,
    /// What it does, units, expression syntax, what ids it takes and returns.
    pub description: &'static str,
    pub fields: Vec<Field>,
    /// Changes the document (one microversion, undoable like the UI's edit).
    pub mutates: bool,
    /// Deletes or overwrites data.
    pub destructive: bool,
    pub run: Handler,
}

impl Op {
    pub fn tool_name(&self) -> String {
        format!("{}_{}", self.group, self.verb.replace('-', "_"))
    }

    pub fn full_description(&self) -> String {
        let mut s = if self.description.is_empty() { self.summary.to_string() } else { format!("{}\n\n{}", self.summary, self.description) };
        let examples = crate::examples::of(&self.tool_name());
        if !examples.is_empty() {
            s.push_str("\n\nExamples (arguments; ids like \"edge:…\" come from the query and sketch tools):");
            for e in examples {
                s.push_str(&format!("\n  {e}"));
            }
        }
        s
    }

    pub fn input_schema(&self) -> Value {
        let mut props = serde_json::Map::new();
        for f in &self.fields {
            let mut s = f.schema.clone();
            if !f.description.is_empty()
                && let Some(o) = s.as_object_mut()
            {
                o.insert("description".into(), Value::String(f.description.into()));
            }
            props.insert(f.name.into(), s);
        }
        let required: Vec<&str> = self.fields.iter().filter(|f| f.required).map(|f| f.name).collect();
        json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
    }
}

/// Builder helpers for fields.
pub fn req(name: &'static str, schema: Value, description: &'static str) -> Field {
    Field { name, schema, required: true, description }
}
pub fn opt(name: &'static str, schema: Value, description: &'static str) -> Field {
    Field { name, schema, required: false, description }
}

/// The arguments of one call, with typed accessors that report which field
/// is wrong.
pub struct Args<'a> {
    pub op: &'a Op,
    pub value: &'a Value,
}

impl Args<'_> {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.value.get(name).filter(|v| !v.is_null())
    }
    pub fn str(&self, name: &str) -> Result<&str, OpError> {
        self.get(name).and_then(Value::as_str).ok_or_else(|| OpError::param(name, "required string"))
    }
    pub fn opt_str(&self, name: &str) -> Result<Option<&str>, OpError> {
        match self.get(name) {
            None => Ok(None),
            Some(v) => v.as_str().map(Some).ok_or_else(|| OpError::param(name, "must be a string")),
        }
    }
    pub fn f64(&self, name: &str) -> Result<f64, OpError> {
        self.get(name).and_then(Value::as_f64).ok_or_else(|| OpError::param(name, "required number"))
    }
    pub fn opt_f64(&self, name: &str) -> Result<Option<f64>, OpError> {
        match self.get(name) {
            None => Ok(None),
            Some(v) => v.as_f64().map(Some).ok_or_else(|| OpError::param(name, "must be a number")),
        }
    }
    pub fn opt_u64(&self, name: &str) -> Result<Option<u64>, OpError> {
        match self.get(name) {
            None => Ok(None),
            Some(v) => v.as_u64().map(Some).ok_or_else(|| OpError::param(name, "must be a non-negative integer")),
        }
    }
    pub fn bool_or(&self, name: &str, default: bool) -> Result<bool, OpError> {
        match self.get(name) {
            None => Ok(default),
            Some(v) => v.as_bool().ok_or_else(|| OpError::param(name, "must be true or false")),
        }
    }
    pub fn array(&self, name: &str) -> Result<&Vec<Value>, OpError> {
        self.get(name).and_then(Value::as_array).ok_or_else(|| OpError::param(name, "required array"))
    }
    pub fn opt_array(&self, name: &str) -> Result<Option<&Vec<Value>>, OpError> {
        match self.get(name) {
            None => Ok(None),
            Some(v) => v.as_array().map(Some).ok_or_else(|| OpError::param(name, "must be an array")),
        }
    }
    pub fn object(&self, name: &str) -> Result<Option<&serde_json::Map<String, Value>>, OpError> {
        match self.get(name) {
            None => Ok(None),
            Some(v) => v.as_object().map(Some).ok_or_else(|| OpError::param(name, "must be an object")),
        }
    }
}

static REGISTRY: std::sync::OnceLock<Vec<Op>> = std::sync::OnceLock::new();

/// Every operation, in a stable order (grouped).
/// The groups, in order of their first operation, each once.
pub fn groups() -> Vec<&'static str> {
    let mut g: Vec<&'static str> = Vec::new();
    for op in all() {
        if !g.contains(&op.group) {
            g.push(op.group);
        }
    }
    g
}

pub fn all() -> &'static [Op] {
    REGISTRY.get_or_init(crate::ops::all)
}

pub fn by_tool_name(name: &str) -> Option<&'static Op> {
    all().iter().find(|o| o.tool_name() == name)
}

pub fn by_command(group: &str, verb: &str) -> Option<&'static Op> {
    all().iter().find(|o| o.group == group && o.verb == verb)
}

/// Validate the arguments against the schema's field list and run.
pub fn run(ctx: &mut Ctx, op: &'static Op, args: &Value) -> Result<OpOutput, OpError> {
    let empty = json!({});
    let args = if args.is_null() { &empty } else { args };
    let Some(obj) = args.as_object() else { return Err(OpError::invalid("arguments must be a JSON object")) };
    for k in obj.keys() {
        if !op.fields.iter().any(|f| f.name == k) {
            let known: Vec<&str> = op.fields.iter().map(|f| f.name).collect();
            return Err(OpError::param(k, &format!("unknown argument; `{}` takes: {}", op.tool_name(), known.join(", "))));
        }
    }
    for f in op.fields.iter().filter(|f| f.required) {
        if obj.get(f.name).is_none_or(Value::is_null) {
            return Err(OpError::param(f.name, &format!("required: {}", f.description)));
        }
    }
    // Live mode: the document is open in the app; the call runs there.
    if let Some(r) = crate::live::forward(ctx, op, args) {
        return r;
    }
    (op.run)(ctx, &Args { op, value: args }).map(|mut out| {
        tidy_numbers(&mut out.value);
        out
    })
}

/// Reply numbers without floating-point noise: 12 significant digits,
/// magnitudes below 1e-12 as 0, no -0 (3.3000000000000007 -> 3.3,
/// 1.4e-14 -> 0). No value an operation reports is that small.
pub fn tidy_numbers(v: &mut Value) {
    match v {
        Value::Number(n) if n.is_f64() => {
            let x = n.as_f64().unwrap_or(0.0);
            let y = if x.abs() < 1e-12 { 0.0 } else { format!("{x:.11e}").parse::<f64>().unwrap_or(x) + 0.0 };
            if let Some(m) = serde_json::Number::from_f64(y) {
                *n = m;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(tidy_numbers),
        Value::Object(m) => m.values_mut().for_each(tidy_numbers),
        _ => {}
    }
}
