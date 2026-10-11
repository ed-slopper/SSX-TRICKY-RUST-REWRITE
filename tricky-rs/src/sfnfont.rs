//! The game's bitmap fonts (DATA/FONTS/TITLE.SFN and MENU.SFN, `Font_ParseSfn` 0x19c858,
//! `Font_DrawStringA` 0x19d0c0, `Font_PrintfShadowedA` 0x19d568) and the in-race text drawn with
//! them (`HUD_DrawPlayerText` 0x1a4f70), in the game's 640x480 screen.
//!
//! Atlases and glyph metrics come from `chars/fonts/{title,menu}.{png,json}` (tools/sfn/sfn2png.py).
//! STANDIN: atlases exported by tools/sfn/sfn2png.py, for reading the game's SFN files at run time
//! Drawing: each glyph at pen + offset, both times the scale (the font's own: title 1.4 x 1.3,
//! menu 1.8 x 1.4, times the caller's); the pen moves by the glyph's advance; no kerning, no extra
//! spacing; a glyph the font lacks is skipped. Width for centring and right-aligning is the
//! glyphs' box, not the sum of advances. The shadow is the same text in black at the same alpha,
//! 2 px right and down (not scaled).
//!
//! Without the font files the plain text in `ui.rs` stays.

use crate::{ui, CharLib, RiderRes};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Glyph { x: f32, y: f32, w: f32, h: f32, xoff: f32, yoff: f32, adv: f32 }

struct Font { image: Handle<Image>, glyphs: HashMap<char, Glyph>, line: f32, sx: f32, sy: f32 }

#[derive(Resource, Default)]
pub struct SfnFonts { tried: bool, fonts: Vec<Font> }

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Align { #[default] Left, Centre, Right }

/// A run of text in one of the game's fonts, placed in the 640x480 screen.
#[derive(Component, Clone, PartialEq, Default)]
pub struct SfnText {
    /// 0 TITLE, 1 MENU
    pub font: usize,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub align: Align,
    /// y is the bottom of the (last) line rather than the top
    pub bottom: bool,
    pub color: Color,
    pub shadow: bool,
    /// wrap at this width (640 units), lines stacking upward from the bottom; 0 = no wrap
    pub wrap: f32,
}

/// Which HUD text an `SfnText` shows.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum Slot { Place, Suffix, Lap, Time, Score, Goal, Mult, SpeedNum, SpeedUnit, Trick, Big, Note }

#[derive(Component)]
struct GlyphNode;

fn vw(x: f32) -> Val { Val::Vw(x / 640.0 * 100.0) }
fn vh(y: f32) -> Val { Val::Vh(y / 480.0 * 100.0) }

impl Font {
    /// A font's glyphs (what `Font_ParseSfn` 0x19c858 reads, from our export).
    fn load(dir: &std::path::Path, name: &str, images: &mut Assets<Image>) -> Option<Font> {
        let img = image::open(dir.join(format!("{name}.png"))).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let image = images.add(Image::new(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            bevy::render::render_resource::TextureDimension::D2, img.into_raw(), bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default()));
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).ok()?).ok()?;
        let mut glyphs = HashMap::new();
        for (k, g) in v.get("glyphs")?.as_object()? {
            let Some(c) = k.parse::<u32>().ok().and_then(char::from_u32) else { continue };
            let f = |n: &str| g.get(n).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
            glyphs.insert(c, Glyph { x: f("x"), y: f("y"), w: f("w"), h: f("h"), xoff: f("xoff"), yoff: f("yoff"), adv: f("advance") });
        }
        let gs = v.get("game_scale");
        let s = |n: &str| gs.and_then(|g| g.get(n)).and_then(|x| x.as_f64()).unwrap_or(1.0) as f32;
        Some(Font { image, glyphs, line: v.get("line_height").and_then(|x| x.as_f64()).unwrap_or(16.0) as f32, sx: s("x"), sy: s("y") })
    }
    /// The glyph boxes' width (Font_MeasureA), in 640 units.
    fn width(&self, text: &str, scale: f32) -> f32 {
        let (sx, mut pen, mut lo, mut hi) = (self.sx * scale, 0.0f32, f32::MAX, f32::MIN);
        for c in text.chars() {
            let Some(g) = self.glyphs.get(&c) else { continue };
            lo = lo.min(pen + g.xoff * sx);
            hi = hi.max(pen + (g.xoff + g.w) * sx);
            pen += g.adv * sx;
        }
        if hi < lo { 0.0 } else { hi.ceil() - lo.floor() }
    }
    /// Word-wrap at a width (HUD_DrawWrappedText).
    fn wrap(&self, text: &str, scale: f32, width: f32) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for para in text.split('\n') {
            let mut cur = String::new();
            for word in para.split(' ') {
                let tryit = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
                if !cur.is_empty() && self.width(&tryit, scale) > width { lines.push(std::mem::take(&mut cur)); cur = word.to_string(); } else { cur = tryit; }
            }
            lines.push(cur);
        }
        lines
    }
}

/// Lay out every changed text as glyph nodes (`HUD_DrawPlayerText` 0x1a4f70 with `Font_DrawStringA` 0x19d0c0 and
/// `Font_PrintfShadowedA` 0x19d568).
#[allow(clippy::too_many_arguments)]
pub fn sfn_text(
    mut commands: Commands, lib: Res<CharLib>, mut fonts: ResMut<SfnFonts>, mut images: ResMut<Assets<Image>>,
    root: Query<Entity, With<ui::HudRoot>>, rider: Res<RiderRes>, time: Res<Time>,
    mut items: Query<(&Text, &ui::HudItem, &TextColor, &mut Visibility), Without<SfnText>>,
    mut slots: Query<(Entity, &Slot, &mut SfnText, Option<&Children>)>,
    mut mult: Local<(u32, f32)>,
) {
    if !fonts.tried {
        fonts.tried = true;
        let dir = lib.dir.join("fonts");
        let (Some(t), Some(m)) = (Font::load(&dir, "title", &mut images), Font::load(&dir, "menu", &mut images)) else { println!("fonts: chars/fonts not found, plain text"); return };
        fonts.fonts = vec![t, m];
        let Ok(root) = root.single() else { return };
        let full = Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
        commands.entity(root).with_children(|p| {
            for s in [Slot::Place, Slot::Suffix, Slot::Lap, Slot::Time, Slot::Score, Slot::Goal, Slot::Mult, Slot::SpeedNum, Slot::SpeedUnit, Slot::Trick, Slot::Big, Slot::Note] {
                p.spawn((full.clone(), s, SfnText { scale: 1.0, color: Color::WHITE, shadow: true, ..default() }));
            }
        });
        println!("fonts: TITLE and MENU");
        return;
    }
    if fonts.fonts.is_empty() { return; }

    // the HUD's strings (worked out in ui::hud) go to the game's places and fonts
    let mut got: HashMap<u8, (String, Color)> = HashMap::new();
    for (text, item, color, mut vis) in items.iter_mut() {
        let k = match item { ui::HudItem::Place => 0, ui::HudItem::Time => 1, ui::HudItem::Score => 2, ui::HudItem::Speed => 3, ui::HudItem::Trick => 4, ui::HudItem::Big => 5, ui::HudItem::Letters => 6, _ => continue };
        if *vis != Visibility::Hidden { *vis = Visibility::Hidden; }
        got.insert(k, (text.0.clone(), color.0));
    }
    let r = &rider.0;
    if r.multiplier != mult.0 { *mult = (r.multiplier, if r.multiplier > mult.0 { 2.0 } else { 0.0 }); }
    mult.1 -= time.delta_secs();
    let empty = (String::new(), Color::WHITE);
    for (_, slot, mut t, _) in slots.iter_mut() {
        let mut n = t.clone();
        let (s, c) = match slot {
            Slot::Place | Slot::Suffix | Slot::Lap => got.get(&0).unwrap_or(&empty),
            Slot::Time => got.get(&1).unwrap_or(&empty),
            Slot::Score | Slot::Goal | Slot::Mult => got.get(&2).unwrap_or(&empty),
            Slot::SpeedNum | Slot::SpeedUnit => got.get(&3).unwrap_or(&empty),
            Slot::Trick => got.get(&4).unwrap_or(&empty),
            Slot::Big => got.get(&5).unwrap_or(&empty),
            Slot::Note => got.get(&6).unwrap_or(&empty),
        };
        let first = s.lines().next().unwrap_or("").trim().to_string();
        n.color = Color::WHITE;
        n.align = Align::Left;
        n.bottom = false;
        n.wrap = 0.0;
        n.font = 0;
        match slot {
            // place: the number large and right-aligned, its "st" small beside it (gold in the lead)
            Slot::Place => { n.text = first.trim_end_matches(char::is_alphabetic).into(); (n.x, n.y, n.scale, n.align, n.color) = (66.0, 27.0, 1.9, Align::Right, *c); }
            Slot::Suffix => { n.text = first.trim_start_matches(|ch: char| ch.is_ascii_digit()).into(); (n.x, n.y, n.scale, n.color) = (71.0, 30.0 + 18.0, 0.7, *c); }
            Slot::Lap => { n.text = s.lines().nth(1).unwrap_or("").trim().into(); (n.x, n.y, n.scale) = (26.0, 78.0, 0.7); }
            Slot::Time => { n.text = first; (n.x, n.y, n.scale, n.bottom) = (26.0, 460.0, 0.9, true); }
            Slot::Score => { n.text = first.split("  x").next().unwrap_or("").into(); (n.x, n.y, n.scale, n.align) = (620.0, 24.0, 1.1, Align::Right); }
            Slot::Goal => { n.text = s.lines().nth(1).unwrap_or("").trim().into(); (n.x, n.y, n.scale, n.align) = (620.0, 60.0, 0.6, Align::Right); }
            // the multiplier, for a moment when it goes up: x2 gold, x3 orange, x5 red
            Slot::Mult => {
                n.text = if mult.1 > 0.0 && r.multiplier > 1 { format!("x{}", r.multiplier) } else { String::new() };
                n.color = match r.multiplier { 0..=2 => Color::srgb(1.0, 0.88, 0.137), 3 | 4 => Color::srgb(1.0, 0.47, 0.04), _ => Color::srgb(0.99, 0.0, 0.0) };
                (n.x, n.y, n.scale, n.align) = (320.0, 110.0, 1.9, Align::Centre);
            }
            // speed: the number right-aligned to x 90 and the unit after it, both standing on y 430
            Slot::SpeedNum => { n.text = first.split(' ').next().unwrap_or("").into(); (n.x, n.y, n.scale, n.align, n.bottom) = (90.0, 430.0, 0.9, Align::Right, true); }
            Slot::SpeedUnit => { n.text = if first.is_empty() { String::new() } else { " km/h".into() }; (n.x, n.y, n.scale, n.bottom) = (90.0, 430.0, 0.7, true); }
            Slot::Trick => { n.text = s.trim().into(); (n.font, n.x, n.y, n.scale, n.align, n.bottom, n.wrap, n.color) = (1, 320.0, 442.0, 1.0, Align::Centre, true, 256.0, *c); }
            // the TRICKY letters are the meter's sprites; what is left is the hint after them
            Slot::Note => { n.text = first.get(6..).unwrap_or("").trim().into(); (n.font, n.x, n.y, n.scale, n.align, n.color) = (1, 320.0, 80.0, 0.8, Align::Centre, *c); }
            Slot::Big => { n.text = first; (n.x, n.y, n.scale, n.align, n.color) = (320.0, 150.0, 3.0, Align::Centre, *c); }
        }
        if *t != n { *t = n; }
    }

    // build the glyphs of what changed
    for (e, _, t, kids) in slots.iter_mut() {
        if !t.is_changed() { continue; }
        if let Some(kids) = kids { for k in kids.iter() { commands.entity(k).try_despawn(); } }
        let Some(f) = fonts.fonts.get(t.font) else { continue };
        if t.text.is_empty() { continue; }
        let (sx, sy) = (f.sx * t.scale, f.sy * t.scale);
        let lines = if t.wrap > 0.0 { f.wrap(&t.text, t.scale, t.wrap) } else { t.text.split('\n').map(String::from).collect() };
        let step = f.line * sy + 4.0;
        let first_top = if t.bottom { t.y - f.line * sy - step * (lines.len() as f32 - 1.0) } else { t.y };
        // positions follow the 640x480 screen across the window; the glyphs keep their shape
        // (sized by the window's height, as on a 4:3 screen)
        let u = |v: f32| Val::Vh(v / 480.0 * 100.0);
        let mut spawn = Vec::new();
        for (li, line) in lines.iter().enumerate() {
            let w = f.width(line, t.scale);
            let x0 = match t.align { Align::Left => 0.0, Align::Centre => -w * 0.5, Align::Right => -w };
            let top = first_top + step * li as f32 - t.y;
            for pass in 0..if t.shadow { 2 } else { 1 } {
                let (d, col) = if t.shadow && pass == 0 { (2.0, Color::BLACK.with_alpha(t.color.alpha())) } else { (0.0, t.color) };
                let mut pen = x0;
                for c in line.chars() {
                    let Some(g) = f.glyphs.get(&c) else { continue };
                    if g.w > 1.0 {
                        spawn.push((ImageNode { image: f.image.clone(), rect: Some(Rect::new(g.x, g.y, g.x + g.w, g.y + g.h)), color: col, ..default() },
                            Node { position_type: PositionType::Absolute, left: vw(t.x), top: vh(t.y),
                                margin: UiRect { left: u(pen + g.xoff * sx + d), top: u(top + g.yoff * sy + d), ..default() },
                                width: u(g.w * sx), height: u(g.h * sy), ..default() }, GlyphNode));
                    }
                    pen += g.adv * sx;
                }
            }
        }
        commands.entity(e).with_children(|p| { for b in spawn { p.spawn(b); } });
    }
}
