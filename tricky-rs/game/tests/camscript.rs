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
        let mut e = E { t: tab.clone(), log: log.clone(), owner: own, owner_ptr: owner, q: q.clone() };
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
