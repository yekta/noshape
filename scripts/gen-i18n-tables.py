#!/usr/bin/env python3
"""Generate the UI string tables (crates/app/assets/i18n/<code>.tsv) from
Onshape's locale files, as the live capture saves them
(<locales>/<locale>/{messages,tooltips,server}.json, English text → text).

usage: scripts/gen-i18n-tables.py <locales dir>

Only whole UI strings are kept: no markup, nothing the language leaves as
it is. `__name__` placeholders stay (formatted texts look them up by key).
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
        with open(os.path.join(OUT, f"{code}.tsv"), "w", encoding="utf-8") as f:
            for english in sorted(table):
                f.write(f"{escape(english)}\t{escape(table[english])}\n")
        print(code, len(table))


if __name__ == "__main__":
    main()
