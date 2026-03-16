//
/// Rat Race 2 - Tilemap Editor & Game
///

use crate::prelude::*;

use std::collections::HashMap;

const CHUNK_SIZE: usize = 8; // must be power-of-2 for morton encoding to produce dense indices
const TILE_SIZE: f32 = 10.0;
const GRID_SIZE: i32 = 2000;
const ARRAY_SIZE: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

/// Init function for ratrace demo
#[no_mangle]
pub fn ratrace2(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_ratrace2"
        ],
        update: systems![
            "update_tile_editor_ui",
            "update_flow_field",
            "update_tile_editor",
            "update_agents",
            "update_agent_transforms",
            "debug_draw_agents"
        ],
        render_graph: "mesh_wireframe_overlay"
    }
}

#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum TileType {
    Empty,
    Wall,
    Barrier,
    Escalator,
    Platform,
    TrainTrack,
}

/// Baked wall line segment with inward-facing normal, in absolute world space (y=0)
struct WallLine {
    p0:   Vec3f,
    p1:   Vec3f,
    perp: Vec3f, // inward normal pointing from wall surface toward free tile
}

pub struct MapChunk {
    /// enum of tile type
    tiles: [TileType; ARRAY_SIZE],
    /// 2D diffusion flow field
    flow: [Vec2f; ARRAY_SIZE],
    /// agent count per cell — reset each frame alongside agents
    density: [f32; ARRAY_SIZE],
    /// pressure curve based on density, controls flow
    pressure: [f32; ARRAY_SIZE],
    /// per-cell agent entity IDs (SoA); cleared and rebuilt each frame
    agents: [Vec<Entity>; ARRAY_SIZE],
    /// baked wall lines for collision, updated when map.dirty
    walls: [Vec<WallLine>; ARRAY_SIZE],
}

impl MapChunk {
    fn new() -> Self {
        Self {
            tiles:    [TileType::Empty; ARRAY_SIZE],
            flow:     [Vec2f::new(0.0, 1.0); ARRAY_SIZE],
            density:  [0.0; ARRAY_SIZE],
            pressure: [0.0; ARRAY_SIZE],
            agents:   std::array::from_fn(|_| Vec::new()),
            walls:    std::array::from_fn(|_| Vec::new()),
        }
    }

    fn clear_agents(&mut self) {
        for (v, d) in self.agents.iter_mut().zip(self.density.iter_mut()) {
            v.clear();
            *d = 0.0;
        }
    }
}

/// Spread bits of a value into every-3rd-bit position for 3D morton encoding
fn spread_bits_3d(mut v: u32) -> u32 {
    v &= 0x000003FF;
    v = (v | (v << 16)) & 0x030000FF;
    v = (v | (v <<  8)) & 0x0300F00F;
    v = (v | (v <<  4)) & 0x030C30C3;
    v = (v | (v <<  2)) & 0x09249249;
    v
}

/// Encode local chunk coords (x, y, z) into a morton z-order index
fn morton_encode(x: usize, y: usize, z: usize) -> usize {
    (spread_bits_3d(x as u32) | (spread_bits_3d(y as u32) << 1) | (spread_bits_3d(z as u32) << 2)) as usize
}

/// Pack chunk coordinates into a u64 key (supports negative coords)
fn chunk_key(cx: i32, cz: i32) -> u64 {
    let x = cx as i64 as u64;
    let z = cz as i64 as u64;
    (x & 0xFFFFFFFF) | ((z & 0xFFFFFFFF) << 32)
}

/// Unpack chunk key back into (cx, cz)
fn chunk_key_unpack(key: u64) -> (i32, i32) {
    let cx = (key & 0xFFFFFFFF) as u32 as i32;
    let cz = ((key >> 32) & 0xFFFFFFFF) as u32 as i32;
    (cx, cz)
}

/// Floor division that handles negatives correctly
fn floor_div(a: i32, b: i32) -> i32 {
    if a >= 0 || a % b == 0 { a / b } else { a / b - 1 }
}

/// World tile coord to chunk coord + local offset
fn tile_to_chunk(tx: i32, ty: i32, tz: i32) -> (i32, i32, i32, usize, usize, usize) {
    let cs = CHUNK_SIZE as i32;
    let cx = floor_div(tx, cs);
    let cy = floor_div(ty, cs);
    let cz = floor_div(tz, cs);
    let lx = (tx - cx * cs) as usize;
    let ly = (ty - cy * cs) as usize;
    let lz = (tz - cz * cs) as usize;
    (cx, cy, cz, lx, ly, lz)
}

/// Convert world-space position to 2D tile coordinates (Vec2i.x = tile X, Vec2i.y = tile Z)
fn world_to_tile(pos: Vec3f) -> Vec2i {
    Vec2i::new(
        (pos.x / TILE_SIZE).floor() as i32,
        (pos.z / TILE_SIZE).floor() as i32,
    )
}

/// Simple deterministic hash of a spawn position → value in [0, 1)
fn pos_hash(x: f32, z: f32) -> f32 {
    let bits = x.to_bits() ^ z.to_bits().wrapping_mul(2654435761);
    let mixed = (bits as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (mixed >> 33) as f32 / u32::MAX as f32
}

const TILE_NAMES: &[&str] = &[
    "Wall",
    "Barrier",
    "Escalator",
    "Platform",
    "TrainTrack"
];

const PLACE_NAMES: &[&str] = &[
    "Wall",
    "Barrier",
    "Escalator",
    "Platform",
    "TrainTrack",
    "Agent"
];

const MAP_SAVE_PATH: &str = "ratrace2_map.bin";

#[derive(PartialEq)]
pub enum PlaceMode {
    Tile(usize),
    Agent,
}

const AGENT_SPEED: f32 = 0.1;
const AGENT_RADIUS: f32 = 3.0;
const AGENT_BODY_HALF_WIDTH: f32 = 1.5;  // half-extent x/z (body 3 units wide)
const AGENT_BODY_HALF_HEIGHT: f32 = 4.5; // half-extent y (body 9 units tall, ~1:3 ratio)
const CORRECTION_ITERATIONS: usize = 4;
const TURN_RATE: f32 = 0.1; // fraction of angular gap closed per frame (exponential smoothing)

/// Separation radius at low density (shrinks as crowd grows)
const BASE_SEP_RADIUS: f32 = 24.0;
const MIN_SEP_RADIUS: f32 = 8.0;
const SEP_RADIUS_DECAY: f32 = 0.25;   // world units of radius lost per agent
const SEPARATION_STRENGTH: f32 = 0.02;   // world units per frame at full push (flow dominates; sep spikes when critically close)
const WANDER_DRIFT: f32 = 0.012;  // radians per frame
const WANDER_STRENGTH: f32 = 0.08;   // fraction of agent speed
const WALL_AVOID_RADIUS: f32 = AGENT_RADIUS * 2.5; // soft avoidance lookahead, larger than collision radius

const TILE_TYPES: &[TileType] = &[
    TileType::Wall,
    TileType::Barrier,
    TileType::Escalator,
    TileType::Platform,
    TileType::TrainTrack
];

/// Shared cube mesh resource for agent body rendering
#[derive(Resource)]
pub(crate) struct AgentMesh(pmfx::Mesh<gfx_platform::Device>);

/// Marker — identifies human agents in the ECS
#[derive(Component)]
pub(crate) struct HumanAgent;

/// World position component (XZ for grid navigation, Y held for rendering)
#[derive(Component)]
pub(crate) struct AgentPos(Vec3f);

/// Per-agent speed multiplier randomised at spawn [0.8, 1.2]
#[derive(Component)]
pub(crate) struct SpeedScale(f32);

/// Slowly drifting random direction bias angle (radians), unique per agent
#[derive(Component)]
pub(crate) struct WanderAngle(f32);

/// Accumulated separation force from nearby agents — cleared each frame
#[derive(Component)]
pub(crate) struct SepForce(Vec2f);

/// Accumulated local density from all registered grid cells — cleared each frame
#[derive(Component)]
pub(crate) struct LocalDensity(f32);

/// Current facing rotation — smoothly nlerp'd toward velocity direction each frame
#[derive(Component)]
pub(crate) struct FacingRot(Quatf);

#[derive(Resource)]
pub struct EditorState {
    selected: PlaceMode,
    left_was_down: bool,
}

#[derive(Resource)]
pub struct Map {
    chunks: HashMap<u64, MapChunk>,
    dirty: bool,
}

impl Map {
    fn new() -> Self {
        Self { chunks: HashMap::new(), dirty: true }
    }

    fn set_tile(&mut self, tile: Vec2i, t: TileType) {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, 0, tile.y);
        let key = chunk_key(cx, cz);
        let chunk = self.chunks.entry(key).or_insert_with(MapChunk::new);
        chunk.tiles[morton_encode(lx, ly, lz)] = t;
        self.dirty = true;
    }

    fn get_flow(&self, tile: Vec2i) -> Vec2f {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, 0, tile.y);
        let key = chunk_key(cx, cz);
        match self.chunks.get(&key) {
            Some(chunk) => chunk.flow[morton_encode(lx, ly, lz)],
            None => Vec2f::zero(),
        }
    }

    fn get_walls(&self, tile: Vec2i) -> &[WallLine] {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, 0, tile.y);
        let key = chunk_key(cx, cz);
        match self.chunks.get(&key) {
            Some(chunk) => &chunk.walls[morton_encode(lx, ly, lz)],
            None => &[],
        }
    }

    fn get_tile(&self, tile: Vec2i) -> TileType {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, 0, tile.y);
        let key = chunk_key(cx, cz);
        match self.chunks.get(&key) {
            Some(chunk) => chunk.tiles[morton_encode(lx, ly, lz)],
            None => TileType::Empty,
        }
    }

    fn clear_all_agents(&mut self) {
        for chunk in self.chunks.values_mut() { chunk.clear_agents(); }
    }

    /// Register an agent entity into a tile cell; also increments density
    fn register_agent(&mut self, tile: Vec2i, entity: Entity) {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, 0, tile.y);
        if let Some(chunk) = self.chunks.get_mut(&chunk_key(cx, cz)) {
            let idx = morton_encode(lx, ly, lz);
            chunk.agents[idx].push(entity);
            chunk.density[idx] += 1.0;
        }
    }

    fn get_agents_at(&self, tile: Vec2i) -> &[Entity] {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, 0, tile.y);
        match self.chunks.get(&chunk_key(cx, cz)) {
            Some(chunk) => &chunk.agents[morton_encode(lx, ly, lz)],
            None => &[],
        }
    }

    fn save(&self) -> Result<(), hotline_rs::Error> {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"RR2M");
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        for (key, chunk) in &self.chunks {
            let (cx, cz) = chunk_key_unpack(*key);
            buf.extend_from_slice(&cx.to_le_bytes());
            buf.extend_from_slice(&cz.to_le_bytes());
            let tile_bytes: &[u8; ARRAY_SIZE] = unsafe { std::mem::transmute(&chunk.tiles) };
            buf.extend_from_slice(tile_bytes);
        }
        std::fs::write(MAP_SAVE_PATH, buf)?;
        Ok(())
    }

    fn load(path: &str) -> Result<Self, hotline_rs::Error> {
        let data = std::fs::read(path)?;
        let mut pos = 0usize;
        if data.get(pos..pos+4) != Some(b"RR2M") {
            return Err("invalid map file".into());
        }
        pos += 4;
        let _version = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
        pos += 4;
        let num_chunks = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;
        let mut map = Map::new();
        for _ in 0..num_chunks {
            let cx = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            pos += 4;
            let cz = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            pos += 4;
            let key = chunk_key(cx, cz);
            let chunk = map.chunks.entry(key).or_insert_with(MapChunk::new);
            let tile_bytes: [u8; ARRAY_SIZE] = data[pos..pos+ARRAY_SIZE].try_into().unwrap();
            chunk.tiles = unsafe { std::mem::transmute(tile_bytes) };
            pos += ARRAY_SIZE;
        }
        Ok(map)
    }
}

/// Dijkstra flood fill from all Platform tiles, writes flow + pressure into chunks
#[export_update_fn]
pub fn update_flow_field(mut map: ResMut<Map>) -> Result<(), hotline_rs::Error> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    if !map.dirty { return Ok(()); }
    map.dirty = false;

    // pass 1: reset flow/pressure and seed the heap with Platform tiles
    let mut cost: HashMap<(i32, i32), u32> = HashMap::new();
    let mut heap: BinaryHeap<(Reverse<u32>, i32, i32)> = BinaryHeap::new();

    for (key, chunk) in &mut map.chunks {
        let (cx, cz) = chunk_key_unpack(*key);
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let idx = morton_encode(lx, 0, lz);
                chunk.flow[idx] = Vec2f::zero();
                chunk.pressure[idx] = f32::MAX;
                if chunk.tiles[idx] == TileType::Platform {
                    let tx = cx * CHUNK_SIZE as i32 + lx as i32;
                    let tz = cz * CHUNK_SIZE as i32 + lz as i32;
                    cost.insert((tx, tz), 0);
                    heap.push((Reverse(0), tx, tz));
                }
            }
        }
    }

    // pass 2: Dijkstra flood fill
    const NEIGHBORS: [(i32, i32); 8] = [
        (1,0),(-1,0),(0,1),(0,-1),
        (1,1),(-1,1),(-1,-1),(1,-1)
    ];
    while let Some((Reverse(c), tx, tz)) = heap.pop() {
        if cost.get(&(tx, tz)).copied().unwrap_or(u32::MAX) < c { continue; }
        for (dx, dz) in NEIGHBORS {
            let (nx, nz) = (tx + dx, tz + dz);
            // only traverse tiles in existing chunks — prevents infinite expansion into void
            let (cx, _cy, cz, _, _, _) = tile_to_chunk(nx, 0, nz);
            if !map.chunks.contains_key(&chunk_key(cx, cz)) { continue; }
            let tile = map.get_tile(Vec2i::new(nx, nz));
            if tile == TileType::Wall || tile == TileType::Barrier { continue; }
            let new_cost = c + 1;
            if new_cost < cost.get(&(nx, nz)).copied().unwrap_or(u32::MAX) {
                cost.insert((nx, nz), new_cost);
                heap.push((Reverse(new_cost), nx, nz));
            }
        }
    }

    // pass 3: compute flow direction (gradient toward lowest-cost neighbour)
    let flow_updates: Vec<(i32, i32, Vec2f, f32)> = cost.iter()
        .filter(|(&(tx, tz), _)| map.get_tile(Vec2i::new(tx, tz)) != TileType::Platform)
        .map(|(&(tx, tz), &c)| {
            let mut best_dir = Vec2f::zero();
            let mut best_cost = c;
            for (dx, dz) in NEIGHBORS {
                let nc = cost.get(&(tx+dx, tz+dz)).copied().unwrap_or(u32::MAX);
                if nc < best_cost {
                    best_cost = nc;
                    best_dir = Vec2f::new(dx as f32, dz as f32);
                }
            }
            (tx, tz, best_dir, c as f32)
        })
        .collect();

    for (tx, tz, flow_dir, pressure) in flow_updates {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, 0, tz);
        let key = chunk_key(cx, cz);
        if let Some(chunk) = map.chunks.get_mut(&key) {
            let idx = morton_encode(lx, ly, lz);
            chunk.flow[idx] = flow_dir;
            chunk.pressure[idx] = pressure;
        }
    }

    // pass 4: bake wall lines per tile
    // collect all tile coords first to avoid borrow conflict
    let all_tiles: Vec<(i32, i32)> = map.chunks.iter()
        .flat_map(|(key, chunk)| {
            let (cx, cz) = chunk_key_unpack(*key);
            (0..CHUNK_SIZE).flat_map(move |lz| (0..CHUNK_SIZE).map(move |lx| {
                let tx = cx * CHUNK_SIZE as i32 + lx as i32;
                let tz = cz * CHUNK_SIZE as i32 + lz as i32;
                (tx, tz)
            }))
            .filter(|&(tx, tz)| {
                let idx = morton_encode((tx - cx * CHUNK_SIZE as i32) as usize, 0, (tz - cz * CHUNK_SIZE as i32) as usize);
                chunk.tiles[idx] != TileType::Wall
            })
            .collect::<Vec<_>>()
        })
        .collect();

    for (tx, tz) in all_tiles {
        let cx_f = tx as f32 * TILE_SIZE + TILE_SIZE * 0.5;
        let cz_f = tz as f32 * TILE_SIZE + TILE_SIZE * 0.5;

        let mut lines: Vec<WallLine> = Vec::new();

        // cardinal neighbors
        for (dx, dz) in [(-1i32,0i32),(1,0),(0,-1),(0,1)] {
            if map.get_tile(Vec2i::new(tx + dx, tz + dz)) != TileType::Wall { continue; }
            let (p0x, p0z, p1x, p1z) = match (dx, dz) {
                (-1, 0) => (-0.5, -0.5, -0.5,  0.5),
                ( 1, 0) => ( 0.5, -0.5,  0.5,  0.5),
                ( 0,-1) => (-0.5, -0.5,  0.5, -0.5),
                ( 0, 1) => (-0.5,  0.5,  0.5,  0.5),
                _ => unreachable!(),
            };
            let perp = vec3f(-dx as f32, 0.0, -dz as f32);
            lines.push(WallLine {
                p0: vec3f(cx_f + p0x * TILE_SIZE, 0.0, cz_f + p0z * TILE_SIZE),
                p1: vec3f(cx_f + p1x * TILE_SIZE, 0.0, cz_f + p1z * TILE_SIZE),
                perp,
            });
        }

        // diagonal neighbors — two lines each, skip if blocked by adjacent cardinal wall
        for (dx, dz) in [(-1i32,-1i32),(-1,1),(1,1),(1,-1)] {
            if map.get_tile(Vec2i::new(tx + dx, tz + dz)) != TileType::Wall { continue; }
            // Line A: x-facing (vertical in xz), extends in z-direction
            if map.get_tile(Vec2i::new(tx, tz + dz)) != TileType::Wall {
                let (ax, az0, az1) = (dx as f32 * 0.5, dz as f32 * 0.5, dz as f32 * 1.5);
                lines.push(WallLine {
                    p0: vec3f(cx_f + ax * TILE_SIZE, 0.0, cz_f + az0 * TILE_SIZE),
                    p1: vec3f(cx_f + ax * TILE_SIZE, 0.0, cz_f + az1 * TILE_SIZE),
                    perp: vec3f(-dx as f32, 0.0, 0.0),
                });
            }
            // Line B: z-facing (horizontal in xz), extends in x-direction
            if map.get_tile(Vec2i::new(tx + dx, tz)) != TileType::Wall {
                let (bz, bx0, bx1) = (dz as f32 * 0.5, dx as f32 * 0.5, dx as f32 * 1.5);
                lines.push(WallLine {
                    p0: vec3f(cx_f + bx0 * TILE_SIZE, 0.0, cz_f + bz * TILE_SIZE),
                    p1: vec3f(cx_f + bx1 * TILE_SIZE, 0.0, cz_f + bz * TILE_SIZE),
                    perp: vec3f(0.0, 0.0, -dz as f32),
                });
            }
        }

        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, 0, tz);
        let key = chunk_key(cx, cz);
        if let Some(chunk) = map.chunks.get_mut(&key) {
            chunk.walls[morton_encode(lx, ly, lz)] = lines;
        }
    }

    Ok(())
}

/// Moves agents along the flow field with per-cell peer-repulsion spreading
#[export_update_fn]
pub fn update_agents(
    mut agents: Query<(Entity, &mut AgentPos, &SpeedScale, &mut WanderAngle, &mut SepForce, &mut LocalDensity, &mut FacingRot), With<HumanAgent>>,
    mut map: ResMut<Map>,
    mut imdraw: ResMut<ImDrawRes>,
) -> Result<(), hotline_rs::Error> {
    
    //
    // clear grid
    //

    map.clear_all_agents();
    for (.., mut sf, mut ld, _) in agents.iter_mut() {
        sf.0 = Vec2f::zero();
        ld.0 = 0.0;
    }

    let total_agents = agents.iter().count();
    let sep_radius = (BASE_SEP_RADIUS - total_agents as f32 * SEP_RADIUS_DECAY).max(MIN_SEP_RADIUS);
    let sep_radius_sq = sep_radius * sep_radius;

    //
    // Pass 1: register each agent in the spatial grid (read-only)
    // Primary tile only — loose-grid registration removed to prevent double-counted separation forces
    //

    for (entity, pos, ..) in agents.iter() {
        let tile = world_to_tile(pos.0);
        map.register_agent(tile, entity);
    }

    //
    // Pass 2: per-cell separation + density accumulation
    // Snapshot occupied cells with precomputed (chunk_key, morton idx) to avoid
    // re-computing chunk_key/morton in the hot inner loop and to prevent borrow conflicts.
    // `occupied` is also reused by the post-movement correction pass.
    //

    struct OccupiedCell { tile: Vec2i, chunk_key: u64, idx: usize, cell_agents: Vec<Entity> }
    let occupied: Vec<OccupiedCell> = map.chunks.iter()
        .flat_map(|(&key, chunk)| {
            let (cx, cz) = chunk_key_unpack(key);
            let cs = CHUNK_SIZE as i32;
            // iterate lx/lz directly — no morton_decode needed
            (0..CHUNK_SIZE).flat_map(move |lz| (0..CHUNK_SIZE).filter_map(move |lx| {
                let idx = morton_encode(lx, 0, lz);
                if chunk.agents[idx].is_empty() { return None; }
                Some(OccupiedCell {
                    tile: Vec2i::new(cx * cs + lx as i32, cz * cs + lz as i32),
                    chunk_key: key,
                    idx,
                    cell_agents: chunk.agents[idx].clone(),
                })
            }))
        })
        .collect();

    // Snapshot positions once — no per-entity unsafe reads needed
    let positions: HashMap<Entity, Vec3f> = agents.iter()
        .map(|(e, p, ..)| (e, p.0))
        .collect();

    //
    // entity separation per tile, build 3x3, check per entity
    //

    for cell in &occupied {
        let cell_density = map.chunks[&cell.chunk_key].density[cell.idx];

        // Build 3×3 neighbourhood once per tile
        let mut nbr: Vec<(Entity, Vec2f)> = Vec::new();
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                for &e in map.get_agents_at(Vec2i::new(cell.tile.x + dx, cell.tile.y + dy)) {
                    if let Some(&p) = positions.get(&e) { nbr.push((e, vec2f(p.x, p.z))); }
                }
            }
        }

        // wall avoidance — look up once per cell, reuse for all entities in it
        let walls = map.get_walls(cell.tile);
        let flow_dir = map.get_flow(cell.tile);
        let tile_flow = normalize(Vec3f::new(flow_dir.x, 0.0, flow_dir.y));

        for &entity_a in &cell.cell_agents {
            let Some(&pos_a) = positions.get(&entity_a) else { continue };
            let pa = vec2f(pos_a.x, pos_a.z);
            let mut sep = Vec2f::zero();
            for &(entity_b, pb) in &nbr {
                if entity_b == entity_a { continue; }
                let diff = pa - pb;
                let dist_sq = dot(diff, diff);
                if dist_sq > 0.01 && dist_sq < sep_radius_sq {
                    let d = sqrt(dist_sq);
                    sep += diff / d * (1.0 - (d / sep_radius).min(1.0));
                }
            }

            let density_scale = 1.0 + (cell_density - 1.0).max(0.0) * 0.3;
            let sep_vel = if length(sep) > 0.001 {
                normalize(sep) * SEPARATION_STRENGTH * density_scale
            } else { Vec2f::zero() };

            // wall avoidance: same dot-product trigger as before, larger radius
            let mut wall_vel = Vec2f::zero();
            for wl in walls {
                let cp = closest_point_on_line_segment(pos_a, wl.p0, wl.p1);
                let d = dist(cp, pos_a);
                if d < WALL_AVOID_RADIUS {
                    let tangent = normalize(wl.p0 - wl.p1);
                    if abs(dot(tangent, tile_flow)) > 0.75 {
                        wall_vel = vec2f(wl.perp.x, wl.perp.z) * AGENT_SPEED;
                    }
                }
            }

            if let Ok((.., mut sf, mut ld, _)) = agents.get_mut(entity_a) {
                sf.0 = sep_vel + wall_vel;
                ld.0 += cell_density;
            }
        }
    }

    //
    // Pass 3: apply forces and move
    //

    for (entity, mut pos, speed, mut wander, sf, _ld, mut facing) in agents.iter_mut() {
        let tile = world_to_tile(pos.0);

        // sep + wall avoidance pre-computed in Pass 2, stored in sf.0
        let avoid_vel = vec3f(sf.0.x, 0.0, sf.0.y);

        // Wander: each agent drifts at a unique rate derived from entity index
        wander.0 += WANDER_DRIFT * (1.0 - (entity.index() % 5) as f32 * 0.1);
        let wander_vel = vec3f(wander.0.cos(), 0.0, wander.0.sin()) * AGENT_SPEED * speed.0 * WANDER_STRENGTH;

        // Flow — normalise so diagonal (1,1) and cardinal (1,0) produce equal speed
        let flow = map.get_flow(tile);
        let flow_norm = if length(flow) > 0.001 { normalize(flow) } else { flow };
        let flow_vel = vec3f(flow_norm.x, 0.0, flow_norm.y) * AGENT_SPEED * speed.0;

        let total_vel = flow_vel + avoid_vel + wander_vel;

        // Smooth turn toward velocity direction
        let vel_len = length(vec3f(total_vel.x, 0.0, total_vel.z));
        if vel_len > 0.001 {
            let yaw = f32::atan2(total_vel.x, total_vel.z);
            let desired = Quatf::from_euler_angles(0.0, yaw, 0.0);
            facing.0 = slerp(facing.0, desired, TURN_RATE);
        }

        pos.0 = pos.0 + total_vel;
    }

    // 
    // Correction pass: resolve agent-agent overlaps (PBD Jacobi, per occupied tile)
    // Reuses `occupied` from Pass 2. Snapshot post-movement positions for borrow-safe access.
    //
    
    let mut positions: HashMap<Entity, Vec3f> = agents.iter()
        .map(|(e, p, ..)| (e, p.0))
        .collect();

    let min_dist    = AGENT_RADIUS * 2.0;
    let min_dist_sq = min_dist * min_dist;

    for _ in 0..CORRECTION_ITERATIONS {
        let mut corrections: HashMap<Entity, Vec3f> = HashMap::new();

        for cell in &occupied {
            // Build 3×3 neighbourhood positions once per tile
            let mut nbr_pos: Vec<(Entity, Vec3f)> = Vec::new();
            for dz in -1i32..=1 {
                for dx in -1i32..=1 {
                    for &e in map.get_agents_at(Vec2i::new(cell.tile.x + dx, cell.tile.y + dz)) {
                        if let Some(&p) = positions.get(&e) { nbr_pos.push((e, p)); }
                    }
                }
            }

            for &entity_a in &cell.cell_agents {
                let Some(&pos_a) = positions.get(&entity_a) else { continue };
                let mut correction = Vec3f::zero();
                for &(entity_b, pos_b) in &nbr_pos {
                    if entity_b == entity_a { continue; }
                    let diff = (pos_a - pos_b) * vec3f(1.0, 0.0, 1.0);
                    let dist_sq = dot(diff, diff);
                    if dist_sq < min_dist_sq && dist_sq > 0.0001 {
                        let d = sqrt(dist_sq);
                        correction += (diff / d) * (min_dist - d) * 0.5;
                    }
                }
                if length(correction) > 0.0001 {
                    *corrections.entry(entity_a).or_insert(Vec3f::zero()) += correction;
                }
            }
        }

        for (e, corr) in corrections {
            if let Some(pos) = positions.get_mut(&e) { *pos += corr; }
        }
    }

    // TODO: this would be ideally structured again per occupied tile per entity.
    //
    // Pass 5: wall collision resolution — hard push-out after all corrections
    //

    for (entity, mut pos, ..) in agents.iter_mut() {
        let Some(p) = positions.get_mut(&entity) else { continue };
        let wall_tile = world_to_tile(*p);
        for wl in map.get_walls(wall_tile) {
            imdraw.add_line_3d(wl.p0, wl.p1, Vec4f::red());
            let mid = (wl.p0 + wl.p1) * 0.5;
            imdraw.add_line_3d(mid, mid + wl.perp * 2.0, Vec4f::cyan());

            let cp = closest_point_on_line_segment(*p, wl.p0, wl.p1);
            let d = dist(cp, *p);
            if d < AGENT_RADIUS {
                *p += normalize(*p - cp) * (AGENT_RADIUS - d) * vec3f(1.0, 0.0, 1.0);
            }
        }
        pos.0 = *p;
    }

    Ok(())
}

/// Syncs AgentPos and FacingRot to Position and Rotation for mesh rendering
#[export_update_fn]
pub fn update_agent_transforms(
    mut agents: Query<(&AgentPos, &FacingRot, &mut Position, &mut Rotation), With<HumanAgent>>,
) -> Result<(), hotline_rs::Error> {
    for (agent_pos, facing, mut position, mut rotation) in agents.iter_mut() {
        position.0 = vec3f(agent_pos.0.x, AGENT_BODY_HALF_HEIGHT, agent_pos.0.z);
        rotation.0 = facing.0;
    }
    Ok(())
}

#[export_update_fn]
pub fn debug_draw_agents(
    agents: Query<(&AgentPos, &FacingRot), With<HumanAgent>>,
    mut imdraw: ResMut<ImDrawRes>,
) -> Result<(), hotline_rs::Error> {
    const Y_OFFSET: f32 = 1.0;
    let agent_col = Vec4f::new(1.0, 0.0, 0.0, 1.0);
    for (pos, facing) in agents.iter() {
        let p = pos.0;
        let base = vec3f(p.x, Y_OFFSET, p.z);
        imdraw.add_circle_3d_xz(base, AGENT_RADIUS, agent_col);
        let fwd = facing.0 * vec3f(0.0, 0.0, 1.0);
        imdraw.add_line_3d(base, base + fwd * AGENT_RADIUS * 1.5, Vec4f::yellow());
    }
    Ok(())
}

/// Sets up the ratrace game world
#[export_update_fn]
pub fn setup_ratrace2(
    mut session_info: ResMut<SessionInfo>,
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    // enable the grid
    session_info.debug_draw_flags |= DebugDrawFlags::GRID;

    // create resources — load map from disk if it exists, otherwise start empty
    let map = Map::load(MAP_SAVE_PATH).unwrap_or_else(|_| Map::new());
    commands.insert_resource(map);
    commands.insert_resource(EditorState { selected: PlaceMode::Tile(0), left_was_down: false });

    // create shared cube mesh for agent bodies
    let cube = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.insert_resource(AgentMesh(cube));

    Ok(())
}

#[export_update_fn]
pub fn update_tile_editor_ui(
    mut imgui: ResMut<ImGuiRes>,
    mut map: ResMut<Map>,
    mut editor: ResMut<EditorState>,
) -> Result<(), hotline_rs::Error> {
    imgui.set_global_context();

    if imgui.begin_main_menu_bar() {
        if imgui.button("clear") {
            *map = Map::new();
        }
        let selected_name = match editor.selected {
            PlaceMode::Tile(i) => PLACE_NAMES[i].to_string(),
            PlaceMode::Agent => "Agent".to_string(),
        };
        let place_names: Vec<String> = PLACE_NAMES.iter().map(|s| s.to_string()).collect();
        imgui.set_next_item_width(150.0);
        let (_, new_selected) = imgui.combo_list("Place", &place_names, &selected_name);
        editor.selected = if new_selected == "Agent" {
            PlaceMode::Agent
        } else {
            let idx = TILE_NAMES.iter().position(|&n| n == new_selected.as_str()).unwrap_or(0);
            PlaceMode::Tile(idx)
        };
        imgui.end_main_menu_bar();
    }
    Ok(())
}

/// Main update - handles mouse input and draws tilemap editor overlay
#[export_update_fn]
pub fn update_tile_editor(
    app: Res<AppRes>,
    viewport: Res<ViewportInfo>,
    pmfx: Res<PmfxRes>,
    mut imdraw: ResMut<ImDrawRes>,
    mut map: ResMut<Map>,
    mut editor: ResMut<EditorState>,
    cube_mesh: Res<AgentMesh>,
    mut commands: Commands,
) -> Result<(), hotline_rs::Error> {

    let (_, enable_mouse) = app.get_input_enabled();

    // get camera for unprojection
    let camera = pmfx.get_camera_constants("main_camera");
    if !camera.is_ok() {
        return Ok(()); // we might need to skip frames
    }
    let camera = camera?;

    let inv_vp = camera.view_projection_matrix.inverse();

    // mouse screen pos relative to the dock viewport (both in screen coords)
    let screen_pos = app.get_mouse_pos();
    let local_x = screen_pos.x as f32 - viewport.pos.0;
    let local_y = screen_pos.y as f32 - viewport.pos.1;

    let ndc_x = (local_x / viewport.size.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - (local_y / viewport.size.1) * 2.0;

    // unproject near/far to get a ray
    let near_ndc = Vec4f::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_ndc = Vec4f::new(ndc_x, ndc_y, 1.0, 1.0);

    let near_world = inv_vp * near_ndc;
    let far_world = inv_vp * far_ndc;

    let near_pos = near_world.xyz() / near_world.w;
    let far_pos = far_world.xyz() / far_world.w;

    const Y_OFFSET: f32 = 1.0;

    let tile_cols : &[Vec4f] = &[
        Vec4f::black(),
        Vec4f::white(),
        Vec4f::red(),
        Vec4f::blue(),
        Vec4f::yellow(),
        Vec4f::green(),
    ];

    if enable_mouse {
        // intersect ray with y=0 ground plane
        let ray_dir = far_pos - near_pos;
        if ray_dir.y.abs() > 0.0001 {
            let t = -near_pos.y / ray_dir.y;
            if t >= 0.0 {
                let hit = near_pos + ray_dir * t;

                // snap to grid tile
                let tile_x = (hit.x / TILE_SIZE).floor() as i32;
                let tile_z = (hit.z / TILE_SIZE).floor() as i32;

                // clamp to grid bounds
                let tile_x = tile_x.clamp(-GRID_SIZE / 2, GRID_SIZE / 2 - 1);
                let tile_z = tile_z.clamp(-GRID_SIZE / 2, GRID_SIZE / 2 - 1);

                let min_x = tile_x as f32 * TILE_SIZE;
                let min_z = tile_z as f32 * TILE_SIZE;
                let max_x = min_x + TILE_SIZE;
                let max_z = min_z + TILE_SIZE;

                // draw highlighted tile cursor on ground plane
                let y = Y_OFFSET;
                let col = Vec4f::new(0.0, 1.0, 1.0, 1.0);
                imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, min_z), col);
                imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(max_x, y, max_z), col);
                imdraw.add_line_3d(Vec3f::new(max_x, y, max_z), Vec3f::new(min_x, y, max_z), col);
                imdraw.add_line_3d(Vec3f::new(min_x, y, max_z), Vec3f::new(min_x, y, min_z), col);

                // place / erase tiles
                if !app.is_sys_key_down(os::SysKey::Alt) {
                    let buttons = app.get_mouse_buttons();
                    let left_down = buttons[os::MouseButton::Left as usize];
                    let left_pressed = left_down && !editor.left_was_down;

                    if !app.is_sys_key_down(os::SysKey::Ctrl) {
                        if left_down {
                            match editor.selected {
                                PlaceMode::Tile(i) => {
                                    map.set_tile(Vec2i::new(tile_x, tile_z), TILE_TYPES[i]);
                                }
                                PlaceMode::Agent => {
                                    if left_pressed && map.get_tile(Vec2i::new(tile_x, tile_z)) == TileType::Empty {
                                        commands.spawn((
                                            HumanAgent,
                                            AgentPos(Vec3f::new(hit.x, 0.0, hit.z)),
                                            SpeedScale(0.8 + pos_hash(hit.x, hit.z) * 0.4),
                                            WanderAngle(pos_hash(hit.z, hit.x) * std::f32::consts::TAU),
                                            SepForce(Vec2f::zero()),
                                            LocalDensity(0.0),
                                            FacingRot(Quatf::identity()),
                                            MeshComponent(cube_mesh.0.clone()),
                                            Position(vec3f(hit.x, AGENT_BODY_HALF_HEIGHT, hit.z)),
                                            Rotation(Quatf::identity()),
                                            Scale(vec3f(AGENT_BODY_HALF_WIDTH, AGENT_BODY_HALF_HEIGHT, AGENT_BODY_HALF_WIDTH)),
                                            WorldMatrix(Mat34f::identity()),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    else {
                        if left_down {
                            map.set_tile(Vec2i::new(tile_x, tile_z), TileType::Empty);
                        }
                    }
                    editor.left_was_down = left_down;
                }
            }
        }
    }

    // save / load with Ctrl+S / Ctrl+L
    if app.is_sys_key_down(os::SysKey::Ctrl) {
        let keys = app.get_keys_pressed();
        if keys['S' as usize] {
            let _ = map.save();
        }
        if keys['L' as usize] {
            if let Ok(mut loaded) = Map::load(MAP_SAVE_PATH) {
                loaded.dirty = true;
                *map = loaded;
            }
        }
    }

    // draw all placed tiles and flow arrows
    let y = Y_OFFSET;
    for (key, chunk) in &map.chunks {
        let (cx, cz) = chunk_key_unpack(*key);
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let idx = morton_encode(lx, 0, lz);

                let tx = cx * CHUNK_SIZE as i32 + lx as i32;
                let tz = cz * CHUNK_SIZE as i32 + lz as i32;

                let min_x = tx as f32 * TILE_SIZE;
                let min_z = tz as f32 * TILE_SIZE;
                let max_x = min_x + TILE_SIZE;
                let max_z = min_z + TILE_SIZE;

                let mid_x = min_x + (max_x - min_x) * 0.5;
                let mid_z = min_z + (max_z - min_z) * 0.5;

                let col = tile_cols[chunk.tiles[idx] as usize];

                if chunk.tiles[idx] != TileType::Empty {
                    imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, min_z), col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(max_x, y, max_z), col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, max_z), Vec3f::new(min_x, y, max_z), col);
                    imdraw.add_line_3d(Vec3f::new(min_x, y, max_z), Vec3f::new(min_x, y, min_z), col);
                    imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, max_z), col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(min_x, y, max_z), col);
                }
                else {
                    let flow_dir = chunk.flow[idx]; // read directly via precomputed idx
                    let flow_dir3 = Vec3f::new(flow_dir.x, 0.0, flow_dir.y);
                    let flow_start = Vec3f::new(mid_x, y, mid_z);
                    let flow_col = Vec4f::from((flow_dir3.xy() * 0.5 + 0.8, 0.0, 1.0));

                    let flow_end = flow_start + flow_dir3;
                    imdraw.add_line_3d(flow_start, flow_end, flow_col);

                    let arrow_size = 0.2;
                    let perp = maths_rs::perp(flow_dir3.xz()) * arrow_size;
                    let perp = Vec3f::new(perp.x, 0.0, perp.y);
                    let tip = flow_end - flow_dir3 * arrow_size;
                    imdraw.add_line_3d(flow_end, tip + perp, flow_col);
                    imdraw.add_line_3d(flow_end, tip - perp, flow_col);
                }
            }
        }
    }

    Ok(())
}
