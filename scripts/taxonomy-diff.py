#!/usr/bin/env python3
"""Structural changes in right_taxonomy.json, with label churn suppressed.

Run from the repo root AFTER `just right-taxonomy`:
    python3 taxdiff.py
Compares the regenerated working-tree file against the committed one.
"""
import json, subprocess, sys

def committed(path):
    r = subprocess.run(["git", "show", f"HEAD:{path}"], capture_output=True, text=True)
    if r.returncode:
        sys.exit(f"cannot read HEAD:{path} - {r.stderr.strip()}")
    return json.loads(r.stdout)

PATH = "backend/src/glpi/right_taxonomy.json"
old = committed(PATH) if len(sys.argv) < 3 else json.load(open(sys.argv[1]))
new = json.load(open(PATH if len(sys.argv) < 3 else sys.argv[2]))

# Keyed on (right, interface): nine rights exist under both interfaces, and
# keying on the name alone double-reports every one of them.
k = lambda e: (e["right"], e["interface"])
o = {k(e): e for e in old}
n = {k(e): e for e in new}
where = lambda e: f"{e['tab']}/{e['group']}"

hits = 0
for key in sorted(set(o) - set(n)):
    print(f"REMOVED  {key[0]:24} [{key[1]}]  was {where(o[key])}"); hits += 1
for key in sorted(set(n) - set(o)):
    print(f"ADDED    {key[0]:24} [{key[1]}]  now {where(n[key])}"); hits += 1
for key in sorted(set(o) & set(n)):
    a, b = o[key], n[key]
    if where(a) != where(b):
        print(f"MOVED    {key[0]:24} [{key[1]}]  {where(a)} -> {where(b)}"); hits += 1

labels = sum(1 for key in set(o) & set(n)
             if o[key]["label"] != n[key]["label"] and where(o[key]) == where(n[key]))
print(f"\n{hits} structural change(s); {labels} label-only change(s) suppressed.")
