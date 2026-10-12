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
