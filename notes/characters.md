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
