//! The user's twisted hexagon vase (11-02, P1) through the agent API, as their agent built it: five sketches on Top and
//! planes offset 30 / 60 / 90 / 120 mm, each a regular hexagon (circumradius 35 / 45 / 40 / 28 / 35 mm, turned 15° more
//! each time), sharp or with R5 corners drawn as lines and tangent arcs; a Loft through them, then Shell with the top
//! face removed. Live 1769 / 1782: the smooth loft one side face (3 faces, 24 edges), its Shells 5 faces (2 mm in
//! 58695.0522, out 63461.6237 mm³); the sharp loft 8 faces, its 1.6 mm Shell 15 faces, 47868.3858 mm³.

use noshape_ops::registry;
use noshape_ops::Ctx;
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).map(|o| o.value).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json()))
}

/// The vase's loft in a new document; `rounded`: R5 corners.
fn loft(ctx: &mut Ctx, name: &str, rounded: bool) {
    let dir = std::env::temp_dir().join(format!("noshape-ops-vase-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.noshape"));
    let _ = std::fs::remove_file(&p);
    call(ctx, "document_create", json!({ "path": p.to_str().unwrap(), "length_unit": "mm" }));
    let mut regions = Vec::new();
    for (k, (z, r)) in [(0.0, 35.0), (30.0, 45.0), (60.0, 40.0), (90.0, 28.0), (120.0, 35.0)].into_iter().enumerate() {
        let plane = if z == 0.0 {
            "plane:top".to_string()
        } else {
            let f = call(ctx, "feature_add", json!({ "type": "cPlane", "params": { "entities": ["plane:top"], "offset": format!("{z} mm") } }));
            format!("fplane:{}", f["feature"].as_str().unwrap())
        };
        let s = call(ctx, "sketch_create", json!({ "plane": plane }))["sketch"].as_str().unwrap().to_string();
        let corner = |i: usize| {
            let a = (15.0 * k as f64 + 60.0 * i as f64).to_radians();
            [r * a.cos(), r * a.sin()]
        };
        let shapes: Vec<Value> = if rounded {
            // Each side cut back 5 / tan 60° at both ends, the corners tangent arcs (a sketch fillet's result).
            let t = 5.0 / 60f64.to_radians().tan();
            let ends: Vec<([f64; 2], [f64; 2])> = (0..6)
                .map(|i| {
                    let (a, b) = (corner(i), corner((i + 1) % 6));
                    let l = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
                    let u = [(b[0] - a[0]) / l, (b[1] - a[1]) / l];
                    ([a[0] + u[0] * t, a[1] + u[1] * t], [b[0] - u[0] * t, b[1] - u[1] * t])
                })
                .collect();
            (0..6).flat_map(|i| [json!({ "tool": "line", "points": [ends[i].0, ends[i].1] }), json!({ "tool": "tangent_arc", "points": [ends[i].1, ends[(i + 1) % 6].0] })]).collect()
        } else {
            vec![json!({ "tool": "line", "points": (0..=6).map(|i| corner(i % 6)).collect::<Vec<_>>() })]
        };
        call(ctx, "sketch_draw", json!({ "sketch": s, "shapes": shapes }));
        let regs = call(ctx, "sketch_get", json!({ "sketch": s }))["regions"].as_array().unwrap().clone();
        assert_eq!(regs.len(), 1, "section {k}: {regs:?}");
        regions.push(regs[0]["id"].clone());
    }
    let profiles: Vec<Value> = regions.iter().map(|r| json!({ "sheetProfileEntities": [r] })).collect();
    let f = call(ctx, "feature_add", json!({ "type": "loft", "params": { "sheetProfilesArray": profiles } }));
    assert_eq!(f["feature_status"]["status"], "Ok", "{f}");
}

/// Shell with the top face removed: (status, volume, faces), the shell undone after.
fn shell(ctx: &mut Ctx, t: f64, outward: bool) -> (String, f64, u64) {
    let faces = call(ctx, "query_entities", json!({ "kind": "faces" }))["faces"].as_array().unwrap().clone();
    let top = faces.iter().find(|f| (f["centroid"][2].as_f64().unwrap() - 120.0).abs() < 1e-6 && f["surface"] == "plane").expect("the top face")["id"].clone();
    let f = call(ctx, "feature_add", json!({ "type": "shell", "params": { "entities": [top], "thickness": format!("{t} mm"), "oppositeDirection": outward } }));
    let p = call(ctx, "query_parts", json!({}))["parts"][0].clone();
    call(ctx, "history_undo", json!({}));
    (f["feature_status"]["status"].as_str().unwrap().to_string(), p["volume"].as_f64().unwrap(), p["faces"].as_u64().unwrap())
}

#[test]
fn the_users_twisted_vases_loft_and_shell_as_live() {
    let mut ctx = Ctx::default();
    loft(&mut ctx, "smooth", true);
    let p = call(&mut ctx, "query_parts", json!({}))["parts"][0].clone();
    assert_eq!((p["faces"].as_u64(), p["edges"].as_u64()), (Some(3), Some(24)), "{p}");
    for (outward, live) in [(false, 58695.0522), (true, 63461.6237)] {
        let (status, v, faces) = shell(&mut ctx, 2.0, outward);
        assert!(status == "Ok" && faces == 5 && (v - live).abs() < 1e-4 * live, "smooth, 2 mm {}: {status} {v} mm³ (live {live}), {faces} faces", if outward { "out" } else { "in" });
    }
    let mut ctx = Ctx::default();
    loft(&mut ctx, "sharp", false);
    let p = call(&mut ctx, "query_parts", json!({}))["parts"][0].clone();
    assert_eq!(p["faces"].as_u64(), Some(8), "{p}");
    let (status, v, faces) = shell(&mut ctx, 1.6, false);
    assert!(status == "Ok" && faces == 15 && (v - 47868.3858).abs() < 1e-4 * 47868.3858, "sharp, 1.6 mm in: {status} {v} mm³, {faces} faces");
}
