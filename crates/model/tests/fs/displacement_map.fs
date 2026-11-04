/**
 * Work with The Onsherpa to reach peak Onshape Productivity
 * www.TheOnsherpa.com
 *
 * Copyright (c) 2026 Evan Reese
 *
 * Licensed under the MIT License.
 *
 * Permission is granted to anyone to use, copy, modify, merge, publish,
 * distribute, sublicense, and/or sell this software, including for
 * commercial purposes, provided this copyright notice and license are
 * retained.
 *
 * This software is provided "as is", without warranty of any kind.
 *
 * Full license: https://opensource.org/licenses/MIT
 */

FeatureScript 3044;
export import(path : "onshape/std/common.fs", version : "3044.0");
export import(path : "onshape/std/constrainedSurface.fs", version : "3044.0");

export enum DisplacementSurfaceType
{
    annotation { "Name" : "B-spline Surface" }
    B_SPLINE,
    annotation { "Name" : "Constrained surface" }
    CONSTRAINED_SURFACE,
}

annotation { "Feature Type Name" : "Displacement map",
        "Feature Type Description" : "Use pixel values to create surface deformations. Useful for texturing parts, modeling topology, and creating lithophanes. The image must be converted to CSV using the free tool at https://theonsherpa.github.io/Image-to-CSV/",
        "Editing Logic Function" : "displacementEditingLogic" }
export const displacementMap = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Image file (CSV)", "Description" : "To convert your image go to https://theonsherpa.github.io/Image-to-CSV/" }
        definition.imageTable is TableData;

        annotation { "Name" : "Surface to displace", "Filter" : EntityType.FACE, "MaxNumberOfPicks" : 1 }
        definition.face is Query;

        annotation { "Name" : "flip", "UIHint" : ["OPPOSITE_DIRECTION", "FIRST_IN_ROW"] }
        definition.flip is boolean;

        annotation { "Name" : "Reorient secondary axis", "UIHint" : ["MATE_CONNECTOR_AXIS_TYPE", "DISPLAY_SHORT"], "Default" : MateConnectorAxisType.PLUS_X }
        definition.secondaryAxisType is MateConnectorAxisType;

        annotation { "Name" : "Black value" }
        isLength(definition.blackValue, { (millimeter) : [-1e6, 0, 1e6] } as LengthBoundSpec);

        annotation { "Name" : "White value" }
        isLength(definition.whiteValue, { (millimeter) : [-1e6, 1, 1e6] } as LengthBoundSpec);

        // Hidden for now since constrained surface seems to fail too easily to be useful, but we can turn it back on easily this way.
        annotation { "Name" : "Surface type", "UIHint" : UIHint.ALWAYS_HIDDEN }
        definition.surfaceType is DisplacementSurfaceType;

        if (definition.surfaceType == DisplacementSurfaceType.CONSTRAINED_SURFACE)
        {
            annotation { "Name" : "Tolerance" }
            isLength(definition.tolerance, TOLERANCE_BOUND);

            annotation { "Name" : "Optimize", "UIHint" : UIHint.SHOW_LABEL }
            definition.optimize is OptimizationMethod;
        }

        if (definition.surfaceType == DisplacementSurfaceType.B_SPLINE)
        {
            annotation { "Name" : "Degree" }
            isInteger(definition.degree, { (unitless) : [2, 5, 15] } as IntegerBoundSpec);
        }

        annotation { "Name" : "Replace face", "Default" : false }
        definition.replaceFace is boolean;

        annotation { "Name" : "Use caching", "Description" : "When caching is used, control point positions are not regenerated. This speeds up the feature considerably, but it will not update if you change the base surface unless you click \" Update cache \"" }
        definition.useCaching is boolean;

        if (definition.useCaching)
        {
            annotation { "Group Name" : "Use caching", "Driving Parameter" : "useCaching", "Collapsed By Default" : false }
            {
                annotation { "Name" : "Update cache" }
                isButton(definition.updateCache);

                annotation { "Name" : "Warn when out of date", "Description" : "When on, monitors the input face for changes and notifies the user when it is out of date." }
                definition.warnWhenOutOfDate is boolean;

                annotation { "Name" : "Cached face", "UIHint" : UIHint.ALWAYS_HIDDEN }
                isAnything(definition.cachedFace);

                annotation { "Name" : "Cached points", "UIHint" : UIHint.ALWAYS_HIDDEN }
                isAnything(definition.cachedPoints);
            }
        }

        annotation { "Name" : "Show image coordinates" }
        definition.showCoord is boolean;
    }
    {
        checkIfFaceIsOutOfDate(definition, context, id);

        var pointsTable;
        if (definition.useCaching && definition.cachedPoints is array)
        {
            pointsTable = rehydrateCachedPointsTable(definition.cachedPoints);
        }
        else
        {
            pointsTable = createPointsTable(context, definition, id);
        }

        if (definition.surfaceType == DisplacementSurfaceType.CONSTRAINED_SURFACE)
            doConstrainedSurfaceMethod(pointsTable, definition, id + "constrainedSurface", context);

        if (definition.surfaceType == DisplacementSurfaceType.B_SPLINE)
            doBSplineSurfaceMethod(pointsTable, definition, id + "bSplineSurface", context);

        if (definition.replaceFace)
        {
            const template = qCreatedBy(id + "bSplineSurface", EntityType.FACE)->qUnion(qCreatedBy(id + "constrainedSurface", EntityType.FACE));
            try silent
            {
                opReplaceFace(context, id + "replaceFace1", {
                            "replaceFaces" : definition.face,
                            "templateFace" : template,
                            "oppositeSense" : false
                        });
            }
            catch (error)
            {
                opReplaceFace(context, id + "replaceFace2", {
                            "replaceFaces" : definition.face,
                            "templateFace" : template,
                            "oppositeSense" : true
                        });
            }

            opDeleteBodies(context, id + "deleteBodies1", {
                        "entities" : qOwnerBody(template)
                    });
        }

        if (definition.showCoord)
            showImageCsys(definition, context, id);
    });

function showImageCsys(definition is map, context is Context, id is Id)
{
    var imagePlane = evFaceTangentPlane(context, {
            "face" : definition.face,
            "parameter" : vector(0.5, 0.5)
        });

    if (!definition.flip)
    {
        imagePlane.normal *= -1;
    }

    const planeAxis = line(imagePlane.origin, imagePlane.normal);
    const sign = definition.flip ? -1 : 1;
    const quarterTurns = switch (definition.secondaryAxisType)
        {
                MateConnectorAxisType.PLUS_X : 1,
                MateConnectorAxisType.PLUS_Y : 2,
                MateConnectorAxisType.MINUS_X : 3,
                MateConnectorAxisType.MINUS_Y : 0
            };

    imagePlane = rotationAround(planeAxis, sign * quarterTurns * 90 * degree) * imagePlane;

    debug(context, coordSystem(imagePlane));
}

function remap(value is number, fromLow is number, fromHigh is number, toLow is number, toHigh is number) returns number
{
    return toLow + (value - fromLow) * (toHigh - toLow) / (fromHigh - fromLow);
}

function transposeNestedArray(table is array) returns array
{
    var result = [];
    for (var j = 0; j < size(table[0]); j += 1)
    {
        var column = [];
        for (var row in table)
        {
            column = append(column, row[j]);
        }
        result = append(result, column);
    }
    return result;
}

/**
 * Caching strips out the vector designation, so this adds it back.
 */
function rehydrateCachedPointsTable(cachedPointsTable is array) returns array
{
    return mapArray(cachedPointsTable, function(row)
        {
            return mapArray(row, function(vec)
                {
                    return vec as Vector;
                });
        });
}

/**
 * A cheap fingerprint of the face: its area and a few tangent planes, rounded to 1e-9 m.
 */
function currentFaceFingerprint(context is Context, definition is map) returns array
{
    const planes = evFaceTangentPlanes(context, {
                "face" : definition.face,
                "parameters" : [vector(0, 0), vector(1, 0), vector(0, 1), vector(1, 1), vector(0.5, 0.5), vector(0.25, 0.75)]
            });
    var result = [roundToPrecision(evArea(context, { "entities" : definition.face }) / meter ^ 2, 12)];
    for (var plane in planes)
    {
        for (var k = 0; k < 3; k += 1)
        {
            result = append(result, roundToPrecision(plane.origin[k] / meter, 9));
            result = append(result, roundToPrecision(plane.normal[k], 9));
        }
    }
    return result;
}

function checkIfFaceIsOutOfDate(definition is map, context is Context, id is Id) returns boolean
{
    if (!definition.useCaching || !definition.warnWhenOutOfDate)
        return true;

    if (definition.cachedFace == currentFaceFingerprint(context, definition))
        return true;

    reportFeatureWarning(context, id, "Input face has been modified. Edit the feature to update the cache.");
    return false;
}

function doConstrainedSurfaceMethod(pointsTable is array, definition is map, id is Id, context is Context)
{
    var constrainedSurfaceDefinition = {};
    constrainedSurfaceDefinition.smooth = definition.optimize == OptimizationMethod.SMOOTH;
    constrainedSurfaceDefinition.tolerance = definition.tolerance;

    constrainedSurfaceDefinition.points = mapArray(concatenateArrays(pointsTable), function(point)
        {
            return { "point" : point };
        });

    opConstrainedSurface(context, id, constrainedSurfaceDefinition);
}

function doBSplineSurfaceMethod(pointsTable is array, definition is map, id is Id, context is Context)
{
    const rowCount = size(pointsTable);
    const columnCount = size(pointsTable[0]);
    if (rowCount <= definition.degree || columnCount <= definition.degree)
    {
        throw regenError("The image needs more than " ~ definition.degree ~ " rows and columns for degree " ~ definition.degree ~ ".", ["degree"]);
    }

    const facePeriodicity = evFacePeriodicity(context, { "face" : definition.face });

    const surface = bSplineSurface({
                "uDegree" : definition.degree,
                "vDegree" : definition.degree,
                "isUPeriodic" : facePeriodicity[0],
                "isVPeriodic" : facePeriodicity[1],
                "controlPoints" : controlPointMatrix(pointsTable)
            });

    opCreateBSplineSurface(context, id, {
                "bSplineSurface" : surface
            });
}

function readImageTable(definition is map) returns array
{
    const table = definition.imageTable.csvData;
    if (!(table is array) || size(table) < 2 || !(table[0] is array) || size(table[0]) < 2)
    {
        throw regenError("Select a CSV file of pixel values (at least 2 rows and 2 columns).", ["imageTable"]);
    }
    for (var row in table)
    {
        if (size(row) != size(table[0]))
            throw regenError("Every row of the CSV file needs the same number of values.", ["imageTable"]);
        for (var item in row)
        {
            if (!(item is number))
                throw regenError("The CSV file must hold only pixel values (0 to 255).", ["imageTable"]);
        }
    }
    return table;
}

function createPointsTable(context is Context, definition is map, id is Id) returns array
{
    var table = readImageTable(definition);

    if (definition.flip)
        table = reverse(table);

    const rotateCount = switch (definition.secondaryAxisType)
        {
                MateConnectorAxisType.PLUS_X : 0,
                MateConnectorAxisType.PLUS_Y : 1,
                MateConnectorAxisType.MINUS_X : 2,
                MateConnectorAxisType.MINUS_Y : 3
            };

    for (var i = 0; i < rotateCount; i += 1)
    {
        table = transposeNestedArray(table)->reverse();
    }

    const rowCount = size(table);
    const columnCount = size(table[0]);

    reportFeatureInfo(context, id, rowCount * columnCount ~ " samples");

    var parameters = [];
    for (var i = 0; i < rowCount; i += 1)
    {
        for (var j = 0; j < columnCount; j += 1)
        {
            parameters = append(parameters, vector(i / (rowCount - 1), j / (columnCount - 1)));
        }
    }

    const planes = evFaceTangentPlanes(context, {
                "face" : definition.face,
                "parameters" : parameters
            });

    var pointsTable = [];
    for (var i, row in table)
    {
        var pointRow = [];
        for (var j, item in row)
        {
            const plane = planes[i * columnCount + j];
            const offset = remap(item, 0, 255, definition.blackValue / meter, definition.whiteValue / meter) * meter;
            pointRow = append(pointRow, plane.origin + offset * plane.normal);
        }
        pointsTable = append(pointsTable, pointRow);
    }

    return pointsTable;
}

export function displacementEditingLogic(context is Context, id is Id, oldDefinition is map, definition is map,
    isCreating is boolean, specifiedParameters is map, hiddenBodies is Query, clickedButton is string) returns map
{
    if (!definition.useCaching)
        return definition;

    const turnedOn = !(oldDefinition.useCaching == true);
    const inputsChanged = oldDefinition.imageTable != definition.imageTable || oldDefinition.face != definition.face ||
        oldDefinition.flip != definition.flip || oldDefinition.secondaryAxisType != definition.secondaryAxisType ||
        oldDefinition.blackValue != definition.blackValue || oldDefinition.whiteValue != definition.whiteValue;

    if (!(definition.cachedPoints is array) || clickedButton == "updateCache" || turnedOn || inputsChanged)
    {
        try silent
        {
            definition.cachedPoints = createPointsTable(context, definition, id);
            definition.cachedFace = currentFaceFingerprint(context, definition);
        }
    }

    return definition;
}
