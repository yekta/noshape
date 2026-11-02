FeatureScript 3070;
import(path : "onshape/std/common.fs", version : "3070.0");

/**
 * Y Ribs on Faces
 *
 * Select the faces to put ribs on (flat or curved, e.g. the outer face of a
 * disc) and a direction across them (a flat face, a straight or circular edge,
 * a cylinder or a mate connector). Each rib is the section of those faces by a
 * plane normal to the direction, with a round rib of `ribDiameter` swept along
 * it, the circle centred on the surface so the rib is half buried. Ribs start
 * ribDiameter/2 in from one end of the faces and follow at a pitch of
 * ribDiameter + ribGap, as many as fit on the faces or a given count.
 *
 * With "Use Existing Variables" on, the rib size comes from the Part Studio variables
 * #rib_diameter and #rib_pattern_empty_space and follows them when they
 * change. The box is ticked automatically on creation when those variables
 * exist; otherwise the typed Rib Diameter / Rib Empty Space are used.
 */
annotation { "Feature Type Name" : "Y Ribs on Faces", "Feature Type Description" : "Half-buried round ribs following flat or curved faces, patterned across them", "Editing Logic Function" : "yRibsOnFacesEditingLogic" }
export const yRibsOnFaces = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Faces", "Filter" : EntityType.FACE && ConstructionObject.NO && SketchObject.NO, "MaxNumberOfPicks" : 500 }
        definition.faces is Query;

        annotation { "Name" : "Use Existing Variables", "Default" : false, "Description" : "Take the rib size from #rib_diameter and #rib_pattern_empty_space" }
        definition.useVariables is boolean;

        if (definition.useVariables)
        {
            annotation { "Name" : "Diameter Variable", "UIHint" : UIHint.READ_ONLY }
            definition.diameterVariable is string;

            annotation { "Name" : "Empty Space Variable", "UIHint" : UIHint.READ_ONLY }
            definition.gapVariable is string;
        }

        if (!definition.useVariables)
        {
            annotation { "Name" : "Rib Diameter" }
            isLength(definition.ribDiameter, { (millimeter) : [0.1, 1.6, 100] } as LengthBoundSpec);

            annotation { "Name" : "Rib Empty Space" }
            isLength(definition.ribGap, { (millimeter) : [0, 0.6, 100] } as LengthBoundSpec);
        }

        annotation { "Name" : "Fill Faces", "Default" : true }
        definition.fillPart is boolean;

        if (!definition.fillPart)
        {
            annotation { "Name" : "Rib Count" }
            isInteger(definition.ribCount, { (unitless) : [1, 9, 1000] } as IntegerBoundSpec);
        }

        annotation { "Name" : "Direction", "Filter" : (EntityType.FACE && (GeometryType.PLANE || GeometryType.CYLINDER || GeometryType.CONE)) || (EntityType.EDGE && (GeometryType.LINE || GeometryType.CIRCLE || GeometryType.ARC)) || BodyType.MATE_CONNECTOR, "MaxNumberOfPicks" : 1 }
        definition.direction is Query;

        annotation { "Name" : "Opposite direction", "UIHint" : UIHint.OPPOSITE_DIRECTION }
        definition.oppositeDirection is boolean;
    }
    {
        var D = definition.ribDiameter;
        var G = definition.ribGap;
        if (definition.useVariables)
        {
            D = lengthVariableOr(context, RIB_DIAMETER_VAR, D);
            G = lengthVariableOr(context, RIB_GAP_VAR, G);
        }
        const pitch = D + G;

        const faces = definition.faces;
        if (isQueryEmpty(context, faces))
            throw regenError("Select the faces to put the ribs on", ["faces"]);

        const partBody = qOwnerBody(faces);
        if (size(evaluateQuery(context, partBody)) != 1)
            throw regenError("The faces must all belong to one part", ["faces"]);

        // Direction across the faces: the selection, or the faces' own axis
        // (a cylinder or cone) when nothing is selected.
        var dir = extractDirection(context, definition.direction);
        if (dir == undefined && isQueryEmpty(context, definition.direction))
            dir = extractDirection(context, qNthElement(faces, 0));
        if (dir == undefined)
            throw regenError("Select a flat face, a straight or circular edge, a cylinder or a mate connector for the direction", ["direction"]);
        dir = normalize(dir);

        // Extent of the faces along the direction; ribs start at one end
        // ("Opposite direction" picks the other end).
        const cSys = coordSystem(vector(0, 0, 0) * meter, perpendicularVector(dir), dir);
        const facesBox = evBox3d(context, { "topology" : faces, "tight" : true, "cSys" : cSys });
        const span = facesBox.maxCorner[2] - facesBox.minCorner[2];
        const start = definition.oppositeDirection ? facesBox.maxCorner[2] : facesBox.minCorner[2];
        const step = definition.oppositeDirection ? -1 : 1;

        var count;
        if (definition.fillPart)
        {
            count = floor((span - D) / pitch + 1e-6) + 1;
            if (count < 1)
                throw regenError("The faces are too narrow along the direction for one rib", ["faces"]);
        }
        else
        {
            count = definition.ribCount;
        }

        // Cutting planes are centred on the faces and large enough to cross them.
        const worldBox = evBox3d(context, { "topology" : faces, "tight" : true });
        const centre = (worldBox.minCorner + worldBox.maxCorner) / 2;
        const planeSize = 2 * norm(worldBox.maxCorner - worldBox.minCorner);

        var ribBodies = [];
        for (var i = 0; i < count; i += 1)
        {
            const sub = id + ("rib" ~ i);
            const z = start + step * (D / 2 + i * pitch);

            // Section of the faces at this height: the rib's path, on the surface.
            const planeId = sub + "plane";
            const cutId = sub + "cut";
            opPlane(context, planeId, {
                "plane" : plane(centre + dir * (z - dot(centre, dir)), dir),
                "width" : planeSize,
                "height" : planeSize
            });
            try silent
            {
                opIntersectFaces(context, cutId, { "tools" : qCreatedBy(planeId, EntityType.FACE), "targets" : faces });
            }
            const sectionEdges = qCreatedBy(cutId, EntityType.EDGE);
            if (isQueryEmpty(context, sectionEdges))
                throw regenError("Rib " ~ (i + 1) ~ " is past the end of the selected faces", ["faces"]);

            // One rib per connected curve of the section.
            const paths = constructPaths(context, sectionEdges, {});
            var failed = false;
            for (var j = 0; j < size(paths); j += 1)
            {
                const path = paths[j];

                // Circle profile centred on the path, normal to its tangent. On a
                // closed path start on a straight segment when there is one, so the
                // profile sits square to a flat face; an open path starts at its end.
                var tangent = evPathTangentLines(context, path, [0]).tangentLines[0];
                if (path.closed)
                {
                    for (var edge in path.edges)
                    {
                        if (!isQueryEmpty(context, qGeometry(edge, GeometryType.LINE)))
                        {
                            tangent = evEdgeTangentLine(context, { "edge" : edge, "parameter" : 0.5 });
                            break;
                        }
                    }
                }
                const skId = sub + ("prof" ~ j);
                var sk = newSketchOnPlane(context, skId, { "sketchPlane" : plane(tangent.origin, tangent.direction) });
                skCircle(sk, "c", { "center" : vector(0, 0) * millimeter, "radius" : D / 2 });
                skSolve(sk);

                const sweepId = sub + ("sweep" ~ j);
                try silent
                {
                    opSweep(context, sweepId, {
                        "profiles" : qSketchRegion(skId),
                        "path" : qUnion(path.edges)
                    });
                    ribBodies = append(ribBodies, qCreatedBy(sweepId, EntityType.BODY));
                }
                catch
                {
                    failed = true;
                }
                opDeleteBodies(context, sub + ("delsk" ~ j), { "entities" : qCreatedBy(skId, EntityType.BODY) });
                if (failed)
                    break;
            }
            opDeleteBodies(context, sub + "delcut", { "entities" : qUnion([qCreatedBy(cutId, EntityType.BODY), qCreatedBy(planeId, EntityType.BODY)]) });
            if (failed)
                throw regenError("Rib " ~ (i + 1) ~ " could not be swept along the faces", ["faces"]);
        }

        // Part first so the result keeps its name, appearance and material.
        opBoolean(context, id + "union", {
            "tools" : qUnion(concatenateArrays([[partBody], ribBodies])),
            "operationType" : BooleanOperationType.UNION
        });
        setVariable(context, "ribsMade", count);
    }, {
        useVariables : false,
        diameterVariable : "",
        gapVariable : "",
        ribDiameter : 1.6 * millimeter,
        ribGap : 0.6 * millimeter,
        oppositeDirection : false,
        fillPart : true,
        ribCount : 9,
        direction : qNothing()
    });

const RIB_DIAMETER_VAR = "rib_diameter";
const RIB_GAP_VAR = "rib_pattern_empty_space";

/**
 * When the feature is created, tick "Use Existing Variables" if the Part Studio has
 * #rib_diameter or #rib_pattern_empty_space; otherwise leave it off so the
 * typed values are used.
 */
export function yRibsOnFacesEditingLogic(context is Context, id is Id, oldDefinition is map, definition is map,
    isCreating is boolean, specifiedParameters is map, hiddenBodies is Query) returns map
{
    if (isCreating && specifiedParameters.useVariables != true)
    {
        definition.useVariables = hasLengthVariable(context, RIB_DIAMETER_VAR) || hasLengthVariable(context, RIB_GAP_VAR);
    }
    definition.diameterVariable = variableStatus(context, RIB_DIAMETER_VAR);
    definition.gapVariable = variableStatus(context, RIB_GAP_VAR);
    return definition;
}

// "#name" when the variable exists, "#name (missing)" otherwise.
function variableStatus(context is Context, name is string) returns string
{
    return "#" ~ name ~ (hasLengthVariable(context, name) ? "" : " (missing)");
}

function hasLengthVariable(context is Context, name is string) returns boolean
{
    return lengthVariableOr(context, name, undefined) != undefined;
}

// Value of a length variable, or `fallback` if it is missing or not a length.
function lengthVariableOr(context is Context, name is string, fallback)
{
    try silent
    {
        const v = getVariable(context, name);
        if (v is ValueWithUnits && v.unit == LENGTH_UNITS && v.value > 0)
            return v;
    }
    return fallback;
}
