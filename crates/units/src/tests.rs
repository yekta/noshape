//! Expression language tests.

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::*;

fn scope() -> HashMap<String, Quantity> {
    let mut m = HashMap::new();
    m.insert("w".to_string(), Quantity::mm(40.0));
    m.insert("h".to_string(), Quantity::mm(10.0));
    m.insert("n".to_string(), Quantity::unitless(3.0));
    m.insert("a".to_string(), Quantity::degrees(30.0));
    m.insert("my_var2".to_string(), Quantity::unitless(2.0));
    m
}

fn ev(s: &str) -> Quantity {
    evaluate(s, &scope()).unwrap_or_else(|e| panic!("{s}: {e}"))
}

fn ev_err(s: &str) -> ExprError {
    evaluate(s, &scope()).expect_err(s)
}

fn assert_mm(s: &str, mm: f64) {
    let q = ev(s);
    assert_eq!(q.dim, Dim::LENGTH, "{s}");
    assert!(
        (q.value * 1000.0 - mm).abs() < 1e-9,
        "{s}: got {} mm, want {mm}",
        q.value * 1000.0
    );
}

fn assert_num(s: &str, v: f64) {
    let q = ev(s);
    assert_eq!(q.dim, Dim::UNITLESS, "{s}");
    assert!((q.value - v).abs() < 1e-9, "{s}: got {}, want {v}", q.value);
}

fn assert_deg(s: &str, deg: f64) {
    let q = ev(s);
    assert_eq!(q.dim, Dim::ANGLE, "{s}");
    assert!(
        (q.value.to_degrees() - deg).abs() < 1e-9,
        "{s}: got {}°",
        q.value.to_degrees()
    );
}

#[test]
fn numbers() {
    assert_num("42", 42.0);
    assert_num("1e-3", 0.001);
    assert_num(".5", 0.5);
    assert_num("2.5E2", 250.0);
    assert_num("  7  ", 7.0);
}

#[test]
fn units_with_and_without_space() {
    assert_mm("25mm", 25.0);
    assert_mm("25 mm", 25.0);
    assert_mm("2.5 cm", 25.0);
    assert_mm("0.025m", 25.0);
    assert_mm("1 in", 25.4);
    assert_mm("1\"", 25.4);
    assert_mm("1 ft", 304.8);
    assert_mm("2'", 609.6);
    assert_mm("1 yd", 914.4);
    assert_mm("500 um", 0.5);
    assert_mm("500 µm", 0.5);
    assert_mm("3 inches", 76.2);
    assert_mm("1 meter", 1000.0);
    assert_deg("30 deg", 30.0);
    assert_deg("30°", 30.0);
    assert_deg("30 degrees", 30.0);
    assert_deg("1 rad", 1.0f64.to_degrees());
}

#[test]
fn onshape_shorthands() {
    assert_mm("1/4 in", 6.35);
    assert_mm("1/4\"", 6.35);
    assert_mm("1 1/2 in", 38.1);
    assert_mm("1 ft 3 in", 381.0);
    assert_mm("5' 3\"", 1600.2);
    assert_mm("1 ft 1/2 in", 304.8 + 12.7);
    assert_mm("2 * 1 ft 3 in", 762.0);
    assert_mm("-1 ft 3 in", -381.0);
    // A plain division is still a division.
    assert_num("1/4", 0.25);
    assert_mm("#w / 2", 20.0);
}

#[test]
fn unit_powers() {
    let q = ev("4 mm^2");
    assert_eq!(q.dim, Dim::AREA);
    assert!((q.value - 4e-6).abs() < 1e-18);
    assert_eq!(ev("4 mm²").dim, Dim::AREA);
    assert_eq!(ev("8 mm³").dim, Dim::VOLUME);
    assert_eq!(ev("2 mm^-1").dim, Dim::new(-1, 0));
    // A parenthesised quantity raised to a power squares the number too.
    assert!((ev("(2 mm)^2").value - 4e-6).abs() < 1e-18);
    assert_mm("sqrt(4 mm^2)", 2.0);
    assert_mm("(27 mm^3) ^ (1/3)", 3.0);
}

#[test]
fn arithmetic_and_precedence() {
    assert_num("1 + 2 * 3", 7.0);
    assert_num("(1 + 2) * 3", 9.0);
    assert_num("2 ^ 3 ^ 2", 512.0);
    assert_num("-2 ^ 2", -4.0);
    assert_num("(-2) ^ 2", 4.0);
    assert_num("2 ^ -1", 0.5);
    assert_num("10 - 4 - 3", 3.0);
    assert_num("12 / 4 / 3", 1.0);
    assert_num("7 % 4", 3.0);
    assert_num("--3", 3.0);
    assert_num("+3", 3.0);
    assert_mm("#w + 5 mm", 45.0);
    assert_mm("2 * #n * 0 mm + 1 mm", 1.0);
    assert_mm("#w * #n - 1 in", 120.0 - 25.4);
    assert_mm("#w * mm / mm", 40.0);
    assert_mm("#n * mm", 3.0);
    let area = ev("#w * #h");
    assert_eq!(area.dim, Dim::AREA);
    assert_num("#w / #h", 4.0);
    assert_num("#my_var2 * 2", 4.0);
}

#[test]
fn constants() {
    assert_num("PI", PI);
    assert_num("pi * 2", 2.0 * PI);
    assert_num("2 * π", 2.0 * PI);
    assert_num("true + true", 2.0);
}

#[test]
fn trig_follows_onshape() {
    assert_num("sin(30 deg)", 0.5);
    assert_num("sin(#a)", 0.5);
    assert_num("cos(PI)", -1.0); // bare numbers are radians
    assert_num("tan(45 deg)", 1.0);
    assert_deg("asin(0.5)", 30.0);
    assert_deg("acos(0)", 90.0);
    assert_deg("atan(1)", 45.0);
    assert_deg("atan2(1 mm, 1 mm)", 45.0);
    assert_deg("atan2(1, -1)", 135.0);
    assert_mm("10 mm * sin(30 deg)", 5.0);
    assert!(matches!(
        ev_err("sin(1 mm)"),
        ExprError::Eval(EvalError::BadArgument { func: "sin", .. })
    ));
    assert!(matches!(
        ev_err("asin(2)"),
        ExprError::Eval(EvalError::Domain { func: "asin" })
    ));
    assert!(matches!(
        ev_err("asin(1 deg)"),
        ExprError::Eval(EvalError::BadArgument { .. })
    ));
    assert!(matches!(
        ev_err("atan2(1 mm, 1)"),
        ExprError::Eval(EvalError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        ev_err("tan(90 deg)"),
        ExprError::Eval(EvalError::Domain { func: "tan" })
    ));
}

#[test]
fn other_functions() {
    assert_mm("abs(-3 mm)", 3.0);
    assert_num("floor(2.7)", 2.0);
    assert_num("ceil(2.1)", 3.0);
    assert_num("round(2.5)", 3.0);
    assert_mm("round(12.3 mm, 0.5 mm)", 12.5);
    assert_mm("floor(1.3 in, 1 mm)", 33.0);
    assert_mm("ceil(#w / 3, 1 mm)", 14.0);
    assert_mm("min(3 mm, 1 in, #w)", 3.0);
    assert_mm("max(3 mm, 1 in, #w)", 40.0);
    assert_num("max(5)", 5.0);
    assert_num("exp(0)", 1.0);
    assert_num("ln(exp(2))", 2.0);
    assert_num("log(1)", 0.0);
    assert_num("log10(1000)", 3.0);
    assert_num("sqrt(16)", 4.0);
    assert!(matches!(
        ev_err("floor(2 mm)"),
        ExprError::Eval(EvalError::BadArgument { .. })
    ));
    assert!(matches!(
        ev_err("round(2 mm, 1)"),
        ExprError::Eval(EvalError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        ev_err("min(1 mm, 1)"),
        ExprError::Eval(EvalError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        ev_err("ln(0)"),
        ExprError::Eval(EvalError::Domain { .. })
    ));
    assert!(matches!(
        ev_err("ln(1 mm)"),
        ExprError::Eval(EvalError::BadArgument { .. })
    ));
    assert!(matches!(
        ev_err("sqrt(-1)"),
        ExprError::Eval(EvalError::Domain { .. })
    ));
    assert!(matches!(
        ev_err("sqrt(2 mm)"),
        ExprError::Eval(EvalError::FractionalUnits { .. })
    ));
    assert!(matches!(
        ev_err("exp(1000)"),
        ExprError::Eval(EvalError::NotFinite)
    ));
}

#[test]
fn comparisons_logic_ternary() {
    assert_num("1 < 2", 1.0);
    assert_num("2 <= 2", 1.0);
    assert_num("3 > 4", 0.0);
    assert_num("1 in >= 25.4 mm", 1.0);
    assert_num("1 in == 25.4 mm", 1.0);
    assert_num("1 in != 25.4 mm", 0.0);
    assert_num("#w > 0", 1.0); // zero is comparable with anything
    assert_mm("#w > 30 mm ? 5 mm : 1 mm", 5.0);
    assert_mm("#w > 50 mm ? 5 mm : 1 mm", 1.0);
    assert_mm("#n == 1 ? 1 mm : #n == 3 ? 3 mm : 0 mm", 3.0);
    assert_num("true && false", 0.0);
    assert_num("true || false", 1.0);
    assert_num("1 < 2 and 2 < 3", 1.0);
    assert_num("1 > 2 or 2 > 3", 0.0);
    assert_num("not 0", 1.0);
    assert_num("!(1 < 2)", 0.0);
    assert_num("1 + 1 == 2 && 2 * 2 == 4", 1.0);
    // Short-circuiting skips evaluation errors in untaken branches.
    assert_num("false && #undefined > 0", 0.0);
    assert_num("true || #undefined > 0", 1.0);
    assert_mm("true ? 1 mm : #undefined", 1.0);
    assert!(matches!(
        ev_err("1 mm < 1 deg"),
        ExprError::Eval(EvalError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        ev_err("1 mm == 1"),
        ExprError::Eval(EvalError::DimensionMismatch { .. })
    ));
}

#[test]
fn dimension_errors() {
    match ev_err("1 mm + 1 deg") {
        ExprError::Eval(EvalError::DimensionMismatch { op, left, right }) => {
            assert_eq!(op, "add");
            assert_eq!(left, Dim::LENGTH);
            assert_eq!(right, Dim::ANGLE);
        }
        e => panic!("{e:?}"),
    }
    assert!(
        ev_err("10 + 5 mm")
            .to_string()
            .contains("cannot add unitless and length")
    );
    assert!(matches!(
        ev_err("2 ^ (1 mm)"),
        ExprError::Eval(EvalError::ExponentNotUnitless(_))
    ));
    assert!(matches!(
        ev_err("1 mm / 0"),
        ExprError::Eval(EvalError::DivisionByZero)
    ));
    assert!(matches!(
        ev_err("1 % 0"),
        ExprError::Eval(EvalError::DivisionByZero)
    ));
    assert!(matches!(
        ev_err("mm ^ 100"),
        ExprError::Eval(EvalError::FractionalUnits { .. })
    ));
    assert_eq!(
        ev_err("#missing + 1"),
        ExprError::Eval(EvalError::UndefinedVariable("missing".into()))
    );
    // length*angle is representable (but not accepted by plain fields)
    assert_eq!(ev("2 mm * 3 deg").dim, Dim::new(1, 1));
}

#[test]
fn evaluate_as_applies_default_units() {
    let mm = UnitPrefs::default();
    let inch = UnitPrefs::imperial();
    let s = scope();
    let q = evaluate_as("10", Dim::LENGTH, &mm, &s).unwrap();
    assert!((q.value - 0.01).abs() < 1e-15);
    let q = evaluate_as("10", Dim::LENGTH, &inch, &s).unwrap();
    assert!((q.value - 0.254).abs() < 1e-15);
    let q = evaluate_as("10 mm", Dim::LENGTH, &inch, &s).unwrap();
    assert!((q.value - 0.01).abs() < 1e-15);
    let q = evaluate_as("45", Dim::ANGLE, &mm, &s).unwrap();
    assert!((q.value - PI / 4.0).abs() < 1e-15);
    let q = evaluate_as("100", Dim::AREA, &mm, &s).unwrap();
    assert!((q.value - 1e-4).abs() < 1e-18);
    let q = evaluate_as("#n * 2", Dim::LENGTH, &mm, &s).unwrap();
    assert!((q.value - 0.006).abs() < 1e-15);
    let q = evaluate_as("3", Dim::UNITLESS, &mm, &s).unwrap();
    assert_eq!(q, Quantity::unitless(3.0));
    assert_eq!(
        evaluate_as("10 deg", Dim::LENGTH, &mm, &s),
        Err(ExprError::Eval(EvalError::WrongDimension {
            expected: Dim::LENGTH,
            found: Dim::ANGLE
        }))
    );
    assert!(evaluate_as("10 mm", Dim::UNITLESS, &mm, &s).is_err());
    assert!(evaluate_as("#w * #h", Dim::LENGTH, &mm, &s).is_err());
    assert!(matches!(
        evaluate_as("1 +", Dim::LENGTH, &mm, &s),
        Err(ExprError::Parse(_))
    ));
    let e = parse("#w / 2").unwrap();
    assert!((e.eval_as(Dim::LENGTH, &mm, &s).unwrap().value - 0.02).abs() < 1e-15);
}

#[test]
fn parse_errors_have_spans() {
    let e = parse("").unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::Empty);
    let e = parse("   ").unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::Empty);

    let e = parse("1 +").unwrap_err();
    assert!(matches!(e.kind, ParseErrorKind::UnexpectedEnd { .. }));
    assert_eq!(e.span, 3..3);

    let e = parse("2 * (3 + 4").unwrap_err();
    assert!(matches!(e.kind, ParseErrorKind::UnexpectedEnd { .. }));

    let e = parse("10 furlongs").unwrap_err();
    assert!(matches!(e.kind, ParseErrorKind::UnexpectedToken { .. }));
    assert_eq!(e.span, 3..11);

    let e = parse("width * 2").unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnknownIdentifier("width".into()));
    assert_eq!(e.span, 0..5);
    assert!(e.to_string().contains("#width"));

    let e = parse("1 + foo(2)").unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnknownFunction("foo".into()));
    assert_eq!(e.span, 4..7);

    let e = parse("atan2(1)").unwrap_err();
    assert!(matches!(
        e.kind,
        ParseErrorKind::WrongArgCount { found: 1, .. }
    ));
    assert_eq!(e.span, 0..8);
    assert!(parse("sin(1, 2)").is_err());
    assert!(parse("min()").is_err());
    assert!(parse("round(1, 2, 3)").is_err());

    let e = parse("sin 30").unwrap_err();
    assert!(matches!(e.kind, ParseErrorKind::UnexpectedToken { .. }));

    let e = parse("1 ? 2").unwrap_err();
    assert!(matches!(e.kind, ParseErrorKind::UnexpectedEnd { .. }));

    let e = parse("3 4").unwrap_err();
    assert_eq!(e.span, 2..3);

    let e = parse("#").unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::InvalidVariable);

    let e = parse("2 mm $").unwrap_err();
    assert_eq!(e.kind, ParseErrorKind::UnexpectedChar('$'));
    assert_eq!(e.span, 5..6);

    // Spans are byte offsets, also after multi-byte characters.
    let e = parse("30° @").unwrap_err();
    assert_eq!(e.span, 5..6);
    assert!(parse(")").is_err());
    assert!(parse("1 mm mm").is_err());
}

#[test]
fn variables_are_collected_in_order() {
    let e = parse("#b + #a * #b - max(#c, 1)").unwrap();
    assert_eq!(e.variables(), vec!["b", "a", "c"]);
    assert!(!e.is_constant());
    assert!(parse("1 mm + PI").unwrap().is_constant());
    assert_eq!(
        parse("#x ? #y : #z").unwrap().variables(),
        vec!["x", "y", "z"]
    );
}

#[test]
fn display_is_canonical() {
    let cases = [
        ("25mm", "25 mm"),
        ("1/4\"", "(1 / 4) in"),
        ("1 1/2 in", "(1 + 1 / 2) in"),
        ("5' 3\"", "5 ft + 3 in"),
        ("30°", "30 deg"),
        ("4 mm²", "4 mm^2"),
        ("2 mm^-1", "2 mm^-1"),
        ("(2 mm)^2", "(2 mm) ^ 2"),
        ("1+2*3", "1 + 2 * 3"),
        ("(1+2)*3", "(1 + 2) * 3"),
        ("1-(2-3)", "1 - (2 - 3)"),
        ("(1-2)-3", "1 - 2 - 3"),
        ("2^3^2", "2 ^ 3 ^ 2"),
        ("(2^3)^2", "(2 ^ 3) ^ 2"),
        ("-2^2", "-2 ^ 2"),
        ("(-2)^2", "(-2) ^ 2"),
        ("2^-1", "2 ^ -1"),
        ("-(1+2)", "-(1 + 2)"),
        ("#w/2", "#w / 2"),
        ("1/(4 in)", "1 / (4 in)"),
        ("pi", "PI"),
        ("π", "PI"),
        ("#a>#b", "#a > #b"),
        ("#a > 1 and #b < 2 or not #c", "#a > 1 && #b < 2 || !#c"),
        ("#a ? 1 : #b ? 2 : 3", "#a ? 1 : #b ? 2 : 3"),
        ("(#a ? 1 : 2) ? 3 : 4", "(#a ? 1 : 2) ? 3 : 4"),
        ("#a ? (#b ? 1 : 2) : 3", "#a ? #b ? 1 : 2 : 3"),
        ("1 + (#a ? 1 : 2)", "1 + (#a ? 1 : 2)"),
        ("max( 1 ,2,  3 )", "max(1, 2, 3)"),
        ("log(2)", "ln(2)"),
        ("2 * mm", "2 * mm"),
        ("(#w + 1 mm) mm", "(#w + 1 mm) mm"),
        ("1e-3", "0.001"),
        ("10 inches", "10 in"),
        ("7 % 2", "7 % 2"),
        ("!!1", "!!1"),
        ("--1", "--1"),
    ];
    for (input, canonical) in cases {
        let e = parse(input).unwrap_or_else(|err| panic!("{input}: {err}"));
        assert_eq!(e.to_string(), canonical, "input {input}");
    }
}

#[test]
fn display_round_trips() {
    let inputs = [
        "25 mm",
        "1/4 in",
        "1 1/2 in",
        "1 ft 3 in + 2 * 5' 3\"",
        "#w / 2 + 1/4 in",
        "sin(30 deg) * 10 mm",
        "-#a ^ 2 ^ -3",
        "(1 / (4 in)) * 2 in",
        "#x > 2 mm ? max(#x, 1 in) : (#y + 1) mm",
        "!(#a && #b) || #c != 2",
        "1 - (2 + 3) - (4 - 5)",
        "2 / (3 / 4) / 5",
        "1 / 4 / 2 mm",
        "3 * 1 / (4 in)",
        "(2 mm) ^ 2 + 4 mm^2",
        "round(#w, 0.5 mm) % 3 mm",
        "atan2(1 mm, 2 mm) + 30°",
        "1 < 2 == 3 > 4",
        "(1 < 2) < 3",
        "1 < (2 < 3)",
        "#a ? #b : #c ? #d : #e",
        "(#a ? #b : #c) ? #d : #e",
        "1e21 + .5",
    ];
    for input in inputs {
        let e = parse(input).unwrap();
        let printed = e.to_string();
        let reparsed = parse(&printed).unwrap_or_else(|err| panic!("{input} -> {printed}: {err}"));
        assert_eq!(reparsed, e, "{input} -> {printed}");
        assert_eq!(reparsed.to_string(), printed);
    }
}

#[test]
fn display_of_constructed_exprs_round_trips_by_value() {
    // Expressions built programmatically (not from the parser).
    let e = Expr::Binary(
        BinaryOp::Pow,
        Box::new(Expr::Number(-2.0)),
        Box::new(Expr::Number(2.0)),
    );
    let s = e.to_string();
    assert_eq!(s, "(-2) ^ 2");
    assert_eq!(
        evaluate(&s, &EmptyScope).unwrap(),
        e.eval(&EmptyScope).unwrap()
    );
    let e = Expr::WithUnit {
        expr: Box::new(Expr::Number(-3.0)),
        unit: Unit::Length(LengthUnit::Inch),
        power: 1,
    };
    assert_eq!(e.to_string(), "(-3) in");
    assert_eq!(
        evaluate(&e.to_string(), &EmptyScope).unwrap(),
        e.eval(&EmptyScope).unwrap()
    );
}

#[test]
fn scopes() {
    let f = FnScope(|n: &str| (n == "x").then(|| Quantity::mm(1.0)));
    assert_eq!(evaluate("#x", &f).unwrap(), Quantity::mm(1.0));
    let list: &[(&str, Quantity)] = &[("x", Quantity::mm(1.0)), ("x", Quantity::mm(2.0))];
    assert_eq!(evaluate("#x", &list).unwrap(), Quantity::mm(2.0));
    let inner = std::collections::BTreeMap::from([("y".to_string(), Quantity::unitless(5.0))]);
    let layered = LayeredScope {
        inner: &inner,
        outer: &f,
    };
    assert_mm("0 mm", 0.0);
    assert!((evaluate("#x * #y", &layered).unwrap().value - 0.005).abs() < 1e-15);
}
