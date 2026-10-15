//! `tricky_game::menudraw` (board row G8f) against the original in the function runner (TRICKY_ELF; skipped
//! without it): random backdrops, menus and labels, every draw call logged on both sides (floats bit for bit) and
//! all memory compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::menudraw::*;
use tricky_game::pf::Mem;
use tricky_game::replaymgr::GAPP;
use tricky_game::replayui::{DrawEnv, PanelEnv, FONT_COLOR};

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

#[derive(Default)]
struct Log {
    calls: Vec<(&'static str, Vec<u32>)>,
    answers: Vec<u32>,
    next: usize,
}
impl Log {
    fn take(&mut self) -> u32 {
        let v = self.answers.get(self.next).copied().unwrap_or(0);
        self.next += 1;
        v
    }
}
type L = Rc<RefCell<Log>>;

const R_FN: u32 = 0x7e0000;
const ARENA: u32 = 0x2000;
fn fb(f: f32) -> u32 {
    f.to_bits()
}

struct At {
    b: u32,
}
impl At {
    fn back(&self) -> u32 {
        self.b
    }
    fn font(&self) -> u32 {
        self.b + 0x400
    }
    fn rect(&self) -> u32 {
        self.b + 0x480
    }
    fn app(&self) -> u32 {
        self.b + 0x500
    }
    fn renderer(&self) -> u32 {
        self.b + 0x600
    }
    fn r_vt(&self) -> u32 {
        self.b + 0x700
    }
    fn items(&self) -> u32 {
        self.b + 0x980
    }
    fn flags(&self) -> u32 {
        self.b + 0x9c0
    }
}

fn regions(a: &At) -> Vec<(u32, u32)> {
    vec![(FONT_COLOR, 0x14), (PULSE, 8), (GAPP, 4), (a.b, ARENA)]
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

fn fpick(g: &mut Rng, xs: &[f32]) -> u32 {
    xs[g.below(xs.len() as u32) as usize].to_bits()
}

fn build(g: &mut Rng, r: &mut Runner, a: &At) {
    for k in 0..ARENA / 4 {
        r.mem.write_u32(a.b + 4 * k, fpick(g, &[0.0, 1.0, 0.5, 12.5, -3.0, 300.0]));
    }
    let w = |r: &mut Runner, at: u32, v: u32| r.mem.write_u32(at, v);
    for k in 0..5 {
        w(r, FONT_COLOR + 4 * k, fpick(g, &[1.0, 0.5, 0.25]));
    }
    w(r, PULSE, fpick(g, &[1.0, 1.05, 1.11, 0.98, 0.995]));
    w(r, PULSE + 4, fpick(g, &[0.01, -0.01, 0.003]));
    w(r, GAPP, a.app());
    w(r, a.app() + 0x724, a.renderer());
    w(r, a.renderer() + 0x10, a.r_vt());
    for s in [0x1c4u32, 0x1cc, 0x1d4, 0x1e4, 0x1f4, 0x1fc] {
        w(r, a.r_vt() + s, R_FN + s);
        r.mem.write_u16(a.r_vt() + s - 4, 0);
    }
    let b = a.back();
    w(r, b, 0x1234);
    w(r, b + 4, a.font());
    w(r, b + 8, a.font());
    w(r, a.font() + 0xc, [16, 20, 13][g.below(3) as usize]);
    w(r, b + 0x60, [0x40, 0, 2, 0x46][g.below(4) as usize]);
    w(r, b + 0x2c, fpick(g, &[300.0, 120.5, 0.0]));
    w(r, b + 0x38, fpick(g, &[0.0, -640.0, 33.0]));
    w(r, b + 0x48, fpick(g, &[0.0, 20.0, -480.0]));
    w(r, b + 0x50, fpick(g, &[1.0, 0.65, 0.0]));
    w(r, b + 0x70, fpick(g, &[1.0, 0.5, 0.0]));
    w(r, b + 0x80, fpick(g, &[0.0, -850.0, 12.0]));
    w(r, b + 0x88, fpick(g, &[0.0, -680.0, 4.0]));
    w(r, a.rect() + 4, fpick(g, &[128.0, 200.0, 33.0]));
    w(r, a.rect() + 8, fpick(g, &[64.0, 47.5]));
    for i in 0..8 {
        w(r, a.items() + 4 * i, 0x5000 + i);
        w(r, a.flags() + 4 * i, g.below(3).min(1));
    }
}

fn arg(c: &r5900::Cpu, i: usize) -> u32 {
    c.gpr(i) as u32
}

fn width(v: u32) -> i32 {
    [0, 40, 123, 300][(v % 4) as usize]
}

struct Ours(L, u32);
impl Ours {
    fn push(&self, n: &'static str, v: Vec<u32>) {
        self.0.borrow_mut().calls.push((n, v));
    }
}
impl DrawEnv for Ours {
    fn renderer(&mut self, slot: u32, a1: u32, a2: u32) {
        let v = if slot == 0x1fc || slot == 0x1f4 { vec![slot, a1, a2] } else { vec![slot, a1] };
        self.push("rend", v);
    }
    fn renderer_this(&mut self, slot: u32) {
        self.push("rend", vec![slot]);
    }
    fn renderer_view(&mut self, slot: u32, _v: [f32; 6]) {
        self.push("rend", vec![slot]);
    }
    fn sprite_rect(&mut self, set: u32, id: u32) -> u32 {
        self.push("rect", vec![set, id]);
        self.1 + 0x480
    }
    fn sprite(&mut self, x: f32, y: f32, w: f32, h: f32, o: u32, id: i32, corner: u32) {
        self.push("sprite", vec![fb(x), fb(y), fb(w), fb(h), o, id as u32, corner]);
    }
    fn set_color(&mut self, c: [f32; 4]) {
        self.push("color", c.iter().map(|f| fb(*f)).collect());
    }
    fn loc(&mut self, id: u32) -> u32 {
        id
    }
    fn measure_wide(&mut self, scale: f32, font: u32, s: u32) -> i32 {
        let mut l = self.0.borrow_mut();
        l.calls.push(("wide", vec![fb(scale), font, s]));
        width(l.take())
    }
    fn measure_ascii(&mut self, _scale: f32, _font: u32, _s: u32) -> i32 {
        0
    }
    fn label_box(&mut self, _y: f32, _o: u32, _s: u32, _style: u32) {}
    fn print(&mut self, func: u32, scale: f32, x: f32, y: f32, font: u32, s: u32) {
        self.push("print", vec![func, fb(scale), fb(x), fb(y), font, s]);
    }
    fn chase_index(&mut self) -> i32 {
        0
    }
    fn camera_preset(&mut self) -> u32 {
        0
    }
    fn backdrop_frame(&mut self, _y: f32, _b: u32, _a1: u32) {}
    fn menu_list(&mut self, _x: f32, _y: f32, _scale: f32, _b: u32, _items: [u32; 3], _sel: u32, _enabled: [u32; 3]) {}
}
impl PanelEnv for Ours {
    fn bar(&mut self, _x: f32, _y: f32, _w: f32, _h: f32, _o: u32, _kind: u32) {}
    fn line(&mut self, _x0: f32, _y0: f32, _x1: f32, _y1: f32) {}
    fn fill_rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.push("fill", vec![fb(x), fb(y), fb(w), fb(h)]);
    }
}
impl MenuDrawEnv for Ours {
    fn sprite_scaled(&mut self, x: f32, y: f32, sx: f32, sy: f32, rect: u32, corner: u32) {
        self.push("scaled", vec![fb(x), fb(y), fb(sx), fb(sy), rect, corner]);
    }
    fn renderer_scaled(&mut self, slot: u32, a1: u32, a2: u32, f: f32) {
        self.push("rend", vec![slot, a1, a2, fb(f)]);
    }
}

fn setup(r: &mut Runner, l: &L, a: &At) {
    let base = a.b;
    for s in [0x1c4u32, 0x1cc, 0x1d4, 0x1e4, 0x1f4, 0x1fc] {
        let l2 = l.clone();
        r.stub(R_FN + s, move |c, _| {
            let v = match s {
                0x1fc | 0x1f4 => vec![s, arg(c, 5), arg(c, 6)],
                0x1cc => vec![s, arg(c, 5), arg(c, 6), fb(c.f(12))],
                _ => vec![s, arg(c, 5)],
            };
            l2.borrow_mut().calls.push(("rend", v));
        });
    }
    let l2 = l.clone();
    r.stub(0x1f9e50, move |c, _| {
        l2.borrow_mut().calls.push(("rect", vec![arg(c, 4), arg(c, 5)]));
        c.set_gpr(2, (base + 0x480) as u64);
    });
    let l2 = l.clone();
    r.stub(0x1c0068, move |c, _| {
        l2.borrow_mut().calls.push(("sprite", vec![fb(c.f(12)), fb(c.f(13)), fb(c.f(14)), fb(c.f(15)), arg(c, 4), arg(c, 5), arg(c, 6)]));
    });
    let l2 = l.clone();
    r.stub(0x14eee8, move |c, m| {
        assert_eq!(arg(c, 4), FONT_COLOR);
        let p = arg(c, 5);
        let v = (0..4).map(|k| m.read_u32(p + 4 * k)).collect();
        l2.borrow_mut().calls.push(("color", v));
    });
    let l2 = l.clone();
    r.stub(0x19b108, move |c, _| {
        assert_eq!(arg(c, 4), FONT_COLOR);
        l2.borrow_mut().calls.push(("fill", vec![fb(c.f(12)), fb(c.f(13)), fb(c.f(14)), fb(c.f(15))]));
    });
    let l2 = l.clone();
    r.stub(0x19dc78, move |c, _| {
        assert_eq!(arg(c, 6), 0);
        let mut l = l2.borrow_mut();
        l.calls.push(("wide", vec![fb(c.f(12)), arg(c, 4), arg(c, 5)]));
        let v = width(l.take());
        c.set_gpr(2, v as i64 as u64);
    });
    let l2 = l.clone();
    r.stub(0x19d568, move |c, _| {
        l2.borrow_mut().calls.push(("print", vec![0x19d568, fb(c.f(12)), fb(c.f(13)), fb(c.f(14)), arg(c, 4), arg(c, 5)]));
    });
    let l2 = l.clone();
    r.stub(0x14eed0, move |c, _| {
        l2.borrow_mut().calls.push(("scaled", vec![fb(c.f(12)), fb(c.f(13)), fb(c.f(14)), fb(c.f(15)), arg(c, 4), arg(c, 5)]));
    });
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = r5900::ps2float::Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn menu_drawing_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let a = At { b: r.mem.alloc(ARENA) };
    let l: L = Rc::new(RefCell::new(Log::default()));
    setup(&mut r, &l, &a);
    let mut g = Rng(0x1aa6_30ff_8f8f_0001);
    let b = a.back();
    for case in 0..40_000u32 {
        build(&mut g, &mut r, &a);
        let which = g.below(3);
        let answers: Vec<u32> = (0..32).map(|_| g.next()).collect();
        let fl = |g: &mut Rng, xs: &[f32]| xs[g.below(xs.len() as u32) as usize];
        let (x, y, scale) = (fl(&mut g, &[320.0, 100.5, 0.0]), fl(&mut g, &[240.0, 80.0, 18.0]), fl(&mut g, &[1.0, 0.92, 0.79]));
        let layer = g.below(20);
        let align = g.below(16);
        let title = if g.below(2) == 0 { 0 } else { 0x6000 };
        let n = g.below(9);
        let sel = (g.below(n.max(1) + 1) as i32) - 1;
        let pointer = [-1i32, 116, 7][g.below(3) as usize];
        let pulse_on = g.below(2) == 1 && sel >= 0;
        let items: Vec<u32> = (0..n).map(|i| 0x5000 + i).collect();
        let mut m = snapshot(&mut r, &a);
        let enabled: Vec<u32> = (0..8).map(|i| m.r32(a.flags() + 4 * i)).collect();
        match which {
            0 => {
                r.cpu.set_f(12, y);
                r.cpu.set_gpr(4, b as u64);
                r.cpu.set_gpr(5, layer as u64);
            }
            1 => {
                r.cpu.set_f(12, x);
                r.cpu.set_f(13, y);
                r.cpu.set_f(14, scale);
                for (k, v) in [b, align, title, a.items(), n, sel as u32, pointer as u32, 1].iter().enumerate() {
                    r.cpu.set_gpr(4 + k, *v as i32 as i64 as u64);
                }
                // the stack's two: the pulse on, the enabled flags
                let sp = 0x01ff_0000u32;
                r.mem.write_u32(sp, pulse_on as u32);
                r.mem.write_u32(sp + 8, a.flags());
            }
            _ => {
                r.cpu.set_f(12, y);
                r.cpu.set_gpr(4, b as u64);
                r.cpu.set_gpr(5, 0x6000);
                r.cpu.set_gpr(6, layer as u64);
            }
        }
        *l.borrow_mut() = Log { answers: answers.clone(), ..Log::default() };
        let addr = [0x1a9bb8u32, 0x1aa630, 0x1a9928][which as usize];
        r.call(addr).unwrap_or_else(|e| panic!("case {case} ({which}): {e}"));
        let theirs = std::mem::take(&mut l.borrow_mut().calls);
        *l.borrow_mut() = Log { answers, ..Log::default() };
        let mut e = Ours(l.clone(), a.b);
        match which {
            0 => draw_frame(&mut m, &mut e, y, b, layer),
            1 => draw_menu_list(&mut m, &mut e, x, y, scale, b, align, title, &items, sel, pointer, pulse_on, &enabled),
            _ => draw_label_box(&mut m, &mut e, y, b, 0x6000, layer),
        }
        let mine = std::mem::take(&mut l.borrow_mut().calls);
        if theirs != mine {
            let i = (0..theirs.len().min(mine.len())).find(|&i| theirs[i] != mine[i]).unwrap_or(theirs.len().min(mine.len()));
            panic!("case {case} ({which}): call {i}: {:x?} vs {:x?}", theirs.get(i), mine.get(i));
        }
        compare(&mut r, &a, &m, &format!("case {case} ({which})"));
    }
}
