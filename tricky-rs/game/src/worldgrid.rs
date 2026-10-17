//! The world grid's position lookup (board row G7c4): `WorldGrid_CellAtPos` 0x260b18, the `.ltg` grid's INode
//! under a point, on `tricky_data::ltg::Ltg` (whose `inode_at` is `WorldGrid_CellAt` 0x260ba8). Checked in the
//! function runner on every course's grid (`tests/ltg.rs`).

use crate::animseq::{cvt_s_w, div};
use crate::ps2f::F;
use tricky_data::ltg::{INode, Ltg};

/// The INode coordinates of (x, y) as `WorldGrid_CellAtPos` makes them: an INode's size is the TGroup size over
/// the INodes a side, made a whole number (toward zero); then (p − the grid's corner) over that, toward zero. The EE's
/// division and conversions (both toward zero)
pub fn inode_coords(ltg: &Ltg, x: f32, y: f32) -> (i32, i32) {
    let size = cvt_s_w(div(F(ltg.tgroup_size), cvt_s_w(ltg.inode_wide)).ee() as i32);
    let cx = div(F(x) - F(ltg.bbox.min[0]), size).ee() as i32;
    let cy = div(F(y) - F(ltg.bbox.min[1]), size).ee() as i32;
    (cx, cy)
}

/// `WorldGrid_CellAtPos` 0x260b18(&grid, p, &x, &y): with no grid, nothing (0, the coordinates not written);
/// otherwise the coordinates ([`inode_coords`], written out) and the INode there (`WorldGrid_CellAt`)
pub fn cell_at_pos(ltg: Option<&Ltg>, x: f32, y: f32) -> Option<((i32, i32), Option<&INode>)> {
    let ltg = ltg?;
    let (cx, cy) = inode_coords(ltg, x, y);
    Some(((cx, cy), ltg.inode_at(cx, cy)))
}
