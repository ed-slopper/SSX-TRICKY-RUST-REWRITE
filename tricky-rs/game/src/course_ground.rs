//! The course's own collision for the port (board row E7c3): the world the casts run on ([`World::from_course`],
//! from the course's `.pbd`, `.ltg` and `.ssf`) and its patch cache (`World_LookupCollCache`: a patch tessellated
//! the first time a query needs it, `terrain::PatchGrid`). `World_RayCast` 0x25aff8 on it is what the ground
//! probe and the other casts ask, in the game's units (cm, z up).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::terrain::{LastHit, PatchGrid};
use crate::world::{CastHit, Shape, World, WorldEnv};
use crate::worldline::Hit;

type V4 = [f32; 4];

/// The course's world and its patch cache.
pub struct CourseGround {
    /// the world, whose objects' flags +0xe8 bit 0x20 (solid) the port switches (see [`CourseGround::ray_cast`])
    world: Mutex<World>,
    /// each object's flags as the course loaded them
    flags: Vec<u32>,
    /// each patch's coefficients (+0x50)
    coeffs: Vec<[V4; 16]>,
    /// STANDIN: every patch tessellated stays (about 7 KB each), for the game's fixed-size cache and its eviction
    /// (`World_LookupCollCache`), which only change how fast a cast is, not what it hits
    cache: Mutex<HashMap<usize, PatchGrid>>,
}

struct Env<'a> {
    coeffs: &'a [[V4; 16]],
    cache: &'a mut HashMap<usize, PatchGrid>,
}

impl WorldEnv for Env<'_> {
    /// The shape's +0x48 on the patch's cache entry; `last` notes the patch as its index + 1 (the game notes the
    /// patch's address, never 0, which a cleared record holds).
    fn patch(&mut self, q: &dyn Shape, i: usize, last: Option<&mut LastHit>) -> Option<Hit> {
        let coeffs = self.coeffs;
        let grid = self.cache.entry(i).or_insert_with(|| PatchGrid::new(&coeffs[i]));
        q.patch(grid, i as u32 + 1, last).map(|h| h.hit)
    }
    /// STANDIN: the rest pose; the keyframed objects are left out of the port's casts (row E7c3b)
    fn frame(&mut self, _: usize, _: usize) -> [V4; 4] {
        [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
    }
}

impl CourseGround {
    /// From the course's files (`data/models/<course>.pbd`, `.ltg`, `.ssf`); `None` when one does not parse.
    pub fn new(pbd_bytes: &[u8], ltg_bytes: &[u8], ssf_bytes: &[u8]) -> Option<CourseGround> {
        let pbd = tricky_data::pbd::Pbd::parse(pbd_bytes)?;
        let ltg = tricky_data::ltg::Ltg::parse(ltg_bytes)?;
        let ssf = tricky_data::ssf::Ssf::parse(ssf_bytes, pbd.instances.len() as u32).ok()?;
        let coeffs = pbd.patches.iter().map(|r| tricky_data::pbd::PatchRec::parse(r).map_or([[0.0; 4]; 16], |p| p.coeffs)).collect();
        let world = World::from_course(pbd_bytes, &pbd, ltg_bytes, &ltg, &ssf);
        let flags = world.objects.iter().map(|o| o.flags).collect();
        Some(CourseGround { world: Mutex::new(world), flags, coeffs, cache: Mutex::new(HashMap::new()) })
    }

    /// `World_RayCast` 0x25aff8 on the course: the distance along the query (−1 for nothing) and what was hit.
    /// `show_off` for the show-off modes (3, 5); `last` the caller's last-patch record.
    ///
    /// STANDIN: `skip(instance)` clears a solid object's bit 0x20 for this cast (the port's switched-off, broken,
    /// opened and keyframed objects), for the game's own switching of an instance (scripts, glass, doors; rows
    /// E9b2, E7c3b).
    pub fn ray_cast(&self, q: &mut dyn Shape, show_off: bool, last: Option<&mut LastHit>, skip: &dyn Fn(u32) -> bool) -> (f32, Option<CastHit>) {
        let mut world = self.world.lock().unwrap_or_else(|e| e.into_inner());
        for (i, (o, f)) in world.objects.iter_mut().zip(&self.flags).enumerate() {
            o.flags = if f & 0x20 != 0 && skip(i as u32) { f & !0x20 } else { *f };
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut env = Env { coeffs: &self.coeffs, cache: &mut cache };
        world.ray_cast(q, show_off, last, &mut env)
    }
}
