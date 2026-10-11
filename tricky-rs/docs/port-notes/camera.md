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
