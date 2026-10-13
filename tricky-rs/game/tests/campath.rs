//! `tricky_game::campath` against `CamPath_SegmentPoint` 0x170610, `CamPath_SegmentAngles` 0x1708e8 and
//! `Camera_UpdateCutFade` 0x174b38 (with `CamScript_UpdateCutFade` 0x174c58) in the function runner (TRICKY_ELF;
//! skipped without it).

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::campath::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 8 {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 100.0 - 100.0,
        }
    }
    fn ang(&mut self) -> f32 {
        (self.next() % 20_000) as f32 / 1000.0 - 10.0
    }
}

fn key(g: &mut Rng) -> Key {
    Key { pos: [g.f(), g.f(), g.f()], ang: [g.ang(), g.ang(), g.ang()], w: g.f() }
}

fn put_key(r: &mut Runner, a: u32, k: &Key) {
    for i in 0..3 {
        r.mem.write_f32(a + 4 * i, k.pos[i as usize]);
        r.mem.write_f32(a + 0xc + 4 * i, k.ang[i as usize]);
    }
    r.mem.write_f32(a + 0x18, k.w);
}

#[test]
fn camera_path_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x137140);
    r.allow(0x1741d0);
    r.allow(0x1704b0);
    r.allow(0x170610);
    r.allow(0x174b38);
    let out = r.mem.alloc(0x100);
    let path = r.mem.alloc(0x40);
    let owner = r.mem.alloc(0x200);
    let keys = [r.mem.alloc(0x20), r.mem.alloc(0x20), r.mem.alloc(0x20), r.mem.alloc(0x20)];
    let rr = r.mem.alloc(0x10);
    r.mem.write_u32(path + 0x24, owner);
    let mut g = Rng(0x1706_10a9_0c3d_5e11);
    let (mut firsts, mut lasts, mut mids) = (0, 0, 0);
    for case in 0..30_000 {
        let count = 2 + (g.next() % 8) as i32;
        let idx = (g.next() % (count as u32 + 1)) as i32;
        let at = PathAt { idx, count };
        let ks = [key(&mut g), key(&mut g), key(&mut g), key(&mut g)];
        for (a, k) in keys.iter().zip(ks.iter()) {
            put_key(&mut r, *a, k);
        }
        r.mem.write_u32(path + 4, idx as u32);
        r.mem.write_u32(owner + 0x12c, count as u32);
        let t = [0.0f32, 0.25, 0.5, 1.0, (g.next() % 1000) as f32 / 1000.0][(g.next() % 5) as usize];
        if idx == 0 {
            firsts += 1;
        } else if idx >= count - 2 {
            lasts += 1;
        } else {
            mids += 1;
        }
        // SegmentPoint
        for k in 0..3 {
            r.mem.write_u32(out + 4 * k, 0xdead_beef);
        }
        r.mem.write_u32(rr, 0x7777_7777);
        r.mem.write_u32(rr + 4, 0x7777_7777);
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, path as u64);
        r.cpu.set_gpr(6, keys[0] as u64);
        r.cpu.set_gpr(7, keys[1] as u64);
        r.cpu.set_gpr(8, keys[2] as u64);
        r.cpu.set_gpr(9, keys[3] as u64);
        r.cpu.set_gpr(10, rr as u64);
        r.cpu.set_gpr(11, (rr + 4) as u64);
        r.cpu.set_f(12, t);
        r.call(0x170610).unwrap();
        let (p, ratios) = segment_point(at, t, &ks[0], &ks[1], &ks[2], &ks[3]);
        for k in 0..3 {
            assert_eq!(r.mem.read_u32(out + 4 * k), p[k as usize].to_bits(), "case {case} point {k}");
        }
        let want0 = ratios.r0.map(f32::to_bits).unwrap_or(0x7777_7777);
        let want1 = ratios.r1.map(f32::to_bits).unwrap_or(0x7777_7777);
        assert_eq!(r.mem.read_u32(rr), want0, "case {case} r0");
        assert_eq!(r.mem.read_u32(rr + 4), want1, "case {case} r1");
        // SegmentAngles
        for k in 0..0x16 {
            r.mem.write_u32(out + 4 * k, 0xdead_beef);
        }
        r.cpu.set_gpr(4, out as u64);
        r.cpu.set_gpr(5, path as u64);
        r.cpu.set_gpr(6, keys[0] as u64);
        r.cpu.set_gpr(7, keys[1] as u64);
        r.cpu.set_gpr(8, keys[2] as u64);
        r.cpu.set_gpr(9, keys[3] as u64);
        r.cpu.set_f(12, t);
        r.call(0x1708e8).unwrap();
        let a = segment_angles(at, t, &ks[0], &ks[1], &ks[2], &ks[3]);
        let want = [a.pos[0], a.pos[1], a.pos[2], a.ang[0], a.ang[1], a.ang[2], a.w, 100.0];
        for (k, w) in want.iter().enumerate() {
            assert_eq!(r.mem.read_u32(out + 4 * k as u32), w.to_bits(), "case {case} angles word {k} at {at:?} t {t} keys {ks:?}");
        }
        for k in [8u32, 12, 21] {
            assert_eq!(r.mem.read_u32(out + 4 * k), 0, "case {case} zero word {k}");
        }
    }
    eprintln!("first {firsts} last {lasts} mid {mids}");
    // the cut fade
    let this = r.mem.alloc(0x500);
    let other = r.mem.alloc(0x20);
    let view = r.mem.alloc(0x200);
    r.mem.write_u32(other + 4, view);
    for case in 0..20_000 {
        let mut f = CutFade {
            hold: [0, 1, 2, -1][(g.next() % 4) as usize],
            active: (g.next() % 2) as u32,
            t: [0.0f32, 0.1, 0.3, 0.59, 0.5999, 0.6, 0.7][(g.next() % 7) as usize],
        };
        r.mem.write_u32(0x3365f4, f.hold as u32);
        r.mem.write_u32(0x3365f8, f.active);
        r.mem.write_f32(0x3365fc, f.t);
        r.mem.write_u32(this + 0x44c, 0x5555_5555);
        r.cpu.set_gpr(4, this as u64);
        if case % 2 == 0 {
            r.call(0x174b38).unwrap();
        } else {
            let v = g.f();
            r.mem.write_f32(view + 0x168, v);
            r.mem.write_u32(this + 0x448, 0);
            r.cpu.set_gpr(5, other as u64);
            r.call(0x174c58).unwrap();
            assert_eq!(r.mem.read_u32(this + 0x448), v.to_bits(), "case {case} copied");
        }
        let level = update_cut_fade(&mut f);
        assert_eq!(r.mem.read_u32(this + 0x44c), level.to_bits(), "case {case} level");
        assert_eq!(r.mem.read_u32(0x3365f4), f.hold as u32, "case {case} hold");
        assert_eq!(r.mem.read_u32(0x3365f8), f.active, "case {case} active");
        assert_eq!(r.mem.read_u32(0x3365fc), f.t.to_bits(), "case {case} t");
    }
}

struct Table {
    keys: Vec<Key>,
    owner: u32,
    cur: i32,
    log: Vec<i32>,
}

#[test]
fn camera_path_eval_matches_the_original() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x137140u32, 0x1741d0, 0x1704b0, 0x170610, 0x1708e8, 0x170ca8] {
        r.allow(a);
    }
    let out = r.mem.alloc(0x100);
    let path = r.mem.alloc(0x40);
    let owner = r.mem.alloc(0x400);
    let flag = r.mem.alloc(0x10);
    let rider = r.mem.alloc(0x10);
    let tab = Rc::new(RefCell::new(Table { keys: vec![], owner, cur: -1, log: vec![] }));
    let t = tab.clone();
    r.stub(0x16dd68, move |c, _| {
        let mut t = t.borrow_mut();
        let off = c.gpr(5) as u32 - t.owner;
        assert_eq!(off % 15, 0, "key pointer");
        t.cur = (off / 15) as i32;
        let k = t.cur;
        t.log.push(k);
        c.set_gpr(2, c.gpr(4));
    });
    r.stub(0x16dda8, move |c, _| c.set_gpr(2, 0x60_0000));
    r.stub(0x16ddb8, move |c, _| c.set_gpr(2, 0x60_0100));
    let t = tab.clone();
    r.stub(0x16e900, move |c, m| {
        let t = t.borrow();
        let k = &t.keys[t.cur as usize];
        for i in 0..3u32 {
            m.write_f32(c.gpr(5) as u32 + 4 * i, k.pos[i as usize]);
        }
    });
    let t = tab.clone();
    r.stub(0x1722e8, move |c, m| {
        let t = t.borrow();
        let k = &t.keys[t.cur as usize];
        for i in 0..3u32 {
            m.write_f32(c.gpr(5) as u32 + 0xc + 4 * i, k.ang[i as usize]);
        }
        m.write_f32(c.gpr(5) as u32 + 0x18, k.w);
    });
    r.stub(0x16ded0, move |_, _| {});
    struct E {
        t: Rc<RefCell<Table>>,
    }
    impl KeyEnv for E {
        fn key(&mut self, i: i32) -> Key {
            let mut t = self.t.borrow_mut();
            t.log.push(-1000 - i);
            t.keys[i as usize]
        }
    }
    let mut g = Rng(0x170c_a8e5_71b2_9d03);
    let (mut ticks, mut ended, mut moved, mut eased) = (0, 0, 0, 0);
    for seq in 0..400 {
        let count = [1i32, 2, 2, 3, 3, 4, 6, 8][(g.next() % 8) as usize];
        let keys: Vec<Key> = (0..count)
            .map(|i| Key {
                pos: [i as f32 * 300.0 + g.f() * 2.0, g.f() * 3.0, g.f() * 3.0],
                ang: [g.ang() * 0.3, g.ang() * 0.3, g.ang() * 0.3],
                w: (g.next() % 1500) as f32 / 1000.0 + 0.5,
            })
            .collect();
        tab.borrow_mut().keys = keys.clone();
        let own = Owner { count, speed: [600.0f32, 1500.0, 3000.0][(g.next() % 3) as usize] };
        r.mem.write_u32(owner + 0x12c, count as u32);
        r.mem.write_f32(owner + 0x130, own.speed);
        r.mem.write_u32(path + 0x24, owner);
        let mut p = PathState {
            idx: 0,
            t: 0.0,
            dt: 0.05,
            last: [0.0; 3],
            speed: [0.0f32, 0.0, 400.0, 1200.0, 6000.0][(g.next() % 5) as usize],
            mode: [0i32, 1, 2, 3, 4, -1][(g.next() % 6) as usize],
        };
        let mut done = false;
        for tick in 0..2000 {
            if done {
                break;
            }
            r.mem.write_u32(path + 4, p.idx as u32);
            r.mem.write_f32(path + 8, p.t);
            r.mem.write_f32(path + 0xc, p.dt);
            for k in 0..3 {
                r.mem.write_f32(path + 0x10 + 4 * k, p.last[k as usize]);
            }
            r.mem.write_f32(path + 0x1c, p.speed);
            r.mem.write_u32(path + 0x20, p.mode as u32);
            for k in 0..0x16 {
                r.mem.write_u32(out + 4 * k, 0xdead_beef);
            }
            r.mem.write_u32(flag, 0x5555_5555);
            tab.borrow_mut().log.clear();
            r.cpu.set_gpr(4, out as u64);
            r.cpu.set_gpr(5, path as u64);
            r.cpu.set_gpr(6, rider as u64);
            r.cpu.set_gpr(7, flag as u64);
            r.call(0x170ca8).unwrap();
            let want_log: Vec<i32> = std::mem::take(&mut tab.borrow_mut().log);
            let before = p;
            let (a, end) = eval(&mut p, &own, &mut E { t: tab.clone() });
            let got_log: Vec<i32> = tab.borrow().log.iter().map(|v| if *v <= -1000 { -1000 - *v } else { *v }).collect();
            let want_keys: Vec<i32> = want_log.iter().filter(|v| **v >= 0).cloned().collect();
            let got_keys: Vec<i32> = got_log;
            assert_eq!(want_keys, got_keys, "seq {seq} tick {tick} key order");
            let want = [a.pos[0], a.pos[1], a.pos[2], a.ang[0], a.ang[1], a.ang[2], a.w, 100.0];
            for (k, w) in want.iter().enumerate() {
                assert_eq!(r.mem.read_u32(out + 4 * k as u32), w.to_bits(), "seq {seq} tick {tick} out word {k} before {before:?} after {p:?} count {count}");
            }
            for k in [8u32, 12, 21] {
                assert_eq!(r.mem.read_u32(out + 4 * k), 0, "seq {seq} tick {tick} zero word {k}");
            }
            assert_eq!(r.mem.read_u32(flag), end as u32, "seq {seq} tick {tick} flag");
            assert_eq!(r.mem.read_u32(path + 4), p.idx as u32, "seq {seq} tick {tick} idx");
            assert_eq!(r.mem.read_u32(path + 8), p.t.to_bits(), "seq {seq} tick {tick} t");
            assert_eq!(r.mem.read_u32(path + 0xc), p.dt.to_bits(), "seq {seq} tick {tick} dt");
            for k in 0..3u32 {
                assert_eq!(r.mem.read_u32(path + 0x10 + 4 * k), p.last[k as usize].to_bits(), "seq {seq} tick {tick} last {k}");
            }
            ticks += 1;
            if p.idx != before.idx {
                moved += 1;
            }
            if p.mode != 0 && (p.idx == 0 || p.idx >= count - 2) {
                eased += 1;
            }
            if end {
                ended += 1;
                done = true;
            }
        }
    }
    eprintln!("ticks {ticks} ended {ended} moved {moved} eased {eased}");
    assert!(ended > 100 && moved > 500);
}

#[test]
fn camera_blend_linear_matches_the_original() {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x1741d0u32, 0x174d28] {
        r.allow(a);
    }
    const FROM: u32 = 0x3cda78;
    const TO: u32 = 0x3cdad0;
    let obj = r.mem.alloc(0xf00);
    let q: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let log: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let cams: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(vec![]));
    let slot_of = |cams: &Vec<u32>, p: u32| cams.iter().position(|c| *c == p).expect("camera") as u32;
    let (c, l) = (cams.clone(), log.clone());
    r.stub(0x16dd68, move |cpu, _| {
        let mut c = c.borrow_mut();
        let s = c.len() as u32;
        c.push(cpu.gpr(4) as u32);
        l.borrow_mut().push(vec![1, s, cpu.gpr(5) as u32]);
        cpu.set_gpr(2, cpu.gpr(4));
    });
    r.stub(0x16dda8, move |cpu, _| cpu.set_gpr(2, cpu.gpr(4)));
    r.stub(0x16ddb8, move |cpu, _| cpu.set_gpr(2, cpu.gpr(4)));
    let (c, l, qq) = (cams.clone(), log.clone(), q.clone());
    r.stub(0x16e900, move |cpu, m| {
        let s = slot_of(&c.borrow(), cpu.gpr(4) as u32);
        assert_eq!(cpu.gpr(5) as u32, if s == 0 { FROM } else { TO });
        assert_eq!(cpu.gpr(6) as u32, obj + 0xb68);
        assert_eq!(cpu.gpr(8) as u32 & 0xf, (c.borrow()[0] + 0x50) & 0xf);
        l.borrow_mut().push(vec![2, s, cpu.gpr(7) as u32]);
        let mut q = qq.borrow_mut();
        for i in 0..3 {
            m.write_u32(cpu.gpr(5) as u32 + 4 * i, q.pop_front().unwrap());
        }
        m.write_u32(cpu.gpr(8) as u32, q.pop_front().unwrap());
    });
    let (c, l, qq) = (cams.clone(), log.clone(), q.clone());
    r.stub(0x1722e8, move |cpu, m| {
        let s = slot_of(&c.borrow(), cpu.gpr(4) as u32);
        assert_eq!(cpu.gpr(5) as u32, if s == 0 { FROM } else { TO });
        assert_eq!(cpu.gpr(8) as u32, obj + 0x10);
        assert_eq!(cpu.gpr(9) as u32, m.read_u32(obj + 0xeac));
        assert_eq!(cpu.gpr(10), 0);
        l.borrow_mut().push(vec![3, s, cpu.gpr(7) as u32, cpu.f(12).to_bits()]);
        let mut q = qq.borrow_mut();
        for i in 3..7 {
            m.write_u32(cpu.gpr(5) as u32 + 4 * i, q.pop_front().unwrap());
        }
    });
    let (l, c) = (log.clone(), cams.clone());
    r.stub(0x174238, move |_, m| {
        let _ = &c;
        let mut v = vec![4];
        v.extend((0..22).map(|i| m.read_u32(obj + 0x484 + 4 * i)));
        v.push(m.read_u32(obj + 0x18));
        l.borrow_mut().push(v);
    });
    let (l, c) = (log.clone(), cams.clone());
    r.stub(0x174c58, move |cpu, m| {
        let s = slot_of(&c.borrow(), cpu.gpr(5) as u32);
        let mut v = vec![5, s];
        v.extend((0..22).map(|i| m.read_u32(obj + 0x484 + 4 * i)));
        v.push(m.read_u32(obj + 0x18));
        l.borrow_mut().push(v);
    });
    let (l, c) = (log.clone(), cams.clone());
    r.stub(0x16ded0, move |cpu, _| {
        let s = slot_of(&c.borrow(), cpu.gpr(4) as u32);
        l.borrow_mut().push(vec![6, s, cpu.gpr(5) as u32]);
    });

    struct E {
        q: Rc<RefCell<VecDeque<u32>>>,
        log: Rc<RefCell<Vec<Vec<u32>>>>,
        arg: [u32; 2],
    }
    impl BlendEnv for E {
        fn build(&mut self, slot: u32, arg: u32) {
            assert_eq!(arg, self.arg[slot as usize]);
            self.log.borrow_mut().push(vec![1, slot, arg]);
        }
        fn eval(&mut self, slot: u32, rec: &mut [u32; 22], _: &[u32], flag: u32, out: &mut f32) {
            self.log.borrow_mut().push(vec![2, slot, flag]);
            let mut q = self.q.borrow_mut();
            for i in 0..3 {
                rec[i] = q.pop_front().unwrap();
            }
            *out = f32::from_bits(q.pop_front().unwrap());
            self.log.borrow_mut().push(vec![3, slot, flag, out.to_bits()]);
            for i in 3..7 {
                rec[i] = q.pop_front().unwrap();
            }
        }
        fn smooth(&mut self, o: &mut [u32]) {
            let mut v = vec![4];
            v.extend_from_slice(&o[0x484 / 4..0x484 / 4 + 22]);
            v.push(o[0x18 / 4]);
            self.log.borrow_mut().push(v);
        }
        fn hand_over(&mut self, slot: u32, o: &mut [u32]) {
            let mut v = vec![5, slot];
            v.extend_from_slice(&o[0x484 / 4..0x484 / 4 + 22]);
            v.push(o[0x18 / 4]);
            self.log.borrow_mut().push(v);
        }
        fn done(&mut self, slot: u32) {
            self.log.borrow_mut().push(vec![6, slot, 2]);
        }
    }

    let mut g = Rng(0x174d_28b1_3f5a_c3e7);
    let (mut n, mut full, mut part, mut first) = (0, 0, 0, 0);
    for case in 0..20_000 {
        let mut o: Vec<u32> = (0..0x3c0).map(|_| g.next()).collect();
        for i in 0..22 {
            let v = if i < 3 { (g.f() * 2000.0) as f32 } else if i < 6 { g.ang() * 1.5 } else if i == 6 { (g.next() % 1500) as f32 / 1000.0 + 0.4 } else { f32::from_bits(g.next()) };
            o[0x484 / 4 + i] = v.to_bits();
        }
        o[0x18 / 4] = g.next() % 3;
        let a0 = g.next();
        let a1 = g.next();
        let t = match g.next() % 9 {
            0 => 0.0,
            1 => -(g.f().abs()),
            2 => 1.0,
            3 => 1.0 + g.f().abs(),
            4 => 1e-30,
            _ => (g.next() % 100_000) as f32 / 100_000.0,
        };
        let mut st = BlendState {
            from: std::array::from_fn(|_| g.next()),
            to: std::array::from_fn(|_| g.next()),
            from_set: g.next() % 3 != 0,
            to_set: g.next() % 3 != 0,
        };
        if st.from_set {
            for i in 0..8 {
                st.from[i] = [g.f(), g.f(), g.f(), g.ang(), g.ang(), g.ang(), g.f(), g.f()][i].to_bits();
            }
        }
        if st.to_set {
            for i in 0..8 {
                st.to[i] = [g.f(), g.f(), g.f(), g.ang(), g.ang(), g.ang(), g.f(), g.f()][i].to_bits();
            }
        }
        let words: Vec<u32> = (0..16)
            .map(|i| {
                let k = i % 8;
                (if k < 3 { g.f() * 1500.0 } else if k == 3 { g.f() } else if k < 7 { g.ang() * 1.5 } else { g.f() }).to_bits()
            })
            .collect();
        for (i, w) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *w);
        }
        for i in 0..22 {
            r.mem.write_u32(FROM + 4 * i, st.from[i as usize]);
            r.mem.write_u32(TO + 4 * i, st.to[i as usize]);
        }
        r.mem.write_u32(0x336648, st.from_set as u32);
        r.mem.write_u32(0x33664c, st.to_set as u32);
        *q.borrow_mut() = words.iter().cloned().collect();
        log.borrow_mut().clear();
        cams.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, a0 as u64);
        r.cpu.set_gpr(6, a1 as u64);
        r.cpu.set_f(12, t);
        r.call(0x174d28).unwrap();
        let want = std::mem::take(&mut *log.borrow_mut());
        let want_ret = r.cpu.gpr(2) & 1;

        *q.borrow_mut() = words.iter().cloned().collect();
        let mut e = E { q: q.clone(), log: log.clone(), arg: [a0, a1] };
        let ret = blend_linear(&mut st, &mut o, t, a0, a1, &mut e);
        let got = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(want, got, "case {case} calls, t {t}");
        assert_eq!(want_ret, ret as u64, "case {case} return");
        for (i, w) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *w, "case {case} object word {i:#x} t {t}");
        }
        for i in 0..22 {
            assert_eq!(r.mem.read_u32(FROM + 4 * i), st.from[i as usize], "case {case} from {i}");
            assert_eq!(r.mem.read_u32(TO + 4 * i), st.to[i as usize], "case {case} to {i}");
        }
        assert_eq!(r.mem.read_u32(0x336648), st.from_set as u32);
        assert_eq!(r.mem.read_u32(0x33664c), st.to_set as u32);
        n += 1;
        if ret {
            full += 1
        } else if t <= 0.0 {
            first += 1
        } else {
            part += 1
        }
    }
    eprintln!("blend: {n} cases, {full} finished, {part} part way, {first} at the start");
}

#[test]
fn camera_blend_orbit_matches_the_original() {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x1741d0u32, 0x175450] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0xf00);
    let views: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x130)).collect();
    let lenses: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x60)).collect();
    let pars: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x130)).collect();
    let lens_blk: Vec<u32> = (0..3).map(|_| r.mem.alloc(0x60)).collect();
    for k in 0..3 {
        r.mem.write_u32(views[k] + 0x11c, pars[k]);
        r.mem.write_u32(lenses[k] + 0x44, lens_blk[k]);
    }
    let q: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let log: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let cams: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(vec![]));
    let slot_of = |cams: &Vec<u32>, p: u32| cams.iter().position(|c| *c == p).expect("camera") as u32;
    fn hash(w: &[u32]) -> u32 {
        w.iter().fold(0x1234_5678u32, |h, x| h.rotate_left(5) ^ x.wrapping_mul(0x9e37_79b1))
    }
    let (c, l) = (cams.clone(), log.clone());
    r.stub(0x16dd68, move |cpu, _| {
        let mut c = c.borrow_mut();
        let s = c.len() as u32;
        c.push(cpu.gpr(4) as u32);
        l.borrow_mut().push(vec![1, s, cpu.gpr(5) as u32]);
        cpu.set_gpr(2, cpu.gpr(4));
    });
    let (c, v) = (cams.clone(), views.clone());
    r.stub(0x16dda8, move |cpu, _| {
        let s = c.borrow().iter().position(|x| *x == cpu.gpr(4) as u32).unwrap();
        cpu.set_gpr(2, v[s] as u64);
    });
    let (c, v) = (cams.clone(), lenses.clone());
    r.stub(0x16ddb8, move |cpu, _| {
        let s = c.borrow().iter().position(|x| *x == cpu.gpr(4) as u32).unwrap();
        cpu.set_gpr(2, v[s] as u64);
    });
    let (l, qq, pc) = (log.clone(), q.clone(), pars[2]);
    let vc = views[2];
    r.stub(0x16e900, move |cpu, m| {
        assert_eq!(cpu.gpr(4) as u32, vc);
        assert_eq!(cpu.gpr(5) as u32, obj + 0x484);
        assert_eq!(cpu.gpr(6) as u32, obj + 0xb68);
        assert_eq!(cpu.gpr(8), 0);
        let h = hash(&(0..71).map(|i| m.read_u32(pc + 4 * i)).collect::<Vec<_>>());
        l.borrow_mut().push(vec![2, cpu.gpr(7) as u32, h]);
        let mut q = qq.borrow_mut();
        for i in 0..3 {
            m.write_u32(obj + 0x484 + 4 * i, q.pop_front().unwrap());
        }
    });
    let (l, qq, ln) = (log.clone(), q.clone(), lenses.clone());
    let lb = lens_blk.clone();
    r.stub(0x1722e8, move |cpu, m| {
        let slot = ln.iter().position(|x| *x == cpu.gpr(4) as u32).unwrap() as u32;
        assert_eq!(cpu.gpr(6) as u32, obj + 0xb68);
        assert_eq!(cpu.gpr(8) as u32, obj + 0x10);
        assert_eq!(cpu.gpr(9) as u32, m.read_u32(obj + 0xeac));
        assert_eq!(cpu.gpr(10), 0);
        let h = if slot == 2 { hash(&(0..17).map(|i| m.read_u32(lb[2] + 4 * i)).collect::<Vec<_>>()) } else { 0 };
        let rec = cpu.gpr(5) as u32;
        l.borrow_mut().push(vec![3, slot, cpu.gpr(7) as u32, cpu.f(12).to_bits(), h, (rec == obj + 0x484) as u32]);
        let mut q = qq.borrow_mut();
        for i in 3..7 {
            m.write_u32(rec + 4 * i, q.pop_front().unwrap());
        }
    });
    let l = log.clone();
    r.stub(0x174238, move |_, m| {
        let mut v = vec![4];
        v.extend((0..22).map(|i| m.read_u32(obj + 0x484 + 4 * i)));
        v.push(m.read_u32(obj + 0x18));
        l.borrow_mut().push(v);
    });
    let (l, c) = (log.clone(), cams.clone());
    r.stub(0x174c58, move |cpu, m| {
        let s = slot_of(&c.borrow(), cpu.gpr(5) as u32);
        let mut v = vec![5, s];
        v.extend((0..22).map(|i| m.read_u32(obj + 0x484 + 4 * i)));
        v.push(m.read_u32(obj + 0x18));
        l.borrow_mut().push(v);
    });
    let (l, c) = (log.clone(), cams.clone());
    r.stub(0x16ded0, move |cpu, _| {
        let s = slot_of(&c.borrow(), cpu.gpr(4) as u32);
        l.borrow_mut().push(vec![6, s, cpu.gpr(5) as u32]);
    });

    struct E {
        q: Rc<RefCell<VecDeque<u32>>>,
        log: Rc<RefCell<Vec<Vec<u32>>>>,
    }
    impl OrbitEnv for E {
        fn build(&mut self, slot: u32, arg: u32) {
            self.log.borrow_mut().push(vec![1, slot, arg]);
        }
        fn position(&mut self, c: &[u32; 71], rec: &mut [u32], flag: u32) {
            self.log.borrow_mut().push(vec![2, flag, hash(c)]);
            let mut q = self.q.borrow_mut();
            for i in 0..3 {
                rec[i] = q.pop_front().unwrap();
            }
        }
        fn look_at(&mut self, slot: u32, lc: &[u32; 17], rec: &mut [u32], flag: u32, f12: f32) {
            let h = if slot == 2 { hash(lc) } else { 0 };
            self.log.borrow_mut().push(vec![3, slot, flag, f12.to_bits(), h, (slot == 2) as u32]);
            let mut q = self.q.borrow_mut();
            for i in 3..7 {
                rec[i] = q.pop_front().unwrap();
            }
        }
        fn smooth(&mut self, o: &mut [u32]) {
            let mut v = vec![4];
            v.extend_from_slice(&o[0x484 / 4..0x484 / 4 + 22]);
            v.push(o[0x18 / 4]);
            self.log.borrow_mut().push(v);
        }
        fn hand_over(&mut self, slot: u32, o: &mut [u32]) {
            let mut v = vec![5, slot];
            v.extend_from_slice(&o[0x484 / 4..0x484 / 4 + 22]);
            v.push(o[0x18 / 4]);
            self.log.borrow_mut().push(v);
        }
        fn done(&mut self, slot: u32) {
            self.log.borrow_mut().push(vec![6, slot, 2]);
        }
    }

    let mut g = Rng(0x1754_50c1_9e3b_77a5);
    let (mut n, mut full, mut flipped) = (0, 0, 0);
    for case in 0..20_000 {
        let mut o: Vec<u32> = (0..0x3c0).map(|_| g.next()).collect();
        for i in 0..22 {
            let v = if i < 3 { g.f() * 2000.0 } else if i < 6 { g.ang() * 1.5 } else if i == 6 { (g.next() % 1500) as f32 / 1000.0 + 0.4 } else { f32::from_bits(g.next()) };
            o[0x484 / 4 + i] = v.to_bits();
        }
        o[0x18 / 4] = g.next() % 3;
        let a0 = g.next();
        let a1 = g.next();
        let flip = g.next() % 3 == 0;
        let t = match g.next() % 9 {
            0 => 0.0,
            1 => 1.0,
            2 => 1.0 + g.f().abs(),
            3 => -(g.f().abs()),
            _ => (g.next() % 100_000) as f32 / 100_000.0,
        };
        let mut a: [u32; 71] = std::array::from_fn(|_| g.next());
        let mut b: [u32; 71] = std::array::from_fn(|_| g.next());
        for (blk, off) in [(&mut a, 0usize), (&mut b, 1)] {
            let _ = off;
            blk[0xdc / 4] = (g.ang() * 1.5).to_bits();
            blk[0x100 / 4] = (g.ang() * 0.8).to_bits();
            blk[0xcc / 4] = (g.f() * 300.0).to_bits();
        }
        if g.next() % 4 == 0 {
            b[0xdc / 4] = a[0xdc / 4];
        }
        let c0: [u32; 71] = std::array::from_fn(|_| g.next());
        let la: [u32; 17] = std::array::from_fn(|_| g.next());
        let lc0: [u32; 17] = std::array::from_fn(|_| g.next());
        let words: Vec<u32> = (0..16)
            .map(|i| {
                let k = i % 8;
                (if k < 3 { g.f() * 1500.0 } else if k < 7 { g.ang() * 1.5 } else { g.f() }).to_bits()
            })
            .collect();
        for (i, w) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *w);
        }
        for i in 0..71 {
            r.mem.write_u32(pars[0] + 4 * i, a[i as usize]);
            r.mem.write_u32(pars[1] + 4 * i, b[i as usize]);
            r.mem.write_u32(pars[2] + 4 * i, c0[i as usize]);
        }
        for i in 0..17 {
            r.mem.write_u32(lens_blk[0] + 4 * i, la[i as usize]);
            r.mem.write_u32(lens_blk[2] + 4 * i, lc0[i as usize]);
        }
        *q.borrow_mut() = words.iter().cloned().collect();
        log.borrow_mut().clear();
        cams.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        r.cpu.set_gpr(5, a0 as u64);
        r.cpu.set_gpr(6, a1 as u64);
        r.cpu.set_gpr(7, flip as u64 * (1 + g.next() as u64 % 5));
        r.cpu.set_f(12, t);
        r.call(0x175450).unwrap();
        let want = std::mem::take(&mut *log.borrow_mut());
        let want_ret = r.cpu.gpr(2) & 1;

        *q.borrow_mut() = words.iter().cloned().collect();
        let mut e = E { q: q.clone(), log: log.clone() };
        let (mut c, mut lc) = (c0, lc0);
        let ret = blend_orbit(&mut o, t, flip, &a, &b, &mut c, &la, &mut lc, [a0, a1], &mut e);
        let got = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(want, got, "case {case} calls, t {t}");
        assert_eq!(want_ret, ret as u64, "case {case} return");
        for (i, w) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *w, "case {case} object word {i:#x} t {t}");
        }
        for i in 0..71 {
            assert_eq!(r.mem.read_u32(pars[2] + 4 * i), c[i as usize], "case {case} param {i}");
            assert_eq!(r.mem.read_u32(pars[0] + 4 * i), a[i as usize]);
        }
        for i in 0..17 {
            assert_eq!(r.mem.read_u32(lens_blk[2] + 4 * i), lc[i as usize], "case {case} lens {i}");
        }
        n += 1;
        full += ret as u32;
        flipped += flip as u32;
    }
    eprintln!("orbit: {n} cases, {full} finished, {flipped} flipped");
}
