//
/// Rat Race 2 - Tilemap Editor & Game
///

// TODO: keep eye on the random intermittent crash wne adding agents
// TODO: lateral movement is happening, but they stop bunching together so much and dont form a tighter queue.

use crate::prelude::*;
use maths_rs::Vec3i;
use rayon::prelude::*;

use std::collections::HashMap;

const CHUNK_SIZE: usize = 8; // must be power-of-2 for morton encoding to produce dense indices
const TILE_SIZE: f32 = 10.0;
const TILE_OCCUPIED: u8 = 1;
const GRID_SIZE: i32 = 2000;
const ARRAY_SIZE: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

const AGENT_SPEED: f32 = 0.1;
const AGENT_RADIUS: f32 = 1.0;
const AGENT_BODY_HALF_WIDTH: f32 = AGENT_RADIUS / 2.0;  // half-extent x/z (body 3 units wide)
const AGENT_BODY_HALF_HEIGHT: f32 = AGENT_RADIUS * 1.5; // half-extent y (body 9 units tall, ~1:3 ratio)
const CORRECTION_ITERATIONS: usize = 4;
const TURN_RATE: f32 = 0.1; // fraction of angular gap closed per frame (exponential smoothing)

/// Separation radius at low density (shrinks as crowd grows)
const BASE_SEP_RADIUS: f32 = 24.0;
const MIN_SEP_RADIUS: f32 = 8.0;
const SEP_RADIUS_DECAY: f32 = 0.25;   // world units of radius lost per agent
const SEPARATION_STRENGTH: f32 = 0.02;   // world units per frame at full push (flow dominates; sep spikes when critically close)
const QUEUE_SPREAD_STRENGTH: f32 = 0.04; // lateral drift toward less-dense side when queuing
const WANDER_DRIFT: f32 = 0.012;  // radians per frame
const WANDER_STRENGTH: f32 = 0.08;   // fraction of agent speed
const WALL_AVOID_RADIUS: f32 = AGENT_RADIUS * 2.5; // soft avoidance lookahead, larger than collision radius

const MAP_SAVE_PATH: &str = "ratrace2_map.bin";

/// Init function for ratrace demo
#[no_mangle]
pub fn ratrace2(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_ratrace2"
        ],
        update: systems![
            "update_flow_field",
            "update_tile_editor",
            "update_agents",
            "update_agent_transforms",
            "debug_draw_agents",
            "update_tile_editor_ui",
            "update_perf_ui"
        ],
        render_graph: "mesh_wireframe_overlay"
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum TileType {
    Empty,      // 0 — unset / outside region; treated as wall
    Wall,       // 1 — explicit interior wall
    Barrier,    // 2
    Escalator,  // 3
    Platform,   // 4 — flow sink
    TrainTrack, // 5
    Floor,      // 6 — explicitly placed walkable surface
}

fn is_walkable(t: TileType) -> bool {
    matches!(t, TileType::Floor | TileType::Platform | TileType::Escalator | TileType::TrainTrack)
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
    /// general per-tile ID — used as goal index for Platform tiles, available for all tile types
    index: [u8; ARRAY_SIZE],
    /// 2D flow fields per goal: flow[goal_idx][morton] = direction toward that goal
    flow: Vec<[Vec2f; ARRAY_SIZE]>,
    /// agent count per cell — reset each frame alongside agents
    density: [f32; ARRAY_SIZE],
    /// pressure curve based on density, controls flow
    pressure: [f32; ARRAY_SIZE],
    /// per-cell agent entity IDs (SoA); cleared and rebuilt each frame
    agents: [Vec<Entity>; ARRAY_SIZE],
    /// baked wall lines for collision, updated when map.dirty (flat array, morton order)
    walls:      Vec<WallLine>,
    wall_start: [u32; ARRAY_SIZE],
    wall_count: [u8;  ARRAY_SIZE],
    /// per-tile flags (bit 0 = TILE_OCCUPIED); set by register_agent, cleared by clear_agents
    flags: [u8; ARRAY_SIZE],
}

impl MapChunk {
    fn new() -> Self {
        Self {
            tiles:      [TileType::Empty; ARRAY_SIZE],
            index:      [0u8; ARRAY_SIZE],
            flow:       vec![[Vec2f::zero(); ARRAY_SIZE]],
            density:    [0.0; ARRAY_SIZE],
            pressure:   [0.0; ARRAY_SIZE],
            agents:     std::array::from_fn(|_| Vec::new()),
            walls:      Vec::new(),
            wall_start: [0; ARRAY_SIZE],
            wall_count: [0; ARRAY_SIZE],
            flags:      [0u8; ARRAY_SIZE],
        }
    }

    fn clear_agents(&mut self) {
        for (idx, f) in self.flags.iter_mut().enumerate() {
            if *f & TILE_OCCUPIED != 0 {
                self.agents[idx].clear();
                self.density[idx] = 0.0;
                *f = 0;
            }
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

/// Convert world-space position to tile coordinates
fn world_to_tile(pos: Vec3f) -> Vec3i {
    Vec3i::new(
        (pos.x / TILE_SIZE).floor() as i32,
        (pos.y / TILE_SIZE).floor() as i32,
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

#[derive(PartialEq)]
pub enum EditorMode { Tile, Agent }

const TILE_TYPES: &[TileType] = &[
    TileType::Floor,
    TileType::Wall,
    TileType::Barrier,
    TileType::Escalator,
    TileType::Platform,
    TileType::TrainTrack,
];

/// Shared cube mesh resource for agent body rendering
#[derive(Resource)]
pub(crate) struct AgentMesh(pmfx::Mesh<gfx_platform::Device>);

/// Flat per-entity SoA buffers, all indexed by entity.index()
#[derive(Resource, Default)]
pub(crate) struct AgentSoa {
    pub pos:     Vec<Vec3f>,
    pub sep:     Vec<Vec2f>,
    pub density: Vec<f32>,
    pub wander:  Vec<f32>,
    pub facing:  Vec<Quatf>,
    pub speed:   Vec<f32>,
    pub goal:    Vec<u8>,
    pub flags:   Vec<u8>,
    /// per-frame pass timers: (name, microseconds), refreshed every frame
    pub timers:  Vec<(&'static str, u64)>,
}

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

/// Which platform goal this agent is navigating toward (matches chunk.index on Platform tiles)
#[derive(Component)]
pub(crate) struct Goal(u8);

/// Agent state flags bitmask (u8, expandable to u16/u32)
pub const FLAG_WAITING: u8 = 1 << 0;  // agent has reached goal and is holding position
#[derive(Component)]
pub(crate) struct Flags(u8);

#[derive(Resource)]
pub struct EditorState {
    mode: EditorMode,
    tile_idx: usize,
    agent_grid: i32,
    index: u8,
    agent_goal: u8,
    viz_goal: u8,
    left_was_down: bool,
    save_filepath: String
}

#[derive(Resource)]
pub struct Map {
    chunks: HashMap<(i32, i32, i32), MapChunk>,
    dirty: bool,
}

impl Map {
    fn new() -> Self {
        Self { chunks: HashMap::new(), dirty: true }
    }

    fn set_tile(&mut self, tile: Vec3i, t: TileType) {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        let chunk = self.chunks.entry((cx, cy, cz)).or_insert_with(MapChunk::new);
        chunk.tiles[morton_encode(lx, ly, lz)] = t;
        self.dirty = true;
    }

    fn set_tile_index(&mut self, tile: Vec3i, id: u8) {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        if let Some(chunk) = self.chunks.get_mut(&(cx, cy, cz)) {
            chunk.index[morton_encode(lx, ly, lz)] = id;
            self.dirty = true;
        }
    }

    fn get_flow(&self, tile: Vec3i, goal: u8) -> Vec2f {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => {
                let layer = goal as usize;
                if layer < chunk.flow.len() { chunk.flow[layer][morton_encode(lx, ly, lz)] }
                else { Vec2f::zero() }
            }
            None => Vec2f::zero(),
        }
    }

    fn is_goal_tile(&self, tile: Vec3i, goal: u8) -> bool {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => {
                let idx = morton_encode(lx, ly, lz);
                chunk.tiles[idx] == TileType::Platform && chunk.index[idx] == goal
            }
            None => false,
        }
    }

    fn get_walls(&self, tile: Vec3i) -> &[WallLine] {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => {
                let i = morton_encode(lx, ly, lz);
                let s = chunk.wall_start[i] as usize;
                let n = chunk.wall_count[i] as usize;
                &chunk.walls[s..s + n]
            }
            None => &[],
        }
    }

    fn get_tile(&self, tile: Vec3i) -> TileType {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => chunk.tiles[morton_encode(lx, ly, lz)],
            None => TileType::Empty,
        }
    }

    // TODO: ambiguous. this is for per frame clear
    fn clear_all_agents(&mut self) {
        for chunk in self.chunks.values_mut() { chunk.clear_agents(); }
    }

    /// Register an agent entity into a tile cell; also increments density and sets TILE_OCCUPIED flag
    fn register_agent(&mut self, tile: Vec3i, entity: Entity) {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        if let Some(chunk) = self.chunks.get_mut(&(cx, cy, cz)) {
            let idx = morton_encode(lx, ly, lz);
            chunk.flags[idx] |= TILE_OCCUPIED;
            chunk.agents[idx].push(entity);
            chunk.density[idx] += 1.0;
        }
    }

    fn get_density(&self, tile: Vec3i) -> f32 {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => chunk.density[morton_encode(lx, ly, lz)],
            None => 0.0,
        }
    }

    fn get_agents_at(&self, tile: Vec3i) -> &[Entity] {
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        match self.chunks.get(&(cx, cy, cz)) {
            Some(chunk) => &chunk.agents[morton_encode(lx, ly, lz)],
            None => &[],
        }
    }

    fn save(&self, filepath: &str) -> Result<(), hotline_rs::Error> {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"RR2M");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        for (&(cx, cy, cz), chunk) in &self.chunks {
            buf.extend_from_slice(&cx.to_le_bytes());
            buf.extend_from_slice(&cy.to_le_bytes());
            buf.extend_from_slice(&cz.to_le_bytes());
            let tile_bytes: &[u8; ARRAY_SIZE] = unsafe { std::mem::transmute(&chunk.tiles) };
            buf.extend_from_slice(tile_bytes);
            buf.extend_from_slice(&chunk.index);
        }
        std::fs::write(filepath, buf)?;
        Ok(())
    }

    fn load(path: &str) -> Result<Self, hotline_rs::Error> {
        let data = std::fs::read(path)?;
        let mut pos = 0usize;
        if data.get(pos..pos+4) != Some(b"RR2M") {
            return Err("invalid map file".into());
        }
        pos += 4;
        let version = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
        pos += 4;
        if version != 2 && version != 3 { return Err("map version mismatch".into()); }
        let num_chunks = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap()) as usize;
        pos += 4;
        let mut map = Map::new();
        for _ in 0..num_chunks {
            let cx = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            pos += 4;
            let cy = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            pos += 4;
            let cz = i32::from_le_bytes(data[pos..pos+4].try_into().unwrap());
            pos += 4;
            let chunk = map.chunks.entry((cx, cy, cz)).or_insert_with(MapChunk::new);
            let tile_bytes: [u8; ARRAY_SIZE] = data[pos..pos+ARRAY_SIZE].try_into().unwrap();
            chunk.tiles = unsafe { std::mem::transmute(tile_bytes) };
            pos += ARRAY_SIZE;
            if version >= 3 {
                chunk.index.copy_from_slice(&data[pos..pos+ARRAY_SIZE]);
                pos += ARRAY_SIZE;
            }
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

    const NEIGHBORS: [(i32, i32); 8] = [
        (1,0),(-1,0),(0,1),(0,-1),
        (1,1),(-1,1),(-1,-1),(1,-1)
    ];

    // pass 0: find max goal index across all Platform tiles, resize + zero all flow layers
    let mut max_goal = 0u8;
    for chunk in map.chunks.values() {
        for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
            let idx = morton_encode(lx, ly, lz);
            if chunk.tiles[idx] == TileType::Platform {
                max_goal = max_goal.max(chunk.index[idx]);
            }
        }}}
    }
    let num_goals = max_goal as usize + 1;
    for chunk in map.chunks.values_mut() {
        chunk.flow.resize(num_goals, [Vec2f::zero(); ARRAY_SIZE]);
        for layer in &mut chunk.flow { layer.fill(Vec2f::zero()); }
        chunk.pressure.fill(f32::MAX);
    }

    // passes 1-3: one Dijkstra per goal index
    for goal_idx in 0..num_goals as u8 {
        let mut cost: HashMap<(i32, i32, i32), u32> = HashMap::new();
        let mut heap: BinaryHeap<(Reverse<u32>, i32, i32, i32)> = BinaryHeap::new();

        // pass 1: seed heap with Platform tiles matching this goal
        for (&(cx, cy, cz), chunk) in &map.chunks {
            let cs = CHUNK_SIZE as i32;
            for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
                let idx = morton_encode(lx, ly, lz);
                if chunk.tiles[idx] == TileType::Platform && chunk.index[idx] == goal_idx {
                    let (tx, ty, tz) = (cx*cs + lx as i32, cy*cs + ly as i32, cz*cs + lz as i32);
                    cost.insert((tx, ty, tz), 0);
                    heap.push((Reverse(0), tx, ty, tz));
                }
            }}}
        }
        if heap.is_empty() { continue; }

        // pass 2: Dijkstra flood fill (XZ neighbors only)
        while let Some((Reverse(c), tx, ty, tz)) = heap.pop() {
            if cost.get(&(tx, ty, tz)).copied().unwrap_or(u32::MAX) < c { continue; }
            for (dx, dz) in NEIGHBORS {
                let (nx, nz) = (tx + dx, tz + dz);
                let (cx, cy, cz, _, _, _) = tile_to_chunk(nx, ty, nz);
                if !map.chunks.contains_key(&(cx, cy, cz)) { continue; }
                let tile = map.get_tile(Vec3i::new(nx, ty, nz));
                if !is_walkable(tile) { continue; }
                let new_cost = c + 1;
                if new_cost < cost.get(&(nx, ty, nz)).copied().unwrap_or(u32::MAX) {
                    cost.insert((nx, ty, nz), new_cost);
                    heap.push((Reverse(new_cost), nx, ty, nz));
                }
            }
        }

        // pass 3: compute flow direction (gradient toward lowest-cost neighbour)
        let flow_updates: Vec<(i32, i32, i32, Vec2f, f32)> = cost.iter()
            .map(|(&(tx, ty, tz), &c)| {
                let mut best_dir = Vec2f::zero();
                let mut best_cost = c;
                for (dx, dz) in NEIGHBORS {
                    let nc = cost.get(&(tx+dx, ty, tz+dz)).copied().unwrap_or(u32::MAX);
                    if nc < best_cost { best_cost = nc; best_dir = Vec2f::new(dx as f32, dz as f32); }
                }
                (tx, ty, tz, best_dir, c as f32)
            })
            .collect();

        for (tx, ty, tz, flow_dir, pressure) in flow_updates {
            let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
            if let Some(chunk) = map.chunks.get_mut(&(cx, cy, cz)) {
                let idx = morton_encode(lx, ly, lz);
                chunk.flow[goal_idx as usize][idx] = flow_dir;
                chunk.pressure[idx] = pressure;
            }
        }
    }

    // pass 4: bake wall lines into flat per-chunk arrays, indexed by morton tile
    // Phase A: build (chunk_key, [(morton_idx, Vec<WallLine>)]) using only immutable map borrows
    let baked: Vec<((i32,i32,i32), Vec<(usize, Vec<WallLine>)>)> = map.chunks.keys()
        .copied()
        .map(|(cx, cy, cz)| {
            let cs = CHUNK_SIZE as i32;
            let tile_walls: Vec<(usize, Vec<WallLine>)> = (0..CHUNK_SIZE)
                .flat_map(|ly| (0..CHUNK_SIZE).flat_map(move |lz| (0..CHUNK_SIZE).map(move |lx| (lx, ly, lz))))
                .filter_map(|(lx, ly, lz)| {
                    let idx = morton_encode(lx, ly, lz);
                    if !is_walkable(map.chunks[&(cx,cy,cz)].tiles[idx]) { return None; }
                    let (tx, ty, tz) = (cx*cs + lx as i32, cy*cs + ly as i32, cz*cs + lz as i32);
                    let cx_f = tx as f32 * TILE_SIZE + TILE_SIZE * 0.5;
                    let cy_f = ty as f32 * TILE_SIZE;
                    let cz_f = tz as f32 * TILE_SIZE + TILE_SIZE * 0.5;
                    let mut lines: Vec<WallLine> = Vec::new();

                    for (dx, dz) in [(-1i32,0i32),(1,0),(0,-1),(0,1)] {
                        if is_walkable(map.get_tile(Vec3i::new(tx+dx, ty, tz+dz))) { continue; }
                        let (p0x, p0z, p1x, p1z) = match (dx, dz) {
                            (-1, 0) => (-0.5, -0.5, -0.5,  0.5),
                            ( 1, 0) => ( 0.5, -0.5,  0.5,  0.5),
                            ( 0,-1) => (-0.5, -0.5,  0.5, -0.5),
                            ( 0, 1) => (-0.5,  0.5,  0.5,  0.5),
                            _ => unreachable!(),
                        };
                        lines.push(WallLine {
                            p0: vec3f(cx_f + p0x * TILE_SIZE, cy_f, cz_f + p0z * TILE_SIZE),
                            p1: vec3f(cx_f + p1x * TILE_SIZE, cy_f, cz_f + p1z * TILE_SIZE),
                            perp: vec3f(-dx as f32, 0.0, -dz as f32),
                        });
                    }

                    for (dx, dz) in [(-1i32,-1i32),(-1,1),(1,1),(1,-1)] {
                        if is_walkable(map.get_tile(Vec3i::new(tx+dx, ty, tz+dz))) { continue; }
                        if is_walkable(map.get_tile(Vec3i::new(tx, ty, tz+dz))) {
                            let (ax, az0, az1) = (dx as f32 * 0.5, dz as f32 * 0.5, dz as f32 * 1.5);
                            lines.push(WallLine {
                                p0: vec3f(cx_f + ax * TILE_SIZE, cy_f, cz_f + az0 * TILE_SIZE),
                                p1: vec3f(cx_f + ax * TILE_SIZE, cy_f, cz_f + az1 * TILE_SIZE),
                                perp: vec3f(-dx as f32, 0.0, 0.0),
                            });
                        }
                        if is_walkable(map.get_tile(Vec3i::new(tx+dx, ty, tz))) {
                            let (bz, bx0, bx1) = (dz as f32 * 0.5, dx as f32 * 0.5, dx as f32 * 1.5);
                            lines.push(WallLine {
                                p0: vec3f(cx_f + bx0 * TILE_SIZE, cy_f, cz_f + bz * TILE_SIZE),
                                p1: vec3f(cx_f + bx1 * TILE_SIZE, cy_f, cz_f + bz * TILE_SIZE),
                                perp: vec3f(0.0, 0.0, -dz as f32),
                            });
                        }
                    }

                    Some((idx, lines))
                })
                .collect();
            ((cx, cy, cz), tile_walls)
        })
        .collect();

    // Phase B: write baked walls into chunks
    for (key, tile_walls) in baked {
        let chunk = map.chunks.get_mut(&key).unwrap();
        chunk.walls.clear();
        chunk.wall_start = [0; ARRAY_SIZE];
        chunk.wall_count = [0; ARRAY_SIZE];
        for (idx, lines) in tile_walls {
            chunk.wall_start[idx] = chunk.walls.len() as u32;
            chunk.wall_count[idx] = lines.len() as u8;
            chunk.walls.extend(lines);
        }
    }

    Ok(())
}

/// Moves agents along the flow field with per-cell peer-repulsion spreading
#[export_update_fn]
pub fn update_agents(
    mut agents: Query<(Entity, &mut AgentPos, &SpeedScale, &mut WanderAngle, &mut SepForce, &mut LocalDensity, &mut FacingRot, &Goal, &mut Flags), With<HumanAgent>>,
    mut map: ResMut<Map>,
    mut soa: ResMut<AgentSoa>,
) -> Result<(), hotline_rs::Error> {

    // reborrow as raw ref so NLL can split field borrows (ResMut treats whole struct as one borrow)
    let soa: &mut AgentSoa = &mut *soa;
    soa.timers.clear();

    //
    // clear grid
    //
    let t = std::time::Instant::now();

    map.clear_all_agents();

    let total_agents = agents.iter().count();
    let sep_radius = (BASE_SEP_RADIUS - total_agents as f32 * SEP_RADIUS_DECAY).max(MIN_SEP_RADIUS);
    let sep_radius_sq = sep_radius * sep_radius;

    soa.timers.push(("clear", t.elapsed().as_micros() as u64));

    //
    // Pass 1: register each agent in the spatial grid (read-only)
    // Primary tile only — loose-grid registration removed to prevent double-counted separation forces
    //
    let t = std::time::Instant::now();

    for (entity, pos, ..) in agents.iter() {
        let tile = world_to_tile(pos.0);
        map.register_agent(tile, entity);
    }

    soa.timers.push(("p1 reg", t.elapsed().as_micros() as u64));

    //
    // Pass 2: scan flags to build occupied list (chunk_key, morton_idx), then flat per-agent cells.
    //
    let t = std::time::Instant::now();

    let occupied: Vec<((i32,i32,i32), usize)> = map.chunks.iter()
        .flat_map(|(&ck, chunk)| {
            chunk.flags.iter().enumerate()
                .filter(|(_, &f)| f & TILE_OCCUPIED != 0)
                .map(move |(idx, _)| (ck, idx))
        })
        .collect();

    // (entity_idx, chunk_key, morton_idx) — one entry per agent
    let cells: Vec<(usize, (i32,i32,i32), usize)> = occupied.iter()
        .flat_map(|&(ck, idx)| {
            map.chunks[&ck].agents[idx].iter().map(move |&e| (e.index() as usize, ck, idx))
        })
        .collect();

    // Fill flat buffers — indexed by entity.index()
    let max_idx = agents.iter().map(|(e, ..)| e.index() as usize).max().unwrap_or(0);
    if soa.pos.len() <= max_idx { soa.pos.resize(max_idx + 1, Vec3f::zero()); }
    if soa.sep.len() <= max_idx { soa.sep.resize(max_idx + 1, Vec2f::zero()); }
    if soa.density.len() <= max_idx { soa.density.resize(max_idx + 1, 0.0); }
    if soa.wander.len() <= max_idx { soa.wander.resize(max_idx + 1, 0.0); }
    if soa.facing.len() <= max_idx { soa.facing.resize(max_idx + 1, Quatf::identity()); }
    if soa.speed.len() <= max_idx { soa.speed.resize(max_idx + 1, 1.0); }
    if soa.goal.len() <= max_idx { soa.goal.resize(max_idx + 1, 0); }
    if soa.flags.len() <= max_idx { soa.flags.resize(max_idx + 1, 0); }
    for (e, pos, speed, wander, _, _, facing, goal, flags) in agents.iter() {
        let i = e.index() as usize;
        soa.pos[i]    = pos.0;
        soa.wander[i] = wander.0;
        soa.facing[i] = facing.0;
        soa.speed[i]  = speed.0;
        soa.goal[i]   = goal.0;
        soa.flags[i]  = flags.0;
    }

    soa.timers.push(("p2 build", t.elapsed().as_micros() as u64));

    //
    // p2_bake: sequential pass — gather nbr agent indices and pre-compute per-agent data.
    // Moves all chunk/morton lookups out of the parallel hot loop.
    //
    let t = std::time::Instant::now();

    let pb = soa.pos.as_slice();
    let gb = soa.goal.as_slice();
    let fbp = soa.flags.as_ptr() as usize;
    let fbp_len = soa.flags.len();

    let n = cells.len();
    let mut baked_density = vec![0.0f32;        n];
    let mut baked_flow    = vec![Vec2f::zero();  n];
    let mut baked_flags   = vec![0u8;            n]; // FLAG_WAITING
    // let mut baked_spread  = vec![Vec2f::zero();  n]; // lateral spread velocity
    let mut nbr_start     = vec![0usize;     n + 1];
    let mut nbr_flat      = Vec::<usize>::new();

    for (ci, &(ia, ck, idx)) in cells.iter().enumerate() {
        let chunk    = &map.chunks[&ck];
        let goal     = gb[ia] as usize;
        let density  = chunk.density[idx];
        let flow_dir = if goal < chunk.flow.len() { chunk.flow[goal][idx] } else { Vec2f::zero() };

        baked_density[ci] = density;
        baked_flow[ci]    = flow_dir;

        // gather all neighbour agents (3x3 in XZ) into nbr_flat
        let tile = world_to_tile(pb[ia]);
        for dz in -1i32..=1 { for dx in -1i32..=1 {
            let (ncx, ncy, ncz, nlx, nly, nlz) = tile_to_chunk(tile.x+dx, tile.y, tile.z+dz);
            let nidx = morton_encode(nlx, nly, nlz);
            if let Some(nc) = map.chunks.get(&(ncx, ncy, ncz)) {
                nbr_flat.extend(nc.agents[nidx].iter().map(|&e| e.index() as usize));
            }
        }}
        nbr_start[ci + 1] = nbr_flat.len();

        // waiting flag: at goal (neighbor_waiting deferred to p2_sep via nbr_flat)
        let at_goal = chunk.tiles[idx] == TileType::Platform && chunk.index[idx] == gb[ia];
        baked_flags[ci] = if at_goal { FLAG_WAITING } else { 0u8 };

        // TODO: not working
        // lateral spread velocity
        /*
        baked_spread[ci] = if (neighbor_waiting || at_goal) && length(flow_norm_2d) > 0.001 {
            let perp = Vec2f::new(-flow_norm_2d.y, flow_norm_2d.x);
            let pi   = Vec3i::new(perp.x.round() as i32, 0, perp.y.round() as i32);
            let (nx1,ny1,nz1,lx1,ly1,lz1) = tile_to_chunk(tile.x + pi.x, tile.y, tile.z + pi.z);
            let (nx2,ny2,nz2,lx2,ly2,lz2) = tile_to_chunk(tile.x - pi.x, tile.y, tile.z - pi.z);
            let d_pos = map.chunks.get(&(nx1,ny1,nz1)).map_or(0.0, |c| c.density[morton_encode(lx1,ly1,lz1)]);
            let d_neg = map.chunks.get(&(nx2,ny2,nz2)).map_or(0.0, |c| c.density[morton_encode(lx2,ly2,lz2)]);
            perp * (d_neg - d_pos) * QUEUE_SPREAD_STRENGTH
        } else { Vec2f::zero() };
        */
    }

    soa.timers.push(("p2 bake", t.elapsed().as_micros() as u64));

    //
    // p2_sep: parallel — pure arithmetic + direct array indexing, no chunk/morton lookups
    //
    let t = std::time::Instant::now();

    let sp = soa.sep.as_mut_ptr() as usize;
    let dp = soa.density.as_mut_ptr() as usize;
    let fgp = soa.flags.as_mut_ptr() as usize;

    cells.par_iter().enumerate().for_each(|(ci, &(ia, ck, idx), )| {
        let chunk = &map.chunks[&ck];
        let pos_a = pb[ia];
        let pa = vec2f(pos_a.x, pos_a.z);
        let nbrs = &nbr_flat[nbr_start[ci]..nbr_start[ci+1]];
        let flow_dir = baked_flow[ci];

        // seaparation vs agents
        let mut sep = Vec2f::zero();
        for &ib in nbrs {
            if ib == ia { continue; }
            let diff = pa - vec2f(pb[ib].x, pb[ib].z);
            let dist_sq = dot(diff, diff);
            if dist_sq > 0.01 && dist_sq < sep_radius_sq {
                let d = sqrt(dist_sq);
                sep += diff / d * (1.0 - (d / sep_radius).min(1.0));
            }
        }
        let cell_density  = baked_density[ci];
        let density_scale = 1.0 + (cell_density - 1.0).max(0.0) * 0.3;
        let sep_vel = if length(sep) > 0.001 { normalize(sep) * SEPARATION_STRENGTH * density_scale } else { Vec2f::zero() };

        // queuing vs agents
        // waiting flag: at_goal baked; check nbr_flat for same-goal forward neighbour waiting last frame
        let at_goal = (baked_flags[ci] & FLAG_WAITING) != 0;
        let flow_norm_2d = if length(flow_dir) > 0.001 { normalize(flow_dir) } else { Vec2f::zero() };
        let neighbor_waiting = !at_goal && length(flow_norm_2d) > 0.001 && nbrs.iter().any(|&ib| {
            // SAFETY: fbp aliases fgp (same soa.flags buffer); ib != ia so no write-read overlap
            ib != ia
            && ib < fbp_len && unsafe { *(fbp as *const u8).add(ib) } & FLAG_WAITING != 0
            && gb[ib] == gb[ia]
            && dot(vec2f(pb[ib].x, pb[ib].z) - pa, flow_norm_2d) > 0.0
        });

        // collision avoidance vs walls
        let s = chunk.wall_start[idx] as usize;
        let walls = &chunk.walls[s .. s + chunk.wall_count[idx] as usize];
        let flow3 = Vec3f::new(flow_dir.x, 0.0, flow_dir.y);
        let tile_flow = if length(flow3) > 0.001 { 
            normalize(flow3) 
        } 
        else { 
            Vec3f::zero() 
        };
        let mut wall_vel = Vec2f::zero();
        for wl in walls {
            let cp = closest_point_on_line_segment(pos_a, wl.p0, wl.p1);
            let d = dist(cp, pos_a);
            if d < WALL_AVOID_RADIUS {
                let tangent = normalize(wl.p0 - wl.p1);
                if dot(wl.perp, tile_flow) >= 0.0 {
                    wall_vel += vec2f(wl.perp.x, wl.perp.z) * AGENT_SPEED;
                } else {
                    wall_vel += tangent.xz() * dot(tangent, tile_flow) * AGENT_SPEED;
                }
            }
        }

        // SAFETY: ia is unique per entity
        unsafe {
            *(sp  as *mut Vec2f).add(ia) = sep_vel + wall_vel; // + baked_spread[ci];
            *(dp  as *mut f32).add(ia) = cell_density;
            *(fgp as *mut u8).add(ia) = if at_goal || neighbor_waiting { FLAG_WAITING } else { 0u8 };
        }
    });
    soa.timers.push(("p2 sep", t.elapsed().as_micros() as u64));

    //
    // Pass 3: apply forces and move — parallel scatter to flat buffers, batch ECS write
    //
    let t = std::time::Instant::now();

    let pp = soa.pos.as_mut_ptr() as usize;
    let wp = soa.wander.as_mut_ptr() as usize;
    let fp = soa.facing.as_mut_ptr() as usize;
    let sep = soa.sep.as_slice();
    let wb = soa.wander.as_slice();
    let fb = soa.facing.as_slice();
    let spd = soa.speed.as_slice();
    let pb = soa.pos.as_slice();
    let fb_prev = soa.flags.as_slice();
    cells.par_iter().enumerate().for_each(|(ci, &(ia, _, _))| {
        let pos_a = pb[ia];
        let avoid = vec3f(sep[ia].x, 0.0, sep[ia].y);
        let waiting = ia < fb_prev.len() && (fb_prev[ia] & FLAG_WAITING) != 0;
        let wander = wb[ia] + WANDER_DRIFT * (1.0 - (ia % 5) as f32 * 0.1);
        let wander_vel = vec3f(wander.cos(), 0.0, wander.sin()) * AGENT_SPEED * spd[ia] * WANDER_STRENGTH * if waiting { 0.3 } else { 1.0 };
        let flow = baked_flow[ci];
        let flow_norm = if length(flow) > 0.001 { normalize(flow) } else { flow };
        let flow_vel = vec3f(flow_norm.x, 0.0, flow_norm.y) * AGENT_SPEED * spd[ia] * if waiting { 0.0 } else { 1.0 };
        let total_vel = flow_vel + avoid + wander_vel;
        let new_pos = pos_a + total_vel;
        let new_facing = {
            let vel_len = length(vec3f(total_vel.x, 0.0, total_vel.z));
            if vel_len > 0.001 {
                let yaw = f32::atan2(total_vel.x, total_vel.z);
                slerp(fb[ia], Quatf::from_euler_angles(0.0, yaw, 0.0), TURN_RATE)
            } else { fb[ia] }
        };
        // SAFETY: ia is unique per entity
        unsafe {
            *(pp as *mut Vec3f).add(ia)  = new_pos;
            *(wp as *mut f32).add(ia)    = wander;
            *(fp as *mut Quatf).add(ia)  = new_facing;
        }
    });
    soa.timers.push(("p3 move", t.elapsed().as_micros() as u64));

    //
    // Correction pass: resolve agent-agent overlaps
    // Re-register from pos_buf so neighborhoods reflect post-movement positions (fixes tile-boundary gaps).
    //
    let t = std::time::Instant::now();

    map.clear_all_agents();
    for (entity, ..) in agents.iter() {
        map.register_agent(world_to_tile(soa.pos[entity.index() as usize]), entity);
    }

    // rebuild occupied + cells from post-move positions
    let occupied: Vec<((i32,i32,i32), usize)> = map.chunks.iter()
        .flat_map(|(&ck, chunk)| {
            chunk.flags.iter().enumerate()
                .filter(|(_, &f)| f & TILE_OCCUPIED != 0)
                .map(move |(idx, _)| (ck, idx))
        })
        .collect();
    let cells: Vec<(usize, (i32,i32,i32), usize)> = occupied.iter()
        .flat_map(|&(ck, idx)| {
            map.chunks[&ck].agents[idx].iter().map(move |&e| (e.index() as usize, ck, idx))
        })
        .collect();

    // bake neighbour lists for correction (no flow/flags needed)
    let n = cells.len();
    let mut corr_nbr_start = vec![0usize; n + 1];
    let mut corr_nbr_flat  = Vec::<usize>::new();
    for (ci, &(ia, _, _)) in cells.iter().enumerate() {
        let tile = world_to_tile(soa.pos[ia]);
        for dz in -1i32..=1 { for dx in -1i32..=1 {
            let (ncx, ncy, ncz, nlx, nly, nlz) = tile_to_chunk(tile.x+dx, tile.y, tile.z+dz);
            let nidx = morton_encode(nlx, nly, nlz);
            if let Some(nc) = map.chunks.get(&(ncx, ncy, ncz)) {
                corr_nbr_flat.extend(nc.agents[nidx].iter().map(|&e| e.index() as usize));
            }
        }}
        corr_nbr_start[ci + 1] = corr_nbr_flat.len();
    }

    let min_dist    = AGENT_RADIUS * 2.0;
    let min_dist_sq = min_dist * min_dist;

    let mut delta = vec![Vec3f::zero(); soa.pos.len()];
    for _ in 0..CORRECTION_ITERATIONS {
        // SAFETY: each par task writes to a unique index (entity indices are unique).
        let dp = delta.as_mut_ptr() as usize;
        let pb = soa.pos.as_slice();
        cells.par_iter().enumerate().for_each(|(ci, &(ia, _, _))| {
            let pos_a = pb[ia];
            let mut correction = Vec3f::zero();
            for &ib in &corr_nbr_flat[corr_nbr_start[ci]..corr_nbr_start[ci+1]] {
                if ib == ia { continue; }
                let diff = (pos_a - pb[ib]) * vec3f(1.0, 0.0, 1.0);
                let dist_sq = dot(diff, diff);
                if dist_sq < min_dist_sq && dist_sq > 0.0001 {
                    let d = sqrt(dist_sq);
                    correction += (diff / d) * (min_dist - d) * 0.5;
                }
            }
            unsafe { *(dp as *mut Vec3f).add(ia) = correction; }
        });
        for (p, d) in soa.pos.iter_mut().zip(delta.iter_mut()) {
            *p += *d;
            *d = Vec3f::zero();
        }
    }

    soa.timers.push(("correction", t.elapsed().as_micros() as u64));

    //
    // Pass 5: wall collision resolution — per occupied tile, walls stay in cache
    //
    let t = std::time::Instant::now();

    for &(ck, idx) in &occupied {
        let chunk = &map.chunks[&ck];
        let s = chunk.wall_start[idx] as usize;
        let walls = &chunk.walls[s .. s + chunk.wall_count[idx] as usize];
        for &entity_a in &chunk.agents[idx] {
            let p = &mut soa.pos[entity_a.index() as usize];
            for wl in walls {
                let cp = closest_point_on_line_segment(*p, wl.p0, wl.p1);
                let d = dist(cp, *p);
                if d < AGENT_RADIUS && d > 0.0001 {
                    *p += normalize(*p - cp) * (AGENT_RADIUS - d) * vec3f(1.0, 0.0, 1.0);
                }
            }
        }
    }

    soa.timers.push(("p5 walls", t.elapsed().as_micros() as u64));

    // single batch ECS write — all flat buffers are final after p5
    for (entity, mut pos, _, mut wander, mut sf, mut ld, mut facing, _, mut fl) in agents.iter_mut() {
        let ia = entity.index() as usize;
        pos.0    = soa.pos[ia];
        wander.0 = soa.wander[ia];
        sf.0     = soa.sep[ia];
        ld.0     = soa.density[ia];
        facing.0 = soa.facing[ia];
        fl.0     = soa.flags[ia];
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
    for (pos, facing) in agents.iter() {
        let p = pos.0;
        let base = vec3f(p.x, Y_OFFSET, p.z);
        imdraw.add_circle_3d_xz(base, AGENT_RADIUS, Vec4f::red());
        imdraw.add_circle_3d_xz(base, WALL_AVOID_RADIUS, Vec4f::blue());
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
    commands.insert_resource(
        EditorState { 
            mode: EditorMode::Tile, 
            tile_idx: 0, 
            agent_grid: 1,
            index: 0,
            agent_goal: 0,
            viz_goal: 0,
            left_was_down: false,
            save_filepath: MAP_SAVE_PATH.to_string()
        });

    // create shared cube mesh for agent bodies
    let cube = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.insert_resource(AgentMesh(cube));
    commands.insert_resource(AgentSoa::default());

    Ok(())
}

#[export_update_fn]
pub fn update_tile_editor_ui(
    mut commands: Commands,
    mut imgui: ResMut<ImGuiRes>,
    mut map: ResMut<Map>,
    mut editor: ResMut<EditorState>,
    agents: Query<Entity, With<HumanAgent>>,
) -> Result<(), hotline_rs::Error> {
    imgui.set_global_context();

    if imgui.begin_window("Editor") {
        // save / load
        imgui.input_text("##filepath", &mut editor.save_filepath, 256);
        imgui.same_line();
        if imgui.button("Save") {
            let _ = map.save(&editor.save_filepath);
        }
        imgui.same_line();
        if imgui.button("Load") {
            if let Ok(mut loaded) = Map::load(&editor.save_filepath) {
                loaded.dirty = true;
                *map = loaded;
            }
        }

        if imgui.button(if editor.mode == EditorMode::Tile { ">Tile" } else { " Tile" }) {
            editor.mode = EditorMode::Tile;
        }
        imgui.same_line();
        if imgui.button(if editor.mode == EditorMode::Agent { ">Agent" } else { " Agent" }) {
            editor.mode = EditorMode::Agent;
        }
        imgui.separator();
        match editor.mode {
            EditorMode::Tile => {
                let names: Vec<String> = TILE_TYPES.iter().map(|t| format!("{:?}", t)).collect();
                let cur = format!("{:?}", TILE_TYPES[editor.tile_idx]);
                let (_, chosen) = imgui.combo_list("Tile", &names, &cur);
                if let Some(idx) = names.iter().position(|n| n == &chosen) {
                    editor.tile_idx = idx;
                }
                let mut pi = editor.index as i32;
                imgui.input_int("Index", &mut pi);
                editor.index = pi.clamp(0, 255) as u8;
            }
            EditorMode::Agent => {
                imgui.input_int("Grid", &mut editor.agent_grid);
                editor.agent_grid = editor.agent_grid.max(1);
                let mut ag = editor.agent_goal as i32;
                imgui.input_int("Goal", &mut ag);
                editor.agent_goal = ag.clamp(0, 255) as u8;
                imgui.text(&format!("{}x{} agents → goal {}", editor.agent_grid, editor.agent_grid, editor.agent_goal));
            }
        }
        imgui.separator();
        let num_layers = map.chunks.values().map(|c| c.flow.len()).max().unwrap_or(1);
        let flow_items: Vec<String> = (0..num_layers).map(|i| format!("Goal {}", i)).collect();
        let cur_viz = format!("Goal {}", editor.viz_goal.min((num_layers - 1) as u8));
        let (_, selected) = imgui.combo_list("Viz Flow", &flow_items, &cur_viz);
        editor.viz_goal = selected.trim_start_matches("Goal ").parse::<u8>().unwrap_or(0);

        imgui.separator();

        if imgui.button("clear map") { 
            *map = Map::new(); 
        }
        imgui.same_line();
        if imgui.button("clear agents") { 
            for entity in &agents {
                commands.entity(entity).despawn();
            }
        }

        imgui.separator();
        let floor_tiles: usize = map.chunks.values()
            .map(|c| c.tiles.iter().filter(|&&t| is_walkable(t)).count())
            .sum();
        let wall_count: usize = map.chunks.values().map(|c| c.walls.len()).sum();
        imgui.text(&format!("agents: {}", agents.iter().count()));
        imgui.text(&format!("chunks: {}", map.chunks.len()));
        imgui.text(&format!("floors: {}", floor_tiles));
        imgui.text(&format!("walls:  {}", wall_count));
    }
    imgui.end();
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
        Vec4f::black(),                          // Empty
        Vec4f::white(),                          // Wall
        Vec4f::red(),                            // Barrier
        Vec4f::blue(),                           // Escalator
        Vec4f::yellow(),                         // Platform
        Vec4f::green(),                          // TrainTrack
        Vec4f::new(0.4, 0.4, 0.4, 1.0),          // Floor
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
                if !app.is_sys_key_down(os::SysKey::Alt) && !app.is_sys_key_down(os::SysKey::Shift) {
                    let buttons = app.get_mouse_buttons();
                    let left_down = buttons[os::MouseButton::Left as usize];
                    let left_pressed = left_down && !editor.left_was_down;

                    if !app.is_sys_key_down(os::SysKey::Ctrl) {
                        if left_down {
                            match editor.mode {
                                EditorMode::Tile => {
                                    map.set_tile(Vec3i::new(tile_x, 0, tile_z), TILE_TYPES[editor.tile_idx]);
                                    map.set_tile_index(Vec3i::new(tile_x, 0, tile_z), editor.index);
                                }
                                EditorMode::Agent => {
                                    if left_pressed {
                                        let n = editor.agent_grid;
                                        let spacing = AGENT_RADIUS * 2.5;
                                        let half = (n - 1) as f32 * spacing * 0.5;
                                        for gz in 0..n {
                                            for gx in 0..n {
                                                let ax = hit.x + gx as f32 * spacing - half;
                                                let az = hit.z + gz as f32 * spacing - half;
                                                commands.spawn((
                                                    HumanAgent,
                                                    AgentPos(Vec3f::new(ax, 0.0, az)),
                                                    SpeedScale(0.8 + pos_hash(ax, az) * 0.4),
                                                    WanderAngle(pos_hash(az, ax) * std::f32::consts::TAU),
                                                    SepForce(Vec2f::zero()),
                                                    LocalDensity(0.0),
                                                    FacingRot(Quatf::identity()),
                                                    Goal(editor.agent_goal),
                                                    Flags(0),
                                                    MeshComponent(cube_mesh.0.clone()),
                                                    Position(vec3f(ax, AGENT_BODY_HALF_HEIGHT, az)),
                                                    Rotation(Quatf::identity()),
                                                    Scale(vec3f(AGENT_BODY_HALF_WIDTH, AGENT_BODY_HALF_HEIGHT, AGENT_BODY_HALF_WIDTH)),
                                                    WorldMatrix(Mat34f::identity()),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    else {
                        if left_down {
                            map.set_tile(Vec3i::new(tile_x, 0, tile_z), TileType::Empty);
                        }
                    }
                    editor.left_was_down = left_down;
                }
            }
        }
    }

    // draw all placed tiles, flow arrows, and baked collision wall lines
    for (&(cx, cy, cz), chunk) in &map.chunks {
        let cs = CHUNK_SIZE as i32;
        for ly in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let idx = morton_encode(lx, ly, lz);
                let tile = chunk.tiles[idx];

                let tx = cx * cs + lx as i32;
                let ty = cy * cs + ly as i32;
                let tz = cz * cs + lz as i32;
                let y = ty as f32 * TILE_SIZE + Y_OFFSET;

                let min_x = tx as f32 * TILE_SIZE;
                let min_z = tz as f32 * TILE_SIZE;
                let max_x = min_x + TILE_SIZE;
                let max_z = min_z + TILE_SIZE;

                let mid_x = min_x + (max_x - min_x) * 0.5;
                let mid_z = min_z + (max_z - min_z) * 0.5;

                if is_walkable(tile) {
                    let col = tile_cols[tile as usize];
                    imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, min_z), col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(max_x, y, max_z), col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, max_z), Vec3f::new(min_x, y, max_z), col);
                    imdraw.add_line_3d(Vec3f::new(min_x, y, max_z), Vec3f::new(min_x, y, min_z), col);
                    imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, max_z), col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(min_x, y, max_z), col);

                    let flow_dir = chunk.flow.get(editor.viz_goal as usize).map(|l| l[idx]).unwrap_or(Vec2f::zero());
                    if flow_dir != Vec2f::zero() {
                        let flow_dir3 = Vec3f::new(flow_dir.x, 0.0, flow_dir.y);
                        let flow_start = Vec3f::new(mid_x, y, mid_z);
                        let flow_col = Vec4f::from((flow_dir3.xy() * 0.5 + 0.8, 0.0, 1.0));
                        let flow_end = flow_start + flow_dir3;
                        imdraw.add_line_3d(flow_start, flow_end, flow_col);
                        let arrow_size = 0.2;
                        let perp = maths_rs::perp(flow_dir3.xz()) * arrow_size;
                        let perp3 = Vec3f::new(perp.x, 0.0, perp.y);
                        let tip = flow_end - flow_dir3 * arrow_size;
                        imdraw.add_line_3d(flow_end, tip + perp3, flow_col);
                        imdraw.add_line_3d(flow_end, tip - perp3, flow_col);
                    }
                }
            }
            }
        }

        // draw baked collision wall outlines
        let wall_col = Vec4f::white();
        let y_off = Vec3f::new(0.0, Y_OFFSET, 0.0);
        for wl in &chunk.walls {
            imdraw.add_line_3d(wl.p0 + y_off, wl.p1 + y_off, wall_col);
        }
    }

    Ok(())
}

#[export_update_fn]
pub fn update_perf_ui(
    mut imgui: ResMut<ImGuiRes>,
    soa: Res<AgentSoa>,
) -> Result<(), hotline_rs::Error> {
    imgui.set_global_context();
    if imgui.begin_window("perf") {
        imgui.separator();
        imgui.text("agent soa");
        for (name, us) in &soa.timers {
            imgui.text(&format!("{:<12} {:>6} us", name, us));
        }
    }
    imgui.end();
    Ok(())
}
