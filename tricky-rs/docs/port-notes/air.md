# Jumps, air, tricks, landing: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Jump, air, tricks, landing

- Prewind `PrewindState_Update` 0x1050e8: crouch → 1 at 5.0025·max(|1−c|,0.1)/s (full ≈0.66 s, exponential);
  wind-up → stick at 2.0005·|stick−w|·(0.812+0.329·s18)/s (×2 when +0x1dc). On release: |w|<0.2 → 0;
  a = atan(|wf|/|ws|): a > 80° → spin 0, a < 10° → flip 0. Crouch unwinds at 13.33/s; the jump fires when it
  reaches 0 (takeoff c/13.33 s after release).
- Jump `Jump_ApplyImpulse` 0x1284e0: Δv = max(630.88, c²(0.0985+0.787J)·S(v)), S = 0.881v+24.68 below 993.5 cm/s
  else 899.87; direction normalize(N + 0.2F), N = ground normal rider+0x2a0, F = board forward rider+0x320 (0x1286e8; checked with the function runner, F4e); on a rail (rider+0x424 = 3) Δv goes along the board's up rider+0x1a0. Steep lips (50–70°) blend velocity toward the lip, up to 0.9.
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
