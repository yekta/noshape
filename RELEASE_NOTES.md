**Faster**
- Big assemblies: evaluating again and dragging parts in large mated assemblies is many times faster (a 96-part assembly re-evaluates in 25 ms instead of almost a second).
- Clicking or hovering in a document with many Feature Studios no longer stalls.
- Hover responds at once after the mouse rests, even on heavy documents.

**Fixed**
- Fillets, sweeps and Booleans give the same result on a slow or busy computer as on a fast one.
- Thicken, threads, undercuts and Replace face build as in Onshape; sheet metal parts keep their references after a thicken.
- Shell no longer leaves a tiny stray solid beside the wall, and several fuse and mirror cases that failed now build.
- A Variable Studio can use its own configuration variables, and a mate offset naming an undefined variable is refused as in Onshape.

**Closer to Onshape**
- Sheet metal: Jog builds its two bends as in Onshape.
- Assemblies: a relation can drive a mate inside a flexible subassembly; Tangent mates flip as in Onshape; mate previews use Onshape's colours; Insert › Other documents lists released items.
- Frames: frame members, trims, holes and cut lists match Onshape on large weldments.
- Drawings: centerlines on cylinders, tables, dimensions and panels look and act as in Onshape.
- Export: URDF export as Onshape writes it; IGES compatibility export, DXF layers and glTF and Parasolid v28 import match Onshape.
- Part Studio: Section view… appears in a feature row's menu as in Onshape; the rollback bar and menu paste behave as in Onshape.
