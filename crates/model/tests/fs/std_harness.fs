FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

// Runs standard library features from a custom feature and prints how each
// one went (tests/custom_features.rs std_features_run_in_custom_features).
function step(context is Context, id is Id, name is string, f is function)
{
    try
    {
        f(id + name);
        const err = getFeatureError(context, id + name);
        println(name ~ (err == undefined ? " ok" : " " ~ err));
    }
    catch (e)
    {
        println(name ~ " threw " ~ e);
    }
}

annotation { "Feature Type Name" : "Std harness" }
export const stdHarness = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        fCuboid(context, id + "cube", { "corner1" : vector(0, 0, 0) * meter, "corner2" : vector(20, 20, 20) * millimeter });
        const faces = qCreatedBy(id + "cube", EntityType.FACE);
        const edges = qCreatedBy(id + "cube", EntityType.EDGE);
        const at = function(q, x, y, z) { return qContainsPoint(q, vector(x, y, z) * millimeter); };
        step(context, id, "extrude", function(sub) {
            extrude(context, sub, { "entities" : at(faces, 10, 10, 20), "endBound" : BoundingType.BLIND, "depth" : 5 * millimeter, "operationType" : NewBodyOperationType.ADD });
        });
        step(context, id, "fillet", function(sub) {
            fillet(context, sub, { "entities" : at(edges, 10, 0, 0), "radius" : 2 * millimeter });
        });
        step(context, id, "chamfer", function(sub) {
            chamfer(context, sub, { "entities" : at(edges, 0, 10, 0), "width" : 1 * millimeter });
        });
        step(context, id, "linearPattern", function(sub) {
            linearPattern(context, sub, { "patternType" : PatternType.PART, "entities" : qCreatedBy(id + "cube", EntityType.BODY),
                    "directionOne" : at(edges, 10, 20, 0), "distance" : 30 * millimeter, "instanceCount" : 2 });
        });
        step(context, id, "circularPattern", function(sub) {
            circularPattern(context, sub, { "patternType" : PatternType.PART, "entities" : qCreatedBy(id + "cube", EntityType.BODY),
                    "axis" : at(edges, 20, 20, 10), "angle" : 360 * degree, "instanceCount" : 3, "equalSpace" : true });
        });
        step(context, id, "mirror", function(sub) {
            mirror(context, sub, { "patternType" : MirrorType.PART, "entities" : qCreatedBy(id + "cube", EntityType.BODY),
                    "mirrorPlane" : qCreatedBy(makeId("Right"), EntityType.FACE) });
        });
        step(context, id, "shell", function(sub) {
            shell(context, sub, { "entities" : at(faces, 10, 0, 10), "thickness" : 1 * millimeter });
        });
        step(context, id, "draft", function(sub) {
            draft(context, sub, { "neutralPlane" : at(faces, 10, 10, 0), "draftFaces" : at(faces, 20, 10, 10), "angle" : 3 * degree });
        });
        step(context, id, "booleanBodies", function(sub) {
            booleanBodies(context, sub, { "tools" : qAllModifiableSolidBodies(), "operationType" : BooleanOperationType.UNION });
        });
        step(context, id, "splitPart", function(sub) {
            splitPart(context, sub, { "targets" : qAllModifiableSolidBodies(), "tool" : qCreatedBy(makeId("Top"), EntityType.FACE) });
        });
        step(context, id, "skText", function(sub) {
            const sk = newSketchOnPlane(context, sub + "sk", { "sketchPlane" : plane(vector(0, 0, 40) * millimeter, vector(0, 0, 1)) });
            skText(sk, "t", { "text" : "Hi", "fontName" : "OpenSans-Regular.ttf", "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(10, 5) * millimeter });
            skSolve(sk);
            opExtrude(context, sub + "ex", { "entities" : qSketchRegion(sub + "sk"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : 1 * millimeter });
            if (size(evaluateQuery(context, qCreatedBy(sub + "ex", EntityType.BODY))) < 2)
                throw "text extruded into fewer than 2 bodies";
        });
        step(context, id, "deleteBodies", function(sub) {
            deleteBodies(context, sub, { "entities" : qNothing() });
        });
        println("parts " ~ size(evaluateQuery(context, qAllModifiableSolidBodies())));
    });
