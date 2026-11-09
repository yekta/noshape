//! A Part Studio's tab menu › Show code: its FeatureScript representation, view only (live dom/fs-showcode.txt):
//! the version and geometry.fs import, the "Default Units" annotation, main / build / build(configuration) /
//! buildPrivate = definePartStudio(…), then one `features.<id> = function(id) { … }` per feature, each tried in turn.
//! Queries are written as `qUnion([])` (live packs them with qCompressed).

use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_units::{Dim, UnitPrefs};

/// The code of `ps` in a document of units `units`.
pub fn part_studio_code(ps: &PartStudio, units: &UnitPrefs) -> String {
    let v = noshape_featurescript::STD_VERSION;
    let length = units.length.name().to_lowercase();
    let angle = units.angle.name().to_lowercase();
    let cap = |s: &str| format!("{}{}", s[..1].to_uppercase(), &s[1..]);
    let mut defaults = vec![format!("cubic{}", cap(&length)), angle, "degreePerSecond".into(), "footPoundForce".into(), "hertz".into(), "inchPerSecondSquared".into(), "inchPound".into(), length.clone(), "pound".into(), "poundForce".into(), "poundPerCubicInch".into(), "poundPerSquareInch".into(), "second".into(), format!("square{}", cap(&length))];
    defaults.sort();
    defaults.dedup();
    let defaults: Vec<String> = defaults.iter().map(|d| format!("\"{d}\"")).collect();
    let mut out = format!(
        "FeatureScript {v};\nimport(path : \"onshape/std/geometry.fs\", version : \"{v}.0\");\n\nannotation {{ \"Default Units\" : [{}] }}\n\
export function main()\n{{\n    return build({{}});\n}}\n\nexport function build()\n{{\n    return buildPrivate({{}});\n}}\n\n\
export function build(configuration is map)\n{{\n    return buildPrivate(configuration);\n}}\n\n\
const buildPrivate = definePartStudio(function(context is Context, configuration is map, lookup is function)\n    precondition\n    {{\n    }}\n    {{\n\
        const id is Id = newId();\n        annotation {{ 'unused' : true }}\n        var features = {{}};\n",
        defaults.join(", ")
    );
    for f in &ps.features {
        let fid = format!("F{}", f.id.0);
        out.push_str(&format!("        features.{fid} = function(id)\n            {{\n                annotation {{ 'unused' : true }}\n                var features = features;\n                if ({})\n                {{\n                    {{\n                    }}\n", !f.suppressed));
        out.push_str(&format!("                    annotation {{ \"Feature Name\" : {} }}\n", quoted(&f.name)));
        if f.type_id == "sketch" {
            out.push_str(&format!("                    var sketch = newSketch(context, id + \"{fid}\", {{ \"sketchPlane\" : qUnion([]) }});\n                    skSolve(sketch);\n"));
        } else {
            out.push_str(&format!("                    {}(context, id + \"{fid}\", {});\n", f.type_id, map(&f.params)));
        }
        out.push_str(&format!("                }}\n            }};\n        try(features.{fid}(id));\n"));
    }
    out.push_str(&format!("        return context;\n    }}, {length}, {{}});\n"));
    out
}

fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// A feature's parameters as a FeatureScript map.
fn map(params: &ParamValues) -> String {
    let fields: Vec<String> = params.0.iter().filter_map(|(k, v)| value(v).map(|v| format!("{} : {v}", quoted(k)))).collect();
    if fields.is_empty() { "{}".into() } else { format!("{{ {} }}", fields.join(", ")) }
}

fn value(v: &ParamValue) -> Option<String> {
    Some(match v {
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Enum(e) => quoted(e),
        ParamValue::Text(t) => quoted(t),
        ParamValue::Query(_) => "qUnion([])".into(),
        ParamValue::Lookup(path) => quoted(&path.join("/")),
        ParamValue::Array(items) => format!("[{}]", items.iter().map(map).collect::<Vec<_>>().join(", ")),
        ParamValue::Sketch(_) => return None,
        // Live: { 'value' : try(3 * millimeter), 'expression' : "3 mm" }.value, the value in SI here.
        ParamValue::Expr(e) => {
            let si = match e.cached {
                Some(q) if q.dim == Dim::LENGTH => format!("{} * meter", q.value),
                Some(q) if q.dim == Dim::ANGLE => format!("{} * radian", q.value),
                Some(q) => q.value.to_string(),
                None => return Some(quoted(&e.text)),
            };
            format!("{{ 'value' : try({si}), 'expression' : {} }}.value", quoted(&e.text))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live's frame around the features (dom/fs-showcode.txt), one block per feature, in a millimeter document.
    #[test]
    fn a_part_studio_reads_as_live_frames_it() {
        let mut ps = PartStudio::default();
        let mut params = ParamValues::default();
        params.set("oppositeDirection", ParamValue::Bool(false));
        ps.features.push(noshape_model::partstudio::Feature { id: noshape_model::reference::FeatureId(7), type_id: "fillet".into(), name: "Fillet 1".into(), suppressed: false, params, folder: None, custom_name: false, hidden: false, suppression_expr: None, sub_features: Vec::new() });
        let code = part_studio_code(&ps, &UnitPrefs::default());
        let v = noshape_featurescript::STD_VERSION;
        assert!(code.starts_with(&format!("FeatureScript {v};\nimport(path : \"onshape/std/geometry.fs\", version : \"{v}.0\");\n\nannotation {{ \"Default Units\" : [")), "{code}");
        assert!(code.contains("const buildPrivate = definePartStudio(function(context is Context, configuration is map, lookup is function)\n"), "{code}");
        assert!(code.contains("        features.F7 = function(id)\n            {\n                annotation { 'unused' : true }\n                var features = features;\n                if (true)\n"), "{code}");
        assert!(code.contains("                    annotation { \"Feature Name\" : \"Fillet 1\" }\n                    fillet(context, id + \"F7\", { \"oppositeDirection\" : false });\n"), "{code}");
        assert!(code.ends_with("        try(features.F7(id));\n        return context;\n    }, millimeter, {});\n"), "{code}");
    }
}
