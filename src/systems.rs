use bevy::prelude::*;
use rand::Rng;
use std::sync::atomic::Ordering;
use crate::components::*;
use crate::grid::*;
use crate::interaction::GridVisuals;

#[derive(Resource)]
pub struct SimulationFrame {
    pub frame_number: u64,
}

#[derive(Resource)]
pub struct SimulationSpeed {
    pub multiplier: u32,
}

#[derive(Resource)]
pub struct DebugSettings {
    pub show_pheromones: bool,
}

#[derive(Resource)]
pub struct TextureSettings {
    pub enabled: bool,
}

#[derive(Resource, Default)]
pub struct CombinedTextureState {
    pub handle: Option<Handle<Image>>,
    pub created: bool,
}

#[derive(Resource, Clone)]
pub struct TextureAssets {
    pub ant_walk: [Handle<Image>; 4],
    pub wall: Handle<Image>,
    pub cave: Handle<Image>,
    pub grass: Handle<Image>,
    pub ground: Handle<Image>,
    pub food: Handle<Image>,
    pub portal_blue: [Handle<Image>; 3],
    pub portal_orange: [Handle<Image>; 3],
}

#[derive(Resource)]
pub struct AntMeshHandle(pub Handle<Mesh>);

// Consolidated parallel system that updates all ant behaviors in a single pass to maximize CPU cache locality
pub fn ant_update_system(
    grid: Res<SwarmGrid>,
    frame: Res<SimulationFrame>,
    speed_settings: Res<SimulationSpeed>,
    mut query: Query<(
        &mut AntPosition,
        &mut AntDirection,
        &mut AntSpeed,
        &mut AntState,
        &mut AntMemory,
        &AntPhysics,
        &AntComNeeded,
    )>,
) {
    let multiplier = speed_settings.multiplier;
    if multiplier == 0 {
        return; // Simulation is paused
    }
    
    let base_frame = frame.frame_number;
    
    const SIGHT_DISTANCE: f32 = 30.0;
    const FOV: f32 = std::f32::consts::PI / 6.0; // 30 degrees (pi/6 rad)
    
    for step in 0..multiplier {
        let current_frame = base_frame + step as u64;
        let current_frame_u32 = current_frame as u32;
        let frame_num_i64 = current_frame as i64;
        
        query.par_iter_mut().for_each(|(
            mut position, 
            mut direction, 
            mut speed, 
            mut state, 
            mut memory, 
            physics, 
            com
        )| {
        let (gx, gy) = grid.get_grid_coords(position.0);
        
        // -------------------------------------------------------------
        // 1. Pheromone Communication (Read & Write)
        // -------------------------------------------------------------
        if (current_frame_u32 + com.com_offset) % com.com_every == 0 {
            let mut best_time: i64 = state.max_time_seen;
            let mut best_pos = Vec2::ZERO;
            let mut found_pheromone = false;
            
            // Scan 3x3 neighborhood
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(cell) = grid.get_cell(gx + dx, gy + dy) {
                        let packed = match state.looking_for {
                            Target::Food => cell.food_pheromone.load(Ordering::Relaxed),
                            Target::Cave => cell.cave_pheromone.load(Ordering::Relaxed),
                        };
                        
                        let (time, pos) = unpack_pheromone(packed);
                        if time as i64 > best_time {
                            best_time = time as i64;
                            best_pos = pos;
                            found_pheromone = true;
                        }
                    }
                }
            }
            
            if found_pheromone {
                state.max_time_seen = best_time;
                let to_target = (best_pos - position.0).normalize_or_zero();
                if to_target != Vec2::ZERO {
                    direction.0 = to_target;
                }
            }
            
            // Write pheromones if write cooldown has expired
            let write_enabled = state.pheromones_write || (current_frame >= state.pheromones_back_time);
            if write_enabled {
                if !state.pheromones_write {
                    state.pheromones_write = true;
                }
                
                // Get oldest position remembered in our ring buffer
                let oldest_pos = memory.positions[memory.index];
                
                // Write food path memory if we have seen food
                if state.last_time_seen_food > 0 {
                    grid.update_pheromone(
                        gx,
                        gy,
                        true, // is_food = true
                        state.last_time_seen_food as u32,
                        oldest_pos,
                    );
                }
                
                // Write cave path memory if we have seen cave
                if state.last_time_seen_cave > 0 {
                    grid.update_pheromone(
                        gx,
                        gy,
                        false, // is_food = false (cave)
                        state.last_time_seen_cave as u32,
                        oldest_pos,
                    );
                }
            }
        }
        
        // -------------------------------------------------------------
        // 2. Object Avoidance (FOV Sensors)
        // -------------------------------------------------------------
        let ahead_check = position.0 + direction.0 * SIGHT_DISTANCE;
        if !grid.is_passable(ahead_check) {
            let cos_fov = FOV.cos();
            let sin_fov = FOV.sin();
            
            let left_dir = Vec2::new(
                direction.0.x * cos_fov - direction.0.y * (-sin_fov),
                direction.0.x * (-sin_fov) + direction.0.y * cos_fov,
            ).normalize_or_zero();
            
            let right_dir = Vec2::new(
                direction.0.x * cos_fov - direction.0.y * sin_fov,
                direction.0.x * sin_fov + direction.0.y * cos_fov,
            ).normalize_or_zero();
            
            let left_pos = position.0 + left_dir * (SIGHT_DISTANCE * 0.5);
            let right_pos = position.0 + right_dir * (SIGHT_DISTANCE * 0.5);
            
            let free_left = grid.is_passable(left_pos);
            let free_right = grid.is_passable(right_pos);
            
            if free_left && !free_right {
                direction.0 = left_dir;
            } else if free_right && !free_left {
                direction.0 = right_dir;
            }
        }
        
        // -------------------------------------------------------------
        // 3. Movement Physics (Speed, Friction, Erratic movement, Collisions)
        // -------------------------------------------------------------
        // Save current position into individual short-term memory (Ring Buffer)
        let oldest_idx = memory.index;
        memory.positions[oldest_idx] = position.0;
        memory.index = (oldest_idx + 1) % 10;
        
        // Calculate grass friction (slows down ants on grass)
        let mut friction = 1.0;
        if let Some(cell) = grid.get_cell(gx, gy) {
            if cell.cell_type == GridCellType::Grass {
                friction = 0.8;
            }
        }
        
        // Accelerate
        speed.0 = (speed.0 + physics.acceleration) * friction;
        let max_allowed_speed = physics.max_speed * friction;
        if speed.0 > max_allowed_speed {
            speed.0 = max_allowed_speed;
        }
        
        // Erratic movement (direction noise)
        let mut rng = rand::thread_rng();
        let erratic_angle = rng.gen_range(-physics.erratic * 0.5..physics.erratic * 0.5);
        let cos_e = erratic_angle.cos();
        let sin_e = erratic_angle.sin();
        let rotated_dir = Vec2::new(
            direction.0.x * cos_e - direction.0.y * sin_e,
            direction.0.x * sin_e + direction.0.y * cos_e,
        );
        direction.0 = rotated_dir.normalize_or_zero();
        
        // Collision resolution with sliding behavior
        let mut num_tries = 0;
        let mut collision;
        let mut test_pos;
        let mut current_dir = direction.0;
        
        loop {
            num_tries += 1;
            test_pos = position.0 + current_dir * speed.0;
            
            let out_of_bounds = test_pos.x < MAP_MIN_X 
                || test_pos.x > MAP_MAX_X 
                || test_pos.y < MAP_MIN_Y 
                || test_pos.y > MAP_MAX_Y;
                
            if out_of_bounds {
                collision = true;
                if test_pos.x < MAP_MIN_X && current_dir.x < 0.0 {
                    current_dir.x *= -1.0;
                } else if test_pos.x > MAP_MAX_X && current_dir.x > 0.0 {
                    current_dir.x *= -1.0;
                }
                if test_pos.y < MAP_MIN_Y && current_dir.y < 0.0 {
                    current_dir.y *= -1.0;
                } else if test_pos.y > MAP_MAX_Y && current_dir.y > 0.0 {
                    current_dir.y *= -1.0;
                }
            } else {
                let (tx, ty) = grid.get_grid_coords(test_pos);
                if let Some(cell) = grid.get_cell(tx, ty) {
                    if !cell.passable {
                        collision = true;
                        
                        // Perpendicular sliding behavior
                        let center = Vec2::new(
                            (tx as f32 + 0.5) * GRID_SIZE,
                            (ty as f32 + 0.5) * GRID_SIZE,
                        );
                        let rel = test_pos - center;
                        
                        if (rel.y < -rel.x && rel.y > rel.x) || (rel.y > -rel.x && rel.y < rel.x) {
                            current_dir = if current_dir.y >= 0.0 {
                                Vec2::new(0.0, 1.0)
                            } else {
                                Vec2::new(0.0, -1.0)
                            };
                        } else {
                            current_dir = if current_dir.x >= 0.0 {
                                Vec2::new(1.0, 0.0)
                            } else {
                                Vec2::new(-1.0, 0.0)
                            };
                        }
                    } else {
                        collision = false;
                    }
                } else {
                    collision = true;
                }
            }
            
            if !collision {
                break;
            }
            
            if num_tries == 3 {
                // stuck, reverse direction
                current_dir = -direction.0;
            } else if num_tries >= 6 {
                speed.0 = 0.0;
                break;
            }
        }
        
        direction.0 = current_dir;
        if !collision {
            position.0 = test_pos;
        }
        state.traveled += speed.0;
        
        // -------------------------------------------------------------
        // 4. Grid Cell Interactions (Food, Cave, Task Swapping, Portals)
        // -------------------------------------------------------------
        let (nx, ny) = grid.get_grid_coords(position.0);
        if let Some(cell) = grid.get_cell(nx, ny) {
            match cell.cell_type {
                GridCellType::Food => {
                    state.last_time_seen_food = frame_num_i64;
                    if state.looking_for == Target::Food {
                        state.looking_for = Target::Cave;
                        direction.0 = -direction.0;
                        speed.0 = 0.0;
                        state.max_time_seen = 0;
                        state.pheromones_write = false;
                        state.pheromones_back_time = current_frame + 10;
                    }
                }
                GridCellType::Cave => {
                    state.last_time_seen_cave = frame_num_i64;
                    if state.looking_for == Target::Cave {
                        state.looking_for = Target::Food;
                        direction.0 = -direction.0;
                        speed.0 = 0.0;
                        state.max_time_seen = 0;
                        state.pheromones_write = false;
                        state.pheromones_back_time = current_frame + 10;
                    }
                }
                GridCellType::Portal(_color) => {
                    if current_frame - state.teleported_on_frame > 30 {
                        if let Some((lx, ly)) = cell.portal_link {
                            let teleport_pos = Vec2::new(
                                (lx as f32 + 0.5) * GRID_SIZE,
                                (ly as f32 + 0.5) * GRID_SIZE,
                            );
                            position.0 = teleport_pos;
                            
                            // Clear history positions in ring buffer to prevent lines connecting the portals
                            for pos in memory.positions.iter_mut() {
                                *pos = teleport_pos;
                            }
                            memory.index = 0;
                            
                            state.teleported_on_frame = current_frame;
                        }
                    }
                }
                _ => {}
            }
        }
    });
    }
}

// Dynamic Mesh Rendering Updater (Single-Threaded CPU -> GPU Batching)
pub fn mesh_render_update_system(
    query: Query<(&AntPosition, &AntDirection, &AntState)>,
    mesh_handle: Res<AntMeshHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
    texture_settings: Res<TextureSettings>,
    combined_texture: Res<CombinedTextureState>,
) {
    if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
        let count = query.iter().len();
        
        if texture_settings.enabled && combined_texture.created {
            // Textured Mode: 12 vertices per ant (6 for body, 6 for cargo)
            let mut positions = Vec::with_capacity(count * 12);
            let mut colors = Vec::with_capacity(count * 12);
            let mut uvs = Vec::with_capacity(count * 12);
            
            const R: f32 = 8.0; // Scale radius (16x16 sprite has radius 8)
            
            for (pos, dir, state) in query.iter() {
                let px = pos.0.x;
                let py = pos.0.y;
                let dx = dir.0.x;
                let dy = dir.0.y;
                
                let angle = dy.atan2(dx);
                let cos = angle.cos();
                let sin = angle.sin();
                
                // 1. Ant Body Quad Vertices (pointing right by default in antWalk textures)
                let v0 = [px - R * cos - R * sin, py - R * sin + R * cos, 0.0]; // Top-Left
                let v1 = [px + R * cos - R * sin, py + R * sin + R * cos, 0.0]; // Top-Right
                let v2 = [px + R * cos + R * sin, py + R * sin - R * cos, 0.0]; // Bottom-Right
                let v3 = [px - R * cos + R * sin, py - R * sin - R * cos, 0.0]; // Bottom-Left
                
                positions.push(v0); positions.push(v3); positions.push(v2); // Triangle 1
                positions.push(v0); positions.push(v2); positions.push(v1); // Triangle 2
                
                // Colors (white tint for original texture representation)
                let body_color = match state.looking_for {
                    Target::Food => [1.0, 1.0, 1.0, 0.95],
                    Target::Cave => [1.0, 0.85, 0.15, 0.95],
                };
                for _ in 0..6 {
                    colors.push(body_color);
                }
                
                // UVs for body (frame 0..3) based on state.traveled
                let frame_idx = (state.traveled.floor() as usize) % 4;
                let u_start = frame_idx as f32 * 0.2;
                let u_end = u_start + 0.2;
                
                uvs.push([u_start, 0.0]); uvs.push([u_start, 1.0]); uvs.push([u_end, 1.0]);
                uvs.push([u_start, 0.0]); uvs.push([u_end, 1.0]);   uvs.push([u_end, 0.0]);
                
                // 2. Ant Cargo Quad Vertices (positioned slightly in front of the ant)
                let carrying_cargo = state.looking_for == Target::Cave;
                if carrying_cargo {
                    let cx = px + dx * 8.0;
                    let cy = py + dy * 8.0;
                    const CR: f32 = 1.5; // Cargo size 3x3 (radius 1.5)
                    
                    let c0 = [cx - CR * cos - CR * sin, cy - CR * sin + CR * cos, 0.1];
                    let c1 = [cx + CR * cos - CR * sin, cy + CR * sin + CR * cos, 0.1];
                    let c2 = [cx + CR * cos + CR * sin, cy + CR * sin - CR * cos, 0.1];
                    let c3 = [cx - CR * cos + CR * sin, cy - CR * sin - CR * cos, 0.1];
                    
                    positions.push(c0); positions.push(c3); positions.push(c2);
                    positions.push(c0); positions.push(c2); positions.push(c1);
                    
                    let cargo_color = [1.0, 1.0, 1.0, 0.95];
                    for _ in 0..6 {
                        colors.push(cargo_color);
                    }
                    
                    // UVs for cargo (frame 4, i.e. food texture)
                    uvs.push([0.8, 0.0]); uvs.push([0.8, 1.0]); uvs.push([1.0, 1.0]);
                    uvs.push([0.8, 0.0]); uvs.push([1.0, 1.0]); uvs.push([1.0, 0.0]);
                } else {
                    // Transparent zero-size coordinates if not carrying cargo
                    for _ in 0..6 {
                        positions.push([px, py, 0.0]);
                        colors.push([0.0, 0.0, 0.0, 0.0]);
                        uvs.push([0.0, 0.0]);
                    }
                }
            }
            
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        } else {
            // Procedural Vector Mode: 3 vertices (1 triangle) per ant
            let mut positions = Vec::with_capacity(count * 3);
            let mut colors = Vec::with_capacity(count * 3);
            
            const R: f32 = 1.5; // Ant rendering base scale
            
            for (pos, dir, state) in query.iter() {
                let px = pos.0.x;
                let py = pos.0.y;
                let dx = dir.0.x;
                let dy = dir.0.y;
                
                // Triangle Vertices (pointed arrowhead)
                let v0 = [px + dx * R * 1.5, py + dy * R * 1.5, 0.0];
                let v1 = [px - dx * R * 0.7 - dy * R * 0.7, py - dy * R * 0.7 + dx * R * 0.7, 0.0];
                let v2 = [px - dx * R * 0.7 + dy * R * 0.7, py - dy * R * 0.7 - dx * R * 0.7, 0.0];
                
                positions.push(v0);
                positions.push(v1);
                positions.push(v2);
                
                let color = match state.looking_for {
                    Target::Food => [1.0, 1.0, 1.0, 0.95],      // White: seeking food
                    Target::Cave => [1.0, 0.85, 0.15, 0.95],     // Yellow: carrying cargo
                };
                
                colors.push(color);
                colors.push(color);
                colors.push(color);
            }
            
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            mesh.remove_attribute(Mesh::ATTRIBUTE_UV_0); // Remove UV attribute for flat vector colors
        }
    }
}

// Debug Pheromones Visualization using Bevy Gizmos
pub fn pheromone_debug_system(
    grid: Res<SwarmGrid>,
    debug_settings: Res<DebugSettings>,
    frame: Res<SimulationFrame>,
    mut gizmos: Gizmos,
) {
    if !debug_settings.show_pheromones {
        return;
    }
    let current_frame = frame.frame_number;
    
    for cell in grid.cells.iter() {
        let cx = (cell.gx as f32 + 0.5) * GRID_SIZE;
        let cy = (cell.gy as f32 + 0.5) * GRID_SIZE;
        let center = Vec2::new(cx, cy);
        
        // A. Food pheromone (Yellowish-gold lines pointing towards food)
        let packed_food = cell.food_pheromone.load(Ordering::Relaxed);
        let (food_time, food_pos) = unpack_pheromone(packed_food);
        if food_time > 0 {
            let age = current_frame.saturating_sub(food_time as u64);
            if age < 1275 {
                let alpha = 1.0 - (age as f32 / 1275.0);
                let color = Color::srgba(1.0, 0.94, 0.78, alpha * 0.5); // gold/yellow
                gizmos.circle_2d(center, 1.0, color);
                if food_pos != Vec2::ZERO {
                    gizmos.line_2d(center, food_pos, color);
                }
            }
        }
        
        // B. Cave pheromone (Light blue lines pointing towards cave)
        let packed_cave = cell.cave_pheromone.load(Ordering::Relaxed);
        let (cave_time, cave_pos) = unpack_pheromone(packed_cave);
        if cave_time > 0 {
            let age = current_frame.saturating_sub(cave_time as u64);
            if age < 1275 {
                let alpha = 1.0 - (age as f32 / 1275.0);
                let color = Color::srgba(0.78, 0.78, 1.0, alpha * 0.5); // light blue
                gizmos.circle_2d(center, 1.0, color);
                if cave_pos != Vec2::ZERO {
                    gizmos.line_2d(center, cave_pos, color);
                }
            }
        }
    }
}

// Draw faint lines between linked portals
pub fn portal_line_debug_system(
    grid: Res<SwarmGrid>,
    mut gizmos: Gizmos,
) {
    for cell in grid.cells.iter() {
        if let GridCellType::Portal(PortalColor::Blue) = cell.cell_type {
            if let Some((lx, ly)) = cell.portal_link {
                let start = Vec2::new(
                    (cell.gx as f32 + 0.5) * GRID_SIZE,
                    (cell.gy as f32 + 0.5) * GRID_SIZE,
                );
                let end = Vec2::new(
                    (lx as f32 + 0.5) * GRID_SIZE,
                    (ly as f32 + 0.5) * GRID_SIZE,
                );
                gizmos.line_2d(start, end, Color::srgba(1.0, 1.0, 1.0, 0.2));
            }
        }
    }
}

// Update Frame Counter
pub fn update_frame_counter(
    mut frame: ResMut<SimulationFrame>,
    speed_settings: Res<SimulationSpeed>,
) {
    frame.frame_number += speed_settings.multiplier as u64;
}

// Toggle Debug Settings via Spacebar
pub fn debug_toggle_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut debug_settings: ResMut<DebugSettings>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        debug_settings.show_pheromones = !debug_settings.show_pheromones;
    }
}

// System to toggle background ground grid visibility
pub fn update_ground_visibility_system(
    texture_settings: Res<TextureSettings>,
    mut query: Query<&mut Visibility, With<GroundGridParent>>,
) {
    if texture_settings.is_changed() {
        if let Ok(mut visibility) = query.get_single_mut() {
            *visibility = if texture_settings.enabled {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

// System to update cell textures and portal animations
pub fn update_cell_textures_system(
    grid: Res<SwarmGrid>,
    visuals: Res<GridVisuals>,
    texture_settings: Res<TextureSettings>,
    texture_assets: Res<TextureAssets>,
    frame: Res<SimulationFrame>,
    mut sprite_query: Query<(&mut Handle<Image>, &mut Sprite)>,
) {
    let textures_enabled = texture_settings.enabled;
    
    for (idx, cell) in grid.cells.iter().enumerate() {
        if let Some(entity) = visuals.entities[idx] {
            if let Ok((mut texture, mut sprite)) = sprite_query.get_mut(entity) {
                if textures_enabled {
                    match cell.cell_type {
                        GridCellType::Wall => {
                            *texture = texture_assets.wall.clone();
                            sprite.color = Color::WHITE;
                        }
                        GridCellType::Grass => {
                            *texture = texture_assets.grass.clone();
                            sprite.color = Color::WHITE;
                        }
                        GridCellType::Cave => {
                            *texture = texture_assets.cave.clone();
                            sprite.color = Color::WHITE;
                        }
                        GridCellType::Food => {
                            *texture = texture_assets.food.clone();
                            sprite.color = Color::WHITE;
                        }
                        GridCellType::Portal(color) => {
                            let frame_idx = ((frame.frame_number / 4) % 3) as usize;
                            match color {
                                PortalColor::Blue => {
                                    *texture = texture_assets.portal_blue[frame_idx].clone();
                                }
                                PortalColor::Orange => {
                                    *texture = texture_assets.portal_orange[frame_idx].clone();
                                }
                            }
                            sprite.color = Color::WHITE;
                        }
                        GridCellType::Empty => {
                            *texture = Handle::default();
                            sprite.color = Color::NONE;
                        }
                    }
                } else {
                    // Reset to flat colors
                    *texture = Handle::default();
                    sprite.color = match cell.cell_type {
                        GridCellType::Wall => Color::srgb(0.22, 0.22, 0.22),
                        GridCellType::Grass => Color::srgb(0.2, 0.6, 0.2),
                        GridCellType::Cave => Color::srgb(0.05, 0.25, 0.55),
                        GridCellType::Food => Color::srgb(0.9, 0.75, 0.1),
                        GridCellType::Portal(PortalColor::Blue) => Color::srgb(0.0, 0.5, 1.0),
                        GridCellType::Portal(PortalColor::Orange) => Color::srgb(1.0, 0.5, 0.0),
                        GridCellType::Empty => Color::NONE,
                    };
                }
            }
        }
    }
}

// System to combine ant walking and food textures into a single texture sheet
pub fn create_combined_texture_system(
    texture_assets: Res<TextureAssets>,
    mut images: ResMut<Assets<Image>>,
    mut combined_state: ResMut<CombinedTextureState>,
) {
    if combined_state.created {
        return;
    }
    
    let all_loaded = texture_assets.ant_walk.iter().all(|h| images.get(h).is_some());
        
    if all_loaded {
        info!("All source textures loaded. Merging into combined texture sheet...");
        let img0 = images.get(&texture_assets.ant_walk[0]).unwrap();
        let img1 = images.get(&texture_assets.ant_walk[1]).unwrap();
        let img2 = images.get(&texture_assets.ant_walk[2]).unwrap();
        let img3 = images.get(&texture_assets.ant_walk[3]).unwrap();
        
        let w = img0.texture_descriptor.size.width as usize;
        let h = img0.texture_descriptor.size.height as usize;
        let format = img0.texture_descriptor.format;
        
        let combined_width = w * 5;
        let combined_height = h;
        let pixel_size = 4; // RGBA8
        
        let mut combined_data = vec![0u8; combined_width * combined_height * pixel_size];
        let sources = [img0, img1, img2, img3];
        
        for y in 0..h {
            for (i, src) in sources.iter().enumerate() {
                let src_offset = y * w * pixel_size;
                let dest_offset = (y * combined_width + i * w) * pixel_size;
                if src.data.len() >= src_offset + w * pixel_size {
                    combined_data[dest_offset..dest_offset + w * pixel_size]
                        .copy_from_slice(&src.data[src_offset..src_offset + w * pixel_size]);
                }
            }
            // Fill 5th frame with solid yellow color [250, 240, 100, 255]
            let dest_offset = (y * combined_width + 4 * w) * pixel_size;
            for px in 0..w {
                let p_idx = dest_offset + px * pixel_size;
                combined_data[p_idx] = 250;     // R
                combined_data[p_idx + 1] = 240; // G
                combined_data[p_idx + 2] = 100; // B
                combined_data[p_idx + 3] = 255; // A
            }
        }
        
        use bevy::render::render_resource::{Extent3d, TextureDimension};
        let mut combined_image = Image::new(
            Extent3d {
                width: combined_width as u32,
                height: combined_height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            combined_data,
            format,
            bevy::render::render_asset::RenderAssetUsages::default(),
        );
        combined_image.sampler = img0.sampler.clone();
        
        let handle = images.add(combined_image);
        combined_state.handle = Some(handle);
        combined_state.created = true;
        
        // Mutate ground image sampler to repeat
        if let Some(ground_img) = images.get_mut(&texture_assets.ground) {
            ground_img.sampler = bevy::render::texture::ImageSampler::Descriptor(
                bevy::render::texture::ImageSamplerDescriptor {
                    address_mode_u: bevy::render::texture::ImageAddressMode::Repeat,
                    address_mode_v: bevy::render::texture::ImageAddressMode::Repeat,
                    ..default()
                }
            );
        }
        
        info!("Combined texture sheet created successfully!");
    }
}

// System to toggle the ant mesh material's texture between combined sheet and flat colors
pub fn update_ant_material_system(
    texture_settings: Res<TextureSettings>,
    combined_texture: Res<CombinedTextureState>,
    mesh_query: Query<&Handle<ColorMaterial>, With<AntMeshMarker>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if texture_settings.is_changed() || combined_texture.is_changed() {
        if let Ok(material_handle) = mesh_query.get_single() {
            if let Some(material) = materials.get_mut(material_handle) {
                if texture_settings.enabled && combined_texture.created {
                    material.texture = combined_texture.handle.clone();
                } else {
                    material.texture = None;
                }
            }
        }
    }
}
