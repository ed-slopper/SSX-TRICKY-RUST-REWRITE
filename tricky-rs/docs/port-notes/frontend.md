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
