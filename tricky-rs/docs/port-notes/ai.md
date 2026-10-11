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

- Table layout (proven, G4a1): `RelTable_Init` fills 12 × 12 entries of 16 bytes (row = who feels, column = about
  whom, in `rivals::NAMES` order): +0 tolerance, +4 attitude, +8 a third value (mostly 100; not ported, G4a2),
  +0xc relation (0 neutral, 1 friend, 2 foe). `tricky-rs/game/src/rivals.rs` matches all 432 ported values
  (`tests/rel_table.rs`).

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

## Grudges, exactly (board row G4a2, checked: `tests/rivals.rs`, `tests/rel_table.rs`; `rivals::RivalRider`)

- The relations table (`RelTable_Init` 0x167208, 12 × 12 × 16 bytes): +0 tolerance, +4 attitude, +8 the attitude's
  cap (mostly 100; 79–80 toward a friend, so never a level-2 grudge; Kaori and Brodi 59 toward most riders, so no
  grudge at all), +0xc relation. `Rivals_InitPairs` 0x167ea0 copies them per pair of the race (attitude: the table
  for a computer rider, the career save for a player; cap, tolerance, relation) into the pair records.
- In the pair record (rider+0x40 + 0x20 × the other's index): +0x10 attitude, +0x11 cap, +0x12 tolerance, +0x13
  anger, +0x14 the attitude at the start, +0x15 relation, +0x16 / +0x18 / +0x1a bumps / shoves / knock-downs taken
  from that rider, +0x1c rivalry. The grudge: rider+0x118 on, +0x11c since (tick), +0x120 against whom, +0x124
  level, +0x128 cooled. A rider is a player if its pad port (stats+0x65) is ≥ 0.
- `Rivals_OnBump` 0x11ea70 / `Rivals_OnHit` 0x11eaf8 (shoved off balance) / `Rivals_OnKnockdown` 0x11ebf8: the
  count goes up; a player hitting a computer rider adds 9 / 15 / 30 to its anger, held to the tolerance, and at the
  tolerance it starts a grudge; a computer rider shoving or knocking down a player eases its grudge against that
  player (`Rivals_EaseGrudge` 1 / 2) and adds 1 / 2 to its rivalry.
- `Rivals_StartGrudge` 0x11ecf8: starts or renews the grudge (moving it from another rider eases that one first);
  level by the attitude: ≥ 80.98926 → 2 (`Speech_OnGrudge`, rivalry +3), ≥ 60 → 1 (+2), else 0 (+1); a friend
  always 0; attitude +15 (to its cap); anger 0.
- `Rivals_EaseGrudge` 0x11ef00: anger −45 (1), −15 and cooled (0), −max(45, half) (2); never below 0.
  `Rivals_UpdateGrudgeTimeout` 0x11efd8: a level-1 grudge not cooled cools 360.44952 ticks (6 s) after it began.
- `Rivals_UpdateAttitude` 0x11f050 after the race: back to the start's attitude when there was no contact, else
  start + (bumps + 2 shoves + 3 knock-downs) / (3 (bumps + shoves + knock-downs)) × the gain.
- The app's computer riders keep their grudge against the player with these (row G4a3, `rivals::Grudge` over
  `RivalRider`): the pair set up as `Rivals_InitPairs` does, carried between heats by `Rivals_DecayAttitudes`
  0x168028 (−3 friend, −2 neutral, −1 foe; at least 0 and the table's), time in ticks. Chasing is
  `AIComputer_IsRivalTarget` 0x139928 exactly (grudge on, against that rider, level 2 or level 1 not cooled; every
  rider in knockdown practice). STANDIN: shoving still reads level ≥ 1; the game's choice of target and when to
  shove is in `AIComputer_PickTargetRider` 0x1396c8 (every 12 ticks under 540 of rider+0x348, riders within 700 cm by
  the pair records, weighted distance × (1 − cos bearing) or × (1 + |cos bearing|) for a rival target; dropped past
  1050 cm) and `AIComputer_RideNearRider` 0x137dc0 / `AIComputer_RideAtRival` 0x137a18 (row G4).

## The AI's target and steering helpers, exactly (board row G4b, checked: `tests/aicomp.rs`; `tricky-rs/game/src/aicomp.rs`)

- The computer object (cComputer, player+… for AI): +0 its rider, +0x86c the skill scale, +0x890 the target rider
  (−1 none). A race has at most six riders: six pair records end at rider+0x11f, just before the grudge (+0x118).
- `AIComputer_SteerTowards` 0x1372f8: the target's direction from the rider (+0x140), flattened onto the rider's
  plane (up +0x4ac0), angle −atan2(d·fwd (+0x4aa0), d·side (+0x4ab0)); steer = |angle| × 6.2897024 (× skill /
  1.0241041 when skill ≠ 1), 0 under 0.2, at most 0.9705659, the angle's sign.
- `AIComputer_SetThrottle` 0x139f60 / `AIComputer_SetTuck` 0x13a028: the pad word's crouch stick (bits 11–16) at
  1, or skill / 2.211699 when skill ≠ 1 and the rider is at 833.3333 cm/s or more; the tuck also sets bit 3.
- `AIComputer_IsRivalTarget` 0x139928 and `AIComputer_PickTargetRider` 0x1396c8 as in the grudges section above
  (the pick every 12th tick, riders within 700 cm, weight distance × (1 − cos bearing), a rival × (1 + |cos|), the
  lightest with ties to the later rider; dropped past 1050 cm or over 600 of rider+0x348; under 540 to pick).
- `AIComputer_ControlsDispatch` 0x1389c8 every tick: `AIComputer_CheckStuck` into bit 0, then the shove:
  `AIComputer_FindNearbyRider` 0x1394e0 — not during a shove clip (layer 1 type 0x15), every 6th tick, the first rider
  within 200 cm (pair record) whose bearing is 30°–150° off the heading (+0x1b0) and that the rider holds a grudge
  against at level 1 or 2 (cooled or not); the pad word's shove field (bits 23–24) 3 (ShoveLeft) when the bearing
  difference is above 0, else 1 (ShoveRight). Then the riding mode by cComputer+0x8a0..0x8a4 (+0x89c the mode: 0
  to a jump `AIComputer_RideToJump` 0x137858, 1 the line `AIComputer_RideLine` 0x1375c8, 2 near a rider
  `AIComputer_RideNearRider` 0x137dc0, 3 at a rival `AIComputer_RideAtRival` 0x137a18; method table 0x31d318).
  `AIComputer_RideAtRival` steers at a point 300 cm ahead of the rival along its velocity, brakes (bits 17–22 full)
  when faster than the rival, tucks within 15°, throttles 15°–50°; behind 90° it steers to the line and brakes past
  200 cm when faster.
- The app's computer riders shove through `aicomp::find_nearby_rider` (row G4c; STANDIN: the pair record and the
  heading from our positions and yaw, the shove clip as our shove timer).

## The riding modes, exactly (board row G4d, checked: `tests/aicomp.rs`; `aicomp::Ai::ride`)

- `AIComputer_RideLine` 0x1375c8: steer at the line point (+0x360). With a speed hint (`AIComputer_FindSpeedHint`
  0x1393e0, which may set cComputer+0x868, the speed to ride at): over speed + 138.89 cm/s brake if the hint says
  so; under (speed − 138.89) (× skill / 1.4103198) crouch (hint flag 0) or tuck. Without: boost (bit 3) with a
  full meter (× skill / 1.4103198) and crouch. Then a jump marker ahead (`AIComputer_FindJumpMarker` 0x1392a8,
  which may set the speed) → to the jump, target −1; else a target rider (`AIComputer_PickTargetRider`) → near it,
  or at it if a rival.
- `AIComputer_RideToJump` 0x137858: no marker → the line (and rides it this tick); else steer at the line; a
  marker wanting a trick (its first word), rider+0x348 under 153.10213 and the steer under 0.53616524 →
  `AIComputer_PlanJumpTrick`(marker words 2, 3) and jump (bit 1); else tuck under speed − 138.89, brake over
  speed + 138.89.
- `AIComputer_RideNearRider` 0x137dc0: the target over 90° off the travel heading (+0x370): steer at the line, tuck
  if slower than it (else crouch). Within the cone a 150 cm body takes at its distance (atan 150 / d) and faster
  than it: steer at the line point turned (`Math_SinCos`) by the angle out of the cone, and tuck. Else the line and
  crouch. Then jump marker → to the jump; no target → the line; a rival → at it.
- `AIComputer_RideAtRival` 0x137a18 as above (aim 300 cm ahead of the rival along its velocity); then jump marker →
  to the jump; no target → the line; not a rival → near it.
- In line mode the game reads an unset stack word for "rival" if a target were kept; it never is (line mode has
  no target).
