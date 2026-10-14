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
