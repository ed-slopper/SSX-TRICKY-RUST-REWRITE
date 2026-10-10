# Function index

[`function-index.csv`](function-index.csv) lists every function in `SLUS_203.26` (SSX Tricky, PS2 NTSC-U, md5
`162580fd65611e72a90deed640a70aef`): 7,425 functions, 2,035,304 bytes of code. It holds addresses, sizes and our
names only, nothing of the game's code.

| Column | Meaning |
|---|---|
| `address` | Start address in `SLUS_203.26` |
| `size` | Bytes, as Ghidra 12.1.4 sees the function |
| `name` | Our name: Ghidra's (RTTI names from `ghidra/symbols.txt`), replaced for `FUN_…` by the name our notes or code give that address (`` `Name` 0xaddr ``) |
| `system` | Worked out from the name. Unnamed functions that sit between two named ones of the same system get that system with a `?`. Empty = not known yet |
| `status` | `not started`, `ported` (the address or the name is written in `tricky-rs/src`, outside lines tagged `STANDIN`), or `checked` (matches the original on the comparison harness, row F4; set by hand until F4 exists) |

Regenerate after porting something:

```
python tools/function_index.py status
```

The first build used the function list exported by `ghidra/scripts/ExportDecomp.java`
(`python tools/function_index.py build <functions.csv>`); that list stays on your own PC, the index does not need it.

"Ported" is generous for now: code that names a function in a comment counts, ported in full or not. Row F9
puts the address on every recreated function and F8 tags the stand-ins, which makes this column exact.

## By system (2026-10-10)

| System | Functions | Bytes | Ported |
|---|---|---|---|
| (not known yet) | 3,959 | 1,006,496 | 6 |
| frontend | 969 | 354,216 | 0 |
| runtime (C++ runtime, type info, thunks) | 423 | 34,392 | 0 |
| world | 353 | 79,960 | 28 |
| boarder | 253 | 144,396 | 28 |
| render | 253 | 69,048 | 0 |
| audio | 187 | 61,400 | 14 |
| game (app, loaders, race handlers) | 175 | 61,964 | 0 |
| debugmenu | 158 | 27,544 | 0 |
| other (named, no system yet) | 141 | 9,244 | 2 |
| save | 133 | 25,380 | 1 |
| race | 80 | 36,300 | 1 |
| ai | 59 | 18,116 | 4 |
| camera | 53 | 19,416 | 1 |
| scoring | 38 | 10,036 | 3 |
| files | 34 | 5,076 | 0 |
| comm (debug link to the dev kit) | 31 | 3,252 | 0 |
| animation | 30 | 10,256 | 4 |
| video | 25 | 2,664 | 0 |
| hud | 20 | 39,524 | 6 |
| wipeout | 19 | 13,408 | 11 |
| tutorial | 18 | 2,284 | 0 |
| input | 14 | 932 | 0 |
| **total** | **7,425** | **2,035,304** | **109** |
