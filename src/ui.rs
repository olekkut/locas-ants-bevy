use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rand::Rng;
use crate::interaction::{ActiveTool, ToolMode};
use crate::systems::{DebugSettings, SimulationSpeed, TextureSettings, SimulationFrame};
use crate::components::*;
use crate::grid::GRID_SIZE;

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
pub struct AntCountText;

#[derive(Component)]
pub struct SpeedText;

#[derive(Component)]
pub struct TimeCountText;

#[derive(Component, Clone, Copy)]
pub enum UiButton {
    ZoomIn,
    ZoomOut,
    Tool(ToolMode),
    TogglePheromones,
    ToggleTextures,
    AntsInc,
    AntsDec,
    SpeedInc,
    SpeedDec,
}

pub fn setup_ui(mut commands: Commands) {
    // Spawn root sidebar container
    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Px(110.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Start,
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            background_color: Color::srgb(0.32, 0.32, 0.32).into(),
            z_index: ZIndex::Global(10), // Render on top of the simulation
            ..default()
        })
        .with_children(|parent| {
            // FPS Label
            parent.spawn((
                TextBundle::from_section(
                    "0 FPS",
                    TextStyle {
                        font: Default::default(),
                        font_size: 16.0,
                        color: Color::srgb(0.6, 0.8, 0.5),
                    },
                )
                .with_style(Style {
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                }),
                FpsText,
            ));

            // Ant Count Label
            parent.spawn((
                TextBundle::from_section(
                    "0 ants",
                    TextStyle {
                        font: Default::default(),
                        font_size: 16.0,
                        color: Color::srgb(0.6, 0.8, 0.5),
                    },
                )
                .with_style(Style {
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                }),
                AntCountText,
            ));

            // Speed Label
            parent.spawn((
                TextBundle::from_section(
                    "1x speed",
                    TextStyle {
                        font: Default::default(),
                        font_size: 16.0,
                        color: Color::srgb(0.6, 0.8, 0.5),
                    },
                )
                .with_style(Style {
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                }),
                SpeedText,
            ));

            // Time Count Label
            parent.spawn((
                TextBundle::from_section(
                    "Time: 0.0s",
                    TextStyle {
                        font: Default::default(),
                        font_size: 16.0,
                        color: Color::srgb(0.6, 0.8, 0.5),
                    },
                )
                .with_style(Style {
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                }),
                TimeCountText,
            ));

            // Helper to spawn buttons
            let spawn_button = |parent: &mut ChildBuilder, text: &str, action: UiButton| {
                parent.spawn((
                    ButtonBundle {
                        style: Style {
                            width: Val::Px(90.0),
                            height: Val::Px(32.0),
                            margin: UiRect::bottom(Val::Px(6.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        background_color: Color::srgb(0.18, 0.18, 0.18).into(),
                        ..default()
                    },
                    action,
                )).with_children(|btn| {
                    btn.spawn(TextBundle::from_section(
                        text,
                        TextStyle {
                            font: Default::default(),
                            font_size: 13.0,
                            color: Color::WHITE,
                        },
                    ));
                });
            };

            spawn_button(parent, "zoom+", UiButton::ZoomIn);
            spawn_button(parent, "zoom-", UiButton::ZoomOut);
            
            spawn_button(parent, "ants+", UiButton::AntsInc);
            spawn_button(parent, "ants-", UiButton::AntsDec);
            
            spawn_button(parent, "speed+", UiButton::SpeedInc);
            spawn_button(parent, "speed-", UiButton::SpeedDec);
            
            spawn_button(parent, "pan view", UiButton::Tool(ToolMode::PanView));
            spawn_button(parent, "block", UiButton::Tool(ToolMode::Block));
            spawn_button(parent, "grass", UiButton::Tool(ToolMode::Grass));
            spawn_button(parent, "cave", UiButton::Tool(ToolMode::Cave));
            spawn_button(parent, "food", UiButton::Tool(ToolMode::Food));
            spawn_button(parent, "portal", UiButton::Tool(ToolMode::Portal));
            spawn_button(parent, "remove", UiButton::Tool(ToolMode::Remove));
            
            spawn_button(parent, "phrms", UiButton::TogglePheromones);
            spawn_button(parent, "txtrs", UiButton::ToggleTextures);
        });
}

pub fn ui_button_interaction_system(
    mut commands: Commands,
    interaction_query: Query<
        (&Interaction, &UiButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut active_tool: ResMut<ActiveTool>,
    mut debug_settings: ResMut<DebugSettings>,
    mut camera_query: Query<&mut OrthographicProjection>,
    ants_query: Query<Entity, With<AntPosition>>,
    mut simulation_speed: ResMut<SimulationSpeed>,
    mut texture_settings: ResMut<TextureSettings>,
) {
    if let Ok(mut projection) = camera_query.get_single_mut() {
        for (interaction, action) in interaction_query.iter() {
            if let Interaction::Pressed = *interaction {
                match action {
                    UiButton::ZoomIn => {
                        projection.scale = (projection.scale - 0.25).clamp(0.25, 4.0);
                    }
                    UiButton::ZoomOut => {
                        projection.scale = (projection.scale + 0.25).clamp(0.25, 4.0);
                    }
                    UiButton::Tool(mode) => {
                        active_tool.mode = *mode;
                    }
                    UiButton::TogglePheromones => {
                        debug_settings.show_pheromones = !debug_settings.show_pheromones;
                    }
                    UiButton::ToggleTextures => {
                        texture_settings.enabled = !texture_settings.enabled;
                    }
                    UiButton::SpeedInc => {
                        simulation_speed.multiplier = match simulation_speed.multiplier {
                            0 => 1,
                            1 => 2,
                            2 => 4,
                            4 => 8,
                            _ => 8,
                        };
                    }
                    UiButton::SpeedDec => {
                        simulation_speed.multiplier = match simulation_speed.multiplier {
                            8 => 4,
                            4 => 2,
                            2 => 1,
                            1 => 0,
                            _ => 0,
                        };
                    }
                    UiButton::AntsInc => {
                        let mut rng = rand::thread_rng();
                        let cave_center_x = 5.0 * GRID_SIZE;
                        let cave_center_y = 4.0 * GRID_SIZE;
                        
                        for _ in 0..1000 {
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
                    }
                    UiButton::AntsDec => {
                        let ants: Vec<Entity> = ants_query.iter().collect();
                        let count = ants.len();
                        let num_to_despawn = 1000.min(count);
                        for i in 0..num_to_despawn {
                            commands.entity(ants[i]).despawn_recursive();
                        }
                    }
                }
            }
        }
    }
}

pub fn ui_update_text_system(
    diagnostics: Res<DiagnosticsStore>,
    ants_query: Query<&AntPosition>,
    active_tool: Res<ActiveTool>,
    debug_settings: Res<DebugSettings>,
    simulation_speed: Res<SimulationSpeed>,
    texture_settings: Res<TextureSettings>,
    frame: Res<SimulationFrame>,
    mut fps_query: Query<&mut Text, (With<FpsText>, Without<AntCountText>, Without<SpeedText>, Without<TimeCountText>)>,
    mut ants_text_query: Query<&mut Text, (With<AntCountText>, Without<FpsText>, Without<SpeedText>, Without<TimeCountText>)>,
    mut speed_text_query: Query<&mut Text, (With<SpeedText>, Without<FpsText>, Without<AntCountText>, Without<TimeCountText>)>,
    mut time_text_query: Query<&mut Text, (With<TimeCountText>, Without<FpsText>, Without<AntCountText>, Without<SpeedText>)>,
    mut button_query: Query<(&UiButton, &mut BackgroundColor, &Interaction)>,
) {
    // 1. Update FPS Text
    if let Ok(mut text) = fps_query.get_single_mut() {
        if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                text.sections[0].value = format!("{:.0} FPS", value);
            }
        }
    }
    
    // 2. Update Ant Count Text
    if let Ok(mut text) = ants_text_query.get_single_mut() {
        text.sections[0].value = format!("{} ants", ants_query.iter().len());
    }

    // 3. Update Speed Text
    if let Ok(mut text) = speed_text_query.get_single_mut() {
        if simulation_speed.multiplier == 0 {
            text.sections[0].value = "Paused".to_string();
        } else {
            text.sections[0].value = format!("{}x speed", simulation_speed.multiplier);
        }
    }

    // 4. Update Time Count Text
    if let Ok(mut text) = time_text_query.get_single_mut() {
        let seconds = frame.frame_number as f32 / 60.0;
        text.sections[0].value = format!("Time: {:.1}s", seconds);
    }
    
    // 5. Update Button Highlight Colors
    for (button_type, mut bg_color, interaction) in button_query.iter_mut() {
        let is_selected = match button_type {
            UiButton::Tool(mode) => active_tool.mode == *mode,
            UiButton::TogglePheromones => debug_settings.show_pheromones,
            UiButton::ToggleTextures => texture_settings.enabled,
            _ => false,
        };
        
        let target_color = if is_selected {
            Color::srgb(0.21, 0.44, 0.55) // Highlighted selection color
        } else {
            match *interaction {
                Interaction::Pressed => Color::srgb(0.12, 0.12, 0.12),
                Interaction::Hovered => Color::srgb(0.26, 0.26, 0.26),
                Interaction::None => Color::srgb(0.18, 0.18, 0.18),
            }
        };
        
        *bg_color = target_color.into();
    }
}
