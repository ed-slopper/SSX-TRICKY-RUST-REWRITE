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

## E1g: the boot films and the execution manager (`game/src/boot.rs`, `tests/boot.rs`)

- **`cVideoModule`** plays one boot film.
  - Its constructor copies the film's name to +0xc and keeps the word after it at +0x10c.
  - Its enter makes the video player (+8), and its start hands the player the name.
  - Its frame ticks the sound (0.02 s) and runs the player until it has finished. Then, once (+0x110), it makes the
    next module. The choice is by the career's flags, the same tests as `cApplication` vf3's: the front end, or one of
    the six results modules.
  - Its draw runs the player's +0x20 between the renderer's checks.
- **`cVideoLoadModule`'s** virtuals do nothing, or answer 1.
- **The static initialiser** 0x186a00 is 0x1858c0's again, 0x50 bytes on. Both now go through one function,
  `start_screen::colours_init`.
- **`cPS2ExecutionMan`** runs the game's frame timing.
  - vf1 makes the frame semaphore (+0xc) and the vsync semaphore (+0x4024, counting to 0x400). It starts the vsync
    thread (+0x4020): 0x1825e8, on the 0x4000-byte stack inside the manager, one priority above the main thread's.
    Then it adds a handler for interrupt 3.
  - That handler (0x1825d0) signals the vsync semaphore. The thread waits on it and runs `App_VSyncTick`, forever.
  - vf3 waits for a frame and then drains the semaphore; vf4 signals it while something waits (+0x10).
  - vf2 undoes vf1.
- **The stack canary.** `Stack_FillCanary` keeps the stack's top, bottom and end, and the stack pointer's page. It
  fills from the bottom to the top, and from the end to 1 KiB under the stack pointer, with 0xdeafc0ed.
  `Mem_PrintStackUsage` counts how much of each is still the canary and prints it.
- **`Iop_RebootAndLoadModules`** runs the boot's steps in order:
  - it reboots and syncs the IOP, retrying until each takes;
  - it loads the six IOP modules, each retried until it loads;
  - it seeds the effects' generator from the clock and prints the date (BCD);
  - it lays out the main heap: 2 KiB past the 64-byte line after a probing 1 KiB `malloc`, up to the heap's end. The
    heap is filled with 0xdeafc0de (one letter off the canary) and the canary is laid.
  - `PS2ExecutionMan_Create` calls it, then makes the 0x4030-byte manager.
- **Calls out.** Everything that talks to the kernel, the SDK or the IOP stays a call out.

The test runs 1,200 cases of each of the 28 functions.
- **The vsync thread.** It never returns. The test stops it at its fifth wait: the stand-in points the return
  address at the runner's sentinel, and the port's side unwinds.
- **Stand-ins that write.** The thread's status, the clock and `memset` write their buffers. `malloc` answers near a
  small heap whose end the stand-in picks.
- **Index.** 0x1825d0, 0x1825e8 and 0x186a00 were missing from it.

Of the 66 mutants, 65 are caught. The survivor is equivalent: the PS2 manager's destructor stores its table just
before the base destructor overwrites it. Three were first missed and are now covered: the results path for two
players, and a canary run that reaches the stack's bottom (twice).

## E1h: `cGame::Init` (`game/src/game_main.rs`, `tests/game_init.rs`)

`cGame::Init` (0x17e148) sets the race up.
- **Progress.** The app keeps the game at +0x730 and counts its progress at +8, from 0 to 10. The load screen reads
  that count.
- **Before the objects.** It resets the world and sets each pad's vibration from the career's settings (+0x1678,
  +0x167c). It seeds the game's generator from the career's +0x2b04, then sets up the pool, the world's lists and
  the lesson.
- **The objects,** in the game's order:
  - the replay node (+0x14) and the race (+0x1c): the race gets its own table and eleven part tables, and three
    lists;
  - a node (+0x18), then the riders (+0x40; none in mode 6): they are given the career's list and how many of its
    entries are in use;
  - the world (+0x24), the tricks (+0x30) and the course (+0x10, found by number in the course table);
  - the courses (+0x28): this course's record (+0x2c, the index kept from 0 to 12) gives the fog colour (/255, at
    +0x5c);
  - the HUD's fades, the overlays, the snow, the speech, the lesson helper (mode 6), three more objects, the
    controllers, the shadows and the race menu (+0x2a0).
- **The attract flag** (+0x74) is set when an effects random draw falls under the course's chance (+0x54). It is
  only drawn outside modes 2 and 4, with fewer than two players and fewer than three AI.
- **The snow's rate** is 0.05 plus 0.15 of a draw, kept at +0x78. Its drift is a draw plus 5, and 7 less a draw.
- **The end.** The two fonts are sized (1.3 by 1.4, 1.4 by 1.8), and the second is the pause menu's. Then the
  course's archive and world are opened, the music is picked by the course's name upper-cased, the HUD option and
  viewport mode are set again, the brightness likewise, the archives closed, and the game restarted.

The test runs 3,000 cases.
- **Run for real:** the checked course table look-ups, the archive's open and close, the effects loader and the
  restart.
- **Stand-ins that act:** the fill stand-in really fills; the node constructor leaves the race its player and AI
  counts; the courses' constructor fills its records; `strcpy` copies a short string.
- **The race's table** is the game's (0x364738); its two virtuals are stood in for.
- **The progress count** at app+8 is logged with every call, so each step shows.

Of the 51 mutants, 50 are caught. The survivor is the fill skipped when an allocation fails. The test's allocator
never fails: a failed one would write the tables at address 0 on both sides. Two gaps were first missed: the
progress steps, and the pads' settings (any nonzero is on). A third, the attract draw landing exactly on the chance,
is now covered by draws of exactly 0, 0.25 and 0.5.

## Names from F1c batch 20 (proven)

- The vsync thread (settles the batch 18 gap 0x1825e8): `App_OnVSync` 0x1825d0 (the interrupt handler:
  `iSignalSema` on gApp+0x28's +0x4024), `App_VSyncThread` 0x1825e8 (`WaitSema` on the same, then `App_VSyncTick`,
  forever). Both made with `create_function`.
- `cGame::vf2` 0x180df0 (the vtable 0x386980 slot at +0x1c; `game_main.rs` calls it the leave): +0xc = 0, return 1.
- `SpriteSet_DrawScaled` 0x1843e0 (`Sprite_DrawAnchored` with w, h scaled by the set's +4 and +8),
  `Color_SetRGBFromVec` 0x184528, `Color_SetRGBCopy1` 0x184580 (a copy of `Color_SetRGB`). `TextBox_DrawLines`
  0x195a48.
- Not settled: 0x1843a8 (resets the record at 0x3cdd28), 0x184578 (empty), 0x1858c0 / 0x186a00 (static
  initialisers), 0x11f488, 0x257dc0.

## E1i: the game's and the app's small helpers (`game/src/game_helpers.rs`, `tests/game_helpers.rs`)

- **Pause tests.** `cGame::IsPaused` is true for any of: the game's own flag (+0x48), the pause menu, the lesson
  (when there are riders) or the speech. `Game_IsMenuOrOverlayActive` is the same without the lesson.
- **Setters.** `Game_SetHudOption`, `Game_SetViewportMode` and `Render_SetBrightness` keep their value in the career
  (+0x16b0, +0x1688, +0x16b4) and set the settings-changed flag (+0x3258) only when it changes.
  - The viewport mode also sets +0x29c to the other mode and lays the views out.
  - The brightness reads the display's colour, sets the renderer's +0x67c to the value + 0.5 (kept from 0 to 2,
    as in E1f2's state 6), and sets the colour again. It compares as floats, so -0 and +0 are the same.
- **The race restart and the camera reset** share a start: the race's restart call, each camera reset, the camera
  script stopped and reset, the riders restarted. They also share an end: the replay, the speech, the sound and the
  pads' latch.
  - The restart calls the race's +0x80; in mode 7 it takes the career's course record (+0x2b08 its +0x10).
  - The camera reset calls the lesson helper and the race's +0x88.
- **Views.** `Game_RestoreViews` sets as many views as cameras; `Game_UseSingleView` sets one. Both lay the views
  out.
- **The app.**
  - `App_SetMode` keeps the module pending and enters it.
  - `Loc_LoadForRegion` makes the strings' buffer and loads region 0's or region 1's strings file.
  - `App_OpenBigFiles` opens the seven archives (three only when their file is there) with the quiet flag at
    0x3bb978 cleared. It then fills the course table and writes the build string.
- **0x182170** is the interrupt wrapper. It keeps every float register over the handler's call and ends with `ei`,
  which the runner does not run, so it is ported but not checked.
- **`cPS2Device`** (the pads, on structs in `controls.rs`) stays with the input rows.

The test runs 1,000 cases of each of the 15 functions. `Loc_Load` and the course table's fill run for real. The
course table's `sprintf` is logged with three arguments and the build string's with four. The archive stand-in logs
the quiet flag.

Of the 52 mutants, 51 are caught. The survivor is equivalent: the brightness compared as plain floats, where -0 and
+0 are equal too. Two were first missed: the speech test's answer of 2, and the quiet flag.

## Names from F1c batch 30 (proven)

- `Gfx_ResetVif1IrqCounts` 0x1c0990: zeroes the three pairs +0xc / +0x10 of the VIF1 interrupt record 0x33d7b0
  (from `Gfx_StartVif1Irq` 0x1c0920; `Gfx_Vif1Interrupt` 0x1c0c70 counts +0xc and writes VIF1_FBRST = 8 when
  VIF1_STAT & 0x400). `Gfx_UpdateVif1Irq` 0x1c09c8: empty, called each frame by `cPS2GraphicsMan` vf15 0x1dd4f0
  (at 0x1dd5a4) with 0x33d7b0.
