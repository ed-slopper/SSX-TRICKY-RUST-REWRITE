# Instant replay

How the game records and plays back a race. Ported so far: the byte streams and the pad streams (board row G12a;
`tricky_game::replay`, checked: `tests/replay_streams.rs`). Keyframes, the node save and restore, pins and
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
