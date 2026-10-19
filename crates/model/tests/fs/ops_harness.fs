FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

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
        println("done");
    });
