# Instant replay

How the game records and plays back a race. Ported so far: the byte streams and the pad streams (board row G12a;
`tricky_game::replay`, checked: `tests/replay_streams.rs`), the replay codec (G12b, G12c: files.md), the keyframe
list, seeking and playlists (G12d; `tricky_game::replaymgr`, checked: `tests/replaymgr.rs`), the highlights
and pins (G12e; the same module, checked: `tests/replay_highlights.rs`), the camera tracks (G12f; checked:
`tests/replay_camtracks.rs`), playback control and the manager's life (G12g; checked:
`tests/replay_playback.rs`), the save to the card and the load (G12h; checked: `tests/replay_save.rs`), the world snapshot's
orchestration (G12i; checked: `tests/replay_snapshot.rs`), the world nodes' snapshot saves (G12j;
`tricky_game::snapshot`, checked: `tests/snapshot_saves.rs`), their load constructors (G12k; checked:
`tests/snapshot_loads.rs`), the riders' and AI's snapshots (G12l; checked: `tests/snapshot_riders.rs`), and the last load constructors
(G12m; checked: `tests/snapshot_last.rs`). Keyframes, the node save and restore, pins and
highlights, camera tracks, `Replay_Update` and the memory-card save are the rest of G12.

## The replay control

gApp+0x730 -> +0x40. The words the streams use: +0xc the size of a keyframe's stream, +0x20 the mode (0 recording;
1, 2 and 4 play the pads back; up to 13 in the jump table at 0x38a340), +0x3c / +0x40 the pad streams full (180
frames of a message), +0x4c and +0x50 hold recording, +0x58 a stream overflowed.

## Byte streams (`Replay_AllocStream` 0x192b38 .. `Replay_WriteOffset` 0x193020)

- A stream is four words: the buffer, the write pointer, the read pointer and the bytes written. `Replay_AllocStream`
  allocates the buffer ("Frame Block") and fills it with 0xdeadc0ed through `EAMem_Set`. `EAMem_Set` 0x2cc808
  aligns with a byte, a half word and a word, then stores 64- and 16-byte blocks, words, a half word and a byte.
  Each store takes the value's low byte, half or word, so the pattern depends on the address. `Replay_FreeStream`
  frees it and zeroes all four words.
- `Replay_Write` copies bytes only while no stream has overflowed. If the bytes do not fit in the control's
  stream size less what is written, it sets the overflow flag and writes nothing.
- `Replay_Read` copies with no check: it can read past what was written. `Replay_Peek` reads and puts the read
  pointer back. `Replay_PeekNodeType` gives the third word of the next 12-byte node header.
- `Replay_RewindRead` puts the read pointer at the start. `Replay_ResetWrite` does the same for the write pointer
  and zeroes the count.
- `Replay_UnpackBlock` reads a packed block from a loaded replay: its size, then `Decompress_Block` (0x241e28 ->
  `Huff_DecodeBlock` 0x240ef8) into the buffer. The write pointer goes to the unpacked end, the read pointer to the
  start, and the source cursor moves on by the packed size. It also copies 4 bytes from its third argument into a
  stack word that nothing reads.
- Node references are instance numbers: the node's address less the course's instance array (*0x34648c +0x48),
  shifted right by 8, or -1 for none. They are read back through `Course_GetInstance` 0x254a28, which gives 0 at
  or past the course's count (+0xc).
- A link (`Replay_WriteLink` / `ReadLink`) is 8 bytes: the reference, the low half of the node's +0xe8, and
  whether the node's +0xe4 is the owner. If that flag is set, reading puts the owner back in +0xe4 and the half
  word back in +0xe8.
- Offsets (`Replay_WriteOffset` / `WriteOffset2` / `ReadOffset` / `ReadOffset2`, the same code twice) are taken
  from the data base at 0x34649c.

## Pad streams (`ReplayInput_*`, `Replay_*Input` 0x1926f0 .. 0x192b38)

- Each rider has a pad stream of 0x800c bytes:
  - up to 0x1999 pad words, run-length coded: a byte at +0x6664 gives each word's run of frames;
  - the count at +0x8000;
  - for playback, the current word (+0x8004) and the frames left of its run (+0x8008).
- `ReplayInput_Init` and `ReplayInput_Reset` zero the count, the current word and the frames left. The words and
  runs stay as they are.
- `Replay_RecordInput` lengthens the last run when the pad is the same and the run is under 255; otherwise
  `Replay_PushInput` adds a word with a run of 1. Past 0x1998 words, push marks the control's pad streams full
  instead (+0x3c 180, +0x40 1).
- `Replay_ReadInput` zeroes the pad. While words are left it loads the run when the frames left are used up,
  gives the word, and moves to the next word when that run ends.
- `ReplayInput_SeekFrame` adds up the runs to find the word playing at a frame and the frames left of its run. Past
  the end it answers 0 and changes nothing.
- `Replay_RecordOrPlayInput` (called from `GetControls`):
  - mode 0, with +0x50, +0x4c and +0x40 all clear: records the pad and answers 1;
  - modes 1, 2 and 4: play the pad back;
  - any other mode: answers 0.
- `ReplayInput_Load` reads an 8-byte header (its second word is the count), then two packed blocks: the words, then
  the runs.
- Checked in the runner on random calls through all 24 entry points:
  - 4,000 `EAMem_Set` cases at every alignment;
  - every fill size up to 40;
  - 200 runs of 3,000 calls each, with the control's modes and holds, near-full pad streams and full runs forced in.
  - Compared after every call: the stream's pointers, the control, the pad stream's words, every value read and
    every node touched. The whole buffer and the pad stream are compared every 100 calls.
  - Pad streams filled, full runs recorded over and stream overflows each happen.
  - `Huff_DecodeBlock` is a stand-in on both sides of this test; the codec itself is `compress::huff_decode_block`
    (G12b, files.md).

## The keyframe list, seeking and playlists (G12d)

- The manager's fields are documented at the head of `replaymgr.rs`. The port works on the game's bytes at their
  addresses (`pf::Mem`), because the manager, its keyframes, highlights and playlists all point at each other.
- **Keyframes.** There are 16 of them, 0x24 bytes each from +0x3e68: a stream, next / previous links, a pin count,
  an in-use flag and the frame. They form a list from +0 in frame order and a free list from +4.
- **Allocating.** `Replay_AllocKeyframe` takes the first free keyframe. When none is free it first evicts one
  (`Replay_EvictKeyframe`). The keyframe evicted is the first unpinned one, after the keyframe at +0x18, that has
  another unpinned one somewhere after it (`Replay_FindEvictableKeyframe`). If there is none, the search starts
  again from the second keyframe. +0x18 then moves past the evicted keyframe. The game also walks on past pinned
  keyframes and drops what it finds.
- **Freeing.** `Replay_FreeKeyframe` unlinks a keyframe, resets it and puts it first on the free list. If it was the
  newest (+0x10), the newest moves back to the one before it. `Replay_TryFreeKeyframe` frees only a keyframe with
  one before it and two after it, not in use and not pinned. `Replay_PruneKeyframes` frees every such keyframe from
  the second on. Without its argument it keeps the last.
- **Finding.** `Replay_FindKeyframe(frame)` gives the keyframe at the frame, or the last one before it, among those
  unpinned or starting one of the current rider's (+0x24) highlights. It gives the last keyframe when none fits.
  Next / previous step from that keyframe to the next usable one. `Replay_KeyframeIndex` and `_CountUsed` skip the
  keyframe being written (+0x1c). The counts are 0 until there is a newest keyframe.
- **Seeking.** Seeking sets the frame (+0x34) and the keyframe playing (+0x14), and seeks every rider's pad stream.
  The start is frame 0 at the first keyframe. The end is the last keyframe (`Replay_FindKeyframe` of 100,000,000).
  Stepping goes to the next or previous keyframe, and does nothing while a playlist plays. When a playlist plays
  (+0x391c), its current entry gives the keyframe and frame instead.
- **At the end.** `Replay_IsAtEnd` is true:
  - in modes 1, 2 and 7: at or past the end frame (+0x30), or no longer playing (+0x48), or past the playlist
    entry's end;
  - in modes 3, 5, 6, 8 and 9: always;
  - in mode 4: at or past the end frame.
- **Playlists.** A playlist is a count (up to 10), the entry playing and the entries. Each entry is a highlight: its
  keyframe and its length in frames.
- **Seeking a playlist by frame** (`ReplayPlaylist_SeekForward` 0x1954b8 / `_SeekBack` 0x195510, for the overlay's
  scrubbing; G12d first named them as camera-track functions): forward, the first entry after the frame; back, the
  entry before one at the frame, or the last entry before it.
- **Walks that never end.** On a circular list the game's walks never end. The port panics after a million steps
  with a message saying so.
- **Checked in the runner:** 3,000 random managers (lists, free lists, pins, highlights, playlists, camera tracks
  and pad streams), with 30 random calls each over the 32 entry points. Every result and all the memory was
  compared after each call. The 57 walks that never ended were checked to run past the game's step limit too.

## Highlights and pins (G12e)

- **Highlights.** A highlight is a moment worth showing again: a keyframe, a length in frames, a kind (1 or 2), a
  score for each kind (floats) and a rider. There is a pool of 0x1c-byte records at +0x3968, with a free list from
  +0x3964. Each rider keeps up to ten, in frame order.
- **Inserting and removing.** `Replay_InsertHighlight` puts a free record into the rider's list at its frame. It
  refuses a frame that already has one, and an eleventh. `Replay_RemoveHighlight` unpins its keyframe (freeing the
  keyframe if it may be: `Replay_TryFreeKeyframe`), clears its keyframe word, and takes it out of the list and back
  to the free pool. `Replay_FindHighlight` gives the rider's highlight starting at a frame.
- **Adding one.** `Replay_CanAddHighlight` returns -1 at the end frame. It returns 1 inside one of the rider's
  highlights, and otherwise whether the rider has fewer than ten.
- **Recording.** +0x395c is the highlight being recorded, and +0x3960 the frame it started at.
  - `Replay_BeginHighlight`, inside one of the rider's highlights (or up to 11 frames past one), carries that one
    on. Otherwise it inserts a new one at the frame on the keyframe there. When that keyframe is not at the frame
    it records one (`Replay_RecordKeyframe`, a call into the env here; if that fails, the highlight is dropped).
    The keyframe is pinned.
  - `Replay_EndHighlight` adds the frames since the start to the length. The rider's other highlights that start
    inside it are merged in: what they run past its end is added to its length, their pin slots are cleared, and
    they are removed.
  - `Replay_DropCurrentHighlight` removes the rider's first highlight that the frame is not past the end of. It is
    measured from the highlight's start with no lower bound, so an earlier highlight counts too.
- **Pins.** Each rider has a pin (+0x3920, four of them), which keeps a rider's best moments for the replay. It
  holds the highlight being made, four slots of kept highlights and a pinned keyframe. +0x3940 says how many slots
  are used (2 or 4), and +0x3944 / +0x3948 are the quotas for kinds 1 and 2.
  - `ReplayPin_Begin` runs while recording: +0x40 clear, mode 0 or past 9, and not frozen. It pins the keyframe
    before the newest and begins a highlight of the kind at that keyframe's frame.
  - `ReplayPin_End` does nothing while a replay plays (modes 1..9). While recording, it gives the highlight a
    length from its keyframe to +0x2c plus 60 frames, and commits it.
  - `ReplayPin_Commit` keeps the highlight in the slot picked (removing the one already there) on the pinned
    keyframe. A highlight with no kind or no slot is released instead.
  - `ReplayPin_PickSlot` takes an empty slot first. If there is none, a kind under its quota less one takes the
    other kind's worst slot, but only when the other kind is at its quota. A kind at its quota takes its own worst
    slot, unless the new highlight scores lower. A kind-1 highlight scoring 0 or less is never kept.
- **Releasing, freezing and adding.**
  - `ReplayPin_Release` lets go of the pinned keyframe and removes the highlight being made.
  - `ReplayPin_Clear` also removes every kept highlight.
  - `Replay_Freeze` releases all the pins the first time, then marks the replay frozen (+0x50).
  - `Replay_AddPin` and `Replay_RemovePin` allocate and delete a pin; the allocator is a call into the env here.
  - Where the game finds its manager through gApp (gApp+0x730 -> +0x40), the port is handed the manager.
- **Saving and loading** highlights and pins: G12h, below.
- **Checked in the runner:** 3,000 random managers (keyframes, the pool and the riders' lists, pins), with 25 random
  calls each over the 26 entry points. Every result and all the memory were compared after each call. Highlights
  begun, inserted and dropped, slots picked and commits that kept their highlight all happen many times.

## Camera tracks (G12f)

- **Layout.** Each rider has a camera track: four at +0x5c, 0xe24 apart. A track holds a cut count (up to 128),
  the cut playing (-1 for none), a default cut at +8, and the cuts from +0x24. Each cut is 0x1c bytes: the five
  chase-camera words, a preset and a frame.
- **The cuts flag.** The flag at 0x32ca4c switches the cuts on; when it is off, only the default cut is used.
- **Building a track.** `ReplayCamTrack_Generate` builds a track fresh. It switches the camera to replay mode for
  the rider (`Camera_SetReplayMode`), takes the camera's view as the cut at frame 0 and seeks to frame 0.
  `ReplayCamTrack_Insert(frame)` stores the camera's chase words and preset as a cut:
  - at frame 0 it also becomes the default;
  - a cut already at the frame is overwritten, and the call answers false;
  - otherwise the new cut goes in after the last cut before the frame;
  - when no cut is before the frame, it goes at the end, not at the front;
  - with 128 cuts it answers false.
- **Changing a track.** `_UpdateCurrent(frame)` writes the camera into the last cut at or before the frame (the
  default when the flag is off). `_RemoveAt(frame)` removes the first cut at or past the frame, but never the first
  cut, and only with the flag on and more than one cut.
- **Playing a track.**
  - `_SeekFrame(frame)` finds the last cut at or before the frame. When there is none it takes the count less one,
    and with the flag off it takes 0.
  - `_Advance(frame)` moves to the next cut once the frame reaches it.
  - `_Apply` selects the playing cut's preset (the default's with the flag off) and sets its chase words. They are
    passed to `Camera_SetChaseParams` as four float registers, the fifth word, and 1.
- **The manager's wrappers** act on the current rider's (+0x24) track. `ReplayCam_NextCut(frame)` removes a cut at
  the frame, then advances and applies the next one when the frame has reached it. `Replay_GenerateCameraTracks`
  builds all four tracks.
- **Camera calls.** The port makes them through `CamEnv`.
- **Checked in the runner:** 2,000 random sets of tracks (0 to 128 cuts, shared frames, the flag on and off), with
  30 random calls each over the 16 entry points. Every result, every camera call and all the memory were compared.

## Playback control and the manager's life (G12g)

- **More manager fields.** +0x28 the overlay's command, +0x2c the race frame recorded, +0x38 every how many race
  frames a keyframe is kept for good, +0x3c a countdown, +0x40 recording held (the pad streams are full), +0x4c /
  +0x50 frozen, +0x54 the looping replay shown after the race, +0x58 a stream overflowed. Words +0x40a8.. belong to
  the card save. The world, HUD and camera calls go through `ReplayEnv`. The game's state is read through gApp
  (0x337c58) +0x730: +0x68 the focused rider, and +0x1c the race, whose +0x8c players' boarders' +0x460 is the
  chase camera's target. A missing player reads address 0x460.
- **`Replay_Update`, each frame** (not while a menu or overlay is up). It counts the countdown down and clears
  playing-on; command 7 forces mode 3. Then by mode:
  - **0 and 12 (recording):**
    - Not frozen: every 60 race frames (unsigned) it prunes and records a keyframe, and every +0x38-th one is
      marked in use, kept for good. An interval of 0 traps. Otherwise every 10 frames it sorts the contacts. While
      not held, it counts the end frame; it always counts the race frame.
    - Frozen (unless +0x4c): one last keyframe after pruning everything, then mode 13.
  - **2:** every 10 frames (unsigned) it begins a highlight (mode 1, or 3 when it can't), then runs the command.
  - **3:** ends the highlight, then runs the command.
  - **4 to 7:** set mode 3.
  - **8:** runs the command, then mode 3.
  - **9 (the looping replay):** stops the camera script, clears the manager's own playlist and takes the focused
    rider. The playlist gets the kind-1 highlights from that rider's pin and then from the other pin (pin 0, or
    pin 1 when the focused rider is 0), then the other kinds the same way. It builds the camera tracks, puts the
    camera in replay mode and starts playing the playlist (or everything when it is empty). +0x54 is set and the
    mode goes to 1.
  - **11:** deletes node lists 3, 13 and 4, resets the world and builds the camera tracks; mode 12.
  - **10 and 13:** nothing. **Any other mode** runs the command.
- **`Replay_DoCommand`** (the command is cleared afterwards):
  - 1 / 2 go to the start / end;
  - 3 goes a keyframe back (two when the frame sits on one);
  - 4 goes a keyframe on, with nothing past the last;
  - after 1 to 4 the replay pauses there (mode 5, or 6 at frame 0) with a cut fade;
  - 5 plays one frame (mode 4);
  - 6 toggles pause (3) and play (1); 7 pauses; 8 slows (2);
  - 9 and 10 restart (mode 5);
  - none plays a frame unless paused (and sets playing-on).
- **`Replay_Step(jump)`** plays a frame on. At the end of a playlist entry it goes to the next. After the last
  entry the looping replay starts again on the focused rider; otherwise the mode goes to 3. It advances the camera
  cut. Every 60 frames it refreshes the riders, otherwise every 10 it sorts the contacts.
  - Past the end: a looping replay restarts from frame 0; otherwise it stops at the end frame (mode 3).
  - With a jump, the world is put back from the keyframe for the frame, HUD items 7 and 4 are hidden and the
    riders' renders reset. The camera follows the playlist entry's rider (preset 7) in the looping replay, and
    otherwise seeks its track and applies the cut. A playlist gets a cut fade.
- **Starting and stopping.**
  - `Replay_StartPlayback(playlist)` releases the pins and keeps the race's frame as a keyframe (+0x1c; once,
    unless frozen), then plays from the start (mode 7).
  - `Replay_StopPlayback` puts the camera back on rider 0 for a looping replay and releases the pins. Unless
    frozen it puts the race back from the kept keyframe, frees that keyframe (when +0x50 is clear), refreshes the
    riders, and sets mode 0 (13 with +0x50).
  - `Replay_Abort` also cancels the race's sound events.
- **Recording and status.**
  - `Replay_RecordKeyframe(frame, flag)` links a new keyframe after the last one at or before the frame. The first
    keyframe is never compared. It saves the world into it (`Replay_SaveKeyframe`, in the env) and sorts the
    contacts. If the stream overflowed, it frees the keyframe again and answers 0.
  - `Replay_SaveCurrentKeyframe` ends the highlight and keeps the race's frame before a card save.
  - `Replay_GetStatus` fills a 0x78-byte status: the keyframe count, the end frame, the frames of the unpinned
    keyframes (or the kept one; each keeps its place in the list), the rider's highlights and its camera track.
- **The manager's life.**
  - `ReplayMgr_Init(units, two players, seconds)`:
    - a stream of 0x8000 + 0xc00 per unit;
    - as many keyframes as fit in 0xc9800 bytes, up to 16;
    - a keyframe kept every seconds × 3600 / (keyframes - 10) race frames, rounded down to 10 (ten keyframes
      traps);
    - pin slots and quotas of 4 / 3 / 1 with two players, else 2 / 1 / 1.
  - `Replay_Init` allocates each keyframe's stream (filled with 0xdeadc0ed).
  - `Replay_Clear` goes back to recording from nothing:
    - mode 10, and every count, frame and flag zeroed;
    - the pad streams rewound and the pins cleared;
    - the highlights emptied, and only the first ten highlight records put on the free list (the 9th first);
    - the keyframes reset and all free (the last first);
    - the camera tracks cleared.
  - `Replay_FreeAll` frees the streams. `Replay_AddInputStream` / `_RemoveInputStream` add a pad stream (0x800c
    bytes, filled with 0xdeadc0de) or delete one.
- **Checked in the runner.**
  - Playback: 1,500 random managers, with 12 random calls each over 16 entry points. Every result, every world,
    HUD and camera call, and all the memory were compared. Every mode and command occurs. The 17 cases where the
    game traps or never ends stop on both sides.
  - Set-up: 200 random cases, 76 of them through `ReplayMgr_Init`.

## The save to the card and the load (G12h)

- **The writer's state** is kept in the manager from +0x40a8 (see `replaymgr.rs`). `ReplaySave_Init` allocates two
  buffers: a 16 KB chunk (filled with 0xdeadc0de) and a 0xc800-byte scratch buffer (filled with 0xdeadc0ed). It
  starts the sum at 0xdeadbeef.
- **`ReplaySave_Step`** fills the next 16 KB chunk and returns its address and length; when it is done it returns 0.
  Each stage hands over a piece (`ReplaySave_SetChunk`), which adds the piece's length to a byte count and its
  signed bytes to the sum. `ReplaySave_FlushChunk` copies as much of the piece as fits into the chunk. The stages:
  - 0-1: the 0x44-byte header: 0x12300003, the keyframes (not counting the one being written), +0x38, +0x40,
    +0x2c, +0x30, the race's riders (race +0x7c), the four highlight counts and the rider;
  - 2-3: each rider's camera track (0xe24 bytes);
  - 4-5: each keyframe except the one being written. First a 0x18-byte record (0x11111113, the bytes written, the
    frame, pins, use, 0x11111111), then its stream compressed (`Compress_Block` mode 0) after its length;
  - 7: every rider's highlights (`Highlight_Save`, 0x20 bytes each: 0x11111115, kind, the keyframe's frame,
    length, the two scores, the keyframe's place in the list, rider);
  - 8-9: each pad stream: an 8-byte header (0x11111112, the count), then the 0x6664 bytes of words and the 0x1999
    bytes of runs, each compressed after its length;
  - 11: the pins (`ReplayPin_Save`: 0x11111114, how many highlights are kept, their keyframes' frames, then those
    highlights);
  - 12-13: zeros up to 0x7f6d8 bytes;
  - 15: the byte count and the sum. A save is always 0x7f6e0 bytes.
- **The load.** `Replay_LoadStream` refuses a save whose rider count is not the race's. Otherwise it reads back,
  frozen (+0x4c): the header's numbers, the camera tracks, the keyframes in order (`Replay_LoadKeyframe`: the record,
  then the stream unpacked), the highlights from the free pool (each keyframe found by its place), the riders' pad
  streams (`ReplayInput_Load`), and the pins (each kept highlight found by its frame). It then sets command 1 and
  mode 8, and updates each rider's lighting.
- **The career's replay buffer** is 0x80000 bytes at the career block's +0x36fc. `Replay_LoadRiderSetup`
  (0x166870; first named `Replay_SaveRiderSetup`) copies the race's set-up from the buffer's first 0x920 bytes into
  the career's +0x27dc: `EAMem_Copy` takes the destination first. `Replay_LoadFromBuffer` loads the replay that
  follows the set-up and frees the buffer.
- **STANDIN.** In two places the game writes stack words it never set, and the port writes 0:
  - a highlight with no keyframe saves its second word from the stack;
  - a pin with fewer than four kept highlights leaves the unused frame words as the stack held them.
- **Checked in the runner:** 12 random managers (with every pin slot filled and every highlight on a keyframe).
  Each save was compared chunk by chunk (384 chunks), with the writer's state and all the memory after each step.
  The game's saved bytes were then loaded back on both sides, directly or (every other save) through the career's
  buffer, and compared again; then the buffers were freed. The runner's step limit is now set per runner
  (`step_limit`), because the game's compression of a pad stream runs past five million instructions.

## The world snapshot's orchestration (G12i)

- **What this covers.** The order and bookkeeping of a keyframe's snapshot, on the world's node lists (E9k). Each
  node class's own snapshot (its virtual at +0x44), load (+0x4c), object (+0x5c) and load constructor go through
  `SnapshotEnv` and are left to their own rows.
- **`Replay_SaveKeyframe(keyframe, frame, flag)`:**
  - clears the overflow flag (+0x58) and empties the stream;
  - writes the game's random state (24 bytes at 0x345e70: `Rand_SaveGameSnapshot`) and sets the frame;
  - lets each node of lists 2, 5, 7 and 8 save itself (the table at 0x38aac0);
  - with the flag, first deletes the nodes of lists 3 and 4;
  - saves lists 3 and 4 (`Replay_SaveNodeList`: a count and a zero word, then each node), then the dead objects
    (`Replay_SaveObjRefs`: list 13's count, then each node's object as an instance number).
- **`Replay_RestoreKeyframe`:**
  - deletes the nodes of lists 3, 13 and 4, rewinds the stream and reads the random state back;
  - lets the nodes of lists 2, 5, 7 and 8 load themselves;
  - makes lists 3 and 4 again (`Replay_RestoreNodes`: the count, each node, then the list's pending nodes merged);
  - makes the dead objects again (`Replay_LoadDeadNodes`: a 0x2c-byte `cDeadNode` on each saved instance; 0 past
    the course's count).
- **`Replay_RestoreNode`** peeks the saved type (the header's third word). The type picks the size, the pool name
  and the class's load constructor, called as (node, list, stream) on a pool node filled with 0xdeadc0de. There
  are 31 types, from cRollerNode (0) to cRestoreNode (0x3ee); the table is `replaymgr::NODE_TYPES`. An unknown type
  makes nothing and reads nothing, so the rest of the stream is then read out of step, in the game too.
- **Checked in the runner:** 2,000 random cases on the world's 14 lists, with fake nodes whose virtuals are
  logging stand-ins. Saves, node-list saves, object references and merges were checked with the stream filling up
  and overflowing. Restores (whole keyframes, node lists, single nodes and dead nodes) were checked from made-up
  streams holding every node type, with unknown types in the single restore. The load constructors were stand-ins
  that log and take their record. Every call and all the memory were compared.

## The world nodes' snapshot saves (G12j)

- **The common pattern.** Each class writes its parent's part, then a block of its own words. Before writing, the
  block's first two words are set to its length and a tag, so the tags end up in the node too. References follow:
  - instances as numbers (`Replay_WriteNodeRef`);
  - a link to the node's own instance: the instance number, its +0xe8 half word, and whether its +0xe4 is this
    node (`Replay_WriteLink`);
  - data as offsets from 0x34649c.
- **The base chain.** `cBxTypeObjNode` (0xc bytes, 0xdeadbefc) and `cBxSortObjNode` (0x10, 0xdeadbefb) come first.
  The dead node (`cDeadNode`, `cRestoreNode`, `cMovieNode`) adds its instance link.
- **One block on top of the dead node's part** (offset, length, tag in `snapshot::SIMPLE`): counter 0x2c / 0x10 /
  0xdeadbefe, timer 0xdeadbf21, rail 0xdeadbf13, the boosts at +0x30 (0x40 bytes: 0xdeadbefa, lap 0xdeadbf0d,
  z 0xdeadbf26), tex flip 0xdeadbf1d, cracked 0xdeadbeff, crowd box 0xdeadbf00, mesh anim 0xdeadbf10, trick
  trigger 0xdeadbf20.
  - The roller adds a 0x90-byte header (`Snapshot_WriteHeader`, 0xdeadbf17).
  - The UV scroll adds its data as an offset.
  - The random boost's block (+0x100, 0x90, 0xdeadbf16) sits on the boost's part.
  - The tube end's block (+0x100, 0xc0, 0xdeadbf29) adds the 0x18 bytes at 0x31d338, which all tube ends share.
- **Others:**
  - the debounce adds one word;
  - the fence and the flag write their data offset before their block;
  - the UV-scroll tex flip saves the UV scroll at +0x24 first;
  - the emitter and the collide emitter (on the typed node) write their instance and data, then the particles'
    state (`ParticleNode_SaveState`: two words);
  - the rail manager writes 0x200 bytes of rail states;
  - the script writes 0xb0 bytes with its program counter made an offset from 0x3464a0 for the write (and put back
    after), then its rider's +0x460 (or -1), its data offset and its instance;
  - the anim object (its dead node at +0x30) writes whether it has an overlap (then 0x10 and 0xc bytes of it), then
    each key state (`AnimObjectNode_SaveState`: 0x48 bytes, 0xdeadbef2, and its key as an offset from the
    course's +0x6c);
  - the anim combo, delta and tex flip build on the anim object. The combo writes its +0x78 × 64 bytes; the delta
    and the tex flip use the saves at 0x199ee0 and 0x19a940;
  - the spline path writes its block, instance and +0x30 × 32 bytes, then its spline (`Spline_SaveSnapshot`).
- **Checked in the runner:** 20,000 random nodes, about 600 for each of the 32 save functions, with their pointers
  aimed at real places. The streams were sometimes nearly full or already overflowed (3,989 ended overflowed). The
  stream, the node and the control were compared byte for byte. 

## The world nodes' load constructors (G12k)

- **The common pattern.** Each builds its parent from the stream, sets its vtable, reads its words back over the
  saved block, and calls the world's set-up. The set-up calls go through `LoadEnv`, with the exact arguments.
- **The base.** `cControllerNode` 0x13a888 brings back the sorted node and its instance by the link. The link
  re-points the instance at the node when it was the node's, and restores the instance's +0xe8 half word.
- **The world set-up each class calls:**
  - the rail enables itself (`RailNode_Enable` 0x1403c0) when +0x3c is set, else disables itself
    (`RailNode_Disable` 0x140400);
  - the boosts and the trick trigger make their box, set it from the instance's corners and fit it into the
    broadphase; the trick trigger also finds its rider (`Race_GetRider`);
  - the UV scroll reads its data offset and registers itself;
  - the tex flips (tex flip, cracked, UV-scroll tex flip) make and start their `cTexFlip`;
  - the fence and the flag read their data offset before their shape is made;
  - the crowd box starts itself;
  - the emitters and the particle node make their particle systems (`ParticleSys_ReadReplay`: two words back);
  - the collide emitter counts itself in the table at 0x31d498 by its kind (+0x50);
  - the movie opens its video and sets its instance's +0xe8 bit 2 (clearing bit 1);
  - the rail manager makes itself the one at 0x31d4b0 and applies each of its 0x80 rail states not -1;
  - the script puts its program counter back from 0x3464a0, its data back from 0x34649c, and its rider from the
    race's riders (+0xc4) by number (-1 none);
  - the spline path computes its instance's box corners less its position (PS2 floats, w 1.0 less the position's
    w), allocates and reads its point array, then its spline (`SplinePathNode_Load`, which asks
    `Spline_GetLength`).
- **`cDeadNode` 0x13af38(instance, flag)** is a controller on list 13 (type 5) for an object that has gone. It
  clears its instance's +0xe8 bits 1 and 4 to 7, sets bit 2, and sets 0x800 with the flag.
- **The roller and the anim objects:** G12m, below.
- **Checked in the runner:** 12,000 random cases. A node of each class was saved by our checked save and loaded
  into a fresh node by the game's constructor and ours, about 400 per class. The set-up was logging stand-ins, and
  every call and all the memory were compared.

## The riders' and AI's snapshots (G12l)

- **A rider** (`cPlayer` / `cComputer`; its first word points at its boarder data) saves:
  - the boarder data's 0x450 bytes at +0x10 (0xdeadbef8);
  - two words: 1, and a stack word the game never sets (STANDIN: the port writes 0);
  - its animation controller (+0x46c0: 0x24 bytes, 0xdeadbef6, then three layers);
  - its air prediction (+0x57b0: 0x60, 0xdeadbef9) and its score (+0x5820: 0x24c, 0xdeadbf1f);
  - fifteen motion and control blocks, each tagged by its class (`snapshot::RIDER_PARTS`). The wipe-out motion adds
    its 0x90-byte header;
  - its AI path and line as indices (`AIP_GetPathIndex` / `AIP_GetLineIndex`, two shorts);
  - its own part at +0x860: the player 0xc bytes (0xdeadbf11), the computer 0x40 (0xdeadbefd).
- **Loading a rider** reads the same back. It also:
  - remakes the score's trick name (`Score_FormatTrickName`);
  - looks up +0x430 again from the boarder's three unsigned shorts at +0x440 (`FUN_00260cb8`);
  - turns the path and line back into pointers (0x337fc4 + path × 0x3c, 0x337fbc + line × 0x40);
  - for the computer, takes its 8 bytes at +0x8a0 from the table at 0x31d318 by its +0x89c (the decompile hides
    this).
- **An animation layer** saves its clip count and lets each sequence on its ring save itself (vtable at +0x34, slot
  +0x24). The load empties the layer, then makes each sequence (`BdrSeq_Create`), has it load itself (slot +0x2c)
  and puts it last on the ring.
- **A boarder sequence** saves the skeleton sequence's 0x2c bytes (0xdeadbf18), then its 0x40 (0xdeadbef7), then
  each tree's two words. The load destroys its trees and remakes each one (`SkelAnim_CreateTree` on 0x3211a8) from
  the two words' low halves: (first << 16) | second.
- **The AI** saves 0x18 bytes (0xdeadbef1) and the node counters. The load makes its handlers again
  (`Race_CreateHandlers`) for +0x20 and then the old +0x1c.
- **The AI world** saves its 0x6c cell numbers. The load releases the active cells and resets the instances, then
  sets each slot: -1 empty, otherwise the course's cell (0xffff for none), marked in use and woken.
- **Checked in the runner:** 1,500 random cases, about 300 of each kind, saved and then loaded back on both sides.
  The fake animation sequences' virtuals log, as do the world calls. Every call and all the memory were compared,
  except the stack word, which is taken from the game.

## The last load constructors (G12m)

- **The roller:**
  - brings its controller back and makes its rigid body (+0x30), its sphere tree (+0x150, with the `bxSphereTree`
    vtable) and its box (+0x2c0);
  - reads its 0x90 bytes at +0xc0 and the body's 0x90-byte header back;
  - copies the instance's sphere tree and places it where the body is (`RollerNode_PlaceFromBody`, then
    `Mat4_FromPosQuat`, then `SphTree_SetTransform`, which pass values through stack buffers; the port's env
    passes the values);
  - sets its box from +0x1e0 less and plus the tree's radius, asked three times for each (PS2 floats), and fits it
    into the broadphase.
- **The anim object:**
  - brings its controller back at +0x30 and reads its 0x30 bytes;
  - when it had an overlap, makes it (`AnimObjectNode_CreateOverlap`) and reads it back;
  - gives each animated part of its instance's model a key frame: a pool block of n × 0xd0 + 0x10 bytes holding n,
    with each frame cleared, made identity from 0x346440 and keyed;
  - reads each frame's saved state back and applies the frames.
- **The overlap** is 0xe0 bytes with an identity rotation and a cube around the instance's position. The cube's half
  side is the largest distance from the position to the instance's box corners, on each axis (at least 1e-10). Its
  box goes into the broadphase, and the instance's +0xe8 bits 5 and 6 are cleared.
- **`AnimFrame_SetKey`** gets (the frame, the part's key) and the node's +0x24 as a float in f12. The decompile shows
  the arguments wrongly.
- **The combo, delta and tex flip** build on the anim object: the combo's array (+0x78 × 64 bytes), the delta's
  word, and the tex flip's `cTexFlip`.
- **Checked in the runner:** 6,000 random nodes, about 1,200 per class. Each was saved by our checked save, then
  loaded on both sides. Instances' models had 0 to 4 parts, some animated. The stack-buffer calls were compared by
  value, and every call and all the memory were compared.
- **Every world node type a keyframe holds can now be made again.**


## The replay's controls and the race menu's replay page (G12n)

`tricky_game::replayui`, on the game's bytes. The drawing is not ported yet.

- **`cReplayControlOverlay::Update`** runs each frame while a replay plays:
  - the panel slides 6 a frame towards its mode's y: 480, +0x354 or +0x358 (`ReplayControl_Slide` 0x1ec728);
  - a message is held for 60 frames, then fades by 0.016 a frame; the slide offset +0x474 goes back to 0 by 5;
  - nothing more while it is hidden (+0x49c), paused (mode 7) or highlighting (mode 2).
- **The pad** (`DAT_003381e0`: +0x10 held, +0xc last frame):
  - up / down change the panel's mode, up to +0x48c, with the scroll sound;
  - the four held bits 0x200000..0x1000000 turn the chase camera (`Camera_AdjustChase` 1.0, 6/2, 7/3, 8/0, 9/1;
    the first number with 0x200 held), only in preset 8;
  - with 0x200 held, L1 / R1 jump to the start (command 1) / the end (2);
  - otherwise L1 / R1 step back (3) / forward (4) and 0x400 fast-forwards (5). These repeat while held: the
    button is kept in +0x494, and the count in +0x490 lets it act again only after 15 frames;
  - 0x800 pauses (6); 0x100 is the next camera preset (twice past preset 8 without the full controls); 0xc cuts
    the camera to the next / previous rider (with two or more riders);
  - only with the full controls (+0x4a8, `ReplayCam` cuts on at 0x32ca4c): 0x20 removes a cut (message 0x14f, or
    0x15b at frame 0), 0x40 begins a highlight (8; message 0x16d when there are already ten),
    0x10 drops one (0x16c), 0x100000 / 0x80000 go to the highlight before (10) / after (9), 0x20000 plays the
    highlights not yet ended (6, the overlay's own playlist +0x430), 0x40000 goes back to the plain replay, and
    0x8000 inserts a cut (0x149, or 0x153 when it is full);
  - a command that needs the replay paused (mode 3) gives 7 (pause) instead.
- **After the pad:** the command goes to the manager's +0x28. The sound stops on 6 and resumes otherwise (with no
  command, only when paused). The status is read into +0x378, and the end frame shown is capped at 1800 (scale
  180).
- **`Reset`:** the full controls follow the cuts flag. The replay follows the rider whose pad is the replay's,
  unless it is a loaded one (+0x4c). Its cut is applied, the status read, and the command set to 1. The panel is put
  at mode 1 (full) or 0.
- **The race menu's replay page:**
  - three items: watch, save (state 4 for a loaded replay, else 5) and back;
  - `RaceMenuOverlay_BeginSaveReplay` keeps the current keyframe and the focused rider, then starts the card's
    save of 0x80000 bytes. `_EndSaveReplay` puts the views and the rider back.
- **`cReplayOverlay` (the card save's page):**
  - vf3 `SaveBegin` takes the settings block's CRC (0x32db6c, 0x920 bytes, with its own word at 0x32dea0 zeroed
    first) and names the file `%s-%s%04d`: the game code, then REP and the slot +0x38 plus one, or SET and 1;
  - vf4 `SaveNext` writes each `ReplaySave_Step` chunk;
  - vf6 and vf5 end and abort the save.
- **The two replay sounds** 0x214428 / 0x214650 queue ids 0x7d / 0x7f (`sndqueue`). What `SndSys_PlayPending`
  returns is kept in +0x6e48.
- **Checked in the runner** on random memory, with every call out logged on both sides and all memory compared:
  - the pad and the replay states: 60,000 updates;
  - 5,000 resets, 5,000 save begins, 3,000 save ends and 20,000 menu updates;
  - the card page: 200 begins, 2,000 chunks, and 300 each of done and abort;
  - 10,000 sounds;
  - the replay manager's calls are logged stand-ins with scripted answers (they were checked in G12d–G12g). The
    overlay's own playlist runs the game's code;
  - the tests caught every mutant tried except two that cannot change the result: the second `SeekBack`, and the
    slide's `<` vs `<=`.

## The replay's drawing and leftovers (G12o)

`tricky_game::replayui`. The drawing is ported as the draw calls the game makes (`DrawEnv`), with every float
computed as the game computes it.

- **`cReplayControlOverlay::Draw`** draws nothing during a cut-scene (the race's +0x34) or while the overlay is off
  (+0x498). It sets the renderer's state with three of its virtuals (the object at the game's +0x724, +0x10 vtable,
  slots +0x1e4, +0x1fc, +0x1c4), then:
  - **while the panel moves or is up:** sprite 0x37 gives the frame's scale, 340 over its width.
    - Not hidden: a dark box (0.88 / 0.1) from the panel's y down to 480, and a box inside the frame (from 0.455 of
      its width less 20, between 0.26 and 0.79 of its height).
    - Hidden: one darker box over the whole screen, set through the font colour words at 0x337fe8 directly.
    - Then the frame's two halves at x -20 and 320, 26 above the panel's y.
  - **while the panel is still:** the up arrow (sprite 0x61) and the down arrow (0x65 at the top mode, else 0x64),
    left of "more" / "less" (0x16a / 0x16b). Their x is 42 (or 41) left of that word's width.
  - **then** the camera line (`HUD_DrawReplayCamInfo`) with the panel's other drawings. With the full controls:
    0x1edef0, 0x1ed490, the camera line, 0x1eee00. Without: the camera line, then `HUD_DrawStartSelectPrompt`.
    Those four are not ported yet.
- **`HUD_DrawReplayCamInfo`:**
  - the label box at y 18 (style 6): "end of replay" (0xeba) at the end, else the overlay's title (+0x3f0);
  - while the panel moves or is up, the camera preset's name 47 above the panel. The names are 1: 0x152, 2: 0x14a,
    3: 0x14b, 5: 0x14c, 6: 0x14e, 7: 0x150, 8: 0x151, 10: the "Artist" format at 0x394010, others: 0x14d. The chase
    camera's name (0x32db74 + 0x84 × the index `Camera_GetChaseParams` gives) is centred at 320;
  - "highlighting" (0xdec, scale 0.9) while a highlight is being made;
  - the fading message, centred in the panel, its alpha written to 0x337fe8;
  - "more" / "less" right-aligned at the panel's right less 20.
  - Hidden, its three colours are greyed.
- **`RaceMenuOverlay_DrawReplayMenu`:** the frame at 165 (style 15) and the three items (0x187, 0x161, 0x1db, all
  enabled, from 0x3940b8). The list is at (320, 240), scale 0.92, with the selection +0x43c. Then the title box
  (0xe53) at 155.
- **`ReplayMgr_Free`:** `Replay_FreeAll`, then the manager deleted with flag 1. The call before it (0x18ebc8) and
  the two arrays' destructors (0x195710 over 16 records of 0x24 at +0x3e68, 0x1956f0 over 40 of 0x1c at +0x3968,
  both with 0) do nothing.
- **`Replay_RefreshRiders`:** each rider's pose is blended and updated when its +0x46b0 is set, its broadphase box is
  updated, and +0x42c is set from +0x16c. Then the broadphase and the contacts are sorted.
- **`Camera_SetReplayMode`** (on, rider):
  - the flag goes to 0x337774, the camera control 0x3368b0 is reset and 0x337770 is cleared;
  - when on: the chase camera goes on the rider's camera target (0, 0, 10.0, 1.0). That is rider 0 when the index is
    not below the race's +0x7c plus +0x84. Then preset 7.
- **`cReplayEndRaceHandler`'s Enter / Update / Exit** are empty.
- **Checked in the runner:**
  - 30,000 draws, 30,000 camera lines and 2,000 menu pages, every draw call compared with its float arguments bit
    for bit;
  - 400 frees, 3,000 rider refreshes and 3,000 replay-mode switches, with all memory compared;
  - the floats follow the port's PS2 type: the runner rounds to nearest, as the other runner tests set it (F4c);
  - the tests caught every mutant tried.

## The replay panel's larger drawings (G12p)

`tricky_game::replayui`: the four drawings `Draw` hands on to, as their draw calls (`PanelEnv` adds the bar, the line,
the filled rectangle and the renderer's screen-space switch 0x1ee660 / 0x1ee6e8, which is left as a call). With the
full controls `Draw` runs the strip, the timeline, the camera line and the key; without them, the camera line and
the start / select legend.

- **The strip (0x1edef0, `ReplayControl_DrawStrip`)** shows the whole replay, 409.6 wide and 9 high. Its x is
  (+0x36c × 0.8) − 204.8 and its y is (the panel's y × 0.9333) + 26.0465. It draws:
  - each camera cut from the second as a 6-high tick (two lines); the one at the frame while paused is in
    0.7 / 1 / 1, the others are written straight into the font colour (0.8, 1, 0, 0);
  - the rider's highlights as bars, clipped at the end frame;
  - the highlight being made;
  - the frame around it, and the playing head (sprite 42, 5 over its width wide) at the frame, coloured when a
    highlight starts there.
  - The end frame and the highlights' frames and lengths are converted as **unsigned** words (the halve-and-double
    for the top bit); the frame is converted as signed.
- **The timeline (0x1ed490, `ReplayControl_DrawTimeline`)** shows a window of +0x460 frames.
  - It starts at +0x464 plus the slide +0x474, which `Update` takes back to 0 by 5 a frame.
  - When the frame runs past the window's end, the window jumps to end there less +0x468, and the slide is set to
    1 − +0x468. Before the window's start it starts at the frame less +0x468, with the slide +0x468 − 1.
  - It is kept within the end frame and above 0.
  - The bar is 328.4 wide inside a 358.4 frame. Its x is ((+0x36c × 0.8) + 15) − 179.2 and its y is (y × 0.9333) +
    68.923. Its height is 0.85 of sprite 42's.
  - It draws the highlights inside the window, the highlight being made, and the cuts (full controls) as ticks
    marked by a 4 × 4 square. Their x is truncated to a whole pixel (`cvt.w.s`). It also draws the keyframes as
    ticks, and the head (7 over the sprite's width) while the frame is inside the window.
- **The key (0x1eee00, `ReplayControl_DrawKey`)** is drawn while the panel is above its +0x354, 70 below it. It has
  13 pad sprites from the table at 0x33f0b0 (`KEY_SPRITES`) and 18 labels from 0x33f150 (`KEY_LINES`: x, y, the
  string, the kind, the line's length, a bend, a sprite and its corner).
  - Kind 0 is centred with a line down.
  - Kind 1 sits left of x with a line left.
  - Kind 2 sits right of x with a line right.
  - Some labels have a sprite beside the text (0x2f) and a bend at the line's end.
  - The text scale is 0.9, or 0.7605 when the game's +0x58 is 2 or 3. The lines are 3 wide.
- **`HUD_DrawStartSelectPrompt`:**
  - two rows of button sprites, 35 and 83 below the panel;
  - the six names right-aligned at 125, 340 and 568, raised by half the font's height;
  - "SELECT" and "START" (0x394048 / 0x394050) centred under the first and second columns (scale 0.7; the second
    is measured at 0.8);
  - with the game's +0x58 at 4 the columns are 190 / 450 and the sprites 150 / 375 / 570.
- **Checked in the runner:** 30,000 strips, 30,000 timelines, 10,000 keys and 10,000 legends. `Draw` and the camera
  line run them all for real. The memory is random: windows and scales, end frames with the top bit set, highlights
  across the window's edges, camera tracks, and the font's height. Every draw call's floats were compared bit for
  bit. The tests caught 11 of 12 mutants; the one left (`<` vs `<=` on the strip's end clip) cannot change a
  result.

## The overlays' constructors, destructors and the screen-space switch (G12q)

`tricky_game::replayui` (`LifeEnv` for the bases' constructors and destructors).

- **`Gfx_BeginScreenSpace`** 0x1ee660 and **`Gfx_EndScreenSpace`** 0x1ee6e8 (inside 0x1edef0's range in the index
  until now) call the renderer's virtuals with only the renderer:
  - begin: +0x8c, then +0xb4 with the view (0, 0, 512, 448, 0, 1000) in f12..f17, then +0x10c;
  - end: +0x94.

  The strip and the timeline now run them.
- **`cReplayControlOverlay::cReplayControlOverlay`** 0x1eb5d0:
  - the backdrop is set up with the control's vtable (0x394338) and its playlist is cleared;
  - the title (0xeb8) is copied to +0x3f0 and measured (the box's third word to +0x370);
  - the fonts are the game's +0x60 and +0x5c (+0x348 / +0x34c), with the first's height at +0x374;
  - the rider is -1, the mode 1 of 1, and the rest 0.

  Its destructor puts its vtable back and runs the backdrop's.
- **`ReplayOverlay_Init`** 0x1ef610: the backdrop at +0x44c (+0x42c), the save overlay set up, the vtables
  (0x394170, and the backdrop's 0x3940d8), five items, and no slot.
  - Its only caller (the overlay manager, 0x1a72d4) passes 1, the complete object. With 0 the game would point the
    backdrop at a copy of a construction vtable on its own stack.
- **`~cReplayOverlay`** 0x1f05a0: the vtables go back through the save overlay's (0x38f9b0 / 0x38f918).
  - Then, whatever the flags, the backdrop's vtable pointer is left at the destructor's stack copy of a construction
    vtable (`sw sp` at 0x1f072c), before the memory-card manager's destructor. The backdrop's destructor (flag 2) and
    the delete (flag 1) follow. The overlay manager passes 3.
  - The port takes that stack address from the env. That is a STANDIN for the address only; the runner's is its
    stack top less 384.
- **Checked in the runner:** 500 of each constructor and destructor (the destructors with flags 0..3), with every
  call and all memory compared. The drawing test now runs the screen-space pair for real. The tests caught the 4
  mutants tried.
- **With this, every G12 split is done:** the instant replay from its byte streams to its last overlay is ported and
  checked.
