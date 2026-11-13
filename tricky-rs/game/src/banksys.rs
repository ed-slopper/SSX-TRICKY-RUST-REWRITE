//! The bank manager and its loader (board row E1b2g7d5c): `cAsyncSys` (the queue of bank loads read from the disc a
//! file at a time) and `cBankManager` on it (the bank slots, 0x60 each), on the game's bytes (`pf::Mem`), checked in
//! the function runner (`tests/banksys.rs`). Both are bases of the audio system (`DAT_00343050`): `gAsyncSys`
//! 0x342c40 and `gBankManager` 0x342c44 point at it. What they call out to (EA's memory, the file and async file
//! readers, the sound library's banks, the class's virtuals other than the bank manager's own) goes through
//! [`SongEnv`].
//!
//! The loader (`cAsyncSys`): +4 its vtable, +0x8e4 the file being read (then the bank's handle), +0x8e8 busy, +0x8ec
//! the next name buffer (0..3), +0x8f0 four names (0x40 each), four ring queues {size, data, write, read} of the
//! loads: +0x9f0 the slot, +0xa00 the name buffer, +0xa10 how the slot keeps its memory, +0xa20 the memory flags;
//! +0xa30 / +0xa34 the shared load buffer and its size.
//!
//! The bank manager: +0xa38 the slot count, +0xa3c the slots. A slot: +0 its state (0 none, 1 loaded, 2 loading),
//! +4 the bank's handle, +8 how it keeps its memory (0 allocated per load, 1 a fixed buffer, 2 a fixed buffer and sound
//! memory), +0xc the sound memory's block, +0x10 / +0x14 the buffer and its size, +0x18 / +0x1c the ring of slots
//! sharing it, +0x20 the bank's name.

use crate::musicsong::SongEnv;
use crate::pf::Mem;

pub const ASYNC_SYS: u32 = 0x342c40;
pub const BANK_MANAGER: u32 = 0x342c44;
pub const ASYNC_VTABLE: u32 = 0x39e630;
pub const MANAGER_VTABLE: u32 = 0x39e6f8;
/// EA's memory names: "AsyncSys"-side queues, the load buffer, the static and dynamic sound memory, the slots
pub const NAME_QUEUE: u32 = 0x39e610;
pub const NAME_BUFFER: u32 = 0x39e620;
pub const NAME_STATIC: u32 = 0x39e6c8;
pub const NAME_DYNAMIC: u32 = 0x39e6d8;
pub const NAME_SLOTS: u32 = 0x39e6e8;
/// The pool `AsyncSys_AllocBuffer` takes from when asked (flag 0x40000000)
pub const POOL: u32 = 0x345e48;

pub const STRCPY: u32 = 0x2fd14c;
pub const STRCMP: u32 = 0x2fd008;
pub const EAMEM_SET: u32 = 0x2cc808;
pub const EAMEM_COPY: u32 = 0x2ce114;
pub const EAMEM_ALLOC: u32 = 0x2ccf70;
pub const EAMEM_FREE: u32 = 0x2ccfc0;
pub const EAMEM_GET_SIZE: u32 = 0x2cd9b0;
pub const EAMEM_RESIZE: u32 = 0x2cda18;
pub const MEM_DELETE: u32 = 0x114f80;
pub const MEM_FREE: u32 = 0x14a040;
pub const MEMPOOL_ALLOC: u32 = 0x23d3b0;
pub const TIMER_WAIT: u32 = 0x2cfbb0;
pub const FILE_LOAD_ALLOC: u32 = 0x2c8068;
pub const FILE_LOAD_INTO: u32 = 0x2c8348;
pub const ASYNC_LOAD: u32 = 0x2c5e20;
pub const ASYNC_LOAD_INTO: u32 = 0x2c5ed0;
pub const ASYNC_IS_DONE: u32 = 0x2c6108;
/// `AsyncFile_Finish` (the file, two words' addresses): asked through `call_out` with the file alone, it answers the
/// two words (the data, its size)
pub const ASYNC_FINISH: u32 = 0x2c6160;
pub const ASYNC_CANCEL: u32 = 0x2c6278;
/// `SndBank_Register` (the handle's address, the bank's data): asked through `call_out` with the data alone, it
/// answers [what it returns, the handle it writes]
pub const BANK_REGISTER: u32 = 0x2d0f98;
pub const BANK_UNREGISTER: u32 = 0x2d1200;
pub const BANK_RESIDENT_SIZE: u32 = 0x2d1428;
pub const BANK_RELOCATE: u32 = 0x2d1448;
pub const BANK_IS_PLAYING: u32 = 0x2d14d0;
pub const SND_LOCK: u32 = 0x2d0908;
pub const SND_UNLOCK: u32 = 0x2d0938;
pub const MONITOR_INIT: u32 = 0x20bb98;
pub const MONITOR_DTOR: u32 = 0x20bc60;
pub const RING_DESTROY: u32 = 0x209b40;

/// The bank manager's virtuals the loader calls (its vtable +0x50 / +0x58 / +0x60): run here as ported
pub const VF_RELEASE: u32 = 0x20a640;
pub const VF_ALLOC: u32 = 0x20a658;
pub const VF_CONFIGURE: u32 = 0x20a688;

const NONE: u32 = u32::MAX;

/// The queues' places in the loader
const SLOTS: u32 = 0x9f0;
const NAMES: u32 = 0xa00;
const KINDS: u32 = 0xa10;
const FLAGS: u32 = 0xa20;
const QUEUES: [u32; 4] = [SLOTS, NAMES, KINDS, FLAGS];

/// A ring queue's push (the code inlined everywhere): dropped when full
fn push(m: &mut Mem, q: u32, v: u32) {
    let size = m.r32(q) as i32;
    assert!(size != 0, "the original traps on a zero-sized queue");
    let w = m.r32(q + 8) as i32;
    if w.wrapping_add(1) % size != m.r32(q + 0xc) as i32 {
        let data = m.r32(q + 4);
        m.w32(data.wrapping_add((w as u32).wrapping_mul(4)), v);
        let w = w.wrapping_add(1);
        m.w32(q + 8, if size <= w { 0 } else { w as u32 });
    }
}

/// The queue's front, −1 when empty
fn peek(m: &Mem, q: u32) -> u32 {
    let r = m.r32(q + 0xc);
    if m.r32(q + 8) == r {
        NONE
    } else {
        m.r32(m.r32(q + 4).wrapping_add(r.wrapping_mul(4)))
    }
}

/// The front taken off (−1 when empty)
fn pop(m: &mut Mem, q: u32) -> u32 {
    let r = m.r32(q + 0xc);
    if m.r32(q + 8) == r {
        return NONE;
    }
    let v = m.r32(m.r32(q + 4).wrapping_add(r.wrapping_mul(4)));
    let r = (r as i32).wrapping_add(1);
    m.w32(q + 0xc, if m.r32(q) as i32 <= r { 0 } else { r as u32 });
    v
}

/// `strcpy` 0x2fd14c
pub fn strcpy(m: &mut Mem, to: u32, from: u32) {
    let mut k = 0;
    loop {
        let b = m.r8(from.wrapping_add(k));
        m.w8(to.wrapping_add(k), b);
        if b == 0 {
            break;
        }
        k += 1;
    }
}

fn strcmp_eq(m: &Mem, a: u32, b: u32) -> bool {
    let mut k = 0;
    loop {
        let (x, y) = (m.r8(a.wrapping_add(k)), m.r8(b.wrapping_add(k)));
        if x != y {
            return false;
        }
        if x == 0 {
            return true;
        }
        k += 1;
    }
}

/// A virtual call on the object `obj` whose vtable is at `obj + at`: the entry at `off` (its this-adjust, a short) and
/// `off + 4` (the function). The bank manager's own three run as ported.
fn vcall(m: &mut Mem, e: &mut dyn SongEnv, obj: u32, at: u32, off: u32, args: &[u32]) -> u32 {
    let vt = m.r32(obj.wrapping_add(at));
    let this = obj.wrapping_add(m.r16(vt.wrapping_add(off)) as i16 as i32 as u32);
    let f = m.r32(vt.wrapping_add(off + 4));
    let a = |k: usize| args.get(k).copied().unwrap_or(0);
    match f {
        VF_RELEASE => {
            manager_release(m, e, this, a(0));
            0
        }
        VF_ALLOC => manager_alloc(m, e, this, a(0), a(1), a(2)),
        VF_CONFIGURE => {
            manager_configure(m, e, this, a(0), a(1), a(2), a(3));
            0
        }
        _ => {
            let mut all = vec![this];
            all.extend_from_slice(args);
            e.call(m, f, &all)
        }
    }
}

/// A call on the object `gBankManager` keeps at its +0 (the sound memory's owner; vtable at its +0)
fn owner_call(m: &mut Mem, e: &mut dyn SongEnv, off: u32, args: &[u32]) -> u32 {
    let mgr = m.r32(BANK_MANAGER);
    let obj = m.r32(mgr);
    vcall(m, e, obj, 0, off, args)
}

fn name(sys: u32, index: u32) -> u32 {
    sys.wrapping_add(index.wrapping_mul(0x40)).wrapping_add(0x8f0)
}

/// `AsyncSys_Init` 0x208950 (the system, a word for the monitor): the monitor's init, the four queues of 4 (EA's
/// memory, filled with 0xdeadc0ed), `gAsyncSys`, no load buffer.
pub fn async_init(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, arg: u32) -> u32 {
    e.call(m, MONITOR_INIT, &[sys, arg]);
    m.w32(sys + SLOTS, 4);
    m.w32(sys + 0x8e8, 0);
    m.w32(sys + 0x8ec, 0);
    m.w32(sys + SLOTS + 8, 0);
    m.w32(sys + SLOTS + 0xc, 0);
    m.w32(sys + 4, ASYNC_VTABLE);
    for (k, q) in QUEUES.iter().enumerate() {
        let n = m.r32(sys + q) << 2;
        let p = e.call(m, EAMEM_ALLOC, &[NAME_QUEUE, n, 0]);
        if p != 0 {
            e.call(m, EAMEM_SET, &[p, 0xdead_c0ed, n]);
        }
        if let Some(next) = QUEUES.get(k + 1) {
            m.w32(sys + next, 4);
            m.w32(sys + next + 8, 0);
            m.w32(sys + next + 0xc, 0);
        }
        m.w32(sys + q + 4, p);
    }
    m.w32(ASYNC_SYS, sys);
    m.w32(sys + 0xa30, 0);
    m.w32(sys + 0xa34, 0);
    sys
}

/// `cAsyncSys::~cAsyncSys` 0x208af8 (the system, the delete flags)
pub fn async_destroy(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, flags: u32) {
    m.w32(sys + 4, ASYNC_VTABLE);
    async_shutdown(m, e, sys);
    m.w32(ASYNC_SYS, 0);
    for q in [FLAGS, KINDS, NAMES, SLOTS] {
        e.call(m, RING_DESTROY, &[sys + q, 2]);
    }
    e.call(m, MONITOR_DTOR, &[sys, flags]);
}

/// `AsyncSys_AllocBuffer` 0x208b78 (the system, the size, flags): the shared load buffer, from EA's memory (flags'
/// low 26 bits) or the pool (0x40000000); zeroed when the top bit is set, else filled with 0xdeadc0ed.
pub fn async_alloc_buffer(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, size: u32, flags: u32) {
    if size == 0 {
        return;
    }
    m.w32(sys + 0xa34, size);
    let p = if flags & 0x4000_0000 == 0 {
        e.call(m, EAMEM_ALLOC, &[NAME_BUFFER, size, flags & 0x3ff_ffff])
    } else {
        e.call(m, MEMPOOL_ALLOC, &[POOL, size, flags, NAME_BUFFER, 0, 0])
    };
    if p == 0 {
        m.w32(sys + 0xa30, 0);
    } else {
        e.call(m, EAMEM_SET, &[p, if (flags as i32) < 0 { 0 } else { 0xdead_c0ed }, size]);
        m.w32(sys + 0xa30, p);
    }
}

/// `AsyncSys_Shutdown` 0x208c48: every load cancelled (waiting two ticks a time while one is busy), the buffer freed
pub fn async_shutdown(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    async_pop_queue(m, e, sys, 1);
    while m.r32(sys + 0x8e8) != 0 {
        e.call(m, TIMER_WAIT, &[2]);
        async_pop_queue(m, e, sys, 1);
    }
    let buf = m.r32(sys + 0xa30);
    if buf != 0 {
        e.call(m, MEM_FREE, &[buf]);
        m.w32(sys + 0xa34, 0);
        m.w32(sys + 0xa30, 0);
    }
}

/// `AsyncSys_StartNext` 0x208cb0: when idle and a load is queued, it starts: the slot released (vtable +0x50); a slot
/// of kind 1 asks for its memory (+0x58, size −1) and reads into it, or into memory the reader allocates; kinds 0 and
/// 2 read into the shared buffer.
pub fn async_start_next(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    if m.r32(sys + 0x8e8) != 0 || m.r32(sys + SLOTS + 8) == m.r32(sys + SLOTS + 0xc) {
        return;
    }
    m.w32(sys + 0x8e8, 1);
    let slot = peek(m, sys + SLOTS);
    vcall(m, e, sys, 4, 0x50, &[slot]);
    match peek(m, sys + KINDS) {
        1 => {
            let file = name(sys, peek(m, sys + NAMES));
            let (slot, flags) = (peek(m, sys + SLOTS), peek(m, sys + FLAGS));
            let at = vcall(m, e, sys, 4, 0x58, &[slot, NONE, flags]);
            let h = if at == 0 {
                let flags = peek(m, sys + FLAGS);
                e.call(m, ASYNC_LOAD, &[file, flags])
            } else {
                e.call(m, ASYNC_LOAD_INTO, &[file, at, 0])
            };
            m.w32(sys + 0x8e4, h);
        }
        0 | 2 => {
            let file = name(sys, peek(m, sys + NAMES));
            let buf = m.r32(sys + 0xa30);
            let h = e.call(m, ASYNC_LOAD_INTO, &[file, buf, 0]);
            m.w32(sys + 0x8e4, h);
        }
        _ => {}
    }
}

/// `AsyncSys_QueueLoad` 0x208ed8 (the system, the slot, the file's name, the slot's kind, memory flags): queued, the
/// name copied into the next of the four buffers, and started if idle.
pub fn async_queue_load(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, slot: u32, file: u32, kind: u32, flags: u32) {
    push(m, sys + SLOTS, slot);
    push(m, sys + KINDS, kind);
    push(m, sys + FLAGS, flags);
    let i = m.r32(sys + 0x8ec);
    strcpy(m, name(sys, i), file);
    m.w32(sys + 0x8ec, i.wrapping_add(1));
    push(m, sys + NAMES, i);
    if 3 < m.r32(sys + 0x8ec) as i32 {
        m.w32(sys + 0x8ec, 0);
    }
    async_start_next(m, e, sys);
}

/// The front load done (or failed): the slot told through the vtable +0x60 (the slot, the handle or −1, 0 loaded /
/// 1 cancelled / 2 failed, the name, the data, its size)
fn told(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, handle: u32, status: u32, data: u32, size: u32) {
    let slot = peek(m, sys + SLOTS);
    let file = name(sys, peek(m, sys + NAMES));
    vcall(m, e, sys, 4, 0x60, &[slot, handle, status, file, data, size]);
}

/// `AsyncSys_PopQueue` 0x2086d8 (the system, all): the front load taken off (cancelled if it was being read, its slot
/// told 1 when the cancel took, else 2); with `all` every queue emptied; the next started.
pub fn async_pop_queue(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, all: u32) {
    let slot = pop(m, sys + SLOTS);
    let file = pop(m, sys + NAMES);
    let kind = pop(m, sys + KINDS);
    pop(m, sys + FLAGS);
    if m.r32(sys + 0x8e8) != 0 && kind <= 2 {
        let h = m.r32(sys + 0x8e4);
        let r = e.call(m, ASYNC_CANCEL, &[h]);
        let h = m.r32(sys + 0x8e4);
        e.call(m, ASYNC_FINISH, &[h, 0, 0]);
        let status = if r == 1 { 1 } else { 2 };
        vcall(m, e, sys, 4, 0x60, &[slot, NONE, status, name(sys, file), 0, 0]);
        m.w32(sys + 0x8e8, 0);
    }
    if all != 0 {
        for q in QUEUES {
            let w = m.r32(sys + q + 8);
            m.w32(sys + q + 0xc, w);
        }
    }
    async_start_next(m, e, sys);
}

/// `AsyncSys_PopNext` 0x209aa0
pub fn async_pop_next(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    async_pop_queue(m, e, sys, 0);
}

/// `AsyncSys_CancelAll` 0x209a98
pub fn async_cancel_all(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    async_pop_queue(m, e, sys, 1);
}

/// `AsyncSys_CancelBank` 0x209088 (the system, the slot): the slot's loads taken out of the queue (the one being read
/// cancelled through `AsyncSys_PopNext`), the others put back in order; the next started when idle.
pub fn async_cancel_bank(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, slot: u32) {
    if m.r32(sys + 0x8e8) != 0 && peek(m, sys + SLOTS) == slot {
        async_pop_next(m, e, sys);
        return;
    }
    let size = m.r32(sys + SLOTS) as i32;
    assert!(size != 0, "the original traps on a zero-sized queue");
    let w = m.r32(sys + SLOTS + 8) as i32;
    let n = (w.wrapping_sub(m.r32(sys + SLOTS + 0xc) as i32).wrapping_add(size)) % size;
    for _ in 0..n.max(0) {
        let v = pop(m, sys + SLOTS);
        if v == slot {
            for q in [NAMES, KINDS, FLAGS] {
                pop(m, sys + q);
            }
        } else {
            push(m, sys + SLOTS, v);
            for q in [NAMES, KINDS, FLAGS] {
                let x = pop(m, sys + q);
                push(m, sys + q, x);
            }
        }
    }
    if m.r32(sys + 0x8e8) == 0 {
        async_start_next(m, e, sys);
    }
}

/// `AsyncSys_Update` 0x209408: the file being read checked; when done, its bank registered (a kind-1 slot's in
/// place; the shared buffer's moved to the slot's memory: relocated when the bank is resident (7), else copied) and the
/// slot told; failed reads pop the load; then the next started.
pub fn async_update(m: &mut Mem, e: &mut dyn SongEnv, sys: u32) {
    if m.r32(sys + 0x8e8) == 0 {
        return;
    }
    let kind = peek(m, sys + KINDS);
    if kind > 2 {
        return;
    }
    let h = m.r32(sys + 0x8e4);
    let r = e.call(m, ASYNC_IS_DONE, &[h]);
    if r == 0 {
        return;
    }
    if r != 1 {
        async_pop_next(m, e, sys);
        return;
    }
    let h = m.r32(sys + 0x8e4);
    let out = e.call_out(m, ASYNC_FINISH, &[h], 2);
    let (data, size) = (out.first().copied().unwrap_or(0), out.get(1).copied().unwrap_or(0));
    if kind == 1 {
        if (size as i32) < 1 {
            told(m, e, sys, NONE, 2, data, 0);
        } else {
            register(m, e, sys + 0x8e4, data);
            let h = m.r32(sys + 0x8e4);
            told(m, e, sys, h, 0, data, size);
        }
    } else if (size as i32) < 1 {
        told(m, e, sys, NONE, 2, 0, 0);
    } else if register(m, e, sys + 0x8e4, data) == 7 {
        let h = m.r32(sys + 0x8e4);
        let resident = e.call(m, BANK_RESIDENT_SIZE, &[h]);
        let (slot, flags) = (peek(m, sys + SLOTS), peek(m, sys + FLAGS));
        let at = vcall(m, e, sys, 4, 0x58, &[slot, resident, flags]);
        let h = m.r32(sys + 0x8e4);
        e.call(m, BANK_RELOCATE, &[at, h]);
        let h = m.r32(sys + 0x8e4);
        told(m, e, sys, h, 0, at, resident);
    } else {
        let h = m.r32(sys + 0x8e4);
        e.call(m, BANK_UNREGISTER, &[h]);
        let (slot, flags) = (peek(m, sys + SLOTS), peek(m, sys + FLAGS));
        let at = vcall(m, e, sys, 4, 0x58, &[slot, size, flags]);
        e.call(m, EAMEM_COPY, &[at, data, size]);
        register(m, e, sys + 0x8e4, at);
        let h = m.r32(sys + 0x8e4);
        told(m, e, sys, h, 0, at, size);
    }
    m.w32(sys + 0x8e8, 0);
    for q in QUEUES {
        pop(m, sys + q);
    }
    async_start_next(m, e, sys);
}

/// `SndBank_Register` (the handle's address, the data): the handle written, its answer returned
fn register(m: &mut Mem, e: &mut dyn SongEnv, at: u32, data: u32) -> u32 {
    let out = e.call_out(m, BANK_REGISTER, &[data], 2);
    m.w32(at, out.get(1).copied().unwrap_or(0));
    out.first().copied().unwrap_or(0)
}

/// `BankSlot_Init` 0x209c18
pub fn slot_init(m: &mut Mem, s: u32) -> u32 {
    m.w32(s + 0xc, NONE);
    m.w32(s, 0);
    m.w32(s + 4, NONE);
    for k in [8, 0x10, 0x14, 0x18, 0x1c] {
        m.w32(s + k, 0);
    }
    m.w8(s + 0x20, 0);
    s
}

/// `BankSlot_Destroy` 0x209c48 (the slot, the delete flags)
pub fn slot_destroy(m: &mut Mem, e: &mut dyn SongEnv, s: u32, flags: u32) {
    slot_unload(m, e, s);
    slot_unlink(m, s);
    slot_free(m, e, s);
    if flags & 1 != 0 {
        e.call(m, MEM_DELETE, &[s]);
    }
}

/// `BankInstance_UnloadChain` 0x209ca8: every other slot in the ring unloaded
pub fn slot_unload_chain(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    let mut n = m.r32(s + 0x18);
    if n == 0 {
        return;
    }
    while n != s {
        slot_unload(m, e, n);
        n = m.r32(n + 0x18);
    }
}

/// `BankSlot_Load` 0x209cf8 (the slot, the file's name, memory flags): the bank read now (not queued), registered, the
/// slot loaded (1) or emptied.
pub fn slot_load(m: &mut Mem, e: &mut dyn SongEnv, s: u32, file: u32, flags: u32) {
    slot_unload(m, e, s);
    slot_unload_chain(m, e, s);
    let mut data = 0;
    let mut kind = m.r32(s + 8);
    match kind {
        1 => {
            let (at, size) = (m.r32(s + 0x10), m.r32(s + 0x14));
            if e.call(m, FILE_LOAD_INTO, &[file, at, size]) == 0 {
                return;
            }
            data = m.r32(s + 0x10);
            kind = m.r32(s + 8);
        }
        0 | 2 => {
            data = e.call(m, FILE_LOAD_ALLOC, &[file, if kind == 0 { flags } else { 0x10 }]);
            if data == 0 {
                return;
            }
            kind = m.r32(s + 8);
        }
        _ => {}
    }
    if kind == 2 {
        let block = m.r32(s + 0xc);
        owner_call(m, e, 0x30, &[block]);
    }
    let r = register(m, e, s + 4, data);
    if r == 7 {
        let h = m.r32(s + 4);
        let size = e.call(m, BANK_RESIDENT_SIZE, &[h]);
        if m.r32(s + 8) == 2 {
            let (at, h) = (m.r32(s + 0x10), m.r32(s + 4));
            e.call(m, BANK_RELOCATE, &[at, h]);
        } else {
            m.w32(s + 0x10, data);
            e.call(m, EAMEM_RESIZE, &[data, size]);
            m.w32(s + 0x14, size);
        }
        m.w32(s, 1);
    } else if r == 8 {
        if m.r32(s + 8) == 0 {
            m.w32(s + 0x10, data);
            let size = e.call(m, EAMEM_GET_SIZE, &[data]);
            m.w32(s + 0x14, size);
        }
        m.w32(s, 1);
    } else {
        if m.r32(s + 8) != 1 {
            e.call(m, EAMEM_FREE, &[data]);
        }
        m.w32(s, 0);
    }
    owner_call(m, e, 0x38, &[]);
    strcpy(m, s + 0x20, file);
}

/// `BankInstance_Unload` 0x209f10: a loaded slot's bank unregistered, its memory freed when allocated per load
pub fn slot_unload(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    if m.r32(s) != 1 {
        return;
    }
    m.w32(s, 0);
    let h = m.r32(s + 4);
    e.call(m, BANK_UNREGISTER, &[h]);
    m.w32(s + 4, NONE);
    if m.r32(s + 8) == 0 {
        let p = m.r32(s + 0x10);
        e.call(m, EAMEM_FREE, &[p]);
        m.w32(s + 0x14, 0);
        m.w32(s + 0x10, 0);
    }
}

/// `BankInstance_AllocStatic` 0x209f70 (the slot, the size, the sound memory wanted, flags): a fixed buffer; with
/// sound memory a block of it from the owner (+0x20; −3 means none left: the buffer takes its size too).
pub fn slot_alloc_static(m: &mut Mem, e: &mut dyn SongEnv, s: u32, size: u32, spu: u32, flags: u32) {
    if size == 0 && spu == 0 {
        return;
    }
    let mut size = size;
    m.w32(s + 8, 1);
    if spu != 0 {
        let r = owner_call(m, e, 0x20, &[spu]) as i32;
        if r < 0 {
            if r == -3 {
                size = size.wrapping_add(spu);
            }
        } else {
            m.w32(s + 0xc, r as u32);
            m.w32(s + 8, 2);
        }
    }
    m.w32(s + 0x14, size);
    let p = e.call(m, EAMEM_ALLOC, &[NAME_STATIC, size, flags]);
    m.w32(s + 0x10, p);
}

/// `BankSlot_Free` 0x20a038: unloaded and its fixed memory given back
pub fn slot_free(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    slot_unload(m, e, s);
    if m.r32(s + 8) == 0 {
        return;
    }
    if m.r32(s + 8) == 2 {
        let block = m.r32(s + 0xc);
        owner_call(m, e, 0x28, &[block]);
        m.w32(s + 0xc, NONE);
    }
    let p = m.r32(s + 0x10);
    e.call(m, EAMEM_FREE, &[p]);
    m.w32(s + 8, 0);
    m.w32(s + 0x10, 0);
    m.w32(s + 0x14, 0);
}

/// `BankSlot_Link` 0x20a0b0 (the slot, the head): put in the head's ring, sharing its memory
pub fn slot_link(m: &mut Mem, s: u32, head: u32) {
    if m.r32(head + 0x18) == 0 {
        m.w32(head + 0x18, s);
        m.w32(head + 0x1c, s);
        m.w32(s + 0x18, head);
        m.w32(s + 0x1c, head);
    } else {
        m.w32(s + 0x18, head);
        let prev = m.r32(head + 0x1c);
        m.w32(s + 0x1c, prev);
        m.w32(head + 0x1c, s);
        let prev = m.r32(s + 0x1c);
        m.w32(prev + 0x18, s);
    }
    let kind = m.r32(head + 8);
    if kind != 0 {
        let (buf, size, block) = (m.r32(head + 0x10), m.r32(head + 0x14), m.r32(head + 0xc));
        m.w32(s + 0x14, size);
        m.w32(s + 8, kind);
        m.w32(s + 0x10, buf);
        m.w32(s + 0xc, block);
    }
}

/// `BankSlot_Unlink` 0x20a118
pub fn slot_unlink(m: &mut Mem, s: u32) {
    let next = m.r32(s + 0x18);
    if next == 0 {
        return;
    }
    m.w32(s + 0xc, NONE);
    m.w32(s + 0x10, 0);
    m.w32(s + 0x14, 0);
    m.w32(s + 8, 0);
    if next == m.r32(s + 0x1c) {
        m.w32(next + 0x18, 0);
        let n = m.r32(s + 0x18);
        m.w32(n + 0x1c, 0);
    } else {
        let prev = m.r32(s + 0x1c);
        m.w32(next + 0x1c, prev);
        let (prev, next) = (m.r32(s + 0x1c), m.r32(s + 0x18));
        m.w32(prev + 0x18, next);
    }
    m.w32(s + 0x18, 0);
    m.w32(s + 0x1c, 0);
}

/// `BankSlot_IsLoaded` 0x20a170 (the slot, a name): loaded with that name
pub fn slot_is_loaded(m: &Mem, s: u32, file: u32) -> bool {
    m.r32(s) == 1 && strcmp_eq(m, file, s + 0x20)
}

/// `BankInstance_Release` 0x20a1b8: unloaded with its ring, marked loading (2), its sound memory's block handed to the
/// owner (+0x30)
pub fn slot_release(m: &mut Mem, e: &mut dyn SongEnv, s: u32) {
    slot_unload(m, e, s);
    slot_unload_chain(m, e, s);
    m.w32(s, 2);
    if m.r32(s + 8) == 2 {
        let block = m.r32(s + 0xc);
        owner_call(m, e, 0x30, &[block]);
    }
}

/// `BankInstance_AllocDynamic` 0x20a218 (the slot, the size, flags): a slot allocated per load gets its memory; the
/// slot's buffer returned
pub fn slot_alloc_dynamic(m: &mut Mem, e: &mut dyn SongEnv, s: u32, size: u32, flags: u32) -> u32 {
    if m.r32(s + 8) == 0 {
        let p = e.call(m, EAMEM_ALLOC, &[NAME_DYNAMIC, size, flags]);
        m.w32(s + 0x10, p);
        m.w32(s + 0x14, size);
    }
    m.r32(s + 0x10)
}

/// `BankSlot_Configure` 0x20a268 (the slot, the handle, the status, the name): a finished load (status 0 loaded)
pub fn slot_configure(m: &mut Mem, e: &mut dyn SongEnv, s: u32, handle: u32, status: u32, file: u32) {
    owner_call(m, e, 0x38, &[]);
    m.w32(s + 4, handle);
    m.w32(s, (status == 0) as u32);
    strcpy(m, s + 0x20, file);
}

fn slot(m: &Mem, mgr: u32, n: u32) -> u32 {
    n.wrapping_mul(0x60).wrapping_add(m.r32(mgr + 0xa3c))
}

/// `BankManager_Init` 0x20a2f0 (the manager, the slot count, the monitor's word): the loader, then the slots (EA's
/// memory: a count word, 0x10 bytes, the slots), `gBankManager`.
pub fn manager_init(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, count: u32, arg: u32) -> u32 {
    async_init(m, e, mgr, arg);
    m.w32(mgr + 0xa38, count);
    m.w32(mgr + 4, MANAGER_VTABLE);
    let n = count.wrapping_mul(0x60).wrapping_add(0x10);
    let p = e.call(m, EAMEM_ALLOC, &[NAME_SLOTS, n, 0]);
    if p != 0 {
        e.call(m, EAMEM_SET, &[p, 0xdead_c0ed, n]);
    }
    m.w32(p, count);
    let slots = p.wrapping_add(0x10);
    for k in 0..count {
        slot_init(m, slots + k * 0x60);
    }
    m.w32(mgr + 0xa3c, slots);
    m.w32(BANK_MANAGER, mgr);
    mgr
}

/// `cBankManager::~cBankManager` 0x20a3d0 (the manager, the delete flags): the slots destroyed last to first
pub fn manager_destroy(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, flags: u32) {
    m.w32(mgr + 4, MANAGER_VTABLE);
    let mut slots = m.r32(mgr + 0xa3c);
    m.w32(BANK_MANAGER, 0);
    if slots != 0 {
        let mut s = m.r32(slots.wrapping_sub(0x10)).wrapping_mul(0x60).wrapping_add(slots);
        while slots != s {
            s = s.wrapping_sub(0x60);
            slot_destroy(m, e, s, 0);
            slots = m.r32(mgr + 0xa3c);
        }
        e.call(m, MEM_FREE, &[slots.wrapping_sub(0x10)]);
    }
    async_destroy(m, e, mgr, flags);
}

/// `BankManager_LoadIfChanged` 0x20a460 (the manager, the slot, the name, flags): read now unless already loaded
pub fn manager_load_now(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32, file: u32, flags: u32) {
    let s = slot(m, mgr, n);
    if !slot_is_loaded(m, s, file) {
        async_cancel_bank(m, e, mgr, n);
        let s = slot(m, mgr, n);
        slot_load(m, e, s, file, flags);
    }
}

/// `BankManager_UnloadBank` 0x20a508 (the manager, the slot)
pub fn manager_unload(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32) {
    let s = slot(m, mgr, n);
    slot_unload(m, e, s);
}

/// `BankManager_LoadBank` 0x20a520 (the manager, the slot, the name, flags): queued unless already loaded
pub fn manager_load(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32, file: u32, flags: u32) {
    let s = slot(m, mgr, n);
    if !slot_is_loaded(m, s, file) {
        async_cancel_bank(m, e, mgr, n);
        let kind = m.r32(slot(m, mgr, n) + 8);
        async_queue_load(m, e, mgr, n, file, kind, flags);
    }
}

/// `BankManager_AllocStatic` 0x20a5d0 (the manager, the slot, the size, the sound memory, flags)
pub fn manager_alloc_static(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32, size: u32, spu: u32, flags: u32) {
    let s = slot(m, mgr, n);
    slot_alloc_static(m, e, s, size, spu, flags);
}

/// `BankManager_FreeSlot` 0x20a5f0
pub fn manager_free(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32) {
    let s = slot(m, mgr, n);
    slot_free(m, e, s);
}

/// `BankManager_LinkSlots` 0x20a608 (the manager, the slot, the head)
pub fn manager_link(m: &mut Mem, mgr: u32, n: u32, head: u32) {
    let (s, h) = (slot(m, mgr, n), slot(m, mgr, head));
    slot_link(m, s, h);
}

/// `BankManager_UnlinkSlot` 0x20a628
pub fn manager_unlink(m: &mut Mem, mgr: u32, n: u32) {
    let s = slot(m, mgr, n);
    slot_unlink(m, s);
}

/// `cBankManager::vf9` 0x20a640 (the manager, the slot): `BankInstance_Release`
pub fn manager_release(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32) {
    let s = slot(m, mgr, n);
    slot_release(m, e, s);
}

/// `cBankManager::vf10` 0x20a658 (the manager, the slot, the size, flags): `BankInstance_AllocDynamic`
pub fn manager_alloc(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32, size: u32, flags: u32) -> u32 {
    let s = slot(m, mgr, n);
    slot_alloc_dynamic(m, e, s, size, flags)
}

/// `cBankManager::vf11` 0x20a688 (the manager, the slot, the handle, the status, the name, ...): `BankSlot_Configure`
pub fn manager_configure(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32, handle: u32, status: u32, file: u32) {
    let s = slot(m, mgr, n);
    slot_configure(m, e, s, handle, status, file);
}

/// `BankManager_GetHandle` 0x20a6b0 (the manager, the slot): the bank's handle when loaded, else −1
pub fn manager_handle(m: &Mem, mgr: u32, n: u32) -> u32 {
    let s = slot(m, mgr, n);
    if m.r32(s) == 1 {
        m.r32(s + 4)
    } else {
        NONE
    }
}

/// `BankManager_IsSlotLoaded` 0x20a6e0 (the manager, the slot): loaded and its bank ready (`SndBank_IsPlaying` 1),
/// under the sound library's lock
pub fn manager_is_ready(m: &mut Mem, e: &mut dyn SongEnv, mgr: u32, n: u32) -> bool {
    e.call(m, SND_LOCK, &[]);
    let mut r = false;
    if m.r32(slot(m, mgr, n)) == 1 {
        let h = manager_handle(m, mgr, n);
        r = e.call(m, BANK_IS_PLAYING, &[h]) == 1;
    }
    e.call(m, SND_UNLOCK, &[]);
    r
}

// ---- cBankSys (board row E1b2g7d5f): the bank manager with the queue's kind and id arrays, and the plays ----

/// `gBankSys` 0x342c48
pub const BANK_SYS: u32 = 0x342c48;
pub const BANKSYS_VTABLE: u32 = 0x39e878;
/// The kind and id arrays' allocation names
pub const NAME_KINDS: u32 = 0x39e860;
pub const NAME_IDS: u32 = 0x39e868;
/// `SndSys_PlayPending` 0x20c948 (ported in `sndplay::play_pending`), called out
pub const PLAY_PENDING: u32 = 0x20c948;

/// `BankSys_Init` 0x20c560 (the audio system, the slot count, the `cMonitor` argument): the bank manager made, then
/// the queue's kind (+0xa40) and id (+0xa44) arrays, four entries each
pub fn bank_sys_init(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, count: u32, arg: u32) -> u32 {
    manager_init(m, e, sys, count, arg);
    m.w32(sys + 4, BANKSYS_VTABLE);
    m.w32(BANK_SYS, sys);
    for (name, at) in [(NAME_KINDS, 0xa40), (NAME_IDS, 0xa44)] {
        let p = e.call(m, EAMEM_ALLOC, &[name, 0x10, 0]);
        if p != 0 {
            e.call(m, EAMEM_SET, &[p, 0xdead_c0ed, 0x10]);
        }
        m.w32(sys + at, p);
    }
    sys
}

/// `cBankSys::~cBankSys` 0x20c610 (the audio system, the delete flags)
pub fn bank_sys_destroy(m: &mut Mem, e: &mut dyn SongEnv, sys: u32, flags: u32) {
    m.w32(sys + 4, BANKSYS_VTABLE);
    for at in [0xa40, 0xa44] {
        let p = m.r32(sys + at);
        if p != 0 {
            e.call(m, MEM_FREE, &[p]);
        }
    }
    m.w32(BANK_SYS, 0);
    manager_destroy(m, e, sys, flags);
}

/// `StaticInit_SndSys` 0x20cdb0: the two curves `Sfx_EvalCurve` 0x20d928 reads (0x3d2988..0x3d29b0)
pub fn static_init(m: &mut Mem) {
    for (at, v) in [
        (0x3d29b0, 0x3b03_126f),
        (0x3d29a8, 0x3f80_0000),
        (0x3d2988, 0x3f80_0000),
        (0x3d29ac, 0x3d9b_3d08),
        (0x3d2998, 0x3f80_0000),
        (0x3d299c, 0x3ef2_fec5),
        (0x3d29a0, 0),
        (0x3d298c, 0x3f76_e979),
        (0x3d2990, 0),
    ] {
        m.w32(at, v);
    }
}

/// A new entry on the sound queue (the audio system's `cMonitor` part at +0x20; `sndqueue::Queue`): the header
/// record (+8) copied in, its arrays cleared, the envelope 100 / 90 / 50 and +0x7c 1. The new count (its index)
fn queue_push(m: &mut Mem, q: u32) -> u32 {
    let c = m.r32(q + 0x20).wrapping_add(1);
    m.w32(q + 0x20, c);
    let rec = c.wrapping_mul(0x18).wrapping_add(m.r32(q + 0x24));
    let header: Vec<u8> = (0..0x18).map(|k| m.r8(q + 8 + k)).collect();
    for (k, b) in header.iter().enumerate() {
        m.w8(rec.wrapping_add(k as u32), *b);
    }
    let at = |m: &Mem, arr: u32, size: u32| c.wrapping_mul(size).wrapping_add(m.r32(q + arr));
    m.w32(q.wrapping_add(c.wrapping_mul(4)) + 0x28, 0);
    let a = at(m, 0x38, 4);
    m.w32(a, 0);
    let b0 = m.r8(rec) as i8 as i32 as u32;
    let a = at(m, 0x3c, 4);
    m.w32(a, b0);
    let a = at(m, 0x40, 4);
    m.w32(a, 0);
    let p = q.wrapping_add(c.wrapping_mul(8)) + 0x44;
    for k in 0..8 {
        m.w8(p.wrapping_add(k), 0);
    }
    for arr in [0x68, 0x6c] {
        let a = at(m, arr, 4);
        m.w32(a, 0);
    }
    let a74 = at(m, 0x74, 16);
    m.w32(a74.wrapping_add(4), 0);
    let a = at(m, 0x70, 4);
    m.w32(a, 0);
    m.w32(a74, 100);
    let a = at(m, 0x64, 4);
    m.w32(a, 0);
    m.w32(a74.wrapping_add(8), 0x5a);
    m.w32(a74.wrapping_add(0xc), 0x32);
    let a = at(m, 0x78, 4);
    m.w32(a, 0);
    let a = at(m, 0x7c, 4);
    m.w32(a, 1);
    c
}

fn set_kind(m: &mut Mem, q: u32, c: u32, kind: u32) {
    let a = c.wrapping_mul(4).wrapping_add(m.r32(q + 0xa40));
    m.w32(a, kind);
}

fn set_id(m: &mut Mem, q: u32, c: u32, id: u32) {
    let a = c.wrapping_mul(4).wrapping_add(m.r32(q + 0xa44));
    m.w32(a, id);
}

/// `SndSys_PushEntryCopy1` 0x20d6b0
pub fn push_entry(m: &mut Mem, q: u32) {
    queue_push(m, q);
}

/// `SndSys_PushEntryKind` 0x20d4d0 (the audio system, the kind)
pub fn push_entry_kind(m: &mut Mem, q: u32, kind: u32) {
    let c = queue_push(m, q);
    set_kind(m, q, c, kind);
}

/// `SndSys_PushEntryKindId` 0x20d2d8 (the audio system, the kind, the id)
pub fn push_entry_kind_id(m: &mut Mem, q: u32, kind: u32, id: u32) {
    let c = queue_push(m, q);
    set_kind(m, q, c, kind);
    set_id(m, q, c, id);
}

/// `cBankSys::PlaySound` 0x20d0d0 (the audio system, the kind, the id): pushed and played at once
pub fn play_sound(m: &mut Mem, e: &mut dyn SongEnv, q: u32, kind: u32, id: u32) {
    push_entry_kind_id(m, q, kind, id);
    e.call(m, PLAY_PENDING, &[q]);
}

/// `SndSys_PlaySoundVolume` 0x20ce30 (the audio system, the kind, the id, the volume): the volume kept whole at
/// +0x3c and clamped to 0..127 into the record's first byte, then played
pub fn play_sound_volume(m: &mut Mem, e: &mut dyn SongEnv, q: u32, kind: u32, id: u32, volume: u32) {
    let c = queue_push(m, q);
    set_kind(m, q, c, kind);
    set_id(m, q, c, id);
    let a = c.wrapping_mul(4).wrapping_add(m.r32(q + 0x3c));
    m.w32(a, volume);
    let v = m.r32(a) as i32;
    let byte = if v < 0 {
        0
    } else if v < 0x7f {
        m.r8(a)
    } else {
        0x7f
    };
    let rec = c.wrapping_mul(0x18).wrapping_add(m.r32(q + 0x24));
    m.w8(rec, byte);
    e.call(m, PLAY_PENDING, &[q]);
}
