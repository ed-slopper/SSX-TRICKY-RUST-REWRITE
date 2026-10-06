# Findings - SSX Tricky (USA) SLUS_203.26

## Executable
- MIPS R5900 ELF, stripped (no symbol table), one RWX load segment: file offset 0x1000 -> 0x00100000.
  File offset = address - 0xFF000 for .text/.data/.rodata.
- `.text` 0x100000-0x30F960, `.vutext` (VU microcode) 0x30F960-0x31D2A0, `.data` 0x31D300-0x35F6D0,
  `.rodata` 0x35F700-0x3BB790, `.sdata` 0x3BB900, `.sbss/.bss` 0x3BBB00-0x40C174.
- Two zero-filled, file-backed gaps usable as code caves: 0x31D2A0-0x31D300 (96 bytes) and 0x35F6D0-0x35F700 (48 bytes).
- Compiler is GCC 2.95-era (old g++ ABI). Internal engine prefix is "BX"; classes are `cSomething`.

## Class names survive (RTTI)
Every polymorphic class has a lazy type_info getter (`__tf`) that passes its mangled name string
(e.g. `10cDebugMenu`) to `__rtti_si` (0x2F8E00), `__rtti_class` (0x2F8E20) or `__rtti_user` (0x2F8DD8).
The getter's address is the first slot of the class vtable, so name -> vtable -> virtual functions can be
recovered mechanically. `tools/rtti_scan.py` does this: 394 classes, 291 vtables, ~2,100 virtual functions named
`Class::vfN_<addr>`. For single inheritance the base class comes from the third `__rtti_si` argument.

Vtable layout: 8-byte entries `{s16 this_delta, s16 pad, u32 func}`; entry 0 is the `__tf` getter,
so virtual N is at vtable + 8 + 8*N and call sites look like `(*(vt + 0xC + 8*N))(obj + *(short*)(vt + 8 + 8*N))`.

## Globals
| Address | Name | Notes |
|---|---|---|
| 0x337C58 | `gApp` | pointer to the application object. `+0x24` input manager, `+0x724` renderer, `+0x730` current `cGame`, `+0x734` front end |
| 0x337E08 | `gMenuManager` | debug menu stack, see below |
| 0x3352A0 | `gCheat_UnlockEverything` | int, non-zero makes every "is unlocked" query return true (readers at 0x163B10, 0x163BC0, 0x163CC0, 0x163D30, 0x163D98) |
| 0x3352AC | `gCheat_MaxAttrib` | int, max rider attributes |
| 0x3352B0 | `gCheat_Mallora` | int, Mallora board |

The three cheat flags are XOR-toggled by `cCheatCodes::Update` (0x26CF60), which matches a 12-button sequence
against tables at 0x347E60 / 0x347E90 / 0x347EC0 while a two-button chord (pad bits 0x600) is held.

## The leftover debug menu
A complete text-menu framework is still in the retail build:
`cMenu`, `cMenuItem` and item types (`cBoolMenuItem`, `cEnumMenuItem`, `cIntMenuItem`, `cFloatMenuItem`,
`cSubMenuItem`, `cPopMenuItem`, `cCommandMenuItem`, ...), plus the menus `cDebugMenu`, `cGameOptionsMenu`,
`cRenderOptionsMenu`, `cResolutionMenu`, `cBezierOptionsMenu`, `cSoundVolumeMenu`.

`cMenuManager` (global at 0x337E08):
```
+0x00 int   depth            +0x2C int pad index
+0x04 cMenu* stack[...]      +0x30 int last input event
                             +0x34 int active
                             +0x38 font (set from gApp+0x60 in cGame::Init)
0x1895B0 Reset   0x1895C8 IsActive   0x1895E0 SetActive   0x1895E8 Update
0x1897B0 Draw    0x1897F0 Push(menu) 0x189898 Pop         0x189760 GetButtons(mask)
```
`cMenu` virtuals (vtable pointer lives at +0x114): vf1 Update, vf2 OnCommand(item, id), vf4 Draw, vf5 OnEnter(mgr), vf6 OnExit.

`cGame::Init` (0x17E148) allocates a `cDebugMenu` (0x1D7C bytes, heap label "DebugMenu"), constructs it at
0x187850 and stores it at `game+0x2A0`. The constructor ends by calling 0x187AD0, which is an empty function
in retail - most likely where the dev build installed the menu. Everything downstream is still live:
`cGame::IsPaused` (0x17F078) treats an active menu manager as paused, the render function at 0x1C72E8 calls
`cMenuManager::Draw` when the manager is active, and the pause handler in `cGame::Update` (0x181638) still contains

```
if (gMenuManager.depth == 0) ShowOverlay(game->overlays, 6, -1);   // retail pause screen
else                         gMenuManager.SetActive(1);            // debug menu
```
Since nothing ever pushes a menu, depth is always 0. The `debug_menu` mod rewrites the first branch
(0x18181C-0x181824) into `gMenuManager.Push(game->debugMenu)`.

Root menu items: "BX Debug Menu" (title), version string, Return To Race (pop), Instant Replay (cmd 2,
only added when `game+0x40` is set and mode != 6), Restart Race (cmd 1), Render Options, Game Options
(AI Paused, End Race: User Rank, Force End Game), Sound Options, Exit the Game (cmd 0).

## From the old tools (D:\Games\games\ps2\ssx modding)
- The three copies of `SLUS_203.26` there are byte-identical to the one in this project's ISO.
  This project's ISO (`SSX_TRICKY_USA_MODDED`) is an ImgBurn rebuild from May 2023 with a re-packed `DATA/MODELS/GARI.BIG`
  (4,453,869 bytes vs 4,382,471 original) and shows broken textures in game. The two ISOs in the old folder are the
  untouched 2001 master; builds should use one of those via `--iso`.
- SSX-ElfLdr's `GameApi.h` addresses are for SSX (2000) SLUS-20095, not Tricky; none were reused.
- The multitool/level-editor crash in notes.txt (rebuilt `.big` twice the size, crash on load screen) is a data-format
  problem, not looked at yet. The loader code for `.big`/`.map`/`.pbd`/`.ltg` is now readable in `decomp/`, which is the
  way to find out what the rebuilt archive gets wrong.

## Ideas for next steps
- Name the `.big` (EA "BIGF"/RefPack) and level loaders and diff against what the multitool writes.
- Map pad bit masks (the chord 0x600 and the cheat tables) to buttons, then gate the debug menu behind a button combo
  so the normal pause screen is kept.
- Label rider physics classes (`c*Control`, `c*ControlState`) and expose tunables as mods.
