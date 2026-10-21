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
            // Flat pattern placements (live sm2 at y = 0; ours sits 200 mm along y).
            const shift = transform(vector(0, 0.2, 0) * meter);
            const c = 6.123233995736766e-17;
            const live = {
                "base" : transform(matrix([[1, 0, 0], [0, 1, 0], [0, 0, 1]]), vector(0, 0, -0.002) * meter),
                "flange" : transform(matrix([[1, 0, 0], [0, c, -1], [0, 1, c]]), vector(0, 0.001015486979089696, 0) * meter),
                "bend" : transform(matrix([[1, 0, 0], [0, 0.7071067811865476, -0.7071067811865475], [0, 0.7071067811865475, 0.7071067811865476]]),
                    vector(0, 0.000921957051917943, -0.00182842712474619) * meter) };
            const faceAt = { "base" : vector(20, 210, 2), "flange" : vector(20, 198, 6.5), "bend" : vector(20, 201 - 3 * 0.7071067811865476, 3 - 3 * 0.7071067811865476) };
            var flatOk = true;
            for (var name, p in faceAt)
            {
                const t = try silent(evSheetMetalFlatTransformation(context, { "face" : qContainsPoint(qOwnedByBody(part2, EntityType.FACE), p * mm) }));
                const want = shift * live[name] * inverse(shift);
                const probe = vector(13, 207, 5) * mm;
                if (t == undefined || norm(t * probe - want * probe) > 1e-9 * meter)
                {
                    flatOk = false;
                    println("sm2 flat " ~ name ~ " got " ~ toString(t) ~ " want " ~ toString(want));
                }
            }
            println("sm2 flat " ~ (flatOk ? "ok" : "FAILED"));
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
        // sm4: CONVERT of a 40 x 30 x 20 box (at x 200) without its top face, the bottom
        // edges as bends, 1 mm, r 1 (live dom/fs-smprobe3-4.txt: the side edges become rips).
        const sk4 = newSketchOnPlane(context, id + "sk4", { "sketchPlane" : plane(vector(200, 0, 0) * mm, vector(0, 0, 1), vector(1, 0, 0)) });
        skRectangle(sk4, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(40, 30) * mm });
        skSolve(sk4);
        opExtrude(context, id + "box4", { "entities" : qSketchRegion(id + "sk4"), "direction" : vector(0, 0, 1), "endBound" : BoundingType.BLIND, "endDepth" : 20 * mm });
        const box4 = qCreatedBy(id + "box4", EntityType.BODY);
        var bottom = [];
        for (var p in [vector(220, 0, 0), vector(220, 30, 0), vector(200, 15, 0), vector(240, 15, 0)])
            bottom = append(bottom, qContainsPoint(qOwnedByBody(box4, EntityType.EDGE), p * mm));
        run(context, id + "sm4", mergeMaps(model, { "process" : SMProcessType.CONVERT, "partToConvert" : box4,
                        "facesToExclude" : qContainsPoint(qOwnedByBody(box4, EntityType.FACE), vector(220, 15, 20) * mm), "bends" : qUnion(bottom),
                        "keepInputParts" : false, "thickness" : 1 * mm, "radius" : 1 * mm }));
        var joints = {};
        for (var f in evaluateQuery(context, qOwnedByBody(qBodyType(qCreatedBy(id + "sm4", EntityType.BODY), BodyType.SOLID), EntityType.FACE)))
        {
            for (var d in getSMDefinitionEntities(context, f, EntityType.EDGE))
            {
                for (var a in getSmObjectTypeAttributes(context, d, SMObjectType.JOINT))
                    joints[a.attributeId] = toString(a.jointType.value);
            }
        }
        var jointKinds = {};
        for (var k, v in joints)
            jointKinds[v] = (jointKinds[v] == undefined ? 0 : jointKinds[v]) + 1;
        println("sm4 joints " ~ toString(jointKinds));
        // sm7: sm1's plate (at x 300) + a closed Hem (straight, minimal gap) on its y = 0 top edge,
        // 12.5 mm, outer (live dom/fs-smprobe3-7.txt).
        const sk7 = newSketchOnPlane(context, id + "sk7", { "sketchPlane" : plane(vector(300, 0, 0) * mm, vector(0, 0, 1), vector(1, 0, 0)) });
        skRectangle(sk7, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(40, 20) * mm });
        skSolve(sk7);
        run(context, id + "sm7", mergeMaps(model, { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "sk7"), "thickness" : 2 * mm, "radius" : 1 * mm }));
        try
        {
            const part7 = qBodyType(qCreatedBy(id + "sm7", EntityType.BODY), BodyType.SOLID);
            sheetMetalHem(context, id + "hem7", { "edges" : qContainsPoint(qOwnedByBody(part7, EntityType.EDGE), vector(320, 0, 2) * mm),
                        "hemType" : SMHemType.STRAIGHT, "oppositeDirection" : false, "useMinimalGapForStraight" : true, "innerRadius" : 2.3 * mm,
                        "useMinimalGapForTipGap" : true, "tipGap" : 0.02 * mm, "angle" : 270 * degree, "length" : 12.5 * mm,
                        "hemAlignment" : SMHemAlignment.OUTER, "hemCornerType" : SMHemCornerType.SIMPLE });
            const err = getFeatureError(context, id + "hem7");
            const parts = evaluateQuery(context, part7);
            println("sm7 " ~ (err == undefined ? "" : toString(err) ~ " ") ~ "parts " ~ size(parts) ~ (size(parts) == 1 ? " areas " ~ toString(faceAreas(context, parts[0])) : ""));
        }
        catch (e)
        {
            println("sm7 hem threw " ~ toString(e));
        }
        // sm8: sm1's plate (at x 400) + a Tab: a 10 x 5 region on the definition plane (z = 0)
        // touching the y = 0 edge, merged into the plate's top face (live dom/fs-smprobe3-8.txt).
        const sk8 = newSketchOnPlane(context, id + "sk8", { "sketchPlane" : plane(vector(400, 0, 0) * mm, vector(0, 0, 1), vector(1, 0, 0)) });
        skRectangle(sk8, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(40, 20) * mm });
        skSolve(sk8);
        run(context, id + "sm8", mergeMaps(model, { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "sk8"), "thickness" : 2 * mm, "radius" : 1 * mm }));
        try
        {
            const part8 = qBodyType(qCreatedBy(id + "sm8", EntityType.BODY), BodyType.SOLID);
            const tabSketch = newSketchOnPlane(context, id + "tabSk", { "sketchPlane" : plane(vector(400, 0, 0) * mm, vector(0, 0, 1), vector(1, 0, 0)) });
            skRectangle(tabSketch, "r", { "firstCorner" : vector(15, -5) * mm, "secondCorner" : vector(25, 0) * mm });
            skSolve(tabSketch);
            sheetMetalTab(context, id + "tab8", { "tabFaces" : qSketchRegion(id + "tabSk"),
                        "booleanUnionScope" : qContainsPoint(qOwnedByBody(part8, EntityType.FACE), vector(420, 10, 2) * mm),
                        "booleanOffset" : 0 * mm, "booleanSubtractScope" : qNothing() });
            const err = getFeatureError(context, id + "tab8");
            const parts = evaluateQuery(context, part8);
            println("sm8 " ~ (err == undefined ? "" : toString(err) ~ " ") ~ "parts " ~ size(parts) ~ (size(parts) == 1 ? " areas " ~ toString(faceAreas(context, parts[0])) : ""));
        }
        catch (e)
        {
            println("sm8 tab threw " ~ toString(e));
        }
        // Reliefs (live dom/fs-smprobe3-{5,6,6b,6c}.txt, dom/fs-smprobe4-*.txt): a 40 x 20 plate at
        // x0 with a 90°, 8 mm INNER Flange on its front edge (partial: 10 mm off each end) or on
        // its front and right edges, under the given model settings.
        const relief = function(name is string, x0 is number, settings is map, edges is array, partial is boolean)
            {
                const sk = newSketchOnPlane(context, id + (name ~ "sk"), { "sketchPlane" : plane(vector(x0, 0, 0) * mm, vector(0, 0, 1), vector(1, 0, 0)) });
                skRectangle(sk, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(40, 20) * mm });
                skSolve(sk);
                const smId = id + name;
                const def = mergeMaps(mergeMaps(model, { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + (name ~ "sk")), "thickness" : 2 * mm, "radius" : 1 * mm }), settings);
                try silent(sheetMetalStart(context, smId, def));
                const part = qBodyType(qCreatedBy(smId, EntityType.BODY), BodyType.SOLID);
                var q = [];
                for (var p in edges)
                    q = append(q, qContainsPoint(qOwnedByBody(part, EntityType.EDGE), (vector(x0, 0, 0) + p) * mm + vector(0, 0, 1) * def.thickness));
                try
                {
                    sheetMetalFlange(context, smId + "fl", { "edges" : qUnion(q), "flangeAlignment" : SMFlangeAlignment.INNER,
                                "limitType" : SMFlangeBoundingType.BLIND, "distance" : 8 * mm, "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE,
                                "bendAngle" : 90 * degree, "autoMiter" : true, "useDefaultRadius" : true, "oppositeDirection" : false,
                                "isPartialFlange" : partial, "chainType" : SMPartialFlangeChainType.PER_EDGE, "flipFlangeBounds" : false, "holdAdjacentEdges" : true,
                                "firstBoundType" : SMFlangeBoundingType.BLIND, "firstBoundOffset" : 10 * mm,
                                "hasSecondBound" : true, "secondBoundType" : SMFlangeBoundingType.BLIND, "secondBoundOffset" : 10 * mm });
                    const err = getFeatureError(context, smId + "fl");
                    const parts = evaluateQuery(context, part);
                    println(name ~ " flanged " ~ (err == undefined ? "" : toString(err) ~ " ") ~ "parts " ~ size(parts) ~ (size(parts) == 1 ? " areas " ~ toString(faceAreas(context, parts[0])) : ""));
                }
                catch (e)
                {
                    println(name ~ " flange threw " ~ toString(e));
                }
            };
        const thin = { "thickness" : 1 * mm, "radius" : 2 * mm };
        const front = [vector(20, 0, 0)];
        const both = [vector(20, 0, 0), vector(40, 10, 0)];
        relief("sm5", 500, {}, front, true);
        relief("sm5b", 600, mergeMaps(thin, { "defaultBendReliefDepthScale" : 3, "defaultBendReliefScale" : 1.5 }), front, true);
        relief("sm5c", 700, mergeMaps(thin, { "defaultBendReliefDepthScale" : 3, "defaultBendReliefScale" : 1.5, "defaultBendReliefStyle" : SMBendStrategyType.RECTANGLE }), front, true);
        relief("sm5d", 800, mergeMaps(thin, { "defaultBendReliefDepthScale" : 3, "defaultBendReliefScale" : 1.5, "defaultBendReliefStyle" : SMBendStrategyType.TEAR }), front, true);
        relief("sm6", 900, {}, both, false);
        relief("sm6b", 1000, { "defaultCornerStyle" : SMCornerStrategyType.ROUND }, both, false);
        relief("sm6c", 1100, { "defaultCornerStyle" : SMCornerStrategyType.RECTANGLE }, both, false);
        relief("sm6d", 1200, mergeMaps(thin, { "defaultCornerStyle" : SMCornerStrategyType.ROUND, "defaultRoundReliefDiameter" : 3 * mm }), both, false);
        relief("sm6e", 1300, mergeMaps(thin, { "defaultCornerStyle" : SMCornerStrategyType.SIZED_ROUND, "defaultCornerReliefScale" : 2 }), both, false);
        relief("sm6f", 1400, mergeMaps(thin, { "defaultCornerStyle" : SMCornerStrategyType.RECTANGLE, "defaultSquareReliefWidth" : 3 * mm }), both, false);
        relief("sm6g", 1500, mergeMaps(thin, { "defaultCornerStyle" : SMCornerStrategyType.SIZED_RECTANGLE, "defaultCornerReliefScale" : 2 }), both, false);
        relief("sm6h", 1600, mergeMaps(thin, { "defaultCornerStyle" : SMCornerStrategyType.CLOSED }), both, false);
        // G34 (live behaviours.md "G34", qa5 SM1 / SM3): a 60 x 40 plate, 2 mm, r 2, 20 mm flanges on the
        // y = 0 and x = 0 top edges (8511.4987 mm³, 24 faces), then a Closed Corner at (2, 2, 2) (8535.0418, 23).
        {
            const x0 = 2000;
            const sk = newSketchOnPlane(context, id + "g34sk", { "sketchPlane" : plane(vector(x0, 0, 0) * mm, vector(0, 0, 1), vector(1, 0, 0)) });
            skRectangle(sk, "r", { "firstCorner" : vector(0, 0) * mm, "secondCorner" : vector(60, 40) * mm });
            skSolve(sk);
            sheetMetalStart(context, id + "g34", mergeMaps(model, { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "g34sk"), "thickness" : 2 * mm, "radius" : 2 * mm }));
            const part = qBodyType(qCreatedBy(id + "g34", EntityType.BODY), BodyType.SOLID);
            const report = function(name is string)
                {
                    const parts = evaluateQuery(context, part);
                    println(name ~ " parts " ~ size(parts) ~ (size(parts) == 1 ? " v " ~ roundToPrecision(evVolume(context, { "entities" : parts[0] }) / mm ^ 3, 4) ~
                                " faces " ~ size(evaluateQuery(context, qOwnedByBody(parts[0], EntityType.FACE))) : ""));
                };
            try
            {
                sheetMetalFlange(context, id + "g34fl", { "edges" : qUnion([qContainsPoint(qOwnedByBody(part, EntityType.EDGE), vector(x0 + 30, 0, 2) * mm),
                                    qContainsPoint(qOwnedByBody(part, EntityType.EDGE), vector(x0, 20, 2) * mm)]),
                            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : 20 * mm,
                            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "bendAngle" : 90 * degree, "autoMiter" : true, "useDefaultRadius" : true,
                            "oppositeDirection" : false, "isPartialFlange" : false });
                report("g34");
                sheetMetalCorner(context, id + "g34c", { "corner" : qContainsPoint(qOwnedByBody(part, EntityType.VERTEX), vector(x0 + 2, 2, 2) * mm),
                            "cornerStyle" : SMCornerReliefStyle.CLOSED, "cornerReliefScale" : 1.5, "roundReliefDiameter" : 5 * mm, "squareReliefWidth" : 5 * mm });
                report("g34a");
            }
            catch (e)
            {
                println("g34 threw " ~ toString(e));
            }
        }
        println("done");
    });
