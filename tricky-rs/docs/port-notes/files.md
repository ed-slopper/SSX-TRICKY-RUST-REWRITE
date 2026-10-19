# Files: the disc, BIG archives, compression

## The disc (row E2)

Proven (`lib-file`, host code on the PS2; ported 1:1 anyway in `tricky-data/src/disc.rs`):

- `CdFs_Mount` 0x2c9970 runs once (from 0x2cc2c0, at most 1000 files, 0x100 bytes of path per entry, entries
  0x10c bytes). It reads the primary volume descriptor (sector 16), takes the root directory record at +0x9c
  (extent +2, size +10) and walks the tree with `CdFs_ScanDir` 0x2c9200 from the root (the folder prefix at
  0x3bb938 is empty).
- `CdFs_ScanDir`: skips the `.` and `..` records, a zero length byte ends a sector's records. Directories
  (flags +0x19 bit 2) are walked at once with `NAME/` added to the prefix; a file's name loses its last two
  characters (`;1`) and then a trailing `.`. Entry: `CdFs_HashPath(path)`, extent, size, path
  (`/DATA/CHAR/MDLPS2.BIG`).
- `CdFs_HashPath` 0x2c9190: ELF-style hash (`h = h*16 + c; g = h & 0xf0000000; if g: h ^= g>>24 ^ g`) taken
  from the terminating zero back to the second character: the leading `/` is not hashed.
- The table is sorted with the C library's `qsort` 0x2fb5c8 (Bentley-McIlroy) by the 32-bit wrapping hash
  difference (0x2c9460). An equal pair switches to path order: sorted again with `strcmp` (0x2c9420) and
  searched with `strcmp` (0x2c9440). The shipped disc has no collision (260 files).
- `CdFs_FindFile` 0x2c9cc0: drops one leading `/`, turns `\` into `/`, upper-cases a-z, puts `/` in front,
  hashes it and `bsearch`es 0x2fa258 the table (comparison 0x2c9488: key hash minus entry hash). Returns the
  1-based entry number.
- File opens go through EA's async file system (0x2c9018 → 0x2ca7f8 → 0x2cb328): BIG archives mounted with
  `BigArchive_Open` are searched first (`BIG_Lookup` 0x2cbdc0), then the disc (0x2cc4b8). `host0:` paths
  (0x2cc368) are the dev kit's and never used on a retail disc.

Checked: the table of `SSX Tricky (USA).iso` equals an independent ISO 9660 walk (260 files, extents and
sizes); `data/char/mdlps2.big` read through it is byte-identical to the copy extracted before.

## BIG archives (row E2a)

Proven (`lib-big` and the async file system; ported in `tricky-data/src/big.rs`):

- `BIG_Identify` 0x2cbc88: big-endian u16 0xc0fb → `C0FB` (1), `BIGF` → 2. `BIG_HeaderSize` 0x2cbd08: `C0FB` u16
  at +2 plus 4, `BIGF` u32 at +0xc (both big-endian).
- `BIG_Lookup` 0x2cbdc0: `C0FB` directory from +6 (u24 offset, u24 size, name), `BIGF` from +0x10 (u32, u32, name);
  names end with a zero; matched with `stricmp` 0x2fcc98, or by entry number when no name is given. Offsets
  are from the archive's start. `BIG_GetFileData` 0x2cc0a8 takes offset 0 as "not found".
- `App_OpenBigFiles` 0x17bdf8 mounts, in order: `data/audio/speech.big`, `data/audio/audio.big` (if there),
  `data/char/mdlps2.big`, `brdps2.big`, `anm.big`, `texps2.big`, `data/audio/music.big` (if there).
  `FileSys_MountBig` 0x2c8f50 reads the file's start; `FileSys_OnBigHeaderRead` 0x2caca8 reads the whole
  directory again if it is longer, and puts the archive at the head of the mount list (newest searched first).
  Only the directory stays in memory.
- `FileSys_RunQueue` 0x2cb328 opens a path: no `|` → the disc first (`FileSys_OpenOnDisc` 0x2cc4b8 →
  `CdFs_FindFile`), then, only when the open mode has bit 0, every archive; `|name` → every archive, not the
  disc; `archive|name` → the archive mounted under exactly that path (`strcmp`).
- `FileSys_MakeDevicePath` 0x2cc368 strips `cd:` and handles `host0:` (dev kit only) and prepends the current
  folder (0x34eef8) to a path that does not start with `\`, `/` or `~`; on a retail disc the current folder is
  the root. Inferred: our `Files::open` does not keep a current folder.

Checked: all 793 entries of the four character archives read through `Files` equal the archives extracted
before (sizes and byte sums); the three audio archives mount (1146 entries in all).

## Compression (row E2b)

Proven (ported in `tricky-data/src/compress.rs`):

- A packed file starts with a type byte and 0xfb. `Compress_Decode` 0x2c2de0 dispatches on the type with its
  low bit cleared: RefPack 0x10/0x90 (`RefPack_Decode` 0x2c31b0), Huffman 0x30/0x32/0x34/0xb0/0xb2/0xb4
  (`Huff_Decode` 0x2c3730), BTree 0x46/0xc6 (`BTree_Decode` 0x2c3540), 0x2c2d18 for 0x18/0x1a/0x1c/0x98/0x9a/0x9c,
  and a registered codec (gp-0x7fc8) for 0x1e/0x9e.
- `Compress_UnpackedSize` 0x2c2fb0: a 3-byte size (4 bytes with bit 7) at +2, whatever the low bit says.
  `RefPack_Decode` instead skips one more size field when the low bit is set (0x11fb: size at +5) and returns
  that field.
- A second, older Huffman set sits at 0x240ef8–0x242200 (`Huff_DecodeBlock`, `Huff_BuildTree`, ...).
  `Huff_IsCompressed` 0x241d28 returns 100 for the headers 0x30fb–0x35fb and 0 for anything else;
  `Huff_GetUnpackedSize` 0x241dd8 reads the 3-byte big-endian size at +2 (+5 when header bit 0x100 is set). Nothing
  calls either directly (inferred: they sit in a codec table).
  - The replays use this set (`Decompress_Block` 0x241e28 -> `Huff_DecodeBlock` 0x240ef8; row G12b). This is
    the newer `Huff_Decode`'s code without the 4-byte size, so a type with bit 7 reads its size from the bit
    stream like the others. Bit 7 only stops 0xb2 and 0xb4 from summing. The port is
    `compress::huff_decode_block`, with `bytes_read_be`, `huff_block_is_compressed` and
    `huff_block_unpacked_size` for the three header helpers.
  - Checked in the runner (`tests/compress.rs`): the game's decoder and ours give the same bytes on the Huffman
    test's streams, 1,500 of them plus the long runs, with half of each type's streams marked with bit 7. The
    helpers were checked on 20,000 random headers.
  - The encoder half (row G12c; `tricky_data::huffenc`, checked: `tricky-game/tests/huffenc.rs`):
    - `Compress_Block` 0x243a60(src, size, dst, options) is what the replay save calls. The options' first word
      is the delta mode: 1 delta-codes a copy once (`Delta_Encode` 0x241e48: each byte less the one before), 2
      does it twice. The type it writes, 0x32fb or 0x34fb, makes the decoder sum the bytes back. It then calls
      `Huff_Encode` on a new 0x31ec-byte context from `Compress_Alloc`, which is not cleared.
    - `Huff_Encode` 0x2438c0 writes the header: the type, then the size in 24 bits. When the unpacked size
      differs, the type is 0x31fb / 0x33fb / 0x35fb and both sizes are written. A mode outside 0..2 writes type
      0. Then come `Huff_Analyze` (flags 0x39, codes up to 15 bits) and `Huff_WriteTable`.
    - `Huff_Analyze` 0x2426f8, the first pass, counts each byte, each delta from the one before, and each repeat
      run (a run of a byte equal to the last, up to 30,000 more).
    - It then picks the clue, the escape: the start of the longest run of unused byte values. The values after
      it stand for repeat runs of 1, 2, ... Flag 0x20 makes the rarest byte the clue when every value is used.
      Flag 2 clear keeps one clue value, and flag 1 then none. Flag 4 clear would also give the second-longest
      run of free values to near deltas (a byte as a small step from the last).
    - It builds the tree, then drops each run symbol that saves less than half its run in bits
      (`Huff_RunCost` 0x242210: the cheapest split of a run over the run symbols, or the clue and a number).
    - The second pass counts again the way the table will be written. Each repeat run goes to the cheapest of:
      the byte repeated, the clue and a number, or the run symbols. The tree is built again
      (`Huff_BuildTree` 0x241ff8: the two lightest joined each time, depths by `Huff_AssignDepths` 0x241f78).
    - Codes over 15 bits are cut down. The two last found at the deepest level go to one past the deepest code
      still under 15, which moves down with them, and the other deepest goes up one level. Canonical codes
      follow, by length then value.
    - `Huff_WriteTable` 0x243368 writes the clue in 8 bits, then the code count for each length as a number
      (`Huff_WriteVarNum` 0x242320: a 1 after b zeros, then the value less its base in b + 2 bits). For each code
      it writes how many free values to step past. Then the bytes: runs, near deltas, plain codes, and the clue's
      own value escaped (`Huff_WriteClueSymbol` 0x2424d0: the clue, 0, then a 0 bit and the byte). It ends with
      the clue, 0, a 1 bit and padding.
    - `Huff_WriteBits` 0x241e88 collects bits from bit 23 of a word downwards.
    - Nothing it reads is left over from the uncleared context except one word: the second clue (+0x2dd0) takes
      the old clue word, and only the near deltas use it. `Encoder::new` takes the starting bytes.
    - An empty block has one code, the clue, one bit long. The decoder then keeps reading code lengths past the
      end, in the game as in our decoder.
  - Checked in the runner:
    - `Huff_Encode`: 1,200 blocks and the long runs, on contexts full of leftovers. The stream, the bytes
      written and all 0x31ec context bytes match, and the streams decode back.
    - `Compress_Block`: 400 blocks in every delta mode.
    - `Huff_Analyze`: 1,500 cases with other flags and code limits, every context byte compared. The 12 the
      port declines are flag sets the game never passes, where a near symbol reads the function's own locals.
    - `Huff_BuildTree` and `Huff_RunCost`: 3,000 random count tables.
    - This found a runner bug: `mult`/`multu`/`div`/`divu` cleared LO1/HI1. The analysis keeps a cost in LO1
      across a `mult`. The runner now keeps them and has learnt `madd1` / `maddu1`.
- `File_LoadAndUnpack` 0x2c8938 (via `File_Load` 0x2c8a70): opens with mode 1 (so the archives are searched),
  reads the file, and if `Compress_UnpackedSize` is not 0 decodes it into a buffer of that size.
- Every packed file on the disc is RefPack 0x10 (the four character archives, the course archives, `PS2LOAD`,
  `SSXFE`, `TRICK`); nothing uses the other codecs (row E2c).
- The other three (row E2c; `compress::{huff_decode, btree_decode, decode18}`, checked: `tricky-game/tests/
  compress.rs`, streams made by the test unpacked by the game's `Compress_Decode` and ours, byte for byte):
  - Huffman (`Huff_Decode` 0x2c3730): big-endian 16-bit reads, most significant bit first. Size: 4 bytes at +2
    with bit 7 (+6 with bit 0), else the bit stream's first 24 bits (from +2, +5 with bit 0). Then the escape
    byte; per code length from 1 a number − 4 codes, until the tree is full; per code a number − 3 not-yet-used
    byte values to step on from the last (from 0xff, wrapping). A number: `1ab` → 4 + ab, else z zeros, a one and
    n = z + 2 bits → bits + 2^n; with 16 or more zeros the game counts them one by one and gets n = z + 3, so no
    number from 2^18 to 2^19 − 1 can be written. Codes ≤ 8 bits come from a 256-entry table, longer ones by
    comparing the top 16 bits with each length's last code. After the escape code a number − 4: not 0 repeats the
    last byte that often, 0 then a bit: 0 an 8-bit literal, 1 the end. 0x32fb sums the bytes once, 0x34fb twice;
    the type is compared with bit 7 kept, so 0xb2/0xb4 never do.
  - BTree (`BTree_Decode` 0x2c3540, `BTree_EmitSymbol` 0x2c34d8): a 3-byte size at +2 (+5 for 0x47fb only; bit
    7 does not make it 4 bytes), the escape byte, a count and (byte, left, right) for each byte that stands for a
    pair; then bytes: plain, pairs (expanded recursively), or the escape and a literal (0 ends).
  - 0x2c2d18 (`Compress_Decode18`): 16-bit pictures in DXT1-like 4×4 blocks: a 3-byte size at +2, the width at
    +6, 8-byte blocks from +8 (two RGB565 colours, four rows of 2-bit picks), rows of blocks until the size is
    reached. The middle colours are 5:3 and 3:5 mixes when colour 0 > colour 1, else the average and 0 (clear);
    a 0 in colour 0 or 1 becomes 1 first; out as PS2 1-5-5-5 (`0x8000 | b << 10 | g << 5 | r`, 0 stays 0).
    `Compress_IsType18` returns 100 (0x1afb) or 0 but `Compress_Decode18Rows` checks for 3 or 5 (blocks with 8
    more bytes before them), so that form is never used.

Checked: all 793 entries of the character archives unpack to the same bytes as `tools/afl/refpack.py`.

## Character and board models, `.mpf` (row E4a)

Proven (loader, see also `notes/characters.md`): file header (+4 part count, +6 directory, +8 data), the
0x50-byte part entries, `MpfFile_AddPart`'s name match (`strnicmp` 0x2fd398 on 15 characters), and the part
tables `SkinPart_Load` 0x205080 fixes up:
- texture sets at the data's start, 0x20 each (count +0x44): five 4-character names, gloss strength at +0x14;
- bones at +0x18, 0x54 each (count +0x42): name, +0x10 parent slot, +0x12 parent index, +0x18 position, +0x24
  rotation (Euler, inverse); a bone is named `slot << 8 | index`, the slot being a part's +0x4a (body 0,
  eyes 5, goggles 6, hair 7 or 8);
- weight sets at +0x30, 0xc each (count +0x3c): (count, pointer) of u32 `bone << 16 | percent`;
- weight palettes at +0x2c, 8 each (count +0x3e): (count, pointer) of weight-set numbers, one per packet list;
- materials at +0x20, 0x14 each (count +0x40): +0 flags (1; 0x100 face; 0x11 shadow), +4 texture set, +0xc /
  +0x10 its packet lists: (pointer, count) of 0xc-byte records whose first word starts a DMA chain.
- The names of the parts a rider is built from (`BdrLoad_BuildSkin` 0x201138): tables 0x341318 (body, three
  per level of detail), 0x341588 (head, five) and 0x341998 (board), copied into `mpf::names`.

Inferred from the data (the vector unit programs are not decompiled; `tricky-data/src/mpf.rs`):
- A packet chain is `cnt` DMA tags of VIF codes, ending with `end` or `ret`. A batch is four unpacks then a
  program start (`MSCAL`; a `ret` record has none): a V4-32 header (a GS tag: triangle strip, ST/RGBAQ/XYZ2;
  then the strip count and one strip length per quadword), and per vertex, three quadwords apart: V4-16 UV
  (4.12 fixed; z is the vector unit address of the vertex's weight palette entry: `(z - 14) / 4`), V3-16
  normal (/32767) and V3-32 position (cm).
- A list with several records draws all of them (the face: 18 pieces, each followed by more data, likely its
  expressions, not read). Shadow meshes (flags 0x10) are positions only for another program: not decoded
  (row E4d).

Checked: for the 12 riders exported before and the six board shapes, the models built from the disc
(`CharModel::rider_from_disc`, `board_from_disc`) equal the exports: bones, parents, bind positions, and every
vertex's position, UV, normal and weights.

## Textures, `.ssh` (row E3b)

Proven (`lib-shape`, `tricky-data/src/ssh.rs`): `SHPS`, total size, count, version, then (name[4], offset)
pairs (`Shape_GetEntry` 0x2c266c). A shape is a chain of blocks; each block's first word holds the type (low
byte) and the offset to the next block (upper 24 bits, 0 = last). `Shape_GetLongName` 0x2c2610 finds type 0x70
(name at +4); `Shape_FindByName` 0x2c2528 compares the long name if there is one, else the 4-character name
(`strcmp`). `Shape_Unpack` 0x2c2318 unpacks blocks with bit 0x80 (codecs) and converts types 0x60-0x62.

Every `.ssh` on the disc and in its archives (807 files) uses only: 0x02 (8-bit indices), 0x21 (its 256-colour
palette, RGBA bytes in order), 0x05 (RGBA), 0x70 (long name), 0x7c (hotspots). Colours and alpha are stored as
the GS takes them, 0-0x80 (`TexMan_UploadTexture` 0x1c3ba8 uploads PSMT8 / PSMCT32 as they are).

The textures exported before (`chars/*/*.png`) are these images with colour `c * 255 / 128` rounded down and
alpha doubled (capped at 255): checked equal for all 192 rider textures (12 riders: suit and boots of outfit 1,
head, helm, 12 boards). That mapping is a stand-in for the GS's texture function (`Image::rgba_full_range`).

Also found: a `BIGF` directory's size (+0xc) can end inside the last entry's name; the game reads on into the
rest of its first read, so `Files::mount` keeps the whole first read as the header buffer.

## Text, `.loc` (row E5a)

Proven (`tricky-data/src/loc.rs`): `Loc_LoadForRegion` 0x17c150 loads `data/lang/american.loc` (region 0) or
`british.loc` (region 1, not on the NTSC disc) whole. +8 flags (bit 0: +4 is an id map, `Loc_MapId` 0x2c50b0,
a `bsearch` over (u16 key, u16 value) pairs); +0xe u16 picks the string block (offset at +0x10 + 4 * it); a
block has +0xc the count and from +0x10 the strings' offsets; strings are 16-bit characters ending in 0.

`american.loc` has 3,774 strings. The disc also has two files the exe never names:
- `CONSTANT.LOC`: the same table with the developers' name of every string id (`kHUDKnockdown` 0xe93,
  `kOvCombo` 0xe41, `kOVInvalidCard` 0x16e, ...). Use it to read which text a `Loc_GetString` call asks for.
- `LETTER.LOC`: the English text with a few strings changed (a variant, unused).

## Trick book, `TRICKDEF.DAT` (row E5d)

Proven (`tricky-data/src/trickdef.rs`): `TrickDef_Load` 0x154580 reads `data/tutorial/trickdef.dat` whole
(0x2760 bytes: 12 riders × 30 tricks × 28 bytes). `Score_FormatTrickName` 0x1551c0 prints fourteen `%s` from
pointer tables at 0x320970-0x320f58 (read from the disassembly: rail-to `w1>>12&15` at 0x320b08, rail spin
`w1>>4&15` 0x320b20, stance `w1>>16&15` 0x320970, spin side `w1>>27&7` 0x320988, spin `w0>>24&15` 0x320998,
flip count `w1>>24&7` 0x3209c8, flip kind `w1>>8&15` 0x3209d8, combo `w0&255` 0x3209f0, spin after `w0>>28`
0x320aa8, grab `w0>>8&255` 0x320b58, late `w1>>30` 0x320b50, second grab `w0>>16&255`, ending `w1>>20&15`
0x320ad8, crash `w1&15` 0x320ae8). The tables are adjacent and an index past one reads the next, so
`trick_names.rs` keeps the whole region. All 360 names equal `chars/trickbook.json`.

## Fonts, `.SFN` (row E5e)

Proven (`tricky-data/src/sfn.rs`): `cGame::Init` 0x17e148 loads `data/fonts/title.sfn` (gApp+0x5c, scale
1.4 x 1.3) and `data/fonts/menu.sfn` (gApp+0x60, 1.8 x 1.4). `Font_ParseGlyphs` 0x19c910: +8 version (records
of 12 bytes from 200, 11 before), +0xa count, +0x10/+0x11 origin, +0x14 records, +0x1c bitmap.
`Font_BuildTexture` 0x19cca8: 4-bit pixels, low nibble first, into a square texture (side: max of the width and
the smallest power of two above the height), colours RGB 0x80 with alpha `i * 128 / 15`. The glyphs and line
heights equal `chars/fonts/*.json` for both fonts.

## Camera files, `.CML` (row E5c)

Proven: `Camera_LoadCmlFiles` 0x1783e8 loads, in order, the course's file (`Camera_GetCourseCmlName` 0x178330:
course id 0 garibald, 1 snowdrea, 2 elysium, 3 mesablan, 4 merqury, 5 aloha, 6 and 9 pipedrea, 7 untracke, 8
tokyo, 10 trick, 11 alaska), `commonob` and `scripts`, each `%s.cml` (0x37fba8) in DATA/CAMERA, skipping a
missing one (`Cml_LoadIfExists` 0x16e4c0). `intro::Cml::from_disc` reads them; equal to the level folders' copies.
Our level folder to course id mapping (`intro::course_of_folder`) is a stand-in for the course table (row C0).

## `.aip` / `.sop`: AI paths and race lines (board row E3d, checked: `tricky-game/tests/aip.rs`, `tricky-data/tests/aip_json.rs`)

- In each course's DATA/MODELS/<course>.BIG (gari, snow, elysium, mesa, merquer, aloha, pipe, untrack, megaple,
  trick, alaska): `<course>.aip` (the race) and `<course>.sop` (the same layout; show-off), compressed.
- `AIP_LoadPathsFromFile` 0x197da0 (`tricky_data::aip::read`): a word it skips (0x0a0a0a0a), the section count,
  sections of (type, size, data). Type 0, AI paths (`AIP_LoadAIPaths` 0x198518, `cAIPath`, 0x40 bytes each): count,
  start-position count, the start positions (path indices; always six). Type 1, race lines (`AIP_LoadEventPaths`
  0x1983d0, `cEventPath`, 0x3c bytes): count, the main line's index. Each path (`AIPath_Load` 0x196f28):
  parameters (id, size, bytes; `cAIPath` 100 = U3 (default 50), 101 = respawnable (default 1); `cEventPath` 0 = the
  distance to the finish), point and event counts, the start, the box (min, max), points of (unit direction,
  length), events of (type, value, start, end), the type kept as its place in the table at 0x337e70 (−1, 0..23,
  100..105, 300).
- The game keeps the points and events where they lie in the loaded file (only the event types rewritten in place).
- Names (F1c, proven): `AIPath_LoadPoint` 0x197130 (the direction, `Vec3_Assign` 0x198720, then the length,
  `Float_Assign` 0x198750: copies of 12 and 4 bytes unless source and target are the same) and `AIPath_LoadEvent`
  0x197170 (the type looked up in the 0x337e70 table, 32 entries, 0 if absent; then value, start, end).
- The app reads the course's `.aip` from the disc with TRICKY_DISC (`aip::from_disc`, `AiFile::from_aip`), else
  the level folder's AIP.json (SSX-Library's export: the same paths, its points direction × length).

## Config files (row E5b; checked: `tricky-game/tests/ini.rs`)

`DATA/CONFIG/*.INF` are read with one reader, `IniReader_*` 0x221668..0x222040 (`tricky_data::ini::IniReader`): a
cursor over the text (a NUL after it) and one token buffer. `#` and `;` comment to the line's end; `[NAME]` heads a
section (`IniReader_NextSection` scans for the next `[`); `KEY = "string"` / `= n` (`sscanf` "%d") / `= x` ("%f"),
the key compared with the last token read (`stricmp`). `IniReader_FindSection` matches a name in any case,
`IniReader_FindSectionMatching` by prefix either way (case matters); `IniReader_SkipToNextSection` also skips the
headers of sections that follow at once, so an empty section's keys are its successor's. The loaders (the pointer
table at 0x343070): `Audio_LoadFEConfig` 0x20f160, `Audio_LoadInGameConfig` 0x211098 (audio, banks, crowd, chant),
`Sfx_LoadSurfaceConfig` 0x210c38 (snow), `Music_StartCourseSong` 0x214e60 (intromus), `MusicSys_Init` 0x224a98
(music), `MusicSys_LoadMusicMap` 0x224cf8 (musicmap), speech 0x3436fc, nascript.

Music (`tricky_data::music`): `MusicSys_Init` reads `[GLOBAL]` BASEPATH, then every other section in order (32 at
most) is a song: its name, the path BASEPATH + LOOPDATA. `MusicSys_LoadMusicMap`(course name upper-cased, from
`Course_GetInfo`) finds the map's section by prefix and enables, for each `SONG = "…"`, the first song whose name
contains it (case matters; four at most); none: song 0. The app plays these (TRICKY_DISC), in the game's song order.

Audio settings (row E5b2, `tricky_data::audiocfg`, checked: `tricky-game/tests/ini.rs`): `Audio_LoadVolumeLevels`
0x21ca20 reads SFXLEVEL, MUSICLEVEL, CHARSPCHLEVEL, PASPCHLEVEL, JUKEBOXLEVEL, UBERLEVEL, INTRODUCK from the scaling
section (IGLEVELSCALING in a race, FELEVELSCALING in the menus) and then from the course's section, searched on from
there (no rewind); a level is the value / 100 when 0..200 (the duck 0..100), else 1.0 (the duck 0.75), into
0x343054..0x34306c. `Music_StartCourseSong` 0x214e60: the course's name lower-cased, the first INTROMUS section
(lower-cased) it contains that is not empty, its first word SONG = "file" → `data/audio/<file>` as the intro stream
(TRICK has none). Then segment 0, one of 1..3 at random (`(rand & 0x7fff)·3/0x7fff + 1`: 4 for 0x7fff), and two of
4..7 (`·4/0x7fff + 4`: 8 for 0x7fff); not in modes 3, 7, 8, 11.


## Course logic and collision, `.ssf` (row E3c1; `tricky_data::ssf`, checked: `tricky-game/tests/ssf.rs`)

- `Course_LoadRenderData` 0x25edf0 builds the course's file names as `|data/models/<file><ext>` (in the course's
  archive): `.ltg`, `.pbd`, `.ssh`, `.ssf`, `.adl`, `.aip` (`.sop` in show-off), and `<file>_L.ssh` (the first six
  letters of a long name, then `_L`) for the lightmaps. `Course_LoadSphereTrees` 0x25f598 reads the `.ssf`; without
  one (or with the wrong version, or an object count that is not the course's) every object gets the default
  physics record 0x3d2d70 (1000.0, 0.5, flags 3 << 16, −1).
- Header (19 words): +0 0x1e0200, +4 version u16 0x1500, +8 a build stamp; then count/offset pairs: +0xc/+0x10
  event slots (7 words: the effect header each of seven events runs, −1 none; `Instance_GetEventScript` 0x25fe78),
  +0x14/+0x18 sphere-tree groups and +0x1c/+0x20 collision groups (offset, bytes, count), +0x24/+0x28 effect headers
  (op count, offset from the end of that table), +0x2c/+0x30 script functions (op count, offset as the headers', a
  16-byte name; `World_FindScript` 0x260dd8 compares case-blind), +0x34/+0x38 physics records (0x18 bytes: +8
  flags, copied to instance+0xe8; +0x14 the event-slot record as a signed half), +0x3c/+0x40 one physics record
  index per object (instance+0xec points at it; flag bit 0x2000 of the upper half sets model+0x10 bit 0x10),
  +0x44/+0x48 spline segments (word 0 to segment+0x18 with its upper half also moved down, word 1 to +0x24).
- A script op: word 0 the op, word 1 its size in bytes (`TriggerScript_Run` 0x13be78 steps by it), the arguments
  after. A collision mesh: triangle count n, vertex count m, padding bytes, n triangles of three u32 indices, the
  padding, m vertices (4 floats), n planes (normal and w); the loader keeps (n, m, triangles, vertices, planes) in
  0x14-byte records (course+0x24) and (first record, count) per group (course+0x20).
- An instance's collision meshes (row E3a5; checked against the levels folders' Collision/*.obj, 3,789 meshes,
  88,828 triangles, all 11 folders, exact): its physics record's collision mode not 3 and group ≥ 0 → that group's
  meshes, each its own OBJ in the exports (numbered by mesh across all groups); each triangle's normal is its plane's.
- On the disc: gari 3393 objects, 306 effect headers, 20 functions, 412 collision groups; ssxfe (the front end)
  has 33 functions and no collision. `untrack` and `trick` have no functions.
- Sphere trees (row E3c2; `Ssf::sphere_trees`): each group's trees follow one another (`SphTree_Load` 0x239340
  returns the bytes it read; the group's byte count is their sum). Header 0x70 bytes: +0 / +4 the sizes of the two
  data blocks after the levels, +8 a flag (0: the occupancy bytes are the data itself, else 0 — the tree's cache
  fills them), +0xc the depth, +0x10 the centre (also the tree's +0x90, w 1), +0x1c the centre of mass, +0x28 and
  +0x4c two 3×3 matrices (inertia and inverse); then depth + 1 level records (12 bytes: sphere radius, cell
  half-size, child index stride 8ⁿ) and the data. The trees are octrees: a node's eight children sit at the
  corners (the tree's +0x10, 8 vectors from 0x3439d0) × the next level's half-size, an occupancy byte saying which
  exist (`SphTree_SphereRecurse` 0x23a728). The loaded tree is 0x150 bytes (header copy at +0xd0, its +0 the tree's
  running index).

## World grid, audio links, skies (row E3e; `tricky_data::ltg`, `tricky_data::adl`, `courses::skies`; checked: `tricky-game/tests/ltg.rs`, `tests/courses.rs`)

- `.ltg` (`cCourseResolve_Common::ResolveTGroupHeader` 0x252470 and on; their debug prints name the fields):
  version (3 bytes) and endianness, the world's box (min, max, centre), the TGroup size (10000 cm), the grid's
  width × height in TGroups, used / empty, the INode size (2500), INodes across (4) and in a TGroup (16), the
  TGroup table's offset (a word per square, row by row, 0 empty) and where the TGroups start. A TGroup: its box,
  counts (patches, instances, splines, lights inside, lights crossing, particle instances, two more, elements), the
  offsets of its index lists into the `.pbd` (resolved to pointers by `Course_GetPatch` / `GetInstance` /
  `GetSpline` / `GetLight` / `GetParticleInstance`), and 16 INodes (0x4c bytes at +0x54: box, seven counts, six
  list offsets into the TGroup's lists). `WorldGrid_CellAt` 0x260ba8 maps INode coordinates to the INode.
- `.adl`: a zero byte, 1.0, a count, then (instance name hash, zone offset) pairs (`AudioTGroups_Bind` 0x22a5f0:
  the hash becomes the instance, whose +0xf4 points back). A zone: a word, a count, shapes (type 0: 7 words, 1 and
  2: 12, 3: 6; 0/2/3 spheres at words 2..5, 1 a box with half-sizes 5..7 along direction 8..10).
  `AudioTGroup_GetBounds` 0x22aaa0 boxes them (moved by the instance's position) for the audio grid.
- Skies: `Course_LoadSkies` 0x185b18 loads `data/models/<course>_sky.pbd` (+ `.ssh`), one per course.
- `.map` files are not read by the game (no such name in the ELF); they are the tools' instance names.

## Level data, `.pbd` (row E3a; `tricky_data::pbd`, checked: `tricky-game/tests/pbd.rs`)

- `cCourseResolve_PS2::ResolveDataHeader` 0x255630 and the resolvers it calls (their debug prints name every
  field). Header: version, format (1 = PS2), 15 counts (player starts, patches, instances, particle instances,
  materials, material blocks, lights, splines, spline segments, texture flips, models, particle models, textures,
  cameras, light map size) and 18 offsets (the same tables, the model / particle model / camera pointer tables, the
  hash table, the VIF block). Records: patch 0x1c0, instance 0x100, particle instance 0x70, material 0x48, light
  0x5c, spline 0x28, spline segment 0x80; material blocks and texture flips are runs of (count, words).
- The resolver turns offsets into pointers and indices into records: instance +0xc0 its model (by id through the
  model pointer table), +0xc4 / +0xc8 other instances, +0xf0 its material block; particle instance +0x40 its model;
  material +0x44 its texture flip; spline +0x20 a segment; segment +0x50 / +0x54 segments, +0x58 a spline; models'
  hierarchies (6-word nodes: parent, three LOD meshes, animation, matrix — −1 the identity at 0x346440), meshes
  (contexts table, each context's VIF data from the VIF block), animations (channels); particle models; cameras;
  the hash table's six lists (patches, instances, models, lights, splines, cameras).
- Naming: the game's "splines" (+0x20 / +0x5c, 0x28 bytes) are what `Course_GetSplineSegment` returns and the
  `.ssf` calls segments; its "spline segments" (0x80) are `Course_GetSpline`'s.
- A quirk: the texture flips' offset (+0x64) is resolved twice, so the header word ends up file + file + offset
  (the flip table is built in between, from the right address).
- Skies (`<course>_sky.pbd`, in the course's archive) are the same format: one model; most courses share one file.
- Fields (row E3a2; `pbd::{PatchRec, InstanceRec, MaterialRec, SplineRec, SegmentRec, LightRec}`, `ssf::Physics`,
  `map::Map`; checked against the levels folders' JSON exports: `tricky-data/tests/pbd_json.rs`):
  - patch: lightmap rectangle, four corner UVs (u, v, 1, 1), the surface as 16 polynomial coefficients (highest
    power first; the exports' Bézier points are converted from them), box, corners, +0x1a8 surface type, +0x1ac bit
    31 show-off only, +0x1b0 texture (low half) and lightmap page (high half).
  - instance: matrix, light directions (columns), three light colours, ambient colour, model id, previous / next,
    box (zero on the disc: filled at run time, row E7a3), material block. Its `.ssf` physics record: +0 mass (1e30 never moves), +4 bounce, the flag defaults (0x1
    shown, 0x20 collides, 0x80 bounces, 0x1000 ?, 0x2000 UV scroll), +0xc surface type, +0x10 collision mode,
    +0x12 the sphere-tree group (mode 3) or collision group, +0x14 effect slot.
  - material: texture (low half), +0x44 texture flip (a run of texture numbers). Spline: box, segment count, first
    segment. Segment: a cubic as four coefficients (highest first), previous / next, spline, box, length. Light:
    type, sprite size, colour, direction, position, box.
  - The `.map` (on the disc beside the others, not read by the game) names everything (and gives the hashes).
  - gari's export in the levels folder is of another build: its splines all differ and its patches' flag word is 0
    there (its show-off patches are surface 18); everything else agrees.
- Meshes (row E3a3; `pbd::{context_strips, strip_triangles}`, checked against the levels folders' Meshes/*.obj:
  8,325 meshes, 313,373 triangles, positions, UVs and normals). A model's node +4 is its mesh; each mesh context's
  word 3 is its offset into the VIF block (header +0x84). Read from the packets themselves (the VU1 program is not
  decompiled): a DMA chain of `cnt` packets (and `ref` ones, which hold no vertices) to `ret` / `end`; in it,
  batches of five unpacks: V4-32 (a GS tag; word 4 the strip count), V2-8 (each strip's length × 3, three
  quadwords a vertex), then per vertex (masked, `STCYCL` 1/3) V2-16 UVs (4.12; the exports write 1 − v), V3-16
  normals (1.15) and V3-16 positions (1.15 times the model's vertex scale, model head +0x18). Each strip is drawn
  as a triangle strip, every other triangle reversed. gari is not compared (its export is of another build).
- The app from the disc (row E3a4; `Level::from_disc`, checked: `tricky-data/tests/level_disc.rs` against all 11
  levels folders: 28,835 patches, 25,128 instances, 8,325 world and 275 sky meshes, 12,388 collision meshes, 1,540
  splines). Proven: `Course_LoadRenderData` 0x25edf0 mounts `data/models/<file>.big` and reads `<file>.pbd`, `.ssf`,
  `.aip`; `Course_LoadSkies` 0x185b18 reads `<file>_sky.pbd`. The `.map` (names only) is read by no code in the game;
  the port takes the names from it to keep the folders' names. A model node's matrix (+0x14, -1 none) is split into
  position, rotation and scale; its animation keeps the head's six floats, the action word and each channel's cubic
  segments (count at the channel, then the offset to the first). The scripts still come from the levels folder (row
  E9b2); with `TRICKY_SIM=600` elysium and aloha ride the same from the disc as from
  the folder (distance, speed, airtime, falls within a few percent; the folder's numbers are rounded to 6 digits).
- The course's textures from the disc (row E3a7; checked: `tricky-data/tests/level_disc.rs`, 1,710 textures,
  lightmaps and sky textures of all 11 folders, pixel for pixel). Proven: `Course_RemapTextures` 0x2606f8 and
  `Course_RemapLightmapTextures` 0x260898 load an `.ssh` each (`World_LoadTextures`) and replace every material's
  four texture numbers, its flip-book's, and the patches' (+0x1b0, four shorts, kind 1 = texture, 5 = lightmap in
  +0x1ac's 3-bit fields) by the table entry of that number: the numbers are the shapes' order in the file, which is
  what the folders' `<n>.png` are. The files: `<file>.ssh`, the lightmaps' `<file cut to 6 letters>_L.ssh`
  (`Course_LoadRenderData` 0x25edf0: `elysiu_L`), the sky's `<file>_sky.ssh`. The exports kept the lightmaps' stored
  alpha (0-0x80); the app reads only their colour. With the disc the app needs no levels folder: the course list is
  the 11 courses, and a course's folder, if there is one, gives only the scripts.

## The small films (row G11a1; `data/src/video.rs`, checked: `game/tests/video.rs`)

The disc's videos come in two kinds:
- **`.MPC` (the intro, the extras, the characters' films):** they interleave MPEG-2 video (`MPCh` chunks, one picture
  each, 512 × 448) with EA's streamed audio (`SCHl`, `SCCl`, `SCDl`, `SCEl`). `cVideoModule` plays them on the PS2's
  MPEG decoder; that is row G11a2.
- **`.SSS` (the courses' previews, and the trick book's films in `DATA/VIDEO/TB/TB_*.BIG`):** small films that
  `cVideoStreamMan` plays.

An `.SSS` film is a run of chunks, each an 8-byte head (the tag `SSSF`, then the chunk's size with the head) and one
frame. The frame is a shape of type 0x60 (DXT1), 128 × 64 or 128 × 128. The playback goes:
- `VideoStream_Open` 0x1ff990 sets the rate: 60 / the frames a second, kept to 1..30;
- `VideoStreamMan_UpdateFrame` 0x1ffa88 takes the stream's next chunk at that rate;
- `VideoStreamMan_DecodeFrame` 0x1ff928 sets the frame's type byte to 0x60, unpacks it, copies the pixels past the
  16-byte header into the film's texture (+0x94, its size at +0x98) and frees the unpacked shape.

`Shape_Unpack` 0x2c2318 handles the converted types:
- 0x60 is DXT1. 0x61 and 0x62 are the same colour blocks 16 bytes apart (DXT3 and DXT5 with their alpha skipped);
- the output keeps the 16-byte header with its type byte set to `Shape_PixelFormatIndex(0xf)`, which is 3;
- the pixels are 16-bit PS2 ones from `Compress_Decode18Rows` 0x2c2bf8, already ported in `compress.rs`;
- its buffer is the width × 2 (0x60) or × 4 (0x61, 0x62) × the height. For 0x61 and 0x62 only the first half is
  written, and the rest is whatever the allocator gave (the port zeros it, STANDIN).

Checked in the runner on 3,000 random shapes of the converted types and others, through `Shape_Unpack` and
`DecodeFrame`. With `TRICKY_DISC`, every film on the disc decodes: 406 films, 28,894 frames. 8 of 9 mutants are caught
(two by the data crate's unit tests of the chunk walk). The survivor turns 0x62's mode 5 into 3, which the block
decoder treats alike.


## Names from F1c batch 17 (proven)

- `MemCard_ClearCurrentSlotStatus` 0x23ea98: clears +0x174 / +0x178 of the current slot (+0x10, 0x14 apart).
- `MemCard_CheckRoomForFiles` 0x23fcb8: free space (vf 0x8c) and each file's size (vf 0x9c); a file not yet on the
  card gets +0x1a4 + i·0x70 = 1 if it does not fit, else its size comes off the free space.
- `App_GetCourseId` 0x17c360, `App_FindCourseById` 0x17c380, `App_GetCourseInfo` 0x17c3a8: wrappers (no callers)
  on the course table at gApp+100; `CourseTable_GetId` 0x186440: record i's +0x80 id (i capped at 13).
- `App_FreeEffectsLoader` 0x17b938 (gApp+0x30, made by `App_CreateEffectsLoader`), `EffectsLoader_Delete` 0x17c468.
- Not settled: 0x23ed48 (sets a +0x19c flag), 0x240c78 (patches a `CourseDma` chain).

## Names from F1c batch 18 (proven)

- Sony's `libmc` (all RPC calls to the card server 0x405580, command kept in 0x35d3a0, semaphore 0x35d3a4):
  `sceMcGetInfo` 0x2f1c00 (1), `sceMcOpen` 0x2f1428 (2), `sceMcClose` 0x2f1598 (3), `sceMcSeek` 0x2f1650 (4),
  `sceMcRead` 0x2f17b8 (5), `sceMcWrite` 0x2f18d8 (6), `sceMcMkdir` 0x2f1560 (`sceMcOpen` with mode 0x40, then
  command 0xb), `sceMcChdir` 0x2f1f68 (0xc; its end callback `Mc_ChdirDone` 0x2f1ee0 copies the current dir out),
  `sceMcChangeThreadPriority` 0x2f12b0 (0x14), `sceMcSync` 0x2f1ac8 (mode 0 waits, polling every 0x3c ticks with
  `Mc_Delay` 0x2f1a80 = `SetAlarm` + `SleepThread`, woken by `Mc_AlarmWakeup` 0x2f1a58). Not settled: 0x2f1368
  (command 0x15, waits for its result), 0x2f1280 (hands out the RPC buffers). The read and info callbacks 0x2f1728 /
  0x2f1ba8 are not functions in Ghidra.
- `MemCard_Process`'s steps use them: 0x16 / 0x25 open to write (mode 0x203), 0x17 write, 0x18 / 0x1e close,
  0x1c open to read (mode 1), 0x1d read, 0x24 mkdir.
- `cPS2MemCard` virtuals: `BeginSave` 0x23dea8 (vf29: dir name into +0xb1, "%s/%s" path into +0xf1, buffer +0x2c,
  size +0x20, step 0x15), `BeginSaveSlot` 0x23df20 (vf31: the same with the slot's file name), `SaveMore` 0x23df80
  (vf33: buffer, size, step 0x17), `EndSave` 0x23dfb8 (vf35: step 0x18, the close); `BeginLoadSlot` 0x23e038 (vf22,
  through `MemCard_BeginLoad` 0x23dfc0: step 0x1b), `LoadMore` 0x23e090 (vf23: step 0x1d), `EndLoad` 0x23e0c8
  (vf25: step 0x1e). `MemCard_GetSlotFileName` 0x23ecf0: the slot's name (+0x1a8 + slot·0x70).

## Names from F1c batch 20 (proven)

- `BIG_FindEntry` 0x2cc0d8 (`BIG_Lookup` with the table at 0x3bb960; the entry's address or 0),
  `BIG_GetFileCount` 0x2cc1b0 (`BIG_Identify` 1: the big-endian count after the magic), `Shape_Find` 0x2c25f0
  (a thunk to `Shape_FindByName`).
- Pad library: `scePadGetButtonMask` 0x2e7e08 (the four mask bytes +0x79..+0x7c when the pad's +0x72 is 1),
  `scePadSetButtonInfo` 0x2e7ec0 (RPC command 10; on 1 `scePadSetReqState` 2), `scePadInfoPressMode` 0x2e7f70 (the
  mask is 0x3ffff), `scePadEnterPressMode` 0x2e7fd0 (`scePadSetButtonInfo` 0xfff). `game/src/inputring.rs` calls
  these two `enter_press_mode` and `press_mode_next`; by the SDK they are the press-mode query and its entry. The
  Rust code was not changed.
- Not settled: 0x2c8068 (a file open through `FileSys_RunWithHeap` 0x2c7f60; `speechload.rs` and `crowdbox.rs` call
  it), 0x2cc2c0 (from `FileSys_InitWithBuffer`: a poll task 0x2cc680 and the mode at 0x3bba60).

## Names from F1c batch 21 (proven)

Sony SDK libraries linked into the game, named after the SDK functions they match. Each was matched by the
RPC command number it sends (or the hardware register it touches) and by its callers in the game.

- **SIF and file I/O (libkernel / sifrpc / fileio):** `sceSifInitCmd` 0x3089b8, `sceSifExitCmd` 0x308c38,
  `SifCmd_IntrHandler` 0x308eb0, `sceSifInitRpc` 0x309088 (boot.rs `SDK_INIT`), `sceSifExitRpc` 0x309228 (called by
  `sceSifRebootIop`), the RPC packet helpers `SifRpc_GetPacket` 0x309318 / `GetFPacket` 0x309348 / `OnEnd` 0x309388
  (packet ids 0x80000009..0x8000000c) / `OnRData` 0x309440 / `FindServer` 0x3095f8 / `OnBind` 0x309648.
  File server (bind 0x80000001, buffer 0x40a640): `sceFsInit` 0x30a588, `sceFsReset` 0x30a7b8, and by RPC number
  `sceLseek` 4, `sceRead` 2, `sceIoctl` 5, `sceIoctl2` 0x1a, `sceMkdir` 7, `sceFormat` 0xe, `sceAddDrv` 0xf,
  `sceDclose` 10, `sceDread` 0xb, `sceGetstat` 0xc, `sceChstat` 0xd, `sceRename` 0x11, `sceSync` 0x13, `sceMount`
  0x14, `sceLseek64` 0x16, `sceDevctl` 0x17, `sceSymlink` 0x18, `sceReadlink` 0x19. `Fio_PathCmd` 0x30b888 sends
  a path with the command in a1; one-line wrappers pass `sceRemove` 6, `sceRmdir` 8, `sceDelDrv` 0x10, `sceChdir`
  0x12, `sceUmount` 0x15; `sceDopen` 0x30bfd0 passes 9 and turns the reply into an fd. The fd table and lock:
  `Fio_InitFdSema`, `Fio_AllocFd`, `Fio_GetFd`, `Fio_InitSema`, `Fio_Lock`, `Fio_Unlock` (0x309fe8..0x30a578).
- **Module loader (loadfile, server 0x40aac0):** `SifLoad_Init` 0x30d7b8 (binds, RPC 0xff), `SifLoad_ModuleCmd`
  0x30dbc8 (`sceSifLoadModule` and 0x30de10 call it), `SifLoad_ModuleBufferCmd` 0x30d980 (RPC 6, under
  `sceSifLoadModuleBuffer` 0x30db88 and `sceSifLoadStartModuleBuffer` 0x30dba8), `SifLoad_ElfCmd` 0x30de30 (under
  `sceSifLoadElf` 0x30df38, a3 = 1), `sceSifLoadIopHeap` 0x30d6c8 (IOP heap server 0x40a6c0, RPC 3).
- **CD (libcdvd):** `sceCdInitEeCB` 0x2e8538 (creates the callback thread `Cd_CallbackThread` 0x2e8478; CdFs_Mount
  calls it with priority 0xb, stack 0x34a5c0, 0x800), `Cd_CallbackHandler` 0x2e83d8, `Cd_InitSemas` 0x2e86b0,
  `Cd_Exit` 0x2e8748 (from `sceCdInit` mode exit: kills the thread, removes SIF handler 0x80000012), `Cd_Delay`
  0x2e8318 / `Cd_AlarmWakeup` 0x2e82f0 (alarm + semaphore sleep, used by `sceCdSync`/`sceCdSyncS`),
  `Cd_CheckNCmd` 0x2e8d00 (N-command server 0x80000595) and `Cd_CheckSCmd` 0x2e9018 (S-command server 0x80000593),
  `sceCdMmode` 0x2e9660 (S-cmd 0x22; boot.rs `CD_MODE`), `sceCdGetToc` 0x2e9908 (N-cmd 4, 0x810-byte buffer),
  `sceCdStop` 0x2e9b98 (N-cmd 7, async with the callback).
- **Pad (libpad, server 0x403c40):** `Pad_SendActData` 0x2e6f48 (DMAs the actuator block, from
  `scePadSetActDirect`), `Pad_SendInit` 0x2e71b8 (cmd 0x10, from `scePadInit`), `scePadEnd` 0x2e7258 (0xf),
  `Pad_GetDmaStr` 0x2e7578 (returns the newer of the two DMA frames; used by every pad reader),
  `scePadSetVrefParam` 0x2e8080 (0xb), `scePadGetPortMax` 0x2e8150 (0xc), `scePadGetSlotMax` 0x2e81b8 (0xd),
  `scePadGetModVersion` 0x2e8220 (0x12, from `scePadInit`).
- **Memory card (libmc, from batch 18's list):** `sceMcFormat` 0x2f20b8 (0x10), `sceMcDelete` 0x2f2180 (0xf),
  `sceMcFlush` 0x2f22a8 (0xa), `sceMcSetFileInfo` 0x2f2360 (0xe), `sceMcRename` 0x2f2538 (0xe with flag 0x10, cmd
  word 0x13), `sceMcUnformat` 0x2f26a0 (0x11), `sceMcGetEntSpace` 0x2f2768 (0x12).
- **MPEG (libmpeg, used by the movie player vf1 0x2ba560):** `sceMpegInit` 0x2e9e70 (resets the IPU DMA
  channels), `sceMpegCreate` 0x2e9f10, `sceMpegDelete` 0x2ea150 (returns 1), `sceMpegGetPicture` 0x2ea190,
  `sceMpegGetPictureRAW8` 0x2ea1d8, `sceMpegGetPictureRAW8xy` 0x2ea220, `sceMpegAddCallback` 0x2ea370.
- **VU and VIF0 debug (libdev):** `sceDevVu0Reset/Pause/Continue/Exec/GetTpc` 0x2e62c8..0x2e6460,
  `sceDevVu1Reset/Pause/Continue/Exec/GetTpc/GetCnd/PutCnd` 0x2e6778..0x2e6ad0, `sceDevVif0Reset/Pause/Continue/
  GetCnd` 0x2e6c18..0x2e6d68, plus `DevVu0_IsBusy` 0x2e62b0, `DevVu_AnyBusy` 0x2e66c0, `DevVu0_GetState`,
  `DevVu1_GetState`, `DevVif0_GetState`, `DevVu0_SaveIntRegs` 0x2e6470, `DevVu0_SaveFloatRegs` 0x2e6550,
  `DevVu_PutQword` 0x2e66f0 / `DevVu_GetQword` 0x2e6730 (VU1 registers through VU0 memory 0x400+). Reset and pause
  set the FBRST bits (VU0 reset 2, break 1; VU1 0x200 / 0x100); only the resets are called by the game (0x1dbad8).

Unclear (batch 21): 0x2f1368 (libmc cmd 0x15), 0x2e8288 (libpad cmd 0x14), 0x2e8e70 (CD N-cmd 0xe, result
compared with 6 in CdFs_Mount, maybe the drive status), 0x2e87c8 / 0x2e8838 (power-off callback set / call),
0x30d8b8, 0x30d948, 0x30e470 (from `_InitSys`), 0x307830..0x308c38 (deci2 / SIO debug console and printf helpers).

## Names from F1c batch 22 (proven)

EA's system library (memory, debug output, threads, timer) and the file library, read from the code.

- **Memory (EAMem):** `EAMem_Zero` 0x2cc938 (`EAMem_Set` to 0), `EAMem_HeapHeaderSize` 0x2cd990 (heap table
  0x34eff8[flags & 0xf] +0x30), `EAMem_BlockHeaderSize` 0x2cc9a0 (that plus the name's length + 1 when the heap keeps
  names, flag 0x100), `EAMem_GetName` 0x2cd9b8, `EAMem_AllocAligned` 0x2ccf50 (passes all of `EAMem_AllocEx`'s
  arguments: name, size, alignment, alignment offset, flags), `EAMem_Alloc0` 0x2ccf98 (as `EAMem_Alloc` but passes 0
  in the sixth argument, which `EAMem_AllocEx` never reads), `EAMem_Resize` 0x2cda18 (resizes a block in place, -1 =
  as large as the free space after it allows; `BankSlot_Load` shrinks banks with it), `EAMem_ClearHeapTable`
  0x2cd250, `EAMem_InitDefaultHeapMax` 0x2cd890 (`EAMem_InitDefaultHeap(0x6000000)`, no callers).
  Heap checks under `EAMem_ValidateHeap` (block magics 'FB' 0x4246 used, 'SB' 0x4253 heap sentinel, error code in
  gp-0x7f44): `EAMem_IsBlockHeader` 0x2cdc60, `EAMem_CheckNeighbours` 0x2cdcc8, `EAMem_CheckBackLinks` 0x2cddc0,
  `EAMem_CheckInHeap` 0x2cde70, `Mem_IsReadable` 0x2ce2c0 (stub: pointer != 0).
- **Debug output:** 8 outputs at 0x350700 (12 bytes: name, handler, enabled bit) and 64 categories at 0x34f2f8
  (8 bytes: name, enabled bit, group bit). `Debug_Message` 0x2ce5e8 (category, format, ...: if the category is on,
  vsprintf and call every enabled output; `Debug_Fatal` sends to category 2), `Debug_InitChannels` 0x2ce4c0
  (outputs 4..7 and categories 16..63 cleared, category 2 and output 2 on), `DebugOut_SetEnabled` 0x2ce6e0 / `Raw`
  0x2ce3c0, `DebugOut_IsEnabled`, `DebugOut_SetHandler`, `DebugOut_Call`, `DebugCat_SetEnabled` 0x2ce800 / `Raw`
  0x2ce3f0 (category 0 = all, 1 = the group), `DebugCat_IsEnabled`, `DebugCat_SetName`.
- **Threads and timer:** `Sys_InitPollTasks` 0x2cebb8 (clears the 16-entry table at 0x403a00), `Thread_Exit`
  0x2cf358 (hands its own stack to the poll task `Thread_FreeExitedStack` 0x2cee88, then ExitDeleteThread),
  `Thread_WakeSleepers` 0x2cf068 (timer callback added by `Thread_SleepTicks`: wakes the 256 sleep slots at
  0x34f5f8 whose tick has come, removes itself when none are left), `Timer_AddCallback` 0x2cf598 /
  `Timer_RemoveCallback` 0x2cf608 (8 slots at 0x350688, run by `Timer_IrqHandler`, which also counts the tick in
  gp-0x7e60), `Fpu_SaveRegs` 0x2cf7e0 / `Fpu_RestoreRegs` 0x2cf8e8 (the handler saves the 32 FPU registers around
  them).
- **Async file loads:** callbacks of `AsyncFile_Load`/`LoadInto`/`StartRead`: `AsyncFile_OnOpened` 0x2c5a98,
  `AsyncFile_OnSized` 0x2c59c0 (allocates through the hook gp-0x7f98), `AsyncFile_OnChunkThenClose` 0x2c58d0 and
  `AsyncFile_OnChunk` 0x2c5b98 (0x8000-byte reads; the first closes the file at the end), `AsyncFile_OnClosed` 0x2c5898.
- **Streams:** `Strm_ClassifyPackets` 0x2c6668, `Strm_OnDataRead` 0x2c6928, `Strm_SetPriorities` 0x2c7220
  (+0x38 normal, +0x3c urgent read priority), `Strm_SetLowWater` 0x2c7270 (+0x40), `Strm_SetUrgent` 0x2c72e0
  (+0x44; raises the read's priority with `FileSys_SetPriority`), `Strm_GetFillBytes` 0x2c7cf0 (+0x48;
  `Strm_ConsumeBytes` turns urgent when it falls under the low-water mark).
- **Sync file calls:** `File_GetSize0` 0x2c7f18, `File_LoadAlloc0` 0x2c8068, `File_LoadInto0` 0x2c8348,
  `File_OpenBig0` 0x2c86b8: the same as `File_GetSize`, `File_LoadAlloc`, `File_LoadInto`, `File_OpenBig` but they
  store 0 instead of 1 in word +0xc of the argument block, which neither the callbacks nor `FileSys_RunWithHeap`
  read. Note: `speechload.rs` calls 0x2c8068 `OPEN_BIG` and `crowdbox.rs` `OPEN_FILE`; its callback is
  `File_LoadAllocCb` 0x2c7f60 (load the whole file into new memory). `File_PeekUnpackedSize` 0x2c88d8 (reads the
  first 0x80 bytes, returns `Compress_UnpackedSize`; 0x2c8910 is the same without the read check,
  `File_PeekUnpackedSizeUnchecked`), `File_LoadUnpackInto` 0x2c8a90 (reads, unpacks if packed, copies at most n
  bytes, then flushes the caches; `File_LoadUnpackInto2` 0x2c8bc8 is a jump to it, called by the lesson handlers'
  Enter), `File_WriteSync` 0x2c8e28 (`File_ReadChunked` with `FileSys_QueueWrite`, as `File_ReadSync` passes
  `FileSys_QueueRead`).
- **Disc and queue:** `CdFs_CdCallback` 0x2c9498 (given to `sceCdCallback`), `CdFs_StartRead` 0x2c9e20 (from
  `FileSys_DevRead`: splits a read into whole and part sectors), `CdFs_RecoverRead` 0x2c9710 (re-inits the drive,
  rereads sector 0x10, then the pending read) and `CdFs_PollRecover` 0x2c9930, `FileSys_SetPriority` 0x2ca460
  (op +0x10, re-sorts the queue), `FileSys_GetOpState` 0x2ca3b0 (+0xc), `FileSys_GetOpContext` 0x2ca3e0 (+0x14),
  `FileSys_OnBigOpened` 0x2cadc0 / `FileSys_OnBigClosed` 0x2cafa8 (callbacks of the BIG mount and unmount),
  `FileSys_SetHeap` 0x2cb1f0, `FileSys_ParsePath` 0x2cb830 (splits `device|path`), `HostFs_Init` 0x2cc2c0
  (adds `HostFs_ServiceRead` as a poll task). `disc.rs` says 1000 files is "0x2cc2c0's default"; 0x2cc2c0 has no
  such number (left for the owner of disc.rs).

Unclear (batch 22): 0x2cf2f8 (ReferThreadStatus, result unused), 0x2c7098 (stream call from the movie player),
0x2c73a0 / 0x2c7de8 (request word +4), 0x2c7d20, 0x2c84a0, 0x2c8ef8 / 0x2c9070 (sync ops through the stub 0x2cebb0,
which always returns 0), 0x2cac38 (queues op type 7), 0x2cb9c8 (empty), thunks 0x2cc328 / 0x2cc348.

## Names from F1c batch 23 (proven)

Host code linked into the exe: gcc 2.95's C++ runtime (libgcc2 `L_eh`, `frame.c`, `exception.cc`,
`tinfo.cc`/`tinfo2.cc`, `fp-bit.c`), newlib and Sony's libkernel, plus a few libmpeg helpers. Each was read in
Ghidra and matched to the library's code by its structure, its callers and the fields it touches; the names are
the libraries' own (static ones included).

- **Exception throwing (libgcc2):** `old_find_exception_handler` 0x2f4998 (3-word {start, end, handler} table
  ending in -1), `find_exception_handler` 0x2f4a70 (4-word entries, match function per entry),
  `get_reg_addr` 0x2f4bb8 (follows saved-in-register state 2, offset state 1 from the CFA, else `abort`),
  `copy_reg` 0x2f4c40, `next_stack_level` 0x2f4cc0 (calls `__frame_state_for`), `__unwinding_cleanup`
  0x2f4d50 (empty), `throw_helper` 0x2f4d58 (called by `__throw` / `__rethrow`).
- **`exception.cc`:** `terminate` 0x2f67b0 (calls the handler at 0x35d3ac, initially `__default_terminate`),
  `__default_unexpected` 0x2f67c8 (the initial value of 0x35d9a4; calls `terminate`), `set_terminate` 0x2f67d8
  and `set_unexpected` 0x2f67e8 (swap 0x35d3ac / 0x35d9a4), `unexpected` 0x2f67f8, `__cp_exception_info` 0x2f6810
  (&info->value, +8), `__cp_eh_info` 0x2f6838, `__start_cp_handler` 0x2f6858 (caught +0x14 = 1, handlers +0x20
  += 1), `__eh_alloc` 0x2f6890 (malloc, `terminate` on failure), `__eh_free` 0x2f68b8, `__cplus_type_matcher`
  0x2f68d8 (language 4 = C++), `__cp_push_exception` 0x2f6948 (0x30-byte cp_eh_info: value +8 and +0x28, type
  +0xc, cleanup +0x10, next +0x18), `__cp_pop_exception` 0x2f69d8, `__uncatch_exception` 0x2f6aa0,
  `__check_eh_spec` 0x2f6ad0 (throws `bad_exception` when the spec allows it).
- **`frame.c` (DWARF2 unwind tables):** `fde_merge` 0x2f7b80, `end_fde_sort` 0x2f7c80 (aborts on count
  mismatch; heapsort inlined), `count_fdes` 0x2f7ee8, `add_fdes` 0x2f7f30, `frame_init` 0x2f7fb8, `find_fde`
  0x2f80f8 (object list at 0x406bd0), `extract_cie_info` 0x2f81e0 (augmentation "" / "eh" / 'z'),
  `execute_cfa_insn` 0x2f82e8, `__frame_state_for` 0x2f87c8, `fde_split` 0x2f89a0.
- **RTTI (`tinfo.cc` / `tinfo2.cc`):** `type_info::OperatorEq` 0x2f8d98 (`operator==`: same object or `strcmp`
  of the names), `type_info::OperatorNe` 0x2f93b8 (`operator!=`), `type_info::Before` 0x2f9438 (`before`:
  `strcmp` < 0), `__throw_type_match_rtti_2` 0x2f9460, `__is_pointer` 0x2f9928, `__dynamic_cast` 0x2f9a40,
  `__tfv` 0x2f9af0 (type info of `void`: name "v", `__builtin_type_info` vtable). The three operators carry
  CamelCase method names because the name check takes no `operator` names.
- **fp-bit (float):** `_fpadd_parts_f` 0x2f70a8 (float copy of `_fpadd_parts`, called by `__addsf3` /
  `__subsf3`; `_f` added by us to tell it from the double one at 0x2f5c88), `__mulsf3` 0x2f73a0 (between
  `__subsf3` and `__divsf3`; no callers).
- **newlib:** `std` 0x2fa4f0 (FILE setup with `__sread`/`__swrite`/`__sseek`/`__sclose`, from `__sinit`),
  `__sfmoreglue` 0x2fa548 (n × 0x58-byte FILEs), `__sfp` 0x2fa5b8, `__sprint_vfi` 0x2fea28 / `__sbprintf_vfi`
  0x2fea70 (`vfiprintf.c`'s copies of `__sprint` / `__sbprintf`, which are at 0x2ff770 / 0x2ff7b8; `_vfi`
  added by us), `quorem` 0x301ec0 (dtoa's Bigint quotient, called by `_dtoa_r`).
- **Sony libkernel:** `_SyncDCache` 0x306f88 (D-cache `DXWBIN` loop over a range) under `SyncDCache` 0x307030
  (DIntr/EIntr around it, start and end masked to 64 bytes; called by libpad), `_InvalidDCache` 0x3070c8
  (`DXIN`) under `InvalidDCache` 0x307170, `iWakeupThread` 0x307718 (wakes another thread with
  `_iWakeupThread`; the current thread is queued instead), `iRotateThreadReadyQueue` 0x3077b0 (queued, priority
  < 128), `TopThread` 0x307568 (the thread draining that queue: op 0 `WakeupThread`, 1
  `RotateThreadReadyQueue`, 2 `SuspendThread`), `InitThread` 0x307640 (from `_InitSys`: makes the semaphore and
  that thread, priority 1). `InitThread` / `TopThread` are ps2sdk's names for the same code (Sony's own
  unknown; ps2sdk has `topThread`, made `TopThread` for the name check).
- **libmpeg helpers:** `Mpeg_RunCallback` 0x2ea398 (calls the callback stored for a type at mp+0x40, +0xc +8k),
  `MpegWork_Init` / `_Mark` / `_Release` / `_Alloc` 0x2ea4b8 / 0x2ea4d0 / 0x2ea4e0 / 0x2ea4f0 (the work area
  bump allocator; `_Alloc` aligns and fails with "work area size is too small"), `Mpeg_GetPictureCommon`
  0x2ea570 (shared body of the three `sceMpegGetPicture*`), `Mpeg_StopDma` 0x2eab28 (stops the IPU_FROM, IPU_TO
  and SPR_TO DMA channels), `Mpeg_PrintError` 0x2eabf0 ("[MPEG ERROR]%s\n", from `Mpeg_Error`).
- Made into functions in Ghidra this batch: 0x2f67d8, 0x2f67e8, 0x2f6810 (were tails of the noreturn
  functions before them). They were added by hand to `function-index.csv` and `function-systems.csv` as
  `runtime`.
- **Unclear (batch 23):** the libkernel newlib stubs 0x306df8, 0x306f38, 0x306f60 (each sets errno 5 and
  returns -1; which newlib call each one is cannot be told apart); 0x306c50 (from `sceSifSyncIop`, clears
  0x35ee70 then jumps to 0x307f08); 0x306ca0 (no callers, waits on the vsync-start bit of I_STAT); 0x307438
  (syscall 0x5a) and its caller 0x307490 (from `_InitSys`, reads T3_MODE); the other libmpeg internals
  (0x2ea158, 0x2ea2a8, 0x2ea2c8, 0x2ea308, 0x2ea3e8, 0x2ea6f8..0x2f0e90).

## Names from F1c batch 24 (proven)

The debug link to the host PC (`CCommDriver` and its Deci2 driver, 0x2b6ae0..0x2b7e78) and the `Cstring` helpers:

- `CCommDriver::CCommDriver` 0x2b6fd0: stores `CCommDriver::vtable` at +0x1c8, clears the counters and builds two
  arrays of ten `Cstring`s (+0x38 and +0x100). No callers (an out-of-line copy).
- `CommStack_Resize` 0x2b70c0 (`realloc` the data to n words; +0 count, +4 data, +8 the last word),
  `CommStack_Pop` 0x2b7118 (resize to n−1), `CommStack_Clear` 0x2b7178 (resize to 0), `CommStack_Delete` 0x2b7128
  (clear, then `Mem_Delete` when bit 0 of the flag is set; from `~CCommDriver`). `CommStack_ResizeCopy1` 0x2b79a0 is
  the same code as `CommStack_Resize`, with no callers. The Deci2 driver's vf6 pushes a 0 on both stacks, runs
  `Deci2Comm_PumpReceive` until the top word is set, then pops both.
- `CommPacket_SetHeader` 0x2b7530: fills a 12-byte header (+0 short, +4 short, +6 byte, +7 byte, +8 word, +2 = 0);
  from the Deci2 driver's vf2 and vf5 (its sends).
- `Deci2Comm_HasPacket` 0x2b7700: the received byte count 0x3d37d4 > 0. `Deci2Comm_PollTask` 0x2b7848: the same as a
  bool; the driver's vf1 (`sceDeci2Open(0xe001, …, Deci2Comm_Handler)`) adds it with `Sys_AddPollTask`.
- `Deci2Comm_ReceiveRecord` 0x2b7550: `Net_ReceivePacket(buf, 0x4d8)`, from `CommDriver_RecvPacket`.
  `Deci2Comm_PumpReceive` 0x2b7570: `Net_ReceivePacket` into a 0x2800-byte stack buffer (the packet is handled
  through the driver's virtuals inside `Net_ReceivePacket`).
- `Cstring_InitInt` 0x2b81c0 (`sprintf "%d"`, then `Cstring_AssignAscii`), `Cstring_ToInt` 0x2b93b0 (`atoi` of an
  8-bit copy), `Cstring_ToFloat` 0x2b9400 (`atof`, to float), `Cstring_GetBinarySize` 0x2b9598 (the length word at the
  head of the buffer, 0 without one; `Cstring_GetBinary` copies that many bytes after it).

## Names from F1c batch 27 (proven)

The kernel's system call stubs (`li v1, n; syscall; jr ra`) that Ghidra's Emotion Engine extension names, put in
`ghidra/symbols.txt` after checking each number: 0x3063e0 `SetGsCrt` (2) .. 0x306c10 `Deci2Call` (0x7c), 52 stubs,
plus `RFU091` 0x307480 (0x5b) and the two copies of `RFU116_SetSyscall` (0x74) at 0x307428 and 0x30e460. Interrupt
context versions use the negative number (`iSignalSema` −0x43, `isceSifSetDma` −0x77, ...). `SetAlarm` 0x306560
uses 0xfc. Two numbers have two stubs each: 0x10 at 0x3064c0 / 0x3064d0 and 0x12 at 0x3064f0 / 0x306500; the first
of each pair is called with three arguments (`Timer_Init`, `Gfx_InstallVif1Handler`, `sceSifInitCmd`), the second
with four (the render setup 0x1dbad8 and 0x182430 pass the handler's argument), so they are `AddIntcHandler` /
`AddIntcHandler2` and `AddDmacHandler` / `AddDmacHandler2` (Sony's names; Ghidra already has the `…2` labels there).
`game/src/boot.rs` `ADD_INTC` is 0x3064d0, `AddIntcHandler2`. Also `entry` 0x100008.

## Names from F1c batch 28 (proven)

- `App_VSyncIntcHandler` 0x182170 (cause, arg): calls `arg`, then `SYNC`, `EI`, returns 1; `boot.rs` adds it with
  `AddIntcHandler2`(3, 0x182170, 0, `App_OnVSync` 0x1825d0). Settles the batch 18 gap at 0x182170.
- `TrickDef_Unload` 0x1545b8: clears the loaded flag 0x31e08c that `TrickDef_Load` 0x154580 sets (called from the
  game's teardown).
- `Score_GetTrickName` 0x157978: score + 0x250, the text `Score_AnnounceTrick` formats the trick's name into
  (`lesson.rs` `trick_text_nonempty`, `LessonOverlay_DrawTipPage`).
- `Rand_Init` 0x2401a0: copies the 0x18-byte default state at 0x3a4a00 into a random generator (the size
  `Rand_SaveSnapshot` writes); from `StaticInit_MemCard`.
- `Vec4_ScaleCopy21`..`31` (0x1780b8, 0x197d70, 0x1cae48, 0x1ce058, 0x1d8dc0, 0x1ea358, 0x201098, 0x208628,
  0x238a80, 0x23b588, 0x2b6a70) and `Mat4_TransformVec4Copy1` 0x2085e8: more out-of-line copies of the VU0
  scale (44 bytes) and `Mat4_TransformVec4` 0x178078 (64 bytes), no references.

## Names from F1c batch 29 (proven)

- `sceDmaPutEnv` 0x2e5950 (env): libdma's environment set: checks the fields (−1..−4 when out of range), then
  D_CTRL (bits from tables 0x35ade8 / 0x35adf8 / 0x35ae08, the release cycle), D_PCR, D_SQWC, D_RBOR,
  D_RBSR, and keeps a copy at 0x35ae18. `sceDmaReset` 0x2e5860 calls it on a zeroed 0x14-byte env.
- `Dma_Bzero` 0x2e5800 (p, n): n bytes zeroed (libdma's own; Sony's name unknown).
