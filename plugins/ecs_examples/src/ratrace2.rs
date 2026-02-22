//
/// Rat Race 2 - Tilemap Editor & Game
///

use crate::prelude::*;

use std::collections::HashMap;

const CHUNK_SIZE: usize = 8; // must be power-of-2 for morton encoding to produce dense indices
const TILE_SIZE: f32 = 10.0;
const GRID_SIZE: i32 = 2000;

#[derive(Clone, Copy, PartialEq)]
enum TileType {
    Empty,
    Wall,
    Barrier,
    Escalator,
    Platform,
    TrainTrack,
}

struct MapChunk {
    tiles: [TileType; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
}

impl MapChunk {
    fn new() -> Self {
        Self {
            tiles: [TileType::Empty; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
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

const TILE_NAMES: &[&str] = &["Wall", "Barrier", "Escalator", "Platform", "TrainTrack"];
const TILE_TYPES: &[TileType] = &[TileType::Wall, TileType::Barrier, TileType::Escalator, TileType::Platform, TileType::TrainTrack];

#[derive(Resource)]
struct EditorState {
    selected_tile: usize,
}

#[derive(Resource)]
struct Map {
    chunks: HashMap<u64, MapChunk>,
}

impl Map {
    fn new() -> Self {
        Self { chunks: HashMap::new() }
    }

    fn set_tile(&mut self, tx: i32, ty: i32, tz: i32, tile: TileType) {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
        let key = chunk_key(cx, cz);
        let chunk = self.chunks.entry(key).or_insert_with(MapChunk::new);
        chunk.tiles[morton_encode(lx, ly, lz)] = tile;
    }

    fn get_tile(&self, tx: i32, ty: i32, tz: i32) -> TileType {
        let (cx, _cy, cz, lx, ly, lz) = tile_to_chunk(tx, ty, tz);
        let key = chunk_key(cx, cz);
        match self.chunks.get(&key) {
            Some(chunk) => chunk.tiles[morton_encode(lx, ly, lz)],
            None => TileType::Empty,
        }
    }
}

/// Init function for ratrace demo
#[no_mangle]
pub fn ratrace2(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_ratrace2"
        ],
        update: systems![
            "update_tile_editor"
        ],
        render_graph: "mesh_wireframe_overlay"
    }
}

/// Sets up the ratrace game world
#[export_update_fn]
pub fn setup_ratrace2(
    mut session_info: ResMut<SessionInfo>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    // enable the grid
    session_info.debug_draw_flags |= DebugDrawFlags::GRID;

    // create resources
    commands.insert_resource(Map::new());
    commands.insert_resource(EditorState { selected_tile: 0 });

    Ok(())
}

/// Main update - handles mouse input and draws tilemap editor overlay
#[export_update_fn]
pub fn update_tile_editor(
    app: Res<AppRes>,
    viewport: Res<ViewportInfo>,
    pmfx: Res<PmfxRes>,
    mut imgui: ResMut<ImGuiRes>,
    mut imdraw: ResMut<ImDrawRes>,
    mut map: ResMut<Map>,
    mut editor: ResMut<EditorState>,
) -> Result<(), hotline_rs::Error> {

    // tile editor UI window
    let tile_names: Vec<String> = TILE_NAMES.iter().map(|s| s.to_string()).collect();
    let selected_name = TILE_NAMES[editor.selected_tile].to_string();

    /*
    imgui.begin_window("Tile Editor");
    let (_open, new_selected) = imgui.combo_list("Tile Type", &tile_names, &selected_name);
    if let Some(idx) = TILE_NAMES.iter().position(|&n| n == new_selected) {
        editor.selected_tile = idx;
    }
    imgui.text(&format!("Chunks: {}", map.chunks.len()));
    imgui.end();
    */

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
            let y = 0.01;
            let col = Vec4f::new(0.0, 1.0, 1.0, 1.0);
            imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, min_z), col);
            imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(max_x, y, max_z), col);
            imdraw.add_line_3d(Vec3f::new(max_x, y, max_z), Vec3f::new(min_x, y, max_z), col);
            imdraw.add_line_3d(Vec3f::new(min_x, y, max_z), Vec3f::new(min_x, y, min_z), col);

            // place / erase tiles
            if !app.is_sys_key_down(os::SysKey::Alt) {
                let buttons = app.get_mouse_buttons();
                if buttons[os::MouseButton::Left as usize] {
                    map.set_tile(tile_x, 0, tile_z, TILE_TYPES[editor.selected_tile]);
                }
                if buttons[os::MouseButton::Right as usize] {
                    map.set_tile(tile_x, 0, tile_z, TileType::Empty);
                }
            }
        }
    }

    // draw all placed tiles
    let wall_col = Vec4f::new(0.2, 0.6, 1.0, 1.0);
    let y = 0.02;
    for (key, chunk) in &map.chunks {
        let (cx, cz) = chunk_key_unpack(*key);
        for lz in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                let idx = morton_encode(lx, 0, lz);
                if chunk.tiles[idx] != TileType::Empty {
                    let tx = cx * CHUNK_SIZE as i32 + lx as i32;
                    let tz = cz * CHUNK_SIZE as i32 + lz as i32;

                    let min_x = tx as f32 * TILE_SIZE;
                    let min_z = tz as f32 * TILE_SIZE;
                    let max_x = min_x + TILE_SIZE;
                    let max_z = min_z + TILE_SIZE;

                    imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, min_z), wall_col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(max_x, y, max_z), wall_col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, max_z), Vec3f::new(min_x, y, max_z), wall_col);
                    imdraw.add_line_3d(Vec3f::new(min_x, y, max_z), Vec3f::new(min_x, y, min_z), wall_col);
                    imdraw.add_line_3d(Vec3f::new(min_x, y, min_z), Vec3f::new(max_x, y, max_z), wall_col);
                    imdraw.add_line_3d(Vec3f::new(max_x, y, min_z), Vec3f::new(min_x, y, max_z), wall_col);
                }
            }
        }
    }

    Ok(())
}
