FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

// Implementation of ANSI/ASME B94.6 knurling standard according to https://www.engineersedge.com/manufacturing_spec/knurl_chart.htm
// along with https://www.cjwinter.com/thread-rolling/knurls/standard for TPI-based pitch.

// Adapted from Novel Tinker's implementation:
// https://cad.onshape.com/documents/132a7deba6e4c1cc45b656cf/w/16dfe4e3f79954b0c995da4c/e/7ac49e41498288712f38b75f

// Updated for FeatureScript 3095: helix() no longer takes HelixType (it is no longer exported);
// it takes axisType / pathType / startType / endType instead.

const PITCH_ANGLE_BOUNDS = {
    (degree) : [0, 30, 45]
} as AngleBoundSpec;

const TOOTH_ANGLE_BOUNDS = {
    (degree) : [0, 80, 90]
} as AngleBoundSpec;

const GROOVE_DEPTH_PERCENT_BOUNDS = {
    (unitless) : [0, 100, 100]
} as RealBoundSpec;

export enum KnurlType
{
    annotation { "Name" : "Straight knurl" }
    STRAIGHT,
    annotation { "Name" : "Diagonal knurl" }
    DIAGONAL,
    annotation { "Name" : "Diamond knurl" }
    DIAMOND
}

export enum KnurlFace
{
    annotation { "Name" : "Knurl outer face" }
    OUTER,
    annotation { "Name" : "Knurl inner face" }
    INNER
}

export enum PitchType
{
    annotation { "Name" : "Circular pitch" }
    CIRCULAR,
    annotation { "Name" : "Diametral pitch" }
    DIAMETRAL
}

// Standard circular pitches (grooves/teeth per inch circumference)
export enum CircularPitch
{
    annotation { "Name" : "12 TPI" }
    CP12,
    annotation { "Name" : "16 TPI" }
    CP16,
    annotation { "Name" : "20 TPI" }
    CP20,
    annotation { "Name" : "25 TPI" }
    CP25,
    annotation { "Name" : "30 TPI" }
    CP30,
    annotation { "Name" : "35 TPI" }
    CP35,
    annotation { "Name" : "40 TPI" }
    CP40
}

// Standard tooth depth/increase in knurled diameter (TODO: implement increase in knurled diameter?) based on circular pitch
const cpTable = {
    CircularPitch.CP12 : { toothDepth : 0.034 * inch, circularPitch : 12 },
    CircularPitch.CP16 : { toothDepth : 0.025 * inch, circularPitch : 16 },
    CircularPitch.CP20 : { toothDepth : 0.020 * inch, circularPitch : 20 },
    CircularPitch.CP25 : { toothDepth : 0.016 * inch, circularPitch : 25 },
    CircularPitch.CP30 : { toothDepth : 0.013 * inch, circularPitch : 30 },
    CircularPitch.CP35 : { toothDepth : 0.011 * inch, circularPitch : 35 },
    CircularPitch.CP40 : { toothDepth : 0.009 * inch, circularPitch : 40 }
};

// Standard diametral pitches (circumferential grooves/teeth per inch diameter)
export enum DiametralPitch
{
    annotation { "Name" : "64 per inch diameter" }
    DP64,
    annotation { "Name" : "96 per inch diameter" }
    DP96,
    annotation { "Name" : "128 per inch diameter" }
    DP128,
    annotation { "Name" : "160 per inch diameter" }
    DP160
}

// Standard tooth depth/increase in knurled diameter (TODO: implement increase in knurled diameter?) based on diametral pitch
const dpTable = {
    DiametralPitch.DP64 : { toothDepth : 0.024 * inch, diametralPitch : 64 },
    DiametralPitch.DP96 : { toothDepth : 0.016 * inch, diametralPitch : 96 },
    DiametralPitch.DP128 : { toothDepth : 0.012 * inch, diametralPitch : 128 },
    DiametralPitch.DP160 : { toothDepth : 0.009 * inch, diametralPitch : 160 }
};

annotation { "Feature Type Name" : "Printable ANSI/ASME B94.6 knurling" }
export const myFeature = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Start and end edges", "Filter" : EntityType.EDGE && QueryFilterCompound.ALLOWS_AXIS, "MaxNumberOfPicks" : 2 }
        definition.edges is Query;
        annotation { "Name" : "Knurl type" }
        definition.knurlType is KnurlType;
        annotation { "Name" : "Cylinder face to knurl" }
        definition.knurlFace is KnurlFace;
        annotation { "Name" : "Pitch type (TPI)" }
        definition.pitchType is PitchType;
        if (definition.pitchType == PitchType.CIRCULAR) {
            annotation { "Name" : "Circular pitch" }
            definition.circularPitch is CircularPitch;
        }
        else {
            annotation { "Name" : "Diametral pitch" }
            definition.diametralPitch is DiametralPitch;
        }
        if (definition.knurlType == KnurlType.DIAGONAL) {
            annotation { "Name" : "Opposite direction", "UIHint" : UIHint.OPPOSITE_DIRECTION }
            definition.oppositeDirection is boolean;
        }
        annotation { "Name" : "Tooth angle (angle of the pointy end--70, 80, 90 degrees are common)" }
        isAngle(definition.toothAngle, TOOTH_ANGLE_BOUNDS);
        if (definition.knurlType != KnurlType.STRAIGHT) {
            annotation { "Name" : "Tooth helix angle (angle of teeth relative to cylinder axis / tool path--30, 45 degrees are common)" }
            isAngle(definition.toothHelixAngle, PITCH_ANGLE_BOUNDS);
        }
        annotation { "Name" : "Groove depth percent" }
        isReal(definition.grooveDepthPercent, GROOVE_DEPTH_PERCENT_BOUNDS);
        annotation { "Name" : "Targets", "Filter" : EntityType.BODY && ModifiableEntityOnly.YES &&
            (BodyType. SOLID || (BodyType.SHEET && ConstructionObject.NO && SketchObject.NO))  && AllowMeshGeometry.YES}
        definition.targets is Query;
    }
    {
        if (definition.knurlType == KnurlType.STRAIGHT) {
            definition.toothHelixAngle = 0 * degree;
        }

        // The two circular edges of the selected cylinder
        const sourceEdge = qNthElement(definition.edges, 0);
        const destEdge = qNthElement(definition.edges, 1);
        const overshoot = 0 * inch; // how much to go past each edge to ensure a good/fast boolean operation
        const height = evDistance(context, {"side0": sourceEdge, "side1": destEdge}).distance + 2 * overshoot;

        const sourceRadius = evCurveDefinition(context, {
                "edge" : sourceEdge
        }).radius;
        const sourceDiameter = 2 * sourceRadius;
        const pitch = sourceDiameter * PI * tan(90 * degree - definition.toothHelixAngle);

        const toothHelixCosine = cos(definition.toothHelixAngle);
        const cpt = cpTable[definition.circularPitch];
        const dpt = dpTable[definition.diametralPitch];
        const grooveDepth = (definition.pitchType == PitchType.CIRCULAR) ?
                            cpt.toothDepth :
                            dpt.toothDepth * toothHelixCosine;
        const grooveHalfBase = grooveDepth * tan(definition.toothAngle / 2) / toothHelixCosine;
        const toothExtra = 2 * sourceRadius * (1 - cos(asin(grooveHalfBase / sourceRadius)));
        const toothHeight = grooveDepth + toothExtra;
        const toothZero = -(toothExtra + grooveDepth * (100 - definition.grooveDepthPercent) / 100);
        const grooves = (definition.pitchType == PitchType.CIRCULAR) ?
                        floor((sourceDiameter / inch) * PI * cpt.circularPitch + 0.5) :
                        floor((sourceDiameter / inch) * dpt.diametralPitch / toothHelixCosine + 0.5);

        const body = (definition.targets == undefined) ? qOwnerBody(sourceEdge) : definition.targets;

        const sourceAxis = evAxis(context, {"axis":sourceEdge});
        const destAxis = evAxis(context, {"axis":destEdge});

        // oppositeDirection only exists for diagonal knurls, so treat a missing value as false
        const opposite = definition.oppositeDirection == true;
        const handedness = opposite ? Direction.CCW : Direction.CW;
        const sourceToDest = destAxis.origin - sourceAxis.origin;
        const reverseHelix = dot(sourceToDest, sourceAxis.direction) < 0;

        // Main helix going the whole length of the cylinder, starting on the start edge (a circle)
        var helixDef = {
            "axisType" : AxisType.CIRCLE,
            "edge" : sourceEdge,
            "oppositeDirection" : reverseHelix,
            "handedness" : handedness,
            "startType" : StartType.START_ANGLE,
            "startAngle" : 0 * degree,
            "endType" : EndType.HEIGHT,
            "height" : height
        };
        if (definition.knurlType == KnurlType.STRAIGHT) {
            helixDef.pathType = PathType.TURNS;
            helixDef.revolutions = 1e-5; // the minimum the helix allows: effectively a straight line
        }
        else {
            helixDef.pathType = PathType.PITCH;
            helixDef.helicalPitch = pitch;
        }
        helix(context, id + "threadHelix", helixDef);

        const helix1 = qCreatedBy(id + "threadHelix", EntityType.EDGE);
        const endpoints = qAdjacent(helix1, AdjacencyType.VERTEX, EntityType.VERTEX);
        const endpoint1 = qNthElement(endpoints, 0);

        //Positive Z direction of sketch coord system is from source edge toward destination edge (along the cylinder's length)
        const zDirection is Vector = normalize(destAxis.origin - sourceAxis.origin);
        //Origin is at the vertex at the end of the helix
        //Well, actually slightly outside the vertex to deal with numerical precision issues in OnShape.
        const startPosition is Vector = evVertexPoint(context, {"vertex": endpoint1}) - zDirection * overshoot;
        //Positive Y direction points outward from the center of the cylinder
        const yDirection is Vector = normalize(startPosition - sourceAxis.origin);
        const xDirection is Vector = cross(yDirection, zDirection);

        const cSys is CoordSystem = coordSystem(startPosition, xDirection, zDirection);
        const sketchPlane is Plane = plane(cSys);

        //Draw the triangle to sweep
        const sketch = newSketchOnPlane(context, id + "sketch", {
            "sketchPlane" : sketchPlane
        });

        // Shape the triangle to give the specified groove profile
        // -- how wide is half a tooth
        const toothHalfWidth = toothHeight * tan(definition.toothAngle / 2);
        // -- project a tooth back along the tooth helix angle onto the sketch plane
        const halfBase = toothHalfWidth / toothHelixCosine;

        // Point the triangle according to whether we're knurling an inner face (outward cut) or outer face (inward cut).
        const cutDirection = (definition.knurlFace == KnurlFace.OUTER) ? -1 : 1;

        skLineSegment(sketch, "a", {
            start:vector(halfBase, cutDirection * toothZero),
            end:vector(0 * millimeter, cutDirection * (toothHeight + toothZero)),
        });

        skLineSegment(sketch, "b", {
            start:vector(0 * millimeter, cutDirection * (toothHeight + toothZero)),
            end:vector(-halfBase, cutDirection * toothZero),
        });

        skLineSegment(sketch, "c", {
            start:vector(-halfBase, cutDirection * toothZero),
            end:vector(0 * millimeter, cutDirection * (toothZero - toothHeight)),
        });

        skLineSegment(sketch, "d", {
            start:vector(0 * millimeter, cutDirection * (toothZero - toothHeight)),
            end:vector(halfBase, cutDirection * toothZero),
        });

        skSolve(sketch);
        opSweep(context, id + "cut1", {
            "profiles" : qSketchRegion(id + "sketch"),
            "path" : helix1,
        });

        const threads = qCreatedBy(id + "cut1", EntityType.BODY);

        circularPattern(context, id + "pattern", {
            "patternType" : PatternType.PART,
            "operationType" : NewBodyOperationType.NEW,
            "entities" : threads,
            "axis" : sourceEdge,
            "angle" : 360 * degree,
            "instanceCount" : grooves,
            "equalSpace" : true,
        });

        var knurlTool = qUnion([threads, qCreatedBy(id + "pattern", EntityType.BODY)]);
        if (definition.knurlType == KnurlType.DIAMOND) {
            const mirrorCS is CoordSystem = coordSystem(startPosition, yDirection, xDirection);
            opPlane(context, id + "mirrorPlane", {
                    "plane" : plane(mirrorCS)
            });
            mirror(context, id + "knurlTool2", {
                "patternType" : MirrorType.PART,
                "operationType" : NewBodyOperationType.NEW,
                "entities" : knurlTool,
                "mirrorPlane" : qCreatedBy(id + "mirrorPlane"),
            });
            knurlTool = qUnion([knurlTool, qCreatedBy(id + "knurlTool2", EntityType.BODY)]);
        }

        booleanBodies(context, id + "boolean", {
            "operationType" : BooleanOperationType.SUBTRACTION,
            "tools" : knurlTool,
            "targets" : body,
            "offset" : false
        });

        const entitiesToCleanUp = qUnion([
            helix1,
            qCreatedBy(id + "sketch"),
            qCreatedBy(id + "mirrorPlane"),
        ]);
        opDeleteBodies(context, id + "cleanup", {"entities": entitiesToCleanUp});
    });
