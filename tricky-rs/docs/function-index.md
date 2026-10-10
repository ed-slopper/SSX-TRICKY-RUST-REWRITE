# Function index

[`function-index.csv`](function-index.csv) lists every function in `SLUS_203.26` (SSX Tricky, PS2 NTSC-U, md5
`162580fd65611e72a90deed640a70aef`): 7,425 functions, 2,035,304 bytes of code. It holds addresses, sizes and our
names only, nothing of the game's code.

| Column | Meaning |
|---|---|
| `address` | Start address in `SLUS_203.26` |
| `size` | Bytes, as Ghidra 12.1.4 sees the function |
| `name` | Our name: Ghidra's, replaced for `FUN_…` by the name in `ghidra/symbols.txt`, else the name our notes or code give that address (`` `Name` 0xaddr ``) |
| `system` | From the name when it has one. Otherwise a guess, marked `?`, from [`function-systems.csv`](function-systems.csv) (call graph, shared globals, neighbours, address ranges the notes prove; see `tools/function_systems.py`), or from the named functions on both sides. Empty = not known yet |
| `status` | `not started`, `host` (a library Rust and Bevy stand in for: not ported, see below), `ported` (the address or the name is written in `tricky-rs/src`, outside lines tagged `STANDIN`), or `checked` (matches the original on the comparison harness, row F4; set by hand until F4 exists) |

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

| System | Functions | of them guessed | Bytes | Ported | Host |
|---|---|---|---|---|---|
| frontend | 1,510 | 870 | 448,044 | 0 | 0 |
| audio | 668 | 627 | 171,576 | 17 | 0 |
| world | 563 | 328 | 166,464 | 28 | 0 |
| runtime (gcc C++ runtime, type info, thunks) | 514 | 91 | 47,872 | 0 | 514 |
| render | 494 | 300 | 139,184 | 0 | 0 |
| boarder | 453 | 384 | 210,772 | 29 | 0 |
| lib-snd (EA sound library) | 408 | 408 | 97,436 | 0 | 0 |
| race | 358 | 292 | 101,680 | 1 | 0 |
| sdk (Sony libraries) | 342 | 342 | 81,680 | 0 | 342 |
| game (app, loaders, race handlers) | 290 | 173 | 71,624 | 0 | 0 |
| camera | 235 | 219 | 73,916 | 2 | 0 |
| lib-ea (EA middleware, not identified yet) | 204 | 204 | 41,032 | 0 | 0 |
| debugmenu | 177 | 71 | 31,800 | 0 | 0 |
| ai | 157 | 118 | 41,952 | 6 | 0 |
| libc (newlib) | 154 | 154 | 61,700 | 0 | 154 |
| (not known yet) | 150 | 0 | 30,788 | 1 | 0 |
| save | 145 | 41 | 26,532 | 1 | 0 |
| comm (debug link to the dev kit) | 102 | 78 | 12,160 | 0 | 0 |
| kernel (EE syscall stubs) | 86 | 86 | 3,644 | 0 | 86 |
| lib-eamem (EA memory manager) | 85 | 85 | 11,668 | 0 | 85 |
| animation | 74 | 59 | 37,656 | 4 | 0 |
| hud | 65 | 57 | 82,932 | 6 | 0 |
| files | 60 | 32 | 10,348 | 0 | 0 |
| scoring | 46 | 43 | 14,048 | 3 | 0 |
| wipeout | 20 | 9 | 12,400 | 11 | 0 |
| common (helpers called from four or more systems) | 19 | 19 | 2,300 | 0 | 0 |
| video | 19 | 1 | 1,816 | 0 | 0 |
| input | 14 | 0 | 932 | 0 | 0 |
| tutorial | 11 | 1 | 1,168 | 0 | 0 |
| other (named, no system yet) | 2 | 0 | 180 | 0 | 0 |
| **total** | **7,425** | **5,092** | **2,035,304** | **109** | **1,181** |

## The libraries

Game code calls game code up to 0x2bb000. From 0x2c2044 to the end of `.text` (0x30F960) nothing calls back into the
game, so it is linked libraries; 0x2bec00–0x2c2044 is EA's Pathfinder music (`PF_*`) with its glue to the game, and
counts as `audio`. The bands below were placed by who calls them, plus the functions named in `ghidra/symbols.txt`
that were read by hand (`memcpy`, `memset`, `sprintf`, `strcpy`, `strlen`, `EAMem_Alloc`, `EAMem_Free`, `SND_Lock`, ...).
They live in `LIB_BANDS` in `tools/function_systems.py`.

| Span | System | Evidence | Port? |
|---|---|---|---|
| 0x2bb000–0x2bec00 | lib-snd | the effect player: called only by the game's audio code, plays by priority (0x2bba80, 93 callers) | port |
| 0x2c2044–0x2cc800 | lib-ea | called by the game's renderer, front end, HUD and file code; 0x2c5160 reads a packed table for 130 callers; uses the kernel's threads and semaphores | port (to be identified, row F1d) |
| 0x2cc800–0x2cf800 | lib-eamem | `EAMem_Alloc(name, size, align)` 0x2ccf70, `EAMem_Free` 0x2ccfc0, its own memset (callers fill new blocks with 0xdeadc0de) and memcpy | host |
| 0x2cf800–0x2e4c00 | lib-snd | `SND_*` (0x2d0908 lock, 0x2d5288, 0x2d5410); 0x2d5800 on is called only from the SND code | port |
| 0x2e4c00–0x2f2800 | sdk | called by `cPS2Device`, `cPS2GraphicsMan`, `cPS2VideoPlayer`, `cApplication` and by the memory-card code (0x23d000–0x23f000 → 0x2f1000) | host |
| 0x2f2800–0x2f6000 | libc | `fmodf`, start-up helpers called from `main` and the runtime | host |
| 0x2f6000–0x2fa800 | runtime | gcc's exceptions and type info (`__rtti_si`, `__class_type_info`, ...) | host |
| 0x2fa800–0x3063e0 | libc | `memcpy`, `memset`, `sprintf` over `libc_vfprintf` ("bug in vfprintf: bad base"), `strcpy`, `strlen`, strtod ("Infinity") | host |
| 0x3063e0–0x307830 | kernel | syscall stubs (`CreateThread`, `WaitSema`, `SetGsCrt`, ...) | host |
| 0x307830–0x30F960 | sdk | SIF and IOP (`rom0:UDNL` reboot), called by the sdk band above and the dev-kit link | host |

"Host" means Rust, its standard library and Bevy do that job, so the function is not ported; what the game does
with it (the sizes it allocates, the order it loads things) is still ported where it changes behaviour. The game's
own type-info getters (`cClass::__tf`) are `runtime` and host too. EA's libraries are ported because they decide
behaviour: which sounds play, how music moves, how tables are read.

Guesses are guesses. In a spot check of 14 (2026-10-10, before the address ranges and `lib` were added) 12 looked
right; the two wrong ones were a `Score_*` function placed in boarder (its caller) and a libc function placed in
frontend. Both kinds are fixed now, but a small helper called mostly from one system still lands in that system
even when it belongs to another. Fix one by naming the function (`ghidra/symbols.txt`).
