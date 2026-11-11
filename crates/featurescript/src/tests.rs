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

/// Every shipped release unpacks (its pack still matches the embedded std it was packed against: else
/// tools/std-pack.py repacks it) and parses.
#[test]
fn every_shipped_standard_library_unpacks_and_parses() {
    let mut failures = Vec::new();
    for v in crate::std_versions() {
        let files = crate::std_files(v).expect("shipped");
        assert!(files.iter().any(|(n, _)| *n == "geometry.fs") && files.windows(2).all(|w| w[0].0 < w[1].0), "std {v}: {} modules", files.len());
        for (name, src) in files {
            if let Err(e) = crate::parser::parse(src) {
                failures.push(format!("std {v} {name}:{e}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every shipped release compiles (geometry.fs and what it imports: a release's modules import its own, whatever
/// version their imports name: 3044's box.fs names math.fs at 3008).
/// The older releases' packs are made against the embedded release's bytes: a checkout that turns std's line ends
/// into CRLF (git's text conversion on Windows, without .gitattributes' `-text`) fails here on every platform, not
/// only when a pack unpacks on Windows.
#[test]
fn the_embedded_standard_library_has_no_carriage_returns() {
    let crlf: Vec<&str> = crate::STD_SOURCES.iter().filter(|(_, src)| src.contains('\r')).map(|(name, _)| *name).collect();
    assert!(crlf.is_empty(), "carriage returns in {crlf:?} (.gitattributes: -text on crates/featurescript/std/**)");
}

/// A pack that doesn't unpack (made against other bytes) and a release not shipped are errors, never a panic.
#[test]
fn a_pack_that_does_not_unpack_is_an_error() {
    let (_, pack) = crate::STD_PACKS[0];
    let mut bad = pack.to_vec();
    let mid = bad.len() / 2;
    bad[mid] ^= 0xff;
    assert!(crate::unpack(&bad).is_err());
    assert_eq!(crate::std_files(1).err().as_deref(), Some("Standard library 1 is not available"));
}

#[test]
fn every_shipped_standard_library_compiles() {
    let failures: Vec<String> = crate::std_versions()
        .into_iter()
        .filter_map(|v| crate::compile::std_module_at(v, "geometry.fs").err().map(|e| format!("std {v}: {}", e.iter().map(|e| e.to_string()).take(3).collect::<Vec<_>>().join("; "))))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A pin gets the newest shipped release at or below it (Onshape: exactly its own); one older than all, the
/// oldest; one newer, the embedded release.
#[test]
fn a_pin_gets_the_newest_release_at_or_below_it() {
    let shipped = crate::std_versions();
    let (oldest, newest) = (shipped[0], *shipped.last().unwrap());
    assert_eq!(newest, crate::STD_VERSION);
    for w in shipped.windows(2) {
        assert_eq!(crate::std_version_for(w[0]), w[0]);
        assert_eq!(crate::std_version_for(w[1] - 1), w[0]);
    }
    assert_eq!(crate::std_version_for(oldest.saturating_sub(1)), oldest);
    assert_eq!(crate::std_version_for(newest + 100), newest);
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
    // `~` binds looser than `+` (live runs a Fill pattern's "Pattern contains " ~ size(names) + 1 ~ " instances.").
    assert_eq!(run_str("function f() { return 'n ' ~ 2 + 1 ~ ' m'; }"), "n 3 m");
    assert_eq!(run_str("function f() { var x = undefined; return x ?? 5; }"), "5");
}

/// Operator precedence as live answers it (behaviours.md "FeatureScript precedence", request 1999): `is` / `as`
/// below `==` and above `&&`, `~` looser than all of them, even `||`.
#[test]
fn precedence_is_lives() {
    let cases = [
        ("1 + 1 is number", "true"),
        ("\"a\" ~ 1 is string", "afalse"),
        ("1 < 2 is boolean", "true"),
        ("1 == 1 is boolean", "true"),
        ("2 + 2 as number", "4"),
        ("\"a\" ~ 1 == 1", "atrue"),
        ("true && 1 is number", "true"),
        ("false || 1 is boolean", "false"),
        ("2 * 3 is number", "true"),
        ("\"a\" ~ 2 + 2", "a4"),
        ("\"a\" ~ 2 as number", "a2"),
        ("1 is number ? \"y\" : \"n\"", "y"),
        ("!true is boolean", "true"),
        ("1 is number == true", "true"),
        ("\"a\" ~ 1 < 2", "atrue"),
        ("1 + 1 as number is number", "true"),
        ("1 is number is boolean", "true"),
        ("\"a\" ~ true && false", "afalse"),
        ("\"a\" ~ false || true", "atrue"),
        ("2 * 3 as number", "6"),
        ("2 ^ 2 is number", "true"),
        ("1 is number && 2 is number", "true"),
        ("1 == 1 as boolean", "true"),
        ("1 < 2 as boolean", "true"),
        ("-1 is number", "true"),
        ("true ? 1 : 2 is number", "1"),
        ("1 != 2 is boolean", "true"),
        ("5 % 3 is number", "true"),
    ];
    for (expr, want) in cases {
        assert_eq!(run_str(&format!("function f() {{ return {expr}; }}")), want, "{expr}");
    }
}

/// String literals: plain runs, escapes after and between them, either
/// quote, and an unterminated string's error.
#[test]
fn string_literals_lex() {
    use crate::lexer::{Tok, lex};
    let strs = |src: &str| lex(src).unwrap().into_iter().filter_map(|t| if let Tok::Str(s) = t.tok { Some(s.to_string()) } else { None }).collect::<Vec<_>>();
    assert_eq!(strs(r#""0.3338 in" 'a"b' "x\ty\"z" "\u00e9t\u00e9" "" 'it\'s' "é\n""#), ["0.3338 in", "a\"b", "x\ty\"z", "été", "", "it's", "é\n"]);
    let e = lex("const s = \"ab\ncd\";").unwrap_err();
    assert_eq!((e.pos.line, e.pos.col, e.message.as_str()), (1, 11, "token recognition error at: '\"ab'"));
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

/// A typecheck type tags a function (`function(t) { … } as Expression`): `is` sees the type and operators overloaded for
/// it apply to it (cf-parametric-curve's symbolic parameter `#t`, live OK).
#[test]
fn a_function_takes_a_type() {
    let src = r#"
export type Expression typecheck canBeExpression;
predicate canBeExpression(value) { value is function; }
const t_ = function(t) { return t; } as Expression;
export operator*(lhs is Expression, rhs) returns Expression { return function(t) { return lhs(t) * rhs; } as Expression; }
function f() { const twice = t_ * 2; return [t_ is Expression, twice is Expression, twice(3), (function(t) { return t; }) is Expression]; }
"#;
    assert_eq!(run_str(src), "[ true , true , 6 , false ]");
}

#[test]
fn enums_and_types() {
    let src = "enum E { A, B }\nfunction f() { return [E.B is E, E.A is string, ('A' as E) == E.A, E.A < E.B, try('Z' as E)]; }";
    assert_eq!(run_str(src), "[ true , true , true , true , undefined ]");
    // A comma after the last value (live compiles cf-straight-spline's enum).
    assert_eq!(run_str("enum S { A, B, }\nfunction f() { return S.B is S; }"), "true");
}

/// A module pinned before std 1963 may name a type in another case (live compiles FeatureScript 369's O-Rings, `var
/// … is vector = vector(…)`); a current one may not.
#[test]
fn old_modules_name_types_in_any_case() {
    let compile = |version: u32| {
        let src = format!("FeatureScript {version};\nimport(path : \"onshape/std/geometry.fs\", version : \"{version}.0\");\nfunction f() {{ var v is vector = vector(1, 2); return v[1]; }}");
        compile_module(&unique_path(), &src, &NoImports).map(|m| {
            let f = m.functions("f").expect("f");
            to_display(&Interp::new().call_set(&f, vec![]).expect("runs"))
        })
    };
    assert_eq!(compile(369).expect("compiles at 369"), "2");
    let e = compile(3095).expect_err("an error at 3095");
    assert!(e.iter().any(|e| e.message == "Type vector not found"), "{e:?}");
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

/// `x = append(x, v)` and its kin grow x in place: the same values as the call written apart (`y = append(x, v); x =
/// y`), with the type tag, another variable's copy, a module's own overload and the errors as written.
#[test]
fn growing_an_array_in_place_answers_as_the_call() {
    let pre = "export type T typecheck isT;\nexport predicate isT(v) { v is array; }\n\
               function append(arr is array, v is string) returns array { return ['own ' ~ v]; }\n";
    let cases = [
        ("var a = [1]; const b = a;", "append(a, 2)", "return [a, b];"),
        ("var a = [1] as T;", "append(a, 2)", "return [a, a is T];"),
        ("var a = [1] as T;", "concatenateArrays(a, [2, 3])", "return [a, a is T];"),
        ("var a = [1] as T;", "@resize(a, 3, 7)", "return [a, a is T];"),
        ("var a = [1, 2, 3];", "@resize(a, 1)", "return a;"),
        ("var a = [1];", "append(a, 'x')", "return a;"),
        ("var a = [1];", "append(a, a)", "return a;"),
        ("var a = 5; try {", "append(a, 1)", "} catch (e) { return [a, e]; } return a;"),
        ("var a = [1]; try {", "@resize(a, -1)", "} catch (e) { return [a, e]; } return a;"),
        ("var a = [1]; try {", "concatenateArrays(a, 2)", "} catch (e) { return [a, e]; } return a;"),
        ("var a = 5;", "append(a, 1)", "return a;"),
        ("var a = [1];", "@resize(a, -1)", "return a;"),
        ("var a = [1];", "concatenateArrays(a, 2)", "return a;"),
    ];
    let message = |r: String| r.split(" at test").next().unwrap_or_default().to_string();
    for (head, call, tail) in cases {
        let in_place = format!("{pre}function f() {{ {head} a = {call}; {tail} }}");
        let apart = format!("{pre}function f() {{ {head} var y = {call}; a = y; {tail} }}");
        assert_eq!(message(run_str(&in_place)), message(run_str(&apart)), "{head} {call}");
    }
}

/// Building an array element by element is linear, as in Onshape (a Displacement map appends 262,144 parameters; a
/// copy per append took minutes).
#[test]
fn building_large_arrays_is_linear() {
    let src = "function f() {\n\
               var p = [];\n\
               for (var i = 0; i < 512; i += 1) { for (var j = 0; j < 512; j += 1) { p = append(p, vector(i / 511, j / 511)); } }\n\
               const q = mapArray(p, function(v) { return v[0]; });\n\
               const r = filter(q, function(x) { return x > 0.5; });\n\
               var s = [];\n\
               for (var k = 0; k < 512; k += 1) { s = concatenateArrays(s, [k, k]); }\n\
               return [size(p), size(q), size(r), size(s)];\n}";
    // std compiled first (once a process): only the run is timed. ~0.3 s alone; a copy per append took minutes.
    assert_eq!(run_str("function f() { return size(append([], 1)); }"), "1");
    let t = std::time::Instant::now();
    assert_eq!(run_str(src), "[ 262144 , 262144 , 131072 , 1024 ]");
    let secs = t.elapsed().as_secs_f64();
    assert!(secs < 3.0, "262,144 appends, a mapArray and a filter of them took {secs:.2} s");
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
    // Real Feature Studios from github.com/yekta/onshape-featurescripts: that
    // repo has no licence, so they are not copied in as fixtures.
    const DIR: &str = "/root/onshape-featurescripts";
    let Ok(dir) = std::fs::read_dir(DIR) else {
        eprintln!("real_feature_studios_compile skipped: {DIR} is not on this machine (clone github.com/yekta/onshape-featurescripts there to run it)");
        return;
    };
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

/// A cancelled run's frames (a big array in one) come back to be freed off the run's thread, not as the calls unwind.
#[test]
fn a_cancelled_run_hands_back_its_frames() {
    let full = "FeatureScript 3095;\nfunction g(a) { var i = 0; while (true) { i += 1; } }\nfunction f() { var a = [1, 2, 3]; return g(a); }";
    let m = compile_module("cancel", full, &NoImports).unwrap();
    let f = m.functions("f").unwrap();
    let mut i = Interp::new();
    let start = std::time::Instant::now();
    i.set_cancel(Some(Box::new(move || start.elapsed() > std::time::Duration::from_millis(50))));
    let e = i.call_set(&f, vec![]).unwrap_err();
    assert_eq!(e.message(), "Regeneration cancelled");
    let frames = i.take_stopped_frames();
    assert_eq!(frames.len(), 2, "g's frame and f's");
    assert!(frames.iter().flatten().any(|v| v.as_array().is_some_and(|a| a.len() == 3)), "the array among them");
    assert!(i.take_stopped_frames().is_empty());
}

/// A cancel or the deadline goes through `try` (statement and expression, silent too): the run stops, not its
/// fallback.
#[test]
fn try_does_not_catch_a_cancel_or_the_deadline() {
    let full = "FeatureScript 3095;\nfunction spin() { var i = 0; while (true) { i += 1; } }\n\
        function f() { try silent { spin(); } catch { return \"fallback\"; } return \"after\"; }\n\
        function g() { try(spin()); return \"after\"; }";
    let m = compile_module("stop", full, &NoImports).unwrap();
    for name in ["f", "g"] {
        let func = m.functions(name).unwrap();
        let mut i = Interp::new();
        let start = std::time::Instant::now();
        i.set_cancel(Some(Box::new(move || start.elapsed() > std::time::Duration::from_millis(30))));
        assert_eq!(i.call_set(&func, vec![]).unwrap_err().message(), "Regeneration cancelled", "{name}");
        let mut i = Interp::new();
        i.set_deadline(Some(std::time::Instant::now() + std::time::Duration::from_millis(30)));
        assert_eq!(i.call_set(&func, vec![]).unwrap_err().message(), "Regeneration timed out", "{name}");
    }
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


/// std's ValueWithUnits and Vector operators and its length predicates answer here as their FeatureScript bodies do:
/// the values, and where the bodies fail (units that differ, a NaN, a vector of another size), the same errors.
#[test]
fn std_unit_and_vector_operators_answer_as_their_bodies() {
    let cases = [
        "1 * meter + 2 * meter", "meter - inch", "-(3 * meter)", "2 * meter", "meter * 2", "meter * meter", "meter / meter",
        "meter / second", "1 / meter", "meter / 2", "meter < inch", "meter < 0", "0 < meter", "inch > meter", "meter <= meter",
        "meter < 1", "meter + second", "(0 * meter) / 0", "meter * meter / meter", "degree * 2", "radian / degree",
        "vector(1, 2, 3) * meter", "meter * vector(1, 2, 3)", "vector(1, 2) * meter + vector(3, 4) * meter",
        "vector(1, 2) * meter - vector(3, 4) * inch", "-(vector(1, 2) * meter)", "vector(1, 2) * meter / 2",
        "vector(1, 2) / 2", "2 * vector(1, 2)", "vector(1, 2) + vector(1, 2, 3)", "vector(1, 2) * meter + vector(1, 2)",
        "vector(1, 2) * vector(3, 4)", "vector(1, 2) * meter * meter", "vector(1, meter)", "vector('a', 2, [3])", "vector([1, 2])", "size([1, 2, [3]])", "size({ 'a' : 1 })", "size([])", "size('ab')", "vector(1, 2) * meter / meter", "meter ^ 2",
        "squaredNorm(vector(3, 4) * meter)", "norm(vector(3, 4) * meter) == 5 * meter", "(vector(1, 2) as Vector) * (meter / second)",
        "isLength(meter)", "isLength(inch)", "isLength(1)", "isLength(meter * meter)", "isLength(degree)",
        "isLength({ 'value' : 1, 'unit' : { 'meter' : 1 } } as ValueWithUnits)", "isLength({ 'value' : 1, 'unit' : { 'meter' : 1 } })",
        "isLengthVector(vector(1, 2) * meter)", "isLengthVector([meter, meter])", "isLengthVector(vector(1, 2))",
        "is3dLengthVector(vector(1, 2, 3) * meter)", "is3dLengthVector(vector(1, 2) * meter)",
        "canBeControlPointMatrix([[vector(1, 2, 3) * meter, vector(1, 2, 3) * inch], [vector(1, 2, 3) * meter, vector(0, 0, 0) * meter]])",
        "canBeControlPointMatrix([[vector(1, 2, 3) * meter], [vector(1, 2, 3) * meter, vector(1, 2, 3) * meter]])",
        "canBeControlPointMatrix([[vector(1, 2, 3) * meter], 5])", "canBeControlPointMatrix([])", "canBeControlPointMatrix(5)",
        "canBeControlPointMatrix([[vector(1, 2) * meter]])",
    ];
    let message = |r: String| r.split(" at test").next().unwrap_or_default().to_string();
    for c in cases {
        let src = format!("function f() {{ return toString({c}); }}");
        let native = run_str(&src);
        crate::std_ops::OFF.with(|o| o.set(true));
        let body = run_str(&src);
        crate::std_ops::OFF.with(|o| o.set(false));
        assert_eq!(message(native), message(body), "{c}");
    }
    // Operands held by locals stay as they were (answers reuse only temporaries' maps and arrays); a vector whose
    // second element has no answer gets std's error from untouched operands.
    let bodies = [
        "const a = vector(1, 2) * meter; const b = a + vector(3, 4) * meter; const c = vector(5, 6) * meter - a; return [a, b, c, -a, a * 2, 2 * a, a / 2, a, b];",
        "var a = meter; const b = a + 2 * meter; const c = 3 * meter + a; a = -a; return [a, b, c, a * 4, a / 2];",
        "const n = vector(1, 0, 0); const o = 2 * meter; return [o * n, n * o, n, o, vector(1, 2, 3) * meter + o * n];",
        "return ([meter, 2] as Vector) + ([meter, inch] as Vector);",
        "const a = [meter, inch] as Vector; const b = ([2 * meter, 1] as Vector) + a; return [a, b];",
    ];
    for b in bodies {
        let src = format!("function f() {{ {b} }}");
        let native = run_str(&src);
        crate::std_ops::OFF.with(|o| o.set(true));
        let body = run_str(&src);
        crate::std_ops::OFF.with(|o| o.set(false));
        assert_eq!(native.starts_with("ERROR"), b.contains("as Vector)"), "{b}: {native}");
        assert_eq!(message(native), message(body), "{b}");
    }
}


/// std_ops answers std's operators and length predicates natively from std_ops::SINCE: every shipped release from
/// there has them as the embedded release has (a new release that changes one fails here: move SINCE past it).
#[test]
fn natively_answered_std_texts_are_the_embedded_releases_from_since() {
    let ops = |src: &str, file: &str| -> Vec<String> {
        let heads: &[&str] = match file {
            "units.fs" => &["export operator", "export predicate isLength(val)", "export function reciprocal(", "export function unitProduct(", "export const LENGTH_UNITS"],
            "vector.fs" => &["export operator", "export predicate isLengthVector(", "export predicate is3dLengthVector(", "export function vector(x, y)", "export function vector(x, y, z)"],
            "containers.fs" => &["export function size(container is array)", "export function size(container is map)"],
            _ => &["export predicate canBeControlPointMatrix("],
        };
        let mut out = Vec::new();
        for head in heads {
            let mut rest = src;
            while let Some(i) = rest.find(head) {
                let tail = &rest[i..];
                let end = tail[1..].find("\nexport ").map(|e| e + 1).unwrap_or(tail.len());
                out.push(tail[..end].split("\n/**").next().unwrap_or("").trim().to_string());
                rest = &tail[end..];
            }
        }
        out
    };
    for file in ["units.fs", "vector.fs", "surfaceGeometry.fs", "containers.fs"] {
        let now = ops(crate::std_source_at(crate::STD_VERSION, file).expect("embedded"), file);
        for v in crate::std_versions().into_iter().filter(|v| *v >= crate::std_ops::SINCE) {
            let theirs = ops(crate::std_source_at(v, file).expect("shipped"), file);
            let differ: Vec<&String> = now.iter().filter(|x| !theirs.contains(x) && !x.starts_with("export operator^")).collect();
            assert!(differ.is_empty(), "std {v} {file}: {:?}", differ.iter().map(|d| d.lines().next().unwrap_or("")).collect::<Vec<_>>());
        }
    }
}

/// The interpreter's speed on a heavy community feature's own FeatureScript: the Displacement map's loops over an
/// image at NOSHAPE_FS_BENCH_N² (default 1024², a million samples; tests/fs/displacement_map.fs) on planes made as
/// the host makes evFaceTangentPlanes' (lead 11-05: the script's own part under 1 s at 1024²). Each step's time.
#[test]
#[ignore = "T2: timing, ~10 s"]
fn displacement_map_loops_timing() {
    use crate::value::{Map, Tag, UNIT_KEY, VALUE_KEY, tag_of_key};
    use std::sync::Arc;
    let n: usize = std::env::var("NOSHAPE_FS_BENCH_N").ok().and_then(|s| s.parse().ok()).unwrap_or(1024);
    let src = r#"FeatureScript 3044;
import(path : "onshape/std/common.fs", version : "3044.0");
function remap(value is number, fromLow is number, fromHigh is number, toLow is number, toHigh is number) returns number
{
    return toLow + (value - fromLow) * (toHigh - toLow) / (fromHigh - fromLow);
}
function check(table is array)
{
    for (var row in table)
    {
        if (size(row) != size(table[0]))
            throw regenError("Every row of the CSV file needs the same number of values.", ["imageTable"]);
        for (var item in row)
        {
            if (!(item is number))
                throw regenError("The CSV file must hold only pixel values (0 to 255).", ["imageTable"]);
        }
    }
    return table;
}
function parameters(rowCount is number, columnCount is number)
{
    var parameters = [];
    for (var i = 0; i < rowCount; i += 1)
    {
        for (var j = 0; j < columnCount; j += 1)
        {
            parameters = append(parameters, vector(i / (rowCount - 1), j / (columnCount - 1)));
        }
    }
    return parameters;
}
function preconditionLoop(parameters is array)
{
    for (var uv in parameters)
    {
        uv is Vector || uv is MeshFaceParameter;
        @size(uv) == 2;
    }
    return true;
}
function points(table is array, planes is array)
{
    const definition = { "blackValue" : 0 * meter, "whiteValue" : 1 * millimeter };
    const columnCount = size(table[0]);
    var pointsTable = [];
    for (var i, row in table)
    {
        var pointRow = [];
        for (var j, item in row)
        {
            const plane = planes[i * columnCount + j];
            const offset = remap(item, 0, 255, definition.blackValue / meter, definition.whiteValue / meter) * meter;
            pointRow = append(pointRow, plane.origin + offset * plane.normal);
        }
        pointsTable = append(pointsTable, pointRow);
    }
    return pointsTable;
}
"#;
    let m = compile_module(&unique_path(), src, &NoImports).unwrap_or_else(|e| panic!("{}", e.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")));
    let tag = |k: &str| tag_of_key(k).unwrap_or(Tag::NONE);
    let (vwu, spec, vector, plane) = (tag("onshape/std/units.fs#ValueWithUnits"), tag("onshape/std/units.fs#UnitSpec"), tag("onshape/std/vector.fs#Vector"), tag("onshape/std/surfaceGeometry.fs#Plane"));
    let meter = Value::Map(Arc::new(Map::from([(Value::key("meter"), Value::num(1.0))])), spec);
    let length = |x: f64| Value::Map(Arc::new(Map::from([(UNIT_KEY.clone(), meter.clone()), (VALUE_KEY.clone(), Value::num(x))])), vwu);
    let keys = ["origin", "normal", "x"].map(Value::key);
    let planes: Vec<Value> = (0..n * n)
        .map(|k| {
            let (u, v) = ((k / n) as f64 / n as f64, (k % n) as f64 / n as f64);
            let (s, c) = (u * std::f64::consts::TAU).sin_cos();
            // (As the host makes them: compact while FeatureScript runs on compact values.)
            let three = |p: [f64; 3], unit: Option<&Value>| match (crate::interp::compact_values(), unit) {
                (true, Some(u)) => Value::Vector(crate::value::Compact { v: p, unit: crate::value::unit_id(u).expect("meter"), len: crate::value::VecLen::Three }),
                (true, None) => Value::Vector(crate::value::Compact { v: p, unit: crate::value::UnitId(0), len: crate::value::VecLen::Three }),
                (false, Some(_)) => Value::Array(Arc::new(p.iter().map(|x| length(*x)).collect()), vector),
                (false, None) => Value::Array(Arc::new(p.iter().map(|x| Value::num(*x)).collect()), vector),
            };
            let origin = three([0.025 * c, 0.025 * s, 0.06 * v], Some(&meter));
            let normal = three([c, s, 0.0], None);
            let x = three([-s, c, 0.0], None);
            Value::Map(Arc::new(Map::from([(keys[0].clone(), origin), (keys[1].clone(), normal), (keys[2].clone(), x)])), plane)
        })
        .collect();
    let table = Value::array((0..n).map(|i| Value::array((0..n).map(|j| Value::num(((i * 7 + j * 13) % 256) as f64)).collect())).collect());
    let mut interp = Interp::new();
    let mut step = |name: &str, args: Vec<Value>| -> Value {
        let f = m.functions(name).expect(name);
        let t = std::time::Instant::now();
        // (The interpreter's own call: a points table returned to a host would be converted back for it.)
        let r = interp.call_set_in(&f, args).unwrap_or_else(|e| panic!("{name}: {}", e.message()));
        println!("{name}: {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        r
    };
    let t = std::time::Instant::now();
    step("check", vec![table.clone()]);
    let params = step("parameters", vec![Value::num(n as f64), Value::num(n as f64)]);
    // (The values passed stay held here: the frames don't hand them to the freeing thread mid-measure.)
    step("preconditionLoop", vec![params.clone()]);
    let planes = Value::array(planes);
    let pts = step("points", vec![table.clone(), planes.clone()]);
    println!("loops at {n}²: {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    assert_eq!(pts.as_array().map(|r| r.len()), Some(n));
}

/// What one step of a loop costs (ns per iteration over a million): the loop itself, arithmetic, a call, an append,
/// unit and vector arithmetic, a member read (NOSHAPE_FS_BENCH_N iterations, default 1M).
#[test]
#[ignore = "T2: timing, ~5 s"]
fn interpreter_step_costs() {
    let n: usize = std::env::var("NOSHAPE_FS_BENCH_N").ok().and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
    let steps = [
        ("loop", ""),
        ("number arithmetic", "x = i * 2 + 1;"),
        ("call", "x = id(i);"),
        ("call of five", "x = five(i, 0, 255, 0, 1);"),
        ("append", "a = append(a, i);"),
        ("length * number", "x = m * 2;"),
        ("length / length", "x = m / m;"),
        ("vector(x, y)", "x = vector(i, 1);"),
        ("vector * length", "x = n * m;"),
        ("length vector + length vector", "x = p + q;"),
        ("member", "x = pl.origin;"),
        ("index", "x = arr[2];"),
    ];
    let mut src = String::from("FeatureScript 3044;\nimport(path : \"onshape/std/common.fs\", version : \"3044.0\");\nfunction id(v) { return v; }\nfunction five(a is number, b is number, c is number, d is number, e is number) returns number { return a; }\n");
    for (k, (_, body)) in steps.iter().enumerate() {
        src += &format!("function s{k}(n is number) {{ var x; var a = []; const m = 2 * meter; const p = vector(1, 2, 3) * meter; const q = vector(3, 2, 1) * meter; const pl = plane(p, vector(0, 0, 1)); const arr = [1, 2, 3]; for (var i = 0; i < n; i += 1) {{ {body} }} return x; }}\n");
    }
    let m = compile_module(&unique_path(), &src, &NoImports).unwrap_or_else(|e| panic!("{}", e.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")));
    let mut interp = Interp::new();
    let mut base = 0.0;
    let only = std::env::var("NOSHAPE_FS_STEP").ok();
    for (k, (name, _)) in steps.iter().enumerate() {
        if k > 0 && only.as_deref().is_some_and(|o| o != *name) {
            continue;
        }
        let f = m.functions(&format!("s{k}")).expect("step");
        let t = std::time::Instant::now();
        interp.call_set(&f, vec![Value::num(n as f64)]).unwrap_or_else(|e| panic!("{name}: {}", e.message()));
        let ns = t.elapsed().as_secs_f64() * 1e9 / n as f64;
        if k == 0 {
            base = ns;
        }
        println!("{name}: {ns:.0} ns ({:.0} over the loop)", ns - base);
    }
}

/// An error's trace: each call with its caller's module (the host's empty one first), within a module and into std.
#[test]
fn a_trace_names_each_callers_module() {
    let path = unique_path();
    let src = "FeatureScript 3044;\nimport(path : \"onshape/std/common.fs\", version : \"3044.0\");\nfunction f() { return g(); }\nfunction g() { return abs(\"a\"); }\n";
    let m = compile_module(&path, src, &NoImports).unwrap_or_else(|e| panic!("{}", e.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")));
    let Err(e) = Interp::new().call_set(&m.functions("f").expect("f"), vec![]) else { panic!("abs of a string fails") };
    let trace: Vec<(&str, &str)> = e.trace.iter().map(|t| (&*t.function, &*t.module)).collect();
    assert_eq!(trace, [("f", ""), ("g", path.as_str()), ("abs", path.as_str())]);
    assert_eq!(&*e.module, "onshape/std/math.fs");
}

/// A value (compact Vectors held in it) stays 32 bytes: arrays of numbers grow by a third at most.
#[test]
fn a_value_is_32_bytes() {
    assert_eq!(std::mem::size_of::<Value>(), 32);
}

/// Compact lengths and Vectors (FS-COMPILED-INTERP stage 2) answer as the maps and arrays they stand for, whatever a
/// script does with them: the same output with them as with maps and arrays only.
#[test]
fn compact_values_answer_as_maps_and_arrays() {
    let bodies = [
        "const a = 2 * meter; return [a, a.value, a.unit, a == { 'value' : 2, 'unit' : meter.unit } as ValueWithUnits, a is map, a is ValueWithUnits, size(a), a['value'], keys(a)];",
        "const v = vector(1, 2, 3) * meter; return [v, v[0], v[2].value, size(v), v is array, v is Vector, v == [meter, 2 * meter, 3 * meter] as Vector, @size(v), v as array];",
        "var v = vector(1, 2) * inch; v[1] = 5 * inch; return [v, v[1] + v[0], -v, v * 2, v / 2, 2 * v];",
        "var q = 3 * meter; q.value = 4; return [q, q * q, q / q, q / (2 * second), 1 / q, sqrt(q * q)];",
        "var out = []; for (var x in vector(1, 2, 3) * meter) { out = append(out, x); } for (var k, x in 2 * meter) { out = append(out, k); } return out;",
        "const m = { (vector(1, 2) * meter) : 'a', (2 * meter) : 'b' }; return [m[vector(1, 2) * meter], m[[meter, 2 * meter] as Vector], m[2 * meter], m];",
        "const v = vector(3, 1, 2); var w = concatenateArrays(v, [4]); w = append(w, vector(1, 1)); return [w, v ~ '', toString(v), sort([vector(2, 1), vector(1, 2)], function(a, b) { return a[0] - b[0]; })];",
        "const p = vector(1, 2, 3) * meter; return [p + vector(1, 1, 1) * inch, cross(vector(1, 0, 0), vector(0, 1, 0)), dot(p, p), norm(p), normalize(vector(3, 4, 0)), squaredNorm(p)];",
        "const a = [vector(1, 2, 3) * meter, vector(4, 5, 6) * meter]; return [a, isLengthVector(a[0]), is3dLengthVector(a[1]), canBeControlPointMatrix([a]), a[1] - a[0], (a[1] - a[0]) / 3];",
        "return [vector(1, meter), vector(meter, inch), vector(1, 2, 3, 4), vector('a', 1), [vector(1, 2)] as Vector, (vector(1, 2) * meter) as Vector];",
        "const b = new box(vector(1, 2) * meter); b[] = b[] * 2; return [b[], b[][0], b[] is Vector];",
    ];
    for b in bodies {
        let src = format!("function f() {{ {b} }}");
        let run_mode = |compact: bool| -> String {
            let full = format!("FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\n{src}");
            let m = match compile_module(&unique_path(), &full, &NoImports) {
                Ok(m) => m,
                Err(e) => return format!("COMPILE: {}", e.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n")),
            };
            let Some(f) = m.functions("f") else { return "no f".into() };
            let mut i = Interp::new();
            i.compact = compact;
            match i.call_set(&f, vec![]) {
                Ok(v) => to_display(&v),
                Err(e) => format!("ERROR: {} at {}:{}", e.message(), e.module, e.pos),
            }
        };
        let (compact, plain) = (run_mode(true), run_mode(false));
        let message = |r: &str| r.split(" at test").next().unwrap_or_default().to_string();
        assert_eq!(message(&compact), message(&plain), "{b}");
        assert!(!compact.starts_with("COMPILE"), "{b}: {compact}");
    }
    // (They are compact inside: a length, a Vector of lengths, of numbers; maps and arrays without compact.)
    let inside = |expr: &str, compact: bool| -> Value {
        let full = format!("FeatureScript 3095;\nimport(path : \"onshape/std/common.fs\", version : \"3095.0\");\nfunction f() {{ return {expr}; }}");
        let m = compile_module(&unique_path(), &full, &NoImports).unwrap_or_else(|_| panic!("{expr}"));
        let mut i = Interp::new();
        i.compact = compact;
        i.call_set_in(&m.functions("f").expect("f"), vec![]).unwrap_or_else(|e| panic!("{expr}: {}", e.message()))
    };
    for expr in ["2 * meter", "vector(1, 2, 3) * meter", "vector(1, 2)", "vector(1, 2) * meter + vector(3, 4) * meter"] {
        assert!(inside(expr, true).is_compact(), "{expr} compact");
        assert!(!inside(expr, false).has_compact(), "{expr} plain");
    }
}

/// Live 2485: a "/" typed before `FeatureScript 3112;` gives two notices, the stray token's at 1:1 and the import's
/// version check at 2:1 (the version then counts as 0).
#[test]
fn a_stray_token_before_the_version_gives_two_notices() {
    let src = "/FeatureScript 3112;\nimport(path : \"onshape/std/common.fs\", version : \"3112.0\");\n";
    assert_eq!(
        notices(src),
        [
            "[1:1] extraneous input '/' expecting {<EOF>, 'FeatureScript', 'annotation', 'export', 'import', 'enum', 'type', 'function', 'predicate', 'operator', 'typeconvert', 'const', TOP_SEMI, ID}",
            "[2:1] Cannot import a module with FeatureScript version: 3112 from module with older version 0",
        ]
    );
}
