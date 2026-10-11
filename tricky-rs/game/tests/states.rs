//! The boarder's state changes against the original's (board row G18): `Boarder_SetMotion` 0x11f610 and
//! `Boarder_SetState` 0x11f608 run in the function runner on the player's SLUS_203.26 (TRICKY_ELF; skipped without
//! it) for every old and new value 0..0x20, every enter and exit routine stubbed and recorded with its a0 (the state
//! object's offset in the manager) and a1, against `tricky_game::states`.

use std::cell::RefCell;
use std::rc::Rc;

use r5900::Runner;
use tricky_game::states::{self, Change};

#[test]
fn state_changes_match_the_original() {
    let Some(mut r) = Runner::from_env() else { return };
    let rider = r.mem.alloc(0x6000);
    let mgr = r.mem.alloc(0x1000);
    r.mem.write_u32(rider + 0x5ae0, mgr);
    r.mem.write_u32(mgr, rider);
    for a in [0x11c648u32, 0x11c7d8, 0x11c6b0, 0x11c710, 0x11c840, 0x11c8e8] {
        r.allow(a);
    }
    let log: Rc<RefCell<Vec<(u32, u32, u32, u32)>>> = Rc::default();
    let mut fns = Vec::new();
    for v in 0..0x20 {
        fns.extend([states::motion_exit(v), states::motion_enter(v), states::control_exit(v), states::control_enter(v)].into_iter().flatten());
    }
    fns.sort_by_key(|f| f.addr);
    fns.dedup();
    for f in fns {
        let l = log.clone();
        let (a, field) = (f.addr, if f.name.contains("Motion") { 0x424 } else { 0x428 });
        r.stub(a, move |c, m| l.borrow_mut().push((a, c.gpr(4) as u32 - mgr, c.gpr(5) as u32, m.read_u32(rider + field))));
    }
    let mut changes = 0;
    for motion in [true, false] {
        for old in 0..0x20u32 {
            for new in 0..0x20u32 {
                let field = if motion { 0x424 } else { 0x428 };
                r.mem.write_u32(rider + field, old);
                r.mem.write_u32(mgr + 0x818, 0x1234);
                log.borrow_mut().clear();
                r.cpu.set_gpr(4, rider as u64);
                r.cpu.set_gpr(5, new as u64);
                r.call(if motion { 0x11f610 } else { 0x11f608 }).unwrap_or_else(|e| panic!("motion {motion}: {old:#x} -> {new:#x}: {e}"));
                let mut cur = old;
                let ours = if motion { states::set_motion(&mut cur, new) } else { states::set_state(&mut cur, new) };
                let want: Vec<(u32, u32, u32, u32)> = ours.iter().filter_map(|c| match *c {
                    Change::Exit(f) => Some((f.addr, f.obj, new, old)),
                    Change::Enter(f, o) => Some((f.addr, f.obj, o, new)),
                    Change::ClearResetTimer => None,
                }).collect();
                let ctx = format!("motion {motion}: {old:#x} -> {new:#x}");
                // the exit routine's a1 is whatever it held: the new value, as the change passes it on
                let got: Vec<(u32, u32, u32, u32)> = log.borrow().clone();
                assert_eq!(got, want, "{ctx}: calls");
                assert_eq!(r.mem.read_u32(rider + field), cur, "{ctx}: field");
                let cleared = ours.contains(&Change::ClearResetTimer);
                assert_eq!(r.mem.read_u32(mgr + 0x818), if cleared { 0 } else { 0x1234 }, "{ctx}: reset timer");
                changes += ours.len();
            }
        }
    }
    assert!(changes > 1000, "{changes}");
}
