//! An Assembly's URDF as live writes it (behaviours "URDF single link (11-20)", "URDF joint types and limits (11-20)"):
//! a link per part (the fixed ones' `root`), a joint per mate at its connector (a mate of several degrees of freedom a
//! chain of them through empty links), each explicit mate connector a 1e-9 kg link; with "Export rigid subassemblies as
//! a single link", a rigid subassembly or fastened parts one link with a visual per part and the connectors folded into
//! their parts.

use glam::{DMat3, DMat4, DQuat, DVec3};
use noshape_model::assembly::MateType;

/// What the URDF takes from the solved assembly beside its placed parts (no geometry: cheap on the UI thread).
#[derive(Clone, Debug, Default)]
pub struct Kinematics {
    /// Per instance: its part's density (kg/m³; None: no material, taken as 1) and Mass override (kg).
    pub density: Vec<(Option<f64>, Option<f64>)>,
    /// Per instance: fixed in the assembly.
    pub fixed: Vec<bool>,
    /// Explicit mate connectors: (name, the instance it is on, world frame).
    pub connectors: Vec<(String, usize, DMat4)>,
    pub mates: Vec<JointMate>,
}

/// A mate: its sides' instances (None: the assembly's origin) and world frames.
#[derive(Clone, Debug)]
pub struct JointMate {
    pub name: String,
    pub kind: MateType,
    pub a: Option<usize>,
    pub b: Option<usize>,
    pub frame_a: DMat4,
    pub frame_b: DMat4,
    /// (solver free value index: 0 the Z angle, 1 / 2 / 3 the X / Y / Z distance; min, max), SI.
    pub limits: Vec<(usize, Option<f64>, Option<f64>)>,
}

/// A part's volume (m³), centre and inertia about it at unit density, in its own coordinates.
pub type PartMass = (f64, DVec3, DMat3);

#[derive(Clone, Debug)]
pub struct Link {
    pub name: String,
    pub mass: f64,
    /// Centre of mass and inertia about it, in the link's frame.
    pub com: DVec3,
    pub inertia: DMat3,
    /// (part, its placement in the link's frame).
    pub visuals: Vec<(usize, DMat4)>,
}

#[derive(Clone, Debug)]
pub struct Joint {
    pub name: String,
    pub kind: &'static str,
    pub parent: String,
    pub child: String,
    pub origin: DMat4,
    pub axis: DVec3,
    /// A revolute or prismatic joint's (lower, upper).
    pub limit: Option<(f64, f64)>,
}

/// A connector link's mass and inertia (live: 1e-09 kg, 4e-14).
const CONNECTOR_MASS: f64 = 1e-9;
const CONNECTOR_INERTIA: f64 = 4e-14;
/// A slide without limits (live: ±10000 m).
const FREE_SLIDE: f64 = 10000.0;

/// A mate's joints, parent to child: (prismatic or rotating, the parent-side connector's axis 0 / 1 / 2 = X / Y / Z,
/// its limit's index). None: a mate that isn't a joint (Parallel, Tangent, Width).
fn joint_steps(kind: MateType) -> Option<&'static [(bool, usize, Option<usize>)]> {
    Some(match kind {
        MateType::Fastened => &[],
        MateType::Revolute => &[(false, 2, Some(0))],
        MateType::Slider => &[(true, 2, Some(3))],
        MateType::Cylindrical => &[(true, 2, Some(3)), (false, 2, Some(0))],
        MateType::PinSlot => &[(true, 0, Some(1)), (false, 2, Some(0))],
        MateType::Planar => &[(true, 0, Some(1)), (true, 1, Some(2)), (false, 2, Some(0))],
        MateType::Ball => &[(false, 0, None), (false, 1, None), (false, 2, None)],
        _ => return None,
    })
}

/// The robot's links and joints, each sorted by name as live writes them. `instances`: (part, placement);
/// `rigid`: each instance's rigid subassembly (its path), `single`: the option.
pub fn tree(part_names: &[String], instances: &[(usize, DMat4)], rigid: &[Option<Vec<noshape_model::assembly::InstanceId>>], kin: &Kinematics, masses: &[PartMass], single: bool) -> (Vec<Link>, Vec<Joint>) {
    let n = instances.len();
    let root = n;
    let mut uf: Vec<usize> = (0..=n).collect();
    fn find(uf: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while uf[i] != i {
            uf[i] = uf[uf[i]];
            i = uf[i];
        }
        i
    }
    let union = |uf: &mut Vec<usize>, a: usize, b: usize| {
        let (a, b) = (find(uf, a), find(uf, b));
        if a != b {
            // The root's (and the earlier instance's) set keeps its name.
            let (lo, hi) = if a == root || (b != root && a < b) { (a, b) } else { (b, a) };
            uf[hi] = lo;
        }
    };
    let side = |s: Option<usize>| s.unwrap_or(root);
    let mates: Vec<&JointMate> = kin.mates.iter().filter(|m| joint_steps(m.kind).is_some() && side(m.a) != side(m.b)).collect();
    let mated: Vec<bool> = (0..n).map(|i| mates.iter().any(|m| m.a == Some(i) || m.b == Some(i)) || kin.connectors.iter().any(|c| c.1 == i)).collect();
    let siblings = |i: usize, j: usize| i != j && rigid[i].is_some() && rigid[i] == rigid[j];

    let mass_of = |i: usize| {
        let (v, ..) = masses[instances[i].0];
        match kin.density.get(i).copied().unwrap_or_default() {
            (_, Some(m)) if v > 0.0 => m,
            (d, _) => d.unwrap_or(1.0) * v,
        }
    };
    // The fixed instances and their rigid subassemblies' parts. None fixed: the first part of the heaviest cluster of
    // fastened parts or a rigid subassembly, a tie the later's (live: Assembly 1's and J-free's Sub, Top3's Part 1, J-twin's
    // second instance).
    let mut ground: Vec<bool> = kin.fixed.clone();
    ground.resize(n, false);
    if !ground.iter().any(|g| *g) && n > 0 {
        let mut cl: Vec<usize> = (0..=n).collect();
        for i in 0..n {
            for j in (0..i).filter(|j| siblings(i, *j)) {
                let (a, b) = (find(&mut cl, i), find(&mut cl, j));
                cl[a.max(b)] = a.min(b);
            }
        }
        for m in mates.iter().filter(|m| m.kind == MateType::Fastened) {
            let (a, b) = (find(&mut cl, side(m.a)), find(&mut cl, side(m.b)));
            cl[a.max(b)] = a.min(b);
        }
        let mut weight = vec![0.0; n + 1];
        let mut last = vec![0; n + 1];
        for i in 0..n {
            let c = find(&mut cl, i);
            weight[c] += mass_of(i);
            last[c] = i;
        }
        let best = (0..n).filter(|c| find(&mut cl, *c) == *c).max_by(|a, b| weight[*a].total_cmp(&weight[*b]).then(last[*a].cmp(&last[*b])));
        if let Some(c) = best {
            ground[c] = true;
        }
    }
    let seeds = ground.clone();
    for (i, g) in ground.iter_mut().enumerate().take(n) {
        *g |= (0..n).any(|j| seeds[j] && siblings(i, j));
    }
    let mut hung_on_root = Vec::new();
    for i in (0..n).filter(|i| ground[*i]) {
        if single || !mated[i] {
            union(&mut uf, i, root);
        } else {
            hung_on_root.push(i);
        }
    }
    if single {
        for i in 0..n {
            for j in (0..i).filter(|j| siblings(i, *j)) {
                union(&mut uf, i, j);
            }
        }
        for m in mates.iter().filter(|m| m.kind == MateType::Fastened) {
            union(&mut uf, side(m.a), side(m.b));
        }
    }
    let group: Vec<usize> = (0..=n).map(|i| find(&mut uf, i)).collect();
    let members = |g: usize| -> Vec<usize> { (0..n).filter(|i| group[*i] == g).collect() };

    // Each group's link frame and parent joint, outward from the root.
    let mut frame: std::collections::HashMap<usize, DMat4> = std::collections::HashMap::new();
    let mut parent_of: Vec<(usize, usize, Option<&JointMate>)> = Vec::new();
    let rotation_of = |i: usize, at: DVec3| DMat4::from_rotation_translation(DQuat::from_mat4(&instances[i].1).normalize(), at);
    frame.insert(root, DMat4::IDENTITY);
    let mut order = vec![root];
    let mut queue = std::collections::VecDeque::from([root]);
    loop {
        while let Some(g) = queue.pop_front() {
            if g == root {
                for &i in &hung_on_root {
                    if frame.contains_key(&group[i]) {
                        continue;
                    }
                    frame.insert(group[i], instances[i].1);
                    parent_of.push((group[i], root, None));
                    order.push(group[i]);
                    queue.push_back(group[i]);
                }
            }
            for m in &mates {
                let (ga, gb) = (group[side(m.a)], group[side(m.b)]);
                let (child, at) = match (ga == g, gb == g) {
                    (true, false) if !frame.contains_key(&gb) => (gb, m.frame_b),
                    (false, true) if !frame.contains_key(&ga) => (ga, m.frame_a),
                    _ => continue,
                };
                let first = members(child)[0];
                frame.insert(child, rotation_of(first, at.w_axis.truncate()));
                parent_of.push((child, g, Some(m)));
                order.push(child);
                queue.push_back(child);
            }
        }
        // A group no joint reaches hangs on a reached part of its rigid subassembly, else on the root.
        let Some(g) = (0..n).map(|i| group[i]).find(|g| !frame.contains_key(g)) else { break };
        let first = members(g)[0];
        let on = (0..n).find(|j| frame.contains_key(&group[*j]) && siblings(first, *j)).map_or(root, |j| group[j]);
        frame.insert(g, instances[first].1);
        parent_of.push((g, on, None));
        order.push(g);
        queue.push_back(g);
    }

    // Names: `root`, else the first part's, lower case; repeats numbered from 1 (live: part_1, part_1_1).
    let mut used = std::collections::HashSet::new();
    let mut unique = |base: String| {
        let mut name = base.clone();
        let mut k = 0;
        while !used.insert(name.clone()) {
            k += 1;
            name = format!("{base}_{k}");
        }
        name
    };
    let mut name_of: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    // In instance order, whatever the tree's (live's J-twin: the second instance `part_1_1` hangs on the root).
    let mut by_instance = order.clone();
    by_instance.sort_by_key(|g| if *g == root { None } else { members(*g).first().copied() });
    for &g in &by_instance {
        let name = if g == root { "root".to_string() } else { link_name(&part_names[instances[members(g)[0]].0]) };
        name_of.insert(g, unique(name));
    }

    let mut links = Vec::new();
    let mut joints = Vec::new();
    for &g in &order {
        let f = frame[&g];
        let inv = f.inverse();
        let mut bodies: Vec<(f64, DVec3, DMat3)> = Vec::new();
        let mut visuals = Vec::new();
        for i in members(g) {
            let (p, t) = instances[i];
            let (v, c, unit) = masses[p];
            let m = mass_of(i);
            let rho = if v > 0.0 { m / v } else { 0.0 };
            let r = DMat3::from_mat4(t);
            bodies.push((m, t.transform_point3(c), r * unit * r.transpose() * rho));
            visuals.push((p, inv * t));
        }
        if single {
            for c in kin.connectors.iter().filter(|c| group[c.1] == g) {
                bodies.push((CONNECTOR_MASS, c.2.w_axis.truncate(), DMat3::IDENTITY * CONNECTOR_INERTIA));
            }
        }
        let (mass, com, inertia) = combined(&bodies);
        let r = DMat3::from_mat4(f);
        links.push(Link { name: name_of[&g].clone(), mass, com: inv.transform_point3(com), inertia: r.transpose() * inertia * r, visuals });
        let Some(&(_, p, m)) = parent_of.iter().find(|x| x.0 == g) else { continue };
        let origin = frame[&p].inverse() * f;
        let Some(m) = m else {
            joints.push(Joint { name: format!("hanging_node_to_{}_joint", name_of[&p]), kind: "fixed", parent: name_of[&p].clone(), child: name_of[&g].clone(), origin, axis: DVec3::Z, limit: None });
            continue;
        };
        let parent_side = if group[side(m.a)] == p { m.frame_a } else { m.frame_b };
        // In the child's frame, reversed; a zero component stays `-0` as live writes it.
        let axis_of = |col: usize| {
            let a = DMat3::from_mat4(f).transpose() * DMat3::from_mat4(parent_side).col(col);
            -DVec3::from_array(a.to_array().map(|c| if c.abs() < 1e-12 { 0.0 } else { c }))
        };
        let base = link_name(&m.name);
        let steps = joint_steps(m.kind).unwrap_or_default();
        if steps.is_empty() {
            joints.push(Joint { name: base, kind: "fixed", parent: name_of[&p].clone(), child: name_of[&g].clone(), origin, axis: axis_of(2), limit: None });
            continue;
        }
        // Between the steps, empty links at the child's frame, named from the last (live: planar_1 → link planar_1_1
        // → planar_1_1 → link planar_1 → planar_1_2).
        let mut between: Vec<String> = (0..steps.len() - 1).map(|k| unique(if k == 0 { base.clone() } else { format!("{base}_{k}") })).collect();
        between.reverse();
        for name in &between {
            links.push(Link { name: name.clone(), mass: 0.0, com: DVec3::ZERO, inertia: DMat3::ZERO, visuals: Vec::new() });
        }
        let mut parent = name_of[&p].clone();
        for (k, &(slides, col, limit_ix)) in steps.iter().enumerate() {
            let child = between.get(k).cloned().unwrap_or_else(|| name_of[&g].clone());
            let limit = limit_ix.and_then(|ix| m.limits.iter().find(|l| l.0 == ix)).map(|l| (l.1.unwrap_or(-FREE_SLIDE), l.2.unwrap_or(FREE_SLIDE)));
            let (kind, limit) = match (slides, limit) {
                (true, l) => ("prismatic", Some(l.unwrap_or((-FREE_SLIDE, FREE_SLIDE)))),
                (false, Some(l)) => ("revolute", Some(l)),
                (false, None) => ("continuous", None),
            };
            let name = if k == 0 { base.clone() } else { format!("{base}_{k}") };
            joints.push(Joint { name, kind, parent, child: child.clone(), origin: if k == 0 { origin } else { DMat4::IDENTITY }, axis: axis_of(col), limit });
            parent = child;
        }
    }
    if !single {
        for (name, i, at) in &kin.connectors {
            let g = group[*i];
            let link = unique(link_name(name));
            links.push(Link { name: link.clone(), mass: CONNECTOR_MASS, com: DVec3::ZERO, inertia: DMat3::IDENTITY * CONNECTOR_INERTIA, visuals: Vec::new() });
            joints.push(Joint { name: link.clone(), kind: "fixed", parent: name_of[&g].clone(), child: link, origin: frame[&g].inverse() * *at, axis: DVec3::Z, limit: None });
        }
    }
    links.sort_by(|a, b| a.name.cmp(&b.name));
    joints.sort_by(|a, b| a.name.cmp(&b.name));
    (links, joints)
}

/// Masses at their centres with inertias about them: the total, its centre and the inertia about it.
fn combined(bodies: &[(f64, DVec3, DMat3)]) -> (f64, DVec3, DMat3) {
    let mass: f64 = bodies.iter().map(|b| b.0).sum();
    if mass <= 0.0 {
        return (0.0, DVec3::ZERO, DMat3::ZERO);
    }
    let com = bodies.iter().map(|b| b.1 * b.0).sum::<DVec3>() / mass;
    let inertia = bodies.iter().fold(DMat3::ZERO, |acc, (m, c, i)| {
        let d = *c - com;
        acc + *i + (DMat3::IDENTITY * d.length_squared() - DMat3::from_cols(d * d.x, d * d.y, d * d.z)) * *m
    });
    (mass, com, inertia)
}

/// A link or joint name: lower case, every other character `_` (live: "Part 1" → `part_1`, "MC sub" → `mc_sub`).
pub fn link_name(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect()
}

/// A number as live writes it: C++'s default stream format (6 significant digits, `%g`), `-0` kept.
pub fn g6(x: f64) -> String {
    if x == 0.0 {
        return if x.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let e = format!("{x:.5e}");
    let (mantissa, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let trim = |s: &str| if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() };
    if !(-5..6).contains(&exp) {
        return format!("{}e{}{:02}", trim(mantissa), if exp < 0 { '-' } else { '+' }, exp.abs());
    }
    trim(&format!("{x:.*}", (5 - exp) as usize))
}

/// `xyz` and `rpy` of a placement (URDF's fixed-axis roll, pitch, yaw).
fn origin(m: &DMat4) -> String {
    let t = m.w_axis;
    let r = DMat3::from_mat4(*m);
    let (roll, pitch, yaw) = (r.y_axis.z.atan2(r.z_axis.z), -r.x_axis.z.clamp(-1.0, 1.0).asin(), r.x_axis.y.atan2(r.x_axis.x));
    format!("<origin xyz=\"{} {} {}\" rpy=\"{} {} {}\" />", g6(t.x), g6(t.y), g6(t.z), g6(roll), g6(pitch), g6(yaw))
}

/// The robot's `.urdf`: `meshes[p]` each part's mesh file and colour (sRGB 0..1).
pub fn urdf_text(robot: &str, links: &[Link], joints: &[Joint], meshes: &[(String, [f64; 3])]) -> String {
    let mut s = format!("<!--URDF generated by Noshape-->\n<robot name=\"{robot}\">\n");
    for l in links {
        // Live: a link with no part and no mass (an empty root, a joint chain's links) is an empty element.
        if l.mass == 0.0 && l.visuals.is_empty() {
            s += &format!("    <link name=\"{}\" />\n", l.name);
            continue;
        }
        let i = l.inertia;
        s += &format!(
            "    <link name=\"{}\">\n        <inertial>\n            <mass value=\"{}\" />\n            {}\n            <inertia ixx=\"{}\" ixy=\"{}\" ixz=\"{}\" iyy=\"{}\" iyz=\"{}\" izz=\"{}\" />\n        </inertial>\n",
            l.name,
            g6(l.mass),
            origin(&DMat4::from_translation(l.com)),
            g6(i.x_axis.x),
            g6(i.y_axis.x),
            g6(i.z_axis.x),
            g6(i.y_axis.y),
            g6(i.z_axis.y),
            g6(i.z_axis.z)
        );
        for (p, at) in &l.visuals {
            let (file, c) = &meshes[*p];
            s += &format!(
                "        <visual>\n            {}\n            <geometry>\n                <mesh filename=\"package://{robot}/meshes/{file}\" scale=\"1 1 1\" />\n            </geometry>\n            <material name=\"{:.6}_{:.6}_{:.6}_0.000000_0.000000\">\n                <color rgba=\"{} {} {} 1\" />\n            </material>\n        </visual>\n",
                origin(at),
                c[0],
                c[1],
                c[2],
                g6(c[0]),
                g6(c[1]),
                g6(c[2])
            );
        }
        s += "    </link>\n";
    }
    for j in joints {
        let a = j.axis;
        let limit = j.limit.map(|(lo, hi)| format!("        <limit effort=\"1\" velocity=\"1\" lower=\"{}\" upper=\"{}\" />\n", g6(lo), g6(hi))).unwrap_or_default();
        s += &format!(
            "    <joint name=\"{}\" type=\"{}\">\n        {}\n        <axis xyz=\"{} {} {}\" />\n        <parent link=\"{}\" />\n        <child link=\"{}\" />\n{limit}    </joint>\n",
            j.name,
            j.kind,
            origin(&j.origin),
            g6(a.x),
            g6(a.y),
            g6(a.z),
            j.parent,
            j.child
        );
    }
    s + "</robot>\n"
}

/// The ROS launch file beside it (live's: the robot and joint state publishers, map → root, rviz).
pub fn launch_text(robot: &str) -> String {
    format!(
        "<launch>\n      <arg name=\"urdf_file\" default=\"$(find {robot})/urdf/{robot}.urdf\"/>\n      <param name=\"robot_description\" textfile=\"$(arg urdf_file)\"/>\n\n      <node pkg=\"robot_state_publisher\" type=\"robot_state_publisher\" name=\"robot_state_publisher\" output=\"screen\"/>\n      <arg name=\"use_gui\" default=\"true\"/>\n      <node pkg=\"joint_state_publisher_gui\" type=\"joint_state_publisher_gui\" name=\"joint_state_publisher_gui\">\n            <rosparam param=\"zeros\">\n            </rosparam>\n      </node>\n\n      <node pkg=\"tf2_ros\" type=\"static_transform_publisher\" name=\"map_to_root\" args=\"0 0 0 0 0 0 map root\" />\n      <node pkg=\"rviz\" type=\"rviz\" name=\"rviz\" output=\"screen\"/>\n</launch>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_as_live_writes_them() {
        let cases = [(1.666666e-10, "1.66667e-10"), (4e-6, "4e-06"), (std::f64::consts::PI, "3.14159"), (-2.7485, "-2.7485"), (0.01, "0.01"), (-0.0, "-0"), (0.0, "0"), (8.001e-6, "8.001e-06"), (0.0249981, "0.0249981"), (123456.7, "123457"), (1234567.0, "1.23457e+06")];
        for (x, s) in cases {
            assert_eq!(g6(x), s, "{x}");
        }
    }

    type Sample = (Vec<String>, Vec<(usize, DMat4)>, Vec<Option<Vec<noshape_model::assembly::InstanceId>>>, Kinematics, Vec<PartMass>);

    /// Live's Assembly 1 (urdf-single-link-1120): a rigid Sub of Part 2 (first) and Part 1, and Part 3 on Part 1 by a
    /// revolute between the connectors "MC sub" (Part 1's top) and "MC arm" (Part 3's bottom), Part 3 turned over.
    fn assembly_1() -> Sample {
        let names = vec!["Part 2".to_string(), "Part 1".to_string(), "Part 3".to_string()];
        let turned = DMat4::from_translation(DVec3::new(0.01, 0.01, 0.01)) * DMat4::from_rotation_z(-2.7485) * DMat4::from_rotation_x(std::f64::consts::PI) * DMat4::from_translation(DVec3::new(-0.01, -0.05, 0.0));
        let instances = vec![(0, DMat4::IDENTITY), (1, DMat4::IDENTITY), (2, turned)];
        let sub = Some(vec![noshape_model::assembly::InstanceId(7)]);
        let rigid = vec![sub.clone(), sub, None];
        let mc_sub = DMat4::from_translation(DVec3::new(0.01, 0.01, 0.01));
        let mc_arm = turned * DMat4::from_translation(DVec3::new(0.01, 0.05, 0.0)) * DMat4::from_rotation_x(std::f64::consts::PI);
        let kin = Kinematics {
            density: vec![(None, None); 3],
            fixed: vec![false; 3],
            connectors: vec![("MC sub".into(), 1, mc_sub), ("MC arm".into(), 2, mc_arm)],
            mates: vec![JointMate { name: "Revolute 1".into(), kind: MateType::Revolute, a: Some(1), b: Some(2), frame_a: mc_sub, frame_b: mc_arm, limits: Vec::new() }],
        };
        let block = |c: DVec3| (4e-6, c, DMat3::from_diagonal(DVec3::new(1.66667e-10, 1.66667e-10, 2.66667e-10)));
        let masses = vec![block(DVec3::new(0.04, 0.01, 0.005)), block(DVec3::new(0.01, 0.01, 0.005)), block(DVec3::new(0.01, 0.05, 0.005))];
        (names, instances, rigid, kin, masses)
    }

    #[test]
    fn a_link_per_part_a_joint_per_mate_and_connector_links() {
        let (names, instances, rigid, kin, masses) = assembly_1();
        let (links, joints) = tree(&names, &instances, &rigid, &kin, &masses, false);
        let ln: Vec<&str> = links.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(ln, ["mc_arm", "mc_sub", "part_1", "part_3", "root"]);
        let jn: Vec<(&str, &str, &str, &str)> = joints.iter().map(|j| (j.name.as_str(), j.kind, j.parent.as_str(), j.child.as_str())).collect();
        assert_eq!(jn, [("hanging_node_to_root_joint", "fixed", "root", "part_1"), ("mc_arm", "fixed", "part_3", "mc_arm"), ("mc_sub", "fixed", "part_1", "mc_sub"), ("revolute_1", "continuous", "part_1", "part_3")]);
        let text = urdf_text("assembly_1", &links, &joints, &[("Part_2.stl".into(), [0.647059; 3]), ("Part_1.stl".into(), [0.6; 3]), ("Part_3.stl".into(), [0.2; 3])]);
        // Live's revolute and Part 3's visual (its mesh in Part Studio coordinates, the link at the connector).
        let revolute = text.split("<joint name=\"revolute_1\"").nth(1).expect("the revolute");
        let numbers = |key: &str| -> Vec<f64> { revolute.split(&format!("{key}=\"")).nth(1).and_then(|r| r.split('"').next()).expect(key).split(' ').map(|x| x.parse().expect("a number")).collect() };
        let near = |a: Vec<f64>, b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        assert!(near(numbers("xyz"), [0.01, 0.01, 0.01]) && near(numbers("rpy"), [std::f64::consts::PI, 0.0, -2.7485]), "{text}");
        assert!(revolute.contains("<axis xyz=\"-0 -0 1\" />"), "{text}");
        assert!(text.contains("<origin xyz=\"-0.01 -0.05 0\" rpy="), "{text}");
        assert!(text.contains("<mass value=\"1e-09\" />"), "{text}");
        assert!(text.contains("<mass value=\"4e-06\" />\n            <origin xyz=\"0.04 0.01 0.005\" rpy=\"0 -0 0\" />"), "{text}");
    }

    #[test]
    fn single_link_merges_the_rigid_subassembly_and_folds_the_connectors() {
        let (names, instances, rigid, kin, masses) = assembly_1();
        let (links, joints) = tree(&names, &instances, &rigid, &kin, &masses, true);
        let ln: Vec<(&str, usize)> = links.iter().map(|l| (l.name.as_str(), l.visuals.len())).collect();
        assert_eq!(ln, [("part_3", 1), ("root", 2)]);
        assert_eq!(joints.iter().map(|j| (j.name.as_str(), j.parent.as_str(), j.child.as_str())).collect::<Vec<_>>(), [("revolute_1", "root", "part_3")]);
        // Live: 8.001e-06 at (0.0249981, 0.01, 0.00500062); Part 3 4.001e-06.
        let root = &links[1];
        assert!((root.mass - 8.001e-6).abs() < 1e-12 && (root.com - DVec3::new(0.0249981, 0.01, 0.00500062)).abs().max_element() < 1e-7, "{root:?}");
        assert!((links[0].mass - 4.001e-6).abs() < 1e-12);
        assert!((root.inertia.y_axis.y - 2.13362e-9).abs() < 1e-13, "{:?}", root.inertia);
    }

    /// Live's "URDF joint types and limits (11-20)": a Planar is two prismatic joints and a continuous one through two
    /// empty links named from the last; a limited Revolute is `revolute` with its limits in radians; a Slider without
    /// limits slides ±10000 m.
    #[test]
    fn several_degrees_of_freedom_chain_through_empty_links_and_limits_are_written() {
        let (names, instances, rigid, mut kin, masses) = assembly_1();
        kin.mates[0].kind = MateType::Planar;
        let (links, joints) = tree(&names, &instances, &rigid, &kin, &masses, false);
        let jn: Vec<(&str, &str, &str, &str)> = joints.iter().filter(|j| j.name.starts_with("revolute")).map(|j| (j.name.as_str(), j.kind, j.parent.as_str(), j.child.as_str())).collect();
        assert_eq!(jn, [("revolute_1", "prismatic", "part_1", "revolute_1_1"), ("revolute_1_1", "prismatic", "revolute_1_1", "revolute_1"), ("revolute_1_2", "continuous", "revolute_1", "part_3")]);
        assert_eq!(joints[4].origin, DMat4::IDENTITY);
        let text = urdf_text("assembly_1", &links, &joints, &[("Part_2.stl".into(), [0.6; 3]), ("Part_1.stl".into(), [0.6; 3]), ("Part_3.stl".into(), [0.6; 3])]);
        assert!(text.contains("    <link name=\"revolute_1\" />\n    <link name=\"revolute_1_1\" />\n"), "{text}");
        assert!(text.contains("<child link=\"revolute_1_1\" />\n        <limit effort=\"1\" velocity=\"1\" lower=\"-10000\" upper=\"10000\" />\n    </joint>"), "{text}");
        assert!(text.contains("<axis xyz=\"-0 -0 1\" />\n        <parent link=\"revolute_1\" />"), "{text}");

        kin.mates[0].kind = MateType::Revolute;
        kin.mates[0].limits = vec![(0, Some(-30f64.to_radians()), Some(60f64.to_radians()))];
        let (links, joints) = tree(&names, &instances, &rigid, &kin, &masses, false);
        let text = urdf_text("assembly_1", &links, &joints, &[("Part_2.stl".into(), [0.6; 3]), ("Part_1.stl".into(), [0.6; 3]), ("Part_3.stl".into(), [0.6; 3])]);
        assert!(text.contains("<joint name=\"revolute_1\" type=\"revolute\">"), "{text}");
        assert!(text.contains("<limit effort=\"1\" velocity=\"1\" lower=\"-0.523599\" upper=\"1.0472\" />"), "{text}");
    }

    /// Live's J-twin: two instances of one part, nothing fixed, a revolute between: the later instance hangs on an empty
    /// root, named `part_1_1` after the first's `part_1`.
    #[test]
    fn twin_instances_take_numbered_names_and_the_later_one_is_the_root() {
        let names = vec!["Part 1".to_string()];
        let other = DMat4::from_translation(DVec3::new(0.0, 0.05, 0.0));
        let instances = vec![(0, DMat4::IDENTITY), (0, other)];
        let kin = Kinematics {
            density: vec![(None, None); 2],
            fixed: vec![false; 2],
            connectors: Vec::new(),
            mates: vec![JointMate { name: "Revolute 1".into(), kind: MateType::Revolute, a: Some(0), b: Some(1), frame_a: DMat4::IDENTITY, frame_b: other, limits: Vec::new() }],
        };
        let block = (4e-6, DVec3::new(0.01, 0.01, 0.005), DMat3::IDENTITY * 1e-5);
        let (links, joints) = tree(&names, &instances, &[None, None], &kin, &[block], false);
        assert_eq!(links.iter().map(|l| (l.name.as_str(), l.visuals.len())).collect::<Vec<_>>(), [("part_1", 1), ("part_1_1", 1), ("root", 0)]);
        let jn: Vec<(&str, &str, &str)> = joints.iter().map(|j| (j.name.as_str(), j.parent.as_str(), j.child.as_str())).collect();
        assert_eq!(jn, [("hanging_node_to_root_joint", "root", "part_1_1"), ("revolute_1", "part_1_1", "part_1")]);
        assert!(urdf_text("j_twin", &links, &joints, &[("Part_1.stl".into(), [0.6; 3])]).contains("    <link name=\"root\" />\n"));
    }
}
