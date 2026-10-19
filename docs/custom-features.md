# Custom features (Feature Studios)

Feature Studios hold [FeatureScript](https://cad.onshape.com/FsDoc/), Onshape's feature language, and behave
like Onshape's: a new Feature Studio starts with

```
FeatureScript 3095;
import(path : "onshape/std/common.fs", version : "3095.0");
```

and every exported `defineFeature` constant with a `"Feature Type Name"` annotation becomes a feature in the
document's Part Studios, with a dialog built from its precondition.

```
FeatureScript 3095;
import(path : "onshape/std/geometry.fs", version : "3095.0");

annotation { "Feature Type Name" : "Block" }
export const block = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
        annotation { "Name" : "Width" }
        isLength(definition.width, LENGTH_BOUNDS);
    }
    {
        fCuboid(context, id + "box", { "corner1" : vector(0, 0, 0) * meter, "corner2" : vector(definition.width, 20 * millimeter, 10 * millimeter) });
    });
```

## How it runs

- Language: `crates/featurescript` (lexer, parser, compiler to slot-resolved IR, evaluator). The Onshape
  FeatureScript Standard Library (v3095.0, MIT License, `crates/featurescript/std/`) runs unmodified.
- Modeling builtins (`@opExtrude`, `@evaluateQuery`, `@newSketch`, ...): `crates/model/src/custom/host/`.
  Operations reuse the built-in features' kernel helpers and persistent naming: everything a custom feature
  creates is named after the custom feature and the operation id (`id + "extrude1"`), so downstream features keep
  their references when parameters change.
- Imports of other Feature Studios use the tab id as the path: `import(path : "<tab id>", version : "");`.
- Errors: parse and compile errors are Feature Studio notices; runtime errors become the feature's error
  (`regenError("...")` texts, or the `ErrorStringEnum` message); `println` output and caught exceptions are kept
  per feature type (`custom::run::outputs()`) for the notices panel.
