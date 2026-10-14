//! The world sound emitters' small functions (board row G10a2), each the game's and checked in the function runner
//! (`tests/worldemitter.rs`): `WorldEmitter_Clear` 0x22d080, `_FindFree` 0x22d188, `_Find` 0x22d1b8, `_Matches`
//! 0x22bf30, `_Falloff` 0x22bc40, `_Stop` 0x22c5a0, `_StopAll` 0x22d408, `_UpdateAll` 0x22d370.
//!
//! The emitter table is 40 records of 0x50 bytes (20 words) a manager points at with its first word. Word 0 is the
//! state (0 free, 1 playing), +4 / +8 / +0x3c the key `Matches` compares, +0x30 the voice handle (-1 none), +0x34 a
//! "still wanted" flag `UpdateAll` clears every pass, +0x38 which voice engine the handle is in, +0x40 -1 once stopped.

/// Words in an emitter record and the number of records.
pub const WORDS: usize = 20;
pub const COUNT: usize = 40;

/// `WorldEmitter_Clear`: free the record and forget its voice.
pub fn clear(e: &mut [u32]) {
    e[0] = 0;
    e[0x10] = 0xffff_ffff;
    e[0xd] = 0;
    e[0xc] = 0xffff_ffff;
}

/// `WorldEmitter_Matches`: the record's key words (+4, +8, +0x3c) equal `a`, `b`, `c`.
pub fn matches(e: &[u32], a: u32, b: u32, c: u32) -> bool {
    a == e[1] && b == e[2] && c == e[0xf]
}

/// `WorldEmitter_FindFree`: the first record whose state is 0.
pub fn find_free(t: &[u32]) -> Option<usize> {
    (0..COUNT).find(|i| t[i * WORDS] == 0)
}

/// `WorldEmitter_Find`: the first in-use record (state not 0) that matches the key.
pub fn find(t: &[u32], a: u32, b: u32, c: u32) -> Option<usize> {
    (0..COUNT).find(|&i| t[i * WORDS] != 0 && matches(&t[i * WORDS..], a, b, c))
}

/// `WorldEmitter_Falloff`: the volume factor of distance fraction `x` for curve `kind` (0 square, 1 and 3 hyperbolic,
/// 2 linear, 4 squared linear, 5 flat to 0.7 then linear). Another kind returns `f3`, whatever the register held.
pub fn falloff(kind: f32, x: f32, f3: f32) -> f32 {
    let k = if kind.is_nan() { -1 } else { kind as i32 };
    match k {
        0 => 1.0 - x * x,
        1 => 1.0 - x / (1.5 - x * 0.5),
        2 => 1.0 - x,
        3 => {
            let t = 1.0 - x;
            t / (1.5 - t * 0.5)
        }
        4 => {
            let t = 1.0 - x;
            t * t
        }
        5 => {
            if x <= f32::from_bits(0x3f33_3333) {
                1.0
            } else {
                (1.0 - x) / f32::from_bits(0x3e99_999a)
            }
        }
        _ => f3,
    }
}

/// What the emitters call out to: the two voice engines' stop (0x20c2a0, 0x220f98: the global at 0x343050, the
/// handle, the fade 0.25), and the per-record update (0x22bf60).
pub trait Env {
    fn global(&mut self) -> u32;
    fn stop_a(&mut self, global: u32, handle: u32);
    fn stop_b(&mut self, global: u32, handle: u32);
    fn update(&mut self, index: usize);
}

/// `WorldEmitter_Stop`: stop the record's voice in the engine it is in and forget it.
pub fn stop(e: &mut [u32], env: &mut dyn Env) {
    let g = env.global();
    if e[0xe] == 0 {
        env.stop_a(g, e[0xc]);
    } else {
        env.stop_b(g, e[0xc]);
    }
    e[0x10] = 0xffff_ffff;
    e[0xc] = 0xffff_ffff;
}

/// `WorldEmitter_StopAll`: free and stop every playing record.
pub fn stop_all(t: &mut [u32], env: &mut dyn Env) {
    for i in 0..COUNT {
        if t[i * WORDS] == 1 {
            t[i * WORDS] = 0;
            stop(&mut t[i * WORDS..(i + 1) * WORDS], env);
        }
    }
}

/// `WorldEmitter_UpdateAll`: every playing record not marked wanted is freed and stopped, the others updated;
/// the wanted mark is cleared on each.
pub fn update_all(t: &mut [u32], env: &mut dyn Env) {
    for i in 0..COUNT {
        if t[i * WORDS] == 1 {
            if t[i * WORDS + 0xd] == 0 {
                t[i * WORDS] = 0;
                stop(&mut t[i * WORDS..(i + 1) * WORDS], env);
            } else {
                env.update(i);
            }
            t[i * WORDS + 0xd] = 0;
        }
    }
}

/// Reads of the game's memory by address (a word at a time), for the functions that walk the emitter grid.
pub trait Mem {
    fn u32(&self, addr: u32) -> u32;
    fn f32(&self, addr: u32) -> f32 {
        f32::from_bits(self.u32(addr))
    }
}

/// `WorldEmitter_GatherForListener`'s call out: `FUN_0022d260` (the volume the falloff gave, the object at 0x34370c (`global`),
/// the instance, the record's index, the caller's third argument, the record's word +4, 1 for a faded sound or 0, the
/// sound's centre).
pub trait GatherEnv {
    fn add_sound(&mut self, volume: f32, global: u32, instance: u32, index: u32, arg3: u64, record_word_1: u32, faded: u32, centre: [f32; 3]);
}

/// `Vec3_Dot` 0x137140.
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `WorldEmitter_GatherForListener` 0x22b370: the emitters near a listener at `listener` (the address of its three
/// floats) in the grid of the emitter manager at `sys` (+0x10 width, +0x14 height, +0x18 the cells of 0x14 bytes
/// whose +0x10 heads a list of {instance group, next}, +0x1c / +0x20 the grid's origin, +0x24 the cell size). Each
/// group is an instance (position at +0x30) and a block of records (count at +4, records from +8: kind 0 a sphere,
/// 1 a box, 2 a cone, 3 a plain radius; of 0x1c, 0x30, 0x30 and 0x18 bytes). A record the listener is inside gives
/// one `add_sound` with the volume the record's falloff curve gives. The pointers to the records are kept on the
/// original's stack four to a group: more than four records in a group is not modelled.
pub fn gather_for_listener(m: &dyn Mem, sys: u32, listener: u32, arg3: u64, env: &mut dyn GatherEnv) {
    let cell = m.f32(sys + 0x24);
    let lis = [m.f32(listener), m.f32(listener + 4), m.f32(listener + 8)];
    let w = m.u32(sys + 0x10) as i32;
    let yy = (((lis[1] - m.f32(sys + 0x20)) + cell) / cell) as i32;
    let xx = (((lis[0] - m.f32(sys + 0x1c)) + cell) / cell) as i32;
    let idx = w.wrapping_mul(yy.wrapping_sub(1)).wrapping_add(xx.wrapping_sub(1));
    if idx < 0 || idx >= w.wrapping_mul(m.u32(sys + 0x14) as i32) {
        return;
    }
    let mut node = m.u32((idx as u32).wrapping_mul(0x14).wrapping_add(m.u32(sys + 0x18)) + 0x10);
    if node == 0 {
        return;
    }
    let global = m.u32(0x34370c);
    let half_pi = f32::from_bits(0x3fc9_0fdb);
    while node != 0 {
        let group = m.u32(node);
        let inst = m.u32(group);
        let p = [m.f32(inst + 0x30), m.f32(inst + 0x34), m.f32(inst + 0x38)];
        let block = m.u32(group + 4);
        let count = m.u32(block + 4) as i32;
        // the records' addresses, by their kinds' sizes
        let mut recs = Vec::new();
        let mut at = block + 8;
        for _ in 0..count.max(0) {
            recs.push(at);
            at = at.wrapping_add(match m.u32(at) {
                0 => 0x1c,
                1 | 2 => 0x30,
                3 => 0x18,
                _ => 0,
            });
        }
        for (i, &r) in recs.iter().enumerate() {
            let i = i as u32;
            let off = [m.f32(r + 8), m.f32(r + 0xc), m.f32(r + 0x10)];
            let c = [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
            let d = [lis[0] - c[0], lis[1] - c[1], lis[2] - c[2]];
            let kind = m.u32(r);
            match kind {
                0 => {
                    let dist = dot3(d, d).sqrt().abs();
                    let rad = m.f32(r + 0x14);
                    if dist < rad {
                        let f = super::worldemitter::falloff(m.f32(r + 0x18), dist / rad, 0.0);
                        env.add_sound(f, global, inst, i, arg3, m.u32(r + 4), 1, c);
                    }
                }
                1 => {
                    let a = [m.f32(r + 0x20), m.f32(r + 0x24), m.f32(r + 0x28)];
                    // up x a, normalised, then a x that
                    let u = [0.0f32, 0.0, 1.0];
                    let r0 = u[1] * a[2] - u[2] * a[1];
                    let r1 = u[2] * a[0] - u[0] * a[2];
                    let r2 = u[0] * a[1] - u[1] * a[0];
                    let q = dot3([r0, r1, r2], [r0, r1, r2]).sqrt();
                    let inv = 1.0 / q;
                    let (n0, n1, n2) = (r0 * inv, r1 * inv, r2 * inv);
                    let s0 = a[1] * n2 - a[2] * n1;
                    let s1 = a[2] * n0 - a[0] * n2;
                    let s2 = a[0] * n1 - a[1] * n0;
                    let e5 = m.f32(r + 0x14);
                    let e6 = m.f32(r + 0x18);
                    let e7 = m.f32(r + 0x1c);
                    let sd = (s0 * d[0] + s1 * d[1]) + s2 * d[2];
                    let ad = (a[0] * d[0] + a[1] * d[1]) + a[2] * d[2];
                    let nd = (n0 * d[0] + n1 * d[1]) + n2 * d[2];
                    let t_a = if e5 == 0.0 { 0.0 } else { (ad * ad) / (e5 * e5) };
                    let t_n = if e6 == 0.0 { 0.0 } else { (nd * nd) / (e6 * e6) };
                    let t_s = if e7 == 0.0 { 0.0 } else { (sd * sd) / (e7 * e7) };
                    let total = (t_a + t_n) + t_s;
                    if total < 1.0 {
                        let f = super::worldemitter::falloff(m.f32(r + 0x2c), total, 0.0);
                        env.add_sound(f, global, inst, i, arg3, m.u32(r + 4), 1, c);
                    }
                }
                2 => {
                    let dist = dot3(d, d).sqrt().abs();
                    let rad = m.f32(r + 0x14);
                    if dist < rad {
                        let inv = 1.0 / dist;
                        let dn = [d[0] * inv, d[1] * inv, d[2] * inv];
                        let dir = [m.f32(r + 0x18), m.f32(r + 0x1c), m.f32(r + 0x20)];
                        let cosine = (dn[0] * dir[0] + dn[1] * dir[1]) + dn[2] * dir[2];
                        let cone = m.f32(r + 0x28);
                        if cone < cosine {
                            let f1 = super::worldemitter::falloff(m.f32(r + 0x24), dist / rad, 0.0);
                            let wide = half_pi - crate::math::asin(cone);
                            let ang = half_pi - crate::math::asin(cosine);
                            let f2 = super::worldemitter::falloff(m.f32(r + 0x2c), ang / wide, 0.0);
                            env.add_sound(f1 * f2, global, inst, i, arg3, m.u32(r + 4), 1, c);
                        }
                    }
                }
                3 => {
                    let dist = dot3(d, d).sqrt().abs();
                    if dist < m.f32(r + 0x14) {
                        env.add_sound(1.0, global, inst, i, arg3, m.u32(r + 4), 0, c);
                    }
                }
                _ => {}
            }
        }
        node = m.u32(node + 4);
    }
}

/// The game's `(int)(v * 127.0)` (`mul.s`, `cvt.w.s`: toward zero, out of range or NaN by the sign).
fn volume_int(v: f32) -> i32 {
    let f = v * 127.0;
    let bits = f.to_bits();
    if (bits >> 23) & 0xff >= 158 {
        if bits >> 31 != 0 {
            i32::MIN
        } else {
            i32::MAX
        }
    } else {
        f as i32
    }
}

/// What `WorldEmitter_Update` calls out to.
pub trait UpdateEnv: crate::sndqueue::QueueEnv {
    /// 0x221000 (the audio system, the handle, the volume).
    fn engine_volume(&mut self, handle: i32, volume: i32);
    /// `BankMonitor_Find` 0x20c200 (the audio system, the handle, 1): the monitor or 0.
    fn bank_find(&mut self, handle: i32) -> u32;
    /// A word stored to the monitor.
    fn poke(&mut self, addr: u32, v: u32);
    /// 0x22c638 (the record, its key).
    fn key_known(&mut self, addr: u32, key: u32) -> bool;
    /// 0x22c610 (the record, its key, two out words, the rider): the two words when it finds the sound.
    fn resolve(&mut self, addr: u32, key: u32, rider: u32) -> Option<(u32, u32)>;
    /// 0x22dc88 (the grid at 0x34370c, +4, +8, the record).
    fn grid_blocks(&mut self, a: u32, b: u32, addr: u32) -> bool;
    /// `*(gApp[0x730] + rider * 0x80 + 0xa4)`.
    fn rider_bank(&mut self, rider: u32) -> i32;
    /// `Audio_GetListenerRider` 0x20ed20 (the audio system, -1).
    fn listener_rider(&mut self) -> i64;
    /// 0x22dc38 (the grid, the key).
    fn ambient_blocked(&mut self, key: u32) -> bool;
    /// 0x22c648 (the record, the key, the record's +0x40): the bank written through the pointer.
    fn load_trigger_banks(&mut self, addr: u32, key: u32, bank: &mut u32) -> bool;
    /// `Audio_LoadAmbientBank` 0x21a548 (the audio system, the bank).
    fn load_ambient_bank(&mut self, bank: u32) -> i64;
    /// 0x22e240 (the grid, the key).
    fn has_queued(&mut self, key: u32) -> bool;
    /// 0x22e270 (the grid, +4).
    fn is_queued_other(&mut self, a: u32) -> bool;
    /// The audio system's +0x372c.
    fn voice_table(&mut self) -> u32;
    /// 0x21fc98 (the table entry, the audio system, the second word).
    fn voice_start(&mut self, entry: u32, arg: u32) -> i64;
}

/// 0x2212a0: start the voice of the last queued entry and drop the entry; the handle (-1 without a table).
fn finish_voice(q: &mut crate::sndqueue::Queue, env: &mut dyn UpdateEnv, arg: u32) -> u32 {
    let table = env.voice_table();
    if table == 0 {
        return 0xffff_ffff;
    }
    let i = q.count as usize;
    let r = env.voice_start(q.a68[i].wrapping_mul(400).wrapping_add(table), arg);
    let mut v = r as u32;
    if r >= 0 {
        v = (r as u32) << 8 | q.a68[i];
    }
    q.count -= 1;
    v
}

/// `WorldEmitter_Update` 0x22bf60: keep a record's voice going. With a voice handle: pass the volume on to the
/// engine it is in. Without one: look the sound up and either put a voice on the queue (the engine kind, at once)
/// or queue a sound (the other kind, behind the grid's checks and the ambient bank for kind 6), or free the
/// record when anything refuses. `addr` is the record's address.
pub fn update(e: &mut [u32], addr: u32, q: &mut crate::sndqueue::Queue, env: &mut dyn UpdateEnv) {
    use crate::sndqueue::{queue_begin, queue_push, queue_volume};
    let handle = e[0xc];
    if handle != 0xffff_ffff {
        let v = volume_int(f32::from_bits(e[4]));
        if e[0xe] != 0 {
            env.engine_volume(handle as i32, v);
            return;
        }
        let b = env.bank_find(handle as i32);
        if b != 0 {
            env.poke(b.wrapping_add(0x64), v as u32);
        }
        return;
    }
    if env.key_known(addr, e[3]) {
        let Some((_, arg)) = env.resolve(addr, e[3], e[0xf]) else {
            clear(e);
            return;
        };
        e[0xe] = 1;
        let i = queue_push(q);
        queue_volume(q, i, volume_int(f32::from_bits(e[4])) as i64);
        q.a40[i] = env.get_slot(4);
        if e[5] != 0 {
            q.a78[i] = 1;
            q.emitter[i] = addr.wrapping_add(0x20);
            q.a7c[i] = 0;
        }
        q.a68[i] = e[0xf];
        e[0xc] = finish_voice(q, env, arg);
        return;
    }
    if env.grid_blocks(e[1], e[2], addr) {
        clear(e);
        return;
    }
    let Some((kind, id)) = env.resolve(addr, e[3], e[0xf]) else {
        clear(e);
        return;
    };
    e[0xe] = 0;
    let mut loaded: i64 = -1;
    if kind == 6 {
        let mine = env.rider_bank(e[0xf]) as i64;
        let listener = env.listener_rider();
        if mine != listener || env.ambient_blocked(e[3]) {
            clear(e);
            return;
        }
        if e[0x10] == 0xffff_ffff {
            let mut bank = e[0x10];
            let ok = env.load_trigger_banks(addr, e[3], &mut bank);
            e[0x10] = bank;
            if !ok {
                clear(e);
                return;
            }
        }
        loaded = env.load_ambient_bank(e[0x10]);
    }
    if env.has_queued(e[3]) && !env.is_queued_other(e[1]) {
        clear(e);
        return;
    }
    if kind == 6 && loaded != 1 {
        return;
    }
    let i = queue_begin(q, kind, id, Some(volume_int(f32::from_bits(e[4])) as i64));
    q.a40[i] = env.get_slot(4);
    if e[5] != 0 {
        q.a78[i] = 1;
        q.emitter[i] = addr.wrapping_add(0x20);
        q.a7c[i] = 0;
    }
    e[0xc] = env.play_pending() as u32;
}
