//! Live behaviours "qa4 round 5 s3 mate flip" (qa4 MATE-FLIP-AXIAL): a fixed
//! 100×60×10 plate (z 0..10) and a Ø10×40 pin at (80, 0, 0) mm; the pin's
//! top-face connector is mated to the plate's top-face connector.
//! - Revolute, Cylindrical and Planar by default align the Z axes: the pin
//!   hangs through the plate, t (0, 0, −30), Z (0, 0, 1).
//! - Flip primary axis turns it over: t (0, 0, 50), Z (0, 0, −1).
//! - The pin laid on its side first (its Z perpendicular to the plate's)
//!   gets the default result too; started upside down (Z opposed) it keeps
//!   that pose: the flip is set automatically, t (0, 0, 50), Z (0, 0, −1)
//!   (live "skb (10-25) axial mate default flip").

use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};

fn call(ctx: &mut Ctx, tool: &str, args: Value) -> Value {
    let op = registry::by_tool_name(tool).unwrap_or_else(|| panic!("no tool {tool}"));
    registry::run(ctx, op, &args).unwrap_or_else(|e| panic!("{tool} failed: {}", e.to_json())).value
}

fn studio(ctx: &mut Ctx, tab: &str, draw: Value, depth: &str) {
    let sk = call(ctx, "sketch_create", json!({ "plane": "plane:top", "tab": tab }))["sketch"].as_str().unwrap().to_string();
    let mut d = draw;
    d["sketch"] = json!(sk);
    d["tab"] = json!(tab);
    call(ctx, "sketch_draw", d);
    let region = call(ctx, "sketch_get", json!({ "sketch": sk, "tab": tab }))["regions"][0]["id"].as_str().unwrap().to_string();
    call(ctx, "feature_add", json!({ "type": "extrude", "params": { "entities": [region], "depth": depth }, "tab": tab }));
}

/// The face of `inst` whose world normal is `n`.
fn face(ctx: &mut Ctx, inst: &str, n: [f64; 3]) -> String {
    let faces = call(ctx, "assembly_entities", json!({ "instance": inst, "kind": "faces" }));
    let f = faces["faces"].as_array().unwrap().iter().find(|f| (0..3).all(|k| (f["normal"][k].as_f64().unwrap_or(9.0) - n[k]).abs() < 1e-6)).expect("face");
    f["id"].as_str().unwrap().to_string()
}

/// The pin's translation (mm) and Z axis after a mate.
#[derive(Clone, Copy, Debug)]
enum Start {
    Upright,
    Side,
    UpsideDown,
}

fn mate(kind: &str, flip: Option<bool>, start: Start) -> ([f64; 3], [f64; 3]) {
    let dir = std::env::temp_dir().join(format!("noshape-mate-flip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{kind}-{flip:?}-{start:?}.noshape"));
    let _ = std::fs::remove_file(&path);
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    studio(&mut ctx, "Part Studio 1", json!({ "tool": "corner_rectangle", "points": [[-50, -30], [50, 30]] }), "10 mm");
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "pin" }));
    studio(&mut ctx, "pin", json!({ "tool": "circle", "points": [[0, 0], [5, 0]] }), "40 mm");
    call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
    call(&mut ctx, "assembly_insert", json!({ "source": "pin", "part": "Part 1" }));
    call(&mut ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>"], "fixed": true }));
    let (t, top) = match start {
        Start::Upright => (json!({ "instance": "Part 1 <2>", "translation": [80, 0, 0] }), [0.0, 0.0, 1.0]),
        Start::Side => (json!({ "instance": "Part 1 <2>", "translation": [80, 0, 0], "axis": [1, 0, 0], "angle": "90 deg" }), [0.0, -1.0, 0.0]),
        Start::UpsideDown => (json!({ "instance": "Part 1 <2>", "translation": [80, 0, 40], "axis": [1, 0, 0], "angle": "180 deg" }), [0.0, 0.0, -1.0]),
    };
    call(&mut ctx, "assembly_transform", t);
    let a = face(&mut ctx, "Part 1 <2>", top);
    let b = face(&mut ctx, "Part 1 <1>", [0.0, 0.0, 1.0]);
    let mut args = json!({ "type": kind, "a": { "instance": "Part 1 <2>", "entity": a, "inference": "centroid" }, "b": { "instance": "Part 1 <1>", "entity": b, "inference": "centroid" } });
    if let Some(f) = flip {
        args["flip_primary"] = json!(f);
    }
    call(&mut ctx, "assembly_mate", args);
    let list = call(&mut ctx, "assembly_list", json!({}));
    let pin = list["instances"].as_array().unwrap().iter().find(|i| i["name"] == "Part 1 <2>").unwrap().clone();
    let t: Vec<f64> = pin["translation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let q: Vec<f64> = pin["rotation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    let zax = [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)];
    let _ = std::fs::remove_file(&path);
    ([t[0], t[1], t[2]], zax)
}

fn near(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < tol)
}

#[test]
fn axial_mates_default_align_and_flip_turns_over() {
    for kind in ["revolute", "cylindrical", "planar"] {
        // The CLI's flip_primary is the model's: true = Z axes aligned.
        let (t, z) = mate(kind, None, Start::Upright);
        assert!(near(t, [0.0, 0.0, -30.0], 1e-2) && near(z, [0.0, 0.0, 1.0], 1e-6), "{kind} default: t {t:?} Z {z:?}");
        let (t, z) = mate(kind, Some(false), Start::Upright);
        assert!(near(t, [0.0, 0.0, 50.0], 1e-2) && near(z, [0.0, 0.0, -1.0], 1e-6), "{kind} flipped: t {t:?} Z {z:?}");
    }
    for kind in ["revolute", "fastened"] {
        let (t, z) = mate(kind, None, Start::Side);
        assert!(near(t, [0.0, 0.0, -30.0], 1e-2) && near(z, [0.0, 0.0, 1.0], 1e-6), "{kind} from the side: t {t:?} Z {z:?}");
    }
    for kind in ["revolute", "planar"] {
        let (t, z) = mate(kind, None, Start::UpsideDown);
        assert!(near(t, [0.0, 0.0, 50.0], 1e-2) && near(z, [0.0, 0.0, -1.0], 1e-6), "{kind} from upside down: t {t:?} Z {z:?}");
    }
}

/// A Width mate in any pick order (api benchmark F19): the block's two X
/// faces and the base's two X faces, given as the documented roles or both
/// block faces first: the block is centred between the base's faces and the
/// mate solves (live: OK in every order).
#[test]
fn width_mate_solves_in_any_face_order() {
    let dir = std::env::temp_dir().join(format!("noshape-width-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (k, order) in ["ab|cd", "ba|cd", "ab|dc", "ba|dc", "documented"].iter().enumerate() {
        let path = dir.join(format!("width-{k}.noshape"));
        let _ = std::fs::remove_file(&path);
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        studio(&mut ctx, "Part Studio 1", json!({ "tool": "corner_rectangle", "points": [[-50, -30], [50, 30]] }), "10 mm");
        call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "block" }));
        studio(&mut ctx, "block", json!({ "tool": "corner_rectangle", "points": [[-10, -10], [10, 10]] }), "10 mm");
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "source": "block", "part": "Part 1" }));
        call(&mut ctx, "assembly_set_state", json!({ "instances": ["Part 1 <1>"], "fixed": true }));
        call(&mut ctx, "assembly_transform", json!({ "instance": "Part 1 <2>", "translation": [17, 5, 10] }));
        let (bl, br) = (face(&mut ctx, "Part 1 <2>", [-1.0, 0.0, 0.0]), face(&mut ctx, "Part 1 <2>", [1.0, 0.0, 0.0]));
        let (wl, wr) = (face(&mut ctx, "Part 1 <1>", [-1.0, 0.0, 0.0]), face(&mut ctx, "Part 1 <1>", [1.0, 0.0, 0.0]));
        let c = |inst: &str, f: &str| json!({ "instance": inst, "entity": f });
        let (blk, base) = ("Part 1 <2>", "Part 1 <1>");
        let picks = match *order {
            "ab|cd" => [c(blk, &bl), c(blk, &br), c(base, &wl), c(base, &wr)],
            "ba|cd" => [c(blk, &br), c(blk, &bl), c(base, &wl), c(base, &wr)],
            "ab|dc" => [c(blk, &bl), c(blk, &br), c(base, &wr), c(base, &wl)],
            "ba|dc" => [c(blk, &br), c(blk, &bl), c(base, &wr), c(base, &wl)],
            _ => [c(blk, &bl), c(base, &wl), c(base, &wr), c(blk, &br)],
        };
        let [a, b, cc, d] = picks;
        let r = call(&mut ctx, "assembly_mate", json!({ "type": "width", "a": a, "b": b, "c": cc, "d": d }));
        assert_eq!(r["mate_ok"], json!(true), "{order}: {r}");
        let list = call(&mut ctx, "assembly_list", json!({}));
        let block = list["instances"].as_array().unwrap().iter().find(|i| i["name"] == "Part 1 <2>").unwrap().clone();
        let x = block["translation"][0].as_f64().unwrap();
        assert!(x.abs() < 1e-6, "{order}: centred, x {x}");
        let _ = std::fs::remove_file(&path);
    }
}

/// Live (behaviours "skasm (2026-10-27) follow-up: which part moves with
/// nothing fixed"): a new mate between two free parts moves one of them,
/// never both. Without a flip the smaller part (the pin) moves, whichever
/// connector was picked first; the plate keeps its placement exactly.
#[test]
fn new_mate_between_free_parts_moves_the_smaller_one() {
    for plate_first in [false, true] {
        let dir = std::env::temp_dir().join(format!("noshape-free-mate-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("free-{plate_first}.noshape"));
        let _ = std::fs::remove_file(&path);
        let mut ctx = Ctx::new();
        call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
        studio(&mut ctx, "Part Studio 1", json!({ "tool": "corner_rectangle", "points": [[-50, -30], [50, 30]] }), "10 mm");
        call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "pin" }));
        studio(&mut ctx, "pin", json!({ "tool": "circle", "points": [[0, 0], [5, 0]] }), "40 mm");
        call(&mut ctx, "assembly_insert", json!({ "source": "Part Studio 1", "part": "Part 1" }));
        call(&mut ctx, "assembly_insert", json!({ "source": "pin", "part": "Part 1" }));
        call(&mut ctx, "assembly_transform", json!({ "instance": "Part 1 <1>", "translation": [5, -7, 3] }));
        call(&mut ctx, "assembly_transform", json!({ "instance": "Part 1 <2>", "translation": [80, 0, 0] }));
        // The pin's bottom face on the plate's top face: the Zs are opposed already.
        let pin = json!({ "instance": "Part 1 <2>", "entity": face(&mut ctx, "Part 1 <2>", [0.0, 0.0, -1.0]), "inference": "centroid" });
        let plate = json!({ "instance": "Part 1 <1>", "entity": face(&mut ctx, "Part 1 <1>", [0.0, 0.0, 1.0]), "inference": "centroid" });
        let (a, b) = if plate_first { (plate, pin) } else { (pin, plate) };
        call(&mut ctx, "assembly_mate", json!({ "type": "revolute", "a": a, "b": b }));
        let list = call(&mut ctx, "assembly_list", json!({}));
        let t = |name: &str| -> [f64; 3] {
            let i = list["instances"].as_array().unwrap().iter().find(|i| i["name"] == name).unwrap();
            let v: Vec<f64> = i["translation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
            [v[0], v[1], v[2]]
        };
        assert!(near(t("Part 1 <1>"), [5.0, -7.0, 3.0], 1e-9), "plate first {plate_first}: the plate moved to {:?}", t("Part 1 <1>"));
        assert!(near(t("Part 1 <2>"), [5.0, -7.0, 13.0], 1e-6), "plate first {plate_first}: the pin at {:?}", t("Part 1 <2>"));
        let _ = std::fs::remove_file(&path);
    }
}

/// A new mate from the assembly Origin (picked first) to a free part moves
/// the part: the origin never moves.
#[test]
fn new_mate_from_the_origin_moves_the_part() {
    let dir = std::env::temp_dir().join(format!("noshape-origin-mate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("origin.noshape");
    let _ = std::fs::remove_file(&path);
    let mut ctx = Ctx::new();
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    call(&mut ctx, "tab_create", json!({ "kind": "partstudio", "name": "pin" }));
    studio(&mut ctx, "pin", json!({ "tool": "circle", "points": [[0, 0], [5, 0]] }), "40 mm");
    call(&mut ctx, "assembly_insert", json!({ "source": "pin", "part": "Part 1" }));
    call(&mut ctx, "assembly_transform", json!({ "instance": "Part 1 <1>", "translation": [30, 20, 10] }));
    let pin = json!({ "instance": "Part 1 <1>", "entity": face(&mut ctx, "Part 1 <1>", [0.0, 0.0, -1.0]), "inference": "centroid" });
    let r = call(&mut ctx, "assembly_mate", json!({ "type": "fastened", "a": { "origin": true }, "b": pin }));
    assert_eq!(r["mate_ok"], json!(true), "{r}");
    let list = call(&mut ctx, "assembly_list", json!({}));
    let i = list["instances"].as_array().unwrap().iter().find(|i| i["name"] == "Part 1 <1>").unwrap();
    let t: Vec<f64> = i["translation"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!(t.iter().all(|v| v.abs() < 1e-6), "the pin's bottom centre on the origin: {t:?}");
    let _ = std::fs::remove_file(&path);
}
