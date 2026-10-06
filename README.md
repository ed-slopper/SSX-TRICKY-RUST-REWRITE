# SSX Tricky (PS2, USA) reverse engineering

Target: `SLUS_203.26` from `SSX Tricky (USA).iso` (ELF md5 `162580fd65611e72a90deed640a70aef`, PCSX2 CRC `8E7CFF62`).

## Try it

**Option A - patched ISO (any emulator, no settings needed)**
Boot `build/SSX Tricky (USA) [debug_menu+unlock_all] v2.iso` in PCSX2. (v2 is built from the untouched 2001 disc image in the old modding folder. The first build used the ISO in this folder, which is a 2023 rebuild with a re-packed Garibaldi level and broken textures.)

**Option B - PCSX2 cheat file on your normal ISO**
- PCSX2 1.7 / 2.x: copy `mods/pcsx2/SLUS-20326_8E7CFF62.pnach` into PCSX2's `cheats` folder, then right-click the game > Properties > Cheats, tick "Enable Cheats" and the ones you want.
- PCSX2 1.6 or older: copy `mods/pcsx2/8E7CFF62.pnach` into `cheats` and tick System > Enable Cheats (all four mods turn on; delete the ones you don't want from the file).

What to look for:
1. **Unlock everything** - every character, board, outfit and course is selectable on a fresh save.
2. **BX Debug Menu** - start any race and press START. Instead of the normal pause screen you should get the developers' text menu: Return To Race / Instant Replay / Restart Race / Render Options / Game Options / Sound Options / Exit the Game. "Return To Race" unpauses.

Status: the patches are verified byte-for-byte against the disassembly, but they have **not been run in an emulator yet**. The unlock flag is the same variable the game's own button cheat flips, so it is low risk. The debug menu is the experimental one. If START freezes or shows nothing, use the cheat file instead of the ISO and untick it.

## Layout

| Path | What |
|---|---|
| `mods/mods.json` | Mod definitions: address, original word, new word, asm |
| `mods/pcsx2/` | Generated `.pnach` files |
| `tools/ssxpatch.py` | Patcher: `list`, `build`, `pnach`, `elf`, `verify` (Python 3, no packages) |
| `tools/rtti_scan.py` | Recovers class names / vtables / hierarchy from the ELF |
| `build/` | Patched ISOs |
| `decomp/` | Ghidra decompiler output for all 7,425 functions, one file per 64 KB of code, plus `functions.csv` |
| `ghidra/project/` | Ghidra 12.1.4 project (open `ssxtricky.gpr`), already analyzed and named |
| `ghidra/scripts/` | `ApplySsxSymbols.java`, `ExportDecomp.java` |
| `ghidra/symbols.txt`, `ghidra/rtti.json` | about 2,900 recovered names |
| `notes/findings.md` | What has been worked out so far |
| `notes/class-hierarchy.txt` | 394 classes with vtable addresses |
| `tools/downloads/` | Ghidra 12.1.4 and the PS2 (Emotion Engine) extension zips |

## Making another build

```
python tools/ssxpatch.py list
python tools/ssxpatch.py build debug_menu --iso "D:\Games\games\ps2\ssx modding\new iso and new extracted\SSX Tricky (USA).iso"
python tools/ssxpatch.py build unlock_all max_stats mallora_board
python tools/ssxpatch.py verify "build/<name>.iso"
```
`build` copies the original ISO and overwrites the changed words inside the ELF in place. It checks the original words first and refuses if they differ. To add a mod, add an entry to `mods/mods.json`.

## Opening the Ghidra project on Windows

1. Unzip `tools/downloads/ghidra_12.1.4_PUBLIC_20260921.zip` (needs JDK 21).
2. Run `ghidraRun.bat`, then File > Install Extensions > `+` > pick `tools/downloads/ghidra_12.1.4_PUBLIC_..._ghidra-emotionengine-reloaded.zip`, restart.
3. File > Open Project > `ghidra/project/ssxtricky.gpr`.

To rebuild from scratch: import `SLUS_203.26` (language `r5900:LE:32:default`), auto-analyze, then run `ApplySsxSymbols.java` with `ghidra/symbols.txt`.
