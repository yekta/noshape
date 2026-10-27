#!/usr/bin/env python3
"""The menu coverage check of crates/ops/tests/coverage.rs (every_menu_item_is_listed), in a
second, without building: every menu item of the app's sources has a line in
crates/ops/src/coverage.rs MENUS, and MENUS lists no item the UI no longer has.

    tools/menu-coverage.py [commit]      (default: the working tree)

Exit 1 with the lines to add / remove. Run by the shared pre-push hook when a push to main changes
crates/app/src, crates/app-core/src, crates/drawing-ui/src, crates/assembly-ui/src, crates/part-studio-ui/src or the coverage map. Keep the parsing in step with `menu_labels` in the Rust test.
"""
import os, re, subprocess, sys

# The Part Studio's sources (crates/app/src/part_studio/ before the noshape-part-studio-ui split).
PART_STUDIO_DIRS = ("crates/part-studio-ui/src/part_studio/", "crates/app/src/part_studio/")

ROOT = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True).stdout.strip()


def files_at(commit):
    """{path: text} of crates/app/src, crates/app-core/src, crates/drawing-ui/src, crates/assembly-ui/src, crates/part-studio-ui/src (top level and part_studio/) and the coverage map."""
    wanted = []
    if commit:
        names = subprocess.run(["git", "-C", ROOT, "ls-tree", "-r", "--name-only", commit, "crates/app/src", "crates/app-core/src", "crates/drawing-ui/src", "crates/assembly-ui/src", "crates/part-studio-ui/src"], capture_output=True, text=True).stdout.split()
        wanted = [n for n in names if n.count("/") == 3 or n.startswith(PART_STUDIO_DIRS)]
        wanted.append("crates/ops/src/coverage.rs")
        p = subprocess.Popen(["git", "-C", ROOT, "cat-file", "--batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        out, _ = p.communicate("".join(f"{commit}:{n}\n" for n in wanted).encode())
        texts, i = {}, 0
        for n in wanted:
            head_end = out.index(b"\n", i)
            size = int(out[i:head_end].split()[2])
            texts[n] = out[head_end + 1 : head_end + 1 + size].decode("utf-8", "replace")
            i = head_end + 1 + size + 1
        return texts
    texts = {}
    for crate in ("app", "app-core", "drawing-ui", "assembly-ui", "part-studio-ui"):
        top = os.path.join(ROOT, "crates", crate, "src")
        for name in os.listdir(top):
            if name.endswith(".rs"):
                texts[f"crates/{crate}/src/{name}"] = open(os.path.join(top, name), encoding="utf-8").read()
    src = os.path.join(ROOT, "crates/part-studio-ui/src")
    for name in os.listdir(os.path.join(src, "part_studio")):
        texts[f"crates/part-studio-ui/src/part_studio/{name}"] = open(os.path.join(src, "part_studio", name), encoding="utf-8").read()
    texts["crates/ops/src/coverage.rs"] = open(os.path.join(ROOT, "crates/ops/src/coverage.rs"), encoding="utf-8").read()
    return texts


def area_of(path):
    if path.startswith(PART_STUDIO_DIRS):
        return "partStudio"
    name = os.path.basename(path)
    if name.startswith("assembly_"):
        return "assembly"
    if name.startswith("drawing_"):
        return "drawing"
    if name.startswith("variable_") or name == "history_panel.rs":
        return "document"
    if name == "feature_dialog.rs":
        return "partStudio"
    if name == "camera_menu.rs":
        return "view"
    return None


def labels(texts):
    out = set()
    for path, text in texts.items():
        area = area_of(path)
        if not area or not path.endswith(".rs"):
            continue
        for pat in ('item("', 'OsMenuItem::new("'):
            start = 0
            while True:
                at = text.find(pat, start)
                if at < 0:
                    break
                start = at + len(pat)
                if pat.startswith("item"):
                    before = text[at - 1] if at > 0 else " "
                    if (before.isalnum() or before == "_") and not text[:at].endswith("icon_"):
                        continue
                end = start
                while end < len(text) and not (text[end] == '"' and (end == start or text[end - 1] != "\\")):
                    end += 1
                out.add(f"{area}:{text[start:end]}")
    return out


def main():
    commit = sys.argv[1] if len(sys.argv) > 1 else None
    texts = files_at(commit)
    have = labels(texts)
    menus = texts["crates/ops/src/coverage.rs"]
    body = menus[menus.index("pub const MENUS"):]
    # MENUS holds Rust string literals: `\\t` there is the source's `\t`.
    listed = {m.replace("\\\\", "\\").replace('\\"', '"') for m in re.findall(r'^\s*\("((?:[^"\\]|\\.)+)",', body, re.M)}
    missing = sorted(have - listed)
    stale = sorted(listed - have)
    if not missing and not stale:
        return 0
    print("menu coverage (crates/ops/src/coverage.rs MENUS) is out of step with the app's menus:", file=sys.stderr)
    if missing:
        print("  add these lines to MENUS (map each to the operation that does it, e.g. Op(\"feature_add\")):", file=sys.stderr)
        for m in missing:
            print(f'    ("{m}", Op("…")),', file=sys.stderr)
    if stale:
        print("  remove these lines (the UI no longer has them):", file=sys.stderr)
        for m in stale:
            print(f'    ("{m}", …),', file=sys.stderr)
    print("  (main goes red on every_menu_item_is_listed otherwise; api owns the map, ask if unsure)", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
