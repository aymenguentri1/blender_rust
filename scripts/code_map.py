#!/usr/bin/env python3
"""Cheap map of src/: no args = one line per file (lines + main items); with a file = its items with line numbers."""
import re, sys, glob, os
ITEM = re.compile(r"^\s*(pub(\([a-z]+\))?\s+)?(fn|struct|enum|trait|impl|mod|const|static|type)\s+([A-Za-z_][\w<>:, ]*)")
root = os.path.join(os.path.dirname(__file__), "..", "src")
def items(path):
    out = []
    for n, l in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
        m = ITEM.match(l)
        if m and not l.lstrip().startswith("//"): out.append((n, l.strip().rstrip("{").strip()[:110]))
    return out
if len(sys.argv) > 1:
    for n, t in items(sys.argv[1]): print(f"{n}: {t}")
else:
    for f in sorted(glob.glob(root + "/**/*.rs", recursive=True)):
        it = items(f); names = [re.sub(r"^(pub(\([a-z]+\))?\s+)?", "", t).split("(")[0][:28] for _, t in it if t.startswith("pub")][:6]
        print(f"{os.path.relpath(f, root)} ({sum(1 for _ in open(f, errors='replace'))} lines): {', '.join(names)}")
