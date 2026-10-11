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
