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

## Rank catch-up: the unchecked list walk

Proven on the PS2: `Race_UpdatePlacingsAndRubberBand` 0x115100 runs every 6 ticks (`race+0x18 % 6`), over
`race+0x88` riders, with `race+0x7c` players (rider+0x418 is the finish time, race time × 100 set by `Boarder_Finish` 0x11da40, −1 until the line; sorted by `0x7fffffff - x`).

Lead from another version (Xbox `default.xbe`, not `SLUS_203.26`; from the OpenTricky project's notes,
`port/tools/fork/fix_riderlist.py`, Xbox addresses there): the Xbox function sorts the riders, splits them into a
player list (4 slots) and an AI list (6 slots) on its stack, then walks the AI list up to the AI count fixed when
the race was built and gives each AI a target from the player list, with no check of how many entries either
list really got. When the counts disagree, it reads stack leftovers as a rider index (a crash was seen once on
the Xbox). Not yet checked on the PS2: whoever ports this function should read whether 0x115100 walks its lists
the same unchecked way, and port that 1:1 (the bug included), noting it here.

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
  `FUN_00166c70` numbers the 13 character records.
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
