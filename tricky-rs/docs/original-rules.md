# SSX Tricky: how the original works (read from SLUS_203.26)

Notes from the decompiled executable, gathered to make tricky-rs match the game 1:1. Units in the game are cm,
cm/s, and a fixed 60 Hz tick (`dt = timescale/60`, timescale at boarder+0x12c, normally 1). Functions are named
in the Ghidra project (ghidra-mcp). "Proven" = read from code; "inferred" = interpretation.

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

## Jump, air, tricks, landing

- Prewind `PrewindState_Update` 0x1050e8: crouch → 1 at 5.0025·max(|1−c|,0.1)/s (full ≈0.66 s, exponential);
  wind-up → stick at 2.0005·|stick−w|·(0.812+0.329·s18)/s (×2 when +0x1dc). On release: |w|<0.2 → 0;
  a = atan(|wf|/|ws|): a > 80° → spin 0, a < 10° → flip 0. Crouch unwinds at 13.33/s; the jump fires when it
  reaches 0 (takeoff c/13.33 s after release).
- Jump `Jump_ApplyImpulse` 0x1284e0: Δv = max(630.88, c²(0.0985+0.787J)·S(v)), S = 0.881v+24.68 below 993.5 cm/s
  else 899.87; direction normalize(N + 0.2F). Steep lips (50–70°) blend velocity toward the lip, up to 0.9.
- Takeoff rates `Takeoff_SetSpinRates` 0x126e30: K = 11.517(0.5449+0.6742T); spin ω0 = K·ws (×2/3 alpine),
  flip ω0 = (2/3)K·wf; ×0.8 fakie; ×1.6 in spin-boost zones (course event 0x12). Band hi = ω0, lo = 0.3ω0,
  floor = min(|ω0|, 1.396); all 0 with no wind-up.
- Air control `SpinState_Update` 0x100908 (stick digital −1/0/+1):
  stick held: ω → dir·|hi+lo| at ≤ 75 rad/s².
  first tick with no stick: mode 2 for good; the lower bound on the spin's side and the floor go to 0; emax = 2π.
  mode 2: ω = 150.54(0.5449+0.6742T)·dt·clamp(err, ±smallest |err| so far) (≈2.21/s);
  spin err: within 40° of 0 → back to 0; within 40° of 180° → 180°; else on to the next half turn in the spin's
  direction. Flip: within 40° of upright → back; else on to the next full turn.
  every tick: hi, lo ×(0.99301+0.00201T), kept ≥ floor; ω clamped to [lo, hi] (never reverses); angle += ω·dt.
  Restart `SpinState_StartFromStick` 0x102158: in mode 2 with both |ω| < 0.7, a stick press starts
  5.2531(0.5449+0.6742T) on both axes.
  Pre-landing: board turns toward the landing slope at ≤ 1.396 rad/s; a settled backwards board turns 180° and
  toggles fakie before touchdown.
- Ballistics `Air_IntegrateRK4` 0x12b340: gravity −850.24 rising / −1900.84 falling cm/s², horizontal drag
  −0.20002v, cap 3347.2 cm/s.
- Landing `AirMotion_Update` 0x108378, `Landing_CheckAngles` 0x12ba78, `Landing_ChooseState` 0x109308:
  yaw = acos(forward·travel) 0..π; pitch = −atan2(fwd·N, up·N) (sign flipped when yaw > 90°).
  Crash: grab/tweak/uber clip not at its safe marker; surface 0, 6 or 10; pitch·(2.0408−1.0374·s16) outside
  [−2.968, +2.684]. Speed ×(1.0643−0.2454·clamp(|pitch|,15°,50°))·(1.1136−0.2603·clamp(|yaw|,25°,80°)).
  Hard landing (blocks riding/prewind for its clip) if pitch < −45° or normal speed < −2777 cm/s.
  The landing does not re-aim the board; edge grip does.
- Grabs follow clip markers at speed 0.782+0.806·s14.

## Rails (`RailSlideMotion_Update` 0x10b0d0, `RailSlideControl_Update` 0x1073e0, `Rail_TryCapture` 0x125cb8)

- Stick alone balances. Boost held + left/right starts a quarter turn (clips 0x258–0x25f), committed at clip end;
  holding chains turns. Types cycle +90°: 1→4→2→3→1 (1 = 50-50, 2 = fakie 50-50 toggling switch, 3/4 slides).
- Balance: g = v<555.56 ? 0.5 : 0.0009v; gc = clamp(g,0.5,2); h = clamp(1/g²,1,4); k = −5 for the first 0.6 s or
  steep rails (|tan.z|>0.92) else +1.8153523; |off|<2.5 cm → k = −1 (if k > −1); k ≥ 0 → off clamped ±20 cm.
  lateral velocity = −s·145.07674·gc − k·h·off cm/s (s = smoothed steer).
- Fall off when |d·row0| ≥ 150w+80(1−w), |d·row1| ≥ 50, |d·row2| ≥ 50w+30(1−w) (w = 1 until 0.6 s then → 0 at
  1.667/s): v −= 277.8 cm/s away from the rail, into the air, not a crash; trick still scored.
- Motion: velocity blended to the tangent at 30·dt; gravity along the tangent; no friction, cap or minimum (slow
  riders roll back). Boost: per press Δv = 0.408·level m/s, cost 0.00075 meter; held boost only while crouched.
- Capture: same box ×0.9; v = tangent·min(|v|, max(555.56, v·t)); vertical ×0.1.

## Meter, uber, scoring (score object boarder+0x5820, `Score_*` 0x155410–0x157490)

- Points = round10(mult·units·6786.545); meter += units·0.6786545·(0.97878+0.35966·a1d)/(repeats+1) (negative too).
- Units: half spin 0.0699543; flip 0.2499898; grab start (n+1)·0.0424975; hold rate·0.000750064/tick;
  uber clip seconds·0.0450038·rate; rail per tick f(v)·0.0025006 (f = v/(3200−3v), 0 under 50, 1 from 800 cm/s);
  rail spin +0.0699543 per new max of |round180|; onto rail 0.18; off rail 0.19998.
- Leaving a rail scores the rail trick with no repeat check; > 1 s adds 1 to the chain; leaving switch/fakie gives
  the next air trick ×1.3.
- Landing (only if an air trick was done): repeat ring of 5 divides by (n+1); chain +1; on a snow landing with
  chain ≥ 2 add 4000/8000/12000/16000 (2/3/4/5+), then reset; air time ≥ 4 s adds (int)(t−3)·1000. Bonuses are
  unmultiplied, after the repeat division.
- Boost held drains 0.00075015/tick (22.2 s full). Human passive leak (15.4835 − 15.2104·a1b)·9.1667e-6/tick.
- Uber timer = 20 s whenever meter gain happens at a full meter; −1/60 per tick in every state, never below one
  tick in the air. TRICKY (cumulative ubers > 5) sets meter 1, timer 1. Bail −0.10, cap 0.666, timer 0.
- Pickups ×2/×3/×5 only while airborne or on a rail, reset at landing.

## AI, race, camera

- Rubber band (`Race_UpdatePlacingsAndRubberBand` 0x115100, `AI_RubberBandSpeedScale` 0x13a0f0,
  `Boarder_SetTimeScaleRateLimited` 0x11cff0): every 6 ticks in race modes each AI gets a human target and a lead
  window; d = −dist2D·cos(angleToTarget − heading); m = d<min ? ((d−min)/min+1)·1.1359849 : d>max ? 2.0487268/(d−max)
  : 1, clamped 0.70005476..1.5022597; timescale → m at ≤ 0.008446341/tick.
  Windows (cm, rider slot 0..5): single race max 6030.36, 5509.69, 5008.22, 5000.68, 4017.70, 1508.16; min −800.17,
  −1101.80, −1415.93, −1809.33, −3506.75, −3911.98. Circuit max 5014.84, 4519.08, 4034.01, 4097.28, 3029.93,
  2022.42; min −1869.04, −2147.75, −2427.94, −2937.53, −3560.38, −4086.21. + trackOff·k (gari 1532.37, snow 907.67,
  elysium 421.18, mesa −504.23, merq 503.47, aloha 46.70); difficulty k 0.6/0.4/0.35, f 1/0.5/0. Endgame distance E
  per track (gari 140005.72, snow 125085.09, elysium 159083.5, mesa 138040.55, merq 142028.88, aloha 131061.63,
  tokyo 40011.45, alaska 160036.53): ahead max = base + (R<E ? R(1−f) : 0); behind min = R<E ? −courseLen : base.
- AI path events (AIP): 0x1A target speed value·27.78 cm/s, looked up 600 cm ahead; 0x19 jump 300 cm ahead,
  speed (value>>2)·27.78, bit1 spin, bit0 flip. (AIP.json PathEvents type = runtime + 75, inferred.)
  Cruise 0x1375c8: brake when speed > target+138.89; tuck(+boost) when speed < (target−138.89)·skill/1.4103;
  boost at a full meter. Tuck = skill/2.2117 (1 below 833 cm/s). Steer 0x1372f8: |angle|·6.2897·skill/1.0241,
  dead zone 0.2, cap 0.9706.
- Skill pairs (player ahead, AI ahead) re-picked every 60 ticks: circuit (0.7549, 0.4048), (0.9092, 0.7084),
  (1.0, 0.9513); single race (0.6069, 0.2741), (0.8723, 0.6566), (1.0, 0.9491); other (1.0, 0.5088).
- AI tricks (`AIComputer_PlanJumpTrick` 0x139d58, `AIComputer_AirTrickInput` 0x138d60): at a jump event (0x19,
  value bit0 flip, bit1 spin) reached within 153 cm of the path and |steer| < 0.536: do-trick = (spin|flip bit) &&
  rand%100 − 16 < B (B = tricks stat byte, 0..255); late let-go = rand%100·(1 − skill/14.454) > 20; grab = separate
  roll, uniform among the rider's 15 combos; directions ±1 at random. Wind-up stick = directions during the zone.
  In the air: keep flipping while 5° ≤ |flip|%360 ≤ 340°, near a whole turn stop if time left < 2π/|ω|; spin
  likewise per 180° (360° alpine) with a 20°/5° window; grab held until time left < 0.3 s (late) or
  0.8/(s14·0.8057834+0.78212035). Plain jumps: a random grab once air time > thr with > thr left. AIs never tweak
  or uber, never pick rails; on a rail they balance (−2·offset) then roll quarter turns.
- Time left in the air (`AirPredict_AtTakeoff` 0x123b10): RK4 flight cast against the world at takeoff.
- Course triggers (`cBoarder::vf10` 0x11a348): 10 finish, 0xd lap, 0xc checkpoint (`Boarder_HitCheckpoint`
  0x11e6f8: show-off clock += event value seconds, first rider only). Trigger-script ops (`TriggerScript_ExecOp`
  0x13bfd0): 6 points pickup 500/2000/5000 (+0.04 meter), 0xe ×2/×3/×5 multiplier (air/rail only), 0xf boost,
  0x10 show-off time, 0x11 speed boost timer, 0x12 spin-boost zone (spin/flip rates ×1.6 at takeoff while active).
- AI reset: > 60° off the path heading for 60 ticks, or ≥ 1000 cm off the path for 222/(skill/31.41) ticks.
- Shoves: target nearest rider within 700 cm (cos-weighted) every 12 ticks; shove if within 200 cm, 30–150° to
  the side and in bump contact.
- Countdown 2.5 s (t from −300, +2 per tick; beeps at 0.5, 1.0, 1.5, 2.0 s).
- Places re-sorted every 6 ticks by distance to finish (+ place·20); finish = trigger event 10.
- Advance when place < (N+1)/2 (N = number of riders).
- Show-off: clock counts down from the track's limit (gari 120 s; snow, elysium, mesa, merq, aloha, pipe, tokyo
  90 s; alaska 135 s); checkpoints (trigger event 0xC) add seconds. Medals (gold/silver/bronze): gari 55k/40k/25k;
  snow 95k/65k/35k; elysium 225k/150k/75k; mesa 225k/150k/75k; merq 275k/175k/125k; aloha 175k/115k/75k;
  tokyo 350k/225k/100k; alaska 500k/300k/150k; pipe 800k/500k/250k.
- Camera: named cameras from data/camera/commonob.cml (+ <track>.cml, scripts.cml); default "chase far"; air
  blend +0.0291667/tick to 0.98 in motion states 5/6, else −0.003/tick.

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


## Race lines, course events (`AIP_LoadPathsFromFile` 0x197da0, `Boarder_UpdateRaceLineProgress` 0x118298)

- Path: start + segments {dir, len}; len is horizontal (2D) — event distances are along the 2D length. DTF per race line.
- File event type → runtime: −1..23 → +1; 100..105 → 0x19..0x1e; 300 → 0x1f (branch).
- RaceLines events (file type): 0 announcer, 1/2 3/4 5/6 shortcut 1/2/3 entry/exit, 8 announcer, **9 finish**
  (when laps left = 0), **11 checkpoint** (show-off: clock += value s, first rider to reach it only; race: split),
  **12 lap**, 13/22/23 sound zones, 17 final stretch, 18–21 HUD tips, 300 branch.
- Progress: project onto the current line (2D, clamped per segment), dtf = DTF − along; best dtf only improves;
  events between old and new best fire (enter), exits when no longer inside.
- Line choice (`Boarder_SelectRaceLine` 0x118610): when > 500 cm off (every 60 ticks), near the line end (−200 cm,
  excluding current), or at a branch event: P = pos + 800·v̂; 3 nearest lines by bbox; cost = |P − pt(a+800)|² +
  |pos − proj(P)|²; lowest wins.
- Laps: Tokyo Megaplex only, 4 laps. Placing key = −(distance to finish + place·20), every 6 ticks.
- AI paths: same selection over 6 nearest, minus (100 − |U3 − pref|)·23187.357 for AIs; pref by class and meter/place
  (freestyle: 100 if meter < 0.312, 0 if place 3–4, else 50; BX: 100 if meter < 0.103, 0 if place 2–3; alpine: 0 if
  leading else 50). 103/105 move respawns back/forward out of zones; 104 = no-reset zone.

## Rails (more), board classes, circuit

- Hop-off: prewind works on rails (wind-up rate doubled, 4.001), jump Δv = max(630.88, ...) along the board's up
  (rail normal tilted by the balance offset); spins/flips from rails as on the ground. Running off the end: no
  velocity change, into the air. Grind scored the same on every exit. Re-capture weight −0.25 → 1 at 1.667/s.
- Balance sign: positive steer moves the rider to the left of travel (same as turning left on snow).
- Board class: 1 freestyle, 0 BX, 2 alpine. Switch penalty (drag & grip) none / ×0.85 / ×0.70. Yaw-to-velocity
  (a, b): BX (0.30019, 0.34919), freestyle (0.4, 0.52398), alpine (0.20834, 0.26221). Alpine lean 65° (else 50°).
  Per character boards: index 1 alpine, index 6 BX, others freestyle. Board unlock points 0, 0, 5, 20, 35, 60, 90,
  120, 160, 200, 240, 999.
- World Circuit: tracks gari, snowdream, elysium, mesablanca, merqury, tokyo, aloha, alaska. Race: 3 rounds of 6
  riders, top 3 advance; medal = place in the final (gold/silver/bronze = 15/10/5 points). Show-off: medal by score.
  A medal unlocks the next track of that event; Alaska race medal → Untracked, Alaska show-off medal → Pipedream.


## Tokyo Megaplex laps

- 4 laps (vf8 0x116fac, track 8): lap line (file 12) takes one off, finish (file 9) only on the last.
- The glass tube on race line 1 (instances Mdl_Endboost_Lap/_Z/_End, effects type 0 subtypes 15/18/24):
  cLapBoostNode (0x140b90) only with laps left: within 10 m of the tube centre, nudge in, v.xy ×0.92/tick,
  v.z → 2500 cm/s at rate 5/s; cZBoostNode (0x141cd8) sets z = 0 (top of the course) when inside, velocity kept;
  cTubeEndBoostNode (0x1418a8) pushes toward d_k at S_k (rate 2/s): k = 0 rode in low (d (0.159, 0.954, 0.254),
  2700), k = 1/2 from the −x/+x side ((0.226, 0.904, 0.362) / (0.092, 0.925, 0.370), 3500).
- Distance to finish adds n·L (L = race line 0's DTF); progress best resets when dtf jumps up by > L/2.

## Camera scripts (CML)

- Regions in the track CML files are replay-only (live-race flag 0) on Garibaldi and Snowdream; in a race the CML
  only drives the intro: PreLoad → fly-through (50/50 of two) → staging scene → gate shot (5 s) → chase camera.
- Director script ops: 0 cut, 1/0x1C linear blend, 2/3 orbit blend, 5/6 chase near/far, 0xA random branch,
  0xB call, 0xD back to chase, 0xE scene, 0x20 end. Blend curves 0 linear, 1–3 Hermite variants.


## Animation timing

- Clips are 30 fps data played at 0.5 frame per 60 Hz tick × rate (`AnimClip_Advance` 0x15a030); a clip of n
  frames reaches its end after 2(n−1) ticks and chains on the next.
- Grab (and tweak) clips play at rate 0.782 + 0.806·s14.
- Rail quarter turn: a 15-frame clip, committed at frame 14 (28 ticks, 0.467 s); the clip turns the board.
- Get-up: by slide (butt/face) and direction to the course: FROMFACEFWD 30 frames, FROMBUTTFWD/BUTTRIGHT/FACERIGHT
  35, FROMFACEBWD 41, CT_ROLLBWD2BASE 51; riding resumes 2n−1 ticks after it starts.
- Clip descriptor table 0x325fa8 (7 ints per game clip id: category, lookup mode, update type, end action, layer,
  blend-in, fade-out); game clip id → name via `AnimClip_ResolveAnmIndex` 0x15ada8.


## Trick names and points (`Score_BuildTrickId` 0x157490, `Score_FormatTrickName` 0x1551c0)

- Parts in order, each with a trailing space, no "+": rail type, rail half-turns, prefix ("Switch ", "Rail To ",
  "Rail To Switch ", "Late "), side ("BS " spin > 0, "FS "), spin degrees (no flip), flip count ("Double ",
  "Triple "), flip type ("Front Flip ", "Back Flip ", "Rodeo " back + 540/flip, "Misty " front + 540/flip),
  special name (table 0x320cf0 by side, half turns, flips: Mindless, Deathwish, Banzai, ...), spin after the flips
  (when not exactly 540 per flip), grab 1, "To Late " + grab 2 (3+: "Combo Grab"), suffix ("Air" for a plain grab,
  "To Fakie", "To Rail"). More than 1800 of spin or 4 flips: "???".
- Points = units × 0.6786545 × 10000 × multiplier, rounded to 10. No off-axis bonus. Grab hold rates by grab
  (Indy/Method/Mute/Stalefish 1.0 … Cross/Experimental 2.5), tweaks 2×, ubers 18/20.
- Late tricks (`SpinState_StartFromStick` 0x102158): with the stick let go, no grab, and both |ω| < 0.7, a
  press restarts rotation at 5.2531(0.5449+0.6742T) per axis (no 2/3 on flips); the trick so far is banked
  (Score_Takeoff) and the rest is named "Late …" with no repeat check.
- Multiplier: 1, ×1.3 only for a trick off a rail left switch, pickups replace it (max). Repeats: last 5 IDs,
  points and meter ÷ (n+1). No landing-quality grades.


## Pre-race scenes, surfaces, meter (more)

- Stage scripts (`PreRaceSel_*` 0x168720–0x169378): a race picks STG_COM_1–4 and GAT_6COM_1–4 at 25% each (circuit
  heats 2–3 skip staging and the fly-through; friend/foe gates when such a rider is in the field; show-off GAT_SO).
- Scene records (CML type 5): 8 × {anim script, enabled, rider selector (13 = next rider), place, frame (1 = start
  stage area `Mdl_StageArea_Start_0`), xyz, yaw}. Staging clips scrSG_* (S_COM1–4), gate clips scrGAT_6COM{n}{a–f}.
  Cameras with +0x30 = 1 are relative to the stage area too. Op 0x17 = white flash 0.6 s; 0x29 rider announcement.
- Gate: G_GATESTART's frame follows the rider's gate progress (anticipation in the countdown with the stick,
  launch at GO at k·0.2147/s, k = (0.4876+0.7181·b)·4.4377).
- Surfaces are the patches' SurfaceType (0 reset, 1 snow, 2 off-track, 3/4 powder, 5 ice, 6 bounce/unskiable,
  7 ice-water, 8 glidy powder, 9 rock, 10 wall, 11 ice, 12 wood, 13/14 metal, 15 standard, 16 sand, 17 no
  collision, 18 show-off ramp metal); the ground spring band (d1, d2) is per surface (powder sinks 15–35 cm).
- Meter: tricks give unmultiplied points/10000/(repeats+1), never negative, humans only; crash −0.1 (cap to 0.666
  only when crashing mid-trick in the uber window), stumble −0.02, reset −0.12, knockdown +1. Boost drains 0.00075
  per tick on ground/rails only (no boost in the air); the boost cap goes by the meter left; boost mass ×(1+8·level).


## World objects (`Boarder_InstanceCollisions` 0x125088)

- Crash bags / cans (cRollerNode, MainType 0 sub 0): launched with ½·approach speed along the rider's heading plus
  an upward pop min(100 + 5000/m, 600) cm/s, spin 10 rad/s; gravity −980; contacts e = 0, friction removes half
  the sliding speed, damping c·(1+2t); asleep under ~1 m/s or after 10 s where it lies. The rider loses no speed.
- Path markers and broken glass (cMeshAnimNode, sub 20): parts thrown with ½·s·v̂ + random (±2, ±2, +4 m/s),
  gravity 58800·U1 cm/s², no ground, gone after U2 s (markers 2 s). Markers have mass 0: the rider stumbles.
- Glass breaks on any contact (no threshold, no points); fences flex only. Pickups are used up for the race
  (DeadNode mode 2) by whoever touches them first, AI riders included; gems/points pickups only in show-off.
- HUD words (american.loc): "KNOCKDOWN!", "BIG AIR BONUS", chain "1x COMBO"…"4+ COMBO", race checkpoint
  "CHECKPOINT" + split, show-off "TIME BONUS" + "%d seconds", "FINAL LAP" / "%d LAPS TO GO", "FINISH", "TIME UP".

## Start gate (`GateAnticipate_*`, `AIComputer_GateAnticipateInput`)

- The gate clip is a 0..1 position p with two markers: bx/fr boards 16/30 and 20/30, alpine (ex) 0.4 and 0.8.
- During the countdown (state 8) the stick sets a target: up → m0, nothing → m0/2, down → 0. p moves towards it at
  k·max(|target − p|, 0.2146618) per second, k = (0.4875571 + 0.71808594·gateStat)·4.437673. The highest and
  lowest p reached and the time p has been still are kept.
- GO: launch speed 5.5555 m/s (20 km/h); if the target is back (< 0.5) and p moved within the last 1/150 s, x =
  (hi − lo)·20.248037·(1 − p)/(still·60 + 1) replaces it when larger. × (0.4875571 + 0.71808594·gateStat).
- After GO (state 9) p runs to 1 at k·0.2146618; at m1 the rider gets the launch speed along its heading; it is
  held (no control) until m1 and rides normally from p = 1.
- gateStat per character: Kaori/Marisol 0.80, Luther 0.64, Mac 0.85, Moby/Seeiah 0.75, Zoe 0.82, JP 0.88, others 0.70.
- AI: with left = ticks to GO and sc = 1 (skill ≥ 1) or skill/3.2585914, forward when left < 30·sc, back when
  left < 60·sc, else rock (turn round each time p reaches the target).
- Result: rocking back about 26 ticks before GO gives ~10.7 m/s instead of 5.5.

## Rivals and grudges (`RelTable_Init` 0x167208, `Rider_OnKnockedDownBy`, `AI_StartVendetta`, `AI_CoolVendetta`)

- Each rider has a friend and a foe and a 12×12 attitude (0..100) and tolerance table (src/rivals.rs).
- The player hitting an AI adds to its grudge: knockdown 30, shove-stumble 15, bump-stumble 9 (capped at the
  tolerance). Reaching the tolerance starts a vendetta: level 0 for a friend, else 2 if attitude ≥ 80.98926,
  1 if ≥ 60, else 0; attitude += 15 (max 100); grudge reset to 0.
- Level 1 cools after 360.45 ticks (grudge −15). Hostile (chasing) = level 2, or level 1 not yet cooled.
  Only level ≥ 1 riders shove. An AI knocking the player down or off balance takes 45 (knockdown: max(45, g/2)).
- Attack steering (`AIComputer_AttackTarget`): target ahead → steer for target + 3 m along its velocity; tuck when
  within 50°, boost too within 15°. Target behind, > 2 m away and slower → brake.
- Taunts (bxRT_AITAUNT1-5 behind, bxRT_TAUNTHS/TS1-5 beside): grudge holders within 10 m with the player more than
  60° off their heading, after 1 s of riding, a 9% roll per check.

## Sound and music (`Audio_*`, `Music_*`, `cMusicSys` 0x343150)

- Countdown beeps at 0.5, 1.0, 1.5, 2.0 s; no GO sound of its own (the song is unpaused at GO, `Audio_OnRaceGo`).
- Songs: musicmap.inf enables up to 4 songs per course; at race start the song is advanced rand(0..21) times
  through the enabled ones; the pause menu's Music Selection steps through them. Songs loop.
- Music events: 0 song start, 10 finish on the last lap (once), shortcut music zones 1/3/5 on entry, 2/4/6 on
  leaving (after 1 s). Pause sets the music rate to 0.
- Big air (air time > 1.5 s): music × clamp(1 − 0.437·T, 16/127, 1) with the wind brought up; a big-air loop
  phrase plays on the next beat. "It's Tricky" is the last big-air phrase, played when the meter fills with the
  uber timer at 0 (plus the announcer's "It's Tricky" while ubers < 6); it plays at least 16 beats and stops when
  the uber timer runs out.
- Ambient zones: trigger 0xE tunnel loop, 0x17 river loop (volume 35), 2 s fade out. Crowd reacts to landings and
  wipe-outs in tiers by score (≤ 0 calm, < 0.25, < 0.65, else top); full cheer at the finish.

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

## The finish (`Boarder_Finish` 0x11da40, `FinishState_Update` 0x106668, `cEndRaceHandler_Update` 0x1141e0)

- Every rider over the line enters state 6 (pad/AI ignored): substate 0 brakes hard until under 277.78 cm/s;
  1 coasts at ×0.93 a tick to a stop; 2 stands up (bxR_CRUISE2FINISH) and reacts: win (place < (N+1)/2;
  show-off: any medal) → random bxRR_POS1-10, else bxRR_NEG1-9, then cmFL_FINISHCYCLE. Each stage gives up
  after 301 ticks.
- The race ends when every human has finished and stopped for 240 ticks (4 s). Riders still racing are frozen.
- Then up to two camera-scripted scenes (a rival's FL_<RIVAL>_VS_USR, then FL_WIN_n / FL_LOS_n) and the results.

## Post-race scenes (`cEndRaceHandler_Update` 0x1141e0, `PreRaceSel_ResolveWithRoster`, scripts table 0x32f380)

- Scripts by index: 27–29 FL_WIN_1..3, 30–32 FL_LOS_1..3, 45–56 FL_<EDD..MAR>_VS_USR, 57 Null. Rows (12 bytes):
  b0 player character (12 any), b1 course id, b2 mode, b3 tier, b4..b8 required co-riders, b11 script; best score
  wins, ties at random. Win table 0x37c1b8 (63 rows), lose 0x37c4b0 (52), rival 0x37c720 (133).
- Each win/lose row fixes the player's clip (scr<Chr>_WIN_n / _LOS_n) and the script; per character (clip→script):
  win EDD 1→3 2→3 4→3, KAO 1/2/3→1, LUT 2/3/4→1, MAC 1→1 3→3 4→1, MOB 1/2→2, ZOE 1→3 3→1 4→2, JP 2/3/4→2,
  ELI 1→2 2→1 4→3, PSY 1/2/3→2, SEE 2→2 3/4→1, BRO 2/3/4→1, MAR 1/2/3→2; lose EDD 1/2→3 3/4→2, LUT 1→3 2..4→2,
  MAC 1..3→2 4→1, ELI 1→2 2→3 3→2 4→3, PSY 1→1 2→3 3→2 4→3, BRO 1→1 2..4→3, others all →2.
- FL_WIN_n: flash, scene "WIN_1", FL_PAN_n, camera FLchar_1, stop at 4/4/6 s; FL_LOS_n: scene "LOS_1", PAN_LOSEn,
  camera FLchar_2, stop at 4.5/5/6 s. Slot 0 the player (scr<Chr>_WIN/LOS_n), slots 1–5 the others in order
  (scrFL_Common1..5). Rival FL_<RIV>_VS_USR: scene "<Riv>_U", slot 0 the player (scr<User>_RCON_n), slot 1 the
  rival (scr<Riv>_CON_n), slots 2–5 scrFL_Common1..4; n random of 4 (Psymon rival 1..3, Psymon player 1/3, Elise
  player 1..3). All scene spots are relative to Mdl_StageArea_Finish_0 (frame 2). Clips loop. The rival scene
  plays first; the rival is the rider whose feeling toward the player is strongest (> 2).

## HUD (`HUD_DrawPlayerSprites` 0x1a2c40, `HUD_DrawPlayerText` 0x1a4f70; 640×480 virtual)

- Place (66,27) ×1.9, gold (0.953,0.729,0.106) when 1st; time (26,460) m:ss.cc; speed (90,430) km/h; score
  (620,24) green while pulsing. Boost meter: 14 segments at (570,140) in three bands, fill slews 0.005/frame.
- Trick name: gold (1,0.867,0), centred 38 px above the bottom, 3 s, no fade, replaced by the next. Pending points
  (320,80) tinted by size (green ≤1000, blue, yellow 2501+, orange 4001+, red 7501+, navy 11501+). Banked points fly
  to the score over 2.5 s. Multipliers ×2/×3/×5 at (320,110). Combo and air bonuses 2.2 s. Crash shows nothing.
- Split (5 s): label then ±m:ss.cc (red behind, green ahead). Knockdown 2.1 s. No wrong-way indicator.

## World Circuit (`Circuit_*` 0x16b000–0x16d400)

- Each race is heat 1, heat 2, final (6 riders each); 1st–3rd go through, 4th+ ends the event; AI riders
  finishing 4th–6th are out for the event. Medal = place in the final (show-off: by score).
- Points 15/10/5 per medal, paid as the improvement on that track and event; 240 in all fills every attribute
  bar (cap − start sums to 237–240 per character). Rank thresholds 0, 5, 20, 35, 60, 90, 120, 160, 200, 240 (Master);
  boards unlock at the same thresholds (the 999 board from the trick book). Each gold unlocks the next character
  (order 3, 4, 7, 0, 10, 5, 6, 1, 11, 8, 9, 2). A medal on a track unlocks the next in order 0, 1, 2, 3, 4, 8, 5, 11.
- Computer riders gain a quarter of the player's points as attribute points (f = min(1, 0.25·pts/(100 − base))).

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

## Unlocks (`Profile_ResetNewGame` 0x161b40, `Circuit_ProcessUnlocks` 0x16c420)

- New game: tracks Garibaldi, Snowdream, Elysium (and the tutorial) open (0x407); riders Mac, Moby, Elise, Eddie.
  Each improved gold opens the next rider: Brodi, Zoe, JP, Kaori, Marisol, Psymon, Seeiah, Luther.
- A medal on track k of the order Garibaldi, Snowdream, Elysium, Mesablanca, Merqury, Megaplex, Aloha, Alaska
  opens the next; a race medal on Alaska opens Untracked, a show-off medal there Pipedream.
- Outfits: 7 per rider; 1–5 from trick book chapters 1–5, 6 at Master. The trick book (6 × 5 tricks) is in
  data/tutorial/trickdef.dat (28-byte records); finishing it gives the UBERBOARD (999).
- Cheat (hold the two shoulder buttons in mask 0x600, then 12 presses): X ▲ → ● ■ ↓ ▲ ■ ← ● X ↑ unlocks everything.

## Rivalry (pair record boarder+0x40+other·0x20)

- Fields: distance, bearing, attitude, cap, tolerance, grudge, base attitude, relation, bump/shove/knockdown
  counts, rivalry score (+0x1c). Score: vendetta start +1/+2/+3 (level 0/1/2), an AI shoving the player +1,
  knocking the player down +2. The post-race rival scene plays when an AI's score toward the player is > 2.
- After a race attitude settles: att = base + (bumps + 2·shoves + 3·knockdowns)/(3n)·(att − base) (back to base
  with no contact); only AI→player attitude carries over the circuit (and into the save); between heats it
  decays (friend −3, neutral −2, rival −1).
- Taunts (`Ride_AITauntCheck` 0x103660): riding ≥ 1.025 s, rand%100 < 91, the vendetta target (any level) within
  10 m and more than 60° off the heading; behind (> 135°) AITAUNT, else the side's TAUNTHS/TS.

## Scoring (the rest)

- Modes: 0 knockdown practice, 1 free ride, 2 practice race, 3 practice show-off, 4 circuit race, 5 circuit
  show-off, 6 lesson, 7 timed race. Trick points are the same in every mode; nothing is banked after the finish.
- Points, boost, multiplier and show-off-time pickups only work in show-off; speed and spin boosts everywhere.
  Points pickups never reach the score total. Breaking things scores nothing; a knockdown gives +1 meter only.
- TRICKY = more than 5 cleanly landed ubers; lasts the event: meter and uber timer held at 1, no drain, no uber
  bail penalty, no points multiplier. Bail: chain 0, pending trick lost, meter −0.10. Stumble −0.02.
- Trick-trigger zones (controller 0x15): run a script if the rider's score rises by a threshold within a time.

## Trick book (DATA/TUTORIAL/TRICKDEF.DAT, `Score_TrickBookCheck` 0x157a00)

- 12 sets (one per rider, in character order) × 30 records × 28 bytes; 6 chapters of 5. Only the first
  unfinished chapter counts. A trick matches on its id word (combo name, grabs, spin size) and flip bits
  (count and type); the spin's direction, a switch takeoff and how it lands ("Air", "To Fakie") don't matter.
- trickId: byte 0 named combo (Crippler … Roadkill, 44 "???"), byte 1 grab 1, byte 2 grab 2 (1–26 grabs, 27–52
  tweaked, 53–100 ubers, 101 Combo Grab), bits 24–27 spin ×180 without flips, 28–31 spin with flips.
  flags: 0–3 crash name, 4–7 rail spin, 8–11 flip type (front, back, rodeo, misty), 12–15 rail type, 16–19
  takeoff (switch, rail to, rail to switch, late), 20–23 ending (Air, To Fakie, To Rail), 24–26 flip count,
  27–29 FS/BS, 30–31 two or more grabs.
- Chapters: 1 grab Airs, 2 tweaks and a 360, 3 flips and 540–720s, 4 360 flips, Misty/Rodeo, 900–1080, 5 doubles
  and named combos, 6 the rider's five ubers (the last its signature: Eddie Worm, Kaori Pirouette Grind,
  Luther Bronco Buster, Mac Walking The Dog, Moby SuperMan Barspin, Zoe Pommel Me, JP HeadSpin 2 Poseur,
  Elise LaLaLa Lock Step, Psymon Guillotine, Seeiah Soul Grind, Brodi Hang 10 Backflip, Marisol Aerial Spock 540).
- Each rider's uber names by the grab they start from are in src/trickdata.rs (UBER_NAMES); an uber's name
  replaces its grab's in the trick name.

## World controllers (`TriggerScript_ExecOp`, `TriggerScript_Op0_SpawnController` 0x13c600)

- Controllers attach to instances (inst+0xe4); flags inst+0xe8: 0x4 animated, 0x80 bounces the rider, 0x800
  disabled. Script slots: 1 on collide, 3 on rest, 4 on timer/counter/break. Lifetimes are seconds × 60.
- Types: 0 Roller, 2 Debounce, 5 Dead, 6 Counter, 7 Boost, 8 Timer, 9 Rail (enable/disable a rail), 0xa UVScroll,
  0xb TexFlip, 0xc Fence, 0xd Flag, 0xe Cracked (health −= |v·n·k|·0.036 per hit, 30-tick cooldown), 0xf LapBoost,
  0x10 RandomBoost, 0x11 CrowdBox, 0x12 ZBoost, 0x13 UVScrollTexFlip, 0x14 MeshAnim, 0x15 TrickTrigger,
  0x16 Particle, 0x17 Movie (jumbotron), 0x18 TubeEndBoost, 0x100 AnimObject (keyframed model, loop once / wrap /
  ping-pong, speed/30, collides via a cube AABB), 0x101 AnimDelta (advances only while given delta), 0x102
  AnimCombo (loop + one-shot), 0x103 AnimTexFlip. Op 2: Emitter, SplinePath (vehicles/conveyors: N copies spaced
  along a spline, dist += v·100/60 a tick; mode 3 keeps 6000 units ahead of the race leader), CollideEmitter.
- Other ops: 1 camera anim, 3/9 SetParam, 4 wait, 5 conditions (rider speed in km/h, random, human, has
  controller), 6 points, 7 run another instance's script, 8 sound, 10 stop, 11 kill/restore, 0xd reset rider,
  0xe–0x12 pickups and boosts, 0x17 camera param, 0x18 teleport rider to an instance, 0x19 rail state.
- No hard-coded per-track hazards: everything moving comes from level data. Track ids: 0 Garibaldi, 1 Snowdream,
  2 Elysium, 3 Mesablanca, 4 Merqury, 5 Aloha, 6 Pipedream, 7 Untracked, 8 Tokyo Megaplex, 9 Big Air Dome,
  10 Trick Tutorial, 11 Alaska.

## Uber sets per board

- Each rider has a set of ubers for each board type (bx/fr/ex <Chr>Uber.afl), on Indy, Method, Mute and Stalefish
  with different styles (MT/OT/SK); the signature uber (UT_SIG<CHR>) is only in the set of the rider's own
  board type: freestyle for Eddie, JP, Kaori, Mac, Seeiah; alpine for Brodi, Marisol; BX for the rest.

## Level scripts (SSFLogic.json; `TriggerScript_Run`, `World_CellActivate_RunPersistant` 0x25fed0)

- A script is a list of effects run as its own thread (instance, rider, contact) until its first wait (op 4,
  WaitTime seconds, at least a frame); a condition (op 5) returning false ends the thread. EffectSlots:
  PersistantEffectSlot when the object's area wakes (near a rider or the camera), Slot3 when it sleeps,
  CollisionEffectSlot on rider contact (only when the object has no controller; flag PlayerCollision and no
  surface type; one object per rider per frame, every frame), Slot4 when a controller stops, EffectTriggerSlot
  when a controller reaches its goal (counter at 0, timer, broken, trick points).
- Op 5 (type5: U0 kind, U1 int, U2 float): 0 speed (M0 ≤ T cm/s, M1 ≥ T km/h), 1 random (M0 passes with
  probability T), 2 human only, 3 stop if the object already has a controller (the usual "once" guard, then
  op 0 Debounce). Op 7 runs EffectHeaders[EffectIndex] on Instances[InstanceIndex]; op 21 runs
  Functions[FunctionRunIndex]; op 13 resets the rider; op 24 teleports; op 25 switches a rail.
- The engine fires named functions: RaceMode (modes 0, 2, 4, 7, 9), ShowoffMode (3, 5), FreerideMode (others) at
  the start; StartCountDown / NoCountDown; EndCountDown at GO. In the data they call HideShowOff (show-off rails
  off), HideRace (race-only objects dead), HideStartGate, CountDownStart (the lights).
- DeadNodeMode: 0 restore, 1 stop (Slot4), 2 hidden with no collision for good, 3 also stops spline riders,
  4 hidden until the area sleeps (broken logos come back).
- Boost (sub-type 7): Mode, U1 re-arm seconds, U2 rate per second, BoostAmount ×100 cm/s target, BoostDir world
  direction; while touching: d = target − v·dir, if d > 0: v += dir·d·rate/60 a tick.
- Spline riders (op 2 sub-type 1): SplineIndex, U1 mode (0 stop, 1 loop, 2 back and forth, 3 pace the leader 60 m
  ahead), U2 orientation, InstanceCount copies spaced evenly, AnimationSpeed m/s, U5 yaw offset, U6 draw the
  cable in (R, G, B) — the Merqury subway, Snowdream's ski lifts.
- MeshAnim (sub-type 20): breakable debris; U0 stays broken, U1 gravity scale (×980 cm/s²), U2 life, U3..U5 base
  velocity (0: the rider's), U6..U8 random spread, U9 multiplier. Cracked (14): U1 health, hits take
  |v·n| km/h (30-tick cooldown), U0 reset seconds. Fence (12): FlexAmmount bend, decaying zig-zag.

## Object collision (`World_RayCast` 0x25aff8, `World_RayCastInstance` 0x25bf48, `World_QueryInstances` 0x25b878, `Boarder_InstanceBounce` 0x125a00)

- Instance record (inst+0xec): +0 mass (U0), +4 PlayerBounceAmmount, +0xc SurfaceType, +0x10 collision mode
  (short), +0x12 collision model index; inst+0xe8 flags 0x20 PlayerCollision, 0x80 PlayerBounce.
- SurfaceType ≥ 0: the object is part of the ground ray (`Boarder_GroundProbe` and every world ray) exactly like a
  patch: the hit's surface row is the instance's SurfaceType (ramps 1, glass floors 5, metal frames/platforms 13,
  wood 12, cliff walls 10 ...). Such objects are never in the bounce test.
- SurfaceType −1: never ground. `World_QueryInstances` (flag 0x20 and SurfaceType −1) gives the contact
  (Instance_OnPlayerContact = its scripts) and, only with PlayerBounce, `Boarder_InstanceBounce`: mass 0 →
  `Boarder_Stumble` only (no push; megaplex spinners); else pushed out by 1.1·depth and v += n·max((1+b)·vin,
  vin + 55.56 cm/s) (b = PlayerBounceAmmount, usually 0.5), then `Boarder_WallImpactCheck`. Without PlayerBounce
  the object is not solid at all (glass panes and undersides, boost volumes, penguins, fan blades, icicles):
  only its touch scripts run. Here: −1 + bounce objects are walls on every face (tops bounce, never wipe out
  as "surface 6"); the bounce amount is the rider's fixed 0.5; mass-0 stumble not done.
- Mode 1 collision models have one sub-mesh per model part that has a mesh, in part order, each in that
  part's frame (posed by the part's matrix); mode 3 uses the drawn meshes; mode 2 is a box (not solid here yet:
  tree trunks, rail supports, crowd stands, pylons).
- Moving objects: `World_RayCastInstance` poses each part with `cAnimObjectNode_GetFrameMatrix` when the
  instance has an anim controller (flag 0x100 of controller+0x14), and QueryInstances recomputes the box, so the
  collision of every keyframed object follows its clip (flippers, BEntrance ramps, bumpers, pillars, the
  rotating door's reset zone, aloha's side-to-side barriers and boats, merqury's sewer fans, mesa's bridge).
  Clips a touch script starts (op 0 sub 256/258) play from the start; mode 0 ones end where they rest.
  Here: `CollisionWorld::movers` keeps their triangles per part and re-poses them when the clip moves (the
  game pushes the drawn clip's time; the sim runs its own clock and plays a clip when its trigger box is
  touched). The rider is pushed out / lifted, not carried sideways (the original adds no platform velocity).

## Keyframed objects (`cAnimObjectNode` 0x198810, `cMeshAnimFrame::vf1` 0x1cb498, `AnimCurve_FindSegment` 0x1cb770)

- Model objects with an Animation: base pose U1..U6 (tx, ty, tz cm; rx, ry, rz degrees), AnimationAction a mask
  of animated channels (bit i = channel i; entries in bit order), each a list of cubic segments
  ((V1 t + V2) t + V3) t + V4 over [V5, V6] with t absolute seconds, clamped at the ends. Animated channels
  replace the base; local = T · Rz · Ry · Rx; world = parent · local. The object's static matrix is unused.
- The node's clock: frames / 30 s; start U1, end U2 (−1: the model's AnimTime), rate U3 frames a second (30 =
  normal; random up to U4), U0 mode 0 once, 1 loop, 2 back and forth; U6 random start; U7 = 4 reversed.
  Fans, penguins, sailboats, side-to-side barriers, helicopters, the multiplier gems (which spin at 1.5× and
  are never removed: their touch script only sets the multiplier and bursts particles).

## Particle emitters (`Emitter_InitFromRecord`, `PBurst_*`, VU1 microcode at 0x31c288)

- type2Sub0: U0 N particles born evenly over U2 s (U2 < 0: continuous, recycled every T = U5 + U7/2, pre-warmed);
  U1 trail copies (copy j drawn at age − j·U8, alpha × (1 − j/U1)); U3 drag k (1/s); size U4 ± U6/2; life
  U5 ± U7/2; origin U9..11 + u·(U12..14) + u·(U15..17) and velocity U18..20 + u·(U21..23, U24..26, U27..29)
  through the instance matrix (u ∈ [−½, ½)); acceleration U30..32 world; pos(t) = P + W t + (V − W)/k ·
  f(min(k t, 2.7)), W = accel/k, f(x) = 0.73x − 0.113x²; colour (×128) c0 → c1 over T plus random c2, c3
  (keys at U33.., U37.., U41.., U45.., alpha first); U49 texture in particle.ssh (part, snfl, clod, spry, halo,
  ...), U50 blend (0 additive, 1 alpha, 4 darken). Collision-slot emitters burst on contact; sub-type 2 bursts at
  the rider's hit point (cap 1 per AI, 3 per human).

## Rider sounds (`Audio_SurfaceGroup`, `BoardIn_*`, `Voice_Update3D`, DATA/CONFIG/SNOW.INF)

- Surface groups: 0 PACK (1, 15–17, default), 1 POWDER (0, 3, 4), 2 LOOSE (2), 3 ICE (5, 6), 4 METAL (13),
  5 WOOD (12), 6 RAIL, 7 ROCK (9–11), 8 GLASS (14), 9 CHUTE (18, 19). zboard.bnk program = group·8 + slot:
  1 landing, 2 takeoff, 3 carve loop, 4 glide loop, 5 AI landing, 6 AI glide; program 0 the slow scrape.
- SNOW.INF programs (accumulator + stack: Load, Map n → x·127/n clamped, Square, Sqrt, Add/Sub/Mul/Div,
  Bound, Push/Pop, SAdd..., AssignVol, AssignBend) per section and GLIDE / AIGLIDE / CARVE, on LOAD = |v·fwd|,
  SLIP = forward drag deceleration, DIG = |steer|·127, LEAN = LOAD/4 + 2·SLIP, BEND = LEAN + LOAD (cm/s).
- Scrape: vol curve (10,0)(60,127)(200,100)(300,0), bend (10,0)(30,30)(500,80)(900,127) of LOAD. Takeoff vol
  clamp(LOAD·127/1000, 64, 127); landing vol by impact (100,22)(300,60)(800,100)(1200,127); crash zbxsfx 48–50
  (64 in powder) by impact (100,80)(800,90)(1200,105)(1600,127), slide loop 51.
- zbxsfx: 32 wind (big air only; gain = 1 − music duck), 0x77 grab, 0x78/0x79/0x7a boost by meter, 0x6c boost
  empty, 0x72 trick boost, 0x73 speed boost, 0x74/0x75/0x76 multipliers, 0x7b reset, 0x6b uber ready, 90/91
  rider bump, 0x31 wall. 3D: vol × ((R − max(d − 0.5, 0))/R)², R 40 m (human loops), 50 m (AI glide), 30 m
  one-shots. BNK: 'BNKl', u16 version, u16 program count, offsets from byte 20 (0 = none).

## Sound banks (EA BNKl; `SND_ParsePTLayer` 0x2d5410, `SNDVoice_CalcPitchMult` 0x2d5288)

- 'BNKl', u16 version 5, u16 program slots, u32 header size, u32, u32 data size; slot i at 20 + 4i holds an
  offset relative to the slot itself (0 = empty). A program: "PT" u16 5, tags (u8 tag, u8 length, big-endian
  value; FC pad, FD sample header, FE next layer, FF end). 07 root note [60], 0A bend range semitones [0],
  0C pan [64], 0E volume [127], 0F volume random, 10 detune cents, 11 random pitch cents, 1D/1E tremolo,
  20/21/22 vibrato table, length, depth cents; sample: 84 rate, 85 samples, 86 loop start, 87 loop end
  (inclusive), 88 data offset, A0 codec (5 VAG, 9 signed 8-bit PCM after a 16-byte header).
- Pitch: cents = detune − (root − note)·100 + (bend − 64)·range·100/64 (note 60), ratio 2^(cents/1200); bend
  does nothing without a range. Volume: (vol ± rand)·velocity/127 × request × envelope / 127³. Loops play to
  the loop end once, then loop start..end.

## HUD sprites (`HUD_DrawSprite` 0x1c0068, `SpriteSet_InitHudGameRects` 0x1f4440, `SpriteSet_BindHudGameTex` 0x1f9080)

- 200 records {texture, draw w, draw h, v0, u0, u1, v1} written in code from immediates for HUDGAME.SSH (map1,
  map4, hud1; HUDTRICK.SSH in the tutorial). Boost segments 0x12–0x14 (red, orange, yellow) / 0x15 unlit,
  TRICKY letters 25–30 unlit / 31–36 lit, uber orb 0x25 / 0x26, medals 0x27–0x29, character badges 0x45–0x51,
  button glyphs 0x6c–0x75. Text uses data/fonts/title.sfn (shadow +2, +2) and menu.sfn.

## Unresolved

Ragdoll sphere places (masses known); song names (music.inf / musicmap.inf); post-race scene row filters; spark and dust
sprite rendering; HUD text (american.loc).

## Fonts (DATA/FONTS/*.SFN)

- `FNTS` files: header (version at 0x08, glyph count 0x0a, origin 0x10/0x11, glyph table 0x14, kerning 0x18, bitmap 0x1c), glyph records `{u16 code, u8 w, u8 h, u16 x, u16 y, s8 advance, s8 xoff, s8 yoff}` (+1 pad from version 200), a 4bpp linear bitmap with a built-in ramp palette (alpha i*128/15).
- Only TITLE.SFN and MENU.SFN are loaded (SMLFONT is never used). After loading the game scales them: title 1.4 x 1.3, menu 1.8 x 1.4.
- `Font_DrawStringA` 0x19d0c0: quad at pen + offset (both times scale), pen += advance; no kerning (the table exists but the loader never reads it), no extra spacing, missing glyphs skipped without moving the pen.
- Width for alignment (`Font_MeasureA`) is the glyph boxes' extent, not the advances.
- `Font_PrintfShadowedA` 0x19d568: black copy at +2,+2 (screen units, not scaled), same alpha, one layer below.
- In-race text (`HUD_DrawPlayerText` 0x1a4f70, 640x480): place number title x1.9 right-aligned at (66,27), suffix x0.7 at (71,30), gold (.953,.729,.106) in the lead; time x0.9 at (26,460) anchored at its bottom; points x1.1 right-aligned at (620,24); speed x0.9 right-aligned at x 90, y 430 with " km/h" at x0.7; multiplier "xN" x1.9 centred at (320,110) - x2 gold, x3 orange, x5 red; trick names in MENU font, centred at x 320, bottom 38 above the screen's foot, gold (1,.866,0), wrapped at 0.4 of the screen width, lines stacking upward.

## Level sounds (DATA/AUDIO/AUDIO.BIG, DATA/CONFIG/BANKS.INF)

- BANKS.INF per course: MAIN zbxsfx, BOARD zboard, TRICKY tricky (trickyut on Untracked), BANK = the course bank (garibaldi1, snowdream1, elysium1, mesabanca1, merqurycity1, megaplex1, iceberg, alaska1, untracked1, pipedream1, tricktutorial), CROWD Crowd.bnk, and a list of SWAP banks (one-sample loops: River, Snowcat, Truckidle, birds, cowbells ...). All live inside AUDIO.BIG.
- A world sound id picks the bank (`SoundId_ToBankProgram` 0x22d8a8): 0x4f-0x60, 0x67-0x93, 0x9f-0xb2, 0xb7-0xb9 a swap bank (program 0, loaded into slot 6; only one swap bank at a time); 0x61-0x63 crowd programs 0-2; 0x66 a random chant (C_*.bnk); the rest a program of the course bank from a table.
- Ambient emitters (`WorldEmitter_GatherForListener` 0x22b370, `WorldEmitter_Update` 0x22bf60): each instance with IncludeSound has ExternalSounds records; type 0 is a sphere at Location + (U2,U3,U4), radius U5, volume curve U6 of x = d/radius: 0 `1-x^2`, 1 `1-x/(1.5-0.5x)`, 2 `1-x`, 3 `(1-x)/(1+0.5x)`, 4 `(1-x)^2`, 5 flat to 0.7 then linear. No other distance loss; dropped emitters fade in 0.25 s; at most 40.
- Knocks (`Sfx_InstanceCollision` 0x216c50): the instance's CollisonSound id; volume by rider speed (cm/s) 200 -> 33, 400 -> 70, 550 -> 100, 800 -> 127, times ((30 - max(d - 0.5, 0)) / 30)^2 with d metres from the camera.
- Script SoundPlay (op 8, `Sfx_ScriptSoundPlay` 0x2165c8): a program of the course bank played once at the target instance, same 30 m falloff (the fireworks: program 82).
- Course music BIGs (DATA/AUDIO/GARI.BIG etc., per INTROMUS.INF): EA-XA stereo 22050 Hz segments A1-A4, B1-B4, C1-C8, end; start A1, A[2-4], B twice, then random C segments; `end` at the finish.

## Music (Audio_RaceInit 0x2116d8, Audio_InGameUpdate 0x20ee58, Audio_OnRaceGo 0x215288)

- Menus: the `[Menu]` Pathfinder song of MUSIC.INF, sections switched by events 0-5. The jukebox (jukebox.big) is only the DVD-extras player.
- Race start (all modes but the trick lesson): a random enabled MUSICMAP.INF song (at most four per course) is loaded, given event 0 and the Pathfinder variable 80, and paused; the course BIG plays the intro (A1, A[2-4], B, B, then a random C whenever fewer than two are queued; Mesablanca, Untracked, Megaplex, Alaska in order 0..15 then C1-C8 looping). End of intro (pre-race stage 0x12, or skip): queue flushed, `end` queued. GO: course stream stopped, race song resumed from the start.
- During the race the Pathfinder variable is set every frame: with rivals 99 - (place*99)/(n-1) (place 0-based); alone, clamp(boost*1.5,0,1)*99. Events: race-line trigger types 1 -> 0, 9 -> 9, finish -> 10 (once, with the crowd cheer); shortcut zones 1/2/3 post 1/3/5 on entry, 2/4/6 one second after leaving.
- In-air layer: the song's LOOPDATA bank, one sample per beat (60000/BPM ms), phrases of BeatsPerPhrase (8) from bank program phrase*PhraseAlign(16)+beat; normal phrase rand%(PPB-1), in a shortcut zone PPB+rand%2, Tricky PPB-1. Silent unless big air; Tricky at UBERLEVEL 85%.
- Big-air duck (Audio_BigAirMusicDuck 0x21b828): after 1.5 s of air the song drops to clamp((1-(1-floor/127)*t*0.5)*127, floor, 127), floor 16 (Untracked 50); the wind comes up by the same amount.
- "It's Tricky" (Audio_TrickyTrigger 0x21c610, when the meter fills): speech plus the Tricky phrase from the next beat, twice through, ending with the uber timer.

## World animations (`cTexFlipNode` 0x142e90, `cUVScrollNode` 0x141ff0, `cAnimDeltaNode` 0x199d10, `cFlagNode` 0x144938)

- All tick at 60 Hz; lifetime U×60 ticks (0 for ever), at 0 the controller dies (base texture back, scroll gone).
- TexFlip (11; update 0x1430b8): frames = the material's TextureFlipbook (by MaterialID); one frame for every
  flipbook mesh of the instance, count from the first. U0 start frame (< 0: ceil(8r); −2: a random frame
  each step), Direction (≠ 0 backwards), Speed frames/s (acc += Speed/60, step at ≥ 1; 0 holds), Length s, U4
  mode: 0 cycle with wrap; 1 blink: next frame for 6 ticks, then the next (wrapping to 0) for
  60/(Speed(0.25 + 0.75r)) ticks. SetParam (0x1433d8, ops 3 and 9): 1 rate a tick, 2 frame = ceil(v) for
  0 ≤ v < 9, 3 step (wrap to 1), 4 visible.
- Start lights: StartCountDown → CountDownStart: TexFlip U0 0 Speed 0 Length 7 on Mdl_StartLights, then frame
  1, 2, 3, 4 at 1.0, 1.5, 2.0, 2.5 s (GO) — off, red, yellow, yellow, green.
- UVScroll (10; update 0x1422e0): shared by every instance of the model. U1, U2 du, dv a tick; U3 run s, U4
  pause s (both ≤ 0: still; run 1 pause 0 = for ever), U5 life; U0 0 linear, 1 eased (du·min(τ, run−τ)/run),
  1 and 2 turn round after each run; offsets wrap into [−1, 1].
- AnimDelta (257; 0x199db8): a keyframed clip (the 256 fields) that only runs while its budget is positive;
  SetParam(2, N) adds N/30 s. Trees falling, sewer gates opening, Elysium's kicker.
- Flag (13; `cFlagNode_BuildStrip` 0x144ec8): the instance's one quad (v0..v3 in strip order) becomes 8 × 2:
  rows v0→v1 and v2→v3, column k at k/7. Phase random [0, 1), += U1/60 a tick (U1 −1: 0.5–2); column
  k = 1..7 moves U2·(k/8)·sin(2π(k/8 − phase)) along model X (U2 cm, divided by the instance scale; −1
  random); U0 = 1 mirrors (column 7 fixed). U3 life.
- Scripts run as threads: a wait (op 4) parks the thread; op 7 runs the other object's script at once.
  The collision slot runs only while the object has no controller: Debounce (2) keeps it busy D s (0 for good),
  a dead node for good.
