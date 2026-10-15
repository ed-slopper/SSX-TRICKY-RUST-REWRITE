# HUD and fonts: how the original works

Part of the port notes (index: [README.md](README.md)); units and wording as described there.

## HUD (`HUD_DrawPlayerSprites` 0x1a2c40, `HUD_DrawPlayerText` 0x1a4f70; 640×480 virtual)

- Place (66,27) ×1.9, gold (0.953,0.729,0.106) when 1st; time (26,460) m:ss.cc; speed (90,430) km/h; score
  (620,24) green while pulsing. Boost meter: 14 segments at (570,140) in three bands, fill slews 0.005/frame.
- Trick name: gold (1,0.867,0), centred 38 px above the bottom, 3 s, no fade, replaced by the next. Pending points
  (320,80) tinted by size (green ≤1000, blue, yellow 2501+, orange 4001+, red 7501+, navy 11501+). Banked points fly
  to the score over 2.5 s. Multipliers ×2/×3/×5 at (320,110). Combo and air bonuses 2.2 s. Crash shows nothing.
- Split (5 s): label then ±m:ss.cc (red behind, green ahead). Knockdown 2.1 s. No wrong-way indicator.

## HUD sprites (`HUD_DrawSprite` 0x1c0068, `SpriteSet_InitHudGameRects` 0x1f4440, `SpriteSet_BindHudGameTex` 0x1f9080)

- 200 records {texture, draw w, draw h, v0, u0, u1, v1} written in code from immediates for HUDGAME.SSH (map1,
  map4, hud1; HUDTRICK.SSH in the tutorial). Boost segments 0x12–0x14 (red, orange, yellow) / 0x15 unlit,
  TRICKY letters 25–30 unlit / 31–36 lit, uber orb 0x25 / 0x26, medals 0x27–0x29, character badges 0x45–0x51,
  button glyphs 0x6c–0x75. Text uses data/fonts/title.sfn (shadow +2, +2) and menu.sfn.

## Fonts (DATA/FONTS/*.SFN)

- `FNTS` files: header (version at 0x08, glyph count 0x0a, origin 0x10/0x11, glyph table 0x14, kerning 0x18, bitmap 0x1c), glyph records `{u16 code, u8 w, u8 h, u16 x, u16 y, s8 advance, s8 xoff, s8 yoff}` (+1 pad from version 200), a 4bpp linear bitmap with a built-in ramp palette (alpha i*128/15).
- Only TITLE.SFN and MENU.SFN are loaded (SMLFONT is never used). After loading the game scales them: title 1.4 x 1.3, menu 1.8 x 1.4.
- `Font_DrawStringA` 0x19d0c0: quad at pen + offset (both times scale), pen += advance; no kerning (the table exists but the loader never reads it), no extra spacing, missing glyphs skipped without moving the pen.
- Width for alignment (`Font_MeasureA`) is the glyph boxes' extent, not the advances.
- `Font_PrintfShadowedA` 0x19d568: black copy at +2,+2 (screen units, not scaled), same alpha, one layer below.
- In-race text (`HUD_DrawPlayerText` 0x1a4f70, 640x480): place number title x1.9 right-aligned at (66,27), suffix x0.7 at (71,30), gold (.953,.729,.106) in the lead; time x0.9 at (26,460) anchored at its bottom; points x1.1 right-aligned at (620,24); speed x0.9 right-aligned at x 90, y 430 with " km/h" at x0.7; multiplier "xN" x1.9 centred at (320,110) - x2 gold, x3 orange, x5 red; trick names in MENU font, centred at x 320, bottom 38 above the screen's foot, gold (1,.866,0), wrapped at 0.4 of the screen width, lines stacking upward.

## The overlay manager (G8b)

`tricky_game::overlays`, on the game's bytes (`OverlayManEnv`: virtual calls with their integer and float arguments,
direct calls, the heap, the pads).

- **`cOverlayMan`** holds nineteen overlays at +0x10..+0x58. In order: the HUD, results, high-score record, next
  race, countdown, the replay's controls, pause, pop finish, pop async load, the replay save page, lesson, view high
  scores, black fade, unlocked, lesson controller, point distribution, next venue, next event, race help.
- **Every overlay starts as a `cScreenOverlay`** (vtable 0x38fc10):
  - +0 the sprite set (the game's +0x728), +4 / +8 the fonts (+0x60 / +0x5c);
  - +0xc the players it shows for: a bit each, or -1 for all;
  - +0x10 drawn with the HUD, +0x14 active, +0x18 cleared on hiding, +0x1c / +0x20 the repeat timers.

  `cBackdropOverlay` (0x38fb78) adds its animation at +0x94 and its defaults.
- **`OverlayMan_Init`** builds each overlay in turn:
  1. allocates it from the manager's heap (name 0x38e2b0) and fills it with 0xdeadc0de;
  2. constructs it: by its constructor, or as a screen / backdrop overlay with its own vtable;
  3. takes the overlay at the object's +0, +4 → +0x42c or +0x42c, depending on the class;
  4. sets its two flags.

  The three save pages (point distribution, next venue, next event) build their backdrop inside themselves, then
  the save overlay, then their two vtables. Last come the HUD's sprite rectangles and the overlays' textures: 0x2ca
  in game mode 6 (0x32de8c), else 0x2cb.
- **Hiding** (`Overlay_HideForPlayer`): the player's bit goes off.
  - An overlay that hides itself (its +0x94 true) gets its +0x8c call when the bit was on.
  - Any other gets it when no players are left.
  - +0x18 is cleared.

  `Overlay_HideAll` and the destructor hide them all; the destructor then deletes each (its +0xc with 3).
- **Drawing** (vf2):
  1. The renderer's state is kept (+0x1dc), and the renderer goes into screen space at 640 × 480. The font's fifth
     word (0x337ff8) is set to 0.15, and its +0x1cc gets 0.08 with 2, 2.
  2. Every overlay's items (layer 4), then every overlay's text (layer 9).
  3. An overlay draws when its bit for the view being drawn (the race's +0x290) is set. One shown for all players
     (-1) draws only in the last view, across the whole screen. The viewport takes the renderer's size: its +0x5c /
     +0x64, converted as unsigned.
  4. The renderer goes back.
- **Updating** (vf1) runs each shown overlay's +0x7c. The `OverlayMan_Call*` wrappers each call one overlay's
  virtual; `_CallAllVf10` / `_CallAllVf11` call every overlay's +0x5c / +0x64.
- **`Overlay_GetNewButtons`** (a pad, or -1 for the first player pad with a button down):
  - returns the buttons newly down and marks them as held;
  - a new press restarts the overlay's repeat timers;
  - the pad becomes the one the overlays read (0x3381e0).
- **Checked in the runner** on 36,000 cases: a manager over random overlays on fake vtables, with two different
  `this` adjustments. Every virtual and direct call, its integer and float arguments, and all memory were compared.
  The tests caught 6 of 8 mutants; the two left cannot change a result (bit 0 of an arithmetic or a logical shift,
  and `last | (held & !last)` = `last | held`).
- **The runner stubs must return sign-extended words,** as the EE's 32-bit results are. The viewport size is
  converted signed or unsigned by the 64-bit register's sign.

## The backdrop's animation (G8c)

`tricky_game::overlays`. A backdrop overlay moves in three parts, each with a set of bits (1 still, 2 fading in, 4
/ 8 from the right / left, 16 / 32 from below / above, 64 opening).

- **The frame** (+0x60):
  - its alpha +0x50 rises by +0x54 (0.034 a frame) to +0x58;
  - its x +0x38 starts at ±640 and moves by +0x3c (∓90), which itself changes by +0x40 (∓5) and is held at +0x44
    (±10);
  - its y +0x48 starts at ∓480 and moves by ±20;
  - each stops at 0;
  - after a delay of +0x5c frames, its width +0x2c grows from 60 by 13 to 500.
- **Its contents** (+0x7c): after a delay of +0x78 frames, the alpha +0x70 rises by 0.034 to 1.0 and the x +0x64
  slides in. From the left it starts at -640 moving 10. From the right it starts at 640 moving -90, slowing by 5.
  The x goes to both fonts' +0x20, the first font's as it stood after the stop test.
- **The edges** (+0x90): two offsets, -850 rising by 25 and -680 rising by 16, each to 0.
- **`cBackdropOverlay::Update`** steps the screen animation (`ScreenAnim_Update` at +0x94), then each part not
  still. `cBackdropOverlay::Reset` and `cScreenOverlay::Update` are empty.
- **Checked in the runner** on 60,000 random backdrops, over every start and step with random bits and values around
  each limit, with all memory compared. The tests caught 6 of 8 mutants; the two left cannot change a result (a
  reload of the value just stored, and `<=` at a limit writing the same value).

## The screen animation, the menu sounds and the repeat poll (G8d)

`tricky_game::overlays`.

- **A screen animation** (`ScreenAnim`, in a backdrop at +0x94) is a frame counter and twenty tracks.
  - Header: +0 the frame, +4 the tracks still running, +8 the tracks set up, +0xc done, +0x10 the last frame.
  - Each track: +0 id (-1 free), +4 the start frame, +8 a counter, +0xc the target, +0x10 the rate, +0x14 the
    value, +0x18 the base, +0x1c the kind.
- **A step** adds to the value:
  - by one every `rate` frames for rates above 1;
  - by the truncated 1 / rate each frame for rates in 0..1;
  - by one each frame for kinds 2 and 3.

  It reports when the value reaches the target.
- **The kinds evaluate as:**
  - 0 base + value; 1 base − value;
  - 2: halfway between base and target (in thousandths), plus half their difference times sin(value / rate
    turns), with `Math_Sin`; 0 while the value is 0;
  - 3: base to target as the value runs up to the rate, held after.
- **The update** runs the set-up tracks: kinds 2 and 3 from their start frame on, the others only on it, counting
  off the running ones as they finish. When none is running and the animation isn't done, it moves to the next
  frame, starts the tracks on it, and marks done once past the last frame with nothing running. The backdrop's set-up
  and update now run this port.
- **The menu sounds** (`Overlay_PlayTickSound` .. `_PlayErrorSound`) are tail calls into the audio system (0x343050):
  tick 0x213b38, move 0x213d60, back 0x213f88, select 0x2138f0, error 0x2141d0.
- **`PauseMenu_PollRepeatButton`** gives the held button: when it changes, then every 12 frames, then every other
  frame past 60.
  - With pad −1 it searches the player pads. A `beq` delay slot records every pad found with a button held, so the
    pad holding the last button wins, and otherwise the last pad found.
- **Checked in the runner** on 60,000 cases over every function, with random animations (kinds 0..4, rates around
  and between the integers), counters near the repeat's thresholds and every call compared, plus the backdrop test
  with its animation run for real. The tests caught 7 of 8 mutants; the one left (≥ 60 for ≥ 61) cannot change a
  result, since 60 is a multiple of 12.

## The HUD overlay's state (G8e)

`tricky_game::hudstate`: what feeds `HUD_DrawPlayerSprites` / `HUD_DrawPlayerText`.

- **The split markers** (`HUD_BuildSplitList` 0x1a1968; `cHUDOverlay::Reset` and vf16 both jump to it):
  - entry 0 is the start (kind 1 at distance 0);
  - each race path's markers of kind 12 (shown as kind 0) and 13 (kind 2) sit at the path's length less the
    marker's distance;
  - in lap mode 8 they repeat for three laps of the main path: laps 0..2 for kind 12, 1..2 for kind 13;
  - a marker within 500 of one of the same kind is skipped (`HUD_HasSplitAt`).

  Each player's 0x140-byte block is then set up: the band colours unlit, the fades at 1.0, the markers' distances
  ahead from its rider's course length (+0x380).
- **The update** (`cHUDOverlay::Update` 0x1a1f18), after the HUD's flags from the HUD option (0x32ca40), and
  nothing while an overlay is active:
  - in time trials and races (modes 3 / 5) each of the last ten whole seconds beeps once, with the tenth spoken,
    for the first player still racing;
  - the blink counts 0..53 and is on from 19 (while the rider is in state 19 or not finished);
  - the score shown rolls up to the points in steps of 50, with a tick each time its hundreds roll over (HUD flag
    0x10000). The old score goes through a float and back for that test;
  - the meter and the multiplier shown ease toward the rider's by 0.005 a frame;
  - the meter's 14 bands each blend from unlit to lit as the meter (else the multiplier plus the meter) fills
    them, a 14th each. In the tutorial's fifth lesson the meter is the rider's and, from 20 up, every band is lit;
  - the wobble (+0x84 / +0x88) comes from a running trick timer (kinds 2 / 3 of the rider's twelve at +0x597c):
    `HUD_WobbleOffset` is sin(4π x) × (1 − x) / 2 × 1.4 over the first quarter;
  - the speed flash fires when the speed reaches 3319.44. The speed is the length of the 4-word velocity at
    +0x150, w included, computed on VU0 (x² + y², then + z², then + w², then the square root);
  - the riders' progress bars and the markers ahead ease toward the rider's progress by a 36th;
  - the HUD fades in by 1/180; the lap change shows; and in time trials the three medal times each start a flash
    as the rider's checkpoint count passes them, in order.
- **`Draw` / `DrawText`** draw nothing while the replay page, results or pause are up, or during a cut-scene other
  than 5, 7 or 8. Otherwise each view is scaled to 640 × 480 (the renderer's size, as unsigned) and its player's
  sprites / text are drawn, the text on layer 5. `HUD_SetViewLayout` picks the layout: 0 for one view, 1 / 2 for
  two side by side.
- **Checked in the runner** on 40,000 random cases over the reset, update and draws, with every call (floats bit
  for bit) and all memory compared. The tests caught the 10 mutants tried.
- **The test fills its float fields with real floats.** The EE reads a denormal as 0, so random small integers in
  a float field make `0 < x` differ from Rust's; no HUD or rider float is ever denormal.

## The menus' shared drawing (G8f)

`tricky_game::menudraw`: the frame, the list and the label box every menu overlay draws, as their draw calls.

- **`Backdrop_DrawFrame`** (the height, the backdrop, the layer):
  - stores the height at +0x2c (or +0x34 while the frame opens);
  - draws three dark panels (the frame's alpha +0x50, grey 0.1): one 680 wide behind it, and two inside between
    0.26 and 0.79 of sprite 55's height, from 0.455 of the scaled width;
  - draws the four corners of sprite 55, scaled to 340 wide, at the backdrop's x (+0x38) ± and y (+0x48);
  - the font's fifth word goes to 0.15 first.
- **`Overlay_DrawMenuList`** (x, y, the scale; the backdrop, the alignment, the title, the items, their count, the
  selection, the pointer sprite; on the stack: the pulse on, the items' enabled flags):
  - the line height is the font's height × scale + 4;
  - flags 2 / 8 put the list above / centred on y, counting the title (its height × 1.2 + 9) when there is one;
  - flags 1 / 4 put it right of / centred on x by the widest item;
  - the title is centred at scale 1.2; the pointer sprite sits 16 left of the selection, at 0.9 scale;
  - items are white, dimmed to 0.4 when disabled. The selection is gold and pulses: its scale bounces between 0.99
    and 1.1 (0x338208, stepping by 0x33820c), keeping its centre;
  - each text's alpha is the contents' (+0x70).
- **`HUD_DrawLabelBox`** (y, the backdrop, the string, the layer): sprites 62 / 63 / 64 at the backdrop's edge
  offsets (+0x80, +0x88 plus y). The middle one is stretched to the text's width + 8 at scale 0.79. The text sits 2
  left of the middle and 7 down, on layer + 2.
- **Checked in the runner** on 40,000 random frames, lists (every alignment, with and without a title, pointer
  and pulse) and labels, with every draw call (floats bit for bit) and all memory compared. The tests caught 5 of
  8 mutants; the three left cannot change a result for these inputs (an equal width, (w − 2) + 10 for integer
  widths, and selection 0's offset of 0).

## A player's HUD sprites (G8g)

`tricky_game::hudsprites`: `HUD_DrawPlayerSprites` 0x1a2c40 (the view's top and left, its width; the HUD, the
player), every sprite, number and caption of one view as the game's draw calls.

- **Placement.** Everything is placed from the view's layout table: 0x424 bytes per view from 0x33ca40, filled at
  run time by `HudOverlay_InitTables`. An entry reads +0 x, +4 y, +8 the corner, +0xc / +0x10 the scales.
- **Alpha.** The alpha is 1.0 while no overlay is visible and the rider is racing (+0x418 < 0), else 0.5. Layer 6,
  then each part is gated by a HUD flag (+0x104).
- **The meter and the parts drawn with it:**
  - **0x1000000:** the speed wobble's sprite (+0x94 + 38), grown by `HUD_WobbleOffset` at 0.7.
  - **0x1000, the meter:**
    - drawn on layer 8;
    - fourteen bands (the table at 0x338008, 20 bytes each: +8 the lit sprite, +0xc / +0x10 the offset). A band is
      sprite 21 when unlit; when lit it is its own sprite, scaled by the wobble scale (+0x84);
    - a band's height is its colour (+0xc + 8k), and its width is that times the scale;
    - the whole meter is at 0.5 alpha when band 0 sits at the table's unlit value;
    - then the end cap (sprite 23).
  - **0x2000, the tricky orb** (sprites 37 / 38), drawn while the rider's tricky (+0x24) is above 0:
    - it rises 40 × (the tricky / 19.3, or within 0.7 of 20, (20 − tricky) / 0.7), kept at +0xac;
    - its width swings through +0xb0, which falls by 0.15 × (1 − rise / 40) each racing frame and folds back past
      −1 / −2 / −3;
    - it marks +0xa8.
  - **0x8000, the six TRICKY letters:** the letter entries are at 0x338198 (12 bytes each: the layout entry, the
    unlit sprite, the lit sprite). The first (rider +0x58a8) letters are lit; +0xc0 marks them shown.
  - **0x4000, the combo stars:** one to five stars (sprites 13–17, the switch falling through) for
    (+0x88 × 14) mod 7. They are drawn only when neither the orb nor the letters showed. Their colour follows the
    last lit band's sprite: 0x12 red, 0x13 orange, else yellow.
- **0x80** the lap sprite (24) while +0xa4 is set.
- **0x100, the progress track:**
  - on layer 10; the track is sprite 3 for views 0 / 1 and sprite 2 for view 2;
  - other riders' dots sit at the bar (+0xc4) / 20000 × 150 along it. When a rider's +0x374 < 0, the dot uses the
    *drawing* rider's `Boarder_GetRaceProgress` (the game passes its own rider);
  - a dot fades over the last half of the range;
  - the sprite and layer depend on the rider's character byte +0x65: layer 8, sprite 5 / 6 when it is negative;
    otherwise layer 9, sprite 1 / 4 (none in view 0);
  - then the split markers (layer 7) by kind: 1 is sprite 7 and never ahead, 0 / 2 are sprite 8. A marker fades
    from 0.25 ahead or 0.5 behind;
  - then the three medal sprites (9 / 10 / 9) when their width is positive.
- **After `OverlayMan_IsAnyActive`, the captions.** These come from the rider's twelve score entries at +0x597c
  (20 bytes: kind, points, time shown, time so far, rank points), each faded by 1 − time so far / time shown:
  - **kind 15 (0x4000000):** the tier name (Loc 0xeb3–0xeb6 for tiers 2 / 3 / 4 / 5+; tiers 0–1 print whatever is
    left in the buffer) and the points ("%d" at 0x38da30), centred at 175 / 225 down, in gold;
  - **kind 16 (0x8000000):** Loc 0xe94 and the points, 100 lower when a kind-15 caption showed;
  - **kind 4 (0x10000000)**, unless both of those showed: Loc 0xe93 at 210 (300 after either);
  - **kind 17 (0x40000000):** sprite 53 at its own layout entry, with no view offset.
- **0x10000, the score number** (`HUD_DrawNumber`):
  - with flag 0x2000000 it sits in a rank frame: the tiled middle 58 and the ends 56 / 57, coloured by
    `Score_GetRankForPoints`;
  - when the rank changes the number swells (+0xb8, up to 0.8 × its scale over 18 frames and back over 36);
  - points ≤ 0 reset the rank to −1;
  - while racing, the trick scores of kinds 2 / 3 fly from the score toward the layout's +0x50 entry along
    r² + r, shrinking. Kind 3 is pink. They get their own rank frames;
  - kind 1 is a plain number that fades; kind 14 is the caption Loc 0xe41 growing by 0.75.
- **0x400000:** captions of kinds 11 (Loc 0xe59, cyan) and 10 (Loc 0xe40) over the first half of their time.
- **0x800000:** the countdown (the world's +0x14 between 1 and 11) as a number in red, its alpha and growth from
  the fraction, unless the rider has finished and stopped or is not racing.
- **Checked in the runner** on 30,000 random cases:
  - the inputs: every view layout, flag word, HUD block, rider and score entry;
  - compared: every call (floats bit for bit), and all memory, including the frame's colour vector and buffers.

  - every draw call also logs the font colour it would read.

  The tests caught 77 of 82 mutants. The five left cannot change a result:
  - 20 − tricky can never be exactly 0.7 as a float;
  - at s = −1 both of the width's folds agree;
  - at n = 5 the star count is 5 either way;
  - at a dot's or a marker's fade edges (0 and 1) both alphas agree.
