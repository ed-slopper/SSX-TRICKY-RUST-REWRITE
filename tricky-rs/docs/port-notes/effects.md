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

## Particle emitters (`Emitter_InitFromRecord`, `PBurst_*`, VU1 microcode at 0x31c288)

- type2Sub0: U0 N particles born evenly over U2 s (U2 < 0: continuous, recycled every T = U5 + U7/2, pre-warmed);
  U1 trail copies (copy j drawn at age − j·U8, alpha × (1 − j/U1)); U3 drag k (1/s); size U4 ± U6/2; life
  U5 ± U7/2; origin U9..11 + u·(U12..14) + u·(U15..17) and velocity U18..20 + u·(U21..23, U24..26, U27..29)
  through the instance matrix (u ∈ [−½, ½)); acceleration U30..32 world; pos(t) = P + W t + (V − W)/k ·
  f(min(k t, 2.7)), W = accel/k, f(x) = 0.73x − 0.113x²; colour (×128) c0 → c1 over T plus random c2, c3
  (keys at U33.., U37.., U41.., U45.., alpha first); U49 texture in particle.ssh (part, snfl, clod, spry, halo,
  ...), U50 blend (0 additive, 1 alpha, 4 darken). Collision-slot emitters burst on contact; sub-type 2 bursts at
  the rider's hit point (cap 1 per AI, 3 per human).
