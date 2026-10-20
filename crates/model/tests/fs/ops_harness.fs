FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");
import(path : "onshape/std/splineUtils.fs", version : "3095.0");
import(path : "onshape/std/path.fs", version : "3095.0");
import(path : "onshape/std/fitSpline.fs", version : "3095.0");

// Operations and evaluations called directly (tests/custom_features.rs
// ops_and_evaluations_run_in_custom_features). Prints "name ok" or the problem.
function check(name is string, ok is boolean, detail)
{
    println(name ~ (ok ? " ok" : " FAILED " ~ detail));
}

function step(context is Context, id is Id, name is string, f is function)
{
    try
    {
        f(id + name);
    }
    catch (e)
    {
        println(name ~ " threw " ~ e);
    }
}

function near(a, b) returns boolean
{
    return abs(a - b) < 1e-9 * meter ^ 3 + abs(b) * 1e-6;
}

annotation { "Feature Type Name" : "Ops harness" }
export const opsHarness = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const mm = millimeter;
        step(context, id, "box", function(sub) {
            fCuboid(context, sub, { "corner1" : vector(0, 0, 0) * mm, "corner2" : vector(10, 10, 10) * mm });
            const v = evVolume(context, { "entities" : qCreatedBy(sub, EntityType.BODY) });
            check("box", near(v, 1000 * mm ^ 3), v);
            check("evArea", abs(evArea(context, { "entities" : qCreatedBy(sub, EntityType.FACE) }) - 600 * mm ^ 2) < 1e-12 * meter ^ 2, "");
            const box = evBox3d(context, { "topology" : qCreatedBy(sub, EntityType.BODY) });
            check("evBox3d", tolerantEquals(box.maxCorner, vector(10, 10, 10) * mm), box);
            const edge = qContainsPoint(qCreatedBy(sub, EntityType.EDGE), vector(5, 0, 0) * mm);
            check("evLength", abs(evLength(context, { "entities" : edge }) - 10 * mm) < 1e-12 * meter, evLength(context, { "entities" : edge }));
            const line = evEdgeTangentLine(context, { "edge" : edge, "parameter" : 0.5 });
            check("evEdgeTangentLine", tolerantEquals(line.origin, vector(5, 0, 0) * mm) && parallelVectors(line.direction, vector(1, 0, 0)), line);
            const top = qContainsPoint(qCreatedBy(sub, EntityType.FACE), vector(5, 5, 10) * mm);
            const plane = evFaceTangentPlane(context, { "face" : top, "parameter" : vector(0.5, 0.5) });
            check("evFaceTangentPlane", tolerantEquals(plane.normal, vector(0, 0, 1)), plane);
            check("evDistance", abs(evDistance(context, { "side0" : top, "side1" : vector(5, 5, 30) * mm }).distance - 20 * mm) < 1e-12 * meter, "");
            const surf = evSurfaceDefinition(context, { "face" : top });
            check("evSurfaceDefinition", surf is Plane, surf);
            const curve = evCurveDefinition(context, { "edge" : edge });
            check("evCurveDefinition", curve is Line, curve);
        });
        step(context, id, "revolve", function(sub) {
            const sk = newSketchOnPlane(context, sub + "sk", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, -1, 0), vector(1, 0, 0)) });
            skRectangle(sk, "r", { "firstCorner" : vector(20, 0) * mm, "secondCorner" : vector(25, 10) * mm });
            skSolve(sk);
            opRevolve(context, sub + "rev", { "entities" : qSketchRegion(sub + "sk"), "axis" : line(vector(0, 0, 0) * meter, vector(0, 0, 1)), "angleForward" : 2 * PI * radian });
            const v = evVolume(context, { "entities" : qCreatedBy(sub + "rev", EntityType.BODY) });
            check("revolve", near(v, PI * (25 ^ 2 - 20 ^ 2) * 10 * mm ^ 3), v);
            opDeleteBodies(context, sub + "del", { "entities" : qCreatedBy(sub + "sk", EntityType.BODY) });
            const circle = qNthElement(qGeometry(qCreatedBy(sub + "rev", EntityType.EDGE), GeometryType.CIRCLE), 0);
            const exact = evApproximateBSplineCurve(context, { "edge" : circle });
            const cubic = evApproximateBSplineCurve(context, { "edge" : circle, "forceCubic" : true, "forceNonRational" : true });
            check("evApproximateBSplineCurve", exact.isRational && cubic.degree == 3 && !cubic.isRational, [exact.degree, exact.isRational, cubic.degree, cubic.isRational]);
        });
        step(context, id, "patternTransform", function(sub) {
            opPattern(context, sub, { "entities" : qCreatedBy(id + "box", EntityType.BODY), "transforms" : [transform(vector(0, 0, 20) * mm), transform(vector(0, 0, 40) * mm)], "instanceNames" : ["a", "b"] });
            check("opPattern", size(evaluateQuery(context, qCreatedBy(sub, EntityType.BODY))) == 2, "");
            opTransform(context, sub + "move", { "bodies" : qCreatedBy(sub, EntityType.BODY), "transform" : transform(vector(100, 0, 0) * mm) });
            const b = evBox3d(context, { "topology" : qCreatedBy(sub, EntityType.BODY) });
            check("opTransform", tolerantEquals(b.minCorner[0], 100 * mm), b);
        });
        step(context, id, "fillets", function(sub) {
            opFillet(context, sub + "f", { "entities" : qContainsPoint(qCreatedBy(id + "box", EntityType.EDGE), vector(5, 0, 0) * mm), "radius" : 1 * mm });
            opChamfer(context, sub + "c", { "entities" : qContainsPoint(qCreatedBy(id + "box", EntityType.EDGE), vector(5, 10, 10) * mm), "chamferType" : ChamferType.EQUAL_OFFSETS, "width" : 1 * mm });
            const v = evVolume(context, { "entities" : qCreatedBy(id + "box", EntityType.BODY) });
            check("opFillet+opChamfer", near(v, (1000 - (1 - PI / 4) * 10 - 0.5 * 10) * mm ^ 3), v);
        });
        step(context, id, "shellThicken", function(sub) {
            fCuboid(context, sub + "b", { "corner1" : vector(0, 0, 50) * mm, "corner2" : vector(10, 10, 60) * mm });
            opShell(context, sub + "s", { "entities" : qContainsPoint(qCreatedBy(sub + "b", EntityType.FACE), vector(5, 5, 60) * mm), "thickness" : -1 * mm });
            const v = evVolume(context, { "entities" : qCreatedBy(sub + "b", EntityType.BODY) });
            check("opShell", near(v, (1000 - 8 * 8 * 9) * mm ^ 3), v);
        });
        step(context, id, "planesPoints", function(sub) {
            opPlane(context, sub + "pl", { "plane" : plane(vector(0, 0, 100) * mm, vector(0, 0, 1)) });
            opPoint(context, sub + "pt", { "point" : vector(1, 2, 3) * mm });
            opMateConnector(context, sub + "mc", { "coordSystem" : coordSystem(vector(0, 0, 0) * mm, vector(1, 0, 0), vector(0, 0, 1)), "owner" : qCreatedBy(id + "box", EntityType.BODY) });
            check("opPlane", size(evaluateQuery(context, qCreatedBy(sub + "pl"))) > 0, "");
            check("opPoint", tolerantEquals(evVertexPoint(context, { "vertex" : qCreatedBy(sub + "pt", EntityType.VERTEX) }), vector(1, 2, 3) * mm), "");
            check("evMateConnector", tolerantEquals(evMateConnector(context, { "mateConnector" : qCreatedBy(sub + "mc", EntityType.BODY) }).origin, vector(0, 0, 0) * mm), "");
        });
        step(context, id, "curves", function(sub) {
            opFitSpline(context, sub + "sp", { "points" : [vector(0, 0, 0) * mm, vector(10, 5, 0) * mm, vector(20, 0, 0) * mm] });
            check("opFitSpline", size(evaluateQuery(context, qCreatedBy(sub + "sp", EntityType.EDGE))) == 1, "");
            opHelix(context, sub + "hx", { "axisStart" : vector(0, 0, 0) * mm, "direction" : vector(0, 0, 1), "startPoint" : vector(5, 0, 0) * mm, "interval" : [0, 2], "clockwise" : false, "helicalPitch" : 2 * mm, "spiralPitch" : 0 * mm });
            check("opHelix", size(evaluateQuery(context, qCreatedBy(sub + "hx", EntityType.EDGE))) >= 1, "");
            opPolyline(context, sub + "pl", { "points" : [vector(0, 0, 0) * mm, vector(5, 0, 0) * mm, vector(5, 5, 0) * mm] });
            check("opPolyline", size(evaluateQuery(context, qCreatedBy(sub + "pl", EntityType.EDGE))) == 2, "");
        });
        step(context, id, "split", function(sub) {
            fCuboid(context, sub + "b", { "corner1" : vector(0, 0, 200) * mm, "corner2" : vector(10, 10, 210) * mm });
            opSplitPart(context, sub + "s", { "targets" : qCreatedBy(sub + "b", EntityType.BODY), "tool" : qCreatedBy(makeId("Right"), EntityType.FACE) });
            fCuboid(context, sub + "c", { "corner1" : vector(-5, 0, 300) * mm, "corner2" : vector(5, 10, 310) * mm });
            opSplitPart(context, sub + "t", { "targets" : qCreatedBy(sub + "c", EntityType.BODY), "tool" : qCreatedBy(makeId("Right"), EntityType.FACE) });
            check("opSplitPart", size(evaluateQuery(context, qCreatedBy(sub + "c", EntityType.BODY))) == 2, [size(evaluateQuery(context, qCreatedBy(sub + "c", EntityType.BODY))), size(evaluateQuery(context, qCreatedBy(sub + "t", EntityType.BODY))), size(evaluateQuery(context, qCreatedBy(sub + "c", EntityType.FACE)))]);
        });
        step(context, id, "extrudeLoftSweep", function(sub) {
            const sk = newSketchOnPlane(context, sub + "sk", { "sketchPlane" : plane(vector(0, 0, 400) * mm, vector(0, 0, 1)) });
            skCircle(sk, "c", { "center" : vector(0, 0) * mm, "radius" : 5 * mm });
            skSolve(sk);
            opExtrude(context, sub + "e", { "entities" : qSketchRegion(sub + "sk"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : 10 * mm });
            const v = evVolume(context, { "entities" : qCreatedBy(sub + "e", EntityType.BODY) });
            check("opExtrude", near(v, PI * 25 * 10 * mm ^ 3), v);
            const sk2 = newSketchOnPlane(context, sub + "sk2", { "sketchPlane" : plane(vector(0, 0, 450) * mm, vector(0, 0, 1)) });
            skRectangle(sk2, "r", { "firstCorner" : vector(-5, -5) * mm, "secondCorner" : vector(5, 5) * mm });
            skSolve(sk2);
            opLoft(context, sub + "l", { "profileSubqueries" : [qSketchRegion(sub + "sk"), qSketchRegion(sub + "sk2")], "bodyType" : ToolBodyType.SOLID });
            const lofted = size(evaluateQuery(context, qCreatedBy(sub + "l", EntityType.BODY)));
            check("opLoft", lofted == 1, lofted ~ " bodies");
        });
        step(context, id, "composite", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 500) * mm, "corner2" : vector(5, 5, 505) * mm });
            fCuboid(context, sub + "b", { "corner1" : vector(10, 0, 500) * mm, "corner2" : vector(15, 5, 505) * mm });
            opCreateCompositePart(context, sub + "c", { "bodies" : qUnion([qCreatedBy(sub + "a", EntityType.BODY), qCreatedBy(sub + "b", EntityType.BODY)]), "closed" : true });
            check("opCreateCompositePart", size(evaluateQuery(context, qCreatedBy(sub + "c", EntityType.BODY))) == 1, size(evaluateQuery(context, qCreatedBy(sub + "c", EntityType.BODY))));
        });
        step(context, id, "cluster", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 600) * mm, "corner2" : vector(5, 5, 605) * mm });
            fCuboid(context, sub + "b", { "corner1" : vector(20, 0, 600) * mm, "corner2" : vector(25, 5, 605) * mm });
            fCuboid(context, sub + "c", { "corner1" : vector(40, 0, 600) * mm, "corner2" : vector(45, 5, 615) * mm });
            const clusters = clusterBodies(context, { "bodies" : qUnion([qCreatedBy(sub + "a", EntityType.BODY), qCreatedBy(sub + "b", EntityType.BODY), qCreatedBy(sub + "c", EntityType.BODY)]), "relativeTolerance" : 1e-6 });
            var sizes = [];
            for (var c in clusters)
                sizes = append(sizes, size(c));
            check("clusterBodies", sort(sizes, function(x, y) { return x - y; }) == [1, 2], clusters);
        });
        step(context, id, "splitEdges", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 700) * mm, "corner2" : vector(10, 10, 710) * mm });
            const body = qCreatedBy(sub + "a", EntityType.BODY);
            const before = size(evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)));
            opSplitEdges(context, sub + "s", { "edges" : qContainsPoint(qOwnedByBody(body, EntityType.EDGE), vector(5, 0, 700) * mm), "parameters" : [[0.5]] });
            const after = size(evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)));
            check("opSplitEdges", after == before + 1, [before, after]);
        });
        step(context, id, "extendSheet", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 800) * mm, "corner2" : vector(10, 10, 810) * mm });
            opExtractSurface(context, sub + "x", { "faces" : qContainsPoint(qCreatedBy(sub + "a", EntityType.FACE), vector(5, 5, 810) * mm) });
            const sheet = qCreatedBy(sub + "x", EntityType.BODY);
            const before = evArea(context, { "entities" : sheet });
            opExtendSheetBody(context, sub + "e", { "entities" : sheet, "endCondition" : ExtendEndType.EXTEND_BLIND, "extendDistance" : 1 * mm });
            const after = evArea(context, { "entities" : sheet });
            check("opExtractSurface+opExtendSheetBody", abs(before - 100 * mm ^ 2) < 1e-12 * meter ^ 2 && after > before, [before, after]);
        });
        step(context, id, "fill", function(sub) {
            opPolyline(context, sub + "loop", { "points" : [vector(0, 0, 900) * mm, vector(10, 0, 900) * mm, vector(10, 10, 900) * mm, vector(0, 10, 900) * mm, vector(0, 0, 900) * mm] });
            opFillSurface(context, sub + "f", { "edgesG0" : qCreatedBy(sub + "loop", EntityType.EDGE) });
            const area = evArea(context, { "entities" : qCreatedBy(sub + "f", EntityType.BODY) });
            check("opFillSurface", abs(area - 100 * mm ^ 2) < 1e-9 * meter ^ 2, area);
        });
        step(context, id, "bsurf", function(sub) {
            opPolyline(context, sub + "a", { "points" : [vector(0, 0, 1000) * mm, vector(10, 0, 1000) * mm] });
            opPolyline(context, sub + "b", { "points" : [vector(0, 10, 1000) * mm, vector(10, 10, 1000) * mm] });
            opBoundarySurface(context, sub + "s", { "uProfileSubqueries" : [qCreatedBy(sub + "a", EntityType.EDGE), qCreatedBy(sub + "b", EntityType.EDGE)], "vProfileSubqueries" : [] });
            const area = evArea(context, { "entities" : qCreatedBy(sub + "s", EntityType.BODY) });
            check("opBoundarySurface", abs(area - 100 * mm ^ 2) < 1e-9 * meter ^ 2, area);
        });
        step(context, id, "intersect", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 1100) * mm, "corner2" : vector(10, 10, 1110) * mm });
            fCuboid(context, sub + "b", { "corner1" : vector(5, 5, 1105) * mm, "corner2" : vector(15, 15, 1115) * mm });
            opIntersectFaces(context, sub + "x", { "tools" : qContainsPoint(qCreatedBy(sub + "a", EntityType.FACE), vector(5, 5, 1110) * mm),
                    "targets" : qContainsPoint(qCreatedBy(sub + "b", EntityType.FACE), vector(5, 10, 1110) * mm) });
            const len = evLength(context, { "entities" : qCreatedBy(sub + "x", EntityType.EDGE) });
            check("opIntersectFaces", size(evaluateQuery(context, qCreatedBy(sub + "x", EntityType.EDGE))) > 0 || len > 0 * meter, len);
        });
        step(context, id, "paths", function(sub) {
            opPolyline(context, sub + "open", { "points" : [vector(0, 0, 1200) * mm, vector(10, 0, 1200) * mm, vector(10, 10, 1200) * mm, vector(0, 10, 1200) * mm] });
            opPolyline(context, sub + "loop", { "points" : [vector(0, 0, 1300) * mm, vector(5, 0, 1300) * mm, vector(5, 5, 1300) * mm, vector(0, 0, 1300) * mm] });
            const paths = constructPaths(context, qUnion([qCreatedBy(sub + "open", EntityType.EDGE), qCreatedBy(sub + "loop", EntityType.EDGE)]), {});
            check("constructPaths", size(paths) == 2 && size(paths[0].edges) == 3 && !paths[0].closed && size(paths[1].edges) == 3 && paths[1].closed, paths);
        });
        step(context, id, "faceblend", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 1400) * mm, "corner2" : vector(10, 10, 1410) * mm });
            const before = evVolume(context, { "entities" : qCreatedBy(sub + "c", EntityType.BODY) });
            opFaceBlend(context, sub + "b", { "side1" : qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), vector(5, 5, 1410) * mm),
                    "side2" : qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), vector(10, 5, 1405) * mm), "radius" : 2 * mm });
            const after = evVolume(context, { "entities" : qCreatedBy(sub + "c", EntityType.BODY) });
            check("opFaceBlend", after < before && after > before * 0.99, [before, after]);
        });
        step(context, id, "approx", function(sub) {
            const pts = [vector(0, 0, 0) * mm, vector(10, 5, 0) * mm, vector(20, 0, 0) * mm, vector(30, 5, 0) * mm];
            const curves = approximateSpline(context, { "degree" : 3, "tolerance" : 1e-5 * meter, "isPeriodic" : false, "targets" : [approximationTarget({ "positions" : pts })] });
            opCreateBSplineCurve(context, sub + "c", { "bSplineCurve" : curves[0] });
            const edge = qCreatedBy(sub + "c", EntityType.EDGE);
            const d = evDistance(context, { "side0" : edge, "side1" : vector(10, 5, 0) * mm }).distance;
            check("approximateSpline", size(curves) == 1 && curves[0].degree == 3 && d < 1e-9 * meter, [curves[0].degree, d]);
        });
        step(context, id, "fitspline", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 1500) * mm, "corner2" : vector(10, 10, 1510) * mm });
            const corner = function(p) { return qContainsPoint(qCreatedBy(sub + "c", EntityType.VERTEX), p); };
            fitSpline(context, sub + "s", { "fitType" : FitSplineType.VERTICES,
                    "vertices" : qUnion([corner(vector(0, 0, 1500) * mm), corner(vector(10, 10, 1500) * mm), corner(vector(10, 10, 1510) * mm)]) });
            const len = evLength(context, { "entities" : qCreatedBy(sub + "s", EntityType.EDGE) });
            check("fitSpline (built-in)", len > 24.1 * mm, len);
        });
        step(context, id, "csurf", function(sub) {
            const pts = [vector(0, 0, 1600) * mm, vector(10, 0, 1600) * mm, vector(10, 10, 1600) * mm, vector(0, 10, 1600) * mm, vector(5, 5, 1600) * mm];
            var points = [];
            for (var p in pts)
                points = append(points, { "point" : p });
            opConstrainedSurface(context, sub + "s", { "points" : points });
            const area = evArea(context, { "entities" : qCreatedBy(sub + "s", EntityType.BODY) });
            check("opConstrainedSurface", area > 99 * mm ^ 2 && area < 101 * mm ^ 2, area);
        });
        step(context, id, "isocline", function(sub) {
            opPoint(context, sub + "o", { "point" : vector(0, 0, 1700) * mm });
            fSphere(context, sub + "sp", { "center" : qCreatedBy(sub + "o", EntityType.VERTEX), "radius" : 10 * mm });
            opCreateIsocline(context, sub + "i", { "faces" : qCreatedBy(sub + "sp", EntityType.FACE), "direction" : vector(0, 0, 1), "angle" : 0 * degree });
            const len = evLength(context, { "entities" : qCreatedBy(sub + "i", EntityType.EDGE) });
            check("opCreateIsocline", abs(len - 2 * PI * 10 * mm) < 1e-2 * mm, len);
        });
        step(context, id, "drop", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 1800) * mm, "corner2" : vector(20, 20, 1810) * mm });
            opPolyline(context, sub + "l", { "points" : [vector(5, 5, 1830) * mm, vector(15, 5, 1830) * mm] });
            opDropCurve(context, sub + "d", { "tools" : qCreatedBy(sub + "l", EntityType.EDGE),
                    "targets" : qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), vector(10, 10, 1810) * mm),
                    "projectionType" : ProjectionType.DIRECTION, "direction" : vector(0, 0, -1) });
            const len = evLength(context, { "entities" : qCreatedBy(sub + "d", EntityType.EDGE) });
            check("opDropCurve", abs(len - 10 * mm) < 1e-6 * mm, len);
        });
        step(context, id, "wrap", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 1900) * mm, "corner2" : vector(10, 10, 1901) * mm });
            const top = qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), vector(5, 5, 1901) * mm);
            opWrap(context, sub + "w", { "wrapType" : WrapType.SIMPLE, "entities" : top,
                    "source" : { "plane" : plane(vector(0, 0, 1901) * mm, vector(0, 0, 1), vector(1, 0, 0)), "anchorPoint" : vector(0, 0, 1901) * mm, "anchorDirection" : vector(1, 0, 0) },
                    "destination" : { "cylinder" : cylinder(coordSystem(vector(0, 0, 2000) * mm, vector(0, 1, 0), vector(1, 0, 0)), 20 * mm),
                            "anchorPoint" : vector(0, 0, 2020) * mm, "anchorDirection" : vector(1, 0, 0) } });
            const area = evArea(context, { "entities" : qCreatedBy(sub + "w", EntityType.BODY) });
            check("opWrap", abs(area - 100 * mm ^ 2) < 1e-6 * meter ^ 2, area);
        });
        step(context, id, "ruled", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 2100) * mm, "corner2" : vector(10, 10, 2110) * mm });
            const edge = qContainsPoint(qCreatedBy(sub + "c", EntityType.EDGE), vector(5, 0, 2110) * mm);
            opRuledSurface(context, sub + "r", { "path" : edge, "width" : 5 * mm, "ruledSurfaceType" : RuledSurfaceType.ALIGNED_WITH_VECTOR,
                    "ruledDirection" : vector(0, 0, 1), "angle" : 0 * degree });
            const area = evArea(context, { "entities" : qCreatedBy(sub + "r", EntityType.BODY) });
            check("opRuledSurface", abs(area - 50 * mm ^ 2) < 1e-6 * meter ^ 2, area);
        });
        step(context, id, "editcurve", function(sub) {
            opPolyline(context, sub + "w", { "points" : [vector(0, 0, 2200) * mm, vector(30, 0, 2200) * mm] });
            const pts = [vector(0, 0, 2200) * mm, vector(10, 5, 2200) * mm, vector(20, -5, 2200) * mm, vector(30, 0, 2200) * mm];
            opCreateBSplineCurve(context, sub + "c", { "bSplineCurve" : approximateSpline(context, { "degree" : 3, "tolerance" : 1e-5 * meter, "isPeriodic" : false,
                    "targets" : [approximationTarget({ "positions" : pts })] })[0] });
            const wire = qCreatedBy(sub + "w", EntityType.BODY);
            const curve = qCreatedBy(sub + "c", EntityType.EDGE);
            const dev = evMaxPathDeviation(context, { "side1" : wire, "side2" : curve }).deviation;
            opEditCurve(context, sub + "e", { "wire" : wire, "edge" : curve });
            const after = evMaxPathDeviation(context, { "side1" : wire, "side2" : curve }).deviation;
            check("opEditCurve+evMaxPathDeviation", dev > 3 * mm && after < 1e-6 * mm, [dev, after]);
        });
        step(context, id, "extracttangent", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 2300) * mm, "corner2" : vector(10, 10, 2310) * mm });
            opFillet(context, sub + "f", { "entities" : qContainsPoint(qCreatedBy(sub + "c", EntityType.EDGE), vector(5, 10, 2310) * mm), "radius" : 2 * mm });
            const top = qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), vector(5, 5, 2310) * mm);
            opExtractSurface(context, sub + "x", { "faces" : top, "tangentPropagation" : true });
            const n = size(evaluateQuery(context, qOwnedByBody(qCreatedBy(sub + "x", EntityType.BODY), EntityType.FACE)));
            check("opExtractSurface tangentPropagation", n == 3, n);
        });
        step(context, id, "conicpart", function(sub) {
            const sk = newSketchOnPlane(context, sub + "sk", { "sketchPlane" : plane(vector(0, 0, 2400) * mm, vector(0, 0, 1)) });
            skConicSegment(sk, "c", { "start" : vector(0, 0) * mm, "controlPoint" : vector(5, 10) * mm, "end" : vector(10, 0) * mm, "rho" : 0.5,
                    "lowParameter" : 0, "highParameter" : 0.5 });
            skSolve(sk);
            const n = size(evaluateQuery(context, qContainsPoint(qCreatedBy(sub + "sk", EntityType.VERTEX), vector(5, 5, 2400) * mm)));
            check("skConicSegment low/highParameter", n == 1, n);
        });
        step(context, id, "modifycomposite", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 2500) * mm, "corner2" : vector(1, 1, 2501) * mm });
            fCuboid(context, sub + "b", { "corner1" : vector(2, 0, 2500) * mm, "corner2" : vector(3, 1, 2501) * mm });
            fCuboid(context, sub + "c", { "corner1" : vector(4, 0, 2500) * mm, "corner2" : vector(5, 1, 2501) * mm });
            opCreateCompositePart(context, sub + "cp", { "bodies" : qUnion([qCreatedBy(sub + "a", EntityType.BODY), qCreatedBy(sub + "b", EntityType.BODY)]) });
            const cp = qCreatedBy(sub + "cp", EntityType.BODY);
            opModifyCompositePart(context, sub + "m", { "composite" : cp, "toAdd" : qCreatedBy(sub + "c", EntityType.BODY), "toRemove" : qCreatedBy(sub + "a", EntityType.BODY) });
            const v = evVolume(context, { "entities" : qCreatedBy(sub + "cp", EntityType.BODY) });
            check("opModifyCompositePart", abs(v - 2 * mm ^ 3) < 1e-12 * meter ^ 3, v);
        });
        step(context, id, "curvderiv", function(sub) {
            fCylinder(context, sub + "c", { "bottomCenter" : vector(0, 0, 2600) * mm, "topCenter" : vector(0, 0, 2610) * mm, "radius" : 10 * mm });
            const circle = qContainsPoint(qCreatedBy(sub + "c", EntityType.EDGE), vector(0, 10, 2600) * mm);
            const d = evEdgeCurvatureDerivative(context, { "edge" : circle, "parameter" : 0.3 });
            const dev = evPointsDeviation(context, { "points" : [vector(0, 0, 2600) * mm, vector(20, 0, 2600) * mm], "topologies" : circle, "allDeviations" : true });
            check("evEdgeCurvatureDerivative+evPointsDeviation", abs(norm(d) - 10000) < 1 && size(dev) == 2 && abs(dev[1].deviation - 10 * mm) < 1e-9 * meter, [norm(d), dev]);
        });
        step(context, id, "fullround", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 2700) * mm, "corner2" : vector(4, 20, 2710) * mm });
            const face = function(p) { return qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), p); };
            const before = evVolume(context, { "entities" : qCreatedBy(sub + "c", EntityType.BODY) });
            opFullRoundFillet(context, sub + "f", { "side1Face" : face(vector(0, 10, 2705) * mm), "side2Face" : face(vector(4, 10, 2705) * mm),
                    "centerFaces" : face(vector(2, 10, 2710) * mm), "tangentPropagation" : false });
            const after = evVolume(context, { "entities" : qCreatedBy(sub + "c", EntityType.BODY) });
            check("opFullRoundFillet", after < before && after > before * 0.9, [before, after]);
        });
        step(context, id, "curvesonface", function(sub) {
            fCuboid(context, sub + "c", { "corner1" : vector(0, 0, 2800) * mm, "corner2" : vector(10, 20, 2810) * mm });
            const top = qContainsPoint(qCreatedBy(sub + "c", EntityType.FACE), vector(5, 5, 2810) * mm);
            opCreateCurvesOnFace(context, sub + "i", { "curveDefinition" : [curveOnFaceDefinition(top, FaceCurveCreationType.DIR1_ISO, ["a", "b"], [0.25, 0.75])] });
            const n = size(evaluateQuery(context, qCreatedBy(sub + "i", EntityType.EDGE)));
            const a = size(evaluateQuery(context, qCreatedBy(sub + "i" + "a", EntityType.EDGE)));
            check("opCreateCurvesOnFace", n == 2 && a == 1, [n, a]);
        });
        step(context, id, "hole", function(sub) {
            fCuboid(context, sub + "a", { "corner1" : vector(0, 0, 2900) * mm, "corner2" : vector(10, 10, 2910) * mm });
            fCuboid(context, sub + "b", { "corner1" : vector(0, 0, 2890) * mm, "corner2" : vector(10, 10, 2900) * mm });
            const both = qUnion([qCreatedBy(sub + "a", EntityType.BODY), qCreatedBy(sub + "b", EntityType.BODY)]);
            const def = holeDefinition([
                        holeProfileBeforeReference(HolePositionReference.TARGET_START, 0 * mm, 3 * mm, { "name" : "a" }),
                        holeProfile(HolePositionReference.TARGET_START, 2 * mm, 3 * mm, { "name" : "b" }),
                        holeProfile(HolePositionReference.TARGET_START, 2 * mm, 1.5 * mm, { "name" : "c" }),
                        holeProfile(HolePositionReference.LAST_TARGET_START, 4 * mm, 1.5 * mm, { "name" : "d" }),
                        holeProfile(HolePositionReference.LAST_TARGET_START, 5 * mm, 0 * mm, { "name" : "tip" })
                    ], { "faceNames" : ["cap", "cbore", "cboreFloor", "shaft", "tip"] });
            const r = opHole(context, sub + "h", { "holeDefinition" : def, "axes" : [line(vector(5, 5, 2915) * mm, vector(0, 0, -1))], "targets" : both });
            const ex = r[0].targetToDepthExtremes[qCreatedBy(sub + "b", EntityType.BODY)];
            var exB;
            for (var q, e in r[0].targetToDepthExtremes)
            {
                if (e.firstEntrance > 10 * mm)
                    exB = e;
            }
            const refs = r[0].positionReferenceInfo;
            const ok = r[0].success && size(r[0].targetToDepthExtremes) == 2 && abs(exB.firstEntrance - 15 * mm) < 1e-9 * meter && abs(exB.fullExit - 25 * mm) < 1e-9 * meter &&
                abs(refs[HolePositionReference.TARGET_START].referenceRootEnd - 5 * mm) < 1e-9 * meter &&
                abs(refs[HolePositionReference.LAST_TARGET_START].referenceRootStart - 15 * mm) < 1e-9 * meter;
            check("opHole return", ok, r);
            check("opHole holeDepth", abs(r[0].holeDepth - 14 * mm) < 1e-9 * meter, r[0].holeDepth);
            // Live dom/fs-ophole.txt: faces cap 0, cbore 1, cboreFloor 1, shaft 2, tip 1; profiles a 0, b/c/d one
            // edge, tip one vertex; 5 faces, 4 profiles in all.
            var counts = [];
            for (var n in ["cap", "cbore", "cboreFloor", "shaft", "tip"])
                counts = append(counts, size(evaluateQuery(context, qOpHoleFace(sub + "h", { "name" : n }))));
            for (var n in ["a", "b", "c", "d"])
                counts = append(counts, size(evaluateQuery(context, qEntityFilter(qOpHoleProfile(sub + "h", { "name" : n }), EntityType.EDGE))));
            counts = append(counts, size(evaluateQuery(context, qEntityFilter(qOpHoleProfile(sub + "h", { "name" : "tip" }), EntityType.VERTEX))));
            counts = append(counts, size(evaluateQuery(context, qOpHoleFace(sub + "h"))));
            counts = append(counts, size(evaluateQuery(context, qOpHoleProfile(sub + "h"))));
            const vol = evVolume(context, { "entities" : qCreatedBy(sub + "a", EntityType.BODY) });
            check("opHole faces/profiles", counts == [0, 1, 1, 2, 1, 0, 1, 1, 1, 1, 5, 4] && vol < 1000 * mm ^ 3, [counts, vol, ex]);
            // Slanted entrance (live case 2): extremes on the 1.5 mm shaft, TARGET_START range on the 3 mm counterbore.
            const sk = newSketchOnPlane(context, sub + "wsk", { "sketchPlane" : plane(vector(0, 100, 2900) * mm, vector(0, -1, 0), vector(1, 0, 0)) });
            skPolyline(sk, "p", { "points" : [vector(0, 0) * mm, vector(20, 0) * mm, vector(20, 12) * mm, vector(0, 10) * mm, vector(0, 0) * mm] });
            skSolve(sk);
            opExtrude(context, sub + "W", { "entities" : qSketchRegion(sub + "wsk"), "direction" : vector(0, 1, 0), "endBound" : BoundingType.BLIND, "endDepth" : 20 * mm });
            const r2 = opHole(context, sub + "h2", { "holeDefinition" : def, "axes" : [line(vector(10, 110, 2920) * mm, vector(0, 0, -1))], "targets" : qCreatedBy(sub + "W", EntityType.BODY) });
            const ts = r2[0].positionReferenceInfo[HolePositionReference.TARGET_START];
            var e2;
            for (var q, e in r2[0].targetToDepthExtremes)
                e2 = e;
            const near = function(a, b) { return abs(a - b * mm) < 1e-6 * mm; };
            check("opHole slanted", near(ts.referenceRootStart, 8.7) && near(ts.referenceRootEnd, 9.3) && near(e2.firstEntrance, 8.85) && near(e2.fullEntrance, 9.15) && near(r2[0].holeDepth, 4.3), r2);
            // Misses: HOLE_NO_HITS with the targets at fault; a second axis that misses is { success : false }.
            const miss = try silent(opHole(context, sub + "h3", { "holeDefinition" : def, "axes" : [line(vector(500, 5, 2915) * mm, vector(0, 0, -1))], "targets" : both }));
            var thrown;
            try silent
            {
                opHole(context, sub + "h4", { "holeDefinition" : def, "axes" : [line(vector(500, 5, 2915) * mm, vector(0, 0, -1))], "targets" : both });
            }
            catch (e)
            {
                thrown = e;
            }
            const mixed = opHole(context, sub + "h5", { "holeDefinition" : def, "axes" : [line(vector(2, 8, 2915) * mm, vector(0, 0, -1)), line(vector(500, 5, 2915) * mm, vector(0, 0, -1))], "targets" : both });
            check("opHole misses", miss == undefined && thrown.message == ErrorStringEnum.HOLE_NO_HITS && thrown.faultyParameters == ["targets"] && mixed[0].success && mixed[1] == { "success" : false }, [thrown, mixed]);
        });
        step(context, id, "offsetwire", function(sub) {
            const sk = newSketchOnPlane(context, sub + "sk", { "sketchPlane" : plane(vector(0, 0, 3000) * mm, vector(0, 0, 1)) });
            skPolyline(sk, "l", { "points" : [vector(0, 0) * mm, vector(20, 0) * mm, vector(20, 10) * mm] });
            skRectangle(sk, "r", { "firstCorner" : vector(40, 0) * mm, "secondCorner" : vector(60, 10) * mm });
            skSolve(sk);
            const open = qUnion([sketchEntityQuery(sub + "sk", EntityType.EDGE, "l.line0"), sketchEntityQuery(sub + "sk", EntityType.EDGE, "l.line1")]);
            opOffsetWire(context, sub + "o", { "edges" : open, "normal" : vector(0, 0, 1), "offset1" : 1 * mm, "offset2" : 0 * mm, "makeRegions" : true });
            const a = evArea(context, { "entities" : qCreatedBy(sub + "o", EntityType.FACE) });
            // Left of the first edge (+x direction, seen from +z) is +y: inside the L corner.
            const inside = size(evaluateQuery(context, qContainsPoint(qCreatedBy(sub + "o", EntityType.FACE), vector(10, 0.5, 3000) * mm)));
            const rect = qCreatedBy(sub + "sk", EntityType.EDGE);
            opOffsetWire(context, sub + "c", { "edges" : qSubtraction(rect, open), "normal" : vector(0, 0, 1), "offset1" : 1 * mm, "offset2" : 1 * mm, "makeRegions" : true });
            const ca = evArea(context, { "entities" : qCreatedBy(sub + "c", EntityType.FACE) });
            opOffsetWire(context, sub + "w", { "edges" : open, "normal" : vector(0, 0, -1), "offset1" : 1 * mm, "offset2" : 0 * mm });
            const wires = size(evaluateQuery(context, qCreatedBy(sub + "w", EntityType.BODY)));
            check("opOffsetWire", abs(a - 29 * mm ^ 2) < 1e-9 * meter ^ 2 && inside == 1 && abs(ca - 120 * mm ^ 2) < 1e-6 * mm ^ 2 && wires == 1, [a, inside, ca, wires]);
        });
        step(context, id, "offsetdetect", function(sub) {
            fCuboid(context, sub + "plate", { "corner1" : vector(0, 0, 3100) * mm, "corner2" : vector(40, 20, 3102) * mm });
            fCylinder(context, sub + "outer", { "bottomCenter" : vector(100, 0, 3100) * mm, "topCenter" : vector(100, 0, 3110) * mm, "radius" : 7 * mm });
            fCylinder(context, sub + "inner", { "bottomCenter" : vector(100, 0, 3099) * mm, "topCenter" : vector(100, 0, 3111) * mm, "radius" : 5 * mm });
            opBoolean(context, sub + "tube", { "tools" : qCreatedBy(sub + "inner", EntityType.BODY), "targets" : qCreatedBy(sub + "outer", EntityType.BODY), "operationType" : BooleanOperationType.SUBTRACTION });
            fCuboid(context, sub + "thin", { "corner1" : vector(0, 300, 3100) * mm, "corner2" : vector(20, 320, 3101) * mm });
            fCuboid(context, sub + "thick", { "corner1" : vector(50, 300, 3100) * mm, "corner2" : vector(70, 320, 3103) * mm });
            const plate = evOffsetDetection(context, { "bodies" : qCreatedBy(sub + "plate", EntityType.BODY) });
            const tube = evOffsetDetection(context, { "bodies" : qCreatedBy(sub + "outer", EntityType.BODY) });
            const tt = evOffsetDetection(context, { "bodies" : qUnion([qCreatedBy(sub + "thin", EntityType.BODY), qCreatedBy(sub + "thick", EntityType.BODY)]) });
            const all = evOffsetDetection(context, { "bodies" : qUnion([qCreatedBy(sub + "plate", EntityType.BODY), qCreatedBy(sub + "outer", EntityType.BODY), qCreatedBy(sub + "thin", EntityType.BODY), qCreatedBy(sub + "thick", EntityType.BODY)]) });
            const areaOf = function(q) { return evArea(context, { "entities" : q }); };
            const ok = size(plate) == 1 && size(plate[0].side0) == 1 && abs(plate[0].offsetLow - 2 * mm) < 1e-9 * meter && abs(areaOf(plate[0].side0[0]) - 800 * mm ^ 2) < 1e-9 * meter ^ 2 &&
                size(tube) == 1 && abs(tube[0].offsetHigh - 2 * mm) < 1e-9 * meter &&
                size(tt) == 1 && abs(tt[0].offsetLow - 1 * mm) < 1e-9 * meter &&
                size(all) == 2;
            check("evOffsetDetection", ok, [plate, tube, tt, all]);
        });
        step(context, id, "packed", function(sub) {
            const top = qCompressed(1.0, "%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY", newId());
            const prefixed = qCompressed(1.0, "%B5$QueryM4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpS9$queryTypeS5$DUMMY", makeId("X"));
            const both = qCompressed(1.0, "%B5$QueryM2S9$queryTypeB9$QueryTypeS5$UNIONSa$subqueriesA2C0M4Sa$entityTypeBa$EntityTypeS4$FACESb$historyTypeS8$CREATIONSb$operationIdB2$IdA1S3.7$TopplaneOpR0S5$DUMMYC0M4R3C6S6$VERTEXR5R6R7CbA1S6.7$OriginpointOpR0Ra", newId());
            const ok = top.entityType == EntityType.FACE && top is Query && prefixed.operationId == makeId("X") + "Top.planeOp" &&
                size(evaluateQuery(context, top)) == 1 && size(evaluateQuery(context, prefixed)) == 0 && size(evaluateQuery(context, both)) == 2 &&
                both.queryType == QueryType.UNION;
            check("qCompressed", ok, [top, prefixed, both]);
        });
        println("done");
    });
