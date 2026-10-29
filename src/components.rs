use bevy::prelude::*;
use rand::Rng;

#[derive(Component, Clone, Copy, Debug)]
pub struct AntPosition(pub Vec2);

#[derive(Component, Clone, Copy, Debug)]
pub struct AntDirection(pub Vec2);

#[derive(Component, Clone, Copy, Debug)]
pub struct AntSpeed(pub f32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Food,
    Cave,
}

#[derive(Component, Debug)]
pub struct AntState {
    pub looking_for: Target,
    pub last_time_seen_food: i64,
    pub last_time_seen_cave: i64,
    pub max_time_seen: i64,
    pub pheromones_write: bool,
    pub pheromones_back_time: u64,
    pub teleported_on_frame: u64,
    pub traveled: f32,
}

impl Default for AntState {
    fn default() -> Self {
        Self {
            looking_for: Target::Food,
            last_time_seen_food: -1,
            last_time_seen_cave: -1,
            max_time_seen: -1,
            pheromones_write: true,
            pheromones_back_time: 0,
            teleported_on_frame: 0,
            traveled: 0.0,
        }
    }
}

#[derive(Component, Debug)]
pub struct AntMemory {
    pub positions: [Vec2; 10],
    pub index: usize,
}

#[derive(Component, Debug)]
pub struct AntPhysics {
    pub acceleration: f32,
    pub erratic: f32,
    pub max_speed: f32,
}

#[derive(Component, Debug)]
pub struct AntComNeeded {
    pub com_every: u32,
    pub com_offset: u32,
}

// Builds the full component set of a fresh ant spawned around `center`
pub fn new_ant_bundle(center: Vec2, rng: &mut impl Rng) -> impl Bundle {
    let radius = rng.gen_range(0.0..15.0);
    let theta = rng.gen_range(0.0..std::f32::consts::TAU);
    let pos = center + Vec2::new(theta.cos(), theta.sin()) * radius;

    let move_angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let dir = Vec2::new(move_angle.cos(), move_angle.sin());

    (
        AntPosition(pos),
        AntDirection(dir),
        AntSpeed(0.0),
        AntPhysics {
            acceleration: rng.gen_range(0.03..0.07),
            erratic: 0.2,
            max_speed: 1.2,
        },
        AntState::default(),
        AntMemory {
            positions: [pos; 10],
            index: 0,
        },
        AntComNeeded {
            com_every: rng.gen_range(3..=13),
            com_offset: rng.gen_range(0..13),
        },
    )
}

#[derive(Component)]
pub struct GridSprite;

#[derive(Component)]
pub struct GroundGridParent;

#[derive(Component)]
pub struct AntMeshMarker;
