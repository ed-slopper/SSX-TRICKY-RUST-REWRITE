//! `tricky_game::camscript` against `CamScript_UpdatePath` 0x175cb0, `CamScript_EndPath` 0x175c70, `Camera_Cut`
//! 0x177db0 and `CamScript_StartPath` 0x175ba0 (with `CamPath_EvalStart` 0x170510) in the function runner (TRICKY_ELF;
//! skipped without it), the camera class, the smoothing, the cut fade and the sound call stubbed and logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::collections::VecDeque;
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

const OBJ_WORDS: usize = 0xf00 / 4;

struct Table {
    keys: Vec<Key>,
    owner: u32,
    cur: i32,
    log: Vec<i32>,
}

struct E {
    t: Rc<RefCell<Table>>,
    log: Rc<RefCell<Vec<Vec<u32>>>>,
    owner: Owner,
    owner_ptr: u32,
    q: Rc<RefCell<VecDeque<u32>>>,
    count: i32,
}
impl KeyEnv for E {
    fn key(&mut self, i: i32) -> Key {
        let mut t = self.t.borrow_mut();
        t.log.push(-1000 - i);
        t.keys[i as usize]
    }
}
impl ScriptEnv for E {
    fn owner(&mut self, ptr: u32) -> Owner {
        assert_eq!(ptr, self.owner_ptr);
        self.owner
    }
    fn smooth(&mut self, obj: &mut [u32]) {
        let mut v = vec![4];
        v.extend_from_slice(&obj[0x484 / 4..0x484 / 4 + 8]);
        v.extend_from_slice(&[obj[1], obj[2], obj[0x1e4 / 4]]);
        self.log.borrow_mut().push(v);
    }
    fn cut_fade(&mut self, obj: &mut [u32]) {
        self.log.borrow_mut().push(vec![5, obj[1]]);
    }
    fn audio_cut(&mut self) {
        self.log.borrow_mut().push(vec![6]);
    }
    fn find_owner(&mut self) -> u32 {
        self.log.borrow_mut().push(vec![7, 3]);
        self.owner_ptr
    }
    fn audio_stop(&mut self) {
        self.log.borrow_mut().push(vec![9]);
    }
    fn entry_count(&mut self) -> i32 {
        self.count
    }
    fn reset_rider_anim(&mut self, j: i32) {
        self.log.borrow_mut().push(vec![10, j as u32]);
    }
    fn first_position(&mut self, owner: u32) -> [f32; 3] {
        assert_eq!(owner, self.owner_ptr);
        self.log.borrow_mut().push(vec![8, owner]);
        let mut q = self.q.borrow_mut();
        [f32::from_bits(q.pop_front().unwrap()), f32::from_bits(q.pop_front().unwrap()), f32::from_bits(q.pop_front().unwrap())]
    }
}

#[test]
fn camera_path_steps_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x137140u32, 0x1741d0, 0x1704b0, 0x170610, 0x1708e8, 0x170ca8, 0x170470, 0x170510, 0x175c70, 0x175cb0, 0x175ba0, 0x177db0] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0xf00);
    let owner = r.mem.alloc(0x400);
    let rider = r.mem.alloc(0x10);
    let tab = Rc::new(RefCell::new(Table { keys: vec![], owner, cur: -1, log: vec![] }));
    let log: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let q: Rc<RefCell<VecDeque<u32>>> = Rc::new(RefCell::new(VecDeque::new()));
    let starting = Rc::new(std::cell::Cell::new(false));
    let (t, st, l) = (tab.clone(), starting.clone(), log.clone());
    r.stub(0x16dd68, move |c, _| {
        if st.get() {
            l.borrow_mut().push(vec![8, c.gpr(5) as u32]);
        }
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
    let (t, qq, st) = (tab.clone(), q.clone(), starting.clone());
    r.stub(0x16e900, move |c, m| {
        let t = t.borrow();
        if st.get() {
            assert!(c.gpr(7) == 1 && c.gpr(6) as u32 == obj + 0x4e8 && c.gpr(8) == 0);
            // CamPath_EvalStart's first point
            let mut q = qq.borrow_mut();
            for i in 0..3u32 {
                m.write_u32(c.gpr(5) as u32 + 4 * i, q.pop_front().unwrap());
            }
            return;
        }
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
    r.stub(0x1713d8, move |_, _| {});
    let l = log.clone();
    r.stub(0x174238, move |_, m| {
        let mut v = vec![4];
        v.extend((0..8).map(|i| m.read_u32(obj + 0x484 + 4 * i)));
        v.extend_from_slice(&[m.read_u32(obj + 4), m.read_u32(obj + 8), m.read_u32(obj + 0x1e4)]);
        l.borrow_mut().push(v);
    });
    let l = log.clone();
    r.stub(0x174b38, move |c, m| {
        assert_eq!(c.gpr(4) as u32, obj);
        l.borrow_mut().push(vec![5, m.read_u32(obj + 4)]);
    });
    let l = log.clone();
    r.stub(0x20c3e8, move |_, _| l.borrow_mut().push(vec![6]));
    let (l, cnt) = (log.clone(), tab.clone());
    r.stub(0x16e518, move |c, _| {
        assert_eq!(c.gpr(6), 3);
        l.borrow_mut().push(vec![7, 3]);
        c.set_gpr(2, cnt.borrow().owner as u64);
    });

    let mut g = Rng(0x175c_b0e5_1a2f_93d7);
    let (mut ran, mut ended, mut idle, mut started, mut empty) = (0, 0, 0, 0, 0);
    for case in 0..30_000 {
        let which = g.next() % 4; // 0 update, 1 end, 2 cut (via end), 3 start
        let count = [1i32, 2, 2, 3, 3, 4, 6, 8][(g.next() % 8) as usize];
        let keys: Vec<Key> = (0..count)
            .map(|i| Key {
                pos: [i as f32 * 300.0 + g.f() * 2.0, g.f() * 3.0, g.f() * 3.0],
                ang: [g.ang() * 0.3, g.ang() * 0.3, g.ang() * 0.3],
                w: (g.next() % 1500) as f32 / 1000.0 + 0.5,
            })
            .collect();
        tab.borrow_mut().keys = keys;
        let own_count = if which == 3 && g.next() % 6 == 0 { [0i32, -3][(g.next() % 2) as usize] } else { count };
        let own = Owner { count: own_count, speed: [600.0f32, 1500.0, 3000.0][(g.next() % 3) as usize] };
        r.mem.write_u32(owner + 0x12c, own.count as u32);
        r.mem.write_f32(owner + 0x130, own.speed);
        let mut o: Vec<u32> = (0..OBJ_WORDS).map(|_| g.next()).collect();
        o[0x1e4 / 4] = [1u32, 1, 1, 0, 2][(g.next() % 5) as usize];
        o[0x1e8 / 4 + 1] = g.next() % count as u32;
        o[0x1e8 / 4 + 2] = ((g.next() % 1000) as f32 / 1000.0).to_bits();
        o[0x1e8 / 4 + 3] = 0.05f32.to_bits();
        for k in 0..3 {
            o[0x1e8 / 4 + 4 + k] = g.f().to_bits();
        }
        o[0x1e8 / 4 + 7] = [0.0f32, 400.0, 1200.0, 6000.0][(g.next() % 4) as usize].to_bits();
        o[0x1e8 / 4 + 8] = [0i32, 1, 2, 3, 4, -1][(g.next() % 6) as usize] as u32;
        o[0x1e8 / 4 + 9] = owner;
        o[0x4e8 / 4] = rider;
        let words: Vec<u32> = (0..3).map(|_| g.f().to_bits()).collect();
        let speed = [0.0f32, 300.0, 900.0, 2500.0][(g.next() % 4) as usize];
        let mode = [0i32, 1, 2, 3][(g.next() % 4) as usize];
        for (i, w) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *w);
        }
        *q.borrow_mut() = words.iter().cloned().collect();
        log.borrow_mut().clear();
        tab.borrow_mut().log.clear();
        starting.set(which == 3);
        r.cpu.set_gpr(4, obj as u64);
        match which {
            0 | 2 => r.call(0x175cb0).unwrap(),
            1 => r.call(0x175c70).unwrap(),
            _ => {
                r.cpu.set_f(12, speed);
                r.cpu.set_gpr(5, 0x1234);
                r.cpu.set_gpr(6, mode as i64 as u64);
                r.call(0x175ba0).unwrap()
            }
        }
        let want = std::mem::take(&mut *log.borrow_mut());
        let want_keys: Vec<i32> = tab.borrow().log.iter().filter(|v| **v >= 0).cloned().collect();
        tab.borrow_mut().log.clear();

        *q.borrow_mut() = words.iter().cloned().collect();
        let mut e = E { t: tab.clone(), log: log.clone(), owner: own, owner_ptr: owner, q: q.clone(), count: 0 };
        match which {
            0 | 2 => update_path(&mut o, &mut e),
            1 => end_path(&mut o, &mut e),
            _ => start_path(&mut o, speed, mode, &mut e),
        }
        let got = std::mem::take(&mut *log.borrow_mut());
        let got_keys: Vec<i32> = tab.borrow().log.iter().map(|v| if *v <= -1000 { -1000 - *v } else { *v }).collect();
        assert_eq!(want, got, "case {case} calls (which {which})");
        if which != 3 {
            assert_eq!(want_keys, got_keys, "case {case} key order");
        }
        for (i, w) in o.iter().enumerate() {
            let byte = i * 4;
            // the unwritten words of the evaluation's stack record are leftovers in the original
            let leftover = which != 1 && which != 3 && (0x4a4..0x4dc).contains(&byte) && !matches!(byte, 0x4a4 | 0x4b4 | 0x4d8) || which == 3 && own.count <= 0 && (0x1ec..0x204).contains(&byte);
            if leftover {
                continue;
            }
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *w, "case {case} word {byte:#x} (which {which}, count {})", own.count);
        }
        match which {
            0 | 2 if o[0x1e4 / 4] == 1 || got.is_empty() => {
                if got.is_empty() {
                    idle += 1
                } else {
                    ran += 1
                }
            }
            _ => {}
        }
        ended += got.iter().filter(|v| v[0] == 6).count();
        started += (which == 3) as i32;
        empty += (which == 3 && own.count <= 0) as i32;
    }
    eprintln!("path steps: {ran} ticks, {idle} idle, {ended} cuts, {started} starts ({empty} without keys)");
}

#[test]
fn camera_script_state_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x175e88u32, 0x175de0, 0x175eb8, 0x175e50, 0x176f38, 0x175c70, 0x177db0] {
        r.allow(a);
    }
    let obj = r.mem.alloc(0xf00);
    let names = r.mem.alloc(0x100);
    let app = r.mem.alloc(0x800);
    let racepl = 0x5555_0000u32;
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, racepl);
    let log: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    const RIDER: u32 = 0x4000_0000;
    const STRIDE: u32 = 0x8000;
    r.stub(0x2fd14c, move |c, m| {
        let (d, s) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let mut i = 0;
        loop {
            let b = (m.read_u32((s + i) & !3) >> (8 * ((s + i) & 3))) as u8;
            let w = m.read_u32((d + i) & !3);
            let sh = 8 * ((d + i) & 3);
            m.write_u32((d + i) & !3, (w & !(0xff << sh)) | (b as u32) << sh);
            if b == 0 {
                break;
            }
            i += 1;
        }
        c.set_gpr(2, d as u64);
    });
    r.stub(0x2fd008, move |c, m| {
        let rd = |m: &mut r5900::Mem, a: u32| {
            let mut v = vec![];
            loop {
                let b = (m.read_u32((a + v.len() as u32) & !3) >> (8 * ((a + v.len() as u32) & 3))) as u8;
                if b == 0 {
                    return v;
                }
                v.push(b);
            }
        };
        let (a, b) = (rd(m, c.gpr(4) as u32), rd(m, c.gpr(5) as u32));
        c.set_gpr(2, if a == b { 0 } else { 1 });
    });
    let l = log.clone();
    r.stub(0x231318, move |_, _| l.borrow_mut().push(vec![9]));
    let l = log.clone();
    r.stub(0x20c3e8, move |_, _| l.borrow_mut().push(vec![6]));
    r.stub(0x1814a8, move |c, _| {
        assert_eq!(c.gpr(4) as u32, racepl);
        c.set_gpr(2, (RIDER + STRIDE * c.gpr(5) as u32) as u64);
    });
    let l = log.clone();
    r.stub(0x15ee30, move |c, _| {
        let a = c.gpr(4) as u32 - 0x46c0 - RIDER;
        assert_eq!(a % STRIDE, 0);
        l.borrow_mut().push(vec![10, a / STRIDE]);
    });
    for (k, nm) in [&b"chase near"[..], b"chase far", b"intro 1", b"", b"a", b"fifteen chars x"].iter().enumerate() {
        let a = names + 0x10 * k as u32;
        for i in 0..4 {
            r.mem.write_u32(a + 4 * i, 0);
        }
        for (i, b) in nm.iter().enumerate() {
            let w = r.mem.read_u32((a + i as u32) & !3);
            let sh = 8 * ((a + i as u32) & 3);
            r.mem.write_u32((a + i as u32) & !3, w | (*b as u32) << sh);
        }
    }
    let list: [&[u8]; 6] = [b"chase near", b"chase far", b"intro 1", b"", b"a", b"fifteen chars x"];

    let tab = Rc::new(RefCell::new(Table { keys: vec![], owner: 0, cur: -1, log: vec![] }));
    let mut g = Rng(0x175e_88a1_c4d3_0b27);
    let mut counts = [0u32; 5];
    for case in 0..30_000 {
        let which = (g.next() % 5) as usize; // start, stop, set_name, is_running, reset
        let mut o: Vec<u32> = (0..OBJ_WORDS).map(|_| g.next() % 4).collect();
        // the current name
        let cur = list[(g.next() % 6) as usize];
        copy_name(&mut o, 0x24, cur);
        o[0x34 / 4] = [0u32, 1, 1, 2][(g.next() % 4) as usize];
        o[0x428 / 4] = [4u32, 4, 3, 0][(g.next() % 4) as usize];
        o[0x1e4 / 4] = [1u32, 0, 2][(g.next() % 3) as usize];
        o[0x210 / 4] = g.next() % 2;
        for i in 0..8 {
            o[(0x230 + 0x40 * i) / 4] = g.next() % 3;
        }
        let count = [0i32, 1, 2, 3, 6][(g.next() % 5) as usize];
        r.mem.write_u32(0x32db6c, count as u32);
        let nm_i = (g.next() % 6) as usize;
        let flag = g.next() % 3 != 0;
        for (i, w) in o.iter().enumerate() {
            r.mem.write_u32(obj + 4 * i as u32, *w);
        }
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, obj as u64);
        let mut want_ret = None;
        match which {
            0 => {
                r.cpu.set_gpr(5, (names + 0x10 * nm_i as u32) as u64);
                r.cpu.set_gpr(6, flag as u64);
                r.call(0x175de0).unwrap()
            }
            1 => {
                r.cpu.set_gpr(5, flag as u64);
                r.call(0x175eb8).unwrap()
            }
            2 => {
                r.cpu.set_gpr(5, (names + 0x10 * nm_i as u32) as u64);
                r.call(0x175e88).unwrap()
            }
            3 => {
                r.call(0x175e50).unwrap();
                want_ret = Some(r.cpu.gpr(2) as u32);
            }
            _ => r.call(0x176f38).unwrap(),
        }
        let want = std::mem::take(&mut *log.borrow_mut());
        let mut e = E {
            t: tab.clone(),
            log: log.clone(),
            owner: Owner { count: 0, speed: 0.0 },
            owner_ptr: 0,
            q: Rc::new(RefCell::new(VecDeque::new())),
            count,
        };
        let mut got_ret = None;
        match which {
            0 => start(&mut o, list[nm_i], flag, &mut e),
            1 => stop(&mut o, flag, &mut e),
            2 => set_name(&mut o, list[nm_i]),
            3 => got_ret = Some(is_running(&o)),
            _ => reset_actor_anims(&mut o, &mut e),
        }
        let got = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(want, got, "case {case} calls (which {which}, count {count})");
        assert_eq!(want_ret, got_ret, "case {case} return");
        for (i, w) in o.iter().enumerate() {
            assert_eq!(r.mem.read_u32(obj + 4 * i as u32), *w, "case {case} word {:#x} (which {which})", i * 4);
        }
        counts[which] += 1;
    }
    eprintln!("script state: {counts:?}");
}
