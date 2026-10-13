//! `tricky_game::camscript::is_rider_anim_done` against `IsRiderAnimDone` 0x16db88 in the function runner
//! (TRICKY_ELF; skipped without it), `Race_GetRider` and the animation progress stubbed and logged.

use r5900::ps2float::Rules;
use r5900::Runner;
use std::cell::RefCell;
use std::rc::Rc;
use tricky_game::camscript::*;

struct E {
    log: Rc<RefCell<Vec<Vec<u32>>>>,
    riders: [u32; 4],
    prog: f32,
}
impl RiderEnv for E {
    fn rider(&mut self, i: u32) -> u32 {
        self.log.borrow_mut().push(vec![1, i]);
        self.riders[i as usize]
    }
    fn anim_progress(&mut self, rider: u32) -> f32 {
        self.log.borrow_mut().push(vec![2, rider]);
        self.prog
    }
}

#[test]
fn rider_anim_done_matches_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    r.float_rules = Rules::parse("add=n,mul=n,div=n,cvt=n").unwrap();
    r.allow(0x16db88);
    let app = r.mem.alloc(0x800);
    let race = r.mem.alloc(0x40);
    let st = r.mem.alloc(0x40);
    r.mem.write_u32(0x337c58, app);
    r.mem.write_u32(app + 0x730, race);
    let log: Rc<RefCell<Vec<Vec<u32>>>> = Rc::new(RefCell::new(vec![]));
    let riders = Rc::new(RefCell::new([0u32; 4]));
    let prog = Rc::new(RefCell::new(0f32));
    let (l, rd) = (log.clone(), riders.clone());
    r.stub(0x1814a8, move |c, _| {
        assert_eq!(c.gpr(4) as u32, race);
        l.borrow_mut().push(vec![1, c.gpr(5) as u32]);
        c.set_gpr(2, rd.borrow()[c.gpr(5) as usize] as u64);
    });
    let (l, p) = (log.clone(), prog.clone());
    r.stub(0x15fe70, move |c, _| {
        assert_eq!(c.gpr(5), 0);
        l.borrow_mut().push(vec![2, c.gpr(4) as u32 - 0x46c0]);
        c.set_f(0, *p.borrow());
    });
    let vals = [0.0f32, 0.5, 0.999_999_94, 1.0, 1.000_000_1, 2.0, -1.0];
    let mut n = [0u32; 2];
    for case in 0..2000u32 {
        let idx = case % 4;
        let rv = [[0u32, 0x4000_0000, 0x4001_0000, 0][idx as usize], 0x5000_0000][(case / 4 % 2) as usize];
        *riders.borrow_mut() = [rv; 4];
        let pv = vals[(case / 8) as usize % vals.len()];
        *prog.borrow_mut() = pv;
        r.mem.write_u32(st + 0xc, idx);
        log.borrow_mut().clear();
        r.cpu.set_gpr(4, st as u64);
        r.call(0x16db88).unwrap();
        let want = r.cpu.gpr(2) as u32;
        let wl = std::mem::take(&mut *log.borrow_mut());
        let mut e = E { log: log.clone(), riders: *riders.borrow(), prog: pv };
        let got = is_rider_anim_done(idx, &mut e);
        assert_eq!((want, wl), (got, std::mem::take(&mut *log.borrow_mut())), "case {case}");
        n[got as usize] += 1;
    }
    eprintln!("rider anim: {n:?}");
}
