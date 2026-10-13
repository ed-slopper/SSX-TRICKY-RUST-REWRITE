# Instant replay

How the game records and plays back a race. Ported so far: the byte streams and the pad streams (board row G12a;
`tricky_game::replay`, checked: `tests/replay_streams.rs`), the replay codec (G12b, G12c: files.md), and the keyframe
list, seeking and playlists (G12d; `tricky_game::replaymgr`, checked: `tests/replaymgr.rs`). Keyframes, the node save and restore, pins and
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
- **Camera tracks.** A track's cuts are found by frame: forward, the first cut after the frame; back, the cut before
  a cut at the frame, or the last cut before it.
- **Walks that never end.** On a circular list the game's walks never end. The port panics after a million steps
  with a message saying so.
- **Checked in the runner:** 3,000 random managers (lists, free lists, pins, highlights, playlists, camera tracks
  and pad streams), with 30 random calls each over the 32 entry points. Every result and all the memory was
  compared after each call. The 57 walks that never ended were checked to run past the game's step limit too.

