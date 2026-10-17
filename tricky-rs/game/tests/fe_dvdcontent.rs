//! `tricky_game::fe_dvdcontent` (board row G9t) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random DVD extras screens (the active list, the swap's state and its step widget, the pictures' fades,
//! the items and songs picked, and the answers of the list, texture and scene calls), every `cFE3DStateDVDContent`
//! method run on both sides with the same answers from the calls out, the calls logged in order (vectors by value; the
//! stack's scratch, ours `BUF`, logged as one marker) and the memory compared. The screen's own helpers (the song
//! shown, the captions) are stubbed in its methods and run on their own in a second runner. The builder's icons,
//! entries and group get the game's own vtables, so the functions behind the slots it calls are stubbed by their
//! addresses read from those vtables.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{ALLOC, FILL, GAPP, INPUT_MASK, LIST_END, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use tricky_game::fe_charselect::{PRINT, SPRITE_RECT};
use tricky_game::fe_dvdcontent::*;
use tricky_game::fe_options::LIST_AT;
use tricky_game::fe_riderprofile::{SET_FILM, TEXTURES_IN};
use tricky_game::fe_title::{ADD_WIDGET, BASE_CTOR, BUILD_BASE, HELP_BUTTON, HELP_BUTTONS, HELP_LAYOUT, TEXT_CTOR};
use tricky_game::fe_username::{ICON_COLOUR, ICON_FRAMES};
use tricky_game::fe_widgets::*;
use tricky_game::pf::Mem;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (z ^ (z >> 31)) | 1
}

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
const FE: u32 = ARENA + 0x1000;
const SCREEN: u32 = ARENA + 0x2000;
const MENU: u32 = ARENA + 0x2400;
const ITEM: u32 = ARENA + 0x2600;
const GROUP: u32 = ARENA + 0x2800;
const SCENE: u32 = ARENA + 0x3000;
const P0: u32 = ARENA + 0x3400;
const P1: u32 = ARENA + 0x3800;
/// the four sub-menus (characters, demos, jukebox, promos)
const SUBS: [u32; 4] = [ARENA + 0x3c00, ARENA + 0x3e00, ARENA + 0x4000, ARENA + 0x4200];
const BAR: u32 = ARENA + 0x4400;
const RECT: u32 = ARENA + 0x4800;
/// the help bar's buttons and their icons (apart from the made widgets, which the captions write later)
const BUTTONS: [u32; 3] = [ARENA + 0x4c00, ARENA + 0x4d00, ARENA + 0x4e00];
const BUTTON_ICONS: [u32; 3] = [ARENA + 0x6400, ARENA + 0x6500, ARENA + 0x6600];
const MADE: [u32; 6] = [ARENA + 0x5000, ARENA + 0x5200, ARENA + 0x5400, ARENA + 0x5600, ARENA + 0x5800, ARENA + 0x5a00];
const PREV: u32 = MADE[3];
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
const HEAP: u32 = ARENA + 0x8000;
const V_BASE: u32 = 0x7f_0000;
const SCRATCH: u32 = 0x5c5c_5c5c;

type Real = Rc<Vec<(u32, usize, usize)>>;
/// (99 words by value: a getter, its vector filled from the case)
const REAL_SLOTS: [(u32, u32, usize, usize); 14] = [
    (ICON_VTABLE, 0x138, 2, 4),
    (ICON_VTABLE, 0x50, 2, 4),
    (ICON_VTABLE, 0x20, 2, 0),
    (ICON_VTABLE, 0x28, 2, 0),
    (ICON_VTABLE, 0x40, 1, 0),
    (TEXT_ENTRY_VTABLE, 0x50, 2, 4),
    (TEXT_ENTRY_VTABLE, 0x130, 2, 99),
    (TEXT_ENTRY_VTABLE, 0x110, 2, 4),
    (TEXT_ENTRY_VTABLE, 0xf8, 2, 0),
    (TEXT_ENTRY_VTABLE, 0x20, 2, 0),
    (TEXT_ENTRY_VTABLE, 8, 2, 0),
    (GROUP_VTABLE, 0x20, 2, 0),
    (GROUP_VTABLE, 0x28, 2, 0),
    (GROUP_VTABLE, 0x70, 2, 3),
];

struct Answers {
    seed: u64,
    count: Vec<(u32, u32)>,
    log: Vec<(u32, Vec<u32>)>,
}
impl Answers {
    fn answer(&mut self, target: u32) -> u32 {
        let n = match self.count.iter_mut().find(|c| c.0 == target) {
            Some(c) => {
                c.1 += 1;
                c.1
            }
            None => {
                self.count.push((target, 0));
                0
            }
        };
        // (a multiplying mix: xorshift alone keeps successive answers of one call in a fixed relation)
        let mut g = Rng(mix(self.seed ^ (target as u64) << 16 ^ n as u64));
        for _ in 0..3 {
            g.next();
        }
        match target {
            TEXT_CTOR | LIST_NTH => MADE[n as usize % MADE.len()],
            LIST_AT => [0, MADE[g.below(6) as usize]][g.below(2) as usize],
            LIST_INDEX => [0u32, 1, 2, u32::MAX][g.below(4) as usize],
            // (never null: the game builds its widgets in place without checking, so a failed allocation is not a
            // case the original handles)
            ALLOC => HEAP + 0x400 * (n % 64),
            HELP_BUTTON => BUTTONS[n as usize % BUTTONS.len()],
            SPRITE_RECT => RECT,
            t if t == V_BASE + 0x70 + 4 => [0xcu32, 0, 1][g.below(3) as usize],
            _ => g.below(2),
        }
    }
}
type A = Rc<RefCell<Answers>>;

fn arity(real: &Real, target: u32) -> (usize, usize) {
    if let Some(r) = real.iter().find(|r| r.0 == target) {
        return (r.1, r.2);
    }
    let n = match target {
        ICON_FRAMES => 5,
        TEXT_CTOR => 4,
        ALLOC | FILL | ICON_COLOUR | LOAD_SET | HELP_BUTTONS | PRINT => 3,
        INPUT_MASK | WIDGET_PLACE | LIST_NTH | LIST_AT | GROUP_ADD | SPRITE_RECT | WIDGETS_RESET | ADD_WIDGET
        | HELP_LAYOUT | HELP_BUTTON | SET_FILM | BUILD_BASE | BUILD_CHARACTER_LIST | BASE_DTOR | SHOW_SONG
        | CAPTIONS_ID | CAPTIONS_STRING => 2,
        t if [8u32, 0x10, 0x20, 0x28, 0x50, 0x58, 0x70, 0x100, 0x108, 0x110, 0x118, 0x120, 0x130, 0x138, 0x148, 0x150,
            0x158, 0x160, 0xe8]
            .iter()
            .any(|o| t == V_BASE + o + 4) =>
        {
            2
        }
        _ => 1,
    };
    let by = if target == V_BASE + 0x70 + 4 {
        3
    } else if [0x50u32, 0x108, 0x110, 0x118, 0x120, 0x138].iter().any(|o| target == V_BASE + o + 4) || target == WIDGET_PLACE {
        4
    } else {
        0
    };
    (n, by)
}

fn getter_words(a: &A) -> [u32; 4] {
    let mut g = Rng(a.borrow().seed ^ 0x5151 ^ a.borrow().log.len() as u64);
    [0, 1, 2, 3].map(|_| (g.below(600) as f32 - 100.0).to_bits())
}

/// The game's stack (the runner's grows down from 0x1ff0000) or our scratch
fn is_scratch(v: u32) -> bool {
    (0x01f0_0000..0x01ff_0000).contains(&v) || (BUF..BUF + 0x300).contains(&v)
}

fn trim(target: u32, v: &mut Vec<u32>) {
    // +0x20, +0x28 and +0x50 take nothing on a screen (the base's update asks +0x50 of it)
    if [0x20u32, 0x28, 0x50].iter().any(|o| target == V_BASE + o + 4) && v[0] == SCREEN {
        v.truncate(1);
    }
    // +0x70 is a query on the screen before
    if target == V_BASE + 0x70 + 4 && v[0] == PREV {
        v.truncate(1);
    }
    // the 3D scene's +0x100 takes nothing (the game leaves the scene in a1)
    if target == V_BASE + 0x100 + 4 && v[0] == SCENE {
        v.truncate(1);
    }
    // a film's name or a print's leftover word
    if target == PRINT {
        v.truncate(3);
    }
}

fn log_call(a: &A, real: &Real, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, mut v: Vec<u32>) -> u32 {
    trim(target, &mut v);
    let (_, by) = arity(real, target);
    // (the 3D scene's arguments are values)
    if by != 0 && by != 99 && v.len() > 1 && v[0] != SCENE {
        let at = v[1];
        v.truncate(1);
        v.extend((0..by as u32).map(|k| read(at + 4 * k)));
    }
    if target == ICON_COLOUR || target == ICON_FRAMES {
        let at = v[2];
        let mut w: Vec<u32> = v[..2].to_vec();
        w.extend((0..4).map(|k| read(at + 4 * k)));
        w.extend_from_slice(&v[3..]);
        v = w;
    }
    if (target == V_BASE + 0x130 + 4 || real.iter().any(|r| r.0 == target && r.2 == 99)) && v.len() > 1 {
        let at = v[1];
        for (k, x) in getter_words(a).iter().enumerate() {
            write(at + 4 * k as u32, *x);
        }
        v.truncate(1);
    }
    for x in v.iter_mut() {
        if is_scratch(*x) {
            *x = SCRATCH;
        }
    }
    a.borrow_mut().log.push((target, v));
    a.borrow_mut().answer(target)
}

struct Ours(A, Real);
impl tricky_game::fe3dstate::FeEnv for Ours {
    fn call(&mut self, m: &mut Mem, target: u32, args: &[u32]) -> u32 {
        let (n, _) = arity(&self.1, target);
        let v: Vec<u32> = (0..n).map(|k| args.get(k).copied().unwrap_or(0)).collect();
        let cell = RefCell::new(m);
        log_call(&self.0, &self.1, &mut |at| cell.borrow().r32(at), &mut |at, x| cell.borrow_mut().w32(at, x), target, v)
    }
}

const REGIONS: [(u32, u32); 9] = [
    (ARENA, 0x8000),
    (HEAP, 0x10000),
    (0x337c40, 0x40),
    (0x331ea8, 0xde0),
    (0x3a9e00, 0xe400),
    (0x3ae500, 0x60),
    (0x348000, 0x200),
    (0x32de80, 0x10),
    (0, 0x600),
];

fn snap(r: &mut Runner) -> Mem {
    let mut m = Mem::default();
    for (at, n) in REGIONS {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn run(direct: bool) {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    let real: Real = Rc::new(
        REAL_SLOTS
            .iter()
            .map(|&(vt, off, n, by)| {
                let f = r.mem.read_u32(vt + off + 4);
                (f, n, by)
            })
            .collect(),
    );
    let a: A = Rc::new(RefCell::new(Answers { seed: 0, count: vec![], log: vec![] }));
    let mut leaves: Vec<u32> = vec![
        ALLOC, FILL, TEXT_CTOR, WIDGET_PLACE, INPUT_MASK, LIST_NTH, LIST_END, LIST_AT, LIST_INDEX, UPDATE_COLOR,
        LIST_INIT, GROUP_ADD, SPRITE_RECT, PRINT, WIDGETS_RESET, ADD_WIDGET, HELP_LAYOUT, HELP_BUTTON, HELP_BUTTONS,
        ICON_COLOUR, ICON_FRAMES, TEXTURES_IN, LOAD_SET, SET_FILM, BASE_CTOR, BUILD_BASE, BUILD_CHARACTER_LIST,
        BUILD_JUKEBOX, BUILD_CHARACTER_MENU, BUILD_DEMO_MENU, BASE_DTOR, CUE_START, CUE_CHARACTERS, CUE_JUKEBOX,
        CUE_PROMOS,
    ];
    if !direct {
        leaves.extend([SHOW_SONG, CAPTIONS_ID, CAPTIONS_STRING, CAPTIONS_SHOW]);
    }
    leaves.extend(real.iter().map(|r| r.0));
    for off in (0..0x170).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    leaves.sort_unstable();
    leaves.dedup();
    // the base's methods the screen calls run as the original
    for f in [0x274df0u32, 0x274ec0, 0x275148, 0x275888, 0x2758c0, 0x2759a0] {
        r.allow(f);
    }
    for f in leaves {
        let a2 = a.clone();
        let real2 = real.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let (n, _) = arity(&real2, f);
            let v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &real2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(if direct { 0x2939_0000_0000_0002 } else { 0x2939_0000_0000_0001 });
    let methods: &[u32] = if direct {
        &[0x299338, 0x2994c8, 0x2995a8, 0x299688]
    } else {
        &[0x293c20, 0x293e10, 0x2986c0, 0x299270, 0x2992f0, 0x299700, 0x299e10, 0x299ff0, 0x29dbb0, 0x29dc30]
    };
    let mut counts = vec![0u32; methods.len()];
    let cases = if direct { 4_000u32 } else { 16_000 };
    for case in 0..cases {
        for k in 0..0x8000 / 4 {
            r.mem.write_u32(ARENA + 4 * k, if g.below(3) == 0 { 0 } else { g.below(4) });
        }
        for k in 0..0x10000 / 4 {
            r.mem.write_u32(HEAP + 4 * k, 0x5a5a_5a5a ^ k);
        }
        for k in 0..0x600 / 4 {
            r.mem.write_u32(4 * k, 0);
        }
        r.mem.write_u32(GAPP, APP);
        r.mem.write_u32(APP + 0x734, FE);
        r.mem.write_u32(APP + 0x5c, g.next());
        r.mem.write_u32(APP + 0x60, g.next());
        r.mem.write_u32(APP + 0x728, g.next());
        r.mem.write_u32(FE + 0x74, P0);
        r.mem.write_u32(FE + 0x78, P1);
        r.mem.write_u32(FE + 0xa4, SCENE);
        r.mem.write_u32(FE + 0x50, 0);
        r.mem.write_u32(FE + 0xf0, [0u32, 2, 3][g.below(3) as usize]);
        r.mem.write_u8(FE + 4, [0u8, 1, 2][g.below(3) as usize]);
        for k in 0..12 {
            r.mem.write_u32(FE + 0x20 + 4 * k, MADE[k as usize % MADE.len()]);
        }
        for off in (0..0x170).step_by(8) {
            r.mem.write_u32(VT + off, 0);
            r.mem.write_u32(VT + off + 4, V_BASE + off + 4);
        }
        let mut objects = vec![SCREEN, MENU, GROUP, P0, P1, ITEM, BAR];
        objects.extend(MADE);
        objects.extend(SUBS);
        for o in objects {
            r.mem.write_u32(o + 0xc, VT);
        }
        r.mem.write_u32(SCENE, VT);
        for k in 0..3 {
            r.mem.write_u32(BUTTONS[k] + 0xc, VT);
            r.mem.write_u32(BUTTONS[k] + 0x110, BUTTON_ICONS[k]);
            r.mem.write_u32(BUTTON_ICONS[k] + 0xc, VT);
        }
        // (a word where a missing list's line would be read: the game never reads it)
        r.mem.write_u32(0x140, MADE[0]);
        for (k, x) in MADE.iter().enumerate() {
            r.mem.write_u32(*x, g.below(25));
            r.mem.write_u32(x + 8, MADE[(k + 1) % MADE.len()]);
            r.mem.write_u32(x + 0x10, g.below(2));
            r.mem.write_u32(x + 0x14, g.below(2));
            r.mem.write_u32(x + 0x24, g.below(5));
            r.mem.write_u32(x + 0x28, g.below(5));
            r.mem.write_u32(x + 0x30, g.below(2));
            r.mem.write_u32(x + 0x110, MADE[(k + 2) % MADE.len()]);
            r.mem.write_u32(x + 0x114, MADE[(k + 3) % MADE.len()]);
            r.mem.write_u32(x + 0x140, [0, MADE[(k + 4) % MADE.len()], ITEM][g.below(3) as usize]);
            for o in [0x80u32, 0x84, 0x88] {
                r.mem.write_f32(x + o, g.below(400) as f32 - 50.0);
            }
        }
        r.mem.write_f32(RECT + 4, g.below(40) as f32);
        r.mem.write_f32(RECT + 8, g.below(40) as f32);
        let s = SCREEN;
        r.mem.write_u32(s + 0x10, 0);
        r.mem.write_u32(s + 0x34, GROUP);
        r.mem.write_u32(s + 0x3c, MENU);
        r.mem.write_u32(s + 0x48, BAR);
        // (positions are floats: the VU's subtract takes a denormal as 0)
        for x in [BAR, ITEM, MENU, GROUP, SUBS[0], SUBS[1], SUBS[2], SUBS[3]] {
            for o in [0x80u32, 0x84, 0x88] {
                r.mem.write_f32(x + o, g.below(300) as f32);
            }
        }
        r.mem.write_u8(s + 0x2e, [0u8, 1, 0xff][g.below(3) as usize]);
        for k in 0..21u32 {
            r.mem.write_u32(s + 0x54 + 4 * k, MADE[k as usize % MADE.len()]);
        }
        for (k, l) in SUBS.iter().enumerate() {
            r.mem.write_u32(s + [0x68u32, 0x6c, 0x70, 0x74][k], *l);
            r.mem.write_u32(l + 0x140, [ITEM, MADE[k]][g.below(2) as usize]);
        }
        r.mem.write_u32(s + 0x30, [MENU, MENU, SUBS[0], SUBS[1], SUBS[2], SUBS[2], SUBS[3], 0, GROUP][g.below(9) as usize]);
        r.mem.write_u32(MENU + 0x140, ITEM);
        r.mem.write_u32(GROUP + 0x140, [0, MADE[2]][g.below(2) as usize]);
        r.mem.write_u32(s + 0x94, MADE[g.below(6) as usize]);
        r.mem.write_u32(s + 0xa8, [0u32, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12][g.below(15) as usize]);
        for o in [0xacu32, 0xb0, 0xb4] {
            r.mem.write_u32(s + o, g.below(25));
        }
        for x in [PREV, ITEM] {
            r.mem.write_u32(x, g.below(13));
            r.mem.write_u32(x + 0xa8, g.below(2));
            r.mem.write_u32(x + 0x20, [5u32, 0][g.below(2) as usize]);
            r.mem.write_u32(x + 0x10, g.below(2));
        }
        r.mem.write_u32(ITEM + 0x140, MADE[g.below(6) as usize]);
        let which = g.below(methods.len() as u32) as usize;
        let addr = methods[which];
        counts[which] += 1;
        {
            let mut b = a.borrow_mut();
            b.seed = g.next() as u64 | (g.next() as u64) << 32;
            b.count.clear();
            b.log.clear();
        }
        let mut m = snap(&mut r);
        let a1 = match addr {
            0x299270 | 0x299700 => [0, PREV][g.below(2) as usize],
            0x2994c8 | 0x2995a8 => g.next(),
            0x29dbb0 => g.below(2),
            0x299338 => MADE[g.below(6) as usize],
            _ => ITEM,
        };
        let a1 = if addr == 0x299270 { PREV } else { a1 };
        r.cpu.set_gpr(4, s as u64);
        r.cpu.set_gpr(5, a1 as u64);
        r.cpu.set_gpr(6, ITEM as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let ret = r.cpu.gpr(2) as u32;
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone(), real.clone());
        let mine_ret = match addr {
            0x293c20 => Some(create_next_state(&mut m, e, s, BUF)),
            0x293e10 => {
                build(&mut m, e, s, BUF);
                None
            }
            0x2986c0 => {
                update(&mut m, e, s, BUF);
                None
            }
            0x299270 => {
                activate(&mut m, e, s, a1);
                None
            }
            0x2992f0 => {
                deactivate(&mut m, e, s);
                None
            }
            0x299700 => {
                vf22(&mut m, e, s, a1, ITEM);
                None
            }
            0x299e10 => {
                vf26(&mut m, e, s, a1);
                None
            }
            0x299ff0 => {
                vf21(&mut m, e, s);
                None
            }
            0x29dbb0 => {
                destroy(&mut m, e, s, a1);
                None
            }
            0x29dc30 => Some(vf13()),
            0x299338 => {
                show_song(&mut m, e, s, a1, BUF);
                None
            }
            0x2994c8 => {
                captions_id(&mut m, e, s, a1);
                None
            }
            0x2995a8 => {
                captions_string(&mut m, e, s, a1);
                None
            }
            _ => {
                captions_show(&mut m, e, s);
                None
            }
        };
        let mine = std::mem::take(&mut a.borrow_mut().log);
        if theirs != mine {
            let k = (0..theirs.len().min(mine.len())).find(|&k| theirs[k] != mine[k]).unwrap_or(theirs.len().min(mine.len()));
            panic!(
                "case {case}: {addr:#x} calls differ at {k}: theirs {:x?}, mine {:x?} ({} and {} calls; before {:x?})",
                theirs.get(k),
                mine.get(k),
                theirs.len(),
                mine.len(),
                if k > 0 { theirs.get(k - 1) } else { None }
            );
        }
        if let Some(v) = mine_ret {
            assert_eq!(ret, v, "case {case}: {addr:#x} answer");
        }
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x300).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by method: {counts:?}");
}

#[test]
fn dvd_extras_screen_matches_the_original() {
    run(false);
}

#[test]
fn song_and_captions_match_the_original() {
    run(true);
}
