# Characters

Source files on the disc (`DATA/CHAR/`, clean copies in `chars/original/`):

| Archive | Contents |
|---|---|
| `MDLPS2.BIG` | `<name>_body.mpf`, `<name>_head.mpf` for 12 riders (+ `zz_mmm`), `board.mpf` |
| `TEXPS2.BIG` | `<name><N>_suit.ssh`, `_boot.ssh`, `_bord.ssh` (outfits 1-6, boards 1-12), `<name>_head.ssh`, `_helm.ssh` |
| `ANM.BIG` | 446 `.afl` animation files: `bxanim`, `cmanim`, `ps2anim`, per-rider uber tricks (`fr*Uber`, `bx*Uber`, `ex*Uber`), win/lose/select clips |
| `BRDPS2.BIG` | `board.mpf` |

Extracted with `tools/ssxlevel`:

```
ssxlevel unbig   chars/original/MDLPS2.BIG <dir>
ssxlevel model   chars/mac/model.json <dir>/.../mac_body.mpf <dir>/.../mac_head.mpf     # LOD 0 by default
ssxlevel ssh2png <dir>/.../mac1_suit.ssh chars/mac/
```

`chars/<name>/model.json`: `bones` (name, parent, local position in cm, Euler angles), `materials`, `meshes`
(flat triangle lists: `pos`, `nrm`, `uv`, per-vertex `weights` as [bone, percent, ...], per-face `mat`).

Things worked out while getting them on screen:
- Model space is centimetres, hips at the origin, +X = the rider's left, -Y = facing, +Z = up.
- Bone rotation is `Rz(-z) * Ry(-y) * Rx(-x)` of the stored Euler angles (the stored angles are the inverse rotation).
- 23 bones: hips, lower_back, upper_back, neck, head, l/r clav, bicep, forearm, hand, l/r thigh, calf, foot, goggles, strap, eye_l/r.
- Material ids come in threes per texture (plain, gloss, environment): 0-2 helm, 3-5 boot, 6-8 head, 9-11 suit.
  "helm" holds gloves and hat, "boot" holds goggles and boots.
- The `.afl` animation format is decoded: see `animation-format.md` and `tools/afl/`.

## The game's model loader (F1c, SLUS_203.26)

How the game builds a rider from `.mpf` files (names in `ghidra/symbols.txt`):
- `BdrLoad_RequestRider` 0x201790 queues the files by flag bits: 0x100 body + head `.mpf`, 0x200 `board.mpf`,
  0x1000 head/helm `.ssh`, 0x800 the outfit textures. `BdrLoad_BuildSkin` 0x201138 then builds the skin.
- `.mpf` header as the game reads it (`MpfFile_Open` 0x208468): +4 s16 part count, +6 s16 offset of the part
  directory, +8 u32 offset of the part data. Directory entries are 0x50 bytes: name at +0 (compared on 15
  chars), +0x10 data offset, +0x14 data size, +0x40/+0x42/+0x44/+0x46 s16 counts (materials, bone matrices,
  texture-name sets, vertices to scale), +0x4a s8 slot.
- A part is picked by name (`MpfFile_AddPart` 0x208488): the game asks for named parts such as `Body3000`,
  `Goggles3000`, `EddieHair3000` from per-rider tables at 0x341318 (body, 3 per outfit) and 0x341588 (head, 5).
- The skin lives in the boarder at +0x79c: up to 4 LODs of 0x5900 bytes (`RiderSkin_AllocLods`, array at
  +0x3f18, count +0x3f1c); LOD 3 is the board (+0x3f20). Each LOD has a list of 0x48-byte parts
  (`SkinLod_AllocParts`); part +0xc is the copied directory entry, +0x10 the copied data.
- After loading: texture names are resolved per part (5 names per 0x20-byte entry), materials get the
  gloss/env strength as `float * 127` and the flag 0x8000 when their texture is `boot`
  (`SkinPart_ResolveMaterials` 0x205670), and vertices are scaled once (`SkinPart_Scale`).
- Each frame `RiderSkin_SkinMatrices` 0x207c30 multiplies each part's bind matrices (0x40 bytes each) by the
  bone matrices at +0x3d80.

### Part data layout (E4a, from `SkinPart_Load` 0x205080)

Offsets in a part's 0x50-byte directory entry are from the start of the part's data (proven; field meanings
partly inferred):
- data +0: texture-name sets, 0x20 bytes each, count entry+0x44 (`SkinPart_ResolveTextureNames`).
- entry+0x18 → bone records, 0x54 bytes each, count entry+0x42 (scaled at +0x18..+0x20 by `SkinPart_Scale`;
  parent at +0x10, index at +0x12, read by `SkinPart_SetChildMatrices`).
- entry+0x1c → vec4 list, count entry+0x46 (scaled with the bones).
- entry+0x20 → materials, 0x14 bytes each, count entry+0x40: +0xc count and +0x10 pointer to (pointer, count)
  pairs, each pointing at `count` 0xc-byte records (start pointer, end pointer, ...): the mesh packet ranges.
- entry+0x28 → part+0x20 (not yet known).
- entry+0x2c → 8-byte records (second word a pointer), count entry+0x3e.
- entry+0x30 → 0xc-byte records (second word a pointer), count entry+0x3c (`SkinLod_CollectMaterials` pairs).
