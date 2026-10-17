//! `tricky_game::sky` (board row G7c3) against the original in the function runner (TRICKY_ELF; skipped without it):
//! every case a fresh random sky object (layers with a fake vtable, their data, counts, layer numbers by id), course
//! sky table, sky names, model parts and materials, texture table, sky box node and renderer; then one of 16
//! functions. The models' functions, the files, the course info, the texture table's build, `Lights_Build`, free,
//! delete, the base object and the virtuals are logging stand-ins with answers made from what they are given;
//! `sprintf` is a `%s` formatter; `stricmp` runs as the original. The memory, the calls and the answers compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::pf::Mem;
use tricky_game::sky::{self as s, SkyEnv};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        // splitmix64: successive answers unrelated
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn mix(v: u32) -> u32 {
    let mut z = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^= z >> 29;
    (z.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> 32) as u32
}
fn hash(p: &[u8]) -> u32 {
    p.iter().fold(0x811c_9dc5u32, |h, &c| (h ^ c as u32).wrapping_mul(0x0100_0193))
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x8000;
const APP: u32 = ARENA;
const GAME: u32 = ARENA + 0x400;
const RENDER: u32 = ARENA + 0x1000;
const RVT: u32 = ARENA + 0x1100;
const LVT: u32 = ARENA + 0x1400;
const NAMESTR: u32 = ARENA + 0x1500;
const INFOSTR: u32 = ARENA + 0x1700;
const PATHSTR: u32 = ARENA + 0x1780;
const LDATA: u32 = ARENA + 0x2000;
const PARTS: u32 = ARENA + 0x3000;
const LISTS: u32 = ARENA + 0x3400;
const MATS: u32 = ARENA + 0x4000;
const TEXTAB: u32 = ARENA + 0x5000;
const MODEL: u32 = ARENA + 0x6000;
const NODE: u32 = ARENA + 0x6100;
const LAYER_SLOTS: [u32; 3] = [8, 0x10, 0x30];
const R_SLOTS: [u32; 5] = [0x1c0, 0x1c8, 0x1d0, 0x1d8, 0x1f8];
const WORDS: [&str; 6] = ["gari", "Snow", "mesa", "ALOHA", "pipe", "tri"];
fn lvaddr(off: u32) -> u32 {
    0x01fe_0000 + off
}
fn rvaddr(off: u32) -> u32 {
    0x01ff_0000 + off
}

#[derive(Clone, Debug, PartialEq)]
enum Call {
    V(u32, u32, Vec<u32>),
    C(u32, Vec<u32>),
    P(u32, Vec<u8>, Vec<u32>),
    Table(Vec<u8>),
    Lights([u32; 4], [[u32; 4]; 6]),
    Draw(u32, u32, u32, u32, [u32; 16], Vec<u32>),
}

fn vanswer(func: u32, this: u32, args: &[u32]) -> u32 {
    if func == lvaddr(0x30) {
        (mix(this ^ args[0]) % 3 == 0) as u32
    } else if func == rvaddr(0x1d8) {
        mix(this) & 0xff
    } else {
        0
    }
}
fn canswer(func: u32, args: &[u32]) -> u32 {
    match func {
        s::MODEL_FIRST => mix(args[0] ^ args[1]),
        s::MODEL_PART => PARTS + 0x50 * args[1],
        s::MODEL_MATERIAL => MATS + 0x200 * args[1],
        s::COURSE_INFO => INFOSTR + 0x10 * (mix(args[1]) % 5),
        _ => 0,
    }
}
fn panswer(func: u32, path: &[u8]) -> u32 {
    match func {
        s::FILE_EXISTS => (hash(path) % 4 != 0) as u32,
        s::FILE_LOAD => hash(path),
        _ => 0,
    }
}
fn table_answer(path: &[u8]) -> (u32, u32) {
    let h = hash(path);
    ((h % 5 != 0) as u32, if (h >> 8) % 6 == 0 { 0 } else { TEXTAB })
}
fn lights_answer(ambient: &[u32; 4], lights: &[[u32; 4]; 6]) -> Vec<u32> {
    (0..32u32).map(|k| mix(k ^ ambient[(k % 4) as usize] ^ lights[(k % 6) as usize][(k % 4) as usize].rotate_left(5))).collect()
}

struct Ours {
    log: Vec<Call>,
}
impl SkyEnv for Ours {
    fn vcall(&mut self, _m: &mut Mem, func: u32, this: u32, args: &[u32]) -> u32 {
        self.log.push(Call::V(func, this, args.to_vec()));
        vanswer(func, this, args)
    }
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        self.log.push(Call::C(func, args.to_vec()));
        canswer(func, args)
    }
    fn call_path(&mut self, _m: &mut Mem, func: u32, path: &[u8], args: &[u32]) -> u32 {
        self.log.push(Call::P(func, path.to_vec(), args.to_vec()));
        panswer(func, path)
    }
    fn texture_table(&mut self, _m: &mut Mem, path: &[u8]) -> (u32, u32) {
        self.log.push(Call::Table(path.to_vec()));
        table_answer(path)
    }
    fn lights_build(&mut self, _m: &mut Mem, ambient: [u32; 4], lights: [[u32; 4]; 6]) -> Vec<u32> {
        self.log.push(Call::Lights(ambient, lights));
        lights_answer(&ambient, &lights)
    }
    fn draw_model(&mut self, _m: &mut Mem, func: u32, this: u32, model: u32, part: u32, matrix: [u32; 16], lights: &[u32]) {
        self.log.push(Call::Draw(func, this, model, part, matrix, lights.to_vec()));
    }
}

fn cread(m: &mut r5900::Mem, mut a: u32) -> Vec<u8> {
    let mut v = Vec::new();
    loop {
        let mut b = [0u8];
        m.read(a, &mut b).unwrap();
        if b[0] == 0 {
            return v;
        }
        v.push(b[0]);
        a += 1;
    }
}
fn cwrite(m: &mut Mem, at: u32, s: &[u8]) {
    for (i, &c) in s.iter().enumerate() {
        m.w8(at + i as u32, c);
    }
    m.w8(at + s.len() as u32, 0);
}

#[test]
fn sky_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for a in [
        0x1859c0u32, 0x185a28, 0x185a78, 0x185b18, 0x185c28, 0x185d50, 0x185d68, 0x185dc0, 0x185dc8, 0x185dd0, 0x185f90,
        0x186078, 0x1c7900, 0x1c79d0, 0x1c79e8, 0x1c7c58, 0x2fcc98,
    ] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for off in LAYER_SLOTS {
        let l = log.clone();
        r.stub(lvaddr(off), move |c, _| {
            let (this, f) = (c.gpr(4) as u32, lvaddr(off));
            let args = match off {
                0x10 => vec![],
                _ => vec![c.gpr(5) as u32],
            };
            c.set_gpr(2, vanswer(f, this, &args) as u64);
            l.borrow_mut().push(Call::V(f, this, args));
        });
    }
    for off in R_SLOTS {
        let l = log.clone();
        r.stub(rvaddr(off), move |c, _| {
            let (this, f) = (c.gpr(4) as u32, rvaddr(off));
            let args = match off {
                0x1d8 => vec![],
                0x1c0 | 0x1d0 => vec![c.gpr(5) as u32],
                0x1f8 => vec![c.gpr(5) as u32, c.gpr(6) as u32],
                _ => vec![c.gpr(5) as u32, c.gpr(6) as u32, c.f(12).to_bits()],
            };
            c.set_gpr(2, vanswer(f, this, &args) as u64);
            l.borrow_mut().push(Call::V(f, this, args));
        });
    }
    let l = log.clone();
    r.stub(rvaddr(0x2a8), move |c, m| {
        let (a3, t0) = (c.gpr(7) as u32, c.gpr(8) as u32);
        assert_eq!((c.gpr(9), c.gpr(10)), (0, 0));
        let mat = std::array::from_fn(|k| m.read_u32(a3 + 4 * k as u32));
        let lights = (0..32).map(|k| m.read_u32(t0 + 4 * k)).collect();
        l.borrow_mut().push(Call::Draw(rvaddr(0x2a8), c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, mat, lights));
    });
    for (f, n) in [
        (s::MEM_FREE, 1),
        (s::DELETE, 1),
        (s::MODEL_RELEASE, 2),
        (s::MODEL_CONSTRUCT, 1),
        (s::MODEL_FIRST, 2),
        (s::MODEL_PART, 2),
        (s::MODEL_MATERIAL, 2),
        (s::COURSE_INFO, 2),
        (s::BASE_CONSTRUCT, 2),
        (s::BASE_DESTRUCT, 2),
    ] {
        let l = log.clone();
        r.stub(f, move |c, _| {
            let args: Vec<u32> = (0..n).map(|k| c.gpr(4 + k) as u32).collect();
            c.set_gpr(2, canswer(f, &args) as u64);
            l.borrow_mut().push(Call::C(f, args));
        });
    }
    for (f, n) in [(s::FILE_EXISTS, 0), (s::FILE_LOAD, 1)] {
        let l = log.clone();
        r.stub(f, move |c, m| {
            let path = cread(m, c.gpr(4) as u32);
            let args: Vec<u32> = (0..n).map(|k| c.gpr(5 + k) as u32).collect();
            c.set_gpr(2, panswer(f, &path) as u64);
            l.borrow_mut().push(Call::P(f, path, args));
        });
    }
    let l = log.clone();
    r.stub(s::TEXTURE_TABLE, move |c, m| {
        assert_eq!(c.gpr(4) as u32, s::WORLD);
        let path = cread(m, c.gpr(5) as u32);
        let (ok, t) = table_answer(&path);
        m.write_u32(c.gpr(6) as u32, t);
        c.set_gpr(2, ok as u64);
        l.borrow_mut().push(Call::Table(path));
    });
    let l = log.clone();
    r.stub(0x1ce140, move |c, m| {
        let out = c.gpr(4) as u32;
        let a = c.gpr(5) as u32;
        let ambient = std::array::from_fn(|k| m.read_u32(a + 4 * k as u32));
        let lights: [[u32; 4]; 6] = std::array::from_fn(|i| {
            let p = c.gpr(6 + i) as u32;
            std::array::from_fn(|k| m.read_u32(p + 4 * k as u32))
        });
        for (k, w) in lights_answer(&ambient, &lights).into_iter().enumerate() {
            m.write_u32(out + 4 * k as u32, w);
        }
        l.borrow_mut().push(Call::Lights(ambient, lights));
    });
    // sprintf(out, fmt, ...) for %s
    r.stub(0x2fca28, |c, m| {
        let fmt = cread(m, c.gpr(5) as u32);
        let mut out = Vec::new();
        let (mut arg, mut i) = (6, 0);
        while i < fmt.len() {
            if fmt[i] == b'%' && fmt.get(i + 1) == Some(&b's') {
                out.extend(cread(m, c.gpr(arg) as u32));
                arg += 1;
                i += 2;
            } else {
                out.push(fmt[i]);
                i += 1;
            }
        }
        out.push(0);
        m.write(c.gpr(4) as u32, &out).unwrap();
        c.set_gpr(2, out.len() as u64 - 1);
    });
    let elf = |r: &mut Runner, at: u32, n: usize| {
        let mut v = vec![0u8; n];
        r.mem.read(at, &mut v).unwrap();
        (at, v)
    };
    let fixed = [elf(&mut r, s::PATH_FORMAT, 0x20), elf(&mut r, s::EXT_MODEL, 8), elf(&mut r, s::SKY_FORMAT, 8)];
    assert_eq!(&fixed[0].1[..0x11], b"data/models/%s%s\0");
    let mut g = Rng(0x5c7_0000_0000_0001);
    let mut counts = [0usize; 16];
    let (mut loads, mut draws, mut found) = (0usize, 0usize, 0usize);
    for case in 0..16_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (0, vec![0; 0x100]),
                (s::SKY, vec![0; 0x100]),
                (s::NAMES, vec![0; 0x30]),
                (s::GAPP, vec![0; 4]),
                (s::TABLE, vec![0; 0x30 * 8]),
                (0x346440, vec![0; 0x30]),
                (0x3d0d20, vec![0; 0x50]),
            ],
        };
        m.regions.extend(fixed.iter().cloned());
        for (at, len) in [(ARENA, SIZE), (0, 0x100), (s::SKY, 0x100), (0x346440, 0x30), (0x3d0d20, 0x50)] {
            for k in 0..len / 4 {
                m.w32(at + 4 * k, g.next());
            }
        }
        // (a layer without data reads its counts from address 0)
        m.w32(8, g.below(3));
        m.w32(0x14, g.below(3));
        m.w32(s::GAPP, APP);
        m.w32(APP + 0x724, RENDER);
        m.w32(APP + 0x730, GAME);
        m.w32(GAME + 0x290, g.below(4));
        m.w32(RENDER + 0x10, RVT);
        for off in R_SLOTS.into_iter().chain([0x2a8]) {
            m.w16(RVT + off, [0u16, 0x20, 0xfff0][g.below(3) as usize]);
            m.w32(RVT + off + 4, rvaddr(off));
        }
        for off in LAYER_SLOTS {
            m.w16(LVT + off, [0u16, 0x10, 0xffec][g.below(3) as usize]);
            m.w32(LVT + off + 4, lvaddr(off));
        }
        // the sky names (some the same but for case) and the course names
        for i in 0..12 {
            let w = WORDS[g.below(6) as usize];
            let n = if g.below(4) == 0 { format!("{}_SKY", w.to_ascii_lowercase()) } else { format!("{w}_sky") };
            cwrite(&mut m, NAMESTR + 0x18 * i, n.as_bytes());
            m.w32(s::NAMES + 4 * i, NAMESTR + 0x18 * i);
        }
        for i in 0..5 {
            cwrite(&mut m, INFOSTR + 0x10 * i, [WORDS[g.below(6) as usize], "none"][(g.below(5) == 0) as usize].as_bytes());
        }
        cwrite(&mut m, PATHSTR, format!("data/models/{}.ssh", WORDS[g.below(6) as usize]).as_bytes());
        // the layers: their vtable, data (now and then none), counts
        for i in 0..10 {
            let l = s::layer(s::SKY, i);
            m.w32(l + 0x10, LVT);
            let d = LDATA + 0x20 * i;
            m.w32(l + 4, if g.below(5) == 0 { 0 } else { d });
            m.w32(d + 0x14, [u32::MAX, 0, 1, 2, 3, 6][g.below(6) as usize]);
            m.w32(d + 8, [u32::MAX, 0, 1, 2, 4][g.below(5) as usize]);
        }
        m.w32(s::SKY + 0xcc, [0u32, 1, 2, 5, 9, 10, 11, g.below(10)][g.below(8) as usize]);
        for k in 0..12 {
            m.w32(s::SKY + 0xd0 + 4 * k, if g.below(4) == 0 { u32::MAX } else { g.below(10) });
        }
        // parts (texture shorts, now and then negative; word lists) and materials (shorts and flags)
        for p in 0..6 {
            let at = PARTS + 0x50 * p;
            for k in 0..4 {
                m.w16(at + 2 * k, if g.below(4) == 0 { 0x8000 | g.next() as u16 } else { g.below(64) as u16 });
            }
            let list = LISTS + 0x40 * p;
            m.w32(at + 0x44, if g.below(4) == 0 { 0 } else { list });
            m.w32(list, [u32::MAX, 0, 1, 3, 8][g.below(5) as usize]);
            for k in 0..8 {
                m.w32(list + 4 + 4 * k, g.below(64));
            }
        }
        for q in 0..4 {
            let at = MATS + 0x200 * q;
            for k in 0..4 {
                m.w16(at + 0x1b0 + 2 * k, if g.below(4) == 0 { 0x8000 | g.next() as u16 } else { g.below(64) as u16 });
            }
            let mut fl = g.next();
            for k in 0..4 {
                if g.below(3) == 0 {
                    fl = (fl & !(7 << (3 * k))) | (5 << (3 * k));
                }
            }
            m.w32(at + 0x1ac, fl);
        }
        // the texture table (numbers that index it again, as a second load of a layer looks its new ones up)
        for k in 0..64 {
            m.w32(TEXTAB + 4 * k, g.below(64));
        }
        // the course sky table: rows (course, count, ids), a count of −1 now and then, a −1 row last
        let rows = g.below(7);
        for k in 0..8 {
            let at = s::TABLE + 0x30 * k;
            if k < rows {
                m.w32(at, if g.below(10) == 0 { u32::MAX } else { g.below(8) });
                m.w32(at + 4, if g.below(10) == 0 { u32::MAX } else { g.below(5) });
                for j in 0..10 {
                    m.w32(at + 8 + 4 * j, g.below(12));
                }
            } else {
                m.w32(at, u32::MAX);
                m.w32(at + 4, u32::MAX);
            }
        }
        m.w32(NODE + 0xc, if g.below(6) == 0 { 0 } else { MODEL });
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut ours = Ours { log: Vec::new() };
        let kind = g.below(16) as usize;
        let mut ans = None;
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                s::init_layers(&mut m, &mut ours, s::SKY);
                (0x1859c0, vec![s::SKY])
            }
            1 => {
                s::free_layers(&mut m, &mut ours, s::SKY);
                (0x185a28, vec![s::SKY])
            }
            2 => {
                let n = m.r32(s::SKY + 0xcc);
                let i = if g.below(2) == 0 && (1..=10).contains(&n) { n - 1 } else { g.below(10) };
                s::free_layer(&mut m, &mut ours, s::SKY, i);
                (0x185a78, vec![s::SKY, i])
            }
            3 => {
                let i = g.below(13).wrapping_sub(1);
                ans = Some(s::model_by_index(&mut m, &mut ours, s::SKY, i));
                (0x185d68, vec![s::SKY, i])
            }
            4 => {
                let c = g.below(9);
                s::load_skies(&mut m, &mut ours, s::SKY, c);
                (0x185b18, vec![s::SKY, c])
            }
            5 => {
                let name = NAMESTR + 0x18 * g.below(12);
                ans = Some(s::load_sky(&mut m, &mut ours, s::SKY, name) as u32);
                (0x185c28, vec![s::SKY, name])
            }
            6 => {
                let l = s::layer(s::SKY, g.below(10));
                let mut p = Vec::new();
                for k in 0.. {
                    let c = m.r8(PATHSTR + k);
                    if c == 0 {
                        break;
                    }
                    p.push(c);
                }
                s::remap_textures(&mut m, &mut ours, s::SKY, l, &p);
                (0x185dd0, vec![s::SKY, l, PATHSTR])
            }
            7 => {
                let fl = g.below(4);
                s::delete(&mut m, &mut ours, s::SKY, fl);
                (0x186078, vec![s::SKY, fl])
            }
            8 => {
                s::static_construct(&mut m, &mut ours);
                (0x185f90, vec![])
            }
            9 => {
                let a1 = g.next();
                ans = Some(s::node_construct(&mut m, &mut ours, NODE, a1));
                (0x1c7900, vec![NODE, a1])
            }
            10 => {
                let a1 = g.below(4);
                s::node_destruct(&mut m, &mut ours, NODE, a1);
                (0x1c79d0, vec![NODE, a1])
            }
            11 | 12 => {
                s::node_draw(&mut m, &mut ours, NODE);
                (0x1c79e8, vec![NODE])
            }
            13 => {
                s::node_static_init(&mut m);
                (0x1c7c58, vec![])
            }
            14 => {
                let i = g.below(12);
                ans = Some(s::layer(s::SKY, i));
                (0x185d50, vec![s::SKY, i])
            }
            _ => {
                if g.below(2) == 0 {
                    ans = Some(s::layer_count(&m, s::SKY));
                    (0x185dc0, vec![s::SKY])
                } else {
                    let n = g.next();
                    s::set_layer_count(&mut m, s::SKY, n);
                    (0x185dc8, vec![s::SKY, n])
                }
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {func:#x} answer");
        }
        assert_eq!(*log.borrow(), ours.log, "case {case}: {func:#x} calls");
        loads += ours.log.iter().filter(|c| matches!(c, Call::C(s::MODEL_RELEASE, _))).count();
        draws += ours.log.iter().filter(|c| matches!(c, Call::Draw(..))).count();
        if kind == 9 && m.r32(NODE + 0xc) != 0 {
            found += 1;
        }
        for (at, b) in &m.regions {
            let mut theirs = vec![0u8; b.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != b {
                let i = theirs.iter().zip(b).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], b[i]);
            }
        }
    }
    println!("{counts:?}, {loads} loads, {draws} draws, {found} found");
    assert!(loads > 100 && draws > 100 && found > 100);
}
