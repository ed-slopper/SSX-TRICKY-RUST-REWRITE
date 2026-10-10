# WORK.md: the board

Goal and rules: [`AGENTS.md`](AGENTS.md). Every address here is `SLUS_203.26` (SSX Tricky, PS2 NTSC-U, md5
`162580fd65611e72a90deed640a70aef`).

Columns:
- **ID**: stable, letter per section (F foundations, E engine, G gameplay, C content, X finishing, L later).
  Split rows get a letter or digit added (`E3` → `E3a`). IDs are never reused.
- **Area**: a person, `unowned`, `any`, or `humans` (a decision, not for agents).
- **Needs**: rows that must be in Done first.
- **Who**: empty = open, or `person/agent (date)`.
- **Notes**: `[repo]` = the work is already noted in the repo (README, notes, code comments); `[est]` = my
  estimate of what a complete 1:1 rewrite needs.

Where things stand (2026-10-10): `tricky-rs` rides all eleven tracks with the game's own riding, air, rail,
trick, scoring, wipe-out, AI, camera, race-line and music rules read from the exe
(`tricky-rs/docs/original-rules.md`). It does **not** yet read the game's files itself: levels come from
SSX-Library's JSON export and characters, sounds and fonts from our Python tools. The front end, the animation
state machine and the renderer are ours. Nothing has been checked against the original by machine.

## P0 Foundations

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| F1c | Name the functions F1a placed only by guess (5,094 marked `?` in `tricky-rs/docs/function-index.csv`), system by system, in Ghidra and in `ghidra/symbols.txt`; start with boarder, race and world, the systems being ported now | any | F3 | | Gap of F1a: naming needs the binary in Ghidra (ghidra-mcp), which was not connected |
| F2b | Names from the notes that F2 could not place, to settle in Ghidra: `Score_TrickBookCheck` 0x157a00 and `TriggerScript_Op0_SpawnController` 0x13c600 fall 0x20 and 0x30 bytes inside 0x1579e0 and 0x13c5d0 (wrong address in the notes, or Ghidra merged two functions); `WipeoutMotion_Update` 0x10c2f0 is code right after `WipeoutMotion_Enter` that our export never made a function (add it to the index too); `cMusicSys` 0x343150 is in `.data` (the object or its vtable: a `D` line); roles of `cAnimObjectNode` 0x198810 and `cAnimDeltaNode` 0x199d10 (the notes only use them as headings); `cTubeEndBoostNode::vf27` 0x1418a8 | any | F3 | | Gap of F2 |
| F2a | Naming rules written down (AGENTS.md §14) and checked against the names already used | any | F2 | | [est] |
| F3 | ghidra-mcp set up against this project on ed-slopper's PC: Ghidra 12.1.4 + EE extension (zips in `tools/downloads/`), program `SLUS_203.26`, `symbols.txt` applied, MCP tools visible to Claude | humans | | | [est] Agents can't install it for you. Steps in AGENTS.md §14 |
| F4 | Design how we check our code against the original: (a) a small R5900 interpreter that runs one original function from the ELF on given inputs and compares with ours, for leaf maths (drag, friction, jump, scoring); (b) PCSX2 traces of the boarder struct per tick on a race, replayed through our code | any | F1 | | [est] Write the design in `tricky-rs/docs/checking.md` |
| F4a | R5900 function runner (MIPS III + the FPU and MMI ops the leaf functions use) in a Rust test crate, reading the player's own ELF | unowned | F4 | | [est] |
| F4b | PCSX2 trace capture of boarder state (pos, vel, state, meter at `boarder+…`) per tick, and a test that replays it | unowned | F4 | | [est] |
| F5 | CI: GitHub Actions building `tricky-rs` (Windows MSVC and Linux), `cargo clippy`, `cargo test`, plus a check that no game file types are committed | any | | | [est] Locally `cargo check` fails with the GNU toolchain (`dlltool.exe` not found): use `stable-x86_64-pc-windows-msvc` or install the MinGW binutils |
| F6 | Port notes per system: split `tricky-rs/docs/original-rules.md` (687 lines) into `tricky-rs/docs/port-notes/<system>.md` (boarder, air, rails, scoring, ai, race, camera, wipeout, world, scripts, audio, hud, frontend, files), with an index | any | | | [repo] |
| F7 | Decide the reference binary: `SLUS_203.26` (NTSC-U) as the only one, or also PAL / other platforms | humans | | | [est] Everything so far is NTSC-U |
| F8 | Tag every stand-in already in `tricky-rs/src` with `STANDIN:` (SSX-Library JSON loader, Python-exported chars/sounds/fonts, our animation choice, our front end, ball physics for props, guessed sound stream picks, "my numbers" medals, `rider.rs` guessed constants, the 1.7 m sphere rider in `wipeout.rs`) | any | | | [repo] The README's "still mine" and "Not done yet" lists |
| F9 | Address on every recreated function: sweep `tricky-rs/src` so each ported function's doc comment has `` `Name` 0xaddr ``, and functions without an original are either tagged `STANDIN:` or plainly our glue | any | F8 | | [repo] Most modules already name their originals in the header |
| F10 | Remove the duplicate tools: `tools/{bnk,mpf,music,sfn}` and `tricky-rs/tools/{…}` are copies of each other; keep one | any | | | [repo] |
| F11 | Split `tricky-rs` into a workspace: `tricky-data` (file formats, no Bevy), `tricky-game` (the game's logic, no Bevy), `tricky-rs` (Bevy host). Makes F4 tests possible without a window | any | | | [est] `main.rs` is 2,466 lines |

## P1 Core engine

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| E1 | Start-up and main loop: `entry` 0x100008, the `gApp` (0x337C58) init, `cGame::Init` 0x17E148, `cGame::Update` 0x181638, game modes and the front end ↔ race switch, pause (`cGame::IsPaused` 0x17F078) | unowned | F1 | | [est] Our Bevy app loop is the stand-in |
| E1a | Fixed 60 Hz tick with per-boarder time scale (`dt = timescale/60`, boarder+0x12c) driving every system, frame-rate independent of the window | any | | | [repo] original-rules.md "Units"; check what `SmoothDt` in `main.rs` does today |
| E1b | Random numbers: the game's `rand` and seeding, used by AI tricks, grabs, announcer, so runs can match the original | unowned | F1 | | [est] |
| E2 | Read the player's disc: ISO 9660 reader in Rust, the `DATA/` tree, so the player points us at their ISO or drive | unowned | F11 | | [est] Replaces copying files out by hand |
| E2a | BIG archives (C0FB and BIGF) in Rust, ported from the game's loader | unowned | E2 | | [repo] Python in `tools/afl/anmbig.py`; findings.md "Ideas for next steps". The game's reader: `lib-big`, `BIG_Identify` 0x2cbc88, `BIG_Lookup` 0x2cbdc0, `BIG_GetFileData` 0x2cc0a8 (F1d) |
| E2b | RefPack decompression in Rust | unowned | E2a | | [repo] Python in `tools/afl/refpack.py`. The game's decoders: `Compress_Decode` 0x2c2de0 → `RefPack_Decode` 0x2c31b0, `Huff_Decode` 0x2c3730, `BTree_Decode` 0x2c3540 (F1d) |
| E3 | Level files read by our code, as the game reads them, replacing the SSX-Library JSON project folder (`level.rs`) | unowned | E2b | | [repo] level-editing.md. Split before claiming: |
| E3a | `.pbd`: Bezier patches, instances, models and meshes, materials, splines, lights | unowned | E3 | | [repo] |
| E3b | `.ssh` textures (incl. `_L` lightmaps, `_sky`) | unowned | E2b | | [repo] Python in `tools/ssh/ssh2png.py`. The game's shape reader: `lib-shape` 0x2c2044–0x2c27c8, `Shape_FindByName` 0x2c2528 (F1d) |
| E3c | `.ssf`: collision models, physics data, trigger scripts | unowned | E3 | | [repo] |
| E3d | `.aip` / `.sop`: AI paths, race lines, start positions (`AIP_LoadPathsFromFile` 0x197da0) | unowned | E3 | | [repo] `course.rs` reads AIP.json today |
| E3e | `.ltg` grid, `.map` names, `.adl` audio links, `_sky.pbd` | unowned | E3 | | [repo] |
| E4 | Character files in our code: `.mpf` models (`tools/mpf/SPEC.md`), `.afl` animations (`notes/animation-format.md`), outfit and board textures, read from `DATA/CHAR/*.BIG` | unowned | E2b | | [repo] Today exported to `chars/` by Python |
| E5 | Config and text files: `DATA/CONFIG/*.INF` (SNOW, BANKS, MUSICMAP, INTROMUS), `.cml` cameras, `TRICKDEF.DAT`, `american.loc` and the other languages, `.SFN` fonts | unowned | E2b | | [repo] "Unresolved: HUD text (american.loc)". Text: `Loc_GetString` 0x2c5160, `WStr_FormatArgs` 0x2c5578 (`lib-text`, F1d) |
| E6 | Loading and memory: what the game loads when (load screens, course switch), only as far as it changes behaviour | unowned | E1 | | [est] |
| E7 | World: terrain tessellation as the game does it, world cells (`World_CellActivate_RunPersistant` 0x25fed0), instance queries `World_QueryInstances` 0x25b878, ray casts `World_RayCast` 0x25aff8 / `World_RayCastInstance` 0x25bf48 | unowned | E3a | | [repo] `collide.rs` (triangle grid of the drawn mesh) is a stand-in for these |
| E8 | Physics: check the ported boarder motion against the original once F4 exists: ground `cBoarder_GroundMotion_Update` 0x10a0d8, drag 0x109cb8, side friction 0x109ef8, thrust 0x109950, spring 0x109878, probe 0x128ae8, surface table `SurfaceTable_Init` 0x256188 | any | F4a | | [repo] Ported in `rider.rs`; README still says some constants at the top of `rider.rs` are guesswork: find which |
| E8a | Air: `Air_IntegrateRK4` 0x12b340, `AirPredict_AtTakeoff` 0x123b10, landing `Air_MotionUpdate` 0x108378, `Landing_CheckAngles` 0x12ba78, `Landing_ChooseState` 0x109308 | any | F4a | | [repo] Ported, unchecked |
| E8b | Walls and objects: `Boarder_WallImpactCheck` 0x126540, wall collision 0x126250, `Boarder_InstanceCollisions` 0x125088, `Boarder_InstanceBounce` 0x125a00 | unowned | E7 | | [repo] |
| E8c | Knockable objects with the game's rigid bodies (`CollBody_PointMassInertia` 0x236530 and its step), replacing the ball physics in `props.rs` | unowned | E7 | | [repo] props.rs: "simulated as balls" |
| E8d | Rider-to-rider: `Boarder_RiderCollisions` 0x123fc8, `Boarder_RiderHit` 0x124920, shoves | any | F4a | | [repo] Ported, unchecked |
| E9 | Script machine: `TriggerScript_Run`, `TriggerScript_ExecOp` 0x13bfd0 with every op, controllers (`TriggerScript_Op0_SpawnController` 0x13c600), persistent cell scripts, run at runtime instead of read statically | unowned | E3c | | [repo] `logic.rs` reads SSFLogic.json statically today |
| E9a | List every trigger-script op and controller type with its address and what it does, as rows E9b… | unowned | | | [est] |
| E10 | Input: pad reading, stick shaping and dead zones, button bits (the chord 0x600), vibration, `cCheatCodes::Update` 0x26CF60 | unowned | E1 | | [repo] findings.md |
| E11 | Camera: named cameras `cBxCamera_EvalNamedCam` 0x174888 (ported), director scripts `cBxCamera_RunDirectorScript`, replay and finish cameras, camera collision | any | | | [repo] Chase cam ported in `rider.rs`; free camera is ours (keep, tag) |
| E12 | Save and load: the memory-card profile, options, records, unlocks (`Profile_ResetNewGame` 0x161b40), in our own save file with the same fields | unowned | E1 | | [repo] `tricky-save.json` is the stand-in |

## P2 Gameplay systems

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| G1 | Boarder state machine: every state (0x00–0x14) and motion (0–6), their enter/update/exit, `cBoarder` vtable, in the original's order per tick | unowned | F1 | | [repo] Many states ported one by one; the dispatch is ours |
| G1a | Prewind, jump, takeoff, spin and flip control (`PrewindState_Update` 0x1050e8, `Jump_ApplyImpulse` 0x1284e0, `Takeoff_SetSpinRates` 0x126e30, `SpinState_Update` 0x100908, `SpinState_StartFromStick` 0x102158) checked | any | F4a | | [repo] Ported, unchecked |
| G1b | Rails checked: `RailSlideMotion_Update` 0x10b0d0, `RailSlideControl_Update` 0x1073e0, `Rail_TryCapture` 0x125cb8 | any | F4a | | [repo] README: rail speeds "still mine"; find what isn't ported |
| G1c | Finish, reset and gate states: `FinishState_Update` 0x106668, `ResetState_Update` 0x106a90, `Boarder_RequestReset` 0x118f10, `GateAnticipate_*` | any | F4a | | [repo] |
| G2 | Tricks and scoring checked: `Score_*` 0x155410–0x157490, `Score_BuildTrickId` 0x157490, `Score_FormatTrickName` 0x1551c0, repeats, chains, multipliers, uber timer, TRICKY | any | F4a | | [repo] Ported in `rider.rs`/`trickdata.rs` |
| G3 | Rider attributes and progression: stats per rider and level, training points, boards per rider and their unlock points, outfits | unowned | E12 | | [repo] `trickdata.rs` has the tables; rookie/master is our menu |
| G4 | AI checked: path following 0x1375c8 / 0x1372f8, rubber band 0x115100 / 0x13a0f0, skill pairs, tricks 0x139d58 / 0x138d60, resets, shoves | any | F4a, E1b | | [repo] Ported in `rider.rs`/`main.rs` |
| G4a | Rivals and vendettas checked: `RelTable_Init` 0x167208, `Rider_OnKnockedDownBy`, `AI_StartVendetta`, `AI_CoolVendetta` | any | F4a | | [repo] `rivals.rs` |
| G5 | Animation: the game's own animation state machine and blending for all 248 `bxanim` clips (plus cmanim, ps2anim, ubers), clip markers, replacing our choice of clip | unowned | F1, E4 | | [repo] README: "Which clip plays when is my state logic". Split by state before claiming |
| G6 | Wipe-out body: ragdoll spheres where the game puts them (`Ragdoll_Step` 0x10dca8, `Ragdoll_Contacts` 0x10e190, `CollBody_PointMassInertia` 0x236530) | unowned | E4 | | [repo] "Unresolved: ragdoll sphere places"; `wipeout.rs` uses a stand-in 1.7 m rider |
| G7 | Rendering, as the PS2 renderer does it. Split before claiming: | unowned | E3a, E3b | | [repo] |
| G7a | Terrain patches: the game's tessellation and LOD, subtractive lightmaps | unowned | G7 | | [repo] README "What the viewer taught us" |
| G7b | Objects: per-instance lights (ambient + 3 directional, 128 = 1.0), materials, the PS2 blend modes for transparent materials | unowned | G7 | | [repo] "exact PS2 blending modes" not done |
| G7c | Sky, fog, distance culling, `.ltg` grid use | unowned | G7, E3e | | [est] |
| G7d | Lights and halos, glows, lens flares | unowned | G7 | | [repo] "lights/halos" not done |
| G7e | Particles as the VU1 microcode draws them (0x31c288), spark and dust sprites | unowned | G7 | | [repo] "Unresolved: spark and dust sprite rendering"; `particles.rs` has the emitters |
| G7f | Board tracks, spray, breakable shards checked against `Fx_BoardTrack` 0x132648, `Boarder_UpdateEffects` 0x1350c0 | any | F4a | | [repo] `tracks.rs`, `spray.rs` |
| G7g | Rider shading and shadows, board reflections (env materials) | unowned | G7, E4 | | [est] |
| G7h | World animations checked: `cTexFlipNode` 0x142e90, `cUVScrollNode` 0x141ff0, `cAnimDeltaNode` 0x199d10, `cFlagNode` 0x144938, `cAnimObjectNode` 0x198810 | any | E9 | | [repo] `worldanim.rs` |
| G8 | HUD: `HUD_DrawPlayerSprites` 0x1a2c40, `HUD_DrawPlayerText` 0x1a4f70, `HUD_DrawSprite` 0x1c0068, all of it at 640×480, with the game's text from `american.loc` | unowned | E5 | | [repo] `hudsprites.rs`, `sfnfont.rs` partly done |
| G9 | Front end: title, main menu, mode select, rider / outfit / board select, options, records, trick book screens, unlock screens, music player, loading screens (`gApp+0x734`) | unowned | E1, E5 | | [repo] `ui.rs` is ours. Split by screen before claiming |
| G9a | Pause menu and in-race options; the leftover `cDebugMenu` (0x187850) kept as a dev menu | unowned | G9 | | [repo] findings.md |
| G10 | Audio engine (includes EA's sound library, `lib-snd` in the function index: 408 functions): EA BNK voices (`SND_ParsePTLayer` 0x2d5410, `SNDVoice_CalcPitchMult` 0x2d5288), 3D voices, mixing and volumes, decoded from the disc at run time | unowned | E2b | | [repo] Today `.wav` exported by vgmstream; README: stream picks "by length, looping and tone" |
| G10a | Rider and world sounds checked: `BoardIn_*`, `Voice_Update3D`, `Audio_SurfaceGroup`, `WorldEmitter_Update` 0x22bf60 | any | G10 | | [repo] `boardsound.rs`, `worldsound.rs` |
| G10b | Music: Pathfinder (`PF_*` 0x2bec00–0x2c2040) decoded at run time, song names from `music.inf` / `musicmap.inf` | unowned | G10 | | [repo] "Unresolved: song names"; `pathmusic.rs` |
| G10c | Announcer (DJ Atomika) and rider voices: when each line is picked and played | unowned | G10 | | [est] |
| G11 | Pre- and post-race scenes: `cPreRaceHandler`, `cEndRaceHandler_Update` 0x1141e0, scripts table 0x32f380, row filters | unowned | E11, G5 | | [repo] `intro.rs`; "Unresolved: post-race scene row filters" |
| G11a | Movies: the disc's video files (EA logo, intro, unlock videos) | unowned | E2 | | [est] Find the format first |
| G12 | Instant replay | unowned | E1, E11 | | [repo] in the debug menu (findings.md) |
| G13 | Pickups and course events checked: points, multipliers, boost, show-off time, speed boost, spin-boost zones (ops 6, 0xe–0x12) | any | E9 | | [repo] |
| G14 | Megaplex laps and boost tubes checked: `cLapBoostNode` 0x140b90, `cZBoostNode` 0x141cd8 | any | E9 | | [repo] |

## P3 Content

SSX Tricky has no missions. Its content is courses and events, and what a course does lives in the course's own
files (`.ssf` trigger scripts, `.aip` race lines), which we run on our engine. So we do not rewrite courses: a
course row is done when it plays start to finish, as race and as show-off, from the game files on our engine and
matches the original.

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| C0 | Read the real course and event list from the game: course table, which modes each course has, track limits and medal scores, so the rows below come from the game, not from us | unowned | F1 | | [est] original-rules.md already has time limits and medals per course |
| C1 | Garibaldi | unowned | C0, E9 | | [est] |
| C2 | Snowdream | unowned | C0, E9 | | [est] |
| C3 | Elysium Alps | unowned | C0, E9 | | [est] |
| C4 | Mesablanca | unowned | C0, E9 | | [est] |
| C5 | Merqury City Meltdown | unowned | C0, E9 | | [est] |
| C6 | Tokyo Megaplex (4 laps) | unowned | C0, E9, G14 | | [est] |
| C7 | Aloha Ice Jam | unowned | C0, E9 | | [est] |
| C8 | Alaska | unowned | C0, E9 | | [est] |
| C9 | Pipedream (show-off only) | unowned | C0, E9 | | [est] |
| C10 | Untracked (freeride) | unowned | C0, E9 | | [est] |
| C11 | The `trick` course (find what it is used for: tutorial or practice) | unowned | C0 | | [est] |
| C12 | World Circuit race: heats, advancement, medals, points, unlocks (`Circuit_*` 0x16b000–0x16d400, `Circuit_ProcessUnlocks` 0x16c420) | unowned | C0, E12 | | [repo] `ui.rs` has a stand-in |
| C13 | World Circuit show-off: clock, checkpoints, medals | unowned | C0, E12 | | [repo] README: show-off medals "still mine" |
| C14 | Single event, freeride and practice modes | unowned | C0, G9 | | [est] |
| C15 | Trick book: chapters, unlock of uber tricks, rewards (`Score_TrickBookCheck` 0x157a00) | any | E5 | | [repo] `book.rs` |
| C16 | Unlockables: characters, outfits, boards, the hidden ones, and the button cheats | unowned | C12, E10 | | [repo] |

## P4 Finishing

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| X1 | No stand-ins left: `grep -rn STANDIN tricky-rs` is empty | any | F8 | | [est] |
| X2 | Function index at 100% ported (every function either ported or marked not needed, with a reason, e.g. PS2 hardware glue) | any | F1 | | [est] |
| X3 | Function index at 100% checked | any | F4, X2 | | [est] |
| X4 | Full playthrough of World Circuit, race and show-off, every course, gold everywhere, on our code | any | C1–C16 | | [est] |
| X5 | 100% completion: every rider maxed, every trick book chapter, every unlock | any | X4 | | [est] |
| X6 | Runs from the player's own disc only: no SSX-Library, no Python export step, no vgmstream | any | E2–E5, G10 | | [est] |

## P5 Later

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| L1 | Two-player split screen | unowned | X4 | | [est] |
| L2 | Other versions (PAL, GameCube, Xbox): only after F7 | humans | F7 | | [est] |
| L3 | Our own extras clearly kept apart from the port: track editor (`editor.rs`), free camera, widescreen, higher resolution | any | | | [repo] |
| L4 | The `mods/` patches for the original game (debug menu, unlock all): test them in PCSX2 | any | | | [repo] README: "have not been run in an emulator yet" |

## Done

| ID | Task | Section | Who | Date | Commit |
|---|---|---|---|---|---|
| D1 | Ghidra project analysed, RTTI recovered (394 classes, 2,884 names, `tools/rtti_scan.py`), decompiler export | P0 | ed-slopper | 2026-10-05 | 9e4e994 |
| D2 | Patcher (`tools/ssxpatch.py`) and the debug-menu / unlock-all mods | P5 | ed-slopper | 2026-10-05 | 9e4e994 |
| D3 | Level viewer and first riding (`tricky-rs` stage 1–2), `.afl` animation format decoded | P1 | ed-slopper | 2026-10-06 | bd4670b |
| D4 | First pass of riding, air, rails, tricks, scoring, wipe-outs, AI, race lines, camera, world animations, sounds and music from the exe (`tricky-rs` stage 4, `original-rules.md`) | P1/P2 | ed-slopper | 2026-10-10 | 84ea0bf |
| F1 | Function index `tricky-rs/docs/function-index.csv` (7,425 functions, 2,320 named, 109 ported, 0 checked), `tools/function_index.py` to refresh it | P0 | ed-slopper/puffin | 2026-10-10 | 22a443f |
| F1a | Systems for the unplaced functions: `tools/function_systems.py` (call graph, shared globals, neighbours, proven address ranges, the library spans) writes `tricky-rs/docs/function-systems.csv`; 3,959 without a system down to 150 | P0 | ed-slopper/puffin | 2026-10-10 | f0c7b2d |
| F1b | Linked libraries split into bands (`LIB_BANDS` in `tools/function_systems.py`): EA sound and middleware (port), EA memory, Sony SDK, libc, C++ runtime, kernel stubs (host); `host` status in the index (1,181 functions); 13 library functions read and named in `ghidra/symbols.txt`, which the index now reads | P0 | ed-slopper/puffin | 2026-10-10 | 45ffac7 |
| F1d | `lib-ea` split into EA's shape files, compression (RefPack, Huffman, BTree), wide text and the `.loc` string table, BIG archives (port) and the async file system and streams (host); `comm` (DECI2 dev-kit link) is host; 24 functions named in `ghidra/symbols.txt` | P0 | ed-slopper/puffin | 2026-10-10 | 1ada4af |
| F2 | 107 names from `original-rules.md`, the notes and the source comments into `ghidra/symbols.txt` (97 global, 10 class methods incl. 2 constructors), 3 RTTI placeholders renamed (`cEndRaceHandler::Update`, `cLapBoostNode::Update`, `cZBoostNode::Update`); the index now takes every name from symbols.txt first | P0 | ed-slopper/puffin | 2026-10-10 | COMMIT |

## Changes to the board

- 2026-10-10 ed-slopper/puffin: board created from the repo, the local tree and the notes.
- 2026-10-10 ed-slopper/puffin: F1 done; added F1a, F1b (gaps of F1).
- 2026-10-10 ed-slopper/puffin: F1a done; F1b rewritten with the library spans found; added F1c (gap of F1a).
- 2026-10-10 ed-slopper/puffin: F1b done; added F1d (gap of F1b); G10 notes the EA sound library; AGENTS.md §14 had the symbols.txt line format wrong, fixed.
- 2026-10-10 ed-slopper/puffin: F1d done; E2a, E2b, E3b, E5 notes get the game's own functions for them.
- 2026-10-10 ed-slopper/puffin: F1d's Done row pointed at the claim commit; the work is 1ada4af (committed with the message "done F1d: commit hash on the board" after a broken command chain).
- 2026-10-10 ed-slopper/puffin: F2 done; added F2b (gap of F2).
