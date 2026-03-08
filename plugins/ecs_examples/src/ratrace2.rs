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
    /// enum of tile type, currently were just thinking about walls and no walls
    tiles: [TileType; ARRAY_SIZE],
    /// 2D diffusion to flow downhill away from pressure
    flow: [Vec2f; ARRAY_SIZE],
    /// number agents per grid square
    density: [f32; ARRAY_SIZE],
    /// pressure curve based on density, which controls flow
    pressure: [f32; ARRAY_SIZE]
}

impl MapChunk {
    fn new() -> Self {
        Self {
            tiles: [TileType::Empty; ARRAY_SIZE],
            flow: [Vec2f::new(0.0, 1.0); ARRAY_SIZE],
            density: [0.0; ARRAY_SIZE],
            pressure: [0.0; ARRAY_SIZE]
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
const AGENT_SPEED: f32 = 0.1;   // world units per frame
const AGENT_RADIUS: f32 = 3.0;  // debug circle radius
const AGENT_SEGMENTS: usize = 8;

const TILE_TYPES: &[TileType] = &[
    TileType::Wall, 
    TileType::Barrier, 
    TileType::Escalator, 
    TileType::Platform, 
    TileType::TrainTrack
];

#[derive(Component)]
pub(crate) struct HumanAgent {
    pos: Vec3f,
}

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

    fn set_tile(&mut self, tx: i32, ty: i32, tz: i32, tile: TileType) {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
        let key = chunk_key(cx, cz);
        let chunk = self.chunks.entry(key).or_insert_with(MapChunk::new);
        chunk.tiles[morton_encode(lx, ly, lz)] = tile;
        self.dirty = true;
    }

    fn get_flow(&self, tx: i32, ty: i32, tz: i32) -> Vec2f {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
        let key = chunk_key(cx, cz);
        match self.chunks.get(&key) {
            Some(chunk) => chunk.flow[morton_encode(lx, ly, lz)],
            None => Vec2f::zero(),
        }
    }

    fn get_tile(&self, tx: i32, ty: i32, tz: i32) -> TileType {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
        let key = chunk_key(cx, cz);
        match self.chunks.get(&key) {
            Some(chunk) => chunk.tiles[morton_encode(lx, ly, lz)],
            None => TileType::Empty,
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
            let tile = map.get_tile(nx, 0, nz);
            if tile == TileType::Wall || tile == TileType::Barrier { continue; }
            let new_cost = c + 1;
            if new_cost < cost.get(&(nx, nz)).copied().unwrap_or(u32::MAX) {
                cost.insert((nx, nz), new_cost);
                heap.push((Reverse(new_cost), nx, nz));
            }
        }
    }

    // --- pass 3: compute flow direction (gradient toward lowest-cost neighbour) ---
    // collect results first to avoid simultaneous borrow of map
    let flow_updates: Vec<(i32, i32, Vec2f, f32)> = cost.iter()
        .filter(|(&(tx, tz), _)| map.get_tile(tx, 0, tz) != TileType::Platform)
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

/// Moves agents along the flow field and draws them as debug circles
#[export_update_fn]
pub fn update_agents(
    mut query: Query<&mut HumanAgent>,
    map: Res<Map>,
    mut imdraw: ResMut<ImDrawRes>,
) -> Result<(), hotline_rs::Error> {
    const Y_OFFSET: f32 = 1.0;
    let agent_col = Vec4f::new(1.0, 0.5, 0.0, 1.0); // orange

    for mut agent in query.iter_mut() {
        // derive tile coords from 3D position
        let tile_x = (agent.pos.x / TILE_SIZE).floor() as i32;
        let tile_z = (agent.pos.z / TILE_SIZE).floor() as i32;

        // read flow at current tile (Vec2f: x -> world X, y -> world Z)
        let flow = map.get_flow(tile_x, 0, tile_z);

        if flow.x != 0.0 || flow.y != 0.0 {
            let new_x = agent.pos.x + flow.x * AGENT_SPEED;
            let new_z = agent.pos.z + flow.y * AGENT_SPEED;
            let new_tile_x = (new_x / TILE_SIZE).floor() as i32;
            let new_tile_z = (new_z / TILE_SIZE).floor() as i32;
            // only move if destination tile is not a wall
            if map.get_tile(new_tile_x, 0, new_tile_z) == TileType::Empty {
                agent.pos.x = new_x;
                agent.pos.z = new_z;
            }
        }

        // draw agent as an octagon
        let pos = agent.pos;
        for i in 0..AGENT_SEGMENTS {
            let a0 = (i as f32 / AGENT_SEGMENTS as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32 / AGENT_SEGMENTS as f32) * std::f32::consts::TAU;
            let p0 = Vec3f::new(pos.x + a0.cos() * AGENT_RADIUS, Y_OFFSET, pos.z + a0.sin() * AGENT_RADIUS);
            let p1 = Vec3f::new(pos.x + a1.cos() * AGENT_RADIUS, Y_OFFSET, pos.z + a1.sin() * AGENT_RADIUS);
            imdraw.add_line_3d(p0, p1, agent_col);
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
    let camera = pmfx.get_camera_constants("main_camera")?;
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
                                    map.set_tile(tile_x, 0, tile_z, TILE_TYPES[i]);
                                }
                                PlaceMode::Agent => {
                                    if left_pressed && map.get_tile(tile_x, 0, tile_z) == TileType::Empty {
                                        commands.spawn(HumanAgent { pos: Vec3f::new(hit.x, 0.0, hit.z) });
                                    }
                                }
                            }
                        }
                    }
                    else {
                        if left_down {
                            map.set_tile(tile_x, 0, tile_z, TileType::Empty);
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

    // draw all placed tiles
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
                    let flow_dir = map.get_flow(tx, 0, tz);
                    let flow_dir = Vec3f::new(flow_dir.x, 0.0, flow_dir.y);
                    let flow_start = Vec3f::new(mid_x, y, mid_z);
                    let flow_col = Vec4f::from((flow_dir.xy() * 0.5 + 0.8, 0.0, 1.0));

                    let flow_end = flow_start + flow_dir;

                    imdraw.add_line_3d(flow_start, flow_end, flow_col);

                    let arrow_size = 0.2;
                    let perp = maths_rs::perp(flow_dir.xz()) * arrow_size;
                    let perp = Vec3f::new(perp.x, 0.0, perp.y);
                    let tip =  flow_end - flow_dir * arrow_size;

                    imdraw.add_line_3d(flow_end, tip + perp, flow_col);
                    imdraw.add_line_3d(flow_end, tip - perp, flow_col);
                }
            }
        }
    }

    Ok(())
}
