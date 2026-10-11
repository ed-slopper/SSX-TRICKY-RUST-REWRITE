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
- Playback state +0x20 (1 play, 3 stopped, 4–8 paused/stepping, 9 start, 0xb reset world, 0xd idle); command +0x28
  handled by `Replay_DoCommand` (1 rewind, 2 to end, 3 step back, 4 step forward, 5 play, 6 toggle pause, 7 pause,
  8 stop, 9/10 restart). A playlist (+0x391c, `ReplayPlaylist_*`) plays clips back to back.
- Camera tracks: one per rider (0xe24 bytes at +0x5c), up to 128 cuts {preset, chase params, frame};
  `ReplayCamTrack_Generate` records them from a dry run with `Camera_SetReplayMode(1, rider)`.
- Saving (`ReplaySave_Step`, 17 stages, 0x4000-byte blocks, byte-sum checksum): header 0x12300003 (0x44 bytes),
  camera tracks, keyframes (0x11111113 + compressed body), highlights (0x11111115), input streams (0x11111112,
  compressed), pins (0x11111114), zero padding to 0x7f6d8 bytes, then the 8-byte checksum/length tail.
