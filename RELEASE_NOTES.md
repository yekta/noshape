**New**
- The app is also the command line tool and MCP server: `noshape mcp` and `noshape <command>`. One download instead of two.
- Installing puts `noshape` on your PATH: a Windows installer, a Linux install script and .deb, and on macOS Help › Install command line tool….
- Help › Connect to coding agents… registers noshape with Claude Code, Codex and Claude Desktop when they're installed (also `noshape agents connect`).
- A separate small CLI-only download for servers.

**Faster**
- Cancelling a dialog while its preview is still computing puts the model back at once, and the work behind it stops within a fraction of a second (it could take 20 s).
- noshape's own fillets now also round corners between drafted walls, about 10× faster than before on the slowest cases.

**Fixed**
- Drawings of a Part Studio that uses variables from a Variable Studio or derived parts were empty.
- The sheet metal table could show the wrong joints after a rip split the model into two parts.
- Draft could lose a small wedge on some faces.

**Closer to Onshape**
- Sheet metal: the Flange dialog's Partial flange options, flat patterns at closed and butt corners.
- Assemblies: groups can hold assembly patterns.
- Every dark mode screen checked against Onshape; the Appearances panel's empty tabs; the Versions and history menu.
- Drawings: stacked callouts, hole table Y direction, decals and crops in shaded views, section hatching that keeps clear of dimensions, ISO dimensions too narrow for their arrows, broken-out section editing, sketches in assembly views.
- The macOS icon now fills its tile like other Mac apps.
