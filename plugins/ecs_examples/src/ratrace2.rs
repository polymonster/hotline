//
/// Rat Race 2 - Tilemap Editor & Game
///

// TODO: keep eye on the random intermittent crash wne adding agents
// TODO: lateral movement is happening, but they stop bunching together so much and dont form a tighter queue.

// TODO: rebuild flow should be continuous async


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
const WANDER_DRIFT: f32 = 0.012;  // radians per frame
const WANDER_STRENGTH: f32 = 0.08;   // fraction of agent speed
const WAIT_PULL_STRENGTH:  f32 = 0.025; // spring constant pulling waiting agents back to wait_pos
const WAIT_DAMPING:        f32 = 0.4;   // velocity scale for waiting agents — dampens sep/pull jitter
const WAIT_SLOTS_PER_TILE: u32 = 32;    // max distinct halton-sampled wait positions before wrap
const WAIT_SLOT_INSET:     f32 = 0.7;   // halton sample range within tile (fraction of TILE_SIZE)
const JOIN_CROWD_MIN_DENSITY: f32 = 9.0; // low-tolerance agents commit at this nbr density
const JOIN_CROWD_MAX_DENSITY: f32 = 12.0; // high-tolerance agents hold out until at least this dense
const WALL_AVOID_RADIUS: f32 = AGENT_RADIUS * 2.5; // soft avoidance lookahead, larger than collision radius

const MAP_SAVE_PATH: &str = "trains.bin";

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
            "update_train_cycle",
            "update_entrance_auto_spawn",
            "update_train_boarding",
            "update_train_motion",
            "update_entrance_spawn",
            "update_train_dropoff",
            "update_entrance_despawn",
            "update_agents",
            "update_agent_transforms",
            // "debug_draw_agents",
            "update_tile_editor_ui",
            "update_perf_ui"
        ],
        render_graph: "mesh_wireframe_overlay"
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum TileType {
    Empty,         // 0 — unset / outside region; treated as wall
    Wall,          // 1 — explicit interior wall
    Barrier,       // 2
    Escalator,     // 3
    Platform,      // 4 — flow sink
    TrainTrack,    // 5
    Floor,         // 6 — explicitly placed walkable surface
    Train,         // 7 — seat sink inside a train, paired with Platform of same index (TRAIN_GOAL_OFFSET)
    TrainBoarding, // 8 — standing area inside a train at door positions (BOARDING_GOAL_OFFSET)
    Entrance,      // 9 — spawn / despawn point; agents flow here via goal (index + ENTRANCE_GOAL_OFFSET)
}

fn is_walkable(t: TileType) -> bool {
    matches!(t, TileType::Floor | TileType::Platform | TileType::Escalator | TileType::TrainTrack | TileType::Train | TileType::TrainBoarding | TileType::Entrance)
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
    /// monotonically increments (mod WAIT_SLOTS_PER_TILE) on each agent's first arrival;
    /// combined with a per-tile hash offset to pick a Halton sample for the wait position
    wait_slot: [std::sync::atomic::AtomicU8; ARRAY_SIZE],
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
            wait_slot:  std::array::from_fn(|_| std::sync::atomic::AtomicU8::new(0)),
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

/// Halton low-discrepancy sequence in 1D for the given prime base; output in [0, 1)
fn halton_1d(mut i: u32, base: u32) -> f32 {
    let mut f = 1.0_f32;
    let mut r = 0.0_f32;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// 2D Halton sample → tile-relative XZ offset in [-INSET/2, +INSET/2] * TILE_SIZE
fn halton_wait_offset(slot: u32) -> (f32, f32) {
    // skip index 0 (always (0,0)); +1 gives a well-spread first sample
    let hx = halton_1d(slot + 1, 2);
    let hz = halton_1d(slot + 1, 3);
    ((hx - 0.5) * WAIT_SLOT_INSET * TILE_SIZE,
     (hz - 0.5) * WAIT_SLOT_INSET * TILE_SIZE)
}

/// Deterministic per-tile offset into the Halton sequence — adds variation between tiles
fn tile_slot_offset(tx: i32, tz: i32) -> u32 {
    (tx as u32).wrapping_mul(73856093) ^ (tz as u32).wrapping_mul(19349663)
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
    TileType::Train,
    TileType::TrainBoarding,
    TileType::Entrance,
];

const BOARDING_GOAL_OFFSET: u8 = 64;  // TrainBoarding tile index N seeds flow layer (N + 64)
const TRAIN_GOAL_OFFSET:    u8 = 128; // Train tile index N seeds flow layer (N + 128)
const ENTRANCE_GOAL_OFFSET: u8 = 192; // Entrance tile index N seeds flow layer (N + 192)
const DEPTH_BIAS_SCALE:     u32 = 3;  // per-step penalty added to seat seed cost so back-of-train wins
const TRAIN_TRAVEL_DIST:    f32 = 200.0; // world units the train slides during leave/arrive
const TRAIN_TRANSIT_SECS:   f32 = 3.0;   // duration of leaving / arriving animations
const CYCLE_BOARDING_SECS:  f32 = 6.0;   // how long doors stay open per cycle
const CYCLE_DOOR_DWELL:     f32 = 1.0;   // pause between open/close and motion start
const CYCLE_GONE_SECS:      f32 = 4.0;   // how long the train is offscreen between cycles
const ENTRANCE_SPAWN_SECS:  f32 = 1.5;   // auto-spawn interval per entrance

/// Shared cube mesh resource for agent body rendering
#[derive(Resource)]
pub(crate) struct AgentMesh(pmfx::Mesh<gfx_platform::Device>);

/// Per-train motion state. Translate-in/out is render-only (logical positions are station-aligned).
#[derive(Clone, Copy, Debug)]
pub enum TrainState { AtStation, Leaving, Gone, Arriving }

#[derive(Clone, Copy)]
pub struct TrainMotion {
    pub state:   TrainState,
    pub started: std::time::Instant, // moment the current state began (for animated progress)
    pub axis:    Vec3f,              // unit vector along the platform's long axis
}

impl Default for TrainMotion {
    fn default() -> Self {
        Self { state: TrainState::AtStation, started: std::time::Instant::now(), axis: Vec3f::new(1.0, 0.0, 0.0) }
    }
}

/// Auto-cycle step indicator for the timed train sequence.
#[derive(Clone, Copy, Debug)]
pub enum CycleStep { OpenDoors, Boarding, CloseDoors, Leave, GoneDwell, Arrive, ArriveDwell }

/// Per-train auto-cycle state. When enabled, marches through CycleStep transitions on a timer
/// without manual button presses. Sequence: Open → Boarding → Close → Leave → Gone → Arrive → settle → loop.
#[derive(Clone, Copy)]
pub struct TrainCycle {
    pub enabled:    bool,
    pub step:       CycleStep,
    pub step_started: std::time::Instant,
    pub dropoff_emitted: bool, // ensure drop-off only happens once per ArriveDwell phase
}

impl Default for TrainCycle {
    fn default() -> Self {
        Self { enabled: false, step: CycleStep::OpenDoors, step_started: std::time::Instant::now(), dropoff_emitted: false }
    }
}

/// Train state: doors, motion, drop-off requests, cycle, and queued button presses.
#[derive(Resource, Default)]
pub struct Trains {
    pub doors_open:       HashMap<u8, bool>,
    pub pending:          Vec<(u8, bool)>, // (train_id, target_open) consumed by update_train_boarding
    pub motion:           HashMap<u8, TrainMotion>,
    pub pending_motion:   Vec<(u8, bool)>, // (train_id, true=leave / false=arrive)
    pub pending_dropoff:  Vec<(u8, u32)>,  // (train_id, count) consumed by update_train_dropoff
    pub cycle:            HashMap<u8, TrainCycle>,
    pub dropoff_density:  HashMap<u8, f32>, // 0..1 — fraction of train capacity to drop off per cycle
}

const DEFAULT_DROPOFF_DENSITY: f32 = 0.3;

/// Capacity = (number of Train tiles for train_id) * WAIT_SLOTS_PER_TILE.
fn train_capacity(map: &Map, train_id: u8) -> u32 {
    let mut tiles = 0u32;
    for chunk in map.chunks.values() {
        for i in 0..ARRAY_SIZE {
            if chunk.tiles[i] == TileType::Train && chunk.index[i] == train_id {
                tiles += 1;
            }
        }
    }
    tiles * WAIT_SLOTS_PER_TILE
}

/// Dropoff count = capacity * density (rounded). Density defaults to DEFAULT_DROPOFF_DENSITY.
fn train_dropoff_count(trains: &Trains, map: &Map, train_id: u8) -> u32 {
    let density = trains.dropoff_density.get(&train_id).copied().unwrap_or(DEFAULT_DROPOFF_DENSITY);
    ((train_capacity(map, train_id) as f32) * density).round() as u32
}

/// Per-entrance auto-spawn state.
#[derive(Clone, Copy)]
pub struct EntranceAuto {
    pub enabled:     bool,
    pub last_spawn:  std::time::Instant,
    pub interval_s:  f32,
}

impl Default for EntranceAuto {
    fn default() -> Self {
        Self { enabled: false, last_spawn: std::time::Instant::now(), interval_s: ENTRANCE_SPAWN_SECS }
    }
}

/// Entrance state: spawn requests, per-entrance auto-spawn, seed for random goal selection.
#[derive(Resource, Default)]
pub struct Entrances {
    pub pending_spawn: Vec<(u8, u32)>, // (entrance_id, count)
    pub auto:          HashMap<u8, EntranceAuto>,
    pub seed:          u64,
}

/// Cheap pseudo-random — used for goal selection on spawn; quality unimportant.
fn rand_u32(seed: u64) -> u32 {
    let mut x = seed.wrapping_mul(2654435761).wrapping_add(1442695040888963407);
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x as u32
}

/// All unique tile indices for a given TileType in the map.
fn collect_indices(map: &Map, kind: TileType) -> Vec<u8> {
    let mut out: Vec<u8> = map.chunks.values()
        .flat_map(|c| c.tiles.iter().enumerate()
            .filter(|(_, &t)| t == kind)
            .map(|(i, _)| c.index[i]))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// World-space tile centres for tiles of `kind` with matching `index`.
fn collect_tile_centres(map: &Map, kind: TileType, index: u8) -> Vec<Vec3f> {
    let mut out = Vec::new();
    for (&(cx, _, cz), chunk) in &map.chunks {
        let cs = CHUNK_SIZE as i32;
        for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
            let idx = morton_encode(lx, ly, lz);
            if chunk.tiles[idx] == kind && chunk.index[idx] == index {
                let tx = (cx*cs + lx as i32) as f32 + 0.5;
                let tz = (cz*cs + lz as i32) as f32 + 0.5;
                out.push(Vec3f::new(tx * TILE_SIZE, 0.0, tz * TILE_SIZE));
            }
        }}}
    }
    out
}

/// Animated progress 0..1 for Leaving/Arriving; clamped.
fn train_motion_progress(m: &TrainMotion) -> f32 {
    let elapsed = m.started.elapsed().as_secs_f32();
    (elapsed / TRAIN_TRANSIT_SECS).clamp(0.0, 1.0)
}

/// Visual world-space offset to apply to a train's tiles and to agents who boarded it.
/// Leaving slides 0 → +axis*D; Gone holds at +D; Arriving slides −D → 0.
fn train_visual_offset(m: &TrainMotion) -> Vec3f {
    match m.state {
        TrainState::AtStation => Vec3f::zero(),
        TrainState::Leaving   => m.axis * train_motion_progress(m) * TRAIN_TRAVEL_DIST,
        TrainState::Gone      => m.axis * TRAIN_TRAVEL_DIST,
        TrainState::Arriving  => m.axis * (train_motion_progress(m) - 1.0) * TRAIN_TRAVEL_DIST,
    }
}

/// Infer the train's translation axis from the bounding box of Platform tiles with matching index.
/// Long axis = direction the train travels along.
fn infer_train_axis(map: &Map, train_id: u8) -> Vec3f {
    let mut min_x = i32::MAX; let mut max_x = i32::MIN;
    let mut min_z = i32::MAX; let mut max_z = i32::MIN;
    let mut found = false;
    for (&(cx, _, cz), chunk) in &map.chunks {
        let cs = CHUNK_SIZE as i32;
        for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
            let idx = morton_encode(lx, ly, lz);
            if chunk.tiles[idx] == TileType::Platform && chunk.index[idx] == train_id {
                let tx = cx*cs + lx as i32;
                let tz = cz*cs + lz as i32;
                min_x = min_x.min(tx); max_x = max_x.max(tx);
                min_z = min_z.min(tz); max_z = max_z.max(tz);
                found = true;
            }
        }}}
    }
    if !found { return Vec3f::new(1.0, 0.0, 0.0); }
    if (max_x - min_x) >= (max_z - min_z) { Vec3f::new(1.0, 0.0, 0.0) }
    else                                  { Vec3f::new(0.0, 0.0, 1.0) }
}

/// Flat per-entity SoA buffers, all indexed by entity.index()
#[derive(Resource, Default)]
pub(crate) struct AgentSoa {
    pub pos:      Vec<Vec3f>,
    pub sep:      Vec<Vec2f>,
    pub density:  Vec<f32>,
    pub wander:   Vec<f32>,
    pub facing:   Vec<Quatf>,
    pub speed:    Vec<f32>,
    pub goal:     Vec<u8>,
    pub flags:    Vec<u8>,
    pub wait_pos: Vec<Vec3f>, // assigned on first arrival at goal, pulled toward while waiting
    /// per-frame pass timers: (name, microseconds), refreshed every frame
    pub timers:  Vec<(&'static str, u64)>,
}

/// Marker — identifies human agents in the ECS
#[derive(Component)]
pub(crate) struct HumanAgent;

/// Marker — agent is currently being carried by train N (drop-off rider during Arriving).
/// When present, agent_train_offset uses this train's motion for visual offset regardless of goal.
#[derive(Component)]
pub(crate) struct RidingTrain(pub u8);

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
pub const FLAG_WAITING:    u8 = 1 << 0;  // agent has reached/decided to wait, holding position
pub const FLAG_CROWD_JOIN: u8 = 1 << 1;  // committed via crowd-join (vs at_goal/stop_short)
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
    save_filepath: String,
    /// debug one-shot: when set, the next update_agents frame snapshots all current agents into
    /// waiting state at their current tiles, then clears the flag — new agents spawned afterwards
    /// are unaffected
    pub force_all_waiting: bool,
    /// debug toggle: when true, the "join nearby queue" self-decision is disabled —
    /// agents only commit via at_goal or stop_short
    pub disable_crowd_join: bool,
    /// last tile under the mouse cursor (set by update_tile_editor, displayed in UI)
    pub hover_tile: Option<(i32, i32, i32)>,
}

#[derive(Resource)]
pub struct Map {
    chunks: HashMap<(i32, i32, i32), MapChunk>,
    dirty: bool,
    flow_last_rebuild: std::time::Instant,
}

impl Map {
    fn new() -> Self {
        Self { chunks: HashMap::new(), dirty: true, flow_last_rebuild: std::time::Instant::now() }
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
pub fn update_flow_field(
    mut map: ResMut<Map>,
    mut soa: ResMut<AgentSoa>,
) -> Result<(), hotline_rs::Error> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    // first system in the schedule — clears the per-frame timer buffer for everyone else to append to
    soa.timers.clear();
    let _flow_t = std::time::Instant::now();
    if !map.dirty {
        soa.timers.push(("flow_field", 0));
        return Ok(());
    }
    map.dirty = false;

    const NEIGHBORS: [(i32, i32); 8] = [
        (1,0),(-1,0),(0,1),(0,-1),
        (1,1),(-1,1),(-1,-1),(1,-1)
    ];

    // pass 0: find max goal index across Platform, TrainBoarding and Train tiles, resize + zero all flow layers.
    // Goal-space layout: platforms [0..64), boarding [64..128), seats [128..192).
    let mut max_goal = 0u8;
    for chunk in map.chunks.values() {
        for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
            let idx = morton_encode(lx, ly, lz);
            match chunk.tiles[idx] {
                TileType::Platform      => max_goal = max_goal.max(chunk.index[idx]),
                TileType::TrainBoarding => max_goal = max_goal.max(chunk.index[idx].saturating_add(BOARDING_GOAL_OFFSET)),
                TileType::Train         => max_goal = max_goal.max(chunk.index[idx].saturating_add(TRAIN_GOAL_OFFSET)),
                TileType::Entrance      => max_goal = max_goal.max(chunk.index[idx].saturating_add(ENTRANCE_GOAL_OFFSET)),
                _ => {}
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

        let is_boarding_goal = goal_idx >= BOARDING_GOAL_OFFSET && goal_idx < TRAIN_GOAL_OFFSET;
        let is_seat_goal     = goal_idx >= TRAIN_GOAL_OFFSET    && goal_idx < ENTRANCE_GOAL_OFFSET;
        let is_entrance_goal = goal_idx >= ENTRANCE_GOAL_OFFSET;
        let train_id    = if is_seat_goal     { goal_idx - TRAIN_GOAL_OFFSET }
                          else if is_boarding_goal { goal_idx - BOARDING_GOAL_OFFSET }
                          else { 0 };
        let entrance_id = if is_entrance_goal { goal_idx - ENTRANCE_GOAL_OFFSET } else { 0 };

        // pass 1.5 (seat goals only): BFS from TrainBoarding tiles of this train_id through walkable
        // tiles, recording depth. Seat seed cost is then biased so deeper Train tiles win.
        let (depth_map, max_depth) = if is_seat_goal {
            use std::collections::VecDeque;
            let mut q: VecDeque<(i32, i32, i32)> = VecDeque::new();
            let mut d: HashMap<(i32, i32, i32), u32> = HashMap::new();
            for (&(cx, cy, cz), chunk) in &map.chunks {
                let cs = CHUNK_SIZE as i32;
                for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
                    let idx = morton_encode(lx, ly, lz);
                    if chunk.tiles[idx] == TileType::TrainBoarding && chunk.index[idx] == train_id {
                        let p = (cx*cs + lx as i32, cy*cs + ly as i32, cz*cs + lz as i32);
                        d.insert(p, 0);
                        q.push_back(p);
                    }
                }}}
            }
            let mut mx = 0u32;
            while let Some((tx, ty, tz)) = q.pop_front() {
                let dist = d[&(tx, ty, tz)];
                mx = mx.max(dist);
                let cur_tile = map.get_tile(Vec3i::new(tx, ty, tz));
                for (dx, dz) in NEIGHBORS {
                    let (nx, nz) = (tx + dx, tz + dz);
                    let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(nx, ty, nz);
                    if let Some(c) = map.chunks.get(&(cx, cy, cz)) {
                        let nidx = morton_encode(lx, ly, lz);
                        let nt = c.tiles[nidx];
                        if !is_walkable(nt) { continue; }
                        let blocked = (cur_tile == TileType::Platform && nt == TileType::Train)
                                   || (cur_tile == TileType::Train    && nt == TileType::Platform);
                        if blocked { continue; }
                        if d.contains_key(&(nx, ty, nz)) { continue; }
                        d.insert((nx, ty, nz), dist + 1);
                        q.push_back((nx, ty, nz));
                    }
                }
            }
            (d, mx)
        } else { (HashMap::new(), 0) };

        // pass 1: seed heap. Tile type selected by goal range; cost mixes density and (seats only) depth bias.
        // If all candidate tiles exceed the density threshold, double it and retry.
        const PLATFORM_CAPACITY: f32 = 3.0;
        const DENSITY_COST_SCALE: u32 = 5;
        let mut threshold = PLATFORM_CAPACITY;
        loop {
            for (&(cx, cy, cz), chunk) in &map.chunks {
                let cs = CHUNK_SIZE as i32;
                for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
                    let idx = morton_encode(lx, ly, lz);
                    let seeds = if is_entrance_goal {
                        chunk.tiles[idx] == TileType::Entrance && chunk.index[idx] == entrance_id
                    } else if is_seat_goal {
                        chunk.tiles[idx] == TileType::Train && chunk.index[idx] == train_id
                    } else if is_boarding_goal {
                        chunk.tiles[idx] == TileType::TrainBoarding && chunk.index[idx] == train_id
                    } else {
                        chunk.tiles[idx] == TileType::Platform && chunk.index[idx] == goal_idx
                    };
                    if seeds {
                        let density = chunk.density[idx];
                        if density >= threshold { continue; }
                        let (tx, ty, tz) = (cx*cs + lx as i32, cy*cs + ly as i32, cz*cs + lz as i32);
                        let base = (density as u32) * DENSITY_COST_SCALE;
                        let seed_cost = if is_seat_goal {
                            let dep = depth_map.get(&(tx, ty, tz)).copied().unwrap_or(0);
                            base + (max_depth - dep) * DEPTH_BIAS_SCALE
                        } else { base };
                        cost.insert((tx, ty, tz), seed_cost);
                        heap.push((Reverse(seed_cost), tx, ty, tz));
                    }
                }}}
            }
            if !heap.is_empty() { break; }
            threshold *= 2.0;
            if threshold > 1024.0 { break; } // no candidate tiles at all
        }
        if heap.is_empty() { continue; }

        // pass 2: Dijkstra flood fill (XZ neighbors only).
        // Constraint: agents must enter the train through TrainBoarding. Direct Platform↔Train
        // transitions are forbidden in any goal layer, forcing the flow through door tiles.
        // For *non-seat* goals (platforms, entrances) we also heavily penalize traversing Train
        // tiles, so drop-off passengers exiting head straight for the nearest TrainBoarding door
        // instead of cutting diagonally across the carriage interior.
        const TRAIN_TRAVERSE_PENALTY: u32 = 50;
        while let Some((Reverse(c), tx, ty, tz)) = heap.pop() {
            if cost.get(&(tx, ty, tz)).copied().unwrap_or(u32::MAX) < c { continue; }
            let cur_tile = map.get_tile(Vec3i::new(tx, ty, tz));
            for (dx, dz) in NEIGHBORS {
                let (nx, nz) = (tx + dx, tz + dz);
                let (cx, cy, cz, _, _, _) = tile_to_chunk(nx, ty, nz);
                if !map.chunks.contains_key(&(cx, cy, cz)) { continue; }
                let tile = map.get_tile(Vec3i::new(nx, ty, nz));
                if !is_walkable(tile) { continue; }
                let blocked_transition =
                    (cur_tile == TileType::Platform && tile == TileType::Train) ||
                    (cur_tile == TileType::Train    && tile == TileType::Platform);
                if blocked_transition { continue; }
                let penalty = if tile == TileType::Train && !is_seat_goal {
                    TRAIN_TRAVERSE_PENALTY
                } else { 0 };
                let new_cost = c + 1 + penalty;
                if new_cost < cost.get(&(nx, ty, nz)).copied().unwrap_or(u32::MAX) {
                    cost.insert((nx, ty, nz), new_cost);
                    heap.push((Reverse(new_cost), nx, ty, nz));
                }
            }
        }

        // pass 3: compute flow direction (gradient toward lowest-cost neighbour).
        // Apply the same wall constraint as pass 2 — flow vectors must never point through a
        // forbidden Train↔Platform edge. Otherwise pass 2 stops the *cost* from propagating
        // through the wall, but pass 3 still happily picks the cheap Platform neighbour and the
        // agent walks straight through what should be a train side panel.
        let flow_updates: Vec<(i32, i32, i32, Vec2f, f32)> = cost.iter()
            .map(|(&(tx, ty, tz), &c)| {
                let mut best_dir = Vec2f::zero();
                let mut best_cost = c;
                let cur_tile = map.get_tile(Vec3i::new(tx, ty, tz));
                for (dx, dz) in NEIGHBORS {
                    let neighbor_tile = map.get_tile(Vec3i::new(tx+dx, ty, tz+dz));
                    let blocked =
                        (cur_tile == TileType::Platform && neighbor_tile == TileType::Train) ||
                        (cur_tile == TileType::Train    && neighbor_tile == TileType::Platform);
                    if blocked { continue; }
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

    soa.timers.push(("flow_field", _flow_t.elapsed().as_micros() as u64));
    Ok(())
}

/// Processes train door open/close events: swaps agent goals between Goal(N) and Goal(N + TRAIN_GOAL_OFFSET).
/// On close, agents physically inside the train footprint keep their train goal; outsiders revert to platform.
#[export_update_fn]
pub fn update_train_boarding(
    mut commands: Commands,
    mut trains: ResMut<Trains>,
    mut map: ResMut<Map>,
    mut soa: ResMut<AgentSoa>,
    mut agents: Query<(Entity, &AgentPos, &mut Goal, &mut Flags), With<HumanAgent>>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    if trains.pending.is_empty() {
        soa.timers.push(("train_boarding", _t.elapsed().as_micros() as u64));
        return Ok(());
    }
    let events: Vec<(u8, bool)> = trains.pending.drain(..).collect();
    for (train_id, target_open) in events {
        let boarding_goal = train_id.saturating_add(BOARDING_GOAL_OFFSET);
        let seat_goal     = train_id.saturating_add(TRAIN_GOAL_OFFSET);
        if target_open {
            // open: platform-queued agents head for the boarding (door) tiles first.
            // The auto-advance in p2_bake takes over once they reach a TrainBoarding tile.
            for (_entity, _pos, mut goal, mut flags) in agents.iter_mut() {
                if goal.0 == train_id {
                    goal.0  = boarding_goal;
                    flags.0 = 0;
                }
            }
        } else {
            // close: agents whose goal is boarding OR seat for this train stay if they're physically inside
            // the train footprint (Train or TrainBoarding tile with matching index). Inside agents get the
            // RidingTrain marker (which is what gives them the visual offset when the train moves). Outsiders
            // revert to the platform goal.
            for (entity, pos, mut goal, mut flags) in agents.iter_mut() {
                if goal.0 != boarding_goal && goal.0 != seat_goal { continue; }
                let tile = world_to_tile(pos.0);
                let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
                let inside = map.chunks.get(&(cx, cy, cz)).map_or(false, |c| {
                    let mi = morton_encode(lx, ly, lz);
                    matches!(c.tiles[mi], TileType::Train | TileType::TrainBoarding) && c.index[mi] == train_id
                });
                if inside {
                    commands.entity(entity).insert(RidingTrain(train_id));
                } else {
                    goal.0  = train_id;
                    flags.0 = 0;
                }
            }
        }
        trains.doors_open.insert(train_id, target_open);
    }
    map.dirty = true; // force flow rebuild with the new active goal layers
    soa.timers.push(("train_boarding", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Advances each train's leave/arrive animation. On Leaving→Gone, bakes the visual offset into the
/// agent's logical position + wait_pos and clears their goal (so they no longer track the train).
/// Render offset is applied elsewhere (update_agent_transforms / tile draw).
#[export_update_fn]
pub fn update_train_motion(
    mut commands: Commands,
    mut trains:   ResMut<Trains>,
    map:          Res<Map>,
    mut soa:      ResMut<AgentSoa>,
    mut agents:   Query<(Entity, &Goal, Option<&RidingTrain>, &mut Flags), With<HumanAgent>>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    // 1. consume button presses → state transitions
    let cmds: Vec<(u8, bool)> = trains.pending_motion.drain(..).collect();
    for (tid, leave) in cmds {
        let entry = trains.motion.entry(tid).or_insert_with(TrainMotion::default);
        if leave && matches!(entry.state, TrainState::AtStation) {
            entry.axis    = infer_train_axis(&map, tid);
            entry.state   = TrainState::Leaving;
            entry.started = std::time::Instant::now();
        } else if !leave && matches!(entry.state, TrainState::Gone) {
            entry.state   = TrainState::Arriving;
            entry.started = std::time::Instant::now();
        }
    }

    // 2. advance Leaving / Arriving; capture completions for post-processing
    let mut completed_leaving:  Vec<u8> = Vec::new();
    let mut completed_arriving: Vec<u8> = Vec::new();
    for (&tid, motion) in trains.motion.iter_mut() {
        match motion.state {
            TrainState::Leaving if train_motion_progress(motion) >= 1.0 => {
                motion.state   = TrainState::Gone;
                motion.started = std::time::Instant::now();
                completed_leaving.push(tid);
            }
            TrainState::Arriving if train_motion_progress(motion) >= 1.0 => {
                motion.state   = TrainState::AtStation;
                motion.started = std::time::Instant::now();
                completed_arriving.push(tid);
            }
            _ => {}
        }
    }

    // 3. on Leaving→Gone: despawn every agent on this train — either by goal (regular boarders)
    // or by RidingTrain marker (drop-off stragglers who didn't leave the train before it departed).
    for tid in completed_leaving {
        let boarding = tid.saturating_add(BOARDING_GOAL_OFFSET);
        let seat     = tid.saturating_add(TRAIN_GOAL_OFFSET);
        for (entity, goal, riding, _flags) in agents.iter() {
            let by_goal   = goal.0 == boarding || goal.0 == seat;
            let by_marker = riding.map_or(false, |r| r.0 == tid);
            if by_goal || by_marker {
                commands.entity(entity).despawn();
            }
        }
    }

    // 4. on Arriving→AtStation: release drop-off riders. Remove RidingTrain marker, clear sticky
    // FLAG_WAITING so they start flowing toward their actual destination (entrance / other platform).
    for tid in completed_arriving {
        for (entity, _goal, riding, mut flags) in agents.iter_mut() {
            if riding.map_or(false, |r| r.0 == tid) {
                commands.entity(entity).remove::<RidingTrain>();
                flags.0 = 0;
            }
        }
    }

    soa.timers.push(("train_motion", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Drives the auto-cycle state machine per train. Each phase has a fixed duration; when it elapses
/// the appropriate event is pushed onto Trains.pending* queues — same channel the manual buttons use.
#[export_update_fn]
pub fn update_train_cycle(
    mut trains: ResMut<Trains>,
    mut soa:    ResMut<AgentSoa>,
    map:        Res<Map>,
    agents:     Query<(&AgentPos, &Goal, Option<&RidingTrain>), With<HumanAgent>>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    let now = std::time::Instant::now();

    // auto-init: any train (Train + TrainBoarding) that's never been seen gets cycle enabled
    // and starts in the depot (Gone), step Arrive — so it immediately slides in with drop-offs.
    let mut discovered: Vec<u8> = collect_indices(&map, TileType::Train);
    discovered.extend(collect_indices(&map, TileType::TrainBoarding));
    discovered.sort_unstable();
    discovered.dedup();
    for tid in discovered {
        if !trains.cycle.contains_key(&tid) {
            let axis = infer_train_axis(&map, tid);
            trains.cycle.insert(tid, TrainCycle {
                enabled:        true,
                step:           CycleStep::Arrive,
                step_started:   now,
                dropoff_emitted: false,
            });
            trains.motion.insert(tid, TrainMotion {
                state:   TrainState::Gone,
                started: now,
                axis,
            });
        }
    }

    // Count drop-off passengers still aboard each train. A drop-off is considered "aboard" if
    // either it still has its RidingTrain marker (mid-Arriving) OR it's physically on a Train /
    // TrainBoarding tile but its goal isn't this train's boarding/seat layer (i.e. it's a released
    // drop-off heading somewhere else, walking out). When this hits zero the train is ready to
    // open doors for the next round of boarders.
    let mut on_train: HashMap<u8, u32> = HashMap::new();
    for (pos, goal, riding) in agents.iter() {
        let tile = world_to_tile(pos.0);
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        if let Some(c) = map.chunks.get(&(cx, cy, cz)) {
            let mi = morton_encode(lx, ly, lz);
            if matches!(c.tiles[mi], TileType::Train | TileType::TrainBoarding) {
                let tid = c.index[mi];
                let board_goal = tid.saturating_add(BOARDING_GOAL_OFFSET);
                let seat_goal  = tid.saturating_add(TRAIN_GOAL_OFFSET);
                let is_dropoff = riding.is_some() || (goal.0 != board_goal && goal.0 != seat_goal);
                if is_dropoff {
                    *on_train.entry(tid).or_insert(0) += 1;
                }
            }
        }
    }

    // collect train_ids first to avoid borrow issues while pushing to queues
    let ids: Vec<u8> = trains.cycle.iter().filter(|(_, c)| c.enabled).map(|(&k, _)| k).collect();
    for tid in ids {
        let cycle = trains.cycle.get(&tid).copied().unwrap_or_default();
        if !cycle.enabled { continue; }
        let elapsed = now.duration_since(cycle.step_started).as_secs_f32();
        let (next_step, action, duration) = match cycle.step {
            CycleStep::ArriveDwell => (CycleStep::OpenDoors, None,                                   CYCLE_DOOR_DWELL),
            CycleStep::OpenDoors   => (CycleStep::Boarding,  Some(("doors_open",  true)),           CYCLE_DOOR_DWELL),
            CycleStep::Boarding    => (CycleStep::CloseDoors, None,                                  CYCLE_BOARDING_SECS),
            CycleStep::CloseDoors  => (CycleStep::Leave,     Some(("doors_open",  false)),          CYCLE_DOOR_DWELL),
            CycleStep::Leave       => (CycleStep::GoneDwell, Some(("motion",      true)),           TRAIN_TRANSIT_SECS + 0.1),
            CycleStep::GoneDwell   => (CycleStep::Arrive,    None,                                  CYCLE_GONE_SECS),
            CycleStep::Arrive      => (CycleStep::ArriveDwell, Some(("motion",    false)),          TRAIN_TRANSIT_SECS + 0.1),
        };
        // Gate ArriveDwell: stall until the train motion has actually arrived AND all drop-off
        // passengers have walked off the train. Min wait of CYCLE_DOOR_DWELL still applies.
        let in_arrive_dwell = matches!(cycle.step, CycleStep::ArriveDwell);
        let motion_arriving = trains.motion.get(&tid)
            .map(|m| matches!(m.state, TrainState::Arriving)).unwrap_or(false);
        let still_disembarking = in_arrive_dwell
            && (motion_arriving || on_train.get(&tid).copied().unwrap_or(0) > 0);
        if elapsed < duration || still_disembarking { continue; }
        // emit the action that closes the current step, then advance
        if let Some((channel, val)) = action {
            match channel {
                "doors_open" => trains.pending.push((tid, val)),
                "motion"     => trains.pending_motion.push((tid, val)),
                _ => {}
            }
        }
        let entering_arrive_dwell = matches!(next_step, CycleStep::ArriveDwell);
        let entering_leave        = matches!(next_step, CycleStep::Leave);
        let mut emit_dropoff = false;
        {
            let entry = trains.cycle.entry(tid).or_default();
            entry.step = next_step;
            entry.step_started = now;
            if entering_arrive_dwell && !entry.dropoff_emitted {
                emit_dropoff = true;
                entry.dropoff_emitted = true;
            }
            if entering_leave { entry.dropoff_emitted = false; }
        }
        if emit_dropoff {
            let n = train_dropoff_count(&trains, &map, tid);
            if n > 0 { trains.pending_dropoff.push((tid, n)); }
        }
    }
    soa.timers.push(("train_cycle", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Per-entrance auto-spawn: enqueues 1 spawn each interval when enabled.
#[export_update_fn]
pub fn update_entrance_auto_spawn(
    mut entrances: ResMut<Entrances>,
    mut soa: ResMut<AgentSoa>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    let now = std::time::Instant::now();
    // gather list to push without aliasing
    let mut to_spawn: Vec<u8> = Vec::new();
    for (&eid, auto) in entrances.auto.iter_mut() {
        if !auto.enabled { continue; }
        if now.duration_since(auto.last_spawn).as_secs_f32() >= auto.interval_s {
            auto.last_spawn = now;
            to_spawn.push(eid);
        }
    }
    for eid in to_spawn { entrances.pending_spawn.push((eid, 1)); }
    soa.timers.push(("entr_auto_spawn", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Spawns agents at Entrance tiles when their pending_spawn queue has entries.
/// Each spawn picks a random Platform index (from those present in the map) as the goal.
#[export_update_fn]
pub fn update_entrance_spawn(
    mut commands:  Commands,
    mut entrances: ResMut<Entrances>,
    mut soa:       ResMut<AgentSoa>,
    map:           Res<Map>,
    cube_mesh:     Res<AgentMesh>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    if entrances.pending_spawn.is_empty() {
        soa.timers.push(("entr_spawn", _t.elapsed().as_micros() as u64));
        return Ok(());
    }
    let mut platforms = collect_indices(&map, TileType::Platform);
    // fallback: if no platforms painted, agents still spawn with goal 0 so they're visible
    if platforms.is_empty() { platforms.push(0); }
    let events: Vec<(u8, u32)> = entrances.pending_spawn.drain(..).collect();
    for (entrance_id, count) in events {
        let centres = collect_tile_centres(&map, TileType::Entrance, entrance_id);
        if centres.is_empty() { continue; }
        for _ in 0..count {
            entrances.seed = entrances.seed.wrapping_add(1);
            let r1 = rand_u32(entrances.seed);
            let r2 = rand_u32(entrances.seed.wrapping_add(0x9E37_79B9));
            let pos    = centres[(r1 as usize) % centres.len()];
            let goal   = platforms[(r2 as usize) % platforms.len()];
            let jitter = (rand_u32(entrances.seed.wrapping_add(7)) as f32 / u32::MAX as f32 - 0.5) * TILE_SIZE * 0.5;
            let jitter2= (rand_u32(entrances.seed.wrapping_add(11)) as f32 / u32::MAX as f32 - 0.5) * TILE_SIZE * 0.5;
            let ax = pos.x + jitter;
            let az = pos.z + jitter2;
            commands.spawn((
                HumanAgent,
                AgentPos(Vec3f::new(ax, 0.0, az)),
                SpeedScale(0.8 + pos_hash(ax, az) * 0.4),
                WanderAngle(pos_hash(az, ax) * std::f32::consts::TAU),
                SepForce(Vec2f::zero()),
                LocalDensity(0.0),
                FacingRot(Quatf::identity()),
                Goal(goal),
                Flags(0),
                MeshComponent(cube_mesh.0.clone()),
                Position(vec3f(ax, AGENT_BODY_HALF_HEIGHT, az)),
                Rotation(Quatf::identity()),
                Scale(vec3f(AGENT_BODY_HALF_WIDTH, AGENT_BODY_HALF_HEIGHT, AGENT_BODY_HALF_WIDTH)),
                WorldMatrix(Mat34f::identity()),
            ));
        }
    }
    soa.timers.push(("entr_spawn", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Spawns drop-off agents on a train's Train tiles. Each gets a random goal from
/// (all Entrance indices) ∪ (all Platform indices except this train's own).
#[export_update_fn]
pub fn update_train_dropoff(
    mut commands:  Commands,
    mut trains:    ResMut<Trains>,
    mut entrances: ResMut<Entrances>, // reuse the seed counter so randomness keeps advancing
    mut soa:       ResMut<AgentSoa>,
    map:           Res<Map>,
    cube_mesh:     Res<AgentMesh>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    if trains.pending_dropoff.is_empty() {
        soa.timers.push(("train_dropoff", _t.elapsed().as_micros() as u64));
        return Ok(());
    }
    let platforms = collect_indices(&map, TileType::Platform);
    let entrance_ids = collect_indices(&map, TileType::Entrance);
    let events: Vec<(u8, u32)> = trains.pending_dropoff.drain(..).collect();
    for (train_id, count) in events {
        let centres = collect_tile_centres(&map, TileType::Train, train_id);
        if centres.is_empty() { continue; }
        // build destination pool: entrances (as goal = id + offset) + platforms (excluding this train's id)
        let mut pool: Vec<u8> = Vec::new();
        for &p in &platforms { if p != train_id { pool.push(p); } }
        for &e in &entrance_ids { pool.push(e.saturating_add(ENTRANCE_GOAL_OFFSET)); }
        if pool.is_empty() { continue; }
        for _ in 0..count {
            entrances.seed = entrances.seed.wrapping_add(1);
            let r1 = rand_u32(entrances.seed);
            let r2 = rand_u32(entrances.seed.wrapping_add(0x9E37_79B9));
            let pos  = centres[(r1 as usize) % centres.len()];
            let goal = pool[(r2 as usize) % pool.len()];
            let jitter = (rand_u32(entrances.seed.wrapping_add(7)) as f32 / u32::MAX as f32 - 0.5) * TILE_SIZE * 0.5;
            let jitter2= (rand_u32(entrances.seed.wrapping_add(11)) as f32 / u32::MAX as f32 - 0.5) * TILE_SIZE * 0.5;
            let ax = pos.x + jitter;
            let az = pos.z + jitter2;
            let entity = commands.spawn((
                HumanAgent,
                AgentPos(Vec3f::new(ax, 0.0, az)),
                SpeedScale(0.8 + pos_hash(ax, az) * 0.4),
                WanderAngle(pos_hash(az, ax) * std::f32::consts::TAU),
                SepForce(Vec2f::zero()),
                LocalDensity(0.0),
                FacingRot(Quatf::identity()),
                Goal(goal),
                Flags(FLAG_WAITING), // sticky-waits at spawn during transit; cleared on arrival
                RidingTrain(train_id),
                MeshComponent(cube_mesh.0.clone()),
                Position(vec3f(ax, AGENT_BODY_HALF_HEIGHT, az)),
                Rotation(Quatf::identity()),
                Scale(vec3f(AGENT_BODY_HALF_WIDTH, AGENT_BODY_HALF_HEIGHT, AGENT_BODY_HALF_WIDTH)),
                WorldMatrix(Mat34f::identity()),
            )).id();
            // ensure SoA wait_pos buffer is big enough and stores the actual spawn position
            // so the pull force holds the agent at the train tile instead of dragging toward origin
            let ia = entity.index() as usize;
            if soa.wait_pos.len() <= ia { soa.wait_pos.resize(ia + 1, Vec3f::zero()); }
            soa.wait_pos[ia] = Vec3f::new(ax, 0.0, az);
        }
    }
    soa.timers.push(("train_dropoff", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Despawns agents that have reached their entrance goal — i.e. they're on an Entrance tile
/// whose index matches their goal-offset value.
#[export_update_fn]
pub fn update_entrance_despawn(
    mut commands: Commands,
    mut soa:      ResMut<AgentSoa>,
    map:          Res<Map>,
    agents:       Query<(Entity, &AgentPos, &Goal), With<HumanAgent>>,
) -> Result<(), hotline_rs::Error> {
    let _t = std::time::Instant::now();
    for (entity, pos, goal) in agents.iter() {
        if goal.0 < ENTRANCE_GOAL_OFFSET { continue; }
        let want = goal.0.saturating_sub(ENTRANCE_GOAL_OFFSET);
        let tile = world_to_tile(pos.0);
        let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tile.x, tile.y, tile.z);
        if let Some(c) = map.chunks.get(&(cx, cy, cz)) {
            let i = morton_encode(lx, ly, lz);
            if c.tiles[i] == TileType::Entrance && c.index[i] == want {
                commands.entity(entity).despawn();
            }
        }
    }
    soa.timers.push(("entr_despawn", _t.elapsed().as_micros() as u64));
    Ok(())
}

/// Moves agents along the flow field with per-cell peer-repulsion spreading
#[export_update_fn]
pub fn update_agents(
    mut agents: Query<(Entity, &mut AgentPos, &SpeedScale, &mut WanderAngle, &mut SepForce, &mut LocalDensity, &mut FacingRot, &mut Goal, &mut Flags), With<HumanAgent>>,
    mut map: ResMut<Map>,
    mut soa: ResMut<AgentSoa>,
    mut editor: ResMut<EditorState>,
) -> Result<(), hotline_rs::Error> {
    let force_all_waiting = editor.force_all_waiting;
    let disable_crowd_join = editor.disable_crowd_join;
    editor.force_all_waiting = false;

    // reborrow as raw ref so NLL can split field borrows (ResMut treats whole struct as one borrow)
    let soa: &mut AgentSoa = &mut *soa;
    // (timers cleared by update_flow_field at start of frame)

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
    if soa.wait_pos.len() <= max_idx { soa.wait_pos.resize(max_idx + 1, Vec3f::zero()); }
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
    let wait_pos_ptr = soa.wait_pos.as_mut_ptr() as usize;

    let n = cells.len();
    let mut baked_density   = vec![0.0f32;        n];
    let mut baked_flow      = vec![Vec2f::zero();  n];
    let mut baked_flags     = vec![0u8;            n]; // FLAG_WAITING
    let mut baked_goal_swap = vec![0u8;            n]; // 0 = no swap; otherwise = new goal (>= TRAIN_GOAL_OFFSET so 0 is unambiguous)
    let mut nbr_start       = vec![0usize;     n + 1];
    let mut nbr_flat        = Vec::<usize>::new();

    for (ci, &(ia, ck, idx)) in cells.iter().enumerate() {
        let chunk    = &map.chunks[&ck];
        let goal     = gb[ia] as usize;
        let density  = chunk.density[idx];
        let flow_dir = if goal < chunk.flow.len() { chunk.flow[goal][idx] } else { Vec2f::zero() };

        baked_density[ci] = density;
        baked_flow[ci]    = flow_dir;

        // gather all neighbour agents (3x3 in XZ) into nbr_flat; also peak-density of surrounding tiles
        let tile = world_to_tile(pb[ia]);
        let mut max_nbr_density = 0.0f32;
        for dz in -1i32..=1 { for dx in -1i32..=1 {
            let (ncx, ncy, ncz, nlx, nly, nlz) = tile_to_chunk(tile.x+dx, tile.y, tile.z+dz);
            let nidx = morton_encode(nlx, nly, nlz);
            if let Some(nc) = map.chunks.get(&(ncx, ncy, ncz)) {
                nbr_flat.extend(nc.agents[nidx].iter().map(|&e| e.index() as usize));
                if dx != 0 || dz != 0 {
                    max_nbr_density = max_nbr_density.max(nc.density[nidx]);
                }
            }
        }}
        nbr_start[ci + 1] = nbr_flat.len();

        // waiting flag: sticky — once an agent reaches (or stops short of) a goal it stays in queue mode.
        // First arrival assigns a Halton-sampled wait_pos on the current tile; the pull keeps them near it.
        // TrainBoarding is a *staging* sink: reaching it auto-advances goal to the seat layer (no waiting commit here).
        let mut at_goal = match chunk.tiles[idx] {
            TileType::Platform => chunk.index[idx] == gb[ia],
            TileType::Train    => chunk.index[idx].saturating_add(TRAIN_GOAL_OFFSET) == gb[ia],
            TileType::Entrance => chunk.index[idx].saturating_add(ENTRANCE_GOAL_OFFSET) == gb[ia],
            _ => false,
        };
        if chunk.tiles[idx] == TileType::TrainBoarding
            && chunk.index[idx].saturating_add(BOARDING_GOAL_OFFSET) == gb[ia] {
            // Boarding tile reached. Allocate a seat only if a neighbouring Train tile (same train_id)
            // has room. Otherwise wait here — TrainBoarding acts as standing-room overflow with the
            // same Halton/density mechanics as the platform queue.
            let train_id = chunk.index[idx];
            let mut seat_open = false;
            'check: for dz in -1i32..=1 { for dx in -1i32..=1 {
                if dx == 0 && dz == 0 { continue; }
                let (ncx, ncy, ncz, nlx, nly, nlz) = tile_to_chunk(tile.x + dx, tile.y, tile.z + dz);
                if let Some(nc) = map.chunks.get(&(ncx, ncy, ncz)) {
                    let nidx = morton_encode(nlx, nly, nlz);
                    if nc.tiles[nidx] == TileType::Train
                        && nc.index[nidx] == train_id
                        && nc.density[nidx] < WAIT_SLOTS_PER_TILE as f32 {
                        seat_open = true;
                        break 'check;
                    }
                }
            }}
            if seat_open {
                baked_goal_swap[ci] = train_id.saturating_add(TRAIN_GOAL_OFFSET);
                at_goal = false;
            } else {
                at_goal = true; // overflow: wait on the boarding tile
            }
        }

        // Stop one tile short of the goal — some agents, scaled by goal crowding.
        // Per-entity roll is deterministic so each agent has a stable disposition;
        // crowd ramps 0→1 between 50% and 100% goal density, no early bail-outs below that.
        let stop_short = if !at_goal && (flow_dir.x != 0.0 || flow_dir.y != 0.0) {
            let ax = tile.x + flow_dir.x as i32;
            let az = tile.z + flow_dir.y as i32;
            let (acx, acy, acz, alx, aly, alz) = tile_to_chunk(ax, tile.y, az);
            if let Some(ac) = map.chunks.get(&(acx, acy, acz)) {
                let aidx = morton_encode(alx, aly, alz);
                let ahead_is_goal = match ac.tiles[aidx] {
                    TileType::Platform => ac.index[aidx] == gb[ia],
                    TileType::Train    => ac.index[aidx].saturating_add(TRAIN_GOAL_OFFSET) == gb[ia],
                    // TrainBoarding is a transient stage, not a stop_short target
                    _ => false,
                };
                if ahead_is_goal {
                    let crowd = ((ac.density[aidx] / WAIT_SLOTS_PER_TILE as f32) - 0.5).max(0.0) * 2.0;
                    let roll = ((ia as u32).wrapping_mul(2654435761) >> 24) as f32 / 255.0;
                    roll < crowd
                } else { false }
            } else { false }
        } else { false };

        // "I'm next to a packed queue for my goal" — each agent has a stable per-entity tolerance
        // between MIN and MAX. Low-tolerance agents commit early (nbr density >= 15), high-tolerance
        // agents push on until the queue is truly saturated (>= 30). Same-goal-waiting check
        // confirms it's an actual queue, not a transient cluster.
        let tolerance_roll = ((ia as u32).wrapping_mul(2246822519) >> 24) as f32 / 255.0;
        let tolerance = JOIN_CROWD_MIN_DENSITY + (JOIN_CROWD_MAX_DENSITY - JOIN_CROWD_MIN_DENSITY) * tolerance_roll;
        let join_crowd = !disable_crowd_join
            && max_nbr_density >= tolerance
            && nbr_flat[nbr_start[ci]..nbr_start[ci+1]].iter().any(|&ib| {
                ib != ia
                    && ib < fbp_len
                    && unsafe { *(fbp as *const u8).add(ib) } & FLAG_WAITING != 0
                    && gb[ib] == gb[ia]
            });

        let should_wait = at_goal || stop_short || join_crowd || force_all_waiting;
        let prev_flags = if ia < fbp_len { unsafe { *(fbp as *const u8).add(ia) } } else { 0u8 };
        let was_waiting = (prev_flags & FLAG_WAITING) != 0;
        // sticky FLAG_WAITING; FLAG_CROWD_JOIN sticks once set so the colour stays after they settle
        let mut bf = if should_wait || was_waiting { FLAG_WAITING } else { 0u8 };
        if (join_crowd && !was_waiting) || (prev_flags & FLAG_CROWD_JOIN) != 0 {
            bf |= FLAG_CROWD_JOIN;
        }
        baked_flags[ci] = bf;
        if should_wait && !was_waiting {
            use std::sync::atomic::Ordering;
            // bump current tile's counter; offset by per-tile hash so each tile uses a different halton phase
            let n = chunk.wait_slot[idx].fetch_add(1, Ordering::Relaxed) as u32;
            let slot = (n + tile_slot_offset(tile.x, tile.z)) % WAIT_SLOTS_PER_TILE;
            let (ox, oz) = halton_wait_offset(slot);
            let wp = vec3f(
                (tile.x as f32 + 0.5) * TILE_SIZE + ox,
                pb[ia].y,
                (tile.z as f32 + 0.5) * TILE_SIZE + oz,
            );
            unsafe { *(wait_pos_ptr as *mut Vec3f).add(ia) = wp; }
            // wrap counter explicitly so it never grows beyond u8 range over long sessions
            if n + 1 >= WAIT_SLOTS_PER_TILE {
                chunk.wait_slot[idx].store(((n + 1) % WAIT_SLOTS_PER_TILE) as u8, Ordering::Relaxed);
            }
        }
    }

    soa.timers.push(("p2 bake", t.elapsed().as_micros() as u64));

    // scatter goal-swap requests by entity index for the writeback loop
    let mut goal_swap_by_ia: Vec<u8> = vec![0; soa.pos.len()];
    let mut any_goal_swap = false;
    for (ci, &(ia, _, _)) in cells.iter().enumerate() {
        let s = baked_goal_swap[ci];
        if s != 0 && ia < goal_swap_by_ia.len() {
            goal_swap_by_ia[ia] = s;
            any_goal_swap = true;
        }
    }

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
        // sep applies to waiting agents too — others can push them off wait_pos; the pull force returns them
        unsafe {
            *(sp  as *mut Vec2f).add(ia) = sep_vel + wall_vel;
            *(dp  as *mut f32).add(ia) = cell_density;
            *(fgp as *mut u8).add(ia) = baked_flags[ci] & (FLAG_WAITING | FLAG_CROWD_JOIN);
        }
    });
    soa.timers.push(("p2 sep", t.elapsed().as_micros() as u64));

    // TODO; reduce useage of lengths and normalizes, sin cos?
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
    let wpb = soa.wait_pos.as_slice();
    cells.par_iter().enumerate().for_each(|(ci, &(ia, _, _))| {
        let pos_a = pb[ia];
        let avoid = vec3f(sep[ia].x, 0.0, sep[ia].y);
        let waiting = ia < fb_prev.len() && (fb_prev[ia] & FLAG_WAITING) != 0;
        let wander = wb[ia] + WANDER_DRIFT * (1.0 - (ia % 5) as f32 * 0.1);
        let wander_vel = vec3f(wander.cos(), 0.0, wander.sin()) * AGENT_SPEED * spd[ia] * WANDER_STRENGTH * if waiting { 0.0 } else { 1.0 };
        let flow = baked_flow[ci];
        let flow_norm = if length(flow) > 0.001 { normalize(flow) } else { flow };
        let flow_vel = vec3f(flow_norm.x, 0.0, flow_norm.y) * AGENT_SPEED * spd[ia] * if waiting { 0.0 } else { 1.0 };
        let pull_vel = if waiting {
            let wp = wpb[ia];
            vec3f(wp.x - pos_a.x, 0.0, wp.z - pos_a.z) * WAIT_PULL_STRENGTH
        } else { Vec3f::zero() };
        let total_vel = flow_vel + avoid + wander_vel + pull_vel;
        let new_pos = if waiting { pos_a + total_vel * WAIT_DAMPING } else { pos_a + total_vel };
        let new_facing = if waiting {
            // freeze facing once waiting — sep/pull jitter would otherwise spin the agent
            fb[ia]
        } else {
            let vel_len = length(vec3f(total_vel.x, 0.0, total_vel.z));
            if vel_len > 0.001 {
                let yaw = f32::atan2(total_vel.x, total_vel.z);
                slerp(fb[ia], Quatf::from_euler_angles(0.0, yaw, 0.0), TURN_RATE)
            } else { fb[ia] }
        };
        // SAFETY: ia is unique per entity
        unsafe {
            *(pp as *mut Vec3f).add(ia) = new_pos;
            *(wp as *mut f32).add(ia) = wander;
            *(fp as *mut Quatf).add(ia) = new_facing;
        }
    });
    soa.timers.push(("p3 move", t.elapsed().as_micros() as u64));

    //
    // Correction pass: resolve agent-agent overlaps
    // Re-register from pos_buf so neighborhoods reflect post-movement positions (fixes tile-boundary gaps).
    //
    let t = std::time::Instant::now();

    // TODO: separate agent buckets? world_to_tile in pass 3?
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

    // TODO: parallel? separate nbrs
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

    // periodic density-driven flow rebuild
    const FLOW_REBUILD_INTERVAL_SECS: f32 = 1.5;
    if map.flow_last_rebuild.elapsed().as_secs_f32() >= FLOW_REBUILD_INTERVAL_SECS {
        map.dirty = true;
        map.flow_last_rebuild = std::time::Instant::now();
    }

    // single batch ECS write — all flat buffers are final after p5
    for (entity, mut pos, _, mut wander, mut sf, mut ld, mut facing, mut goal, mut fl) in agents.iter_mut() {
        let ia = entity.index() as usize;
        pos.0    = soa.pos[ia];
        wander.0 = soa.wander[ia];
        sf.0     = soa.sep[ia];
        ld.0     = soa.density[ia];
        facing.0 = soa.facing[ia];
        fl.0     = soa.flags[ia];
        if ia < goal_swap_by_ia.len() {
            let s = goal_swap_by_ia[ia];
            if s != 0 {
                goal.0 = s;
                fl.0   = 0; // drop sticky waiting so they move toward the new seat goal
            }
        }
    }

    if any_goal_swap {
        // a boarding->seat swap happened this frame; force a flow rebuild so the new active layer is current
        map.dirty = true;
    }

    Ok(())
}

/// Syncs AgentPos and FacingRot to Position and Rotation for mesh rendering
#[export_update_fn]
pub fn update_agent_transforms(
    mut agents: Query<(&AgentPos, &FacingRot, &Goal, Option<&RidingTrain>, &mut Position, &mut Rotation), With<HumanAgent>>,
    trains: Res<Trains>,
) -> Result<(), hotline_rs::Error> {
    for (agent_pos, facing, goal, riding, mut position, mut rotation) in agents.iter_mut() {
        let offset = agent_train_offset(&trains, riding, goal.0);
        position.0 = vec3f(agent_pos.0.x, AGENT_BODY_HALF_HEIGHT, agent_pos.0.z) + offset;
        rotation.0 = facing.0;
    }
    Ok(())
}

/// Visual offset for an agent — applied only when the agent has an explicit RidingTrain marker.
/// Markers are placed by update_train_boarding on door-close for agents inside the train footprint,
/// and by update_train_dropoff on spawn for inbound drop-off riders. The goal value alone is not
/// enough: platform-queued agents transitioning Goal(N)→Goal(N+64) on door-open would otherwise
/// translate with a still-arriving train. The marker is the only authoritative "on the train" signal.
fn agent_train_offset(trains: &Trains, riding: Option<&RidingTrain>, _goal: u8) -> Vec3f {
    let r = match riding { Some(r) => r, None => return Vec3f::zero() };
    trains.motion.get(&r.0).map(train_visual_offset).unwrap_or(Vec3f::zero())
}

#[export_update_fn]
pub fn debug_draw_agents(
    agents: Query<(&AgentPos, &FacingRot, &Flags, &Goal, Option<&RidingTrain>), With<HumanAgent>>,
    trains: Res<Trains>,
    mut imdraw: ResMut<ImDrawRes>,
) -> Result<(), hotline_rs::Error> {
    const Y_OFFSET: f32 = 1.0;
    for (pos, facing, flags, goal, riding) in agents.iter() {
        let p = pos.0 + agent_train_offset(&trains, riding, goal.0);
        let base = vec3f(p.x, Y_OFFSET, p.z);
        // colour by state: red = moving, green = waiting via at_goal/stop_short, cyan = crowd-join
        let waiting    = (flags.0 & FLAG_WAITING)    != 0;
        let crowd_join = (flags.0 & FLAG_CROWD_JOIN) != 0;
        let body_col = if !waiting        { Vec4f::red() }
                       else if crowd_join { Vec4f::new(0.0, 1.0, 1.0, 1.0) }
                       else               { Vec4f::green() };
        imdraw.add_circle_3d_xz(base, AGENT_RADIUS, body_col);
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
            save_filepath: MAP_SAVE_PATH.to_string(),
            force_all_waiting: false,
            disable_crowd_join: false,
            hover_tile: None,
        });

    // create shared cube mesh for agent bodies
    let cube = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.insert_resource(AgentMesh(cube));
    commands.insert_resource(AgentSoa::default());
    commands.insert_resource(Trains::default());
    commands.insert_resource(Entrances::default());

    Ok(())
}

#[export_update_fn]
pub fn update_tile_editor_ui(
    mut commands: Commands,
    mut imgui: ResMut<ImGuiRes>,
    mut map: ResMut<Map>,
    mut editor: ResMut<EditorState>,
    mut trains: ResMut<Trains>,
    mut entrances: ResMut<Entrances>,
    agents: Query<Entity, With<HumanAgent>>,
    agent_flags: Query<&Flags, With<HumanAgent>>,
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
        if imgui.button(font_awesome::strs::SYNC) {
            if let Ok(mut reloaded) = Map::load(MAP_SAVE_PATH) {
                reloaded.dirty = true;
                *map = reloaded;
            } else {
                *map = Map::new();
            }
            for entity in agents.iter() {
                commands.entity(entity).despawn();
            }
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

        if imgui.button("Wait All (debug)") {
            editor.force_all_waiting = true;
        }
        let mut disable_join = editor.disable_crowd_join;
        if imgui.checkbox("Disable crowd-join", &mut disable_join) {
            editor.disable_crowd_join = disable_join;
        }

        imgui.separator();

        // hover-tile inspector
        if let Some((tx, ty, tz)) = editor.hover_tile {
            let (cx, cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
            imgui.text(&format!("Tile ({}, {}, {})", tx, ty, tz));
            if let Some(c) = map.chunks.get(&(cx, cy, cz)) {
                let mi = morton_encode(lx, ly, lz);
                let agents_here = c.agents[mi].len();
                let density    = c.density[mi];
                let pressure   = c.pressure[mi];
                let tile_kind  = c.tiles[mi];
                let index      = c.index[mi];
                let wait_slot  = c.wait_slot[mi].load(std::sync::atomic::Ordering::Relaxed);
                let goal       = editor.viz_goal as usize;
                let flow       = if goal < c.flow.len() { c.flow[goal][mi] } else { Vec2f::zero() };
                // count waiting agents in this tile
                let (mut waiting_n, mut crowd_n) = (0usize, 0usize);
                for &e in &c.agents[mi] {
                    if let Ok(f) = agent_flags.get(e) {
                        if (f.0 & FLAG_WAITING)    != 0 { waiting_n += 1; }
                        if (f.0 & FLAG_CROWD_JOIN) != 0 { crowd_n += 1; }
                    }
                }
                imgui.text(&format!("Type: {:?}  Index: {}", tile_kind, index));
                imgui.text(&format!("Agents: {}  Waiting: {}  Crowd-join: {}", agents_here, waiting_n, crowd_n));
                imgui.text(&format!("Density: {:.1}  Pressure: {:.1}  Wait slot: {}", density, pressure, wait_slot));
                imgui.text(&format!("Flow (g{}): ({:.2}, {:.2})", goal, flow.x, flow.y));
            } else {
                imgui.text("(no chunk)");
            }
            imgui.separator();
        }

        // Trains panel: list unique train indices (any Train or TrainBoarding tile contributes) and a button per train
        let mut train_ids: Vec<u8> = map.chunks.values()
            .flat_map(|c| c.tiles.iter().enumerate()
                .filter(|(_, &t)| matches!(t, TileType::Train | TileType::TrainBoarding))
                .map(|(i, _)| c.index[i]))
            .collect();
        train_ids.sort_unstable();
        train_ids.dedup();

        if !train_ids.is_empty() {
            imgui.text("Trains:");
            for tid in train_ids {
                let open  = trains.doors_open.get(&tid).copied().unwrap_or(false);
                let state = trains.motion.get(&tid).map(|m| m.state).unwrap_or(TrainState::AtStation);
                let state_label = match state {
                    TrainState::AtStation => if open { "at station, open" } else { "at station, closed" },
                    TrainState::Leaving   => "leaving...",
                    TrainState::Gone      => "gone",
                    TrainState::Arriving  => "arriving...",
                };
                imgui.text(&format!("Train {} [{}]", tid, state_label));
                // door buttons only when at station
                if matches!(state, TrainState::AtStation) {
                    let door_label = if open { format!("Close##{}", tid) } else { format!("Open##{}", tid) };
                    if imgui.button(&door_label) {
                        trains.pending.push((tid, !open));
                    }
                    imgui.same_line();
                    // leave only when doors closed
                    if !open {
                        if imgui.button(&format!("Leave##{}", tid)) {
                            trains.pending_motion.push((tid, true));
                        }
                    }
                }
                // arrive only when gone
                if matches!(state, TrainState::Gone) {
                    if imgui.button(&format!("Arrive##{}", tid)) {
                        trains.pending_motion.push((tid, false));
                    }
                }
                // drop off only at station — count scales with train capacity × density slider
                if matches!(state, TrainState::AtStation) {
                    imgui.same_line();
                    if imgui.button(&format!("Drop Off##{}", tid)) {
                        let n = train_dropoff_count(&trains, &map, tid);
                        if n > 0 { trains.pending_dropoff.push((tid, n)); }
                    }
                }
                // auto-cycle checkbox per train
                let mut auto = trains.cycle.get(&tid).map(|c| c.enabled).unwrap_or(false);
                if imgui.checkbox(&format!("Auto cycle##t{}", tid), &mut auto) {
                    let entry = trains.cycle.entry(tid).or_default();
                    entry.enabled = auto;
                    entry.step_started = std::time::Instant::now();
                    // when enabling, start from OpenDoors if at station, ArriveDwell otherwise
                    entry.step = if matches!(state, TrainState::AtStation) { CycleStep::OpenDoors }
                                 else                                       { CycleStep::ArriveDwell };
                    entry.dropoff_emitted = false;
                }
                // drop-off density slider (0..1) — gets multiplied by train capacity to set the spawn count.
                // Cap shown for clarity: "density × capacity = N agents".
                let cap = train_capacity(&map, tid);
                let mut density = trains.dropoff_density.get(&tid).copied().unwrap_or(DEFAULT_DROPOFF_DENSITY);
                if imgui.slider_float(&format!("Dropoff density##t{}", tid), &mut density, 0.0, 1.0) {
                    trains.dropoff_density.insert(tid, density);
                }
                imgui.same_line();
                imgui.text(&format!("({}/{} agents)", ((cap as f32) * density).round() as u32, cap));
            }
            imgui.separator();
        }

        // Entrances panel — Spawn button + Auto checkbox per entrance
        let entrance_ids = collect_indices(&map, TileType::Entrance);
        if !entrance_ids.is_empty() {
            imgui.text("Entrances:");
            for eid in entrance_ids {
                imgui.text(&format!("Entrance {}", eid));
                imgui.same_line();
                if imgui.button(&format!("Spawn##e{}", eid)) {
                    entrances.pending_spawn.push((eid, 10));
                }
                imgui.same_line();
                let mut auto = entrances.auto.get(&eid).map(|a| a.enabled).unwrap_or(false);
                if imgui.checkbox(&format!("Auto##e{}", eid), &mut auto) {
                    let entry = entrances.auto.entry(eid).or_default();
                    entry.enabled = auto;
                    entry.last_spawn = std::time::Instant::now();
                }
            }
            imgui.separator();
        }

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
    trains: Res<Trains>,
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
        Vec4f::new(1.0, 0.5, 0.0, 1.0),          // Train (orange)
        Vec4f::new(0.0, 0.7, 0.7, 1.0),          // TrainBoarding (teal)
        Vec4f::new(1.0, 0.2, 0.8, 1.0),          // Entrance (magenta)
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

                editor.hover_tile = Some((tile_x, 0, tile_z));

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

    // per-train bounds box — XZ AABB of all Train + TrainBoarding tiles with matching index.
    // Box translates with the train's motion offset (tile data itself stays put).
    {
        use std::collections::HashMap;
        let mut bounds: HashMap<u8, (f32, f32, f32, f32)> = HashMap::new(); // tid -> (min_x, min_z, max_x, max_z)
        for (&(cx, _, cz), chunk) in &map.chunks {
            let cs = CHUNK_SIZE as i32;
            for ly in 0..CHUNK_SIZE { for lz in 0..CHUNK_SIZE { for lx in 0..CHUNK_SIZE {
                let idx = morton_encode(lx, ly, lz);
                let t = chunk.tiles[idx];
                if !matches!(t, TileType::Train | TileType::TrainBoarding) { continue; }
                let tid = chunk.index[idx];
                let tx = (cx*cs + lx as i32) as f32 * TILE_SIZE;
                let tz = (cz*cs + lz as i32) as f32 * TILE_SIZE;
                let b = bounds.entry(tid).or_insert((tx, tz, tx + TILE_SIZE, tz + TILE_SIZE));
                b.0 = b.0.min(tx); b.1 = b.1.min(tz);
                b.2 = b.2.max(tx + TILE_SIZE); b.3 = b.3.max(tz + TILE_SIZE);
            }}}
        }
        const TRAIN_HEIGHT: f32 = 15.0; // 1.5 tiles tall
        let y_lo = Y_OFFSET;
        let y_hi = Y_OFFSET + TRAIN_HEIGHT;
        for (tid, (mnx, mnz, mxx, mxz)) in bounds {
            let offset = trains.motion.get(&tid).map(train_visual_offset).unwrap_or(Vec3f::zero());
            let (ox, oz) = (offset.x, offset.z);
            let col = Vec4f::new(1.0, 0.5, 0.0, 1.0); // orange to match train
            // 8 corners: l = lo Y, h = hi Y; 00/10/11/01 = (min,min)(max,min)(max,max)(min,max)
            let l00 = Vec3f::new(mnx + ox, y_lo, mnz + oz);
            let l10 = Vec3f::new(mxx + ox, y_lo, mnz + oz);
            let l11 = Vec3f::new(mxx + ox, y_lo, mxz + oz);
            let l01 = Vec3f::new(mnx + ox, y_lo, mxz + oz);
            let h00 = Vec3f::new(mnx + ox, y_hi, mnz + oz);
            let h10 = Vec3f::new(mxx + ox, y_hi, mnz + oz);
            let h11 = Vec3f::new(mxx + ox, y_hi, mxz + oz);
            let h01 = Vec3f::new(mnx + ox, y_hi, mxz + oz);
            // bottom face
            imdraw.add_line_3d(l00, l10, col);
            imdraw.add_line_3d(l10, l11, col);
            imdraw.add_line_3d(l11, l01, col);
            imdraw.add_line_3d(l01, l00, col);
            // top face
            imdraw.add_line_3d(h00, h10, col);
            imdraw.add_line_3d(h10, h11, col);
            imdraw.add_line_3d(h11, h01, col);
            imdraw.add_line_3d(h01, h00, col);
            // vertical edges
            imdraw.add_line_3d(l00, h00, col);
            imdraw.add_line_3d(l10, h10, col);
            imdraw.add_line_3d(l11, h11, col);
            imdraw.add_line_3d(l01, h01, col);
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
        // sum the frame and find the slowest single entry to scale the bars
        let total_us: u64 = soa.timers.iter().map(|(_, us)| *us).sum();
        let max_us:   u64 = soa.timers.iter().map(|(_, us)| *us).max().unwrap_or(1).max(1);

        imgui.text(&format!("Frame: {} us  ({:.2} ms)", total_us, total_us as f32 / 1000.0));
        imgui.text(&format!("Slowest entry: {} us", max_us));
        imgui.separator();

        // per-system bar — width proportional to that entry's share of max
        const BAR_WIDTH: usize = 40;
        for (name, us) in &soa.timers {
            let n   = ((*us as f32 / max_us as f32) * BAR_WIDTH as f32).round() as usize;
            let bar: String = "|".repeat(n.min(BAR_WIDTH));
            let col = if *us > max_us * 3 / 4 { Vec4f::new(1.0, 0.3, 0.3, 1.0) }      // red — hot
                      else if *us > max_us / 2 { Vec4f::new(1.0, 0.8, 0.3, 1.0) }     // amber
                      else                     { Vec4f::new(0.5, 0.9, 0.5, 1.0) };    // green
            imgui.colour_text(&format!("{:<18} {:>6} us  {}", name, us, bar), col);
        }
        imgui.separator();

        // sequential timeline: a single horizontal strip showing the proportions in order
        if total_us > 0 {
            const TIMELINE_WIDTH: usize = 60;
            let mut strip = String::with_capacity(TIMELINE_WIDTH);
            let mut acc_us = 0u64;
            for (i, (_, us)) in soa.timers.iter().enumerate() {
                acc_us += us;
                let end = ((acc_us as f32 / total_us as f32) * TIMELINE_WIDTH as f32).round() as usize;
                let segment_char = std::char::from_u32('A' as u32 + (i as u32 % 26)).unwrap_or('?');
                while strip.chars().count() < end.min(TIMELINE_WIDTH) {
                    strip.push(segment_char);
                }
            }
            imgui.text("Timeline (left=start, right=end, each letter=system):");
            imgui.text(&strip);
            imgui.text("Legend:");
            for (i, (name, us)) in soa.timers.iter().enumerate() {
                let c = std::char::from_u32('A' as u32 + (i as u32 % 26)).unwrap_or('?');
                imgui.text(&format!("  {} = {:<18} {:>6} us", c, name, us));
            }
        }
    }
    imgui.end();
    Ok(())
}
