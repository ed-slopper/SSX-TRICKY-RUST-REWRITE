# Instant replay

How the game records and plays back a race. Ported so far: the byte streams and the pad streams (board row G12a;
`tricky_game::replay`, checked: `tests/replay_streams.rs`), the replay codec (G12b, G12c: files.md), the keyframe
list, seeking and playlists (G12d; `tricky_game::replaymgr`, checked: `tests/replaymgr.rs`), the highlights
and pins (G12e; the same module, checked: `tests/replay_highlights.rs`), the camera tracks (G12f; checked:
`tests/replay_camtracks.rs`), and playback control and the manager's life (G12g; checked:
`tests/replay_playback.rs`). Keyframes, the node save and restore, pins and
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
- **Not ported yet.** `Highlight_Save` / `_Load` and `ReplayPin_Save` / `_Load` go with the memory-card save. When a
  highlight has no keyframe, its saved record's second word is whatever was on the stack.
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

