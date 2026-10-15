//! The menus' shared drawing (board row G8f): the backdrop's frame, the menu list and the label box, as the draw
//! calls they make (`replayui::DrawEnv` / `PanelEnv`), checked in the function runner (`tests/menudraw.rs`).

use crate::pf::Mem;
use crate::ps2f::F;
use crate::replayui::{PanelEnv, FONT_COLOR};

/// What the menus draw with besides `PanelEnv`
pub trait MenuDrawEnv: PanelEnv {
    /// `Sprite_DrawRectScaled` 0x14eed0 (x, y, the scales, the sprite's rectangle, the corner)
    #[allow(clippy::too_many_arguments)]
    fn sprite_scaled(&mut self, x: f32, y: f32, sx: f32, sy: f32, rect: u32, corner: u32);
    /// The renderer's virtual at `slot` with two integers and a float (f12)
    fn renderer_scaled(&mut self, slot: u32, a1: u32, a2: u32, f: f32);
}

/// The menu's pulse (the selected item's scale, 0.99..1.1) and its step
pub const PULSE: u32 = 0x338208;
pub const PULSE_STEP: u32 = 0x33820c;

fn fl(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}

/// The font colour's first word times the backdrop's contents alpha (+0x70)
fn fade(m: &mut Mem, b: u32) {
    let v = fl(m, FONT_COLOR) * fl(m, b + 0x70);
    m.w32(FONT_COLOR, v.0.to_bits());
}

/// `Backdrop_DrawFrame` 0x1a9bb8 (the height, the backdrop, the layer): the height to +0x2c (+0x34 while opening),
/// then three dark panels (alpha +0x50, 0.1 grey) and the four corners of sprite 55 scaled to 340 wide, around
/// the backdrop's x (+0x38) and y (+0x48)
pub fn draw_frame(m: &mut Mem, env: &mut dyn MenuDrawEnv, height: f32, b: u32, layer: u32) {
    if m.r32(b + 0x60) & 0x40 != 0 {
        m.w32(b + 0x34, height.to_bits());
    } else {
        m.w32(b + 0x2c, height.to_bits());
    }
    m.w32(FONT_COLOR + 0x10, 0x3e19999a);
    env.renderer(0x1c4, 1, 0);
    env.renderer_scaled(0x1cc, 2, 2, f32::from_bits(0x3da3d70a));
    env.renderer(0x1e4, 3, 0);
    env.renderer(0x1fc, 1, 0);
    let w = fl(m, env.sprite_rect(m.r32(b), 55) + 4);
    let h = fl(m, env.sprite_rect(m.r32(b), 55) + 8);
    let scale = F(340.0) / w;
    let half = fl(m, b + 0x2c) * F(0.5);
    let x = fl(m, b + 0x38);
    let bottom = (half + F(240.0)) - h;
    let alpha = fl(m, b + 0x50);
    let full = scale * w;
    let left = x - F(20.0);
    let top = F(240.0) - half;
    let h21 = h * F(f32::from_bits(0x3e570a3d));
    let h26 = h * F(f32::from_bits(0x3e851eb8));
    let right = x + F(320.0);
    let inner = full * F(f32::from_bits(0x3ee8f5c3));
    let tenth = f32::from_bits(0x3dcccccd);
    env.set_color([alpha.0, tenth, tenth, tenth]);
    let mid = left + inner;
    m.w32(FONT_COLOR, alpha.0.to_bits());
    env.renderer(0x1d4, layer, 0);
    env.renderer(0x1f4, u32::MAX, 0);
    let y = fl(m, b + 0x48);
    let hh = fl(m, b + 0x2c);
    env.fill_rect(left.0, (((y + top) + h) - h21).0, 680.0, ((((hh - h) - h) + h21) + h21).0);
    let width = (full - inner) + (full - inner);
    let tall = (h - h21) - h26;
    let y = fl(m, b + 0x48);
    env.fill_rect(mid.0, ((y + top) + h26).0, width.0, tall.0);
    let y = fl(m, b + 0x48);
    env.fill_rect(mid.0, ((y + bottom) + h21).0, width.0, tall.0);
    env.set_color([1.0; 4]);
    env.renderer(0x1d4, layer.wrapping_add(1), 0);
    for (cx, cy, corner) in [(left, top, 2), (right, top, 3), (left, bottom, 0), (right, bottom, 1)] {
        let y = fl(m, b + 0x48);
        env.sprite(cx.0, (y + cy).0, scale.0, 1.0, b, 55, corner);
    }
}

/// `Overlay_DrawMenuList` 0x1aa630 (x, y, the scale; the backdrop, the alignment, the title, the items, their
/// count, the selection, the pointer sprite; the pulse on, the items' enabled flags): flags 2 / 8 put the list
/// above / centred on y, 1 / 4 right / centred on x (by the widest item); the title at 1.2 scale centred above; the
/// pointer sprite left of the selection; the items in white (dimmed when disabled), the selection gold and pulsing
#[allow(clippy::too_many_arguments)]
pub fn draw_menu_list(
    m: &mut Mem,
    env: &mut dyn MenuDrawEnv,
    x: f32,
    y: f32,
    scale: f32,
    b: u32,
    align: u32,
    title: u32,
    items: &[u32],
    sel: i32,
    pointer: i32,
    pulse_on: bool,
    enabled: &[u32],
) {
    if pulse_on && enabled[sel as usize] != 0 {
        let p = fl(m, PULSE);
        if f32::from_bits(0x3f8ccccd) < p.0 || p.0 < f32::from_bits(0x3f7d70a4) {
            let s = -fl(m, PULSE_STEP);
            m.w32(PULSE_STEP, s.0.to_bits());
        }
        let p = fl(m, PULSE) + fl(m, PULSE_STEP);
        m.w32(PULSE, p.0.to_bits());
    }
    env.renderer(0x1e4, 3, 0);
    env.renderer(0x1fc, 1, 0);
    let font = m.r32(b + 4);
    let fh = |m: &Mem| F(m.r32(m.r32(b + 4) + 0xc) as i32 as f32);
    let line = fh(m) * F(scale) + F(4.0);
    let n = items.len();
    let mut top = F(y);
    if align & 10 != 0 {
        let mut tall = F(n as i32 as f32) * line;
        if title != 0 {
            tall = tall + ((line * F(f32::from_bits(0x3f99999a))) + F(9.0));
        }
        top = if align & 2 != 0 { F(y) - tall } else { F(y) - tall * F(0.5) };
    }
    let mut widest = F(0.0);
    let mut left = F(x);
    if title != 0 || align & 5 != 0 {
        for &s in items {
            let w = F(env.measure_wide(scale, m.r32(b + 4), s) as f32);
            if widest.0 < w.0 {
                widest = w;
            }
        }
        if align & 1 != 0 {
            left = F(x) - widest;
        } else if align & 4 != 0 {
            left = F(x) - widest * F(0.5);
        }
    }
    let big = F(f32::from_bits(0x3f99999a));
    if title != 0 {
        let centre = left + widest * F(0.5);
        let w = F(env.measure_wide(big.0, font, title) as f32);
        env.set_color([1.0; 4]);
        let tx = centre - w * F(0.5);
        fade(m, b);
        env.print(0x19d568, big.0, tx.0, top.0, m.r32(b + 4), title);
        top = top + ((fh(m) * big) + F(9.0));
    }
    if pointer >= 0 {
        env.set_color([1.0; 4]);
        fade(m, b);
        let py = if sel >= 0 {
            (top + line * F(sel as f32)) + (fh(m) * F(scale)) * F(0.5)
        } else {
            top + (fh(m) * F(scale)) * F(0.5)
        };
        let r = env.sprite_rect(m.r32(b), pointer as u32);
        let s = f32::from_bits(0x3f666666);
        env.sprite_scaled((left + F(-16.0)).0, py.0, s, s, r, 0x60);
    }
    let p4 = f32::from_bits(0x3ecccccd);
    for (i, &s) in items.iter().enumerate() {
        if i as i32 == sel {
            env.set_color([1.0, f32::from_bits(0x3f73f7cf), f32::from_bits(0x3f3a9fbe), f32::from_bits(0x3dd91687)]);
            let h = fh(m) * F(scale);
            let grow = ((h * fl(m, PULSE)) - h) * F(0.5);
            if enabled[i] == 0 {
                env.set_color([1.0, p4, p4, p4]);
            }
            fade(m, b);
            env.print(0x19d568, (F(scale) * fl(m, PULSE)).0, left.0, (top - grow).0, m.r32(b + 4), s);
        } else {
            env.set_color([1.0; 4]);
            if enabled[i] == 0 {
                env.set_color([1.0, p4, p4, p4]);
            }
            fade(m, b);
            env.print(0x19d568, scale, left.0, top.0, m.r32(b + 4), s);
        }
        top = top + line;
    }
}

/// `HUD_DrawLabelBox` 0x1a9928 (y, the backdrop, the string, the layer): a box of three sprites (62 / 63 stretched
/// to the text / 64) at the backdrop's edge offsets (+0x80 x, +0x88 y) plus y, and the text in it (scale 0.79)
pub fn draw_label_box(m: &mut Mem, env: &mut dyn MenuDrawEnv, y: f32, b: u32, s: u32, layer: u32) {
    let zero = F(0.0);
    let ts = f32::from_bits(0x3f4a3d71);
    let (sw, sh) = (f32::from_bits(0x3f666666), f32::from_bits(0x3f5eb852));
    env.renderer(0x1c4, 0, 0);
    env.renderer(0x1e4, 3, 0);
    env.renderer(0x1fc, 1, 0);
    env.set_color([1.0; 4]);
    env.renderer(0x1d4, layer, 0);
    let w = F(env.measure_wide(ts, m.r32(b + 8), s) as f32);
    let x0 = fl(m, b + 0x80) - zero;
    let yy = fl(m, b + 0x88) + F(y);
    let width = (w + F(-2.0)) + F(10.0);
    env.sprite(x0.0, yy.0, sw, sh, b, 62, 0);
    let cap = fl(m, env.sprite_rect(m.r32(b), 62) + 4) * F(sw);
    let x0 = fl(m, b + 0x80) - zero;
    let yy = fl(m, b + 0x88) + F(y);
    env.sprite((x0 + cap).0, yy.0, width.0, sh, b, 63, 0);
    let x0 = fl(m, b + 0x80) - zero;
    let yy = fl(m, b + 0x88) + F(y);
    env.sprite(((x0 + cap) + width).0, yy.0, sw, sh, b, 64, 0);
    env.set_color([1.0; 4]);
    env.renderer(0x1d4, layer.wrapping_add(2), 0);
    let tx = ((fl(m, b + 0x80) + cap) - zero) + F(-2.0);
    let ty = (fl(m, b + 0x88) + F(y)) + F(7.0);
    env.print(0x19d568, ts, tx.0, ty.0, m.r32(b + 8), s);
}
