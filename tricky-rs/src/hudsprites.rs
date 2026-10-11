//! The in-race HUD's own sprites (`HUD_DrawPlayerSprites` 0x1a2c40, sprite table built by
//! `SpriteSet_InitHudGameRects` 0x1f4440 on DATA/TEXTURES/HUDGAME.SSH): the boost meter's fourteen
//! segments, the TRICKY letters and the uber icon, laid out in the game's 640x480 screen.
//! Needs `chars/hud/{map1,map4,hud1}.png` and `chars/hud/sprites.json`; without them the plain
//! meter in `ui.rs` stays.

use crate::{ui, CharLib, Mode, RiderRes};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Sprite { sheet: usize, rect: Rect, w: f32, h: f32 }

#[derive(Component, Clone, Copy)]
pub enum HudSprite { Segment(usize), Letter(usize), Uber }

#[derive(Resource, Default)]
pub struct HudSprites { loaded: bool, on: bool, sheets: Vec<Handle<Image>>, sprites: HashMap<u32, Sprite> }

/// The meter's base point and its segments' heights (from the base) in the 640x480 HUD.
const BASE: (f32, f32) = (570.0, 140.0);
const SEG_Y: [f32; 14] = [288.0, 270.0, 252.0, 234.0, 216.0, 188.0, 170.0, 152.0, 134.0, 106.0, 88.0, 70.0, 52.0, 34.0];
/// The TRICKY letters around the meter's top: base + (0, 15) + these.
const LETTERS: [(f32, f32); 6] = [(-42.0, -25.0), (-27.0, -35.0), (-13.0, -43.0), (2.0, -42.0), (20.0, -41.0), (35.0, -24.0)];

fn vw(x: f32) -> Val { Val::Vw(x / 640.0 * 100.0) }
fn vh(y: f32) -> Val { Val::Vh(y / 480.0 * 100.0) }

/// The HUD's sprites (`HUD_DrawPlayerSprites` 0x1a2c40, rectangles from `SpriteSet_InitHudGameRects` 0x1f4440).
#[allow(clippy::too_many_arguments)]
pub fn hud_sprites(
    mut commands: Commands, lib: Res<CharLib>, rider: Res<RiderRes>, game: Res<ui::Game>, mode: Res<Mode>, time: Res<Time>,
    mut hs: ResMut<HudSprites>, mut images: ResMut<Assets<Image>>, root: Query<Entity, With<ui::HudRoot>>,
    mut nodes: Query<(&HudSprite, &mut Node, &mut ImageNode, &mut Visibility)>, mut plain: Query<&mut Visibility, (With<ui::BoostSeg>, Without<HudSprite>)>,
    mut shown: Local<f32>,
) {
    if !hs.loaded {
        hs.loaded = true;
        let dir = lib.dir.join("hud");
        let names = ["map1", "map4", "hud1"];
        let sheets: Vec<Option<Handle<Image>>> = names.iter().map(|n| {
            let img = image::open(dir.join(format!("{n}.png"))).ok()?.to_rgba8();
            let (w, h) = img.dimensions();
            Some(images.add(Image::new(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                bevy::render::render_resource::TextureDimension::D2, img.into_raw(), bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::default())))
        }).collect();
        let Ok(text) = std::fs::read_to_string(dir.join("sprites.json")) else { return };
        let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(&text) else { return };
        if sheets.iter().any(|s| s.is_none()) { return; }
        hs.sheets = sheets.into_iter().flatten().collect();
        for e in list {
            let g = |k: &str| e.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let sheet = match e.get("sheet").and_then(|s| s.as_str()).unwrap_or("") { s if s.ends_with("map1") => 0, s if s.ends_with("map4") => 1, s if s.ends_with("hud1") => 2, _ => continue };
            let (x, y, w, h) = (g("x"), g("y"), g("w"), g("h"));
            hs.sprites.insert(g("id") as u32, Sprite { sheet, rect: Rect::new(x, y, x + w, y + h), w: g("draw_w").max(w), h: g("draw_h").max(h) });
        }
        let Ok(root) = root.single() else { return };
        hs.on = true;
        let mk = |hs: &HudSprites, id: u32| -> Option<ImageNode> {
            let s = hs.sprites.get(&id)?;
            Some(ImageNode { image: hs.sheets[s.sheet].clone(), rect: Some(s.rect), ..default() })
        };
        commands.entity(root).with_children(|p| {
            for i in 0..14 {
                if let Some(img) = mk(&hs, 0x15) { p.spawn((img, Node { position_type: PositionType::Absolute, ..default() }, HudSprite::Segment(i))); }
            }
            for i in 0..6 {
                if let Some(img) = mk(&hs, 25 + i as u32) { p.spawn((img, Node { position_type: PositionType::Absolute, ..default() }, HudSprite::Letter(i))); }
            }
            if let Some(img) = mk(&hs, 0x26) { p.spawn((img, Node { position_type: PositionType::Absolute, ..default() }, HudSprite::Uber)); }
        });
    }
    if !hs.on { return; }
    for mut v in plain.iter_mut() { *v = Visibility::Hidden; }
    let r = &rider.0;
    let riding = *mode == Mode::Ride && game.screen == ui::Screen::Playing;
    // the shown level slews 0.005 a frame toward the meter
    let want = if r.tricky() { 1.0 } else { r.boost.clamp(0.0, 1.0) };
    let step = 0.3 * time.delta_secs();
    *shown += (want - *shown).clamp(-step, step);
    // place a sprite centred on a 640x480 point at a scale
    let place = |node: &mut Node, img: &mut ImageNode, s: &Sprite, sheet: &Handle<Image>, at: (f32, f32), k: f32| {
        img.image = sheet.clone();
        img.rect = Some(s.rect);
        node.width = vw(s.w * k);
        node.height = vw(s.h * k);
        node.left = vw(at.0 - s.w * k * 0.5);
        node.top = vh(at.1) ;
        node.margin = UiRect { top: Val::Vw(-(s.h * k * 0.5) / 640.0 * 100.0), ..default() };
    };
    for (kind, mut node, mut img, mut vis) in nodes.iter_mut() {
        *vis = if riding { Visibility::Inherited } else { Visibility::Hidden };
        match *kind {
            HudSprite::Segment(i) => {
                let f = (*shown * 14.0 - i as f32).clamp(0.0, 1.0);
                let id = if f > 0.0 { if i < 5 { 0x14 } else if i < 9 { 0x13 } else { 0x12 } } else { 0x15 };
                let k = if f > 0.0 { 0.2 + 0.8 * f } else { 1.0 };
                if let Some(s) = hs.sprites.get(&id) { place(&mut node, &mut img, s, &hs.sheets[s.sheet], (BASE.0, BASE.1 + SEG_Y[i]), k); }
                // the whole meter at half strength when empty
                img.color = Color::srgba(1.0, 1.0, 1.0, if *shown <= 0.0 { 0.5 } else { 1.0 });
            }
            HudSprite::Letter(i) => {
                let id = if (i as u8) < r.letters { 31 + i as u32 } else { 25 + i as u32 };
                let at = (BASE.0 + LETTERS[i].0, BASE.1 + 15.0 + LETTERS[i].1);
                if let Some(s) = hs.sprites.get(&id) { place(&mut node, &mut img, s, &hs.sheets[s.sheet], at, 1.0); }
            }
            HudSprite::Uber => {
                // lit while ubers can be started; it rises a little as the window opens
                let on = r.uber_ready();
                let id = if on { 0x25 } else { 0x26 };
                let lift = if on && !r.tricky() { 40.0 * ((20.0 - r.uber_timer) / 0.7).clamp(0.0, 1.0) } else { 0.0 };
                if let Some(s) = hs.sprites.get(&id) { place(&mut node, &mut img, s, &hs.sheets[s.sheet], (BASE.0, BASE.1 + 10.0 - lift), 1.25); }
                img.color = Color::srgba(1.0, 1.0, 1.0, if on { 1.0 } else { 0.7 });
            }
        }
    }
}
