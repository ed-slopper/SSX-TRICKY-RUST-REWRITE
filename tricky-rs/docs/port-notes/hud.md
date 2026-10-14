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
