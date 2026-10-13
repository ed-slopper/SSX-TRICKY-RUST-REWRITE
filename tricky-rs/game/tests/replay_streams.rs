//! The replay's byte and pad streams (board row G12a) against the game in the function runner (TRICKY_ELF; skipped
//! without it): random runs of every `Replay_*` stream call and `ReplayInput_*` call on one stream and one pad
//! stream, the stream's buffer and pointers, the pad stream's bytes, the replay control and every node touched
//! compared after each call.

use r5900::Runner;
use tricky_game::replay::{self, Control, Input, Refs, Stream};

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

/// The stand-in for `Huff_DecodeBlock` 0x240ef8(src, dst) on both sides: the first byte (under 32) the length, the
/// bytes after it xor 0x5a
fn decode(src: &[u8], dst: &mut [u8]) -> usize {
    let n = (src[0] % 32) as usize;
    for k in 0..n {
        dst[k] = src[1 + k] ^ 0x5a;
    }
    n
}

const SIZE: usize = 256;
const COUNT: i32 = 8;

#[test]
fn replay_streams_are_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    // EAMem_Set / _Copy, Course_GetInstance and the stream calls calling each other
    for f in [0x2cc808, 0x2ce114, 0x254a28, 0x192738, 0x192880, 0x1928f0, 0x192998, 0x192bf0, 0x192c10, 0x192ca0, 0x241e28] {
        r.allow(f);
    }
    r.stub(0x2ccf70, |c, m| {
        let a = m.alloc(c.gpr(5) as u32);
        c.set_gpr(2, a as u64)
    });
    r.stub(0x14a040, |_, _| {});
    r.stub(0x240ef8, |c, m| {
        let (src, dst) = (c.gpr(4) as u32, c.gpr(5) as u32);
        let mut b = [0u8; 33];
        m.read(src, &mut b).unwrap();
        let mut out = [0u8; 32];
        let n = decode(&b, &mut out);
        m.write(dst, &out[..n]).unwrap();
        c.set_gpr(2, n as u64)
    });
    let app = r.mem.alloc(0x800);
    r.mem.write_u32(0x337c58, app);
    let game = r.mem.alloc(0x100);
    r.mem.write_u32(app + 0x730, game);
    let ctl = r.mem.alloc(0x80);
    r.mem.write_u32(game + 0x40, ctl);
    let world = r.mem.alloc(0x60);
    let inst = r.mem.alloc(0x100 * COUNT as u32);
    r.mem.write_u32(0x34648c, world);
    r.mem.write_u32(world + 0xc, COUNT as u32);
    r.mem.write_u32(world + 0x48, inst);
    let data = 0x0040_0000u32;
    r.mem.write_u32(0x34649c, data);
    let refs = Refs { instances: inst, count: COUNT, data };
    let sp = r.mem.alloc(16);
    let ip = r.mem.alloc(0x800c);
    let src = r.mem.alloc(0x400);
    let cur = r.mem.alloc(4);
    let buf = r.mem.alloc(64);
    let call = |r: &mut Runner, f: u32, args: &[u64]| -> u64 {
        for (k, a) in args.iter().enumerate() {
            // 32-bit values sign-extended, as the game's registers hold them
            r.cpu.set_gpr(4 + k, *a as u32 as i32 as i64 as u64);
        }
        r.call(f).unwrap_or_else(|e| panic!("{f:#x}: {e:?}"));
        r.cpu.gpr(2)
    };
    let read_ctl = |r: &mut Runner| Control {
        stream_size: r.mem.read_u32(ctl + 0xc) as i32,
        mode: r.mem.read_u32(ctl + 0x20),
        full_timer: r.mem.read_u32(ctl + 0x3c),
        input_full: r.mem.read_u32(ctl + 0x40),
        x4c: r.mem.read_u32(ctl + 0x4c),
        x50: r.mem.read_u32(ctl + 0x50),
        overflow: r.mem.read_u32(ctl + 0x58),
    };
    let write_ctl = |r: &mut Runner, c: &Control| {
        r.mem.write_u32(ctl + 0xc, c.stream_size as u32);
        r.mem.write_u32(ctl + 0x20, c.mode);
        r.mem.write_u32(ctl + 0x3c, c.full_timer);
        r.mem.write_u32(ctl + 0x40, c.input_full);
        r.mem.write_u32(ctl + 0x4c, c.x4c);
        r.mem.write_u32(ctl + 0x50, c.x50);
        r.mem.write_u32(ctl + 0x58, c.overflow);
    };
    let input_bytes = |inp: &Input| {
        let mut b = Vec::with_capacity(0x800c);
        for w in &inp.words {
            b.extend_from_slice(&w.to_le_bytes());
        }
        b.extend_from_slice(&inp.runs);
        for v in [inp.count, inp.pos, inp.left] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    };
    let mut g = Rng(0x9e37_79b9_7f4a_7c15);
    // EAMem_Set at every alignment and size up to 150
    let area = r.mem.alloc(0x200);
    for k in 0..4000u32 {
        let (off, n, v) = (g.below(16), g.below(150) as usize, g.next());
        let mut junk = vec![0u8; 0x200];
        for b in junk.iter_mut() {
            *b = g.next() as u8;
        }
        r.mem.write(area, &junk).unwrap();
        call(&mut r, 0x2cc808, &[(area + off) as u64, v as u64, n as u64]);
        replay::ea_mem_set(&mut junk[off as usize..off as usize + n], area + off, v);
        let mut gb = vec![0u8; 0x200];
        r.mem.read(area, &mut gb).unwrap();
        assert!(gb == junk, "EAMem_Set case {k}: at +{off}, {n} bytes of {v:#x}");
    }
    // a new stream's fill, at every size up to 40
    for size in 0..40usize {
        call(&mut r, 0x192b38, &[sp as u64, size as u64]);
        let mut gb = vec![0u8; size];
        let at = r.mem.read_u32(sp);
        r.mem.read(at, &mut gb).unwrap();
        assert_eq!(gb, Stream::alloc(size).buf, "fill of {size}");
    }
    let mut seen = [0u32; 24];
    // pad streams filled, full runs recorded over, streams overflowed
    let mut hits = [0u32; 3];
    for trial in 0..200 {
        let mut c = Control { stream_size: (SIZE as u32 - g.below(64)) as i32, mode: g.below(6), ..Default::default() };
        write_ctl(&mut r, &c);
        call(&mut r, 0x192b38, &[sp as u64, SIZE as u64]);
        let mut s = Stream::alloc(SIZE);
        // the input stream from random bytes, then initialised
        let mut inp = Input::default();
        let mut junk = vec![0u8; 0x800c];
        for b in junk.iter_mut() {
            *b = g.next() as u8;
        }
        r.mem.write(ip, &junk).unwrap();
        for k in 0..replay::INPUT_WORDS {
            inp.words[k] = u32::from_le_bytes(junk[4 * k..4 * k + 4].try_into().unwrap());
        }
        inp.runs.copy_from_slice(&junk[0x6664..0x8000]);
        call(&mut r, 0x1926f0, &[ip as u64]);
        inp.reset();
        for step in 0..3000 {
            let what = format!("trial {trial} step {step}");
            let op = g.below(24) as usize;
            seen[op] += 1;
            let base = r.mem.read_u32(sp);
            let room = |s: &Stream, n: usize| s.read + n <= SIZE;
            match op {
                0 | 1 | 2 => {
                    let n = g.below(24) as usize;
                    let b: Vec<u8> = (0..n).map(|_| g.next() as u8).collect();
                    if s.write + n > SIZE && c.overflow == 0 && c.stream_size - s.write as i32 >= n as i32 {
                        continue;
                    }
                    r.mem.write(buf, &b).unwrap();
                    call(&mut r, 0x192c10, &[sp as u64, buf as u64, n as u64]);
                    let was = c.overflow;
                    s.write(&mut c, &b);
                    hits[2] += (was == 0 && c.overflow != 0) as u32;
                }
                3 => {
                    let n = g.below(24) as usize;
                    if !room(&s, n) {
                        continue;
                    }
                    call(&mut r, 0x192ca0, &[sp as u64, buf as u64, n as u64]);
                    let mut b = vec![0u8; n];
                    s.read(&mut b);
                    let mut gb = vec![0u8; n];
                    r.mem.read(buf, &mut gb).unwrap();
                    assert_eq!(gb, b, "{what}: read");
                }
                4 => {
                    let n = g.below(24) as usize;
                    if !room(&s, n) {
                        continue;
                    }
                    call(&mut r, 0x192ce8, &[sp as u64, buf as u64, n as u64]);
                    let mut b = vec![0u8; n];
                    s.peek(&mut b);
                    let mut gb = vec![0u8; n];
                    r.mem.read(buf, &mut gb).unwrap();
                    assert_eq!(gb, b, "{what}: peek");
                }
                5 => {
                    if !room(&s, 12) {
                        continue;
                    }
                    let v = call(&mut r, 0x192d20, &[sp as u64]) as u32;
                    assert_eq!(v, s.peek_node_type(), "{what}: node type");
                }
                6 => {
                    call(&mut r, 0x192bf0, &[sp as u64]);
                    s.rewind_read();
                }
                7 => {
                    call(&mut r, 0x192c00, &[sp as u64]);
                    s.reset_write();
                }
                8 | 9 => {
                    let node = match g.below(4) {
                        0 => 0,
                        _ => inst + (g.below(COUNT as u32 + 2) << 8) + g.below(3) * 0x40,
                    };
                    if s.write + 8 > SIZE && c.overflow == 0 && c.stream_size - (s.write as i32) >= 4 {
                        continue;
                    }
                    if op == 8 {
                        call(&mut r, 0x192f30, &[0, node as u64, sp as u64]);
                        replay::write_node_ref(&mut s, &mut c, &refs, node);
                    } else {
                        let owner = g.next();
                        let (e4, e8) = (if g.below(2) == 0 { owner } else { g.next() }, g.next() as u16);
                        if node != 0 && node + 0xec <= inst + 0x100 * COUNT as u32 {
                            r.mem.write_u32(node + 0xe4, e4);
                            r.mem.write_u16(node + 0xe8, e8);
                        }
                        let (e4, e8) = if node != 0 && node + 0xec <= inst + 0x100 * COUNT as u32 {
                            (e4, e8)
                        } else if node != 0 {
                            continue;
                        } else {
                            (0, 0)
                        };
                        if s.write + 8 > SIZE && c.overflow == 0 && c.stream_size - (s.write as i32) >= 8 {
                            continue;
                        }
                        call(&mut r, 0x192f78, &[0, owner as u64, node as u64, sp as u64]);
                        replay::write_link(&mut s, &mut c, &refs, owner, node, e4, e8);
                    }
                }
                10 => {
                    if !room(&s, 4) {
                        continue;
                    }
                    let v = call(&mut r, 0x192df0, &[0, sp as u64]) as u32;
                    assert_eq!(v, replay::read_node_ref(&mut s, &refs), "{what}: node ref");
                }
                11 => {
                    if !room(&s, 8) {
                        continue;
                    }
                    let owner = g.next();
                    let mut b = [0u8; 8];
                    let at = s.read;
                    b.copy_from_slice(&s.buf[at..at + 8]);
                    let i = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    let node_before = if (0..COUNT).contains(&i) { Some(inst + ((i as u32) << 8)) } else { None };
                    let before = node_before.map(|n| (r.mem.read_u32(n + 0xe4), r.mem.read_u32(n + 0xe8)));
                    let v = call(&mut r, 0x192e40, &[0, owner as u64, sp as u64]) as u32;
                    let (node, set) = replay::read_link(&mut s, &refs);
                    assert_eq!(v, node, "{what}: link");
                    if let (Some(n), Some((e4, e8))) = (node_before, before) {
                        let (e4, e8) = match set {
                            Some(w) => (owner, e8 & 0xffff_0000 | w as u32),
                            None => (e4, e8),
                        };
                        assert_eq!(r.mem.read_u32(n + 0xe4), e4, "{what}: link owner");
                        assert_eq!(r.mem.read_u32(n + 0xe8), e8, "{what}: link word");
                    }
                }
                12 | 13 => {
                    let addr = data.wrapping_add(g.next() >> g.below(32));
                    if s.write + 4 > SIZE && c.overflow == 0 && c.stream_size - (s.write as i32) >= 4 {
                        continue;
                    }
                    call(&mut r, if op == 12 { 0x193020 } else { 0x192fe8 }, &[0, addr as u64, sp as u64]);
                    replay::write_offset(&mut s, &mut c, &refs, addr);
                }
                14 | 15 => {
                    if !room(&s, 4) {
                        continue;
                    }
                    let v = call(&mut r, if op == 14 { 0x192ec0 } else { 0x192ef8 }, &[0, sp as u64]) as u32;
                    assert_eq!(v, replay::read_offset(&mut s, &refs), "{what}: offset");
                }
                16 => {
                    let mut b = vec![0u8; 0x40];
                    for x in b.iter_mut() {
                        *x = g.next() as u8;
                    }
                    let size = g.below(40) as i32;
                    b[0..4].copy_from_slice(&size.to_le_bytes());
                    r.mem.write(src, &b).unwrap();
                    r.mem.write_u32(cur, src);
                    let v = call(&mut r, 0x192d60, &[sp as u64, cur as u64, src as u64]);
                    let mut cursor = 0usize;
                    let ok = s.unpack_block(&b, &mut cursor, &mut decode);
                    assert_eq!(v, ok as u64, "{what}: unpack");
                    assert_eq!(r.mem.read_u32(cur), src + cursor as u32, "{what}: unpack cursor");
                }
                17 => {
                    if g.below(20) != 0 {
                        continue;
                    }
                    call(&mut r, 0x192ba8, &[sp as u64]);
                    s.free();
                    assert_eq!([r.mem.read_u32(sp), r.mem.read_u32(sp + 4), r.mem.read_u32(sp + 8), r.mem.read_u32(sp + 12)], [0; 4], "{what}: free");
                    c.overflow = 0;
                    write_ctl(&mut r, &c);
                    call(&mut r, 0x192b38, &[sp as u64, SIZE as u64]);
                    s = Stream::alloc(SIZE);
                }
                // the pad stream
                18 => {
                    let frame = g.below(600);
                    let v = call(&mut r, 0x192758, &[ip as u64, frame as u64]);
                    assert_eq!(v, inp.seek_frame(frame) as u64, "{what}: seek");
                }
                19 | 20 => {
                    let pad = if inp.count > 0 && g.below(2) == 0 {
                        inp.words[(inp.count - 1) as usize]
                    } else {
                        [0u32, 1, 0x8000, g.next()][g.below(4) as usize]
                    };
                    let before = (c.input_full, c.overflow, inp.count > 0 && inp.runs[(inp.count - 1) as usize] == 0xff);
                    r.mem.write_u32(buf, pad);
                    let mut p = pad;
                    let v = call(&mut r, 0x1927d8, &[ip as u64, buf as u64]);
                    assert_eq!(v, replay::record_or_play(&mut c, &mut inp, &mut p) as u64, "{what}: record or play");
                    assert_eq!(r.mem.read_u32(buf), p, "{what}: pad");
                    hits[0] += (before.0 == 0 && c.input_full != 0) as u32;
                    hits[1] += (before.2 && c.mode == 0) as u32;
                }
                21 => {
                    // the controls a frame may change
                    c.mode = [0, 0, 1, 2, 3, 4, 5, 13, 14][g.below(9) as usize];
                    c.x4c = (g.below(8) == 0) as u32;
                    c.x50 = (g.below(8) == 0) as u32;
                    if g.below(4) == 0 {
                        c.input_full = 0;
                    }
                    if g.below(30) == 0 {
                        inp.count = [0x1990, 0x1998, 0x1999, g.below(0x1999) as i32][g.below(4) as usize];
                        inp.pos = g.below(inp.count as u32 + 2) as i32;
                        inp.left = g.below(4) as i32;
                        r.mem.write_u32(ip + 0x8000, inp.count as u32);
                        r.mem.write_u32(ip + 0x8004, inp.pos as u32);
                        r.mem.write_u32(ip + 0x8008, inp.left as u32);
                        // the last run nearly or quite full
                        if inp.count > 0 {
                            let last = (inp.count - 1) as usize;
                            inp.runs[last] = [0xfe, 0xff, 1][g.below(3) as usize];
                            r.mem.write_u8(ip + 0x6664 + last as u32, inp.runs[last]);
                            let w = inp.words[last];
                            r.mem.write_u32(ip + 4 * last as u32, w);
                            r.mem.write_u32(buf, w);
                        }
                    }
                    write_ctl(&mut r, &c);
                }
                22 => {
                    let mut b = vec![0u8; 0x80];
                    for x in b.iter_mut() {
                        *x = g.next() as u8;
                    }
                    b[4..8].copy_from_slice(&(g.below(0x1999) as i32).to_le_bytes());
                    let s1 = g.below(40) as i32;
                    b[8..12].copy_from_slice(&s1.to_le_bytes());
                    let at = 12 + s1 as usize;
                    b[at..at + 4].copy_from_slice(&(g.below(40) as i32).to_le_bytes());
                    r.mem.write(src, &b).unwrap();
                    r.mem.write_u32(cur, src);
                    let v = call(&mut r, 0x192a40, &[ip as u64, cur as u64]);
                    let mut cursor = 0usize;
                    assert_eq!(v, inp.load(&b, &mut cursor, &mut decode) as u64, "{what}: input load");
                    assert_eq!(r.mem.read_u32(cur), src + cursor as u32, "{what}: input cursor");
                }
                _ => {
                    call(&mut r, 0x192738, &[ip as u64]);
                    inp.reset();
                }
            }
            assert_eq!(read_ctl(&mut r), c, "{what} op {op}: control");
            let base = if op == 17 { r.mem.read_u32(sp) } else { base };
            assert_eq!(r.mem.read_u32(sp), base, "{what}: base");
            assert_eq!(r.mem.read_u32(sp + 4).wrapping_sub(base) as usize, s.write, "{what} op {op}: write");
            assert_eq!(r.mem.read_u32(sp + 8).wrapping_sub(base) as usize, s.read, "{what} op {op}: read");
            assert_eq!(r.mem.read_u32(sp + 12) as i32, s.written, "{what} op {op}: written");
            for (k, v) in [inp.count, inp.pos, inp.left].iter().enumerate() {
                assert_eq!(r.mem.read_u32(ip + 0x8000 + 4 * k as u32) as i32, *v, "{what} op {op}: input word {k}");
            }
            if step % 100 == 99 {
                let mut gb = vec![0u8; SIZE];
                r.mem.read(base, &mut gb).unwrap();
                if let Some(k) = (0..SIZE).find(|&k| gb[k] != s.buf[k]) {
                    panic!("{what}: the stream's buffer at {k}: {:x?} vs {:x?}", &gb[k..(k + 16).min(SIZE)], &s.buf[k..(k + 16).min(SIZE)]);
                }
                let mut gi = vec![0u8; 0x800c];
                r.mem.read(ip, &mut gi).unwrap();
                assert!(gi == input_bytes(&inp), "{what}: the pad stream");
            }
        }
    }
    eprintln!("ops {seen:?} hits {hits:?}");
    assert!(hits.iter().all(|&h| h > 0));
}
