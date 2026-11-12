**Fixed**
- Sketching: a click now acts on exactly what is highlighted. Dimensions start on the line you hover, a point drawn onto a highlighted point or line stays there with a coincident constraint, and points you can see can be picked (on Retina displays, sketch points and lines were picked in the wrong place).
- Sketch snapping uses Onshape's distances on screen: 7 px to a line, 8–9 px to a point, 6 px to a midpoint, also when dragging a point and in angled views. A snapped point shows Onshape's yellow-orange square.
- In sketch mode, your sketch's points and lines win over the model edges under them.
- A hovered sketch point shows its whole orange disc on slanted sketch planes.
- An under-defined sketch keeps its badge in the feature list while you edit it.
- File dialogs remember the folder you last used, separately for export, import and opening documents.
- A saved document whose pattern direction stopped resolving in v0.4.44 builds again.

**Closer to Onshape**
- Release management: Approvers and Observers, Submit to Pending, Approve and Reject, drafts, part numbers and Onshape's item messages; Revision tables use your company's settings.
- Rib picks the direction that reaches a part, as Onshape does; Split takes the face you click; Thicken and Sweep keep Add as in Onshape; Edit curve's Elevate works on lines.
- Mass properties, named views, appearance and view cube behave as in Onshape; mirrored holes on another wall build; mirrors and patterns with Add merge their seeds.
- Drawings: the dimension palette's precision list and tooltips, and tolerances keep the drawing's precision.
- Assemblies: many more parts and mates place as in Onshape; IGES assemblies import part by part; imports can be split into documents.

**Faster**
- The Boolean handles more cases itself (the infento tyre, surface patterns of spheres, ring cuts), and several fillets, chamfers and the impeller's Fillet 1 are faster.

**Updates**
- Checked by qa on this exact build, including three users' own documents and documents saved by previous releases.
