**New**
- In-app updates: noshape checks for new releases at launch and every few minutes, downloads them in the background and shows a banner with progress, then "Updated — restart to finish". Restart reopens your documents. Every update is verified before it installs (signed by our release build; on macOS also by our Apple Developer ID). Preferences › Updates turns it off. This version still has to be installed by hand once; later ones update in place.
- macOS builds are signed with our Apple Developer ID and notarized: macOS opens noshape without the unidentified-developer warning.

**Faster**
- Edits regenerate faster: features an edit doesn't affect now keep their results instead of being rebuilt (about 15 % faster across long editing sessions).
- Big assemblies are far faster than before: regenerating 1000 instances 3.7 s → 0.08 s, and a 200-instance assembly regenerates in 19 ms (Onshape: 13–16 s); dragging parts runs at several hundred frames a second.
- Shell builds its walls directly for freeform faces too, and the first FeatureScript feature after starting the app is ready twice as fast.
- Custom features that cut or fuse many tools (like the Y Ribs example) run several times faster.
- Re-importing our own STEP files of imported threaded parts: 25 screws 433 s → 19 s, files 99 MB → 16 MB.

**Fixed**
- A twisted loft through several sections (like a twisted vase) built faces Shell couldn't offset; lofts now build proper faces, and flat parts of a loft are true planes.
- A fillet on one member of an open composite part also changed the composite.
- Shell of a part with a tube half sunk into it hollowed half the tube wall; shells reaching past half the part, and solids with voids, now fail as in Onshape.
- Exporting imported parts to Parasolid could crash; to Rhino it now writes far more of them.
- A chamfer on some imported parts could hang.
- "Install command line tool…" and "Connect to coding agents…" showed nothing on macOS; they now open at once and show their progress and results.

**Closer to Onshape**
- Sketches: when you change a dimension or regenerate, the sketch moves the way Onshape's does (a line grows from its far end, rectangles keep their corner and height, attached geometry follows rigidly).
- Assemblies: conflicting mates move nothing and show Onshape's errors ("Mate overdefines the assembly." / "Mate is not consistent."), pattern copies follow their seed, and a changed part in a mate chain moves alone.
- File tabs: PDFs show their pages, Markdown renders, CSV files show as tables, images and text as in Onshape.
