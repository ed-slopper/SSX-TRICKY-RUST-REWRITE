#!/usr/bin/env python3
"""Function index of SLUS_203.26: tricky-rs/docs/function-index.csv.

One row per function in the executable: address, size, our name, system, status. Addresses and our names
only: nothing of the game's code goes in it.

    python tools/function_index.py build <functions.csv>   first build, from ghidra/scripts/ExportDecomp.java's list
    python tools/function_index.py status                  refresh names and statuses in the existing index

Names: the Ghidra name, replaced by any name our notes and code give the address (`Name` 0xaddr).
System: from the name; for unnamed functions, the system of the named functions on both sides when they
agree, marked with a trailing '?'. Status: 'host' for the libraries Rust and Bevy stand in for (HOST), 'ported' when the address or the name is written in tricky-rs/src (lines tagged STANDIN do not
count), 'checked'
is set by hand (or by the comparison harness, row F4) and is kept, otherwise 'not started'.
"""
import csv
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INDEX = ROOT / "tricky-rs" / "docs" / "function-index.csv"
GUESSES = ROOT / "tricky-rs" / "docs" / "function-systems.csv"
SRC = ROOT / "tricky-rs" / "src"
SYMBOLS = ROOT / "ghidra" / "symbols.txt"
# Systems Rust and Bevy stand in for: their functions are not ported (status 'host'). Row F1b.
HOST = {"sdk", "kernel", "libc", "runtime", "lib-eamem"}
NOTES = [ROOT / "tricky-rs" / "docs", ROOT / "notes"]

# First match wins. Matched against the whole name (class::method or Free_Function).
SYSTEMS = [
    (r"^(thunk|__|_GLOBAL_|entry$)|::__tf$|::__(dt|vt)", "runtime"),
    (r"MemCard|^cMC|MCOverlay|^Profile_|Save", "save"),
    (r"^cPad|^Pad_|^Input_|Cheat", "input"),
    (r"Camera|^cBxCam|^Cam_|\bcml", "camera"),
    (r"^(cMenu|cMenuManager|cDebugMenu|c\w*MenuItem|cGameOptionsMenu|cRenderOptionsMenu|cResolutionMenu|cBezierOptionsMenu|cSoundVolumeMenu)\b", "debugmenu"),
    (r"^cFE|Overlay|^FE_|^Frontend|^cTitle", "frontend"),
    (r"^HUD_|^Font_|^SpriteSet_|^cHud|^cFont", "hud"),
    (r"Audio|^Music|^SND|^PF_|^Voice|^BoardIn_|Sound|^cBXAudio|^WorldEmitter", "audio"),
    (r"BigFile|^Big_|^RefPack|^File_|^cFile|^cAsync|^Load_|^AIP_Load", "files"),
    (r"^Score_|^TrickBook|^cScore", "scoring"),
    (r"^(AI|cAI|AIComputer|AIP|RelTable|Rider_OnKnocked)", "ai"),
    (r"^(Ragdoll|Wipeout|GetUp|CollBody|cRagdoll)", "wipeout"),
    (r"^(Anim|cAnim|cMeshAnim|AnimCurve|AnimClip)", "animation"),
    (r"^(Circuit|cCircuit|Race_|cRace|cCourse|Course|cEndRace|cPreRace|PreRaceSel|c2P)", "race"),
    (r"^(TriggerScript|cWorld|World_|Instance_|Crowd|RigidBody|c\w+Node\b|c\w+Node(_|::))", "world"),
    (r"^(cBoarder|Boarder_|cWorldBoarder|Air_|AirPredict|Landing_|Jump_|Spin|Prewind|Rail|Takeoff|GateAnticipate|FinishState|ResetState|SurfaceTable|c\w*Motion|c\w*ControlState|c\w*Control\b|Fx_|Emitter_|PBurst_)", "boarder"),
    (r"^(cPS2|cGraphics|Render|Gfx|Tex|cTex|Mesh|cMesh|Light|cLight|VU|Draw)", "render"),
    (r"^(CComm|Net_|cNet)", "comm"),
    (r"^(cApp|cGame|App_|Game_|c\w*Load\b|c\w*Load::|cFirstLoad|cStartScreen|c\w*Handler\b|c\w*Handler::)", "game"),
    (r"^(cVideo|Video_)", "video"),
    (r"^(cTrickInfo|cTrickTutorial|cLesson|c\w*Lesson)", "tutorial"),
    (r"^(cParticle|cSnowFall|cFogVolume|cSkelSequence)", "render"),
    (r"^(cInput)\b", "input"),
    (r"^(cPath|cEventPath)\b", "ai"),
    (r"^(cBankManager)\b", "audio"),
    (r"^(CDeci2|C8bit)", "comm"),
]
SYS_RES = [(re.compile(p), s) for p, s in SYSTEMS]

ADDR_IN_SRC = re.compile(r"0x([0-9a-fA-F]{5,6})\b")
# a function name of ours: Some_Name, cClass::Method or cClassName, not a plain word before an address
LOOKS_NAMED = re.compile(r"^[A-Za-z0-9]+_[A-Za-z0-9_]+$|::|^c[A-Z]")
WORD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*")
NAMED = re.compile(r"`?([A-Za-z_][A-Za-z0-9_:]*[A-Za-z0-9_])`?\s*\(?0x([0-9a-fA-F]{5,6})\b")


def system_of(name):
    if not name or name.startswith("FUN_"):
        return ""
    for rx, sysname in SYS_RES:
        if rx.search(name):
            return sysname
    return "other"


def read_text(p):
    """A file's text without the lines tagged STANDIN: naming what a stand-in replaces is not porting it."""
    text = p.read_text(encoding="utf-8", errors="replace")
    return "\n".join(l for l in text.splitlines() if "STANDIN" not in l)


def text_files():
    for p in sorted(SRC.rglob("*.rs")):
        yield p
    for d in NOTES:
        for p in sorted(d.rglob("*.md")):
            yield p


def harvest(starts):
    """Our names for function starts, and which starts our code mentions (by address)."""
    names, ported = {}, set()
    for p in text_files():
        text = read_text(p)
        for m in NAMED.finditer(text):
            name, a = m.group(1), int(m.group(2), 16)
            if a in starts and LOOKS_NAMED.search(name) and not name.startswith("FUN_"):
                # first name seen wins; the notes and code are consistent where they overlap
                names.setdefault(a, name)
        if p.suffix == ".rs":
            for m in ADDR_IN_SRC.finditer(text):
                a = int(m.group(1), 16)
                if a in starts:
                    ported.add(a)
    return names, ported


def fill_guesses(rows):
    """Unnamed functions between two named ones of the same system get that system with '?'."""
    named = [i for i, r in enumerate(rows) if r["system"] and not r["system"].endswith("?")]
    for lo, hi in zip(named, named[1:]):
        s = rows[lo]["system"]
        if s in ("runtime", "other") or rows[hi]["system"] != s:
            continue
        for i in range(lo + 1, hi):
            if not rows[i]["system"]:
                rows[i]["system"] = s + "?"


def names_in_src():
    """Every function-like name our Rust source mentions (in code or comments)."""
    found = set()
    for p in sorted(SRC.rglob("*.rs")):
        found.update(WORD.findall(read_text(p)))
    return found


def guessed_systems():
    """Systems from the call graph and neighbours, made by tools/function_systems.py (row F1a)."""
    if not GUESSES.exists():
        return {}
    with open(GUESSES, newline="", encoding="utf-8") as f:
        return {int(r["address"], 16): r["system"] for r in csv.DictReader(f)}


def symbol_names():
    """Function names in ghidra/symbols.txt, the names agents gave in Ghidra:
    `G <addr> <name>` a global function, `F <addr> <class> <method>` a class's."""
    names = {}
    for line in SYMBOLS.read_text(encoding="utf-8").splitlines():
        p = line.split()
        if len(p) == 3 and p[0] == "G":
            names[int(p[1], 16)] = p[2]
        elif len(p) == 4 and p[0] == "F":
            names[int(p[1], 16)] = f"{p[2]}::{p[3]}"
    return names


def refresh(rows):
    starts = {int(r["address"], 16) for r in rows}
    names, ported = harvest(starts)
    symbols = symbol_names()
    for r in rows:
        a = int(r["address"], 16)
        if r["name"].startswith("FUN_"):
            r["name"] = symbols.get(a) or names.get(a) or r["name"]
    in_src = names_in_src()
    guessed = guessed_systems()
    for r in rows:
        a = int(r["address"], 16)
        r["system"] = system_of(r["name"])
        if r["system"] in ("", "other") and a in guessed:
            r["system"] = guessed[a] + "?"
        named_in_src = LOOKS_NAMED.search(r["name"]) and r["name"] in in_src
        if r["status"] != "checked":
            if a in ported or named_in_src:
                r["status"] = "ported"
            elif r["system"].rstrip("?") in HOST:
                r["status"] = "host"
            else:
                r["status"] = "not started"
    fill_guesses(rows)
    return rows


def write(rows):
    INDEX.parent.mkdir(parents=True, exist_ok=True)
    with open(INDEX, "w", newline="\n", encoding="utf-8") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["address", "size", "name", "system", "status"])
        for r in rows:
            w.writerow([r["address"], r["size"], r["name"], r["system"], r["status"]])
    total = len(rows)
    by = {}
    for r in rows:
        by[r["status"]] = by.get(r["status"], 0) + 1
    named = sum(1 for r in rows if not r["name"].startswith("FUN_"))
    print(f"{INDEX.relative_to(ROOT)}: {total} functions, {named} named, "
          + ", ".join(f"{k} {v}" for k, v in sorted(by.items())))


def build(export_csv):
    rows = []
    with open(export_csv, newline="", encoding="utf-8") as f:
        for r in csv.DictReader(f):
            a = int(r["address"], 16)
            name = r["name"].strip() or f"FUN_{a:08x}"
            rows.append({"address": f"0x{a:06x}", "size": r["size"], "name": name, "system": "",
                         "status": "not started"})
    rows.sort(key=lambda r: int(r["address"], 16))
    write(refresh(rows))


def status():
    with open(INDEX, newline="", encoding="utf-8") as f:
        rows = list(csv.DictReader(f))
    write(refresh(rows))


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "build":
        build(sys.argv[2])
    elif len(sys.argv) == 2 and sys.argv[1] == "status":
        status()
    else:
        sys.exit(__doc__)
