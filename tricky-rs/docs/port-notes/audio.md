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
  ASSIGNBEND 20; unknown words skipped; 0x7f ends. The interpreter that runs them is not found yet (the programs are
  reached through pointers copied elsewhere); the app's is ours.
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
