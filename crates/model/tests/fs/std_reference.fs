FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

// Exact numbers of common std features, the same file run live (Onshape) and here
// (tests/custom_features.rs std_features_match_onshape). Each case builds its own
// bodies 100 mm apart along x and prints one line:
// "<case> v <volume mm³> a <area mm²> f/e/v <faces>/<edges>/<vertices> box <min> <max>".

function r6(x is number) returns number
{
    return roundToPrecision(x, 6);
}

function report(context is Context, name is string, bodies is Query)
{
    const evaluated = evaluateQuery(context, bodies);
    if (evaluated == [])
    {
        println(name ~ " no bodies");
        return;
    }
    const v = evVolume(context, { "entities" : bodies }) / millimeter ^ 3;
    const faces = qOwnedByBody(bodies, EntityType.FACE);
    const a = evArea(context, { "entities" : faces }) / millimeter ^ 2;
    const bx = evBox3d(context, { "topology" : bodies, "tight" : true });
    const lo = bx.minCorner / millimeter;
    const hi = bx.maxCorner / millimeter;
    println(name ~ " v " ~ r6(v) ~ " a " ~ r6(a) ~ " f/e/v " ~ size(evaluateQuery(context, faces)) ~ "/" ~ size(evaluateQuery(context, qOwnedByBody(bodies, EntityType.EDGE))) ~ "/" ~ size(evaluateQuery(context, qOwnedByBody(bodies, EntityType.VERTEX)))
            ~ " box (" ~ r6(lo[0]) ~ ", " ~ r6(lo[1]) ~ ", " ~ r6(lo[2]) ~ ") (" ~ r6(hi[0]) ~ ", " ~ r6(hi[1]) ~ ", " ~ r6(hi[2]) ~ ")");
}

function run(context is Context, id is Id, name is string, f is function)
{
    try
    {
        const bodies = f(id + name);
        const err = getFeatureError(context, id + name);
        if (err != undefined)
            println(name ~ " error " ~ toString(err));
        report(context, name, bodies);
    }
    catch (e)
    {
        println(name ~ " threw " ~ toString(e));
    }
}

/** A sketch on a plane parallel to Top at height z, origin (x, 0). */
function sketchAt(context is Context, id is Id, x, z)
{
    return newSketchOnPlane(context, id, { "sketchPlane" : plane(vector(x, 0, z) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });
}

function box20(context is Context, id is Id, x)
{
    fCuboid(context, id, { "corner1" : vector(x, 0, 0) * millimeter, "corner2" : vector(x + 20, 20, 20) * millimeter });
    return qCreatedBy(id, EntityType.BODY);
}

annotation { "Feature Type Name" : "Std reference" }
export const stdReference = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const mm = millimeter;
        const at = function(q, x, y, z) { return qContainsPoint(q, vector(x, y, z) * mm); };
        var x = 0;

        run(context, id, "extrudeBlind", function(sub) {
            const sk = sketchAt(context, sub + "sk", x, 0);
            skRectangle(sk, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(20, 10) * mm });
            skSolve(sk);
            extrude(context, sub + "ex", { "entities" : qSketchRegion(sub + "sk"), "endBound" : BoundingType.BLIND, "depth" : 15 * mm, "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "ex", EntityType.BODY);
        });
        x += 100;
        run(context, id, "extrudeSymmetricDraft", function(sub) {
            const sk = sketchAt(context, sub + "sk", x, 0);
            skCircle(sk, "c", { "center" : vector(0, 0) * mm, "radius" : 5 * mm });
            skSolve(sk);
            extrude(context, sub + "ex", { "entities" : qSketchRegion(sub + "sk"), "endBound" : BoundingType.BLIND, "depth" : 10 * mm, "symmetric" : true,
                        "hasDraft" : true, "draftAngle" : 5 * degree, "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "ex", EntityType.BODY);
        });
        x += 100;
        run(context, id, "extrudeSecondDirection", function(sub) {
            const sk = sketchAt(context, sub + "sk", x, 0);
            skRectangle(sk, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(10, 10) * mm });
            skSolve(sk);
            extrude(context, sub + "ex", { "entities" : qSketchRegion(sub + "sk"), "endBound" : BoundingType.BLIND, "depth" : 10 * mm,
                        "hasSecondDirection" : true, "secondDirectionBound" : BoundingType.BLIND, "secondDirectionDepth" : 4 * mm, "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "ex", EntityType.BODY);
        });
        x += 100;
        run(context, id, "revolve90", function(sub) {
            const sk = newSketchOnPlane(context, sub + "sk", { "sketchPlane" : plane(vector(x, 0, 0) * mm, vector(0, -1, 0), vector(1, 0, 0)) });
            skRectangle(sk, "r", { "firstCorner" : vector(5, 0) * mm, "secondCorner" : vector(10, 8) * mm });
            skLineSegment(sk, "axis", { "start" : vector(0, 0) * mm, "end" : vector(0, 10) * mm, "construction" : true });
            skSolve(sk);
            revolve(context, sub + "rv", { "entities" : qSketchRegion(sub + "sk"), "axis" : sketchEntityQuery(sub + "sk", EntityType.EDGE, "axis"),
                        "fullRevolve" : false, "endBound" : RevolveBoundingType.BLIND, "angle" : 90 * degree, "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "rv", EntityType.BODY);
        });
        x += 100;
        run(context, id, "filletEdge", function(sub) {
            const b = box20(context, sub + "c", x);
            fillet(context, sub + "f", { "entities" : at(qOwnedByBody(b, EntityType.EDGE), x + 10, 0, 20), "radius" : 3 * mm });
            return b;
        });
        x += 100;
        run(context, id, "filletCorner", function(sub) {
            const b = box20(context, sub + "c", x);
            const e = qOwnedByBody(b, EntityType.EDGE);
            fillet(context, sub + "f", { "entities" : qUnion([at(e, x + 10, 0, 20), at(e, x, 10, 20), at(e, x, 0, 10)]), "radius" : 2 * mm });
            return b;
        });
        x += 100;
        run(context, id, "chamferEqual", function(sub) {
            const b = box20(context, sub + "c", x);
            chamfer(context, sub + "f", { "entities" : at(qOwnedByBody(b, EntityType.EDGE), x + 10, 0, 20), "chamferType" : ChamferType.EQUAL_OFFSETS, "width" : 2 * mm });
            return b;
        });
        x += 100;
        run(context, id, "chamferTwoOffsets", function(sub) {
            const b = box20(context, sub + "c", x);
            chamfer(context, sub + "f", { "entities" : at(qOwnedByBody(b, EntityType.EDGE), x + 10, 0, 20), "chamferType" : ChamferType.TWO_OFFSETS,
                        "width1" : 2 * mm, "width2" : 4 * mm, "oppositeDirection" : false });
            return b;
        });
        x += 100;
        run(context, id, "chamferOffsetAngle", function(sub) {
            const b = box20(context, sub + "c", x);
            chamfer(context, sub + "f", { "entities" : at(qOwnedByBody(b, EntityType.EDGE), x + 10, 0, 20), "chamferType" : ChamferType.OFFSET_ANGLE,
                        "width" : 3 * mm, "angle" : 30 * degree, "oppositeDirection" : false });
            return b;
        });
        x += 100;
        run(context, id, "shellTop", function(sub) {
            const b = box20(context, sub + "c", x);
            shell(context, sub + "s", { "entities" : at(qOwnedByBody(b, EntityType.FACE), x + 10, 10, 20), "thickness" : 1.5 * mm });
            return b;
        });
        x += 100;
        run(context, id, "draftTwoFaces", function(sub) {
            const b = box20(context, sub + "c", x);
            const f = qOwnedByBody(b, EntityType.FACE);
            draft(context, sub + "d", { "neutralPlane" : at(f, x + 10, 10, 0), "draftFaces" : qUnion([at(f, x + 20, 10, 10), at(f, x, 10, 10)]), "angle" : 5 * degree });
            return b;
        });
        x += 100;
        run(context, id, "booleanSubtract", function(sub) {
            const b = box20(context, sub + "c", x);
            fCylinder(context, sub + "t", { "bottomCenter" : vector(x + 10, 10, -1) * mm, "topCenter" : vector(x + 10, 10, 21) * mm, "radius" : 4 * mm });
            booleanBodies(context, sub + "b", { "targets" : b, "tools" : qCreatedBy(sub + "t", EntityType.BODY), "operationType" : BooleanOperationType.SUBTRACTION });
            return b;
        });
        x += 100;
        run(context, id, "booleanIntersect", function(sub) {
            const b = box20(context, sub + "c", x);
            opSphere(context, sub + "s", { "center" : vector(x + 20, 20, 20) * mm, "radius" : 12 * mm });
            booleanBodies(context, sub + "b", { "targets" : b, "tools" : qCreatedBy(sub + "s", EntityType.BODY), "operationType" : BooleanOperationType.INTERSECTION });
            return qUnion([b, qCreatedBy(sub + "s", EntityType.BODY)]);
        });
        x += 100;
        run(context, id, "linearPatternPart", function(sub) {
            const b = box20(context, sub + "c", x);
            linearPattern(context, sub + "p", { "patternType" : PatternType.PART, "entities" : b, "directionOne" : at(qOwnedByBody(b, EntityType.EDGE), x + 20, 10, 0),
                        "distance" : 25 * mm, "instanceCount" : 3 });
            return qUnion([b, qCreatedBy(sub + "p", EntityType.BODY)]);
        });
        x += 100;
        run(context, id, "circularPatternPart", function(sub) {
            const b = box20(context, sub + "c", x);
            fCylinder(context, sub + "pin", { "bottomCenter" : vector(x + 30, 20, 0) * mm, "topCenter" : vector(x + 30, 20, 5) * mm, "radius" : 2 * mm });
            circularPattern(context, sub + "p", { "patternType" : PatternType.PART, "entities" : qCreatedBy(sub + "pin", EntityType.BODY),
                        "axis" : at(qOwnedByBody(b, EntityType.EDGE), x + 20, 20, 10), "angle" : 360 * degree, "instanceCount" : 4, "equalSpace" : true });
            return qUnion([qCreatedBy(sub + "pin", EntityType.BODY), qCreatedBy(sub + "p", EntityType.BODY)]);
        });
        x += 100;
        run(context, id, "mirrorPart", function(sub) {
            const b = box20(context, sub + "c", x);
            mirror(context, sub + "m", { "patternType" : MirrorType.PART, "entities" : b, "mirrorPlane" : at(qOwnedByBody(b, EntityType.FACE), x + 20, 10, 10) });
            return qUnion([b, qCreatedBy(sub + "m", EntityType.BODY)]);
        });
        x += 100;
        run(context, id, "sweepLineArc", function(sub) {
            const p = sketchAt(context, sub + "p", x, 0);
            skCircle(p, "c", { "center" : vector(0, 0) * mm, "radius" : 3 * mm });
            skSolve(p);
            const path = newSketchOnPlane(context, sub + "path", { "sketchPlane" : plane(vector(x, 0, 0) * mm, vector(0, -1, 0), vector(1, 0, 0)) });
            skLineSegment(path, "l", { "start" : vector(0, 0) * mm, "end" : vector(0, 30) * mm });
            skArc(path, "a", { "start" : vector(0, 30) * mm, "mid" : vector(10 - 10 * cos(45 * degree), 30 + 10 * sin(45 * degree)) * mm, "end" : vector(10, 40) * mm });
            skSolve(path);
            sweep(context, sub + "s", { "profiles" : qSketchRegion(sub + "p"), "path" : qCreatedBy(sub + "path", EntityType.EDGE), "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "s", EntityType.BODY);
        });
        x += 100;
        run(context, id, "loftSquareCircle", function(sub) {
            const a = sketchAt(context, sub + "a", x, 0);
            skRectangle(a, "r", { "firstCorner" : vector(-10, -10) * mm, "secondCorner" : vector(10, 10) * mm });
            skSolve(a);
            const b = sketchAt(context, sub + "b", x, 20);
            skCircle(b, "c", { "center" : vector(0, 0) * mm, "radius" : 8 * mm });
            skSolve(b);
            loft(context, sub + "l", { "sheetProfilesArray" : [{ "sheetProfileEntities" : qSketchRegion(sub + "a") }, { "sheetProfileEntities" : qSketchRegion(sub + "b") }],
                        "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "l", EntityType.BODY);
        });
        x += 100;
        run(context, id, "thickenFace", function(sub) {
            const b = box20(context, sub + "c", x);
            thicken(context, sub + "t", { "entities" : at(qOwnedByBody(b, EntityType.FACE), x + 10, 10, 20), "thickness1" : 2 * mm, "thickness2" : 0 * mm,
                        "operationType" : NewBodyOperationType.NEW });
            return qCreatedBy(sub + "t", EntityType.BODY);
        });
        x += 100;
        run(context, id, "moveFaceOffset", function(sub) {
            const b = box20(context, sub + "c", x);
            moveFace(context, sub + "m", { "moveFaces" : at(qOwnedByBody(b, EntityType.FACE), x + 20, 10, 10), "moveFaceType" : MoveFaceType.OFFSET, "offsetDistance" : 3 * mm });
            return b;
        });
        x += 100;
        run(context, id, "splitByPlane", function(sub) {
            const b = box20(context, sub + "c", x);
            opPlane(context, sub + "pl", { "plane" : plane(vector(x, 0, 7) * mm, normalize(vector(0, 1, 3))) });
            splitPart(context, sub + "s", { "targets" : b, "tool" : qCreatedBy(sub + "pl", EntityType.FACE) });
            return qUnion([b, qCreatedBy(sub + "s", EntityType.BODY)]);
        });
        x += 100;
        run(context, id, "deleteFilletFace", function(sub) {
            const b = box20(context, sub + "c", x);
            fillet(context, sub + "f", { "entities" : at(qOwnedByBody(b, EntityType.EDGE), x + 10, 0, 20), "radius" : 3 * mm });
            deleteFace(context, sub + "d", { "deleteFaces" : qGeometry(qOwnedByBody(b, EntityType.FACE), GeometryType.CYLINDER), "heal" : true });
            return b;
        });
        println("done");
    });
