use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use crate::grid::*;
use crate::systems::{TextureSettings, TextureAssets};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, States)]
pub enum ToolMode {
    #[default]
    PanView,
    Block,
    Grass,
    Cave,
    Food,
    Portal,
    Remove,
}

#[derive(Resource, Default)]
pub struct ActiveTool {
    pub mode: ToolMode,
}

#[derive(Resource, Default)]
pub struct LastPortal {
    pub coords: Option<(i32, i32)>,
}

#[derive(Resource)]
pub struct GridVisuals {
    pub entities: Vec<Option<Entity>>,
}

impl GridVisuals {
    pub fn new() -> Self {
        Self {
            entities: vec![None; GRID_WIDTH * GRID_HEIGHT],
        }
    }
}

// System to handle panning and zooming camera controls
pub fn camera_zoom_pan_system(
    mut mouse_motion: EventReader<MouseMotion>,
    mut mouse_wheel: EventReader<MouseWheel>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    active_tool: Res<ActiveTool>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    mut camera_query: Query<(&Camera, &GlobalTransform, &mut Transform, &mut OrthographicProjection)>,
) {
    let window = window_query.single();
    let (camera, global_transform, mut camera_transform, mut projection) = camera_query.single_mut();
    
    // 1. Zooming (Mouse Wheel)
    let mut zoom_change = 0.0;
    for event in mouse_wheel.read() {
        // Scroll up is positive y, which zooms in (reduces scale)
        zoom_change -= event.y * 0.1;
    }
    
    if zoom_change != 0.0 {
        if let Some(cursor_pos) = window.cursor_position() {
            if let Some(world_pos) = camera.viewport_to_world_2d(global_transform, cursor_pos) {
                let old_scale = projection.scale;
                projection.scale = (projection.scale + zoom_change).clamp(0.25, 4.0);
                let scale_ratio = projection.scale / old_scale;
                
                // Pin the zoom to the mouse cursor position
                camera_transform.translation.x = world_pos.x + (camera_transform.translation.x - world_pos.x) * scale_ratio;
                camera_transform.translation.y = world_pos.y + (camera_transform.translation.y - world_pos.y) * scale_ratio;
            }
        }
    }
    
    // 2. Panning (Middle/Right mouse drag, or Left mouse drag when ToolMode::PanView)
    let is_panning_button = mouse_input.pressed(MouseButton::Right) 
        || mouse_input.pressed(MouseButton::Middle)
        || (mouse_input.pressed(MouseButton::Left) && active_tool.mode == ToolMode::PanView);
        
    if is_panning_button {
        let mut delta = Vec2::ZERO;
        for event in mouse_motion.read() {
            delta += event.delta;
        }
        
        if delta != Vec2::ZERO {
            // Adjust for projection scale
            camera_transform.translation.x -= delta.x * projection.scale;
            camera_transform.translation.y += delta.y * projection.scale; // Inverted Y screen-to-world
        }
    }
}

// System to edit grid cell types with drag and click drawing
pub fn grid_editing_system(
    mouse_input: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    mut grid: ResMut<SwarmGrid>,
    mut visuals: ResMut<GridVisuals>,
    mut last_portal: ResMut<LastPortal>,
    active_tool: Res<ActiveTool>,
    mut commands: Commands,
    texture_settings: Res<TextureSettings>,
    texture_assets: Res<TextureAssets>,
) {
    if active_tool.mode == ToolMode::PanView {
        return;
    }
    
    let window = window_query.single();
    let (camera, camera_transform) = camera_query.single();
    
    if let Some(cursor_pos) = window.cursor_position() {
        // Ignore clicks on the left UI sidebar (x <= 110.0 pixels)
        if cursor_pos.x <= 110.0 {
            return;
        }
        
        let is_action = match active_tool.mode {
            ToolMode::Portal => mouse_input.just_pressed(MouseButton::Left),
            _ => mouse_input.pressed(MouseButton::Left),
        };
        
        if is_action {
            if let Some(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                let (gx, gy) = grid.get_grid_coords(world_pos);
                if let Some(idx) = grid.get_cell_index(gx, gy) {
                    
                    // Determine new properties
                    let (new_type, new_passable) = match active_tool.mode {
                        ToolMode::Block => (GridCellType::Wall, false),
                        ToolMode::Grass => (GridCellType::Grass, true),
                        ToolMode::Cave => (GridCellType::Cave, true),
                        ToolMode::Food => (GridCellType::Food, true),
                        ToolMode::Portal => {
                            // Determine portal color
                            let color = if let Some(last_coords) = last_portal.coords {
                                // Double check the last portal still exists and is a Portal
                                let last_idx = grid.get_cell_index(last_coords.0, last_coords.1).unwrap();
                                if let GridCellType::Portal(PortalColor::Blue) = grid.cells[last_idx].cell_type {
                                    PortalColor::Orange
                                } else {
                                    PortalColor::Blue
                                }
                            } else {
                                PortalColor::Blue
                            };
                            (GridCellType::Portal(color), true)
                        }
                        ToolMode::Remove => (GridCellType::Empty, true),
                        ToolMode::PanView => unreachable!(),
                    };
                    
                    // Avoid redundant work if cell is already modified to this state
                    if grid.cells[idx].cell_type == new_type && grid.cells[idx].passable == new_passable {
                        return;
                    }
                    
                    // 1. Clean up old cell visual entity and links
                    if let Some(entity) = visuals.entities[idx] {
                        commands.entity(entity).despawn_recursive();
                        visuals.entities[idx] = None;
                    }
                    
                    // If old cell was a portal, unlink it
                    if let GridCellType::Portal(_) = grid.cells[idx].cell_type {
                        if let Some((lx, ly)) = grid.cells[idx].portal_link {
                            if let Some(l_idx) = grid.get_cell_index(lx, ly) {
                                grid.cells[l_idx].portal_link = None;
                                grid.cells[l_idx].cell_type = GridCellType::Empty;
                                grid.cells[l_idx].passable = true;
                                if let Some(l_ent) = visuals.entities[l_idx] {
                                    commands.entity(l_ent).despawn_recursive();
                                    visuals.entities[l_idx] = None;
                                }
                            }
                        }
                        // If it was the pending portal for pairing, clear it
                        if let Some(last_coords) = last_portal.coords {
                            if last_coords == (gx, gy) {
                                last_portal.coords = None;
                            }
                        }
                    }
                    
                    // 2. Set new cell properties
                    grid.cells[idx].cell_type = new_type;
                    grid.cells[idx].passable = new_passable;
                    grid.cells[idx].portal_link = None;
                    
                    if new_type == GridCellType::Food {
                        grid.cells[idx].food_storage = 999999.0;
                    }
                    
                    // 3. Handle Portal pairing
                    if let GridCellType::Portal(color) = new_type {
                        match color {
                            PortalColor::Blue => {
                                last_portal.coords = Some((gx, gy));
                            }
                            PortalColor::Orange => {
                                if let Some(last_coords) = last_portal.coords {
                                    let last_idx = grid.get_cell_index(last_coords.0, last_coords.1).unwrap();
                                    
                                    // Link them together
                                    grid.cells[idx].portal_link = Some(last_coords);
                                    grid.cells[last_idx].portal_link = Some((gx, gy));
                                    
                                    // Reset last portal tracking
                                    last_portal.coords = None;
                                }
                            }
                        }
                    }
                    
                    // 4. Spawn new visual entity
                    let size = Vec2::splat(GRID_SIZE - 1.0);
                    let center = Vec3::new(
                        (gx as f32 + 0.5) * GRID_SIZE,
                        (gy as f32 + 0.5) * GRID_SIZE,
                        0.0,
                    );
                    
                    let sprite_color = match new_type {
                        GridCellType::Wall => Color::srgb(0.22, 0.22, 0.22),
                        GridCellType::Grass => Color::srgb(0.2, 0.6, 0.2),
                        GridCellType::Cave => Color::srgb(0.05, 0.25, 0.55),
                        GridCellType::Food => Color::srgb(0.9, 0.75, 0.1),
                        GridCellType::Portal(PortalColor::Blue) => Color::srgb(0.0, 0.5, 1.0),
                        GridCellType::Portal(PortalColor::Orange) => Color::srgb(1.0, 0.5, 0.0),
                        GridCellType::Empty => Color::NONE,
                    };
                    
                    if new_type != GridCellType::Empty {
                        let (texture_handle, color_tint) = if texture_settings.enabled {
                            let tex = match new_type {
                                GridCellType::Wall => texture_assets.wall.clone(),
                                GridCellType::Grass => texture_assets.grass.clone(),
                                GridCellType::Cave => texture_assets.cave.clone(),
                                GridCellType::Food => texture_assets.food.clone(),
                                GridCellType::Portal(PortalColor::Blue) => texture_assets.portal_blue[0].clone(),
                                GridCellType::Portal(PortalColor::Orange) => texture_assets.portal_orange[0].clone(),
                                GridCellType::Empty => Handle::default(),
                            };
                            (tex, Color::WHITE)
                        } else {
                            (Handle::default(), sprite_color)
                        };

                        let entity = commands.spawn(SpriteBundle {
                            sprite: Sprite {
                                color: color_tint,
                                custom_size: Some(size),
                                ..default()
                            },
                            texture: texture_handle,
                            transform: Transform::from_translation(center),
                            ..default()
                        }).id();
                        visuals.entities[idx] = Some(entity);
                    }
                }
            }
        }
    }
}
