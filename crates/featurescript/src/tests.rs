//! Language tests: syntax, semantics, and the standard library running on
//! the evaluator.

use crate::compile::{NoImports, compile_module};
use crate::interp::Interp;
use crate::value::{Value, to_display};

#[test]
fn whole_standard_library_parses() {
    let mut failures = Vec::new();
    for (name, src) in crate::STD_SOURCES {
        if let Err(e) = crate::parser::parse(src) {
            failures.push(format!("{name}:{e}"));
        }
    }
    assert!(failures.is_empty(), "{} of {} modules failed:\n{}", failures.len(), crate::STD_SOURCES.len(), failures.join("\n"));
}

/// A module path of its own per compile: tests run in parallel and modules
/// register their operators / exported functions by path.
fn unique_path() -> String {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    format!("test{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// Compile `src` (after the usual header) and call `f()`.
fn run(src: &str) -> Result<Value, String> {
    let full = format!("FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n{src}");
    let m = compile_module(&unique_path(), &full, &NoImports).map_err(|e| e.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))?;
    let f = m.functions("f").ok_or("no f")?;
    let mut i = Interp::new();
    i.call_set(&f, vec![]).map_err(|e| format!("{} at {}:{}", e.message(), e.module, e.pos))
}

fn run_str(src: &str) -> String {
    match run(src) {
        Ok(v) => to_display(&v),
        Err(e) => format!("ERROR: {e}"),
    }
}

#[test]
fn arithmetic_and_strings() {
    assert_eq!(run_str("function f() { return 1 + 2 * 3 ^ 2; }"), "19");
    assert_eq!(run_str("function f() { return -7 % 3; }"), "2");
    assert_eq!(run_str("function f() { return 'a' ~ 1 ~ true; }"), "a1true");
    assert_eq!(run_str("function f() { var x = undefined; return x ?? 5; }"), "5");
}

/// Onshape's `?:` is left-associative (live behaviours.md, fs-smprobe).
#[test]
fn conditional_is_left_associative() {
    assert_eq!(run_str("function f() { return false ? 1 : true ? 2 : 3; }"), "2");
    assert_eq!(run_str("function f() { return true ? (false ? 1 : 2) : 3; }"), "2");
    assert_eq!(run_str("function f() { return true ? false ? 1 : 2 : 3; }"), "2");
    let e = run_str("function f() { return true ? \"X\" : false ? \"Y\" : \"Z\"; }");
    assert!(e.contains("First operand of logical or conditional operator must be boolean, value is string"), "{e}");
}

#[test]
fn value_semantics_and_boxes() {
    assert_eq!(run_str("function f() { var a = [1, 2]; var b = a; a[0] = 9; return b; }"), "[ 1 , 2 ]");
    assert_eq!(run_str("function f() { var m = {}; m.k = { 'x' : 1 }; m.k.x += 1; return m; }"), "{ \"k\" : { \"x\" : 2 } }");
    assert_eq!(run_str("function f() { const b = new box(0); const g = function() { b[] += 1; return b[]; }; g(); return g(); }"), "2");
    assert_eq!(run_str("function f() { var m = { a : 1 }; m.a = undefined; return size(m); }"), "0");
}

#[test]
fn closures_capture_by_value() {
    assert_eq!(run_str("function f() { var x = 1; const g = () => x; x = 2; return g(); }"), "1");
    assert_eq!(run_str("function f() { const add = a => b => a + b; return add(2)(3); }"), "5");
    assert_eq!(run_str("function f() { return [3, 1, 2]->sort(function(a, b) { return a - b; }); }"), "[ 1 , 2 , 3 ]");
}

#[test]
fn units_from_the_standard_library() {
    assert_eq!(run_str("function f() { return toString(2 * meter) ~ ' ' ~ toString(3 * squareMeter); }"), "2 meter 3 meter^2");
    assert_eq!(run_str("function f() { return abs((1 * meter + 5 * millimeter) / millimeter - 1005) < 1e-9; }"), "true");
    assert!(run_str("function f() { return 1 * meter + 1; }").starts_with("ERROR"));
    assert_eq!(run_str("function f() { return sqrt(4 * meter ^ 2) == 2 * meter; }"), "true");
}

#[test]
fn enums_and_types() {
    let src = "enum E { A, B }\nfunction f() { return [E.B is E, E.A is string, ('A' as E) == E.A, E.A < E.B, try('Z' as E)]; }";
    assert_eq!(run_str(src), "[ true , true , true , true , undefined ]");
}

#[test]
fn exceptions() {
    assert_eq!(run_str("function f() { try { throw 'x'; } catch (e) { return e; } }"), "x");
    assert_eq!(run_str("function f() { return try(1 / 0 - inf); }"), "undefined");
    assert_eq!(run_str("function g(n is number) precondition n > 0; { return n; }\nfunction f() { return try silent(g(-1)) ?? 'failed'; }"), "failed");
}

#[test]
fn overloads_and_operators() {
    let src = r#"
export type T typecheck isT;
export predicate isT(v) { v is map; }
export operator+(a is T, b is T) returns T { return { 'n' : a.n + b.n } as T; }
function g(x) { return 'any'; }
function g(x is number) { return 'number'; }
function g(x is T) { return 'T'; }
function f() { const t = { 'n' : 1 } as T; return [g(1), g('s'), g(t), (t + t).n]; }
"#;
    assert_eq!(run_str(src), "[ \"number\" , \"any\" , \"T\" , 2 ]");
}

#[test]
fn predicates_and_switch() {
    let src = r#"
predicate small(x) { x is number; x < 10; }
enum K { P, Q }
function f() { return [small(3), small(30), small('a'), switch (K.Q) { K.P : 'p', K.Q : 'q' }]; }
"#;
    assert_eq!(run_str(src), "[ true , false , false , \"q\" ]");
}

#[test]
fn ids_and_vectors() {
    assert_eq!(run_str("function f() { return toString(newId() + 'a' + 1); }"), "[ a , 1 ]");
    assert_eq!(run_str("function f() { return toString(vector(1, 2, 3) * 2); }"), "(2, 4, 6)");
    assert_eq!(run_str("function f() { return norm(vector(3, 4) * meter) == 5 * meter; }"), "true");
}

#[test]
fn whole_standard_library_compiles() {
    let t = std::time::Instant::now();
    if let Err(errs) = crate::compile::std_module("geometry.fs") {
        let shown: Vec<String> = errs.iter().take(40).map(|e| e.to_string()).collect();
        panic!("{} errors:\n{}", errs.len(), shown.join("\n"));
    }
    eprintln!("geometry.fs compiled in {:?}", t.elapsed());
}

/// Real custom features from /root/onshape-featurescripts (skipped when absent).
#[test]
fn real_feature_studios_compile() {
    let Ok(dir) = std::fs::read_dir("/root/onshape-featurescripts") else { return };
    for e in dir.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "fs") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        if let Err(errs) = compile_module(&p.display().to_string(), &src, &NoImports) {
            panic!("{}: {}", p.display(), errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"));
        }
    }
}

#[test]
fn infinite_loops_hit_the_deadline() {
    let full = "FeatureScript 3095;\nfunction f() { var i = 0; while (true) { i += 1; } }";
    let m = compile_module("loop", full, &NoImports).unwrap();
    let f = m.functions("f").unwrap();
    let mut i = Interp::new();
    i.set_deadline(Some(std::time::Instant::now() + std::time::Duration::from_millis(200)));
    let e = i.call_set(&f, vec![]).unwrap_err();
    assert_eq!(e.message(), "Regeneration timed out");
}

/// Notices of a whole source as `[line:col] message` (Onshape's panel order).
fn notices(src: &str) -> Vec<String> {
    match compile_module(&unique_path(), src, &NoImports) {
        Ok(m) => m.warnings.iter().map(|e| format!("[{}:{}] {}", e.pos.line, e.pos.col, e.message)).collect(),
        Err(es) => es.iter().map(|e| if e.pos.line == 0 { e.message.clone() } else { format!("[{}:{}] {}", e.pos.line, e.pos.col, e.message) }).collect(),
    }
}

/// Live captures (onshape-research/live/dom/fs-parse-errors.txt).
#[test]
fn notices_match_onshape() {
    let h = "FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n";
    let cases: &[(&str, &[&str])] = &[
        ("const x = 1\nconst y = 2;\n", &["[4:1] missing TOP_SEMI at 'const'"]),
        ("function f() { var x = ; }\n", &["[3:20] Missing initializer expression"]),
        ("function f() { return 1 + ; }\n", &["[3:27] no viable alternative at input '+ ;'"]),
        (
            "const a = 1;\nimport(path : \"onshape/std/math.fs\", version : \"3095.0\");\n",
            &["[4:1] extraneous input 'import' expecting {<EOF>, 'annotation', 'export', 'enum', 'type', 'function', 'predicate', 'operator', 'typeconvert', 'const', TOP_SEMI}"],
        ),
        ("function f() { (x).y = 1; }\n", &["[3:22] no viable alternative at input '(x).y ='"]),
        ("function f() { x = 1; }\n", &["[3:16] Variable x not found.", "[3:10] Unused declaration: f"]),
        ("const z = undefinedThing + 1;\n", &["[3:11] Variable undefinedThing not found.", "[3:7] Unused declaration: z"]),
        ("export const f2 = function(a) { return a +; };\n", &["[3:43] no viable alternative at input '+;'"]),
        ("const x = 1\nfunction f() { var x = ; }\n", &["[4:1] missing TOP_SEMI at 'function'"]),
        (
            "function f() { x = 1; }\nconst z = undefinedThing + 1;\n",
            &["[3:16] Variable x not found.", "[3:10] Unused declaration: f", "[4:11] Variable undefinedThing not found.", "[4:7] Unused declaration: z"],
        ),
    ];
    let mut bad = Vec::new();
    for (src, want) in cases {
        let got = notices(&format!("{h}{src}"));
        if got != *want {
            bad.push(format!("{src:?}\n  want {want:?}\n  got  {got:?}"));
        }
    }
    // fs-probe-output.txt: compile warnings of the "FS probe" studio.
    let got = notices(include_str!("fsprobe.fs.txt"));
    let want = ["[21:5] Variable m set but not used", "[40:23] Function undefinedFunctionCall with 0 argument(s) not found"];
    if got != want {
        bad.push(format!("fs probe\n  want {want:?}\n  got  {got:?}"));
    }
    // fs-errors3.txt (a): parse warnings at literal operands / conditions.
    let got = notices(include_str!("fserrors3.fs.txt"));
    let want = [
        "[6:10] Unused declaration: two",
        "[22:23] Non-numeric operand",
        "[23:27] Non-numeric operand",
        "[24:27] Non-numeric operand",
        "[25:27] Non-numeric operand",
        "[29:19] Non-boolean operand",
        "[32:23] Function two with 1 argument(s) not found",
    ];
    if got != want {
        bad.push(format!("fs errors3\n  want {want:?}\n  got  {got:?}"));
    }
    let got = notices("import(path : \"onshape/std/common.fs\", version : \"3095.0\");\nconst a = 1;\n");
    let want = ["Missing FeatureScript version", "Cannot import a module with FeatureScript version: 3095 from module with older version 0"];
    if got != want {
        bad.push(format!("no version\n  want {want:?}\n  got  {got:?}"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn numbers_print_like_onshape() {
    // dom/fs-errors3.txt.
    assert_eq!(run_str("function f() { return [1e21, 1e-7, 123456789.123, -0, 1 / 3, 0.05, 12] ~ ''; }"), "[ 1e+21 , 1e-7 , 1.23456789123e+8 , -0 , 0.3333333333333333 , 0.05 , 12 ]");
}

#[test]
fn evaluate_spline_matches_the_curve() {
    // Quadratic Bezier (0,0)-(1,2)-(2,0): at 0.5 the point is (1,1), the
    // tangent (2,0), the second derivative (0,-8).
    let out = run_str("function f() { const c = bSplineCurve(2, false, [vector(0, 0, 0) * meter, vector(1, 2, 0) * meter, vector(2, 0, 0) * meter], knotArray([0, 0, 0, 1, 1, 1])); return toString(evaluateSpline({ 'spline' : c, 'parameters' : [0.5], 'nDerivatives' : 2 })); }");
    assert_eq!(out, "[ [ (1 meter, 1 meter, 0 meter) ] , [ (2 meter, 0 meter, 0 meter) ] , [ (0 meter, -8 meter, 0 meter) ] ]");
}

#[test]
fn evaluate_spline_rational_quarter_circle() {
    let out = run_str("function f() { const c = bSplineCurve(2, false, [vector(1, 0, 0) * meter, vector(1, 1, 0) * meter, vector(0, 1, 0) * meter], [1, sqrt(0.5), 1], knotArray([0, 0, 0, 1, 1, 1])); const p = evaluateSpline({ 'spline' : c, 'parameters' : [0.5] })[0][0]; return abs(p[0] - sqrt(0.5) * meter) < 1e-12 * meter && abs(p[1] - sqrt(0.5) * meter) < 1e-12 * meter; }");
    assert_eq!(out, "true");
}

#[test]
fn lookup_table_expressions_evaluate() {
    // std tables use "0.2415 in", "5 mm", "1/4 in", "1 + 1/2 in".
    assert_eq!(run_str("function f() { return toString(lookupTableEvaluate('0.138 in')); }"), "0.0035052000000000004 meter");
    assert_eq!(run_str("function f() { return toString(lookupTableEvaluate('5 mm')); }"), "0.005 meter");
    assert_eq!(run_str("function f() { return toString(lookupTableEvaluate('1/4 in')); }"), "0.006350000000000001 meter");
}

