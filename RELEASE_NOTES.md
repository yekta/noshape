**Fixed**
- Cancelling a feature dialog shows the model as it was at once and stops the spinner; nothing the cancelled preview delivers late is applied.
- Displacement maps of any size build, 1024 × 1024 included (Onshape refuses anything over 500,000 points), with Replace face too; every step stops within about 100 ms when cancelled.
- A notched end arc (bushing) shows, picks, measures and fillets as one edge, as in Onshape.

**Closer to Onshape**
- noshape's own fillets now build full rounds, rounds wider than their face, and equal chamfers over a narrow face exactly as Onshape does; countersink and hole rims stay whole edges.
- A face pattern of cuts through a tube copies the first cut to each position, as Onshape does.
- Sketch text boxes size their letters as Onshape; a sketch's first dimension scales the whole sketch; a free line's new length moves its start.
- Assemblies: Apply limit position as in Onshape.
- Drawings: notes, the Export dialog and more of Insert view behave as in Onshape; hidden lines in PDFs dashed as Onshape's.

**Faster**
- 3D orbit on big models meets 120 fps on Mac (back faces of solids are skipped, as in Onshape, which draws them one-sided).
- A face pattern through a tube: 158 → 8 ms. Gear documents up to 30 % faster; hose routing and sheet metal documents faster than Onshape.
- Big B-spline faces mesh with fewer triangles than before for the same quality.
