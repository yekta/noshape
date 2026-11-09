# Noshape help

Noshape works the way Onshape does, on your computer and offline: every menu, dialog and shortcut is where Onshape has it. This guide covers what is particular to a local app and where to start.

## Documents

- A document is one **.noshape** file. It holds its tabs, its whole history, its versions and its workspaces.
- New documents go in your documents folder (**Documents/noshape**). The documents page's footer shows the folder; click it to open it in your file manager.
- **Create ▾ › Document…** makes a document with Part Studio 1 and Assembly 1. **Create ▾ › Folder…** makes a folder on disk.
- Right-click a document for Open, Rename, Copy workspace…, Details, Versions and history, Labels…, Move to…, Send to trash, and **Show in Explorer** (Windows), **Show in Finder** (Mac) or **Show in folder** (Linux).
- Edits are saved as you make them. There is no Save command.

### Document notes

The left rail's **Document notes** holds notes for the whole document, written in Markdown. Everyone who opens the file sees them; they are kept with the document's history.

### Performance

The left rail's **Performance** panel shows how long each tab takes to regenerate and to draw; its stopwatch shows every feature's regeneration time in the feature list, to find the feature that makes a Part Studio slow.

## Tabs

- The tab bar's **+** adds Part Studios, Assemblies, Drawings, Variable Studios, Feature Studios, Material Libraries and folders, or imports a file.
- Right-click a tab to rename, duplicate, copy, export or delete it. Drag a tab to reorder it, or onto a folder to file it.
- **Ctrl+Space** (hold) switches between recent tabs; **Alt+T** opens the Tabs panel.

The Tabs panel (**Alt+T**) lists every tab and folder: search them, sort them, filter them by kind, and right-click one for its menu. Select several with Ctrl+click to file or move them together.

## Modeling

- **Part Studio:** sketches and features build parts; the feature list on the left is the order they regenerate in. Roll back with the rollback bar, edit a feature by double-clicking it.
- **Assembly:** insert parts and subassemblies (**i**), then mate them. Instances can come from this document or from a version of another document.
- **Drawing:** Create Drawing… from a Part Studio or an Assembly, then add views, dimensions, notes and tables. Export as PDF, DXF or DWG from the tab's menu.
- **Variable Studio:** variables (#name) any Part Studio or Assembly can use; tick Insert into all Part Studios and Assemblies, or insert it where needed.
- **Feature Studio:** your own features in FeatureScript. Commit (**Ctrl+S**, **⌘S** on a Mac) to use them; Help › FeatureScript documentation opens the standard library's source.

### Variable Studios

- A Variable Studio is a table of variables (#name) shared between tabs. **Insert into all Part Studios and Assemblies** puts it in every one of them; otherwise insert it where it's needed with the Variable table's **Insert Variable Studio**.
- Values can be expressions (`#width * 2`); a variable inserted from another studio can be used in this one.
- Renaming a variable renames it in every tab that uses the studio. A Variable Studio can be configured (the Configuration panel on the right), and inserted in a chosen configuration.

### Variable tables

A Part Studio's or Assembly's **Variable table** (right strip) lists its own variables and those of the Variable Studios it uses, inserted ones first. A variable defined twice takes the last value; the earlier row says it is overwritten.

### Configurations

The **Configuration panel** (right strip) adds inputs (lists, checkboxes, variables and text) to a Part Studio, Assembly or Variable Studio. Right-click a feature's value to configure it; each row of the table is one configuration. Visibility conditions show an input only when others have given values.

### Custom tables

**Custom tables** (right strip) are tables a Feature Studio defines for a Part Studio: a parts list, a measurement summary, or anything else its FeatureScript computes. Pick one from the panel's table selector.

### Inspection data

The Part Studio's **Inspection** panel (right strip) lists the tolerances given to sketch dimensions and feature parameters, and the annotations and dimensions added with its tools, for any part of the Part Studio.

### Custom features

Features from a Feature Studio, in this document or another one's version, are added to the toolbar with **Add custom features**. Commit the Feature Studio for its features to update.

### Custom materials

A **Material Library** tab holds your own materials (density and properties); add it in Preferences › Material libraries to use its materials in every document.

### Exploded views

In an Assembly, **Exploded views** (right strip) move instances away from their mated positions step by step, to show how the parts fit together. The assembly itself is unchanged.

## Versions and workspaces

- **Versions and history** (the clock on the left rail) lists versions, workspaces and every change.
- **Create version…** (**Ctrl+Alt+V**, **⌘⌥V** on a Mac) keeps the current state. Right-click a version to open it, branch a workspace from it, compare, or restore it.
- Right-click another workspace to merge it into the current one.

### Compare

Right-click a version or workspace › **Compare** shows the differences with the current workspace: changed features in the list (> only in the first, < only in the second), the geometry blended with the slider, and each feature's parameters side by side. Assemblies and drawings can't be compared.

### Merging

Right-click another workspace › **Merge into current workspace…** brings its changes in: per tab, keep this workspace's, merge both, or replace with the other's.

### Workspace protection

**Workspace protection** limits a workspace to merges: geometry, drawing and metadata changes and new tabs come only from merging another workspace in, while versions, branching and release management stay allowed.

## Import and export

- Import STEP, IGES, Parasolid, STL, DXF, DWG, images and other files from **+ › Import…** or the documents page's **Create ▾ › Import files…**.
- Export parts, Part Studios and Assemblies from a tab's or a part's **Export…**; drawings from the drawing tab's menu.

### Printing

In a Part Studio or an Assembly, **☰ › Print…** opens **Print setup** beside the feature list and draws a dashed frame over the graphics area: what is inside the frame is printed. Pick the **paper** (Letter, Legal, Tabloid, A0–A10, ANSI C–E) and the **orientation**; the frame takes the paper's shape. You can still rotate and zoom the view while the panel is open. **Download image** saves the framed view as a PNG at about 300 dpi, named after the tab; **Print** opens it on a page of that paper for your system's print dialog. The paper and orientation you chose last come back next time. In a drawing, **☰ › Print…** prints the sheets.

## Preferences

On the documents page, **your name ▾ › Preferences…** sets the units of new documents, the language, the theme, mouse controls, keyboard shortcuts, the shortcut toolbars, material libraries and export rules.

### Default units

Preferences › Units sets the units new documents start with. A document's own units are in its ☰ › Workspace units…; changing them shows every value in the new unit.

### Onshape Labs

Preferences › Onshape Labs turns on features that are still experimental, such as FeatureScript Co-complete (completions as you type in a Feature Studio).

## Keyboard shortcuts

Press **?** (Shift+/) or use Help › Keyboard shortcuts for the list of the tab you are in. Preferences › Keyboard shortcuts changes them.

## Command line and coding agents

- **Help › Install command line tool…** adds the `noshape` command: everything the app does, from a terminal or a script.
- **Help › Connect to coding agents…** lets an agent (Claude, Codex and others) work on your documents through the same commands.

## When something looks wrong

- **Help › System check** shows your graphics, memory and disk, and whether Noshape runs on your graphics card or in software.
- **Help › About** shows the version.
