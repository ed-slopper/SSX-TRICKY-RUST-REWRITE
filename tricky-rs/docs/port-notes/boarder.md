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
- Normal bleed (not on surfaces 3, 4): v −= 0.4(v·n′)n′, |v| restored.
- Airborne when the probe misses or h > surf[6] (2.74 cm on groomed snow).
- Speed caps (vf7): 2788.84 cm/s; boosting 2932/3072/3347 by meter level; 3347 while timer +0x134 > 0;
  cap = max(target, cap − 3.472) per tick.
- Backwards turnaround: vF < −111 cm/s, brake 0, state 3/5/12 → board turns 180°, switch toggles, steer negated.

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

**Surface table** (`SurfaceTable_Init` 0x256188; 20 rows × 25 floats): [0] g, [1] lin, [2] quad, [3] cubic drag,
[4] side grip, [5] lean°, [6] airborne height cm, [9] normal damping, [11] push target km/h, [12] thrust scale.
surf[6]/surf[9] by row: 1 2.742/5.009; 2 2.849/5.530; 3 15.04/2.842; 4 30.03/2.976; 5 2.775/4.052;
6,10,15,16,17,19 20/30; 7 21.37/45.2; 8 20/39.4; 9 2.02/30; 11 0.70/17.9; 12 3.95/30; 13 2.64/0; 14 1.56/30; 18 2.58/40.

## Animation timing

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
