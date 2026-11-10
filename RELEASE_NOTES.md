**Fixed**
- On a Mac, models now compute exactly as on Windows and Linux (Mac builds fused some arithmetic differently, which made some fillets fail and take long, only on a Mac).
- Renaming a variable now also renames it in sketch dimensions and in list parameters (they used to break with an undefined variable).
- An added custom feature shows as added in the Add custom features panel, and clicking it again removes it instead of adding it twice; the "added to your toolbar" message stays long enough to read.
- Opening the fan-blade style documents no longer hangs for minutes on a thread sweep; the command-line tool no longer crashes on exit on Windows.
- Delete face removes bosses through curved walls where it used to fail.

**Closer to Onshape**
- Import files… › Split into multiple documents builds linked documents per subassembly; imports run in the background with progress in the notifications bell.
- Documents page: Labels… opens the Details panel; Versions and history from a document's menu; sort menus and recent cards as in Onshape.
- Extrude start offsets with Opposite direction, arcs of one circle in a profile, and mate connectors on sketch points now match Onshape.

**Faster**
- Dialogs open within a frame; Shells and some Boolean steps are several times faster on heavy documents.

**Updates**
- Not checked by our QA agent this time (out of its weekly limit): the release was checked by the land queue, the main-branch tests and CI on all three platforms.
