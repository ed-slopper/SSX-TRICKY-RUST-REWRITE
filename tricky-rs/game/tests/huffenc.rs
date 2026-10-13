//! The replay's Huffman encoder (board row G12c) against the game in the function runner (TRICKY_ELF; skipped
//! without it): `Huff_Encode` on contexts full of leftovers (the output, its length and every context byte
//! compared), `Compress_Block` with its delta modes, `Huff_Analyze` with other flags and code limits, and every
//! stream decoded back by `huff_decode_block`.

use r5900::Runner;
use tricky_data::compress::huff_decode_block;
use tricky_data::huffenc::{self, at, Encoder, Out};

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

/// Bytes like a replay's: few values, runs (some past 0xfc and past 0x7531), ramps, noise
fn sample(g: &mut Rng, n: usize) -> Vec<u8> {
    let kind = g.below(6);
    let mut v = Vec::with_capacity(n);
    let alphabet: Vec<u8> = (0..1 + g.below(40)).map(|_| g.next() as u8).collect();
    while v.len() < n {
        match kind {
            0 => v.push(g.next() as u8),
            1 => v.push(alphabet[g.below(alphabet.len() as u32) as usize]),
            2 => {
                let b = alphabet[g.below(alphabet.len() as u32) as usize];
                let r = [1, 2, 3, 5, 20, 300, 1000][g.below(7) as usize];
                v.extend(std::iter::repeat_n(b, r));
            }
            3 => {
                let s = g.next() as u8;
                let d = [1u8, 2, 255, 254][g.below(4) as usize];
                for k in 0..g.below(200) {
                    v.push(s.wrapping_add(d.wrapping_mul(k as u8)));
                }
            }
            4 => {
                if g.below(4) == 0 {
                    v.extend(std::iter::repeat_n(0u8, g.below(64) as usize));
                } else {
                    v.extend((g.next() as u32 & 0x0101_0f3f).to_le_bytes());
                }
            }
            _ => {
                let w = (g.next() as f32 / 1e6).sin();
                v.push((w * 40.0) as i8 as u8);
            }
        }
    }
    v.truncate(n);
    v
}

fn sx(v: u64) -> u64 {
    v as u32 as i32 as i64 as u64
}

struct Game {
    r: Runner,
    ctx: u32,
    src: u32,
    dst: u32,
    out: u32,
    args: u32,
}

const SRC_MAX: usize = 0x30000;
const DST_MAX: usize = 0x40000;

fn runner() -> Option<Game> {
    let mut r = Runner::from_env()?;
    for a in [0x241e48, 0x241e88, 0x241f78, 0x241ff8, 0x242210, 0x242320, 0x2424d0, 0x242548, 0x242578, 0x2426f8, 0x243368, 0x2438c0] {
        r.allow(a);
    }
    let ctx = r.mem.alloc(at::LEN as u32);
    let src = r.mem.alloc(SRC_MAX as u32);
    let dst = r.mem.alloc(DST_MAX as u32);
    let out = r.mem.alloc(8);
    let args = r.mem.alloc(16);
    Some(Game { r, ctx, src, dst, out, args })
}

fn call(r: &mut Runner, f: u32, args: &[u64]) -> u64 {
    for (k, a) in args.iter().enumerate() {
        r.cpu.set_gpr(4 + k, sx(*a));
    }
    r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e:?}"));
    r.cpu.gpr(2)
}

fn junk(g: &mut Rng) -> Vec<u8> {
    match g.below(3) {
        0 => vec![0; at::LEN],
        1 => vec![0xcd; at::LEN],
        _ => (0..at::LEN).map(|_| g.next() as u8).collect(),
    }
}

/// `Huff_Encode` in the runner and ours from the same context bytes
fn encode_both(gm: &mut Game, g: &mut Rng, data: &[u8], unpacked: u32, mode: i64, what: &str) -> Vec<u8> {
    let start = junk(g);
    let r = &mut gm.r;
    r.mem.write(gm.ctx, &start).unwrap();
    r.mem.write(gm.src, data).unwrap();
    r.mem.write_u32(gm.args, gm.src);
    r.mem.write_u32(gm.args + 4, data.len() as u32);
    r.mem.write_u32(gm.out, gm.dst);
    r.mem.write_u32(gm.out + 4, 0x5555);
    let n = call(r, 0x2438c0, &[gm.ctx as u64, gm.args as u64, gm.out as u64, unpacked as u64, mode as u64]) as u32;
    let mut e = Encoder::new(start, data, gm.src);
    let mut out = Out::default();
    let ours = e.encode(&mut out, unpacked, mode);
    let mut ctx = vec![0u8; at::LEN];
    r.mem.read(gm.ctx, &mut ctx).unwrap();
    let diffs: Vec<String> = (0..at::LEN / 4)
        .filter(|&w| ctx[4 * w..4 * w + 4] != e.ctx[4 * w..4 * w + 4])
        .take(16)
        .map(|w| format!("+{:#x}: {:#x} vs {:#x}", 4 * w, u32::from_le_bytes(ctx[4 * w..4 * w + 4].try_into().unwrap()), e.g(4 * w)))
        .collect();
    assert!(diffs.is_empty(), "{what}: the context differs: {diffs:?}");
    assert_eq!(n, ours, "{what}: bytes written");
    assert_eq!(r.mem.read_u32(gm.out + 4), out.pos, "{what}: out pos");
    let mut theirs = vec![0u8; n as usize];
    r.mem.read(gm.dst, &mut theirs).unwrap();
    assert!(theirs == out.buf[..n as usize], "{what}: the stream differs at {:?}", (0..n as usize).find(|&k| theirs[k] != out.buf[k]));
    let mut ctx = vec![0u8; at::LEN];
    r.mem.read(gm.ctx, &mut ctx).unwrap();
    if let Some(k) = (0..at::LEN).find(|&k| ctx[k] != e.ctx[k]) {
        panic!("{what}: the context differs at +{k:#x}: {:x?} vs {:x?}", &ctx[k & !3..(k & !3) + 4], &e.ctx[k & !3..(k & !3) + 4]);
    }
    theirs
}

#[test]
fn huff_encode_is_the_original() {
    let Some(mut gm) = runner() else { return };
    let mut g = Rng(0x4875_6645_6e63_0001);
    let mut bytes = 0usize;
    let mut empty = 0;
    for case in 0..1200 {
        let n = [0usize, 1, 2, 3, 10, 100, 700, 3000, 20000][g.below(9) as usize];
        let data = sample(&mut g, n);
        let mode = [0i64, 0, 1, 2, 3, -1][g.below(6) as usize];
        let unpacked = if g.below(5) == 0 { data.len() as u32 + g.below(9) } else { data.len() as u32 };
        let what = format!("case {case}: {n} bytes, mode {mode}");
        let s = encode_both(&mut gm, &mut g, &data, unpacked, mode, &what);
        if n == 0 {
            if mode != 0 || unpacked != 0 {
                continue;
            }
            // an empty block has one code (the clue, one bit long): the decoder keeps reading code lengths past
            // the stream, in the game too
            let r = &mut gm.r;
            let mut tail = s.clone();
            tail.resize(SRC_MAX, 0);
            r.mem.write(gm.src, &tail).unwrap();
            r.cpu.set_gpr(4, gm.src as u64);
            r.cpu.set_gpr(5, gm.dst as u64);
            r.allow(0x240ef8);
            let res = r.call(0x240ef8);
            assert!(matches!(res, Err(r5900::Error::TooLong { .. })), "{what}: the game decodes an empty block: {res:?}, {:#x}", r.cpu.gpr(2));
            empty += 1;
        } else if unpacked == data.len() as u32 && (0..=2).contains(&mode) {
            let mut back = vec![];
            let size = huff_decode_block(&s, &mut back);
            assert_eq!(size as usize, data.len(), "{what}: decoded size");
            if mode == 0 {
                assert!(back == data, "{what}: round trip");
            }
        }
        bytes += s.len();
    }
    // long runs (past 0xfc, the 30001-byte limit) and every byte value
    for (case, run) in [0xfbusize, 0xfc, 0xfd, 1000, 30000, 30001, 30002, 70000].into_iter().enumerate() {
        let mut data = sample(&mut g, 300);
        data.extend(std::iter::repeat_n(g.next() as u8, run));
        data.extend((0..=255u8).collect::<Vec<_>>());
        let what = format!("long {case}: run {run}");
        let s = encode_both(&mut gm, &mut g, &data, data.len() as u32, 0, &what);
        let mut back = vec![];
        huff_decode_block(&s, &mut back);
        assert!(back == data, "{what}: round trip");
        bytes += s.len();
    }
    eprintln!("{bytes} stream bytes compared ({empty} empty blocks)");
}

#[test]
fn compress_block_is_the_original() {
    let Some(mut gm) = runner() else { return };
    let alloc = std::rc::Rc::new(std::cell::RefCell::new(Vec::<(u32, u32)>::new()));
    let a2 = alloc.clone();
    let fill = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let f2 = fill.clone();
    gm.r.stub(0x2cc958, move |c, m| {
        let n = c.gpr(4) as u32;
        let p = m.alloc(n);
        m.write(p, &vec![f2.get(); n as usize]).unwrap();
        a2.borrow_mut().push((p, n));
        c.set_gpr(2, p as u64)
    });
    gm.r.stub(0x2cc980, |_, _| {});
    gm.r.allow(0x243a60);
    let mut g = Rng(0x4875_6645_6e63_0002);
    for case in 0..400 {
        let n = [1usize, 2, 50, 600, 4000][g.below(5) as usize];
        let data = sample(&mut g, n);
        let opt = [None, Some(0), Some(1), Some(2), Some(3)][g.below(5) as usize];
        fill.set(g.next() as u8);
        alloc.borrow_mut().clear();
        let r = &mut gm.r;
        r.mem.write(gm.src, &data).unwrap();
        if let Some(m) = opt {
            r.mem.write_u32(gm.args, m as u32);
        }
        let ret = call(r, 0x243a60, &[gm.src as u64, n as u64, gm.dst as u64, if opt.is_some() { gm.args as u64 } else { 0 }]) as u32;
        let ctx = vec![fill.get(); at::LEN];
        // the bytes encoded are a copy for the delta modes (its address is the encoder's +0x2db0)
        let src_addr = alloc.borrow().iter().find(|a| a.1 == n as u32).map_or(gm.src, |a| a.0);
        let mut out = Out::default();
        let ours = huffenc::compress_block(&data, src_addr, &mut out, opt, ctx);
        let what = format!("case {case}: {n} bytes, options {opt:?}");
        assert_eq!(ret, ours, "{what}: bytes written");
        let mut theirs = vec![0u8; ret as usize];
        r.mem.read(gm.dst, &mut theirs).unwrap();
        assert!(theirs == out.buf[..ret as usize], "{what}: the stream");
        let mut back = vec![];
        huff_decode_block(&theirs, &mut back);
        if matches!(opt, None | Some(0..=2)) {
            assert!(back == data, "{what}: round trip");
        }
    }
}

#[test]
fn huff_analyze_is_the_original() {
    let Some(mut gm) = runner() else { return };
    let mut g = Rng(0x4875_6645_6e63_0003);
    let mut done = 0;
    for case in 0..1500 {
        let n = [1usize, 5, 100, 700, 3000][g.below(5) as usize];
        let data = sample(&mut g, n);
        let flags = [0x39u32, 0x38, 0x3b, 0x3d, 0x31, 0x19, 0x30, 0x00, 0x3f, g.below(0x40)][g.below(10) as usize];
        let max_len = [0xfu32, 0xf, 8, 9, 12, 16, 99, 100][g.below(8) as usize];
        let mut start = junk(&mut g);
        // the fields Huff_Encode sets first
        let mut e0 = Encoder::new(start.clone(), &data, gm.src);
        e0.s(at::SRC, gm.src);
        e0.s(at::END, gm.src + n as u32);
        e0.s(at::SIZE2, n as u32);
        e0.init_run_costs();
        start = e0.ctx.clone();
        let mut e = Encoder::new(start.clone(), &data, gm.src);
        let ours = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            e.analyze(flags, max_len);
            e.ctx.clone()
        }));
        let Ok(ours) = ours else { continue };
        let r = &mut gm.r;
        r.mem.write(gm.ctx, &start).unwrap();
        r.mem.write(gm.src, &data).unwrap();
        for (k, a) in [gm.ctx as u64, flags as u64, max_len as u64].iter().enumerate() {
            r.cpu.set_gpr(4 + k, sx(*a));
        }
        let mut fsp = 0u32;
        r.call_traced(0x2426f8, |pc, c| {
            if pc == 0x2426fc {
                fsp = c.gpr(29) as u32;
            }
        })
        .unwrap();
        let mut gs = vec![0u8; 0x400];
        r.mem.read(fsp, &mut gs).unwrap();
        let gs: Vec<u32> = gs.chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        if let Some(k) = (0..0x100).find(|&k| gs[k] != e.saved[k]) {
            panic!("case {case}: flags {flags:#x}: the first pass's counts differ at {k:#x}: {} vs {}", gs[k], e.saved[k]);
        }
        let mut ctx = vec![0u8; at::LEN];
        r.mem.read(gm.ctx, &mut ctx).unwrap();
        let w = |b: &[u8], i: usize| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        let diffs: Vec<String> = (0..at::LEN / 4)
            .filter(|&i| w(&ctx, i) != w(&ours, i))
            .take(16)
            .map(|i| format!("+{:#x}: {:#x} vs {:#x}", 4 * i, w(&ctx, i), w(&ours, i)))
            .collect();
        assert!(diffs.is_empty(), "case {case}: flags {flags:#x}, longest {max_len}, {n} bytes: {diffs:?}");
        done += 1;
    }
    eprintln!("{done} analyses compared");
}

#[test]
fn huff_build_tree_and_run_cost_are_the_original() {
    let Some(mut gm) = runner() else { return };
    let mut g = Rng(0x4875_6645_6e63_0004);
    for case in 0..3000 {
        let mut start = junk(&mut g);
        let used = [2u32, 3, 5, 20, 100, 256][g.below(6) as usize];
        let mut e0 = Encoder::new(start.clone(), &[], 0);
        for i in 0..0x100u32 {
            e0.s(at::COUNT + 4 * i as usize, 0);
        }
        for _ in 0..used {
            let i = g.below(256) as usize;
            let w = [1u32, 1, 2, 3, 7, 100, 5000, g.next() >> 12][g.below(8) as usize];
            e0.s(at::COUNT + 4 * i, w);
        }
        e0.init_run_costs();
        e0.s(at::CLUE, g.below(250));
        start = e0.ctx.clone();
        let mut e = Encoder::new(start.clone(), &[], 0);
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| e.build_tree())).is_err() {
            continue;
        }
        let r = &mut gm.r;
        r.mem.write(gm.ctx, &start).unwrap();
        call(r, 0x241ff8, &[gm.ctx as u64]);
        let mut ctx = vec![0u8; at::LEN];
        r.mem.read(gm.ctx, &mut ctx).unwrap();
        if let Some(k) = (0..at::LEN).find(|&k| ctx[k] != e.ctx[k]) {
            panic!("tree case {case}: the context differs at +{k:#x}: {:x?} vs {:x?}", &ctx[k & !3..(k & !3) + 4], &e.ctx[k & !3..(k & !3) + 4]);
        }
        for _ in 0..10 {
            let (n, k) = (g.below(300), g.below(9));
            let v = call(r, 0x242210, &[gm.ctx as u64, n as u64, k as u64]) as u32 as i32;
            assert_eq!(v, e.run_cost(n, k), "cost case {case}: run {n}, {k} symbols");
        }
    }
}

/// Small inputs through `Huff_Analyze` with the encoder's flags (these found the runner's LO1 bug: `mult` cleared
/// it)
#[test]
fn huff_analyze_small_inputs() {
    let Some(mut gm) = runner() else { return };
    let inputs: Vec<Vec<u8>> = vec![
        b"a".to_vec(),
        b"ab".to_vec(),
        b"aa".to_vec(),
        b"aab".to_vec(),
        b"aaab".to_vec(),
        b"abcabc".to_vec(),
        b"aaaaaaaaab".to_vec(),
        b"abbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbc".to_vec(),
        b"hello world hello world".to_vec(),
        b"aabbccddeeff".to_vec(),
        (0..=255u8).collect(),
    ];
    for (case, data) in inputs.iter().enumerate() {
        let n = data.len();
        let start = vec![0u8; at::LEN];
        let mut e0 = Encoder::new(start, data, gm.src);
        e0.s(at::SRC, gm.src);
        e0.s(at::END, gm.src + n as u32);
        e0.s(at::SIZE2, n as u32);
        e0.init_run_costs();
        let start = e0.ctx.clone();
        let mut e = Encoder::new(start.clone(), data, gm.src);
        e.analyze(0x39, 15);
        let r = &mut gm.r;
        r.mem.write(gm.ctx, &start).unwrap();
        r.mem.write(gm.src, data).unwrap();
        call(r, 0x2426f8, &[gm.ctx as u64, 0x39, 15]);
        let mut ctx = vec![0u8; at::LEN];
        r.mem.read(gm.ctx, &mut ctx).unwrap();
        let w = |b: &[u8], i: usize| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        let diffs: Vec<String> = (0..at::LEN / 4)
            .filter(|&i| w(&ctx, i) != w(&e.ctx, i))
            .take(12)
            .map(|i| format!("+{:#x}: {:#x} vs {:#x}", 4 * i, w(&ctx, i), w(&e.ctx, i)))
            .collect();
        assert!(diffs.is_empty(), "input {case} ({n} bytes): {diffs:?}");
    }
}

