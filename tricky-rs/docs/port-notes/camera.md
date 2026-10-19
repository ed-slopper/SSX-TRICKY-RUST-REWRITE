# Cameras: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Chase camera (`cBxCamera_EvalNamedCam` 0x174888)

- CML record 0x194 bytes at name−0x10; body = name+0x10 is the camera object (position node 0x120, target node).
- Modes: 1 reverse, 2 board, 3 eyes, 4 near, 5 far, 6 over; the user cycles 0..3 → board, near, far, over.
- | cam | D cm | base pitch | slope div down/up | pos lag | look z | pitch off | FOV | look lag | roll div |
  | far | 500 | −0.70 | 2/1.5 | 15 | 100 | +0.03 | 1.65 | 5 | 8 |
  | near | 180+30·r130 | −0.50 | 2/1 | 16 | 100 (+35 base z) | −0.16 | 1.65 | 5 | 4 |
  | board | 150 | −0.50 | 1.5/1 | 6 | 0 (bone 0) | +0.27 | 1.20 | 4 | 2 |
  | over | 1000 | −0.85 | 2/2 | 15 | 100 | +0.15 | 1.65 | 25 | 4 (off) |
- Direction d = norm(0.95d + 0.05·norm(v)) per tick (board forward under 10 cm/s; 0.99/0.01 while tumbling).
  e = atan2(d.z,|d.xy|); p_t = base + e/div; p += (p_t−p)/lag; yaw += wrap(y_t−yaw)/lag;
  C = P + D(cos p sin y, −cos p cos y, −sin p) (behind and above). Collide pivot→camera, push 40 cm off.
- Wipe-out blend b: +0.0291667/tick to 0.98 in motion 5/6, else −0.003/tick; C = C_old + (C_new−C_old)(1−b).
- Look at T = P+(0,0,100): angles from C to T plus pitch offset; roll on the ground = ±acos(z⊥·u)/div.
  Output: Ps = 0.8Ps + 0.2C per tick; heading/pitch += Δ/L (L = look lag, or 10+20b while b>0); roll += Δ/25.
- Landing dip: hard landings (>1000 cm/s into the snow) drop T.z by ≤200 cm for 5 ticks.

## Camera scripts (CML)

- Regions in the track CML files are replay-only (live-race flag 0) on Garibaldi and Snowdream; in a race the CML
  only drives the intro: PreLoad → fly-through (50/50 of two) → staging scene → gate shot (5 s) → chase camera.
- Director script ops: 0 cut, 1/0x1C linear blend, 2/3 orbit blend, 5/6 chase near/far, 0xA random branch,
  0xB call, 0xD back to chase, 0xE scene, 0x20 end. Blend curves 0 linear, 1–3 Hermite variants.

## Replay (`Replay_*`, gApp+0x730 → +0x40)

- Recording keeps a linked list of keyframes (+0x10 next, +0x14 prev, +0x20 frame, +0x18 pin refs, +0x1c keep);
  one every 60 frames (`Replay_Update` 0x191298), with an input stream per player (`ReplayInput_*`, 0x800c bytes,
  run-length frame bytes at +0x6664, count +0x8000). Full keyframe pool → `Replay_EvictKeyframe` drops the oldest
  unpinned one.
- Highlights: per rider up to 10 clips (+0x394c count, +0x3dc8 list); a clip is [keyframe, length, …]. Pins
  (`ReplayPin_*`, 4 slots at +0x3920) hold the best clips by two scores.
  `ReplayPin_GetQuota` 0x193250: how many pins a kind may hold (kind 1 +0x3944, kind 2 +0x3948, else 0);
  `ReplayPin_PickSlot` 0x1935d8 takes a free slot, else, while this kind is below its quota − 1, the other kind's worst slot when that kind is at or over its quota;
  at or over it, its own worst slot if the new clip scores at least as well.
- Playback state +0x20 (1 play, 2 marking a highlight, 3 paused, 4–8 stepping, 9 start, 0xb reset world, 0xd idle); command +0x28
  handled by `Replay_DoCommand` (1 rewind, 2 to end, 3 step back, 4 step forward, 5 play, 6 toggle pause, 7 pause,
  8 mark highlight, 9/10 restart). A playlist (+0x391c, `ReplayPlaylist_*`) plays clips back to back.
- Camera tracks: one per rider (0xe24 bytes at +0x5c), up to 128 cuts {preset, chase params, frame};
  `ReplayCamTrack_Generate` records them from a dry run with `Camera_SetReplayMode(1, rider)`.
- Saving (`ReplaySave_Step`, 17 stages, 0x4000-byte blocks, byte-sum checksum): header 0x12300003 (0x44 bytes),
  camera tracks, keyframes (0x11111113 + compressed body), highlights (0x11111115), input streams (0x11111112,
  compressed), pins (0x11111114), zero padding to 0x7f6d8 bytes, then the 8-byte checksum/length tail.

## Presets, chase parameters, modes and focus (E11g)

`tricky_game::camera_ctl`, on the game's bytes. It covers the race camera at 0x3368b0 and the globals around it:
0x3368a0 the preset, 0x337770 the chase camera's rider, 0x337774 the replay's camera flag, 0x337788 the second
camera.

- **Named cameras:** `CamCamera` finds a block by name (`Cml_FindObject`). `GetView` gives the block, `GetLens` its
  +0x120. Each stores its own address in that part (+0x11c, the lens's +0x44).
  - So when `Camera_SelectPreset` copies a preset's view (0x120 bytes) and lens (0x48 bytes) into "replay cam", the
    copy's self pointers point at the preset's block until the next `GetView` / `GetLens`.
- **Presets** (the table at 0x336608, up to id 13): 1 chase reverse, 2 board, 3 eyes, 4 near, 5 far, 6 over, 8
  "replay start". 7 and anything outside 1..8 take "replay start" too, by another copy of the string (0x383910).
  - Selecting a preset sets it as the main camera's mode, stops its script, copies the preset, and cuts.
  - `Camera_NextPreset` goes 1..8 and skips 3.
- **Chase parameters,** in "replay cam": +0xdc the heading, +0x100 the pitch, +0xcc the distance, the lens's +0x20
  the height ratio, plus the rider's index.
  - Setting them clamps the pitch to ±1 and the distance to 200..1200. The height ratio is raised to 300 / distance
    when it would put the camera under 300, then clamped to 0.5..1.9 (0x3ff33333).
  - Then the main camera's focus is set (`Camera_SetFocusRider`) and the index kept.
- **`Camera_AdjustChase`** (step, which):
  - 0 / 1 the heading and 2 / 3 the pitch, by 2° × step (0x3d0efa35);
  - 4 / 5 the next or previous rider, wrapping over the race's riders, with a cut of both cameras;
  - 6 / 7 the height ratio by step / 180;
  - 8 / 9 the distance by 5 × step.

  It also calls `Math_AngleDiff(heading, 0)` and drops the result.
- **Modes:** the camera's +0x428 is 2, 4, 5 or 6 for the pad's index 0..3 at +0. Anything else maps to index 1 /
  mode 4. Mode 3 is looking back, which keeps the previous mode at +0x14. Look-back does nothing while a script runs
  (+0x34) or the replay's camera is on. A change of mode asks for a cut (+4, +8).
- **Focus** (`Camera_SetFocusRider`): another camera first sets the main one's.
  - Slot 0 (+0x4e8) is the focus. Slots 1.. hold the other riders in race order. Slots 10 / 11 hold the riders
    before and after the focus. Slots 14..25 hold each rider by its character (+0x464 → +0x38).
  - `Camera_RiderSlotIndex` gives a slot's race index (0 when none). `Camera_IsFocusRider` compares a rider's +0x460
    with the chase index.
- **`Camera_Reset`:**
  - applies the mode index;
  - clears the state: +0x14 back to 4, +0x444 to 1.5;
  - copies the first record (+0x42c..) into the second (+0x488..) in the game's order. Some of the source words were
    cleared just before, so their copies are 0.

  `CamControl_Reset` sets the index to 1 first.
- **`Camera_UpdateRiderTracks`:**
  - keeps the 26 slots as last frame's (+0x340 on);
  - tracks each rider (`Camera_TrackRider`);
  - puts the focus and its +0x57c0 point in slot 8;
  - resets slots 4..19 from their riders (`Camera_ResetRiderTrack`: +0x4ad0.. the position, +0x1b0);
  - keeps the slots as the cut's (+0x680 on) while the camera's +0xc is below 3.
- **Checked in the runner** on 45,000 random cases over the 22 functions. The main camera and a second one, the
  named cameras' blocks, riders and characters were random, with every call (find, cut, script stop, rider, track,
  cut-to) and all memory compared. The tests caught 11 of 12 mutants; the one left (`<` vs `<=` at exactly 300)
  recomputes the same ratio.

## The camera's frame and the game's cameras (E11h)

`tricky_game::camera_ctl` (`CamFrameEnv`, `CamLifeEnv`).

- **`cBxCamera::Update`** (vf1, 0x177078), each frame:
  1. The rider tracks are updated.
  2. **At a cut** (+0xea8 1, +0xeb0 clear) the region scan runs. It goes through the camera files 0x336688,
     0x336690 and 0x336698; in each, the first region of kind 4 that passes all of these starts a script:
     - the region is on (+0x164; or, with the replay's camera, +0x168 while the mode is 7);
     - its rider (slot +0x154) isn't held back. Each of these holds it: +0x16c with the rider's +0x418 negative,
       +0x170 with the rider's +0x114 zero, +0x172 with 0x11f488 false. A missing rider isn't held;
     - its trigger fires (`CamRegion_CheckTrigger` with the slots and last frame's).

     Trigger kind 0 (+0x15c) starts the script named at +0x120, kind 1 (+0x158) the one at +0x12f. Kind 2 (+0x160)
     starts the one at +0x13e, when not already running, unless the camera's script is running under that name
     (`strcmp` with +0x50). Either way, that ends the file's scan.
  3. The clock (+0x10) goes on by 1/60.
  4. The script, its path and the cast's clips are stepped. While a replay is paused or at its end with no cut
     pending, only the cut fade runs instead.
  5. If a script was running and has started another this frame, nothing more.
  6. **The look-back ease** (+0x20), when not in mode 1 or the replay's camera: unless the focus rider's +0x428 is
     0x12, it rises by 0.029 to 0.98 while the rider's motion is 5 or 6 (and +0xeb4 is clear). Otherwise it falls by
     0.003 to 0. +0xeb4 is cleared outside motions 5 and 6.
  7. **The mode's named camera** is cut to when idle:
     - modes 1..6: the preset's (or "replay cam" with the replay's camera);
     - mode 8: "replay cam";
     - mode 10: "manual target", first given the focus point of the camera at 0x3365f0 (its view's +0x3c / +0x68 /
       +0x94, and its lens's +0x20 from the point's seventh word);
     - modes 7, 9 and anything else: "chase far".

     The focus rider is read without a null check, as the game does.
- **`Game_CreateCameras`:**
  - one camera for one player, two for two or three (+0x298, copied to +0x294);
  - the camera control (12 bytes, `cCamControl` with 9) at +0x84;
  - each camera: allocated (0xec0 bytes, zeroed) and set up (`BxCamera_Init`); the players' saved mode indexes
    (0x32ca38 / 0x32ca3c) set; registered (0x337780 counts the list at 0x337788);
  - each camera given its rider. A player's camera takes the rider with the smallest player slot (below 4) above
    the previous camera's. The others count back from the end of the race's list at +0xc4;
  - the rider's +0x38 set to the camera's number, and the camera focused;
  - then the viewports are laid out.
  - When the first camera finds no player slot, the game stores whatever a register held. The port uses 0
    (STANDIN), and the test keeps every player's slot in 0..3.
- **`Game_AssignCameraRiders`** swaps the viewport chosen at +0x68 into the first. That is the 0x7c bytes from +0x90,
  less +0xa8 / +0xac. It then numbers and focuses each camera's rider the same way, without the player-slot search,
  and clears +0x68.
- **`BxCamera_Init`:**
  - sets the vtables: the camera 0x3830a8, its path 0x3814c0, its scene 0x382180, the eight animations 0x37ee70;
  - sets both records' 1.6 and 100.0 (+0x444 / +0x448, +0x49c / +0x4a0);
  - clears the three slot sets' trackers and the globals 0x3365f4..0x336600;
  - then runs `CamControl_Reset`.
- **The rest:**
  - `Game_DeleteCameras` runs each camera's and the control's virtual destructor with 3.
  - `Game_CutAllCameras`, `Game_GetCamera` (0 past +0x294) and `Game_RegisterCamera` do what their names say.
  - 0x1739f0 sets the focus record from the main AI path's start.
  - `Game_UpdateCamerasStep` updates the camera nodes (`BxList_UpdateAll` 0x337c60, 9) and the cameras, then
    counts the frame (+0x50).
- **Checked in the runner** on 39,000 cases: 20,000 frames over random cameras, regions, riders and replay states,
  plus the set-up, teardown, assignment and helpers. Every call and all memory were compared. The tests caught 10 of
  11 mutants; the one left (`<` vs `<=` at the 0.98 cap) writes the same value.
- **Left:** `Game_ResetCameras` 0x180b58 (a race reset beyond the cameras), the destructors and the CML loading.

## The camera files' lookups (E11i)

`tricky_game::cml` follows the game's own lookups on the loaded files. `tricky_data::intro`'s `Cml::find` (a
case-insensitive walk of a kind's list) stays for the app's intro until it moves to these.

- **A handle** is a word holding a loaded file's base, or 0. The game keeps three: the course's (0x336688),
  `commonob` (0x336690) and `scripts` (0x336698). All offsets are from the base (`Cml_ResolveOffset`).
- **The file's +4 + 4 × kind** is its first record of the kind (-1 none). Kind 6 (+0x1c) is the root of the node and
  scope trees.
- **A record** is a 0x20-byte header: +0 the next of the kind, +8 the body, +0xc the name's hash, +0x10 the name.
  `CmlFile_FirstOfType` / `_NextOfType` give bodies; the next is read from the header at body − 0x20.
- **`Cml_HashName`:** for each character, signed, the hash becomes (hash << 23) ^ (hash >> 7, arithmetic) ^ the
  character. An optional limit caps the count.
- **Trees** are searched depth first, left, right, then the node:
  - a node by its hash at +0x10;
  - a scope by its hash at +0xc and its name at +0x24.

  Neither search checks for a missing file.
- **A scope's objects** are listed from its +8: each has +4 the next, +8 the body, +0xc the hash, +0x10 the name.
- **`Cml_FindObject`** (name, kind) tries the three files in order. `Cml_FindObjectInScope` (name, scope, kind) tries
  each file's scope, then falls back to `Cml_FindObject` with the kind.
- **`Cml_FreeHandle`** 0x179068 (F1c, proven): `Mem_Delete` of the handle's word; `~cCamControl` calls it on the three.
- **Setters** (F1c, proven): `CamView_SetTarget` 0x1703a0 writes x, y, z to the view's +0x3c / +0x68 / +0x94 (through its
  self pointer +0x11c); `CamLens_SetHeight` 0x1730c0 writes +0x14..+0x1c and the height ratio +0x20 (through +0x44).
  They are the "manual target" set-up's two calls (0x1775ac, 0x1775c4).
- **Checked in the runner:** 20,000 cases of three random files, one sometimes missing. They had random records,
  trees and scope lists, and names with right and wrong hashes, one with high bytes. Every lookup's result was
  compared. The game's own `strcmp` runs too; the runner learnt its `PSUBW` / `PSUBB` / `PNOR` / `PAND` for it.
  The tests caught the 8 mutants tried.

## Names from F1c batch 17 (proven)

- `cCameraNode::cCameraNode` 0x148218 (from `TriggerOp_SpawnCameraNode`): `cCamAnimFrame` at +0x1c (vtable at
  +0xa8), `cBxTypeObjNode` at +0xac (type 1000), vtable 0x36c6b0 at +0xb4, +0xc4 = `Course_GetCamera`(course,
  record +0x18), then `CamAnim_SetShot`(this, record + 8).
- The vtable 0x36c6b0 (entries of 8 bytes, delta 0xff54 = −0xac): `__tf` 0x1493e8, `~cCameraNode` 0x148478 (was
  listed wrongly as `cCollideEmitterNode::vf12`), then 0x1484c8 and 0x1493c0, which Ghidra has no function at
  (gap: make them functions and name them).
- `CameraNodeState_Destruct` 0x149448: destroys the state's `cCamAnimFrame` (+0x1c); deletes it if flag & 1.
- `Course_GetCamera` 0x254b78: the `.pbd` hash table's camera list (+0x78) by index; `ResolveHashTable` resolves the
  camera list with it.
- Ghidra: the HTTP rename cannot set a namespace, so 0x148218 / 0x148478 and the class methods of this batch carry
  the plain method name in Ghidra; running `ApplySsxSymbols.java` on `ghidra/symbols.txt` moves them into their class.

## Names from F1c batch 26 (proven)

`Camera_FocusFromMainPath` 0x1739f0 (Rust `camera_ctl::focus_from_main_path`): +0x42c..+0x434 from the main AI path's
+0xc..+0x14 (`AIPathSet_GetMainPath` 0x337fb8). No callers in Ghidra.
