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
    /// each object's flags (+0xe8): as the course loaded them, then as its controllers set them
    /// ([`CourseGround::set_flags`])
    flags: Mutex<Vec<u32>>,
    /// each patch's coefficients (+0x50)
    coeffs: Vec<[V4; 16]>,
    /// STANDIN: every patch tessellated stays (about 7 KB each), for the game's fixed-size cache and its eviction
    /// (`World_LookupCollCache`), which only change how fast a cast is, not what it hits
    cache: Mutex<HashMap<usize, PatchGrid>>,
    /// the keyframed objects' animated parts' frames as their clocks stand ([`CourseGround::pose`])
    poses: Mutex<HashMap<usize, Vec<[V4; 4]>>>,
    /// the course's `.ssf` (its scripts, physics records and sphere trees)
    pub ssf: tricky_data::ssf::Ssf,
    /// the knockable objects that are moving and the script threads the riders' contacts started (row E8c3)
    pub knock: Mutex<crate::knockables::Knockables>,
    /// the world grid as `Rail_ClosestPoint` 0x259330 walks it (row G20c2c1a, [`rail_world`])
    pub rails: crate::rider::ground_tick::RailWorld,
    /// STANDIN: the objects whose persistent script spawns a controller other than a roller (op 0 sub-type not 0:
    /// animated doors, boosts...), which the port does not run (row E7b2); the rider's instance query passes
    /// through them unless the port poses them ([`CourseGround::pose`]), as the app did before row E8c3
    driven: Vec<bool>,
}

struct Env<'a> {
    coeffs: &'a [[V4; 16]],
    cache: &'a mut HashMap<usize, PatchGrid>,
    poses: &'a HashMap<usize, Vec<[V4; 4]>>,
}

impl WorldEnv for Env<'_> {
    /// The shape's +0x48 on the patch's cache entry; `last` notes the patch as its index + 1 (the game notes the
    /// patch's address, never 0, which a cleared record holds).
    fn patch(&mut self, q: &dyn Shape, i: usize, last: Option<&mut LastHit>) -> Option<Hit> {
        let coeffs = self.coeffs;
        let grid = self.cache.entry(i).or_insert_with(|| PatchGrid::new(&coeffs[i]));
        q.patch(grid, i as u32 + 1, last).map(|h| h.hit)
    }
    /// `MeshAnimNode_GetFrameMatrix` 0x1998d0: the frame [`CourseGround::pose`] last gave (the identity for none).
    fn frame(&mut self, object: usize, n: usize) -> [V4; 4] {
        self.poses.get(&object).and_then(|f| f.get(n)).copied().unwrap_or([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
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
        let flags = Mutex::new(world.objects.iter().map(|o| o.flags).collect());
        let knock = Mutex::new(crate::knockables::Knockables::default());
        let driven = (0..world.objects.len())
            .map(|o| ssf.effects.get(ssf.event_script(o, 0) as usize).is_some_and(|ops| ops.iter().any(|op| op.op == 0 && op.args.first().is_some_and(|&t| t != 0))))
            .collect();
        let rails = rail_world(&world, ltg_bytes, &ltg, &pbd);
        Some(CourseGround { world: Mutex::new(world), flags, coeffs, cache: Mutex::new(HashMap::new()), poses: Mutex::new(HashMap::new()), ssf, knock, driven, rails })
    }

    /// `World_RayCast` 0x25aff8 on the course: the distance along the query (−1 for nothing) and what was hit.
    /// `show_off` for the show-off modes (3, 5); `last` the caller's last-patch record.
    ///
    /// STANDIN: `skip(instance)` clears a solid object's bit 0x20 for this cast (the port's switched-off, broken
    /// and opened objects), for the game's own switching of an instance (scripts, glass, doors; row E9b2).
    pub fn ray_cast(&self, q: &mut dyn Shape, show_off: bool, last: Option<&mut LastHit>, skip: &dyn Fn(u32) -> bool) -> (f32, Option<CastHit>) {
        let mut world = self.world.lock().unwrap_or_else(|e| e.into_inner());
        for (i, (o, f)) in world.objects.iter_mut().zip(self.flags.lock().unwrap_or_else(|e| e.into_inner()).iter()).enumerate() {
            o.flags = if f & 0x20 != 0 && skip(i as u32) { f & !0x20 } else { *f };
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let poses = self.poses.lock().unwrap_or_else(|e| e.into_inner());
        let mut env = Env { coeffs: &self.coeffs, cache: &mut cache, poses: &poses };
        world.ray_cast(q, show_off, last, &mut env)
    }

    /// `World_QueryInstances` 0x25bdc4 on the course: the objects only, the deepest touch (−1 for nothing).
    /// `skip` as [`CourseGround::ray_cast`]'s.
    pub fn query_instances(&self, q: &mut dyn Shape, show_off: bool, skip: &dyn Fn(u32) -> bool) -> (f32, Option<CastHit>) {
        let mut world = self.world.lock().unwrap_or_else(|e| e.into_inner());
        let poses = self.poses.lock().unwrap_or_else(|e| e.into_inner());
        for (i, (o, f)) in world.objects.iter_mut().zip(self.flags.lock().unwrap_or_else(|e| e.into_inner()).iter()).enumerate() {
            let skip = skip(i as u32) || self.driven[i] && !poses.contains_key(&i);
            o.flags = if f & 0x20 != 0 && skip { f & !0x20 } else { *f };
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mut env = Env { coeffs: &self.coeffs, cache: &mut cache, poses: &poses };
        world.query_instances(q, show_off, &mut env)
    }

    /// An object's flags (+0xe8) as they stand (0 for none).
    pub fn object_flags(&self, object: usize) -> u32 {
        self.flags.lock().unwrap_or_else(|e| e.into_inner()).get(object).copied().unwrap_or(0)
    }

    /// An object's flags (+0xe8) set (by its controller).
    pub fn set_flags(&self, object: usize, flags: u32) {
        if let Some(f) = self.flags.lock().unwrap_or_else(|e| e.into_inner()).get_mut(object) {
            *f = flags;
        }
    }

    /// `WorldNode_RayCast` 0x13aad0 = `World_RayCastInstance` 0x25bf48 on the course for one object (instance):
    /// the distance along the query (> 0 when it hits) and what was hit.
    pub fn ray_cast_instance(&self, q: &mut dyn Shape, object: usize) -> (f32, Option<CastHit>) {
        let world = self.world.lock().unwrap_or_else(|e| e.into_inner());
        if object >= world.objects.len() {
            return (-1.0, None);
        }
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let poses = self.poses.lock().unwrap_or_else(|e| e.into_inner());
        let mut env = Env { coeffs: &self.coeffs, cache: &mut cache, poses: &poses };
        world.ray_cast_instance(q, object, &mut env)
    }

    /// The world as it stands (for tests and tools).
    pub fn with_world<R>(&self, f: impl FnOnce(&World) -> R) -> R {
        f(&self.world.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// A keyframed object's (`cAnimObjectNode`) animated parts are at `frames` (the n-th animated part's local
    /// matrix, game units, `MeshAnimNode_GetFrameMatrix` 0x1998d0): its controller (+0xe4) animated (+0x14 bit
    /// 0x100), so `World_RayCastInstance` poses those parts with them.
    /// STANDIN: the object's box (+0xcc) is its parts' mesh boxes in that pose, for the game's run-time box (row
    /// E7a6); the controller is only its animated bit, for the course's controllers (row E7b2).
    pub fn pose(&self, object: u32, frames: Vec<[V4; 4]>) {
        let o = object as usize;
        let mut world = self.world.lock().unwrap_or_else(|e| e.into_inner());
        let Some(obj) = world.objects.get(o) else { return };
        let (rows, model) = (obj.rows, obj.model);
        let (min, max) = crate::world::object_box(rows, world.models.get(model).map_or(&[][..], |v| &v[..]), &frames);
        let obj = &mut world.objects[o];
        (obj.min, obj.max, obj.controller) = (min, max, Some(0x100));
        self.poses.lock().unwrap_or_else(|e| e.into_inner()).insert(o, frames);
    }
}

/// The world grid `Rail_ClosestPoint` 0x259330 walks (row G20c2c1a), from the course as the game resolves it: the same
/// grid as `World_RayCast`'s (the world's origin, cell side and size), each cell a `.ltg` TGroup (its box) whose 16
/// INodes are the patch records (their box; the in-use words +0x24, +0x28, the halfword +0x2c and the word +0x48 as the
/// file has them, nonzero where the game's are), each INode's spline list (+0x2a count, +0x3c offset from its TGroup)
/// the `.pbd` spline segments it holds (checked on gari: every entry a segment index whose box meets the INode's).
/// A curve is the segment's four rows (+0x00..+0x3c: the cubic's coefficients), its box (+0x5c) and its spline (+0x58)
/// as the owner: the spline record's +0x18 (bit 0, the rail switched on: clear in the file, set when the rail's
/// `cRailNode` comes on, [`crate::rider::ground_tick::RailWorld::set_live`]) and +0x24. The game turns the indices
/// into pointers after loading; the port keeps the records.
pub fn rail_world(world: &World, ltg_bytes: &[u8], ltg: &tricky_data::ltg::Ltg, pbd: &tricky_data::pbd::Pbd) -> crate::rider::ground_tick::RailWorld {
    use crate::rider::ground_tick::{RailCell, RailCurve, RailOwner, RailPatch, RailWorld};
    let f = |w: u32| f32::from_bits(w);
    let curve = |k: u32| -> Option<RailCurve> {
        let s = pbd.segments.get(k as usize)?;
        let row = |r: usize| [f(s[4 * r]), f(s[4 * r + 1]), f(s[4 * r + 2]), f(s[4 * r + 3])];
        let spline = tricky_data::pbd::SegmentRec(*s).spline();
        let owner = pbd.splines.get(spline as usize).map(|sp| RailOwner { flags: sp[6], tag: sp[9] });
        Some(RailCurve { m: [row(0), row(1), row(2), row(3)], owner, bbox: [f(s[0x17]), f(s[0x18]), f(s[0x19]), f(s[0x1a]), f(s[0x1b]), f(s[0x1c])], spline: Some(spline) })
    };
    let word = |at: usize| ltg_bytes.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let cells = ltg
        .tgroups
        .iter()
        .map(|g| {
            g.as_ref().map(|(off, g)| RailCell {
                bbox: [g.bbox.min[0], g.bbox.min[1], g.bbox.min[2], g.bbox.max[0], g.bbox.max[1], g.bbox.max[2]],
                patches: g
                    .inodes
                    .iter()
                    .map(|n| {
                        let at = *off as usize + n.offsets[2] as usize;
                        let curves = (0..n.counts[3].max(0) as usize).filter_map(|k| curve(word(at + 4 * k))).collect();
                        RailPatch {
                            bbox: [n.bbox.min[0], n.bbox.min[1], n.bbox.min[2], n.bbox.max[0], n.bbox.max[1], n.bbox.max[2]],
                            x24: (n.counts[0] as u16 as u32) | (n.counts[1] as u16 as u32) << 16,
                            x28: n.counts[2] as u16,
                            x2c: n.counts[4],
                            x48: n.offsets[5],
                            curves,
                        }
                    })
                    .collect(),
            })
        })
        .collect();
    RailWorld { x0: world.origin[0], y0: world.origin[1], cell: world.size, nx: world.width, ny: world.height, cells }
}
