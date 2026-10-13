//! `tricky_game::camscript::update` against `CamScript_Update` 0x175f50 in the function runner (TRICKY_ELF; skipped
//! without it). `Camera_Cut`, `CamScript_Stop`, `SetName` and `ResetActorAnims` run as they are; the director, the
//! cuts, blends, path start, scene start and the sound and world calls are stubbed and logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::campath::*;
use tricky_game::camscript::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

const OBJ_WORDS: usize = 0xf00 / 4;
const RIDER: u32 = 0x4000_0000;
const STRIDE: u32 = 0x8000;

struct Script {
    name: Vec<u8>,
    addr: u32,
    steps: Vec<Step>,
}

#[derive(Default)]
struct W {
    scripts: Vec<Script>,
    oracle: Vec<u32>,
    pos: usize,
    log: Vec<Vec<u32>>,
    entry_count: i32,
    list_flag: u32,
    statics: [u32; 3],
    race_flag: u32,
}
impl W {
    fn next(&mut self) -> u32 {
        let v = self.oracle[self.pos % self.oracle.len()];
        self.pos += 1;
        v
    }
}

struct U(Rc<RefCell<W>>);
impl KeyEnv for U {
    fn key(&mut self, _: i32) -> Key {
        unreachable!()
    }
}
impl ScriptEnv for U {
    fn owner(&mut self, _: u32) -> Owner {
        unreachable!()
    }
    fn smooth(&mut self, _: &mut [u32]) {
        unreachable!()
    }
    fn cut_fade(&mut self, _: &mut [u32]) {
        unreachable!()
    }
    fn audio_cut(&mut self) {
        self.0.borrow_mut().log.push(vec![6]);
    }
    fn find_owner(&mut self) -> u32 {
        unreachable!()
    }
    fn first_position(&mut self, _: u32) -> [f32; 3] {
        unreachable!()
    }
    fn audio_stop(&mut self) {
        self.0.borrow_mut().log.push(vec![9]);
    }
    fn entry_count(&mut self) -> i32 {
        self.0.borrow().entry_count
    }
    fn reset_rider_anim(&mut self, j: i32) {
        self.0.borrow_mut().log.push(vec![10, j as u32]);
    }
}
fn sp(w: &W, dp: u32, i: i32) -> u32 {
    let _ = w;
    dp + i as u32 * 0x2c
}
impl UpdateEnv for U {
    fn director_open(&mut self, name: &[u8]) -> u32 {
        let mut w = self.0.borrow_mut();
        let dp = w.scripts.iter().find(|s| s.name == name).map_or(0, |s| s.addr);
        w.log.push(vec![20, dp]);
        dp
    }
    fn director_close(&mut self, dp: u32) {
        self.0.borrow_mut().log.push(vec![21, dp]);
    }
    fn step(&mut self, dp: u32, i: i32) -> Step {
        let w = self.0.borrow();
        w.scripts.iter().find(|s| s.addr == dp).unwrap().steps[i as usize].clone()
    }
    fn count(&mut self, dp: u32) -> i32 {
        self.0.borrow().scripts.iter().find(|s| s.addr == dp).unwrap().steps.len() as i32
    }
    fn wait(&mut self) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![22]);
        w.next() % 2
    }
    fn rand(&mut self) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![23]);
        w.next()
    }
    fn cut_to_current(&mut self, _: &mut [u32]) {
        self.0.borrow_mut().log.push(vec![24, 0]);
    }
    fn cut_to_step(&mut self, _: &mut [u32], dp: u32, i: i32) {
        let mut w = self.0.borrow_mut();
        let p = sp(&w, dp, i);
        w.log.push(vec![24, p]);
    }
    fn blend(&mut self, _: &mut [u32], dp: u32, i: i32, mode: u32, orbit: u32, flip: u32, start: f32, now: f32, end: f32) -> u32 {
        let mut w = self.0.borrow_mut();
        let p = sp(&w, dp, i);
        w.log.push(vec![25, p, mode, orbit, flip, start.to_bits(), now.to_bits(), end.to_bits()]);
        w.next() % 2
    }
    fn start_path_step(&mut self, _: &mut [u32], dp: u32, i: i32, mode: u32, speed: f32) {
        let mut w = self.0.borrow_mut();
        let p = sp(&w, dp, i);
        w.log.push(vec![26, p, mode, speed.to_bits()]);
    }
    fn set_mode(&mut self, _: &mut [u32], mode: u32) {
        self.0.borrow_mut().log.push(vec![27, mode]);
    }
    fn start_scene(&mut self, _: &mut [u32], dp: u32, i: i32) {
        let mut w = self.0.borrow_mut();
        let p = sp(&w, dp, i);
        w.log.push(vec![28, p]);
    }
    fn rider_anim_done(&mut self) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![29]);
        w.next() % 2
    }
    fn sound_a(&mut self) {
        self.0.borrow_mut().log.push(vec![30]);
    }
    fn sound_b(&mut self) {
        self.0.borrow_mut().log.push(vec![31]);
    }
    fn sound_busy(&mut self) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![32]);
        w.next() % 2
    }
    fn sound_set(&mut self, v: u32) {
        self.0.borrow_mut().log.push(vec![33, v]);
    }
    fn sound_list(&mut self, list: u32, mode: u32) {
        self.0.borrow_mut().log.push(vec![34, list, mode]);
    }
    fn set_statics(&mut self) {
        self.0.borrow_mut().statics = [1, 0, 0.5f32.to_bits()];
    }
    fn run_script(&mut self, name: &[u8]) {
        let mut v = vec![35];
        v.extend(name.iter().map(|b| *b as u32));
        self.0.borrow_mut().log.push(v);
    }
    fn race_flag(&mut self, v: u32) {
        self.0.borrow_mut().race_flag = v;
    }
    fn list_flag(&mut self, _: u32) -> u32 {
        self.0.borrow().list_flag
    }
}

fn cstr(m: &mut r5900::Mem, a: u32) -> Vec<u8> {
    let mut v = vec![];
    loop {
        let b = (m.read_u32((a + v.len() as u32) & !3) >> (8 * ((a + v.len() as u32) & 3))) as u8;
        if b == 0 {
            return v;
        }
        v.push(b);
    }
}

#[test]
fn camera_script_update_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x175f50u32, 0x175e88, 0x175eb8, 0x176f38, 0x177db0, 0x175c70] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0xf00);
    let app = r.mem.alloc(0x800);
    let racepl = r.mem.alloc(0x100);
    let listp = r.mem.alloc(0x200);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, racepl);
    let w = Rc::new(RefCell::new(W::default()));
    let names: [&[u8]; 4] = [b"s0", b"s1", b"s2", b"s3"];
    for n in names {
        let addr = r.mem.alloc(0x400);
        w.borrow_mut().scripts.push(Script { name: n.to_vec(), addr, steps: vec![] });
    }

    let l = w.clone();
    r.stub(0x16e6b0, move |c, m| {
        let n = cstr(m, c.gpr(5) as u32);
        let mut w = l.borrow_mut();
        let dp = w.scripts.iter().find(|s| s.name == n).map_or(0, |s| s.addr);
        m.write_u32(c.gpr(4) as u32 + 4, dp);
        w.log.push(vec![20, dp]);
    });
    let l = w.clone();
    r.stub(0x16e7c0, move |c, m| {
        let dp = m.read_u32(c.gpr(4) as u32 + 4);
        l.borrow_mut().log.push(vec![21, dp]);
    });
    macro_rules! stub {
        ($addr:expr, $body:expr) => {{
            let l = w.clone();
            r.stub($addr, move |c, m| {
                let f: &dyn Fn(&mut r5900::Cpu, &mut r5900::Mem, &mut W) = &$body;
                f(c, m, &mut l.borrow_mut())
            });
        }};
    }
    stub!(0x230ff8, |c, _, w| {
        w.log.push(vec![22]);
        let v = w.next() % 2;
        c.set_gpr(2, v as u64);
    });
    stub!(0x23d4a8, |c, _, w| {
        w.log.push(vec![23]);
        let v = w.next();
        c.set_gpr(2, v as u64);
    });
    stub!(0x174c98, |c, _, w| {
        let a1 = c.gpr(5) as u32;
        let cur = c.gpr(4) as u32 + 0x24;
        w.log.push(vec![24, if a1 == cur { 0 } else { a1 }]);
    });
    stub!(0x175a08, |c, _, w| {
        w.log.push(vec![25, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32, c.gpr(9) as u32, c.f(12).to_bits(), c.f(13).to_bits(), c.f(14).to_bits()]);
        let v = w.next() % 2;
        c.set_gpr(2, v as u64);
    });
    stub!(0x175ba0, |c, _, w| {
        w.log.push(vec![26, c.gpr(5) as u32, c.gpr(6) as u32, c.f(12).to_bits()]);
    });
    stub!(0x1776d0, |c, _, w| w.log.push(vec![27, c.gpr(5) as u32]));
    stub!(0x176850, |c, _, w| w.log.push(vec![28, c.gpr(5) as u32]));
    stub!(0x16db88, |c, _, w| {
        assert_eq!(c.gpr(4) as u32, obj + 0x220);
        w.log.push(vec![29]);
        let v = w.next() % 2;
        c.set_gpr(2, v as u64);
    });
    stub!(0x214e08, |_, _, w| w.log.push(vec![30]));
    stub!(0x214e00, |_, _, w| w.log.push(vec![31]));
    stub!(0x2312b0, |c, _, w| {
        w.log.push(vec![32]);
        let v = w.next() % 2;
        c.set_gpr(2, v as u64);
    });
    stub!(0x230ee0, |c, _, w| w.log.push(vec![33, c.gpr(5) as u32]));
    stub!(0x2310c8, |c, _, w| w.log.push(vec![34, c.gpr(5) as u32, c.gpr(6) as u32]));
    stub!(0x231318, |_, _, w| w.log.push(vec![9]));
    stub!(0x20c3e8, |_, _, w| w.log.push(vec![6]));
    stub!(0x1814a8, |c, _, _| {
        assert_eq!(c.gpr(4) as u32, racepl);
        c.set_gpr(2, (RIDER + STRIDE * c.gpr(5) as u32) as u64);
    });
    stub!(0x15ee30, |c, _, w| {
        let a = c.gpr(4) as u32 - 0x46c0 - RIDER;
        assert_eq!(a % STRIDE, 0);
        w.log.push(vec![10, a / STRIDE]);
    });
    r.stub(0x2fd14c, move |c, m| {
        let (d, s) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let mut i = 0;
        loop {
            let b = (m.read_u32((s + i) & !3) >> (8 * ((s + i) & 3))) as u8;
            let wd = m.read_u32((d + i) & !3);
            let sh = 8 * ((d + i) & 3);
            m.write_u32((d + i) & !3, (wd & !(0xff << sh)) | (b as u32) << sh);
            if b == 0 {
                break;
            }
            i += 1;
        }
        c.set_gpr(2, d as u64);
    });
    r.stub(0x2fd008, move |c, m| {
        let (a, b) = (cstr(m, c.gpr(4) as u32), cstr(m, c.gpr(5) as u32));
        c.set_gpr(2, if a == b { 0 } else { 1 });
    });
    // sprintf(buf, "CameraScript_%d", n)
    r.stub(0x2fca28, move |c, m| {
        assert_eq!(cstr(m, c.gpr(5) as u32), b"CameraScript_%d");
        let s = format!("CameraScript_{}", c.gpr(6) as u32 as i32);
        let d = c.gpr(4) as u32;
        for (k, b) in s.bytes().chain(std::iter::once(0)).enumerate() {
            let a = d + k as u32;
            let wd = m.read_u32(a & !3);
            let sh = 8 * (a & 3);
            m.write_u32(a & !3, (wd & !(0xff << sh)) | (b as u32) << sh);
        }
    });
    let l = w.clone();
    r.stub(0x13bad8, move |c, m| {
        let mut v = vec![35];
        v.extend(cstr(m, c.gpr(4) as u32).iter().map(|b| *b as u32));
        l.borrow_mut().log.push(v);
    });

    let cams: [&[u8]; 5] = [b"chase near", b"chase far", b"intro 1", b"", b"fifteen chars x"];
    let mut g = Rng(0x175f_50a1_c4d3_9b27);
    let mut kinds = [0u32; 40];
    let (mut ran, mut returned, mut pops, mut pushes) = (0, 0, 0u32, 0u32);
    for case in 0..20_000 {
        // the scripts
        for si in 0..4 {
            let count = 1 + g.next() as usize % 8;
            let mut steps = vec![];
            for _ in 0..count {
                let start = (g.next() % 400) as f32 / 100.0;
                let kind = match g.next() % 4 {
                    0 => g.next() % 0x30,
                    1 => [0u32, 1, 2, 3, 4, 7, 10, 11, 12, 13, 31, 33][g.next() as usize % 12],
                    _ => g.next() % 0x2d,
                };
                steps.push(Step {
                    name: if kind == 10 || kind == 11 { names[g.next() as usize % 4].to_vec() } else { cams[g.next() as usize % 5].to_vec() },
                    kind,
                    a: g.next() % 4,
                    b: (g.next() % 3000) as f32 / 10.0,
                    c: (g.next() % 161) as f32 - 80.0,
                    start,
                    end: start + (g.next() % 300) as f32 / 100.0,
                    w28: [0u32, 1, 7, 0xffff_fff9, 100][g.next() as usize % 5],
                });
            }
            let mut wb = w.borrow_mut();
            let addr = wb.scripts[si].addr;
            for (i, st) in steps.iter().enumerate() {
                let a = addr + 0x2c * i as u32;
                for k in 0..4 {
                    r.mem.write_u32(a + 4 * k, 0);
                }
                for (k, b) in st.name.iter().enumerate() {
                    let wd = r.mem.read_u32((a + k as u32) & !3);
                    r.mem.write_u32((a + k as u32) & !3, wd | (*b as u32) << (8 * ((a + k as u32) & 3)));
                }
                r.mem.write_u32(a + 0x10, st.kind);
                r.mem.write_u32(a + 0x14, st.a);
                r.mem.write_f32(a + 0x18, st.b);
                r.mem.write_f32(a + 0x1c, st.c);
                r.mem.write_f32(a + 0x20, st.start);
                r.mem.write_f32(a + 0x24, st.end);
                r.mem.write_u32(a + 0x28, st.w28);
            }
            r.mem.write_u32(addr + 0x370, steps.len() as u32);
            wb.scripts[si].steps = steps;
        }
        let mut o: Vec<u32> = (0..OBJ_WORDS).map(|_| g.next() % 4).collect();
        let cur = cams[g.next() as usize % 5];
        copy_name(&mut o, 0x24, cur);
        let sname: &[u8] = if g.next() % 12 == 0 { b"s9" } else { names[g.next() as usize % 4] };
        copy_name(&mut o, 0x50, sname);
        o[0x34 / 4] = [1u32, 1, 1, 1, 1, 0, 2][g.next() as usize % 7];
        o[0x1e4 / 4] = [0u32, 0, 0, 0, 1, 2][g.next() as usize % 6];
        o[0xc / 4] = [3u32, 0, 1, 2][g.next() as usize % 4];
        o[0x44 / 4] = g.next() % 2;
        o[0x48 / 4] = [0u32, 0, 1, 2, 5, 15, 16][g.next() as usize % 7];
        o[0x428 / 4] = [4u32, 4, 3, 0][g.next() as usize % 4];
        o[0x210 / 4] = g.next() % 2;
        for i in 0..8 {
            o[(0x230 + 0x40 * i) / 4] = g.next() % 3;
        }
        o[0x4e8 / 4] = [0u32, listp, listp][g.next() as usize % 3];
        o[ELAPSED_W] = [0.0f32, 0.0, 0.5, 1.5, 2.9, 3.5, 5.0, 7.0][g.next() as usize % 8].to_bits();
        let nsteps = w.borrow().scripts.iter().find(|s| s.name == sname).map_or(1, |s| s.steps.len());
        o[0x60 / 4] = g.next() % nsteps as u32;
        for d in 0..16 {
            let at = 0x64 / 4 + d * 6;
            o[at] = [0.0f32, 1.0, 2.5, 6.0][g.next() as usize % 4].to_bits();
            copy_name(&mut o, 4 * (at + 1), names[g.next() as usize % 4]);
            let n = w.borrow().scripts.iter().find(|s| s.name == names[0]).map_or(1, |s| s.steps.len());
            let _ = n;
            o[at + 5] = g.next() % 9;
        }
        let ec = [0i32, 1, 2, 3][g.next() as usize % 4];
        r.mem.write_u32(0x32db6c, ec as u32);
        let lf = g.next() % 2;
        r.mem.write_u32(listp + 0x114, lf);
        for (i, wd) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *wd);
        }
        for k in 0..3 {
            r.mem.write_u32(0x3365f8 + 4 * k, 0xdead_0000 + k);
        }
        r.mem.write_u32(racepl + 0x4c, 0xbeef);
        {
            let mut wb = w.borrow_mut();
            wb.oracle = (0..64).map(|_| g.next()).collect();
            wb.pos = 0;
            wb.log.clear();
            wb.entry_count = ec;
            wb.list_flag = lf;
            wb.statics = [0xdead_0000, 0xdead_0001, 0xdead_0002];
            wb.race_flag = 0xbeef;
        }
        let depth0 = o[0x48 / 4];
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x175f50).unwrap();
        let want = std::mem::take(&mut w.borrow_mut().log);
        w.borrow_mut().pos = 0;
        let mut e = U(w.clone());
        update(&mut o, &mut e);
        let got = std::mem::take(&mut w.borrow_mut().log);
        assert_eq!(want, got, "case {case} calls");
        for (i, wd) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *wd, "case {case} word {:#x}", i * 4);
        }
        let st = w.borrow().statics;
        for k in 0..3 {
            assert_eq!(r.mem.read_u32(0x3365f8 + 4 * k as u32), st[k], "case {case} static {k}");
        }
        assert_eq!(r.mem.read_u32(racepl + 0x4c), w.borrow().race_flag, "case {case} race flag");
        for g in &got {
            kinds[g[0] as usize] += 1;
        }
        pops += (o[0x48 / 4] < depth0) as u32;
        pushes += (o[0x48 / 4] > depth0) as u32;
        if got.is_empty() {
            returned += 1;
        } else {
            ran += 1;
        }
    }
    eprintln!("update: {ran} ticks with calls, {returned} without, {pops} pops, {pushes} pushes; calls by tag {kinds:?}");
}

const ELAPSED_W: usize = 0x4c / 4;
