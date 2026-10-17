//! `tricky_game::bdrload` (board row G7r1) against the original in the function runner (TRICKY_ELF; skipped
//! without it). Every case is one of the row's functions on fresh random memory: banks of any size (and negative
//! requests), shape lists of 0 to 12 names with repeats and empty names, files whose shapes are found or not,
//! riders whose records have ids below 0 and the flag bits set or not. The heaps, files, `strncmp`, the copies and
//! fills, the renderer's and texture managers' virtuals and the skin calls are stand-ins on both sides that do
//! their work (compare, copy and fill inside the test's memory, answer by their arguments) and log what they are
//! handed. The calls, the answer and the memory are compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::bdrload as bl;
use tricky_game::pf::Mem;
use tricky_game::ps2f::F;
use tricky_game::textbox::TextBoxEnv;

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
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x2000;
const OBJ: u32 = ARENA;
const LIST: u32 = ARENA + 0x400;
const NAME: u32 = ARENA + 0x600;
const APPOBJ: u32 = ARENA + 0x800;
const RENDER: u32 = ARENA + 0xc00;
const RVT: u32 = ARENA + 0xd00;
const MANAGERS: u32 = ARENA + 0xe00;
const MVT: u32 = ARENA + 0x1000;
/// The rider's skin flag (its +0x79c + 0x3f14)
const SKIN_FLAG: u32 = 0x0140_0000;
/// The app (0x337c58) and the skin's strings (0x39e4f0..)
const DATA: [(u32, u32); 2] = [(0x337c00, 0x100), (0x39e4f0, 0x50)];
/// The renderer's and the managers' virtuals
fn rv(off: u32) -> u32 {
    0x01ff_0000 + off
}
const MV: u32 = 0x01fe_0068;

#[derive(Clone, Debug, PartialEq)]
struct Call(u32, Vec<u32>);

thread_local! {
    static FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

trait Io {
    fn r8(&mut self, a: u32) -> u8;
    fn w8(&mut self, a: u32, v: u8);
}
impl Io for Mem {
    fn r8(&mut self, a: u32) -> u8 {
        Mem::r8(self, a)
    }
    fn w8(&mut self, a: u32, v: u8) {
        Mem::w8(self, a, v)
    }
}
impl Io for r5900::Mem {
    fn r8(&mut self, a: u32) -> u8 {
        let mut b = [0u8; 1];
        self.read(a, &mut b).unwrap();
        b[0]
    }
    fn w8(&mut self, a: u32, v: u8) {
        self.write_u8(a, v)
    }
}

/// Inside the test's arena, where the copies and fills are done for real
fn inside(a: u32, n: u32) -> bool {
    a >= ARENA && (a as u64 + n as u64) <= (ARENA + SIZE) as u64
}

/// A stand-in's work, answer and log (`w` the words a0.., then the stack's two)
fn standin(io: &mut dyn Io, func: u32, w: &[u32]) -> (u32, Call) {
    let mut log = Vec::new();
    let answer = match func {
        bl::STRNCMP => {
            log.extend(&w[..3]);
            let mut d = 0i32;
            for k in 0..w[2] {
                let (a, b) = (io.r8(w[0] + k), io.r8(w[1] + k));
                if a != b {
                    d = a as i32 - b as i32;
                    break;
                }
                if a == 0 {
                    break;
                }
            }
            d as u32
        }
        bl::MEM_COPY | bl::MEM_SET => {
            log.extend(&w[..3]);
            if inside(w[0], w[2]) && (func == bl::MEM_SET || inside(w[1], w[2])) {
                for k in 0..w[2] {
                    let c = if func == bl::MEM_SET { (w[1] >> (8 * (k % 4))) as u8 } else { io.r8(w[1] + k) };
                    io.w8(w[0] + k, c);
                }
            }
            0
        }
        bl::ALLOC => {
            log.extend(&w[..3]);
            // (the buffer sometimes not had)
            if w[1] == 0x10000 { if FAILS.get() { 0 } else { 0x0300_0000 } } else { 0x0300_0000 + 0x100 * (w[1] % 13) }
        }
        bl::FILE_LOAD => {
            log.extend(&w[..2]);
            0x0310_0000 + w[0] % 0x100
        }
        bl::SHAPE_FIND => {
            // found for names whose first character is even
            let c = io.r8(w[1]);
            log.extend([w[0], c as u32]);
            if c % 2 == 0 { 0x0320_0000 + c as u32 } else { 0 }
        }
        bl::RESET_SKIN | bl::BUILD_SKIN => {
            log.extend(&w[..2]);
            0
        }
        f if f == rv(0x158) => {
            log.extend(&w[..10]);
            0x0400_0000 + w[3]
        }
        f if f == rv(0x150) => {
            log.extend(&w[..4]);
            0x0410_0000 + (w[1] & 0xff)
        }
        f if f == rv(0x160) => {
            log.extend(&w[..3]);
            0
        }
        MV => {
            log.extend(&w[..3]);
            0x0420_0000 + w[1].wrapping_mul(3)
        }
        _ => {
            log.push(w[0]);
            0
        }
    };
    log.push(answer);
    (answer, Call(func, log))
}

const STANDINS: [u32; 13] = [
    bl::ALLOC, bl::MEM_SET, bl::MEM_COPY, bl::FREE, bl::DELETE, bl::STRNCMP, bl::FILE_LOAD, bl::EAMEM_FREE, bl::SHAPE_FIND,
    bl::RESET_SKIN, bl::CLEAR_PARTS, bl::BUILD_SKIN, MV,
];

struct Ours(Vec<Call>);
impl TextBoxEnv for Ours {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32], _fargs: &[F]) -> u32 {
        let mut w = args.to_vec();
        w.resize(10, 0);
        let (a, c) = standin(m, func, &w);
        self.0.push(c);
        a
    }
}

#[test]
fn shape_lists_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let funcs = [
        0x203e88u32, 0x203e98, 0x203f08, 0x203f60, 0x203fa0, 0x203fd0, 0x203ff8, 0x204048, 0x204068, 0x2042a0, 0x2042e8, 0x2043b0,
        0x2043f8, 0x2044c0, 0x204588, 0x204598, 0x2021f8,
    ];
    for f in funcs {
        r.allow(f);
    }
    let log: Rc<RefCell<Vec<Call>>> = Rc::default();
    for func in STANDINS.into_iter().chain([rv(0x158), rv(0x150), rv(0x160)]) {
        let l = log.clone();
        r.stub(func, move |c, mem| {
            let mut w: Vec<u32> = (4..12).map(|k| c.gpr(k) as u32).collect();
            let sp = c.gpr(29) as u32;
            w.push(mem.read_u32(sp));
            w.push(mem.read_u32(sp + 8));
            let (a, call) = standin(mem, func, &w);
            l.borrow_mut().push(call);
            c.set_gpr(2, a as i32 as i64 as u64);
        });
    }
    let data: Vec<Vec<u8>> = DATA
        .iter()
        .map(|&(at, n)| {
            let mut b = vec![0u8; n as usize];
            r.mem.read(at, &mut b).unwrap();
            b
        })
        .collect();
    let mut g = Rng(0x5ce1_0000_0000_0001);
    let (mut added, mut found, mut built) = (0usize, 0usize, 0usize);
    for case in 0..17 * 1200u32 {
        let mut m = Mem { regions: vec![(ARENA, vec![0; SIZE as usize]), (SKIN_FLAG, vec![0; 0x10])] };
        for (k, &(at, _)) in DATA.iter().enumerate() {
            m.regions.push((at, data[k].clone()));
        }
        for k in 0..SIZE / 4 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // the bank
        m.w32(OBJ, g.pick(&[0u32, 0x0100_0081, 0x0100_0100, 0xffff_ff90, 0xffff_ff00]));
        // the list: names of letters, some repeated, ending each in a 0
        let n = g.pick(&[0u32, 1, 5, 12]);
        m.w32(LIST, n);
        let pool: [&[u8; 4]; 6] = [b"head", b"helm", b"boot", b"suit", b"bord", b"arms"];
        for k in 0..n {
            let nm = pool[g.below(6) as usize];
            for (j, &c) in nm.iter().enumerate() {
                m.w8(LIST + 4 + 5 * k + j as u32, c);
            }
            m.w8(LIST + 8 + 5 * k, 0);
            m.w32(LIST + 0x130 + 4 * k, { let v = g.next() & 0xffff; g.pick(&[0xffff_ffffu32, 0, v]) });
        }
        let nm: &[u8] = g.pick(&[&b"head"[..], b"suit", b"arms", b"legs", b"he", b""]);
        for (j, &c) in nm.iter().chain([0u8].iter()).enumerate() {
            m.w8(NAME + j as u32, c);
        }
        // the app's renderer and the three texture managers
        m.w32(0x337c58, APPOBJ);
        m.w32(APPOBJ + 0x724, RENDER);
        m.w32(RENDER + 0x10, RVT);
        for off in [0x150u32, 0x158, 0x160] {
            m.w16(RVT + off, g.pick(&[0u16, 0x10]));
            m.w32(RVT + off + 4, rv(off));
        }
        for k in 0..3 {
            let tm = MANAGERS + 0x40 * k;
            m.w32(OBJ + 8 + 4 * k, if k == 1 && g.below(2) == 0 { 0 } else { tm });
            m.w32(tm + 0x10, MVT);
        }
        m.w16(MVT + 0x68, g.pick(&[0u16, 0x10]));
        m.w32(MVT + 0x6c, MV);
        // the rider's record: its ids, the rider, the flags
        let rider_n = g.below(3);
        let rec = OBJ + 0x14 + 0x50 * rider_n;
        for k in 0..8 {
            let at = if k < 3 { rec + 0x10 + 4 * k } else { rec + 0x28 + 4 * (k - 3) };
            m.w32(at, { let v = g.below(40); g.pick(&[0xffff_ffffu32, v]) });
        }
        m.w32(rec + 4, SKIN_FLAG - 0x79c - 0x3f14);
        m.w32(SKIN_FLAG, g.below(2));
        m.w32(rec + 0xc, g.pick(&[0u32, 0x100, 0x2100, 0x200, 0x300]));
        let (size, flags) = (g.pick(&[0u32, 0x100, 0x7f, 0x81, 0xffff_ff00, 0xffff_ff7f]), g.below(4));
        FAILS.set(g.below(4) == 0);
        let file = g.next();
        for (at, b) in &m.regions {
            r.mem.write(*at, b).unwrap();
        }
        log.borrow_mut().clear();
        let mut o = Ours(Vec::new());
        let which = (case % 17) as usize;
        let func = funcs[which];
        let (regs, ours): (Vec<u32>, u32) = match which {
            0 => (vec![OBJ], bl::bank_init(&mut m, OBJ)),
            1 => {
                bl::mem_bank_init(&mut m, &mut o, OBJ, size);
                (vec![OBJ, size], 0)
            }
            2 => {
                bl::bank_delete(&mut m, &mut o, OBJ, flags);
                (vec![OBJ, flags], 0)
            }
            3 => (vec![OBJ, size], bl::bank_alloc(&mut m, OBJ, size)),
            4 => {
                bl::align(&mut m, OBJ);
                (vec![OBJ], 0)
            }
            5 => (vec![LIST], bl::list_construct(&mut m, &mut o, LIST)),
            6 => {
                bl::list_destruct(&mut m, &mut o, LIST, flags);
                (vec![LIST, flags], 0)
            }
            7 => {
                bl::list_clear(&mut m, &mut o, LIST);
                (vec![LIST], 0)
            }
            8 => {
                bl::load_outfit_parts(&mut m, &mut o, LIST);
                (vec![LIST], 0)
            }
            9 => {
                bl::register_shape_file(&mut m, &mut o, LIST, file);
                (vec![LIST, file], 0)
            }
            10 => {
                bl::register_shapes(&mut m, &mut o, LIST, file);
                (vec![LIST, file], 0)
            }
            11 => {
                bl::rebind_shape_file(&mut m, &mut o, LIST, file);
                (vec![LIST, file], 0)
            }
            12 => {
                bl::rebind_shapes(&mut m, &mut o, LIST, file);
                (vec![LIST, file], 0)
            }
            13 => (vec![LIST, NAME], bl::find_part_name(&mut m, &mut o, LIST, NAME)),
            14 => {
                let i = g.below(12);
                (vec![LIST, i], bl::shape_slot(&m, LIST, i))
            }
            15 => (vec![LIST, NAME], bl::find_texture(&mut m, &mut o, LIST, NAME)),
            _ => {
                bl::bind_textures(&mut m, &mut o, OBJ, rider_n);
                (vec![OBJ, rider_n], 0)
            }
        };
        for (k, v) in regs.into_iter().enumerate() {
            r.cpu.set_gpr(4 + k, v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case} {func:#x}: {e}"));
        let theirs = log.borrow().clone();
        if theirs != o.0 {
            let i = theirs.iter().zip(&o.0).position(|(a, b)| a != b).unwrap_or(theirs.len().min(o.0.len()));
            let show = |c: Option<&Call>| c.map(|c| format!("{:x} {:x?}", c.0, &c.1[..c.1.len().min(12)])).unwrap_or_default();
            panic!("case {case} {func:#x}: call {i} of {}/{}: theirs {} ours {}", theirs.len(), o.0.len(), show(theirs.get(i)), show(o.0.get(i)));
        }
        let answer = r.cpu.gpr(2) as u32;
        let returns = matches!(which, 0 | 3 | 5 | 13 | 14 | 15);
        assert!(!returns || answer == ours, "case {case} {func:#x}: answer theirs {answer:#x} ours {ours:#x}");
        for (at, b) in &m.regions {
            let mut th = vec![0u8; b.len()];
            r.mem.read(*at, &mut th).unwrap();
            if &th != b {
                let i = th.iter().zip(b).position(|(x, y)| x != y).unwrap() & !3;
                let w = |v: &[u8]| u32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                panic!("case {case} {func:#x}: memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, w(&th), w(b));
            }
        }
        added += (which == 13 && ours == n) as usize;
        found += (which == 15 && (ours as i32) >= 0) as usize;
        built += (which == 16 && o.0.iter().any(|c| c.0 == bl::RESET_SKIN || c.0 == bl::CLEAR_PARTS)) as usize;
    }
    println!("{added} names added, {found} textures found, {built} skins reset or cleared");
    assert!(added > 200 && found > 80 && built > 100);
}
