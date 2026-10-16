//! `tricky_game::fe_trickbook` (board row G9p) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random trick book screens (its state, chapter, film, the items picked and the answers of the career,
//! texture and film calls), every `cFE3DStateTrickBook` method run on both sides with the same answers from the calls
//! out, the calls logged in order (vectors by value; the stack's scratch, ours `BUF`, logged as one marker) and the
//! memory compared. The builder's icons, group, list and page count get the game's own vtables, so the functions
//! behind the slots it calls are stubbed by their addresses read from those vtables.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::fe3dstate::{ALLOC, FILL, GAPP, INPUT_MASK, LIST_NTH, WIDGETS_RESET, WIDGET_PLACE};
use tricky_game::fe_characcept::PLAYER_SLOT;
use tricky_game::fe_charselect::{PRINT, SEEN, SET_STRING, SLOT_PROFILE, SPRITE_RECT};
use tricky_game::fe_modeselect::BUILD_TITLED;
use tricky_game::fe_numplayers::SET_TEXT;
use tricky_game::fe_riderprofile::TEXTURES_IN;
use tricky_game::fe_title::{ADD_WIDGET, BACKDROP, HELP_BUTTON, HELP_LAYOUT, TEXT_CTOR};
use tricky_game::fe_trickbook::*;
use tricky_game::fe_username::{ICON_COLOUR, ICON_FRAMES};
use tricky_game::fe_venuesel::{LOC_STRING, WSTR_FORMAT};
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

const ARENA: u32 = 0x0100_0000;
const APP: u32 = ARENA;
const FE: u32 = ARENA + 0x1000;
const SCREEN: u32 = ARENA + 0x2000;
const MENU: u32 = ARENA + 0x2400;
const ITEM: u32 = ARENA + 0x2600;
const GROUP: u32 = ARENA + 0x2800;
const PROMPT: u32 = ARENA + 0x2c00;
const SCENE: u32 = ARENA + 0x3000;
const P0: u32 = ARENA + 0x3400;
const P1: u32 = ARENA + 0x3800;
/// the trick list (a widget with the test's vtable)
const LIST: u32 = ARENA + 0x4400;
const RECT: u32 = ARENA + 0x4800;
const MADE: [u32; 6] = [ARENA + 0x5000, ARENA + 0x5200, ARENA + 0x5400, ARENA + 0x5600, ARENA + 0x5800, ARENA + 0x5a00];
const PREV: u32 = MADE[3];
const VT: u32 = ARENA + 0x6000;
const BUF: u32 = ARENA + 0x7000;
/// the heap the builder's objects come from, 0x400 apart
const HEAP: u32 = ARENA + 0x8000;
const V_BASE: u32 = 0x7f_0000;
/// a pointer into the stack's (or our) scratch, as logged
const SCRATCH: u32 = 0x5c5c_5c5c;

/// The game's functions behind the slots the builder calls on its in-place widgets: (address, arguments, words by
/// value); read from the ELF's vtables at start
type Real = Rc<Vec<(u32, usize, usize)>>;
/// (99 words by value: a getter, its vector filled from the case)
const REAL_SLOTS: [(u32, u32, usize, usize); 26] = [
    (ICON_VTABLE, 0x50, 2, 4),
    (ICON_VTABLE, 0x138, 2, 4),
    (ICON_VTABLE, 0x100, 2, 0),
    (ICON_VTABLE, 0x20, 2, 0),
    (ICON_VTABLE, 0x70, 2, 3),
    (ICON_VTABLE, 0x130, 2, 99),
    (ICON_VTABLE, 0x108, 2, 4),
    (ICON_VTABLE, 0x110, 2, 4),
    (ICON_VTABLE, 0x118, 2, 4),
    (ICON_VTABLE, 0x120, 2, 4),
    (TILED_VTABLE, 0x50, 2, 4),
    (TILED_VTABLE, 0x138, 2, 4),
    (GROUP_VTABLE, 0x20, 2, 0),
    (GROUP_VTABLE, 0x140, 1, 0),
    (GROUP_VTABLE, 0x138, 2, 4),
    (LIST_VTABLE, 0x168, 1, 0),
    (LIST_VTABLE, 0xf0, 2, 0),
    (LIST_VTABLE, 0x148, 2, 0),
    (LIST_VTABLE, 0x138, 2, 4),
    (LIST_VTABLE, 0x70, 2, 3),
    (BUFFERED_TEXT_VTABLE, 0x158, 2, 0),
    (BUFFERED_TEXT_VTABLE, 0x110, 2, 4),
    (BUFFERED_TEXT_VTABLE, 0x50, 2, 4),
    (BUFFERED_TEXT_VTABLE, 0x130, 2, 99),
    (BUFFERED_TEXT_VTABLE, 0x138, 2, 4),
    (BUFFERED_TEXT_VTABLE, 0x70, 2, 3),
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
        let mut g = Rng(self.seed ^ (target as u64) << 20 ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for _ in 0..3 {
            g.next();
        }
        match target {
            TEXT_CTOR | TEXT_BOX | LIST_NTH => MADE[n as usize % MADE.len()],
            ALLOC => {
                if g.below(16) == 0 {
                    0
                } else {
                    HEAP + 0x400 * (n % 64)
                }
            }
            PLAYER_SLOT => MADE[2] + g.below(4),
            SPRITE_RECT => RECT,
            HELP_BUTTON => MADE[1],
            SLOT_PROFILE => g.below(12),
            PAGES_COMPLETE => (g.below(8) as i32 - 1) as u32,
            FILM_OPEN => [0u32, 3, u32::MAX][g.below(3) as usize],
            CAREER_BOOK | TRICK_ID | LOC_STRING | FILM_TEXTURE => g.next(),
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
        FILM_OPEN | LIST_CTOR => 6,
        ICON_FRAMES => 5,
        TEXT_CTOR | BUILD_TITLED | SEEN | WSTR_FORMAT | TEXT_BOX | PRINT => 4,
        ALLOC | FILL | ICON_COLOUR | CHAPTER_TEXTURES | BOOK_TEXTURES | SPRITE_TEXTURE | ARCHIVE_OPEN => 3,
        INPUT_MASK | WIDGET_PLACE | LIST_NTH | SET_TEXT | GROUP_ADD | SPRITE_RECT | PLAYER_SLOT | SLOT_PROFILE
        | SET_STRING | WIDGETS_RESET | ADD_WIDGET | HELP_LAYOUT | HELP_BUTTON | CAREER_BOOK | TRICK_DONE
        | PAGE_DONE_COUNT | TRICK_ID | TRICK_NAME | RANK_COLOUR | CLEAR_SLOT | FILM_FINISHED | FILM_RELEASE
        | FILM_TEXTURE | ARCHIVE_DELETE | BASE_DTOR | SELECT_CHAPTER | PAGE_COUNT | LOC_STRING | TEXT_BOX_SCALE => 2,
        TRICKS_LOAD | TRICKS_FREE => 0,
        t if [8u32, 0x10, 0x20, 0x28, 0x50, 0x58, 0x70, 0x100, 0x110, 0x130, 0x138, 0x148, 0x150, 0x158, 0x160, 0xf0]
            .iter()
            .any(|o| t == V_BASE + o + 4) =>
        {
            2
        }
        _ => 1,
    };
    let by = if target == V_BASE + 0x70 + 4 {
        3
    } else if [0x50u32, 0x110, 0x138].iter().any(|o| target == V_BASE + o + 4) || target == WIDGET_PLACE {
        4
    } else if target == SPRITE_TEXTURE {
        7
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
    // the archive's name takes one word
    if target == PRINT && v[1] == ARCHIVE_FORMAT {
        v.truncate(3);
    }
}

fn log_call(a: &A, real: &Real, read: &mut dyn FnMut(u32) -> u32, write: &mut dyn FnMut(u32, u32), target: u32, mut v: Vec<u32>) -> u32 {
    trim(target, &mut v);
    let (_, by) = arity(real, target);
    if by == 7 {
        let at = v[2];
        v.truncate(2);
        v.extend((0..7).map(|k| read(at + 4 * k)));
    } else if by != 0 && by != 99 && v.len() > 1 {
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
    if (target == V_BASE + 0x130 + 4 || target == RANK_COLOUR || real.iter().any(|r| r.0 == target && r.2 == 99)) && v.len() > 1 {
        let at = v[1];
        for (k, x) in getter_words(a).iter().enumerate() {
            write(at + 4 * k as u32, *x);
        }
        v.truncate(1);
    }
    // the list's constructor makes its arrows
    if target == LIST_CTOR {
        write(v[0].wrapping_add(0x148), MADE[4]);
        write(v[0].wrapping_add(0x14c), MADE[5]);
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

const REGIONS: [(u32, u32); 10] = [
    (ARENA, 0x8000),
    (HEAP, 0x10000),
    (0x337c40, 0x40),
    (0x331ea8, 0xde0),
    (0x3352c0, 0x40),
    (0x3a9e00, 0xe400),
    (0x3ae520, 0x40),
    (0x348000, 0x40),
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

#[test]
fn trick_book_screen_matches_the_original() {
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
        ALLOC, FILL, TEXT_CTOR, BACKDROP, WIDGET_PLACE, INPUT_MASK, LIST_NTH, BUILD_TITLED, SET_TEXT, PLAYER_SLOT,
        UPDATE_COLOR, LIST_INIT, GROUP_ADD, SLOT_PROFILE, SEEN, SPRITE_RECT, PRINT, SET_STRING, WIDGETS_RESET,
        ADD_WIDGET, HELP_LAYOUT, HELP_BUTTON, ICON_COLOUR, ICON_FRAMES, LOC_STRING, WSTR_FORMAT, TEXTURES_IN,
        CAREER_BOOK, TRICK_DONE, PAGES_COMPLETE, PAGE_DONE_COUNT, TRICK_ID, TRICK_NAME, RANK_COLOUR, CLEAR_SLOT,
        TRICKS_LOAD, TRICKS_FREE, CHAPTER_TEXTURES, BOOK_TEXTURES, SPRITE_TEXTURE, FILM_OPEN, FILM_FINISHED,
        FILM_RELEASE, FILMS_RELEASE, FILM_TEXTURE, ARCHIVE_OPEN, ARCHIVE_CLOSE, ARCHIVE_DELETE, BASE_DTOR,
        SELECT_CHAPTER, PAGE_COUNT, TEXT_BOX, TEXT_BOX_SCALE, LIST_CTOR,
    ];
    leaves.extend(real.iter().map(|r| r.0));
    for off in (0..0x170).step_by(8) {
        leaves.push(V_BASE + off + 4);
    }
    leaves.sort_unstable();
    leaves.dedup();
    // the base's methods the screen calls run as the original
    for f in [0x274df0u32, 0x274ec0, 0x275148, 0x275888, 0x2759a0] {
        r.allow(f);
    }
    for f in leaves {
        let a2 = a.clone();
        let real2 = real.clone();
        r.stub(f, move |c, mem| {
            let reg = |k: usize| if k < 4 { 4 + k } else { 8 + k - 4 };
            let (n, _) = arity(&real2, f);
            let mut v: Vec<u32> = (0..n).map(|k| c.gpr(reg(k)) as u32).collect();
            if f == BUILD_TITLED {
                v = vec![c.f(12).to_bits(), c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32];
            }
            if f == TEXT_BOX_SCALE {
                v = vec![c.gpr(4) as u32, c.f(12).to_bits()];
            }
            let cell = RefCell::new(mem);
            let ans = log_call(&a2, &real2, &mut |at| cell.borrow_mut().read_u32(at), &mut |at, x| cell.borrow_mut().write_u32(at, x), f, v);
            c.set_gpr(2, ans as i32 as i64 as u64);
        });
    }
    let mut g = Rng(0x27a5_0000_0000_0001);
    let methods: &[u32] =
        &[0x27a288, 0x27a4a8, 0x27a5b8, 0x27dc58, 0x27e008, 0x27e2a8, 0x27e338, 0x27e3c0, 0x27e5d0, 0x27e7f8, 0x28e360];
    let mut counts = vec![0u32; methods.len()];
    let cases = 8_000u32;
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
        r.mem.write_u32(APP + 0x50, g.next());
        r.mem.write_u32(APP + 0x728, g.next());
        r.mem.write_u32(APP + 0x72c, g.next());
        r.mem.write_u32(FE + 0x74, P0);
        r.mem.write_u32(FE + 0x78, P1);
        r.mem.write_u32(FE + 0xa4, SCENE);
        r.mem.write_u32(FE + 0x124, g.next());
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
        let mut objects = vec![SCREEN, MENU, GROUP, PROMPT, P0, P1, ITEM, LIST];
        objects.extend(MADE);
        for o in objects {
            r.mem.write_u32(o + 0xc, VT);
        }
        r.mem.write_u32(SCENE, VT);
        for (k, x) in MADE.iter().enumerate() {
            r.mem.write_u32(x + 0xf0, MADE[(k + 2) % MADE.len()]);
            r.mem.write_u32(x + 0x14, g.below(2));
            r.mem.write_u32(x + 0x110, [(-5i32) as u32, 0x86, 7][g.below(3) as usize]);
            for o in [0x60u32, 0x64, 0x68, 0x6c, 0x80, 0x84, 0x88, 0x104] {
                r.mem.write_f32(x + o, g.below(400) as f32 - 50.0);
            }
        }
        r.mem.write_f32(RECT + 4, g.below(40) as f32);
        r.mem.write_f32(RECT + 8, g.below(40) as f32);
        for k in 3..7 {
            r.mem.write_u32(RECT + 4 * k, g.next());
        }
        for k in 0..4 {
            r.mem.write_u8(MADE[2] + k, g.below(4) as u8);
        }
        let s = SCREEN;
        r.mem.write_u32(s + 0x10, 0);
        r.mem.write_u32(s + 0x30, g.below(2));
        r.mem.write_u32(s + 0x34, GROUP);
        r.mem.write_u32(s + 0x3c, MENU);
        r.mem.write_u32(s + 0x40, [0, PROMPT][g.below(2) as usize]);
        r.mem.write_u32(s + 0x44, MADE[0]);
        r.mem.write_u32(s + 0x4c, PROMPT);
        r.mem.write_u8(s + 0x2e, [0u8, 1, 0xff][g.below(3) as usize]);
        r.mem.write_u32(s + 0x54, [0u32, 0, 1, 2][g.below(4) as usize]);
        r.mem.write_u32(s + 0x58, [5u32, 3, 0][g.below(3) as usize]);
        r.mem.write_u32(s + 0x5c, g.below(0x300));
        r.mem.write_u32(s + 0x60, [u32::MAX, 0, 1, 3, 5, 6][g.below(6) as usize]);
        r.mem.write_u32(s + 0x64, [u32::MAX, 0, 2][g.below(3) as usize]);
        r.mem.write_u32(s + 0xfc, MADE[1]);
        r.mem.write_u32(s + 0x100, MADE[2]);
        r.mem.write_u32(s + 0x104, LIST);
        r.mem.write_u32(s + 0x108, MADE[0]);
        r.mem.write_u32(s + 0x10c, MADE[4]);
        r.mem.write_u32(s + 0x110, MADE[5]);
        r.mem.write_u32(LIST + 0x140, MADE[3]);
        r.mem.write_u32(LIST + 0x148, MADE[4]);
        r.mem.write_u32(LIST + 0x14c, MADE[5]);
        r.mem.write_f32(LIST + 0x15c, g.below(30) as f32);
        r.mem.write_f32(LIST + 0x84, g.below(300) as f32);
        r.mem.write_u32(MENU + 0x140, ITEM);
        r.mem.write_f32(MENU + 0x170, g.below(600) as f32);
        r.mem.write_f32(MENU + 0x80, g.below(300) as f32);
        r.mem.write_u32(MENU + 0x144, g.below(2));
        r.mem.write_u32(MENU + 0x148, MADE[1]);
        r.mem.write_u32(MENU + 0x14c, MADE[2]);
        for x in [PREV, ITEM] {
            r.mem.write_u32(x, [g.below(13), g.below(13), 0x105][g.below(3) as usize]);
            r.mem.write_u32(x + 0xa8, g.below(2));
            r.mem.write_u32(x + 0x20, [5u32, 0][g.below(2) as usize]);
            r.mem.write_u32(x + 0x10, g.below(2));
            r.mem.write_u32(x + 0x110, g.below(0x100));
        }
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
            0x27a288 | 0x27a4a8 => [0u32, 1, 4, 5, 6, 7][g.below(6) as usize],
            0x27e338 | 0x27e3c0 => ITEM,
            0x28e360 => g.below(2),
            _ => PREV,
        };
        r.cpu.set_gpr(4, s as u64);
        r.cpu.set_gpr(5, a1 as u64);
        r.cpu.set_gpr(6, ITEM as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {addr:#x}: {e}"));
        let theirs = std::mem::take(&mut a.borrow_mut().log);
        let after = snap(&mut r);
        a.borrow_mut().count.clear();
        let e = &mut Ours(a.clone(), real.clone());
        match addr {
            0x27a288 => select_chapter(&mut m, e, s, a1, BUF),
            0x27a4a8 => page_count(&mut m, e, s, a1, BUF),
            0x27a5b8 => build(&mut m, e, s, BUF),
            0x27dc58 => update(&mut m, e, s, BUF),
            0x27e008 => activate(&mut m, e, s, BUF),
            0x27e2a8 => deactivate(&mut m, e, s),
            0x27e338 => vf21(&mut m, e, s, a1),
            0x27e3c0 => vf26(&mut m, e, s, a1, BUF),
            0x27e5d0 => vf22(&mut m, e, s, a1, ITEM, BUF),
            0x27e7f8 => vf24(&mut m, e, s),
            _ => destroy(&mut m, e, s, a1),
        }
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
        for (k, (at, b)) in after.regions.iter().enumerate() {
            if let Some(i) = (0..b.len()).find(|&i| b[i] != m.regions[k].1[i] && !(BUF..BUF + 0x300).contains(&(at + i as u32))) {
                panic!("case {case}: {addr:#x}: memory at {:#x} (theirs {:#x}, mine {:#x})", at + i as u32, b[i], m.regions[k].1[i]);
            }
        }
    }
    println!("cases by method: {counts:?}");
}
