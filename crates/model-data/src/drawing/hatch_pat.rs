//! Hatch pattern definitions and the line generator (the `.pat` model
//! Onshape's patterns come from: live drawing/468–472, the pattern icons
//! are `/os/icons/hatches/<group>/<name>.png`). A pattern is a set of line
//! families; each family is an angle, an origin, a delta (shift along the
//! line, offset to the next line) and a dash list (positive dash, negative
//! gap, 0 a dot). ANSI and General patterns are in inches, ISO ones in mm.
//! Live ANSI31 at "Auto" draws 3.175 mm (0.125 in) apart.

/// One line family: angle (deg), origin, delta, dashes.
pub struct Family {
    pub angle: f64,
    pub origin: [f64; 2],
    pub delta: [f64; 2],
    pub dashes: &'static [f64],
}

const fn f(angle: f64, x: f64, y: f64, dx: f64, dy: f64, dashes: &'static [f64]) -> Family {
    Family { angle, origin: [x, y], delta: [dx, dy], dashes }
}

/// A named pattern: its families and their unit (mm per pattern unit).
pub struct Pattern {
    pub name: &'static str,
    pub unit: f64,
    pub families: &'static [Family],
}

const IN: f64 = 25.4;
/// The architectural (AR-) patterns are drawn at 1/48 of their size.
const AR: f64 = 25.4 / 48.0;

const ANSI31: &[Family] = &[f(45.0, 0.0, 0.0, 0.0, 0.125, &[])];
const ANSI32: &[Family] = &[f(45.0, 0.0, 0.0, 0.0, 0.375, &[]), f(45.0, 0.176776695, 0.0, 0.0, 0.375, &[])];
const ANSI33: &[Family] = &[f(45.0, 0.0, 0.0, 0.0, 0.25, &[]), f(45.0, 0.176776695, 0.0, 0.0, 0.25, &[0.125, -0.0625])];
const ANSI34: &[Family] = &[
    f(45.0, 0.0, 0.0, 0.0, 0.75, &[]),
    f(45.0, 0.176776695, 0.0, 0.0, 0.75, &[]),
    f(45.0, 0.353553391, 0.0, 0.0, 0.75, &[]),
    f(45.0, 0.530330086, 0.0, 0.0, 0.75, &[]),
];
const ANSI35: &[Family] = &[f(45.0, 0.0, 0.0, 0.0, 0.25, &[]), f(45.0, 0.176776695, 0.0, 0.0, 0.25, &[0.3125, -0.0625, 0.0, -0.0625])];
const ANSI36: &[Family] = &[f(45.0, 0.0, 0.0, 0.21875, 0.125, &[0.3125, -0.0625, 0.0, -0.0625])];
const ANSI37: &[Family] = &[f(45.0, 0.0, 0.0, 0.0, 0.125, &[]), f(135.0, 0.0, 0.0, 0.0, 0.125, &[])];
const ANSI38: &[Family] = &[f(45.0, 0.0, 0.0, 0.0, 0.125, &[]), f(135.0, 0.0, 0.0, 0.25, 0.125, &[0.3125, -0.1875])];

/// ANSI patterns (index 8: the drawing's default, ANSI31).
pub const ANSI: [Pattern; 9] = [
    Pattern { name: "ANSI31 general", unit: IN, families: ANSI31 },
    Pattern { name: "ANSI32 steel", unit: IN, families: ANSI32 },
    Pattern { name: "ANSI33 bronze/ brass/ copper", unit: IN, families: ANSI33 },
    Pattern { name: "ANSI34 rubber/ plastic", unit: IN, families: ANSI34 },
    Pattern { name: "ANSI35", unit: IN, families: ANSI35 },
    Pattern { name: "ANSI36", unit: IN, families: ANSI36 },
    Pattern { name: "ANSI37 lead/ alloys", unit: IN, families: ANSI37 },
    Pattern { name: "ANSI38 aluminium", unit: IN, families: ANSI38 },
    Pattern { name: "ANSI31 general (drawing)", unit: IN, families: ANSI31 },
];

macro_rules! iso {
    ($name:literal, $($d:expr),*) => {
        Pattern { name: $name, unit: 1.0, families: &[f(0.0, 0.0, 0.0, 0.0, 5.0, &[$($d),*])] }
    };
}

/// ISO patterns, in the order older documents stored them (02, 03, 04, 07
/// first); [`display_order`] lists them by name.
pub const ISO: [Pattern; 14] = [
    iso!("ISO02W100", 12.0, -3.0),
    iso!("ISO03W100", 12.0, -18.0),
    iso!("ISO04W100", 24.0, -3.0, 0.5, -3.0),
    iso!("ISO07W100", 0.5, -3.0),
    iso!("ISO05W100", 24.0, -3.0, 0.5, -3.0, 0.5, -3.0),
    iso!("ISO06W100", 24.0, -3.0, 0.5, -3.0, 0.5, -3.0, 0.5, -3.0),
    iso!("ISO08W100", 24.0, -3.0, 6.0, -3.0),
    iso!("ISO09W100", 24.0, -3.0, 6.0, -3.0, 6.0, -3.0),
    iso!("ISO10W100", 12.0, -3.0, 0.5, -3.0),
    iso!("ISO11W100", 12.0, -3.0, 12.0, -3.0, 0.5, -3.0),
    iso!("ISO12W100", 12.0, -3.0, 0.5, -3.0, 0.5, -3.0),
    iso!("ISO13W100", 12.0, -3.0, 12.0, -3.0, 0.5, -3.0, 0.5, -3.0),
    iso!("ISO14W100", 12.0, -3.0, 0.5, -3.0, 0.5, -3.0, 0.5, -3.0),
    iso!("ISO15W100", 12.0, -3.0, 12.0, -3.0, 0.5, -3.0, 0.5, -3.0, 0.5, -3.0),
];

const PARQUET: &[Family] = &[
    f(90.0, 0.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(90.0, 2.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(90.0, 4.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(90.0, 6.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(90.0, 8.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(90.0, 10.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(90.0, 12.0, 0.0, 12.0, 12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 12.0, 12.0, -12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 14.0, 12.0, -12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 16.0, 12.0, -12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 18.0, 12.0, -12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 20.0, 12.0, -12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 22.0, 12.0, -12.0, &[12.0, -12.0]),
    f(0.0, 0.0, 24.0, 12.0, -12.0, &[12.0, -12.0]),
];

/// General patterns, in the order older documents stored them (Solid,
/// lines, cross, dots first); [`display_order`] lists Solid then by name.
pub const GENERAL: [Pattern; 31] = [
    Pattern { name: "Solid", unit: IN, families: &[] },
    Pattern { name: "Parallel lines", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.125, &[])] },
    Pattern { name: "Netting", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.125, &[]), f(90.0, 0.0, 0.0, 0.0, 0.125, &[])] },
    Pattern { name: "Dots", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.03125, 0.0625, &[0.0, -0.0625])] },
    Pattern { name: "AEC 8 x 16 Block elev", unit: AR, families: &[f(0.0, 0.0, 0.0, 0.0, 8.0, &[]), f(90.0, 0.0, 0.0, 8.0, 8.0, &[8.0, -8.0])] },
    Pattern { name: "AEC 8 x 8 Block elev", unit: AR, families: &[f(0.0, 0.0, 0.0, 0.0, 8.0, &[]), f(90.0, 0.0, 0.0, 8.0, 4.0, &[8.0, -8.0])] },
    Pattern { name: "AEC Herringbone", unit: AR, families: &[f(45.0, 0.0, 0.0, 4.0, 4.0, &[12.0, -4.0]), f(135.0, 2.828427125, 2.828427125, 4.0, -4.0, &[12.0, -4.0])] },
    Pattern { name: "AEC Parquet", unit: AR, families: PARQUET },
    Pattern { name: "AEC Roof shingle", unit: AR, families: &[f(0.0, 0.0, 0.0, 0.0, 6.0, &[]), f(90.0, 0.0, 0.0, 6.0, 5.0, &[6.0, -6.0])] },
    Pattern { name: "AEC Roof wood shake", unit: AR, families: &[f(0.0, 0.0, 0.0, 0.0, 6.0, &[]), f(90.0, 0.0, 0.0, 6.0, 3.5, &[6.0, -6.0]), f(90.0, 1.5, 0.0, 6.0, 7.0, &[2.5, -9.5])] },
    Pattern { name: "Aluminium", unit: IN, families: ANSI38 },
    Pattern { name: "Concrete", unit: IN, families: &[f(50.0, 0.0, 0.0, 0.104, 0.22, &[0.075, -0.825]), f(355.0, 0.0, 0.0, -0.1, 0.25, &[0.06, -0.73]), f(100.45, 0.059, -0.005, 0.12, 0.21, &[0.064, -0.83]), f(46.17, 0.0, 0.2, 0.15, 0.31, &[0.0, -0.6]), f(96.17, 0.0, 0.0, 0.17, 0.17, &[0.0, -0.4]), f(23.0, 0.0, 0.0, 0.12, 0.27, &[0.0, -0.5])] },
    Pattern { name: "Copper", unit: IN, families: ANSI33 },
    Pattern { name: "Dash lines", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.125, 0.125, &[0.125, -0.125])] },
    Pattern { name: "Electric", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.25, &[]), f(0.0, 0.0, 0.125, 0.0, 0.25, &[0.125, -0.0625, 0.0, -0.0625])] },
    Pattern { name: "Glass", unit: IN, families: &[f(45.0, 0.0, 0.0, 0.125, 0.25, &[0.25, -0.25]), f(45.0, 0.04, -0.04, 0.125, 0.25, &[0.125, -0.375])] },
    Pattern { name: "Grated area", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.03125, &[]), f(90.0, 0.0, 0.0, 0.0, 0.125, &[])] },
    Pattern { name: "Honeycomb", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.1875, 0.108253175, &[0.125, -0.25]), f(120.0, 0.0, 0.0, 0.1875, 0.108253175, &[0.125, -0.25]), f(60.0, 0.125, 0.0, 0.1875, 0.108253175, &[0.125, -0.25])] },
    Pattern { name: "Houndstooth", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.25, 0.0625, &[1.0, -0.5]), f(90.0, 0.0, 0.0, -0.25, 0.0625, &[1.0, -0.5])] },
    Pattern { name: "Insulation", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.375, &[]), f(0.0, 0.0, 0.125, 0.0, 0.375, &[0.125, -0.125]), f(0.0, 0.0, 0.25, 0.0, 0.375, &[0.125, -0.125])] },
    Pattern { name: "Iron", unit: IN, families: ANSI31 },
    Pattern { name: "Mud/ Sand", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.5, 0.25, &[0.25, -0.25, 0.0, -0.25, 0.0, -0.25])] },
    Pattern { name: "Plastic", unit: IN, families: ANSI34 },
    Pattern { name: "Squares", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.125, &[0.125, -0.125]), f(90.0, 0.0, 0.0, 0.0, 0.125, &[0.125, -0.125])] },
    Pattern { name: "Stars", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.0, 0.216506351, &[0.125, -0.125]), f(60.0, 0.0, 0.0, 0.1875, 0.108253175, &[0.125, -0.125]), f(120.0, 0.0625, 0.108253175, 0.1875, 0.108253175, &[0.125, -0.125])] },
    Pattern { name: "Steel", unit: IN, families: ANSI32 },
    Pattern { name: "Titanium", unit: IN, families: ANSI35 },
    Pattern { name: "Triangles", unit: IN, families: &[f(60.0, 0.0, 0.0, 0.1875, 0.324759526, &[0.1875, -0.1875]), f(120.0, 0.0, 0.0, 0.1875, 0.324759526, &[0.1875, -0.1875]), f(0.0, -0.09375, 0.162379763, 0.1875, 0.324759526, &[0.1875, -0.1875])] },
    Pattern { name: "Wood", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.3, 0.09, &[0.5, -0.1, 0.25, -0.05]), f(0.0, 0.15, 0.045, 0.3, 0.09, &[0.2, -0.4])] },
    Pattern { name: "Zigzag", unit: IN, families: &[f(0.0, 0.0, 0.0, 0.125, 0.125, &[0.125, -0.125]), f(90.0, 0.125, 0.0, 0.125, 0.125, &[0.125, -0.125])] },
    Pattern { name: "Zinc", unit: IN, families: ANSI37 },
];

/// The pattern groups (dialog tabs ANSI | ISO | General).
pub const GROUPS: [(&str, &[Pattern]); 3] = [("ANSI", &ANSI), ("ISO", &ISO), ("General", &GENERAL)];

/// The index of pattern `name` (case-insensitive) in group `group`.
pub fn find(group: usize, name: &str) -> Option<usize> {
    GROUPS.get(group)?.1.iter().position(|p| p.name.eq_ignore_ascii_case(name.trim()))
}

pub fn pattern(group: usize, index: usize) -> Option<&'static Pattern> {
    GROUPS.get(group)?.1.get(index)
}

/// Pattern indices in the order the dialog lists them: ANSI as live
/// (ANSI31 general, ANSI31 general (drawing), ANSI32 …), ISO by name,
/// General Solid then by name.
pub fn display_order(group: usize) -> Vec<usize> {
    match group {
        0 => vec![0, 8, 1, 2, 3, 4, 5, 6, 7],
        _ => {
            let Some((_, list)) = GROUPS.get(group) else { return vec![] };
            let mut v: Vec<usize> = (0..list.len()).collect();
            v.sort_by_key(|&i| (list[i].name != "Solid", list[i].name.to_ascii_lowercase()));
            v
        }
    }
}

/// A new hatch's pattern in a group (ANSI: "ANSI31 general (drawing)").
pub fn default_pattern(group: usize) -> usize {
    if group == 0 { 8 } else { display_order(group).first().copied().unwrap_or(0) }
}

/// The angle (deg) the pattern's first family is drawn at: the dialog's
/// "Auto" angle.
pub fn base_angle(group: usize, index: usize) -> f64 {
    pattern(group, index).and_then(|p| p.families.first()).map(|f| f.angle).unwrap_or(0.0)
}

/// The pattern's lines inside `boundary` minus `holes` (even-odd), scaled by
/// `scale` and turned by `rotate` degrees about the sheet origin.
pub fn lines(p: &Pattern, boundary: &[[f64; 2]], holes: &[Vec<[f64; 2]>], scale: f64, rotate: f64) -> Vec<[[f64; 2]; 2]> {
    let mut out = Vec::new();
    if boundary.len() < 3 {
        return out;
    }
    let ring = |b: &[[f64; 2]]| (0..b.len()).map(|i| [b[i], b[(i + 1) % b.len()]]).collect::<Vec<_>>();
    let mut segs = ring(boundary);
    for h in holes.iter().filter(|h| h.len() >= 3) {
        segs.extend(ring(h));
    }
    let k = p.unit * scale.max(0.01);
    let r = rotate.to_radians();
    let turn = |v: [f64; 2]| [v[0] * r.cos() - v[1] * r.sin(), v[0] * r.sin() + v[1] * r.cos()];
    for fam in p.families {
        let a = fam.angle.to_radians() + r;
        let u = [a.cos(), a.sin()];
        let n = [-u[1], u[0]];
        let o = turn([fam.origin[0] * k, fam.origin[1] * k]);
        // Successive lines: shift `dx` along, `dy` across (the family's own frame).
        let (dx, dy) = (fam.delta[0] * k, fam.delta[1] * k);
        if dy.abs() < 1e-9 {
            continue;
        }
        let dash: Vec<f64> = fam.dashes.iter().map(|d| d * k).collect();
        let period: f64 = dash.iter().map(|d| d.abs()).sum();
        let across = |q: [f64; 2]| (q[0] - o[0]) * n[0] + (q[1] - o[1]) * n[1];
        let along = |q: [f64; 2]| (q[0] - o[0]) * u[0] + (q[1] - o[1]) * u[1];
        let (lo, hi) = segs.iter().flatten().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), q| (l.min(across(*q)), h.max(across(*q))));
        let (i0, i1) = ((lo / dy).min(hi / dy).floor() as i64, (lo / dy).max(hi / dy).ceil() as i64);
        if i1 - i0 > 20000 {
            continue;
        }
        let mut ts: Vec<f64> = Vec::new();
        for i in i0..=i1 {
            let w = i as f64 * dy;
            // The line's own start (dash phase 0) along `u`.
            let t0 = i as f64 * dx;
            ts.clear();
            for s in &segs {
                let (wa, wb) = (across(s[0]), across(s[1]));
                if (wa <= w) != (wb <= w) {
                    let fr = (w - wa) / (wb - wa);
                    ts.push(along(s[0]) + (along(s[1]) - along(s[0])) * fr);
                }
            }
            ts.sort_by(f64::total_cmp);
            let at = |t: f64| [o[0] + u[0] * t + n[0] * w, o[1] + u[1] * t + n[1] * w];
            for pair in ts.as_chunks::<2>().0 {
                let (ta, tb) = (pair[0], pair[1]);
                if period < 1e-9 {
                    out.push([at(ta), at(tb)]);
                    continue;
                }
                // Walk the dash list from the period containing `ta`.
                let mut t = t0 + ((ta - t0) / period).floor() * period;
                let mut guard = 0;
                'walk: while t < tb && guard < 100_000 {
                    for d in &dash {
                        let t1 = t + d.abs();
                        if *d >= 0.0 {
                            // A dot: a 0.2 mm stroke.
                            let e = if *d == 0.0 { t + 0.2 } else { t1 };
                            let (s0, s1) = (t.max(ta), e.min(tb));
                            if s1 > s0 + 1e-9 {
                                out.push([at(s0), at(s1)]);
                            }
                        }
                        t = t1.max(t + 1e-9);
                        guard += 1;
                        if t >= tb {
                            break 'walk;
                        }
                    }
                }
            }
        }
    }
    out
}

/// The dialog's pattern icon (live `/os/icons/hatches/…`): a 20 × 14
/// box outlined in black with the pattern (Solid: filled grey).
pub fn icon_svg(group: usize, p: &Pattern) -> String {
    let (w, h) = (20.0, 14.0);
    let mut body = String::new();
    if p.families.is_empty() {
        body.push_str(r##"<rect x="0.5" y="0.5" width="19" height="13" fill="#5a5a5a"/>"##);
    } else {
        let scale = if group == 1 { 0.6 } else { 1.0 };
        let mut d = String::new();
        for l in lines(p, &[[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]], &[], scale, 0.0) {
            d.push_str(&format!("M{:.2} {:.2}L{:.2} {:.2}", l[0][0], h - l[0][1], l[1][0], h - l[1][1]));
        }
        body.push_str(&format!(r##"<path d="{d}" stroke="#000" stroke-width="0.7" stroke-linecap="round"/>"##));
    }
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="14" viewBox="0 0 20 14" fill="none">{body}<rect x="0.5" y="0.5" width="19" height="13" stroke="#000" stroke-width="1"/></svg>"##)
}

/// The asset name of [`icon_svg`]: `hatch-<group>-<index>`.
pub fn icon_name(group: usize, index: usize) -> String {
    format!("hatch-{group}-{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_fill_and_dash_inside_the_profile() {
        let sq = vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0], [0.0, 50.0]];
        let hole = vec![[20.0, 20.0], [30.0, 20.0], [30.0, 30.0], [20.0, 30.0]];
        let inside = |q: [f64; 2]| (-1e-6..=50.0 + 1e-6).contains(&q[0]) && (-1e-6..=50.0 + 1e-6).contains(&q[1]);
        let in_hole = |q: [f64; 2]| q[0] > 20.0 + 1e-6 && q[0] < 30.0 - 1e-6 && q[1] > 20.0 + 1e-6 && q[1] < 30.0 - 1e-6;
        for (g, list) in GROUPS.iter().enumerate() {
            for (i, p) in list.1.iter().enumerate() {
                let ls = lines(p, &sq, std::slice::from_ref(&hole), 1.0, 0.0);
                assert_eq!(ls.is_empty(), p.families.is_empty(), "{}", p.name);
                for l in &ls {
                    assert!(inside(l[0]) && inside(l[1]), "{} {:?}", p.name, l);
                    let mid = [(l[0][0] + l[1][0]) / 2.0, (l[0][1] + l[1][1]) / 2.0];
                    assert!(!in_hole(mid), "{} {:?}", p.name, l);
                }
                assert!(pattern(g, i).is_some());
            }
            let mut order = display_order(g);
            order.sort();
            assert_eq!(order, (0..list.1.len()).collect::<Vec<_>>());
        }
        // ANSI31: 0.125 in apart at 45°.
        let a31 = lines(&ANSI[0], &sq, &[], 1.0, 0.0);
        assert!(a31.iter().all(|l| ((l[1][1] - l[0][1]) - (l[1][0] - l[0][0])).abs() < 1e-6));
        assert!((a31.len() as f64 - 50.0 * 2f64.sqrt() / 3.175).abs() <= 1.5, "{}", a31.len());
        // ISO02W100: 12 mm dashes, 3 mm gaps, 5 mm apart.
        let iso = lines(&ISO[0], &sq, &[], 1.0, 0.0);
        assert!(iso.iter().all(|l| (l[1][0] - l[0][0]) <= 12.0 + 1e-6 && (l[1][1] - l[0][1]).abs() < 1e-9));
        assert_eq!(display_order(1)[3], 4);
        assert_eq!(GENERAL[display_order(2)[1]].name, "AEC 8 x 16 Block elev");
        assert_eq!(base_angle(0, default_pattern(0)), 45.0);
    }

    /// Regenerates the dialog's pattern icons:
    /// `cargo test -p noshape-model hatch_icons -- --ignored`.
    #[test]
    #[ignore]
    fn hatch_icons() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/assets/icons");
        for (g, list) in GROUPS.iter().enumerate() {
            for (i, p) in list.1.iter().enumerate() {
                std::fs::write(dir.join(format!("{}.svg", icon_name(g, i))), icon_svg(g, p)).unwrap();
            }
        }
    }
}
