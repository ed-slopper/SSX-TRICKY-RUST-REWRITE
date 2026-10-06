# The .afl animation format (SSX Tricky PS2)

Worked out from the files in `ANM.BIG` plus the game's code; `tools/afl/afl.py` is a complete reader,
`aflexport.py` writes a clip set to JSON, `aflviz.py` draws stick-figure contact sheets.
1,519 clips across 446 files decode; 1,507 of them have their original names.

## File layout (little endian)

```
u8  76, u8 17, u16 headerCount
u32 pointerTableOffset
u32 dataOffset
headerCount x 36-byte header
pointer table: u32 offsets into the data block, one per curve
data block: the curves
```

Header (36 bytes): `u32 hash, u32 firstPointer, u8 type, u8 groupCount, 11 x u16, u32`.

- type 255 starts a clip. `hash` is the clip's name hash, the first u16 is the frame count, `groupCount` says how
  many group headers follow, and the other u16s are event frames (65535 = none).
- type 0 is the body: 60 curves starting at pointer `firstPointer`.
- type 1 is the board: 6 curves.
- cut-scene files also have types 4-15 with 3 to 23 curves (faces, props, cameras); not needed for riding.

## The 66 curves of a riding clip

| Curves | Meaning |
|---|---|
| 0-2 | hips position, cm |
| 3-59 | Euler angles (x, y, z) for 19 bones in skeleton order: hips, lower_back, upper_back, neck, head, l_clav, l_bicep, l_forearm, l_hand, r_clav, r_bicep, r_forearm, r_hand, l_thigh, l_calf, l_foot, r_thigh, r_calf, r_foot |
| 60-62 | board position, cm |
| 63-65 | board Euler angles |

Angles use the same convention as the model's bind pose: rotation = `Rz(-z) * Ry(-y) * Rx(-x)`.
Space: board at the origin lying along Y, +Y = direction of travel, +X = toe side, +Z up; the board's
underside is about 4.8 cm below the origin. Air and trick clips are authored about 55 cm above it.
Playback is 30 frames a second (my assumption; it looks right, not confirmed in code).

## Curve encoding

Each curve starts with a u16: low 4 bits = kind, high 12 bits = count. A "float24" is the top three bytes of a
float; rebuild it as bytes `[0x80, b0, b1, b2]`.

| Kind | Size | Value at frame t |
|---|---|---|
| 0-3 | 2 + 3*(kind+1) | polynomial of that degree in t, kind+1 float24 coefficients, highest power first |
| 4 | 2 + 4*count | raw floats, one per frame |
| 6 | 8 + count | float24 offset, float24 scale, then one byte per frame: offset + scale*byte |
| 7 | 8 + 2*count | same with u16 per frame |
| 5 | varies | `count` sub-curves back to back; each sub-curve's own count is how many frames it covers, and t restarts at 0 in each |

For kinds 0-3 the count is the number of frames the piece covers.

## Names

Clip hashes are a PJW-style string hash of the clip name with one twist, found in the decompilation at
0x240E80: the overflow nibble is folded back with a shift of 23, not 24.

```
h = 0
for c in name:  h = h*16 + c;  g = h & 0xF0000000;  if g: h ^= (g >> 23) ^ g
```

The names themselves are in the executable as a table (`bxRL_CROUCHCYCLE`, `bxT_NOSEGRAB`, ...); `animnames.json`
maps every clip in every file to its name. Prefixes: `bx`, `fr`, `ex` = the three board types' sets;
`A_` air, `AA_` air adjust, `B_` bails, `G_` gate, `GU_` get-ups, `J_` jumps, `L_` landings, `R_`/`RL_`/`RC_` riding,
`RR_` rail riding?, `RS_` rail slides, `RT_` taunts, `T_`/`TW_` tricks and tweaks, `UT_` uber tricks.

## What is not done

- Only `bxanim.afl` is exported and used. `fr`/`ex` sets and the per-rider uber trick files decode the same way.
- The event fields in the clip header are not interpreted.
- The game's own rules for which clip plays when are not reconstructed; tricky-rs uses its own.
