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
