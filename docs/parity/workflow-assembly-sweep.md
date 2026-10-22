# Workflow: assembly behaviour sweep (mates, relations, patterns, Replicate, Group, Insert)

skb, 2026-10-22, after psb's feature sweep. Every mate type (Fastened, Revolute, Slider, Planar, Cylindrical,
Pin slot, Ball, Parallel, Tangent, Width), the relations (Gear, Rack and pinion, Screw, Linear), the assembly
patterns (Linear, Circular, Part pattern, Mirror), Replicate, Group and the Insert dialog.
Compared: defaults, field order and labels, error texts (and the field each marks), and the result.
Sources: Onshape's error strings (`/root/onshape-research/docs-raw/fs-std/errorstringenum.gen.fs`), live dialog
dumps (`live/dom/dialog-mate-*.txt`, `dialog-assembly-*.txt`, `assembly-insert-*.txt`), behaviours.md.
Live request "skb (10-22 20:30, assembly sweep)" in live/REQUESTS.md, items E1–E16 below.
Status: ✅ matches, ✗ differs (open), 🔧 fixed on agent/skb, ? live pending (Enn).

## Error texts
Onshape shows a failed mate / relation as a red row with the tooltip "<name> has error: <text>"; the texts are
the error-string table's.

| # | Situation | Onshape | Ours (before) | Status |
|---|---|---|---|---|
| 1 | Gear on wrong mate types | "Gear relations require two rotational degrees of freedom." | same without the period | 🔧 |
| 2 | Rack and pinion on wrong types | "Rack and pinion relations require one translational degree of freedom and one rotational degree of freedom." | "…require one rotational and one translational degree of freedom" | 🔧 |
| 3 | Screw on wrong types | "Screw relations require one cylindrical mate." | no period | 🔧 |
| 4 | Linear on wrong types | "Linear relations require two translational degrees of freedom." | no period | 🔧 |
| 5 | Relation's mate deleted | "One or more mates in this relation do not exist." | "Relation mate does not exist" | 🔧 |
| 6 | Relation's mate suppressed | "One or more mates in this relation are suppressed." | "Relation mate is suppressed" | 🔧 |
| 7 | Relation's mate failed | "One or more mates in this relation are invalid." | "Relation mate could not be resolved" | 🔧 |
| 8 | Width with one connector | "Two width mate connectors are required." | "Width mate needs two width mate connectors" | 🔧 |
| 9 | Secondary axis ∥ primary | "Failed to resolve mate connector. Secondary axis conflicts with primary axis." | "Secondary axis can't be parallel to the primary axis" | 🔧 |
| 10 | Tangent on unsupported geometry / on a connector | "Geometry is not supported for tangent mate." | two long texts of ours | 🔧 ? E9 |
| 11 | Replicate, nothing matches | "Selected target did not contain identical geometry." | "No matching geometry found (on the target instance)" | 🔧 ? E12 |
| 12 | Both connectors on one instance | "Mate connectors are on same instance." | "Both mate connectors are on the same rigid set" | 🔧 ? E1 |
| 13 | Both in one group | "Mate cannot be added between member of same group." | same as 12 | 🔧 ? E2 |
| 14 | Both instances fixed | "Mate is between fixed instances." | no check (solves as fixed) | ? E3 |
| 15 | Mated instance suppressed | "One or more mated instance is suppressed." or "Instance of mate connector is suppressed." | "Instance is suppressed" | ? E4 |
| 16 | Relation over-defining | "Relation overdefines assembly." / "Relation is not consistent." | "Relation could not be solved (conflicts with other mates or relations)" | ? E8 |
| 17 | Relation with one mate twice | "Relation is invalid." (?) | "A relation needs two different mates" | ? E5 |
| 18 | Mate limits min > max | "Minimum mate limit should be smaller than maximum limit." | no check (both limits kept) | ✗ ? E11 |
| 19 | Replicate without target scope | "A target scope must be selected." (dialog) | "Select parts to find match in" | ? E12 |
| 20 | Replicate seed mate on a connector / not mating the seed | "Invalid mate to be matched." / "Missing mate to be matched." | our own texts | ? E12 |
| 21 | Pattern limits | "Linear pattern distance must be positive.", "Circular pattern angle must be positive.", "Total pattern instances cannot exceed 2500.", "Instance count cannot be less than 1." | no distance / angle check; the count is clamped to 1–2500 silently ("Instance count must be a whole number" otherwise) | ✗ ? E13 |
| 22 | Insert creating a cycle | "The instance could not be inserted as it would create a cyclic relationship." | "Circular assembly reference" (on the node) | ? E14 |
| 23 | Mate's part deleted | "Mate cannot resolve mate connectors." | same (MT9) | ✅ |

## Mates (dialogs, defaults, DOF)
Fastened and Revolute: see workflow-mates.md (MT1–MT9). Slider, Planar, Cylindrical, Pin slot, Ball, Parallel,
Tangent, Width: ? (dialog dumps requested).

## Relations, patterns, Replicate, Group, Insert
? (dialog dumps requested); Group and Insert dialogs: dom/dialog-assembly-group.txt, dialog-assembly-insert.txt
(compared in earlier passes, A-items in workflow-assembly.md).
