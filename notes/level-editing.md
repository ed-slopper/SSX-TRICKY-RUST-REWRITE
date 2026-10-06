# Level editing - state of play (Oct 2026)

## What a track is
Each track is one archive in `DATA/MODELS/<NAME>.BIG` (EA "C0FB" BIG, RefPack-compressed) holding:

| File | Contents |
|---|---|
| `.pbd` | The level itself: Bezier terrain patches (4x4 control points), object instances, models/meshes, materials, splines (rails), lights, particles |
| `.ssh`, `_L.ssh`, `_sky.ssh` | Textures, lightmaps, skybox textures |
| `_sky.pbd` | Skybox models |
| `.ltg` | Spatial grid used for culling/lighting lookups (regenerated on build) |
| `.map` | Name table for everything in the pbd |
| `.ssf` | Collision models, physics, scripted effects/triggers |
| `.aip`, `.sop` | AI paths, race lines, start positions |
| `.adl` | Audio links |

Clean copies of all 12 archives from the 2001 disc are in `levels/original/`.

## Toolchain that exists today
The tools in the old modding folder are from May 2023. The same project is now much further along:
- **SSX-Library** (GlitcherOG, .NET 10, cross-platform) - reads/writes every file above, extracts a level to a
  folder of JSON + OBJ + PNG and builds it back.
- **SSX Multitool V0.5.2.1** - Windows GUI over that library (`tools/downloads/level-tools/`).
- **IceSaw V0.3.1** - Unity plugin that opens the extracted project as a 3D scene (needs Unity 2021.3).
- **SSX Mod Manager V0.2.1** - installs level/character mods; its first release shipped nine community tracks.
- **OpenSlope / Slopesmith** (swax) - separate browser-based editor that claims to repack for PCSX2. Not evaluated.

## What is verified here
`tools/ssxlevel/` is a small command-line wrapper I built on SSX-Library (source + the three Linux fixes it
needed are in `tools/ssxlevel/src/`). Run with `dotnet tools/ssxlevel/ssxlevel.dll` (needs the .NET 10 runtime):

```
ssxlevel extract levels/original/GARI.BIG levels/gari      # archive -> editable project
ssxlevel build   levels/gari levels/build/GARI.BIG         # project -> archive
python tools/ssxpatch.py build unlock_all --replace DATA/MODELS/GARI.BIG=levels/build/GARI.BIG
```

Round-trip test on Garibaldi (extract -> build -> extract again, compare the two projects):
- terrain patches (3,885), instances (3,393), splines, lights, AI paths, SSF logic, particles: identical
- textures: identical to within 1/255 per channel after accounting for renumbering; lightmaps and skybox identical
- collision meshes: identical; render meshes: same shapes within vertex quantization, re-stripped (32,590 -> 35,084 faces)
- differences: textures and collision models are renumbered, one unused texture is dropped, model names in the
  rebuilt `.map` are shifted by one (cosmetic per the library's own comments), and the `.ltg` grows from 0.7 MB to 12.7 MB

Not yet verified: that the rebuilt archive loads and plays correctly in the game. That is what
`build/SSX Tricky (USA) [roundtrip-garibaldi].iso` is for.

## Project folder format (levels/gari)
`Patches.json` terrain, `Instances.json` placed objects, `Models.json` + `Meshes/*.obj`, `Materials.json` +
`Textures/*.png`, `Splines.json` rails, `Lights.json`, `Lightmaps/*.png`, `Skybox/`, `Collision/*.obj`,
`AIP.json` / `SOP.json` AI and race lines, `SSFLogic.json` triggers/effects/physics, `ConfigTricky.ssx` build options.
Coordinates are X/Y horizontal, Z up. `notes/garibaldi_map.png` is a top-down plot made straight from these files.
