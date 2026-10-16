//! The front end's screen base class (board row G9b): `cFE3DState`'s methods on the game's bytes (`pf::Mem`), every
//! call (the screen's and its widgets' virtuals, the front end's pad queries, the fade, the screen stack) through
//! [`FeEnv`], checked in the function runner (`tests/fe3dstate.rs`).
//!
//! The screen: +0xc its vtable, +0x10 the phase (0 hidden, 1 fading in, 2 active, 3 fading out, 4 waiting for the
//! next screen, 5 done), +0x14 the next screen, +0x18 back allowed, +0x1c accept allowed, +0x20 / +0x24 the fade in
//! and out (1 half, 2 slow), +0x28 the pause overlay allowed, +0x30 its menu, +0x34 its widget group, +0x38 the
//! title widget, +0x3c the title text, +0x40 the help bar, +0x44 the backdrop. The front end (`gApp`+0x734): +5 the
//! player who accepted, +0x50 the overlay up (or 0), +0x74 the two players, +0xa4 the 3D scene, +0xa8 the screen
//! stack. A virtual is an 8-byte entry at the vtable: the `this` adjust (a halfword) and the function.

use crate::pf::Mem;

pub const GAPP: u32 = 0x337c58;
/// `FE3DTransition_Start` 0x1ff648 (f12 the in time, f13 the hold, f14 the out, a0 the renderer word gApp+0x72c,
/// a1 the colour (three floats), a2 3)
pub const TRANSITION_START: u32 = 0x1ff648;
/// The front end's pad queries on a player (`FEPlayer_*`): 0x26ce08 the pause button, 0x26ce30 accept, 0x26ce58
/// back, 0x26cef8 any input; 0x26c430 the overlay closed
pub const PAD_PAUSE: u32 = 0x26ce08;
pub const PAD_ACCEPT: u32 = 0x26ce30;
pub const PAD_BACK: u32 = 0x26ce58;
pub const PAD_ANY: u32 = 0x26cef8;
pub const OVERLAY_CLOSE: u32 = 0x26c430;
/// The heap (`MemPool_Alloc` 0x2ccf70: pool, size, align), its fill (0x2cc808: at, word, size), the "back" screen's
/// constructor 0x29e920, the screen stack's push 0x275b10 and pop 0x275c10, 0x2ac030 (the stack's top), 0x271868 (the
/// player's input mask for this screen)
pub const ALLOC: u32 = 0x2ccf70;
pub const FILL: u32 = 0x2cc808;
pub const BACK_SCREEN: u32 = 0x29e920;
pub const STACK_PUSH: u32 = 0x275b10;
pub const STACK_POP: u32 = 0x275c10;
pub const STACK_TOP: u32 = 0x2ac030;
pub const INPUT_MASK: u32 = 0x271868;
/// The widget list's walk (0x2ac018 first, 0x2abf10 at the end, 0x2ac048 the n-th), 0x2acb20 a widget's place,
/// 0x275468 the screen's widget reset
pub const LIST_FIRST: u32 = 0x2ac018;
pub const LIST_END: u32 = 0x2abf10;
pub const LIST_NTH: u32 = 0x2ac048;
pub const WIDGET_PLACE: u32 = 0x2acb20;
pub const WIDGETS_RESET: u32 = 0x275468;

/// What the screen calls out to: a function (a direct call's address, or a virtual's function read from the vtable)
/// with its integer arguments (floats as their bits, as the caller puts them)
pub trait FeEnv {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32;
}

/// A virtual call: the entry at `off` of `obj`'s vtable (+0xc): the adjust halfword at `off`, the function at
/// `off` + 4
pub fn vcall(m: &mut Mem, e: &mut dyn FeEnv, obj: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(0xc));
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    let mut a = vec![obj.wrapping_add(adj)];
    a.extend_from_slice(args);
    e.call(m, f, &a)
}

fn fe(m: &Mem) -> u32 {
    m.r32(m.r32(GAPP).wrapping_add(0x734))
}

/// A widget's shown flag (+0x30) set and its virtual +0x20 (show, 1)
fn show_widget(m: &mut Mem, e: &mut dyn FeEnv, w: u32, on: u32) {
    if w != 0 {
        m.w32(w + 0x30, on);
        vcall(m, e, w, 0x20, &[1]);
    }
}

/// The fade a screen asks for (1: half a second in and out; 2: 1.5 s out) toward `rgb` (three floats at `buf`)
fn fade(m: &mut Mem, e: &mut dyn FeEnv, kind: u32, rgb: f32, buf: u32) {
    let (a, b) = match kind {
        1 => (0.5f32, 0.5f32),
        2 => (0.0, 1.5),
        _ => return,
    };
    for k in 0..3 {
        m.w32(buf + 4 * k, rgb.to_bits());
    }
    let word = m.r32(m.r32(GAPP).wrapping_add(0x72c));
    e.call(m, TRANSITION_START, &[a.to_bits(), 0, b.to_bits(), word, buf, 3]);
}

/// `cFE3DState::Hide` 0x2747b0: the title, help bar and backdrop hidden (each +0x30 0, its show virtual), phase 0;
/// with a help bar or backdrop the 3D scene's virtual +0xd8
pub fn hide(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    for o in [0x38, 0x40] {
        let w = m.r32(s + o);
        show_widget(m, e, w, 0);
    }
    let w = m.r32(s + 0x44);
    let mut bd = 0;
    if w != 0 {
        show_widget(m, e, w, 0);
        bd = m.r32(s + 0x44);
    }
    m.w32(s + 0x10, 0);
    if bd != 0 || m.r32(s + 0x40) != 0 {
        let scene = m.r32(fe(m) + 0xa4);
        vcall_scene(m, e, scene, 0xd8);
    }
}

/// The 3D scene's virtuals go through its vtable at +0 (not +0xc)
fn vcall_scene(m: &mut Mem, e: &mut dyn FeEnv, scene: u32, off: u32) -> u32 {
    let vt = m.r32(scene);
    let adj = m.r16(vt.wrapping_add(off)) as i16 as u32;
    let f = m.r32(vt.wrapping_add(off + 4));
    e.call(m, f, &[scene.wrapping_add(adj)])
}

/// `cFE3DState::TransitionIn` 0x274878 (`buf` the colour's scratch): the fade +0x20 asks for (to white), the
/// screen's virtual +0x98, the title, help bar and backdrop shown, phase 1, the 3D scene's virtual +0xd0
pub fn transition_in(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let k = m.r32(s + 0x20);
    fade(m, e, k, 1.0, buf);
    vcall(m, e, s, 0x98, &[]);
    for o in [0x38, 0x40, 0x44] {
        let w = m.r32(s + o);
        show_widget(m, e, w, 1);
    }
    m.w32(s + 0x10, 1);
    let scene = m.r32(fe(m) + 0xa4);
    vcall_scene(m, e, scene, 0xd0);
}

/// `cFE3DState::TransitionOut` 0x2749f0: the fade +0x24 asks for (to black), the widgets kept shown, phase 3, the 3D
/// scene's virtual +0xd0
pub fn transition_out(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let k = m.r32(s + 0x24);
    fade(m, e, k, 0.0, buf);
    for o in [0x38, 0x40, 0x44] {
        let w = m.r32(s + o);
        show_widget(m, e, w, 1);
    }
    m.w32(s + 0x10, 3);
    let scene = m.r32(fe(m) + 0xa4);
    vcall_scene(m, e, scene, 0xd0);
}

/// `cFE3DState::Finish` 0x274b48: fading in, the screen is popped off the stack (0x2ac030, 0x275c10); else its next
/// screen (virtual +0x58, `CreateNextState`) kept at +0x14 and started (its virtual +0x60); phase 5
pub fn finish(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if m.r32(s + 0x10) == 1 {
        let stack = fe(m) + 0xa8;
        e.call(m, STACK_TOP, &[stack]);
        e.call(m, STACK_POP, &[stack]);
    } else {
        let next = vcall(m, e, s, 0x58, &[]);
        m.w32(s + 0x14, next);
        if next != 0 {
            vcall(m, e, next, 0x60, &[]);
        }
    }
    m.w32(s + 0x10, 5);
}

/// 0x274be0 (vf8): not active or done: hidden, the screen's virtual +0x38 (`TransitionIn`) and the widget group's
/// +0x38; fading out, +0x40 (`TransitionOut`) and the group's +0x38; otherwise just +0x40
pub fn vf8(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let ph = m.r32(s + 0x10);
    if ph == 2 || ph == 5 {
        return;
    }
    if ph == 0 {
        vcall(m, e, s, 0x38, &[]);
    } else if ph == 3 {
        vcall(m, e, s, 0x40, &[]);
    } else {
        vcall(m, e, s, 0x40, &[]);
        return;
    }
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x38, &[]);
}

/// `cFE3DState::UpdateTransition` 0x274c78: once the title, help bar and backdrop stop animating (their +0x10): from
/// hidden, `TransitionIn` (+0x38) and `Activate`... (+0xa0); fading in, +0x40; fading out, +0xa8 (`Deactivate`)
/// and phase 4; waiting, with the overlay's virtual +0xe8 answering, +0x40
pub fn update_transition(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let ph = m.r32(s + 0x10);
    if ph == 2 || ph == 5 {
        return;
    }
    let mut busy = false;
    let t = m.r32(s + 0x38);
    if t != 0 {
        busy = m.r32(t + 0x10) != 0;
    }
    let h = m.r32(s + 0x40);
    if h != 0 && m.r32(h + 0x10) != 0 {
        busy = true;
    }
    let b = m.r32(s + 0x44);
    if b != 0 && m.r32(b + 0x10) != 0 {
        busy = true;
    }
    match ph {
        1 => {
            if !busy {
                vcall(m, e, s, 0x40, &[]);
            }
        }
        3 => {
            if !busy {
                vcall(m, e, s, 0xa8, &[]);
                m.w32(s + 0x10, 4);
            }
        }
        4 => {
            let o = m.r32(fe(m) + 0x50);
            if o != 0 && vcall(m, e, o, 0xe8, &[]) != 0 {
                vcall(m, e, s, 0x40, &[]);
            }
        }
        0 => {
            if !busy {
                vcall(m, e, s, 0x38, &[]);
                vcall(m, e, s, 0xa0, &[]);
            }
        }
        _ => {}
    }
}

/// `cFE3DState::Activate` 0x274df0: +0x24 cleared, the widget group's virtuals +0x20 and +0x28 (1), the screen's
/// +0x20, and each player's input mask for the screen (0x271868) at its +0x3c
pub fn activate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    m.w32(s + 0x24, 0);
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x20, &[1]);
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x28, &[1]);
    // (the screen's +0x20 takes nothing; a1 is still the 1 of the group's call)
    vcall(m, e, s, 0x20, &[1]);
    for k in 0..2u32 {
        let f = fe(m);
        let mask = e.call(m, INPUT_MASK, &[s, k]);
        let player = m.r32(f + 0x74 + 4 * k);
        m.w32(player + 0x3c, mask);
    }
}

/// `cFE3DState::Deactivate` 0x274ec0: the widget group's virtuals +0x20 and +0x28 (0)
pub fn deactivate(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x20, &[0]);
    let g = m.r32(s + 0x34);
    vcall(m, e, g, 0x28, &[0]);
}

/// 0x275888 (vf21): with accept allowed (+0x1c), the screen's virtual +0x28
pub fn vf21(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    if m.r32(s + 0x1c) != 0 {
        vcall(m, e, s, 0x28, &[]);
    }
}

/// 0x2758c0 (vf22, the screen that goes and the one that comes): a screen with +0xa8 set and +0x20 5 going while
/// shown (+0x10), its virtuals +0x40 and +0x20(0); one coming while hidden, +0x20(1) and +0x40
pub fn vf22(m: &mut Mem, e: &mut dyn FeEnv, from: u32, to: u32) {
    if from != 0 && m.r32(from + 0xa8) != 0 && m.r32(from + 0x10) != 0 && m.r32(from + 0x20) == 5 {
        vcall(m, e, from, 0x40, &[]);
        vcall(m, e, from, 0x20, &[0]);
    }
    if to != 0 && m.r32(to + 0xa8) != 0 && m.r32(to + 0x10) == 0 && m.r32(to + 0x20) == 5 {
        vcall(m, e, to, 0x20, &[1]);
        vcall(m, e, to, 0x40, &[]);
    }
}

/// 0x2759a0 (vf26): the screen's virtual +0x30
pub fn vf26(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    vcall(m, e, s, 0x30, &[]);
}

/// 0x2764a8 (vf6): active (phase 2)
pub fn vf6(m: &mut Mem, s: u32) {
    m.w32(s + 0x10, 2);
}

/// 0x2717e8 (vf12): the widgets reset (0x275468(screen, 0)), the widget group's virtual +8 (3), and the screen's
/// widget pointers (+0x34..+0x44) cleared
pub fn vf12(m: &mut Mem, e: &mut dyn FeEnv, s: u32) {
    e.call(m, WIDGETS_RESET, &[s, 0]);
    let g = m.r32(s + 0x34);
    if g != 0 {
        vcall(m, e, g, 8, &[3]);
    }
    for o in [0x44, 0x34, 0x38, 0x3c, 0x40] {
        m.w32(s + o, 0);
    }
}

/// `cFE3DState::Update` 0x275148 (`buf` a scratch for the fade's colour): the screen's virtual +0x50 first. With the
/// overlay up and not closing (its +0xe8 answering 0), each player's pause button closes it (0x26c430), the 3D
/// scene's +0x90 and a half-second fade to black. Active (phase 2) with no overlay or one closing: per player (two)
/// the back button with back allowed (+0x18) makes and pushes the "back" screen (allocated 0xcc bytes, filled,
/// 0x29e920, its +0x60) and returns; with a menu (+0x30) and accept allowed (+0x1c) accept goes to the menu's +0xe8;
/// any input with a mask (+0x3c) to the menu's +0xe8 too unless something took it, noting the player at the front
/// end's +5. Done (phase 5) with a next screen: it is pushed and +0x14 cleared
pub fn update(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let f = fe(m);
    vcall(m, e, s, 0x50, &[]);
    let o = m.r32(f + 0x50);
    let mut ph;
    if o == 0 {
        ph = m.r32(s + 0x10);
    } else if vcall(m, e, o, 0xe8, &[]) == 0 {
        if m.r32(s + 0x28) != 0 {
            for k in 0..2u32 {
                let p = m.r32(f + 0x74 + 4 * k);
                if e.call(m, PAD_PAUSE, &[p]) != 0 {
                    let o = m.r32(f + 0x50);
                    e.call(m, OVERLAY_CLOSE, &[o]);
                    let scene = m.r32(f + 0xa4);
                    let vt = m.r32(scene);
                    let adj = m.r16(vt + 0x90) as i16 as u32;
                    let fun = m.r32(vt + 0x94);
                    e.call(m, fun, &[scene.wrapping_add(adj)]);
                    for c in 0..3 {
                        m.w32(buf + 4 * c, 0);
                    }
                    let word = m.r32(m.r32(GAPP).wrapping_add(0x72c));
                    e.call(m, TRANSITION_START, &[0, 0, 0.5f32.to_bits(), word, buf, 3]);
                }
            }
        }
        ph = m.r32(s + 0x10);
    } else {
        ph = m.r32(s + 0x10);
    }
    if ph == 2 {
        let o = m.r32(f + 0x50);
        if o == 0 || vcall(m, e, o, 0xe8, &[]) != 0 {
            let mut taken = 0u32;
            let mut k = 0u32;
            loop {
                let p = m.r32(f + 0x74 + 4 * k);
                let menu;
                if m.r32(s + 0x18) == 0 {
                    menu = m.r32(s + 0x30);
                } else {
                    if e.call(m, PAD_BACK, &[p]) != 0 {
                        m.w32(s + 0x24, 0);
                        vcall(m, e, s, 0x30, &[]);
                        let g = m.r32(s + 0x34);
                        vcall(m, e, g, 0x38, &[]);
                        let at = e.call(m, ALLOC, &[0x3ae5e0, 0xcc, 0x10]);
                        if at != 0 {
                            e.call(m, FILL, &[at, 0xdead_c0de, 0xcc]);
                        }
                        let back = e.call(m, BACK_SCREEN, &[at]);
                        vcall(m, e, back, 0x60, &[]);
                        e.call(m, STACK_PUSH, &[f + 0xa8, back]);
                        return;
                    }
                    menu = m.r32(s + 0x30);
                }
                if menu != 0 && m.r32(s + 0x1c) != 0 && e.call(m, PAD_ACCEPT, &[p]) != 0 {
                    let menu = m.r32(s + 0x30);
                    taken = vcall(m, e, menu, 0xe8, &[p]);
                }
                let p = m.r32(f + 0x74 + 4 * k);
                let mut any = false;
                if m.r32(p + 0x3c) != 0 {
                    any = e.call(m, PAD_ANY, &[p]) != 0;
                }
                k += 1;
                if any && m.r32(p) != 0 && taken == 0 && m.r32(s + 0x30) != 0 {
                    m.w8(f + 5, k as u8);
                    let menu = m.r32(s + 0x30);
                    taken = vcall(m, e, menu, 0xe8, &[p]);
                }
                if 1 < k {
                    break;
                }
            }
            return;
        }
        ph = m.r32(s + 0x10);
    }
    if ph == 5 && m.r32(s + 0x14) != 0 {
        let next = m.r32(s + 0x14);
        e.call(m, STACK_PUSH, &[f + 0xa8, next]);
        m.w32(s + 0x14, 0);
    }
}

/// `cFE3DState::LayoutWidgets` 0x274f18 (`buf` 0x30 bytes of scratch for its three vectors): the widget group's
/// list (from its +0xa0: 0x2ac018, each node's +8 the next) walked to its end (0x2abf10), each widget whose virtual
/// +0x80 answers getting its +0x140; then the title text (+0x3c) moved by ((640 − its width +0x170) / 2 − its x +0x80,
/// 0, 0) (virtual +0x70), and its two halves (+0x148, +0x14c) the other way when it has them (+0x144); then the help
/// bar (+0x40): its first two widgets' sizes (virtual +0x130 of the second, then the first), the second placed at
/// (the first's width + 20, the second's height) (+0x138), the third at (the first's width + 20, its own +0x84, 0, 1)
/// (0x2acb20)
pub fn layout_widgets(m: &mut Mem, e: &mut dyn FeEnv, s: u32, buf: u32) {
    let (b60, b70, b80) = (buf, buf + 0x10, buf + 0x20);
    let g = m.r32(s + 0x34);
    let mut node = e.call(m, LIST_FIRST, &[g + 0xa0]);
    while node != 0 && e.call(m, LIST_END, &[node]) == 0 {
        if vcall(m, e, node, 0x80, &[]) != 0 {
            vcall(m, e, node, 0x140, &[]);
        }
        node = m.r32(node + 8);
    }
    let t = m.r32(s + 0x3c);
    if t != 0 {
        for k in 0..4 {
            let w = m.r32(t + 0x170 + 4 * k);
            m.w32(b80 + 4 * k, w);
            let x = m.r32(t + 0x80 + 4 * k);
            m.w32(b70 + 4 * k, x);
        }
        let width = f32::from_bits(m.r32(b80));
        let x = f32::from_bits(m.r32(b70));
        let dx = (640.0 - width) * 0.5 - x;
        m.w32(b60, dx.to_bits());
        m.w32(b60 + 4, 0);
        m.w32(b60 + 8, 0);
        vcall(m, e, t, 0x70, &[b60]);
        if m.r32(t + 0x144) != 0 {
            let t1 = m.r32(t + 0x148);
            m.w32(b60, (-dx).to_bits());
            vcall(m, e, t1, 0x70, &[b60]);
            let t2 = m.r32(m.r32(s + 0x3c) + 0x14c);
            vcall(m, e, t2, 0x70, &[b60]);
        }
    }
    let h = m.r32(s + 0x40);
    if h != 0 {
        let w0 = e.call(m, LIST_NTH, &[h + 0x110, 0]);
        let h = m.r32(s + 0x40);
        let w1 = e.call(m, LIST_NTH, &[h + 0x110, 1]);
        let h = m.r32(s + 0x40);
        let w2 = e.call(m, LIST_NTH, &[h + 0x110, 2]);
        vcall(m, e, w1, 0x130, &[b70]);
        vcall(m, e, w0, 0x130, &[b80]);
        let x = f32::from_bits(m.r32(b80)) + 20.0;
        m.w32(b60, x.to_bits());
        m.w32(b60 + 4, m.r32(b70 + 4));
        m.w32(b60 + 8, 0);
        m.w32(b60 + 0xc, 0);
        vcall(m, e, w1, 0x138, &[b60]);
        m.w32(b60 + 4, m.r32(w2 + 0x84));
        m.w32(b60, x.to_bits());
        m.w32(b60 + 0xc, 1.0f32.to_bits());
        m.w32(b60 + 8, 0);
        e.call(m, WIDGET_PLACE, &[w2, b60]);
    }
}
