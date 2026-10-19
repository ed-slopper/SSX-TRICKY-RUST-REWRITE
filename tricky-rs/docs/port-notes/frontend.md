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

### The venue select ported (row G9g; `game/src/fe_venuesel.rs`, checked: `tests/fe_venuesel.rs`)

`cFE3DStateVenueSel` lists the ten courses in the order of the signed byte table at 0x3a8af0. An item's number is its
place in that table, and the screen keeps the ten text widgets at +0x54..+0x7c. The help line (+0x4c) carries the
event's title.
- **`CreateNextState`:** the chosen course (the table's byte at the item) goes to 0x32de88, and the race is set up
  from the menu (0x2657a8). There is no next screen.
- **The locks (0x29baa0):** for each of the ten courses, whether it is unlocked in the current game mode (0x163bb8)
  goes to its widget's +0x100. Its marker on the front end's +0xfc list fades to 1.0 if unlocked, else 0 (0x269be8,
  f12 the level, then 2).
- **The builder:** the base widgets with the title (0x274208: 105.0, text 0xdad, 0x200a), and the menu's layout
  bytes as in the mode select, with +0x158 1. Each course's item has:
  - its name as text (+0x158; the word at the course table 0x332f28 + 0x1c × course + 4);
  - +0x118 the word at 0x3ae520;
  - +0x74 the name's hash (0x240ed8);
  - its number.

  Then the locks, the backdrop, the menu set on the overlay's venue (its +0x185, a signed byte) and no pause
  overlay.
- **`Activate`** (the screen before as its second argument): the locks, the medals shown (0x262d80 with 1), and a
  fade in only after a screen that answers 6 from its +0x70. Then the base's `Activate` and the event title. When an
  item is chosen, its video (0x263000 with its signed first byte and +0xa8). Then the venue select's music (0x2624b8).
- **`Deactivate`:** the medals hidden, ten "no video" calls (-1, 0), then the base's.
- **vf18:** for game type 4, the music's 0x262c60.
- **`LayoutWidgets`:** the event title, then the base's.
- **vf22:** the base's. When an item goes, the overlay's camera moves to the coming item's venue: the "next" move
  (0x26c1a8) when the coming item is one on from the going one, or 9 to 0; otherwise a pan (0x26c090). Then the
  coming item's video and the event title.
- **The event title (0x29c010):**
  - a World Circuit item (+0xa8 set): the career's event title id (0x16b8e0) as the text;
  - courses 6 and 7: while locked for mode 1, the course's name (0x16b860); once unlocked, string 0xde7 or 0xde8.
    The chosen string is copied wide (0x17cc80);
  - other courses, by the game mode: 1 and 7 format the name with string 0xd31, 2 and 4 with 0x31, 3 and 5 with
    0x32 (0x17ce38); any other mode leaves the line alone.

  The wide text then goes to the help line (0x2b0f18).
- **vf26:** the overlay's shot of the venue (0x26c2c0) and the venue chosen (0x263150), each with the item's signed
  first byte. Then a transition (0x1ff648: 1.0, 10.0, 0, gApp+0x72c, a zero vector, 3), then the base's vf26.

Checked in the runner on 20,000 random screens (about 2,500 per method; the base's `Activate`, `Deactivate`, vf22,
vf26 and `LayoutWidgets` run as the original). The locks and the event title are stubbed there, and run on their own
in a second runner over 20,000 cases. Items include places past the table, signed bytes past 0x7f, and the one-on and
9-to-0 moves. The order table is restored each case, with a byte past 0x7f planted past its ten courses half the
time. 22 of 23 mutants are caught; the survivor drops `a < b` from the "next" test, which `a == b - 1` already
implies.

### The difficulty select ported (row G9h; `game/src/fe_difficultsel.rs`, checked: `tests/fe_difficultsel.rs`)

`cFE3DStateDifficultSel` has three items: difficulties 0, 1 and 2.
- **`CreateNextState`:** always the venue select (0x7c bytes, vtable 0x3b2e10).
- **The builder:** the base widgets with the title (0x274278: 224.0, text 0xda9, 0x200a). Then the three items
  (texts 0xe03, 0xe09, 0xe08; +0x118 1.0; the highlight colour at +0x108, white at +0x110 and +0x50; numbered),
  the backdrop, and no fade in.
- **`Activate`:** the difficulty select's music (0x262338), the base's `Activate`, and the help line.
- **vf22:** the base's. When an item goes, the coming item's cue (0x2631b0, 0x2631f0, 0x263230) plays and the
  difficulty word 0x32dea4 is set to it. Then the help line.
- **The help line (0x29b9f8):** text 0xcc8, 0xcc9 or 0xcca for items 0..2; none for any other item.

Checked in the runner on 20,000 random screens, with the help line on its own over 2,000. 11 of 11 mutants are
caught. The other three of the row's seven functions are the destructor and runtime type entries.

### The character accept ported (row G9j; `game/src/fe_characcept.rs`, checked: `tests/fe_characcept.rs`)

`cFE3DStateCharacterAccept` confirms one player's rider. +0x2e is the player (a signed byte); +0x2c and +0x2d are
handed on through the character screens.
- **`CreateNextState`:** the character modify screen (0x54 bytes, vtable 0x3b03b0), with the three bytes copied.
- **The builder:** the base widgets with the title (0x274208: 105.0; text 0xdae for the first player, 0xdaf for the
  second). One item (the text word at 0x3af6b8, +0x118 1.0), the backdrop, and a fade in.
- **`Activate`:** the accept music (0x262228), the 3D scene's +0x58(1), the help line, and the base's `Activate`.
  If the profile word 0x32e5f0 is 0x10, it becomes 2.
- **`Deactivate`:** the player's rider is found on the front end's +0x20 list at (the player's pick at +0x7d +
  2 × player) mod 12, with C's signed remainder. It saves its selection (0x266190) with the first byte of the
  player's career slot (0x164318). Then the 3D scene's +0x58(0) and the base's `Deactivate`.
- **vf22:** the help line, then the base's.
- **vf26:** the player's career slot is asked for (the answer is unused). For the last player (the front end's
  count at +4 minus 1), the players' records are merged (0x1637f8) and the profile's audio applied (0x161a98). Then
  the base's vf26.
- **The help line (0x27a240):** text 0xca9, then the line's +0x140.

Checked in the runner on 20,000 random screens (players 0, 1 and -1; picks including 12, 13 and negative bytes;
career slot bytes past 0x7f), with the help line on its own over 2,000. 13 of 13 mutants are caught (the slot
byte's sign once the test had slot bytes past 0x7f).

### The hint book ported (row G9q; `game/src/fe_hintbook.rs`, checked: `tests/fe_hintbook.rs`)

- **The builder:** the base widgets with the title (0x274208: 105.0, text 0xe19, 0x200a), one item (the text word at
  0x3afb58, +0x118 1.0, numbered 0), and the backdrop.
- **vf26:** the screen's +0x30, as the base's.

Checked in the runner on 20,000 random screens. 5 of 5 mutants are caught. The row's other two functions are the
destructor and the runtime type entry.

### The video screen ported (row G9s; `game/src/fe_video.rs`, checked: `tests/fe_video.rs`)

`cFE3DStateVideo` plays one movie: +0x54 is its name, and +0x158 a word the player starts with (set by 0x2aba70). The
player sits at +0x154 with its vtable at +0. +0x15c is the screen pushed when the movie ends.
- **`Update`:**
  - with no player yet: one is made (0x2b9fd8), started (+0x10 with +0x54 and +0x158), and the screen becomes the
    front end's video screen (+0xf8);
  - otherwise: the player steps (+0x18), and when it answers finished (+0x28) the screen's +0x68 runs.
- **vf12** (the end):
  - the player is asked +0x30, the front end's video screen cleared, and the player released (+8 with 3);
  - when the player answered, the 3D scene's +0x50(6);
  - the screen stack is popped (0x275b90), and the following screen pushed if there is one;
  - the screen releases itself (+8 with 3).
- **`Activate` / `Deactivate`:** the 3D scene's +0x108 / +0x110; `Deactivate` also clears the front end's +0xf4.
- **vf27:** the player's +0x20(-1). **vf11** is empty.

Checked in the runner on 20,000 random screens. 9 of 9 mutants are caught. The row's other functions are the
destructor and runtime type entries.

### The widgets the builders make in place (`game/src/fe_widgets.rs`)

The bigger builders (the character select's, the custom board's, the rider profile's, the options') don't call the
widget constructors: the compiler put the constructors' bodies in place, so the builders write each widget's words
themselves. `fe_widgets` has those bodies, each writing what the game writes and leaving the rest of the object
alone:
- **`widget_init`** (`cFE3DThing` then `cFE3DWidget`, as 0x267f58 has them out of line). It writes:
  - the thing's links: +0 -1, +4 and +8 the object;
  - its flags: +0x14 and +0x28 1, the rest 0;
  - its vectors: +0x40 and +0x50 zero, +0x60 white, +0x80 (0, 0, 0, 1), +0x90 zero;
  - its colours: the highlight (1, 0.953, 0.729, 0.106) at +0xac, white at +0xbc, two half greys at +0xcc and
    +0xdc;
  - the widget's vtable 0x3b5e18, +0xa8 1, its owner at +0xec, its parent at +0xf0, and its position (0, 0, 0, 1)
    at +0x100.

  Then it calls the colour update (0x2ad988). +0x34..+0x3c, +0x78, +0x9c and +0xf4..+0xfc are left as they were.
- **`group_init`:** the widget, then the group's vtable 0x3a9e98 and its list set up (0x2abfb0 on +0x110).
- **`text_entry_init`:** the widget, then the entry's fields: no text or second widget (+0x110, +0x114), +0x118
  15.0, +0x11c 2, (0, 0, 0, 1) at +0x120 and +0x140, +0x130 6. Its vtable ends as the text entry's, 0x3ae7f0.
- **`progress_init`:** the widget, then the bar's colours (+0x110..+0x14c), its range (100, 0, -9999, -9999) and
  value fields, its three sprites none (-1), and its vtable 0x3b6cf8.
- **`icon_init`:** the widget, the icon's vtable 0x3b6f88, no sprite (+0x110 -1), +0x114 0.
- **`vcall_class`:** a virtual call through a class's vtable named in the code (the builders size a widget they
  have just made through its class's table, not the object's).

The tests give these objects the game's own vtables, so the functions behind the slots the builders call (read from
those vtables at start) are stubbed by address. 10 of 11 mutants of the helpers are caught, through the character
select's and the user name's tests. The survivor changes the progress bar's third sprite (+0x174); the character
select, the only builder with progress bars, always overwrites it.

### The character select ported (row G9i; `game/src/fe_charselect.rs`, checked: `tests/fe_charselect.rs`)

`cFE3DStateCharacterSelect` lists the twelve riders in the front end's character order (0x265788). +0x2e is the
player (a signed byte). Beside the menu it keeps a stats panel (+0x120):
- four stat rows, each a text entry holding the stat's name, a progress bar (+0xc8) and a number (+0xd8);
- the rider's name (+0xec) and a heading entry (+0xe8);
- ten rank icons (+0xf0) and a rank marker (+0x118);
- a profile line (+0x11c).

+0xc4 times the panel's swap between the stats and the rank.
- **The constructor:** the base constructor, the vtable 0x3b0490, the bars and numbers cleared, the stats' text ids
  (0x25, 0x27, 0x28, 0x29), no panel.
- **`CreateNextState`:** for the World Circuit (game type 2), the user name screen (0x78 bytes: +0x68 1, +0x6c 1,
  +0x70 0). Otherwise the character accept (0x54 bytes). The three bytes are handed on.
- **The locks (0x276d78):** each item's +0x100 says whether its character is unlocked (0x163b10).
- **The builder:**
  - the twelve riders' items (no parent; the rider's name, the record kept at +0x84), then the locks;
  - the panel, a group under the help bar;
  - each stat row:
    - its entry at (170, 45 + 15k);
    - the name at 0.7 height;
    - a bar 100 × 30 with sprites 0x1f / 0x1e / 0x1d, placed at (250, 10);
    - a buffered number at (450, centred on the row by the small font's height);
  - the rider's name: 0.8 height, above the heading, hidden;
  - the heading entry: text 0xdd, at (250 − the sprite's width − 6, 75), hidden;
  - the ten rank icons: sprite 0x1c, spaced by the sprite's width + 6 from x 250, hidden;
  - the rank marker: sprite -5, hidden;
  - the profile line on the help bar, then the backdrop.
- **`Activate`** (the screen before as its second argument):
  - the locks; the pause overlay off;
  - the player's rider: booted, from the player's slot's profile (0x164330, 0x2658e8); otherwise the player's pick;
  - the menu set on that rider, the selected rider set (0x261b38), the stats panel;
  - the overlay's shot: the rider shot when booted, else the camera shot;
  - after a screen answering 6 from its +0x70 whose player is lower, every rider's choice restored for this player;
  - each rider shown (0x2661e8): 1 for the pick when booted, else 0;
  - the 3D scene's +0x58(1); when not booted, the select's music start; the base's `Activate`; when booted, +0x40.
- **`LayoutWidgets`:** the base's, then the stats panel.
- **The stats panel (0x278840),** by the item:
  - **an unlocked rider:**
    - marked seen (0x1635b8);
    - the four stats for the player's slot and the rider's outfit (0x1646b0). Each bar is sized to its maximum × 5
      and given its range and value, and each number is printed (value + bonus, format 0x3af5d0);
    - the rank icons get sprite 0x1b at the rank and 0x1c elsewhere. The marker moves to the rank's icon, and the
      rank's name is centred over it, then again over icon 9 − rank;
    - icons up to 9 − rank are shaded by tenths from the highlight towards (1, 1, 0, 0), with sprite 0x1b; the rest
      are white with 0x1c. The marker goes to the last shaded icon;
    - the panel shown; the help line's string 0x3af5c8;
  - **a locked rider:** the panel hidden; the help line the locked text for the rider's place in 0x3352c0 (the eight
    words at 0x3af5d8 for places 4..11). Places 0..3 would read the game's stack below the table; the port gives 0
    there (STANDIN). Those places are the starting riders 3, 4, 7 and 0, which are never locked;
  - then the help line's +0x140, and the profile line's text by the character's kind: 1 gives 0x14, 2 gives 0x12,
    any other 0x13. The 0x13 sits in a plain branch's delay slot, so it is the default.
- **`Update`:** while the panel is shown, the timer counts down. At zero, the rider name, heading, icons and marker
  swap places with the stat rows, for 0x10e frames when the name was shown and 0x78 otherwise. Then the base's
  `Update`.
- **vf18:** for the first player, the music's 0x262c20.
- **vf22:**
  - the base's; the coming item's +0x20(1) and the going one's +0x20(0);
  - the timer restarted (0x78 with the name shown, else 0x10e);
  - when an item goes: the selected rider set, the stats panel, the rider's line (0x261ee0) and the overlay's pan
    (0x26b4e8).
- **vf26:**
  - each player pressing (0x26ce08) becomes the chooser: +0x2c is the player + 1, the career's rider slot is set,
    +0x2d 0, and the player is added (0x271888);
  - the 3D scene's +0x10(-1); the pause overlay on;
  - this player's pick (+0x7c + 2 × player) becomes the item; the rider at the other byte's pick (+0x7d) is shown;
  - the rider shot, then the screen's +0x30.

Checked in the runner on 6,000 random screens with the locks and the stats panel stubbed, and those two on their own
in a second runner over 20,000 cases. The base's `Activate`, vf22, `LayoutWidgets` and `Update` run as the original;
the base update's phase is 0 with no overlay, so it runs only its first call. 33 of 34 mutants are caught (the
pan's signed byte once the test had item words past 0x7f). The survivor is the rank icons' first sprite pass, which
is equivalent: the shading loops rewrite every icon's sprite afterwards, whatever the second rank call answers.

### The character modify ported (row G9k; `game/src/fe_charmodify.rs`, checked: `tests/fe_charmodify.rs`)

`cFE3DStateCharacterModify` has six items: 0 is "done", and 1..5 open the screens that change the rider.
- **`CreateNextState`,** by the item:
  - **0 (done), for the last player:** by the game type, 4 gives the venue select, 3 gives the 3D scene's +0x60 and
    +0x10(-1) and the race set up (no screen), any other gives the event select;
  - **0 (done), for any other player:** the front end's next screen for the next player, with that player in its
    +0x2e, its rider slot set (0x164950) and its +0x2d (-2 after a +0x2d of 1, else the bits flipped);
  - **1..5:** the 3D scene's camera (0x1e, 0x1f, 0x20, 0x22, 0x23) and the item's screen, with the three bytes
    handed on:
    - 1: the custom body (0x58 bytes, vtable 0x3afe70);
    - 2: the custom board (its constructor 0x28c690);
    - 3: a 0x50c-byte screen (vtable 0x3b0110);
    - 4: a 0x114-byte screen (vtable 0x3b01f0, its +0x68 list set up);
    - 5: the user name screen.
- **The builder:** the title (0x274278: 314.0; 0xdae or 0xdaf by player), six white items (the texts at
  0x3af688), the backdrop, and no fade in.
- **`Activate`:**
  - back from a screen answering 7 from its +0x70, in game type 3 with that screen's +0xf4 clear, it only sets up
    the race;
  - otherwise: the modify music, the selected rider set and the overlay's rider shot of the player's pick;
  - after another player's screen: all twelve riders' choices restored when that screen answers 4, then the pick's
    rider shown with 2. This index is not taken mod 12;
  - the base's `Activate`.
- **vf20:** leaving on "done" as the last player, outside game type 3: the 3D scene's +0x58(0) and the music's
  0x262b78.
- **vf22:** the base's, then the help line by the coming item:
  - 0: the 3D scene's +0x10(-1) and text 0xca8;
  - 1..5: texts 0xcaa, 0xcb2, 0xcbe, 0xcc4 and 0xcc6 with the item's line (0x261f20..0x262020).

  Then the line's +0x140.
- **vf26:** the fade out only for "done", then the base's.

Checked in the runner on 20,000 random screens. Picks include 12 and 13, which reach the rider index the code
doesn't take mod 12. All 20 mutants are caught (one after picks 12 and 13 were added).

### The custom body ported (row G9m; `game/src/fe_custombody.rs`, checked: `tests/fe_custombody.rs`)

`cFE3DStateCustomBody` picks one of the player's rider's seven outfits. The rider is the front end's +0x20 list at
(the player's pick) mod 12, using C's signed remainder. +0x54 keeps the outfit the screen opened on.
- **The builder:** the title (0x274208: 105.0, text 0xb6, 0x200a), then seven items. Each item gets its number, the
  word at 0x3ae520, the outfit's name from the rider's set (0x332c88 + 0x38 × set + 8 × outfit), and whether it is
  available (0x267cc0) to +0x100. Then the backdrop and no fade in; the rider's outfit (its +0x159) is kept and the
  menu set on it.
- **`Activate`:** the menu set on the rider's outfit, the help line, the overlay's rider view off (0x26bcb8), the
  custom screens' music (0x262300), the base's.
- **vf18:** the rider view on.
- **vf21:** the rider put back in the outfit the screen opened on (0x267c48), then the base's vf21.
- **vf22:** the base's; the rider tries the coming outfit on; the help line.
- **vf26:** the rider saves its selection with the first byte of the player's career slot (0x266190), then the
  base's vf21 (the accept).
- **The help line (0x28de90):** 0xcab for a locked item; otherwise the texts at 0x3afba0 for outfits 1..6, and 0xcab
  for any other; then the line's +0x140.

Checked in the runner on 20,000 random screens, with the help line on its own over 2,000. All 13 mutants are caught
(the slot byte's sign once the test had slot bytes past 0x7f).

### The custom board ported (row G9l; `game/src/fe_customboard.rs`, checked: `tests/fe_customboard.rs`)

`cFE3DStateCustomBoard` picks one of the player's rider's twelve boards. The rider is the front end's +0x20 list at
(the player's pick) mod 12; +0x54 keeps the board the screen opened on. Like the character select it has a stats
panel (+0x8c) of four rows (+0x58 the text ids, +0x68 the bars, +0x78 the numbers) and a profile line (+0x88).
- **The constructor (0x28c690):** the base constructor, +0x54 0, no profile line or panel, the bars and numbers
  cleared, the stats' text ids (0x25, 0x27, 0x28, 0x29).
- **The builder:**
  - the title (0x274208: 105.0, text 0xb5, 0x200a);
  - twelve items, each numbered, with the word at 0x3ae520, the board's name from the rider's character's table, and
    whether the board is unlocked (0x267c70) in +0x100;
  - the stats panel, built as the character select's, except that each text's colour is set before its id and its
    +0x70 is 4;
  - the help bar laid out (0x2755e8) with the float at 0x348024 in f12, the profile line, the backdrop, no fade in;
  - the board the rider has kept, stored in +0x54, with the menu set on it and the panel filled for it.
- **`Activate`:** the overlay's board view off (0x26bb38), the board music (0x2622c8), the base's.
- **vf18:** the board view on. **vf21:** the rider put back on the board the screen opened on (0x267c20), then the
  base's vf21. **vf22:** the base's, then the panel for the coming item when one goes. **vf26:** the rider saves its
  selection with the first byte of the player's career slot, then the base's vf21.
- **The panel (0x28d668):**
  - the rider tries the board on;
  - an unlocked board (+0xa8) gets its four stats (0x1646b0) as bars and numbers, the panel shown, and the help line
    0x3af5c8;
  - a locked board hides the panel and puts the locked text on the help line (ten words at 0x3afb78 for boards
    2..11). Boards 0 and 1 would read the game's stack below that table; the port gives 0 (STANDIN). Those are the
    starting boards, which are never locked;
  - then the help line's +0x140, and the profile line's text by the board's kind: 1 gives 0x14, 2 gives 0x12, any
    other 0x13.

Checked in the runner on 8,000 random screens, with the panel on its own over 20,000. 19 of 20 mutants are caught.
The survivor turns the board number passed to the try-on call from a signed byte into a word. That is equivalent:
the boards are 0..11, which are the same either way.

### The rider profile ported (row G9n; `game/src/fe_riderprofile.rs`, checked: `tests/fe_riderprofile.rs`)

`cFE3DStateRiderProfile` has five items, each with a panel (a widget group at +0x4f0 + 4k) shown while it is chosen.
+0x4ec is a picture whose sprite follows the item, +0x504 the rank list and +0x508 its marker. +0x54 is set once the
rider's textures are in. The panels themselves are made and filled by the front end's own builders (0x27e918, then
0x27f600, 0x284f60, 0x282d30, 0x285230, 0x284c70), which stay calls out.
- **The builder:**
  - the title (0x274208: 105.0, text 0xda6, 12), the help bar's layout 12, no back;
  - the panels' builder, then five items (the texts at 0x3afac0, numbered), each given its panel (a group with no
    parent, +0x70 4);
  - the five fillers; each panel laid out and hidden, the first shown;
  - the title's +0x70 4, the help bar's and backdrop's 8, the help line sized 420 × 40, the backdrop.
- **`Update`:** waits for the textures (0x1f9d28) and sets +0x54, then the base's.
- **`CreateNextState`:** asks the help bar's fourth button (0x271848), then makes a video screen (0x160 bytes: +0x158
  1, +0x154 and +0x15c 0) playing the rider's film ("data/video/rr_" and the name at the character record's +8).
- **`Activate`:**
  - the 3D scene's +0xa8;
  - until the textures are in, the character's profile textures loaded (0x1f9c18; pictures from 0x11 by character).
    Characters past 11 would pass a leftover register; the port uses 0x11 (STANDIN);
  - the rank list set on the slot's rank and opened;
  - the marker placed level with the chosen line: the line's y, plus half the difference of the line's height and
    the list's +0x174, less 2;
  - the base's.
- **vf26:** the third item (the film) is picked only while the help bar's fourth button is enabled, then the base's.
  **vf21:** backing out once the textures are in.
- **vf22:**
  - the base's; every panel hidden and closed;
  - the help bar's layout: 12 on the third item, with its fourth button enabled while the rank list is on its first
    line; otherwise 8;
  - for items 0..4: the help text 0xcbf + item, the picture white (half dark for item 2) with sprite 0x7c, 0x7d,
    0x7d, 0x7e or 0x7f, and the item's panel shown and opened;
  - the help line's +0x140.

Checked in the runner on 14,000 random screens. All 23 mutants are caught.

### The options ported (row G9o; `game/src/fe_options.rs`, checked: `tests/fe_options.rs`)

`cFE3DStateOptions` lists five pages in its menu; each page has a text id (+0x7c + 12k), a page widget (+0x80) and
a group (+0x84). +0x78 is the page open (-1 for none). Page 3 (+0xa4) is the one whose choice opens another screen.
An item whose first word is 0xabcd1234 is a page's "accept" line. The pages' own builders, layouts and value
handlers are calls out.
- **The constructor (0x29e920)** and **vf12 (0x2a9208):** no page open, no back, the pages' widgets and groups
  cleared (vf12 after the base's).
- **The builder:**
  - the title (0x274278: 450.0, text 0xda5, 10) coloured (0.88, 0.1, 0.1, 0.1), with its extras (0x2a8520);
  - the pages' text ids (0xe18, 0xb0, 0xba, 0xcc, 0xb4); the menu's +0x114 and +0x115 bytes -1;
  - per page: its menu item; its group (no parent, +0x70 1, hidden then opened, under the title) holding the page's
    title centred at (320, 60); its page widget (0x26d3b8; +0x114 and +0x115 -1, +0x158 1; page 3's +0x154 0);
  - the five page builders; each page placed below the title (centred, with page-specific x and y for pages 1, 3
    and 4), laid out and its group closed; the backdrop; no fade in.
- **`Activate`:** nothing while the front end's +6 is set. Otherwise the base's, the open page's group opened, the
  values read back (0x29fb40), then either the screen's +0x48 (back from the memory card screen, whose +0x70 answers
  0xd) or the options music, and page 1's line chosen.
- **`CreateNextState`:** on page 3 only, the memory card screen (0x2c84 bytes, 16-aligned; 0x28f260 with (1, 1) for
  line 0, (1, 0) for line 1, else (0, 0)), returning its +0x42c, or none when it failed.
- **vf22:**
  - the base's; the going item's control's arrows hidden and the coming one's shown (a slider's through 0x26d8c8, a
    list's through 0x2ae3c0);
  - the help line (0x2aa998); the help bar's second button enabled only on an accept line with no page or page 3
    open;
  - with a page open, an item without a control moves the page's own way (0x2a9720.. by page), and on page 1 the
    line is chosen.
- **vf23 (a value changed):** on page 0, lines 2..6 set the 3D scene's +0x158, +0x178, +0x198, +0x168 or +0x1a8
  from the item's +0x160, each with a cue on its +0x50 (0x12, 0x13, 0x15, none, 0x16); on page 2, line 5 sets the
  sound volume.
- **vf26 (picking):**
  - with no page open: the page opens, the widgets reset onto it, its cue and opener run (page 1 also chooses its
    line; page 4 opens the screen's +0xb4 and disables the second button), the menu is closed and the group opened;
  - on page 3: the screen's +0x30 and +0x48;
  - on another page's accept line: the page's values kept (0x29f178, 0x29f508, or the display's for page 2; page 4
    keeps nothing), the control's arrows hidden, the page closed, none open, the widgets reset onto the menu.
- **vf21 (backing out):** with no page open, the base's. Otherwise the page closed without keeping its values (its
  closer, then its values put back), the control's arrows hidden, the group closed with the 3D scene's +0x10(0x24),
  none open.
- **`LayoutWidgets`:** the base's, each page title centred across 640, the pages' layout (0x2a9488), and page 3
  centred by the help bar's extent.
- **The display's apply (0x29f720):** the display page's six lines into the career's settings (words at
  0x32ca10..0x32ca4c): widescreen, the screen mode (which also sets the display's mode and resets its picture
  offset), two flags, a 2..6 choice, and the sound volume. Unsigned words become floats as the game converts them.

Checked in the runner on 20,000 random screens, with the display's apply on its own in a second runner. 37 of 39 mutants are caught, four of them after the test was widened: page answers that differ from the pages the
screen already holds, the scene's cue logged as a value rather than read as a vector, display words whose top bit is
set and whose dropped bits tie, and display lines that reach the screen modes 9..11. The two survivors are
equivalent. The game writes the screen mode's word twice, so the first value never shows. And when a page item's own
list is missing, the game calls through it unchecked, which the test can't reach without the game crashing.

### The user name ported (row G9r; `game/src/fe_username.rs`, checked: `tests/fe_username.rs`)

`cFE3DStateUserName` edits the player's name with an on-screen keyboard. +0x54 holds the name (up to nine
characters and its end), +0x70 the keyboard, +0x74 the name's text. +0x68 is set while the stored name shows, so the
first key typed replaces it. +0x6c is set for the World Circuit, where "done" goes on to the character accept.
- **`CreateNextState`:** for the World Circuit, the character accept (0x54 bytes) with the three bytes handed on;
  otherwise none.
- **The builder:**
  - the title (0x274208: 300.0, text 0xdb1, 0x200a) and one menu item (text 0xe1c);
  - the keyboard (no parent; +0x114 14, +0x118 1.0, white, keys up to 0x65), centred 73 above y 300 by its size,
    +0x70 2, its bytes +0x120 0xc and +0x121 / +0x122 0xd;
  - its cursor icon: sprite 0x2a sized 1.1 × the sprite, its colours and frames, opened, +0x114 0x60, +0x70 -1, kept
    at the keyboard's +0x11c;
  - the name's text (under the title, 0.7 high, highlighted, at (320, 100));
  - the name box (245 × 30 at (320, 94)) with its two ends (sprite 0x2c at x 197.5 and 442.5);
  - the keyboard under the title, the backdrop, the widgets reset onto the keyboard.
- **`Activate`:** the stored name (0x164918) loaded and centred, +0x68 set, the base's, then the screen's +0x40 when
  booted. **vf21:** the stored name loaded back, then the base's vf21.
- **vf26 (a key):**
  - delete (0x64) drops the last character;
  - done (0x65) stores the name for the player's slot (0x164968) and leaves: +0x30 for the World Circuit, else +0x28;
  - any other key is typed after the name while it is under nine characters, onto an empty name while the stored
    one shows;
  - the name re-centred after an edit.
- **0x29d7a8:** sets the World Circuit flag.

Checked in the runner on 12,000 random screens. All 23 mutants are caught.

### The trick book ported (row G9p; `game/src/fe_trickbook.rs`, checked: `tests/fe_trickbook.rs`)

`cFE3DStateTrickBook` shows the six chapters of the rider's trick book in its menu (items 0..5) and a chapter's five
tricks in a trick list (+0x104, items 7..11). The screen keeps:
- +0x54 the textures' state: 2 while the book's load, 1 while a chapter's load, 0 when ready;
- +0x58 the tricks on a page (5), +0x5c the chapter's first sprite, +0x60 the chapter shown (-1 none);
- +0x64 the trick film playing (-1 none), +0x68 the films' archive, +0xf4 set when backing out;
- +0xf8 the trick page, +0xfc the picture, +0x100 the rank colour bar, +0x108 the page count, +0x10c the trick's
  name box, +0x110 the book's cover.

The methods:
- **The builder (0x27a5b8):** most of its 14 KB is widgets made in place:
  - the title (0x274208: 105.0, text 0xdb0, 0x800e), the menu's bytes 4, the help bar's layout 0x800a, no back, the
    six chapter items (the texts at 0x3af6f0);
  - a backdrop icon (640 × 480, tinted); the page band, a tiled icon (vtable 0x3b6e40, sprite 0x3c, 640 × 80 at y
    320);
  - the cover (195 × 195 at y 150, its colour cycle, sprite -5 until the book is in), with the character's name
    centred over it;
  - the rank colour bar (sprite 0x2b at (480, 105)), the trick name box (0x2b0b90, 150 × 175 at (460, 142), scaled
    0.85), the picture (257 × 158 at (320, 205)) and its frame of eight pieces of sprite 0x29;
  - the trick page, a group at y 305 holding: three lines of sprite 0x29; the trick list (made by 0x26d3b8, vtable
    0x3b7fe0, at (68, 10), its arrows at x 29 and 563) with five trick icons numbered 7..11; the list's cursor
    (sprite 0x2e); and the page count (a buffered text under the help bar, at y 65);
  - no chapter shown; the backdrop; the cursor moved onto the list's first line, less 5 on x and y; the widgets
    reset off.
- **Choosing a chapter (0x27a288):** for a chapter that isn't shown, the widgets reset off, and for chapters 0..5:
  the chapter's textures loaded (the character's first sprite from a table of twelve, plus the chapter), state 1,
  the picture and the five trick lines blanked (sprite -5) and closed, the rank colour bar's colour (0x19e840), the
  page count. Characters past 11 would use a leftover register; the port uses the first character's (STANDIN).
- **The page count (0x27a4a8):** string 0xe6b with the chapter's tricks done and the tricks on a page, centred
  across 640.
- **`Update`:**
  - a film that has finished is released and the picture goes back to the chosen line's sprite;
  - state 2, once the textures are in: the cover gets sprite 0x86 and turns white, then chapter 0 is chosen;
  - state 1, once the textures are in: each trick line is opened, enabled when the trick is done, and given sprite
    0x7c + k; the list is centred by its line width (+0x15c + 90) with its arrows either side; the picture takes the
    chosen line's sprite and colour; the widgets reset onto the menu unless the screen is fading out;
  - then the base's.
- **`Activate`:** the 3D scene's +0xa8, the trick definitions loaded (0x154580); the book's textures loaded on the
  first visit (the cover still -5; the set by character, state 2), otherwise chapter 0 chosen if none is shown; the
  films' archive opened ("data/video/tb/tb_" and the character's name); the chapters up to the complete pages
  opened and the rest closed; the menu laid out; the base's.
- **`Deactivate`:** the trick definitions freed, a film playing stopped, all films released, the archive closed, the
  base's.
- **vf21 (backing out):** nothing while loading; from a trick back to the chapters (the widgets reset onto the menu,
  the picture back, the name box's string 0x3af5c8); otherwise +0xf4 set and the base's vf21.
- **vf26 (picking):** nothing while loading. A chapter is chosen if it isn't shown, and the widgets reset onto the
  trick list. A trick opens its film (the character's name and chapter × 5 + trick, 128 × 128) unless one is
  playing, and puts the film's texture on the picture's sprite 0x7b. Anything else goes to the base's vf26.
- **vf22 (the item that comes):** a film playing stopped. For a chapter: its page count, the help layout 0x800a, the
  chapter chosen once the book is in, the help text 0xdd9. For a trick: the picture on its sprite and colour, its
  name printed (0x154638, 0x1551c0), the rank colour, the help layout 0x800c with the fourth button enabled for a done
  trick, the help text 0xdda.
- **vf24 (leaving for a lesson):** the players' slots cleared (the second player's rider marked seen first), game
  mode 6, the 3D scene's +0x10(-1), the front end's game type 3, the screen's +0x28(3) and +0x48.
- **The destructor (0x28e360):** the vtable, the archive deleted, the base's destructor.

Checked in the runner on 8,000 random screens covering all eleven methods (about 700 each). The builder's icons,
group, list and page count carry the game's own vtables, so the functions behind the 26 slots it calls are stubbed by
the addresses read from those vtables. All 46 mutants are caught.

### The DVD extras ported (row G9t; `game/src/fe_dvdcontent.rs`, checked: `tests/fe_dvdcontent.rs`)

`cFE3DStateDVDContent` is the extras menu. Its menu (+0x3c) has nine items. Items 4, 7 and 8 open a sub-menu: the
characters (+0x68, which leads on to their demos, +0x6c), the jukebox (+0x70, two columns of songs) and the promos
(+0x74). The other items play a film. A menu swap runs as a state in +0xa8 (0 for none), stepping through the
items of the list going or coming (+0x94) until each is half through its slide. The screen also keeps:
- three cursors: the menu's (+0x54, a group of five pieces), the jukebox's (+0x98) and its marker (+0x9c);
- two captions (+0x58, +0x5c), the description (+0x60, with texts at +0x110 and +0x114) and its heading (+0x64);
- two pictures (+0x78 for the characters, +0x7c for the menu; their next sprites in +0xac and +0xb0) and four
  decorations (+0x80..+0x8c);
- the song's title and artist (+0xa0, +0xa4) and the song playing (+0xb4).

The four sub-menus are built by the front end's own builders (0x296870, 0x297178, 0x297fa0, 0x298368), which stay
calls out; vf24 and vf25 do nothing.
- **The builder (0x293e10):**
  - a backdrop (sprite 0x2f) and the help bar's buttons (0xe; buttons 2, 4 and 8 white), the help layout 10, and
    the bar moved to y 425;
  - the logo (sprite 0x30, 145 × 85 at (15, 10));
  - the two captions (text 0xcfc, the first in the small font, the second in the large one at 0.2 alpha), sliding
    in from x 1250 to y 20;
  - the four decorations (sprites 0x31..0x34) along the bottom, each growing from half size in turn;
  - the base widgets and the nine menu items (texts 0xcfb, 0xcf7, 0xcf6, 0xcf9, 0xcf8, 0xcfa, 0xcf5, 0xcfc, 0xcfd at
    0x3b2420; 0.9 high, red highlight, sliding in from x 740), the menu at (-640, 132);
  - the menu picture (262 × 224 at (355, 124)) and the characters' picture (297 × 283 at (349, 133)), hidden;
  - the menu's cursor: five pieces of sprite 0x35 side by side, each starting its pulse 12 frames later;
  - the jukebox's cursor (sprite 0x35) and marker (sprite 0x37);
  - the description (two texts at 0.8 high, tinted (1, 1, 0.533, 0), at (-620, 440), sliding in from x 640), then
    the sub-menus' builders;
  - the heading (string 0x3b24f0 at (90, 94), hidden);
  - then swap state 1 from the menu's first item, the menu laid out, the widgets reset off, the cursor hidden, and
    +0x20 2.
- **`CreateNextState`:** a video screen (0x160 bytes) playing the chosen film: a menu item's (evolve, tricks,
  tracks, charactr, music, audio for items 0, 1, 2, 3, 5, 6), a character's (`data/video/cv_` and the character's
  name), or a promo's (sled2pr, nbapromo). Any other choice gives the screen with no film.
- **`Update`:**
  - **during a swap:** the step widget waits until it is half through its slide. The FPU's divide by zero gives
    the largest float, not NaN, so a widget with no frames counts as through. At the list's end, the state's work
    runs:
    - 1: the menu is in once the textures are, with its caption 0xcf3 and the decorations;
    - 2: the menu goes out to the item's sub-menu (states 3, 8 or 10, each with its music cue);
    - 3, 8, 10: a sub-menu is in, with its caption (0xcf8, 0xcfc, 0xcfd). The jukebox also shows its song texts,
      calls the scene's +0xe0 and shows the chosen song;
    - 4 and 7: between the characters and their demos;
    - 6: the demos are in, captioned with the character's demo string;
    - 5, 9, 11: back to the menu.

    Otherwise the next widget in the list takes over (in states 8 and 9, the second column's line at the same place
    too);
  - **with no swap:**
    - on the jukebox, when the song ends (the scene's +0xf8), the next of the 21 songs is found in either column and
      shown;
    - on the characters or the menu, the picture swaps to its next sprite once the textures are in, or once its fade
      has finished;
  - **then:** the cursors are hidden and the active list's one is put on its line: the menu's cursor 80 to the left,
    the jukebox's 5 to the left with the marker shown; none on the demos. Then the base's `Update`.
- **`Activate`:** unless back from a film (the screen before answers 0xc), the start cue and the textures (2); the
  base's. **`Deactivate`:** asks whether the textures are in (the answer unused), then the base's.
- **vf22:**
  - the base's;
  - on the characters, the coming character's sound (the scene's +0x10) and picture (0x71..0x7a, none for 1 and 6),
    with the description 0xdd0 and the character's text;
  - on the demos, the character's text and its record's second string;
  - on the menu, the item's sound (0x3a..0x42) and picture (0x68..0x70);
  - on the jukebox, a column coming takes the going column's line, stepping back past lines it lacks, and the
    description 0xdd4 shows the song's artist.

  A picture that is fading is moved to two frames before its end.
- **vf26:** nothing during a swap. On the menu, items 4, 7 and 8 start the swap out, any other item below 9 a film
  (the base's vf26). On the characters, their demos open (state 4, the help layout 12). On the jukebox, the song is
  shown. On the demos and the promos, the base's vf26.
- **vf21:** on the menu, the base's. On a sub-menu, its swap back: state 5, 7, 9 or 11 (the jukebox first calls the
  scene's +0x100), then the widgets reset off.
- **The helpers:**
  - 0x299338 shows a song: the scene plays it (+0xe8), the marker goes to its line, and the title and artist come
    from the 21-song table at 0x3480c8;
  - 0x2994c8 and 0x2995a8 set the captions' text by id or by string, unless the first caption is moving;
  - 0x299688 shows both captions.
- **vf13:** answers 0xb. **The destructor:** the vtable 0x3b3270, then the base's.

Checked in the runner on 16,000 random screens over the ten methods, with the four helpers stubbed there and run on
their own over 4,000 in a second runner. The test positions are real floats: the VU's subtract treats a denormal as
0, which the port's arithmetic doesn't copy. 63 of 64 mutants are caught (two after the test was widened: the help
buttons' icons apart from the objects the captions write later, and a nonzero word where a missing list's line would
be read). The survivor turns the swap's `!(0.5 <= t)` into `t < 0.5`, which only differs for a NaN, and the PS2's
divide never makes one.

### The front end's small helpers ported (row G9b2a; `game/src/fe_helpers.rs`, checked: `tests/fe_helpers.rs`)

The front end's lists are intrusive. A node keeps its previous node at +4, its next at +8 and its vtable at +0xc;
+0 is the node's id, which the widgets use as their item number. A list is a head node with a tail node at +0x10
(both with id -1) and the list's vtable (0x3b5ae8) at +0x20. The head's next is the first entry and the tail's
previous is the last. The head points back at itself and the tail on to itself, which is how a walk knows it has
reached an end (`ListNode_IsSentinel` 0x2abf10).
- **The node:**
  - `ListNode_Index` 0x2abec0 counts the steps back to the head, so the head is -1;
  - `ListNode_Unlink` 0x2abf38 joins the neighbours, leaving one that is the node itself or none alone, and points
    the node at itself both ways;
  - `cListNode` vf2 0x2abf80 hands the node to an object's +0x10 (that object's vtable is at its +0).
- **The list:**
  - `List_Init` 0x2abfb0 and `List_Reset` 0x2ac160 set up and empty the list;
  - `List_First` 0x2ac018 and `List_Last` 0x2ac030 give 0 for an empty list;
  - `FE3DGroup_GetChild` 0x2ac048 gives the k-th entry (k below 1 gives the first) or 0 past the end;
  - `FE3DGroup_FindById` 0x2ac0c0 gives the first entry with an id (the code the options port called the k-th by
    number);
  - `List_PushFront` 0x2ac120, `List_PushBack` 0x2ac140, `List_Count` 0x2ac1e8, and `List_DeleteAll` 0x2ac178
    (each entry's +8(3), then reset);
  - `cFE3DGroup` vf12 0x2abe60: while shown, an event (vtable 0x3a9fe0) taken from the group's +0xa0 (0x2ac220),
    the group's +0x48, then the event destroyed.
- **The thing's tweens:** the routines our ports call `ICON_COLOUR` / `ICON_FRAMES` are `FE3DThing_SetTweenStart`
  0x2ac348 and `FE3DThing_StartTween` 0x2ac3b0. For tracks 1..6 they set the tween's start (+0x40), or its end
  (+0x50) with the track (+0x20), frames (+0x28) and mode (+0x2c), clearing +0x18, +0x24 and +0x30. Track 5 copies
  its vector at any alignment; the others with the quadword load and store, which take the addresses down to 16.
- **A widget's place** (`FE3DWidget_SetWorldPos` 0x2acb20): the widget's move (+0x70) by the position less its own
  (the VU's subtract on x, y, z; w 1).
- **The screen stack** (the list at the front end's stacks, the finished screens at +0x24):
  - each screen's +0x88 (0x275a70);
  - the top's +0x90, then the finished deleted (0x275ac8);
  - `FE_PushState` 0x275b10: the old top told of the new (+0x80), which is told of the old (+0x78);
  - `FE_PopState` 0x275b90: the same in reverse; it answers the screen taken off;
  - `FE_FinishState` 0x275c10: pop, +0x68, then onto the finished list;
  - `FEStateStack_Clear` 0x275c60.
- **The screen's players** (bits in +0x2d): `FE3DState_HasFlag` 0x271868, `FE3DState_AddPlayer` 0x271888 (a player
  below 0 sets every bit; the bit is made as the game makes it, 0x01000000 shifted and brought down by 24) and the
  bit cleared (0x2718c0, not in the index).
- **The help bar's buttons:** `FE3DButtonBar_FindButton` 0x271848 (by id), and `FE3DButtonBar_SetVisible` 0x2718f0,
  which shows each button whose id meets the mask, taking its bits off the mask as it goes, then lays the bar out.
- **The pad's key queries** (0x26ce08..0x26cee8): accept is 0x64 or 0xd, back 0x62 or 0x1b; the others test one
  code each (0x61, 0x63, up 0x4800, down 0x5000, left 0x4b00, right 0x4d00, 0x72, 0x52, 0x6c, 0x4c). The pad
  record's +4 (`FEPlayer_IsPadConnected` 0x26cef8) and +8 set (0x26cf28) go through 0x17a830.
- **0x2763f0** (`FE3D_AddToWorldList`): the widget added to the screen's world list (+0x34, 0x2abc68).

Checked in the runner on 40,000 random lists (well formed, empty, or with stray links between nodes), things,
screens and key events, about 900 for each of the 44 helpers. All 46 mutants are caught, two of them after the test added null links and an
unaligned position. Five of the 40,000 random lists loop with no end, which the game walks forever; the test skips
those.

### The front end's shared builders ported (row G9b2b; `game/src/fe_builders.rs`, checked: `tests/fe_builders.rs`)

- **`FE_CreateList2` 0x274108 (the screen's menu):** a list (0x1b0 bytes, made by 0x26d3b8, the list's vtable):
  +0x150, +0x190 and +0x158 1, +0x15c 4.0, +0x194..+0x1a0, +0x13c and +0x154 cleared, placed at (320, 93), +0x70 1.
- **0x2742e8 (the screen's widgets laid out and slid in; the constant our ports call `BACKDROP`):**
  - the menu (+0x3c) is laid out and centred on its size: moved by -0.5 × +0x170, through the VU's multiply;
  - with arrows (+0x144), the left and right arrows go to x 138 and 485, level with the menu: its y, plus half its
    height less the arrow's, plus 2;
  - when the items slide in (+0x150 is 1), each item not already moving fades from white to the highlight (track
    5, 30 frames, mode 2) and is hidden;
  - the menu is shown and the widgets reset onto it;
  - the title (+0x38) is put above the top by its height;
  - the prompt (+0x40) is put below by its extent and slid back up (track 1, 30 frames);
  - the help bar (+0x44) is laid out (0x2755e8 with sprite 7's height + 75), put at y 480 and slid up by its
    extent;
  - finally the world group and the screen are shown.
- **`FE3D_BuildButtonBar` 0x2719c0 (the help bar):** with any bits, the bar (+0x48: a list made by 0x26d3b8, its
  +0xf8(1)) gets one button for each bit of the 17-row table at 0x347f20 (the bit, a sprite, a text id). Each
  button is:
  - an entry (15.0 high) with its text in the small font and its icon (15 × 15);
  - given the help button's vtable 0x3ae690, its sprite and text (0x2ae230), and its bit as its id;
  - white, with its text's +0x114 1, added to the bar.

  Then the bar's spacing is 2.0, laid out and placed at (595, 37).

Checked in the runner on 20,000 random screens (menus with and without arrows and sliding items, titles, prompts,
help bars, and any button bits). When an allocation fails, the button's entry and icon both land at address 0, so
the entry is coloured through the icon's vtable; the test stubs that slot too. 33 of 34 mutants are caught. The survivor drops the
`bit == 0` test, which is equivalent: a zero bit never meets the mask.

### The widget and its text ported (row G9b2c; `game/src/fe_widget.rs`, checked: `tests/fe_widget.rs`)

A thing's tween has these fields:
- +0x18 running, +0x1c its start taken;
- +0x20 the track: 1 position, 2 position by an offset, 3 size, 4 size by a factor, 5 colour;
- +0x24 the frame, +0x28 the frames;
- +0x2c the mode (0 once, 2 back and forth), +0x30 running backwards;
- +0x40 the start, +0x50 the end.

A widget's colour comes from two flags, +0xa0 (chosen) and +0xa8 (enabled), and four colour sets: +0xac chosen,
+0xbc enabled, +0xcc disabled, +0xdc chosen while disabled.
- **The tween's start from where the thing is (0x2ac288):** for tracks 1 and 2 the position, 3 and 6 zero, 4 (1, 1, 1,
  0), 5 the colour. The widget's own start (vf5 0x2ad250) takes its size (+0x130) for the size tracks.
- **The start (vf7 0x2ad2c0):** an offset becomes a move to start + offset (a VU add) and a factor becomes a size of
  start × factor; then the tween runs from frame 0, forwards, and takes its first step. On the offset track the end's w
  (+0x5c) takes the VU register's leftover w, which the port can't know; it leaves the word as it was (STANDIN) and
  the test skips it.
- **The step (vf8 0x2ad380):**
  - the start is taken first (or the tween started);
  - the fraction is frame / frames, with the PS2's divide;
  - position: moved to the blend, start × (1 - t) + end × t, through the VU (the scale is 0x102f50);
  - size: set to that blend (its w is again the VU's leftover, so the test logs three words);
  - colour: each part blended on the FPU and kept in 0..1;
  - then the next frame. At the end, mode 0 hides the widget and mode 2 turns its direction.
- **The colour update (`FE3DWidget_UpdateColor` 0x2ad988):** picks the set from the flags unless a colour tween is
  running. Its three setters (vf29..vf31) store a flag first; the four colour-set copies (vf32..vf35) store a set.
- **Small parts:**
  - the bounds (vf36: the position, and position + size);
  - the vector at +0x100 (vf37 / vf38, quadword moves);
  - the size from four floats (0x2ad7e8);
  - the owner, flag and parent getters;
  - an event posted through the record at 0x347e10 (0x2ada58);
  - vf15 answers 1, vf28 answers 0;
  - the destructor (the widget's vtable, then the thing's 0x2acd10).
- **The single-line text (`cFE3DWSingleLineText`):**
  - its fields: +0x110 the font, +0x114 a 2-unit border, +0x118 the scale, +0x11c wide, +0x120 the text id (-1
    none), +0x124 the string;
  - the constructor (0x2b1138, the `TEXT_CTOR` every screen calls) is the widget's init, then those fields and the
    vtable 0x3b73a8;
  - the draw (vf11): the renderer's layer raised by +0x70, the drawer's colour, one of four font calls (wide or
    narrow, with or without the border: with it the scale goes first), then the layer put back;
  - the size (vf37): the font's width at the scale and its height × the scale, both 2 more with the border, and (0,
    0, 0, 1) for no string;
  - the string setters (vf40 wide, vf41 by id through the localiser, vf42 narrow); a text from an id fetched again on
    layout (vf39).

Checked in the runner on 40,000 random widgets and texts (about 1,150 for each of 34 functions), with the colour
update stubbed there and run on its own over 4,000 in a second runner. All 46 mutants are caught.
### The front end manager's per-frame logic ported (row G9b2d1; `game/src/fe_manager.rs`, checked: `tests/fe_manager.rs`)

The manager (gApp+0x734) keeps:
- +4 the players, +6 its state (0 the menus, 1 leaving for the game, 2 left);
- +0x74 / +0x78 the two players, +0x7c their picks;
- +0x80 a list of things drawn and updated;
- +0xa4 the 3D scene, +0xa8 the screen stack;
- +0xf0 the game type, +0xf4 the idle frames, +0xf8 the attract film;
- +0x124 the video manager.

The functions:
- **vf5 0x265398 (the frame's update):**
  - the transition and the video manager are ticked;
  - in the menus: each player's cheats, the drawn list, the sound bank's channels 3 and 4. With no attract film, any
    pad's input resets the idle count; otherwise, unless the top screen answers its +0x70, the attract film (a video
    screen playing 0x3a8d48) is pushed after 0xe11 idle frames. Then the finished screens are closed and the music
    ticks once;
  - leaving: the game is made by the career's state (0x24c748 for +0x3710, 0x24d7f8 for +0x2afc 6, 0x247a60 for two
    players, else 0x245840) and handed to the app, then state 2;
  - after the menus: two music ticks.
- **vf6 0x264fd8 (the frame's draw):**
  - the attract film, or a film screen on top, is drawn alone;
  - otherwise: the fade, then in the menus the scene. That is the renderer's set-up; with a course loaded, its camera
    and the drawn list over it; the 640 × 480 projection; the drawn list; then the frame's end.
- **The player count (0x263670):** fewer players clear the slots from the new count up; more seat each new one.
- **vf2:** the 3D scene's +0x128 clears +8. **vf3:** +8.
- **The small helpers:**
  - the character order (0x265788, the bytes at 0x3470b8) and a character's place in it (0x2658e8);
  - the pads in (0x265870) and two or more (0x265c28);
  - the career set for the game (0x2657a8, by its mode word: the tutorial, the trick book, or the players);
  - the character select made with the players not yet picked (0x265970), or after the players' screen (0x265a58)
    a 0x54-byte screen for two pads;
  - the light presets (0x265b10), an alternating value (0x265758), a conditional free (0x265bc8);
  - the picks, game type, idle count, vector and state accessors.

Checked in the runner on 40,000 random managers (about 1,400 for each of 28 functions); the update always has a screen
on the stack, as in the game. 44 of 45 mutants are caught, seven after the test was widened: bytes past 0x7f, the career word 2, an
idle count one short, and stub answers mixed by multiplying. A xorshift alone kept a call's successive answers in a
fixed relation, so the two pads were never one in and one out. The survivor passes 1 to the game constructors that
take one argument, which is equivalent.

The set-up and tear-down (the constructors 0x263280 / 0x265ca0, the destructor, vf1 and vf4) make the front end's
objects, load its fonts, models and sounds, and play the boot films. They are object glue, which row G9b2d2 leaves to
the app, as G4k does for the AI's.

### The front end's screen flow as a model (row G9u1; `game/src/fe_flow.rs`, unit-tested)

The app's menus (G9u2) follow the game's screens through a model made from the ports, until the game's own widgets
are drawn. `Flow` is a stack of screens, each with its cursor and the player it is for. Its items carry the game's
text ids (the strings in `DATA/LANG/AMERICAN.LOC`); courses and riders carry their numbers. Each pick does what that
screen's `CreateNextState` makes:
- **The title:** "Start Game" opens the mode select, "DVD Content" the extras.
- **The mode select:** highlighting an item sets the game type (its vf22): Practice 1, Single Event 0, World
  Circuit 2. Picking Practice opens the event select; Single Event the players (two pads) or one player and the
  rider select; World Circuit one player and the rider select.
- **The event select:** lists by the game type (Single Event: Race, Showoff, Time Challenge; Practice: Freeride,
  Trick Tutorial; World Circuit: Race, Showoff) and sets the game mode word:
  - Time Challenge: 7, game type 0;
  - Freeride: 1, game type 4, then the players;
  - Race: 2 with the difficulty select, or 4 in the World Circuit;
  - Showoff: 3, or 5 in the World Circuit;
  - Trick Tutorial: 6, game type 3, one player.

  Every pick except Freeride and the Trick Tutorial then opens the venue select (Race only after the difficulty).
- **The difficulty** sets its word, then the venue select. **The venue select** picks the course from its order
  (the bytes at 0x3a8af0) and leaves for the race.
- **The rider select** lists the twelve riders in the front end's order (0x3470b8) and opens the user name in the
  World Circuit, otherwise the accept; then the character modify.
- **The modify:**
  - Continue: the next player's rider select, or, for the last player, the venue (freeride), straight into the
    game (the tutorial), or the event select;
  - Outfit, Board, Rider Profile, Trick Book, User Name: their screens. An outfit or board picked is kept and the
    screen closes.
- **Back** pops the stack.

The app sets the career's locks (riders, courses) and the pads in; locked items can't be picked. Five tests walk the
World Circuit race, a two-player single-event race, both practice paths, the modify screens with back, and the locks.

## The 3D transition and fade record (F1c; ported in row G9b2e, `game/src/fe_transition.rs`, checked: `tests/fe_transition.rs`)

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

Ported on the game's bytes (row G9b2e):
- **`Update`:** the clock steps by 1/60 each tick. Each blend tick starts again from the saved values, then adds
  t × delta (blend in) or takes the targets less (1 - t) × delta (blend out). A finished blend lands on the targets;
  a finished blend out also clears +0x64. The values go back to the renderer through its +0x128.
- **The fade's phases** each hand their overshoot on to the next.
- **`StartIn`:** while +0x64 holds the blend's values, it only turns round a running blend out, keeping the values.
  Otherwise it stops any fade, saves the renderer's values (+0x130) and uses them as the current ones. The targets
  depend on the saved kind word:
  - 0: everything toward (0.125, 0.75, 0.75, 0.75);
  - 1: the saved values, unchanged;
  - 2: the first two toward (0.125, 0.75), the last two kept.
- **`StartOut`:** leaves a running fade alone; any other active mode except a blend in is stopped as a fade first.
- **0x1ff590 and 0x1ff608:** hand the renderer the saved values (with the second 0 and the third 1.0) or the current
  ones, while +0x64 is set.
- **The fade's draw 0x1ff000** (indexed as `VideoStream_Draw`) sets the renderer up for a full-screen quad:
  - the viewport from the renderer's size; its unsigned words converted as the game does;
  - the 640 × 480 projection with depths 0..1000, then states 0x13, 1, (2, 0, 0.08) and the fade's blend word;
  - then it draws the quad in the fade's colour and alpha through the drawer at 0x337fe8 (0x18df60, 0x19b108).
- **The arithmetic** is the PS2 FPU's (`ps2f::F`): no infinities (a result too large is the largest float), and
  division by zero gives the largest float. The comparisons read the values as the EE does (`c.ole.s`).

Checked in the runner on 40,000 random records, including infinite, largest, denormal and one-tick-short-of-0
times. 35 of 37 mutants are caught, one after those values were added. The two survivors are equivalent:
- stopping a fade on a record that isn't active changes nothing;
- `StartOut`'s stop only clears the active word, which it sets again straight after.

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

### The character select's board and outfit choices (row G3c; `game/src/fe_character.rs`, checked: `tests/fe_character.rs`)

The 3D character (`cFE3DCharacter`) keeps its rider at +0xb0, and its board and outfit at +0x158 and +0x159. When
either changes, a flag is set (+0x15c, +0x160). That is all `FE3DCharacter_SetBoard` 0x267c20 and `_SetOutfit`
0x267c48 do. A "slot k" below is the career's player slot (`Career_GetPlayerSlot` 0x164318: +0x30fc, 0x44 each, any
k), whose first byte names the player there. A player "on this rider" has a profile rider (profile word 0) equal to
+0xb0.
- **Blocked (0x267858; player p, player q, outfit):** q wears that outfit on this rider, or p doesn't have the outfit
  unlocked (`Career_IsUnlocked`, the record's +0x5c).
- **Taken (0x267910; _, q, outfit):** the first half of the above alone. Its second argument is not read.
- **Available (0x267cc0; n, outfit):** none of slots 0..n's players blocks slot n's player from the outfit, and
  that player has it unlocked.
- **A free outfit (0x2679a0; n, outfit, start):**
  - With n ≤ 0: the outfit when slot n's player has it unlocked, else 0.
  - Otherwise, when no player in slots 0..n is on this rider: the outfit as given, unchecked.
  - Otherwise: (start + 7) mod 7 when one of those players wears outfit 0 on this rider, else 0. It doesn't look
    for another free outfit; the result depends only on outfit 0.
- **The choice restored (0x267b18; n):** slot n's player's board and outfit when they're on this rider, else 0 and
  0. Each is kept only if unlocked for that player (the board by `Career_IsUnlockBitSet`, the record's +0x58). The
  outfit then goes through the free-outfit pick with start 1, and both are set.
- **A custom board unlocked (0x267c70; _, n, board):** the board's bit for slot n's player. The object isn't read.

All of these read the unlock-everything cheat (0x3352a0) through the career's two bit tests. Checked in the runner
on 30,000 random careers (the slots' players, profile and record pointers into the block, small profile words) and
characters, about 3,700 calls for each of the eight functions. The answer and the character's bytes were compared.
21 of 21 mutants were caught.

## Unused copies of the FE3D group virtuals (F1c batch 16, proven)

Four translation units each carry an out-of-line copy of `cFE3DWGroup`'s virtuals and of the list ops' `vf1`, which
nothing calls: no code reference, and no word anywhere in 0x380000..0x3c0000 (the vtables) holds their addresses. Their
decompiled bodies match the live functions token for token. The copies sit at 0x275e38..0x276b20, 0x28e640..0x28f198,
0x29dd00..0x29e858 and 0x2aac40..0x2ab798 (one more `cListFE3DFormatOp::vf1` and `cListFE3DUpdateOp::vf1` at 0x26fa70 /
0x270008). They are named after the op each one runs (numbered by address) and need no port:

| copy names (1..4) | live original | what it does |
|---|---|---|
| `FE3DWGroup_FreeChildren` | `cFE3DWGroup::vf1` 0x2691c8 | deletes the child list (+0x110), then `cListNode::vf1` |
| `FE3DWGroup_SkipActivity` / `_ResetActivity` / `_UpdateActivity` | vf6 / vf7 / vf8 | that op over the children, then the base class's vf |
| `FE3DWGroup_Render` / `_RenderText` | vf10 / vf11 | when +0x14 is set: the renderer's (gApp+0x724) value from its vf +0x1dc raised by +0x70 through vf +0x1d4, the op over the children, the value put back |
| `FE3DWGroup_Update` | vf12 | when +0x10 is set: the update op, then its own vf at +0x48 |
| `FE3DWGroup_Offset` | vf13 | the offset op, then +0x80 += the offset |
| `FE3DWGroup_OnInput` | vf28 | when +0xa4 is 0: 1 if a child took the input |
| `FE3DWGroup_Format` | vf39 | the format op |
| `ListFE3D<Op>Op_Apply` (Format and Update 1..5) | `cListFE3D<Op>Op::vf1` | calls the child's matching virtual (`cList::vf1` 0x2ac220 runs it on each node until one returns non-zero) |

The batch 13 "no xrefs" list's 0x2761c0, 0x28e9c8, 0x29e088 and 0x2aafc8 are the four `FE3DWGroup_FreeChildren`.

## Names from F1c batch 17 (proven)

- Menus: `MenuItem_GetKey` 0x188b48, `MenuItem_IsPadKey` 0x188b68, `MenuItem_GetButtons` 0x188b88 (item+4 its menu,
  +4 its manager); `cMenuManager::GetKey` 0x189718 (+0x30), `cMenuManager::IsPadKey` 0x189720 (the pad device's
  +0x2c == key); `cEnumMenuItem::cEnumMenuItem` 0x189c80 (the second constructor), `cCommandMenuItem::cCommandMenuItem`
  0x18a460; `ExpandMenuItem_ReplaceItem` 0x18b9f8 (no callers: takes the current item +0x10 out of the menu +0x14,
  sets the new one, adds it).
- Memory card screen: `FEMemCard_ShowLocDialog` 0x291da8 (the localised-string form of `FEMemCard_ShowDialog`),
  `FEMemCard_HideDialog` 0x2923b8 (+0x2c78), `FEMemCard_ShowPrompt` 0x292638 (+0x2c80, text 0xdeb or 0x15e, placed
  under the message +0x2c7c), `FEMemCard_HidePrompt` 0x2927a8, `FEMemCard_HideInfoGroups` 0x2930a8 (groups 0x15, 0x16).
- Text formatting (the wide-char printf): `Fmt_WriteNumber` 0x2c45e0 (one number conversion: digits by
  `Fmt_WriteDigits` / `Fmt_WriteAbsDigits` / `Fmt_WriteFixed`, then zero padding, sign, `0` / `0x` prefix, padding
  left or right; called from 0x2c4d1c, inside code Ghidra has no function for), `Fmt_WriteFixed` 0x2c4508 (the
  fixed-point digits with the `.`).
- `Vec4_SetFromVec3` 0x18df60: `Vec4_Set`(dst, v.x, v.y, v.z, w from f12).
- Not settled: 0x2c5558 (no callers), 0x26ac30, 0x260fa0.

## The results screens' pages (G9v, `resultpages`)

After a race, `cResultsOverlay` shows pages built into one table in the game's data.

- **The table:**
  - 0x3385c0 holds 8 rows of 16 cells, each a 0x80-byte wide string (a cell is at `row << 11 | col << 7`);
  - the rows' names are at 0x33c5c0 (8 of 0x80 bytes) and the title at 0x33c9c0;
  - the columns' places are 17 floats at 0x338550, with more places from 0x338598 and their alignments at
    0x3384d0.
- **The setters** (`Results_SetCell*`, `_SetRowName*`, `_SetTitle*`, 0x1bf918..0x1bfb2c) each copy, set from
  narrow, or format ("%d", "%d%s", "%f", "%f%s") into one place. The float ones pass the value as a double made by
  the compiler's `__extendsfdf2`. Two pairs of them only return a cell's or a row name's place. All of them ignore
  their first argument (the page).
- **`Results_SetRowLayout` 0x1aae10**(page; x, w) keeps x as the left edge. The first column is at
  x + 0.85·width + 12, and each later one 0.85·width + w further on; the width is the page's +0 → +4 → +0xc, a
  whole number. **`Results_ClearText` 0x1aae88** clears the names and the cells.
- **`Results_BuildPages` 0x1af720**:
  - resets the backdrop and clears the table, the overlay's verdict line and four 0x6e-byte lines (each with the
    word before it);
  - empties the menu and records whether the final unlock was had (the race's in career mode 4, else the
    show-off's);
  - sets the medal rank to 3 and the rows' riders to −1;
  - builds the page for the career's mode: 1 the stats, 2 and 9 the standings, 3 and 5 the medals, 4 the career, 6
    the lesson, 7 the time challenge, 8 none;
  - keeps the menu's count and queues the race's end lines (`Speech_QueueRaceEndLines`).

  0x1af940 is a separate small function that clears the speech's event.
- **The menu** is up to five items at the page's +0xc, each a kind and a string, with the count at +0x34. The count
  is read again after each item is stored, so a sixth item would overwrite the count itself. The pages never add
  more than five.
- **`Results_BuildCareerPage` 0x1af950:**
  - the layout comes from the HUD's sprite 0x37 (its x + 79, rows 18 apart), plus five fixed columns and four
    alignments;
  - it sets the rows' names, the page's numbers and the title of the event's mode;
  - in mode 2 the menu is continue, the points entry (only when there are none to spend), retry, quit and save
    (again only when none), and the verdict is shown with the first columns 37 apart;
  - in the other modes the verdict line says "advance" when no player finished in the first half of the race,
    else the mode's line, and the menu is the continue entry plus four more. The check uses the career's +0x3264,
    which is the finishing order's first word (0x32e5f4);
  - then each finisher's row: the place ("%d" with st, nd, rd, th), the name (the character's for a rider who
    isn't a player, the rider's own for a player), a fixed narrow string, and the time. With the verdict shown, a
    player's row also sets the rank (`Circuit_ClampMedalRank`) and makes the verdict.
- **`Results_SetMedalVerdict` 0x1b1ba8** picks the verdict line from the career's rank for the event (0x163e28):
  - 3 gives 0x249;
  - below the medal's rank gives 0x248;
  - otherwise rank 0 gives 0x247, and anything else 0x246.

Checked in the runner (`tests/resultpages.rs`): 300 cases of each of the row's 20 functions. Each case starts from
the ELF's own data at 0x328000..0x340000, with these made random:
- the career's mode and its event;
- the race's riders (players or not), the finishing order (six at most: its six words run into the times) and the
  times;
- the run-time globals (the lines, the modes' titles);
- the sprite's x, up to past 2^24 so the sums' order matters.

Every callee is a stand-in on both sides, answering from the case's seed. A format's buffer on the game's stack
and the port's scratch log as one token.

53 of 55 mutants were caught:
- starting the "advance" loop at 0 instead of 1 is equivalent, since its first check reads the same word;
- the career-mode test (4 against 3) can only be told apart once the medal page (mode 3) is ported (G9v3).

### The standings, the time challenge and the players' stats (G9v2)

- **`Results_BuildStandingsPage` 0x1b0188** has two layouts, chosen by the number of profiles:
  - one player: x + 79, rows 18 apart, five columns, row names on rows 0 to 3;
  - more: x + 87, rows 16 apart, six columns, row names on rows 0, 1 and 4.

  It sets the page's numbers (4 or 5, and the race's riders), the menu (0xdb5, 0x191, retry, save, 0x18e) and the
  title 0xe57. Each finisher's row has the place, the rider's own name and the time (fourth cell for one player,
  fifth for more), then the verdict as on the career page.
- **`Results_BuildTimeChallengePage` 0x1b07f8** is laid out from x + 119 with rows 3 apart. It has no numbers, a
  four-item menu and the title 0xe9c.
- **The race table is part of the career.** It starts at the career's +0x27dc, so:
  - a race rider's +4 is the index of its stats block (0xbc bytes at the career's +0x3294, the results' 0x32e624);
  - the career's +0x327c is the times table (0x32e60c);
  - the career's mode (+0x2afc) is the event's +0x320, which the medal verdict reads.
- **`Results_BuildRiderStatsRows` 0x1b0998**(this, who): `who` holds a player (+0) and a row (+4). The row gets
  six labels and the next row the player's stats:
  - the block's +0x7c;
  - +0 / 180 and +4 / 360 (whole-number divides);
  - +8 and +0xc;
  - the time at +0x5c × 100, made whole.

  It returns 6.
- **`Results_AddRiderStats` 0x1b1278**(this, who, col) does the same from column `col`, with ten labels. The
  values are:
  - the speed: +0x68 / +0x6c (the FPU's divide), times 0.6214 for miles when the career's +0x1680 is set,
    formatted with `sprintf` "%d%s" and the unit (0x38e438 / 0x38e440);
  - +0x10 and +0x1c;
  - four times (+0x5c, +0x50, +0x54, +0x58);
  - the fixed narrow string;
  - the race time from the times table.

  It first copies the narrow string's first byte into the buffer it then formats into (dead: the format writes
  over it). It returns the column after the last.

The same runner test covers them: 7200 cases over the 24 functions so far. It first got the career's mode wrong:
the test set the event's +0x320 after the mode, which is the same word, so `BuildPages` never saw a real mode. With
that fixed, G9v1's mode test is caught too (54 of 55).

44 of 45 mutants were caught. The one left changes the miles factor by a bit, which the truncation to a whole speed
does not show.

### The medal, stats and lesson pages (G9v3)

- **`Results_BuildMedalPage` 0x1b0bb8** (modes 3 and 5):
  - sets the overlay's box (363 by 320), the medal flag and the rows' ids to −1;
  - picks the menu: with the show-off's final unlock, continue, retry and save; else in the event's state 3,
    0xdb5, 0x191, retry, save and 0x18e; else continue when the player has points to spend, or 0xdb5 and 0x191,
    then retry and save;
  - writes each player's trick line (`Score_FormatTrickName` into the four 0x78-byte lines; the block here is the
    stats block's +0x74, at the career's +0x3308);
  - sets the page's number to 3 · players and the title to 0xe56.

  With one player it takes the medal from the circuit (`Circuit_GetMedal` on the order's +0xac and the event's
  +0x31c), shows the verdict in state 5, and lays out at 140 with the first column at 191. With more it collects the
  players with the best score (the block's +0x7c) into the overlay's +0xd0 (+0xe8 says how many; none when every
  player ties) and clears their rows' ids (+4 for player 0, +0x10 for player 1), with the first column at 20.
  Finally it fills the first player's stats rows (and the second's with two profiles) into the players' records
  at 0x3382d0 (the second at +0x78, row 3) and spaces the columns.
- **`Results_BuildStatsPage` 0x1b16f0** (mode 1) is laid out from x + 79 with rows 3 apart:
  - it marks as many rows' riders −1 as the page's +4 says (left over from the page before) and sets the menu and
    the title 0xe46;
  - with one rider the first column is at 192 (number 2), else at 70 (number 5);
  - it adds each player's stats from column 1 (the second player's only with more than one rider) and spaces the
    columns: 160, 90, 40 for one; 151, 80, 30 for more.
- **`Results_BuildLessonPage` 0x1b1aa8** (mode 6) is laid out at 142, with the first columns at 131 and 400, the
  title 0xe47 and the number 1. Its first menu item is written in place (+0xc) after the count is set to 0; the
  second is added as usual.

Checked in the same runner test, now 8100 cases over the 27 functions, with every career mode from 0 to 10 through
`BuildPages`. Two words the pages leave alone had to be kept realistic: the page's +4 (at most 16 rows) and the
overlay's tie count.

45 of 47 mutants were caught. The two left are equivalent: the best score starting at 0 instead of -1 lists the same
players while the tie count starts at 0 (as `BuildPages` leaves it), and adding 0 times a place to a place.

### The results' draws (G9v4)

- **`Results_DrawBestTrick` 0x1b2158**(this, who, align; x, y, w, h) draws a player's best trick in a text box built
  on its stack: the HUD's small font at 0.83 (0.747 in the app's mode 2 when the page's +0x48 is set), the heading
  0x1f9 in gold, shadowed at 0.66, and the trick line (`who` + 8, the lines the medal page fills) wrapped in the box
  from y + the font's height · 0.66 + 6. The heading's x depends on the alignment:
  - 0: at x;
  - 2: right-aligned to x + w by its measured width, with the box's alignment words set to (0, 1);
  - 1: whatever the caller left in f21 and f23. That path never sets them (a game bug), but its only caller, the
    versus page, passes 0 or 2. The port takes the two as `kept`.

  With the medal flag and one rider, the heading moves 25 to the left.
- **`Results_DrawVersusPage` 0x1b2428:**
  - It draws nothing while the HUD's +0x328 is set, and the memory card overlay while the HUD's +0x430 is 2.
    Otherwise it draws the high scores' text, then in gold (each colour set with the HUD's fade, +0x70, on its
    first component).
  - With the medal flag: the players' names and best tricks (one rider: the name at 226 and the trick at 191; two:
    names at 78 and 363, tricks right-aligned at 108 and at 303).
  - In mode 1: the names again.
  - In modes 4 and 2: with two profiles, the heading 0xe5c and each player's line (0xdde, a "%s" format) above the
    columns. It counts a blink at 0x338214 on by 1 a frame (back to 0 past 30). Each row of a rider who isn't a
    player draws that rider's score deltas (`HUD_DrawScoreDelta`, from the block's +0xd4 / +0xdc, plus +0xd8 /
    +0xe0 with two profiles, with blink / 2.5) and turns the colour white; a player's row turns it gold.
  - In mode 7, the time challenge: four headings, then for each profile its rider's name, character, time and that
    time's difference from the career's +0x2b08 (magenta when ahead, yellow else), rows 25 apart from 200. A box
    says who won: 0xc76 when the first is behind, or with two profiles when the second is; else 0xc77. The two
    differences start from two words at 0x38e448, both −1.
  - With the overlay's message (+0x20, a half word tested), a box with it, 15 lower when the verdict or the medal
    flag is shown.

  Each text box's colour and alpha are copied into it by hand (+0x3c..+0x48 and +0x38).

Checked in the same runner test, now 8700 cases over the 29 functions. The text boxes' and the font's functions are
stand-ins that log their float arguments; the colour stand-in logs the four words it is given, and the box's draw
logs the box's alignment and colour words. The runner's stack is cleared before each case, as the port's scratch
starts.

51 of 53 mutants were caught:
- the second start word read from the first is equivalent (both are −1);
- the second profile's "behind" test at a difference of exactly 0 is a boundary the cases rarely reach.

## The loading screens (G9w, `loadscreen`)

Every loading screen (gApp+0x734) is built on `cFlakeSpinnerLoad`: the bars at the top and bottom, the ticker,
the snowflakes counting the load, the tip boxes with pictures, a fade.

- **`FlakeSpinnerLoad_Init` 0x243c78** sets up the screen:
  - its table, and the two fonts with room for 16 glyphs each (+0xc menu, +0x48 title);
  - the HUD's shape −1 and its sprite record (+0xb8);
  - the pictures' archive (+0xd4), with the pictures not yet asked for (+0x160 = 1 "all in");
  - the bars' animation (+0x178) and the app's +0x728 (+4, the sprite set);
  - the fade at 1, not fading in, with both fade colours red; the time 0.

  Two of the sprite record's words (+0xc8, +0xd0) are written twice, the second time with the sprite's real
  values.
- **The load steps** (vf2 0x243f00, the step at +0xb4) wait for each file, one call at a time:
  - 0: the HUD's textures; their "map4" shape goes to the renderer's 0x150;
  - 1 and 2: the title and menu fonts, both scaled 1.25 by 1;
  - then the screen's own +0x50;
  - 3: the pictures in the list (+0x170 / +0x174, 0x34 bytes each: loaded, the file name, the 4-letter shape name
    at +0x29, the shape at +0x30), one file at a time. A packed one is unpacked into the 0x11800-byte buffer first.
    When none are left, the buffer is freed;
  - 4: done when the screen's own +0x58 says so.
- **`LoadScreen_LoadPics` 0x244290** opens the archive and keeps the list, and takes the buffer when there are
  pictures. **`AsyncLoad_CloseArchive` 0x244330** cancels a picture still being read, frees the buffer and closes
  the archive.
- **vf4 0x2443b0** starts the bars' animation: three bars at 100, 250 and 400. **vf3** starts fading in and says
  whether the fade is at 1. **vf5** steps the time by a 60th and the fade by a 30th up while fading in (else a 60th
  down), clamped to 0..1.
- **The draws:**
  - `LoadScreen_DrawProgressFlakes`: ten flakes from x 120 (the steps, ys and scales are tables at 0x3a4c30..);
    the first is left out when the word at 0x32ca50 is 3. The first `done` (at most 10) are gold, the rest a
    faded white.
  - `_DrawTipBox`: a black shadow, a white box 254 by 79 and the text right-aligned in black in the title font
    (1.27 by 1.34, then put back), all faded by the box's alpha.
  - `_DrawTicker`: the ticker's sheet tiled 700 wide along the bottom.
  - The spinner (0x244ee8): 45 · cos(angle) wide at y 420.5.
  - `_DrawBars`: a dark gold screen, then two bands at the top and two at the bottom (8 apart, the second at 0.4),
    each a black strip with the sheets' pieces at 120 and 520 and the cap at the right, then the animation's three
    white lines.
  - vf6 (the frame): it sets up the renderer (the viewport from the screen's size as unsigned words, a 640 by 480
    projection), draws the screen's own background and overlay, shows "loading" at step 4, and lays the fade over
    everything in the fade's colour.
- **`FE_GetSelectedCourse` 0x245598** gives the career's course, or the World Circuit's while its +0x3708 is 1.
- **`cFirstLoad`** (the first load) has a single picture (+0x40c, shape name at 0x3a4cc8), drawn full screen.

Checked in the runner (`tests/loadscreen.rs`): 300 cases of each of the 22 functions. Each case starts from the
ELF's data and the screens' tables, with a screen made by the port's `init` or random. The renderer's virtuals and
the screen's own are stand-ins through their tables, and so is every callee. A file's out words are written, the
fills done for real, the draws log their floats and the colour stand-in logs its four words.

66 of 72 mutants were caught. The six left are equivalent:
- `init`'s two words that are written again later;
- the fade's clamps at exactly 1 and 0;
- the flake count's cap at 10 (`< 11` against `< 10`);
- the unsigned conversion's low bit, which the PS2's truncating conversion never uses.

### The one-player track load (G9w2)

- **`c1PTrackLoad`** (table 0x3a5278) adds to the base:
  - an animation at +0x40c, which types the summary in line by line;
  - 28 sprite records at +0xb14, which the poll fills;
  - the event's summary as wide strings 0x3c apart from +0x6a0;
  - three pictures at +0xe28: the event mode's, the course's and the player's rider's. Each file name is made by
    `LoadScreen_Get{Mode,Track,Rider}PicName` ("ldmode…%s", "ldtrack…%s", "ldrider…%s" with ".ssh"), and the shape
    names are "ldmo", "ldtr", "ldri". Some indexes give no file: mode 6, and courses 9 and 10.
- **The poll** (0x245ec0) waits for the sound bank and the sprite set, then fills the 28 records with 197 stores:
  sheets, places, sizes and the texture's corners, the sheet words taken from the set's +0x15e8 / +0x15ec /
  +0x15f0, and the loaded flag at records + 0x310. The front end's loads, the load from quit, the trick tutorial's
  and the rider ranking's polls are the same code with the records at another place, so the port shares it
  (`poll_sprites`, `base` per screen).
- **`FE_FillEventSummary` 0x246700** writes the lines:
  - the mode's heading and line (two switches on the career's mode; nothing at all for modes 8 and 9);
  - the course's name, eight labels, and the course's difficulty, location, drop and length;
  - the player's character and level;
  - the trick book: "%d" of the next page, or 0xc9e once all 6 are done;
  - the medal line for the event.

  Then it adds the animation's bars, each as long as its text.
- **vf3..vf5:** the loading loop is played with the bars and stopped once the app's +8 reaches 8; the archive is
  closed if the fade isn't in.
- **`DrawBackground` 0x246fe0:**
  - the bars, the ticker, and each picture that is in drawn in a tip box (the mode's with its line, the course's,
    the rider's; faded by the animation);
  - the map (record +0xb30, 1.8 wide, 80 / its height high), with the course's marker placed by
    `Course_GetMapMarkerX` / `Y` (not for course 7);
  - the mode's badge in modes 4 and 5, then the flakes.
- **`DrawOverlay` 0x2473d8** uses two text styles on the stack (title and menu fonts):
  - the headings (0xe73 / 0xe74 in modes 4 and 5), or else the mode's heading typed in at 1.2;
  - the course's name in pink;
  - the labels right-aligned at 460, and their values typed in at 475 with the cursor on the animation's current
    line.

The same runner test, now 10,500 cases over the 35 functions. Its draw stand-ins also log the drawer's colour
(0x337fe8) as it stands when they draw. Without that, a wrong colour that a later draw overwrote went unseen. 58 of
59 mutants were caught; the one left is the constructor's second pass setting the records' first words to −1,
which the first pass already did.

### The two-player track load (G9w3)

- **`c2PTrackLoad`** (table 0x3a5218) has the animation at +0x410, 28 records at +0x9ec with its loaded flag at
  +0x40c, and two pictures at +0xcfc: each player's rider (shapes "ldr1", "ldr2").
- **Its poll** (0x247d58) has its own 197 stores; the last one sits in the delay slot of the first call after
  them. It then makes six copies of sprite records within the sprite set (0x1f9e90: 0xc7 ← 0x11, 0xc6 ← 0x12,
  0xc5 ← 0x13, 0xc4 ← 0x19, 0xc3 ← 0x1a, 0xc2 ← 0x1b).
- **`LoadScreen_FillEventText` 0x2485f8** writes four label ids (0xe81..0xe84) into a table at 0x3463f8 and reads
  them back for its lines:
  - the mode's heading and line as the one-player load's;
  - both players' characters, levels and trick book pages, and two more labels.

  For "all pages done" the format is passed an a2 the function never sets (a value left from earlier). The format
  takes no argument, so it doesn't matter, and the test does not log it.
- **`DrawBackground` 0x248b18** builds a `cFE3DProgressBar` on its stack, then uses it for every bar:
  - it fills the thing's fields, switches the table to the sprite's and refreshes its colour, then switches to the
    bar's (0x3b6cf8) and sets its size through its virtual (0x2ad828);
  - its three colours are written through a buffer, and both players' attributes fetched
    (`Career_GetPlayerAttributes`, four of four words each);
  - for each attribute, the bar is sized (its +0xc · 3.75 wide), given its value (+4 out of +0xc) and placed (the
    first player's to end at 215, the second's at 425; rows from 237, the bar's height + 10 apart), then drawn
    (vf11);
  - afterwards come the pictures in tip boxes and the flakes, and the bar is deleted.

  The port builds the same object in its scratch. The test's widget stand-ins log the whole object (0x190 bytes,
  its stack addresses as a token) at every call, so every field is checked as it stands at each step.
- **0x249250** draws a divider: two half-width gradient rects fading black out to both sides.
  **`DrawOverlay` 0x2493e0** draws the heading (also measured, though the width is never used), four dividers, the
  centred lines between them, and the two players' levels and pages. It draws nothing in modes 8 and 9.

The same runner test, now 13,800 cases over the 46 functions. 58 of 59 mutants were caught; the one left drops
the bar's `this` adjustment, which is 0 in the table.

### The front end loads (G9w4)

- **`c1PFELoad`** (table 0x3a51b8) and **`c2PFELoad`** (0x3a5158) are shown when going back to the front end after
  a race. Both start the same way as the other loading screens: the base, an animation at +0x40c, 28 sprite records
  (at +0xc04 for one player, +0xe5c for two), the fade's colours. The port shares that start and the matching end
  as `screen_new` and `screen_delete`.
- **`FE_FillRaceSummary1P` 0x24a188** fills the lines from the race just run:
  - the mode's heading (0xe7d free ride, 0xe7e race, show-off and time challenge, 0xe7f modes 4 and 5) and line;
  - the course's name and ten labels;
  - the player's character, level and trick book pages;
  - four counts from the rider's stats block (`Career_FindRider` 0x166598 picks the block, 0xbc each from the
    career's +0x3294) and three times, each a float · 100 made whole and passed to the race time format 0x243b88.

  It then adds the animation's bars, one per line.
- **`FE_FillRaceSummary2P` 0x24bbd0** fills the same lines for both players and adds no bars. Mode 7's line is
  0xe2c here, not the one-player screen's. The two-player screen also loads both riders' pictures ("ldr1",
  "ldr2") and draws them in tip boxes at 23 and 360.
- **The overlays:**
  - the one-player overlay types the values in at 475, with the cursor on the animation's current line;
  - the two-player overlay draws a centred column of labels with the players' values right-aligned at 210 and
    left-aligned at 430. It uses a centred shadowed print (0x19d770), and sets the drawer's colour directly
    instead of through the colour call.

The runner test now has 19,500 cases over 65 functions. 45 of 46 mutants were caught. The one left gives the
one-player destructor the two-player table; the base's destructor overwrites that word straight after, so the
mutant is equivalent.

### The load from quit, the trick tutorial's load and the rider ranking's (G9w5)

- **`cFELoadFromQuit`** (table 0x3a50f8) is the loading screen shown when quitting a race back to the menus. Its
  constructor takes a flag (kept at +0x1e64: the screen comes from the menu) and also builds a text box at +0x6dc.
  - **`LoadScreen_SetupTips` 0x24d1a0** picks one of 19 tips (+0x1b4c) at random, never tip 17. When the flag is
    clear and the player's level string id is 31 or 32, the pick is redrawn until it is one of tips 3, 4, 6 and 15.
    The tip's title and text are the labels 0xcff + 2·tip and 0xd00 + 2·tip, and the heading and both of them get
    one animation bar each.
  - Its overlay draws the tip's title in black at x = min(620 − its width, 320), then the text wrapped in the text
    box (320, 205, 280 by 320, at 1.07). The menu style's "typed in" flag is set only when the animation's +0xc
    is not 1.
- **`cTrickTutorialLoad`** (0x3a5098) has no pictures. Its overlay draws the pad's labels in one of two layouts
  picked by the career's controller setting (+0x168c), the two pad sprites (+0x928, +0x944) and their captions.
  Its loop is played and stopped with 1, as on the track loads.
- **`cRiderRankingLoad`** (0x3a5038) loads the player's rider ("ldr1"), shown in a tip box at (23, 150).
  - **`FE_FillCareerSummary` 0x24f520** walks the career's unlock list (0x3353f8 the count, 12-byte entries from
    0x335400) for the first entry of kind 4. That entry's index picks the item among the player's profile's items
    (0x332148, 0xf0 per profile, 0x14 each), giving the item's name and, by its kind (0..2), the line 0x81, 0xb9 or
    0x79. Without such an entry it takes item 0.
  - Its overlay builds a text box on its stack (sp+0x30, frame 0x15a0) and deletes it after drawing. The vf5 stops
    the loop only when the app's +8 is exactly 9 (the other screens stop it from 8 on).
- **The three polls** are the shared sprite poll, at the records +0x1b50, +0x6dc and +0xa60.
- Two small functions after them that the index doesn't list are ported too:
  - 0x24ff58 fills four static colour records at 0x3d2c88;
  - 0x250000 passes a vector's four floats to 0x14f5f0, with the vector's pointer left in a1.

The same runner test, now 28,200 cases over 94 functions. The text boxes' stand-ins log the box's first 0x70 bytes
when it is scaled and when it is drawn, so the colour copied into it and the fields cleared by hand are checked
for the box on the stack too.
All 55 mutants are caught. Five first survived and showed gaps in the test, now closed: the text styles' +4 is
logged when they are destroyed, the text box's init stand-in fills the box and its wrap sets +0x6c, and the
static records start non-zero.

## Pause menu draws and constructor (G9a3, `game/src/pausedraw.rs`)

- **`cPauseMenuOverlay::cPauseMenuOverlay` 0x1ab9f8(this, whole)** is built over the backdrop overlay it draws
  on (+0x42c).
  - When `whole` is set, it is the most-derived object: the backdrop is the overlay's own at +0x820, and it is
    built first.
  - Otherwise the compiler's constructor for a virtual base runs. It copies the backdrop's table (0x98 bytes from
    0x38f0c8) onto its own stack, moves the copy's six `this` adjustments (+8, +0x18, +0x68, +0x70, +0x78, +0x80)
    by where the backdrop really sits, and points the backdrop's +0x28 at the stack copy. The pointer is left
    dangling once the constructor returns. The port makes the copy in its scratch, and the test checks the copy's
    bytes against the game's stack.
  - Then: the second table (0x38f160), the trick info at +0x7f8, the three camera names (labels 0x1d8, 0x1da,
    0x1d9 into 0x338230), the pages cleared (0x3a8 bytes from +0x438) and built, +0x7e8 −1, +0x7f8 the app's +0x60,
    and three of the career's words (+0x3704, +0x3708, +0x370c) cleared. The main page's builder 0x1ac678 is G9a1's
    (`pausemenu`), called by address here.
- **`PauseMenu_DrawMainPage` 0x1ac7e0** draws the main page's list (`Overlay_DrawMenuList`, centred on y; its ninth and tenth
  arguments go on the stack: the pulse, and the items' enabled words, all 1) and the heading box.
  - With the trick book shown (+0x7f0) the list moves to y = the page's edge + 95, with no pointer sprite (0x74
    → −1) and no pulse.
  - It then draws the rider's name, the page ("%d" 0xe64: the next page, at most 6) and the trick's number
    (0xea7). The trick's name is smaller from 50 characters on.
  - The trick's info (`cTrickInfo` vf2 / `cGameTrickInfo` vf5) is drawn at (320, 303), with arrows when there are
    tricks before or after on the page, and 0xc93 once the trick is done.
- **`PauseMenu_DrawMusicList` 0x1ad198** draws nothing despite its name: it builds the options page's items.
  - The items are brightness (kind 8, top 50) and camera (9, top 3), the view for one player (0xb) and for two
    (0xc), the song (0xd, top: the songs enabled) and five volumes (0xe, 0x10, 0x13, 0xf, 0x15, top 50).
  - It also measures the four view names and every enabled song, then throws the widths away.
  - Finally it copies the page's 16 bytes of colour (0x38e380).
- **`PauseMenu_DrawOptionsPage` 0x1ad628** lays the rows out centred on 240, the menu font's height · 0.8 + 6
  apart, starting from `240 − height · count · 0.5`. In each row:
  - the label and ":" are right-aligned at 264 and 270, gold when picked;
  - the value is drawn at 305: the camera's name (kind 9), the song (0xd), or the view's name by its value (kinds
    0xb, 0xc: 0 → 0x14a, 1 → 0x14d, 2 → 0x14c, 3 → 0x14e, else 0x14d);
  - a slider (+0x10 clear) is the bar sprite 0x56 with 0x57 tiled over it by value / top (`div.s`);
  - the picked row's arrows sit at 282 and past the value's width.

  Every colour is scaled by the backdrop's alpha (+0x70) except the slider's white.

The runner test (tests/pausedraw.rs) has 12,000 cases. Its menu-list stand-in reads the two stack arguments and
logs both arrays.
All 63 mutants are caught. Two first survived because the ELF loads the career's three words and the camera
names as zero; the test now makes them random.

## The debug menu's framework (G9a4a, `game/src/debugmenu.rs`)

The leftover developer menu (`cDebugMenu`) is built on three small classes.
- **`cMenuItem`** (table 0x389e80) is the item base: +0 the label, +4 the menu, +8 flags (1 shown, 8 not
  selectable), +0xc the table.
  - Its virtuals: +0x10 the key's handler, +0x18 whether it can be picked, +0x20 its size, +0x28 its draw with a
    box, +0x30 picked, +0x38 left.
  - The size virtual returns a 16-byte box (x, y, w, h) through a hidden first pointer, so it takes (out, this).
  - The base's key handler turns the frame's key into the menu's answer: back 8 → 1, escape 0x1b → 2, down 0x5000
    → 3, up 0x4800 → 4.
  - The index has the bit-1 test as `MenuItem_IsDisabled`, but it is the "shown" bit. Two setters the index
    doesn't list, 0x188ae8 and 0x188b20, set and clear bits 1 and 8.
- **`cMenu`** (0x389e40) has +4 its manager, +8 the count, +0xc the item picked, +0x10 the first shown, and up to
  64 items from +0x14.
  - Moving the selection goes round the list with a signed `div` (it traps on an empty menu) to the next item
    that can be picked. The old item is left and the new one picked, told how: 0 by the next key, 1 by the
    previous, 2 set.
  - `AddItem` keeps the selection on its item only when the menu has a manager and was not empty. `RemoveItem`
    keeps it inside the list, and skips on when the removed item was the picked one. 0x189390, also unlisted,
    clears the count and the selection.
  - **vf4 0x188e88** lays the menu out and draws it:
    - Every shown item is measured into a box on the stack; a hidden one gets height and width 0.
    - The list is centred across 640 by its widest item. When the total height is under 421 it is also centred
      down 480, and the first shown goes back to 0.
    - Otherwise it starts at y = 30: the first shown is pulled back to the selection if it is past it, then
      moved on while the items from it to the selection add up to 421 or more.
    - From the first shown, each item that still fits above 451 is drawn (the picked one always is), at the
      list's x, the running y and the widest item's width.
    - The arrows show when there is more above (the first shown is not 0) or below (the loop stopped early).
- **`cMenuManager`** holds a stack of menus (+0 the depth, +4.. the menus), the pad (+0x2c), the frame's key
  (+0x30) and whether it is active (+0x34).
  - `Update` reads the key from the pressed device (`Input_FindPressedDevice` on the app's +0x24). In the app's
    mode 4, 13 and 100 become back (8) and 99 becomes select (13); otherwise 100 is select and 98 back. A top
    menu answering 1 is popped; one answering 2 pops them all.
  - `Push` leaves the old top menu and enters the new one; `Pop` leaves the top and enters the one under it.
    The manager is inactive once none is left.
  - `Draw` doesn't check the depth: with no menu it takes the depth word itself as one.
  - 0x189590 (the destructor) and 0x1897a8 (the pad's getter) are not in the index.
- The destructors pass the free (`Mem_Delete` 0x114f80) the flags masked to bit 1, not the flags.

The runner test (tests/debugmenu.rs) has 22,000 cases over the 44 functions, with stand-in tables for the items
and menus. 60 of 64 mutants are caught. The other 4 are equivalent: the hidden item's zero width (never read);
pulling the first shown back when it equals the selection; one extra step of `AddItem`'s shift (overwritten
straight after); and mode 4's select test on the key instead of the stored word (they differ only when the key
became 8).

## The debug menu's item kinds (G9a4b, `game/src/dmitems.rs`)

Every kind keeps the `cMenuItem` base and adds fields from +0x10. Labels go through `MenuItem_MeasureLabel` and
`MenuItem_DrawLabel` (G9a4c) as a title with a value beside it, or as a lone label. A menu's +0x18 virtual runs
a command (item, argument) and its +0x20 hears that an item's value changed.
- **bool** (table 0x389e00): +0x10 the title, +0x14 / +0x18 the words for true and false, +0x1c the bool.
  - Select flips it; with both words, left (0x4b00) and right (0x4d00) flip it too.
  - Its measure is the wider of the title beside each word.
- **enum** (0x389dc0): +0x14 the count, +0x18 the list (8 bytes each: name, value), +0x1c the value.
  - Select runs the menu's command (argument 0). Left and right step round the list; a value not in the list
    goes to the first or the last entry. The steps are a signed `div`, which traps on an empty list.
  - It also checks up and down, but the base turns those keys into answers first, so they never get there.
  - 0x189d38 (not indexed) sets the list and counts it to the first entry without a name.
- **sub-menu**, **pop**, **command**, **null**, **space**:
  - select pushes the item's menu (+0x14), or pops;
  - a command runs the menu's command with its argument (+0x18), showing a value (+0x14) when it has one;
  - the null and space items can't be picked; the space measures as its height only.
- **int** (0x389c40), **float** (0x389c00), **angle** (0x389bc0): held left or right (the pad's buttons 8 and 4)
  step the value inside its range, faster the longer they are held. The menu is told, and a second value
  (+0x2c / +0x30) is kept equal.
  - The int steps by 1 + held² / 288 · 0.1, taking the whole part and keeping the fraction (+0x28) for later.
  - The float moves by the range · 0.00005, times (held² · 50 / 60 · 50 / 60 / 200) once that reaches 1. The
    angle is the same code, drawn in degrees (· 57.2958). The float is drawn as value · scale (+0x20) + offset
    (+0x24).
  - Both formats get their float through 0x2f7aa8, which makes it a double for the variable arguments.
- **string** (0x389b80) is a name editor (+0x14 the buffer, +0x18 its size, +0x1c the cursor).
  - Left and right move the cursor; right past the end appends '0' while there is room.
  - 'a' inserts '0' at the cursor and 'A' deletes there.
  - 'c' and select step the character up and down through the name characters (letters, digits, '_'), round the
    signed byte.
- **colour** (0x389b40) edits one channel (+0x14) of a float colour (+0x10). Held left or right moves it by
  0.0004 · max(1, held² / 288), inside 0..1.
- **expand** (0x389ab0) holds a whole menu at +0x14 (its own table 0x389b00, its owner at +0x12c), with a header
  item (+0x10) first.
  - Its key handler passes the key to the inner menu's picked item. Moving past the inner menu's ends puts the
    selection back and answers down or up to the outer menu.
  - When the item is picked in the outer menu, it measures and draws all its inner items, the others 20 in from
    the header; otherwise just the header. Its draw moves the given box's y down past each item it draws.
  - The inner menu's command and change go to the owner's +0x40 / +0x48, which pass them on to the outer menu.
- **RGB** (0x389a20) is an expand item over three colour items (red, green, blue) under a title with a swatch
  (`cRGBTitleMenuItem`, 0x389a70). After each key it copies the colour to a kept one (+0x5ac).
- **The error menu** (0x3899e0) is a menu of a message (a null item), a gap (a space of 10, or 30 with a message)
  and a pop item.

The runner test (tests/dmitems.rs) has 23,700 cases over the 75 functions, with each kind's fields set up as it
reads them. 72 of 77 mutants are caught. The other 5 are equivalent:
- the enum's up and down checks (unreachable);
- the float's faster rate from exactly 1 (the same product);
- one extra step of the string's insert shift (overwritten straight after);
- the expand item's header added at 0 instead of the end of an empty menu (twice).

## The debug menu's drawing (G9a4c, `game/src/dmdraw.rs`)

- **The frame:** `DebugMenu_BeginDraw` keeps the renderer's texture in the menu's +0 (its first word, otherwise
  unused) and sets texture 12 and the blend (2, 2; 0.08). `EndDraw` puts both back (1, 2; 0.08).
- **`DebugMenu_DrawBox`** fills the box in its colour at alpha 0.8 (texture 11, untextured, mode 3).
  - It then draws the top and left edges 1.4 times lighter (clamped to 0..1) and the bottom and right edges 0.7
    times as dark, each half a pixel in.
  - The arrows are a box in the picked colour with a chevron drawn twice, at 4 and 3..5 pixels in.
- **`MenuItem_MeasureLabel`**(out, item, title, label, value) adds up the font's widths (the font is the manager's
  +0x38).
  - With all three given: the label's width, plus twice the wider of the title and the value, plus 40.
  - Otherwise: the given widths, plus 20 when the title has something beside it (or, without a title, when both
    a label and a value are given).
  - The result is at least 200, then + 20 wide, and the font's height + 4 high.
- **`DrawItemBox` / `DrawLabel`** draw the title 10 in from the left, the label centred and the value 10 in from the
  right, all one pixel down. The colours (three floats each, filled by the start code 0x18de60) are:
  - disabled (flag 8): box 0x3cde68, text 0x3cde38;
  - picked: box 0x3cde78, text 0x3cde48;
  - the others: box 0x3cde88, text 0x3cde58.
- **The string item's draw** underlines the cursor when picked. To get the line's ends it measures the string cut
  at the cursor, then cut after the cursor's character, writing a 0 into the buffer and putting the byte back each
  time. The wider of the first width + 2 and the second gives the right end.
- **The colour item's draw** shows the channel's name, its value · 255.99 as "%d" and a gradient bar 182 by 11,
  with a marker over it at the value.
  - The bar runs from the colour with the channel at 0 to the colour with it at 1. Its two four-float colours start
    with 1, written separately from the three channels.
  - Its value's text goes into the stack slot that had just held the box colour, so the test's format stand-in,
    which writes nothing, leaves those bytes for the measure to read. The port lays out its scratch at the game's
    frame offsets, so it reads the same bytes.
- **The RGB title** measures at least 27 high and 53 wider, and draws a 23 by 23 swatch of its colour 33 in from the
  right.
- **The start code 0x18de60** builds the debug menu's global manager (0x337e08) and fills the six colours; 0x18de50
  destroys the manager at exit.
- **The kinds' destructors** are the item's, tail-called. The error, RGB and expand items first destroy what they
  hold, passing flags 2 (no free).
- **Unlisted functions:** `Color_SetRGBV`'s twin 0x18df70, eighteen setters of one word (+0x10, +0x14, +0x1c)
  or two floats, an enum's list setter 0x18e650, and five accessors (an item's or a menu's font, +4, the depth).

The runner test (tests/dmdraw.rs) has 19,500 cases over the 65 functions. The font's stand-ins log the string
they are given, up to its end, and the gradient's stand-in logs its four colours. 68 of 71 mutants are caught;
the 3 left are equivalent: the two "at least" floors at their own value, and the expand item's menu destroyed with
flags 0 instead of 2 (neither frees).

## The debug menus (G9a4d, `game/src/dmmenus.rs`)

The developer menu's pages are built from the item kinds (G9a4b). Each item's +0 is used as its id (the pages'
handlers switch on it), and each page keeps the values its items edit from +0x118.
- **`cDebugMenu`** (table 0x389070) holds, in order:
  - a title;
  - the version line, formatted into 0x337de8 at start;
  - a gap and a back (pop) item;
  - three commands: 2 (only when the race has +0x40, and not in the career's mode 6), 1, and 0;
  - sub items for the render, game and sound pages, which it holds at +0x3d8, +0x218 and +0x1968.

  Its command handler (vf2) does: 0 saves the career (+0x3704 1, `0x161a98`, +0x3260 2, the race's +0x54 1,
  `0x161840`); 1 quits the race (0x180a10); 2 closes the menu (the global manager inactive) and shows the race's
  overlay 9. Entering it resumes and pauses the sound.
  Leaving it as the last menu, while the race isn't ending (+0x54), calls `Debug_Printf2` (empty in this build:
  it only stores its arguments) and stops the sound. Either way it shows overlay 9 when the race's +0x40 object
  has a +0x4c.
- **`cRenderOptionsMenu`** (0x389130): the bezier page and the resolutions behind sub items, the frame-rate bool
  (the race's +0x74), the split-screen bool (only with two players; the race's +0x29c, then its mode set again),
  the gamma float (0.5..2, the renderer's +0x67c, set through the app's brightness) and the filter enum (the
  renderer's +0x130's word, set through its +0x120).
- **`cResolutionMenu`** (0x389170) makes 128 command items. It shows one per renderer mode (the renderer's +0x50
  the count, +0x48 a mode's name), with the mode as the item's id. Entering it picks the item of the current mode
  (+0x40); choosing sets the mode (+0x28), the race's mode and the input again.
- **`cBezierOptionsMenu`** (0x3891b0): three bools and two floats (1000..3000, 5000..15000), read from and set on the
  renderer's +0x20 object's virtuals (+0x60/+0x48, +0x68/+0x50, +0x70/+0x58, +0x30/+0x20, +0x38/+0x28).
- **`cGameOptionsMenu`** (0x3890f0): a bool (0x17f078 on the race, set to the race's +0x48), a riders int (1..the
  race's riders) and a restart command. The restart resets every rider (0x11da40), then calls the race's +0xa0
  virtual (its table at +8) with the count − 1.
- **`cSoundVolumeMenu`** (0x3890b0): seven volume ints (0..11), three enums and the song enum.
  - The song enum's list (+0x144) is built from the songs that are on.
  - The volumes are read from the sound system as ints of its floats (channels 0, 1, 4, 2).
  - A changed volume is set as int / 11 (clamped to 0..1); the preset sets the system's and reads the volumes back;
    the song picks and starts the music.
- **The destructors** take the items and pages apart, last first. The resolution page's 128 commands go through
  their tables' destructors (+8, flags 0).
- **0x188310** fills four static colour records at 0x3cdde8, as the loading screens' 0x24ff58 does at 0x3d2c88.

The runner test (tests/dmmenus.rs) has 8,400 cases over the 28 functions, with the whole debug menu's code
(G9a4a..c) real on both sides. 70 of 71 mutants are caught; the one left gives the riders int a minimum of 0, which its range sets to 1 straight after.

## The pause overlay's own virtuals (G9a5, `game/src/pausedraw.rs`)

The index named these four `cCountdownOverlay::vf31..34`. They sit in the pause overlay's table and act on its
fields, so they are really `cPauseMenuOverlay`'s.

- **Its pages** are kept as up to two pointers at +0x7e0, with the page shown at +0x7e8.
  - Each page has three of GCC 2's member-function pointers: +0x110 the update, +0x118 the draw, +0x120 the text.
  - Each pointer is a `this` shift (+0), an index (+2) and then a word. The index is 0 for none. Above 0 it is a
    virtual: the word's low half is the table's offset from `this`, the entry is at index · 8 − 8, and the entry
    holds its own shift and the function. Below 0, the word is the function.
- **vf34 0x1abc00 (open):**
  - resets the backdrop and resumes the pause (0x1ad8a8 with 0);
  - starts the frame's three animations (0.88, then two slides);
  - sets the trick book's page from the career's first trick not done (`0x1a9f30`; when there is none, or it is 30
    or more, page 5's first);
  - reads the options page's values by item kind: the brightness · 50; the race's two modes (kinds 9 and 10); the
    two players' cameras (the camera index − 0xb); the song (the music's +0x36e8); the five volumes · 50 (channels 0,
    1, 4, 2 and 3 for kinds 0xe, 0xf, 0x13, 0x10 and 0x15). The other kinds get 0;
  - clears the card operation (+0x430) and the trick book's flag (+0x7f0), and sets +0x7ec.
- **vf31 0x1abe70 (draw):**
  - with a dialog up (the backdrop's +0x328), draws only the dialog and returns;
  - while a card operation is saving (+0x430 is 2), draws nothing;
  - otherwise sizes the backdrop's frame (item 0xe) by the page's items: the menu font's height · 0.92 + 4 each,
    from 100 (95 on the main page), and 120 more with the trick book;
  - on texture 16, calls the page's draw (+0x118);
  - once the frame is fully open (+0x34 equals +0x2c), draws the help line 0x1d7 at 600, (h + 480) / 2 − 40.
- **vf32 0x1ac080 (text):** does nothing while a dialog is up. Otherwise it draws the card operation while one
  saves (+0x430 is 2), or else calls the page's text (+0x120).
- **vf33 0x1ac130 (update):** updates the backdrop first.
  - **A card operation:** runs until it is done; a good result saves the career (+0x3704 1, `0x161a98`, +0x3260
    2, `0x161840`). Then it is destroyed and +0x430 cleared.
  - **A dialog's answer:**
    - 3, while not in the save question, does nothing;
    - 2 (no) clears +0x430;
    - 1 (yes) to the save question starts the save (+0x434 0x1c4, the operation begun at the career's +0x27d4,
      saving with 1).
    - Otherwise the race is left. With +0x7ec the career is saved. Without it, the four players' flags are
      cleared, and then the lesson ends (in a lesson) or the race quits. The page is popped.
  - **No dialog:** the page's update (+0x110) runs.

The test now has 27,000 cases. The pages and their member pointers are random (none, plain, or virtual through a
table at +0x7c0, with random shifts). 47 of 48 mutants are caught. The one left saves the career before setting
+0x3260 to 2; that order can't be seen while the save's own code is a stand-in.

## G9x1: the widget bases and their lists (`game/src/fe_wgroup.rs`)

The code that every front end widget builds on, 0x2aab58..0x2adf48 and 0x2b3950.

- **Operations over the children.** A widget group (`FE3DWGroup_*`) keeps its children in a list at +0x110; the
  plain group (`cFE3DGroup`) keeps them at +0xa0. To do something to every child, the group builds an operation on
  the stack: its table at +0, one argument at +4, or a vector at +0x10. It walks the list with it (`cList` vf1
  0x2ac220: each node's +0x18 with the operation, stopping at the first that answers non-zero), then destroys it
  (the operation's destructor with 2). Each operation's apply (`ListFE3D*Op_Apply`) calls one virtual of the
  child: on input +0xe8 (the answer is the child when it took the input, else 0), format +0x140, update activity
  +0x48, reset +0x40, skip +0x38, offset +0x70 (with the vector), update +0x68, render text +0x60, render +0x58
  (with the matrix).
- **The group's own steps.** Most do the children's step, then the widget's own: the activity update
  (`0x2ad380`), the reset (`0x2ad2c0`), the skip (the thing's vf6), and so on. Update, render and render text do
  nothing while the widget is hidden (+0x10 for update, +0x14 for the others).
  - Render and render text raise the renderer's state by the widget's +0x70 around the children (the renderer's
    +0x1d8 read, +0x1d0 set) and put it back after.
  - The plain group's render copies the identity matrix (0x346440, 0x40 bytes) to the stack, multiplies it by the
    matrix it is given (0x2abc18), renders the children with it, and destroys it (0x270218 with 2).
- **Offsets.** The children are moved first; then the widget's own position (+0x80) gets the vector's x, y and z
  (its w is set to 0 and not added).
- **The thing.** `cFE3DThing` vf6 (when active and not looping, the tween's frame goes to its end, +0x24 from
  +0x28), vf7 (running, frame 0), vf12 (its +0x48), vf13 (the position moved by a vector), vf9 (the colour, +0x60,
  from a vector), the one-word setters (+0x10, +0x14) and the virtuals that answer 0 or 1 or do nothing.
- **Destructors.** Each sets its own table and then calls its base's. The plain group deletes its children
  (0x2ac178), destroys its list (`cList::~cList`: its table at +0x20, its two nodes, freed on flag 1), then the
  thing's and the node's (the node unlinks itself, 0x2abf38, and is freed on flag 1).
- **The plain group's constructor** writes the thing's words: the links, the flags, white, the position (0, 0, 0,
  1), the size 0. Then it sets up its list (0x2abfb0) and its table.
- **Placing and sliding in.** `FE3DWidget_Place` shows the widget (+0xf8 with 1), puts it at (x, y) (+0x138), sets
  its anchor (+0x110) and clears sliding (+0x114). `FE3DWidget_SlideIn` places it, sets its colour to 0x3d36f0
  (+0x50), and runs a colour tween (track 5) from 0x3d3700 to 0x3d36f0 over the frames given. Then it hides the
  widget (+0x20 with 0). The two `SlideInDir` forms slide in at anchors 3 and 4 over 15 frames from (18, 18).
  `FE3DWidget_SetValue` stores a float at +0x160.

The test has 48,800 cases (61 functions, 800 each). The list's walk and the row's own helpers run for real. The
lists are chains of up to five nodes in the arena, so some 15,000 nodes are given an operation. The identity matrix
is in memory the game clears at start-up, so the test fills it with random words; that way all 16 words are seen
being copied. The group's table is logged as its children are deleted.

60 of 63 mutants are caught. The other three are equivalent:
- the stores of 0 to the w of the offset's vector (on the stack, and never read);
- the store of 0 to the w of the thing's vf13 vector (the same);
- the thing destructor's table, which the node's destructor overwrites at once.

## G9x2: the text widgets (`game/src/fe_wtext.rs`)

The front end's text widgets other than the plain single-line text (G9b2c's `fe_widget`), 0x2b0b90..0x2b4f18.

- **The wrapped text (`cFE3DWText`)** keeps a text box at +0x12c.
  - Its font is at +0x110. +0x11c is the wrap mode it passes to the box. +0x120 says whether the text came from a
    string id, and +0x124 holds that id (or -1).
  - `FE3DTextBox_Init` builds the widget, sets the font, the table and the box, and then sizes itself to (640, 50)
    through its own vf38.
  - Setting a text clears the box first:
    - `SetWideText`: the box's font and rectangle, then the wide text added;
    - `SetStringId`: the same with the id's string, and the id kept;
    - `SetText`: the narrow text added (`0x196128`).
  - The box's rectangle always comes from the widget's position (+0x80) and size (+0x100).
  - vf13 moves the position by a vector's x, y and z; vf38 takes a new size and rewraps; vf39 rewraps.
  - vf9 sets the colour (+0x60) and copies it to the box's (+0x168).
  - vf37 answers the box's widest line and its height. `SetScale` sets the box's line height. `ApplyScale` sets the
    box's scale, with the box's +0x14 as the second value.
- **The buffered text (`cFE3DWBufferedSingleLineText`)** copies the string into its own buffer at +0x130 (wide or
  narrow) and points the single-line text at it. Setting an id empties the buffer first.
- **The two-coloured text (`cFE3DWColouredText`)** draws its string's first word in a second colour (+0x130,
  set by 0x2b4858).
  - The split (+0x140, found by 0x2b1a68 after each layout) is the index of the first space, or -1.
  - When the split is past the start, the draw copies the first word into a stack buffer (with a limit, then ends
    it) and draws it in the second colour. It moves x on by the word's width and goes back to the widget's colour
    (+0x60). It then draws the rest after the space.
  - With no split, the second colour is set and the whole string is drawn in it. A string that starts with a space
    (a split of 0) is drawn from its second character, also in the second colour.
  - Its size is the single-line text's, less the narrow width of the string at 0x3b67d0 when split.
- **The scrolling text (`cFE3DScrollingText`)**, the credits' roll, keeps up to 30 line ids at +0x110 (-1 for
  none). The other fields are listed in the module's header.
  - A line's height is the font's height times the scale, plus the gap. vf39 and vf38 fit as many lines as the
    height allows, truncated.
  - Each update moves the roll by the speed (+ when rolling up, − when rolling down). Once it passes a whole line
    height, the slots are shifted (up or down), the roll is brought back by a line, and `Advance` runs.
    - `Advance`: while there is room and the next id is within the first and last ids, that id is added and the
      next id moves on (+1 rolling up, −1 rolling down). When no lines are left, the next id goes back to the
      first id (the last id when rolling down) and the slots are cleared.
    - `AddLine` scans from the last slot forward, past the end of the slots, until it finds an empty one, and only
      stops early when the index turns negative. After a shift up the last slot is always empty, so the scan stops
      at once.
  - Pad up and down change the speed by 0.1 (`SetSpeed` keeps it between 0.1 and 0.35 of the font's height). Up
    speeds a roll up and slows a roll down; down does the opposite.
  - The draw takes each slot's string by id:
    - a string starting with the character at 0x337e50 is skipped (but still takes its line);
    - one starting with the character at 0x337e48 is drawn without that character in the chosen colour (+0xac)
      with border flag +0x1b8;
    - the others are drawn in the enabled colour (+0xbc) with flag +0x1b4.
  - The colour's first word is replaced by the line's fade: the first line fades by the roll over a line height,
    the last by the roll over the font's height. The fade is clamped to 0..1 and flipped when rolling down.
  - Lines are always drawn wide, aligned against the size by the font's width (2 centred, 1 to the right).

Two of the coloured text's unnamed helpers (`FUN_002b1a68`, the split, and `FUN_002b4858`, the second colour) are
ported here, not in G9x4.

The test has 72,000 cases (48 functions, 1,500 each). The copies with a limit fill their buffer in the test, so
where the end of the first word goes is seen. The strings are random wide strings of letters and spaces, starting
with the hide or mark character or not. The scrolling text's slots, ids, roll, speed and alignment are random too.

91 of 95 mutants are caught. The other four are equivalent:
- `SetStringId`'s store of the id, which `LoadStringId` has just made;
- the split's check for an empty string, which falls through to the same -1;
- a fade of exactly 1 clamped to 1;
- the scrolling text destructor's table, which the widget's destructor overwrites at once.

## G9x3a: the entries, list box, name entry and button (`game/src/fe_wentry.rs`)

- **The entry (`cFE3DWEntry`)** is a widget that holds two children, +0x110 and an optional +0x114 (an icon and
  its label, or a label and its value).
  - Most of its virtuals do their own part and then call the same virtual on each child: the colours (vf9, vf32..35),
    the active and shown words (vf3, vf4), the chosen and enabled flags (vf29, vf31), the move (vf13), the text draw
    (vf11) and the update (vf12). Input goes to the second child only (vf28).
  - Each child sits in a box: +0x120 for the first and +0x140 for the second (0 means the child's own size). The
    alignment is +0x11c for the first and +0x130 for the second: bit 1 keeps x where it is, bit 0 puts the child
    at the box's right edge, and neither centres it; bits 2 and 3 do the same for y. The gap after the first box is
    +0x118.
  - The layout (vf39) formats and sizes the first child and moves it into its box at the entry's position. The
    second child's box starts after the first box (or after the first child's width, by the alignment) and the gap.
  - The size (vf37) and the bounds (vf36) follow the same rules. The bounds start from the position and the first
    box, widen to the first child's bounds, then push the high corner out to the second box and widen to the second
    child's.
- **`FE3DButton_Layout`** sets up an entry as a button. The icon gets the sprite's size (at most 17 each way) and
  white; the label gets its text, scale 0.7, no border, and colour (1, 0, 0, 0). Both boxes are (0, the icon's
  height); the icon goes to the left and the label is centred, with a gap of 4. Then the entry is laid out.
- **`FE3D_GetSpriteRect`** is the sprite set's rectangle (`0x1f9e50` on gApp+0x728); the widget it is given is not
  used.
- **The list box (`cFE3DWListBox`)** shows one widget of its list (+0x114) at a time (+0x110).
  - Left and right move to the node before or after, wrapping to the list's last or first at the end. The old
    widget is told it is left (+0xf0 with 0) and the new one that it is chosen (+0xf0 with 1); then the owner is
    told (+0xc0).
  - The draw moves the shown widget to the list box's position and gives it the list box's colour before its text.
- **The name entry (`cFE3DWNameEntry`)** is the on-screen keyboard: 42 characters (the table at 0x3b7f58) in rows of
  +0x114, each cell 0x348464 by 0x348468 at the scale +0x118.
  - The chosen character is drawn in the chosen colour (with the cursor widget, +0x11c, put over it), the others in
    the enabled colour.
  - 'e' is the end key: string 0x112, printed at the scale times 0x34846c. 'd' is the delete key, sprite 0x3b.
  - Input is only taken from the front end's player (the input's +0x40 must equal the front end's +5 less one).
    Left and right wrap within the row. Up from the top row goes to the same column of the last row, or to the last
    character; down from the last row goes to the top. Accept and back tell the owner (+0xd8, +0xb0).
  - Each key plays its sound (+0x120 move, +0x121 accept, +0x122 back) on the front end's sound player.
  - `NameEntry_SetChar` picks the last of the 42 that matches.

The test has 58,500 cases (39 functions, 1,500 each). The children's bounds and sizes are written by their
stand-ins. The 42 characters (with 'e', 'd' and spaces among them), the cell sizes, the end key's scale, the rows'
length (1 to 42) and the front end's player are random.

The name entry calls its owner without checking for none; the button needs both children.

101 of 106 mutants are caught. The other five are equivalent:
- raising the first child's size to a box of 0 writes the same 0;
- `(mode >> 2) & 2` tests the same bit as `(mode >> 3) & 1`;
- the right move's `max(0)` can't bite: the index there is never negative;
- `r < last` against `r <= last` in the up move gives the same character;
- the down move's rows check against 1 instead of 2 lands on the same index.

## G9x3b: the menu list (`game/src/fe_wlist.rs`)

`cFE3DWList` is a menu (`cFE3DMenu`, whose own code is G9b's and is a stand-in here) that shows its items in one
of four ways, set by +0x190:
- **1, plain:** everything is left to the menu.
- **2, one item at a time:** the chosen item sits in the box (+0x170, +0x174) at the list's position, aligned by
  +0x154. Bit 0 puts it at the right or bottom edge, bit 1 keeps it where it is, and neither centres it. Bits 2 and
  3 do the same for y. A list running across (+0x150 0) always centres x; one running down (+0x150 1) always
  centres y.
- **3, in a frame:** a frame widget (+0x194) is moved over the chosen item (0x2b5958). The frame's offset is the
  item's position less the frame's, less half the difference of their sizes.
- **4, a window:** +0x1a0 items are shown, from +0x198 to +0x19c.
  - Scrolling moves both ends on together: forward when stepping on with the pad's way as it is, or back with it reversed (+0x13c). The
    end that comes into view is aligned (0x26df58 for the last, 0x26dd60 for the first), and every item is moved
    back by the frame's offset (an offset operation over the list).
  - The layout sets the window's count: all the items when unset, or when it is more than there are. It turns the
    list so the first item sits in the middle (half the window from the back moved to the front), takes the first
    and last shown, lets the menu lay out, then turns the list back.
  - The size adds up the shown items along the list, each with the box's size that way when set, plus the gap
    (+0x15c), with the gap taken off the end; across, it takes the box or the largest item. Hidden items (+0x14
    clear) don't count.

Other things to know:
- `cFE3DWList` vf44 makes the two arrows: icons allocated (0x120 bytes, filled with 0xdeadc0de), owned by the list's
  owner, 10 by 10, slid in from anchors 3 and 4. It also sets mode 2, the pad's way not reversed and a gap of 15.
- The render (vf10) runs only while +0x14 is *clear*. For modes other than 1, 2 and 4, render and render text put
  the renderer's state back and raise it again (render) or run the menu's vf11 (render text). Their arrows and frame
  are drawn only while the list is chosen (+0xa0).
- vf43 chooses the k-th item: the old one is told it is left, and the new one chosen while the list is. Then it
  scrolls: from the first item in mode 2; in the others (but 1), only when the choice changed.
- `StaticInit_FE3DWList` writes three colours and a block of words at 0x3d3760..0x3d37ac.
- The PS2's FPU has no NaN: a word with an exponent of all ones compares as a huge number. The list's comparisons
  go through `ee()` so that random words in the test compare as the game's do.

The test has 47,500 cases (19 functions, 2,500 each). The menu's code, the list helpers and the heap are stand-ins.
The list's walk, the placing and sliding in and the vector scale run for real. The icon's own virtuals, reached
through its real table, are stubbed by the addresses read from the game's data. The frame's offset reads the frame
and the chosen item without checking for none, as does the scroll that calls it, so those cases always have both.

All 97 mutants are caught. To catch them, the menu's own steps and layout sometimes choose another item in their
stand-ins, and the boxes are sometimes a denormal (0 to the EE).

**Follow-up (G9x2, G9x3a):** the text widgets' and the entries' float comparisons now go through `ee()` too, as
the menu list's do (G9x3b): the EE has no NaN and takes denormals as 0. Both tests pass unchanged.

## G9x4: the bars and pictures (`game/src/fe_wbar.rs`)

- **The progress bar (`cFE3DProgressBar`)** holds a value (+0x160) between a bottom (+0x154) and a top (+0x150). The
  value may also be held between a lowest (+0x15c) and a highest (+0x158) setting, where -9999 means none.
  - `StepUp` / `StepDown` move the value by +0x164 and hold it to the limit, or else to the top or bottom. They
    update the arrows and answer the value in f0.
  - The pad steps the value up the way the bar fills (+0x168: 0 right, 1 left, 2 up, 3 down) and down the other
    way. Each step restarts the arrow on that side and tells the owner (+0xc0). Accept answers on its own.
  - `UpdateFill` shows each arrow (+0x188 towards the bottom, +0x18c towards the top) unless the value has reached
    its end. It puts the arrow 5 beyond that end of the bar, centred across.
  - The draw tiles the background (+0x16c), flipped for a bar that fills left or up. It puts the end sprite
    (+0x170, none below 0) at the far end, 6 wide. One state higher, it draws the cap up to the highest setting in
    the cap colour (+0x140). Two states higher, it draws the fill, shaded from the low colour (+0x110) to the colour
    at the value: the high colour +0x120 scaled by the value, the alpha kept. For a way of filling outside 0..3 the
    draw stops after raising the renderer's state, and leaves it raised.
- **The icons.** The icon (`cFE3DIcon`) draws its sprite (+0x110, flip +0x114) at its position and size in its
  colour. The tiled icon (`cFE3DTiledIcon`) tiles it by +0x120 and +0x124. Its constructor sizes it to 10 by 10
  through the icon's +0x138, which it inherits.
- **The second button layout (0x2ae230).** The help buttons' one. It differs from `FE3DButton_Layout`: the children
  are the other way round (icon +0x114, label +0x110), there is no first gap, and the boxes are one higher than the
  icon.
- **Small ones:**
  - the video and options states' destructors and virtuals, `FE3DState_SetName` (+0x54), `FE3D_AddWidget`, a group's
    first widget;
  - setters that pass their floats through the stack (+0x90, the world position, the colour, the size with one side
    kept);
  - two operation applies (skip, move);
  - the vector scale 0x2adcb8, the slide-in from anchor 3 that then sets +0x114 16;
  - the list box's add, and constructors that build only the widget (list box, single-line text, icon, bar).

The test has 61,500 cases (41 functions, 1,500 each). Values are sometimes below the -9999 mark, so that it is seen
to mean "none". The stack and the port's scratch are filled alike (0xa5), so a word left unwritten is seen.

87 of 89 mutants are caught. The other two are equivalent:
- the tiled icon's destructor's table, which the icon's overwrites at once;
- the tiled constructor's size through the tiled table instead of the icon's (both have 0x2ad828 at +0x138).

## G9y1: the menu (`game/src/fe_menu.rs`)

`cFE3DMenu` is the widget that holds a list of items (+0x118) the pad moves between. G9x3b's list builds on it.
Its fields are listed in the module's header.
- **Choosing.** An item can be chosen while shown (+0x14) and enabled (+0xa4 clear).
  - `FE3DMenu_NextSelectable` / `PrevSelectable` walk the items (+8 / +4). At the list's end they go round to its
    first / last when the menu wraps (+0x158); otherwise they answer the item they started from. With wrap and no
    item that can be chosen, the game's search never ends.
  - The steps (vf41 / vf42, the other way with +0x13c) play the no-move sound when the choice stays (+0x110).
    Otherwise they play the move sound (+0x111), tell the old item it is left and the owner (+0xb8: old, new), and
    restart the arrow on that side. Then they update the arrows and tell the new item it is chosen.
- **The pad (vf28).** It takes a pressed input only while the menu is enabled. While active, the owner's update
  must also leave it in state 2. The chosen item gets the input first.
  - Then the way along the menu: left/up step back (+0x158), right/down step on (+0x150).
  - Accept plays its sound by whether the item can be chosen, and tells the owner (+0xd8) when it can.
  - Back plays its sound when the owner allows it (+0x1c), and tells the owner (+0xb0).
  - The keys 0x63 and 0x61 play their sounds and tell the owner (+0xc8 / +0xd0) when the item is enabled. They
    answer 0, not 1.
- **Layout (vf39).** The choice is kept when it can still be chosen, else the first that can is taken.
  - With items that lay themselves out (+0x180), each item is formatted and the arrows are updated.
  - Otherwise the box (+0x170) becomes the largest item (when +0x160), or the items are formatted. Then the items
    are laid along the menu from its position by `cListFE3DWMenuFormatOp`. Each shown item is placed at the running
    position in its box by the alignment (+0x154), and the position moves on by the box (or the item's size) and
    the gap.
- **Arrows (`FE3DMenu_UpdateArrows`).** Each arrow is hidden when the choice can't go its way (without wrap), and
  put 5 beside the chosen item's bounds, centred on it across. For a menu running down, both go before it, side
  by side. +0x144 leaves them where they are, but only for a menu running across.
- **Aligning (`FE3DMenu_AlignItem` and 0x26df58).** These put an item before / after another along the menu: by
  the box when set (half the size difference) or by the other's size, plus the gap. Across, it is centred on the
  other by the box when set.
- **The list operations' destructors** (`cListOperator::~cListOperator` 0x269560 and the eleven that jump to it):
  the base table, freed on flag 1. The menu calls them directly.
- **Other operations:**
  - `cListFE3DGetExtentOp` widens two corners by each shown item's bounds;
  - `cListFE3DFindHashNameOp` answers the item whose +0x74 matches;
  - `cListFE3DSetActiveDefaultStartDataOp` calls the item's +0x30.

The test has 73,500 cases (49 functions, 1,500 each). The items are linked both ways and one of them can always be
chosen. A list's end is found where the nodes leave the items, or now and then at random. The sizes and bounds are
written by the items' stand-ins; the size call's buffer is not logged, as only the callee writes it. Sounds past
0x7f, sizes below 1 and a gap of 0 are in the mix.

All 124 mutants are caught.

## G9y2: the copies of the widget group's operations (`tests/fe_wgroup.rs`)

The group operations (`FE3DWGroup_*`) and the list operations' applies (`ListFE3D*Op_Apply*`) are compiled into
several translation units. There are 67 copies in all: three of each group operation and up to five of each apply.
Each copy has the same bytes as the G9x1 function it copies (`*4` / `Apply4`), but for its branch targets (compared
with the branches taken relative to each function's start). G9x1's test now also runs every copy in the runner
against the port of the function it copies. That is 20,100 cases (300 a copy), with some 10,000 nodes given an
operation. The port is G9x1's, and its mutants were caught there, so the copies are not mutated again. One copy,
`ListFE3DFormatOp_Apply1` 0x26fa70, is checked in G9y1 as the menu's format apply.

## Names from F1c batch 18 (proven)

- The card overlays' calls on the card system (+0x224): `MCOverlay_BeginLoad` 0x1ce6b8 (`BeginLoadSlot`, +0x28 =
  the bytes), `MCOverlay_LoadMore` 0x1d1fd0 (`LoadMore`, +0x28 += n), `MCOverlay_EndLoad` 0x1ce700;
  `MCOverlay_BeginSave` 0x1d1e30 (`BeginSaveSlot`, 0x33efbc = 1, +0x24 = the bytes), `MCOverlay_SaveMore` 0x1d1f18
  (`SaveMore`, +0x24 += n; the replay save's chunks), `MCOverlay_EndSave` 0x1ce880; `MCOverlay_ResetCard` 0x1d1e88
  (vf11, which syncs and resets the card state). Not settled: 0x1d1dd8 (calls the empty vf30), 0x1d1f60 (vf36, +0x44
  = 1, then state 0x14), 0x1d2010 (vf26, +0x40 = 1, then state 0x22), 0x1ce730 (vf15, card +0x24 = size; overlay
  +0x30).
- The app's VSync mode (app+4, state +0xc: 1 start, 2 run, 3 stop, 0 none). `cPS2ExecutionMan` vf1 starts a thread
  (entry 0x1825e8, not a function in Ghidra) and a VBlank-start handler (intc 3, 0x182170); the thread calls
  `App_VSyncTick` 0x17c9d8: input devices read for each elapsed tick, the mode run by its state, then the
  execution manager's vf at +0x2c. `App_RunVSyncModeTicks` 0x17cb40 (the mode's +0x34 once per recorded input tick,
  then +0x3c), `App_StartVSyncMode` 0x17c998 (+0x2c, state 2), `App_StopVSyncMode` 0x17c908 (state 3, waits on the
  manager's +0x24 until the tick clears it, deletes the mode). Inferred, not proven: the VSync mode is a loading
  screen.
- `TextBox_Destruct` 0x14f390 (results, lessons, `cStartScreen` +0x48), `SpriteSet_SetShape` 0x1f9ee8 (the 0x1c-byte
  rect `SpriteSet_GetRect` reads), `LessonOverlay_DrawPage2` / `3` 0x14c2c0 / 0x14c2c8 (empty in retail).

## G9y3: the front end's sound object (`game/src/fe_sound.rs`, `tests/fe_sound.rs`)

`cFE3DSound` is the front end's handle on the game's sound system (its object at 0x343050). Its base,
`cFE3DSoundBase`, has 54 virtuals that do nothing or answer 0, 0.0 or 1.

- **Construction and destruction.** The constructor sets the table, keeps its id at +4 and opens the sound system
  for it. The destructor sets its own table again, closes the sound system, then runs the base's destructor (the
  base table, freed on flag 1).
- **Handing on.** Most virtuals (24 of them) only hand their registers on to one of the sound system's functions,
  with the sound system in place of themselves. Three of them call rather than jump, which comes to the same.
- **Front end events (vf2, vf4).** The event is a signed byte; past 11 nothing plays. Each event has a sound id in
  one of two tables (the second has none for events 9 and 10). The id is played through the object's own +0x10.
- **Music (vf9).** By `k + 1` (nothing past 25), one of seven music calls is made with `k`, or 0x2129f8 is made
  with a track (-1 or 0..4).
- **Music switches (vf12..vf15, vf24).** These call their own +0x10 with -1, then start or stop the music by
  whether they are switched on.
- **Settings (vf34).** When the career holds sound settings (+0x169c of its block at +8), the sound system is told
  the bank (+0x1668), the five volumes (channels 0..2, the master volume, channels 3 and 4) and the mode (+0x166c).
  Then the sound system is updated.
- **Volumes.** Five virtual pairs get and set the volume of channels 0..4. vf37, vf39 and vf47 read words of the
  sound system.
- **Reloading (vf38).** When the sound system answers 1 for the bank, the five volumes are kept and the sound is
  stopped. Then each of these steps follows a frame after the last:
  1. the sound system is closed;
  2. the load is waited for;
  3. the bank is loaded;
  4. the volumes are set back;
  5. the object's id is opened again;
  6. the sound is updated.

  Then it is reloaded. A frame here is the front end's own wait: its widgets are updated, the renderer is asked
  until it answers, then the front end's +0x38 is called.

The test runs 66,600 cases (111 functions, 600 each). The k picks cover every one of vf9's indices. The object's
table is logged as the sound system is closed.

Of the 56 mutants, 55 are caught. The survivor reads the event as an unsigned byte rather than a signed one. That
gives the same result: both forms are past 11 for any byte with its top bit set.

## G9y4: the memory card overlay (`game/src/fe_mcard.rs`, `tests/fe_mcard.rs`)

`cFEStateMCOverlay` is the front end state shown over a screen while the memory card manager (the save code's
`cMCOverlayManager`, which the overlay is built on) saves or loads the career or the options. The `FEMemCard_*`
functions are its dialog, message, prompt and slot list.

- **Construction.** The manager is made and the front end state at +0x42c set up. Its flags (+0x480..+0x49c) and
  the dialog and message are cleared and both tables set, then the manager is started.
  - A save of the options first sets up their buffer (0x27d4 bytes, +0x490 0x1c4).
  - A load sets up the career's 0x80000 bytes (+0x490 0x176) or the options' (+0x490 0x177).
- **The pad (vf2).** While not quiet, a few of the manager's states take their own keys first: the key 0x63 in 7, 23
  and 28, enter in 9, and back / accept in 10 (neither with escape / enter). Otherwise accept, back, up and down give
  the manager's bits. Either pad counts.
- **The steps.**
  - vf8 begins: the career reads from the last loaded block, which is the career's +0x36fc (0x920 bytes); the options
    read into their cleared buffer.
  - vf9 reads more (at most 0x4000 at a time). First the block's header is summed (0x2ce04c) against the word it
    carries at +0x334, which is cleared first.
  - vf10 finishes:
    - the career block's bytes summed as signed bytes from 0xdeadbeef must match the word after its length, then the
      player is kept and the career loaded;
    - the options' sum must match, then they are copied into the career's settings, the sound mode set and the options
      saved.
- **The slots.** The slot line (vf44) is enabled when its 0x58-byte record has a save (+0x174 and +0x168). The slot
  list's buttons (`FEMemCard_FillSlotList`) follow what the card says: ready, empty (the empty-slot name 0x15f, by a
  compare of the wide strings), unusable (+0x1c8) or corrupt (0x148).
- **The dialog.** The text is set and centred, then the two buttons are hidden. Each bit of the buttons word then lays
  out button 1 or 2 with its sprite and string. Two shown buttons are placed side by side under the dialog (x 320 less
  the first's width less 50, and 370); one is centred. The state's buttons are disabled while a dialog is up.
- **The message and prompt.** The message is centred and given the menu font's height plus 4 as its line height. The
  prompt (0x15e when the card is in, else 0xdeb) goes 5 below it. The layout of the message (0x292e70) puts the card's
  text for the chosen slot at the root group's y + 60 and its detail under it.
- **The manager's state (vf54).** When quiet, only state 1 (the front end state's +0x28 and +0x40) and states 1..3's
  message are shown. Otherwise the dialog, message and prompt are hidden first, and each of the 34 states puts its
  own words up. State 1 also keeps the loaded career's player, or puts back the options' brightness (clamped to
  0..2) and music.
- **Two findings while testing.** The buttons' enable (0x2924d0) hands on 1 or 0, not the value it was given, and
  reads the two-slot flag after the first button. The front end state's +0x28 and +0x40 take only the state (what
  the other registers hold then is left by earlier calls).

The test runs 55,500 cases (37 functions, 1,500 each). The overlay, the front end state, the card (its table of
stand-ins) and eight widgets are random. The buttons' labels and icons are objects of their own, as laying out a
button writes its icon's sprite and clears its label's +0x114. The slot records run over the card's word (slot 2's
+0x174 is +0x224), so the card is written after them.

The runner converts ints to floats toward zero here (`cvt=c`), as the port's line height does; font heights past
2^24 are in the mix. The colour stand-in scribbles over the vector it is given, so a vector handed on again must be
written again.

Of the 95 mutants, 92 are caught. Three survive:
- clamping the read at `>= 0x4000` rather than `> 0x4000`, which is equivalent;
- the slot list's guard on a second button that is never null once the card has two slots' buttons;
- the slot list's buffer moved within the stack, which the test can't see, as stack addresses are compared as one
  token.

## G9y5: the world group (`game/src/fe_world.rs`, `tests/fe_world.rs`)

`FE3D_BuildWorldGroup` 0x2720f8 builds the two groups every screen keeps in its world. The screen's own widgets
come later.

- **The top group (+0x40).** It holds the screen's title, a white single-line text in the menu font at (20, 20).
  The title is measured, and two end pieces (sprites 0x21 and 0x20, grey) are put 27 below the text's top; the second
  sits at the text's width + 20. Then come the frame across the top (scaled sprites 0xd and 0xe, each half the
  screen wide and sprite 13 high) and the band under it (sprites 5 and 6), white.
- **The bottom group (+0x44).** It has its own frame pieces (scaled 0xb and 0xc, as tall as sprites 13 and 9 together,
  and 7 and 8), grey. The help bar's buttons are added when the screen asks for them (0x2719c0, kept at +0x48).
  Last comes the scrolling text (+0x4c): a text box in the highlight colour at (20, 37), sized 480 × 40, its string
  set, scaled 0.8 and given the font's height.
- **Both groups** are raised (+0x70 6) and added to the world. Sprite 5's rectangle is asked for twice; the second
  answer is not used.
- **The widgets are made in place.** The builder inlines the thing, widget, group and icon constructors
  ([`crate::fe_widgets`]). A scaled icon is an icon given the table 0x3b6e40 and +0x120 / +0x124 1.0. Every size call
  goes through the class's table, not the object's.
- **`FE_CreateList` 0x274000** makes the screen's menu list (0x1b0 bytes, the menu's constructor, then the list's
  table 0x3b7fe0 and its +0x168), at (320, 63).
- **`FE3DWorld_FindByHash` 0x26fe50** hands a find operation (its table and the hash, on the stack) to the world's
  +0x10, then destroys it.

The test runs 4,500 cases (3 functions, 1,500 each). The allocator stand-in hands out a heap in order; the text
constructors give their object a table of stand-ins; the icon classes' own virtuals are read from the game's tables
and stubbed. The font's height runs past 2^24 (cvt chops, `cvt=c`).

Of the 55 mutants, 50 are caught. The five survivors are equivalent:
- the fill guarded on a null allocation, which crashes either way;
- `y - 0` against `y`, which is the same here (twice);
- a size call through the class's table against the object's, when the object was just given that class's table
  (twice).

## G9y6: the rest of the front end's own code (`game/src/fe_misc.rs`, `tests/fe_misc.rs`)

The small functions left in 0x26c000..0x2aab58:

- **Destructors.**
  - The cameras' (combined, scripted, plain) and the animations' and geometry's: each sets its table, runs what its
    class adds (the node's vf1, the scripted camera's own part 0x26ac50), then goes on to its base, down to the
    thing's destructor (0x2acd10).
  - The fourteen states' destructors: each sets its table and goes on to the state's (0x2717a0).
  - The text icon's, text entry's, icon text's and icon entry's: each sets its table and goes on to the entry's.
- **Answers.** The twelve screen numbers the states' vf13 answer, nine virtuals that do nothing, one that answers 0
  and twelve that answer 1.
- **Helpers.**
  - `FE3DCamera_PlayShot`, and the scripted camera's vf28 (+0xb8).
  - `Vec4_ScaleCopy13`: a quadword scaled, read and written at the quadwords the pointers fall in.
  - The pad's enter / escape tests.
  - A free on flag 1 (0x270218).
  - `FE_CreateDialog`: a thing of 0x170 bytes, its float set, run in on track 1, raised and added to the state.
  - The count of the item table's groups present in the state's list (0x2754d8).
  - The title group's text and pieces resized (0x275760).
  - A two-list destructor (0x2759f0).
  - A child placed (0x28e020).
  - The overlay's state update (0x293120).
  - The player count's vf26.
- **The options pages' refreshes.**
  - Sound: two choices, five volume bars and an inverted choice from the sound player.
  - Game: four choices from the career's settings.
  - Display: five choices, one inverted and one less 2, and the brightness bar.

The test runs 60,000 cases (75 functions, 800 each). The thing's destructor 0x2acd10 is a stand-in on both sides,
as `fe_wentry`'s entry destructor already calls it, so the cameras' ports call it too. Only the group count is
given finds that answer none, as the other callers don't look. The vector scale is handed unaligned pointers and
zero lanes.

Of the 68 mutants, 64 are caught. The four survivors drop a destructor's own table store (the text icon's, text
entry's, icon text's and icon entry's). Each is a dead store: the next destructor in the chain writes its own
table before anything reads it.

## G9y4b: the memory card overlay's builder (`fe_mcard::build`, `tests/fe_mcard_build.rs`)

The builder 0x28f3e0 is not in the function index. It sits between the overlay's destructor and vf2, and makes the
overlay's widgets once:

- **Opening.** The dialog thing and the world group, titled 0xdd5 for a two-slot card, else 0xdd7 for the career
  or 0xdd6 for the options. The accept button is hidden unless two-slot.
- **The slot list.** One highlighted line for each slot the card reports. The card is asked again on every round.
- **The info list.** Four entries for the career, one for the options. Each holds the slot's number (formatted
  into the frame) and a line, the line's id 0xc + k.
- **The message group and the detail box.** Both start with the same 3-byte text.
- **The dialog** (+0x2c78), with its text and two buttons. Each button is an icon text entry with an icon 15 × 15
  and a label, at x 270 / 370 and the dialog's y + 165.
- **The message (+0x2c7c) and the prompt (+0x2c80).** Each is moved to its place through its +0x70 by the
  difference from its position (the VU0's subtract, w 1).
- **Closing.** The screen is laid out and the manager started unless quiet. The root and title groups go to the
  origin, and the bottom group to 480 less its height. Each one's +0x20 is cleared. The builder answers the bottom
  group.

The inline entry constructors are the widget's followed by the entry's words, so the port writes the widget's
(`fe_widgets`) and then the words the game leaves. The port keeps the builder's buffers at the stack frame's own
offsets: the strings the game builds on the stack are read by value, so the words around them must match too.

The test runs 1,500 cases with random flags, slot counts and positions. The widget classes' virtuals are read from
the game's tables and stubbed.

Of the 55 mutants, 52 are caught. The three survivors are equivalent:
- the short text's third byte, which is 0 in the game's data;
- the icon entry's table store, which the icon text's overwrites before anything reads it;
- the old-child destroy, never reached as the entries' children were just cleared.

## G9y7: the front end's small ported-but-unchecked functions

The index had 42 small functions in 0x26c000..0x2aab58 marked `ported`. Most were only named as calls in other
rows' ports. Three of them (the button cheats' `CheatCode_Matches`, `cCheatCodes::Update` and `FE_RumblePad`) were
already run by E10a's test and are now marked. The overlay's empty vf26 joined G9y4's test. The rest are ported in
`fe_misc.rs`, in batches.

### Batch 1 (`tests/fe_small.rs`)

- **The venue cameras.** `FE3DCamera_PanToVenue` and `FE3DCamera_PlayVenueShot` play a shot from a table of ten
  for a signed venue byte and keep the venue at +0x185, which they also store for venues out of range.
  `VenueSel_PlayCurrentShot` plays the kept venue's shot (0x90 + k). Out of range, it leaves 0x3b0000 in its
  answer, from an address loaded in a delay slot.
- **0x26c430** hands the scripted camera's part on to 0x26ac30.
- **The world's passes.** The render, text and update passes (0x26feb8, `FE3DTextList_RenderAll`,
  `FE3D_UpdateAll`) each put a list operation on the stack and hand it to the world's +0x10. The render pass's
  operation points at a stack copy of the render's matrix 0x346440.
- **The state.** `cFE3DState::cFE3DState` sets the state's words and makes its world: a thing of 0xd0 bytes with
  its own table and list, added to the front end's list. The destructor sets the table, runs vf12, then the node's
  destructor.
- **Small calls.** `FE_GetSpriteRect` asks the app's sprite set. `FE3DGroup_Add` adds to the group's list.
- **The two dialog screens.** World group, dialog thing (30 frames), menu list into the root group, then the
  state's buttons.
- **`FEList_SetFocused`** tells the old widget it is left (+0xf0 0) and the new one it is focused (+0xf0 1).
- **The help bar's layout (0x2755e8).** It measures the bar's fourth widget without using the size. If the height
  left over sprite 5's is more than 0, its first two widgets are sized 320 by it and scaled by it over sprite 15's
  height.

The test runs 15,000 cases (15 functions, 1,000 each). The camera's shot, the free on flag and the node's
destructor run as the original. Sprite heights and the help bar's height share a few values, so the help bar's
"more than 0" is tested at equality.

Of the 50 mutants, 47 are caught. The three survivors are equivalent: two read the venue byte unsigned (any byte
at or past 0x80 is out of range either way), and one drops the `- 0` from the help bar's height.

### Batch 2: the options screen's small functions (`tests/fe_opts.rs`)

- **The pages' reads and writes** (`FE3DOptions_ReadPage1..3` / `WritePage1..3`). Each is a fixed list of (item,
  page word, kind): a choice keeps the chosen node's index and is put back through the value widget's +0x160; a bar
  keeps the widget's +0x160 and is put back through the bar's value set.
- **The applies.** `SoundOptions_Apply` maps the first choice's id 8..11 to the sound mode, then hands the second
  choice's index, the five bars and the last choice (on for 0x12) to the sound player, keeping them in the career's
  settings. `GameOptions_Apply` sets four flags from the chosen ids (5, 5, 7, 7). `OptionsState_Refresh` runs the
  three pages' refreshes.
- **The change handlers.** Sound: the mode, the second choice (with a refresh) or the last choice, each followed by
  a sound cue (0x10, 0x11, 0x17). Game: the rumble on the pad that changed it (the node's +0xf0), or one of the two
  arrows. Display: the career's display words, and for the screen mode the renderer's mode, then its size from its
  width and height as unsigned floats. 0x2a9c50 does nothing.
- **The arrows' line chooser (0x2a9350).** It picks the third or fourth item by the item's id (0/2, 1/3; 4..7 by the
  list's chosen id). Any other id makes the game read the value widget of no item, the word at 0x114. The test
  keeps ids in range.
- **`FE3DOptions_SetHelpText`.** 0xdce for the end marker, else the open page's string table by the item's id, or
  the controller page's own string.
- **`ControllerConfig_Layout`.** Each of the three groups' widgets is laid out, placed by its entry, and moved back by
  its width / half width and up by its height / half height, by the entry's flags.
- **Builders.** The credits page's scrolling text, the title's two arrows and the three-line single-text page.

The test runs 15,200 cases (19 functions, 800 each). The controller tables' flags are randomised in the test's
copy, as the game's own entries don't use every mix.

Of the 68 mutants, 67 are caught. The survivor is equivalent under the chop rule: it drops the sticky bit from the
unsigned conversion.

### Batch 3: the rider bio's texts and the DVD screen's menus (`tests/fe_bio.rs`)

- **`FE3D_BuildRiderBioTextA` / `B`.** A scrolling text in the bio's group shows the line range for the screen's
  rider. The career's bio line for the rider's record picks the range: text A reads its table at 0x3af9c8, text B
  has its own (none past 11). The text is sized 350 × 260 at (205, 120), then laid out and added, and the group is
  shown.
- **`FE_BuildDvdCharacterMenu` / `FE_BuildDvdDemoMenu`.** A menu list of the screen's lines: the twelve character
  ids in order that have a string, or the two demo lines. Each line is red with a grey outline and slides in from
  740. The menu goes at (-640, 132 / 200), is kept at +0x68 / +0x74 and added.

The test runs 4,000 cases (4 functions, 1,000 each). The menu list's own virtuals are read from the game's table
and stubbed; the list-add stand-in logs the list's +0x190.

Of the 40 mutants, 38 are caught. The two survivors are equivalent on the game's own data: the two text scales
(0x3481c8, 0x3481cc) hold the same value, and the twelfth character id has no string.

G9y7 is done: every small function in 0x26c000..0x2aab58 is now checked. The nine big screen builders that the
index also marked `ported` (only named as calls) are G9y8.

## G9y8a: the rider profile's frame and records panel (`game/src/fe_riderbuild.rs`, `tests/fe_riderbuild.rs`)

- **`FE_BuildRiderProfileScreen` 0x27e918.** It builds the profile's frame, five icons added to the state:
  - the pale backdrop, 460 × 480 at x 180;
  - the picture (+0x4ec, sprite 0x7c);
  - the side bar, a scaled sprite 0x28 as wide as the sprite and scaled to 480 high;
  - two red bands at y 0 and 422.
- **`FE_BuildRiderRecordsScreen` 0x282d30.** It fills the records panel (the group at +0x4f8) for the screen's
  rider.
  - **Rows.** Each of the 10 events (0x3af8d0) gets a row in the left list: the event's name, and a text icon with
    either its record value (events 6 and 7, the icon hidden) or its medal and count. The other events also get a
    row in the right list with the second medal and count. A count of none shows the dash. The left counts are
    written as numbers into the screen's +0x58 buffers; the right ones go through the format into +0x364.
  - **Headings and rank list.** Six column headings follow. Then the rank list (+0x504): ten lines in the second
    font, with its marker (+0x508).
- **Made in place.** The entries are text entries (`fe_widgets::text_entry_init`) and text icons (a text entry with
  the text icon's table and an icon as its second child). A replaced child is destroyed through its +8 with 3.

The test runs 1,200 cases (600 each). The constructors give their objects a table of stand-ins and a random
+0xa8 (a value or none). Medals answer 0..3 and -1, and counts may be none or negative.

Of the 56 mutants, 55 are caught. The survivor is equivalent: it replaces a second child that is always none
there.

## G9y8b: the rider bio panel (`fe_riderbuild::build_bio`)

`FE_BuildRiderBioScreen` 0x27f600 fills the bio panel (the group at +0x4f0) with a list of 15 rows. Each row is a
text entry holding a heading (string 0x24a + row) and the rider's line. The line's string is the first string of
the rider's bio line (0xbac, 15 apart for bio lines 0..11; 0 past them) plus the row. Both texts are red at 0.8. The
list goes at (350, 111) with a gap of 4 and is added to the group, then the group is shown.

The test (with G9y8a's, `tests/fe_riderbuild.rs`) runs 600 cases of it. The text entry's own colour call goes
through the game's table, which the test reads and stubs. All 18 mutants are caught.

## G9y8d: the DVD screen's lists (`game/src/fe_dvdbuild.rs`, `tests/fe_dvdbuild.rs`)

- **`FE_BuildDvdCharacterList` 0x296870(s, item).** It builds a character's credits.
  - **The list.** The list at +0x6c is made the first time (later it is emptied through its +0x10). The title (+0x64)
    is given the character's title.
  - **The lines.** Each string of the character's range (from a table of 12; none for items 1, 6 and past 11)
    becomes either a heading or a line. A heading starts with the marker at 0x337e48: a buffered text in the second
    font, white, 0.6. A line is red, 0.7. Each goes 20 or 25 (a heading) or 15 or 20 (a line) below the one before,
    depending on what that was, and is moved there through its +0x70.
  - **The rest.** A marker icon carries the item. The list goes at -640, and each of its widgets slides in from 640.
- **`FE_BuildDvdJukebox` 0x297178.** A menu (+0x70) holds two lists of the 21 songs: the first 11 slide in from 640,
  the rest from -640. Two now-playing entries (+0xa0, +0xa4) go at (250, 115) and (250, 145).

The test runs 1,600 cases (800 each). Stand-in strings sometimes start with the heading marker, and the list's walk
stops after a few widgets. Some layouts take only the object.

Of the 45 mutants, 44 are caught. The survivor is equivalent: it swaps two colour buffers that hold the same
colour.

## G9y8f: the controller page (`game/src/fe_optbuild.rs`, `tests/fe_optbuild.rs`)

`FE_BuildControllerConfig` 0x2a36e8 builds the options screen's controller page.

- **Once** (the flag at 0x348460 is game data): the pad pictures' x is set, centring sprite 0x3d at 0.8 across
  640.
- **Pictures.** Three pad pictures, sized by their sprites at 0.8, go in the group at +0x90.
- **Label groups** (+0xc0..+0xc8). Each label comes from the same layouts that `ControllerConfig_Layout` uses. The
  third group is shown, then the pages are laid out.
- **Entries.** The page (+0x8c) gets four entries, each with a sub-menu as its second child, and an end entry (the
  id 0xabcd1234). The first two sub-menus get the choices 4 and 5, the next two get 6 and 7.

The test runs 800 cases with the once flag either way. The group's own +0x28 and the text entry's colours go
through the game's tables, which the test reads and stubs. All 36 mutants are caught.

## G9y8g: the display page (`game/src/fe_optbuild.rs`, `tests/fe_optbuild.rs`)

The board called 0x2a53d0 `FE_BuildOptionsScreen`, but it builds the options screen's display page (+0x98).

- **Entries.** Five entries (ids 0..4) each get a sub-menu as their second child. A sixth (id 5) gets a progress bar
  sized 100 by 30. Then comes the end entry.
- **Kind check.** Each child's kind is checked. A slider (its +0xc0 answers) gets +0x174 0x1a, +0x16c 0x19 and white
  at three colours. Otherwise, when its +0x90 answers, its bytes +0x112, +0x114 and +0x115 are set to -1.
- **Choices.** Nine choices (ids 7..15) go to the first four sub-menus; each time a choice is added, its sub-menu's
  +0x154 is set to 2. The fifth sub-menu gets five names (ids 0x10..0x14), set through +0x158 from a table at 0x3b40e0, 8 bytes
  apart.
- **Layout and the bar.** The sub-menus are laid out. The bar is shown from 0, sized 15 high, and set to the display
  brightness (0x17b970) through 0x2b3950.

The test now answers the kind checks with 0 or 1, so both sides of the check run. All 29 mutants are caught.

## G9y8e: the sound page (`game/src/fe_optbuild.rs`, `tests/fe_optbuild.rs`)

`FE_BuildSoundOptions` 0x29fb70 builds the options screen's sound page (+0x80). It is laid out like the display
page.

- **Entries.** There are eight entries (ids 0..7). The first two and the last get a sub-menu; the middle five get a
  progress bar sized 100 by 30. Then comes the end entry.
- **Kind check.** It is the display page's check, with one addition: a choice list (the +0x90 side) also gets
  +0x154 2.
- **Choices.** Entry 0 gets four choices (ids 8..11), entry 1 six (12..17) and entry 7 two (18, 19). Each sub-menu
  gets +0x154 2 once, before its choices.
- **Bars.** Each bar that is a slider is set to 1 by 1/11 (0x3dba2e8c), shown from 0, made visible and sized 150 by
  15.
- **The end.** The page is refreshed from the sound player (`SoundOptions_Refresh` 0x29ef50, called out with the
  screen) and laid out.

The test runs 800 cases. The stand-in tables' +0xc0 answers 0 or 1 and takes only the object (the game leaves the
loop counter in a1). Of the 29 mutants, 28 are caught. The survivor is equivalent: it moves the first bar's size
vector to where the other bars put theirs, on the stack, where nothing reads it afterwards.

## G9y8c: the rider short bio (`game/src/fe_riderbuild.rs`, `tests/fe_riderbuild.rs`)

`FE_BuildRiderShortBioScreen` 0x285230 builds the rider profile's short bio panel in the group at +0x4fc.

- **Bests.** It first copies the rider's bests: 0x9c bytes at +0x160 of the career's rider record (records are
  0x1c4 apart). The bests are pairs of (best, venue).
- **Rows.** A list gets 13 rows:
  - Ten rows show counts, written with "%d" through `WStr_FormatAscii` 0x17ce70.
  - Three show times, written in hundredths through `HUD_FormatTime` 0x1a6ba0 (the float times 100, chopped).
  - Each row's value goes in its own 0x28-byte wide string at 0x3d3098. The best's venue name comes from the venue
    table at 0x332f28 (0x1c bytes apart, the name at +4).
  - A count of 0 or less, or a time of 0 or less, shows "-" and "---" instead.
- **Favourite venue.** The last row is the favourite venue: the first venue with the most of the 13 counts at +0x38
  (compared unsigned), shown with "%s (%d)". When every count is 0 it shows "---".
- **Layout.** The list goes at (390, 135) with a gap of 3. Two headings follow, and the group is shown.

The test (with G9y8a's) runs 600 cases. Each case gives every rider bests either side of 0, venues among the 13,
and sometimes all-zero favourite counts. Two quirks in the test:

- The records panel's sprintf leaves a stray fourth word, so only the favourite's format logs its fourth argument.
- The short bio's group is its own object, apart from the records' group.

All 34 mutants are caught.

## G9z1: the screens' audio, voice-line and world helpers (`game/src/fe_scene.rs`, `tests/fe_scene.rs`)

There are 65 small functions in 0x261b38..0x263270 (two of them, 0x2623f0 and 0x262478, were missing from the
index), plus four from further up that they call. Most work through the front end's sound player: `cFE3D` +0xa4,
with its table at +0, not +0xc.

- **Voice lines.** 28 functions only play a voice line through the player's +0x10; line -1 stops it.
  `FE3DCharSel_SayIndexedLine` plays its byte argument through +0x18.
- **One sound call.** Eight functions only make a single sound-player call. The screens' `StartAudio` functions
  play a line, then one or two calls, or end on the music's third event (0x262f48).
- **The world's panels.** `FEWorld_SetOpenPanel` runs the open panel's closing script (panels 0..2), then the new
  panel's opening script, then sets the byte. The mode select's three lines open panels 0, 1 and 2.
- **The character select.** `FE3DCharSel_SetSelected` (f, player, -, sel) hides the twelve riders and shows `sel`
  and its two neighbours on each side, mod 12. It fades the player's old picture out and the new one in, clears
  every rider's selection and marker, then marks the chosen rider. The pictures are world things found by name hash
  and driven through 0x269be8: the thing's instance's controller +0xc0, with its table at +8. 0x262ad0 resets both
  players' choices to 4 and selects rider 4 for player 0.
- **Venues.** `VenueSel_SetVenue` / `_ClearVenue` run a venue's opening or closing script (8 bytes a venue at
  0x347180). `FEVenueSel_ShowMedals` sets each venue's medal mark from the career's medal, but only in mode 2 and
  when shown: gold 2, silver 1 and bronze 0 on parameter 2, and parameter 4 for visible. `FEWorld_ShowCourseVideo`
  stops and hides the ten course films, then opens and shows the course's film (128 by 128) and sets its music.

The test runs 200 cases of each. The functions that call each other run for real on both sides. The world's finds
answer none or one of four things, each with or without its instance and controller.

Of the 52 mutants, 48 are caught. The four survivors are equivalent:

- Player 1 for 0x262ad0's selection: both choices were just reset to 4.
- The course test without its `andi`: the course is a sign-extended byte, so "below 10" is the same test.
- The music byte read unsigned: the game's ten music bytes are all positive.

## G9z2: the front end's characters and their lighting (`game/src/fe_scene.rs`, `tests/fe_scene_char.rs`)

These are `cFE3DCharacter`, the riders standing in the front end's 3D scene, and its scene base. The draw (vf10)
is G9z2b.

- **Set-up.** `FE3DCharacter_Init` (c, rider) does the thing's set-up, then:
  - the character's table, poses 3 (shown +0xa0, wanted +0xa1), +0xa4 1, the rider (+0xb0), and no load in flight
    (+0xb4 and +0xb8 -1);
  - a rider model (0x5b00 bytes, its parts set with 1) and a rider record (0x84 bytes);
  - the model's textures requested through the loader with a request on the stack (0, then table 0x39e280);
  - lit fully but not selected.
  The destructor destroys the model (its table at +0) and frees the record.
- **Rider info.** `FE3DCharacter_FillRiderInfo` fills the record:
  - +0x44 from the board table at 0x332148 (0x14 bytes a board, 0xf0 a rider);
  - five parts from the career (0x16b1d0 / 220 / 1f8 / 270 / 248);
  - the rider row's words at 0x331ea8 (0x38 bytes a rider).
  `_LoadRiderAsync` then starts the load on the front end's loader (`cFE3D` +0x18) with the load options' first
  word.
- **Update (vf12).**
  - An outfit or board change (+0x15c, +0x160) releases a load in flight. With no load, it reloads instead, with
    bits 10 and 11 of the options set from the two changes.
  - A load that is done is finished and released.
  - When the model has its rider, a finished pose 1 goes to 2. A new wanted pose plays its clip: 0 0x2f9; 1 by the
    screen's mode (0x2fa..0x2fd); 2 0x2fe. Then the animation runs at speed 1 and steps.
  - The lights are worked out.
- **Lighting.** `FE3DCharacter_UpdateLighting` moves the light (+0x150) by 1/15 toward 1 when selected or toward 0
  when not, clamped to 0..1.
  - When the light moves, the light block (+0xc0) is filled. `_FillLightsA` (unselected) is used at 0 and
    `_FillLightsB` (selected) at 1. In between, `FE3DLighting_Lerp` mixes the two, from sets built on the stack.
  - Each of the three lights then gets a direction (+0x120 + 0x10 k) from its two angles in degrees:
    (cos a cos b, sin b, sin a cos b), normalised on VU0 with w 0.

The test runs 500 cases of each of the 17 functions. They run for real on both sides with the game's
`Math_Sin` / `Math_Cos`. The loader's options write a word, and a request on the stack is logged by value. Only the
destructor sees a character without its model or record. 0x2661f0 (the pose stopped) was missing from the index.

All 52 mutants are caught.

## G9z3: the dialog box, the group's own virtuals and two memory helpers (`game/src/fe_scene.rs`, `tests/fe_scene_dialog.rs`)

- **The constructor.** 0x267f58, which the index names `cFE3DThing::cFE3DThing`, is `cFE3DDialog`'s constructor.
  The dialog starts as a group. It then gets its own table and five sprite slots (+0x140..+0x150, none) and is
  sized 100 by 30 through its class. Its colours are (0.88, 0.1, 0.1, 0.1) and its bottom corners are sprites 0x11
  and 0x12. Its edge (+0x154) is that sprite's height; the corners' colour (+0x158) is white.
- **The frame (0x268308).** It is drawn in two passes:
  - In the dialog's colour: the body sprite over its box, less the edge (minus a sixth of it) at a top or a bottom
    that has corners. Then, unless the edge is 0, the edges' middles, inset 0.36 of the half width.
  - In the corners' colour, one render state up: the corners, half the box wide and the edge high, with anchors 0,
    8, 0xc and 4.
  An edge of 0 returns before the render state is put back, as in the game.
- **Layouts.** 0x2686c0 is the boxed layout: four corners, centred down the screen. 0x268808 is the bar layout:
  bottom corners only, at the top.
- **Virtuals.** The render and text render (vf10, vf11) raise the render state by its +0x70 around the children's
  operations. The update (vf12) runs the children, then its own +0x48 when active. Twelve small accessors (0x268ce0,
  0x268d00..0x268db0) and the scene's colour set-up (0x268c38) were missing from the index.
- **The group.** Ten of `cFE3DWGroup`'s virtuals here are byte-for-byte copies of `FE3DWGroup_*4` (all but the
  branch targets). They join `tests/fe_wgroup.rs`'s copies. Its extent (vf36: 9999 / -9999, then the extent
  operation), its vf21 (1) and its destructor are ported.
- **Memory.** 0x267f18 frees to the pool at 0x345e48 when the block lies inside it, and to the heap otherwise.

The test runs 400 cases of each of the 26 functions. The group's free of its children (a checked copy) runs for real.
All 48 mutants are caught.

## G9z4: the geometry, its films and the animations (`game/src/fe_scene.rs`, `tests/fe_scene_anim.rs`)

- **Geometry.** `FE3DGeometry_BindInstance` finds the world's instance by name hash (0x2550c8 on the world at
  0x346488) and sets bit 2 of its flags (+0xe8), clearing bit 1. `_GetPos` reads the instance's position with w 1.
- **Draws.** The geometry's draw (vf10) makes a matrix (0x1ce278) and draws the instance with its own matrix copied
  beside it (0x200680), at the render state raised by its +0x70. The film geometry's draw does the same unless a
  film is playing. With a film, it draws the film's texture on the instance through the renderer's +0x2a8, after
  its +0x1e0 with 3 and a lighting set-up (0x1ce140) of (1, 0.5, 0.5, 0.5), (0, 0, 0, 1) and 0. Its destructor
  stops the film.
- **Animations.**
  - The rate (0x269ea0) is 1 / (seconds x 60 x 60), multiplied in that order. The step (`cFE3DAnim` vf12) adds the
    rate to the time.
  - `cFE3DAnimRotate`'s apply (vf10) turns the matrix about x, y and z by its three angles in turn. Each rotation
    is built from `Math_SinCos` as the game builds it inline (axis-angle, t = 1 - cos), then multiplied in on VU0.
  - Its update (vf12) moves the angles from start to end by the time, onto the driven thing's +0x90 or its own.
  - `cFE3DAnimTranslate`'s apply moves the matrix's last row through the matrix by its offset.
  - Neither apply does anything when it drives another thing (+0xac).
- **Copies.** 0x2695c8 is a copy of 0x267f18's pool-or-heap free. 0x269608 is a copy of `Vec4_Set`. These two,
  an empty function (0x26957c) and the rate were missing from the index.

The test runs 600 cases of each of the 15 functions with the game's `Math_SinCos` for real. All 37 mutants are
caught.

## G9z5: the front end's cameras (`game/src/fe_scene.rs`, `tests/fe_scene_cam.rs`)

- **`cFE3DCamera`.** The constructor is the thing's set-up plus a field of view of pi/4 and planes 20 and 20000.
  - Its view (vf10) starts from the identity at 0x346440 with y and z swapped. It turns that about x by +0x90 less
    pi/2, then about z by +0x98, then about y by +0x94. Each turn is the inline axis-angle rotation (as in G9z4)
    multiplied in on VU0. The last row is the matrix times the position (+0x80).
  - It then sets the renderer's projection (+0xa8) and view (+0xf0).
  - `FE3DCamera_DrawWorld` draws the world's cells out to (far - 1) / 10000 + 1, then the world with the far plane.
- **`cFE3DCameraScripted`.**
  - Its set-up (0x26ab60) makes a camera-animation frame (0xc8 bytes from the pool at 0x345e48) with
    (0, -1, 30, 0) and the parameter table 0x3aa918, on shot 0x9a.
  - `_SetShot` remakes the frame from the shot's row (0x347208, 0x14 bytes a row) unless the shot is unchanged.
  - Its view takes the frame's angles in degrees and its position. The field of view is 2 atan(8.997951 / lens)
    when the lens is positive. The translation is the position through `Vec4_Scale` by -1, with w 1, and the camera
    keeps the angles, the field of view and the negated position.
- **`cFE3DCameraCombined`.** It holds a scripted camera at +0xc0. Its vf10, vf12 and vf28 pass through to it.
- **Shots.**
  - `FE3DCamera_PanToRider`: the rider after the kept one (+0x184, mod 12) gets shot 0xf + k. Any other rider gets
    0x1b (rider 0) or 0x27 - k.
  - The screens' shots: CharSelect 0x27 + k (mode 8), RiderShot 0x33 + k (mode 9), CharModify 0x3f + k (mode 0xa).
  - The custom board and body shots: 0x4b / 0x57 and 0x63 / 0x6f plus the kept rider, by a flag.
  - All of them go through the jump tables at 0x3ab2b0..0x3ab430, which run in order.
- **`FE3DCamera_SetMode`.** It works the game's nested switches through: on the mode; then, for modes 2 and 6, on
  the front end's screen kind (`cFE3D` +0xf0); and, in mode 6, on whether the app's +0x58 is 4. Mode 2 can keep a
  different mode (5, 4 or 3) from the one asked for. Only mode 1 answers whether it moved from a mode.

The test runs 500 cases of each of the 20 functions. `Math_SinCos`, `Math_Atan` and `Vec4_Scale` run for real. A
view matrix on the stack is logged by value, all 16 words.

Of the 50 mutants, 49 are caught. The survivor is data-equivalent: the shots' byte read unsigned, and all 16 rows'
bytes are 0 in the game's data.

## G9z2b: the front end character's draw (`game/src/fe_scene.rs`, `tests/fe_scene_char.rs`)

`cFE3DCharacter` vf10 0x2663f8 (c, mat) draws the character. It runs only when the character is shown and its
model has its rider.

- **Placement.** The model is placed by `mat`. When its turn (+0xb8) is set, the matrix is first turned about z by
  turn x -pi/6 - pi/2 and put at the front end's +0x60.
- **Shadow.** Unless +0xa4, the shadow's matrix is raised 1000 and turned a quarter turn about y, then about x, and
  the shadow's part (+0x7c0 + 0x394 x its index) is cleared. The shadow's matrix is then set and the shadow made.
- **Culling.** The renderer's box test (+0x280) gets a box of 70 either side across and from -50 to 150 up, about
  the placed position. A 1 culls the draw.
- **Lights.** The three lights' directions go through the transpose of the renderer's view (+0xf8). The game
  transposes with the MMI `PEXTLW` / `PEXTUW` / `PCPYLD` / `PCPYUD`, which the disassembler shows as words.
- **Passes.** The model is drawn twice: first with the ambient's first lane cleared, then with it restored. The
  render states, fog (0.08) and texture modes are set around the passes as the game does. Board 6 has the
  renderer's +0x2b8 on.

The test (with G9z2's) runs 500 draw cases. Matrices on the stack are logged by value (16 words), vectors 4 words.
The lights handed to the lighting set-up are logged by value too: otherwise clearing and restoring the ambient
would leave nothing to see. All 30 mutants are caught.

## Names from F1c batch 20 (proven)

Functions already ported under a bare address in `game/src/fe_*.rs`, named from the code and the Rust port. Several
were not functions in Ghidra and were made with `create_function` first: 0x262478, 0x2661f0, 0x267f18,
0x268ce0..0x268db0, 0x269608, 0x269ea0.

- Sound and characters: `FE3DSound_StopTitleSayLine0D` 0x2623f0, `FE3DSound_SayLine00` 0x262478 (the say-line
  table in `fe_scene.rs`: the front end's +0xa4, its +0x14 with 0), `FE3DPlayer_ClearPick` 0x265bb8 (+0x44 = -1),
  `FE3DCharacter_HasRider` 0x2663e8 (model +0x46b0), `FE3DCharacter_StopPose` 0x2661f0 (+0xa1 = 3,
  `AnimCtl_Reset` on the model's +0x46c0).
- Memory: `FE3D_ZeroBlock108` 0x267f08 (0x108 bytes), `FE3D_FreeBlock` 0x267f18 (`MemPool_Free` when inside the
  pool at 0x345e48, else `EAMem_Free`); `Mem_DeleteIfFlag1`..`4` 0x265bc8, 0x265da0, 0x270218, 0x184508 (the
  destructor tail of empty classes: `Mem_Delete` on bit 0; 0x270218 is the list operator's in `fe_wgroup.rs`).
- The dialog box (`cFE3DDialog`): `FE3DDialog_DrawFrame` 0x268308, `_SetupCentred` 0x2686c0 (four corners, y =
  (480 - height) / 2), `_SetupTopBar` 0x268808 (bottom corners 0x11, 0x12 at (0, 0)); accessors `SetEdge` /
  `GetEdge` 0x268d00 / 0x268d08 (+0x154), `SetCornerColor` / `GetCornerColor` 0x268d10 / 0x268d38 (+0x158, 16
  bytes), `SetSprite` / `GetSprite` 0x268d60 / 0x268d70 (+0x140 + 4k), `SetBodySprite` / `GetBodySprite`
  0x268d80 / 0x268d88 (+0x140), `HasBottomCorners` 0x268d90 (+0x150 and +0x14c both ≥ 0), `HasTopCorners`
  0x268db0 (+0x144 and +0x148). `Vec4_SetFromArray` 0x268ce0, `Vec4_SetCopy1` 0x269608.
- `FE3DAnim_SetRate` 0x269ea0: +0xa8 = 1 / (+0xa4 · 60 · 60).
- Menus: `FE3DMenu_AlignItemAfter` 0x26df58 (`FE3DMenu_AlignItem` 0x26dd60's twin), `FE3DMenu_RenderArrowsText`
  0x26f7c0 (+0x60 while chosen), `FE3DMenu_RenderArrows` 0x26f830 (+0x58), `FE3DMenu_GetItem` 0x26f970 and
  `FE3DMenu_FindItem` 0x26f990 (`FE3DGroup_GetChild` / `FindById` on +0x118). `FEWorld_Render` 0x26feb8.
- Screens and the stack: `FE3DState_LayoutAndSlideIn` 0x2742e8, `FE3DState_CountHelpButtons` 0x2754d8 (the 17
  (id, group) items at 0x347f20 in the list +0x48's +0x118, a group counted once), `FE3DState_LayoutHelpBar`
  0x2755e8, `FE3DState_SetTitle` 0x275760, `FE3DStack_Destruct` 0x2759f0 (the two lists at 0 and +0x24),
  `FE3DStack_LayoutAll` 0x275a70 (each screen's vtable +0x8c, `cFE3DState::LayoutWidgets`),
  `FE3DStack_UpdateTopAndFlush` 0x275ac8 (the top's +0x94, `cFE3DState::Update`, then the finished list deleted).
  `FEMemCard_ActivateState` 0x293120 (`cFE3DState::Activate` on +0x42c, then its +0x4c).
- DVD content: `FEDVDContent_SetCaptionId` 0x2994c8, `_SetCaptionText` 0x2995a8, `_ShowCaptions` 0x299688.
- Options: `FE3DOptions_BuildTitleArrows` 0x2a8520, `GameOptions_OnLineChosen` 0x2a9350, `GameOptions_OnChange`
  0x2a9848, `OptionsPage3_OnChange` 0x2a9c50 (empty).
- Widgets: `FE3DVideo_SetStartWord` 0x2aba70 (+0x158), `FE3DGroup_GetFirstWidget` 0x2ac698 (`List_First` on
  +0xa0), `FE3DThing_SetVec90` / `SetVec90Xyz` 0x2aca88 / 0x2acaa8, `FE3DWidget_SetWorldPosXyzw` 0x2acb70,
  `_SetColorRgba` 0x2acc40 (its +0x54), `_SetSizeXyzw` 0x2ad7e8 (its +0x13c), `_PostEvent` 0x2ada58 (the record
  at 0x347e10 through its +0xec), `FE3DWidget_PlaceChild` 0x28e020 (`FE3DWidget_Place` on +0x114),
  `FE3DWidget_SlideInDir3Flag` 0x2adf88, `FE3DButton_LayoutReversed` 0x2ae230, `FE3DListBox_AddWidget` 0x2aff90,
  `FE3DColouredText_FindSplit` 0x2b1a68, `FE3DColouredText_SetColour2` 0x2b4858, `FE3DEntry_SetFirstChild` /
  `SetSecondChild` 0x2b3300 / 0x2b32a8 (+0x110 / +0x114), `FE3DWidget_SetWidth` / `SetHeight` 0x2b3860 /
  0x2b3820, `FE3DWList_GetFrameOffset` 0x2b5958, `Vec4_ScaleCopy20` 0x2adcb8. The bare constructors
  `cFE3DWListBox::cFE3DWListBox` 0x2b2ec0 and `cFE3DWSingleLineText::cFE3DWSingleLineText` 0x2b4ab0 have no
  callers (unused out-of-line copies).
- Not settled: the music cues 0x262ad0, 0x262b78, 0x262bd0, 0x262c20, 0x262c60, 0x262ca0; 0x263270 (a player's
  byte +1 = 4); 0x265758; 0x26c430 (`j 0x26ac30` with this + 0xc0; 0x26ac30 not settled).

## Names from F1c batch 26 (proven)

The results table helpers of `game/src/resultpages.rs`, made functions in Ghidra and named: `Results_SetCellFloatPlain`
0x1bf980 ("%f" at 0x38e210), `Results_SetCellIntSuffix` 0x1bf9d8 ("%d%s" at 0x38e208), `Results_GetCell` 0x1bfa70 and
`Results_GetCellB` 0x1bfa90 (0x3385c0 + row·0x800 + col·0x80, the same code), `Results_GetRowName` 0x1bfae0 and
`Results_GetRowNameB` 0x1bfaf8 (0x33c5c0 + row·0x80), `Results_ClearSpeechEvent` 0x1af940 (`SpeechSys_ClearEvent`,
in the page table at 0x38f874). None has a direct caller.

## Names from F1c batch 28 (proven)

Functions the index had with no system at all, read in Ghidra:

- `Draw_GradientLine` 0x19c310, `Draw_RectOutline` 0x19c458, `Draw_Triangle` 0x19c620: the 2D line with a colour
  per end, the rectangle's four edges and the filled triangle (`quad2d.rs`, checked), beside `Draw_Line` 0x19c1e8.
- `FE3DTransition_ApplySavedValues` 0x1ff590 / `_ApplyCurrentValues` 0x1ff608: while a blend's values are held
  (+0x64), the renderer's vf +0x128 given the saved values (+0x1c, the second word 0 and the third 1.0) or the
  current ones (+0x30) (`fe_transition.rs` `hand_saved` / `hand_current`).
- `ListFE3DSkipActivityOp_Apply5` 0x2ad148 (the child's +0x38, 0) and `ListFE3DOffsetOp_Apply5` 0x2ad1c8 /
  `_Apply6` 0x2b6978 (the child's +0x70 with the op's vector +0x10, 0): byte-for-byte the bodies of
  `cListFE3DSkipActivityOp::vf1` 0x269968 and `cListFE3DOffsetOp::vf1` 0x2699e8, with no references (more unused
  copies, as in batch 16; `fe_wbar.rs` `op_skip` / `op_move`).
- `MockupEditor_AddLocText` 0x1d4538 and `MockupEditor_AddSprite` 0x1d4410: `MockupEditor_AddItem` 0x1d4660's
  siblings. The item's kind (+0x14 of the 0xd4-byte record) is 1 with a string id (+0x3c, drawn through
  `Loc_GetString`), 2 with ASCII text (+0x40, `MockupEditor_AddItem`) and 3 with a sprite (+0xc4, drawn with
  `Sprite_DrawRectScaled`), as `MockupEditor_DrawItemInfo` 0x1d3c00 reads them; both called from
  `MockupEditor_DrawItems`.
- `BdrLoad_FERequest` 0x201a38: a rider load request with byte 0 = 1 and the bits 0x1f00 set, 0x2000 cleared
  (`BdrLoad_DefaultRequest` 0x201a98 is the same with byte 0 = 0xf); its one caller is
  `FE3DCharacter_LoadRiderAsync`. The name is from that caller (inferred: what byte 0 selects).

Left unnamed (no references, meaning open): 0x1818c0 (renderer vf +0x108 and the font word 0x337ff8 = 0.002),
0x1c0148 (`cScreenOverlay`: when shown, its +0x28 table's +0x78; in no vtable), 0x19aa18, 0x238980, 0x201c10 (+8,
+0xc, +0x10 set from three arguments), 0x256100 (empty), 0x1fb4e0 / 0x2049b0 / 0x204f28.
