use bevy::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

// Map boundaries and configuration
pub const MAP_MIN_X: f32 = -350.0;
pub const MAP_MAX_X: f32 = 550.0;
pub const MAP_MIN_Y: f32 = -250.0;
pub const MAP_MAX_Y: f32 = 350.0;
pub const GRID_SIZE: f32 = 16.0;

// Food cells are effectively infinite sources
pub const FOOD_STORAGE: f32 = 999_999.0;

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
        // Ensure the maps/ directory exists
        let _ = std::fs::create_dir_all("maps");

        let mut grid = Self {
            cells: Self::generate_default_cells(),
        };

        let default_path = "maps/default.txt";
        if std::path::Path::new(default_path).exists() {
            if let Err(e) = grid.load_from_file(default_path) {
                eprintln!(
                    "Failed to load default map file: {}. Falling back to programmatic setup.",
                    e
                );
            }
        } else {
            // Write the generated default map layout to default.txt
            if let Err(e) = grid.save_to_file(default_path) {
                eprintln!("Failed to save default map file: {}", e);
            }
        }

        grid
    }

    // Programmatic default cells setup used as fallback and to write default.txt
    pub fn generate_default_cells() -> Vec<GridCell> {
        let mut cells = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT);

        for y in MIN_YG..=MAX_YG {
            for x in MIN_XG..=MAX_XG {
                let mut passable = true;
                let mut cell_type = GridCellType::Empty;
                let mut food_storage = 0.0;

                // Borders are walls
                if x == MIN_XG || x == MAX_XG || y == MIN_YG || y == MAX_YG {
                    passable = false;
                    cell_type = GridCellType::Wall;
                } else {
                    // Spawn cave (home) gx: 3..=7, gy: 2..=6
                    if (3..=7).contains(&x) && (2..=6).contains(&y) {
                        cell_type = GridCellType::Cave;
                    }
                    // Spawn food sources
                    else if ((20..=22).contains(&x) && (10..=12).contains(&y))
                        || ((18..=20).contains(&x) && (-10..=-8).contains(&y))
                        || ((-15..=-13).contains(&x) && (8..=10).contains(&y))
                    {
                        cell_type = GridCellType::Food;
                        food_storage = FOOD_STORAGE;
                    }
                    // Obstacles
                    else if (x == 12 && (-5..=15).contains(&y)) || (x == -5 && (-12..=3).contains(&y)) {
                        passable = false;
                        cell_type = GridCellType::Wall;
                    }
                }

                cells.push(GridCell {
                    passable,
                    cell_type,
                    food_storage,
                    portal_link: None,
                    food_pheromone: AtomicU64::new(0),
                    cave_pheromone: AtomicU64::new(0),
                    gx: x,
                    gy: y,
                });
            }
        }
        cells
    }

    #[inline]
    pub fn get_cell_index(&self, gx: i32, gy: i32) -> Option<usize> {
        if !(MIN_XG..=MAX_XG).contains(&gx) || !(MIN_YG..=MAX_YG).contains(&gy) {
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
        self.get_cell(gx, gy).is_some_and(|c| c.passable)
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

            let mut current = atomic.load(Ordering::Relaxed);
            loop {
                let (old_time, _) = unpack_pheromone(current);
                if time <= old_time {
                    break;
                }

                match atomic.compare_exchange_weak(current, new_val, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(actual) => current = actual,
                }
            }
        }
    }

    // Save grid layout to an ASCII Art text file
    pub fn save_to_file(&self, filepath: &str) -> Result<(), std::io::Error> {
        let mut map_str = String::new();
        // Traverse top to bottom, left to right
        for gy in (MIN_YG..=MAX_YG).rev() {
            for gx in MIN_XG..=MAX_XG {
                let cell = self.get_cell(gx, gy).unwrap();
                let ch = match cell.cell_type {
                    GridCellType::Wall => '#',
                    GridCellType::Cave => 'C',
                    GridCellType::Food => 'F',
                    GridCellType::Grass => 'g',
                    GridCellType::Portal(PortalColor::Blue) => 'P',
                    GridCellType::Portal(PortalColor::Orange) => 'p',
                    GridCellType::Empty => '.',
                };
                map_str.push(ch);
            }
            map_str.push('\n');
        }
        std::fs::write(filepath, map_str)
    }

    // Load grid layout from an ASCII Art text file
    pub fn load_from_file(&mut self, filepath: &str) -> Result<(), std::io::Error> {
        let content = std::fs::read_to_string(filepath)?;
        self.load_from_str(&content)
            .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {}", filepath, e)))
    }

    // Parse an ASCII Art map. The whole map is validated before any cell is touched,
    // so a malformed file leaves the current grid unchanged.
    pub fn load_from_str(&mut self, content: &str) -> Result<(), std::io::Error> {
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

        if lines.len() != GRID_HEIGHT {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid map height: expected {}, got {}", GRID_HEIGHT, lines.len()),
            ));
        }

        let mut parsed = Vec::with_capacity(GRID_WIDTH * GRID_HEIGHT);
        for (y_idx, line) in lines.iter().enumerate() {
            let gy = MAX_YG - y_idx as i32; // Match the top-to-bottom .rev() order
            let chars: Vec<char> = line.trim_end().chars().collect();

            if chars.len() != GRID_WIDTH {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "invalid map width at row {}: expected {}, got {}",
                        y_idx,
                        GRID_WIDTH,
                        chars.len()
                    ),
                ));
            }

            for (x_idx, &ch) in chars.iter().enumerate() {
                let gx = MIN_XG + x_idx as i32;
                let cell_type = match ch {
                    '#' => GridCellType::Wall,
                    'C' => GridCellType::Cave,
                    'F' => GridCellType::Food,
                    'g' => GridCellType::Grass,
                    'P' => GridCellType::Portal(PortalColor::Blue),
                    'p' => GridCellType::Portal(PortalColor::Orange),
                    _ => GridCellType::Empty,
                };
                parsed.push((gx, gy, cell_type));
            }
        }

        for (gx, gy, cell_type) in parsed {
            if let Some(idx) = self.get_cell_index(gx, gy) {
                let cell = &mut self.cells[idx];
                cell.passable = cell_type != GridCellType::Wall;
                cell.cell_type = cell_type;
                cell.food_storage = if cell_type == GridCellType::Food {
                    FOOD_STORAGE
                } else {
                    0.0
                };
                cell.portal_link = None;

                // Clear pheromones on load to start simulation fresh
                cell.food_pheromone.store(0, Ordering::Relaxed);
                cell.cave_pheromone.store(0, Ordering::Relaxed);
            }
        }

        // Link portals if they exist
        let mut blue_portal = None;
        let mut orange_portal = None;
        for cell in self.cells.iter() {
            if cell.cell_type == GridCellType::Portal(PortalColor::Blue) {
                blue_portal = Some((cell.gx, cell.gy));
            } else if cell.cell_type == GridCellType::Portal(PortalColor::Orange) {
                orange_portal = Some((cell.gx, cell.gy));
            }
        }
        if let (Some(blue), Some(orange)) = (blue_portal, orange_portal) {
            for cell in self.cells.iter_mut() {
                if cell.cell_type == GridCellType::Portal(PortalColor::Blue) {
                    cell.portal_link = Some(orange);
                } else if cell.cell_type == GridCellType::Portal(PortalColor::Orange) {
                    cell.portal_link = Some(blue);
                }
            }
        }

        Ok(())
    }

    // Compute the average center position of all Cave cells
    pub fn get_cave_center(&self) -> Vec2 {
        let mut sum = Vec2::ZERO;
        let mut count = 0;
        for cell in self.cells.iter() {
            if cell.cell_type == GridCellType::Cave {
                let cx = (cell.gx as f32 + 0.5) * GRID_SIZE;
                let cy = (cell.gy as f32 + 0.5) * GRID_SIZE;
                sum += Vec2::new(cx, cy);
                count += 1;
            }
        }
        if count > 0 {
            sum / count as f32
        } else {
            // Fallback cave center
            Vec2::new(5.0 * GRID_SIZE, 4.0 * GRID_SIZE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_grid() -> SwarmGrid {
        SwarmGrid {
            cells: SwarmGrid::generate_default_cells(),
        }
    }

    #[test]
    fn pheromone_pack_roundtrip() {
        let pos = Vec2::new(123.4, -56.7);
        let (time, unpacked) = unpack_pheromone(pack_pheromone(42_000, pos));
        assert_eq!(time, 42_000);
        assert!((unpacked - pos).length() < 0.2);
    }

    #[test]
    fn cell_index_bounds() {
        let grid = default_grid();
        assert_eq!(grid.cells.len(), GRID_WIDTH * GRID_HEIGHT);
        assert_eq!(grid.get_cell_index(MIN_XG, MIN_YG), Some(0));
        assert_eq!(grid.get_cell_index(MAX_XG, MAX_YG), Some(GRID_WIDTH * GRID_HEIGHT - 1));
        assert_eq!(grid.get_cell_index(MIN_XG - 1, 0), None);
        assert_eq!(grid.get_cell_index(0, MAX_YG + 1), None);
        let cell = grid.get_cell(7, -3).unwrap();
        assert_eq!((cell.gx, cell.gy), (7, -3));
    }

    #[test]
    fn pheromone_update_keeps_newest() {
        let grid = default_grid();
        grid.update_pheromone(0, 0, true, 10, Vec2::new(1.0, 1.0));
        grid.update_pheromone(0, 0, true, 5, Vec2::new(9.0, 9.0));
        let (time, pos) = unpack_pheromone(grid.get_cell(0, 0).unwrap().food_pheromone.load(Ordering::Relaxed));
        assert_eq!(time, 10);
        assert!((pos - Vec2::new(1.0, 1.0)).length() < 0.2);
        let (cave_time, _) = unpack_pheromone(grid.get_cell(0, 0).unwrap().cave_pheromone.load(Ordering::Relaxed));
        assert_eq!(cave_time, 0);
    }

    #[test]
    fn map_save_load_roundtrip() {
        let mut grid = default_grid();
        let dir = std::env::temp_dir().join(format!("locas-ants-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("map.txt");
        let path = path.to_str().unwrap();

        grid.save_to_file(path).unwrap();
        grid.load_from_file(path).unwrap();
        let reference = SwarmGrid::generate_default_cells();
        for (a, b) in grid.cells.iter().zip(reference.iter()) {
            assert_eq!(a.cell_type, b.cell_type);
            assert_eq!(a.passable, b.passable);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_map_leaves_grid_untouched() {
        let mut grid = default_grid();
        let mut rows = vec!["#".repeat(GRID_WIDTH); GRID_HEIGHT];
        rows[GRID_HEIGHT - 1] = "#".repeat(GRID_WIDTH - 1); // last row too short
        assert!(grid.load_from_str(&rows.join("\n")).is_err());
        assert_eq!(grid.get_cell(0, 0).unwrap().cell_type, GridCellType::Empty);
    }

    #[test]
    fn portals_are_linked_on_load() {
        let mut grid = default_grid();
        let mut rows: Vec<String> = vec![".".repeat(GRID_WIDTH); GRID_HEIGHT];
        rows[5].replace_range(3..4, "P");
        rows[20].replace_range(40..41, "p");
        grid.load_from_str(&rows.join("\r\n")).unwrap();

        let blue = (MIN_XG + 3, MAX_YG - 5);
        let orange = (MIN_XG + 40, MAX_YG - 20);
        assert_eq!(grid.get_cell(blue.0, blue.1).unwrap().portal_link, Some(orange));
        assert_eq!(grid.get_cell(orange.0, orange.1).unwrap().portal_link, Some(blue));
    }
}
