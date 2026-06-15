use bevy::prelude::*;

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

#[derive(Component, Debug)]
pub struct AntMemory {
    pub positions: [Vec2; 10],
    pub index: usize,
}

#[derive(Component, Debug)]
#[allow(dead_code)]
pub struct PheromoneCooldown {
    pub write_blocked_until: u64,
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

#[derive(Component)]
pub struct GroundGridParent;

#[derive(Component)]
pub struct AntMeshMarker;
