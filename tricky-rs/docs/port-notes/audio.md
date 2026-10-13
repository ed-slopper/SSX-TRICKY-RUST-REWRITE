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
