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
