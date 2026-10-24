#!/usr/bin/env python3
"""Generate the UI string tables (crates/app/assets/i18n/<code>.tsv) from
Onshape's locale files, as the live capture saves them
(<locales>/<locale>/{messages,tooltips,server}.json, English text → text).

usage: scripts/gen-i18n-tables.py <locales dir>

Only whole UI strings are kept: no markup, nothing the language leaves as
it is. `__name__` placeholders stay (formatted texts look them up by key).

Also names-<code>.tsv: the bases of new features' default names, which
Onshape takes from other sources than the UI text (live "ui2 (10-23
17:00)"): Part Studio features from the `server` namespace (Fillet →
"Verrundung", where messages has "Verrunden"); assembly features and mates
from the server-localised assembly feature specs
(<locale>/assembly-default-names.json: a mate is named after its mate type
option, Fastened → "Fest").

And specs-<code>.tsv: the feature dialogs' strings (parameter labels, enum
options, group names) from the localised feature specs, decoded from the
websocket frames (<locale>/{partstudio,assembly}-featurespecs-strings.json
"flat"). Dialogs look these up first (Draft → "Formschräge" there, where
the UI text has "Entwurf"; a spec string left in English stays English);
strings only the specs have also join the main table.
"""

import json
import os
import re
import sys

# i18n::LANGUAGES codes → the capture's locale folders.
LOCALES = {"zh-TW": "zh_TW", "fr": "fr_FR", "de": "de_DE", "zh-CN": "zh_CN", "ja": "ja_JP", "ko": "ko_KR", "es": "es", "it": "it_IT"}
# Earlier namespaces win when they disagree on a key.
NAMESPACES = ["messages", "tooltips", "server"]
OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "crates", "app", "assets", "i18n")
MARKUP = re.compile(r"<[a-zA-Z/][^>]*>|&[a-z]+;|&#\d+;")


def escape(s):
    return s.replace("\\", "\\\\").replace("\t", "\\t").replace("\n", "\\n").replace("\r", "")


SHORTCUT = re.compile(r"^(.+) \(((?:(?:Shift|Alt|Ctrl|Cmd|shift|alt|ctrl|cmd)[-+])*[^ ()]{1,3})\)$")
# The translation's shortcut may be spelt in the language ("Umschalt-f").
LOCAL_SHORTCUT = re.compile(r"^(.+) \(([^()]{1,16})\)$")


def derive(table):
    """Entries for how the app draws some keys (only where the table has
    no entry of its own):
    - a tool name without its shortcut: "Sketch fillet (Shift-f)" →
      "Sketch fillet" (the app shows the shortcut apart);
    - each line of a multi-line text whose translation has as many lines
      (the mouse-control rows draw a line at a time)."""
    extra = {}
    for english, text in table.items():
        m, t = SHORTCUT.match(english), LOCAL_SHORTCUT.match(text)
        if m and t:
            extra.setdefault(m.group(1), t.group(1))
        lines, local = english.split("\n"), text.split("\n")
        if len(lines) > 1 and len(lines) == len(local):
            for a, b in zip(lines, local):
                if a.strip() and b.strip() and a.strip() != b.strip():
                    extra.setdefault(a.strip(), b.strip())
    for english, text in extra.items():
        table.setdefault(english, text)


def sane(text):
    """Values the spec capture split cleanly (no stray half of a pair)."""
    return text.count("(") == text.count(")") and "\n" not in text


def names(src, folder):
    """English default-name base → the language's."""
    out = {}
    with open(os.path.join(src, folder, "server.json"), encoding="utf-8") as f:
        for english, text in json.load(f).items():
            if isinstance(text, str) and text != english and not MARKUP.search(text) and sane(text):
                out[english] = text
    with open(os.path.join(src, "en_US", "assembly-default-names.json"), encoding="utf-8") as f:
        en = json.load(f)
    with open(os.path.join(src, folder, "assembly-default-names.json"), encoding="utf-8") as f:
        local = json.load(f)
    # Mate types last: a mate's name wins over a same-named feature.
    for group in ["featureTypeName", "mateTypeOptionNames"]:
        for key, english in en[group].items():
            text = local[group].get(key)
            if text and text != english and sane(text):
                out[english] = text
    return out


def specs(src, folder):
    """English → the language's, for every feature dialog string."""
    out = {}
    for kind in ["partstudio", "assembly"]:
        path = os.path.join(src, folder, f"{kind}-featurespecs-strings.json")
        with open(path, encoding="utf-8") as f:
            for english, text in json.load(f).get("flat", {}).items():
                if isinstance(text, str) and english.strip() and not MARKUP.search(english) and sane(text):
                    out.setdefault(english, text)
    return out


def main():
    src = sys.argv[1]
    for code, folder in LOCALES.items():
        table = {}
        for ns in NAMESPACES:
            path = os.path.join(src, folder, f"{ns}.json")
            with open(path, encoding="utf-8") as f:
                for english, text in json.load(f).items():
                    if not isinstance(text, str) or not english.strip() or text == english or english in table:
                        continue
                    if MARKUP.search(english) or MARKUP.search(text):
                        continue
                    table[english] = text
        # The documents page's tree ("Owned by me", "Trash"): server-sent
        # names (locales/<locale>/server-globaltreenodes.json).
        tree = os.path.join(src, folder, "server-globaltreenodes.json")
        if os.path.exists(tree):
            with open(tree, encoding="utf-8") as f:
                for english, text in json.load(f).get("names", {}).items():
                    if isinstance(text, str) and text != english:
                        table.setdefault(english, text)
        spec = specs(src, folder)
        for english, text in spec.items():
            if text != english:
                table.setdefault(english, text)
        derive(table)
        with open(os.path.join(OUT, f"{code}.tsv"), "w", encoding="utf-8") as f:
            for english in sorted(table):
                f.write(f"{escape(english)}\t{escape(table[english])}\n")
        base = names(src, folder)
        with open(os.path.join(OUT, f"names-{code}.tsv"), "w", encoding="utf-8") as f:
            for english in sorted(base):
                f.write(f"{escape(english)}\t{escape(base[english])}\n")
        with open(os.path.join(OUT, f"specs-{code}.tsv"), "w", encoding="utf-8") as f:
            for english in sorted(spec):
                f.write(f"{escape(english)}\t{escape(spec[english])}\n")
        print(code, len(table), "names", len(base), "specs", len(spec))


if __name__ == "__main__":
    main()
