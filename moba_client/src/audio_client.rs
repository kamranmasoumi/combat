use bevy::audio::{SpatialListener, Volume};
use bevy::prelude::*;
use moba_shared::{GameConfig, ImpactSurface, ShotAudioSource, WeaponKind};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AudioCategory {
    Music,
    Ambience,
    Weapons,
    Movement,
    Units,
    Ui,
    Alerts,
}

impl AudioCategory {
    pub const ALL: [Self; 7] = [
        Self::Music,
        Self::Ambience,
        Self::Weapons,
        Self::Movement,
        Self::Units,
        Self::Ui,
        Self::Alerts,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Music => "Music",
            Self::Ambience => "Ambience",
            Self::Weapons => "Weapons & Impacts",
            Self::Movement => "Footsteps & Moves",
            Self::Units => "Creeps & Turrets",
            Self::Ui => "Shop & UI",
            Self::Alerts => "Alerts & Heartbeat",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Music => 0,
            Self::Ambience => 1,
            Self::Weapons => 2,
            Self::Movement => 3,
            Self::Units => 4,
            Self::Ui => 5,
            Self::Alerts => 6,
        }
    }
}

#[derive(Resource, Clone, Debug)]
pub struct AudioSettingsState {
    pub master_volume: f32,
    pub master_muted: bool,
    pub channel_volumes: [f32; 7],
    pub channel_muted: [bool; 7],
    pub dirty: bool,
}

impl AudioSettingsState {
    pub fn from_config(cfg: &moba_shared::AudioConfig) -> Self {
        Self {
            master_volume: cfg.master_volume.clamp(0.0, 1.0),
            master_muted: false,
            channel_volumes: [
                cfg.music_volume.clamp(0.0, 1.0),
                cfg.ambience_volume.clamp(0.0, 1.0),
                cfg.weapons_volume.clamp(0.0, 1.0),
                cfg.movement_volume.clamp(0.0, 1.0),
                cfg.units_volume.clamp(0.0, 1.0),
                cfg.ui_volume.clamp(0.0, 1.0),
                cfg.alerts_volume.clamp(0.0, 1.0),
            ],
            channel_muted: [false; 7],
            dirty: true,
        }
    }

    pub fn reset_from_config(&mut self, cfg: &moba_shared::AudioConfig) {
        *self = Self::from_config(cfg);
    }

    pub fn effective_gain(&self, cat: AudioCategory, low_hp_duck: f32) -> f32 {
        if self.master_muted || self.channel_muted[cat.index()] {
            return 0.0;
        }
        let ch_v = self.channel_volumes[cat.index()].clamp(0.0, 1.0);
        // افکت بم و گنگ شدن صدای محیط و نبرد هنگام کم بودن سلامتی (ضربان قلب و هشدارها دک نمی‌شوند)
        let duck = match cat {
            AudioCategory::Alerts | AudioCategory::Ui => 1.0,
            _ => low_hp_duck,
        };
        (self.master_volume * ch_v * duck).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub enum ClientAudioEvent {
    ShotFired {
        pos: Vec3,
        source: ShotAudioSource,
    },
    TurretShotFired {
        pos: Vec3,
    },
    BulletImpact {
        pos: Vec3,
        surface: ImpactSurface,
    },
    GrenadeBeep {
        pos: Vec3,
    },
    GrenadeExploded {
        pos: Vec3,
    },
    LastHitGold,
    CreepDenied,
    HeroLeveledUp,
    WeaponSwitch,
    EmptyGunClick,
    EmptyGrenadeClick,
    PrimaryReload,
    PistolReload,
    GrenadeThrow,
    TraversalMove {
        anim_name: String,
    },
    HeroDeath,
    HeroRespawn,
    TurretDestroyed {
        pos: Vec3,
    },
    BuildingDestroyed {
        pos: Vec3,
    },
    CourierDeath,
    CourierRespawn,
    MatchStarted,
    MatchEnded {
        victory: bool,
    },
    ShopOpen,
    ShopClose,
    ShopAddCart,
    ShopBuy,
    ShopCombine,
    ShopSell,
}

#[derive(Resource, Default)]
pub struct AudioEventQueue(pub Vec<ClientAudioEvent>);

#[derive(Resource, Default)]
pub struct AudioFrameContext {
    pub in_game: bool,
    pub listener_pos: Vec3,
    pub hero_pos: Vec3,
    pub hero_hp_ratio: f32,
    pub hero_is_dead: bool,
    pub current_locomotion: String,
    pub in_water: bool,
    pub active_courier_pos: Option<Vec3>,
    pub running_creeps_nearest_pos: Option<Vec3>,
    pub running_creeps_count: usize,
}

#[derive(Resource, Default)]
struct AudioAssetCache(HashMap<String, Option<Handle<AudioSource>>>);

impl AudioAssetCache {
    fn get_or_load(&mut self, path: &str, asset_server: &AssetServer) -> Option<Handle<AudioSource>> {
        if path.trim().is_empty() {
            return None;
        }
        if let Some(cached) = self.0.get(path) {
            return cached.clone();
        }
        let exists = Path::new("assets").join(path).is_file()
            || Path::new("../../assets").join(path).is_file()
            || Path::new(path).is_file();
        let handle = if exists {
            Some(asset_server.load(path.to_string()))
        } else {
            println!("هشدار صدا: فایل صوتی '{}' در پوشه assets پیدا نشد!", path);
            None
        };
        self.0.insert(path.to_string(), handle.clone());
        handle
    }
}

#[derive(Component)]
struct TaggedAudio {
    category: AudioCategory,
    base_gain: f32,
}

#[derive(Component)]
struct BackgroundMusicLoop;

#[derive(Component)]
struct CourierFlyLoop;

#[derive(Component)]
struct CreepMarchLoop;

#[derive(Component)]
struct AmbientZoneLoop {
    zone_index: usize,
    base_volume: f32,
}

#[derive(Component)]
struct LowHpVignetteEdge;

#[derive(Resource, Default)]
struct AudioRuntimeTimers {
    step_timer: f32,
    step_index: usize,
    swim_timer: f32,
    shallow_water_timer: f32,
    heartbeat_timer: f32,
    low_hp_duck_factor: f32,
    active_creep_shots_window: usize,
    creep_window_timer: f32,
    prev_locomotion: String,
    listener_spawned: bool,
    music_spawned: bool,
    courier_loop_spawned: bool,
    creep_loop_spawned: bool,
    ambient_spawned: bool,
}

pub struct MobaAudioPlugin;

impl Plugin for MobaAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AudioEventQueue>()
            .init_resource::<AudioFrameContext>()
            .init_resource::<AudioAssetCache>()
            .init_resource::<AudioRuntimeTimers>()
            .add_systems(Startup, init_audio_settings_and_vignette_system)
            .add_systems(
                Update,
                (
                    process_audio_events_system,
                    update_locomotion_and_low_hp_audio_system,
                    update_loops_and_ambient_zones_system,
                    sync_audio_sink_volumes_system,
                )
                    .chain(),
            );
    }
}

fn init_audio_settings_and_vignette_system(
    mut commands: Commands,
    config: Res<GameConfig>,
) {
    commands.insert_resource(AudioSettingsState::from_config(&config.audio));

    // ۴ نوار حاشیه قرمز اطراف صفحه برای افکت کم شدن سلامتی
    let edge_thick = 22.0;
    for (left, right, top, bottom, width, height) in [
        (Some(px(0.0)), Some(px(0.0)), Some(px(0.0)), None, None, Some(px(edge_thick))),
        (Some(px(0.0)), Some(px(0.0)), None, Some(px(0.0)), None, Some(px(edge_thick))),
        (Some(px(0.0)), None, Some(px(0.0)), Some(px(0.0)), Some(px(edge_thick)), None),
        (None, Some(px(0.0)), Some(px(0.0)), Some(px(0.0)), Some(px(edge_thick)), None),
    ] {
        let mut node = Node {
            position_type: PositionType::Absolute,
            ..default()
        };
        if let Some(v) = left { node.left = v; }
        if let Some(v) = right { node.right = v; }
        if let Some(v) = top { node.top = v; }
        if let Some(v) = bottom { node.bottom = v; }
        if let Some(v) = width { node.width = v; }
        if let Some(v) = height { node.height = v; }

        commands.spawn((
            LowHpVignetteEdge,
            Pickable::IGNORE,
            GlobalZIndex(135),
            node,
            BackgroundColor(Color::srgba(0.85, 0.04, 0.04, 0.0)),
        ));
    }
}

fn play_2d_sound(
    commands: &mut Commands,
    asset_server: &AssetServer,
    cache: &mut AudioAssetCache,
    settings: &AudioSettingsState,
    duck: f32,
    cat: AudioCategory,
    path: &str,
    base_gain: f32,
) {
    let eff = settings.effective_gain(cat, duck) * base_gain;
    if eff <= 0.001 {
        return;
    }
    let Some(handle) = cache.get_or_load(path, asset_server) else {
        return;
    };
    commands.spawn((
        TaggedAudio {
            category: cat,
            base_gain,
        },
        AudioPlayer::new(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(eff)),
    ));
}

#[allow(clippy::too_many_arguments)]
fn play_spatial_sound(
    commands: &mut Commands,
    asset_server: &AssetServer,
    cache: &mut AudioAssetCache,
    settings: &AudioSettingsState,
    config: &GameConfig,
    listener_pos: Vec3,
    world_pos: Vec3,
    duck: f32,
    cat: AudioCategory,
    path: &str,
    base_gain: f32,
) {
    let dist = listener_pos.distance(world_pos);
    if dist > config.audio.max_hearing_distance {
        return;
    }
    let dist_atten = (1.0 - (dist / config.audio.max_hearing_distance)).clamp(0.0, 1.0);
    let gain = base_gain * dist_atten;
    let eff = settings.effective_gain(cat, duck) * gain;
    if eff <= 0.001 {
        return;
    }
    let Some(handle) = cache.get_or_load(path, asset_server) else {
        return;
    };
    commands.spawn((
        TaggedAudio {
            category: cat,
            base_gain: gain,
        },
        AudioPlayer::new(handle),
        PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(eff))
            .with_spatial(true),
        Transform::from_translation(world_pos * config.audio.spatial_scale),
    ));
}

#[allow(clippy::too_many_arguments)]
fn process_audio_events_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    mut cache: ResMut<AudioAssetCache>,
    settings: Res<AudioSettingsState>,
    ctx: Res<AudioFrameContext>,
    mut queue: ResMut<AudioEventQueue>,
    mut timers: ResMut<AudioRuntimeTimers>,
) {
    timers.creep_window_timer += time.delta_secs();
    if timers.creep_window_timer >= 0.25 {
        timers.creep_window_timer = 0.0;
        timers.active_creep_shots_window = 0;
    }

    if queue.0.is_empty() {
        return;
    }

    let duck = if timers.low_hp_duck_factor > 0.0 {
        timers.low_hp_duck_factor
    } else {
        1.0
    };
    let a = &config.audio;

    for ev in queue.0.drain(..) {
        match ev {
            ClientAudioEvent::ShotFired { pos, source } => match source {
                ShotAudioSource::Hero { hero_id, weapon_kind } => {
                    if let Some(hw) = a.hero_weapons.get(&hero_id) {
                        let path = if weapon_kind == WeaponKind::Pistol {
                            &hw.pistol_fire_sound
                        } else {
                            &hw.primary_fire_sound
                        };
                        play_spatial_sound(
                            &mut commands,
                            &asset_server,
                            &mut cache,
                            &settings,
                            &config,
                            ctx.listener_pos,
                            pos,
                            duck,
                            AudioCategory::Weapons,
                            path,
                            1.0,
                        );
                    }
                }
                ShotAudioSource::Creep => {
                    if timers.active_creep_shots_window < a.creep_fire_max_concurrent {
                        timers.active_creep_shots_window += 1;
                        let crowd_scale = 1.0
                            / (1.0
                                + (timers.active_creep_shots_window.saturating_sub(1) as f32)
                                    * a.creep_fire_crowd_ducking);
                        play_spatial_sound(
                            &mut commands,
                            &asset_server,
                            &mut cache,
                            &settings,
                            &config,
                            ctx.listener_pos,
                            pos,
                            duck,
                            AudioCategory::Units,
                            &a.creep_fire_sound,
                            crowd_scale,
                        );
                    }
                }
            },
            ClientAudioEvent::TurretShotFired { pos } => {
                play_spatial_sound(
                    &mut commands,
                    &asset_server,
                    &mut cache,
                    &settings,
                    &config,
                    ctx.listener_pos,
                    pos,
                    duck,
                    AudioCategory::Units,
                    &a.turret_fire_sound,
                    1.0,
                );
            }
            ClientAudioEvent::BulletImpact { pos, surface } => {
                let path = match surface {
                    ImpactSurface::StoneWall => &a.impact_stone_wall_sound,
                    ImpactSurface::Turret => &a.impact_turret_sound,
                    ImpactSurface::Character => &a.impact_character_sound,
                    ImpactSurface::Building => &a.impact_building_sound,
                    ImpactSurface::Water => &a.impact_water_sound,
                };
                play_spatial_sound(
                    &mut commands,
                    &asset_server,
                    &mut cache,
                    &settings,
                    &config,
                    ctx.listener_pos,
                    pos,
                    duck,
                    AudioCategory::Weapons,
                    path,
                    0.85,
                );
            }
            ClientAudioEvent::GrenadeBeep { pos } => {
                play_spatial_sound(
                    &mut commands,
                    &asset_server,
                    &mut cache,
                    &settings,
                    &config,
                    ctx.listener_pos,
                    pos,
                    duck,
                    AudioCategory::Weapons,
                    &a.grenade_countdown_beep_sound,
                    0.75,
                );
            }
            ClientAudioEvent::GrenadeExploded { pos } => {
                play_spatial_sound(
                    &mut commands,
                    &asset_server,
                    &mut cache,
                    &settings,
                    &config,
                    ctx.listener_pos,
                    pos,
                    duck,
                    AudioCategory::Weapons,
                    &a.grenade_explosion_sound,
                    1.0,
                );
            }
            ClientAudioEvent::LastHitGold => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.gold_gain_sound, 1.0);
            }
            ClientAudioEvent::CreepDenied => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.deny_sound, 1.0);
            }
            ClientAudioEvent::HeroLeveledUp => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, &a.level_up_sound, 1.0);
            }
            ClientAudioEvent::WeaponSwitch => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Weapons, &a.weapon_switch_sound, 0.9);
            }
            ClientAudioEvent::EmptyGunClick => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Weapons, &a.empty_gun_click_sound, 0.9);
            }
            ClientAudioEvent::EmptyGrenadeClick => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Weapons, &a.empty_grenade_click_sound, 0.9);
            }
            ClientAudioEvent::PrimaryReload => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Weapons, &a.primary_reload_sound, 1.0);
            }
            ClientAudioEvent::PistolReload => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Weapons, &a.pistol_reload_sound, 1.0);
            }
            ClientAudioEvent::GrenadeThrow => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Weapons, &a.grenade_throw_sound, 1.0);
            }
            ClientAudioEvent::TraversalMove { anim_name } => {
                let path = match anim_name.as_str() {
                    "run_to_dive" => &a.run_to_dive_sound,
                    "running_slide" => &a.running_slide_sound,
                    "quick_roll_to_run" => &a.quick_roll_to_run_sound,
                    "running_dive_roll" => &a.running_dive_roll_sound,
                    "jump_over" => &a.jump_over_sound,
                    _ => &a.obstacle_jump_sound,
                };
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Movement, path, 1.0);
            }
            ClientAudioEvent::HeroDeath => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, &a.hero_death_sound, 1.0);
            }
            ClientAudioEvent::HeroRespawn => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, &a.hero_respawn_sound, 1.0);
            }
            ClientAudioEvent::TurretDestroyed { pos } => {
                play_spatial_sound(&mut commands, &asset_server, &mut cache, &settings, &config, ctx.listener_pos, pos, duck, AudioCategory::Units, &a.turret_destroy_sound, 1.0);
            }
            ClientAudioEvent::BuildingDestroyed { pos } => {
                play_spatial_sound(&mut commands, &asset_server, &mut cache, &settings, &config, ctx.listener_pos, pos, duck, AudioCategory::Units, &a.building_destroy_sound, 1.0);
            }
            ClientAudioEvent::CourierDeath => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, &a.courier_death_sound, 1.0);
            }
            ClientAudioEvent::CourierRespawn => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, &a.courier_respawn_sound, 1.0);
            }
            ClientAudioEvent::MatchStarted => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, &a.match_start_sound, 1.0);
            }
            ClientAudioEvent::MatchEnded { victory } => {
                let path = if victory { &a.victory_sound } else { &a.defeat_sound };
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Alerts, path, 1.0);
            }
            ClientAudioEvent::ShopOpen => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.shop_open_sound, 0.9);
            }
            ClientAudioEvent::ShopClose => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.shop_close_sound, 0.9);
            }
            ClientAudioEvent::ShopAddCart => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.shop_add_cart_sound, 0.9);
            }
            ClientAudioEvent::ShopBuy => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.shop_buy_sound, 1.0);
            }
            ClientAudioEvent::ShopCombine => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.shop_combine_sound, 1.0);
            }
            ClientAudioEvent::ShopSell => {
                play_2d_sound(&mut commands, &asset_server, &mut cache, &settings, duck, AudioCategory::Ui, &a.shop_sell_sound, 1.0);
            }
        }
    }
}

fn update_locomotion_and_low_hp_audio_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    mut cache: ResMut<AudioAssetCache>,
    settings: Res<AudioSettingsState>,
    ctx: Res<AudioFrameContext>,
    mut timers: ResMut<AudioRuntimeTimers>,
    mut vignette_q: Query<&mut BackgroundColor, With<LowHpVignetteEdge>>,
) {
    let dt = time.delta_secs();
    let a = &config.audio;

    // ۱. محاسبه وضعیت کم بودن سلامتی (ضربان قلب + بم شدن صداهای محیط + افکت قرمز اطراف صفحه)
    let mut vignette_alpha = 0.0_f32;
    let mut target_duck = 1.0_f32;

    if ctx.in_game && !ctx.hero_is_dead && ctx.hero_hp_ratio < a.low_hp_threshold_ratio {
        let severity = ((a.low_hp_threshold_ratio - ctx.hero_hp_ratio) / a.low_hp_threshold_ratio)
            .clamp(0.0, 1.0);
        target_duck = 1.0 - severity * (1.0 - a.low_hp_world_duck_min_factor);

        let hb_interval = a.heartbeat_max_interval_secs
            - severity * (a.heartbeat_max_interval_secs - a.heartbeat_min_interval_secs);
        timers.heartbeat_timer -= dt;
        if timers.heartbeat_timer <= 0.0 {
            timers.heartbeat_timer = hb_interval.max(0.15);
            play_2d_sound(
                &mut commands,
                &asset_server,
                &mut cache,
                &settings,
                1.0,
                AudioCategory::Alerts,
                &a.heartbeat_sound,
                0.55 + severity * 0.45,
            );
        }

        let pulse = ((time.elapsed_secs() * (3.0 + severity * 5.0)).sin() * 0.5 + 0.5) * 0.35 + 0.65;
        vignette_alpha = severity * a.low_hp_vignette_max_alpha * pulse;
    } else {
        timers.heartbeat_timer = 0.0;
    }

    if timers.low_hp_duck_factor <= 0.0 {
        timers.low_hp_duck_factor = 1.0;
    }
    timers.low_hp_duck_factor = timers.low_hp_duck_factor + (target_duck - timers.low_hp_duck_factor) * (6.0 * dt).clamp(0.0, 1.0);

    for mut bg in &mut vignette_q {
        bg.0 = Color::srgba(0.86, 0.05, 0.05, vignette_alpha);
    }

    if !ctx.in_game || ctx.hero_is_dead {
        return;
    }

    // ۲. صدای ردپا، دویدن در آب کم‌عمق و شنا
    let loco = ctx.current_locomotion.as_str();
    let is_swimming = loco == "swimming";
    let is_moving_ground = loco.starts_with("run_") || loco.starts_with("walk_");

    if is_swimming {
        timers.swim_timer -= dt;
        if timers.swim_timer <= 0.0 {
            timers.swim_timer = a.swim_interval_secs;
            play_2d_sound(
                &mut commands,
                &asset_server,
                &mut cache,
                &settings,
                timers.low_hp_duck_factor,
                AudioCategory::Movement,
                &a.swim_stroke_sound,
                0.95,
            );
        }
    } else if is_moving_ground && ctx.in_water {
        timers.shallow_water_timer -= dt;
        if timers.shallow_water_timer <= 0.0 {
            timers.shallow_water_timer = a.shallow_water_step_interval_secs;
            play_2d_sound(
                &mut commands,
                &asset_server,
                &mut cache,
                &settings,
                timers.low_hp_duck_factor,
                AudioCategory::Movement,
                &a.shallow_water_step_sound,
                0.9,
            );
        }
    } else if is_moving_ground {
        let rate_mult = if loco.starts_with("walk_") { 1.35 } else { 1.0 };
        timers.step_timer -= dt;
        if timers.step_timer <= 0.0 {
            timers.step_timer = a.footstep_interval_secs * rate_mult;
            if !a.footstep_sounds.is_empty() {
                let path = &a.footstep_sounds[timers.step_index % a.footstep_sounds.len()];
                timers.step_index = (timers.step_index + 1) % a.footstep_sounds.len();
                play_2d_sound(
                    &mut commands,
                    &asset_server,
                    &mut cache,
                    &settings,
                    timers.low_hp_duck_factor,
                    AudioCategory::Movement,
                    path,
                    if loco.starts_with("walk_crouching") { 0.45 } else { 0.80 },
                );
            }
        }
    } else {
        timers.step_timer = 0.05;
        timers.swim_timer = 0.05;
        timers.shallow_water_timer = 0.05;
    }

    timers.prev_locomotion = ctx.current_locomotion.clone();
}

#[allow(clippy::too_many_arguments)]
fn update_loops_and_ambient_zones_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    mut cache: ResMut<AudioAssetCache>,
    settings: Res<AudioSettingsState>,
    ctx: Res<AudioFrameContext>,
    mut timers: ResMut<AudioRuntimeTimers>,
    mut listener_q: Query<&mut Transform, With<SpatialListener>>,
    mut courier_loop_q: Query<(&mut Transform, Option<&mut SpatialAudioSink>), (With<CourierFlyLoop>, Without<SpatialListener>, Without<CreepMarchLoop>)>,
    mut creep_loop_q: Query<(&mut Transform, Option<&mut SpatialAudioSink>), (With<CreepMarchLoop>, Without<SpatialListener>, Without<CourierFlyLoop>)>,
    mut ambient_q: Query<(&AmbientZoneLoop, Option<&mut AudioSink>)>,
) {
    let a = &config.audio;
    let duck = if timers.low_hp_duck_factor > 0.0 {
        timers.low_hp_duck_factor
    } else {
        1.0
    };

    // ۱. همگام‌سازی موقعیت شنونده فضایی (SpatialListener) با دوربین/فوکوس
    let scaled_listener = ctx.listener_pos * a.spatial_scale;
    if !timers.listener_spawned {
        commands.spawn((
            SpatialListener::new(0.25),
            Transform::from_translation(scaled_listener),
        ));
        timers.listener_spawned = true;
    } else if let Ok(mut ltf) = listener_q.single_mut() {
        ltf.translation = scaled_listener;
    }

    // ۲. لوپ موزیک پس‌زمینه بازی
    if ctx.in_game && !timers.music_spawned {
        if let Some(handle) = cache.get_or_load(&a.background_music_sound, &asset_server) {
            let vol = settings.effective_gain(AudioCategory::Music, duck);
            commands.spawn((
                BackgroundMusicLoop,
                TaggedAudio {
                    category: AudioCategory::Music,
                    base_gain: 1.0,
                },
                AudioPlayer::new(handle),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(vol)),
            ));
            timers.music_spawned = true;
        }
    }

    // ۳. لوپ صدای پرواز Courier
    if ctx.in_game && !timers.courier_loop_spawned {
        if let Some(handle) = cache.get_or_load(&a.courier_fly_loop_sound, &asset_server) {
            commands.spawn((
                CourierFlyLoop,
                AudioPlayer::new(handle),
                PlaybackSettings::LOOP
                    .with_volume(Volume::Linear(0.0))
                    .with_spatial(true),
                Transform::default(),
            ));
            timers.courier_loop_spawned = true;
        }
    }
    if let Ok((mut ctf, sink_opt)) = courier_loop_q.single_mut() {
        let mut target_vol = 0.0_f32;
        if let Some(c_pos) = ctx.active_courier_pos {
            ctf.translation = c_pos * a.spatial_scale;
            let dist = ctx.listener_pos.distance(c_pos);
            if dist < a.max_hearing_distance {
                let atten = (1.0 - dist / a.max_hearing_distance).clamp(0.0, 1.0);
                target_vol = settings.effective_gain(AudioCategory::Units, duck) * atten * 0.75;
            }
        }
        if let Some(mut sink) = sink_opt {
            sink.set_volume(Volume::Linear(target_vol));
        }
    }

    // ۴. لوپ صدای دویدن سربازان لاین (با کنترل هوشمند شلوغی و فاصله)
    if ctx.in_game && !timers.creep_loop_spawned {
        if let Some(handle) = cache.get_or_load(&a.creep_run_loop_sound, &asset_server) {
            commands.spawn((
                CreepMarchLoop,
                AudioPlayer::new(handle),
                PlaybackSettings::LOOP
                    .with_volume(Volume::Linear(0.0))
                    .with_spatial(true),
                Transform::default(),
            ));
            timers.creep_loop_spawned = true;
        }
    }
    if let Ok((mut cr_tf, sink_opt)) = creep_loop_q.single_mut() {
        let mut target_vol = 0.0_f32;
        if let Some(near_pos) = ctx.running_creeps_nearest_pos {
            cr_tf.translation = near_pos * a.spatial_scale;
            let dist = ctx.listener_pos.distance(near_pos);
            if dist < a.max_hearing_distance {
                let atten = (1.0 - dist / a.max_hearing_distance).clamp(0.0, 1.0);
                let count_factor = ((ctx.running_creeps_count as f32).sqrt() * 0.35).clamp(0.35, 1.0);
                target_vol = settings.effective_gain(AudioCategory::Units, duck) * atten * count_factor * 0.65;
            }
        }
        if let Some(mut sink) = sink_opt {
            sink.set_volume(Volume::Linear(target_vol));
        }
    }

    // ۵. لوپ‌های محیطی بر اساس موقعیت قاب دوربین (برکه، آبشار و هر منطقه‌ای در config.ron)
    if ctx.in_game && !timers.ambient_spawned {
        for (idx, zone) in a.ambient_zones.iter().enumerate() {
            if let Some(handle) = cache.get_or_load(&zone.sound_path, &asset_server) {
                commands.spawn((
                    AmbientZoneLoop {
                        zone_index: idx,
                        base_volume: zone.base_volume,
                    },
                    AudioPlayer::new(handle),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
                ));
            }
        }
        timers.ambient_spawned = true;
    }

    for (az, sink_opt) in &mut ambient_q {
        let Some(zone) = a.ambient_zones.get(az.zone_index) else {
            continue;
        };
        let z_pos = Vec3::new(zone.center.0, zone.center.1, zone.center.2);
        let dist = Vec2::new(ctx.listener_pos.x, ctx.listener_pos.z)
            .distance(Vec2::new(z_pos.x, z_pos.z));
        let proximity = (1.0 - (dist / zone.radius.max(1.0))).clamp(0.0, 1.0);
        let vol = if ctx.in_game {
            settings.effective_gain(AudioCategory::Ambience, duck) * az.base_volume * proximity
        } else {
            0.0
        };
        if let Some(mut sink) = sink_opt {
            sink.set_volume(Volume::Linear(vol));
        }
    }
}

fn sync_audio_sink_volumes_system(
    settings: Res<AudioSettingsState>,
    timers: Res<AudioRuntimeTimers>,
    mut normal_sinks: Query<(&TaggedAudio, &mut AudioSink), Without<SpatialAudioSink>>,
    mut spatial_sinks: Query<(&TaggedAudio, &mut SpatialAudioSink), Without<AudioSink>>,
) {
    let duck = if timers.low_hp_duck_factor > 0.0 {
        timers.low_hp_duck_factor
    } else {
        1.0
    };
    for (tag, mut sink) in &mut normal_sinks {
        let eff = settings.effective_gain(tag.category, duck) * tag.base_gain;
        sink.set_volume(Volume::Linear(eff));
    }
    for (tag, mut sink) in &mut spatial_sinks {
        let eff = settings.effective_gain(tag.category, duck) * tag.base_gain;
        sink.set_volume(Volume::Linear(eff));
    }
}