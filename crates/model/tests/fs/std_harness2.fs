FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

// More standard library features from a custom feature
// (tests/custom_features.rs std_features_run_in_custom_features_2).
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

annotation { "Feature Type Name" : "Std harness 2" }
export const stdHarness2 = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const mm = millimeter;
        const at = function(q, x, y, z) { return qContainsPoint(q, vector(x, y, z) * mm); };
        fCuboid(context, id + "cube", { "corner1" : vector(0, 0, 0) * meter, "corner2" : vector(20, 20, 20) * mm });
        const faces = qCreatedBy(id + "cube", EntityType.FACE);
        const edges = qCreatedBy(id + "cube", EntityType.EDGE);
        // A sketch on the Front plane for revolve / sweep.
        const sk = newSketchOnPlane(context, id + "sk", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, -1, 0), vector(1, 0, 0)) });
        skRectangle(sk, "r", { "firstCorner" : vector(30, 0) * mm, "secondCorner" : vector(35, 5) * mm });
        skSolve(sk);
        step(context, id, "revolve", function(sub) {
            revolve(context, sub, { "entities" : qSketchRegion(id + "sk"), "axis" : qCreatedBy(makeId("Top"), EntityType.FACE)->qNthElement(0) });
        });
        step(context, id, "cPlane", function(sub) {
            cPlane(context, sub, { "entities" : at(faces, 10, 10, 20), "cplaneType" : CPlaneType.OFFSET, "offset" : 10 * mm });
        });
        step(context, id, "thicken", function(sub) {
            thicken(context, sub, { "entities" : at(faces, 10, 0, 10), "thickness1" : 1 * mm, "thickness2" : 0 * mm, "operationType" : NewBodyOperationType.NEW });
        });
        step(context, id, "transform", function(sub) {
            transform(context, sub, { "entities" : qCreatedBy(id + "cube", EntityType.BODY), "transformType" : TransformType.TRANSLATION_DISTANCE,
                    "transformDirection" : at(edges, 10, 0, 0), "distance" : 50 * mm, "makeCopy" : true });
        });
        step(context, id, "moveFace", function(sub) {
            moveFace(context, sub, { "moveFaces" : at(faces, 20, 10, 10), "moveFaceType" : MoveFaceType.OFFSET, "offsetDistance" : 2 * mm });
        });
        step(context, id, "deleteFace", function(sub) {
            fCuboid(context, sub + "b", { "corner1" : vector(0, 0, 100) * mm, "corner2" : vector(10, 10, 110) * mm });
            deleteFace(context, sub, { "deleteFaces" : qContainsPoint(qCreatedBy(sub + "b", EntityType.FACE), vector(5, 5, 110) * mm), "heal" : false });
        });
        step(context, id, "offsetSurface", function(sub) {
            // The bottom face, extended by the move face above: still the cube's.
            offsetSurface(context, sub, { "surfacesAndFaces" : at(faces, 10, 10, 0), "offset" : 3 * mm });
        });
        step(context, id, "loft", function(sub) {
            const a = newSketchOnPlane(context, sub + "a", { "sketchPlane" : plane(vector(0, 0, 200) * mm, vector(0, 0, 1)) });
            skCircle(a, "c", { "center" : vector(0, 0) * mm, "radius" : 5 * mm });
            skSolve(a);
            const b = newSketchOnPlane(context, sub + "b", { "sketchPlane" : plane(vector(0, 0, 220) * mm, vector(0, 0, 1)) });
            skRectangle(b, "r", { "firstCorner" : vector(-4, -4) * mm, "secondCorner" : vector(4, 4) * mm });
            skSolve(b);
            loft(context, sub, { "sheetProfilesArray" : [{ "sheetProfileEntities" : qSketchRegion(sub + "a") }, { "sheetProfileEntities" : qSketchRegion(sub + "b") }] });
        });
        step(context, id, "sweep", function(sub) {
            const p = newSketchOnPlane(context, sub + "p", { "sketchPlane" : plane(vector(0, 0, 300) * mm, vector(0, 0, 1)) });
            skCircle(p, "c", { "center" : vector(0, 0) * mm, "radius" : 2 * mm });
            skSolve(p);
            opFitSpline(context, sub + "path", { "points" : [vector(0, 0, 300) * mm, vector(0, 5, 320) * mm, vector(0, 0, 340) * mm] });
            sweep(context, sub, { "profiles" : qSketchRegion(sub + "p"), "path" : qCreatedBy(sub + "path", EntityType.EDGE) });
        });
        println("parts " ~ size(evaluateQuery(context, qAllModifiableSolidBodies())));
    });
