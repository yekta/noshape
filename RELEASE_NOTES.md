**Fixed**
- A document whose Draft could hang forever on Mac and Windows now opens (under 9 s on both).
- The same saved document always rebuilds to the same parts, on any machine and under any load (a Draft and some custom features used to depend on how fast the machine was).
- Parasolid files of several parts exported from noshape now open in Onshape (Onshape rejected them before).
- A length typed while drawing a line, circle or rectangle keeps its unit ("20 mm" stays 20 mm when you change the document's units).
- A sketch fillet keeps the old corner as a point, and Escape in sketch fillet or chamfer drops the corner being previewed.
- Make transparent and Zoom to selection leave your selection as it was.

**Closer to Onshape**
- Sheet metal: patterns and mirrors of sheet metal parts work as in Onshape, and "Merge with all" no longer takes active sheet metal parts.
- A new feature adds only into the parts it touches.
- Parasolid export keeps extruded ellipses and splines exact; STEP import no longer adds extra vertices on circles.
- Sketch Mirror, the Line tool's tangent arc and the feature row's menu ("Clear selection") as in Onshape; the Properties dialog stays open on Escape, as in Onshape.
- Drawings: a picked part's first view is scaled as in Onshape; hole tables list only Hole features' holes.
- The search in the toolbar ranks tools as Onshape does.

**Faster**
- Custom features (FeatureScript) run up to several times faster: the 1024 × 1024 Displacement map builds in about 1.7 s (was 10 s).
- A sheet metal louver document: 616 s → about 12 s. A helical thread sweep: 7 s → 0.16 s. A Draft's face removal: 4.6 → 2.9 s. A wave spring: 0.9 → 0.34 s, faster than Onshape.
- Opening a feature dialog takes about 5 ms of work, well within a frame.

**Updates**
- Releases are now built with GCC 15 for every 64-bit Intel/AMD processor, and each release is timed on all three systems.
