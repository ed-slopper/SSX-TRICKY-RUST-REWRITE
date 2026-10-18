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
