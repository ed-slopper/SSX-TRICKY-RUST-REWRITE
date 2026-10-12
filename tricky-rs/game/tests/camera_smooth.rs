//! `tricky_game::camera::smooth_output` (board row E11c) against `Camera_SmoothOutput` 0x174238 in the function
//! runner (TRICKY_ELF; skipped without it), the push call logged and the whole object compared.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::camera::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn f(&mut self) -> f32 {
        match self.next() % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => (self.next() % 2000) as f32 / 1000.0 - 1.0,
            4 => (self.next() % 200_000) as f32 / 100.0 - 1000.0,
            _ => (self.next() % 20_000) as f32 / 1000.0 - 10.0,
        }
    }
    fn ang(&mut self) -> f32 {
        (self.next() % 20_000) as f32 / 1000.0 - 10.0
    }
}

fn bits(v: &[f32]) -> String {
    v.iter().map(|x| format!("{:08x}", x.to_bits())).collect::<Vec<_>>().join(",")
}

#[derive(Default)]
struct Script {
    new_pos: Option<[f32; 3]>,
    log: Vec<String>,
}
struct Env(Rc<RefCell<Script>>);
impl SmoothEnv for Env {
    fn collide_push(&mut self, k: f32, pos: &mut [f32; 3], t: [f32; 3]) {
        let mut s = self.0.borrow_mut();
        s.log.push(format!("push {} {} {}", bits(&[k]), bits(pos), bits(&t)));
        if let Some(p) = s.new_pos {
            *pos = p;
        }
    }
}

#[test]
fn smooth_output_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    for a in [0x1741d0u32, 0x102f80] {
        r.allow(a);
    }
    let st = Rc::new(RefCell::new(Script::default()));
    let s = st.clone();
    r.stub(0x16fb08, move |c, m| {
        let pa = c.gpr(4) as u32;
        let pos: Vec<f32> = (0..3).map(|k| m.read_f32(pa + 4 * k)).collect();
        let t: Vec<f32> = (0..3).map(|k| m.read_f32(c.gpr(5) as u32 + 4 * k)).collect();
        let mut s = s.borrow_mut();
        s.log.push(format!("push {} {} {}", bits(&[c.f(12)]), bits(&pos), bits(&t)));
        if let Some(p) = s.new_pos {
            for k in 0..3 {
                m.write_f32(pa + 4 * k as u32, p[k]);
            }
        }
    });
    const N: usize = 0x4f0 / 4;
    let obj = r.mem.alloc(0x500);
    let mut g = Rng(0xaaaa_5555_0f0f_1234);
    for case in 0..60_000 {
        let mut w = [0u32; N];
        for k in 0..N {
            w[k] = g.f().to_bits();
        }
        for off in [0x438usize, 0x43c, 0x440, 0x490, 0x494, 0x498, 0x4d0] {
            w[off / 4] = g.ang().to_bits();
        }
        // 0x4d0 relation to 0x490 decides the flip
        let pick = |g: &mut Rng, n: u32| (g.next() % n) as u32;
        w[4 / 4] = (g.next() % 4 == 0) as u32;
        w[8 / 4] = (g.next() % 2) as u32;
        w[0xc / 4] = [0u32, 1, 2, 3, 3, 4][pick(&mut g, 6) as usize];
        w[0x18 / 4] = (g.next() % 2) as u32;
        w[0x4b4 / 4] = (g.next() % 2) as u32;
        w[0x4bc / 4] = (g.next() % 2) as u32;
        w[0x4c0 / 4] = [0u32, 1, 2, 0xffff_ffff, 5][pick(&mut g, 5) as usize];
        w[0x4d8 / 4] = (g.next() % 5 == 0) as u32;
        w[0x20 / 4] = [0.0f32, 0.5, 1.0, -1.0, 3.0][pick(&mut g, 5) as usize].to_bits();
        w[0x4cc / 4] = [10.0f32, 49.0, 50.0, 80.0, 0.0, g.f()][pick(&mut g, 6) as usize].to_bits();
        let sc = Script { new_pos: if g.next() % 2 == 0 { Some([g.f(), g.f(), g.f()]) } else { None }, log: vec![] };
        *st.borrow_mut() = Script { new_pos: sc.new_pos, log: vec![] };
        for k in 0..N {
            r.mem.write_u32(obj + 4 * k as u32, w[k]);
        }
        r.cpu.set_gpr(4, obj as u64);
        r.call(0x174238).unwrap();
        let want: Vec<u32> = (0..N).map(|k| r.mem.read_u32(obj + 4 * k as u32)).collect();
        let want_log = st.borrow().log.clone();
        let es = Rc::new(RefCell::new(sc));
        smooth_output(&mut w, &mut Env(es.clone()));
        let got_log = es.borrow().log.clone();
        assert_eq!(got_log, want_log, "case {case} log");
        for k in 0..N {
            assert_eq!(w[k], want[k], "case {case} word {:#x}", k * 4);
        }
    }
}
