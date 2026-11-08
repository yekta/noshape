# noshape help

noshape works the way Onshape does, on your computer and offline: every menu, dialog and shortcut is where Onshape has it. This guide covers what is particular to a local app and where to start.

## Documents

- A document is one **.noshape** file. It holds its tabs, its whole history, its versions and its workspaces.
- New documents go in your documents folder (**Documents/noshape**). The documents page's footer shows the folder; click it to open it in your file manager.
- **Create ▾ › Document…** makes a document with Part Studio 1 and Assembly 1. **Create ▾ › Folder…** makes a folder on disk.
- Right-click a document for Open, Rename, Copy workspace…, Details, Versions and history, Labels…, Move to…, Send to trash, and **Show in Explorer** (Windows), **Show in Finder** (Mac) or **Show in folder** (Linux).
- Edits are saved as you make them. There is no Save command.

## Tabs

- The tab bar's **+** adds Part Studios, Assemblies, Drawings, Variable Studios, Feature Studios, Material Libraries and folders, or imports a file.
- Right-click a tab to rename, duplicate, copy, export or delete it. Drag a tab to reorder it, or onto a folder to file it.
- **Ctrl+Space** (hold) switches between recent tabs; **Alt+T** opens the Tabs panel.

## Modeling

- **Part Studio:** sketches and features build parts; the feature list on the left is the order they regenerate in. Roll back with the rollback bar, edit a feature by double-clicking it.
- **Assembly:** insert parts and subassemblies (**i**), then mate them. Instances can come from this document or from a version of another document.
- **Drawing:** Create Drawing… from a Part Studio or an Assembly, then add views, dimensions, notes and tables. Export as PDF, DXF or DWG from the tab's menu.
- **Variable Studio:** variables (#name) any Part Studio or Assembly can use; tick Insert into all Part Studios and Assemblies, or insert it where needed.
- **Feature Studio:** your own features in FeatureScript. Commit (**Ctrl+S**, **⌘S** on a Mac) to use them; Help › FeatureScript documentation opens the standard library's source.

## Versions and workspaces

- **Versions and history** (the clock on the left rail) lists versions, workspaces and every change.
- **Create version…** (**Ctrl+Alt+V**, **⌘⌥V** on a Mac) keeps the current state. Right-click a version to open it, branch a workspace from it, compare, or restore it.
- Right-click another workspace to merge it into the current one.

## Import and export

- Import STEP, IGES, Parasolid, STL, DXF, DWG, images and other files from **+ › Import…** or the documents page's **Create ▾ › Import files…**.
- Export parts, Part Studios and Assemblies from a tab's or a part's **Export…**; drawings from the drawing tab's menu.

## Preferences

On the documents page, **your name ▾ › Preferences…** sets the units of new documents, the language, the theme, mouse controls, keyboard shortcuts, the shortcut toolbars, material libraries and export rules.

## Keyboard shortcuts

Press **?** (Shift+/) or use Help › Keyboard shortcuts for the list of the tab you are in. Preferences › Keyboard shortcuts changes them.

## Command line and coding agents

- **Help › Install command line tool…** adds the `noshape` command: everything the app does, from a terminal or a script.
- **Help › Connect to coding agents…** lets an agent (Claude, Codex and others) work on your documents through the same commands.

## When something looks wrong

- **Help › System check** shows your graphics, memory and disk, and whether noshape runs on your graphics card or in software.
- **Help › About** shows the version.
