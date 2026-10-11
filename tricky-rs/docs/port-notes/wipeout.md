# Wipe-outs: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Wipe-outs (`Boarder_Wipeout` 0x11d830 → state 0x13, motion 5; `Score_Crash` 0x156de0: −0.10 meter)

- Causes: bad landing (above); airborne sphere hit with v·n<0: f = min(0.9, 0.6+0.000135·vin); riderUp·n > f
  glances (≤6°/tick re-orient, normal velocity removed) else wipe-out; surface 6/10 on the ground; ground wall
  (`Boarder_WallImpactCheck` 0x126540): s = max(0.5·vin, 55.56); crash if s > 833.33(up·n)+1944.44, stumble
  (state 2, −0.02 meter) if s > 833.33(up·n) and (up·n ≤ 0 or grounded).
- Riders (`Boarder_RiderCollisions` 0x123fc8, `Boarder_RiderHit` 0x124920): pushed apart 0.55×overlap (pair once per
  3 ticks); Δ = (v_rel·n)·m_o/(m+m_o), applied clamp ±500 cm/s; |Δ| > 733.97 wipe-out (rail always knocked off;
  airborne pushed up never; motion 5/6 immune), |Δ| > 167.32 stumble; > 1000 same heading on ground → pops a jump.
  One down: the other +1.0 meter, faller no meter loss. Shove (clip 0x15): within 150 up, 180 away, 55° of heading;
  J = [108.58 + (v_rel·dir)·m/(m+m_o)]·(0.54663557+0.739932·s18).
- Tumble (ragdoll): gravity −1900.84, drag −0.5v and −0.5L, +600 cm/s² toward the course after ground contact;
  contacts e = 0.8−0.7|n.z|, μ = 0.15|n.z| above 100 cm/s, ×0.95 per contact. Into the slide after 0.25 s on
  ground (n.z > 0.349). Get up at once if t>0.5, ground>0.1, spin<7, upright (up·n>0.85). Righting in air →
  ordinary landing. Time-out: ground ≥ 3 s or total > 7 s → human reset, AI gets up.
- Slide (clips 0x2dc/0x2dd): pos += v/60; v += (n.x n.z g, n.y n.z g, n.z g − g)/60; v ×= 0.983333; snap ±8.33 cm;
  normal velocity removed. Get up when speed < min(2222.2, 833.33+694.44·t_ground) (`Wipeout_StartGetUp` 0x10f178)
  → motion 6 state 0x14 (`GetUpMotion_Update` 0x1102c8, same slide, facing → course at 1/12 per tick); ride away at
  the speed left.
- Reset (`Boarder_RequestReset` 0x118f10): button while down; stuck counter > 4.492; air-wall counter > 12.002;
  100 m below the world. `ResetState_Update` 0x106a90: fade, at 0.8 s place on the course spline, drop 101.46 cm
  above ground moving along the course at 833.33 cm/s (motion 1). A requested reset costs −0.12 meter.

## Wipe-out body (`Ragdoll_Step` 0x10dca8, `Ragdoll_Contacts` 0x10e190, `Wipeout_StartGetUp` 0x10f178)

- One rigid body from the collision spheres (masses 4,2,2,...; inertia ×1.5); |ω| ≤ 23.56 rad/s. Gravity 1900.84
  cm/s², drag 0.5/s linear and angular; the hit's impulse (e 0.15) is the only initial spin; angular momentum
  × (1 − 3.3475·dt). Contacts: e = 0.8 − 0.7|n_z|, μ = 0.15|n_z| (none under 100 cm/s), then ×0.95.
- Impact clips on hitting the snow: cmI_/cmCI_ HEAD, FEET, LEFT, RIGHT, BUTT, FACE by which body axis meets the
  ground. In the air the body rights itself when time to land > 0.1 s, and recovers to riding (xA_FALLRECOVER)
  if upright (Z·n > 0.9) and spinning under 4 rad/s. Slides after 0.25 s on the ground (back > 0.75 / front).
- Get-up clip by lying side and where the course is: GU_FROMBUTTFWD/RIGHT, GU_FROMFACEFWD/BWD/RIGHT, CT_ROLLBWD2BASE.

## Wipe-out motion (`WipeoutMotion_Update` 0x10c2f0, `GetUpMotion_Update` 0x1102c8, `Wipeout_StartGetUp` 0x10f178)

- Enter (0x10c198): body P = m·v, L = 0, one impulse at the crash point (e 0.15) is the only spin; surface := 10
  until something is touched; impact clip. Timers: t (down), t_a (since last contact), t_g (on snow).
- Tumble tick: F = −1900.84·m·z − 0.5P (+600·m toward the nearest course-spline point 0x350, along the snow, while
  t_a < 0.15); τ = −0.5L; explicit Euler (x, q from the old v, ω); contacts (2 passes, deepest; e 0.8−0.7|n_z|,
  μ 0.15|n_z|, both 0 under 100 cm/s; then P, L ×0.95; instances e 0.5); probe ±400 cm through the hips.
- Air (t_a > 0.1): CRUNCHCYCLE → CRUNCH2STRETCH → FALLINGCYCLE; if predicted landing > 0.1 s away and not surface
  6/10: L += m·dt·(250951·(Z×n_land) ± 125476·(X×d_land)), L ×(1−3.3475dt); Z·n_land > 0.9 and |ω| < 4 →
  xA_FALLRECOVER, ordinary air state 0xD. Else L ×(1−3.3475dt). t_g counts while the probe hits within 0.1 s of a
  contact, else resets.
- Contact: impact clip if a tumble cycle is playing (curled, category 17 → cmCI_; stretched, 18 → cmI_; FEET
  Z·n>0.7, HEAD <−0.7, LEFT Y·n>0.7, RIGHT <−0.7, BUTT X·n>0, else FACE); impacts chain to CRUNCHCYCLE (end action
  5), CRUNCH2STRETCH to FALLINGCYCLE (6).
- t_g > 0.25, n_z > 0.349, surface ok: X·n > 0.75 BUTTSLIDE, < −0.75 FACESLIDE, else L += 400000·m·dt·sgn(X·n)(X×n).
- Up at once (no get-up): t > 0.5, t_g > 0.1, |ω| < 7, n_z > 0.349, |X·dir(spline+8 m)| > 0.8, Z·n > 0.85 →
  CT_CRUNCH2BASE (crunch) / CT_STRETCH2BASE, state 3 riding.
- t_g ≥ 3 or t > 7: player reset (stuck counter := 1e6); AI slide clip by X·n and get up where it is. Surface 0
  in states 0x13/0x14 resets at once. Surface 6/10 blocks slide and get-up (only the time-out ends it).
- Slide (fixed 1/60): pos += v/60; v += (n_x n_z g, n_y n_z g, n_z g − g)/60; ×0.98333; snap ±8.33 cm/tick; no
  normal velocity; lie flat 1/12 a tick (X or −X to n); ground lost / n_z < 0.3 / surface 6, 10 → FALLINGCYCLE and
  the body re-made (e 0.15). Get up when flat (sin < 0.1), |X·n| > 0.9, speed < min(2222, 833+694·t_g).
- Get-up: d = dir(spline point 800 cm ahead, 0x360); f = Z·d, r = Y·d (lying axes). Butt: f>0.707 CT_ROLLBWD2BASE
  (0x24a), r>0.707 GU_FROMBUTTRIGHT ending switch (0x24c; ex 0x252 *2FAKIE), f<−0.707 GU_FROMBUTTFWD (0x24b),
  else 0x24c. Face: f>0.707 GU_FROMFACEFWD (0x24e), r>0.707 GU_FROMFACERIGHT switch (0x24f; ex 0x253), f<−0.707
  GU_FROMFACEBWD (0x24d), else 0x24f. On the spot (root bone kept); then slide motion with the frame slerped 1/12
  a tick to up = n, nose → 0x360; riding after 2n−1 ticks.
- Clip ids: 0x2dc BUTTSLIDE, 0x2dd FACESLIDE, 0x2de F_FALLINGCYCLE, 0x2df CC_CRUNCHCYCLE, 0x2e0 CT_CRUNCH2STRETCH,
  0x2e1–6 cmCI_ and 0x2e8–d cmI_ (BUTT FACE FEET HEAD LEFT RIGHT), 0x224 A_FALLRECOVER, 0x250/1 CT_CRUNCH/STRETCH2BASE.
- Mass: sphere masses 4,2,2,2,2,2,2,2,4,4,2,2,2,2,3,3 (0x3a3f68); I = 1.5× point-mass inertia. Sphere places
  are per character (not extracted): the remake uses a 1.7 m stand-in (rider/wipeout.rs).

## Rider hits, exactly (board row E8d1, checked: `tests/hit.rs`; `tricky-rs/game/src/hit.rs`)

- `Boarder_RiderHit` 0x124920(speed, rider, other, pair record, dir): the push is the speed clamped to ±500 cm/s;
  over 1000 cm/s on the ground (motion 2), with the rider's heading (+0x1b0) within 45° of the record's angle and the
  push out of the ground, the rider hops instead (crouch +0x1c8 = 1, `Jump_ApplyImpulse` with the speed in $f12,
  motion 1, state 0xd); else the direction is flattened onto the ground plane (not in the air, motion 1), the push
  added to the velocity, the rider's virtual +0x6c given push × 0.0012. Motion 5/6 (down) stops there. In the air a
  push out of the ground counts as 0. Over 733.9661 cm/s (530.50287 in game mode 0, knockdown practice, 0x32de8c),
  or any hit on a rail: `Boarder_Wipeout` with (position + dir × record[0], the velocity's direction, dir, push),
  `Rivals_OnKnockdown`, the speech lines; over 167.31738: `Boarder_Stumble` (−0.02 meter) and `Rivals_OnHit`
  (the other shoving, clip type 0x15) or `Rivals_OnBump`.
- `Boarder_RiderCollisions`: the pair's closing speed k = (v_b·n − v_a·n) / (m_a + m_b) with m =
  `Boarder_GetSpeedStat` 0x11d7b0 = weight byte × (0.79599 + 2.145779 × stability byte / 255) × (1 + 8 × boost
  level) (1e30 for a rider finished and stopped, who is not moved either); a takes m_b·k, b −m_a·k. One down:
  `Boarder_ScoreKnockdown` 0x11e9a0 for the other (`Scoring_MsgKnockdown`: +0xbc, message kind 4; +1.0 meter) and
  `Boarder_ScoreCrash` 0x11e988 for the fallen (`Score_Crash`(mode 0), no meter); both: each `Score_Crash`(0) and its
  −0.1. Two equal riders closing at over 14.7 m/s both go down.
- In our riders (`Rider::hit`, `props::bump`): STANDIN one body sphere per rider, a three-tick wait per rider
  instead of per pair, the pair record (wipe-out offset, hop angle) not kept (no hop), never knockdown practice, the
  push's virtual not called.

## Collision bodies, pair records and the shove, exactly (board row E8d2, checked: `tests/collbody.rs`, `tests/hit.rs`)

- The rider's collision body (rider+0x470, `tricky-rs/game/src/collbody.rs`): `CollBody_InitRider` 0x236050 sets 18
  spheres (radii 25, 20, 15, 15, 15, 15, 10, 10, 25, 25, 20, 20, 30, 30, 20, 20, 20, 20 cm) and a bounding radius
  (m × 100; its centre kept); `CollBody_UpdateFromBones` 0x2361b0 puts spheres 0–13 on skeleton bones 2, 4, 6, 10, 7,
  11, 8, 12, 13, 16, 14, 17, 15, 18, the board's spheres 14–17 at ±70 and ±45 cm from the middle of 12 and 13 along
  the line between them, the bounding centre halfway between 8 and 9; `CollBody_Translate` moves it all;
  `CollBody_OverlapSpheres` 0x236a00: bounding spheres first, then the first overlapping pair (a's spheres in order,
  then b's) gives the push (unit a→b × overlap) and the touching point on a's sphere.
- The pair records (rider+0x40 + 0x20 × the other's index), written every tick by `Race_UpdatePairDistances`
  0x115a80: [0] the flat distance between the riders (rider+0x480, x and y), [1] the bearing to the other (the later
  rider of a pair keeps atan2 of the difference as is, the earlier one + π wrapped to −π..π); +8 and +0xc the ticks
  of the last hit and shove, +0x1a a count. The AI reads them too (`AIComputer_PickTargetRider`: within 700 cm, and
  others). `Boarder_RiderHit`'s wipe-out goes the pair's distance along the hit; its hop needs the rider heading
  within 45° of the bearing to the one that hit it.
- The shove (`Boarder_RiderCollisions`, `hit::shove`): the shover's layer-1 clip type 0x15 between clip flags 1 and
  2, three ticks since the pair's last shove both ways; the other's point (+0x4ad0) within 150 cm along the shover's
  up (+0x4ac0), 180 cm across, and its bearing −atan2(d·fwd (+0x4aa0), d·side (+0x4ab0)) within 55° of the shove's
  heading (+0x1e0). Speed (108.58082, or 250.59537 in knockdown practice) + (6.6·n · (v_other − v_me)) × m_me /
  (m_me + m_other), if above 0 × (0.54663557 + 0.739932 × stability / 255), into `Boarder_RiderHit` on the other
  along n (positions +0x140); a knock-down scores as in a collision.
- Our riders' shoves go through `hit::shove` (`props::shove_push`; STANDIN frame: heading, right, world up, the
  position; shove heading 0 right, π left). Still STANDIN: the collision body (our riders have no game skeleton in
  the game crate, so one sphere), the pair records (not kept), the hop.
