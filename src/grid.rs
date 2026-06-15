use bevy::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

// Map boundaries and configuration
pub const MAP_MIN_X: f32 = -350.0;
pub const MAP_MAX_X: f32 = 550.0;
pub const MAP_MIN_Y: f32 = -250.0;
pub const MAP_MAX_Y: f32 = 350.0;
pub const GRID_SIZE: f32 = 16.0;

// Calculated grid bounds
pub const MIN_XG: i32 = -24;
pub const MAX_XG: i32 = 36;
pub const MIN_YG: i32 = -18;
pub const MAX_YG: i32 = 24;

pub const GRID_WIDTH: usize = (MAX_XG - MIN_XG + 1) as usize; // 61
pub const GRID_HEIGHT: usize = (MAX_YG - MIN_YG + 1) as usize; // 43

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortalColor {
    Blue,
    Orange,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GridCellType {
    Empty,
    Wall,
    Food,
    Cave,
    Grass,
    Portal(PortalColor),
}

pub struct GridCell {
    pub passable: bool,
    pub cell_type: GridCellType,
    pub food_storage: f32,
    pub portal_link: Option<(i32, i32)>,
    // Atomic packing for pheromones:
    // Bits 0..31: Time (u32)
    // Bits 32..47: X coordinate (u16)
    // Bits 48..63: Y coordinate (u16)
    pub food_pheromone: AtomicU64,
    pub cave_pheromone: AtomicU64,
    pub gx: i32,
    pub gy: i32,
}

impl Default for GridCell {
    fn default() -> Self {
        Self {
            passable: true,
            cell_type: GridCellType::Empty,
            food_storage: 0.0,
            portal_link: None,
            food_pheromone: AtomicU64::new(pack_pheromone(0, Vec2::ZERO)),
            cave_pheromone: AtomicU64::new(pack_pheromone(0, Vec2::ZERO)),
            gx: 0,
            gy: 0,
        }
    }
}

// Convert float coordinates to/from packed integer representation
pub fn pack_pheromone(time: u32, pos: Vec2) -> u64 {
    // Offset position to keep it positive and scale to preserve precision
    let x_val = (((pos.x - MAP_MIN_X) * 10.0).clamp(0.0, 65535.0) as u16) as u64;
    let y_val = (((pos.y - MAP_MIN_Y) * 10.0).clamp(0.0, 65535.0) as u16) as u64;
    (time as u64) | (x_val << 32) | (y_val << 48)
}

pub fn unpack_pheromone(packed: u64) -> (u32, Vec2) {
    let time = (packed & 0xFFFFFFFF) as u32;
    let x_val = ((packed >> 32) & 0xFFFF) as u16;
    let y_val = ((packed >> 48) & 0xFFFF) as u16;
    let x = (x_val as f32) / 10.0 + MAP_MIN_X;
    let y = (y_val as f32) / 10.0 + MAP_MIN_Y;
    (time, Vec2::new(x, y))
}

#[derive(Resource)]
pub struct SwarmGrid {
    pub cells: Vec<GridCell>,
}

impl SwarmGrid {
    pub fn new() -> Self {
        let mut cells = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT);
        
        for y in MIN_YG..=MAX_YG {
            for x in MIN_XG..=MAX_XG {
                // Determine cell type and passability
                let mut passable = true;
                let mut cell_type = GridCellType::Empty;
                let mut food_storage = 0.0;
                
                // Borders are walls
                if x == MIN_XG || x == MAX_XG || y == MIN_YG || y == MAX_YG {
                    passable = false;
                    cell_type = GridCellType::Wall;
                } else {
                    // Let's spawn a cave (home) around center (gx: 5, gy: 5)
                    if x >= 3 && x <= 7 && y >= 2 && y <= 6 {
                        cell_type = GridCellType::Cave;
                    }
                    // Let's spawn some food sources
                    else if (x >= 20 && x <= 22 && y >= 10 && y <= 12) 
                         || (x >= 18 && x <= 20 && y >= -10 && y <= -8) 
                         || (x >= -15 && x <= -13 && y >= 8 && y <= 10) 
                    {
                        cell_type = GridCellType::Food;
                        food_storage = 999999.0; // effectively infinite food source
                    }
                    // Add some static obstacles
                    else if (x == 12 && y >= -5 && y <= 15) 
                         || (x == -5 && y >= -12 && y <= 3)
                    {
                        passable = false;
                        cell_type = GridCellType::Wall;
                    }
                }
                
                cells.push(GridCell {
                    passable,
                    cell_type,
                    food_storage,
                    portal_link: None,
                    food_pheromone: AtomicU64::new(0), // initial time = 0
                    cave_pheromone: AtomicU64::new(0), // initial time = 0
                    gx: x,
                    gy: y,
                });
            }
        }
        
        Self { cells }
    }
    
    #[inline]
    pub fn get_cell_index(&self, gx: i32, gy: i32) -> Option<usize> {
        if gx < MIN_XG || gx > MAX_XG || gy < MIN_YG || gy > MAX_YG {
            None
        } else {
            let x_idx = (gx - MIN_XG) as usize;
            let y_idx = (gy - MIN_YG) as usize;
            Some(y_idx * GRID_WIDTH + x_idx)
        }
    }
    
    #[inline]
    pub fn get_cell(&self, gx: i32, gy: i32) -> Option<&GridCell> {
        self.get_cell_index(gx, gy).map(|idx| &self.cells[idx])
    }
    
    #[inline]
    pub fn get_grid_coords(&self, pos: Vec2) -> (i32, i32) {
        let gx = (pos.x / GRID_SIZE).floor() as i32;
        let gy = (pos.y / GRID_SIZE).floor() as i32;
        (gx, gy)
    }
    
    #[inline]
    pub fn is_passable(&self, pos: Vec2) -> bool {
        let (gx, gy) = self.get_grid_coords(pos);
        self.get_cell(gx, gy).map_or(false, |c| c.passable)
    }
    
    // Updates a cell's pheromone using lock-free CAS (Compare-And-Swap) atomic update
    pub fn update_pheromone(&self, gx: i32, gy: i32, is_food: bool, time: u32, pos: Vec2) {
        if let Some(cell) = self.get_cell(gx, gy) {
            let atomic = if is_food {
                &cell.food_pheromone
            } else {
                &cell.cave_pheromone
            };
            
            let new_val = pack_pheromone(time, pos);
            
            // Loop until CAS succeeds or we see a newer timestamp
            let mut current = atomic.load(Ordering::Relaxed);
            loop {
                let (old_time, _) = unpack_pheromone(current);
                if time <= old_time {
                    break; // The stored pheromone is already newer or equal
                }
                
                match atomic.compare_exchange_weak(
                    current,
                    new_val,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break, // Successfully updated!
                    Err(actual) => current = actual, // Try again with the updated value
                }
            }
        }
    }
}
