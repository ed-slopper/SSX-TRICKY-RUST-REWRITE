//! `tricky_game::camscript::cut_to` against `CamScript_CutTo` 0x174c98 with `Camera_GotoNamed` 0x174888 and 0x174c58
//! in the function runner (TRICKY_ELF; skipped without it): the camera class, the position and angle evaluations,
//! the smoothing and the cut fade stubbed and logged.

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
    fn f(&mut self) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => 1.0,
            2 => (self.next() % 1000) as f32 / 1000.0,
            _ => (self.next() % 20_000) as f32 / 10.0 - 1000.0,
        }
    }
}

struct Class {
    name: Vec<u8>,
    addr: u32,
    w170: u32,
    f168: f32,
}

#[derive(Default)]
struct W {
    classes: Vec<Class>,
    oracle: Vec<u32>,
    pos: usize,
    log: Vec<Vec<u32>>,
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
    fn smooth(&mut self, obj: &mut [u32]) {
        let mut v = vec![4];
        v.extend_from_slice(&obj[0x484 / 4..0x484 / 4 + 8]);
        v.extend_from_slice(&[obj[1], obj[2], obj[0x1e4 / 4]]);
        self.0.borrow_mut().log.push(v);
    }
    fn cut_fade(&mut self, obj: &mut [u32]) {
        self.0.borrow_mut().log.push(vec![5, obj[1], obj[0x448 / 4]]);
    }
    fn audio_cut(&mut self) {
        unreachable!()
    }
    fn find_owner(&mut self) -> u32 {
        unreachable!()
    }
    fn first_position(&mut self, _: u32) -> [f32; 3] {
        unreachable!()
    }
    fn audio_stop(&mut self) {
        unreachable!()
    }
    fn entry_count(&mut self) -> i32 {
        unreachable!()
    }
    fn reset_rider_anim(&mut self, _: i32) {
        unreachable!()
    }
}
impl CutEnv for U {
    fn class_open(&mut self, name: &[u8]) -> u32 {
        let mut w = self.0.borrow_mut();
        let a = w.classes.iter().find(|c| c.name == name).unwrap().addr;
        w.log.push(vec![20, a]);
        a
    }
    fn class_close(&mut self, cls: u32) {
        self.0.borrow_mut().log.push(vec![21, cls]);
    }
    fn class_word_170(&mut self, cls: u32) -> u32 {
        self.0.borrow().classes.iter().find(|c| c.addr == cls).unwrap().w170
    }
    fn class_f168(&mut self, cls: u32) -> f32 {
        self.0.borrow().classes.iter().find(|c| c.addr == cls).unwrap().f168
    }
    fn eval_pos(&mut self, obj: &mut [u32], flag: u32) -> u32 {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![40, 0x60_0000, 0x484, 0xb68, flag, 0]);
        for k in 0..3 {
            obj[0x484 / 4 + k] = w.next();
        }
        w.next()
    }
    fn eval_ang(&mut self, obj: &mut [u32], flag: u32, eac: u32, t2: u32) {
        let mut w = self.0.borrow_mut();
        w.log.push(vec![41, 0x60_0100, 0x484, 0xb68, flag, 0x10, eac, t2, 0]);
        for k in 3..7 {
            obj[0x484 / 4 + k] = w.next();
        }
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
fn camera_cut_to_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x174c98u32, 0x174888, 0x174c58] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0xf00);
    let names: [&[u8]; 4] = [b"c0", b"chase near", b"intro cam 1", b"x"];
    let strs = r.mem.alloc(0x100);
    let w = Rc::new(RefCell::new(W::default()));
    for (k, n) in names.iter().enumerate() {
        let addr = r.mem.alloc(0x200);
        w.borrow_mut().classes.push(Class { name: n.to_vec(), addr, w170: 0, f168: 0.0 });
        let a = strs + 0x20 * k as u32;
        for i in 0..8 {
            r.mem.write_u32(a + 4 * i, 0);
        }
        for (i, b) in n.iter().enumerate() {
            let wd = r.mem.read_u32((a + i as u32) & !3);
            r.mem.write_u32((a + i as u32) & !3, wd | (*b as u32) << (8 * ((a + i as u32) & 3)));
        }
    }
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
    let l = w.clone();
    r.stub(0x16dd68, move |c, m| {
        let n = cstr(m, c.gpr(5) as u32);
        let mut w = l.borrow_mut();
        let a = w.classes.iter().find(|k| k.name == n).unwrap().addr;
        m.write_u32(c.gpr(4) as u32 + 4, a);
        w.log.push(vec![20, a]);
    });
    let l = w.clone();
    r.stub(0x16ded0, move |c, m| {
        let a = m.read_u32(c.gpr(4) as u32 + 4);
        l.borrow_mut().log.push(vec![21, a]);
    });
    r.stub(0x16dda8, move |c, _| c.set_gpr(2, 0x60_0000));
    r.stub(0x16ddb8, move |c, _| c.set_gpr(2, 0x60_0100));
    let l = w.clone();
    r.stub(0x16e900, move |c, m| {
        let mut w = l.borrow_mut();
        let (a1, a2) = (c.gpr(5) as u32, c.gpr(6) as u32);
        w.log.push(vec![40, c.gpr(4) as u32, a1 - obj, a2 - obj, c.gpr(7) as u32, c.gpr(8) as u32]);
        for k in 0..3 {
            let v = w.next();
            m.write_u32(a1 + 4 * k, v);
        }
        let v = w.next();
        c.set_gpr(2, v as u64);
    });
    let l = w.clone();
    r.stub(0x1722e8, move |c, m| {
        let mut w = l.borrow_mut();
        let (a1, a2) = (c.gpr(5) as u32, c.gpr(6) as u32);
        w.log.push(vec![41, c.gpr(4) as u32, a1 - obj, a2 - obj, c.gpr(7) as u32, c.gpr(8) as u32 - obj, c.gpr(9) as u32, c.gpr(10) as u32, c.f(12).to_bits()]);
        for k in 3..7 {
            let v = w.next();
            m.write_u32(a1 + 4 * k, v);
        }
    });
    let l = w.clone();
    r.stub(0x174238, move |_, m| {
        let mut v = vec![4];
        v.extend((0..8).map(|i| m.read_u32(obj + 0x484 + 4 * i)));
        v.extend_from_slice(&[m.read_u32(obj + 4), m.read_u32(obj + 8), m.read_u32(obj + 0x1e4)]);
        l.borrow_mut().log.push(v);
    });
    let l = w.clone();
    r.stub(0x174b38, move |c, m| {
        assert_eq!(c.gpr(4) as u32, obj);
        l.borrow_mut().log.push(vec![5, m.read_u32(obj + 4), m.read_u32(obj + 0x448)]);
    });

    let mut g = Rng(0x174c_98a1_c4d3_0b27);
    let (mut early, mut cut, mut blend) = (0, 0, 0);
    for case in 0..30_000 {
        for k in 0..4 {
            let (w170, f168) = ([0u32, 1, 1][g.next() as usize % 3], g.f());
            let mut wb = w.borrow_mut();
            wb.classes[k].w170 = w170;
            wb.classes[k].f168 = f168;
            let a = wb.classes[k].addr;
            r.mem.write_u32(a + 0x170, w170);
            r.mem.write_f32(a + 0x168, f168);
        }
        let mut o: Vec<u32> = (0..0xf00 / 4).map(|_| g.next()).collect();
        for k in 0..22 {
            o[0x484 / 4 + k] = g.f().to_bits();
        }
        o[1] = g.next() % 2;
        o[7] = [0u32, 0, 1][g.next() as usize % 3];
        o[8] = [0.0f32, 0.25, 0.5, 1.0, 0.75][g.next() as usize % 5].to_bits();
        o[3] = [0u32, 1, 2, 3, 4, 7, 0xffff_ffff][g.next() as usize % 7];
        o[0x428 / 4] = g.next() % 9;
        let ni = g.next() as usize % 4;
        let current = g.next() % 3 == 0;
        for (i, wd) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *wd);
        }
        {
            let mut wb = w.borrow_mut();
            wb.oracle = (0..16).map(|_| g.f().to_bits()).collect();
            wb.pos = 0;
            wb.log.clear();
        }
        // the name: a string of its own, or the object's current camera name
        let ptr = if current {
            copy_name(&mut o, 0x24, names[ni]);
            for i in 0..4 {
                r.mem.write_u32(obj + 0x24 + 4 * i, o[0x24 / 4 + i as usize]);
            }
            obj + 0x24
        } else {
            strs + 0x20 * ni as u32
        };
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, ptr as u64);
        r.call(0x174c98).unwrap();
        let want = std::mem::take(&mut w.borrow_mut().log);
        w.borrow_mut().pos = 0;
        let mut e = U(w.clone());
        cut_to(&mut o, names[ni], &mut e);
        let got = std::mem::take(&mut w.borrow_mut().log);
        assert_eq!(want, got, "case {case} calls");
        for (i, wd) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *wd, "case {case} word {:#x}", i * 4);
        }
        early += (o[1] == 0 && o[7] != 0) as u32;
        cut += o[1] as u32;
        blend += (o[1] == 0 && o[7] == 0) as u32;
    }
    eprintln!("cut_to: {cut} cut, {early} early, {blend} blended");
}
