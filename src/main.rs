mod components;
mod grid;
mod systems;
mod interaction;
mod ui;

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::mesh::PrimitiveTopology;
use bevy::sprite::Mesh2dHandle;
use rand::Rng;

use crate::components::*;
use crate::grid::*;
use crate::systems::*;
use crate::interaction::*;
use crate::ui::*;

fn main() {
    App::new()
        // Initialize Bevy with default plugins and premium window config
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Locas-Ants ECS Simulation - 100,000 Ants in Rust (WebGPU/WebGpu Ready)".to_string(),
                resolution: (1280.0, 720.0).into(),
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        // Diagnostics for real-time FPS reporting
        .add_plugins(FrameTimeDiagnosticsPlugin)
        
        // Dark background color resource
        .insert_resource(ClearColor(Color::srgb(0.04, 0.04, 0.04)))
        
        // Swarm global resources
        .insert_resource(SwarmGrid::new())
        .insert_resource(SimulationFrame { frame_number: 0 })
        .insert_resource(SimulationSpeed { multiplier: 1 })
        .insert_resource(DebugSettings { show_pheromones: false })
        .insert_resource(TextureSettings { enabled: true })
        .insert_resource(CombinedTextureState::default())
        .insert_resource(ActiveTool::default())
        .insert_resource(LastPortal::default())
        .insert_resource(GridVisuals::new())
        
        // Setup static grid visual entities, spawn ants and UI sidebar
        .add_systems(Startup, (setup_simulation, setup_ui))
        
        // ECS Simulation, Camera, and UI Systems
        .add_systems(Update, (
            debug_toggle_system,
            create_combined_texture_system,
            update_ant_material_system,
            ant_update_system,
            mesh_render_update_system,
            update_ground_visibility_system,
            update_cell_textures_system,
            update_gizmo_config_system,
            pheromone_debug_system,
            portal_line_debug_system,
            update_frame_counter,
            fps_display_system,
            camera_zoom_pan_system,
            grid_editing_system,
            ui_button_interaction_system,
            ui_update_text_system,
        ).chain()) // Chain them to run sequentially in logic
        
        .run();
}

fn setup_simulation(
    mut commands: Commands,
    grid: Res<SwarmGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut visuals: ResMut<GridVisuals>,
    asset_server: Res<AssetServer>,
) {
    // 1. Load texture assets and insert resource
    let texture_assets = TextureAssets {
        ant_walk: [
            asset_server.load("images/antWalk_00.png"),
            asset_server.load("images/antWalk_01.png"),
            asset_server.load("images/antWalk_02.png"),
            asset_server.load("images/antWalk_03.png"),
        ],
        wall: asset_server.load("images/block01.png"),
        cave: asset_server.load("images/cave.png"),
        grass: asset_server.load("images/grass01.png"),
        ground: asset_server.load("images/ground01.png"),
        food: asset_server.load("images/food04.png"),
        portal_blue: [
            asset_server.load("images/portalBlue_00.png"),
            asset_server.load("images/portalBlue_01.png"),
            asset_server.load("images/portalBlue_02.png"),
        ],
        portal_orange: [
            asset_server.load("images/portalOrange_00.png"),
            asset_server.load("images/portalOrange_01.png"),
            asset_server.load("images/portalOrange_02.png"),
        ],
    };
    commands.insert_resource(texture_assets.clone());

    // 2. Camera setup
    commands.spawn(Camera2dBundle::default());

    // 3. Spawn ground background as a single repeating mesh (rendered at z = -0.5)
    let half_w = 976.0 / 2.0;
    let half_h = 688.0 / 2.0;
    let mut ground_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    ground_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![
        [-half_w,  half_h, 0.0],
        [ half_w,  half_h, 0.0],
        [ half_w, -half_h, 0.0],
        [-half_w, -half_h, 0.0],
    ]);
    ground_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![
        [0.0, 0.0],
        [61.0, 0.0],
        [61.0, 43.0],
        [0.0, 43.0],
    ]);
    ground_mesh.insert_indices(bevy::render::mesh::Indices::U32(vec![
        0, 3, 2,
        0, 2, 1,
    ]));
    let ground_mesh_handle = meshes.add(ground_mesh);
    commands.spawn((
        ColorMesh2dBundle {
            mesh: Mesh2dHandle(ground_mesh_handle),
            material: materials.add(ColorMaterial::from(texture_assets.ground.clone())),
            transform: Transform::from_xyz(104.0, 56.0, -0.5),
            ..default()
        },
        GroundGridParent,
    ));
    
    // 4. Spawn static sprites for the Grid Cells (Obstacles, Caves, Food)
    for cell in grid.cells.iter() {
        let center_x = (cell.gx as f32 + 0.5) * GRID_SIZE;
        let center_y = (cell.gy as f32 + 0.5) * GRID_SIZE;
        let center = Vec3::new(center_x, center_y, 0.0);
        let size = Vec2::splat(GRID_SIZE - 1.0);
        
        let idx = grid.get_cell_index(cell.gx, cell.gy).unwrap();
        let entity = match cell.cell_type {
            GridCellType::Wall => {
                Some(commands.spawn(SpriteBundle {
                    sprite: Sprite {
                        color: Color::WHITE,
                        custom_size: Some(size),
                        ..default()
                    },
                    texture: texture_assets.wall.clone(),
                    transform: Transform::from_translation(center),
                    ..default()
                }).id())
            }
            GridCellType::Cave => {
                Some(commands.spawn(SpriteBundle {
                    sprite: Sprite {
                        color: Color::WHITE,
                        custom_size: Some(size),
                        ..default()
                    },
                    texture: texture_assets.cave.clone(),
                    transform: Transform::from_translation(center),
                    ..default()
                }).id())
            }
            GridCellType::Food => {
                Some(commands.spawn(SpriteBundle {
                    sprite: Sprite {
                        color: Color::WHITE,
                        custom_size: Some(size),
                        ..default()
                    },
                    texture: texture_assets.food.clone(),
                    transform: Transform::from_translation(center),
                    ..default()
                }).id())
            }
            _ => None,
        };
        visuals.entities[idx] = entity;
    }
    
    // 5. Create the massive dynamic mesh for 100,000 ants
    const ANTS_COUNT: usize = 4_000;
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    
    let dummy_positions = vec![[0.0, 0.0, 0.0]; ANTS_COUNT * 3];
    let dummy_colors = vec![[1.0, 1.0, 1.0, 1.0]; ANTS_COUNT * 3];
    
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, dummy_positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, dummy_colors);
    
    let mesh_handle = meshes.add(mesh);
    commands.insert_resource(AntMeshHandle(mesh_handle.clone()));
    
    // Spawn the mesh bundle, tagged with AntMeshMarker, initially visible (used for both modes)
    commands.spawn((
        ColorMesh2dBundle {
            mesh: Mesh2dHandle(mesh_handle),
            material: materials.add(ColorMaterial::default()),
            visibility: Visibility::Inherited,
            ..default()
        },
        AntMeshMarker,
    ));
    
    // 6. Spawn initial ant entities (plain components for mesh-based rendering)
    let mut rng = rand::thread_rng();
    let cave_center_x = 5.0 * GRID_SIZE;
    let cave_center_y = 4.0 * GRID_SIZE;
    
    info!("Spawning initial ants inside the cave...");
    
    for _ in 0..ANTS_COUNT {
        let radius = rng.gen_range(0.0..15.0);
        let theta = rng.gen_range(0.0..std::f32::consts::TAU);
        let pos = Vec2::new(
            cave_center_x + radius * theta.cos(),
            cave_center_y + radius * theta.sin(),
        );
        
        let move_angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let dir = Vec2::new(move_angle.cos(), move_angle.sin());
        
        commands.spawn((
            AntPosition(pos),
            AntDirection(dir),
            AntSpeed(0.0),
            AntPhysics {
                acceleration: rng.gen_range(0.03..0.07),
                erratic: 0.2,
                max_speed: 1.2,
            },
            AntState {
                looking_for: Target::Food,
                last_time_seen_food: -1,
                last_time_seen_cave: -1,
                max_time_seen: -1,
                pheromones_write: true,
                pheromones_back_time: 0,
                teleported_on_frame: 0,
                traveled: 0.0,
            },
            AntMemory {
                positions: [pos; 10],
                index: 0,
            },
            AntComNeeded {
                com_every: rng.gen_range(3..=13),
                com_offset: rng.gen_range(0..13),
            },
        ));
    }
    
    info!("Simulation initialized. Press [Spacebar] or click UI buttons to interact.");
}

// System to print performance metrics to terminal
fn fps_display_system(
    diagnostics: Res<DiagnosticsStore>,
    mut last_time: Local<f32>,
    time: Res<Time>,
) {
    if time.elapsed_seconds() - *last_time > 1.0 {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                info!("Simulation Performance: {:.1} FPS (100,000 ants)", value);
            }
        }
        *last_time = time.elapsed_seconds();
    }
}
