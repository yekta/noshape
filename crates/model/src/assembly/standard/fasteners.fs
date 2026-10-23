FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

// Built-in standard content. One feature per family (screw, nut, washer,
// pin, key, retaining ring, O-ring); integer kinds pick the head, drive,
// neck and point forms, and the lengths are the standards' own symbols
// (catalogue.rs passes each size's table values). Parts are built along Z:
// a screw's bearing face (a countersunk head's top) on z = 0 with the shank
// down to z = -l, nuts and washers from z = 0 up.

const Z = 0 * meter;

/** Optional dimensions: 0 unless the size sets them. */
const LENGTH_0 = { (meter) : [0, 0, 500] } as LengthBoundSpec;
const EPS = 1e-5 * meter;

/** The items of `a` followed by those of `b`. */
function join(a is array, b is array) returns array
{
    var out = a;
    for (var x in b)
        out = append(out, x);
    return out;
}

/** A 2D point of a revolved profile: radius `r` at height `z`. */
function rz(r is ValueWithUnits, z is ValueWithUnits) returns Vector
{
    return vector(r, z);
}

/** The sketch plane whose x is the radius and y the height (Z). */
function profilePlane() returns Plane
{
    return plane(vector(Z, Z, Z), vector(0, -1, 0), vector(1, 0, 0));
}

/**
 * Revolve a closed profile about Z. `segments` is an array of maps: the
 * first {"p"} starts it, each next one is a line to "p" or, with "mid", an
 * arc through "mid" to "p"; the last point closes back to the first.
 */
function revolveProfile(context is Context, id is Id, segments is array) returns Query
{
    const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : profilePlane() });
    var prev = segments[0].p;
    for (var i = 1; i <= size(segments); i += 1)
    {
        const s = i < size(segments) ? segments[i] : { "p" : segments[0].p };
        if (s.mid != undefined)
            skArc(sketch, "a" ~ i, { "start" : prev, "mid" : s.mid, "end" : s.p });
        else if (norm(s.p - prev) > EPS)
            skLineSegment(sketch, "l" ~ i, { "start" : prev, "end" : s.p });
        prev = s.p;
    }
    skSolve(sketch);
    opRevolve(context, id + "revolve", { "entities" : qSketchRegion(id + "sketch"), "axis" : line(vector(Z, Z, Z), vector(0, 0, 1)), "angleForward" : 2 * PI * radian });
    opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
    return qCreatedBy(id + "revolve", EntityType.BODY);
}

/** A polygon (2D points in XY) extruded from z0 by h. */
function prismZ(context is Context, id is Id, z0 is ValueWithUnits, h is ValueWithUnits, points is array) returns Query
{
    var p = XY_PLANE;
    p.origin[2] = z0;
    const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : p });
    skPolyline(sketch, "outline", { "points" : append(points, points[0]) });
    skSolve(sketch);
    opExtrude(context, id + "extrude", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : h });
    opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
    return qCreatedBy(id + "extrude", EntityType.BODY);
}

/** Regular polygon of `n` sides, across flats `s`, a flat facing +Y. */
function polygonPoints(n is number, s is ValueWithUnits, turn is number) returns array
{
    const r = s / 2 / cos(PI / n * radian);
    var pts = [];
    for (var i = 0; i < n; i += 1)
    {
        const a = (2 * i + 1 + turn) * PI / n * radian;
        pts = append(pts, vector(r * cos(a), r * sin(a)));
    }
    return pts;
}

function cylinderZ(context is Context, id is Id, z0 is ValueWithUnits, z1 is ValueWithUnits, r is ValueWithUnits) returns Query
{
    fCylinder(context, id, { "bottomCenter" : vector(Z, Z, z0), "topCenter" : vector(Z, Z, z1), "radius" : r });
    return qCreatedBy(id, EntityType.BODY);
}

function cuboid(context is Context, id is Id, c1 is Vector, c2 is Vector) returns Query
{
    fCuboid(context, id, { "corner1" : c1, "corner2" : c2 });
    return qCreatedBy(id, EntityType.BODY);
}

/** Keep `target` only where `tool` is (an intersection takes all bodies as tools). */
function intersect(context is Context, id is Id, target is Query, tool is Query)
{
    opBoolean(context, id, { "tools" : qUnion([target, tool]), "operationType" : BooleanOperationType.INTERSECTION });
}

function subtract(context is Context, id is Id, target is Query, tool is Query)
{
    opBoolean(context, id, { "tools" : tool, "targets" : target, "operationType" : BooleanOperationType.SUBTRACTION });
}

function unite(context is Context, id is Id, bodies is array)
{
    if (size(evaluateQuery(context, qUnion(bodies))) > 1)
        opBoolean(context, id, { "tools" : qUnion(bodies), "operationType" : BooleanOperationType.UNION });
}

/**
 * The solid bodies feature `id` has made so far, resolved now (a boolean
 * can give a body a new identity, so queries made before it go stale).
 * Resolve it before creating the next tool body.
 */
function solidsOf(context is Context, id is Id) returns Query
{
    return qUnion(evaluateQuery(context, qCreatedBy(id, EntityType.BODY)));
}

/** Unite every body feature `id` has made into one part. */
function uniteAll(context is Context, id is Id)
{
    unite(context, id + "uniteAll", [qCreatedBy(id, EntityType.BODY)]);
}

/**
 * The 30 degree chamfer of hex and square heads and nuts: the corners cut
 * by cones from diameter `dw` on the face (`top` and / or bottom), one tool
 * (the region under both cones).
 */
function cornerChamfer(context is Context, id is Id, body is Query, z0 is ValueWithUnits, h is ValueWithUnits, dw is ValueWithUnits, top is boolean, bottom is boolean)
{
    if (!top && !bottom)
        return;
    const slope = 1 / tan(30 * degree);
    const big = dw * 10 + h * 10;
    const r0 = bottom ? dw / 2 : big;
    const r1 = top ? dw / 2 : big;
    const z1 = z0 + h;
    // Where the two cones cross, inside the prism's height.
    const zc = min(max((r1 - r0 + (z1 + z0) * slope) / (2 * slope), z0), z1);
    const rc = min(r0 + (zc - z0) * slope, r1 + (z1 - zc) * slope);
    // The tool reaches just past both faces (no faces coplanar with the
    // prism's), its radius there along the tighter cone.
    const zb = z0 - EPS;
    const zt = z1 + EPS;
    const rb = min(r0 - EPS * slope, r1 + (z1 - zb) * slope);
    const rt = min(r1 - EPS * slope, r0 + (zt - z0) * slope);
    intersect(context, id + "cut", body, revolveProfile(context, id + "cones", [{ "p" : rz(Z, zb) }, { "p" : rz(rb, zb) }, { "p" : rz(rc, zc) }, { "p" : rz(rt, zt) }, { "p" : rz(Z, zt) }]));
}

/**
 * A round head's profile from z0: radius `r`, height `k`, top radius `rf`.
 * rf >= r: a spherical top of radius rf over a cylindrical side; a smaller
 * rf rounds the top edge; 0: a flat top.
 */
function roundHead(r is ValueWithUnits, k is ValueWithUnits, rf is ValueWithUnits, z0 is ValueWithUnits) returns array
{
    if (rf <= EPS)
        return [{ "p" : rz(Z, z0) }, { "p" : rz(r, z0) }, { "p" : rz(r, z0 + k) }, { "p" : rz(Z, z0 + k) }];
    if (rf >= r - EPS)
    {
        const side = max(k - rf + sqrt(max(rf * rf - r * r, Z * Z)), Z);
        const midR = r / 2;
        const midZ = z0 + k - rf + sqrt(rf * rf - midR * midR);
        if (side <= EPS)
            return [{ "p" : rz(Z, z0) }, { "p" : rz(r, z0) }, { "p" : rz(Z, z0 + k), "mid" : rz(midR, midZ) }];
        return [{ "p" : rz(Z, z0) }, { "p" : rz(r, z0) }, { "p" : rz(r, z0 + side) }, { "p" : rz(Z, z0 + k), "mid" : rz(midR, midZ) }];
    }
    const c = min(rf, k);
    const m = c * (1 - 1 / sqrt(2));
    return [{ "p" : rz(Z, z0) }, { "p" : rz(r, z0) }, { "p" : rz(r, z0 + k - c) }, { "p" : rz(r - c, z0 + k), "mid" : rz(r - m, z0 + k - m) }, { "p" : rz(Z, z0 + k) }];
}

/**
 * A pan head (live's tapping screws): the side of radius r up to an edge
 * fillet re, tangent to a crown sphere of radius rc whose top is at z0 + k.
 */
function panCrown(r is ValueWithUnits, k is ValueWithUnits, rc is ValueWithUnits, re is ValueWithUnits, z0 is ValueWithUnits) returns array
{
    const zc = z0 + k - rc;
    const fx = r - re;
    const fz = zc + sqrt((rc - re) * (rc - re) - fx * fx);
    // The tangent point on the sphere, along the line from its centre
    // through the fillet's centre.
    const dist = sqrt(fx * fx + (fz - zc) * (fz - zc));
    const tx = fx * rc / dist;
    const tz = zc + (fz - zc) * rc / dist;
    // Arc midpoints: the fillet's halfway between +x and the tangent
    // direction; the crown's halfway between the tangent point and the top.
    const ux = 1 + fx / dist;
    const uz = (fz - zc) / dist;
    const ul = sqrt(ux * ux + uz * uz);
    const a = atan2(tx, tz - zc) / 2;
    return [{ "p" : rz(Z, z0) }, { "p" : rz(r, z0) }, { "p" : rz(r, fz) },
            { "p" : rz(tx, tz), "mid" : rz(fx + re * ux / ul, fz + re * uz / ul) },
            { "p" : rz(Z, z0 + k), "mid" : rz(rc * sin(a), zc + rc * cos(a)) }];
}

/**
 * A crowned head under a Type III square recess: the head's profile up to
 * the crown's rim, extended straight up and revolved, intersected with the
 * crown sphere (centre z = zc, radius rc) revolved about X. The sphere's
 * seam and poles then lie at the centre's height, far below the crown, so no
 * seam crosses the square mouth and its four fillets stay single faces
 * (kern; live's 19 faces). Returns the head body.
 */
function seamlessCrown(context is Context, id is Id, toRim is array, rimR is ValueWithUnits, top is ValueWithUnits, zc is ValueWithUnits, rc is ValueWithUnits) returns Query
{
    const up = top + rc;
    revolveProfile(context, id + "ext", join(toRim, [{ "p" : rz(rimR, up) }, { "p" : rz(Z, up) }]));
    var p = XY_PLANE;
    p.origin[2] = zc;
    const sketch = newSketchOnPlane(context, id + "sphereSketch", { "sketchPlane" : p });
    skArc(sketch, "half", { "start" : vector(-rc, Z), "mid" : vector(Z, rc), "end" : vector(rc, Z) });
    skLineSegment(sketch, "chord", { "start" : vector(rc, Z), "end" : vector(-rc, Z) });
    skSolve(sketch);
    opRevolve(context, id + "sphere", { "entities" : qSketchRegion(id + "sphereSketch"), "axis" : line(vector(Z, Z, zc), vector(1, 0, 0)), "angleForward" : 2 * PI * radian });
    opDeleteBodies(context, id + "deleteSphereSketch", { "entities" : qCreatedBy(id + "sphereSketch", EntityType.BODY) });
    intersect(context, id + "crown", qCreatedBy(id + "ext", EntityType.BODY), qCreatedBy(id + "sphere", EntityType.BODY));
    return qCreatedBy(id, EntityType.BODY);
}

/** A spherical cap of base radius r and height h on z0 (h > 0 up). */
function domeCap(r is ValueWithUnits, h is ValueWithUnits, z0 is ValueWithUnits) returns array
{
    const big = (r * r + h * h) / (2 * h);
    const midR = r / 2;
    return [{ "p" : rz(Z, z0) }, { "p" : rz(r, z0) }, { "p" : rz(Z, z0 + h), "mid" : rz(midR, z0 + h - big + sqrt(big * big - midR * midR)) }];
}

// Screw heads (definition.head).
const HEAD_NONE = 0;
const HEAD_HEX = 1;
const HEAD_SQUARE = 2;
const HEAD_ROUND = 3;
const HEAD_COUNTERSUNK = 4;
const HEAD_RAISED_COUNTERSUNK = 5;
const HEAD_T = 6;
const HEAD_EYE = 7;
const HEAD_LIFTING_EYE = 8;
const HEAD_WING = 9;

// Drives (definition.drive).
const DRIVE_NONE = 0;
const DRIVE_HEX_SOCKET = 1;
const DRIVE_SLOT = 2;
const DRIVE_CROSS = 3;
const DRIVE_POZI = 4;
const DRIVE_HEXALOBULAR = 5;
// ASME B18.6.4 tapping-screw recesses as live models them (behaviours.md
// "skb: Type III / Type VI recess features"): square side sd / hexalobular
// spline through Ø sd and Ø b, both with a 140 degree cone bottom at depth t
// and a 0.002 in mouth fillet.
const DRIVE_SQUARE_III = 6;
const DRIVE_TYPE_VI = 7;

// Necks under the head (definition.neck).
const NECK_NONE = 0;
const NECK_SQUARE = 1;
const NECK_RIBBED = 2;
const NECK_FIN = 3;
const NECK_SHOULDER = 4;
const NECK_COLLAR = 5;

// Ends of the shank (definition.point).
const POINT_CHAMFER = 0;
const POINT_CONE = 1;
const POINT_CUP = 2;
const POINT_DOG = 3;
const POINT_CONE_TIP = 4;
const POINT_ROUND = 5;
const POINT_FLAT = 6;
// ASME B18.6.4 tapping screw points beyond the cone ones: the blunt cone
// (z = taper length, dp = point diameter) with a cutting feature:
// BT / T: the quarter-cut slot (btSlot); D: a slot through one radius;
// G: the slot through the full diameter; F: two flutes tilted 7 degrees
// (tipSlots). C is POINT_CONE.
const POINT_BT = 7;
const POINT_TIP_D = 8;
const POINT_TIP_G = 9;
const POINT_TIP_F = 10;

/** The head; returns [body, z of its top]. */
function screwHead(context is Context, id is Id, def is map) returns array
{
    const kind = def.head;
    if (kind == HEAD_HEX || kind == HEAD_SQUARE)
    {
        const n = kind == HEAD_HEX ? 6 : 4;
        // A washer face or flange (dc, c) under the head.
        const z0 = def.c > EPS ? def.c : Z;
        const prism = prismZ(context, id + "prism", z0, def.k - z0, polygonPoints(n, def.s, 0));
        cornerChamfer(context, id + "chamfer", prism, z0, def.k - z0, def.dw > EPS ? def.dw : def.s, true, false);
        if (def.c > EPS)
            cylinderZ(context, id + "flange", Z, def.c + EPS, def.dc / 2);
        return [qCreatedBy(id, EntityType.BODY), def.k];
    }
    if (kind == HEAD_ROUND)
    {
        // A collar (dc, c) under the head (DIN 967) lifts it by c.
        const pan = def.re > EPS && def.rf > EPS;
        const profile = pan ? panCrown(def.dk / 2, def.k - def.c, def.rf, def.re, def.c) : roundHead(def.dk / 2, def.k - def.c, def.rf, def.c);
        // Spherical crowns (the pan's, a dome) under a square recess: the
        // profile up to the crown's rim, the crown from a seamless sphere.
        const dome = !pan && def.rf >= def.dk / 2 - EPS && def.rf > EPS;
        if ((def.drive == DRIVE_SQUARE_III || (def.drive == DRIVE_CROSS && def.rcd > EPS)) && (pan || dome))
        {
            const rim = profile[size(profile) - 2]["p"];
            seamlessCrown(context, id + "round", subArray(profile, 0, size(profile) - 1), rim[0], def.k, def.k - def.rf, def.rf);
        }
        else
            revolveProfile(context, id + "round", profile);
        if (def.c > EPS)
            cylinderZ(context, id + "collar", Z, def.c + EPS, def.dc / 2);
        return [qCreatedBy(id, EntityType.BODY), def.k];
    }
    if (kind == HEAD_COUNTERSUNK || kind == HEAD_RAISED_COUNTERSUNK)
    {
        // The cone from dk on top down to the shank, a cylindrical edge c.
        // Undercut heads (live's tapping screws): the 82 degree cone stops
        // at the depth k, then a flat step in to the shank.
        var pts = [{ "p" : rz(Z, -def.k) }, { "p" : rz(def.d / 2, -def.k) }, { "p" : rz(def.dk / 2, -def.c) }, { "p" : rz(def.dk / 2, Z) }];
        if (def.undercut == 1)
            pts = [{ "p" : rz(Z, -def.k) }, { "p" : rz(def.d / 2, -def.k) }, { "p" : rz(def.dk / 2 - def.k * tan(41 * degree), -def.k) }, { "p" : rz(def.dk / 2, Z) }];
        if (kind == HEAD_RAISED_COUNTERSUNK && def.f > EPS)
        {
            const big = (def.dk * def.dk / 4 + def.f * def.f) / (2 * def.f);
            const midR = def.dk / 4;
            // (Type I too: the seam would split the +x arm end's mouth edge.)
            if (def.drive == DRIVE_SQUARE_III || (def.drive == DRIVE_CROSS && def.rcd > EPS))
                return [seamlessCrown(context, id + "csk", pts, def.dk / 2, def.f, def.f - big, big), def.f];
            pts = append(pts, { "p" : rz(Z, def.f), "mid" : rz(midR, def.f - big + sqrt(big * big - midR * midR)) });
            return [revolveProfile(context, id + "csk", pts), def.f];
        }
        pts = append(pts, { "p" : rz(Z, Z) });
        return [revolveProfile(context, id + "csk", pts), Z];
    }
    if (kind == HEAD_T)
        return [prismZ(context, id + "t", Z, def.k, [vector(-def.dk / 2, -def.s / 2), vector(def.dk / 2, -def.s / 2), vector(def.dk / 2, def.s / 2), vector(-def.dk / 2, def.s / 2)]), def.k];
    if (kind == HEAD_EYE)
    {
        // A flat eye (thickness s, outer dk, hole e1) on a collar (dc, c).
        const sketch = newSketchOnPlane(context, id + "eyeSketch", { "sketchPlane" : plane(vector(Z, def.s / 2, Z), vector(0, -1, 0), vector(1, 0, 0)) });
        const cz = def.c + def.dk / 2;
        skCircle(sketch, "outer", { "center" : vector(Z, cz), "radius" : def.dk / 2 });
        skCircle(sketch, "hole", { "center" : vector(Z, cz), "radius" : def.e1 / 2 });
        skLineSegment(sketch, "l1", { "start" : vector(-def.dk / 2, cz), "end" : vector(-def.dk / 2, Z) });
        skLineSegment(sketch, "l2", { "start" : vector(-def.dk / 2, Z), "end" : vector(def.dk / 2, Z) });
        skLineSegment(sketch, "l3", { "start" : vector(def.dk / 2, Z), "end" : vector(def.dk / 2, cz) });
        skSolve(sketch);
        opExtrude(context, id + "eye", { "entities" : qSketchRegion(id + "eyeSketch", true), "direction" : vector(0, -1, 0), "endBound" : BoundingType.BLIND, "endDepth" : def.s });
        opDeleteBodies(context, id + "deleteEyeSketch", { "entities" : qCreatedBy(id + "eyeSketch", EntityType.BODY) });
        if (def.c > EPS)
            cylinderZ(context, id + "collar", Z, def.c + EPS, def.dc / 2);
        return [qCreatedBy(id, EntityType.BODY), cz + def.dk / 2];
    }
    if (kind == HEAD_LIFTING_EYE)
    {
        // A ring (inner e1, outer dk, section e2) standing on a collar (dc, c).
        const section = (def.dk - def.e1) / 2;
        const center = def.c + def.k - def.dk / 2;
        const sketch = newSketchOnPlane(context, id + "ringSketch", { "sketchPlane" : plane(vector(def.e1 / 2 + section / 2, Z, center), vector(0, 0, 1), vector(1, 0, 0)) });
        skCircle(sketch, "section", { "center" : vector(Z, Z), "radius" : section / 2 });
        skSolve(sketch);
        opRevolve(context, id + "ring", { "entities" : qSketchRegion(id + "ringSketch"), "axis" : line(vector(Z, Z, center), vector(0, 1, 0)), "angleForward" : 2 * PI * radian });
        opDeleteBodies(context, id + "deleteRingSketch", { "entities" : qCreatedBy(id + "ringSketch", EntityType.BODY) });
        cylinderZ(context, id + "collar", Z, def.c + EPS, def.dc / 2);
        // A neck (e2) up to the ring when it doesn't stand on the collar.
        const gap = center - def.dk / 2 - def.c;
        if (gap > EPS && def.e2 > EPS)
            cylinderZ(context, id + "neck", def.c - EPS, center - def.e1 / 2, def.e2 / 2);
        return [qCreatedBy(id, EntityType.BODY), def.c + def.k];
    }
    if (kind == HEAD_WING)
    {
        // Two wings (span dk, height k, thickness s) on a hub (dc).
        const hub = cylinderZ(context, id + "hub", Z, def.k * 0.5, def.dc / 2);
        const wing = prismZ(context, id + "wing", Z, def.s, [vector(-def.dk / 2, Z), vector(def.dk / 2, Z), vector(def.dk / 2, def.k), vector(-def.dk / 2, def.k)]);
        opTransform(context, id + "stand", { "bodies" : wing, "transform" : rotationAround(line(vector(Z, Z, Z), vector(1, 0, 0)), 90 * degree) * transform(vector(Z, Z, -def.s / 2)) });
        return [qCreatedBy(id, EntityType.BODY), def.k];
    }
    return [qNothing(), Z];
}

/**
 * The head top's height at radius r: the crown sphere of a pan, dome or
 * raised countersunk head, else the flat top at `top`. Live's recesses draft
 * the arm ends about the plane through the points where they meet the head
 * (its "THREE_POINT" plane).
 */
function headTopAt(def is map, r is ValueWithUnits, top is ValueWithUnits) returns ValueWithUnits
{
    if (def.head == HEAD_ROUND && def.rf > EPS && (def.re > EPS || def.rf >= def.dk / 2 - EPS) && r < def.dk / 2)
        return def.k - def.rf + sqrt(max(def.rf * def.rf - r * r, Z * Z));
    if (def.head == HEAD_RAISED_COUNTERSUNK && def.f > EPS && r < def.dk / 2)
    {
        const big = (def.dk * def.dk / 4 + def.f * def.f) / (2 * def.f);
        return def.f - big + sqrt(max(big * big - r * r, Z * Z));
    }
    return top;
}

/**
 * Live's Type I (Phillips) recess (behaviours.md "skb (10-22 19:40)", Hex #8
 * Type I): the cross (arms L × W) with a kite in each inner corner (starts
 * Cd apart, apex Fd/2 from the centre) cut from the floor plane at
 * top − (D − T) upward; the 4 arm ends drafted 26.3 degrees about the top,
 * the 8 arm sides and 8 kite walls by the slot angle about the floor; a cone
 * (core diameter, thickness th) below the floor; the ceiling pieces of the
 * cone's top ring deleted (heal); a 0.002 in fillet on the mouth edges.
 * `top` is the head top (indent floor on indented hex heads).
 */
function recessTypeI(context is Context, id is Id, target is Query, def is map, top is ValueWithUnits)
{
    const L = def.sd;
    const W = def.b;
    const zf = top - (def.t - def.rth);
    const s = W / 2 + def.rcd / sqrt(2);
    const a = def.rfd / 2 / sqrt(2);
    // The outline, 5 points per quadrant: arm end, arm side to the kite,
    // kite apex, the next arm's side.
    var pts = [];
    for (var q = 0; q < 4; q += 1)
    {
        const c = cos(q * 90 * degree);
        const sn = sin(q * 90 * degree);
        for (var p in [vector(L / 2, -W / 2), vector(L / 2, W / 2), vector(s, W / 2), vector(a, a), vector(W / 2, s)])
            pts = append(pts, vector(p[0] * c - p[1] * sn, p[0] * sn + p[1] * c));
    }
    var plane = XY_PLANE;
    plane.origin[2] = zf;
    const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : plane });
    for (var i = 0; i < size(pts); i += 1)
        skLineSegment(sketch, "l" ~ i, { "start" : pts[i], "end" : pts[(i + 1) % size(pts)] });
    skSolve(sketch);
    opExtrude(context, id + "cross", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : (top - zf) * 1.1 });
    // The same outline below the floor: live's healed delete face extends
    // the walls down onto the cone, so the cone is cut only inside them.
    opExtrude(context, id + "below", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, 0, -1), "endBound" : BoundingType.BLIND, "endDepth" : def.rth * 2 });
    opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
    // The tool's walls drafted (a tool face points out of the void, so the
    // pull is reversed): the arm ends 26.3 degrees about the top, the arm
    // sides and kite walls by the slot angle about the floor; then one cut.
    var ends = [];
    var walls = [];
    for (var f in evaluateQuery(context, qGeometry(qOwnedByBody(qCreatedBy(id + "cross", EntityType.BODY), EntityType.FACE), GeometryType.PLANE)))
    {
        const pl = evPlane(context, { "face" : f });
        if (abs(pl.normal[2]) > 0.01)
            continue;
        if (abs(abs(dot(pl.origin, pl.normal)) - L / 2) < EPS)
            ends = append(ends, f);
        else
            walls = append(walls, f);
    }
    // The arm ends' neutral plane: through their corners on the head top.
    var topPlane = XY_PLANE;
    topPlane.origin[2] = headTopAt(def, sqrt(L * L + W * W) / 2, top);
    opPlane(context, id + "topPlane", { "plane" : topPlane });
    opDraft(context, id + "draftEnds", { "draftFaces" : qUnion(ends), "neutralPlane" : qCreatedBy(id + "topPlane", EntityType.FACE), "angle" : 26.3 * degree, "pullDirection" : vector(0, 0, -1), "tangentPropagation" : true });
    opPlane(context, id + "floorPlane", { "plane" : plane });
    opDraft(context, id + "draftWalls", { "draftFaces" : qUnion(walls), "neutralPlane" : qCreatedBy(id + "floorPlane", EntityType.FACE), "angle" : def.sa * degree, "pullDirection" : vector(0, 0, -1), "tangentPropagation" : true });
    opDeleteBodies(context, id + "deletePlanes", { "entities" : qUnion([qCreatedBy(id + "topPlane", EntityType.BODY), qCreatedBy(id + "floorPlane", EntityType.BODY)]) });
    subtract(context, id + "cut", target, qCreatedBy(id + "cross", EntityType.BODY));
    // The screw: every solid but this recess's tools.
    var screw = qUnion(evaluateQuery(context, qSubtraction(qBodyType(qEverything(EntityType.BODY), BodyType.SOLID), qCreatedBy(id, EntityType.BODY))));
    // The tool's walls drafted onto the same planes (a tool face points
    // out of the void, so the opposite pull).
    var below = [];
    for (var f in evaluateQuery(context, qGeometry(qOwnedByBody(qCreatedBy(id + "below", EntityType.BODY), EntityType.FACE), GeometryType.PLANE)))
    {
        const pl = evPlane(context, { "face" : f });
        if (abs(pl.normal[2]) < 0.01 && abs(abs(dot(pl.origin, pl.normal)) - L / 2) > EPS)
            below = append(below, f);
    }
    opPlane(context, id + "floorPlane2", { "plane" : plane });
    opDraft(context, id + "draftBelow", { "draftFaces" : qUnion(below), "neutralPlane" : qCreatedBy(id + "floorPlane2", EntityType.FACE), "angle" : def.sa * degree, "pullDirection" : vector(0, 0, -1), "tangentPropagation" : true });
    opDeleteBodies(context, id + "deletePlane2", { "entities" : qCreatedBy(id + "floorPlane2", EntityType.BODY) });
    // The cone's height: the Type I thickness (Pan: live's part uses the
    // Type IA one, rch).
    const coneH = def.rch > EPS ? def.rch : def.rth;
    revolveProfile(context, id + "core", [{ "p" : rz(Z, zf) }, { "p" : rz(def.rcore / 2, zf) }, { "p" : rz(Z, zf - coneH) }]);
    intersect(context, id + "coreIn", qCreatedBy(id + "core", EntityType.BODY), qCreatedBy(id + "below", EntityType.BODY));
    subtract(context, id + "coreCut", screw, qUnion(evaluateQuery(context, qUnion([qCreatedBy(id + "core", EntityType.BODY), qCreatedBy(id + "below", EntityType.BODY), qCreatedBy(id + "coreIn", EntityType.BODY)]))));
    // The 20 mouth edges (arm sides, ends, kites) at the top: 0.002 in.
    screw = qUnion(evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)));
    const reach = L / 2 * 1.01;
    var mouth = [];
    for (var e in evaluateQuery(context, qOwnedByBody(screw, EntityType.EDGE)))
    {
        const bounds = evBox3d(context, { "topology" : e });
        if (max([abs(bounds.minCorner[0]), abs(bounds.maxCorner[0]), abs(bounds.minCorner[1]), abs(bounds.maxCorner[1])]) < reach * 1.04 && bounds.minCorner[2] > zf + (top - zf) / 2)
            mouth = append(mouth, e);
    }
    if (size(mouth) > 0)
    {
        try silent
        {
            opFillet(context, id + "mouth", { "entities" : qUnion(mouth), "radius" : 0.002 * inch });
        }
    }
}

/** The drive recess cut into the head's top at z = top. */
function driveCut(context is Context, id is Id, target is Query, def is map, top is ValueWithUnits)
{
    const kind = def.drive;
    if (kind == DRIVE_NONE || def.t <= EPS)
        return;
    if (kind == DRIVE_CROSS && def.rcd > EPS)
    {
        // Indented hex heads: the recess starts on the indent floor (live).
        recessTypeI(context, id, target, def, (def.indent == 1 && def.head == HEAD_HEX) ? top - (def.k - def.c) / 6 : top);
        return;
    }
    const reach = max(def.dk, def.s) + def.d + def.sd;
    var tool = qNothing();
    if (kind == DRIVE_HEX_SOCKET)
    {
        const e = def.sd / cos(30 * degree);
        prismZ(context, id + "hex", top - def.t, def.t + reach, polygonPoints(6, def.sd, 0));
        // The drill point cone at the bottom of the socket (118 degrees).
        revolveProfile(context, id + "cone", [{ "p" : rz(Z, top - def.t - e / 2 * tan(31 * degree)) }, { "p" : rz(e / 2, top - def.t + EPS) }, { "p" : rz(Z, top - def.t + EPS) }]);
        tool = qCreatedBy(id, EntityType.BODY);
    }
    else if (kind == DRIVE_SLOT)
        tool = cuboid(context, id + "slot", vector(-reach, -def.sd / 2, top - def.t), vector(reach, def.sd / 2, top + reach));
    else if (kind == DRIVE_CROSS || kind == DRIVE_POZI)
    {
        // Two crossed wedge slots of width b inside a cone of diameter m
        // on the top narrowing to b at the depth t.
        const b = def.b > EPS ? def.b : def.sd / 5;
        const a = cuboid(context, id + "wingA", vector(-def.sd / 2, -b / 2, top - def.t), vector(def.sd / 2, b / 2, top + reach));
        const c = cuboid(context, id + "wingB", vector(-b / 2, -def.sd / 2, top - def.t), vector(b / 2, def.sd / 2, top + reach));
        unite(context, id + "cross", [a, c]);
        const wings = solidsOf(context, id);
        const cone = revolveProfile(context, id + "crossCone", [{ "p" : rz(Z, top - def.t) }, { "p" : rz(b / 2, top - def.t) }, { "p" : rz(def.sd / 2, top) }, { "p" : rz(def.sd / 2, top + reach) }, { "p" : rz(Z, top + reach) }]);
        intersect(context, id + "crossShape", wings, cone);
        tool = solidsOf(context, id);
    }
    else if (kind == DRIVE_HEXALOBULAR)
    {
        // Six lobes: arcs through the points diameter A between the
        // inner diameter B (sd = A, b = B).
        var p = XY_PLANE;
        p.origin[2] = top - def.t;
        const sketch = newSketchOnPlane(context, id + "lobeSketch", { "sketchPlane" : p });
        const ra = def.sd / 2;
        const rb = (def.b > EPS ? def.b : def.sd * 0.72) / 2;
        for (var i = 0; i < 6; i += 1)
        {
            const a0 = (i * 60 - 30) * degree;
            const a1 = (i * 60 + 30) * degree;
            const am = i * 60 * degree;
            skArc(sketch, "lobe" ~ i, { "start" : vector(rb * cos(a0), rb * sin(a0)), "mid" : vector(ra * cos(am), ra * sin(am)), "end" : vector(rb * cos(a1), rb * sin(a1)) });
        }
        skSolve(sketch);
        opExtrude(context, id + "lobes", { "entities" : qSketchRegion(id + "lobeSketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : def.t + reach });
        opDeleteBodies(context, id + "deleteLobeSketch", { "entities" : qCreatedBy(id + "lobeSketch", EntityType.BODY) });
        tool = qCreatedBy(id + "lobes", EntityType.BODY);
    }
    else if (kind == DRIVE_SQUARE_III || kind == DRIVE_TYPE_VI)
    {
        // The walls go down to where the 70 degree (140 included) cone
        // from the wall radius r reaches the depth t.
        const square = kind == DRIVE_SQUARE_III;
        const r = square ? def.sd / 2 : def.b / 2;
        const side = def.t - r * tan(20 * degree);
        if (square)
            prismZ(context, id + "square", top - side, side + reach, polygonPoints(4, def.sd, 0));
        else
        {
            // One closed spline through 12 points: lobe tips on Ø sd at
            // 90 + 60k degrees, the valleys on Ø b at 60 + 60k. It starts
            // at 0 degrees, on the crown's seam, so the mouth edge keeps a
            // single vertex there (kern: one fillet face, live's 15 faces).
            var p = XY_PLANE;
            p.origin[2] = top - side;
            const sketch = newSketchOnPlane(context, id + "lobeSketch", { "sketchPlane" : p });
            var pts = [];
            for (var k = 0; k < 12; k += 1)
            {
                const a = (30 * k) * degree;
                const rk = k % 2 == 0 ? def.b / 2 : def.sd / 2;
                pts = append(pts, vector(rk * cos(a), rk * sin(a)));
            }
            pts = append(pts, pts[0]);
            skFitSpline(sketch, "lobes", { "points" : pts });
            skSolve(sketch);
            opExtrude(context, id + "lobes", { "entities" : qSketchRegion(id + "lobeSketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : side + reach });
            opDeleteBodies(context, id + "deleteLobeSketch", { "entities" : qCreatedBy(id + "lobeSketch", EntityType.BODY) });
        }
        revolveProfile(context, id + "cone", [{ "p" : rz(Z, top - def.t) }, { "p" : rz(r, top - side) }, { "p" : rz(r, top + reach) }, { "p" : rz(Z, top + reach) }]);
        unite(context, id + "recess", [solidsOf(context, id)]);
        subtract(context, id + "cut", target, solidsOf(context, id));
        // The mouth: the edges around the recess that stay in the upper
        // half of its wall (the walls' vertical edges reach the bottom).
        const reachR = (square ? def.sd / 2 * sqrt(2) : def.sd / 2) * 1.05;
        // (The cut gives the screw a new identity: it is the one solid left.)
        const screw = qUnion(evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)));
        var mouth = [];
        for (var e in evaluateQuery(context, qOwnedByBody(screw, EntityType.EDGE)))
        {
            const bounds = evBox3d(context, { "topology" : e });
            if (max([abs(bounds.minCorner[0]), abs(bounds.maxCorner[0]), abs(bounds.minCorner[1]), abs(bounds.maxCorner[1])]) < reachR && bounds.minCorner[2] > top - side / 2)
                mouth = append(mouth, e);
        }
        // (OCCT can't make this 0.05 mm fillet yet; the part stays without it.)
        if (size(mouth) > 0)
        {
            try silent
            {
                opFillet(context, id + "mouth", { "entities" : qUnion(mouth), "radius" : 0.002 * inch });
            }
        }
        return;
    }
    subtract(context, id + "cut", target, tool);
}

/** Shank profile from z = top down to the point at -l (revolved). */
function shankProfile(def is map, top is ValueWithUnits) returns array
{
    const r = def.d / 2;
    const l = def.l;
    const rp = def.dp / 2;
    var pts = [{ "p" : rz(Z, top) }, { "p" : rz(r, top) }];
    const kind = def.point;
    if (kind == POINT_FLAT || (kind == POINT_CHAMFER && def.dp <= EPS))
        return join(pts, [{ "p" : rz(r, -l) }, { "p" : rz(Z, -l) }]);
    if (kind == POINT_CHAMFER)
        // 45 degrees down to the chamfer diameter dp.
        return join(pts, [{ "p" : rz(r, -l + (r - rp)) }, { "p" : rz(rp, -l) }, { "p" : rz(Z, -l) }]);
    if (kind == POINT_CONE || kind == POINT_BT || kind == POINT_TIP_D || kind == POINT_TIP_G || kind == POINT_TIP_F)
        // A cone of length z down to the tip diameter dp.
        return join(pts, [{ "p" : rz(r, -l + def.z) }, { "p" : rz(rp, -l) }, { "p" : rz(Z, -l) }]);
    if (kind == POINT_CONE_TIP)
        return join(pts, [{ "p" : rz(r, -l + def.z) }, { "p" : rz(Z, -l) }]);
    if (kind == POINT_CUP)
    {
        // 45 degree chamfer to the cup diameter dp; a 118 degree cup.
        const depth = rp * 0.6;
        return join(pts, [{ "p" : rz(r, -l + (r - rp)) }, { "p" : rz(rp, -l) }, { "p" : rz(Z, -l + depth) }]);
    }
    if (kind == POINT_DOG)
        // 45 degrees to the dog diameter dp, then the dog of length z.
        return join(pts, [{ "p" : rz(r, -l + def.z + (r - rp)) }, { "p" : rz(rp, -l + def.z) }, { "p" : rz(rp, -l) }, { "p" : rz(Z, -l) }]);
    if (kind == POINT_ROUND)
    {
        // A spherical end of height z.
        const h = def.z > EPS ? def.z : r;
        const big = (r * r + h * h) / (2 * h);
        const midR = r / 2;
        return join(pts, [{ "p" : rz(r, -l + h) }, { "p" : rz(Z, -l), "mid" : rz(midR, -l + big - sqrt(big * big - midR * midR)) }]);
    }
    return join(pts, [{ "p" : rz(r, -l) }, { "p" : rz(Z, -l) }]);
}

/** Neck under the head, from z = 0 down; returns [body, its bottom]. */
function screwNeck(context is Context, id is Id, def is map, bottom is ValueWithUnits) returns array
{
    const kind = def.neck;
    if (kind == NECK_SQUARE)
        return [prismZ(context, id + "square", bottom - def.nv, def.nv + EPS, polygonPoints(4, def.nf, 0)), bottom - def.nv];
    if (kind == NECK_RIBBED)
    {
        // ASME B18.5 ribbed neck: after a plain length M (ng, bolts up to
        // lv1 long; ng2 longer) come `nribs` ribs of 90 degrees over the
        // body d up to the diameter nf, P long (nv up to lv1, nv2 up to
        // lv2, nv3 longer).
        const short = def.l <= def.lv1 + EPS;
        const p = short ? def.nv : (def.l <= def.lv2 + EPS ? def.nv2 : def.nv3);
        const top = bottom - (short ? def.ng : def.ng2);
        const tip = def.nf / 2;
        const rin = def.d / 2 - (tip - def.d / 2);
        const w = tip - rin;
        const rib = prismZ(context, id + "rib", top - p, p, [vector(rin, -w), vector(tip, Z), vector(rin, w)]);
        var transforms = [];
        var names = [];
        for (var i = 1; i < def.nribs; i += 1)
        {
            transforms = append(transforms, rotationAround(line(vector(Z, Z, Z), vector(0, 0, 1)), i * 360 / def.nribs * degree));
            names = append(names, "r" ~ i);
        }
        if (size(transforms) > 0)
            opPattern(context, id + "ribs", { "entities" : rib, "transforms" : transforms, "instanceNames" : names });
        return [qCreatedBy(id, EntityType.BODY), bottom];
    }
    if (kind == NECK_FIN)
    {
        const fins = cuboid(context, id + "fins", vector(-def.nf / 2, -def.d / 4, bottom - def.nv), vector(def.nf / 2, def.d / 4, bottom + EPS));
        return [fins, bottom];
    }
    if (kind == NECK_SHOULDER || kind == NECK_COLLAR)
        return [cylinderZ(context, id + "shoulder", bottom - def.ls, bottom + EPS, def.ds / 2), bottom - def.ls];
    return [qNothing(), bottom];
}

annotation { "Feature Type Name" : "Screw" }
export const screw = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Head" }
        isInteger(definition.head, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Drive" }
        isInteger(definition.drive, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Neck" }
        isInteger(definition.neck, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Point" }
        isInteger(definition.point, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "d" }
        isLength(definition.d, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dk" }
        isLength(definition.dk, LENGTH_0);
        annotation { "Name" : "k" }
        isLength(definition.k, LENGTH_0);
        annotation { "Name" : "s" }
        isLength(definition.s, LENGTH_0);
        annotation { "Name" : "dw" }
        isLength(definition.dw, LENGTH_0);
        annotation { "Name" : "dc" }
        isLength(definition.dc, LENGTH_0);
        annotation { "Name" : "c" }
        isLength(definition.c, LENGTH_0);
        annotation { "Name" : "rf" }
        isLength(definition.rf, LENGTH_0);
        annotation { "Name" : "Head edge fillet" }
        isLength(definition.re, LENGTH_0);
        annotation { "Name" : "Undercut head" }
        isInteger(definition.undercut, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Indented head" }
        isInteger(definition.indent, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Type I corner distance" }
        isLength(definition.rcd, LENGTH_0);
        annotation { "Name" : "Type I face distance" }
        isLength(definition.rfd, LENGTH_0);
        annotation { "Name" : "Type I core diameter" }
        isLength(definition.rcore, LENGTH_0);
        annotation { "Name" : "Type I core thickness" }
        isLength(definition.rth, LENGTH_0);
        annotation { "Name" : "Type I cone height" }
        isLength(definition.rch, LENGTH_0);
        annotation { "Name" : "Slot angle" }
        isReal(definition.sa, { (unitless) : [0, 0, 90] } as RealBoundSpec);
        annotation { "Name" : "f" }
        isLength(definition.f, LENGTH_0);
        annotation { "Name" : "e1" }
        isLength(definition.e1, LENGTH_0);
        annotation { "Name" : "e2" }
        isLength(definition.e2, LENGTH_0);
        annotation { "Name" : "sd" }
        isLength(definition.sd, LENGTH_0);
        annotation { "Name" : "b" }
        isLength(definition.b, LENGTH_0);
        annotation { "Name" : "t" }
        isLength(definition.t, LENGTH_0);
        annotation { "Name" : "nf" }
        isLength(definition.nf, LENGTH_0);
        annotation { "Name" : "nv" }
        isLength(definition.nv, LENGTH_0);
        annotation { "Name" : "ds" }
        isLength(definition.ds, LENGTH_0);
        annotation { "Name" : "ls" }
        isLength(definition.ls, LENGTH_0);
        annotation { "Name" : "dp" }
        isLength(definition.dp, LENGTH_0);
        annotation { "Name" : "z" }
        isLength(definition.z, LENGTH_0);
        annotation { "Name" : "Thread under the shoulder" }
        isLength(definition.tu, LENGTH_0);
        annotation { "Name" : "Ribs" }
        isInteger(definition.nribs, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Rib length, longer bolts" }
        isLength(definition.nv2, LENGTH_0);
        annotation { "Name" : "Rib length, longest bolts" }
        isLength(definition.nv3, LENGTH_0);
        annotation { "Name" : "Gap to the ribs" }
        isLength(definition.ng, LENGTH_0);
        annotation { "Name" : "Gap to the ribs, longer bolts" }
        isLength(definition.ng2, LENGTH_0);
        annotation { "Name" : "Short bolts up to" }
        isLength(definition.lv1, LENGTH_0);
        annotation { "Name" : "Longer bolts up to" }
        isLength(definition.lv2, LENGTH_0);
        annotation { "Name" : "Socket depth of short screws" }
        isLength(definition.ts, LENGTH_0);
        annotation { "Name" : "Shorter than this, ts" }
        isLength(definition.lts, LENGTH_0);
        annotation { "Name" : "Dog length of short screws" }
        isLength(definition.zs, LENGTH_0);
    }
    {
        var def = definition;
        // Shoulder screws: the Length is the shoulder's, the thread (tu)
        // goes on below it.
        if (def.neck == NECK_SHOULDER)
        {
            def.ls = def.l;
            def.l = def.l + def.tu;
        }
        // Short set screws take the shallower socket of the standard (ts)
        // and, dog points, the shorter dog (zs; ISO 4028 short lengths).
        if (def.ts > EPS && def.l < def.lts - EPS)
            def.t = def.ts;
        if (def.zs > EPS && def.l < def.lts - EPS)
            def.z = def.zs;
        const head = screwHead(context, id + "head", def);
        // Countersunk heads reach down to -k; others sit on z = 0.
        const below = (def.head == HEAD_COUNTERSUNK || def.head == HEAD_RAISED_COUNTERSUNK) ? -def.k : Z;
        const neck = screwNeck(context, id + "neck", def, below);
        // The shank starts well inside the neck or head (the union then
        // never depends on touching faces, even on miniature screws).
        var inside = Z;
        if (neck[1] < below - EPS)
            inside = neck[1] + (below - neck[1]) / 2;
        else if (def.head != HEAD_NONE)
            // In the flange or collar (c) under the head, else in the head.
            inside = def.c > EPS && below == Z ? def.c / 2 : below + (head[1] - below) / 4;
        revolveProfile(context, id + "shank", shankProfile(def, inside));
        uniteAll(context, id);
        // Live's indented hex heads: a flat dish of diameter 0.9 s, a sixth
        // of the hex height deep, before any slot or recess.
        if (def.indent == 1 && def.head == HEAD_HEX)
        {
            const screw = solidsOf(context, id);
            cylinderZ(context, id + "indentTool", head[1] - (def.k - def.c) / 6, head[1] + def.k, def.s * 0.45);
            subtract(context, id + "indent", screw, qCreatedBy(id + "indentTool", EntityType.BODY));
        }
        driveCut(context, id + "drive", solidsOf(context, id), def, def.head == HEAD_NONE ? Z : head[1]);
        if (def.point == POINT_BT)
            btSlot(context, id + "bt", solidsOf(context, id), def);
        else if (def.point == POINT_TIP_D || def.point == POINT_TIP_G || def.point == POINT_TIP_F)
            tipSlots(context, id + "tipSlots", solidsOf(context, id), def);
    });

/**
 * Type BT / T cutting slot (as Onshape's component: a sketch revolved 90
 * degrees, cut). In the axis plane, from the tip: up the axis to the top of
 * the taper (L = z), an arc centred on the axis down to the basic diameter
 * line at L/2, back down to the tip; revolved over the x < 0, y < 0
 * quadrant. The arc's centre is (3L^2 - D^2) / (4L) above the tip, its
 * radius L minus that.
 */
function btSlot(context is Context, id is Id, target is Query, def is map)
{
    const L = def.z;
    const r = def.d / 2;
    const tip = -def.l;
    const c = (3 * L * L - def.d * def.d) / (4 * L);
    const R = L - c;
    const a1 = atan2(L / 2 - c, r);
    const am = (a1 + 90 * degree) / 2;
    const ring = revolveProfile(context, id + "ring", [{ "p" : rz(Z, tip) }, { "p" : rz(Z, tip + L) }, { "p" : rz(r, tip + L / 2), "mid" : rz(R * cos(am), tip + c + R * sin(am)) }, { "p" : rz(r, tip) }]);
    const big = def.d * 2;
    const quadrant = cuboid(context, id + "quadrant", vector(-big, -big, tip - big), vector(Z, Z, tip + L + big));
    intersect(context, id + "slot", ring, quadrant);
    subtract(context, id + "cut", target, solidsOf(context, id));
}

/**
 * Type D / G slot and Type F flutes (Onshape's component sketches on the
 * Front plane, cut Through all): width D / 11 over the taper length from
 * the tip. D: one slot centred on the axis through the y < 0 radius; G: the
 * same through the full diameter; F: two flutes through the y < 0 half,
 * their walls tilted 7 degrees (leaning to -x going up), one's edge on the
 * axis at the tip and the other's outer edge at the point radius.
 */
function tipSlots(context is Context, id is Id, target is Query, def is map)
{
    const w = def.d / 11;
    const tip = -def.l;
    const L = def.z;
    const big = def.d * 2;
    if (def.point == POINT_TIP_D || def.point == POINT_TIP_G)
        cuboid(context, id + "slot", vector(-w / 2, -big, tip - big), vector(w / 2, def.point == POINT_TIP_G ? big : Z, tip + L));
    else
    {
        const lean = L * tan(7 * degree);
        for (var k = 0; k < 2; k += 1)
        {
            const x0 = k == 0 ? -w : def.dp / 2 - w;
            const sketch = newSketchOnPlane(context, id + ("flute" ~ k), { "sketchPlane" : profilePlane() });
            // Below the tip the flute only cuts air: start well under it.
            skPolyline(sketch, "outline", { "points" : [rz(x0 + big * tan(7 * degree), tip - big), rz(x0 + w + big * tan(7 * degree), tip - big), rz(x0 + w - lean, tip + L), rz(x0 - lean, tip + L), rz(x0 + big * tan(7 * degree), tip - big)] });
            skSolve(sketch);
            opExtrude(context, id + ("fluteCut" ~ k), { "entities" : qSketchRegion(id + ("flute" ~ k)), "direction" : vector(0, -1, 0), "endBound" : BoundingType.BLIND, "endDepth" : big });
            opDeleteBodies(context, id + ("deleteFlute" ~ k), { "entities" : qCreatedBy(id + ("flute" ~ k), EntityType.BODY) });
        }
    }
    subtract(context, id + "cut", target, solidsOf(context, id));
}

// Nuts (definition.form).
const NUT_HEX = 0;
const NUT_SQUARE = 1;
const NUT_SLOTTED = 2;
const NUT_CASTLE = 3;
const NUT_CAP = 4;
const NUT_INSERT = 5;
const NUT_WELD = 6;

annotation { "Feature Type Name" : "Nut" }
export const nut = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Chamfer both faces" }
        isInteger(definition.both, { (unitless) : [0, 1, 1] } as IntegerBoundSpec);
        annotation { "Name" : "d" }
        isLength(definition.d, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "m" }
        isLength(definition.m, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "h" }
        isLength(definition.h, LENGTH_0);
        annotation { "Name" : "dw" }
        isLength(definition.dw, LENGTH_0);
        annotation { "Name" : "dc" }
        isLength(definition.dc, LENGTH_0);
        annotation { "Name" : "c" }
        isLength(definition.c, LENGTH_0);
        annotation { "Name" : "n" }
        isLength(definition.n, LENGTH_0);
        annotation { "Name" : "w" }
        isLength(definition.w, LENGTH_0);
        annotation { "Name" : "de" }
        isLength(definition.de, LENGTH_0);
    }
    {
        const form = definition.form;
        const sides = form == NUT_SQUARE || form == NUT_WELD ? 4 : 6;
        // Flange (dc, c) under the body; the body from c to m.
        const z0 = definition.c;
        // A castle nut's hex ends at w and its crown (de) goes up to m.
        const bodyTop = (form == NUT_CASTLE || form == NUT_CAP || form == NUT_INSERT) && definition.w > EPS ? definition.w : definition.m;
        const body = prismZ(context, id + "body", z0, bodyTop - z0, polygonPoints(sides, definition.s, 0));
        const dw = definition.dw > EPS ? definition.dw : definition.s;
        cornerChamfer(context, id + "chamfer", body, z0, bodyTop - z0, dw, true, definition.both == 1 && z0 <= EPS);
        if (z0 > EPS)
            cylinderZ(context, id + "flange", Z, z0 + EPS, definition.dc / 2);
        if (form == NUT_CASTLE)
            cylinderZ(context, id + "crown", bodyTop - EPS, definition.m, definition.de / 2);
        if (form == NUT_CAP)
            // The domed cap (diameter de) from w up to the overall height m.
            revolveProfile(context, id + "cap", roundHead(definition.de / 2, definition.m - bodyTop + EPS, definition.de / 2, bodyTop - EPS));
        if (form == NUT_INSERT)
            // The collar holding the insert (diameter de) up to h... m.
            cylinderZ(context, id + "collar", bodyTop - EPS, definition.m, definition.de / 2);
        if (form == NUT_WELD && definition.h > EPS)
            // The pilot (diameter de, height h) under a weld nut.
            cylinderZ(context, id + "pilot", -definition.h, EPS, definition.de / 2);
        uniteAll(context, id);
        // The hole (blind under a cap nut).
        const holeTop = form == NUT_CAP ? definition.m - (definition.m - bodyTop) / 2 : definition.m + definition.m;
        var nutBody = solidsOf(context, id);
        subtract(context, id + "holeCut", nutBody, cylinderZ(context, id + "hole", -definition.m - definition.h, holeTop, definition.d / 2));
        if ((form == NUT_SLOTTED || form == NUT_CASTLE) && definition.n > EPS)
        {
            nutBody = solidsOf(context, id);
            const reach = definition.s * 2;
            const a = cuboid(context, id + "slotA", vector(-reach, -definition.n / 2, definition.w), vector(reach, definition.n / 2, definition.m + reach));
            opPattern(context, id + "slots", { "entities" : a, "transforms" : [rotationAround(line(vector(Z, Z, Z), vector(0, 0, 1)), 60 * degree), rotationAround(line(vector(Z, Z, Z), vector(0, 0, 1)), 120 * degree)], "instanceNames" : ["b", "c"] });
            subtract(context, id + "slotCut", nutBody, qUnion([a, qCreatedBy(id + "slots", EntityType.BODY)]));
        }
    });

// Washers (definition.form).
const WASHER_PLAIN = 0;
const WASHER_CHAMFERED = 1;
const WASHER_SPLIT = 2;
const WASHER_EXTERNAL_TEETH = 3;
const WASHER_INTERNAL_TEETH = 4;
const WASHER_COUNTERSUNK_TEETH = 5;
const WASHER_CONICAL = 6;
const WASHER_SQUARE = 7;

annotation { "Feature Type Name" : "Washer" }
export const washer = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Teeth" }
        isInteger(definition.teeth, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "d1" }
        isLength(definition.d1, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "d2" }
        isLength(definition.d2, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "h" }
        isLength(definition.h, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, LENGTH_0);
    }
    {
        const form = definition.form;
        const r1 = definition.d1 / 2;
        const r2 = definition.d2 / 2;
        const h = definition.h;
        if (form == WASHER_CONICAL)
        {
            // A cone frustum of material thickness s, overall height h.
            const s = definition.s;
            revolveProfile(context, id + "washer", [{ "p" : rz(r1, h - s) }, { "p" : rz(r2, Z) }, { "p" : rz(r2, s) }, { "p" : rz(r1, h) }]);
            return;
        }
        var body;
        if (form == WASHER_SQUARE)
        {
            body = prismZ(context, id + "square", Z, h, polygonPoints(4, definition.d2, 0));
            subtract(context, id + "holeCut", body, cylinderZ(context, id + "hole", -h, 2 * h, r1));
            return;
        }
        if (form == WASHER_CHAMFERED)
            // ISO 7090: the outer edge chamfered at 30 degrees to h / 4 .. h / 2.
            body = revolveProfile(context, id + "washer", [{ "p" : rz(r1, Z) }, { "p" : rz(r2, Z) }, { "p" : rz(r2, h / 2) }, { "p" : rz(r2 - h / 2 * tan(60 * degree) / 2, h) }, { "p" : rz(r1, h) }]);
        else if (form == WASHER_COUNTERSUNK_TEETH)
            // A cone of 82 degrees from d2 down to d1.
        {
            // An 82 degree cone shell, the large end up, h thick.
            const rise = (r2 - r1) / tan(41 * degree);
            const v = h / sin(41 * degree);
            body = revolveProfile(context, id + "washer", [{ "p" : rz(r1, Z) }, { "p" : rz(r2, rise) }, { "p" : rz(r2, rise + v) }, { "p" : rz(r1, v) }]);
        }
        else
            body = revolveProfile(context, id + "washer", [{ "p" : rz(r1, Z) }, { "p" : rz(r2, Z) }, { "p" : rz(r2, h) }, { "p" : rz(r1, h) }]);
        if (form == WASHER_SPLIT)
            // The split: a gap as wide as the section is thick.
            subtract(context, id + "split", body, cuboid(context, id + "gap", vector(r1 - h, -h / 2, -h), vector(r2 + h, h / 2, 2 * h)));
        if (form == WASHER_EXTERNAL_TEETH || form == WASHER_INTERNAL_TEETH || form == WASHER_COUNTERSUNK_TEETH)
        {
            // Radial slots between the teeth, from the toothed edge to 60 % of the section.
            const n = definition.teeth > 0 ? definition.teeth : 9;
            const w = (r2 - r1) * 0.6;
            const gap = PI * (form == WASHER_INTERNAL_TEETH ? definition.d1 : definition.d2) / n / 3;
            const x0 = form == WASHER_INTERNAL_TEETH ? r1 - h : r2 - w;
            const x1 = form == WASHER_INTERNAL_TEETH ? r1 + w : r2 + h;
            const zTop = form == WASHER_COUNTERSUNK_TEETH ? (r2 - r1) / tan(41 * degree) + 2 * h / sin(41 * degree) : 2 * h;
            const slot = cuboid(context, id + "slot", vector(x0, -gap / 2, -h), vector(x1, gap / 2, zTop));
            var transforms = [];
            var names = [];
            for (var i = 1; i < n; i += 1)
            {
                transforms = append(transforms, rotationAround(line(vector(Z, Z, Z), vector(0, 0, 1)), i * 360 / n * degree));
                names = append(names, "t" ~ i);
            }
            opPattern(context, id + "slots", { "entities" : slot, "transforms" : transforms, "instanceNames" : names });
            subtract(context, id + "teeth", body, qUnion([slot, qCreatedBy(id + "slots", EntityType.BODY)]));
        }
    });

// Pins (definition.form).
const PIN_PARALLEL = 0;
const PIN_DOWEL = 1;
const PIN_TAPER = 2;
const PIN_SLOTTED = 3;
const PIN_COILED = 4;
const PIN_COTTER = 5;
const PIN_CLEVIS = 6;
const PIN_STUD = 7;
const PIN_DRIVE_STUD = 8;
const PIN_T_HEAD = 9;

annotation { "Feature Type Name" : "Pin" }
export const pin = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "d" }
        isLength(definition.d, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "c" }
        isLength(definition.c, LENGTH_0);
        annotation { "Name" : "r" }
        isLength(definition.r, LENGTH_0);
        annotation { "Name" : "s" }
        isLength(definition.s, LENGTH_0);
        annotation { "Name" : "dk" }
        isLength(definition.dk, LENGTH_0);
        annotation { "Name" : "k" }
        isLength(definition.k, LENGTH_0);
        annotation { "Name" : "d1" }
        isLength(definition.d1, LENGTH_0);
        annotation { "Name" : "le" }
        isLength(definition.le, LENGTH_0);
        annotation { "Name" : "Taper (1 : n on the diameter)" }
        isReal(definition.taper, { (unitless) : [1, 50, 1000] } as RealBoundSpec);
    }
    {
        const form = definition.form;
        const r = definition.d / 2;
        const l = definition.l;
        const c = min(definition.c, r * 0.9);
        if (form == PIN_TAPER)
        {
            // d at the large end, the diameter shrinking 1 : taper (ASME
            // B18.8.2 1/4 in per foot = 1 : 48, ISO 2339 1 : 50).
            const small = r - l / definition.taper / 2;
            revolveProfile(context, id + "pin", [{ "p" : rz(Z, Z) }, { "p" : rz(r, Z) }, { "p" : rz(small, -l) }, { "p" : rz(Z, -l) }]);
            return;
        }
        if (form == PIN_COTTER)
        {
            // Two half-round legs (d) side by side and the eye over them (r = eye radius).
            const legs = cylinderZ(context, id + "legA", -l, Z, r / 2);
            const legB = cylinderZ(context, id + "legB", -l + c, Z, r / 2);
            opTransform(context, id + "moveA", { "bodies" : legs, "transform" : transform(vector(-r / 2, Z, Z)) });
            opTransform(context, id + "moveB", { "bodies" : legB, "transform" : transform(vector(r / 2, Z, Z)) });
            const eye = revolveProfile(context, id + "eye", [{ "p" : rz(Z, Z) }, { "p" : rz(definition.r + r / 2, Z) }, { "p" : rz(definition.r + r / 2, r) }, { "p" : rz(Z, r) }]);
            subtract(context, id + "eyeHole", eye, cylinderZ(context, id + "eyeCut", -r, 2 * r, max(definition.r - r / 2, EPS)));
            uniteAll(context, id);
            return;
        }
        var pts = [{ "p" : rz(Z, Z) }];
        const headed = form == PIN_CLEVIS || form == PIN_DRIVE_STUD || form == PIN_T_HEAD;
        if ((form == PIN_DOWEL || form == PIN_PARALLEL) && definition.r > EPS)
        {
            // Dowel: one end rounded (r), the other chamfered (c); a
            // parallel pin with r: both ends rounded (production dowels).
            const rr = definition.r;
            const m = rr * (1 - 1 / sqrt(2));
            const top = form == PIN_DOWEL ? [{ "p" : rz(r - c, Z) }, { "p" : rz(r, -c) }] : [{ "p" : rz(r - rr, Z) }, { "p" : rz(r, -rr), "mid" : rz(r - m, -m) }];
            pts = join(join([{ "p" : rz(Z, Z) }], top), [{ "p" : rz(r, -l + rr) }, { "p" : rz(r - rr, -l), "mid" : rz(r - m, -l + m) }, { "p" : rz(Z, -l) }]);
        }
        else
        {
            const top = headed ? [{ "p" : rz(r, Z) }] : [{ "p" : rz(r - c, Z) }, { "p" : rz(r, -c) }];
            pts = join(pts, join(top, [{ "p" : rz(r, -l + c) }, { "p" : rz(r - c, -l) }, { "p" : rz(Z, -l) }]));
        }
        revolveProfile(context, id + "pin", pts);
        if (headed)
        {
            // A flat head (clevis pin), a round head (drive stud), both dk
            // across and k high, or a T head dk long, s wide, k high.
            const head = form == PIN_T_HEAD
                ? prismZ(context, id + "tHead", -EPS, definition.k + EPS, [vector(-definition.dk / 2, -definition.s / 2), vector(definition.dk / 2, -definition.s / 2), vector(definition.dk / 2, definition.s / 2), vector(-definition.dk / 2, definition.s / 2)])
                : revolveProfile(context, id + "head", roundHead(definition.dk / 2, definition.k, form == PIN_DRIVE_STUD ? definition.dk : Z, -EPS));
            uniteAll(context, id);
        }
        if (form == PIN_SLOTTED || form == PIN_COILED)
        {
            // A tube of wall s; a slotted pin's slot as wide as its wall.
            var body = solidsOf(context, id);
            subtract(context, id + "bore", body, cylinderZ(context, id + "boreTool", -l - r, r, r - definition.s));
            if (form == PIN_SLOTTED)
            {
                body = solidsOf(context, id);
                subtract(context, id + "slot", body, cuboid(context, id + "slotTool", vector(r - 2 * definition.s, -definition.s / 2, -l - r), vector(r + definition.s, definition.s / 2, r)));
            }
        }
        if (form == PIN_CLEVIS && definition.d1 > EPS)
        {
            // The cross hole (d1) at le from the head.
            const body = solidsOf(context, id);
            const hole = cylinderZ(context, id + "crossHole", -r * 2, r * 2, definition.d1 / 2);
            opTransform(context, id + "turnHole", { "bodies" : hole, "transform" : transform(vector(Z, Z, -definition.le)) * rotationAround(line(vector(Z, Z, Z), vector(1, 0, 0)), 90 * degree) });
            subtract(context, id + "holeCut", body, hole);
        }
    });

// Keys (definition.form).
const KEY_ROUND_ENDS = 0;
const KEY_SQUARE_ENDS = 1;
const KEY_WOODRUFF = 2;
const KEY_TAPER = 3;
const KEY_GIB_HEAD = 4;

annotation { "Feature Type Name" : "Key" }
export const key = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "b" }
        isLength(definition.b, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "h" }
        isLength(definition.h, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "h1" }
        isLength(definition.h1, LENGTH_0);
        annotation { "Name" : "d1" }
        isLength(definition.d1, LENGTH_0);
    }
    {
        const form = definition.form;
        const b = definition.b;
        const h = definition.h;
        const l = definition.l;
        if (form == KEY_WOODRUFF)
        {
            // A disc segment of diameter d1 and height h, b thick (along Y).
            const r = definition.d1 / 2;
            const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : plane(vector(Z, b / 2, Z), vector(0, -1, 0), vector(1, 0, 0)) });
            const half = sqrt(max(r * r - (r - h) * (r - h), Z * Z));
            skLineSegment(sketch, "top", { "start" : vector(-half, h), "end" : vector(half, h) });
            skArc(sketch, "arc", { "start" : vector(half, h), "mid" : vector(Z, Z), "end" : vector(-half, h) });
            skSolve(sketch);
            opExtrude(context, id + "key", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, -1, 0), "endBound" : BoundingType.BLIND, "endDepth" : b });
            opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
            return;
        }
        const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : XY_PLANE });
        if (form == KEY_ROUND_ENDS)
        {
            skLineSegment(sketch, "s1", { "start" : vector(-l / 2 + b / 2, -b / 2), "end" : vector(l / 2 - b / 2, -b / 2) });
            skArc(sketch, "e1", { "start" : vector(l / 2 - b / 2, -b / 2), "mid" : vector(l / 2, Z), "end" : vector(l / 2 - b / 2, b / 2) });
            skLineSegment(sketch, "s2", { "start" : vector(l / 2 - b / 2, b / 2), "end" : vector(-l / 2 + b / 2, b / 2) });
            skArc(sketch, "e2", { "start" : vector(-l / 2 + b / 2, b / 2), "mid" : vector(-l / 2, Z), "end" : vector(-l / 2 + b / 2, -b / 2) });
        }
        else
            skPolyline(sketch, "outline", { "points" : [vector(-l / 2, -b / 2), vector(l / 2, -b / 2), vector(l / 2, b / 2), vector(-l / 2, b / 2), vector(-l / 2, -b / 2)] });
        skSolve(sketch);
        opExtrude(context, id + "key", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : h });
        opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
        const body = qCreatedBy(id + "key", EntityType.BODY);
        if (form == KEY_TAPER || form == KEY_GIB_HEAD)
        {
            // Taper 1:100 on the top face, h at the thick (head) end (-X).
            const cut = cuboid(context, id + "taperTool", vector(-l, -b, h), vector(l, b, h + b));
            opTransform(context, id + "tilt", { "bodies" : cut, "transform" : rotationAround(line(vector(-l / 2, Z, h), vector(0, 1, 0)), atan(0.01)) });
            subtract(context, id + "taperCut", body, cut);
        }
        if (form == KEY_GIB_HEAD && definition.h1 > EPS)
        {
            // The gib head (height h1, as long as it is high) at the thick end.
            cuboid(context, id + "head", vector(-l / 2 - definition.h1, -b / 2, Z), vector(-l / 2 + EPS, b / 2, definition.h1));
            uniteAll(context, id);
        }
    });

annotation { "Feature Type Name" : "Retaining ring" }
export const retainingRing = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Internal" }
        isInteger(definition.internal, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "d3" }
        isLength(definition.d3, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "a" }
        isLength(definition.a, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "b" }
        isLength(definition.b, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "d5" }
        isLength(definition.d5, LENGTH_0);
    }
    {
        // DIN 471 / 472: a ring of free diameter d3 (inner for shafts, outer
        // for bores), section b opposite the opening narrowing toward the
        // lugs, two lugs (a high, with holes d5) at the opening.
        const internal = definition.internal == 1;
        const r3 = definition.d3 / 2;
        const b = definition.b;
        const a = definition.a;
        // The ring's other edge: eccentric so the section is b at the back
        // and b / 2 at the lugs.
        const rOther = internal ? r3 - b * 0.75 : r3 + b * 0.75;
        const shift = b * 0.25;
        var p = XY_PLANE;
        const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : p });
        skCircle(sketch, "free", { "center" : vector(Z, Z), "radius" : r3 });
        skCircle(sketch, "other", { "center" : vector(internal ? shift : -shift, Z), "radius" : rOther });
        skSolve(sketch);
        opExtrude(context, id + "ring", { "entities" : qSketchRegion(id + "sketch", true), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : definition.s });
        opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
        const ring = qCreatedBy(id + "ring", EntityType.BODY);
        // The opening at +X between the lugs, and the lugs beside it.
        const gap = a * 0.5;
        const rOut = internal ? r3 : rOther;
        const rIn = internal ? rOther : r3;
        subtract(context, id + "opening", ring, cuboid(context, id + "gap", vector(Z, -gap / 2, -definition.s), vector(rOut + a, gap / 2, 2 * definition.s)));
        const lugX0 = internal ? rOut - a : rIn - EPS;
        const lugX1 = internal ? rIn + EPS : rIn + a;
        cuboid(context, id + "lugA", vector(min(lugX0, lugX1), gap / 2, Z), vector(max(lugX0, lugX1), gap / 2 + a, definition.s));
        cuboid(context, id + "lugB", vector(min(lugX0, lugX1), -gap / 2 - a, Z), vector(max(lugX0, lugX1), -gap / 2, definition.s));
        uniteAll(context, id);
        if (definition.d5 > EPS)
        {
            const ringBody = solidsOf(context, id);
            const hx = (lugX0 + lugX1) / 2;
            const hA = cylinderZ(context, id + "holeA", -definition.s, 2 * definition.s, definition.d5 / 2);
            opTransform(context, id + "moveHoleA", { "bodies" : hA, "transform" : transform(vector(hx, gap / 2 + a / 2, Z)) });
            const hB = cylinderZ(context, id + "holeB", -definition.s, 2 * definition.s, definition.d5 / 2);
            opTransform(context, id + "moveHoleB", { "bodies" : hB, "transform" : transform(vector(hx, -gap / 2 - a / 2, Z)) });
            subtract(context, id + "holes", ringBody, qUnion([hA, hB]));
        }
    });

annotation { "Feature Type Name" : "O-ring" }
export const oRing = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "d1" }
        isLength(definition.d1, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "d2" }
        isLength(definition.d2, NONNEGATIVE_LENGTH_BOUNDS);
    }
    {
        // Inner diameter d1, cross-section d2, lying on z = 0.
        const r = definition.d2 / 2;
        const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : profilePlane() });
        skCircle(sketch, "section", { "center" : vector(definition.d1 / 2 + r, r), "radius" : r });
        skSolve(sketch);
        opRevolve(context, id + "ring", { "entities" : qSketchRegion(id + "sketch"), "axis" : line(vector(Z, Z, Z), vector(0, 0, 1)), "angleForward" : 2 * PI * radian });
        opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
    });
