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
| 6 | Relation's mate suppressed | (error string exists, but live shows Inactive: row 17) | "Relation mate is suppressed" | 🔧 |
| 7 | Relation's mate failed | "One or more mates in this relation are invalid." | "Relation mate could not be resolved" | 🔧 |
| 8 | Width with one connector | "Two width mate connectors are required." | "Width mate needs two width mate connectors" | 🔧 |
| 9 | Secondary axis ∥ primary | "Failed to resolve mate connector. Secondary axis conflicts with primary axis." | "Secondary axis can't be parallel to the primary axis" | 🔧 |
| 10 | Tangent on unsupported geometry / on a connector | "Geometry is not supported for tangent mate." | two long texts of ours | 🔧 (E9: cylinder–cylinder is supported; connectors are not pickable) |
| 11 | Replicate, nothing matches | "Selected target did not contain identical geometry." | "No matching geometry found (on the target instance)" | 🔧 |
| 12 | Both connectors on one instance | "Mate connectors are on same instance." | "Both mate connectors are on the same rigid set" | 🔧 (E1: the dialog refuses the pick, row 24) |
| 13 | Both in one group | "Mate cannot be added between member of same group." | same as 12 | 🔧 (E2 confirmed) |
| 14 | Both instances fixed (E3) | "Mate is between fixed instances."; the fix icons get a yellow ⚠, the root an error badge | no check (solves as fixed) | 🔧 |
| 15 | Mated instance suppressed (E4) | not an error: "Revolute 2 (Inactive: One or more instances is suppressed)" | inactive, no reason | 🔧 |
| 16 | Relations fighting (E8: two gears, ratios 1 and 2, one pair) | no error | "Relation could not be solved (conflicts with other mates or relations)" | 🔧 |
| 17 | Relation on a suppressed / inactive mate (E4, E5) | Inactive: "Gear 1 (Inactive: One or more mates is inactive or suppressed)" | error "One or more mates in this relation are suppressed." (row 6) | 🔧 |
| 18 | Mate limits min > max (E11) | "Minimum mate limit should be smaller than maximum limit. A parameter value is not valid.", both limit inputs pink | no check (both limits kept) | 🔧 |
| 19 | Replicate without target scope / mate (E12) | "A target scope must be selected." / "Missing mate to be matched." (dialog title) | "Select parts to find match in" / our own | 🔧 |
| 20 | Pattern count 0 or 2501 (E13) | count pink, title black, tooltip "… did not regenerate properly:  A parameter value is not valid." (two spaces) | pink, no tooltip | 🔧 |
| 21 | Pattern distance / angle 0 (E13) | "Linear pattern distance must be positive. A parameter value is not valid." / "Circular pattern angle must be positive. …" | no check | 🔧 |
| 22 | Insert the assembly into itself (E14) | not offered; search "No assemblies match current search." | not offered; "No results." | 🔧 |
| 24 | Second pick on the same instance (E1) | not accepted; title "Fastened 1 did not regenerate properly: Mate needs two mate connectors." | a yellow alert "Select a mate connector on a different instance" | 🔧 |
| 25 | Dialog errors (all assembly feature dialogs) | red title with tooltip "<name> did not regenerate properly: <text>" (mate, relation, pattern) | red title, no tooltip; mate errors in a yellow alert box | 🔧 |
| 26 | Tangent dialog fields | two "Face, edge or vertex" fields; incomplete: "Tangent mate needs two entities." | one "Mate connectors" field | 🔧 |
| 27 | Width tab field | "Tab mate connector(s)" | "Tab mate connectors" | 🔧 |
| 28 | Part deleted (E15) | instance rows red "Failed to resolve instance."; connector mate "Failed to resolve the instance for mate." (its chip red); face mate "Selection is invalid." with "Missing entity of …" chips; mate connector "Failed to resolve mate connector origin."; badges "Assembly contains error" / "Mate error" | ours differs | ✗ |
| 29 | Secondary axis conflict (E16) | only the secondary chip red | ours differs | ✗ |
| 30 | Animate | absent for Ball / Fastened; 0→360 deg / 0→25 mm, Steps 300, Reciprocate, DOF chooser | ours differs | ✗ |
| 23 | Mate's part deleted | E15: "Failed to resolve the instance for mate." (row 28) | "Mate cannot resolve mate connectors." (MT9) | ✗ |

## Mates (dialogs, defaults, DOF)
Fastened and Revolute: see workflow-mates.md (MT1–MT9). Slider, Planar, Cylindrical, Pin slot, Ball, Parallel,
Tangent, Width: ? (dialog dumps requested).

## Relations, patterns, Replicate, Group, Insert
? (dialog dumps requested); Group and Insert dialogs: dom/dialog-assembly-group.txt, dialog-assembly-insert.txt
(compared in earlier passes, A-items in workflow-assembly.md).
