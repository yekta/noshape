**Faster**
- Documents open much faster the second time: Noshape now keeps each Part Studio's finished results on disk, so a reopen shows the parts without rebuilding them. A user's 137-feature model reopens in about 1 s, and the impeller test model in 0.02 s instead of 15 s. If anything changes, inside the studio or in what it uses (variables, linked tabs, Feature Studios, configurations, units, files), it rebuilds as before.
- A user's model with an imported Mac mini opens about twice as fast: the big union with the imported body runs in Noshape's own Boolean (43 s → under 2 s on a test machine), the imported body is meshed once and quickly, and its chamfers are faster.
- While a model builds, the app stays responsive: building runs below the screen's priority.
- Fillets and chamfers are faster in general (an OCCT improvement), and Export of a CAD file tab works.

**Fixed**
- Thin extrudes no longer make invalid end faces (their chamfers took seconds and could come out slightly wrong).
- A Union with Keep tools now makes a new part and keeps the tools, as in Onshape.
- On Windows, a rare failure reading back a Rhino export now retries.

**Closer to Onshape**
- Shell matches Onshape on free-form walls; tilted-rim chamfers and the Chamfer FeatureScript operation match Onshape's results.
- A Feature Studio at a version is view-only with Onshape's banner; Ctrl+click on a standard library symbol keeps the cursor in place.
- Release… on another document's part, Create version and update in assembly, Variable Studio undo, documents search and several drawing and versions panels as in Onshape.

**Updates**
- Checked by qa on this exact build: the saved-results cache across every test model (no difference with or without it, after changes, and with damaged entries), gear and ticket documents, sweeps and the model suite. This release is late: GitHub's Linux machines couldn't reach Ubuntu's servers for most of the afternoon.
