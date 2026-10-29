// Bevy systems naturally take many parameters and nested query types
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod components;
mod grid;
mod interaction;
mod systems;
mod ui;

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::texture::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::sprite::Mesh2dHandle;

use crate::components::*;
use crate::grid::*;
use crate::interaction::*;
use crate::systems::*;
use crate::ui::*;

// Number of ants spawned at startup
const INITIAL_ANTS: usize = 4_000;

fn main() {
    App::new()
        // Initialize Bevy with default plugins and window config
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Locas-Ants Bevy - ant colony simulation".to_string(),
                resolution: (1280.0_f32, 720.0_f32).into(),
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
        .add_systems(
            Update,
            (
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
                camera_zoom_pan_system,
                grid_editing_system,
                ui_button_interaction_system,
                ui_update_text_system,
            )
                .chain(),
        ) // Chain them to run sequentially in logic
        .run();
}

fn setup_simulation(
    mut commands: Commands,
    grid: Res<SwarmGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut visuals: ResMut<GridVisuals>,
    texture_settings: Res<TextureSettings>,
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
        // The ground quad tiles this texture, so it needs a repeating sampler
        ground: asset_server.load_with_settings("images/ground01.png", |settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..default()
            });
        }),
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
    // One texture repeat per grid cell, covering the whole grid
    let half_w = GRID_WIDTH as f32 * GRID_SIZE / 2.0;
    let half_h = GRID_HEIGHT as f32 * GRID_SIZE / 2.0;
    let mut ground_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    ground_mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-half_w, half_h, 0.0],
            [half_w, half_h, 0.0],
            [half_w, -half_h, 0.0],
            [-half_w, -half_h, 0.0],
        ],
    );
    ground_mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![
            [0.0, 0.0],
            [GRID_WIDTH as f32, 0.0],
            [GRID_WIDTH as f32, GRID_HEIGHT as f32],
            [0.0, GRID_HEIGHT as f32],
        ],
    );
    ground_mesh.insert_indices(bevy::render::mesh::Indices::U32(vec![0, 3, 2, 0, 2, 1]));
    let ground_mesh_handle = meshes.add(ground_mesh);
    commands.spawn((
        ColorMesh2dBundle {
            mesh: Mesh2dHandle(ground_mesh_handle),
            material: materials.add(ColorMaterial::from(texture_assets.ground.clone())),
            transform: Transform::from_xyz(
                (MIN_XG + MAX_XG + 1) as f32 * GRID_SIZE / 2.0,
                (MIN_YG + MAX_YG + 1) as f32 * GRID_SIZE / 2.0,
                -0.5,
            ),
            ..default()
        },
        GroundGridParent,
    ));

    // 4. Spawn static sprites for the Grid Cells (Obstacles, Caves, Food, Grass, Portals)
    for (idx, cell) in grid.cells.iter().enumerate() {
        visuals.entities[idx] = spawn_cell_sprite(
            &mut commands,
            cell.cell_type,
            cell.gx,
            cell.gy,
            &texture_assets,
            texture_settings.enabled,
        );
    }

    // 5. Create the dynamic mesh that renders every ant in a single draw call
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

    let dummy_positions = vec![[0.0, 0.0, 0.0]; INITIAL_ANTS * 3];
    let dummy_colors = vec![[1.0, 1.0, 1.0, 1.0]; INITIAL_ANTS * 3];

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
        bevy::render::view::NoFrustumCulling,
    ));

    // 6. Spawn initial ant entities (plain components for mesh-based rendering)
    let mut rng = rand::thread_rng();
    let cave_center = grid.get_cave_center();

    info!("Spawning {} ants inside the cave...", INITIAL_ANTS);
    for _ in 0..INITIAL_ANTS {
        commands.spawn(new_ant_bundle(cave_center, &mut rng));
    }

    info!("Simulation initialized. Press [Spacebar] or click UI buttons to interact.");
}
