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
  - in show-offs (modes 3 / 5, which run against the clock) each of the last ten whole seconds beeps once, with the tenth spoken,
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
  - the HUD fades in by 1/180; the lap change shows; and in show-offs (modes 3 / 5) the three medal scores each start a flash
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

## A player's HUD text (G8h)

`tricky_game::hudtext`: `HUD_DrawPlayerText` 0x1a4f70 (the view's top, left, width and height; the HUD, the
player). It draws every number and caption of one view as the game's calls, on layer 7. Text is placed from the same
layout tables as the sprites; a text entry's +8 flags 0x20 / 0x40 centre it on x / y by its measured size.

- **The alpha** is 1.0 while no overlay is visible and the rider is racing, else 0.35. The sprites use 0.5 here.
- **0x200, the race place:**
  - the place (+0x110, 0 first, + 1) via `HUD_DrawNumber`, gold in the lead;
  - its ordinal, Loc 0xe44 / 0xe54 / 0xe58 / 0xe45 ("st" / "nd" / "rd" / "th") for 1st–4th. These ids are copied to the frame from 0x38da60 and
    capped at the 4th.
- **0x1, the race time** (`HUD_FormatTime` of `Boarder_GetRaceTime`), hidden in game state 3:
  - in view 1 a rider who has finished shows the other player's time if it is longer;
  - the alpha is full while that player still races and no overlay is visible.
- **0x20000000, the gap** (views 0 / 1): the clock at 0x32de98 less the leading player's race time, signed
  (`HUD_FormatTimeSigned`):
  - green while ahead, red otherwise;
  - in the last ten seconds it pulses, by 0.7 × a triangle wave of the second's fraction;
  - in view 0 it is hidden once the player finishes.
- **0x800, the speed:**
  - the velocity's length as in G8e (VU0: x² + y², then + z², then + w², square-rooted);
  - × 0.0224 for mph (0x32ca10 set; suffix at 0x38da70), else × 0.036 for km/h (0x38da78);
  - the number is right-aligned against its unit, and both grow with the speed flash's wobble (scale 1.5).
- **0x200 without 0x1000000:** "/" and the riders' count as one byte ("/4").
- **0x400:** the first kind-13 score entry, as Loc 0x115 (place 2) or Loc 0x116 formatted with place − 1.
- **0x8:** the multiplier (rider +0x5910) scaled by the wobble scale (+0x84), green unless that is exactly 1;
  right-aligned when the entry's flag 8 is set.
- **0x100000:** the first kind-12 entry as "X" and its digit, coloured for 2 / 3 / 5.
- **0x20000:**
  - the trick message from `FUN_00155538`, wrapped (`Font_PrintWrapped`) to 0.4 of the view's width;
  - with no message, no cut-scene, tricky above 0, a career level of 8 or more and the rider racing: the tricky
    prompt. That is Loc 0xdf2 when the world's flags have 0x20, then the tricky meter object's two virtuals (via
    0x1a7c58 + 0x7f8: set up, then draw at the view's centre). The meter is placed 0.16 / 0.09 of the height up, or
    0.2 / 0.14 in two-player races. Its alpha is the tricky / 5 (full while tricky).
- **0x400000:** the first kind-11 or kind-10 entry, over the second half of its time:
  - kind 11: Loc 0xe29 with the points, in green;
  - kind 10: the split time with a "+" (green) or "−" (red).
- **0x100, the split markers' names** (Loc 0xe22 / 0xe23 / 0xe27 for kinds 0 / 1 / 2):
  - faded as the markers' sprites are;
  - left of the track in view 0 (20) and view 1 (their width + 3), right of it in view 2;
  - their y is the track's layout offsets alone. The game leaves out the view's top here, and the port keeps that.
- **0x1000000** (show-offs, one rider): the medal text:
  - Loc 0xe28; the next medal's name and time (0xe32 / 0xe26 and the HUD's +0xfc; 0xe3b / 0xe25 and +0xf8; 0xe35
    for the last), or the gold time (+0x100);
  - shown wobbling (0.7) while its flash runs, else placed at the layout's +0x2f8 / +0x30c entries.
- **The pause prompt** (Loc 0x1b8) is shown 30 above the view's bottom, centred, while the race's +0x40 object is
  live, not done and counting.
- **Checked in the runner** on 30,000 random cases:
  - the inputs: every view, flag word, layout, rider, score entry and race state;
  - compared: every call (floats bit for bit; the font colour and the string's first bytes with each print and
    measure) and all memory, the frame's strings, sizes and colour vectors included.

  The tests caught 68 of 72 mutants. Of the four left:
  - two cannot change a result: lap 4 caps the same either way, and re-reading +0x84 returns the same value;
  - two change a constant's last bit (the mph factor, the two-player meter height), which rounding absorbs.
    Coarser changes to both are caught.
- **Float fields in the test hold real floats.** The layout tables' flag words are floats with their low bits set:
  a small integer there would be a denormal, which the EE reads as 0.

## The HUD's drawing helpers (G8i)

In `tricky_game::hudsprites`, under the two big draws:

- **`HUD_DrawSprite`** (x, y, the scales; the HUD, the sprite, the corner) draws the HUD's sprite set's (+0)
  rectangle with `Sprite_DrawRectScaled`. `SpriteSet_GetRect` hands out the 28-byte rectangle at +8 + 28 × id, or
  0x3d2060 for a negative id. `HUD_GetSpriteRect` is that on the HUD's set.
- **`HUD_DrawBar`:**
  - flag 1 stretches sprite 42 to w × (h − 0.5);
  - flag 2 raises the renderer's layer by one.
- **`HUD_DrawNumber`** (x, y, the scale, the roll; the HUD, the number, the flags) returns its width. It prints with
  the HUD's font (+8):
  - flags 0x20 / 0x40 centre it on x / y; otherwise 8 / 4 right- / bottom-align it;
  - with flag 0x80 it measures each digit, and draws them in equal cells of the widest (each centred in its cell).
    Each digit drops 200 × the roll × (2 + its place xor its digit, halved past 4), so rolling digits stagger.
- **`HUD_FormatTime`** (hundredths) gives:
  - Loc 0x15d for exactly 50000 (500 s, no time);
  - Loc 0x18c when negative;
  - the no-time string (0x38e280) from an hour up;
  - else m:ss.cc.

  **`HUD_FormatTimeSigned`** uses the same rules, with a minus format for a negative time.
- **`HUD_ShowItem` / `_HideItem`** forward to the item at +0x10 + 4k.
- **Checked in the runner** (tests/hudhelpers.rs) on 60,000 random calls:
  - the inputs: inexact floats, negative and out-of-range ids, every flag, and boundary times and numbers;
  - compared: every call out, the results, and all memory including `HUD_DrawNumber`'s frame.

  The tests caught 25 of 26 mutants. The one left cannot change a result: a tie in the widest digit keeps the same
  width either way.

## The HUD overlay's constructor (G8j)

`tricky_game::hudstate::hud_overlay_new`: `HudOverlay_InitTables` 0x19e888 (named for what most of it does; it is
`cHUDOverlay`'s constructor, called from `OverlayMan_Init`).

- **The base and the vtable:** `ScreenOverlay_Init` first, then the vtable 0x38da80.
- **The meter's fourteen bands** (0x338008, 20 bytes each):
  - each band is a 14th, summed in floats;
  - it is sprite 20 below a third (0.3336), 19 below two thirds (0.6660), then 18;
  - the first is at 288, each next one 18 above the last, or 28 where the colour changes.
- **The split markers** (`HUD_BuildSplitList`) and the race clock (+0x614, the world's +0x14).
- **The view layout tables** (three views of fifty-three 20-byte entries from 0x33ca40) are copied from static
  initialisers in the executable (24 bytes apart, from 0x38d060). The order is:
  - view 0;
  - view 0 copied whole to views 1 and 2;
  - view 1's differences;
  - two entries zeroed through a stack temporary (one with +8 = 0x20);
  - view 2's differences.

  The port keeps them as 26 runs of (entry, source, count) and reads the sources from the game's memory.
- **The show-off medals' points** (+0xf8 / +0xfc / +0x100: gold, silver, bronze; the same as `courses::MEDALS`) by
  course (`DAT_0032de88`). A course without them keeps whatever was there:

  | Course | Gold | Silver | Bronze |
  |---|---|---|---|
  | 0 Garibaldi | 55,000 | 40,000 | 25,000 |
  | 1 Snowdream | 95,000 | 65,000 | 35,000 |
  | 2 / 3 Elysium Alps / Mesablanca | 225,000 | 150,000 | 75,000 |
  | 4 Merqury City | 275,000 | 175,000 | 125,000 |
  | 5 Aloha Ice Jam | 175,000 | 115,000 | 75,000 |
  | 8 Tokyo Megaplex | 350,000 | 225,000 | 100,000 |
  | 11 Alaska | 500,000 | 300,000 | 150,000 |
  | 6 Pipedream in a show-off (mode 3) | 800,000 | 500,000 | 250,000 |

- **The HUD's flags for the three HUD options** (+0x108 / +0x10c / +0x110):
  - they start at 0xb000 / 0x4002b9c0 / 0 (0xb400 / 0x4002bdc0 on course 8);
  - more than one player adds 0x200 / 0x300;
  - the tutorial (mode 6) clears 0x40000000;
  - by game mode:
    - 1 / 6 clear the lap bits and add 0x10000000;
    - 2 / 4 / 9 add 0x1c410009;
    - 3 / 5 add 0x1cd10009, or 0x1dd10009 on +0x10c unless the world's +0x7c and +0x84 sum to 2;
    - 7 swaps bit 0x100 for 0x30000000;
    - 8 sets all three to 0x80000;
  - every option then gets 0x2000000 (the rank frames).
- **Checked in the runner** (tests/hudinit.rs) on 20,000 random game modes, courses, player counts, source tables
  and starting memory, with every call and all memory compared:
  - the game's `memset` is stubbed, because its PCPYH isn't in the runner; it only touches the stack temporary;
  - the tests caught 20 of 26 mutants. The six left cannot change a result: the band thresholds act on a constant
    sum; views 1 and 2's last entry is rewritten later; and the bits the last two clear are always zero by then.

## A sprite on screen (G8j)

What a HUD sprite call means as a quad, in `tricky_game::hudsprites`:

- **`Sprite_DrawRectScaled`** (the rectangle, the flags; x, y, the scales) scales the rectangle's draw size (+4 / +8)
  and hands it to `Sprite_DrawAnchored`.
- **`Sprite_DrawAnchored`** (the rectangle, the flags; x, y, w, h):
  - binds the rectangle's texture (+0) through the renderer's +0x1f4;
  - takes its four texture coordinates (+0xc..+0x18);
  - draws them as a quad in the font colour (0x337fe8).
- **The flags:**
  - 1 / 2 swap the coordinates' middle pair / outer pair, flipping the image;
  - 0x10 turns it a quarter, so w and h trade places;
  - 4 / 8 anchor it by its bottom / right edge;
  - 0x20 / 0x40 centre it on x / y.

  An unturned quad goes to `Draw_TexturedRect` 0x19b730, a turned one to `Sprite_SubmitQuad` 0x19bdf8.
- **`Sprite_DrawTiled`** (the rectangle, the flags; x, y, w, h, the columns, the rows) repeats the texture over
  w × h, cols × rows times, row by row:
  - the whole tiles come first; a last part column and a last part row take the fraction of the texture's
    coordinates that is left;
  - the anchors are as above, but over the whole area;
  - with 0x10 every tile is turned, and its part tiles are cut along the other coordinates;
  - nothing is drawn for no columns or no rows.

  The rank frames use it with one row and as many columns as the frame is wide.
- **Checked in the runner** (tests/spritequad.rs) on 60,000 random rectangles, flags and placements, with whole,
  part, zero and negative columns and rows. Compared: the bind and the quads, floats bit for bit. The tests caught
  all 11 mutants of the first two functions and 13 of 14 for the tiles; the one left gives the same 0 rows either
  way.

## The shadowed prints (G8j)

`tricky_game::fontprint`: every HUD text goes through these.

- **`Font_PrintfShadowed` (ASCII) / `Font_PrintfShadowedA` (wide)** (the font, the format; the scale, x, y):
  - format the string into the frame (`vsprintf` / `WStr_VFormat`);
  - draw it twice through `Font_DrawStringRaw` / `Font_DrawStringA`: first a shadow 2 right and 2 down, in black
    with the font colour's alpha kept, one layer below the renderer's current layer (never under 0); then the text
    in its own colour, on the layer above the shadow's.
- **The centred forms** (`Font_PrintfShadowedCentered` / `Font_PrintShadowedCentered`) measure the string (its
  size written to their frame) and start half its width left of x.
- **The right-aligned form** (`Font_PrintShadowedRight`) ends at x. `Font_PrintAsciiShadowedRight` makes the
  ASCII string wide first.
- What is left for a renderer to supply: the formatting, the measures and the two string draws.
- **Checked in the runner** (tests/fontprint.rs) on 40,000 random cases:
  - compared: every call (floats bit for bit, with the font colour each draw sees) and the font colour after;
  - the tests caught all 12 mutants.

## The app's HUD from the game's code (G8j)

`src/gamehud.rs` draws the in-race HUD with the ported code (the HUD overlay's constructor and update,
`draw_player_sprites` and `draw_player_text`) on a copy of the game's memory, and shows the quads and strings they
end in. The memory comes from:

- **The executable on the player's disc** (`TRICKY_DISC`, `/SLUS_203.26`): the HUD's globals (the bands, the font
  colour, the combo and rank tables), the layouts' initialisers, the number and time formats, and the settings the
  constructor reads.
- **`DATA/LANG/AMERICAN.LOC`**: the localised strings, written into the copy as `Loc_GetString` hands them out.
- **`chars/hud/sprites.json`**: the sprite rectangles, as `SpriteSet_InitHudGameRects` lays them out (texture,
  draw size, v0, u0, u1, v1).
- **`sfnfont`**: the fonts. Its glyph layout is now its own system (`sfn_glyphs`), used by the app's slots and by
  the game-drawn strings alike.

**Each frame:**
- the app's race goes into the game's fields: the meter, the tricky timer, the laps, the velocity (cm/s), racing or
  not, the TRICKY letters, the multiplier, the timed score entries, and the riders' count;
- the HUD state updates at the game's 60 ticks a second;
- both draws run;
- the quads are shown as image nodes and the strings as `SfnText` runs, stacked by the renderer layer they were
  drawn on, then in the order drawn.

**Turned quads never happen in the HUD.** The layout entries' corners in the executable are only 0, 3, 8, 0x20,
0x23, 0x40, 0x60, 0xa0 and 0xe0, and the code's own corners are 0, 1, 8, 0x20, 0x40, 0x48 and 0x60, so the quarter
turn (0x10) is never asked for. `Sprite_SubmitQuad` would show the texture turned anticlockwise (its right edge on
top).

**The constructor re-runs when the app's event changes**, since it sets the HUD's flags by the game mode.
`TRICKY_HUDDBG=1` prints what was drawn once a second. Without the disc, the sheets or the fonts, the app's own HUD
stays.

**The fonts** are the game's own: `gApp+0x5c` is TITLE.SFN and `+0x60` MENU.SFN (`Font_ParseSfn` called at
0x17e934 / 0x17e964, which also sets the title font's scales).

**The game mode and course** are the game's own: the app's events are its menu's World Circuit race and show-off
and free ride (`courses::Mode` 4, 5 and 7), and the course number is found in the game's course table by the level
folder's name.

**No STANDINs are left.** G8k, G8l and G8m replaced them with the game's own code and data (below).

## The fonts in memory, and the HUD's last stand-ins (G8k)

`tricky_game::font`: the font object as the game builds and measures it.

- **`Font_ParseGlyphs`** (the font, the SFN file, an advance added to every glyph, a y added to the origin):
  - copies the records (11 bytes before version 200, 12 from it) into a 12-byte glyph table: code, w, h, texture
    place, advance, x and y offsets;
  - sets the object's count, table, origin (+0x10 / +0x14), and line height (+0xc: the tallest h + y offset, the
    height the HUD reads);
  - notes the run of consecutive codes from the first glyph (+0x28..+0x2c).

  `Font_ParseSfn` then sets the scales to 1.0, and `cGame::Init` sets the title font's to 1.4 × 1.3 and the menu
  font's to 1.8 × 1.4.
- **`Font_FindGlyph`** looks a code up directly inside that run, by a binary search past it, and finds nothing
  below it.
- **`Font_MeasureAscii` / `_MeasureWide`** move the pen from the origin by each glyph's advance. Each glyph's box is
  floored at its left and top and ceiled at its right and bottom, in the font's scales times the caller's, into a
  box that starts at 0. The size is (left, top, width, height), and the width is returned.
- **Checked in the runner** (tests/font.rs) on 20,000 random SFN files:
  - the inputs: both record sizes, versions 100 / 199 / 200 / 201, runs and gaps of codes, and random strings in
    both widths at random scales;
  - compared: the object, the table, the sizes, and every glyph lookup;
  - the tests caught 15 of 17 mutants (two of them by hanging the search). The two left cannot change a result: at
    the first glyph its code can't be the first + 1, and at the run's last code the direct lookup and the search find
    the same glyph.

**In the app's HUD (`src/gamehud.rs`):**
- the two fonts are parsed from the disc into the copy of memory and measured with the port;
- `ScreenOverlay_Init` (the G8b port) sets the HUD's sprite set (gApp+0x728) and fonts;
- the trick points and base points, the meter's share, the rank and its colour (the rank's record at 0x338120) and
  the icon tier all come through the ported score functions (`score.rs`, `grade.rs`), from the rider's trick in
  progress;
- finished-and-stopped comes from the app's race.

**`vsprintf`'s missing arguments change nothing:** of the HUD's strings in `american.loc`, only 0x116 ("%d LAPS TO
GO") and 0xe29 ("%d seconds") hold a `%`, and both go through `WStr_Format` with their argument before any print.

**What is left (G8l)** needs the race model rather than the HUD:
- the race's path set at 0x337fb8 (its 0x3c-byte records: the AI notes have +0x38 as the preference, while
  `HUD_BuildSplitList` uses it as the length its marker distances are taken from; the loader decides);
- each rider's race progress (`Boarder_GetRaceProgress` on +0x110 / +0x114 / +0x374 / +0x37c);
- the opponents' fields.

## The race's path set, progress and place in the app's HUD (G8l)

**The path set at 0x337fb8**, as `AIP_LoadPathsFromFile` 0x197da0 leaves it:
- +0 the AI paths' count and +4 their 0x40-byte `cAIPath` records (+0x38 the preference, parameter 100);
- +8 the event paths' count, +0xc their 0x3c-byte `cEventPath` records, +0x10 the main one.

An event path (`AIPath_Load` 0x196f28):
- +0 its events' count and +4 the events;
- +8 the points' count, +0xc the start, +0x18 the points, +0x1c / +0x28 its box;
- +0x38 parameter 0, its distance to the finish at its start.

An event is 16 bytes: its type's place in the table at 0x337e70 (0 if absent), the value, the start, the end. So
the "length" `HUD_BuildSplitList` takes the markers' distances from is the event path's distance to the finish. The
AI notes' "+0x38 the preference" is the other class, `cAIPath`.

**What the app now does:**
- builds the event paths from the course's `.aip` on the disc (`aip::from_disc`, with its main index and the
  events' table places), without the points, which nothing the HUD calls reads;
- runs the real `HUD_BuildSplitList` from the constructor (the course's checkpoint markers, kinds 12 / 13);
- feeds each rider's race progress through `progress::race_progress` (`Boarder_GetRaceProgress`):
  - +0x374, the distance to the finish (the race line's distance less how far along it, per the AI notes), from the
    app's race lines in centimetres;
  - +0x114, the laps to go: the app's own count, the same convention;
  - the lap line, the main path's length less its lap event's (0xd) distance.

**Two findings from feeding it:**
- **+0x110 is the race place** (0 first), as `progress.rs` has it. `HUD_DrawPlayerText`'s flag 0x200 prints it with
  its ordinal (Loc 0xe44 / 0xe54 / 0xe58 / 0xe45: "st" / "nd" / "rd" / "th"), so that item is the place, not the lap
  (G8h's notes said lap; corrected above).
- **A rider's profile byte +0x65 is its player number**, −1 for a computer rider. The progress dots are drawn (in
  view 0) only for the negative ones, which is why the opponents need −1 there.

**Seen in the app on Garibaldi:**
- the path set has 14 event paths, the main one 460,747 cm long, and 4 split markers;
- the opponents' dots move along the track;
- the first checkpoint's name slides down beside the track as the rider reaches it, fading in and out as the game's
  edges have it.

## The race's placings in the app's HUD (G8m)

`tricky_game::placing`: `Race_UpdatePlacingsAndRubberBand` 0x115100, exactly (race.md has the whole function).

- **What it does:** every 6th tick, each rider gets a key by game mode, the keys are shell-sorted, and the new places
  go to +0x110 (with `Sfx_OnPlacingChange` where they change). In races it is the negated progress plus place × 20,
  or 0x7fffffff − the finishing time once a rider has one.
- **Then the rubber band:** the bounds and targets of the computer riders.
- **Checked in the runner** (tests/placing.rs) on 40,000 random races:
  - the inputs: every mode, course and difficulty, AI counts that agree with the lists and ones that do not, finish
    times, and places;
  - compared: every call and all memory, the stack frame's order, keys, tables and lists included;
  - the tests caught 18 of 20 mutants. The two left cannot change a result: adding 0 × the lag, and dividing it by
    1.
- **`F::ee()` (`ps2f`)** gives a float as the EE reads it for a comparison (a denormal as zero). The port needs it
  where the unchecked walk reads stale indices as floats.

**In the app's HUD:** the copy of memory runs the placing at the game's 60 ticks on the world object (+0x18 its tick,
+0x7c one player, +0x80 the computer riders, +0x88 and +0xc4 the riders). Each rider carries its finishing time
(+0x418: the race time in hundredths at the line, −1 until then) and whether it is human (+0x41c). The places the HUD
shows are therefore the game's own, ordered by the same progress and updated every sixth tick.


## The race's other overlays, part one (G8n1, `game/src/raceoverlays.rs`)

- **`Dialog_Draw` 0x1aa268** draws the dialog box. `Dialog_Set` (G9a1's callee) left its fields: two lines (+0x328,
  +0x32c), three button words (A +0x330, C +0x334, B +0x338), the box's size (+0x33c, +0x340) and the buttons'
  indent (+0x344).
  - It sets untextured mode 3 and keeps the texture to put back at the end.
  - It draws the backdrop's frame (layer 17, 165 high, on texture 18), then switches to texture 19.
  - In white: up to two lines, each centred in the box, from 20 down and 30 apart.
  - The buttons in a row from the indent: A (pad sprite 0x74), B (0x72), C (0x73). Each word sits 23 past its
    sprite and 2 down; the next button starts 40 past the word.
- **`ResultsOverlay_Init` 0x1aacb0**(this, whole) makes the results overlay's backdrop and gives it the results'
  view of the backdrop's table (0x38f880).
  - With `whole`, the backdrop is its own at +0x54.
  - Without it, it uses the same stack copy of the table as the pause menu's constructor (G9a3), with three `this`
    adjustments moved (+8, +0x70, +0x78).
  - It then sets three words (0xe1e, 0xe1f, 0xe1e at 0x3384c0) and clears +0x3c..+0x50.
- **Results helpers the index doesn't list:**
  - 0x1aae10(this; x, gap) lays out the results' 17 title rows at 0x338550. The first is at x + the menu font's
    height · 0.85 + 12, each later one that height + `gap` below; x is kept at 0x3385bc.
  - 0x1aae88 clears the results' two buffers (0x400 bytes at 0x33c5c0, 0x4000 at 0x3385c0).
- **`Overlay_DrawCourseTitle` 0x1aaec0**(this; height) draws the backdrop's frame (layer 5), then the course's name
  (`Course_GetName` 0x16b650 on the career's course) in a pale blue scaled by the backdrop's alpha. It sits at x =
  116, 7 below the frame's top (the frame's top plus sprite 0x37's +8).
- **0x1ab028** (unnamed) draws a 3-wide white divider. In the career's mode 3 it is 260 high at 286.5; otherwise 214
  high at 316.5.
- **The countdown overlay:**
  - `Reset` sets 90 frames of "GO" and the count from the race's clock (its +0x14 in 60ths): +0x30 is 1 − the
    whole seconds, +0x34 the fraction left.
  - `Update`, unless the overlay is hidden, keeps that up while the race counts down (its +0x1c 3). After that it
    fades "GO" over its 90 frames, then hides overlay item 4.
  - `Draw`, unless hidden or in a two-player race, draws on texture 19 either "GO" (0xeaf: growing 3.5 · t,
    rising, full for its first half then fading) or the count's digits (0x1a82e8).
- **The async load's overlay:** `Update` counts its frames. `Draw` puts 0x175 right-aligned at 600 on 440. It also
  takes a sine of its frames (· π / 120) that nothing uses (an alpha pulse compiled out); `DrawText` is empty.

The runner test (tests/raceoverlays.rs) has 18,000 cases over the 12 functions. As with the pause menu, the
results overlay's table copy is checked against the game's stack. 57 of 60 mutants are caught. The 3 left are
equivalent: the last button's returned x (unused), "GO"'s fade at exactly a half (both 1), and the async load's
last colour word (already 1).

## The pop finish overlay and the black fade (G8n2, `game/src/raceoverlays.rs`)

- **`cPopFinishOverlay`** keeps a record per player, 0x3c bytes each from +0x2c. Its fields: +0 shown, +4 the frames
  to wait, +8 waiting, +0xc the free ride's flag, +0x10 the medal sprite (−1 for none), +0x14 the place's
  character, +0x16 a wide-string score, +0x38 the alpha.
  - `Reset`(player) starts that player's wait at 8 frames, with alpha 0. A negative player means the first.
  - Its vf16 stops the sound system's channel 0.
- **`Update`** counts each waiting player's wait down unless the overlay is hidden. When it reaches 0 it fills the
  record by the career's mode, through a jump table at 0x38e3e0:
  - **the race (modes 2, 4):** shown in mode 2, or in mode 4 when 0x1635b0 says 2. The medal comes from the
    rider's place (`0x16bb58`; 1 → sprite 0x28, 0 → 0x29, 2 → 0x27). The place's character is '1' + the place. The
    score is the rider's (0x11f2a0, formatted by 0x1a6ba0).
  - **the time trial (3, 5):** only once the rider has finished (+0x418). Its time is "%s" of the rider's +0x5910.
    In a single race (the event table's count under 2) it gets a medal by the time (0x16baa8 against the course),
    shown only when there is one ('2', '1', '3' for 1, 0, 2).
  - **the free ride (7):** nothing shown; its flag set for the text.
  - **the show-off (9):** the score.
- **`Draw`** does nothing while the overlay is hidden or the pause menu's backdrop is up (+0xc). Otherwise, once the
  viewed player's wait is over, it raises that player's alpha by 0.05 (up to 1). When the record is shown, it draws
  the medal sprite at 72, 211 on the small screen (the display mode word 0x32ca50 is 3) or 217, at 1.3.
- **`DrawText`**, in the event's mode 7 with the free ride's flag:
  - 0xe43, then the time (0x11ea28, kept per player at 0x3ce368), drawn right-aligned at 305.
  - Its difference from the event's target (+0x32c), green with a "+" format (0x38e400) when over, red when under.
  - It calls the career's best time 0x163468 for the course but doesn't use the answer.

  Otherwise:
  - an unshown record gets 0xe43, or 0xe5b when the rider hasn't finished;
  - a medal gets the line "%c%s %s" (0x38e408) of the place's character, its word (0xe44 '1', 0xe54 '2', 0xe58 '3',
    else 0xe45) and 0xe4f, at 2.0 (1.76 in the career's +0x16c0 mode 3);
  - then the score's text, if any.

  Each colour is scaled by the player's alpha.
- **`cBlackFadeOverlay::Draw`** acts while the viewed rider fades (+0x44c above 0) and the race's +0x40 lets it (with
  its +0x20 between 1 and 9 it asks 0x193ce0). It fills the screen with the fade as the drawer's first word,
  untextured in mode 3, with the renderer's +0x1c0 switched off around it.

The test now has 28,500 cases over the 18 functions. A variadic format's stand-in logs the arguments its format
takes (three for the medal line). 53 of 54 mutants are caught. The one left takes a player of 0 as "not
positive", which maps it to 0 all the same.

## The results table and the high scores overlay (G8n3, part 1, `game/src/raceoverlays.rs`)

- **The results table** lives in fixed buffers that every results overlay shares:
  - the title (wide, 0x33c9c0);
  - the column heads (0x33c5c0, 0x80 bytes each);
  - the cells (0x3385c0: column c's at 0x800 c, row r's 0x80 r further on);
  - eight alignments (0x3384d0: 0 left, 1 centred, 2 right);
  - the columns' x (0x338598, column c spanning x[c]..x[c+1]);
  - eight column highlights (0x3384f0) and sixteen row highlights (0x338510), −1 for none.

  The event's mode word +0x320 (`0x32db6c` + 0x320) is the same word as the career's +0x2afc.
- **`cViewHiScoresOverlay::DrawText` 0x1ab1a0** draws the table for every results overlay, not only the high scores
  (the index's name):
  - **The title:** on texture 7, white by the backdrop's alpha, 30 below the frame's top. In a two-player table
    (+0x48) it is at 0.8, and at 76 rather than 116 in the app's mode 2.
  - **The heads:** in gold at 0.7, 8 above the titles' x (0x3385bc). The third head in a long table (+0x50) is at
    0.6 and 2 lower. A head is right-aligned for alignment 2, and so is the first column's with +0x4c; centred for 1;
    at the column's x for 0. Any other alignment keeps the last x: at the first column that is the caller's `$f21`,
    a STANDIN 0.
  - **The cells:** at 0.85 (0.8 in the career's mode 1 with a two-race event), placed by the column's alignment
    (others at x). Each is coloured by the larger of its row's and column's highlights: 0 gold, 1 yellow (its alpha
    and blue set straight, not through `Color_Set`), 2 white, others left as they are. Highlight 2 also sets a flag
    that only picks between three identical prints.
  - **The menu:** the overlay's own (+0x34 items from +0x10, 8 bytes apart, +0x38 picked), at 600, 40 above the
    frame's middle.
- **`Reset` 0x1b3a00:**
  - resets the backdrop and clears the buffers;
  - lays the rows out from the frame's top + 79 (3 apart);
  - sets the columns' x to 131, 214, 342, 415, 505, with the fourth right-aligned;
  - clears the highlights;
  - unless in mode 7, names the heads 0xe5f, 0xe5d, 0x1cd and 0xe61. The fourth is renamed straight after anyway,
    0xe61 for scores or else 0xe5e.

  It then reads the course's best table (`0x163468`): scores in modes 3 and 5, times in 2, 4 and 9. The title is
  formatted from the word at 0x3384c4 or 0x3384c0 with the count (+0x64). Each entry (0x14 bytes) fills a row: its
  rank, its name, the rider's name (from the characters' table), and the score formatted (`0x1a6ba0`) or the time as
  a number. In the other modes there is no table and the game reads one from address 0. The port does the same.
- **`Draw` 0x1b3e30:** the course's title (460 high) and the help line 0x1d7 at 600, 430.
- **`Update` 0x1b3ec8:** the results' update. The select button (0x20) plays the select sound, hides overlay item 11
  and shows item 1.

The test now has 36,000 cases. The table's layout, highlights, counts (up to 20 rows), flags and best table are all
random, and low memory is mapped for the null table. 66 of 68 mutants are caught. The two left remove one of the
column highlights' two clears, which are each enough alone.

## The next race overlay and the results' builders (G8n3, part 2, `game/src/raceoverlays.rs`)

The event's course +0x31c (`0x32db6c` + 0x31c) is the same word as the career's +0x2af8, as its mode +0x320 is the
career's +0x2afc.

- **`cNextRaceOverlay::Reset` 0x1b4120:**
  - resets the backdrop and clears the buffers;
  - sets no menu (+0x34, +0x38, +0xc), help 0x18d (+0x10), no highlights, no targets (+0x1d8);
  - builds the page by the career's mode: 2, 4 and 9 the standings, 3 and 5 the hi-score page, 7 the time
    challenge's;
  - calls the speech's next race (`0x231ff8`, with the word at 0x343050).
- **Its vf16 0x1b4240** (not in the index) stops the sound system's channel 0.
- **`Results_BuildStandingsHeader` 0x1b4250:**
  - **One player:** rows from 137, 23 apart, and two columns (131, 259, 434).
  - **Otherwise:** rows from 142, 29 apart, and three columns (116, 244, 419, 594).
  - It clears the note (+0x54).
  - **The title and help:** 0x207 and 0x1c5 in event modes 2 and 9. Otherwise, by the career's state (`0x1635b0`),
    the title word at 0x338290 and the help id at 0x390838.
  - **The rows:** each rider whose player byte (+0x69 of its 0x84-byte record) is −1 gets a row with its name. The
    row count is the riders less the players.
- **`Results_BuildHiScorePage` 0x1b44c0:**
  - one column (131..410) and no rows;
  - the note 0xe3f with the course's best score;
  - the targets shown at 50 (+0x1dc, +0x1d8).
  - **Title and help:** in event mode 3, 0xe67 with help 0x1c6 in a one-rider event, else help 0x1c7 with the targets
    hidden at 148. In the other modes, 0xe6e and 0x1d4.
  - **The three targets** (+0x118, +0x158, +0x198) are formatted by 0x185. On course 6 of mode 3 they are the
    numbers 800000, 500000 and 250000, printed through "%d" (0x38e200). Otherwise they are the course's three
    strings (courses 0..5, 8, 11). Any other course leaves the frame's three words as they are.
- **`Results_BuildTimeChallengeHeader` 0x1b4890:** the course's best of kind 2, giving its score (+0x1e0), its name
  (+0x208) and its rider's name (+0x230); the title 0xe96 and help 0xc75.
- **`Draw` 0x1b4950:** sets the backdrop's +0x50 to 0.8 for the duration and puts it back after.
  - **The course's title:** 410 high in mode 7, else 460.
  - **In modes 3 and 5:** the targets' three medals, sprite 0x2e at +0x1dc: at 150 in gold, 215 pale and 280 yellow.
  - **In modes 2 and 4:** each computer rider's row gets its attitude to the first player (`0x1a8fa0`, at the second
    column, 3 lower), and to the second player with two. The second call's a1 is left at the 1 of the rider lookup,
    not the attitude byte.
  - **When the targets show:** the three target sprites (0x390848) at 426, from 189 and 40 apart.
- **`DrawText` 0x1b4d98:**
  - **In event mode 7, the time challenge's head:**
    - the players' names and riders at 117 and 330, in white with the alpha in place of the first component;
    - the headings 0xea2..0xea4 in gold;
    - the best's score, name and rider.
  - **Modes 4 and 5** mark the table two-player (+0x48).
  - **In modes 2 and 4, the standings' heads:** 0xe60 and 0xe5c, and the players' riders through 0xeb7.
  - **Then, in every mode:**
    - the table;
    - the note (not in mode 7);
    - the help (+0x114) in a text box at 346 in modes 3 and 5, 320 in mode 7, else 352. Its colour is gold, with the
      alpha written straight into the box;
    - in modes 3 and 5, the targets' labels 0x204..0x206;
    - the targets, when shown.
- **`Update` 0x1b5878:** the results' update; the menu is shown once the race's +0x1c object's +0x4f8 is clear.

The test now has 49,500 cases. The riders' records, attitudes, courses, the targets' flags and the standings'
titles are random, and the sprites' stand-in logs the backdrop's +0x50. 119 of 120 mutants are caught. The one left
reads the hi-score page's course through the career's +0x2af8, the same word.

## The unlocked and high-score record overlays (G8n4, `game/src/raceoverlays.rs`)

The index marks `cUnlockedOverlay::Reset` 0x1b5ef8 "ported", but nothing ported it. Each overlay also has a vf16
(0x1b6458, 0x1b7068: stop the sound system's channel 0) and a `Draw` (0x1b6468, 0x1b7078) that the index doesn't
list.

- **The unlocked overlay's records:**
  - +0x54: a count per player (0x28 bytes each) and kind.
  - +0xf4: the last value for each.
  - +0x194: the players' flags.
  - +0x1a4: the next row.
  - +0x1a8 / +0x1ac: the race's and the show-off's final unlocks.
  - +0x1b0 / +0x1c0: the players' names and riders.
  - +0x1d0: the kinds' sprites.
  - +0x1f8: the rows' sprites (28).
- **`Results_AddAwardLine` 0x1b5bf8(this, kind, player)** adds a row for a player's unlocks of one kind:
  - **Two or more:** formatted by the kind's plural format (0x338240 + 4 kind) with the count.
  - **One:** formatted by the single format (0x338268 + 4 kind), with the kind's own words:
    - kinds 0 and 1: the course's name;
    - kind 2: only courses 6 and 7, with sprite 0x58 or none;
    - kind 3: the rider's name;
    - kind 5: the player's complete trick book pages;
    - other kinds: the player.
  - **None:** nothing.
  - Unless named above, the row's sprite is the kind's (+0x1d0 + 4 kind).
- **`Reset` 0x1b5ef8:**
  - clears the buffers and sets every highlight to 2;
  - sets the title to 0x38e220;
  - asks for the final unlock (`0x1618c0` in the career's mode 4, else `0x1618e0`);
  - gathers the race's unlock list (0x3353f8: a count and entries of 0xc bytes: the player, the kind, the value)
    per player slot (`0x166550`; a negative slot counts as 0). Kind 9 stores its value as the count; the others
    count up.
  - For each flagged player:
    - the lines in the order 5, 9, 8, 4, 3, 2, 1, 0, 6;
    - the name;
    - with a kind 4 or 5, the rider;
    - +4 keeps the highest row;
    - the next player's rows start at row 7.
  - **The rows' layout:** one column at 156..436, the first rows 13 apart (10 with two players), from the frame's
    top + 91.
  - **The reveal:** each row is added to the backdrop's animation (+0x94, `0x1f11c0`). Its length is the text's
    plus 0x50, or 0 when the row is empty.
  - Last, the speech's unlock lines.
- **`Draw`:**
  - the course's title (460 high);
  - with one player, the sprite of the row the animation is revealing (+0x94 is the row + 1), or, when that row has
    none, the last row's.
- **`DrawText` 0x1b65f0:**
  - each player's name line ("%s %s" 0x38e548 of 0xdf6 and the name) in gold, 148 apart, with 0xdf7 under it;
  - the rows through `Font_DrawAligned`, by the animation's count of each row;
  - the final unlock's note (0xc9c or 0xc9d) in a white box at 65, 330;
  - the menu.
- **`Update` 0x1b6ac0:**
  - the menu shows once the race's +0x1c object's +0x4f8 is clear;
  - button 0x800 plays the select sound and, unless the picked item's word (+0xc + 8 the pick) is set, hides
    overlay item 13 and shows item 1.
- **The high-score record.** The name grid at 0x3382a0 is three rows of 14 characters, where 'd' deletes, 'e' ends
  and 0 is empty. `Init` 0x1b6b90 copies the grid in from 0x38e550 (and to its own frame, unused), with the record's
  backdrop table 0x38f4f0 (six shifts) as the results overlay does.
- **`Reset` 0x1b6d60:**
  - sets the high scores' layout and clears the name (+0x64, 0xb bytes);
  - sets entering (+0x114);
  - sets the title by the mode: 3/5, 2/4/9, and 7 (0xea1, with the best of kind 2). Other modes read the table from
    address 0;
  - submits the first record.
- **`Draw` 0x1b7078:**
  - the cursor (sprite 0x36 frame 0x20) over the picked character: +0x58 is the row, +0x5c the column;
  - in mode 7, a faint bar;
  - otherwise the name's caret (sprite 0x32). The measures of the character and of the name with "." are unused.
- **`DrawText` 0x1b73e0:**
  - the table and the grid's characters ('e' as the word 0x112, 'd' as sprite 0x32);
  - **In mode 7, the time challenge's box:** 0xea0 (its format takes nothing; a2 is whatever the colour copy left),
    the labels, the best's score and the target in white, and their difference (`0x1a6c90`) in magenta.
  - **Otherwise:** the name typed at the entry's row.
  - **Then:** the help line (0x113, or 0x18d on the end key) and the two prompts.
- **`Update` 0x1b7d98** reads the entering player's pad (the race's +0x38's +0xc):
  - up/down (0x20001 / 0x40002) and left/right (0x100008 / 0x80004) move past empty cells. Up and left wrap only
    once and then keep going down; down and right wrap by 3 and 14.
  - select types the character, the tenth character on replacing the ninth.
  - 'd' deletes the last character.
  - 'e' submits the next record. When there is none, it hides overlay item 2 and shows item 13 if any unlock isn't
    kind 7, else item 1.
- **`HiScore_SubmitEntry` 0x1b81b8** takes the race's new records at 0x32eca0 (0x10 each: the pad, _, the best
  table, the new entry):
  - Past the first record, it sets the name typed as the entering player's (`0x164968`) and copies it over the last
    record's entry.
  - A record with a pad rebuilds the table. The new entry is highlighted (2), its row kept (+0x54), its speech run,
    and while entering its name copied in and line 0x114 formatted. Mode 7 blanks the cells.
  - The cursor goes to the end key.

The test now has 72,000 cases. The unlock lists, counts, the grid (row 0 and each first column filled, so every
move ends), the cursor, the name and the new records are random. The stand-ins log the text style's +4, an ASCII
string's first characters, a stacked text's first word and the text box's +0x20 / +0x24 / +0x6c.
195 of 197 mutants are caught. The two left are equivalent: a slot of 0 maps to 0 under either test, and the row count +4 is compared signed or unsigned but is never negative.

## The next event overlays: the card screens, points, venue and event choices, race help (G8n5, `game/src/nextevent.rs`)

`cSelectNextEventOverlay`'s base is the memory card UI (`cMCOverlayManager`), so the index's names for its slots
("DrawText", "Reset", vf16, vf17, vf23, vf26) are the card UI's screens. The overlay `o` has:
- +0x10 / +0x18 / +0x1c: the options' flags;
- +0x38: the slot picked;
- +0x48: the progress;
- +0x4c: the message's alpha;
- +0x54: the state;
- +0x168 / +0x174 + 0x58 i: a slot's two flags;
- +0x218 / +0x21c: the entries and the pick;
- +0x224: the card system;
- +0x428: its second table;
- +0x42c: its backdrop;
- +0x434: its title's id.

The functions the index doesn't list are the points overlay's `Reset` 0x1ba208 and vf16 0x1ba2f0, the venue and
event overlays' `Reset`s 0x1bb678 and 0x1bc6a8, the help's empty 0x1bd308 and the three small functions
0x1bda18..0x1bda38.

**A widths' clamp is shared by the screens.** Under 200 becomes 200; over 384 becomes 320; over 512 becomes 384.

### The card screens
- **vf17 0x1b8718, the progress:**
  - the frame (item 0xe, 290 high);
  - a box (texture 15) across 104..516, its rect 0x2a's height · 0.8 tall, centred on 240;
  - 14 divider lines, 27.47 apart from 131.47;
  - the progress (+0x48, when above 0) as a purple bar, step · 14 · the share long, plus a step;
  - the frame (texture 18);
  - line 0x1c3 (with +0xd8) in a box clamped to its width + 10, at 144;
  - the overlay's own virtual +0xd8.
- **vf26 0x1b8bb8, the card system's message:** 0x15e when its +0x1a0 says so, else 0xdeb. It goes in a box at
  128, 312 (384 × 96) with the alpha +0x4c.
- **vf16 0x1b8d08, the card screen:**
  - the frame (item 5, 420 high) and the title box;
  - four colours: white, a dim white, yellow, and the pick's purple. Their alphas are 0.4 out of state 7.
  - the card system's name (+0x1e0 with +0x21c) centred at 86.4, and its description (+0x1b0) in a box under it;
  - the list (+0x1d0 the count, +0x70 an entry), numbered at 204.8 and named at 236.8, 36 apart from 168. Only
    the first entry shows while the overlay's word 0 is 1. The pick, when named, sits on a purple bar; its
    colour is set to yellow and straight back to white.
  - the three options 0x1bb, 0x154, 0x157 at 448, dim unless +0x10 / +0x1c / +0x18, with the pad sprites 0x74,
    0x73, 0x72 at 420;
  - out of state 7, the screen shaded.
- **"DrawText" 0x1b9530(this, text, first, second, swapped), a question:**
  - the frame (item 0x10, 300 high);
  - the text in a box across the screen at 204 (its line height + 4), centred;
  - the answers with their pad sprites on 295: both side by side (placed from 640 · 0.13 and · 0.27), or the one
    alone centred. Its measure of the text and the clamp's compares are unused.
- **"Reset" 0x1b9968(this, text), a notice:** the text in a box clamped to its width + 10, one line high. Its y is
  240 − half the height, cut to a whole number.
- **vf23 0x1b9b80, the slots screen:**
  - the title at 38.4, 19.2;
  - each slot (`MCSlot_GetLabel`) at 96, 26.4 apart, white when usable (both its flags), else dim;
  - the pick on a purple bar;
  - 0x155 (dim unless the pick is usable) and 0x154 with their sprites.
- **The small virtuals:** vf47..vf53 play the move sound unless at the end, then call the base's next/previous; or
  they play the select or back sound.

### The point distribution overlay
- **`Reset`:**
  - the attributes' names 0x25, 0x27, 0x28, 0x29 into 0x33d6b0;
  - the player's points kept (+0x438, +0x43c);
  - no card operation, no exit timer, no pending video;
  - the career's +4 cleared (in a delay slot, so in every mode);
  - in the career's +0x370c mode 1, the speech for the level.
- **`AttribPointsOverlay_StartExitTimer` 0x1ba300:** when the exit waits (0x32ea9c is 1), sets the timer to 5
  seconds. It returns the timer's flag.
- **`Update` 0x1ba330:**
  - **A card save:** runs to its end; a good result saves and, unless the exit waits, leaves.
  - **The exit timer:** counts down 1/60 a frame while no dialog is up, and leaves at 0.
  - **A dialog's answer:** 3 (no) leaves unless the exit waits. 1 (yes) to the save question starts the save. 0 and
    2 return.
  - **The buttons:** up/down pick the attribute (wrapping 0..3); left/right refund or spend a point, with the tick
    or error sound. Select with no points left asks to save (0x1c1).
  - **Leaving:** hides item 15. In mode 1 it ends the race, with the video "%s" of the player's rider pending,
    +0x3260 set to 0x30, and the race's +0x54 and the career's +4 set; otherwise it shows item 17.
- **`Draw` 0x1ba788:**
  - **First:** the dialog, or the card save's draw.
  - **The level screen** (with the exit timer): 0xd25 / 0xd26, the level's name (+1) and the next level's in gold,
    and 0x175.
  - **The points screen:** a `cFE3DProgressBar` built on the stack exactly as the game inlines it: the thing's and
    the widget's words, the colour update, the bar's, its rectangle virtual (+0x138) and colours.
    - The points left, out of the points at open (+0x438 · 6 wide), at 335, 244. The first time the screen opens
      it notes the points (0x338228, 0x3ce370).
    - Four attribute rows, 19 apart from 270. The picked row is gold with arrows. Each has its name, its bar (its
      maximum · 5 wide, the base + the bonus out of the maximum) at 200, and "%d/%d".
    - The rider's name; 0xe50; help 0xc78 in a box.
    - **With no points:** the help line 0x113.
    - **Otherwise:** 0xc79 in a second box. Its alignment words are written into the first box, and its load of
      0x197 is unused.
    - Last, the bar's destructor.
- **vf3:** `Career_SaveToCard`.

### The venue and event choices
- **The venue `Reset`:** the venues of the career's +0x3818 mode:
  - mode 1: the race courses;
  - modes 2, 4, 7: the second list;
  - others: the third. Mode 3 also adds course 6, at the third list's count.

  It then sets each venue's sprite (0x45 + the venue, for venues under 12), its text's buffer (0x3ce378, 0x50
  apart) and its unlocked flag; the menu's top and left (320 less half the longest name, from the courses' records
  at 0x332f28); the second-to-last item empty and the last 0x18f.
- **`NextEventOverlay_DrawVenue` 0x1bbf20:**
  - the head (0xe4e, its text by the mode through the format 0x38e5f0 of 0xe76, the mode's word, 0xe77);
  - the venues' menu, and the pick's sprite when unlocked;
  - the help line 0x1d7, right-aligned;
  - in modes 4 and 5, the medals (0x135, 0x12f, 0x12c, 0x132), dim when locked.
- **The event `Reset`:**
  - the five items 0x19b, 0x1c9, 0xc7a, 0x1c8, 0x18f, the third off in modes 4 and 5;
  - the pick: 2 in mode 7, 1 in modes 3 and 5;
  - the menu centred;
  - no pending video, the career's +4 cleared.
- **`DrawEvent` 0x1bcd28:** the head (0xe49), the help line when +0x464 is set, and the five items' menu.

### The race help
- **`Reset`, `Update` and `Draw`:** as the other results overlays.
- **`DrawText` 0x1bd3b8:**
  - the heading 0xebd in gold;
  - five lines with bullet squares (at 115 in the app's modes 2 and 3, else 160);
  - help 0x155;
  - pad sprites 0x71 and 0x75 after the third and fifth lines.
- **`StaticInit_HelpRaceOverlay` 0x1bd898:** fills its static vectors and colours, and sets the name grid's x
  (0x3ce788, the static 0x338220 + 131).

The runner test (tests/nextevent.rs) has 176,000 cases over the 44 functions: the overlay's flags, slots, lists, points, timer, dialog answers and pad buttons, the venue lists and modes, and the font's height (odd too) are random. The progress bar's 0x1b0 bytes are logged at each of its calls, and the text boxes' words as they draw and go. 170 of 174 mutants are caught. The four left are equivalent: the single answer's pick between first and second when only one is set; the notice's width plus a zero; the unsigned length's halving keeping its low bit (the same float); and the event menu's measure at its scale +0x470, which it has just set to 1.

## The race overlays' constructors, destructors and small virtuals (G8n6, `game/src/overlaybase.rs`)

The overlays use GCC 2 virtual bases in two families:

| | the backdrop | the class's own table |
|---|---|---|
| the results family | at the overlay's +0 | (none) |
| the memory card family | at +0x42c | at +0x428 |

The backdrop's +0x28 is the table that the overlay's class sees it through.

When the overlay is part of a whole object (a constructor's `whole` 0, a destructor's flags 0), the class's
table is copied to the frame (0x98 bytes). Its `this` shifts (half words at the class's offsets) are moved by
`this` − (the backdrop − its offset), and the backdrop's +0x28 then points at the copy. A destructor with flags
sets the table and leaves it; flag 2 destroys the backdrop and flag 1 frees the object (0x114f80).

- **The results family's destructors** (the index's vf0s: unlocked, race help, next race, high-score record, high
  scores; and `cResultsOverlay`'s own) do the class's stage (its copy at the frame's +0xa0) and then the results
  overlay's (its copy at the frame). The classes' tables, backdrop offsets and shifts are:
  - unlocked: 0x38f328 at 0x268;
  - race help: 0x38f3c0 at 0x54;
  - next race: 0x38f458 at 0x258;
  - high-score record: 0x38f4f0 at 0x118;
  - high scores: 0x38f588 at 0x54;
  - the results overlay: 0x38f880 at 0x54, with shifts 8, 0x70 and 0x78.
- **The constructors** (`NextRaceOverlay_Init` 0x1b3f50, the unlocked overlay's 0x1b58c0, the race help's 0x1bd190)
  run `ResultsOverlay_Init` and then the class's stage. The index marks all three "ported", but none was.
  - The next race's sets three standings titles (0xe4b, 0xe4c, 0xe4a), clears the targets, and empties their
    texts.
  - The unlocked overlay's loads the unlocks' ten single and ten plural formats and the kinds' sprites
    (0x58, 0x59, −1, 0x5a, 0x5b, 0x5c, 0x5d, −1, 0x5f, 0x5e).
- **The card family's destructors** (next event, venue, points, pause menu) set their own table at +0x428 and
  their backdrop's, with a copy at +0xa0 when there are no flags. The pause menu also destroys its member at +0x7f8
  (0x155058). Then comes the card UI's own stage: its table 0x38f9b0, its backdrop's 0x38f918 with one shift at
  0x438, and its base destructor (0x1ce580). `cSaveOverlay`'s destructor 0x1bfe00 is that stage alone.
- **The card family's constructors** set up the backdrop (their own when whole), call `SaveOverlay_Init`, then set
  their tables and clear one field (+0x464, +0x55c, +0x440).
- **`cViewHiScoresOverlay` vf19 0x1bf580** destroys the race results overlay, an object of two parts: `this`+0 is
  the results part and +4 the card part.
  - **With no flags:** the high scores' table (shifts 8, 0x20, 0x68..0x88; its backdrop at 0x10c) is copied to the
    frame's +0xa0. The card part's own table (0x1c8 bytes, shifts 8 and 0x20..0x38, at 0x454) goes to its +0x140.
  - **Flag 2:** sets the results part's table back (its copy at the frame). The card UI's copy then overwrites
    the same frame before the base destructor and the backdrop's destructor run.
- **The backdrop's destructor** 0x1bff78 (the index's "ported" again wrong) sets its table, destroys its
  animation (+0x94), and then runs the screen overlay's 0x1c01e8, which sets its table and frees on flag 1.
- **The small overlays'** (the lesson controller's and the lesson's on backdrops; the black fade's, async load's,
  pop finish's and countdown's on screen overlays) destructors set their tables and run the base's. Their
  constructors, which the index doesn't list, run the base's init and set their tables.
- **The rest are one-liners:**
  - answers of 0, 1 or `this`, and empty virtuals;
  - the career saved to the card (directly, or through the card part at +4);
  - the async load's `Reset` (+0x2c);
  - setters of +0x55c, +0x434 and the pad state (0x3381e0);
  - the new buttons (+0x10 and not +0xc, its +0x2c cleared);
  - whether a card save runs, and whether a dialog is up;
  - the screen overlay's flag tests and copies (+0xc, +0x10, +0x14, +0x24);
  - its virtual +0x78 when shown;
  - `Draw_SolidRect`: untextured, then the colour, then the rectangle.

The runner test (tests/overlaybase.rs) has 45,000 cases over the 75 functions, with the results overlay's init and the backdrop's and screen overlay's destructors real on both sides. The tables copied to the frame are compared byte for byte (6,855 of them). 62 of 64 mutants are caught. The two left drop stores that are overwritten before anything reads them: the card destructor's first +0x428, and the race results' shifts that the card UI's copy overwrites.

## Names from F1c batch 19 (proven)

- `Draw_GradientRect` 0x19b2a0 (was `FUN_0019b2a0` in `game/src/quad2d.rs`): a rectangle with a colour at each
  corner, z from the first word's +0x10, u and v 0..1.
