# SSX Tricky Pathfinder music: format, runtime rules, and remake spec

Source: SLUS_203.26. The Pathfinder library is `PF_*` at 0x2bec00..0x2c2040; the game glue is
`Music_*`/`Audio_*`. Function names are in Ghidra (log: `gmcp/renames.txt`). The longer notes are in
`../music/README.md`.

Tools:
* `mpf2json.py SONG.mpf SONG.mus OUT [--inf MUSIC.INF|--bpm N] [--preview]` writes `node_NNN.wav` and `graph.json`.
* `pfplayer.py OUT "t:evt,t:v=N,..." out.wav` is the reference player (the algorithm below) and prints the node order.
* Shared parser and EA-XA decoder: `../music/pathfinder.py`.

## 1. `.mpf` (magic bytes `xDFP` = LE `'PFDx'`, version u16 0x0103 in every file; offsets are 4-byte words)

```
0x00 'xDFP'  0x04 u16 0x0103  0x06 u16 0x00b0  0x08 u32 0
0x0c u8 instance(0)  0x0d u8 nTracks(=1)  0x0e u8 nColumns  0x0f u8 nEvents (11 race, 6 menu)
0x10 u8 nActions     0x11 u8 nRouters     0x12 u16 nNodes   0x14..0x23 zero
0x24 u16 nodeOfs[nNodes]                              (word offset of each node)
node (12 + 4*nBr bytes):
  +0 s16 seg     >0 audio segment (1-based) of the track's segment table; 0 = section START marker; -1 = END marker
  +2 u8  track   +3 u8 sectionFlags: low 7 bits = event-table column (section id); 0x80 on "main" START markers; 0xff on END
  +4 u8  (1; 8/16/32 in the menus, unused)   +5 u8 nSub(4)  +6 s8 sync(4)  +7,+8 sync params
         (sub-node sync; only used when the track lookahead is < 50 ms, never in Tricky)
  +9 u8  loop    END marker pass counter (255 or 0)
  +10 u8 router  (0 = none, else 1-based)
  +11 u8 nBr, then nBr x { s8 lo, s8 hi, s16 dest }
event table   u8 action[nTracks][nEvents][nColumns], padded to 4
actions       nActions x { u8 vol (0xff = keep), u8 flags, s16 target }
              flags 0x01/0x02 = transition, 0x80 = immediate. Target -1 with 0x02 = stop. flags 0 = no-op.
routers       u32 ofs[nRouters+1]: router k = u32 words [ofs[k-1], ofs[k]), each (s16 src << 16 | u16 dst)
              ofs[nRouters] = word offset of the track table: u32 segTableOfs[nTracks]
segment table { u32 musOffset/4, u32 length_ms } per segment
```

**`.mus` layout.** The file holds one `SCHl` stream per segment, at `musOffset*4`.
* PT header tags: 0x82 = 2 channels, 0x84 = 36000 Hz (slaybreak: 35999), 0x85 = sample count, 0xA0 = 0x0A (EA-XA v2).
* Blocks: `SCDl` = u32 ns, u32 chanOfs[2], then per channel s16le hist1, s16le hist2 and 15-byte frames of 28 samples each. Every block decodes on its own.

**Decode check (smartbomb).** The decode was verified against the independent decoder in `../bnk/levelaudio.py` and is bit-exact.
* Peak 30617, 0 clipped samples. High-frequency/total energy is 0.03–0.15, so the output is not noise.
* Decoded length matches `length_ms` to within 1 ms.
* The joins are continuous: the median step across a node boundary is 0.18x the local 99.9th-percentile step, and the maximum is 1.6x.
* 316 of the 317 nodes are 4.00 beats (±0.14) at 136 BPM (63.2k–63.6k samples). Node 294, the outro tail, is 6.57 beats.

**What the "24 tracks" are.** The library has 4 instances x 24 track slots. A track is one independent stream cursor through the graph, with its own path variable, volume, loop counter and section. An event or a `SetPathVar` call is applied to every track in a bitmask. Every Tricky `.mpf` has `nTracks = 1`, and the game opens only track 0 (`PF_OpenTrack(0,0,…,500 ms lookahead)` in `SongInstance_Init`). So there is a single stereo stream with no layering. `Music_SetPathVar` writes the byte into all 24 slots only for generality.

## 2. Runtime rules (exact, from PF_Update 0x2c0488 / PF_AdvanceNode 0x2bfbb8 / PF_ApplyEvent 0x2bf508)

**State per track:**
* `var`: u8 0..127. The game sends 0..99.
* `last`: the last node that was queued. This is the lib's "current node" (+0x16). Event columns and routers use it.
* `loopNode`: starts at -1.
* `loopCnt`: u8.

**Successor (no randomness, no weights):**
```
branch(n, v): if n == loopNode { v = loopCnt & 0x7f; if node[n].loop { loopCnt--; if loopCnt == 0xff: loopNode = -1 } }
              return dest of FIRST branch with lo <= v <= hi (signed, inclusive), else -1
route(cur, x): if cur >= 0 and node[cur].router: for (src,dst) in routers[router-1] in order: if x == src: x = dst
resolve(cur, x): x = route(cur, x)
              while x >= 0 and node[x].seg < 1:                              # skip markers
                  if node[x].seg == -1 and node[x].loop and x != loopNode: loopCnt = node[x].loop; loopNode = x
                  x = route(cur, branch(x, var))
              return route(cur, x)                                          # re-applied once more (no effect in shipped data)
next(cur) = resolve(cur, branch(cur, var))
```

**Variable ranges.** The ranges are things like (0,28)(28,66)(66,127), with the edge value going to the first range. They pick the intensity variant of the same bar: smartbomb has 3 parallel 16-node lanes in its main sections, and the variable can switch lanes at every node. With `loop = 255` on an END marker, `v = loopCnt & 0x7f` counts 127..0, so the section repeats about 127 times before the `(0,0)` branch is taken. For example, smartbomb node 271 goes to the outro 288 when the count reaches 0. In practice this means "loop forever".

**Timing.** Decisions are made when queued audio is at most 500 ms (the lookahead), which is about 0.5 s before a node ends. The chosen node is appended gaplessly. The variable is read at that moment, so a variable change takes effect at the next boundary that is more than 0.5 s away. A node is always played whole.

**Events:**
* `PF_PostEvent(mask, evt, prio)` inserts the event into a 16-entry queue, sorted by priority with FIFO among equals.
  * `Audio_MusicEvent` uses priority 0; `Audio_MusicEventPrio1` uses priority 1.
  * A negative priority means "end branch".
  * Priority 2 flushes the queue and stops.
* Action = `actions[events[track][evt][node[last].sectionFlags & 0x7f]]`. In Tricky all columns are equal.
* **Immediate (flags 0x80):** applied on the next update. The stream queue is flushed and `resolve(last, target)` starts now. The race uses this for event 0 (start), and for 7/8 in some songs.
* **Normal:** when the 500 ms window opens, the event replaces the normal successor: `resolve(last, target)`, with `-1` for a stop action. A no-op action is consumed and the normal successor plays. The event takes effect at the end of the currently playing node if it was posted more than about 0.55 s before that node ends; otherwise it takes effect one node later.
* One event is consumed per boundary, so two events posted together act on two consecutive boundaries.
* **Restore:** when the target START marker lacks flag 0x80 (the zone-enter targets), the lib remembers the event. If that branch later reaches -1, it restarts at the target. None of the zone chains ends, so this never fires in Tricky.
* **End of song:** reaching node -1 stops queueing. The stream plays out what is queued and goes silent.

**Race events:**
| event | meaning |
|---|---|
| 0 | start (immediate; the game then sets the variable to 80) |
| 1/3/5 | enter shortcut zone 1/2/3 (jump to a zone loop) |
| 2/4/6 | leave the zone (1 s after exit) |
| 9 | boarder message 9: a long loop section that eventually leads into the outro |
| 10 | finish: outro chain of 1–6 nodes, then -1 (smartbomb: 288→290..294, ~10 s) |

**Start latency.** The stream is primed and unpaused 500 ms after the first node is queued (`+0x28`).

## 3. LOOPDATA banks (`/home/claude/lvl/musicloops/<bank>/<bank>_<prog>.wav`, `<bank>.json`, `loops.json`)

Each bank has 48 mono one-shot programs. finsym has 40 (no phrase 3) and zslaylpz1 has 32 (phrases 0–3 only).
* Rate: 24000 Hz, except the `z*` banks (zgin, zpeak, zshake, zslay), which are 22050 Hz.
* Each program is exactly one beat at the song's BPM (0.92–1.02 beats).

**Program numbering:** `prog = phrase * PhraseAlign(16) + beat`, with beat = 0..BeatsPerPhrase-1 (7).

**Phrase selection** (`Music_PickAirLoopPhrase` 0x2246a8, PPB = PhrasesPerBank = 4, Z = 2):
* Tricky: phrase = PPB-1 (3). Tricky takes precedence over a zone.
* In a shortcut zone (not Tricky): phrase = PPB + rand % Z (4–5).
* Otherwise: phrase = rand % (PPB-1) (0–2).

## 4. Remake algorithm

### Load
* Load `graph.json`, the node wavs (36 kHz stereo i16) and the MUSIC.INF params.
* Resample to the output rate once at load, or run the mixer at 36 kHz.

### Main track (audio thread, per callback)
```
state: queue<(prio,seq,evt)>, play: deque<(node,pos)>, last=-1, var=0, loopNode=-1, loopCnt=0
post(e, prio): insert sorted (higher prio first, FIFO within prio)
render(n):
  if queue.head and action(head).immediate: pop; play.clear(); n0 = resolve(last, target)  (stop if target<0)
                                            last = n0; if n0>=0 play.push(n0)
  while last >= 0 and remaining(play) <= 0.5*rate:
      if queue nonempty: a = action(pop()); nxt = a.transition&&a.target>=0 ? resolve(last,a.target)
                                                 : a.stop ? -1 : None
      if nxt is None: nxt = next(last)
      last = nxt; if nxt >= 0: play.push(nxt)
  copy samples from play front to back (no crossfade; joins are sample-continuous)
```
* Race start: `post(0, 1)`, then `set_var(80)`. Delay the audible start by 500 ms if you want exact timing.
* Path variable: single player uses `min(boost*1.5, 1) * 99`. With n racers: `99 - (rank-1)*99/(n-1)`. Replays use 80.
* Finish: `post(10)`. The outro starts at the next boundary, and then the song ends (`last = -1`, the queue drains).
* Looping: the graph loops by itself (END markers branch back). No looping code is needed in the player.
* Volume: `music_vol * PathLevel/100 * duck`. The duck comes from `Music_SetDuckVol` while a big air loop plays.

### Air loops / Tricky layer (separate one-shot sampler, mono)
```
start (takeoff with predicted air > 1.5 s, or Tricky start):
    phrase = pick(zone, tricky); base = phrase*16; beat_ms = 60000/BPM
    pos = ms into the currently sounding node; delay = beat_ms - (pos mod beat_ms); k = floor(pos/beat_ms)+1; if k>=8: k=0
    after delay: every beat_ms trigger program base+k (one beat long), k = (k+1) % 8
stop: on landing / Music_BigAirLoopCancel. Tricky mode stops at the end of the phrase once not in big air.
gain = AsyncLevel/100 x (the amount the music was ducked) or the Tricky category volume
echo: DelayCount taps, DelayTime ms apart (EIGHTH = BPM/60*0.125*1000 ms, the engine's formula),
      each tap DelayLevel% (feedback is 0 in all songs)
```
Phrases missing from a bank (finsym phrase 3, zslay 4–5) play silence.
