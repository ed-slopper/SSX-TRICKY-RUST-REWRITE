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
