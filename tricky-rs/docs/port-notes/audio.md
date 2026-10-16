# Sound and music: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## Sound and music (`Audio_*`, `Music_*`, `gMusicSys` 0x343150, the one `cMusicSys`)

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

## Rider sounds (`Audio_SurfaceGroup`, `BoardIn_*`, `Voice_Update3D`, DATA/CONFIG/SNOW.INF)

- Surface groups: 0 PACK (1, 15–17, default), 1 POWDER (0, 3, 4), 2 LOOSE (2), 3 ICE (5, 6), 4 METAL (13),
  5 WOOD (12), 6 RAIL, 7 ROCK (9–11), 8 GLASS (14), 9 CHUTE (18, 19). zboard.bnk program = group·8 + slot:
  1 landing, 2 takeoff, 3 carve loop, 4 glide loop, 5 AI landing, 6 AI glide; program 0 the slow scrape.
- SNOW.INF programs (accumulator + stack: Load, Map n → x·127/n clamped, Square, Sqrt, Add/Sub/Mul/Div,
  Bound, Push/Pop, SAdd..., AssignVol, AssignBend) per section and GLIDE / AIGLIDE / CARVE, on LOAD = |v·fwd|,
  SLIP = forward drag deceleration, DIG = |steer|·127, LEAN = LOAD/4 + 2·SLIP, BEND = LEAN + LOAD (cm/s).
- Compiled exactly (row E5b2, `tricky_data::snowsfx`, checked: `tricky-game/tests/ini.rs`): `Sfx_LoadSurfaceConfig`
  0x210c38 calls `Sfx_LoadBoardConfig` 0x21a7c8 for each of PACK, POWDER, LOOSE, ICE, METAL, WOOD, RAIL, ROCK, GLASS,
  CHUTE × GLIDE, AIGLIDE, CARVE (table +0x745c + surface·0x50 + mode·0x320: the program pointer, its length at +0x44):
  from the top, `IniReader_FindSection`(surface) then (mode), so a run of empty surface headers shares the modes after
  it (PACK, ICE, CHUTE, ROCK, METAL share one set; POWDER, LOOSE another; WOOD, RAIL, GLASS another). Bytes: LOAD 0 +
  input (SLIP 0, DIG 1, LEAN 2, BOARD 3, BEND 4; another word: the 0 alone, and the next byte is then the input),
  LOADCONST 1, MAP 2, SQUARE 3, SQRT 4, LOG 5, ADD 6, SUB 7, MUL 8, DIV 9 (operands: `sscanf` "%d" words, 4 bytes
  little-endian), NEG 10, ABS 11, BOUND 12 lo hi, PUSH 13, POP 14, SADD 15, SSUB 16, SMUL 17, SDIV 18, ASSIGNVOL 19,
  ASSIGNBEND 20; unknown words skipped; 0x7f ends. The interpreter is `FUN_0021ae90` (state words: program ptr, stack at [1..], pc [0x11], sp [0x12], acc [0x13];
  LOAD n calls 0x21a6a8, 0x21a6f0, 0x21a710, 0x21a740, 0x21a788 on the sound object); ported as
  `sfxcalc::run_snow_program`, runner-checked (tests/sfxprog.rs).
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

## Announcer's race-end lines (`Speech_*`, F1c)

- Proven (read in Ghidra): `Speech_QueueResultLine` 0x231548 picks one of `Speech_QueuePlaceLine` 0x231800 (the
  human's placing as a bit, 1, 2, 4, 8, 0x10, or 0x20 for last; event 0x4008007, plus 0x4008009 in a career event of
  mode 2) and `Speech_QueueMedalLine` 0x231af8 (one entry only: `Circuit_GetMedal` as a bit, event 0x4008013). Both
  set race+0x7dcc bit 0 so they speak once, and skip while a replay plays. `Speech_RaceStatsMask` 0x2315e0 turns the
  rider's run into condition bits from per-course thresholds (course id 0x32de88; time fast/slow, two counts, and
  +0x66c = 0 or > 1). `Speech_QueueUnlockLines` 0x232330 (from `cUnlockedOverlay::Reset` 0x1b5ef8) posts 0x4008015,
  then one 0x400800d per unlocked item (list at 0x3353f8, entries of 12 bytes; item kinds 0..8, 0xb mapped to bits).

- Rider event sounds (F1c, proven unless marked): `Audio_QueueEventSound` 0x21d0b0 (audio object `*0x343050`) stores
  a kind at +0x7428 and sets +0x7420 = 1 (pending); `Audio_StopEventSound` 0x21d5c8 stops the bank sound whose handle is
  at +0x7424 (if ≥ 0) and sets it to −1. `cBoarder::vf10` 0x11a348 queues kinds 0, 1, 2 when rider+0x388 bits 14, 23,
  24 turn on and stops on their turning off. **Inferred:** whatever reads +0x7420/+0x7428 later starts the sound and
  keeps its handle at +0x7424 (the reader is not found yet; the bits' meaning neither).

## The announcer's and riders' speech triggers (board row G10c1; checked: `tests/speech.rs`)

`tricky_game::speech`. A trigger posts a speech event: `SpeechEvt_Post<id>` (0x22e4e0..0x2306c0, 92 of them) zero a
0x50-byte record, put the id in word 0 and their arguments in order after it (through pointers or not, the record
is the same; the 0x8000..0x8005 posts too) and hand it to `SpeechLib_PostEvent` 0x2bba80. Slot 0 is the announcer,
slot 1 the riders' own voices: those post a 0x3006xxx event with the speaker's character bit (`Speech_CharacterBit`:
by voice 0..11 a bit of 0xfff, 0 past them) and call `SpeechSys_Trigger` at the rider's position with its voice
(13 the crash voice, 12 the announcer's).

- `Speech_RollChance` 0x235d58(kind): one `Rand_Fx`, its low ten bits below the table 0x3a3528 (24 kinds × 10
  levels, 0..1024) at the speech level (+0x383c truncated, held under +0x6e68).
- The announcer's guards, in the game's order (it matters for the draws): a player's rider (+0x418 < 0), then
  either roll → not the tutorial (mode word 0x32de8c ≠ 6) → no replay (boost 10 / 9, trick landed 4, air time 0x17,
  ten seconds left 0xc, 0xd, rider collision 7 then the listener) or no replay → not the tutorial → roll (grudge 0x11
  with the other a local player, 0x12 / tricky 0x13 the listener, tricky under six, rider hit 0x14, air enter 0x15 at
  1341.12 cm/s, wipe-out 8 / 0x10). Checkpoints count per race index (+0x6e6c) for local players before the roll.
- The riders' voices: a crash nearby interrupts the rider's voice and the announcer's, then (if slot 1 can play and
  a listener is near) 0x3006023 / 0x3006024 by a `Rand_Fx` bit; two riders hitting each other pick a speaker (a draw
  when both are near), the mask from `Speech_IsRivalOrGrudge` (relation 2, or 0..1 with a rival target or an attitude
  above 60) and which hit; taunts need both near the same listener; air nearby needs three seconds of air.
- The start (`Speech_OnRaceGo` 0x2334b0, roll 5): 0x100204a with the mode's bits (race 2|1, show-off 2|1 kind 2,
  circuit 4|1 kind 1 / 2), the course's bit and the kind; then the start line (`Speech_PickRaceGoLine`: by mode, the
  career's event mode for the circuit race, Pipedream / Mercury for the show-off) as 0x1002020 when it has bit 2,
  else 0x100201f. Score messages (`Speech_OnScoreMessage`, roll 3, the listener, not mode 1 or 6) post 0x100204e and,
  for messages 0xe / 0xf, 0x1002031. The next race (`Speech_OnNextRace`, one profile in mode 2 or 4) posts 0x4008017
  with how the computer riders' attitudes to the player's rider split (above 80, 20 or under) and the gApp+0x58 bit.
- Big scores (`Speech_OnBigScore` 0x235ba8, 1000 or more): 0x500a000 with the score, spoken at gain 1500 at the rider
  (race rider 0 when none); slot 1 held (+0x34c) while the library may call back `Speech_QueueBigScore` with values for
  the speech system's ring (+0x7dd4: size, buffer, write, read; a push is dropped when full), else −1 is queued; then
  −2. `Speech_CheckBigScore` passes scores over 999.
- The masks: `Speech_CourseMask` / `Speech_CharMaskOf` (12 bits, 0 / 0x2000 past 11), `Speech_CurrentCourseMask`
  (the course word, a different order), `Speech_GetModeMask` (gApp+0x58).
- The race's end (`tricky_game::speech`, checked; the F1c notes above read): `Speech_QueueResultLine` speaks once
  (+0x7dcc bit 0): the placing line (`FUN_00231800`: the first player rider in the places, its place bit, 2 | 4 when
  it is in the new-record list (the game reads on past the list's five entries into the pre-race selection object at
  0x32ecf0 until a negative word), the mode bit, the stats mask; 0x4008009 too at a circuit final) or the medal line
  (`FUN_00231af8`, one rider: slot 0 cleared, 0x4008013). The stats mask (`FUN_002315e0`) reads the rider's run at
  career +0x3300 (time), +0x32ac / +0x32b0 (counts), +0x32dc, 0xbc apart, against per-course thresholds.
  `Speech_QueueRaceEndLines` (once, bit 4) adds the circuit lines (top half: 0x400800c, then 0x400800a naming the
  character with the most of the player's count at +0x3348) or the show-off medal's 0x400800c; `Speech_OnHiScore`
  posts 0x400800b when no result line was spoken; `Speech_QueueUnlockLines` (0x232330) one 0x400800d per course
  unlocked (kinds 0..2 of the circuit's list). The mode and course words the lines read (0x32de8c, 0x32de88) are the
  career's +0x2afc and +0x2af8. `SpeechEvt_Post4008015` takes one word (a caller passes two).
- Overtakes (`Speech_OnPlacingChange` 0x2347c8): 600 frames after the last (+0x7ddc against the race clock), two
  players' riders near the same listener; who speaks from local player / listener / relation 1 / attitude above 60
  (a also needs its count +0x5c + 0x20 · slot above 0), a `Rand_Fx` bit when both; spoken when their velocities
  along b's heading (`Math_SinCos`, cos · x + sin · y + z · 0) part by 670.56..1788.16: 0x3006019..0x300601c.
- Tricks (`Speech_AnnounceTrick` 0x232498(rider, the trick's two words, score)): no replay, a line allowed (+0x6e04),
  a player's rider; the roll's kind is the score's band (0x3a38e8: 0, 4000, 10000 → 0, 1, 2; below 0 the ten words
  before the chance table, string data in the exe); then 0x100201c (4, 2, 2, under 4000 1 else 2) and, for the first
  line since +0x6e00 was cleared and a trick with parts, 0x100200f: the rotation's bit (word 0 bits 24..27, or 28..31
  when set; 0..10), the stance (word 1 bits 16..19, overridden by bits 12..15, which also add 4 to the last word),
  the grab and flip counts (bits 8..11), the spin (bits 24..26), the name (byte 1) and move (byte 0, ',' 0x3d) ids
  and word 1 bits 27..29. The post's answer is kept in +0x6e04 (0 stops later lines); when not 0 the line is counted
  and byte 2 names a second trick (0x1002010). Last, roll 0x10 in show-off 3 or 5: 0x1002054 with 1.
- Lessons and cut scenes: `Speech_LessonLine` 0x231398 (0x400800f; lessons 0..2 once each, +0x7dcc bits 1..3),
  `FUN_002314e0` (0x4008010 with a bit from 0x3a2750), `Speech_QueueCamScriptLine` 0x230dd0 (lines 1000..6999 by the
  thousand: 0x4008002, 0x4008004, 0x4008003, 0x4008005, 0x4008015 with the bit of line − 5001, 0x4008016; slot 0 at
  voice 1 with `FUN_00230998` as the callback, which keeps the library's value at +0x7dc8),
  `Speech_QueueCamScriptLines` 0x230ee0 (the last table entry with the id, +0x7dc0 / +0x7dc4, 31 words: its lines to a
  0, −1 queuing +0x7dc8 on the ring at +0x7dbc), `FUN_00230ff8` (drains the ring past finished handles; the game loops
  for ever when a negative word empties it), `Speech_IsCamScriptLinePlaying` / `Speech_StopCamScriptLine` (slot 0's
  bank monitor, +0x80 = 1), `Speech_OnCamScriptIntro` 0x2310c8 (0x4008006 at a circuit race's first event: the
  course's bit | 0x400, the riders in relation 2 or 1 with the rider's character, its bit, the mode bit) and
  `Speech_InterruptIfPlaying` 0x234bb8 (slot 1's monitor: a line of the voice cleared).
- Checked in the runner: each post four times with random arguments; 60,000 random calls over 52 handlers and
  helpers with the speech library, replay, listener, near test, relations, the race's rider 0, the library's
  callbacks, the race clock and `Rand_Fx` scripted the same and random career blocks: every call and draw in order,
  the checkpoint counts, the big-score ring and the end flags compared. `Speech_OnScriptCue47` / `48`, despite
  the names, fill a sound manager's slot (`SndMgr_GetSlot`, `SndSys_PlayPending`) before their one post: G10.

## Pathfinder's graph as the engine walks it (board row G10b; `tricky_game::pf`, checked: `tests/pf.rs`)

The engine keeps four song records from 0x3495e0 (0x1d4 each; the selected one's address at 0x3495c8, its number at
0x3495d3, the marker callback at 0x3495cc) and works on the `.mpf` graph and the 24 track records through pointers;
the port keeps the same bytes at the same addresses (`pf::Mem`).

- `PathX_RegisterSong` 0x2c0b08: the graph ("xDFP" = 0x50464478, version 0x103) goes to the slot its +0xc names when
  free; the record's tables: +0x18 the node offsets (shorts from graph +0x24, +0x12 of them), +0x1c after them, +0x20
  the event grid (after the last node's branches: tracks (+0xd) × rows (+0xf) × columns (+0xe) bytes, rounded to 4),
  +0x24 its entries (+0x10 words), +0x28 the router table, +0x2c / +0x30; +8 = 50 (ms of look-ahead), +0xc = 10.
  `Path_Init` clears the four records, `PathX_SelectSong` picks one.
- A node (graph + offset · 4): +0 its audio (below 1 none: 0 a marker, −1 a loop start), +2 its track, +3 the event
  grid column (top bit: the target has one), +9 a loop count, +10 its router, +0xb the branch count, from +0xc four
  bytes a branch (low, high as signed bytes, the target).
- `FUN_002bf038` the branch by the path variable (0..255): on a looping node (the track's +0xc) the loop count (+5)
  is the variable, counted down per pass (the node forgotten at −1). `FUN_002bf290` the router: the pairs (high half
  → low half) between the router table's entry and the one before it remap a node. `FUN_002bf108` walks from a node
  past every node without audio, telling the callback of markers (and keeping the marker in the track's +0x10),
  starting loop counts, branching each step; the node reached goes through the router once more.
  `FUN_002bf320` / `FUN_002bf490`: an event's grid cell (track · rows · columns + event · columns + the current
  node's column, or the first track's with one) and its entry (low byte the action, 0x300 moves, 0x400 set when the
  target's column has the top bit, high half the target).
- Checked in the runner on all 21 songs of MUSIC.BIG (TRICKY_MUSIC_DIR): registration, then 3000 random calls a song
  on random track records, the song table, the graph, the tracks and the marker calls compared (a small graph's
  lookups read past its end; the test gives both sides the same bytes there).
- The scheduler (`pf::PfEnv` the stream library under it: `PathX_TimeMs`, `PathX_GetPositionMs`,
  `StreamSlot_Play` / `_Stop` / `_Fade` / `_IsDone` / `_GetPosition`, `FUN_00223a90`, and `FUN_00223b70` the volume,
  which is empty in this build). A track record (0xd8 bytes): +0 its number, +1 its path variable, +4 its volume, +5
  the loop count, +6 the beat counter, +8 / +9 its handle ring's size and place (+0xd4 the ring: node, beat, handle),
  +10 the stream latency (under 50 ms a beat-counted track), +0xc the looping node, +0xe the event it waits on,
  +0x10 its marker, +0x14 the event entry's volume, +0x15 flags, +0x16 its node, +0x18 the saved entry, +0x19 /
  +0x1a what to do when released, +0x20 the beat length, +0x24 / +0x28 when to start, +0x2c the stream slot, +0x30
  the handle, +0x34 the name. The song record: +0 the lead track, +0x34 the pending event (below 0: the complement of
  a node to walk to), +0x37 immediate, +0x3c / +0x40 the earliest / latest start, +0x44 the tracks done this pass.
- `FUN_002bfe28` starts a track's node on its stream (a beat-counted track counts beats and times the next, rounded
  to 10 ms; a longer one starts at its time when the stream is idle) and keeps the handle in its ring; a track with no
  node stops and releases the tracks waiting on its event (`FUN_002bfa78`: their volume, then walked on or stopped).
  `FUN_002bfbb8` advances a track at its segment's end (the same node within its beats, else the branch walked on;
  restored when the start fails). `FUN_002bf508` takes the song's event for a track (or walks a pending node): the
  entry's volume and waits, a move synced to the lead track by `FUN_002c00d0` (the 0x7f marker: the lead's position
  less the segment, wrapped by the lead's length; the −1 marker: the next beat of the target; else the next bar at the
  lead's tempo, computed in soft-float doubles as the game's libgcc does); a track that cannot move stops or waits
  (restored). `PathX_CountActiveTracks`, `FUN_002bef08` (volumes over 500 ms or fades), `FUN_002befd8` (a node's
  segment offset).
- Where the game crashes or hangs on bad data the port panics instead, saying so: a division by zero (its traps),
  a lead segment or bar of no length (an endless loop), a released track whose walk ends nowhere (endless
  recursion). Checked in the runner on all 21 songs: 2500 random calls a song over random tracks, events, leads and
  stream answers; results, every stream call, the song table, the graph and the tracks compared; traps, endless
  loops and recursions agree. The runner learnt `multu`, `dsllv`, `dsrlv` and `dsrav` (libgcc's `__muldi3`).
- The driver's callbacks (`PathX_Init` takes them): `FUN_002c0488` (the update) finds each song's lead track (the
  longest latency among those at a node with a marker); a track not yet done with the head event takes it when due
  within the look-ahead (+8; `next_segment`); a track whose stream is done sets the song's earliest start (+0x3c, the
  track at +0x38) from its position and, when its time is near (+8, or +0xc for a beat-counted track) or its
  position past its latency, advances (or takes the event); a track waiting to start (+0x28) is released at its time.
  When every track is done with the event (+0x44 all set) and its latest start has passed, the volumes ease in and
  the event queue (sixteen entries of 0x14 from +0x34: the event, its priority, immediate, the lead, the earliest and
  latest start, the tracks' mask) moves up one. `FUN_002c0898` (the beat) advances the beat-counted tracks at their
  time. `FUN_002bedb8` (the stop) frees every track.
- The controls: `PathX_LoadSong` 0x2c0cb0 (a track record set up: number, variable, default volume (0x3495d2),
  latency, the handle ring, the name and its base name as "%-8s" with every '.' cut, the driver started with the
  first track, the stream opened in the record's rest, 16-aligned after the ring; the id song bit 24 + n | track bit),
  `FUN_002c15c8` (an event for some tracks queued by priority; −1 releases those waiting; priority 2 is immediate,
  and at the head stops the song and unpauses), `FUN_002c1890` (a track's jump to a node at the head), `FUN_002c1288`
  (the path variable, under 128), `FUN_002c1068` (the look-aheads), `PathX_SetVolume` (0..100; all songs: the
  default), `PathX_FadeTracks`, `PathX_StopSong`, `PathX_StopTracks` (frees the first named track; the driver
  stopped with none left), `FUN_002c1b98` (pitch / pause), `FUN_002c1130` (latency), `FUN_002c1168` (the stream
  slot), `FUN_002c1190` (what a track plays: its ring entry for the stream's handle), `FUN_002c1380` /
  `FUN_002c1430` / `FUN_002c1500` (the queue cleared, counted, its head). A mask of −1 is compared as a 64-bit −1:
  the callers pass ints.
- Checked in the runner on all 21 songs: 1500 random calls a song over the nineteen callbacks and controls, with the
  song queue, tracks, pause, volumes and driver answers random; results, every driver call and all memory compared
  (a track with no marker has the sync read address 0's bytes: compared too).

## Front-end sounds (`cFE3DSound` → `Audio_PlayFE*`, F1c)

Proven (live Ghidra):
- `cBXAudio::PlayFESound` 0x20eaf0 (vtable slot +0x6c, was vf12): queues one bank sound with no position
  (volume 100, 0x5a, 0x32 defaults), bank in the `+0xa40` column, sound in `+0xa44`, voice slot 4
  (`SndMgr_GetSlot(4)`), plays it and returns the handle. Every FE sound helper calls it with bank 0.
- The menu helpers are named by the sound they play: `Audio_PlayFESound11` 0x212680 (handle kept in
  audio+0x7434, the old one stopped first; `Audio_PlayFERiderChange` 0x212428 is the same with sound 0x10
  and +0x7430), `Audio_PlayFESound12/30/31/32` 0x2123c8/0x212348/0x212388/0x2123f8,
  `Audio_PlayFESound32Gated` 0x212300 (only when audio+0x3714 > 3), `Audio_PlayFESound01` 0x21c930.
  `cFE3DSound::vf9` 0x270610 maps its event index to them.
- audio+0x7438 is the screen sound handle: `Audio_PlayFEScreenChange` 0x212960 plays 0x21 there,
  `Audio_PlayFEScreenSound20` 0x2128d8 plays 0x20 there (both only when +0x3714 is set, the first also
  when +0x7454 is set). `Audio_PlayFESound11ThenScreen` 0x21c960 plays 0x11 and, when +0x3714 is 0,
  sets +0x7454 and plays the screen change.
- audio+0x7450 is a held sound: `Audio_PlayFEHeld01/02/32` 0x21c868/0x21c8a8/0x21c8e8 and
  `Audio_PlayFEHeld20And00` 0x21c808 (plays 0x20, holds 0x00); `Audio_StopFEHeld` 0x21c9a8 stops it if
  still alive and sets +0x7454 = 1.
- Jukebox (DVD extras): `Jukebox_Open` 0x21cf38 opens stream 2 on data/audio/jukebox.big;
  `Jukebox_PlayTrack` 0x21cfa0 plays track n (audio+0x6e98 count, names 0x40 bytes each from +0x6e9c,
  empty names skipped) with `StreamSys_PlayFile(2, 2000, name)`; `Jukebox_StopTrack` 0x21d020 stops stream 2.
- Crowd sample lists (`TRICKn`/`FALLn`/`ANTICIPATEn` in `Sfx_LoadTrickConfig` 0x220ce8, 3 levels of 0x54
  bytes from audio+0x3730): `CrowdSoundList_Construct` 0x21eca8 zeroes {count, 5 entries};
  `CrowdSoundList_Add` 0x21ed30 appends a loaded file (max 5), returns 1 when added.

Inferred: the sound ids are entries of the front-end bank; what each one sounds like is not checked.

## The pending-sound queue (`sndqueue.rs`)

Every `Sfx_On*` handler and `Sfx_UpdateAir` inline the same entry: `++count` (+0x20) indexes parallel arrays of the audio
system (record copy of +0x08..0x20 via +0x24, emitter inline at +0x28, +0x38, volume +0x3c, slot +0x40, 8 bytes inline at
+0x44, +0x64, listener +0x68, +0x6c, +0x70, 16-byte rows +0x74 = 100/0/0x5a/0x32, +0x78, +0x7c, kind +0xa40, sound id
+0xa44), then `SndMgr_GetSlot(4)`, `Audio_GetRiderListener`, `SndSys_PlayPending`. The inline arrays at +0x44 would run
into the pointers at +0x64 from the fourth entry, so the queue never holds more than three. `queue_sound` is that entry;
`update_air` is `Sfx_UpdateAir` (tests/sndqueue.rs).

`WorldEmitter_Update` 0x22bf60 (`worldemitter::update`, tests/sndqueue.rs) keeps one emitter record's voice going. With a
handle (+0x30): the volume `(int)(v * 127.0)` goes to 0x221000 when +0x38 is set, else into the bank monitor
(`BankMonitor_Find`) at +0x64. Without one, and the key known (0x22c638): the entry is pushed (0x2210e8, the same default
entry as above with no kind or id), 0x2212a0 starts the voice and drops the entry again (count is left alone when the
audio system has no voice table at +0x372c), and the handle becomes `handle << 8 | rider`. Otherwise the grid may refuse
(0x22dc88), kind 6 needs the rider to be the listener's, the trigger banks and the ambient bank loaded (it only queues when
that returns 1), and a full entry is queued whose `PlayPending` result is the handle; any refusal frees the record.

## EA's sound library pieces (`sndlib.rs`)

`SNDVoice_CalcPitchMult` 0x2d5288 works a voice's pitch ratio (base cents bent by the note and by a table at a position,
`(byte - 0x40) * depth >> 6` each) through 0x2d51f8: whole octaves of 1200 cents shift 0x1000, the rest goes through a
256-byte table at 0x351288 (about `256 * 2^(i/256) - 256`, not exactly a formula, so the bytes are kept; the negative side
reads the same table from the end). `SndVoice_SetVolume` 0x20ae28 scales the volume by the voice's float at +0x90, clamps
to 0..127, and sends it to the voice's engine (1 and 2 under the sound lock, 3 the stream) unless within four of the last
one sent; -1 means stop. The scale is read as the hardware does (a denormal is zero; an exponent of 255 is a big number,
not a NaN, so tests keep it out). `SndVoice_IsDone` 0x20a8e0 asks the engine the voice is in.

## BANKS.INF (`tricky_data::audiocfg::{ingame_banks, fe_banks}`)

`Audio_LoadInGameConfig` reads `[GLOBAL]` for the last `BASEPATH`, then every section whose name contains the course's
upper-cased name (`strstr` on the section name, so case matters and an empty course matches all): `BASEPATH` changes the
base path for the rest of the section, `MAIN` / `BOARD` / `BANK` / `CROWD` / `AUX` / `TRICKY` load `base + file` into slots
0 to 5 (`BankManager_LoadIfChanged`), `SWAP` appends to the swap list at +0x38e0. `Audio_LoadFEConfig` does the same for
section `FE`: `MAIN` into slot 0, `CHARSPCH<n>` into slot 7. The game compares the reader's token buffer, which a read
`BASEPATH` value has overwritten, so a section whose base path is "MAIN" loads a bank. A key without a readable value uses
the file name left from before, and the first one the stack's: the `_checked` functions say when that happened (the
tests skip it). The disc's own BANKS.INF is not in this repo, so the tests run the originals on generated text.

## CROWD.INF, CHANT.INF, NASCRIPT.INF (`tricky_data::audiocfg`)

`Audio_LoadCrowd` reads CROWD.INF's `[GLOBAL]` and then the course's section through `Sfx_LoadTrickConfig`: keys
`TRICK1..3`, `FALL1..3`, `ANTICIPATE1..3` add a file name (at most five per list) to the set `n - 1` at +0x3730 + 0x54 × (n - 1)
(+0x18 per list), `ATTACK`, `DECAY`, `MAXBEND` set the three floats. `Audio_LoadGlobalBankList` appends every `[GLOBAL]` value of
CHANT.INF to the swap list. `Speech_LoadNarration` reads NASCRIPT.INF's `[<course>]` then `[<course>_ENG|_GER|_FRE|_JAP]`
(by the app's language word +0x58; English twice at 0 and 1): each line's key is the line's id (`sscanf %d`), its value
comma-separated words (`b` is -1, a non-number 0) into a 0x7c-byte record that holds fifteen lines in all. A line whose `=` does not
read uses the value before it (the stack's, for the first), so a line without `=` repeats the last one's words; the `_checked`
and tuple results say when that was the stack's. A float key without a number leaves the stack word in the game, so the tests do
not use one.

## Speech library tables (0x2bb018..0x2bba50; `tricky_game::speechlib`)
The effect player's own state, all at fixed addresses: the bank table (0x348dcc pointer, 0x348dd0 count), eight event-bank slots (0x348e08, two words each), per-group voice counts and two limit arrays (0x348e48, 0x348e68, 0x348e88) and sixteen 0x60-byte voice records from 0x348eac (+0 start time, +6 in use, +7 group, +8 state, +0xc the playing sound whose +2 is the minimum age and +4 the size).
- A bank: +0 and +2 ids, +4 flags (low nibble: bytes an entry less two; bits 4..6: header bytes in eighths; bit 7: a bitmap of one bit an entry), +5 entries, +6 history length, +8/+9 event-bank kinds, +10 sound count (0xffff none), +0x10 u16 event count, +0x14 size, +0x18 offsets (in words) of the event entries.
- The history ring sits after the entries, header and bitmap: first byte the write position, then `bank[6]` bytes; `register_bank` only takes a bank whose ring is clean (position 0, every byte 0xff) and, with bit 7, stirs the bitmap first (`stir_bitmap`: the divisions by the entry count are unsigned, with the number at 0x3494f0).
- `alloc_voice(size, group, limited)`: a free record; else the first whose state is 0, whose sound has a minimum age and is older than it, and is not the group's kept slot (released first); else a record of the same group with a sound at most `size` big, taken over; else -1.
- `reset_voices` ends with a halfword store at 0x349520.
- The library's own random numbers (`rand` 0x2be678: six words 0x349528..0x34953c added with carries, the last counting up by one and rolling over into the rest; `seed_rand` 0x2be768; `init_history` 0x2be8c8 seeds with `seed · 0x10001`) and the variant history (a ring of 32 id / tag pairs at 0x349548, position 0x349540). `pick_variant(n, tag)` 0x2be908 draws up to 32 numbers `(rand >> 16) · n >> 16` and keeps the one heard longest ago among the last `min(n / 2, 10)` of that tag, stopping at one not heard; the divisions and compares are unsigned where the decompiler shows them so.
- The event queue (0x348950 id, 0x348954 count, 0x348958 entries used, eight 0x5c-byte records from 0x348adc, 48 eight-byte entries from 0x34895c): `queue_event` 0x2bdb68 takes an event for one sound-system id at a time and stores up to eight; **a ninth leaves the count at nine** (the count is written back as `count + 1` whenever the id fits, stored or not). Its entries copy the first `bank[3] >> 2` triples of the sound's list, with a byte looked up at list + 0x62 + the triple's index byte.
- `free_voice` 0x2bc200 (only when the voice's state is 0): the record not in use, no longer the group's kept slot (either limit array), the chain after it (record +0xa, a signed short) freed link by link, stopping at the first link whose state is 0 **or** that is not in use, the group's count down one, not below 0. `shuffle` 0x2bd198 swaps `count / 2` bytes with `pick_variant(count, count)`'s picks; the pending (id, value) at 0x348dc0.

## Speech_LoadEvents (G10j, `game/src/speechload.rs`)

0x227d70 walks the `data/config/speech.inf` section that matches the level and knows three tokens (strings 0x3a03c8, 0x3a03d0, 0x3a0410): set the streaming file; open a BIG of speech banks named from the level (`sys+0x68`, the format at 0x3a03e0), or, when it is not there, the plain file (0x3a03f0, a `|` in the name splits archive from entry) and register each entry's bank with `SpeechSys_LoadBank` unless it is the one already at `sys+0xc8+n*8`; or load one named bank. `sys+0xe8` counts the open archives (words at +0x358); entries are two words from +0xc8. A failed open or lookup ends the whole read. The port takes every callee through `LoadEnv::call` by address; the test fakes them with strings written into the frame, 117 mutants killed.

## Chant banks and the music list (G10j2, `game/src/chantload.rs`)

`Audio_LoadChantBanks` 0x22dce0 sets +0x4104 / +0x4108 to -1, reads two ints from the first section, then for each token of the second tries the keys 1..20 in turn: per row `n` it formats the thirteen key names (0x3a1908 first, then 0x3a1918 .. 0x3a1990) and the first one the token equals names the string's place, `obj + (n-1)*0x40 + offset` (0xf04 through a format, the others copied as they are: 0x1404, 0x2304, 0x504, 0x1e04, 0x1904, 0xa04, 4, 0x3204, 0x2804, 0x3704, 0x2d04, 0x3c04). `SoundMenu_BuildMusicList` 0x187f58 lists the enabled songs (pairs of index and the song record address `sys + cur*0x20 + 0x12e0` at +0x144) and fills the menu's list header at +0x404. Tested with faked callees; 83 mutants killed.

## Crowd box bookkeeping (G10m1, `game/src/crowdbox.rs`)

A crowd box is 0x190 bytes: +0 its sound id, +0x10..0x1c a list of cheer files (count and five handles; `CrowdSoundList_Add` 0x21ed30 opens the file, keeps the handle even when it is 0 and counts it only when it opened; `List_PickRandom` takes `Rand % count`), fifteen slots of three words from +0x20 (in use, a value, the sound entry) with the number in use at +0xd4, three sound entries of 0x30 bytes from +0xe0 (id, voice, use count), +0x170 the voices playing, +0x174 the cheer voice, +0x180 the cheer state (0 off, 1 armed, 2 playing), +0x188 the cheer level (capped to 0.25 when a cheer stops). `StopSlot` 0x2201f8 frees a slot, stops the entry's voice (mode 1, with the fade as float) when its use count reaches 0, and when the last slot goes caps the level and stops the cheer voice (mode 2, only for a fade of 0); `StopAll` 0x220338 does it for every slot in use; `Stop` 0x2206e0 stops a playing cheer. The voice stop is 0x20c2a0 (audio object at 0x343120, voice, mode, fade in f12). Not done: `CrowdBox_Update` 0x21efb0, `StartSound` 0x21fc98, `PlayCheer` 0x2203a8, `StartCheer` 0x220538 (they fill voice request records in the audio object's tables). 110 mutants killed.

## Speech library effect player, helpers (G10l, `game/src/speechplay.rs`)

Part two of the sound library (0x2bbcf8..0x2bd078): small helpers over the sound headers and the voice records. A header (a bank entry) has: +0 u16 id, +2 u8 index, +3 u8 (low two bits the mode, the rest / 4 the part count), +4 a byte (low nibble n: rows are n + 2 bytes; bits 4..6 the number of 13-bit keys; bit 7 a second bitmap), +5 the row count, +6 a byte, +8 a signed count, +0xa a u16, then the rows from +0xc, the keys (big-endian halfwords, top three bits a kind) and the row bitmaps. Entries of the tables: 0x3494b0 (id, count pairs), 0x3494f4 the total for percentages, 0x3494f8 a word table, 0x348930 weights (words, by the top three bits of a byte), 0x348dec an optional check callback. Ported (all checked in the runner, `tests/speechplay.rs`, 60,000 cases, every reachable mutation fails): `PostEventArgs` 0x2bbcf8 (the event and its arguments laid out in the frame and posted through 0x2bba80), list contains 0x2bbd78, release-all 0x2bc170, check callback 0x2bc490, mask agreement 0x2bc7a0, id lookup 0x2bc850, row-bit clear 0x2bc938 / tests 0x2bc9d8 / 0x2bca80, percentage 0x2bccd0, weight 0x2bcdb0, entry check 0x2bcf48, mode check 0x2bcf90, key find 0x2bcfd8. Left for the next rows: the event row builder (0x2bcb30, 0x2bcd08, 0x2bd078..0x2bd6a8), the starter / updater (0x2bd798..0x2be468) and the voice picker (0x2bbdd0, 0x2bc048, 0x2bc320, 0x2bc4c0, 0x2bc548, 0x2bc5e0).

### The voice picker (G10l4)

Voice records are 0x60 bytes from 0x348e48. Ported in `speechplay.rs`, checked in the runner (`tests/speechpick.rs`, call log, memory and the output frame compared): `find_victim` 0x2bbdd0 (the voice to steal: filter by id list, minimum age, group limit, size then age then priority), `pick_voice` 0x2bc048 (writes the event key and header pointer to the frame at sp+0 / sp+4), `claim_voice` 0x2bc320, `run_group` 0x2bc4c0, `try_start` 0x2bc548 (with a busy word that makes a re-entrant call return 0), the wrappers 0x2bc5d8 / 0x2bc700 and `start_voice` 0x2bc5e0. Still open in G10l: the event row builder and the starter / updater.

### The event row builder and the starter (G10l5, `speechplay.rs`, `tests/speechrows.rs`, `tests/speechstart.rs`)

Three kinds of header meet here. A *bank* header (the table at 0x348dcc by id): +4 low nibble n the extra bytes of a row, bits 4..6 the key count, bit 7 a row bitmap; +5 the row count; +6 the history length; +7 the scale (minus one, times 256); +8 u16 the end value; rows from +0xc, n + 2 bytes each (two bytes of value, big endian; with bit 7 of the first byte the row is an alias for the row the second byte names). An *event* header (what a sound id finds): +6 the number of variants (u16 offsets / 4 of their row headers from +0xc), +7 the number of three-byte records (value, parameter number, kind) after them, then a word of two-bit parameter tests and a u16 per variant, +0xa bit 0 / bit 3 flags. A *row* header (a variant): +3 top six bits the parts (at most twelve), +4.. a byte per part naming its record at four times the byte; a part record has the bank id at +0, an index and kind at +2 / +3, a parameter count at +4 (at most four), the parameter bytes at +8 (0xfe: any row, 0xff: the row whose mask bit is pending) and the bit numbers at +0xc. The built event (`ctx`) is eight-byte part records (+0 bank id, +4 first index in the list at ctx+0x62, +5 how many, +6 the one in use), the part count at +0x60 and the list length at +0x61.

Ported: `row_info` 0x2bea48 (a row of a bank described in sixteen bytes: the weight to the next row that is not an alias, its own threshold, the extra bytes), `collect_rows` 0x2bcb30, `weight_sum` 0x2bcd08, `order_rows` 0x2bcdd8, `build_event` 0x2bd078, `shuffle_rows` 0x2bd218, `next_combo` 0x2bd2e8 (an odometer over the parts), `find_combo` 0x2bd370, `select_row` 0x2bd430, `select_rows` 0x2bd6a8, `has_value` 0x2be218, `get_record` 0x2be2b0, `record_callback` 0x2be310 (only kinds 0 and 3, per the jump table at 0x3b8e20), `param_masks` 0x2be468, `fire_params` 0x2bd798, `play_rows` 0x2bd928, `queue_run` 0x2bdc98, `param_test` 0x2bddf0, `start_rec` 0x2bde40 and `start_entry` 0x2be140. The four callbacks are the words at 0x348de0 (the block of seven words describing a row being played), 0x348de4 (asks about a parameter), 0x348de8 (parameter values), 0x348df0 (asks for another try).

Things that are true of the game's code and show in the port: the word at 0x348ea8 (not 0x3488a8) is the flag that raises the slot count; the bit number bytes are read with `sllv`, so numbers of 32 and up wrap; the part counts above four would read the saved registers of the frame (the real events never have them: `masks_agree` refuses five); `find_combo` on an event without parts never ends (the odometer reads the record before the first); the callback block's eighth word is whatever the frame held.


## The crowd boxes' voice requests (G10m2, `tricky_game::crowdbox`)

`CrowdBox_Update` 0x21efb0 (frame 0x130), `_StartSound` 0x21fc98, `_SetSlotValue` 0x21fc68, `_PlayCheer` 0x2203a8 and
`_StartCheer` 0x220538. Every one builds its request inline in the audio object's queue (the new entry `++count`, then the
parallel arrays; `sndqueue.rs` has the same entry on structs). The order that matters: defaults (record copy of the header
at +8, +0x3c = the first byte, a memset call on the +0x44 row, +0x74 row 100 / 0 / 0x5a / 0x32, +0x7c = 1), then the caller
sets the kind (+0xa40), the id (+0xa44), the emitter, the volume (+0x3c whole, record byte 0 clamped to 0..127), +0x40 =
box+0x17c, **+0x78 = 1**, the 8 bytes at 0x39fbd8 into the +0x44 row, then +0x64 = the box, +0x7c = 0, +0x68 = the
listener (box+0), and 0x20c948 plays the pending sounds; its answer is the voice. The cheers do not push: 0x222160 begins
the entry and the box fills in the newest one before 0x222358 plays it.

- Update blends the slots (15 of three words: the position pointer, the weight, the sound entry): the box position is a
  running weighted mean, `p += (s - p) * w / (max + w)`, with `max` the largest weight so far and the smaller ones
  added in as a quarter each (`sum`); the box's loudness is `max + sum`. With two or more voices the same mean is kept
  per voice (the entry's +0x20 is its index, the weight kept is the largest, the quarters of the others are summed).
  One voice: the last slot's entry gets the position and, when its monitor (0x20c200) is gone, a new request with the
  loudness; several voices: each entry in use does the same with its own loudness.
- The cheer part calls the audio object's virtual function (vtable at audio+4, the `this` shift a halfword at +0x80, the
  function at +0x84) with the listener and the addresses of two frame words that start at 0x1000 (a scale for the
  volume and for the monitor's own call); its answer 1 or 2 is kept in box+0x18c; with the cheer armed (1) and the answer
  not 3 a new cheer is picked from the list at audio+0x3760+0x54*answer and played (state 2).
- `lq` / `sq` ignore the low four bits of the address; the port does too.
- Not reached by the game and not tested: the slot / entry scans running off their ends (they would read address 0).

## Names from F1c batch 14 (proven from the code, live Ghidra)

- Snow sound programs: `Sfx_RunSnowProgram` 0x21ae90 is the interpreter (`sfxcalc.rs`); its inputs 1, 3 and 4 are
  `Sfx_GetDragAmount` 0x21a6f0 (|(int) rider+0x160|, the drag), `Sfx_GetSnowInput3` 0x21a740 (impact / 4 + 2 × drag)
  and `Sfx_GetSnowInput4` 0x21a788 (input 3 + impact).
- `Audio_SetVolumePreset` 0x21bca8: audio+0x3834 = preset, then one of six cases (jump table 0x39f0b0) sets the volumes;
  the sound volume menu's item 9 (`cSoundVolumeMenu::vf3`) and `cFE3DSound::vf40` call it, the constructor with 0.
- `Music_CancelTricky` 0x21c6c0: clears the Tricky rider (+0x6e7c) and `Music_StopTricky`; from `Music_UpdateMeterCues`.
- `Audio_SetVoiceVolume` 0x221000 (handle, volume): the voice table (+0x372c, 400 bytes a bank) slot's +0x24, through
  `VoiceTable_SetSlotVolume` 0x21fc68 (only when the slot's +0x20 is set); `WorldEmitter_Update` passes v × 127.
- `Song_MusicEvent` 0x224458 / `Song_SetSection` 0x224470: `FUN_002c15c8`(song, event, 0 / 1); `Song_SetPathVar`
  0x224488 jumps to `FUN_002c1288`; `Song_GetStreamStatus` 0x224790; `SongBank_PickVariant` 0x224658 (`fxrand.rs`).
- `StreamSlot_SetVolume` 0x223b70 is empty (returns 0). `Audio_PlayTextCursorSound` 0x214dd0: `PlayFESound`(bank 0xd,
  sound 0x10), from `Font_DrawWithCursor`. `BankInstance_DistanceVolume` 0x20ab28 wraps `SndVoice_DistanceVolume`.
  `Sfx_OnGrabHeld` 0x218e20 (empty; each tick a grab clip holds, `SpinState_UpdateTrickClips`). `Debug_Printf2`
  0x188990 is a second empty printf (bank monitor and speech messages).
- Still unclear: 0x212270 / 0x21e688 set / get audio+0x3844 (profile +0x1674, the presets set it; `Sfx_SpinTrickStart`
  reads it); 0x21e498 (schedules event 0xfa, callback 0x21e4c0); 0x225948 (music+0xfc = 0, from `Music_CancelBreak`).
