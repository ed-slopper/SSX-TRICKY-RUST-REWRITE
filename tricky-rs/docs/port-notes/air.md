# Jumps, air, tricks, landing: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Jump, air, tricks, landing

- Prewind `PrewindState_Update` 0x1050e8: crouch → 1 at 5.0025·max(|1−c|,0.1)/s (full ≈0.66 s, exponential);
  wind-up → stick at 2.0005·|stick−w|·(0.812+0.329·s18)/s (×2 when +0x1dc). On release: |w|<0.2 → 0;
  a = atan(|wf|/|ws|): a > 80° → spin 0, a < 10° → flip 0. Crouch unwinds at 13.33/s; the jump fires when it
  reaches 0 (takeoff c/13.33 s after release).
- Jump `Jump_ApplyImpulse` 0x1284e0: Δv = max(630.88, c²(0.0985+0.787J)·S(v)), S = 0.881v+24.68 below 993.5 cm/s
  else 899.87; direction normalize(N + 0.2F), N = ground normal rider+0x2a0, F = board forward rider+0x320 (0x1286e8; checked with the function runner, F4e); on a rail (rider+0x424 = 3) Δv goes along the board's up rider+0x1a0. Steep lips (50–70°) blend velocity toward the lip, up to 0.9.
  Proven exactly (G1a1, `tricky-rs/game/src/jump.rs`, bit for bit in the runner): c = max(0, rider+0x1c8 − rider+0x208);
  Δv = c·(c·J)·S with J = stat·0.78700274/255 + 0.09851468, then at least $f12. Steep lip when F.z ≥ 0, v.z ≥ 0 and
  t = (N.z − cos 50°)/(cos 70° − cos 50°) ≥ 0: h = normalize(N.x, N.y, 0), L = normalize(F − (F·h)h),
  b = clamp(t·L.z/sin 20°, 0, 0.9), v′ = ((1−b)v + b|v|L)/2 + Δv·normalize((1−b)N + bL); the flag goes to
  `Score_StartTrickChain`.
- Takeoff rates `Takeoff_SetSpinRates` 0x126e30: K = 11.517(0.5449+0.6742T); spin ω0 = K·ws (×2/3 alpine), Ported exactly and checked bit for bit (G1a2, `jump::takeoff_spin_rates`): K = (T·0.6741781/255 + 0.5449139)·11.5171995, spin = K·ws
  (×0.6666667 alpine), flip = (K·0.6666667)·wf (negated fakie); clips 0x218, 0x260, 0x261 take off with none.
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
- `Landing_CheckAngles` proven exactly (E8a1, `tricky-rs/game/src/landing.rs`): early out (returns 0, nothing written) for a
  grab/tweak/uber clip (types 0xe–0x10) without flag 4, or surface 0, 6, 10; else state 1, forward = normalize(B − (B·n)n)
  (B = rider+0x180) to rider+0x320, side = n × forward (w 0) to rider+0x330; pitch = π/2 − asin(clamp(B·t, ±0.9999999)),
  t = the velocity's direction with its part along the board's up removed; roll = −atan2(B·n, U·n); clean when
  ±roll (sign flipped past |pitch| > π/2) × (2.040804 − 1.0374061·stability/255) is within −2.967825..2.6840878.
- `Landing_ChooseState` 0x109308 proven (E8a2, `landing::choose_state`): motion 2; a grab clip (type 0xe) resets crouch/steer
  and lands clean (clip 0x1ff, state 0xc). Soft = pitch ≥ −45°, firm = vn ≥ −2777.1052 cm/s. Non-alpine: soft and firm
  → 0x1ff/0xc; else facing forwards (|yaw| ≤ 90°) 0x23e (soft) or 0x242, backwards 0x23f or 0x243, state 0xb. Alpine
  forwards in stance: firm → (soft 0x1ff/0xc, else 0x242/0xb), not firm 0x23e/0xb; riding switch 0x240/0xb. Alpine
  backwards: half turn (π about up, layers −π, forward and side negated, switch flips, rider+0x46e0 = wrapped heading),
  then 0x1ff (back in stance) or 0x240, state 0xb.
- Grabs follow clip markers at speed 0.782+0.806·s14.
- `SpinState_StartFromStick` 0x102158 proven (G1a3, `jump::spin_start_from_stick`, every reachable pad word): the pad word's two
  2-bit direction fields (bits 16–19, or 12–15 with bit 0x400) give x, y in {−1, 0, 1}; none held → no spin (returns 0).
  Within 10° of an axis it snaps; switch flips y. Rates spin+0x2c = K·x·5.2530804, spin+0x30 = K·y·5.2530804 (negated
  switch), K = 0.5449139 + 0.6741781·tricks/255; the clip by direction (0x226–0x22d, octants of atan2 riding stance);
  then state 1 and 0x10. From the pad only eight directions exist and the game's atan puts the diagonals just inside
  45°, so clips 0x22a and 0x226 and the snap are never reached.
- `SpinState_Update` 0x100908 proven (G1a5a, `spin::spin_update`, 40,000 random ticks in the function runner, every field and
  call bit for bit, half-pipe turn included): the landing alignment turns at most 80°/s of the time left
  (clamp(+0x34, ±1.3962634·t), × dt/max(dt, t)) by a quaternion about +0x40; spin/flip += rate·dt and are scored in whole
  degrees (spin mirrored by stats word +0x40). Stick (pad fields at bits 18/16, or 14/12 once started from the stick
  without 0x400): rates go to x·|lo+hi| and −y·|lo+hi| at 75 rad/s². Let go (or mode 2): the exit clip, mode 2, the band's
  inner bound and floor to 0, owed spin ±π for the half-pipe clips 0x23c/0x23d (which then turn the rider by their phase),
  emax 2π; the errors as before, but measured from the late wind-up (+0x274/+0x278 in thirds of a turn): spin back
  to 0 within 40°, to the half turn within 40° of 180°, else on to the next half turn on the band's side; flip back within 40° else on
  to the next whole turn; rate = (0.5449139 + 0.6741781·T/255)·150.53665·clamp(err, ±emax)·dt. Every tick the bands
  shrink by 0.9930117 + 0.002014339·T/255 and are held at the floor on their own side; spin clamped to [lo, hi], flip to
  [lo, hi] (upper strict). Mode 2 settled (|rates| < 0.7, not clip 0x225): `SpinState_StartFromStick`, else the half-pipe
  turn (not alpine, +0x1f0 = 0, facing back into the pipe, < 277.78 cm/s along +0x4ab0): rotate by
  q(spin)·q(flip)·z·q(flip)⁻¹·q(spin)⁻¹, keep the pivot bone's height in place, toggle switch, clip 0x225; else the
  late wind-up (stat +0x10: (0.54741764 + 0.6545031·s/255)·1.3750083·dt a tick, started by clip 0x222). Pad bit 0x100
  asks for a reset (101.46).
- `PrewindState_Update` 0x1050e8 proven (G1a4, `boarder::prewind_update` on the G1d/G1e helpers, 20,000 random ticks): from
  a standing start (rider+0x1dc 0, no jump bit 4, not started, rider+0x418 < 0) it waits for crouch release; else it
  primes both wind-up rates to 2.0005 and, with no wind-up, rides on (clip 0x1ff, state 3). Crouch held (bit 2): boost
  from bit 3, crouch target 1, wind-up targets from the stick fields (bits 5–10 spin, 11–16 flip, /31; rate 2.0005,
  4.0005 off snow), steer ±0.5 from bits 17–22 (or the spin field with bit 0x10) only while moving along the board.
  Release: crouch target 0 at 13.333/s, rider+0x1c8 := crouch, wind-ups under 0.2 dropped, snapped to pure spin or
  flip within 10° of an axis, the wind-up clip, state 10. Bit 1 or surface 0 asks for a reset (rider+0x304 = !bit 1).

## The pad word (`cPlayer::GetControls` 0x151250, board row G1a5b1, checked: `tests/controls.rs`)

- Every boarder state reads one 32-bit pad word, built each tick for the state it is in (rider+0x428) from the pad
  mapper (player+0x870): axes (`Pad_GetAxis` 0x1797f8) packed as `(int)(axis·31) & 0x3f`, directions as two bits
  (−1 below −0.5, 1 above 0.5, else 0; NaN −1), buttons as one bit from four queries (mapper vtable +0xc, +0x14,
  +0x1c, +0x24, by action id). Ported in `tricky-rs/game/src/controls.rs`.
- States 2–5, 7, 0xb, 0xc, 0x12 (riding): axis 4 → bits 5–10, axis 0 → 17–22, axis 1 → 11–16; +0x1c(0x1b) bit 1,
  +0xc(0x1b) bit 2, +0x1c(0x25) bit 0, +0x14(0x1d) bit 3, +0x24(0x1d) bit 4; bits 23–24 = 3 for action 0x26, 1 for
  0x27, else axis 5's direction; bits 25–26 axis 6's direction. These and 0xf also call
  `Boarder_UpdateSlopeTimeScale` 0x153130.
- State 0xd (prewind): +0x1c(0x25) bit 1; axis 4 → bits 8–13, axis 0xd → 14–19, axis 0xe → 20–25, axis 1 → 2–7;
  bits 26–27 the 0x26/0x27/axis 5 field, 28–29 axis 6; bit 0 if action 0x24 or any of 0..0x19 is down (+0xc).
- State 0x10 (spin): +0x1c(0x25) bit 8, +0x14(0x20) bit 9, +0x14(0x21) bit 10; directions of axes 2, 3, 8, 9, 10,
  11 in bits 12–23; +0xc(0x24) bit 11; bits 24–25 the 0x26/0x27/axis 5 field, 26–27 axis 6; the low byte 0x1a, or
  the last of actions 0..0x19 down. With bit 11 the low byte is the trick slot (player+0x868), filled from
  `AIComputer_RandomEasyTrick` 0x11d3d8 when empty (0x1a); bit 11 is then cleared.
- States 8, 9, 0x11: axis 0xf → bits 0–5. 10: +0xc(0x1b) bit 1, +0x1c(0x25) bit 0. 0xe: axis 0xc → 17–22, 0xd →
  5–10, 0xe → 11–16, +0x1c(0x25) bit 0, +0xc(0x1b) 1, +0xc(0x1c) 2, +0x14(0x1d) 3, +0x14(0x23) 4. 0xf: axis 4 →
  5–10, 1 → 11–16, 7 → 17–22, +0x1c(0x1b) 1, +0xc(0x1b) 2, +0x1c(0x25) 0, +0x14(0x22) 3, +0x14(0x1d) 4. 0x13:
  axis 10 → 1–6, 11 → 7–12, +0x1c(0x25) 0. 0x14: +0x1c(0x25) 0. 0x15: +0x24(0x28) 0. Others: 0.
- With an overlay up and no race running the pad is not read (word 0, or 0x1a in state 0x10). What the action ids
  and axes are on the pad is row G1a5b2.

## The pad mapper (`cInput`, board row G1a5b2, checked: `tests/pad.rs`, `tests/rider_score.rs`)

- The pad (`cPS2Device` vf10 0x17b278): libpad's button word mapped to the game's bits by the table at 0x337c08
  (d-pad 1/2/4/8 up/down/right/left, square 0x10, triangle 0x20, circle 0x40, start 0x80, select 0x100, L1 0x200,
  R1 0x400, X 0x800, L2 0x1000, R2 0x2000, L3 0x4000, R3 0x8000); the d-pad as an axis pair (left, up −1); sticks
  (byte − 127.5)/127.5 (right, down +), past ±0.5 also as bits (0x20000 up … 0x1000000 right stick left).
- `DATA/CONFIG/BTNMAP<n>.DAT` (n = stats+0x6c; `Pad_ParseButtonMap` 0x179b18, bound by `Pad_SetupVibration`
  0x152770): `IS_BOOL` buttons are actions 0x1b–0x28 in file order (Antic X, AnticAbort L3 / none, Boost square,
  CameraToggle, CameraReverse none / triangle, Tweak square, LateSpinMode, RailSpinMode, PrewindTurnMode,
  RandomGrab (no shoulder), Reset select, ShoveLeft, ShoveRight, LessonWait X); `IS_BOOLGRAB` combos (easyA L2,
  easyB R1, easyC R2, easyD L1, the pairs, the triples, all four) go to the rider's trick slots (column 0 of the
  trick table at 0x333098, per rider stats+0x5c and board stats+0x44: slot, grab clip, tweak clip, uber clip
  (0x300 none); `tricky_data::trick_slots`); `IS_ANALOG` are axes 0–15 (Brake, Crouch, Spin, Flip, Turn,
  ShoveSide, ShoveFwd, RailSpin, LateSpin, LateFlip, AirTurn, AirTilt, PrewindTurn, PrewindSpin, PrewindFlip,
  GateRock). Map 0 spins and flips on the d-pad only; map 1 also on the left stick, with no prewind turn.
- `Input_BindButton` 0x179818 keeps a button's "pressed" set and "pressed ∪ others" (the buttons looked at);
  queries: `Input_ComboHeld` (looked-at buttons down = exactly the pressed set), `Input_AnyHeld` (any of them;
  NONE never), `Input_ComboPressed` / `Input_AnyPressed` (the same counting only buttons that went down this tick).
- `Pad_ReadAxis` 0x179940: the d-pad if the mask has it and it is pressed, else the left stick, else the right,
  negated; a POS / NEG mask keeps one side; dead zone 0.38046828; then the map's negate. Stick up is Crouch, down
  is Brake.
- Our `trickdata` rows are the table's board-0 rows for every rider; boards 1 and 2 change some of the later rows.
