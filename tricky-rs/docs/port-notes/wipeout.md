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

- One rigid body from the collision spheres (masses 4,2,2,...; inverse inertia 0.8·(1.2·I)⁻¹, see below); |ω| ≤ 23.56 rad/s. Gravity 1900.84
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
- Mass: sphere masses 4,2,2,2,2,2,2,2,4,4,2,2,2,2,3,3,3,3 (0x3a3f68); inverse inertia 0.8·(1.2·I)⁻¹ of the point
  masses. The spheres are where the skeleton puts them (`CollBody_UpdateFromBones`, row E8d2); the app poses them
  from the bind pose (rider/wipeout.rs, row G6c, below).

## The wipe-out body, exactly (board row G6a; `tricky_game::ragdoll`, checked: `tests/ragdoll.rs`)

- The motion's body (wipe-out motion +0x70, a `cRigidBody`): +8 seconds since the last touch, +0x20 / +0x40 the
  crash point and normal, +0x60 the rider.
- `Ragdoll_ComputeMassProps` 0x10d6f8 (each step): `CollBody_PointMassInertia` of the rider's body (rider+0x470);
  1/m; the inertia × 1.2 inverted by cofactors, × 0.8; v and ω again (`RigidBody_UpdateVelocity`, with the spin
  limit). Returns the centre of mass.
- `Ragdoll_InitFromBoarder` 0x10dad8: the body at the centre of mass, the rider's orientation (+0x170) and
  velocity (p = v·m), no spin (L from ω = 0); the crash normal turned round and kept at rider+0x2f0; if the crash
  point closes on it, `RigidBody_ContactImpulse`(0.15, 0).
- `Ragdoll_Step` 0x10dca8(dt): F = (0, 0, −1900.8359·m) − ½p, τ = −½L; while +8 < 0.15 s also 600·m toward the
  course point (rider+0x350) taken along the ground (rider+0x2a0) and made unit. Integrate, v and ω, the contacts.
  Then the rider: its offset from the centre of mass turned back by its old orientation, turned by the body's,
  the rider placed at the body's position less it; its orientation and velocity the body's;
  `Boarder_UpdateOrientation`.
- `Ragdoll_Contacts` 0x10e190: the record's kind 3; the body's +0x24 set to the word at 0x31d314 while it casts
  (−1 after); one `cWorldBoarder` built at the start serves every cast, so the later casts see the body where it
  was. The world: a hit pushes the body (and the rider's spheres) out by its depth; if the point closes,
  `Boarder_AccumulateImpact`, the rider's virtual +0x64(point, v, n), the bounce and the record kept. The bounce:
  e = 0.8 − 0.7|n_z| and μ = 0.15|n_z| at 100 cm/s or more (else 0, 0), then p and L × 0.95 (v and ω from them,
  no spin limit). A second cast the same way, without the virtual; a normal facing against the first is first
  made to slide along it (n₂ − (n₂·n₁)n₁, unit). The objects: the normal taken along the ground and made unit; only
  objects with flag 0x80 push the body out; a closing point is a contact (`Instance_OnPlayerContact` with the
  point, the body's direction of travel, the normal and its speed), with flag 0x80 also an impact and a bounce
  e 0.5, μ 0, and the sound. Then the rider's broadphase pairs: rollers get `Ragdoll_HitRoller`; type 0x3ef nodes
  (riders) call `FUN_0010ebc8`, which is empty.
- `Ragdoll_HitRoller` 0x10ed68: as two rollers (world.md) but j = −1.5·closing / (…), no bounces added: the
  roller is woken (`Roller_OnHit` without a rider).
- `Ragdoll_GroundProbe` 0x10fd58: the hip bone (rider + 0x394·[+0x44d8] + 0x8c8) turned by the rider's
  orientation, from the rider, taken onto the plane through the rider; a line ±400 cm along the ground's normal
  (`cWorldLine` 0.5). No ground: normal (0, 0, 1), fall line = the board's up (+0x1a0), kind 3. Ground: its kind
  (+0x2e0), its surface type (out +0x34 to +0x290), normal, height above it (+0x1bc), `Boarder_GetFallLine`, the
  point (+0x2d0). Both: +0x320 the board's up (+0x1a0) along the ground, unit.
- Checked in the runner on 6,000 random riders (bodies spread about them, every cast scripted, the shapes handed
  over compared): the rider, the body, the record, the roller and every call bit for bit.

## The wipe-out motion, exactly (board row G6b; `tricky_game::wipeout_motion`, checked: `tests/ragdoll.rs`)

- The motion (motion 5): the body (above) with +0xc seconds down, +0x10 seconds on the ground, +0x14 / +0x18 the
  stick (the tumble's own control: torque about the board's side and forward), +0x30 / +0x50 the normal and the
  speed when a slide ended. A tick is rider+0x12c / 60 s. The board's axes: +0x180 forward, +0x190 side, +0x1a0 up.
- Enter (0x10c198): `Ragdoll_InitFromBoarder`; the stance (+0x1b4, +0x46d4: switch when +0x464's +0x40 is set and
  +0x420 is 2) and its angle (+0x46e0, π wrapped to −π..π); `AnimCtl_BlendOutAll`; timers 0; the surface type kept
  at +0x310 and taken as 10 until something is touched; +0x1dc 0; the impact clip; six pairs (+0x1f4, +0x200,
  +0x20c, +0x218, +0x248, +0x26c) set to (2.0, 0); `Sfx_MuteRiderLoops`. Exit (0x10d680): `Sfx_StopWipeoutLoop`,
  `Sfx_UnmuteRiderLoops` unless into motion 6, the rider's virtual +0x9c.
- Slides (clips 0x2dc / 0x2dd): pos += v/60; v += (n_x·f, n_y·f, f − g)/60 with f = n_z·g, g from the surface
  table (race +0x24, 100 bytes a type), × 0.98333; held to the ground by at most 8.333333 cm (height + ride height
  +0x294); the velocity into the ground removed; `Ragdoll_GroundProbe`. Lost ground, n_z < 0.3 or a bad surface:
  falling cycle and the body made again from the rider (the crash point and normal the rider's, +0x50 its speed).
  The board turned toward lying flat (axis n × forward, negated on the face, angle asin(min(|·|, 0.99999))/12).
  Get up when that sine is under 0.1, the speed under min(833.33 + 694.44·t_ground, 2222.22), |forward·n| > 0.9
  and `Boarder_ProbeGround` finds the ground; else the world's collisions with rider+0x494 set to the word at
  0x31d310.
- Tumble: the rider lifted by (+0x294 + 10)/2·max(n_z, 0) along the normal for the step (`Boarder_Translate`) and
  put back after it; `Ragdoll_Step`, `Ragdoll_GroundProbe`; +0x2e0 the record's kind. No touch: after 0.1 s in the
  air the curled cycle becomes curled-to-stretched (`AirPredict_AtTakeoff`, else `AirPredict_Tick`); landing in
  over 0.1 s on a good surface with the stick centred (both under 0.5): L = L·(1 − 3.3475056 dt) + (up × n_land)·
  250951.34·dt·m + (forward × d_land)·(±125475.67)·dt·m (d_land the landing velocity along the ground, the sign by
  forward · d_land), and up · n_land > 0.9 with |ω| < 4 recovers (clip 0x224, motion 1, state 0xd). A touch: the
  ground normal the record's when steeper or the probe missed, the surface type the record's +0x34 for kinds 0 and
  2 (else 10), the impact clip (and `FUN_00123f00`) when landing from a cycle.
- On the ground over 0.25 s on a good surface (`Sfx_PlayWipeoutSlide`), steeper than 0.349 n_z: forward · n >
  0.75 butt slide, < −0.75 face slide, else rolled with L += (forward × n)·(±400000)·dt·m. Straight up: t > 0.5,
  t_ground > 0.1, |ω| < 7, n_z > 0.349, |forward · dir(course 8 m ahead)| > 0.8, up · n > 0.85 and the ground probe:
  clip 0x250 (curled) or 0x251, motion 2, state 3. Then the stick: L += side·75285.4·dt·m·s_x + forward·(−125475.67)
  ·dt·m·s_y, damped by (1 − 3.3475056 dt) unless the landing righting did it. t_ground ≥ 3 or t > 7: a human
  (+0x418 < 0) gets the stuck counter (+0x2e4) 1e6; the computer slides by forward · n and gets up.
- `Wipeout_StartGetUp` 0x10f178: the course 8 m ahead against the board's up (f) and side (r); the board turned a
  quarter about its side, then about its up as the clip needs; clips 0x24a..0x253 as listed above; the rider moved
  by the hip's place less the clip's displacement (`AnimCtl_GetClipDisplacement`), both turned; motion 6, state
  0x14. `Wipeout_PlayImpactClip` 0x10f9a0: `Sfx_OnWipeoutImpact` first; up · n < −0.7 HEAD, > 0.7 FEET, else side ·
  n > 0.7 LEFT, < −0.7 RIGHT, else forward · n > 0 BUTT, else FACE.
- `GetUpMotion_Update` 0x1102c8 (motion 6): the slide's step with `Boarder_ProbeGround` before holding to the
  ground; up turned to the normal (`Quat_RotateVecToVec` from the identity), its forward to the course along the
  ground, the orientation slerped 1/12 of the way (`Quat_Slerp`); `Boarder_UpdateOrientation`; the collisions.
- Checked in the runner on 40,000 cases (enter, update, the get-up start, impact clips, exit, the get-up motion;
  the animation, probes, predictions and sounds scripted): the rider, the body, the timers and every call bit for
  bit.

## The app's wipe-out (board row G6c, with E7c3e's wipe-out half; `tricky-rs/game/src/rider/wipeout.rs`)

- Proven (read from code): `Boarder_Wipeout` 0x11d830 copies the contact's point and normal into the body (+0x510,
  +0x530 of the rider's motion object), then state 0x13, motion 5. `GetUpState_Update` 0x108198 ends the get-up on
  clips 0x1ff/0x221 (state 3) and 0x202 (state 5), and asks `Boarder_RequestReset` whenever the surface under the
  rider (+0x290) is 0. The masks: 0x31d310 = 0xfc3 (rider+0x494 during the wipe-out's walls), 0x31d314 = 0x273c3
  (the spheres `Ragdoll_Contacts` casts). `Boarder_Translate` 0x1267a0 moves the collision body, +0x6e0/+0x6f0 and
  the broadphase box. `AirPredict_Tick` 0x123ec8: +0x4c += 1/60, +8 = +0x44 − it, +0x50 = +0x48 − it.
- The app runs `wipeout_motion` (motion 5) and `get_up_update` (motion 6) a 60 Hz game tick at a time in the game's
  units (cm, z up). The casts go to the course (`CollisionWorld::course`): the body's `cWorldBoarder`(body, 0) through
  `World_RayCast` and `World_QueryInstances`, the probe's `cWorldLine`(0.5, …, 2) through the rider's last-hit record,
  the body's line `cWorldLine`(0.5, …, 1), the flight prediction's `cWorldLine`(0, …, 0), and the walls through
  `walls::wall_collide` with `cWorldBoarder`(body, 1) under the mask 0xfc3. Without the course (a levels folder) the
  drawn mesh stands in.
- `WipeEnv::sync` is ours: the game's `Ragdoll_Step` hands `Boarder_UpdateOrientation` the whole rider, so the
  motion reads the forward and side it wrote; the port's motion keeps them in `WipeRider`, which the glue updates.
- STANDIN, inferred or left: the collision body re-posed from the bind pose each tick (for the skeleton, G5); the
  hip 91.7 cm over the root; the clip clock, chaining (impacts → crunch cycle, crunch-to-stretch → falling) and the
  get-up's end after 2n − 1 ticks (the animation controller, G5); no clip displacement (G5); no rollers handed to
  the body (E8c3); the wipe-out's walls' callbacks (done with G6d, below).
- Sims (`TRICKY_SIM=600`, the 11 disc courses): stuck 12 → 8, falls 26 → 30. Aloha's extra falls are wipe-outs
  sliding onto surface 0 and reset there, as `GetUpState_Update` does (the sim counts each reset twice: the
  wipe-out's and the autopilot's rejoin).

### The wipe-out's walls and contacts (board row G6d)

- Proven: during motions 5 and 6 `Boarder_WallCollide` 0x126250 and `Boarder_InstanceCollisions` 0x125088 run with
  the mask 0xfc3; their callbacks are `Boarder_Wipeout` 0x11d830 (from the get-up only: motion 5's impact check
  returns first), `Score_Crash` 0x156de0 then `Boarder_AddMeter` 0x11b018, `Boarder_Stumble` 0x124dd0 (outside
  state 2 and without its flag: −0.02 to the meter; the stumble itself only in states 3 and 0xe, so never here),
  `Boarder_AccumulateImpact` 0x125c48 (+0x2e4 += 1 − max(n · +0x2f0, 0), +0x2f0 = n), `Instance_OnPlayerContact`
  0x13bd40 and the rider's controller's virtuals +0x64 / +0x6c (a human's: `cPlayer::vf11` 0x1532c0 = brake spray
  then `Rumble_Impact`, `cPlayer::vf12` 0x153350 = `Rumble_Add`). `Ragdoll_Contacts` adds its own impacts and
  object contacts. cBoarder's update (vf7 0x117198) multiplies +0x2e4 by 0.95603746 each tick and above 4.492007
  calls `Boarder_RequestReset` 0x118f10: a body wedged against walls is put back.
- The app does all of these on the course's collision (touch objects through `walls::instance_collisions`, rollers
  through `roller::rider_kick`, contacts through `knockables`); the rider-side calls (crash score, meter, spray,
  wipe-out again) run on the port's rider after the tick.
- STANDIN: +0x2e4/+0x2f0 start from 0 at each wipe-out (the riding walls' accumulation and the decay outside the
  wipe-out are not ported, row G6e); no rumble; no `Sfx_OnWipeout` / `Sfx_OnWallHit` (rows G10); rollers paired by
  box overlap (the race's broadphase).
- Sims (`TRICKY_SIM=600`, the 11 disc courses, same build without/with G6d): stuck 19 → 22, falls 30 → 32. The
  extra put-backs are the reset above (untracked t 67: a wipe-out wedged at 1 km/h against a wall).

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
- Body set-up (proven, F1c): `CollBody_Construct` 0x235fc8 only runs the 18 sphere constructors (empty; from `cBoarder`
  on rider+0x470 and `cWorldBoarder` on +0x60); `CollBody_ConstructCopy` 0x236000 constructs then `CollBody_Copy`s
  (no direct callers). Body+0 is the skeleton's bone matrices (`CollBody_UpdateFromBones` reads bone k at +0x30 + 0x40·k):
  `CollBody_SetSkeleton` 0x236128 stores it (`cBoarder` passes rider+0x4ae0, `cBoarder::vf8` the pointer at +4 of
  the object at rider+0x4494).
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
