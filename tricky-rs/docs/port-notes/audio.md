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

## Names from F1c batch 15 (proven from the code, live Ghidra)

- Stream state (`SndHandle_GetState` 0x2d2c10 fills four words: state 0 waiting, 1 queued, 2 playing, 3 done or no
  request; the ms played, req+0x14 × 1000 / rate; the ms left, (req+0x18 − req+0x14) × 1000 / rate; req+0x1c in ms):
  `Stream_GetPlayedMs` 0x229530 and `Stream_GetTimeLeftMs` 0x229578 (0 unless playing), `Stream_IsQueued` 0x2295f8
  (state 1), `StreamSys_GetSlotTimeLeftMs` 0x229fc8 and `StreamSys_IsSlotQueued` 0x22a088 (the slot table at +0xa50),
  `SndHandle_IsDone` 0x22a0c8 (state 3 or an error), `SndHandle_IsPlaying` 0x22a120 (state 2, no error).
  `SpeechSys_IsIdle` counts a playing slot as idle when under 500 ms are left.
- Speech slots: `SpeechSlot_IsPlaying` 0x228700, `SpeechSys_IsSlotPlaying` 0x228dd0 (slot table +0x3718),
  `SpeechSys_SetSlotHold` 0x228bb0 (+0x34c, the big-score hold above). `SndPlayOpts_InitCurrent` 0x227708
  (`SndPlayOpts_Init` on the entry +0x24 + [+0x20] × 0x18).
- World trigger manager (0x34370c, 40 emitter records of 0x50): `WorldTriggerMgr_Destroy` 0x22be88 (no callers found;
  deletes it and clears the pointer) and `WorldTriggerMgr_Destruct` 0x22d168, `WorldEmitter_Construct` 0x22bec0 (the
  record's defaults; +0x2c = 1.0, +0x30 and +0x40 = −1), `WorldTriggerMgr_ToBankProgram` 0x22c610
  (`SoundId_ToBankProgram` on the manager), `WorldEmitter_IsSpecialKey` 0x22c638 (keys 0x61..0x63),
  `WorldTriggerMgr_StopKey` 0x22d480 (stops every active record with that key), `WorldTriggerMgr_OtherKeyPlaying`
  0x22dc38 (an active record with a voice, +0x40 ≠ −1, of another key), `WorldTriggerMgr_HasSameSound` 0x22dc88 (another
  active record with the same +4, +8). These are the calls `worldemitter.rs` lists by address.
- `Speech_GetFEModeMask` 0x22e550 (gApp+0x58: 0/1 → 1, 2 → 2, 3 → 8, 4 → 4, else 1; for `Speech_PlayFELine`).
  `Speech_OnTimeUp` 0x234058 (`cRaceHandler::Update`, modes 3 and 5: when the clock runs out with a rider unfinished,
  rider+0x418 < 0; roll 0xd, not mode 6, not in a replay: 0x1002037). `Sfx_OnClipPlay` 0x235778 is empty (the clip's
  type, from `AnimCtl_PlayClip`).
- Still unclear: 0x22e1b8 / 0x22e240 (four values from 0x22baf8 tested for 0x1c, 0x10, 0x39), 0x22e388 / 0x22e3f0 (id
  ranges for `SoundId_ToBankProgram`), 0x233ab0 (jump table 0x3a2e30), 0x235e70.

## The speech system's glue (G10c2, `game/src/speechsys.rs`; checked: `tests/speechsys.rs`)

- `cSpeechSys` is the stream system (`MusicSys_Init` builds it) with two speech slots: `SpeechSys_Init` 0x228728 sets
  `gSpeechSys` 0x343700 to it and makes the slots' table (+0x3718, two words); `SpeechSys_AddBank` 0x228a00 makes a
  slot (0x36c bytes) through `SpeechBank_Init` 0x227cb8, which opens its stream (`StreamSys_Open`, 0x20000) and loads
  its events (`Speech_LoadEvents`). A slot holds its stream (+0), the stream it follows (+4, −1 none) and its banks
  (+0xec count, +0xf0 list; a bank's +0 is its id). `SpeechSys_FreeBanks` / `_FreeAll` undo it, taking the slot's
  banks off the loaded count (0x343704).
- `Speech_InitLibrary` 0x228830 starts the speech library (`SpeechLib_Init` 0x2baf38) with a tenth of a second of
  22050 Hz 16-bit sound as its buffer (0x2bae08: rate × width bits in bytes, a tenth for kind 1, two sevenths for
  kind 2) and gives it 300 event slots.
- `SpeechSys_UpdateAll` services each slot whose stream is idle (`SpeechSys_IsIdle`: no stream, or playing with
  under 500 ms left).
- `Speech_PlayEvent` 0x228400 finds the first bank with the event's id and has the stream system play the line
  (named after the bank). A slot that follows another stream (+0x348 set) times its line to that stream's end: the
  line starts at once and is scheduled (`SndSys_ScheduleEvent`) for the other's time left less 30 ms, plus its play
  time when that is under 300 ms. While the slot's own stream is busy this holds only when it is playing; with 300
  ms or more left on the other, the line is played as usual unless the two times left are 40 ms or more apart, and
  the play time is then added only when the other's time less 30 ms is under the slot's own.
- `Speech_PlayFELine` 0x22e5a8 posts a front-end line (two words from 0x343710, the game mode's bit from
  `Speech_GetFEModeMask`: 1, 2 race, 8 mode 3, 4 mode 4) and services the slots. `Speech_FreeNarration`,
  `SpeechSys_IsCharScriptReady` (1 unless a script's slot has a stream, then `StreamSys_HasReached` at 500 ms), the
  stream getters (state, time left, play time, playing; 0 or 0.0 with no stream) and the library's fatal stop
  0x2bebe8 are in the same module.
- Checked in the runner on 40,000 random systems over all 23 entry points, every call out logged with the same
  answers on both sides; 26 of 28 mutants caught (the two left cannot happen: equal times are 30 ms apart, and
  nothing the slot calls changes its bank count).

## The music system's song objects (G10b2a, `game/src/musicsong.rs`; checked: `tests/musicsong.rs`)

- A song (0x7134 bytes) is made by `Music_CreatePathfinder` 0x223ca8 and `Music_LoadPathfinder` 0x223d60 from its
  `MUSIC.INF` section: the defaults (BPM 120, 4 beats a bar, 2 bars, ...), a 0x37000-byte stream buffer, then the
  section read with the ini reader at the music system's +0xa54 while tokens last: PATHDATA (+0x18), MUSDATA (+0x38),
  BPM (+0x100, a float), eleven numbers (+0x104..+0x130), and LOOPDATA banks (+0x58, 0x20 each, counted at +0xf8). A
  token naming a note length ("Sixteenth", "Eighth", "Quarter", "Whole"; `stricmp`, so case does not matter) sets
  +0x124 to that note's milliseconds at the BPM. The graph (base path + PATHDATA) is loaded into +0x134 and registered
  with Pathfinder; the music system's volume for the song (its table at +0x3c, index +0x20) is scaled by the song's
  percent (+0x12c) and held to 0..127, the track record's byte (+0x24 + 0x18 · index) set from it; the track is
  loaded (`PathX_LoadSong`: base path + MUSDATA, latency 500, the record, 2, 0x96, the buffer) and its id kept at +0.
- `Song_Pause` / `_Resume` set the stream's pitch (0 / 0x1000); `_MusicEvent` / `_SetSection` post an event (priority
  0 / 1); `_SetPathVar` and 0x224490 set Pathfinder's variable and the track's latency; `_UpdateVolume` (not while
  fading) gives the stream volume byte × percent (× the scale) × the master volume (0x343058), held to 0..127;
  `_FadeOut` fades over the seconds × 1000 and sets +0x14; `Song_Delete` cancels the bank, stops the track and frees
  the buffer. The song's stream is the word at 0x343158 + 0x3c · its Pathfinder slot.
- Timing: a beat is 60 / BPM × 1000 ms, the song's length beat × beats × bars; the play position and length come from
  the stream handle's state when it plays (state 2); `Song_MsToNextBeat` is a beat less `fmodf`(position, beat),
  `Song_CurrentBeat` the position over a beat. 0x2249c0 says whether the song's bank is ready.
- Checked in the runner on 30,000 random songs, music systems and ini sections over every function (the ini's answers,
  upper- and mixed-case tokens, allocations failing); 18 of 18 mutants caught.

## The music system's song control (G10b2b, `game/src/musicsys.rs`; checked: `tests/musicsys.rs`)

- `cMusicSys` keeps one song (+0x36f0). `MusicSys_PlaySong` 0x224e68 opens a new stream index (+0x20 up one: its
  record copied from the settings at +8, its tables cleared, its volume the record's first byte, the defaults 100,
  90, 50, 1, the volume scale) around `MusicSys_StartSong` 0x225090, which stops the old song, scales the stream's
  volume (held to 0..127, the record's byte from it), clears the front end's event and creates the song
  (`Music_CreatePathfinder` with the base path +0x1260, the name or the chosen song's +0x12e0 + 0x20 · +0x36e8, the
  volume as it was before the scale), loads its LOOPDATA bank and marks it playing. `MusicSys_StopSong` cancels its
  three scheduled events, its bank, resumes and deletes it.
- PlaySong's store at +0x28 + 4 · index reaches the table pointers from +0x38 on for an index of 4 or more; the game
  read them first, and the port keeps the machine code's order of loads and stores so it does the same.
- Events: `Audio_MusicEvent` / `MusicSys_SetSection` pass an event to the song (the end, 10, only while playing,
  and it stops it playing); `Music_SetPathVar`, 0x225408 (latency), `Audio_SetSongVolume`, 0x225448 / 0x225c30
  (fade out, 4 s), `Audio_UpdateSongVolume`, `MusicSys_StopCurrent` / `Audio_ResumeSong` (pause, resume) forward to the
  song; the front end's `Music_SetFEEvent0..5` post their event when it changes (+0x3714).
- `MusicSys_FadeTo` sets a fade (target, the volume now, a step of a twenty-fifth of the way over the seconds) and
  schedules its step (40 ms); the step 0x2254f8 moves the volume, writes it truncated to the song's volume byte while
  short of the target (rising or falling), else the target's (done).
- Breaks and air loops: `Music_ScheduleBreak` (a song not in a break; with Tricky on only when asked), its cancel,
  `Music_ScheduleAirLoop` (at the next beat: the song, the loop program from `Song_RandomSegmentMs` with the phrase
  +0x3704 and Tricky +0x3708, the next beat's number, a beat's ms), `Music_TryScheduleAirLoop` by the bank's state,
  0x2259d0 (the bank reloaded unless loaded), `Music_StartTricky` / `_StopTricky`.
- The song list: `MusicSys_CountEnabled` (+0x36ec's bits), `_SelectSong`, `Music_PrevSong`, `_NextEnabledSong`
  (round the +0x36e0 songs to an enabled one), `Music_CountEnabledSongs` (those among the first +0x36e8).
- Checked in the runner on 40,000 random systems over all 33 functions with the song functions running as the
  original underneath; 20 of 20 mutants caught.

## The race songs on the game's Pathfinder (G10b2c1, `game/src/pfplay.rs`, `src/pathmusic.rs`; checked: `tests/pfplay.rs`)

- With the disc, each race song runs on `tricky_game::pf` itself: its `.mpf` graph read from MUSIC.BIG (MUSIC.INF's
  BASEPATH `|data\audio\` + the song's name, with the game's archives mounted), registered, and its track loaded as
  `Music_LoadPathfinder` calls `PathX_LoadSong` (song 0, track 0, latency 500, ring 2, 0x96, the track record in a
  0x37000-byte buffer); the start event (0) and the finish (10) posted as `Song_MusicEvent` does, the path variable as
  `Song_SetPathVar`, and the engine's `update` run every frame (the game runs it as a poll task, `Sys_AddPollTask`).
  The engine's memory starts with no song selected (the byte at 0x3495d3 is 0xff in the game's data).
- The stream driver beneath is the port's (STANDIN for the PS2's `StreamSlot_*` / EA's `SndStream_*`), built on what
  the EE side asks: a play queues a request at the segment's offset (purging first when immediate), held when its
  start is −1 until released; `StreamSlot_IsDone` means at most one request outstanding and read in full (so the
  engine queues the next once the one playing is past the latency, and it follows sample-exact); the position is the
  request playing's ms; the clock is the frames played. The decoded segments (the tools' `song.ogg`) are found by the
  graph's own offsets (`Pf::node_segment`, `segment_offset`).
- Checked headless on bassinvader and kingbeat (`tests/pfplay.rs`): every request follows the last with no gap; the
  segments played match the port's earlier walk (the first 40 of bassinvader and the 181 in all, with the finish at
  300 s). In the app the first segment starts at the latency (0.5 s) and the rest follow every bar.
- Without the disc the earlier walk of `pathmusic.rs` plays (STANDIN). The music system's layers, its scheduler and
  the air loops on it are row G10b2c2.

## The sound system's scheduler (G10b2c2a, `game/src/sndevents.rs`; checked: `tests/sndevents.rs`)

- `cMonitor` (part of the audio system) keeps 48 events of 0x2c bytes from +0x90: in use, a period, a countdown, the
  callback as a doubleword (a this-adjust, a vtable index or −1 for a direct call, the function or the object's
  vtable field) and five words. `SndSys_ScheduleEvent` takes the free event at the scheduling depth (+0x88, so a
  callback that schedules gets the next free one) and sets it (the delay as both period and countdown);
  `SndSys_CancelEvents` clears every event with the same vtable index and, for a direct call or a differing low half,
  the same function word.
- Every 10 ms (`cController::vf4` 0x21eb10, unless paused at +0x8c) the tick count (+0x80) goes up and each event's
  countdown drops by 10; under 5 its callback runs with the adjusted object and the event, a nonzero answer clearing
  it, zero rearming it by its period. The music system's events are direct callbacks: the break 0x225958, the air loop
  0x225850, the bank reload 0x2259d0, the fade step 0x2254f8 (the doublewords at 0x3a0090..0x3a00a8).
- Checked in the runner on 30,000 random event tables over every function; 14 of 14 mutants caught.

## The race songs on the game's music system (G10b2c2b, `game/src/musichost.rs`, `src/pathmusic.rs`; checked: `tests/musichost.rs`)

- With the disc the app's race song runs through the ported music system: a `cMusicSys` in the engine's memory
  (MUSIC.INF's base path, song names and enabled songs, as `MusicSys_Init` leaves them; the stream index none yet; the
  stream settings' volume 0x7f; the music volume's slot 1.0), and `Audio_StartRaceMusic` 0x215220 as the game calls
  it: `MusicSys_PlaySong` with the chosen song (the app's pick, found by its PATHDATA), the bank (the enabled songs'
  count + 8) and the music volume's slot (`SndMgr_GetSlot` 0x211f38: +0x3850 + 0xc · 1), then the section 0 and the
  path variable 80. The song's section is read by the game's loader through the game's ini reader (BPM, the beats,
  PathLevel ...), its graph loaded from MUSIC.BIG and registered, its track loaded.
- Every frame: `Music_SetPathVar`, `Audio_UpdateSongVolume` (the stream volume: 0x7f × PathLevel × the music volume ×
  the master; the app's own sink no longer applies PathLevel), the scheduler's 10 ms ticks (the break, the bank reload
  and the fade step fire as the game's callbacks), and Pathfinder's update; the finish is `Audio_MusicEvent` 10.
- The host answers the layers' calls: a bump heap, `memset`, `sprintf` ("%s%s"), `stricmp`, the ini reader with its
  token buffer in memory (the loader compares the note names against that buffer after its reads, so "DelaySwitch =
  EIGHTH" sets the delay from the token two reads on, as in the game), the disc's files, Pathfinder, the driver
  (volume, fade, pitch as pause, status and handle state) and the scheduler. STANDIN: the banks (LOOPDATA) are not
  loaded (row G10b2c2c).
- Checked headless on Bassinvaders: the section's numbers as MUSIC.INF gives them (BPM 142, 4, 2, PathLevel 90,
  AsyncLevel 90), 181 segments with no gap through the finish at 300 s, the stream volume 114 (127 × 0.9). In the
  app the first segment starts at the latency and smartbomb's stream volume is 101 (PathLevel 80).

## The air loop's beat on the game's code (G10b2c2c1, `game/src/musicsys.rs`, `game/src/musichost.rs`; checked: `tests/musicsys.rs`, `tests/musichost.rs`)

- The chain:
  - A break (`Music_ScheduleBreak`) fires the break event. Its callback `Music_TryScheduleAirLoop` schedules the
    air loop at the song's next beat once the song's LOOPDATA bank is loaded.
  - The air-loop event's callback is 0x225850. It calls the audio object's virtual at +0x74 (this adjusted by
    +0x70) and passes the event on in a1; the port dropped the event until now. On the game's object that virtual
    is `cBXAudio::vf13` 0x20df08 (`air_loop_beat`).
  - The event's words are: +0x18 the song, +0x1c the loop's first program, +0x20 the beat, +0x24 a beat's ms.
- `air_loop_beat`:
  - **Past the phrase's beats** (song +0x114 BeatsPerPhrase): the beat goes back to 0, and the break flag +0x3710
    is cleared with Tricky on. The exception is Tricky on with no break asked and the word at 0x34308c clear; then
    `Music_CancelTricky` 0x21c6c0 runs (+0x6e7c cleared, `Music_StopTricky`) and nothing plays.
  - **Otherwise:** the beat is brought into the phrase (negative beats wrap) and a sound is queued:
    - It goes in the next stream index, set up in the same order of loads and stores as `MusicSys_PlaySong`.
    - Its kind (+0xa40) is the song's bank and its id (+0xa44) is the program plus the beat.
    - With Tricky on, its volume is the Tricky volume +0x6e84. It is then scaled by AsyncLevel (+0x130) / 100,
      truncated and held to 0..127.
    - Its slot is +0x389c. 0x224a50 copies the song's DelayCount, DelayFeedback and DelayTime (+0x11c, +0x120,
      +0x124) into words 1, 2 and 0 of its effect row.
  - Then `SndSys_PlayPending` runs, the beat goes up one, and the event's period becomes a beat's ms.
- `Song_RandomSegmentMs` 0x2246a8 picks the loop's first program, as a phrase × PhraseAlign (+0x118):
  - With Tricky: the bank's last phrase, PhrasesPerbank (+0x10c) − 1.
  - Otherwise: a random number's unsigned remainder by PhrasesPerbank − 1, or, with the phrase set +0x3704, by
    +0x110 counted after the bank's phrases.
- 0x224658 picks which LOOPDATA bank to load: a random number's remainder by +0xf8.
- Both take their random number from the game's random object (`[0x3436f8]`, its virtual +0x24 adjusted by +0x20).
- Checked in the runner inside the musicsys test (40,000 random systems; queue indices up to 5, whose +0x28 stores
  reach the table pointers).
- Mutants: 16 of 21 are caught (all 5 on the phrase pick). The 5 that survive are equivalent while the tables do
  not overlap, as in the game:
  - the t64 index read again;
  - the < 127 guard on the first product;
  - < against <= 127 (127 truncates to 127 either way);
  - the +0x3c table pointer read again after the slot store;
  - the period taken from +0x24 at the end.
- The host:
  - It frees the scheduler's 48 events at start (`SndEvent_Init`). Before this none were free, so nothing could be
    scheduled.
  - It gives the audio object its vtable (+0x74 → vf13), a random object answered by its own generator, the kind
    and id tables, and room up to +0x8000.
  - It sets the loop slot +0x389c to 1.0.
  - It answers vf13, `Song_RandomSegmentMs` and the variant pick with the ported functions.
  - STANDIN for `BankManager_Load`: the bank is marked loaded (state 1) and its path kept (`loop_bank`).
  - STANDIN for `SndSys_PlayPending`: the queue's top sound is taken (volume times its slot, held to 0..127) and
    kept for the app (`take_loops`), without the play or the delay's echoes, and the index goes down one.
- Checked on Bassinvaders, from the disc (bank `basslpz.bnk`, 8 beats a phrase, 4 phrases, align 16, AsyncLevel 90):
  - Nothing plays before a break.
  - After one, a sound comes every beat (422 ms ± 10). The programs run from the song's next beat in a phrase of the
    first three, through 8 and round. The volume is 114.
  - The forced cancel stops it.
  - Tricky plays phrase 3 (programs 48..). Its volume is 0 here, because the Tricky volume +0x6e84 is set by the
    Tricky trigger, which is not ported (G10b2c2c2).

## The app's air loop on the music system (G10b2c2c2a, `src/pathmusic.rs`, `game/src/rider.rs`; checked: `tests/musicsys.rs`, `tests/musichost.rs`)

- `Audio_TrickyTrigger` 0x21c610 (`musicsys::tricky_trigger`) runs only when all of these hold: music is on
  (+0x3844), there is a song, it is playing (+0x3700), no replay is playing (`Replay_IsPlaying` 0x21bfd0), the
  rider is not finished (+0x418 negative) and it is the rider listened to (`Audio_GetListenerRider` 0x20ed20 with −1).
  It then:
  - keeps the rider in +0x6e7c;
  - sets the Tricky volume +0x6e84 to 0x7f, or 0x5a when +0x6e50 is set;
  - calls `Music_StartTricky` with the rider, then `Speech_OnTricky` 0x2337c8.
- Checked in the runner inside the musicsys test. 6 of 6 mutants are caught.
- The host's STANDINs for it: music on, no replay, its one rider record (only +0x418) as the listened rider, and the
  speech call answered with nothing. With the trigger, Bassinvaders' Tricky plays the bank's last phrase at 114
  (0x7f × AsyncLevel 90 %). It plays from the song's next beat to the phrase's end, then once through again (the
  break it asked clears +0x3710 at the first wrap), then `Music_CancelTricky` stops it.
- The break at take-off: the air motion's enter 0x108310 predicts the flight (`AirPredict_AtTakeoff` 0x123b10 on
  rider+0x57b0), then calls `Sfx_OnAirEnter` 0x2174f8. For the listened rider, that asks `Music_ScheduleBreak`(audio,
  0) when the predicted landing time is over 1.5 s. The prediction's landing time is +0x57f4, the prediction's +0x44.
  - `sndqueue::on_air_enter` already had this call, under the misleading name `crowd_clear`.
  - The port's rider now makes the prediction when it leaves the snow in `step` (`Rider::land_time`,
    `Rider::takeoffs`), with the course's rays `cWorldLine`(0, …, 0). STANDIN: in a levels folder with no course
    collision, it uses the drawn mesh's ground instead (as the wipe-out does).
- The app: with the disc's music system, `pathmusic` handles three things itself (the app's own beat-timed loops
  are kept only without the disc, as a STANDIN):
  - it asks the break on such a take-off;
  - it calls the Tricky trigger when the rider's Tricky starts;
  - it plays each sound the host queued: `loops_<program>.wav` at the music level × its volume / 127.
- Seen on gari (`TRICKY_LOOPLOG`, autoplay, 90 s): 33 take-offs. The first one predicted over 1.5 s (2.26 s,
  44 s in) starts the loop on the next beat. From then on it plays one program a beat (about 0.44 s apart on
  smartbomb) at 114, through the phrase and round, for the rest of the run, as the game's loop does.

## The queued sound's play and the delay's echoes (G10b2c2c2b, `game/src/sndplay.rs`; checked: `tests/musicsys.rs`, `tests/musichost.rs`)

- `SndSys_PlayPending` 0x20c948 (the system) plays the queue's top sound:
  - **Volume:**
    - A rider's sound (+0x28 set) gets 0.
    - Otherwise, with a slot (+0x40), the volume is multiplied by the slot's float, truncated and held to 0..127.
      With no slot it is left as it is.
    - The record's first byte gets the volume held to the byte.
  - **Play:** with the sound library locked, `SND_PlayBankSound` 0x2d1718 runs with the kind's bank handle (state
    1: its +4; else −1), the id and the record.
  - **Bank monitor:** a sound that played, or one that failed without +0x7c set, goes to `BankMonitor_AddInstance`
    0x20bd68 (the system, the sound, the volume before the scaling, 1). 0x20aba8 then runs on its answer, with the
    handle and the id.
  - **Echoes:** while the effect row (+0x74: DelayTime, the count, DelayFeedback %, DelayLevel %) has a count left,
    each echo is scheduled with the event at 0x39e870 (callback 0x20c680). Each lands DelayTime further on and
    carries the slot, the kind, the id and its volume. The first echo's volume is the volume × level % ×
    feedback %; each later one is the last × feedback % (integer divisions, held to 0..127). It stops when the
    count runs out or the volume reaches 0.
  - Finally the index goes down one, and the sound's handle is returned.
- The echo callback 0x20c680 (the event: +0x18 the slot, +0x1c the kind, +0x20 the id, +0x24 the volume) queues a
  sound on `gAudio` (0x342c48):
  - It is set up as `MusicSys_PlaySong`'s stream is, with those words; its effect row is the default (no echoes).
  - It then calls `SndSys_PlayPending` and returns 1, which clears the event.
- Checked in the runner inside the musicsys test, with the real `SndSys_PlayPending` now running under vf13 too.
  14 of 15 mutants are caught; the survivor (< 0x80 against < 0x7f in the clamp) gives 0x7f either way.
- **In the game's music, echoes never sound.** MUSIC.INF gives every race song a DelayCount (2 or 4) but
  `DELAYFEEDBACK = 0`, so the first echo's volume is 0 and nothing is scheduled. Two more details:
  - 0x224a50 copies the song's count, feedback and time into the row, but not its DelayLevel, so the row keeps
    its default 50.
  - The host test checks both: no echoes on Bassinvaders as it is. With the song's feedback set to 90, two echoes
    of the same program come 295 ms apart at 51 and 45 (114 × 50 % × 90 %, then × 90 %).
- The host now runs this function. The only STANDIN left is `SND_PlayBankSound`, which keeps each sound for the
  app (its record byte as the volume) instead of playing it. The sound library and the bank monitor's own
  functions are G10.

## Names from F1c batch 17 (proven)

- `WorldTriggerMgr_SetActive` 0x22e400 (was `FUN_0022e400` in `game/src/prerace.rs`): manager 0x34370c +0x443c =
  on; called with (1, 1) it schedules the switch instead.
- `Audio_ScheduleWorldTriggersOn` 0x21e498 (from `cPreRaceHandler` Enter / Update): `SndSys_ScheduleEvent` in 0xfa
  with callback 0x21e4c0 (not a function in Ghidra), which calls `WorldTriggerMgr_SetActive`(mgr, 1, 0).
  Settles 0x21e498 from the batch 14 list.
- `SnowProgram_Destruct` 0x21e738: frees the program pointer of a 0x50-byte snow program record (the arrays at
  cBXAudio +0x745c, +0x777c, +0x7a9c).

## Names from F1c batch 19 (proven)

- Crowd sets: `CrowdSoundList_Destruct` 0x21ece0 (`PtrArray_FreeAll`, delete on flag 1), `CrowdSoundSet_Destruct`
  0x221538 (its three lists at +0, +0x18, +0x30: trick, fall, anticipate); `~cCrowdSys` runs it over the three sets
  at +0x3730.
- The speech library (`game/src/speechlib.rs`, `speechplay.rs`, `speechsys.rs`; the Rust function in brackets):
  - tables: `SpeechLib_InitBankTable` 0x2bb060 (from Init) and `SpeechLib_ClearBankTable` 0x2bb078 (from Shutdown),
    the same body [`clear_bank_table`]; `SpeechLib_BankHasSound` 0x2bb270; `SpeechLib_FindEventInBank` 0x2bb610;
    `SpeechLib_GetEventBankWord` / `Kind` / `Size` 0x2bb700 / 0x2bb750 / 0x2bb7d0; `SpeechLib_FindBankKey`
    0x2bcfd8; `SpeechLib_LookupHeaderBanks` 0x2bc850.
  - a bank's history ring and bitmap: `SpeechBank_PushRecent` 0x2bb2d8, `SpeechBank_FindRecent` 0x2bb370,
    `SpeechBank_StirBitmap` 0x2bb430, `SpeechBank_ClearRowBit` 0x2bc938, `SpeechBank_TestEntryBit` 0x2bc9d8,
    `SpeechBank_RowAllowed` 0x2bca80, `SpeechBank_GetKinds` 0x2bc738 (bytes +8, +9).
  - voices: `SpeechLib_ResetVoices` 0x2bb810, `SpeechLib_GroupLimitsOk` 0x2bba50, `SpeechLib_StartVoice0`
    0x2bc700 (0x2bc5e0 for group 0), `SpeechVoice_ClearFlags` 0x2bdad0 (+0x60, +0x61).
  - checks: `SpeechLib_MasksAgree` 0x2bc7a0, `SpeechLib_MaskMatch` 0x2be650, `SpeechLib_ModeOk` 0x2bcf90,
    `SpeechLib_IdCountOk` 0x2bcf48 and `SpeechLib_ClearIdCounts` 0x2badc0 (the eight (id, count) pairs at 0x3494b0),
    `SpeechLib_RunCheckCallback` 0x2bc490 (0x348dec), `SpeechLib_Percent` 0x2bccd0 (of 0x3494f4),
    `SpeechLib_Weight` 0x2bcdb0 (0x348930).
  - the pending (id, value) at 0x348dc0: `SpeechLib_SetPending` 0x2bc750, `_GetPending` 0x2bc768, `_ClearPending`
    0x2bc788.
  - `SpeechIdList_Clear` 0x2bbd68, `SpeechIdList_Contains` 0x2bbd78 ({count, halfwords}); `SpeechRow_Clear` 0x2bea20;
    `SpeechLib_SetClockBase` 0x2be208 (0x349518, from Init); `SpeechLib_CalcBufferBytes` 0x2bae08 (from
    `Speech_InitLibrary`); `SpeechLib_Fatal` 0x2bebe8 (0x3495d3 = 'B', `exit(0)`).
- Not settled: 0x2bad70 (`REGISTER` in `speechload.rs`: 0x2bac78 for a slot below 8), 0x2baeb8 / 0x2baed0 (each
  clears three words, at 0x348df8 and 0x348de0; `speechlib.rs` calls 0x2baed0 "stop every voice", which its body
  does not show), 0x2bb090 (`v << 2`).

## Names from F1c batch 20 (proven)

Functions already ported under a bare address, named from the code and the Rust port.

- Pathfinder (`game/src/pf.rs`, checked in the function runner): the driver's callbacks handed over by `PathX_Init`
  0x223648: `Path_OnDriverUpdate` 0x2c0488, `Path_OnDriverBeat` 0x2c0898, `Path_OnDriverStop` 0x2bedb8. The walk:
  `Path_FindTrack` 0x2bedf0, `Path_ApplyTrackVolumes` 0x2bef08, `Path_GetSegmentOffset` 0x2befd8,
  `Path_TakeBranch` 0x2bf038, `Path_WalkToAudioNode` 0x2bf108, `Path_RouteValue` 0x2bf290, `Path_GetEventCell`
  0x2bf320, `Path_GetEventTarget` 0x2bf490, `Path_NextSegment` 0x2bf508, `Path_ReleaseWaiting` 0x2bfa78,
  `Path_AdvanceTrack` 0x2bfbb8, `Path_StartSegment` 0x2bfe28, `Path_SyncToLead` 0x2c00d0. The calls in:
  `PathX_GetTrackBytes` 0x2c1018, `PathX_SetLookahead` 0x2c1068, `PathX_SetTrackLatency` 0x2c1130,
  `PathX_GetTrackSlot` 0x2c1168, `PathX_GetPlaying` 0x2c1190, `PathX_SetVariable` 0x2c1288, `PathX_ClearEvents`
  0x2c1380, `PathX_GetQueuedEvents` 0x2c1430, `PathX_GetCurrentEvent` 0x2c1500, `PathX_PostEvent` 0x2c15c8,
  `PathX_JumpToNode` 0x2c1890, `PathX_SetPitch` 0x2c1b98. `PathDriver_GetTrackBytes` 0x2231a8: for n > 0,
  `SndStream_CalcMemSize` + 0x20000, else 0.
- Music: `Song_GetPlayingNode` 0x224768 (the first word `PathX_GetPlaying` answers), `Song_IsBankReady` 0x2249c0
  (1 unless +0xfc is clear and the bank (+0xa3c table, 0x60 each) is loading with its handle's state, 0x2d14d0,
  not 1), `MusicSys_CheckSongBank` 0x2259d0 (the bank not loaded: +0xfc cleared, the system's +0x24, and with its low
  two bits clear `MusicSys_LoadSongBank`).
- Sound events: `SndEvent_Tick` 0x21ea48 (in use: the countdown down 10, under 5 the callback, direct or by vtable
  index; a nonzero answer clears the event), `SndEvent_SetTimes` 0x222ab8 (+4 and +8 the same value; from
  `SndEvent_Set` and `SndEvent_Clear`). `BankInstance_SetHandle` 0x20aba8 (+0xc the handle, +0x10 the program; from
  `SndSys_PlayPending`). `SndQueue_StartLastVoice` 0x2212a0 (from `WorldEmitter_Update`: `CrowdBox_StartSound` on the
  last queued entry, the handle `<< 8 |` the entry, the count down one; -1 without a table at +0x372c).
- Speech: `SpeechSys_DrainCutsceneRing` 0x230ff8 (from `CamScript_Update`), `Speech_TriggerIndexedLine` 0x2314e0
  (0x4008010 with the word at 0x3a2750 + 4i and the mode mask). From `cBoarder::vf10` 0x11a348:
  `Speech_OnPlaceChange` 0x233ba0 (event bit 0xc: roll 6, the listener, more than one rider, 0x1002049 with 0/1/2 by
  the career's +0xa4 / +0x124), `Speech_OnEventBit13Or14` 0x234558 (roll 0xe, 0x1002051 with 2 for bit 0x14, else
  1), `Speech_OnEventBit15` 0x234610 (roll 0xf, 0x1002053), `Speech_OnEventBit16` 0x233740 (roll 0x12, the listener,
  0x1002056), and the event's sound `Audio_SetEventBitSound` 0x21c0a0. The bits' meaning is not known, so the names
  say the bit.
- Not settled: 0x233e10 (roll 9, 0x1002046 with 1; its one caller 0x219258 has no callers and fires it when a
  value at +0x18 → +0x1c passes 0.875; `speech.rs` calls it `on_boost_full`, unproven), 0x2129f8 and 0x212e98 (long
  music switches, from `cFE3DSound` vf9 0x270610 and `PauseMenu_UpdateOptionsPage`), 0x223a90 (0x2d2d50 on a slot
  unless its byte +0xe is set; from `Path_OnDriverUpdate`), 0x2d14d0 (the sound library's bank-handle state, prefix
  unknown).

## Names from F1c batch 24 (proven)

- `AsyncSys_PopNext` 0x209aa0: `AsyncSys_PopQueue(sys, 0)`; the flag, when set, empties all four queues as well.
- `BankInstance_SetStream` 0x20abb8 (+0xc the stream, +0x10 its name, +0x50; from the bank manager's vf14),
  `BankInstance_SetHandleCopy1` 0x20abf8 (the same code as `BankInstance_SetHandle`; from `MidiSys_PlayPending`).
- `BankInstance_SetPan` 0x20ac50: only when the pan moved by 0x200 or more (+0xac the last); by kind +0: 1 sound
  (`SndHandle_SetPan`), 2 MIDI (`SndMidi_SetPan`), 3 stream (`Stream_SetPan`). From `BankInstance_UpdatePositional`.
- `Stream_SetPan` 0x229348 (`SND_Lock`, `SndStream_SetPan` on +4), `SndStream_SetPan` 0x2d2eb8 (stores the two
  values at +0xfc/+0xfe and calls `SndHandle_SetPan` on its handle; −8 without a stream).
- `SpuAlloc_PopRecord` 0x2266a8: takes a record (0x18 bytes from +0xc) off the sound manager's free list (+0x310,
  link +0x14); 0 when empty. From `cPS2SoundManager` vf3, the sound-memory allocator.

Still unclear: 0x20a268 (bank manager vf11's record setup), 0x21e7e0 / 0x2215a8 (`SND_Lock` around 0x2d6a18, which
sets each voice's byte +0x2b and recalculates its pitch), 0x229658 / 0x2296b0 / 0x22a1e0 (0x2d2d50 sets a stream
request's +0x28), 0x228b88 / 0x228c80 (speech slot +0x348; four triggers with 0x343708 set), 0x2168f8, 0x222ae0,
0x223128, 0x2248f8, 0x226e58, 0x2c2040; the front end's music cues 0x262ad0..0x262ca0 are calls to `cFE3DSound`
vf12..vf15 and vf20, whose meaning is still open.

## Names from F1c batch 25 (proven)

EA's sound library (the `SNDAUTHOR Dave Mercier` one, 0x2cf000..0x2e5000), named from its code and callers.

- Setup: `SND_GetConfig` 0x2cfeb0 (the first call fills the defaults with `SND_SetDefaultConfig`; then copies the
  0x108-byte config at 0x350fb8 out), `SND_SetConfig` 0x2d0080 (copies it back in). `SND_Init` calls both.
  `SND_BuildPanTable` 0x2d6618 (from `SND_Init`: the pan table at 0x351398, rows of 6 levels by speaker mode
  0x350ff3, through `Snd_PanCurve` 0x2d65f0 = 2x − x²/127), `SND_GetPanLevels` 0x2d68a8 (the row for pan >> 8, each
  level × 0x102). `SND_SetMasterVolume` 0x2d2f90 (0x351191, 0x7f at init; recalculates every playing voice).
- Ticks: `SND_Tick` 0x2d0698 (the default tick at 0x3511a0: counter 0x35119c + 1, then the `SND_AddTickCallback`
  list), `SND_RunTicks` 0x2d05a0 (from `SND_MixThread`: runs the tick until the counter catches up with
  calls × 100 / rate). `SND_StartStreamPoll` 0x2d0548 adds `SND_StreamPollTask` 0x2d0520 (`SND_RunStreamTasks`)
  as a poll task once.
- Memory: `SndMem_Alloc` 0x2d0b98 (16-byte aligned, from the heap at 0x351210), `SndMem_ClampSize` 0x2d0ac0.
  Sound RAM: `SpuMem_Alloc` 0x2de640 / `SpuMem_Free` 0x2de848 (kind 0x100 goes to `sceSifAllocIopHeap` /
  `sceSifFreeIopHeap`; otherwise 64-byte blocks in a sorted table at 0x353c80), `SpuMem_GetLargestFree` 0x2de998
  (returns the size, writes the address), `SpuMem_ClampRange` 0x2de5c8 (to 0x353c7a..0x353c7c). Thunks:
  `SND_SetSpuRange` 0x2d0a80 (`SpuMem_SetRange`), `SND_GetLargestSpuFree` 0x2d0aa0.
- Transfers to the IOP: `SndXfer_Queue` 0x2de108 (a 28-entry queue at 0x353328, 16 bytes each, with a priority byte
  +0xe; kind 0x100 is sent at once with `sceSifSetDma`), `SndXfer_Upload` 0x2de258 (in 0x1000-byte pieces),
  `SndXfer_Kick` 0x2de088, `SndXfer_StartNext` 0x2ddeb8 (highest priority, then lowest id: `sceSifSetDma` and
  `sceSifSendCmd`), `SndXfer_IsDone` 0x2de368, `SndXfer_OnDone` 0x2de3b8; `SND_OnIopReply` 0x2db690 (the IOP's reply:
  kind 2 = a transfer done, kind 0 = an RPC answer, signals the semaphore). Thunks `SND_Upload` 0x2dddc8,
  `SND_IsUploadDone` 0x2ddde8.
- Banks (slots of 0x10 at 0x3511fc): `SndBank_Register` 0x2d0f98 (from `BankSlot_Load`, `PathX_LoadBank`,
  `AsyncSys_Update`: uploads the bank's sample data, flag +0xc while loading), `SndBank_Unregister` 0x2d1200 (−1 =
  all; stops its voices first), `SndBank_Check` 0x2d13d8 (−8 bad slot, −18 still loading),
  `SndBank_GetResidentSize` 0x2d1428 (header +8), `SndBank_Relocate` 0x2d1448 (copies that part elsewhere and
  points the slot at it), `SndBank_IsPlaying` 0x2d14d0 (1 while a voice of the bank, voice +0x12, is alive).
  `musicsong.rs` calls 0x2d14d0 `SND_HANDLE_STATE`; the value it reads is this one (Rust left unchanged).
- Streams: `SndStream_Create` 0x2d24e0 (behind `SndStream_Open`; slots at 0x350768), `SndStream_CalcSize` 0x2d2a88
  (n × 0x2c + 0x124 + the voice's size), `SndStream_UpdateAll` 0x2d2270, `SndStream_ParseChunk` 0x2d20c8 (EA's
  chunks: `SCHl` → `SndStream_OnHeaderChunk` 0x2d1b70, `SCCl` → `SndStream_OnCountChunk` 0x2d1e30, `SCDl` →
  `SndStream_OnDataChunk` 0x2d1e88, `SCEl` → `SndStream_OnEndChunk` 0x2d2090), `SndStream_StartVoice` 0x2d17b0,
  `SndStream_RemoveQueued` 0x2d1960, `SndStream_ConsumeBytes` 0x2d1ab8, `SndFormat_BytesPerSecond` 0x2d1840 (rate
  × channels × bytes per sample × 256: 0x92 for codec 5 = PS2 ADPCM's 16 bytes per 28 samples, 0x200 for 8 = PCM16,
  0x88 for 10, 0x33 for 4), `SndStream_QueueHandle` 0x2d2db0 (`SndStream_Queue` mode 2: an existing request),
  `SndStream_SetLowWater` 0x2d2f50.
  - The prebuffer: `SndStream_Queue`'s second argument goes to the request's +0x28; `SndStream_IsPrebuffering`
    0x2d2178 holds the request while its buffered time (bytes × 1000 / rate) is under that many ms, then clears
    it. `SndStream_SetPrebufferMs` 0x2d2d50 sets it. Its callers, unclear in batch 24: `Stream_ClearPrebuffer`
    0x229658 (0 on the current request), `Stream_ClearRequestPrebuffer` 0x2296b0, `StreamSys_ClearRequestPrebuffer`
    0x22a1e0 (from `Sfx_PlayClipSound`), `PathX_SetTrackPrebuffer` 0x223a90 (its a1, unless the track's byte +0x16
    is set; `pf.rs` calls it with 0).
  - The stream's voice (table 0x358c38): `SndStreamVoice_CalcBaseSize` 0x2df238 (n × 0x20 + 0x7c),
    `_CalcMemSize` 0x2df248, `_Create` 0x2df278, `_Play` 0x2df3b0, `_AddBlock` 0x2df6f8, `_GetFreeBlocks` 0x2df840,
    `_GetQueuedBytes` 0x2df878, `_Stop` 0x2df8c8, `_Destroy` 0x2df978.
- MIDI: `SndMidi_RunEvents` 0x2d3f00 (from `SndMidi_Tick`; by status byte: 0x80 `SndMidi_NoteOff` 0x2d3b90, 0x90
  `SndMidi_NoteOn` 0x2d39c8, 0xb0 `SndMidi_OnController` 0x2d3c30, 0xc0 the program byte, 0xe0
  `SndMidi_OnPitchBend` 0x2d3e18, meta 0x2f end of track: `SndMidi_Stop` once `SndMidi_VoicesDone` 0x2d3938).
- Voices (0x88 each at 0x3511f8; each channel's work is done by the software mixer below):
  `SndVoice_StopChannels` 0x2dc340, `SndVoice_UpdatePitch` 0x2dc4e8, `SndVoice_UpdateVolume` 0x2dc8e8,
  `SndVoice_UpdatePan` 0x2dd560 (from `SndHandle_SetPan`), `SndVoice_UpdateFxSend` 0x2dc6f0,
  `SndVoice_CalcFxSend` 0x2dfef0 (+0x5c × +0x5d × the bus level at 0x351204/0x351205/0x351206 >> 14 into +0x5e/+0x5f),
  `SndHandle_SetFxSend` 0x2dfcf0, `SndVoice_SetLowpassHz` 0x2dcdf0 (Hz / (rate / 2)), `SndVoice_GetPosition`
  0x2dde08, `SndVoice_AddFx` 0x2deb38 / `SndVoice_RemoveFx` 0x2debf8 and their `SndHandle_` loops 0x2d3718 /
  0x2d3780. `SndPlayParams_SetDefaults` 0x2dfe78 (volume 0x7f, pan 0x40, +0x10 = 8), `Snd_RandRange` 0x2dff90
  (random ±x, x clamped to 0..0x10000).
- Software mixer (channels of 100 bytes at 0x358e80): `SoftMix_Init` 0x2e0348 (from `SND_LoadIopDriver`),
  `SoftMix_Shutdown` 0x2e0538 (from `SND_StopDriver`), `SoftMix_Mix` 0x2e09e8 (from `SND_MixThread`),
  `SoftVoice_Start` 0x2e0640 (picks the decoder by codec), `_Stop` 0x2e08e0, `_SetVolume` 0x2e0980, `_SetFxSend`
  0x2e1000, `_GetPosition` 0x2e1030, `_SetPitch` 0x2e1078 (adds a resampler, id 0xa0), `_SetLowpass` 0x2e12a0 (adds
  a one-pole filter, id 0x28, while the value is under 1.0), `_AddFx` 0x2e1360, `_RemoveFx` 0x2e13f8 (by id).
  `SndMix_ClampToPcm16` 0x2e3b2c (>> 7, clamped). Effects are pulled through `SndFxChain_*`: `SndFx_PcmOut_Init` /
  `_Pull` 0x2e3a48 / 0x2e39d0 (`Snd_FloatToPcm16`), `SndFx_Lowpass_Init` / `_Pull` / `_SetCoef` 0x2e3cc0 / 0x2e3c00
  / 0x2e3c90, `SndFx_Fir_Init` / `_Pull` / `_Process` / `_Design` 0x2e3e08 / 0x2e4280 / 0x2e3ce8 / 0x2e3fe0 (9 taps,
  symmetric; cut-off 2f / rate, at most 0.8; `_DesignUnity` 0x2e4238 gain 1.0, `_DesignGain` 0x2e4260),
  `SndFx_Resample_Init` / `_Pull` / `_Interp` / `_SetStep` 0x2e4a78 / 0x2e46e8 / 0x2e45a0 / 0x2e46c8 (16.16 steps).
  Maths: `SndMath_Sin` 0x2e3e40 / `SndMath_Cos` 0x2e3f10 (Taylor series, angle wrapped at 2π), `SndMath_ISqrt`
  0x2e4bd8. `SndBits_Write` 0x2d6c08 (the writer beside `SndBits_Read`).

Still unclear: 0x2bb090 (x << 2), 0x2cfe90 (`printf` of a1, no callers), 0x2d0e98 (runs the callbacks at 0x3511d4
for each of a record's pending items), 0x2d1570, 0x2d1680 / 0x2dd990 (bank layer fix-ups), 0x2d1a80, 0x2d2dd8
(`SndStream_Create` with a1..a5 moved and 1), 0x2d3358 (largest time left of a handle's voices), 0x2d3548 /
0x2d35f8 / 0x2dd188 / 0x2dd0d0 / 0x2dd278 / 0x2dd388 / 0x2dd3f0 (effect-bus setup, flags 0x10/0x20/0x40),
0x2d4ab0 / 0x2d4ae8 (voice picking), 0x2d4f58 / 0x2dacb0 (a voice's end), 0x2d59d8 / 0x2d5e18 / 0x2d60b0 (playing a
bank sound), 0x2d6910, 0x2d6a18 (voice +0x2b), 0x2dfda0 (from `SndMidi_NoteOff`), 0x2dd9c8, 0x2dcc88 / 0x2dcd40 /
0x2dced0 / 0x2e1138 / 0x2e11e8 / 0x2e4300 / 0x2e4480 / 0x2e4500 (two more per-channel effects), 0x2def00, 0x2df1d8,
0x2df8c0 (returns −15), 0x2df9b8 / 0x2dfc08, 0x2dffe8, 0x2e00e0, 0x2e0100, 0x2e0610, 0x2e0630, 0x2e0828, 0x2e08a8,
0x2e0c10 / 0x2e0c40 / 0x2e0c90 / 0x2e0ff0 (an effect with a 2000-tick timeout), 0x2e1130 and 0x2dc338 (empty), the
three decoders installed by 0x2e1530 / 0x2e1578 / 0x2e15c0 (0x2e1600..0x2e2848, 0x2e28c8, 0x2e2c50..0x2e3620), the
VU0 audio codec 0x2d6ad0..0x2da8d8 (0x2d6ad0 installs its three callbacks at 0x353d40), 0x2daee8, 0x2db3b8,
0x2db738.

## Names from F1c batch 26 (proven)

Read in Ghidra (`SLUS_203.26`) and checked against the ported Rust code where there is some.

- **Speech library voice and row logic** (`game/src/speechplay.rs`, which named them only by address and Rust
  function): `SpeechLib_FindVictimVoice` 0x2bbdd0, `SpeechLib_PickVoice` 0x2bc048, `SpeechLib_ClaimVoice` 0x2bc320,
  `SpeechLib_RunGroup` 0x2bc4c0 (no callers), `SpeechLib_StartVoice` 0x2bc5e0 (from `SpeechLib_Service` and
  `SpeechLib_StartVoice0`), `SpeechLib_CollectRows` 0x2bcb30, `SpeechLib_WeightSum` 0x2bcd08, `SpeechLib_OrderRows`
  0x2bcdd8, `SpeechLib_BuildEvent` 0x2bd078, `SpeechLib_ShuffleRows` 0x2bd218, `SpeechLib_NextCombo` 0x2bd2e8,
  `SpeechLib_FindCombo` 0x2bd370, `SpeechLib_SelectRow` 0x2bd430, `SpeechLib_SelectRows` 0x2bd6a8,
  `SpeechLib_FireParams` 0x2bd798 (callback 0x348de8), `SpeechLib_PlayRows` 0x2bd928 (callback 0x348de0),
  `SpeechLib_RunQueue` 0x2bdc98, `SpeechLib_ParamTest` 0x2bddf0, `SpeechLib_StartEvent` 0x2bde40,
  `SpeechLib_StartVoiceEvent` 0x2be140 (callback 0x348df0), `SpeechLib_HasValue` 0x2be218, `SpeechLib_GetRecord`
  0x2be2b0, `SpeechLib_RecordCallback` 0x2be310, `SpeechLib_ParamMasks` 0x2be468, `SpeechLib_GetRowInfo` 0x2bea48.
  Each one's callees, callers and globals match its doc comment in `speechplay.rs`.
- **The audio system's pending-sound queue** (`game/src/sndqueue.rs`):
  - `SndSys_PushEntry` 0x2210e8: `++count` and the entry's defaults (Rust `queue_push`), from `WorldEmitter_Update`.
  - Four out-of-line copies with no callers and no vtable word: `SndSys_PushEntryCopy1` 0x20d6b0 (the same code as
    0x2210e8), `SndSys_PushEntryKind` 0x20d4d0 (and kind +0xa40 = arg), `SndSys_PushEntryKindId` 0x20d2d8 (kind and
    id +0xa44), `SndSys_PlaySoundVolume` 0x20ce30 (kind, id, volume clamped 0..127 into the record and kept in +0x3c,
    then `SndSys_PlayPending`).
  - `cBankSys::PlaySound` 0x20d0d0 (was vf12; slot +0x6c of the vtable 0x39e878, also slot 12 of cController's):
    an entry with kind and id, then `SndSys_PlayPending`. `cBXAudio::PlayFESound` 0x20eaf0 overrides the same slot.
  - `SndSys_EchoCallback` 0x20c680 (made a function in Ghidra): the delay echo's scheduler callback (event words
    0x39e874 / 0x39ecf4), Rust `sndplay::echo`.
- **Music:** `Song_SetSoundDelay` 0x224a50 (the song's +0x11c/+0x120/+0x124 into words 1, 2, 0 of the entry's
  +0x74 row); `Music_AirLoopEvent` 0x225850 (the air-loop event's callback, word at 0x3a009c; calls the system's
  virtual +0x74). This settles 0x225850 from the batch 14 unclear list.

## Names from F1c batch 27 (proven)

- **The rider's loop sounds.** Each start function queues a loop at the rider (+0x140) with an update callback in
  the entry's +0x44 (a pointer-to-member word pair `0xffff0000, fn`; the same pairs sit at 0x39eb38..0x39eb58):
  - `Sfx_StartGlideLoop` 0x210048 (from `Sfx_StartRiderLoops` and its own update): the human's rider (+0x41c = 1)
    gets zboard program surface·8 + 4 with `Sfx_UpdateGlideLoop` 0x21b508 (made a function in Ghidra) and pending
    volume 40; an AI rider program surface·8 + 6 with `Sfx_UpdateAiGlideLoop` 0x21b628 and 50.
  - The updates, per tick: `Sfx_UpdateAirLoop` 0x21b440 (curves 5 and 6 of `Sfx_ImpactSpeed` while +0x424 = 2),
    `Sfx_UpdateGlideLoop` (GLIDE programs at cBXAudio +0x745c), `Sfx_UpdateAiGlideLoop` (AIGLIDE, +0x777c, only
    while +0x424 is 2 or 3), `Sfx_UpdateCarveLoop` 0x21b718 (CARVE, +0x7a9c; restarts `Sfx_StartCarveLoop` when
    the surface changes). The program's volume goes to the instance's +0x64 (divided by `Boarder_TickSfxFadeIn`,
    0 while +0x5ae4), its bend to `BankInstance_SetPitchBend` 0x21e7e0.
  - `BankInstance_SetPitchBend` 0x21e7e0: `SndHandle_SetPitchBend` 0x2d6a18 under `SND_Lock` (each voice of the
    handle: byte +0x2b = bend, then `SNDVoice_CalcPitchMult` and `SndVoice_UpdatePitch`; also from
    `SndMidi_OnPitchBend`). This settles 0x21e7e0 and 0x2d6a18 of the earlier unclear lists; 0x2215a8 (the same body
    on an object not proven to be a bank instance, no callers) stays open.
- `SndSys_UpdateQueuedSounds` 0x215658 (from `Audio_InGameUpdate`): each record of the ring `SndSys_QueueSounds`
  fills (+0x6e24 size, +0x6e28 words, +0x6e2c head, +0x6e30 tail; four words: owner, handle, end tick, flag) is
  taken off; one whose handle is 0 or still known to `BankMonitor_Find` goes back while its end tick is after the
  system's +0x80, else, with the flag set, its sound is stopped (`SndSys_StopBankSound`).
- `Sfx_PlayTakeoff` 0x216d58 (no callers): the human's rider's takeoff sound (program surface·8 + 2, volume
  impact·127/1000 kept within 64..127), the same as the end of `Sfx_OnAirEnter`.
- `Sfx_PlayClipBoardSound50` 0x21d9b0 / `Sfx_PlayClipBoardSound51` 0x21d680 (from `Sfx_PlayClipSound` for clip
  events 0xd0 / 0xd1): the human's rider only, kind 1, id 0x50 / 0x51, volume 127.
- The cues of `Music_UpdateMeterCues`, kind 5 on slot 6: `Music_PlayMeterCue1` 0x21ddc0 (id 1; handle kept at
  +0x7458; when the watched rider's +0x24 drops below 2.7), `Music_PlayMeterCue2` 0x21dfe8 (stops +0x7458, then id 2;
  when +0x24 reaches 0), `Music_RestartMeterCue` 0x21e248 (stops +0x7458 and plays kind 5 without setting an id, so
  the entry keeps the id word it had; when +0x58a8 went past the stored count).
- Queue entry copies: `SndSys_PushEntryCopy2` 0x227350 (the same code as `SndSys_PushEntry`), `SndSys_ResetEntry`
  0x227508 (the same without `++count`: the current entry back to its defaults). No callers.
- `MidiSys_PlayEventCallback` 0x222798 (word at 0x39fd84, before the cMidiSys vtable): a scheduled event (+0x18
  slot, +0x1c kind, +0x20 id, +0x24 volume) pushed with `MidiSys_PushEvent` and played; returns 1. The MIDI twin of
  `SndSys_EchoCallback`.
- `SpeechSys_PlayEventAnySlot` 0x228ae8 (from 0x227b24, not a function in Ghidra): `Speech_PlayEvent` on slot 0,
  then slot 1 (+0x3718); when the slot that played is +0x3724 the callback +0x3720 gets the result.
- Not settled: 0x215f18 (no callers: kind 0 id 0x32 at a rider, volume curve 3 of the length of the two objects'
  +0x150 sum, then `SndSys_QueueSounds`, like `Sfx_OnWallHit`), 0x219258 (pointer-to-member at 0x39eb5c),
  0x2215a8.

## Names from F1c batch 28 (proven)

- `SpeechEvt_Post8000` 0x22fae0 .. `SpeechEvt_Post8005` 0x22fca8: each zeroes a 0x50-byte event, sets the id
  0x8000..0x8005 and its argument words (one for 0x8000, two for the others) and calls `SpeechLib_PostEvent`,
  like the named `SpeechEvt_Post*` beside them. None has a caller.
- `SndEvent_ClearStreamPrebuffer` 0x22a2d0 (event, record): `SndStream_SetPrebufferMs`(record+0x1c, 0), returns 1.
  It is the callback word of the 8-byte record at 0x3a0420 (0, −1, 0x22a2d0) that `speechsys.rs` passes to the
  scheduler, so a delayed speech stream starts when its delay runs out.

## Names from F1c batch 29 (proven)

EA's sound library, from the batch 25 unclear list and the `sdk?` rows at 0x2e4c08..0x2e5528 (EA's code, not
Sony's):

- The software reverb (the effect-bus flag 0x10; "reverb" is our word for it, inferred from what it does:
  delay lines with feedback, damping and left/right gains):
  - `SoftReverb_Setup` 0x2e4ca0 (rate, preset): frees the old lines, then for each of 3 stages the preset's
    tap count (byte +4 + stage) and per tap (4 bytes from +8): pan as (b3·128/127, 128 − that) scaled by
    (preset byte 7 + 100) / 350, feedback ±(b2·128/127), and a delay of `SndMath_MsToPrimeSamples`(rate,
    big-endian 16-bit ms at the tap) samples, each line `SndMem_Alloc`ed and zeroed; ends in `SoftReverb_Clear`.
    Called from 0x2dd278 with preset 0x351208 (param 1) or 0x358b80, which then installs `SoftReverb_Process`,
    `SoftReverb_Clear` and `SoftReverb_AddInput` at 0x358cd0 / 0x358cc8 / 0x358ccc (all three cleared to 0
    when it is turned off).
  - `SndMath_MsToPrimeSamples` 0x2e4c08 (rate, ms): rate·ms/1000, at least 3, raised to the next prime (trial
    division up to `SndMath_ISqrt` + 1).
  - `SoftReverb_FreeBuffers` 0x2e4ec0: `SndMem_Free` of each of the 11 line buffers (0x35aba4, stride 0x24).
  - `SoftReverb_RunDelay` 0x2e4f98 (line, n, in, out): one delay line over n samples, 4 at a time after the odd
    ones: out = (in>>7)·g5 + delayed, written back = (out>>7)·g4 + in, then a one-pole damping
    (state = g8·(w>>7) + (state>>7)·g7) on the written sample.
  - `SoftReverb_Process` 0x2e0c90 (n): unless paused (0x358cd4), counts the idle ticks (0x35a6c0) and below
    2000 runs every line through the three input/output buffers (0x358ec0, 0x3596c0, 0x359ec0), wrapping
    at each line's length.
  - `SoftReverb_Clear` 0x2e0c10: idle count 2000 and the 0x800-byte input buffer 0x358ec0 zeroed.
  - `SoftReverb_AddInput` 0x2e0c40 (voice, …, channel): when the voice has samples (+8), the add hook
    (0x358e84, set by 0x2e0610 to 0x2e4af8) mixes them into the input buffer at the channel; idle count 0.
  - `SoftReverb_SetPaused` 0x2e0ff0: the 0x358cd4 word (from `SND_StopDriver` and 0x2db738).
- `SndMix_AddSat32` 0x2e4f18 (bytes, src, dst): dst += src with 32-bit saturating adds (`paddsw`), 64 bytes a
  loop (from `SoftReverb_Process`).
- `Snd_ReadBEInt` 0x2e5528 (bytes, n): an n-byte big-endian integer, sign-extended for n = 1, 2, 3.
- Voice groups (0x88-byte records at 0x3511f8; soft voices are numbered after the hardware ones, the counts at
  0x350fe3 / 0x350fe4): `SndVoice_GetHead` 0x2e5218 (the record's +0x28, or itself when −1) and
  `SndVoice_GetChannelIndex` 0x2e5250 (its place 1..5 in the head's list at +6; 0 when none);
  `SoftVoice_StartChannels` 0x2e52e8 (from `SndStreamVoice_Play`: for a voice with flag 4 and not 8 at +0x24,
  `SoftVoice_Start` on each of its +0x23 channels, then pitch, pan, effect send, 0x2dcc88 / 0x2dcd40,
  low-pass, 0x2dced0, and 0x2e0828 on each channel).
- `SoftVoice_StopAndNotify` 0x2e08a8: `SoftVoice_Stop` and the done hook 0x358cc0 (from `SoftMix_Mix`).
- `Mem_DeleteIfFlag5`..`9` (0x1956f0, 0x195710, 0x20a8c0, 0x2229b0, 0x260fa0): the same body as
  `Mem_DeleteIfFlag1` (`Mem_Delete` when bit 0 of the second argument), at the end of `ReplayMgr_Free` (two),
  `~cBankMonitor`, `~cMonitor` and `Course_FreeCollision`.

Still unclear: 0x2e5490 (the index in 0x358c38 of the entry whose voice validates to the argument), 0x2e00e0
(+8 = 1.5 · (+0xc · +4) / 32768), 0x2e0828, 0x2e0610, the three return-0 stubs 0x2e52d0 / 0x2e52d8 / 0x2e52e0
(from `SndStreamVoice_CalcMemSize`, `_Create`, `_Destroy`), 0x2dd278 (flags 0x20 / 0x40 paths), and the
earlier batch 25 list.

## Names from F1c batch 30 (proven)

- `Audio_SetMusicFx` 0x212270 / `Audio_GetMusicFx` 0x21e688: set / get the audio word +0x3844. Set by
  `Profile_ApplyAudioSettings` 0x161a98 and the sound options screen (`cFE3DSound` vf34 0x270b18, after the five
  volumes and the speech level; vf54 0x271130); read by `Sfx_SpinTrickStart` 0x218b78 (the spin sound 0x77 in
  slot 4 plays only when it is set), `Profile_SyncAudioSettings`, `cFE3DSound` vf53 and `Audio_TrickyTrigger`
  0x21c610. What the flag means is inferred (musichost.rs: "music on"; career.rs: "the output mode").
  `Audio_GetMusicFxThunk` 0x21e690: a wrapper with no callers.
- `SpeechLib_AddEventBank` 0x2bac78 (bank): a bank with the halfword 0x803 header; reads its kinds
  (`SpeechBank_GetKinds`), refuses (0) if one of the 8 slots {bank, word} at 0x348e08 has the same kinds (+9, +8),
  else stores it in the first free one and returns 1. `SpeechLib_RegisterEventBank` 0x2bad70 (bank, i): calls it
  only when i < 8 (speechload.rs REGISTER; caller `Speech_LoadEvents` 0x227d70). `SpeechLib_RegisterEventBank0`
  0x2bada0: (bank, 0), no callers.
- `SpeechLib_Printf` 0x2bec68: an empty variadic (stores its arguments and returns), no callers.
- `AsyncSys_CancelAll` 0x209a98 (sys): `AsyncSys_PopQueue(sys, 1)`, the flag that empties all four queues.
  Callers: `AsyncSys_Shutdown`, `Audio_Shutdown`, `Audio_ShutdownRace`, `Audio_StopMusicAndSpeech`.

## Names from F1c batch 31 (proven)

- Stream voices: `SndStreamVoice_GetBuffer` 0x2df9b8 (voice, channel, &addr, &flag): the stream record
  0x358c38[voice]; per channel a ring of buffers (+8 read index, +0x14 counts, +0x20 ring size, +0x24 master
  index); calls the record's +0x34 callback when a buffer is used up; returns the size or −1.
  `SndStreamVoice_Consume` 0x2dfc08 (voice, channel, n): for the master channel (+0x22) takes n from +0x2c and calls
  the +0x38 callback (voice, n, +0x3c).
- Soft-mixer decoders. `SoftVoice_Start` 0x2e0640 picks an init function from the table 0x358cd8[codec base +
  variant] and a state size from 0x358d44[...]. Codec base: 7 / 8 → 3, 9 → 0 (+0x14 flag 0: no copy), 10 → 6,
  4 → 9, 0xe → 12, 0xf → 15, 0x10 → 18, 0x40 → 21, else 0. Variant: one-shot 0, loop 1 (when the loop argument
  > 0), stream 2 (data from the stream voice). An init sets state +4 to the pull function and the args block's
  [8] to the position getter (kept at voice +0x48). Registered by `SoftMix_Init`:
  - PCM16, codecs 7 / 8 (`SoftMix_RegisterPcm16` 0x2e1530, states 0x20 / 0x24 / 0x28): `SndPcm_InitOneShot`
    0x2e1ab0, `SndPcm_PullOneShot` 0x2e19e8 (copies up to the length +0x18, pads with zeros, −1 when done),
    `SndPcm_GetPosOneShot` 0x2e19e0 (+0x14), `SndPcm_InitLoop` 0x2e1ba8, `SndPcm_PullLoop` 0x2e1af0 (from the end
    +0x1c back to the loop start +0x18), `SndPcm_GetPosLoop` 0x2e1ae8, `SndPcm_InitStream` 0x2e1d08,
    `SndPcm_PullStream` 0x2e1be8 (through `SndStreamVoice_GetBuffer` / `_Consume`).
  - XA ADPCM, codec 10 (`SoftMix_RegisterXa` 0x2e1578, states 0x6c / 0xb4 / 0x74). Proven by the coefficient
    table 0x35a700 = (0, 0), (240, 0), (460, −208), (392, −220) and frames of 1 header byte (coefficient index in
    the high nibble, shift = low nibble + 8) and 14 bytes = 28 samples, MMI `phmadh`, clamped to ±32767:
    `SndXa_DecodeFrames` 0x2e28c8. `SndXa_InitOneShot` 0x2e1fa8, `SndXa_PullOneShot` 0x2e1d80,
    `SndXa_GetPosOneShot` 0x2e1fa0 (+0x28), `SndXa_InitLoop` 0x2e24b8, `SndXa_PullLoop` 0x2e2280,
    `SndXa_DecodeToLoopEnd` 0x2e1ff0 (whole frames up to the loop end +0x30, then back to the loop start +0x2c;
    the samples left over are kept at +0x44), `SndXa_GetPosLoop` 0x2e24b0, `SndXa_InitStream` 0x2e27c0,
    `SndXa_PullStream` 0x2e2510, `SndXa_TakeBuffered` 0x2e2848 (copies min(n, kept) kept samples out, moves the
    rest down).
  - MicroTalk, codec 4 (`SoftMix_RegisterMicroTalk` 0x2e15c0, states 0xd6c / 0xd78; one-shot and stream only).
    `SndMt_ParseHeader` 0x2e3070 reads bits 1, 4 (0x20 − x), 4 (gain (x + 1) · 8.0) and 6 (64 gains, geometric
    with ratio 1.04 + 0.001 · x), clears the 12-entry arrays and the 324-float buffer. That this is EA's
    MicroTalk (UTK) is a lead from the known format; the constants are proven. `SndMt_DecodeFrame` 0x2e31e8,
    `SndMt_ReadExcitation` 0x2e2c50 (108 samples, Huffman tables 0x35a838 / 0x35aa38, or ± fixed / zero runs),
    `SndMt_InterpolateOdd` 0x2e2e98 (odd samples from 6 taps 0.59738594 / −0.11459156 / 0.01803268),
    `SndMt_RcToLpc` 0x2e2f10 (12 reflection coefficients → predictor), `SndMt_SynthSubframe` 0x2e3010 (RcToLpc,
    then 0x2e3620), `SndMt_LpcSynth` 0x2e3620 (synthesis: inferred from its arguments), `SndMt_InitOneShot`
    0x2e1760, `SndMt_PullOneShot` 0x2e1600, `SndMt_GetPosOneShot` 0x2e15f8 (+0xd68), `SndMt_InitStream` 0x2e1968,
    `SndMt_PullStream` 0x2e17d0.
- DTS output (sound mode 3). `Dts_InstallEncoder` 0x2d6ad0, called by `cPS2SoundManager` vf1 0x226168 only for
  mode 3 (which then sets manager +8 = 2): 0x353d40 = `Dts_GetWorkSize` 0x2d8de8, 0x353d44 = `Dts_Init` 0x2d8e78
  (both called by `SND_LoadIopDriver` at 0x2dbcac / 0x2dbd58 with a0 = 1 or 2 by config byte +0x3b == 5, a1 =
  config byte +0x3e) and 0x353d48 = `Dts_EncodeFrame` 0x2d9220 (called by `SND_MixThread` at 0x2dbb70 between
  `Vu0_SaveRegs` / `Vu0_RestoreRegs` when byte 0x350ff5 & 2 is set). Proven DTS by `Dts_WriteFrameHeader`
  0x2d6cc0: it writes with `SndBits_Write` the IEC 61937 preamble 0xF8724E1F, 0x000B3EE8 (data type 11 = DTS type
  I, burst length) and the DTS sync word 0x7FFE8001, then frame-header fields; the 6-bit channel arrangement is 8
  when config byte +0 == 1, else 9. `Dts_GetWorkSize` (a0 1 → ×2, 2 → ×4, else −1; a1 < 0x22 → 0xe0, < 0x43 →
  0x160, < 0x65 → 0x1e0, else −2) returns two sizes ((4·n + 0x3c)·k + 0xa8 and 4·(2n + 0x200) + 0xc00).
  `Dts_EncodeFrame` (buffers, flags, scratch): odd flags → −3; tables 0x351f00 / 0x352300 / 0x352900 by that
  0xe0 / 0x160 / 0x1e0 size (copied to the scratchpad with `Dma_ToScratchpad` when work +0xa3c is set); on a flag
  change writes a new header; per channel 0x2d7350 then 0x2d76d0 (config +0 == 1) or 0x2d7f38; finally 0x2d7af0
  or 0x2d8828. The stages 0x2d7350..0x2d8828 and the helpers 0x2d9510..0x2da7e0 are not named yet.

