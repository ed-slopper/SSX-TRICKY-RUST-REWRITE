//! The world nodes' snapshot saves (board row G12j), 1:1 on the game's bytes at their addresses ([`Mem`]), checked
//! byte for byte in the runner (`tests/snapshot_saves.rs`). Each class writes its parent's part, then a block of
//! its own words headed by the block's length and a 0xdeadbXXX tag (written into the node first), and any node
//! references (instance numbers), links and offsets. Everything goes through `Replay_Write` on the keyframe's stream
//! (`replaymgr::stream_write_mem`: nothing once the stream has overflowed).

pub use crate::pf::Mem;
use crate::replaymgr::{stream_write_bytes, stream_write_mem};

/// The course (*0x34648c: +0x48 its instances, +0x5c its splines, +0x6c its animation keys) and the two data bases
/// offsets are taken from (0x34649c; the scripts' 0x3464a0)
const COURSE: u32 = 0x34648c;
const DATA: u32 = 0x34649c;
const SCRIPTS: u32 = 0x3464a0;

fn words(m: &mut Mem, s: u32, w: &[u32]) {
    let b: Vec<u8> = w.iter().flat_map(|v| v.to_le_bytes()).collect();
    stream_write_bytes(m, s, &b);
}

/// A block of the node's own words: its length and tag written at its start, then the block
fn tagged(m: &mut Mem, s: u32, at: u32, len: u32, tag: u32) {
    m.w32(at, len);
    m.w32(at + 4, tag);
    stream_write_mem(m, s, at, len);
}

/// `Replay_WriteNodeRef` 0x192f30 on memory: the node's instance number, -1 for none
pub fn write_node_ref(m: &mut Mem, s: u32, node: u32) {
    let v = if node != 0 { (node.wrapping_sub(m.r32(m.r32(COURSE) + 0x48)) as i32) >> 8 } else { -1 };
    words(m, s, &[v as u32]);
}

/// `Replay_WriteLink` 0x192f78 on memory(owner, node): the instance number, the node's +0xe8 half word, whether its
/// +0xe4 is the owner
pub fn write_link(m: &mut Mem, s: u32, owner: u32, node: u32) {
    let (i, word, flag) = if node == 0 {
        (-1, 0u16, 0u16)
    } else {
        ((node.wrapping_sub(m.r32(m.r32(COURSE) + 0x48)) as i32) >> 8, m.r16(node + 0xe8), (m.r32(node + 0xe4) == owner) as u16)
    };
    let mut b = (i as u32).to_le_bytes().to_vec();
    b.extend(word.to_le_bytes());
    b.extend(flag.to_le_bytes());
    stream_write_bytes(m, s, &b);
}

/// `Replay_WriteOffset` 0x193020 / `_WriteOffset2` 0x192fe8 on memory: an address less the data base
pub fn write_offset(m: &mut Mem, s: u32, addr: u32) {
    let v = addr.wrapping_sub(m.r32(DATA));
    words(m, s, &[v]);
}

/// `cDeadNode::SaveSnapshot` 0x13aef8 (also `cRestoreNode`'s 0x13b0b0 and `cMovieNode`'s 0x148dc0): the sorted node's
/// header, then the link to its instance (+0x28)
pub fn dead_save(m: &mut Mem, this: u32, s: u32) {
    crate::bxlist::sort_node_save(m, this, s);
    let inst = m.r32(this + 0x28);
    write_link(m, s, this, inst);
}

/// `Snapshot_WriteHeader` 0x154480: 0x90 bytes tagged 0xdeadbf17
pub fn header_save(m: &mut Mem, p: u32, s: u32) {
    m.w32(p + 4, 0xdead_bf17);
    m.w32(p, 0x90);
    stream_write_mem(m, s, p, 0x90);
}

/// `ParticleNode_SaveState` 0x1d8d48: its first word and +0x18
pub fn particle_state_save(m: &mut Mem, p: u32, s: u32) {
    let w = [m.r32(p), m.r32(p + 0x18)];
    words(m, s, &w);
}

/// `AnimObjectNode_SaveState` 0x1cb198: 0x48 bytes tagged 0xdeadbef2, then its key (+0x48) as an offset from the
/// course's keys
pub fn anim_state_save(m: &mut Mem, p: u32, s: u32) {
    m.w32(p + 4, 0xdead_bef2);
    m.w32(p, 0x48);
    stream_write_mem(m, s, p, 0x48);
    let v = m.r32(p + 0x48).wrapping_sub(m.r32(m.r32(COURSE) + 0x6c));
    words(m, s, &[v]);
}

/// `Spline_SaveSnapshot` 0x2519b0: 0xc bytes tagged 0xdeadbf1a, then its spline (+0xc) as an offset from the
/// course's splines
pub fn spline_save(m: &mut Mem, p: u32, s: u32) {
    m.w32(p + 4, 0xdead_bf1a);
    m.w32(p, 0xc);
    stream_write_mem(m, s, p, 0xc);
    let v = m.r32(p + 0xc).wrapping_sub(m.r32(m.r32(COURSE) + 0x5c));
    words(m, s, &[v]);
}

/// The classes whose save is the dead node's and one tagged block: (class, the save's address, block offset,
/// length, tag)
pub const SIMPLE: [(&str, u32, u32, u32, u32); 14] = [
    ("cCounterNode", 0x13b520, 0x2c, 0x10, 0xdead_befe),
    ("cTimerNode", 0x140080, 0x2c, 0x10, 0xdead_bf21),
    ("cRailNode", 0x140440, 0x2c, 0x14, 0xdead_bf13),
    ("cBoostNode", 0x1408f0, 0x30, 0x40, 0xdead_befa),
    ("cLapBoostNode", 0x1411b0, 0x30, 0x40, 0xdead_bf0d),
    ("cZBoostNode", 0x141f98, 0x30, 0x40, 0xdead_bf26),
    ("cTexFlipNode", 0x1434f8, 0x2c, 0x2c, 0xdead_bf1d),
    ("cCrackedNode", 0x145888, 0x2c, 0x14, 0xdead_beff),
    ("cCrowdBoxNode", 0x146098, 0x2c, 0x14, 0xdead_bf00),
    ("cMeshAnimNode", 0x147898, 0x2c, 0xc, 0xdead_bf10),
    ("cTrickTriggerNode", 0x147ca0, 0x2c, 0x20, 0xdead_bf20),
    ("cRollerNode", 0x13fb90, 0xc0, 0x90, 0xdead_bf05),
    ("cUVScrollNode", 0x142a60, 0x2c, 0x24, 0xdead_bf22),
    ("cRandomBoostNode", 0x141630, 0x100, 0x90, 0xdead_bf16),
];

/// A simple class's save (by its address in `SIMPLE`): the parent's part (the boost's for the random boost), the
/// block, then the roller's 0x90-byte header at +0x30 or the UV scroll's offset (+0x50)
pub fn simple_save(m: &mut Mem, addr: u32, this: u32, s: u32) {
    let &(_, _, off, len, tag) = SIMPLE.iter().find(|c| c.1 == addr).expect("not a simple class");
    if addr == 0x141630 {
        simple_save(m, 0x1408f0, this, s);
    } else {
        dead_save(m, this, s);
    }
    tagged(m, s, this + off, len, tag);
    match addr {
        0x13fb90 => header_save(m, this + 0x30, s),
        0x142a60 => {
            let a = m.r32(this + 0x50);
            write_offset(m, s, a);
        }
        _ => {}
    }
}

/// `cDebounceNode::SaveSnapshot` 0x13fd90: the dead node's part and its word at +0x2c
pub fn debounce_save(m: &mut Mem, this: u32, s: u32) {
    dead_save(m, this, s);
    stream_write_mem(m, s, this + 0x2c, 4);
}

/// `cTubeEndBoostNode::SaveSnapshot` 0x141a88: the boost's part, 0xc0 bytes tagged 0xdeadbf29, and the 0x18 bytes
/// at 0x31d338 (shared by all tube ends)
pub fn tube_end_save(m: &mut Mem, this: u32, s: u32) {
    simple_save(m, 0x1408f0, this, s);
    tagged(m, s, this + 0x100, 0xc0, 0xdead_bf29);
    stream_write_mem(m, s, 0x31d338, 0x18);
}

/// `cFenceNode::SaveSnapshot` 0x144710 / `cFlagNode::SaveSnapshot` 0x1453f0: the dead node's part, an offset (+0x50 /
/// +0x48), then the block (0x1c tagged 0xdeadbf03 / 0xc tagged 0xdeadbf04)
pub fn fence_save(m: &mut Mem, this: u32, s: u32) {
    dead_save(m, this, s);
    let a = m.r32(this + 0x50);
    write_offset(m, s, a);
    tagged(m, s, this + 0x2c, 0x1c, 0xdead_bf03);
}
pub fn flag_save(m: &mut Mem, this: u32, s: u32) {
    dead_save(m, this, s);
    let a = m.r32(this + 0x48);
    write_offset(m, s, a);
    tagged(m, s, this + 0x2c, 0xc, 0xdead_bf04);
}

/// `cUVScrollTexFlipNode::SaveSnapshot` 0x146660: the UV scroll at +0x24, then its own 0x24 bytes tagged 0xdeadbf23
pub fn uv_scroll_tex_flip_save(m: &mut Mem, this: u32, s: u32) {
    simple_save(m, 0x142a60, this + 0x24, s);
    tagged(m, s, this, 0x24, 0xdead_bf23);
}

/// `cEmitterNode::SaveSnapshot` 0x147f30: the typed node's header, its instance (+0x18) and data (+0x1c), the
/// particles' state (+0x20)
pub fn emitter_save(m: &mut Mem, this: u32, s: u32) {
    crate::bxlist::type_node_save(m, this, s);
    let (n, d) = (m.r32(this + 0x18), m.r32(this + 0x1c));
    write_node_ref(m, s, n);
    write_offset(m, s, d);
    particle_state_save(m, this + 0x20, s);
}

/// `cParticleNode::SaveSnapshot` 0x1481d0: the dead node's part, its data (+0x2c), the particles' state (+0x30)
pub fn particle_save(m: &mut Mem, this: u32, s: u32) {
    dead_save(m, this, s);
    let d = m.r32(this + 0x2c);
    write_offset(m, s, d);
    particle_state_save(m, this + 0x30, s);
}

/// `cCollideEmitterNode::SaveSnapshot` 0x1489f0: the typed node's header, 0x40 bytes tagged 0xdeadbf28, its instance
/// (+0x60) and data (+0x64), the particles' state (+0x70)
pub fn collide_emitter_save(m: &mut Mem, this: u32, s: u32) {
    crate::bxlist::type_node_save(m, this, s);
    tagged(m, s, this + 0x20, 0x40, 0xdead_bf28);
    let (n, d) = (m.r32(this + 0x60), m.r32(this + 0x64));
    write_node_ref(m, s, n);
    write_offset(m, s, d);
    particle_state_save(m, this + 0x70, s);
}

/// `cRailManNode::SaveSnapshot` 0x149158: the sorted node's header and its 0x200 bytes of rail states
pub fn rail_man_save(m: &mut Mem, this: u32, s: u32) {
    crate::bxlist::sort_node_save(m, this, s);
    stream_write_mem(m, s, this + 0x28, 0x200);
}

/// `cScriptNode::SaveSnapshot` 0x13d3d8: the sorted node's header, 0xb0 bytes tagged 0xdeadbf01 with its program
/// counter (+0xc0) made an offset from the scripts for the write (and put back), then its rider (its +0x460, or -1)
/// and its data (+0xe0) as an offset, then its instance (+0xe4)
pub fn script_save(m: &mut Mem, this: u32, s: u32) {
    crate::bxlist::sort_node_save(m, this, s);
    let scripts = m.r32(SCRIPTS);
    m.w32(this + 0x30, 0xb0);
    m.w32(this + 0x34, 0xdead_bf01);
    m.w32(this + 0xc0, m.r32(this + 0xc0).wrapping_sub(scripts));
    stream_write_mem(m, s, this + 0x30, 0xb0);
    let data = m.r32(this + 0xe0).wrapping_sub(m.r32(DATA));
    m.w32(this + 0xc0, m.r32(SCRIPTS).wrapping_add(m.r32(this + 0xc0)));
    let rider = m.r32(this + 0xe8);
    let r = if rider == 0 { u32::MAX } else { m.r32(rider + 0x460) };
    words(m, s, &[r, data]);
    let inst = m.r32(this + 0xe4);
    write_node_ref(m, s, inst);
}

/// `cAnimObjectNode::SaveSnapshot` 0x1998f8: the dead node at +0x30, 0x30 bytes tagged 0xdeadbef3, whether it has an
/// overlap (+0x64: then its 0x10 bytes and 0xc at +200), then each of its +0x5c key states (+0x60, 0xd0 apart)
pub fn anim_object_save(m: &mut Mem, this: u32, s: u32) {
    dead_save(m, this + 0x30, s);
    tagged(m, s, this, 0x30, 0xdead_bef3);
    let ov = m.r32(this + 0x64);
    words(m, s, &[(ov != 0) as u32]);
    if ov != 0 {
        stream_write_mem(m, s, ov, 0x10);
        stream_write_mem(m, s, ov + 200, 0xc);
    }
    let n = m.r32(this + 0x5c) as i32;
    for i in 0..n.max(0) as u32 {
        let p = m.r32(this + 0x60) + 0xd0 * i;
        anim_state_save(m, p, s);
    }
}

/// `cAnimComboNode::SaveSnapshot` 0x19a410: the anim object at +0x1c, 0x1c bytes tagged 0xdeadbef4, then its +0x78 × 64
/// bytes at +0x84
pub fn anim_combo_save(m: &mut Mem, this: u32, s: u32) {
    anim_object_save(m, this + 0x1c, s);
    tagged(m, s, this, 0x1c, 0xdead_bef4);
    let (p, n) = (m.r32(this + 0x84), m.r32(this + 0x78));
    stream_write_mem(m, s, p, n << 6);
}

/// `cAnimDeltaNode`'s save 0x199ee0: the anim object and its word at +0x68
pub fn anim_delta_save(m: &mut Mem, this: u32, s: u32) {
    anim_object_save(m, this, s);
    stream_write_mem(m, s, this + 0x68, 4);
}

/// `cAnimTexFlipNode`'s save 0x19a940: the anim object at +0x18, then 0x18 bytes tagged 0xdeadbef5
pub fn anim_tex_flip_save(m: &mut Mem, this: u32, s: u32) {
    anim_object_save(m, this + 0x18, s);
    tagged(m, s, this, 0x18, 0xdead_bef5);
}

/// `cSplinePathNode::SaveSnapshot` 0x1fb1a8: the sorted node's header, 0x3c bytes tagged 0xdeadbf1c, its instance
/// (+0x78), its +0x30 × 32 bytes at +0x7c, then its spline (+0x64)
pub fn spline_path_save(m: &mut Mem, this: u32, s: u32) {
    crate::bxlist::sort_node_save(m, this, s);
    tagged(m, s, this + 0x28, 0x3c, 0xdead_bf1c);
    let inst = m.r32(this + 0x78);
    write_node_ref(m, s, inst);
    let (p, n) = (m.r32(this + 0x7c), m.r32(this + 0x30));
    stream_write_mem(m, s, p, n << 5);
    spline_save(m, this + 0x64, s);
}

/// The save for a world node class by the save's address (as the vtable's +0x44 slot holds it)
pub fn save_by_address(m: &mut Mem, addr: u32, this: u32, s: u32) -> bool {
    match addr {
        0x13aef8 | 0x13b0b0 | 0x148dc0 => dead_save(m, this, s),
        0x13fd90 => debounce_save(m, this, s),
        0x141a88 => tube_end_save(m, this, s),
        0x144710 => fence_save(m, this, s),
        0x1453f0 => flag_save(m, this, s),
        0x146660 => uv_scroll_tex_flip_save(m, this, s),
        0x147f30 => emitter_save(m, this, s),
        0x1481d0 => particle_save(m, this, s),
        0x1489f0 => collide_emitter_save(m, this, s),
        0x149158 => rail_man_save(m, this, s),
        0x13d3d8 => script_save(m, this, s),
        0x1998f8 => anim_object_save(m, this, s),
        0x19a410 => anim_combo_save(m, this, s),
        0x199ee0 => anim_delta_save(m, this, s),
        0x19a940 => anim_tex_flip_save(m, this, s),
        0x1fb1a8 => spline_path_save(m, this, s),
        a if SIMPLE.iter().any(|c| c.1 == a) => simple_save(m, a, this, s),
        _ => return false,
    }
    true
}

// The world nodes' load constructors (board row G12k): each builds its parent from the stream, sets its vtable,
// reads its words back and calls the world's set-up (boxes into the broadphase, texture flips, particles, the rail
// manager's rails...) through the env. The roller and the anim objects are not here yet.

/// The world calls the load constructors make: the function at `func` with `args` (its answer, when it has one:
/// `Race_GetRider` a rider, `Spline_GetLength` a float's bits)
pub trait LoadEnv {
    fn call(&mut self, m: &mut Mem, func: u32, args: &[u32]) -> u32;
    /// `MemPool_Alloc` 0x23d3b0(the node pool, size, 0x40000000, name, 0, 0)
    fn pool_alloc(&mut self, m: &mut Mem, size: u32, name: u32) -> u32;
}

/// The classes' vtables
pub mod vt {
    pub const CONTROLLER: u32 = 0x36b388;
    pub const DEAD: u32 = 0x36b2a8;
    pub const RESTORE: u32 = 0x36b1c8;
    pub const COUNTER: u32 = 0x36b0e8;
    pub const DEBOUNCE: u32 = 0x36caf8;
    pub const TIMER: u32 = 0x36d6b0;
    pub const RAIL: u32 = 0x36d5d0;
    pub const BOOST: u32 = 0x36d4d8;
    pub const LAP_BOOST: u32 = 0x36d3f8;
    pub const RANDOM_BOOST: u32 = 0x36d300;
    pub const TUBE_END: u32 = 0x36d208;
    pub const Z_BOOST: u32 = 0x36d128;
    pub const UV_SCROLL: u32 = 0x36d048;
    pub const TEX_FLIP: u32 = 0x36cf58;
    pub const FENCE: u32 = 0x36cd98;
    pub const FLAG: u32 = 0x36ccb8;
    pub const CRACKED: u32 = 0x36cbd8;
    pub const CROWD_BOX: u32 = 0x36ca18;
    pub const UV_SCROLL_TEX_FLIP: u32 = 0x36ce78;
    pub const MESH_ANIM: u32 = 0x36c938;
    pub const TRICK_TRIGGER: u32 = 0x36c858;
    pub const EMITTER: u32 = 0x36c7f8;
    pub const PARTICLE: u32 = 0x36c718;
    pub const COLLIDE_EMITTER: u32 = 0x36c650;
    pub const MOVIE: u32 = 0x36c570;
    pub const RAIL_MAN: u32 = 0x36c510;
    pub const SCRIPT: u32 = 0x36bc20;
    pub const SPLINE_PATH: u32 = 0x39a710;
}

fn read_into(m: &mut Mem, s: u32, dst: u32, n: u32) {
    crate::replaymgr::stream_read_mem(m, s, dst, n);
}

fn read_words<const N: usize>(m: &mut Mem, s: u32) -> [u32; N] {
    let at = m.r32(s + 8);
    let mut w = [0u32; N];
    for (i, v) in w.iter_mut().enumerate() {
        *v = m.r32(at + 4 * i as u32);
    }
    m.w32(s + 8, at.wrapping_add(4 * N as u32));
    w
}

fn game(m: &Mem) -> u32 {
    m.r32(m.r32(0x337c58) + 0x730)
}

/// `Course_GetInstance` 0x254a28 on the course: an instance, 0 at or past the count
pub fn course_instance(m: &Mem, i: i32) -> u32 {
    let w = m.r32(COURSE);
    if i < m.r32(w + 0xc) as i32 { m.r32(w + 0x48).wrapping_add((i as u32) << 8) } else { 0 }
}

/// `Replay_ReadNodeRef` 0x192df0 on memory: an instance number back (0 for -1)
pub fn read_node_ref(m: &mut Mem, s: u32) -> u32 {
    let [v] = read_words::<1>(m, s);
    if (v as i32) < 0 { 0 } else { course_instance(m, v as i32) }
}

/// `Replay_ReadLink` 0x192e40 on memory(owner): the instance back; when the link was the owner's, the instance's +0xe4
/// is the owner again and the low half of +0xe8 the saved half word
pub fn read_link(m: &mut Mem, s: u32, owner: u32) -> u32 {
    let [i, hw] = read_words::<2>(m, s);
    if (i as i32) < 0 {
        return 0;
    }
    let node = course_instance(m, i as i32);
    if hw >> 16 != 0 {
        m.w32(node + 0xe4, owner);
        m.w32(node + 0xe8, m.r32(node + 0xe8) & 0xffff_0000 | (hw & 0xffff));
    }
    node
}

/// `Replay_ReadOffset` 0x192ec0 / `_ReadOffset2` 0x192ef8 on memory
pub fn read_offset(m: &mut Mem, s: u32) -> u32 {
    let [v] = read_words::<1>(m, s);
    m.r32(DATA).wrapping_add(v)
}

/// `ParticleSys_ReadReplay` 0x1d8d80: two words back into +0 and +0x18
pub fn particle_state_load(m: &mut Mem, p: u32, s: u32) {
    let [a, b] = read_words::<2>(m, s);
    m.w32(p, a);
    m.w32(p + 0x18, b);
}

/// `SplinePathNode_Load` 0x251a28: 0xc bytes, the spline from the course's splines, its length (`Spline_GetLength`
/// 0x251968) at +0x10
pub fn spline_load(m: &mut Mem, env: &mut dyn LoadEnv, p: u32, s: u32) {
    read_into(m, s, p, 0xc);
    let [v] = read_words::<1>(m, s);
    m.w32(p + 0xc, m.r32(m.r32(COURSE) + 0x5c).wrapping_add(v));
    let len = env.call(m, 0x251968, &[p]);
    m.w32(p + 0x10, len);
}

/// `cControllerNode::cControllerNode` 0x13a840(list, type, instance): a sorted node on the instance (each pointing at
/// the other)
pub fn controller_new(m: &mut Mem, node: u32, l: u32, ty: u32, inst: u32) {
    crate::bxlist::sort_node_new(m, node, l, ty);
    m.w32(inst + 0xe4, node);
    m.w32(node + 0x28, inst);
    m.w32(node + 8, vt::CONTROLLER);
}

/// `cControllerNode::cControllerNode` 0x13a888(list, stream): the sorted node back, its instance by its link
pub fn controller_load(m: &mut Mem, node: u32, l: u32, s: u32) {
    crate::bxlist::sort_node_load(m, node, l, s);
    m.w32(node + 8, vt::CONTROLLER);
    let inst = read_link(m, s, node);
    m.w32(node + 0x28, inst);
}

/// `cDeadNode::cDeadNode` 0x13af38(instance, flag): a controller on list 13 (type 5); the instance's +0xe8 bits 1,
/// 4..7 cleared and bit 2 set (and 0x800 with the flag)
pub fn dead_new(m: &mut Mem, node: u32, inst: u32, flag: u32) {
    controller_new(m, node, 0xd, 5, inst);
    let i = m.r32(node + 0x28);
    let v = m.r32(i + 0xe8) & 0xffff_ff0d;
    m.w32(node + 8, vt::DEAD);
    m.w32(i + 0xe8, v | 4);
    if flag != 0 {
        m.w32(i + 0xe8, v | 0x804);
    }
}

fn plain(m: &mut Mem, node: u32, l: u32, s: u32, vtable: u32, at: u32, n: u32) {
    controller_load(m, node, l, s);
    m.w32(node + 8, vtable);
    if n != 0 {
        read_into(m, s, node + at, n);
    }
}

fn broadphase(m: &Mem) -> u32 {
    m.r32(game(m) + 0x30)
}

/// The boosts' load (`cBoostNode` 0x140590, `cLapBoostNode` 0x140a98, `cZBoostNode` 0x141be0): the controller, the
/// vtable, the box (+0x70) made, 0x40 bytes at +0x30, the box's owner, its extent from the instance, fitted into the
/// broadphase
pub fn boost_load(m: &mut Mem, env: &mut dyn LoadEnv, node: u32, l: u32, s: u32, vtable: u32) {
    controller_load(m, node, l, s);
    m.w32(node + 8, vtable);
    env.call(m, 0x150670, &[node + 0x70]);
    read_into(m, s, node + 0x30, 0x40);
    m.w32(node + 0xf4, node);
    let inst = m.r32(node + 0x28);
    env.call(m, 0x150818, &[node + 0x70, inst.wrapping_add(0xcc), inst.wrapping_add(0xd8)]);
    let bp = broadphase(m);
    env.call(m, 0x14fa70, &[bp, node + 0x70]);
}

/// `cUVScrollNode` 0x1420b8 (also the base of `cUVScrollTexFlipNode`)
pub fn uv_scroll_load(m: &mut Mem, env: &mut dyn LoadEnv, node: u32, l: u32, s: u32) {
    plain(m, node, l, s, vt::UV_SCROLL, 0x2c, 0x24);
    let o = read_offset(m, s);
    m.w32(node + 0x50, o);
    env.call(m, 0x1421a8, &[node]);
    env.call(m, 0x142220, &[node]);
}

/// A world node's load constructor by its address (the `NODE_TYPES` table's): false for the ones not ported yet
pub fn load_by_address(m: &mut Mem, env: &mut dyn LoadEnv, addr: u32, node: u32, l: u32, s: u32) -> bool {
    match addr {
        0x13b078 => plain(m, node, l, s, vt::RESTORE, 0, 0),
        0x13b128 => plain(m, node, l, s, vt::COUNTER, 0x2c, 0x10),
        0x13fc58 => plain(m, node, l, s, vt::DEBOUNCE, 0x2c, 4),
        0x13fe50 => plain(m, node, l, s, vt::TIMER, 0x2c, 0x10),
        0x1401a0 => {
            plain(m, node, l, s, vt::RAIL, 0x2c, 0x14);
            // +0x3c set: `RailNode_Enable` 0x1403c0; clear: `RailNode_Disable` 0x140400
            let f = if m.r32(node + 0x3c) != 0 { 0x1403c0 } else { 0x140400 };
            env.call(m, f, &[node]);
        }
        0x140590 => boost_load(m, env, node, l, s, vt::BOOST),
        0x140a98 => boost_load(m, env, node, l, s, vt::LAP_BOOST),
        0x141be0 => boost_load(m, env, node, l, s, vt::Z_BOOST),
        0x1412e8 => {
            boost_load(m, env, node, l, s, vt::BOOST);
            m.w32(node + 8, vt::RANDOM_BOOST);
            read_into(m, s, node + 0x100, 0x90);
        }
        0x141800 => {
            boost_load(m, env, node, l, s, vt::BOOST);
            m.w32(node + 8, vt::TUBE_END);
            read_into(m, s, node + 0x100, 0xc0);
            read_into(m, s, 0x31d338, 0x18);
        }
        0x1420b8 => uv_scroll_load(m, env, node, l, s),
        0x143040 => {
            plain(m, node, l, s, vt::TEX_FLIP, 0, 0);
            env.call(m, 0x142ac8, &[node + 0x58]);
            read_into(m, s, node + 0x2c, 0x2c);
            let (a, b) = (m.r32(node + 0x28), m.r32(node + 0x48));
            env.call(m, 0x142af0, &[node + 0x58, a, b]);
        }
        0x143870 | 0x144810 => {
            let (vtable, at, init, n) = if addr == 0x143870 { (vt::FENCE, 0x50, 0x143ff8, 0x1c) } else { (vt::FLAG, 0x48, 0x144938, 0xc) };
            plain(m, node, l, s, vtable, 0, 0);
            let o = read_offset(m, s);
            m.w32(node + at, o);
            env.call(m, init, &[node]);
            read_into(m, s, node + 0x2c, n);
        }
        0x145528 => {
            plain(m, node, l, s, vt::CRACKED, 0, 0);
            env.call(m, 0x142ac8, &[node + 0x40]);
            read_into(m, s, node + 0x2c, 0x14);
            let a = m.r32(node + 0x28);
            env.call(m, 0x142af0, &[node + 0x40, a, u32::MAX]);
        }
        0x145ba8 => {
            plain(m, node, l, s, vt::CROWD_BOX, 0x2c, 0x14);
            env.call(m, 0x145c00, &[node]);
        }
        0x146258 => {
            uv_scroll_load(m, env, node + 0x24, l, s);
            m.w32(node + 0x2c, vt::UV_SCROLL_TEX_FLIP);
            env.call(m, 0x142ac8, &[node + 0x90]);
            read_into(m, s, node, 0x24);
            let (a, b) = (m.r32(node + 0x4c), m.r32(node + 0x10));
            env.call(m, 0x142af0, &[node + 0x90, a, b]);
        }
        0x146e10 => {
            plain(m, node, l, s, vt::MESH_ANIM, 0x2c, 0xc);
            m.w32(node + 0x6cc, 0);
            m.w32(node + 0x6c0, 0);
        }
        0x1479d0 => {
            plain(m, node, l, s, vt::TRICK_TRIGGER, 0, 0);
            env.call(m, 0x150670, &[node + 0x50]);
            read_into(m, s, node + 0x2c, 0x20);
            let g = game(m);
            let rider = env.call(m, 0x1814a8, &[g, m.r32(node + 0x40)]);
            m.w32(node + 0xd4, node);
            m.w32(node + 0x4c, rider);
            let inst = m.r32(node + 0x28);
            env.call(m, 0x150818, &[node + 0x50, inst.wrapping_add(0xcc), inst.wrapping_add(0xd8)]);
            let bp = broadphase(m);
            env.call(m, 0x14fa70, &[bp, node + 0x50]);
        }
        0x147d78 => {
            crate::bxlist::type_node_load(m, node, l, s);
            m.w32(node + 8, vt::EMITTER);
            env.call(m, 0x1d8488, &[node + 0x20]);
            let r = read_node_ref(m, s);
            m.w32(node + 0x18, r);
            let o = read_offset(m, s);
            m.w32(node + 0x1c, o);
            particle_state_load(m, node + 0x20, s);
            let d = m.r32(node + 0x1c);
            let data = if d == 0 { 0 } else { d + 0xc };
            let inst = m.r32(node + 0x18);
            env.call(m, 0x1d8490, &[node + 0x20, inst, data]);
        }
        0x148028 => {
            plain(m, node, l, s, vt::PARTICLE, 0, 0);
            env.call(m, 0x1d8488, &[node + 0x30]);
            let o = read_offset(m, s);
            m.w32(node + 0x2c, o);
            particle_state_load(m, node + 0x30, s);
            let d = m.r32(node + 0x2c);
            let data = if d == 0 { 0 } else { d + 0xc };
            let inst = m.r32(node + 0x28);
            env.call(m, 0x1d8490, &[node + 0x30, inst, data]);
        }
        0x148680 => {
            crate::bxlist::type_node_load(m, node, l, s);
            m.w32(node + 8, vt::COLLIDE_EMITTER);
            env.call(m, 0x1d8488, &[node + 0x70]);
            read_into(m, s, node + 0x20, 0x40);
            let r = read_node_ref(m, s);
            m.w32(node + 0x60, r);
            let o = read_offset(m, s);
            m.w32(node + 0x64, o);
            particle_state_load(m, node + 0x70, s);
            env.call(m, 0x148748, &[node]);
            let at = 0x31d498u32.wrapping_add(m.r32(node + 0x50).wrapping_mul(4));
            m.w32(at, m.r32(at).wrapping_add(1));
        }
        0x148ad0 => {
            plain(m, node, l, s, vt::MOVIE, 0, 0);
            env.call(m, 0x148b28, &[node]);
            let i = m.r32(node + 0x28);
            m.w32(i + 0xe8, m.r32(i + 0xe8) & 0xffff_fffd | 4);
        }
        0x148e38 => {
            crate::bxlist::sort_node_load(m, node, l, s);
            m.w32(0x31d4b0, node);
            m.w32(node + 8, vt::RAIL_MAN);
            read_into(m, s, node + 0x28, 0x200);
            for i in 0..0x80u32 {
                if m.r16(node + 0x28 + 4 * i) as i16 != -1 {
                    env.call(m, 0x148ed8, &[node, i]);
                }
            }
        }
        0x13d2f8 => {
            crate::bxlist::sort_node_load(m, node, l, s);
            m.w32(node + 8, vt::SCRIPT);
            read_into(m, s, node + 0x30, 0xb0);
            m.w32(node + 0xc0, m.r32(SCRIPTS).wrapping_add(m.r32(node + 0xc0)));
            let [rider, data] = read_words::<2>(m, s);
            m.w32(node + 0xe0, m.r32(DATA).wrapping_add(data));
            let r = if rider == u32::MAX { 0 } else { m.r32(m.r32(game(m) + 0x1c).wrapping_add(rider.wrapping_mul(4)).wrapping_add(0xc4)) };
            m.w32(node + 0xe8, r);
            let inst = read_node_ref(m, s);
            m.w32(node + 0xe4, inst);
        }
        0x1fa480 => spline_path_load(m, env, node, l, s),
        _ => return false,
    }
    true
}

/// `cSplinePathNode` 0x1fa480: the sorted node, 0x3c bytes at +0x28, its instance, the instance's box corners less its
/// position (w 1.0 less its w) at +0x80 / +0x90, a pool array of +0x30 × 32 bytes read back (+0x7c), its spline
pub fn spline_path_load(m: &mut Mem, env: &mut dyn LoadEnv, node: u32, l: u32, s: u32) {
    use crate::ps2f::F;
    crate::bxlist::sort_node_load(m, node, l, s);
    m.w32(node + 0x70, 0);
    m.w32(node + 8, vt::SPLINE_PATH);
    read_into(m, s, node + 0x28, 0x3c);
    let inst = read_node_ref(m, s);
    m.w32(node + 0x78, inst);
    let n = m.r32(node + 0x30);
    for (corner, out) in [(0xccu32, 0x80u32), (0xd8, 0x90)] {
        for k in 0..4u32 {
            let a = if k < 3 { m.r32(inst.wrapping_add(corner + 4 * k)) } else { 0x3f80_0000 };
            let b = m.r32(inst.wrapping_add(0x30 + 4 * k));
            let v = (F(f32::from_bits(a)) - F(f32::from_bits(b))).0.to_bits();
            m.w32(node + out + 4 * k, v);
        }
    }
    let size = n << 5;
    let p = env.pool_alloc(m, size, 0x39a6f8);
    if p != 0 {
        crate::replaymgr::mem_set_pub(m, p, 0xdead_c0ed, size);
    }
    m.w32(node + 0x7c, p);
    let n = m.r32(node + 0x30);
    read_into(m, s, p, n << 5);
    spline_load(m, env, node + 0x64, s);
}

// The riders' and the AI's snapshots (board row G12l). A rider (`cPlayer` / `cComputer`, a pointer to its boarder data
// first) saves its boarder data's 0x450-byte block, its animation, air prediction and score, then each of its motions
// and controls (each a small block tagged by class), then its AI path and line as indices. The AI saves its handlers'
// state and the node counters, the AI world its 0x6c cell slots. World calls go through `LoadEnv::call`.

/// The motions' and controls' blocks: (their offset in the rider, length, tag), in the order saved
pub const RIDER_PARTS: [(u32, u32, u32); 15] = [
    (0x480, 0x20, 0xdead_bef0), // AirMotion
    (0x4b0, 0xc, 0xdead_bf09),  // GroundMotion
    (0x4c0, 0x10, 0xdead_bf15), // RailSlideMotion
    (0x4f0, 0x60, 0xdead_bf24), // WipeoutMotion (and its 0x90-byte header at +0x70)
    (0x5f0, 0xc, 0xdead_bf25),  // WipeOutRecoverMotion
    (0x610, 0x10, 0xdead_bf06), // BumpControl
    (0x630, 0x14, 0xdead_bf07), // CruiseControl
    (0x670, 0x10, 0xdead_bf0a), // FinishLineControl
    (0x6a0, 0x14, 0xdead_bf08), // GateAnticipateControl
    (0x6f0, 0xc, 0xdead_bf0c),  // LandNormalControl
    (0x700, 0xc, 0xdead_bf0f),  // NaturalAirControl
    (0x710, 0xc, 0xdead_bf12),  // PrewindControl
    (0x720, 0x14, 0xdead_bf14), // RailSlideControl
    (0x740, 0x70, 0xdead_bf19), // SpinControl
    (0x810, 0x18, 0xdead_bf0b), // ResetWaitControl
];

/// A part's save (`*_SaveSnapshot`: the tag, then the length, then the block) and load (`*_LoadSnapshot`)
pub fn part_save(m: &mut Mem, p: u32, len: u32, tag: u32, s: u32) {
    m.w32(p + 4, tag);
    m.w32(p, len);
    stream_write_mem(m, s, p, len);
}
pub fn part_load(m: &mut Mem, p: u32, len: u32, s: u32) {
    read_into(m, s, p, len);
}

/// `AnimLayer_SaveSnapshot` 0x15a798(layer): the clip count (`ClipList_Count` 0x203b98), then each sequence of the
/// layer's ring (from +0x64 through +0x2c) saving itself (its vtable at +0x34, slot +0x24)
pub fn anim_layer_save(m: &mut Mem, env: &mut dyn LoadEnv, layer: u32, s: u32) {
    let n = env.call(m, 0x203b98, &[layer]);
    words(m, s, &[n]);
    let mut q = m.r32(layer + 0x64);
    while q != layer {
        let vtab = m.r32(q + 0x34);
        let this = q.wrapping_add(m.r16(vtab + 0x20) as i16 as i32 as u32);
        let f = m.r32(vtab + 0x24);
        env.call(m, f, &[this, s]);
        q = m.r32(q + 0x2c);
    }
}

/// `AnimLayer_LoadSnapshot` 0x15a818(layer): as many sequences as saved, each made (`BdrSeq_Create` 0x15a700), loading
/// itself (slot +0x2c) and put last in the layer's ring
pub fn anim_layer_load(m: &mut Mem, env: &mut dyn LoadEnv, layer: u32, s: u32) {
    let [n] = read_words::<1>(m, s);
    for _ in 0..(n as i32).max(0) {
        let q = env.call(m, BDR_SEQ_CREATE, &[layer]);
        let vtab = m.r32(q + 0x34);
        let this = q.wrapping_add(m.r16(vtab + 0x28) as i16 as i32 as u32);
        let f = m.r32(vtab + 0x2c);
        env.call(m, f, &[this, s]);
        m.w32(q + 0x2c, layer);
        m.w32(q + 0x30, m.r32(layer + 0x30));
        let tail = m.r32(layer + 0x30);
        m.w32(tail + 0x2c, q);
        m.w32(layer + 0x30, q);
    }
}

/// `BdrSeq_Create`
pub const BDR_SEQ_CREATE: u32 = 0x15a700;

/// `AnimCtl_SaveSnapshot` 0x160058 / `AnimCtl_LoadSnapshot` 0x1600d0: 0x24 bytes tagged 0xdeadbef6, then its three
/// layers (+0x28, 0x70 apart; on load each emptied first, `ClipList_DestroyAll` 0x203c20)
pub fn anim_ctl_save(m: &mut Mem, env: &mut dyn LoadEnv, p: u32, s: u32) {
    part_save(m, p, 0x24, 0xdead_bef6, s);
    for i in 0..3 {
        anim_layer_save(m, env, p + 0x28 + 0x70 * i, s);
    }
}
pub fn anim_ctl_load(m: &mut Mem, env: &mut dyn LoadEnv, p: u32, s: u32) {
    read_into(m, s, p, 0x24);
    for i in 0..3 {
        let layer = p + 0x28 + 0x70 * i;
        env.call(m, 0x203c20, &[layer]);
        anim_layer_load(m, env, layer, s);
    }
}

/// `Boarder_SaveSnapshot` 0x117d20(rider): the boarder data's (the rider's first word) 0x450 bytes at +0x10 tagged
/// 0xdeadbef8, a 1 and a word the game never sets (STANDIN: 0 here), its animation (+0x46c0), air prediction
/// (+0x57b0, 0x60 tagged 0xdeadbef9) and score (+0x5820, 0x24c tagged 0xdeadbf1f), the rider's motions and controls,
/// then its AI path and line (+0x57a0 / +0x57a4) as indices (`AIP_GetPathIndex` / `AIP_GetLineIndex` on 0x337fb8)
pub fn boarder_save(m: &mut Mem, env: &mut dyn LoadEnv, rider: u32, s: u32) {
    let d = m.r32(rider);
    part_save(m, d + 0x10, 0x450, 0xdead_bef8, s);
    words(m, s, &[1, 0]);
    let d = m.r32(rider);
    anim_ctl_save(m, env, d + 0x46c0, s);
    let d = m.r32(rider);
    part_save(m, d + 0x57b0, 0x60, 0xdead_bef9, s);
    let d = m.r32(rider);
    part_save(m, d + 0x5820, 0x24c, 0xdead_bf1f, s);
    for (off, len, tag) in RIDER_PARTS {
        part_save(m, rider + off, len, tag, s);
        if off == 0x4f0 {
            crate::snapshot::header_save(m, rider + off + 0x70, s);
        }
    }
    let d = m.r32(rider);
    let path = env.call(m, 0x198688, &[0x337fb8, m.r32(d + 0x57a0)]);
    let line = env.call(m, 0x1986d8, &[0x337fb8, m.r32(d + 0x57a4)]);
    let b = [(path as u16).to_le_bytes(), (line as u16).to_le_bytes()].concat();
    crate::replaymgr::stream_write_bytes(m, s, &b);
}

/// `Boarder_LoadSnapshot` 0x117ec0(rider): the same back; the boarder's +0x430 found again from its three shorts at
/// +0x440 (`FUN_00260cb8` on 0x346488), the score's trick name made again (`Score_FormatTrickName`), the AI path and
/// line from their indices (0x337fc4 + path × 0x3c, 0x337fbc + line × 0x40)
pub fn boarder_load(m: &mut Mem, env: &mut dyn LoadEnv, rider: u32, s: u32) {
    let d = m.r32(rider);
    read_into(m, s, d + 0x10, 0x450);
    let _ = read_words::<2>(m, s);
    let d = m.r32(rider);
    anim_ctl_load(m, env, d + 0x46c0, s);
    let d = m.r32(rider);
    read_into(m, s, d + 0x57b0, 0x60);
    let d = m.r32(rider);
    read_into(m, s, d + 0x5820, 0x24c);
    env.call(m, SCORE_FORMAT_TRICK_NAME, &[d + 0x5820 + 0x250, d + 0x5820 + 0x6c]);
    let d = m.r32(rider);
    let sh = |m: &Mem, a: u32| m.r16(a) as u32;
    let v = env.call(m, 0x260cb8, &[0x346488, sh(m, d + 0x440), sh(m, d + 0x442), sh(m, d + 0x444)]);
    m.w32(d + 0x430, v);
    for (off, len, _) in RIDER_PARTS {
        read_into(m, s, rider + off, len);
        if off == 0x4f0 {
            read_into(m, s, rider + off + 0x70, 0x90);
        }
    }
    let at = m.r32(s + 8);
    let (path, line) = (m.r16(at) as i16 as i32, m.r16(at + 2) as i16 as i32);
    m.w32(s + 8, at.wrapping_add(4));
    let (lines, paths) = (m.r32(0x337fbc), m.r32(0x337fc4));
    let d = m.r32(rider);
    m.w32(d + 0x57a0, paths.wrapping_add((path * 0x3c) as u32));
    m.w32(d + 0x57a4, lines.wrapping_add((line * 0x40) as u32));
}

/// `Score_FormatTrickName`
pub const SCORE_FORMAT_TRICK_NAME: u32 = 0x1551c0;

/// `cPlayer::SaveSnapshot` 0x1533f8 / its load 0x153450: the boarder, then 0xc bytes at +0x860 tagged 0xdeadbf11
pub fn player_save(m: &mut Mem, env: &mut dyn LoadEnv, rider: u32, s: u32) {
    boarder_save(m, env, rider, s);
    part_save(m, rider + 0x860, 0xc, 0xdead_bf11, s);
}
pub fn player_load(m: &mut Mem, env: &mut dyn LoadEnv, rider: u32, s: u32) {
    boarder_load(m, env, rider, s);
    read_into(m, s, rider + 0x860, 0xc);
}

/// `cComputer::SaveSnapshot` 0x13a460 / its load 0x13a4b8: the boarder, then 0x40 bytes at +0x860 tagged 0xdeadbefd;
/// on load its 8 bytes at +0x8a0 taken again from the table at 0x31d318 by its +0x89c
pub fn computer_save(m: &mut Mem, env: &mut dyn LoadEnv, rider: u32, s: u32) {
    boarder_save(m, env, rider, s);
    part_save(m, rider + 0x860, 0x40, 0xdead_befd, s);
}
pub fn computer_load(m: &mut Mem, env: &mut dyn LoadEnv, rider: u32, s: u32) {
    boarder_load(m, env, rider, s);
    read_into(m, s, rider + 0x860, 0x40);
    let i = m.r16(rider + 0x89c) as i16 as i32;
    let from = 0x31d318u32.wrapping_add((i * 8) as u32);
    for k in 0..8 {
        let b = m.r8(from + k);
        m.w8(rider + 0x8a0 + k, b);
    }
    m.w16(rider + 0x89c, i as u16);
}

/// `cSkelSequence::SaveSnapshot` 0x203aa0 / its load 0x203ac8: 0x2c bytes tagged 0xdeadbf18
/// `cBoarderSequence::SaveSnapshot` 0x15a3d8: the skeleton sequence, 0x40 bytes at +0x38 tagged 0xdeadbef7, then each
/// of its +0x44 trees' (+0x78) two first words
pub fn boarder_sequence_save(m: &mut Mem, this: u32, s: u32) {
    part_save(m, this, 0x2c, 0xdead_bf18, s);
    part_save(m, this + 0x38, 0x40, 0xdead_bef7, s);
    let mut i = 0;
    while (i as i32) < m.r32(this + 0x44) as i32 {
        let t = m.r32(this + 0x78 + 4 * i);
        let (a, b) = (m.r32(t), m.r32(t + 4));
        words(m, s, &[a]);
        words(m, s, &[b]);
        i += 1;
    }
}

/// `cBoarderSequence`'s load 0x15a4b0: its trees destroyed (`SkelAnim_DestroyTree` 0x159a80), the skeleton sequence's
/// 0x2c bytes and its 0x40 back, then each tree made again (`SkelAnim_CreateTree` 0x158dc0 on 0x3211a8) from the two
/// words' low halves (first << 16 | second)
pub fn boarder_sequence_load(m: &mut Mem, env: &mut dyn LoadEnv, this: u32, s: u32) {
    let mut i = 0;
    while (i as i32) < m.r32(this + 0x44) as i32 {
        let t = m.r32(this + 0x78 + 4 * i);
        env.call(m, 0x159a80, &[t]);
        i += 1;
    }
    read_into(m, s, this, 0x2c);
    read_into(m, s, this + 0x38, 0x40);
    let mut i = 0;
    while (i as i32) < m.r32(this + 0x44) as i32 {
        let [a, b] = read_words::<2>(m, s);
        let key = (a & 0xffff) << 16 | (b & 0xffff);
        let t = env.call(m, 0x158dc0, &[0x3211a8, key]);
        m.w32(this + 0x78 + 4 * i, t);
        i += 1;
    }
}

/// `cAI::SaveSnapshot` 0x112790 / its load 0x1127e0: 0x18 bytes at +0xc tagged 0xdeadbef1 and the node counters; on
/// load the handlers made again (`Race_CreateHandlers` 0x1125c8) for +0x20 and then the old +0x1c, +0x60 from +0x64
pub fn ai_save(m: &mut Mem, this: u32, s: u32) {
    part_save(m, this + 0xc, 0x18, 0xdead_bef1, s);
    crate::bxlist::save_counters(m, s);
}
pub fn ai_load(m: &mut Mem, env: &mut dyn LoadEnv, this: u32, s: u32) {
    read_into(m, s, this + 0xc, 0x18);
    crate::bxlist::load_counters(m, s);
    let u = m.r32(this + 0x1c);
    env.call(m, 0x1125c8, &[this, m.r32(this + 0x20)]);
    m.w32(this + 0x60, m.r32(this + 100));
    env.call(m, 0x1125c8, &[this, u]);
}

/// `cAIWorld::SaveSnapshot` 0x1164c0 / its load 0x1164d8: its 0x1b0 bytes of cell numbers (+0x36c); on load the active
/// cells released (0x1163a0) and the instances reset (`World_ResetInstances` 0x115fe8) first, then each of the 0x6c
/// slots (+0xc, 8 bytes) set from its number: -1 empty, else the course's cell (0xffff none) marked in use and
/// woken (`FUN_00260060` on 0x346488)
pub fn ai_world_save(m: &mut Mem, this: u32, s: u32) {
    stream_write_mem(m, s, this + 0x36c, 0x1b0);
}
pub fn ai_world_load(m: &mut Mem, env: &mut dyn LoadEnv, this: u32, s: u32) {
    env.call(m, 0x1163a0, &[this]);
    env.call(m, 0x115fe8, &[this]);
    read_into(m, s, this + 0x36c, 0x1b0);
    for k in 0..0x6cu32 {
        let v = m.r32(this + 0x36c + 4 * k);
        let out = this + 0xc + 8 * k;
        if v == u32::MAX {
            m.w32(out, 0);
            m.w16(out + 4, 0);
            m.w16(out + 6, 0);
        } else {
            let cell = if v != 0xffff { m.r32(m.r32(m.r32(0x346488) + 0x4c).wrapping_add(v.wrapping_mul(4))) } else { 0 };
            m.w16(out + 4, 1);
            m.w32(out, cell);
            env.call(m, 0x260060, &[0x346488, cell]);
            m.w16(out + 6, 0);
        }
    }
}

// The last load constructors (board row G12m): the roller (its rigid body, sphere tree and box) and the anim objects
// (their overlap box and key frames).

/// What the roller and the anim objects need beyond `LoadEnv::call`
pub trait LoadEnv2: LoadEnv {
    /// `MemPool_Alloc` 0x23d3b0 with its flags (the anim objects' frames ask 0x50000000)
    fn pool_alloc_flags(&mut self, m: &mut Mem, size: u32, flags: u32, name: u32) -> u32;
    /// `RollerNode_PlaceFromBody` 0x13e6a0(out, node): the roller's position and orientation (8 words)
    fn roller_place(&mut self, m: &mut Mem, node: u32) -> [u32; 8];
    /// `Mat4_FromPosQuat` 0x250428(out, in): the matrix (16 words) of a position and orientation
    fn mat4_from_pos_quat(&mut self, m: &mut Mem, pq: [u32; 8]) -> [u32; 16];
    /// `SphTree_SetTransform` 0x239d60(tree, matrix)
    fn sph_tree_set_transform(&mut self, m: &mut Mem, tree: u32, mat: [u32; 16]);
}

fn vu(a: [u32; 4], b: [u32; 4], add: bool) -> [u32; 4] {
    use crate::ps2f::F;
    let mut o = [0u32; 4];
    for k in 0..4 {
        let (x, y) = (F(f32::from_bits(a[k])), F(f32::from_bits(b[k])));
        o[k] = if add { (x + y).0.to_bits() } else { (x - y).0.to_bits() };
    }
    o
}

fn quad(m: &Mem, at: u32) -> [u32; 4] {
    [m.r32(at), m.r32(at + 4), m.r32(at + 8), m.r32(at + 12)]
}

fn put_quad(m: &mut Mem, at: u32, q: [u32; 4]) {
    for (k, v) in q.iter().enumerate() {
        m.w32(at + 4 * k as u32, *v);
    }
}

/// `cRollerNode` 0x13dee8(list, stream): the controller back, the rigid body (+0x30, `RigidBody_Construct` 0x1537d0),
/// the sphere tree (+0x150, `SphereTree::SphereTree` 0x239ba8, then the `bxSphereTree` vtable 0x3a4018) and the box
/// (+0x2c0, `BroadphaseProxy_Init`) made; its 0x90 bytes at +0xc0 and the body's 0x90-byte header back; the instance's
/// sphere tree copied (`World_GetInstanceSphTree` 0x260ca0, `SphTree_Copy` 0x2391e8) and placed where the body is;
/// the box from +0x1e0 less and plus the tree's radius three times over (PS2 floats), fitted into the broadphase
pub fn roller_load(m: &mut Mem, env: &mut dyn LoadEnv2, node: u32, l: u32, s: u32) {
    controller_load(m, node, l, s);
    env.call(m, 0x1537d0, &[node + 0x30]);
    m.w32(node + 8, 0x36d790);
    env.call(m, 0x239ba8, &[node + 0x150]);
    m.w32(node + 0x150, 0x3a4018);
    env.call(m, 0x150670, &[node + 0x2c0]);
    read_into(m, s, node + 0xc0, 0x90);
    read_into(m, s, node + 0x30, 0x90);
    let p = env.call(m, 0x260ca0, &[0x346488, m.r32(node + 0x28)]);
    let tree = m.r32(p);
    env.call(m, 0x2391e8, &[node + 0x150, tree]);
    let place = env.roller_place(m, node);
    let mat = env.mat4_from_pos_quat(m, place);
    env.sph_tree_set_transform(m, node + 0x150, mat);
    for (add, out) in [(false, 0x2a0u32), (true, 0x2b0)] {
        let r1 = env.call(m, 0x239c50, &[node + 0x150]);
        let r2 = env.call(m, 0x239c50, &[node + 0x150]);
        let r3 = env.call(m, 0x239c50, &[node + 0x150]);
        let v = vu(quad(m, node + 0x1e0), [r1, r2, r3, 0], add);
        put_quad(m, node + out, v);
    }
    m.w32(node + 0x344, node);
    env.call(m, 0x123098, &[node + 0x2c0, node + 0x2a0]);
    let bp = broadphase(m);
    env.call(m, 0x14fa70, &[bp, node + 0x2c0]);
}

/// `AnimObjectNode_CreateOverlap` 0x1999e8(node): a 0xe0-byte overlap (+0x64) on the node's instance (+0x58): an
/// identity rotation, a cube around the instance's position whose half side is the largest of the instance's box
/// corners' distances (each axis, both corners, at least 1e-10), its box (+0x40) set to the cube and fitted into the
/// broadphase; the instance's +0xe8 bits 5 and 6 cleared
pub fn create_overlap(m: &mut Mem, env: &mut dyn LoadEnv2, node: u32) {
    let o = env.pool_alloc_flags(m, 0xe0, 0x4000_0000, 0x38c348);
    if o != 0 {
        crate::replaymgr::mem_set_pub(m, o, 0xdead_c0de, 0xe0);
    }
    env.call(m, 0x150670, &[o + 0x40]);
    m.w32(node + 100, o);
    let inst = m.r32(node + 0x58);
    let one = 1f32.to_bits();
    let pos = quad(m, inst + 0x30);
    let d1 = vu([m.r32(inst + 0xcc), m.r32(inst + 0xd0), m.r32(inst + 0xd4), one], pos, false);
    let d2 = vu([m.r32(inst + 0xd8), m.r32(inst + 0xdc), m.r32(inst + 0xe0), one], pos, false);
    let mut r = 1e-10f32;
    for v in [d1[0], d2[0], d1[1], d2[1], d1[2], d2[2]] {
        let a = f32::from_bits(v).abs();
        if r < a {
            r = a;
        }
    }
    let (rb, nb) = (r.to_bits(), (-r).to_bits());
    put_quad(m, o, [0, 0, 0, one]);
    let ov = m.r32(node + 100);
    m.w32(ov + 200, 0);
    m.w32(ov + 204, 0);
    m.w32(ov + 208, 0);
    put_quad(m, ov + 0x10, [rb, rb, rb, 0]);
    let pos = quad(m, m.r32(node + 0x58) + 0x30);
    let lo = vu(pos, [nb, nb, nb, 0], true);
    put_quad(m, m.r32(node + 100) + 0x20, lo);
    let pos = quad(m, m.r32(node + 0x58) + 0x30);
    let hi = vu(pos, [rb, rb, rb, 0], true);
    put_quad(m, m.r32(node + 100) + 0x30, hi);
    let ov = m.r32(node + 100);
    m.w32(ov + 0x40 + 132, node + 0x30);
    env.call(m, 0x1507e8, &[ov + 0x40, ov + 0x20, ov + 0x30]);
    let bp = broadphase(m);
    let ov = m.r32(node + 100);
    env.call(m, 0x14fa70, &[bp, ov + 0x40]);
    let inst = m.r32(node + 0x58);
    m.w32(inst + 0xe8, m.r32(inst + 0xe8) & 0xffff_ff9f);
}

/// `AnimObjectNode_LoadState` 0x1cb210: 0x48 bytes back, its key from the course's keys
pub fn anim_state_load(m: &mut Mem, p: u32, s: u32) {
    read_into(m, s, p, 0x48);
    let [v] = read_words::<1>(m, s);
    m.w32(p + 0x48, m.r32(m.r32(COURSE) + 0x6c).wrapping_add(v));
}

/// `AnimFrame_Clear` 0x1cb150 / `AnimFrame_SetIdentity` 0x1cb960 (the identity matrix at 0x346440 into +0x90)
pub fn anim_frame_clear(m: &mut Mem, f: u32) {
    for k in 0..16 {
        m.w32(f + 8 + 4 * k, 0);
        m.w32(f + 0x4c + 4 * k, 0);
    }
    m.w32(f + 0x48, 0);
}
pub fn anim_frame_set_identity(m: &mut Mem, f: u32) {
    for k in 0..16 {
        m.w32(f + 0x90 + 4 * k, m.r32(0x346440 + 4 * k));
    }
}

/// `cAnimObjectNode` 0x198b48(list, stream): the controller at +0x30 and the vtable (+0x38); its 0x30 bytes; an
/// overlap made and read back when it had one; a frame (0xd0 bytes, from a pool block of n × 0xd0 + 0x10 holding n)
/// for each of its model's animated parts (the instance +0xc0: +4 parts, +8 their 0x18-byte records, animated with
/// +0x10), each cleared, made identity and keyed (`AnimFrame_SetKey` 0x1cb0d8: the frame, the part's key, the node's +0x24),
/// then their saved states;
/// the frames applied (`MeshAnim_ApplyFrames` 0x199258)
pub fn anim_object_load(m: &mut Mem, env: &mut dyn LoadEnv2, node: u32, l: u32, s: u32) {
    controller_load(m, node + 0x30, l, s);
    m.w32(node + 0x38, 0x38c650);
    read_into(m, s, node, 0x30);
    m.w32(node + 100, 0);
    let [flag] = read_words::<1>(m, s);
    if flag != 0 {
        create_overlap(m, env, node);
        let ov = m.r32(node + 100);
        read_into(m, s, ov, 0x10);
        read_into(m, s, ov + 200, 0xc);
    }
    let model = m.r32(m.r32(node + 0x58) + 0xc0);
    let parts = m.r32(model + 8);
    m.w32(node + 0x5c, 0);
    let mut count = m.r32(model + 4) as i32;
    let mut n = 0u32;
    for i in 0..count.max(0) as u32 {
        if m.r32(parts + 0x18 * i + 0x10) != 0 {
            n += 1;
            m.w32(node + 0x5c, n);
            count = m.r32(model + 4) as i32;
        }
    }
    if n == 0 {
        m.w32(node + 0x60, 0);
    } else {
        let size = n * 0xd0 + 0x10;
        let p = env.pool_alloc_flags(m, size, 0x5000_0000, 0x38c338);
        if p != 0 {
            crate::replaymgr::mem_set_pub(m, p, 0xdead_c0ed, size);
        }
        m.w32(p, n);
        for k in 0..n {
            let f = p + 0x10 + 0xd0 * k;
            m.w32(f + 0x8c, 0x391b10);
            anim_frame_clear(m, f);
            m.w32(f + 0x8c, 0x391af8);
            anim_frame_set_identity(m, f);
        }
        m.w32(node + 0x60, p + 0x10);
        count = m.r32(model + 4) as i32;
    }
    let mut j = 0u32;
    for i in 0..count.max(0) as u32 {
        let key = m.r32(parts + 0x18 * i + 0x10);
        if key != 0 {
            // (the frame, the part's key, and the node's +0x24 as a float in f12)
            let t = m.r32(node + 0x24);
            env.call(m, 0x1cb0d8, &[m.r32(node + 0x60) + j, key, t]);
            j += 0xd0;
            count = m.r32(model + 4) as i32;
            let _ = count;
        }
    }
    let n = m.r32(node + 0x5c) as i32;
    for k in 0..n.max(0) as u32 {
        let f = m.r32(node + 0x60) + 0xd0 * k;
        anim_state_load(m, f, s);
    }
    env.call(m, 0x199258, &[node]);
}

/// `cAnimComboNode` 0x19a0a8: the anim object at +0x1c, its vtable (+0x54), 0x1c bytes, then a pool block of +0x78 ×
/// 64 bytes (+0x84) read back
pub fn anim_combo_load(m: &mut Mem, env: &mut dyn LoadEnv2, node: u32, l: u32, s: u32) {
    anim_object_load(m, env, node + 0x1c, l, s);
    m.w32(node + 0x54, 0x38c460);
    read_into(m, s, node, 0x1c);
    let n = m.r32(node + 0x78);
    let p = env.pool_alloc_flags(m, n << 6, 0x4000_0000, 0x38c358);
    if p != 0 {
        crate::replaymgr::mem_set_pub(m, p, 0xdead_c0ed, n << 6);
    }
    m.w32(node + 0x84, p);
    let n = m.r32(node + 0x78);
    read_into(m, s, p, n << 6);
}

/// `cAnimDeltaNode` 0x199d68: the anim object, its vtable (0x38c558) and the word at +0x68
pub fn anim_delta_load(m: &mut Mem, env: &mut dyn LoadEnv2, node: u32, l: u32, s: u32) {
    anim_object_load(m, env, node, l, s);
    m.w32(node + 0x38, 0x38c558);
    read_into(m, s, node + 0x68, 4);
}

/// `cAnimTexFlipNode` 0x19a700: the anim object at +0x18, its vtable (+0x50), its tex flip (+0x80) made, 0x18 bytes,
/// the tex flip started
pub fn anim_tex_flip_load(m: &mut Mem, env: &mut dyn LoadEnv2, node: u32, l: u32, s: u32) {
    anim_object_load(m, env, node + 0x18, l, s);
    m.w32(node + 0x50, 0x38c368);
    env.call(m, 0x142ac8, &[node + 0x80]);
    read_into(m, s, node, 0x18);
    let (a, b) = (m.r32(node + 0x70), m.r32(node + 0x10));
    env.call(m, 0x142af0, &[node + 0x80, a, b]);
}
