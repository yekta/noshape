//! Errors of the agent API: explicit about which parameter is wrong and why,
//! and about regeneration failures (which feature, Onshape's message).

use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct OpError {
    /// `invalid_argument`, `not_found`, `regeneration_failed`, `read_only`,
    /// `protected` (workspace protection), `io`, `internal`.
    pub code: &'static str,
    pub message: String,
    /// The argument (path into the arguments, e.g. `params.depth`) at fault.
    pub param: Option<String>,
    /// Extra structured detail (failed features...).
    pub detail: Option<Box<Value>>,
}

impl OpError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        OpError { code, message: message.into(), param: None, detail: None }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_argument", message)
    }
    pub fn param(param: &str, message: &str) -> Self {
        OpError { code: "invalid_argument", message: message.to_string(), param: Some(param.to_string()), detail: None }
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }
    pub fn with_param(mut self, param: impl Into<String>) -> Self {
        self.param = Some(param.into());
        self
    }
    /// Prefix the parameter path (`depth` -> `params.depth`).
    pub fn under(mut self, prefix: &str) -> Self {
        self.param = Some(match self.param.take() {
            Some(p) if p.starts_with('[') => format!("{prefix}{p}"),
            Some(p) => format!("{prefix}.{p}"),
            None => prefix.to_string(),
        });
        self
    }
    pub fn with_detail(mut self, detail: Value) -> Self {
        self.detail = Some(Box::new(detail));
        self
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({ "error": self.code, "message": self.message });
        if let Some(p) = &self.param {
            v["param"] = Value::String(p.clone());
        }
        if let Some(d) = &self.detail {
            v["detail"] = (**d).clone();
        }
        v
    }
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.param {
            Some(p) => write!(f, "{} ({p}): {}", self.code, self.message),
            None => write!(f, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for OpError {}

impl From<anyhow::Error> for OpError {
    fn from(e: anyhow::Error) -> Self {
        let m = format!("{e:#}");
        // The session's refusals: a protected workspace, a read-only view.
        let code = if m.contains(crate::session::PROTECTED_MESSAGE) {
            "protected"
        } else if m.contains(crate::session::READ_ONLY_MESSAGE) {
            "read_only"
        } else {
            "io"
        };
        OpError::new(code, m)
    }
}

/// " (did you mean `x`?)" for the option closest to `given` (at most two
/// edits away, or one that starts with it), else "".
pub fn did_you_mean<'a>(given: &str, options: impl IntoIterator<Item = &'a str>) -> String {
    fn edits(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let cur = row[j + 1];
                row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
                prev = cur;
            }
        }
        row[b.len()]
    }
    let g = given.to_lowercase();
    options
        .into_iter()
        .map(|o| (edits(&g, &o.to_lowercase()), o))
        .filter(|(d, o)| *d <= 2 || (g.len() >= 3 && o.to_lowercase().starts_with(&g)))
        .min_by_key(|(d, _)| *d)
        .map(|(_, o)| format!(" (did you mean `{o}`?)"))
        .unwrap_or_default()
}
