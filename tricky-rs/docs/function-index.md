# Function index

[`function-index.csv`](function-index.csv) lists every function in `SLUS_203.26` (SSX Tricky, PS2 NTSC-U, md5
`162580fd65611e72a90deed640a70aef`): 7,425 functions, 2,035,304 bytes of code. It holds addresses, sizes and our
names only, nothing of the game's code.

| Column | Meaning |
|---|---|
| `address` | Start address in `SLUS_203.26` |
| `size` | Bytes, as Ghidra 12.1.4 sees the function |
| `name` | Our name: Ghidra's (RTTI names from `ghidra/symbols.txt`), replaced for `FUN_…` by the name our notes or code give that address (`` `Name` 0xaddr ``) |
| `system` | From the name when it has one. Otherwise a guess, marked `?`, from [`function-systems.csv`](function-systems.csv) (call graph, shared globals, neighbours, address ranges the notes prove; see `tools/function_systems.py`), or from the named functions on both sides. Empty = not known yet |
| `status` | `not started`, `ported` (the address or the name is written in `tricky-rs/src`, outside lines tagged `STANDIN`), or `checked` (matches the original on the comparison harness, row F4; set by hand until F4 exists) |

Regenerate after porting something:

```
python tools/function_index.py status
```

Redo the system guesses (needs your own decompiler export from `ghidra/scripts/ExportDecomp.java`), then run `status`:

```
python tools/function_systems.py <decomp-dir>
```

The first build used the function list exported by `ghidra/scripts/ExportDecomp.java`
(`python tools/function_index.py build <functions.csv>`); that list stays on your own PC, the index does not need it.

"Ported" is generous for now: code that names a function in a comment counts, ported in full or not. Row F9
puts the address on every recreated function and F8 tags the stand-ins, which makes this column exact.

## By system (2026-10-10)

| System | Functions | of them guessed | Bytes | Ported |
|---|---|---|---|---|
| frontend | 1,510 | 870 | 448,044 | 0 |
| lib (linked libraries: PS2 SDK, kernel stubs, libc, EA sound; split by F1b) | 1,372 | 1,372 | 310,736 | 0 |
| audio | 666 | 627 | 171,480 | 17 |
| world | 563 | 328 | 166,464 | 28 |
| render | 494 | 300 | 139,184 | 0 |
| boarder | 453 | 384 | 210,772 | 29 |
| runtime (C++ runtime, type info, thunks) | 423 | 0 | 34,392 | 0 |
| race | 358 | 292 | 101,680 | 1 |
| game (app, loaders, race handlers) | 290 | 173 | 71,624 | 0 |
| camera | 235 | 219 | 73,916 | 2 |
| debugmenu | 177 | 71 | 31,800 | 0 |
| ai | 157 | 118 | 41,952 | 6 |
| (not known yet) | 150 | 0 | 30,788 | 1 |
| save | 145 | 41 | 26,532 | 1 |
| comm (debug link to the dev kit) | 102 | 78 | 12,160 | 0 |
| animation | 74 | 59 | 37,656 | 4 |
| hud | 65 | 57 | 82,932 | 6 |
| files | 60 | 32 | 10,348 | 0 |
| scoring | 46 | 43 | 14,048 | 3 |
| wipeout | 20 | 9 | 12,400 | 11 |
| common (helpers called from four or more systems) | 19 | 19 | 2,300 | 0 |
| video | 19 | 1 | 1,816 | 0 |
| input | 14 | 0 | 932 | 0 |
| tutorial | 11 | 1 | 1,168 | 0 |
| other (named, no system yet) | 2 | 0 | 180 | 0 |
| **total** | **7,425** | **5,094** | **2,035,304** | **109** |

Where the libraries start: game code calls game code up to 0x2bb000. 0x2bb000–0x2bec00 and 0x2c2044 to the end
of `.text` (0x30F960) never call back into the game, so they are linked libraries (`lib`). 0x2bec00–0x2c2044 is EA's
Pathfinder music (`PF_*`) with its glue to the game (`audio`). The kernel syscall stubs are at 0x3063e0–0x307830,
the C++ runtime at about 0x2f6000–0x2fa800, libc's printf and maths after that.

Guesses are guesses. In a spot check of 14 (2026-10-10, before the address ranges and `lib` were added) 12 looked
right; the two wrong ones were a `Score_*` function placed in boarder (its caller) and a libc function placed in
frontend. Both kinds are fixed now, but a small helper called mostly from one system still lands in that system
even when it belongs to another. Fix one by naming the function (`ghidra/symbols.txt`).
