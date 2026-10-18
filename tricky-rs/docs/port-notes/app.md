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
