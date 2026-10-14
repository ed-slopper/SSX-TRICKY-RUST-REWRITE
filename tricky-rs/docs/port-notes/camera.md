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
