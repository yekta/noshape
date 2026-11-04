**Fixed**
- Windows: the buttons in the top bar work again. The account menu, a document's ☰ menu, Help and the bell did nothing, because Windows took every click there as a window drag.
- The documents window has a Help menu (? ▾) beside the account menu on every platform, with Install command line tool… and Connect to coding agents…, so you can see what's installed and connected and set it up. Before, Windows users could only reach them from inside a document.
- macOS: closing the last document shows the documents window, as on Windows and Linux.
- Undo and Redo do nothing while a dialog control has the keyboard, as in Onshape.
- An Onshape crash in sheet-metal louvers no longer closes the app.

**Closer to Onshape**
- Community FeatureScripts written for older Onshape versions compile against the library they were written for, so many more of them build.
- Replace face works on curved faces and with a surface on both sides of the face.
- Mate connectors can be made inside a feature (like Hole's location field), with Onshape's inference points while you hover a face.
- Ctrl+Shift+F opens Search Feature Studios from any tab.
- A deleted reference shows as "Missing Edge" (or Face / Vertex) in red, as in Onshape.
- More fillets, holes, sketches on construction planes and face patterns match Onshape exactly.

**Faster**
- A plate with 1,700 holes cuts 10× faster; many FeatureScript-driven documents regenerate faster.
- Shell is faster on parts with many faces; selecting a feature in a big Part Studio highlights it at once.

**For coding agents**
- The MCP instructions say where the app keeps documents, so files agents make show in your documents list, and when a FeatureScript custom feature beats sketches and features.
