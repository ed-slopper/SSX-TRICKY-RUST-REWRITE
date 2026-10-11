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
  5 play 0x2df, 6 play 0x2de, 7 back to 0x202 on the ground (motion 2) else 0x1ff.
- Pad rumble (`cPlayer::vf7`/`vf11` → `Rumble_Update` 0x150d18, `Rumble_Impact` 0x150a80): an impact of
  −dot(velocity, normal)·1.389 (clamped 0..2) above 0.3 buzzes the small motor for 0.02–0.1 s and sets the big
  motor's strength, which decays ×0.98333 − 0.025 per tick; nothing during replays.

## Animation clip modes (F1c)

`AnimCtl_UpdateLayers` 0x15f390 walks the 3 animation layers (0x70 bytes each) and calls `AnimCtl_UpdateClip`
0x15f440 on every clip. That picks how the clip is driven from a per-clip table at 0x325fb0 (stride 0x1c, by the
clip id at clip+0x40), then fires the clip's sound markers (`AnimCtl_FireClipSounds`):

| mode | function | driven by |
|---|---|---|
| 0-3 | `AnimMode_Advance` 0x161340 (and three copies) | time |
| 4 | `AnimMode_BrakeBlend` 0x160680 | brake (rider+0x1fc) and rider+0x268, two sources per board class (tables at 0x3791e8) |
| 5 | `AnimMode_CrouchBlend` 0x1605a0 | crouch (rider+0x208), sources per board class at 0x3791d0 |
| 6 | `AnimMode_CrouchFrame` 0x161300 | frame = (1 - crouch) × length |
| 7 | `AnimMode_CrouchSteerBlend` 0x161118 | crouch and the steer sign (rider+0x214), tables at 0x3794b8 |
| 8 | 0x160140 | frame = rider+0x1f0 × length |
| 9 | `AnimMode_DirectionBlend` 0x1601d0 | angle of (rider+0x27c, rider+0x280), quadrant blend |
| 10 | 0x160b20 | like 9 |
| 11 | 0x160af0 | frame = rider+0x238 × length |

Riding switch (rider+0x1b4) mirrors the inputs of modes 4 and 9.

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
- not done: where the career's level bytes come from (training points; row G3 — the port feeds the training fraction
  between each group's start and cap, STANDIN), the computer riders' value (their rival level, row G20a2), and the stat
  bytes other code still takes from the port's floats (tricks → `Boarder_AddMeter`, wind-up, gate, stability).

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
