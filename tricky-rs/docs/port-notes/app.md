# The application and the game object

Notes for the board's E1 rows: `cApplication` (the game's `gApp` at 0x337c58), the modules it runs, and `cGame`.
Addresses are `SLUS_203.26`.

## E1c: the course table and the app's small helpers (`game/src/app.rs`, `tests/app.rs`)

- **Constructors.** `cAppMan`'s constructor (0x17c530) makes the object the game's `gApp`. It keeps the execution
  manager it is handed at +0x28 and sets +0x14 to 10 and +0x18 to 1.0.
  - `cApplication`'s constructor is 0x17b748; the index calls it `App_LoadFonts`. It does the manager's part, sets
    its table and clears +0x34..+0x4c. It then makes an empty course table at +0x64 and the course archives at
    +0x698 (`BigArchive_Init`), and two fonts (0x3c bytes each) at +0x5c and +0x60.
  - `Game_Main` hands both constructors the execution manager.
- **The course table** (`CourseTable_*`, 0x186218..0x186440) holds a count, then 0x84-byte records: a path at +4,
  the course's file name at +0x44 and its number at +0x84.
  - `Course_OpenModelsBig` fills it from the courses at 0x332f28 (0x1c bytes each), all but the front end's.
  - `_FindByName` answers the count when no record matches; `_FindById` answers -1.
  - `_GetId` answers 13 for any number of 14 or more.
  - The app's `App_GetCourseId` / `_FindCourseById` / `_GetCourseInfo` go to the table at +0x64.
- **Strings.** `Loc_Load` frees the strings, then makes a buffer at least the strings file's size: the larger of
  that size and the size kept at +0x54, which the free has just cleared. `Loc_GetUnquotedString` steps past an
  opening wide quote and cuts a closing one.
- **The effects loader.** `App_CreateEffectsLoader` copies the loader's fog colour words into the renderer's fog
  record. It reads them through `gApp`'s +0x30, which is the loader it has just made.
- **Copies.** 0x186170 and 0x1861b0 are copies of the pool-or-heap free (0x267f18). They were missing from the index.

The test runs 400 cases of each of the 27 functions. The table's own functions run for real; `strcmp` matches
about half the time.

Of the 43 mutants, 41 are caught. The two survivors are equivalent:

- The course's file name read from +0x14 instead of +0x10: every course holds the same pointer at both.
- `_GetId`'s limit of 13 instead of 14: a number of 13 answers 13 either way.

## E1d: the game object (`game/src/game_main.rs`, `tests/game_main.rs`)

`cGame` is the module the application runs for a race and the screens around it.

- **Frame.** `cGame::Update` runs the lesson's update and the world's object lists (three while paused, eleven and
  a pass otherwise). Then:
  - It updates the pause menu when that is open.
  - Otherwise, when the game can pause, it looks for a rider whose controller's +4 is clear; that rider's player
    pauses. Without one, the pause button pauses for a player in the race.
  - The pause goes to the race menu when the overlay manager's page is up. Otherwise it goes to the pause menu,
    which is opened for the player with its page (the overlays' page 6 when it has none).
  - Last come the cameras and the frame count.
- **Pause test.** `Game_CanPause` refuses while the overlays are busy, and in modes 1 and 6. It allows every mode
  but 5; in mode 5 it allows only a lesson step from 1 to 9.
- **Results switch.** `Game_UpdateOrShowResults` runs the frame while the race runs. Once the race is over it makes
  the next module, once:
  - the quit module, when the career goes back;
  - otherwise, by the career's flags (+0x3710, +0x3704, mode 6, +0x370c, +0x3708, mode 1) and whether two players
    are playing, the front end (with 1 or 0) or one of five results modules.
- **Draw.** The draw (vf6) draws the race frame while racing. On the results it sets up the whole display: the
  viewport takes the renderer's sizes as unsigned words, through the EE's round-toward-zero `cvt.s.w`; then the
  depth range, fog, the font's scale and so on.
- **Destructor.** The destructor (vf0) waits for the renderer twice. It tears down about twenty objects in the
  game's order and resets the lesson object.
- **`Game_Main`.** It makes the execution manager, then the application (0x7cc bytes, zeroed) on it, runs it, and
  destroys it.
- **Index.** 0x181878 (the lesson-pause frame) and 0x180df0 (the leave) were missing from the index.

The test runs 500 cases of each of the 16 functions. The game's own functions and the application's constructors
run for real. The pause button writes a player; the race and the +0x2a0 object have their own stand-in table.

Of the 50 mutants, 49 are caught. The survivor is equivalent: `(v >> 1) | (v & 1)` without the low bit converts
to the same float, because `cvt.s.w` chops and the sticky bit never changes a chopped result.

## E1e: the application's start, shutdown and main loop (`game/src/app.rs`, `tests/app_loop.rs`)

- **Start.** `cApplication` vf1 (0x17ba50) makes the world, the renderer (+0x724), the fades (+0x72c) and 200
  records at +0x728. Each record is reset to -1, 1, 1, 0, 0, 1, 1. It then loads the strings and starts the career
  and the sound.
- **Next module.** vf3 (0x17bbd8) picks the next module by the app's stage at +0x73c:
  - 0: the start screen;
  - 1: the archives opened, then the front end;
  - 2: a film when the career's +0x3260 bit 5 asks for one (the bit is then cleared); otherwise the front end when
    any of the other bits are set; otherwise the race set up from the career's chosen course, mode and player, and
    the game;
  - 3: the game.
- **Shutdown.** vf2 (0x17bf48) and the two destructors tear the app down in the game's order.
- **The vsync mode.** While a module is pending at +4, the vsync interrupt (`App_VSyncTick`) runs it.
  - The interrupt moves the fraction at +0x1c on by the rate at +0x18. It steps the input through every whole frame
    the fraction crosses, then acts on the vsync stage at +0xc:
    - 1: enter the module, then stage 2;
    - 2: tick;
    - 3: ask whether it is done (stage 0), else tick.
  - `App_StopVSyncMode` sets stage 3 and waits for the stage to change. It then destroys the pending module.
- **Main loop.** `App_MainLoop` enters each module and stops the vsync mode, then runs frame by frame:
  - After the display's wait, it leaves the module when the app is quitting or a pending module is ready, provided
    the module agrees.
  - Otherwise each pending input frame updates the module and, unless paused, draws it. At most the app's +0x14
    frames run at a time.
  - When the module is left, the next one comes from the app's +0x20 virtual.

The test runs 400 cases of each of the 15 functions. In the test, the display's wait stands in for the vsync
interrupt: it sometimes hands over a pending module and sometimes sets the quit flag (to 1 or 2). The heap starts
as junk, so a missing clear shows.

Of the 51 mutants, 50 are caught. The survivor is equivalent: the front end's test of the career's bits without
bit 5, which has already been handled just above.

## E1f1: the start screen's module (`game/src/start_screen.rs`, `tests/start_screen.rs`)

The start screen is the module the application opens first. `cStartScreenSingle` is the one-player screen built on
`cStartScreen`.

- **Making it.** `StartScreenSingle_Create` (0x184318) makes it: 0x3cc8 bytes, then its font at +8 and its text box
  at +0x48. It keeps going even when the allocation fails.
- **Start.** vf4 (0x182dd8) makes the memory card object (+4) and sets the renderer white with alpha 0. It clears
  the counters, makes the start module (0x410 bytes) and switches the app to it.
  - The single screen's vf4 also clears its state, its wait and the card's answers.
- **Enter.** vf1 loads the font (sized 1.8 by 1.4) and picks sprite set 0x2c9. It starts the blink at 1/60 with a
  step of 1.
- **Leave.** vf2 does nothing until +0x44 is set. It then draws one last black frame on a 640 by 480 screen and
  waits for it. Then it frees the card and switches the app. It starts the career unless the card has already
  loaded one (+0x3cb0), and releases the font.
- **Draw.** vf6 sets up the same screen. It draws a white rectangle and sprite 0x28 of the sets over the whole
  screen, then the screen's own +0x40 while it runs.
  - The single screen's vf6 is this draw, answering 0.
- **Small functions.**
  - 0x1843a8 resets the record at 0x3cdd28 (-1, 1, 1, 0, 0, 1, 1).
  - 0x1843e0 draws a sprite with its size scaled by the set's +4 and +8.
  - 0x184528 and 0x184580 set a colour (0x184580 is a copy of `Color_SetRGB`).
  - 0x1844a0 is a copy of the module's destructor; 0x184538 is a copy of the pool-or-heap free.
  - Eight of these small functions were missing from the index.

The test runs 400 cases of each of the 19 functions. The module's destructor and `Color_SetRGB` run for real.

Of the 46 mutants, 45 are caught. The survivor is equivalent: the sticky bit in the unsigned conversion, which
never changes a chopped `cvt.s.w` (as in E1d).

## E1f2: the single start screen's state machine (same file, `tests/start_screen_state.rs`)

The single start screen checks the memory card for a saved career before the front end opens. Its state at +0x3cc4
(28 states) drives that check through the card object's virtuals.

- **`StartScreenSingle_SetState`** (0x1850e8) keeps the state and does its entry work:
  - the timer at +0x14b8 (75, 120 or 180 frames) or the wait at +0x14c4 (15 or 120);
  - the card's questions in state 2: is a card there, is it formatted wrong, is there a save, is it damaged;
  - the save's file name in states 3 and 9: the game code and a tag, put together with `sprintf`;
  - reading the block into +0x14d0 in state 5.
  It calls itself for the state it picks.
- **The frame** (vf5, 0x1845e0):
  - It moves the blink between 0.2 and 1, turning it back past either end. Nothing more runs once +0x44 is set.
  - It updates the card, counts the timers down and reads the new buttons.
  - Outside the states that wait on the card, a changed card makes it pick a state again (the same questions as
    state 2, mapped a little differently).
  - Then the state's own step: the button answers (0x880 back, 0x120 yes), the card's file count and room, and so on.
  - In state 6 a block whose CRC matches the one kept at +0x3c9c is copied into the career. Then the audio is set up
    again when the system changed, and the brightness is set from the block (+0.5, kept from 0 to 2).
  - The index gives this function as 968 bytes; it is 0xb08.
- **The message** (vf7, 0x185430) resets the text box and draws the state's message in the card's box. The message
  is one or two localised strings, some with the size or count put in.
- **`Input_GetAnyNewButtons`** (0x1834a8) is ported here on the game's bytes: the buttons that are down now and were
  not before, on any device.
- **0x1858c0** is a static initialiser: three colours and a record. It was missing from the index.

The test runs 3,000 cases of each of the 5 functions. `SetState` and `Loc_GetUnquotedString` run for real. The
stand-ins that fill a buffer write four words into it. `sprintf`'s stand-in also logs where the name's parts lie,
so a moved buffer shows.

All 83 mutants are caught. Six were first missed by gaps in the test; the cases now cover them:
- the buffer offsets;
- a blink landing exactly on its limit;
- a sound answer of 2;
- a career block that starts as random words;
- four files on the card;
- counts other than 1 and 2.

## E1f3: the memory card messages (same file, `tests/start_screen_msg.rs`)

- **`MemCardMsg_DrawBox`** (0x183538) draws a message box.
  - It places the screen's text box at (112, 96), 416 wide and four font lines high. The line height is a word
    turned into a float by the chopping `cvt.s.w`.
  - It sets the text, centres it, lays it out, and draws it white.
  - Then it draws the buttons when the message has any.
- **`Overlay_DrawDontRemoveCard`** draws string 0x15e at (112, 230.4) in the colour of the blink.
- **`MemCardMsg_Draw`** (0x183c20) draws up to three groups of buttons:
  - the groups: 1 or 2; 4 or 0x20; 8, 0x10 or 0x40;
  - each button is a sprite with its string beside it;
  - while waiting, the buttons go in a row centred on 320, measured first by `_DrawOptions`; otherwise down a
    column at x 160, 30 apart from y 340.
- **The game's row.** In the three-group row, each step moves x on by the running total of the widths so far. So
  the first button's width counts twice. The port does the same.
- **One group.** With flag 1, its button is drawn, and then the other flags are still checked.
- **`_DrawButtonPrompt`** picks the button's sprite: 0x51, 0x50, 0x4f or 0x52. For a button without one it uses
  whatever the caller left in a3; the game's own callers never pass such a button.

The test runs 4,000 cases of each of the 5 functions. They run for real together; the strings, widths, sprites and
the text box are stand-ins.

Of the 44 mutants, 43 are caught. The survivor is equivalent: in the two-group row, button 2 instead of 1 for the
first group. Both use sprite 0x51.

`cStartScreenSingle` vf7 still calls the box through the call seam (`DRAW_BOX`), as it was checked in E1f2.
