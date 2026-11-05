FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");
import(path : "onshape/std/sheetMetalAttribute.fs", version : "3095.0");

function r6(x) { return roundToPrecision(x, 6); }
function vs(v) { return r6(v[0] / millimeter) ~ "," ~ r6(v[1] / millimeter) ~ "," ~ r6(v[2] / millimeter); }
function ds(v) { return r6(v[0]) ~ "," ~ r6(v[1]) ~ "," ~ r6(v[2]); }

function edgeInfo(context is Context, e is Query, wall is Query) returns string
{
    var s = transientQueriesToStrings(e);
    try silent
    {
        const t = evEdgeTangentLines(context, { "edge" : e, "parameters" : [0, 1], "face" : wall });
        s = s ~ " F[" ~ vs(t[0].origin) ~ " d " ~ ds(t[0].direction) ~ " ; " ~ vs(t[1].origin) ~ " d " ~ ds(t[1].direction) ~ "]";
    }
    try silent
    {
        const t = evEdgeTangentLines(context, { "edge" : e, "parameters" : [0, 1] });
        s = s ~ " N[" ~ vs(t[0].origin) ~ " d " ~ ds(t[0].direction) ~ " ; " ~ vs(t[1].origin) ~ " d " ~ ds(t[1].direction) ~ "]";
    }
    return s;
}

function list(context is Context, q is Query, wall is Query) returns string
{
    var s = "";
    for (var e in evaluateQuery(context, q))
        s = s ~ "{" ~ edgeInfo(context, e, wall) ~ "}";
    return s;
}

annotation { "Feature Type Name" : "Edge order probe" }
export const edgeOrderProbe = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "X" }
        isLength(definition.x, LENGTH_BOUNDS);
        annotation { "Name" : "Y" }
        isLength(definition.y, LENGTH_BOUNDS);
        annotation { "Name" : "Z" }
        isLength(definition.z, LENGTH_BOUNDS);
        annotation { "Name" : "Sheet metal definition" }
        definition.definition is boolean;
    }
    {
        const p = vector(definition.x, definition.y, definition.z);
        var walls = evaluateQuery(context, qContainsPoint(qEverything(EntityType.FACE), p));
        var pts = evaluateQuery(context, qContainsPoint(qEverything(EntityType.VERTEX), p));
        // The (hidden) sheet metal definition's entities there instead: what std sheet metal features work on.
        if (definition.definition)
        {
            walls = evaluateQuery(context, qUnion(getSMDefinitionEntities(context, qUnion(walls), EntityType.FACE)));
            pts = evaluateQuery(context, qUnion(getSMDefinitionEntities(context, qUnion(pts), EntityType.VERTEX)));
        }
        var k = 0;
        for (var w in walls)
        {
            const body = qOwnerBody(w);
            var bt = "SOLID";
            if (!isQueryEmpty(context, qBodyType(body, BodyType.SHEET)))
                bt = "SHEET";
            var nrm = "";
            try silent { nrm = ds(evPlane(context, { "face" : w }).normal); }
            for (var v in pts)
            {
                if (isQueryEmpty(context, qIntersection([qOwnerBody(v), body])))
                    continue;
                const qa = qAdjacent(w, AdjacencyType.EDGE, EntityType.EDGE);
                const qb = qAdjacent(v, AdjacencyType.VERTEX, EntityType.EDGE);
                var s = "WALL " ~ transientQueriesToStrings(w) ~ " body " ~ transientQueriesToStrings(body) ~ " " ~ bt ~ " n " ~ nrm ~ " | PT " ~ transientQueriesToStrings(v);
                s = s ~ " | A " ~ list(context, qa, w) ~ " | B " ~ list(context, qb, w) ~ " | C " ~ list(context, qIntersection([qa, qb]), w);
                println("EO" ~ k ~ " " ~ s);
                k += 1;
            }
        }
    });

