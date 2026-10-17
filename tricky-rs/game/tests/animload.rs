//! `tricky_game::animload` (board row G5c2) against the original in the function runner (TRICKY_ELF; skipped
//! without it): every case a fresh random bank at its own address (its memory bank's block in an arena, its files
//! loaded, asked for or neither), random clip name hashes, a random race roster (humans and computers, repeated
//! characters and classes) and a BIG file object with a fake vtable; then one of ten functions. `sprintf`, the bank's
//! own functions and the bind run as the original; opening, sizing, reading and freeing files and the BIG file's
//! calls are logging stand-ins whose reads write generated `.afl` images. Memory and calls compared.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::animdata as d;
use tricky_game::animload as l;
use tricky_game::animseq::{self as a, SeqEnv};
use tricky_game::pf::Mem;

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

const ARENA: u32 = 0x0100_0000;
const SIZE: u32 = 0x20000;
const BLOCK: u32 = ARENA + 0x1000;
const BIGVT: u32 = ARENA + 0x100;
const LIST: u32 = ARENA + 0x200;
const KEY: u32 = ARENA + 0x300;
const DATA: u32 = ARENA + 0x400;
const BANK_LEN: u32 = 0x30f4;
const ROSTER_LEN: u32 = 4 + 0x84 * 8;
// the BIG object's virtuals (stand-in addresses)
const V_REQUEST: u32 = 0x01ff_0000;
const V_READY: u32 = 0x01ff_0010;
const V_CLOSE: u32 = 0x01ff_0020;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Open(Vec<u8>),
    Size(u32),
    Read(u32, u32),
    Free(u32),
    Request(Vec<u8>),
    Ready(u32),
    BigSize(u32),
    BigRead(u32, u32),
    Close(u32),
    BigOpen,
    BigReset,
    Other(u32, Vec<u32>),
}

fn hash_of(path: &[u8]) -> u32 {
    path.iter().fold(0x811c_9dc5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// A generated `.afl` image for a handle: a few clips whose names' hashes are among the table's (or not)
fn image(h: u32, hashes: &[u32]) -> Vec<u8> {
    let n = 1 + h % 6;
    let mut v = vec![0u8; 0xc + 0x24 * n as usize + 0x10];
    v[2..4].copy_from_slice(&(n as u16).to_le_bytes());
    v[4..8].copy_from_slice(&(0xc + 0x24 * n).to_le_bytes());
    v[8..12].copy_from_slice(&(0xc + 0x24 * n + 8).to_le_bytes());
    for c in 0..n {
        let k = (h >> (3 * c)) as usize;
        let hash = match k % 4 {
            0 => 0,
            1 => 0x1234_5678,
            _ => hashes[(k / 4) % hashes.len()],
        };
        let at = 0xc + 0x24 * c as usize;
        v[at..at + 4].copy_from_slice(&hash.to_le_bytes());
    }
    v
}

fn size_of(h: u32) -> u32 {
    // (sometimes exactly on the bank's 128-byte steps)
    if h % 7 == 0 { 0x80 * (1 + h % 3) - (h / 7) % 2 } else { 0xc + 0x24 * (1 + h % 6) + 0x10 }
}

struct Ours {
    log: Vec<Call>,
    hashes: Vec<u32>,
}
impl l::LoadEnv for Ours {
    fn open(&mut self, path: &[u8]) -> u32 {
        self.log.push(Call::Open(path.to_vec()));
        hash_of(path)
    }
    fn size(&mut self, h: u32) -> u32 {
        self.log.push(Call::Size(h));
        size_of(h)
    }
    fn read(&mut self, m: &mut Mem, h: u32, dst: u32) {
        self.log.push(Call::Read(h, dst));
        for (k, b) in image(h, &self.hashes).iter().enumerate() {
            m.w8(dst + k as u32, *b);
        }
    }
    fn free(&mut self, h: u32) {
        self.log.push(Call::Free(h));
    }
    fn big_request(&mut self, _m: &mut Mem, _big: u32, path: &[u8]) -> u32 {
        self.log.push(Call::Request(path.to_vec()));
        hash_of(path) ^ 0xb16
    }
    fn big_ready(&mut self, _m: &mut Mem, _big: u32, h: u32) -> bool {
        self.log.push(Call::Ready(h));
        h % 3 != 0
    }
    fn big_size(&mut self, _m: &mut Mem, _big: u32, h: u32) -> u32 {
        self.log.push(Call::BigSize(h));
        size_of(h)
    }
    fn big_read(&mut self, m: &mut Mem, _big: u32, h: u32, dst: u32) {
        self.log.push(Call::BigRead(h, dst));
        for (k, b) in image(h, &self.hashes).iter().enumerate() {
            m.w8(dst + k as u32, *b);
        }
    }
    fn big_close(&mut self, _m: &mut Mem, _big: u32, h: u32) {
        self.log.push(Call::Close(h));
    }
    fn big_open(&mut self, _m: &mut Mem, _big: u32) {
        self.log.push(Call::BigOpen);
    }
    fn big_reset(&mut self, _m: &mut Mem, _big: u32) {
        self.log.push(Call::BigReset);
    }
}
impl SeqEnv for Ours {
    fn call(&mut self, _m: &mut Mem, func: u32, args: &[u32]) -> u32 {
        if func == d::BIG_RESET {
            self.log.push(Call::BigReset);
        } else {
            self.log.push(Call::Other(func, args.to_vec()));
        }
        0
    }
}

fn cstr(m: &mut r5900::Mem, mut p: u32) -> Vec<u8> {
    let mut v = Vec::new();
    loop {
        let b = (m.read_u32(p & !3) >> (8 * (p & 3))) as u8;
        if b == 0 {
            return v;
        }
        v.push(b);
        p += 1;
    }
}

#[test]
fn the_loads_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    for f in [
        0x158c10u32, 0x158908, 0x1589b8, 0x158a30, 0x158b10, 0x1597d0, 0x159738, 0x158fc0, 0x159150, 0x159730, 0x159140,
        0x158ea0, 0x157e68, 0x159070, 0x157ea8, 0x158af0, 0x2cc808,
    ] {
        r.allow(f);
    }
    // sprintf's helpers (the C library) run as the original too
    let hashes_live: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
    let log = Rc::new(RefCell::new(Vec::new()));
    let stub = |r: &mut Runner, f: u32, mk: Box<dyn Fn(&mut r5900::Cpu, &mut r5900::Mem) -> (Call, u64)>| {
        let l = log.clone();
        r.stub(f, move |c, m| {
            let (call, v) = mk(c, m);
            l.borrow_mut().push(call);
            c.set_gpr(2, v);
        });
    };
    // sprintf with one %s (the loader's only use): the string written as the C library would
    r.stub(0x2fca28, |c, m| {
        let (dst, fmt, arg) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        let f = cstr(m, fmt);
        let s = cstr(m, arg);
        let mut out = Vec::new();
        let mut k = 0;
        while k < f.len() {
            if f[k] == b'%' && f.get(k + 1) == Some(&b's') {
                out.extend_from_slice(&s);
                k += 2;
            } else {
                out.push(f[k]);
                k += 1;
            }
        }
        out.push(0);
        for (i, b) in out.iter().enumerate() {
            m.write_u8(dst + i as u32, *b);
        }
        c.set_gpr(2, (out.len() - 1) as u64);
    });
    stub(&mut r, 0x2c8018, Box::new(|c, m| {
        let p = cstr(m, c.gpr(4) as u32);
        let h = hash_of(&p);
        (Call::Open(p), h as i32 as i64 as u64)
    }));
    stub(&mut r, 0x2c3190, Box::new(|c, _| (Call::Size(c.gpr(4) as u32), size_of(c.gpr(4) as u32) as u64)));
    let hl = hashes_live.clone();
    stub(&mut r, 0x2c2f90, Box::new(move |c, m| {
        let (h, dst) = (c.gpr(4) as u32, c.gpr(5) as u32);
        for (k, b) in image(h, &hl.borrow()).iter().enumerate() {
            m.write_u8(dst + k as u32, *b);
        }
        (Call::Read(h, dst), 0)
    }));
    stub(&mut r, 0x2ccfc0, Box::new(|c, _| (Call::Free(c.gpr(4) as u32), 0)));
    stub(&mut r, V_REQUEST, Box::new(|c, m| {
        let p = cstr(m, c.gpr(5) as u32);
        let h = hash_of(&p) ^ 0xb16;
        (Call::Request(p), h as i32 as i64 as u64)
    }));
    stub(&mut r, V_READY, Box::new(|c, _| {
        let h = c.gpr(5) as u32;
        (Call::Ready(h), (h % 3 != 0) as u64)
    }));
    stub(&mut r, V_CLOSE, Box::new(|c, _| (Call::Close(c.gpr(5) as u32), 0)));
    stub(&mut r, 0x23c7c8, Box::new(|c, _| (Call::BigSize(c.gpr(5) as u32), size_of(c.gpr(5) as u32) as u64)));
    let hl = hashes_live.clone();
    stub(&mut r, 0x23c818, Box::new(move |c, m| {
        let (h, dst) = (c.gpr(5) as u32, c.gpr(6) as u32);
        for (k, b) in image(h, &hl.borrow()).iter().enumerate() {
            m.write_u8(dst + k as u32, *b);
        }
        (Call::BigRead(h, dst), 0)
    }));
    stub(&mut r, 0x23be98, Box::new(|_, _| (Call::BigOpen, 0)));
    stub(&mut r, 0x23bea0, Box::new(|_, _| (Call::BigReset, 0)));
    // the game's constant regions: names and their strings, the path formats, the none key
    let mut consts = Vec::new();
    let mut names = vec![0u8; 4 * d::NFILE as usize];
    r.mem.read(l::NAMES, &mut names).unwrap();
    let ptrs: Vec<u32> = names.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).filter(|&p| p != 0).collect();
    let (lo, hi) = (*ptrs.iter().min().unwrap(), *ptrs.iter().max().unwrap() + 0x20);
    for (at, len) in [(l::NAMES, 4 * d::NFILE), (lo, hi - lo), (l::PATH_FORMAT, 0x20), (l::BIG_PATH, 0x18), (a::NO_KEY, 4)] {
        let mut b = vec![0u8; len as usize];
        r.mem.read(at, &mut b).unwrap();
        consts.push((at, b));
    }
    let mut g = Rng(0x1589_0000_0000_0001);
    let mut counts = [0usize; 10];
    for case in 0..8_000u32 {
        let mut m = Mem {
            regions: vec![
                (ARENA, vec![0; SIZE as usize]),
                (d::BANK, vec![0; BANK_LEN as usize]),
                (a::KEY_TABLE, vec![0; 4 * 0x584]),
                (d::HASHES, vec![0; 4 * 0x584]),
                (l::CAREER + 0x27dc, vec![0; ROSTER_LEN as usize]),
            ],
        };
        m.regions.extend(consts.iter().cloned());
        for k in 0..0x800 {
            m.w32(ARENA + 4 * k, g.next());
        }
        // (now and then a zero hash among them)
        let hashes: Vec<u32> = (0..0x584).map(|_| if g.below(400) == 0 { 0 } else { g.next() | 1 }).collect();
        for (i, h) in hashes.iter().enumerate() {
            m.w32(d::HASHES + 4 * i as u32, *h);
            m.w32(a::KEY_TABLE + 4 * i as u32, g.next());
        }
        *hashes_live.borrow_mut() = hashes.clone();
        let b = d::BANK;
        for k in 0..BANK_LEN / 4 {
            m.w32(b + 4 * k, g.next());
        }
        m.w32(b, BLOCK);
        m.w32(b + 4, BLOCK + 0x80 * g.below(4));
        m.w32(b + 8, g.below(0x200));
        m.w32(b + 0x10, g.below(4));
        m.w32(b + 0x14, g.below(2));
        m.w32(b + 0x710, g.below(2));
        m.w32(b + 0x710 + 0x10, BIGVT);
        for (o, f) in [(0x80u32, V_REQUEST), (0x60, V_READY), (0x58, V_CLOSE)] {
            m.w32(BIGVT + o, 0);
            m.w32(BIGVT + o + 4, f);
        }
        // the files: loaded (an image in the arena), asked for (often only a few, so that all can be in), or neither
        let few = [4u32, 200, 2000][g.below(3) as usize];
        for i in 0..d::NFILE {
            let f = b + d::FILES + 0x18 * i;
            m.w32(f, b);
            m.w32(b + 0x18 + 4 * i, if g.below(few) == 0 { 1 } else { 0 });
            match g.below(4) {
                0 => {
                    let h = g.next();
                    let at = DATA + 0x200 * g.below(8);
                    for (k, x) in image(h, &hashes).iter().enumerate() {
                        m.w8(at + k as u32, *x);
                    }
                    // (sometimes a clip count with the sign bit set)
                    if g.below(16) == 0 {
                        m.w16(at + 2, 0x8000 | g.next() as u16);
                    }
                    m.w32(f + 4, at);
                    m.w32(f + 8, at + 0xc);
                }
                _ => m.w32(f + 4, 0),
            }
            m.w32(f + 0x14, g.next());
        }
        // the roster: humans and computers, repeats
        let n = g.below(9);
        m.w32(l::CAREER + 0x27dc, n);
        for i in 0..8 {
            let row = l::CAREER + 0x27dc + 0x84 * i;
            m.w8(row + 0x69, if g.below(2) == 0 { g.below(4) as u8 } else { 0xff });
            m.w32(row + 0x3c, if g.below(4) == 0 { g.below(14) } else { g.below(3) });
            m.w32(row + 0x48, g.below(4));
        }
        for k in 0..4 {
            m.w32(LIST + 4 * k, g.below(d::NFILE));
        }
        for (at, bts) in &m.regions[..5] {
            r.mem.write(*at, bts).unwrap();
        }
        let file_no = g.below(d::NFILE);
        let f = b + d::FILES + 0x18 * file_no;
        let big = b + 0x710;
        let now = g.below(2) == 1;
        let kind = g.below(10) as usize;
        log.borrow_mut().clear();
        let mut ours = Ours { log: Vec::new(), hashes: hashes.clone() };
        let mut ans: Option<u32> = None;
        let (func, args): (u32, Vec<u32>) = match kind {
            0 => {
                let data = DATA + 0x200 * g.below(8);
                l::bind(&mut m, f, data);
                (0x158c10, vec![f, data])
            }
            1 => {
                l::load_now(&mut m, &mut ours, f, file_no);
                (0x158908, vec![f, file_no])
            }
            2 => {
                l::load_async(&mut m, &mut ours, f, big, file_no);
                (0x1589b8, vec![f, big, file_no])
            }
            3 => {
                ans = Some(l::poll_file(&mut m, &mut ours, f, big) as u32);
                (0x158a30, vec![f, big])
            }
            4 => {
                m.w32(KEY, file_no | g.next() << 16);
                r.mem.write(ARENA, &m.regions[0].1).unwrap();
                l::index_clip_names(&mut m, f, KEY);
                (0x158b10, vec![f, KEY])
            }
            5 => {
                l::index_all(&mut m, b);
                (0x1597d0, vec![b])
            }
            6 => {
                l::load_file(&mut m, &mut ours, b, file_no, now);
                (0x159738, vec![b, file_no, now as u32])
            }
            7 => {
                ans = Some(l::poll(&mut m, &mut ours, b) as u32);
                (0x158fc0, vec![b])
            }
            8 => {
                l::load_clip_table(&mut m, &mut ours, b, now);
                (0x159150, vec![b, now as u32])
            }
            _ => {
                let (mode, cnt) = (g.below(4), g.below(5));
                l::open_big(&mut m, &mut ours, b, mode, now, LIST, cnt as i32);
                (0x158ea0, vec![b, mode, now as u32, LIST, cnt])
            }
        };
        counts[kind] += 1;
        for (i, v) in args.iter().enumerate() {
            r.cpu.set_gpr(4 + i, *v as i32 as i64 as u64);
        }
        r.call(func).unwrap_or_else(|e| panic!("case {case}: {func:#x}: {e}"));
        assert_eq!(*log.borrow(), ours.log, "case {case}: {func:#x} calls");
        if let Some(v) = ans {
            assert_eq!(r.cpu.gpr(2) as u32, v, "case {case}: {func:#x} answer");
        }
        for (at, bts) in &m.regions[..5] {
            let mut theirs = vec![0u8; bts.len()];
            r.mem.read(*at, &mut theirs).unwrap();
            if &theirs != bts {
                let i = theirs.iter().zip(bts).position(|(x, y)| x != y).unwrap();
                panic!("case {case}: {func:#x} memory at {:#x}: theirs {:#x} ours {:#x}", at + i as u32, theirs[i], bts[i]);
            }
        }
    }
    println!("{counts:?}");
}
