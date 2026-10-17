# Riding on the snow: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Ground riding (`GroundMotion_Update` 0x10a0d8)

**Input shaping** (targets moved by cBoarder::vf7 0x117198 at `rate/60` per tick, timescale ignored):
- Steer target = clamp(stick, ±0.905) × min(1, |v|/1135 cm/s); clamp ±1 while boosting. Rate =
  clamp(7.017·|err|, 0.1, 8.018)/s, ×0.6 on surfaces 3 and 4.
- Crouch rate = max(|err|, 0.1)·5.0025/s. Brake rate = max(|err|, 0.1)·5.035/s (release (1.1−|err|)·5.035).
  Brake forced to 0 while crouched; crouch and brake cancel.

**Forces** (previous probe's normal n, forward f, side r = n×f, height h above snow):
- Ground spring `Boarder_GroundSpringForce` 0x109878, d1 = 0.5 cm, d2 = 2.504 cm:
  h > 0: −(g/30)·h − (vn>0 ? surf[9]·vn : 0); −d1 < h ≤ 0: −g·h/d1 − surf[9]·vn;
  else g·(1 − 2(max(h,−d2)+d1)/(d2−d1)) − surf[9]·vn.
- Lean φ = surf[5]°·steer, m = min(fN, 2g):
  `a = n(fN − m/cosφ) + (n cosφ + r sinφ)(m/cosφ) + f(drag + thrust) + r·side + (0,0,−g)`.
- If h < −d2: pos −= n·max(h+d2, −10); remove normal velocity; clamp a·n ≥ 0.
- Explicit Euler: pos += v·dt; v += a·dt.
- Board yaws toward the velocity:
  θ = asin((v×f)·n/|v|); tgt = −θ + s·b·(1 + a_c/2·(1−s²))/(1 + 0.01·crouch) (negated if v·f<0);
  k = min(1, |v|²·2.003e-5·dt)·max(G, s·sign(tgt)); G = min(1, clamp(min(|v|/555.6,1)·(0.5−|v̂·fallLine|)·40, 0, ∞)+0.01);
  rotate about n by clamp(tgt·k, ±6°) per tick. (a_c, b) = class 0 (0.30019, 0.34919), alpine (0.20834, 0.26221),
  class 1 (0.4, 0.52398).
- Wall collision 0x126250: v += n·(vn + max(0.5·vn, 55.6 cm/s)), push-out 1.1× depth.
- Ground probe 0x128ae8: ray +200 → −100 cm along n → new n, h, surface.
  Proven (E8g; exact in `tricky-rs/game/src/probe.rs`, checked bit for bit in `tests/probe.rs`): the probe point is
  pos (+0x140) + side (+0x330)·(stats+0x60 · rider+0x244); `cWorldLine::cWorldLine`(0.5, point − 100n, point + 200n, 2),
  `World_RayCast` 0x25aff8 on the world 0x346488 with the cache rider+0x430. Miss (distance < 0): n = up, kind
  (+0x2e0) 3, fall line (+0x2b0) = forward, returns 0. Hit: surface +0x290 (a new one sets the tick count +0x29c to
  0x80000001; the same one zeroes it unless it is negative), n +0x2a0, ground point +0x2d0, kind = the hit's, h +0x1bc
  = (point − hit)·n, fall line `Boarder_GetFallLine` 0x128a18 (n × unit(n × up), or forward when |n × up| ≤ 0.0001),
  returns 1. Both ways: forward in the plane +0x320 = unit(f − (f·n)n) (vrsqrt), side +0x330 = n × that (w 0). The hit
  record: +0 kind, +0x10 point, +0x20 normal, +0x34 surface.
- Normal bleed (not on surfaces 3, 4): v −= 0.4(v·n′)n′, |v| restored.
- Airborne when the probe misses or h > surf[6] (2.74 cm on groomed snow).
- Speed caps (vf7): 2788.84 cm/s; boosting 2932/3072/3347 by meter level; 3347 while timer +0x134 > 0;
  cap = max(target, cap − 3.472) per tick.
- Backwards turnaround: vF < −111 cm/s, brake 0, state 3/5/12 → board turns 180°, switch toggles, steer negated.

**One ground tick, in the original's order** (`GroundMotion_Update` 0x10a0d8). Proven (E8h): exact in
`tricky-rs/game/src/ground_motion.rs`, checked bit for bit in `tests/ground_motion.rs` over 20,000 random ticks (every
rider field it writes, and every call it makes to what the test records, with its arguments, in order). In cm, z up:
1. Before anything moves: vn = v·n, vf = v·fwd(+0x320), vr = v·side(+0x330); dt = rider+0x12c / 60; the surface row of
   rider+0x290 as it is now. **That row is used to the end**, also for the airborne test (row[6] against the height the
   probe finds), not the row of the surface the probe lands on.
2. The four forces (`ground.rs`): spring(h, vn), thrust, drag(vf, load) with load = (`DAT_0031d308` · spring) / g, where
   `StaticInit_GroundMotion` 0x10af18 sets `DAT_0031d308` = `DAT_0031d304` / 30 = 1.0, and side friction(vr, vf, steer).
   Powder in the drag is surface 3 **or** 4. Then spring = 0 when n.z < 0 and spring < 0; rider+0x160 = drag.
3. Lean: rider+0x1d8 = row[5]·(π/180)·steer, `Math_SinCos`; m = min(2g, spring) / cos; the acceleration is
   n·(spring − m) + (n·cos + side·sin)·m, plus fwd·(drag + thrust), plus side·friction, plus (0, 0, −g), summed in that order.
4. Riding backwards (brake exactly 0, vf < −111.11111, state 3, 5 or 0xc): `Boarder_SetState`(4), here, before the
   acceleration is built.
5. Sunk: s = max(d2 + h, −10); when s < 0: pos −= n·s, h −= s, v −= n·vn (vn from step 1), and a −= n·(a·n) when a·n < 0.
6. Euler: pos += v·dt (the velocity before this tick's), then v += a·dt.
7. The bob (rider+0x1c0, cm): (bob + (pos·up before − pos·up after))·(1 − 4.684·dt), held between (1 − crouch)·lo and
   crouch·hi (lo, hi = −20, 10; alpine −10, 15), 0 in states 0xb and 0xc, then moved to it at most 1.6667 cm a tick.
8. The board yaws toward the velocity (formula above, now exact), through `Boarder_RotateAboutAxis` about the normal,
   unless state 0xe with no steer. The clamp is ±0.10471976 on the angle after the gain, and the gain is
   min(1, |v|²·2.0029847e-5·dt) · max(G, ±steer).
9. rider+0x494 = 3 while `Boarder_WallCollide` runs (the constant at 0x31d300), −1 after; then `Boarder_ProbeGround`.
10. Off the powder (surface not 3 or 4, of the probe's new one) 0.4 of the speed into the ground is removed and the speed
    restored.
11. A wall hit skips the next test; otherwise no ground, or row[6] < h, is `Boarder_TakeOff` and the end.
12. Surface 6 or 10 (of the probe's) puts the rider down: the rider's virtual 0x64 (`cBoarder::EmitBrakeSpray` 0x11a798, position, velocity, normal), then
    `Boarder_Wipeout` with a record (position, unit velocity, normal, normal·velocity, unit velocity),
    `Score_Crash`(rider+0x5820, 0), `Boarder_AddMeter` with its float, and the end.
13. Otherwise: the board is turned toward the ground, about unit(n + (0, 0, n.z/2)) × rider+0x1a0 (the board's
    up, proven in E8j), by −asin(|axis|)·0.23333861, when |axis| > 1e-5; the wobble phase (motion+8) += min(1, 0.00036·|v|) ·
    (|steer| + |brake| + 0.1)·2.65, wrapped at 2π, rider+0x1cc = 7.5·that amplitude·cos(phase);
    rail capture (G1b), `Boarder_InstanceCollisions`.
Not part of this proof (recorded calls only, answered by the test): the rotation, the wall collision, take-off, the
virtual 0x64, the crash calls, rail capture and instance collisions.

**The port rides on it (E8k).** `Rider::step` (`game/src/rider.rs`) no longer has its own ground branch: on the snow it
calls `ground_motion::update` through `game/src/rider/ground_tick.rs`, with the port's fields converted to the game's
cm and z up. Proven by construction, not by a recording: the conversion is a rotation, game (x, y, z) = port (x, −z, y)
(a mirror would flip every cross product, and with it the side and the lean): the port's forward (−sin yaw, 0, −cos yaw)
is the game's heading yaw + π/2, the identity attitude (forward along +y) is the port's yaw 0 on level ground, and the
game's steer is positive to the left, the port's to the right (`steer` is negated both ways). The game-side rider
(`Rider::gm`) is kept between ticks, so the attitude quaternion, the board's lag behind the ground, the bob, the wobble
phase and the probe's results are the game's; it is rebuilt from the port's yaw and ground normal when the port moved
the rider itself (position, velocity or yaw differ from what the last tick stored: landing, respawn, get-up, a pad).
What `Env` is answered with:
- exact: `Boarder_TakeOff` 0x128470 (`orient::take_off`) with `Jump_ApplyImpulse` 0x1284e0 (`jump::jump_impulse`, min 0,
  crouch = rider+0x208) on the game-side velocity; the revert (`Boarder_SetState`(4)) swings the board π about the
  normal with `Boarder_RotateAboutAxis` (the exact rotation), toggles switch and negates the steer and the lean value (rider+0x244, as `ChangeFakieControl_Enter` 0x103eb8 does)
- the port's own, tagged STANDIN: the walls (a sphere against the drawn mesh, `Rider::walls`, for
  `Boarder_WallCollide` 0x126250 and `Boarder_WallImpactCheck` 0x126540, row E8b), the ray cast under the probe (a
  vertical query of the drawn mesh 2 m over, 1 m under the probe point, for `World_RayCast` 0x25aff8, row E7), the
  control state (3, or 0xb after a hard landing: no state machine yet, row G1),
  rider+0x2c0 = the rider's up, the game's own record since G20c (`Rider::up`, section "The rider's up" below). The stat bytes the tick reads (speed +0x0e, edging +0x11, grip +0x13,
  cubic +0x19) and the jump's (+0x17) are the game's exact ones since G20a (section below)
- `Boarder_Wipeout`, `Score_Crash` and `Boarder_AddMeter` of a crash on surface 6 / 10 are `Rider::go_down` (one call;
  `score_crash` and `add_meter` of the `Env` do nothing); inside the Megaplex tube the crash is skipped (ours, row E9)
- the crash spray (`cBoarder::EmitBrakeSpray` 0x11a798) is recorded in `Rider::crash_spray` (position, velocity, normal),
  nothing draws it yet (row G19); `Boarder_SetMotion` / `Boarder_SetState` bodies do nothing (row G18)
- not done on the snow, as before: rail capture (the port captures a rail from the air only, row G1b), instance
  collisions (row E8b)
Changes this makes to the ride, all the game's: the spring, drag, thrust and friction now see one boost level
(rider+0x130, 1 on a speed-boost pickup) and one steer (the shaped steer, positive left); the airborne test and the
surface-6/10 test use the probe's new answer; the ground normal is the probe's every tick (straight up when the ray
misses); a take-off on a steep lip (normal z over cos 50°) goes through the steep-lip branch of `Jump_ApplyImpulse`.
A rider at exactly zero speed gets a speed of 1e-6 cm/s along the board (the yaw step divides by it; the game never
rides at a standstill, the start gate has its own states: ours, row G1c). Checked in `game/tests/physics.rs` (a rider on
a slope stays on the snow, gains speed within the limit, follows the surface, turns the way the stick points; rides
off the edge; surface 6 puts it down), not against the running game: that needs a recording of a ride through the port
(rows F4b3, F4h2). `TRICKY_SIM=600` needs a level export and was not run in this session.

**Forward drag** `Boarder_ForwardDrag` 0x109cb8 on vF only, with L = max(1, fN/g):
`−vF·mul·(L·surf1·lin + (1−crouch)·0.10573 + (1+1.2154·lvl)·1.7513·B²·Bst + |vF|·0.001·L·(surf2 + |vF|·0.001·surf3·cub))`,
lin = 1.0649822 − 0.30845523·S (alpine 0.70764244 − 0.30197912·S), cub = 1.2848105 − 0.29035342·S2, Bst = 0.6150995 + 0.9181778·E.

**Side friction** `Boarder_SideFriction` 0x109ef8: `−vR·curve(|vF|)·surf[4]·(0.001+1.1412·E)/(1+3.5·lvl)`,
×0.85 switch (×0.70 alpine). curve (m/s): v<5.556 → 0.20104+0.088997v; v<13.889 → 0.69547+0.03613(v−5.556);
else 0.99655−0.010441(v−13.889).

**Self-push / boost** `Boarder_GroundThrust` 0x109950:
push = surf12·k·ang·min(surf11/3.6 − |v|, 11.11) m/s², k = 0.738+0.277·S (alpine 1.205+0.305·S),
ang = (60° − |boardYaw − courseYaw|)/30° capped at 1 (courseYaw = course spline 8 m ahead). Skate anim
(crouched, |steer|<0.2, v<8.33) gives 0.2·k·deficit. No push while braking.
boost = lvl·(0.07983 − |s|)·(23.50 + 10.53·max(0, f_up))·surf12 m/s² — any steer above 0.08 kills it.
Boost level 1.0 / 0.6013 / 0.25 by meter > 0.666 / > 0.3336 / else.

**Surface table** (`SurfaceTable_Init` 0x256188; 20 rows × 25 words, table pointer in $a0): [0] g, [1] lin, [2] quad,
[3] cubic drag, [4] side grip, [5] lean°, [6] airborne height cm, [7] [8] spring band d1 d2 cm, [9] normal damping,
[10] body drawn above the snow cm, [11] push target km/h, [12] thrust scale, [13..21] ground spray, [22..24] board trail
and splashes (`tricky-rs/game/src/surface.rs` has every word and its reader; E13).
Proven (E8f): every word read by running the function in the runner; the exact values are
`tricky-rs/game/src/surface.rs` and `tests/surface_table.rs` checks all 500 bit for bit. Row 0 (off the course) is
7.347 cm / 11.61 per s for [6]/[9], not 20/30 as noted before.
surf[6]/surf[9] by row: 0 7.347/11.61; 1 2.742/5.009; 2 2.849/5.530; 3 15.04/2.842; 4 30.03/2.976; 5 2.775/4.052;
6,10,15,16,17,19 20/30; 7 21.37/45.2; 8 20/39.4; 9 2.02/30; 11 0.70/17.9; 12 3.95/30; 13 2.64/0; 14 1.56/30; 18 2.58/40.

**Play test of the ground tick on Garibaldi (E8l, 2026-10-11; measured, not proven).** The player's own extraction of
`gari.big` (3,885 patches, 3,393 instances, 169 rails, 561,141 collision triangles), the release build, headless:
- `TRICKY_SIM=600`: the autopilot reaches the finish in 235 s (231 s before E8k, commit 73e6f8a), 5,562 m, top speed
  121 km/h, 0 falls out of the world, "autopilot stuck" 1 time (also 1 before). It leaves the snow 150 times
  (301 ground/air switches, 209 before: the exact tick takes off where the old branch did not) and is put down 5 times
  (2 before): 4 "air wall" (the port's air walls, row G20c3c3) and 1 "grab" (a landing with a grab held); none at a lip, none on the
  ground. The slowest moment after a wipe-out is 11 km/h, up again after 2 to 3 s. No stall, no crash loop.
- `TRICKY_RAILTEST=1`: all five longest rails lock on within 0.2 s, ride 4.7 to 9.5 s and leave into the air; identical to the
  run before E8k (the rail code is the port's, as before).
- `TRICKY_TRICKTEST=1`: the nine scripted drops score as before E8k, line for line.
`game/tests/physics.rs` keeps the lip as a test (`a_lip_takes_the_rider_off_without_a_crash_and_the_landing_is_back_on_the_snow`:
a run-in, an 8 m gap, 8 m of drop, the landing slope: it leaves the snow, is not put down at the lip, lands and keeps riding).
Seen and left (not the ground tick): in that run a rider wiped out in the air was captured by a rail 0.5 s later and
"got up" at 106 km/h, because the port's rail capture (`rider.rs`, the `self.rail = Some(..)` branch) does not look at
`crashed` and sets it to 0 (row E8m); the autopilot's distance from the race line (100 to 400 m on the second half) is
the same before and after E8k.

## Orientation, rotation and take-off (E8j; exact in `tricky-rs/game/src/orient.rs`, `tests/orient.rs`)
Proven (run in the function runner, 30,000 / 30,000 / 20,000 random cases bit for bit, planted bugs caught; the whole
`GroundMotion_Update` with the exact rotation inside matches over 20,000 ticks):
- rider+0x170 is the attitude quaternion (w, x, y, z). `Boarder_UpdateOrientation` 0x129008 normalises it (zero
  length: only w := 1) and derives, each with the w lane 0: rider+0x190 side (x axis turned), rider+0x180 forward (y
  axis), **rider+0x1a0 up (z axis)**, and rider+0x1b0 the heading (`Math_Atan` of fy/fx, ±π when fx < 0, ±π/2 when
  fx == 0 and fy != 0, fy itself when both are 0). Turning an axis a: t = u × a, result = (a + t·2w) + (u × t)·2.
- `Boarder_RotateAboutAxis` 0x128e90(angle, axis): sin/cos of angle/2, the quaternion (cos, sin·axis) multiplied in
  front of the rider's, then `Boarder_UpdateOrientation`.
- `Boarder_TakeOff` 0x128470: `Boarder_SetMotion`(1); then the state (rider+0x428) is read. If it is not 0xa
  (`cJumpControl`): rider+0x1c8 := rider+0x208, `Jump_ApplyImpulse`(min 0), `Boarder_SetState`(0xd). If it is 0xa:
  `Jump_ApplyImpulse`(min 0), tail call `Takeoff_SetSpinRates`.
- The crash virtual of `GroundMotion_Update` (vtable+0x64 of cBoarder's vtable 0x367640, this-delta 0) is
  `cBoarder::EmitBrakeSpray` 0x11a798 (slot 11): `Boarder_EmitBrakeSpray` 0x136e70 → `BoarderRender_EmitBrakeSpray`
  0x135258 → `Fx_BrakeSpray_Emit` 0x12e008. It makes spray from the position, velocity and normal and changes
  nothing of the ride. (The old test assumed a this-delta of 0x20: wrong.)
Open: the bodies of `Boarder_SetMotion` 0x11f610 → 0x11c648 and `Boarder_SetState` 0x11f608 → 0x11c7d8, and the spray
effect chain above, are not ported (rows G18, G19).

## Animation timing

- Body height (proven, E13): `cBoarder::vf7` 0x117198 eases rider+0x308 toward surface word [10] at 1.6667 cm a tick
  in ground motion (rider+0x424 = 2), toward 0 otherwise; `Boarder_UpdatePose` 0x129628 draws the body at
  rider+0x140 + rider+0x308 · up (rider+0x2c0). The same function eases the spring band d1, d2 (rider+0x294, +0x298)
  toward words [7], [8] at 1.6667 cm a tick.
- Clips are 30 fps data played at 0.5 frame per 60 Hz tick × rate (`AnimClip_Advance` 0x15a030); a clip of n
  frames reaches its end after 2(n−1) ticks and chains on the next.
- Grab (and tweak) clips play at rate 0.782 + 0.806·s14.
- Rail quarter turn: a 15-frame clip, committed at frame 14 (28 ticks, 0.467 s); the clip turns the board.
- Get-up: by slide (butt/face) and direction to the course: FROMFACEFWD 30 frames, FROMBUTTFWD/BUTTRIGHT/FACERIGHT
  35, FROMFACEBWD 41, CT_ROLLBWD2BASE 51; riding resumes 2n−1 ticks after it starts.
- Clip descriptor table 0x325fa8 (7 ints per game clip id: category, lookup mode, update type, end action, layer,
  blend-in, fade-out); game clip id → name via `AnimClip_ResolveAnmIndex` 0x15ada8.

## The rider's states and motions are classes (from `cBoarderHandler::__tf` 0x11f628, which registers them)

- Controls (the state, rider+0x428): `cVoidControl`, `cBumpControl`, `cCruiseControl`, `cChangeFakieControl`,
  `cFakieCruiseControl`, `cFinishLineControl`, `cGate2BaseControl`, `cGateAnticipateControl`, `cGateLaunchControl`,
  `cJumpControl`, `cLandHardControl`, `cLandNormalControl`, `cNaturalAirControl`, `cPrewindControl`,
  `cRailSlideControl`, `cSpinControl`, `cSitAndWaitControl`, `cGetupFromSitControl`, `cWipeOutControl`,
  `cWipeOutRecoverControl`, `cLessonWaitControl`, `cResetWaitControl` (vtables: `cFakieCruiseControl` 0x366eb0,
  `cChangeFakieControl` 0x366f60, `cCruiseControl` 0x367010; the rest are L lines in symbols.txt).
- State number → class, from the boarder constructor 0x11b348 (each control object's virtual-base pointer gets its
  class's vtable) and the three dispatches on rider+0x428: `Boarder_ExitControlState` 0x11c840 (old state),
  `Boarder_EnterControlState` 0x11c8e8 (new state), `Boarder_UpdateControl` 0x11c9a0. `Boarder_SetState` →
  `Boarder_ChangeControlState` 0x11c7d8 does nothing when the state is unchanged, else exit, set, enter.

  | state | class | object (word in the control block) |
  |---|---|---|
  | 1 | `cVoidControl` | 0x180 |
  | 2 | `cBumpControl` (the stumble) | 0x184 |
  | 3 | `cCruiseControl` (riding) | 0x18c |
  | 4 | `cChangeFakieControl` (the revert) | 0x194 |
  | 5 | `cFakieCruiseControl` (riding fakie; its update is inlined in the dispatch, turn clips 0x201–0x204) | 0x198 |
  | 6 | `cFinishLineControl` | 0x19c |
  | 7 | `cGate2BaseControl` | 0x1a4 |
  | 8 | `cGateAnticipateControl` | 0x1a8 |
  | 9 | `cGateLaunchControl` | 0x1b0 |
  | 0xa | `cJumpControl` | 0x1b4 |
  | 0xb | `cLandHardControl` | 0x1b8 |
  | 0xc | `cLandNormalControl` | 0x1bc |
  | 0xd | `cNaturalAirControl` (in the air) | 0x1c0 |
  | 0xe | `cPrewindControl` | 0x1c4 |
  | 0xf | `cRailSlideControl` | 0x1c8 |
  | 0x10 | `cSpinControl` | 0x1d0 |
  | 0x11 | `cSitAndWaitControl` | 0x1f0 |
  | 0x12 | `cGetupFromSitControl` | 0x1f4 |
  | 0x13 | `cWipeOutControl` (update inlined: asks for a reset) | 0x1f8 |
  | 0x14 | `cWipeOutRecoverControl` (the get-up) | 0x1fc |
  | 0x15 | `cLessonWaitControl` | 0x200 |
  | 0x16 | `cResetWaitControl` (`Boarder_StartReset` 0x119be0) | 0x204 |

  The handlers are named `<Class>_Enter/_Update/_Exit` (older names kept: `SpinState_Update`, `PrewindState_Update`,
  `FinishState_Update`, `RailSlideControl_Update`, `GetUpState_Update`, `ResetState_Update`).
- Motions (rider+0x424): 1 `cAirMotion`, 2 `cGroundMotion`, 3 `cRailSlideMotion`, 4 `cStaticMotion` (no enter or
  exit), 5 `cWipeOutMotion`, 6 `cWipeOutRecoverMotion`; same pattern through `Boarder_ChangeMotionState` 0x11c648.
- The rider objects: `cPlayer` (human) and `cComputer` (AI) derive from `cBoarderHandler` (constructor 0x11b348,
  which builds every control and motion object), which derives from `cBoarderRender` (0x135278) over `cBoarder`
  (0x116828).
- Every control and motion class has a destructor in the vtable slot after `__tf` (`cCruiseControl::~cCruiseControl`
  0x122168 and so on). The rider's controls come from virtual 9: `cBoarder::vf7` 0x117198 (the per-tick update) calls
  it to fill a control word, then hands that to `Boarder_UpdateControl`. `cPlayer::GetControls` 0x151250 packs the
  pad into it per state (stick axes ×31 in 6-bit fields, buttons in the low bits; `Pad_GetAxis` 0x1797f8 has a 0.38
  dead zone); `cComputer::GetControls` 0x1384b8 is the AI's.
- Clip end actions (`AnimCtl_HandleClipEnd` 0x15f5b8, the 4th int of the clip descriptor): 0 stop, 1 back to the
  base clip 0x1ff, 2 play 0x223, 3 play 0x202, 4 chain 0x226–0x22d → 0x22e–0x235 (`AnimCtl_EndActionChain`),
  5 play 0x2df, 6 play 0x2de, 7 back to 0x202 on the alpine board (board class rider+0x420 = 2; G5a2 read it) else 0x1ff.
- Pad rumble (`cPlayer::vf7`/`vf11` → `Rumble_Update` 0x150d18, `Rumble_Impact` 0x150a80): an impact of
  −dot(velocity, normal)·1.389 (clamped 0..2) above 0.3 buzzes the small motor for 0.02–0.1 s and sets the big
  motor's strength, which decays ×0.98333 − 0.025 per tick; nothing during replays.

## Animation clip modes (F1c)

`AnimCtl_UpdateLayers` 0x15f390 walks the 3 animation layers (0x70 bytes each) and calls `AnimCtl_UpdateClip`
0x15f440 on every clip. That picks how the clip is driven from a per-clip table at 0x325fb0 (stride 0x1c, by the
clip id at clip+0x40), then fires the clip's sound markers (`AnimCtl_FireClipSounds`):

| mode | function | driven by |
|---|---|---|
| 0-3 | `AnimMode_Advance` 0x161340 (and three copies, `AnimMode_Advance1..3` 0x161348 / 0x161350 / 0x161358) | time |
| 4 | `AnimMode_BrakeBlend` 0x160680 | brake (rider+0x1fc) and rider+0x268, two sources per board class (tables at 0x3791e8) |
| 5 | `AnimMode_CrouchBlend` 0x1605a0 | crouch (rider+0x208), sources per board class at 0x3791d0 |
| 6 | `AnimMode_CrouchFrame` 0x161300 | frame = (1 - crouch) × length |
| 7 | `AnimMode_CrouchSteerBlend` 0x161118 | crouch and the steer sign (rider+0x214), tables at 0x3794b8 |
| 8 | `AnimMode_BoardClassFrame` 0x160140 | frame = rider+0x1f0 × length |
| 9 | `AnimMode_DirectionBlend` 0x1601d0 | angle of (rider+0x27c, rider+0x280), quadrant blend |
| 10 | `AnimMode_WindUpBlend` 0x160b20 | like 9, on the spin and flip wind-ups (rider+0x220, rider+0x22c), crouch-weighted |
| 11 | `AnimMode_GateLeanFrame` 0x160af0 | frame = rider+0x238 (the start-gate lean) × length |

Riding switch (rider+0x1b4) mirrors the inputs of modes 4 and 9.

Mode 8 picks its source by board class (rider+0x420) from the table at 0x379180 and sets frame = rider+0x1f0 ×
(length − 1), speed 1. `AnimCtl_BeginClip` 0x15f060 also calls `AnimClip_SetEndBlendOut` 0x15a330 with the clip's
blend-out frames (table 0x325fc0): n = 0 means never (rate −1, start frame 0xfffffff), else the blend-out starts at
length − (n + 1) with rate −|speed|·30/(60·n) a tick (as `AnimClip_SetBlendOut` 0x159d88). With a blend-in it first
clears the layer's "current" flags (`AnimLayer_ClearCurrentFlags` 0x15a760, clip+0x68; the new clip gets 1).
`AnimCtl_FindFirstMarkerFrame` 0x15fb08 (from `RaceScene_Select`): the frame of the clip's first marker of types 0–3,
−1 if none. `Euler_Lerp` 0x202758: a + t·(shortest delta to b), per angle; `Euler_LerpArray` 0x2027f0 does it in place
on n pairs (pose blending).
`AnimLayer_SetStance` 0x15a690 (F1c, proven): every clip in a layer gets clip+0x64 = the value; `AnimCtl_SetAllLayersStance`
0x15ef48 does it for the three layers (ctl+0x28, 0x70 apart), as `AnimCtl_TurnAllLayers` does the facing. clip+0x64 is
the controller's +0x14 (rider+0x46d4, the animation's stance) copied by `AnimCtl_BeginClip` and handed to
`PoseBlend_Add`; the rail control's quarter turn calls it with 0 after clearing +0x46d4.
`AnimCtl_GetMarkerPhase` 0x15fde0 (ctl, n, layer; F1c, proven): the phase of marker n − 1 of the clip playing in the
layer (`AnimData_GetMarkerPhase` on the clip's entry at 0x3218cc); the start gate's anticipate (n 1) and launch (n 2)
controls compare the gate lean (rider+0x238) with it.

## Meter, boost, crouch and wind-up helpers (proven, G1d; exact in `tricky-rs/game/src/boarder.rs`)

- `Boarder_AddMeter` 0x11b018: gain = units·(0.9787758 + 0.35966223·stat1d/255), passed to the rider's virtual +0x84; meter
  (rider+0x1c) += gain, clamped 0..1 (crossing 1 from below 0.85 calls 0x2191c0). A positive gain at a full meter sets the
  uber timer (rider+0x24) to 20 and calls 0x21dce0; from an empty timer it also counts rider+0x28 = (n + 1) mod 4 and
  tail-calls `Audio_TrickyTrigger`.
- `Boarder_UpdateBoost` 0x11d038 (held, start): no button or no meter → level (rider+0x130) 0, flame +0x254 = 1.5259, +0x258 = 0
  (sounds 0x2197d8 / 0x219760 when the race state allows). Boosting: the first tick sets the flame by meter (+0x258
  0.749/0.550/0.650, +0x254 2.144, fade +0x25c 1); level 1.0 / 0.6013 / 0.2500 by meter > 0.666 / > 0.3336 / else; drains
  0.0007501507 a tick unless TRICKY; the flame fades once +0x250 reaches the level's mark.
- `Boarder_SetCrouchTarget` 0x126c00: target (0 while rider+0x1fc is set) to +0x210, rate 5.002488·max(0.1, |target − crouch|)
  to +0x20c. `PrewindState_SetWindTarget` 0x105a60: wind-up [now, rate, target]: target = v (negated to flip), rate =
  r·|v − now|·(0.8124317 + 0.32901537·stat18/255).
- `Boarder_SetSteerTarget` 0x126948 (G1e): lock ±0.9051856 unless boosting; × |v|/1138.2727 (alpine), /1134.9739 (BX), /1119.0095
  (freestyle) below that speed; rate = clamp(7.0173593·|target − steer|, 0.1, 8.018349), ×0.5999726 on powder (3, 4); to
  rider+0x21c, +0x218. `Boarder_IsTricky` 0x11f430: rider+0x58a8 > 5 ubers.
- `PrewindState_UpdateWindClip` 0x1055f0 (G1e): the wind-up clip from the table at 0x35fed0 (9 rows × rider+0x1dc 0..4;
  `boarder::WIND_CLIPS`): centre when both wind-ups are 0, else by atan2(flip, spin) (negated riding switch): back past
  157.5°, then 112.5°, 67.5°, 22.5° up and −112.5°, −67.5°, −22.5° down, else forward.


## The stat bytes (proven, G20a; exact in `tricky-rs/game/src/stats.rs`, `tests/stats.rs`)

A rider's stats are 16 bytes at stats+0x0e..0x1d of its slot's entry in the race table (`gRace_Entries` 0x32db70, 0x84
a slot, count `gRace_EntryCount` 0x32db6c; `rider+0x464` points at the entry, `Boarder_SetRiderInfo` 0x116a68 sets it).
`Career_PrepareRace` 0x163980 → `Career_SetupRidersForMode` 0x165558 → `Career_BuildRaceRiders` 0x165cf0 fills the
entries, one call per slot: `Career_SetupPlayerRider` 0x16a9d8 for a human, `Career_SetupAIRider` 0x16ae68 for the
computer riders.

- the entry starts as the rider's 16 base bytes (`gRider_Table` 0x331ea8: 0x38 a rider, id in word 0, bytes +0x0c..0x1b
  the base, +0x1c..0x1f the four caps, word +0x20 the weight). Four groups of those bytes (`gStat_Groups` 0x335260: a
  count then the byte positions) are mixed; the rest stay at the base:

  | group | stat | bytes (stats+) | base bytes |
  |---|---|---|---|
  | 0 | edging | 0x11, **0x13 grip** | 3, 5 |
  | 1 | speed | 0x0e, **0x19 cubic drag** | 0, 11 |
  | 2 | stability | 0x12, 0x16 | 4, 8 |
  | 3 | tricks | 0x14, 0x1a, 0x1d (the meter's stat1d) | 6, 12, 15 |

  Bytes in no group: 1, 2, 7 (the gate), 9 (**+0x17 jump**), 10 (+0x18 wind-up), 13, 14. The bytes of one group hold
  the same base in all 12 riders, so grip is the edging value and the cubic term the speed value: the port's guess
  (grip from its edging, cubic from its speed) was the right shape, only not the integers.
- player: `avg` = `Career_AverageStatGroups` 0x16b298 (each group's base bytes, integer mean); `frac` = (level − avg) /
  (100 − avg) in floats (0 when avg is 100, may be negative), level = the career's byte for the group (career record
  +0xc..+0xf); a grouped byte = trunc(0.2·bonus + (base + frac·(100 − base))·0.8), the bonus the board's byte of the group
  (`gBoard_Table` 0x332148: 12 boards a rider, 20 bytes each, the four bonuses at +16).
  With `gCheat_MaxAttrib` (0x3352ac), the entry's +0x80 not 2 (set to 0 by the word at 0x3352b8) or the Mallora cheat on
  rider 7, every grouped byte is the rider's cap byte instead; game mode 6 (tutorial, 0x32de8c) always mixes.
- computer: no board; `frac` = min(1, (value·0.25) / (100 − avg)), the value the caller's `*param_3` (the rival level,
  null = bare base bytes); a grouped byte = trunc(base + 0 + frac·(100 − base)); the cap byte with `gCheat_MaxAttrib`.
- last, both: every byte b becomes b·255/100 (integers): 70 → 178, so the jump stat is 178, not our rounded 179.
- checked: both set-ups run in the function runner on 20,000 random riders, boards, levels, cheats and modes each, the 16
  bytes bit for bit; the tables (groups, bases, caps, weights, 144 board bonuses, trickdata's attributes) equal the exe's.
- since G20a2a: the app's player has the bytes from the career (`Career_SetupPlayerRider`'s levels are the slot's
  character record +0xc..+0xf, the block E12d2 keeps: `ui::CareerRes::levels`, `Game::levels`; the app's "master" is
  the max-attribute path, the caps), and `Boarder_AddMeter` (+0x1d), `Boarder_DrainMeter` (+0x1b), `Boarder_GetSpeedStat`
  and the shove (stability +0x12) and the jump (+0x17) read `Stats::bytes` (`stats::METER`, `LEAK`, `STABILITY`, `JUMP`).
  `Stats::default` sets those bytes from its fractions for the tests' neutral rider.
- since G20a2b: the computer riders' bytes are `stats::ai_bytes` with the value `Career_BuildRaceRiders` 0x165cf0
  passes `Career_SetupAIRider`: the last player's character record's word at +0x164 + 8 · the computer rider's
  character (how much that rider dislikes the player's; `ui::CareerRes::rivals`, `Game::rivals`), with no board. The
  wind-up (+0x18: `boarder::windup_factor`, byte · 0.32901537 / 255 + 0.8124317) and the gate start (+0x15:
  `gate::k`, byte · 0.71808594 / 255 + 0.4875571) are read from the bytes in the port's prewind and gate, in place of
  the rounded fraction forms (0.812 + 0.329 · wind-up, 0.4875571 + 0.71808594 · gate). The port's fractions (edging,
  speed, stability, tricks) still drive its own riding formulas that have no stat byte yet.
- since G3a: the port's own riding fractions (edging, speed, stability, tricks) for the player come from the same
  career levels: each group's level mixed 80% with the board's 20% over 100 (`Stats::of_levels`), equal to the old
  training-fraction ones at a new career (checked for every rider and board, `tests/stats.rs`); the app's "master"
  keeps the fully trained ones. Since G3b the computer riders' fractions come from their rival value as their stat
  bytes do (`Stats::of_rival`: each group's base average moved toward 100 by a quarter of the value, at most all the
  way, no board, as `Career_SetupAIRider`). The attribute points' spending (the front end's rider screen) stays in G3.

## The probe's sideways offset: stats+0x60 and rider+0x244 (board row G20b)

Proven (read from code, run in the function runner in `game/tests/stats.rs`):
- `Boarder_ProbeGround` 0x128ae8 and `Boarder_UpdatePose` 0x129628 both start from pos + side (+0x330) · (stats+0x60 ·
  rider+0x244); the pose scales its body offsets (15, 25, 60 and the 96/98/114.. cm of the hips) by stats+0x60 too, so
  stats+0x60 is the rider's **body scale** and rider+0x244 is a lean, in cm for a full-size rider.
- stats+0x60 is the float at `gRider_Table` (0x331ea8) + rider · 0x38 + 0x24: `Career_SetupPlayerRider` 0x16a9d8 copies
  it (`DAT_00331ecc`), and `Career_SetupAIRider` 0x16ae68 copies the same offset of the row it is given (the tests pass the
  table's own rows). The twelve values are `stats::SCALE` (0.85, 0.70, 1.00, 0.75, 0.87, 0.85, 0.90, 0.95, 0.85, 0.90,
  0.98, 0.85). The player alone gets 0.8 (`0x3f4ccccd`) when the all-max word `DAT_003352b8` is set outside mode 6.
- rider+0x244 is a three-word easing record, the same kind as the steer (+0x214, rate +0x218, target +0x21c; set by
  `Boarder_SetSteerTarget` 0x126948): value +0x244, rate +0x248, target +0x24c. `cBoarder::vf7` 0x117198 moves the value
  toward the target by rate · (1/60) a tick, whatever the timescale (`lean_ease`; the port uses rate · dt, equal at 60 Hz).
  `Boarder_UpdateLeanTargets` 0x129500, at the end of vf7, sets the rate to 66.67 (`0x42855555`) and the target to 0 in
  motion 3 (the rail) or else −60 · steer (+0x214) · min(1, |v| (+0x150) · 0.6944444) (`lean_targets`). So the lean follows
  the steer, up to 60 · 0.905 = 54 cm · scale to the side.
- Writers of rider+0x244, all of them (a scan of the exe for every store at +0x244, and for the record's base): `Boarder_PlaceAt`
  0x11cae8 zeroes the record, vf7 eases the value, `ChangeFakieControl_Enter` 0x103eb8 and `RailSlideControl_SwitchStance`
  0x107fc8 negate it with the steer. `WipeoutMotion_Enter` 0x10c198 sets the rate to 2.0 and the target to 0 (the next
  `Boarder_UpdateLeanTargets` overwrites them). Nothing else writes it.
- Motion ids (`Boarder_EnterMotionState` 0x11c710, rider+0x424): 1 air, 2 ground, 3 rail slide, 5 wipeout, 6 recover.

Ported: `stats::SCALE`, `Stats::scale`, `Rider::lean` (value, rate, target), `lean_ease` and `lean_targets` in
`rider/ground_tick.rs`, run every tick by `Rider::step` (ease first, aim last); the ground tick's probe reads
`stats.scale · lean[0]`. Zeroed in `Rider::respawn` (PlaceAt), negated in the revert (state 4).

Not done (new rows): the port's `touch_down` and rail code flip `switch` without negating the steer or the lean as
`ChangeFakieControl_Enter` / `RailSlideControl_SwitchStance` do (G20b2); the motion id is derived from the port's flags
(STANDIN, G18); the pose's use of rider+0x244 is the pose's (G16). (The other half of `Boarder_UpdateLeanTargets`, the rider's up
rider+0x2c0, is ported since G20c, next section.)

## The rider's up, rider+0x2c0 (G20c)

Proven from the code of `SLUS_203.26` (Ghidra, plus a scan of the whole text for stores and address-of at offsets
0x2c0..0x2cc):

- **Writers**: exactly two. `Boarder_UpdateLeanTargets` 0x129500 (its last store, 0x129618) every tick:
  up = unit(up + 0.5 · normal), over all four lanes with the vector unit's order (`vmulbc` the half normal,
  `vadd`, the sum of squares, `vrsqrt`, `vmulq`); the normal is rider+0x2a0. And `Boarder_PlaceAt` 0x11cae8 (0x11cdf4):
  up = normal (the four words copied) after the placement's ray cast has set the normal (the default, before the
  cast, is (0, 0, 1, 0)). `Boarder_UpdatePose` 0x129628 does not write it (its `Vec4_Scale(rider+0x308, out, rider+0x2c0)`
  reads it); no other function stores to it with that offset, and the address is formed (`addiu rX, rider, 0x2c0`) only
  by `Boarder_UpdatePose` (0x12994c, to read) and `cBoarderRender::vf7_1356f8` (0x135c40, which works on the model
  object at rider+0x4494, not the rider). Not excluded: a copy of the whole record through a pointer built another way.
- **Readers**: `GroundMotion_Update` 0x10a0d8 twice (0x10a510, 0x10a590: the dot of the position with it before and after
  the tick's move, for the bob rider+0x1c0), `Boarder_UpdateLeanTargets` itself, `Boarder_UpdatePose` 0x129628 (0x129fec,
  the body drawn at position + rider+0x308 · up, G16) and `Boarder_UpdateSkeleton` 0x12ae18 (0x12ae88, 0x12af20).
- **Order in a tick** (`cBoarder::vf7` 0x117198): `Boarder_UpdateMotion` calls `GroundMotion_Update` (call at 0x11c7a8) in
  the middle, `Boarder_UpdateLeanTargets` is the last call before the render's `vf7` (call at 0x117ce4). So the ground tick
  reads the up the previous tick's end left (or `Boarder_PlaceAt`'s), and the tick's own normal (set by its probe)
  only reaches the up at the end of that tick.
- The up is not the board's up (rider+0x1a0, `Boarder_UpdateOrientation`): it trails the ground's normal.

Ported: `Rider::up` (game space, z up, w 0), `lean_up` in `rider/ground_tick.rs`, run at the end of every tick by
`Rider::lean_aim` (with the lean record's aim), set equal to the normal by `Rider::respawn` (`Boarder_PlaceAt`) and
by `Rider::new` ((0, 0, 1, 0), flat ground); `Rider::ground_tick` gives it to `GroundMotion_Update` as `g.up`
(it used the board's up, a STANDIN, before). Checked in the function runner (`tests/stats.rs`): `lean_up_is_the_originals`
runs `Boarder_UpdateLeanTargets` over 2,000 chains of 12 ticks (each result fed back in as the next up, normals
unit, tilted, not unit length, with and without a w) and `lean_up` matches the four words bit for bit, the normal
untouched; `rider_up_follows_the_original` does the same through `Rider::lean_aim` from the port's normal (y up).

Not done (new rows): in the air the port's normal is its own, where the game's rider+0x2a0 is written by
`AirMotion_Update` 0x108378 from its down ray whenever that hits (0x108588), so the up is exact on the snow and on a
rail but not through a jump (G20c2b); the control state, the ray cast's hit kind (+0x2e0) and the attitude quaternion
across a landing are G20c3.

## The ground normal, rider+0x2a0 (G20c2)

Writers (decompile and disassembly of the three originals):
- `Boarder_PlaceAt` 0x11cae8 sets it to (0, 0, 1, 0) (0x11cc5c), then, when it is not told to skip the ray, casts one
  under the spawn (`World_RayCast` 0x25aff8) and takes the hit normal when the ray hits (0x11cd08); the up (+0x2c0)
  is a copy of it (0x11cdf4).
- `RailSlideMotion_Update` 0x10b0d0 (store 0x10bbb8), every tick the board is on the rail and inside the fall-off
  limits. The tangent T is the rail's at the closest point (`Rail_ClosestPoint` 0x259330), reversed (all four lanes)
  when the unit velocity is against it. The lean angle (object +8) starts at 0 (`RailSlideMotion_Enter` 0x10b068) and
  moves to `lateral * k1 * k2` (lateral = board right (+0x190) dot the vector to the closest point, cm; k1, k2 =
  0.6981317, 0.0125 for kinds 3 and 4, 0.34906584, 0.033333335 otherwise; negated when the board's up z (+0x1a8)
  is negative) by at most 5 per tick. Flat rail (|T.z| <= 0.92): n = unit(Z - T * T.z), turned to the side of the
  board's up (+0x1a0). Steep rail: c1 = unit(up x T), c2 = T x c1, n = c2 * cos(a) - c1 * sin(a).
- `AirMotion_Update` 0x108378 (0x108588): the hit normal of its down ray, when it hits. Not ported (needs E7).

Ported (`rider/ground_tick.rs`): `rail_lean_angle`, `rail_normal`, `placed_normal`, `rail_travel`, `rail_offset`;
`Rider::rail_lean` (reset at the capture), `Rider::rail_step` sets `normal` from them every tick on the rail, and
`Rider::respawn` sets (0, 0, 1, 0) and the up from it. Checked in the function runner (`tests/normal.rs`): the real
`RailSlideMotion_Update` runs 20,000 times with the closest point given and the vector helpers real (steep and flat
tangents, reversed travel, all four easing cases, w lanes): its normal (four words) and the lean angle equal
`rail_normal` and `rail_lean_angle` bit for bit; the real `Boarder_PlaceAt` with the ray given (hit, miss, skipped)
leaves the normal and up that `placed_normal` gives.

STANDIN: the rail's closest point and the board's right/up are the game's attitude and a world query. The port builds
the lateral offset from its own rail offset (`rail_lateral`: the board right taken as T x up, lying along the rail) and
the board's up from the last normal; the respawn has no ray under it, so it is always (0, 0, 1, 0). Gap rows G20c2b
(air normal and the placement's ray hit, E7) and G20c2c (the board's attitude as the rail's input).

## The rail normal's inputs from the game (G20c2c)

Proven (disassembly of `Rail_ClosestPoint` 0x259330, `SLUS_203.26`): the world is the object the pointer `gWorld`
0x346488 holds (the grid `World_RayCast` 0x25aff8, `AIWorld_*` and the camera's collision also read): origin x, y at
+0x04, +0x08, cell side +0x28 (cm), cells across and down +0x2c, +0x30, the number of patch records per cell +0x48,
the table of cell pointers +0x4c (row by row, null = empty). A cell: box (min x, y, z, max x, y, z) at +0x00, the
patch records (0x4c bytes each) at the pointer +0x38. A patch: box at +0x00, in use when the word +0x24, the halfword
+0x28, the curve count (halfword +0x2a), the halfword +0x2c or the word +0x48 is not zero, the curve pointers at the
pointer +0x3c. A curve: the cubic's four rows at +0x00..+0x3f (the point is `m0 t³ + m1 t² + m2 t + m3`, all four
lanes, `vmulax`/`vmadday`/`vmaddaz`/`vmaddw` order), its owner at the pointer +0x58 (bit 0 of the owner's +0x18 = the
rail is on, the owner's +0x24 word goes to the hit's +0x34), its box at +0x5c..+0x73.

`Rail_ClosestPoint`(world, pos, unused, out) walks the cells from `ix − 2` to `ix` and `iy − 2` to `iy`
(`ix = ((x − x0) + side) / side` cut toward zero, clamped into the grid), x outer and y inner, and in each cell, patch
and curve whose box is within 300 cm of the point (every test is `min ≤ p + 300` and `p − 300 ≤ max`, all three axes)
searches the curve: the start and the chords to t = ¼, ½, ¾, 1 (each chord's foot of the point, clamped to it) give the
nearest chord k (0 if the start itself wins; ties keep the earlier), then a golden-section search over the bracket
the tables at 0x3a78e0 (lower: 0, 0, 0, ¼, ¼) and 0x3a78f8 (upper: ¾, ¾, 1, 1, 1) give for k, with the ratios 0.381966
and 0.618034, stops at a width of 0.0005 (at most 24 steps). The result is the **last point evaluated**, its parameter
and its distance (not the best of the search); a curve's result replaces the best when it is the first or its
distance is smaller. The hit is the point (out+0x10), the unit tangent from the derivative basis (3t², 2t, 1, 0)
through the same rows (out+0x20, four lanes) and the owner's word (out+0x34); the function returns 1 when any curve
was taken and writes nothing otherwise. Every distance is `vsqrt` of the four-lane sum of squares, so a w lane in the
point counts.

Ported (`rider/ground_tick.rs`): `RailWorld::closest_point` (with `RailCell`, `RailPatch`, `RailCurve`, `RailOwner`,
`RailHit`) and `rail_slide_normal` (`RailSlideIn`, `RailSlide`, `RailNormal`): `RailSlideMotion_Update` 0x10b0d0 from the
closest point to the normal's store, i.e. the hit recorded (rider+0x2d0 point, +0x290 word), the fall-off test with
the blend (1 for the first 0.6 s of the rail, else rider+0x284: limits `150b + 80(1−b)`, `50`, `50b + 30(1−b)` on the
frame rows dotted with the point minus the position moved 30 cm along the frame's y row), and inside the limits the
tangent turned to the travel, the lean angle and the normal (`rail_travel`, `rail_offset`, `rail_lean_angle`,
`rail_normal`). Checked in the function runner (`tests/rail_world.rs`, both against a world grid built in the runner's
memory with random cubic curves, boxes that miss, switched-off owners, empty records and missing cells):
`Rail_ClosestPoint` 9,000 queries, its point, tangent, word and the "found" result bit for bit (and untouched output
when nothing is found); the real `RailSlideMotion_Update` with the real `Rail_ClosestPoint` 12,000 times (inside and
outside the limits, both blends, reversed travel, no rail in reach): the normal, lean angle, recorded hit and blend
equal ours, and the original calls `Boarder_SetState` 0x11f608 exactly when ours says the board is off the rail.

The rail attitude (row G20c2c2; `ground_tick::rail_attitude`, `orient::quat_multiply`, `rotate_vec_to_vec`,
`slerp`): after the normal's store (0x10bbb8) the board's wanted up is `n·cos(lean) + (n × t)·sin(lean)` (t the
travel tangent); the target quaternion is the identity turned z → that up (`Quat_RotateVecToVec` 0x2508b8: axis a × b
over its length, angle asin(min(len, 0.99999)) or π − that when a·b < 0, (cos ½θ, sin ½θ·axis) · q), then turned
from its own y axis (turned as `Boarder_UpdateOrientation` turns it) to t; rider+0x170 goes 1/12 of the way to it
(`Quat_Slerp` 0x250a48: dot summed x, y, z, w; the shorter way; π/2 − asin(dot) and three `Math_Sin`, or linear
when 1 − dot ≤ 0.001), then `Boarder_UpdateOrientation`. `Quat_Multiply` 0x102e80 sums each lane left to right
(w: ((p0q0 − p1q1) − p2q2) − p3q3; x: ((p0q1 + p1q0) + p2q3) − p3q2; …). Checked: the three routines alone
(30,000 cases each, near-equal and opposite inputs) and the quaternion, axes and heading the real
`RailSlideMotion_Update` leaves in the rail cases above, bit for bit (`tests/rail_world.rs`).

The whole rail tick (row G20c2c4; `ground_tick::rail_slide_motion`, `RailMotion`, `RailEnd`), dt = rider+0x12c / 60:
- k = 0.5 under 555.55554 cm/s, else speed × 0.0009; no rail in reach: take off.
- Outside the fall-off limits: pushed 277.77777 cm/s off along the unit offset to the rail (the tangent when
  |offset·t| > 0.5), squared to the velocity first when the velocity runs against it; take off.
- Inside: time += dt; position += up · clamp(offset·up, ±8.333333); velocity = t(v·t)·(dt·30) + v·(1 − dt·30);
  gravity (0, 0, −980) along t, minus t(v·t)·0 (sic), plus t·+0x130·±2450.8232 (the sign of v·t) when +0x130 > 0,
  × dt into the velocity; position += v·dt. Balance along the board's right × dt: steer·(clamp(k, 0.5, 2)·145.07674)
  (steer = +0x214, negated unless the board's up points down) − lateral′·(gain·clamp(1/k², 1, 4)); gain −5 for the
  first 0.6 s (after this tick's dt) or on a steep rail (|t.z| > 0.92), else 1.8153523, capped at −1 within
  2.5000005 of the rail; lateral′ clamped ±20 only when the gain is not negative; upside down within 20 of the rail
  the force is at least 290.15347 back toward it. Then the lean angle, normal and attitude; rider+0x494 = 3 around
  `Boarder_InstanceCollisions` 0x125088 and `Boarder_WallCollide` 0x126250, then −1.
- Take off: +0x214, +0x218, +0x21c = 0; `Score_StartTrickChain` 0x156618(rider+0x5820, rider+0x1b4 ≠ stats+0x40,
  +0x1dc, 0, 0) → `Boarder_AddMeter` 0x11b018(its result); `Takeoff_Setup` 0x12bee0; `Boarder_PlayTakeoffClip`
  0x127298; motion 1; state manager +0x798, +0x76c, +0x770 = 0; state 0x10.
Checked against the real function on 12,000 random rail ticks (`tests/rail_world.rs`): every field and call bit
for bit.

Not wired, new rows: `Rider::rail_step` still uses `rail_lateral` and the last normal (STANDIN), because it has no
world grid (the game builds one when it loads the course; the port's rails are the level's splines flattened to
polylines in `rails.rs`: G20c2c1) and no attitude quaternion through a rail (the second half of
`RailSlideMotion_Update`, 0x10bbb8 on, turns the quaternion rider+0x170 a twelfth of the way (`Quat_Slerp` 0x250a48,
rate `0x3daaaaab`) to a target made from the normal, the tangent and the lean angle with `Quat_RotateVecToVec`
0x2508b8 (read from the decompile, not ported or run yet), then calls `Boarder_UpdateOrientation` 0x129008, which makes +0x190 and +0x1a0 for the next tick: G20c2c2
ports it, G20c2c3 wires it with the attitude across a landing, G20c3). The rest of `RailSlideMotion_Update` (velocity
blended to the tangent at 30 per second, gravity along it, the balance, the push off the rail outside the limits,
the take-off call chain) is hand-written in `rail_step`: G20c2c4, with G1b.

### The grid from the course (row G20c2c1a; `course_ground::rail_world`, checked: `tests/railgrid.rs`)

`CourseGround::rails` is the grid `Rail_ClosestPoint` walks, built from the course's files at load. It uses the same
origin, cell side and size as `World_RayCast`'s grid. Its parts:
- **Cells:** each cell is a `.ltg` TGroup, with that TGroup's box.
- **Patches:** each TGroup's 16 INodes are the patch records, with the INode's box and its in-use words (+0x24,
  +0x28, +0x2c, +0x48) as the file has them.
- **Curves:** each INode's spline list (count at +0x2a, offset from its TGroup at +0x3c) gives indices of `.pbd`
  spline segments. A curve is that segment's 16 words (the cubic's rows), its box (words 0x17..0x1c) and its spline
  (+0x58) as the owner (the spline record's +0x18 and +0x24).

The game turns these indices into pointers after loading; the port keeps the records. The rail's on bit (+0x18
bit 0) is clear in every spline record on the disc; `cRailNode` (0x1403c0) sets it at run time when the rail comes
on. In the port, `RailWorld::set_live(spline, on)` does that. Each curve carries its spline's number for it.

Checked on the disc on gari and elysium:
- Every INode spline-list entry is a segment index whose box meets the INode's box.
- With every rail switched on, `RailWorld::closest_point` finds points at t = 0.1, 0.5 and 0.9 on every segment
  within 2 cm (1626 of 1626 on gari, 1326 of 1326 on elysium).

Mutants: the offset, count, box words, row order, grid size and origin, and the on bit are all caught. Zeroing the
in-use words survives, and those mutants are equivalent: the walk skips a patch only when the words are zero *and*
it has no curves, and a patch with no curves finds nothing either way.

Compared with the game's own (row G20c2c1b2, `tests/railgrid_game.rs`): `Course_LoadRenderData` 0x25edf0 resolves
the loaded files through the course object's (0x346488, vtable 0x3a7d50) virtual +0x2c,
`cCourseResolve_Common::Resolve` 0x252350(object, `.ltg`, `.pbd`). Its virtuals +0x5c and +0x3c resolve the `.pbd`
and then the `.ltg` against it (headers, materials, models, meshes, animations, instances, splines, segments,
cameras, the hash table, the TGroups and their INodes), turning the offsets into pointers in place.

Run in the function runner on gari's and elysium's own files, with every spline switched on, the real
`Rail_ClosestPoint` 0x259330 on that resolved world equals `RailWorld::closest_point` on this grid. Three points
were taken on each rail segment, spread 40 cm (or 300 cm); all 1,626 queries on gari and 1,326 on elysium match
bit for bit (point, tangent, word). The builder's mutants are caught, except the four equivalent in-use words.
The wiring into the capture and slide is rows G20c2c1b1 and G20c2c3.

## State changes (row G18; `states.rs`, checked: `tests/states.rs`)

`Boarder_SetMotion` 0x11f610 and `Boarder_SetState` 0x11f608 are one-instruction jumps to
`Boarder_ChangeMotionState` 0x11c648 / `Boarder_ChangeControlState` 0x11c7d8 with a0 = the state manager
(rider+0x5ae0, whose first word is the rider). Nothing when the value is already set; else the old state's exit
routine (the field still old, a1 still the new value), the store (rider+0x424 motion, +0x428 control state), then the
new state's enter routine with a1 = the old value. Each routine gets its object in the manager: motions 1 air +0x480,
2 ground +0x4b0, 3 rail +0x4c0, 5 wipe-out +0x4f0, 6 recover +0x5f0 (0 and 4 none). Control states: exits for 2, 3,
4, 5, 8, 0xb, 0xc, 0xe, 0xf, 0x10, 0x14, 0x16; enters for 2, 3, 4, 6, 7, 8, 9, 0xc, 0xd, 0xe, 0xf, 0x10, 0x13,
0x15 (objects +0x610 … +0x810, see `states.rs`); entering 0x16 (reset) zeroes the reset timer (manager +0x818)
inline. Checked for every old and new value 0..0x20 of both.

The state routines (row G18a): `states::run_routine` has 28 of the small enter/exit routines (field resets, the
boost off, clips, the sound flag at rider+0x5ae4 cleared through 0x218b68 → 0x11f440, which also sets +0x5af0 = 5;
`ChangeFakieControl_Enter` flips the stance +0x1b4, turns the board π about its up, sets the animation's facing
+0x46e0 to the stance's 0 or π wrapped, negates steer +0x214 and lean +0x244, and plays clip 0x200 (0x1ff for an
alpine board in its natural stance); `FinishLineControl_Enter` marks a crossing at 277.77777 cm/s or slower).
`SpinControl_Enter` (`spin::spin_enter`): rates × 1.6 when rider+0x138 > 0, × 0.8 against the natural stance
(stats+0x40); bands (rate, 0.3·rate, |rate| up to 1.3962634; swapped for a negative rate); mode 0 if the layer-0
clip type is 0xb, else 2; the landing alignment `SpinControl_CalcLandingAlign` 0x11db00 (`spin::landing_align`:
the turn conj(q)·target, the target lying on the slope normal (the slope with z × 1.5) with the board's forward
kept; off alpine boards the target without the forward turn too, the smaller wrapped angle wins; none over slope
kinds 6 and 10), dropped when bigger than 2.0943952 × the normal's z (at most 1.3962634) on the ground or with the
board's up above level. Checked: 20,000 routine runs and 40,000 spin entries against the original.
`SpinControl_Exit` (`spin::spin_exit`): with spin or flip left, q ← q · (cos −s/2, 0, 0, sin −s/2) · (cos −f/2,
sin −f/2, 0, 0), renormalised (`Boarder_UpdateOrientation`), and the position moved by the new attitude turning
(p turned back by that turn − p), p = (0, 0, the pivot bone's z) (bone = rider + +0x44d8 × 0x394 + 0x8c8); spin and
flip then 0. Checked 40,000 times.
`LandNormalControl_Enter` (in `run_routine`): only past 0.25 s (object +8); the layer mask +0x46dc = bits
(b + 1 … b + 12) of the pivot bone's +0xb50 word b (0 with no bone); the travel flattened onto the board's up:
within 30° of the board's forward (|dot| > 0.866) clip 0x20c, else 0x20e when it goes to the board's left (side ·
travel ≤ 0, mirrored riding switch), else 0x20d; played under the mask, which is then set to −1.
`RailSlideControl_Enter` (in `run_routine`): object +8, +0xc, +0x10 = 0, `Score_UpdateSpin`(0),
`Boarder_SetBrakeTarget`(0), rider+0x1f4 = 3.3333333, +0x1f8 = 0, the clips blended out. An alpine board in its
natural switch stance (stats+0x40 ≠ 0) has rail kinds 1 and 2 swapped. Kind 4 / 3 (across): the stance made regular
(alpine natural-switch: switch), the board turned ∓π/2 / ±π/2 about its up, the layers +π/2 (kind 4) / −π/2 (kind
3), the facing +0x46e0 the board's turn; clip 0x257 / 0x256. Kind 2: the stance made switch (alpine natural-switch:
regular; +0x46d4 then 0 / 1), facing −π or 0, clip 0x255. Others: regular (alpine natural-switch: switch), clip
0x254. A stance change is `RailSlideControl_SwitchStance` 0x107fc8 (stance flipped, board turned π about its up,
steer, lean, +0x27c and +0x280 negated, +0x46d4 = stance) with the layers turned π (not for kinds 3 / 4 off an
alpine board). Then `Boarder_OnClipQueued`. The facing values are the compiler's folded wraps (π → −π).

## Rail capture and the rail control (row G1b; checked: `tests/rail_world.rs`, `tests/rail_control.rs`)

`Rail_TryCapture` 0x125cb8 (`ground_tick::rail_try_capture`): the closest rail point to the board; not when the
board has passed it (offset · travel < −0.2), not during a clip of type 0xe/0xf/0x10 without flag 4, and only within
0.9 of the fall-off limits. The velocity goes along the travel tangent at min(speed, max(555.55554, speed along
it)), z × 0.1; the rail kind from atan2(z row · t, x row · t): |a| > 150° 2, a < −30° 4, a > 30° 3, else 1; the rail
word to rider+0x290 and the sound manager +0x6e40; `Score_LandTrick`(kind, stance, 0), `Boarder_AddMeter`,
motion 3, state 0xf.

`RailSlideControl_Update` 0x1073e0 (`rail_control::update`): boost from pad bit 4; the armed flag (+0x10) set by pad
bit 1, then kept while bits 1 or 2 are held. With no quarter turn queued: no turn axis (pad bits 17..22) and no bit
3: steer target (bits 5..10 / 31, unscaled), crouch target (bits 11..16 / 31), and the prewind (state 0xe) when
armed; else the crouch target 0 and, uncrouched, a quarter turn the way of the turn axis (or the steer).
`RailSlideControl_CommitQuarterTurn` 0x107730: layers ±π/2, ±90° spin scored, by rail kind the facing, stance and
clip (0x258..0x25f) and the clip to queue (+8); `RailSlideControl_PlayQueuedClip` 0x107628 plays it when the turn's
clip ends and sets the new rail kind (1→4, 2→3, 3→1, 4→2). Pad bit 0: reset at 101.46.

### A rider that is down is not captured (row E8m; `tests/physics.rs`: `a_rider_put_down_in_the_air_is_not_captured_by_a_rail`)

**Proven (decompiler, SLUS_203.26).** `Rail_TryCapture` 0x125cb8 reads no wipe-out flag of its own: its refusals are the
offset test, the clip-type test and the fall-off limits above. It has exactly two callers (`get_xrefs_to`):
`AirMotion_Update` 0x108378 (the call at 0x1089f0) and `GroundMotion_Update` 0x10a0d8 (0x10ae78). The wipe-out state
never reaches them: `Boarder_Wipeout` 0x11d830 calls `Boarder_SetState`(0x13) and `Boarder_SetMotion`(5) at once
(`Boarder_SetMotion` 0x11f610 exits the old motion and enters the new in the same call, no deferral), and
`WipeoutMotion_Update` 0x10c2f0 and `GetUpMotion_Update` 0x1102c8 do not call `Rail_TryCapture`. So the game refuses a
downed rider by structure (the motion that captures is not the one running), not by a test.

**The way back into capture is the fall recover.** `WipeoutMotion_Update`, in the air with the body upright to the landing
(dot of +0x1a0 and +0x57e0 over 0.9, speed under 4): clip 0x224, `Boarder_SetMotion`(1), `Boarder_SetState`(0xd), a
rider in the ordinary air motion again, which `AirMotion_Update` may then capture on a rail. The port's
`Rider::end_wipe(.., CLIP_FALLRECOVER, false)` is that exit (`crashed` = 0, airborne).

**The port had one slip, now fixed.** `Rider::step_tick` runs the wall check before the rail check; an air wall put the
rider down (`crashed` = 1) and the rail branch under "dropping onto a rail from the air" then locked the board on and
set `crashed = 0.0` in the same tick, so a downed rider "got up" on the rail. Now the branch needs `crashed <= 0.0`
and no longer writes `crashed`. A rider already down on an earlier tick never reached the branch (`wipe_step` returns
first). Inferred, not shown: the E8l run's capture 0.5 s after the wipe-out was most likely the fall-recover exit above,
which the game allows (the sim run after the fix is unchanged: 235.0 s, 5 wipe-outs).

**Not ported (new rows):** the clip-type gate (a layer-0 clip of type 0xe/0xf/0x10 without flag 4 refuses a capture;
which type clip 0x224 has is not read; ported in row E8n, below). The order of the wall check against `Rail_TryCapture` inside the game's tick was
row E8o, below: **the "down first" reading above was wrong for a rail**, the game captures first.

### The order of the air tick (row E8o; `tests/physics.rs`: `the_air_tick_captures_on_a_rail_before_the_wall_puts_the_rider_down`, `a_landing_on_the_tick_is_before_both_the_rail_and_the_wall`)

**Proven (decompiler and disassembly, SLUS_203.26).** `AirMotion_Update` 0x108378, in the order it runs:

1. `Air_IntegrateRK4` 0x12b340 (call 0x1083d4), `AirPredict_Tick` 0x123ec8 (0x1083e0), the air's rail-index bookkeeping
   (+0x57f0 into +0x290/+0x29c).
2. **The landing probe**: a `cWorldLine` (0x25d7f0) of 300 cm (from -100 to +200 cm along a direction made from the
   motion record at +0x8c8, negated and carried through the matrix at +0x4aa0; inferred: the down axis), `World_RayCast`
   0x25aff8 (0x10856c). On a hit (`f0 >= 0`) it stores the hit normal in +0x2a0 and takes
   `dot(normal, velocity)`; going in (`< 0`) it moves the board by the predicted attitude, runs `Boarder_ProbeGround`
   0x128ae8 (0x108934) and, when the probe is good and the height test at +0x1bc passes (0x108998..0x1089bc), clears
   the "not landed" flag (`s6`). Anything else leaves it set.
3. **Landed** (`s6` = 0, branch 0x1089e4 to 0x108e58): `FUN_00123f00`, `Landing_CheckAngles` 0x12ba78, then
   `Boarder_Wipeout` or `Landing_ChooseState` 0x109308 + `Score_LandTrick` 0x1569b0 (0x1090a0), `Boarder_AddMeter`, the
   celebrate / speech calls, `Sfx_UpdateAir`. **No `Rail_TryCapture`, no `Boarder_InstanceCollisions`, no wall ray** on
   that tick.
4. **Not landed**, in this order: `Rail_TryCapture` 0x125cb8 (0x1089f0), `Boarder_InstanceCollisions` 0x125088
   (0x1089f8), then the wall ray (a `cWorldBoarder` 0x257810 on the collision body at +0x470, mask +0x494 = 0xcf,
   `World_RayCast` 0x25aff8 at 0x108a2c). `AirMotion_Update` does **not** return after a capture: a successful
   `Rail_TryCapture` has already called `Score_LandTrick`, `Boarder_SetMotion`(3) and `Boarder_SetState`(0xf) (the
   motion switch is immediate, E8m), and the two collision steps still run on the same object.
5. The wall ray, on a hit (`f20 > 0`): `Boarder_Translate` 0x1267a0 moves the board by `1.1 * ray * hit` (0x108a7c); then
   `f = dot(hit normal, velocity)`. `f >= 0`: nothing more. `f < 0`: with `t = min(0.9, 0.6 + 0.3 * (-f * 0.00045))` (0.9
   when `-f * 0.00045 > 1`), if `dot(hit normal, +0x4ac0 axis) > t` the board glances: its quaternion (+0x170) is turned
   toward the wall by at most 0x3dd67750 rad (`Boarder_UpdateOrientation` 0x129008) and the normal part of the velocity
   is taken off (`vel -= f * normal`, no bounce); else `Boarder_Wipeout` 0x11d830 + `Score_Crash` 0x156de0 +
   `Boarder_AddMeter`. The mask +0x494 goes back to -1 at the end (0x108e38).
6. `Boarder_WallImpactCheck` 0x126540 is called from **`Boarder_InstanceCollisions`** (0x12552c, sphere-tree instances,
   only when the velocity runs into the contact normal) and `Boarder_InstanceBounce` 0x125c0c, so in the air only for
   *instances* (rollers, props), not the static world. **`Boarder_WallCollide` 0x126250 is never called in the air**:
   its four callers (`get_xrefs_to`) are `GroundMotion_Update` 0x10aa2c, `RailSlideMotion_Update` 0x10c00c,
   `GetUpMotion_Update` 0x110738 and `WipeoutMotion_Update` 0x10c7e4.

This is the order of the checked port `air_motion::update` (`game/src/air_motion.rs`, row E8a3: `probe_ground`, then
`rail_try_capture`, `instance_collisions`, `wall_ray`), which is not wired into `Rider::step` yet (G1a5b3 / G20c2b); the
glance threshold's axis there is the board's up (+0x4ac0).

**What follows for a rider over a rail that meets a wall.** If it is not landed that tick it is **captured first** and
then hit by the wall ray: the rail landing is scored (`Score_LandTrick`(rail kind, stance, 0)), then the wipe-out and
`Score_Crash`. The end state is the E8m one (down, motion 5, no rail), but the capture happened and its score stands. If
the landing probe puts it down on that tick, neither the rail nor the wall is reached. The test fixes both.

**Ported (`Rider::step_tick`).** The air branch now runs: integrate and rotate (the port's own jump release aside), the
landing (`world.ground` at the integrated position, *before* any wall push), the rail capture (`self.pos`, the speed
going in), then the air walls (`Rider::walls`, from where the capture left the board). A landed or already-down rider gets
no air wall that tick. The port's landing used to come *after* a wall push-out of the position; it now takes the position
the integration gave, as the game's ray does.

**Where the port's own code cannot follow (new rows).** The port's wall is a sphere against the drawn mesh
(`Rider::walls`, STANDIN, row E8b), not the game's ray from the collision body: its `else` bounce branch (`vel += n *
(vin + max(0.5 vin, 0.556))`, from `Boarder_WallCollide`) and its gates (`vin > 1`, `n.y < 0.7`, `air_time > 0.15`) have no
original in the air, where a hit with the velocity into the wall always glances or wipes out (item 5), and the glance
threshold takes the +0x4ac0 axis, the port's `up`. The instance contacts (`Boarder_InstanceCollisions`) are not in the
port's air at all (row E7). The landing probe is the port's `world.ground`, not the game's ray plus `Boarder_ProbeGround`.


## The attitude at a placement (row G20c3a; checked: `tests/placement.rs`)

`Boarder_PlaceAt` 0x11cae8 builds the rider's attitude (the quaternion rider+0x170 and the axes rider+0x180 forward,
+0x190 side, +0x1a0 up, the heading +0x1b0) in three steps (`orient::placed_attitude`), proven from the disassembly:

1. `Boarder_ResetOrientation` 0x128e20 (`orient::reset_orientation`): `Vec4_Set` 0x10c120 gives (1, 0, 0, 0), then
   `Boarder_RotateAboutAxis` by −π/2 (`0xbfc90fdb`) about (0, 0, 1, 0). The board points along +x: heading 0.
2. `Boarder_RotateAboutAxis` by the first argument (f12, the heading in radians, the angle of the forward from +x)
   about (0, 0, 1, 0) (0x11cb7c).
3. Only when the last argument is 0 and the placement's ray (`cWorldLine` 0x25d7f0 from 2 cm above the place to
   `height` + 200 below, `World_RayCast` 0x25aff8, result > 0) hits: the normal is stored at +0x2a0 and the board is
   turned about the axis c = normal × board up (`vopmula` / `vopmsub` on (normal, up): c.x = n.y·up.z − up.y·n.z …, w
   cleared) with s = |c| (`vsqrt`). Skipped when s ≤ 1e-5 (`0x3727c5ac`); else the axis is c · (1 / s) (`vdiv`, `vmulq`:
   from the unclamped s), s is held to 0.99999 (`0x3f7fff58`) and the angle is −`Math_Asin`(s) (0x11cdb0). The
   rotation never takes the angle past 90° (asin only): a hit whose normal points down still turns the short way.

With the last argument non-zero (`Boarder_PlaceAt`'s alternate placement) there is no ray and step 3 does not run.
The first argument is the game heading, proven for the callers below (row G20c3b); the port's `Rider` keeps a yaw, and
`rider::placed_heading(yaw)` = wrap(yaw + π/2) is the conversion (the port's forward (−sin, 0, −cos) in x, y up is the
game's (x, −z, y), the angle atan2(cos yaw, −sin yaw)).

Checked bit for bit against the real function in the runner (6,000 placements: every branch, hits that tilt the board
and ones behind it, horizontal normals that reach the 0.99999 hold, normals a hair off the up near 1e-5, no ray, the
flag): the quaternion, the three axes and the heading.

### Who calls `Boarder_PlaceAt`, and with which heading (row G20c3b; proven from the decompiler)

`Boarder_PlaceAt`(f12 heading, f13 height, a0 rider, a1 &place, a2 &velocity, a3 flag). Six callers (the node `Update` 0x141cd8 and `CamScript_StartScene` read in row G20c3b3a):

| caller | heading | place, velocity, flag |
|---|---|---|
| `ResetState_PlaceRider` 0x119220 (0x119b70) | atan2(dir.y, dir.x), the path's direction at the respawn point (`AIP_GetDirectionAtDistance`), unless the spread moved the rider to a neighbour's direction (within 30°), then that one's | place +0x350, velocity dir.xy × 833.3333 (z 0), flag 0, height the caller's |
| `Boarder_StartReset` 0x119be0 | none: it only stores the height (reset state +0x81c) and the distance to move on (+0x820), then state 0x16; `ResetState_Update` places later through `ResetState_PlaceRider` | |
| the start line, `cBoarder::vf8` 0x116ab8 (0x116d88) | atan2(dir.y, dir.x) of the path's direction at distance 0 | path point (+0xc) + dir × `Boarder_StartLineOffset` 0x11ea00 (stats+0x60 · 90 − 90 cm), velocity 0, height 0, flag 0 |
| `Boarder_TeleportToInstance` 0x11a098 (0x11a2fc) | atan2 of the instance matrix's y row (x, y) after `Matrix_Transform` | the matrix's translation + 300 cm along an offset chosen by rider+0x460, velocity 0, height 0, flag 0 |
| `RaceScene_UpdateCastClip` 0x16daa8 (0x16db00) | the scene's own float (+0x30) | the scene's place (+0x20), velocity 0, flag 1 (no ray, no `AnimCtl_Reset`) |
| the node `Update` 0x141cd8 (0x141e90): while a rider's flag byte (node +0x64 + j) is set and the rider is in state 5 (rider+0x424) | the rider's own heading (rider+0x1b0) | place rider+0x140, velocity rider+0x150 (its own, copied), height 0, flag 0: re-places the rider where it is; the rider's timer (+0x148) must have run out first |
| `CamScript_StartScene` 0x176850 (0x176ea0), for an actor record with +0x18 set | the record's heading (+0x2c), or for a record whose position comes from the scene (+0x1c) `Math_AngleDiff(heading + the scene's offset, 0)` | place the record's position (+0x20..+0x28, w 1.0) or the scene's point (`0x172100`), velocity (0, 0, 0, 0), height 0, flag 1 (no ray); `0x16da10` then starts the actor's animation with the same position and heading |

Every heading read is the same inline atan2 (±π/2 on the y axis, `Math_Atan` 0x2510f8 over y/x, ±π by the sign of x):
in (−π, π], and with the approximate `Math_Atan`, **not** `f32::atan2`. Compared with `Rider::respawn(at: (pos, yaw))`:
the port's yaw is `atan2(−x, −z)` of its own line (`race.rs`), so `placed_heading(yaw)` is the same angle to within
1.3e-5 rad (the game's `Math_Atan` error; measured on 20,000 directions, `tests/placement.rs`), exactly the same
only when the yaw is made with `jump::atan2_inline` from the game's direction (gap, row G20c3b2).
Row G20c3b2 takes that route: `Rider::respawn_heading(pos, heading)` places the rider at the game's own heading and
keeps it exact (the attitude `Rider::att` is made from it; the port's yaw is only derived), and the race's resets
(`race.rs`: the inline atan2 of the line's direction in the game's axes, x and −z) and the teleport pads (G20c3b3's
heading) call it. `tests/placement.rs` (`respawn_at_a_heading_keeps_it_exact`) checks the seeded attitude against the
real `Boarder_PlaceAt` for 3,000 headings, and that the old trip through the yaw does move some of them. The other
respawns (the safe point, the wipe-outs' return points) still pass the port's yaw.

### The placement's ray at a respawn (row G20c3b4)

- `Boarder_PlaceAt` with flag 0 casts `World_RayCast` with a `cWorldLine` (0, the place less (height + 200) cm in z, the
  place plus 200, kind 2) and, when it hits (t > 0), puts the rider at the hit point (+0x140) and takes the hit's
  normal for +0x2a0 / +0x2c0 and the board's tip (`orient::placed_attitude`'s hit). `Rider::respawn` has no world,
  so it leaves `Rider::place_ray` (the place, the game's heading) and the rider's next `step` casts it on the course
  (`ground_tick::placement_ray`). With no hit, or no course (a levels folder), the port keeps its own placement: 1.0146
  m above the place, flat, dropping (the game keeps the place itself there).
- Seen on elysium with the disc: the race line's respawns (1 m above the line) hit and stand on the course with its
  normal; a respawn at the port's safe point recorded mid-air (22 m up) finds nothing within 2 m, as the game's would.

### The air's rail capture on the grid (row G20c2c1b1; checked: `tests/rail_capture.rs`)

With the course loaded, `Rider::step`'s air capture is the game's `Rail_TryCapture` 0x125cb8
(`ground_tick::rail_try_capture`), run every air tick on `CourseGround::rails`. It reads the rider's velocity, the
board bone's frame and position, and the fall-off blend the last rail left (`Rider::rail_blend`, +0x284). It decides
whether the rider is caught, the rail kind (`rail_kind`, +0x1dc), the new velocity and the rail's word
(`rail_word`, +0x290).

The port's old gates stay only without the course: the cooldown, the time in the air, the upward speed and the
reach. After a capture the slide still runs on the port's polyline of the rail (the nearest point,
`Rails::nearest`; STANDIN, row G20c2c3).

The grid's on bits follow the event's scripts: the app copies each rail's `on` into `CourseGround::set_rail_live`.
At load every rail is on, as the port's `Rails` are (STANDIN: in the game they are off in the file and their
`cRailNode`s switch them on, row E7b2).

The board bone's frame (rider+0x4a60..+0x4a90) is not the board's pose frame (+0x4aa0, `pose::board_frame`).
`Boarder_UpdateSkeleton` 0x12ae18 copies it from the skeleton's board bone, and the skeleton is not run (row G5).
The rail code fixes how its rows are read: the kind is the tangent's angle from the x row toward the z row, so x
is along the board and z across it. The y row's sign comes from the rail tick (row G20c2c3):
`RailSlideMotion_Update` holds the bone on the rail itself. With y up, the capture's window (30 cm along y, ± 45 ×
0.9) takes a board from 15 cm over the rail to 75 cm under it, and a board falling onto the rail passes the "not
past it" test (offset · velocity ≥ −0.2). With y down, it would catch boards up to 75 cm over the rail, which the
tick then lowers at most 8.3 cm a tick, and refuse falling ones.

`Rider::board_bone_frame` is therefore the pose frame's rows reordered (STANDIN): x = its forward (y), y = its up
(z), z = its side (x), at its position, with the trick's spin and flip left out. Two parts are a convention: the z
row's sign only decides which of kinds 3 and 4 a board turned across is, and the y sign was chosen as above. The
test cannot settle either without the skeleton.

Checked on gari's spline 7 from the disc: a rider in the air 10 cm over the rail's middle, flying along it at
15 m/s and falling at 3 m/s, is caught along (kind 1) with the game's velocity: the speed along it (at most the
speed, at least 555.56 cm/s if that is more), z × 0.1. Turned across it is kind 3, turned round it is kind 2.
40 cm over it, or with the rail switched off, it is not caught. Mutants: the bone's y sign, the clip gate, the
kind, the velocity and the switch are all caught.

### The slide on the grid (row G20c2c3; checked: `tests/rail_capture.rs`)

A rider caught by the game's capture (`Rider::rail_grid`) runs each rail tick as `RailSlideMotion_Update` 0x10b0d0
itself (`Rider::rail_step_grid` → `ground_tick::rail_slide_motion`), on the course's grid. It is given the rider's
position and velocity (cm, z up), the kept attitude and its axes (+0x170..), the board bone's frame
(`board_bone_frame`, as for the capture), the rail kind, the fall-off blend (+0x284), the normal (+0x2a0), the last
rail point and word (+0x2d0, +0x290), the tick (`dt` · 60), the steer (+0x214), the lean angle and the time on the
rail. Everything it leaves goes back to the rider; the attitude is now the game's through the rail, and the yaw
comes from its heading. When it takes off (no rail in reach, or outside the fall-off limits after the push away),
the rider leaves as before.

What runs around it is still the port's (STANDIN, the rail control `RailSlideControl_Update` 0x1073e0 is row G1):
- boost pressed or held is the push +0x130, at the boost level, spending the meter as before;
- held boost with left or right turns the drawn body a quarter (`rail_twist`) and scores the spin;
- a released crouch hops off along the attitude's up.

`rail_lateral` and the port's polyline are used only for a rail caught without the course.

Checked on gari's spline 7:
- After a capture 10 cm over the rail, 20 ticks with nothing held stay on the rail. The bone is held on it within
  0.01 cm (0.9 cm aside).
- The blend is 1 and the time counts 20/60 s. The board's forward turns to the rail's slope (z 0.059 against
  0.068), and the normal is up.
- It travels 384 cm (the speed × 20 ticks).
- Held steer balances it off after 46 ticks. Nothing held drifts off after 59: the pull to the middle ends at
  0.6 s, as in the game.
- Mutants: the tick length, the steer, the position, the motion's time, the attitude, the take-off and the switch
  to this tick are all caught.

### The air's wall on the course (row G20c3c3; checked: `tests/air_wall.rs`, `tests/air_motion.rs`)

The wall part of `AirMotion_Update` 0x108378 is now two functions, `air_motion::wall_push` and
`air_motion::wall_bump`; `update` calls them, and its runner test still matches. They do this:
- on a hit (fraction > 0) the rider is pushed out by the wall's normal × 1.1 × the fraction;
- moving into the wall, a glance (normal · the board's up, the pose frame's z row, over 0.6 + 0.3 · (−vn ·
  0.00045), at most 0.9) turns the attitude about up × normal by asin of its length, at most 0x3dd67750 rad
  (0.1047), and removes the velocity's part into the wall;
- anything else is a crash.

With the course loaded, `Rider::step`'s air wall is now the game's (`Rider::air_wall`). It casts the rider's body
(`cWorldBoarder`(body, 0), mask 0xcf as rider+0x494) at the course and hands any hit to `Rider::air_wall_hit`. That
pushes the rider, then turns the kept attitude (`Rider::att`) and the velocity on a glance, or puts the rider down
as the port's air-wall wipe-out on a crash. The port's sphere on the drawn mesh (`walls`) stays only without the
course. STANDIN: the body is on the bind pose (`bind_body`, row G5).

Checked on gari:
- **A real wall:** an upright rider flying into one of its walls is put down ("air wall") by the body's real cast.
- **A glance:** a slope (normal z 0.83) hit 5 cm deep, handed to `air_wall_hit`, pushes the rider 5.5 cm along the
  normal. The attitude turns 0.1047 rad toward it (0.5891 → 0.4844 rad off the normal) and the part into it goes.
- **Why the glance is handed in:** over a plane slope the body's lowest spheres are nearest the slope, and the
  board lands (the down ray) before the 0xcf spheres reach it. A searched grid of 4,500 flights found no natural
  glance; one needs an edge or a lip.
- **Mutants:** the push, the glance's attitude and velocity, the crash and the switch to this path are caught. The
  mask survives, because both sets of spheres reach that wall.

### The landing as the game decides it (row G20c3e; checked: `tests/landing_game.rs`)

The landing branch of `AirMotion_Update` 0x108378 is the exact form of the port's hand-written landing. Its two
speed losses use the same constants as `touch_down`, and its roll range is `Stats::crash_pitch`'s limit. In order:
- `Landing_CheckAngles` 0x12ba78. A refusal (the clip early out, or surface 0, 6 or 10) or an unclean roll returns
  0, and the rider crashes.
- Otherwise `Landing_ChooseState` 0x109308 runs with the check's two angles: the angle from the board to the travel
  (its "pitch", 0..π) and the roll about the normal.
- Then the speed is scaled by the roll (clamped 15°..50°: × 1.0643 − 0.2454 · it) and by the angle to the travel
  (25°..80°: × 1.1136 − 0.2603 · it). Landing backwards (angle over π/2) rides switch.

`Rider::land_as_game` runs these for a rider landing on the course with its attitude kept through the air. It reads
the board's forward and up (+0x180, +0x1a0), the velocity going in, the snow's normal and surface, and the stability
byte stats+0x16 (`bytes[8]`). A hard landing (state 0xb) holds the rider in it (`hard_land`, so no jump or tuck,
and the ground tick's control state is 0xb) until its landing clip ends, as `LandHardControl_Update` 0x104800
leaves it.

STANDINs:
- the clip type is the port's grab or uber: 0xe while short of its safe point;
- `land_kind` comes from the two angles;
- the landing clips' length is `wipeout::clip_frames`' 30-frame default: which clips the ids 0x1ff and 0x23e..0x243
  name is row G5.

A rider with no kept attitude (after a spin or flip, row G20c3c2) still lands by the port's rules.

Checked on gari's flat snow:
- level and forwards: clean;
- turned round: clean too (soft and slow enough, as `Landing_ChooseState` has it), riding switch;
- nose up 50° (the roll under −π/4): hard (clip 0x242);
- falling 30 m/s into the snow (over 27.77): hard (clip 0x23e);
- an uber cut short (a clip of type 0xe without flag 4): refused, down.

Leaning back 60° or nose down 120° puts the rider down by the air wall instead, as the game would: the body reaches
the snow first. That is also why the unclean-roll crash cannot be staged here, and its mutant survives (the check's
own `ok` is covered by `tests/landing.rs`). The clip-length mutant survives too, because every landing clip falls to
the same default.

### The spin state on the game's code (rows G1a5b3b1, G20c3c2; checked: `tests/spin_air.rs`)

A take-off into the air now runs the game's spin state. The take-off is detected after the rider's step, where the
flight prediction is made, from the snow or from a rail. Then:
- **Rates:** `Takeoff_SetSpinRates` 0x126e30 (`jump::takeoff_spin_rates`) from the wind-ups the take-off left the
  snow with (`Rider::takeoff_wind`) and the tricks stat (stats+0x1a).
- **Enter:** `SpinControl_Enter` 0x100598 (`spin::spin_enter`). It scales the rates (× 1.6 on a spin boost, × 0.8
  riding against the natural stance) and sets the bands. It also sets the turn toward the predicted landing's slope
  (`spin::landing_align` with the prediction's surface word and normal, `land_word`, `land_normal`) on the kept
  attitude.
- **Each air tick:** the pad word `cPlayer::GetControls` 0x151250 makes for state 0x10 (`controls::get_controls`,
  on a `PadDevice` read through button map 0), then `SpinState_Update` 0x100908 (`spin::spin_update`). That turns
  the kept attitude toward the landing, runs the spin and flip in their bands on the stick, auto-completes them
  once it is let go, and scores the degrees (`Score_UpdateSpin` / `_UpdateFlip`).
- **Landing:** any landing or placement ends the state (`SpinControl_Exit` 0x1029d0, `spin::spin_exit`). Whatever
  spin and flip are left turn into the attitude, and the position moves so the pivot stays put.

The attitude is therefore the game's through spins and flips too (row G20c3c2), and `land_as_game` uses it at the
landing. As in the game, the landing check reads the attitude's own axes (+0x180, +0x1a0), before the exit folds
in what is left. The port's own spin (`air_rotation`, its axes and banking) and its scoring of the angles run only
for a rider that never entered the state.

STANDINs:
- **The pad:** our input as a PS2 pad, in the word libpad hands over with its bytes swapped (`controls::PS2_BITS`):
  the steer and flip on the left stick and, past half, on the d-pad (up 0x1000, down 0x4000, right 0x2000, left
  0x8000: button map 0 spins and flips on the d-pad), cross 0x40 the jump, square 0x80 the boost. Button map 0 has
  no trick slots.
- **The animation:** no take-off clip, so the launch clip counts as of type 0xb when the take-off spins or flips,
  and the state starts on the stick (mode 0) as after the game's launch clips. Its other answers are 0, and the
  clips and lean the state asks for are not played (row G5).
- **The pivot bone:** at the hips' height, 91.7 cm × stats+0x60. There is no half-pipe.
- **The wind-ups:** the port's own prewind's (row G1a5b3b2b).

Late tricks (row G1a5b3b2a) work the same way, through `SpinState_StartFromStick` 0x102158
(`jump::spin_start_from_stick`). It is worked out on the tick's pad word before the state runs, so it can answer
when the state asks. When it starts:
- the rates go to the object (+0x2c, +0x30) and +0x58 is set;
- the trick so far is scored and the meter given (`Score_CommitAndRestartChain` 0x156568, `Boarder_AddMeter`:
  `Rider::bank_late`);
- states 1 and 0x10 follow: `SpinControl_Exit`, then `SpinControl_Enter` on the new rates, in `Rider::late_start`.
  The start's clip (0x226..0x22d by the direction) is not played and counts as of type 0xb.

Glue: the AI driver plans with the port's spin axes, so `spin_ax` and `flip_ax` and `air_mode2` mirror the spin
object's rates, bands and mode each tick. Checked: a straight jump with the stick pressed at its 12th tick in the
air starts the late spin at the 13th, on the stick (+0x58 set) at 2.86 rad/s, the trick banked.

Checked on a test slope: a jump with a spin wound up crouching takes off with the prediction 0.909 s and the rate
4.79 rad/s on the stick (mode 0). The state runs every tick in the air. Held for a third of a second, it turns
1.42 rad; let go, it is completing (mode 2). It landed after 61 ticks at 2.58 rad, short of the half turn, which
the exit folded into the attitude. The state ended.

### The other respawns at the game's reset place (row G20c3b5; checked: `tests/reset_place.rs`)

In the game every respawn that is not the start line, a teleport or a cast clip goes through the reset state.
`ResetState_Update` calls `ResetState_PlaceRider`, which places the rider at the respawn point on its own path
(`ResetState_GetRespawnPos`) with the inline atan2 of the path's direction there. The port's resets have their own
causes and timing. These include the safe spot, the wipe-outs' surface-0 and out-of-the-world returns, the stuck
rider, and the unstick button. G1c3's ported reset state is not wired yet. What the port now does take from the
game is the place and its heading:
- `aiglue::reset_place(paths, follow)` is `place_rider` with no spread on the rider's own place on its path.
- The hosts (the app's `keep` and the race self-test) keep `Rider::safe` at that place and `Rider::safe_heading`
  at its heading while the rider rides the snow. For computer riders they use the ported path following
  (`PortAi`). For the player, a STANDIN projects the rider onto the nearest respawnable path
  (`reset_place_near`), because the port does not track the player along the paths as `Boarder_UpdatePathAhead`
  does.
- Every respawn to the safe spot is `Rider::respawn_safe`, which calls `respawn_heading` with the game's heading.
- Without the AI paths (a levels folder), the old walker (`AiPaths::respawn_point`) and the port's yaw are kept.
- The race harness's rejoin on its own line after a respawn now runs only when there is no game place.

Checked on all ten courses' paths from the disc, 40 places along every path (37,280 places):
- `reset_place`'s heading is `place_rider`'s (itself checked against the original in `tests/reset.rs`) and the
  atan2 of `direction_at` at `respawn_at`'s point, bit for bit.
- Its point is that respawn point, within the conversion's rounding.
- A rider sent back with `respawn_safe` is placed exactly as by `respawn_heading`.

All the mutants are caught (the axes, the heading, the branch). The race self-test on gari is about as before
(line point 223 against 204, 6 wipe-outs against 7; these runs are not exactly repeatable).

### The normal in the air (row G20c2b)

- `AirMotion_Update` 0x108378 stores its down ray's normal at +0x2a0 at every hit (t ≥ 0, 0x108588), before it
  tests the velocity against it for a landing; `Rider::course_landing` now does the same on the course (a levels
  folder keeps the port's own normal in the air). With the placement's ray (G20c3b4) the respawn's normal is the
  game's too. The normal feeds the lean record's up (`lean_aim`), a late jump's impulse and the rider hits, as +0x2a0
  does in the game.
- Measured with the race self-test (`TRICKY_SIM=240`, disc) on eight courses: the first fall comes at the same place
  and time as before; after the runs part, the autopilot (the port's own) loses the line more often on aloha (4 → 10
  falls), megaplex (2 → 6) and elysium (2 → 6), less on mesablanca (4 → 2), the same elsewhere. The extra falls are 100
  to 350 m off the line, where the autopilot has gone, not at the landings.
- The self-test is not exactly repeatable (megaplex gave 5220 m and 5030 m on two runs of one build), so fall counts
  differ between runs of the same code; with G20c3d's hit kind as well, aloha fell 4 times and megaplex 4.
- The ground probe's hit kind (+0x2e0: 0 terrain, 1 an object with no surface, 2 a moving object, 3 none) is
  `Rider::hit_kind` (row G20c3d): the landing's probe casts in the game's space and keeps it, the seed of the
  game-side rider takes it, and each ground tick writes it back (it had been 0 at every landing).

### The teleport and the cast clips in the port (row G20c3b3; checked: `tests/teleport.rs`)

- `teleport::teleport_target` is `Boarder_TeleportToInstance` 0x11a098 up to its `Boarder_PlaceAt` call: the side
  vector by the rider's slot (+0x460: 0 (−1, 0), 1 (−1, 1), 2 (0, 1), 3 (1, 1), 4 (1, 0), any other none) through
  the object's matrix, ×300 cm, on the object's position (all four lanes, the vector unit's multiply-adds row by
  row), and the heading the inline atan2 of the matrix's y row. Then `Boarder_PlaceAt` with height 0, velocity 0,
  flag 0, and for a player's rider (+0x38, its player number, ≥ 0) 0x177db0 on that player's word (+0xa0 + 0x80·player). Checked
  on 60,000 random matrices and slots, bit for bit.
- The app's teleport pads (script op 24's objects) carry the six places and the heading from the object's matrix
  (the disc's instance record, `Instance::matrix`; the folder's location, rotation and scale put back together
  otherwise); `Rider::slot` is +0x460 (0 the player, the computer riders after it) and the pad places the rider with
  no velocity. On merqury's one teleport the heading equals the old `atan2` of the object's forward and the places
  sit 3 m to either side by slot.
- `camscript::update_cast_clip` is `RaceScene_UpdateCastClip` 0x16daa8: `Race_GetRider` (not checked for none), the
  rider held at the struct's place and heading (+0x20, +0x30; flag 1) when +0x14 is set, and once its clip's
  progress reaches 1.0 the next clip of the list at +4 (back to the first at a negative entry, `0x169be8`).
  `update_cast_clips` is `RaceScene_UpdateCastClips` 0x177020 (eight structs of 0x40 from +0x220 while +0x210 is
  set). The app does not play race scenes yet (G11), so nothing calls them there.

### The attitude kept through the air (row G20c3b; checked: `tests/placement.rs`)

The game keeps the placed quaternion (rider+0x170) while the rider drops: the trick's spin and flip are separate
angles (rider+0x1d0, +0x1d4) applied at the pose (`pose::board_frame`) and at the pivot (`air_motion`'s
`pivot_offset`: the bone's height turned back by them, seen from the attitude *without* them). In the air the
quaternion is turned only by the wall bump (`AirMotion_Update` 0x108378, the lean toward the wall's normal, at most
0x3dd67750 rad) and, at the landing, by `SpinState_Update`'s alignment (`spin.rs`, rider with a spin or flip).
So after `Boarder_PlaceAt` the board lies as placed until the landing, and the ground tick starts from it.

`Rider::att` = (`orient::placed_attitude(placed_heading(yaw), None)`, the port's yaw it was made for), set by
`Rider::new` (the start line) and `Rider::respawn`. `Rider::seed_ground` takes it while the port's yaw equals the
one it was made for (the port turns the yaw in the air with the spin), instead of rebuilding the attitude from yaw and
normal; the first ground tick takes it (`att = None`: the game-side rider's own quaternion goes on), and so does any
spin or flip in the air (`Rider::step`, after `air_rotation`); a take-off replaces it with the ground tick's own (G20c3c,
below). Checked: the
quaternion, forward, side, up and heading of the seed equal the real `Boarder_PlaceAt`'s bit for bit for 3,000 yaws
(respawn and new), and after a real drop onto a slope in `Rider::step` (the rider lands with the attitude still the
placed one; a spin drops it).

Left (STANDIN, rows from this one): the placement's own ray cast is not made at a respawn (E7), so a respawn on a
slope starts flat instead of tipped to the slope (the ground tick turns it, 1/12 a tick); a landing from a spin or
flip is `SpinState_Update`'s alignment, not wired (G1a5b3b, G5); the wall bump in the air is the port's own.

### ... and at a take-off (row G20c3c; checked: `tests/takeoff_attitude.rs`)

A jump off the snow keeps the attitude the same way. The take-off tick is `GroundMotion_Update` 0x10a0d8 (its turns:
the yaw toward the velocity, the lean toward the ground's normal) ending in `Boarder_TakeOff` 0x128470, which sets the
motion (air), `Jump_ApplyImpulse` 0x1284e0 (velocity only) and the spin rates; none of them writes rider+0x170, the
axes +0x180..+0x1a0 or the heading +0x1b0, and the air motion does not either (see above). So the attitude at the
landing is the one the take-off tick left (proven: the real `AirMotion_Update` leaves it as it was on every tick of
a free flight, in the test).

`Rider::take_off` takes the quaternion and axes of the ground tick's game-side rider (`GroundCache::attitude`, kept in
`Rider::gm` since the last ground tick) into `Rider::att`, with the port's yaw it was made for, when the port has not
moved the rider since (`GroundCache::is_at`); a take-off with no game-side rider (from a rail or the air) keeps what the
rider already keeps. `Rider::seed_ground` starts the landing's first ground tick from it while the yaw is the same and
there is no spin or flip. The ground tick's input is built by `Rider::ground_input`, public so the test can run the real
function on exactly what the port hands its own.

Checked in the function runner (the player's `SLUS_203.26`): 1,500 random riders on a plain riding state with no
ground under the board (the tick ends in `Boarder_TakeOff`), the real `GroundMotion_Update` on `ground_input`'s result
and the port's `ground_tick` + `take_off`: the quaternion, the three axes and the heading equal bit for bit; then 1 to
24 real `AirMotion_Update` ticks (nothing hit) leave them unchanged, and `seed_ground` after as many port air steps
equals them. 939 more cases were set aside because the original calls `Boarder_SetState` (a revert or landing state, which
the port's tick runs through `states::set_state`, checked in `tests/state_routines.rs`). The rebuilt attitude (before this
row) differs from the original's in all of 200 cases, and a spin in the air still drops the kept one.

Left (STANDIN, rows from this one): a jump with spin or flip (the kept attitude is dropped; the landing's alignment is
`SpinState_Update`'s, G20c3c2, G1a5b3b); the air's wall bump (`AirMotion_Update`'s turn at most 0x3dd67750 rad; the
port's air walls are `Rider::walls`, G20c3c3, E8b); a take-off from a rail (G20c2c3).

## The board's frame and the body height (row G16; checked: `tests/pose.rs`)

`Boarder_UpdatePose` 0x129628 builds the frame rider+0x4aa0.. (rows: the board's x, y, z axes, then the drawn
position) before posing the skeleton (`pose::board_frame`): the rotation matrix of the quaternion +0x170; with a steer
(+0x214 ≠ 0) and not on a rail, times a turn about the board's y axis by steer × 1.134464 (alpine) or × 0.87266463;
the position row = +0x140 + (+0x308)·up(+0x2c0) + (stats+0x60 · lean +0x244)·side(+0x330); with a spin or flip
(+0x1d0, +0x1d4) the frame turned by the spin about z then the flip about x, about the pivot bone's height (the
position row moved to the bone and back). The matrix products are the vector unit's (each row the frame's rows
weighted by the turn's row). `cBoarder::vf7` 0x117198 (only with rider+0x46b0 set) eases +0x308 toward the surface
table's word 10 in ground motion, toward 0 otherwise, at most 1.6666666 cm a tick (`pose::ease_body_height`). The port
draws its rider model at the position + `Rider::body_height` along the rider's up.

### The capture's clip-type gate in the air (row E8n; `tests/physics.rs`: `a_grab_short_of_its_fourth_marker_refuses_the_rail_capture`)

**Proven (decompiler and the exe's table).** `Rail_TryCapture` 0x125cb8 refuses when `AnimCtl_GetClipType`(layer 0) is
0xe, 0xf or 0x10 and `AnimCtl_TestClipFlag`(4, 0) is clear. `AnimCtl_GetClipType` 0x15eec0 is the first word of the
descriptor row of the layer's clip id (table 0x325fa8, 0x1c bytes a row; id 0x300 answers 8). Read from the table in the
exe (numbers only): clip ids **0x278..=0x291 are type 0xe, 0x292..=0x2ab type 0xf, 0x2ac..=0x2db type 0x10** (26 + 26 + 48 rows,
the grab, tweak and uber clips; `Landing_CheckAngles` 0x12ba78 has the same test, `landing::check_angles`), and clip
**0x224 (`bxA_FALLRECOVER`) is type 0**: the fall recover is not gated, a rider that recovers over a rail is captured.
`AnimCtl_TestClipFlag` 0x15ffb8 reads bit `flag - 1` of the byte at +0x74 of the layer's clip record (`AnimLayer_FindClip`
0x15a6c8: the layer's list of playing clips, the clip id at +0x40 of each node).

**Where flag 4 comes from (row E8s, proven).** `AnimClip_Advance` 0x15a030 clears the four bytes +0x74..+0x77 of the clip
node every tick, then for each of its source animations (the count at +0x44; the clip time `t` of each at +0x48 + 4i,
advanced by `dt * rate * 30 / 60`) reads two lists of four ushort frame numbers from the animation's data, at +0xc and
+0x14 (0xffff = unused). With `hi` the larger and `lo` the smaller of the old and new time: marker k of the first list
sets bit k of **+0x74** when `marker < hi` (the clip is past it), and bit k of +0x75 as well when `lo <= marker` (it was
crossed this very tick); the second list does the same into +0x76 / +0x77. So **flag 4 = bit 3 of +0x74 = marker 3 (the
fourth) of the first list, set while the clip is strictly past that frame**; a clip with no fourth marker (0xffff) never
sets it and so never clears the gate (those frame numbers live in the player's animation files, not in the exe, and are
not read). It is rebuilt every tick from the time, not latched. The port's grab notes call this marker `m4`
(`grab_markers().2`, the 4th of the clip's markers 1, 2, 4 and the end); a grab held stops at its second marker, so
holding never clears the gate.

**Ported.** `Rider::capture_gated` (`!grab_safe() || uber not done`), the port's form of the same test it makes in
`Rider::land` for `Landing_CheckAngles`, now refuses the air capture in `Rider::step_tick`: a grab held or playing out
short of its fourth marker, or an uber trick not done, flies over the rail. The spin and flip clips are not in the gated
ranges. Row E8s made `grab_safe` strict (`f > m4`, the game's `marker < time`; it was `>=`). **Inferred, not read:** that
the uber clips' flag 4 is the port's `uber_done` (the same mechanism, but the uber clips' marker frames are in the
animation files).

### The board's corners (row G7f3a; `pose::board_corners`, checked: `tests/pose.rs`)

After `Boarder_UpdateSkeleton`, `Boarder_UpdatePose` 0x129628 lays out the board's four corners for the board
track: rider+0x5760 front left, +0x5770 front right, +0x5780 back left, +0x5790 back right. They come from the
board bone's position (+0x4a90), its x row (+0x4a60, along the board) and its z row (+0x4a80, across). That fits
the rail code's reading of the bone. The lengths and half width depend on the board class (+0x420):

| class | to the front end | to the back end | half width |
|---|---|---|---|
| 0 | 96.09368 cm | 98.02858 cm | 16.22651 cm |
| 1 | 84.922966 cm | 84.07263 cm | 16.232784 cm |
| other | 114.15547 cm | 124.23839 cm | 15.998993 cm |

The two lengths swap for the other natural stance (stats+0x40), and everything is scaled by stats+0x60
(`Vec4_Scale`, all four lanes). Checked in the runner: the skeleton stub leaves a random bone, and the four corners
equal the original's bit for bit over the 40,000 pose cases. 5 of 5 mutants are caught.

## Skeleton pose helpers (F1c names, not ported)

- `Boarder_PoseBonesFromAngles` 0x205a78 (only from `Boarder_BuildBoneMatrices` 0x2083f8; proven): copies the root
  matrix in, then for bones 0..12 multiplies the parent's matrix (parent index at skeleton table +0x12 per 0x54-byte
  bone) by a rotation from three Euler angles (`Math_SinCos` of the 12-byte angle triple per bone) and a translation
  (angles +0x108), on VU0; then `Boarder_SolveLimbIk` 0x205e98 twice, on bones 16, 17, 18 and 13, 14, 15 with its own
  third and fourth arguments. **Inferred:** 0x205e98 is a two-bone limb (leg) solver (`Math_Asin`, three joints).
- `BdrLoad_GetShapeSlot` 0x204588: entry i of the slot words at list+0x130 of the list `BdrLoad_RebindShapes`
  0x2043f8 walks (count, then 5-byte shape names from +4); a slot ≥ 0 with a shape found by name is rebound through
  the renderer (gApp+0x724, vtable +0x164).
- Rider members torn down by `cBoarder::~cBoarder` 0x116990 (F1c, proven; all empty bodies with the gcc delete-flag
  check): `Score_Destruct` 0x1230a0 (rider+0x5820), `AirPredict_Destruct` 0x1230c0 (+0x57b0),
  `BdrLoad_DestructShapeList` 0x203ff8 (+0x4838), `CollBody_Destruct` 0x236190 (+0x470, also `cWorldBoarder`). The
  shape list (+0x4838) is built by `BdrLoad_ConstructShapeList` 0x203fd0 and both call `BdrLoad_ClearShapeList`
  0x204048: count 0, the 60 slot words at +0x130 set to −1.
- `cBoarderRender` 0x135278 allocates one 0xc360-byte effects block (render+0x3e0: puffs +0, trail +0x1c20, surface
  spray +0x4b60, board track +0x8020, brake spray +0x99b0, speed dust +0xb600); `BoarderRender_DestructEffects`
  0x136f88 runs `Fx_SpeedDust_Destruct` 0x12f490 and `Fx_BoardPuff_Destruct` 0x12ec78 (both empty) and frees it.
  `BoarderRender_CopyBoarderMatrix` 0x136e38 copies the rider's 4 × 4 matrix at rider+0x5760 (from `BoarderRender_Draw`).
- render+0x3e4 is the split-screen player marker (0x260 bytes). Proven: `PlayerMarker_Construct` 0x133808 builds it
  (`EaseRecord_Init` on +0x254, then `PlayerMarker_Reset` 0x133890: 8 records with +0x40.. = (0,0,0,1) and stride-0x30
  scale 1.0; +0x240 = 20000.0, +0x244 = −130.9, +0x248 = 132.54, +0x24c = 50.5, +0x250 = 300.13).
  `PlayerMarker_Update` 0x133940 (from `cBoarderRender::vf7` 0x1356f8, human riders only, rider+0x41c == 1) places 4
  points above bone +0x130 (skeleton at rider+0x4494 → +4); colour (0x80,0,0), or (0x80,0x6e,0) when stats+0x65 (pad
  port) ≠ 0; a size eased between 20 and 40 at 45.19/s × 1/60 is written as an int to the 4 records (it pulses).
  `PlayerMarker_Draw` 0x133c20 is called by `BoarderRender_Draw` only with 2 players (gApp+0x730 → +0x298 > 1 and mode
  +0x1c → +0x7c > 1), for a human rider that is not the focus of the current viewport's camera (`Camera_GetFocusRider`).
  Inferred: the marker over the other player in split screen.
- render+0x3e8 is the rival attitude icon (0x150 bytes, `AttitudeIcon_Construct` 0x134288). Proven:
  `AttitudeIcon_Update` 0x1343d0 runs for non-human riders in game modes (0x32de8c) 2 and 4; for each human player
  (`Career_FindRiderOfPlayer`) a 0x80 slot gets +0x50 = level from the pair attitude a (rider+0x50 + 0x20·i):
  a < 60 → a/60; a < 80.98926 → 1 + (a − 60)/20.989258; else 2 + (a − 80.98926)/40. A flash (+0xa0/+0xa4, +0.016386366
  per tick) starts when the grudge level (rider+0x124; grudge +0x118 against +0x120) changes, another (+0xac/+0xb0,
  +0.016652344) when anger (pair+0x13) rises; +0xb8 ping-pongs by ±0.066181876. Position = bone +0x130 + up
  (rider+0x4ac0) × 95.9087. `AttitudeIcon_Draw` 0x134830 is drawn by `BoarderRender_Draw` for non-human riders.
- Boost streaks. Proven: `BoarderRender_RecordStreaks` 0x136148 (from vf7) records every 3rd frame (mode+0x18 % 3)
  while boost level rider+0x130 > 0 or boost timer +0x134 > 0: colour 0x3bc230 / 0x3bc240 / 0x3bc250 by level
  (≤ 0.33362994, ≤ 0.66599226, else; the timer uses 0x3bc250) into render+0x1e8, and
  `BoarderRender_GetHandStreakPoints` 0x136c48 (bone 0x1f0 → 0x230 extended 15.0 cm past 0x230 plus bone 0x1b0; same for
  0x2f0 → 0x330 and 0x2b0) into a 7-slot ring at render+0x20 (index +0x1e0, count +0x1e4); otherwise the count decays.
  In control state rider+0x428 == 0x10 the rider matrix is copied (`BoarderRender_CopyBoarderMatrix`) into a second
  7-slot ring at render+0x200 (+0x3c0 index, +0x3c4 count). `Streak_Draw` 0x1f08d0 draws a ring (7 × 0x40-byte
  records) as two strips (points 0/1 and 2/3), newest to oldest, colour ring+0x1c8, alpha table 0x33f398 × 128;
  `BoarderRender_Draw` draws each ring with count > 1, the board ring only when stats+0x78 == 0.
- Also from `BoarderRender_Draw`: `BoarderShadow_Update` 0x1c7e90 (inferred from the call: shadow render+8, feet
  midpoint (bone 0x3f0 + 0x4b0)/2, rider+0x1a0, direction (0,0,−1), rider+0x4ae0, rider+0x5760; then
  `BoarderShadow_Draw`), and `Career_IsPlayerOutfit6` 0x167038 (`Career_GetPlayerOutfit() == 6`, passed to renderer
  vtable +0x2bc when the pad port ≠ −1).
- Controls set-up (proven): `cAI::vf11` 0x110bf8 calls `Input_InitDefaults` 0x179238 for each human rider with
  `Input_GetDevice(gApp+0x24)`: device stored, dev+0x2c = 0, dev+0xc |= dev+0x10; buttons 0..0x28 bound to −1 except
  0x1b 0x800, 0x1d 0x40, 0x1e 0x20, 0x1f 0x800, 0x20 0x10, 0x25 0x100, 0x28 0x40; axes 0..0xf bound (0: 0x20020 inv,
  1: 0x10010, 2: 0x1001 inv, 3: 0x8008 inv, 4: 0x1001, 5: 0x40, 6: 0x200, 10: 0x1001, 11: 0x8008, 13: 0x1001 inv,
  14: 0x8008 inv, 15: 0x8008, rest 0). `Pad_SetupVibration` 0x152770 uses `Input_SetShownButton` 0x179850
  (cInput+0x1cc[i] = v), `Vibration_Construct` 0x150a70 (8 bytes {device, 0}, kept at player[0x21d]) and
  `PadCfg_Destruct` 0x14f550 (empty, deleting).
- `AIComputer_CrossedEvent1b` 0x139210 (no callers): true if any path event (rider+0x57a4, cPath vtable +0xc) between
  +0x340 and +0x344 has type 0x1b. `SplinePathNode_IsVisible` 0x251a98: renderer vtable +0x284 cull test on +0x5c.

## The control states' updates (board row G1f; `tricky_game::control`, checked: `tests/control.rs`)

Each reads the pad word as `cPlayer::GetControls` packs it (sticks: six signed bits / 31 at shifts 0x15 steer,
0xf crouch, 9 brake, 0x12 / 0x18 / 0xc / 6 in the air; directions: two signed bits at 7 / 5 (ground) and 4 / 2
(air); bit 0 reset, bit 1 the air's reset, bit 2 the brake, bits 3 / 4 the boost buttons) and calls the boarder's
helpers. Most end alike: off the course (+0x290 = 0) or with bit 0, +0x304 = !bit 0 and `Boarder_RequestReset`(101.46).

- `CruiseControl_Update` 0x102fc8: a computer rider whose clip is not the stop clip 0x221 goes to state 6; boost,
  steer, brake / crouch (the crouch held to 0.5 for board class 2 unless layer 1 plays a look); +0x10 the brake held
  (bit 1 starts it, bits 1 / 2 keep it), which clears the board's +0x718; the lean target follows the steer (rate 2;
  0 while stopping). Braking with the board's +0x718 clear: the brake target 0 and, stopped, state 0xe and the
  rider's virtual +0x7c(1). Crouched, steering under 0.2, not braking and under 833.33 cm/s (|v| by the VU): the stop
  clip. Then the lean animation, `Boarder_PlayCelebrate`, `Boarder_PlayDisappointed`, `Ride_AITauntCheck`, the
  look back, `CruiseControl_RandomFlourish`, the reset check.
- `CruiseControl_LookBack` 0x1033d0: while layer 1 plays a look (type 8) a timer (+8) runs; after a second the first
  other rider (by slot) under 1477.63 away whose bearing (+0x44 + 0x20 · k) is more than 120° off the heading
  (wrapped by floor(a / 2π + 0.5)) turns the head: clip 0x210 when that side matches the stance (+0x1b4), else
  0x20f, played with the clip filter +0x46dc set from the trick slot's word (bits word + 3, + 4, + 5, + 9), then −1.
- `ChangeFakieControl_Update`, `LandHardControl_Update`, `BumpControl_Update`: the landing clip decides (0x202 → 5;
  0x1ff (and 0x221 for the fakie change) → 3, or 0xe with the brake button); a hard landing steers backward when the
  velocity is against the board; a bump turns with its clip's phase (+8 · phase, the change since +0xc).
- `LandNormalControl_Update`: 3 / 0xe once crouched back to the landing's crouch (+8); steering unless braking;
  not braking, the crouch target at least the landing's, rate 8.33.
- `NaturalAirControl_Update` 0x1000e0: the crouch (held at 1 on clip 0x224), the lean animation, the wind-up; still
  settling (brake, steer, lean or the two other triples not at rest): eased back (rates 4) or, with the steer target
  set, steering held to the air's limit (+8); at rest and not crouched past half with no steer: the crouch-out
  (+0x1f0 / +0x1f8 = 1 − crouch), the take-off clip after 0.35 s of air, the board's +0x76c / +0x770 / +0x798
  cleared, state 0x10 (the jump's wind-up). Bit 1 asks for a reset.
- `SitAndWaitControl_Update` 0x105bc0: at race state 4, the rider's camera starts "RaceStarted2", clip 0x1fe, motion
  6, state 0x12. `GetupFromSitControl_Update` 0x105c60: held still until the get-up clip ends (or 0x1ff), then clip
  0x221, state 3, motion 2 and a push of 416.67 along the board; brake and crouch cleared every frame.
- Checked in the runner on 80,000 random riders, pads and clips: every field, the state objects and every call with
  its arguments (and the clip filter when a clip starts) compared; every branch reached.

## The control dispatch, event bits and rumble (board row G1g; `tricky_game::control`, checked: `tests/control_misc.rs`)

- `Boarder_UpdateControl` 0x11c9a0: by the control state (+0x428) the state's update on its object in the boarder
  handler (2 bump, 3 cruise, 4 fakie change, 6 finish, 7..9 the gate, 10 jump, 0xb / 0xc landings, 0xd natural air,
  0xe prewind, 0xf rail, 0x10 spin, 0x11 / 0x12 sitting, 0x14 get-up, 0x16 reset); 0x13 only the reset check; 1,
  0x15 and the rest nothing. 5, the fakie cruise, runs inline: the slide-out clips (0x203 / 0x204) slow the
  velocity by 1%; on the turn clip (0x202) the stick (full right for a computer rider, turned for a goofy one, the
  character entry's +0x40) picks the way round unless near centre and the board already faces the velocity: the
  stance flips (+0x1b4, its copy +0x46d4, the facing +0x46e0 as π or 0), then clip 0x203 (> 0.5), 0x204 (< −0.5)
  or a half turn (`Boarder_RotateAboutAxis` π) and 0x201; 0x1ff / 0x221 to cruise.
- `cBoarder::vf10` 0x11a348 (an event: its bit in +0x388 this tick against +0x384 last tick, and an argument):
  turning on during a lesson (gApp+0x730 → +0x34), 0x10 goes to the lesson and 10 ends it; else 10 is
  `Boarder_Finish` (no multipliers left, +0x114), 0xd a multiplier (+0x114 − 1), 0xc a player's checkpoint
  (`Speech_OnCheckpoint`, `Boarder_HitCheckpoint`); still on, 0xc's checkpoint speech again and the place line
  (+0x10c ← +0x110); for the lead or focus rider the music events (1 → 0, 9, 10 and 0x12 → 10 with a cheer) and the
  speech lines (0x13 / 0x14 / 0x15 / 0x16); the event's sound on. Turning off, its sound off. 0xe / 0x17 / 0x18
  queue event sound 0 / 1 / 2 on, stop it off.
- Rumble (`Vibration_Construct` 0x150a70: the pad device and a float level): `Rumble_Add`(amount) above 0.5 (unless
  the race control's +0x20 is 1..9) pulses the pad for amount · 1.67 s held to 0.02..0.1, in frames (device virtual
  +0x24(1, n)); the level rises to amount + 0.8 (at most 2, never down). `Rumble_Update`: a level above 0 runs the
  motor (+0x2c(min(level, 1), 2)), then decays (· 0.9833 − 0.025) and stops it at 0; held off or empty, the motor
  off (0, −1).
- Helpers: `Boarder_SetTimeScaleRateLimited` (+0x12c toward the target by at most 0.008446), `Boarder_IsDoneFinishing`
  (a computer rider in state 1, or 6 with its finish line (board object +0x670) at phase 4),
  `FinishLineControl_IsDone` (past phase 1 and 240 ticks), `Boarder_StartLineOffset` (character +0x60 · 90 − 90),
  `Boarder_TickSfxFadeIn` (+0x5af0 down to 0), `FUN_0011f488` (+0x1ec above 0, or +0x1e8 = 4 past half its clip),
  `Boarder_LaunchFromGate` (8 → 9), `Boarder_AddSpeedSample` (+0xdc += speed · 0.036, +0xe0 += 1).
- Checked in the runner: 40,000 dispatches over every state (the update called with its object), 60,000 event bits,
  40,000 rumbles and helpers: fields and every call compared.

## The animation sequences and layers (row G5a1; `game/src/animseq.rs`, checked: `tests/animseq.rs`)

The controller's three layers are lists of sequences. A sequence is one clip being played. The code works through
pointers and virtual calls, so the port keeps the bytes at their addresses and runs its own functions for the
vtables' entries.
- **The list (`ClipList_*` 0x203ae0..0x203cd8):** the list object holds both end nodes, itself and itself + 0x38.
  Nodes link through +0x2c and +0x30. A new node goes in right after the list, so a walk by +0x30 meets the newest
  first. Remove calls the node's slot 1 (delete); "RemoveAndDestroy" 0x203b00 calls slot 2, which for a boarder
  sequence is its reset. DestroyAll deletes each node (reading the next one first), then empties the list.
- **The vtables:** pairs of (this-adjustment short, function). `cSkelSequence` 0x39e470 has delete (nothing), reset
  0x203cf8 and the weight update 0x203a40. `cBoarderSequence` 0x379020 has delete 0x159ff8 (reset, then
  `operator delete` 0x114f80), reset 0x159ee8 and update 0x15a3a8, then save and load.
- **The weight:** each tick it moves by its rate. At 1 or more it stays at 1 and the rate stops. At 0 or less it
  stays at 0, the rate stops and the sequence has ended (+0x24). The boarder sequence first takes the end's fade
  rate (+0x70) once source 0's whole frame reaches the fade frame (+0x6c).
- **The fades:**
  - `SetBlendOut` 0x159d88 (frames; below 0 means what is left of source 0, at least 1) gives the rate
    −|speed| × 30 / (60 × frames). A sequence that isn't held also gets a stop frame: 0 when playing backwards, the
    last frame otherwise.
  - `SetBlendIn` 0x159e38 gives (1 − weight) × |speed| × 30 / (60 × frames).
  - `SetEndBlendOut` 0x15a330 puts the fade at length − (frames + 1). 0 frames means never: 0x0fffffff, rate −1.
- **A source (`SetSource` 0x159bb8; slot, clip):**
  - The key comes from the key table 0x3bc530 (a word per clip id). When that word is the none key (0x39e468), the
    table's first word that isn't none is used instead.
  - Only a new key changes anything: the old tree is destroyed (`SkelAnim_DestroyTree` 0x159a80, children first),
    a new one is made from the bank 0x3211a8 (`SkelAnim_Create` 0x158dc0), and the stop frame is cleared.
  - A tree's header (its +8) holds the child count (+9), the length (+0xa) and two lists of four marker frames
    (+0xc, +0x14).
- **The advance (0x15a030; dt):**
  - A held sequence at its end goes back to frame 0.
  - Otherwise each source's time moves by dt × speed × 30 / 60, and the marker bits are rebuilt (see the rail
    capture notes for +0x74..+0x77).
  - The time stops at the stop frame and at the clip's ends. When it was already at the end, a sequence that isn't
    held has ended.
- **`GetPhase` 0x159e80:** time / (length − 1) (1 for a one-frame clip), measured from the end when playing
  backwards.
- **The layer helpers** fade out, turn the facing, set the stance, find a clip (the newest) and clear "current".
- **`BdrSeq_Create` 0x15a700:** 0x90 bytes from the node pool 0x345e48 (named "BdrSeq"), zeroed. Then the
  constructor sets the skeleton sequence's vtable and reset, its own vtable, no trees and no keys, and calls the
  reset.
- **The EE's division rounds toward zero** (the runner's rule), where `ps2f::F` rounds to nearest. The port divides
  with its own chopped `div`. The last bit of the fade rates depends on it.

Checked in the runner on 40,000 random worlds: three layers with sequences linked on them and some loose, the
sequences' fields (often whole or half frames, and weights that land exactly on 1 and 0), their trees and headers,
and the key table. That is about 1,400 calls for each of 28 functions. Delete, the tree maker and the pool's alloc
were logged. 39 of 42 mutants were caught. The other 3 are equivalent: two memsets that later writes cover, and the
backward stop frame stored over an equal time.

## The animation controller (row G5a2; `game/src/animctl.rs`, checked: `tests/animctl.rs`)

The rider's controller (rider+0x46c0) starts clips on its three layers, advances them each tick and ends them. It
works on the sequences and layers of G5a1. Its calls out are the pose blend (`AnimCtl_BlendPose` 0x15f7c8), the
clip data (`AnimClip_ResolveAnmIndex`, `AnimData_GetClipId` / `_GetMarkerFrame` / `_SetMarker` / `_GetMarkerPhase`),
the sounds and the motion-driven update modes. The pose blend and clip data are G5c; the modes are G5b.
- **The fields:**
  - +8..+0x10: each layer's current clip (0x300 none);
  - +0x14 the stance;
  - +0x18 the speed new clips take (reset to 1);
  - +0x1c (reset to −1), which new clips copy to their +0x18;
  - +0x20 the facing new clips take;
  - +0x24 the rider;
  - the layers from +0x28, 0x70 apart.

  The constructor makes each layer's two ends skeleton sequences and empties the list. The first controller made
  also runs `AnimIds_Init` (the flag at 0x32b38c).
- **The clip table** 0x325fa8 has 0x2ff rows; the word after it is the flag, and the career block starts two words
  later. Each row is 7 words: category, lookup mode, update mode, end action, layer, blend-in frames, end fade
  frames. In the exe, the blend-ins are 0 or 7, the end fades 0 or 10, and the layers 0..2.
- **Playing a clip (`AnimCtl_PlayClip` 0x15fcf8; clip, frame, marker):** a new sequence is begun on the clip's layer
  (`AnimCtl_BeginClip` 0x15f060), and the answer becomes that layer's current clip. The audio system (0x343050) is
  told the layer's clip type (0x235778).
- **`AnimCtl_BeginClip`:**
  - With no animation for this rider (0x585), the sequence is deleted and the answer is none.
  - Otherwise the sequence becomes current, gets the clip, +0x28 = 1, and source 0 = the animation (one source). It
    takes the controller's stance, facing and +0x1c.
  - The start frame is `frame` plus: marker 1..4 of the first list for `marker` 1..4 (0 if that marker is unused),
    the last frame for `marker` 6, otherwise 0. It is never below 0. The speed is the controller's.
  - The update mode then sets a stop frame: mode 1 stops at marker 2, mode 2 at the last frame; mode 3 holds.
  - The weight is 1, and the end's fade comes from the table.
  - With no blend-in, the layer and layer 1 are emptied (`AnimCtl_ClearClips`). With a blend-in, when it is layer 1
    or the layer has sequences, those lose "current" and fade out over the blend-in, and the new one fades in from
    weight 0.
  - The sequence is then put on the layer.
- **Each tick (`AnimCtl_UpdateLayers` 0x15f390; dt):** every sequence of the three layers, newest first, is updated
  (`AnimCtl_UpdateClip` 0x15f440):
  - its update mode moves it: 0..3 is the plain advance (`AnimMode_Advance..3` jump to it); 4..11 are the
    rider-driven modes; past 11 nothing;
  - the first list's markers crossed this tick play their sounds (the header's +0x1c + 2k, 0x235780);
  - the weight is updated (vtable slot 3).

  Then the pose is blended.
- **Ends (`AnimCtl_FlushEndedClips` 0x15f700):** the ended sequences of all layers are listed first, then each
  is handled (`AnimCtl_HandleClipEnd` 0x15f5b8).
  - A sequence that isn't current is just taken off its clip's layer.
  - A current one runs its clip's end action, and the answer becomes the layer's current clip. Actions past 7 do
    nothing.
  - Every action but 0 removes the sequence from layer 0 (+0x28), whatever layer it is on.
  - The actions: 0 stop; 1 back to the base 0x1ff, after clearing the rider's six words +0x1fc..+0x210; 2 play
    0x223; 3 play 0x202; 4 chain; 5 play 0x2df; 6 play 0x2de; 7 0x202 on the alpine board (board class rider+0x420 = 2), else
    0x1ff.
  - Each clip played this way uses frame 0 and marker 5 (0).
  - Stop (0x161640) takes the sequence off its own layer and answers none.
  - The chain (0x161400, table 0x379620): 0x226..0x22d go to 0x22e..0x235; 0x270 → 0x235, 0x271 → 0x234,
    0x272 → 0x22f, 0x273 → 0x22e, 0x274 → 0x235, 0x275 → 0x234, 0x276 and 0x277 → 0x22f. Any other clip id
    answers itself.
- **The queries:**
  - the layer's clip; its type (the category, 8 with none);
  - the current clip's phase (1 when not playing); its duration (length / (speed × 30), 0 when not playing); whether
    it is at its end;
  - marker n − 1's phase;
  - the clip's first marker of 0..3 that has a frame (−1 if none);
  - `TestClipFlag` / `TestClipBit` (+0x74 / +0x75 bit flag − 1). These read the current clip's sequence without
    checking that one exists.
- **The rest:** `RaceScene_SetClipMarker` 0x15fbb8 sets the clip's first marker that has a frame. A frame of −1
  then clears all four. `StopClipsInCategory` clears the stop frame of a category's sequences (any category for
  0x1c). `BlendOutAll` fades layer 1's sequences over 10 frames. Turn and stance apply to all three layers.

Checked in the runner on 40,000 random controllers, about 1,700 calls for each of 23 functions. The data was three
layers of sequences with real clip ids (picked by the table's end action and update mode, often the layer's current
clip), their trees and headers, the rider's words and the key table, with the game's own clip table. The sequences'
functions ran as the original; the calls out were logged. 48 of 50 mutants were caught. The other 2 are equivalent: stop's answer when the clips differ (it is only reached with the sequence's own clip), and the chain's "any other clip" (every clip whose end action is the chain is in its table).

## The motion-driven update modes (row G5b; `game/src/animmode.rs`, checked: `tests/animmode.rs`)

Clips whose update mode (the clip table's +8) is 4..11 aren't advanced by time. Instead the mode sets the
sequence's sources, their weights (+0x54..) and their times (+0x48..) from the rider. Each picks its clips from an
exe table by board class (rider+0x420). Riding switch (+0x1b4) mirrors the inputs of modes 4, 9 and 10.
- **8, board class frame (table 0x379180):** the class's clip alone at rider+0x1f0 of its length.
- **9, direction (0x379190, 5 words a class):**
  - The direction (+0x27c, +0x280) becomes an angle. The atan2 is written inline around `Math_Atan`: ±π/2 on the y
    axis, ±π added on the left.
  - Each quarter turn blends two of words 0..3. The first's weight is |angle − the quarter's start| × 2/π (0x3f22f983),
    the second's 1 minus that.
  - Both play at the stick's length (clamped 0..1) of their clip.
  - With no direction, word 4 plays alone at frame 0.
- **5, crouch (0x3791d0):** the standing clip advances by time with weight 1 − crouch. The crouched clip follows at
  the same phase with weight crouch.
- **4, brake (0x3791e8, 10 words a class):**
  - With neither the brake (+0x1fc) nor +0x268, words 4 and 5 are blended by the crouch, held and advanced by time.
    On class 2 this happens only at a crouch of exactly 0 or 1.
  - With no brake, words 1 / 3 (+0x268 above 0) or 0 / 2 are blended by the crouch. Class 2 uses words 1, 9, 3 (or
    0, 8, 2) over two halves of the crouch. Both play at |+0x268| of their length.
  - Braking: word 1 or 0 (by +0x268's sign) with weight 1 − |brake| at |+0x268|, then word 6 or 7 (by the brake's
    sign) with weight |brake| at |brake|.
- **11, gate lean:** source 0 at the start gate's lean (+0x238) of its length.
- **6, crouch frame:** source 0 at 1 − crouch of its length.
- **10, wind-up (0x379260, 0xc8 a class):**
  - The spin and flip wind-ups (+0x220, +0x22c) become an angle in eighth turns (4/π, 0x3fa2f983) over the row
    (+0x1dc) of the clip table.
  - The two clips are weighted by the crouch. The row's word 0 plays as a third source at frame 0 with weight
    1 − crouch.
  - With no wind-up: word 0 at frame 0, and word 5 at the crouch of its length.
- **7, crouch-steer (0x3794b8, 0x78 a class):** two clips weighted 1 − crouch and crouch.
  - With no steer (+0x214), or row 1 or 2: words 0x50 / 0x64, held and advanced by time.
  - Otherwise 0 / 0x14 steering right or 0x28 / 0x3c steering left, at |steer| of their length.
- **The EE's rounding:** the runner rounds `sqrt.s` like division, toward zero. `Math_Atan` divides and takes a
  square root, so the port uses `math::atan_with` with `animseq::div` / `animseq::sqrt`. With the plain `math::atan`,
  a quarter of the angles differ in the last bit. The ports checked with all-nearest rules keep `math::atan`.

Checked in the runner on 60,000 random riders and sequences, about 7,500 calls for each of 8 modes. The inputs were
often exactly 0, ½ or 1. 34 of 42 mutants were caught. Four of the others are equivalent: the both-zero and y-axis
tests inside the angle (zero input is handled before), the length clamp at exactly 1, and the brake's sign in the
braking branch. The last four are the wind-up sectors' boundaries, where the angle would have to equal ±π/4 or ±3π/4
bit for bit.

## The animation bank (row G5c1; `game/src/animdata.rs`, checked: `tests/animdata.rs`)

`AnimData` at 0x3211a8 is the bank the sequences take their clips from.
- **Its layout:**
  - +0 a memory bank: its block, the next free byte, the bytes used, the block's size;
  - +0x10 (4), +0x14;
  - 0x1be words at +0x18;
  - the BIG file object (`cAsyncBigFile`) at +0x710;
  - 0x1be file records of 0x18 bytes from +0x724: the bank, +4, the clip headers (+8, 0x24 bytes each), +0xc, +0x10,
    +0x14 (−1).
- **A clip's key** (the key table 0x3bc530) is two shorts: the file and the clip in it. `AnimData_GetClipId`
  0x158d68 just reads that word.
- **A clip header:** +9 the child count, +0xa the length in frames, then three lists of four shorts. +0xc are the
  markers (`AnimData_GetMarkerPhase`: marker / (length − 1), at least 1; the sequences' marker bits). +0x14 are the
  marker frames the race scenes read (`AnimData_GetMarkerFrame`, −1 for 0xffff). +0x1c are the marker sounds
  (`AnimCtl_FireClipSounds`).
- **`AnimData_SetMarker` 0x158818(file, clip, k, v):** −1 clears the second list's frame k. Any other value is
  written to the third list, the sounds, at k. That is what the game does; `RaceScene_SetClipMarker` calls it.
- **The memory bank:**
  - `Init` allocates size + 0x80 (`EAMem_Alloc` with the name at 0x378fa0) and fills it with 0xdeadc0ed through
    `EAMem_Set`. That writes each byte, half and word at its alignment from the value's low bits, so the bytes run
    ed c0 ad de.
  - `AlignStart` puts the next free byte at the block rounded up to 128 (+0x7f, then division toward zero) and counts
    the skipped bytes as used.
  - `Alloc` hands out the next free byte and moves on by the size rounded toward zero to 128, plus 128.
- **The clip names:** `StaticInit_AnimNameHashes` 0x159820 hashes the 0x584 names (pointers at 0x3242a0) into
  0x3bdb40. `Name_Hash` 0x240e80 is a PJW-style hash: h × 16 plus each signed byte, with the top four bits folded
  back in at bit 23 and cleared.
- **Construct, clear, init, reset, destruct:**
  - Construct makes the BIG file object, empties every file, hashes the names, sets +0x10 4 and +0x14 0, and clears
    the words at +0x18.
  - Init gives every file this bank. Clear frees the block and takes the files' bank away.
  - Reset aligns the block again, empties the files, calls the BIG file's 0x23bea0 when its first word is set, and
    makes every key none.
  - Destruct destroys the files from the last, then the BIG file and the memory bank (both with flags 2); flag 1
    also deletes the bank.

Checked in the runner on 30,000 random banks, about 1,600 calls for each of 19 functions. The data was the files'
headers in an arena, random markers (often unset), random names with high bytes, and blocks at unaligned addresses.
27 of 27 mutants were caught.

`tools/disasm.py` now decodes the VU0 macro instructions (`lqc2` / `sqc2`, `qmfc2` / `cfc2` / `qmtc2` and the vector
operations, as `tools/r5900` runs them), which capstone shows as `.word` or Octeon branches.

## The animation curves and angle helpers (row G5c3a; `game/src/animcurve.rs`, checked: `tests/animcurve.rs`)

- **A curve** (`AnimCurve_Eval` 0x157ed8, one piece `AnimCurve_EvalPiece` 0x158030):
  - It starts with a 16-bit header: the low four bits are the kind, the rest (signed) the length in frames.
  - Constants are 24-bit floats: three bytes as the float's top bits, with a low byte of 0x80.
  - Kinds 0..3 are polynomials (constant, t·a + b, (t·a + b)·t + c, the cubic likewise).
  - Kind 4 is a table of 32-bit floats, one a frame. Kinds 6 and 7 are tables of bytes or 16-bit values, scaled by
    the second constant and offset by the first. The tables are interpolated linearly between whole frames, and the
    next value isn't read at a whole frame.
  - Kind 5 is a run of pieces: each covers its length − 1 frames, the frame counts on through them, and past the last
    the value is 0. A piece's size is 5, 8, 0xb, 0xe, 4·len + 2, 0, len + 8 or 2·len + 8 bytes by kind.
  - Any other kind is 0.
- **`AnimData_GetRootOffset` 0x1586c0:** the clip's three root curves at frame 0, taken from its first child when
  it has children. The first curve's index is the header's +4; the indices are looked up in the file's offset table.
- **0x202520 (the Euler delta from a to b):** each angle difference is wrapped into a turn (rounded with + ½, toward
  zero, and one down when that overshot). The same rotation's other triple (x + π, −b.y − a.y + π, z + π) is used
  when its absolute angles sum to less. `Euler_Lerp` 0x202758 adds t times that delta; `Euler_LerpArray` does it in
  place on lists of pointers.
- **0x250c10 (the sine and cosine pair):** the sine comes from its polynomial on the quarter turn's remainder, and
  the cosine is √(1 − sin²). The quarter swaps and negates them. The square root is the EE's, toward zero.

Checked in the runner on 60,000 random curves, files, triples and angles, about 7,500 calls for each of 8 functions.
The curves sat at unaligned addresses, and some runs had a negative count. 29 of 31 mutants were caught. The
other two are boundaries: the two Euler sums exactly equal, and an angle of exactly 0 in the sine's quarter rounding,
which is equivalent because ±½ both truncate to 0.
