<img src="packaging/icons/noshape-256.png" alt="noshape" width="96">

# noshape

A fast, fully local, parametric feature-based CAD application for macOS, Linux and Windows.
noshape works like Onshape — documents with Part Studios, Assemblies and Drawings, a
history-based feature list, constraint-driven sketches, mates, and a version graph with
branches and merges — but runs entirely on your machine. No account, no server, no network.

Built with Rust, [GPUI](https://gpui.rs) via [GPUI Kit](https://gpui-kit.com), wgpu, and the
OpenCascade (OCCT) B-rep kernel.

## What it does

**Documents.** A `.noshape` file is a local database holding every change as a
microversion. There is no Save command. You get undo/redo, named versions, branches,
merges, read-only views of any point in history, and "compare with current". A start
window lists recent documents and folders with thumbnails.

**Part Studios**
- **Sketches:** lines, arcs, circles, splines, conics, slots, polygons and text.
- **Sketch constraints:** a dual-number Levenberg–Marquardt solver, DOF display,
  inference and snapping.
- **Driving dimensions:** values can be expressions, e.g. `#width / 2 + 1/4 in`.
- **Features:** extrude, revolve, sweep, loft, thicken, fillet, chamfer, shell, draft,
  holes, patterns, mirror, booleans, split, surfaces, curves, direct edits, and more.
- **Sheet metal:** flanges, hems, tabs, bends, joints, reliefs, corner breaks, and a flat
  pattern exported to DXF/SVG.
- **Frames:** trim, gussets, end caps and a cut list.
- **Feature dialogs:** every dialog previews live. Dialogs have a "Final" toggle, and
  some features have drag handles in the viewport (extrude depth, revolve angle).
- **Persistent naming:** downstream references survive upstream edits.
- **History list:** rollback bar, suppression, reordering, folders, and copy/paste of
  features (also between documents).
- **Variables and configurations:** variables, Variable Studios, configurations, and
  Derived parts across Part Studios.
- **Analysis:** measure and mass properties.
- **View:** section view, display modes and named views.
- **Custom features:** defined in script in Feature Studios.

**Assemblies.**
- **Mates:** all Onshape mate types on mate connectors, plus relations, groups, and
  limits.
- **Moving parts:** a triad manipulator, snap mode, and solver-driven drag.
- **Patterns and replicate.**
- **Explode:** exploded views and named positions.
- **Checks and lists:** interference checks and a BOM.

**Drawings.**
- **Views:** generated with hidden-line removal: base, projected, section (including
  jogged and aligned), detail, auxiliary, broken and cropped views.
- **Annotations:** associative dimensions with tolerances, GD&T, notes with fields, a
  title block, and BOM, hole and general tables.
- **Export:** PDF, SVG and DXF.

**Import and export.** Import STEP, IGES and BREP; export STEP, IGES, STL, OBJ and BREP.

## Installing

Download the file for your system from the [releases](https://github.com/yekta/noshape/releases):

| System | Download | What the install does |
|---|---|---|
| macOS (Apple Silicon) | `noshape-<version>-macos-arm64.dmg` | Drag noshape to Applications. On first launch it offers to install the `noshape` command (a link in `/usr/local/bin`, else `~/.local/bin`) and to connect your coding agents; later: Help › Install command line tool… / Connect to coding agents… (also in the noshape menu). |
| Windows | `noshape-<version>-windows-x86_64-setup.exe` | Installs to Program Files with a Start menu entry, puts `noshape` on your PATH (removed on uninstall), and the last page offers to connect your coding agents. `noshape-<version>-windows-x86_64.zip` is the portable version (nothing installed). |
| Linux (x86_64) | `noshape-<version>-linux-x86_64.tar.gz` | Unpack and run `./install.sh` (for you: `~/.local/bin` and your menu; `--system` for everyone, `--agents` to connect your coding agents without asking). Debian and Ubuntu: `sudo apt install ./noshape-<version>-linux-x86_64.deb`. |
| Servers (no GUI) | `noshape-cli-<version>-<system>` | The headless command line tool and MCP server alone, `noshape-cli`. |

**macOS:** the app is signed ad hoc, not notarized, so macOS blocks it when it comes from a download ("cannot be
opened" / "developer cannot be verified"). After copying it to Applications, clear the download flag once:
`xattr -dr com.apple.quarantine /Applications/noshape.app`.

## Using noshape from Claude Code and other agents

The app is also a command line tool and an MCP server that can do everything the app does (see
[docs/AGENT_API.md](docs/AGENT_API.md)): `noshape <command>` from a terminal, `noshape mcp` for agents. Installing it
connects it to the coding agents it finds, with your OK: Claude Code (`claude mcp add --scope user`), the Codex CLI
(`codex mcp add`, or `~/.codex/config.toml`) and Claude Desktop (`claude_desktop_config.json`, backed up first).
From a terminal:

```sh
noshape agents status       # which agents are installed and connected
noshape agents connect      # connect every one found (--agents claude_code,codex,claude_desktop to choose)
noshape agents disconnect
```

To do it by hand, with `noshape` on your PATH (any platform):

```sh
claude mcp add noshape -- noshape mcp          # add --scope user to use it in every project
codex mcp add noshape -- noshape mcp
```

Without the install (the Windows zip, the Linux tarball unpacked anywhere), give the full path instead:
`claude mcp add noshape -- ~/noshape-<version>-linux-x86_64/noshape mcp`, or
`claude mcp add noshape -- C:\noshape-<version>-windows-x86_64\noshape.exe mcp`. Claude Desktop and other MCP clients
take the same in their JSON configuration: `{"mcpServers": {"noshape": {"command": "/path/to/noshape", "args": ["mcp"]}}}`.
When the app has a document open, agents edit it live in the window.

## Building

You need Rust (stable), a C++17 compiler and CMake. Build OCCT once, then build the app:

```sh
scripts/build-occt.sh ~/.cache/noshape/occt        # Windows: scripts\build-occt.ps1
export OCCT_ROOT=~/.cache/noshape/occt
cargo run --release -p noshape                    # start window
cargo run --release -p noshape -- --demo /tmp/demo.noshape   # sample document
```

See [docs/USER_GUIDE.md](docs/USER_GUIDE.md) for how to use it, [docs/BUILDING.md](docs/BUILDING.md) for build details, and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the crates fit together.

## Repository layout

| Crate | Purpose |
|---|---|
| `crates/units` | Quantities, units and the expression language |
| `crates/sketch` | 2D constraint solver and sketch tools |
| `crates/kernel` | OCCT bridge (cxx): modeling, booleans, tessellation, HLR, I/O |
| `crates/model` | Documents, features, regeneration, naming, assemblies, drawings |
| `crates/store` | Local versioned storage (SQLite, content-addressed blobs) |
| `crates/render` | wgpu renderer (offscreen, picking, reverse-Z, MSAA) |
| `crates/app` | The GPUI application |
