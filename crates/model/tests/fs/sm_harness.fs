FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

// std sheet metal features from a custom feature, checked against live
// dom/fs-smprobe{,2}-{1,2}.txt (tests/custom_features.rs std_sheet_metal_matches_onshape).
function faceAreas(context is Context, part is Query) returns array
{
    var out = [];
    for (var f in evaluateQuery(context, qOwnedByBody(part, EntityType.FACE)))
        out = append(out, roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 4));
    return sort(out, function(a, b) { return b - a; });
}

function run(context is Context, smId is Id, definition is map)
{
    try
    {
        sheetMetalStart(context, smId, definition);
    }
    catch (e)
    {
        println(toString(smId) ~ " threw " ~ toString(e));
    }
    const err = getFeatureError(context, smId);
    const parts = evaluateQuery(context, qBodyType(qCreatedBy(smId, EntityType.BODY), BodyType.SOLID));
    println(smId[size(smId) - 1] ~ " " ~ (err == undefined ? "" : toString(err) ~ " ") ~ "parts " ~ size(parts) ~ (size(parts) == 1 ? " areas " ~ toString(faceAreas(context, parts[0])) : ""));
}

annotation { "Feature Type Name" : "SM harness" }
export const smHarness = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        const mm = millimeter;
        // The Sheet metal model dialog's defaults (live model attribute, dom/fs-smprobe2-1.txt).
        const model = { "initEntities" : qNothing(), "tangentPropagation" : false, "bends" : qNothing(), "clearance" : 0 * mm, "bendsIncluded" : false,
                "oppositeDirection" : false, "flipDirectionUp" : false,
                "bendCalculationType" : SMBendCalculationType.K_FACTOR, "kFactor" : 0.45, "kFactorRolled" : 0.5, "minimalClearance" : 2e-5 * meter,
                "defaultCornerStyle" : SMCornerStrategyType.SIMPLE, "defaultCornerReliefScale" : 1.5,
                "defaultRoundReliefDiameter" : 5 * mm, "defaultSquareReliefWidth" : 5 * mm,
                "defaultBendReliefStyle" : SMBendStrategyType.OBROUND, "defaultBendReliefDepthScale" : 2, "defaultBendReliefScale" : 1.0625 };
        // sm1: Thicken of a 40 x 20 region on Top, 2 mm, r 1.
        const sk = newSketchOnPlane(context, id + "sk", { "sketchPlane" : plane(vector(0, 0, 0) * mm, vector(0, 0, 1)) });
        skRectangle(sk, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(40, 20) * mm });
        skSolve(sk);
        run(context, id + "sm1", mergeMaps(model, { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "sk"), "thickness" : 2 * mm, "radius" : 1 * mm }));
        // sm2: sm1's plate (at y 200) + a 90° Flange, 10 mm, on its top long edge (live dom/fs-smprobe-2.txt).
        const sk2 = newSketchOnPlane(context, id + "sk2", { "sketchPlane" : plane(vector(0, 200, 0) * mm, vector(0, 0, 1)) });
        skRectangle(sk2, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(40, 20) * mm });
        skSolve(sk2);
        run(context, id + "sm2", mergeMaps(model, { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "sk2"), "thickness" : 2 * mm, "radius" : 1 * mm }));
        try
        {
            const part2 = qBodyType(qCreatedBy(id + "sm2", EntityType.BODY), BodyType.SOLID);
            sheetMetalFlange(context, id + "fl2", { "edges" : qContainsPoint(qOwnedByBody(part2, EntityType.EDGE), vector(20, 200, 2) * mm),
                        "limitType" : SMFlangeBoundingType.BLIND, "distance" : 10 * mm, "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE,
                        "bendAngle" : 90 * degree, "autoMiter" : true, "useDefaultRadius" : true });
            const err = getFeatureError(context, id + "fl2");
            const parts = evaluateQuery(context, part2);
            println("sm2 " ~ (err == undefined ? "" : toString(err) ~ " ") ~ "parts " ~ size(parts) ~ (size(parts) == 1 ? " areas " ~ toString(faceAreas(context, parts[0])) : ""));
        }
        catch (e)
        {
            println("sm2 flange threw " ~ toString(e));
        }
        // sm3: Extrude of an open L (0,0) (30,0) (30,20) on Top (moved to y 100), 15 mm, 1.5 mm, r 2.
        const sk3 = newSketchOnPlane(context, id + "sk3", { "sketchPlane" : plane(vector(0, 100, 0) * mm, vector(0, 0, 1)) });
        skPolyline(sk3, "l", { "points" : [vector(0, 0) * mm, vector(30, 0) * mm, vector(30, 20) * mm] });
        skSolve(sk3);
        run(context, id + "sm3", mergeMaps(model, { "process" : SMProcessType.EXTRUDE, "sketchCurves" : qCreatedBy(id + "sk3", EntityType.EDGE), "bendArcs" : qNothing(),
                        "endBound" : SMExtrudeBoundingType.BLIND, "depth" : 15 * mm, "oppositeExtrudeDirection" : false, "symmetric" : false, "hasSecondDirection" : false,
                        "thickness" : 1.5 * mm, "radius" : 2 * mm }));
        println("parts " ~ size(evaluateQuery(context, qAllModifiableSolidBodies())));
        // What sm3's part faces come from (live: 4 walls, 8 edges incl. both bend cylinders, 2 vertices).
        const part3 = qBodyType(qCreatedBy(id + "sm3", EntityType.BODY), BodyType.SOLID);
        var kinds = { "FACE" : 0, "EDGE" : 0, "VERTEX" : 0, "NONE" : 0 };
        for (var f in evaluateQuery(context, qOwnedByBody(part3, EntityType.FACE)))
        {
            const d = getSMDefinitionEntities(context, f);
            const k = size(d) != 1 ? "NONE" : size(evaluateQuery(context, qEntityFilter(d[0], EntityType.FACE))) == 1 ? "FACE" :
                    (size(evaluateQuery(context, qEntityFilter(d[0], EntityType.EDGE))) == 1 ? "EDGE" : "VERTEX");
            kinds[k] += 1;
        }
        println("sm3 definitions " ~ toString(kinds));
        println("done");
    });
