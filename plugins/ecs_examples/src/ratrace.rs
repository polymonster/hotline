// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Rat Race - Tube Station Flow Simulation Game
///

use crate::prelude::*;

/// Normalized lerp for quaternions - takes shortest path
fn quat_nlerp(a: Quatf, b: Quatf, t: f32) -> Quatf {
    let b = if Quatf::dot(a, b) < 0.0 { -b } else { b };
    Quatf::normalize(a * (1.0 - t) + b * t)
}

const AVOID_RADIUS: f32 = 15.0;
const AVOID_STRENGTH: f32 = 80.0;

// Barrier constants
const GATE_WIDTH: f32 = 12.0;
const GATE_PROCESS_TIME: f32 = 1.5;
const BARRIER_APPROACH_DIST: f32 = 20.0;
const QUEUE_SPACING: f32 = 8.0;
const WALL_HEIGHT: f32 = 6.0;
const BARRIER_Z: f32 = 0.0;

// Train / platform constants
const PLATFORM_X: [f32; 2] = [-80.0, 80.0];
const TRAIN_STOP_Z: f32 = 40.0;
const TRAIN_SPEED: f32 = 40.0;
const TRAIN_OFFSCREEN_Z: f32 = 300.0;
const TRAIN_STOP_DURATION: f32 = 5.0;
const TRAIN_SPAWN_INTERVAL: f32 = 8.0;
const TRAIN_MIN_PASSENGERS: usize = 1;
const TRAIN_MAX_PASSENGERS: usize = 4;
const AGENT_SPAWN_INTERVAL: f32 = 3.0;
const AGENT_SPAWN_X_RANGE: f32 = 40.0;

#[derive(Clone, Copy, PartialEq)]
enum AgentState {
    Moving,
    Queuing { barrier: usize, gate: usize },
    Processing { barrier: usize, gate: usize },
    WaitingForTrain { platform: usize },
    OnTrain { train_entity: Entity },
}

#[derive(Clone, Copy, PartialEq)]
enum PassDir {
    PosZ,
    NegZ,
}

/// Agent component - represents a person moving through the station
#[derive(Component)]
pub(crate) struct Agent {
    waypoints: Vec<Vec3f>,
    current_wp: usize,
    speed: f32,
    turn_speed: f32,
    state: AgentState,
    pass_dir: PassDir,
    target_platform: Option<usize>,
}

/// Marker for gate flap entities so we can animate them
#[derive(Component)]
pub(crate) struct GateFlap {
    barrier: usize,
    gate: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum TrainState {
    Arriving,
    Stopped,
    Departing,
}

/// Train component - rides along a platform track (one direction only, despawns at far end)
#[derive(Component)]
pub(crate) struct Train {
    platform: usize,
    state: TrainState,
    timer: f32,
    arrive_dir: f32,   // -1.0 or 1.0 (direction train comes FROM)
}

struct GateInfo {
    x: f32,
    dir: PassDir,
    queue: Vec<Entity>,
    processing: Option<Entity>,
    timer: f32,
    open: bool,
}

struct BarrierInfo {
    z: f32,
    gates: Vec<GateInfo>,
}

#[derive(Resource)]
pub(crate) struct BarrierRes {
    barriers: Vec<BarrierInfo>,
}

/// Mesh handles stored for runtime spawning of trains and agents
#[derive(Resource)]
pub(crate) struct GameMeshes {
    agent_mesh: pmfx::Mesh<gfx_platform::Device>,
    cube_mesh: pmfx::Mesh<gfx_platform::Device>,
    agent_facing: Quatf,
}

struct PlatformSpawnInfo {
    spawn_timer: f32,
    arrive_dir: f32,
}

/// Per-platform train spawn timing
#[derive(Resource)]
pub(crate) struct PlatformRes {
    platforms: Vec<PlatformSpawnInfo>,
}

/// Timer for spawning incoming agents outside the station
#[derive(Resource)]
pub(crate) struct AgentSpawnTimer {
    timer: f32,
}

/// Init function for ratrace demo
#[no_mangle]
pub fn ratrace(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_ratrace"
        ],
        update: systems![
            "update_ratrace"
        ],
        render_graph: "mesh_wireframe_overlay"
    }
}

/// Sets up the ratrace game world - spawns barriers and initial agents
#[no_mangle]
#[export_update_fn]
pub fn setup_ratrace(
    mut device: ResMut<DeviceRes>,
    mut session_info: ResMut<SessionInfo>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    session_info.debug_draw_flags |= DebugDrawFlags::GRID;

    let agent_mesh = hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true);
    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let facing = Quatf::from_euler_angles(f32::pi() * 0.5, 0.0, 0.0);

    // Store meshes for runtime spawning
    commands.insert_resource(GameMeshes {
        agent_mesh: agent_mesh.clone(),
        cube_mesh: cube_mesh.clone(),
        agent_facing: facing,
    });

    // --- Barrier ---
    let gate_positions: Vec<(f32, PassDir)> = vec![
        (-10.0, PassDir::PosZ),  // IN gate
        ( 10.0, PassDir::NegZ),  // OUT gate
    ];

    let barrier_res = BarrierRes {
        barriers: vec![
            BarrierInfo {
                z: BARRIER_Z,
                gates: gate_positions.iter().map(|&(x, dir)| GateInfo {
                    x, dir, queue: Vec::new(), processing: None, timer: 0.0, open: false,
                }).collect(),
            },
        ],
    };

    // Spawn barrier visuals
    for (bi, barrier) in barrier_res.barriers.iter().enumerate() {
        let z = barrier.z;
        let gate_xs: Vec<f32> = barrier.gates.iter().map(|g| g.x).collect();

        let mut edges: Vec<f32> = vec![-60.0];
        for &gx in &gate_xs {
            edges.push(gx - GATE_WIDTH * 0.5);
            edges.push(gx + GATE_WIDTH * 0.5);
        }
        edges.push(60.0);

        for chunk in edges.chunks(2) {
            let (x_min, x_max) = (chunk[0], chunk[1]);
            let width = x_max - x_min;
            if width < 0.1 { continue; }
            let center_x = (x_min + x_max) * 0.5;
            commands.spawn((
                MeshComponent(cube_mesh.clone()),
                Position(Vec3f::new(center_x, WALL_HEIGHT * 0.5, z)),
                Rotation(Quatf::identity()),
                Scale(Vec3f::new(width * 0.5, WALL_HEIGHT * 0.5, 0.5)),
                WorldMatrix(Mat34f::identity()),
            ));
        }

        for (gi, gate) in barrier.gates.iter().enumerate() {
            let gx = gate.x;
            // Left post
            commands.spawn((
                MeshComponent(cube_mesh.clone()),
                Position(Vec3f::new(gx - GATE_WIDTH * 0.5, WALL_HEIGHT * 0.5, z)),
                Rotation(Quatf::identity()),
                Scale(Vec3f::new(0.4, WALL_HEIGHT * 0.5, 0.6)),
                WorldMatrix(Mat34f::identity()),
            ));
            // Right post
            commands.spawn((
                MeshComponent(cube_mesh.clone()),
                Position(Vec3f::new(gx + GATE_WIDTH * 0.5, WALL_HEIGHT * 0.5, z)),
                Rotation(Quatf::identity()),
                Scale(Vec3f::new(0.4, WALL_HEIGHT * 0.5, 0.6)),
                WorldMatrix(Mat34f::identity()),
            ));
            // Gate flap
            commands.spawn((
                MeshComponent(cube_mesh.clone()),
                Position(Vec3f::new(gx, WALL_HEIGHT * 0.35, z)),
                Rotation(Quatf::identity()),
                Scale(Vec3f::new(GATE_WIDTH * 0.45, 0.3, 0.15)),
                WorldMatrix(Mat34f::identity()),
                GateFlap { barrier: bi, gate: gi },
            ));
        }
    }

    commands.insert_resource(barrier_res);

    // --- Platform spawn timers (trains spawn at runtime) ---
    let train_dirs: [f32; 2] = [1.0, -1.0]; // platform 0 from +z, platform 1 from -z
    commands.insert_resource(PlatformRes {
        platforms: train_dirs.iter().enumerate().map(|(pi, &dir)| PlatformSpawnInfo {
            spawn_timer: 1.0 + pi as f32 * 3.0, // stagger first arrivals
            arrive_dir: dir,
        }).collect(),
    });

    // Incoming agents spawn at runtime via timer
    commands.insert_resource(AgentSpawnTimer { timer: 0.5 });

    Ok(())
}

/// Updates trains, agents, and gate animations
#[no_mangle]
#[export_update_fn]
pub fn update_ratrace(
    time: Res<TimeRes>,
    mut commands: Commands,
    meshes: Res<GameMeshes>,
    mut platform_res: ResMut<PlatformRes>,
    mut barriers: ResMut<BarrierRes>,
    mut agent_query: Query<(Entity, &mut Position, &mut Rotation, &mut Agent), (Without<GateFlap>, Without<Train>)>,
    mut train_query: Query<(Entity, &mut Position, &mut Train), (Without<Agent>, Without<GateFlap>)>,
    mut flap_query: Query<(&GateFlap, &mut Rotation), (Without<Agent>, Without<Train>)>,
    mut agent_spawn: ResMut<AgentSpawnTimer>) -> Result<(), hotline_rs::Error> {

    let dt = time.0.delta;
    let mut rng = rand::thread_rng();

    // --- Phase 0a: Spawn incoming agents outside the station ---
    agent_spawn.timer -= dt;
    if agent_spawn.timer <= 0.0 {
        agent_spawn.timer = AGENT_SPAWN_INTERVAL;

        let in_gate_x = -10.0;
        let x = rng.gen_range(-AGENT_SPAWN_X_RANGE..AGENT_SPAWN_X_RANGE);
        let platform = rng.gen_range(0..PLATFORM_X.len());

        commands.spawn((
            MeshComponent(meshes.agent_mesh.clone()),
            Position(Vec3f::new(x, 0.0, -80.0)),
            Rotation(meshes.agent_facing),
            Scale(splat3f(2.5)),
            WorldMatrix(Mat34f::identity()),
            Agent {
                waypoints: vec![
                    Vec3f::new(in_gate_x, 0.0, -30.0),
                    Vec3f::new(PLATFORM_X[platform], 0.0, TRAIN_STOP_Z),
                ],
                current_wp: 0,
                speed: rng.gen_range(15.0..35.0),
                turn_speed: rng.gen_range(3.0..6.0),
                state: AgentState::Moving,
                pass_dir: PassDir::PosZ,
                target_platform: Some(platform),
            },
        ));
    }

    // --- Phase 0b: Spawn trains from platform timers ---
    for (pi, platform) in platform_res.platforms.iter_mut().enumerate() {
        platform.spawn_timer -= dt;
        if platform.spawn_timer <= 0.0 {
            platform.spawn_timer = TRAIN_SPAWN_INTERVAL;

            let px = PLATFORM_X[pi];
            let arrive_dir = platform.arrive_dir;
            let start_z = arrive_dir * TRAIN_OFFSCREEN_Z;
            let train_pos = Vec3f::new(px, 6.0, start_z);

            let train_entity = commands.spawn((
                MeshComponent(meshes.cube_mesh.clone()),
                Position(train_pos),
                Rotation(Quatf::identity()),
                Scale(Vec3f::new(6.0, 6.0, 40.0)),
                WorldMatrix(Mat34f::identity()),
                Train {
                    platform: pi,
                    state: TrainState::Arriving,
                    timer: 0.0,
                    arrive_dir,
                },
            )).id();

            // Spawn passengers riding the train
            let num_passengers = rng.gen_range(TRAIN_MIN_PASSENGERS..=TRAIN_MAX_PASSENGERS);
            let out_gate_x = 10.0;
            for _ in 0..num_passengers {
                commands.spawn((
                    MeshComponent(meshes.agent_mesh.clone()),
                    Position(train_pos),
                    Rotation(meshes.agent_facing),
                    Scale(splat3f(2.5)),
                    WorldMatrix(Mat34f::identity()),
                    Agent {
                        waypoints: vec![
                            Vec3f::new(out_gate_x, 0.0, 30.0),   // approach OUT gate
                            Vec3f::new(out_gate_x, 0.0, -30.0),  // past gate
                            Vec3f::new(0.0, 0.0, -80.0),         // exit
                        ],
                        current_wp: 0,
                        speed: rng.gen_range(15.0..35.0),
                        turn_speed: rng.gen_range(3.0..6.0),
                        state: AgentState::OnTrain { train_entity },
                        pass_dir: PassDir::NegZ,
                        target_platform: None,
                    },
                ));
            }
        }
    }

    // --- Phase 1: Gate timers ---
    for barrier in barriers.barriers.iter_mut() {
        for gate in barrier.gates.iter_mut() {
            if gate.processing.is_some() && !gate.open {
                gate.timer -= dt;
                if gate.timer <= 0.0 {
                    gate.open = true;
                }
            }
        }
    }

    // --- Phase 2: Update trains (one-direction travel), collect info ---
    struct TrainInfo {
        entity: Entity,
        platform: usize,
        state: TrainState,
        pos_z: f32,
        delta_z: f32,
        just_stopped: bool,
    }
    let mut train_infos: Vec<TrainInfo> = Vec::new();
    let mut despawn_entities: Vec<Entity> = Vec::new();

    for (train_entity, mut train_pos, mut train) in &mut train_query {
        let target_z = TRAIN_STOP_Z;
        let arrive_z = train.arrive_dir * TRAIN_OFFSCREEN_Z;
        // Train travels opposite to its arrive direction
        let travel_dir = if arrive_z > target_z { -1.0 } else { 1.0 };
        let mut delta_z: f32 = 0.0;
        let mut just_stopped = false;

        match train.state {
            TrainState::Arriving => {
                let step = TRAIN_SPEED * dt * travel_dir;
                delta_z = step;
                train_pos.0.z += step;
                if (travel_dir > 0.0 && train_pos.0.z >= target_z) ||
                   (travel_dir < 0.0 && train_pos.0.z <= target_z) {
                    train_pos.0.z = target_z;
                    delta_z = 0.0;
                    train.state = TrainState::Stopped;
                    train.timer = TRAIN_STOP_DURATION;
                    just_stopped = true;
                }
            }
            TrainState::Stopped => {
                train.timer -= dt;
                if train.timer <= 0.0 {
                    train.state = TrainState::Departing;
                }
            }
            TrainState::Departing => {
                // Continue in same travel direction past the stop
                let step = TRAIN_SPEED * dt * travel_dir;
                delta_z = step;
                train_pos.0.z += step;
                // Despawn when reaching the far side
                let depart_limit = -train.arrive_dir * TRAIN_OFFSCREEN_Z;
                if (travel_dir > 0.0 && train_pos.0.z >= depart_limit) ||
                   (travel_dir < 0.0 && train_pos.0.z <= depart_limit) {
                    despawn_entities.push(train_entity);
                    delta_z = 0.0;
                }
            }
        }

        train_infos.push(TrainInfo {
            entity: train_entity,
            platform: train.platform,
            state: train.state,
            pos_z: train_pos.0.z,
            delta_z,
            just_stopped,
        });
    }

    // --- Phase 3: Update agents ---
    let positions: Vec<(Entity, Vec3f)> = agent_query.iter()
        .map(|(e, p, _, _)| (e, p.0))
        .collect();

    for (entity, mut pos, mut rot, mut agent) in &mut agent_query {
        match agent.state {
            AgentState::Moving => {
                // Check if approaching a barrier
                let mut entered_queue = false;
                for (bi, barrier) in barriers.barriers.iter_mut().enumerate() {
                    let in_approach = match agent.pass_dir {
                        PassDir::PosZ => pos.0.z > barrier.z - BARRIER_APPROACH_DIST && pos.0.z < barrier.z,
                        PassDir::NegZ => pos.0.z < barrier.z + BARRIER_APPROACH_DIST && pos.0.z > barrier.z,
                    };
                    if in_approach {
                        let best = barrier.gates.iter().enumerate()
                            .filter(|(_, g)| g.dir == agent.pass_dir)
                            .min_by_key(|(_, g)| g.queue.len() + if g.processing.is_some() { 1 } else { 0 });
                        if let Some((gi, _)) = best {
                            barrier.gates[gi].queue.push(entity);
                            agent.state = AgentState::Queuing { barrier: bi, gate: gi };
                            entered_queue = true;
                        }
                        break;
                    }
                }

                if !entered_queue {
                    if agent.current_wp >= agent.waypoints.len() {
                        if let Some(platform) = agent.target_platform {
                            agent.state = AgentState::WaitingForTrain { platform };
                        } else {
                            // Finished all waypoints, no destination - despawn
                            despawn_entities.push(entity);
                        }
                        continue;
                    }
                    let target = agent.waypoints[agent.current_wp];
                    let to_target = target - pos.0;
                    let dist = length(to_target);

                    if dist > 0.1 {
                        let seek_dir = to_target / dist;

                        let mut avoid = Vec3f::zero();
                        for &(other_entity, other_pos) in &positions {
                            if other_entity == entity { continue; }
                            let offset = pos.0 - other_pos;
                            let d = length(offset);
                            if d > 0.0 && d < AVOID_RADIUS {
                                avoid = avoid + (offset / d) * (1.0 - d / AVOID_RADIUS);
                            }
                        }

                        let desired_vel = seek_dir * agent.speed + avoid * AVOID_STRENGTH;
                        let desired_speed = length(desired_vel);
                        let dir = if desired_speed > 0.001 { desired_vel / desired_speed } else { seek_dir };

                        let move_dist = agent.speed * dt;
                        if move_dist >= dist {
                            pos.0 = target;
                            agent.current_wp += 1;
                        } else {
                            pos.0 = pos.0 + dir * move_dist;
                        }

                        let yaw = f32::atan2(dir.x, dir.z);
                        let desired = Quatf::from_euler_angles(f32::pi() * 0.5, yaw, 0.0);
                        let t = f32::min(agent.turn_speed * dt, 1.0);
                        rot.0 = quat_nlerp(rot.0, desired, t);
                    } else {
                        agent.current_wp += 1;
                    }
                }
            }

            AgentState::Queuing { barrier: bi, gate: gi } => {
                let barrier_z = barriers.barriers[bi].z;
                let gate_dir = barriers.barriers[bi].gates[gi].dir;
                let gate_x = barriers.barriers[bi].gates[gi].x;
                let queue_idx = barriers.barriers[bi].gates[gi].queue.iter()
                    .position(|&e| e == entity).unwrap_or(0);

                if queue_idx == 0 && barriers.barriers[bi].gates[gi].processing.is_none() {
                    barriers.barriers[bi].gates[gi].queue.remove(0);
                    barriers.barriers[bi].gates[gi].processing = Some(entity);
                    barriers.barriers[bi].gates[gi].timer = GATE_PROCESS_TIME;
                    barriers.barriers[bi].gates[gi].open = false;
                    agent.state = AgentState::Processing { barrier: bi, gate: gi };
                } else {
                    let queue_offset_z = match gate_dir {
                        PassDir::PosZ => -QUEUE_SPACING * (queue_idx as f32 + 1.0),
                        PassDir::NegZ => QUEUE_SPACING * (queue_idx as f32 + 1.0),
                    };
                    let target = Vec3f::new(gate_x, 0.0, barrier_z + queue_offset_z);
                    let to_target = target - pos.0;
                    let dist = length(to_target);
                    if dist > 0.5 {
                        let dir = to_target / dist;
                        let move_dist = f32::min(agent.speed * dt, dist);
                        pos.0 = pos.0 + dir * move_dist;

                        let yaw = f32::atan2(dir.x, dir.z);
                        let desired = Quatf::from_euler_angles(f32::pi() * 0.5, yaw, 0.0);
                        let t = f32::min(agent.turn_speed * dt, 1.0);
                        rot.0 = quat_nlerp(rot.0, desired, t);
                    }
                }
            }

            AgentState::Processing { barrier: bi, gate: gi } => {
                let barrier_z = barriers.barriers[bi].z;
                let gate_dir = barriers.barriers[bi].gates[gi].dir;
                let gate_x = barriers.barriers[bi].gates[gi].x;

                let gate_pos = Vec3f::new(gate_x, 0.0, barrier_z);
                let to_gate = gate_pos - pos.0;
                let dist = length(to_gate);
                if dist > 0.5 {
                    let dir = to_gate / dist;
                    pos.0 = pos.0 + dir * f32::min(agent.speed * dt, dist);
                }

                if barriers.barriers[bi].gates[gi].open {
                    let past_z = match gate_dir {
                        PassDir::PosZ => barrier_z + 3.0,
                        PassDir::NegZ => barrier_z - 3.0,
                    };
                    pos.0 = Vec3f::new(gate_x, 0.0, past_z);
                    barriers.barriers[bi].gates[gi].processing = None;
                    barriers.barriers[bi].gates[gi].open = false;
                    agent.state = AgentState::Moving;
                }
            }

            AgentState::WaitingForTrain { platform } => {
                if let Some(ti) = train_infos.iter().find(|t| t.platform == platform && t.state == TrainState::Stopped) {
                    let board_pos = Vec3f::new(PLATFORM_X[platform], 0.0, ti.pos_z);
                    let to_train = board_pos - pos.0;
                    let dist = length(to_train);
                    if dist > 2.0 {
                        let dir = to_train / dist;
                        let move_dist = f32::min(agent.speed * dt, dist);
                        pos.0 = pos.0 + dir * move_dist;

                        let yaw = f32::atan2(dir.x, dir.z);
                        let desired = Quatf::from_euler_angles(f32::pi() * 0.5, yaw, 0.0);
                        let t = f32::min(agent.turn_speed * dt, 1.0);
                        rot.0 = quat_nlerp(rot.0, desired, t);
                    } else {
                        agent.state = AgentState::OnTrain { train_entity: ti.entity };
                    }
                }
            }

            AgentState::OnTrain { train_entity } => {
                if let Some(ti) = train_infos.iter().find(|t| t.entity == train_entity) {
                    if ti.just_stopped {
                        // Disembark: start walking toward exit
                        agent.state = AgentState::Moving;
                        agent.current_wp = 0;
                    } else if despawn_entities.contains(&train_entity) {
                        // Train despawning - take agent with it
                        despawn_entities.push(entity);
                    } else {
                        // Inherit train velocity
                        pos.0.z += ti.delta_z;
                    }
                }
            }
        }
    }

    // --- Phase 4: Animate gate flaps ---
    for (flap, mut flap_rot) in &mut flap_query {
        let gate = &barriers.barriers[flap.barrier].gates[flap.gate];
        let target_yaw = if gate.open || gate.processing.is_none() { f32::pi() * 0.5 } else { 0.0 };
        let desired = Quatf::from_euler_angles(0.0, target_yaw, 0.0);
        flap_rot.0 = quat_nlerp(flap_rot.0, desired, f32::min(8.0 * dt, 1.0));
    }

    // --- Phase 5: Despawn entities that left the scene ---
    for e in despawn_entities {
        commands.entity(e).despawn();
    }

    Ok(())
}
