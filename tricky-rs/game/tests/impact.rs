//! `tricky_game::impact` against `cPlayer::vf11_1532c0` 0x1532c0 and `Rumble_Impact` 0x150a80 in the function runner
//! (TRICKY_ELF; skipped without it): the spray's call (`Fx_BrakeSpray_Emit` stubbed to record its arguments) and the
//! pad's shake (the device's virtual stubbed) over random impacts and modes.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::impact::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 6 {
            0 => 0.0,
            1 => 1.0,
            2 => -0.5,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
}

fn put(r: &mut Runner, a: u32, v: [f32; 4]) {
    for k in 0..4 {
        r.mem.write_f32(a + 4 * k, v[k as usize]);
    }
}

#[test]
fn impact_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x11a798);
    r.allow(0x136e70);
    r.allow(0x135258);
    r.allow(0x150a80);
    let emits: Rc<RefCell<Vec<[u32; 5]>>> = Rc::new(RefCell::new(vec![]));
    let e = emits.clone();
    r.stub(0x12e008, move |c, _| e.borrow_mut().push([c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32, c.gpr(7) as u32, c.gpr(8) as u32]));
    let shakes: Rc<RefCell<Vec<[u32; 3]>>> = Rc::new(RefCell::new(vec![]));
    let s = shakes.clone();
    r.stub(0x500000, move |c, _| s.borrow_mut().push([c.gpr(4) as u32, c.gpr(5) as u32, c.gpr(6) as u32]));
    let app = r.mem.alloc(0x800);
    let mid = r.mem.alloc(0x400);
    let mode = r.mem.alloc(0x40);
    let player = r.mem.alloc(0x900);
    let boarder = r.mem.alloc(0x5c00);
    let obj = r.mem.alloc(0x400);
    let render = r.mem.alloc(0x100);
    let dev = r.mem.alloc(0x100);
    let devvt = r.mem.alloc(0x100);
    let (pv, vv, nv) = (r.mem.alloc(0x10), r.mem.alloc(0x10), r.mem.alloc(0x10));
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, mid);
    r.mem.write_u32(player, boarder);
    r.mem.write_u32(boarder + 0x5ae0, obj);
    r.mem.write_u32(obj, 0x1234_0000);
    r.mem.write_u32(obj + 0x3e0, render);
    r.mem.write_u32(player + 0x874, dev);
    // Rumble_Impact's object: word 0 the device, word 1 the peak; the device's vtable record +0x38, its short at +0x20
    r.mem.write_u32(dev, dev + 0x10);
    r.mem.write_u32(dev + 0x10 + 0x38, devvt);
    r.mem.write_u32(devvt + 0x20, 0x0000_0014); // the offset (short) at +0x20
    r.mem.write_u32(devvt + 0x24, 0x500000);
    let mut g = Rng(0x1532_c0a1_50a8_0777);
    let (mut shook, mut muted_n) = (0, 0);
    for case in 0..20_000 {
        let m: Mode = [None, Some(0), Some(1), Some(9), Some(10), Some(-1), Some(40)][(g.next() % 7) as usize];
        match m {
            None => r.mem.write_u32(mid + 0x40, 0),
            Some(x) => {
                r.mem.write_u32(mid + 0x40, mode);
                r.mem.write_u32(mode + 0x20, x as u32);
            }
        }
        let vel = [g.f(), g.f(), g.f(), g.f()];
        let nrm = [g.f() * 0.1, g.f() * 0.1, g.f() * 0.1, 0.0];
        let pos = [g.f(), g.f(), g.f(), 1.0];
        put(&mut r, pv, pos);
        put(&mut r, vv, vel);
        put(&mut r, nv, nrm);
        let peak0 = [0.0f32, 0.5, 1.5, 3.0][(g.next() % 4) as usize];
        r.mem.write_f32(dev + 4, peak0);
        emits.borrow_mut().clear();
        shakes.borrow_mut().clear();
        r.cpu.set_gpr(4, player as u64);
        r.cpu.set_gpr(5, pv as u64);
        r.cpu.set_gpr(6, vv as u64);
        r.cpu.set_gpr(7, nv as u64);
        r.call(0x1532c0).unwrap();
        assert_eq!(*emits.borrow(), vec![[render + 0x99b0, 0x1234_0000, pv, vv, nv]], "case {case} emit");
        let mut peak = peak0;
        let want = if player_rumbles(m) { rumble_impact(&mut peak, vel, nrm, m) } else { None };
        // vf11 skips Rumble_Impact when muted, which also leaves the peak alone
        let want_peak = if player_rumbles(m) { peak } else { peak0 };
        let got: Vec<[u32; 3]> = shakes.borrow().clone();
        match want {
            Some(t) => {
                shook += 1;
                assert_eq!(got, vec![[dev + 0x10 + 0x14, 1, t as u32]], "case {case} shake {m:?}");
            }
            None => {
                assert!(got.is_empty(), "case {case} no shake {m:?}");
                if !player_rumbles(m) {
                    muted_n += 1;
                }
            }
        }
        assert_eq!(r.mem.read_f32(dev + 4).to_bits(), want_peak.to_bits(), "case {case} peak {m:?}");
    }
    eprintln!("shook {shook} muted {muted_n}");
    assert!(shook > 500 && muted_n > 2000);
}
