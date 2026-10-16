# Meter, scoring, tricks: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Meter, uber, scoring (score object boarder+0x5820, `Score_*` 0x155410–0x157490)

- Points = round10(mult·units·6786.545) (`Score_CalcPoints`; the score object's arithmetic is ported exactly in
  `tricky-rs/game/src/score.rs` and checked bit for bit, G2a); meter += units·0.6786545·(0.97878+0.35966·a1d)/(repeats+1) (negative too).
- Units (exact floats in `tricky-rs/game/src/rider.rs`, G17): half spin 0.0699543 (`Score_UpdateSpin`); flip in
  `Score_UpdateFlip` 0x155e68, grab start and grab rate (score+0x2c, by clip id) in `Score_StartGrab` 0x155ee8, hold
  and rail per tick in `Score_Tick` 0x156e80. Values: flip 0.2499898; grab start (n+1)·0.0424975; hold rate·0.000750064/tick;
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

## Landing, chains and banking, exactly (board row G2b, checked: `tests/trick_score.rs`)

- `Score_LandTrick` 0x1569b0(rail, a, b): a trick (any spin, flip or grab) adds one to the chain (+0xf4), 0.19998 units
  if a chain kind was set (+0x20) and 0.18 more into a rail, then counts repeats (not with +0x18) and announces it.
  A rider over the line (rider+0x418 ≥ 0: its finish time, `Boarder_Finish` 0x11da40; −1 while racing) stops there: combo reset, share 0 — nothing scores after the finish, for any rider. Otherwise share = meter(units)/(n+1),
  points = points/(n+1) + chain bonus (none for 0–1 tricks, 4,000 per trick from two up to 16,000; skipped into a
  rail, chain cleared either way) + 1,000 a whole second of air from four (+0x44 ticks ÷ 60). Points > 0 push a
  2.5 s message (kind 2, 3 if a repeat) and count toward the rank table (0/1001/2501/4001/7501/11501).
- `Score_StartTrickChain` 0x156618(switch, kind, a, b): a chain in progress is scored first as a rail trick (id
  word 1 = kind·0x1000 + half turns·0x10, time = rail ticks ÷ 60, a chain trick if over 1 s), then reset with the
  multiplier 1.3 for a switch start. The focus player clears the audio object's +0x6e00 and sets +0x6e04.
- `Score_BankPoints` 0x155aa8: points pushed as a 1.5 s message (kind 1), combo reset, chain 0, one more bank.
- Trick id: special-name table by spin column (half turns; +10 for a positive spin or more than 10) and flip row
  (+3 backwards or more than 3); a second table (0x320f40) for chains begun with +0x14 and called with b. Past the
  table (21 × 7) the trick is 0x2c, "???", never a repeat (nor are three-grab tricks).
- Names: grab ids 0–0x65 have names (0x65 "Combo Grab"); past them the original reads into the next data (no
  grab in the game is that high), which our port prints as "(null)".

## Uber timer and TRICKY, exactly (board row G2c, checked: `tests/boarder.rs`, `tests/trick_score.rs`)

- `Scoring_OnTrickEvent` 0x156148(seconds, clip) is a grab let go (only while one is held): the clip's grab id
  (jump table 0x370fc0; clips 0x278..0x2db, ubers 0x2ac.. = ids 0x35..0x64) goes into the jump's grab list
  (`Score_AddGrab` 0x1571e0: not again if it is the last one, three at most), +0x5c counts ubers and +0x58 other
  grabs, hold time summed (+0x50) and longest kept (+0x54), units += seconds × 0.04500384 × the grab's rate.
- Landing adds +0x5c into score+0x88 = rider+0x58a8: `Boarder_IsTricky` is that total > 5, so TRICKY is the sixth
  uber landed (a landing after the finish never adds it).
- `Boarder_DrainMeter` 0x11b1f8 each tick: the uber timer −1/60 to 0, except in the air (+0x424 = 1) where it
  holds at one tick; TRICKY holds timer and meter at 1; else the meter of a rider still racing (rider+0x418 < 0) drains (15.483508 − 15.210359 ×
  stat/255) × 9.166667e-6 a tick (stats+0x1b).

## Who calls the score functions (board row G2d; wired in `Rider`, `tricky-rs/game/src/rider.rs`)

- `Jump_ApplyImpulse` 0x1284e0: `Score_StartTrickChain`(stance ≠ stats+0x40, rider+0x1dc, steep lip, 0) and the
  share to `Boarder_AddMeter`. Riding off an edge starts no chain (no air-time count).
- `SpinState_Update` 0x100908 each tick: `Score_UpdateSpin`(rider+0x1d0 in degrees, negated for stats+0x40),
  `Score_UpdateFlip`(rider+0x1d4 in degrees).
- `SpinState_UpdateTrickClips` 0x1272e8: `Score_StartGrab`(clip) as a grab clip reaches its hold flag;
  `Scoring_OnTrickEvent`(0, clip) as a grab clip ends; an uber clip (type 0x10) ends with its length in seconds.
- `AirMotion_Update` 0x108378: a landing is `Score_LandTrick`(0, landing switch: stance ≠ stats+0x40, flipped when
  the board came down more than 90° round, normal z < 0.7); a crash there `Score_Crash`(0).
- `Rail_TryCapture` 0x125cb8: rider+0x1dc = 1 (board within 30° of the rail), 2 (past 150°), 3 / 4 (across, by
  side), then `Score_LandTrick`(that kind, rider+0x1b4, 0); `RailSlideControl_Enter` `Score_UpdateSpin`(0);
  `RailSlideControl_CommitQuarterTurn` `Score_UpdateSpin`(the rail's turn in degrees).
- `RailSlideMotion_Update` 0x10b0d0 leaving: `Score_StartTrickChain`(stance ≠ stats+0x40, rider+0x1dc, 0, 0).
- `SpinState_StartFromStick` 0x102158 (late trick): `Score_CommitAndRestartChain`(rider+0x1b4).
- `Boarder_PlaceAt` 0x11cae8: `Score_BankPoints`, its −0.12 to the meter unless a reset was asked for (+0x304).
- Crashes (`AirMotion_Update`, `GroundMotion_Update`, `Boarder_RiderCollisions`, `Boarder_WallImpactCheck`):
  `Score_Crash`(0) and its −0.1 to the meter. Every tick: `Score_Tick`(|v| cm/s), and per rider in the race tick
  `Boarder_DrainMeter`. The HUD reads the run's points from score+0xf0 (rider+0x5910).
- Our rider runs these on a 60 Hz clock of its own (the physics steps at up to 1/120 s). STANDIN in the wiring:
  stat bytes from our 0..1 attributes (×255), rider+0x424 = 1 taken as in the air, the steep-lip flag (needs
  `jump::jump_impulse`, row G1a5b), the rail capture angle's sign, a tweak (rate and name of the tweak clip on
  the held grab), an uber's `Score_StartGrab` when it takes over a grab, the spin counted again from a late
  restart, the pickup multiplier, the HUD text for chain and big-air bonuses, and stats+0x40 taken as the rider's
  goofy stance (so a goofy rider's spin is scored mirrored: the same turn is FS for one, BS for the other).
- What changes against our old scoring: a flip counts only when the rounded flip is a whole turn (265° lands as no
  flip; ours rounded it up), and the side follows the stance. `TRICKY_SCOREDBG=1` prints what each landing hands
  `Score_LandTrick`.

## Scoring (the rest)

- Modes: 0 knockdown practice, 1 free ride, 2 practice race, 3 practice show-off, 4 circuit race, 5 circuit
  show-off, 6 lesson, 7 timed race. Trick points are the same in every mode; nothing is banked after the finish.
- Points, boost, multiplier and show-off-time pickups only work in show-off; speed and spin boosts everywhere.
  Points pickups never reach the score total. Breaking things scores nothing; a knockdown gives +1 meter only.
- TRICKY = more than 5 cleanly landed ubers; lasts the event: meter and uber timer held at 1, no drain, no uber
  bail penalty, no points multiplier. Bail: chain 0, pending trick lost, meter −0.10. Stumble −0.02.
- Trick-trigger zones (controller 0x15): run a script if the rider's score rises by a threshold within a time.

## Trick book (DATA/TUTORIAL/TRICKDEF.DAT, `Score_TrickBookCheck` 0x1579e0)

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

## Uber sets per board

- Each rider has a set of ubers for each board type (bx/fr/ex <Chr>Uber.afl), on Indy, Method, Mute and Stalefish
  with different styles (MT/OT/SK); the signature uber (UT_SIG<CHR>) is only in the set of the rider's own
  board type: freestyle for Eddie, JP, Kaori, Mac, Seeiah; alpine for Brodi, Marisol; BX for the rest.

## The trick book check (row C15)

`Career::trick_book_check` ports `Score_TrickBookCheck` 0x1579e0 (the real start; the board's older 0x157a00 is mid-function), checked in the runner (`game/tests/trick_book.rs`, 60,000 random states). A landed trick's id words match a book entry when word 0 is equal and word 1 agrees under 0x07000f00 (`FUN_00157c18`). With a tutorial lesson running (`(gApp+0x730)+0x34`) it only compares the lesson's trick and sets the lesson's +0x64; otherwise each of the 30 entries that matches, is neither done (+4) nor forced (+8), and is on the first page not yet complete (`FUN_001640a8`: five a page, against `TrickBook_CountCompletePages`) is forced and flagged in the slot's race flags (+0x310c). Forcing counts as done, so one landing ticks at most the entries of one page, and a page finishing in the loop moves the gate on to the next within the same pass. Any tick gives one message (`Scoring_PushMessage(1.2, 0x11)`) and `Sfx_OnTrickBookDone`. Skipped while the race object's `+0x40` has a lesson mode between 1 and 9, or when the rider's player byte is negative. The app's `src/book.rs` keeps its own name-based book until it moves onto this (row C15b).

## Grades and the trick list's helpers (board row G4l; `tricky_game::grade`, checked: `tests/aipad.rs`)

- `Score_GetGradeForPoints` 0x155588: `Score_CalcPoints` of the score's +0x24 units and +0x28 multiplier against
  the rider's level (the character entry's +0x74): grade 0..3 at the level's four thresholds (`grade::GRADES`:
  level 0 50000 / 40000 / 30000 / 20000, each level 2000 less), 4 for any points from 0, 5 below 0 or for a level
  out of 0..9. Level 2's grade-2 test asks for 36000, which its grade 1 already took, so level 2 skips grade 2.
- `Score_PointsToIconTier` (4000 / 8000 / 12000 / 16000 → 2..5, else 0), `TrickId_Matches` (the first word and the
  second's 0x7000f00 bits), `FUN_00155538` (the first of the twelve entries at +0x15c of kind 8 or 7),
  `Score_ClearTrickEntries`, `Scoring_MsgCheckpointTime` (kind 10 for 5 s with the time negated when ahead, 11 for
  2.5 s behind), `RelTable_GetTolerance` / `_GetAttitudeCap` (the pair's words +0 / +8).
- Checked in the runner on 60,000 cases each, every grade reached.

## The score's messages (board row G2; `tricky_game::score`, checked: `tests/score_msg.rs`)

- The twelve timed entries at the score's +0x15c (20 bytes: the kind, 0 free; the value; how long, seconds; the
  age; an extra word) are what the HUD's score messages draw from. `Scoring_PushMessage` 0x157330 (the life in f12;
  the kind, the value, the extra) picks a slot: kinds 7 and 8 (a trick, a repeated trick) reuse the first entry
  holding 7 or 8, kind 12 the first holding 12; otherwise, or with none, the first free entry, and with all twelve
  taken the last whose age / life is under 1. With no slot at all the game writes the entry one before the first
  (the score's +0x148); the port leaves that write out. It always ends in `Speech_OnScoreMessage` 0x233220 (the
  rider at +0x24c, the kind, the value).
- `Scoring_MsgMultiplier` 0x155ca0: kind 13 for 2.5 s with the multiplier, only above 1. `Scoring_MsgKnockdown`
  0x155d98: +0xbc counts one more and kind 4 for 2.1 s. 0x155cc8 is an empty function.
- `Score_AnnounceTrick` 0x157088 (the id, a repeat): keeps the id at +0x6c (so the grab list at +0x60 is three
  words), formats its name into +0x250, has the focus player's announcer say the trick with its points
  (`Speech_AnnounceTrick`) when it is no repeat or in the tutorial, else `Speech_OnTrickLanded`, then a 3 s
  (0x404004da) message, kind 8 for a repeat and 7 otherwise.
- `SpinState_SetTrickClipRate` 0x156050 sets the spin state's +0x2c from a 100-entry jump table (0x370e30) that gives
  each clip 0x278..0x2db the same rate `Score_StartGrab` does; other clips leave it.
- Checked in the runner: 60,000 random sets of entries (all kinds, lives and ages around their end, all twelve
  taken), 2,000 announcements over every mode, repeat and focus case, every clip 0x200..0x2ff; 20 of 21 mutants
  caught (the survivor, kind 12 keeping a slot found for 7 / 8, cannot happen).
- The app's `Rider::show` pushes every message and announcement into the entries, which `gamehud` copies to the
  rider's +0x597c for the HUD. The announcer side (speech) is not wired yet.
