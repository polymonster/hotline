# Rat Race - Tube Station Flow Simulation Game

A relaxing building game where you design the infrastructure inside a tube (underground) station. The aim is to maximise the flow of footfall traffic through barriers, escalators, one-way systems, and other station infrastructure.

## Current State

Working prototype with agents, barrier gates, and trains. Runs as a demo inside the hotline plugin system (`ecs_examples` crate). Select `ratrace` from the demo list.

### What's Built

**Agents (people)**
- Small pyramid-shaped agents (scale 2.5, tip = facing direction) on the XZ plane
- Two flows: IN agents spawn outside and enter the station, OUT agents arrive on trains and exit
- Continuous spawning: new IN agents every 3s at random X positions (z=-80), train passengers arrive with each train
- Each agent has randomised speed (15-35), turn speed (3-6)
- Waypoint-following with smooth quaternion rotation (custom `quat_nlerp`)
- Boids-style separation avoidance (AVOID_RADIUS=15, AVOID_STRENGTH=80)
- Agents despawn when they finish all waypoints with no destination

**Barrier System**
- Single barrier wall at z=0 with two gates:
  - IN gate at x=-10 (allows +Z travel, entering the station)
  - OUT gate at x=10 (allows -Z travel, exiting)
- Direction (`dir: PassDir`) is per-gate, not per-barrier
- Wall segments fill gaps between gates (extending to x=+-60)
- Gate posts mark each opening, with a rotating flap bar

**Agent State Machine**
- `Moving` - follows waypoints with avoidance. Detects barrier approach zone (20 units before barrier) and picks shortest queue
- `Queuing` - moves to queue position (spaced 8 units apart behind the gate). Waits for front of queue
- `Processing` - slides to gate, waits 1.5s (payment timer), gate flap opens, agent passes through
- `WaitingForTrain` - at platform, walks toward a stopped train
- `OnTrain` - inherits train velocity along Z axis. Disembarks when train stops

**Train System**
- Two platforms at x=-80 and x=+80, trains travel along Z axis
- One-direction travel: trains spawn offscreen (z=+-300), arrive at TRAIN_STOP_Z=40, stop for 5s, depart continuing in same direction, despawn at the far side (z=-/+300)
- Trains spawn every 8s per platform (staggered), carrying 1-4 random passenger agents
- When train stops: passengers disembark (transition to Moving with exit waypoints)
- When train despawns: any remaining passengers (boarded agents) despawn with it
- Train mesh: cube scaled (6, 6, 40) at y=6, positioned at PLATFORM_X
- Platform 0 (x=-80): train arrives from +Z, Platform 1 (x=+80): arrives from -Z

**Gate Flap Animation**
- Flaps rotate 90 deg open/closed via `quat_nlerp`
- Open when no one is processing, closed during processing

**Agent Flow**
- IN flow: spawn at z=-80 → waypoint to IN gate approach (x=-10, z=-30) → queue at IN gate → pass through → walk to platform → wait for train → board → ride away → despawn
- OUT flow: spawn on arriving train → ride to station → disembark → walk to OUT gate approach (x=10, z=30) → queue at OUT gate → pass through → walk to exit (z=-80) → despawn

### Architecture

```
ratrace.rs
  ratrace()           -> ScheduleInfo (setup + update systems, render_graph)
  setup_ratrace()     -> spawns barriers, inserts resources (GameMeshes, PlatformRes, AgentSpawnTimer, BarrierRes)
  update_ratrace()    -> Phase 0a: agent spawning
                         Phase 0b: train spawning with passengers
                         Phase 1: gate timers
                         Phase 2: train movement (collect TrainInfo, despawn list)
                         Phase 3: agent state machine
                         Phase 4: flap animation
                         Phase 5: despawn entities
```

**Key types:**
- `Agent` (Component) - waypoints, current_wp, speed, turn_speed, state, pass_dir, target_platform
- `Train` (Component) - platform, state, timer, arrive_dir
- `GateFlap` (Component) - barrier/gate indices for animation lookup
- `BarrierRes` (Resource) - holds Vec<BarrierInfo> with gate queues/timers
- `GameMeshes` (Resource) - stores agent_mesh, cube_mesh, agent_facing for runtime spawning
- `PlatformRes` (Resource) - per-platform spawn timer and arrive direction
- `AgentSpawnTimer` (Resource) - timer for spawning incoming agents
- `AgentState` enum - Moving / Queuing / Processing / WaitingForTrain / OnTrain
- `TrainState` enum - Arriving / Stopped / Departing
- `PassDir` enum - PosZ / NegZ

**Render:** Uses existing `mesh_wireframe_overlay` render graph with `DebugDrawFlags::GRID`. No custom render function needed.

### Constants

```
AVOID_RADIUS=15, AVOID_STRENGTH=80
GATE_WIDTH=12, GATE_PROCESS_TIME=1.5, BARRIER_APPROACH_DIST=20, QUEUE_SPACING=8
WALL_HEIGHT=6, BARRIER_Z=0
PLATFORM_X=[-80, 80], TRAIN_STOP_Z=40, TRAIN_SPEED=40, TRAIN_OFFSCREEN_Z=300
TRAIN_STOP_DURATION=5, TRAIN_SPAWN_INTERVAL=8
TRAIN_MIN_PASSENGERS=1, TRAIN_MAX_PASSENGERS=4
AGENT_SPAWN_INTERVAL=3, AGENT_SPAWN_X_RANGE=40
```

### Technical Notes

- **Quaternion interpolation:** maths-rs 0.2.7 `slerp`/`nlerp` free functions don't work with f32 (trait bound issues). Hand-rolled `quat_nlerp` using Quat operator overloads (Dot, Neg, Mul, Add, normalize). Quat fields are private.
- **Quat * Vec3:** `Quatf` implements `Mul<Vec3f>` for rotating vectors. Useful for extracting facing direction from rotation.
- **Bevy query disjointness:** Three queries with filters: agent_query `Without<GateFlap, Train>`, train_query `Without<Agent, GateFlap>`, flap_query `Without<Agent, Train>`
- **Avoidance pattern:** Collect positions into Vec first, then iterate mutably (avoids borrow conflict on the query)
- **Train info pattern:** Collect TrainInfo structs during train iteration, use during agent iteration (avoids cross-query borrow)
- **Runtime spawning:** Meshes stored in `GameMeshes` resource (type: `pmfx::Mesh<gfx_platform::Device>`). `Commands` works in update functions via `#[export_update_fn]` macro.
- **Despawn pattern:** Collect entity IDs in `despawn_entities` Vec across phases, call `commands.entity(e).despawn()` at end
- **Grid:** Don't draw custom grid - use `session_info.debug_draw_flags |= DebugDrawFlags::GRID`
- **Cube mesh:** Extends -1 to +1 in all axes. Scale = half-extents.
- **Pyramid tip direction:** `Quatf::from_euler_angles(pi * 0.5, 0.0, 0.0)` points tip toward -Z

### Known Issues

- **Agent sliding:** Agents can visibly slide sideways when avoidance pushes them perpendicular to their facing direction. The movement direction (seek + avoid) updates instantly but rotation lerps. Attempted fix using facing-direction movement (`rot * Vec3f`) made it worse. Needs investigation - possible approaches: higher turn speed, storing yaw directly, or projecting avoidance onto facing axis.

### Planned Features

- **More barrier types** - escalators, one-way corridors
- **Player interaction** - place/remove/upgrade infrastructure to improve flow
- **Flow metrics** - measure throughput, average wait times, bottleneck detection
- **Agent variety** - different urgency levels, tourists vs commuters
- **Visual improvements** - colour coding agents by state/direction, different meshes for IN vs OUT
- **Multiple levels** - different station layouts with increasing complexity

### File Locations

- Game code: `plugins/ecs_examples/src/ratrace.rs`
- Module registration: `plugins/ecs_examples/src/lib.rs` (mod + demo list)
- Render graph: `shaders/draw.jsn` (mesh_wireframe_overlay)
- ImDraw API: `src/imdraw.rs`
- Primitives: `src/primitives.rs` (create_pyramid_mesh, create_cube_mesh, etc.)
- ECS export macros: `plugins/export_macros/src/lib.rs` (export_update_fn, export_render_fn)
