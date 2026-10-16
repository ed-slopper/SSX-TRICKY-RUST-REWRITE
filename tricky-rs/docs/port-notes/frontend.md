# World Circuit and unlocks: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## World Circuit (`Circuit_*` 0x16b000–0x16d400)

- Each race is heat 1, heat 2, final (6 riders each); 1st–3rd go through, 4th+ ends the event; AI riders
  finishing 4th–6th are out for the event. Medal = place in the final (show-off: by score).
- Points 15/10/5 per medal, paid as the improvement on that track and event; 240 in all fills every attribute
  bar (cap − start sums to 237–240 per character). Rank thresholds 0, 5, 20, 35, 60, 90, 120, 160, 200, 240 (Master);
  boards unlock at the same thresholds (the 999 board from the trick book). Each gold unlocks the next character
  (order 3, 4, 7, 0, 10, 5, 6, 1, 11, 8, 9, 2). A medal on a track unlocks the next in order 0, 1, 2, 3, 4, 8, 5, 11.
- Computer riders gain a quarter of the player's points as attribute points (f = min(1, 0.25·pts/(100 − base))).

## Unlocks (`Profile_ResetNewGame` 0x161b40, `Circuit_ProcessUnlocks` 0x16c420)

- New game: tracks Garibaldi, Snowdream, Elysium (and the tutorial) open (0x407); riders Mac, Moby, Elise, Eddie.
  Each improved gold opens the next rider: Brodi, Zoe, JP, Kaori, Marisol, Psymon, Seeiah, Luther.
- A medal on track k of the order Garibaldi, Snowdream, Elysium, Mesablanca, Merqury, Megaplex, Aloha, Alaska
  opens the next; a race medal on Alaska opens Untracked, a show-off medal there Pipedream.
- Outfits: 7 per rider; 1–5 from trick book chapters 1–5, 6 at Master. The trick book (6 × 5 tricks) is in
  data/tutorial/trickdef.dat (28-byte records); finishing it gives the UBERBOARD (999).
- Cheats (row E10a; `tricky_game::cheats`, checked: `tests/cheats.rs`): `cCheatCodes::Update` 0x26cf60 runs on each
  front-end player every frame (`cFE3D` vf5 0x265398). With the pad connected (device +4) the player's +0 takes the
  device's input word (+0x2c; a rumble when set: `FE_RumblePad`, the device's virtuals +0x34(1), +0x24(1, +0x4c)).
  While the player takes codes (+0x38): holding L1 + R1 (0x600), each new press (this frame's buttons & ~last
  frame's & ~0x600) goes into the twelve words at +4 (the count byte +0x34; a thirteenth press starts again at the
  first) and +0 is cleared. On letting go after exactly twelve, `CheatCode_Matches` (memcmp of 0x30 bytes) against
  0x347e60 unlock everything X ▲ → ● ■ ↓ ▲ ■ ← ● X ↑ (`gCheat_UnlockEverything` 0x3352a0), 0x347e90 max attributes
  ▲ ▲ → ▲ ▲ ↓ X X ← X X ↑ (`gCheat_MaxAttrib` 0x3352ac), 0x347ec0 Mallora X X → ● ● ↓ ▲ ▲ ← ■ ■ ↑ (`gCheat_Mallora`
  0x3352b0), then the one character code at 0x347ef0 X X → X X ↓ X X ← X X ↑ (`Career_ToggleCharacterCheat`: the
  word at 0x3352b8 toggled, any others cleared); the first match toggles its switch and plays the front end's
  sound (gApp+0x734 → +0xa4, virtual +0x54(3)). The count starts again whatever was entered. Checked in the
  runner on 200,000 frames (2,786 codes taken).

## Frontend state slots (`cFE3DState` and every `cFE3DState*` screen)

- Phase at state+0x10: 0 hidden, 1 fading in, 2 active, 3 fading out, 4 waiting for the next screen, 5 done.
- vf3 `Hide`, vf4 `TransitionIn` (optional screen fade by state+0x20: 1 half, 2 slow; shows the widgets, phase 1),
  vf5 `TransitionOut` (fade by state+0x24, phase 3), vf7 `Finish` (phase 5; asks vf10 `CreateNextState` for the
  next screen and starts it), vf9 `UpdateTransition` (moves 0→1→2 and 3→4 once the widgets stop animating),
  vf14 `Activate` / vf15 `Deactivate` (input on/off for the screen's widget group), vf16 `LayoutWidgets`
  (centres the title on 640), vf17 `Update` (per frame: pad input, back/accept, starting the next screen).
- Debug menu items (`cMenuItem` and subclasses): vf1 `HandleInput`, vf2 `IsSelectable`, vf3 `Measure`, vf4 `Draw`.
- Loading screens (`c1PTrackLoad`, `cFirstLoad` …): vf7 `DrawBackground`, vf8 `DrawOverlay`, vf9 `StartAssetLoad`,
  vf10 `PollAssetLoad`.
- Overlays (`cScreenOverlay` and every `c*Overlay`): vf12 `Draw` (sprites), vf13 `DrawText`, vf14 `Update` (per tick), vf15 `Reset` (at race start).

### The screen base ported (row G9b; `game/src/fe3dstate.rs`, checked: `tests/fe3dstate.rs`)

The screen's fields beyond the phase:
- +0x14 the next screen, +0x18 back allowed, +0x1c accept allowed;
- +0x20 / +0x24 the fade in and out (1 half a second, 2 1.5 s out), +0x28 the pause overlay allowed;
- +0x30 its menu, +0x34 its widget group, +0x38 the title widget, +0x3c the title text, +0x40 the help bar, +0x44
  the backdrop.

The front end (`gApp` 0x337c58 → +0x734):
- +5 the player who accepted, +0x50 the overlay up;
- +0x74 the two players (+0 connected, +0x3c their input mask for the screen);
- +0xa4 the 3D scene (its vtable at +0, the others' at +0xc), +0xa8 the screen stack.

A virtual is an 8-byte entry: the `this` adjust (a halfword), then the function.

Ported: `Hide`, `TransitionIn`, `TransitionOut`, `Finish`, vf8, `UpdateTransition`, `Activate`, `Deactivate`,
`LayoutWidgets`, `Update`, vf21, vf22, vf26, vf6 and vf12. Every call goes through `FeEnv`: the virtuals, the
front end's pad queries 0x26ce08 (pause), 0x26ce30 (accept), 0x26ce58 (back) and 0x26cef8 (any input), the fade
(`FE3DTransition_Start` 0x1ff648: three floats, the renderer word gApp+0x72c, a colour, 3), the heap, the "back"
screen's constructor 0x29e920 and the screen stack (0x275b10 push, 0x275c10 pop, 0x2ac030 top). Notes:
- `Update` gives the pause button to the overlay when one is up and not closing (it closes it, the 3D scene's +0x90
  and a half-second fade to black). Active, per player:
  - back (when allowed) makes and pushes the "back" screen and returns;
  - accept (when allowed) goes to the menu's +0xe8 with the player;
  - any input with an input mask goes there too, unless it was taken, noting the player at the front end's +5.

  Done with a next screen, it pushes it.
- `LayoutWidgets` gives every widget in the group's list whose +0x80 answers its +0x140. It centres the title text
  on 640, moving it by (640 − width) / 2 − x, and its two halves the other way. The help bar's second widget goes
  20 past the first's width, at its own height, and the third likewise at its own +0x84 (w 1).
- `Activate`'s call of the screen's own +0x20 takes no argument; a1 still holds the 1 from the group's call before it.
- The slot +0xe8 is the menu's input (with the player) but the overlay's "closing?" query (no argument). +0x28 is
  the widget group's input switch (a flag) but a plain call on a screen.

Checked in the runner on 30,000 random screens: their phases, flags and widgets (and whether those still animate),
an overlay or none, the players' masks, the widget list and the title's size. All 15 methods' calls, in order (the
fade's colour and the vectors by value, the size getter's answers written alike on both sides), and the memory
equal the original's. 10 of 10 mutants are caught.

### The title screen ported (row G9c; `game/src/fe_title.rs`, checked: `tests/fe_title.rs`)

`cFE3DStateTitle` (0x5c bytes, vtable 0x3b3350) adds to the base +0x48 the press-start prompt, +0x54 the EA logo
and +0x58 the copyright line. Its menu is the title text (+0x3c): two items, "start" and "extras", with the chosen
one in the first word of its +0x140.
- **Constructor:** the base built, then accept and back off.
- **`CreateNextState`:** "start" makes the mode select (0x54 bytes, vtable 0x3b3190). "Extras" makes the DVD
  content screen (0xb8 bytes, vtable 0x3b3270, its fields cleared, the 3D scene's +0xc0 and +0x88). Anything
  else makes none.
- **The builder (vf11):** the base widgets (0x274108), then the menu (+0x154 = 2) with its two items, built by the
  text widget's constructor 0x2b1138 with the font word at gApp+0x5c, their texts the pair of ids at 0x3b2108.
  Then two widgets with the font at gApp+0x60, each white, +0x118 0.8, +0x114 1, sized and placed:
  - the EA logo, text 0x6b, at (38, 410);
  - the copyright line, text 0xdd8, at (20, 428).

  Then the help bar's two buttons; the prompt at (its x, 428), its button's +0x118 0.8; the backdrop; the menu at
  (20, 330).
- **`Activate`:** both players' +0x38 set, the title music on (0x262438), the menu, logo and line taking input,
  the base's `Activate`. If the game booted to the title (0x265af8), the menu's +0x160(0) and the screen's +0x40.
- **`Deactivate`:** the players' +0x38 cleared, the group's input off, then the base's.
- **vf19:** the 3D scene's +0x10(0).
- **vf20:** the group's input off, and the music off (0x262a68) when "start" was chosen.
- **vf26:** the fade out 2 for "extras", then the screen's +0x30.
- **`LayoutWidgets`:** the logo's, line's, menu's and prompt's +0x140.

Checked in the runner on 20,000 random screens (the base's `Activate` and `Deactivate` running as the original):
the calls in order, with colours and positions by value, the answers and the memory all equal the original's.
10 of 10 mutants are caught.

### The number-of-players screen ported (row G9d; `game/src/fe_numplayers.rs`, checked: `tests/fe_numplayers.rs`)

`cFE3DStateNumPlayers` has a menu of two items (one player, two). +0x4c is its help line.
- **`CreateNextState`:** the players set (0x263670 with the item + 1 as a signed byte), then the front end's next
  screen for them (0x265970).
- **The builder:** the base widgets with the title (0x274278: 194.0, text 0xdac, 0x200a). Then the two items: the
  texts at 0x3b2a80, +0x118 the word at 0x3ae520, the highlight colour (1, 0.953, 0.729, 0.106) at +0x108, white at
  +0x110 and +0x50. Then the backdrop; no fade in, and the pause overlay allowed.
- **`Activate`:** the overlay's mode 2, the base's `Activate`, the fade out 1. If the game booted here, the menu's
  +0x160 by the profile's player count (0x164288 on 0x32b390), the base's vf22 from the first item to the chosen
  one, and the screen's +0x40. Then the help line.
- **vf19:** the menu music (0x262cd8) and the 3D scene's +0x10(0xd).
- **vf20:** both players reset (0x263270 on the front end's +0x7c and +0x7e), the front end's mode (0, 1, 4), the
  overlay's mode 6 and the scene's +0x78(1).
- **vf22:** the base's, then the one- or two-player music cue (0x2628e0 / 0x262920) when an item goes, then the help
  line 0x29ad28 (`highlight`: the text 0xca6 for one player, 0xca7 for two, via 0x2b0f68).

Checked in the runner on 20,000 random screens, with `highlight` on its own over 2,000. 8 of 8 mutants are caught,
including the byte's sign once the test had items past 0x7f.

### The mode select ported (row G9e; `game/src/fe_modeselect.rs`, checked: `tests/fe_modeselect.rs`)

`cFE3DStateModeSelect` has a menu of three items (0, 1, 2). +0x4c is its help line.
- **`CreateNextState`:**
  - item 0: the event select (0x54 bytes from the heap, the base constructor, vtable 0x3b2fd0);
  - item 1: the front end's number-of-players screen (0x265a58);
  - item 2: one player set (0x263670), then the front end's next screen (0x265970, the character select);
  - any other item: none.
- **The builder (vf11):** the base widgets with the title (0x274208: 105.0, text 0xdab, 0x200a). Then the menu's
  layout bytes +0x110..+0x116 (3, 0, 1, 2, 1, 1, 3) and the three items (texts 0xe0a, 0xe0b, 0xe0c; +0x118 1.0;
  numbered). Then the backdrop, the pause overlay off (+0x28 0), and the menu's +0x160(1).
- **`Activate`:** the mode select's music (0x2623d8), then the base's `Activate`. If the game booted here (0x265af8),
  the game mode word 0x32de8c picks the item the menu opens on, then the screen's +0x40 runs:
  - modes 1 and 6: item 0;
  - modes 2, 3 and 7: item 1;
  - modes 4 and 5: item 2;
  - any other mode: the menu is left alone.

  Then the screen's +0xb8 with the chosen item twice, and the help line.
- **vf18 / vf19:** tail calls into the music: 0x262bd0, and the menu music 0x262cd8.
- **vf20:** on item 2, or on item 1 with fewer than two players joined (0x265870), the music's 0x262ad0.
- **vf22:**
  - with no item going: item 1's cue (0x262840), and the front end's +0xf0 0;
  - otherwise by the item coming:
    - item 0: +0xf0 1, the overlay's mode 4, cue 0x2627f0;
    - item 1: +0xf0 0, mode 5, cue 0x262840;
    - item 2: +0xf0 2, mode 3, cue 0x262890.

  Then the help line, then the base's vf22.
- **vf26:**
  - item 1: the fade out (+0x24) and the pause overlay (+0x28) are each on with fewer than two players joined (the
    count is asked twice);
  - item 0: no fade out;
  - item 2: both on.

  Then the base's vf26.
- **The help line (0x29a7a8):** text 0xca3 for item 0, 0xca1 for item 1, 0xca2 for item 2 (via 0x2b0f68); none for
  any other item.

Checked in the runner on 20,000 random screens (about 2,500 per method; the game mode drawn from 0..9; items 0..3
and -1), with the help line on its own over 2,000. The base's `Activate`, vf22 and vf26 run as the original. 22 of 22
mutants are caught.

### The event select ported (row G9f; `game/src/fe_eventsel.rs`, checked: `tests/fe_eventsel.rs`)

`cFE3DStateEventSel` lists the items that the game type allows. The game type is the front end's +0xf0, which the
mode select sets:
- type 0: items 2, 3, 0 (0x3b2ad0);
- type 1: items 1, 4 (0x3b2ae0);
- type 2: items 2, 3 (0x3b2ae8);
- any other type: no items.

Each item is numbered 0..4 and has its own text (0xe85, 0xe04, 0xe05, 0xe06, 0xe07), help line (0xccc, 0xca4,
0xcc7, 0xccb, 0xca5), music cue and camera. Items 4 and 1 (the latter only with fewer than two players joined) are
the ones that leave the front end's 3D scene.

- **`CreateNextState`:** the 3D scene's +0x10 with the item's camera. Then, by the item (the game type read first):

  | item | camera | game mode (0x32de8c) | game type | next screen |
  |---|---|---|---|---|
  | 0 | 0xc | 7 | 0 | — |
  | 1 | 10 | 1 | 4 | number of players (0x265a58) |
  | 2 | 8 | 2 for type 0, 4 for type 2 | kept | type 0: the difficulty select (0x54 bytes, vtable 0x3b2ef0) |
  | 3 | 9 | 3 for type 0, 5 for type 2 | kept | — |
  | 4 | 0xb | 6 | 3 | one player set, then the front end's next screen (0x265970) |

  When no screen was made by then (including when the heap or a call answered none), the venue select (0x7c bytes,
  vtable 0x3b2e10).
- **The builder:** the base widgets with the title (0x274278: 224.0 for three items, else 194.0; text 0xdaa,
  0x200a). Each item: +0x118 1.0, the highlight colour (1, 0.953, 0.729, 0.106) at +0x108, white at +0x110 and
  +0x50, and the item's own number at +0. Then the backdrop.
- **`Activate`** (the screen before as its second argument):
  - the overlay's mode 2 when the chosen item leaves the scene;
  - no fade in (+0x20 0) when the screen before answers 1 from its +0x70;
  - the event select's music (0x262388), the base's `Activate`, and the help line.
- **vf18:** for game types 0 and 2, the music's 0x262c60.
- **vf20:** leaving on an item that leaves the scene, the music's 0x262ad0.
- **vf22:** the base's, then the coming item's cue when an item goes (0x262160, 0x2620e0, 0x262060, 0x2620a0,
  0x262120 for items 0..4), then the help line 0x29b5d0.
- **vf26:** the fade out (+0x24) only for an item that leaves the scene, then the base's.

Checked in the runner on 20,000 random screens (about 2,850 per method; game types 0..3 and -1, items 0..5 and -1,
the number-of-players and next-screen calls answering none half the time), with the help line on its own over 2,000. 22 of 22 mutants
are caught.

## The 3D transition and fade record (F1c, proven from the code, not ported)

One record drives both the screen fades and a blend of four renderer values (read by the renderer's virtual +0x134,
written back each tick by virtual +0x12c; probably the fog settings): +0 active, +4 mode (0 blend in, 1 blend out,
2 fade, 4 done), +8 fade phase, +0xc/+0x10/+0x14 the in/hold/out times (s), +0x18 the clock (+1/60 a tick), +0x1c..+0x2c
the saved values, +0x30..+0x40 the current ones, +0x44..+0x60 targets and deltas, +0x64 a "blend running" flag,
+0x74 the fade alpha.
- `FE3DTransition_Start` 0x1ff648: a fade (mode 2) when none is active. `FE3DTransition_Update` 0x1fece8 (it was
  named `VideoStream_Update`): fade alpha up over the in time, 1 for the hold, down over the out time; modes 0 and 1
  move the current values by the elapsed fraction of the delta and hand them to the renderer.
- `FE3DTransition_StartIn` 0x1ff2b0 (from `RaceScene_Play`): saves the renderer's values and blends toward type 0
  (0.125, 0.75, 0.75, 0.75), type 1 (the saved values), or type 2 (mixed). `FE3DTransition_StartOut` 0x1ff4c8
  (`cPreRaceHandler::Update`, `cEndRaceHandler::Update`): blends back to the saved values. `FE3DTransition_StopFade`
  0x1ff6a0 (also from `cFE3D::~cFE3D`): clears active when the mode is 2 or 3.

## Screen value tracks (`ScreenAnim_*` 0x1f0e58–0x1f14e0; F1c, proven from the code, not ported)

- A `ScreenAnim` holds 20 tracks of 0x20 bytes from +0x14 (`ScreenAnim_Init` 0x1f10c0 builds them with
  `ScreenAnimTrack_Construct` 0x1f0e58; `ScreenAnim_Delete` 0x1f1128 is the deleting destructor the loading screens,
  `cBackdropOverlay` and friends call). Track: +0 id, +0xc/+0x18 two ends (thousandths in modes 2 and 3), +0x10 period (ticks),
  +0x14 clock, +0x1c mode.
- `ScreenAnim_Eval` 0x1f1488 finds a track by id and returns `ScreenAnimTrack_Eval` 0x1f0f48 (0 if none): mode 0
  +0x18 + clock, mode 1 +0x18 − clock, mode 2 a sine between the two ends (centre (a+b)/2, 0 at clock 0), mode 3 a
  linear ramp from +0x18 to +0xc over the period, held at the end. `ScreenAnim_GetValue` 0x1f1448 returns the raw clock.


## The career file (row E12d)

The app keeps the game's career block as the memory card's image in `tricky-career.sav` beside the program (`career::CareerFile::open`: the image loaded if its CRC16 matches, else `Career::init_new_game`; saved with `save_image` whenever a run's records are kept). The medals the menus read are still `tricky-save.json`: the race results do not yet go into the block (row E12d2).

## The overlay manager's slots (F1c, proven from `OverlayMan_Init` 0x1a6fb8, not ported)

`cOverlayMan` holds 19 overlays at +0x10..+0x58: +0x10 HUD, +0x14 results, +0x18 high-score record, +0x1c next race,
+0x20 countdown, +0x24 replay control, +0x28 pause menu, +0x2c pop finish, +0x30 pop async load, +0x34 replay,
+0x38 lesson, +0x3c view high scores, +0x40 black fade, +0x44 unlocked, +0x48 lesson controller, +0x4c point
distribution, +0x50 select next venue, +0x54 select next event, +0x58 help race. Each overlay's `cScreenOverlay`
vtable is at overlay+0x28; the base's vf1..vf11 (0x1c01e0..0x1c0180) are empty hooks, each overridden by one overlay.
The small wrappers call one hook on one slot: `OverlayMan_CallReplayVf1` 0x1a7be0, `CallPauseMenuVf2` 0x1a7c58,
`CallResultsVf3` 0x1a7c88, `CallPopFinishVf4` 0x1a7c10, `CallPointDistributionVf5` 0x1a7cb8,
`CallSelectNextEventVf6` 0x1a7ce8, `CallSelectNextVenueVf7` 0x1a7d48, `CallNextRaceVf8` 0x1a7d18; `CallAllVf10`
0x1a7b30 / `CallAllVf11` 0x1a7b88 call it on all 19; `OverlayMan_GetReplayControl` 0x1a7c40 (+0x24),
`OverlayMan_GetLesson` 0x1a7c48 (+0x38). What each hook means is still open. `Overlay_DrawTextForView` 0x1a8110 is
`Overlay_DrawItemForView` 0x1a7f40 for the text pass (vtable +0x70 instead of +0x68): player mask overlay+0xc, −1 =
once, in the last viewport, over a full 640 × 480 view.

Also named in batch 12: `MCOverlay_Draw` 0x1cf628 (overlay+4 == 0 → `MCOverlay_DrawState` 0x1cf6e8, a switch on
state +0x54, 2..0x22, to the per-state virtual in the vtable at +0x428, like `MCOverlay_RunState` 0x1cf040);
`SpriteSet_RegisterRiderProfileShapes` 0x1f9660 ("rpb1".."rpb4" into slots 0x7c..) and
`SpriteSet_RegisterLoadScreenShapes` 0x1f9798 (19 shapes from table 0x39a608); `Loc_Unload` 0x17c118 (frees app+0x50,
the text `Loc_Load` 0x17c0a8 allocates); `Career_GetPendingVideo` 0x163f08 (career+4: `Game_UpdateOrShowResults`
switches to `cVideoLoadModule` when set, pairs with `Career_SetPendingVideo` 0x163f10); `AnimData_Destruct` 0x15a8c0
(the global at 0x3211a8: 446 `AnimFile_Destruct` 0x15ac48 records 0x18 apart from +0x724, the big file at +0x710,
`AnimMemBank_Destruct` 0x157e10).

## Memory card overlay (G9m, `game/src/mcoverlay.rs`)

The state machine at 0x1cf040 runs one function per state (state-2 indexes the table; 0x1f calls the owner's +0x6c, 0x12 and 0x20 wait for a press on statics 0x33efd4 / 0x33eff0). The card system (`o+0x224`) and the owner (`o+0x428`, second vtable) are reached only through virtual calls, so the port takes a `CardEnv` that answers by vtable offset; the state is changed through owner +0x1bc and the pad is owner +0x1c. After the dispatch a closing match on the state issues owner +0x15c / +0x164. The slot status records are 0x58 bytes from o+0x168, so only slots 0 and 1 fit before the card pointer at +0x224. Test: fake vtables with a this-adjust halfword, a shared seeded oracle, 686 mutants killed.

## Pause menu stack and main page (G9a1, `game/src/pausemenu.rs`)

The menu object keeps a stack of page records (0x138 bytes from +0x438; the top's index at +0x7e8, the record pointers at +0x7e0). Each record has an enter slot (+0x128 adjust, +0x12a index, +0x12c pointer or table offset) and a leave slot (+0x130 / +0x132 / +0x134): a positive index selects an eight-byte (adjust, pointer) entry of a table in the menu, otherwise the slot holds the pointer. `PauseMenu_Resume` 0x1ac410 leaves the top page (argument 0), pushes page n (the first push resumes the audio and plays the pause sound), clears the new record's +0x14 and enters it; `PauseMenu_PopLevel` 0x1ac580 leaves the top (argument 1) and pops: a still-open stack stops the current song, an empty one hides HUD item 6 and stops all audio unless the race object's +0x54 is 1. The main page (`BuildMainPage` 0x1ac678) has five items (kinds 1, 4, 5, 7, 6; text ids 0x1db, 0x1e0, 0x1dd, 0x1de, 0x1ef); `UpdateMainPage` 0x1ace00 handles back (0x120), the trick book view when +0x7f0 is set (pages by 0x100008 / 0x80004), up / down (0x20001 / 0x40002) and select (0x800): kind 1 pops, 4 and 7 open a dialog (7 only after `Profile_SyncAudioSettings` says there is something to save), 5 resumes page 1, 6 opens the trick book. 164 mutants killed.

## Pause menu options page (G9a2, `PauseMenu_UpdateOptionsPage` 0x1add28)

The options page is the record at +0x570 (selection +0x584, count +0x588); its items are 0x18 bytes from +0x58c: kind, value (+8), count (+0xc) and a wrap flag (+0x10). Back (0x20) pops one page, 0x100 pops them all. Left / right (the held word, mask 0x18000c) change the value: with no wrap it runs 0..=count (up while below the count, down while above 0), with wrap 0..count-1 round. The kinds: 8 brightness (value / 50 as a float to `Render_SetBrightness`), 9 and 10 the race object's mode and viewport, 0xb and 0xc the two cameras (step, the camera's own virtual at its vtable +0x14, then read its setting back into the value and 0x32ca38 / 0x32ca3c), 0xd the song (previous or next, then restart the race music, no click), 0xe 0xf 0x10 0x13 0x15 the five volumes (step, value = whole number of volume x 50, the volume kept at 0x32c9e8, ec, f0, fc, f8 for channels 0 1 2 4 3, no click); every other kind only clicks. 116 mutants killed.

## Save-before-exit menus and card operation starts (G9n, `game/src/saveexit.rs`)

`MemCardOp_StartSave` 0x1ce778 and `StartLoad` 0x1ce5b0 are one routine (save sets o+8 to 1, load to 0): the mode goes to o+0, owner vcall +0x1c4 (second vtable at o+0x428) with 1, then the card (o+0x224) vcall +0x84 with 0x80000 for the first operation (state o+0x2c = 4) or 0x27d4 (state 1), and the title string is built with sprintf into o+0x98 from two frame buffers. The three menus share a tick: overlay tick 0x1a8bd8, and while the operation state p+0x430 is 2 the operation is updated; once done its result becomes the "leave" flag and the state is cleared (the results list instead applies the career updates 0x161a98 / 0x161840 and leaves the state alone on success). A dialog answer (0x1aac08) of 2 cancels, of 1 with state 1 restarts the operation with the 0x1c4 prompt, anything else leaves. Menu A (0x1bbb30) selects among p+0x438 items skipping the separator at count-2, the last item opens the save dialog (state 1 when the profile has something to save), the others start a race by career fields. Menu B (0x1bc8a0) has five fixed items (0 and 1 set the career's pending action 0x3818 / 0x381c, 2 only when p+0x454, 3 a notice, 4 the save dialog); leaving with item 3 or 4 starts the race (0x113bb0) with career +0x3260 = 0x10 or 2. The results list (0x1b32c0) keeps a count at list+0x34 (zero when the race base +0x4f8 is set) and eight-byte entries (kind at +0xc): kind 0 depends on the career mode (1, 5, 4 each flag the HUD items differently), 1 / 2 / 5 call the race object, 3 the save dialog, 4 swaps HUD items. Test: seeded oracle, fake vtables with this-adjust halfwords, 30k cases over the five entry points.

## Names from F1c batch 15 (proven from the code, live Ghidra)

- `FE3D_GetSlotChar` 0x265788: the character byte of a slot (table 0x3470b8); `FE3D_FindCharSlot` searches it.
- `FE3DCharacter_SetSelected` 0x2670e0 (+0x154): `FE3DCharSel_SetSelected` clears it on all twelve and sets the chosen one.
- `FE_SetRumbleEnabled` 0x26d1c0 (pad object +0x48, which `FE_RumblePad` checks); 0x2a9848 sets it, rumbles, clears it.
- `FE3DTextList_RenderAll` 0x26ff40: runs a `cListFE3DRenderTextOp` over the list (from cFE3D vf6 0x264fd8).
- `Vec4_ScaleCopy18` 0x239930 (the same VU scale-and-store as the others).
- Still unclear: 0x262b78..0x262ca0 (calls on gApp+0x734 → +0xa4), 0x2661e8 (+0xa1), 0x2663e8, 0x269be8, 0x26ce58 /
  0x26ce68 (type 0x61 / 99 tests), 0x26cf28, 0x26f688 / 0x26f698, 0x265758, 0x263270, 0x265bb8.

## The app's results on the career block (E12d2, `src/ui.rs`)

- The app keeps its medals, best times and scores and unlocks in the game's career block (`tricky-career.sav`), as
  the game does: the player is slot 0 on the chosen character (`Career_AssignPlayerRider`), so the World Circuit's
  medals, points and boards are that character's (its record at +0x100 + 0x1c4 · character: +0x24 / +0x25 the show-off
  / race medal by course, +0x26 / +0x27 the course open in each, +4 / +8 the points, +0x58 the boards).
- The menus ask the block: `Career_IsCharacterUnlocked` for the rider, `Career_IsCourseUnlocked` for the track in
  the event's mode (the app's race is a World Circuit race, 4; its show-off a circuit show-off, 5; free ride 7, on the
  single events' course bits +8), `Career_IsUnlockBitSet` for the board (plus the app's own trick book for the uber
  board until C15b), `Career_GetCourseMedal` and the character's best time / score words for the panel.
- At a run's end `CareerRes::record_run` fills the career's race area from the app's standings (the riders'
  characters and player bytes, places, times in hundredths, the player's score in its stats block), the mode, course
  and heat, and runs `Career_CheckNewRecords` (high-score tables; in the circuit `Circuit_ProcessRaceResult`, which
  gives the medal at the final heat and its unlocks), then `Career_RecordBestTime` / `_RecordBestScore`. The race area
  comes from the app's standings, not the game's race objects (STANDIN, G4j's `cAI::vf17` tables).
- The slots are not in the card's part, so they are cleared at load and filled when the menus pick a rider. The old
  `tricky-save.json` (the app's own earlier save) is read once into the current character's record (medals, their
  points, the next course opened) and renamed `.json.carried`.
- Checked by unit tests (`ui::tests`: a won final gives gold, 15 points, the next course and rider, the boards to 5
  points; a worse final keeps the gold; another rider starts with none; a show-off's medal) and a run of the app on
  a carried save.
