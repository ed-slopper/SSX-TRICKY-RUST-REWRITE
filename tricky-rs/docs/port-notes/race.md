# Race, course and finish: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

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

## Tokyo Megaplex laps

- 4 laps (vf8 0x116fac, track 8): lap line (file 12) takes one off, finish (file 9) only on the last.
- The glass tube on race line 1 (instances Mdl_Endboost_Lap/_Z/_End, effects type 0 subtypes 15/18/24):
  cLapBoostNode (0x140b90) only with laps left: within 10 m of the tube centre, nudge in, v.xy ×0.92/tick,
  v.z → 2500 cm/s at rate 5/s; cZBoostNode (0x141cd8) sets z = 0 (top of the course) when inside, velocity kept;
  cTubeEndBoostNode (0x1418a8) pushes toward d_k at S_k (rate 2/s): k = 0 rode in low (d (0.159, 0.954, 0.254),
  2700), k = 1/2 from the −x/+x side ((0.226, 0.904, 0.362) / (0.092, 0.925, 0.370), 3500).
- Distance to finish adds n·L (L = race line 0's DTF); progress best resets when dtf jumps up by > L/2.

## Pre-race scenes, surfaces, meter (more)

- Stage scripts (`PreRaceSel_*` 0x168720–0x169378): a race picks STG_COM_1–4 and GAT_6COM_1–4 at 25% each (circuit
  heats 2–3 skip staging and the fly-through; friend/foe gates when such a rider is in the field; show-off GAT_SO).
- Objects (proven, F1c): the picks at 0x32ecf0 (`PreRaceSel_*`, `PreRaceSel_Destruct` 0x168688) and the scene at
  0x32edc0 (`RaceScene_*`: `RaceScene_Construct` 0x169640 / `RaceScene_Reset` 0x169688, +0 = 0x10, +0x43c = 0;
  `RaceScene_SetupFromPicks` 0x169698 copies 15 × 6 rows from the picks (+0x33c, 100 bytes a slot), one `Rand_Fx` draw
  per row with a variant, and lists the distinct row values (+0x348, 0x1bf = none) at +0x458, at most 0x5a;
  `RaceScene_GetAnimList` 0x169948 returns that list and its count, which `cAI::vf14_111108` passes to `AnimData_OpenBig`).
  `PreRaceSel_BuildSceneIndex` 0x168360 (no caller in the exe; static init) builds the 10-byte records at 0x3cc028 that
  `PreRaceSel_PickSceneVariants` 0x169378 draws from, out of 0x385 shorts at 0x37ab50.
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

### Proven (G1c2; exact in `tricky-rs/game/src/gate.rs`, checked bit for bit in `tests/gate.rs`)

- k = stat·0.71808594·0.003921569 + 0.4875571 with stat the start byte stats+0x15 (0..255), in that order.
- `GateAnticipateControl_Update` 0x105f58: the stick is the pad word's low 6 bits, sign-extended, /31. Target m0
  (`FUN_0015fde0` on layer 1), m0·0.5 or 0; rider+0x240 target, +0x23c rate k·4.437673·max(|target − p|, 0.2146618),
  p (+0x238) steps by rate·0.016666668 and snaps to the target when within a step. If p did not move, still
  (state+8) += 0.016666668 s; else still = 0 and hi (+0x10) or lo (+0xc) = p. rider+0x1c0 = `Boarder_GetGateStartTimer`.
- `GateAnticipateControl_Exit` 0x106140: rider+0x1c8 = 0x440ae38e (555.5555 cm/s); when target < 0.5 and still <
  0.006666667 (so: p moved on the last tick), v = (hi − lo)·2024.8037·(1 − p)/(still + 1) (still in seconds, not
  ×60 as above), kept when ≥ 555.55554.
- `GateLaunchControl_Enter` 0x106228: target 1, rate k·4.437673·0.2146618. `_Update` 0x1062b0: p steps as above; if
  the VU length of the velocity is exactly 0 and m1 (`FUN_0015fde0` layer 2) ≤ p: velocity = board forward
  (rider+0x180) × (k·launch), `Boarder_SetMotion`(2). At p == 1: `Boarder_SetMotion`(2) unless already 2,
  `AnimCtl_PlayClip` 0x1fd, `Boarder_SetState`(7).

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

## The race scenes' set-up (board row G11b; `tricky_game::scene`, checked: `tests/scene.rs`)

- The scene object (`RaceScene`, 0x5c0 bytes: +0 the scene chosen, +4 fifteen scenes of six slots (a, b, the rider
  picked), +0x43c the selection, +0x440 the cast, +0x458 the riders list) is filled from the pre-race selection
  (0x32ecf0 from +0x33c: per scene the script number, then six slots of a, b, rider id, candidate).
  `RaceScene_SetupFromPicks` 0x169698 copies every slot; one with a candidate draws `Rand_Fx` and takes candidate
  `r % 1` (the count of candidates never grows past one, so it is always the first; the draw still happens), else −1.
  Then the riders of all scenes once each, in scene and slot order, `NO_RIDER` (0x1bf) skipped and ending the list
  under 90. `RaceScene_Reset` sets the scene to 0x10 and drops the selection.
- `RaceScene_Select` 0x1699a0(scene): for each picked slot `Race_GetRider`(0) and `AnimCtl_FindFirstMarkerFrame`
  (its +0x46c0, the slot's b as the clip); the slots with a marker become the cast ordered by that frame (ties keep the
  earlier slot), −1 after them. `RaceScene_GetCast` / `FUN_00169948` count the cast (to −1, at most 6) and the riders
  (to 0x1bf, 90 when none). `RaceScene_GetScriptName`: `SCRIPTS` (0x32f380, 58 names, RaceLoaded .. Null) by the
  chosen scene's first word.
- Checked in the runner on 20,000 random selections and objects: the whole object, the draws and the calls compared.
  The scene index 0x3cc028 the selection reads (`PreRaceSel_BuildSceneIndex`) is E1b2o's; the cast's clips
  (`RaceScene_PlayCastClip`, `_StartCastClip`, `_UpdateCastClip`) are animation (G5).

## The pre-race sequence (board row G11c; `tricky_game::prerace`, checked: `tests/prerace.rs`)

- `cPreRaceHandler::Enter` 0x112b88: overlays hidden, one view; a race entered again (race +0xdc) starts at 0x12
  (the countdown's end), a replayed one (+0xe0) at 0, a first one at 2, counting the course as started for each
  profile's character (`Career_CountCourseStart`); the camera script "PreLoad"; "NoCountdown" run without a start
  gate (`Race_HasStartGate`: not Untracked, modes 0, 2, 3, 4, 5, 7, 9); the race's virtual +0x6c.
- `Update` 0x112d10 (state +4; each state falls into the next in the same frame when it can): 0 PreLoad, 1 wait
  for loading (race +0x4f8); three scenes (2/6/10): a scene whose script word (career +0x2b18, 100 apart) is 0x39
  "Null" is skipped, else `RaceScene_Select`(k) and `RaceScene_Prepare` (its cast's samples; it always answers 0, so
  the wait-for-loading states 3, 7 and 0xb are never reached) then `RaceScene_Play` (the scene's script on the camera,
  `FE3DTransition_StartIn`), held while the camera runs unless skipped (`RaceScene_CheckSkip`: the first player pad
  pressed this frame, new X / triangle; a skip in the second scene skips the third too). Before the third the riders'
  virtual +0x4c. 0xe: `FE3DTransition_StartOut`, the riders placed at the start; then (when loaded) the countdown's
  screens: HUD item 0x12 when a player is over level 7 (0x10), else item 3 (0x11; none in mode 1), each left by a
  skip; 0x12 the intro stream stopped; 0x14 the camera and speech stopped, the riders placed again, the race's
  virtual +0x9c, 0x15 done. `Exit` restores the views.
- Checked in the runner: 4,000 random careers and races through Enter, up to 14 frames of Update and Exit, the
  engine's calls scripted the same: the state, the career block, the scene object, the pads and every call compared.

## The post-race sequence (board row G11d; `tricky_game::prerace::EndRace`, checked: `tests/prerace.rs`)

- `cEndRaceHandler::Enter` 0x113e10: `Race_ClearAiTargets`, `Game_MoveLeaderToSlot0`, one view,
  `Career_CheckNewRecords`. The first scene is a rival's: over the other riders (in race order, the player's rider
  at career +0x30f4 skipped) the highest count from 3 up (rider +0x5c + 0x20 · the other's index) — another rider's
  about the player's rider gives scene 10 + its place among the others (a jump table, 0x364660), the player's rider's
  about another 5 + it; none 0x10. The second is the win (3) or loss (4) scene: in a show-off (3, 5) a medal for the
  score (rider +0x5910), in freeride (7) first place (+0x110) in under the course's time (career +0x2b08, compared
  as floats), otherwise a place in the top half ((riders + 1) / 2). A circuit race keeps the best time (+0x418), a
  circuit show-off the best score, under slot 0's player's character. HUD items 0 and 7 hidden, the PreLoad camera,
  every rider frozen.
- `cEndRaceHandler::Update` 0x1141e0: the race clock (+0x14) on by 1/60; the two scenes as in the pre-race sequence
  (states 0..3 and 4..7; a skip in the first skips both); 8, once the race control's +0x54 is clear and the results
  overlay is not busy: the fade out, the speech stopped, HUD item 2 for a new record (career +0x3910), else 0xd when
  the circuit unlocked anything but a medal, else 1; the PreLoad camera; 9 waits for loading; 10 starts the replay's
  rider sounds each frame while +0x54 is clear; 0xc (set by the results menu) shows item 9 and opens the menu;
  0xb restarts the race. `Exit` does nothing.
- Checked in the runner with the pre-race test: 4,000 random careers and races through Enter and up to 14 frames,
  the menu's 0xb / 0xc thrown in: the states, the clock, the career block, the scene object and every call compared.

## The countdown, looking back and the race menu (board row G4k1; `tricky_game::prerace`, checked: `tests/raceglue.rs`)

- `cCountdownHandler::Enter` 0x113520: with a start gate, the world script "StartCountdown", the race's virtual
  +0x6c and, outside the tutorial, the race clock (+0x14) at -300 with HUD item 4; every rider (+0x88 of them, from
  +0xc4) to its start state (`Boarder_SetStartState`, the gate flag).
- `cCountdownHandler::Update` 0x113600: the clock up by 2 a frame. At 0 or over: the race's state 4, with a gate
  "EndCountdown" and +0x6c, then `Audio_OnRaceGo` unless a replay holds the race (the replay control at gApp+0x730
  -> +0x40, its +0x20 non-zero and under 10). Before 0, unless a replay holds: a beep each time the clock, truncated,
  is a multiple of 60.
- `Race_UpdateLookBack` 0x113a88: in the race (the game's state 4) with no replay holding, each player (+0x7c) whose
  latch (+0xe4) is set and still holds the look combo (0x1f) looks back; otherwise its latch becomes whether the combo
  was just pressed and it looks ahead.
- `cAI::vf12` 0x112550 runs the start script by mode: "RaceMode" (0, 2, 4, 7, 9), "ShowoffMode" (3, 5), else
  "FreerideMode". `vf18` 0x111708 sets the race's state 2. `vf19` 0x111738: with a replay, `Replay_Freeze` first; a
  replay whose +0x4c is set then gives state 0xb, else the race's +0x94 (the argument) and state 5.
- The race menu on the post-race handler (+4 the state): `RaceMenu_GoToState11` 0x113bf0 / `_GoToState12` 0x113ce8
  abort the replay, set 0xb / 0xc and start the PreLoad camera; `RaceMenu_Restart` 0x113c60 aborts the replay, clears
  every player's flags, prepares the race again, resets the cameras and starts PreLoad; `RaceMenu_EndRace` 0x113d58
  sets 0xe, aborts, PreLoad, `Career_FinishRace`, then resets the cameras for another heat (career +0x3260 is 4) or
  sets the game's +0x54 (the results shown).
  The race menu's own four (0x113bb0 / bc0 / bd0 / be0) are tail calls to EndRace / GoToState11 / Restart /
  GoToState12 with its +0x38, the handler.
- Checked in the runner: 40,000 random cases over the nine functions (careers, modes, clocks, replays, pads,
  latches, tutorial): every call, the clock, the latches, the handler's state and the results flag compared.

## Rank catch-up: the unchecked list walk

Proven on the PS2: `Race_UpdatePlacingsAndRubberBand` 0x115100 runs every 6 ticks (`race+0x18 % 6`), over
`race+0x88` riders, with `race+0x7c` players (rider+0x418 is the finish time, race time × 100 set by `Boarder_Finish` 0x11da40, −1 until the line; sorted by `0x7fffffff - x`).

Lead from another version (Xbox `default.xbe`, not `SLUS_203.26`; from the OpenTricky project's notes,
`port/tools/fork/fix_riderlist.py`, Xbox addresses there): the Xbox function sorts the riders, splits them into a
player list (4 slots) and an AI list (6 slots) on its stack, then walks the AI list up to the AI count fixed when
the race was built and gives each AI a target from the player list, with no check of how many entries either
list really got. When the counts disagree, it reads stack leftovers as a rider index (a crash was seen once on
the Xbox).

**Checked on the PS2 (G8m, `tricky_game::placing`, tests/placing.rs):** 0x115100 walks its lists the same unchecked
way. The riders split by +0x41c into two lists in its stack frame (8 slots at +0xc0 for +0x41c = 0, 4 at +0xe0 for the
rest). The first walks to the race's +0x80, and the second's index steps every n / players, neither checked against
how many each list got; past them it reads the frame's leftovers (stale indices). The port keeps the lists in the
game's frame and reads them the same way, the bug included. It compares floats as the EE does (a stale index read as
a float is a denormal, so zero). The test keeps the second walk inside its four slots, because past them the game reads
its saved registers.

## The finish state (proven, G1c1; exact in `tricky-rs/game/src/finish.rs`)

- `FinishState_Update` 0x106668 counts ticks (state+0xc; "early" while under 301). Phase 0: brake/crouch input (0, 1)
  until slow (|v| < 277.78 cm/s), fully braked, or 301 ticks; steer target 0; phase 1. Phase 1: crouch, steer and brake
  targets 0, velocity ×0.93 a tick until brake and steer are 0 (or 301 ticks); clip 0x2ee, stop, phase 2. Phase 2:
  when the clip ends (or 301 ticks): celebrate (`Boarder_StartCelebrate` 0x11f3b8: rider+0x2c = race tick, +0x34 = 0)
  or be disappointed (`Boarder_StartDisappointed` 0x11f3f8: rider+0x30 = race tick): circuit modes 3/5 by medal (3 =
  none: disappointed); mode 7 first place under the event's target (0x32de98); mode 6 by gApp+0x730 → +0x34 → +100;
  else the front half of the field (place < (riders + 1)/2). Clip 0x2ef, the finish pose, motion 4, phase 3; phase 4
  once layer 1 plays a type-8 clip.
- `Boarder_SetBrakeTarget` 0x126c78, `Boarder_SetBrakeCrouchInput` 0x126b38: see `finish.rs` (its first argument goes
  to the crouch target, the second to the brake target, as the machine code passes them).

## Courses and modes (row C0; `tricky_game::courses`, checked: `tests/courses.rs`)

- The course table 0x332f28 (7 words a course): number, name, menu code, show-off time (hundredths), file name ×2,
  a flag. 0 Garibaldi (gari, 120 s), 1 Snowdream (snow, 90), 2 Elysium Alps (elysium, 90), 3 Mesablanca (mesa, 90),
  4 Merqury City (merquer, 90), 5 Aloha Ice Jam (aloha, 90), 6 Pipedream (pipe, 90), 7 Untracked (untrack, none),
  8 Tokyo Megaplex (megaple, 90), 9 Big Air Dome (bigair, 90: not on the disc as a course), 10 Trick Tutorial
  (trick, none), 11 Alaska (alaska, 135), 12 the front end (ssxfe). `Course_OpenModelsBig` 0x186268 builds the
  runtime list (0x84-byte records: `data/models/<file>.big`, the file name at +0x44 = `Course_GetInfo` 0x186350).
- 0x32de88 is the course being played (its number); 0x32de8c the game mode, set by the event menu
  (`cFE3DStateEventSel::CreateNextState` 0x29ad48) with the game type (gApp+0x734 → +0xf0: 0 practice, 1 single
  event, 2 World Circuit): 0 knock-down practice, 1 head to head, 2 race, 3 show-off, 4 World Circuit race,
  5 World Circuit show-off, 6 trick tutorial, 7 freeride (the names of 0 and 1 from what reads them).
- `cRaceHandler::Enter` 0x113740: the race clock's limit = the course's show-off time / 100 in modes 3 and 5.
- World Circuit order (0x3352f0): Garibaldi, Snowdream, Elysium, Mesablanca, Merqury, Megaplex, Aloha, Alaska.
  Races (mode 4): heats, the top three go on (`Circuit_ScoreEventResult` 0x16d0b0: places 1–3 weigh 0.833, 0.334,
  0.167). Medals (`Circuit_GetMedal` 0x16baa8; thresholds 0x335370, gold / silver / bronze): gari 55k/40k/25k, snow
  95k/65k/35k, elysium and mesa 225k/150k/75k, merquer 275k/175k/125k, megaple 350k/225k/100k, aloha
  175k/115k/75k, alaska 500k/300k/150k (also any course not listed), pipe 800k/500k/250k (its own branch).

- Which courses a mode offers (row C0b; `courses::VENUES`, `course_unlocked`). The venue menu
  (`cFE3DStateVenueSel`) lists 0x3a8af0: the circuit's eight, then Untracked and Pipedream; `CreateNextState` sets
  0x32de88 from it, `FEVenueSel_UpdateLocks` 0x29baa0 greys each by `Career_IsCourseUnlocked` 0x163bb8 (career
  0x32b390, mode 0x32de8c). That function: with the unlock-everything cheat (0x3352a0) the mode's mask alone (a
  stack table: knock-down / head to head everything but the tutorial; race no Pipedream, Untracked, tutorial;
  show-off no Untracked, tutorial); in modes 4–6 the tutorial always, nothing without a circuit career (+0x30fc < 0),
  else the career's circuit table (+0x3114, per course +0x26 race, +0x27 show-off); else the career's bit (+8) and
  the mask. The venue help line asks Pipedream and Untracked with mode 1 (open: strings 0xde7 / 0xde8, else the
  career's hint `FUN_0016b860`); the others print the career's line with " race" (0x31), show-off (0x32) or 0xd31.
- Per-mode race setup (read, port with C12): `Circuit_SetupRace` 0x16d230 (show-off): one rider (the player,
  `Career_SetupPlayerRider`), race+0x14 = 2, +0x330 = 2 + the course's medal-table index << 16.
  `Circuit_SetupEventRiders` 0x16d300 (circuit race): six riders; +0x330 = heat + circuit index << 16; on heat 0 it
  marks, for every rider, who the pre-race picks (`PreRaceSel_Choose` 0x32ecf0) put in each of the three heats
  (0x335658 + rider·16); the player goes in slot 1 + rand&3 (Alaska: slot 1 when that is 1 or 2), the two picked
  rivals either side, the other picks into empty slots, then the rest by a random draw among riders still out
  (rand % 10000, highest first); then `Circuit_SetupRider` ×6, `Rivals_InitPairsCircuit`, `Career_FinalizeRoster`.

## The career block and a new game (board row E12a; `tricky_game::career`, checked: `tests/career.rs`)

- The career (0x32b390, what the memory card saves) is kept as its bytes (0x3900): our save file is the block.
  Known so far: +4, +8 the course bits (0x407 at the start: Garibaldi, Snowdream, Elysium... as `courses.rs`),
  +0xc the characters unlocked (bits of the starters at 0x3352c0..0x3352cc: Eddie, Mac, Moby, Elise), +0x10 a short
  record per character (0x14: the rider, its entry's words +0x28 and +0x30), +0x100 a record per character (0x1c4:
  per course bytes at +0x24, the boards unlocked +0x58, the group averages +0xc and levels +0x10, each other
  rider's attitude to it at +0x164 + 8j), +0x1630 four player names (10 bytes), +0x1658..+0x1674 the sound
  options, +0x1670..+0x16c0 the other options, +0x16c4 the high scores (13 courses × 0x150: three tables of five
  0x14-byte entries: name, character, score), +0x30fc four player slots (0x44; +0 the slot's character, −1 none),
  +0x3258 changed, +0x325c, +0x3260, +0x36fc a buffer (`Career_AllocSave`), +0x3820.. the slots' attributes.
- `Career_InitNewGame` 0x1617a0: the relations table made once (`RelTable_Init` on 0x900 bytes at 0x3352b4),
  `Profile_ResetNewGame`, `Career_UpdateAttributes` (nothing with no slots), +0x325c 0, +0x3260 1.
  `Profile_ResetNewGame` 0x161b40: +8..+0x27dc cleared; the options (2, 1, 1, 1, 1, 1, 1, 1, 0.5, 6, 5); the player
  names (`Career_DefaultPlayerName`: PLAYER%d in languages 0, 1, 4, SPIELER%d in 2, JOUEUR%d in 3); no slots;
  each character's records (`Career_InitCharacterRecord`: boards needing 0 points open, levels avg × 0.4 + 0.5);
  the high scores emptied (`HighScore_Reset`: each entry's character `Rand_Fx` % 12, unsigned: 195 draws of the
  effects stream) then the 165 default entries (SOZ 22500, PONCH 24000 ...); the unlocks; per character 13
  course words; the sound options synced from and back to the sound manager; +0x2b14, +0x3714, +4 cleared;
  `Profile_NumberCharacterRecords` 0x166c70 numbers the 13 character records (+0x114 = i, +0x118 = +0x11c = 0,
  0x1c4 apart).
- Checked in the runner on 24 blocks of random bytes (each language, with and without a sound manager, the
  relations table made or not): every byte of the block, the effects stream and the sound calls.

## The career's players, records and the memory card (board row E12b; checked: `tests/career_ops.rs`, `tests/career.rs`)

- Pointers in the block are game addresses (the block at 0x32b390): a slot's +0x3108 its name, +0x3110 its
  profile (+0x3118, five words: character, the entry's +0x28 and +0x30, outfit, board), +0x3114 its character
  record; `RiderSetup_Init` writes the constants 0x32b398, 0x32db6c and 0x32e5f4. Ours keeps the same numbers.
- `Career_AssignPlayerRider` (slot −1: the first free, past the fourth when all are taken, which writes over the
  rider set-up at +0x320c: the flags are cleared in the call's delay slot, before `RiderSetup_Init`): the profile
  it raced with if the same character (+0x312c), else the character's short record; `Career_UpdateAttributes`.
- Attributes (+0x3820.. per slot): the record's points and their level (×0.4 + 0.5) to spend, the group levels
  now and at the start, the caps (the rider's × 0.4 + 0.5); the max-attributes cheat takes every group to its cap
  and leaves no points. Spending moves a group up to its cap, refunding back to its start; each rescales the
  record's points (left / all × base) and its group byte (now / cap × the rider's cap).
- `Career_CheckNewRecords`: show-off players riding a computer lose the race's flags; the circuit's result; the
  slots' race flags cleared; the race's places into the course's tables (times in race modes 2 and 4, scores in
  show-off 3 and 5, mode 7 the winner's time into the third table), each new record into the list after the block
  (0x32eca0). A high-score entry is the name (the game's 12-byte buffer keeps its stack past the terminator) and
  the character. `Career_UpdateBestStats` keeps a character's bests with a tag.
- The memory card (`Career_SaveToCard` 0x1d2078): +8..+0x27dc, the sound options synced, +0x27d8 the version
  0x12300003, +0x27d4 the CRC-16/ARC (`Crc_Compute16`, from 0xeafb) of the part with that word 0. The load takes
  the word out (zeroed), checks it (error 0x12 if wrong), copies the part back and applies the sound options; the
  version is not checked.
- Checked in the runner: 100,000 random operations from a new game (assigning, clearing, spending, refunding,
  resets, merges, names, flags, race results, bests) with the block compared each time; 50 saves.

## The career's static tables (board row E12c; checked: `tests/career.rs`)

- `Career_InitStaticTables` 0x169e38, once (while `DAT_00335258` is set, which it clears): the outfit table at
  0x3cd0e0 (by character and outfit, 7 × 0x10: the outfit's model 7 · character + outfit, outfit 6 repeating 5,
  then the character three times) and the board table at 0x3cd620 (12 × 12 words, each its own index), and
  `gCheat_Character` cleared. Its readers (`Career_GetOutfitWord0/1/3`, `Career_GetRiderAttrib`,
  `Career_GetBoardWord`) are lookups. `StaticTables` keeps them, with the table as data.
- `Career_GetStatusName` 0x1634c0: statuses 0..9 are the strings 0x17..0x20 (`Loc_GetString`, E5a2), others none.
- Checked in the runner on 20 cases of random memory (pending and not), every reader of every entry, and each
  status −2..11.

## The World Circuit's results and unlocks (board row C12; checked: `tests/circuit.rs`)

`tricky_game::circuit`, on the rider set-up at career +0x320c (+4 the saved part, +8 the race, +0xc the places,
+0x10 the player's slot, +0x14 the heat, +0x18 the heat's bonus, +0x1c twelve rider points) and the circuit's
globals (0x3353f0 show-off final, 0x3353f4 race final, the unlock list 0x335400 of (who, kind, value) and its
count 0x3353f8, the riders out 0x335664 + 0x10 · character). The heats' rider set-up is E1b2o's.

- `Circuit_ProcessRaceResult` 0x16bcb0 (from `Career_CheckNewRecords`): only a circuit show-off (5), or a circuit
  race (4) at its last heat (2). The medal: the show-off's score (places +0xac) through `Circuit_GetMedal`(score,
  course); the race's first place held by a player's rider (clamped to 3). The course's medal byte in the player's
  character record (+0x24 + 4·course show-off, +0x25 race; 3 none). A better medal adds the points difference
  (0x37ee40: 15, 10, 5, 0) to the mode's total (+8 show-off, +4 race), to +0 and to all twelve rider points
  (`Circuit_AddPointsToAll`, +0x164); +0 is then held to what the caps leave (Σ cap − group byte +0xc..), or set to
  it once every circuit course has gold in both modes. Then `Circuit_ProcessUnlocks`(old +4 + +8, new, medal); a
  non-empty list marks the career changed (+0x3258).
- `Circuit_ProcessUnlocks` 0x16c420: a medal listed (kind 7); for each active slot, trick-book pages 0..4 newly
  complete (kind 5, the record's +0x5c bits 2 << page) and the whole book (kind 8, with page 5: +0x20 set and the
  circuit player's +0x58 bit 0x800), the race's flags (kind 9) — listed by slot, all listed before any bit is set.
  When the total moved: the next course in the mode's order opened (the saved +0 bit, kind 2; the record's +0x26
  race / +0x27 show-off byte, kind 1 / 0); a medal on Alaska (the last) opens the finals (bits 7 race, 6 show-off,
  kind 2, the two globals); a gold unlocks the next character in the order 0x3352c0 (3, 4, 7, 0, 10, 5, 6, 1, 11,
  8, 9, 2: kind 3, saved +4); boards whose points (0, 0, 5, 20, … 240, 999) the total reaches (+0x58, kind 4); the
  level crossed (240, 200, 160, 120, 90, 60, 35, 20, 5, 0: the last crossed wins; kind 6, career +0x370c set, level
  0 the +0x5c bit 0x40).
- `Circuit_ScoreEventResult` 0x16d0b0: the player's character's heat place sets the bonus (+0x18: 0.833, 0.334,
  0.167), past third it is out; riders past third are out; it goes on while the heat is not the last; at the last
  heat a player not out keeps the rider points (`Circuit_SaveRiderPoints` 0x16c328: the set ones into +0x168, 8
  apart). `Circuit_AdvanceEvent` 0x16c3b0: a race going on takes the next heat (4); else the heat back to 0 (2).
  `Circuit_StoreRiderAttitudes` 0x16c228 (proven; from `Career_StoreCircuitAttitudes` 0x114f70, which `cAI::vf17`
  0x111820 calls after the results in game mode 4 with career +0x27dc count and +0x27e0 entries): finds the last
  human entry (+0x65 ≥ 0, entries 0x84 apart) and stores, per rider i, `circuit+0x1c[entry+0x38 character]` = that
  rider's attitude to the player (race rider list +0xc4[i] → +0x50 + 0x20 · player). So the "rider points" are the
  rivals' attitudes. `Circuit_ResetHeat` 0x16bca8: circuit+0x14 = 0 (from `Career_ClearAllPlayerFlags` 0x161840).
- The lookups: `Circuit_FindEntry` / `Circuit_GetMedalThresholds` give 8 for a course not in the order (and the
  last entry's address), `Circuit_IsFinalEvent` is the seventh.
- Checked in the runner: 20,000 random careers (players in slots, races in every mode, medals, trick books, flags,
  points) over every function, the block, globals and list compared; every kind of unlock reached.

## The career's remaining wrappers and the course info (board row E12e; checked: `tests/circuit.rs`)

- `Career_FinishRace` 0x163ab0: a circuit race or show-off (4, 5) takes `Circuit_AdvanceEvent`'s answer as the
  next screen (+0x3260: 4 another heat, 2 done); any other mode 2, the tutorial (6) setting the mode back to 4;
  then `Career_UpdateAttributes`. `Career_IsCircuitFinalEvent` 0x1618a0 and the final-unlock getters 0x1618c0 /
  0x1618e0 forward to the circuit's (`Circuit::is_final_event`, its two flags); `Career_Construct` 0x161730 runs
  `Career_InitStaticTables` twice (itself and through `Circuit_Construct`; the second does nothing).
- The player's words: the board table at (character, profile +0x10 board), the outfit table at (character, profile
  +0xc outfit) word 0, 2 (`Career_GetPlayerRiderAttrib`), 1, 3; the `…For` forms take the board or outfit; both
  tables are read flat (character · 12 + board, character · 7 + outfit), so a board or outfit past the row reads the
  next character's. The profile's words 1 and 2 have a getter and setter each. `Career_SetPendingVideo` copies a
  name to +0x3714.
- The course info (`courses::INFO_STRINGS`, `NAME_IDS`, `UNLOCK_HINT_IDS`, `DESCRIPTION_IDS`, `MAP_MARKERS`): string
  ids for the difficulty, location, vertical drop and length (switches: course 9 shares Pipedream's, anything else
  past 11 the last), the name, unlock hint and description ids and the venue map's marker (jump tables 0x37ebf0..
  0x37ecb0; past course 11 by an unsigned compare: 0xe1d, 0x242, 0x240, 0.05). Course 10 (the trick tutorial) has
  its own name 0xe1a but the default hint and description.
- Checked in the runner: each wrapper on the random careers of `tests/circuit.rs` (1,600 cases each kind), the
  course lookups for courses 0..15 and three values past them.

- `StaticInit_RaceScene` 0x169dc0 (row E1b2q): nine scene constants written at 0x3cbff8..0x3cc020 (1.0, 0.9646, 0.0 and 0.0188, 0.949, 0.0012 as floats; nothing reads them by name), `FUN_00168360` (the scene table a, `fxrand::scene_table_a`), and `FUN_00169640` on the race scene object at 0x32edc0: its first word is 0x10 and its word 0x10f is 0 (`FUN_00169688`). That object is what `Update`, `RaceScene_Prepare` and `RaceScene_Play` run on; their rows own it.
