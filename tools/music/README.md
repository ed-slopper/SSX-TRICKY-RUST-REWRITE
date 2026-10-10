# SSX Tricky "Pathfinder" interactive music

Reverse engineered from SLUS_203.26. Ghidra names: the `PF_*` lib (0x2bec00-0x2c2040) and the
game glue (`Music_*`, `Audio_*`, `SongInstance_Init` 0x223d60). Renames are logged in gmcp/renames.txt.

## Tools

See also `../mpf/` (`mpf2json.py SONG.mpf SONG.mus OUT` -> node wavs + graph.json, `pfplayer.py` reference player, `SPEC.md`).

| tool | what it does |
|---|---|
| `mpf2json.py SONG.mpf OUT.json [--var N]` | dumps the graph, plus `event_entry`, `default_path` and `finish_path` |
| `mus2wav.py SONG.mpf SONG.mus OUTDIR [--var N] [--preview]` | writes `node_###.wav` for every node that has audio, plus `song.json`: the graph, per-node `wav`/`samples`/`rate`, and a `next` table (the resolved successor for each path value 0..127) |
| `musicbig.py SRC MUSIC.INF OUTDIR [--only k1,k2] [--preview]` | runs every song. `SRC` is `MUSIC.BIG` / `MUSIC.BIG.00` (later `.01`... parts are joined automatically) or a folder of loose files. Output is `OUTDIR/<key>/` with the song and `loops/` (the LOOPDATA bank through `../bnk/bnk2wav.py`, prefix `loops`), and `OUTDIR/songs.json` (MUSIC.INF params with the engine defaults filled in, plus the loop bank's used programs per phrase) |

`pathfinder.py` is the shared library: the BIG reader, the INF parser, the MPF parser, the runtime
model (`PathState`) and a numpy EA-XA decoder (it decodes every SCDl block independently).

## .mpf format (magic `xDFP` = LE 0x50464478, version 0x0103; offsets are 4-byte words)

```
0x00 'xDFP'  0x04 u16 0x103  0x06 u16 0xb0  0x08 u32 0
0x0c u8 instance slot   0x0d u8 nTracks (1 in all files)   0x0e u8 nColumns (section ids)
0x0f u8 nEvents (11 race / 6 menu)  0x10 u8 nActions  0x11 u8 nRouters  0x12 u16 nNodes  0x14..0x23 0
0x24 u16 nodeOfs[nNodes]
node: s16 seg | u8 track | u8 sectionFlags | u8 b4 | u8 nSub | s8 sync | u8 s7 | u8 s8 | u8 loop | u8 router | u8 nBr
      then nBr x {s8 lo, s8 hi, u16 dest}
  seg > 0  : plays segment seg (1-based) of the track's segment table
  seg == 0 : START marker of a section (sectionFlags = 0x80|section). Has no audio.
  seg == -1: END marker (sectionFlags 0xff). Has no audio. loop = 255 on sections that repeat.
  sectionFlags & 0x7f = event-table column used while this node plays
event table  u8 action[nTracks][nEvents][nColumns]          (padded to 4)
actions      {u8 vol (0xff keep), u8 flags, s16 target}     flags: 0x01/0x02 go-to, 0x80 immediate cut,
             target -1 with 0x02 = stop, flags 0 = no-op
routers      u32 ofs[nRouters+1]; router k = words [ofs[k-1], ofs[k]) of (src<<16 | dst)
             ofs[nRouters] -> track table: u32 segTableOfs[nTracks]
segment table per track: {u32 musOffset/4, u32 length_ms}
```

## .mus format

The file is the track's segments laid end to end. Each segment is one EA `SCHl` stream, found at
`musOffset*4`. Its PT header gives EA-XA v2 (codec 0x0A), 2 channels, 36000 Hz (slaybreak: 35999)
and the sample count. Each `SCDl` block holds per channel `s16le hist1, s16le hist2` plus 15-byte
frames (28 samples each). The first block of every segment starts at history 0. A node's length is
`samples/rate` and equals `length_ms`. One `.mus` holds one track, so there is no interleaving
beyond stereo.

`/tmp/rt2/audio/music/smartbomb.ogg` is every segment in file order: 20176065 samples, which
matches the sum of the decoded node wavs exactly. It includes the intensity variants, the zone
loops and the outro, so it is not the song as the game plays it.

## Runtime (one track; `SongInstance_Init` opens track 0 with a 500 ms lookahead)

### Start

* `Audio_StartRaceMusic` posts event 0 at priority 1 (`Audio_MusicEventPrio1`).
* Event 0's action has flags 0x81 and targets node 0 (or the song's first START marker). That node
  is resolved to the first playable node.
* The path variable is then set to 80.
* In the menus, `Music_SetFEState0..5` post events 0..5.

### Path variable

* `Music_SetPathVar` writes a byte (0..127, the game sends 0..99) to all 24 track slots.
* Single player: `min(boostMeter*1.5, 1) * 99`, where `boostMeter` is boarder+0x1c, 0..1.
* Several racers: `99 - (rank-1)*99/(n-1)`.
* Replay or start of race: 80.

### Next node (`PF_AdvanceNode`)

This runs when less than 500 ms of queued audio is left, so the choice is made about 0.5 s before
the boundary. The new node's audio is appended gap-free.

```
next = resolve(cur, branch(cur, var), var)
branch(n, v): if n == loopNode { v = loopCnt & 0x7f; if node.loop { loopCnt = (loopCnt-1)&0xff; if loopCnt==0xff: loopNode=-1 } }
              first {lo,hi,dest} with lo <= v <= hi (signed) -> dest, none -> -1
route(cur, x): if node[cur].router: for (src,dst) in routers[router-1]: if x==src: x=dst
resolve(cur, x, v): x = route(cur, x)
              while x >= 0 and node[x].seg < 1:
                  if seg == 0: sectionStart = x
                  elif node[x].loop and x != loopNode: loopCnt = node[x].loop; loopNode = x
                  x = route(cur, branch(x, v))
              return x            # -1: the song stops
```

* Branch ranges are the intensity variants. For example, smartbomb sections have three parallel
  chains selected by 0..28, 28..66 and 66..127; the first match wins, so 28 takes the lower chain.
* Routers make the song loop. For example, slaybreak node 55 has router 1 (56 -> 10), so the song
  goes back to section 2 instead of into the outro.
* Event-only sections end in an END marker with loop = 255, and repeat until another event (127
  passes before the counter would let them out).

### Events (`PF_PostEvent` / `PF_ApplyEvent`)

* Each event has a priority:
  * `Audio_MusicEvent` posts at priority 0 and `Audio_MusicEventPrio1` at priority 1.
  * The queue has 16 entries. A higher priority goes first; equal priorities stay in posting order.
  * Priority 2 (unused) flushes the queue.
* The action is `actions[events[0][e][node[cur].sectionFlags & 0x7f]]`. In every Tricky file, all
  columns are equal.
* When the action applies:
  * Non-immediate: at the next node boundary, in place of the normal successor. The decision is
    made 500 ms + 50 ms before the boundary; an event posted after that applies one node later.
  * Immediate (0x80): on the next tick. The stream queue is flushed and the target starts now.
* The target goes through `resolve(cur, target, var)`.
* A no-op action consumes the event. Target -1 with 0x02 stops the music.
* Only one event is applied per boundary; the queue head is popped once its target has started.

#### Event numbers in the race

| event | posted by |
|---|---|
| 0 | race start (also boarder message 1) |
| 1, 3, 5 | entering shortcut music zone 1, 2, 3 (`Audio_ShortcutMusicZone`, zone types 2/4/6) |
| 2, 4, 6 | leaving that zone (`Music_ShortcutZoneExitCb`, 1 s after exit; `Music_ShortcutZoneReset`) |
| 7, 8 | not posted by the race code. Some songs map them to immediate jumps |
| 9 | boarder message 9 |
| 10 | finish (boarder messages 10/0x12 on the last lap). Plays the outro chain, which ends in -1 |

The menu songs (6 events) are driven by `Music_SetFEState0..5`.

### Timing and volume

* **Granularity:** whole nodes. smartbomb nodes are 4 beats (1.76 s); slaybreak nodes are 8 beats
  (4.56 s).
* **Unused node fields:** the nSub/sync fields (4 subdivisions; START-marker beat-matched cuts)
  only apply to tracks opened with less than 50 ms lookahead, or to secondary tracks. Tricky uses
  neither.
* **Tracks:** the lib supports 4 instances x 24 tracks, but Tricky uses 1 track, so nothing is
  mixed.
* **Volume:** music category volume x PathLevel/100 (MUSIC.INF), times the duck from
  `Music_SetDuckVol` (big air > 1.5 s, see `Audio_BigAirMusicDuck`). The action vol byte is always
  0xff.

## Async in-air loops (LOOPDATA bank)

### When they play

* Start: takeoff with predicted airtime > 1.5 s (`Sfx_Takeoff` -> `Music_BigAirLoopStart`), or
  Tricky (`Music_TrickyStart`). They are cancelled on landing or `Music_TrickyStop`.

### Which phrase (`Music_PickAirLoopPhrase`)

* `PPB` = PhrasesPerBank (4), `A` = PhraseAlign (16), `Z` = 2 zone phrases.
* Normal: phrase = `rand % (PPB-1)` (0..2).
* Tricky: phrase = `PPB-1` (3).
* In a shortcut zone and not Tricky: phrase = `PPB + rand % Z` (4..5).
* Base program = phrase*A.

### How the beats play (`Music_ScheduleAirLoop`, `cBXAudio::vf13_20df08`)

* Every beat (60000/BPM ms), the bank program `base + k` is triggered as a one-shot, one beat long,
  at 24 kHz.
* Start: wait until the next beat of the playing node (`beat - pos % beat`), with
  `k = floor(pos/beat)+1`. If k >= BeatsPerPhrase(8), k = 0.
* After each beat, `k++`. On reaching 8 it wraps to 0 and keeps looping while airborne. In Tricky
  mode it stops after the phrase once the player is no longer in big air.
* Volume: AsyncLevel/100 x the music volume that the duck takes away (`Audio_BigAirMusicDuck`
  +0x389c), or the Tricky category volume.
* Echo: DelayCount taps, DelayTime ms apart, DelayFeedback %, DelayLevel %. The engine computes
  "EIGHTH" as `BPM/60*0.125*1000` ms (an engine bug, e.g. 283 ms at 136 BPM). All songs use
  feedback 0.

### Loop banks

* Programs are `phrase*16 + beat`, with phrases 0-5 x beats 0-7 (48 programs). Exceptions:
  * finsym has no phrase 3, so its Tricky phrase is silent.
  * zslaylpz1 (slaybreak) has only phrases 0-3, so its zone phrases are silent.
  * The menus have no LOOPDATA.

## Rust player spec (data from `song.json` / `songs.json`)

### State

`cur` (node id), `loop_node = -1`, `loop_cnt: u8`, `var: u8`, `queue: Vec<(event, prio)>`, and the
sample position within `cur`.

### Startup

```
n = resolve(-1, actions[events[0][0][0]].target, var)
play n
```

### Each audio callback

1. When the frames left in `cur` drop below 0.5 s, and `next` is not chosen yet:
   1. If the queue head's action is not immediate: take it. If its target >= 0 and its flags & 3,
      then `next = resolve(cur, target, var)`; if the target is -1 and the flags have 0x02, then
      `next = -1`. Pop it.
   2. Otherwise: `next = resolve(cur, branch(cur, var), var)`. Use `node.next[var]` only as a
      cache, because loop counters make the general case stateful.
2. At the end of `cur`, continue sample-exact with `next`; the joins need no crossfade. If
   `next == -1`, the music ends.
3. An immediate event (flags & 0x80) cuts now: resolve its target from `cur` and start it at
   sample 0. A short fade of a few ms is optional; the PS2 hard-cuts.

### Inputs

* `set_var(0..99)`
* `post(e)`: 0 = start, 1-6 = zones, 9, 10 = finish

### Loops

* Use the separate one-shot sampler described above.
* Programs come from `songs.json[key].loops[0].phrases`. Files are
  `<key>/loops/loops_<prog>.wav`, with metadata in `loops.json`.
