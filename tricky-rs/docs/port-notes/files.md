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
