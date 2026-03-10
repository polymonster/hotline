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
            "update_agents"
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
}

impl MapChunk {
    fn new() -> Self {
        Self {
            tiles:    [TileType::Empty; ARRAY_SIZE],
            flow:     [Vec2f::new(0.0, 1.0); ARRAY_SIZE],
            density:  [0.0; ARRAY_SIZE],
            pressure: [0.0; ARRAY_SIZE],
            agents:   std::array::from_fn(|_| Vec::new()),
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
const AGENT_RADIUS2: f32 = AGENT_RADIUS * AGENT_RADIUS;
const AGENT_SEGMENTS: usize = 8;

/// Separation radius at low density (shrinks as crowd grows)
const BASE_SEP_RADIUS: f32     = 24.0;
const MIN_SEP_RADIUS: f32      =  8.0;
const SEP_RADIUS_DECAY: f32    =  0.25;   // world units of radius lost per agent
const SEPARATION_STRENGTH: f32 =  0.02;   // world units per frame at full push (flow dominates; sep spikes when critically close)
const WANDER_DRIFT: f32        =  0.012;  // radians per frame
const WANDER_STRENGTH: f32     =  0.08;   // fraction of agent speed

const TILE_TYPES: &[TileType] = &[
    TileType::Wall,
    TileType::Barrier,
    TileType::Escalator,
    TileType::Platform,
    TileType::TrainTrack
];

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

    // --- pass 1: reset flow/pressure and seed the heap with Platform tiles ---
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

    // --- pass 2: Dijkstra flood fill ---
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

    // --- pass 3: compute flow direction (gradient toward lowest-cost neighbour) ---
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

    Ok(())
}

/// Moves agents along the flow field with per-cell peer-repulsion spreading
#[export_update_fn]
pub fn update_agents(
    mut agents: Query<(Entity, &mut AgentPos, &SpeedScale, &mut WanderAngle, &mut SepForce, &mut LocalDensity), With<HumanAgent>>,
    mut map: ResMut<Map>,
    mut imdraw: ResMut<ImDrawRes>,
    app:      Res<AppRes>,
    viewport: Res<ViewportInfo>,
    pmfx:     Res<PmfxRes>,
) -> Result<(), hotline_rs::Error> {
    const Y_OFFSET: f32 = 1.0;
    let agent_col = Vec4f::new(1.0, 0.5, 0.0, 1.0);

    // --- Pass 0: clear spatial grid and reset per-frame agent state ---
    map.clear_all_agents();
    for (_, _, _, _, mut sf, mut ld) in agents.iter_mut() {
        sf.0 = Vec2f::zero();
        ld.0 = 0.0;
    }

    let total_agents = agents.iter().count();
    let sep_radius = (BASE_SEP_RADIUS - total_agents as f32 * SEP_RADIUS_DECAY).max(MIN_SEP_RADIUS);
    let sep_radius_sq = sep_radius * sep_radius;

    // --- Pass 1: register each agent in the spatial grid (read-only) ---
    // Primary tile only — loose-grid registration removed to prevent double-counted separation forces
    for (entity, pos, _, _, _, _) in agents.iter() {
        let tile = world_to_tile(pos.0);
        map.register_agent(tile, entity);
    }

    // --- Pass 2: per-cell separation + density accumulation ---
    // Snapshot occupied cells with precomputed (chunk_key, morton idx) to avoid
    // re-computing chunk_key/morton in the hot inner loop and to prevent borrow conflicts.
    struct OccupiedCell {
        tile:       Vec2i,
        chunk_key:  u64,
        idx:        usize,
        cell_agents: Vec<Entity>,
    }
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

    for cell in &occupied {
        // density fetched directly via precomputed idx — no repeated chunk_key/morton math
        let cell_density = map.chunks[&cell.chunk_key].density[cell.idx];


        for &entity_a in &cell.cell_agents {
            // Read entity_a's position — Vec3f is Copy, borrow ends immediately
            let pos_a: Vec3f = unsafe { agents.get_unchecked(entity_a) }
                .map(|(_, p, _, _, _, _)| p.0)
                .unwrap_or(Vec3f::zero());

            // Accumulate separation from 3×3 neighbourhood
            let mut sep_x = 0.0f32;
            let mut sep_z = 0.0f32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nbr = Vec2i::new(cell.tile.x + dx, cell.tile.y + dy);
                    // snapshot neighbour list so we can read map without holding a borrow
                    let nbr_agents: Vec<Entity> = map.get_agents_at(nbr).to_vec();
                    for entity_b in nbr_agents {
                        if entity_b == entity_a { continue; }
                        // SAFETY: entity_b != entity_a — no aliased mutable access
                        let pos_b: Vec3f = unsafe { agents.get_unchecked(entity_b) }
                            .map(|(_, p, _, _, _, _)| p.0)
                            .unwrap_or(pos_a);
                        let diff_x = pos_a.x - pos_b.x;
                        let diff_z = pos_a.z - pos_b.z;
                        let dist_sq = diff_x * diff_x + diff_z * diff_z;
                        if dist_sq > 0.01 && dist_sq < sep_radius_sq {
                            let dist = dist_sq.sqrt();
                            // falloff: agents at the edge of sep_radius contribute ~0; near-overlap contributes ~1
                            let falloff = 1.0 - (dist / sep_radius).min(1.0);
                            sep_x += diff_x / dist * falloff;
                            sep_z += diff_z / dist * falloff;
                        }
                    }
                }
            }

            // Write accumulated force + density back to entity_a — all reads are complete
            // SAFETY: entity_a is uniquely targeted here, no aliased mutable borrow
            if let Ok((_, _, _, _, mut sf, mut ld)) = unsafe { agents.get_unchecked(entity_a) } {
                sf.0 = Vec2f::new(sf.0.x + sep_x, sf.0.y + sep_z);
                ld.0 += cell_density;
            }
        }
    }

    // Compute mouse world-hit once for agent hover visualisation
    let mouse_world_hit: Option<Vec3f> = pmfx.get_camera_constants("main_camera").ok()
        .and_then(|cam| {
            let inv_vp = cam.view_projection_matrix.inverse();
            let sp = app.get_mouse_pos();
            let lx = sp.x as f32 - viewport.pos.0;
            let ly = sp.y as f32 - viewport.pos.1;
            let nx = (lx / viewport.size.0) * 2.0 - 1.0;
            let ny = 1.0 - (ly / viewport.size.1) * 2.0;
            let near = inv_vp * Vec4f::new(nx, ny, 0.0, 1.0);
            let far  = inv_vp * Vec4f::new(nx, ny, 1.0, 1.0);
            let np = near.xyz() / near.w;
            let fp = far.xyz()  / far.w;
            let dir = fp - np;
            if dir.y.abs() > 0.0001 {
                let t = -np.y / dir.y;
                if t >= 0.0 { Some(np + dir * t) } else { None }
            } else { None }
        });

    // --- Pass 3: apply forces, move, and draw ---
    for (entity, mut pos, speed, mut wander, sf, ld) in agents.iter_mut() {
        let tile = world_to_tile(pos.0);

        // Separation — density already accumulated above, no extra map lookup needed
        let sep_len = (sf.0.x * sf.0.x + sf.0.y * sf.0.y).sqrt();
        let density_scale = 1.0 + (ld.0 - 1.0).max(0.0) * 0.3;
        let sep_vel = if sep_len > 0.001 {
            Vec3f::new(sf.0.x / sep_len, 0.0, sf.0.y / sep_len)
                * SEPARATION_STRENGTH * density_scale
        } else {
            Vec3f::zero()
        };

        // Wander: each agent drifts at a unique rate derived from entity index
        wander.0 += WANDER_DRIFT * (1.0 - (entity.index() % 5) as f32 * 0.1);
        let wander_vel = Vec3f::new(wander.0.cos(), 0.0, wander.0.sin())
            * AGENT_SPEED * speed.0 * WANDER_STRENGTH;

        // Flow — normalise so diagonal (1,1) and cardinal (1,0) produce equal speed
        let flow = map.get_flow(tile);
        let flow_len = (flow.x * flow.x + flow.y * flow.y).sqrt();
        let flow_norm = if flow_len > 0.001 { flow / flow_len } else { flow };
        let flow_vel = Vec3f::new(flow_norm.x, 0.0, flow_norm.y) * AGENT_SPEED * speed.0;

        // Compute tile range covering the agent's bounding circle — position-continuous,
        // avoids snap artifacts when the centre crosses a tile boundary.
        let wall_tile_range = |px: f32, pz: f32| -> (i32, i32, i32, i32) {
            (
                ((px - AGENT_RADIUS) / TILE_SIZE).floor() as i32,
                ((px + AGENT_RADIUS) / TILE_SIZE).floor() as i32,
                ((pz - AGENT_RADIUS) / TILE_SIZE).floor() as i32,
                ((pz + AGENT_RADIUS) / TILE_SIZE).floor() as i32,
            )
        };
        let (tx0, tx1, tz0, tz1) = wall_tile_range(pos.0.x, pos.0.z);

        let total_vel = flow_vel + sep_vel + wander_vel;
        
        //let target_speed = (total_vel.x * total_vel.x + total_vel.z * total_vel.z).sqrt();
        //let mut slide_x = total_vel.x;
        //let mut slide_z = total_vel.z;

        /*
        // Project velocity to remove components pointing into nearby walls.
        // Accumulate the combined away-from-wall normal for corner escape.
        let mut wall_away_x = 0.0f32;
        let mut wall_away_z = 0.0f32;
        let (tx0, tx1, tz0, tz1) = wall_tile_range(pos.0.x, pos.0.z);
        for tz in tz0..=tz1 {
            for tx in tx0..=tx1 {
                let nbr = Vec2i::new(tx, tz);
                let t = map.get_tile(nbr);
                if t != TileType::Wall && t != TileType::Barrier { continue; }
                let min_x = nbr.x as f32 * TILE_SIZE;
                let min_z = nbr.y as f32 * TILE_SIZE;
                let cx = pos.0.x.clamp(min_x, min_x + TILE_SIZE);
                let cz = pos.0.z.clamp(min_z, min_z + TILE_SIZE);
                let nx = pos.0.x - cx;
                let nz = pos.0.z - cz;
                let dist_sq = nx * nx + nz * nz;
                if dist_sq > 0.0001 && dist_sq < AGENT_RADIUS * AGENT_RADIUS {
                    let dist = dist_sq.sqrt();
                    let nnx = nx / dist;
                    let nnz = nz / dist;
                    wall_away_x += nnx;
                    wall_away_z += nnz;
                    let dot = slide_x * nnx + slide_z * nnz;
                    if dot < 0.0 {
                        slide_x -= dot * nnx;
                        slide_z -= dot * nnz;
                    }
                }
            }
        }
        // Speed preservation: when projection significantly reduces speed (flow from current
        // tile conflicts with a wall), look at cardinal neighbour tiles' flows and use
        // whichever one projects to the most usable speed. This prevents the oscillation
        // where the current tile's flow is fully blocked (e.g. (-1,0) against a -X wall)
        // while an adjacent tile has a valid direction (e.g. (0,-1)). Wall-away is only
        // used as a last resort when all neighbours are also stuck.
        let away_len = (wall_away_x * wall_away_x + wall_away_z * wall_away_z).sqrt();
        if target_speed > 0.001 && slide_x * slide_x + slide_z * slide_z < (target_speed * 0.5).powi(2) && away_len > 0.001 {
            let wax = wall_away_x / away_len;
            let waz = wall_away_z / away_len;
            let mut best_sq = slide_x * slide_x + slide_z * slide_z;
            let mut best_sx = slide_x;
            let mut best_sz = slide_z;
            for (ddx, ddz) in [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
                let nt = Vec2i::new(tile.x + ddx, tile.y + ddz);
                if map.get_tile(nt) == TileType::Wall || map.get_tile(nt) == TileType::Barrier { continue; }
                let af = map.get_flow(nt);
                let al = (af.x * af.x + af.y * af.y).sqrt();
                if al < 0.001 { continue; }
                let mut avx = af.x / al * target_speed;
                let mut avz = af.y / al * target_speed;
                // project against combined wall normal
                let dot = avx * wax + avz * waz;
                if dot < 0.0 { avx -= dot * wax; avz -= dot * waz; }
                let asq = avx * avx + avz * avz;
                if asq > best_sq { best_sq = asq; best_sx = avx; best_sz = avz; }
            }
            // Last resort: wall-away if all flows are blocked
            if best_sq < (target_speed * 0.1).powi(2) {
                best_sx = wax * target_speed * 0.5;
                best_sz = waz * target_speed * 0.5;
            }
            slide_x = best_sx;
            slide_z = best_sz;
        }
        */

        /*
        // Renormalise to target_speed — no energy gain or loss from wall contact
        let final_len = (slide_x * slide_x + slide_z * slide_z).sqrt();
        if final_len > 0.001 && target_speed > 0.001 {
            slide_x = slide_x / final_len * target_speed;
            slide_z = slide_z / final_len * target_speed;
        }

        // Move; axis-split as safety against tunnelling

        */

        let can_move = |p: Vec3f| {
            let t = world_to_tile(p);
            let d = map.get_tile(t);
            d == TileType::Empty || d == TileType::Platform
        };
        let new_pos = pos.0 + total_vel;
        pos.0 = if can_move(new_pos) {
            new_pos
        } else if can_move(Vec3f::new(new_pos.x, pos.0.y, pos.0.z)) {
            Vec3f::new(new_pos.x, pos.0.y, pos.0.z)
        } else if can_move(Vec3f::new(pos.0.x, pos.0.y, new_pos.z)) {
            Vec3f::new(pos.0.x, pos.0.y, new_pos.z)
        } else {
            pos.0
        };

        let v_look_nbrs = [
            vec3f(-1.0, 0.0,  0.0),
            vec3f( 0.0, 0.0,  1.0),
            vec3f( 1.0, 0.0,  0.0),
            vec3f( 0.0, 0.0, -1.0),
        ];

        let wall = [
            (vec3f(-0.5, 0.0, -0.5) * TILE_SIZE, vec3f(-0.5, 0.0, 0.5) * TILE_SIZE),
            (vec3f(-0.5, 0.0,  0.5) * TILE_SIZE, vec3f( 0.5, 0.0, 0.5) * TILE_SIZE),
            (vec3f( 0.5, 0.0, -0.5) * TILE_SIZE, vec3f( 0.5, 0.0, 0.5) * TILE_SIZE),
            (vec3f(-0.5, 0.0,  0.5) * TILE_SIZE, vec3f( 0.5, 0.0, 0.5) * TILE_SIZE),
        ];

        let tile_mid = (floor(pos.0 / TILE_SIZE) * TILE_SIZE) + vec3f(0.5, 0.1, 0.5) * TILE_SIZE;
        imdraw.add_point_3d(tile_mid, 1.0, Vec4f::white());

        for i in 0..v_look_nbrs.len() {
            let look_nbr = tile_mid + v_look_nbrs[i] * TILE_SIZE;
            let nbr = floor(look_nbr / TILE_SIZE);
            let t = map.get_tile(Vec2i::from(nbr.xz()));
            if t == TileType::Wall {
                let wall_line = (tile_mid + wall[i].0, tile_mid + wall[i].1);

                let cp = closest_point_on_line_segment(pos.0, wall_line.0, wall_line.1);

                imdraw.add_line_3d(wall_line.0, wall_line.1, Vec4f::red());
                imdraw.add_point_3d(cp, 1.0, Vec4f::magenta());

                let d = dist(cp, pos.0);
                if d <= AGENT_RADIUS {
                    let offset = normalize(pos.0 - cp) * (AGENT_RADIUS - d) * vec3f(1.0, 0.0, 1.0);
                    imdraw.add_line_3d(cp, cp + offset, Vec4f::yellow());
                    pos.0 += offset;
                }
            }
        }


        /*
        // Circle-vs-AABB push-out — same bounding-circle tile range, position-continuous
        let (tx0, tx1, tz0, tz1) = wall_tile_range(pos.0.x, pos.0.z);
        for tz in tz0..=tz1 {
            for tx in tx0..=tx1 {
                let nbr = Vec2i::new(tx, tz);
                let t = map.get_tile(nbr);
                if t != TileType::Wall && t != TileType::Barrier { continue; }
                let min_x = nbr.x as f32 * TILE_SIZE;
                let min_z = nbr.y as f32 * TILE_SIZE;
                let cx = pos.0.x.clamp(min_x, min_x + TILE_SIZE);
                let cz = pos.0.z.clamp(min_z, min_z + TILE_SIZE);
                let diff_x = pos.0.x - cx;
                let diff_z = pos.0.z - cz;
                let dist_sq = diff_x * diff_x + diff_z * diff_z;
                if dist_sq > 0.0001 && dist_sq < AGENT_RADIUS * AGENT_RADIUS {
                    let dist = dist_sq.sqrt();
                    let push = (AGENT_RADIUS - dist) / dist;
                    pos.0.x += diff_x * push;
                    pos.0.z += diff_z * push;
                }
            }
        }
        */

        // Draw agent as an octagon
        let p = pos.0;
        for i in 0..AGENT_SEGMENTS {
            let a0 = (i as f32 / AGENT_SEGMENTS as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / AGENT_SEGMENTS as f32) * std::f32::consts::TAU;
            let p0 = Vec3f::new(p.x + a0.cos() * AGENT_RADIUS, Y_OFFSET, p.z + a0.sin() * AGENT_RADIUS);
            let p1 = Vec3f::new(p.x + a1.cos() * AGENT_RADIUS, Y_OFFSET, p.z + a1.sin() * AGENT_RADIUS);
            imdraw.add_line_3d(p0, p1, agent_col);
        }

        // Mouse-hover force visualisation
        if let Some(hit) = mouse_world_hit {
            let dx = pos.0.x - hit.x;
            let dz = pos.0.z - hit.z;
            if dx * dx + dz * dz < AGENT_RADIUS * AGENT_RADIUS * 4.0 {
                let base = Vec3f::new(pos.0.x, Y_OFFSET + 1.0, pos.0.z);
                // separation force — yellow
                let sep_end = base + Vec3f::new(sf.0.x, 0.0, sf.0.y) * 2.0;
                imdraw.add_line_3d(base, sep_end, Vec4f::new(1.0, 1.0, 0.0, 1.0));
                // flow direction — cyan
                let flow_end = base + Vec3f::new(flow_norm.x, 0.0, flow_norm.y) * 5.0;
                imdraw.add_line_3d(base, flow_end, Vec4f::new(0.0, 1.0, 1.0, 1.0));
            }
        }
    }

    Ok(())
}

/// Sets up the ratrace game world
#[export_update_fn]
pub fn setup_ratrace2(
    mut session_info: ResMut<SessionInfo>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    // enable the grid
    session_info.debug_draw_flags |= DebugDrawFlags::GRID;

    // create resources — load map from disk if it exists, otherwise start empty
    let map = Map::load(MAP_SAVE_PATH).unwrap_or_else(|_| Map::new());
    commands.insert_resource(map);
    commands.insert_resource(EditorState { selected: PlaceMode::Tile(0), left_was_down: false });

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
                    // Density heat-map overlay: green → red as crowd grows
                    let density = chunk.density[idx];
                    if density > 0.0 {
                        let heat = (density / 8.0).min(1.0);
                        let dcol = Vec4f::new(heat, 1.0 - heat, 0.0, 1.0);
                        let dy = y + 0.5;
                        for i in 1..=3 {
                            let frac = i as f32 / 4.0;
                            let z = min_z + (max_z - min_z) * frac;
                            imdraw.add_line_3d(Vec3f::new(min_x, dy, z), Vec3f::new(max_x, dy, z), dcol);
                        }
                    }

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
