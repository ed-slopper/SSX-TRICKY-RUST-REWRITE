//! The pad mapper against the original's (board row G1a5b2), in the function runner on the player's SLUS_203.26
//! (TRICKY_ELF; skipped without it): `cPS2Device` vf10 0x17b278 (the pad read), `Pad_ReadAxis` 0x179940, the four
//! `cInput` queries (0x179860, 0x179890, 0x1798c8, 0x179900) on random pads and maps; and, with
//! TRICKY_BTNMAP_DIR set to a folder holding the disc's `BTNMAP0.DAT` and `BTNMAP1.DAT`, the game's own parser
//! (`Pad_ParseButtonMap` 0x179b18) on them against `ButtonMap::btnmap`, and against our parser
//! (`tricky_game::input`, row E10b) on them and on 600 edited copies; the key repeat (`PadDevice_UpdateRepeat`
//! 0x17a958) and the rumble (`cPS2Device` vf3 / vf4 / vf5 and the motor timers of the poll) frame by frame.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::controls::{read_axis, ButtonMap, Mapper, PadDevice, PadMapper};
use tricky_game::input::{bind, parse_button_map, KeyRepeat, PadConfig, Rumble};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() % 1_000_000) as f32 / 1_000_000.0
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

fn put_device(m: &mut r5900::Mem, at: u32, d: &PadDevice) {
    m.write_u32(at + 0xc, d.prev);
    m.write_u32(at + 0x10, d.buttons);
    for (o, v) in [(0x14, d.dpad[0]), (0x18, d.dpad[1]), (0x1c, d.left[0]), (0x20, d.left[1]), (0x24, d.right[0]), (0x28, d.right[1])] {
        m.write_f32(at + o, v);
    }
}

fn get_device(m: &mut r5900::Mem, at: u32) -> PadDevice {
    let f = |m: &mut r5900::Mem, o: u32| m.read_f32(at + o);
    PadDevice {
        prev: m.read_u32(at + 0xc),
        buttons: m.read_u32(at + 0x10),
        dpad: [f(m, 0x14), f(m, 0x18)],
        left: [f(m, 0x1c), f(m, 0x20)],
        right: [f(m, 0x24), f(m, 0x28)],
    }
}

fn random_device(g: &mut Rng) -> PadDevice {
    let v = |g: &mut Rng| match g.next() % 5 {
        0 => 0.0,
        1 => [1.0, -1.0, 0.38046828, -0.38046828, 0.3804682][g.next() as usize % 5],
        _ => g.f(-1.0, 1.0),
    };
    PadDevice { prev: g.next() | g.next() << 16, buttons: g.next() | g.next() << 16, dpad: [v(g), v(g)], left: [v(g), v(g)], right: [v(g), v(g)] }
}

#[test]
fn pad_read_matches_the_original() {
    let Some(mut r) = runner() else { return };
    r.allow(0x2ce114); // EAMem_Copy
    r.stub(0x17a958, |_, _| {}); // PadDevice_UpdateRepeat
    let dev = r.mem.alloc(0x100);
    let rec = r.mem.alloc(0x40);
    let mut g = Rng(0x7ad1_3c55_09e4_1b77);
    for case in 0..20_000 {
        let mut ours = random_device(&mut g);
        put_device(&mut r.mem, dev, &ours);
        let ps2 = g.next() as u16;
        let analog = if g.next() % 4 == 0 { None } else { Some([g.next() as u8, g.next() as u8, g.next() as u8, g.next() as u8]) };
        r.mem.write_u32(rec + 0x10, 1);
        r.mem.write_u32(rec + 0x14, analog.is_some() as u32);
        r.mem.write_u16(rec + 0x18, ps2);
        let a = analog.unwrap_or([0; 4]);
        for (k, b) in a.iter().enumerate() {
            r.mem.write_u32(rec + 0x1c + 4 * k as u32, *b as u32);
        }
        r.cpu.set_gpr(4, dev as u64);
        r.cpu.set_gpr(5, rec as u64);
        r.call(0x17b278).unwrap_or_else(|e| panic!("case {case}: {e}"));
        ours.read(ps2, analog);
        let theirs = get_device(&mut r.mem, dev);
        assert_eq!(format!("{theirs:?}"), format!("{ours:?}"), "case {case}: ps2 {ps2:#06x} analog {analog:?}");
    }
}

#[test]
fn axes_and_queries_match_the_original() {
    let Some(mut r) = runner() else { return };
    let dev = r.mem.alloc(0x100);
    let inp = r.mem.alloc(0x200);
    r.mem.write_u32(inp, dev);
    let mut g = Rng(0x2f6c_a903_8811_d4e5);
    let mut nonzero = 0;
    for case in 0..30_000 {
        let d = random_device(&mut g);
        put_device(&mut r.mem, dev, &d);
        // an axis: any mix of the mask bits, often a map's own
        let mask = if g.next() % 2 == 0 { g.next() & 0x3ffff } else { [0x20020, 0x10010, 0x1000, 0x8000, 0x1001, 0x8008, 0x40, 0x200, 1, 8, 0][g.next() as usize % 11] };
        let negate = g.next() % 2 == 0;
        r.mem.write_u32(inp + 0x14c, mask);
        r.mem.write_u32(inp + 0x150, negate as u32);
        r.cpu.set_gpr(4, inp as u64);
        r.cpu.set_gpr(5, 0);
        r.call(0x179940).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let ours = read_axis(&d, mask, negate);
        assert_eq!(r.cpu.f(0).to_bits(), ours.to_bits(), "case {case}: axis mask {mask:#x} negate {negate} on {d:?}");
        nonzero += (ours != 0.0) as u32;
        // the four queries on action 0
        let pressed = [0xffff_ffff, 0, g.next() & 0x3fff, 1 << (g.next() % 16)][g.next() as usize % 4];
        let looked = pressed | [0, 0x3600, g.next() & 0xffff][g.next() as usize % 3];
        r.mem.write_u32(inp + 4, pressed);
        r.mem.write_u32(inp + 0xa8, looked);
        let mut map = ButtonMap { pressed: [0; 0x29], looked_at: [0; 0x29], axes: [(0, false); 16] };
        map.pressed[0] = pressed;
        map.looked_at[0] = looked;
        let mut m = Mapper { map: &map, pad: &d };
        for (addr, ours) in [(0x179860u32, m.q0c(0)), (0x179890, m.q14(0)), (0x1798c8, m.q1c(0)), (0x179900, m.q24(0))] {
            r.cpu.set_gpr(4, inp as u64);
            r.cpu.set_gpr(5, 0);
            r.call(addr).unwrap_or_else(|e| panic!("case {case}: {e}"));
            assert_eq!(r.cpu.gpr(2) as u32, ours, "case {case}: query {addr:#x} pressed {pressed:#x} looked {looked:#x} on {d:?}");
        }
    }
    assert!(nonzero > 1000, "axes read: {nonzero}");
}

#[test]
fn button_maps_parse_as_the_original() {
    let Some(dir) = std::env::var_os("TRICKY_BTNMAP_DIR") else { return };
    let Some(mut r) = runner() else { return };
    // the parser and the newlib it calls (sscanf); strlen's and strncmp's MMI loops are not in the runner: answered the same
    for f in [0x179b18u32, 0x179d98, 0x17a148, 0x17a208, 0x179fb0, 0x17a038, 0x17a0a0, 0x179e60, 0x2fcaa0, 0x17a1a8, 0x301090, 0x301b98] {
        r.allow(f);
    }
    // strncmp 0x2fd478 (MMI too): the C library's answer
    r.stub(0x2fd478, |c, m| {
        let (a, b, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        let byte = |m: &mut r5900::Mem, p: u32| (m.read_u32(p & !3) >> (8 * (p & 3))) as u8;
        let mut d = 0i32;
        for k in 0..n {
            let (x, y) = (byte(m, a + k), byte(m, b + k));
            if x != y || x == 0 {
                d = x as i32 - y as i32;
                break;
            }
        }
        c.set_gpr(2, d as i64 as u64);
    });
    r.stub(0x2fd260, |c, m| {
        let s = c.gpr(4) as u32;
        let mut n = 0;
        while (m.read_u32((s + n) & !3) >> (8 * ((s + n) & 3))) as u8 != 0 {
            n += 1;
        }
        c.set_gpr(2, n as u64);
    });
    for n in 0..2u32 {
        let text = std::fs::read(std::path::Path::new(&dir).join(format!("BTNMAP{n}.DAT"))).expect("BTNMAP file");
        let buf = r.mem.alloc(text.len() as u32 + 16);
        for (k, b) in text.iter().enumerate() {
            r.mem.write_u8(buf + k as u32, *b);
        }
        r.mem.write_u8(buf + text.len() as u32, 0);
        let ctx = r.mem.alloc(0x200);
        r.mem.write_u32(ctx, buf);
        r.mem.write_u32(ctx + 4, text.len() as u32);
        r.cpu.set_gpr(4, ctx as u64);
        loop {
            match r.call(0x179b18) {
                Ok(()) => break,
                Err(r5900::Error::UnstubbedCall { target, name, .. }) => panic!("allow {target:#x} ({name})"),
                Err(e) => panic!("map {n}: {e}"),
            }
        }
        // the parse context: +8/+0xc/+0x10 counts, +0x14 IS_BOOL pairs, +0x84 grab pairs, +0xfc axes
        assert_eq!((r.mem.read_u32(ctx + 8), r.mem.read_u32(ctx + 0xc), r.mem.read_u32(ctx + 0x10)), (14, 15, 16), "map {n}: counts");
        let slots: [u16; 15] = std::array::from_fn(|k| k as u16);
        let ours = ButtonMap::btnmap(n, slots);
        for k in 0..14u32 {
            let (p, o) = (r.mem.read_u32(ctx + 0x14 + 8 * k), r.mem.read_u32(ctx + 0x18 + 8 * k));
            assert_eq!((p, o | p), (ours.pressed[0x1b + k as usize], ours.looked_at[0x1b + k as usize]), "map {n}: button {k}");
        }
        for k in 0..15u32 {
            let (p, o) = (r.mem.read_u32(ctx + 0x84 + 8 * k), r.mem.read_u32(ctx + 0x88 + 8 * k));
            assert_eq!((p, o | p), (ours.pressed[k as usize], ours.looked_at[k as usize]), "map {n}: grab {k}");
        }
        for k in 0..16u32 {
            let (m, neg) = (r.mem.read_u32(ctx + 0xfc + 8 * k), r.mem.read_u32(ctx + 0x100 + 8 * k));
            assert_eq!((m, neg != 0), ours.axes[k as usize], "map {n}: axis {k}");
        }
    }
}

/// The parser's runner set-up (the newlib it calls; strncmp and strlen answered as the C library would).
fn parser_runner() -> Option<Runner> {
    let mut r = runner()?;
    for f in [0x179b18u32, 0x179d98, 0x17a148, 0x17a208, 0x179fb0, 0x17a038, 0x17a0a0, 0x179e60, 0x2fcaa0, 0x17a1a8, 0x301090, 0x301b98, 0x305bd0] {
        r.allow(f);
    }
    r.stub(0x2fd478, |c, m| {
        let (a, b, n) = (c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32);
        let byte = |m: &mut r5900::Mem, p: u32| (m.read_u32(p & !3) >> (8 * (p & 3))) as u8;
        let mut d = 0i32;
        for k in 0..n {
            let (x, y) = (byte(m, a + k), byte(m, b + k));
            if x != y || x == 0 {
                d = x as i32 - y as i32;
                break;
            }
        }
        c.set_gpr(2, d as i64 as u64);
    });
    r.stub(0x2fd260, |c, m| {
        let s = c.gpr(4) as u32;
        let mut n = 0;
        while (m.read_u32((s + n) & !3) >> (8 * ((s + n) & 3))) as u8 != 0 {
            n += 1;
        }
        c.set_gpr(2, n as u64);
    });
    Some(r)
}

/// The game's `Pad_ParseButtonMap` on `text`: the parse context's counts and pairs.
fn game_parse(r: &mut Runner, text: &[u8]) -> PadConfig {
    let buf = r.mem.alloc(text.len() as u32 + 16);
    for (k, b) in text.iter().enumerate() {
        r.mem.write_u8(buf + k as u32, *b);
    }
    r.mem.write_u8(buf + text.len() as u32, 0);
    let ctx = r.mem.alloc(0x200);
    loop {
        for o in (0..0x200).step_by(4) {
            r.mem.write_u32(ctx + o, 0);
        }
        r.mem.write_u32(ctx, buf);
        r.mem.write_u32(ctx + 4, text.len() as u32);
        r.cpu.set_gpr(4, ctx as u64);
        // newlib's internals behind sscanf at the end of the text (`__srefill`, `__sinit`, ...) run as they are
        match r.call(0x179b18) {
            Ok(()) => break,
            Err(r5900::Error::UnstubbedCall { target, .. }) if target >= 0x2f0000 => r.allow(target),
            Err(e) => panic!("{e}"),
        }
    }
    let n = |r: &mut Runner, o: u32| r.mem.read_u32(ctx + o) as usize;
    let (nb, ng, na) = (n(r, 8), n(r, 0xc), n(r, 0x10));
    let pairs = |r: &mut Runner, base: u32, k: usize| (0..k as u32).map(|i| (r.mem.read_u32(ctx + base + 8 * i), r.mem.read_u32(ctx + base + 4 + 8 * i))).collect::<Vec<_>>();
    PadConfig { bools: pairs(r, 0x14, nb.min(14)), grabs: pairs(r, 0x84, ng.min(15)), analog: pairs(r, 0xfc, na.min(16)) }
}

#[test]
fn button_map_parser_is_the_original() {
    let Some(dir) = std::env::var_os("TRICKY_BTNMAP_DIR") else { return };
    let mut g = Rng(0xb7a3_0000_0000_0001);
    let mut variants = 0;
    for n in 0..2u32 {
        let text = std::fs::read(std::path::Path::new(&dir).join(format!("BTNMAP{n}.DAT"))).expect("BTNMAP file");
        // the file as it is: the game's parse, ours, and the binding against the transcription
        let Some(mut r) = parser_runner() else { return };
        let theirs = game_parse(&mut r, &text);
        let ours = parse_button_map(&text);
        assert_eq!(theirs, ours, "map {n}");
        let slots: [u16; 15] = std::array::from_fn(|k| (k as u16 * 7 + 3) % 0x1b);
        assert_eq!(bind(&ours, slots), ButtonMap::btnmap(n, slots), "map {n}: bound");
        // and edited: lines dropped or doubled, characters deleted, changed, inserted
        for _ in 0..300 {
            let mut t = text.clone();
            for _ in 0..1 + g.next() % 6 {
                let at = g.next() as usize % t.len().max(1);
                match g.next() % 6 {
                    0 => {
                        t.remove(at);
                    }
                    1 => t.insert(at, b" \t|(),{};\nXNA_09\x80"[g.next() as usize % 16]),
                    2 => t[at] = b" \t|(),{};\nXNA_09\x80"[g.next() as usize % 16],
                    3 => {
                        let e = t[at..].iter().position(|c| *c == b'\n').map_or(t.len(), |p| at + p);
                        t.drain(at..e);
                    }
                    4 => {
                        let s = t[..at].iter().rposition(|c| *c == b'\n').map_or(0, |p| p + 1);
                        let e = t[at..].iter().position(|c| *c == b'\n').map_or(t.len(), |p| at + p + 1);
                        let line = t[s..e].to_vec();
                        for (k, b) in line.iter().enumerate() {
                            t.insert(s + k, *b);
                        }
                    }
                    _ => t.truncate(at),
                }
            }
            let Some(mut r) = parser_runner() else { return };
            let theirs = game_parse(&mut r, &t);
            let ours = parse_button_map(&t);
            let cap = |c: &PadConfig| PadConfig { bools: c.bools.iter().take(14).copied().collect(), grabs: c.grabs.iter().take(15).copied().collect(), analog: c.analog.iter().take(16).copied().collect() };
            assert_eq!(theirs, cap(&ours), "map {n} edited:\n{}", String::from_utf8_lossy(&t));
            variants += 1;
        }
    }
    eprintln!("{variants} edited maps parsed the same");
}

#[test]
fn key_repeat_is_the_original() {
    let Some(mut r) = runner() else { return };
    r.allow(0x17a958);
    let dev = r.mem.alloc(0x40);
    let mut g = Rng(0x4e9_0000_0000_0001);
    let mut k = KeyRepeat { repeat: 1, key: 0, held: 0xffff_ffff, frames: 0 };
    let mut b = 0u32;
    let mut keys = 0;
    for frame in 0..100000 {
        if g.next() % 6 == 0 {
            b = match g.next() % 4 {
                0 => 0,
                1 => 1 << (g.next() % 25),
                _ => (1 << (g.next() % 25)) | (1 << (g.next() % 25)),
            };
        }
        if g.next() % 200 == 0 {
            k.repeat ^= 1;
        }
        r.mem.write_u32(dev + 8, k.repeat);
        r.mem.write_u32(dev + 0x10, b);
        r.mem.write_u32(dev + 0x2c, k.key);
        r.mem.write_u32(dev + 0x30, k.held);
        r.mem.write_u32(dev + 0x34, k.frames as u32);
        r.cpu.set_gpr(4, dev as u64);
        r.call(0x17a958).unwrap();
        k.update(b);
        let theirs = KeyRepeat { repeat: r.mem.read_u32(dev + 8), key: r.mem.read_u32(dev + 0x2c), held: r.mem.read_u32(dev + 0x30), frames: r.mem.read_u32(dev + 0x34) as i32 };
        assert_eq!(theirs, k, "frame {frame} buttons {b:#x}");
        keys += (k.key != 0) as u32;
    }
    eprintln!("{keys} keys given");
}

#[test]
fn rumble_is_the_original() {
    let Some(mut r) = runner() else { return };
    for a in [0x17ad68, 0x17b590, 0x17b5b0, 0x17b5e8] {
        r.allow(a);
    }
    let state: Rc<RefCell<(u64, u64, u8)>> = Rc::new(RefCell::new((6, 1, 0)));
    let s = state.clone();
    r.stub(0x2e76a8, move |c, _| c.set_gpr(2, s.borrow().0));
    let s = state.clone();
    r.stub(0x2e7628, move |c, m| {
        let (_, ret, b0) = *s.borrow();
        m.write_u8(c.gpr(6) as u32, b0);
        c.set_gpr(2, ret);
    });
    let sent: Rc<RefCell<Vec<(u8, u8)>>> = Rc::new(RefCell::new(vec![]));
    let l = sent.clone();
    r.stub(0x2e7c78, move |c, m| {
        let a = c.gpr(6) as u32;
        let w = m.read_u32(a);
        l.borrow_mut().push((w as u8, (w >> 8) as u8));
    });
    let dev = r.mem.alloc(0x100);
    let put = |m: &mut r5900::Mem, d: &Rumble| {
        m.write_u32(dev + 0x48, d.allowed);
        m.write_u8(dev + 0x78, d.small);
        m.write_u8(dev + 0x79, d.large);
        m.write_u32(dev + 0x80, d.send);
        m.write_u32(dev + 0x84, d.small_frames as u32);
        m.write_u32(dev + 0x88, d.large_frames as u32);
    };
    let get = |m: &mut r5900::Mem| {
        let w = m.read_u32(dev + 0x78);
        Rumble { allowed: m.read_u32(dev + 0x48), small: w as u8, large: (w >> 8) as u8, send: m.read_u32(dev + 0x80), small_frames: m.read_u32(dev + 0x84) as i32, large_frames: m.read_u32(dev + 0x88) as i32 }
    };
    let mut d = Rumble::default();
    put(&mut r.mem, &d);
    let mut g = Rng(0x7a6b_1e00_0000_0001);
    for frame in 0..60000 {
        match g.next() % 8 {
            0 => {
                let (on, f) = ((g.next() % 2) as u8, [-1i32, 0, 1, 5, 30][g.next() as usize % 5]);
                r.cpu.set_gpr(4, dev as u64);
                r.cpu.set_gpr(5, on as u64);
                r.cpu.set_gpr(6, f as i64 as u64);
                r.call(0x17b590).unwrap();
                d.set_small(on, f);
            }
            1 => {
                let (a, f) = ([0.0f32, 0.3, 0.5, 1.0, 0.999][g.next() as usize % 5], [-1i32, 0, 2, 12][g.next() as usize % 4]);
                r.cpu.set_f(12, a);
                r.cpu.set_gpr(4, dev as u64);
                r.cpu.set_gpr(5, f as i64 as u64);
                r.call(0x17b5b0).unwrap();
                d.set_large(a, f);
            }
            2 if g.next() % 4 == 0 => {
                let on = g.next() % 2;
                r.cpu.set_gpr(4, dev as u64);
                r.cpu.set_gpr(5, on as u64);
                r.call(0x17b5e8).unwrap();
                d.allow(on);
            }
            _ => {}
        }
        // the poll at state 10: read ready or not
        let ready = g.next() % 10 != 0;
        *state.borrow_mut() = (6, if ready { 1 } else { 0 }, 0);
        r.mem.write_u32(dev + 0x60, 10);
        sent.borrow_mut().clear();
        r.cpu.set_gpr(4, dev as u64);
        r.cpu.set_gpr(5, 0);
        r.call(0x17ad68).unwrap();
        let ours = d.tick(ready);
        assert_eq!(get(&mut r.mem), d, "frame {frame}");
        assert_eq!(sent.borrow().first().copied(), ours, "frame {frame}: sent");
    }
}
