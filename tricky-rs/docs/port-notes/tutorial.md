# Trick Tutorial (the `trick` course, game mode 6)

Read from SLUS_203.26 by trailrig/heron for board row C11. "Proven" = read from code.

## What the `trick` course is

`FE_SetupRaceFromMenu` 0x2657a8 sets it up: in game mode 6 it writes course `DAT_0032de88 = 10` and one entry (`gRace_EntryCount = 1`) before `Career_PrepareRace`.

Course 10 in the course table is "Trick Tutorial" (`trick`, no show-off time). It is the stage of the **Trick Tutorial**
(game mode 6, the "Lesson" mode): `cGame::Init` creates the lesson manager (`LessonMan_Init` 0x14aaa0, a
`cBxObjNode` subclass `cLessonMan`, vtable 0x36e570) when `DAT_0032de8c == 6`, and the loading screen is
`cTrickTutorialLoad` (0x250068: `StartAssetLoad` 0x24d968 loads the loading-screen sound bank and the icon table 0x2cc;
`PollAssetLoad` 0x24d9b0; `TrickTutorialLoad_SetTitle` 0x24e228). It is neither freeride nor practice: those use the
other courses. (Proven.)

## Data it reads

- `data/tutorial/<rider>.big` (format string 0x31df58, `<rider>` = `gRider_Table[rider][2]`), opened with `BigArchive_Open`.
- `data\tutorial\<board><NN>.inp` (0x31df38; `<board>` from the table at 0x331eb0 by rider, `NN` = trick index + 1): the
  recorded pad input of the demonstration of each trick, played back by the preview. The buffer (0x4d598 bytes) is the
  lesson manager's +0x46c.
- `data/textures/tb_<rider><n><suffix>` (0x31df70): 6 pages x 5 trick pictures per rider (`.ssh`), loaded in the
  constructor into +0x470 .. (30 handles; entry `Shape_GetEntry(sheet, 0..4)` named `%02d`).
- `data/tutorial/trickdef.dat` is the trick book (see frontend.md, scoring.md); the page is `lesson+0x50c` (0..5),
  `TrickBook_PageFirstTrick(page)` gives its first trick; the chosen trick is `lesson+0x4e8`.
- The pad map: `Pad_LoadButtonMap`, the 14 lesson buttons (0x1b..0x28) and 16 stick entries copied to the input
  system at the start (0x17a290, 0x179818/0x179838), restored at exit (0x17a2f8).

## The lesson manager

`cLessonMan` fields (proven): +0xc state, +0x10 requested state, +0x14 previous state, +0x48 current handler, +0x4c
previous handler, +0x50 ticks in state, +0x54 "attempt done" flag, +0x58 menu choice, +0x60, +0x64 "trick input seen",
+0xa0/+0xa4 = 20.0, +0xa8 the archive, +0xac, +0xb0 menu level (0 page, 1 trick, 2 action), +0xb4/+0xb8 list window,
+0x46c recording buffer, +0x4e8 trick, +0x510 rider, +0x50c page, +0x514 a `cGameTrickInfo`.

`vf1` (0x14b9b8, the per-tick Update): does nothing while the HUD dialog (`FUN_001a7c58` +0x42c) or the menu manager is
active; counts ticks in state; a requested state (`LessonMan_SetState` 0x14baa8 stores it at +0x10) runs the switch
`LessonMan_SwitchState` 0x14bb08 (if it differs from +0xc: old state to +0x14, old handler to +0x4c, new state to +0xc
and its handler slot to +0x48; outside 1..11 both 0); then, while +0x14 differs from +0xc, `vf1` calls the old handler's
`Exit`, zeroes the tick count and calls the new one's `Enter`; then every tick the handler's `Update`. Eleven handlers,
one per state (slots +0x18..+0x44, 0 is none):

| State | Class | Meaning |
|---|---|---|
| 1 | cLessonInitHandler | set up |
| 2 | cLessonIntroHandler | intro |
| 3 | cLessonStartHandler | start |
| 4 | cLessonWaitHandler | the menu (pages, tricks, actions); Update 0x14dcc0 |
| 5 | cLessonPreviewHandler | demonstration: plays the `.inp` (Enter 0x14e138: `Lesson_SetupRider`, `Speech_LessonLine(1)`, sound, load the `.inp`, HUD item 10 hidden, 0xe shown) |
| 6 | cLessonPreviewEndHandler | after the demonstration |
| 7 | cLessonPerformHandler | the player tries the trick (`Speech_LessonLine(2)`, clears the replay) |
| 8 | cLessonPracticeHandler | free practice: Update 0x14e7c0 goes to state 9 when the attempt is done (+0x54), sets +0x64 when the trick text is non-empty |
| 9 | cLessonPracticeEndHandler | |
| 10 | cLessonEndHandler | result menu: 0 retry (reset scene, menu level 1), 1 `LessonMan_EndAttempt`, 2 exit |
| 11 | cLessonExitHandler | leave (`Camera_ForceCutFade`, hides HUD item 10) |

`LessonMan_EndAttempt` (0x14b638): reset the scene, hide HUD 10, show 0xe, then next state by the current one: 5/7/8
-> `Lesson_SetupRider`; 6 -> 5; 9 -> 8; 10 -> 7; else 5.

The Wait menu (proven): menu level 0 = pages (left/right through 0..5), 1 = tricks of the page (window of 5), 2 = actions
0 preview (state 5, `.inp` of the trick), 1 perform (7), 2 practice (8), 3 exit with a three-string dialog
(strings 0x146, 0x1d6, 0x188) then state 11. The buttons are the masks 0x100008 (up), 0x80004 (down), 0x800 (accept),
0x20 (back) from `Lesson_HandleMenuInput` (0x14d6e0).

`Lesson_SetupRider` (0x14b768): clears +0x54, +0x64, +0xac; for page 5 (Master) sets the rider's speed fields to
(1.0, 20.0), else 0, 0. `Lesson_ResetScene` 0x14d7d0, `Lesson_RestartRider` 0x14d898.

Overlays: `cLessonOverlay`, `cLessonControllerOverlay` (0x1be4a0..); the results page for a lesson is
`Results_BuildLessonPage` (0x1b1aa8: strings 0xe47 title, 0x191 and 0x18e as the two rows). `Boarder_StartLessonWait`
0x11afe8, `Speech_LessonLine` 0x231398.

## Handlers, all read (C11b)

Ported as `tricky-rs/game/src/lesson.rs` (`Lesson`, `Env`; unit-tested against the decompiled flow, not runner-checked).
Flow: Init (1, set by the constructor, handler does nothing) -> Intro (2: speech line 0, camera script `RaceInBlocks2`,
HUD 10 shown; Update requests Start) -> Start (3; Update waits for `CamScript_IsRunning` to end, then Wait) -> Wait (4).
From Wait: Preview (5) loads `.inp`, swaps in the lesson pad map (restored in its Exit), speech line 1, ends when the
recording is played (`rec[0] >= rec[1]`) in PreviewEnd (6, menu 0x22c 0x195 0x191 0x18e: retry list, perform, end
attempt, exit). Perform (7): speech line 2, clears the replay; Update waits for the attempt-done flag, then End (10); Exit
speaks line 4 if a trick was seen (+0x64) else 5. Practice (8): `Rand_Fx % 3` picks a camera from {577, 579, 306} (+0xac),
ends in PracticeEnd (9, menu 0x22c 0x191 0x18e). End (10): same three-item menu, panel y 330.0 (PreviewEnd/PracticeEnd 240.0,
Wait 435.0). Exit (11): sets the game object's done flag (+0x54), hides HUD 10 and 0xe.

## The recorded input (C11c)

Read from `cPlayer::GetControls` (calls `LessonMan_PreviewLoadPad` 0x14b7c8 before and `LessonMan_AdvanceAndRestorePad` 0x14b8a8 or `LessonMan_RestorePad` 0x14b958 after the controls
when the lesson manager exists), `cLessonPreviewHandler::Enter/Update`. Ported as `tricky-rs/data/src/inp.rs` (`Inp::parse`,
`Playback`; tested on synthetic bytes, no real `.inp` is in the inputs).

- The 0x4d598-byte buffer is a six-word header (word 0 position, zeroed after loading; word 1 frame count; words 2..5
  unread) and up to 7200 records of 44 bytes (24 + 7200 x 44 = 0x4d598): a 12-byte control record, then the 32-byte state of
  the pad device (previous buttons, buttons, six stick floats, read by `Pad_ReadAxis`).
- Preview (state 5): before the controls are read the pad's state is saved (to lesson +0xbc, 32 bytes) and replaced by the
  frame at the position; after, the position advances while below the count and the live state is put back. Perform (state 7)
  only advances the position. The preview ends when the position reaches the count.
- The game's own replay system is a different thing: `Replay_*Input` keep run-length coded input words
  (`Replay_ReadInput` 0x192998: 0x1999 words at +0, run counts at +0x6664, count +0x8000, play index +0x8004).
- The recorder (`InputRecorder_Playback` 0x17a740, `Input_ReadDevices` 0x17a668) is a ring of device states per frame
  (frame size [1], capacity [2], read [3], write [4], devices from [6]): how the `.inp` files were made is probably that.

## The race's lesson states (C11b2)

The race object (`gApp+0x730 -> +0x1c`) is a state machine too: `Race_CreateHandlers` 0x1125c8 names 1 PreRace, 2 StartRace,
3 Countdown, 4 Race, 5 EndRace, 6 LoadLesson, 7 PreLesson, 8 RestartLesson, 9 SelectLesson, 10 EndLesson, 11
ReplayEndRace (handler slots +0x24.. and +0x48..+0x5c, the name at +0x68, the state at +0x1c; slot +0x54 of its vtable is the
state change). In the tutorial:

- `cLoadLessonHandler` (6): Enter sets the load flag (+0x4f8); Update runs the texture loader (`FUN_00112920`) and when it
  is done: `Lesson_RestartRider`, lesson state Intro, race state 7. This is who leaves the lesson's Init state.
- PreLesson (7) and SelectLesson (9) do nothing: the lesson manager is in charge.
- RestartLesson (8): Enter puts the lesson in Intro (2); Update moves the race to 9. The lesson's retry choices call it.
- Countdown (3): what `set_mode(3)` is: the race clock counts up 2.0 a tick (to 0), then Race (4) and the rider rides.
- EndLesson (10): clears the pad device's +0x2c; Update adds 1/60 to the race clock.
- Names (F1c, proven): `Race_GoToEndLesson` 0x1117f0 is the race object's state change (vtable +0x54) to 10, from the
  three lesson end handlers' Enter. `Lesson_SetAttemptDone` 0x14d930 sets the lesson's +0x54 to 1 when it is 0, on the
  rider's finish event (10, `cBoarder::vf10`); `Lesson_OnBoarderEvent` 0x14d928 (event 0x10) is empty.

## Checked against the original (row C11b3)

`tests/lesson.rs` runs the real `cLessonMan::vf1` and handlers in the function runner with the outside stubbed and
logged, against `lesson.rs` on a scripted `Env` (200,000 random steps). It found one difference, now fixed: the Wait
menu's refresh (`FUN_0014daa8`) at the page and trick levels does not clear the string list, only sets the first
string (the action level clears all seven), so the strings left from an earlier menu stay. Also read there: the
Practice Enter writes 0 to byte +0x5a70 of the game's player object (not modelled), and `Replay_Clear` runs only
when the game object has a replay (+0x40).

The race side (row C11b4, `lesson::RaceLesson`, `race_lesson_modes_match_the_original`): modes 6 to 10 of `cAI`
(`Race_CreateHandlers` 0x1125c8 makes the handler for a mode; `vf9` 0x1125a0 notes the mode the handlers last ran for
and the handler before it; `vf1` 0x1126f0 runs `Exit`, `Enter`, `Update`). One quirk: LoadLesson's Update goes to mode 7
with `Race_CreateHandlers` itself, not `vf9`, so nothing is noted and the next tick enters PreLesson with no `Exit`
before it; RestartLesson's Update goes to 9 through `vf9`. 120,000 steps of random mode requests matched.

## Not read yet

- `Lesson_ResetScene` 0x14d7d0's callees, the contents of `data/tutorial/<rider>.big` (C11c2), the overlays and the load
  screen (C11d), the texture loader `FUN_00112920`.
