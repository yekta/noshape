**Fixed**
- Community FeatureScripts that build big lists (like the Displacement map, 262,144 samples) no longer time out: they build in seconds.
- Windows: Feature Studios written for older Onshape versions load (as in v0.4.27); a standard library that can't load is a clear error, never a crash.
- First launch shows what is set up (the command line tool, each coding agent) once, even when there's nothing to do; Help ▾ › Connect to coding agents… shows it any time.
- Windows installer: its last page says which coding agents it found and connects them, or that none was found; Claude Desktop from the Microsoft Store is found; noshape starts after installing.
- The Windows key no longer acts like Ctrl when stepping values or adding to a selection.

**Closer to Onshape**
- B-spline surfaces from FeatureScript (opCreateBSplineSurface), so the Displacement map builds on flat and cylindrical faces (Replace face on a 512×512 image is still being made fast).
- 93 older Onshape standard library versions ship, so more community FeatureScripts compile as they were written.
- The documents window's Help ▾ matches Onshape's, with Keyboard shortcuts first.
- Mate connector inference points look like Onshape's: white discs lying in the face, 12 px at corners.
- Sketch region edges run in the direction the lines were drawn, as patterns in Onshape expect.

**Faster**
- FeatureScript maps and unit math are much faster; several community documents regenerate 2–5× faster.
- Opening a feature dialog on a big part no longer probes every face; large B-spline faces mesh faster.
