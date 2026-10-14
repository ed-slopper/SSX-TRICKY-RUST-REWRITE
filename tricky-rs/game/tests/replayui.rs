//! `tricky_game::replayui` (board row G12n) against the original in the function runner (TRICKY_ELF; skipped
//! without it): the replay's controls, the race menu's replay page and its card save, on random memory with every
//! call out logged on both sides and all memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;
use tricky_game::replayui::*;
use tricky_game::sndqueue::{Queue, QueueEnv};

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

/// The calls out, in order, and the answers both sides take from the same list
#[derive(Default)]
struct Log {
    calls: Vec<(&'static str, u32, u32)>,
    answers: Vec<u32>,
    next: usize,
}
impl Log {
    fn take(&mut self) -> u32 {
        let v = self.answers.get(self.next).copied().unwrap_or(0x1234_5678);
        self.next += 1;
        v
    }
    fn push(&mut self, name: &'static str, a: u32, b: u32) {
        self.calls.push((name, a, b));
    }
}
type L = Rc<RefCell<Log>>;

/// The answer of each kind from a raw value (both sides)
fn preset(v: u32) -> u32 {
    if v & 1 == 0 {
        8
    } else {
        v % 10
    }
}
fn new_mode(v: u32) -> u32 {
    [3, 3, 3, 0, 7, 2, 5, 12][(v % 8) as usize]
}
fn can_add(v: u32) -> u32 {
    (v % 3).wrapping_sub(1)
}
fn status_word(v: u32, i: u32) -> u32 {
    if i == 1 {
        [0, 1, 0x707, 0x708, 0x709, v][(v % 6) as usize]
    } else {
        v.wrapping_mul(i + 1) ^ 0x5a5a
    }
}

// ---- the layout ----

const ARENA: u32 = 0x4600;
struct At {
    b: u32,
}
impl At {
    fn o(&self) -> u32 {
        self.b
    }
    fn mgr(&self) -> u32 {
        self.b + 0x600
    }
    fn race(&self) -> u32 {
        self.b + 0x700
    }
    fn app(&self) -> u32 {
        self.b + 0xa00
    }
    fn game(&self) -> u32 {
        self.b + 0x1200
    }
    fn world(&self) -> u32 {
        self.b + 0x1900
    }
    fn rider(&self, i: u32) -> u32 {
        self.b + 0x1a00 + i * 0x880
    }
    fn cell(&self, i: u32) -> u32 {
        self.b + 0x3c00 + 4 * i
    }
    fn pad(&self) -> u32 {
        self.b + 0x3c20
    }
    fn x(&self) -> u32 {
        self.b + 0x3c40
    }
    fn y(&self) -> u32 {
        self.b + 0x3c80
    }
    fn hl(&self, i: u32) -> u32 {
        self.b + 0x4200 + 8 * i
    }
    fn kf(&self, i: u32) -> u32 {
        self.b + 0x4300 + 0x24 * i
    }
    fn vtable(&self) -> u32 {
        self.b + 0x4500
    }
}

const GLOBALS: [u32; 3] = [GAPP, PAD, CUTS_ON];

fn regions(a: &At) -> Vec<(u32, u32)> {
    let mut v = vec![(a.b, ARENA), (SAVE_DATA, SAVE_LEN)];
    for g in GLOBALS {
        v.push((g, 4));
    }
    v
}

fn snapshot(r: &mut Runner, a: &At) -> Mem {
    let mut m = Mem::default();
    for (at, n) in regions(a) {
        let mut b = vec![0u8; n as usize];
        r.mem.read(at, &mut b).unwrap();
        m.regions.push((at, b));
    }
    m
}

fn compare(r: &mut Runner, a: &At, m: &Mem, what: &str) {
    let theirs = snapshot(r, a);
    for ((at, x), (_, y)) in theirs.regions.iter().zip(&m.regions) {
        if let Some(i) = (0..x.len()).find(|&i| x[i] != y[i]) {
            let i = i & !3;
            panic!("{what}: memory at {:#x}: {:x?} vs {:x?}", at + i as u32, &x[i..(i + 4).min(x.len())], &y[i..(i + 4).min(y.len())]);
        }
    }
}

const BITS: [u32; 23] = [
    1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x8000, 0x20000, 0x40000, 0x80000,
    0x100000, 0x200000, 0x400000, 0x800000, 0x1000000,
];
fn buttons(g: &mut Rng, p: u32) -> u32 {
    let mut v = 0;
    for b in BITS {
        if g.below(100) < p {
            v |= b;
        }
    }
    v
}
fn fpick(g: &mut Rng, xs: &[f32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize].to_bits()
}

/// A random game around the overlay, written into the runner
fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for k in 0..ARENA / 4 {
        r.mem.write_u32(a.b + 4 * k, g.next());
    }
    for k in 0..SAVE_LEN / 4 {
        r.mem.write_u32(SAVE_DATA + 4 * k, g.next());
    }
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    w(r, GAPP, a.app());
    w(r, PAD, if g.below(30) == 0 { 0 } else { a.pad() });
    w(r, CUTS_ON, g.below(3));
    w(r, a.app() + 0x730, a.race());
    w(r, a.app() + 0x724, a.game());
    w(r, a.race() + 0x40, a.mgr());
    w(r, a.race() + 0x1c, a.world());
    w(r, a.race() + 0x38, a.x());
    w(r, a.x() + 0x24, a.y());
    let n = g.below(5);
    w(r, a.world() + 0x7c, n);
    w(r, a.world() + 0x88, g.below(4));
    for i in 0..4 {
        w(r, a.world() + 0x8c + 4 * i, a.rider(i));
        w(r, a.rider(i) + 0x870, a.cell(i));
        w(r, a.cell(i), if g.below(3) == 0 { a.pad() } else { g.next() });
    }
    // the pad
    let held = buttons(g, 12);
    w(r, a.pad() + 0x10, held);
    w(r, a.pad() + 0xc, if g.below(2) == 0 { buttons(g, 12) } else { held & buttons(g, 40) });
    // the manager
    let m = a.mgr();
    w(r, m + 0x20, [0, 2, 3, 3, 3, 3, 7, 5, 12][g.below(9) as usize]);
    w(r, m + 0x24, g.below(4));
    w(r, m + 0x2c, g.below(6));
    w(r, m + 0x30, g.below(6));
    w(r, m + 0x34, g.below(6));
    w(r, m + 0x4c, (g.below(3) == 0) as u32);
    w(r, m + 0x50, (g.below(3) == 0) as u32);
    // the highlights
    for i in 0..12 {
        w(r, a.hl(i), a.kf(i));
        w(r, a.hl(i) + 4, g.below(5));
        w(r, a.kf(i) + 0x20, g.below(7));
    }
    // the overlay
    let o = a.o();
    let ys = [0.0, 105.0, 104.0, 109.0, 360.0, 357.0, 480.0, 477.5, 300.0];
    w(r, o + 0x350, fpick(g, &ys));
    w(r, o + 0x354, fpick(g, &ys));
    w(r, o + 0x358, fpick(g, &ys));
    w(r, o + 0x3c0, g.below(11));
    for i in 0..10 {
        w(r, o + 0x3c4 + 4 * i, a.hl(g.below(12)));
    }
    w(r, o + 0x430, g.below(4));
    for i in 0..10 {
        w(r, o + 0x438 + 4 * i, a.hl(g.below(12)));
    }
    w(r, o + 0x470, g.below(62));
    w(r, o + 0x474, g.below(25).wrapping_sub(12));
    w(r, o + 0x478, fpick(g, &[0.0, 0.01, 0.016, 0.5, 1.0, -0.25]));
    w(r, o + 0x47c, g.below(3));
    w(r, o + 0x488, g.below(4));
    w(r, o + 0x48c, g.below(4));
    w(r, o + 0x490, [0, 1, 2, 14, 15, 16, 7][g.below(7) as usize]);
    w(r, o + 0x494, [0, 0, 0x1000, 0x2000, 0x400, 0x20, g.next()][g.below(7) as usize]);
    w(r, o + 0x49c, (g.below(12) == 0) as u32);
    w(r, o + 0x4a8, g.below(2));
    // the menu page
    w(r, o + 0x428, a.vtable() - 0x18);
    r.mem.write_u16(a.vtable(), (g.below(5) * 4) as u16);
    w(r, a.vtable() + 4, 0x1f0498);
    w(r, o + 0x43c, g.below(4));
    w(r, o, (g.below(2) == 0) as u32);
    w(r, o + 0x38, g.below(20));
}

// ---- the stubs ----

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

fn log(r: &mut Runner, l: &L, addr: u32, name: &'static str, args: usize) {
    let l = l.clone();
    r.stub(addr, move |c, _| {
        let a = if args > 0 { arg(c, 4) } else { 0 };
        let b = if args > 1 { arg(c, 5) } else { 0 };
        l.borrow_mut().push(name, a, b);
    });
}

fn answer(r: &mut Runner, l: &L, addr: u32, name: &'static str, args: usize, f: fn(u32) -> u32) {
    let l = l.clone();
    r.stub(addr, move |c, _| {
        let a = if args > 0 { arg(c, 4) } else { 0 };
        let b = if args > 1 { arg(c, 5) } else { 0 };
        let mut l = l.borrow_mut();
        l.push(name, a, b);
        let v = f(l.take());
        c.set_gpr(2, v as i32 as i64 as u64);
    });
}

fn stub_all(r: &mut Runner, l: &L) {
    for a in [0x1ec728, 0x1eded0, 0x195690, 0x1955b8, 0x1955e0, 0x1954b8, 0x195510] {
        r.allow(a);
    }
    log(r, l, 0x1a8648, "move", 1);
    log(r, l, 0x214428, "scroll", 0);
    log(r, l, 0x214650, "camsnd", 0);
    log(r, l, 0x228d10, "speech", 0);
    answer(r, l, 0x1786c0, "preset", 0, preset);
    log(r, l, 0x178680, "next", 1);
    let l2 = l.clone();
    r.stub(0x178be8, move |c, _| l2.borrow_mut().push("chase", arg(c, 4), c.f(12).to_bits()));
    log(r, l, 0x22d370, "emit", 0);
    log(r, l, 0x215370, "crowd", 2);
    log(r, l, 0x2153f8, "resume", 0);
    log(r, l, 0x215488, "stop", 0);
    let l2 = l.clone();
    r.stub(0x191b18, move |c, m| {
        let mut l = l2.borrow_mut();
        l.push("playlist", arg(c, 4), arg(c, 5));
        let v = new_mode(l.take());
        m.write_u32(arg(c, 4) + 0x20, v);
    });
    let l2 = l.clone();
    r.stub(0x191d88, move |c, m| {
        let mut l = l2.borrow_mut();
        l.push("status", arg(c, 4), arg(c, 5));
        let v = l.take();
        for i in 0..0x1e {
            m.write_u32(arg(c, 5) + 4 * i, status_word(v, i));
        }
    });
    answer(r, l, 0x1921e0, "canadd", 1, can_add);
    answer(r, l, 0x1925f8, "drop", 1, |v| v & 1);
    log(r, l, 0x190c28, "apply", 1);
    log(r, l, 0x190c70, "camtrack", 2);
    answer(r, l, 0x190c88, "caninsert", 1, |v| v & 1);
    answer(r, l, 0x190c40, "insert", 2, |v| v & 1);
    answer(r, l, 0x190cb8, "nextcut", 2, |v| v & 1);
}

/// Our side: the same calls and answers
struct Ours(L);
impl OverlayEnv for Ours {
    fn move_sound(&mut self, o: u32) {
        self.0.borrow_mut().push("move", o, 0);
    }
    fn scroll_sound(&mut self) {
        self.0.borrow_mut().push("scroll", 0, 0);
    }
    fn camera_sound(&mut self) {
        self.0.borrow_mut().push("camsnd", 0, 0);
    }
    fn speech_clear(&mut self) {
        self.0.borrow_mut().push("speech", 0, 0);
    }
    fn camera_preset(&mut self) -> u32 {
        let mut l = self.0.borrow_mut();
        l.push("preset", 0, 0);
        preset(l.take())
    }
    fn camera_next_preset(&mut self, n: u32) {
        self.0.borrow_mut().push("next", n, 0);
    }
    fn camera_adjust_chase(&mut self, which: u32) {
        self.0.borrow_mut().push("chase", which, 1.0f32.to_bits());
    }
    fn emitters_update(&mut self) {
        self.0.borrow_mut().push("emit", 0, 0);
    }
    fn crowd_boxes(&mut self, v: u32) {
        self.0.borrow_mut().push("crowd", AUDIO_WORD.with(|w| *w.borrow()), v);
    }
    fn resume_all(&mut self) {
        self.0.borrow_mut().push("resume", 0, 0);
    }
    fn stop_all(&mut self) {
        self.0.borrow_mut().push("stop", 0, 0);
    }
    fn set_playlist(&mut self, m: &mut Mem, mgr: u32, pl: u32) {
        let mut l = self.0.borrow_mut();
        l.push("playlist", mgr, pl);
        let v = new_mode(l.take());
        m.w32(mgr + 0x20, v);
    }
    fn get_status(&mut self, m: &mut Mem, mgr: u32, out: u32) {
        let mut l = self.0.borrow_mut();
        l.push("status", mgr, out);
        let v = l.take();
        for i in 0..0x1e {
            m.w32(out + 4 * i, status_word(v, i));
        }
    }
    fn can_add_highlight(&mut self, _m: &mut Mem, mgr: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.push("canadd", mgr, 0);
        can_add(l.take())
    }
    fn drop_current_highlight(&mut self, _m: &mut Mem, mgr: u32) -> bool {
        let mut l = self.0.borrow_mut();
        l.push("drop", mgr, 0);
        l.take() & 1 != 0
    }
    fn apply_camera_cut(&mut self, _m: &mut Mem, mgr: u32) {
        self.0.borrow_mut().push("apply", mgr, 0);
    }
    fn update_cam_track(&mut self, _m: &mut Mem, mgr: u32, frame: u32) {
        self.0.borrow_mut().push("camtrack", mgr, frame);
    }
    fn can_insert_cut(&mut self, _m: &mut Mem, mgr: u32) -> bool {
        let mut l = self.0.borrow_mut();
        l.push("caninsert", mgr, 0);
        l.take() & 1 != 0
    }
    fn insert_cut(&mut self, _m: &mut Mem, mgr: u32, frame: u32) -> bool {
        let mut l = self.0.borrow_mut();
        l.push("insert", mgr, frame);
        l.take() & 1 != 0
    }
    fn next_cut(&mut self, _m: &mut Mem, mgr: u32, frame: u32) -> bool {
        let mut l = self.0.borrow_mut();
        l.push("nextcut", mgr, frame);
        l.take() & 1 != 0
    }
}

thread_local! {
    /// The audio system's pointer (`DAT_00343050`) as the runner holds it: the crowd call's first argument
    static AUDIO_WORD: RefCell<u32> = const { RefCell::new(0) };
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.step_limit = 200_000;
    Some(r)
}

fn setup(r: &mut Runner) -> (At, L) {
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    stub_all(r, &l);
    let audio = r.mem.read_u32(0x343050);
    AUDIO_WORD.with(|w| *w.borrow_mut() = audio);
    (a, l)
}

/// Run `addr` on the overlay in the runner and `ours` on a copy, both from the same random game
fn check(r: &mut Runner, a: &At, l: &L, g: &mut Rng, cases: u32, addr: u32, what: &str, ours: &mut dyn FnMut(&mut Mem, &L)) {
    for case in 0..cases {
        build(g, r, a);
        let answers: Vec<u32> = (0..12).map(|_| g.next()).collect();
        let mut m = snapshot(r, a);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.cpu.set_gpr(4, a.o() as u64);
        r.call(addr).unwrap_or_else(|e| panic!("{what} case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        ours(&mut m, l);
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "{what} case {case}: calls");
        compare(r, a, &m, &format!("{what} case {case}"));
    }
}

#[test]
fn control_update_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (a, l) = setup(&mut r);
    let mut g = Rng(0x1eb6_d0aa_5511_0001);
    let o = a.o();
    check(&mut r, &a, &l, &mut g, 60_000, 0x1eb6d0, "update", &mut |m, l| update(m, &mut Ours(l.clone()), o));
}

#[test]
fn control_reset_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (a, l) = setup(&mut r);
    let mut g = Rng(0x1ec5_48bb_2222_0002);
    let o = a.o();
    check(&mut r, &a, &l, &mut g, 5_000, 0x1ec548, "reset", &mut |m, l| reset(m, &mut Ours(l.clone()), o));
}

#[test]
fn control_leave_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (a, l) = setup(&mut r);
    log(&mut r, &l, 0x1785c0, "mode", 2);
    let mut g = Rng(0x1ec7_d8cc_3333_0003);
    check(&mut r, &a, &l, &mut g, 500, 0x1ec7d8, "leave", &mut |m, l| {
        leave(m, &mut |x, y| l.borrow_mut().push("mode", x, y));
    });
}

// ---- the race menu's page ----

struct Menu(L);
impl MenuEnv for Menu {
    fn apply_camera_cut(&mut self, _m: &mut Mem, mgr: u32) {
        self.0.borrow_mut().push("apply", mgr, 0);
    }
    fn save_current_keyframe(&mut self, _m: &mut Mem, mgr: u32) {
        self.0.borrow_mut().push("savekf", mgr, 0);
    }
    fn memcard_create(&mut self, o: u32) {
        self.0.borrow_mut().push("create", o, 0);
    }
    fn memcard_begin(&mut self, o: u32, size: u32) {
        self.0.borrow_mut().push("begin", o, size);
    }
    fn memcard_start_save(&mut self, o: u32, x: u32) {
        self.0.borrow_mut().push("startsave", o, x);
    }
    fn memcard_destroy(&mut self, o: u32) {
        self.0.borrow_mut().push("destroy", o, 0);
    }
    fn focus_rider(&mut self, cam: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.push("focus", cam, 0);
        l.take()
    }
    fn set_focus_rider(&mut self, cam: u32, rider: u32) {
        self.0.borrow_mut().push("setfocus", cam, rider);
    }
    fn restore_views(&mut self) {
        self.0.borrow_mut().push("views", 0, 0);
    }
    fn new_buttons(&mut self, o: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.push("buttons", o, 0);
        let v = l.take();
        menu_buttons(v)
    }
    fn move_sound(&mut self, b: u32) {
        self.0.borrow_mut().push("move", b, 0);
    }
    fn back_sound(&mut self, b: u32) {
        self.0.borrow_mut().push("back", b, 0);
    }
    fn select_sound(&mut self, b: u32) {
        self.0.borrow_mut().push("select", b, 0);
    }
    fn set_state(&mut self, o: u32, s: u32) {
        self.0.borrow_mut().push("state", o, s);
    }
}

fn menu_buttons(v: u32) -> u32 {
    [0x80, 0x20, 0x800, 0x1, 0x20000, 0x2, 0x40000, 0, 0x880, 0x801, 0x40002][(v % 11) as usize] | (v & 0x1000)
}

fn stub_menu(r: &mut Runner, l: &L) {
    log(r, l, 0x193d30, "savekf", 1);
    log(r, l, 0x1cee68, "create", 1);
    log(r, l, 0x1ce8b0, "begin", 2);
    log(r, l, 0x1ce778, "startsave", 2);
    log(r, l, 0x1ceeb0, "destroy", 1);
    answer(r, l, 0x174008, "focus", 1, |v| v);
    log(r, l, 0x181d50, "views", 0);
    log(r, l, 0x173c50, "setfocus", 2);
    answer(r, l, 0x1f0498, "buttons", 1, menu_buttons);
    log(r, l, 0x1a8658, "back", 1);
    log(r, l, 0x1a8668, "select", 1);
    log(r, l, 0x1ef990, "state", 2);
    r.allow(0x1a7c40);
}

#[test]
fn race_menu_replay_page_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (a, l) = setup(&mut r);
    stub_menu(&mut r, &l);
    let o = a.o();
    let mut g = Rng(0x1ef7_f0dd_4444_0004);
    check(&mut r, &a, &l, &mut g, 5_000, 0x1ef7f0, "begin save", &mut |m, l| begin_save(m, &mut Menu(l.clone()), o));
    check(&mut r, &a, &l, &mut g, 3_000, 0x1ef918, "end save", &mut |m, l| end_save(m, &mut Menu(l.clone()), o));
    // the page's own virtual call takes the overlay plus the vtable's adjustment
    check(&mut r, &a, &l, &mut g, 20_000, 0x1efef0, "menu", &mut |m, l| {
        let adj = m.r16(m.r32(o + 0x428) + 0x18) as i16 as i32;
        let mut e = MenuAdj(Menu(l.clone()), (o as i32 + adj) as u32);
        update_menu(m, &mut e, o)
    });
}

/// The menu env with the button call's `this` as the runner sees it
struct MenuAdj(Menu, u32);
impl MenuEnv for MenuAdj {
    fn apply_camera_cut(&mut self, m: &mut Mem, mgr: u32) {
        self.0.apply_camera_cut(m, mgr)
    }
    fn save_current_keyframe(&mut self, m: &mut Mem, mgr: u32) {
        self.0.save_current_keyframe(m, mgr)
    }
    fn memcard_create(&mut self, o: u32) {
        self.0.memcard_create(o)
    }
    fn memcard_begin(&mut self, o: u32, size: u32) {
        self.0.memcard_begin(o, size)
    }
    fn memcard_start_save(&mut self, o: u32, x: u32) {
        self.0.memcard_start_save(o, x)
    }
    fn memcard_destroy(&mut self, o: u32) {
        self.0.memcard_destroy(o)
    }
    fn focus_rider(&mut self, cam: u32) -> u32 {
        self.0.focus_rider(cam)
    }
    fn set_focus_rider(&mut self, cam: u32, rider: u32) {
        self.0.set_focus_rider(cam, rider)
    }
    fn restore_views(&mut self) {
        self.0.restore_views()
    }
    fn new_buttons(&mut self, _o: u32) -> u32 {
        self.0.new_buttons(self.1)
    }
    fn move_sound(&mut self, b: u32) {
        self.0.move_sound(b)
    }
    fn back_sound(&mut self, b: u32) {
        self.0.back_sound(b)
    }
    fn select_sound(&mut self, b: u32) {
        self.0.select_sound(b)
    }
    fn set_state(&mut self, o: u32, s: u32) {
        self.0.set_state(o, s)
    }
}

// ---- the card save's page ----

fn cstr(m: &mut r5900::Mem, mut at: u32) -> String {
    let mut s = String::new();
    loop {
        let mut b = [0u8];
        m.read(at, &mut b).unwrap();
        if b[0] == 0 {
            return s;
        }
        s.push(b[0] as char);
        at += 1;
    }
}

struct Save(L);
impl SaveOverlayEnv for Save {
    fn save_init(&mut self, _m: &mut Mem, mgr: u32) {
        self.0.borrow_mut().push("init", mgr, 0);
    }
    fn crc16(&mut self, m: &mut Mem, at: u32, n: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.push("crc", at, n);
        l.push("crcword", m.r32(SAVE_DATA + 0x334), 0);
        l.take() & 0xffff
    }
    fn game_code(&mut self) -> String {
        self.0.borrow_mut().push("code", 0, 0);
        "BASLUS-20326".into()
    }
    fn tag(&mut self, set: bool) -> String {
        self.0.borrow_mut().push(if set { "set" } else { "rep" }, 8, 0);
        if set { "SET".into() } else { "REP".into() }
    }
    fn start_write(&mut self, o: u32, name: &str, at: u32, n: u32) {
        let mut l = self.0.borrow_mut();
        l.push("write", o, at ^ n);
        NAMES.with(|v| v.borrow_mut().push(name.to_string()));
    }
    fn save_step(&mut self, _m: &mut Mem, mgr: u32, n: u32) -> Option<(u32, u32)> {
        let mut l = self.0.borrow_mut();
        l.push("step", mgr, n);
        let v = l.take();
        (v & 1 != 0).then_some((v >> 8, v & 0x7ff0))
    }
    fn write_chunk(&mut self, o: u32, at: u32, n: u32) {
        self.0.borrow_mut().push("chunk", o, at ^ n.rotate_left(16));
    }
    fn after_save(&mut self, _m: &mut Mem, mgr: u32, which: u32) {
        self.0.borrow_mut().push(if which == 0x194870 { "done" } else { "abort" }, mgr, 0);
    }
}

thread_local! {
    static NAMES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[test]
fn card_save_page_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let (a, l) = setup(&mut r);
    log(&mut r, &l, 0x193d98, "init", 1);
    let l2 = l.clone();
    r.stub(0x2ce04c, move |c, m| {
        let mut l = l2.borrow_mut();
        l.push("crc", arg(c, 4), arg(c, 5));
        l.push("crcword", m.read_u32(SAVE_DATA + 0x334), 0);
        let v = l.take() & 0xffff;
        c.set_gpr(2, v as u64);
    });
    let l2 = l.clone();
    r.stub(0x1d2f60, move |c, m| {
        l2.borrow_mut().push("code", 0, 0);
        assert_eq!(arg(c, 5), 16);
        m.write(arg(c, 4), b"BASLUS-20326\0").unwrap();
    });
    for (addr, name, text) in [(0x1d2fa0u32, "rep", b"REP\0"), (0x1d2fc8, "set", b"SET\0")] {
        let l2 = l.clone();
        r.stub(addr, move |c, m| {
            l2.borrow_mut().push(name, arg(c, 5), 0);
            m.write(arg(c, 4), text).unwrap();
        });
    }
    // sprintf: only the format the page uses
    r.stub(0x2fca28, move |c, m| {
        assert_eq!(cstr(m, arg(c, 5)), "%s-%s%04d");
        let s = format!("{}-{}{:04}", cstr(m, arg(c, 6)), cstr(m, arg(c, 7)), c.gpr(8) as u32 as i32);
        let mut b = s.into_bytes();
        b.push(0);
        m.write(arg(c, 4), &b).unwrap();
    });
    let l2 = l.clone();
    r.stub(0x1d1d80, move |c, m| {
        l2.borrow_mut().push("write", arg(c, 4), arg(c, 6) ^ arg(c, 7));
        let s = cstr(m, arg(c, 5));
        NAMES.with(|v| v.borrow_mut().push(s));
    });
    let l2 = l.clone();
    r.stub(0x193f70, move |c, m| {
        let mut l = l2.borrow_mut();
        l.push("step", arg(c, 4), m.read_u32(arg(c, 6)));
        let v = l.take();
        if v & 1 != 0 {
            m.write_u32(arg(c, 5), v >> 8);
            m.write_u32(arg(c, 6), v & 0x7ff0);
        }
        c.set_gpr(2, (v & 1) as u64);
    });
    let l2 = l.clone();
    r.stub(0x1d1f18, move |c, _| l2.borrow_mut().push("chunk", arg(c, 4), arg(c, 5) ^ arg(c, 6).rotate_left(16)));
    log(&mut r, &l, 0x194870, "done", 1);
    log(&mut r, &l, 0x1948c0, "abort", 1);
    let o = a.o();
    let mut g = Rng(0x1f03_08ee_5555_0005);
    let names = |what: &str| {
        NAMES.with(|v| {
            let v = std::mem::take(&mut *v.borrow_mut());
            assert!(v.len() == 2 && v[0] == v[1], "{what}: names {v:?}");
        })
    };
    for _ in 0..200 {
        check(&mut r, &a, &l, &mut g, 1, 0x1f0308, "save begin", &mut |m, l| save_begin(m, &mut Save(l.clone()), o));
        names("save begin");
    }
    for case in 0..2_000u32 {
        let k = g.next() >> (case % 20);
        let mut ok = 0u32;
        check_with_a1(&mut r, &a, &l, &mut g, k, 0x1f0400, "save next", &mut |m, l| {
            ok = save_next(m, &mut Save(l.clone()), o, k) as u32;
        });
        assert_eq!(r.cpu.gpr(2) as u32, ok, "save next {case}: result");
    }
    check(&mut r, &a, &l, &mut g, 300, 0x1f0468, "save done", &mut |m, l| save_finish(m, &mut Save(l.clone()), false));
    check(&mut r, &a, &l, &mut g, 300, 0x1f0480, "save abort", &mut |m, l| save_finish(m, &mut Save(l.clone()), true));
}

fn check_with_a1(r: &mut Runner, a: &At, l: &L, g: &mut Rng, a1: u32, addr: u32, what: &str, ours: &mut dyn FnMut(&mut Mem, &L)) {
    build(g, r, a);
    let answers: Vec<u32> = (0..12).map(|_| g.next()).collect();
    let mut m = snapshot(r, a);
    *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
    r.cpu.set_gpr(4, a.o() as u64);
    r.cpu.set_gpr(5, a1 as i32 as i64 as u64);
    r.call(addr).unwrap_or_else(|e| panic!("{what}: {e}"));
    let theirs = std::mem::take(&mut l.borrow_mut().calls);
    *l.borrow_mut() = Log { answers, ..Log::default() };
    ours(&mut m, l);
    let mine = std::mem::take(&mut l.borrow_mut().calls);
    assert_eq!(theirs, mine, "{what}: calls");
    compare(r, a, &m, what);
}

// ---- the two replay sounds ----

const N: usize = 8;
const PTRS: [u32; 13] = [0x24, 0x38, 0x3c, 0x40, 0x64, 0x68, 0x6c, 0x70, 0x74, 0x78, 0x7c, 0xa40, 0xa44];

fn read_queue(r: &mut Runner, sys: u32, arr: &[u32; 13]) -> Queue {
    let mut q = Queue::with_capacity(N);
    r.mem.read(sys + 8, &mut q.header).unwrap();
    q.count = r.mem.read_u32(sys + 0x20) as i32;
    let w = |r: &mut Runner, p: u32, n: usize| -> Vec<u32> { (0..n).map(|k| r.mem.read_u32(p + 4 * k as u32)).collect() };
    for i in 0..N {
        r.mem.read(arr[0] + 24 * i as u32, &mut q.record[i]).unwrap();
    }
    for i in 0..7 {
        q.emitter[i] = r.mem.read_u32(sys + 0x28 + 4 * i as u32);
    }
    q.a38 = w(r, arr[1], N);
    q.a3c = w(r, arr[2], N);
    q.a40 = w(r, arr[3], N);
    for i in 0..4 {
        q.a44[i] = [r.mem.read_u32(sys + 0x44 + 8 * i as u32), r.mem.read_u32(sys + 0x48 + 8 * i as u32)];
    }
    q.a64 = w(r, arr[4], N);
    q.a68 = w(r, arr[5], N);
    q.a6c = w(r, arr[6], N);
    q.a70 = w(r, arr[7], N);
    for i in 0..N {
        let v = w(r, arr[8] + 16 * i as u32, 4);
        q.a74[i] = [v[0], v[1], v[2], v[3]];
    }
    q.a78 = w(r, arr[9], N);
    q.a7c = w(r, arr[10], N);
    q.kind = w(r, arr[11], N);
    q.id = w(r, arr[12], N);
    q
}

struct Snd(L);
impl QueueEnv for Snd {
    fn get_slot(&mut self, which: u32) -> u32 {
        let mut l = self.0.borrow_mut();
        l.push("slot", which, 0);
        l.take()
    }
    fn rider_listener(&mut self) -> u32 {
        unreachable!()
    }
    fn play_pending(&mut self) -> u64 {
        let mut l = self.0.borrow_mut();
        l.push("pp", 0, 0);
        l.take() as u64
    }
}

#[test]
fn replay_sounds_match_the_original() {
    let Some(mut r) = runner() else { return };
    let l: L = Rc::new(RefCell::new(Log::default()));
    let sys = r.mem.alloc(0x6f00);
    let mut arr = [0u32; 13];
    for p in arr.iter_mut() {
        *p = r.mem.alloc(24 * N as u32);
    }
    let l2 = l.clone();
    r.stub(0x211f38, move |c, _| {
        let mut l = l2.borrow_mut();
        l.push("slot", arg(c, 5), 0);
        let v = l.take();
        c.set_gpr(2, v as u64);
    });
    let l2 = l.clone();
    r.stub(0x20c948, move |c, _| {
        let mut l = l2.borrow_mut();
        l.push("pp", 0, 0);
        let v = l.take();
        c.set_gpr(2, v as u64);
    });
    r.allow(0x2fb468);
    let mut g = Rng(0x2144_28ff_6666_0006);
    for case in 0..10_000 {
        for k in 0..0xb00 / 4 {
            r.mem.write_u32(sys + 4 * k, g.next());
        }
        for (k, p) in PTRS.iter().enumerate() {
            r.mem.write_u32(sys + p, arr[k]);
        }
        for p in arr {
            for j in 0..(24 * N as u32 / 4) {
                r.mem.write_u32(p + 4 * j, g.next());
            }
        }
        r.mem.write_u32(sys + 0x20, g.below(3));
        let (addr, id) = if case & 1 == 0 { (0x214428, 0x7d) } else { (0x214650, 0x7f) };
        let answers = vec![g.next(), g.next()];
        let before = read_queue(&mut r, sys, &arr);
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        r.cpu.set_gpr(4, sys as u64);
        r.call(addr).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        let after = read_queue(&mut r, sys, &arr);
        let mut q = before;
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let v = replay_sound(&mut q, id, &mut Snd(l.clone()));
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        assert_eq!(theirs, mine, "case {case}");
        assert_eq!(after, q, "case {case} queue");
        assert_eq!(r.mem.read_u32(sys + 0x6e48), v as u32, "case {case} +0x6e48");
    }
}
