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
(`tricky-rs/docs/port-notes/`). It does **not** yet read the game's files itself: levels come from
SSX-Library's JSON export and characters, sounds and fonts from our Python tools. The front end, the animation
state machine and the renderer are ours. Nothing has been checked against the original by machine.

## P0 Foundations

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| F1c | Name the functions F1a placed only by guess (5,094 marked `?` in `tricky-rs/docs/function-index.csv`), system by system, in Ghidra and in `ghidra/symbols.txt`; start with boarder, race and world, the systems being ported now | any | F3 |  | Stopped 2026-10-11 by ed-slopper/puffin: index 5,702/7,426 named (~1,230 names this round, log below). The rest are mostly speech triggers on the race-line event markers (types 0x13-0x16, need the `.aip` event paths, E3d), surface sound loops whose bank sounds are unlabeled (`audio/unlabeled`), per-frame render/VU0 maths and duplicate FE list ops. Call counting and a sound log over PINE (entry patched to a counting stub; see the session notes in the log) tie functions to what plays in the running game
| F4c | Decide how exact our floats must be: `tricky-rs` computing in a PS2-float type (no inf/NaN/denormals, round toward zero: bit-exact with the original) or in IEEE `f32` with a stated tolerance in ULPs. Sets what `checked` means for every row | humans | | | From F4 (`tricky-rs/docs/checking.md`, "Floats are not IEEE"). Measured by F4f: in PCSX2 the game's add/sub/mul round to nearest (18 of 24 calls bit-exact, rest 1 ulp); the manual's toward-zero rule matched 3 of 24 |
| F4h2 | The rest of F4h, per tick over PINE: takeoff and the jump (`Jump_ApplyImpulse` 0x1284e0: find the ticks a rider leaves the snow in a recording, replay the game's tick code there in the runner from the recorded rider), the flight prediction (`AirPredict_AtTakeoff` 0x123b10, RK4 with sub-steps, $a3 ≠ 0: its results land in the rider) and the jump's steep-lip branch, which no recording reaches yet (the runner does: G1a1); add them to `tricky-rs/game/tests/replay.rs` | any | F4h | | Gap of F4h. Needs a recording with jumps (`pine.py record`, PCSX2 running a race); 2026-10-10 part done by ed-slopper/puffin: recording 3 (with jumps, stats saved) gives the air step 4,275 of 4,275 bit for bit; the game's jump impulse (smallest jump 630.88) explains 13 of 18 takeoffs to within one tick of gravity and ground forces, the rest are lip launches or charged jumps (`checking.md`, F4h2). Left: bit-exact takeoffs need the in-tick state (F4b3), the flight prediction, the steep lip |
| F4b3 | Replay test (pad recorded since F4b2; our physics callable without Bevy since F11a): load a recorded tick t (`pine.py record`) into our rider structs, feed the pad, step one tick, compare with t+1 field by field; or replay the game's own tick code in the function runner from a full RAM dump | unowned | F4b1, F4b2, F11a | | Gap of F4b1. Our tick must be callable without Bevy (F11) |
| F4g | Reproduce PCSX2's float rounding exactly: over 159 captured calls of `Boarder_ForwardDrag`/`Boarder_SideFriction` the best simple rule (add/sub/mul to nearest, the default) gives 111 bit-exact, the rest 1–2 ulp. The first instruction where runner and game part is `add.s` at 0x109d6c (drag's cub term), where only toward-zero multiplies + nearest add, or the EE adder, reproduce the game: PCSX2 likely emulates the EE multiplier/adder bit by bit. Model that (from the EE manual's description, not PCSX2's GPL source) in `tools/r5900/src/ps2float.rs`, test it against the 159 saved captures (`TRICKY_FLOAT_RULES`) and the per-tick replays of F4b, and tighten `tests/captured.rs` to exact | any | F4f | | Stopped 2026-10-10 by ed-slopper/puffin: probe tool, per-op `Rules`, `TRICKY_FLOAT_RULES` sweep, findings in `tricky-rs/docs/checking.md` (F4g). Captures stay on your PC; new data comes per tick over PINE (F4b); `tools/pcsx2_debug.py` needs PCSX2-MCP, no longer recommended; 2026-10-10 again by ed-slopper/puffin: normal play (PINE replays) rounds the air step to nearest, 341 of 341, no other rule comes close; the 1–2 ulp misses were COP1 under the debugger, single-op samples there are nearest too and a truncated Booth multiplier doesn't fit (`checking.md`, F4g continued). Next: a COP1-heavy function per tick in normal play |
| F7 | Decide the reference binary: `SLUS_203.26` (NTSC-U) as the only one, or also PAL / other platforms | humans | | | [est] Everything so far is NTSC-U. Option raised 2026-10-11: OpenTricky (github.com/GiZcesi/OpenTricky, GPL-3) recompiles the Xbox `default.xbe` into a native PC program, so the Xbox build could serve as a second running reference (behaviour checks, per-tick traces) without being a port source; its addresses would need an `xbox:` tag everywhere |

## P1 Core engine

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| E1 | Start-up and main loop: `entry` 0x100008, the `gApp` (0x337C58) init, `cGame::Init` 0x17E148, `cGame::Update` 0x181638, game modes and the front end ↔ race switch, pause (`cGame::IsPaused` 0x17F078) | unowned | F1 | | [est] Our Bevy app loop is the stand-in |
| E1a | Fixed 60 Hz tick with per-boarder time scale (`dt = timescale/60`, boarder+0x12c) driving every system, frame-rate independent of the window | any | | | [repo] `port-notes/README.md` (units); check what `SmoothDt` in `main.rs` does today Split: E1a1, E1a2 |
| E1a2 | The rest of E1a: world animations, props, particles, spray, camera on the same tick; draw between ticks (interpolate, ours: tag STANDIN) | any | E1a1 | | Split of E1a |
| E1b | Random numbers: the game's `rand` and seeding, used by AI tricks, grabs, announcer, so runs can match the original | unowned | F1 | | [est] Split: E1b1, E1b2 |
| E1b2 | Replace the xorshift stand-ins (`race.rs`, `props.rs`, `intro.rs`) with the game's streams at the game's call sites, in the game's order | unowned | E1b1 | | Split of E1b |
| E2c | The codecs the disc does not use: Huffman (`Huff_Decode` 0x2c3730), BTree (`BTree_Decode` 0x2c3540), 0x2c2d18 (types 0x18/0x1a/0x1c), for mods that pack files with them | unowned | E2b | | Gap of E2b. `Compress_Decode` 0x2c2de0's table is in `port-notes/files.md` |
| E3 | Level files read by our code, as the game reads them, replacing the SSX-Library JSON project folder (`level.rs`) | unowned | E2b | | [repo] level-editing.md. Split before claiming: |
| E3a | `.pbd`: Bezier patches, instances, models and meshes, materials, splines, lights | unowned | E3 | | [repo] |
| E3c | `.ssf`: collision models, physics data, trigger scripts | unowned | E3 | | [repo] |
| E3e | `.ltg` grid, `.map` names, `.adl` audio links, `_sky.pbd` | unowned | E3 | | [repo] |
| E4d | The rest of the `.mpf` meshes: the shadow meshes (material flags 0x10: positions only, another vector unit program) and the data after each of the face's 18 records (likely its expressions); the vector unit programs themselves, to prove the packet layout read from the data | unowned | E4a | | Gap of E4a. `port-notes/files.md` "Character and board models" |
| E5a2 | The HUD's and menus' text from `american.loc` by the ids the game asks for (`Loc_GetString` callers; `CONSTANT.LOC` names them) instead of our strings | unowned | E5a | | Gap of E5a |
| E5b | Config files: `DATA/CONFIG/*.INF` (SNOW, BANKS, MUSIC, MUSICMAP, INTROMUS, AUDIO, CROWD, CHANT, SPEECH, JUKEBOX, NASCRIPT) and `BTNMAP*.DAT` | unowned | E2b | | Split of E5 |
| E6 | Loading and memory: what the game loads when (load screens, course switch), only as far as it changes behaviour | unowned | E1 | | [est] |
| E7 | World: terrain tessellation as the game does it, world cells (`World_CellActivate_RunPersistant` 0x25fed0), instance queries `World_QueryInstances` 0x25b878, ray casts `World_RayCast` 0x25aff8 / `World_RayCastInstance` 0x25bf48 | unowned | E3a | | [repo] `collide.rs` (triangle grid of the drawn mesh) is a stand-in for these |
| E8 | Physics: check the ported boarder motion against the original once F4 exists: ground `cBoarder_GroundMotion_Update` 0x10a0d8, drag 0x109cb8, side friction 0x109ef8, thrust 0x109950, spring 0x109878, probe 0x128ae8, surface table `SurfaceTable_Init` 0x256188 | any | F4a | | [repo] Ported in `rider.rs`; README still says some constants at the top of `rider.rs` are guesswork: find which. Split: E8f–E8i |
| E8l | Play test of E8k on a level: `TRICKY_SIM=600`, `TRICKY_RAILTEST=1`, `TRICKY_TRICKTEST=1` with an SSX-Library level export (the E8k session had none); look for riders that stall, never leave the snow, or crash on every lip (the steep-lip branch of `Jump_ApplyImpulse` now runs on every take-off) and fix what the exact tick shows | any | E8k | | Gap of E8k. Needs a level folder on the player's PC. 2026-10-11 trailrig/heron released it unrun: the player has no SSX-Library export yet and the cloud workspace can't run `ssxlevel` (no .NET); export `gari.big` (SSX Multitool, or `ssxlevel extract`), or copy ed-slopper's `levels/gari`, then run the three self-tests and fix what they show 2026-10-11 trailrig/heron tried again: `RE-WORK/SSX Tricky/SSX-Library-main` on the PC is the library's C# source only (no program, no CLI) and `gari.big` is not extracted; the SSX Multitool (a separate download) is needed to write `levels\gari`
| E8a | Air: `Air_IntegrateRK4` 0x12b340, `AirPredict_AtTakeoff` 0x123b10, landing `Air_MotionUpdate` 0x108378, `Landing_CheckAngles` 0x12ba78, `Landing_ChooseState` 0x109308 | any | F4a | | [repo] Ported, unchecked Split: E8a1, E8a2 |
| E8a3 | The rest of E8a: `AirPredict_AtTakeoff` 0x123b10 (needs the world ray, E7), `Air_MotionUpdate` 0x108378 checked | any | F4a | | Split of E8a |
| E8b | Walls and objects: `Boarder_WallImpactCheck` 0x126540, wall collision 0x126250, `Boarder_InstanceCollisions` 0x125088, `Boarder_InstanceBounce` 0x125a00 | unowned | E7 | | [repo] |
| E8c | Knockable objects with the game's rigid bodies (`CollBody_PointMassInertia` 0x236530 and its step), replacing the ball physics in `props.rs` | unowned | E7 | | [repo] props.rs: "simulated as balls" |
| E8d | Rider-to-rider: `Boarder_RiderCollisions` 0x123fc8, `Boarder_RiderHit` 0x124920, shoves | any | F4a | | [repo] Ported, unchecked. Split: E8d1, E8d2 |
| E9 | Script machine: `TriggerScript_Run`, `TriggerScript_ExecOp` 0x13bfd0 with every op, controllers (`TriggerScript_Op0_SpawnController` 0x13c600), persistent cell scripts, run at runtime instead of read statically | unowned | E3c | | [repo] `logic.rs` reads SSFLogic.json statically today |
| E9b | Script threads: `TriggerScript_Run` 0x13be78 and the flow ops of `TriggerScript_ExecOp` 0x13bfd0 (4 wait, 5 conditions, 7/21/26 new threads, 3/9/10/11 controller messages) run at runtime, in the game's order | unowned | E3c | | Gap of E9a. Table in port-notes/scripts.md |
| E9c | Pickup ops 6, 14, 15, 16 (points, multiplier, meter, time; show-off only) and 17, 18 (speed, spin boost): `Pickup_*` 0x11e608–0x11e958, `Score_AddPickupPoints` 0x155b10, `Score_SetPickupMultiplier` 0x155cd0, `Score_AddPickupTime` 0x155bb8 | unowned | E9b | | Gap of E9a. G13 checks them |
| E9d | World ops 1 (camera node), 2 (effects, spline riders), 8 (sound), 13 (reset), 23 (camera shake `Cam_Shake` 0x177758), 24 (teleport), 25 (rail switch `RailMan_SetRail` 0x149030) | unowned | E9b | | Gap of E9a |
| E9e | Logic controllers: `cDebounceNode` (2), `cCounterNode` (6), `cTimerNode` (8), `cTrickTriggerNode` (21), dead-node modes (5, `TriggerScript_ApplyControllerOp` 0x13ac88) | unowned | E9b | | Gap of E9a |
| E9f | Boost controllers: `cBoostNode` (7), `cLapBoostNode` (15), `cRandomBoostNode` (16), `cZBoostNode` (18), `cTubeEndBoostNode` (24) | unowned | E9b | | Gap of E9a. G14 checks 15 and 18 |
| E9g | Breakables: `cFenceNode` (12), `cCrackedNode` (14), `cMeshAnimNode` (20) | unowned | E9b | | Gap of E9a |
| E9h | Movers: `cRollerNode` (0), `cRailNode` (9) | unowned | E9b | | Gap of E9a |
| E9i | Texture and object animation controllers: `cUVScrollNode` (10), `cTexFlipNode` (11), `cFlagNode` (13), `cUVScrollTexFlipNode` (19), `cAnimObjectNode` (0x100), `cAnimDeltaNode` (0x101), `AnimComboNode_Init` (0x102), the unnamed node 0x19a5a8 (0x103) | unowned | E9b | | Gap of E9a. G7h checks five of them |
| E9j | The rest: `cCrowdBoxNode` (17), `cParticleNode` (22), `cMovieNode` (23) | unowned | E9b | | Gap of E9a |
| E10 | Input: pad reading, stick shaping and dead zones, button bits (the chord 0x600), vibration, `cCheatCodes::Update` 0x26CF60 | unowned | E1 | | [repo] findings.md |
| E11 | Camera: named cameras `cBxCamera_EvalNamedCam` 0x174888 (ported), director scripts `cBxCamera_RunDirectorScript`, replay and finish cameras, camera collision | any | | | [repo] Chase cam ported in `rider.rs`; free camera is ours (keep, tag) |
| E12 | Save and load: the memory-card profile, options, records, unlocks (`Profile_ResetNewGame` 0x161b40), in our own save file with the same fields | unowned | E1 | | [repo] `tricky-save.json` is the stand-in |

## P2 Gameplay systems

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| G1 | Boarder state machine: every state (0x00–0x14) and motion (0–6), their enter/update/exit, `cBoarder` vtable, in the original's order per tick | unowned | F1 | | [repo] Many states ported one by one; the dispatch is ours |
| G1a | Prewind, jump, takeoff, spin and flip control (`PrewindState_Update` 0x1050e8, `Jump_ApplyImpulse` 0x1284e0, `Takeoff_SetSpinRates` 0x126e30, `SpinState_Update` 0x100908, `SpinState_StartFromStick` 0x102158) checked | any | F4a | | [repo] Ported, unchecked Split: G1a1, G1a2 |
| G1a5b | The rest of G1a5: `Rider::step` (`tricky-rs/game/src/rider.rs`) calls the exact prewind, jump, take-off, spin start and spin (`boarder::prewind_update`, `jump::jump_impulse`, `jump::takeoff_spin_rates`, `jump::spin_start_from_stick`, `spin::spin_update`, all checked) on the rider's own fields (cm, z up), replacing `Rider::air_rotation` and the hand-written prewind and jump | any | G1a5a | | Split of G1a5 (wiring; needs a play test). Split: G1a5b1, G1a5b2, G1a5b3 (the exact functions read the game's pad word, built per boarder state by `cPlayer::GetControls`) |
| G1a5b3 | The wiring: `Rider::step` builds the pad word with `GetControls` from our input and runs the exact prewind, jump, take-off, spin start and spin (`boarder::prewind_update`, `jump::*`, `spin::spin_update`) on the rider's own fields (cm, z up; the spin needs the rider's orientation as the game's quaternion), replacing `Rider::air_rotation` and the hand-written prewind and jump | any | G1a5b1, G1a5b2 | | Split of G1a5b (needs a play test). Split: G1a5b3a, G1a5b3b |
| G1a5b3b | The rest of G1a5b3: the rider in the game's boarder states (prewind 0xe, spin 0x10, …) builds the pad word from our input (`controls::PadDevice`, `ButtonMap::btnmap`, `get_controls`) and runs `boarder::prewind_update` (with the game's per-tick crouch and wind-up integration, to find), `jump::takeoff_spin_rates` and `spin::spin_update` (the rider's orientation as the game's quaternion), `jump::spin_start_from_stick` for late tricks; needs the boarder state machine's state numbers (G1) | any | G1a5b3a | | Split of G1a5b3 (needs a play test) |
| G1b | Rails checked: `RailSlideMotion_Update` 0x10b0d0, `RailSlideControl_Update` 0x1073e0, `Rail_TryCapture` 0x125cb8 | any | F4a | | [repo] README: rail speeds "still mine"; find what isn't ported |
| G1c | Finish, reset and gate states: `FinishState_Update` 0x106668, `ResetState_Update` 0x106a90, `Boarder_RequestReset` 0x118f10, `GateAnticipate_*` | any | F4a | | [repo] Split: G1c1, G1c2 |
| G1c3 | The rest of G1c: `Gate2BaseControl_Update` 0x105dc0, `ResetState_Update` 0x106a90, `ResetState_PlaceRider` 0x119220, `Boarder_RequestReset` 0x118f10 checked, with `Boarder_AdvanceOnPath` 0x118c08 and `Boarder_FindNearestLine` 0x119d30 (on `aipath.rs`) | any | F4a | ed-slopper/puffin (2026-10-11) | Split of G1c. Done so far: `AIP_CanResetAt`, `ResetState_GetRespawnPos`, `AIPathSet_FindRaceLineAt` 0x1982d0, `Boarder_AdvanceOnPath`, `Boarder_FindNearestLine` (to its `PlaceRider` call), bit for bit on every course (tests/aipath.rs) |
| G2 | Tricks and scoring checked: `Score_*` 0x155410–0x157490, `Score_BuildTrickId` 0x157490, `Score_FormatTrickName` 0x1551c0, repeats, chains, multipliers, uber timer, TRICKY | any | F4a | | [repo] Ported in `rider.rs`/`trickdata.rs` Split: G2a, G2b |
| G3 | Rider attributes and progression: stats per rider and level, training points, boards per rider and their unlock points, outfits | unowned | E12 | | [repo] `trickdata.rs` has the tables; rookie/master is our menu |
| G4 | AI checked: path following 0x1375c8 / 0x1372f8, rubber band 0x115100 / 0x13a0f0, skill pairs, tricks 0x139d58 / 0x138d60, resets, shoves | any | F4a, E1b | | [repo] Ported in `rider.rs`/`main.rs`. Split: G4a, G4b, G4c, G4d, G4e, G4f |
| G4a | Rivals and vendettas checked: `RelTable_Init` 0x167208, `Rider_OnKnockedDownBy`, `AI_StartVendetta`, `AI_CoolVendetta` | any | F4a | | [repo] `rivals.rs` Split: G4a1, G4a2 |
| G4e | The app's computer riders ride with `aicomp::Ai::ride` (the four modes, the shove, the target choice) on the course's own jump markers and speed hints (`AIComputer_FindJumpMarker` 0x1392a8 / `AIComputer_FindSpeedHint` 0x1393e0 over the AI paths, E3d) and the line point (+0x360), replacing `AiDriver`'s steering and throttle | any | G4d, E3d, G4f | | Split of G4 (wiring; needs a play test) |
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
| G15 | Spray, trail and splashes per surface from the real table (`tricky_game::surface`): `surf()` in `tricky-rs/src/spray.rs` is rounded and guesses which surfaces spray; the game sprays from words [13..21] on every surface but 9, 13, 18, 19 (those spark, `Fx_SurfaceSpray` 0x1311b0) and uses [22..24] for the trail and splashes (port-notes/effects.md) | any | E8f | | Gap of E8f; corrected by E13 |
| G16 | Draw the rider at rider+0x308 above the snow (surface word [10], eased 1.6667 cm a tick on the ground), as `Boarder_UpdatePose` 0x129628 does; the port draws at the board's position | any | E8f | | Gap of E13 |
| G18 | `Boarder_SetMotion` 0x11f610 → 0x11c648 and `Boarder_SetState` 0x11f608 → 0x11c7d8 bodies exact (the motion and state switch `Boarder_TakeOff` and the ground tick call; the `TakeOffEnv` / `Env` of `orient.rs` / `ground_motion.rs` take them) | any | E8j | | Gap of E8j |
| G19 | The crash spray exact: `cBoarder::EmitBrakeSpray` 0x11a798 → `Boarder_EmitBrakeSpray` 0x136e70 → `BoarderRender_EmitBrakeSpray` 0x135258 → `Fx_BrakeSpray_Emit` 0x12e008 (effects only; position, velocity, normal) | any | E8f | | Gap of E8j; overlaps G15 |
| G20 | The inputs the ground tick still takes from the port: stats+0x13 (grip), +0x19 (cubic drag), +0x60 (probe offset), rider+0x2c0 (the rider's up), the control state (always 3, or 0xb after a hard landing: `Env::set_state` revert is ours), the ray cast's hit kind (+0x2e0, always 0), the attitude quaternion across a landing / respawn (rebuilt from yaw and normal) | any | E8k | | Gap of E8k; grip and cubic overlap G3, the state G1, the ray cast E7. Split: G20a–G20c |
| G20a2 | The rest of the stat bytes: the computer riders' bytes from `stats::ai_bytes` with their rival level (`*param_3` of `Career_SetupAIRider` 0x16ae68: find where `Career_BuildRaceRiders` 0x165cf0 gets it, career +0x164 + rider·8?), the player's career level bytes (career record +0xc..+0xf) instead of our training fraction, and the stats the port still takes from its floats (stability, tricks → `Boarder_AddMeter`'s stat1d, wind-up stat18, gate stat7, meter leak) read from `Stats::bytes` | any | G20a | | Gap of G20a; overlaps G3, G4 |
| G20b2 | The stance flips that negate the steer and the lean: `touch_down` and the rail code flip `switch` without negating steer (+0x214) or lean (+0x244) as `ChangeFakieControl_Enter` 0x103eb8 and `RailSlideControl_SwitchStance` 0x107fc8 do (only the ground revert, state 4, does) | any | G20b | | Gap of G20b; overlaps G1, G18 |
| G20c2b | The normal (rider+0x2a0) in the air and under a respawn: `AirMotion_Update` 0x108378 takes it from its down ray (`World_RayCast` 0x25aff8) whenever that hits (store at 0x108588) and `Boarder_PlaceAt` 0x11cae8 from its placement ray (0x11cd08); the port keeps its last normal in the air and always (0, 0, 1, 0) at a respawn (`Rider::lean_aim`, `Rider::respawn` STANDIN) | any | G20c2, E7 | | Gap of G20c2; runner test as `tests/normal.rs` (the ray given) |
| G20c2c1 | The world grid the rail query walks, built as the game builds it: `RailWorld` (`ground_tick.rs`; the object at `gWorld` 0x346488: cells, patch records, curves with their 4×4 rows and owners) filled from the course instead of the port's flattened polylines (`rails.rs`); find the builder (who writes 0x346488: `Course_LoadRenderData` 0x25f35c?, the spline-to-curve conversion), port it, then `Rider::rail_step` and `Rail_TryCapture` 0x125cb8 take `RailWorld::closest_point` for `Rails::nearest` | any | G20c2c, E7 | | Gap of G20c2c; the grid is shared with `World_RayCast` (E7). Checked by building the grid from a level export and comparing it with the one the real builder makes in the runner
| G20c2c2 | The rail's attitude exact: `Quat_RotateVecToVec` 0x2508b8 and `Quat_Slerp` 0x250a48 ported (`math.rs` or `orient.rs`) and the second half of `RailSlideMotion_Update` 0x10b0d0 (0x10bbb8 on: the target quaternion from the normal, the tangent and the lean angle, `Quat_Slerp` at 1/12 into rider+0x170, `Boarder_UpdateOrientation` 0x129008 for +0x190 and +0x1a0) as `ground_tick::rail_attitude` | any | G20c2c | | Gap of G20c2c; runner test as `tests/rail_world.rs` (quaternion and axes bit for bit)
| G20c2c3 | `Rider::rail_step` runs the exact rail normal: the rider keeps the attitude quaternion (+0x170) and its axes through a rail (seeded at the capture from the air or ground attitude), the board's right and up (+0x190, +0x1a0) and the frame rows (+0x4a60..) are the game's, and `ground_tick::rail_slide_normal` + `rail_attitude` replace `rail_lateral` and the last normal | any | G20c2c1, G20c2c2, G20c3 | | Gap of G20c2c; overlaps G1, G5
| G20c2c4 | The rest of `RailSlideMotion_Update` 0x10b0d0 exact: velocity blended to the tangent at 30 per second, gravity along it, the balance (`rail_step`'s hand-written offset, its clamps and the stick force), the push (277.8 cm/s) off the rail outside the fall-off limits and the take-off chain it calls (`Score_StartTrickChain`, `Boarder_AddMeter`, `Takeoff_Setup`, `Boarder_PlayTakeoffClip`, `Boarder_SetMotion` 0x11f610 → 0x11c648, `Boarder_SetState` 0x11f608), run in the runner next to `tests/rail_world.rs` | any | G20c2c | | Gap of G20c2c; overlaps G1b, G18
| G20c3 | The control state the ground tick takes as 3 (or 0xb after a hard landing: rider+0x428, `Env::set_state` revert is ours), the ray cast's hit kind (+0x2e0, always 0 here) and the attitude quaternion across a landing and a respawn (rebuilt from yaw and normal) | any | G20c | | Gap of G20c; overlaps G1, G18, E7 |

## P3 Content

SSX Tricky has no missions. Its content is courses and events, and what a course does lives in the course's own
files (`.ssf` trigger scripts, `.aip` race lines), which we run on our engine. So we do not rewrite courses: a
course row is done when it plays start to finish, as race and as show-off, from the game files on our engine and
matches the original.

| ID | Task | Area | Needs | Who | Notes |
|---|---|---|---|---|---|
| C0 | Read the real course and event list from the game: course table, which modes each course has, track limits and medal scores, so the rows below come from the game, not from us | unowned | F1 | | [est] `port-notes/ai.md` ("AI, race, camera") already has time limits and medals per course |
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
| F2 | 107 names from `original-rules.md`, the notes and the source comments into `ghidra/symbols.txt` (97 global, 10 class methods incl. 2 constructors), 3 RTTI placeholders renamed (`cEndRaceHandler::Update`, `cLapBoostNode::Update`, `cZBoostNode::Update`); the index now takes every name from symbols.txt first | P0 | ed-slopper/puffin | 2026-10-10 | 4f78122 |
| F2a | Naming rules in AGENTS.md §14 rewritten from the names in use (modules, states and motions, when a function is a class method, data, exceptions) and checked by `python tools/function_index.py names` (3,028 names, 0 breaking); 4 renames recorded in the new `ghidra/renamed.txt`, which the index reads so old names in the source still count | P0 | ed-slopper/puffin | 2026-10-10 | 1c87a92 |
| F4 | Design for checking our code against the original: `tricky-rs/docs/checking.md` (F4a function runner: loading, EABI calls, stubs, PS2 float rules, the 75 instructions the first ten targets use, worked example `Boarder_ForwardDrag`; F4b PCSX2 PINE traces); `tools/insn_census.py` counts the instructions any function set needs from your own ELF | P0 | ed-slopper/puffin | 2026-10-10 | bb32dca |
| F4a | R5900 function runner `tools/r5900` (no dependencies): loads your own `SLUS_203.26` from `TRICKY_ELF` (md5 checked, skips without it), `$gp` from the entry code, EABI calls, stubbed calls, PS2 float rules, 47 instruction kinds with tests; runs `Boarder_ForwardDrag` and `Boarder_SideFriction` and agrees with their formulas to 2–7 ULPs toward zero | P0 | ed-slopper/puffin | 2026-10-10 | e8c902d |
| F3b | PCSX2-MCP set up: its patched PCSX2 running SSX Tricky (SLUS-20326, CRC 8e7cff62), DebugServer and PINE connected, MCP server registered for Claude Code (`setup-mcp.bat` only configures Antigravity, so it was added to `~/.claude.json` by hand); `tools/pine.py` is our own PINE client | P0 | ed-slopper | 2026-10-10 | cb3b121 |
| F4f | The runner checked against the running game: `tools/pcsx2_debug.py capture` records real calls in a race (registers, the memory they read, the result), `tools/r5900/tests/captured.rs` replays them; 24 calls of `Boarder_ForwardDrag` and `Boarder_SideFriction`: 18 bit-exact, 6 within 1 ulp once add/sub/mul round to nearest (`ps2float::Rules::Measured`, now the default; the manual's toward-zero rule matched 3) | P0 | ed-slopper/puffin | 2026-10-10 | 8d0298f |
| F4e | Runner widened (`vrsqrt`) and `tools/r5900/tests/targets.rs`: `AI_RubberBandSpeedScale`, `Takeoff_SetSpinRates`, `Jump_ApplyImpulse` (snow and rail), `Air_IntegrateRK4` run to the end and match the notes' formulas; pure callees run as the original, the rest are recording stubs; `Math_Sin`, `Math_Cos`, `Vec4_Scale`, `Boarder_AddMeter` named; the jump's F found (rider+0x320) | P0 | ed-slopper/puffin | 2026-10-10 | 10575d3 |
| F5 | CI on GitHub Actions: Guard on every push (`tools/check_no_game_files.py`: no game file types, executables, disc images, decompiler output, big files; Python tools compile; names follow §14), Function runner (Linux and Windows, clippy `-D warnings`) when `tools/r5900/` changes, tricky-rs (Linux: test, clippy with findings as public annotations) only when its code changes; superseded runs cancelled, build cache kept; fixed the one clippy error (taunt angle, exact cos 3π/4) | P0 | ed-slopper/puffin | 2026-10-10 | 69eb873 |
| F4b1 | Race memory map over PINE and a per-tick recorder: race object (riders +0xC4, count +0x88, tick +0x18 at 60/s), rider position +0x140, velocity +0x150, time scale +0x12C, place +0x110, stats +0x464 → fixed table 0x32DB70 + slot·0x84; `pine.py riders`, `record` (181 consecutive ticks in 3 s), `counters`; map in `tricky-rs/docs/checking.md` | P0 | ed-slopper/puffin | 2026-10-10 | 56e2c5e |
| F4b2 | The pad state over PINE: Sony's pad double buffer (0x845940/0x8459C0, 20 bytes: status, mode 0x79, buttons active-low, sticks, 12 pressures), checked live key by key; `pine.py` finds it by its shape and `record` saves it with every tick | P0 | ed-slopper/puffin | 2026-10-10 | 3314364 |
| F11a | Riding physics as the Bevy-free crate `tricky-rs/game` (`tricky-game`, glam + serde): rider physics, wipe-outs, trick names, collide, rails, trickdata, anim; `tricky-rs` re-exports them (paths unchanged); builds and tests on Windows GNU; first tests: free fall at the game's −1900.84 cm/s², angle wrap; CI green | P0 | ed-slopper/puffin | 2026-10-10 | 3348a20 |
| E8e | `Boarder_ForwardDrag` and `Boarder_SideFriction` ported exactly into `tricky-rs/game/src/ground.rs` (constants, cm units, the machine code's order), called from `Rider::step`; the "state 3/4 factor" was the powder depth, already there; side friction's rider+0x214 term added | P1 | ed-slopper/puffin | 2026-10-10 | b638d9f |
| F4d | First functions `checked`: `Boarder_ForwardDrag` 0x109cb8 and `Boarder_SideFriction` 0x109ef8 bit-identical to the original in the function runner on 3,000 random inputs each (`tricky-rs/game/tests/checked.rs`); `function_index.py` scans both crates | P0 | ed-slopper/puffin | 2026-10-10 | b638d9f |
| F4d2 | The other leaves `checked`: `Boarder_GroundSpringForce` 0x109878, `Boarder_GroundThrust` 0x109950 (boost inside the thrust, none while braking; skate clip 0x221 a STANDIN until G5) and `Air_IntegrateRK4` 0x12b340 (`tricky-rs/game/src/air.rs`, now the port's air step), each bit-identical on 3,000 random inputs; five functions checked | P0 | ed-slopper/puffin | 2026-10-10 | 27160f4 |
| F4h | Per tick against the running game (`tricky-rs/game/tests/replay.rs`, recordings via `TRICKY_RECORDING`): the air step is bit-identical to PCSX2 on 341 of 342 airborne rider-ticks (the other one nudged by something else), the rubber band (`AI_RubberBandSpeedScale` in the runner + rate limit) on 1,782 of 1,800 (misses are the recorder's timing, one step either way); findings in `tricky-rs/docs/checking.md` | P0 | ed-slopper/puffin | 2026-10-10 | ded4df1 |
| F5a | Clippy strict on tricky-rs: the 53 warnings fixed (31) or allowed with a reason (`too_many_arguments`, `type_complexity` crate-wide in `main.rs`, as Bevy code does; one parsed-but-unread animation field); `.github/workflows/tricky-rs.yml` runs clippy with `-D warnings` | P0 | ed-slopper/puffin | 2026-10-10 | 39eecac |
| F6 | Port notes split per system: `tricky-rs/docs/original-rules.md` into `tricky-rs/docs/port-notes/` (boarder, air, rails, scoring, wipeout, ai, race, camera, world, scripts, effects, audio, hud, frontend; `README.md` is the index with the units and the unresolved list); every line kept, references updated, function index unchanged | P0 | ed-slopper/puffin | 2026-10-10 | 5e5f751 |
| F8 | Stand-ins tagged `STANDIN:` (14 in all with the skate-push one): SSX-Library level folder, Python-exported characters/clips and fonts, the code-built pose, our clip choice, our front end, ball physics for props, effect samples picked by ear, the particles' drag curve, the wipe-out spheres' placement, the body sphere and rail reach, the box placeholder rider; five unused guessed constants removed (`EDGE_GRIP`, `WALL_BOUNCE`, `CRASH_PITCH`, `RAIL_FRICTION`, `GRAB_REACH`); README no longer calls the game's show-off medals ours; function index unchanged | P0 | ed-slopper/puffin | 2026-10-10 | 9455dd4 |
| F9 | Addresses on recreated functions: 51 of 473 functions in `tricky-rs/src` and `tricky-rs/game/src` now carry `` `Name` 0xaddr `` in their doc comment (was 21): names without their address, originals cited only in the body, and the module headers' lists moved onto the functions that port them (course, music, HUD, fonts, tracks, world sound, wipe-out, controllers, `Rider::step`). The other 422 are Bevy systems, loaders and helpers (our glue) or pieces of a function that carries the address; function index +2 ported (`cMeshAnimFrame::vf1`, `Landing_ChooseState`) | P0 | ed-slopper/puffin | 2026-10-10 | 944ea96 |
| F10 | Duplicate tools removed: `tricky-rs/tools/{bnk,mpf,music,sfn}` was a byte-identical copy of `tools/{bnk,mpf,music,sfn}`; the repo's `tools/` stays (its scripts find each other by relative path), and the tricky-rs README says where they are. `tools/mpf/mpf2json.py` and `tools/music/mpf2json.py` are two different tools with one name, both kept | P0 | ed-slopper/puffin | 2026-10-10 | 2664d58 |
| F11b | The rest of F11: new Bevy-free crate `tricky-rs/data` (`tricky-data`: level folder, level scripts, characters and clips, race lines and AI paths, camera scripts, object curves), and the race, AI driver, chase camera (`game/src/race.rs`) and props (`game/src/props.rs`) into `tricky-game`, which now builds on `tricky-data`; the app re-exports both (paths unchanged), CI tests the new crate, `function_index.py` scans it; all tests pass, function index unchanged. The sound loaders stay in the app (they hand audio to Bevy) | P0 | ed-slopper/puffin | 2026-10-10 | 3eb8498 |
| F3 | ghidra-mcp set up on ed-slopper's PC: Ghidra 12.1.4 + Emotion Engine extension at `D:\code\modding\ghidra_12.1.4_PUBLIC`, the repo's `ghidra/Project` opened (owner changed from root), `symbols.txt` applied (ApplySsxSymbols), ghidra-mcp 7.0.0 built from source and enabled, bridge added to Claude; `force_decompile` 0x12b340 returns `Air_IntegrateRK4`. Install snags written into AGENTS.md §14 | P0 | ed-slopper (with puffin) | 2026-10-10 | 406fa6b |
| F2b | Settled in Ghidra (ghidra-mcp): `Score_TrickBookCheck` and `TriggerScript_Op0_SpawnController` are 0x1579e0 and 0x13c5d0 (the notes' addresses were 0x20/0x30 in; nothing branches there); `WipeoutMotion_Update` 0x10c2f0 created as a function (one 5,004-byte body up to 0x10d680) and added to the index; 0x343150 is `gMusicSys`, the global pointer to the one `cMusicSys` (its destructor clears it); 0x198810 and 0x199d10 are the `cAnimObjectNode` and `cAnimDeltaNode` constructors (they install vtables 0x38c650/0x38c558, whose type-info entries are those classes'); `cTubeEndBoostNode::vf27` renamed `BoostRider` (pushes a rider in slot n toward the slot's speed along the tube); all seven F2 class methods proven (called only from their class's vtable methods or constructors, or in the vtable: `cAnimDeltaNode::Update`), five constructors named. symbols.txt, renamed.txt, port notes, index updated | P0 | ed-slopper/puffin | 2026-10-10 | 7ef6924 |
| E8f | Surface table checked: `SurfaceTable_Init` 0x256188 run in the function runner, all 500 words in `tricky-rs/game/src/surface.rs` bit-identical (`tests/surface_table.rs`); SURFACES, SURF_EXTRA, SURF_BAND in rider.rs now taken from it (were rounded; row 0's air height and damping were wrong) | P1 | trailrig/heron | 2026-10-11 | c2a1967 |
| E2 | Read the player's disc: the game's CD file table (`CdFs_Mount` 0x2c9970, `CdFs_FindFile` 0x2c9cc0, its `qsort`/`bsearch`) ported 1:1 in `tricky-data/src/disc.rs`, from an image or a drive; checked against an independent ISO walk (260 files) and a byte compare; notes in `port-notes/files.md` | P1 | ed-slopper/puffin | 2026-10-11 | 27e5bfb |
| E2a | BIG archives (`C0FB`, `BIGF`) in `tricky-data/src/big.rs`, ported from the game's loader: `BIG_Identify` 0x2cbc88, `BIG_HeaderSize` 0x2cbd08, `BIG_Lookup` 0x2cbdc0, the mounts of `App_OpenBigFiles` 0x17bdf8 in order and the opener's (0x2cb328) disc / `|name` / `archive|name` rules; all 793 character-archive entries checked against the extracted copies | P1 | ed-slopper/puffin | 2026-10-11 | 3886064 |
| E2b | RefPack decompression in `tricky-data/src/compress.rs`: `Compress_UnpackedSize` 0x2c2fb0, `Compress_Decode` 0x2c2de0 (RefPack: the only codec on the disc), `RefPack_Decode` 0x2c31b0, and the loader's rule (`File_LoadAndUnpack` 0x2c8938) as `Files::load`; all 793 character-archive entries equal `tools/afl/refpack.py` | P1 | ed-slopper/puffin | 2026-10-11 | b01f37e |
| E13 | Surface table's untraced words read in Ghidra: [10] body height above the snow (`cBoarder::vf7` 0x117198 → rider+0x308 → `Boarder_UpdatePose` 0x129628), [13..21] ground spray (`Fx_SurfaceSpray` 0x1311b0), [22..24] trail and splashes (`Fx_BoardTrail_Update` 0x12fef0, `Fx_LandingSplash` 0x12ccc8, new `Fx_Splash_EmitAtPoint` 0x12c7e0); readers found by scanning for the table pointer (cGame+0x24); `surface.rs`, port notes | P1 | trailrig/heron | 2026-10-11 | 1348d46 |
| E1b1 | The game's random numbers in `tricky-game` (`rand.rs`): `Rand_Seed` 0x2402c0, `Rand_Next` 0x2401e0, `Rand_Fx` 0x23d4a8, `Rand_Game` 0x23d4d8, `Rand_GameRange` 0x23d4f8, `Rand_GameJitter` 0x23d560 checked bit for bit in the function runner (`tests/rand.rs`: 3,000+ draws incl. every carry, 3,000 range/jitter calls); seeding wrappers and the replay snapshot ported | P1 | trailrig/heron | 2026-10-11 | c38d5b4 |
| G4a1 | `RelTable_Init` 0x167208 checked: tolerance, attitude and relation of all 144 pairs in `rivals.rs` equal the original's in the function runner (`tests/rel_table.rs`); `rivals.rs` moved into `tricky-game` (the app uses it from there) | P2 | trailrig/heron | 2026-10-11 | d80e0b1 |
| E8i | The constants at the top of `rider.rs` sorted out: 22 unused or doc-only ones removed (and the dead `spin_rate`/`flip_rate` fields); speed caps and their decay (`cBoarder::vf7` 0x117198), air gravity (`Air_IntegrateRK4`, from `air.rs`), the smallest jump (`JumpControl_Update` 0x1046e4), the jump's speed curve (`Jump_ApplyImpulse` 0x1284e0) and the boost drain (`Boarder_UpdateBoost` 0x11d218) now the game's exact floats; the boost cap needs a meter above 0, as the game's does; `TURN_ACCEL` tagged STANDIN (AI corner speed); scoring constants left to G17 | P1 | trailrig/heron | 2026-10-11 | cc1a764 |
| G17 | Scoring constants exact: `PTS_*` and `SWITCH_BONUS` in `rider.rs` are the game's units (bit patterns from `Score_UpdateSpin` 0x155e00, `Score_UpdateFlip` 0x155e68, `Score_StartGrab` 0x155ee8, `Score_Tick` 0x156e80, `Score_LandTrick` 0x1569b0, `Score_StartTrickChain` 0x156618) × the points scale of `Score_CalcPoints` 0x155440; ubers score by their own unit (`PTS_UBER`, 0x156148), not the hold one; 3 score functions named | P2 | trailrig/heron | 2026-10-11 | 5587fea |
| E4a | Character and board models from the player's disc: the `.mpf` reader in `tricky-data/src/mpf.rs` (the loader's tables proven from `SkinPart_Load` 0x205080 and `MpfFile_AddPart` 0x208488, the VIF packets inferred from the data), the game's part-name tables, `CharModel::rider_from_disc` / `board_from_disc`, `TRICKY_DISC` in the app; all 12 riders and 6 board shapes equal the exported models | P1 | ed-slopper/puffin | 2026-10-11 | 82b6fdd |
| E9a | Every trigger-script op (0–26) and controller type (0–24, 0x100–0x103) listed with its handler or class and what it does (`TriggerScript_ExecOp` 0x13bfd0, `TriggerScript_Op0_SpawnController` 0x13c5d0), port-notes/scripts.md; rows E9b–E9j; 9 functions named (pickups, their score helpers, camera shake) | P1 | trailrig/heron | 2026-10-11 | bc5ef48 |
| G1a1 | `Jump_ApplyImpulse` 0x1284e0 ported exactly (`tricky-rs/game/src/jump.rs`: cm, the vector unit's order, the game's own cos 50°/70° and sin 20°, the steep-lip branch) and checked bit for bit against the original in the function runner on 6,000 random riders, every branch (`tests/jump.rs`) | P2 | trailrig/heron | 2026-10-11 | 559d4f0 |
| G1a2 | `Takeoff_SetSpinRates` 0x126e30 ported exactly (`jump::takeoff_spin_rates`: the rates, the no-spin clips 0x218/0x260/0x261, which clip routine runs) and checked bit for bit in the function runner on 5,000 random take-offs, with its callees and their order (`tests/takeoff.rs`) | P2 | trailrig/heron | 2026-10-11 | ee98c47 |
| G2a | The score object's arithmetic ported exactly (`tricky-rs/game/src/score.rs`: points and base points, the meter share, spin rounding, spin/flip/grab units with the grab rate table, the per-tick hold, rail and timer step) and checked bit for bit in the function runner (`tests/score.rs`: 5,000 point calls, 5,000 roundings incl. the extremes, 8,000 unit updates); 8 functions checked | P2 | trailrig/heron | 2026-10-11 | 3352ce8 |
| E14 | The game's math library ported exactly (`tricky-rs/game/src/math.rs`: sine, cosine, sine-cosine, arcsine, arctangent with their polynomials and $gp coefficients, angle difference, Hermite) and checked bit for bit in the function runner (`tests/math.rs`: 20,000+ inputs each, the branch edges); port-notes/math.md; 7 functions checked | P1 | trailrig/heron | 2026-10-11 | 01744d3 |
| E4b | Animation clips from the player's disc: `tricky-data/src/afl.rs` (the game's curve evaluators `AnimCurve_Eval` 0x157ed8 / `AnimCurve_EvalPiece` 0x158030, file and clip headers), the exe's clip and file names (`anim_names.rs`, 0x3242a0), `AnimSet::afl_from_disc` / `uber_from_disc` / `scenes_from_disc`, read by the app with `TRICKY_DISC`; same clips as the exports, which read multi-piece curves one frame too long (`notes/animation-format.md`) | P1 | ed-slopper/puffin | 2026-10-11 | faeca52 |
| E8a1 | `Landing_CheckAngles` 0x12ba78 ported exactly (`tricky-rs/game/src/landing.rs`, using E14's `Math_Asin`/`Math_Atan`) and checked bit for bit in the function runner on 8,000 random landings, the early outs, state change and clean/unclean verdicts included (`tests/landing.rs`); the runner learnt vopmula/vopmsub (the vector unit's cross product, with a unit test) | P1 | trailrig/heron | 2026-10-11 | a04687e |
| E8a2 | `Landing_ChooseState` 0x109308 ported exactly (`landing::choose_state`: landing clip and state, grab reset, the alpine half turn) and checked in the function runner on 6,000 random landings: same clips, states, calls in order and fields bit for bit (`tests/landing_choice.rs`) | P1 | trailrig/heron | 2026-10-11 | ab9c4c4 |
| G1a3 | `SpinState_StartFromStick` 0x102158 ported exactly (`jump::spin_start_from_stick`: pad direction fields, axis snap, spin and flip rates, the clip by direction) and checked in the function runner on every pad word whose direction bits matter × both stances × every tricks stat (196,608 starts): rates bit for bit, same clip, states (`tests/spin_start.rs`) | P2 | trailrig/heron | 2026-10-11 | d503d45 |
| G1d | Boarder meter and input helpers ported exactly (`tricky-rs/game/src/boarder.rs`): `Boarder_AddMeter` 0x11b018, `Boarder_UpdateBoost` 0x11d038, `Boarder_SetCrouchTarget` 0x126c00, `PrewindState_SetWindTarget` 0x105a60, each bit for bit in the function runner with its sound/HUD calls in order (`tests/boarder.rs`, 8,000 cases each); the runner now sends tail calls (`j` into a stubbed function) through the stub (it ran the real callee before; unit test added) | P2 | trailrig/heron | 2026-10-11 | fd62ef1 |
| G1e | `Boarder_SetSteerTarget` 0x126948, `Boarder_IsTricky` 0x11f430 and `PrewindState_UpdateWindClip` 0x1055f0 (with its clip table 0x35fed0) ported exactly into `boarder.rs` and checked bit for bit in the function runner (10,000 steers, 20,000 wind-ups); the runner learnt ldl/ldr/sdl/sdr (unaligned doublewords, with a unit test) | P2 | trailrig/heron | 2026-10-11 | 2e951ad |
| E3b | `.ssh` textures in `tricky-data/src/ssh.rs`: the shape library's directory, block chain and name lookup (`Shape_FindByName` 0x2c2528, `Shape_GetLongName` 0x2c2610), 8-bit palette and RGBA images (the only kinds in the disc's 807 files); all 192 exported rider textures equal the disc's under the exports' 0-0x80 to 0-255 mapping (`port-notes/files.md`) | P1 | ed-slopper/puffin | 2026-10-11 | a363515 |
| G1a4 | `PrewindState_Update` 0x1050e8 ported exactly (`boarder::prewind_update`, on the checked G1d/G1e helpers) and checked in the function runner on 20,000 random ticks with the original's helpers running: every field and every call (boost sounds, clips, states, reset) bit for bit (`tests/prewind.rs`) | P2 | trailrig/heron | 2026-10-11 | 6b74122 |
| G1a5a | `SpinState_Update` 0x100908 ported exactly (`tricky-rs/game/src/spin.rs`: landing alignment, stick rates and band, auto-complete after letting go, half-pipe clips and the half-pipe turn, late wind-up, every call in order) and checked bit for bit in the function runner on 40,000 random ticks, every branch reached (`tests/spin.rs`) | P2 | ed-slopper/puffin | 2026-10-11 | 1e8df1e |
| G2b | Trick scoring ported exactly (`tricky-rs/game/src/score.rs`: `Score_LandTrick`, `Score_StartTrickChain`, `Score_BankPoints`, `Score_BuildTrickId` with both name tables, repeats, the chain bonus, air-time points, ranks, best trick, combo reset) and checked bit for bit in the function runner on 60,000 random calls in runs (every field, every call, every branch), plus `Score_FormatTrickName` against `TrickDef::name` on 20,000 ids with newlib's own sprintf (`tests/trick_score.rs`) | P2 | ed-slopper/puffin | 2026-10-11 | b03d74d |
| G2c | Uber timer and TRICKY ported exactly and checked bit for bit in the function runner: `Boarder_DrainMeter` 0x11b1f8 (`boarder::drain_meter`, 10,000 ticks), the grab end `Scoring_OnTrickEvent` 0x156148 with `Score_AddGrab` 0x1571e0 (`Score::end_grab`, 30,000 cases: the clip→grab table, uber count +0x5c that sums into rider+0x58a8); `Boarder_AddMeter` and `Boarder_IsTricky` were checked in G1d | P2 | ed-slopper/puffin | 2026-10-11 | 07f99c0 |
| G2d | `Rider` scores through the score object (`Rider::sc`): jumps and rail exits start chains, spins/flips/grabs/ubers/rails/landings/late tricks/crashes/respawns call the checked `score.rs` functions as the game's callers do (notes list each caller), the meter through `Boarder_AddMeter`/`Boarder_DrainMeter` on a 60 Hz clock, trick names from `Score_FormatTrickName`; `Score_Crash`, `Score_CommitAndRestartChain`, `Score_Reset` ported and checked; `tests/rider_score.rs`; self-tests run (TRICKY_TRICKTEST, RAILTEST, a 90 s race); STANDINs listed in scoring.md | P2 | ed-slopper/puffin | 2026-10-11 | 0b8d215 |
| E1a1 | The player's and the AI riders' physics in `ride` (`tricky-rs/src/main.rs`) on a fixed 60 Hz tick: whole 1/60 s ticks (× the AI's time scale), leftover frame time carried over, at most 0.1 s of ticks a frame (trailrig/heron's branch 7d77fde, merged). Run on Garibaldi: a windowed autoplay race (TRICKY_SCREEN=play, TRICKY_AUTOPLAY) reaches 1:00 in real time at 145 fps with the HUD and scoring live; TRICKY_SIM=600 finishes in 232.9 s. The `race.rs` self-tests still step 1/120 s | P2 | trailrig/heron, ed-slopper/puffin | 2026-10-11 | 5b51fa9 |
| G1a5b1 | `cPlayer::GetControls` 0x151250 ported exactly (`tricky-rs/game/src/controls.rs`: the pad word per boarder state, sticks, directions, buttons, the spin state's trick slot and random easy trick) and checked bit for bit in the function runner on 40,000 random pads over every state (`tests/controls.rs`); layout in air.md | P2 | ed-slopper/puffin | 2026-10-11 | c5dab73 |
| G1a5b2 | The pad mapper ported exactly (`tricky-rs/game/src/controls.rs`: the PS2 pad read `cPS2Device` vf10, `Pad_ReadAxis`, the four `cInput` queries, both disc button maps as `ButtonMap::btnmap`) and the trick table at 0x333098 (`tricky-rs/data/src/trick_slots.rs`); checked in the function runner (20,000 pad reads, 30,000 axes and queries, the game's own `Pad_ParseButtonMap` on BTNMAP0/1.DAT with TRICKY_BTNMAP_DIR) and the table against our riders' rows; runner gained lwl/lwr/swl/swr | P2 | ed-slopper/puffin | 2026-10-11 | b751c9d |
| G1a5b3a | Every jump of `Rider::step` (three on snow, the rail hop) through `jump::jump_impulse` (exact): steep lips leaned along the lip and their flag passed to `Score_StartTrickChain`. Self-tests on Garibaldi, Alaska, Merqury against main without it: within one wipe-out and 3 s per 600 s run (jump velocities equal to 0.01 m/s off steep lips) | P2 | ed-slopper/puffin | 2026-10-11 | fe79dcf |
| E8d1 | `Boarder_RiderHit` 0x124920 ported exactly (`tricky-rs/game/src/hit.rs`) and checked bit for bit in the function runner on 30,000 hits (tests/hit.rs: hop, push, stumble, wipe-out, practice threshold, every call); `Boarder_GetSpeedStat`, the pair's speed split and the knock-down scoring (`Scoring_MsgKnockdown`) ported; our riders' bumps and shoves go through them (`Rider::hit`, `props::bump`; STANDINs in wipeout.md) | P2 | ed-slopper/puffin | 2026-10-11 | 5146ed3 |
| E8d2 | The collision body (`CollBody_InitRider`, `CollBody_UpdateFromBones`, `CollBody_Translate`, `CollBody_OverlapSpheres`: `tricky-rs/game/src/collbody.rs`), the pair records (`Race_UpdatePairDistances` 0x115a80: distance and bearing; `hit::pair_distances`) and the shove (`hit::shove`) ported exactly and checked bit for bit in the function runner (5,000 bodies, 20,000 overlaps, 3,000 races of records, 30,000 shoves through `Boarder_RiderCollisions` itself); our shoves go through it | P2 | ed-slopper/puffin | 2026-10-11 | 6409b16 |
| G4a2 | The grudge bookkeeping ported exactly (`rivals::RivalRider`: `Rivals_OnBump`, `Rivals_OnHit`, `Rivals_OnKnockdown`, `Rivals_StartGrudge`, `Rivals_EaseGrudge`, `Rivals_UpdateGrudgeTimeout`, `Rivals_UpdateAttitude`; the row's `Rider_OnKnockedDownBy` / `AI_StartVendetta` / `AI_CoolVendetta` are these) and checked bit for bit in the function runner on 40,000 calls (tests/rivals.rs); the table's word +8 is the attitude's cap (`rivals::CAP`, checked in tests/rel_table.rs), read by `Rivals_InitPairs` | P2 | ed-slopper/puffin | 2026-10-11 | 041417b |
| G4a3 | The app's computer riders keep their grudges with the exact bookkeeping (`rivals::Grudge` rebuilt on `RivalRider`: pair records as `Rivals_InitPairs` sets them, the hooks for bumps, shoves and knock-downs both ways, the timeout in ticks, `Rivals_DecayAttitudes` between heats, `Rivals_UpdateAttitude` after); `tests/rivals.rs` (Eddie vs Moby level 2, Kaori's cap of 59 keeps her at level 0, Moby's level 1 cools after 6 s); a race with TRICKY_GRUDGE runs. STANDIN: `AIComputer_IsHostileTarget` / `AIComputer_ShouldShove` not ported | P2 | ed-slopper/puffin | 2026-10-11 | 24f70fa |
| G4b | The AI's target and steering helpers ported exactly (`tricky-rs/game/src/aicomp.rs`: `AIComputer_SteerTowards`, `AIComputer_SetThrottle`, `AIComputer_SetTuck`, `AIComputer_IsRivalTarget`, `AIComputer_PickTargetRider`) and checked bit for bit in the function runner (30,000 steers with throttle and tuck, 30,000 target picks; tests/aicomp.rs). Not yet driving the app's AI (that is G4's riding modes) | P2 | ed-slopper/puffin | 2026-10-11 | 8846c4a |
| G4c | The AI's shove decision ported exactly (`aicomp::find_nearby_rider`: `AIComputer_FindNearbyRider` 0x1394e0, the shove field `AIComputer_ControlsDispatch` writes) and checked bit for bit in the function runner on 40,000 cases (tests/aicomp.rs); the app's computer riders shove through it instead of our 'alongside with any grudge' (STANDIN pair record and heading from our positions) | P2 | ed-slopper/puffin | 2026-10-11 | e834b6d |
| G4d | The AI's four riding modes ported exactly (`aicomp::Ai::ride`: `AIComputer_RideLine`, `AIComputer_RideToJump`, `AIComputer_RideNearRider`, `AIComputer_RideAtRival`, their mode changes through the method table at 0x31d318) and checked bit for bit in the function runner on 40,000 ticks with the course queries scripted (tests/aicomp.rs: pad word, speed, target, mode, `AIComputer_PlanJumpTrick`) | P2 | ed-slopper/puffin | 2026-10-11 | 56c2136 |
| E3d | `.aip` / `.sop` read as the game reads them (`tricky-data/src/aip.rs`: `AIP_LoadPathsFromFile`, `AIP_LoadAIPaths`, `AIP_LoadEventPaths`, `AIPath_Load`, both path classes' parameters, the event-type table), checked against the game's own loader in the function runner on all 22 files of the disc (11 courses; tests/aip.rs) and against the exported AIP.json of every course; the app takes the course's paths from the disc with TRICKY_DISC; `big_file_list` / `aip_dump` examples | P2 | ed-slopper/puffin | 2026-10-11 | 0967a4d |
| G4f | The rider's path following ported exactly (`tricky-rs/game/src/aipath.rs`: the AI paths' geometry `AIP_GetPathLength`, `AIP_GetPositionAtDistance`, `AIP_GetDirectionAtDistance`, `AIP_ProjectOntoPath`, `AIP_ProjectOntoPathFrom`, `AIP_DistanceToLineBox`, `AIP_SortLinesByDistance`; `Boarder_UpdatePathAhead`, `Boarder_ChoosePathAhead` with the path preference) and checked bit for bit in the function runner on every course's own paths (tests/aipath.rs: 33,000 geometry cases, 16,500 ticks of path following). Found: rider+0x418 is the finish time, not human/AI (corrected in 117b482). `Boarder_AdvanceOnPath` / `Boarder_FindNearestLine` (resets) left to G1c3 | P2 | ed-slopper/puffin | 2026-10-11 | 9ff349c |
| E8d3 | The rest of E8d: our riders keep the game's collision body on their skeleton (needs the rider's bone positions in the game crate: animation, G5) and the pair records every tick (so the hop into the air and the wipe-out point work), replacing the one-sphere bump | any | E8d2, G5 | | Split of E8d |
| E4c | Rider textures from the player's disc: the exe's path tables (0x341a38 suit/boots, 0x341ce8 head/helm, 0x341d50 boards) as `ssh::rider_texture_path`, read by the app with `TRICKY_DISC` (outfit 1 and board 1 as the exports had them) | P1 | ed-slopper/puffin | 2026-10-11 | 9898b17 |
| E5a | Text from the disc: `tricky-data/src/loc.rs` reads `american.loc` as `Loc_GetString` 0x2c5160 / `Loc_MapId` 0x2c50b0 do (3,774 strings); `CONSTANT.LOC` (unused by the game) names every string id | P1 | ed-slopper/puffin | 2026-10-11 | c2040d5 |
| G1c1 | `FinishState_Update` 0x106668, `Boarder_SetBrakeCrouchInput` 0x126b38 and `Boarder_SetBrakeTarget` 0x126c78 ported exactly (`tricky-rs/game/src/finish.rs`) and checked in the function runner (`tests/finish.rs`: 20,000 random finish ticks over every phase, mode and outcome, 20,000 brake/crouch inputs): every field and call bit for bit; `Boarder_StartCelebrate` 0x11f3b8 and `Boarder_StartDisappointed` 0x11f3f8 named | P2 | trailrig/heron | 2026-10-11 | 67d0990 |
| E5d | The trick book from the disc: `tricky-data/src/trickdef.rs` (`TrickDef_Load` 0x154580's file, `Score_FormatTrickName` 0x1551c0 from its disassembly with the table region 0x320970-0x320f58), read by the app's book with `TRICKY_DISC`; all 360 names equal the export | P1 | ed-slopper/puffin | 2026-10-11 | 1e83035 |
| E5e | Fonts from the disc: `tricky-data/src/sfn.rs` (`Font_ParseGlyphs` 0x19c910, `Font_BuildTexture` 0x19cca8, the scales `cGame::Init` 0x17e148 sets), read by the app with `TRICKY_DISC`; glyphs equal the export | P1 | ed-slopper/puffin | 2026-10-11 | f59e63f |
| G1c2 | The start gate ported exactly (`tricky-rs/game/src/gate.rs`): `GateAnticipateControl_Enter` 0x105f48, `_Update` 0x105f58, `_Exit` 0x106140, `GateLaunchControl_Enter` 0x106228, `_Update` 0x1062b0, checked in the function runner (`tests/gate.rs`: 20,000 random rocks and launches, every field and call bit for bit); the launch-speed formula in race.md corrected (still is seconds, not ×60) | P2 | trailrig/heron | 2026-10-11 | 9683178 |
| E8g | `Boarder_ProbeGround` 0x128ae8 and `Boarder_GetFallLine` 0x128a18 ported exactly (`tricky-rs/game/src/probe.rs`) and checked in the function runner (`tests/probe.rs`: 20,000 probes, hits, misses, flat and near-vertical ground; the cast line and every field bit for bit, the world ray cast answered by the test) | P2 | trailrig/heron | 2026-10-11 | 870944f |
| E5c | Camera files from the disc: `Cml::from_disc` loads the course's, `commonob` and `scripts` as `Camera_LoadCmlFiles` 0x1783e8 does (`Camera_GetCourseCmlName` 0x178330 ported), read by the app with `TRICKY_DISC`; equal to the level folders' copies | P1 | ed-slopper/puffin | 2026-10-11 | 2586fdd |
| E8h | `GroundMotion_Update` 0x10a0d8 ported exactly (`tricky-rs/game/src/ground_motion.rs`: forces, lean, sink, Euler step, bob, yaw, wall mask, probe, bleed, take-off / crash / tilt / wobble, in the machine code's order) and checked as a whole in the function runner on 20,000 random ticks (`tests/ground_motion.rs`): the four forces, `Math_SinCos`/`Asin`/`Cos`, `Vec4_Scale`, `Boarder_ProbeGround` and `Boarder_GetFallLine` run as the original (the world ray cast answered by the test), the other callees recorded; every field it writes and every call with its arguments, in order, bit for bit; exact-tie cases and a one-bit planted error are caught | P1 | trailrig/heron | 2026-10-11 | 50e74ef |
| E8j | `Boarder_RotateAboutAxis` 0x128e90 and `Boarder_TakeOff` 0x128470 ported exactly and checked, and the rider's virtual 0x64 that `GroundMotion_Update` calls on a crash (surface 6 or 10) identified, so `ground_motion::update` can run them instead of the test recording them; also what rider+0x1a0 is (the board's up, inferred) | P1 | trailrig/heron | 2026-10-11 | 9a73a17 |
| E8k | Run the exact ground tick in the port: `Rider::step` calls `ground_motion::update` (cm, z up) through `tricky-rs/game/src/rider/ground_tick.rs`, with the port's walls, ray cast, `orient::take_off` (+ `jump::jump_impulse`), crash (go_down; spray recorded in `crash_spray`) and revert as `Env`; rail capture and instance collisions stay out. Riding tests in `game/tests/physics.rs`; `cargo test --release` in `tricky-rs/game`, `function_index.py names`, `check_no_game_files.py` pass; `TRICKY_SIM` not run (no level export) | P1 | trailrig/heron | 2026-10-11 | cb5a3d7 |
| G20a | The ground tick's stat inputs exact: the 16 stat bytes (stats+0x0e..0x1d) as `Career_SetupPlayerRider` 0x16a9d8 / `Career_SetupAIRider` 0x16ae68 build them (`tricky-rs/game/src/stats.rs`: the rider's base bytes, four groups mixed with the board and a level, `b·255/100` in integers; `Career_AverageStatGroups` 0x16b298), grip +0x13 and the cubic term +0x19 now the game's (they are the edging and speed group bytes), the jump stat 178 not 179; `Stats::of` carries them, the ground tick and the jump read them; the original three run in the function runner, 20,000 calls each with cheats and modes, bytes bit for bit, and the tables (group map, bases, caps, 144 board bonuses, trickdata) equal the exe's (`tests/stats.rs`); the runner learnt `divu`; 5 data names | P2 | trailrig/heron | 2026-10-11 | 3d6af9f |
| G20b | The probe's sideways offset exact: stats+0x60 = the rider's body scale (`gRider_Table` row +0x24, 0.8 for the player under the all-max word; `stats::SCALE`), rider+0x244 = the value of a lean record (+0x248 rate, +0x24c target): eased by `cBoarder::vf7` 0x117198, aimed by `Boarder_UpdateLeanTargets` 0x129500 (−60·steer, speed knee, 0 on the rail), zeroed by `Boarder_PlaceAt`, negated by the fakie revert; `Rider::lean`, `lean_ease`, `lean_targets`, `Stats::scale`, and the ground tick's probe takes `scale · lean`; the scale of both set-ups, the 12-value table, vf7 whole (the ease) and `Boarder_UpdateLeanTargets` run in the function runner, 20,000 calls each, bit for bit (`tests/stats.rs`); all writers of +0x244 found and listed in the port notes; 1 data name (`EaseRecord_Init`) | P2 | trailrig/heron | 2026-10-11 | 5359c19 |
| G20c | The rider's up (rider+0x2c0) exact: its two writers found (`Boarder_UpdateLeanTargets` 0x129500 every tick, up = unit(up + 0.5 · normal); `Boarder_PlaceAt` 0x11cae8, up = normal) and its readers listed in the port notes; `Rider::up` kept on the rider, `lean_up` at the end of every tick, set by new / respawn, given to the ground tick as `g.up` (it was the board's up); `lean_up` and `Rider::lean_aim` run against `Boarder_UpdateLeanTargets` in the function runner, 2,000 chains of 12 ticks, bit for bit (`tests/stats.rs`); `Boarder_UpdateLeanTargets` set `checked` | P2 | trailrig/heron | 2026-10-11 | 145cb92 |
| G20c2 | The normal (rider+0x2a0) on a rail and at a respawn as the game writes it: `RailSlideMotion_Update` 0x10b0d0 (store 0x10bbb8; lean angle, flat and steep) and `Boarder_PlaceAt` 0x11cae8 (default (0, 0, 1, 0)) ported, `Rider::rail_lean`, port notes; checked bit for bit against the real functions in the runner (`tests/normal.rs`) | P2 | trailrig/heron | 2026-10-11 | 4117a50 |
| G20c2c | `Rail_ClosestPoint` 0x259330 ported exactly over the game's world grid (`ground_tick::RailWorld::closest_point`: the cell walk, the 300 cm box tests, the four chords and the golden-section search, the last point evaluated, the tangent and owner word) and the rail normal from the game's own inputs (`ground_tick::rail_slide_normal`: closest point, fall-off limits with the blend, travel, lean angle, normal); checked bit for bit against the real `Rail_ClosestPoint` (9,000 queries) and the real `RailSlideMotion_Update` with it (12,000 ticks) in the runner (`tests/rail_world.rs`). Not wired into `Rider::rail_step`: the port has no world grid or attitude through a rail (G20c2c1–G20c2c4) | P2 | trailrig/heron | 2026-10-11 | 8d95af9 |

## Changes to the board

- 2026-10-10 ed-slopper/puffin: board created from the repo, the local tree and the notes.
- 2026-10-10 ed-slopper/puffin: F1 done; added F1a, F1b (gaps of F1).
- 2026-10-10 ed-slopper/puffin: F1a done; F1b rewritten with the library spans found; added F1c (gap of F1a).
- 2026-10-10 ed-slopper/puffin: F1b done; added F1d (gap of F1b); G10 notes the EA sound library; AGENTS.md §14 had the symbols.txt line format wrong, fixed.
- 2026-10-10 ed-slopper/puffin: F1d done; E2a, E2b, E3b, E5 notes get the game's own functions for them.
- 2026-10-10 ed-slopper/puffin: F1d's Done row pointed at the claim commit; the work is 1ada4af (committed with the message "done F1d: commit hash on the board" after a broken command chain).
- 2026-10-10 ed-slopper/puffin: F2 done; added F2b (gap of F2).
- 2026-10-10 ed-slopper/puffin: F2a done; F2b also asks to prove F2's class methods in Ghidra.
- 2026-10-10 ed-slopper/puffin: F4 done; added F4c (humans: float exactness) and E8e (drag gaps found by F4); F4a, F4b notes point at the design.
- 2026-10-10 ed-slopper/puffin: F4a done; added F4d (compare tricky-rs with the runner) and F4e (runner coverage).
- 2026-10-10 ed-slopper/puffin, asked by ed-slopper: added F3b (install PCSX2-MCP), F4f (runner against the running game); F4b now traces through PCSX2-MCP breakpoints and needs F3b; AGENTS.md §14 says when to use it.
- 2026-10-10 ed-slopper/puffin: F3b done (set up by ed-slopper); claim F4f.
- 2026-10-10 ed-slopper/puffin: F4f done; added F4g (the last ulp); F4c gets the measurement; AGENTS.md §14: keep PCSX2 breakpoints few.
- 2026-10-10 ed-slopper/puffin: F4e done; added F4h (check the F4e targets in a race).
- 2026-10-10 ed-slopper/puffin: F4g released with findings (first divergence at 0x109d6c; best rule 111/159 exact); F4c gets the PCSX2-vs-console question sharpened.
- 2026-10-10 ed-slopper/puffin, asked by ed-slopper: the running game is read over PINE (stock PCSX2), not PCSX2-MCP (AGENTS.md §14); F4b no longer needs F3b, F4h is per tick over PINE, F4g continues offline on the saved captures.
- 2026-10-10 ed-slopper/puffin: F5 done; added F5a (strict clippy on tricky-rs).
- 2026-10-10 ed-slopper/puffin: F4b split: F4b1 done (map and recorder), F4b2 (pad state) and F4b3 (replay test) open.
- 2026-10-10 ed-slopper/puffin: F4b2 done (pad layout checked live).
- 2026-10-10 ed-slopper/puffin: split F11 into F11a (riding physics crate, claimed) and F11b.
- 2026-10-10 ed-slopper/puffin: F11a done (CI green); F4b3 and F4d now wait only on their other rows.
- 2026-10-10 ed-slopper/puffin: E8e and F4d done (2 functions checked); added F4d2 (the other leaves).
- 2026-10-10 ed-slopper/puffin: F4d2 done (5 functions checked).
- 2026-10-10 ed-slopper/puffin: F4h done (air step and rubber band per tick); added F4h2 (takeoff, jump, prediction, steep lip).
- 2026-10-10 ed-slopper/puffin: F4g released again with findings (normal play rounds to nearest).
- 2026-10-10 ed-slopper/puffin: F5a done (clippy strict).
- 2026-10-10 ed-slopper/puffin: F4h2 part done (takeoffs within a tick's forces; air step on a third recording), left open.
- 2026-10-10 ed-slopper/puffin: F6 done (port notes per system).
- 2026-10-10 ed-slopper/puffin: F8 done (stand-ins tagged).
- 2026-10-10 ed-slopper/puffin: F9 done (addresses on recreated functions).
- 2026-10-10 ed-slopper/puffin: F10 done (duplicate tools removed).
- 2026-10-10 ed-slopper/puffin: F11b done (tricky-data; race, AI and props in tricky-game).
- 2026-10-10 ed-slopper/puffin: F3 done (ghidra-mcp connected); F1c and F2b no longer wait on it.
- 2026-10-10 ed-slopper/puffin: F2b done (first row with ghidra-mcp).
- 2026-10-10 ed-slopper/puffin: F1c in progress (still claimed): boarder done (state machine, snapshots, controls), race and world mostly done, plus class-slot passes (destructors, node Update/Draw/SaveSnapshot, handlers Enter/Exit/Update, FE states, overlays, loading screens, menu items, course loader), RNG, string scan. ~1,470 names; index 3,047/7,426 named. Left: big race/world/camera routines, renderer state setters, frontend screen builders, audio/speech loaders.
- 2026-10-11 trailrig/heron: trailrig added to Areas (AGENTS.md §12); split E8 into E8f–E8i.
- 2026-10-11 trailrig/heron: claim E8f.
- 2026-10-11 trailrig/heron: E8f done; added E13 and G15 (gaps of E8f).
- 2026-10-11 trailrig/heron: split E1a into E1a1 (claimed) and E1a2.
- 2026-10-11 ed-slopper/puffin: F1c in progress (still claimed): 5,201 names (index 4,382/7,426). Added since: audio/speech (85 speech events by id, banks, streams, emitters, crowd), sphere-tree collision and the sweep-and-prune broadphase, EA Huffman encoder, rider skin/mpf loader (notes/characters.md), GS state packets, AI ride modes, pause menu pages, animation clip modes (port-notes/boarder.md). Left: PS2 memcard driver states, renderer mesh/swizzle, replay control drawing, HUD layout tables, ~30 large unnamed maths routines.
- 2026-10-11 ed-slopper/puffin: claim E2 (on the way to E4, asked by ed-slopper); F1c paused, still claimed.
- 2026-10-11 ed-slopper/puffin: E2 done.
- 2026-10-11 ed-slopper/puffin: claim E2a.
- 2026-10-11 ed-slopper/puffin: E2a done; claim E2b.
- 2026-10-11 trailrig/heron: claim E13.
- 2026-10-11 ed-slopper/puffin: E2b done; added E2c (gap of E2b); split E4 into E4a-E4c; claim E4a.
- 2026-10-11 trailrig/heron: E13 done; G15 corrected (which surfaces spray); added G16 (body height, gap of E13).
- 2026-10-11 trailrig/heron: split E1b into E1b1 (claimed) and E1b2.
- 2026-10-11 trailrig/heron: E1b1 done (6 functions checked).
- 2026-10-11 trailrig/heron: split G4a into G4a1 (claimed) and G4a2.
- 2026-10-11 trailrig/heron: G4a1 done; G4a2 gets the table's word +8.
- 2026-10-11 trailrig/heron: claim E8i.
- 2026-10-11 trailrig/heron: E8i done; added G17 (gap of E8i).
- 2026-10-11 trailrig/heron: claim G17.
- 2026-10-11 trailrig/heron: G17 done.
- 2026-10-11 ed-slopper/puffin: E4a done; added E4d (gap of E4a).
- 2026-10-11 trailrig/heron: claim E9a.
- 2026-10-11 trailrig/heron: E9a done; added E9b–E9j.
- 2026-10-11 trailrig/heron: split G1a into G1a1 (claimed) and G1a2.
- 2026-10-11 trailrig/heron: G1a1 done (Jump_ApplyImpulse checked).
- 2026-10-11 ed-slopper/puffin: claim E4b.
- 2026-10-11 trailrig/heron: G1a2 split into G1a2 (claimed) and G1a3.
- 2026-10-11 trailrig/heron: G1a2 done (Takeoff_SetSpinRates checked).
- 2026-10-11 trailrig/heron: split G2 into G2a (claimed) and G2b.
- 2026-10-11 trailrig/heron: G2a done (8 functions checked).
- 2026-10-11 trailrig/heron: added E14 (the math library), claimed.
- 2026-10-11 trailrig/heron: E14 done (7 functions checked).
- 2026-10-11 trailrig/heron: split E8a into E8a1 (claimed) and E8a2.
- 2026-10-11 ed-slopper/puffin: E4b done.
- 2026-10-11 trailrig/heron: E8a1 done (Landing_CheckAngles checked).
- 2026-10-11 trailrig/heron: E1a1 released (code on its branch, needs a level run).
- 2026-10-11 trailrig/heron: E8a2 split into E8a2 (claimed) and E8a3.
- 2026-10-11 trailrig/heron: E8a2 done (Landing_ChooseState checked).
- 2026-10-11 trailrig/heron: G1a3 split into G1a3 (claimed) and G1a4.
- 2026-10-11 trailrig/heron: G1a3 done (SpinState_StartFromStick checked).
- 2026-10-11 trailrig/heron: added G1d (boarder meter and input helpers), claimed.
- 2026-10-11 ed-slopper/puffin: F7 notes: the Xbox build (OpenTricky) as a possible second reference, for the humans to decide; race.md notes its rank catch-up bug lead.
- 2026-10-11 ed-slopper/puffin: claim E3b.
- 2026-10-11 trailrig/heron: G1d done (4 functions checked; runner tail-call fix).
- 2026-10-11 trailrig/heron: added G1e (more boarder input helpers), claimed.
- 2026-10-11 trailrig/heron: G1e done (3 functions checked).
- 2026-10-11 trailrig/heron: G1a4 split into G1a4 (claimed) and G1a5.
- 2026-10-11 ed-slopper/puffin: E3b done; claim E4c.
- 2026-10-11 trailrig/heron: G1a4 done (PrewindState_Update checked).
- 2026-10-11 ed-slopper/puffin: E4c done.
- 2026-10-11 trailrig/heron: split G1c into G1c1 (claimed) and G1c2.
- 2026-10-11 ed-slopper/puffin: split E5 into E5a-E5e; claim E5a.
- 2026-10-11 ed-slopper/puffin: E5a done; added E5a2 (gap of E5a).
- 2026-10-11 ed-slopper/puffin: claim E5d.
- 2026-10-11 trailrig/heron: G1c1 done (finish state and brake helpers checked).
- 2026-10-11 trailrig/heron: G1c2 split into G1c2 (claimed, the start gate) and G1c3.
- 2026-10-11 ed-slopper/puffin: E5d done.
- 2026-10-11 ed-slopper/puffin: claim E5e.
- 2026-10-11 ed-slopper/puffin: E5e done.
- 2026-10-11 trailrig/heron: G1c2 done.
- 2026-10-11 ed-slopper/puffin: claim E5c.
- 2026-10-11 ed-slopper/puffin: E5c done.
- 2026-10-11 trailrig/heron: claim E8g.
- 2026-10-11 trailrig/heron: E8g done.
- 2026-10-11 ed-slopper/puffin: F1c resumed (still claimed): ~1,220 names this round (index 5,702/7,426 named; frontend text named by the string ids it shows, from CONSTANT.LOC): rivals grudge system, race menu callbacks, rider streaming, replay keyframes and pins, lesson pages, circuit setup, char select outfits, camera modes, AI control modes, overlay constructors, audio streams/songs/speech cues, Cstring ops, cdvd/pad/mc SDK calls, video stream manager, bank slots, crowd boxes, song fades, stream slots, course hash lookups, FE input keys, FE3D widget ctors, Cstring/SpeechLib/File/Strm/FileSys/EAMem/thread/timer/SND stream-voice-MIDI library code, libgcc soft-float, newlib (malloc, stdio, strtod, mprec), Sony libdma/libpad/libsif/deci2/kernel wrappers (the names check now accepts Sony and libc spellings (sce*, isce*, DisableIntc, _Balloc) for host functions). Left: boarder render effect buffers, memcard overlay states, FE3D list ops, ~2,650 small or chained functions.
- 2026-10-11 ed-slopper/puffin: F1c stopped (released): 5,702/7,426 named; claimed G1a5.
- 2026-10-11 ed-slopper/puffin: G1a5 split: G1a5a done (SpinState_Update exact, checked); G1a5b (wiring into Rider::step) open.
- 2026-10-11 ed-slopper/puffin: G2b done (land, chain start, bank, trick id, names checked); split off G2c (uber/TRICKY) and G2d (wiring into Rider::step), both open.
- 2026-10-11 ed-slopper/puffin: claim G2c.

- 2026-10-11 ed-slopper/puffin: G2c done (meter drain, grab end/uber count checked; TRICKY = score+0x88 > 5).- 2026-10-11 ed-slopper/puffin: claim G2d.

- 2026-10-11 ed-slopper/puffin: G2d done (rider scores through score.rs; wants a play test).
- 2026-10-11 ed-slopper/puffin: claim E1a1 (run the branch on a level, merge).
- 2026-10-11 ed-slopper/puffin: E1a1 done (branch run on Garibaldi and merged, 5b51fa9).
- 2026-10-11 trailrig/heron: claim E8h.
- 2026-10-11 trailrig/heron: E8h done; added E8j and E8k (gaps of E8h).
- 2026-10-11 trailrig/heron: claim E8j.
- 2026-10-11 trailrig/heron: E8j done; added G18, G19 (gaps of E8j).

- 2026-10-11 trailrig/heron: claim E8k.
- 2026-10-11 trailrig/heron: E8k done; added E8l, G20 (gaps of E8k).
- 2026-10-11 trailrig/heron: claim E8l.
- 2026-10-11 trailrig/heron: E8l released unrun (no level export on the PC); split G20 into G20a–G20c, claim G20a.
- 2026-10-11 trailrig/heron: G20a done (stat bytes exact, 3 functions run in the runner, `Career_AverageStatGroups` checked); added G20a2 (gap of G20a).
- 2026-10-11 trailrig/heron: claim G20b.
- 2026-10-11 trailrig/heron: G20b done (probe offset exact: body scale + lean record, checked in the runner); added G20b2 (gap of G20b), noted the +0x2c0 half in G20c.
- 2026-10-11 trailrig/heron: claim G20c.

- 2026-10-11 ed-slopper/puffin: claim G1a5b.
- 2026-10-11 ed-slopper/puffin: G1a5b split into G1a5b1 (GetControls, claimed), G1a5b2 (pad mapper), G1a5b3 (wiring).
- 2026-10-11 ed-slopper/puffin: G1a5b1 done (GetControls exact, checked).
- 2026-10-11 ed-slopper/puffin: claim G1a5b2.
- 2026-10-11 trailrig/heron: G20c done (the rider's up exact, checked in the runner); added G20c2, G20c3 (gaps of G20c).
- 2026-10-11 trailrig/heron: G20c2 done (rail and placement normal exact, checked in the runner); added G20c2b (air normal and placement ray, E7) and G20c2c (rail inputs from the game).
- 2026-10-11 ed-slopper/puffin: G1a5b2 done (pad mapper, both button maps, trick table; checked).

- 2026-10-11 ed-slopper/puffin: G1a5b3 split: G1a5b3a done (jumps through jump_impulse), G1a5b3b open (pad word, prewind, spin; needs G1's state numbers). Note: 600 s AI runs show more wipe-outs since the E8h–G20c ground work (Alaska 10 → 20, Garibaldi 3 → 6) — not from G1a5b3a.
- 2026-10-11 ed-slopper/puffin: claim E8d.
- 2026-10-11 ed-slopper/puffin: E8d split: E8d1 done (RiderHit exact, checked; bumps and shoves through it), E8d2 open.
- 2026-10-11 ed-slopper/puffin: claim E8d2.
- 2026-10-11 ed-slopper/puffin: E8d2 done (collision body, pair records, shove exact and checked); E8d3 open (wiring: needs bones).
- 2026-10-11 ed-slopper/puffin: claim G4a2.
- 2026-10-11 ed-slopper/puffin: G4a2 done (grudge bookkeeping exact, checked; word +8 = attitude cap); G4a3 open (wiring).
- 2026-10-11 ed-slopper/puffin: claim G4a3.
- 2026-10-11 ed-slopper/puffin: G4a3 done (app grudges on the exact bookkeeping).
- 2026-10-11 ed-slopper/puffin: G4 split off G4b (AI target and steering helpers), claimed.
- 2026-10-11 ed-slopper/puffin: G4b done (AI target and steering helpers exact, checked).
- 2026-10-11 ed-slopper/puffin: G4 split off G4c (AI riding modes), claimed.
- 2026-10-11 ed-slopper/puffin: G4c done (AI shove decision exact, checked, wired); riding modes moved to G4d (open).
- 2026-10-11 trailrig/heron: E8l released unrun again (no level export; SSX-Library on the PC is source only, no CLI); claim G20c2c.

- 2026-10-11 ed-slopper/puffin: claim G4d.
- 2026-10-11 ed-slopper/puffin: G4d done (AI riding modes exact, checked); G4e open (wiring, needs E3d's markers).
- 2026-10-11 ed-slopper/puffin: claim E3d (the .aip/.sop files as the game reads them).
- 2026-10-11 ed-slopper/puffin: E3d done (.aip/.sop exact, checked against the game's loader on every course; app reads them from the disc).
- 2026-10-11 ed-slopper/puffin: G4e course queries done (36acf17); add and claim G4f (path following, before G4e's wiring).- 2026-10-11 ed-slopper/puffin: correction: rider+0x418 is the finish time (Boarder_Finish 0x11da40; −1 while racing), not 'human vs AI': scoring and the meter drain now stop at the finish for every rider (were: never for AI riders); notes fixed.

- 2026-10-11 ed-slopper/puffin: G4f done (AI path geometry and path following exact, checked on every course).
- 2026-10-11 ed-slopper/puffin: claim G1c3.
- 2026-10-11 trailrig/heron: G20c2c done (`Rail_ClosestPoint` and the rail normal from the game's inputs exact, checked in the runner; not wired); added G20c2c1–G20c2c4 (gaps of G20c2c: world grid, rail attitude, wiring, the rest of `RailSlideMotion_Update`); E8l note.
