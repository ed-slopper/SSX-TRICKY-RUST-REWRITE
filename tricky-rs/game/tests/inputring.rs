//! The input device list against the original's (board row E10c), in the function runner on the player's
//! SLUS_203.26 (TRICKY_ELF; skipped without it): `Input_ReadDevices` 0x17a668 and `InputRecorder_Playback`
//! 0x17a740 over two real `cPS2Device` objects (the poll, vf9, answered by the test), the ring's indices and
//! bytes, the pads after each playback, `Input_FindPressedDevice` 0x17a840, `Input_GetDevicePressed` 0x17a898,
//! `Input_GetAnyNewButtons` 0x1834a8, `Pad_LatchButtons` 0x17a658 and `Input_ResetDevices` 0x17a8b0.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::ps2float::Rules;
use r5900::Runner;
use tricky_game::inputring::{any_new_buttons, find_pressed, reset_devices, Input, Record};

fn rb(m: &mut r5900::Mem, a: u32) -> u8 {
    (m.read_u32(a & !3) >> (8 * (a & 3))) as u8
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

fn random_record(g: &mut Rng) -> Record {
    let valid = (g.next() % 6 != 0) as u32;
    let mut raw = [0u8; 16];
    for b in raw.iter_mut() {
        *b = g.next() as u8;
    }
    Record { raw, valid, analog: (g.next() % 3 != 0) as u32, buttons: g.next() as u16, sticks: [g.next() & 255, g.next() & 255, g.next() & 255, g.next() & 255] }
}

#[test]
fn ring_and_devices_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for f in [0x2ce114u32, 0x17ad60, 0x17b278, 0x17a958, 0x17b590, 0x17b5b0, 0x1834a8, 0x17a830, 0x17a840, 0x17a898, 0x17a8b0, 0x17a658, 0x17a668, 0x17a740] {
        r.allow(f);
    }
    // the poll answers with the record the test queued for that pad
    let queue: Rc<RefCell<Vec<Option<Record>>>> = Rc::new(RefCell::new(Vec::new()));
    let q = queue.clone();
    r.stub(0x17b270, move |c, m| {
        let out = c.gpr(5) as u32;
        let rec = q.borrow_mut().remove(0).unwrap();
        for (k, b) in rec.to_bytes().iter().enumerate() {
            m.write_u8(out + k as u32, *b);
        }
    });
    let list = r.mem.alloc(0x40);
    let pads = [r.mem.alloc(0x100), r.mem.alloc(0x100)];
    let buf = r.mem.alloc(0x58 * 8);
    // gApp (0x337c58) holds the app, whose +0x24 is the list
    let app = r.mem.alloc(0x40);
    r.mem.write_u32(app + 0x24, list);
    r.mem.write_u32(0x337c58, app);
    for (n, p) in pads.iter().enumerate() {
        r.mem.write_u32(p + 0x38, 0x385908);
        r.mem.write_u32(list + 0x18 + 4 * n as u32, *p);
        // cPS2Device::cPS2Device's fields that matter
        r.mem.write_u32(p + 0x48, 1);
        r.mem.write_u32(p + 0x80, 1);
        r.mem.write_u32(p + 0x84, 0xffff_ffff);
        r.mem.write_u32(p + 0x88, 0xffff_ffff);
    }
    r.mem.write_u32(list, buf);
    r.mem.write_u32(list + 4, 0x58);
    r.mem.write_u32(list + 8, 8);
    r.mem.write_u32(list + 0x14, 2);
    let mut ours = Input::new();
    let mut g = Rng(0x1ee7_5a31_c0de_0044);
    let mut played = 0;
    for step in 0..6000 {
        match g.next() % 5 {
            0 | 1 => {
                let recs = [random_record(&mut g), random_record(&mut g)];
                *queue.borrow_mut() = recs.iter().map(|x| Some(*x)).collect();
                r.cpu.set_gpr(4, list as u64);
                r.call(0x17a668).unwrap_or_else(|e| panic!("step {step}: {e}"));
                ours.read_devices(&recs);
            }
            2 | 3 => {
                r.cpu.set_gpr(4, list as u64);
                r.call(0x17a740).unwrap_or_else(|e| panic!("step {step}: {e}"));
                let theirs = r.cpu.gpr(2) as u32 != 0;
                assert_eq!(theirs, ours.playback(), "step {step}: playback");
                played += theirs as u32;
            }
            _ => {
                for p in &pads {
                    r.cpu.set_gpr(4, *p as u64);
                    r.call(0x17a658).unwrap();
                }
                for p in &mut ours.pads {
                    p.latch();
                }
                r.cpu.set_gpr(4, list as u64);
                r.call(0x17a8b0).unwrap_or_else(|e| panic!("step {step}: {e}"));
                reset_devices(&mut ours.pads);
            }
        }
        assert_eq!(r.mem.read_u32(list + 0xc) as usize, ours.ring.read, "step {step}: read index");
        assert_eq!(r.mem.read_u32(list + 0x10) as usize, ours.ring.write, "step {step}: write index");
        for k in 0..0x58 * 8 {
            assert_eq!(rb(&mut r.mem, buf + k), ours.ring.buf[k as usize], "step {step}: ring byte {k:#x}");
        }
        for (n, p) in pads.iter().enumerate() {
            let o = &ours.pads[n];
            assert_eq!(r.mem.read_u32(p + 4), o.valid, "step {step}: pad {n} valid");
            assert_eq!(r.mem.read_u32(p + 0xc), o.dev.prev, "step {step}: pad {n} prev");
            assert_eq!(r.mem.read_u32(p + 0x10), o.dev.buttons, "step {step}: pad {n} buttons");
            assert_eq!(r.mem.read_u32(p + 0x2c), o.repeat.key, "step {step}: pad {n} key");
            assert_eq!(r.mem.read_u32(p + 0x30), o.repeat.held, "step {step}: pad {n} held");
            assert_eq!(r.mem.read_u32(p + 0x34) as i32, o.repeat.frames, "step {step}: pad {n} frames");
            for k in 0..16 {
                assert_eq!(rb(&mut r.mem, p + 0x68 + k), o.raw[k as usize], "step {step}: pad {n} raw {k}");
            }
            assert_eq!(r.mem.read_u32(p + 0x84) as i32, o.rumble.small_frames, "step {step}: pad {n} small frames");
            assert_eq!(r.mem.read_u32(p + 0x88) as i32, o.rumble.large_frames, "step {step}: pad {n} large frames");
            assert_eq!(r.mem.read_u32(p + 0x80), o.rumble.send, "step {step}: pad {n} send");
            r.cpu.set_gpr(4, list as u64);
            r.cpu.set_gpr(5, n as u64);
            r.call(0x17a898).unwrap();
            assert_eq!(r.cpu.gpr(2) as u32, o.repeat.key, "step {step}: pressed of {n}");
        }
        r.call(0x1834a8).unwrap_or_else(|e| panic!("step {step}: {e}"));
        assert_eq!(r.cpu.gpr(2) as u32, any_new_buttons(&ours.pads), "step {step}: new buttons");
        let scratch = r.mem.alloc(8);
        r.cpu.set_gpr(4, list as u64);
        r.cpu.set_gpr(5, scratch as u64);
        r.call(0x17a840).unwrap();
        let theirs = r.cpu.gpr(2) as u32;
        let found = find_pressed(&ours.pads);
        assert_eq!(theirs, found.map_or(0, |f| f.0), "step {step}: find pressed");
        if let Some((_, i)) = found {
            assert_eq!(r.mem.read_u32(scratch) as usize, i, "step {step}: found index");
        }
    }
    assert!(played > 1000, "playbacks: {played}");
}

// ---- the poll (vf9 0x17ad68, E10c2) ----

use tricky_game::input::Rumble;
use tricky_game::inputring::{Libpad, Poller};

/// libpad's answers: the same script for both sides, drawn per call from a seed set before each poll.
#[derive(Clone)]
struct Oracle {
    g: u64,
    log: Vec<String>,
}
impl Oracle {
    fn draw(&mut self, id: u32, choices: &[i64]) -> i64 {
        self.g ^= self.g << 13;
        self.g ^= self.g >> 7;
        self.g ^= self.g << 17;
        let v = choices[(self.g >> 20) as usize % choices.len()];
        self.log.push(format!("{id:x}:{v}"));
        v
    }
    fn bytes(&mut self, buf: &mut [u8; 20]) {
        for b in buf.iter_mut() {
            self.g ^= self.g << 13;
            self.g ^= self.g >> 7;
            self.g ^= self.g << 17;
            *b = (self.g >> 24) as u8;
        }
    }
}
const OPEN: &[i64] = &[1, 1, 0, -1];
const GET: &[i64] = &[6, 6, 6, 6, 6, 5, 5, 2, 0, 1, 99];
const INFO1: &[i64] = &[4, 7, 7, 0];
const INFO2: &[i64] = &[0, 4, 7, 7, 7, 2, 9];
const SENT: &[i64] = &[1, 1, 1, 0];
const REQ: &[i64] = &[0, 0, 1, 1, 2];
const ACT: &[i64] = &[0, 1, 1, 2];
const READ: &[i64] = &[1, 1, 1, 1, 0];

impl Libpad for Oracle {
    fn port_open(&mut self) -> i64 { self.draw(0x2e72d8, OPEN) }
    fn get_state(&mut self) -> i64 { self.draw(0x2e76a8, GET) }
    fn info_mode(&mut self, what: i64) -> i64 { if what == 1 { self.draw(0x2e7a88, INFO1) } else { self.draw(0x12e7a88, INFO2) } }
    fn set_main_mode(&mut self, mode: i64, lock: i64) -> i64 {
        self.log.push(format!("mode {mode} {lock}"));
        self.draw(0x2e7bc0, SENT)
    }
    fn enter_press_mode(&mut self) -> i64 { self.draw(0x2e7f70, SENT) }
    fn press_mode_next(&mut self) -> i64 { self.draw(0x2e7fd0, SENT) }
    fn req_state(&mut self) -> i64 { self.draw(0x2e77c0, REQ) }
    fn info_act(&mut self) -> i64 { self.draw(0x2e7848, ACT) }
    fn set_act_align(&mut self) -> i64 { self.draw(0x2e7d30, SENT) }
    fn read(&mut self, buf: &mut [u8; 20]) -> i64 {
        let r = self.draw(0x2e7628, READ);
        self.bytes(buf);
        if self.g % 7 != 0 {
            buf[0] = 0;
        }
        self.log.push(format!("read {:?}", &buf[..]));
        r
    }
    fn set_act_direct(&mut self, small: u8, large: u8) { self.log.push(format!("act {small} {large}")); }
}

#[test]
fn the_poll_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x17ad68);
    let theirs: Rc<RefCell<Oracle>> = Rc::new(RefCell::new(Oracle { g: 1, log: Vec::new() }));
    let hook = |r: &mut Runner, addr: u32, f: fn(&mut Oracle, &mut r5900::Cpu, &mut r5900::Mem)| {
        let o = theirs.clone();
        r.stub(addr, move |c, m| f(&mut o.borrow_mut(), c, m));
    };
    hook(&mut r, 0x2e72d8, |o, c, _| c.set_gpr(2, o.port_open() as u64));
    hook(&mut r, 0x2e76a8, |o, c, _| c.set_gpr(2, o.get_state() as u64));
    hook(&mut r, 0x2e7a88, |o, c, _| {
        let what = c.gpr(6) as i64;
        assert!(what == 1 || what == 2);
        c.set_gpr(2, o.info_mode(what) as u64)
    });
    hook(&mut r, 0x2e7bc0, |o, c, _| {
        let (mode, lock) = (c.gpr(6) as i64, c.gpr(7) as i64);
        c.set_gpr(2, o.set_main_mode(mode, lock) as u64)
    });
    hook(&mut r, 0x2e7f70, |o, c, _| c.set_gpr(2, o.enter_press_mode() as u64));
    hook(&mut r, 0x2e7fd0, |o, c, _| c.set_gpr(2, o.press_mode_next() as u64));
    hook(&mut r, 0x2e77c0, |o, c, _| c.set_gpr(2, o.req_state() as u64));
    hook(&mut r, 0x2e7848, |o, c, _| c.set_gpr(2, o.info_act() as u64));
    hook(&mut r, 0x2e7d30, |o, c, _| c.set_gpr(2, o.set_act_align() as u64));
    hook(&mut r, 0x2e7628, |o, c, m| {
        let at = c.gpr(6) as u32;
        let mut buf = [0u8; 20];
        let v = o.read(&mut buf);
        for (k, b) in buf.iter().enumerate() {
            m.write_u8(at + k as u32, *b);
        }
        c.set_gpr(2, v as u64)
    });
    hook(&mut r, 0x2e7c78, |o, c, m| {
        let at = c.gpr(6) as u32;
        let w = m.read_u32(at);
        o.set_act_direct(w as u8, (w >> 8) as u8)
    });
    // memset(dst, 0, n)
    r.stub(0x2cc808, |c, m| {
        let (d, n) = (c.gpr(4) as u32, c.gpr(6) as u32);
        for k in 0..n {
            m.write_u8(d + k, 0);
        }
        c.set_gpr(2, d as u64);
    });
    let dev = r.mem.alloc(0x200);
    let out = r.mem.alloc(0x40);
    let mut ours = Poller::default();
    let mut rumble = Rumble::default();
    r.mem.write_u32(dev + 0x58, 1);
    r.mem.write_u32(dev + 0x5c, 1);
    r.mem.write_u32(dev + 0x80, 1);
    r.mem.write_u32(dev + 0x84, 0xffff_ffff);
    r.mem.write_u32(dev + 0x88, 0xffff_ffff);
    r.mem.write_u32(dev + 0x3c, 0);
    r.mem.write_u32(dev + 0x40, 0);
    let mut g = Rng(0x9011_77ab_4c42_0e19);
    let mut ready = 0;
    let mut sent = 0;
    for step in 0..40_000u64 {
        // sometimes change what the game asks of the pad
        match g.next() % 40 {
            0 => {
                ours.want = g.next() % 2;
                r.mem.write_u32(dev + 0x58, ours.want);
            }
            1 => {
                ours.lock = g.next() % 2;
                r.mem.write_u32(dev + 0x5c, ours.lock);
            }
            2 | 3 => {
                let f = (g.next() % 6) as i32 - 1;
                let on = g.next() as u8 & 1;
                rumble.set_small(on, f);
                r.mem.write_u8(dev + 0x78, rumble.small);
                r.mem.write_u32(dev + 0x84, rumble.small_frames as u32);
                r.mem.write_u32(dev + 0x80, rumble.send);
            }
            4 => {
                let f = (g.next() % 6) as i32 - 1;
                rumble.set_large(0.5, f);
                r.mem.write_u8(dev + 0x79, rumble.large);
                r.mem.write_u32(dev + 0x88, rumble.large_frames as u32);
                r.mem.write_u32(dev + 0x80, rumble.send);
            }
            _ => {}
        }
        let want_frame = g.next() % 8 != 0;
        let seed = 0x1234_5678_9abc_def1u64 ^ step.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        theirs.borrow_mut().g = seed | 1;
        theirs.borrow_mut().log.clear();
        let mut mine = Oracle { g: seed | 1, log: Vec::new() };
        for k in 0..0x2c {
            r.mem.write_u8(out + k, 0xaa);
        }
        r.cpu.set_gpr(4, dev as u64);
        r.cpu.set_gpr(5, if want_frame { out as u64 } else { 0 });
        r.call(0x17ad68).unwrap_or_else(|e| panic!("step {step}: {e}"));
        let frame = ours.poll(&mut mine, &mut rumble, want_frame);
        assert_eq!(theirs.borrow().log, mine.log, "step {step}: libpad calls");
        let w = |m: &mut r5900::Mem, o: u32| m.read_u32(dev + o);
        assert_eq!(w(&mut r.mem, 0x60), ours.state, "step {step}: state");
        assert_eq!(w(&mut r.mem, 0x44), ours.gone, "step {step}: gone");
        assert_eq!(w(&mut r.mem, 0x4c), ours.pressure, "step {step}: pressure");
        assert_eq!(w(&mut r.mem, 0x50), ours.analog, "step {step}: analog");
        assert_eq!(w(&mut r.mem, 0x54), ours.act_failed, "step {step}: act failed");
        assert_eq!(w(&mut r.mem, 0x64) as i32, ours.count, "step {step}: count");
        assert_eq!(w(&mut r.mem, 0x80), rumble.send, "step {step}: send");
        assert_eq!(w(&mut r.mem, 0x84) as i32, rumble.small_frames, "step {step}: small frames");
        assert_eq!(w(&mut r.mem, 0x88) as i32, rumble.large_frames, "step {step}: large frames");
        assert_eq!(rb(&mut r.mem, dev + 0x78), rumble.small, "step {step}: small");
        assert_eq!(rb(&mut r.mem, dev + 0x79), rumble.large, "step {step}: large");
        let bytes: Vec<u8> = (0..0x2c).map(|k| rb(&mut r.mem, out + k)).collect();
        // bytes 0x1a and 0x1b are the halfword's padding: the original leaves what was there
        let mut bytes = bytes;
        bytes[0x1a] = 0;
        bytes[0x1b] = 0;
        match frame {
            Some(f) => assert_eq!(bytes, f.to_bytes().to_vec(), "step {step}: frame"),
            None => assert!(bytes.iter().enumerate().all(|(k, b)| *b == 0xaa || k == 0x1a || k == 0x1b), "step {step}: frame written without being asked"),
        }
        ready += (ours.state == 10) as u32;
        sent += mine.log.iter().filter(|l| l.starts_with("act")).count();
    }
    assert!(ready > 1000, "ready polls: {ready}");
    assert!(sent > 200, "motor sends: {sent}");
}
