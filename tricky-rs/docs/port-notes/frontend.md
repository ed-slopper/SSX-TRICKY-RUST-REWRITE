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
