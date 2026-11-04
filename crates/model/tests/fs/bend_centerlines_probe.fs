FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");
import(path : "onshape/std/sheetMetalUtils.fs", version : "3095.0");

// dx's probe (REQUESTS 2026-11-04 17:55): the bend centerline wire bodies std sheet metal keeps, for a part's bends.
annotation { "Feature Type Name" : "Bend centerline probe" }
export const bendCenterlineProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Sheet metal part", "Filter" : EntityType.BODY && BodyType.SOLID, "MaxNumberOfPicks" : 1 }
        definition.part is Query;
    }
    {
        const smBody = qOwnerBody(qUnion(getSMDefinitionEntities(context, qOwnedByBody(definition.part, EntityType.FACE))));
        const smEdges = qGeometry(qEdgeTopologyFilter(qOwnedByBody(smBody, EntityType.EDGE), EdgeTopology.TWO_SIDED), GeometryType.LINE);
        const smCylinders = qGeometry(qOwnedByBody(smBody, EntityType.FACE), GeometryType.CYLINDER);
        for (var attribute in getSMAssociationAttributes(context, qUnion([smEdges, smCylinders])))
        {
            const wires = evaluateQuery(context, qEntityFilter(qBodyType(qAttributeQuery(attribute), BodyType.WIRE), EntityType.BODY));
            for (var w in wires)
            {
                const def = qNthElement(qAttributeFilter(qUnion([smEdges, smCylinders]), attribute), 0);
                const joint = try silent(getJointAttribute(context, def));
                println("wire " ~ toString(w) ~ " joint " ~ (joint == undefined ? "none" : toString(joint.jointType.value)) ~ " angle " ~ (joint == undefined || joint.angle == undefined ? "-" : toString(joint.angle.value)) ~ " up " ~ toString(try silent(evSheetMetalBendUp(context, { "wireBody" : w }))));
                println("  definition " ~ toString(evaluateQuery(context, def)) ~ " " ~ toString(try silent(evBox3d(context, { "topology" : def }))));
                for (var e in evaluateQuery(context, qOwnedByBody(w, EntityType.EDGE)))
                {
                    println("  edge " ~ toString(evVertexPoint(context, { "vertex" : qEdgeVertex(e, false) })) ~ " -> " ~ toString(evVertexPoint(context, { "vertex" : qEdgeVertex(e, true) })));
                }
            }
        }
    });
