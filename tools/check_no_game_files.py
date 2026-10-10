#!/usr/bin/env python3
"""Fail if anything from the game is committed (AGENTS.md §13). Run by CI on every push.

    python tools/check_no_game_files.py

Checks every file git tracks: no game file types, no executables or disc images (by name or by their first
bytes), no decompiler output or Ghidra project, nothing over 2 MB. Prints what it found and exits 1.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# File types from the disc and from the decompiler. .map is SSX's level name table.
GAME_EXT = {
    ".iso", ".bin", ".img", ".elf", ".irx", ".big", ".pbd", ".ssh", ".fsh", ".mpf", ".afl", ".bnk", ".mus",
    ".ssf", ".aip", ".sop", ".ltg", ".map", ".adl", ".loc", ".sfn", ".cml", ".pss", ".ipu", ".dat", ".inf",
    ".gpr", ".rep", ".gzf", ".gdt", ".c", ".h",
}
NAMES = re.compile(r"(^|/)(SLUS_\d{3}\.\d{2}|SYSTEM\.CNF|decomp/|ghidra/project/|traces/)", re.I)
MAGIC = {b"\x7fELF": "an ELF executable", b"BIGF": "a BIG archive", b"\xc0\xfb": "a BIG archive",
         b"CD001": "a disc image"}
DECOMPILED = re.compile(rb"\bundefined[148] |^// ===== [0-9a-f]{8}  ", re.M)
MAX_BYTES = 2 << 20


def main():
    files = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout
    bad = []
    for name in filter(None, files.decode("utf-8").split("\0")):
        path = ROOT / name
        if not path.is_file():
            continue
        if path.suffix.lower() in GAME_EXT:
            bad.append(f"{name}: a game or decompiler file type ({path.suffix})")
            continue
        if NAMES.search(name):
            bad.append(f"{name}: a path only game files or the Ghidra project use")
            continue
        size = path.stat().st_size
        if size > MAX_BYTES:
            bad.append(f"{name}: {size:,} bytes, too big for something we wrote")
            continue
        head = path.read_bytes()[:0x8010]
        for magic, what in MAGIC.items():
            if head.startswith(magic) or head[0x8001:0x8006] == magic:
                bad.append(f"{name}: starts like {what}")
        if DECOMPILED.search(path.read_bytes()):
            bad.append(f"{name}: looks like decompiler output")
    if bad:
        print("Game files or decompiler output are committed (AGENTS.md §13: never):")
        print("\n".join("  " + b for b in bad))
        return 1
    print(f"ok: no game files among {len(files.split(chr(0).encode())) - 1} tracked files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
