#!/usr/bin/env python3
"""Systems for the functions the index can't place by name (board row F1a).

    python tools/function_systems.py <decomp-dir>

Reads the decompiler export made by ghidra/scripts/ExportDecomp.java (kept on your own PC, never in the repo)
only to see who calls whom and which globals each function touches, and writes
tricky-rs/docs/function-systems.csv: address, system, how. Addresses and system names only.

How a system is guessed, repeated until nothing changes:
  graph      the systems of its callees (weight 2), callers (1.5) and of the globals it shares with functions
             of one system only (1); the best must score at least 2 and twice the runner-up
  common     called from four or more systems and calls nothing: a shared helper (maths, strings, lists)
  neighbour  the nearest placed functions before and after it agree (functions of one source file sit together)
  loose      after the above: the best score wins if it is at least 1 and ahead of the runner-up
  range      inside an address range whose system the notes prove (RANGES below)
  lib        inside the linked libraries (LIBS below); 'lib' is never spread to game code by the graph

Then `python tools/function_index.py status` puts them in the index, marked with '?'.
"""
import bisect
import collections
import csv
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INDEX = ROOT / "tricky-rs" / "docs" / "function-index.csv"
OUT = ROOT / "tricky-rs" / "docs" / "function-systems.csv"

HDR = re.compile(r"^// ===== ([0-9a-f]{8})  (.*) =====$", re.M)
WORD = re.compile(r"\b([A-Za-z_][A-Za-z0-9_:~]*)\b")
DAT = re.compile(r"\b(?:DAT|PTR_DAT|PTR|s)_[A-Za-z0-9_]*?([0-9a-f]{8})\b")
WEAK = {"", "runtime", "other"}

# Address ranges whose system our notes already prove (tricky-rs/docs/original-rules.md). They win over the graph.
RANGES = [
    (0x155410, 0x157490 + 4, "scoring"),  # Score_*
    (0x16b000, 0x16d400, "race"),         # Circuit_*
    (0x2bec00, 0x2c2040 + 4, "audio"),    # PF_*: EA Pathfinder interactive music, and its glue to the game
]
# Linked libraries: nothing in these spans calls game code (the PS2 SDK and kernel stubs, libc, EA's sound
# library). Row F1b splits them further and decides what is host and what is ported.
LIBS = [(0x2bb000, 0x2bec00), (0x2c2040 + 4, 0x30F960)]


def fixed_system(a):
    for lo, hi, s in RANGES:
        if lo <= a < hi:
            return s, "range"
    for lo, hi in LIBS:
        if lo <= a < hi:
            return "lib", "lib"
    return None


def read_export(d):
    funcs = {}
    for f in sorted(Path(d).glob("*.c")):
        t = f.read_text(encoding="utf-8", errors="replace")
        ms = list(HDR.finditer(t))
        for i, m in enumerate(ms):
            end = ms[i + 1].start() if i + 1 < len(ms) else len(t)
            body = t[m.end():end]
            funcs[int(m.group(1), 16)] = (m.group(2), body.split("{", 1)[1] if "{" in body else body)
    return funcs


def graph(funcs):
    """Calls and references (callbacks, tables) both ways, and the globals each function touches."""
    by_name = {}
    for a, (n, _) in funcs.items():
        by_name.setdefault(n, a)
    out, glob = {}, {}
    for a, (_, body) in funcs.items():
        refs = set()
        for w in WORD.findall(body):
            if w.startswith("FUN_"):
                try:
                    x = int(w[4:], 16)
                except ValueError:
                    continue
                if x in funcs:
                    refs.add(x)
            elif w in by_name and ("::" in w or "_" in w) and not w.startswith(("DAT_", "PTR_", "s_")):
                refs.add(by_name[w])
        refs.discard(a)
        out[a] = refs
        glob[a] = {int(m.group(1), 16) for m in DAT.finditer(body)}
    inn = collections.defaultdict(set)
    for a, rs in out.items():
        for r in rs:
            inn[r].add(a)
    return out, inn, glob


def scores(a, sysof, out, inn, glob, gsys):
    sc = collections.Counter()
    for c in out.get(a, ()):
        if sysof.get(c) not in (None, "lib", "common"):
            sc[sysof[c]] += 2
    for c in inn.get(a, ()):
        if sysof.get(c) not in (None, "lib", "common"):
            sc[sysof[c]] += 1.5
    for x in glob.get(a, ()):
        gs = gsys.get(x)
        if gs and len(gs) == 1:
            sc[next(iter(gs))] += 1
    return sc


def global_systems(sysof, glob):
    g = collections.defaultdict(set)
    for a, gs in glob.items():
        s = sysof.get(a)
        if s and s not in ("common", "lib"):
            for x in gs:
                g[x].add(s)
    return g


def main(d):
    with open(INDEX, newline="", encoding="utf-8") as f:
        rows = {int(r["address"], 16): r for r in csv.DictReader(f)}
    funcs = read_export(d)
    out, inn, glob = graph(funcs)
    sysof = {a: r["system"] for a, r in rows.items()
             if r["system"] not in WEAK and not r["system"].endswith("?")}
    how = {}
    order = sorted(rows)
    for a in order:
        f = fixed_system(a)
        if f and a not in sysof and rows[a]["system"] != "runtime":
            sysof[a], how[a] = f

    def graph_pass(loose):
        n = 0
        while True:
            g = global_systems(sysof, glob)
            new = {}
            for a in order:
                if a in sysof or rows[a]["system"] in ("runtime",):
                    continue
                sc = scores(a, sysof, out, inn, glob, g)
                if not sc:
                    continue
                callers = {sysof[c] for c in inn.get(a, ()) if c in sysof}
                if len(callers) >= 4 and not out.get(a):
                    new[a] = ("common", "common")
                    continue
                (s1, v1), *rest = sc.most_common(2)
                v2 = rest[0][1] if rest else 0
                if (not loose and v1 >= 2 and v1 >= 2 * v2) or (loose and v1 >= 1 and v1 > v2):
                    new[a] = (s1, "loose" if loose else "graph")
            if not new:
                return n
            for a, (s, h) in new.items():
                sysof[a] = s
                how[a] = h
            n += len(new)

    def neighbour_pass():
        n = 0
        placed = [a for a in order if sysof.get(a) not in (None, "common", "lib")]
        for a in order:
            if a in sysof or rows[a]["system"] == "runtime":
                continue
            i = bisect.bisect_left(placed, a)
            if 0 < i < len(placed) and sysof[placed[i - 1]] == sysof[placed[i]]:
                sysof[a] = sysof[placed[i - 1]]
                how[a] = "neighbour"
                n += 1
        return n

    while graph_pass(False) + neighbour_pass():
        pass
    graph_pass(True)
    neighbour_pass()

    with open(OUT, "w", newline="\n", encoding="utf-8") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["address", "system", "how"])
        for a in order:
            if a in how:
                w.writerow([f"0x{a:06x}", sysof[a], how[a]])
    left = sum(1 for a in order if a not in sysof and rows[a]["system"] in ("", "other"))
    c = collections.Counter(how.values())
    print(f"{OUT.relative_to(ROOT)}: {len(how)} placed ({', '.join(f'{k} {v}' for k, v in c.most_common())}), "
          f"{left} still without a system")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
