"""Today's Windows update helper, as crates/update writes it: python extract_helper.py <lib.rs> <out.ps1>"""
import re
import sys

text = open(sys.argv[1], encoding="utf-8").read()
open(sys.argv[2], "w", encoding="utf-8").write(re.search(r'const WINDOWS_HELPER: &str = r#"(.*?)"#;', text, re.S).group(1))
