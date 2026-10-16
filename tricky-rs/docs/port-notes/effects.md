# Board and particle effects: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Board effects (`Boarder_UpdateEffects` 0x1350c0)

- Surface spray (`Fx_SurfaceSpray` 0x1311b0), ground: rate = 0.006·s·(1 + 149·steer²)·surfRate per second (s cm/s);
  velocity = v + side·steer·1.1·s, lateral spread 300 cm/s (beyond 450: spread |lat| − 150, lat → mid(lat, ±450)),
  forward halved with full spread; box ±15 cm × ±90 cm; gravity 100 cm/s²; no drag. Air: 70/s ×0.94667 a tick down
  to 10/s, × min(s/277.8, 1); rails 30/s.
- Surface rows (rate, size cm, life s): snow 0.075/3.0/0.136–0.243, off-track 0.159/4.3/0.30–0.45, powder
  0.505/7.85/0.36–0.58, ice 0.2/1.2/0.10–0.27, show-off ramp 1.854/2.56; rock/metal surfaces give sparks instead.
- Brake fan (`Fx_BrakeFan_Update`): |brake| > 0.7: 12 rays ±30–55° off forward at c·(1 − u⁴)·(0.8–1.2),
  c = clamp(0.54·s, 72, 608), up 0.4; ×0.982 a tick; life 1 s; next after 0.25·(1 − 0.000986·c) + U·0.08 s.
- Landing: clamp(0.0009·|vn|, 2, 8) puffs; a 25-point splash ring when |vn| > 549 cm/s. Carve dust, powder puffs,
  brake puffs and a 100-point board track (new point every 110 cm, fading by count) as well.

### The board track in the app (row G7f3b; `src/tracks.rs`)

Each rider's track is now the game's own code. `Fx_BoardTrack` 0x132648 (`boardtrack::board_track`, checked in
G7f) runs every frame on the rider's state:
- the surface, motion and hit kind;
- the board's four corners (`pose::board_corners` on `Rider::board_bone_frame`, row G7f3a);
- the velocity (+0x150), the normal (+0x2a0) and the ground point (+0x2d0);
- the brake (+0x1fc) and the steer (+0x214).

`Fx_BoardTrack_Draw` 0x1331a8 (`board_track_draw`) draws the strips into two meshes, chosen by the blend state
the draw sets:
- state 4, the shade scale −0x40: weights only where the shading is negative, so the side away from the light is
  darkened (black at the weight / 128);
- state 8, the scale 0x80: the lit side, added on.

The meshes sit at the player, because the draw takes the origin quad × 10 from every point. That makes the
vertices relative to the player, as the old mesh's were.

STANDINs:
- the ground point is the rider's place;
- in motions 5 and 6 the two skeleton vectors are the board's middle corners (row G5);
- the light set is a fixed sun with ambient 0.25 and one light of weight 0.5 from up and to one side (the rider's
  lights, row G7b);
- the strip is untextured (the track texture's u and v unused), so the lit pass is added at a fifth.

Seen on gari (autoplay, 34 s in): the player's and the leader's tracks, about 1,200 vertices a pass. The dark
pass's weights reach 0.4 and the lit pass's 0.96, and the strip reads as a groove behind the board. The old
hand-written trail (a point every 110 cm, 5.1°, 10 cm of width, our fade) is gone. `TRICKY_TRACKDBG` prints the
player's strip state and both passes' counts every frame.

## Particle emitters (`Emitter_InitFromRecord`, `PBurst_*`, VU1 microcode at 0x31c288)

- type2Sub0: U0 N particles born evenly over U2 s (U2 < 0: continuous, recycled every T = U5 + U7/2, pre-warmed);
  U1 trail copies (copy j drawn at age − j·U8, alpha × (1 − j/U1)); U3 drag k (1/s); size U4 ± U6/2; life
  U5 ± U7/2; origin U9..11 + u·(U12..14) + u·(U15..17) and velocity U18..20 + u·(U21..23, U24..26, U27..29)
  through the instance matrix (u ∈ [−½, ½)); acceleration U30..32 world; pos(t) = P + W t + (V − W)/k ·
  f(min(k t, 2.7)), W = accel/k, f(x) = 0.73x − 0.113x²; colour (×128) c0 → c1 over T plus random c2, c3
  (keys at U33.., U37.., U41.., U45.., alpha first); U49 texture in particle.ssh (part, snfl, clod, spry, halo,
  ...), U50 blend (0 additive, 1 alpha, 4 darken). Collision-slot emitters burst on contact; sub-type 2 bursts at
  the rider's hit point (cap 1 per AI, 3 per human).
- The emitter's setters (proven, read in Ghidra; called by `ParticleSys_Create` 0x1d8490 and `Fx_SurfaceSpray`
  0x1311b0; emitter = system+0x10, +0x3c its time scale): `ParticleSys_SetSizeRange` 0x1d52e8 (min, max → +0x10
  max − min, +0x18 min − (max − min)), `ParticleSys_SetLifeRange` 0x1d5300 (the same, × +0x3c, at +0x14 / +0x1c),
  `ParticleSys_SetSpawnPoint` 0x1d5338 / `_SetSpawnLine` 0x1d5380 / `_SetSpawnQuad` 0x1d5418 (origin +0xa0 and its two
  spread axes +0xb0, +0xc0), `ParticleSys_SetVelocityBasis` 0x1d5790 / `_SetVelocityCone` 0x1d5570 (velocity +0x60 and
  three spread axes +0x70, +0x80, +0x90, each ÷ +0x3c, +0x60 less 1.5 × their sum), `ParticleSys_SetForce` 0x1d54e0 /
  `_ClearForce` 0x1d5548 (+0xd0), `ParticleSys_SetColors` 0x1d58e0 (+0xe0 start colour less 1.5 × (c2 + c3), +0x110
  (c1 − c0) ÷ (life × +0x3c), +0xf0 / +0x100 the random c2, c3). `ParticleGroup_UpdateTrail` 0x1d9208 (from
  `cBoarderRender::vf7` 0x1356f8): moves a group slot's particles and spawns new ones along the segment from the last
  point, five `Rand_Fx` draws each. Not yet read: 0x1d5b60 (10.5 KB, vector maths only, called once by
  `ParticleSys_Create` on system+0x150).
- Surface table words used by the effects (proven, E13; values in `tricky-rs/game/src/surface.rs`): ground spray
  (`Fx_SurfaceSpray` 0x1311b0, ground motion, not surfaces 9, 13, 18, 19, which spark) takes [13] rate, [14] size
  × 0.8–1.2, [15] [16] lives, [17] sprite index (gApp+0x30), [18] pull of the light colour (rider+0x5ae0 +0x470)
  toward its grey, [19] brightness added, [20] alpha × 128, [21] draw mode. Board trail (`Fx_BoardTrail_Update`
  0x12fef0, ground only, |e| > 0.3 with e = steer·(1 − |rider+0x1fc|), sign by stance): width [22]·(|e| − 0.2), tilt
  ±[23] along the normal, bend [24]·speed·e eased 1/30 a tick. Splashes (`Fx_LandingSplash` 0x12ccc8,
  `Fx_Splash_EmitAtPoint` 0x12c7e0): none if [22] < 0.02, ring size [22]·0.8.
