//! The overlay manager (`cOverlayMan`, board row G8b), on the game's bytes (`pf::Mem`), checked in the function
//! runner (`tests/overlays.rs`).
//!
//! The manager holds nineteen overlays at +0x10 + 4 i (`OVERLAYS`). Every overlay begins as a `cScreenOverlay`:
//! +0 the sprite set (the game's +0x728), +4 / +8 the fonts (+0x60 / +0x5c), +0xc the players it shows for (a bit
//! each; -1 all, drawn once in the last view), +0x10 drawn with the HUD, +0x14 active, +0x18 cleared on hiding,
//! +0x1c / +0x20 the repeat timers, +0x28 the vtable. Its virtuals (slot: vtable offset, the `this` adjustment 4
//! before): +0x14 the replay's, +0x1c pause, +0x24 results, +0x2c pop finish, +0x34 point distribution, +0x3c next
//! event, +0x44 next venue, +0x4c next race, +0x5c / +0x64 two all-overlay hooks, +0x6c draw, +0x74 draw text, +0x7c
//! update, +0x8c hidden, +0x94 whether it hides itself.

use crate::pf::Mem;
use crate::replay::ea_mem_set;
use crate::replaymgr::GAPP;

/// The pad the overlays read (`DAT_003381e0`)
pub const PAD: u32 = 0x3381e0;
/// The overlays: the HUD, results, high-score record, next race, countdown, the replay's controls, pause, pop
/// finish, pop async load, the replay save page, lesson, view high scores, black fade, unlocked, lesson controller,
/// point distribution, next venue, next event, race help
pub const OVERLAYS: u32 = 19;

/// The manager's vtable and the overlays' own
pub const VT_MANAGER: u32 = 0x38fca8;
pub const VT_SCREEN: u32 = 0x38fc10;
pub const VT_BACKDROP: u32 = 0x38fb78;
pub const VT_COUNTDOWN: u32 = 0x38f030;
pub const VT_POP_FINISH: u32 = 0x38ef98;
pub const VT_POP_ASYNC: u32 = 0x38ef00;
pub const VT_LESSON: u32 = 0x38edd0;
pub const VT_BLACK_FADE: u32 = 0x38ee68;
pub const VT_LESSON_CONTROLLER: u32 = 0x38ed38;
/// The three save pages' backdrop and own vtables: point distribution, next venue, next event
pub const VT_SAVE_PAGES: [(u32, u32); 3] = [(0x38ead8, 0x38eb70), (0x38e878, 0x38e910), (0x38e618, 0x38e6b0)];
/// The heap name the manager allocates under
pub const HEAP_NAME: u32 = 0x38e2b0;

/// What the manager calls out to
pub trait OverlayManEnv {
    /// A virtual at a vtable slot: the function, `this` (with its adjustment), the integer and float arguments
    fn virtual_call(&mut self, m: &mut Mem, func: u32, this: u32, args: &[u32], floats: &[f32]) -> u32;
    /// A function of the game called directly (constructors and the like) with its arguments: its result
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// `EAMem_Alloc` 0x2ccf70 (the name, the size, 0)
    fn alloc(&mut self, m: &mut Mem, name: u32, size: u32) -> u32;
    /// `Input_GetDevice` 0x17a830 (the input, the pad): the pad's state
    fn device(&mut self, pad: u32) -> u32;
    /// `Race_IsPlayerPad` 0x110f28 (the race's riders, the pad) / `Input_GetDevicePressed` 0x17a898 (the input, the pad)
    fn is_player_pad(&mut self, pad: u32) -> bool;
    fn device_pressed(&mut self, pad: u32) -> bool;
}

fn ov(m: &Mem, man: u32, i: u32) -> u32 {
    m.r32(man + 0x10 + 4 * i)
}

/// An object's virtual at `slot` (its vtable at `vt_at`)
pub fn vcall(m: &mut Mem, env: &mut dyn OverlayManEnv, obj: u32, vt_at: u32, slot: u32, args: &[u32], floats: &[f32]) -> u32 {
    let vt = m.r32(vt_at);
    let func = m.r32(vt + slot);
    let this = obj.wrapping_add(m.r16(vt + slot - 4) as i16 as i32 as u32);
    env.virtual_call(m, func, this, args, floats)
}

/// An overlay's virtual (its vtable at +0x28)
fn ovcall(m: &mut Mem, env: &mut dyn OverlayManEnv, o: u32, slot: u32, args: &[u32]) -> u32 {
    vcall(m, env, o, o + 0x28, slot, args, &[])
}

/// The renderer's virtual (the object at the game's +0x724, its vtable at +0x10)
fn rcall(m: &mut Mem, env: &mut dyn OverlayManEnv, slot: u32, args: &[u32], floats: &[f32]) -> u32 {
    let r = m.r32(m.r32(GAPP) + 0x724);
    vcall(m, env, r, r + 0x10, slot, args, floats)
}

/// `ScreenOverlay_Init` 0x1a7d78
pub fn screen_overlay_init(m: &mut Mem, o: u32) {
    let app = m.r32(GAPP);
    m.w32(o + 0x28, VT_SCREEN);
    m.w32(o + 4, m.r32(app + 0x60));
    m.w32(o, m.r32(app + 0x728));
    m.w32(o + 0xc, 0);
    m.w32(o + 8, m.r32(app + 0x5c));
    m.w32(o + 0x18, 0);
}

/// `BackdropOverlay_Init` 0x1a8900: a screen overlay with the backdrop's vtable, its animation (`screen_anim_init`
/// at +0x94) and defaults (+0x50 / +0x58 0.65, +0x70 1.0, +0x60 / +0x7c / +0x90 1)
pub fn backdrop_init(m: &mut Mem, o: u32) {
    screen_overlay_init(m, o);
    m.w32(o + 0x28, VT_BACKDROP);
    screen_anim_init(m, o + 0x94);
    m.w32(o + 0x58, 0x3f266666);
    m.w32(o + 0x70, 0x3f800000);
    m.w32(o + 0x90, 1);
    m.w32(o + 0x328, 0);
    m.w32(o + 0x60, 1);
    for k in [0x38, 0x3c, 0x40, 0x48, 0x4c] {
        m.w32(o + k, 0);
    }
    m.w32(o + 0x50, 0x3f266666);
    for k in [0x54, 0x64, 0x68, 0x6c, 0x74, 0x5c] {
        m.w32(o + k, 0);
    }
    m.w32(o + 0x7c, 1);
    for k in [0x80, 0x84, 0x88, 0x8c] {
        m.w32(o + k, 0);
    }
}

/// A block from the manager's heap, filled with 0xdeadc0de (`EAMem_Set`) when given
fn alloc(m: &mut Mem, env: &mut dyn OverlayManEnv, size: u32) -> u32 {
    let p = env.alloc(m, HEAP_NAME, size);
    if p != 0 {
        let mut b = vec![0u8; size as usize];
        ea_mem_set(&mut b, p, 0xdeadc0de);
        for (k, v) in b.iter().enumerate() {
            m.w8(p + k as u32, *v);
        }
    }
    p
}

fn flags(m: &mut Mem, o: u32, drawn: u32, active: u32) {
    m.w32(o + 0x10, drawn);
    m.w32(o + 0x14, active);
}

/// A save page (point distribution, next venue, next event): its backdrop inside it, the save overlay, the vtables
fn save_page(m: &mut Mem, env: &mut dyn OverlayManEnv, size: u32, backdrop_at: u32, clear: Option<u32>, vts: (u32, u32)) -> u32 {
    let p = alloc(m, env, size);
    m.w32(p + 0x42c, p + backdrop_at);
    backdrop_init(m, p + backdrop_at);
    env.call(m, 0x1b8568, &[p, 0]);
    if let Some(c) = clear {
        m.w32(p + c, 0);
        let o = if p != 0 { m.r32(p + 0x42c) } else { 0 };
        m.w32(m.r32(p + 0x42c) + 0x28, vts.0);
        m.w32(p + 0x428, vts.1);
        o
    } else {
        let b = m.r32(p + 0x42c);
        m.w32(b + 0x28, vts.0);
        let o = if p != 0 { b } else { 0 };
        m.w32(p + 0x440, 0);
        m.w32(p + 0x428, vts.1);
        o
    }
}

/// `OverlayMan_Init` 0x1a6fb8: the node (`cBxObjNode`), the manager's vtable, then each overlay allocated, built and
/// flagged (drawn with the HUD, active), and the HUD's sprite rectangles and the overlays' textures (0x2ca for game
/// mode 6 at 0x32de8c, else 0x2cb)
pub fn man_init(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32) {
    env.call(m, 0x17cf50, &[man]);
    m.w32(man + 8, VT_MANAGER);
    // the HUD
    let p = alloc(m, env, 0x618);
    let o = env.call(m, 0x19e888, &[p]);
    m.w32(o + 0x14, 0);
    m.w32(man + 0x10, o);
    m.w32(o + 0x10, 0);
    // results
    let p = alloc(m, env, 0x8e0);
    let r = env.call(m, 0x1af470, &[p, 1]);
    let o = if r == 0 { 0 } else { m.r32(m.r32(r + 4) + 0x42c) };
    m.w32(o + 0x14, 0);
    m.w32(man + 0x14, o);
    m.w32(o + 0x10, 1);
    // high-score record, next race (the overlay at the object's +0)
    for (size, func, at, drawn) in [(0x460u32, 0x1b6b90u32, 0x18u32, 1u32), (0x5a0, 0x1b3f50, 0x1c, 0)] {
        let p = alloc(m, env, size);
        let r = env.call(m, func, &[p, 1]);
        let o = if r != 0 { m.r32(r) } else { 0 };
        if drawn == 1 {
            flags(m, o, 1, 0);
        } else {
            m.w32(o + 0x14, 0);
        }
        m.w32(man + at, o);
        if drawn == 0 {
            m.w32(o + 0x10, 0);
        }
    }
    // countdown
    let o = alloc(m, env, 0x38);
    screen_overlay_init(m, o);
    m.w32(o + 0x14, 0);
    m.w32(man + 0x20, o);
    m.w32(o + 0x10, 0);
    m.w32(o + 0x28, VT_COUNTDOWN);
    // the replay's controls
    let p = alloc(m, env, 0x4ac);
    let o = env.call(m, 0x1eb5d0, &[p]);
    m.w32(o + 0x14, 0);
    m.w32(man + 0x24, o);
    m.w32(o + 0x10, 0);
    // pause
    let p = alloc(m, env, 0xb68);
    let r = env.call(m, 0x1ab9f8, &[p, 1]);
    let o = if r != 0 { m.r32(r + 0x42c) } else { 0 };
    m.w32(man + 0x28, o);
    flags(m, o, 1, 1);
    // pop finish, pop async load
    for (size, at, vt) in [(0xa4u32, 0x2cu32, VT_POP_FINISH), (0x30, 0x30, VT_POP_ASYNC)] {
        let o = alloc(m, env, size);
        screen_overlay_init(m, o);
        m.w32(o + 0x14, 0);
        m.w32(man + at, o);
        m.w32(o + 0x10, 0);
        m.w32(o + 0x28, vt);
    }
    // the replay save page
    let p = alloc(m, env, 0x794);
    let r = env.call(m, 0x1ef610, &[p, 1]);
    let o = if r != 0 { m.r32(r + 0x42c) } else { 0 };
    m.w32(man + 0x34, o);
    m.w32(o + 0x14, 1);
    m.w32(o + 0x10, 0);
    // lesson
    let o = alloc(m, env, 0x348);
    backdrop_init(m, o);
    flags(m, o, 1, 0);
    m.w32(man + 0x38, o);
    m.w32(o + 0x28, VT_LESSON);
    // view high scores
    let p = alloc(m, env, 0x39c);
    let r = env.call(m, 0x1b38f0, &[p, 1]);
    let o = if r != 0 { m.r32(r) } else { 0 };
    m.w32(o + 0x14, 0);
    m.w32(man + 0x3c, o);
    m.w32(o + 0x10, 1);
    // black fade
    let o = alloc(m, env, 0x2c);
    screen_overlay_init(m, o);
    m.w32(o + 0x14, 0);
    m.w32(man + 0x40, o);
    m.w32(o + 0x10, 0);
    m.w32(o + 0x28, VT_BLACK_FADE);
    // unlocked
    let p = alloc(m, env, 0x5b0);
    let r = env.call(m, 0x1b58c0, &[p, 1]);
    let o = if r != 0 { m.r32(r) } else { 0 };
    m.w32(o + 0x14, 0);
    m.w32(man + 0x44, o);
    m.w32(o + 0x10, 1);
    // lesson controller
    let o = alloc(m, env, 0x348);
    backdrop_init(m, o);
    m.w32(o + 0x14, 0);
    m.w32(man + 0x48, o);
    m.w32(o + 0x10, 0);
    m.w32(o + 0x28, VT_LESSON_CONTROLLER);
    // point distribution, next venue, next event
    let o = save_page(m, env, 0x794, 0x44c, None, VT_SAVE_PAGES[0]);
    m.w32(o + 0x14, 0);
    m.w32(man + 0x4c, o);
    m.w32(o + 0x10, 1);
    for (size, b, clear, vts, at) in [(0x8b8u32, 0x570u32, 0x55cu32, VT_SAVE_PAGES[1], 0x50u32), (0x7c0, 0x478, 0x464, VT_SAVE_PAGES[2], 0x54)] {
        let o = save_page(m, env, size, b, Some(clear), vts);
        flags(m, o, 1, 0);
        m.w32(man + at, o);
    }
    // race help
    let p = alloc(m, env, 0x39c);
    let r = env.call(m, 0x1bd190, &[p, 1]);
    let o = if r != 0 { m.r32(r) } else { 0 };
    m.w32(o + 0x14, 0);
    let app = m.r32(GAPP);
    m.w32(man + 0x58, o);
    let set = m.r32(app + 0x728);
    m.w32(o + 0x10, 0);
    env.call(m, 0x1f4440, &[set]);
    let tex = if m.r32(0x32de8c) == 6 { 0x2ca } else { 0x2cb };
    env.call(m, 0x1f99f8, &[m.r32(m.r32(GAPP) + 0x728), tex, 0]);
}

/// `Overlay_HideForPlayer` 0x1a7e70 (the player, -1 all): the player's bit off (+0xc); hidden (+0x8c) when that
/// leaves none, or (for an overlay that hides itself, +0x94) when the bit was on; +0x18 cleared
pub fn hide_for_player(m: &mut Mem, env: &mut dyn OverlayManEnv, o: u32, player: i32) {
    let bit = if player < 0 { u32::MAX } else { 1u32 << (player as u32 & 31) };
    let own = ovcall(m, env, o, 0x94, &[]);
    let shown = m.r32(o + 0xc);
    if own == 0 {
        if shown != 0 {
            m.w32(o + 0xc, shown & !bit);
            if shown & !bit == 0 {
                ovcall(m, env, o, 0x8c, &[player as u32]);
            }
        }
    } else {
        let mut shown = shown;
        if bit & shown != 0 {
            ovcall(m, env, o, 0x8c, &[player as u32]);
            shown = m.r32(o + 0xc);
        }
        m.w32(o + 0xc, shown & !bit);
    }
    m.w32(o + 0x18, 0);
}

/// `Overlay_HideAll` 0x1a7758
pub fn hide_all(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32) {
    for i in 0..OVERLAYS {
        let o = ov(m, man, i);
        hide_for_player(m, env, o, -1);
    }
}

/// `cOverlayMan::~cOverlayMan` 0x1a76b8 (the flags): each overlay hidden and deleted (its virtual +0xc with 3), then
/// the node's destructor (`cBxObjNode::~cBxObjNode` 0x17cfe0 with the flags)
pub fn man_destroy(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32, fl: u32) {
    m.w32(man + 8, VT_MANAGER);
    for i in 0..OVERLAYS {
        if ov(m, man, i) != 0 {
            let o = ov(m, man, i);
            hide_for_player(m, env, o, -1);
            let o = ov(m, man, i);
            if o != 0 {
                ovcall(m, env, o, 0xc, &[3]);
            }
        }
    }
    env.call(m, 0x17cfe0, &[man, fl]);
}

/// `OverlayMan_IsAnyVisible` 0x1a77a8 / `_IsAnyActive` 0x1a77f8: an overlay shown (+0xc) and drawn with the HUD
/// (+0x10) / active (+0x14)
pub fn any_visible(m: &Mem, man: u32) -> bool {
    (0..OVERLAYS).any(|i| {
        let o = ov(m, man, i);
        m.r32(o + 0xc) != 0 && m.r32(o + 0x10) != 0
    })
}
pub fn any_active(m: &Mem, man: u32) -> bool {
    (0..OVERLAYS).any(|i| {
        let o = ov(m, man, i);
        m.r32(o + 0xc) != 0 && m.r32(o + 0x14) != 0
    })
}

/// `Overlay_SetPlayerPad` 0x1a7878 (the players): the pad the overlays read, the overlay's players (+0xc)
pub fn set_player_pad(m: &mut Mem, env: &mut dyn OverlayManEnv, o: u32, players: u32) {
    let d = env.device(players);
    m.w32(o + 0xc, players);
    m.w32(PAD, d);
}

/// `cOverlayMan` vf1 0x1a7ad0: each shown overlay updated (+0x7c)
pub fn man_update(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32) {
    for i in 0..OVERLAYS {
        let o = ov(m, man, i);
        if m.r32(o + 0xc) != 0 {
            ovcall(m, env, o, 0x7c, &[]);
        }
    }
}

/// `OverlayMan_CallAllVf10` 0x1a7b30 / `_CallAllVf11` 0x1a7b88: every overlay's +0x5c / +0x64
pub fn call_all(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32, slot: u32) {
    for i in 0..OVERLAYS {
        let o = ov(m, man, i);
        ovcall(m, env, o, slot, &[]);
    }
}

/// The `OverlayMan_Call*` wrappers: one overlay's virtual (the replay save page +0x34's +0x14, pop finish +0x2c's
/// +0x2c, pause +0x28's +0x1c, results +0x14's +0x24, point distribution +0x4c's +0x34, next event +0x54's +0x3c,
/// next race +0x1c's +0x4c, next venue +0x50's +0x44)
pub const CALLS: [(u32, u32, u32); 8] = [
    (0x1a7be0, 0x34, 0x14),
    (0x1a7c10, 0x2c, 0x2c),
    (0x1a7c58, 0x28, 0x1c),
    (0x1a7c88, 0x14, 0x24),
    (0x1a7cb8, 0x4c, 0x34),
    (0x1a7ce8, 0x54, 0x3c),
    (0x1a7d18, 0x1c, 0x4c),
    (0x1a7d48, 0x50, 0x44),
];
pub fn call_one(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32, at: u32, slot: u32) {
    let o = m.r32(man + at);
    ovcall(m, env, o, slot, &[]);
}

/// `OverlayMan_GetReplayControl` 0x1a7c40 / `_GetLesson` 0x1a7c48
pub fn replay_control(m: &Mem, man: u32) -> u32 {
    m.r32(man + 0x24)
}
pub fn lesson(m: &Mem, man: u32) -> u32 {
    m.r32(man + 0x38)
}

/// A word converted to a float as unsigned
fn u2f(v: u32) -> f32 {
    use crate::ps2f::F;
    if (v as i32) >= 0 {
        v as i32 as f32
    } else {
        let h = F(((v >> 1) | (v & 1)) as i32 as f32);
        (h + h).0
    }
}

/// 0x1a7f40 / 0x1a8110 (`Overlay_DrawItemForView` / `_DrawTextForView`): an overlay drawn in the view being drawn
/// (the race's +0x290) when it shows for that player; one for all players (-1) only in the last view (+0x298 − 1),
/// across the whole screen: the renderer into screen space with the viewport the renderer's size (its +0x5c / +0x64,
/// unsigned), the view 640 × 480, the overlay's +0x6c (items) or +0x74 (text), and back
pub fn draw_for_view(m: &mut Mem, env: &mut dyn OverlayManEnv, o: u32, text: bool) {
    let slot = if text { 0x74 } else { 0x6c };
    let shown = m.r32(o + 0xc);
    if shown == 0 {
        return;
    }
    let race = m.r32(m.r32(GAPP) + 0x730);
    let view = m.r32(race + 0x290);
    if shown == u32::MAX {
        if view == m.r32(race + 0x298).wrapping_sub(1) {
            rcall(m, env, 0x8c, &[], &[]);
            let w = u2f(rcall(m, env, 0x5c, &[], &[]));
            let h = u2f(rcall(m, env, 0x64, &[], &[]));
            rcall(m, env, 0x9c, &[], &[0.0, 0.0, w, h]);
            rcall(m, env, 0xb4, &[], &[0.0, 0.0, 640.0, 480.0, 0.0, 1000.0]);
            ovcall(m, env, o, slot, &[]);
            rcall(m, env, 0x94, &[], &[]);
        }
    } else if ((shown as i32) >> (view & 31)) & 1 != 0 {
        ovcall(m, env, o, slot, &[]);
    }
}

/// `cOverlayMan` vf2 0x1a78c0, the overlays' drawing: the renderer's state kept (+0x1dc) and put into screen space
/// (640 × 480), the font's fifth word 0.15 (0x337ff8), then every overlay's items (layer 4) and every overlay's text
/// (layer 9), and the renderer back
pub fn man_draw(m: &mut Mem, env: &mut dyn OverlayManEnv, man: u32) {
    let kept = rcall(m, env, 0x1dc, &[], &[]);
    rcall(m, env, 0x8c, &[], &[]);
    rcall(m, env, 0xb4, &[], &[0.0, 0.0, 640.0, 480.0, 0.0, 1000.0]);
    rcall(m, env, 0x10c, &[], &[]);
    rcall(m, env, 0x1e4, &[0], &[]);
    m.w32(0x337ff8, 0x3e19999a);
    rcall(m, env, 0x1c4, &[1], &[]);
    let f = f32::from_bits(0x3da3d70a);
    rcall(m, env, 0x1cc, &[2, 2], &[f]);
    for i in 0..OVERLAYS {
        rcall(m, env, 0x1d4, &[4], &[]);
        let o = ov(m, man, i);
        draw_for_view(m, env, o, false);
    }
    for i in 0..OVERLAYS {
        rcall(m, env, 0x1d4, &[9], &[]);
        let o = ov(m, man, i);
        draw_for_view(m, env, o, true);
    }
    rcall(m, env, 0x94, &[], &[]);
    rcall(m, env, 0x1cc, &[1, 2], &[f]);
    rcall(m, env, 0x1c4, &[0], &[]);
    rcall(m, env, 0x1d4, &[kept], &[]);
}

/// `Overlay_GetNewButtons` 0x1a8688 (the pad, or -1 for the first player pad with a button down): the buttons newly
/// down, now held (the pad's +0xc gains them); a new press restarts the overlay's repeat timers (+0x1c, +0x20)
pub fn new_buttons(m: &mut Mem, env: &mut dyn OverlayManEnv, o: u32, pad: i32) -> u32 {
    let mut pad = pad;
    if pad < 0 {
        let mut found = -1;
        let mut i = 0;
        while i < m.r32(m.r32(m.r32(GAPP) + 0x24) + 0x14) as i32 {
            if env.is_player_pad(i as u32) && env.device_pressed(i as u32) {
                found = i;
                break;
            }
            i += 1;
        }
        if found < 0 {
            return 0;
        }
        pad = found;
    }
    let d = env.device(pad as u32);
    let (last, held) = (m.r32(d + 0xc), m.r32(d + 0x10));
    let new = held & !last;
    let (mut last, mut held) = (last, held);
    if new != 0 {
        m.w32(o + 0x20, 0);
        m.w32(o + 0x1c, 0);
        last = m.r32(d + 0xc);
        held = m.r32(d + 0x10);
    }
    m.w32(d + 0xc, last | (held & new));
    m.w32(PAD, d);
    new
}

// ---- the backdrop's animation (G8c) ----
//
// A backdrop moves in three parts, each with its own bits (1 still, 2 fading in, 4 / 8 from the right / left, 16 /
// 32 from below / above, 64 opening):
// - the frame (+0x60 the bits): +0x50 its alpha rising by +0x54 to +0x58; +0x38 its x moving by +0x3c, itself
//   accelerating by +0x40 up to +0x44; +0x48 its y by +0x4c; +0x2c its width growing by +0x30 to +0x34 after a delay
//   of +0x5c frames
// - its contents (+0x7c): +0x70 the alpha by +0x74 to 1.0 after a delay of +0x78 frames; +0x64 the x by +0x68
//   (accelerating by +0x6c), written to both fonts' +0x20
// - the edges (+0x90): +0x80 / +0x88 two offsets by +0x84 / +0x8c, to 0

use crate::ps2f::F;

fn fget(m: &Mem, a: u32) -> F {
    F(f32::from_bits(m.r32(a)))
}
fn fput(m: &mut Mem, a: u32, v: F) {
    m.w32(a, v.0.to_bits());
}
const RATE: u32 = 0x3d0b4396;

/// `Backdrop_StartFrameAnim` 0x1a89b0 (the bits added, the alpha to reach or the opening's delay)
pub fn start_frame_anim(m: &mut Mem, o: u32, bits: u32, v: f32) {
    let fl = m.r32(o + 0x60) | bits;
    m.w32(o + 0x60, fl);
    if fl & 2 != 0 {
        m.w32(o + 0x50, 0);
        m.w32(o + 0x54, RATE);
        m.w32(o + 0x58, v.to_bits());
    }
    let fl = m.r32(o + 0x60);
    let slide = if fl & 8 != 0 {
        Some([-640.0f32, 90.0, -5.0, 10.0])
    } else if fl & 4 != 0 {
        Some([640.0, -90.0, 5.0, -10.0])
    } else {
        None
    };
    if let Some(s) = slide {
        for (k, x) in s.iter().enumerate() {
            m.w32(o + 0x38 + 4 * k as u32, x.to_bits());
        }
    }
    let rise = if fl & 16 != 0 {
        Some((-480.0f32, 20.0f32))
    } else if fl & 32 != 0 {
        Some((480.0, -20.0))
    } else {
        None
    };
    if let Some((y, dy)) = rise {
        m.w32(o + 0x48, y.to_bits());
        m.w32(o + 0x4c, dy.to_bits());
    }
    if fl & 64 != 0 {
        m.w32(o + 0x5c, v.to_bits());
        m.w32(o + 0x2c, 60.0f32.to_bits());
        m.w32(o + 0x30, 13.0f32.to_bits());
        m.w32(o + 0x34, 500.0f32.to_bits());
    }
}

/// `Backdrop_StartChildSlide` 0x1a8ad8 (the bits added, the delay)
pub fn start_child_slide(m: &mut Mem, o: u32, bits: u32, delay: f32) {
    m.w32(o + 0x78, delay.to_bits());
    let fl = m.r32(o + 0x7c) | bits;
    m.w32(o + 0x7c, fl);
    if fl & 2 != 0 {
        m.w32(o + 0x70, 0);
        m.w32(o + 0x74, RATE);
    }
    let fl = m.r32(o + 0x7c);
    if fl & 8 != 0 {
        m.w32(o + 0x64, (-640.0f32).to_bits());
        m.w32(o + 0x68, 10.0f32.to_bits());
        m.w32(o + 0x6c, 0);
    } else if fl & 4 != 0 {
        m.w32(o + 0x64, 640.0f32.to_bits());
        m.w32(o + 0x6c, 5.0f32.to_bits());
        m.w32(o + 0x68, (-90.0f32).to_bits());
    }
}

/// `Backdrop_StartEdgeSlide` 0x1a8b70 (the bits added)
pub fn start_edge_slide(m: &mut Mem, o: u32, bits: u32) {
    let fl = m.r32(o + 0x90) | bits;
    m.w32(o + 0x90, fl);
    if fl & 2 != 0 {
        m.w32(o + 0x80, 0xc4548000);
        m.w32(o + 0x84, 25.0f32.to_bits());
    }
    if m.r32(o + 0x90) & 4 != 0 {
        m.w32(o + 0x88, (-680.0f32).to_bits());
        m.w32(o + 0x8c, 16.0f32.to_bits());
    }
}

/// `Backdrop_UpdateFrameAnim` 0x1a8c50
pub fn update_frame_anim(m: &mut Mem, o: u32) {
    let fl = m.r32(o + 0x60);
    let zero = F(0.0);
    if fl & 2 != 0 {
        let a = fget(m, o + 0x50) + fget(m, o + 0x54);
        fput(m, o + 0x50, a);
        if a.0 < 0.0 {
            fput(m, o + 0x50, zero);
            fput(m, o + 0x54, zero);
        }
        let top = fget(m, o + 0x58);
        if top.0 < fget(m, o + 0x50).0 {
            fput(m, o + 0x50, top);
            fput(m, o + 0x54, zero);
        }
    }
    let v = fget(m, o + 0x3c);
    let x = fget(m, o + 0x38) + v;
    let nv = v + fget(m, o + 0x40);
    fput(m, o + 0x38, x);
    fput(m, o + 0x3c, nv);
    if fl & 8 != 0 {
        if 0.0 < x.0 {
            for k in [0x40, 0x38, 0x3c] {
                fput(m, o + k, zero);
            }
        }
        let (v, lim) = (fget(m, o + 0x3c), fget(m, o + 0x44));
        if v.0 < lim.0 {
            fput(m, o + 0x3c, lim);
        }
    } else if fl & 4 != 0 {
        if x.0 < 0.0 {
            for k in [0x40, 0x38, 0x3c] {
                fput(m, o + k, zero);
            }
        }
        let (v, lim) = (fget(m, o + 0x3c), fget(m, o + 0x44));
        if lim.0 < v.0 {
            fput(m, o + 0x3c, lim);
        }
    }
    if fl & 16 != 0 || fl & 32 != 0 {
        let y = fget(m, o + 0x48) + fget(m, o + 0x4c);
        fput(m, o + 0x48, y);
        let done = if fl & 16 != 0 { 0.0 < y.0 } else { y.0 < 0.0 };
        if done {
            fput(m, o + 0x4c, zero);
            fput(m, o + 0x48, zero);
        }
    }
    let delay = fget(m, o + 0x5c);
    if 0.0 < delay.0 {
        fput(m, o + 0x5c, delay - F(1.0));
        return;
    }
    if fl & 64 != 0 {
        let w = fget(m, o + 0x2c) + fget(m, o + 0x30);
        fput(m, o + 0x2c, w);
        let top = fget(m, o + 0x34);
        if top.0 < w.0 {
            fput(m, o + 0x2c, top);
        }
    }
}

/// `Backdrop_UpdateChildSlide` 0x1a8e10: after the delay, the alpha and the slide; the x to both fonts' +0x20
pub fn update_child_slide(m: &mut Mem, o: u32) {
    let zero = F(0.0);
    let delay = fget(m, o + 0x78);
    let mut out;
    if 0.0 < delay.0 {
        fput(m, o + 0x70, zero);
        fput(m, o + 0x78, delay - F(1.0));
        out = fget(m, o + 0x64);
    } else {
        let fl = m.r32(o + 0x7c);
        if fl & 2 != 0 {
            let a = fget(m, o + 0x70) + fget(m, o + 0x74);
            fput(m, o + 0x70, a);
            if a.0 < 0.0 {
                fput(m, o + 0x70, zero);
                fput(m, o + 0x74, zero);
            }
            if 1.0 < fget(m, o + 0x70).0 {
                fput(m, o + 0x70, F(1.0));
                fput(m, o + 0x74, zero);
            }
        }
        let v = fget(m, o + 0x68);
        let x = fget(m, o + 0x64) + v;
        let nv = v + fget(m, o + 0x6c);
        fput(m, o + 0x64, x);
        fput(m, o + 0x68, nv);
        out = x;
        let mut stop = fl & 8 != 0 && 0.0 < x.0;
        if !stop {
            out = fget(m, o + 0x64);
            stop = fl & 4 != 0 && out.0 < 0.0;
        }
        if stop {
            out = zero;
            for k in [0x64, 0x68, 0x6c] {
                fput(m, o + k, zero);
            }
        }
    }
    let (a, b) = (m.r32(o + 4), m.r32(o + 8));
    fput(m, a + 0x20, out);
    let x = fget(m, o + 0x64);
    fput(m, b + 0x20, x);
}

/// `Backdrop_UpdateEdgeSlide` 0x1a8f28: each edge offset in, to 0
pub fn update_edge_slide(m: &mut Mem, o: u32) {
    let fl = m.r32(o + 0x90);
    for (bit, at) in [(2u32, 0x80u32), (4, 0x88)] {
        if fl & bit != 0 {
            let a = fget(m, o + at) + fget(m, o + at + 4);
            fput(m, o + at, a);
            if 0.0 < a.0 {
                m.w32(o + at + 4, 0);
                m.w32(o + at, 0);
            }
        }
    }
}

/// `cBackdropOverlay::Update` 0x1a8bd8: the screen animation (`screen_anim_update` at +0x94), then each part
/// not still (bits 1)
pub fn backdrop_update(m: &mut Mem, o: u32) {
    screen_anim_update(m, o + 0x94);
    if m.r32(o + 0x60) != 1 {
        update_frame_anim(m, o);
    }
    if m.r32(o + 0x7c) != 1 {
        update_child_slide(m, o);
    }
    if m.r32(o + 0x90) != 1 {
        update_edge_slide(m, o);
    }
}

// ---- the screen animation, the menu sounds and the repeat poll (G8d) ----
//
// A screen animation (`ScreenAnim`, 0x294 bytes in a backdrop at +0x94): +0 the frame, +4 the tracks still running,
// +8 the tracks set up, +0xc done, +0x10 the last frame; then twenty tracks of 0x20 bytes from +0x14: +0 the id
// (-1 free), +4 the frame it starts on, +8 a counter, +0xc the target, +0x10 the rate, +0x14 the value, +0x18 the
// base, +0x1c the kind (0 counting up from the base, 1 down, 2 a sine ease, 3 a ramp; 2 and 3 run alongside the
// others and don't hold the animation).

const TRACKS: u32 = 20;
fn track(a: u32, i: u32) -> u32 {
    a + 0x14 + 0x20 * i
}

/// `ScreenAnim_Reset` 0x1f1258
pub fn screen_anim_reset(m: &mut Mem, a: u32) {
    m.w32(a + 0xc, 1);
    m.w32(a + 8, 0);
    m.w32(a, 0);
    m.w32(a + 0x10, 0);
    m.w32(a + 4, 0);
    for i in 0..TRACKS {
        let t = track(a, i);
        m.w32(t, u32::MAX);
        for k in [4, 8, 0xc, 0x10, 0x14] {
            m.w32(t + k, 0);
        }
    }
}

/// `ScreenAnim_Init` 0x1f10c0 (the tracks' constructor 0x1f0e58 does nothing)
pub fn screen_anim_init(m: &mut Mem, a: u32) -> u32 {
    screen_anim_reset(m, a);
    a
}

/// `ScreenAnim_Step` 0x1f0e80 (a track): the value on by one every `rate` frames (rates above 1), by 1 / rate a
/// frame (rates in 0..1), by one for kinds 2 and 3; whether it reached the target
pub fn screen_anim_step(m: &mut Mem, t: u32) -> bool {
    let rate = f32::from_bits(m.r32(t + 0x10));
    let count = m.r32(t + 8).wrapping_add(1);
    m.w32(t + 8, count);
    let mut n = if 1.0 < rate {
        let d = rate as i32;
        ((count as i32).wrapping_rem(d) == 0) as i32
    } else if 0.0 < rate {
        (crate::ps2f::F(1.0) / crate::ps2f::F(rate)).0 as i32
    } else {
        0
    };
    let kind = m.r32(t + 0x1c);
    if kind.wrapping_sub(2) <= 1 {
        n = 1;
    }
    for _ in 0..n.max(0) {
        m.w32(t + 0x14, m.r32(t + 0x14).wrapping_add(1));
    }
    !((m.r32(t + 0x14) as i32) < m.r32(t + 0xc) as i32)
}

/// `ScreenAnimTrack_Eval` 0x1f0f48: kind 0 the base plus the value, 1 the base less the value, 2 halfway between
/// the base and the target (both in thousandths) plus half the difference times the sine of the value over the rate
/// turns (0 before it starts), 3 from the base to the target as the value runs to the rate; others 0
pub fn screen_anim_track_eval(m: &Mem, t: u32) -> f32 {
    use crate::ps2f::F;
    let i = |k: u32| m.r32(t + k) as i32;
    match m.r32(t + 0x1c) {
        0 => i(0x18).wrapping_add(i(0x14)) as f32,
        1 => i(0x18).wrapping_sub(i(0x14)) as f32,
        2 => {
            let rate = F(f32::from_bits(m.r32(t + 0x10)));
            let x = F(i(0x14) as f32) / rate;
            let hi = F(i(0xc) as f32) / F(1000.0);
            let lo = F(i(0x18) as f32) / F(1000.0);
            let s = crate::math::sin((x * F(f32::from_bits(0x40c90fd0))).0);
            let mid = (hi + lo) * F(0.5);
            let half = hi - mid;
            let v = mid + F(s) * half;
            if i(0x14) == 0 { 0.0 } else { v.0 }
        }
        3 => {
            let rate = F(f32::from_bits(m.r32(t + 0x10)));
            let hi = F(i(0xc) as f32) / F(1000.0);
            let lo = F(i(0x18) as f32) / F(1000.0);
            let v = F(i(0x14) as f32);
            let mut u = v / rate;
            let span = hi - lo;
            if rate.0 < v.0 {
                u = F(1.0);
            }
            (lo + span * u).0
        }
        _ => 0.0,
    }
}

/// `ScreenAnim_Update` 0x1f12d0: each set-up track stepped (kinds 2 and 3 from their start frame on, the others on
/// it, counting off the running ones as they finish); then (0x1f13b0) when none is running and it isn't done, the
/// next frame: the tracks starting on it running, and done once past the last frame with none
pub fn screen_anim_update(m: &mut Mem, a: u32) {
    let mut i = 0u32;
    while (i as i32) < m.r32(a + 8) as i32 {
        let t = track(a, i);
        let kind = m.r32(t + 0x1c);
        let frame = m.r32(a) as i32;
        let start = m.r32(t + 4) as i32;
        if kind == 2 || kind == 3 {
            if frame >= start {
                screen_anim_step(m, t);
            }
        } else if frame == start && screen_anim_step(m, t) {
            m.w32(a + 4, m.r32(a + 4).wrapping_sub(1));
        }
        i += 1;
    }
    let mut running = m.r32(a + 4);
    if running != 0 || m.r32(a + 0xc) != 0 {
        return;
    }
    let frame = m.r32(a).wrapping_add(1);
    m.w32(a, frame);
    for k in 0..TRACKS {
        let t = track(a, k);
        if m.r32(t + 4) == frame && m.r32(t + 0x1c).wrapping_sub(2) >= 2 {
            running = running.wrapping_add(1);
            m.w32(a + 4, running);
        }
    }
    if running == 0 && (m.r32(a + 0x10) as i32) < frame as i32 {
        m.w32(a + 0xc, 1);
    }
}

/// `ScreenAnim_GetValue` 0x1f1448 (the id): the track's value, or 0
pub fn screen_anim_value(m: &Mem, a: u32, id: u32) -> u32 {
    (0..TRACKS).find(|&i| m.r32(track(a, i)) == id).map(|i| m.r32(track(a, i) + 0x14)).unwrap_or(0)
}

/// `ScreenAnim_Eval` 0x1f1488 (the id): the track's evaluation, or 0
pub fn screen_anim_eval(m: &Mem, a: u32, id: u32) -> f32 {
    (0..TRACKS).find(|&i| m.r32(track(a, i)) == id).map(|i| screen_anim_track_eval(m, track(a, i))).unwrap_or(0.0)
}

/// The menu sounds (`Overlay_PlayTickSound` 0x1a8638 .. `_PlayErrorSound` 0x1a8678): each the audio system's
/// (0x343050) sound function
pub const MENU_SOUNDS: [(u32, u32); 5] = [(0x1a8638, 0x213b38), (0x1a8648, 0x213d60), (0x1a8658, 0x213f88), (0x1a8668, 0x2138f0), (0x1a8678, 0x2141d0)];
pub fn menu_sound(m: &mut Mem, env: &mut dyn OverlayManEnv, func: u32) {
    let audio = m.r32(0x343050);
    env.call(m, func, &[audio]);
}

/// `PauseMenu_PollRepeatButton` 0x1a8688 (the pad, or -1): the held button, repeated. Pad -1 takes a player pad
/// with a button held: the one holding the overlay's last button (+0x20), else the last found (the game notes each
/// in a branch's delay slot). The button counts as pressed when it is new, then every 12 frames, then every other
/// frame after 60 (+0x1c counts); 0 when none is held
pub fn poll_repeat(m: &mut Mem, env: &mut dyn OverlayManEnv, o: u32, pad: i32) -> u32 {
    let mut pad = pad;
    if pad < 0 {
        let mut i = 0;
        while i < m.r32(m.r32(m.r32(GAPP) + 0x24) + 0x14) as i32 {
            if env.is_player_pad(i as u32) {
                let d = env.device(i as u32);
                let held = m.r32(d + 0x10);
                if held != 0 {
                    pad = i;
                    if m.r32(o + 0x20) == held {
                        break;
                    }
                }
            }
            i += 1;
        }
        if pad < 0 {
            m.w32(o + 0x20, 0);
            m.w32(o + 0x1c, 0);
            return 0;
        }
    }
    let d = env.device(pad as u32);
    let held = m.r32(d + 0x10);
    let out = if held == 0 || m.r32(o + 0x20) != held {
        m.w32(o + 0x20, held);
        m.w32(o + 0x1c, 0);
        held
    } else {
        let n = m.r32(o + 0x1c).wrapping_add(1);
        m.w32(o + 0x1c, n);
        let quiet = if (n as i32) >= 61 { n & 1 != 0 } else { (n as i32) % 12 != 0 };
        if quiet { 0 } else { held }
    };
    m.w32(PAD, d);
    out
}
