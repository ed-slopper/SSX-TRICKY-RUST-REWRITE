# AI and rivals: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

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

## Rivalry (pair record boarder+0x40+other·0x20)

- Fields: distance, bearing, attitude, cap, tolerance, grudge, base attitude, relation, bump/shove/knockdown
  counts, rivalry score (+0x1c). Score: vendetta start +1/+2/+3 (level 0/1/2), an AI shoving the player +1,
  knocking the player down +2. The post-race rival scene plays when an AI's score toward the player is > 2.
- After a race attitude settles: att = base + (bumps + 2·shoves + 3·knockdowns)/(3n)·(att − base) (back to base
  with no contact); only AI→player attitude carries over the circuit (and into the save); between heats it
  decays (friend −3, neutral −2, rival −1).
- Taunts (`Ride_AITauntCheck` 0x103660): riding ≥ 1.025 s, rand%100 < 91, the vendetta target (any level) within
  10 m and more than 60° off the heading; behind (> 135°) AITAUNT, else the side's TAUNTHS/TS.
