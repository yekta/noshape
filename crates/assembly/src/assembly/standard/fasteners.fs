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

/**
 * A closed profile (2D points) with every corner rounded `rho`: segments for
 * `revolveProfile` (revolved arcs make exact tori).
 */
function roundedProfile(points is array, rho is ValueWithUnits) returns array
{
    var out = [];
    const n = size(points);
    for (var i = 0; i < n; i += 1)
    {
        const p = points[i];
        const u = normalize(points[(i + n - 1) % n] - p);
        const v = normalize(points[(i + 1) % n] - p);
        const half = acos(dot(u, v)) / 2;
        const t = rho / tan(half);
        const bis = normalize(u + v);
        out = append(out, { "p" : p + u * t });
        out = append(out, { "p" : p + v * t, "mid" : p + bis * (rho / sin(half) - rho) });
    }
    return out;
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
    cornerChamferAngle(context, id, body, z0, h, dw, dw, top, bottom, 30 * degree);
}

/**
 * `cornerChamfer` at `angle` from the face (30 degrees hex, 25 square),
 * starting on the circle dwTop on the top face (a conical top when small).
 */
function cornerChamferAngle(context is Context, id is Id, body is Query, z0 is ValueWithUnits, h is ValueWithUnits, dw is ValueWithUnits, dwTop is ValueWithUnits, top is boolean, bottom is boolean, angle is ValueWithUnits)
{
    if (!top && !bottom)
        return;
    const slope = 1 / tan(angle);
    const big = dw * 10 + h * 10;
    const r0 = bottom ? dw / 2 : big;
    const r1 = top ? dwTop / 2 : big;
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
 * Live's ISO 4762 under-head transition (behaviours "skb (10-25) ISO 4762
 * / ISO 10642 tables"): a cone from radius `ra` at the bearing face (z = 0)
 * to the shank radius `r` at z = -lx, filleted by `ru` against the bearing
 * face and against the shank. The region reaches into the head and the
 * shank so the union never rests on touching faces.
 */
function underTransition(context is Context, id is Id, r is ValueWithUnits, ra is ValueWithUnits, lx is ValueWithUnits, ru is ValueWithUnits)
{
    const dr = r - ra;
    const len = sqrt(dr * dr + lx * lx);
    // The cone's outward normal (r, z).
    const nr = lx / len;
    const nz = dr / len;
    const up = lx / 2;
    if (ru <= EPS)
    {
        revolveProfile(context, id, [{ "p" : rz(r / 2, up) }, { "p" : rz(ra, up) }, { "p" : rz(ra, Z) }, { "p" : rz(r, -lx) }, { "p" : rz(r / 2, -lx) }]);
        return;
    }
    // Fillet centres: against the bearing face (z = -ru) and the shank (r + ru).
    const c1r = ra + ru * (len + dr) / lx;
    const c1z = -ru;
    const c2r = r + ru;
    const c2z = -lx + ru * (len - lx) / dr;
    const m1 = vector(-nr, 1 - nz) / sqrt(nr * nr + (1 - nz) * (1 - nz));
    const m2 = vector(-1 - nr, -nz) / sqrt((1 + nr) * (1 + nr) + nz * nz);
    revolveProfile(context, id, [{ "p" : rz(r / 2, up) }, { "p" : rz(c1r, up) }, { "p" : rz(c1r, Z) },
            { "p" : rz(c1r - ru * nr, c1z - ru * nz), "mid" : rz(c1r + ru * m1[0], c1z + ru * m1[1]) },
            { "p" : rz(c2r - ru * nr, c2z - ru * nz) },
            { "p" : rz(r, c2z), "mid" : rz(c2r + ru * m2[0], c2z + ru * m2[1]) },
            { "p" : rz(r / 2, c2z) }]);
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
const HEAD_ASKEW = 10;

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
    if (kind == HEAD_ROUND && def.rimDome != undefined && def.rimDome == 1)
    {
        // Live's ISO 7380 button head (behaviours "skb (10-25) ISO 7380
        // dome"): a sphere of radius rf centred on the axis through the rim
        // (dk at the bearing face), cut flat at the head height k: the top is
        // a plane out to dt / 2 (HEAD_TOP_DIA), where the sphere meets it.
        // Live's ASME button head: the same dome over a cylindrical side of
        // height hs (HEAD_SIDE_HEIGHT), the top flat dk / 2 across.
        const rb = def.dk / 2;
        const hs = def.hs != undefined ? def.hs : Z;
        const zc = hs - sqrt(max(def.rf * def.rf - rb * rb, Z * Z));
        var base = [{ "p" : rz(Z, Z) }, { "p" : rz(rb, Z) }];
        if (hs > EPS)
            base = append(base, { "p" : rz(rb, hs) });
        if (zc + def.rf > def.k + EPS)
        {
            const rt = sqrt(max(def.rf * def.rf - (def.k - zc) * (def.k - zc), Z * Z));
            const rm = (rb + rt) / 2;
            revolveProfile(context, id + "round", concatenateArrays([base, [{ "p" : rz(rt, def.k), "mid" : rz(rm, zc + sqrt(def.rf * def.rf - rm * rm)) }, { "p" : rz(Z, def.k) }]]));
        }
        else
        {
            const rm = rb / 2;
            revolveProfile(context, id + "round", append(base, { "p" : rz(Z, zc + def.rf), "mid" : rz(rm, zc + sqrt(def.rf * def.rf - rm * rm)) }));
        }
        return [qCreatedBy(id, EntityType.BODY), def.k];
    }
    if (kind == HEAD_ROUND)
    {
        // A collar (dc, c) under the head (DIN 967) lifts it by c.
        const pan = def.re > EPS && def.rf > EPS;
        var profile = pan ? panCrown(def.dk / 2, def.k - def.c, def.rf, def.re, def.c) : roundHead(def.dk / 2, def.k - def.c, def.rf, def.c);
        // Live's ASME socket head cap screws: a 45 degree HEAD_CHAMFER on the
        // top edge (1/4-20: 0.025 in; dom/skb-sc-geom).
        if (!pan && def.rf <= EPS && def.hc != undefined && def.hc > EPS)
        {
            const r = def.dk / 2;
            const top = def.k;
            profile = [{ "p" : rz(Z, def.c) }, { "p" : rz(r, def.c) }, { "p" : rz(r, top - def.hc) }, { "p" : rz(r - def.hc, top) }, { "p" : rz(Z, top) }];
        }
        // Spherical crowns (the pan's, a dome) under a square recess: the
        // profile up to the crown's rim, the crown from a seamless sphere.
        const dome = !pan && def.rf >= def.dk / 2 - EPS && def.rf > EPS;
        if ((def.drive == DRIVE_SQUARE_III || (def.drive == DRIVE_CROSS && def.rcd > EPS) || (def.drive == DRIVE_POZI && def.rfil > EPS)) && (pan || dome))
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
        // Live's flat countersunk tapping screw (Sketch 1): the 82 degree
        // cone from the head's edge, an arc tangent to it and to the shank,
        // k measured to the arc's end on the shank.
        const ca = cos(41 * degree);
        const sa = sin(41 * degree);
        const rfc = (ca * (def.d - def.dk) / 2 + sa * def.k) / (1 - ca);
        if (def.cskFillet == 1 && def.undercut != 1 && rfc > EPS)
        {
            const cx = def.d / 2 + rfc;
            const tangent = rz(cx - rfc * ca, -def.k + rfc * sa);
            const mid = normalize(vector(-1 - ca, sa));
            pts = [{ "p" : rz(Z, -def.k) }, { "p" : rz(def.d / 2, -def.k) }, { "p" : tangent, "mid" : rz(cx + rfc * mid[0], -def.k + rfc * mid[1]) }, { "p" : rz(def.dk / 2, Z) }];
        }
        // Live's ISO 10642 (behaviours "skb (10-25) ISO 4762 / ISO 10642
        // tables"): HEAD_FILLET rounds the cone into the shank, a torus
        // tangent to both whose centre is ru outside the shank.
        // Live's ASME socket countersunk head: the 82 degree cone from the
        // rim (c below the top) tangent to the FILLET_TRANSITION_DIAMETER
        // torus, whose centre is ru outside the shank at the head height k.
        if (def.filletAtK != undefined && def.filletAtK == 1 && def.ru != undefined && def.ru > EPS)
        {
            const ca = cos(41 * degree);
            const sa = sin(41 * degree);
            const cr = def.d / 2 + def.ru;
            const tangent = rz(cr - def.ru * ca, -def.k + def.ru * sa);
            const m = normalize(vector(-1 - ca, sa));
            // The rim's depth from the tangency (the table's c, unrounded).
            const rim = def.k + (cr - def.dk / 2 - def.ru / ca) * ca / sa;
            pts = [{ "p" : rz(Z, -def.k) }, { "p" : rz(def.d / 2, -def.k) }, { "p" : tangent, "mid" : rz(cr + def.ru * m[0], -def.k + def.ru * m[1]) }, { "p" : rz(def.dk / 2, -rim) }, { "p" : rz(def.dk / 2, Z) }];
        }
        else if (def.ru != undefined && def.ru > EPS && def.undercut != 1 && def.cskFillet != 1)
        {
            const u = normalize(vector(def.dk / 2 - def.d / 2, def.k - def.c));
            const nr = u[1];
            const nz = -u[0];
            const cr = def.d / 2 + def.ru;
            const cz = -def.k + def.ru * (1 - nr) / nz;
            const m = normalize(vector(-1 - nr, -nz));
            pts = [{ "p" : rz(Z, cz) }, { "p" : rz(def.d / 2, cz) }, { "p" : rz(cr - def.ru * nr, cz - def.ru * nz), "mid" : rz(cr + def.ru * m[0], cz + def.ru * m[1]) }, { "p" : rz(def.dk / 2, -def.c) }, { "p" : rz(def.dk / 2, Z) }];
        }
        if (kind == HEAD_RAISED_COUNTERSUNK && def.f > EPS)
        {
            const big = (def.dk * def.dk / 4 + def.f * def.f) / (2 * def.f);
            const midR = def.dk / 4;
            // (Type I too: the seam would split the +x arm end's mouth edge.)
            if (def.drive == DRIVE_SQUARE_III || (def.drive == DRIVE_CROSS && def.rcd > EPS) || (def.drive == DRIVE_POZI && def.rfil > EPS))
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

/**
 * Live's Type IA (Pozidriv-like) recess (behaviours.md "skb (10-22 19:40)",
 * Pan #8 Type IA): one arm L × W cut from the top down to the floor
 * top − (D − T) and its ends drafted 26.3 degrees about plane 1 (where the
 * arm corners meet the head top), patterned 90 degrees (Fillister: onto
 * itself, one arm); a core of diameter `rcore` at the floor tapering up by the
 * slot angle, with a 28 degree cone below; four thin wings (10 degrees, tips at
 * wf · L / 2 on the diagonals) cut down to plane 1 with a 10 degree draft and
 * healed into V-grooves (`wv`), or blind to half the head height on flat heads;
 * the 8 arm-side / core edges filleted `rfil`.
 */
function recessTypeIA(context is Context, id is Id, target is Query, def is map, top is ValueWithUnits)
{
    const L = def.sd;
    const W = def.b;
    const zf = top - (def.t - def.rth);
    const zn = headTopAt(def, sqrt(L * L + W * W) / 2, top);
    const up = top + (top - zf) * 0.1;
    var plane = XY_PLANE;
    plane.origin[2] = zf;
    // The arms: a rectangle at the floor up past the top, ends drafted.
    for (var a = 0; a < (def.arms == 1 ? 1 : 2); a += 1)
    {
        const hx = a == 0 ? W / 2 : L / 2;
        const hy = a == 0 ? L / 2 : W / 2;
        const sketch = newSketchOnPlane(context, id + ("arm" ~ a ~ "Sketch"), { "sketchPlane" : plane });
        skRectangle(sketch, "r", { "firstCorner" : vector(-hx, -hy), "secondCorner" : vector(hx, hy) });
        skSolve(sketch);
        opExtrude(context, id + ("arm" ~ a), { "entities" : qSketchRegion(id + ("arm" ~ a ~ "Sketch")), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : up - zf });
        opDeleteBodies(context, id + ("arm" ~ a ~ "DeleteSketch"), { "entities" : qCreatedBy(id + ("arm" ~ a ~ "Sketch"), EntityType.BODY) });
        var ends = [];
        for (var f in evaluateQuery(context, qGeometry(qOwnedByBody(qCreatedBy(id + ("arm" ~ a), EntityType.BODY), EntityType.FACE), GeometryType.PLANE)))
        {
            const pl = evPlane(context, { "face" : f });
            if (abs(pl.normal[2]) < 0.01 && abs(abs(dot(pl.origin, pl.normal)) - L / 2) < EPS)
                ends = append(ends, f);
        }
        var np = XY_PLANE;
        np.origin[2] = zn;
        opPlane(context, id + ("arm" ~ a ~ "Plane"), { "plane" : np });
        opDraft(context, id + ("arm" ~ a ~ "Draft"), { "draftFaces" : qUnion(ends), "neutralPlane" : qCreatedBy(id + ("arm" ~ a ~ "Plane"), EntityType.FACE), "angle" : 26.3 * degree, "pullDirection" : vector(0, 0, -1), "tangentPropagation" : true });
        opDeleteBodies(context, id + ("arm" ~ a ~ "DeletePlane"), { "entities" : qCreatedBy(id + ("arm" ~ a ~ "Plane"), EntityType.BODY) });
    }
    // The core: Ø rcore at the floor, the slot angle up to the top, 28 degrees down.
    const rTop = def.rcore / 2 + (up - zf) * tan(def.sa * degree);
    revolveProfile(context, id + "core", [{ "p" : rz(Z, up) }, { "p" : rz(rTop, up) }, { "p" : rz(def.rcore / 2, zf) }, { "p" : rz(Z, zf - def.rcore / 2 * tan(28 * degree)) }]);
    // The wings on the diagonals.
    const R = L * def.wf / 2;
    const half = R * tan(5 * degree);
    for (var w = 0; w < 4; w += 1)
    {
        const ang = (45 + 90 * w) * degree;
        const u = vector(cos(ang), sin(ang));
        const n = vector(-u[1], u[0]);
        const tip = u * R;
        const b1 = n * half;
        const b2 = -n * half;
        const depth = def.wv == 1 ? R * tan(5 * degree) / tan(10 * degree) : def.k / 2;
        var wp = XY_PLANE;
        wp.origin[2] = def.wv == 1 ? top : top;
        const sketch = newSketchOnPlane(context, id + ("wing" ~ w ~ "Sketch"), { "sketchPlane" : wp });
        skLineSegment(sketch, "a", { "start" : tip, "end" : b1 });
        skLineSegment(sketch, "b", { "start" : b1, "end" : b2 });
        skLineSegment(sketch, "c", { "start" : b2, "end" : tip });
        skSolve(sketch);
        opExtrude(context, id + ("wing" ~ w), { "entities" : qSketchRegion(id + ("wing" ~ w ~ "Sketch")), "direction" : vector(0, 0, -1), "endBound" : BoundingType.BLIND, "endDepth" : depth });
        opDeleteBodies(context, id + ("wing" ~ w ~ "DeleteSketch"), { "entities" : qCreatedBy(id + ("wing" ~ w ~ "Sketch"), EntityType.BODY) });
        if (def.wv == 1)
        {
            // The two sides lean in 10 degrees going down: a V-groove.
            for (var side = 0; side < 2; side += 1)
            {
                const bi = side == 0 ? b1 : b2;
                const edge = normalize(vector(bi[0] - tip[0], bi[1] - tip[1], Z));
                var nh = vector(-edge[1], edge[0], 0);
                if (dot(nh, vector(-tip[0], -tip[1], Z) / meter) > 0)
                    nh = -nh;
                // Outward normal, tilted so the side narrows downward.
                const nOut = normalize(nh * cos(10 * degree) - vector(0, 0, 1) * sin(10 * degree));
                const big = R * 4;
                const cutter = cuboid(context, id + ("wing" ~ w ~ "Cut" ~ side), vector(Z, -big, -big), vector(big, big, big));
                opTransform(context, id + ("wing" ~ w ~ "Place" ~ side), { "bodies" : cutter, "transform" : toWorld(coordSystem(vector(tip[0], tip[1], top), nOut, edge)) });
                subtract(context, id + ("wing" ~ w ~ "Trim" ~ side), qCreatedBy(id + ("wing" ~ w), EntityType.BODY), cutter);
            }
        }
    }
    // One cut with every tool.
    const tools = qUnion(evaluateQuery(context, qCreatedBy(id, EntityType.BODY)));
    subtract(context, id + "cut", target, tools);
    // The 8 edges where the arm sides meet the core: rfil.
    const screw = qUnion(evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)));
    var edges = [];
    for (var e in evaluateQuery(context, qOwnedByBody(screw, EntityType.EDGE)))
    {
        const bb = evBox3d(context, { "topology" : e });
        const flatX = abs(bb.maxCorner[0] - bb.minCorner[0]) < EPS && abs(abs(bb.minCorner[0]) - W / 2) < EPS;
        const flatY = abs(bb.maxCorner[1] - bb.minCorner[1]) < EPS && abs(abs(bb.minCorner[1]) - W / 2) < EPS;
        if ((flatX || flatY) && bb.maxCorner[2] - bb.minCorner[2] > (top - zf) * 0.3 && bb.minCorner[2] > zf - EPS && max([abs(bb.minCorner[0]), abs(bb.maxCorner[0]), abs(bb.minCorner[1]), abs(bb.maxCorner[1])]) < rTop + EPS)
            edges = append(edges, e);
    }
    if (size(edges) > 0)
    {
        try silent
        {
            opFillet(context, id + "fillet", { "entities" : qUnion(edges), "radius" : def.rfil });
        }
    }
}

/** The drive recess cut into the head's top at z = top. */
function driveCut(context is Context, id is Id, target is Query, def is map, top is ValueWithUnits)
{
    const kind = def.drive;
    if (kind == DRIVE_NONE || def.t <= EPS)
        return;
    if (kind == DRIVE_POZI && def.rfil > EPS)
    {
        recessTypeIA(context, id, target, def, top);
        return;
    }
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
        // Live (dom/skb-sc-geom ISO 4762 / 10642 / 7380 M6, ANSI 1/4-20): the
        // hex to the depth t, then a drill point from the flats' circle
        // (`drill`: ISO 140, inch 120 degrees), which leaves six flat ledges
        // in the hex corners.
        const half = (def.drill != undefined && def.drill > 0 ? def.drill : 140) / 2 * degree;
        const r = def.sd / 2;
        prismZ(context, id + "hex", top - def.t, def.t + reach, polygonPoints(6, def.sd, 0));
        // Live's metric set screws: a flat floor, no drill point.
        const flatFloor = def.head == HEAD_NONE && def.dm != undefined && def.dm > EPS;
        if (!flatFloor)
            revolveProfile(context, id + "cone", [{ "p" : rz(Z, top - def.t - r / tan(half)) }, { "p" : rz(r, top - def.t) }, { "p" : rz(Z, top - def.t) }]);
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
    // Live's metric set screws (behaviours "skb (10-25) set screw sockets"):
    // a 30 degree chamfer from the minor diameter dm on top out to d.
    if (def.dm != undefined && def.dm > EPS && def.dm < def.d - EPS)
        pts = [{ "p" : rz(Z, top) }, { "p" : rz(def.dm / 2, top) }, { "p" : rz(r, top - (r - def.dm / 2) * tan(30 * degree)) }];
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
        // 45 degree chamfer to the cup diameter dp; a 120 degree cup (live
        // ISO 4029 M6: 0.866 deep).
        const depth = rp / tan(60 * degree);
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

/**
 * Live's ANSI askew head bolt (raw /features): a square head s across flats
 * whose bearing face is skewed 9.45° about Y through the origin (k high at
 * +X), its top corners cut by a 25° cone from the flats; the shank (d, l
 * below the origin) with a fillet rf into the head and a 45° end chamfer to
 * dp; the shank face split lt above the end.
 */
function askewBolt(context is Context, id is Id, def is map)
{
    const w = def.s / 2;
    const ta = tan(9.45 * degree);
    const top = def.k - w * ta;
    const r = def.d / 2;
    const sketch = newSketchOnPlane(context, id + "headSketch", { "sketchPlane" : plane(vector(Z, w, Z), vector(0, -1, 0), vector(1, 0, 0)) });
    skPolyline(sketch, "outline", { "points" : [vector(-w, w * ta), vector(w, -w * ta), vector(w, top), vector(-w, top), vector(-w, w * ta)] });
    skSolve(sketch);
    opExtrude(context, id + "head", { "entities" : qSketchRegion(id + "headSketch"), "direction" : vector(0, -1, 0), "endBound" : BoundingType.BLIND, "endDepth" : def.s });
    opDeleteBodies(context, id + "deleteHeadSketch", { "entities" : qCreatedBy(id + "headSketch", EntityType.BODY) });
    const corner = w * sqrt(2);
    subtract(context, id + "headChamfer", qCreatedBy(id + "head", EntityType.BODY),
        revolveProfile(context, id + "cone", [{ "p" : rz(w, top) }, { "p" : rz(2 * corner, top - (2 * corner - w) * tan(25 * degree)) }, { "p" : rz(2 * corner, top + w) }, { "p" : rz(w, top + w) }]));
    const zs = (r * ta + top) / 2;
    const c = (def.d - def.dp) / 2;
    revolveProfile(context, id + "shank", [{ "p" : rz(Z, -def.l) }, { "p" : rz(r - c, -def.l) }, { "p" : rz(r, -def.l + c) }, { "p" : rz(r, zs) }, { "p" : rz(Z, zs) }]);
    uniteAll(context, id);
    const bolt = solidsOf(context, id);
    if (def.rf > EPS)
    {
        // The shank's edge on the bearing face.
        var edges = [];
        for (var e in evaluateQuery(context, qOwnedByBody(bolt, EntityType.EDGE)))
        {
            const bb = evBox3d(context, { "topology" : e });
            if (max([abs(bb.minCorner[0]), abs(bb.maxCorner[0]), abs(bb.minCorner[1]), abs(bb.maxCorner[1])]) < r + EPS && bb.maxCorner[2] - bb.minCorner[2] > EPS && bb.minCorner[2] > -r * ta - EPS && bb.maxCorner[2] < r * ta + EPS)
                edges = append(edges, e);
        }
        if (size(edges) > 0)
            opFillet(context, id + "fillet", { "entities" : qUnion(edges), "radius" : def.rf });
    }
    if (def.lt > EPS)
    {
        // The thread's end: the shank face split lt above the end.
        var p = XY_PLANE;
        p.origin[2] = -def.l + def.lt;
        opPlane(context, id + "threadPlane", { "plane" : p, "width" : 4 * def.s, "height" : 4 * def.s });
        try silent
        {
            opSplitFace(context, id + "threadSplit", { "faceTargets" : qGeometry(qOwnedByBody(solidsOf(context, id), EntityType.FACE), GeometryType.CYLINDER),
                        "faceTools" : qCreatedBy(id + "threadPlane", EntityType.FACE), "keepToolSurfaces" : false });
        }
        opDeleteBodies(context, id + "deleteThreadPlane", { "entities" : qCreatedBy(id + "threadPlane", EntityType.BODY) });
    }
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
        annotation { "Name" : "Drill point angle" }
        isInteger(definition.drill, { (unitless) : [0, 0, 180] } as IntegerBoundSpec);
        annotation { "Name" : "Under-head fillet" }
        isLength(definition.ru, LENGTH_0);
        annotation { "Name" : "Top chamfer diameter" }
        isLength(definition.dm, LENGTH_0);
        annotation { "Name" : "Head holes" }
        isInteger(definition.holes, { (unitless) : [0, 0, 3] } as IntegerBoundSpec);
        annotation { "Name" : "Head hole diameter" }
        isLength(definition.hd, LENGTH_0);
        annotation { "Name" : "Head hole offset" }
        isLength(definition.ho, LENGTH_0);
        annotation { "Name" : "Head chamfer" }
        isLength(definition.hc, LENGTH_0);
        annotation { "Name" : "Transition diameter" }
        isLength(definition.da, LENGTH_0);
        annotation { "Name" : "Transition length" }
        isLength(definition.lx, LENGTH_0);
        annotation { "Name" : "Dome through the rim" }
        isInteger(definition.rimDome, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Head side height" }
        isLength(definition.hs, LENGTH_0);
        annotation { "Name" : "Countersink fillet at k" }
        isInteger(definition.filletAtK, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
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
        annotation { "Name" : "Countersink fillet" }
        isInteger(definition.cskFillet, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
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
        annotation { "Name" : "Type IA fillet" }
        isLength(definition.rfil, LENGTH_0);
        annotation { "Name" : "Type IA wing length factor" }
        isReal(definition.wf, { (unitless) : [0, 1.25, 10] } as RealBoundSpec);
        annotation { "Name" : "Type IA V-groove wings" }
        isInteger(definition.wv, { (unitless) : [0, 1, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Type IA arms" }
        isInteger(definition.arms, { (unitless) : [1, 2, 2] } as IntegerBoundSpec);
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
        annotation { "Name" : "Thread length" }
        isLength(definition.lt, LENGTH_0);
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
        if (definition.head == HEAD_ASKEW)
        {
            askewBolt(context, id, definition);
            return;
        }
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
        // An under-head fillet `ru` between the shank and a flat bearing face
        // (live's ISO 7380 HEAD_FILLET: a torus); the region reaches into the
        // head and the shank so the union never rests on touching faces.
        const transition = def.da != undefined && def.lx != undefined && def.da > def.d + EPS && def.lx > EPS;
        if (transition && def.head != HEAD_NONE && below == Z && neck[1] >= below - EPS)
            underTransition(context, id + "underFillet", def.d / 2, def.da / 2, def.lx, def.ru == undefined ? Z : def.ru);
        else if (def.ru != undefined && def.ru > EPS && def.head != HEAD_NONE && below == Z && neck[1] >= below - EPS)
        {
            const r = def.d / 2;
            const q = def.ru * sqrt(0.5);
            revolveProfile(context, id + "underFillet", [{ "p" : rz(r - def.ru, def.ru) }, { "p" : rz(r + def.ru, def.ru) }, { "p" : rz(r + def.ru, Z) },
                    { "p" : rz(r, -def.ru), "mid" : rz(r + def.ru - q, -def.ru + q) }, { "p" : rz(r - def.ru, -def.ru) }]);
        }
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
        // Live's drilled heads (ASME socket head cap screws, "Hex drilled(2 /
        // 4 / 6)"): one, two or three through-holes of diameter hd, ho below
        // the top, across the hex flats at -60, 0 and 60 degrees.
        if (def.holes != undefined && def.holes > 0 && def.hd != undefined && def.hd > EPS && def.head != HEAD_NONE)
        {
            const screw = solidsOf(context, id);
            const zc = head[1] - def.ho;
            const reach = def.dk + def.d;
            const angles = [-60, 0, 60];
            var tools = [];
            for (var i = 0; i < min(def.holes, 3); i += 1)
            {
                const dir = vector(cos(angles[i] * degree), sin(angles[i] * degree), 0);
                fCylinder(context, id + ("hole" ~ i), { "bottomCenter" : vector(Z, Z, zc) - dir * reach, "topCenter" : vector(Z, Z, zc) + dir * reach, "radius" : def.hd / 2 });
                tools = append(tools, qCreatedBy(id + ("hole" ~ i), EntityType.BODY));
            }
            subtract(context, id + "holes", screw, qUnion(tools));
        }
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
const NUT_T_SLOT = 7;
const NUT_CLINCH_HEX = 8;
const NUT_CLINCH_FLUSH = 9;
const NUT_CLINCH = 10;

/**
 * Live's NASM 45938 self-locking clinch nut (raw /features + B-rep): a
 * collar dc at z = 0 whose side falls 15° in to a step (alen - 0.012 in
 * down), a ring out to (dc + de) / 2 then 0.012 in down to alen, the shank de
 * tlen long with its edges rounded de / 40, the bore at the minor diameter
 * dh opening to the major d over the top 0.012 in then out at 25° to the top,
 * countersunk at the bottom over 1.5 (d - dh); the step's ring cut by 24
 * scallops of radius 0.06 de meeting on its edge.
 */
function selfLockingClinchNut(context is Context, id is Id, def is map)
{
    const a = def.alen;
    const t = def.tlen;
    const k = 0.012 * inch;
    const rs = (def.dc + def.de) / 4;
    const R = def.de / 2;
    const f = def.de / 40;
    const rmaj = def.d / 2;
    const rmin = def.dh / 2;
    const rtop = rmaj + (a - k) * tan(25 * degree);
    const s45 = sin(45 * degree);
    revolveProfile(context, id + "body", [{ "p" : rz(rtop, Z) }, { "p" : rz(def.dc / 2, Z) }, { "p" : rz(def.dc / 2 - (a - k) * tan(15 * degree), -(a - k)) },
                { "p" : rz(rs, -(a - k)) }, { "p" : rz(rs, -a) }, { "p" : rz(R - f, -a) }, { "p" : rz(R, -a - f), "mid" : rz(R - f + f * s45, -a - f + f * s45) },
                { "p" : rz(R, -a - t + f) }, { "p" : rz(R - f, -a - t), "mid" : rz(R - f + f * s45, -a - t + f - f * s45) },
                { "p" : rz(rmaj, -a - t) }, { "p" : rz(rmin, -a - t + 1.5 * (def.d - def.dh)) }, { "p" : rz(rmin, -a) }, { "p" : rz(rmaj, -a) },
                { "p" : rz(rmaj, -(a - k)) }]);
    // The scallops: 24 circles whose neighbours meet on the ring's edge.
    const rho = 0.06 * def.de;
    const half = 7.5 * degree;
    const theta = acos(rs * sin(half) / rho);
    const yc = rs * cos(half) + rho * sin(theta);
    var tools = [];
    for (var i = 0; i < 24; i += 1)
    {
        const ang = (90 + 15 * i) * degree;
        const c = cylinderZ(context, id + ("scallop" ~ i), -a, -(a - k) + k, rho);
        opTransform(context, id + ("moveScallop" ~ i), { "bodies" : c, "transform" : transform(vector(yc * cos(ang), yc * sin(ang), Z)) });
        tools = append(tools, c);
    }
    subtract(context, id + "scallops", qCreatedBy(id + "body", EntityType.BODY), qUnion(tools));
}

/**
 * Live's NAS clinch nuts (raw /features), the thread's minor diameter dh
 * bored through, its ends chamfered 45° out to the major diameter d.
 * NASM 45938/2 (NUT_CLINCH_HEX): a hex s across flats (flats facing ±Y) m
 * high on z = 0, its top corners cut at 30°, on a pilot hex sp across flats
 * hp deep below; the bore's pilot end coned out to the pilot's flats over
 * 0.83 hp. NASM 3214 (NUT_CLINCH_FLUSH): a hex s across flats (flats facing
 * ±X) ll / 3 high on a shank dc ll / 3 then 0.92 dc ll / 3 below.
 */
function clinchNut(context is Context, id is Id, def is map)
{
    if (def.form == NUT_CLINCH)
    {
        selfLockingClinchNut(context, id, def);
        return;
    }
    const r = def.dh / 2;
    const c = (def.d - def.dh) / 2;
    const e = c + def.s;
    var top;
    var holeBottom;
    if (def.form == NUT_CLINCH_HEX)
    {
        top = def.m;
        const nut = prismZ(context, id + "nut", Z, def.m, polygonPoints(6, def.s, 1));
        cornerChamferAngle(context, id + "chamfer", nut, Z, def.m, def.s, def.s, true, false, 30 * degree);
        prismZ(context, id + "pilot", -def.hp, def.hp + EPS, polygonPoints(6, def.sp, 1));
        uniteAll(context, id);
        // The pilot end: a cone from the bore 0.83 hp up out to the flats.
        const rp = def.sp / 2;
        const zc = -def.hp + 0.83 * def.hp;
        const slope = (rp - r) / (0.83 * def.hp);
        holeBottom = [{ "p" : rz(Z, -def.hp - e) }, { "p" : rz(rp + e * slope, -def.hp - e) }, { "p" : rz(r, zc) }];
    }
    else
    {
        const l3 = def.ll / 3;
        top = l3;
        prismZ(context, id + "nut", Z, l3, polygonPoints(6, def.s, 0));
        revolveProfile(context, id + "shank", [{ "p" : rz(Z, -2 * l3) }, { "p" : rz(def.dc / 2, -2 * l3) }, { "p" : rz(def.dc / 2, -l3) },
                    { "p" : rz(0.92 * def.dc / 2, -l3) }, { "p" : rz(0.92 * def.dc / 2, l3 / 2) }, { "p" : rz(Z, l3 / 2) }]);
        uniteAll(context, id);
        holeBottom = [{ "p" : rz(Z, -2 * l3 - e) }, { "p" : rz(r + c + e, -2 * l3 - e) }, { "p" : rz(r, -2 * l3 + c) }];
    }
    const nutBody = solidsOf(context, id);
    subtract(context, id + "bore", nutBody, revolveProfile(context, id + "hole", join(holeBottom, [{ "p" : rz(r, top - c) }, { "p" : rz(r + c + e, top + e) }, { "p" : rz(Z, top + e) }])));
}

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
        annotation { "Name" : "Chamfer angle" }
        isReal(definition.ca, { (unitless) : [0, 30, 90] } as RealBoundSpec);
        annotation { "Name" : "dh" }
        isLength(definition.dh, LENGTH_0);
        annotation { "Name" : "Round slot bottoms" }
        isInteger(definition.roundSlot, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Nose radius" }
        isLength(definition.rn, LENGTH_0);
        annotation { "Name" : "Body radius" }
        isLength(definition.rb, LENGTH_0);
        annotation { "Name" : "Drill depth" }
        isLength(definition.dd, LENGTH_0);
        annotation { "Name" : "Top chamfer diameter" }
        isLength(definition.dt, LENGTH_0);
        annotation { "Name" : "Countersink on top" }
        isInteger(definition.cskTop, { (unitless) : [0, 1, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Collar bore" }
        isLength(definition.cb, LENGTH_0);
        annotation { "Name" : "Collar round" }
        isLength(definition.cr, LENGTH_0);
        annotation { "Name" : "Hole countersink half-angle" }
        isInteger(definition.cska, { (unitless) : [0, 0, 89] } as IntegerBoundSpec);
        annotation { "Name" : "Collar bore depth" }
        isLength(definition.cd, LENGTH_0);
        annotation { "Name" : "T-slot nut length" }
        isLength(definition.tl, LENGTH_0);
        annotation { "Name" : "Tongue fillet" }
        isLength(definition.tr, LENGTH_0);
        annotation { "Name" : "Corner chamfer" }
        isLength(definition.tc, LENGTH_0);
        annotation { "Name" : "Pilot across flats" }
        isLength(definition.sp, LENGTH_0);
        annotation { "Name" : "Pilot height" }
        isLength(definition.hp, LENGTH_0);
        annotation { "Name" : "Nut thickness" }
        isLength(definition.ll, LENGTH_0);
        annotation { "Name" : "Shank length" }
        isLength(definition.tlen, LENGTH_0);
        annotation { "Name" : "Clinch length" }
        isLength(definition.alen, LENGTH_0);
    }
    {
        const form = definition.form;
        if (form == NUT_CLINCH_HEX || form == NUT_CLINCH_FLUSH || form == NUT_CLINCH)
        {
            clinchNut(context, id, definition);
            return;
        }
        const sides = form == NUT_SQUARE || form == NUT_WELD ? 4 : 6;
        // Flange (dc, c) under the body; the body from c to m.
        const z0 = definition.c;
        // A castle nut's hex ends at w and its crown (de) goes up to m.
        const bodyTop = (form == NUT_CASTLE || form == NUT_CAP || form == NUT_INSERT) && definition.w > EPS ? definition.w : definition.m;
        if (form == NUT_T_SLOT)
        {
            // Live's ANSI T-slot nut: the section (width s to height w, the
            // tongue n wide up to m, 45 degree corner chamfers tc, the
            // tongue's root filleted tr) extruded tl along Y, centred.
            const hs = definition.s / 2;
            const hn = definition.n / 2;
            const w = definition.w;
            const ch = definition.tc;
            const rf = definition.tr;
            const sketch = newSketchOnPlane(context, id + "tSketch", { "sketchPlane" : plane(vector(Z, definition.tl / 2, Z), vector(0, -1, 0), vector(1, 0, 0)) });
            var pts = [vector(-hs + ch, Z), vector(hs - ch, Z), vector(hs, ch), vector(hs, w - ch), vector(hs - ch, w), vector(hn + rf, w)];
            skPolyline(sketch, "right", { "points" : pts });
            skArc(sketch, "rightRoot", { "start" : vector(hn + rf, w), "mid" : vector(hn + rf - rf * cos(45 * degree), w + rf - rf * sin(45 * degree)), "end" : vector(hn, w + rf) });
            skPolyline(sketch, "tongue", { "points" : [vector(hn, w + rf), vector(hn, definition.m), vector(-hn, definition.m), vector(-hn, w + rf)] });
            skArc(sketch, "leftRoot", { "start" : vector(-hn, w + rf), "mid" : vector(-hn - rf + rf * cos(45 * degree), w + rf - rf * sin(45 * degree)), "end" : vector(-hn - rf, w) });
            skPolyline(sketch, "left", { "points" : [vector(-hn - rf, w), vector(-hs + ch, w), vector(-hs, w - ch), vector(-hs, ch), vector(-hs + ch, Z)] });
            skSolve(sketch);
            opExtrude(context, id + "tNut", { "entities" : qSketchRegion(id + "tSketch"), "direction" : vector(0, -1, 0), "endBound" : BoundingType.BLIND, "endDepth" : definition.tl });
            opDeleteBodies(context, id + "deleteTSketch", { "entities" : qCreatedBy(id + "tSketch", EntityType.BODY) });
        }
        else
        {
            const body = prismZ(context, id + "body", z0, bodyTop - z0, polygonPoints(sides, definition.s, 0));
            const dw = definition.dw > EPS ? definition.dw : definition.s;
            // (Live's ANSI square machine screw nut has none: angle 0.)
            if (definition.ca > 0)
                cornerChamferAngle(context, id + "chamfer", body, z0, bodyTop - z0, dw, definition.dt > EPS ? definition.dt : dw, true, definition.both == 1 && z0 <= EPS, definition.ca * degree);
        }
        if (z0 > EPS)
            cylinderZ(context, id + "flange", Z, z0 + EPS, definition.dc / 2);
        if (form == NUT_CASTLE)
            cylinderZ(context, id + "crown", bodyTop - EPS, definition.m, definition.de / 2);
        if (form == NUT_CAP && definition.rn > EPS)
        {
            // Live's ANSI acorn nuts: the crown (diameter de) rises straight
            // from w, then an arc of radius rb (centred de / 2 - rb off the
            // axis) runs tangent into the nose, a sphere of radius rn
            // topping out at m.
            const rd = definition.de / 2;
            const rb = definition.rb;
            const rn = definition.rn;
            const rc = rd - rb;
            const zs = definition.m - rn;
            const z1 = zs - sqrt((rb - rn) * (rb - rn) - rc * rc);
            const ux = -rc / (rb - rn);
            const uz = (zs - z1) / (rb - rn);
            const tangent = rz(rc + ux * rb, z1 + uz * rb);
            const a1 = atan2(uz, ux) / 2;
            const a2 = (atan2(z1 + uz * rb - zs, rc + ux * rb) + 90 * degree) / 2;
            revolveProfile(context, id + "cap", [{ "p" : rz(Z, bodyTop - EPS) }, { "p" : rz(rd, bodyTop - EPS) }, { "p" : rz(rd, z1) },
                        { "p" : tangent, "mid" : rz(rc + rb * cos(a1), z1 + rb * sin(a1)) }, { "p" : rz(Z, definition.m), "mid" : rz(rn * cos(a2), zs + rn * sin(a2)) }]);
        }
        else if (form == NUT_CAP)
            // The domed cap (diameter de) from w up to the overall height m.
            revolveProfile(context, id + "cap", roundHead(definition.de / 2, definition.m - bodyTop + EPS, definition.de / 2, bodyTop - EPS));
        if (form == NUT_INSERT && definition.cb > EPS)
        {
            // Live's ANSI nylon insert lock nuts: the collar (diameter de)
            // from w to m, its top outer edge rounded 0.2 (m - w); the
            // hole cut below bores it out (cb, cd deep).
            const rc = definition.de / 2;
            // Live's DIN 985: cr = 0.3125 (h - m) (behaviours "skb (10-25) DIN 985").
            const rr = definition.cr != undefined && definition.cr > EPS ? definition.cr : (definition.m - bodyTop) * 0.2;
            revolveProfile(context, id + "collar", [{ "p" : rz(Z, bodyTop - EPS) }, { "p" : rz(rc, bodyTop - EPS) }, { "p" : rz(rc, definition.m - rr) },
                        { "p" : rz(rc - rr, definition.m), "mid" : rz(rc - rr + rr * cos(45 * degree), definition.m - rr + rr * sin(45 * degree)) }, { "p" : rz(Z, definition.m) }]);
        }
        else if (form == NUT_INSERT)
            // The collar holding the insert (diameter de) up to h... m.
            cylinderZ(context, id + "collar", bodyTop - EPS, definition.m, definition.de / 2);
        if (form == NUT_WELD && definition.h > EPS)
            // The pilot (diameter de, height h) under a weld nut.
            cylinderZ(context, id + "pilot", -definition.h, EPS, definition.de / 2);
        uniteAll(context, id);
        // The hole (blind under a cap nut).
        const holeTop = form == NUT_CAP ? definition.m - (definition.m - bodyTop) / 2 : definition.m + definition.m;
        var nutBody = solidsOf(context, id);
        if (definition.dh > EPS)
        {
            // Live's ANSI nuts: the hole at the thread's minor diameter dh,
            // 90 degree countersinks out to d on both faces.
            const rh = definition.dh / 2;
            const rd = definition.d / 2;
            // The countersinks' depth: 90 degrees (ANSI), cska = 60 for live's
            // DIN 985 120 degree countersinks.
            const half = (definition.cska != undefined && definition.cska > 0 ? definition.cska : 45) * degree;
            const cut = (rd - rh) / tan(half);
            // A cap nut's hole is drilled dd deep with a 118 degree tip.
            // (Past the faces along the countersink's own slope.)
            var top = [{ "p" : rz(rh, definition.m - cut) }, { "p" : rz(rd + EPS * tan(half), definition.m + EPS) }, { "p" : rz(Z, definition.m + EPS) }];
            if (form == NUT_CAP && definition.dd > EPS)
                top = [{ "p" : rz(rh, definition.dd) }, { "p" : rz(Z, definition.dd + rh / tan(59 * degree)) }];
            else if (form == NUT_INSERT && definition.cb > EPS)
            {
                // The collar's bore (cb) down to its floor (cd deep, else
                // the hex top), the countersink on the floor; a bore
                // narrower than d cuts the countersink short.
                const zf = definition.cd > EPS ? definition.m - definition.cd : bodyTop;
                const rb = definition.cb / 2;
                top = rd < rb ? [{ "p" : rz(rh, zf - cut) }, { "p" : rz(rd, zf) }, { "p" : rz(rb, zf) }, { "p" : rz(rb, definition.m + EPS) }, { "p" : rz(Z, definition.m + EPS) }] :
                    [{ "p" : rz(rh, zf - cut) }, { "p" : rz(rb, zf - cut + (rb - rh) / tan(half)) }, { "p" : rz(rb, definition.m + EPS) }, { "p" : rz(Z, definition.m + EPS) }];
            }
            else if (definition.cskTop == 0)
                top = [{ "p" : rz(rh, definition.m + EPS) }, { "p" : rz(Z, definition.m + EPS) }];
            subtract(context, id + "holeCut", nutBody, revolveProfile(context, id + "hole", concatenateArrays([[{ "p" : rz(Z, -definition.h - EPS) }, { "p" : rz(rd + EPS * tan(half), -definition.h - EPS) }, { "p" : rz(rh, -definition.h + cut) }], top])));
        }
        else
            subtract(context, id + "holeCut", nutBody, cylinderZ(context, id + "hole", -definition.m - definition.h, holeTop, definition.d / 2));
        if ((form == NUT_SLOTTED || form == NUT_CASTLE) && definition.n > EPS)
        {
            nutBody = solidsOf(context, id);
            const reach = definition.s * 2;
            // Live's ANSI slotted nuts (roundSlot): half-cylinder slot
            // bottoms, lowest at w.
            const r = definition.n / 2;
            const roundBottom = definition.roundSlot == 1;
            cuboid(context, id + "slotA", vector(-reach, -r, roundBottom ? definition.w + r : definition.w), vector(reach, r, definition.m + reach));
            if (roundBottom)
            {
                fCylinder(context, id + "slotRound", { "bottomCenter" : vector(-reach, Z, definition.w + r), "topCenter" : vector(reach, Z, definition.w + r), "radius" : r });
                opBoolean(context, id + "slotJoin", { "tools" : qUnion([qCreatedBy(id + "slotA", EntityType.BODY), qCreatedBy(id + "slotRound", EntityType.BODY)]), "operationType" : BooleanOperationType.UNION });
            }
            const a = qUnion([qCreatedBy(id + "slotA", EntityType.BODY), qCreatedBy(id + "slotRound", EntityType.BODY)]);
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
const WASHER_SPRING = 8;
const WASHER_TOOTH_OUT = 9;
const WASHER_TOOTH_IN = 10;
const WASHER_TOOTH_IN_OUT = 11;

/**
 * Cut a tooth lock washer's `ring` (thickness h, outer radius r2): `teeth`
 * bands 2 half wide on radial lines from t0 degrees; `external`: the bands
 * are teeth beyond the root circle rr (all else beyond it cut away), else
 * slots inside it.
 */
function toothCut(context is Context, id is Id, ring is Query, h is ValueWithUnits, r2 is ValueWithUnits, external is boolean, rr is ValueWithUnits, half is ValueWithUnits, teeth is number, t0 is number)
{
    const big = r2 * 2 + h;
    const axis = line(vector(Z, Z, Z), vector(0, 0, 1));
    const band = cuboid(context, id + "band", vector(Z, -half, -h), vector(big, half, 2 * h));
    opTransform(context, id + "bandTurn", { "bodies" : band, "transform" : rotationAround(axis, t0 * degree) });
    var transforms = [];
    var names = [];
    for (var i = 1; i < teeth; i += 1)
    {
        transforms = append(transforms, rotationAround(axis, i * 360 / teeth * degree));
        names = append(names, "t" ~ i);
    }
    opPattern(context, id + "bands", { "entities" : band, "transforms" : transforms, "instanceNames" : names });
    const bands = qUnion([band, qCreatedBy(id + "bands", EntityType.BODY)]);
    if (external)
    {
        // The cut: outside the root circle, between the teeth.
        const outer = cylinderZ(context, id + "outer", -2 * h, 3 * h, big);
        subtract(context, id + "rootCut", outer, cylinderZ(context, id + "root", -3 * h, 4 * h, rr));
        subtract(context, id + "teethCut", outer, bands);
        subtract(context, id + "slots", ring, outer);
    }
    else
    {
        // The slots: the bands inside the root circle (the disc less the
        // disc without the bands).
        const disc = cylinderZ(context, id + "disc", -2 * h, 3 * h, rr);
        const rest = cylinderZ(context, id + "rest", -3 * h, 4 * h, rr);
        subtract(context, id + "restCut", rest, bands);
        subtract(context, id + "discCut", disc, rest);
        subtract(context, id + "slots", ring, disc);
    }
}

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
        annotation { "Name" : "Slot half angle" }
        isReal(definition.ta, { (unitless) : [0, 0, 90] } as RealBoundSpec);
        annotation { "Name" : "First slot angle" }
        isReal(definition.t0, { (unitless) : [-360, 0, 360] } as RealBoundSpec);
        annotation { "Name" : "Outer teeth" }
        isInteger(definition.teeth2, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "Outer tooth half angle" }
        isReal(definition.ta2, { (unitless) : [0, 0, 90] } as RealBoundSpec);
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
        if (form == WASHER_TOOTH_OUT || form == WASHER_TOOTH_IN || form == WASHER_TOOTH_IN_OUT)
        {
            // Live's ANSI tooth lock washers (raw /features + B-rep): a flat
            // ring and straight-sided bands centred on radial lines.
            // External: the root circle Ø 1.05 (d1 + d2) / 2; the bands (the
            // root circle's chord at ±ta) are the teeth, all else outside the
            // root is cut away. Internal: the bands (the bore's chord at ±ta)
            // are the slots, from the bore out to the root circle Ø (d1 +
            // d2) / 2. Internal/external: 8 inner slots out to Ø d1 + (d2 -
            // d1) / 3 and `teeth2` outer teeth (±ta2) from Ø d1 + 2 (d2 -
            // d1) / 3.
            const ring = revolveProfile(context, id + "washer", [{ "p" : rz(r1, Z) }, { "p" : rz(r2, Z) }, { "p" : rz(r2, h) }, { "p" : rz(r1, h) }]);
            if (form == WASHER_TOOTH_OUT)
            {
                const rr = 1.05 * (r1 + r2) / 2;
                toothCut(context, id + "outer", ring, h, r2, true, rr, rr * sin(definition.ta * degree), definition.teeth, definition.t0);
            }
            else if (form == WASHER_TOOTH_IN)
                toothCut(context, id + "inner", ring, h, r2, false, (r1 + r2) / 2, r1 * sin(definition.ta * degree), definition.teeth, definition.t0);
            else
            {
                toothCut(context, id + "inner", ring, h, r2, false, r1 + (r2 - r1) / 3, r1 * sin(definition.ta * degree), definition.teeth, definition.t0);
                const rr = r1 + 2 * (r2 - r1) / 3;
                toothCut(context, id + "outer", solidsOf(context, id), h, r2, true, rr, rr * sin(definition.ta2 * degree), definition.teeth2, definition.t0);
            }
            return;
        }
        if (form == WASHER_SPRING)
        {
            // Live's ANSI spring lock washers: a section W = (d2 - d1) / 2
            // wide whose faces slope 2.5 degrees (thicker inside, h at the
            // middle, the bottom inner corner on the XY plane), its four
            // edges rounded (W - s) / 2 (s the bearing width), split
            // straight across +X by a gap h / 2 wide.
            const w = r2 - r1;
            const slope = tan(2.5 * degree);
            const topIn = h + w * slope;
            const corners = [rz(r1, Z), rz(r2, w * slope), rz(r2, topIn - w * slope), rz(r1, topIn)];
            const washer = revolveProfile(context, id + "washer", definition.s > EPS && definition.s < w - EPS ? roundedProfile(corners, (w - definition.s) / 2) : mapArray(corners, function(c) { return { "p" : c }; }));
            subtract(context, id + "split", washer, cuboid(context, id + "gap", vector(r1 - h, -h / 4, -h), vector(r2 + h, h / 4, 2 * h)));
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
            if (definition.r > r)
            {
                // Live's ANSI pin (behaviours "… ANSI Commercial class taper
                // pin B-rep", request 1504): crowned ends, spheres of radius
                // r on the axis at +-zc (symmetric about the origin), each
                // through its seam with the cone; the seams l apart, the
                // large one at d / 2.
                const cr = definition.r;
                const big = sqrt(cr * cr - r * r);
                const low = sqrt(cr * cr - small * small);
                const zl = (l - low + big) / 2;
                const zc = zl - big;
                const zs = zl - l;
                const m1 = (atan2(zl - zc, r) + 90 * degree) / 2;
                const m2 = (atan2(zs + zc, small) - 90 * degree) / 2;
                revolveProfile(context, id + "pin", [{ "p" : rz(Z, zc + cr) }, { "p" : rz(r, zl), "mid" : rz(cr * cos(m1), zc + cr * sin(m1)) }, { "p" : rz(small, zs) },
                        { "p" : rz(Z, -zc - cr), "mid" : rz(cr * cos(m2), -zc + cr * sin(m2)) }]);
                return;
            }
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
const KEY_ROUND_TAPER = 5;
const KEY_ONE_ROUND_END = 6;
const KEY_B17_TAPER = 7;

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
        annotation { "Name" : "Flat" }
        isLength(definition.flat, LENGTH_0);
    }
    {
        const form = definition.form;
        const b = definition.b;
        const h = definition.h;
        const l = definition.l;
        if (form == KEY_B17_TAPER)
        {
            // Live's B17.1 tapered key: the side profile (Y along the key, Z
            // up) h high at +Y, flat for `flat` (Plain: the width, Alternate:
            // none), then falling 1/8 in per foot to -Y; extruded symmetric b.
            const drop = 0.125 / 12 * (l - definition.flat);
            const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : plane(vector(-b / 2, Z, Z), vector(1, 0, 0), vector(0, 1, 0)) });
            var pts = [vector(-l / 2, Z), vector(l / 2, Z), vector(l / 2, h)];
            if (definition.flat > EPS)
                pts = append(pts, vector(l / 2 - definition.flat, h));
            pts = append(pts, vector(-l / 2, h - drop));
            skPolyline(sketch, "outline", { "points" : append(pts, pts[0]) });
            skSolve(sketch);
            opExtrude(context, id + "key", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(1, 0, 0), "endBound" : BoundingType.BLIND, "endDepth" : b });
            opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
            return;
        }
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
        // Live (DIN 6885-1 / 6883 / 6886): the key along Y, width b along X,
        // bottom on the XY plane; round ends of radius b / 2 (A), square (B),
        // one round end (AB: round at -Y, square at +Y; live's Extrude 1
        // regions); tapered keys fall 1:100 towards +Y from h at the -Y end.
        const roundHigh = form == KEY_ROUND_ENDS || form == KEY_ROUND_TAPER;
        const roundLow = roundHigh || form == KEY_ONE_ROUND_END;
        const r = b / 2;
        const y0 = roundLow ? -l / 2 + r : -l / 2;
        const y1 = roundHigh ? l / 2 - r : l / 2;
        const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : XY_PLANE });
        skLineSegment(sketch, "s1", { "start" : vector(r, y0), "end" : vector(r, y1) });
        if (roundHigh)
            skArc(sketch, "e1", { "start" : vector(r, y1), "mid" : vector(Z, l / 2), "end" : vector(-r, y1) });
        else
            skLineSegment(sketch, "e1", { "start" : vector(r, y1), "end" : vector(-r, y1) });
        skLineSegment(sketch, "s2", { "start" : vector(-r, y1), "end" : vector(-r, y0) });
        if (roundLow)
            skArc(sketch, "e2", { "start" : vector(-r, y0), "mid" : vector(Z, -l / 2), "end" : vector(r, y0) });
        else
            skLineSegment(sketch, "e2", { "start" : vector(-r, y0), "end" : vector(r, y0) });
        skSolve(sketch);
        opExtrude(context, id + "key", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : h });
        opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
        const body = qCreatedBy(id + "key", EntityType.BODY);
        if (form == KEY_TAPER || form == KEY_ROUND_TAPER || form == KEY_GIB_HEAD)
        {
            const cut = cuboid(context, id + "taperTool", vector(-b, -l, h), vector(b, l, h + l));
            opTransform(context, id + "tilt", { "bodies" : cut, "transform" : rotationAround(line(vector(Z, -l / 2, h), vector(1, 0, 0)), -atan(0.01)) });
            subtract(context, id + "taperCut", body, cut);
        }
        if (form == KEY_GIB_HEAD && definition.h1 > EPS)
        {
            // The gib head (height h1, as long as it is high) at the thick end.
            cuboid(context, id + "head", vector(-b / 2, -l / 2 - definition.h1, Z), vector(b / 2, -l / 2 + EPS, definition.h1));
            uniteAll(context, id);
        }
    });

/**
 * A closed profile in the XY plane extruded symmetrically about z = 0 (2h
 * thick). `segments` as for `revolveProfile`; a last segment back to the
 * first point (an arc) closes it, else a line does.
 */
function extrudeProfileZ(context is Context, id is Id, h is ValueWithUnits, segments is array) returns Query
{
    var p = XY_PLANE;
    p.origin[2] = -h;
    const sketch = newSketchOnPlane(context, id + "sketch", { "sketchPlane" : p });
    var prev = segments[0].p;
    const n = size(segments);
    const closed = norm(segments[n - 1].p - segments[0].p) < EPS;
    for (var i = 1; i <= n; i += 1)
    {
        if (i == n && closed)
            break;
        const s = i < n ? segments[i] : { "p" : segments[0].p };
        if (s.mid != undefined)
            skArc(sketch, "a" ~ i, { "start" : prev, "mid" : s.mid, "end" : s.p });
        else if (norm(s.p - prev) > EPS)
            skLineSegment(sketch, "l" ~ i, { "start" : prev, "end" : s.p });
        prev = s.p;
    }
    skSolve(sketch);
    opExtrude(context, id + "extrude", { "entities" : qSketchRegion(id + "sketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : 2 * h });
    opDeleteBodies(context, id + "deleteSketch", { "entities" : qCreatedBy(id + "sketch", EntityType.BODY) });
    return qCreatedBy(id + "extrude", EntityType.BODY);
}

/** The point mirrored in the YZ plane. */
function mirrorX(p is Vector) returns Vector
{
    return vector(-p[0], p[1]);
}

/** Where the line dot(n, X) = k meets the circle (c, r): the lower point. */
function lineCircleLow(n is Vector, k is ValueWithUnits, c is Vector, r is ValueWithUnits) returns Vector
{
    const m = vector(-n[1], n[0]);
    const pn = dot(n, c);
    const pm = dot(m, c);
    const t = pm - sqrt(r * r - (k - pn) * (k - pn)) * (m[1] > 0 ? 1 : -1);
    return n * k + m * t;
}

/** Where circles (c1, r1) and (c2, r2) meet: the point with the smaller x. */
function circleCircleLeft(c1 is Vector, r1 is ValueWithUnits, c2 is Vector, r2 is ValueWithUnits) returns Vector
{
    const d = norm(c2 - c1);
    const u = (c2 - c1) / d;
    const a = (r1 * r1 - r2 * r2 + d * d) / (2 * d);
    const b = sqrt(r1 * r1 - a * a);
    const w = vector(-u[1], u[0]);
    const p = c1 + u * a + w * b;
    const q = c1 + u * a - w * b;
    return p[0] < q[0] ? p : q;
}

/** Both points where circles (c1, r1) and (c2, r2) meet (none: undefined). */
function circleCircleBoth(c1 is Vector, r1 is ValueWithUnits, c2 is Vector, r2 is ValueWithUnits)
{
    const d = norm(c2 - c1);
    if (d < EPS)
        return undefined;
    const u = (c2 - c1) / d;
    const a = (r1 * r1 - r2 * r2 + d * d) / (2 * d);
    const h2 = r1 * r1 - a * a;
    if (h2 < 0 * meter * meter)
        return undefined;
    const b = sqrt(h2);
    const w = vector(-u[1], u[0]);
    return [c1 + u * a + w * b, c1 + u * a - w * b];
}

/** Where circles (c1, r1) and (c2, r2) meet: the lower point. */
function circleCircleLow(c1 is Vector, r1 is ValueWithUnits, c2 is Vector, r2 is ValueWithUnits) returns Vector
{
    const pq = circleCircleBoth(c1, r1, c2, r2);
    return pq[0][1] < pq[1][1] ? pq[0] : pq[1];
}

/** The middle of the arc (centre c, radius r) from a to b, the short way. */
function arcMid(c is Vector, a is Vector, b is Vector, r is ValueWithUnits) returns Vector
{
    return c + normalize(normalize(a - c) + normalize(b - c)) * r;
}

/**
 * An internal ring's configuration B blend (live's Sketch 1): the circle
 * inside the inner circle (centre ci, radius ri) tangent to it and through
 * F (the A-arc's centre), meeting the lug circle (rl) square, or 0.96 D off
 * the hole at zc (`hole`). Returns [centre, radius]: the largest such.
 */
function ringBlend(ci is Vector, ri is ValueWithUnits, F is Vector, zc is Vector, rl is ValueWithUnits, D is ValueWithUnits, hole is boolean) returns array
{
    const centreOf = function(rho)
        {
            const pq = circleCircleBoth(ci, ri - rho, F, rho);
            if (pq == undefined)
                return undefined;
            return pq[0][1] > pq[1][1] ? pq[0] : pq[1];
        };
    const f = function(rho)
        {
            const c = centreOf(rho);
            if (c == undefined)
                return undefined;
            if (hole)
                return (norm(c - zc) - rho - D / 2 - 0.96 * D) / meter;
            return (dot(c, c) - rl * rl - rho * rho) / (meter * meter);
        };
    const steps = 2000;
    var hi = undefined;
    var fhi = undefined;
    for (var i = steps - 1; i > 0; i -= 1)
    {
        const rho = ri * i / steps;
        const val = f(rho);
        if (val != undefined && fhi != undefined && (val < 0) != (fhi < 0))
        {
            var a = rho;
            var b = hi;
            var fa = val;
            for (var k = 0; k < 60; k += 1)
            {
                const m = (a + b) / 2;
                const fm = f(m);
                if (fm == undefined)
                    break;
                if ((fm < 0) == (fa < 0))
                {
                    a = m;
                    fa = fm;
                }
                else
                    b = m;
            }
            const rho0 = (a + b) / 2;
            return [centreOf(rho0), rho0];
        }
        hi = rho;
        fhi = val;
    }
    throw regenError("No blend for this retaining ring");
}

/** Hole rules of the external rings' lugs: 0.4 hole diameters from the lug fillet (DIN 471) or the lug side. */
const RING_HOLE_TO_FILLET = 0;
const RING_HOLE_TO_SIDE = 1;

annotation { "Feature Type Name" : "Retaining ring" }
export const retainingRing = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Internal" }
        isInteger(definition.internal, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Hole rule" }
        isInteger(definition.holeRule, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
        annotation { "Name" : "Groove diameter" }
        isLength(definition.dg, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "Large section" }
        isLength(definition.large, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "Small section" }
        isLength(definition.small, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "Lug section" }
        isLength(definition.lug, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "Hole diameter" }
        isLength(definition.d5, LENGTH_0);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "Lug corner radius" }
        isLength(definition.rc, LENGTH_0);
        annotation { "Name" : "Gap divisor" }
        isReal(definition.gapDiv, { (unitless) : [0.1, 1.95, 100] } as RealBoundSpec);
        annotation { "Name" : "Lug side angle" }
        isReal(definition.sideAngle, { (unitless) : [0, 7, 45] } as RealBoundSpec);
        annotation { "Name" : "Lug radius factor" }
        isReal(definition.lugK, { (unitless) : [0, 0.8, 10] } as RealBoundSpec);
        annotation { "Name" : "Configuration" }
        isInteger(definition.ringConfig, { (unitless) : [0, 0, 2] } as IntegerBoundSpec);
        annotation { "Name" : "Blend rule" }
        isInteger(definition.blendRule, { (unitless) : [0, 0, 1] } as IntegerBoundSpec);
    }
    {
        // Live's DIN 471 / 472 and ANSI rings (raw /features: one sketch,
        // extruded symmetric s), lugs at -Y.
        const s = definition.s;
        const rg = definition.dg / 2;
        const L = definition.large;
        const S = definition.small;
        const lug = definition.lug;
        const D = definition.d5;
        const big = 4 * rg + 4 * lug;
        if (definition.internal == 0)
        {
            // External: the groove circle inside; outside, a circle as wide
            // as L at +Y and S at -Y; the lugs out to rg + lug with sides
            // tilted sideAngle, rounded rc at the tips and lugK·lug into the
            // ring; the gap s / gapDiv; holes on rg + lug / 2, s apart.
            const Re = rg + (L + S) / 2;
            const E = vector(Z, (L - S) / 2);
            const rl = rg + lug;
            const Rf = definition.lugK * lug;
            const rc = definition.rc;
            const a = definition.sideAngle * degree;
            const n = vector(cos(a), sin(a));
            const hx = s / 2 + D / 2;
            const rh = rg + lug / 2;
            const H = vector(-hx, -sqrt(rh * rh - hx * hx));
            var C;
            var c;
            if (definition.holeRule == RING_HOLE_TO_SIDE)
            {
                c = dot(n, H) - 0.9 * D;
                C = lineCircleLow(n, c - Rf, E, Re + Rf);
            }
            else
            {
                C = circleCircleLeft(E, Re + Rf, H, Rf + 0.9 * D);
                c = dot(n, C) + Rf;
            }
            const K = lineCircleLow(n, c + rc, vector(Z, Z), rl - rc);
            const t1 = E + (C - E) * (Re / (Re + Rf));
            const t2 = C + n * Rf;
            const t3 = K - n * rc;
            const t4 = K * (rl / (rl - rc));
            const bigMid = C + normalize(normalize(E - C) + n) * Rf;
            const smallMid = K + normalize(normalize(K) - n) * rc;
            extrudeProfileZ(context, id + "ring", s / 2, [{ "p" : t1 }, { "p" : t2, "mid" : bigMid }, { "p" : t3 }, { "p" : t4, "mid" : smallMid },
                        { "p" : mirrorX(t4), "mid" : vector(Z, -rl) }, { "p" : mirrorX(t3), "mid" : mirrorX(smallMid) }, { "p" : mirrorX(t2) },
                        { "p" : mirrorX(t1), "mid" : mirrorX(bigMid) }, { "p" : t1, "mid" : vector(Z, E[1] + Re) }]);
            subtract(context, id + "bore", qCreatedBy(id + "ring", EntityType.BODY), cylinderZ(context, id + "groove", -s, s, rg));
            const gap = s / definition.gapDiv;
            subtract(context, id + "opening", solidsOf(context, id), cuboid(context, id + "gap", vector(-gap / 2, -big, -s), vector(gap / 2, Z, s)));
            ringHoles(context, id, s, D, H);
        }
        else
        {
            // Internal: the bore circle outside; inside, a circle as wide as
            // L at +Y and S at -Y; the lugs end on radial planes through the
            // bore's chord of dg / 4. Live's FSRules pick a configuration by
            // size (behaviours "skb (10-24) rings / washers"):
            // A: each lug end an arc lugK (lug - L / 2) tangent to the end
            //    plane and the lug circle rg - lug, a second equal arc into
            //    the inner circle; holes concentric with the lug ends.
            // B: the lug's inner edge on the lug circle, one blend arc from it
            //    into the inner circle (through the A-arc's centre; meeting
            //    the lug circle square, blendRule 0, or 0.96 D from the hole,
            //    1), the four corners rounded rc.
            // C: the inner circle concentric at rg - lug, no lugs; holes on
            //    rg - lug / 2, 1.5 D from the end planes.
            const R = rg;
            const ri = R - (L + S) / 2;
            const ci = vector(Z, (S - L) / 2);
            const rl = R - lug;
            const Rs = definition.lugK * (lug - L / 2);
            const v = vector(-0.25, -sqrt(15) / 4);
            const n = vector(-sqrt(15) / 4, 0.25);
            const pl = v * R;
            if (definition.ringConfig == 2)
            {
                const rh = R - lug / 2;
                const t = 1.5 * D;
                extrudeProfileZ(context, id + "ring", s / 2, [{ "p" : mirrorX(pl) }, { "p" : pl, "mid" : vector(Z, R) }, { "p" : v * rl }, { "p" : mirrorX(v * rl), "mid" : vector(Z, rl) }]);
                ringHoles(context, id, s, D, n * t + v * sqrt(rh * rh - t * t));
                return;
            }
            const zc = n * Rs + v * sqrt((rl + Rs) * (rl + Rs) - Rs * Rs);
            const F = circleCircleLeft(zc, 2 * Rs, ci, ri - Rs);
            if (definition.ringConfig == 1)
            {
                const blend = ringBlend(ci, ri, F, zc, rl, D, definition.blendRule == 1);
                const C = blend[0];
                const rho = blend[1];
                const rf = definition.rc;
                const K1 = n * rf + v * sqrt((rl + rf) * (rl + rf) - rf * rf);
                const L1 = K1 - n * rf;
                const A1 = K1 * (rl / (rl + rf));
                const K2 = circleCircleLow(vector(Z, Z), rl + rf, C, rho + rf);
                const A2 = K2 * (rl / (rl + rf));
                const B2 = K2 + (C - K2) * (rf / (rho + rf));
                const T = ci + (C - ci) * (ri / (ri - rho));
                const m1 = arcMid(K1, L1, A1, rf);
                const m2 = arcMid(K2, A2, B2, rf);
                const mb = arcMid(C, B2, T, rho);
                const ml = normalize(A1 + A2) * rl;
                extrudeProfileZ(context, id + "ring", s / 2, [{ "p" : mirrorX(pl) }, { "p" : pl, "mid" : vector(Z, R) }, { "p" : L1 }, { "p" : A1, "mid" : m1 },
                            { "p" : A2, "mid" : ml }, { "p" : B2, "mid" : m2 }, { "p" : T, "mid" : mb }, { "p" : mirrorX(T), "mid" : vector(Z, ci[1] + ri) },
                            { "p" : mirrorX(B2), "mid" : mirrorX(mb) }, { "p" : mirrorX(A2), "mid" : mirrorX(m2) }, { "p" : mirrorX(A1), "mid" : mirrorX(ml) },
                            { "p" : mirrorX(L1), "mid" : mirrorX(m1) }]);
                ringHoles(context, id, s, D, zc);
                return;
            }
            const tlz = zc - n * Rs;
            const tzf = (zc + F) / 2;
            const tfe = ci + (F - ci) * (ri / (ri - Rs));
            const zMid = zc - normalize(zc) * Rs;
            const fMid = F + normalize(normalize(tzf - F) + normalize(tfe - F)) * Rs;
            extrudeProfileZ(context, id + "ring", s / 2, [{ "p" : mirrorX(pl) }, { "p" : pl, "mid" : vector(Z, R) }, { "p" : tlz }, { "p" : tzf, "mid" : zMid },
                        { "p" : tfe, "mid" : fMid }, { "p" : mirrorX(tfe), "mid" : vector(Z, ci[1] + ri) }, { "p" : mirrorX(tzf), "mid" : mirrorX(fMid) },
                        { "p" : mirrorX(tlz), "mid" : mirrorX(zMid) }]);
            ringHoles(context, id, s, D, zc);
        }
    });

/** The lug holes (diameter d) at `c` and its mirror image. */
function ringHoles(context is Context, id is Id, s is ValueWithUnits, d is ValueWithUnits, c is Vector)
{
    if (d < EPS)
        return;
    const body = solidsOf(context, id);
    const hA = cylinderZ(context, id + "holeA", -s, s, d / 2);
    opTransform(context, id + "moveHoleA", { "bodies" : hA, "transform" : transform(vector(c[0], c[1], Z)) });
    const hB = cylinderZ(context, id + "holeB", -s, s, d / 2);
    opTransform(context, id + "moveHoleB", { "bodies" : hB, "transform" : transform(vector(-c[0], c[1], Z)) });
    subtract(context, id + "holes", body, qUnion([hA, hB]));
}

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

/**
 * Live's DIN 923 slotted pan head screw with shoulder (raw /features,
 * behaviours "skasm (2026-10-27) standard content gaps"): one revolved
 * sketch, the shoulder end at z = 0 (live's "Near" connector) with the head
 * above: the pan head (dk, k) with its top edge chamfered c, a fillet rf into
 * the shoulder (ds, l long), the shoulder end, the thread neck (dn) with
 * fillets rn into the shoulder end and into a 30 degree taper out to d, which
 * it reaches ln below the shoulder end, the thread down to b below it, its
 * end chamfered 0.625 of the thread's triangle height; then the slot (n wide,
 * t deep) across the head.
 */
annotation { "Feature Type Name" : "Shoulder screw" }
export const shoulderScrew = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "d" }
        isLength(definition.d, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dk" }
        isLength(definition.dk, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "k" }
        isLength(definition.k, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "ds" }
        isLength(definition.ds, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "rf" }
        isLength(definition.rf, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "c" }
        isLength(definition.c, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dn" }
        isLength(definition.dn, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "ln" }
        isLength(definition.ln, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "rn" }
        isLength(definition.rn, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "b" }
        isLength(definition.b, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "p" }
        isLength(definition.p, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "n" }
        isLength(definition.n, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "t" }
        isLength(definition.t, NONNEGATIVE_LENGTH_BOUNDS);
    }
    {
        const R = definition.dk / 2;
        const rs = definition.ds / 2;
        const rh = definition.rf;
        const rnk = definition.dn / 2;
        const f = definition.rn;
        const rm = definition.d / 2;
        const ls = definition.l;
        const top = ls + definition.k;
        const c = definition.c;
        const ch = 0.625 * 0.8660254 * definition.p;
        const s45 = sqrt(0.5);
        // The taper: from the neck fillet's end (30 degrees round from the
        // neck) at 30 degrees from the axis out to d, reached ln down.
        const p9x = rnk + f * (1 - cos(30 * degree));
        const run = (rm - p9x) / sin(30 * degree);
        const zc = -definition.ln + run * cos(30 * degree) + f / 2;
        const body = revolveProfile(context, id + "body", [
                    { "p" : rz(Z, top) },
                    { "p" : rz(R - c, top) },
                    { "p" : rz(R, top - c) },
                    { "p" : rz(R, ls) },
                    { "p" : rz(rs + rh, ls) },
                    { "p" : rz(rs, ls - rh), "mid" : rz(rs + rh - rh * s45, ls - rh + rh * s45) },
                    { "p" : rz(rs, Z) },
                    { "p" : rz(rnk + f, Z) },
                    { "p" : rz(rnk, -f), "mid" : rz(rnk + f - f * s45, -f + f * s45) },
                    { "p" : rz(rnk, zc) },
                    { "p" : rz(p9x, zc - f / 2), "mid" : rz(rnk + f - f * cos(15 * degree), zc - f * sin(15 * degree)) },
                    { "p" : rz(rm, -definition.ln) },
                    { "p" : rz(rm, -definition.b + ch) },
                    { "p" : rz(rm - ch, -definition.b) },
                    { "p" : rz(Z, -definition.b) }
                ]);
        const slot = cuboid(context, id + "slot", vector(-definition.n / 2, -R - definition.k, top - definition.t), vector(definition.n / 2, R + definition.k, top + definition.k));
        subtract(context, id + "cut", body, slot);
    });

/**
 * Live's ISO 7379 socket head shoulder screw (raw /features): one revolved
 * sketch, the shoulder end at z = 0 (live's "Near") with the head above: the
 * head (dk, k) with its top edge rounded hf, a fillet hf into the neck under
 * the head (dsn, snw below the head) and a 45 degree step out to the shoulder
 * (ds, l long), the shoulder end, the thread neck (dtn, tnw long) with a
 * fillet tnf into the shoulder end, a step out to d and the thread down to b
 * below the shoulder end, both its edges chamfered 0.625 of the thread's
 * triangle height; then the hex socket (s across flats, t deep) with a 140
 * degree drill point.
 */
annotation { "Feature Type Name" : "Socket shoulder screw" }
export const socketShoulderScrew = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "d" }
        isLength(definition.d, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dk" }
        isLength(definition.dk, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "k" }
        isLength(definition.k, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "hf" }
        isLength(definition.hf, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dsn" }
        isLength(definition.dsn, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "snw" }
        isLength(definition.snw, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "ds" }
        isLength(definition.ds, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dtn" }
        isLength(definition.dtn, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "tnw" }
        isLength(definition.tnw, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "tnf" }
        isLength(definition.tnf, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "b" }
        isLength(definition.b, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "p" }
        isLength(definition.p, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "t" }
        isLength(definition.t, NONNEGATIVE_LENGTH_BOUNDS);
    }
    {
        const R = definition.dk / 2;
        const hf = definition.hf;
        const rsn = definition.dsn / 2;
        const rs = definition.ds / 2;
        const rtn = definition.dtn / 2;
        const f = definition.tnf;
        const rm = definition.d / 2;
        const ls = definition.l;
        const top = ls + definition.k;
        const ch = 0.625 * 0.8660254 * definition.p;
        const s45 = sqrt(0.5);
        const neckEnd = ls - definition.snw;
        const body = revolveProfile(context, id + "body", [
                    { "p" : rz(Z, top) },
                    { "p" : rz(R - hf, top) },
                    { "p" : rz(R, top - hf), "mid" : rz(R - hf + hf * s45, top - hf + hf * s45) },
                    { "p" : rz(R, ls) },
                    { "p" : rz(rsn + hf, ls) },
                    { "p" : rz(rsn, ls - hf), "mid" : rz(rsn + hf - hf * s45, ls - hf + hf * s45) },
                    { "p" : rz(rsn, neckEnd) },
                    { "p" : rz(rs, neckEnd - (rs - rsn)) },
                    { "p" : rz(rs, Z) },
                    { "p" : rz(rtn + f, Z) },
                    { "p" : rz(rtn, -f), "mid" : rz(rtn + f - f * s45, -f + f * s45) },
                    { "p" : rz(rtn, -definition.tnw) },
                    { "p" : rz(rm - ch, -definition.tnw) },
                    { "p" : rz(rm, -definition.tnw - ch) },
                    { "p" : rz(rm, -definition.b + ch) },
                    { "p" : rz(rm - ch, -definition.b) },
                    { "p" : rz(Z, -definition.b) }
                ]);
        // The socket: a hexagon s across flats, t deep, then a 140 degree
        // drill point from its inscribed circle.
        const sock = prismZ(context, id + "socket", top - definition.t, definition.t + definition.k, polygonPoints(6, definition.s, 1));
        const rc = definition.s / 2;
        const tip = revolveProfile(context, id + "drill", [{ "p" : rz(Z, top - definition.t) }, { "p" : rz(rc, top - definition.t) }, { "p" : rz(Z, top - definition.t - rc / tan(70 * degree)) }]);
        subtract(context, id + "cut", body, qUnion([sock, tip]));
    });

/**
 * Live's NAS 1129 self-clinching standoffs (raw /features): one revolved
 * sketch hanging below z = 0 (live's "Near" on the top): the top recessed
 * 0.002 in inside the clinch diameter dc, the hex s (its corners extruded,
 * dc / 10 high), a clinch groove (0.95 dc, dc / 10 high) under it, the body dc
 * down to l. Blind (thru 0): a minor-diameter hole hd deep from the bottom
 * with a 135 degree drill point, its entrance chamfered (dmaj - dm) / 2.
 * Thru (thru 1): the minor-diameter hole from the top, its entrance
 * chamfered the same, and from the bottom a bore db lb long ending in a 45
 * degree step.
 */
annotation { "Feature Type Name" : "Standoff" }
export const standoff = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Thru" }
        isInteger(definition.thru, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dm" }
        isLength(definition.dm, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dmaj" }
        isLength(definition.dmaj, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dc" }
        isLength(definition.dc, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "hd" }
        isLength(definition.hd, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "db" }
        isLength(definition.db, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "lb" }
        isLength(definition.lb, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "bd" }
        isLength(definition.bd, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
    }
    {
        const step = 0.002 * inch;
        const g = definition.dc / 10;
        const rh = definition.s / 2;
        const rc = definition.dc / 2;
        const rg = 0.95 * definition.dc / 2;
        const rm = definition.dm / 2;
        const ch = (definition.dmaj - definition.dm) / 2;
        const l = definition.l;
        const outer = [{ "p" : rz(rc, -step) }, { "p" : rz(rc, Z) }, { "p" : rz(rh, Z) }, { "p" : rz(rh, -g) }, { "p" : rz(rg, -g) }, { "p" : rz(rg, -2 * g) }, { "p" : rz(rc, -2 * g) }, { "p" : rz(rc, -l) }];
        var segments;
        if (definition.thru == 1)
        {
            const rb = definition.db / 2;
            const lb = definition.lb;
            segments = join(join([{ "p" : rz(rm + ch, -step) }], outer),
                    [{ "p" : rz(rb, -l) }, { "p" : rz(rb, -l + lb) }, { "p" : rz(rm, -l + lb + (rb - rm)) }, { "p" : rz(rm, -step - ch) }]);
        }
        else
        {
            const apex = rm / tan(67.5 * degree);
            segments = join(join([{ "p" : rz(Z, -step) }], outer),
                    [{ "p" : rz(rm + ch, -l) }, { "p" : rz(rm, -l + ch) }, { "p" : rz(rm, -l + definition.hd) }, { "p" : rz(Z, -l + definition.hd + apex) }]);
        }
        revolveProfile(context, id + "body", segments);
        // The hex (its corners outside the s / 2 collar): a hex ring round
        // the top recess, united with the body.
        const ring = prismZ(context, id + "hex", -g, g, polygonPoints(6, definition.s, 1));
        subtract(context, id + "trim", ring, cylinderZ(context, id + "recess", -g - step, step, rc));
        uniteAll(context, id);
    });

/**
 * Live's NASM 45938/4 self-clinching flush nut (raw /features): a hex plate
 * s across flats, a / 6.4 high, above z = 0 (live's "Near" on its underside)
 * with the minor-diameter hole; under it a revolved clinch shank tapering
 * from 0.823 dc to dc and then straight for ax, a - a 5/32 deep in all; both
 * hole entrances chamfered (dmaj - dm) / 2.
 */
annotation { "Feature Type Name" : "Flush nut" }
export const flushNut = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "dm" }
        isLength(definition.dm, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dmaj" }
        isLength(definition.dmaj, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dc" }
        isLength(definition.dc, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "a" }
        isLength(definition.a, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "ax" }
        isLength(definition.ax, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
    }
    {
        const rm = definition.dm / 2;
        const ch = (definition.dmaj - definition.dm) / 2;
        const hh = definition.a * 5 / 32;
        const depth = definition.a - hh;
        const r1 = 0.823 * definition.dc / 2;
        const rc = definition.dc / 2;
        prismZ(context, id + "hex", Z, hh, polygonPoints(6, definition.s, 1));
        revolveProfile(context, id + "shank", [{ "p" : rz(Z, Z) }, { "p" : rz(r1, Z) }, { "p" : rz(rc, -(depth - definition.ax)) }, { "p" : rz(rc, -depth) }, { "p" : rz(Z, -depth) }]);
        uniteAll(context, id);
        const part = solidsOf(context, id);
        // The hole, chamfered at both ends.
        const hole = revolveProfile(context, id + "hole", [{ "p" : rz(Z, hh + EPS) }, { "p" : rz(rm + ch + EPS, hh + EPS) }, { "p" : rz(rm, hh - ch) }, { "p" : rz(rm, -depth + ch) }, { "p" : rz(rm + ch + EPS, -depth - EPS) }, { "p" : rz(Z, -depth - EPS) }]);
        subtract(context, id + "cut", part, hole);
    });

/**
 * Live's PEM® self-clinching standoffs (raw /features), one revolved sketch
 * hanging below z = 0 (live's "Near" on the top) plus the hex corners:
 * the hex s, dc / 10 high, over an undercut 0.9333 dc for 0.13333 dc, then
 * the body dc with two serration grooves (0.98 dc, dc / 30 wide, dc / 25
 * apart) down to l. Form 0 (SO™ through-hole unthreaded): a bore db
 * through, both entrances chamfered 45° out to 1.11 db. Form 1 (SO™
 * through-hole threaded): the thread's minor diameter dm from the top, its
 * ends chamfered 45° out to dmaj, then from the bottom a bore db lb long.
 * Form 2 (BSO™ blind threaded): the top closed; from the bottom the minor
 * diameter, its entrance chamfered 45° out to dmaj, ending in a 120 degree
 * drill point 0.9 l up. Form 3 (TSO™ for thin sheets): the undercut's top
 * corner filled by a dc / 50 chamfer; the minor diameter through, both ends
 * chamfered out to dmaj (live's STYLE A), or with bd > 0 (STYLE B) blind
 * from the bottom up to a 118 degree point bd below the top. Form 4 (DSO™
 * for close-to-edge applications): the head round (s across), knurled by 40
 * V teeth down to ((s - dc) / 2 + dc) / 2, their tips on its rim; the hole
 * as TSO's STYLE A. Form 5 (SOSG™ / SOAG™ grounding standoffs): one
 * serration groove, the hex's flats facing ±X, the hole as STYLE A, and 32
 * V notches tt up from the bottom (valleys 0.95 dc, tips on the body).
 */
annotation { "Feature Type Name" : "PEM standoff" }
export const pemStandoff = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "s" }
        isLength(definition.s, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dc" }
        isLength(definition.dc, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "db" }
        isLength(definition.db, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dm" }
        isLength(definition.dm, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dmaj" }
        isLength(definition.dmaj, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "lb" }
        isLength(definition.lb, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "bd" }
        isLength(definition.bd, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "tt" }
        isLength(definition.tt, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
    }
    {
        const dc = definition.dc;
        const g = dc / 10;
        const rh = definition.s / 2;
        const rc = dc / 2;
        const ru = 0.9333 * dc / 2;
        const rs = 0.98 * dc / 2;
        const z1 = -g - 0.13333 * dc;
        const w = dc / 30;
        const gap = dc / 25;
        const l = definition.l;
        const rb = definition.db / 2;
        // The top of the hole (its chamfer's outer radius) and the inside.
        var rt;
        var inside;
        if (definition.form >= 3)
        {
            const rmaj = definition.dmaj / 2;
            const rmin = definition.dm / 2;
            const bd = definition.bd;
            if (bd > EPS)
            {
                rt = Z;
                inside = [{ "p" : rz(rmaj, -l) }, { "p" : rz(rmin, -l + (rmaj - rmin)) }, { "p" : rz(rmin, -bd - rmin / tan(59 * degree)) }, { "p" : rz(Z, -bd) }];
            }
            else
            {
                rt = rmaj;
                inside = [{ "p" : rz(rmaj, -l) }, { "p" : rz(rmin, -l + (rmaj - rmin)) }, { "p" : rz(rmin, -(rmaj - rmin)) }];
            }
        }
        else if (definition.form == 2)
        {
            const rmaj = definition.dmaj / 2;
            const rmin = definition.dm / 2;
            const tip = -l + 0.9 * l;
            rt = Z;
            inside = [{ "p" : rz(rmaj, -l) }, { "p" : rz(rmin, -l + (rmaj - rmin)) }, { "p" : rz(rmin, tip - rmin / tan(60 * degree)) }, { "p" : rz(Z, tip) }];
        }
        else if (definition.form == 1)
        {
            const rmaj = definition.dmaj / 2;
            const rmin = definition.dm / 2;
            const lb = definition.lb;
            rt = rmaj;
            inside = [{ "p" : rz(rb, -l) }, { "p" : rz(rb, -l + lb) }, { "p" : rz(rmaj, -l + lb) }, { "p" : rz(rmin, -l + lb + (rmaj - rmin)) }, { "p" : rz(rmin, -(rmaj - rmin)) }];
        }
        else
        {
            rt = 1.11 * definition.db / 2;
            inside = [{ "p" : rz(rt, -l) }, { "p" : rz(rb, -l + (rt - rb)) }, { "p" : rz(rb, -(rt - rb)) }];
        }
        // TSO™: the undercut's top corner filled by a dc / 50 chamfer.
        const c1 = definition.form == 3 ? dc / 50 : Z;
        const under = c1 > EPS ? [{ "p" : rz(ru + c1, -g) }, { "p" : rz(ru, -g - c1) }] : [{ "p" : rz(ru, -g) }];
        const grooves = definition.form == 5 ? 1 : 2;
        var outer = join(join([{ "p" : rz(rt, Z) }, { "p" : rz(rh, Z) }, { "p" : rz(rh, -g) }], under), [{ "p" : rz(ru, z1) }, { "p" : rz(rc, z1) }]);
        for (var k = 0; k < grooves; k += 1)
        {
            const z = z1 - (k + 1) * gap - k * w;
            outer = join(outer, [{ "p" : rz(rc, z) }, { "p" : rz(rs, z) }, { "p" : rz(rs, z - w) }, { "p" : rz(rc, z - w) }]);
        }
        // Short lengths end inside the serrations: the outside stops at l.
        var kept = [];
        for (var q in outer)
        {
            if (q.p[1] > -l + EPS)
                kept = append(kept, q);
        }
        kept = append(kept, { "p" : rz(kept[size(kept) - 1].p[0], -l) });
        revolveProfile(context, id + "body", join(kept, inside));
        if (definition.form == 4)
        {
            knurlCut(context, id, qCreatedBy(id + "body", EntityType.BODY), -g, g, 40, ((definition.s - dc) / 2 + dc) / 2, rh, (90 + 180 / 40) * degree);
            return;
        }
        // The hex corners: a hex ring round the top of the bore, united.
        const ring = prismZ(context, id + "hex", -g, g, polygonPoints(6, definition.s, definition.form == 5 ? 0 : 1));
        // (A closed top: cut the ring inside the body instead.)
        subtract(context, id + "trim", ring, cylinderZ(context, id + "bore", -g - g, g, rt > EPS ? rt : rc));
        uniteAll(context, id);
        if (definition.form == 5)
            knurlCut(context, id + "teeth", solidsOf(context, id), -l, definition.tt, 32, 0.95 * rc, rc, 90 * degree);
    });

/**
 * Live's PEM® TPS™ / TP4™ flush-head pilot pins (raw /features and body
 * details): z = 0 on the head top (live's "Near"), the head hd across with
 * its rim rounded 0.021 hd, 0.18 sd thick; a collar (p + hd) / 2 across
 * 0.09 sd high; the hole-size band hs with a V groove (0.075 p wide, sides
 * 2:1, its bottom rounded 0.025 p) centred on it; a 45° step to the pin p at
 * 0.7 sd; the pin to l, its end tapered 26° to dd with a 120 degree dimple
 * 0.3 p across (form 0, TPS™); form 1 (TPXS™): tapered 15° to dd, the
 * dimple p / 6 across. Form 2 (FH™ flush-head pins): the head hd / 10
 * thick, its rim a half circle; the displacer r2 = (r1 + R - hd / 20) / 2
 * down to 0.3 sd, a cone in to r1 = (p + r2) / 2 at hd / 5, a half-round
 * groove p / 20 centred on r1's band, a 45° step to the pin at sd, the
 * dimple as form 0; 12 serration slots (±7.5° on r1, walls at ±15°) cut
 * under the head, then a disc (r1 + r2) / 2 across, hd / 20 deep, added back.
 * Form 3 (FH™ flush-head studs, sd = the thread start ts): r1 the major
 * radius (no step), the disc 0.075 ts deep, a half-round groove p / 20
 * midway from it to ts, the face split at ts, the end chamfered 0.625 of the
 * thread's triangle height (pitch pt) with the dimple 0.3 p across. Form 4
 * (FHL™ low-displacement head studs): a flat head 0.2 hd thick, its top edge
 * rounded 0.025 hd, 8 slots (r1 sin 7.5° each side of every 45°) cut 0.075 hd
 * up into it between the major radius and its rim; a flat-bottomed V groove
 * (0.075 p wide, 0.05 p deep) midway from the head to ts, the face split at
 * ts, the end as form 3. Form 5 (TFH™ non-flush studs, td = T_DIM): a head
 * td thick with a 45° dimple 0.15 hd across on top, a 45° collar under it to
 * 1.05 (hd + p) / 4, cut by 8 slots td wide from (hd + p) / 4 out; the groove
 * as form 4 midway from the collar to ts, l and ts from the head's
 * underside. Form 6 (HFLH™ hard panel studs, st = the sheet thickness): a
 * head td thick, its top edge rounded 0.2 td, a collar st / 2 under it with
 * its outer edge rounded st / 2, cut by 8 slots td wide from 1.15 p / 2 out;
 * the groove as form 5 with its bottom rounded 0.025 p. Forms 7 (HFH™,
 * HF109™, HFG8™ heavy-duty studs) and 8 (HFE™ / THFE™): as form 6 with a
 * 45° collar, the head cut to 8 flats (chords hd / 4) facing the slots, the
 * slots td (form 8: hd / 8) wide. Form 9 (FHX™ studs with the X-Press™
 * profile, sd = S_DIM, pt = its taper's length): form 3's serrations and
 * disc, the head's edges rounded 0.02 hd, a collar 1.15 p / 2 coning at 60°
 * to p / 2 at hd / 5; a neck 0.9 p from 0.6 sd to sd between 45° flanks; the
 * end tapered at 25° over pt; no split. Form 10 (SGPC™ swaging collar
 * studs): a head td thick, its edges rounded 0.3 td; under it an undercut
 * td / 10 tall, (hd + p) / 40 deep, then a collar (hd + p) / 4 down to 4 td,
 * its edges chamfered 0.3 td, knurled by 28 V teeth 0.3 td deep; the split
 * and the end as form 5.
 */
annotation { "Feature Type Name" : "PEM pin" }
export const pemPin = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "hd" }
        isLength(definition.hd, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "p" }
        isLength(definition.p, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dd" }
        isLength(definition.dd, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "sd" }
        isLength(definition.sd, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "hs" }
        isLength(definition.hs, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "pt" }
        isLength(definition.pt, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "td" }
        isLength(definition.td, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
        annotation { "Name" : "st" }
        isLength(definition.st, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
    }
    {
        const p = definition.p;
        const sd = definition.sd;
        const l = definition.l;
        const R = definition.hd / 2;
        const rf = 0.021 * definition.hd;
        const hk = 0.18 * sd;
        const rcol = (p + definition.hd) / 4;
        const zcol = -hk - 0.09 * sd;
        const rh = definition.hs / 2;
        const rp = p / 2;
        const z45 = -0.7 * sd;
        const zband = z45 + (rh - rp);
        const zc = (zcol + zband) / 2;
        const hw = 0.0375 * p;
        const ig = 0.0125 * p;
        const dep = 2 * (hw - ig);
        const rg = 0.025 * p;
        const cx = rh - dep + sqrt(rg * rg - ig * ig);
        const re = definition.dd / 2;
        if (definition.form == 2 || definition.form == 3 || definition.form == 9)
        {
            flushHeadPin(context, id, definition);
            return;
        }
        if (definition.form == 4)
        {
            lowDisplacementStud(context, id, definition);
            return;
        }
        if (definition.form >= 5 && definition.form <= 8)
        {
            nonFlushStud(context, id, definition);
            return;
        }
        if (definition.form == 10)
        {
            swagingCollarStud(context, id, definition);
            return;
        }
        const tpxs = definition.form == 1;
        const zt = -l + (rp - re) / tan((tpxs ? 15 : 26) * degree);
        const rd = tpxs ? p / 12 : 0.15 * p;
        const s45 = sqrt(0.5);
        revolveProfile(context, id + "body", [
                    { "p" : rz(Z, Z) }, { "p" : rz(R - rf, Z) }, { "p" : rz(R, -rf), "mid" : rz(R - rf + rf * s45, -rf + rf * s45) },
                    { "p" : rz(R, -hk + rf) }, { "p" : rz(R - rf, -hk), "mid" : rz(R - rf + rf * s45, -hk + rf - rf * s45) },
                    { "p" : rz(rcol, -hk) }, { "p" : rz(rcol, zcol) }, { "p" : rz(rh, zcol) }, { "p" : rz(rh, zc + hw) },
                    { "p" : rz(rh - dep, zc + ig) }, { "p" : rz(rh - dep, zc - ig), "mid" : rz(cx - rg, zc) }, { "p" : rz(rh, zc - hw) },
                    { "p" : rz(rh, zband) }, { "p" : rz(rp, z45) }, { "p" : rz(rp, zt) }, { "p" : rz(re, -l) }, { "p" : rz(rd, -l) },
                    { "p" : rz(Z, -l + rd / tan(60 * degree)) }
                ]);
    });

/** pemPin forms 2 (FH™ flush-head pins) and 3 (FH™ flush-head studs). */
function flushHeadPin(context is Context, id is Id, definition is map)
{
    const xp = definition.form == 9;
    const stud = definition.form == 3 || xp;
    const p = definition.p;
    const sd = definition.sd;
    const l = definition.l;
    const R = definition.hd / 2;
    const rr = definition.hd / 20;
    const hk = definition.hd / 10;
    const rp = p / 2;
    const r1 = stud ? rp : (2 * rp + R - rr) / 3;
    const r2 = xp ? 1.15 * rp : (r1 + R - rr) / 2;
    const zcone = -definition.hd / 5;
    const discDepth = stud ? 0.075 * sd : rr;
    const rg = p / 20;
    const rd = 0.15 * p;
    var tail;
    if (xp)
    {
        const rn = 0.9 * rp;
        const t25 = definition.pt * tan(25 * degree);
        tail = [{ "p" : rz(r1, -0.6 * sd + rp - rn) }, { "p" : rz(rn, -0.6 * sd) }, { "p" : rz(rn, -sd) }, { "p" : rz(r1, -sd - rp + rn) },
                { "p" : rz(r1, -l + definition.pt) }, { "p" : rz(r1 - t25, -l) }];
    }
    else if (stud)
    {
        const zg = (-hk - discDepth - sd) / 2;
        const ch = 0.625 * 0.8660254 * definition.pt;
        tail = [{ "p" : rz(r1, zg + rg) }, { "p" : rz(r1, zg - rg), "mid" : rz(r1 - rg, zg) }, { "p" : rz(r1, -l + ch) }, { "p" : rz(r1 - ch, -l) }];
    }
    else
    {
        const z45 = -sd + (r1 - rp);
        const zg = (zcone + z45) / 2;
        tail = [{ "p" : rz(r1, zg + rg) }, { "p" : rz(r1, zg - rg), "mid" : rz(r1 - rg, zg) }, { "p" : rz(r1, z45) }, { "p" : rz(rp, -sd) }, { "p" : rz(rp, -l) }];
    }
    const f = 0.02 * definition.hd;
    const s45 = sqrt(0.5);
    const head = xp ? [{ "p" : rz(Z, Z) }, { "p" : rz(R - f, Z) }, { "p" : rz(R, -f), "mid" : rz(R - f + f * s45, -f + f * s45) },
                { "p" : rz(R, -hk + f) }, { "p" : rz(R - f, -hk), "mid" : rz(R - f + f * s45, -hk + f - f * s45) },
                { "p" : rz(r2, -hk) }, { "p" : rz(r2, zcone + (r2 - r1) / tan(60 * degree)) }, { "p" : rz(r1, zcone) }] :
        [{ "p" : rz(Z, Z) }, { "p" : rz(R - rr, Z) }, { "p" : rz(R - rr, -hk), "mid" : rz(R, -rr) },
                { "p" : rz(r2, -hk) }, { "p" : rz(r2, -0.3 * sd) }, { "p" : rz(r1, zcone) }];
    revolveProfile(context, id + "body", join(join(head, tail),
                [{ "p" : rz(rd, -l) }, { "p" : rz(Z, -l + rd / tan(60 * degree)) }]));
    // The serration slots: 12 wedges outside r1 under the head (one sketch,
    // one extrude), then live's Sketch 3 disc added back: (r1 + r2) / 2
    // across (its r2 circle is construction), hd / 20 deep. (~2 s a part:
    // the booleans on the slotted body.)
    var under = XY_PLANE;
    under.origin[2] = -hk;
    const far = 2 * R;
    const sk = newSketchOnPlane(context, id + "slotSketch", { "sketchPlane" : under });
    for (var k = 0; k < 12; k += 1)
    {
        const c = (90 + 30 * k) * degree;
        const a = vector(cos(c - 7.5 * degree), sin(c - 7.5 * degree)) * r1;
        const b = vector(cos(c + 7.5 * degree), sin(c + 7.5 * degree)) * r1;
        const a2 = a + vector(cos(c - 15 * degree), sin(c - 15 * degree)) * far;
        const b2 = b + vector(cos(c + 15 * degree), sin(c + 15 * degree)) * far;
        skArc(sk, "arc" ~ k, { "start" : a, "mid" : vector(cos(c), sin(c)) * r1, "end" : b });
        skLineSegment(sk, "wb" ~ k, { "start" : b, "end" : b2 });
        skLineSegment(sk, "out" ~ k, { "start" : b2, "end" : a2 });
        skLineSegment(sk, "wa" ~ k, { "start" : a2, "end" : a });
    }
    skSolve(sk);
    opExtrude(context, id + "slots", { "entities" : qSketchRegion(id + "slotSketch"), "direction" : vector(0, 0, -1), "endBound" : BoundingType.BLIND, "endDepth" : sd });
    opDeleteBodies(context, id + "deleteSlotSketch", { "entities" : qCreatedBy(id + "slotSketch", EntityType.BODY) });
    subtract(context, id + "cut", qCreatedBy(id + "body", EntityType.BODY), qCreatedBy(id + "slots", EntityType.BODY));
    revolveProfile(context, id + "ring", [{ "p" : rz(Z, -hk) }, { "p" : rz((r1 + r2) / 2, -hk) }, { "p" : rz((r1 + r2) / 2, -hk - discDepth) }, { "p" : rz(Z, -hk - discDepth) }]);
    uniteAll(context, id);
    // A stud's shank face split at the thread start (live's Split 1).
    if (definition.form == 3)
    {
        opPlane(context, id + "threadPlane", { "plane" : plane(vector(Z, Z, -sd), vector(0, 0, 1)), "width" : 4 * R, "height" : 4 * R });
        try silent
        {
            opSplitFace(context, id + "threadSplit", { "faceTargets" : qGeometry(qOwnedByBody(solidsOf(context, id), EntityType.FACE), GeometryType.CYLINDER),
                        "faceTools" : qCreatedBy(id + "threadPlane", EntityType.FACE), "keepToolSurfaces" : false });
        }
        opDeleteBodies(context, id + "deleteThreadPlane", { "entities" : qCreatedBy(id + "threadPlane", EntityType.BODY) });
    }
}

/** pemPin form 4 (FHL™ low-displacement head studs). */
function lowDisplacementStud(context is Context, id is Id, definition is map)
{
    const p = definition.p;
    const ts = definition.sd;
    const l = definition.l;
    const R = definition.hd / 2;
    const f = 0.025 * definition.hd;
    const hk = 0.2 * definition.hd;
    const r1 = p / 2;
    const zc = (-hk - ts) / 2;
    const hw = 0.0375 * p;
    const ig = 0.0125 * p;
    const dep = 0.05 * p;
    const ch = 0.625 * 0.8660254 * definition.pt;
    const rd = 0.15 * p;
    const s45 = sqrt(0.5);
    const zs = -hk + 0.075 * definition.hd;
    // The head, slotted, then the shank from the slots' roofs down: its
    // cylinder runs on up each slot as one face (live's).
    revolveProfile(context, id + "head", [
                { "p" : rz(Z, Z) }, { "p" : rz(R - f, Z) }, { "p" : rz(R, -f), "mid" : rz(R - f + f * s45, -f + f * s45) },
                { "p" : rz(R, -hk) }, { "p" : rz(Z, -hk) }
            ]);
    // The 8 slots in the head's underside: parallel sides r1 sin 7.5° off
    // each 45° radius, from inside the shank out past the rim.
    var under = XY_PLANE;
    under.origin[2] = -hk;
    const w = r1 * sin(7.5 * degree);
    const far = R + hk;
    const sk = newSketchOnPlane(context, id + "slotSketch", { "sketchPlane" : under });
    for (var k = 0; k < 8; k += 1)
    {
        const c = 45 * k * degree;
        const u = vector(cos(c), sin(c));
        const v = vector(-sin(c), cos(c));
        skPolyline(sk, "slot" ~ k, { "points" : [u * (0.6 * r1) - v * w, u * far - v * w, u * far + v * w, u * (0.6 * r1) + v * w, u * (0.6 * r1) - v * w] });
    }
    skSolve(sk);
    opExtrude(context, id + "slots", { "entities" : qSketchRegion(id + "slotSketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : 0.075 * definition.hd });
    opDeleteBodies(context, id + "deleteSlotSketch", { "entities" : qCreatedBy(id + "slotSketch", EntityType.BODY) });
    subtract(context, id + "cut", qCreatedBy(id + "head", EntityType.BODY), qCreatedBy(id + "slots", EntityType.BODY));
    revolveProfile(context, id + "shank", [
                { "p" : rz(Z, zs) }, { "p" : rz(r1, zs) }, { "p" : rz(r1, zc + hw) }, { "p" : rz(r1 - dep, zc + ig) },
                { "p" : rz(r1 - dep, zc - ig) }, { "p" : rz(r1, zc - hw) }, { "p" : rz(r1, -l + ch) }, { "p" : rz(r1 - ch, -l) },
                { "p" : rz(rd, -l) }, { "p" : rz(Z, -l + rd / tan(60 * degree)) }
            ]);
    uniteAll(context, id);
    opPlane(context, id + "threadPlane", { "plane" : plane(vector(Z, Z, -ts), vector(0, 0, 1)), "width" : 4 * R, "height" : 4 * R });
    try silent
    {
        opSplitFace(context, id + "threadSplit", { "faceTargets" : qGeometry(qOwnedByBody(solidsOf(context, id), EntityType.FACE), GeometryType.CYLINDER),
                    "faceTools" : qCreatedBy(id + "threadPlane", EntityType.FACE), "keepToolSurfaces" : false });
    }
    opDeleteBodies(context, id + "deleteThreadPlane", { "entities" : qCreatedBy(id + "threadPlane", EntityType.BODY) });
}

/** pemPin forms 5 (TFH™ non-flush studs), 6 (HFLH™ hard panel studs), 7 and 8 (heavy-duty studs). */
function nonFlushStud(context is Context, id is Id, definition is map)
{
    const p = definition.p;
    const t = definition.td;
    const ts = definition.sd + t;
    const R = definition.hd / 2;
    const r1 = p / 2;
    const heavy = definition.form == 7 || definition.form == 8;
    const hard = definition.form == 6 || heavy;
    const rc = hard ? R - definition.st / 2 : 1.05 * (definition.hd + p) / 4;
    const zb = -t - (R - rc);
    const zl = -t - definition.l;
    const zc = (zb - ts) / 2;
    const hw = 0.0375 * p;
    const ig = 0.0125 * p;
    const dep = 0.05 * p;
    const ch = 0.625 * 0.8660254 * definition.pt;
    const rd = 0.15 * p;
    const rt = 0.075 * definition.hd;
    const s45 = sqrt(0.5);
    const f = 0.2 * t;
    const gr = 0.025 * p;
    const top = heavy ? [{ "p" : rz(Z, Z) }, { "p" : rz(R - f, Z) }, { "p" : rz(R, -f), "mid" : rz(R - f + f * s45, -f + f * s45) }, { "p" : rz(R, -t) },
                { "p" : rz(rc, zb) }] :
        (hard ? [{ "p" : rz(Z, Z) }, { "p" : rz(R - f, Z) }, { "p" : rz(R, -f), "mid" : rz(R - f + f * s45, -f + f * s45) }, { "p" : rz(R, -t) },
                { "p" : rz(rc, zb), "mid" : rz(rc + (R - rc) * s45, -t - (R - rc) * s45) }] :
        [{ "p" : rz(Z, -rt) }, { "p" : rz(rt, Z) }, { "p" : rz(R, Z) }, { "p" : rz(R, -t) }, { "p" : rz(rc, zb) }]);
    const groove = hard ? [{ "p" : rz(r1 - dep, zc + ig) }, { "p" : rz(r1 - dep, zc - ig), "mid" : rz(r1 - dep - gr + gr * cos(30 * degree), zc) }] :
        [{ "p" : rz(r1 - dep, zc + ig) }, { "p" : rz(r1 - dep, zc - ig) }];
    revolveProfile(context, id + "body", concatenateArrays([top, [{ "p" : rz(r1, zb) }, { "p" : rz(r1, zc + hw) }], groove, [
                { "p" : rz(r1, zc - hw) }, { "p" : rz(r1, zl + ch) }, { "p" : rz(r1 - ch, zl) }, { "p" : rz(rd, zl) },
                { "p" : rz(Z, zl + rd / tan(60 * degree)) }
            ]]));
    // The 8 slots through the collar: td wide, from (hd + p) / 4 (HFLH:
    // 1.15 p / 2) out past the rim.
    var under = XY_PLANE;
    under.origin[2] = zb;
    const w = definition.form == 8 ? definition.hd / 16 : t / 2;
    const ri = hard ? 1.15 * p / 2 : (definition.hd + p) / 4;
    const far = R + t;
    const sk = newSketchOnPlane(context, id + "slotSketch", { "sketchPlane" : under });
    for (var k = 0; k < 8; k += 1)
    {
        const c = 45 * k * degree;
        const u = vector(cos(c), sin(c));
        const v = vector(-sin(c), cos(c));
        const a = u * sqrt(ri * ri - w * w) - v * w;
        const b = u * sqrt(ri * ri - w * w) + v * w;
        skArc(sk, "arc" ~ k, { "start" : a, "mid" : u * ri, "end" : b });
        skLineSegment(sk, "sb" ~ k, { "start" : b, "end" : u * far + v * w });
        skLineSegment(sk, "out" ~ k, { "start" : u * far + v * w, "end" : u * far - v * w });
        skLineSegment(sk, "sa" ~ k, { "start" : u * far - v * w, "end" : a });
    }
    skSolve(sk);
    opExtrude(context, id + "slots", { "entities" : qSketchRegion(id + "slotSketch"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : -t - zb });
    opDeleteBodies(context, id + "deleteSlotSketch", { "entities" : qCreatedBy(id + "slotSketch", EntityType.BODY) });
    subtract(context, id + "cut", qCreatedBy(id + "body", EntityType.BODY), qCreatedBy(id + "slots", EntityType.BODY));
    if (heavy)
    {
        // The 8 flats: chords hd / 4 long, down through the head into the
        // collar.
        const flats = prismZ(context, id + "flats", zl - t, -zl + 2 * t, polygonPoints(8, 2 * sqrt(R * R - definition.hd * definition.hd / 64), 0));
        intersect(context, id + "flatCut", qCreatedBy(id + "body", EntityType.BODY), flats);
    }
    opPlane(context, id + "threadPlane", { "plane" : plane(vector(Z, Z, -ts), vector(0, 0, 1)), "width" : 4 * R, "height" : 4 * R });
    try silent
    {
        opSplitFace(context, id + "threadSplit", { "faceTargets" : qGeometry(qOwnedByBody(solidsOf(context, id), EntityType.FACE), GeometryType.CYLINDER),
                    "faceTools" : qCreatedBy(id + "threadPlane", EntityType.FACE), "keepToolSurfaces" : false });
    }
    opDeleteBodies(context, id + "deleteThreadPlane", { "entities" : qCreatedBy(id + "threadPlane", EntityType.BODY) });
}

/**
 * Cut a knurl of `n` V teeth into `target` from z0 up h: everything outside
 * the star whose valleys are at rIn (the first at angle a0) and its tips at
 * rOut, halfway between.
 */
function knurlCut(context is Context, id is Id, target is Query, z0 is ValueWithUnits, h is ValueWithUnits, n is number, rIn is ValueWithUnits, rOut is ValueWithUnits, a0 is ValueWithUnits)
{
    var base = XY_PLANE;
    base.origin[2] = z0;
    const sk = newSketchOnPlane(context, id + "knurlSketch", { "sketchPlane" : base });
    var star = [];
    for (var k = 0; k < n; k += 1)
    {
        const a = a0 + 360 / n * k * degree;
        star = append(star, vector(cos(a), sin(a)) * rIn);
        star = append(star, vector(cos(a + 180 / n * degree), sin(a + 180 / n * degree)) * rOut);
    }
    skPolyline(sk, "star", { "points" : append(star, star[0]) });
    skCircle(sk, "out", { "center" : vector(Z, Z), "radius" : 2 * rOut });
    skSolve(sk);
    opExtrude(context, id + "knurl", { "entities" : qSketchRegion(id + "knurlSketch", true), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : h });
    opDeleteBodies(context, id + "deleteKnurlSketch", { "entities" : qCreatedBy(id + "knurlSketch", EntityType.BODY) });
    subtract(context, id + "knurlCut", target, qCreatedBy(id + "knurl", EntityType.BODY));
}

/** pemPin form 10 (SGPC™ swaging collar studs). */
function swagingCollarStud(context is Context, id is Id, definition is map)
{
    const p = definition.p;
    const t = definition.td;
    const ts = definition.sd + t;
    const R = definition.hd / 2;
    const r1 = p / 2;
    const rc = (definition.hd + p) / 4;
    const ru = rc - (definition.hd + p) / 40;
    const f = 0.3 * t;
    const zu = -1.1 * t;
    const zb = -4 * t;
    const zl = -t - definition.l;
    const ch = 0.625 * 0.8660254 * definition.pt;
    const rd = 0.15 * p;
    const s45 = sqrt(0.5);
    revolveProfile(context, id + "body", [
                { "p" : rz(Z, Z) }, { "p" : rz(R - f, Z) }, { "p" : rz(R, -f), "mid" : rz(R - f + f * s45, -f + f * s45) },
                { "p" : rz(R, -t + f) }, { "p" : rz(R - f, -t), "mid" : rz(R - f + f * s45, -t + f - f * s45) },
                { "p" : rz(ru, -t) }, { "p" : rz(ru, zu) }, { "p" : rz(rc - f, zu) }, { "p" : rz(rc, zu - f) }, { "p" : rz(rc, zb + f) },
                { "p" : rz(rc - f, zb) }, { "p" : rz(r1, zb) }, { "p" : rz(r1, zl + ch) }, { "p" : rz(r1 - ch, zl) }, { "p" : rz(rd, zl) },
                { "p" : rz(Z, zl + rd / tan(60 * degree)) }
            ]);
    // The knurl: 28 V teeth 0.3 td deep, their tips on the collar.
    knurlCut(context, id, qCreatedBy(id + "body", EntityType.BODY), zb, zu - zb, 28, rc - f, rc, 90 * degree);
    opPlane(context, id + "threadPlane", { "plane" : plane(vector(Z, Z, -ts), vector(0, 0, 1)), "width" : 4 * R, "height" : 4 * R });
    try silent
    {
        opSplitFace(context, id + "threadSplit", { "faceTargets" : qGeometry(qOwnedByBody(solidsOf(context, id), EntityType.FACE), GeometryType.CYLINDER),
                    "faceTools" : qCreatedBy(id + "threadPlane", EntityType.FACE), "keepToolSurfaces" : false });
    }
    opDeleteBodies(context, id + "deleteThreadPlane", { "entities" : qCreatedBy(id + "threadPlane", EntityType.BODY) });
}

/**
 * Live's PEM® self-clinching nuts (raw /features and body details), z = 0 on
 * the shank's top (live's "Near"), the length select being the sheet
 * thickness (unused here; al = its A_LEN). Form 0 (S™ / SS™ / CLS™ / CLSS™ /
 * SP™): the shank dc across down to tl / 13, coning in at 9° to al / 2; a
 * ring (dc + de) / 2 across down to al, knurled by 24 V teeth (valleys
 * midway to the shank's foot); the body de across tl high, its outer edges
 * rounded de / 40; the thread's minor diameter through, chamfered 45° out to
 * the major at both ends; "PEM" engraved md deep in the bottom face (letters
 * 120° apart, read from below, between 0.381 mm off the chamfer and 0.381 mm
 * inside the rounding).
 */
annotation { "Feature Type Name" : "PEM nut" }
export const pemNut = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Form" }
        isInteger(definition.form, { (unitless) : [0, 0, 100] } as IntegerBoundSpec);
        annotation { "Name" : "l" }
        isLength(definition.l, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dm" }
        isLength(definition.dm, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dmaj" }
        isLength(definition.dmaj, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "dc" }
        isLength(definition.dc, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "de" }
        isLength(definition.de, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "tl" }
        isLength(definition.tl, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "al" }
        isLength(definition.al, NONNEGATIVE_LENGTH_BOUNDS);
        annotation { "Name" : "md" }
        isLength(definition.md, NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS);
    }
    {
        const rmaj = definition.dmaj / 2;
        const rmin = definition.dm / 2;
        const rc = definition.dc / 2;
        const re = definition.de / 2;
        const rk = (definition.dc + definition.de) / 4;
        const al = definition.al;
        const tl = definition.tl;
        const f = definition.de / 40;
        const zb = -al - tl;
        const z1 = -tl / 13;
        const r2 = rc - (al / 2 - tl / 13) * tan(9 * degree);
        const s45 = sqrt(0.5);
        revolveProfile(context, id + "body", [
                    { "p" : rz(rmaj, Z) }, { "p" : rz(rc, Z) }, { "p" : rz(rc, z1) }, { "p" : rz(r2, -al / 2) }, { "p" : rz(rk, -al / 2) },
                    { "p" : rz(rk, -al) }, { "p" : rz(re - f, -al) }, { "p" : rz(re, -al - f), "mid" : rz(re - f + f * s45, -al - f + f * s45) },
                    { "p" : rz(re, zb + f) }, { "p" : rz(re - f, zb), "mid" : rz(re - f + f * s45, zb + f - f * s45) },
                    { "p" : rz(rmaj, zb) }, { "p" : rz(rmin, zb + rmaj - rmin) }, { "p" : rz(rmin, -(rmaj - rmin)) }
                ]);
        knurlCut(context, id, qCreatedBy(id + "body", EntityType.BODY), -al, al / 2, 24, (r2 + rk) / 2, rk, 90 * degree);
        // The marking: each letter's box from 0.381 mm off the chamfer to
        // 0.381 mm inside the rounding, centred (live's widths per height),
        // on a plane facing down turned 0°, -120°, 120°. Open Sans' box
        // height is its ascender: 2189 / 1462 of the cap height.
        const gap = 0.381 * millimeter;
        const h = re - f - rmaj - 2 * gap;
        if (definition.md > EPS && h > EPS)
        {
            const letters = [["P", 0, 0.85502], ["E", -120, 0.77907], ["M", 120, 1.20842]];
            for (var k = 0; k < 3; k += 1)
            {
                const a = letters[k][1] * degree;
                const w = letters[k][2] * h;
                const sk = newSketchOnPlane(context, id + ("mark" ~ k), { "sketchPlane" : plane(vector(Z, Z, zb), vector(0, 0, -1), vector(cos(a), -sin(a), 0)) });
                skText(sk, "text", { "text" : letters[k][0], "fontName" : "OpenSans-Regular.ttf",
                            "firstCorner" : vector(-w / 2, rmaj + gap), "secondCorner" : vector(w / 2, rmaj + gap + h * 2189 / 1462) });
                skSolve(sk);
                opExtrude(context, id + ("engrave" ~ k), { "entities" : qSketchRegion(id + ("mark" ~ k), true), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : definition.md });
                opDeleteBodies(context, id + ("deleteMark" ~ k), { "entities" : qCreatedBy(id + ("mark" ~ k), EntityType.BODY) });
                subtract(context, id + ("cutMark" ~ k), qCreatedBy(id + "body", EntityType.BODY), qCreatedBy(id + ("engrave" ~ k), EntityType.BODY));
            }
        }
    });
