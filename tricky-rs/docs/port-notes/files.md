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
    box, material block. Its `.ssf` physics record: +0 mass (1e30 never moves), +4 bounce, the flag defaults (0x1
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
