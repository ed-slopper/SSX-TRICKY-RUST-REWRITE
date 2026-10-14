//! `tricky_game::sndlib` (board row G10d) against the original in the function runner (TRICKY_ELF; skipped
//! without it).

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::sndlib::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn runner() -> Option<Runner> {
    let mut r = Runner::from_env()?;
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    Some(r)
}

#[test]
fn pitch_ratio_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let mut g = Rng(0x1357_9bdf_2468_ace0);
    let check = |r: &mut Runner, c: i32| {
        r.cpu.set_gpr(4, c as i64 as u64);
        r.call(0x2d51f8).unwrap();
        assert_eq!(r.cpu.gpr(2) as u32 as i32, pitch_ratio(c), "cents {c}");
    };
    for c in -3000..3000 {
        check(&mut r, c);
    }
    for _ in 0..30_000 {
        let c = (g.next() as i32) % 60_000;
        check(&mut r, c);
    }
    for c in [0x4af, 0x4b0, -0x4af, -0x4b0, -0x4b1, 0x1_0000, -0x1_0000, 1_000_000, -1_000_000, 2_000_000, -2_000_000] {
        check(&mut r, c);
    }
}

#[test]
fn calc_pitch_mult_matches_the_original() {
    let Some(mut r) = runner() else { return };
    let pool = r.mem.alloc(0x88 * 4);
    let t1 = r.mem.alloc(0x400) + 0x200;
    let t2 = r.mem.alloc(0x400) + 0x200;
    r.allow(0x2d51f8);
    r.mem.write_u32(0x3511f8, pool);
    let mut g = Rng(0x0bad_cafe_1234_5678);
    for case in 0..40_000 {
        for k in 0..0x400 / 4 {
            r.mem.write_u32(t1 - 0x200 + 4 * k, g.next());
            r.mem.write_u32(t2 - 0x200 + 4 * k, g.next());
        }
        for k in 0..(0x88 * 4 / 4) {
            r.mem.write_u32(pool + 4 * k, g.next());
        }
        let idx = g.next() % 4;
        let at = pool + 0x88 * idx;
        let mut rec = [0u8; 0x88];
        r.mem.read(at, &mut rec).unwrap();
        let w16 = |rec: &mut [u8; 0x88], o: usize, v: u16| rec[o..o + 2].copy_from_slice(&v.to_le_bytes());
        let w32 = |rec: &mut [u8; 0x88], o: usize, v: u32| rec[o..o + 4].copy_from_slice(&v.to_le_bytes());
        rec[0x2b] = (g.next() % 256) as u8;
        w32(&mut rec, 0x70, [0, t1, t1][g.next() as usize % 3]);
        w32(&mut rec, 0x78, [0, t2, t2][g.next() as usize % 3]);
        w16(&mut rec, 0x7e, [0, g.next() as u16, (g.next() % 200) as u16, 0xff00][g.next() as usize % 4]);
        w16(&mut rec, 0x7c, [0, g.next() as u16, (g.next() % 200) as u16][g.next() as usize % 3]);
        w16(&mut rec, 0x80, [g.next() as u16, (g.next() % 3000) as u16, 0xf000][g.next() as usize % 3]);
        w16(&mut rec, 0x82, [0, 0, 0, g.next() as u16][g.next() as usize % 4]);
        rec[0x2b] = if rec[0x70..0x74] != [0; 4] { (g.next() % 200) as u8 } else { rec[0x2b] };
        for (o, v) in rec.iter().enumerate() {
            r.mem.write(at + o as u32, &[*v]);
        }
        let mut v = PitchVoice {
            note: rec[0x2b] as i8,
            pos: rec[0x64],
            note_table: u32::from_le_bytes(rec[0x70..0x74].try_into().unwrap()),
            mod_table: u32::from_le_bytes(rec[0x78..0x7c].try_into().unwrap()),
            mod_depth: i16::from_le_bytes(rec[0x7c..0x7e].try_into().unwrap()),
            note_depth: i16::from_le_bytes(rec[0x7e..0x80].try_into().unwrap()),
            base: i16::from_le_bytes(rec[0x80..0x82].try_into().unwrap()),
            ratio: u16::from_le_bytes(rec[0x82..0x84].try_into().unwrap()),
            scale: u16::from_le_bytes(rec[0x84..0x86].try_into().unwrap()),
            mult: 0,
        };
        // the table bytes (the second table is read at its position, up to 255)
        let mut tab = std::collections::BTreeMap::new();
        for base in [t1, t2] {
            for k in 0..0x400u32 {
                let mut b = [0u8];
                r.mem.read(base - 0x200 + k, &mut b).unwrap();
                tab.insert(base - 0x200 + k, b[0] as i8);
            }
        }
        r.cpu.set_gpr(4, idx as u64);
        r.call(0x2d5288).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let mut after = [0u8; 0x88];
        r.mem.read(at, &mut after).unwrap();
        calc_pitch_mult(&mut v, &|a| *tab.get(&a).unwrap_or_else(|| panic!("case {case}: read {a:#x}")));
        assert_eq!(i16::from_le_bytes(after[0x86..0x88].try_into().unwrap()), v.mult, "case {case} mult");
        assert_eq!(u16::from_le_bytes(after[0x82..0x84].try_into().unwrap()), v.ratio, "case {case} ratio");
        let mut want = rec;
        want[0x82..0x84].copy_from_slice(&v.ratio.to_le_bytes());
        want[0x86..0x88].copy_from_slice(&v.mult.to_le_bytes());
        assert_eq!(&after[..], &want[..], "case {case} record");
    }
}

// ---- SetVolume, IsDone ----

use std::cell::RefCell;
use std::rc::Rc;

type Log = Rc<RefCell<Vec<(&'static str, u64, u64)>>>;

struct VEnv {
    log: Log,
    ret: i64,
    state: i32,
}
impl VoiceEnv for VEnv {
    fn lock(&mut self) {
        self.log.borrow_mut().push(("lock", 0, 0));
    }
    fn unlock(&mut self) {
        self.log.borrow_mut().push(("unlock", 0, 0));
    }
    fn channel1_reset(&mut self, handle: u32) {
        self.log.borrow_mut().push(("c1r", handle as u64, 0));
    }
    fn channel1_volume(&mut self, handle: u32, volume: i32) {
        self.log.borrow_mut().push(("c1v", handle as u64, volume as u32 as u64));
    }
    fn channel2_reset(&mut self, handle: u32) {
        self.log.borrow_mut().push(("c2r", handle as u64, 0));
    }
    fn channel2_volume(&mut self, handle: u32, volume: i32) {
        self.log.borrow_mut().push(("c2v", handle as u64, volume as u32 as u64));
    }
    fn stream_stop(&mut self, stream: u32) {
        self.log.borrow_mut().push(("ss", stream as u64, 0));
    }
    fn stream_volume(&mut self, stream: u32, volume: i32) {
        self.log.borrow_mut().push(("sv", stream as u64, volume as u32 as u64));
    }
    fn channel1_playing(&mut self, handle: u32) -> i64 {
        self.log.borrow_mut().push(("c1p", handle as u64, 0));
        self.ret
    }
    fn channel2_state(&mut self, handle: u32) -> i64 {
        self.log.borrow_mut().push(("c2s", handle as u64, 0));
        self.ret
    }
    fn stream_state(&mut self, handle: u32) -> (i64, i32) {
        self.log.borrow_mut().push(("sts", handle as u64, 0));
        (self.ret, self.state)
    }
}

fn stubs(r: &mut Runner, log: &Log, ret: &Rc<RefCell<(i64, i32)>>) {
    for (a, name) in [
        (0x2d0908u32, "lock"),
        (0x2d0938, "unlock"),
        (0x2d3260, "c1r"),
        (0x2d3108, "c1v"),
        (0x2d4368, "c2r"),
        (0x2d4888, "c2v"),
        (0x229868, "ss"),
        (0x229398, "sv"),
        (0x2d3338, "c1p"),
        (0x2d4a08, "c2s"),
        (0x2d2c10, "sts"),
    ] {
        let l = log.clone();
        let rv = ret.clone();
        r.stub(a, move |c, m| {
            let (x, y) = match name {
                "lock" | "unlock" => (0, 0),
                "c1r" | "c2r" | "ss" | "c1p" | "c2s" | "sts" => (c.gpr(4) as u32 as u64, 0),
                _ => (c.gpr(4) as u32 as u64, c.gpr(5) as u32 as u64),
            };
            l.borrow_mut().push((name, x, y));
            if name == "sts" {
                m.write_u32(c.gpr(5) as u32, rv.borrow().1 as u32);
            }
            c.set_gpr(2, rv.borrow().0 as u64);
        });
    }
}

#[test]
fn set_volume_and_is_done_match_the_original() {
    let Some(mut r) = runner() else { return };
    let log: Log = Rc::new(RefCell::new(vec![]));
    let ret = Rc::new(RefCell::new((0i64, 0i32)));
    stubs(&mut r, &log, &ret);
    let rec = r.mem.alloc(0xb0);
    let mut g = Rng(0x600d_f00d_dead_beef);
    for case in 0..60_000 {
        for k in 0..0xb0 / 4 {
            r.mem.write_u32(rec + 4 * k, g.next());
        }
        let kind = [1u32, 2, 3, 1, 2, 3, 0, 4, 0xffff_ffff][g.next() as usize % 9];
        let scale = [-1.0f32, 0.0, 0.5, 1.0, 2.0, 0.99, 100.0, f32::from_bits(g.next() & 0xbfff_ffff), f32::from_bits(g.next() & 0x807f_ffff), -0.0][g.next() as usize % 10];
        // (the hardware takes an exponent of 255 as a big number, not a NaN: the scale is never one)
        let volume: i32 = [-1, 0, 5, 127, 128, 200, -50, g.next() as i32, (g.next() % 140) as i32, (g.next() % 140) as i32, i32::MAX, i32::MIN, 0x0100_0001][g.next() as usize % 13];
        let last: i32 = [-1, (g.next() % 140) as i32, volume.wrapping_add((g.next() % 13) as i32 - 6), volume.clamp(0, 127) + (g.next() % 11) as i32 - 5, g.next() as i32][g.next() as usize % 5];
        r.mem.write_u32(rec, kind);
        r.mem.write_u32(rec + 0x90, scale.to_bits());
        r.mem.write_u32(rec + 0xa8, last as u32);
        let mut before = [0u8; 0xb0];
        r.mem.read(rec, &mut before).unwrap();
        let mut v = SndVoice { kind: kind as i32, handle: r.mem.read_u32(rec + 4), stream: r.mem.read_u32(rec + 0xc), scale, last };
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rec as u64);
        r.cpu.set_gpr(5, volume as i64 as u64);
        r.call(0x20ae28).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let orig = std::mem::take(&mut *log.borrow_mut());
        let mut after = [0u8; 0xb0];
        r.mem.read(rec, &mut after).unwrap();
        let mut env = VEnv { log: log.clone(), ret: 0, state: 0 };
        set_volume(&mut v, volume, &mut env);
        let mine = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(orig, mine, "case {case} kind {kind} scale {scale} volume {volume} last {last}");
        let mut want = before;
        want[0xa8..0xac].copy_from_slice(&(v.last as u32).to_le_bytes());
        assert_eq!(after, want, "case {case} record kind {kind} scale {scale} volume {volume} last {last}");

        // IsDone
        let rv = [0i64, 1, 2, 3, -1, -5, 0x1_0000_0000, -0x1_0000_0000, g.next() as i64][g.next() as usize % 9];
        let st = [3i32, 0, 1, 2, g.next() as i32][g.next() as usize % 5];
        *ret.borrow_mut() = (rv, st);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, rec as u64);
        r.call(0x20a8e0).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let orig = std::mem::take(&mut *log.borrow_mut());
        let done = r.cpu.gpr(2) as u32;
        let mut env = VEnv { log: log.clone(), ret: rv, state: st };
        let mine_done = is_done(kind as i32, v.handle, &mut env);
        let mine = std::mem::take(&mut *log.borrow_mut());
        assert_eq!(orig, mine, "case {case} isdone kind {kind} ret {rv} state {st}");
        assert_eq!(done, mine_done as u32, "case {case} isdone kind {kind} ret {rv} state {st}");
    }
}
