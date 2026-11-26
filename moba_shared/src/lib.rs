use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use renet::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType};
use renet_netcode::{
    ClientAuthentication, ConnectToken, NetcodeClientTransport, NetcodeServerTransport,
    ServerAuthentication, ServerConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod demo_shared;
pub mod shop_shared;
pub use demo_shared::*;
pub use shop_shared::*;

// -----------------------------------------------------------------------------
// ۱. ساختارهای کانفیگ (بدون هیچ Default یا serde(default))
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PauseConfig {
    pub max_pauses_per_team: u32,
    pub team_time_bank_secs: f32,
    pub player_cooldown_secs: f32,
    pub manual_unpause_lock_secs: f32,
    pub disconnect_unpause_lock_secs: f32,
    pub unpause_countdown_secs: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PingConfig {
    pub duration_secs: f32,
    pub max_active_pings: usize,
    pub world_ring_radius: f32,
    pub beam_height: f32,
    pub minimap_pulse_speed: f32,
    pub min_interval_secs: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatConfig {
    pub max_message_len: usize,
    pub max_visible_messages: usize,
    pub message_fade_secs: f32,
    pub min_interval_secs: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScoreboardConfig {
    pub panel_left_px: f32,
    pub panel_top_px: f32,
    pub panel_width_px: f32,
    pub row_height_px: f32,
    pub portrait_width_px: f32,
    pub portrait_height_px: f32,
    pub level_ring_size_px: f32,
    pub xp_ring_segments: usize,
    pub item_icon_size_px: f32,
    pub max_item_slots: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroWeaponAudioDef {
    pub primary_fire_sound: String,
    pub primary_fire_volume: f32,
    pub primary_impact_sound: String,
    pub primary_impact_volume: f32,
    pub pistol_fire_sound: String,
    pub pistol_fire_volume: f32,
    pub pistol_impact_sound: String,
    pub pistol_impact_volume: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AmbientZoneAudioDef {
    pub id: String,
    pub center: (f32, f32, f32),
    pub radius: f32,
    pub sound_path: String,
    pub base_volume: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioConfig {
    pub master_volume: f32,
    pub music_volume: f32,
    pub ambience_volume: f32,
    pub weapons_volume: f32,
    pub movement_volume: f32,
    pub units_volume: f32,
    pub ui_volume: f32,
    pub alerts_volume: f32,
    pub spatial_scale: f32,
    pub max_hearing_distance: f32,
    pub hero_weapons: HashMap<String, HeroWeaponAudioDef>,
    pub run_to_dive_sound: String,
    pub run_to_dive_volume: f32,
    pub running_slide_sound: String,
    pub running_slide_volume: f32,
    pub swing_start_sound: String,
    pub swing_start_volume: f32,
    pub swing_air_loop_sound: String,
    pub swing_air_loop_volume: f32,
    pub swing_end_sound: String,
    pub swing_end_volume: f32,
    pub swing_land_sound: String,
    pub swing_land_volume: f32,
    pub grenade_countdown_beep_sound: String,
    pub grenade_countdown_beep_volume: f32,
    pub grenade_explosion_sound: String,
    pub grenade_explosion_volume: f32,
    pub grenade_water_explosion_sound: String,
    pub grenade_water_explosion_volume: f32,
    pub footstep_sounds: Vec<String>,
    pub footstep_volume: f32,
    pub rain_footstep_sounds: Vec<String>,
    pub rain_footstep_volume: f32,
    pub snow_footstep_sounds: Vec<String>,
    pub snow_footstep_volume: f32,
    pub crouch_footstep_volume_multiplier: f32,
    pub footstep_interval_secs: f32,
    pub weapon_switch_sound: String,
    pub weapon_switch_volume: f32,
    pub water_submerge_sound: String,
    pub water_submerge_volume: f32,
    pub swim_loop_sound: String,
    pub swim_loop_volume: f32,
    pub shallow_water_step_sound: String,
    pub shallow_water_step_volume: f32,
    pub shallow_water_step_interval_secs: f32,
    pub quick_roll_to_run_sound: String,
    pub quick_roll_to_run_volume: f32,
    pub grenade_throw_sound: String,
    pub grenade_throw_volume: f32,
    pub running_dive_roll_sound: String,
    pub running_dive_roll_volume: f32,
    pub hero_death_sound: String,
    pub hero_death_volume: f32,
    pub jump_over_sound: String,
    pub jump_over_volume: f32,
    pub obstacle_jump_sound: String,
    pub obstacle_jump_volume: f32,
    pub primary_reload_sound: String,
    pub primary_reload_volume: f32,
    pub pistol_reload_sound: String,
    pub pistol_reload_volume: f32,
    pub impact_stone_wall_sounds: Vec<String>,
    pub impact_stone_wall_volume: f32,
    pub impact_tree_sounds: Vec<String>,
    pub impact_tree_volume: f32,
    pub impact_turret_sounds: Vec<String>,
    pub impact_turret_volume: f32,
    pub impact_character_sounds: Vec<String>,
    pub impact_character_volume: f32,
    pub impact_building_sounds: Vec<String>,
    pub impact_building_volume: f32,
    pub impact_water_sounds: Vec<String>,
    pub impact_water_volume: f32,
    pub creep_fire_sound: String,
    pub creep_fire_volume: f32,
    pub creep_fire_max_concurrent: usize,
    pub creep_fire_crowd_ducking: f32,
    pub creep_run_loop_sound: String,
    pub creep_run_loop_volume: f32,
    pub gold_gain_sound: String,
    pub gold_gain_volume: f32,
    pub deny_sound: String,
    pub deny_volume: f32,
    pub turret_destroy_sound: String,
    pub turret_destroy_volume: f32,
    pub turret_fire_sound: String,
    pub turret_fire_volume: f32,
    pub building_destroy_sound: String,
    pub building_destroy_volume: f32,
    pub victory_sound: String,
    pub victory_volume: f32,
    pub defeat_sound: String,
    pub defeat_volume: f32,
    pub low_hp_threshold_ratio: f32,
    pub heartbeat_sound: String,
    pub heartbeat_volume: f32,
    pub heartbeat_min_interval_secs: f32,
    pub heartbeat_max_interval_secs: f32,
    pub low_hp_world_duck_min_factor: f32,
    pub low_hp_vignette_max_alpha: f32,
    pub hero_respawn_sound: String,
    pub hero_respawn_volume: f32,
    pub courier_death_sound: String,
    pub courier_death_volume: f32,
    pub courier_respawn_sound: String,
    pub courier_respawn_volume: f32,
    pub courier_fly_loop_sound: String,
    pub courier_fly_loop_volume: f32,
    pub match_start_sound: String,
    pub match_start_volume: f32,
    pub hero_select_music_sound: String,
    pub hero_select_music_volume: f32,
    pub hero_select_pick_sound: String,
    pub hero_select_pick_volume: f32,
    pub team_ping_sound: String,
    pub team_ping_volume: f32,
    pub game_pause_sound: String,
    pub game_pause_volume: f32,
    pub unpause_countdown_sound: String,
    pub unpause_countdown_volume: f32,
    pub background_music_sound: String,
    pub background_music_volume: f32,
    pub shop_add_cart_sound: String,
    pub shop_add_cart_volume: f32,
    pub shop_buy_sound: String,
    pub shop_buy_volume: f32,
    pub shop_sell_sound: String,
    pub shop_sell_volume: f32,
    pub shop_combine_sound: String,
    pub shop_combine_volume: f32,
    pub not_enough_gold_sound: String,
    pub not_enough_gold_volume: f32,
    pub shop_open_sound: String,
    pub shop_open_volume: f32,
    pub shop_close_sound: String,
    pub shop_close_volume: f32,
    pub shop_enter_sound: String,
    pub shop_enter_volume: f32,
    pub shop_exit_sound: String,
    pub shop_exit_volume: f32,
    pub inventory_full_sound: String,
    pub inventory_full_volume: f32,
    pub empty_gun_click_sound: String,
    pub empty_gun_click_volume: f32,
    pub empty_grenade_click_sound: String,
    pub empty_grenade_click_volume: f32,
    pub level_up_sound: String,
    pub level_up_volume: f32,
    pub ambient_zones: Vec<AmbientZoneAudioDef>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeatherKind {
    Clear,
    Rainy,
    Snowy,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SnowAccumulationConfig {
    pub enabled: bool,
    pub shader_path: String,
    pub accumulate_duration_secs: f32,
    pub melt_duration_secs: f32,
    pub slope_threshold: f32,
    pub slope_softness: f32,
    pub noise_scale: f32,
    pub noise_strength: f32,
    pub ground_coverage_boost: f32,
    pub snow_color_rgb: (f32, f32, f32),
    pub snow_shadow_tint_rgb: (f32, f32, f32),
    pub snow_roughness: f32,
    pub sparkle_strength: f32,
    pub sparkle_scale: f32,
    pub night_snow_glow_rgb: (f32, f32, f32),
    pub night_snow_glow_strength: f32,
    pub night_directional_illuminance_mult: f32,
    pub night_ambient_brightness_mult: f32,
    pub night_ambient_color_rgb: (f32, f32, f32),
    pub night_fog_color_rgb: (f32, f32, f32),
    pub night_fog_start_mult: f32,
    pub night_fog_end_mult: f32,
    pub water_exclusion_margin: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeatherConfig {
    pub initial_weather: WeatherKind,
    pub weather_cycle_start_secs: f32,
    pub clear_min_duration_secs: f32,
    pub clear_max_duration_secs: f32,
    pub rainy_min_duration_secs: f32,
    pub rainy_max_duration_secs: f32,
    pub snowy_min_duration_secs: f32,
    pub snowy_max_duration_secs: f32,
    pub clear_weight: f32,
    pub rainy_weight: f32,
    pub snowy_weight: f32,
    pub transition_duration_secs: f32,
    pub rain_directional_illuminance_mult: f32,
    pub rain_directional_color_rgb: (f32, f32, f32),
    pub rain_ambient_color_rgb: (f32, f32, f32),
    pub rain_ambient_brightness_mult: f32,
    pub rain_fog_color_rgb: (f32, f32, f32),
    pub rain_fog_start_mult: f32,
    pub rain_fog_end_mult: f32,
    pub snow_directional_illuminance_mult: f32,
    pub snow_directional_color_rgb: (f32, f32, f32),
    pub snow_ambient_color_rgb: (f32, f32, f32),
    pub snow_ambient_brightness_mult: f32,
    pub snow_fog_color_rgb: (f32, f32, f32),
    pub snow_fog_start_mult: f32,
    pub snow_fog_end_mult: f32,
    pub rain_fall_speed: f32,
    pub rain_wind_tilt: f32,
    pub rain_density: f32,
    pub rain_streak_width: f32,
    pub rain_streak_length: f32,
    pub rain_drop_color_rgba: (f32, f32, f32, f32),
    pub snow_fall_speed: f32,
    pub snow_sway_amp: f32,
    pub snow_density: f32,
    pub snow_flake_size: f32,
    pub snow_flake_softness: f32,
    pub snow_flake_color_rgba: (f32, f32, f32, f32),
    pub lightning_min_interval_secs: f32,
    pub lightning_max_interval_secs: f32,
    pub lightning_flash_duration_secs: f32,
    pub lightning_flash_directional_illuminance: f32,
    pub lightning_flash_ambient_brightness: f32,
    pub lightning_flash_color_rgb: (f32, f32, f32),
    pub thunder_sound_delay_secs: f32,
    pub thunder_sound: String,
    pub thunder_volume: f32,
    pub rain_loop_sound: String,
    pub rain_loop_volume: f32,
    pub snow_loop_sound: String,
    pub snow_loop_volume: f32,
    pub audio_fade_speed: f32,
    pub snow_accumulation: SnowAccumulationConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DayNightConfig {
    pub phase_duration_secs: f32,
    pub transition_duration_secs: f32,
    pub night_creep_vision_multiplier: f32,
    pub night_courier_vision_multiplier: f32,
    pub night_turret_vision_multiplier: f32,
    pub night_directional_illuminance: f32,
    pub night_directional_color_rgb: (f32, f32, f32),
    pub night_ambient_color_rgb: (f32, f32, f32),
    pub night_ambient_brightness: f32,
    pub day_fog_color_rgb: (f32, f32, f32),
    pub day_fog_start: f32,
    pub day_fog_end: f32,
    pub night_fog_color_rgb: (f32, f32, f32),
    pub night_fog_start: f32,
    pub night_fog_end: f32,
    pub day_start_sound: String,
    pub day_start_volume: f32,
    pub night_start_sound: String,
    pub night_start_volume: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TelemetryConfig {
    pub show_detailed_render_stats: bool,
    pub refresh_interval_secs: f32,
    pub low_fps_warning_threshold: u32,
    pub low_fps_critical_threshold: u32,
    pub high_ping_warning_ms: u32,
    pub high_ping_critical_ms: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BuffHudConfig {
    pub bottom_px: f32,
    pub circle_size_px: f32,
    pub ring_thickness_px: f32,
    pub circle_gap_px: f32,
    pub ring_segments: u32,
    pub buff_ring_color_rgb: (f32, f32, f32),
    pub debuff_ring_color_rgb: (f32, f32, f32),
    pub mixed_ring_color_rgb: (f32, f32, f32),
    pub day_color_rgb: (f32, f32, f32),
    pub night_color_rgb: (f32, f32, f32),
    pub clear_color_rgb: (f32, f32, f32),
    pub rainy_color_rgb: (f32, f32, f32),
    pub snowy_color_rgb: (f32, f32, f32),
    pub popup_width_px: f32,
    pub popup_bottom_offset_px: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuSoldierSlotTransform {
    pub position: [f32; 3],
    pub yaw_deg: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainMenuConfig {
    pub background_image_path: String,
    pub soldier_model_path: String,
    pub soldier_idle_animation: String,
    pub party_max_size: usize,
    pub notification_duration_secs: f32,
    pub party_invite_timeout_secs: f32,
    pub camera_position: [f32; 3],
    pub camera_look_at: [f32; 3],
    pub camera_fov_deg: f32,
    pub key_light_illuminance: f32,
    pub key_light_direction: [f32; 3],
    pub rim_light_intensity: f32,
    pub rim_light_position: [f32; 3],
    pub ambient_light_brightness: f32,
    pub squad_slots: Vec<MenuSoldierSlotTransform>,
    pub game_modes: Vec<String>,
    pub default_map_name: String,
    pub lobby_capacities: Vec<usize>,
    pub abandon_mmr_penalty: i32,
    pub mmr_tier_bar_step: i32,
    pub toast_duration_secs: f32,
    pub social_reconnect_interval_secs: f32,
    pub default_queue_required_players: usize,
    pub http_timeout_secs: u64,
    pub ws_ping_interval_secs: u64,
    pub ws_poll_sleep_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameplayRulesConfig {
    pub allow_shadow_blade_during_teleport: bool,
    pub teleport_breaks_invisibility: bool,
    pub allow_teleport_in_water: bool,
    pub cancel_teleport_on_building_destroyed: bool,
    pub courier_invulnerable_in_fountain: bool,
    pub turret_prioritize_hero_on_ally_attack: bool,
    pub fountain_attacks_enemies: bool,
    pub fountain_reveals_invisible_enemies: bool,
    pub neutral_camps_regen_on_leash: bool,
    pub last_hitter_always_gets_xp: bool,
    pub allow_beam_deny: bool,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct GameConfig {
    pub gameplay_rules: GameplayRulesConfig,
    pub main_menu: MainMenuConfig,
    pub weather: WeatherConfig,
    pub day_night: DayNightConfig,
    pub telemetry: TelemetryConfig,
    pub buff_hud: BuffHudConfig,
    pub pause: PauseConfig,
    pub ping: PingConfig,
    pub chat: ChatConfig,
    pub scoreboard: ScoreboardConfig,
    pub audio: AudioConfig,
    pub network: NetworkConfig,
    pub window: WindowConfig,
    pub camera: CameraConfig,
    pub lighting: LightingConfig,
    pub weapon_tuner: WeaponTunerConfig,
    pub physics: PhysicsConfig,
    pub heroes: Vec<HeroDef>,
    pub soldier: SoldierConfig,
    pub combat: CombatConfig,
    pub fx: FxConfig,
    pub cursor_reticle: CursorReticleConfig,
    pub laser: LaserConfig,
    pub creeps: CreepConfig,
    pub neutral_camps: NeutralCampsConfig,
    pub animations: AnimationConfig,
    pub map: MapConfig,
    pub obstacles: Vec<ObstacleDef>,
    pub targets: Vec<TargetDef>,
}

impl GameConfig {
    pub fn load_from_file(path: &str) -> Self {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("خطای بحرانی: فایل کانفیگ '{}' خوانده نشد: {}", path, e));
        Self::load_from_str(&content, path)
    }

    pub fn load_from_str(content: &str, label: &str) -> Self {
        let mut cfg = ron::from_str::<GameConfig>(content).unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی: ساختار فایل کانفیگ '{}' نامعتبر است: {}",
                label, e
            )
        });
        cfg.validate_unique_map_ids();
        cfg.validate_biome();
        cfg.validate_roshan_pit();
        cfg.validate_fountains();
        cfg.validate_barracks();
        cfg.validate_ancients();
        cfg.validate_chat();
        cfg.validate_scoreboard();
        cfg.validate_audio();
        cfg.validate_weather();
        cfg.validate_day_night();
        cfg.validate_telemetry();
        cfg.validate_buff_hud();
        cfg.validate_ping();
        cfg.validate_pause();
        cfg.validate_camera();
        cfg.validate_heroes();
        cfg.validate_fx();
        cfg.validate_creeps();
        cfg.validate_neutral_camps();
        cfg.validate_obstacles();
        cfg.validate_canyon_mountain_align();
        align_canyon_mountains_to_rim(
            &cfg.map.canyon,
            &cfg.physics.canyon_mountain_align,
            &mut cfg.map.mountains,
        );
        cfg
    }

    pub fn validate_canyon_mountain_align(&self) {
        let align = &self.physics.canyon_mountain_align;
        assert!(
            !align.id_prefix.trim().is_empty(),
            "خطای بحرانی کانفیگ: physics.canyon_mountain_align.id_prefix خالی است"
        );
        assert!(
            align.target_edge_offset.is_finite(),
            "خطای بحرانی کانفیگ: physics.canyon_mountain_align.target_edge_offset نامعتبر است"
        );
        assert!(
            align.gradient_sample_eps > 0.0 && align.gradient_sample_eps <= 5.0,
            "خطای بحرانی کانفیگ: physics.canyon_mountain_align.gradient_sample_eps باید بین ۰ و ۵ باشد"
        );
        assert!(
            align.max_snap_iterations >= 1 && align.max_snap_iterations <= 64,
            "خطای بحرانی کانفیگ: physics.canyon_mountain_align.max_snap_iterations باید بین ۱ تا ۶۴ باشد"
        );
        assert!(
            align.yaw_offset_deg.is_finite(),
            "خطای بحرانی کانفیگ: physics.canyon_mountain_align.yaw_offset_deg نامعتبر است"
        );
    }

    pub fn exact_anim_duration(&self, anim_name: &str) -> f32 {
        *self
            .animations
            .clip_durations
            .get(anim_name)
            .unwrap_or_else(|| {
                panic!(
                    "خطای بحرانی کانفیگ: مدت‌زمان انیمیشن '{}' در config.ron تعریف نشده است!",
                    anim_name
                )
            })
    }

    pub fn validate_weapon_vfx_profile(p: &WeaponVfxProfile, label: &str) {
        assert!(!p.shader_path.trim().is_empty(), "shader_path در '{}' خالی است", label);
        assert!(p.bloom_intensity > 0.0, "bloom_intensity در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.projectile_radius > 0.0, "projectile_radius در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.projectile_length > 0.0, "projectile_length در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.trail_width > 0.0, "trail_width در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.trail_lifetime_secs > 0.0, "trail_lifetime_secs در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.trail_wave_freq >= 0.0, "trail_wave_freq در '{}' نمی‌تواند منفی باشد", label);
        assert!(p.trail_wave_amp >= 0.0, "trail_wave_amp در '{}' نمی‌تواند منفی باشد", label);
        assert!(p.muzzle_flash_scale > 0.0, "muzzle_flash_scale در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.muzzle_flash_lifetime_secs > 0.0, "muzzle_flash_lifetime_secs در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.muzzle_light_intensity >= 0.0, "muzzle_light_intensity در '{}' نمی‌تواند منفی باشد", label);
        assert!(p.muzzle_light_range > 0.0, "muzzle_light_range در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.muzzle_ring_start_scale > 0.0, "muzzle_ring_start_scale در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.muzzle_ring_end_scale >= p.muzzle_ring_start_scale, "muzzle_ring_end_scale در '{}' نامعتبر است", label);
        assert!(p.impact_ring_end_scale > 0.0, "impact_ring_end_scale در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.impact_ring_lifetime_secs > 0.0, "impact_ring_lifetime_secs در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.impact_light_intensity >= 0.0, "impact_light_intensity در '{}' نمی‌تواند منفی باشد", label);
        assert!(p.impact_light_range > 0.0, "impact_light_range در '{}' باید بزرگ‌تر از صفر باشد", label);
        assert!(p.pulse_speed > 0.0, "pulse_speed در '{}' باید بزرگ‌تر از صفر باشد", label);
    }

    pub fn validate_fx(&self) {
        assert!(!self.network.backend_http_url.trim().is_empty(), "network.backend_http_url cannot be empty");
        assert!(!self.network.backend_ws_url.trim().is_empty(), "network.backend_ws_url cannot be empty");
        assert!(!self.network.redis_addr.trim().is_empty(), "network.redis_addr cannot be empty");
        assert!(self.network.battle_loading_max_wait_secs.is_finite() && self.network.battle_loading_max_wait_secs > 2.5, "network.battle_loading_max_wait_secs باید بزرگ‌تر از 2.5 ثانیه باشد");
        assert!(self.network.post_match_shutdown_delay_secs.is_finite() && self.network.post_match_shutdown_delay_secs > 0.0, "network.post_match_shutdown_delay_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.network.empty_room_shutdown_secs.is_finite() && self.network.empty_room_shutdown_secs > 0.0, "network.empty_room_shutdown_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.soldier.team_spawn_slot_spacing > 0.0, "soldier.team_spawn_slot_spacing باید بزرگ‌تر از صفر باشد");
        assert!(self.soldier.cliff_dive_speed > 0.0, "soldier.cliff_dive_speed must be > 0");
        assert!(self.soldier.cliff_dive_arc_boost > 0.0, "soldier.cliff_dive_arc_boost must be > 0");
        assert!(self.soldier.fall_dive_arc_boost > 0.0, "soldier.fall_dive_arc_boost must be > 0");
        assert!(self.soldier.cliff_edge_probe_step > 0.0, "soldier.cliff_edge_probe_step must be > 0");
        assert!(self.soldier.cliff_edge_drop_threshold > 0.0, "soldier.cliff_edge_drop_threshold must be > 0");
        assert!(self.soldier.cliff_edge_stop_margin >= 0.0, "soldier.cliff_edge_stop_margin must be >= 0");
        assert!(self.soldier.cliff_chain_min_slide_dist >= 0.0, "soldier.cliff_chain_min_slide_dist must be >= 0");
        assert!(self.soldier.cliff_edge_min_anim_duration_secs > 0.0, "soldier.cliff_edge_min_anim_duration_secs must be > 0");
        assert!(!self.soldier.vertical_aim_key.trim().is_empty(), "soldier.vertical_aim_key cannot be empty");
        assert!(self.physics.deep_water_swim_depth.is_finite() && self.physics.deep_water_swim_depth > 0.0, "physics.deep_water_swim_depth باید بزرگ‌تر از صفر باشد");
        Self::validate_weapon_vfx_profile(&self.fx.creep_bullet_vfx, "fx.creep_bullet_vfx");
        Self::validate_weapon_vfx_profile(&self.fx.turret_bullet_vfx, "fx.turret_bullet_vfx");
        let bs = &self.fx.blood_splatter_vfx;
        assert!(bs.decal_count_min > 0 && bs.decal_count_max >= bs.decal_count_min, "blood_splatter_vfx.decal_count نامعتبر است");
        assert!(bs.decal_min_scale > 0.0 && bs.decal_max_scale >= bs.decal_min_scale, "blood_splatter_vfx.decal_scale نامعتبر است");
        assert!(bs.decal_scatter_radius > 0.0, "blood_splatter_vfx.decal_scatter_radius باید بزرگ‌تر از صفر باشد");
        assert!(bs.decal_lifetime_secs > 0.0, "blood_splatter_vfx.decal_lifetime_secs باید بزرگ‌تر از صفر باشد");
        let ge = &self.fx.grenade_explosion_vfx;
        assert!(ge.fireball_bloom_intensity > 0.0, "grenade_explosion_vfx.fireball_bloom_intensity باید بزرگ‌تر از صفر باشد");
        assert!(ge.fireball_duration_secs > 0.0, "grenade_explosion_vfx.fireball_duration_secs باید بزرگ‌تر از صفر باشد");
        assert!(ge.water_bloom_intensity > 0.0, "grenade_explosion_vfx.water_bloom_intensity باید بزرگ‌تر از صفر باشد");
        assert!(ge.water_geyser_height > 0.0, "grenade_explosion_vfx.water_geyser_height باید بزرگ‌تر از صفر باشد");
        assert!(ge.water_geyser_duration_secs > 0.0, "grenade_explosion_vfx.water_geyser_duration_secs باید بزرگ‌تر از صفر باشد");
        assert!(ge.shockwave_duration_secs > 0.0, "grenade_explosion_vfx.shockwave_duration_secs باید بزرگ‌تر از صفر باشد");
    }

    pub fn validate_obstacles(&self) {
        let s = &self.soldier;
        assert!(
            s.vault_check_distance.is_finite() && s.vault_check_distance > 0.0,
            "soldier.vault_check_distance باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            s.vault_travel_distance.is_finite() && s.vault_travel_distance > 0.0,
            "soldier.vault_travel_distance باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            s.vault_ray_height.is_finite() && s.vault_ray_height >= 0.0,
            "soldier.vault_ray_height نمی‌تواند منفی باشد"
        );
        assert!(
            s.vault_anim_reference_height.is_finite() && s.vault_anim_reference_height >= 0.0,
            "soldier.vault_anim_reference_height نمی‌تواند منفی باشد"
        );
        assert!(
            s.vault_top_clearance.is_finite() && s.vault_top_clearance >= 0.0,
            "soldier.vault_top_clearance نمی‌تواند منفی باشد"
        );
        assert!(
            s.vault_min_lift_height.is_finite() && s.vault_min_lift_height >= 0.0,
            "soldier.vault_min_lift_height نمی‌تواند منفی باشد"
        );
        assert!(
            s.vault_max_lift_height.is_finite()
                && s.vault_max_lift_height >= s.vault_min_lift_height,
            "soldier.vault_max_lift_height باید بزرگ‌تر یا مساوی vault_min_lift_height باشد"
        );
        assert!(
            s.vault_apex_fraction.is_finite()
                && s.vault_apex_fraction > 0.05
                && s.vault_apex_fraction < 0.95,
            "soldier.vault_apex_fraction باید در بازه (0.05, 0.95) باشد"
        );
        assert!(
            s.vault_arc_exponent.is_finite()
                && s.vault_arc_exponent > 0.0
                && s.vault_arc_exponent <= 4.0,
            "soldier.vault_arc_exponent باید در بازه (0.0, 4.0] باشد"
        );
        assert!(
            s.vault_approach_max_angle_deg.is_finite()
                && s.vault_approach_max_angle_deg > 0.0
                && s.vault_approach_max_angle_deg < 90.0,
            "soldier.vault_approach_max_angle_deg باید در بازه (0.0, 90.0) باشد"
        );

        for obs in &self.obstacles {
            assert!(
                obs.vault_height_offset.is_finite(),
                "vault_height_offset در آبجکت '{}' نامعتبر است",
                obs.id
            );
            let mut seen_sides = HashSet::new();
            for &side in &obs.vault_sides {
                assert!(
                    seen_sides.insert(side),
                    "ضلع تکراری {:?} در vault_sides آبجکت '{}' تعریف شده است",
                    side,
                    obs.id
                );
            }
            if obs.can_vault {
                assert!(
                    !obs.vault_sides.is_empty(),
                    "خطای بحرانی کانفیگ: آبجکت '{}' دارای can_vault: true است اما vault_sides آن خالی است!",
                    obs.id
                );
            }
        }
    }


    pub fn resolve_bullet_vfx(&self, source: &BulletVfxSource) -> &WeaponVfxProfile {
        match source {
            BulletVfxSource::Hero { hero_id, weapon_kind } => {
                let hero = self.heroes.iter().find(|h| &h.id == hero_id).unwrap_or_else(|| {
                    panic!("خطای بحرانی کانفیگ: هیرو '{}' برای VFX تیر یافت نشد!", hero_id)
                });
                if *weapon_kind == WeaponKind::Pistol {
                    &hero.bullet_vfx.pistol
                } else {
                    &hero.bullet_vfx.primary
                }
            }
            BulletVfxSource::Creep => &self.fx.creep_bullet_vfx,
            BulletVfxSource::Turret => &self.fx.turret_bullet_vfx,
        }
    }

    pub fn validate_heroes(&self) {
        assert!(
            self.combat.xp_thresholds_per_level.len() == 25,
            "xp_thresholds_per_level در config.ron باید دقیقاً ۲۵ سطح (۱ تا ۲۵) داشته باشد!"
        );
        assert!(
            self.combat.respawn_times_per_level.len() == 25,
            "respawn_times_per_level در config.ron باید دقیقاً ۲۵ سطح (۱ تا ۲۵) داشته باشد!"
        );
        assert!(
            self.combat.min_fire_rate_secs > 0.0,
            "min_fire_rate_secs باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            self.combat.min_reload_duration_secs > 0.0,
            "min_reload_duration_secs باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            self.combat.passive_gold_interval_secs.is_finite()
                && self.combat.passive_gold_interval_secs > 0.0,
            "passive_gold_interval_secs باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            self.combat.passive_gold_amount > 0,
            "passive_gold_amount باید بزرگ‌تر از صفر باشد"
        );
        assert!(self.combat.hero_kill_gold > 0, "combat.hero_kill_gold باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.hero_kill_xp > 0, "combat.hero_kill_xp باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.hero_assist_gold > 0, "combat.hero_assist_gold باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.hero_assist_xp > 0, "combat.hero_assist_xp باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.hero_assist_window_secs.is_finite() && self.combat.hero_assist_window_secs > 0.0, "combat.hero_assist_window_secs نامعتبر است");
        assert!(self.combat.hero_kill_credit_window_secs.is_finite() && self.combat.hero_kill_credit_window_secs > 0.0, "combat.hero_kill_credit_window_secs نامعتبر است");
        assert!(self.combat.turret_hero_aggro_window_secs.is_finite() && self.combat.turret_hero_aggro_window_secs > 0.0, "combat.turret_hero_aggro_window_secs نامعتبر است");
        assert!(self.combat.weapon_swap_min_cooldown_secs.is_finite() && self.combat.weapon_swap_min_cooldown_secs >= 0.0, "combat.weapon_swap_min_cooldown_secs نامعتبر است");
        assert!(self.combat.attack_reveal_duration_secs.is_finite() && self.combat.attack_reveal_duration_secs > 0.0, "combat.attack_reveal_duration_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.attack_reveal_vision_radius.is_finite() && self.combat.attack_reveal_vision_radius > 0.0, "combat.attack_reveal_vision_radius باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.creep_hero_aggro_radius.is_finite() && self.combat.creep_hero_aggro_radius > 0.0, "combat.creep_hero_aggro_radius باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.creep_hero_aggro_duration_secs.is_finite() && self.combat.creep_hero_aggro_duration_secs > 0.0, "combat.creep_hero_aggro_duration_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.combat.creep_hero_aggro_cooldown_secs.is_finite() && self.combat.creep_hero_aggro_cooldown_secs >= 0.0, "combat.creep_hero_aggro_cooldown_secs نامعتبر است");
        assert!(self.combat.creep_hero_aggro_max_chase_dist.is_finite() && self.combat.creep_hero_aggro_max_chase_dist >= self.combat.creep_hero_aggro_radius, "combat.creep_hero_aggro_max_chase_dist باید بزرگ‌تر یا مساوی creep_hero_aggro_radius باشد");
        assert!(self.soldier.jump_over_speed > 0.0, "soldier.jump_over_speed باید بزرگ‌تر از صفر باشد");
        assert!(self.soldier.jump_over_clearance_height > 0.0, "soldier.jump_over_clearance_height باید بزرگ‌تر از صفر باشد");
        assert!(self.soldier.space_hold_threshold_secs > 0.0, "soldier.space_hold_threshold_secs باید بزرگ‌تر از صفر باشد");
        assert!(
            !self.heroes.is_empty(),
            "خطای بحرانی کانفیگ: لیست heroes خالی است! حداقل یک هیرو باید در config.ron تعریف شود."
        );
        for hero in &self.heroes {
            assert!(!hero.id.trim().is_empty(), "شناسه هیرو نمی‌تواند خالی باشد");
            assert!(!hero.model_path.trim().is_empty(), "model_path هیرو '{}' نمی‌تواند خالی باشد", hero.id);
            assert!(!hero.portrait_path.trim().is_empty(), "portrait_path هیرو '{}' نمی‌تواند خالی باشد", hero.id);
            assert!(hero.max_hp > 0.0, "max_hp هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.pistol.magazine_size > 0, "pistol.magazine_size هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.pistol.fire_rate_secs > 0.0, "pistol.fire_rate_secs هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.pistol.reload_duration_secs > 0.0, "pistol.reload_duration_secs هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(!hero.courier.model_path.trim().is_empty(), "courier.model_path هیرو '{}' نمی‌تواند خالی باشد", hero.id);
            assert!(hero.courier.scale > 0.0, "courier.scale هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.fly_height > 0.0, "courier.fly_height هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.move_speed > 0.0, "courier.move_speed هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.inventory_slots > 0, "courier.inventory_slots هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.max_hp > 0.0, "courier.max_hp هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.respawn_secs > 0.0, "courier.respawn_secs هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.deliver_radius > 0.0, "courier.deliver_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.fountain_radius > 0.0, "courier.fountain_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.move_to_arrival_radius > 0.0, "courier.move_to_arrival_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.move_stop_epsilon > 0.0, "courier.move_stop_epsilon هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.courier.hit_radius > 0.0, "courier.hit_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.vision_range > 0.0, "vision_range هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.night_vision_range > 0.0, "night_vision_range هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(!hero.weapon_models.run_upper_anim.trim().is_empty(), "weapon_models.run_upper_anim هیرو '{}' نباید خالی باشد (مقدار 'same' یا نام انیمیشن بدهید)", hero.id);
            assert!(hero.courier.vision_range > 0.0, "courier.vision_range هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.pistol.damage > 0.0, "pistol.damage هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.pistol.projectile_speed > 0.0, "pistol.projectile_speed هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.pistol.max_range > 0.0, "pistol.max_range هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.max_stamina > 0.0, "max_stamina هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.stamina_regen_per_sec > 0.0, "stamina_regen_per_sec هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.slide_stamina_cost > 0.0, "slide_stamina_cost هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.jump_over_stamina_cost > 0.0, "jump_over_stamina_cost هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.run_stamina_drain_per_sec >= 0.0 && hero.run_stamina_drain_per_sec.is_finite(), "run_stamina_drain_per_sec هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.swim_stamina_drain_per_sec >= 0.0 && hero.swim_stamina_drain_per_sec.is_finite(), "swim_stamina_drain_per_sec هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.climb_stamina_drain_per_sec >= 0.0 && hero.climb_stamina_drain_per_sec.is_finite(), "climb_stamina_drain_per_sec هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.stamina_exhausted_resume_min > 0.0 && hero.stamina_exhausted_resume_min <= hero.max_stamina && hero.stamina_exhausted_resume_min.is_finite(), "stamina_exhausted_resume_min هیرو '{}' باید بین 0 و max_stamina باشد", hero.id);
            assert!(hero.exhausted_run_speed_multiplier > 0.0 && hero.exhausted_run_speed_multiplier <= 1.0 && hero.exhausted_run_speed_multiplier.is_finite(), "exhausted_run_speed_multiplier هیرو '{}' باید بین 0 و 1 باشد", hero.id);
            assert!(hero.exhausted_swim_speed_multiplier > 0.0 && hero.exhausted_swim_speed_multiplier <= 1.0 && hero.exhausted_swim_speed_multiplier.is_finite(), "exhausted_swim_speed_multiplier هیرو '{}' باید بین 0 و 1 باشد", hero.id);
            assert!(hero.exhausted_climb_speed_multiplier > 0.0 && hero.exhausted_climb_speed_multiplier <= 1.0 && hero.exhausted_climb_speed_multiplier.is_finite(), "exhausted_climb_speed_multiplier هیرو '{}' باید بین 0 و 1 باشد", hero.id);
            assert!(hero.level_growth.stamina_regen_per_level >= 0.0, "stamina_regen_per_level هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            assert!(hero.hero_scale > 0.0, "hero_scale هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.xp_gain_radius > 0.0, "xp_gain_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.magazine_size > 0, "magazine_size اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.fire_rate_secs > 0.0, "fire_rate_secs اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.reload_duration_secs > 0.0, "reload_duration_secs اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.projectile_speed > 0.0, "projectile_speed اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.max_range > 0.0, "max_range اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.pellets_per_shot > 0, "pellets_per_shot اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.gravity >= 0.0, "weapon.gravity هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            assert!(hero.weapon.launch_up_boost >= 0.0, "weapon.launch_up_boost هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            assert!(hero.weapon.aoe_radius >= 0.0, "weapon.aoe_radius هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            if hero.weapon.weapon_kind == WeaponKind::Throwable {
                assert!(hero.weapon.gravity > 0.0, "سلاح پرتابی هیرو '{}' باید gravity > 0 داشته باشد", hero.id);
                assert!(hero.weapon.aoe_radius > 0.0, "سلاح پرتابی هیرو '{}' باید aoe_radius > 0 داشته باشد", hero.id);
            }
            assert!(hero.pistol.gravity >= 0.0 && hero.pistol.launch_up_boost >= 0.0 && hero.pistol.aoe_radius >= 0.0, "مقادیر پرتابی کلت هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.weapon.toxic_pool_duration_secs >= 0.0 && hero.weapon.toxic_pool_dps >= 0.0, "مقادیر سم سلاح هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.weapon.toxic_slow_ratio >= 0.0 && hero.weapon.toxic_slow_ratio < 0.95, "toxic_slow_ratio سلاح هیرو '{}' باید بین 0 و 0.95 باشد", hero.id);
            assert!(hero.level_growth.toxic_dps_per_level >= 0.0 && hero.level_growth.toxic_slow_per_level >= 0.0 && hero.level_growth.toxic_duration_per_level >= 0.0, "ضرایب ارتقای سم هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.weapon.beam_max_charge >= 0.0 && hero.weapon.beam_drain_per_sec >= 0.0 && hero.weapon.beam_dps >= 0.0, "مقادیر سلاح برقی هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.weapon.beam_attack_speed_slow >= 0.0 && hero.weapon.beam_attack_speed_slow < 0.90, "beam_attack_speed_slow هیرو '{}' باید بین 0 و 0.90 باشد", hero.id);
            assert!(hero.weapon.beam_slow_duration_secs >= 0.0, "beam_slow_duration_secs هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            if hero.weapon.weapon_kind == WeaponKind::ElectricBeam {
                assert!(hero.weapon.beam_max_charge > 0.0 && hero.weapon.beam_drain_per_sec > 0.0 && hero.weapon.beam_dps > 0.0, "سلاح برقی هیرو '{}' باید beam_max_charge، beam_drain_per_sec و beam_dps بزرگ‌تر از صفر داشته باشد", hero.id);
            }
            assert!(hero.level_growth.beam_dps_per_level >= 0.0 && hero.level_growth.beam_attack_speed_slow_per_level >= 0.0, "ضرایب ارتقای برق هیرو '{}' نامعتبر است", hero.id);
            assert!(hero.weapon.beam_armor_reduction >= 0.0 && hero.level_growth.beam_armor_reduction_per_level >= 0.0, "مقادیر کاهش آرمور هیرو '{}' نامعتبر است", hero.id);
            if hero.weapon.weapon_kind == WeaponKind::FlameBeam {
                assert!(hero.weapon.beam_max_charge > 0.0 && hero.weapon.beam_drain_per_sec > 0.0 && hero.weapon.beam_dps > 0.0 && hero.weapon.beam_armor_reduction > 0.0, "سلاح آتش هیرو '{}' باید مقادیر بزرگ‌تر از صفر داشته باشد", hero.id);
            }
            assert!(hero.grenade.damage > 0.0, "grenade.damage هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.grenade.aoe_radius > 0.0, "grenade.aoe_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.grenade.fuse_secs > 0.0, "grenade.fuse_secs هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.grenade.throw_duration_secs > 0.0, "grenade.throw_duration_secs هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.grenade.max_charges > 0, "grenade.max_charges هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
                        assert!(hero.grenade.recharge_secs > 0.0, "grenade.recharge_secs هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.swing.swing_speed > 0.0, "swing.swing_speed هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.swing.launch_delay_secs >= 0.0, "swing.launch_delay_secs هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            assert!(hero.swing.stamina_cost >= 0.0, "swing.stamina_cost هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            assert!(!hero.swing.start_anim.trim().is_empty(), "swing.start_anim هیرو '{}' خالی است", hero.id);
            assert!(!hero.swing.end_anim.trim().is_empty(), "swing.end_anim هیرو '{}' خالی است", hero.id);
            assert!(!hero.swing.fall_anim.trim().is_empty(), "swing.fall_anim هیرو '{}' خالی است", hero.id);
            assert!(
                hero.swing.start_phase_fraction > 0.0 && hero.swing.end_phase_fraction > hero.swing.start_phase_fraction && hero.swing.end_phase_fraction < 1.0,
                "بازه start_phase_fraction / end_phase_fraction هیرو '{}' نامعتبر است",
                hero.id
            );
            assert!(hero.swing.fall_gravity > 0.0, "swing.fall_gravity هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.swing.fall_forward_momentum >= 0.0, "swing.fall_forward_momentum هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            Self::validate_weapon_vfx_profile(&hero.bullet_vfx.primary, &format!("heroes[{}].bullet_vfx.primary", hero.id));
            Self::validate_weapon_vfx_profile(&hero.bullet_vfx.pistol, &format!("heroes[{}].bullet_vfx.pistol", hero.id));
        }
    }

    pub fn validate_neutral_camps(&self) {
        let nc = &self.neutral_camps;
        assert!(nc.respawn_interval_secs > 0.0, "respawn_interval_secs کمپ‌ها باید بزرگ‌تر از صفر باشد");
        assert!(nc.aggro_leash_radius > 0.0, "aggro_leash_radius کمپ‌ها باید بزرگ‌تر از صفر باشد");
        assert!(nc.leash_regen_pct_per_sec.is_finite() && nc.leash_regen_pct_per_sec >= 0.0, "neutral_camps.leash_regen_pct_per_sec نامعتبر است");
        assert!(!nc.creep_types.is_empty(), "creep_types کمپ‌ها نمی‌تواند خالی باشد");
        for (k, ct) in &nc.creep_types {
            assert!(!ct.model_path.trim().is_empty(), "model_path سرباز کمپ '{}' خالی است", k);
            assert!(ct.scale > 0.0, "scale سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
            assert!(ct.collider_radius > 0.0, "collider_radius سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
            assert!(ct.hitbox_height > 0.0, "hitbox_height سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
            assert!(ct.max_hp > 0.0, "max_hp سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
            assert!(ct.fire_rate_secs > 0.0, "fire_rate_secs سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
            assert!(ct.projectile_speed > 0.0, "projectile_speed سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
            assert!(ct.bullet_max_range > 0.0, "bullet_max_range سرباز کمپ '{}' باید بزرگ‌تر از صفر باشد", k);
        }
        let mut seen_camp_ids = HashSet::new();
        for camp in &nc.camps {
            assert!(!camp.id.trim().is_empty(), "شناسه کمپ نمی‌تواند خالی باشد");
            assert!(seen_camp_ids.insert(camp.id.clone()), "شناسه کمپ '{}' تکراری است", camp.id);
            assert!(camp.spawn_box_radius > 0.0, "spawn_box_radius کمپ '{}' باید بزرگ‌تر از صفر باشد", camp.id);
            assert!(!camp.creep_type_ids.is_empty(), "لیست سربازان کمپ '{}' خالی است", camp.id);
            for tid in &camp.creep_type_ids {
                assert!(
                    nc.creep_types.contains_key(tid),
                    "نوع سرباز '{}' در کمپ '{}' درون neutral_camps.creep_types تعریف نشده است!",
                    tid,
                    camp.id
                );
            }
        }
    }

    pub fn validate_creeps(&self) {
        let c = &self.creeps;
        assert!(!c.dawn_model_path.trim().is_empty(), "dawn_model_path سربازان لاین نمی‌تواند خالی باشد");
        assert!(!c.dusk_model_path.trim().is_empty(), "dusk_model_path سربازان لاین نمی‌تواند خالی باشد");
        assert!(c.scale > 0.0, "scale سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.collider_radius > 0.0, "collider_radius سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.hitbox_height > 0.0, "hitbox_height سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.wave_interval_secs > 0.0, "wave_interval_secs باید بزرگ‌تر از صفر باشد");
        assert!(c.creeps_per_wave > 0, "creeps_per_wave باید حداقل ۱ باشد");
        assert!(c.count_upgrade_interval_secs > 0.0, "count_upgrade_interval_secs باید بزرگ‌تر از صفر باشد");
        assert!(c.max_creeps_per_wave >= c.creeps_per_wave, "max_creeps_per_wave باید بزرگ‌تر یا مساوی creeps_per_wave باشد");
        assert!(c.stat_upgrade_interval_secs > 0.0, "stat_upgrade_interval_secs باید بزرگ‌تر از صفر باشد");
        assert!(c.hp_upgrade_per_step >= 0.0, "hp_upgrade_per_step نمی‌تواند منفی باشد");
        assert!(c.damage_upgrade_per_step >= 0.0, "damage_upgrade_per_step نمی‌تواند منفی باشد");
        assert!(c.armor_upgrade_per_step >= 0.0, "armor_upgrade_per_step نمی‌تواند منفی باشد");
        assert!(
            c.fire_rate_multiplier_per_step > 0.0 && c.fire_rate_multiplier_per_step <= 1.0,
            "fire_rate_multiplier_per_step باید در بازه (0.0, 1.0] باشد"
        );
        assert!(c.min_fire_rate_secs > 0.0, "min_fire_rate_secs باید بزرگ‌تر از صفر باشد");
        assert!(c.max_hp > 0.0, "max_hp سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.fire_rate_secs > 0.0, "fire_rate_secs سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.projectile_speed > 0.0, "projectile_speed سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.bullet_max_range > 0.0, "bullet_max_range سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(
            (0.0..=1.0).contains(&c.deny_hp_threshold_ratio),
            "deny_hp_threshold_ratio باید بین 0 و 1 باشد"
        );
        assert!(
            c.river_barrier_margin >= 0.0 && c.river_barrier_margin.is_finite(),
            "river_barrier_margin باید عددی نامنفی و معتبر باشد"
        );
        assert!(
            c.cliff_lookahead_dist > 0.0 && c.cliff_lookahead_dist.is_finite(),
            "cliff_lookahead_dist باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            c.lane_rejoin_max_deviation > 0.0 && c.lane_rejoin_max_deviation.is_finite(),
            "lane_rejoin_max_deviation باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            c.hero_block_detect_dist > 0.0 && c.hero_block_detect_dist.is_finite(),
            "hero_block_detect_dist باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            (0.0..=1.0).contains(&c.hero_block_front_cone_dot),
            "hero_block_front_cone_dot باید در بازه [0.0, 1.0] باشد"
        );
        assert!(
            c.obstacle_avoid_margin >= 0.0 && c.obstacle_avoid_margin.is_finite(),
            "obstacle_avoid_margin باید عددی نامنفی و معتبر باشد"
        );
        assert!(
            c.obstacle_avoid_lookahead_dist > 0.0 && c.obstacle_avoid_lookahead_dist.is_finite(),
            "obstacle_avoid_lookahead_dist باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            c.obstacle_avoid_steer_strength > 0.0 && c.obstacle_avoid_steer_strength.is_finite(),
            "obstacle_avoid_steer_strength باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            c.hero_block_hesitation_secs >= 0.0 && c.hero_block_hesitation_secs.is_finite(),
            "hero_block_hesitation_secs باید نامنفی و معتبر باشد"
        );
        assert!(
            c.hero_block_hesitation_cooldown_secs >= 0.0 && c.hero_block_hesitation_cooldown_secs.is_finite(),
            "hero_block_hesitation_cooldown_secs باید نامنفی و معتبر باشد"
        );
        assert!(
            c.hero_block_steer_strength > 0.0 && c.hero_block_steer_strength.is_finite(),
            "hero_block_steer_strength باید بزرگ‌تر از صفر باشد"
        );
    }

    pub fn validate_camera(&self) {
        assert!(self.camera.cursor_lead_max_distance_x >= 0.0, "cursor_lead_max_distance_x نمی‌تواند منفی باشد");
        assert!(self.camera.cursor_lead_max_distance_z >= 0.0, "cursor_lead_max_distance_z نمی‌تواند منفی باشد");
        assert!(
            (0.0..1.0).contains(&self.camera.cursor_lead_deadzone_ratio),
            "cursor_lead_deadzone_ratio باید بین [0.0, 1.0) باشد"
        );
        assert!(self.camera.cursor_lead_lerp_speed > 0.0, "cursor_lead_lerp_speed باید بزرگ‌تر از صفر باشد");
        assert!(self.camera.edge_pan_margin_px > 0.0, "edge_pan_margin_px باید بزرگ‌تر از صفر باشد");
        assert!(self.camera.edge_pan_speed > 0.0, "edge_pan_speed باید بزرگ‌تر از صفر باشد");
        assert!(self.camera.occlusion_fade_speed > 0.0, "occlusion_fade_speed باید بزرگ‌تر از صفر باشد");
        assert!(self.camera.occlusion_edge_emissive >= 0.0, "occlusion_edge_emissive نباید منفی باشد");
        assert!(self.camera.occlusion_edge_power > 0.0, "occlusion_edge_power باید بزرگ‌تر از صفر باشد");
        assert!(
            self.camera.occlusion_edge_threshold > 0.0 && self.camera.occlusion_edge_threshold < 1.0,
            "occlusion_edge_threshold باید بین (0.0, 1.0) باشد"
        );
        assert!(self.camera.occlusion_terrain_cutout_radius > 0.0, "occlusion_terrain_cutout_radius باید بزرگ‌تر از صفر باشد");
        assert!(self.camera.occlusion_terrain_rim_width > 0.0, "occlusion_terrain_rim_width باید بزرگ‌تر از صفر باشد");
        for sp in &self.map.swing_points {
            assert!(sp.trigger_radius > 0.0, "swing_points[{}].trigger_radius باید بزرگ‌تر از صفر باشد", sp.id);
            assert!(sp.arc_sag_depth >= 0.0, "swing_points[{}].arc_sag_depth نمی‌تواند منفی باشد", sp.id);
            assert!(sp.pulley_scale > 0.0, "swing_points[{}].pulley_scale باید بزرگ‌تر از صفر باشد", sp.id);
            assert!(sp.ring_radius > 0.0, "swing_points[{}].ring_radius باید بزرگ‌تر از صفر باشد", sp.id);
            assert!(sp.ring_pulse_speed > 0.0, "swing_points[{}].ring_pulse_speed باید بزرگ‌تر از صفر باشد", sp.id);
        }
        for hero in &self.heroes {
            assert!(hero.wall_climb.attach_distance > 0.0, "hero.wall_climb.attach_distance باید بزرگ‌تر از صفر باشد");
            assert!(hero.wall_climb.climb_up_speed > 0.0, "hero.wall_climb.climb_up_speed باید بزرگ‌تر از صفر باشد");
            assert!(hero.wall_climb.climb_down_speed > 0.0, "hero.wall_climb.climb_down_speed باید بزرگ‌تر از صفر باشد");
            assert!(hero.wall_climb.top_ledge_trigger_below > 0.0, "hero.wall_climb.top_ledge_trigger_below باید بزرگ‌تر از صفر باشد");
            assert!(hero.wall_climb.top_exit_forward_dist > 0.0, "hero.wall_climb.top_exit_forward_dist باید بزرگ‌تر از صفر باشد");
            assert!(hero.wall_climb.climb_to_top_duration > 0.0, "hero.wall_climb.climb_to_top_duration باید بزرگ‌تر از صفر باشد");
        }
    }

    pub fn validate_pause(&self) {
        assert!(self.pause.max_pauses_per_team > 0, "pause.max_pauses_per_team باید بزرگ‌تر از صفر باشد");
        assert!(self.pause.team_time_bank_secs > 0.0, "pause.team_time_bank_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.pause.player_cooldown_secs >= 0.0, "pause.player_cooldown_secs نمی‌تواند منفی باشد");
        assert!(self.pause.manual_unpause_lock_secs >= 0.0, "pause.manual_unpause_lock_secs نمی‌تواند منفی باشد");
        assert!(self.pause.disconnect_unpause_lock_secs >= 0.0, "pause.disconnect_unpause_lock_secs نمی‌تواند منفی باشد");
        assert!(self.pause.unpause_countdown_secs > 0.0, "pause.unpause_countdown_secs باید بزرگ‌تر از صفر باشد");
    }

    pub fn is_night_at(&self, match_elapsed_secs: f32) -> bool {
        let dur = self.day_night.phase_duration_secs.max(1.0);
        let phase_idx = (match_elapsed_secs.max(0.0) / dur).floor() as u64;
        phase_idx % 2 == 1
    }

    pub fn day_night_blend_at(&self, match_elapsed_secs: f32) -> f32 {
        let dur = self.day_night.phase_duration_secs.max(1.0);
        let trans = self.day_night.transition_duration_secs.clamp(0.1, dur * 0.45);
        let t = match_elapsed_secs.max(0.0);
        let phase_idx = (t / dur).floor() as u64;
        let phase_elapsed = t - (phase_idx as f32) * dur;
        let in_night = phase_idx % 2 == 1;
        if phase_idx == 0 {
            return 0.0;
        }
        let ramp = (phase_elapsed / trans).clamp(0.0, 1.0);
        let smooth = ramp * ramp * (3.0 - 2.0 * ramp);
        if in_night { smooth } else { 1.0 - smooth }
    }

    pub fn hero_effective_vision(&self, hero: &HeroDef, match_elapsed_secs: f32, weather: WeatherKind) -> f32 {
        let is_night = self.is_night_at(match_elapsed_secs);
        let base = if is_night {
            hero.night_vision_range
        } else {
            hero.vision_range
        };
        let env_bonus = hero.environment.combined(is_night, weather).vision_range_bonus;
        (base + env_bonus).max(4.0)
    }

    pub fn weather_duration_bounds(&self, weather: WeatherKind) -> (f32, f32) {
        match weather {
            WeatherKind::Clear => (self.weather.clear_min_duration_secs, self.weather.clear_max_duration_secs),
            WeatherKind::Rainy => (self.weather.rainy_min_duration_secs, self.weather.rainy_max_duration_secs),
            WeatherKind::Snowy => (self.weather.snowy_min_duration_secs, self.weather.snowy_max_duration_secs),
        }
    }

    pub fn validate_weather(&self) {
        let w = &self.weather;
        assert!(w.weather_cycle_start_secs.is_finite() && w.weather_cycle_start_secs >= 0.0, "weather.weather_cycle_start_secs نمی‌تواند منفی یا نامعتبر باشد");
        assert!(w.clear_min_duration_secs >= 5.0 && w.clear_max_duration_secs >= w.clear_min_duration_secs, "بازه زمانی آب و هوای صاف نامعتبر است");
        assert!(w.rainy_min_duration_secs >= 5.0 && w.rainy_max_duration_secs >= w.rainy_min_duration_secs, "بازه زمانی آب و هوای بارانی نامعتبر است");
        assert!(w.snowy_min_duration_secs >= 5.0 && w.snowy_max_duration_secs >= w.snowy_min_duration_secs, "بازه زمانی آب و هوای برفی نامعتبر است");
        assert!(w.clear_weight > 0.0 && w.rainy_weight > 0.0 && w.snowy_weight > 0.0, "وزن احتمالات آب و هوا باید مثبت باشد");
        assert!(w.transition_duration_secs > 0.0 && w.transition_duration_secs <= 30.0, "weather.transition_duration_secs نامعتبر است");
        assert!(w.rain_fall_speed > 0.1 && w.rain_density > 0.05 && w.rain_streak_width > 0.002 && w.rain_streak_length > 0.02, "پارامترهای شیدر باران نامعتبر است");
        assert!(w.snow_fall_speed > 0.02 && w.snow_density > 0.05 && w.snow_flake_size > 0.005 && (0.05..=1.0).contains(&w.snow_flake_softness), "پارامترهای شیدر برف نامعتبر است");
        assert!(w.lightning_min_interval_secs >= 1.0 && w.lightning_max_interval_secs >= w.lightning_min_interval_secs, "بازه زمانی رعد و برق نامعتبر است");
        assert!(w.lightning_flash_duration_secs > 0.05 && w.thunder_sound_delay_secs >= 0.0, "مدت فلش رعد و برق نامعتبر است");
        assert!(!w.thunder_sound.trim().is_empty() && !w.rain_loop_sound.trim().is_empty() && !w.snow_loop_sound.trim().is_empty(), "مسیر فایل‌های صوتی آب و هوا نباید خالی باشد");
        assert!((0.0..=2.0).contains(&w.thunder_volume) && (0.0..=2.0).contains(&w.rain_loop_volume) && (0.0..=2.0).contains(&w.snow_loop_volume), "ولوم صدای آب و هوا باید بین 0 تا 2 باشد");
        assert!(w.audio_fade_speed > 0.0, "weather.audio_fade_speed باید مثبت باشد");
        let s = &w.snow_accumulation;
        assert!(!s.shader_path.trim().is_empty(), "weather.snow_accumulation.shader_path خالی است");
        assert!(
            s.accumulate_duration_secs > 0.0 && s.melt_duration_secs > 0.0,
            "مدت زمان نشستن و آب شدن برف باید مثبت باشد"
        );
        assert!(
            (0.0..=1.0).contains(&s.slope_threshold) && s.slope_softness > 0.0,
            "پارامترهای شیب نشستن برف نامعتبر است"
        );
        assert!(
            s.noise_scale > 0.0 && s.noise_strength >= 0.0 && s.ground_coverage_boost >= 0.0,
            "پارامترهای نویز و پوشش زمین برف نامعتبر است"
        );
        assert!(
            (0.05..=1.0).contains(&s.snow_roughness) && s.sparkle_strength >= 0.0 && s.sparkle_scale > 0.0,
            "پارامترهای زبری و درخشش برف نامعتبر است"
        );
        assert!(
            s.night_snow_glow_strength >= 0.0
                && s.night_directional_illuminance_mult > 0.0
                && s.night_ambient_brightness_mult > 0.0
                && s.night_fog_start_mult > 0.0
                && s.night_fog_end_mult > 0.0,
            "پارامترهای روشنایی شب برفی نامعتبر است"
        );
    }

    pub fn effective_vision_multiplier(&self, match_elapsed_secs: f32, night_mult: f32) -> f32 {
        if self.is_night_at(match_elapsed_secs) {
            night_mult
        } else {
            1.0
        }
    }

    pub fn validate_day_night(&self) {
        let dn = &self.day_night;
        assert!(dn.phase_duration_secs >= 10.0, "day_night.phase_duration_secs باید حداقل 10 ثانیه باشد");
        assert!(dn.transition_duration_secs > 0.0 && dn.transition_duration_secs < dn.phase_duration_secs * 0.5, "day_night.transition_duration_secs نامعتبر است");
        assert!(dn.night_creep_vision_multiplier > 0.1 && dn.night_creep_vision_multiplier <= 1.0, "night_creep_vision_multiplier نامعتبر است");
        assert!(dn.night_courier_vision_multiplier > 0.1 && dn.night_courier_vision_multiplier <= 1.0, "night_courier_vision_multiplier نامعتبر است");
        assert!(dn.night_turret_vision_multiplier > 0.1 && dn.night_turret_vision_multiplier <= 1.0, "night_turret_vision_multiplier نامعتبر است");
        assert!(dn.day_fog_end > dn.day_fog_start && dn.night_fog_end > dn.night_fog_start, "فواصل مه روز و شب نامعتبر است");
        assert!(!dn.day_start_sound.trim().is_empty() && !dn.night_start_sound.trim().is_empty(), "مسیر صدای شب و روز نباید خالی باشد");
        assert!((0.0..=2.0).contains(&dn.day_start_volume) && (0.0..=2.0).contains(&dn.night_start_volume), "حجم صدای شب و روز باید بین 0 تا 2 باشد");
    }

    pub fn validate_buff_hud(&self) {
        let b = &self.buff_hud;
        assert!(
            b.bottom_px >= 0.0 && b.circle_size_px >= 20.0 && b.ring_thickness_px >= 1.5,
            "خطای بحرانی کانفیگ: ابعاد نوار باف و دیباف (buff_hud) نامعتبر است"
        );
        assert!(
            b.circle_size_px > b.ring_thickness_px * 2.5 && b.circle_gap_px >= 0.0,
            "خطای بحرانی کانفیگ: ضخامت حلقه یا فاصله دایره‌های buff_hud نامعتبر است"
        );
        assert!(
            b.ring_segments >= 12 && b.ring_segments <= 72,
            "خطای بحرانی کانفیگ: buff_hud.ring_segments باید بین 12 تا 72 باشد"
        );
        assert!(
            b.popup_width_px >= 180.0 && b.popup_bottom_offset_px >= 10.0,
            "خطای بحرانی کانفیگ: ابعاد کارت پاپ‌آپ buff_hud نامعتبر است"
        );
    }

    pub fn validate_telemetry(&self) {
        assert!(
            self.telemetry.refresh_interval_secs > 0.0 && self.telemetry.refresh_interval_secs <= 5.0,
            "telemetry.refresh_interval_secs باید بین 0 و 5 ثانیه باشد"
        );
        assert!(
            self.telemetry.low_fps_critical_threshold > 0
                && self.telemetry.low_fps_critical_threshold < self.telemetry.low_fps_warning_threshold,
            "telemetry.low_fps_critical_threshold باید مثبت و کمتر از low_fps_warning_threshold باشد"
        );
        assert!(
            self.telemetry.high_ping_warning_ms > 0
                && self.telemetry.high_ping_warning_ms < self.telemetry.high_ping_critical_ms,
            "telemetry.high_ping_warning_ms باید مثبت و کمتر از high_ping_critical_ms باشد"
        );
        assert!(
            matches!(self.lighting.msaa_samples, 1 | 2 | 4 | 8),
            "lighting.msaa_samples باید یکی از مقادیر 1، 2، 4 یا 8 باشد"
        );
        assert!(
            self.lighting.shadow_map_size >= 256
                && self.lighting.shadow_map_size <= 4096
                && self.lighting.shadow_map_size.is_power_of_two(),
            "lighting.shadow_map_size باید توان 2 بین 256 تا 4096 باشد"
        );
        assert!(
            self.map.fog_fade_speed > 0.0 && self.map.fog_fade_speed <= 30.0,
            "map.fog_fade_speed باید بین 0 و 30 باشد"
        );
        assert!(
            self.map.fog_blur_passes <= 6,
            "map.fog_blur_passes باید بین 0 تا 6 باشد"
        );
        assert!(
            (0.0..=1.0).contains(&self.map.fog_explored_alpha)
                && (0.0..=1.0).contains(&self.map.fog_unexplored_alpha),
            "map.fog_explored_alpha و fog_unexplored_alpha باید بین 0 و 1 باشند"
        );
        assert!(
            self.map.high_ground_occlusion_min_diff > 0.0
                && self.map.high_ground_occlusion_min_diff.is_finite(),
            "map.high_ground_occlusion_min_diff باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            self.map.fow_ray_step_size > 0.0
                && self.map.fow_ray_step_size <= 4.0
                && self.map.fow_ray_step_size.is_finite(),
            "map.fow_ray_step_size باید در بازه (0.0, 4.0] باشد"
        );
        assert!(
            self.map.fow_blocker_min_dist >= 0.0 && self.map.fow_blocker_min_dist.is_finite(),
            "map.fow_blocker_min_dist باید عددی نامنفی و معتبر باشد"
        );
        assert!(
            self.map.fow_fountain_vision_range > 0.0
                && self.map.fow_ancient_vision_range > 0.0
                && self.map.fow_barracks_vision_range > 0.0
                && self.map.fow_fountain_vision_range.is_finite()
                && self.map.fow_ancient_vision_range.is_finite()
                && self.map.fow_barracks_vision_range.is_finite(),
            "شعاع دید سازه‌ها در map (fow_*_vision_range) باید بزرگ‌تر از صفر باشد"
        );
        assert!(
            self.map.fow_stand_eye_height > self.map.fow_crouch_eye_height
                && self.map.fow_crouch_eye_height > 0.0
                && self.map.fow_stand_eye_height.is_finite()
                && self.map.fow_crouch_eye_height.is_finite(),
            "map.fow_stand_eye_height و fow_crouch_eye_height نامعتبر هستند"
        );
        assert!(
            self.map.fow_stand_visible_height > self.map.fow_crouch_visible_height
                && self.map.fow_crouch_visible_height > 0.0
                && self.map.fow_stand_visible_height.is_finite()
                && self.map.fow_crouch_visible_height.is_finite(),
            "map.fow_stand_visible_height و fow_crouch_visible_height نامعتبر هستند"
        );
        assert!(
            self.map.fow_low_cover_ground_epsilon >= 0.0
                && self.map.fow_low_cover_ground_epsilon.is_finite(),
            "map.fow_low_cover_ground_epsilon باید عددی نامنفی و معتبر باشد"
        );
    }

    pub fn validate_ping(&self) {
        assert!(self.ping.duration_secs > 0.0, "ping.duration_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.max_active_pings > 0, "ping.max_active_pings باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.world_ring_radius > 0.0, "ping.world_ring_radius باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.beam_height > 0.0, "ping.beam_height باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.minimap_pulse_speed > 0.0, "ping.minimap_pulse_speed باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.min_interval_secs.is_finite() && self.ping.min_interval_secs >= 0.0, "ping.min_interval_secs نامعتبر است");
    }

    pub fn validate_audio(&self) {
        let a = &self.audio;
        assert!(a.spatial_scale > 0.0, "audio.spatial_scale باید بزرگ‌تر از صفر باشد");
        assert!(a.max_hearing_distance > 1.0, "audio.max_hearing_distance باید بزرگ‌تر از 1 باشد");
        assert!(a.footstep_interval_secs > 0.05, "audio.footstep_interval_secs نامعتبر است");
        assert!(!a.swim_loop_sound.trim().is_empty(), "audio.swim_loop_sound نمی‌تواند خالی باشد");
        assert!(!a.water_submerge_sound.trim().is_empty(), "audio.water_submerge_sound نمی‌تواند خالی باشد");
        assert!(a.shallow_water_step_interval_secs > 0.05, "audio.shallow_water_step_interval_secs نامعتبر است");
        assert!(a.creep_fire_max_concurrent > 0, "audio.creep_fire_max_concurrent باید حداقل 1 باشد");
        assert!(a.low_hp_threshold_ratio > 0.0 && a.low_hp_threshold_ratio < 1.0, "audio.low_hp_threshold_ratio باید بین 0 و 1 باشد");
        assert!(a.heartbeat_min_interval_secs > 0.1 && a.heartbeat_max_interval_secs >= a.heartbeat_min_interval_secs, "بازه ضربان قلب نامعتبر است");
        assert!(!a.grenade_water_explosion_sound.trim().is_empty(), "audio.grenade_water_explosion_sound نمی‌تواند خالی باشد");
        assert!(a.swing_start_volume >= 0.0, "audio.swing_start_volume نمی‌تواند منفی باشد");
        assert!(a.swing_air_loop_volume >= 0.0, "audio.swing_air_loop_volume نمی‌تواند منفی باشد");
        assert!(a.swing_end_volume >= 0.0, "audio.swing_end_volume نمی‌تواند منفی باشد");
        assert!(a.swing_land_volume >= 0.0, "audio.swing_land_volume نمی‌تواند منفی باشد");
        assert!(a.hero_select_music_volume >= 0.0, "audio.hero_select_music_volume نمی‌تواند منفی باشد");
        assert!(a.hero_select_pick_volume >= 0.0, "audio.hero_select_pick_volume نمی‌تواند منفی باشد");
        assert!(a.team_ping_volume >= 0.0, "audio.team_ping_volume نمی‌تواند منفی باشد");
        assert!(a.game_pause_volume >= 0.0, "audio.game_pause_volume نمی‌تواند منفی باشد");
        assert!(a.unpause_countdown_volume >= 0.0, "audio.unpause_countdown_volume نمی‌تواند منفی باشد");
        assert!(!a.not_enough_gold_sound.trim().is_empty(), "audio.not_enough_gold_sound نمی‌تواند خالی باشد");
        assert!(a.not_enough_gold_volume.is_finite() && a.not_enough_gold_volume >= 0.0, "audio.not_enough_gold_volume نمی‌تواند منفی یا نامعتبر باشد");
        assert!(!a.shop_enter_sound.trim().is_empty(), "audio.shop_enter_sound نمی‌تواند خالی باشد");
        assert!(a.shop_enter_volume.is_finite() && a.shop_enter_volume >= 0.0, "audio.shop_enter_volume نامعتبر است");
        assert!(!a.shop_exit_sound.trim().is_empty(), "audio.shop_exit_sound نمی‌تواند خالی باشد");
        assert!(a.shop_exit_volume.is_finite() && a.shop_exit_volume >= 0.0, "audio.shop_exit_volume نامعتبر است");
        assert!(!a.inventory_full_sound.trim().is_empty(), "audio.inventory_full_sound نمی‌تواند خالی باشد");
        assert!(a.inventory_full_volume.is_finite() && a.inventory_full_volume >= 0.0, "audio.inventory_full_volume نامعتبر است");

        let mm = &self.main_menu;
        assert!(!mm.background_image_path.trim().is_empty(), "main_menu.background_image_path نمی‌تواند خالی باشد");
        assert!(!mm.soldier_model_path.trim().is_empty(), "main_menu.soldier_model_path نمی‌تواند خالی باشد");
        assert!(!mm.soldier_idle_animation.trim().is_empty(), "main_menu.soldier_idle_animation نمی‌تواند خالی باشد");
        assert!(mm.party_max_size >= 2 && mm.party_max_size <= 5, "main_menu.party_max_size باید بین ۲ تا ۵ باشد");
        assert!(mm.squad_slots.len() >= mm.party_max_size, "main_menu.squad_slots باید حداقل به تعداد party_max_size جایگاه داشته باشد");
        assert!(mm.notification_duration_secs.is_finite() && mm.notification_duration_secs > 0.0, "main_menu.notification_duration_secs نامعتبر است");
        assert!(mm.party_invite_timeout_secs.is_finite() && mm.party_invite_timeout_secs > 0.0, "main_menu.party_invite_timeout_secs نامعتبر است");
        assert!(mm.camera_fov_deg.is_finite() && mm.camera_fov_deg > 10.0 && mm.camera_fov_deg < 120.0, "main_menu.camera_fov_deg نامعتبر است");
        assert!(mm.key_light_illuminance.is_finite() && mm.key_light_illuminance >= 0.0, "main_menu.key_light_illuminance نامعتبر است");
        assert!(mm.rim_light_intensity.is_finite() && mm.rim_light_intensity >= 0.0, "main_menu.rim_light_intensity نامعتبر است");
        assert!(mm.ambient_light_brightness.is_finite() && mm.ambient_light_brightness >= 0.0, "main_menu.ambient_light_brightness نامعتبر است");
        assert!(!mm.game_modes.is_empty(), "main_menu.game_modes نمی‌تواند خالی باشد");
        assert!(!mm.default_map_name.trim().is_empty(), "main_menu.default_map_name نمی‌تواند خالی باشد");
        assert!(!mm.lobby_capacities.is_empty() && mm.lobby_capacities.iter().all(|&c| c >= 2 && c <= 10 && c % 2 == 0), "main_menu.lobby_capacities باید شامل ظرفیت‌های زوج بین ۲ تا ۱۰ باشد");
        assert!(mm.abandon_mmr_penalty > 0, "main_menu.abandon_mmr_penalty باید بزرگ‌تر از صفر باشد");
        assert!(mm.mmr_tier_bar_step > 0, "main_menu.mmr_tier_bar_step باید بزرگ‌تر از صفر باشد");
        assert!(mm.toast_duration_secs.is_finite() && mm.toast_duration_secs > 0.0, "main_menu.toast_duration_secs نامعتبر است");
        assert!(mm.social_reconnect_interval_secs.is_finite() && mm.social_reconnect_interval_secs > 0.0, "main_menu.social_reconnect_interval_secs نامعتبر است");
        assert!(mm.default_queue_required_players >= 2, "main_menu.default_queue_required_players باید حداقل ۲ باشد");
        assert!(mm.http_timeout_secs > 0, "main_menu.http_timeout_secs باید بزرگ‌تر از صفر باشد");
        assert!(mm.ws_ping_interval_secs > 0, "main_menu.ws_ping_interval_secs باید بزرگ‌تر از صفر باشد");
        assert!(mm.ws_poll_sleep_ms > 0, "main_menu.ws_poll_sleep_ms باید بزرگ‌تر از صفر باشد");
        for (i, slot) in mm.squad_slots.iter().enumerate() {
            assert!(slot.scale.is_finite() && slot.scale > 0.0, "main_menu.squad_slots[{}].scale نامعتبر است", i);
            assert!(slot.yaw_deg.is_finite(), "main_menu.squad_slots[{}].yaw_deg نامعتبر است", i);
            assert!(slot.position.iter().all(|v| v.is_finite()), "main_menu.squad_slots[{}].position نامعتبر است", i);
        }
        for (hid, hw) in &a.hero_weapons {
            assert!(
                !hw.primary_fire_sound.trim().is_empty(),
                "primary_fire_sound برای '{}' نمی‌تواند خالی باشد",
                hid
            );
            assert!(
                hw.primary_fire_volume.is_finite() && hw.primary_fire_volume >= 0.0,
                "primary_fire_volume برای '{}' نمی‌تواند منفی یا نامعتبر باشد",
                hid
            );
            assert!(
                !hw.primary_impact_sound.trim().is_empty(),
                "primary_impact_sound برای '{}' نمی‌تواند خالی باشد",
                hid
            );
            assert!(
                hw.primary_impact_volume.is_finite() && hw.primary_impact_volume >= 0.0,
                "primary_impact_volume برای '{}' نمی‌تواند منفی یا نامعتبر باشد",
                hid
            );
            assert!(
                !hw.pistol_fire_sound.trim().is_empty(),
                "pistol_fire_sound برای '{}' نمی‌تواند خالی باشد",
                hid
            );
            assert!(
                hw.pistol_fire_volume.is_finite() && hw.pistol_fire_volume >= 0.0,
                "pistol_fire_volume برای '{}' نمی‌تواند منفی یا نامعتبر باشد",
                hid
            );
            assert!(
                !hw.pistol_impact_sound.trim().is_empty(),
                "pistol_impact_sound برای '{}' نمی‌تواند خالی باشد",
                hid
            );
            assert!(
                hw.pistol_impact_volume.is_finite() && hw.pistol_impact_volume >= 0.0,
                "pistol_impact_volume برای '{}' نمی‌تواند منفی یا نامعتبر باشد",
                hid
            );
        }
        for hero in &self.heroes {
            assert!(
                a.hero_weapons.contains_key(&hero.id),
                "تنظیمات صدای سلاح برای هیرو '{}' در audio.hero_weapons تعریف نشده است!",
                hero.id
            );
        }
    }

    pub fn validate_scoreboard(&self) {
        let sb = &self.scoreboard;
        assert!(sb.panel_width_px > 100.0, "scoreboard.panel_width_px باید بزرگ‌تر از 100 باشد");
        assert!(sb.row_height_px > 20.0, "scoreboard.row_height_px باید بزرگ‌تر از 20 باشد");
        assert!(sb.portrait_width_px > 10.0 && sb.portrait_height_px > 10.0, "ابعاد پورتریت scoreboard نامعتبر است");
        assert!(sb.level_ring_size_px > 16.0, "scoreboard.level_ring_size_px باید بزرگ‌تر از 16 باشد");
        assert!(sb.xp_ring_segments >= 8, "scoreboard.xp_ring_segments باید حداقل 8 باشد");
        assert!(sb.item_icon_size_px > 10.0, "scoreboard.item_icon_size_px باید بزرگ‌تر از 10 باشد");
        assert!(sb.max_item_slots > 0, "scoreboard.max_item_slots باید بزرگ‌تر از صفر باشد");
    }

    pub fn validate_chat(&self) {
        assert!(self.chat.max_message_len > 0, "chat.max_message_len باید بزرگ‌تر از صفر باشد");
        assert!(self.chat.max_visible_messages > 0, "chat.max_visible_messages باید بزرگ‌تر از صفر باشد");
        assert!(self.chat.message_fade_secs > 0.0, "chat.message_fade_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.chat.min_interval_secs.is_finite() && self.chat.min_interval_secs >= 0.0, "chat.min_interval_secs نامعتبر است");
    }

    pub fn validate_ancients(&self) {
        assert!(
            self.map.ancients.len() == 2,
            "خطای بحرانی کانفیگ: map.ancients باید دقیقاً شامل ۲ ساختمان اصلی (Dawn و Dusk) باشد!"
        );
        let mut has_dawn = false;
        let mut has_dusk = false;
        for a in &self.map.ancients {
            assert!(a.team != TeamId::Neutral, "تیم Ancient '{}' نمی‌تواند Neutral باشد!", a.id);
            assert!(a.max_hp > 0.0, "max_hp ساختمان اصلی '{}' باید بزرگ‌تر از صفر باشد!", a.id);
            assert!(a.armor >= 0.0, "armor ساختمان اصلی '{}' نمی‌تواند منفی باشد!", a.id);
            if a.team == TeamId::Dawn {
                has_dawn = true;
            } else if a.team == TeamId::Dusk {
                has_dusk = true;
            }
        }
        assert!(has_dawn && has_dusk, "هر دو تیم Dawn و Dusk باید Ancient داشته باشند!");
    }

    pub fn validate_barracks(&self) {
        assert!(
            !self.map.barracks.is_empty(),
            "خطای بحرانی کانفیگ: لیست map.barracks خالی است!"
        );
        let bb = &self.map.barracks_buffs;
        assert!(bb.melee_destroyed_hp_multiplier >= 1.0, "melee_destroyed_hp_multiplier باید >= 1.0 باشد");
        assert!(bb.melee_destroyed_scale_multiplier >= 1.0, "melee_destroyed_scale_multiplier باید >= 1.0 باشد");
        assert!(bb.melee_destroyed_armor_bonus >= 0.0, "melee_destroyed_armor_bonus باید >= 0.0 باشد");
        assert!(bb.ranged_destroyed_damage_multiplier >= 1.0, "ranged_destroyed_damage_multiplier باید >= 1.0 باشد");
        assert!(
            bb.ranged_destroyed_fire_rate_multiplier > 0.0 && bb.ranged_destroyed_fire_rate_multiplier <= 1.0,
            "ranged_destroyed_fire_rate_multiplier باید بین (0.0, 1.0] باشد"
        );
        for b in &self.map.barracks {
            assert!(b.team != TeamId::Neutral, "تیم باراکس '{}' نمی‌تواند Neutral باشد!", b.id);
            assert!(b.size.0 > 0.0 && b.size.1 > 0.0 && b.size.2 > 0.0, "اندازه باراکس '{}' نامعتبر است!", b.id);
            assert!(b.max_hp > 0.0, "max_hp باراکس '{}' باید بزرگ‌تر از صفر باشد!", b.id);
        }
    }

    pub fn validate_roshan_pit(&self) {
        let p = &self.map.roshan_pit;
        assert!(p.outer_radius > p.inner_radius && p.inner_radius > 0.5, "شعاع‌های غار روشان نامعتبر است");
        assert!(p.wall_height > 1.0 && p.floor_thickness > 0.05 && p.roof_thickness > 0.05, "ابعاد ارتفاعی غار روشان نامعتبر است");
        assert!(p.entrance_half_angle_deg > 5.0 && p.entrance_half_angle_deg < 85.0, "زاویه ورودی غار روشان نامعتبر است");
        assert!(!p.climb_enter_anim.trim().is_empty(), "climb_enter_anim غار روشان نمی‌تواند خالی باشد");
        assert!(p.climb_enter_distance > 0.5, "climb_enter_distance غار روشان باید بزرگ‌تر از 0.5 باشد");
    }

    pub fn validate_fountains(&self) {
        assert!(
            !self.map.fountains.is_empty(),
            "خطای بحرانی کانفیگ: لیست map.fountains خالی است!"
        );
        for ftn in &self.map.fountains {
            assert!(
                ftn.team != TeamId::Neutral,
                "خطای بحرانی کانفیگ: تیم چشمه '{}' نمی‌تواند Neutral باشد!",
                ftn.id
            );
            assert!(
                ftn.heal_radius > 0.0,
                "خطای بحرانی کانفیگ: heal_radius چشمه '{}' باید بزرگ‌تر از صفر باشد!",
                ftn.id
            );
            assert!(
                ftn.heal_per_sec > 0.0,
                "خطای بحرانی کانفیگ: heal_per_sec چشمه '{}' باید بزرگ‌تر از صفر باشد!",
                ftn.id
            );
            assert!(
                ftn.enemy_damage_per_sec.is_finite() && ftn.enemy_damage_per_sec > 0.0,
                "خطای بحرانی کانفیگ: enemy_damage_per_sec چشمه '{}' باید بزرگ‌تر از صفر باشد!",
                ftn.id
            );
            assert!(
                ftn.fire_rate_secs.is_finite() && ftn.fire_rate_secs > 0.0,
                "خطای بحرانی کانفیگ: fire_rate_secs چشمه '{}' باید بزرگ‌تر از صفر باشد!",
                ftn.id
            );
            assert!(
                ftn.projectile_speed.is_finite() && ftn.projectile_speed > 0.0,
                "خطای بحرانی کانفیگ: projectile_speed چشمه '{}' باید بزرگ‌تر از صفر باشد!",
                ftn.id
            );
            assert!(
                ftn.true_sight_radius.is_finite() && ftn.true_sight_radius > 0.0,
                "خطای بحرانی کانفیگ: true_sight_radius چشمه '{}' باید بزرگ‌تر از صفر باشد!",
                ftn.id
            );
        }
    }

    pub fn active_hero(&self, index: usize) -> &HeroDef {
        self.heroes.get(index).unwrap_or_else(|| {
            panic!(
                "خطای بحرانی: هیرو با اندیس {} در config.ron تعریف نشده است!",
                index
            )
        })
    }

    pub fn hero_by_id(&self, id: &str) -> &HeroDef {
        self.heroes.iter().find(|h| h.id == id).unwrap_or_else(|| self.active_hero(0))
    }

    pub fn validate_biome(&self) {
        let b = &self.map.biome;
        assert!(
            !b.left_tree_model_path.trim().is_empty(),
            "خطای بحرانی کانفیگ: map.biome.left_tree_model_path خالی است"
        );
        assert!(
            !b.right_tree_model_path.trim().is_empty(),
            "خطای بحرانی کانفیگ: map.biome.right_tree_model_path خالی است"
        );
        assert!(
            b.ground_noise_scale > 0.0 && b.ground_noise_scale.is_finite(),
            "خطای بحرانی کانفیگ: map.biome.ground_noise_scale باید مثبت باشد"
        );
        assert!(
            b.river_transition_width > 0.0 && b.river_transition_width.is_finite(),
            "خطای بحرانی کانفیگ: map.biome.river_transition_width باید مثبت باشد"
        );

        let l = &b.autumn_leaves;
        assert!(
            !l.shader_path.trim().is_empty(),
            "خطای بحرانی کانفیگ: map.biome.autumn_leaves.shader_path خالی است"
        );
        assert!(
            l.vein_contrast >= 0.0 && l.vein_contrast <= 1.0 && l.edge_darkening >= 0.0 && l.edge_darkening <= 1.0,
            "خطای بحرانی کانفیگ: پارامترهای ظاهری برگ‌های پاییزی باید بین 0 تا 1 باشد"
        );
        assert!(
            l.leaf_count > 0,
            "خطای بحرانی کانفیگ: map.biome.autumn_leaves.leaf_count باید مثبت باشد"
        );
        assert!(
            l.leaf_size_min > 0.0 && l.leaf_size_max >= l.leaf_size_min,
            "خطای بحرانی کانفیگ: ابعاد برگ‌های پاییزی نامعتبر است"
        );
        assert!(
            l.ground_y_offset > 0.0 && l.min_river_edge_margin >= 0.0 && l.lane_clear_margin >= 0.0 && l.base_clear_margin >= 0.0,
            "خطای بحرانی کانفیگ: ارتفاع و حاشیه‌های برگ‌های پاییزی نامعتبر است"
        );
        assert!(
            l.scatter_radius > 0.0 && l.min_walker_speed >= 0.0 && l.scatter_outward_impulse > 0.0 && l.scatter_upward_impulse > 0.0,
            "خطای بحرانی کانفیگ: پارامترهای پخش شدن برگ‌های پاییزی نامعتبر است"
        );
        assert!(
            l.gravity > 0.0 && l.air_drag >= 0.0 && l.ground_friction > 0.0 && l.spatial_cell_size > 0.0,
            "خطای بحرانی کانفیگ: پارامترهای فیزیک برگ‌های پاییزی نامعتبر است"
        );
        assert!(
            !l.palette_rgb.is_empty(),
            "خطای بحرانی کانفیگ: پالت رنگی برگ‌های پاییزی نباید خالی باشد"
        );
        for (idx, c) in l.palette_rgb.iter().enumerate() {
            assert!(
                c.0 >= 0.0 && c.1 >= 0.0 && c.2 >= 0.0 && c.0.is_finite() && c.1.is_finite() && c.2.is_finite(),
                "خطای بحرانی کانفیگ: رنگ شماره {} در پالت برگ‌های پاییزی نامعتبر است",
                idx
            );
        }

        assert!(
            b.tree_fow_blocker_radius_mult > 0.0
                && b.tree_fow_blocker_min_radius > 0.0
                && b.tree_fow_blocker_max_radius >= b.tree_fow_blocker_min_radius,
            "خطای بحرانی کانفیگ: پارامترهای مسدودسازی دید (FOW) درختان در map.biome نامعتبر است"
        );
        assert!(
            b.tree_min_scale.0 > 0.0
                && b.tree_min_scale.1 > 0.0
                && b.tree_min_scale.2 > 0.0
                && b.tree_max_scale.0 >= b.tree_min_scale.0
                && b.tree_max_scale.1 >= b.tree_min_scale.1
                && b.tree_max_scale.2 >= b.tree_min_scale.2,
            "خطای بحرانی کانفیگ: بازه tree_min_scale و tree_max_scale در map.biome نامعتبر است"
        );

        for tree in &self.map.trees {
            let scl = tree.model_transform.scale;
            assert!(
                scl.0 >= b.tree_min_scale.0
                    && scl.0 <= b.tree_max_scale.0
                    && scl.1 >= b.tree_min_scale.1
                    && scl.1 <= b.tree_max_scale.1
                    && scl.2 >= b.tree_min_scale.2
                    && scl.2 <= b.tree_max_scale.2,
                "خطای بحرانی کانفیگ: مقیاس درخت '{}' برابر ({:.2}, {:.2}, {:.2}) خارج از بازه مجاز ({:.2}..={:.2}) است",
                tree.id,
                scl.0,
                scl.1,
                scl.2,
                b.tree_min_scale.0,
                b.tree_max_scale.0
            );
            let loc = tree.model_transform.location;
            let expected_model = if is_left_of_river(&self.map.canyon, loc.0, loc.2) {
                &b.left_tree_model_path
            } else {
                &b.right_tree_model_path
            };
            assert!(
                tree.model_path == *expected_model,
                "خطای بحرانی کانفیگ: درخت '{}' در مختصات ({:.2}, {:.2}) مدل '{}' دارد ولی باید '{}' باشد",
                tree.id,
                loc.0,
                loc.2,
                tree.model_path,
                expected_model
            );
        }
    }

    pub fn validate_unique_map_ids(&self) {
        let mut seen = HashSet::new();
        let mut check_id = |id: &str, category: &str| {
            assert!(
                !id.trim().is_empty(),
                "خطای بحرانی کانفیگ: شناسه خالی در بخش '{}' مجاز نیست!",
                category
            );
            assert!(
                seen.insert(id.to_string()),
                "خطای بحرانی کانفیگ: شناسه تکراری '{}' در بخش '{}' پیدا شد! تمام آبجکت‌های نقشه باید شناسه منحصر‌به‌فرد داشته باشند.",
                id,
                category
            );
        };

        for item in &self.map.terrains {
            check_id(&item.id, "map.terrains");
        }
        for item in &self.map.ramps {
            check_id(&item.id, "map.ramps");
        }
        for item in &self.map.river_segments {
            check_id(&item.id, "map.river_segments");
        }
        for item in &self.map.waterfalls {
            check_id(&item.id, "map.waterfalls");
        }
        check_id(&self.map.roshan_pit.id, "map.roshan_pit");
        for item in &self.map.swing_points {
            check_id(&item.id, "map.swing_points");
        }
        check_id(&self.map.center_bridge.id, "map.center_bridge");
        for item in &self.map.fountains {
            check_id(&item.id, "map.fountains");
        }
        for item in &self.map.ancients {
            check_id(&item.id, "map.ancients");
            assert!(
                item.hp_bar_height_offset > 0.0 && item.hp_bar_height_offset.is_finite(),
                "خطای بحرانی در config.ron: مقدار hp_bar_height_offset برای {} باید بزرگ‌تر از صفر باشد",
                item.id
            );
        }
        for item in &self.map.turrets {
            check_id(&item.id, "map.turrets");
        }
        for item in &self.map.barracks {
            check_id(&item.id, "map.barracks");
        }
        for item in &self.map.mountains {
            check_id(&item.id, "map.mountains");
        }
        for item in &self.map.trees {
            check_id(&item.id, "map.trees");
        }
        for item in &self.map.lanes {
            check_id(&item.id, "map.lanes");
            assert!(
                item.waypoints.len() >= 3,
                "خطای بحرانی در config.ron: لاین '{}' باید حداقل ۳ waypoint داشته باشد تا ورودی پایگاه Dawn و Dusk متمایز باشند",
                item.id
            );
            for (side_name, stair) in [
                ("dawn_entrance_stair", &item.dawn_entrance_stair),
                ("dusk_entrance_stair", &item.dusk_entrance_stair),
            ] {
                assert!(
                    !stair.model_path.trim().is_empty(),
                    "خطای بحرانی در config.ron: فیلد model_path برای {}.{} خالی است",
                    item.id,
                    side_name
                );
                let s = stair.model_transform.scale;
                assert!(
                    s.0.is_finite() && s.1.is_finite() && s.2.is_finite()
                        && s.0 > 0.0 && s.1 > 0.0 && s.2 > 0.0,
                    "خطای بحرانی در config.ron: مقدار scale برای {}.{} نامعتبر است: {:?}",
                    item.id,
                    side_name,
                    s
                );
            }
        }
        for item in &self.obstacles {
            check_id(&item.id, "obstacles");
        }
        for item in &self.targets {
            check_id(&item.id, "targets");
        }
    }
}

pub const CHANNEL_UNRELIABLE_STATE: u8 = 0;
pub const CHANNEL_RELIABLE_FX: u8 = 1;
pub const CHANNEL_RELIABLE_LOBBY: u8 = 2;

/// خواندن اجباری آرگومان‌های خط فرمان سرور:
/// حالت systemd (۳ آرگومان): `<port> <room_id> <required_players>`
/// حالت تست محلی (۲ آرگومان): `<port> <required_players>`
pub fn parse_server_cli_args() -> (u16, Option<String>, usize) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        panic!(
            "خطای بحرانی: آرگومان‌های سرور ناقص است! مثال:\n  moba_server <port> <room_id> <required_players>"
        );
    }
    let port: u16 = args[1].parse().unwrap_or_else(|e| {
        panic!("خطای بحرانی: پورت '{}' معتبر نیست: {}", args[1], e)
    });
    assert!(port > 0, "خطای بحرانی: شماره پورت باید بزرگ‌تر از صفر باشد");

    if args.len() >= 4 {
        let room_id = args[2].trim().to_string();
        assert!(!room_id.is_empty(), "خطای بحرانی: شناسه room_id نمی‌تواند خالی باشد");
        let required_players: usize = args[3].parse().unwrap_or_else(|e| {
            panic!("خطای بحرانی: تعداد نفرات '{}' معتبر نیست: {}", args[3], e)
        });
        assert!(
            required_players >= 1,
            "خطای بحرانی: تعداد نفرات مجاز باید حداقل ۱ نفر باشد"
        );
        (port, Some(room_id), required_players)
    } else {
        let required_players: usize = args[2].parse().unwrap_or_else(|e| {
            panic!("خطای بحرانی: تعداد نفرات '{}' معتبر نیست: {}", args[2], e)
        });
        assert!(
            required_players >= 1,
            "خطای بحرانی: تعداد نفرات مجاز باید حداقل ۱ نفر باشد"
        );
        (port, None, required_players)
    }
}

/// خواندن اختیاری پورت و client_id برای کلاینت جهت اتصال چند بازیکن هم‌زمان: `<port> <client_id>`
pub fn apply_client_cli_overrides(net: &mut NetworkConfig) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        if args[1].contains(':') {
            let _: SocketAddr = args[1].parse().unwrap_or_else(|e| {
                panic!("خطای بحرانی کلاینت: آدرس سرور '{}' معتبر نیست: {}", args[1], e)
            });
            net.server_public_addr = args[1].clone();
            net.server_bind_addr = args[1].clone();
        } else {
            let port: u16 = args[1].parse().unwrap_or_else(|e| {
                panic!("خطای بحرانی کلاینت: پورت '{}' معتبر نیست: {}", args[1], e)
            });
            let ip = net
                .server_public_addr
                .split(':')
                .next()
                .expect("آدرس سرور نامعتبر است")
                .to_string();
            net.server_public_addr = format!("{}:{}", ip, port);
            net.server_bind_addr = format!("{}:{}", ip, port);
        }
    }
    if args.len() >= 3 {
        let cid: u64 = args[2].parse().unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی کلاینت: شناسه کلاینت '{}' معتبر نیست: {}",
                args[2], e
            )
        });
        net.client_id = cid;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkConfig {
    pub server_bind_addr: String,
    pub server_public_addr: String,
    pub client_bind_addr: String,
    pub backend_http_url: String,
    pub backend_ws_url: String,
    pub redis_addr: String,
    pub server_tick_rate_hz: f64,
    pub protocol_id: u64,
    pub max_clients: usize,
    pub client_id: u64,
    pub connect_token_expire_secs: u64,
    pub connect_token_timeout_secs: i32,
    pub available_bytes_per_tick: u64,
    pub channel_max_memory_bytes: usize,
    pub reliable_resend_time_ms: u64,
    pub battle_loading_max_wait_secs: f32,
    pub post_match_shutdown_delay_secs: f32,
    pub empty_room_shutdown_secs: f32,
    pub private_key: [u8; 32],
}

pub fn build_shared_renet_connection_config(net: &NetworkConfig) -> ConnectionConfig {
    let channels = vec![
        ChannelConfig {
            channel_id: CHANNEL_UNRELIABLE_STATE,
            max_memory_usage_bytes: net.channel_max_memory_bytes,
            send_type: SendType::Unreliable,
        },
        ChannelConfig {
            channel_id: CHANNEL_RELIABLE_FX,
            max_memory_usage_bytes: net.channel_max_memory_bytes,
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(net.reliable_resend_time_ms),
            },
        },
        ChannelConfig {
            channel_id: CHANNEL_RELIABLE_LOBBY,
            max_memory_usage_bytes: net.channel_max_memory_bytes,
            send_type: SendType::ReliableOrdered {
                resend_time: Duration::from_millis(net.reliable_resend_time_ms),
            },
        },
    ];

    ConnectionConfig {
        available_bytes_per_tick: net.available_bytes_per_tick,
        client_channels_config: channels.clone(),
        server_channels_config: channels,
    }
}

pub fn create_shared_renet_server(net: &NetworkConfig) -> (RenetServer, NetcodeServerTransport) {
    let server_addr: SocketAddr = net.server_bind_addr.parse().unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی سرور: آدرس '{}' معتبر نیست: {}",
            net.server_bind_addr, e
        )
    });
    let socket = UdpSocket::bind(server_addr).unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی سرور: امکان بایند روی '{}' نیست: {}",
            net.server_bind_addr, e
        )
    });
    socket
        .set_nonblocking(true)
        .expect("خطای بحرانی: تنظیم حالت non-blocking سوکت سرور شکست خورد");

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("خطای بحرانی: ساعت سیستم نامعتبر است");

    let mut public_addresses = vec![server_addr];
    if let Ok(pub_addr) = net.server_public_addr.parse::<SocketAddr>() {
        if !public_addresses.contains(&pub_addr) {
            public_addresses.push(pub_addr);
        }
    }
    let server_config = ServerConfig {
        current_time,
        max_clients: net.max_clients,
        protocol_id: net.protocol_id,
        public_addresses,
        authentication: ServerAuthentication::Secure {
            private_key: net.private_key,
        },
    };

    let transport = NetcodeServerTransport::new(server_config, socket)
        .expect("خطای بحرانی سرور: ساخت NetcodeServerTransport شکست خورد");
    let server = RenetServer::new(build_shared_renet_connection_config(net));

    (server, transport)
}

pub fn create_shared_renet_client(net: &NetworkConfig) -> (RenetClient, NetcodeClientTransport) {
    let server_addr: SocketAddr = net.server_public_addr.parse().unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی کلاینت: آدرس عمومی سرور '{}' معتبر نیست: {}",
            net.server_public_addr, e
        )
    });
    let client_addr: SocketAddr = net.client_bind_addr.parse().unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی کلاینت: آدرس کلاینت '{}' معتبر نیست: {}",
            net.client_bind_addr, e
        )
    });
    let socket = UdpSocket::bind(client_addr).unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی کلاینت: امکان بایند روی '{}' نیست: {}",
            net.client_bind_addr, e
        )
    });
    socket
        .set_nonblocking(true)
        .expect("خطای بحرانی کلاینت: تنظیم حالت non-blocking سوکت شکست خورد");

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("خطای بحرانی: ساعت سیستم نامعتبر است");

    let connect_token = ConnectToken::generate(
        current_time,
        net.protocol_id,
        net.connect_token_expire_secs,
        net.client_id,
        net.connect_token_timeout_secs,
        vec![server_addr],
        None,
        &net.private_key,
    )
    .expect("خطای بحرانی کلاینت: تولید ConnectToken شکست خورد");

    let auth = ClientAuthentication::Secure { connect_token };
    let transport = NetcodeClientTransport::new(current_time, auth, socket)
        .expect("خطای بحرانی کلاینت: ساخت NetcodeClientTransport شکست خورد");
    let client = RenetClient::new(build_shared_renet_connection_config(net));

    (client, transport)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CameraConfig {
    pub offset: (f32, f32, f32),
    pub follow_lerp_speed: f32,
    pub trauma_decay_rate: f32,
    pub shake_amplitude: (f32, f32, f32),
    pub look_at_shake_factor: f32,
    pub cursor_lead_max_distance_x: f32,
    pub cursor_lead_max_distance_z: f32,
    pub cursor_lead_deadzone_ratio: f32,
    pub cursor_lead_lerp_speed: f32,
    pub edge_pan_margin_px: f32,
    pub edge_pan_speed: f32,
    pub occlusion_fade_speed: f32,
    pub occlusion_edge_color_rgb: (f32, f32, f32),
    pub occlusion_edge_emissive: f32,
    pub occlusion_edge_power: f32,
    pub occlusion_edge_threshold: f32,
    pub occlusion_terrain_cutout_radius: f32,
    pub occlusion_terrain_rim_width: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LightingConfig {
    pub msaa_samples: u32,
    pub bloom_enabled: bool,
    pub shadow_map_size: usize,
    pub directional_illuminance: f32,
    pub directional_pos: (f32, f32, f32),
    pub directional_shadow_depth_bias: f32,
    pub directional_shadow_normal_bias: f32,
    pub ambient_color_rgb: (f32, f32, f32),
    pub ambient_brightness: f32,
    pub ground_y: f32,
    pub ground_size: (f32, f32),
    pub ground_color_rgb: (f32, f32, f32),
    pub ground_roughness: f32,
    pub low_wall_color_rgb: (f32, f32, f32),
    pub low_wall_roughness: f32,
    pub tall_wall_color_rgb: (f32, f32, f32),
    pub tall_wall_roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeaponTunerConfig {
    pub initial_offset: (f32, f32, f32),
    pub initial_rotation_deg: (f32, f32, f32),
    pub initial_scale: f32,
    pub calibration_move_speed: f32,
    pub calibration_rot_speed_deg: f32,
    pub muzzle_local_offset: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PhysicsConfig {
    pub substeps: usize,
    pub gravity_y: f32,
    pub ground_half_thickness: f32,
    pub ground_friction: f32,
    pub ground_restitution: f32,
    pub obstacle_friction: f32,
    pub obstacle_restitution: f32,
    pub target_friction: f32,
    pub target_restitution: f32,
    pub soldier_capsule_half_segment: f32,
    pub soldier_capsule_center_y: f32,
    pub character_gravity_step_y: f32,
    pub character_skin_offset: f32,
    pub character_max_slope_climb_deg: f32,
    pub character_min_slope_slide_deg: f32,
    pub character_autostep_max_height: f32,
    pub character_autostep_min_width: f32,
    pub character_snap_to_ground: f32,
    pub character_normal_nudge_factor: f32,
    pub grenade_collider_radius: f32,
    pub grenade_restitution: f32,
    pub grenade_friction: f32,
    pub grenade_linear_damping: f32,
    pub grenade_angular_damping: f32,
    pub grenade_gravity_scale: f32,
    pub grenade_initial_angular_vel: (f32, f32, f32),
    pub shell_collider_radius: f32,
    pub shell_collider_half_height: f32,
    pub shell_restitution: f32,
    pub shell_friction: f32,
    pub shell_linear_damping: f32,
    pub shell_angular_damping: f32,
    pub shell_gravity_scale: f32,
    pub particle_collider_radius: f32,
    pub particle_restitution: f32,
    pub particle_friction: f32,
    pub particle_linear_damping: f32,
    pub particle_angular_damping: f32,
    pub particle_gravity_scale: f32,
    pub position_reconcile_snap_dist: f32,
    pub position_reconcile_lerp_factor: f32,
    pub aim_ray_min_dir_y_epsilon: f32,
    pub aim_target_min_dist_sq: f32,
    pub muzzle_aim_min_dist_sq: f32,
    pub orient_min_dir_sq: f32,
    pub water_move_speed_multiplier: f32,
    pub water_splash_interval_secs: f32,
    pub deep_water_swim_depth: f32,
    pub canyon_mountain_align: CanyonMountainAlignConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CanyonMountainAlignConfig {
    pub enabled: bool,
    pub id_prefix: String,
    pub target_edge_offset: f32,
    pub gradient_sample_eps: f32,
    pub max_snap_iterations: usize,
    pub align_yaw_to_canyon_normal: bool,
    pub yaw_offset_deg: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WeaponKind {
    AutomaticRifle,
    Sniper,
    Shotgun,
    Pistol,
    Throwable,
    ElectricBeam,
    FlameBeam,
    SpinningBlade,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveWeaponSlot {
    Primary,
    Pistol,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeaponModelDef {
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroWeaponModelsConfig {
    pub primary: WeaponModelDef,
    pub pistol: WeaponModelDef,
    pub run_upper_anim: String,
    pub sprint_offset: (f32, f32, f32),
    pub sprint_rotation_deg: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct HeroWeaponDef {
    pub weapon_kind: WeaponKind,
    pub magazine_size: u32,
    pub fire_rate_secs: f32,
    pub reload_duration_secs: f32,
    pub damage: f32,
    pub projectile_speed: f32,
    pub max_range: f32,
    pub pellets_per_shot: u32,
    pub spread_rad: f32,
    pub gravity: f32,
    pub launch_up_boost: f32,
    pub aoe_radius: f32,
    pub toxic_pool_duration_secs: f32,
    pub toxic_pool_dps: f32,
    pub toxic_slow_ratio: f32,
    pub beam_max_charge: f32,
    pub beam_drain_per_sec: f32,
    pub beam_dps: f32,
    pub beam_attack_speed_slow: f32,
    pub beam_slow_duration_secs: f32,
    pub beam_armor_reduction: f32,
    pub piercing: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CourierConfig {
    pub model_path: String,
    pub scale: f32,
    pub fly_height: f32,
    pub move_speed: f32,
    pub inventory_slots: usize,
    pub max_hp: f32,
    pub respawn_secs: f32,
    pub deliver_radius: f32,
    pub fountain_radius: f32,
    pub move_to_arrival_radius: f32,
    pub move_stop_epsilon: f32,
    pub hit_radius: f32,
    pub bounty_gold: u32,
    pub vision_range: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct HeroGrenadeDef {
    pub damage: f32,
    pub aoe_radius: f32,
    pub fuse_secs: f32,
    pub throw_duration_secs: f32,
    pub max_charges: u32,
    pub recharge_secs: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WeaponVfxProfile {
    pub shader_path: String,
    pub core_color_rgb: (f32, f32, f32),
    pub edge_color_rgb: (f32, f32, f32),
    pub bloom_intensity: f32,
    pub projectile_radius: f32,
    pub projectile_length: f32,
    pub trail_width: f32,
    pub trail_lifetime_secs: f32,
    pub trail_wave_freq: f32,
    pub trail_wave_amp: f32,
    pub trail_style_mode: f32,
    pub muzzle_flash_scale: f32,
    pub muzzle_flash_lifetime_secs: f32,
    pub muzzle_light_intensity: f32,
    pub muzzle_light_range: f32,
    pub muzzle_ring_start_scale: f32,
    pub muzzle_ring_end_scale: f32,
    pub impact_ring_end_scale: f32,
    pub impact_ring_lifetime_secs: f32,
    pub impact_light_intensity: f32,
    pub impact_light_range: f32,
    pub pulse_speed: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroBulletVfxConfig {
    pub primary: WeaponVfxProfile,
    pub pistol: WeaponVfxProfile,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroDef {
    pub id: String,
    pub display_name: String,
    pub model_path: String,
    pub portrait_path: String,
    pub team: TeamId,
    pub starting_gold: u32,
    pub xp_gain_radius: f32,
    pub max_hp: f32,
    pub hp_regen_per_sec: f32,
    pub max_stamina: f32,
    pub stamina_regen_per_sec: f32,
    pub slide_stamina_cost: f32,
    pub jump_over_stamina_cost: f32,
    pub run_stamina_drain_per_sec: f32,
    pub swim_stamina_drain_per_sec: f32,
    pub climb_stamina_drain_per_sec: f32,
    pub stamina_exhausted_resume_min: f32,
    pub exhausted_run_speed_multiplier: f32,
    pub exhausted_swim_speed_multiplier: f32,
    pub exhausted_climb_speed_multiplier: f32,
    pub armor: f32,
    pub hero_scale: f32,
    pub move_speed_multiplier: f32,
    pub shallow_water_speed_multiplier: f32,
    pub deep_water_speed_multiplier: f32,
    pub vision_range: f32,
    pub night_vision_range: f32,
    pub environment: HeroEnvironmentModifiers,
    pub high_ground_vision_bonus: f32,
    pub wall_climb: WallClimbConfig,
    pub swing: HeroSwingConfig,
    pub weapon: HeroWeaponDef,
    pub pistol: HeroWeaponDef,
    pub grenade: HeroGrenadeDef,
    pub level_growth: HeroLevelGrowth,
    pub courier: CourierConfig,
    pub weapon_models: HeroWeaponModelsConfig,
    pub bullet_vfx: HeroBulletVfxConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroLevelGrowth {
    pub fire_rate_reduction_per_level: f32,
    pub reload_time_reduction_per_level: f32,
    pub damage_per_level: f32,
    pub max_range_per_level: f32,
    pub max_hp_per_level: f32,
    pub hp_regen_per_level: f32,
    pub stamina_regen_per_level: f32,
    pub armor_per_level: f32,
    pub toxic_dps_per_level: f32,
    pub toxic_slow_per_level: f32,
    pub toxic_duration_per_level: f32,
    pub beam_dps_per_level: f32,
    pub beam_attack_speed_slow_per_level: f32,
    pub beam_armor_reduction_per_level: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroSwingConfig {
    pub swing_speed: f32,
    pub launch_delay_secs: f32,
    pub stamina_cost: f32,
    pub start_anim: String,
    pub end_anim: String,
    pub fall_anim: String,
    pub start_phase_fraction: f32,
    pub end_phase_fraction: f32,
    pub fall_gravity: f32,
    pub fall_forward_momentum: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SwingPointDef {
    pub id: String,
    pub point_a: (f32, f32, f32),
    pub point_b: (f32, f32, f32),
    pub pulley_pos: (f32, f32, f32),
    pub trigger_radius: f32,
    pub arc_sag_depth: f32,
    pub pulley_scale: f32,
    pub pulley_color_rgb: (f32, f32, f32),
    pub rope_color_rgb: (f32, f32, f32),
    pub ring_color_rgb: (f32, f32, f32),
    pub ring_emissive_rgb: (f32, f32, f32),
    pub ring_radius: f32,
    pub ring_pulse_speed: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WallClimbConfig {
    pub attach_distance: f32,
    pub climb_up_speed: f32,
    pub climb_down_speed: f32,
    pub top_exit_y: f32,
    pub top_ledge_trigger_below: f32,
    pub top_exit_forward_dist: f32,
    pub climb_to_top_duration: f32,
    pub anim_speed_scale: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SoldierConfig {
    pub spawn_pos: (f32, f32, f32),
    pub team_spawn_slot_spacing: f32,
    pub initial_aim_dir: (f32, f32, f32),
    pub initial_aim_hit_pos: (f32, f32, f32),
    pub aim_plane_height_offset: f32,
    pub player_radius: f32,
    pub run_speed: f32,
    pub walk_speed: f32,
    pub crouch_speed: f32,
    pub running_slide_speed: f32,
    pub jump_over_speed: f32,
    pub jump_over_clearance_height: f32,
    pub space_hold_threshold_secs: f32,
    pub dive_roll_speed: f32,
    pub cliff_dive_speed: f32,
    pub cliff_dive_arc_boost: f32,
    pub fall_dive_arc_boost: f32,
    pub cliff_edge_probe_step: f32,
    pub cliff_edge_drop_threshold: f32,
    pub cliff_edge_stop_margin: f32,
    pub cliff_chain_min_slide_dist: f32,
    pub cliff_edge_min_anim_duration_secs: f32,
    pub vertical_aim_key: String,
    pub quick_roll_speed: f32,
    pub vault_check_distance: f32,
    pub vault_travel_distance: f32,
    pub vault_ray_height: f32,
    pub vault_anim_reference_height: f32,
    pub vault_top_clearance: f32,
    pub vault_min_lift_height: f32,
    pub vault_max_lift_height: f32,
    pub vault_apex_fraction: f32,
    pub vault_arc_exponent: f32,
    pub vault_approach_max_angle_deg: f32,
    pub free_traversal_wall_padding: f32,
    pub death_extra_respawn_delay_secs: f32,
    pub death_animations: Vec<String>,
    pub vault_animations: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CombatConfig {
    pub fire_cooldown_secs: f32,
    pub bullet_speed: f32,
    pub bullet_lifetime_secs: f32,
    pub bullet_damage: f32,
    pub bullet_spread_rad: f32,
    pub fallback_muzzle_offset_y: f32,
    pub fallback_muzzle_forward: f32,
    pub shot_fire_trauma_add: f32,
    pub shot_fire_trauma_max: f32,
    pub target_destroy_trauma_add: f32,
    pub target_destroy_trauma_max: f32,
    pub grenade_release_fraction: f32,
    pub grenade_spawn_offset_y: f32,
    pub grenade_spawn_forward: f32,
    pub grenade_min_throw_dist: f32,
    pub grenade_max_throw_dist: f32,
    pub grenade_horizontal_vel_factor: f32,
    pub grenade_vertical_vel: f32,
    pub grenade_explosion_trauma_add: f32,
    pub grenade_explosion_trauma_max: f32,
    pub hero_stand_hitbox_height: f32,
    pub hero_crouch_hitbox_height: f32,
    pub hero_stand_aim_chest_height: f32,
    pub hero_crouch_aim_chest_height: f32,
    pub hero_hitbox_radius_multiplier: f32,
    pub aim_lock_radius_multiplier: f32,
    pub aim_lock_ray_max_distance: f32,
    pub bullet_hit_forgiveness_radius: f32,
    pub bullet_hit_forgiveness_height_bottom: f32,
    pub bullet_hit_forgiveness_height_top: f32,
    pub max_vertical_aim_pitch_sin: f32,
    pub min_fire_rate_secs: f32,
    pub min_reload_duration_secs: f32,
    pub turret_backdoor_creep_radius: f32,
    pub turret_bullet_spread_rad: f32,
    pub passive_gold_interval_secs: f32,
    pub passive_gold_amount: u32,
    pub hero_kill_gold: u32,
    pub hero_kill_xp: u32,
    pub hero_assist_gold: u32,
    pub hero_assist_xp: u32,
    pub hero_kill_gold_per_victim_level: f32,
    pub hero_kill_xp_per_victim_level: f32,
    pub hero_assist_gold_per_victim_level: f32,
    pub hero_assist_xp_per_victim_level: f32,
    pub underdog_gold_mult_per_level: f32,
    pub underdog_xp_mult_per_level: f32,
    pub overlevel_gold_penalty_per_level: f32,
    pub overlevel_xp_penalty_per_level: f32,
    pub min_bounty_gold_mult: f32,
    pub max_bounty_gold_mult: f32,
    pub min_bounty_xp_mult: f32,
    pub max_bounty_xp_mult: f32,
    pub streak_bonus_min_kills: u32,
    pub streak_bonus_max_steps: u32,
    pub streak_gold_bonus_per_kill: u32,
    pub streak_xp_bonus_per_kill: u32,
    pub hero_assist_window_secs: f32,
    pub hero_kill_credit_window_secs: f32,
    pub turret_hero_aggro_window_secs: f32,
    pub weapon_swap_min_cooldown_secs: f32,
    pub attack_reveal_duration_secs: f32,
    pub attack_reveal_vision_radius: f32,
    pub creep_hero_aggro_radius: f32,
    pub creep_hero_aggro_duration_secs: f32,
    pub creep_hero_aggro_cooldown_secs: f32,
    pub creep_hero_aggro_max_chase_dist: f32,
    pub respawn_times_per_level: Vec<f32>,
    pub xp_thresholds_per_level: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CursorReticleConfig {
    pub outer_diameter_px: f32,
    pub inner_diameter_px: f32,
    pub center_dot_px: f32,
    pub outer_border_px: f32,
    pub inner_border_px: f32,
    pub tick_length_px: f32,
    pub tick_thickness_px: f32,
    pub fire_expand_px: f32,
    pub expand_recover_speed: f32,
    pub normal_color_rgba: (f32, f32, f32, f32),
    pub locked_color_rgba: (f32, f32, f32, f32),
    pub reload_color_rgba: (f32, f32, f32, f32),
    pub hide_os_cursor_in_game: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaserConfig {
    pub beam_radius: f32,
    pub beam_core_ratio: f32,
    pub core_color_rgba: (f32, f32, f32, f32),
    pub glow_color_rgba: (f32, f32, f32, f32),
    pub bead_alpha: f32,
    pub emissive_strength: f32,
    pub pulse_speed: f32,
    pub pulse_frequency: f32,
    pub impact_dot_radius: f32,
    pub impact_ring_inner_radius: f32,
    pub impact_ring_outer_radius: f32,
    pub impact_pulse_speed: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreepConfig {
    pub dawn_model_path: String,
    pub dusk_model_path: String,
    pub scale: f32,
    pub collider_radius: f32,
    pub hitbox_height: f32,
    pub aim_chest_height: f32,
    pub muzzle_offset_y: f32,
    pub muzzle_forward: f32,
    pub first_wave_delay_secs: f32,
    pub wave_interval_secs: f32,
    pub creeps_per_wave: usize,
    pub count_upgrade_interval_secs: f32,
    pub count_upgrade_amount: usize,
    pub max_creeps_per_wave: usize,
    pub stat_upgrade_interval_secs: f32,
    pub hp_upgrade_per_step: f32,
    pub damage_upgrade_per_step: f32,
    pub armor_upgrade_per_step: f32,
    pub fire_rate_multiplier_per_step: f32,
    pub min_fire_rate_secs: f32,
    pub gold_upgrade_per_step: u32,
    pub wave_spawn_spacing: f32,
    pub move_speed: f32,
    pub fire_rate_secs: f32,
    pub vision_range: f32,
    pub attack_range: f32,
    pub bullet_damage: f32,
    pub projectile_speed: f32,
    pub bullet_max_range: f32,
    pub armor: f32,
    pub max_hp: f32,
    pub gold_bounty: u32,
    pub xp_bounty: u32,
    pub deny_hp_threshold_ratio: f32,
    pub deny_enemy_xp_multiplier: f32,
    pub anim_run: String,
    pub anim_idle: String,
    pub anim_fire: String,
    pub anim_die: String,
    pub die_despawn_delay_secs: f32,
    pub waypoint_reach_dist: f32,
    pub river_barrier_margin: f32,
    pub cliff_lookahead_dist: f32,
    pub lane_rejoin_max_deviation: f32,
    pub hero_block_detect_dist: f32,
    pub hero_block_front_cone_dot: f32,
    pub hero_block_hesitation_secs: f32,
    pub hero_block_hesitation_cooldown_secs: f32,
    pub hero_block_steer_strength: f32,
    pub obstacle_avoid_margin: f32,
    pub obstacle_avoid_lookahead_dist: f32,
    pub obstacle_avoid_steer_strength: f32,
    pub hovered_tint_emissive_rgb: (f32, f32, f32),
}

impl CreepConfig {
    pub fn model_path_for_team(&self, team: TeamId) -> &str {
        match team {
            TeamId::Dawn => &self.dawn_model_path,
            TeamId::Dusk => &self.dusk_model_path,
            TeamId::Neutral => panic!(
                "خطای بحرانی: برای تیم Neutral نباید از مدل سرباز لاین استفاده شود!"
            ),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NeutralCreepTypeDef {
    pub model_path: String,
    pub scale: f32,
    pub collider_radius: f32,
    pub hitbox_height: f32,
    pub aim_chest_height: f32,
    pub muzzle_offset_y: f32,
    pub muzzle_forward: f32,
    pub move_speed: f32,
    pub fire_rate_secs: f32,
    pub vision_range: f32,
    pub attack_range: f32,
    pub bullet_damage: f32,
    pub projectile_speed: f32,
    pub bullet_max_range: f32,
    pub armor: f32,
    pub max_hp: f32,
    pub gold_bounty: u32,
    pub xp_bounty: u32,
    pub anim_run: String,
    pub anim_idle: String,
    pub anim_fire: String,
    pub anim_die: String,
    pub die_despawn_delay_secs: f32,
    pub hovered_tint_emissive_rgb: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NeutralCampDef {
    pub id: String,
    pub team_side: TeamId,
    pub center: (f32, f32, f32),
    pub spawn_box_radius: f32,
    pub creep_type_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NeutralCampsConfig {
    pub first_spawn_delay_secs: f32,
    pub respawn_interval_secs: f32,
    pub spawn_spacing: f32,
    pub aggro_leash_radius: f32,
    pub leash_regen_pct_per_sec: f32,
    pub creep_types: HashMap<String, NeutralCreepTypeDef>,
    pub camps: Vec<NeutralCampDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BloodSplatterVfxConfig {
    pub decal_count_min: u32,
    pub decal_count_max: u32,
    pub decal_min_scale: f32,
    pub decal_max_scale: f32,
    pub decal_scatter_radius: f32,
    pub decal_lifetime_secs: f32,
    pub core_color_rgb: (f32, f32, f32),
    pub edge_color_rgb: (f32, f32, f32),
    pub gloss_strength: f32,
    pub air_droplet_count: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GrenadeExplosionVfxConfig {
    pub fireball_core_rgb: (f32, f32, f32),
    pub fireball_edge_rgb: (f32, f32, f32),
    pub fireball_bloom_intensity: f32,
    pub fireball_duration_secs: f32,
    pub water_core_rgb: (f32, f32, f32),
    pub water_foam_rgb: (f32, f32, f32),
    pub water_bloom_intensity: f32,
    pub water_geyser_height: f32,
    pub water_geyser_duration_secs: f32,
    pub shockwave_duration_secs: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FxConfig {
    pub trail_lifetime_secs: f32,
    pub muzzle_flash_lifetime_secs: f32,
    pub shell_lifetime_secs: f32,
    pub slide_dust_lifetime_secs: f32,
    pub hit_spark_lifetime_secs: f32,
    pub hit_spark_count_normal: u32,
    pub hit_spark_count_destroyed: u32,
    pub hit_spark_speed_normal: f32,
    pub hit_spark_speed_destroyed: f32,
    pub explosion_particle_count: u32,
    pub explosion_particle_lifetime_secs: f32,
    pub explosion_flash_lifetime_secs: f32,
    pub target_hit_flash_secs: f32,
    pub target_recover_lerp_speed: f32,
    pub level_up_vfx_duration_secs: f32,
    pub level_up_particle_count: u32,
    pub coin_burst_count: u32,
    pub coin_lifetime_secs: f32,
    pub floating_popup_duration_secs: f32,
    pub floating_popup_rise_px: f32,
    pub turret_muzzle_left_offset: (f32, f32, f32),
    pub turret_muzzle_right_offset: (f32, f32, f32),
    pub turret_recoil_offset: f32,
    pub turret_recoil_recover_speed: f32,
    pub turret_recoil_jitter_rad: f32,
    pub turret_aim_slerp_speed: f32,
    pub turret_hovered_tint_emissive_rgb: (f32, f32, f32),
    pub turret_idle_anim: String,
    pub turret_yaw_offset_rad: f32,
    pub creep_bullet_vfx: WeaponVfxProfile,
    pub turret_bullet_vfx: WeaponVfxProfile,
    pub blood_splatter_vfx: BloodSplatterVfxConfig,
    pub grenade_explosion_vfx: GrenadeExplosionVfxConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnimationConfig {
    pub model_path: String,
    pub death_blend_ms: u64,
    pub traversal_blend_ms: u64,
    pub locomotion_blend_ms: u64,
    pub grenade_blend_ms: u64,
    pub reload_blend_ms: u64,
    pub firing_blend_ms: u64,
    pub clip_durations: HashMap<String, f32>,
}

// -----------------------------------------------------------------------------
// ۱-ب. ساختارهای کامل نقشه MOBA + Tactical Shooter (همراه با model_path برای جایگزینی GLB)
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TeamId {
    Neutral,
    Dawn,
    Dusk,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaneId {
    Top,
    Mid,
    Bot,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeSize {
    Small,
    Large,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoverKind {
    LowWall,
    TallWall,
    Sandbag,
    ConcreteBarrier,
    CrateCluster,
    RockOutcrop,
    RuinedPillar,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModelTransformConfig {
    pub location: (f32, f32, f32),
    pub rotation: (f32, f32, f32),
    pub scale: (f32, f32, f32),
}

impl ModelTransformConfig {
    #[inline]
    pub fn location_vec(&self) -> Vec3 {
        Vec3::new(self.location.0, self.location.1, self.location.2)
    }

    #[inline]
    pub fn rotation_quat(&self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.rotation.1.to_radians(),
            self.rotation.0.to_radians(),
            self.rotation.2.to_radians(),
        )
    }

    #[inline]
    pub fn scale_vec(&self) -> Vec3 {
        Vec3::new(self.scale.0.abs().max(0.001), self.scale.1.abs().max(0.001), self.scale.2.abs().max(0.001))
    }

    #[inline]
    pub fn apply_to_transform(&self, base_pos: Vec3, base_rot: Quat) -> Transform {
        let s = self.scale_vec();
        Transform {
            translation: base_pos + base_rot * self.location_vec(),
            rotation: base_rot * self.rotation_quat(),
            scale: s,
        }
    }

    #[inline]
    pub fn to_transform(&self) -> Transform {
        Transform {
            translation: self.location_vec(),
            rotation: self.rotation_quat(),
            scale: self.scale_vec(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CanyonConfig {
    pub enabled: bool,
    pub start_corner: (f32, f32),
    pub end_corner: (f32, f32),
    pub water_y: f32,
    pub floor_depth: f32,
    pub base_half_width: f32,
    pub center_basin_boost: f32,
    pub meander_amplitude: f32,
    pub flow_dir: (f32, f32),
    pub water_color_rgb: (f32, f32, f32),
    pub foam_color_rgb: (f32, f32, f32),
    pub mist_color_rgb: (f32, f32, f32),
    pub mist_height_offset: f32,
    pub mist_fade_distance: f32,
    pub mist_intensity: f32,
    pub mist_drift_speed: f32,
    pub grid_resolution: (u32, u32),
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RoshanPitConfig {
    pub enabled: bool,
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub outer_radius: f32,
    pub inner_radius: f32,
    pub wall_height: f32,
    pub floor_thickness: f32,
    pub roof_thickness: f32,
    pub entrance_angle_deg: f32,
    pub entrance_half_angle_deg: f32,
    pub climb_enter_anim: String,
    pub climb_enter_distance: f32,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AutumnLeavesConfig {
    pub enabled: bool,
    pub leaf_count: u32,
    pub leaf_size_min: f32,
    pub leaf_size_max: f32,
    pub ground_y_offset: f32,
    pub min_river_edge_margin: f32,
    pub lane_clear_margin: f32,
    pub base_clear_margin: f32,
    pub scatter_radius: f32,
    pub min_walker_speed: f32,
    pub scatter_outward_impulse: f32,
    pub scatter_upward_impulse: f32,
    pub scatter_swirl_impulse: f32,
    pub scatter_spin_speed: f32,
    pub gravity: f32,
    pub air_drag: f32,
    pub flutter_amplitude: f32,
    pub flutter_frequency: f32,
    pub ground_friction: f32,
    pub spatial_cell_size: f32,
    pub roughness: f32,
    pub vein_contrast: f32,
    pub edge_darkening: f32,
    pub shader_path: String,
    pub palette_rgb: Vec<(f32, f32, f32)>,
    pub seed: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapBiomeConfig {
    pub left_tree_model_path: String,
    pub right_tree_model_path: String,
    pub tree_min_scale: (f32, f32, f32),
    pub tree_max_scale: (f32, f32, f32),
    pub tree_fow_blocker_radius_mult: f32,
    pub tree_fow_blocker_min_radius: f32,
    pub tree_fow_blocker_max_radius: f32,
    pub left_ground_primary_rgb: (f32, f32, f32),
    pub left_ground_secondary_rgb: (f32, f32, f32),
    pub left_ground_highlight_rgb: (f32, f32, f32),
    pub right_ground_primary_rgb: (f32, f32, f32),
    pub right_ground_secondary_rgb: (f32, f32, f32),
    pub right_ground_highlight_rgb: (f32, f32, f32),
    pub ground_noise_scale: f32,
    pub river_transition_width: f32,
    pub autumn_leaves: AutumnLeavesConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MapConfig {
    pub map_dimensions: (f32, f32),
    pub show_object_labels: bool,
    pub base_vision_range: f32,
    pub high_ground_vision_bonus: f32,
    pub high_ground_occlusion_min_diff: f32,
    pub fog_fade_speed: f32,
    pub fog_blur_passes: u32,
    pub fog_explored_alpha: f32,
    pub fog_unexplored_alpha: f32,
    pub fow_ray_step_size: f32,
    pub fow_blocker_min_dist: f32,
    pub fow_fountain_vision_range: f32,
    pub fow_ancient_vision_range: f32,
    pub fow_barracks_vision_range: f32,
    pub fow_stand_eye_height: f32,
    pub fow_crouch_eye_height: f32,
    pub fow_stand_visible_height: f32,
    pub fow_crouch_visible_height: f32,
    pub fow_low_cover_ground_epsilon: f32,
    pub canyon: CanyonConfig,
    pub biome: MapBiomeConfig,
    pub terrains: Vec<TerrainPlatformDef>,
    pub ramps: Vec<RampDef>,
    pub river_segments: Vec<RiverSegmentDef>,
    pub waterfalls: Vec<WaterfallDef>,
    pub roshan_pit: RoshanPitConfig,
    pub swing_points: Vec<SwingPointDef>,
    pub center_bridge: CenterBridgeDef,
    pub fountains: Vec<FountainDef>,
    pub ancients: Vec<AncientDef>,
    pub turrets: Vec<TurretDef>,
    pub barracks_buffs: BarracksBuffConfig,
    pub barracks: Vec<BarracksDef>,
    pub mountains: Vec<MountainDef>,
    pub trees: Vec<TreeDef>,
    pub lanes: Vec<LanePathDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TerrainPlatformDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub team: TeamId,
    pub elevation_tier: i32,
    pub size: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RampDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub size: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RiverSegmentDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub size: (f32, f32, f32),
    pub flow_dir: (f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub emissive_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaterfallDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub size: (f32, f32, f32),
    pub water_color_rgb: (f32, f32, f32),
    pub foam_emissive_rgb: (f32, f32, f32),
    pub mist_particle_interval_secs: f32,
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CenterBridgeDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub outer_radius: f32,
    pub deck_thickness: f32,
    pub elevation_tier: i32,
    pub color_rgb: (f32, f32, f32),
    pub accent_color_rgb: (f32, f32, f32),
    pub roughness: f32,
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FountainDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub team: TeamId,
    pub outer_radius: f32,
    pub inner_radius: f32,
    pub rim_height: f32,
    pub heal_radius: f32,
    pub heal_per_sec: f32,
    pub enemy_damage_per_sec: f32,
    pub fire_rate_secs: f32,
    pub projectile_speed: f32,
    pub true_sight_radius: f32,
    pub stone_color_rgb: (f32, f32, f32),
    pub water_emissive_rgb: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AncientDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub team: TeamId,
    pub base_size: (f32, f32, f32),
    pub core_radius: f32,
    pub core_height: f32,
    pub hp_bar_height_offset: f32,
    pub max_hp: f32,
    pub armor: f32,
    pub backdoor_protection_requires_creeps: bool,
    pub color_rgb: (f32, f32, f32),
    pub core_emissive_rgb: (f32, f32, f32),
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BarracksKind {
    Melee,
    Ranged,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BarracksBuffConfig {
    pub melee_destroyed_hp_multiplier: f32,
    pub melee_destroyed_scale_multiplier: f32,
    pub melee_destroyed_armor_bonus: f32,
    pub ranged_destroyed_damage_multiplier: f32,
    pub ranged_destroyed_fire_rate_multiplier: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BarracksDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub team: TeamId,
    pub lane: LaneId,
    pub kind: BarracksKind,
    pub size: (f32, f32, f32),
    pub max_hp: f32,
    pub armor: f32,
    pub gold_bounty: u32,
    pub xp_bounty: u32,
    pub backdoor_protection_requires_creeps: bool,
    pub color_rgb: (f32, f32, f32),
    pub accent_emissive_rgb: (f32, f32, f32),
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TurretDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub team: TeamId,
    pub lane: LaneId,
    pub tier: u8,
    pub base_radius: f32,
    pub height: f32,
    pub head_radius: f32,
    pub max_hp: f32,
    pub armor: f32,
    pub bullet_damage: f32,
    pub fire_rate_secs: f32,
    pub projectile_speed: f32,
    pub bullet_max_range: f32,
    pub vision_range: f32,
    pub attack_range: f32,
    pub gold_bounty: u32,
    pub xp_bounty: u32,
    pub backdoor_protection_requires_creeps: bool,
    pub color_rgb: (f32, f32, f32),
    pub crystal_emissive_rgb: (f32, f32, f32),
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MountainDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub size: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
    pub climbable: bool,
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TreeDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub tree_size: TreeSize,
    pub trunk_radius: f32,
    pub trunk_height: f32,
    pub canopy_radius: f32,
    pub canopy_height: f32,
    pub trunk_color_rgb: (f32, f32, f32),
    pub canopy_color_rgb: (f32, f32, f32),
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LaneEntranceStairDef {
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LanePathDef {
    pub id: String,
    pub lane: LaneId,
    pub width: f32,
    pub thickness: f32,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
    pub waypoints: Vec<(f32, f32, f32)>,
    pub occlusion_fade_enabled: bool,
    pub dawn_entrance_stair: LaneEntranceStairDef,
    pub dusk_entrance_stair: LaneEntranceStairDef,
}

/// اضلاع جانبی مجاز مکعب مستطیل برای اجرای حرکت Vault در فضای محلی (Local Space) آبجکت
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VaultSide {
    PosX,
    NegX,
    PosZ,
    NegZ,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ObstacleDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub kind: CoverKind,
    pub size: (f32, f32, f32),
    pub can_vault: bool,
    pub vault_sides: Vec<VaultSide>,
    pub vault_height_offset: f32,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
    pub occlusion_fade_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub size: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub max_hp: f32,
    pub respawn_secs: f32,
    pub roughness: f32,
    pub metallic: f32,
}

// -----------------------------------------------------------------------------
// ۲. مدل‌های مشترک وضعیت بازی و پکت‌های شبکه
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stance {
    Run,
    Walk,
    Crouch,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpperBodyAction {
    None,
    FiringRifle,
    Reloading,
    TossGrenade,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TraversalKind {
    RunningJump,
    JumpOver,
    RunningDiveRoll,
    QuickRollToRun,
    ObstacleVault,
    FallDive,
    PitClimb,
    ClimbToTop,
    RopeSwing,
    SwingFall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientInputCommand {
    pub move_input: Vec3,
    pub shift_held: bool,
    pub toggle_crouch_just_pressed: bool,
    pub reload_just_pressed: bool,
    pub grenade_just_pressed: bool,
    pub space_just_pressed: bool,
    pub space_held: bool,
    pub vertical_aim_held: bool,
    pub switch_weapon_just_pressed: bool,
    pub fire_just_pressed: bool,
    pub fire_held: bool,
    pub aim_ray_origin: Vec3,
    pub aim_ray_dir: Vec3,
    pub has_aim_ray: bool,
    pub client_muzzle_world_pos: Option<Vec3>,
}

impl ClientInputCommand {
    pub fn accumulate_from(&mut self, newer: ClientInputCommand) {
        self.move_input = newer.move_input;
        self.shift_held = newer.shift_held;
        self.fire_held = newer.fire_held;
        self.space_held = newer.space_held;
        self.vertical_aim_held = newer.vertical_aim_held;
        self.aim_ray_origin = newer.aim_ray_origin;
        self.aim_ray_dir = newer.aim_ray_dir;
        self.has_aim_ray = newer.has_aim_ray;
        self.client_muzzle_world_pos = newer.client_muzzle_world_pos;

        self.toggle_crouch_just_pressed |= newer.toggle_crouch_just_pressed;
        self.reload_just_pressed |= newer.reload_just_pressed;
        self.grenade_just_pressed |= newer.grenade_just_pressed;
        self.space_just_pressed |= newer.space_just_pressed;
        self.switch_weapon_just_pressed |= newer.switch_weapon_just_pressed;
        self.fire_just_pressed |= newer.fire_just_pressed;
    }

    pub fn consume_just_pressed(&mut self) {
        self.toggle_crouch_just_pressed = false;
        self.reload_just_pressed = false;
        self.grenade_just_pressed = false;
        self.space_just_pressed = false;
        self.switch_weapon_just_pressed = false;
        self.fire_just_pressed = false;
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImpactSurface {
    StoneWall,
    Tree,
    Turret,
    Character,
    Building,
    Water,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Debug)]
pub enum BulletVfxSource {
    Hero {
        hero_id: String,
        weapon_kind: WeaponKind,
    },
    Creep,
    Turret,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ShotAudioSource {
    Hero {
        hero_id: String,
        weapon_kind: WeaponKind,
    },
    Creep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroKillRewardEntry {
    pub player_name: String,
    pub is_killer: bool,
    pub gold: u32,
    pub xp: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GameplayFxEvent {
    LightningStrike {
        intensity: f32,
    },
    ShotFired {
        muzzle_pos: Vec3,
        shot_dir: Vec3,
        source: ShotAudioSource,
        trauma_add: f32,
        trauma_max: f32,
    },
    BulletTrailSegment {
        start: Vec3,
        end: Vec3,
        vfx_source: BulletVfxSource,
    },
    BulletImpact {
        impact_pos: Vec3,
        bullet_dir: Vec3,
        surface: ImpactSurface,
        target_destroyed: bool,
        trauma_add: f32,
        trauma_max: f32,
        vfx_source: BulletVfxSource,
    },
    GrenadeExploded {
        pos: Vec3,
        aoe_radius: f32,
        in_water: bool,
        water_y: f32,
        trauma_add: f32,
        trauma_max: f32,
        weapon_source: Option<BulletVfxSource>,
    },
    SlideDust {
        pos: Vec3,
    },
    WaterSplash {
        pos: Vec3,
        intensity: f32,
    },
    HeroLeveledUp {
        pos: Vec3,
        new_level: u32,
    },
    LastHitGold {
        pos: Vec3,
        gold: u32,
    },
    HeroKillFeed {
        killer_name: String,
        killer_team: TeamId,
        victim_name: String,
        victim_team: TeamId,
        rewards: Vec<HeroKillRewardEntry>,
    },
    CreepDenied {
        pos: Vec3,
    },
    TurretShotFired {
        turret_index: usize,
        barrel_left: bool,
        target_pos: Vec3,
        shot_dir: Vec3,
    },
    TeleportStarted {
        slot_index: usize,
        origin_pos: Vec3,
        dest_pos: Vec3,
    },
    TeleportCompleted {
        slot_index: usize,
        origin_pos: Vec3,
        dest_pos: Vec3,
    },
    TeleportCanceled {
        slot_index: usize,
        origin_pos: Vec3,
    },
    ShadowBladeActivated {
        slot_index: usize,
        pos: Vec3,
    },
    ShadowBladeEnded {
        slot_index: usize,
        pos: Vec3,
    },
    CombatPopup {
        pos: Vec3,
        text: String,
        color_rgb: (f32, f32, f32),
    },
    TacticalPulse {
        pos: Vec3,
        radius: f32,
        color_rgb: (f32, f32, f32),
    },
    DamageNumber {
        pos: Vec3,
        amount: f32,
        is_crit: bool,
    },
    EvadeMiss {
        pos: Vec3,
    },
    LifestealHeal {
        slot_index: usize,
        pos: Vec3,
        amount: f32,
    },
    ReflectHitPulse {
        slot_index: usize,
        pos: Vec3,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BulletSnapshot {
    pub id: u64,
    pub translation: Vec3,
    pub velocity: Vec3,
    pub vfx_source: BulletVfxSource,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ElectricBeamSnapshot {
    pub id: u64,
    pub hero_id: String,
    pub start_pos: Vec3,
    pub end_pos: Vec3,
    pub hit_target: bool,
    pub hit_any: bool,
    pub is_flame: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToxicPoolSnapshot {
    pub id: u64,
    pub pos: Vec3,
    pub radius: f32,
    pub remaining_secs: f32,
    pub total_duration: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GrenadeSnapshot {
    pub id: u64,
    pub translation: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CreepAnimState {
    Idle,
    Run,
    Fire,
    Die,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreepSnapshot {
    pub id: u64,
    pub team: TeamId,
    pub creep_type_id: Option<String>,
    pub pos: Vec3,
    pub facing_dir: Vec3,
    pub hp: f32,
    pub max_hp: f32,
    pub anim_state: CreepAnimState,
    pub scale_multiplier: f32,
    pub is_dead: bool,
    pub is_deniable: bool,
    pub move_speed: f32,
    pub armor: f32,
    pub damage: f32,
    pub attack_range: f32,
    pub fire_rate_secs: f32,
    pub is_visible: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AncientSnapshot {
    pub index: usize,
    pub team: TeamId,
    pub hp: f32,
    pub max_hp: f32,
    pub is_dead: bool,
    pub is_invulnerable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BarracksSnapshot {
    pub index: usize,
    pub team: TeamId,
    pub lane: LaneId,
    pub kind: BarracksKind,
    pub hp: f32,
    pub max_hp: f32,
    pub is_dead: bool,
    pub is_invulnerable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TurretSnapshot {
    pub index: usize,
    pub team: TeamId,
    pub hp: f32,
    pub max_hp: f32,
    pub is_dead: bool,
    pub is_invulnerable: bool,
    pub aim_target_pos: Option<Vec3>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetSnapshot {
    pub index: usize,
    pub hp: f32,
    pub max_hp: f32,
    pub hit_flash_timer: f32,
    pub is_dead: bool,
    pub is_visible: bool,
}


pub const HERO_PICK_TIME_SECS: f32 = 30.0;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MatchPhase {
    WaitingForPlayers,
    HeroSelect,
    BattleLoading,
    InGame,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LobbyPlayerInfo {
    pub slot_index: usize,
    pub client_id: Option<u64>,
    pub user_id: Option<String>,
    pub team: TeamId,
    pub connected: bool,
    pub selected_hero_id: Option<String>,
    pub map_loaded: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MatchStateSnapshot {
    pub phase: MatchPhase,
    pub required_players: usize,
    pub connected_count: usize,
    pub players: Vec<LobbyPlayerInfo>,
    pub draft_order: Vec<usize>,
    pub current_draft_step: usize,
    pub current_pick_slot: Option<usize>,
    pub pick_time_remaining: f32,
    pub taken_hero_ids: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatMessageKind {
    PlayerTeam,
    PlayerAll,
    SystemDisconnect,
    SystemReconnect,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessageEntry {
    pub id: u64,
    pub sender_name: String,
    pub sender_team: TeamId,
    pub kind: ChatMessageKind,
    pub all_chat: bool,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PausePhase {
    Running,
    Paused,
    UnpauseCountdown,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PauseSnapshot {
    pub phase: PausePhase,
    pub paused_by_team: Option<TeamId>,
    pub paused_by_name: String,
    pub is_disconnect_pause: bool,
    pub lock_remaining_secs: f32,
    pub countdown_remaining_secs: f32,
    pub your_team_pauses_left: u32,
    pub your_team_time_left_secs: f32,
    pub your_player_cooldown_secs: f32,
    pub can_you_unpause: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PingEntry {
    pub id: u64,
    pub sender_name: String,
    pub sender_team: TeamId,
    pub world_pos: Vec3,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ClientLobbyMessage {
    Authenticate { ticket_id: String },
    SelectHero { hero_id: String },
    ReportMapLoaded,
    SendChat { all_chat: bool, text: String },
    SendPing { world_pos: Vec3 },
    TogglePause,
    BuyItem { item_id: String },
    BuyCart { item_ids: Vec<String> },
    SellItem { inv_index: usize },
    CycleItemMode { inv_index: usize },
    SwapInventorySlots { from_slot: usize, to_slot: usize },
    SwapCourierSlots { from_slot: usize, to_slot: usize },
    SwapStashSlots { from_slot: usize, to_slot: usize },
    SellCourierItem { inv_index: usize },
    CourierDeliver,
    CourierReturnToFountain,
    CourierMoveTo { world_pos: Vec3 },
    TransferHeroItemToCourier { hero_slot: usize },
    TransferCourierItemToHero { courier_slot: usize },
    BuyRecipe { item_id: String },
    MoveHeroItemToStash { hero_slot: usize },
    MoveStashItemToHero { stash_slot: usize },
    MoveCourierItemToStash { courier_slot: usize },
    MoveStashItemToCourier { stash_slot: usize },
    SellStashItem { stash_slot: usize },
    TakeAllStashToHero,
    StartTeleport {
        inv_index: usize,
        target_pos: Vec3,
        from_minimap: bool,
    },
    CancelTeleport,
    ActivateShadowBlade { inv_index: usize },
    ActivateItem { inv_index: usize },
    DemoCommand(crate::demo_shared::DemoCommand),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerMatchMessage {
    pub your_slot: usize,
    pub state: MatchStateSnapshot,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ServerLobbyPacket {
    MatchState(ServerMatchMessage),
    SyncConfigs {
        game_config: GameConfig,
        items_config: crate::shop_shared::ItemsConfig,
    },
}

#[derive(Resource, Clone, Debug)]
pub struct AuthoritativeMatchState {
    pub phase: MatchPhase,
    pub required_players: usize,
    pub players: Vec<LobbyPlayerInfo>,
    pub draft_order: Vec<usize>,
    pub current_draft_step: usize,
    pub pick_time_remaining: f32,
    pub battle_loading_timer: f32,
    pub rng_state: u64,
    pub dirty: bool,
}

impl AuthoritativeMatchState {
    pub fn new(required_players: usize) -> Self {
        assert!(required_players >= 1, "تعداد نفرات مجاز باید حداقل ۱ نفر باشد");
        let players = (0..required_players)
            .map(|slot_index| LobbyPlayerInfo {
                slot_index,
                client_id: None,
                user_id: None,
                team: if slot_index % 2 == 0 {
                    TeamId::Dawn
                } else {
                    TeamId::Dusk
                },
                connected: false,
                selected_hero_id: None,
                map_loaded: false,
            })
            .collect();

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5EED_CAFE_1234)
            | 1;

        Self {
            phase: MatchPhase::WaitingForPlayers,
            required_players,
            players,
            draft_order: Vec::new(),
            current_draft_step: 0,
            pick_time_remaining: HERO_PICK_TIME_SECS,
            battle_loading_timer: 0.0,
            rng_state: seed,
            dirty: true,
        }
    }

    fn next_rand(&mut self) -> usize {
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng_state >> 32) as usize
    }

    fn shuffle_vec(&mut self, items: &mut Vec<usize>) {
        if items.len() <= 1 {
            return;
        }
        for i in (1..items.len()).rev() {
            let j = self.next_rand() % (i + 1);
            items.swap(i, j);
        }
    }

    pub fn connected_count(&self) -> usize {
        self.players.iter().filter(|p| p.connected).count()
    }

    pub fn slot_for_client(&self, client_id: u64) -> Option<usize> {
        self.players
            .iter()
            .find(|p| p.connected && p.client_id == Some(client_id))
            .map(|p| p.slot_index)
    }

    pub fn on_client_connected(&mut self, client_id: u64, config: &GameConfig) -> bool {
        self.on_client_connected_with_user(client_id, None, config)
            .is_some()
    }

    pub fn on_client_connected_with_user(
        &mut self,
        client_id: u64,
        user_id: Option<&str>,
        config: &GameConfig,
    ) -> Option<usize> {
        if self.slot_for_client(client_id).is_some() {
            return None;
        }
        if let Some(uid) = user_id {
            if self.players.iter().any(|p| p.connected && p.user_id.as_deref() == Some(uid)) {
                return None;
            }
        }
        // اول بررسی کن آیا این بازیکن (بر اساس user_id یا client_id) قبلاً در یکی از اسلات‌ها بوده (Reconnect)
        let reconnect_idx = user_id
            .and_then(|uid| {
                self.players
                    .iter()
                    .position(|p| !p.connected && p.user_id.as_deref() == Some(uid))
            })
            .or_else(|| {
                self.players
                    .iter()
                    .position(|p| !p.connected && p.client_id == Some(client_id))
            });
        let target_idx = reconnect_idx.or_else(|| {
            self.players
                .iter()
                .position(|p| !p.connected && p.client_id.is_none() && p.user_id.is_none())
        }).or_else(|| {
            if user_id.is_none() {
                self.players.iter().position(|p| !p.connected)
            } else {
                None
            }
        });
        let idx = target_idx?;
        let slot = &mut self.players[idx];
        let slot_index = slot.slot_index;
        slot.connected = true;
        slot.client_id = Some(client_id);
        if let Some(uid) = user_id {
            slot.user_id = Some(uid.to_string());
        }
        if self.phase == MatchPhase::InGame {
            slot.map_loaded = true;
        }
        self.dirty = true;

        if self.phase == MatchPhase::WaitingForPlayers
            && self.connected_count() == self.required_players
        {
            self.start_hero_select(config);
        }
        Some(slot_index)
    }

    pub fn on_client_disconnected(&mut self, client_id: u64) -> Option<usize> {
        if let Some(slot) = self
            .players
            .iter_mut()
            .find(|p| p.connected && p.client_id == Some(client_id))
        {
            let slot_idx = slot.slot_index;
            slot.connected = false;
            if self.phase == MatchPhase::WaitingForPlayers {
                slot.client_id = None;
                slot.selected_hero_id = None;
                slot.map_loaded = false;
            } else {
                // client_id را نگه می‌داریم تا بازیکن هنگام ریکانکت به اسلات و هیروی خودش برگردد
                slot.map_loaded = self.phase == MatchPhase::InGame;
            }
            self.dirty = true;
            Some(slot_idx)
        } else {
            None
        }
    }

    fn start_hero_select(&mut self, config: &GameConfig) {
        assert!(
            config.heroes.len() >= self.required_players,
            "خطای بحرانی: تعداد هیروهای تعریف‌شده ({}) کمتر از تعداد بازیکنان ({}) است و انتخاب تکراری مجاز نیست!",
            config.heroes.len(),
            self.required_players
        );

        let mut dawn_slots: Vec<usize> = self
            .players
            .iter()
            .filter(|p| p.team == TeamId::Dawn)
            .map(|p| p.slot_index)
            .collect();
        let mut dusk_slots: Vec<usize> = self
            .players
            .iter()
            .filter(|p| p.team == TeamId::Dusk)
            .map(|p| p.slot_index)
            .collect();

        self.shuffle_vec(&mut dawn_slots);
        self.shuffle_vec(&mut dusk_slots);

        let mut order = Vec::with_capacity(self.required_players);
        let max_len = dawn_slots.len().max(dusk_slots.len());
        for i in 0..max_len {
            if let Some(&s) = dawn_slots.get(i) {
                order.push(s);
            }
            if let Some(&s) = dusk_slots.get(i) {
                order.push(s);
            }
        }

        self.draft_order = order;
        self.current_draft_step = 0;
        self.pick_time_remaining = HERO_PICK_TIME_SECS;
        self.phase = MatchPhase::HeroSelect;
        self.dirty = true;
    }

    pub fn current_pick_slot(&self) -> Option<usize> {
        if self.phase != MatchPhase::HeroSelect {
            return None;
        }
        self.draft_order.get(self.current_draft_step).copied()
    }

    pub fn taken_hero_ids(&self) -> Vec<String> {
        self.players
            .iter()
            .filter_map(|p| p.selected_hero_id.clone())
            .collect()
    }

    pub fn try_select_hero(&mut self, client_id: u64, hero_id: &str, config: &GameConfig) -> bool {
        if self.phase != MatchPhase::HeroSelect {
            return false;
        }
        let Some(turn_slot) = self.current_pick_slot() else {
            return false;
        };
        if self.slot_for_client(client_id) != Some(turn_slot) {
            return false;
        }
        if !config.heroes.iter().any(|h| h.id == hero_id) {
            return false;
        }
        if self
            .players
            .iter()
            .any(|p| p.selected_hero_id.as_deref() == Some(hero_id))
        {
            return false;
        }

        self.players[turn_slot].selected_hero_id = Some(hero_id.to_string());
        self.advance_draft_turn();
        true
    }

    fn advance_draft_turn(&mut self) {
        self.current_draft_step += 1;
        if self.current_draft_step >= self.draft_order.len() {
            self.phase = MatchPhase::BattleLoading;
            self.battle_loading_timer = 0.0;
        } else {
            self.pick_time_remaining = HERO_PICK_TIME_SECS;
        }
        self.dirty = true;
    }

    pub fn on_map_loaded(&mut self, client_id: u64) -> bool {
        if self.phase != MatchPhase::BattleLoading {
            return false;
        }
        let Some(slot) = self.slot_for_client(client_id) else {
            return false;
        };
        if !self.players[slot].map_loaded {
            self.players[slot].map_loaded = true;
            self.dirty = true;
        }
        true
    }

    pub fn tick(&mut self, dt: f32, config: &GameConfig) -> Option<usize> {
        match self.phase {
            MatchPhase::HeroSelect => {
                self.pick_time_remaining -= dt;
                if self.pick_time_remaining <= 0.0 {
                    if let Some(turn_slot) = self.current_pick_slot() {
                        let taken = self.taken_hero_ids();
                        let available: Vec<String> = config
                            .heroes
                            .iter()
                            .filter(|h| !taken.contains(&h.id))
                            .map(|h| h.id.clone())
                            .collect();
                        if !available.is_empty() {
                            let idx = self.next_rand() % available.len();
                            self.players[turn_slot].selected_hero_id = Some(available[idx].clone());
                        }
                        self.advance_draft_turn();
                    }
                }
            }
            MatchPhase::BattleLoading => {
                self.battle_loading_timer += dt;
                let all_loaded = self
                    .players
                    .iter()
                    .all(|p| !p.connected || p.map_loaded);
                if self.connected_count() > 0
                    && ((all_loaded && self.battle_loading_timer >= 2.5)
                        || self.battle_loading_timer >= config.network.battle_loading_max_wait_secs)
                {
                    self.phase = MatchPhase::InGame;
                    self.dirty = true;
                    let first_hero_id = self
                        .players
                        .iter()
                        .find_map(|p| p.selected_hero_id.as_deref())
                        .expect("هیروی انتخاب‌شده در پایان درفت یافت نشد");
                    let hero_idx = config
                        .heroes
                        .iter()
                        .position(|h| h.id == first_hero_id)
                        .expect("هیروی انتخاب‌شده در config.ron یافت نشد");
                    return Some(hero_idx);
                }
            }
            _ => {}
        }
        None
    }

    pub fn snapshot(&self) -> MatchStateSnapshot {
        MatchStateSnapshot {
            phase: self.phase,
            required_players: self.required_players,
            connected_count: self.connected_count(),
            players: self.players.clone(),
            draft_order: self.draft_order.clone(),
            current_draft_step: self.current_draft_step,
            current_pick_slot: self.current_pick_slot(),
            pick_time_remaining: self.pick_time_remaining.max(0.0),
            taken_hero_ids: self.taken_hero_ids(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CourierOrder {
    #[default]
    Idle,
    ReturnToFountain,
    MoveTo,
    DeliverToHero,
    DeliverSlotToHero { courier_slot: usize },
    PickupStashThenDeliver,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CourierSnapshot {
    pub owner_slot: usize,
    pub team: TeamId,
    pub model_path: String,
    pub scale: f32,
    pub pos: Vec3,
    pub facing_dir: Vec3,
    pub hp: f32,
    pub max_hp: f32,
    pub is_dead: bool,
    pub respawn_remaining: f32,
    pub order: CourierOrder,
    pub inventory: Vec<InventoryItemState>,
    pub is_visible: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActiveBuffSnapshot {
    pub id: String,
    pub item_id: String,
    pub label: String,
    pub short_tag: String,
    pub details: Vec<String>,
    pub is_debuff: bool,
    pub is_timed: bool,
    pub remaining_secs: f32,
    pub total_secs: f32,
    pub color_rgb: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroSnapshot {
    pub hero_id: String,
    pub display_name: String,
    pub team: TeamId,
    pub gold: u32,
    pub level: u32,
    pub xp: u32,
    pub xp_into_level: u32,
    pub xp_for_next_level: u32,
    pub xp_gain_radius: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub hp_regen_per_sec: f32,
    pub in_fountain: bool,
    pub stamina: f32,
    pub max_stamina: f32,
    pub stamina_regen_per_sec: f32,
    pub armor: f32,
    pub hero_scale: f32,
    pub move_speed: f32,
    pub vision_range: f32,
    pub active_weapon: ActiveWeaponSlot,
    pub weapon_kind: WeaponKind,
    pub ammo_in_mag: u32,
    pub magazine_size: u32,
    pub fire_rate_secs: f32,
    pub reload_duration_secs: f32,
    pub weapon_damage: f32,
    pub projectile_speed: f32,
    pub weapon_max_range: f32,
    pub grenade_charges: u32,
    pub grenade_max_charges: u32,
    pub grenade_recharge_remaining: f32,
    pub grenade_throw_duration_secs: f32,
    pub last_hits: u32,
    pub denies: u32,
    pub assists: u32,
    pub kills: u32,
    pub deaths: u32,
    pub respawn_remaining: f32,
    pub inventory: Vec<InventoryItemState>,
    pub tp_slot: Option<InventoryItemState>,
    pub courier_hp: f32,
    pub courier_max_hp: f32,
    pub courier_is_dead: bool,
    pub courier_respawn_remaining: f32,
    pub courier_order: CourierOrder,
    pub courier_in_fountain: bool,
    pub courier_inventory: Vec<InventoryItemState>,
    pub stash: Vec<InventoryItemState>,
    pub shop_notice_seq: u64,
    pub shop_notice_text: String,
    pub shop_notice_is_error: bool,
    pub teleport_cooldown_remaining: f32,
    pub is_teleporting: bool,
    pub teleport_progress: f32,
    pub shadow_blade_cooldown_remaining: f32,
    pub reflect_armor_cooldown_remaining: f32,
    pub bullet_immunity_cooldown_remaining: f32,
    pub null_pulse_cooldown_remaining: f32,
    pub sonar_reveal_cooldown_remaining: f32,
    pub kinetic_dash_cooldown_remaining: f32,
    pub is_invisible: bool,
    pub invisibility_remaining: f32,
    pub is_reflect_armor_active: bool,
    pub is_bullet_immune: bool,
    pub is_cryo_debuffed: bool,
    pub is_null_muted: bool,
    pub is_sonar_revealed: bool,
    pub active_buffs: Vec<ActiveBuffSnapshot>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActiveTeleportSnapshot {
    pub slot_index: usize,
    pub team: TeamId,
    pub hero_id: String,
    pub model_path: String,
    pub hero_scale: f32,
    pub origin_pos: Vec3,
    pub dest_pos: Vec3,
    pub facing_dir: Vec3,
    pub progress: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeleportTargetBuilding {
    Turret(usize),
    Barracks(usize),
    Ancient(usize),
}

#[derive(Clone, Debug)]
pub struct ActiveTeleportState {
    pub inv_index: usize,
    pub origin_pos: Vec3,
    pub dest_pos: Vec3,
    pub facing_dir: Vec3,
    pub elapsed_secs: f32,
    pub total_secs: f32,
    pub cancel_on_left_click: bool,
    pub cancel_on_move: bool,
    pub target_building: Option<TeleportTargetBuilding>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ScoreboardXpInfo {
    pub xp: u32,
    pub xp_into_level: u32,
    pub xp_for_next_level: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScoreboardHeroInfo {
    pub slot_index: usize,
    pub team: TeamId,
    pub hero_id: String,
    pub display_name: String,
    pub portrait_path: String,
    pub is_local_player: bool,
    pub is_dead: bool,
    pub is_disconnected: bool,
    pub respawn_remaining: f32,
    pub level: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub xp_info: Option<ScoreboardXpInfo>,
    pub gold: Option<u32>,
    pub inventory: Option<Vec<InventoryItemState>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TopBarHeroInfo {
    pub slot_index: usize,
    pub team: TeamId,
    pub hero_id: String,
    pub display_name: String,
    pub portrait_path: String,
    pub is_dead: bool,
    pub is_disconnected: bool,
    pub respawn_remaining: f32,
    pub hp_ratio: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemotePlayerSnapshot {
    pub slot_index: usize,
    pub team: TeamId,
    pub hero_id: String,
    pub display_name: String,
    pub model_path: String,
    pub hero_scale: f32,
    pub weapon_kind: WeaponKind,
    pub pos: Vec3,
    pub facing_dir: Vec3,
    pub current_locomotion: String,
    pub traversal_anim: Option<String>,
    pub upper_anim: Option<String>,
    pub death_anim: String,
    pub is_dead: bool,
    pub hp: f32,
    pub max_hp: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub active_weapon: ActiveWeaponSlot,
    pub ammo_in_mag: u32,
    pub magazine_size: u32,
    pub grenade_charges: u32,
    pub grenade_max_charges: u32,
    pub grenade_throw_duration_secs: f32,
    pub level: u32,
    pub armor: f32,
    pub move_speed: f32,
    pub weapon_damage: f32,
    pub fire_rate_secs: f32,
    pub weapon_max_range: f32,
    pub inventory: Vec<InventoryItemState>,
    pub tp_slot: Option<InventoryItemState>,
    pub is_invisible: bool,
    pub is_reflect_armor_active: bool,
    pub is_bullet_immune: bool,
    pub is_cryo_debuffed: bool,
    pub is_null_muted: bool,
    pub is_sonar_revealed: bool,
    pub is_visible: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerWorldSnapshot {
    pub tick: u64,
    pub match_elapsed_secs: f32,
    pub current_weather: WeatherKind,
    pub dawn_total_kills: u32,
    pub dusk_total_kills: u32,
    pub soldier_pos: Vec3,
    pub soldier_facing_dir: Vec3,
    pub aim_world_dir: Vec3,
    pub aim_hit_pos: Vec3,
    pub laser_hit_pos: Vec3,
    pub aim_locked_on_enemy: bool,
    pub hovered_creep_id: Option<u64>,
    pub hovered_turret_index: Option<usize>,
    pub hovered_barracks_index: Option<usize>,
    pub hovered_ancient_index: Option<usize>,
    pub winning_team: Option<TeamId>,
    pub current_locomotion: String,
    pub upper_action: UpperBodyAction,
    pub traversal_active: bool,
    pub traversal_anim_name: String,
    pub traversal_duration: f32,
    pub swing_pulley_pos: Option<Vec3>,
    pub swing_throw_progress: f32,
    pub is_dead: bool,
    pub death_anim_name: String,
    pub in_water: bool,
    pub hero: HeroSnapshot,
    pub other_players: Vec<RemotePlayerSnapshot>,
    pub top_bar_heroes: Vec<TopBarHeroInfo>,
    pub scoreboard_heroes: Vec<ScoreboardHeroInfo>,
    pub fog_visible_mask: Vec<u8>,
    pub bullets: Vec<BulletSnapshot>,
    pub grenades: Vec<GrenadeSnapshot>,
    pub toxic_pools: Vec<ToxicPoolSnapshot>,
    pub electric_beams: Vec<ElectricBeamSnapshot>,
    pub active_teleports: Vec<ActiveTeleportSnapshot>,
    pub targets: Vec<TargetSnapshot>,
    pub creeps: Vec<CreepSnapshot>,
    pub couriers: Vec<CourierSnapshot>,
    pub turrets: Vec<TurretSnapshot>,
    pub barracks: Vec<BarracksSnapshot>,
    pub ancients: Vec<AncientSnapshot>,
    pub recent_chat: Vec<ChatMessageEntry>,
    pub recent_pings: Vec<PingEntry>,
    pub pause: PauseSnapshot,
    pub fx_events: Vec<GameplayFxEvent>,
}

// -----------------------------------------------------------------------------
// ۳. موتور فیزیک ۳بعدی اختصاصی + تشخیص تعاملی جریان آب و برکه‌ها
// -----------------------------------------------------------------------------

pub const GROUP_WORLD: u32 = 1 << 0;
pub const GROUP_SOLDIER: u32 = 1 << 1;
pub const GROUP_GRENADE: u32 = 1 << 2;
pub const GROUP_FX: u32 = 1 << 3;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionGroups {
    pub memberships: u32,
    pub filters: u32,
}

impl CollisionGroups {
    pub const fn new(memberships: u32, filters: u32) -> Self {
        Self {
            memberships,
            filters,
        }
    }

    #[inline]
    pub const fn interacts_with(self, other: Self) -> bool {
        (self.memberships & other.filters) != 0 && (other.memberships & self.filters) != 0
    }
}

pub fn world_collision_groups() -> CollisionGroups {
    CollisionGroups::new(GROUP_WORLD, GROUP_SOLDIER | GROUP_GRENADE | GROUP_FX)
}

pub fn soldier_collision_groups() -> CollisionGroups {
    CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD)
}

pub fn grenade_collision_groups() -> CollisionGroups {
    CollisionGroups::new(GROUP_GRENADE, GROUP_WORLD | GROUP_GRENADE)
}

pub fn fx_collision_groups() -> CollisionGroups {
    CollisionGroups::new(GROUP_FX, GROUP_WORLD)
}

/// محاسبه عمق اضافی کف برکه در نزدیکی آبشارها (هرچه به آبشار نزدیک‌تر شویم کف آب عمیق‌تر می‌شود)
/// فرمول استاندارد کاهش دمیج بر اساس Armor در بازی‌های MOBA (مشابه Dota 2)
#[inline]
pub fn dota2_armor_multiplier(armor: f32) -> f32 {
    1.0 - (0.06 * armor) / (1.0 + 0.06 * armor.abs())
}

/// بررسی تحلیلی و پیوسته برخورد پاره‌خط (اشعه ماوس یا مسیر تیر) با استوانه/کپسول عمودی هیرو
fn segment_hits_hero_capsule(
    seg_start: Vec3,
    seg_dir: Vec3,
    seg_len: f32,
    hero_feet_pos: Vec3,
    hero_radius: f32,
    hero_height: f32,
    bottom_forgiveness: f32,
    top_forgiveness: f32,
) -> Option<f32> {
    if seg_len <= 0.0 || hero_radius <= 0.0 {
        return None;
    }
    let y_min = hero_feet_pos.y - bottom_forgiveness;
    let y_max = hero_feet_pos.y + hero_height + top_forgiveness;

    let ox = seg_start.x - hero_feet_pos.x;
    let oz = seg_start.z - hero_feet_pos.z;
    let dx = seg_dir.x;
    let dz = seg_dir.z;

    let a = dx * dx + dz * dz;
    let b = 2.0 * (ox * dx + oz * dz);
    let c = ox * ox + oz * oz - hero_radius * hero_radius;

    let (t_cyl_enter, t_cyl_exit) = if a < 1e-6 {
        if c > 0.0 {
            return None;
        }
        (0.0_f32, seg_len)
    } else {
        let disc = b * b - 4.0 * a * c;
        if disc < 0.0 {
            return None;
        }
        let sqrt_d = disc.sqrt();
        let inv_2a = 0.5 / a;
        let t1 = (-b - sqrt_d) * inv_2a;
        let t2 = (-b + sqrt_d) * inv_2a;
        (t1.max(0.0), t2.min(seg_len))
    };

    if t_cyl_enter > t_cyl_exit {
        return None;
    }

    if seg_dir.y.abs() < 1e-6 {
        if seg_start.y >= y_min && seg_start.y <= y_max {
            return Some(t_cyl_enter);
        }
        return None;
    }

    let ty1 = (y_min - seg_start.y) / seg_dir.y;
    let ty2 = (y_max - seg_start.y) / seg_dir.y;
    let t_y_enter = ty1.min(ty2);
    let t_y_exit = ty1.max(ty2);

    let t_enter = t_cyl_enter.max(t_y_enter);
    let t_exit = t_cyl_exit.min(t_y_exit);

    if t_enter <= t_exit && t_exit >= 0.0 && t_enter <= seg_len {
        Some(t_enter.max(0.0))
    } else {
        None
    }
}

fn canyon_meander_and_half_w(canyon: &CanyonConfig, x: f32, z: f32) -> (f32, f32, f32, Vec2, Vec2) {
    let start = Vec2::new(canyon.start_corner.0, canyon.start_corner.1);
    let end = Vec2::new(canyon.end_corner.0, canyon.end_corner.1);
    let mid = (start + end) * 0.5;
    let span = end - start;
    let half_len = (span.length() * 0.5).max(1.0);
    let dir = span.normalize_or_zero();
    let perp = Vec2::new(-dir.y, dir.x);

    let p = Vec2::new(x, z) - mid;
    let along = p.dot(dir);
    let s = (along / half_len).clamp(-1.2, 1.2);

    let amp = canyon.meander_amplitude;
    let meander = amp * (s * 3.8).sin()
        + (amp * 0.48) * (s * 7.6 + 0.7).cos()
        + (amp * 0.20) * (s * 13.5 - 0.4).sin();

    let rock_jitter = 2.4 * (x * 0.15 + z * 0.11).sin()
        + 1.3 * (x * 0.29 - z * 0.23).cos();
    let signed_perp = p.dot(perp) - meander;
    let dist = signed_perp.abs() + rock_jitter * 0.55;

    let base_half_w = canyon.base_half_width
        + 4.8 * (s * 5.2 + 0.4).sin()
        + 2.6 * (s * 11.0 - 0.8).cos();
    let center_basin_boost = canyon.center_basin_boost * (-s * s * 13.5).exp();
    let half_w = (base_half_w + center_basin_boost).max(9.5);
    (dist, half_w, signed_perp, dir, perp)
}

/// فاصله افقی نقطه از دیواره ۹۰ درجه دره (>0 یعنی داخل دره، <=0 یعنی بیرون روی فلات)
pub fn canyon_edge_distance(canyon: &CanyonConfig, x: f32, z: f32) -> f32 {
    if !canyon.enabled {
        return -100.0;
    }
    let (dist, half_w, _, _, _) = canyon_meander_and_half_w(canyon, x, z);
    half_w - dist
}

/// هم‌ترازی دقیق و ریاضی کوه‌های دو طرف دره با دیواره واقعی دره
pub fn align_canyon_mountains_to_rim(
    canyon: &CanyonConfig,
    align: &CanyonMountainAlignConfig,
    mountains: &mut [MountainDef],
) {
    if !canyon.enabled || !align.enabled {
        return;
    }

    let eps = align.gradient_sample_eps;
    let inv_2eps = 0.5 / eps;

    for mtn in mountains.iter_mut() {
        if !mtn.id.starts_with(&align.id_prefix) {
            continue;
        }

        let mut x = mtn.model_transform.location.0;
        let mut z = mtn.model_transform.location.2;

        for _ in 0..align.max_snap_iterations {
            let cur_edge = canyon_edge_distance(canyon, x, z);
            let err = align.target_edge_offset - cur_edge;
            if err.abs() < 1e-3 {
                break;
            }
            let gx = (canyon_edge_distance(canyon, x + eps, z)
                - canyon_edge_distance(canyon, x - eps, z))
                * inv_2eps;
            let gz = (canyon_edge_distance(canyon, x, z + eps)
                - canyon_edge_distance(canyon, x, z - eps))
                * inv_2eps;
            let grad_sq = gx * gx + gz * gz;
            if grad_sq <= 1e-8 {
                break;
            }
            let step = (err / grad_sq).clamp(-3.0, 3.0);
            x += gx * step;
            z += gz * step;
        }

        mtn.model_transform.location.0 = x;
        mtn.model_transform.location.2 = z;

        if align.align_yaw_to_canyon_normal {
            let gx = canyon_edge_distance(canyon, x + eps, z)
                - canyon_edge_distance(canyon, x - eps, z);
            let gz = canyon_edge_distance(canyon, x, z + eps)
                - canyon_edge_distance(canyon, x, z - eps);
            if gx * gx + gz * gz > 1e-8 {
                let mut yaw = gx.atan2(gz).to_degrees() + align.yaw_offset_deg;
                while yaw > 180.0 {
                    yaw -= 360.0;
                }
                while yaw <= -180.0 {
                    yaw += 360.0;
                }
                mtn.model_transform.rotation.1 = yaw;
            }
        }
    }
}

/// فاصله علامت‌دار از خط مرکزی رودخانه خمیده:
/// مقدار >= 0 یعنی سمت چپ رودخانه (Dawn / چمنزار) و مقدار < 0 یعنی سمت راست رودخانه (Dusk / پاییزی)
pub fn canyon_river_side_signed(canyon: &CanyonConfig, x: f32, z: f32) -> f32 {
    if !canyon.enabled {
        return -x;
    }
    let (_, _, signed_perp, _, _) = canyon_meander_and_half_w(canyon, x, z);
    signed_perp
}

/// آیا مختصات (x, z) در سمت چپ رودخانه قرار دارد؟
#[inline]
pub fn is_left_of_river(canyon: &CanyonConfig, x: f32, z: f32) -> bool {
    canyon_river_side_signed(canyon, x, z) >= 0.0
}

/// ارتفاع دقیق دره با دیواره‌های کاملاً عمودی ۹۰ درجه (بدون شیب بالا رفتن)
pub fn canyon_sample_height(canyon: &CanyonConfig, x: f32, z: f32) -> f32 {
    if !canyon.enabled {
        return 0.0;
    }
    let (dist, half_w, _, _, _) = canyon_meander_and_half_w(canyon, x, z);
    if dist < half_w {
        let bed_ripple = 0.25 * (x * 0.22).sin() * (z * 0.22).cos();
        canyon.floor_depth + bed_ripple
    } else {
        0.0
    }
}

/// ارتفاع هدف دوربین در حالت Free-Look: روی پل مرکزی و تمام لاین‌ها (بالا، وسط، پایین) بالا می‌رود و فقط در بستر باز دره پایین می‌آید
#[inline]
pub fn is_on_bridge_or_lane(config: &GameConfig, x: f32, z: f32) -> bool {
    let cb = &config.map.center_bridge;
    let cb_tf = cb.model_transform.to_transform();
    let cb_rad = cb.outer_radius * cb_tf.scale.x.max(cb_tf.scale.z);
    let dx = x - cb_tf.translation.x;
    let dz = z - cb_tf.translation.z;
    if dx * dx + dz * dz <= cb_rad * cb_rad {
        return true;
    }
    let p_xz = Vec2::new(x, z);
    for lane in &config.map.lanes {
        let half_w = lane.width * 0.5;
        let half_w_sq = half_w * half_w;
        for pair in lane.waypoints.windows(2) {
            let a_xz = Vec2::new(pair[0].0, pair[0].2);
            let b_xz = Vec2::new(pair[1].0, pair[1].2);
            let ab = b_xz - a_xz;
            let len_sq = ab.length_squared();
            let t = if len_sq > 1e-5 {
                ((p_xz - a_xz).dot(ab) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let closest = a_xz + ab * t;
            if (p_xz - closest).length_squared() <= half_w_sq {
                return true;
            }
        }
    }
    false
}

#[inline]
pub fn clamp_to_bridge_or_lane(config: &GameConfig, x: f32, z: f32) -> (f32, f32) {
    let p_xz = Vec2::new(x, z);
    let mut best_pt = p_xz;
    let mut best_dist_sq = f32::MAX;

    let cb = &config.map.center_bridge;
    let cb_tf = cb.model_transform.to_transform();
    let cb_rad = (cb.outer_radius * cb_tf.scale.x.max(cb_tf.scale.z) - 0.08).max(0.5);
    let cb_center = Vec2::new(cb_tf.translation.x, cb_tf.translation.z);
    let to_cb = p_xz - cb_center;
    let d_cb = to_cb.length();
    if d_cb > 1e-5 {
        let clamped = cb_center + (to_cb / d_cb) * d_cb.min(cb_rad);
        let dsq = p_xz.distance_squared(clamped);
        if dsq < best_dist_sq {
            best_dist_sq = dsq;
            best_pt = clamped;
        }
    }

    for lane in &config.map.lanes {
        let half_w = (lane.width * 0.5 - 0.08).max(0.5);
        for pair in lane.waypoints.windows(2) {
            let a_xz = Vec2::new(pair[0].0, pair[0].2);
            let b_xz = Vec2::new(pair[1].0, pair[1].2);
            let ab = b_xz - a_xz;
            let len_sq = ab.length_squared();
            let t = if len_sq > 1e-5 {
                ((p_xz - a_xz).dot(ab) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let closest = a_xz + ab * t;
            let diff = p_xz - closest;
            let d = diff.length();
            let clamped = if d > half_w && d > 1e-5 {
                closest + (diff / d) * half_w
            } else {
                p_xz
            };
            let dsq = p_xz.distance_squared(clamped);
            if dsq < best_dist_sq {
                best_dist_sq = dsq;
                best_pt = clamped;
            }
        }
    }
    (best_pt.x, best_pt.y)
}

pub fn sample_camera_focus_y(config: &GameConfig, x: f32, z: f32) -> f32 {
    let cb = &config.map.center_bridge;
    let cb_tf = cb.model_transform.to_transform();
    let cb_rad = cb.outer_radius * cb_tf.scale.x.max(cb_tf.scale.z);
    let dx = x - cb_tf.translation.x;
    let dz = z - cb_tf.translation.z;
    if dx * dx + dz * dz <= cb_rad * cb_rad {
        return cb_tf.translation.y + cb.deck_thickness * 0.5 * cb_tf.scale.y;
    }

    let p_xz = Vec2::new(x, z);
    for lane in &config.map.lanes {
        let half_w = lane.width * 0.5;
        let half_w_sq = half_w * half_w;
        for pair in lane.waypoints.windows(2) {
            let a = Vec3::new(pair[0].0, pair[0].1, pair[0].2);
            let b = Vec3::new(pair[1].0, pair[1].1, pair[1].2);
            let a_xz = Vec2::new(a.x, a.z);
            let b_xz = Vec2::new(b.x, b.z);
            let ab = b_xz - a_xz;
            let len_sq = ab.length_squared();
            let t = if len_sq > 1e-5 {
                ((p_xz - a_xz).dot(ab) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let closest = a_xz + ab * t;
            if (p_xz - closest).length_squared() <= half_w_sq {
                return a.y + (b.y - a.y) * t + lane.thickness * 0.5;
            }
        }
    }

    0.0
}

pub fn sample_waterfall_depth_offset(config: &GameConfig, wx: f32, wz: f32) -> f32 {
    let mut max_extra_depth = 0.0_f32;
    let basin_radius = 16.0_f32;
    for wf in &config.map.waterfalls {
        let loc = wf.model_transform.location;
        let dx = wx - loc.0;
        let dz = wz - loc.2;
        let dist = (dx * dx + dz * dz).sqrt();
        if dist < basin_radius {
            let t = 1.0 - (dist / basin_radius);
            let smooth = t * t * (3.0 - 2.0 * t);
            max_extra_depth = max_extra_depth.max(smooth * 1.15);
        }
    }
    max_extra_depth
}

#[inline]
pub fn is_point_in_roshan_pit_circle(pit: &RoshanPitConfig, wx: f32, wz: f32, radius: f32) -> bool {
    if !pit.enabled {
        return false;
    }
    let loc = pit.model_transform.location;
    let dx = wx - loc.0;
    let dz = wz - loc.2;
    dx * dx + dz * dz <= radius * radius
}

#[inline]
pub fn is_point_inside_roshan_pit(pit: &RoshanPitConfig, wx: f32, wz: f32) -> bool {
    is_point_in_roshan_pit_circle(pit, wx, wz, pit.inner_radius)
}

/// بررسی قرار گرفتن یک نقطه ۳بعدی درون برکه‌ها و رودخانه‌های نقشه و برگرداندن ارتفاع سطح آب و بردار جریان
pub fn sample_water_surface_at(config: &GameConfig, world_pos: Vec3) -> Option<(f32, Vec2)> {
    // کف غار روشان سطح زمین خشک محسوب می‌شود و آب نیست
    if is_point_in_roshan_pit_circle(
        &config.map.roshan_pit,
        world_pos.x,
        world_pos.z,
        config.map.roshan_pit.outer_radius,
    ) {
        return None;
    }
    for seg in &config.map.river_segments {
        let seg_tf = seg.model_transform.to_transform();
        let seg_pos = seg_tf.translation;
        let half = Vec3::new(
            seg.size.0 * 0.5 * seg_tf.scale.x,
            seg.size.1 * 0.5 * seg_tf.scale.y,
            seg.size.2 * 0.5 * seg_tf.scale.z,
        );
        let water_top_y = seg_pos.y + half.y;

        if world_pos.y < water_top_y - 2.40 || world_pos.y > water_top_y + 0.45 {
            continue;
        }

        let inv_rot = seg_tf.rotation.inverse();
        let local = inv_rot * (world_pos - seg_pos);
        // استفاده از شکل بیضوی/نرم در صفحه افقی برای طبیعی‌تر شدن مرز برکه‌ها
        let nx = local.x / half.x.max(0.1);
        let nz = local.z / half.z.max(0.1);
        if nx * nx + nz * nz <= 1.00 {
            let flow = Vec2::new(seg.flow_dir.0, seg.flow_dir.1).normalize_or_zero();
            return Some((water_top_y, flow));
        }
    }
    let ch = canyon_sample_height(&config.map.canyon, world_pos.x, world_pos.z);
    let wy = config.map.canyon.water_y;
    if config.map.canyon.enabled && ch < wy - 0.10 && world_pos.y >= ch - 0.5 && world_pos.y <= wy + 2.5 {
        let f = Vec2::new(config.map.canyon.flow_dir.0, config.map.canyon.flow_dir.1).normalize_or_zero();
        return Some((wy, f));
    }
    None
}

/// بررسی اینکه آیا نقطه (پای هیرو) در عمق شنای آب قرار دارد یا خیر
#[inline]
pub fn is_pos_in_deep_water(config: &GameConfig, pos: Vec3) -> bool {
    if let Some((water_y, _)) = sample_water_surface_at(config, pos) {
        (water_y - pos.y) >= config.physics.deep_water_swim_depth
    } else {
        false
    }
}

/// قانون مخفی‌شدن هیرو هنگام شنا در آب عمیق (Server-Authoritative):
/// هیرویی که داخل آب شنا می‌کند برای تیم دشمن غیرقابل‌مشاهده است و مختصاتش ارسال نمی‌شود،
/// مگر اینکه حداقل یک هیروی زنده از تیم دشمن هم داخل آب شنا کند و هر دو در رنج میدان دید همدیگر باشند.
pub fn is_swimming_hero_revealed_to_team(
    players: &[PlayerSlotState],
    target_slot: usize,
    viewer_team: TeamId,
    config: &GameConfig,
    match_elapsed_secs: f32,
    current_weather: WeatherKind,
) -> bool {
    let Some(target) = players.iter().find(|p| p.slot_index == target_slot) else {
        return false;
    };
    if target.hero_team == viewer_team {
        return true;
    }
    if target.sonar_revealed_remaining > 0.0 {
        return true;
    }
    if !is_pos_in_deep_water(config, target.soldier_pos) {
        return true;
    }
    let target_hero = config.active_hero(target.hero_index);
    let target_vis = config.hero_effective_vision(target_hero, match_elapsed_secs, current_weather);

    players.iter().any(|obs| {
        obs.hero_team == viewer_team
            && !obs.is_dead
            && is_pos_in_deep_water(config, obs.soldier_pos)
            && {
                let obs_hero = config.active_hero(obs.hero_index);
                let obs_vis =
                    config.hero_effective_vision(obs_hero, match_elapsed_secs, current_weather);
                let mutual_range = obs_vis.min(target_vis);
                obs.soldier_pos.distance(target.soldier_pos) <= mutual_range
            }
    })
}


/// تقاطع اشعه دوربین با سطح آب برکه‌ها و رودخانه دره (برای دقت هدف‌گیری سه‌بعدی روی سطح آب)
pub fn raycast_water_surface(
    config: &GameConfig,
    origin: Vec3,
    ray_dir: Vec3,
    max_dist: f32,
) -> Option<(f32, Vec3)> {
    if ray_dir.y.abs() <= 1e-5 {
        return None;
    }
    let mut best: Option<(f32, Vec3)> = None;
    for seg in &config.map.river_segments {
        let seg_tf = seg.model_transform.to_transform();
        let seg_pos = seg_tf.translation;
        let half = Vec3::new(
            seg.size.0 * 0.5 * seg_tf.scale.x,
            seg.size.1 * 0.5 * seg_tf.scale.y,
            seg.size.2 * 0.5 * seg_tf.scale.z,
        );
        let water_top_y = seg_pos.y + half.y;
        let t = (water_top_y - origin.y) / ray_dir.y;
        if t <= 0.0 || t > max_dist {
            continue;
        }
        let hit_pt = origin + ray_dir * t;
        if is_point_in_roshan_pit_circle(
            &config.map.roshan_pit,
            hit_pt.x,
            hit_pt.z,
            config.map.roshan_pit.outer_radius,
        ) {
            continue;
        }
        let inv_rot = seg_tf.rotation.inverse();
        let local = inv_rot * (hit_pt - seg_pos);
        let nx = local.x / half.x.max(0.1);
        let nz = local.z / half.z.max(0.1);
        if nx * nx + nz * nz <= 1.00 && best.map_or(true, |(bt, _)| t < bt) {
            best = Some((t, hit_pt));
        }
    }
    if config.map.canyon.enabled {
        let wy = config.map.canyon.water_y;
        let t = (wy - origin.y) / ray_dir.y;
        if t > 0.0 && t <= max_dist {
            let hit_pt = origin + ray_dir * t;
            if !is_point_in_roshan_pit_circle(
                &config.map.roshan_pit,
                hit_pt.x,
                hit_pt.z,
                config.map.roshan_pit.outer_radius,
            ) && canyon_sample_height(&config.map.canyon, hit_pt.x, hit_pt.z) < wy - 0.10
                && best.map_or(true, |(bt, _)| t < bt)
            {
                best = Some((t, hit_pt));
            }
        }
    }
    best
}

/// کامپوننت غیرفعال‌سازی کولایدر (برای اهداف منهدم‌شده)
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ColliderDisabled;

/// سرعت خطی و زاویه‌ای اجسام دینامیک
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

/// نوع جسم فیزیکی در موتور اختصاصی
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomRigidBody {
    Fixed,
    KinematicPositionBased,
    Dynamic,
}

/// اشکال هندسی برخورد در موتور اختصاصی
#[derive(Component, Clone, Copy, Debug)]
pub enum CustomCollider {
    Cuboid { half_extents: Vec3 },
    Cylinder { half_height: f32, radius: f32 },
    Ball { radius: f32 },
    CapsuleY { half_segment: f32, radius: f32 },
    /// منشور مثلثی توپر (گوه / پله توپر با سطح زیرین پر)
    Wedge {
        half_width: f32,
        half_height: f32,
        half_length: f32,
    },
}

impl CustomCollider {
    #[inline]
    pub fn cuboid(hx: f32, hy: f32, hz: f32) -> Self {
        Self::Cuboid {
            half_extents: Vec3::new(hx, hy, hz),
        }
    }

    #[inline]
    pub fn cylinder(half_height: f32, radius: f32) -> Self {
        Self::Cylinder {
            half_height,
            radius,
        }
    }

    #[inline]
    pub fn ball(radius: f32) -> Self {
        Self::Ball { radius }
    }

    #[inline]
    pub fn capsule_y(half_segment: f32, radius: f32) -> Self {
        Self::CapsuleY {
            half_segment,
            radius,
        }
    }

    #[inline]
    pub fn wedge(half_width: f32, half_height: f32, half_length: f32) -> Self {
        Self::Wedge {
            half_width,
            half_height,
            half_length,
        }
    }

    #[inline]
    pub fn bounding_sphere_radius(&self) -> f32 {
        match *self {
            Self::Cuboid { half_extents } => half_extents.length(),
            Self::Cylinder {
                half_height,
                radius,
            } => (half_height * half_height + radius * radius).sqrt(),
            Self::Ball { radius } => radius,
            Self::CapsuleY {
                half_segment,
                radius,
            } => half_segment + radius,
            Self::Wedge {
                half_width,
                half_height,
                half_length,
            } => Vec3::new(half_width, half_height, half_length).length(),
        }
    }

    #[inline]
    pub fn effective_ball_radius(&self) -> f32 {
        match *self {
            Self::Ball { radius } => radius,
            Self::Cylinder {
                half_height,
                radius,
            } => radius.max(half_height),
            Self::Cuboid { half_extents } => {
                half_extents.x.max(half_extents.y).max(half_extents.z)
            }
            Self::CapsuleY { radius, .. } => radius,
            Self::Wedge {
                half_width,
                half_height,
                half_length,
            } => half_width.max(half_height).max(half_length),
        }
    }
}

/// خواص فیزیکی سطح برای کولایدرهای ثابت نقشه
#[derive(Component, Clone, Copy, Debug)]
pub struct SurfaceMaterial {
    pub friction: f32,
    pub restitution: f32,
}

/// خواص فیزیکی اجسام دینامیک (نارنجک، پوکه و ذرات)
#[derive(Component, Clone, Copy, Debug)]
pub struct DynamicBodyProps {
    pub restitution: f32,
    pub friction: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub ccd_enabled: bool,
}

/// کنترلر اختصاصی حرکت کاراکتر
#[derive(Component, Clone, Debug, Default)]
pub struct CustomCharacterController {
    pub translation: Option<Vec3>,
    pub grounded: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub toi: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub struct SphereContact {
    pub normal: Vec3,
    pub penetration: f32,
    pub contact_point: Vec3,
}

#[derive(Clone, Debug)]
pub struct StaticColliderEntry {
    pub entity: Entity,
    pub translation: Vec3,
    pub rotation: Quat,
    pub inv_rotation: Quat,
    pub collider: CustomCollider,
    pub bounding_radius: f32,
    pub material: SurfaceMaterial,
    pub groups: CollisionGroups,
    pub disabled: bool,
    pub is_non_walkable_wall: bool,
}

/// دنیای فیزیک اختصاصی که لیست کولایدرهای ثابت فعال را برای پرتوافکنی و برخوردها نگه می‌دارد
#[derive(Resource, Default, Clone, Debug)]
pub struct CustomPhysicsWorld {
    pub static_colliders: Vec<StaticColliderEntry>,
    pub canyon: Option<CanyonConfig>,
}

impl CustomPhysicsWorld {
    /// پرتوافکنی ۳بعدی دقیق به سمت جعبه‌های چرخیده (OBB) و استوانه‌ها
    pub fn cast_ray<F>(
        &self,
        origin: Vec3,
        dir: Vec3,
        max_toi: f32,
        query_groups: CollisionGroups,
        predicate: F,
    ) -> Option<(Entity, RayHit)>
    where
        F: Fn(Entity) -> bool,
    {
        let dir_len = dir.length();
        if dir_len <= f32::EPSILON || max_toi <= 0.0 {
            return None;
        }
        let unit_dir = dir / dir_len;
        let mut closest_toi = max_toi;
        let mut best_hit: Option<(Entity, RayHit)> = None;

        for entry in &self.static_colliders {
            if entry.disabled || !query_groups.interacts_with(entry.groups) {
                continue;
            }
            let to_center = entry.translation - origin;
            let proj = to_center.dot(unit_dir);
            if proj < -entry.bounding_radius || proj > closest_toi + entry.bounding_radius {
                continue;
            }
            let perp_dist_sq = (to_center.length_squared() - proj * proj).max(0.0);
            if perp_dist_sq > (entry.bounding_radius + 0.05).powi(2) {
                continue;
            }

            if !predicate(entry.entity) {
                continue;
            }

            let hit_opt = ray_intersect_single_entry(origin, unit_dir, closest_toi, entry);

            if let Some(hit) = hit_opt {
                if hit.toi < closest_toi {
                    // داخل دره عمیق، صفحه صاف فلات و زمین زیرین وجود ندارد (حفر شده است)
                    if entry.bounding_radius > 200.0 {
                        if let Some(ref canyon) = self.canyon {
                            if canyon_sample_height(canyon, hit.point.x, hit.point.z) < -0.15 {
                                continue;
                            }
                        }
                    }
                    closest_toi = hit.toi;
                    best_hit = Some((entry.entity, hit));
                }
            }
        }

        if (query_groups.filters & GROUP_WORLD) != 0 {
            if let Some(ref canyon) = self.canyon {
                if let Some(c_hit) = ray_intersect_canyon(canyon, origin, unit_dir, closest_toi) {
                    if c_hit.toi < closest_toi {
                        best_hit = Some((Entity::PLACEHOLDER, c_hit));
                    }
                }
            }
        }

        best_hit
    }

    /// بررسی حرکت افقی کپسول (Sweep) برای غلتیدن، شیرجه و سر خوردن تا برخورد به موانع
    pub fn sweep_capsule_horizontal<F>(
        &self,
        start_center: Vec3,
        half_segment: f32,
        radius: f32,
        travel_dir: Vec3,
        max_distance: f32,
        padding: f32,
        query_groups: CollisionGroups,
        predicate: F,
    ) -> f32
    where
        F: Fn(Entity) -> bool,
    {
        let flat_dir = Vec3::new(travel_dir.x, 0.0, travel_dir.z).normalize_or_zero();
        if flat_dir == Vec3::ZERO || max_distance <= 0.0 {
            return 0.0;
        }

        let expanded_r = radius + padding;
        let step_size = (radius * 0.35).max(0.08);
        let steps = ((max_distance / step_size).ceil() as usize).max(1);
        let dt_dist = max_distance / (steps as f32);

        let sample_offsets_y = [-half_segment * 0.75, 0.0, half_segment * 0.75];

        for step_idx in 1..=steps {
            let dist = (step_idx as f32) * dt_dist;
            let candidate_center = start_center + flat_dir * dist;

            for entry in &self.static_colliders {
                if entry.disabled || !query_groups.interacts_with(entry.groups) {
                    continue;
                }
                let center_dist_sq = candidate_center.distance_squared(entry.translation);
                let max_r = entry.bounding_radius + half_segment + expanded_r;
                if center_dist_sq > max_r * max_r {
                    continue;
                }
                if !predicate(entry.entity) {
                    continue;
                }

                for &oy in &sample_offsets_y {
                    let sphere_center = candidate_center + Vec3::Y * oy;
                    if let Some(contact) =
                        sphere_vs_static_collider(sphere_center, expanded_r, entry)
                    {
                        if contact.normal.y < 0.65 {
                            return ((step_idx - 1) as f32 * dt_dist - contact.penetration).max(0.0);
                        }
                    }
                }
            }
        }

        max_distance
    }

    /// حل کامل حرکت ۳بعدی کپسول سرباز شامل شیب رمپ‌ها، بالا رفتن خودکار از پله (Autostep)،
    /// چسبیدن به زمین (Snap-to-Ground) و سر خوردن نرم کنار دیوارها بدون لرزش
    pub fn move_character_capsule(
        &self,
        current_center: Vec3,
        desired_delta: Vec3,
        config: &GameConfig,
    ) -> (Vec3, bool) {
        if self.static_colliders.is_empty() {
            return (
                Vec3::new(
                    current_center.x + desired_delta.x,
                    current_center.y,
                    current_center.z + desired_delta.z,
                ),
                true,
            );
        }

        let half_seg = config.physics.soldier_capsule_half_segment;
        let radius = config.soldier.player_radius;
        let skin = config.physics.character_skin_offset;
        let eff_radius = radius + skin;
        let center_y_offset = config.physics.soldier_capsule_center_y;
        let max_slope_cos = config
            .physics
            .character_max_slope_climb_deg
            .to_radians()
            .cos();
        let autostep_max_h = config.physics.character_autostep_max_height;
        let snap_dist = config.physics.character_snap_to_ground;
        let nudge = config.physics.character_normal_nudge_factor;

        let mut center = current_center;
        let horiz_delta = Vec3::new(desired_delta.x, 0.0, desired_delta.z);
        let horiz_len = horiz_delta.length();
        let num_horiz_steps = if horiz_len > radius * 0.45 {
            ((horiz_len / (radius * 0.45)).ceil() as usize).clamp(1, 6)
        } else {
            1
        };
        let step_xz = horiz_delta / (num_horiz_steps as f32);

        let soldier_groups = soldier_collision_groups();
        let sample_offsets_y = [-half_seg, 0.0, half_seg];

        // ۱. جابجایی افقی و حل برخورد با دیوارها + Autostep روی لبه‌ها
        for _ in 0..num_horiz_steps {
            center.x += step_xz.x;
            center.z += step_xz.z;

            for _iter in 0..3 {
                let mut any_wall_push = false;
                let feet_y = center.y - center_y_offset;

                for entry in &self.static_colliders {
                    if entry.disabled || !soldier_groups.interacts_with(entry.groups) {
                        continue;
                    }
                    let max_r = entry.bounding_radius + half_seg + eff_radius;
                    if center.distance_squared(entry.translation) > max_r * max_r {
                        continue;
                    }

                    let mut deepest_wall_contact: Option<SphereContact> = None;
                    for &oy in &sample_offsets_y {
                        let sphere_pos = center + Vec3::Y * oy;
                        if let Some(contact) =
                            sphere_vs_static_collider(sphere_pos, eff_radius, entry)
                        {
                            if contact.normal.y >= max_slope_cos {
                                continue;
                            }
                            if deepest_wall_contact
                                .as_ref()
                                .map_or(true, |prev| contact.penetration > prev.penetration)
                            {
                                deepest_wall_contact = Some(contact);
                            }
                        }
                    }

                    if let Some(wall_hit) = deepest_wall_contact {
                        let horiz_normal = Vec3::new(wall_hit.normal.x, 0.0, wall_hit.normal.z)
                            .normalize_or_zero();
                        let mut stepped_up = false;

                        // اگر برخورد در پایین پا و زیر سقف Autostep باشد، هم زیر مرکز و هم کمی جلوتر روی لبه مانع تست می‌کنیم
                        if !entry.is_non_walkable_wall && wall_hit.contact_point.y <= feet_y + autostep_max_h + 0.08 {
                            let probe_points = [
                                Vec2::new(center.x, center.z),
                                Vec2::new(
                                    wall_hit.contact_point.x - horiz_normal.x * 0.06,
                                    wall_hit.contact_point.z - horiz_normal.z * 0.06,
                                ),
                            ];
                            for pt in &probe_points {
                                let step_origin = Vec3::new(
                                    pt.x,
                                    feet_y + autostep_max_h + 0.15,
                                    pt.y,
                                );
                                if let Some(top_hit) = ray_intersect_single_entry(
                                    step_origin,
                                    Vec3::NEG_Y,
                                    autostep_max_h + 0.22,
                                    entry,
                                ) {
                                    if top_hit.normal.y >= max_slope_cos
                                        && top_hit.point.y >= feet_y - 0.05
                                        && top_hit.point.y <= feet_y + autostep_max_h
                                    {
                                        center.y = top_hit.point.y + center_y_offset;
                                        stepped_up = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if !stepped_up {
                            let push_dir = if horiz_normal != Vec3::ZERO {
                                horiz_normal
                            } else if entry.is_non_walkable_wall && config.map.canyon.enabled {
                                let eps = 0.8_f32;
                                let gx = canyon_edge_distance(&config.map.canyon, center.x + eps, center.z)
                                    - canyon_edge_distance(&config.map.canyon, center.x - eps, center.z);
                                let gz = canyon_edge_distance(&config.map.canyon, center.x, center.z + eps)
                                    - canyon_edge_distance(&config.map.canyon, center.x, center.z - eps);
                                Vec3::new(gx, 0.0, gz).normalize_or_zero()
                            } else {
                                Vec3::ZERO
                            };
                            if push_dir != Vec3::ZERO {
                                let push_amt = if entry.is_non_walkable_wall && horiz_normal == Vec3::ZERO {
                                    0.18_f32
                                } else {
                                    wall_hit.penetration + nudge
                                };
                                center += push_dir * push_amt;
                                any_wall_push = true;
                            }
                        }
                    }
                }

                if !any_wall_push {
                    break;
                }
            }
        }

        // قفل قطعی لبه دره و لبه‌های پل/لاین روی دره در فاز افقی (پوشش کامل پیچ‌های تند شمال‌غربی و سراسر دره)
        if config.map.canyon.enabled && (current_center.y - center_y_offset) >= -0.35 {
            let was_on_bridge = is_on_bridge_or_lane(config, current_center.x, current_center.z);
            let now_on_bridge = is_on_bridge_or_lane(config, center.x, center.z);
            if !now_on_bridge {
                let start_edge_d = canyon_edge_distance(&config.map.canyon, current_center.x, current_center.z);
                let edge_d = canyon_edge_distance(&config.map.canyon, center.x, center.z);
                if edge_d > -radius {
                    if was_on_bridge {
                        // اگر بازیکن روی پل یا لاینِ عبوری از دره بوده، روی لبه همان پل/لاین نگه داشته می‌شود
                        let (cx, cz) = clamp_to_bridge_or_lane(config, center.x, center.z);
                        center.x = cx;
                        center.z = cz;
                    } else if start_edge_d <= 0.35 || edge_d < 4.5 {
                        // بیرون راندن تکرارشونده از لبه دره با گام دقیق (رفع سقوط در پیچ‌های تند شمال‌غربی)
                        let eps = 0.35_f32;
                        for _ in 0..4 {
                            let cur_ed = canyon_edge_distance(&config.map.canyon, center.x, center.z);
                            if cur_ed <= -radius {
                                break;
                            }
                            let gx = canyon_edge_distance(&config.map.canyon, center.x + eps, center.z)
                                - canyon_edge_distance(&config.map.canyon, center.x - eps, center.z);
                            let gz = canyon_edge_distance(&config.map.canyon, center.x, center.z + eps)
                                - canyon_edge_distance(&config.map.canyon, center.x, center.z - eps);
                            let outward_dir = -Vec3::new(gx, 0.0, gz).normalize_or_zero();
                            if outward_dir == Vec3::ZERO {
                                break;
                            }
                            let push = (cur_ed + radius + 0.02).clamp(0.02, 2.5);
                            center += outward_dir * push;
                        }
                        // تضمین نهایی: اگر به دلیل پیچ تند لبه همچنان داخل دره ماندیم، به موقعیت امن قبلی روی لبه برمی‌گردیم
                        if canyon_edge_distance(&config.map.canyon, center.x, center.z) > -radius
                            && start_edge_d <= -radius + 0.15
                        {
                            center.x = current_center.x;
                            center.z = current_center.z;
                        }
                    }
                }
            }
        }

        // ۲. یافتن ارتفاع دقیق سطح زمین/رمپ/حوضچه زیر پای کاراکتر (۵ پرتو در مقطع کپسول)
        let feet_y = center.y - center_y_offset;
        let probe_r = radius * 0.62;
        let footprint_offsets = [
            Vec2::ZERO,
            Vec2::new(probe_r, 0.0),
            Vec2::new(-probe_r, 0.0),
            Vec2::new(0.0, probe_r),
            Vec2::new(0.0, -probe_r),
        ];

        let ray_start_offset = autostep_max_h + 0.35;
        let max_down_dist = 100.0_f32;
        let mut highest_walkable_y: Option<f32> = None;

        let canyon_h_center = canyon_sample_height(&config.map.canyon, center.x, center.z);
        let inside_canyon_carve = canyon_h_center < -0.15;

        for offset in &footprint_offsets {
            let ray_origin = Vec3::new(
                center.x + offset.x,
                feet_y + ray_start_offset,
                center.z + offset.y,
            );
            for entry in &self.static_colliders {
                if entry.disabled || entry.is_non_walkable_wall || !soldier_groups.interacts_with(entry.groups) {
                    continue;
                }
                if inside_canyon_carve && entry.bounding_radius > 200.0 {
                    continue;
                }
                let dx = ray_origin.x - entry.translation.x;
                let dz = ray_origin.z - entry.translation.z;
                if dx * dx + dz * dz > (entry.bounding_radius + 0.2).powi(2) {
                    continue;
                }

                if let Some(hit) =
                    ray_intersect_single_entry(ray_origin, Vec3::NEG_Y, max_down_dist, entry)
                {
                    if hit.normal.y >= max_slope_cos && hit.point.y <= feet_y + autostep_max_h + 0.06
                    {
                        highest_walkable_y = Some(
                            highest_walkable_y.map_or(hit.point.y, |prev| prev.max(hit.point.y)),
                        );
                    }
                }
            }
        }

        // ۳. اعمال گرانش، بستر دره عمیق و شناوری روی سطح آب دره
        let mut adjusted_walkable_y = highest_walkable_y;
        if inside_canyon_carve {
            let swim_floor = config.map.canyon.water_y - 1.15;
            let canyon_walk_y = if canyon_h_center < swim_floor {
                swim_floor
            } else {
                canyon_h_center
            };
            adjusted_walkable_y = Some(adjusted_walkable_y.map_or(canyon_walk_y, |y| y.max(canyon_walk_y)));
        } else if canyon_h_center > 0.05 {
            if canyon_h_center <= feet_y + autostep_max_h + 0.12 {
                adjusted_walkable_y = Some(adjusted_walkable_y.map_or(canyon_h_center, |y| y.max(canyon_h_center)));
            }
        }
        if let Some(gy) = adjusted_walkable_y {
            if !inside_canyon_carve {
                let probe_pos = Vec3::new(center.x, gy, center.z);
                if sample_water_surface_at(config, probe_pos).is_some() {
                    let extra_depth = sample_waterfall_depth_offset(config, center.x, center.z);
                    adjusted_walkable_y = Some((gy - extra_depth).max(-1.6));
                }
            }
        }

        let mut grounded = false;
        let new_feet_y = if let Some(ground_y) = adjusted_walkable_y {
            let free_fall_feet_y = feet_y + desired_delta.y;
            if free_fall_feet_y <= ground_y + snap_dist {
                grounded = true;
                ground_y
            } else {
                free_fall_feet_y
            }
        } else {
            (feet_y + desired_delta.y).max(config.map.canyon.floor_depth - 5.0)
        };

        center.y = new_feet_y + center_y_offset;

        // ۴. اصلاح نهایی نفوذ افقی و عمودی در ارتفاع جدید و برخورد به دیواره‌های صخره‌ای دره
        // قفل دیواره ۹۰ درجه دره: وقتی بازیکن داخل دره است نمی‌تواند از دیواره بالا برود (جز با نردبان)
        if config.map.canyon.enabled && new_feet_y < -0.35 {
            let edge_d = canyon_edge_distance(&config.map.canyon, center.x, center.z);
            if edge_d < radius + 0.15 {
                let eps = 0.8_f32;
                let gx = canyon_edge_distance(&config.map.canyon, center.x + eps, center.z)
                    - canyon_edge_distance(&config.map.canyon, center.x - eps, center.z);
                let gz = canyon_edge_distance(&config.map.canyon, center.x, center.z + eps)
                    - canyon_edge_distance(&config.map.canyon, center.x, center.z - eps);
                let inward_dir = Vec3::new(gx, 0.0, gz).normalize_or_zero();
                if inward_dir != Vec3::ZERO {
                    let push = (radius + 0.15 - edge_d).clamp(0.0, 1.5);
                    center += inward_dir * push;
                }
            }
        }

        for entry in &self.static_colliders {
            if entry.disabled || !soldier_groups.interacts_with(entry.groups) {
                continue;
            }
            if inside_canyon_carve && entry.bounding_radius > 200.0 {
                continue;
            }
            let max_r = entry.bounding_radius + half_seg + eff_radius;
            if center.distance_squared(entry.translation) > max_r * max_r {
                continue;
            }
            for &oy in &sample_offsets_y {
                let sphere_pos = center + Vec3::Y * oy;
                if let Some(contact) = sphere_vs_static_collider(sphere_pos, eff_radius, entry) {
                    if contact.normal.y >= max_slope_cos {
                        if !entry.is_non_walkable_wall && oy <= -half_seg * 0.5 && contact.penetration > 0.0 {
                            center.y += contact.normal.y * contact.penetration;
                            grounded = true;
                        }
                    } else {
                        let cur_feet_y = center.y - center_y_offset;
                        // از درزهای ناچیز کف زمین (اختلاف ارتفاع زیر 15 سانتی‌متر در کره پایینی) صرف‌نظر کن تا کاراکتر گیر نکند
                        if oy < 0.0 && contact.contact_point.y <= cur_feet_y + autostep_max_h {
                            continue;
                        }
                        let horiz_normal = Vec3::new(contact.normal.x, 0.0, contact.normal.z)
                            .normalize_or_zero();
                        if horiz_normal != Vec3::ZERO {
                            center += horiz_normal * (contact.penetration + nudge);
                        } else if entry.is_non_walkable_wall && config.map.canyon.enabled {
                            let eps = 0.8_f32;
                            let gx = canyon_edge_distance(&config.map.canyon, center.x + eps, center.z)
                                - canyon_edge_distance(&config.map.canyon, center.x - eps, center.z);
                            let gz = canyon_edge_distance(&config.map.canyon, center.x, center.z + eps)
                                - canyon_edge_distance(&config.map.canyon, center.x, center.z - eps);
                            let inward_dir = Vec3::new(gx, 0.0, gz).normalize_or_zero();
                            if inward_dir != Vec3::ZERO {
                                center += inward_dir * 0.18;
                            }
                        }
                    }
                }
            }
        }

        (center, grounded)
    }
}


fn ray_intersect_canyon(
    canyon: &CanyonConfig,
    origin: Vec3,
    unit_dir: Vec3,
    max_toi: f32,
) -> Option<RayHit> {
    if !canyon.enabled || max_toi <= 0.0 {
        return None;
    }
    if origin.y > 0.5 && unit_dir.y >= 0.0 {
        return None;
    }

    let limit_t = max_toi.min(240.0);
    let step = 0.65_f32;
    let mut prev_t = 0.0_f32;
    let mut prev_p = origin;
    let mut prev_ch = canyon_sample_height(canyon, prev_p.x, prev_p.z);

    let mut t = step;
    while t <= limit_t + step * 0.5 {
        let cur_t = t.min(limit_t);
        let p = origin + unit_dir * cur_t;
        if p.x.abs() <= 180.0 && p.z.abs() <= 120.0 && p.y <= 0.35 {
            let ch = canyon_sample_height(canyon, p.x, p.z);
            let relevant = ch < -0.15 || prev_ch < -0.15;
            if relevant && p.y <= ch && prev_p.y > prev_ch {
                let mut lo = prev_t;
                let mut hi = cur_t;
                for _ in 0..6 {
                    let mid = 0.5 * (lo + hi);
                    let mp = origin + unit_dir * mid;
                    let mch = canyon_sample_height(canyon, mp.x, mp.z);
                    if mp.y <= mch {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let hit_p = origin + unit_dir * hi;
                let hit_ch = canyon_sample_height(canyon, hit_p.x, hit_p.z);
                let normal = if hit_ch < -0.15 && prev_ch < -0.15 {
                    Vec3::Y
                } else {
                    Vec3::new(-unit_dir.x, 0.0, -unit_dir.z).normalize_or_zero()
                };
                return Some(RayHit {
                    toi: hi,
                    point: hit_p,
                    normal,
                });
            }
            prev_ch = ch;
        } else {
            prev_ch = 0.0;
        }
        prev_t = cur_t;
        prev_p = p;
        if cur_t >= limit_t {
            break;
        }
        t += step;
    }

    None
}

fn ray_intersect_single_entry(
    origin: Vec3,
    unit_dir: Vec3,
    max_toi: f32,
    entry: &StaticColliderEntry,
) -> Option<RayHit> {
    match entry.collider {
        CustomCollider::Cuboid { half_extents } => ray_intersect_obb(
            origin,
            unit_dir,
            max_toi,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_extents,
        ),
        CustomCollider::Cylinder {
            half_height,
            radius,
        } => ray_intersect_cylinder(
            origin,
            unit_dir,
            max_toi,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_height,
            radius,
        ),
        CustomCollider::Ball { radius } => {
            ray_intersect_sphere(origin, unit_dir, max_toi, entry.translation, radius)
        }
        CustomCollider::CapsuleY {
            half_segment,
            radius,
        } => ray_intersect_cylinder(
            origin,
            unit_dir,
            max_toi,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_segment + radius,
            radius,
        ),
        CustomCollider::Wedge {
            half_width,
            half_height,
            half_length,
        } => ray_intersect_wedge(
            origin,
            unit_dir,
            max_toi,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_width,
            half_height,
            half_length,
        ),
    }
}

fn ray_intersect_wedge(
    origin: Vec3,
    unit_dir: Vec3,
    max_toi: f32,
    pos: Vec3,
    rot: Quat,
    inv_rot: Quat,
    half_width: f32,
    half_height: f32,
    half_length: f32,
) -> Option<RayHit> {
    let local_origin = inv_rot * (origin - pos);
    let local_dir = inv_rot * unit_dir;
    let slope_normal = Vec3::new(0.0, half_length, -half_height).normalize_or_zero();
    let planes: [(Vec3, f32); 5] = [
        (Vec3::NEG_X, half_width),
        (Vec3::X, half_width),
        (Vec3::NEG_Y, half_height),
        (Vec3::Z, half_length),
        (slope_normal, 0.0),
    ];

    let mut t_enter = 0.0_f32;
    let mut t_exit = max_toi;
    let mut enter_normal = Vec3::Y;
    let mut hit_on_enter = false;

    for (n, d) in planes {
        let denom = n.dot(local_dir);
        let num = d - n.dot(local_origin);
        if denom.abs() < 1e-7 {
            if num < 0.0 {
                return None;
            }
        } else {
            let t = num / denom;
            if denom < 0.0 {
                if t > t_enter {
                    t_enter = t;
                    enter_normal = n;
                    hit_on_enter = true;
                }
            } else if t < t_exit {
                t_exit = t;
            }
            if t_enter > t_exit {
                return None;
            }
        }
    }

    if !hit_on_enter && t_enter == 0.0 {
        return None;
    }
    if t_enter >= 0.0 && t_enter <= max_toi {
        Some(RayHit {
            toi: t_enter,
            normal: (rot * enter_normal).normalize_or_zero(),
            point: origin + unit_dir * t_enter,
        })
    } else {
        None
    }
}

fn ray_intersect_obb(
    origin: Vec3,
    unit_dir: Vec3,
    max_toi: f32,
    box_pos: Vec3,
    box_rot: Quat,
    inv_box_rot: Quat,
    half: Vec3,
) -> Option<RayHit> {
    let local_origin = inv_box_rot * (origin - box_pos);
    let local_dir = inv_box_rot * unit_dir;

    let mut t_enter = -f32::INFINITY;
    let mut t_exit = max_toi;
    let mut enter_normal_local = Vec3::Y;

    let orig_arr = [local_origin.x, local_origin.y, local_origin.z];
    let dir_arr = [local_dir.x, local_dir.y, local_dir.z];
    let half_arr = [half.x, half.y, half.z];

    for axis in 0..3 {
        let o = orig_arr[axis];
        let d = dir_arr[axis];
        let h = half_arr[axis];

        if d.abs() < 1e-6 {
            if o < -h || o > h {
                return None;
            }
        } else {
            let inv_d = 1.0 / d;
            let t_neg_face = (-h - o) * inv_d;
            let t_pos_face = (h - o) * inv_d;

            let (t_near, t_far, near_normal_sign) = if t_neg_face <= t_pos_face {
                (t_neg_face, t_pos_face, -1.0_f32)
            } else {
                (t_pos_face, t_neg_face, 1.0_f32)
            };

            if t_near > t_enter {
                t_enter = t_near;
                enter_normal_local = Vec3::ZERO;
                enter_normal_local[axis] = near_normal_sign;
            }
            t_exit = t_exit.min(t_far);
            if t_enter > t_exit || t_exit < 0.0 {
                return None;
            }
        }
    }

    if t_enter > max_toi {
        return None;
    }

    let (toi, hit_normal_local) = if t_enter >= 0.0 {
        (t_enter, enter_normal_local)
    } else {
        let dx = (half.x - local_origin.x.abs()).abs();
        let dy = (half.y - local_origin.y.abs()).abs();
        let dz = (half.z - local_origin.z.abs()).abs();
        let inside_normal = if dy <= dx && dy <= dz {
            Vec3::new(0.0, if local_origin.y >= 0.0 { 1.0 } else { -1.0 }, 0.0)
        } else if dx <= dz {
            Vec3::new(if local_origin.x >= 0.0 { 1.0 } else { -1.0 }, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, if local_origin.z >= 0.0 { 1.0 } else { -1.0 })
        };
        (0.0, inside_normal)
    };

    let world_normal = (box_rot * hit_normal_local).normalize_or_zero();
    Some(RayHit {
        toi,
        point: origin + unit_dir * toi,
        normal: world_normal,
    })
}

fn ray_intersect_cylinder(
    origin: Vec3,
    unit_dir: Vec3,
    max_toi: f32,
    cyl_pos: Vec3,
    cyl_rot: Quat,
    inv_cyl_rot: Quat,
    half_height: f32,
    radius: f32,
) -> Option<RayHit> {
    let lo = inv_cyl_rot * (origin - cyl_pos);
    let ld = inv_cyl_rot * unit_dir;

    if lo.x * lo.x + lo.z * lo.z <= radius * radius && lo.y.abs() <= half_height {
        let radial = Vec3::new(lo.x, 0.0, lo.z).normalize_or_zero();
        let n_local = if (half_height - lo.y.abs()) < (radius - (lo.x * lo.x + lo.z * lo.z).sqrt())
        {
            Vec3::new(0.0, if lo.y >= 0.0 { 1.0 } else { -1.0 }, 0.0)
        } else {
            radial
        };
        return Some(RayHit {
            toi: 0.0,
            point: origin,
            normal: (cyl_rot * n_local).normalize_or_zero(),
        });
    }

    let mut best_t = max_toi + 1.0;
    let mut best_normal_local = Vec3::Y;

    if ld.y.abs() > 1e-6 {
        for &cap_sign in &[-1.0_f32, 1.0_f32] {
            let cap_y = cap_sign * half_height;
            let t = (cap_y - lo.y) / ld.y;
            if (0.0..=max_toi).contains(&t) && t < best_t {
                let px = lo.x + ld.x * t;
                let pz = lo.z + ld.z * t;
                if px * px + pz * pz <= radius * radius {
                    best_t = t;
                    best_normal_local = Vec3::new(0.0, cap_sign, 0.0);
                }
            }
        }
    }

    let a = ld.x * ld.x + ld.z * ld.z;
    if a > 1e-6 {
        let b = 2.0 * (lo.x * ld.x + lo.z * ld.z);
        let c = lo.x * lo.x + lo.z * lo.z - radius * radius;
        let disc = b * b - 4.0 * a * c;
        if disc >= 0.0 {
            let sqrt_d = disc.sqrt();
            let t1 = (-b - sqrt_d) / (2.0 * a);
            let t2 = (-b + sqrt_d) / (2.0 * a);
            for &t in &[t1, t2] {
                if (0.0..=max_toi).contains(&t) && t < best_t {
                    let py = lo.y + ld.y * t;
                    if py.abs() <= half_height {
                        best_t = t;
                        let px = lo.x + ld.x * t;
                        let pz = lo.z + ld.z * t;
                        best_normal_local = Vec3::new(px, 0.0, pz).normalize_or_zero();
                        break;
                    }
                }
            }
        }
    }

    if best_t <= max_toi {
        Some(RayHit {
            toi: best_t,
            point: origin + unit_dir * best_t,
            normal: (cyl_rot * best_normal_local).normalize_or_zero(),
        })
    } else {
        None
    }
}

fn ray_intersect_sphere(
    origin: Vec3,
    unit_dir: Vec3,
    max_toi: f32,
    sphere_pos: Vec3,
    radius: f32,
) -> Option<RayHit> {
    let m = origin - sphere_pos;
    let b = m.dot(unit_dir);
    let c = m.length_squared() - radius * radius;
    if c <= 0.0 {
        return Some(RayHit {
            toi: 0.0,
            point: origin,
            normal: m.normalize_or_zero(),
        });
    }
    if b > 0.0 {
        return None;
    }
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if (0.0..=max_toi).contains(&t) {
        let point = origin + unit_dir * t;
        Some(RayHit {
            toi: t,
            point,
            normal: ((point - sphere_pos) / radius).normalize_or_zero(),
        })
    } else {
        None
    }
}

pub fn sphere_vs_static_collider(
    sphere_center: Vec3,
    sphere_radius: f32,
    entry: &StaticColliderEntry,
) -> Option<SphereContact> {
    match entry.collider {
        CustomCollider::Cuboid { half_extents } => sphere_vs_obb(
            sphere_center,
            sphere_radius,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_extents,
        ),
        CustomCollider::Cylinder {
            half_height,
            radius,
        } => sphere_vs_cylinder(
            sphere_center,
            sphere_radius,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_height,
            radius,
        ),
        CustomCollider::Ball { radius } => {
            let diff = sphere_center - entry.translation;
            let dist = diff.length();
            let min_d = sphere_radius + radius;
            if dist < min_d {
                let n = if dist > 1e-5 { diff / dist } else { Vec3::Y };
                Some(SphereContact {
                    normal: n,
                    penetration: min_d - dist,
                    contact_point: entry.translation + n * radius,
                })
            } else {
                None
            }
        }
        CustomCollider::CapsuleY {
            half_segment,
            radius,
        } => sphere_vs_cylinder(
            sphere_center,
            sphere_radius,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_segment + radius,
            radius,
        ),
        CustomCollider::Wedge {
            half_width,
            half_height,
            half_length,
        } => sphere_vs_wedge(
            sphere_center,
            sphere_radius,
            entry.translation,
            entry.rotation,
            entry.inv_rotation,
            half_width,
            half_height,
            half_length,
        ),
    }
}

#[inline]
fn closest_point_on_segment_2d(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq <= 1e-8 {
        return a;
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    a + ab * t
}

fn sphere_vs_wedge(
    sphere_center: Vec3,
    sphere_radius: f32,
    pos: Vec3,
    rot: Quat,
    inv_rot: Quat,
    half_width: f32,
    half_height: f32,
    half_length: f32,
) -> Option<SphereContact> {
    let local_p = inv_rot * (sphere_center - pos);
    let slope_n_2d = Vec2::new(-half_height, half_length).normalize_or_zero();
    let p2 = Vec2::new(local_p.z, local_p.y);

    let d_left = -half_width - local_p.x;
    let d_right = local_p.x - half_width;
    let d_bottom = -half_height - local_p.y;
    let d_back = local_p.z - half_length;
    let d_slope = slope_n_2d.dot(p2);

    if d_left <= 0.0 && d_right <= 0.0 && d_bottom <= 0.0 && d_back <= 0.0 && d_slope <= 0.0 {
        let faces = [
            (d_left, Vec3::NEG_X),
            (d_right, Vec3::X),
            (d_bottom, Vec3::NEG_Y),
            (d_back, Vec3::Z),
            (d_slope, Vec3::new(0.0, slope_n_2d.y, slope_n_2d.x)),
        ];
        let mut best_d = f32::NEG_INFINITY;
        let mut best_n = Vec3::Y;
        for (d, n) in faces {
            if d > best_d {
                best_d = d;
                best_n = n;
            }
        }
        let world_n = (rot * best_n).normalize_or_zero();
        return Some(SphereContact {
            normal: world_n,
            penetration: sphere_radius - best_d,
            contact_point: pos + rot * (local_p - best_n * best_d),
        });
    }

    let qx = local_p.x.clamp(-half_width, half_width);
    let q2 = if d_bottom <= 0.0 && d_back <= 0.0 && d_slope <= 0.0 {
        p2
    } else {
        let a = Vec2::new(-half_length, -half_height);
        let b = Vec2::new(half_length, -half_height);
        let c = Vec2::new(half_length, half_height);
        let c_ab = closest_point_on_segment_2d(p2, a, b);
        let c_bc = closest_point_on_segment_2d(p2, b, c);
        let c_ca = closest_point_on_segment_2d(p2, c, a);
        let d_ab = p2.distance_squared(c_ab);
        let d_bc = p2.distance_squared(c_bc);
        let d_ca = p2.distance_squared(c_ca);
        if d_ab <= d_bc && d_ab <= d_ca {
            c_ab
        } else if d_bc <= d_ca {
            c_bc
        } else {
            c_ca
        }
    };

    let closest = Vec3::new(qx, q2.y, q2.x);
    let diff = local_p - closest;
    let dist_sq = diff.length_squared();
    if dist_sq < sphere_radius * sphere_radius {
        let dist = dist_sq.sqrt();
        let local_n = if dist > 1e-6 { diff / dist } else { Vec3::Y };
        Some(SphereContact {
            normal: (rot * local_n).normalize_or_zero(),
            penetration: sphere_radius - dist,
            contact_point: pos + rot * closest,
        })
    } else {
        None
    }
}

fn sphere_vs_obb(
    sphere_center: Vec3,
    sphere_radius: f32,
    box_pos: Vec3,
    box_rot: Quat,
    inv_box_rot: Quat,
    half: Vec3,
) -> Option<SphereContact> {
    let local_p = inv_box_rot * (sphere_center - box_pos);
    let closest = local_p.clamp(-half, half);
    let diff = local_p - closest;
    let dist_sq = diff.length_squared();

    if dist_sq > sphere_radius * sphere_radius {
        return None;
    }

    if dist_sq > 1e-8 {
        let dist = dist_sq.sqrt();
        let local_normal = diff / dist;
        let world_normal = (box_rot * local_normal).normalize_or_zero();
        let world_contact = box_pos + box_rot * closest;
        Some(SphereContact {
            normal: world_normal,
            penetration: sphere_radius - dist,
            contact_point: world_contact,
        })
    } else {
        let dx = half.x - local_p.x.abs();
        let dy = half.y - local_p.y.abs();
        let dz = half.z - local_p.z.abs();

        let (local_normal, face_dist) = if dy <= dx && dy <= dz {
            (
                Vec3::new(0.0, if local_p.y >= 0.0 { 1.0 } else { -1.0 }, 0.0),
                dy,
            )
        } else if dx <= dz {
            (
                Vec3::new(if local_p.x >= 0.0 { 1.0 } else { -1.0 }, 0.0, 0.0),
                dx,
            )
        } else {
            (
                Vec3::new(0.0, 0.0, if local_p.z >= 0.0 { 1.0 } else { -1.0 }),
                dz,
            )
        };

        let world_normal = (box_rot * local_normal).normalize_or_zero();
        Some(SphereContact {
            normal: world_normal,
            penetration: sphere_radius + face_dist.max(0.0),
            contact_point: sphere_center - world_normal * face_dist.max(0.0),
        })
    }
}

fn sphere_vs_cylinder(
    sphere_center: Vec3,
    sphere_radius: f32,
    cyl_pos: Vec3,
    cyl_rot: Quat,
    inv_cyl_rot: Quat,
    half_height: f32,
    cyl_radius: f32,
) -> Option<SphereContact> {
    let local_p = inv_cyl_rot * (sphere_center - cyl_pos);
    let radial_dist = (local_p.x * local_p.x + local_p.z * local_p.z).sqrt();
    let clamped_y = local_p.y.clamp(-half_height, half_height);

    let (clamped_x, clamped_z) = if radial_dist > cyl_radius && radial_dist > 1e-6 {
        let scale = cyl_radius / radial_dist;
        (local_p.x * scale, local_p.z * scale)
    } else {
        (local_p.x, local_p.z)
    };

    let closest = Vec3::new(clamped_x, clamped_y, clamped_z);
    let diff = local_p - closest;
    let dist_sq = diff.length_squared();

    if dist_sq > sphere_radius * sphere_radius {
        return None;
    }

    if dist_sq > 1e-8 {
        let dist = dist_sq.sqrt();
        let local_normal = diff / dist;
        let world_normal = (cyl_rot * local_normal).normalize_or_zero();
        Some(SphereContact {
            normal: world_normal,
            penetration: sphere_radius - dist,
            contact_point: cyl_pos + cyl_rot * closest,
        })
    } else {
        let radial_pen = cyl_radius - radial_dist;
        let cap_pen = half_height - local_p.y.abs();
        let (local_normal, face_dist) = if cap_pen < radial_pen {
            (
                Vec3::new(0.0, if local_p.y >= 0.0 { 1.0 } else { -1.0 }, 0.0),
                cap_pen,
            )
        } else if radial_dist > 1e-5 {
            (
                Vec3::new(local_p.x / radial_dist, 0.0, local_p.z / radial_dist),
                radial_pen,
            )
        } else {
            (Vec3::X, radial_pen)
        };
        let world_normal = (cyl_rot * local_normal).normalize_or_zero();
        Some(SphereContact {
            normal: world_normal,
            penetration: sphere_radius + face_dist.max(0.0),
            contact_point: sphere_center - world_normal * face_dist.max(0.0),
        })
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobaPhysicsSet {
    InputIngress,
    Drive,
    Simulate,
    React,
    NetworkEgress,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhysicsRole {
    Server,
    Client,
}

pub struct MobaSharedPhysicsPlugin {
    pub role: PhysicsRole,
    pub tick_rate_hz: f64,
    pub substeps: usize,
}

impl Plugin for MobaSharedPhysicsPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TransformPlugin>() {
            app.add_plugins(TransformPlugin);
        }

        app.insert_resource(Time::<Fixed>::from_hz(self.tick_rate_hz))
            .init_resource::<CustomPhysicsWorld>()
            .configure_sets(
                FixedUpdate,
                (
                    MobaPhysicsSet::InputIngress,
                    MobaPhysicsSet::Drive,
                    MobaPhysicsSet::Simulate,
                    MobaPhysicsSet::React,
                    MobaPhysicsSet::NetworkEgress,
                )
                    .chain(),
            )
            .add_systems(Startup, spawn_shared_physics_actors_system)
            .add_systems(
                FixedUpdate,
                step_custom_physics_simulation_system.in_set(MobaPhysicsSet::Simulate),
            );

        match self.role {
            PhysicsRole::Server => {
                app.add_systems(Startup, spawn_server_environment_colliders_system)
                    .add_systems(
                        FixedUpdate,
                        (
                            sync_static_colliders_to_world_system,
                            shared_authoritative_drive_system,
                        )
                            .chain()
                            .in_set(MobaPhysicsSet::Drive),
                    )
                    .add_systems(
                        FixedUpdate,
                        shared_authoritative_react_system.in_set(MobaPhysicsSet::React),
                    );
            }
            PhysicsRole::Client => {
                app.add_systems(
                    FixedUpdate,
                    (
                        sync_static_colliders_to_world_system,
                        shared_client_drive_system,
                    )
                        .chain()
                        .in_set(MobaPhysicsSet::Drive),
                )
                .add_systems(
                    FixedUpdate,
                    shared_client_react_system.in_set(MobaPhysicsSet::React),
                );
            }
        }
    }
}

#[derive(Component)]
pub struct MapObjectId(pub String);

#[derive(Component)]
pub struct ElevationSurface {
    pub elevation_tier: i32,
    pub top_y: f32,
}

#[derive(Component)]
pub struct PhysicsGround;

#[derive(Component, Clone, Debug)]
pub struct PhysicsObstacle {
    pub id: String,
    pub can_vault: bool,
    pub vault_sides: Vec<VaultSide>,
    pub vault_height_offset: f32,
    pub top_y: f32,
}

#[derive(Component)]
pub struct PhysicsTarget {
    pub id: String,
    pub index: usize,
}

#[derive(Component)]
pub struct PhysicsMountain {
    pub id: String,
}

#[derive(Component)]
pub struct PhysicsTree {
    pub id: String,
}

#[derive(Component)]
pub struct PhysicsTurret {
    pub id: String,
    pub index: usize,
    pub team: TeamId,
    pub lane: LaneId,
    pub tier: u8,
}

#[derive(Component)]
pub struct PhysicsBarracks {
    pub id: String,
    pub index: usize,
    pub team: TeamId,
    pub lane: LaneId,
    pub kind: BarracksKind,
}

#[derive(Component)]
pub struct PhysicsAncient {
    pub id: String,
    pub index: usize,
    pub team: TeamId,
}

#[derive(Component)]
pub struct PhysicsFountain {
    pub id: String,
    pub team: TeamId,
}

#[derive(Component)]
pub struct PhysicsSoldier {
    pub slot_index: usize,
}

#[derive(Component)]
pub struct PhysicsGrenade {
    pub id: u64,
    pub owner_slot: usize,
    pub owner_team: TeamId,
    pub remaining_fuse: f32,
    pub damage: f32,
    pub aoe_radius: f32,
}

#[derive(Component)]
pub struct PhysicsBullet {
    pub id: u64,
    pub owner_slot: Option<usize>,
    pub owner_creep_id: Option<u64>,
    pub owner_turret_index: Option<usize>,
    pub owner_team: TeamId,
    pub vfx_source: BulletVfxSource,
    pub velocity: Vec3,
    pub damage: f32,
    pub remaining_range: f32,
    pub gravity: f32,
    pub aoe_radius: f32,
    pub toxic_pool_duration_secs: f32,
    pub toxic_pool_dps: f32,
    pub toxic_slow_ratio: f32,
    pub piercing: bool,
    pub hit_entity_keys: Vec<u64>,
}

#[derive(Component)]
pub struct PhysicsShellCasing {
    pub lifetime: Timer,
}

#[derive(Component)]
pub struct PhysicsParticle {
    pub lifetime: Timer,
    pub scale_decay: bool,
}

// -----------------------------------------------------------------------------
// ۴. باندل‌های ساخت کولایدر و اجسام فیزیکی مشترک نقشه
// -----------------------------------------------------------------------------

pub fn ground_physics_bundle(
    config: &GameConfig,
) -> (
    PhysicsGround,
    ElevationSurface,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let hx = config.map.map_dimensions.0 * 0.5;
    let hy = config.physics.ground_half_thickness;
    let hz = config.map.map_dimensions.1 * 0.5;
    let river_floor_y = -1.6;
    (
        PhysicsGround,
        ElevationSurface {
            elevation_tier: -1,
            top_y: river_floor_y,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(hx, hy, hz),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        Transform::from_xyz(0.0, river_floor_y - hy, 0.0),
    )
}

pub fn terrain_platform_physics_bundle(
    config: &GameConfig,
    plat: &TerrainPlatformDef,
) -> (
    MapObjectId,
    ElevationSurface,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let tf = plat.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        plat.size.0 * 0.5 * s.x,
        plat.size.1 * 0.5 * s.y,
        plat.size.2 * 0.5 * s.z,
    );
    (
        MapObjectId(plat.id.clone()),
        ElevationSurface {
            elevation_tier: plat.elevation_tier,
            top_y: tf.translation.y + half.y,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn ramp_physics_bundle(
    config: &GameConfig,
    ramp: &RampDef,
) -> (
    MapObjectId,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let tf = ramp.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        ramp.size.0 * 0.5 * s.x,
        ramp.size.1 * 0.5 * s.y,
        ramp.size.2 * 0.5 * s.z,
    );
    (
        MapObjectId(ramp.id.clone()),
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn roshan_pit_physics_bundles(
    config: &GameConfig,
) -> Vec<(
    MapObjectId,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
)> {
    let pit = &config.map.roshan_pit;
    if !pit.enabled {
        return Vec::new();
    }
    let mut bundles = Vec::new();
    let tf = pit.model_transform.to_transform();
    let base_pos = tf.translation;
    let base_rot = tf.rotation;
    let mat = SurfaceMaterial {
        friction: config.physics.ground_friction,
        restitution: config.physics.ground_restitution,
    };
    let obs_mat = SurfaceMaterial {
        friction: config.physics.obstacle_friction,
        restitution: config.physics.obstacle_restitution,
    };

    // ۱. کف خشک غار (روی سطح آب رودخانه)
    let floor_top_y = base_pos.y + 0.15;
    let floor_center_y = floor_top_y - pit.floor_thickness * 0.5;
    bundles.push((
        MapObjectId(format!("{}_floor", pit.id)),
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(pit.floor_thickness * 0.5, pit.outer_radius),
        mat,
        world_collision_groups(),
        Transform::from_xyz(base_pos.x, floor_center_y, base_pos.z),
    ));

    // ۲. سقف سرپوشیده غار
    let roof_center_y = base_pos.y + pit.wall_height + pit.roof_thickness * 0.5;
    bundles.push((
        MapObjectId(format!("{}_roof", pit.id)),
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(pit.roof_thickness * 0.5, pit.outer_radius),
        obs_mat,
        world_collision_groups(),
        Transform::from_xyz(base_pos.x, roof_center_y, base_pos.z),
    ));

    // ۳. دیواره‌های استوانه‌ای نیم‌دایره (با دهانه ورودی رو به آبشار)
    let segments = 16_usize;
    let ent_rad = pit.entrance_angle_deg.to_radians();
    let half_gap_rad = pit.entrance_half_angle_deg.to_radians();
    let arc_span = std::f32::consts::TAU - 2.0 * half_gap_rad;
    let start_ang = ent_rad + half_gap_rad;
    let step_ang = arc_span / (segments as f32);
    let r_mid = 0.5 * (pit.inner_radius + pit.outer_radius);
    let wall_thick = (pit.outer_radius - pit.inner_radius).max(0.5);
    let seg_len = 2.0 * r_mid * (step_ang * 0.5).sin() + 0.35;
    let wall_center_y = base_pos.y + pit.wall_height * 0.5;

    for i in 0..segments {
        let ang = start_ang + (i as f32 + 0.5) * step_ang;
        let local_offset = Vec3::new(ang.cos() * r_mid, 0.0, ang.sin() * r_mid);
        let world_offset = base_rot * local_offset;
        let radial = Vec3::new(world_offset.x, 0.0, world_offset.z).normalize_or_zero();
        let seg_rot = Transform::IDENTITY.looking_to(radial, Vec3::Y).rotation;
        bundles.push((
            MapObjectId(format!("{}_wall_{}", pit.id, i)),
            CustomRigidBody::Fixed,
            CustomCollider::cuboid(seg_len * 0.5, pit.wall_height * 0.5, wall_thick * 0.5),
            obs_mat,
            world_collision_groups(),
            Transform {
                translation: Vec3::new(
                    base_pos.x + world_offset.x,
                    wall_center_y,
                    base_pos.z + world_offset.z,
                ),
                rotation: seg_rot,
                scale: Vec3::ONE,
            },
        ));
    }

    bundles
}

pub fn center_bridge_physics_bundle(
    config: &GameConfig,
    bridge: &CenterBridgeDef,
) -> (
    MapObjectId,
    ElevationSurface,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let tf = bridge.model_transform.to_transform();
    let s = tf.scale;
    let half_h = bridge.deck_thickness * 0.5 * s.y;
    let rad = bridge.outer_radius * s.x.max(s.z);
    (
        MapObjectId(bridge.id.clone()),
        ElevationSurface {
            elevation_tier: bridge.elevation_tier,
            top_y: tf.translation.y + half_h,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(half_h, rad),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn lane_segment_physics_bundle(
    config: &GameConfig,
    seg_id: String,
    start: Vec3,
    end: Vec3,
    width: f32,
    thickness: f32,
) -> (
    MapObjectId,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let diff = end - start;
    let len_3d = diff.length();
    let mid = (start + end) * 0.5;
    let dir = diff.normalize_or_zero();
    let rot = Transform::IDENTITY.looking_to(dir, Vec3::Y).rotation;
    (
        MapObjectId(seg_id),
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(width * 0.5, thickness * 0.5, len_3d * 0.5),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        Transform {
            translation: mid,
            rotation: rot,
            scale: Vec3::ONE,
        },
    )
}

/// کلیدر منشور مثلثی توپر (مکعب مثلث) برای پله‌های ورودی دو پایگاه تا زیر پله کاملاً پر و غیرقابل نفوذ باشد
pub fn entrance_stair_physics_bundle(
    config: &GameConfig,
    seg_id: String,
    start: Vec3,
    end: Vec3,
    width: f32,
) -> (
    MapObjectId,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let (low_pt, high_pt) = if start.y <= end.y {
        (start, end)
    } else {
        (end, start)
    };
    let height = (high_pt.y - low_pt.y).max(0.05);
    let horiz = Vec3::new(high_pt.x - low_pt.x, 0.0, high_pt.z - low_pt.z);
    let len_xz = horiz.length().max(0.05);
    let dir_xz = horiz / len_xz;
    let right = Vec3::Y.cross(dir_xz).normalize_or_zero();
    let rot = Quat::from_mat3(&Mat3::from_cols(right, Vec3::Y, dir_xz));
    let center = Vec3::new(
        (low_pt.x + high_pt.x) * 0.5,
        (low_pt.y + high_pt.y) * 0.5,
        (low_pt.z + high_pt.z) * 0.5,
    );
    (
        MapObjectId(seg_id),
        CustomRigidBody::Fixed,
        CustomCollider::wedge(width * 0.5, height * 0.5, len_xz * 0.5),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        Transform {
            translation: center,
            rotation: rot,
            scale: Vec3::ONE,
        },
    )
}

pub fn mountain_physics_bundle(
    config: &GameConfig,
    mtn: &MountainDef,
) -> (
    MapObjectId,
    PhysicsMountain,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let tf = mtn.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        mtn.size.0 * 0.5 * s.x,
        mtn.size.1 * 0.5 * s.y,
        mtn.size.2 * 0.5 * s.z,
    );
    (
        MapObjectId(mtn.id.clone()),
        PhysicsMountain {
            id: mtn.id.clone(),
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn tree_physics_bundle(
    config: &GameConfig,
    tree: &TreeDef,
) -> (
    MapObjectId,
    PhysicsTree,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let mut tf = tree.model_transform.to_transform();
    let s = tf.scale;
    let half_h = tree.trunk_height * 0.5 * s.y;
    tf.translation.y += half_h;
    let rad = tree.trunk_radius * s.x.max(s.z);
    (
        MapObjectId(tree.id.clone()),
        PhysicsTree {
            id: tree.id.clone(),
        },
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(half_h, rad),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn turret_physics_bundle(
    config: &GameConfig,
    index: usize,
    turret: &TurretDef,
) -> (
    MapObjectId,
    PhysicsTurret,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let mut tf = turret.model_transform.to_transform();
    let s = tf.scale;
    let half_h = turret.height * 0.5 * s.y;
    tf.translation.y += half_h;
    let rad = turret.base_radius * s.x.max(s.z);
    (
        MapObjectId(turret.id.clone()),
        PhysicsTurret {
            id: turret.id.clone(),
            index,
            team: turret.team,
            lane: turret.lane,
            tier: turret.tier,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(half_h, rad),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn barracks_physics_bundle(
    config: &GameConfig,
    index: usize,
    barrack: &BarracksDef,
) -> (
    MapObjectId,
    PhysicsBarracks,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let mut tf = barrack.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        barrack.size.0 * 0.5 * s.x,
        barrack.size.1 * 0.5 * s.y,
        barrack.size.2 * 0.5 * s.z,
    );
    tf.translation.y += half.y;
    (
        MapObjectId(barrack.id.clone()),
        PhysicsBarracks {
            id: barrack.id.clone(),
            index,
            team: barrack.team,
            lane: barrack.lane,
            kind: barrack.kind,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn ancient_physics_bundle(
    config: &GameConfig,
    index: usize,
    ancient: &AncientDef,
) -> (
    MapObjectId,
    PhysicsAncient,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let mut tf = ancient.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        ancient.base_size.0 * 0.5 * s.x,
        (ancient.base_size.1 + ancient.core_height) * 0.5 * s.y,
        ancient.base_size.2 * 0.5 * s.z,
    );
    tf.translation.y += half.y;
    (
        MapObjectId(ancient.id.clone()),
        PhysicsAncient {
            id: ancient.id.clone(),
            index,
            team: ancient.team,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn obstacle_physics_bundle(
    config: &GameConfig,
    obs: &ObstacleDef,
) -> (
    MapObjectId,
    PhysicsObstacle,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let tf = obs.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        obs.size.0 * 0.5 * s.x,
        obs.size.1 * 0.5 * s.y,
        obs.size.2 * 0.5 * s.z,
    );
    let world_half_y = (tf.rotation * Vec3::X).y.abs() * half.x
        + (tf.rotation * Vec3::Y).y.abs() * half.y
        + (tf.rotation * Vec3::Z).y.abs() * half.z;
    let top_y = tf.translation.y + world_half_y;
    (
        MapObjectId(obs.id.clone()),
        PhysicsObstacle {
            id: obs.id.clone(),
            can_vault: obs.can_vault,
            vault_sides: obs.vault_sides.clone(),
            vault_height_offset: obs.vault_height_offset,
            top_y,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn target_physics_bundle(
    config: &GameConfig,
    index: usize,
    target: &TargetDef,
) -> (
    MapObjectId,
    PhysicsTarget,
    CustomRigidBody,
    CustomCollider,
    SurfaceMaterial,
    CollisionGroups,
    Transform,
) {
    let tf = target.model_transform.to_transform();
    let s = tf.scale;
    let half = Vec3::new(
        target.size.0 * 0.5 * s.x,
        target.size.1 * 0.5 * s.y,
        target.size.2 * 0.5 * s.z,
    );
    (
        MapObjectId(target.id.clone()),
        PhysicsTarget {
            id: target.id.clone(),
            index,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.target_friction,
            restitution: config.physics.target_restitution,
        },
        world_collision_groups(),
        tf,
    )
}

pub fn team_spawn_pos(config: &GameConfig, team: TeamId, team_slot_offset: usize) -> Vec3 {
    // چیدمان متقارن (0, +1, -1, +2, -2, ...) حول محور فاونتن تا هر ۵ بازیکن تیم کاملاً داخل شعاع فاونتن/شاپ اسپاون شوند
    let signed_step: i32 = if team_slot_offset == 0 {
        0
    } else {
        let rank = ((team_slot_offset + 1) / 2) as i32;
        if team_slot_offset % 2 == 1 { rank } else { -rank }
    };
    let z_off = (signed_step as f32) * config.soldier.team_spawn_slot_spacing;
    match team {
        TeamId::Dawn => Vec3::new(
            config.soldier.spawn_pos.0,
            config.soldier.spawn_pos.1,
            config.soldier.spawn_pos.2 + z_off,
        ),
        TeamId::Dusk => Vec3::new(
            -config.soldier.spawn_pos.0,
            config.soldier.spawn_pos.1,
            config.soldier.spawn_pos.2 + z_off,
        ),
        TeamId::Neutral => Vec3::new(
            config.soldier.spawn_pos.0,
            config.soldier.spawn_pos.1,
            config.soldier.spawn_pos.2 + z_off,
        ),
    }
}

pub fn soldier_physics_bundle_for_slot(
    config: &GameConfig,
    slot_index: usize,
    spawn_base: Vec3,
    hero_scale: f32,
) -> (
    PhysicsSoldier,
    CustomRigidBody,
    CustomCollider,
    CollisionGroups,
    CustomCharacterController,
    Transform,
) {
    let spawn_pos = Vec3::new(
        spawn_base.x,
        spawn_base.y + config.physics.soldier_capsule_center_y * hero_scale,
        spawn_base.z,
    );
    (
        PhysicsSoldier { slot_index },
        CustomRigidBody::KinematicPositionBased,
        CustomCollider::capsule_y(
            config.physics.soldier_capsule_half_segment * hero_scale,
            config.soldier.player_radius * hero_scale,
        ),
        soldier_collision_groups(),
        CustomCharacterController {
            translation: None,
            grounded: true,
        },
        Transform::from_translation(spawn_pos),
    )
}

pub fn soldier_physics_bundle(
    config: &GameConfig,
) -> (
    PhysicsSoldier,
    CustomRigidBody,
    CustomCollider,
    CollisionGroups,
    CustomCharacterController,
    Transform,
) {
    let hero = config.active_hero(0);
    let spawn_pos = Vec3::new(
        config.soldier.spawn_pos.0,
        config.soldier.spawn_pos.1 + config.physics.soldier_capsule_center_y * hero.hero_scale,
        config.soldier.spawn_pos.2,
    );
    (
        PhysicsSoldier { slot_index: 0 },
        CustomRigidBody::KinematicPositionBased,
        CustomCollider::capsule_y(
            config.physics.soldier_capsule_half_segment * hero.hero_scale,
            config.soldier.player_radius * hero.hero_scale,
        ),
        soldier_collision_groups(),
        CustomCharacterController {
            translation: None,
            grounded: true,
        },
        Transform::from_translation(spawn_pos),
    )
}

pub fn grenade_physics_bundle(
    config: &GameConfig,
    id: u64,
    owner_slot: usize,
    owner_team: TeamId,
    fuse_secs: f32,
    damage: f32,
    aoe_radius: f32,
    translation: Vec3,
    rotation: Quat,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
) -> (
    PhysicsGrenade,
    CustomRigidBody,
    CustomCollider,
    DynamicBodyProps,
    Velocity,
    CollisionGroups,
    Transform,
) {
    (
        PhysicsGrenade {
            id,
            owner_slot,
            owner_team,
            remaining_fuse: fuse_secs,
            damage,
            aoe_radius,
        },
        CustomRigidBody::Dynamic,
        CustomCollider::ball(config.physics.grenade_collider_radius),
        DynamicBodyProps {
            restitution: config.physics.grenade_restitution,
            friction: config.physics.grenade_friction,
            linear_damping: config.physics.grenade_linear_damping,
            angular_damping: config.physics.grenade_angular_damping,
            gravity_scale: config.physics.grenade_gravity_scale,
            ccd_enabled: true,
        },
        Velocity {
            linear: linear_velocity,
            angular: angular_velocity,
        },
        grenade_collision_groups(),
        Transform {
            translation,
            rotation,
            scale: Vec3::ONE,
        },
    )
}

pub fn shell_casing_physics_bundle(
    config: &GameConfig,
    translation: Vec3,
    linear_velocity: Vec3,
    angular_velocity: Vec3,
) -> (
    PhysicsShellCasing,
    CustomRigidBody,
    CustomCollider,
    DynamicBodyProps,
    Velocity,
    CollisionGroups,
    Transform,
) {
    (
        PhysicsShellCasing {
            lifetime: Timer::from_seconds(config.fx.shell_lifetime_secs, TimerMode::Once),
        },
        CustomRigidBody::Dynamic,
        CustomCollider::cylinder(
            config.physics.shell_collider_half_height,
            config.physics.shell_collider_radius,
        ),
        DynamicBodyProps {
            restitution: config.physics.shell_restitution,
            friction: config.physics.shell_friction,
            linear_damping: config.physics.shell_linear_damping,
            angular_damping: config.physics.shell_angular_damping,
            gravity_scale: config.physics.shell_gravity_scale,
            ccd_enabled: false,
        },
        Velocity {
            linear: linear_velocity,
            angular: angular_velocity,
        },
        fx_collision_groups(),
        Transform::from_translation(translation),
    )
}

pub fn particle_physics_bundle(
    config: &GameConfig,
    translation: Vec3,
    linear_velocity: Vec3,
    lifetime_secs: f32,
    scale_decay: bool,
) -> (
    PhysicsParticle,
    CustomRigidBody,
    CustomCollider,
    DynamicBodyProps,
    Velocity,
    CollisionGroups,
    Transform,
) {
    (
        PhysicsParticle {
            lifetime: Timer::from_seconds(lifetime_secs, TimerMode::Once),
            scale_decay,
        },
        CustomRigidBody::Dynamic,
        CustomCollider::ball(config.physics.particle_collider_radius),
        DynamicBodyProps {
            restitution: config.physics.particle_restitution,
            friction: config.physics.particle_friction,
            linear_damping: config.physics.particle_linear_damping,
            angular_damping: config.physics.particle_angular_damping,
            gravity_scale: config.physics.particle_gravity_scale,
            ccd_enabled: false,
        },
        Velocity {
            linear: linear_velocity,
            angular: Vec3::ZERO,
        },
        fx_collision_groups(),
        Transform::from_translation(translation),
    )
}

/// قانون برتری دید ارتفاعی و خط دید موانع (Dota 2 High-Ground + Line-of-Sight) با برد دلخواه
/// محاسبه ارتفاع واقعی سطح زمین/فلات (مانند High Ground پایگاه) در مختصات افقی (x, z)
#[inline]
pub fn sample_terrain_top_y(config: &GameConfig, x: f32, z: f32) -> f32 {
    let mut best_y = 0.0_f32;
    for plat in &config.map.terrains {
        let loc = plat.model_transform.location;
        let scl = plat.model_transform.scale;
        let hx = plat.size.0 * 0.5 * scl.0.abs();
        let hz = plat.size.2 * 0.5 * scl.2.abs();
        if (x - loc.0).abs() <= hx && (z - loc.2).abs() <= hz {
            let top_y = loc.1 + plat.size.1 * 0.5 * scl.1.abs();
            best_y = best_y.max(top_y);
        }
    }
    best_y
}

/// قانون واحد و فیزیکی خط دید سه‌بعدی برای تمام موجودیت‌های بازی (هیروها، سربازان لاین و جنگل، تاورها، سازه‌ها و کوریرها)
/// با تفکیک دقیق ارتفاع چشم ناظر و ارتفاع قابل‌دیدن هدف (ایستاده در برابر نشسته پشت سنگر)
pub fn can_observer_see_target_3d(
    physics_world: &CustomPhysicsWorld,
    config: &GameConfig,
    observer_feet_pos: Vec3,
    observer_eye_height: f32,
    target_feet_pos: Vec3,
    target_visible_height: f32,
    base_range: f32,
    high_ground_bonus: f32,
) -> bool {
    if base_range <= 0.0 {
        return false;
    }
    let obs_in_pit =
        is_point_inside_roshan_pit(&config.map.roshan_pit, observer_feet_pos.x, observer_feet_pos.z);
    let tgt_in_pit =
        is_point_inside_roshan_pit(&config.map.roshan_pit, target_feet_pos.x, target_feet_pos.z);
    if obs_in_pit != tgt_in_pit {
        return false;
    }

    let height_diff = target_feet_pos.y - observer_feet_pos.y;
    if height_diff >= config.map.high_ground_occlusion_min_diff {
        return false;
    }

    let effective_range = if observer_feet_pos.y - target_feet_pos.y
        >= config.map.high_ground_occlusion_min_diff
    {
        base_range + high_ground_bonus
    } else {
        base_range
    };

    let dist_xz = Vec2::new(
        target_feet_pos.x - observer_feet_pos.x,
        target_feet_pos.z - observer_feet_pos.z,
    )
    .length();
    if dist_xz > effective_range {
        return false;
    }

    let eye_origin = observer_feet_pos + Vec3::Y * observer_eye_height;
    let target_point = target_feet_pos + Vec3::Y * target_visible_height;
    let to_target = target_point - eye_origin;
    let dist = to_target.length();
    if dist <= f32::EPSILON {
        return true;
    }

    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);
    physics_world
        .cast_ray(eye_origin, to_target / dist, dist, query_groups, |_| true)
        .is_none()
}

pub fn can_observer_see_point_with_range(
    physics_world: &CustomPhysicsWorld,
    config: &GameConfig,
    observer_feet_pos: Vec3,
    target_feet_pos: Vec3,
    eye_height: f32,
    base_range: f32,
    high_ground_bonus: f32,
) -> bool {
    can_observer_see_target_3d(
        physics_world,
        config,
        observer_feet_pos,
        eye_height,
        target_feet_pos,
        eye_height,
        base_range,
        high_ground_bonus,
    )
}

/// قانون برتری دید ارتفاعی (Dota 2 High-Ground Vision Rule) با استفاده از موتور فیزیک اختصاصی
pub fn can_observer_see_point(
    physics_world: &CustomPhysicsWorld,
    config: &GameConfig,
    observer_feet_pos: Vec3,
    target_feet_pos: Vec3,
    eye_height: f32,
) -> bool {
    can_observer_see_point_with_range(
        physics_world,
        config,
        observer_feet_pos,
        target_feet_pos,
        eye_height,
        config.map.base_vision_range,
        config.map.high_ground_vision_bonus,
    )
}

pub const FOW_GRID_W: usize = 170;
pub const FOW_GRID_H: usize = 120;

/// سیستم Fog of War مشترک (Server-Authoritative در moba_shared)
/// سرور هر تیک دید هیرو و سازه‌های تیم را محاسبه می‌کند، آبجکت‌های خارج از دید را فیلتر می‌کند
/// و ماسک فشرده دید (`fog_visible_mask`) را برای نمایش به کلاینت می‌فرستد.
#[derive(Clone, Debug)]
pub struct SharedFogOfWar {
    pub terrain_y: Vec<f32>,
    pub ground_terrain_y: Vec<f32>,
    pub bridge_deck_y: Vec<f32>,
    pub blocker_top_y: Vec<f32>,
    pub low_cover_top_y: Vec<f32>,
    pub under_bridge_blocker_top_y: Vec<f32>,
    pub explored: Vec<bool>,
    pub visible_now: Vec<bool>,
    pub min_visible_y: Vec<f32>,
    pub under_bridge_visible: Vec<bool>,
    pub allied_sources: Vec<(Vec3, f32)>,
}

impl SharedFogOfWar {
    pub fn from_config(config: &GameConfig, hero_team: TeamId) -> Self {
        let total = FOW_GRID_W * FOW_GRID_H;
        let mut terrain_y = vec![0.0_f32; total];
        let mut ground_terrain_y = vec![0.0_f32; total];
        let mut bridge_deck_y = vec![f32::NEG_INFINITY; total];
        let mut blocker_top_y = vec![f32::NEG_INFINITY; total];
        let mut low_cover_top_y = vec![f32::NEG_INFINITY; total];
        let mut under_bridge_blocker_top_y = vec![f32::NEG_INFINITY; total];

        let min_diff = config.map.high_ground_occlusion_min_diff;

        for gz in 0..FOW_GRID_H {
            for gx in 0..FOW_GRID_W {
                let (wx, wz) = Self::cell_to_world(config, gx, gz);
                let idx = gz * FOW_GRID_W + gx;

                let canyon_h = canyon_sample_height(&config.map.canyon, wx, wz);
                let inside_canyon = config.map.canyon.enabled && canyon_h < -0.15;
                let mut ground_y = if inside_canyon { canyon_h } else { -1.6_f32 };
                for plat in &config.map.terrains {
                    if inside_canyon && plat.elevation_tier <= 0 {
                        continue;
                    }
                    let loc = plat.model_transform.location;
                    let scl = plat.model_transform.scale;
                    let hx = plat.size.0 * 0.5 * scl.0.abs();
                    let hz = plat.size.2 * 0.5 * scl.2.abs();
                    let inv_rot =
                        Quat::from_rotation_y(-plat.model_transform.rotation.1.to_radians());
                    let local = inv_rot * Vec3::new(wx - loc.0, 0.0, wz - loc.2);
                    if local.x.abs() <= hx && local.z.abs() <= hz {
                        let top_y = loc.1 + plat.size.1 * 0.5 * scl.1.abs();
                        ground_y = ground_y.max(top_y);
                    }
                }

                // جدا کردن ارتفاع عرشه پل‌های روی دره (پل مرکزی و پل‌های لاین) از بستر زیر پل
                let mut bridge_y = f32::NEG_INFINITY;
                if inside_canyon {
                    for lane in &config.map.lanes {
                        let half_w = lane.width * 0.5;
                        for seg_idx in 0..lane.waypoints.len().saturating_sub(1) {
                            let a = lane.waypoints[seg_idx];
                            let b = lane.waypoints[seg_idx + 1];
                            let abx = b.0 - a.0;
                            let abz = b.2 - a.2;
                            let l2 = abx * abx + abz * abz;
                            let t = if l2 > 1e-4 {
                                (((wx - a.0) * abx + (wz - a.2) * abz) / l2).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            let cx = a.0 + abx * t;
                            let cz = a.2 + abz * t;
                            if (wx - cx).hypot(wz - cz) <= half_w {
                                let cy = a.1 + (b.1 - a.1) * t + lane.thickness * 0.5;
                                bridge_y = bridge_y.max(cy);
                            }
                        }
                    }
                }

                let bridge = &config.map.center_bridge;
                let b_loc = bridge.model_transform.location;
                let b_s = bridge
                    .model_transform
                    .scale
                    .0
                    .abs()
                    .max(bridge.model_transform.scale.2.abs());
                let b_rad = bridge.outer_radius * b_s;
                let dx = wx - b_loc.0;
                let dz = wz - b_loc.2;
                if dx * dx + dz * dz <= b_rad * b_rad {
                    bridge_y = bridge_y.max(
                        b_loc.1 + bridge.deck_thickness * 0.5 * bridge.model_transform.scale.1.abs(),
                    );
                }

                let best_y = ground_y.max(bridge_y);
                ground_terrain_y[idx] = ground_y;
                bridge_deck_y[idx] = bridge_y;
                terrain_y[idx] = best_y;

                let has_bridge = bridge_y.is_finite();
                let mut max_blocker_y = f32::NEG_INFINITY;
                let mut max_low_cover_y = f32::NEG_INFINITY;
                let mut max_under_blocker_y = f32::NEG_INFINITY;

                for mtn in &config.map.mountains {
                    let loc = mtn.model_transform.location;
                    let scl = mtn.model_transform.scale;
                    let hy = mtn.size.1 * 0.5 * scl.1.abs();
                    let top_y = loc.1 + hy;
                    let bottom_y = loc.1 - hy;
                    let hx = mtn.size.0 * 0.5 * scl.0.abs();
                    let hz = mtn.size.2 * 0.5 * scl.2.abs();
                    let inv_rot =
                        Quat::from_rotation_y(-mtn.model_transform.rotation.1.to_radians());
                    let local = inv_rot * Vec3::new(wx - loc.0, 0.0, wz - loc.2);
                    if local.x.abs() <= hx && local.z.abs() <= hz {
                        if top_y >= best_y + min_diff {
                            max_blocker_y = max_blocker_y.max(top_y);
                        }
                        if has_bridge
                            && bottom_y < bridge_y - min_diff
                            && top_y >= ground_y + min_diff
                        {
                            max_under_blocker_y = max_under_blocker_y.max(top_y);
                        }
                    }
                }
                for obs in &config.obstacles {
                    if obs.size.0 <= 1e-4 || obs.size.1 <= 1e-4 || obs.size.2 <= 1e-4 {
                        continue;
                    }
                    let loc = obs.model_transform.location;
                    let scl = obs.model_transform.scale;
                    let hy = obs.size.1 * 0.5 * scl.1.abs();
                    let top_y = loc.1 + hy;
                    let bottom_y = loc.1 - hy;
                    let hx = obs.size.0 * 0.5 * scl.0.abs();
                    let hz = obs.size.2 * 0.5 * scl.2.abs();
                    let inv_rot =
                        Quat::from_rotation_y(-obs.model_transform.rotation.1.to_radians());
                    let local = inv_rot * Vec3::new(wx - loc.0, 0.0, wz - loc.2);
                    if local.x.abs() <= hx && local.z.abs() <= hz {
                        if obs.can_vault {
                            // سنگرها و موانع کم‌ارتفاع (مثل sandbag.glb): سایه دید پروجکتیو به اندازه ارتفاع واقعی خود می‌اندازند
                            if !has_bridge || top_y >= bridge_y - 0.5 {
                                max_low_cover_y = max_low_cover_y.max(top_y);
                            }
                        } else {
                            // موانع بلند غیرقابل‌پرش: مسدودکننده کامل خط دید
                            if !has_bridge || top_y >= bridge_y - 0.5 {
                                max_blocker_y = max_blocker_y.max(top_y);
                            }
                            if has_bridge && bottom_y < bridge_y - min_diff {
                                max_under_blocker_y = max_under_blocker_y.max(top_y);
                            }
                        }
                    }
                }
                let fow_r_mult = config.map.biome.tree_fow_blocker_radius_mult;
                let fow_r_min = config.map.biome.tree_fow_blocker_min_radius;
                let fow_r_max = config.map.biome.tree_fow_blocker_max_radius;
                for tree in &config.map.trees {
                    let loc = tree.model_transform.location;
                    let scl = tree.model_transform.scale;
                    let ts = scl.0.abs().max(scl.2.abs());
                    let bottom_y = loc.1;
                    let top_y = loc.1 + (tree.trunk_height + tree.canopy_height) * scl.1.abs();
                    let r = (tree.trunk_radius * fow_r_mult * ts).clamp(fow_r_min, fow_r_max);
                    let tdx = wx - loc.0;
                    let tdz = wz - loc.2;
                    if tdx * tdx + tdz * tdz <= r * r {
                        max_blocker_y = max_blocker_y.max(top_y);
                        if has_bridge && bottom_y < bridge_y - min_diff {
                            max_under_blocker_y = max_under_blocker_y.max(top_y);
                        }
                    }
                }
                blocker_top_y[idx] = max_blocker_y;
                low_cover_top_y[idx] = max_low_cover_y;
                under_bridge_blocker_top_y[idx] = max_under_blocker_y;
            }
        }

        // تضمین ثبت قطعی سلول دقیق هر درخت در شبکه Fog of War (جلوگیری از رد شدن دید از درختان کوچک بین مراکز سلول‌ها)
        for tree in &config.map.trees {
            let loc = tree.model_transform.location;
            let scl = tree.model_transform.scale;
            let bottom_y = loc.1;
            let top_y = loc.1 + (tree.trunk_height + tree.canopy_height) * scl.1.abs();
            if let Some((gx, gz)) = Self::world_to_cell(config, loc.0, loc.2) {
                let idx = gz * FOW_GRID_W + gx;
                blocker_top_y[idx] = blocker_top_y[idx].max(top_y);
                if bridge_deck_y[idx].is_finite() && bottom_y < bridge_deck_y[idx] - min_diff {
                    under_bridge_blocker_top_y[idx] = under_bridge_blocker_top_y[idx].max(top_y);
                }
            }
        }

        // تضمین ثبت قطعی دیوارها و سنگرهای باریک (مثل sandbag.glb) در سلول‌های شبکه Fog of War
        let cell_step = (config.map.map_dimensions.0 / FOW_GRID_W as f32)
            .min(config.map.map_dimensions.1 / FOW_GRID_H as f32)
            .max(0.25)
            * 0.5;
        for obs in &config.obstacles {
            if obs.size.0 <= 1e-4 || obs.size.1 <= 1e-4 || obs.size.2 <= 1e-4 {
                continue;
            }
            let loc = obs.model_transform.location;
            let scl = obs.model_transform.scale;
            let hx = obs.size.0 * 0.5 * scl.0.abs();
            let hy = obs.size.1 * 0.5 * scl.1.abs();
            let hz = obs.size.2 * 0.5 * scl.2.abs();
            let top_y = loc.1 + hy;
            let bottom_y = loc.1 - hy;
            let rot = Quat::from_rotation_y(obs.model_transform.rotation.1.to_radians());
            let steps_x = ((hx * 2.0) / cell_step).ceil().max(1.0) as usize;
            let steps_z = ((hz * 2.0) / cell_step).ceil().max(1.0) as usize;
            for ix in 0..=steps_x {
                let lx = -hx + (hx * 2.0) * (ix as f32 / steps_x as f32);
                for iz in 0..=steps_z {
                    let lz = -hz + (hz * 2.0) * (iz as f32 / steps_z as f32);
                    let wp = Vec3::new(loc.0, 0.0, loc.2) + rot * Vec3::new(lx, 0.0, lz);
                    if let Some((gx, gz)) = Self::world_to_cell(config, wp.x, wp.z) {
                        let idx = gz * FOW_GRID_W + gx;
                        let by = bridge_deck_y[idx];
                        if obs.can_vault {
                            if !by.is_finite() || top_y >= by - 0.5 {
                                low_cover_top_y[idx] = low_cover_top_y[idx].max(top_y);
                            }
                        } else {
                            if !by.is_finite() || top_y >= by - 0.5 {
                                blocker_top_y[idx] = blocker_top_y[idx].max(top_y);
                            }
                            if by.is_finite() && bottom_y < by - min_diff {
                                under_bridge_blocker_top_y[idx] =
                                    under_bridge_blocker_top_y[idx].max(top_y);
                            }
                        }
                    }
                }
            }
        }

        let mut allied_sources = Vec::new();
        for ftn in &config.map.fountains {
            if ftn.team == hero_team {
                let loc = ftn.model_transform.location;
                allied_sources.push((
                    Vec3::new(loc.0, loc.1, loc.2),
                    config.map.fow_fountain_vision_range,
                ));
            }
        }
        for anc in &config.map.ancients {
            if anc.team == hero_team {
                let loc = anc.model_transform.location;
                allied_sources.push((
                    Vec3::new(loc.0, loc.1, loc.2),
                    config.map.fow_ancient_vision_range,
                ));
            }
        }
        for tur in &config.map.turrets {
            if tur.team == hero_team {
                let loc = tur.model_transform.location;
                allied_sources.push((Vec3::new(loc.0, loc.1, loc.2), tur.vision_range));
            }
        }
        for bar in &config.map.barracks {
            if bar.team == hero_team {
                let loc = bar.model_transform.location;
                allied_sources.push((
                    Vec3::new(loc.0, loc.1, loc.2),
                    config.map.fow_barracks_vision_range,
                ));
            }
        }

        Self {
            terrain_y,
            ground_terrain_y,
            bridge_deck_y,
            blocker_top_y,
            low_cover_top_y,
            under_bridge_blocker_top_y,
            explored: vec![false; total],
            visible_now: vec![false; total],
            min_visible_y: vec![f32::INFINITY; total],
            under_bridge_visible: vec![false; total],
            allied_sources,
        }
    }

    #[inline]
    pub fn world_to_cell(config: &GameConfig, wx: f32, wz: f32) -> Option<(usize, usize)> {
        let map_w = config.map.map_dimensions.0.max(1.0);
        let map_h = config.map.map_dimensions.1.max(1.0);
        let u = wx / map_w + 0.5;
        let v = wz / map_h + 0.5;
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return None;
        }
        let gx = ((u * FOW_GRID_W as f32) as usize).min(FOW_GRID_W - 1);
        let gz = ((v * FOW_GRID_H as f32) as usize).min(FOW_GRID_H - 1);
        Some((gx, gz))
    }

    #[inline]
    pub fn cell_to_world(config: &GameConfig, gx: usize, gz: usize) -> (f32, f32) {
        let map_w = config.map.map_dimensions.0;
        let map_h = config.map.map_dimensions.1;
        let u = (gx as f32 + 0.5) / FOW_GRID_W as f32;
        let v = (gz as f32 + 0.5) / FOW_GRID_H as f32;
        ((u - 0.5) * map_w, (v - 0.5) * map_h)
    }

    #[inline]
    pub fn hero_eye_height(config: &GameConfig, stance: Stance, hero_scale: f32) -> f32 {
        let base = if stance == Stance::Crouch {
            config.map.fow_crouch_eye_height
        } else {
            config.map.fow_stand_eye_height
        };
        base * hero_scale
    }

    #[inline]
    pub fn hero_visible_height(config: &GameConfig, stance: Stance, hero_scale: f32) -> f32 {
        let base = if stance == Stance::Crouch {
            config.map.fow_crouch_visible_height
        } else {
            config.map.fow_stand_visible_height
        };
        base * hero_scale
    }

    /// محاسبه ارتفاع واقعی ناظر با تشخیص دقیق حضور در زیر پل (جلوگیری از پرش ارتفاع ناظر به عرشه پل بالایی)
    #[inline]
    pub fn effective_observer_y(&self, config: &GameConfig, origin: Vec3) -> (f32, bool) {
        let Some((gx, gz)) = Self::world_to_cell(config, origin.x, origin.z) else {
            return (origin.y, false);
        };
        let idx = gz * FOW_GRID_W + gx;
        let bridge_y = self.bridge_deck_y[idx];
        if bridge_y.is_finite()
            && origin.y <= bridge_y - config.map.high_ground_occlusion_min_diff
        {
            (origin.y.max(self.ground_terrain_y[idx]), true)
        } else {
            (origin.y.max(self.terrain_y[idx]), false)
        }
    }

    #[inline]
    pub fn is_world_point_visible(&self, config: &GameConfig, wx: f32, wz: f32) -> bool {
        if let Some((gx, gz)) = Self::world_to_cell(config, wx, wz) {
            self.visible_now[gz * FOW_GRID_W + gx]
        } else {
            false
        }
    }

    /// بررسی دید سه‌بعدی با تفکیک زیر پل/روی پل و سایه پروجکتیو سنگرهای کم‌ارتفاع (ایستاده در برابر نشسته)
    #[inline]
    pub fn is_world_entity_visible(
        &self,
        config: &GameConfig,
        feet_pos: Vec3,
        visible_height: f32,
    ) -> bool {
        let Some((gx, gz)) = Self::world_to_cell(config, feet_pos.x, feet_pos.z) else {
            return false;
        };
        let idx = gz * FOW_GRID_W + gx;
        let bridge_y = self.bridge_deck_y[idx];
        if bridge_y.is_finite()
            && feet_pos.y <= bridge_y - config.map.high_ground_occlusion_min_diff
        {
            return self.under_bridge_visible[idx];
        }
        if self.visible_now[idx] {
            return true;
        }
        let min_vis_y = self.min_visible_y[idx];
        min_vis_y.is_finite() && (feet_pos.y + visible_height) > min_vis_y
    }

    /// بررسی دید سه‌بعدی برای نقاط و پرتابه‌ها (گلوله، نارنجک، کوریر، افکت‌ها)
    #[inline]
    pub fn is_world_pos_visible(&self, config: &GameConfig, pos: Vec3) -> bool {
        self.is_world_entity_visible(config, pos, 0.0)
    }

    fn reveal_from_source(
        &mut self,
        config: &GameConfig,
        origin: Vec3,
        vision_range: f32,
        ray_count: usize,
    ) {
        self.reveal_from_source_with_eye(
            config,
            origin,
            config.map.fow_stand_eye_height,
            vision_range,
            ray_count,
        );
    }

    fn reveal_from_source_with_eye(
        &mut self,
        config: &GameConfig,
        origin: Vec3,
        eye_height: f32,
        vision_range: f32,
        ray_count: usize,
    ) {
        if vision_range <= 0.0 {
            return;
        }
        let (origin_y, _) = self.effective_observer_y(config, origin);
        let eye_y = origin_y + eye_height;

        let observer_in_pit = is_point_inside_roshan_pit(&config.map.roshan_pit, origin.x, origin.z);
        let step_size = config.map.fow_ray_step_size;
        let steps = (vision_range / step_size).ceil() as usize;
        let min_diff = config.map.high_ground_occlusion_min_diff;
        let blocker_min_dist = config.map.fow_blocker_min_dist;
        let ground_eps = config.map.fow_low_cover_ground_epsilon;

        // محاسبه تعداد اشعه بدون حفره بر اساس محیط دید و ابعاد سلول (حذف چشمک‌زدن دشمن در لبه دید سرباز/تاور/کوریر)
        let cell_w = config.map.map_dimensions.0 / FOW_GRID_W as f32;
        let cell_h = config.map.map_dimensions.1 / FOW_GRID_H as f32;
        let min_cell = cell_w.min(cell_h).max(0.25);
        let required_rays = ((std::f32::consts::TAU * vision_range) / (min_cell * 0.65)).ceil() as usize;
        let effective_ray_count = ray_count.max(required_rays).clamp(32, 360);

        for r_i in 0..effective_ray_count {
            let angle = (r_i as f32) * (std::f32::consts::TAU / effective_ray_count as f32);
            let dx = angle.cos();
            let dz = angle.sin();
            let mut max_slope = f32::NEG_INFINITY;

            for s_i in 0..=steps {
                let dist = s_i as f32 * step_size;
                if dist > vision_range {
                    break;
                }
                let wx = origin.x + dx * dist;
                let wz = origin.z + dz * dist;
                let cell_in_pit = is_point_inside_roshan_pit(&config.map.roshan_pit, wx, wz);
                if observer_in_pit != cell_in_pit {
                    break;
                }
                let Some((gx, gz)) = Self::world_to_cell(config, wx, wz) else {
                    break;
                };
                let idx = gz * FOW_GRID_W + gx;

                let bridge_y = self.bridge_deck_y[idx];
                let observer_below_bridge =
                    bridge_y.is_finite() && origin_y <= bridge_y - min_diff;

                if observer_below_bridge {
                    // ناظر در زیر پل است: فقط بستر زیر پل را می‌بیند و هرگز عرشه بالای پل را روشن نمی‌کند
                    let floor_y = self.ground_terrain_y[idx];
                    if floor_y - origin_y >= min_diff {
                        break;
                    }
                    self.under_bridge_visible[idx] = true;
                    if self.under_bridge_blocker_top_y[idx] >= origin_y + min_diff
                        && dist > blocker_min_dist
                    {
                        break;
                    }
                } else {
                    let cell_y = self.terrain_y[idx];
                    if cell_y - origin_y >= min_diff {
                        break;
                    }

                    // محاسبه خط دید پروجکتیو واقعی از روی سنگرهای کم‌ارتفاع (مثل sandbag.glb)
                    let cover_y = self.low_cover_top_y[idx];
                    let surface_y = if cover_y.is_finite() {
                        cell_y.max(cover_y)
                    } else {
                        cell_y
                    };
                    let shadow_y = if max_slope.is_finite() {
                        eye_y + max_slope * dist
                    } else {
                        f32::NEG_INFINITY
                    };

                    if shadow_y <= surface_y + ground_eps {
                        self.visible_now[idx] = true;
                        self.explored[idx] = true;
                        self.min_visible_y[idx] = self.min_visible_y[idx].min(cell_y);
                    } else {
                        // زمین پشت سنگر در سایه واقعی قرار دارد؛ تنها اهدافی که بالاتر از shadow_y هستند (مثلاً ایستاده) دیده می‌شوند
                        self.min_visible_y[idx] = self.min_visible_y[idx].min(shadow_y);
                    }

                    if cover_y.is_finite()
                        && dist >= step_size * 0.5
                        && cover_y > origin_y + ground_eps
                    {
                        let slope = (cover_y - eye_y) / dist;
                        if slope > max_slope {
                            max_slope = slope;
                        }
                    }

                    if self.blocker_top_y[idx] >= origin_y + min_diff && dist > blocker_min_dist {
                        break;
                    }
                }
            }
        }
    }

    pub fn update_for_hero(
        &mut self,
        config: &GameConfig,
        hero_pos: Vec3,
        hero: &HeroDef,
        hero_alive: bool,
        live_allied_structures: &[(Vec3, f32)],
        allied_creeps: &[Vec3],
        match_elapsed_secs: f32,
        weather: WeatherKind,
    ) {
        self.update_for_hero_with_stance(
            config,
            hero_pos,
            Stance::Run,
            hero,
            hero_alive,
            live_allied_structures,
            allied_creeps,
            match_elapsed_secs,
            weather,
        );
    }

    pub fn update_for_hero_with_stance(
        &mut self,
        config: &GameConfig,
        hero_pos: Vec3,
        hero_stance: Stance,
        hero: &HeroDef,
        hero_alive: bool,
        live_allied_structures: &[(Vec3, f32)],
        allied_creeps: &[Vec3],
        match_elapsed_secs: f32,
        weather: WeatherKind,
    ) {
        self.visible_now.fill(false);
        self.min_visible_y.fill(f32::INFINITY);
        self.under_bridge_visible.fill(false);

        let struct_mult = config.effective_vision_multiplier(
            match_elapsed_secs,
            config.day_night.night_turret_vision_multiplier,
        );
        let creep_mult = config.effective_vision_multiplier(
            match_elapsed_secs,
            config.day_night.night_creep_vision_multiplier,
        );

        let (hero_ground_y, hero_under_bridge) = self.effective_observer_y(config, hero_pos);

        let base_hero_vision = config.hero_effective_vision(hero, match_elapsed_secs, weather);
        let hero_range = base_hero_vision
            + if !hero_under_bridge && hero_ground_y >= config.map.high_ground_occlusion_min_diff {
                hero.high_ground_vision_bonus
            } else {
                0.0
            };

        if hero_alive {
            let hero_eye = Self::hero_eye_height(config, hero_stance, hero.hero_scale);
            self.reveal_from_source_with_eye(config, hero_pos, hero_eye, hero_range, 240);
        }

        for &(src_pos, src_range) in live_allied_structures {
            self.reveal_from_source_with_eye(
                config,
                src_pos,
                config.map.fow_stand_eye_height,
                src_range * struct_mult,
                96,
            );
        }
        let creep_range = config.creeps.vision_range * creep_mult;
        let creep_eye = config.creeps.hitbox_height * 0.85;
        for &c_pos in allied_creeps {
            self.reveal_from_source_with_eye(config, c_pos, creep_eye, creep_range, 96);
        }
    }

    pub fn pack_visible_mask(&self) -> Vec<u8> {
        let byte_len = self.visible_now.len().div_ceil(8);
        let mut mask = vec![0u8; byte_len];
        for (i, &vis) in self.visible_now.iter().enumerate() {
            if vis {
                mask[i / 8] |= 1 << (i % 8);
            }
        }
        mask
    }

    pub fn apply_visible_mask(&mut self, mask: &[u8]) {
        for (i, vis) in self.visible_now.iter_mut().enumerate() {
            let b = mask.get(i / 8).copied().unwrap_or(0);
            let is_vis = ((b >> (i % 8)) & 1) != 0;
            *vis = is_vis;
            if is_vis {
                self.explored[i] = true;
            }
        }
    }
}


// -----------------------------------------------------------------------------
// ۵. وضعیت کامل شبیه‌سازی (درون moba_shared برای جلوگیری از بازنویسی)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct LaneCreepState {
    pub id: u64,
    pub team: TeamId,
    pub creep_type_id: Option<String>,
    pub camp_index: Option<usize>,
    pub is_aggroed: bool,
    pub aggro_target_slot: Option<usize>,
    pub hero_aggro_target_slot: Option<usize>,
    pub hero_aggro_remaining_secs: f32,
    pub hero_aggro_cooldown_secs: f32,
    pub home_pos: Vec3,
    pub lane: LaneId,
    pub waypoint_index: usize,
    pub pos: Vec3,
    pub facing_dir: Vec3,
    pub hp: f32,
    pub max_hp: f32,
    pub fire_cooldown: f32,
    pub fire_anim_timer: f32,
    pub anim_state: CreepAnimState,
    pub is_dead: bool,
    pub despawn_timer: f32,
    pub hero_block_hesitation_timer: f32,
    pub hero_block_cooldown_timer: f32,
    pub hero_block_steer_side: f32,
}

#[derive(Clone, Debug)]
pub struct AncientState {
    pub index: usize,
    pub id: String,
    pub team: TeamId,
    pub pos: Vec3,
    pub height: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub armor: f32,
    pub is_dead: bool,
    pub is_invulnerable: bool,
}

#[derive(Clone, Debug)]
pub struct BarracksState {
    pub index: usize,
    pub id: String,
    pub team: TeamId,
    pub lane: LaneId,
    pub kind: BarracksKind,
    pub pos: Vec3,
    pub height: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub armor: f32,
    pub is_dead: bool,
    pub is_invulnerable: bool,
}

#[derive(Clone, Debug)]
pub struct TurretState {
    pub index: usize,
    pub id: String,
    pub team: TeamId,
    pub lane: LaneId,
    pub tier: u8,
    pub pos: Vec3,
    pub height: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub armor: f32,
    pub is_dead: bool,
    pub is_invulnerable: bool,
    pub fire_cooldown: f32,
    pub barrel_toggle: bool,
    pub aim_target_pos: Option<Vec3>,
}

pub struct SharedTarget {
    pub max_hp: f32,
    pub hp: f32,
    pub hit_flash_timer: f32,
    pub base_pos: Vec3,
    pub is_dead: bool,
    pub respawn_remaining: f32,
    pub respawn_duration: f32,
}

#[derive(Clone, Debug)]
pub struct SharedTraversalState {
    pub is_active: bool,
    pub kind: TraversalKind,
    pub anim_name: String,
    pub elapsed: f32,
    pub duration: f32,
    pub start_pos: Vec3,
    pub target_pos: Vec3,
    pub travel_dir: Vec3,
    pub vault_peak_lift: f32,
    pub chain_dive_after: bool,
    pub swing_arc_sag: f32,
    pub swing_total_duration: f32,
    pub swing_pulley_pos: Option<Vec3>,
    pub swing_throw_progress: f32,
    pub fall_vel: Vec3,
}

#[derive(Resource)]
pub struct ClientPhysicsPredictionState {
    pub latest_input: Option<ClientInputCommand>,
    pub traversal_active: bool,
    pub base_stance: Stance,
    pub effective_stance: Stance,
    pub is_dead: bool,
    pub match_ended: bool,
    pub is_paused: bool,
    pub hero_index: usize,
    pub latest_hero_scale: f32,
    pub latest_move_speed_multiplier: Option<f32>,
    pub latest_authoritative_feet_pos: Option<Vec3>,
}

impl ClientPhysicsPredictionState {
    pub fn new() -> Self {
        Self {
            latest_input: None,
            traversal_active: false,
            base_stance: Stance::Run,
            effective_stance: Stance::Run,
            is_dead: false,
            match_ended: false,
            is_paused: false,
            hero_index: 0,
            latest_hero_scale: 1.0,
            latest_move_speed_multiplier: None,
            latest_authoritative_feet_pos: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PlayerSlotState {
    pub slot_index: usize,
    pub client_id: Option<u64>,
    pub is_disconnected: bool,
    pub pause_cooldown_remaining: f32,
    pub active_input: Option<ClientInputCommand>,
    pub soldier_pos: Vec3,
    pub soldier_facing_dir: Vec3,
    pub spawn_pos: Vec3,
    pub base_stance: Stance,
    pub effective_stance: Stance,
    pub current_locomotion: String,
    pub aim_world_dir: Vec3,
    pub aim_hit_pos: Vec3,
    pub laser_hit_pos: Vec3,
    pub aim_locked_on_enemy: bool,
    pub hovered_creep_id: Option<u64>,
    pub hovered_turret_index: Option<usize>,
    pub hovered_barracks_index: Option<usize>,
    pub hovered_ancient_index: Option<usize>,
    pub is_moving: bool,
    pub move_input: Vec3,
    pub fire_cooldown_remaining: f32,
    pub firing_anim_remaining: Option<f32>,
    pub reload_remaining: Option<f32>,
    pub grenade_elapsed_and_duration: Option<(f32, f32)>,
    pub grenade_spawned_for_throw: bool,
    pub is_dead: bool,
    pub death_anim_name: String,
    pub respawn_remaining: f32,
    pub hero_model_path: String,
    pub hero_index: usize,
    pub hero_id: String,
    pub hero_display_name: String,
    pub hero_team: TeamId,
    pub hero_gold: u32,
    pub hero_level: u32,
    pub hero_xp: u32,
    pub hero_xp_gain_radius: f32,
    pub hero_hp: f32,
    pub hero_max_hp: f32,
    pub hero_hp_regen_per_sec: f32,
    pub hero_stamina: f32,
    pub hero_max_stamina: f32,
    pub hero_stamina_regen_per_sec: f32,
    pub hero_slide_stamina_cost: f32,
    pub hero_jump_over_stamina_cost: f32,
    pub stamina_exhausted: bool,
    pub space_hold_secs: Option<f32>,
    pub wall_climb_mountain_index: Option<usize>,
    pub wall_climb_last_dir: i8,
    pub hero_armor: f32,
    pub hero_scale: f32,
    pub hero_move_speed_multiplier: f32,
    pub toxic_slow_multiplier: f32,
    pub beam_charge: f32,
    pub electric_fire_rate_slow_remaining: f32,
    pub electric_fire_rate_slow_ratio: f32,
    pub flame_armor_debuff_remaining: f32,
    pub flame_armor_debuff_amount: f32,
    pub active_beam_request: Option<(Vec3, Vec3, f32, f32, f32, f32, f32, bool)>,
    pub hero_shallow_water_speed_multiplier: f32,
    pub hero_deep_water_speed_multiplier: f32,
    pub hero_vision_range: f32,
    pub active_is_night: bool,
    pub active_weather: WeatherKind,
    pub hero_weapon: HeroWeaponDef,
    pub hero_pistol: HeroWeaponDef,
    pub hero_grenade: HeroGrenadeDef,
    pub grenade_charges: u32,
    pub grenade_recharge_remaining: f32,
    pub active_weapon: ActiveWeaponSlot,
    pub other_weapon_ammo_in_mag: u32,
    pub ammo_in_mag: u32,
    pub current_move_speed: f32,
    pub last_hits: u32,
    pub denies: u32,
    pub assists: u32,
    pub kills: u32,
    pub deaths: u32,
    pub kill_streak: u32,
    pub inventory: Vec<InventoryItemState>,
    pub tp_slot: Option<InventoryItemState>,
    pub courier_pos: Vec3,
    pub courier_facing_dir: Vec3,
    pub courier_hp: f32,
    pub courier_max_hp: f32,
    pub courier_is_dead: bool,
    pub courier_respawn_remaining: f32,
    pub courier_order: CourierOrder,
    pub courier_target_pos: Vec3,
    pub courier_inventory: Vec<InventoryItemState>,
    pub stash: Vec<InventoryItemState>,
    pub shop_notice_seq: u64,
    pub shop_notice_text: String,
    pub shop_notice_is_error: bool,
    pub pending_hero_to_courier_slot: Option<usize>,
    pub fog_of_war: SharedFogOfWar,
    pub in_water: bool,
    pub water_splash_cooldown: f32,
    pub teleport_cooldown_remaining: f32,
    pub active_teleport: Option<ActiveTeleportState>,
    pub mode_cycle_cooldown_remaining: f32,
    pub shadow_blade_cooldown_remaining: f32,
    pub invisibility_remaining: f32,
    pub invisibility_total_secs: f32,
    pub invis_move_speed_bonus_pct: f32,
    pub is_revealed_by_turret: bool,
    pub hero_evasion_chance: f32,
    pub hero_true_strike_chance: f32,
    pub hero_crit_chance: f32,
    pub hero_crit_multiplier: f32,
    pub hero_lifesteal_ratio: f32,
    pub hero_on_hit_slow_ratio: f32,
    pub hero_on_hit_heal_reduction_ratio: f32,
    pub hero_on_hit_debuff_duration_secs: f32,
    pub reflect_armor_cooldown_remaining: f32,
    pub bullet_immunity_cooldown_remaining: f32,
    pub null_pulse_cooldown_remaining: f32,
    pub sonar_reveal_cooldown_remaining: f32,
    pub kinetic_dash_cooldown_remaining: f32,
    pub has_reflect_armor_passive: bool,
    pub reflect_armor_remaining: f32,
    pub reflect_armor_total_secs: f32,
    pub bullet_immunity_remaining: f32,
    pub bullet_immunity_total_secs: f32,
    pub cryo_debuff_remaining: f32,
    pub cryo_debuff_total_secs: f32,
    pub cryo_slow_ratio: f32,
    pub cryo_heal_reduction_ratio: f32,
    pub null_mute_remaining: f32,
    pub null_mute_total_secs: f32,
    pub sonar_revealed_remaining: f32,
    pub sonar_revealed_total_secs: f32,
    pub kinetic_dash_remaining: f32,
    pub kinetic_dash_dir: Vec3,
    pub popup_throttle_remaining: f32,
    pub recent_hero_damagers: Vec<(usize, f32)>,
    pub last_attacked_enemy_hero_at: f32,
    pub attack_reveal_to_dawn_remaining: f32,
    pub attack_reveal_to_dusk_remaining: f32,
    pub attack_reveal_to_neutral_remaining: f32,
    pub last_ping_at_secs: f32,
    pub last_chat_at_secs: f32,
    pub traversal: SharedTraversalState,
}

#[derive(Resource)]
pub struct AuthoritativeWorldState {
    pub tick: u64,
    pub rng_seed: u32,
    pub next_entity_id: u64,
    pub active_slot: usize,
    pub active_spawn_pos: Vec3,
    pub hero_model_path: String,
    pub players: Vec<PlayerSlotState>,
    pub active_input: Option<ClientInputCommand>,
    pub soldier_pos: Vec3,
    pub soldier_facing_dir: Vec3,
    pub base_stance: Stance,
    pub effective_stance: Stance,
    pub current_locomotion: String,
    pub aim_world_dir: Vec3,
    pub aim_hit_pos: Vec3,
    pub laser_hit_pos: Vec3,
    pub aim_locked_on_enemy: bool,
    pub hovered_creep_id: Option<u64>,
    pub hovered_turret_index: Option<usize>,
    pub hovered_barracks_index: Option<usize>,
    pub hovered_ancient_index: Option<usize>,
    pub is_moving: bool,
    pub move_input: Vec3,
    pub fire_cooldown_remaining: f32,
    pub firing_anim_remaining: Option<f32>,
    pub reload_remaining: Option<f32>,
    pub grenade_elapsed_and_duration: Option<(f32, f32)>,
    pub grenade_spawned_for_throw: bool,
    pub is_dead: bool,
    pub death_anim_name: String,
    pub respawn_remaining: f32,
    pub hero_index: usize,
    pub hero_id: String,
    pub hero_display_name: String,
    pub hero_team: TeamId,
    pub hero_gold: u32,
    pub hero_level: u32,
    pub hero_xp: u32,
    pub hero_xp_gain_radius: f32,
    pub hero_hp: f32,
    pub hero_max_hp: f32,
    pub hero_hp_regen_per_sec: f32,
    pub hero_stamina: f32,
    pub hero_max_stamina: f32,
    pub hero_stamina_regen_per_sec: f32,
    pub hero_slide_stamina_cost: f32,
    pub hero_jump_over_stamina_cost: f32,
    pub stamina_exhausted: bool,
    pub space_hold_secs: Option<f32>,
    pub wall_climb_mountain_index: Option<usize>,
    pub wall_climb_last_dir: i8,
    pub hero_armor: f32,
    pub hero_scale: f32,
    pub hero_move_speed_multiplier: f32,
    pub toxic_slow_multiplier: f32,
    pub beam_charge: f32,
    pub electric_fire_rate_slow_remaining: f32,
    pub electric_fire_rate_slow_ratio: f32,
    pub flame_armor_debuff_remaining: f32,
    pub flame_armor_debuff_amount: f32,
    pub active_beam_request: Option<(Vec3, Vec3, f32, f32, f32, f32, f32, bool)>,
    pub hero_shallow_water_speed_multiplier: f32,
    pub hero_deep_water_speed_multiplier: f32,
    pub hero_vision_range: f32,
    pub hero_weapon: HeroWeaponDef,
    pub hero_pistol: HeroWeaponDef,
    pub hero_grenade: HeroGrenadeDef,
    pub grenade_charges: u32,
    pub grenade_recharge_remaining: f32,
    pub active_weapon: ActiveWeaponSlot,
    pub other_weapon_ammo_in_mag: u32,
    pub ammo_in_mag: u32,
    pub current_move_speed: f32,
    pub last_hits: u32,
    pub denies: u32,
    pub assists: u32,
    pub kills: u32,
    pub deaths: u32,
    pub kill_streak: u32,
    pub inventory: Vec<InventoryItemState>,
    pub tp_slot: Option<InventoryItemState>,
    pub fog_of_war: SharedFogOfWar,
    pub in_water: bool,
    pub water_splash_cooldown: f32,
    pub teleport_cooldown_remaining: f32,
    pub active_teleport: Option<ActiveTeleportState>,
    pub mode_cycle_cooldown_remaining: f32,
    pub shadow_blade_cooldown_remaining: f32,
    pub invisibility_remaining: f32,
    pub invisibility_total_secs: f32,
    pub invis_move_speed_bonus_pct: f32,
    pub is_revealed_by_turret: bool,
    pub hero_evasion_chance: f32,
    pub hero_true_strike_chance: f32,
    pub hero_crit_chance: f32,
    pub hero_crit_multiplier: f32,
    pub hero_lifesteal_ratio: f32,
    pub hero_on_hit_slow_ratio: f32,
    pub hero_on_hit_heal_reduction_ratio: f32,
    pub hero_on_hit_debuff_duration_secs: f32,
    pub reflect_armor_cooldown_remaining: f32,
    pub bullet_immunity_cooldown_remaining: f32,
    pub null_pulse_cooldown_remaining: f32,
    pub sonar_reveal_cooldown_remaining: f32,
    pub kinetic_dash_cooldown_remaining: f32,
    pub has_reflect_armor_passive: bool,
    pub reflect_armor_remaining: f32,
    pub reflect_armor_total_secs: f32,
    pub bullet_immunity_remaining: f32,
    pub bullet_immunity_total_secs: f32,
    pub cryo_debuff_remaining: f32,
    pub cryo_debuff_total_secs: f32,
    pub cryo_slow_ratio: f32,
    pub cryo_heal_reduction_ratio: f32,
    pub null_mute_remaining: f32,
    pub null_mute_total_secs: f32,
    pub sonar_revealed_remaining: f32,
    pub sonar_revealed_total_secs: f32,
    pub kinetic_dash_remaining: f32,
    pub kinetic_dash_dir: Vec3,
    pub popup_throttle_remaining: f32,
    pub traversal: SharedTraversalState,
    pub targets: Vec<SharedTarget>,
    pub targets_visible: Vec<bool>,
    pub creeps: Vec<LaneCreepState>,
    pub turrets: Vec<TurretState>,
    pub fountain_fire_cooldowns: Vec<f32>,
    pub allow_shadow_blade_during_teleport: bool,
    pub demo_reveal_full_map: bool,
    pub dawn_los_visible_slots: Vec<bool>,
    pub dusk_los_visible_slots: Vec<bool>,
    pub dawn_los_visible_creeps: std::collections::HashSet<u64>,
    pub dusk_los_visible_creeps: std::collections::HashSet<u64>,
    pub dawn_los_visible_couriers: Vec<bool>,
    pub dusk_los_visible_couriers: Vec<bool>,
    pub barracks: Vec<BarracksState>,
    pub ancients: Vec<AncientState>,
    pub chat_history: Vec<ChatMessageEntry>,
    pub next_chat_id: u64,
    pub ping_history: Vec<PingEntry>,
    pub next_ping_id: u64,
    pub pause_phase: PausePhase,
    pub paused_by_team: Option<TeamId>,
    pub paused_by_name: String,
    pub pause_is_disconnect: bool,
    pub pause_lock_remaining: f32,
    pub unpause_countdown_remaining: f32,
    pub dawn_pauses_remaining: u32,
    pub dusk_pauses_remaining: u32,
    pub dawn_time_bank_secs: f32,
    pub dusk_time_bank_secs: f32,
    pub winning_team: Option<TeamId>,
    pub match_elapsed_secs: f32,
    pub passive_gold_accumulator_secs: f32,
    pub current_weather: WeatherKind,
    pub weather_remaining_secs: f32,
    pub lightning_cooldown_secs: f32,
    pub last_env_is_night: bool,
    pub last_env_weather: WeatherKind,
    pub env_stats_initialized: bool,
    pub next_wave_time_secs: f32,
    pub next_camp_spawn_time_secs: f32,
    pub next_creep_id: u64,
    pub bullets_snapshot: Vec<BulletSnapshot>,
    pub grenades_snapshot: Vec<GrenadeSnapshot>,
    pub toxic_pools: Vec<ActiveToxicPool>,
    pub toxic_pools_snapshot: Vec<ToxicPoolSnapshot>,
    pub electric_beams_snapshot: Vec<ElectricBeamSnapshot>,
    pub pending_fx: Vec<GameplayFxEvent>,
    /// رویدادهای خصوصی که فقط برای اسلات مشخص‌شده ارسال می‌شوند (LastHitGold، HeroLeveledUp)
    pub pending_private_fx: Vec<(usize, GameplayFxEvent)>,
}

#[derive(Clone, Debug)]
pub struct ActiveToxicPool {
    pub id: u64,
    pub pos: Vec3,
    pub radius: f32,
    pub remaining_secs: f32,
    pub total_duration: f32,
    pub dps: f32,
    pub slow_ratio: f32,
    pub min_slow_mult: f32,
    pub vertical_tolerance: f32,
    pub owner_slot: Option<usize>,
    pub owner_team: TeamId,
}

impl AuthoritativeWorldState {
    pub fn new_from_config(config: &GameConfig) -> Self {
        let mut targets = Vec::new();
        for t in &config.targets {
            let pos = t.model_transform.location_vec();
            targets.push(SharedTarget {
                max_hp: t.max_hp,
                hp: t.max_hp,
                hit_flash_timer: 0.0,
                base_pos: pos,
                is_dead: false,
                respawn_remaining: 0.0,
                respawn_duration: t.respawn_secs,
            });
        }

        let spawn_pos = Vec3::new(
            config.soldier.spawn_pos.0,
            config.soldier.spawn_pos.1,
            config.soldier.spawn_pos.2,
        );
        let initial_aim_dir = Vec3::new(
            config.soldier.initial_aim_dir.0,
            config.soldier.initial_aim_dir.1,
            config.soldier.initial_aim_dir.2,
        )
        .normalize();
        let hero = config.active_hero(0).clone();
        let fog_of_war = SharedFogOfWar::from_config(config, hero.team);
        let initial_aim_hit = Vec3::new(
            config.soldier.initial_aim_hit_pos.0,
            config.soldier.initial_aim_hit_pos.1,
            config.soldier.initial_aim_hit_pos.2,
        );

        Self {
            tick: 0,
            rng_seed: 987654321,
            next_entity_id: 1,
            active_input: None,
            soldier_pos: spawn_pos,
            soldier_facing_dir: initial_aim_dir,
            base_stance: Stance::Run,
            effective_stance: Stance::Run,
            current_locomotion: "idle".to_string(),
            aim_world_dir: initial_aim_dir,
            aim_hit_pos: initial_aim_hit,
            laser_hit_pos: initial_aim_hit,
            aim_locked_on_enemy: false,
            hovered_creep_id: None,
            hovered_turret_index: None,
            hovered_barracks_index: None,
            hovered_ancient_index: None,
            is_moving: false,
            move_input: Vec3::ZERO,
            fire_cooldown_remaining: 0.0,
            firing_anim_remaining: None,
            reload_remaining: None,
            grenade_elapsed_and_duration: None,
            grenade_spawned_for_throw: false,
            is_dead: false,
            death_anim_name: String::new(),
            respawn_remaining: 0.0,
            active_slot: 0,
            active_spawn_pos: Vec3::new(config.soldier.spawn_pos.0, config.soldier.spawn_pos.1, config.soldier.spawn_pos.2),
            hero_model_path: hero.model_path.clone(),
            players: Vec::new(),
            hero_index: 0,
            hero_id: hero.id,
            hero_display_name: hero.display_name,
            hero_team: hero.team,
            hero_gold: hero.starting_gold,
            hero_level: 1,
            hero_xp: 0,
            hero_xp_gain_radius: hero.xp_gain_radius,
            hero_hp: hero.max_hp,
            hero_max_hp: hero.max_hp,
            hero_hp_regen_per_sec: hero.hp_regen_per_sec,
            hero_stamina: hero.max_stamina,
            hero_max_stamina: hero.max_stamina,
            hero_stamina_regen_per_sec: hero.stamina_regen_per_sec,
            hero_slide_stamina_cost: hero.slide_stamina_cost,
            hero_jump_over_stamina_cost: hero.jump_over_stamina_cost,
            stamina_exhausted: false,
            space_hold_secs: None,
            wall_climb_mountain_index: None,
            wall_climb_last_dir: 1,
            hero_armor: hero.armor,
            hero_scale: hero.hero_scale,
            hero_move_speed_multiplier: hero.move_speed_multiplier,
            toxic_slow_multiplier: 1.0,
            beam_charge: hero.weapon.beam_max_charge,
            electric_fire_rate_slow_remaining: 0.0,
            electric_fire_rate_slow_ratio: 0.0,
            flame_armor_debuff_remaining: 0.0,
            flame_armor_debuff_amount: 0.0,
            active_beam_request: None,
            hero_shallow_water_speed_multiplier: hero.shallow_water_speed_multiplier,
            hero_deep_water_speed_multiplier: hero.deep_water_speed_multiplier,
            hero_vision_range: hero.vision_range,
            ammo_in_mag: hero.weapon.magazine_size,
            hero_weapon: hero.weapon,
            hero_pistol: hero.pistol,
            hero_grenade: hero.grenade,
            grenade_charges: hero.grenade.max_charges,
            grenade_recharge_remaining: 0.0,
            active_weapon: ActiveWeaponSlot::Primary,
            other_weapon_ammo_in_mag: hero.pistol.magazine_size,
            current_move_speed: config.soldier.run_speed * hero.move_speed_multiplier,
            last_hits: 0,
            denies: 0,
            assists: 0,
            kills: 0,
            deaths: 0,
            kill_streak: 0,
            inventory: Vec::new(),
            tp_slot: None,
            fog_of_war,
            in_water: false,
            water_splash_cooldown: 0.0,
            teleport_cooldown_remaining: 0.0,
            active_teleport: None,
            mode_cycle_cooldown_remaining: 0.0,
            shadow_blade_cooldown_remaining: 0.0,
            invisibility_remaining: 0.0,
            invisibility_total_secs: 0.0,
            invis_move_speed_bonus_pct: 0.0,
            is_revealed_by_turret: false,
            hero_evasion_chance: 0.0,
            hero_true_strike_chance: 0.0,
            hero_crit_chance: 0.0,
            hero_crit_multiplier: 1.0,
            hero_lifesteal_ratio: 0.0,
            hero_on_hit_slow_ratio: 0.0,
            hero_on_hit_heal_reduction_ratio: 0.0,
            hero_on_hit_debuff_duration_secs: 0.0,
            reflect_armor_cooldown_remaining: 0.0,
            bullet_immunity_cooldown_remaining: 0.0,
            null_pulse_cooldown_remaining: 0.0,
            sonar_reveal_cooldown_remaining: 0.0,
            kinetic_dash_cooldown_remaining: 0.0,
            has_reflect_armor_passive: false,
            reflect_armor_remaining: 0.0,
            reflect_armor_total_secs: 0.0,
            bullet_immunity_remaining: 0.0,
            bullet_immunity_total_secs: 0.0,
            cryo_debuff_remaining: 0.0,
            cryo_debuff_total_secs: 0.0,
            cryo_slow_ratio: 0.0,
            cryo_heal_reduction_ratio: 0.0,
            null_mute_remaining: 0.0,
            null_mute_total_secs: 0.0,
            sonar_revealed_remaining: 0.0,
            sonar_revealed_total_secs: 0.0,
            kinetic_dash_remaining: 0.0,
            kinetic_dash_dir: Vec3::ZERO,
            popup_throttle_remaining: 0.0,
            traversal: SharedTraversalState {
                is_active: false,
                kind: TraversalKind::RunningJump,
                anim_name: String::new(),
                elapsed: 0.0,
                duration: 0.0,
                start_pos: spawn_pos,
                target_pos: spawn_pos,
                travel_dir: initial_aim_dir,
                vault_peak_lift: 0.0,
                chain_dive_after: false,
                swing_arc_sag: 0.0,
                swing_total_duration: 0.0,
                swing_pulley_pos: None,
                swing_throw_progress: 0.0,
                fall_vel: Vec3::ZERO,
            },
            targets_visible: vec![true; targets.len()],
            targets,
            creeps: Vec::new(),
            fountain_fire_cooldowns: vec![0.0; config.map.fountains.len()],
            allow_shadow_blade_during_teleport: config.gameplay_rules.allow_shadow_blade_during_teleport,
            demo_reveal_full_map: false,
            dawn_los_visible_slots: Vec::new(),
            dusk_los_visible_slots: Vec::new(),
            dawn_los_visible_creeps: std::collections::HashSet::new(),
            dusk_los_visible_creeps: std::collections::HashSet::new(),
            dawn_los_visible_couriers: Vec::new(),
            dusk_los_visible_couriers: Vec::new(),
            turrets: config
                .map
                .turrets
                .iter()
                .enumerate()
                .map(|(index, t)| TurretState {
                    index,
                    id: t.id.clone(),
                    team: t.team,
                    lane: t.lane,
                    tier: t.tier,
                    pos: t.model_transform.location_vec(),
                    height: t.height * t.model_transform.scale_vec().y,
                    hp: t.max_hp,
                    max_hp: t.max_hp,
                    armor: t.armor,
                    is_dead: false,
                    is_invulnerable: t.tier > 1,
                    fire_cooldown: 0.0,
                    barrel_toggle: false,
                    aim_target_pos: None,
                })
                .collect(),
            barracks: config
                .map
                .barracks
                .iter()
                .enumerate()
                .map(|(index, b)| BarracksState {
                    index,
                    id: b.id.clone(),
                    team: b.team,
                    lane: b.lane,
                    kind: b.kind,
                    pos: b.model_transform.location_vec(),
                    height: b.size.1 * b.model_transform.scale_vec().y,
                    hp: b.max_hp,
                    max_hp: b.max_hp,
                    armor: b.armor,
                    is_dead: false,
                    is_invulnerable: true,
                })
                .collect(),
            ancients: config
                .map
                .ancients
                .iter()
                .enumerate()
                .map(|(index, a)| AncientState {
                    index,
                    id: a.id.clone(),
                    team: a.team,
                    pos: a.model_transform.location_vec(),
                    height: (a.base_size.1 + a.core_height) * a.model_transform.scale_vec().y,
                    hp: a.max_hp,
                    max_hp: a.max_hp,
                    armor: a.armor,
                    is_dead: false,
                    is_invulnerable: true,
                })
                .collect(),
            chat_history: Vec::new(),
            next_chat_id: 1,
            ping_history: Vec::new(),
            next_ping_id: 1,
            pause_phase: PausePhase::Running,
            paused_by_team: None,
            paused_by_name: String::new(),
            pause_is_disconnect: false,
            pause_lock_remaining: 0.0,
            unpause_countdown_remaining: 0.0,
            dawn_pauses_remaining: config.pause.max_pauses_per_team,
            dusk_pauses_remaining: config.pause.max_pauses_per_team,
            dawn_time_bank_secs: config.pause.team_time_bank_secs,
            dusk_time_bank_secs: config.pause.team_time_bank_secs,
            winning_team: None,
            match_elapsed_secs: 0.0,
            passive_gold_accumulator_secs: 0.0,
            current_weather: config.weather.initial_weather,
            weather_remaining_secs: config.weather_duration_bounds(config.weather.initial_weather).0,
            lightning_cooldown_secs: config.weather.lightning_min_interval_secs,
            last_env_is_night: false,
            last_env_weather: config.weather.initial_weather,
            env_stats_initialized: false,
            next_wave_time_secs: config.creeps.first_wave_delay_secs,
            next_camp_spawn_time_secs: config.neutral_camps.first_spawn_delay_secs,
            next_creep_id: 1,
            bullets_snapshot: Vec::new(),
            grenades_snapshot: Vec::new(),
            toxic_pools: Vec::new(),
            toxic_pools_snapshot: Vec::new(),
            electric_beams_snapshot: Vec::new(),
            pending_fx: Vec::new(),
            pending_private_fx: Vec::new(),
        }
    }

    pub fn next_rand(&mut self) -> f32 {
        self.rng_seed = self
            .rng_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.rng_seed as f32) / (u32::MAX as f32)
    }

    pub fn upper_body_action(&self) -> UpperBodyAction {
        if self.grenade_elapsed_and_duration.is_some() {
            UpperBodyAction::TossGrenade
        } else if self.reload_remaining.is_some() {
            UpperBodyAction::Reloading
        } else if self.firing_anim_remaining.is_some() {
            UpperBodyAction::FiringRifle
        } else {
            UpperBodyAction::None
        }
    }

    pub fn swap_slot_in(&mut self, idx: usize) {
        let p = &mut self.players[idx];
        std::mem::swap(&mut self.active_input, &mut p.active_input);
        std::mem::swap(&mut self.soldier_pos, &mut p.soldier_pos);
        std::mem::swap(&mut self.soldier_facing_dir, &mut p.soldier_facing_dir);
        std::mem::swap(&mut self.active_spawn_pos, &mut p.spawn_pos);
        std::mem::swap(&mut self.base_stance, &mut p.base_stance);
        std::mem::swap(&mut self.effective_stance, &mut p.effective_stance);
        std::mem::swap(&mut self.current_locomotion, &mut p.current_locomotion);
        std::mem::swap(&mut self.aim_world_dir, &mut p.aim_world_dir);
        std::mem::swap(&mut self.aim_hit_pos, &mut p.aim_hit_pos);
        std::mem::swap(&mut self.laser_hit_pos, &mut p.laser_hit_pos);
        std::mem::swap(&mut self.aim_locked_on_enemy, &mut p.aim_locked_on_enemy);
        std::mem::swap(&mut self.hovered_creep_id, &mut p.hovered_creep_id);
        std::mem::swap(&mut self.hovered_turret_index, &mut p.hovered_turret_index);
        std::mem::swap(&mut self.hovered_barracks_index, &mut p.hovered_barracks_index);
        std::mem::swap(&mut self.hovered_ancient_index, &mut p.hovered_ancient_index);
        std::mem::swap(&mut self.is_moving, &mut p.is_moving);
        std::mem::swap(&mut self.move_input, &mut p.move_input);
        std::mem::swap(&mut self.fire_cooldown_remaining, &mut p.fire_cooldown_remaining);
        std::mem::swap(&mut self.firing_anim_remaining, &mut p.firing_anim_remaining);
        std::mem::swap(&mut self.reload_remaining, &mut p.reload_remaining);
        std::mem::swap(&mut self.grenade_elapsed_and_duration, &mut p.grenade_elapsed_and_duration);
        std::mem::swap(&mut self.grenade_spawned_for_throw, &mut p.grenade_spawned_for_throw);
        std::mem::swap(&mut self.is_dead, &mut p.is_dead);
        std::mem::swap(&mut self.death_anim_name, &mut p.death_anim_name);
        std::mem::swap(&mut self.respawn_remaining, &mut p.respawn_remaining);
        std::mem::swap(&mut self.hero_index, &mut p.hero_index);
        std::mem::swap(&mut self.hero_id, &mut p.hero_id);
        std::mem::swap(&mut self.hero_display_name, &mut p.hero_display_name);
        std::mem::swap(&mut self.hero_model_path, &mut p.hero_model_path);
        std::mem::swap(&mut self.hero_team, &mut p.hero_team);
        std::mem::swap(&mut self.hero_gold, &mut p.hero_gold);
        std::mem::swap(&mut self.hero_level, &mut p.hero_level);
        std::mem::swap(&mut self.hero_xp, &mut p.hero_xp);
        std::mem::swap(&mut self.hero_xp_gain_radius, &mut p.hero_xp_gain_radius);
        std::mem::swap(&mut self.hero_hp, &mut p.hero_hp);
        std::mem::swap(&mut self.hero_max_hp, &mut p.hero_max_hp);
        std::mem::swap(&mut self.hero_hp_regen_per_sec, &mut p.hero_hp_regen_per_sec);
        std::mem::swap(&mut self.hero_stamina, &mut p.hero_stamina);
        std::mem::swap(&mut self.hero_max_stamina, &mut p.hero_max_stamina);
        std::mem::swap(&mut self.hero_stamina_regen_per_sec, &mut p.hero_stamina_regen_per_sec);
        std::mem::swap(&mut self.hero_slide_stamina_cost, &mut p.hero_slide_stamina_cost);
        std::mem::swap(&mut self.hero_jump_over_stamina_cost, &mut p.hero_jump_over_stamina_cost);
        std::mem::swap(&mut self.stamina_exhausted, &mut p.stamina_exhausted);
        std::mem::swap(&mut self.space_hold_secs, &mut p.space_hold_secs);
        std::mem::swap(&mut self.wall_climb_mountain_index, &mut p.wall_climb_mountain_index);
        std::mem::swap(&mut self.wall_climb_last_dir, &mut p.wall_climb_last_dir);
        std::mem::swap(&mut self.hero_armor, &mut p.hero_armor);
        std::mem::swap(&mut self.hero_scale, &mut p.hero_scale);
        std::mem::swap(&mut self.hero_move_speed_multiplier, &mut p.hero_move_speed_multiplier);
        std::mem::swap(&mut self.toxic_slow_multiplier, &mut p.toxic_slow_multiplier);
        std::mem::swap(&mut self.beam_charge, &mut p.beam_charge);
        std::mem::swap(&mut self.electric_fire_rate_slow_remaining, &mut p.electric_fire_rate_slow_remaining);
        std::mem::swap(&mut self.electric_fire_rate_slow_ratio, &mut p.electric_fire_rate_slow_ratio);
        std::mem::swap(&mut self.flame_armor_debuff_remaining, &mut p.flame_armor_debuff_remaining);
        std::mem::swap(&mut self.flame_armor_debuff_amount, &mut p.flame_armor_debuff_amount);
        std::mem::swap(&mut self.active_beam_request, &mut p.active_beam_request);
        std::mem::swap(&mut self.hero_shallow_water_speed_multiplier, &mut p.hero_shallow_water_speed_multiplier);
        std::mem::swap(&mut self.hero_deep_water_speed_multiplier, &mut p.hero_deep_water_speed_multiplier);
        std::mem::swap(&mut self.hero_vision_range, &mut p.hero_vision_range);
        std::mem::swap(&mut self.hero_weapon, &mut p.hero_weapon);
        std::mem::swap(&mut self.hero_pistol, &mut p.hero_pistol);
        std::mem::swap(&mut self.hero_grenade, &mut p.hero_grenade);
        std::mem::swap(&mut self.grenade_charges, &mut p.grenade_charges);
        std::mem::swap(&mut self.grenade_recharge_remaining, &mut p.grenade_recharge_remaining);
        std::mem::swap(&mut self.active_weapon, &mut p.active_weapon);
        std::mem::swap(&mut self.other_weapon_ammo_in_mag, &mut p.other_weapon_ammo_in_mag);
        std::mem::swap(&mut self.ammo_in_mag, &mut p.ammo_in_mag);
        std::mem::swap(&mut self.current_move_speed, &mut p.current_move_speed);
        std::mem::swap(&mut self.last_hits, &mut p.last_hits);
        std::mem::swap(&mut self.denies, &mut p.denies);
        std::mem::swap(&mut self.assists, &mut p.assists);
        std::mem::swap(&mut self.kills, &mut p.kills);
        std::mem::swap(&mut self.deaths, &mut p.deaths);
        std::mem::swap(&mut self.kill_streak, &mut p.kill_streak);
        std::mem::swap(&mut self.inventory, &mut p.inventory);
        std::mem::swap(&mut self.tp_slot, &mut p.tp_slot);
        std::mem::swap(&mut self.fog_of_war, &mut p.fog_of_war);
        std::mem::swap(&mut self.in_water, &mut p.in_water);
        std::mem::swap(&mut self.water_splash_cooldown, &mut p.water_splash_cooldown);
        std::mem::swap(&mut self.teleport_cooldown_remaining, &mut p.teleport_cooldown_remaining);
        std::mem::swap(&mut self.active_teleport, &mut p.active_teleport);
        std::mem::swap(&mut self.mode_cycle_cooldown_remaining, &mut p.mode_cycle_cooldown_remaining);
        std::mem::swap(&mut self.shadow_blade_cooldown_remaining, &mut p.shadow_blade_cooldown_remaining);
        std::mem::swap(&mut self.invisibility_remaining, &mut p.invisibility_remaining);
        std::mem::swap(&mut self.invisibility_total_secs, &mut p.invisibility_total_secs);
        std::mem::swap(&mut self.invis_move_speed_bonus_pct, &mut p.invis_move_speed_bonus_pct);
        std::mem::swap(&mut self.is_revealed_by_turret, &mut p.is_revealed_by_turret);
        std::mem::swap(&mut self.hero_evasion_chance, &mut p.hero_evasion_chance);
        std::mem::swap(&mut self.hero_true_strike_chance, &mut p.hero_true_strike_chance);
        std::mem::swap(&mut self.hero_crit_chance, &mut p.hero_crit_chance);
        std::mem::swap(&mut self.hero_crit_multiplier, &mut p.hero_crit_multiplier);
        std::mem::swap(&mut self.hero_lifesteal_ratio, &mut p.hero_lifesteal_ratio);
        std::mem::swap(&mut self.hero_on_hit_slow_ratio, &mut p.hero_on_hit_slow_ratio);
        std::mem::swap(&mut self.hero_on_hit_heal_reduction_ratio, &mut p.hero_on_hit_heal_reduction_ratio);
        std::mem::swap(&mut self.hero_on_hit_debuff_duration_secs, &mut p.hero_on_hit_debuff_duration_secs);
        std::mem::swap(&mut self.reflect_armor_cooldown_remaining, &mut p.reflect_armor_cooldown_remaining);
        std::mem::swap(&mut self.bullet_immunity_cooldown_remaining, &mut p.bullet_immunity_cooldown_remaining);
        std::mem::swap(&mut self.null_pulse_cooldown_remaining, &mut p.null_pulse_cooldown_remaining);
        std::mem::swap(&mut self.sonar_reveal_cooldown_remaining, &mut p.sonar_reveal_cooldown_remaining);
        std::mem::swap(&mut self.kinetic_dash_cooldown_remaining, &mut p.kinetic_dash_cooldown_remaining);
        std::mem::swap(&mut self.has_reflect_armor_passive, &mut p.has_reflect_armor_passive);
        std::mem::swap(&mut self.reflect_armor_remaining, &mut p.reflect_armor_remaining);
        std::mem::swap(&mut self.reflect_armor_total_secs, &mut p.reflect_armor_total_secs);
        std::mem::swap(&mut self.bullet_immunity_remaining, &mut p.bullet_immunity_remaining);
        std::mem::swap(&mut self.bullet_immunity_total_secs, &mut p.bullet_immunity_total_secs);
        std::mem::swap(&mut self.cryo_debuff_remaining, &mut p.cryo_debuff_remaining);
        std::mem::swap(&mut self.cryo_debuff_total_secs, &mut p.cryo_debuff_total_secs);
        std::mem::swap(&mut self.cryo_slow_ratio, &mut p.cryo_slow_ratio);
        std::mem::swap(&mut self.cryo_heal_reduction_ratio, &mut p.cryo_heal_reduction_ratio);
        std::mem::swap(&mut self.null_mute_remaining, &mut p.null_mute_remaining);
        std::mem::swap(&mut self.null_mute_total_secs, &mut p.null_mute_total_secs);
        std::mem::swap(&mut self.sonar_revealed_remaining, &mut p.sonar_revealed_remaining);
        std::mem::swap(&mut self.sonar_revealed_total_secs, &mut p.sonar_revealed_total_secs);
        std::mem::swap(&mut self.kinetic_dash_remaining, &mut p.kinetic_dash_remaining);
        std::mem::swap(&mut self.kinetic_dash_dir, &mut p.kinetic_dash_dir);
        std::mem::swap(&mut self.popup_throttle_remaining, &mut p.popup_throttle_remaining);
        std::mem::swap(&mut self.traversal, &mut p.traversal);
        self.active_slot = idx;
    }

    pub fn can_slot_unpause(&self, slot_index: usize) -> bool {
        if self.pause_phase != PausePhase::Paused {
            return false;
        }
        let viewer_team = self
            .players
            .get(slot_index)
            .map(|p| p.hero_team)
            .unwrap_or(self.hero_team);
        if self.pause_is_disconnect {
            let team_has_dc = self
                .players
                .iter()
                .any(|p| p.is_disconnected && p.hero_team == viewer_team);
            if team_has_dc {
                return true;
            }
            self.pause_lock_remaining <= 0.0
        } else {
            if self.paused_by_team == Some(viewer_team) {
                return true;
            }
            self.pause_lock_remaining <= 0.0
        }
    }

    pub fn toggle_pause(&mut self, slot_index: usize, config: &GameConfig) {
        if self.winning_team.is_some() {
            return;
        }
        match self.pause_phase {
            PausePhase::Running => {
                let (team, name, player_cd) = if let Some(p) = self.players.get(slot_index) {
                    (p.hero_team, p.hero_display_name.clone(), p.pause_cooldown_remaining)
                } else {
                    (self.hero_team, self.hero_display_name.clone(), 0.0)
                };
                let any_disconnected = self
                    .players
                    .iter()
                    .any(|p| p.is_disconnected && p.hero_team == team);

                let (pauses_left, time_left) = match team {
                    TeamId::Dawn => (self.dawn_pauses_remaining, self.dawn_time_bank_secs),
                    TeamId::Dusk => (self.dusk_pauses_remaining, self.dusk_time_bank_secs),
                    TeamId::Neutral => return,
                };

                if pauses_left == 0 || time_left <= 0.0 {
                    return;
                }
                if !any_disconnected && player_cd > 0.0 {
                    return;
                }

                match team {
                    TeamId::Dawn => self.dawn_pauses_remaining -= 1,
                    TeamId::Dusk => self.dusk_pauses_remaining -= 1,
                    TeamId::Neutral => return,
                }
                if let Some(p) = self.players.get_mut(slot_index) {
                    p.pause_cooldown_remaining = config.pause.player_cooldown_secs;
                }

                self.pause_phase = PausePhase::Paused;
                self.paused_by_team = Some(team);
                self.paused_by_name = name.clone();
                self.pause_is_disconnect = any_disconnected;
                self.pause_lock_remaining = if any_disconnected {
                    config.pause.disconnect_unpause_lock_secs
                } else {
                    config.pause.manual_unpause_lock_secs
                };
                self.unpause_countdown_remaining = 0.0;

                let id = self.next_chat_id;
                self.next_chat_id += 1;
                self.chat_history.push(ChatMessageEntry {
                    id,
                    sender_name: name.clone(),
                    sender_team: team,
                    kind: ChatMessageKind::PlayerAll,
                    all_chat: true,
                    text: format!("{} paused the game.", name),
                });
            }
            PausePhase::Paused => {
                if !self.can_slot_unpause(slot_index) {
                    return;
                }
                self.pause_phase = PausePhase::UnpauseCountdown;
                self.unpause_countdown_remaining = config.pause.unpause_countdown_secs;
            }
            PausePhase::UnpauseCountdown => {}
        }
    }

    pub fn step_pause_system(&mut self, dt: f32, config: &GameConfig) {
        if self.winning_team.is_some() {
            return;
        }
        for p in &mut self.players {
            if p.pause_cooldown_remaining > 0.0 {
                p.pause_cooldown_remaining = (p.pause_cooldown_remaining - dt).max(0.0);
            }
        }
        match self.pause_phase {
            PausePhase::Running => {}
            PausePhase::Paused => {
                if self.pause_lock_remaining > 0.0 {
                    self.pause_lock_remaining = (self.pause_lock_remaining - dt).max(0.0);
                }
                let time_exhausted = match self.paused_by_team {
                    Some(TeamId::Dawn) => {
                        self.dawn_time_bank_secs = (self.dawn_time_bank_secs - dt).max(0.0);
                        self.dawn_time_bank_secs <= 0.0
                    }
                    Some(TeamId::Dusk) => {
                        self.dusk_time_bank_secs = (self.dusk_time_bank_secs - dt).max(0.0);
                        self.dusk_time_bank_secs <= 0.0
                    }
                    _ => false,
                };
                if time_exhausted {
                    self.pause_phase = PausePhase::UnpauseCountdown;
                    self.unpause_countdown_remaining = config.pause.unpause_countdown_secs;
                }
            }
            PausePhase::UnpauseCountdown => {
                self.unpause_countdown_remaining -= dt;
                if self.unpause_countdown_remaining <= 0.0 {
                    self.pause_phase = PausePhase::Running;
                    self.paused_by_team = None;
                    self.paused_by_name.clear();
                    self.pause_is_disconnect = false;
                    self.pause_lock_remaining = 0.0;
                    self.unpause_countdown_remaining = 0.0;

                    let id = self.next_chat_id;
                    self.next_chat_id += 1;
                    self.chat_history.push(ChatMessageEntry {
                        id,
                        sender_name: "System".to_string(),
                        sender_team: TeamId::Neutral,
                        kind: ChatMessageKind::SystemReconnect,
                        all_chat: true,
                        text: "Game resumed.".to_string(),
                    });
                }
            }
        }
    }

    pub fn push_ping(
        &mut self,
        slot_index: usize,
        world_pos: Vec3,
        config: &GameConfig,
    ) {
        if !world_pos.is_finite() {
            return;
        }
        let now = self.match_elapsed_secs;
        let (sender_name, sender_team) = if let Some(p) = self.players.get_mut(slot_index) {
            if now - p.last_ping_at_secs < config.ping.min_interval_secs {
                return;
            }
            p.last_ping_at_secs = now;
            (p.hero_display_name.clone(), p.hero_team)
        } else {
            (self.hero_display_name.clone(), self.hero_team)
        };
        let id = self.next_ping_id;
        self.next_ping_id += 1;
        self.ping_history.push(PingEntry {
            id,
            sender_name,
            sender_team,
            world_pos,
        });
        let max_keep = config.ping.max_active_pings * 4;
        if self.ping_history.len() > max_keep {
            let drain_n = self.ping_history.len() - max_keep;
            self.ping_history.drain(0..drain_n);
        }
    }

    pub fn set_slot_connected(
        &mut self,
        slot_index: usize,
        connected: bool,
        config: &GameConfig,
    ) {
        let Some(p) = self.players.get_mut(slot_index) else {
            return;
        };
        let was_disconnected = p.is_disconnected;
        p.is_disconnected = !connected;
        if !connected {
            p.active_input = None;
            p.move_input = Vec3::ZERO;
            p.is_moving = false;
        }
        if connected && was_disconnected {
            let name = p.hero_display_name.clone();
            let team = p.hero_team;
            let id = self.next_chat_id;
            self.next_chat_id += 1;
            self.chat_history.push(ChatMessageEntry {
                id,
                sender_name: name.clone(),
                sender_team: team,
                kind: ChatMessageKind::SystemReconnect,
                all_chat: true,
                text: format!("{} reconnected to the game.", name),
            });
        } else if !connected && !was_disconnected {
            let name = p.hero_display_name.clone();
            let team = p.hero_team;
            let id = self.next_chat_id;
            self.next_chat_id += 1;
            self.chat_history.push(ChatMessageEntry {
                id,
                sender_name: name.clone(),
                sender_team: team,
                kind: ChatMessageKind::SystemDisconnect,
                all_chat: true,
                text: format!("{} disconnected from the game.", name),
            });
        }
        let max_keep = config.chat.max_visible_messages * 4;
        if self.chat_history.len() > max_keep {
            let drain_n = self.chat_history.len() - max_keep;
            self.chat_history.drain(0..drain_n);
        }
    }

    pub fn push_chat_message(
        &mut self,
        slot_index: usize,
        all_chat: bool,
        raw_text: &str,
        config: &GameConfig,
    ) {
        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            return;
        }
        let clean: String = trimmed
            .chars()
            .filter(|c| !c.is_control())
            .take(config.chat.max_message_len)
            .collect();
        if clean.is_empty() {
            return;
        }
        let now = self.match_elapsed_secs;
        let (sender_name, sender_team) = if let Some(p) = self.players.get_mut(slot_index) {
            if now - p.last_chat_at_secs < config.chat.min_interval_secs {
                return;
            }
            p.last_chat_at_secs = now;
            (p.hero_display_name.clone(), p.hero_team)
        } else {
            (self.hero_display_name.clone(), self.hero_team)
        };
        let id = self.next_chat_id;
        self.next_chat_id += 1;
        self.chat_history.push(ChatMessageEntry {
            id,
            sender_name,
            sender_team,
            kind: if all_chat {
                ChatMessageKind::PlayerAll
            } else {
                ChatMessageKind::PlayerTeam
            },
            all_chat,
            text: clean,
        });
        let max_keep = config.chat.max_visible_messages * 4;
        if self.chat_history.len() > max_keep {
            let drain_n = self.chat_history.len() - max_keep;
            self.chat_history.drain(0..drain_n);
        }
    }

    pub fn set_slot_input(&mut self, slot_index: usize, mut cmd: ClientInputCommand) {
        if !cmd.move_input.is_finite() {
            cmd.move_input = Vec3::ZERO;
        }
        if !cmd.aim_ray_origin.is_finite()
            || !cmd.aim_ray_dir.is_finite()
            || cmd.aim_ray_dir.length_squared() <= 1e-6
        {
            cmd.has_aim_ray = false;
            cmd.aim_ray_origin = Vec3::ZERO;
            cmd.aim_ray_dir = Vec3::NEG_Y;
        }
        if cmd.client_muzzle_world_pos.map_or(false, |v| !v.is_finite()) {
            cmd.client_muzzle_world_pos = None;
        }
        if let Some(p) = self.players.get_mut(slot_index) {
            if let Some(ref mut prev) = p.active_input {
                prev.accumulate_from(cmd);
            } else {
                p.active_input = Some(cmd);
            }
        } else {
            self.active_input = Some(cmd);
        }
    }

    pub fn init_multiplayer_players(
        &mut self,
        config: &GameConfig,
        match_state: &AuthoritativeMatchState,
        commands: &mut Commands,
        existing_soldiers: &[Entity],
    ) {
        for &ent in existing_soldiers {
            commands.entity(ent).despawn();
        }
        self.players.clear();
        self.env_stats_initialized = false;

        let mut dawn_count = 0_usize;
        let mut dusk_count = 0_usize;

        for lp in &match_state.players {
            let hero_id = lp
                .selected_hero_id
                .as_deref()
                .expect("خطای بحرانی: بازیکن بدون هیروی انتخاب‌شده وارد فاز بازی شد");
            let hero_index = config
                .heroes
                .iter()
                .position(|h| h.id == hero_id)
                .expect("خطای بحرانی: هیروی انتخاب‌شده در config.ron یافت نشد");
            let hero = config.active_hero(hero_index).clone();

            let team_offset = match lp.team {
                TeamId::Dawn => {
                    let c = dawn_count;
                    dawn_count += 1;
                    c
                }
                TeamId::Dusk => {
                    let c = dusk_count;
                    dusk_count += 1;
                    c
                }
                TeamId::Neutral => 0,
            };

            let spawn_pos = team_spawn_pos(config, lp.team, team_offset);
            let facing_dir = match lp.team {
                TeamId::Dusk => Vec3::new(-1.0, 0.0, 0.0),
                _ => Vec3::new(1.0, 0.0, 0.0),
            };
            let aim_hit = spawn_pos + facing_dir * 12.0;

            commands.spawn(soldier_physics_bundle_for_slot(
                config,
                lp.slot_index,
                spawn_pos,
                hero.hero_scale,
            ));

            self.players.push(PlayerSlotState {
                slot_index: lp.slot_index,
                client_id: lp.client_id,
                is_disconnected: !lp.connected,
                pause_cooldown_remaining: 0.0,
                active_input: None,
                soldier_pos: spawn_pos,
                soldier_facing_dir: facing_dir,
                spawn_pos,
                base_stance: Stance::Run,
                effective_stance: Stance::Run,
                current_locomotion: "idle".to_string(),
                aim_world_dir: facing_dir,
                aim_hit_pos: aim_hit,
                laser_hit_pos: aim_hit,
                aim_locked_on_enemy: false,
                hovered_creep_id: None,
                hovered_turret_index: None,
            hovered_barracks_index: None,
            hovered_ancient_index: None,
                is_moving: false,
                move_input: Vec3::ZERO,
                fire_cooldown_remaining: 0.0,
                firing_anim_remaining: None,
                reload_remaining: None,
                grenade_elapsed_and_duration: None,
                grenade_spawned_for_throw: false,
                is_dead: false,
                death_anim_name: "die_1".to_string(),
                respawn_remaining: 0.0,
                hero_index,
                hero_id: hero.id,
                hero_display_name: hero.display_name,
                hero_model_path: hero.model_path,
                hero_team: lp.team,
                hero_gold: hero.starting_gold,
                hero_level: 1,
                hero_xp: 0,
                hero_xp_gain_radius: hero.xp_gain_radius,
                hero_hp: hero.max_hp,
                hero_max_hp: hero.max_hp,
                hero_hp_regen_per_sec: hero.hp_regen_per_sec,
                hero_stamina: hero.max_stamina,
                hero_max_stamina: hero.max_stamina,
                hero_stamina_regen_per_sec: hero.stamina_regen_per_sec,
                hero_slide_stamina_cost: hero.slide_stamina_cost,
                hero_jump_over_stamina_cost: hero.jump_over_stamina_cost,
                stamina_exhausted: false,
                space_hold_secs: None,
                wall_climb_mountain_index: None,
                wall_climb_last_dir: 1,
                hero_armor: hero.armor,
                hero_scale: hero.hero_scale,
                hero_move_speed_multiplier: hero.move_speed_multiplier,
                toxic_slow_multiplier: 1.0,
                beam_charge: hero.weapon.beam_max_charge,
                electric_fire_rate_slow_remaining: 0.0,
                electric_fire_rate_slow_ratio: 0.0,
                flame_armor_debuff_remaining: 0.0,
                flame_armor_debuff_amount: 0.0,
                active_beam_request: None,
                hero_shallow_water_speed_multiplier: hero.shallow_water_speed_multiplier,
                hero_deep_water_speed_multiplier: hero.deep_water_speed_multiplier,
                hero_vision_range: hero.vision_range,
                active_is_night: false,
                active_weather: config.weather.initial_weather,
                ammo_in_mag: hero.weapon.magazine_size,
                hero_weapon: hero.weapon,
                hero_pistol: hero.pistol,
                hero_grenade: hero.grenade,
                grenade_charges: hero.grenade.max_charges,
                grenade_recharge_remaining: 0.0,
                active_weapon: ActiveWeaponSlot::Primary,
                other_weapon_ammo_in_mag: hero.pistol.magazine_size,
                current_move_speed: config.soldier.run_speed * hero.move_speed_multiplier,
                last_hits: 0,
                denies: 0,
                assists: 0,
                kills: 0,
                deaths: 0,
                kill_streak: 0,
                inventory: Vec::new(),
                tp_slot: None,
                courier_pos: Vec3::new(spawn_pos.x, hero.courier.fly_height, spawn_pos.z),
                courier_facing_dir: facing_dir,
                courier_hp: hero.courier.max_hp,
                courier_max_hp: hero.courier.max_hp,
                courier_is_dead: false,
                courier_respawn_remaining: 0.0,
                courier_order: CourierOrder::Idle,
                courier_target_pos: Vec3::new(spawn_pos.x, hero.courier.fly_height, spawn_pos.z),
                courier_inventory: Vec::new(),
                stash: Vec::new(),
                shop_notice_seq: 0,
                shop_notice_text: String::new(),
                shop_notice_is_error: false,
                pending_hero_to_courier_slot: None,
                fog_of_war: SharedFogOfWar::from_config(config, lp.team),
                in_water: false,
                water_splash_cooldown: 0.0,
                teleport_cooldown_remaining: 0.0,
                active_teleport: None,
                mode_cycle_cooldown_remaining: 0.0,
                shadow_blade_cooldown_remaining: 0.0,
                invisibility_remaining: 0.0,
                invisibility_total_secs: 0.0,
                invis_move_speed_bonus_pct: 0.0,
                is_revealed_by_turret: false,
                hero_evasion_chance: 0.0,
                hero_true_strike_chance: 0.0,
                hero_crit_chance: 0.0,
                hero_crit_multiplier: 1.0,
                hero_lifesteal_ratio: 0.0,
                hero_on_hit_slow_ratio: 0.0,
                hero_on_hit_heal_reduction_ratio: 0.0,
                hero_on_hit_debuff_duration_secs: 0.0,
                reflect_armor_cooldown_remaining: 0.0,
                bullet_immunity_cooldown_remaining: 0.0,
                null_pulse_cooldown_remaining: 0.0,
                sonar_reveal_cooldown_remaining: 0.0,
                kinetic_dash_cooldown_remaining: 0.0,
                has_reflect_armor_passive: false,
                reflect_armor_remaining: 0.0,
                reflect_armor_total_secs: 0.0,
                bullet_immunity_remaining: 0.0,
                bullet_immunity_total_secs: 0.0,
                cryo_debuff_remaining: 0.0,
                cryo_debuff_total_secs: 0.0,
                cryo_slow_ratio: 0.0,
                cryo_heal_reduction_ratio: 0.0,
                null_mute_remaining: 0.0,
                null_mute_total_secs: 0.0,
                sonar_revealed_remaining: 0.0,
                sonar_revealed_total_secs: 0.0,
                kinetic_dash_remaining: 0.0,
                kinetic_dash_dir: Vec3::ZERO,
                popup_throttle_remaining: 0.0,
                recent_hero_damagers: Vec::new(),
                last_attacked_enemy_hero_at: -999.0,
                attack_reveal_to_dawn_remaining: 0.0,
                attack_reveal_to_dusk_remaining: 0.0,
                attack_reveal_to_neutral_remaining: 0.0,
                last_ping_at_secs: -999.0,
                last_chat_at_secs: -999.0,
                traversal: SharedTraversalState {
                    is_active: false,
                    kind: TraversalKind::RunningJump,
                    anim_name: String::new(),
                    elapsed: 0.0,
                    duration: 0.0,
                    start_pos: spawn_pos,
                    target_pos: spawn_pos,
                    travel_dir: facing_dir,
                    vault_peak_lift: 0.0,
                    chain_dive_after: false,
                    swing_arc_sag: 0.0,
                    swing_total_duration: 0.0,
                    swing_pulley_pos: None,
                    swing_throw_progress: 0.0,
                    fall_vel: Vec3::ZERO,
                },
            });
        }
    }


    /// افزودن پویا و امن یک هیروی جدید به زمین بازی (مخصوص Demo Hero چندنفره تا ۵v۵)
    pub fn spawn_single_player_slot(
        &mut self,
        config: &GameConfig,
        lp: &LobbyPlayerInfo,
        spawn_pos: Vec3,
        commands: &mut Commands,
    ) {
        let hero_id = lp
            .selected_hero_id
            .as_deref()
            .unwrap_or(&config.heroes[0].id);
        let hero_index = config
            .heroes
            .iter()
            .position(|h| h.id == hero_id)
            .unwrap_or(0);
        let hero = config.active_hero(hero_index).clone();
        let facing_dir = match lp.team {
            TeamId::Dusk => Vec3::new(-1.0, 0.0, 0.0),
            _ => Vec3::new(1.0, 0.0, 0.0),
        };
        let aim_hit = spawn_pos + facing_dir * 12.0;

        commands.spawn(soldier_physics_bundle_for_slot(
            config,
            lp.slot_index,
            spawn_pos,
            hero.hero_scale,
        ));

        self.players.push(PlayerSlotState {
                slot_index: lp.slot_index,
                client_id: lp.client_id,
                is_disconnected: !lp.connected,
                pause_cooldown_remaining: 0.0,
                active_input: None,
                soldier_pos: spawn_pos,
                soldier_facing_dir: facing_dir,
                spawn_pos,
                base_stance: Stance::Run,
                effective_stance: Stance::Run,
                current_locomotion: "idle".to_string(),
                aim_world_dir: facing_dir,
                aim_hit_pos: aim_hit,
                laser_hit_pos: aim_hit,
                aim_locked_on_enemy: false,
                hovered_creep_id: None,
                hovered_turret_index: None,
            hovered_barracks_index: None,
            hovered_ancient_index: None,
                is_moving: false,
                move_input: Vec3::ZERO,
                fire_cooldown_remaining: 0.0,
                firing_anim_remaining: None,
                reload_remaining: None,
                grenade_elapsed_and_duration: None,
                grenade_spawned_for_throw: false,
                is_dead: false,
                death_anim_name: "die_1".to_string(),
                respawn_remaining: 0.0,
                hero_index,
                hero_id: hero.id,
                hero_display_name: hero.display_name,
                hero_model_path: hero.model_path,
                hero_team: lp.team,
                hero_gold: hero.starting_gold,
                hero_level: 1,
                hero_xp: 0,
                hero_xp_gain_radius: hero.xp_gain_radius,
                hero_hp: hero.max_hp,
                hero_max_hp: hero.max_hp,
                hero_hp_regen_per_sec: hero.hp_regen_per_sec,
                hero_stamina: hero.max_stamina,
                hero_max_stamina: hero.max_stamina,
                hero_stamina_regen_per_sec: hero.stamina_regen_per_sec,
                hero_slide_stamina_cost: hero.slide_stamina_cost,
                hero_jump_over_stamina_cost: hero.jump_over_stamina_cost,
                stamina_exhausted: false,
                space_hold_secs: None,
                wall_climb_mountain_index: None,
                wall_climb_last_dir: 1,
                hero_armor: hero.armor,
                hero_scale: hero.hero_scale,
                hero_move_speed_multiplier: hero.move_speed_multiplier,
                toxic_slow_multiplier: 1.0,
                beam_charge: hero.weapon.beam_max_charge,
                electric_fire_rate_slow_remaining: 0.0,
                electric_fire_rate_slow_ratio: 0.0,
                flame_armor_debuff_remaining: 0.0,
                flame_armor_debuff_amount: 0.0,
                active_beam_request: None,
                hero_shallow_water_speed_multiplier: hero.shallow_water_speed_multiplier,
                hero_deep_water_speed_multiplier: hero.deep_water_speed_multiplier,
                hero_vision_range: hero.vision_range,
                active_is_night: false,
                active_weather: config.weather.initial_weather,
                ammo_in_mag: hero.weapon.magazine_size,
                hero_weapon: hero.weapon,
                hero_pistol: hero.pistol,
                hero_grenade: hero.grenade,
                grenade_charges: hero.grenade.max_charges,
                grenade_recharge_remaining: 0.0,
                active_weapon: ActiveWeaponSlot::Primary,
                other_weapon_ammo_in_mag: hero.pistol.magazine_size,
                current_move_speed: config.soldier.run_speed * hero.move_speed_multiplier,
                last_hits: 0,
                denies: 0,
                assists: 0,
                kills: 0,
                deaths: 0,
                kill_streak: 0,
                inventory: Vec::new(),
                tp_slot: None,
                courier_pos: Vec3::new(spawn_pos.x, hero.courier.fly_height, spawn_pos.z),
                courier_facing_dir: facing_dir,
                courier_hp: hero.courier.max_hp,
                courier_max_hp: hero.courier.max_hp,
                courier_is_dead: false,
                courier_respawn_remaining: 0.0,
                courier_order: CourierOrder::Idle,
                courier_target_pos: Vec3::new(spawn_pos.x, hero.courier.fly_height, spawn_pos.z),
                courier_inventory: Vec::new(),
                stash: Vec::new(),
                shop_notice_seq: 0,
                shop_notice_text: String::new(),
                shop_notice_is_error: false,
                pending_hero_to_courier_slot: None,
                fog_of_war: SharedFogOfWar::from_config(config, lp.team),
                in_water: false,
                water_splash_cooldown: 0.0,
                teleport_cooldown_remaining: 0.0,
                active_teleport: None,
                mode_cycle_cooldown_remaining: 0.0,
                shadow_blade_cooldown_remaining: 0.0,
                invisibility_remaining: 0.0,
                invisibility_total_secs: 0.0,
                invis_move_speed_bonus_pct: 0.0,
                is_revealed_by_turret: false,
                hero_evasion_chance: 0.0,
                hero_true_strike_chance: 0.0,
                hero_crit_chance: 0.0,
                hero_crit_multiplier: 1.0,
                hero_lifesteal_ratio: 0.0,
                hero_on_hit_slow_ratio: 0.0,
                hero_on_hit_heal_reduction_ratio: 0.0,
                hero_on_hit_debuff_duration_secs: 0.0,
                reflect_armor_cooldown_remaining: 0.0,
                bullet_immunity_cooldown_remaining: 0.0,
                null_pulse_cooldown_remaining: 0.0,
                sonar_reveal_cooldown_remaining: 0.0,
                kinetic_dash_cooldown_remaining: 0.0,
                has_reflect_armor_passive: false,
                reflect_armor_remaining: 0.0,
                reflect_armor_total_secs: 0.0,
                bullet_immunity_remaining: 0.0,
                bullet_immunity_total_secs: 0.0,
                cryo_debuff_remaining: 0.0,
                cryo_debuff_total_secs: 0.0,
                cryo_slow_ratio: 0.0,
                cryo_heal_reduction_ratio: 0.0,
                null_mute_remaining: 0.0,
                null_mute_total_secs: 0.0,
                sonar_revealed_remaining: 0.0,
                sonar_revealed_total_secs: 0.0,
                kinetic_dash_remaining: 0.0,
                kinetic_dash_dir: Vec3::ZERO,
                popup_throttle_remaining: 0.0,
                recent_hero_damagers: Vec::new(),
                last_attacked_enemy_hero_at: -999.0,
                attack_reveal_to_dawn_remaining: 0.0,
                attack_reveal_to_dusk_remaining: 0.0,
                attack_reveal_to_neutral_remaining: 0.0,
                last_ping_at_secs: -999.0,
                last_chat_at_secs: -999.0,
                traversal: SharedTraversalState {
                    is_active: false,
                    kind: TraversalKind::RunningJump,
                    anim_name: String::new(),
                    elapsed: 0.0,
                    duration: 0.0,
                    start_pos: spawn_pos,
                    target_pos: spawn_pos,
                    travel_dir: facing_dir,
                    vault_peak_lift: 0.0,
                    chain_dive_after: false,
                    swing_arc_sag: 0.0,
                    swing_total_duration: 0.0,
                    swing_pulley_pos: None,
                    swing_throw_progress: 0.0,
                    fall_vel: Vec3::ZERO,
                },
            });
        self.env_stats_initialized = false;
    }

    #[inline]
    pub fn is_slot_los_visible_to_team(&self, viewer_team: TeamId, slot_index: usize) -> bool {
        match viewer_team {
            TeamId::Dawn => self
                .dawn_los_visible_slots
                .get(slot_index)
                .copied()
                .unwrap_or(false),
            TeamId::Dusk => self
                .dusk_los_visible_slots
                .get(slot_index)
                .copied()
                .unwrap_or(false),
            TeamId::Neutral => false,
        }
    }

    #[inline]
    pub fn is_creep_los_visible_to_team(&self, viewer_team: TeamId, creep_id: u64) -> bool {
        match viewer_team {
            TeamId::Dawn => self.dawn_los_visible_creeps.contains(&creep_id),
            TeamId::Dusk => self.dusk_los_visible_creeps.contains(&creep_id),
            TeamId::Neutral => false,
        }
    }

    #[inline]
    pub fn is_courier_los_visible_to_team(&self, viewer_team: TeamId, slot_index: usize) -> bool {
        match viewer_team {
            TeamId::Dawn => self
                .dawn_los_visible_couriers
                .get(slot_index)
                .copied()
                .unwrap_or(false),
            TeamId::Dusk => self
                .dusk_los_visible_couriers
                .get(slot_index)
                .copied()
                .unwrap_or(false),
            TeamId::Neutral => false,
        }
    }

    pub fn build_snapshot_for_slot(&mut self, config: &GameConfig, items_cfg: &ItemsConfig, slot_index: usize) -> ServerWorldSnapshot {
        if self.players.is_empty() {
            return self.build_snapshot(config, items_cfg);
        }
        // قبل از swap_slot_in، لیست کامل کارت‌های نوار بالا را از داده‌های واقعی تمام بازیکنان بساز
        let viewer_team = self.players[slot_index].hero_team;
        let top_bar: Vec<TopBarHeroInfo> = self
            .players
            .iter()
            .map(|p| {
                let portrait = config.active_hero(p.hero_index).portrait_path.clone();
                TopBarHeroInfo {
                    slot_index: p.slot_index,
                    team: p.hero_team,
                    hero_id: p.hero_id.clone(),
                    display_name: p.hero_display_name.clone(),
                    portrait_path: portrait,
                    is_dead: p.is_dead,
                    is_disconnected: p.is_disconnected,
                    respawn_remaining: if p.is_dead {
                        p.respawn_remaining.max(0.0)
                    } else {
                        0.0
                    },
                    hp_ratio: if p.hero_team == viewer_team {
                        Some((p.hero_hp / p.hero_max_hp.max(1.0)).clamp(0.0, 1.0))
                    } else {
                        None
                    },
                }
            })
            .collect();

        let scoreboard_heroes: Vec<ScoreboardHeroInfo> = self
            .players
            .iter()
            .map(|p| {
                let portrait = config.active_hero(p.hero_index).portrait_path.clone();
                let is_ally = p.hero_team == viewer_team;
                let xp_info = if is_ally {
                    let cur_base = config.combat.xp_thresholds_per_level[(p.hero_level as usize - 1).min(24)];
                    let xp_into_level = p.hero_xp.saturating_sub(cur_base);
                    let xp_for_next_level = if p.hero_level >= 25 {
                        0
                    } else {
                        let next_base = config.combat.xp_thresholds_per_level[p.hero_level as usize];
                        next_base.saturating_sub(cur_base)
                    };
                    Some(ScoreboardXpInfo {
                        xp: p.hero_xp,
                        xp_into_level,
                        xp_for_next_level,
                    })
                } else {
                    None
                };
                ScoreboardHeroInfo {
                    slot_index: p.slot_index,
                    team: p.hero_team,
                    hero_id: p.hero_id.clone(),
                    display_name: p.hero_display_name.clone(),
                    portrait_path: portrait,
                    is_local_player: p.slot_index == slot_index,
                    is_dead: p.is_dead,
                    is_disconnected: p.is_disconnected,
                    respawn_remaining: if p.is_dead {
                        p.respawn_remaining.max(0.0)
                    } else {
                        0.0
                    },
                    level: p.hero_level,
                    kills: p.kills,
                    deaths: p.deaths,
                    assists: p.assists,
                    xp_info,
                    gold: if is_ally { Some(p.hero_gold) } else { None },
                    inventory: if is_ally { Some(p.inventory.clone()) } else { None },
                }
            })
            .collect();

        let water_hidden_slots: HashSet<usize> = self
            .players
            .iter()
            .filter(|p| {
                p.hero_team != viewer_team
                    && !is_swimming_hero_revealed_to_team(
                        &self.players,
                        p.slot_index,
                        viewer_team,
                        config,
                        self.match_elapsed_secs,
                        self.current_weather,
                    )
            })
            .map(|p| p.slot_index)
            .collect();

        let pre_swap_dawn_kills: u32 = self
            .players
            .iter()
            .filter(|p| p.hero_team == TeamId::Dawn)
            .map(|p| p.kills)
            .sum();
        let pre_swap_dusk_kills: u32 = self
            .players
            .iter()
            .filter(|p| p.hero_team == TeamId::Dusk)
            .map(|p| p.kills)
            .sum();
        self.swap_slot_in(slot_index);
        let mut snap = self.build_snapshot(config, items_cfg);
        snap.scoreboard_heroes = scoreboard_heroes;

        // فیلتر مه جنگ سه‌بعدی برای پرتابه‌ها، نارنجک‌ها، استخر سمی، پرتوها و هدف تاورهای دشمن
        let reveal_all = self.demo_reveal_full_map;
        snap.bullets
            .retain(|b| reveal_all || self.fog_of_war.is_world_pos_visible(config, b.translation));
        snap.grenades
            .retain(|g| reveal_all || self.fog_of_war.is_world_pos_visible(config, g.translation));
        snap.toxic_pools
            .retain(|tp| reveal_all || self.fog_of_war.is_world_pos_visible(config, tp.pos));
        snap.electric_beams.retain(|eb| {
            reveal_all
                || self.fog_of_war.is_world_pos_visible(config, eb.start_pos)
                || self.fog_of_war.is_world_pos_visible(config, eb.end_pos)
        });
        for t in &mut snap.turrets {
            if t.team != viewer_team && !reveal_all {
                let t_pos = self.turrets.get(t.index).map(|ts| ts.pos).unwrap_or(Vec3::ZERO);
                let turret_vis = self.fog_of_war.is_world_pos_visible(config, t_pos);
                let target_vis = t.aim_target_pos.is_some_and(|ap| {
                    self.fog_of_war.is_world_pos_visible(config, ap)
                });
                if !turret_vis || !target_vis {
                    t.aim_target_pos = None;
                }
            }
        }

        // فیلتر مه جنگ و نامرئی‌شدن برای تلپورت‌های فعال (پس از swap_slot_in تا fog_of_war تیم بیننده فعال باشد)
        let mut active_teleports: Vec<ActiveTeleportSnapshot> = Vec::new();
        for p in &self.players {
            let (p_team, p_invis, p_rev, p_tp_opt) = if p.slot_index == slot_index {
                (
                    self.hero_team,
                    self.invisibility_remaining,
                    self.is_revealed_by_turret,
                    self.active_teleport.as_ref(),
                )
            } else {
                (
                    p.hero_team,
                    p.invisibility_remaining,
                    p.is_revealed_by_turret,
                    p.active_teleport.as_ref(),
                )
            };
            let Some(tp) = p_tp_opt else { continue };
            let mut origin_pos = tp.origin_pos;
            if p_team != viewer_team {
                if (p_invis > 0.0 && !p_rev) || water_hidden_slots.contains(&p.slot_index) {
                    continue;
                }
                let origin_vis =
                    reveal_all || self.fog_of_war.is_world_pos_visible(config, tp.origin_pos);
                let dest_vis =
                    reveal_all || self.fog_of_war.is_world_pos_visible(config, tp.dest_pos);
                if !origin_vis && !dest_vis {
                    continue;
                }
                if !origin_vis {
                    origin_pos = tp.dest_pos;
                }
            }
            active_teleports.push(ActiveTeleportSnapshot {
                slot_index: p.slot_index,
                team: p_team,
                hero_id: p.hero_id.clone(),
                model_path: p.hero_model_path.clone(),
                hero_scale: p.hero_scale,
                origin_pos,
                dest_pos: tp.dest_pos,
                facing_dir: tp.facing_dir,
                progress: (tp.elapsed_secs / tp.total_secs.max(0.01)).clamp(0.0, 1.0),
            });
        }
        snap.active_teleports = active_teleports;
        if let Some(me) = self.players.get(slot_index) {
            snap.hero.courier_hp = me.courier_hp;
            snap.hero.courier_max_hp = me.courier_max_hp;
            snap.hero.courier_is_dead = me.courier_is_dead;
            snap.hero.courier_respawn_remaining = me.courier_respawn_remaining.max(0.0);
            snap.hero.courier_order = me.courier_order;
            snap.hero.courier_in_fountain = fountain_regen_bonus_for_pos(config, me.hero_team, me.courier_pos) > 0.0;
            snap.hero.courier_inventory = me.courier_inventory.clone();
            snap.hero.stash = me.stash.clone();
            snap.hero.shop_notice_seq = me.shop_notice_seq;
            snap.hero.shop_notice_text = me.shop_notice_text.clone();
            snap.hero.shop_notice_is_error = me.shop_notice_is_error;
        }
        for p in &self.players {
            let hero_cfg = config.active_hero(p.hero_index);
            let is_vis = p.hero_team == viewer_team
                || reveal_all
                || (self.is_courier_los_visible_to_team(viewer_team, p.slot_index)
                    && self.fog_of_war.is_world_pos_visible(config, p.courier_pos));
            if !is_vis {
                continue;
            }
            snap.couriers.push(CourierSnapshot {
                owner_slot: p.slot_index,
                team: p.hero_team,
                model_path: hero_cfg.courier.model_path.clone(),
                scale: hero_cfg.courier.scale,
                pos: p.courier_pos,
                facing_dir: p.courier_facing_dir,
                hp: p.courier_hp,
                max_hp: p.courier_max_hp,
                is_dead: p.courier_is_dead,
                respawn_remaining: p.courier_respawn_remaining.max(0.0),
                order: p.courier_order,
                inventory: p.courier_inventory.clone(),
                is_visible: is_vis,
            });
        }
        let mut others = Vec::with_capacity(self.players.len().saturating_sub(1));
        for p in &self.players {

            if p.slot_index == slot_index {
                continue;
            }
            let hidden_by_invis = p.hero_team != self.hero_team
                && p.invisibility_remaining > 0.0
                && !p.is_revealed_by_turret
                && p.sonar_revealed_remaining <= 0.0;
            let hidden_by_water = water_hidden_slots.contains(&p.slot_index)
                && p.sonar_revealed_remaining <= 0.0;
            let target_vis_h =
                SharedFogOfWar::hero_visible_height(config, p.effective_stance, p.hero_scale);
            let is_vis = !hidden_by_invis
                && !hidden_by_water
                && (p.hero_team == self.hero_team
                    || self.demo_reveal_full_map
                    || p.sonar_revealed_remaining > 0.0
                    || (self.is_slot_los_visible_to_team(viewer_team, p.slot_index)
                        && self
                            .fog_of_war
                            .is_world_entity_visible(config, p.soldier_pos, target_vis_h)));
            // Server Authority: اگر دشمن در مه جنگ باشد، مختصات و اطلاعات او اصلاً به کلاینت ارسال نمی‌شود!
            if !is_vis {
                continue;
            }
            let trav_anim = if p.traversal.is_active {
                Some(p.traversal.anim_name.clone())
            } else {
                None
            };
            let in_deep_water_loco = p.current_locomotion == "swimming"
                || p.current_locomotion == "treading_water";
            let is_climb_loco = p.current_locomotion.starts_with("climbing_");
            let upper_anim = if p.grenade_elapsed_and_duration.is_some() {
                Some("toss_grenade".to_string())
            } else if p.reload_remaining.is_some() {
                Some("reloading".to_string())
            } else if in_deep_water_loco || is_climb_loco {
                None
            } else if p.active_weapon == ActiveWeaponSlot::Pistol {
                Some("pistol_run".to_string())
            } else {
                Some("rifle_aiming_idle".to_string())
            };
            others.push(RemotePlayerSnapshot {
                slot_index: p.slot_index,
                team: p.hero_team,
                hero_id: p.hero_id.clone(),
                display_name: p.hero_display_name.clone(),
                model_path: p.hero_model_path.clone(),
                hero_scale: p.hero_scale,
                weapon_kind: if p.active_weapon == ActiveWeaponSlot::Pistol {
                    p.hero_pistol.weapon_kind
                } else {
                    p.hero_weapon.weapon_kind
                },
                pos: p.soldier_pos,
                facing_dir: p.soldier_facing_dir,
                current_locomotion: p.current_locomotion.clone(),
                traversal_anim: trav_anim,
                upper_anim,
                death_anim: p.death_anim_name.clone(),
                is_dead: p.is_dead,
                hp: p.hero_hp,
                max_hp: p.hero_max_hp,
                stamina: p.hero_stamina,
                max_stamina: p.hero_max_stamina,
                active_weapon: p.active_weapon,
                ammo_in_mag: p.ammo_in_mag,
                grenade_charges: p.grenade_charges,
                grenade_max_charges: p.hero_grenade.max_charges,
                grenade_throw_duration_secs: p.hero_grenade.throw_duration_secs,
                magazine_size: if p.active_weapon == ActiveWeaponSlot::Pistol {
                    p.hero_pistol.magazine_size
                } else {
                    p.hero_weapon.magazine_size
                },
                level: p.hero_level,
                armor: p.hero_armor,
                move_speed: p.current_move_speed,
                weapon_damage: if p.active_weapon == ActiveWeaponSlot::Pistol {
                    p.hero_pistol.damage
                } else {
                    p.hero_weapon.damage
                },
                fire_rate_secs: if p.active_weapon == ActiveWeaponSlot::Pistol {
                    p.hero_pistol.fire_rate_secs
                } else {
                    p.hero_weapon.fire_rate_secs
                },
                weapon_max_range: if p.active_weapon == ActiveWeaponSlot::Pistol {
                    p.hero_pistol.max_range
                } else {
                    p.hero_weapon.max_range
                },
                inventory: p.inventory.clone(),
                tp_slot: p.tp_slot.clone(),
                is_invisible: p.invisibility_remaining > 0.0,
                is_reflect_armor_active: p.reflect_armor_remaining > 0.0,
                is_bullet_immune: p.bullet_immunity_remaining > 0.0,
                is_cryo_debuffed: p.cryo_debuff_remaining > 0.0,
                is_null_muted: p.null_mute_remaining > 0.0,
                is_sonar_revealed: p.sonar_revealed_remaining > 0.0,
                is_visible: true,
            });
        }
        snap.dawn_total_kills = pre_swap_dawn_kills;
        snap.dusk_total_kills = pre_swap_dusk_kills;
        snap.other_players = others;
        snap.top_bar_heroes = top_bar;
        let max_vis = config.chat.max_visible_messages;
        let filtered_chat: Vec<ChatMessageEntry> = self
            .chat_history
            .iter()
            .filter(|c| c.all_chat || c.sender_team == viewer_team)
            .cloned()
            .collect();
        let start_idx = filtered_chat.len().saturating_sub(max_vis);
        snap.recent_chat = filtered_chat[start_idx..].to_vec();
        let max_pings = config.ping.max_active_pings;
        let filtered_pings: Vec<PingEntry> = self
            .ping_history
            .iter()
            .filter(|pg| pg.sender_team == viewer_team)
            .cloned()
            .collect();
        let ping_start = filtered_pings.len().saturating_sub(max_pings);
        snap.recent_pings = filtered_pings[ping_start..].to_vec();
        let (team_pauses_left, team_time_left) = match viewer_team {
            TeamId::Dawn => (self.dawn_pauses_remaining, self.dawn_time_bank_secs),
            TeamId::Dusk => (self.dusk_pauses_remaining, self.dusk_time_bank_secs),
            TeamId::Neutral => (0, 0.0),
        };
        let player_cd = self
            .players
            .get(slot_index)
            .map(|p| p.pause_cooldown_remaining)
            .unwrap_or(0.0);
        snap.pause = PauseSnapshot {
            phase: self.pause_phase,
            paused_by_team: self.paused_by_team,
            paused_by_name: self.paused_by_name.clone(),
            is_disconnect_pause: self.pause_is_disconnect,
            lock_remaining_secs: self.pause_lock_remaining.max(0.0),
            countdown_remaining_secs: self.unpause_countdown_remaining.max(0.0),
            your_team_pauses_left: team_pauses_left,
            your_team_time_left_secs: team_time_left.max(0.0),
            your_player_cooldown_secs: player_cd.max(0.0),
            can_you_unpause: self.can_slot_unpause(slot_index),
        };
        snap.creeps = self
            .creeps
            .iter()
            .filter_map(|c| {
                let c_vis_h = resolve_creep_stats(config, c.creep_type_id.as_deref()).hitbox_height;
                let is_vis = c.team == self.hero_team
                    || reveal_all
                    || (self.is_creep_los_visible_to_team(viewer_team, c.id)
                        && self.fog_of_war.is_world_entity_visible(config, c.pos, c_vis_h));
                if !is_vis {
                    return None;
                }
                Some(CreepSnapshot {
                    id: c.id,
                    team: c.team,
                    creep_type_id: c.creep_type_id.clone(),
                    pos: c.pos,
                    facing_dir: c.facing_dir,
                    hp: c.hp,
                    max_hp: c.max_hp,
                    anim_state: c.anim_state,
                    scale_multiplier: lane_creep_buffs(
                        config,
                        &self.barracks,
                        c.team,
                        c.lane,
                        c.camp_index.is_none(),
                    ).1,
                    is_dead: c.is_dead,
                    is_deniable: !c.is_dead
                        && c.team == self.hero_team
                        && (c.hp / c.max_hp) <= config.creeps.deny_hp_threshold_ratio,
                    move_speed: resolve_creep_stats(config, c.creep_type_id.as_deref()).move_speed,
                    armor: {
                        let base_stats = resolve_creep_stats(config, c.creep_type_id.as_deref());
                        let is_lane = c.camp_index.is_none();
                        let (_, time_armor_bonus, _, _) =
                            lane_creep_time_scaling(config, self.match_elapsed_secs, is_lane);
                        let (_, _, bar_armor_bonus, _, _) =
                            lane_creep_buffs(config, &self.barracks, c.team, c.lane, is_lane);
                        base_stats.armor + time_armor_bonus + bar_armor_bonus
                    },
                    damage: {
                        let base_stats = resolve_creep_stats(config, c.creep_type_id.as_deref());
                        let is_lane = c.camp_index.is_none();
                        let (_, _, time_dmg_bonus, _) =
                            lane_creep_time_scaling(config, self.match_elapsed_secs, is_lane);
                        let (_, _, _, bar_dmg_mult, _) =
                            lane_creep_buffs(config, &self.barracks, c.team, c.lane, is_lane);
                        (base_stats.bullet_damage + time_dmg_bonus) * bar_dmg_mult
                    },
                    attack_range: resolve_creep_stats(config, c.creep_type_id.as_deref()).attack_range,
                    fire_rate_secs: {
                        let base_stats = resolve_creep_stats(config, c.creep_type_id.as_deref());
                        let is_lane = c.camp_index.is_none();
                        let (_, _, _, time_fr_mult) =
                            lane_creep_time_scaling(config, self.match_elapsed_secs, is_lane);
                        let (_, _, _, _, bar_fr_mult) =
                            lane_creep_buffs(config, &self.barracks, c.team, c.lane, is_lane);
                        (base_stats.fire_rate_secs * time_fr_mult * bar_fr_mult)
                            .max(config.creeps.min_fire_rate_secs)
                    },
                    is_visible: true,
                })
            })
            .collect();
        self.swap_slot_in(slot_index);
        snap
    }

    pub fn filter_fx_for_slot(
        &self,
        config: &GameConfig,
        slot_index: usize,
        fx_batch: &[GameplayFxEvent],
    ) -> Vec<GameplayFxEvent> {
        let Some(viewer) = self.players.get(slot_index) else {
            return fx_batch.to_vec();
        };
        let viewer_team = viewer.hero_team;
        let fow = &viewer.fog_of_war;
        let is_slot_hidden = |target_slot: usize| -> bool {
            let Some(p) = self.players.get(target_slot) else {
                return false;
            };
            if p.hero_team == viewer_team {
                return false;
            }
            if p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret && p.sonar_revealed_remaining <= 0.0 {
                return true;
            }
            !is_swimming_hero_revealed_to_team(
                &self.players,
                p.slot_index,
                viewer_team,
                config,
                self.match_elapsed_secs,
                self.current_weather,
            )
        };

        let mut out = Vec::with_capacity(fx_batch.len());
        for ev in fx_batch {
            let keep = match ev {
                GameplayFxEvent::LightningStrike { .. }
                | GameplayFxEvent::HeroKillFeed { .. }
                | GameplayFxEvent::CreepDenied { .. } => true,
                GameplayFxEvent::ShotFired { muzzle_pos, .. } => {
                    self.demo_reveal_full_map || fow.is_world_pos_visible(config, *muzzle_pos)
                }
                GameplayFxEvent::BulletTrailSegment { start, end, .. } => {
                    self.demo_reveal_full_map
                        || fow.is_world_pos_visible(config, *start)
                        || fow.is_world_pos_visible(config, *end)
                }
                GameplayFxEvent::BulletImpact { impact_pos, .. } => {
                    self.demo_reveal_full_map || fow.is_world_pos_visible(config, *impact_pos)
                }
                GameplayFxEvent::GrenadeExploded { pos, .. }
                | GameplayFxEvent::SlideDust { pos }
                | GameplayFxEvent::WaterSplash { pos, .. } => {
                    self.demo_reveal_full_map || fow.is_world_pos_visible(config, *pos)
                }
                // این دو رویداد صرفاً از طریق pending_private_fx برای خود بازیکن ارسال می‌شوند
                GameplayFxEvent::HeroLeveledUp { .. }
                | GameplayFxEvent::LastHitGold { .. } => false,
                GameplayFxEvent::TurretShotFired {
                    turret_index,
                    target_pos,
                    ..
                } => {
                    let t_team_match = self
                        .turrets
                        .get(*turret_index)
                        .is_some_and(|t| t.team == viewer_team);
                    let t_vis = self
                        .turrets
                        .get(*turret_index)
                        .is_some_and(|t| fow.is_world_pos_visible(config, t.pos));
                    self.demo_reveal_full_map
                        || t_team_match
                        || t_vis
                        || fow.is_world_pos_visible(config, *target_pos)
                }
                GameplayFxEvent::TeleportStarted {
                    slot_index: tp_slot,
                    origin_pos,
                    dest_pos,
                }
                | GameplayFxEvent::TeleportCompleted {
                    slot_index: tp_slot,
                    origin_pos,
                    dest_pos,
                } => {
                    let is_ally = self
                        .players
                        .get(*tp_slot)
                        .is_some_and(|p| p.hero_team == viewer_team);
                    is_ally
                        || (!is_slot_hidden(*tp_slot)
                            && (self.demo_reveal_full_map
                                || fow.is_world_pos_visible(config, *origin_pos)
                                || fow.is_world_pos_visible(config, *dest_pos)))
                }
                GameplayFxEvent::TeleportCanceled {
                    slot_index: tp_slot,
                    origin_pos,
                } => {
                    let is_ally = self
                        .players
                        .get(*tp_slot)
                        .is_some_and(|p| p.hero_team == viewer_team);
                    is_ally
                        || (!is_slot_hidden(*tp_slot)
                            && (self.demo_reveal_full_map
                                || fow.is_world_pos_visible(config, *origin_pos)))
                }
                GameplayFxEvent::ShadowBladeActivated {
                    slot_index: sb_slot,
                    pos,
                }
                | GameplayFxEvent::ShadowBladeEnded {
                    slot_index: sb_slot,
                    pos,
                } => {
                    let is_ally = self
                        .players
                        .get(*sb_slot)
                        .is_some_and(|p| p.hero_team == viewer_team);
                    is_ally
                        || self.demo_reveal_full_map
                        || fow.is_world_pos_visible(config, *pos)
                }
                GameplayFxEvent::CombatPopup { pos, .. }
                | GameplayFxEvent::TacticalPulse { pos, .. }
                | GameplayFxEvent::DamageNumber { pos, .. }
                | GameplayFxEvent::EvadeMiss { pos }
                | GameplayFxEvent::LifestealHeal { pos, .. }
                | GameplayFxEvent::ReflectHitPulse { pos, .. } => {
                    self.demo_reveal_full_map || fow.is_world_pos_visible(config, *pos)
                }
            };
            if keep {
                out.push(ev.clone());
            }
        }
        out
    }

    pub fn apply_selected_hero(&mut self, config: &GameConfig, hero_index: usize) {
        let hero = config.active_hero(hero_index).clone();
        self.hero_index = hero_index;
        self.hero_id = hero.id;
        self.hero_display_name = hero.display_name;
        self.hero_team = hero.team;
        self.hero_gold = hero.starting_gold;
        self.hero_xp_gain_radius = hero.xp_gain_radius;
        self.hero_hp = hero.max_hp;
        self.hero_max_hp = hero.max_hp;
        self.hero_hp_regen_per_sec = hero.hp_regen_per_sec;
        self.hero_stamina = hero.max_stamina;
        self.hero_max_stamina = hero.max_stamina;
        self.hero_stamina_regen_per_sec = hero.stamina_regen_per_sec;
        self.hero_slide_stamina_cost = hero.slide_stamina_cost;
        self.hero_jump_over_stamina_cost = hero.jump_over_stamina_cost;
        self.stamina_exhausted = false;
        self.space_hold_secs = None;
        self.hero_armor = hero.armor;
        self.hero_scale = hero.hero_scale;
        self.hero_move_speed_multiplier = hero.move_speed_multiplier;
        self.toxic_slow_multiplier = 1.0;
        self.beam_charge = hero.weapon.beam_max_charge;
        self.electric_fire_rate_slow_remaining = 0.0;
        self.electric_fire_rate_slow_ratio = 0.0;
        self.flame_armor_debuff_remaining = 0.0;
        self.flame_armor_debuff_amount = 0.0;
        self.active_beam_request = None;
        self.hero_shallow_water_speed_multiplier = hero.shallow_water_speed_multiplier;
        self.hero_deep_water_speed_multiplier = hero.deep_water_speed_multiplier;
        self.hero_vision_range = hero.vision_range;
        self.ammo_in_mag = hero.weapon.magazine_size;
        self.hero_weapon = hero.weapon;
        self.hero_pistol = hero.pistol;
        self.hero_grenade = hero.grenade;
        self.grenade_charges = hero.grenade.max_charges;
        self.grenade_recharge_remaining = 0.0;
        self.active_weapon = ActiveWeaponSlot::Primary;
        self.other_weapon_ammo_in_mag = hero.pistol.magazine_size;
        self.current_move_speed = config.soldier.run_speed * hero.move_speed_multiplier;
        self.fog_of_war = SharedFogOfWar::from_config(config, hero.team);
    }

    pub fn build_snapshot(&mut self, config: &GameConfig, items_cfg: &ItemsConfig) -> ServerWorldSnapshot {
        self.tick += 1;
        ServerWorldSnapshot {
            tick: self.tick,
            match_elapsed_secs: self.match_elapsed_secs,
            current_weather: self.current_weather,
            dawn_total_kills: 0,
            dusk_total_kills: 0,
            soldier_pos: self.soldier_pos,
            soldier_facing_dir: self.soldier_facing_dir,
            aim_world_dir: self.aim_world_dir,
            aim_hit_pos: self.aim_hit_pos,
            laser_hit_pos: self.laser_hit_pos,
            aim_locked_on_enemy: self.aim_locked_on_enemy,
            hovered_creep_id: self.hovered_creep_id,
            hovered_turret_index: self.hovered_turret_index,
            hovered_barracks_index: self.hovered_barracks_index,
            hovered_ancient_index: self.hovered_ancient_index,
            winning_team: self.winning_team,
            current_locomotion: self.current_locomotion.clone(),
            upper_action: self.upper_body_action(),
            traversal_active: self.traversal.is_active,
            traversal_anim_name: self.traversal.anim_name.clone(),
            traversal_duration: self.traversal.duration,
            swing_pulley_pos: if self.traversal.is_active && self.traversal.kind == TraversalKind::RopeSwing {
                self.traversal.swing_pulley_pos
            } else {
                None
            },
            swing_throw_progress: if self.traversal.is_active && self.traversal.kind == TraversalKind::RopeSwing {
                self.traversal.swing_throw_progress
            } else {
                0.0
            },
            is_dead: self.is_dead,
            death_anim_name: self.death_anim_name.clone(),
            in_water: self.in_water,
            hero: HeroSnapshot {
                hero_id: self.hero_id.clone(),
                display_name: self.hero_display_name.clone(),
                team: self.hero_team,
                gold: self.hero_gold,
                level: self.hero_level,
                xp: self.hero_xp,
                xp_into_level: {
                    let cur_base = config.combat.xp_thresholds_per_level[(self.hero_level as usize - 1).min(24)];
                    self.hero_xp.saturating_sub(cur_base)
                },
                xp_for_next_level: {
                    if self.hero_level >= 25 {
                        0
                    } else {
                        let cur_base = config.combat.xp_thresholds_per_level[self.hero_level as usize - 1];
                        let next_base = config.combat.xp_thresholds_per_level[self.hero_level as usize];
                        next_base.saturating_sub(cur_base)
                    }
                },
                xp_gain_radius: self.hero_xp_gain_radius,
                hp: self.hero_hp,
                max_hp: self.hero_max_hp,
                hp_regen_per_sec: {
                    let fountain_bonus = if self.is_dead {
                        0.0
                    } else {
                        fountain_regen_bonus_for_pos(config, self.hero_team, self.soldier_pos)
                    };
                    let cryo_heal_mult = if self.cryo_debuff_remaining > 0.0 {
                        (1.0 - self.cryo_heal_reduction_ratio).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    (self.hero_hp_regen_per_sec + fountain_bonus) * cryo_heal_mult
                },
                in_fountain: !self.is_dead
                    && fountain_regen_bonus_for_pos(config, self.hero_team, self.soldier_pos) > 0.0,
                stamina: self.hero_stamina,
                max_stamina: self.hero_max_stamina,
                stamina_regen_per_sec: self.hero_stamina_regen_per_sec,
                armor: self.hero_armor
                    - if self.flame_armor_debuff_remaining > 0.0 {
                        self.flame_armor_debuff_amount
                    } else {
                        0.0
                    },
                hero_scale: self.hero_scale,
                move_speed: self.current_move_speed,
                vision_range: self.hero_vision_range,
                active_weapon: self.active_weapon,
                weapon_kind: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.weapon_kind
                } else {
                    self.hero_weapon.weapon_kind
                },
                ammo_in_mag: self.ammo_in_mag,
                magazine_size: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.magazine_size
                } else {
                    self.hero_weapon.magazine_size
                },
                fire_rate_secs: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.fire_rate_secs
                } else {
                    self.hero_weapon.fire_rate_secs
                },
                reload_duration_secs: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.reload_duration_secs
                } else {
                    self.hero_weapon.reload_duration_secs
                },
                weapon_damage: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.damage
                } else if self.hero_weapon.weapon_kind == WeaponKind::ElectricBeam
                    || self.hero_weapon.weapon_kind == WeaponKind::FlameBeam
                {
                    self.hero_weapon.beam_dps
                } else {
                    self.hero_weapon.damage
                },
                projectile_speed: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.projectile_speed
                } else {
                    self.hero_weapon.projectile_speed
                },
                weapon_max_range: if self.active_weapon == ActiveWeaponSlot::Pistol {
                    self.hero_pistol.max_range
                } else {
                    self.hero_weapon.max_range
                },
                grenade_charges: self.grenade_charges,
                grenade_max_charges: self.hero_grenade.max_charges,
                grenade_recharge_remaining: self.grenade_recharge_remaining.max(0.0),
                grenade_throw_duration_secs: self.hero_grenade.throw_duration_secs,
                last_hits: self.last_hits,
                denies: self.denies,
                assists: self.assists,
                kills: self.kills,
                deaths: self.deaths,
                respawn_remaining: if self.is_dead { self.respawn_remaining.max(0.0) } else { 0.0 },
                inventory: self.inventory.clone(),
                tp_slot: self.tp_slot.clone(),
                courier_hp: 0.0,
                courier_max_hp: 1.0,
                courier_is_dead: false,
                courier_respawn_remaining: 0.0,
                courier_order: CourierOrder::Idle,
                courier_in_fountain: false,
                courier_inventory: Vec::new(),
                stash: Vec::new(),
                shop_notice_seq: 0,
                shop_notice_text: String::new(),
                shop_notice_is_error: false,
                teleport_cooldown_remaining: self.teleport_cooldown_remaining.max(0.0),
                is_teleporting: self.active_teleport.is_some(),
                teleport_progress: self
                    .active_teleport
                    .as_ref()
                    .map(|tp| (tp.elapsed_secs / tp.total_secs.max(0.01)).clamp(0.0, 1.0))
                    .unwrap_or(0.0),
                shadow_blade_cooldown_remaining: self.shadow_blade_cooldown_remaining.max(0.0),
                reflect_armor_cooldown_remaining: self.reflect_armor_cooldown_remaining.max(0.0),
                bullet_immunity_cooldown_remaining: self.bullet_immunity_cooldown_remaining.max(0.0),
                null_pulse_cooldown_remaining: self.null_pulse_cooldown_remaining.max(0.0),
                sonar_reveal_cooldown_remaining: self.sonar_reveal_cooldown_remaining.max(0.0),
                kinetic_dash_cooldown_remaining: self.kinetic_dash_cooldown_remaining.max(0.0),
                is_invisible: self.invisibility_remaining > 0.0,
                invisibility_remaining: self.invisibility_remaining.max(0.0),
                is_reflect_armor_active: self.reflect_armor_remaining > 0.0,
                is_bullet_immune: self.bullet_immunity_remaining > 0.0,
                is_cryo_debuffed: self.cryo_debuff_remaining > 0.0,
                is_null_muted: self.null_mute_remaining > 0.0,
                is_sonar_revealed: self.sonar_revealed_remaining > 0.0,
                active_buffs: {
                    let mut buffs = Vec::new();
                    let bh = &config.buff_hud;

                    // ۱. باف‌ها و دیباف‌های محیطی شب/روز و آب‌وهوا بر اساس هیرو و سلاح فعلی
                    if let Some(hero_def) = config.heroes.iter().find(|h| h.id == self.hero_id) {
                        let is_night = config.is_night_at(self.match_elapsed_secs);
                        let time_mod = if is_night {
                            &hero_def.environment.night
                        } else {
                            &hero_def.environment.day
                        };
                        let (time_lines, t_buffs, t_debuffs) = time_mod.describe_effects();
                        if !time_lines.is_empty() {
                            let is_debuff = t_debuffs > t_buffs;
                            let col = if is_night { bh.night_color_rgb } else { bh.day_color_rgb };
                            buffs.push(ActiveBuffSnapshot {
                                id: if is_night { "env_night".to_string() } else { "env_day".to_string() },
                                item_id: String::new(),
                                label: if is_night { "NIGHT EFFECT".to_string() } else { "DAYLIGHT EFFECT".to_string() },
                                short_tag: if is_night { "NGT".to_string() } else { "DAY".to_string() },
                                details: time_lines,
                                is_debuff,
                                is_timed: false,
                                remaining_secs: 0.0,
                                total_secs: 1.0,
                                color_rgb: col,
                            });
                        }

                        let (w_mod, w_id, w_label, w_tag, w_col) = match self.current_weather {
                            WeatherKind::Clear => (
                                &hero_def.environment.clear,
                                "env_clear",
                                "CLEAR WEATHER",
                                "SUN",
                                bh.clear_color_rgb,
                            ),
                            WeatherKind::Rainy => (
                                &hero_def.environment.rainy,
                                "env_rainy",
                                "RAINY WEATHER",
                                "RAIN",
                                bh.rainy_color_rgb,
                            ),
                            WeatherKind::Snowy => (
                                &hero_def.environment.snowy,
                                "env_snowy",
                                "SNOWY WEATHER",
                                "SNOW",
                                bh.snowy_color_rgb,
                            ),
                        };
                        let (w_lines, w_buffs, w_debuffs) = w_mod.describe_effects();
                        if !w_lines.is_empty() {
                            let is_debuff = w_debuffs > w_buffs;
                            buffs.push(ActiveBuffSnapshot {
                                id: w_id.to_string(),
                                item_id: String::new(),
                                label: w_label.to_string(),
                                short_tag: w_tag.to_string(),
                                details: w_lines,
                                is_debuff,
                                is_timed: false,
                                remaining_secs: 0.0,
                                total_secs: 1.0,
                                color_rgb: w_col,
                            });
                        }
                    }

                    // ۲. باف‌ها و دیباف‌های زمان‌دار آیتم‌ها و نبرد
                    if let Some(ref tp) = self.active_teleport {
                        buffs.push(ActiveBuffSnapshot {
                            id: "teleport".to_string(),
                            item_id: items_cfg.teleport.item_id.clone(),
                            label: "TELEPORTING".to_string(),
                            short_tag: "TP".to_string(),
                            details: vec!["+Channeling Town Portal".to_string()],
                            is_debuff: false,
                            is_timed: true,
                            remaining_secs: (tp.total_secs - tp.elapsed_secs).max(0.0),
                            total_secs: tp.total_secs.max(0.01),
                            color_rgb: items_cfg.teleport.portal_core_color_rgb,
                        });
                    }
                    if self.invisibility_remaining > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "shadow_blade".to_string(),
                            item_id: items_cfg.shadow_blade.item_id.clone(),
                            label: "SHADOW INVISIBILITY".to_string(),
                            short_tag: "INV".to_string(),
                            details: vec![
                                "+Invisible to Enemies".to_string(),
                                format!("+{:.0}% Move Speed", items_cfg.shadow_blade.invis_move_speed_bonus_pct * 100.0),
                            ],
                            is_debuff: false,
                            is_timed: true,
                            remaining_secs: self.invisibility_remaining.max(0.0),
                            total_secs: self.invisibility_total_secs.max(0.01),
                            color_rgb: items_cfg.shadow_blade.buff_bar_color_rgb,
                        });
                    }
                    if self.reflect_armor_remaining > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "reflect_armor".to_string(),
                            item_id: items_cfg.reflect_armor.item_id.clone(),
                            label: "MIRROR REFLECT".to_string(),
                            short_tag: "REF".to_string(),
                            details: vec![format!(
                                "+Reflects {:.0}% Incoming Damage",
                                items_cfg.reflect_armor.active_damage_ratio * 100.0
                            )],
                            is_debuff: false,
                            is_timed: true,
                            remaining_secs: self.reflect_armor_remaining.max(0.0),
                            total_secs: self.reflect_armor_total_secs.max(0.01),
                            color_rgb: items_cfg.reflect_armor.buff_bar_color_rgb,
                        });
                    }
                    if self.bullet_immunity_remaining > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "bullet_immunity".to_string(),
                            item_id: items_cfg.bullet_immunity.item_id.clone(),
                            label: "PHASE IMMUNITY".to_string(),
                            short_tag: "IMM".to_string(),
                            details: vec!["+100% Immune to Bullets & Beams".to_string()],
                            is_debuff: false,
                            is_timed: true,
                            remaining_secs: self.bullet_immunity_remaining.max(0.0),
                            total_secs: self.bullet_immunity_total_secs.max(0.01),
                            color_rgb: items_cfg.bullet_immunity.buff_bar_color_rgb,
                        });
                    }
                    if self.flame_armor_debuff_remaining > 0.0 && self.flame_armor_debuff_amount > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "flame_armor_debuff".to_string(),
                            item_id: String::new(),
                            label: "MELTED ARMOR".to_string(),
                            short_tag: "ARM".to_string(),
                            details: vec![format!("-{:.1} Armor", self.flame_armor_debuff_amount)],
                            is_debuff: true,
                            is_timed: true,
                            remaining_secs: self.flame_armor_debuff_remaining.max(0.0),
                            total_secs: self.flame_armor_debuff_remaining.max(0.1),
                            color_rgb: items_cfg.stat_limits.flame_debuff_bar_color_rgb,
                        });
                    }
                    if self.electric_fire_rate_slow_remaining > 0.0 && self.electric_fire_rate_slow_ratio > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "electric_fire_rate_slow".to_string(),
                            item_id: String::new(),
                            label: "SHOCKED".to_string(),
                            short_tag: "ZAP".to_string(),
                            details: vec![format!("-{:.0}% Fire Rate", self.electric_fire_rate_slow_ratio * 100.0)],
                            is_debuff: true,
                            is_timed: true,
                            remaining_secs: self.electric_fire_rate_slow_remaining.max(0.0),
                            total_secs: self.electric_fire_rate_slow_remaining.max(0.1),
                            color_rgb: items_cfg.stat_limits.electric_debuff_bar_color_rgb,
                        });
                    }
                    if self.toxic_slow_multiplier < 0.999 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "toxic_slow".to_string(),
                            item_id: String::new(),
                            label: "TOXIC MIASMA".to_string(),
                            short_tag: "TOX".to_string(),
                            details: vec![format!("-{:.0}% Move Speed", (1.0 - self.toxic_slow_multiplier) * 100.0)],
                            is_debuff: true,
                            is_timed: false,
                            remaining_secs: 0.0,
                            total_secs: 1.0,
                            color_rgb: items_cfg.stat_limits.toxic_debuff_bar_color_rgb,
                        });
                    }
                    if self.cryo_debuff_remaining > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "cryo_debuff".to_string(),
                            item_id: String::new(),
                            label: "CRYO STASIS".to_string(),
                            short_tag: "CRYO".to_string(),
                            details: vec![
                                format!("-{:.0}% Move Speed", self.cryo_slow_ratio * 100.0),
                                format!("-{:.0}% Healing & Regen", self.cryo_heal_reduction_ratio * 100.0),
                            ],
                            is_debuff: true,
                            is_timed: true,
                            remaining_secs: self.cryo_debuff_remaining.max(0.0),
                            total_secs: self.cryo_debuff_total_secs.max(0.01),
                            color_rgb: items_cfg.stat_limits.cryo_debuff_bar_color_rgb,
                        });
                    }
                    if self.null_mute_remaining > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "null_mute".to_string(),
                            item_id: String::new(),
                            label: "ITEMS MUTED".to_string(),
                            short_tag: "MUTE".to_string(),
                            details: vec!["-Active Items & Buffs Disabled".to_string()],
                            is_debuff: true,
                            is_timed: true,
                            remaining_secs: self.null_mute_remaining.max(0.0),
                            total_secs: self.null_mute_total_secs.max(0.01),
                            color_rgb: items_cfg.stat_limits.null_mute_debuff_bar_color_rgb,
                        });
                    }
                    if self.sonar_revealed_remaining > 0.0 {
                        buffs.push(ActiveBuffSnapshot {
                            id: "sonar_revealed".to_string(),
                            item_id: String::new(),
                            label: "SONAR TRACKED".to_string(),
                            short_tag: "SCAN".to_string(),
                            details: vec!["-Revealed Through Fog & Invisibility".to_string()],
                            is_debuff: true,
                            is_timed: true,
                            remaining_secs: self.sonar_revealed_remaining.max(0.0),
                            total_secs: self.sonar_revealed_total_secs.max(0.01),
                            color_rgb: items_cfg.stat_limits.sonar_debuff_bar_color_rgb,
                        });
                    }
                    buffs
                },
            },
            other_players: Vec::new(),
            top_bar_heroes: Vec::new(),
            scoreboard_heroes: Vec::new(),
            fog_visible_mask: self.fog_of_war.pack_visible_mask(),
            bullets: self.bullets_snapshot.clone(),
            grenades: self.grenades_snapshot.clone(),
            toxic_pools: self.toxic_pools_snapshot.clone(),
            electric_beams: self.electric_beams_snapshot.clone(),
            active_teleports: Vec::new(),
            creeps: Vec::new(),
            couriers: Vec::new(),
            turrets: self
                .turrets
                .iter()
                .map(|t| TurretSnapshot {
                    index: t.index,
                    team: t.team,
                    hp: t.hp,
                    max_hp: t.max_hp,
                    is_dead: t.is_dead,
                    is_invulnerable: t.is_invulnerable,
                    aim_target_pos: t.aim_target_pos,
                })
                .collect(),
            barracks: self
                .barracks
                .iter()
                .map(|b| BarracksSnapshot {
                    index: b.index,
                    team: b.team,
                    lane: b.lane,
                    kind: b.kind,
                    hp: b.hp,
                    max_hp: b.max_hp,
                    is_dead: b.is_dead,
                    is_invulnerable: b.is_invulnerable,
                })
                .collect(),
            ancients: self
                .ancients
                .iter()
                .map(|a| AncientSnapshot {
                    index: a.index,
                    team: a.team,
                    hp: a.hp,
                    max_hp: a.max_hp,
                    is_dead: a.is_dead,
                    is_invulnerable: a.is_invulnerable,
                })
                .collect(),
            recent_chat: Vec::new(),
            recent_pings: Vec::new(),
            pause: PauseSnapshot {
                phase: self.pause_phase,
                paused_by_team: self.paused_by_team,
                paused_by_name: self.paused_by_name.clone(),
                is_disconnect_pause: self.pause_is_disconnect,
                lock_remaining_secs: self.pause_lock_remaining.max(0.0),
                countdown_remaining_secs: self.unpause_countdown_remaining.max(0.0),
                your_team_pauses_left: 0,
                your_team_time_left_secs: 0.0,
                your_player_cooldown_secs: 0.0,
                can_you_unpause: false,
            },
            targets: self
                .targets
                .iter()
                .enumerate()
                .map(|(index, t)| {
                    let is_visible = self
                        .targets_visible
                        .get(index)
                        .copied()
                        .unwrap_or(true);
                    TargetSnapshot {
                        index,
                        hp: if is_visible { t.hp } else { t.max_hp },
                        max_hp: t.max_hp,
                        hit_flash_timer: if is_visible { t.hit_flash_timer } else { 0.0 },
                        is_dead: t.is_dead,
                        is_visible,
                    }
                })
                .collect(),
            fx_events: Vec::new(),
        }
    }

    pub fn take_fx_events(&mut self) -> Vec<GameplayFxEvent> {
        let mut out = std::mem::take(&mut self.pending_fx);
        out.extend(self.pending_private_fx.drain(..).map(|(_, ev)| ev));
        out
    }
}

// -----------------------------------------------------------------------------
// ۶. توابع کمکی جهت‌گیری، لیزر و سیستم‌های اجرای موتور فیزیک اختصاصی
// -----------------------------------------------------------------------------

pub fn orient_soldier_towards(soldier_tf: &mut Transform, dir: Vec3, min_dir_sq: f32) {
    let flat_dir = Vec3::new(dir.x, 0.0, dir.z);
    if flat_dir.length_squared() > min_dir_sq {
        let look_target = soldier_tf.translation + flat_dir.normalize();
        soldier_tf.look_at(look_target, Vec3::Y);
        soldier_tf.rotate_local_y(std::f32::consts::PI);
    }
}

pub fn angle_to_direction_8(degrees: f32) -> &'static str {
    if (-22.5..22.5).contains(&degrees) {
        "forward"
    } else if (22.5..67.5).contains(&degrees) {
        "forward_right"
    } else if (67.5..112.5).contains(&degrees) {
        "right"
    } else if (112.5..157.5).contains(&degrees) {
        "backward_right"
    } else if degrees >= 157.5 || degrees <= -157.5 {
        "backward"
    } else if (-157.5..-112.5).contains(&degrees) {
        "backward_left"
    } else if (-112.5..-67.5).contains(&degrees) {
        "left"
    } else {
        "forward_left"
    }
}

pub fn angle_to_direction_8_hysteresis(degrees: f32, prev_locomotion: &str) -> &'static str {
    let prev_dir = prev_locomotion
        .strip_prefix("run_")
        .or_else(|| prev_locomotion.strip_prefix("walk_crouching_"))
        .or_else(|| prev_locomotion.strip_prefix("walk_"))
        .unwrap_or("");
    let center_deg = match prev_dir {
        "forward" => Some(0.0_f32),
        "forward_right" => Some(45.0),
        "right" => Some(90.0),
        "backward_right" => Some(135.0),
        "backward" => Some(180.0),
        "backward_left" => Some(-135.0),
        "left" => Some(-90.0),
        "forward_left" => Some(-45.0),
        _ => None,
    };
    if let Some(c) = center_deg {
        let mut diff = (degrees - c).abs();
        if diff > 180.0 {
            diff = 360.0 - diff;
        }
        // هیسترزیس ۸ درجه‌ای برای جلوگیری از لرزش انیمیشن روی مرزهای ۲۲.۵ و ۴۵ درجه
        if diff <= 22.5 + 8.0 {
            return match prev_dir {
                "forward" => "forward",
                "forward_right" => "forward_right",
                "right" => "right",
                "backward_right" => "backward_right",
                "backward" => "backward",
                "backward_left" => "backward_left",
                "left" => "left",
                "forward_left" => "forward_left",
                _ => angle_to_direction_8(degrees),
            };
        }
    }
    angle_to_direction_8(degrees)
}

pub fn compute_laser_hit<F>(
    physics_world: &CustomPhysicsWorld,
    muzzle_pos: Vec3,
    raw_target: Vec3,
    active_target_predicate: F,
) -> Vec3
where
    F: Fn(Entity) -> bool,
{
    let to_target = raw_target - muzzle_pos;
    let max_dist = to_target.length();
    if max_dist <= f32::EPSILON {
        return raw_target;
    }
    let dir = to_target / max_dist;
    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    if let Some((_entity, hit)) =
        physics_world.cast_ray(muzzle_pos, dir, max_dist, query_groups, active_target_predicate)
    {
        hit.point
    } else {
        raw_target
    }
}

pub fn reconcile_soldier_physics_pos(
    soldier_physics_tf: &mut Transform,
    authoritative_feet_pos: Vec3,
    hero_scale: f32,
    config: &GameConfig,
) {
    let auth_capsule_pos = Vec3::new(
        authoritative_feet_pos.x,
        authoritative_feet_pos.y + config.physics.soldier_capsule_center_y * hero_scale,
        authoritative_feet_pos.z,
    );
    let dist = soldier_physics_tf.translation.distance(auth_capsule_pos);
    if dist > config.physics.position_reconcile_snap_dist {
        soldier_physics_tf.translation = auth_capsule_pos;
    } else {
        soldier_physics_tf.translation = soldier_physics_tf
            .translation
            .lerp(auth_capsule_pos, config.physics.position_reconcile_lerp_factor);
    }
}


/// مرکز فاونتن پایگاه تیم خودی بر اساس config.ron (برای هدف بازگشت کوریر)
pub fn team_fountain_pos(config: &GameConfig, team: TeamId) -> Option<Vec3> {
    if team == TeamId::Neutral {
        return None;
    }
    config
        .map
        .fountains
        .iter()
        .find(|ftn| ftn.team == team)
        .map(|ftn| ftn.model_transform.location_vec())
}

/// محاسبه بونوس بازسازی سلامتی در چشمه پایگاه تیم خودی بر اساس config.ron
/// فاصله روی صفحه افقی (XZ) سنجیده می‌شود تا ارتفاع پرواز Courier باعث خروج کاذب از محدوده شاپ نشود.
pub fn fountain_regen_bonus_for_pos(config: &GameConfig, team: TeamId, pos: Vec3) -> f32 {
    if team == TeamId::Neutral {
        return 0.0;
    }
    let mut best_bonus = 0.0_f32;
    for ftn in &config.map.fountains {
        if ftn.team != team {
            continue;
        }
        let ftn_pos = ftn.model_transform.location_vec();
        let dist_xz = Vec2::new(pos.x - ftn_pos.x, pos.z - ftn_pos.z).length();
        if dist_xz <= ftn.heal_radius {
            best_bonus = best_bonus.max(ftn.heal_per_sec);
        }
    }
    best_bonus
}

/// محاسبه آسیب دفاعی چشمه تیم حریف برای جلوگیری از Spawn Camping
pub fn enemy_fountain_damage_for_pos(config: &GameConfig, hero_team: TeamId, pos: Vec3) -> f32 {
    if hero_team == TeamId::Neutral {
        return 0.0;
    }
    let mut best_dps = 0.0_f32;
    for ftn in &config.map.fountains {
        if ftn.team == hero_team || ftn.team == TeamId::Neutral {
            continue;
        }
        let ftn_pos = ftn.model_transform.location_vec();
        let dist_xz = Vec2::new(pos.x - ftn_pos.x, pos.z - ftn_pos.z).length();
        if dist_xz <= ftn.heal_radius {
            best_dps = best_dps.max(ftn.enemy_damage_per_sec);
        }
    }
    best_dps
}

/// مکانیک Dota 2 Attack-Reveal: با شلیک به هر واحد دشمن یا کمپ جنگلی، مکان مهاجم برای مدت مشخصی برای آن تیم/کمپ لو می‌رود
fn mark_attacker_revealed_to_team(
    world: &mut AuthoritativeWorldState,
    attacker_slot_opt: Option<usize>,
    victim_team: TeamId,
    config: &GameConfig,
) {
    let Some(attacker_slot) = attacker_slot_opt else {
        return;
    };
    let dur = config.combat.attack_reveal_duration_secs;
    if let Some(attacker) = world.players.iter_mut().find(|p| p.slot_index == attacker_slot) {
        if attacker.hero_team == victim_team {
            return;
        }
        match victim_team {
            TeamId::Dawn => {
                attacker.attack_reveal_to_dawn_remaining =
                    attacker.attack_reveal_to_dawn_remaining.max(dur);
            }
            TeamId::Dusk => {
                attacker.attack_reveal_to_dusk_remaining =
                    attacker.attack_reveal_to_dusk_remaining.max(dur);
            }
            TeamId::Neutral => {
                attacker.attack_reveal_to_neutral_remaining =
                    attacker.attack_reveal_to_neutral_remaining.max(dur);
            }
        }
    }
}

/// بیدار کردن کل سربازان کمپ جنگلی به‌طور هم‌زمان و ثبت هدف مهاجم + فعال‌سازی Attack-Reveal
fn aggro_creep_and_camp_on_hit(
    world: &mut AuthoritativeWorldState,
    hit_creep_idx: usize,
    attacker_slot_opt: Option<usize>,
    config: &GameConfig,
) {
    let victim_team = world.creeps[hit_creep_idx].team;
    mark_attacker_revealed_to_team(world, attacker_slot_opt, victim_team, config);
    if let Some(hit_camp_idx) = world.creeps[hit_creep_idx].camp_index {
        for c in &mut world.creeps {
            if c.camp_index == Some(hit_camp_idx) && !c.is_dead {
                c.is_aggroed = true;
                if let Some(a_slot) = attacker_slot_opt {
                    c.aggro_target_slot = Some(a_slot);
                }
            }
        }
    }
}

fn record_hero_damage_on_victim(
    world: &mut AuthoritativeWorldState,
    victim_idx: usize,
    attacker_slot_opt: Option<usize>,
    config: &GameConfig,
) {
    let Some(attacker_slot) = attacker_slot_opt else {
        return;
    };
    let now = world.match_elapsed_secs;
    let window = config.combat.hero_assist_window_secs;
    let victim_team = world.players[victim_idx].hero_team;
    let attacker_pos = if let Some(attacker) = world.players.iter_mut().find(|p| p.slot_index == attacker_slot) {
        if attacker.hero_team == victim_team {
            return;
        }
        attacker.last_attacked_enemy_hero_at = now;
        attacker.soldier_pos
    } else {
        return;
    };
    mark_attacker_revealed_to_team(world, attacker_slot_opt, victim_team, config);

    // سیستم Creep Aggro مشابه Dota 2: سربازهای لاین تیم آسیب‌دیده در شعاع aggro هدفشان مهاجم می‌شود
    let aggro_r_sq = config.combat.creep_hero_aggro_radius * config.combat.creep_hero_aggro_radius;
    let max_y_diff = config.map.high_ground_occlusion_min_diff * 2.5;
    for c in &mut world.creeps {
        if c.is_dead || c.team != victim_team || c.camp_index.is_some() {
            continue;
        }
        if c.hero_aggro_cooldown_secs > 0.0 {
            continue;
        }
        if (c.pos.y - attacker_pos.y).abs() > max_y_diff {
            continue;
        }
        let d_xz_sq = Vec2::new(c.pos.x - attacker_pos.x, c.pos.z - attacker_pos.z).length_squared();
        if d_xz_sq <= aggro_r_sq {
            c.hero_aggro_target_slot = Some(attacker_slot);
            c.hero_aggro_remaining_secs = config.combat.creep_hero_aggro_duration_secs;
            c.hero_aggro_cooldown_secs = config.combat.creep_hero_aggro_duration_secs
                + config.combat.creep_hero_aggro_cooldown_secs;
        }
    }
    let damagers = &mut world.players[victim_idx].recent_hero_damagers;
    damagers.retain(|&(_, ts)| (now - ts) <= window);
    if let Some(entry) = damagers.iter_mut().find(|(s, _)| *s == attacker_slot) {
        entry.1 = now;
    } else {
        damagers.push((attacker_slot, now));
    }
}

pub fn grant_xp_to_player_slot(
    p: &mut PlayerSlotState,
    amount: u32,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    pending_private_fx: &mut Vec<(usize, GameplayFxEvent)>,
) {
    if amount == 0 || p.hero_level >= 25 {
        return;
    }
    p.hero_xp = p.hero_xp.saturating_add(amount);
    let growth_hp_per_level = config.active_hero(p.hero_index).level_growth.max_hp_per_level;
    let mut levels_up = 0_u32;

    while p.hero_level < 25 {
        let next_idx = p.hero_level as usize;
        if p.hero_xp >= config.combat.xp_thresholds_per_level[next_idx] {
            p.hero_level += 1;
            levels_up += 1;
            pending_private_fx.push((
                p.slot_index,
                GameplayFxEvent::HeroLeveledUp {
                    pos: p.soldier_pos,
                    new_level: p.hero_level,
                },
            ));
        } else {
            break;
        }
    }

    if levels_up > 0 {
        let prev_hp = p.hero_hp;
        crate::shop_shared::recompute_player_slot_stats(p, config, items_cfg);
        if !p.is_dead {
            p.hero_hp = (prev_hp + growth_hp_per_level * (levels_up as f32)).min(p.hero_max_hp);
        } else {
            p.hero_hp = 0.0;
        }
    }
}

/// محاسبه ضریب کامبک/تعدیل بر اساس اختلاف لول قربانی و مهاجم (کاملاً از config.ron)
#[inline]
fn compute_level_diff_bounty_multipliers(
    victim_level: u32,
    earner_level: u32,
    c: &CombatConfig,
) -> (f32, f32) {
    let diff = (victim_level as i32) - (earner_level as i32);
    if diff > 0 {
        let d = diff as f32;
        let g_mult = (1.0 + d * c.underdog_gold_mult_per_level)
            .clamp(c.min_bounty_gold_mult, c.max_bounty_gold_mult);
        let x_mult = (1.0 + d * c.underdog_xp_mult_per_level)
            .clamp(c.min_bounty_xp_mult, c.max_bounty_xp_mult);
        (g_mult, x_mult)
    } else if diff < 0 {
        let d = (-diff) as f32;
        let g_mult = (1.0 - d * c.overlevel_gold_penalty_per_level)
            .clamp(c.min_bounty_gold_mult, 1.0);
        let x_mult = (1.0 - d * c.overlevel_xp_penalty_per_level)
            .clamp(c.min_bounty_xp_mult, 1.0);
        (g_mult, x_mult)
    } else {
        (1.0, 1.0)
    }
}

fn award_hero_kill_rewards(
    world: &mut AuthoritativeWorldState,
    victim_idx: usize,
    killer_slot_opt: Option<usize>,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
) {
    let c = &config.combat;
    let victim_name = world.players[victim_idx].hero_display_name.clone();
    let victim_team = world.players[victim_idx].hero_team;
    let victim_pos = world.players[victim_idx].soldier_pos;
    let victim_level = world.players[victim_idx].hero_level.max(1);
    let victim_streak = world.players[victim_idx].kill_streak;
    world.players[victim_idx].kill_streak = 0;

    let now = world.match_elapsed_secs;
    let assist_window = c.hero_assist_window_secs;
    let credit_window = c.hero_kill_credit_window_secs;

    let recent_damagers: Vec<(usize, f32)> = world.players[victim_idx]
        .recent_hero_damagers
        .iter()
        .copied()
        .filter(|&(_, ts)| (now - ts) <= assist_window)
        .collect();
    world.players[victim_idx].recent_hero_damagers.clear();

    // اگر ضربه آخر توسط تاور/کریپ/چشمه بود، آخرین هیروی دشمن که در پنجره زمانی آسیب زده کیل را می‌گیرد
    let effective_killer_slot = killer_slot_opt.or_else(|| {
        recent_damagers
            .iter()
            .filter(|&&(_, ts)| (now - ts) <= credit_window)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|&(s, _)| s)
    });

    let victim_lvl_above_1 = victim_level.saturating_sub(1) as f32;
    let base_kill_gold =
        (c.hero_kill_gold as f32) + victim_lvl_above_1 * c.hero_kill_gold_per_victim_level;
    let base_kill_xp =
        (c.hero_kill_xp as f32) + victim_lvl_above_1 * c.hero_kill_xp_per_victim_level;
    let base_assist_gold =
        (c.hero_assist_gold as f32) + victim_lvl_above_1 * c.hero_assist_gold_per_victim_level;
    let base_assist_xp =
        (c.hero_assist_xp as f32) + victim_lvl_above_1 * c.hero_assist_xp_per_victim_level;

    // پاداش قطع کردن Kill Streak حریف (Shutdown Bounty)
    let streak_steps = if victim_streak >= c.streak_bonus_min_kills {
        (victim_streak - c.streak_bonus_min_kills + 1).min(c.streak_bonus_max_steps)
    } else {
        0
    };
    let streak_gold_bonus = streak_steps.saturating_mul(c.streak_gold_bonus_per_kill);
    let streak_xp_bonus = streak_steps.saturating_mul(c.streak_xp_bonus_per_kill);

    let mut rewards: Vec<HeroKillRewardEntry> = Vec::new();
    let mut killer_name = if victim_team == TeamId::Dawn {
        "Dusk".to_string()
    } else {
        "Dawn".to_string()
    };
    let mut killer_team = if victim_team == TeamId::Dawn {
        TeamId::Dusk
    } else {
        TeamId::Dawn
    };

    if let Some(k_slot) = effective_killer_slot {
        let mut awarded_kill_gold = 0_u32;
        let mut killer_level_diff = 0_i32;

        if let Some(attacker) = world.players.iter_mut().find(|p| p.slot_index == k_slot) {
            let attacker_level = attacker.hero_level.max(1);
            killer_level_diff = (victim_level as i32) - (attacker_level as i32);
            let (g_mult, x_mult) =
                compute_level_diff_bounty_multipliers(victim_level, attacker_level, c);

            let kill_gold = ((base_kill_gold * g_mult).round().max(1.0) as u32)
                .saturating_add(streak_gold_bonus);
            let kill_xp = ((base_kill_xp * x_mult).round().max(1.0) as u32)
                .saturating_add(streak_xp_bonus);
            awarded_kill_gold = kill_gold;

            attacker.kills += 1;
            attacker.kill_streak = attacker.kill_streak.saturating_add(1);
            attacker.hero_gold = attacker.hero_gold.saturating_add(kill_gold);
            killer_name = attacker.hero_display_name.clone();
            killer_team = attacker.hero_team;
            grant_xp_to_player_slot(
                attacker,
                kill_xp,
                config,
                items_cfg,
                &mut world.pending_private_fx,
            );
            rewards.push(HeroKillRewardEntry {
                player_name: attacker.hero_display_name.clone(),
                is_killer: true,
                gold: kill_gold,
                xp: kill_xp,
            });
        }

        if awarded_kill_gold > 0 {
            world.pending_private_fx.push((
                k_slot,
                GameplayFxEvent::LastHitGold {
                    pos: victim_pos,
                    gold: awarded_kill_gold,
                },
            ));
        }

        if streak_steps > 0 {
            world.pending_fx.push(GameplayFxEvent::CombatPopup {
                pos: victim_pos + Vec3::Y * 2.4,
                text: format!("SHUTDOWN +{}g!", streak_gold_bonus),
                color_rgb: (1.0, 0.78, 0.20),
            });
        } else if killer_level_diff >= 3 {
            world.pending_fx.push(GameplayFxEvent::CombatPopup {
                pos: victim_pos + Vec3::Y * 2.4,
                text: "COMEBACK BONUS!".to_string(),
                color_rgb: (0.35, 0.96, 0.72),
            });
        }

        // اعطای Assist پویا به هم‌تیمی‌ها (متناسب با لول قربانی و اختلاف لول هر Assister)
        let mut assister_indices: Vec<usize> = Vec::new();
        for (idx, p) in world.players.iter().enumerate() {
            if p.slot_index == k_slot || p.hero_team != killer_team {
                continue;
            }
            let dealt_recent_damage = recent_damagers.iter().any(|&(s, _)| s == p.slot_index);
            let nearby_alive =
                !p.is_dead && p.soldier_pos.distance(victim_pos) <= p.hero_xp_gain_radius;
            if dealt_recent_damage || nearby_alive {
                assister_indices.push(idx);
            }
        }
        for idx in assister_indices {
            let p = &mut world.players[idx];
            let (ag_mult, ax_mult) =
                compute_level_diff_bounty_multipliers(victim_level, p.hero_level.max(1), c);
            let assist_gold = (base_assist_gold * ag_mult).round().max(1.0) as u32;
            let assist_xp = (base_assist_xp * ax_mult).round().max(1.0) as u32;
            p.assists += 1;
            p.hero_gold = p.hero_gold.saturating_add(assist_gold);
            let pname = p.hero_display_name.clone();
            grant_xp_to_player_slot(
                p,
                assist_xp,
                config,
                items_cfg,
                &mut world.pending_private_fx,
            );
            rewards.push(HeroKillRewardEntry {
                player_name: pname,
                is_killer: false,
                gold: assist_gold,
                xp: assist_xp,
            });
        }
    }

    world.pending_fx.push(GameplayFxEvent::HeroKillFeed {
        killer_name,
        killer_team,
        victim_name,
        victim_team,
        rewards,
    });
}

fn trigger_random_death(world: &mut AuthoritativeWorldState, config: &GameConfig) {
    let count = config.soldier.death_animations.len();
    assert!(count > 0, "لیست death_animations در کانفیگ خالی است!");
    let idx = ((world.next_rand() * count as f32) as usize).min(count - 1);
    let chosen = config.soldier.death_animations[idx].clone();
    let _duration = config.exact_anim_duration(&chosen);
    let lvl_idx = (world.hero_level as usize).saturating_sub(1).min(24);
    let dota_respawn_secs = config.combat.respawn_times_per_level[lvl_idx];

    world.is_dead = true;
    world.hero_hp = 0.0;
    world.deaths += 1;
    world.death_anim_name = chosen;
    world.respawn_remaining = dota_respawn_secs;
    if world.invisibility_remaining > 0.0 {
        world.invisibility_remaining = 0.0;
        world.invisibility_total_secs = 0.0;
        world.invis_move_speed_bonus_pct = 0.0;
        world.is_revealed_by_turret = false;
        world.pending_fx.push(GameplayFxEvent::ShadowBladeEnded {
            slot_index: world.active_slot,
            pos: world.soldier_pos,
        });
    }
    if let Some(tp) = world.active_teleport.take() {
        world.pending_fx.push(GameplayFxEvent::TeleportCanceled {
            slot_index: world.active_slot,
            origin_pos: tp.origin_pos,
        });
    }
    world.reload_remaining = None;
    world.grenade_elapsed_and_duration = None;
    world.firing_anim_remaining = None;
    world.space_hold_secs = None;
    world.active_beam_request = None;
    world.traversal.is_active = false;
    world.wall_climb_mountain_index = None;
    world.in_water = false;
    world.flame_armor_debuff_remaining = 0.0;
    world.flame_armor_debuff_amount = 0.0;
    world.electric_fire_rate_slow_remaining = 0.0;
    world.electric_fire_rate_slow_ratio = 0.0;
    world.toxic_slow_multiplier = 1.0;
    world.reflect_armor_remaining = 0.0;
    world.bullet_immunity_remaining = 0.0;
    world.cryo_debuff_remaining = 0.0;
    world.cryo_slow_ratio = 0.0;
    world.cryo_heal_reduction_ratio = 0.0;
    world.null_mute_remaining = 0.0;
    world.sonar_revealed_remaining = 0.0;
    world.kinetic_dash_remaining = 0.0;
}

fn resolve_muzzle_pos(
    world: &AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
) -> Vec3 {
    let hero_scale = config.active_hero(world.hero_index).hero_scale.max(0.25);
    let fallback = world.soldier_pos
        + Vec3::new(0.0, config.combat.fallback_muzzle_offset_y * hero_scale, 0.0)
        + world.aim_world_dir * (config.combat.fallback_muzzle_forward * hero_scale);

    let Some(raw_offset) = input.client_muzzle_world_pos else {
        return fallback;
    };
    if !raw_offset.is_finite() {
        return fallback;
    }

    // محدودسازی شعاع افقی و ارتفاع سر لوله برای حفظ Server Authority
    let max_horiz = (config.combat.fallback_muzzle_forward * 2.0 * hero_scale).max(1.8 * hero_scale);
    let mut horiz = Vec2::new(raw_offset.x, raw_offset.z);
    let horiz_len = horiz.length();
    if horiz_len > max_horiz && horiz_len > f32::EPSILON {
        horiz *= max_horiz / horiz_len;
    }
    let clamped_y = raw_offset.y.clamp(0.35 * hero_scale, 2.25 * hero_scale);
    let candidate = world.soldier_pos + Vec3::new(horiz.x, clamped_y, horiz.y);

    // بررسی برخورد با دیوار بین سینه هیرو و سر لوله تا کلاینت نتواند از پشت دیوار شلیک کند
    let chest_y = if world.effective_stance == Stance::Crouch {
        config.combat.hero_crouch_aim_chest_height
    } else {
        config.combat.hero_stand_aim_chest_height
    } * hero_scale;
    let chest_pos = world.soldier_pos + Vec3::new(0.0, chest_y, 0.0);
    let to_muzzle = candidate - chest_pos;
    let dist = to_muzzle.length();
    if dist > 0.05 {
        let dir = to_muzzle / dist;
        let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);
        if let Some((_ent, hit)) = physics_world.cast_ray(chest_pos, dir, dist, query_groups, |_| true) {
            return chest_pos + dir * (hit.toi - 0.08).max(0.0);
        }
    }

    candidate
}

fn spawn_shared_physics_actors_system(mut commands: Commands, config: Res<GameConfig>) {
    commands.spawn(soldier_physics_bundle(&config));
}

fn spawn_server_environment_colliders_system(mut commands: Commands, config: Res<GameConfig>) {
    commands.spawn(ground_physics_bundle(&config));

    for plat in &config.map.terrains {
        commands.spawn(terrain_platform_physics_bundle(&config, plat));
    }

    for ramp in &config.map.ramps {
        commands.spawn(ramp_physics_bundle(&config, ramp));
    }

    commands.spawn(center_bridge_physics_bundle(
        &config,
        &config.map.center_bridge,
    ));

    for pit_bundle in roshan_pit_physics_bundles(&config) {
        commands.spawn(pit_bundle);
    }

    for lane in &config.map.lanes {
        let seg_count = lane.waypoints.len().saturating_sub(1);
        let last_seg_idx = seg_count.saturating_sub(1);
        for seg_idx in 0..seg_count {
            let a = lane.waypoints[seg_idx];
            let b = lane.waypoints[seg_idx + 1];
            let start = Vec3::new(a.0, a.1, a.2);
            let end = Vec3::new(b.0, b.1, b.2);
            if start.distance(end) > 0.1 {
                if seg_idx == 0 || seg_idx == last_seg_idx {
                    commands.spawn(entrance_stair_physics_bundle(
                        &config,
                        format!("{}_seg_{}", lane.id, seg_idx),
                        start,
                        end,
                        lane.width,
                    ));
                } else {
                    commands.spawn(lane_segment_physics_bundle(
                        &config,
                        format!("{}_seg_{}", lane.id, seg_idx),
                        start,
                        end,
                        lane.width,
                        lane.thickness,
                    ));
                }
            }
        }
    }

    for mtn in &config.map.mountains {
        commands.spawn(mountain_physics_bundle(&config, mtn));
    }

    for tree in &config.map.trees {
        commands.spawn(tree_physics_bundle(&config, tree));
    }

    for (index, turret) in config.map.turrets.iter().enumerate() {
        commands.spawn(turret_physics_bundle(&config, index, turret));
    }

    for (index, barrack) in config.map.barracks.iter().enumerate() {
        commands.spawn(barracks_physics_bundle(&config, index, barrack));
    }

    for (index, ancient) in config.map.ancients.iter().enumerate() {
        commands.spawn(ancient_physics_bundle(&config, index, ancient));
    }

    for obs in &config.obstacles {
        commands.spawn(obstacle_physics_bundle(&config, obs));
    }

    for (index, target) in config.targets.iter().enumerate() {
        commands.spawn(target_physics_bundle(&config, index, target));
    }
}

/// همگام‌سازی لیست کولایدرهای ثابت نقشه با CustomPhysicsWorld در ابتدای هر تیک فیزیک
fn sync_static_colliders_to_world_system(
    config: Res<GameConfig>,
    mut physics_world: ResMut<CustomPhysicsWorld>,
    static_query: Query<(
        Entity,
        &Transform,
        &CustomRigidBody,
        &CustomCollider,
        &SurfaceMaterial,
        &CollisionGroups,
        Has<ColliderDisabled>,
        Option<&PhysicsMountain>,
    )>,
) {
    physics_world.static_colliders.clear();
    physics_world.canyon = if config.map.canyon.enabled {
        Some(config.map.canyon.clone())
    } else {
        None
    };
    for (entity, tf, body, collider, mat, groups, disabled, mtn_opt) in &static_query {
        if *body != CustomRigidBody::Fixed {
            continue;
        }
        let rot = tf.rotation.normalize();
        physics_world.static_colliders.push(StaticColliderEntry {
            entity,
            translation: tf.translation,
            rotation: rot,
            inv_rotation: rot.inverse(),
            collider: collider.clone(),
            bounding_radius: collider.bounding_sphere_radius(),
            material: *mat,
            groups: *groups,
            disabled,
            is_non_walkable_wall: mtn_opt.is_some_and(|m| m.id.starts_with("mtn_canyon_")),
        });
    }
}

/// گام شبیه‌سازی موتور فیزیک اختصاصی (حرکت کپسول کاراکتر + اجسام دینامیک نارنجک/پوکه/ذرات)
fn step_custom_physics_simulation_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    physics_world: Res<CustomPhysicsWorld>,
    mut soldier_query: Query<
        (&mut Transform, &mut CustomCharacterController),
        (With<PhysicsSoldier>, Without<DynamicBodyProps>),
    >,
    mut dynamic_query: Query<
        (
            Entity,
            &mut Transform,
            &mut Velocity,
            &CustomCollider,
            &DynamicBodyProps,
            &CollisionGroups,
        ),
        Without<PhysicsSoldier>,
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    // ۱. اجرای کنترلر حرکتی کاراکتر
    for (mut soldier_tf, mut controller) in &mut soldier_query {
        if let Some(desired_delta) = controller.translation.take() {
            let (new_center, grounded) =
                physics_world.move_character_capsule(soldier_tf.translation, desired_delta, &config);
            soldier_tf.translation = new_center;
            controller.grounded = grounded;
        }
    }

    // ۲. اجرای شبیه‌سازی اجسام دینامیک (نارنجک‌ها، پوکه‌ها و ذرات) با Substeps
    let substeps = config.physics.substeps.max(1);
    let sub_dt = dt / (substeps as f32);
    let gravity = Vec3::new(0.0, config.physics.gravity_y, 0.0);

    for _ in 0..substeps {
        let mut grenade_positions: Vec<(Entity, Vec3, f32)> = Vec::new();

        for (entity, mut tf, mut vel, collider, props, groups) in &mut dynamic_query {
            vel.linear += gravity * props.gravity_scale * sub_dt;
            let lin_damp = (1.0 - props.linear_damping * sub_dt).clamp(0.0, 1.0);
            let ang_damp = (1.0 - props.angular_damping * sub_dt).clamp(0.0, 1.0);
            vel.linear *= lin_damp;
            vel.angular *= ang_damp;

            let ang_speed = vel.angular.length();
            if ang_speed > 1e-5 {
                let axis = vel.angular / ang_speed;
                let dq = Quat::from_axis_angle(axis, ang_speed * sub_dt);
                tf.rotation = (dq * tf.rotation).normalize();
            }

            let radius = collider.effective_ball_radius();
            let step_vec = vel.linear * sub_dt;
            let step_len = step_vec.length();

            if props.ccd_enabled && step_len > radius * 0.65 {
                if let Some((_hit_ent, hit)) = physics_world.cast_ray(
                    tf.translation,
                    step_vec / step_len,
                    step_len + radius,
                    *groups,
                    |_| true,
                ) {
                    tf.translation = hit.point + hit.normal * radius;
                    let vn = vel.linear.dot(hit.normal);
                    if vn < 0.0 {
                        let normal_vel = hit.normal * vn;
                        let tangent_vel = vel.linear - normal_vel;
                        vel.linear = tangent_vel * (1.0 - props.friction).clamp(0.0, 1.0)
                            - normal_vel * props.restitution;
                        vel.angular *= 0.85;
                    }
                    if (groups.memberships & GROUP_GRENADE) != 0 {
                        grenade_positions.push((entity, tf.translation, radius));
                    }
                    continue;
                }
            }

            tf.translation += step_vec;

            for entry in &physics_world.static_colliders {
                if entry.disabled || !groups.interacts_with(entry.groups) {
                    continue;
                }
                let max_r = entry.bounding_radius + radius;
                if tf.translation.distance_squared(entry.translation) > max_r * max_r {
                    continue;
                }

                if let Some(contact) = sphere_vs_static_collider(tf.translation, radius, entry) {
                    tf.translation += contact.normal * contact.penetration;
                    let vn = vel.linear.dot(contact.normal);
                    if vn < 0.0 {
                        let combined_restitution = props.restitution.max(entry.material.restitution);
                        let combined_friction =
                            0.5 * (props.friction + entry.material.friction);
                        let normal_vel = contact.normal * vn;
                        let tangent_vel = vel.linear - normal_vel;
                        vel.linear = tangent_vel * (1.0 - combined_friction * 0.35).clamp(0.0, 1.0)
                            - normal_vel * combined_restitution;

                        if contact.normal.y > 0.7 && vel.linear.y.abs() < 0.25 {
                            vel.linear.y = 0.0;
                        }
                    }
                }
            }

            if (groups.memberships & GROUP_GRENADE) != 0 {
                grenade_positions.push((entity, tf.translation, radius));
            }
        }

        if grenade_positions.len() > 1 {
            for i in 0..grenade_positions.len() {
                for j in (i + 1)..grenade_positions.len() {
                    let (e1, p1, r1) = grenade_positions[i];
                    let (e2, p2, r2) = grenade_positions[j];
                    let diff = p1 - p2;
                    let dist = diff.length();
                    let min_d = r1 + r2;
                    if dist < min_d && dist > 1e-5 {
                        let n = diff / dist;
                        let push = (min_d - dist) * 0.5;
                        if let Ok([mut g1, mut g2]) = dynamic_query.get_many_mut([e1, e2]) {
                            g1.1.translation += n * push;
                            g2.1.translation -= n * push;
                            let rel_v = (g1.2.linear - g2.2.linear).dot(n);
                            if rel_v < 0.0 {
                                let impulse = n * (rel_v * 0.75);
                                g1.2.linear -= impulse;
                                g2.2.linear += impulse;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn shared_authoritative_drive_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_cfg: Res<crate::shop_shared::ItemsConfig>,
    match_state: Option<Res<AuthoritativeMatchState>>,
    physics_world: Res<CustomPhysicsWorld>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut soldier_query: Query<(&PhysicsSoldier, &mut Transform, &mut CustomCharacterController)>,
    obstacle_query: Query<(&PhysicsObstacle, &Transform), Without<PhysicsSoldier>>,
    target_query: Query<&PhysicsTarget>,
    turret_query: Query<&PhysicsTurret>,
    barracks_query: Query<&PhysicsBarracks>,
    ancient_query: Query<&PhysicsAncient>,
    mountain_query: Query<&PhysicsMountain>,
    map_obj_query: Query<&MapObjectId>,
    elev_query: Query<&ElevationSurface>,
) {
    if match_state
        .as_ref()
        .is_some_and(|m| m.phase != MatchPhase::InGame)
    {
        return;
    }
    let dt = time.delta_secs();
    world.step_pause_system(dt, &config);
    for p in &mut world.players {
        if p.is_dead {
            p.attack_reveal_to_dawn_remaining = 0.0;
            p.attack_reveal_to_dusk_remaining = 0.0;
            p.attack_reveal_to_neutral_remaining = 0.0;
        } else {
            p.attack_reveal_to_dawn_remaining = (p.attack_reveal_to_dawn_remaining - dt).max(0.0);
            p.attack_reveal_to_dusk_remaining = (p.attack_reveal_to_dusk_remaining - dt).max(0.0);
            p.attack_reveal_to_neutral_remaining =
                (p.attack_reveal_to_neutral_remaining - dt).max(0.0);
        }
    }
    if world.winning_team.is_some() || world.pause_phase != PausePhase::Running {
        for (_, _, mut controller) in &mut soldier_query {
            controller.translation = None;
        }
        return;
    }
    let hero_aim_targets: Vec<(usize, TeamId, Vec3, f32, Stance, bool, bool, bool)> = world
        .players
        .iter()
        .map(|p| {
            let hidden_invis = p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret;
            let water_vis_dawn = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dawn,
                &config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            let water_vis_dusk = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dusk,
                &config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            (
                p.slot_index,
                p.hero_team,
                p.soldier_pos,
                p.hero_scale,
                p.effective_stance,
                p.is_dead || hidden_invis,
                water_vis_dawn,
                water_vis_dusk,
            )
        })
        .collect();

    for (soldier_marker, mut soldier_tf, mut controller) in &mut soldier_query {
        let slot_idx = soldier_marker.slot_index;
        let has_slots = !world.players.is_empty();
        let mut auto_courier_move: Option<Vec3> = None;
        if has_slots {
            if let Some(p) = world.players.get_mut(slot_idx) {
                if p.pending_hero_to_courier_slot.is_some() {
                    let manual_move = p.active_input.as_ref().is_some_and(|inp| {
                        inp.move_input.is_finite()
                            && (inp.move_input.x.abs() > 0.01 || inp.move_input.z.abs() > 0.01)
                    });
                    if p.is_dead || p.courier_is_dead || p.active_teleport.is_some() || manual_move {
                        p.pending_hero_to_courier_slot = None;
                    } else {
                        let delta = Vec3::new(
                            p.courier_pos.x - p.soldier_pos.x,
                            0.0,
                            p.courier_pos.z - p.soldier_pos.z,
                        );
                        if delta.length() > 0.2 {
                            auto_courier_move = Some(delta.normalize());
                        }
                    }
                }
            }
            world.swap_slot_in(slot_idx);
        }
        if let Some(mut input) = world.active_input.clone() {
            if let Some(dir) = auto_courier_move {
                input.move_input = dir;
            }
            shared_drive_single_player(
                &mut commands,
                dt,
                &config,
                &items_cfg,
                &physics_world,
                &mut world,
                &input,
                &mut soldier_tf,
                &mut controller,
                &obstacle_query,
                &target_query,
                &turret_query,
                &barracks_query,
                &ancient_query,
                &mountain_query,
                &map_obj_query,
                &elev_query,
                &hero_aim_targets,
            );
        }
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
    }

    // ۱. حل برخورد حرکت هیرو با سربازان پیش از حرکت سربازان:
    // هیرو هرگز نمی‌تواند سربازها را هل بدهد یا کنار بزند؛ سرباز برای هیرو یک مانع کاملاً صلب است.
    if !world.creeps.is_empty() && !world.players.is_empty() {
        let base_r = config.soldier.player_radius;
        let world_inner = &mut *world;
        for p in &mut world_inner.players {
            if p.is_dead {
                continue;
            }
            for c in &world_inner.creeps {
                let c_stats = resolve_creep_stats(&config, c.creep_type_id.as_deref());
                let min_dist = base_r * p.hero_scale + c_stats.collider_radius;
                if c.is_dead || (p.soldier_pos.y - c.pos.y).abs() > 1.8 {
                    continue;
                }
                let delta = Vec2::new(p.soldier_pos.x - c.pos.x, p.soldier_pos.z - c.pos.z);
                let dist = delta.length();
                if dist < min_dist {
                    let push_dir = if dist > 0.0001 {
                        delta / dist
                    } else {
                        Vec2::new(1.0, 0.0)
                    };
                    p.soldier_pos.x = c.pos.x + push_dir.x * min_dist;
                    p.soldier_pos.z = c.pos.z + push_dir.y * min_dist;
                }
            }
        }
        for (soldier_marker, mut soldier_tf, _) in &mut soldier_query {
            let p = &world.players[soldier_marker.slot_index];
            soldier_tf.translation.x = p.soldier_pos.x;
            soldier_tf.translation.z = p.soldier_pos.z;
        }
    }

    // تولید موج سربازان هر لاین (دقیقاً پس از دقیقه‌ی اول، هر ۳۰ ثانیه) و هوش مصنوعی حرکت/شلیک
    step_lane_creeps_system(
        &mut commands,
        dt,
        &config,
        &physics_world,
        &mut world,
        &target_query,
        &obstacle_query,
        &elev_query,
    );

    step_turrets_system(
        &mut commands,
        dt,
        &config,
        &physics_world,
        &mut world,
    );

    step_barracks_system(&config, &mut world);
    step_ancients_system(&config, &mut world);

    world.passive_gold_accumulator_secs += dt;
    while world.passive_gold_accumulator_secs >= config.combat.passive_gold_interval_secs {
        world.passive_gold_accumulator_secs -= config.combat.passive_gold_interval_secs;
        if world.players.is_empty() {
            world.hero_gold = world.hero_gold.saturating_add(config.combat.passive_gold_amount);
        } else {
            for p in &mut world.players {
                p.hero_gold = p.hero_gold.saturating_add(config.combat.passive_gold_amount);
            }
        }
    }

    // حل برخورد فیزیکی هیروها با یکدیگر (هیچ دو هیرویی نمی‌توانند از داخل هم عبور کنند)
    if world.players.len() > 1 {
        let n = world.players.len();
        let base_r = config.soldier.player_radius;
        for i in 0..n {
            for j in (i + 1)..n {
                if world.players[i].is_dead || world.players[j].is_dead {
                    continue;
                }
                let p_i = world.players[i].soldier_pos;
                let p_j = world.players[j].soldier_pos;
                if (p_i.y - p_j.y).abs() > 1.8 {
                    continue;
                }
                let min_dist = base_r * (world.players[i].hero_scale + world.players[j].hero_scale);
                let delta = Vec2::new(p_j.x - p_i.x, p_j.z - p_i.z);
                let dist = delta.length();
                if dist < min_dist {
                    let push_dir = if dist > 0.0001 {
                        delta / dist
                    } else {
                        Vec2::new(1.0, 0.0)
                    };
                    let half_push = (min_dist - dist) * 0.5;
                    world.players[i].soldier_pos.x -= push_dir.x * half_push;
                    world.players[i].soldier_pos.z -= push_dir.y * half_push;
                    world.players[j].soldier_pos.x += push_dir.x * half_push;
                    world.players[j].soldier_pos.z += push_dir.y * half_push;
                }
            }
        }
        for (soldier_marker, mut soldier_tf, _) in &mut soldier_query {
            let p = &world.players[soldier_marker.slot_index];
            soldier_tf.translation.x = p.soldier_pos.x;
            soldier_tf.translation.z = p.soldier_pos.z;
        }
    }


}

fn shared_drive_single_player(
    commands: &mut Commands,
    dt: f32,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    physics_world: &CustomPhysicsWorld,
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    soldier_tf: &mut Transform,
    controller: &mut CustomCharacterController,
    obstacle_query: &Query<(&PhysicsObstacle, &Transform), Without<PhysicsSoldier>>,
    target_query: &Query<&PhysicsTarget>,
    turret_query: &Query<&PhysicsTurret>,
    barracks_query: &Query<&PhysicsBarracks>,
    ancient_query: &Query<&PhysicsAncient>,
    mountain_query: &Query<&PhysicsMountain>,
    map_obj_query: &Query<&MapObjectId>,
    elev_query: &Query<&ElevationSurface>,
    hero_aim_targets: &[(usize, TeamId, Vec3, f32, Stance, bool, bool, bool)],
) {

    // تایمر زنده شدن مجدد در چشمه پایگاه

    if !world.is_dead {
      if let Some((c_click, c_move)) = world.active_teleport.as_ref().map(|tp| (tp.cancel_on_left_click, tp.cancel_on_move)) {
        let flat_mv = if input.move_input.is_finite() {
            Vec3::new(input.move_input.x, 0.0, input.move_input.z)
        } else {
            Vec3::ZERO
        };
        let cancel_click = c_click
            && (input.fire_just_pressed || input.fire_held);
        let cancel_move = c_move && flat_mv != Vec3::ZERO;
        if cancel_click || cancel_move {
            if let Some(tp) = world.active_teleport.take() {
                world.pending_fx.push(GameplayFxEvent::TeleportCanceled {
                    slot_index: world.active_slot,
                    origin_pos: tp.origin_pos,
                });
            }
        } else {
            world.current_locomotion = "idle".to_string();
            world.is_moving = false;
            world.move_input = Vec3::ZERO;
            world.firing_anim_remaining = None;
            world.reload_remaining = None;
            world.active_beam_request = None;
            controller.translation = Some(Vec3::new(0.0, config.physics.character_gravity_step_y * dt, 0.0));
            return;
        }
      }
    }

    if world.is_dead {
        controller.translation = None;
        world.respawn_remaining -= dt;
        if world.respawn_remaining <= 0.0 {
            world.is_dead = false;
            world.hero_hp = world.hero_max_hp;
            world.hero_stamina = world.hero_max_stamina;
            world.stamina_exhausted = false;
            world.space_hold_secs = None;
            world.wall_climb_mountain_index = None;
            world.wall_climb_last_dir = 1;
            world.traversal.is_active = false;
            world.in_water = false;
            world.active_beam_request = None;
            world.flame_armor_debuff_remaining = 0.0;
            world.flame_armor_debuff_amount = 0.0;
            world.electric_fire_rate_slow_remaining = 0.0;
            world.electric_fire_rate_slow_ratio = 0.0;
            world.toxic_slow_multiplier = 1.0;
            world.reflect_armor_remaining = 0.0;
            world.bullet_immunity_remaining = 0.0;
            world.cryo_debuff_remaining = 0.0;
            world.cryo_slow_ratio = 0.0;
            world.cryo_heal_reduction_ratio = 0.0;
            world.null_mute_remaining = 0.0;
            world.sonar_revealed_remaining = 0.0;
            world.kinetic_dash_remaining = 0.0;
            world.active_weapon = ActiveWeaponSlot::Primary;
            world.beam_charge = world.hero_weapon.beam_max_charge;
            world.ammo_in_mag = if world.hero_weapon.weapon_kind == WeaponKind::ElectricBeam
                || world.hero_weapon.weapon_kind == WeaponKind::FlameBeam
            {
                world.beam_charge.ceil() as u32
            } else {
                world.hero_weapon.magazine_size
            };
            world.other_weapon_ammo_in_mag = world.hero_pistol.magazine_size;
            world.grenade_charges = world.hero_grenade.max_charges;
            world.grenade_recharge_remaining = 0.0;
            world.base_stance = Stance::Run;
            let spawn_pos = world.active_spawn_pos;
            world.soldier_pos = spawn_pos;
            soldier_tf.translation =
                spawn_pos + Vec3::Y * config.physics.soldier_capsule_center_y;
        }
    } else {
        // ۲. بازسازی سلامتی در ثانیه (HP Regen پایه + بونوس چشمه تیم خودی با اعمال کاهش درمان Cryo)
        let fountain_bonus = fountain_regen_bonus_for_pos(config, world.hero_team, world.soldier_pos);
        let cryo_heal_mult = if world.cryo_debuff_remaining > 0.0 {
            (1.0 - world.cryo_heal_reduction_ratio).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let effective_hp_regen = (world.hero_hp_regen_per_sec + fountain_bonus) * cryo_heal_mult;
        world.hero_hp = (world.hero_hp + effective_hp_regen * dt).min(world.hero_max_hp);
        let mut stamina_drained_this_tick = false;
        let hero_stamina_resume_min = config.active_hero(world.hero_index).stamina_exhausted_resume_min;
        if world.stamina_exhausted && world.hero_stamina >= hero_stamina_resume_min {
            world.stamina_exhausted = false;
        }

        if input.switch_weapon_just_pressed
            && !world.traversal.is_active
            && world.grenade_elapsed_and_duration.is_none()
        {
            // ضد تقلب ریلود و ضد اکسپلویت شلیک سریع با تعویض اسلحه: کولدان شلیک هرگز کوتاه‌تر نمی‌شود
            world.reload_remaining = None;
            world.firing_anim_remaining = None;
            world.fire_cooldown_remaining = world
                .fire_cooldown_remaining
                .max(config.combat.weapon_swap_min_cooldown_secs);
            std::mem::swap(&mut world.ammo_in_mag, &mut world.other_weapon_ammo_in_mag);
            world.active_weapon = match world.active_weapon {
                ActiveWeaponSlot::Primary => ActiveWeaponSlot::Pistol,
                ActiveWeaponSlot::Pistol => ActiveWeaponSlot::Primary,
            };
            if world.active_weapon == ActiveWeaponSlot::Primary
                && (world.hero_weapon.weapon_kind == WeaponKind::ElectricBeam
                    || world.hero_weapon.weapon_kind == WeaponKind::FlameBeam)
            {
                world.ammo_in_mag = world.beam_charge.ceil() as u32;
            }
        }

        let active_w = if world.active_weapon == ActiveWeaponSlot::Pistol {
            world.hero_pistol
        } else {
            world.hero_weapon
        };

        if let Some(rem) = &mut world.reload_remaining {
            *rem -= dt;
            if *rem <= 0.0 {
                world.reload_remaining = None;
                if active_w.weapon_kind == WeaponKind::ElectricBeam || active_w.weapon_kind == WeaponKind::FlameBeam {
                    world.beam_charge = active_w.beam_max_charge;
                    world.ammo_in_mag = world.beam_charge.ceil() as u32;
                } else {
                    world.ammo_in_mag = active_w.magazine_size;
                }
            }
        }

        if let Some(rem) = &mut world.firing_anim_remaining {
            *rem -= dt;
            if *rem <= 0.0 {
                world.firing_anim_remaining = None;
            }
        }

        if let Some((elapsed, dur)) = &mut world.grenade_elapsed_and_duration {
            *elapsed += dt;
            if *elapsed >= *dur {
                world.grenade_elapsed_and_duration = None;
                world.grenade_spawned_for_throw = false;
            }
        }

        // ۳. شروع ریلود (R) و نارنجک (F)
        if input.reload_just_pressed
            && world.reload_remaining.is_none()
            && world.grenade_elapsed_and_duration.is_none()
            && !world.traversal.is_active
            && world.ammo_in_mag < active_w.magazine_size
        {
            world.reload_remaining = Some(active_w.reload_duration_secs);
            world.firing_anim_remaining = None;
        }

        if world.grenade_charges < world.hero_grenade.max_charges {
            if world.grenade_recharge_remaining <= 0.0 {
                world.grenade_recharge_remaining = world.hero_grenade.recharge_secs;
            }
            world.grenade_recharge_remaining -= dt;
            if world.grenade_recharge_remaining <= 0.0 {
                world.grenade_charges += 1;
                if world.grenade_charges < world.hero_grenade.max_charges {
                    world.grenade_recharge_remaining = world.hero_grenade.recharge_secs;
                } else {
                    world.grenade_recharge_remaining = 0.0;
                }
            }
        }

        let mut threw_grenade_this_tick = false;
        if input.grenade_just_pressed
            && world.grenade_charges > 0
            && world.grenade_elapsed_and_duration.is_none()
            && !world.traversal.is_active
        {
            threw_grenade_this_tick = true;
            if world.grenade_charges == world.hero_grenade.max_charges {
                world.grenade_recharge_remaining = world.hero_grenade.recharge_secs;
            }
            world.grenade_charges -= 1;
            let dur = world.hero_grenade.throw_duration_secs;
            world.grenade_elapsed_and_duration = Some((0.0, dur));
            world.grenade_spawned_for_throw = false;
            world.firing_anim_remaining = None;
            world.reload_remaining = None;
        }

        let can_actually_attack = !world.traversal.is_active
            && world.current_locomotion != "swimming"
            && world.wall_climb_mountain_index.is_none();
        let active_is_beam = world.active_weapon == ActiveWeaponSlot::Primary
            && (world.hero_weapon.weapon_kind == WeaponKind::ElectricBeam
                || world.hero_weapon.weapon_kind == WeaponKind::FlameBeam);
        let has_fire_ammo = if active_is_beam {
            world.beam_charge > 0.0
        } else {
            world.ammo_in_mag > 0
        };
        if world.invisibility_remaining > 0.0
            && can_actually_attack
            && (((input.fire_just_pressed || input.fire_held)
                && world.reload_remaining.is_none()
                && has_fire_ammo)
                || threw_grenade_this_tick)
        {
            world.invisibility_remaining = 0.0;
            world.invisibility_total_secs = 0.0;
            world.invis_move_speed_bonus_pct = 0.0;
            world.is_revealed_by_turret = false;
            world.pending_fx.push(GameplayFxEvent::ShadowBladeEnded {
                slot_index: world.active_slot,
                pos: world.soldier_pos,
            });
        }

        // ۴. تغییر وضعیت نشستن (Ctrl) و راه رفتن (Shift)
        if input.toggle_crouch_just_pressed {
            world.base_stance = if world.base_stance == Stance::Crouch {
                Stance::Run
            } else {
                Stance::Crouch
            };
        }

        world.effective_stance = if input.shift_held {
            Stance::Walk
        } else {
            world.base_stance
        };

        // ۵. هدف‌گیری ۳بعدی دقیق روی سطوح مختلف و قفل نرم روی هیروی دشمن (ایستاده، نشسته یا پشت سنگر)
        let max_aim_origin_dist = config.combat.aim_lock_ray_max_distance.max(120.0);
        if !world.traversal.is_active
            && input.has_aim_ray
            && input.aim_ray_origin.is_finite()
            && input.aim_ray_dir.is_finite()
            && input.aim_ray_dir.length_squared() > 1e-6
            && (input.aim_ray_origin - world.soldier_pos).length_squared()
                <= max_aim_origin_dist * max_aim_origin_dist
            && input.aim_ray_dir.y.abs() > config.physics.aim_ray_min_dir_y_epsilon
        {
            let ray_dir = input.aim_ray_dir.normalize();
            let max_aim_dist = config.combat.aim_lock_ray_max_distance;
            let aim_groups = soldier_collision_groups();

            let hero_in_pit = is_point_in_roshan_pit_circle(
                &config.map.roshan_pit,
                world.soldier_pos.x,
                world.soldier_pos.z,
                config.map.roshan_pit.outer_radius,
            );
            let pit_roof_id = format!("{}_roof", config.map.roshan_pit.id);
            let pit_wall_prefix = format!("{}_wall_", config.map.roshan_pit.id);

            // الف) ابتدا برخورد اشعه ماوس با هندسه واقعی نقشه (با نادیده گرفتن سقف و دیوارهای نیمه‌شفاف غار برای بازیکن داخل غار)
            let world_hit_opt = physics_world
                .cast_ray(
                    input.aim_ray_origin,
                    ray_dir,
                    max_aim_dist,
                    aim_groups,
                    |ent| {
                        if let Ok(t) = target_query.get(ent) {
                            if world.targets[t.index].is_dead {
                                return false;
                            }
                        }
                        if let Ok(obj) = map_obj_query.get(ent) {
                            if obj.0 == pit_roof_id {
                                return false;
                            }
                            if hero_in_pit && obj.0.starts_with(&pit_wall_prefix) {
                                return false;
                            }
                        }
                        true
                    },
                )
                .filter(|(_, hit)| {
                    if hero_in_pit {
                        is_point_in_roshan_pit_circle(
                            &config.map.roshan_pit,
                            hit.point.x,
                            hit.point.z,
                            config.map.roshan_pit.outer_radius,
                        )
                    } else {
                        true
                    }
                });

            // ب) بررسی برخورد تحلیلی اشعه ماوس با کپسول هیروهای دشمن قابل مشاهده (مستقل از موانع بین دوربین و هدف)
            let mut best_enemy_hit: Option<(f32, Vec3)> = None;
            for &(e_slot, e_team, e_pos, e_scale, e_stance, e_dead, e_water_vis_dawn, e_water_vis_dusk) in hero_aim_targets {
                let water_visible = match world.hero_team {
                    TeamId::Dawn => e_water_vis_dawn,
                    TeamId::Dusk => e_water_vis_dusk,
                    TeamId::Neutral => false,
                };
                if e_slot == world.active_slot || e_team == world.hero_team || e_dead || !water_visible {
                    continue;
                }
                let e_vis_h = SharedFogOfWar::hero_visible_height(config, e_stance, e_scale);
                if !world.demo_reveal_full_map
                    && !(world.is_slot_los_visible_to_team(world.hero_team, e_slot)
                        && world.fog_of_war.is_world_entity_visible(config, e_pos, e_vis_h))
                {
                    continue;
                }
                let e_rad =
                    config.soldier.player_radius * e_scale * config.combat.aim_lock_radius_multiplier;
                let e_height = if e_stance == Stance::Crouch {
                    config.combat.hero_crouch_hitbox_height * e_scale
                } else {
                    config.combat.hero_stand_hitbox_height * e_scale
                };
                if let Some(toi) = segment_hits_hero_capsule(
                    input.aim_ray_origin,
                    ray_dir,
                    max_aim_dist,
                    e_pos,
                    e_rad,
                    e_height,
                    config.combat.bullet_hit_forgiveness_height_bottom,
                    config.combat.bullet_hit_forgiveness_height_top,
                ) {
                    if best_enemy_hit.map_or(true, |(prev_toi, _)| toi < prev_toi) {
                        let chest_h = if e_stance == Stance::Crouch {
                            config.combat.hero_crouch_aim_chest_height * e_scale
                        } else {
                            config.combat.hero_stand_aim_chest_height * e_scale
                        };
                        best_enemy_hit = Some((toi, e_pos + Vec3::Y * chest_h));
                    }
                }
            }

            // ج) بررسی برخورد اشعه موس با سربازان لاین و کمپ و غول روشان (مستقل از سقف و موانع بین دوربین و هدف)
            let mut best_creep_hit: Option<(u64, f32, Vec3)> = None;
            for c in &world.creeps {
                if c.is_dead {
                    continue;
                }
                let is_enemy = c.team != world.hero_team;
                let is_deniable =
                    c.team == world.hero_team && (c.hp / c.max_hp) <= config.creeps.deny_hp_threshold_ratio;
                if !is_enemy && !is_deniable {
                    continue;
                }
                let c_stats = resolve_creep_stats(config, c.creep_type_id.as_deref());
                if is_enemy
                    && !world.demo_reveal_full_map
                    && !(world.is_creep_los_visible_to_team(world.hero_team, c.id)
                        && world.fog_of_war.is_world_entity_visible(config, c.pos, c_stats.hitbox_height))
                {
                    continue;
                }
                let c_rad = c_stats.collider_radius * config.combat.aim_lock_radius_multiplier;
                if let Some(toi) = segment_hits_hero_capsule(
                    input.aim_ray_origin,
                    ray_dir,
                    max_aim_dist,
                    c.pos,
                    c_rad,
                    c_stats.hitbox_height,
                    config.combat.bullet_hit_forgiveness_height_bottom,
                    config.combat.bullet_hit_forgiveness_height_top,
                ) {
                    let prev_best_toi = best_creep_hit.map(|(_, t, _)| t).unwrap_or(f32::MAX);
                    if toi < prev_best_toi {
                        best_creep_hit = Some((c.id, toi, c.pos + Vec3::Y * c_stats.aim_chest_height));
                    }
                }
            }

            let mut resolved_hit: Option<Vec3> = None;
            let mut locked_enemy = false;
            let mut hovered_creep: Option<u64> = None;
            let mut hovered_turret: Option<usize> = None;
            let mut hovered_barracks: Option<usize> = None;
            let mut hovered_ancient: Option<usize> = None;

            let mut best_courier_hit: Option<(f32, Vec3)> = None;
            for p in &world.players {
                if p.hero_team == world.hero_team || p.courier_is_dead {
                    continue;
                }
                if !world.demo_reveal_full_map
                    && !(world.is_courier_los_visible_to_team(world.hero_team, p.slot_index)
                        && world.fog_of_war.is_world_pos_visible(config, p.courier_pos))
                {
                    continue;
                }
                let h_cfg = config.active_hero(p.hero_index);
                let c_rad = h_cfg.courier.hit_radius * config.combat.aim_lock_radius_multiplier;
                if let Some(toi) = segment_hits_hero_capsule(
                    input.aim_ray_origin,
                    ray_dir,
                    max_aim_dist,
                    p.courier_pos - Vec3::Y * c_rad,
                    c_rad,
                    c_rad * 2.0,
                    0.2,
                    0.2,
                ) {
                    let prev_best = best_courier_hit.map(|(t, _)| t).unwrap_or(f32::MAX);
                    if toi < prev_best {
                        best_courier_hit = Some((toi, p.courier_pos));
                    }
                }
            }

            let hero_wins_aim = match (best_enemy_hit, best_creep_hit) {
                (Some((h_t, _)), Some((_, c_t, _))) => h_t <= c_t,
                (Some(_), None) => true,
                _ => false,
            };

            if hero_wins_aim {
                resolved_hit = Some(best_enemy_hit.unwrap().1);
                locked_enemy = true;
            } else if let Some((c_id, c_toi, c_chest_pos)) = best_creep_hit {
                if let Some((cour_toi, cour_pos)) = best_courier_hit {
                    if cour_toi < c_toi {
                        resolved_hit = Some(cour_pos);
                        locked_enemy = true;
                    } else {
                        resolved_hit = Some(c_chest_pos);
                        locked_enemy = true;
                        hovered_creep = Some(c_id);
                    }
                } else {
                    resolved_hit = Some(c_chest_pos);
                    locked_enemy = true;
                    hovered_creep = Some(c_id);
                }
            } else if let Some((_, cour_pos)) = best_courier_hit {
                resolved_hit = Some(cour_pos);
                locked_enemy = true;
            } else if let Some((hit_ent, hit)) = world_hit_opt {
                if let Ok(pt) = turret_query.get(hit_ent) {
                    if pt.team != world.hero_team && !world.turrets[pt.index].is_dead {
                        resolved_hit = Some(hit.point);
                        locked_enemy = true;
                        hovered_turret = Some(pt.index);
                    }
                } else if let Ok(pb) = barracks_query.get(hit_ent) {
                    if pb.team != world.hero_team && !world.barracks[pb.index].is_dead {
                        resolved_hit = Some(hit.point);
                        locked_enemy = true;
                        hovered_barracks = Some(pb.index);
                    }
                } else if let Ok(pa) = ancient_query.get(hit_ent) {
                    if pa.team != world.hero_team && !world.ancients[pa.index].is_dead {
                        resolved_hit = Some(hit.point);
                        locked_enemy = true;
                        hovered_ancient = Some(pa.index);
                    }
                } else if target_query.get(hit_ent).is_ok() {
                    resolved_hit = Some(hit.point);
                    locked_enemy = true;
                }

                if !locked_enemy {
                    if input.vertical_aim_held {
                        // در حالت هدف‌گیری سه‌بعدی (نگه داشتن X)، اگر اشعه دوربین قبل از کف رودخانه
                        // به سطح آب برخورد کند، سطح آب ملاک هدف قرار می‌گیرد
                        if let Some((_, water_pt)) = raycast_water_surface(
                            config,
                            input.aim_ray_origin,
                            ray_dir,
                            hit.toi,
                        ) {
                            resolved_hit = Some(water_pt);
                        } else {
                            resolved_hit = Some(hit.point);
                        }
                    } else {
                        // حالت پیش‌فرض بدون نگه داشتن X و بدون هدف: شلیک و خط لیزر کاملاً افقی در ارتفاع هیرو
                        let aim_plane_y = world.soldier_pos.y + config.soldier.aim_plane_height_offset;
                        let t = (aim_plane_y - input.aim_ray_origin.y) / ray_dir.y;
                        if t > 0.0 {
                            resolved_hit = Some(input.aim_ray_origin + ray_dir * t);
                        }
                    }
                }
            } else if input.vertical_aim_held {
                if let Some((_, water_pt)) = raycast_water_surface(
                    config,
                    input.aim_ray_origin,
                    ray_dir,
                    max_aim_dist,
                ) {
                    resolved_hit = Some(water_pt);
                } else {
                    let aim_plane_y = world.soldier_pos.y + config.soldier.aim_plane_height_offset;
                    let t = (aim_plane_y - input.aim_ray_origin.y) / ray_dir.y;
                    if t > 0.0 {
                        resolved_hit = Some(input.aim_ray_origin + ray_dir * t);
                    }
                }
            } else {
                // ج) Fallback به صفحه افقی در صورت عدم برخورد با هندسه نقشه
                let aim_plane_y = world.soldier_pos.y + config.soldier.aim_plane_height_offset;
                let t = (aim_plane_y - input.aim_ray_origin.y) / ray_dir.y;
                if t > 0.0 {
                    resolved_hit = Some(input.aim_ray_origin + ray_dir * t);
                }
            }

            world.aim_locked_on_enemy = locked_enemy;
            world.hovered_creep_id = hovered_creep;
            world.hovered_turret_index = hovered_turret;
            world.hovered_barracks_index = hovered_barracks;
            world.hovered_ancient_index = hovered_ancient;

            if let Some(hit) = resolved_hit {
                world.aim_hit_pos = hit;
                let to_target = Vec3::new(
                    hit.x - world.soldier_pos.x,
                    0.0,
                    hit.z - world.soldier_pos.z,
                );
                if to_target.length_squared() > config.physics.aim_target_min_dist_sq {
                    world.aim_world_dir = to_target.normalize();
                    world.soldier_facing_dir = world.aim_world_dir;
                }
            }
        }

        // ۶. بردار حرکت ۸ جهته و انتخاب انیمیشن پایین‌تنه
        let flat_input = if input.move_input.is_finite() {
            Vec3::new(input.move_input.x, 0.0, input.move_input.z)
        } else {
            Vec3::ZERO
        };
        world.is_moving = flat_input != Vec3::ZERO;
        world.move_input = flat_input;

        if world.is_moving {
            let move_norm = world.move_input.normalize();
            let forward = world.aim_world_dir;
            let right = forward.cross(Vec3::Y).normalize();
            let local_forward = move_norm.dot(forward);
            let local_right = move_norm.dot(right);
            let angle_deg = local_right.atan2(local_forward).to_degrees();
            let dir_suffix = angle_to_direction_8_hysteresis(angle_deg, &world.current_locomotion);

            world.current_locomotion = match world.effective_stance {
                Stance::Run => format!("run_{dir_suffix}"),
                Stance::Walk => format!("walk_{dir_suffix}"),
                Stance::Crouch => format!("walk_crouching_{dir_suffix}"),
            };
        } else {
            world.current_locomotion = match world.effective_stance {
                Stance::Run | Stance::Walk => "idle".to_string(),
                Stance::Crouch => "idle_crouching".to_string(),
            };
        }

        // ۷. سیستم کوهنوردی از کوه‌های قابل صعود دره + پرش، غلت و عبور از مانع
        let is_wall_climbing = step_wall_climb_custom(
            world,
            input,
            config,
            soldier_tf,
            controller,
            dt,
            &mut stamina_drained_this_tick,
        );
        if !is_wall_climbing {
        step_traversal_custom(
            world,
            input,
            config,
            physics_world,
            obstacle_query,
            target_query,
            mountain_query,
            elev_query,
            soldier_tf,
            controller,
            dt,
        );
        }

        // ۸. جابجایی فیزیکی ۳بعدی کاراکتر همراه با مقاومت طبیعی آب و رانش جریان برکه‌ها
        let water_sample = sample_water_surface_at(config, world.soldier_pos);
        let was_in_water = world.in_water;
        world.in_water = water_sample.is_some();
        world.water_splash_cooldown = (world.water_splash_cooldown - dt).max(0.0);

        if let Some((water_y, flow_dir)) = water_sample {
            let water_depth = (water_y - world.soldier_pos.y).max(0.0);
            let is_deep_water = water_depth >= config.physics.deep_water_swim_depth;

            let hero_cfg = config.active_hero(world.hero_index);
            let cryo_slow_mult = if world.cryo_debuff_remaining > 0.0 {
                (1.0 - world.cryo_slow_ratio).clamp(items_cfg.stat_limits.min_cryo_slow_multiplier, 1.0)
            } else {
                1.0
            };
            let hero_move_mult = world.hero_move_speed_multiplier * world.toxic_slow_multiplier * cryo_slow_mult * if world.invisibility_remaining > 0.0 { 1.0 + world.invis_move_speed_bonus_pct } else { 1.0 };
            let shallow_mult = hero_cfg.shallow_water_speed_multiplier;
            let deep_mult = hero_cfg.deep_water_speed_multiplier;

            if is_deep_water && !world.traversal.is_active && world.wall_climb_mountain_index.is_none() {
                if world.is_moving {
                    let swim_drain = hero_cfg.swim_stamina_drain_per_sec * dt;
                    if swim_drain > 0.0 {
                        world.hero_stamina = (world.hero_stamina - swim_drain).max(0.0);
                        stamina_drained_this_tick = true;
                        if world.hero_stamina <= 0.0 {
                            world.stamina_exhausted = true;
                        }
                    }
                    world.current_locomotion = "swimming".to_string();
                    world.firing_anim_remaining = None;
                    // در عمق زیاد آب، هیرو به سمت جهت حرکت خود می‌چرخد
                    world.soldier_facing_dir = world.move_input.normalize();
                } else {
                    world.current_locomotion = "treading_water".to_string();
                }
            }

            // استخراج نقطه پاشش قبل از push کردن برای رفع خطای Borrow Checker
            let splash_pos = Vec3::new(world.soldier_pos.x, water_y, world.soldier_pos.z);

            if !was_in_water {
                world.pending_fx.push(GameplayFxEvent::WaterSplash {
                    pos: splash_pos,
                    intensity: 1.35,
                });
                world.water_splash_cooldown = config.physics.water_splash_interval_secs;
            } else if !is_deep_water && world.is_moving && world.water_splash_cooldown <= 0.0 {
                world.pending_fx.push(GameplayFxEvent::WaterSplash {
                    pos: splash_pos,
                    intensity: 0.85,
                });
                world.water_splash_cooldown = config.physics.water_splash_interval_secs;
            }

            if !world.traversal.is_active && world.wall_climb_mountain_index.is_none() {
                let grav_y = config.physics.character_gravity_step_y * dt;
                let flow_drift = Vec3::new(flow_dir.x, 0.0, flow_dir.y) * 0.35 * dt;
                let base_speed = match world.effective_stance {
                    Stance::Run => config.soldier.run_speed,
                    Stance::Walk => config.soldier.walk_speed,
                    Stance::Crouch => config.soldier.crouch_speed,
                } * hero_move_mult;
                let depth_mult = if is_deep_water {
                    deep_mult
                } else {
                    shallow_mult
                };
                if !is_deep_water && world.is_moving && world.effective_stance == Stance::Run {
                    let run_drain = hero_cfg.run_stamina_drain_per_sec * dt;
                    if run_drain > 0.0 {
                        world.hero_stamina = (world.hero_stamina - run_drain).max(0.0);
                        stamina_drained_this_tick = true;
                        if world.hero_stamina <= 0.0 {
                            world.stamina_exhausted = true;
                        }
                    }
                }
                let is_stamina_low = world.stamina_exhausted || world.hero_stamina <= 0.0;
                let stamina_speed_mult = if is_deep_water {
                    if is_stamina_low { hero_cfg.exhausted_swim_speed_multiplier } else { 1.0 }
                } else if world.effective_stance == Stance::Run && is_stamina_low {
                    hero_cfg.exhausted_run_speed_multiplier
                } else {
                    1.0
                };
                let water_speed = base_speed * depth_mult * stamina_speed_mult;
                world.current_move_speed = water_speed;
                if world.is_moving {
                    let desired_delta =
                        world.move_input.normalize() * water_speed * dt + flow_drift;
                    controller.translation =
                        Some(Vec3::new(desired_delta.x, grav_y, desired_delta.z));
                } else {
                    controller.translation = Some(Vec3::new(flow_drift.x, grav_y, flow_drift.z));
                }
            }
        } else if !world.traversal.is_active && world.wall_climb_mountain_index.is_none() {
            let grav_y = config.physics.character_gravity_step_y * dt;
            if world.kinetic_dash_remaining > 0.0 {
                let step_dt = dt.min(world.kinetic_dash_remaining);
                world.kinetic_dash_remaining = (world.kinetic_dash_remaining - dt).max(0.0);
                let dash_speed = items_cfg.kinetic_dash.dash_distance
                    / items_cfg.kinetic_dash.dash_duration_secs.max(0.01);
                let delta = world.kinetic_dash_dir * dash_speed * step_dt;
                world.current_move_speed = dash_speed;
                controller.translation = Some(Vec3::new(delta.x, grav_y, delta.z));
            } else {
            let cryo_slow_mult = if world.cryo_debuff_remaining > 0.0 {
                (1.0 - world.cryo_slow_ratio).clamp(items_cfg.stat_limits.min_cryo_slow_multiplier, 1.0)
            } else {
                1.0
            };
            let hero_move_mult = world.hero_move_speed_multiplier
                * world.toxic_slow_multiplier
                * cryo_slow_mult
                * if world.invisibility_remaining > 0.0 {
                    1.0 + world.invis_move_speed_bonus_pct
                } else {
                    1.0
                };
            let hero_cfg_land = config.active_hero(world.hero_index);
            if world.is_moving && world.effective_stance == Stance::Run {
                let run_drain = hero_cfg_land.run_stamina_drain_per_sec * dt;
                if run_drain > 0.0 {
                    world.hero_stamina = (world.hero_stamina - run_drain).max(0.0);
                    stamina_drained_this_tick = true;
                    if world.hero_stamina <= 0.0 {
                        world.stamina_exhausted = true;
                    }
                }
            }
            let run_stamina_mult = if world.effective_stance == Stance::Run
                && (world.stamina_exhausted || world.hero_stamina <= 0.0)
            {
                hero_cfg_land.exhausted_run_speed_multiplier
            } else {
                1.0
            };
            let speed = match world.effective_stance {
                Stance::Run => config.soldier.run_speed * run_stamina_mult,
                Stance::Walk => config.soldier.walk_speed,
                Stance::Crouch => config.soldier.crouch_speed,
            } * hero_move_mult;
            world.current_move_speed = speed;
            if world.is_moving {
                let desired_delta = world.move_input.normalize() * speed * dt;
                controller.translation = Some(Vec3::new(desired_delta.x, grav_y, desired_delta.z));
            } else {
                controller.translation = Some(Vec3::new(0.0, grav_y, 0.0));
            }
            }
        }

        // احیای استامینا فقط در حالت‌هایی که مصرف فعال نداریم (ایستاده، راه رفتن، نشستن، شناور، توقف روی صخره)
        if !stamina_drained_this_tick && !world.traversal.is_active {
            world.hero_stamina = (world.hero_stamina + world.hero_stamina_regen_per_sec * dt)
                .min(world.hero_max_stamina);
            if world.stamina_exhausted && world.hero_stamina >= hero_stamina_resume_min {
                world.stamina_exhausted = false;
            }
        }
    }

    // ۹. شلیک گلوله و پرتاب نارنجک
    step_shooting_and_grenade_spawn(commands, world, input, config, items_cfg, physics_world, dt);
}

fn find_nearest_climbable_mountain(config: &GameConfig, attach_dist: f32, pos: Vec3) -> Option<(usize, Vec3, Vec3)> {
    let mut best: Option<(f32, usize, Vec3, Vec3)> = None;

    for (idx, mtn) in config.map.mountains.iter().enumerate() {
        if !mtn.climbable {
            continue;
        }
        let tf = mtn.model_transform.to_transform();
        let hx = mtn.size.0 * 0.5 * tf.scale.x.abs();
        let hz = mtn.size.2 * 0.5 * tf.scale.z.abs();
        let inv_rot = tf.rotation.inverse();
        let local = inv_rot * (pos - tf.translation);
        // هر دو وجه روبرویی مکعب (-Z و +Z در فضای محلی کوه) قابل کوهنوردی هستند تا با چرخاندن مدل در کانفیگ نیز کار کند؛ دیواره‌های کناری (±X) خیر
        if local.x.abs() > hx {
            continue;
        }
        let cx = local.x.clamp(-hx, hx);
        for (cz, local_normal) in [
            (-hz, Vec3::new(0.0, 0.0, -1.0)),
            (hz, Vec3::new(0.0, 0.0, 1.0)),
        ] {
            let on_this_face = if cz < 0.0 {
                local.z <= -hz + 0.35
            } else {
                local.z >= hz - 0.35
            };
            if !on_this_face {
                continue;
            }
            let dx = local.x - cx;
            let dz = local.z - cz;
            let dist = (dx * dx + dz * dz).sqrt();
            if dist <= attach_dist {
                let world_normal = (tf.rotation * local_normal).xz().normalize_or_zero();
                let world_normal3 = Vec3::new(world_normal.x, 0.0, world_normal.y);
                let wall_pt = tf.translation + tf.rotation * Vec3::new(cx, 0.0, cz);
                if best.as_ref().is_none_or(|b| dist < b.0) {
                    best = Some((dist, idx, wall_pt, world_normal3));
                }
            }
        }
    }

    best.map(|(_, idx, pt, n)| (idx, pt, n))
}

fn step_wall_climb_custom(
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    soldier_tf: &mut Transform,
    controller: &mut CustomCharacterController,
    dt: f32,
    stamina_drained_this_tick: &mut bool,
) -> bool {
    let hero_cfg = config.active_hero(world.hero_index);
    let wc = &hero_cfg.wall_climb;

    if let Some(mtn_idx) = world.wall_climb_mountain_index {
        let Some(mtn) = config.map.mountains.get(mtn_idx) else {
            world.wall_climb_mountain_index = None;
            return false;
        };
        if !mtn.climbable || world.traversal.is_active {
            world.wall_climb_mountain_index = None;
            return false;
        }

        if input.space_just_pressed {
            world.wall_climb_mountain_index = None;
            return false;
        }

        let tf = mtn.model_transform.to_transform();
        let hx = mtn.size.0 * 0.5 * tf.scale.x.abs();
        let hz = mtn.size.2 * 0.5 * tf.scale.z.abs();
        let inv_rot = tf.rotation.inverse();
        let local = inv_rot * (world.soldier_pos - tf.translation);
        let cx = local.x.clamp(-hx, hx);
        let (cz, local_normal) = if local.z >= 0.0 {
            (hz, Vec3::new(0.0, 0.0, 1.0))
        } else {
            (-hz, Vec3::new(0.0, 0.0, -1.0))
        };
        let wall_normal_xz = (tf.rotation * local_normal).xz().normalize_or_zero();
        let wall_normal = Vec3::new(wall_normal_xz.x, 0.0, wall_normal_xz.y);
        let wall_pt = tf.translation + tf.rotation * Vec3::new(cx, 0.0, cz);

        let mut cur_y = world.soldier_pos.y;
        let move_up = input.move_input.z < -0.15;
        let move_down = input.move_input.z > 0.15;

        if move_up || move_down {
            let climb_drain = hero_cfg.climb_stamina_drain_per_sec * dt;
            if climb_drain > 0.0 {
                world.hero_stamina = (world.hero_stamina - climb_drain).max(0.0);
                *stamina_drained_this_tick = true;
                if world.hero_stamina <= 0.0 {
                    world.stamina_exhausted = true;
                }
            }
            let climb_speed_mult = if world.stamina_exhausted || world.hero_stamina <= 0.0 {
                hero_cfg.exhausted_climb_speed_multiplier
            } else {
                1.0
            };
            if move_up {
                cur_y += wc.climb_up_speed * climb_speed_mult * dt;
                world.wall_climb_last_dir = 1;
                world.current_locomotion = "climbing_up_wall".to_string();
            } else {
                cur_y -= wc.climb_down_speed * climb_speed_mult * dt;
                world.wall_climb_last_dir = -1;
                world.current_locomotion = "climbing_down_wall".to_string();
            }
        } else {
            world.current_locomotion = if world.wall_climb_last_dir >= 0 {
                "climbing_up_wall_paused".to_string()
            } else {
                "climbing_down_wall_paused".to_string()
            };
        }

        if cur_y >= wc.top_exit_y - wc.top_ledge_trigger_below && move_up {
            world.wall_climb_mountain_index = None;
            let into_plateau = -wall_normal;
            let start_pos = Vec3::new(
                wall_pt.x + wall_normal.x * (config.soldier.player_radius + 0.12),
                cur_y,
                wall_pt.z + wall_normal.z * (config.soldier.player_radius + 0.12),
            );
            let mut exit_dist = wc.top_exit_forward_dist;
            for step_i in 0..=24 {
                let d = wc.top_exit_forward_dist + (step_i as f32) * 0.5;
                let cx = wall_pt.x + into_plateau.x * d;
                let cz = wall_pt.z + into_plateau.z * d;
                if canyon_sample_height(&config.map.canyon, cx, cz) >= -0.05 {
                    exit_dist = d + 1.2;
                    break;
                }
            }
            let target_pos = Vec3::new(
                wall_pt.x + into_plateau.x * exit_dist,
                wc.top_exit_y,
                wall_pt.z + into_plateau.z * exit_dist,
            );
            world.traversal = SharedTraversalState {
                is_active: true,
                kind: TraversalKind::ClimbToTop,
                anim_name: "climbing_to_top".to_string(),
                elapsed: 0.0,
                duration: wc.climb_to_top_duration,
                start_pos,
                target_pos,
                travel_dir: into_plateau,
                vault_peak_lift: 0.0,
                chain_dive_after: false,
                swing_arc_sag: 0.0,
                swing_total_duration: 0.0,
                swing_pulley_pos: None,
                swing_throw_progress: 0.0,
                fall_vel: Vec3::ZERO,
            };
            world.soldier_facing_dir = into_plateau;
            controller.translation = None;
            return true;
        }

        if cur_y <= config.map.canyon.water_y + 0.1 && move_down {
            world.wall_climb_mountain_index = None;
            return false;
        }

        let attach_pos = Vec3::new(
            wall_pt.x + wall_normal.x * (config.soldier.player_radius + 0.15),
            cur_y,
            wall_pt.z + wall_normal.z * (config.soldier.player_radius + 0.15),
        );
        world.soldier_pos = attach_pos;
        soldier_tf.translation = attach_pos + Vec3::Y * (config.physics.soldier_capsule_center_y * world.hero_scale);
        world.soldier_facing_dir = -wall_normal;
        controller.translation = None;
        controller.grounded = false;
        return true;
    }

    if !world.traversal.is_active && world.soldier_pos.y < wc.top_exit_y - 1.5 {
        if let Some((idx, wall_pt, wall_normal)) = find_nearest_climbable_mountain(config, wc.attach_distance, world.soldier_pos) {
            let pressing_w = input.move_input.z < -0.15;
            let moving_toward_wall = input.move_input.length_squared() > 0.04
                && input.move_input.normalize().dot(-wall_normal) > 0.35;
            if pressing_w || moving_toward_wall {
                world.wall_climb_mountain_index = Some(idx);
                world.wall_climb_last_dir = 1;
                let attach_pos = Vec3::new(
                    wall_pt.x + wall_normal.x * (config.soldier.player_radius + 0.15),
                    world.soldier_pos.y,
                    wall_pt.z + wall_normal.z * (config.soldier.player_radius + 0.15),
                );
                world.soldier_pos = attach_pos;
                soldier_tf.translation = attach_pos + Vec3::Y * (config.physics.soldier_capsule_center_y * world.hero_scale);
                world.soldier_facing_dir = -wall_normal;
                world.current_locomotion = "climbing_up_wall".to_string();
                controller.translation = None;
                    controller.grounded = false;
                return true;
            }
        }
    }

    false
}

fn sample_ground_and_deep_water_at(
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    world: &AuthoritativeWorldState,
    target_query: &Query<&PhysicsTarget>,
    mountain_query: &Query<&PhysicsMountain>,
    elev_query: &Query<&ElevationSurface>,
    probe_xz: Vec2,
    ref_y: f32,
) -> (Option<f32>, f32, bool) {
    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);
    let canyon_bed = canyon_sample_height(&config.map.canyon, probe_xz.x, probe_xz.y);
    let inside_canyon_carve = config.map.canyon.enabled && canyon_bed < -0.15;
    let landing_ray_orig = Vec3::new(probe_xz.x, ref_y + 4.0, probe_xz.y);
    let bridge_or_ground_hit = physics_world
        .cast_ray(
            landing_ray_orig,
            Vec3::NEG_Y,
            32.0,
            query_groups,
            |entity| {
                if let Ok(t) = target_query.get(entity) {
                    if world.targets[t.index].is_dead {
                        return false;
                    }
                }
                if let Ok(mtn) = mountain_query.get(entity) {
                    if mtn.id.starts_with("mtn_canyon_") {
                        return false;
                    }
                }
                if inside_canyon_carve {
                    if let Ok(elev) = elev_query.get(entity) {
                        if elev.elevation_tier <= 0 {
                            return false;
                        }
                    }
                }
                true
            },
        )
        .filter(|(ent, hit)| {
            *ent != Entity::PLACEHOLDER && hit.point.y >= config.map.canyon.water_y - 0.25
        })
        .map(|(_, hit)| hit.point.y);

    let effective_landing_y = if let Some(hit_y) = bridge_or_ground_hit {
        hit_y
    } else if inside_canyon_carve {
        let swim_floor = config.map.canyon.water_y - 1.15;
        canyon_bed.max(swim_floor)
    } else {
        ref_y
    };

    let is_deep_water = if bridge_or_ground_hit.is_some() && inside_canyon_carve {
        false
    } else if let Some((water_y, _)) =
        sample_water_surface_at(config, Vec3::new(probe_xz.x, effective_landing_y, probe_xz.y))
    {
        let extra_depth = sample_waterfall_depth_offset(config, probe_xz.x, probe_xz.y);
        let effective_bed_y = if inside_canyon_carve {
            canyon_bed
        } else {
            (effective_landing_y - extra_depth).max(-1.6)
        };
        (water_y - effective_bed_y) >= config.physics.deep_water_swim_depth
    } else {
        false
    };

    (bridge_or_ground_hit, effective_landing_y, is_deep_water)
}

struct VaultCandidate {
    vault_dir: Vec3,
    target_pos: Vec3,
    peak_lift: f32,
}

fn try_find_vault_candidate(
    world: &AuthoritativeWorldState,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    obstacle_query: &Query<(&PhysicsObstacle, &Transform), Without<PhysicsSoldier>>,
    target_query: &Query<&PhysicsTarget>,
    travel_dir: Vec3,
) -> Option<VaultCandidate> {
    let ray_origin = world.soldier_pos + Vec3::Y * config.soldier.vault_ray_height;
    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    let (hit_ent, hit) = physics_world.cast_ray(
        ray_origin,
        travel_dir,
        config.soldier.vault_check_distance,
        query_groups,
        |entity| {
            obstacle_query
                .get(entity)
                .map(|(obs, _)| obs.can_vault)
                .unwrap_or(false)
        },
    )?;

    let (obs, obs_tf) = obstacle_query.get(hit_ent).ok()?;
    let local_normal = (obs_tf.rotation.inverse() * hit.normal).normalize_or_zero();

    // فقط اضلاع جانبی مکعب مستطیل مجاز هستند (سطح بالا و پایین نادیده گرفته می‌شود)
    if local_normal.y.abs() >= 0.5 {
        return None;
    }

    let hit_side = if local_normal.x.abs() >= local_normal.z.abs() {
        if local_normal.x >= 0.0 {
            VaultSide::PosX
        } else {
            VaultSide::NegX
        }
    } else if local_normal.z >= 0.0 {
        VaultSide::PosZ
    } else {
        VaultSide::NegZ
    };

    if !obs.vault_sides.contains(&hit_side) {
        return None;
    }

    let world_face_normal = Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize_or_zero();
    if world_face_normal == Vec3::ZERO {
        return None;
    }
    let inward_dir = -world_face_normal;
    let min_approach_dot = config
        .soldier
        .vault_approach_max_angle_deg
        .to_radians()
        .cos();
    if travel_dir.dot(inward_dir) < min_approach_dot {
        return None;
    }

    let vault_dir = inward_dir;
    let target_x = world.soldier_pos.x + vault_dir.x * config.soldier.vault_travel_distance;
    let target_z = world.soldier_pos.z + vault_dir.z * config.soldier.vault_travel_distance;
    let landing_ray_origin = Vec3::new(
        target_x,
        obs.top_y.max(world.soldier_pos.y) + 4.0,
        target_z,
    );
    let landing_y = physics_world
        .cast_ray(
            landing_ray_origin,
            Vec3::NEG_Y,
            20.0,
            query_groups,
            |ent| !obstacle_query.contains(ent) && !target_query.contains(ent),
        )
        .map(|(_, h)| h.point.y)
        .unwrap_or(world.soldier_pos.y);

    let base_mid_y = world
        .soldier_pos
        .y
        .lerp(landing_y, config.soldier.vault_apex_fraction);
    let obstacle_rel_height = (obs.top_y - base_mid_y).max(0.0);
    let required_lift = obstacle_rel_height
        - config.soldier.vault_anim_reference_height
        + config.soldier.vault_top_clearance
        + obs.vault_height_offset;
    let peak_lift = required_lift.clamp(
        config.soldier.vault_min_lift_height,
        config.soldier.vault_max_lift_height,
    );

    Some(VaultCandidate {
        vault_dir,
        target_pos: Vec3::new(target_x, landing_y, target_z),
        peak_lift,
    })
}

fn start_obstacle_vault(
    world: &mut AuthoritativeWorldState,
    config: &GameConfig,
    candidate: VaultCandidate,
) -> bool {
    if world.hero_stamina < world.hero_slide_stamina_cost {
        return false;
    }
    world.hero_stamina -= world.hero_slide_stamina_cost;
    let count = config.soldier.vault_animations.len();
    assert!(count > 0, "لیست vault_animations در کانفیگ خالی است!");
    let idx = ((world.next_rand() * count as f32) as usize).min(count - 1);
    let chosen_anim = config.soldier.vault_animations[idx].clone();
    let exact_duration = config.exact_anim_duration(&chosen_anim);

    world.traversal.is_active = true;
    world.traversal.kind = TraversalKind::ObstacleVault;
    world.traversal.anim_name = chosen_anim;
    world.traversal.elapsed = 0.0;
    world.traversal.duration = exact_duration;
    world.traversal.start_pos = world.soldier_pos;
    world.traversal.target_pos = candidate.target_pos;
    world.traversal.travel_dir = candidate.vault_dir;
    world.traversal.vault_peak_lift = candidate.peak_lift;
    world.traversal.chain_dive_after = false;
    world.soldier_facing_dir = candidate.vault_dir;
    world.firing_anim_remaining = None;
    world.reload_remaining = None;
    true
}

#[allow(clippy::too_many_arguments)]
fn step_traversal_custom(
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    obstacle_query: &Query<(&PhysicsObstacle, &Transform), Without<PhysicsSoldier>>,
    target_query: &Query<&PhysicsTarget>,
    mountain_query: &Query<&PhysicsMountain>,
    elev_query: &Query<&ElevationSurface>,
    soldier_tf: &mut Transform,
    controller: &mut CustomCharacterController,
    dt: f32,
) {
    if world.traversal.is_active {
        if world.traversal.kind == TraversalKind::RopeSwing {
            let swing_cfg = &config.active_hero(world.hero_index).swing;
            let launch_delay = swing_cfg.launch_delay_secs.max(0.0);
            let total_dur = world.traversal.swing_total_duration.max(0.05);
            let start_anim_dur = (launch_delay + total_dur * swing_cfg.start_phase_fraction).max(0.05);

            if !input.space_held {
                world.traversal.swing_pulley_pos = None;
                world.traversal.swing_throw_progress = 0.0;
                if world.traversal.elapsed < launch_delay {
                    world.traversal.is_active = false;
                    world.traversal.chain_dive_after = false;
                    controller.translation = None;
                    return;
                }
                let speed = swing_cfg.swing_speed;
                let fwd = world.traversal.travel_dir * (speed * swing_cfg.fall_forward_momentum);
                world.traversal.kind = TraversalKind::SwingFall;
                world.traversal.anim_name = swing_cfg.fall_anim.clone();
                world.traversal.elapsed = 0.0;
                world.traversal.duration = config.exact_anim_duration(&swing_cfg.fall_anim);
                world.traversal.fall_vel = Vec3::new(fwd.x, 0.0, fwd.z);
                world.traversal.chain_dive_after = false;
                controller.translation = None;
                return;
            }

            world.traversal.elapsed += dt;
            if world.traversal.elapsed < launch_delay {
                let pos = world.traversal.start_pos;
                world.soldier_pos = pos;
                world.soldier_facing_dir = world.traversal.travel_dir;
                soldier_tf.translation = Vec3::new(
                    pos.x,
                    pos.y + config.physics.soldier_capsule_center_y * world.hero_scale,
                    pos.z,
                );
                controller.translation = None;
                world.traversal.anim_name = swing_cfg.start_anim.clone();
                world.traversal.duration = start_anim_dur;
                world.traversal.swing_throw_progress = (world.traversal.elapsed / launch_delay.max(0.001)).clamp(0.0, 1.0);
                return;
            }

            world.traversal.swing_throw_progress = 1.0;
            let air_elapsed = world.traversal.elapsed - launch_delay;
            let frac = (air_elapsed / total_dur).clamp(0.0, 1.0);
            let horiz = world.traversal.start_pos.lerp(world.traversal.target_pos, frac);
            let sag = 4.0 * frac * (1.0 - frac) * world.traversal.swing_arc_sag;
            let pos = Vec3::new(horiz.x, horiz.y - sag, horiz.z);

            world.soldier_pos = pos;
            world.soldier_facing_dir = world.traversal.travel_dir;
            soldier_tf.translation = Vec3::new(
                pos.x,
                pos.y + config.physics.soldier_capsule_center_y * world.hero_scale,
                pos.z,
            );
            controller.translation = None;

            if frac < swing_cfg.end_phase_fraction {
                world.traversal.anim_name = swing_cfg.start_anim.clone();
                world.traversal.duration = start_anim_dur;
            } else {
                world.traversal.anim_name = swing_cfg.end_anim.clone();
                world.traversal.duration = config.exact_anim_duration(&swing_cfg.end_anim);
            }

            if air_elapsed >= total_dur {
                world.traversal.is_active = false;
                world.traversal.chain_dive_after = false;
                world.traversal.swing_pulley_pos = None;
                world.traversal.swing_throw_progress = 0.0;
                world.soldier_pos = world.traversal.target_pos;
                soldier_tf.translation = Vec3::new(
                    world.traversal.target_pos.x,
                    world.traversal.target_pos.y + config.physics.soldier_capsule_center_y * world.hero_scale,
                    world.traversal.target_pos.z,
                );
            }
            return;
        }

        if world.traversal.kind == TraversalKind::SwingFall {
            let swing_cfg = &config.active_hero(world.hero_index).swing;
            world.traversal.elapsed += dt;
            world.traversal.fall_vel.y -= swing_cfg.fall_gravity * dt;
            let next_pos = world.soldier_pos + world.traversal.fall_vel * dt;
            let (_, eff_y, is_deep_water) = sample_ground_and_deep_water_at(
                config,
                physics_world,
                world,
                target_query,
                mountain_query,
                elev_query,
                Vec2::new(next_pos.x, next_pos.z),
                world.soldier_pos.y,
            );

            world.soldier_facing_dir = world.traversal.travel_dir;
            controller.translation = None;

            if next_pos.y <= eff_y + 0.05 {
                let landed_pos = Vec3::new(next_pos.x, eff_y, next_pos.z);
                world.soldier_pos = landed_pos;
                soldier_tf.translation = Vec3::new(
                    landed_pos.x,
                    landed_pos.y + config.physics.soldier_capsule_center_y * world.hero_scale,
                    landed_pos.z,
                );
                world.traversal.is_active = false;
                world.traversal.chain_dive_after = false;
                if is_deep_water {
                    if let Some((water_y, _)) = sample_water_surface_at(config, landed_pos) {
                        world.pending_fx.push(GameplayFxEvent::WaterSplash {
                            pos: Vec3::new(landed_pos.x, water_y, landed_pos.z),
                            intensity: 2.3,
                        });
                    }
                }
            } else {
                world.soldier_pos = next_pos;
                soldier_tf.translation = Vec3::new(
                    next_pos.x,
                    next_pos.y + config.physics.soldier_capsule_center_y * world.hero_scale,
                    next_pos.z,
                );
            }
            return;
        }

        world.traversal.elapsed += dt;
        let frac = (world.traversal.elapsed / world.traversal.duration).clamp(0.0, 1.0);
        let current_pos = world
            .traversal
            .start_pos
            .lerp(world.traversal.target_pos, frac);
        world.soldier_facing_dir = world.traversal.travel_dir;

        if world.traversal.anim_name == "run_to_dive" {
            controller.translation = None;
            let arc_boost = if world.traversal.kind == TraversalKind::FallDive {
                config.soldier.fall_dive_arc_boost
            } else {
                config.soldier.cliff_dive_arc_boost
            };
            let arc_y = world.traversal.start_pos.y.lerp(world.traversal.target_pos.y, frac * frac)
                + (frac * (1.0 - frac)) * arc_boost;
            let mut dive_x = current_pos.x;
            let mut dive_z = current_pos.z;
            if config.map.canyon.enabled && arc_y < -0.35 {
                let edge_d = canyon_edge_distance(&config.map.canyon, dive_x, dive_z);
                let min_edge = config.soldier.player_radius * world.hero_scale + 0.45;
                if edge_d < min_edge {
                    let eps = 0.8_f32;
                    let gx = canyon_edge_distance(&config.map.canyon, dive_x + eps, dive_z)
                        - canyon_edge_distance(&config.map.canyon, dive_x - eps, dive_z);
                    let gz = canyon_edge_distance(&config.map.canyon, dive_x, dive_z + eps)
                        - canyon_edge_distance(&config.map.canyon, dive_x, dive_z - eps);
                    let inward = Vec3::new(gx, 0.0, gz).normalize_or_zero();
                    if inward != Vec3::ZERO {
                        let push = (min_edge - edge_d).clamp(0.0, 2.5);
                        dive_x += inward.x * push;
                        dive_z += inward.z * push;
                    }
                }
            }
            soldier_tf.translation = Vec3::new(
                dive_x,
                arc_y + config.physics.soldier_capsule_center_y * world.hero_scale,
                dive_z,
            );
            world.soldier_pos = Vec3::new(dive_x, arc_y, dive_z);
            if frac >= 0.85 && (world.traversal.elapsed - dt) / world.traversal.duration < 0.85 {
                if let Some((water_y, _)) = sample_water_surface_at(config, world.traversal.target_pos) {
                    world.pending_fx.push(GameplayFxEvent::WaterSplash {
                        pos: Vec3::new(dive_x, water_y, dive_z),
                        intensity: 2.4,
                    });
                }
            }
        } else if world.traversal.kind == TraversalKind::PitClimb {
            controller.translation = None;
            let smooth_y = world.traversal.start_pos.y.lerp(world.traversal.target_pos.y, frac);
            soldier_tf.translation = Vec3::new(
                current_pos.x,
                smooth_y + config.physics.soldier_capsule_center_y * world.hero_scale,
                current_pos.z,
            );
            world.soldier_pos = Vec3::new(current_pos.x, smooth_y, current_pos.z);
        } else if world.traversal.kind == TraversalKind::ClimbToTop {
            controller.translation = None;
            let rise_t = (frac / 0.65).clamp(0.0, 1.0);
            let step_t = ((frac - 0.55) / 0.45).clamp(0.0, 1.0);
            let smooth_y = world.traversal.start_pos.y.lerp(world.traversal.target_pos.y, rise_t);
            let smooth_x = world.traversal.start_pos.x.lerp(world.traversal.target_pos.x, step_t);
            let smooth_z = world.traversal.start_pos.z.lerp(world.traversal.target_pos.z, step_t);
            soldier_tf.translation = Vec3::new(
                smooth_x,
                smooth_y + config.physics.soldier_capsule_center_y * world.hero_scale,
                smooth_z,
            );
            world.soldier_pos = Vec3::new(smooth_x, smooth_y, smooth_z);
        } else if world.traversal.kind == TraversalKind::ObstacleVault {
            controller.translation = None;
            let apex = config.soldier.vault_apex_fraction;
            let arc_u = if frac <= apex {
                0.5 * (frac / apex)
            } else {
                0.5 + 0.5 * ((frac - apex) / (1.0 - apex))
            };
            let arc_factor = (arc_u * std::f32::consts::PI)
                .sin()
                .max(0.0)
                .powf(config.soldier.vault_arc_exponent);
            let vault_y = current_pos.y + world.traversal.vault_peak_lift * arc_factor;
            soldier_tf.translation = Vec3::new(
                current_pos.x,
                vault_y + config.physics.soldier_capsule_center_y * world.hero_scale,
                current_pos.z,
            );
            world.soldier_pos = Vec3::new(current_pos.x, vault_y, current_pos.z);
        } else if world.traversal.kind == TraversalKind::JumpOver {
            controller.translation = None;
            let ray_orig = Vec3::new(current_pos.x, current_pos.y + 4.0, current_pos.z);
            let ground_y = physics_world
                .cast_ray(
                    ray_orig,
                    Vec3::NEG_Y,
                    10.0,
                    soldier_collision_groups(),
                    |ent| !obstacle_query.contains(ent) && !target_query.contains(ent),
                )
                .map(|(_, hit)| hit.point.y)
                .unwrap_or(current_pos.y);
            soldier_tf.translation = Vec3::new(
                current_pos.x,
                ground_y + config.physics.soldier_capsule_center_y * world.hero_scale,
                current_pos.z,
            );
            world.soldier_pos = Vec3::new(current_pos.x, ground_y, current_pos.z);
        } else {
            let step_delta = current_pos - world.soldier_pos;
            let grav_y = config.physics.character_gravity_step_y * dt;
            controller.translation = Some(Vec3::new(step_delta.x, grav_y, step_delta.z));
        }

        if world.traversal.kind == TraversalKind::RunningJump && world.traversal.anim_name != "run_to_dive" {
            let current_soldier_pos = world.soldier_pos;
            let dust_pos = current_soldier_pos - world.traversal.travel_dir * 0.4
                + Vec3::new(0.0, 0.08, 0.0);
            world
                .pending_fx
                .push(GameplayFxEvent::SlideDust { pos: dust_pos });
        }

        if world.traversal.elapsed >= world.traversal.duration {
            let was_quick_roll = world.traversal.kind == TraversalKind::QuickRollToRun;
            if world.traversal.chain_dive_after {
                let dive_dir = world.traversal.travel_dir;
                let dive_dur = config.exact_anim_duration("run_to_dive");
                let dive_dist = config.soldier.cliff_dive_speed * dive_dur;
                let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);
                let shape_pos = Vec3::new(
                    world.soldier_pos.x,
                    world.soldier_pos.y + config.physics.soldier_capsule_center_y,
                    world.soldier_pos.z,
                );
                let safe_dive_dist = physics_world.sweep_capsule_horizontal(
                    shape_pos,
                    config.physics.soldier_capsule_half_segment,
                    config.soldier.player_radius,
                    dive_dir,
                    dive_dist,
                    config.soldier.free_traversal_wall_padding,
                    query_groups,
                    |entity| {
                        if let Ok(t) = target_query.get(entity) {
                            !world.targets[t.index].is_dead
                        } else {
                            true
                        }
                    },
                );
                let dive_x = world.soldier_pos.x + dive_dir.x * safe_dive_dist;
                let dive_z = world.soldier_pos.z + dive_dir.z * safe_dive_dist;
                let dive_bed = canyon_sample_height(&config.map.canyon, dive_x, dive_z);
                let inside_canyon_carve = config.map.canyon.enabled && dive_bed < -0.15;
                let dive_end_y = if inside_canyon_carve {
                    dive_bed.max(config.map.canyon.water_y - 1.15)
                } else {
                    let (_, eff_y, _) = sample_ground_and_deep_water_at(
                        config,
                        physics_world,
                        world,
                        target_query,
                        mountain_query,
                        elev_query,
                        Vec2::new(dive_x, dive_z),
                        world.soldier_pos.y,
                    );
                    eff_y
                };

                world.traversal.is_active = true;
                world.traversal.kind = TraversalKind::RunningJump;
                world.traversal.anim_name = "run_to_dive".to_string();
                world.traversal.elapsed = 0.0;
                world.traversal.duration = dive_dur;
                world.traversal.start_pos = world.soldier_pos;
                world.traversal.target_pos = Vec3::new(dive_x, dive_end_y, dive_z);
                world.traversal.travel_dir = dive_dir;
                world.traversal.chain_dive_after = false;
                world.soldier_facing_dir = dive_dir;
                world.firing_anim_remaining = None;
                world.reload_remaining = None;
                controller.translation = None;
                if was_quick_roll {
                    world.base_stance = Stance::Run;
                    world.effective_stance = Stance::Run;
                }
                return;
            }

            world.traversal.is_active = false;
            world.traversal.vault_peak_lift = 0.0;
            world.traversal.chain_dive_after = false;
            controller.translation = None;
            if was_quick_roll {
                world.base_stance = Stance::Run;
                world.effective_stance = Stance::Run;
            }
        }
        return;
    }

    // ورود به غار روشان از دم در ورودی با فشردن Space و اجرای انیمیشن Freehang Climb
    if input.space_just_pressed && config.map.roshan_pit.enabled {
        let pit = &config.map.roshan_pit;
        if !is_point_in_roshan_pit_circle(pit, world.soldier_pos.x, world.soldier_pos.z, pit.inner_radius) {
            let pit_tf = pit.model_transform.to_transform();
            let pit_center = pit_tf.translation;
            let ent_rad = pit.entrance_angle_deg.to_radians();
            let local_dir = Vec3::new(ent_rad.cos(), 0.0, ent_rad.sin());
            let outward_dir = (pit_tf.rotation * local_dir).normalize_or_zero();
            let ent_world = pit_center + outward_dir * pit.outer_radius;
            let dist_to_ent = Vec2::new(
                world.soldier_pos.x - ent_world.x,
                world.soldier_pos.z - ent_world.z,
            ).length();
            let floor_top_y = pit_center.y + 0.15;

            if dist_to_ent <= pit.climb_enter_distance && world.soldier_pos.y <= floor_top_y + 0.5 {
                let anim = pit.climb_enter_anim.clone();
                let dur = config.exact_anim_duration(&anim);
                let inward_dir = -outward_dir;
                let inside_r = (pit.inner_radius - 1.5).max(1.0);
                let target_pos = Vec3::new(
                    pit_center.x + outward_dir.x * inside_r,
                    floor_top_y,
                    pit_center.z + outward_dir.z * inside_r,
                );
                world.traversal.is_active = true;
                world.traversal.kind = TraversalKind::PitClimb;
                world.traversal.anim_name = anim;
                world.traversal.elapsed = 0.0;
                world.traversal.duration = dur;
                world.traversal.start_pos = world.soldier_pos;
                world.traversal.target_pos = target_pos;
                world.traversal.travel_dir = inward_dir;
                world.traversal.chain_dive_after = false;
                world.soldier_facing_dir = inward_dir;
                world.space_hold_secs = None;
                world.firing_anim_remaining = None;
                world.reload_remaining = None;
                return;
            }
        }
    }

    if let Some((water_y, _)) = sample_water_surface_at(config, world.soldier_pos) {
        if (water_y - world.soldier_pos.y) >= config.physics.deep_water_swim_depth {
            world.space_hold_secs = None;
            return;
        }
    }

    // بررسی نقاط Swing در لبه دره: اگر بازیکن در محدوده یکی از دو طرف باشد و Space را نگه دارد، عمل Swing شروع می‌شود
    if input.space_held {
        let swing_cfg = &config.active_hero(world.hero_index).swing;
        if world.hero_stamina >= swing_cfg.stamina_cost {
            for sp in &config.map.swing_points {
                let pa = Vec3::new(sp.point_a.0, sp.point_a.1, sp.point_a.2);
                let pb = Vec3::new(sp.point_b.0, sp.point_b.1, sp.point_b.2);
                let da = Vec2::new(world.soldier_pos.x - pa.x, world.soldier_pos.z - pa.z).length();
                let db = Vec2::new(world.soldier_pos.x - pb.x, world.soldier_pos.z - pb.z).length();
                let dest_opt = if da <= sp.trigger_radius && (world.soldier_pos.y - pa.y).abs() <= 2.5 {
                    Some(pb)
                } else if db <= sp.trigger_radius && (world.soldier_pos.y - pb.y).abs() <= 2.5 {
                    Some(pa)
                } else {
                    None
                };

                if let Some(dest) = dest_opt {
                    let delta_xz = Vec3::new(dest.x - world.soldier_pos.x, 0.0, dest.z - world.soldier_pos.z);
                    let flat_dir = delta_xz.normalize_or_zero();
                    if flat_dir != Vec3::ZERO {
                        world.hero_stamina -= swing_cfg.stamina_cost;
                        world.space_hold_secs = None;
                        let total_dist = world.soldier_pos.distance(dest);
                        let total_dur = (total_dist / swing_cfg.swing_speed).max(0.25);

                        world.traversal.is_active = true;
                        world.traversal.kind = TraversalKind::RopeSwing;
                        world.traversal.anim_name = swing_cfg.start_anim.clone();
                        world.traversal.elapsed = 0.0;
                        world.traversal.duration = (swing_cfg.launch_delay_secs.max(0.0) + total_dur * swing_cfg.start_phase_fraction).max(0.05);
                        world.traversal.swing_total_duration = total_dur;
                        world.traversal.swing_arc_sag = sp.arc_sag_depth;
                        world.traversal.swing_pulley_pos = Some(Vec3::from(sp.pulley_pos));
                        world.traversal.swing_throw_progress = if swing_cfg.launch_delay_secs > 0.001 { 0.0 } else { 1.0 };
                        world.traversal.start_pos = world.soldier_pos;
                        world.traversal.target_pos = dest;
                        world.traversal.travel_dir = flat_dir;
                        world.traversal.chain_dive_after = false;
                        world.traversal.fall_vel = Vec3::ZERO;
                        world.soldier_facing_dir = flat_dir;
                        world.firing_anim_remaining = None;
                        world.reload_remaining = None;
                        controller.translation = None;
                        return;
                    }
                }
            }
        }
    }

    let travel_dir = if world.is_moving {
        world.move_input.normalize()
    } else {
        world.aim_world_dir
    };

    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    let (kind, anim_name, speed, stamina_cost) = if world.effective_stance == Stance::Run {
        if input.space_just_pressed && world.space_hold_secs.is_none() {
            if let Some(candidate) = try_find_vault_candidate(
                world,
                config,
                physics_world,
                obstacle_query,
                target_query,
                travel_dir,
            ) {
                start_obstacle_vault(world, config, candidate);
                return;
            }
            world.space_hold_secs = Some(0.0);
            if input.space_held {
                return;
            }
        }

        if let Some(held) = world.space_hold_secs {
            if input.space_held {
                world.space_hold_secs = Some(held + dt);
                return;
            }
            world.space_hold_secs = None;
            if held >= config.soldier.space_hold_threshold_secs {
                (
                    TraversalKind::JumpOver,
                    "jump_over",
                    config.soldier.jump_over_speed,
                    world.hero_jump_over_stamina_cost,
                )
            } else {
                (
                    TraversalKind::RunningJump,
                    "running_slide",
                    config.soldier.running_slide_speed,
                    world.hero_slide_stamina_cost,
                )
            }
        } else {
            return;
        }
    } else {
        world.space_hold_secs = None;
        if !input.space_just_pressed {
            return;
        }
        if let Some(candidate) = try_find_vault_candidate(
            world,
            config,
            physics_world,
            obstacle_query,
            target_query,
            travel_dir,
        ) {
            start_obstacle_vault(world, config, candidate);
            return;
        }
        match world.effective_stance {
            Stance::Walk => (
                TraversalKind::RunningDiveRoll,
                "running_dive_roll",
                config.soldier.dive_roll_speed,
                world.hero_slide_stamina_cost,
            ),
            Stance::Crouch => (
                TraversalKind::QuickRollToRun,
                "quick_roll_to_run",
                config.soldier.quick_roll_speed,
                world.hero_slide_stamina_cost,
            ),
            Stance::Run => unreachable!(),
        }
    };

    if world.hero_stamina < stamina_cost {
        return;
    }
    world.hero_stamina -= stamina_cost;

    let mut exact_duration = config.exact_anim_duration(anim_name);
    let mut distance = speed * exact_duration;

    let clearance_y = if kind == TraversalKind::JumpOver {
        config.soldier.jump_over_clearance_height
    } else {
        0.0
    };
    let shape_pos = Vec3::new(
        world.soldier_pos.x,
        world.soldier_pos.y + config.physics.soldier_capsule_center_y + clearance_y,
        world.soldier_pos.z,
    );

    let mut safe_dist = physics_world.sweep_capsule_horizontal(
        shape_pos,
        config.physics.soldier_capsule_half_segment,
        config.soldier.player_radius,
        travel_dir,
        distance,
        config.soldier.free_traversal_wall_padding,
        query_groups,
        |entity| {
            if let Ok(t) = target_query.get(entity) {
                !world.targets[t.index].is_dead
            } else {
                true
            }
        },
    );

    let mut target_pos = Vec3::new(
        world.soldier_pos.x + travel_dir.x * safe_dist,
        world.soldier_pos.y,
        world.soldier_pos.z + travel_dir.z * safe_dist,
    );

    let probe_step = config.soldier.cliff_edge_probe_step;
    let mut first_cliff_drop_dist: Option<f32> = None;
    let mut last_landing_y = world.soldier_pos.y;
    let mut d = probe_step;
    while d <= safe_dist + probe_step * 0.5 {
        let sample_d = d.min(safe_dist);
        let sx = world.soldier_pos.x + travel_dir.x * sample_d;
        let sz = world.soldier_pos.z + travel_dir.z * sample_d;
        let (_, eff_y, is_deep_water) = sample_ground_and_deep_water_at(
            config,
            physics_world,
            world,
            target_query,
            mountain_query,
            elev_query,
            Vec2::new(sx, sz),
            world.soldier_pos.y,
        );
        last_landing_y = eff_y;
        if is_deep_water && (world.soldier_pos.y - eff_y) >= config.soldier.cliff_edge_drop_threshold {
            first_cliff_drop_dist = Some(sample_d);
            break;
        }
        if sample_d >= safe_dist {
            break;
        }
        d += probe_step;
    }

    let chain_dive_after = false;
    let final_anim_name = if let Some(drop_d) = first_cliff_drop_dist {
        let edge_dist = (drop_d - probe_step - config.soldier.cliff_edge_stop_margin).max(0.0);
        if edge_dist >= config.soldier.cliff_chain_min_slide_dist {
            target_pos = Vec3::new(
                world.soldier_pos.x + travel_dir.x * edge_dist,
                world.soldier_pos.y,
                world.soldier_pos.z + travel_dir.z * edge_dist,
            );
            let frac = (edge_dist / distance.max(0.001)).clamp(0.0, 1.0);
            exact_duration = (exact_duration * frac).max(config.soldier.cliff_edge_min_anim_duration_secs);
            anim_name
        } else {
            exact_duration = config.exact_anim_duration("run_to_dive");
            distance = config.soldier.cliff_dive_speed.max(speed) * exact_duration;
            safe_dist = physics_world.sweep_capsule_horizontal(
                shape_pos,
                config.physics.soldier_capsule_half_segment,
                config.soldier.player_radius,
                travel_dir,
                distance,
                config.soldier.free_traversal_wall_padding,
                query_groups,
                |entity| {
                    if let Ok(t) = target_query.get(entity) {
                        !world.targets[t.index].is_dead
                    } else {
                        true
                    }
                },
            );
            let dive_x = world.soldier_pos.x + travel_dir.x * safe_dist;
            let dive_z = world.soldier_pos.z + travel_dir.z * safe_dist;
            let dive_bed = canyon_sample_height(&config.map.canyon, dive_x, dive_z);
            let dive_end_y = if config.map.canyon.enabled && dive_bed < -0.15 {
                dive_bed.max(config.map.canyon.water_y - 1.15)
            } else {
                last_landing_y
            };
            target_pos = Vec3::new(dive_x, dive_end_y, dive_z);
            "run_to_dive"
        }
    } else {
        anim_name
    };

    world.traversal.is_active = true;
    world.traversal.kind = kind;
    world.traversal.anim_name = final_anim_name.to_string();
    world.traversal.elapsed = 0.0;
    world.traversal.duration = exact_duration;
    world.traversal.start_pos = world.soldier_pos;
    world.traversal.target_pos = target_pos;
    world.traversal.travel_dir = travel_dir;
    world.traversal.chain_dive_after = chain_dive_after;
    world.soldier_facing_dir = travel_dir;
}

fn step_shooting_and_grenade_spawn(
    commands: &mut Commands,
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    physics_world: &CustomPhysicsWorld,
    dt: f32,
) {
    world.fire_cooldown_remaining = (world.fire_cooldown_remaining - dt).max(0.0);
    if world.electric_fire_rate_slow_remaining > 0.0 {
        world.electric_fire_rate_slow_remaining = (world.electric_fire_rate_slow_remaining - dt).max(0.0);
        if world.electric_fire_rate_slow_remaining <= 0.0 {
            world.electric_fire_rate_slow_ratio = 0.0;
        }
    }
    if world.flame_armor_debuff_remaining > 0.0 {
        world.flame_armor_debuff_remaining = (world.flame_armor_debuff_remaining - dt).max(0.0);
        if world.flame_armor_debuff_remaining <= 0.0 {
            world.flame_armor_debuff_amount = 0.0;
        }
    }

    if world.is_dead || world.traversal.is_active || world.current_locomotion == "swimming" {
        return;
    }

    if let Some((elapsed, dur)) = world.grenade_elapsed_and_duration {
        if (elapsed / dur) >= config.combat.grenade_release_fraction
            && !world.grenade_spawned_for_throw
        {
            world.grenade_spawned_for_throw = true;
            let spawn_pos = world.soldier_pos
                + Vec3::new(0.0, config.combat.grenade_spawn_offset_y, 0.0)
                + world.aim_world_dir * config.combat.grenade_spawn_forward;
            let to_target = world.aim_hit_pos - spawn_pos;
            let flat_dist = Vec3::new(to_target.x, 0.0, to_target.z).length().clamp(
                config.combat.grenade_min_throw_dist,
                config.combat.grenade_max_throw_dist,
            );

            let id = world.next_entity_id;
            world.next_entity_id += 1;
            let lin_vel = world.aim_world_dir
                * (flat_dist * config.combat.grenade_horizontal_vel_factor)
                + Vec3::Y * config.combat.grenade_vertical_vel;
            let ang_vel = Vec3::new(
                config.physics.grenade_initial_angular_vel.0,
                config.physics.grenade_initial_angular_vel.1,
                config.physics.grenade_initial_angular_vel.2,
            );

            commands.spawn(grenade_physics_bundle(
                config,
                id,
                world.active_slot,
                world.hero_team,
                world.hero_grenade.fuse_secs,
                world.hero_grenade.damage,
                world.hero_grenade.aoe_radius,
                spawn_pos,
                Quat::IDENTITY,
                lin_vel,
                ang_vel,
            ));
        }
        return;
    }

    if world.reload_remaining.is_some() {
        world.active_beam_request = None;
        return;
    }

    let active_w = if world.active_weapon == ActiveWeaponSlot::Pistol {
        world.hero_pistol
    } else {
        world.hero_weapon
    };

    if active_w.weapon_kind == WeaponKind::ElectricBeam || active_w.weapon_kind == WeaponKind::FlameBeam {
        let is_flame_beam = active_w.weapon_kind == WeaponKind::FlameBeam;
        if input.fire_held && !world.is_dead && !world.traversal.is_active && world.wall_climb_mountain_index.is_none() {
            if world.beam_charge <= 0.0 {
                world.ammo_in_mag = 0;
                world.reload_remaining = Some(active_w.reload_duration_secs);
                world.firing_anim_remaining = None;
                world.active_beam_request = None;
                return;
            }
            world.beam_charge = (world.beam_charge - active_w.beam_drain_per_sec * dt).max(0.0);
            world.ammo_in_mag = world.beam_charge.ceil() as u32;
            let dur = config.exact_anim_duration("firing_rifle");
            world.firing_anim_remaining = Some(dur);
            let muzzle_pos = resolve_muzzle_pos(world, input, config, physics_world);
            let to_aim = world.aim_hit_pos - muzzle_pos;
            let beam_dir = if !world.aim_locked_on_enemy && !input.vertical_aim_held {
                let flat = Vec3::new(to_aim.x, 0.0, to_aim.z);
                if flat.length_squared() > config.physics.muzzle_aim_min_dist_sq {
                    flat.normalize()
                } else {
                    world.aim_world_dir
                }
            } else if to_aim.length_squared() > config.physics.muzzle_aim_min_dist_sq {
                let mut dir = to_aim.normalize();
                let max_sin = config.combat.max_vertical_aim_pitch_sin;
                dir.y = dir.y.clamp(-max_sin, max_sin);
                dir.normalize()
            } else {
                world.aim_world_dir
            };
            let beam_slow_mult = if world.electric_fire_rate_slow_remaining > 0.0 {
                (1.0 - world.electric_fire_rate_slow_ratio)
                    .clamp(items_cfg.stat_limits.min_electric_slow_divisor, 1.0)
            } else {
                1.0
            };
            world.active_beam_request = Some((
                muzzle_pos,
                beam_dir,
                active_w.max_range,
                active_w.beam_dps * beam_slow_mult,
                active_w.beam_attack_speed_slow,
                active_w.beam_armor_reduction,
                active_w.beam_slow_duration_secs,
                is_flame_beam,
            ));
            if world.beam_charge <= 0.0 {
                world.ammo_in_mag = 0;
                world.reload_remaining = Some(active_w.reload_duration_secs);
                world.active_beam_request = None;
            }
        } else {
            world.active_beam_request = None;
        }
        return;
    } else {
        world.active_beam_request = None;
    }

    let wants_fire =
        (input.fire_just_pressed || input.fire_held) && world.fire_cooldown_remaining <= 0.0;

    if wants_fire {
        if world.ammo_in_mag == 0 {
            world.reload_remaining = Some(active_w.reload_duration_secs);
            world.firing_anim_remaining = None;
            return;
        }

        world.ammo_in_mag -= 1;
        let slow_div = if world.electric_fire_rate_slow_remaining > 0.0 {
            (1.0 - world.electric_fire_rate_slow_ratio)
                .clamp(items_cfg.stat_limits.min_electric_slow_divisor, 1.0)
        } else {
            1.0
        };
        world.fire_cooldown_remaining = active_w.fire_rate_secs / slow_div;
        if world.active_weapon == ActiveWeaponSlot::Primary {
            let dur = config.exact_anim_duration("firing_rifle");
            world.firing_anim_remaining = Some(dur);
        } else {
            world.firing_anim_remaining = None;
        }

        let muzzle_pos = resolve_muzzle_pos(world, input, config, physics_world);
        let to_aim = world.aim_hit_pos - muzzle_pos;
        let shot_dir = if !world.aim_locked_on_enemy && !input.vertical_aim_held {
            let flat = Vec3::new(to_aim.x, 0.0, to_aim.z);
            if flat.length_squared() > config.physics.muzzle_aim_min_dist_sq {
                flat.normalize()
            } else {
                world.aim_world_dir
            }
        } else if to_aim.length_squared() > config.physics.muzzle_aim_min_dist_sq {
            let mut dir = to_aim.normalize();
            // محدود کردن زاویه عمودی از روی کانفیگ تا تیر بین سطوح بالا و پایین روان حرکت کند
            let max_sin = config.combat.max_vertical_aim_pitch_sin;
            dir.y = dir.y.clamp(-max_sin, max_sin);
            dir.normalize()
        } else {
            world.aim_world_dir
        };

        let pellets = active_w.pellets_per_shot.max(1);
        let crouch_accuracy_mult = if world.effective_stance == Stance::Crouch {
            0.60
        } else {
            1.0
        };
        let spread_rad = active_w.spread_rad * crouch_accuracy_mult;
        let proj_speed = active_w.projectile_speed;
        let dmg = active_w.damage / (pellets as f32);
        let max_range = active_w.max_range;

        let mut first_dir = shot_dir;
        for p_idx in 0..pellets {
            let spread = (world.next_rand() - 0.5) * spread_rad;
            let final_dir = (Quat::from_rotation_y(spread) * shot_dir).normalize();
            if p_idx == 0 {
                first_dir = final_dir;
            }
            let id = world.next_entity_id;
            world.next_entity_id += 1;
            let (init_vel, bullet_range) = if active_w.gravity > 0.0 {
                let target_y = if !world.aim_locked_on_enemy && !input.vertical_aim_held {
                    world.soldier_pos.y
                } else {
                    world.aim_hit_pos.y
                };
                let raw_horiz = Vec3::new(
                    world.aim_hit_pos.x - muzzle_pos.x,
                    0.0,
                    world.aim_hit_pos.z - muzzle_pos.z,
                );
                let raw_dist = raw_horiz.length();
                let horiz_dir = if raw_dist > 0.05 {
                    Quat::from_rotation_y(spread) * (raw_horiz / raw_dist)
                } else {
                    Vec3::new(final_dir.x, 0.0, final_dir.z).normalize_or_zero()
                };
                let clamped_dist = raw_dist.clamp(1.0, max_range);
                let flight_time = (clamped_dist / proj_speed.max(1.0)).max(0.28);
                let horiz_vel = horiz_dir * (clamped_dist / flight_time);
                let dy = target_y - muzzle_pos.y;
                let vy = (dy + 0.5 * active_w.gravity * flight_time * flight_time) / flight_time
                    + active_w.launch_up_boost;
                (
                    Vec3::new(horiz_vel.x, vy, horiz_vel.z),
                    max_range * 1.8,
                )
            } else {
                (final_dir * proj_speed, max_range)
            };
            commands.spawn((
                PhysicsBullet {
                    id,
                    owner_slot: Some(world.active_slot),
                    owner_creep_id: None,
                    owner_turret_index: None,
                    owner_team: world.hero_team,
                    vfx_source: BulletVfxSource::Hero {
                        hero_id: world.hero_id.clone(),
                        weapon_kind: active_w.weapon_kind,
                    },
                    velocity: init_vel,
                    damage: dmg,
                    remaining_range: bullet_range,
                    gravity: active_w.gravity,
                    aoe_radius: active_w.aoe_radius,
                    toxic_pool_duration_secs: active_w.toxic_pool_duration_secs,
                    toxic_pool_dps: active_w.toxic_pool_dps,
                    toxic_slow_ratio: active_w.toxic_slow_ratio,
                    piercing: active_w.piercing || active_w.weapon_kind == WeaponKind::SpinningBlade,
                    hit_entity_keys: Vec::new(),
                },
                Transform::from_translation(muzzle_pos),
            ));
        }

        world.pending_fx.push(GameplayFxEvent::ShotFired {
            muzzle_pos,
            shot_dir: first_dir,
            source: ShotAudioSource::Hero {
                hero_id: world.hero_id.clone(),
                weapon_kind: active_w.weapon_kind,
            },
            trauma_add: config.combat.shot_fire_trauma_add,
            trauma_max: config.combat.shot_fire_trauma_max,
        });
    }
}


/// اعمال یکپارچه دمیج و پاداش نابودی روی Turret (مشترک بین Bullet، Beam و AOE)
#[allow(clippy::too_many_arguments)]

/// اعمال متمرکز و بدون باگ تمام مکانیک‌های تاکتیکی آسیب به هیرو:
/// BulletImmunity، Evasion در برابر TrueStrike، Critical Strike (پشتیبانی کامل از تمام سلاح‌ها از جمله Beam)،
/// Cryo Slow + Anti-Heal (با بونوس برف و آب)، Lifesteal و ReflectArmor (با محافظ ضد حلقه بی‌نهایت).
pub fn apply_combat_damage_to_hero(
    world: &mut AuthoritativeWorldState,
    victim_idx: usize,
    attacker_slot: Option<usize>,
    attacker_creep_id: Option<u64>,
    attacker_turret_index: Option<usize>,
    mut raw_pre_armor_dmg: f32,
    min_hit_dmg: f32,
    is_evadable: bool,
    is_continuous_tick: bool,
    is_shotgun_pellet: bool,
    can_apply_on_hit: bool,
    impact_pos: Vec3,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> (f32, bool, bool) {
    if victim_idx >= world.players.len() || world.players[victim_idx].is_dead {
        return (0.0, false, false);
    }
    let ce = &items_cfg.combat_effects;
    let ra = &items_cfg.reflect_armor;

    // 1. بررسی مصونیت کامل از تیر/اشعه/انفجار (Quantum Phase Shield)
    if world.players[victim_idx].bullet_immunity_remaining > 0.0 {
        if world.players[victim_idx].popup_throttle_remaining <= 0.0 {
            world.players[victim_idx].popup_throttle_remaining = ce.popup_throttle_secs;
            world.pending_fx.push(GameplayFxEvent::CombatPopup {
                pos: impact_pos + Vec3::Y * 1.4,
                text: "IMMUNE".to_string(),
                color_rgb: ce.immune_popup_color_rgb,
            });
        }
        return (0.0, false, false);
    }

    let attacker_idx =
        attacker_slot.and_then(|s| world.players.iter().position(|p| p.slot_index == s));

    // 2. بررسی Evasion در برابر True Strike -> نمایش نوشته مینیمال سفید `miss`
    if is_evadable {
        let evasion = world.players[victim_idx].hero_evasion_chance;
        if evasion > 0.0 && world.next_rand() < evasion {
            let true_strike = attacker_idx
                .map(|ai| world.players[ai].hero_true_strike_chance)
                .unwrap_or(0.0);
            if true_strike > 0.0 && world.next_rand() < true_strike {
                raw_pre_armor_dmg += ce.true_strike_bonus_damage;
            } else {
                world.pending_fx.push(GameplayFxEvent::EvadeMiss {
                    pos: impact_pos + Vec3::Y * 1.35,
                });
                return (0.0, false, false);
            }
        }
    }

    // 3. ضربه بحرانی (Critical Strike)
    let mut is_crit = false;
    if let Some(ai) = attacker_idx {
        let crit_chance = world.players[ai].hero_crit_chance;
        let crit_mult = world.players[ai].hero_crit_multiplier;
        if crit_chance > 0.0 && crit_mult > 1.0 {
            if is_continuous_tick {
                raw_pre_armor_dmg *= 1.0 + crit_chance * (crit_mult - 1.0);
            } else if world.next_rand() < crit_chance {
                raw_pre_armor_dmg *= crit_mult;
                is_crit = true;
            }
        }
    }

    // 4. محاسبه آرمور (فرمول دوتا ۲ + دیباف شعله)
    let armor = world.players[victim_idx].hero_armor
        - if world.players[victim_idx].flame_armor_debuff_remaining > 0.0 {
            world.players[victim_idx].flame_armor_debuff_amount
        } else {
            0.0
        };
    let eff_dmg = (raw_pre_armor_dmg * dota2_armor_multiplier(armor)).max(min_hit_dmg);

    world.players[victim_idx].hero_hp = (world.players[victim_idx].hero_hp - eff_dmg).max(0.0);
    record_hero_damage_on_victim(world, victim_idx, attacker_slot, config);

    // نمایش عدد جامع میزان دمیج (سفید ریز برای عادی، قرمز برجسته برای Crit)
    if !is_continuous_tick {
        if eff_dmg >= 0.5 {
            world.pending_fx.push(GameplayFxEvent::DamageNumber {
                pos: impact_pos + Vec3::Y * 1.45,
                amount: eff_dmg,
                is_crit,
            });
        }
    } else if world.tick % 16 == 0 && eff_dmg > 0.01 {
        // برای تیک ۶۰ هرتز اشعه/سم، هر ~0.26 ثانیه مجموع تقریبی آن بازه نمایش داده می‌شود
        world.pending_fx.push(GameplayFxEvent::DamageNumber {
            pos: impact_pos + Vec3::Y * 1.45,
            amount: (eff_dmg * 16.0).max(1.0),
            is_crit: false,
        });
    }

    // 5. اعمال دیباف Cryo (کندشدن + کاهش درمان با بونوس برف یا آب)
    if can_apply_on_hit {
        if let Some(ai) = attacker_idx {
            let base_dur = world.players[ai].hero_on_hit_debuff_duration_secs;
            if base_dur > 0.0 {
                let slow_r = world.players[ai].hero_on_hit_slow_ratio;
                let heal_r = world.players[ai].hero_on_hit_heal_reduction_ratio;
                let env_mult = if world.current_weather == WeatherKind::Snowy
                    || world.players[victim_idx].in_water
                {
                    ce.cryo_snow_or_water_duration_mult
                } else {
                    1.0
                };
                let dur = base_dur * env_mult;
                world.players[victim_idx].cryo_debuff_remaining = dur;
                world.players[victim_idx].cryo_debuff_total_secs = dur;
                world.players[victim_idx].cryo_slow_ratio = slow_r;
                world.players[victim_idx].cryo_heal_reduction_ratio = heal_r;
                if world.players[victim_idx].slot_index == world.active_slot {
                    world.cryo_debuff_remaining = dur;
                    world.cryo_debuff_total_secs = dur;
                    world.cryo_slow_ratio = slow_r;
                    world.cryo_heal_reduction_ratio = heal_r;
                }
            }
        }
    }

    // 6. مکش سلامتی (Lifesteal) روی هیروها با کنترل کانفیگی + افکت سبز
    if ce.lifesteal_applies_to_heroes {
        if let Some(ai) = attacker_idx {
            if !world.players[ai].is_dead && ai != victim_idx {
                let ls = world.players[ai].hero_lifesteal_ratio;
                if ls > 0.0 {
                    let heal_mult = if world.players[ai].cryo_debuff_remaining > 0.0 {
                        (1.0 - world.players[ai].cryo_heal_reduction_ratio).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let heal = eff_dmg * ls * heal_mult;
                    if heal > 0.0 {
                        let prev_hp = world.players[ai].hero_hp;
                        world.players[ai].hero_hp =
                            (prev_hp + heal).min(world.players[ai].hero_max_hp);
                        let actual_heal = world.players[ai].hero_hp - prev_hp;
                        if actual_heal >= ce.lifesteal_heal_fx_min_amount
                            || (is_continuous_tick && actual_heal > 0.05 && world.tick % 18 == 0)
                        {
                            let apos = world.players[ai].soldier_pos;
                            let aslot = world.players[ai].slot_index;
                            world.pending_fx.push(GameplayFxEvent::LifestealHeal {
                                slot_index: aslot,
                                pos: apos,
                                amount: actual_heal,
                            });
                        }
                    }
                }
            }
        }
    }

    // 7. بازتاب آسیب (Aegis Mirror Harness): هر دو حالت Passive و Active بر اساس دمیج خام قبل از Armor قربانی
    //    فرمول Passive: flat + raw_pre_armor_dmg * passive_ratio  (سپس کاهش توسط Armor مهاجم)
    //    فرمول Active : flat + raw_pre_armor_dmg * active_ratio   (سپس کاهش توسط Armor مهاجم)
    let is_reflect_active = world.players[victim_idx].reflect_armor_remaining > 0.0;
    let has_reflect_passive = world.players[victim_idx].has_reflect_armor_passive;
    if is_reflect_active || has_reflect_passive {
        let victim_slot = world.players[victim_idx].slot_index;
        let victim_pos = world.players[victim_idx].soldier_pos;
        let (base_flat, reflect_ratio) = if is_reflect_active {
            (ra.active_flat_damage, ra.active_damage_ratio)
        } else {
            (ra.passive_flat_damage, ra.passive_damage_ratio)
        };
        // جلوگیری از اکسپلویت دمیج ثابت روی Beamهای ۶۰ هرتز و شات‌گان چندساچمه‌ای
        let flat_component = if is_continuous_tick {
            base_flat / (config.network.server_tick_rate_hz.max(1.0) as f32)
        } else if is_shotgun_pellet {
            base_flat * ra.shotgun_flat_reflect_scale
        } else {
            base_flat
        };
        let raw_return_dmg = flat_component + raw_pre_armor_dmg * reflect_ratio;
        let mut reflected_any = false;

        // 7a. بازتاب روی هیروها (کاهش‌یافته توسط Armor خود مهاجم)
        if ra.reflect_applies_to_heroes && raw_return_dmg > 0.0 {
            if let Some(ai) = attacker_idx {
                if ai != victim_idx
                    && !world.players[ai].is_dead
                    && world.players[ai].bullet_immunity_remaining <= 0.0
                {
                    let attacker_armor = world.players[ai].hero_armor
                        - if world.players[ai].flame_armor_debuff_remaining > 0.0 {
                            world.players[ai].flame_armor_debuff_amount
                        } else {
                            0.0
                        };
                    let reflected_dmg = raw_return_dmg * dota2_armor_multiplier(attacker_armor);
                    world.players[ai].hero_hp = (world.players[ai].hero_hp - reflected_dmg).max(0.0);
                    record_hero_damage_on_victim(world, ai, Some(victim_slot), config);
                    reflected_any = true;
                    if !is_continuous_tick || world.tick % 16 == 0 {
                        let apos = world.players[ai].soldier_pos;
                        let show_dmg = if is_continuous_tick {
                            (reflected_dmg * 16.0).max(1.0)
                        } else {
                            reflected_dmg
                        };
                        if show_dmg >= 0.5 {
                            world.pending_fx.push(GameplayFxEvent::DamageNumber {
                                pos: apos + Vec3::Y * 1.5,
                                amount: show_dmg,
                                is_crit: false,
                            });
                        }
                    }
                    if world.players[ai].hero_hp <= 0.0 && !world.players[ai].is_dead {
                        world.swap_slot_in(ai);
                        trigger_random_death(world, config);
                        world.swap_slot_in(ai);
                        award_hero_kill_rewards(world, ai, Some(victim_slot), config, items_cfg);
                    }
                }
            }
        }

        // 7b. بازتاب روی سربازان لاین و کمپ‌های جنگل (کانفیگی + کاهش توسط Armor سرباز)
        if ra.reflect_applies_to_creeps && raw_return_dmg > 0.0 {
            if let Some(cid) = attacker_creep_id {
                if let Some(c_idx) = world.creeps.iter().position(|c| c.id == cid && !c.is_dead) {
                    let c_stats = resolve_creep_stats(config, world.creeps[c_idx].creep_type_id.as_deref());
                    let (_, _, armor_bonus, _, _) = lane_creep_buffs(
                        config,
                        &world.barracks,
                        world.creeps[c_idx].team,
                        world.creeps[c_idx].lane,
                        world.creeps[c_idx].camp_index.is_none(),
                    );
                    let (_, time_armor_bonus, _, _) = lane_creep_time_scaling(
                        config,
                        world.match_elapsed_secs,
                        world.creeps[c_idx].camp_index.is_none(),
                    );
                    let creep_armor = c_stats.armor + time_armor_bonus + armor_bonus;
                    let reflected_dmg = raw_return_dmg
                        * ra.reflect_creep_damage_ratio
                        * dota2_armor_multiplier(creep_armor);
                    world.creeps[c_idx].hp = (world.creeps[c_idx].hp - reflected_dmg).max(0.0);
                    let cpos = world.creeps[c_idx].pos;
                    reflected_any = true;
                    if reflected_dmg >= 0.5 {
                        world.pending_fx.push(GameplayFxEvent::DamageNumber {
                            pos: cpos + Vec3::Y * 1.2,
                            amount: reflected_dmg,
                            is_crit: false,
                        });
                    }
                    if world.creeps[c_idx].hp <= 0.0 && !world.creeps[c_idx].is_dead {
                        world.creeps[c_idx].is_dead = true;
                        world.creeps[c_idx].anim_state = CreepAnimState::Die;
                        world.creeps[c_idx].despawn_timer = c_stats.die_despawn_delay_secs;
                        let gold_bounty = lane_creep_gold_bounty(
                            config,
                            world.match_elapsed_secs,
                            world.creeps[c_idx].camp_index.is_none(),
                            c_stats.gold_bounty,
                        );
                        world.players[victim_idx].last_hits += 1;
                        world.players[victim_idx].hero_gold =
                            world.players[victim_idx].hero_gold.saturating_add(gold_bounty);
                        world.pending_private_fx.push((
                            victim_slot,
                            GameplayFxEvent::LastHitGold {
                                pos: cpos,
                                gold: gold_bounty,
                            },
                        ));
                    }
                }
            }
        }

        // 7c. بازتاب روی تاورها (کانفیگی + کاهش توسط Armor تاور)
        if ra.reflect_applies_to_turrets && raw_return_dmg > 0.0 {
            if let Some(t_idx) = attacker_turret_index {
                if let Some(tur) = world.turrets.get_mut(t_idx) {
                    if !tur.is_dead && !tur.is_invulnerable {
                        let reflected_dmg = raw_return_dmg
                            * ra.reflect_turret_damage_ratio
                            * dota2_armor_multiplier(tur.armor);
                        tur.hp = (tur.hp - reflected_dmg).max(0.0);
                        let tpos = tur.pos;
                        reflected_any = true;
                        if reflected_dmg >= 0.5 {
                            world.pending_fx.push(GameplayFxEvent::DamageNumber {
                                pos: tpos + Vec3::Y * 3.0,
                                amount: reflected_dmg,
                                is_crit: false,
                            });
                        }
                    }
                }
            }
        }

        // پمپاژ لحظه‌ای شیلد مغناطیسی در کلاینت (فقط هنگام فعال بودن شیلد Active)
        if is_reflect_active && reflected_any && (!is_continuous_tick || world.tick % 10 == 0) {
            world.pending_fx.push(GameplayFxEvent::ReflectHitPulse {
                slot_index: victim_slot,
                pos: victim_pos,
            });
        }
    }

    // 8. بررسی مرگ قربانی
    let victim_killed = world.players[victim_idx].hero_hp <= 0.0 && !world.players[victim_idx].is_dead;
    if victim_killed {
        world.swap_slot_in(victim_idx);
        trigger_random_death(world, config);
        world.swap_slot_in(victim_idx);
        award_hero_kill_rewards(world, victim_idx, attacker_slot, config, items_cfg);
    }

    (eff_dmg, victim_killed, is_crit)
}

/// اعمال ضربه بحرانی، مکش سلامتی روی سربازها/کمپ‌ها (کانفیگی) و عدد دمیج جامع روی کریپ‌ها
fn apply_creep_combat_effects(
    world: &mut AuthoritativeWorldState,
    attacker_slot: Option<usize>,
    is_enemy_creep: bool,
    mut raw_pre_armor_dmg: f32,
    armor_mult: f32,
    min_dmg: f32,
    is_continuous_tick: bool,
    creep_pos: Vec3,
    items_cfg: &ItemsConfig,
) -> (f32, bool) {
    let ce = &items_cfg.combat_effects;
    let attacker_idx =
        attacker_slot.and_then(|s| world.players.iter().position(|p| p.slot_index == s));

    let mut is_crit = false;
    if is_enemy_creep {
        if let Some(ai) = attacker_idx {
            let crit_chance = world.players[ai].hero_crit_chance;
            let crit_mult = world.players[ai].hero_crit_multiplier;
            if crit_chance > 0.0 && crit_mult > 1.0 {
                if is_continuous_tick {
                    raw_pre_armor_dmg *= 1.0 + crit_chance * (crit_mult - 1.0);
                } else if world.next_rand() < crit_chance {
                    raw_pre_armor_dmg *= crit_mult;
                    is_crit = true;
                }
            }
        }
    }

    let eff_dmg = (raw_pre_armor_dmg * armor_mult).max(min_dmg);

    if !is_continuous_tick {
        if eff_dmg >= 0.5 {
            world.pending_fx.push(GameplayFxEvent::DamageNumber {
                pos: creep_pos + Vec3::Y * 1.25,
                amount: eff_dmg,
                is_crit,
            });
        }
    } else if world.tick % 16 == 0 && eff_dmg > 0.01 {
        world.pending_fx.push(GameplayFxEvent::DamageNumber {
            pos: creep_pos + Vec3::Y * 1.25,
            amount: (eff_dmg * 16.0).max(1.0),
            is_crit: false,
        });
    }

    // مکش سلامتی روی سربازان لاین و کمپ‌های جنگل (کانفیگی)
    if is_enemy_creep && ce.lifesteal_applies_to_creeps {
        if let Some(ai) = attacker_idx {
            if !world.players[ai].is_dead {
                let ls = world.players[ai].hero_lifesteal_ratio * ce.lifesteal_creep_ratio_mult;
                if ls > 0.0 {
                    let heal_mult = if world.players[ai].cryo_debuff_remaining > 0.0 {
                        (1.0 - world.players[ai].cryo_heal_reduction_ratio).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    let heal = eff_dmg * ls * heal_mult;
                    if heal > 0.0 {
                        let prev_hp = world.players[ai].hero_hp;
                        world.players[ai].hero_hp =
                            (prev_hp + heal).min(world.players[ai].hero_max_hp);
                        let actual_heal = world.players[ai].hero_hp - prev_hp;
                        if actual_heal >= ce.lifesteal_heal_fx_min_amount
                            || (is_continuous_tick && actual_heal > 0.05 && world.tick % 18 == 0)
                        {
                            let apos = world.players[ai].soldier_pos;
                            let aslot = world.players[ai].slot_index;
                            world.pending_fx.push(GameplayFxEvent::LifestealHeal {
                                slot_index: aslot,
                                pos: apos,
                                amount: actual_heal,
                            });
                        }
                    }
                }
            }
        }
    }

    (eff_dmg, is_crit)
}

/// نمایش عدد دمیج سفید روی ساختمان‌ها (تاور، باراکس، انشنت) + مکش سلامتی اختیاری بر اساس کانفیگ
fn apply_building_damage_effects(
    world: &mut AuthoritativeWorldState,
    owner_slot: Option<usize>,
    eff_dmg: f32,
    building_pos: Vec3,
    items_cfg: &ItemsConfig,
) {
    if eff_dmg >= 0.5 {
        world.pending_fx.push(GameplayFxEvent::DamageNumber {
            pos: building_pos,
            amount: eff_dmg,
            is_crit: false,
        });
    } else if eff_dmg > 0.01 && world.tick % 16 == 0 {
        world.pending_fx.push(GameplayFxEvent::DamageNumber {
            pos: building_pos,
            amount: (eff_dmg * 16.0).max(1.0),
            is_crit: false,
        });
    }
    let ce = &items_cfg.combat_effects;
    if ce.lifesteal_applies_to_buildings {
        if let Some(ai) = owner_slot.and_then(|s| world.players.iter().position(|p| p.slot_index == s)) {
            if !world.players[ai].is_dead && world.players[ai].hero_lifesteal_ratio > 0.0 {
                let heal_mult = if world.players[ai].cryo_debuff_remaining > 0.0 {
                    (1.0 - world.players[ai].cryo_heal_reduction_ratio).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let heal = eff_dmg * world.players[ai].hero_lifesteal_ratio * heal_mult;
                if heal > 0.0 {
                    let prev_hp = world.players[ai].hero_hp;
                    world.players[ai].hero_hp = (prev_hp + heal).min(world.players[ai].hero_max_hp);
                    let actual_heal = world.players[ai].hero_hp - prev_hp;
                    if actual_heal >= ce.lifesteal_heal_fx_min_amount {
                        let apos = world.players[ai].soldier_pos;
                        let aslot = world.players[ai].slot_index;
                        world.pending_fx.push(GameplayFxEvent::LifestealHeal {
                            slot_index: aslot,
                            pos: apos,
                            amount: actual_heal,
                        });
                    }
                }
            }
        }
    }
}

fn apply_damage_to_turret(
    commands: &mut Commands,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    world: &mut AuthoritativeWorldState,
    turret_entity: Option<Entity>,
    tur_idx: usize,
    owner_slot: Option<usize>,
    owner_team: TeamId,
    raw_dmg: f32,
    min_dmg: f32,
) -> (bool, bool) {
    let turret_team = world.turrets[tur_idx].team;
    let turret_invuln = world.turrets[tur_idx].is_invulnerable;
    let turret_dead = world.turrets[tur_idx].is_dead;
    if owner_team == turret_team || turret_invuln || turret_dead || raw_dmg <= 0.0 {
        return (false, false);
    }
    let dmg_mult = dota2_armor_multiplier(world.turrets[tur_idx].armor);
    let eff_dmg = (raw_dmg * dmg_mult).max(min_dmg);
    let t_pos = world.turrets[tur_idx].pos + Vec3::Y * 3.2;
    world.turrets[tur_idx].hp = (world.turrets[tur_idx].hp - eff_dmg).max(0.0);
    apply_building_damage_effects(world, owner_slot, eff_dmg, t_pos, items_cfg);
    if world.turrets[tur_idx].hp <= 0.0 && !world.turrets[tur_idx].is_dead {
        world.turrets[tur_idx].is_dead = true;
        world.turrets[tur_idx].aim_target_pos = None;
        if let Some(ent) = turret_entity {
            commands.entity(ent).insert(ColliderDisabled);
        }
        let tur_pos = world.turrets[tur_idx].pos;
        let gold_b = config.map.turrets[tur_idx].gold_bounty;
        let xp_b = config.map.turrets[tur_idx].xp_bounty;
        if let Some(o_slot) = owner_slot {
            let world_inner = &mut *world;
            if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == o_slot) {
                killer.last_hits += 1;
                killer.hero_gold += gold_b;
                world_inner.pending_private_fx.push((
                    o_slot,
                    GameplayFxEvent::LastHitGold {
                        pos: tur_pos + Vec3::Y * 2.5,
                        gold: gold_b,
                    },
                ));
            }
        }
        let eligible: Vec<usize> = world
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                !p.is_dead
                    && p.hero_team == owner_team
                    && p.soldier_pos.distance(tur_pos) <= p.hero_xp_gain_radius
            })
            .map(|(i, _)| i)
            .collect();
        if !eligible.is_empty() && xp_b > 0 {
            let share = xp_b / (eligible.len() as u32);
            let world_inner = &mut *world;
            for idx in eligible {
                grant_xp_to_player_slot(
                    &mut world_inner.players[idx],
                    share,
                    config,
                    items_cfg,
                    &mut world_inner.pending_private_fx,
                );
            }
        }
        return (true, true);
    }
    (true, false)
}

/// اعمال یکپارچه دمیج و پاداش نابودی روی Barracks (مشترک بین Bullet، Beam و AOE)
#[allow(clippy::too_many_arguments)]
fn apply_damage_to_barracks(
    commands: &mut Commands,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    world: &mut AuthoritativeWorldState,
    bar_entity: Option<Entity>,
    bar_idx: usize,
    owner_slot: Option<usize>,
    owner_team: TeamId,
    raw_dmg: f32,
    min_dmg: f32,
) -> (bool, bool) {
    let bar_team = world.barracks[bar_idx].team;
    let bar_lane = world.barracks[bar_idx].lane;
    let bar_kind = world.barracks[bar_idx].kind;
    let bar_invuln = world.barracks[bar_idx].is_invulnerable;
    let bar_dead = world.barracks[bar_idx].is_dead;
    if owner_team == bar_team || bar_invuln || bar_dead || raw_dmg <= 0.0 {
        return (false, false);
    }
    let dmg_mult = dota2_armor_multiplier(world.barracks[bar_idx].armor);
    let eff_dmg = (raw_dmg * dmg_mult).max(min_dmg);
    let b_pos = world.barracks[bar_idx].pos + Vec3::Y * 2.4;
    world.barracks[bar_idx].hp = (world.barracks[bar_idx].hp - eff_dmg).max(0.0);
    apply_building_damage_effects(world, owner_slot, eff_dmg, b_pos, items_cfg);
    if world.barracks[bar_idx].hp <= 0.0 && !world.barracks[bar_idx].is_dead {
        world.barracks[bar_idx].is_dead = true;
        if let Some(ent) = bar_entity {
            commands.entity(ent).insert(ColliderDisabled);
        }
        let bar_pos = world.barracks[bar_idx].pos;
        let gold_b = config.map.barracks[bar_idx].gold_bounty;
        let xp_b = config.map.barracks[bar_idx].xp_bounty;
        if bar_kind == BarracksKind::Melee {
            let hp_mult = config.map.barracks_buffs.melee_destroyed_hp_multiplier;
            for c in &mut world.creeps {
                if !c.is_dead && c.camp_index.is_none() && c.team != bar_team && c.lane == bar_lane {
                    c.max_hp *= hp_mult;
                    c.hp = (c.hp * hp_mult).min(c.max_hp);
                }
            }
        }
        if let Some(o_slot) = owner_slot {
            let world_inner = &mut *world;
            if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == o_slot) {
                killer.last_hits += 1;
                killer.hero_gold += gold_b;
                world_inner.pending_private_fx.push((
                    o_slot,
                    GameplayFxEvent::LastHitGold {
                        pos: bar_pos + Vec3::Y * 2.5,
                        gold: gold_b,
                    },
                ));
            }
        }
        let eligible: Vec<usize> = world
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                !p.is_dead
                    && p.hero_team == owner_team
                    && p.soldier_pos.distance(bar_pos) <= p.hero_xp_gain_radius
            })
            .map(|(i, _)| i)
            .collect();
        if !eligible.is_empty() && xp_b > 0 {
            let share = xp_b / (eligible.len() as u32);
            let world_inner = &mut *world;
            for idx in eligible {
                grant_xp_to_player_slot(
                    &mut world_inner.players[idx],
                    share,
                    config,
                    items_cfg,
                    &mut world_inner.pending_private_fx,
                );
            }
        }
        return (true, true);
    }
    (true, false)
}

/// اعمال یکپارچه دمیج و پایان بازی روی Ancient (مشترک بین Bullet، Beam و AOE)
#[allow(clippy::too_many_arguments)]
fn apply_damage_to_ancient(
    commands: &mut Commands,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    world: &mut AuthoritativeWorldState,
    anc_entity: Option<Entity>,
    anc_idx: usize,
    owner_team: TeamId,
    raw_dmg: f32,
    min_dmg: f32,
) -> (bool, bool) {
    let anc_team = world.ancients[anc_idx].team;
    let anc_invuln = world.ancients[anc_idx].is_invulnerable;
    let anc_dead = world.ancients[anc_idx].is_dead;
    if owner_team == anc_team || anc_invuln || anc_dead || raw_dmg <= 0.0 {
        return (false, false);
    }
    let dmg_mult = dota2_armor_multiplier(world.ancients[anc_idx].armor);
    let eff_dmg = (raw_dmg * dmg_mult).max(min_dmg);
    let a_pos = world.ancients[anc_idx].pos + Vec3::Y * 3.4;
    world.ancients[anc_idx].hp = (world.ancients[anc_idx].hp - eff_dmg).max(0.0);
    apply_building_damage_effects(world, None, eff_dmg, a_pos, items_cfg);
    if world.ancients[anc_idx].hp <= 0.0 && !world.ancients[anc_idx].is_dead {
        world.ancients[anc_idx].is_dead = true;
        if let Some(ent) = anc_entity {
            commands.entity(ent).insert(ColliderDisabled);
        }
        let anc_pos = world.ancients[anc_idx].pos;
        let winner = if anc_team == TeamId::Dawn {
            TeamId::Dusk
        } else {
            TeamId::Dawn
        };
        world.winning_team = Some(winner);
        world.pending_fx.push(GameplayFxEvent::GrenadeExploded {
            pos: anc_pos + Vec3::Y * 3.0,
            aoe_radius: items_cfg.stat_limits.ancient_destroy_explosion_radius,
            in_water: false,
            water_y: anc_pos.y,
            trauma_add: config.combat.grenade_explosion_trauma_add,
            trauma_max: config.combat.grenade_explosion_trauma_max,
            weapon_source: None,
        });
        return (true, true);
    }
    (true, false)
}

#[allow(clippy::too_many_arguments)]
fn apply_aoe_explosion(
    commands: &mut Commands,
    config: &GameConfig,
    items_cfg: &crate::shop_shared::ItemsConfig,
    world: &mut AuthoritativeWorldState,
    target_entity_by_idx: &HashMap<usize, Entity>,
    turret_entity_by_idx: &HashMap<usize, Entity>,
    barracks_entity_by_idx: &HashMap<usize, Entity>,
    ancient_entity_by_idx: &HashMap<usize, Entity>,
    pos: Vec3,
    owner_slot: Option<usize>,
    owner_team: TeamId,
    dmg: f32,
    aoe_radius: f32,
    weapon_source: Option<BulletVfxSource>,
) {
    // ۱. صدمه به اهداف تمرینی داخل شعاع AOE
    for idx in 0..world.targets.len() {
        let target_dead = world.targets[idx].is_dead;
        let t_pos = world.targets[idx].base_pos;
        if !target_dead && t_pos.distance(pos) <= aoe_radius {
            world.targets[idx].hp -= dmg;
            world.targets[idx].hit_flash_timer = config.fx.target_hit_flash_secs;
            if world.targets[idx].hp <= 0.0 {
                world.targets[idx].hp = 0.0;
                world.targets[idx].is_dead = true;
                world.targets[idx].respawn_remaining = world.targets[idx].respawn_duration;
                if let Some(&t_ent) = target_entity_by_idx.get(&idx) {
                    commands.entity(t_ent).insert(ColliderDisabled);
                }
                if let Some(o_slot) = owner_slot {
                    if let Some(attacker) = world.players.iter_mut().find(|p| p.slot_index == o_slot) {
                        attacker.last_hits += 1;
                        attacker.hero_gold += 35;
                        if attacker.soldier_pos.distance(t_pos) <= attacker.hero_xp_gain_radius {
                            grant_xp_to_player_slot(attacker, 45, config, items_cfg, &mut world.pending_private_fx);
                        }
                    }
                }
            }
        }
    }

    // ۲. صدمه AOE به هیروهای دشمن داخل شعاع انفجار
    for p_idx in 0..world.players.len() {
        if world.players[p_idx].is_dead || world.players[p_idx].hero_team == owner_team {
            continue;
        }
        let h_pos = world.players[p_idx].soldier_pos + Vec3::Y * 0.9;
        if h_pos.distance(pos) <= aoe_radius {
            apply_combat_damage_to_hero(
                world,
                p_idx,
                owner_slot,
                None,
                None,
                dmg,
                1.0,
                false,
                false,
                false,
                true,
                h_pos,
                config,
                items_cfg,
            );
        }
    }

    // ۳. صدمه AOE به سربازان لاین و کمپ‌های دشمن
    for c_idx in 0..world.creeps.len() {
        if world.creeps[c_idx].is_dead || world.creeps[c_idx].team == owner_team {
            continue;
        }
        let c_pos = world.creeps[c_idx].pos;
        if (c_pos + Vec3::Y * 0.8).distance(pos) <= aoe_radius {
            let c_stats = resolve_creep_stats(config, world.creeps[c_idx].creep_type_id.as_deref());
            let (_, _, armor_bonus, _, _) = lane_creep_buffs(
                config,
                &world.barracks,
                world.creeps[c_idx].team,
                world.creeps[c_idx].lane,
                world.creeps[c_idx].camp_index.is_none(),
            );
            let (_, time_armor_bonus, _, _) = lane_creep_time_scaling(
                config,
                world.match_elapsed_secs,
                world.creeps[c_idx].camp_index.is_none(),
            );
            let (eff_dmg, _) = apply_creep_combat_effects(
                world,
                owner_slot,
                true,
                dmg,
                dota2_armor_multiplier(c_stats.armor + time_armor_bonus + armor_bonus),
                1.0,
                false,
                c_pos,
                items_cfg,
            );
            world.creeps[c_idx].hp = (world.creeps[c_idx].hp - eff_dmg).max(0.0);
            aggro_creep_and_camp_on_hit(world, c_idx, owner_slot, config);
            if world.creeps[c_idx].hp <= 0.0 && !world.creeps[c_idx].is_dead {
                world.creeps[c_idx].is_dead = true;
                world.creeps[c_idx].anim_state = CreepAnimState::Die;
                world.creeps[c_idx].despawn_timer = c_stats.die_despawn_delay_secs;
                let gold_bounty = lane_creep_gold_bounty(config, world.match_elapsed_secs, world.creeps[c_idx].camp_index.is_none(), c_stats.gold_bounty);
                let xp_bounty = c_stats.xp_bounty;
                if let Some(o_slot) = owner_slot {
                    if let Some(killer) = world.players.iter_mut().find(|p| p.slot_index == o_slot) {
                        killer.last_hits += 1;
                        killer.hero_gold += gold_bounty;
                        world.pending_private_fx.push((
                            o_slot,
                            GameplayFxEvent::LastHitGold {
                                pos: c_pos,
                                gold: gold_bounty,
                            },
                        ));
                    }
                }
                let eligible_indices: Vec<usize> = world
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        !p.is_dead
                            && p.hero_team == owner_team
                            && (p.soldier_pos.distance(c_pos) <= p.hero_xp_gain_radius
                                || (config.gameplay_rules.last_hitter_always_gets_xp
                                    && owner_slot == Some(p.slot_index)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                if !eligible_indices.is_empty() {
                    let share = xp_bounty / (eligible_indices.len() as u32);
                    for idx in eligible_indices {
                        let p = &mut world.players[idx];
                        grant_xp_to_player_slot(p, share, config, items_cfg, &mut world.pending_private_fx);
                    }
                }
            }
        }
    }

    // ۴. صدمه AOE به Courierهای دشمن داخل شعاع انفجار
    for p_idx in 0..world.players.len() {
        if world.players[p_idx].courier_is_dead || world.players[p_idx].hero_team == owner_team {
            continue;
        }
        let h_cfg = config.active_hero(world.players[p_idx].hero_index).clone();
        let c_pos = world.players[p_idx].courier_pos;
        if c_pos.distance(pos) <= aoe_radius + h_cfg.courier.hit_radius {
            if config.gameplay_rules.courier_invulnerable_in_fountain
                && fountain_regen_bonus_for_pos(config, world.players[p_idx].hero_team, c_pos) > 0.0
            {
                continue;
            }
            world.players[p_idx].courier_hp = (world.players[p_idx].courier_hp - dmg).max(0.0);
            if world.players[p_idx].courier_hp <= 0.0 && !world.players[p_idx].courier_is_dead {
                world.players[p_idx].courier_is_dead = true;
                world.players[p_idx].courier_respawn_remaining = h_cfg.courier.respawn_secs;
                world.players[p_idx].courier_order = CourierOrder::Idle;
                if let Some(o_slot) = owner_slot {
                    let w_inner = &mut *world;
                    if let Some(killer) = w_inner.players.iter_mut().find(|p| p.slot_index == o_slot) {
                        killer.last_hits += 1;
                        killer.hero_gold = killer.hero_gold.saturating_add(h_cfg.courier.bounty_gold);
                        w_inner.pending_private_fx.push((
                            o_slot,
                            GameplayFxEvent::LastHitGold {
                                pos: c_pos,
                                gold: h_cfg.courier.bounty_gold,
                            },
                        ));
                    }
                }
            }
        }
    }

    // ۵. صدمه AOE به ساختمان‌های دشمن (Turret / Barracks / Ancient) با ضریب کانفیگ
    let bldg_dmg = dmg * items_cfg.stat_limits.aoe_building_damage_ratio;
    if bldg_dmg > 0.0 {
        for tur_idx in 0..world.turrets.len() {
            if world.turrets[tur_idx].is_dead
                || world.turrets[tur_idx].is_invulnerable
                || world.turrets[tur_idx].team == owner_team
            {
                continue;
            }
            let t_def = &config.map.turrets[world.turrets[tur_idx].index];
            let s = t_def.model_transform.scale_vec();
            let t_rad = t_def.base_radius * s.x.max(s.z);
            let t_pos = world.turrets[tur_idx].pos;
            let dist_xz = Vec2::new(t_pos.x - pos.x, t_pos.z - pos.z).length();
            if dist_xz <= aoe_radius + t_rad {
                apply_damage_to_turret(
                    commands,
                    config,
                    items_cfg,
                    world,
                    turret_entity_by_idx.get(&tur_idx).copied(),
                    tur_idx,
                    owner_slot,
                    owner_team,
                    bldg_dmg,
                    items_cfg.stat_limits.min_weapon_damage,
                );
            }
        }
        for bar_idx in 0..world.barracks.len() {
            if world.barracks[bar_idx].is_dead
                || world.barracks[bar_idx].is_invulnerable
                || world.barracks[bar_idx].team == owner_team
            {
                continue;
            }
            let b_def = &config.map.barracks[world.barracks[bar_idx].index];
            let s = b_def.model_transform.scale_vec();
            let b_rad = 0.5 * (b_def.size.0 * s.x).max(b_def.size.2 * s.z);
            let b_pos = world.barracks[bar_idx].pos;
            let dist_xz = Vec2::new(b_pos.x - pos.x, b_pos.z - pos.z).length();
            if dist_xz <= aoe_radius + b_rad {
                apply_damage_to_barracks(
                    commands,
                    config,
                    items_cfg,
                    world,
                    barracks_entity_by_idx.get(&bar_idx).copied(),
                    bar_idx,
                    owner_slot,
                    owner_team,
                    bldg_dmg,
                    items_cfg.stat_limits.min_weapon_damage,
                );
            }
        }
        for anc_idx in 0..world.ancients.len() {
            if world.ancients[anc_idx].is_dead
                || world.ancients[anc_idx].is_invulnerable
                || world.ancients[anc_idx].team == owner_team
            {
                continue;
            }
            let a_def = &config.map.ancients[world.ancients[anc_idx].index];
            let s = a_def.model_transform.scale_vec();
            let a_rad = 0.5 * (a_def.base_size.0 * s.x).max(a_def.base_size.2 * s.z);
            let a_pos = world.ancients[anc_idx].pos;
            let dist_xz = Vec2::new(a_pos.x - pos.x, a_pos.z - pos.z).length();
            if dist_xz <= aoe_radius + a_rad {
                apply_damage_to_ancient(
                    commands,
                    config,
                    items_cfg,
                    world,
                    ancient_entity_by_idx.get(&anc_idx).copied(),
                    anc_idx,
                    owner_team,
                    bldg_dmg,
                    items_cfg.stat_limits.min_weapon_damage,
                );
            }
        }
    }

    let water_hit = sample_water_surface_at(config, pos);
    if let Some((water_y, _)) = water_hit {
        world.pending_fx.push(GameplayFxEvent::WaterSplash {
            pos: Vec3::new(pos.x, water_y, pos.z),
            intensity: 2.2,
        });
    }

    world.pending_fx.push(GameplayFxEvent::GrenadeExploded {
        pos,
        aoe_radius,
        in_water: water_hit.is_some(),
        water_y: water_hit.map(|(wy, _)| wy).unwrap_or(pos.y),
        trauma_add: config.combat.grenade_explosion_trauma_add,
        trauma_max: config.combat.grenade_explosion_trauma_max,
        weapon_source,
    });
}


fn creep_toxic_slow_at(pools: &[ActiveToxicPool], pos: Vec3, team: TeamId) -> f32 {
    let mut mult = 1.0_f32;
    for pool in pools {
        if pool.owner_team != team
            && pool.remaining_secs > 0.0
            && Vec2::new(pos.x - pool.pos.x, pos.z - pool.pos.z).length() <= pool.radius
            && (pos.y - pool.pos.y).abs() <= pool.vertical_tolerance
        {
            mult = mult.min((1.0 - pool.slow_ratio).clamp(pool.min_slow_mult, 1.0));
        }
    }
    mult
}

#[allow(clippy::too_many_arguments)]
fn shared_authoritative_react_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_cfg: Res<crate::shop_shared::ItemsConfig>,
    match_state: Option<Res<AuthoritativeMatchState>>,
    physics_world: Res<CustomPhysicsWorld>,
    mut world: ResMut<AuthoritativeWorldState>,
    soldier_query: Query<(&PhysicsSoldier, &Transform)>,
    mut bullet_query: Query<
        (Entity, &mut PhysicsBullet, &mut Transform),
        (Without<PhysicsSoldier>, Without<PhysicsGrenade>),
    >,
    mut grenade_query: Query<
        (Entity, &mut PhysicsGrenade, &Transform, &Velocity),
        (Without<PhysicsSoldier>, Without<PhysicsBullet>),
    >,
    target_query: Query<(Entity, &PhysicsTarget)>,
    turret_query: Query<(Entity, &PhysicsTurret)>,
    barracks_query: Query<(Entity, &PhysicsBarracks)>,
    ancient_query: Query<(Entity, &PhysicsAncient)>,
    tree_query: Query<(), With<PhysicsTree>>,
) {
    if match_state
        .as_ref()
        .is_some_and(|m| m.phase != MatchPhase::InGame)
    {
        return;
    }
    if world.pause_phase != PausePhase::Running {
        return;
    }
    if world.winning_team.is_some() {
        for (entity, _, _) in &bullet_query {
            commands.entity(entity).despawn();
        }
        for (entity, _, _, _) in &grenade_query {
            commands.entity(entity).despawn();
        }
        world.bullets_snapshot.clear();
        world.grenades_snapshot.clear();
        return;
    }
    let dt = time.delta_secs();

    for (p_soldier, soldier_tf) in &soldier_query {
        let slot_idx = p_soldier.slot_index;
        let has_slots = !world.players.is_empty();
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
        if (world.traversal.kind != TraversalKind::ObstacleVault
            && world.traversal.kind != TraversalKind::JumpOver)
            || !world.traversal.is_active
        {
            world.soldier_pos = Vec3::new(
                soldier_tf.translation.x,
                soldier_tf.translation.y - config.physics.soldier_capsule_center_y * world.hero_scale,
                soldier_tf.translation.z,
            );
        }
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
    }

    let mut target_entity_by_idx = HashMap::new();
    let mut target_idx_by_entity = HashMap::new();
    for (entity, p_target) in &target_query {
        target_entity_by_idx.insert(p_target.index, entity);
        target_idx_by_entity.insert(entity, p_target.index);
    }
    let mut turret_entity_by_idx = HashMap::new();
    let mut turret_idx_by_entity = HashMap::new();
    for (entity, p_turret) in &turret_query {
        turret_entity_by_idx.insert(p_turret.index, entity);
        turret_idx_by_entity.insert(entity, p_turret.index);
    }
    let mut barracks_entity_by_idx = HashMap::new();
    let mut barracks_idx_by_entity = HashMap::new();
    for (entity, p_barrack) in &barracks_query {
        barracks_entity_by_idx.insert(p_barrack.index, entity);
        barracks_idx_by_entity.insert(entity, p_barrack.index);
    }
    let mut ancient_entity_by_idx = HashMap::new();
    let mut ancient_idx_by_entity = HashMap::new();
    for (entity, p_anc) in &ancient_query {
        ancient_entity_by_idx.insert(p_anc.index, entity);
        ancient_idx_by_entity.insert(entity, p_anc.index);
    }

    let mut exploded_grenades = Vec::new();
    world.grenades_snapshot.clear();

    for (entity, mut grenade, tf, vel) in &mut grenade_query {
        grenade.remaining_fuse -= dt;
        if grenade.remaining_fuse <= 0.0 {
            exploded_grenades.push((
                tf.translation,
                grenade.owner_slot,
                grenade.owner_team,
                grenade.damage,
                grenade.aoe_radius,
            ));
            commands.entity(entity).despawn();
        } else {
            world.grenades_snapshot.push(GrenadeSnapshot {
                id: grenade.id,
                translation: tf.translation,
                rotation: tf.rotation,
                linear_velocity: vel.linear,
                angular_velocity: vel.angular,
            });
        }
    }

    for (pos, owner_slot, owner_team, dmg, aoe_radius) in exploded_grenades {
        apply_aoe_explosion(
            &mut commands,
            &config,
            &items_cfg,
            &mut world,
            &target_entity_by_idx,
            &turret_entity_by_idx,
            &barracks_entity_by_idx,
            &ancient_entity_by_idx,
            pos,
            Some(owner_slot),
            owner_team,
            dmg,
            aoe_radius,
            None,
        );
    }

    let mut aoe_bullet_explosions: Vec<(Vec3, Option<usize>, TeamId, f32, f32, BulletVfxSource)> = Vec::new();
    let mut spawned_toxic_pools: Vec<ActiveToxicPool> = Vec::new();
    world.bullets_snapshot.clear();
    let bullet_query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    for (entity, mut bullet, mut tf) in &mut bullet_query {
        let old_pos = tf.translation;
        if bullet.gravity > 0.0 {
            bullet.velocity.y -= bullet.gravity * dt;
        }
        let step_vec = bullet.velocity * dt;
        let step_dist = step_vec.length();
        let step_dir = step_vec.normalize_or_zero();
        let new_pos = old_pos + step_vec;
        bullet.remaining_range -= step_dist;

        let mut best_hero_hit: Option<(usize, f32)> = None;
        let mut best_creep_hit: Option<(usize, f32)> = None;
        let mut best_courier_hit: Option<(usize, f32)> = None;
        if step_dist > f32::EPSILON {
            for (p_idx, p) in world.players.iter().enumerate() {
                if p.hero_team == bullet.owner_team || p.courier_is_dead {
                    continue;
                }
                if bullet.piercing
                    && bullet
                        .hit_entity_keys
                        .contains(&(2_000_000_u64 + p.slot_index as u64))
                {
                    continue;
                }
                let h_cfg = config.active_hero(p.hero_index);
                let c_rad = h_cfg.courier.hit_radius + config.combat.bullet_hit_forgiveness_radius;
                if let Some(toi) = segment_hits_hero_capsule(
                    old_pos,
                    step_dir,
                    step_dist,
                    p.courier_pos - Vec3::Y * c_rad,
                    c_rad,
                    c_rad * 2.0,
                    0.2,
                    0.2,
                ) {
                    if best_courier_hit.map_or(true, |(_, prev_toi)| toi < prev_toi) {
                        best_courier_hit = Some((p_idx, toi));
                    }
                }
            }
        }
        if step_dist > f32::EPSILON {
            for (p_idx, p) in world.players.iter().enumerate() {
                if Some(p.slot_index) == bullet.owner_slot || p.hero_team == bullet.owner_team || p.is_dead {
                    continue;
                }
                if bullet.piercing
                    && bullet
                        .hit_entity_keys
                        .contains(&(1_000_000_u64 + p.slot_index as u64))
                {
                    continue;
                }
                let h_rad = config.soldier.player_radius
                    * p.hero_scale
                    * config.combat.hero_hitbox_radius_multiplier
                    + config.combat.bullet_hit_forgiveness_radius;
                let h_height = if p.effective_stance == Stance::Crouch {
                    config.combat.hero_crouch_hitbox_height * p.hero_scale
                } else {
                    config.combat.hero_stand_hitbox_height * p.hero_scale
                };
                if let Some(toi) = segment_hits_hero_capsule(
                    old_pos,
                    step_dir,
                    step_dist,
                    p.soldier_pos,
                    h_rad,
                    h_height,
                    config.combat.bullet_hit_forgiveness_height_bottom,
                    config.combat.bullet_hit_forgiveness_height_top,
                ) {
                    if best_hero_hit.map_or(true, |(_, prev_toi)| toi < prev_toi) {
                        best_hero_hit = Some((p_idx, toi));
                    }
                }
            }
        }

        if step_dist > f32::EPSILON {
            for (c_idx, c) in world.creeps.iter().enumerate() {
                if c.is_dead {
                    continue;
                }
                if bullet.piercing
                    && bullet
                        .hit_entity_keys
                        .contains(&(3_000_000_u64 + c.id))
                {
                    continue;
                }
                let is_enemy = c.team != bullet.owner_team;
                let is_deniable_by_hero = bullet.owner_slot.is_some()
                    && c.team == bullet.owner_team
                    && (c.hp / c.max_hp) <= config.creeps.deny_hp_threshold_ratio;
                if !is_enemy && !is_deniable_by_hero {
                    continue;
                }
                let c_stats = resolve_creep_stats(&config, c.creep_type_id.as_deref());
                let c_rad = c_stats.collider_radius + config.combat.bullet_hit_forgiveness_radius;
                if let Some(toi) = segment_hits_hero_capsule(
                    old_pos,
                    step_dir,
                    step_dist,
                    c.pos,
                    c_rad,
                    c_stats.hitbox_height,
                    config.combat.bullet_hit_forgiveness_height_bottom,
                    config.combat.bullet_hit_forgiveness_height_top,
                ) {
                    if best_creep_hit.map_or(true, |(_, prev_toi)| toi < prev_toi) {
                        best_creep_hit = Some((c_idx, toi));
                    }
                }
            }
        }

        let ray_hit = if step_dist > f32::EPSILON {
            physics_world
                .cast_ray(old_pos, step_dir, step_dist, bullet_query_groups, |hit_ent| {
                    if let Some(&t_idx) = target_idx_by_entity.get(&hit_ent) {
                        !world.targets[t_idx].is_dead
                    } else {
                        true
                    }
                })
                .map(|(hit_ent, hit)| (hit_ent, hit.point, hit.toi))
        } else {
            None
        };

        let hero_wins = match (best_hero_hit, ray_hit.as_ref()) {
            (Some((_, h_toi)), Some((_, _, s_toi))) => h_toi <= *s_toi,
            (Some(_), None) => true,
            _ => false,
        };

        if hero_wins {
            let (enemy_idx, h_toi) = best_hero_hit.unwrap();
            let hit_key = 1_000_000_u64 + world.players[enemy_idx].slot_index as u64;
            if bullet.piercing && bullet.hit_entity_keys.contains(&hit_key) {
                tf.translation = new_pos;
                world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                    start: old_pos,
                    end: new_pos,
                    vfx_source: bullet.vfx_source.clone(),
                });
                if bullet.remaining_range <= 0.0 {
                    commands.entity(entity).despawn();
                } else {
                    world.bullets_snapshot.push(BulletSnapshot {
                        id: bullet.id,
                        translation: tf.translation,
                        velocity: bullet.velocity,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
                continue;
            }
            let impact_pos = old_pos + step_dir * h_toi;
            if bullet.aoe_radius > 0.0 {
                world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                    start: old_pos,
                    end: impact_pos,
                    vfx_source: bullet.vfx_source.clone(),
                });
                aoe_bullet_explosions.push((
                    impact_pos,
                    bullet.owner_slot,
                    bullet.owner_team,
                    bullet.damage,
                    bullet.aoe_radius,
                    bullet.vfx_source.clone(),
                ));
                if bullet.toxic_pool_duration_secs > 0.0 {
                    let pool_id = world.next_entity_id;
                    world.next_entity_id += 1;
                    spawned_toxic_pools.push(ActiveToxicPool {
                        id: pool_id,
                        pos: impact_pos,
                        radius: bullet.aoe_radius,
                        remaining_secs: bullet.toxic_pool_duration_secs,
                        total_duration: bullet.toxic_pool_duration_secs,
                        dps: bullet.toxic_pool_dps,
                        slow_ratio: bullet.toxic_slow_ratio,
                        min_slow_mult: items_cfg.stat_limits.min_toxic_slow_multiplier,
                        vertical_tolerance: items_cfg.stat_limits.toxic_pool_vertical_tolerance,
                        owner_slot: bullet.owner_slot,
                        owner_team: bullet.owner_team,
                    });
                }
                commands.entity(entity).despawn();
                continue;
            }
            let min_hit_dmg = match &bullet.vfx_source {
                BulletVfxSource::Hero { weapon_kind: WeaponKind::Shotgun, .. } => {
                    items_cfg.stat_limits.min_pellet_effective_damage
                }
                _ => items_cfg.stat_limits.min_weapon_damage,
            };
            let is_shotgun_pellet = matches!(
                &bullet.vfx_source,
                BulletVfxSource::Hero { weapon_kind: WeaponKind::Shotgun, .. }
            );
            let (dealt_dmg, enemy_killed, was_crit) = apply_combat_damage_to_hero(
                &mut world,
                enemy_idx,
                bullet.owner_slot,
                bullet.owner_creep_id,
                bullet.owner_turret_index,
                bullet.damage,
                min_hit_dmg,
                true,
                false,
                is_shotgun_pellet,
                true,
                impact_pos,
                &config,
                &items_cfg,
            );
            if was_crit && dealt_dmg > 0.0 {
                let extra = items_cfg.combat_effects.crit_blood_extra_bursts;
                for _ in 0..extra {
                    let jx = (world.next_rand() - 0.5) * 0.4;
                    let jy = (world.next_rand() - 0.5) * 0.3;
                    let jz = (world.next_rand() - 0.5) * 0.4;
                    world.pending_fx.push(GameplayFxEvent::BulletImpact {
                        impact_pos: impact_pos + Vec3::new(jx, jy, jz),
                        bullet_dir: step_dir,
                        surface: ImpactSurface::Character,
                        target_destroyed: true,
                        trauma_add: config.combat.shot_fire_trauma_add,
                        trauma_max: config.combat.shot_fire_trauma_max,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
            }

            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
                vfx_source: bullet.vfx_source.clone(),
            });
            world.pending_fx.push(GameplayFxEvent::BulletImpact {
                impact_pos,
                bullet_dir: step_dir,
                surface: ImpactSurface::Character,
                target_destroyed: enemy_killed,
                trauma_add: config.combat.shot_fire_trauma_add,
                trauma_max: config.combat.shot_fire_trauma_max,
                vfx_source: bullet.vfx_source.clone(),
            });
            if bullet.piercing {
                bullet.hit_entity_keys.push(1_000_000_u64 + world.players[enemy_idx].slot_index as u64);
                tf.translation = new_pos;
                if bullet.remaining_range <= 0.0 {
                    commands.entity(entity).despawn();
                } else {
                    world.bullets_snapshot.push(BulletSnapshot {
                        id: bullet.id,
                        translation: tf.translation,
                        velocity: bullet.velocity,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
            } else {
                commands.entity(entity).despawn();
            }
            continue;
        }

        let courier_wins = match (best_courier_hit, ray_hit.as_ref()) {
            (Some((_, cour_toi)), Some((_, _, s_toi))) => cour_toi <= *s_toi,
            (Some(_), None) => true,
            _ => false,
        };

        if courier_wins {
            let (cour_p_idx, cour_toi) = best_courier_hit.unwrap();
            let impact_pos = old_pos + step_dir * cour_toi;
            if bullet.aoe_radius > 0.0 {
                world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                    start: old_pos,
                    end: impact_pos,
                    vfx_source: bullet.vfx_source.clone(),
                });
                aoe_bullet_explosions.push((
                    impact_pos,
                    bullet.owner_slot,
                    bullet.owner_team,
                    bullet.damage,
                    bullet.aoe_radius,
                    bullet.vfx_source.clone(),
                ));
                if bullet.toxic_pool_duration_secs > 0.0 {
                    let pool_id = world.next_entity_id;
                    world.next_entity_id += 1;
                    spawned_toxic_pools.push(ActiveToxicPool {
                        id: pool_id,
                        pos: impact_pos,
                        radius: bullet.aoe_radius,
                        remaining_secs: bullet.toxic_pool_duration_secs,
                        total_duration: bullet.toxic_pool_duration_secs,
                        dps: bullet.toxic_pool_dps,
                        slow_ratio: bullet.toxic_slow_ratio,
                        min_slow_mult: items_cfg.stat_limits.min_toxic_slow_multiplier,
                        vertical_tolerance: items_cfg.stat_limits.toxic_pool_vertical_tolerance,
                        owner_slot: bullet.owner_slot,
                        owner_team: bullet.owner_team,
                    });
                }
                commands.entity(entity).despawn();
                continue;
            }
            let h_cfg = config.active_hero(world.players[cour_p_idx].hero_index).clone();
            let cour_pos_check = world.players[cour_p_idx].courier_pos;
            if config.gameplay_rules.courier_invulnerable_in_fountain
                && fountain_regen_bonus_for_pos(&config, world.players[cour_p_idx].hero_team, cour_pos_check) > 0.0
            {
                commands.entity(entity).despawn();
                continue;
            }
            world.players[cour_p_idx].courier_hp =
                (world.players[cour_p_idx].courier_hp - bullet.damage).max(0.0);
            let cour_killed = world.players[cour_p_idx].courier_hp <= 0.0
                && !world.players[cour_p_idx].courier_is_dead;
            if cour_killed {
                world.players[cour_p_idx].courier_is_dead = true;
                world.players[cour_p_idx].courier_respawn_remaining = h_cfg.courier.respawn_secs;
                world.players[cour_p_idx].courier_order = CourierOrder::Idle;
                let cour_pos = world.players[cour_p_idx].courier_pos;
                if let Some(owner_slot) = bullet.owner_slot {
                    let world_inner = &mut *world;
                    if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                        killer.last_hits += 1;
                        killer.hero_gold = killer.hero_gold.saturating_add(h_cfg.courier.bounty_gold);
                        world_inner.pending_private_fx.push((
                            owner_slot,
                            GameplayFxEvent::LastHitGold {
                                pos: cour_pos,
                                gold: h_cfg.courier.bounty_gold,
                            },
                        ));
                    }
                }
                world.pending_fx.push(GameplayFxEvent::GrenadeExploded {
                    pos: cour_pos,
                    aoe_radius: 3.5,
                    in_water: false,
                    water_y: cour_pos.y,
                    trauma_add: config.combat.shot_fire_trauma_add,
                    trauma_max: config.combat.shot_fire_trauma_max,
                    weapon_source: None,
                });
            }
            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
                vfx_source: bullet.vfx_source.clone(),
            });
            world.pending_fx.push(GameplayFxEvent::BulletImpact {
                impact_pos,
                bullet_dir: step_dir,
                surface: ImpactSurface::Character,
                target_destroyed: cour_killed,
                trauma_add: config.combat.shot_fire_trauma_add,
                trauma_max: config.combat.shot_fire_trauma_max,
                vfx_source: bullet.vfx_source.clone(),
            });
            if bullet.piercing {
                bullet.hit_entity_keys.push(2_000_000_u64 + world.players[cour_p_idx].slot_index as u64);
                tf.translation = new_pos;
                if bullet.remaining_range <= 0.0 {
                    commands.entity(entity).despawn();
                } else {
                    world.bullets_snapshot.push(BulletSnapshot {
                        id: bullet.id,
                        translation: tf.translation,
                        velocity: bullet.velocity,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
            } else {
                commands.entity(entity).despawn();
            }
            continue;
        }

        let creep_wins = match (best_creep_hit, ray_hit.as_ref()) {
            (Some((_, c_toi)), Some((_, _, s_toi))) => c_toi <= *s_toi,
            (Some(_), None) => true,
            _ => false,
        };

        if creep_wins {
            let (c_idx, c_toi) = best_creep_hit.unwrap();
            let hit_key = 3_000_000_u64 + world.creeps[c_idx].id;
            if bullet.piercing && bullet.hit_entity_keys.contains(&hit_key) {
                tf.translation = new_pos;
                world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                    start: old_pos,
                    end: new_pos,
                    vfx_source: bullet.vfx_source.clone(),
                });
                if bullet.remaining_range <= 0.0 {
                    commands.entity(entity).despawn();
                } else {
                    world.bullets_snapshot.push(BulletSnapshot {
                        id: bullet.id,
                        translation: tf.translation,
                        velocity: bullet.velocity,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
                continue;
            }
            let impact_pos = old_pos + step_dir * c_toi;
            if bullet.aoe_radius > 0.0 {
                world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                    start: old_pos,
                    end: impact_pos,
                    vfx_source: bullet.vfx_source.clone(),
                });
                aoe_bullet_explosions.push((
                    impact_pos,
                    bullet.owner_slot,
                    bullet.owner_team,
                    bullet.damage,
                    bullet.aoe_radius,
                    bullet.vfx_source.clone(),
                ));
                if bullet.toxic_pool_duration_secs > 0.0 {
                    let pool_id = world.next_entity_id;
                    world.next_entity_id += 1;
                    spawned_toxic_pools.push(ActiveToxicPool {
                        id: pool_id,
                        pos: impact_pos,
                        radius: bullet.aoe_radius,
                        remaining_secs: bullet.toxic_pool_duration_secs,
                        total_duration: bullet.toxic_pool_duration_secs,
                        dps: bullet.toxic_pool_dps,
                        slow_ratio: bullet.toxic_slow_ratio,
                        min_slow_mult: items_cfg.stat_limits.min_toxic_slow_multiplier,
                        vertical_tolerance: items_cfg.stat_limits.toxic_pool_vertical_tolerance,
                        owner_slot: bullet.owner_slot,
                        owner_team: bullet.owner_team,
                    });
                }
                commands.entity(entity).despawn();
                continue;
            }
            let c_stats = resolve_creep_stats(&config, world.creeps[c_idx].creep_type_id.as_deref());
            let (_, _, armor_bonus, _, _) = lane_creep_buffs(
                &config,
                &world.barracks,
                world.creeps[c_idx].team,
                world.creeps[c_idx].lane,
                world.creeps[c_idx].camp_index.is_none(),
            );
            let (_, time_armor_bonus, _, _) = lane_creep_time_scaling(
                &config,
                world.match_elapsed_secs,
                world.creeps[c_idx].camp_index.is_none(),
            );
            let dmg_mult = dota2_armor_multiplier(c_stats.armor + time_armor_bonus + armor_bonus);
            let min_hit_dmg = match &bullet.vfx_source {
                BulletVfxSource::Hero { weapon_kind: WeaponKind::Shotgun, .. } => {
                    items_cfg.stat_limits.min_pellet_effective_damage
                }
                _ => items_cfg.stat_limits.min_weapon_damage,
            };
            let is_enemy_c = bullet.owner_team != world.creeps[c_idx].team;
            let c_hit_pos = world.creeps[c_idx].pos;
            let (effective_dmg, _) = apply_creep_combat_effects(
                &mut world,
                bullet.owner_slot,
                is_enemy_c,
                bullet.damage,
                dmg_mult,
                min_hit_dmg,
                false,
                c_hit_pos,
                &items_cfg,
            );
            world.creeps[c_idx].hp = (world.creeps[c_idx].hp - effective_dmg).max(0.0);
            aggro_creep_and_camp_on_hit(&mut world, c_idx, bullet.owner_slot, &config);

            let creep_killed = world.creeps[c_idx].hp <= 0.0 && !world.creeps[c_idx].is_dead;
            if creep_killed {
                world.creeps[c_idx].is_dead = true;
                world.creeps[c_idx].anim_state = CreepAnimState::Die;
                world.creeps[c_idx].despawn_timer = c_stats.die_despawn_delay_secs;
                let creep_pos = world.creeps[c_idx].pos;
                let creep_team = world.creeps[c_idx].team;
                let gold_bounty = lane_creep_gold_bounty(&config, world.match_elapsed_secs, world.creeps[c_idx].camp_index.is_none(), c_stats.gold_bounty);
                let xp_bounty = c_stats.xp_bounty;
                let is_deny = bullet.owner_team == creep_team;

                if is_deny {
                    if let Some(owner_slot) = bullet.owner_slot {
                        if let Some(denier) = world.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                            denier.denies += 1;
                        }
                    }
                    world.pending_fx.push(GameplayFxEvent::CreepDenied { pos: creep_pos });
                    let total_denied_xp =
                        ((xp_bounty as f32) * config.creeps.deny_enemy_xp_multiplier).round() as u32;
                    let eligible_indices: Vec<usize> = world
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            !p.is_dead
                                && p.hero_team != creep_team
                                && p.soldier_pos.distance(creep_pos) <= p.hero_xp_gain_radius
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if !eligible_indices.is_empty() && total_denied_xp > 0 {
                        let share = total_denied_xp / (eligible_indices.len() as u32);
                        let world_inner = &mut *world;
                        for idx in eligible_indices {
                            grant_xp_to_player_slot(
                                &mut world_inner.players[idx],
                                share,
                                &config,
                                &items_cfg,
                                &mut world_inner.pending_private_fx,
                            );
                        }
                    }
                } else {
                    if let Some(owner_slot) = bullet.owner_slot {
                        let world_inner = &mut *world;
                        if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                            killer.last_hits += 1;
                            killer.hero_gold += gold_bounty;
                            world_inner.pending_private_fx.push((
                                owner_slot,
                                GameplayFxEvent::LastHitGold {
                                    pos: creep_pos,
                                    gold: gold_bounty,
                                },
                            ));
                        }
                    }
                    let killer_team = bullet.owner_team;
                    let eligible_indices: Vec<usize> = world
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            !p.is_dead
                                && p.hero_team == killer_team
                                && (p.soldier_pos.distance(creep_pos) <= p.hero_xp_gain_radius
                                    || (config.gameplay_rules.last_hitter_always_gets_xp
                                        && bullet.owner_slot == Some(p.slot_index)))
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if !eligible_indices.is_empty() {
                        let share = xp_bounty / (eligible_indices.len() as u32);
                        let world_inner = &mut *world;
                        for idx in eligible_indices {
                            grant_xp_to_player_slot(
                                &mut world_inner.players[idx],
                                share,
                                &config,
                                &items_cfg,
                                &mut world_inner.pending_private_fx,
                            );
                        }
                    }
                }
            }

            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
                vfx_source: bullet.vfx_source.clone(),
            });
            world.pending_fx.push(GameplayFxEvent::BulletImpact {
                impact_pos,
                bullet_dir: step_dir,
                surface: ImpactSurface::Character,
                target_destroyed: creep_killed,
                trauma_add: config.combat.shot_fire_trauma_add * 0.5,
                trauma_max: config.combat.shot_fire_trauma_max,
                vfx_source: bullet.vfx_source.clone(),
            });
            if bullet.piercing {
                bullet.hit_entity_keys.push(3_000_000_u64 + world.creeps[c_idx].id);
                tf.translation = new_pos;
                if bullet.remaining_range <= 0.0 {
                    commands.entity(entity).despawn();
                } else {
                    world.bullets_snapshot.push(BulletSnapshot {
                        id: bullet.id,
                        translation: tf.translation,
                        velocity: bullet.velocity,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
            } else {
                commands.entity(entity).despawn();
            }
            continue;
        }

        if let Some((hit_entity, impact_pos, _)) = ray_hit {
            let mut target_destroyed = false;
            let mut impact_surface = if tree_query.contains(hit_entity) {
                ImpactSurface::Tree
            } else {
                ImpactSurface::StoneWall
            };
            let min_bldg_hit_dmg = match &bullet.vfx_source {
                BulletVfxSource::Hero { weapon_kind: WeaponKind::Shotgun, .. } => {
                    items_cfg.stat_limits.min_pellet_effective_damage
                }
                _ => items_cfg.stat_limits.min_weapon_damage,
            };
            if let Some(&tur_idx) = turret_idx_by_entity.get(&hit_entity) {
                impact_surface = ImpactSurface::Turret;
                if bullet.aoe_radius <= 0.0 {
                    let (_, destroyed) = apply_damage_to_turret(
                        &mut commands,
                        &config,
                        &items_cfg,
                        &mut world,
                        Some(hit_entity),
                        tur_idx,
                        bullet.owner_slot,
                        bullet.owner_team,
                        bullet.damage,
                        min_bldg_hit_dmg,
                    );
                    target_destroyed = destroyed;
                }
            } else if let Some(&bar_idx) = barracks_idx_by_entity.get(&hit_entity) {
                impact_surface = ImpactSurface::Building;
                if bullet.aoe_radius <= 0.0 {
                    let (_, destroyed) = apply_damage_to_barracks(
                        &mut commands,
                        &config,
                        &items_cfg,
                        &mut world,
                        Some(hit_entity),
                        bar_idx,
                        bullet.owner_slot,
                        bullet.owner_team,
                        bullet.damage,
                        min_bldg_hit_dmg,
                    );
                    target_destroyed = destroyed;
                }
            } else if let Some(&anc_idx) = ancient_idx_by_entity.get(&hit_entity) {
                impact_surface = ImpactSurface::Building;
                if bullet.aoe_radius <= 0.0 {
                    let (_, destroyed) = apply_damage_to_ancient(
                        &mut commands,
                        &config,
                        &items_cfg,
                        &mut world,
                        Some(hit_entity),
                        anc_idx,
                        bullet.owner_team,
                        bullet.damage,
                        min_bldg_hit_dmg,
                    );
                    target_destroyed = destroyed;
                }
            } else if let Some(&t_idx) = target_idx_by_entity.get(&hit_entity) {
                let t_key = 4_000_000_u64 + t_idx as u64;
                if bullet.piercing && bullet.hit_entity_keys.contains(&t_key) {
                    tf.translation = new_pos;
                    if bullet.remaining_range <= 0.0 {
                        commands.entity(entity).despawn();
                    } else {
                        world.bullets_snapshot.push(BulletSnapshot {
                            id: bullet.id,
                            translation: tf.translation,
                            velocity: bullet.velocity,
                            vfx_source: bullet.vfx_source.clone(),
                        });
                    }
                    continue;
                }
                if bullet.piercing {
                    bullet.hit_entity_keys.push(t_key);
                }
                impact_surface = ImpactSurface::Character;
                if bullet.aoe_radius <= 0.0 {
                let target = &mut world.targets[t_idx];
                target.hp -= bullet.damage;
                target.hit_flash_timer = config.fx.target_hit_flash_secs;
                if target.hp <= 0.0 && !target.is_dead {
                    target.is_dead = true;
                    target.respawn_remaining = target.respawn_duration;
                    target_destroyed = true;
                    let t_pos = target.base_pos;
                    commands.entity(hit_entity).insert(ColliderDisabled);
                    if let Some(owner_slot) = bullet.owner_slot {
                        let world_inner = &mut *world;
                        if let Some(attacker) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                            attacker.last_hits += 1;
                            attacker.hero_gold += 35;
                            if attacker.soldier_pos.distance(t_pos) <= attacker.hero_xp_gain_radius {
                                grant_xp_to_player_slot(attacker, 45, &config, &items_cfg, &mut world_inner.pending_private_fx);
                            }
                        }
                    } else {
                        world.last_hits += 1;
                        world.hero_gold += 35;
                        if world.soldier_pos.distance(t_pos) <= world.hero_xp_gain_radius {
                            world.hero_xp += 45;
                        }
                    }
                }
                }
            }

            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
                vfx_source: bullet.vfx_source.clone(),
            });
            if bullet.aoe_radius > 0.0 {
                aoe_bullet_explosions.push((
                    impact_pos,
                    bullet.owner_slot,
                    bullet.owner_team,
                    bullet.damage,
                    bullet.aoe_radius,
                    bullet.vfx_source.clone(),
                ));
                if bullet.toxic_pool_duration_secs > 0.0 {
                    let pool_id = world.next_entity_id;
                    world.next_entity_id += 1;
                    spawned_toxic_pools.push(ActiveToxicPool {
                        id: pool_id,
                        pos: impact_pos,
                        radius: bullet.aoe_radius,
                        remaining_secs: bullet.toxic_pool_duration_secs,
                        total_duration: bullet.toxic_pool_duration_secs,
                        dps: bullet.toxic_pool_dps,
                        slow_ratio: bullet.toxic_slow_ratio,
                        min_slow_mult: items_cfg.stat_limits.min_toxic_slow_multiplier,
                        vertical_tolerance: items_cfg.stat_limits.toxic_pool_vertical_tolerance,
                        owner_slot: bullet.owner_slot,
                        owner_team: bullet.owner_team,
                    });
                }
            } else {
                world.pending_fx.push(GameplayFxEvent::BulletImpact {
                    impact_pos,
                    bullet_dir: step_dir,
                    surface: impact_surface,
                    target_destroyed,
                    trauma_add: config.combat.target_destroy_trauma_add,
                    trauma_max: config.combat.target_destroy_trauma_max,
                    vfx_source: bullet.vfx_source.clone(),
                });
            }
            if bullet.piercing && target_idx_by_entity.contains_key(&hit_entity) && bullet.remaining_range > 0.0 {
                tf.translation = new_pos;
                world.bullets_snapshot.push(BulletSnapshot {
                    id: bullet.id,
                    translation: tf.translation,
                    velocity: bullet.velocity,
                    vfx_source: bullet.vfx_source.clone(),
                });
            } else {
                commands.entity(entity).despawn();
            }
        } else {
            // بررسی برخورد گلوله با سطح آب برکه‌ها و رودخانه‌ها
            let mid_pos = (old_pos + new_pos) * 0.5;
            if let Some((water_y, _)) = sample_water_surface_at(&config, mid_pos) {
                let crossed_water = (old_pos.y >= water_y - 0.15 && new_pos.y <= water_y + 0.15)
                    || (mid_pos.y - water_y).abs() <= 0.35;
                if crossed_water {
                    let wp = Vec3::new(mid_pos.x, water_y, mid_pos.z);
                    if bullet.aoe_radius > 0.0 {
                        world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                            start: old_pos,
                            end: wp,
                            vfx_source: bullet.vfx_source.clone(),
                        });
                        aoe_bullet_explosions.push((
                            wp,
                            bullet.owner_slot,
                            bullet.owner_team,
                            bullet.damage,
                            bullet.aoe_radius,
                            bullet.vfx_source.clone(),
                        ));
                        if bullet.toxic_pool_duration_secs > 0.0 {
                            let pool_id = world.next_entity_id;
                            world.next_entity_id += 1;
                            spawned_toxic_pools.push(ActiveToxicPool {
                                id: pool_id,
                                pos: wp,
                                radius: bullet.aoe_radius,
                                remaining_secs: bullet.toxic_pool_duration_secs,
                                total_duration: bullet.toxic_pool_duration_secs,
                                dps: bullet.toxic_pool_dps,
                                slow_ratio: bullet.toxic_slow_ratio,
                                min_slow_mult: items_cfg.stat_limits.min_toxic_slow_multiplier,
                                vertical_tolerance: items_cfg.stat_limits.toxic_pool_vertical_tolerance,
                                owner_slot: bullet.owner_slot,
                                owner_team: bullet.owner_team,
                            });
                        }
                        commands.entity(entity).despawn();
                        continue;
                    }
                    world.pending_fx.push(GameplayFxEvent::WaterSplash {
                        pos: wp,
                        intensity: 1.15,
                    });
                    world.pending_fx.push(GameplayFxEvent::BulletImpact {
                        impact_pos: wp,
                        bullet_dir: step_dir,
                        surface: ImpactSurface::Water,
                        target_destroyed: false,
                        trauma_add: 0.0,
                        trauma_max: config.combat.shot_fire_trauma_max,
                        vfx_source: bullet.vfx_source.clone(),
                    });
                }
            }

            tf.translation = new_pos;
            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: new_pos,
                vfx_source: bullet.vfx_source.clone(),
            });

            if bullet.remaining_range <= 0.0 {
                if bullet.aoe_radius > 0.0 {
                    aoe_bullet_explosions.push((
                        new_pos,
                        bullet.owner_slot,
                        bullet.owner_team,
                        bullet.damage,
                        bullet.aoe_radius,
                        bullet.vfx_source.clone(),
                    ));
                    if bullet.toxic_pool_duration_secs > 0.0 {
                        let pool_id = world.next_entity_id;
                        world.next_entity_id += 1;
                        spawned_toxic_pools.push(ActiveToxicPool {
                            id: pool_id,
                            pos: new_pos,
                            radius: bullet.aoe_radius,
                            remaining_secs: bullet.toxic_pool_duration_secs,
                            total_duration: bullet.toxic_pool_duration_secs,
                            dps: bullet.toxic_pool_dps,
                            slow_ratio: bullet.toxic_slow_ratio,
                            min_slow_mult: items_cfg.stat_limits.min_toxic_slow_multiplier,
                            vertical_tolerance: items_cfg.stat_limits.toxic_pool_vertical_tolerance,
                            owner_slot: bullet.owner_slot,
                            owner_team: bullet.owner_team,
                        });
                    }
                }
                commands.entity(entity).despawn();
            } else {
                world.bullets_snapshot.push(BulletSnapshot {
                    id: bullet.id,
                    translation: tf.translation,
                    velocity: bullet.velocity,
                    vfx_source: bullet.vfx_source.clone(),
                });
            }
        }
    }

    for (pos, owner_slot, owner_team, dmg, aoe_radius, vfx_source) in aoe_bullet_explosions {
        apply_aoe_explosion(
            &mut commands,
            &config,
            &items_cfg,
            &mut world,
            &target_entity_by_idx,
            &turret_entity_by_idx,
            &barracks_entity_by_idx,
            &ancient_entity_by_idx,
            pos,
            owner_slot,
            owner_team,
            dmg,
            aoe_radius,
            Some(vfx_source),
        );
    }


    world.electric_beams_snapshot.clear();
    let beam_requests: Vec<(usize, String, TeamId, Vec3, Vec3, f32, f32, f32, f32, f32, bool)> = if !world.players.is_empty() {
        world
            .players
            .iter()
            .filter_map(|p| {
                p.active_beam_request.map(|(mpos, bdir, mrange, bdps, bslow, barm, bdur, bflame)| {
                    (
                        p.slot_index,
                        config.active_hero(p.hero_index).id.clone(),
                        p.hero_team,
                        mpos,
                        bdir,
                        mrange,
                        bdps,
                        bslow,
                        barm,
                        bdur,
                        bflame,
                    )
                })
            })
            .collect()
    } else if let Some((mpos, bdir, mrange, bdps, bslow, barm, bdur, bflame)) = world.active_beam_request {
        vec![(
            world.active_slot,
            config.active_hero(world.hero_index).id.clone(),
            world.hero_team,
            mpos,
            bdir,
            mrange,
            bdps,
            bslow,
            barm,
            bdur,
            bflame,
        )]
    } else {
        Vec::new()
    };

    for (owner_slot, hero_id, owner_team, muzzle_pos, beam_dir, max_range, beam_dps, slow_ratio, armor_red, debuff_dur, is_flame) in beam_requests {
        let mut best_toi = max_range;
        let mut hit_hero: Option<usize> = None;
        let mut hit_creep: Option<usize> = None;
        let mut hit_courier: Option<usize> = None;
        let mut hit_world_ent: Option<Entity> = None;

        for (p_idx, p) in world.players.iter().enumerate() {
            if p.slot_index == owner_slot || p.hero_team == owner_team || p.is_dead {
                continue;
            }
            let h_rad = config.soldier.player_radius
                * p.hero_scale
                * config.combat.hero_hitbox_radius_multiplier
                + config.combat.bullet_hit_forgiveness_radius;
            let h_height = if p.effective_stance == Stance::Crouch {
                config.combat.hero_crouch_hitbox_height * p.hero_scale
            } else {
                config.combat.hero_stand_hitbox_height * p.hero_scale
            };
            if let Some(toi) = segment_hits_hero_capsule(
                muzzle_pos,
                beam_dir,
                best_toi,
                p.soldier_pos,
                h_rad,
                h_height,
                config.combat.bullet_hit_forgiveness_height_bottom,
                config.combat.bullet_hit_forgiveness_height_top,
            ) {
                if toi < best_toi {
                    best_toi = toi;
                    hit_hero = Some(p_idx);
                    hit_creep = None;
                    hit_courier = None;
                }
            }
        }

        for (p_idx, p) in world.players.iter().enumerate() {
            if p.hero_team == owner_team || p.courier_is_dead {
                continue;
            }
            let h_cfg = config.active_hero(p.hero_index);
            let c_rad = h_cfg.courier.hit_radius + config.combat.bullet_hit_forgiveness_radius;
            if let Some(toi) = segment_hits_hero_capsule(
                muzzle_pos,
                beam_dir,
                best_toi,
                p.courier_pos - Vec3::Y * c_rad,
                c_rad,
                c_rad * 2.0,
                0.2,
                0.2,
            ) {
                if toi < best_toi {
                    best_toi = toi;
                    hit_courier = Some(p_idx);
                    hit_hero = None;
                    hit_creep = None;
                }
            }
        }

        for (c_idx, c) in world.creeps.iter().enumerate() {
            if c.is_dead {
                continue;
            }
            let is_enemy = c.team != owner_team;
            let is_deniable = config.gameplay_rules.allow_beam_deny
                && c.team == owner_team
                && (c.hp / c.max_hp) <= config.creeps.deny_hp_threshold_ratio;
            if !is_enemy && !is_deniable {
                continue;
            }
            let c_stats = resolve_creep_stats(&config, c.creep_type_id.as_deref());
            let c_rad = c_stats.collider_radius + config.combat.bullet_hit_forgiveness_radius;
            if let Some(toi) = segment_hits_hero_capsule(
                muzzle_pos,
                beam_dir,
                best_toi,
                c.pos,
                c_rad,
                c_stats.hitbox_height,
                config.combat.bullet_hit_forgiveness_height_bottom,
                config.combat.bullet_hit_forgiveness_height_top,
            ) {
                if toi < best_toi {
                    best_toi = toi;
                    hit_creep = Some(c_idx);
                    hit_hero = None;
                    hit_courier = None;
                }
            }
        }

        if let Some((hit_ent, hit)) = physics_world.cast_ray(
            muzzle_pos,
            beam_dir,
            best_toi,
            bullet_query_groups,
            |ent| {
                if let Some(&t_idx) = target_idx_by_entity.get(&ent) {
                    !world.targets[t_idx].is_dead
                } else {
                    true
                }
            },
        ) {
            if hit.toi < best_toi {
                best_toi = hit.toi;
                hit_world_ent = Some(hit_ent);
                hit_hero = None;
                hit_creep = None;
                hit_courier = None;
            }
        }

        if let Some((water_toi, _)) = raycast_water_surface(&config, muzzle_pos, beam_dir, best_toi) {
            if water_toi < best_toi {
                best_toi = water_toi;
                hit_world_ent = None;
                hit_hero = None;
                hit_creep = None;
                hit_courier = None;
            }
        }

        let end_pos = muzzle_pos + beam_dir * best_toi;
        let tick_dmg = beam_dps * dt;
        let mut hit_any_target = false;

        if let Some(enemy_idx) = hit_hero {
            hit_any_target = true;
            if is_flame {
                world.players[enemy_idx].flame_armor_debuff_remaining = debuff_dur;
                world.players[enemy_idx].flame_armor_debuff_amount = armor_red;
                if world.players[enemy_idx].slot_index == world.active_slot {
                    world.flame_armor_debuff_remaining = debuff_dur;
                    world.flame_armor_debuff_amount = armor_red;
                }
            } else {
                world.players[enemy_idx].electric_fire_rate_slow_remaining = debuff_dur;
                world.players[enemy_idx].electric_fire_rate_slow_ratio = slow_ratio;
                if world.players[enemy_idx].slot_index == world.active_slot {
                    world.electric_fire_rate_slow_remaining = debuff_dur;
                    world.electric_fire_rate_slow_ratio = slow_ratio;
                }
            }
            apply_combat_damage_to_hero(
                &mut world,
                enemy_idx,
                Some(owner_slot),
                None,
                None,
                tick_dmg,
                0.0,
                false,
                true,
                false,
                true,
                end_pos,
                &config,
                &items_cfg,
            );
        } else if let Some(cour_idx) = hit_courier {
            hit_any_target = true;
            let h_cfg = config.active_hero(world.players[cour_idx].hero_index).clone();
            let cpos_check = world.players[cour_idx].courier_pos;
            if !config.gameplay_rules.courier_invulnerable_in_fountain
                || fountain_regen_bonus_for_pos(&config, world.players[cour_idx].hero_team, cpos_check) <= 0.0
            {
                world.players[cour_idx].courier_hp = (world.players[cour_idx].courier_hp - tick_dmg).max(0.0);
            }
            if world.players[cour_idx].courier_hp <= 0.0 && !world.players[cour_idx].courier_is_dead {
                world.players[cour_idx].courier_is_dead = true;
                world.players[cour_idx].courier_respawn_remaining = h_cfg.courier.respawn_secs;
                world.players[cour_idx].courier_order = CourierOrder::Idle;
                let cpos = world.players[cour_idx].courier_pos;
                let w_inner = &mut *world;
                if let Some(killer) = w_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                    killer.last_hits += 1;
                    killer.hero_gold = killer.hero_gold.saturating_add(h_cfg.courier.bounty_gold);
                    w_inner.pending_private_fx.push((owner_slot, GameplayFxEvent::LastHitGold { pos: cpos, gold: h_cfg.courier.bounty_gold }));
                }
            }
        } else if let Some(c_idx) = hit_creep {
            hit_any_target = true;
            let c_stats = resolve_creep_stats(&config, world.creeps[c_idx].creep_type_id.as_deref());
            let (_, _, armor_bonus, _, _) = lane_creep_buffs(&config, &world.barracks, world.creeps[c_idx].team, world.creeps[c_idx].lane, world.creeps[c_idx].camp_index.is_none());
            let (_, time_armor_bonus, _, _) = lane_creep_time_scaling(&config, world.match_elapsed_secs, world.creeps[c_idx].camp_index.is_none());
            let is_enemy_c = owner_team != world.creeps[c_idx].team;
            let c_hit_pos = world.creeps[c_idx].pos;
            let (eff_dmg, _) = apply_creep_combat_effects(
                &mut world,
                Some(owner_slot),
                is_enemy_c,
                tick_dmg,
                dota2_armor_multiplier(c_stats.armor + time_armor_bonus + armor_bonus),
                0.0,
                true,
                c_hit_pos,
                &items_cfg,
            );
            world.creeps[c_idx].hp = (world.creeps[c_idx].hp - eff_dmg).max(0.0);
            aggro_creep_and_camp_on_hit(&mut world, c_idx, Some(owner_slot), &config);
            if world.creeps[c_idx].hp <= 0.0 && !world.creeps[c_idx].is_dead {
                world.creeps[c_idx].is_dead = true;
                world.creeps[c_idx].anim_state = CreepAnimState::Die;
                world.creeps[c_idx].despawn_timer = c_stats.die_despawn_delay_secs;
                let cpos = world.creeps[c_idx].pos;
                let creep_team = world.creeps[c_idx].team;
                let gb = lane_creep_gold_bounty(&config, world.match_elapsed_secs, world.creeps[c_idx].camp_index.is_none(), c_stats.gold_bounty);
                let xb = c_stats.xp_bounty;
                let is_deny = owner_team == creep_team;

                if is_deny {
                    if let Some(denier) = world.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                        denier.denies += 1;
                    }
                    world.pending_fx.push(GameplayFxEvent::CreepDenied { pos: cpos });
                    let total_denied_xp =
                        ((xb as f32) * config.creeps.deny_enemy_xp_multiplier).round() as u32;
                    let eligible_indices: Vec<usize> = world
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            !p.is_dead
                                && p.hero_team != creep_team
                                && p.soldier_pos.distance(cpos) <= p.hero_xp_gain_radius
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if !eligible_indices.is_empty() && total_denied_xp > 0 {
                        let share = total_denied_xp / (eligible_indices.len() as u32);
                        let w_inner = &mut *world;
                        for idx in eligible_indices {
                            grant_xp_to_player_slot(
                                &mut w_inner.players[idx],
                                share,
                                &config,
                                &items_cfg,
                                &mut w_inner.pending_private_fx,
                            );
                        }
                    }
                } else {
                    let w_inner = &mut *world;
                    if let Some(killer) = w_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                        killer.last_hits += 1;
                        killer.hero_gold += gb;
                        w_inner.pending_private_fx.push((owner_slot, GameplayFxEvent::LastHitGold { pos: cpos, gold: gb }));
                    }
                    let eligible: Vec<usize> = world
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            !p.is_dead
                                && p.hero_team == owner_team
                                && p.soldier_pos.distance(cpos) <= p.hero_xp_gain_radius
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if !eligible.is_empty() {
                        let share = xb / (eligible.len() as u32);
                        let w_inner = &mut *world;
                        for idx in eligible {
                            grant_xp_to_player_slot(
                                &mut w_inner.players[idx],
                                share,
                                &config,
                                &items_cfg,
                                &mut w_inner.pending_private_fx,
                            );
                        }
                    }
                }
            }
        } else if let Some(hit_ent) = hit_world_ent {
            if let Some(&t_idx) = target_idx_by_entity.get(&hit_ent) {
                hit_any_target = true;
                world.targets[t_idx].hp -= tick_dmg;
                world.targets[t_idx].hit_flash_timer = config.fx.target_hit_flash_secs;
                if world.targets[t_idx].hp <= 0.0 && !world.targets[t_idx].is_dead {
                    world.targets[t_idx].hp = 0.0;
                    world.targets[t_idx].is_dead = true;
                    world.targets[t_idx].respawn_remaining = world.targets[t_idx].respawn_duration;
                    let tpos = world.targets[t_idx].base_pos;
                    commands.entity(hit_ent).insert(ColliderDisabled);
                    let w_inner = &mut *world;
                    if let Some(attacker) = w_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                        attacker.last_hits += 1;
                        attacker.hero_gold += 35;
                        if attacker.soldier_pos.distance(tpos) <= attacker.hero_xp_gain_radius {
                            grant_xp_to_player_slot(attacker, 45, &config, &items_cfg, &mut w_inner.pending_private_fx);
                        }
                    }
                }
            } else if let Some(&tur_idx) = turret_idx_by_entity.get(&hit_ent) {
                let (damaged, _) = apply_damage_to_turret(
                    &mut commands,
                    &config,
                    &items_cfg,
                    &mut world,
                    Some(hit_ent),
                    tur_idx,
                    Some(owner_slot),
                    owner_team,
                    tick_dmg,
                    0.0,
                );
                if damaged {
                    hit_any_target = true;
                }
            } else if let Some(&bar_idx) = barracks_idx_by_entity.get(&hit_ent) {
                let (damaged, _) = apply_damage_to_barracks(
                    &mut commands,
                    &config,
                    &items_cfg,
                    &mut world,
                    Some(hit_ent),
                    bar_idx,
                    Some(owner_slot),
                    owner_team,
                    tick_dmg,
                    0.0,
                );
                if damaged {
                    hit_any_target = true;
                }
            } else if let Some(&anc_idx) = ancient_idx_by_entity.get(&hit_ent) {
                let (damaged, _) = apply_damage_to_ancient(
                    &mut commands,
                    &config,
                    &items_cfg,
                    &mut world,
                    Some(hit_ent),
                    anc_idx,
                    owner_team,
                    tick_dmg,
                    0.0,
                );
                if damaged {
                    hit_any_target = true;
                }
            }
        }

        world.electric_beams_snapshot.push(ElectricBeamSnapshot {
            id: owner_slot as u64,
            hero_id,
            start_pos: muzzle_pos,
            end_pos,
            hit_target: hit_any_target,
            hit_any: hit_any_target || hit_world_ent.is_some(),
            is_flame,
        });
    }

    world.toxic_pools.extend(spawned_toxic_pools);
    world.toxic_slow_multiplier = 1.0;
    for p in &mut world.players {
        p.toxic_slow_multiplier = 1.0;
    }

    let mut active_pools = std::mem::take(&mut world.toxic_pools);
    for pool in &mut active_pools {
        pool.remaining_secs -= dt;
        if pool.remaining_secs <= 0.0 {
            continue;
        }
        let tick_dmg = pool.dps * dt;
        let slow_mult = (1.0 - pool.slow_ratio).clamp(pool.min_slow_mult, 1.0);

        for idx in 0..world.targets.len() {
            if world.targets[idx].is_dead {
                continue;
            }
            let t_pos = world.targets[idx].base_pos;
            let flat_d = Vec2::new(t_pos.x - pool.pos.x, t_pos.z - pool.pos.z).length();
            if flat_d <= pool.radius && (t_pos.y - pool.pos.y).abs() <= pool.vertical_tolerance {
                world.targets[idx].hp -= tick_dmg;
                if world.targets[idx].hp <= 0.0 {
                    world.targets[idx].hp = 0.0;
                    world.targets[idx].is_dead = true;
                    world.targets[idx].respawn_remaining = world.targets[idx].respawn_duration;
                    if let Some(&t_ent) = target_entity_by_idx.get(&idx) {
                        commands.entity(t_ent).insert(ColliderDisabled);
                    }
                    if let Some(o_slot) = pool.owner_slot {
                        let w_inner = &mut *world;
                        if let Some(attacker) = w_inner.players.iter_mut().find(|p| p.slot_index == o_slot) {
                            attacker.last_hits += 1;
                            attacker.hero_gold += 35;
                            if attacker.soldier_pos.distance(t_pos) <= attacker.hero_xp_gain_radius {
                                grant_xp_to_player_slot(attacker, 45, &config, &items_cfg, &mut w_inner.pending_private_fx);
                            }
                        }
                    }
                }
            }
        }

        for p_idx in 0..world.players.len() {
            if world.players[p_idx].is_dead || world.players[p_idx].hero_team == pool.owner_team {
                continue;
            }
            let h_pos = world.players[p_idx].soldier_pos;
            let flat_d = Vec2::new(h_pos.x - pool.pos.x, h_pos.z - pool.pos.z).length();
            if flat_d <= pool.radius && (h_pos.y - pool.pos.y).abs() <= pool.vertical_tolerance {
                world.players[p_idx].toxic_slow_multiplier =
                    world.players[p_idx].toxic_slow_multiplier.min(slow_mult);
                if world.players[p_idx].slot_index == world.active_slot {
                    world.toxic_slow_multiplier = world.toxic_slow_multiplier.min(slow_mult);
                }
                apply_combat_damage_to_hero(
                    &mut world,
                    p_idx,
                    pool.owner_slot,
                    None,
                    None,
                    tick_dmg,
                    0.0,
                    false,
                    true,
                    false,
                    false,
                    h_pos + Vec3::Y * 0.9,
                    &config,
                    &items_cfg,
                );
            }
        }

        for c_idx in 0..world.creeps.len() {
            if world.creeps[c_idx].is_dead || world.creeps[c_idx].team == pool.owner_team {
                continue;
            }
            let c_pos = world.creeps[c_idx].pos;
            let flat_d = Vec2::new(c_pos.x - pool.pos.x, c_pos.z - pool.pos.z).length();
            if flat_d <= pool.radius && (c_pos.y - pool.pos.y).abs() <= pool.vertical_tolerance {
                let c_stats = resolve_creep_stats(&config, world.creeps[c_idx].creep_type_id.as_deref());
                let (_, _, armor_bonus, _, _) = lane_creep_buffs(
                    &config,
                    &world.barracks,
                    world.creeps[c_idx].team,
                    world.creeps[c_idx].lane,
                    world.creeps[c_idx].camp_index.is_none(),
                );
                let (_, time_armor_bonus, _, _) = lane_creep_time_scaling(
                    &config,
                    world.match_elapsed_secs,
                    world.creeps[c_idx].camp_index.is_none(),
                );
                let (eff_dmg, _) = apply_creep_combat_effects(
                    &mut world,
                    pool.owner_slot,
                    true,
                    tick_dmg,
                    dota2_armor_multiplier(c_stats.armor + time_armor_bonus + armor_bonus),
                    0.0,
                    true,
                    c_pos,
                    &items_cfg,
                );
                world.creeps[c_idx].hp = (world.creeps[c_idx].hp - eff_dmg).max(0.0);
                aggro_creep_and_camp_on_hit(&mut world, c_idx, pool.owner_slot, &config);
                if world.creeps[c_idx].hp <= 0.0 && !world.creeps[c_idx].is_dead {
                    world.creeps[c_idx].is_dead = true;
                    world.creeps[c_idx].anim_state = CreepAnimState::Die;
                    world.creeps[c_idx].despawn_timer = c_stats.die_despawn_delay_secs;
                    let gold_bounty = lane_creep_gold_bounty(&config, world.match_elapsed_secs, world.creeps[c_idx].camp_index.is_none(), c_stats.gold_bounty);
                    let xp_bounty = c_stats.xp_bounty;
                    if let Some(o_slot) = pool.owner_slot {
                        if let Some(killer) = world.players.iter_mut().find(|p| p.slot_index == o_slot) {
                            killer.last_hits += 1;
                            killer.hero_gold += gold_bounty;
                            world.pending_private_fx.push((
                                o_slot,
                                GameplayFxEvent::LastHitGold {
                                    pos: c_pos,
                                    gold: gold_bounty,
                                },
                            ));
                        }
                    }
                    let eligible_indices: Vec<usize> = world
                        .players
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            !p.is_dead
                                && p.hero_team == pool.owner_team
                                && (p.soldier_pos.distance(c_pos) <= p.hero_xp_gain_radius
                                    || (config.gameplay_rules.last_hitter_always_gets_xp
                                        && pool.owner_slot == Some(p.slot_index)))
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if !eligible_indices.is_empty() {
                        let share = xp_bounty / (eligible_indices.len() as u32);
                        let w_inner = &mut *world;
                        for idx in eligible_indices {
                            let p = &mut w_inner.players[idx];
                            grant_xp_to_player_slot(p, share, &config, &items_cfg, &mut w_inner.pending_private_fx);
                        }
                    }
                }
            }
        }
    }
    active_pools.retain(|p| p.remaining_secs > 0.0);
    world.toxic_pools_snapshot = active_pools
        .iter()
        .map(|p| ToxicPoolSnapshot {
            id: p.id,
            pos: p.pos,
            radius: p.radius,
            remaining_secs: p.remaining_secs,
            total_duration: p.total_duration,
        })
        .collect();
    world.toxic_pools = active_pools;

    for (idx, target) in world.targets.iter_mut().enumerate() {
        if target.is_dead {
            target.respawn_remaining -= dt;
            if target.respawn_remaining <= 0.0 {
                target.is_dead = false;
                target.hp = target.max_hp;
                target.hit_flash_timer = 0.0;
                if let Some(&t_ent) = target_entity_by_idx.get(&idx) {
                    commands.entity(t_ent).remove::<ColliderDisabled>();
                }
            }
        } else if target.hit_flash_timer > 0.0 {
            target.hit_flash_timer = (target.hit_flash_timer - dt).max(0.0);
        }
    }

    let dead_flags: Vec<bool> = world.targets.iter().map(|t| t.is_dead).collect();
    let laser_hero_targets: Vec<(usize, TeamId, Vec3, f32, Stance, bool, bool, bool)> = world
        .players
        .iter()
        .map(|p| {
            let hidden_invis = p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret;
            let water_vis_dawn = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dawn,
                &config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            let water_vis_dusk = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dusk,
                &config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            (
                p.slot_index,
                p.hero_team,
                p.soldier_pos,
                p.hero_scale,
                p.effective_stance,
                p.is_dead || hidden_invis,
                water_vis_dawn,
                water_vis_dusk,
            )
        })
        .collect();
    let all_hero_vision: Vec<(usize, TeamId, Vec3, Stance, f32, f32, f32)> = world
        .players
        .iter()
        .filter(|p| !p.is_dead)
        .map(|p| {
            let h = config.active_hero(p.hero_index);
            (
                p.slot_index,
                p.hero_team,
                p.soldier_pos,
                p.effective_stance,
                p.hero_scale,
                config.hero_effective_vision(h, world.match_elapsed_secs, world.current_weather),
                h.high_ground_vision_bonus,
            )
        })
        .collect();
    let attack_revealed_enemies: Vec<(TeamId, Vec3, Stance, f32, bool, bool)> = world
        .players
        .iter()
        .filter(|p| {
            !p.is_dead
                && !(p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret)
                && (p.attack_reveal_to_dawn_remaining > 0.0 || p.attack_reveal_to_dusk_remaining > 0.0)
        })
        .map(|p| {
            (
                p.hero_team,
                p.soldier_pos,
                p.effective_stance,
                p.hero_scale,
                p.attack_reveal_to_dawn_remaining > 0.0,
                p.attack_reveal_to_dusk_remaining > 0.0,
            )
        })
        .collect();

    let num_slots = world.players.len().max(1);
    let has_slots = !world.players.is_empty();

    for slot_idx in 0..num_slots {
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
        if let Some(input) = world.active_input.clone() {
            let muzzle_pos = resolve_muzzle_pos(&world, &input, &config, &physics_world);
            let raw_target = world.aim_hit_pos;
            let world_laser_hit = compute_laser_hit(
                &physics_world,
                muzzle_pos,
                raw_target,
                |hit_ent| {
                    if let Some(&t_idx) = target_idx_by_entity.get(&hit_ent) {
                        !dead_flags[t_idx]
                    } else {
                        true
                    }
                },
            );
            let mut final_laser_hit = world_laser_hit;
            let to_laser = world_laser_hit - muzzle_pos;
            let max_laser_dist = to_laser.length();
            if max_laser_dist > f32::EPSILON {
                let laser_dir = to_laser / max_laser_dist;
                let mut best_toi = max_laser_dist;
                for &(e_slot, e_team, e_pos, e_scale, e_stance, e_dead, e_water_vis_dawn, e_water_vis_dusk) in &laser_hero_targets {
                    let water_visible = match world.hero_team {
                        TeamId::Dawn => e_water_vis_dawn,
                        TeamId::Dusk => e_water_vis_dusk,
                        TeamId::Neutral => false,
                    };
                    if e_slot == world.active_slot
                        || e_team == world.hero_team
                        || e_dead
                        || !water_visible
                        || (!world.demo_reveal_full_map
                            && !(world.is_slot_los_visible_to_team(world.hero_team, e_slot)
                                && world.fog_of_war.is_world_entity_visible(
                                    &config,
                                    e_pos,
                                    SharedFogOfWar::hero_visible_height(&config, e_stance, e_scale),
                                )))
                    {
                        continue;
                    }
                    let h_rad = config.soldier.player_radius
                        * e_scale
                        * config.combat.hero_hitbox_radius_multiplier;
                    let h_height = if e_stance == Stance::Crouch {
                        config.combat.hero_crouch_hitbox_height * e_scale
                    } else {
                        config.combat.hero_stand_hitbox_height * e_scale
                    };
                    if let Some(toi) = segment_hits_hero_capsule(
                        muzzle_pos,
                        laser_dir,
                        best_toi,
                        e_pos,
                        h_rad,
                        h_height,
                        config.combat.bullet_hit_forgiveness_height_bottom,
                        config.combat.bullet_hit_forgiveness_height_top,
                    ) {
                        if toi < best_toi {
                            best_toi = toi;
                            final_laser_hit = muzzle_pos + laser_dir * toi;
                        }
                    }
                }
                let c_rad = config.creeps.collider_radius;
                let c_height = config.creeps.hitbox_height;
                for c in &world.creeps {
                    if c.is_dead {
                        continue;
                    }
                    let is_enemy = c.team != world.hero_team;
                    let is_deniable = c.team == world.hero_team
                        && (c.hp / c.max_hp) <= config.creeps.deny_hp_threshold_ratio;
                    if !is_enemy && !is_deniable {
                        continue;
                    }
                    let c_vis_h = resolve_creep_stats(&config, c.creep_type_id.as_deref()).hitbox_height;
                    if is_enemy
                        && !world.demo_reveal_full_map
                        && !(world.is_creep_los_visible_to_team(world.hero_team, c.id)
                            && world.fog_of_war.is_world_entity_visible(&config, c.pos, c_vis_h))
                    {
                        continue;
                    }
                    if let Some(toi) = segment_hits_hero_capsule(
                        muzzle_pos,
                        laser_dir,
                        best_toi,
                        c.pos,
                        c_rad,
                        c_height,
                        config.combat.bullet_hit_forgiveness_height_bottom,
                        config.combat.bullet_hit_forgiveness_height_top,
                    ) {
                        if toi < best_toi {
                            best_toi = toi;
                            final_laser_hit = muzzle_pos + laser_dir * toi;
                        }
                    }
                }
            }
            world.laser_hit_pos = final_laser_hit;
        }
        if let Some(hero) = config.heroes.get(world.hero_index) {
            let hero_pos = world.soldier_pos;
            let my_team = world.hero_team;
            let allied_creep_positions: Vec<Vec3> = world
                .creeps
                .iter()
                .filter(|c| !c.is_dead && c.team == my_team)
                .map(|c| c.pos)
                .collect();
            let allied_courier_vision: Vec<(Vec3, f32)> = world
                .players
                .iter()
                .filter(|p| !p.courier_is_dead && p.hero_team == my_team)
                .map(|p| (p.courier_pos, config.active_hero(p.hero_index).courier.vision_range))
                .collect();
            let elapsed_secs = world.match_elapsed_secs;
            let courier_mult = config.effective_vision_multiplier(elapsed_secs, config.day_night.night_courier_vision_multiplier);
            let cur_weather = world.current_weather;
            let hero_alive = !world.is_dead;
            world.hero_vision_range = if hero_alive {
                config.hero_effective_vision(hero, elapsed_secs, cur_weather)
            } else {
                0.0
            };
            let mut live_allied_structures: Vec<(Vec3, f32)> = Vec::new();
            for ftn in &config.map.fountains {
                if ftn.team == my_team {
                    live_allied_structures.push((
                        ftn.model_transform.location_vec(),
                        config.map.fow_fountain_vision_range,
                    ));
                }
            }
            for a in &world.ancients {
                if !a.is_dead && a.team == my_team {
                    live_allied_structures.push((a.pos, config.map.fow_ancient_vision_range));
                }
            }
            for t in &world.turrets {
                if !t.is_dead && t.team == my_team {
                    if let Some(td) = config.map.turrets.get(t.index) {
                        live_allied_structures.push((t.pos, td.vision_range));
                    }
                }
            }
            for b in &world.barracks {
                if !b.is_dead && b.team == my_team {
                    live_allied_structures.push((b.pos, config.map.fow_barracks_vision_range));
                }
            }
            let hero_stance = world.effective_stance;
            world.fog_of_war.update_for_hero_with_stance(
                &config,
                hero_pos,
                hero_stance,
                hero,
                hero_alive,
                &live_allied_structures,
                &allied_creep_positions,
                elapsed_secs,
                cur_weather,
            );
            for (c_pos, c_range) in allied_courier_vision {
                world.fog_of_war.reveal_from_source_with_eye(
                    &config,
                    c_pos,
                    0.0,
                    c_range * courier_mult,
                    72,
                );
            }
            for &(ally_slot, ally_team, ally_pos, ally_stance, ally_scale, ally_base_range, ally_hg_bonus) in &all_hero_vision {
                if ally_team == my_team && ally_slot != world.active_slot {
                    let (ally_ground_y, ally_under_bridge) =
                        world.fog_of_war.effective_observer_y(&config, ally_pos);
                    let ally_range = ally_base_range
                        + if !ally_under_bridge
                            && ally_ground_y >= config.map.high_ground_occlusion_min_diff
                        {
                            ally_hg_bonus
                        } else {
                            0.0
                        };
                    let ally_eye = SharedFogOfWar::hero_eye_height(&config, ally_stance, ally_scale);
                    world.fog_of_war.reveal_from_source_with_eye(
                        &config,
                        ally_pos,
                        ally_eye,
                        ally_range,
                        160,
                    );
                }
            }
            for &(enemy_team, enemy_pos, enemy_stance, enemy_scale, rev_dawn, rev_dusk) in
                &attack_revealed_enemies
            {
                if enemy_team == my_team {
                    continue;
                }
                let revealed_to_us = match my_team {
                    TeamId::Dawn => rev_dawn,
                    TeamId::Dusk => rev_dusk,
                    TeamId::Neutral => false,
                };
                if revealed_to_us {
                    let e_eye = SharedFogOfWar::hero_eye_height(&config, enemy_stance, enemy_scale);
                    world.fog_of_war.reveal_from_source_with_eye(
                        &config,
                        enemy_pos,
                        e_eye,
                        config.combat.attack_reveal_vision_radius,
                        48,
                    );
                }
            }
        }
        if let Some(ref mut active_input) = world.active_input {
            active_input.consume_just_pressed();
        }
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
    }

    recompute_team_3d_los_visibility(&config, &physics_world, &mut world);

    for idx in 0..world.targets.len() {
        let t_pos = world.targets[idx].base_pos;
        if world.players.is_empty() {
            world.targets_visible[idx] =
                world.demo_reveal_full_map || world.fog_of_war.is_world_pos_visible(&config, t_pos);
        } else {
            world.targets_visible[idx] = true;
        }
    }

    if let Some(ref mut active_input) = world.active_input {
        active_input.consume_just_pressed();
    }
}

/// محاسبه دقیق و یکپارچه خط دید فیزیکی سه‌بعدی برای هر دو تیم (هیروها، سربازان، تاورها، سازه‌ها و کوریرها)
/// تا قانون میدان دید برای همه موجودیت‌ها کاملاً یکسان و بدون نشت سلولی پشت سنگرهای کم‌ارتفاع باشد
fn recompute_team_3d_los_visibility(
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    world: &mut AuthoritativeWorldState,
) {
    let p_len = world.players.len();
    world.dawn_los_visible_slots.clear();
    world.dawn_los_visible_slots.resize(p_len, false);
    world.dusk_los_visible_slots.clear();
    world.dusk_los_visible_slots.resize(p_len, false);
    world.dawn_los_visible_couriers.clear();
    world.dawn_los_visible_couriers.resize(p_len, false);
    world.dusk_los_visible_couriers.clear();
    world.dusk_los_visible_couriers.resize(p_len, false);
    world.dawn_los_visible_creeps.clear();
    world.dusk_los_visible_creeps.clear();

    if p_len == 0 {
        return;
    }

    let elapsed = world.match_elapsed_secs;
    let weather = world.current_weather;
    let struct_mult = config.effective_vision_multiplier(elapsed, config.day_night.night_turret_vision_multiplier);
    let creep_mult = config.effective_vision_multiplier(elapsed, config.day_night.night_creep_vision_multiplier);
    let courier_mult = config.effective_vision_multiplier(elapsed, config.day_night.night_courier_vision_multiplier);
    let struct_eye = config.map.fow_stand_eye_height;

    // (feet_pos, eye_height, base_range, high_ground_bonus, clear_radius)
    let mut dawn_obs: Vec<(Vec3, f32, f32, f32, f32)> = Vec::with_capacity(32);
    let mut dusk_obs: Vec<(Vec3, f32, f32, f32, f32)> = Vec::with_capacity(32);

    for p in &world.players {
        let h = config.active_hero(p.hero_index);
        if !p.is_dead {
            let eye = SharedFogOfWar::hero_eye_height(config, p.effective_stance, p.hero_scale);
            let rng = config.hero_effective_vision(h, elapsed, weather);
            let item = (p.soldier_pos, eye, rng, h.high_ground_vision_bonus, 0.0);
            match p.hero_team {
                TeamId::Dawn => dawn_obs.push(item),
                TeamId::Dusk => dusk_obs.push(item),
                TeamId::Neutral => {}
            }
        }
        if !p.courier_is_dead {
            let c_rng = h.courier.vision_range * courier_mult;
            let item = (p.courier_pos, 0.0, c_rng, 0.0, 0.0);
            match p.hero_team {
                TeamId::Dawn => dawn_obs.push(item),
                TeamId::Dusk => dusk_obs.push(item),
                TeamId::Neutral => {}
            }
        }
    }

    for c in &world.creeps {
        if c.is_dead || c.team == TeamId::Neutral {
            continue;
        }
        let c_stats = resolve_creep_stats(config, c.creep_type_id.as_deref());
        let item = (
            c.pos,
            c_stats.hitbox_height * 0.85,
            c_stats.vision_range * creep_mult,
            0.0,
            0.0,
        );
        match c.team {
            TeamId::Dawn => dawn_obs.push(item),
            TeamId::Dusk => dusk_obs.push(item),
            TeamId::Neutral => {}
        }
    }

    for ftn in &config.map.fountains {
        let pos = ftn.model_transform.location_vec();
        let item = (
            pos,
            struct_eye,
            config.map.fow_fountain_vision_range * struct_mult,
            0.0,
            ftn.outer_radius + 0.45,
        );
        match ftn.team {
            TeamId::Dawn => dawn_obs.push(item),
            TeamId::Dusk => dusk_obs.push(item),
            TeamId::Neutral => {}
        }
    }
    for (i, a) in world.ancients.iter().enumerate() {
        if a.is_dead {
            continue;
        }
        let ad = &config.map.ancients[i];
        let s = ad.model_transform.scale_vec();
        let rad = (ad.base_size.0 * s.x).max(ad.base_size.2 * s.z) * 0.5 + 0.6;
        let item = (
            a.pos,
            struct_eye,
            config.map.fow_ancient_vision_range * struct_mult,
            0.0,
            rad,
        );
        match a.team {
            TeamId::Dawn => dawn_obs.push(item),
            TeamId::Dusk => dusk_obs.push(item),
            TeamId::Neutral => {}
        }
    }
    for t in &world.turrets {
        if t.is_dead {
            continue;
        }
        let Some(td) = config.map.turrets.get(t.index) else {
            continue;
        };
        let s = td.model_transform.scale_vec();
        let rad = td.base_radius * s.x.max(s.z) + 0.45;
        let ground_y = sample_terrain_top_y(config, t.pos.x, t.pos.z).max(t.pos.y);
        let item = (
            Vec3::new(t.pos.x, ground_y, t.pos.z),
            struct_eye,
            td.vision_range * struct_mult,
            0.0,
            rad,
        );
        match t.team {
            TeamId::Dawn => dawn_obs.push(item),
            TeamId::Dusk => dusk_obs.push(item),
            TeamId::Neutral => {}
        }
    }
    for (i, b) in world.barracks.iter().enumerate() {
        if b.is_dead {
            continue;
        }
        let bd = &config.map.barracks[i];
        let s = bd.model_transform.scale_vec();
        let rad = (bd.size.0 * s.x).max(bd.size.2 * s.z) * 0.5 + 0.6;
        let item = (
            b.pos,
            struct_eye,
            config.map.fow_barracks_vision_range * struct_mult,
            0.0,
            rad,
        );
        match b.team {
            TeamId::Dawn => dawn_obs.push(item),
            TeamId::Dusk => dusk_obs.push(item),
            TeamId::Neutral => {}
        }
    }

    let can_any_observer_see = |observers: &[(Vec3, f32, f32, f32, f32)],
                                target_pos: Vec3,
                                target_vis_h: f32|
     -> bool {
        for &(obs_pos, obs_eye, base_rng, hg_bonus, clear_r) in observers {
            let to_xz = Vec2::new(target_pos.x - obs_pos.x, target_pos.z - obs_pos.z);
            let max_r = base_rng + hg_bonus + clear_r;
            if to_xz.length_squared() > max_r * max_r {
                continue;
            }
            let start_pos = if clear_r > 0.0 {
                let d = to_xz.length();
                if d > clear_r {
                    let dir = to_xz / d;
                    Vec3::new(obs_pos.x + dir.x * clear_r, obs_pos.y, obs_pos.z + dir.y * clear_r)
                } else {
                    obs_pos
                }
            } else {
                obs_pos
            };
            if can_observer_see_target_3d(
                physics_world,
                config,
                start_pos,
                obs_eye,
                target_pos,
                target_vis_h,
                base_rng,
                hg_bonus,
            ) {
                return true;
            }
        }
        false
    };

    for p in &world.players {
        let slot = p.slot_index;
        let target_vis_h =
            SharedFogOfWar::hero_visible_height(config, p.effective_stance, p.hero_scale);
        world.dawn_los_visible_slots[slot] = p.hero_team == TeamId::Dawn
            || (!p.is_dead && p.attack_reveal_to_dawn_remaining > 0.0)
            || can_any_observer_see(&dawn_obs, p.soldier_pos, target_vis_h);
        world.dusk_los_visible_slots[slot] = p.hero_team == TeamId::Dusk
            || (!p.is_dead && p.attack_reveal_to_dusk_remaining > 0.0)
            || can_any_observer_see(&dusk_obs, p.soldier_pos, target_vis_h);

        world.dawn_los_visible_couriers[slot] = p.hero_team == TeamId::Dawn
            || (!p.courier_is_dead && can_any_observer_see(&dawn_obs, p.courier_pos, 0.0));
        world.dusk_los_visible_couriers[slot] = p.hero_team == TeamId::Dusk
            || (!p.courier_is_dead && can_any_observer_see(&dusk_obs, p.courier_pos, 0.0));
    }

    for c in &world.creeps {
        let c_vis_h = resolve_creep_stats(config, c.creep_type_id.as_deref()).hitbox_height;
        if c.team == TeamId::Dawn || can_any_observer_see(&dawn_obs, c.pos, c_vis_h) {
            world.dawn_los_visible_creeps.insert(c.id);
        }
        if c.team == TeamId::Dusk || can_any_observer_see(&dusk_obs, c.pos, c_vis_h) {
            world.dusk_los_visible_creeps.insert(c.id);
        }
    }
}

pub fn shared_client_drive_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    prediction: Option<Res<ClientPhysicsPredictionState>>,
    mut soldier_query: Query<(&Transform, &mut CustomCharacterController), With<PhysicsSoldier>>,
) {
    let Some(prediction) = prediction else { return };
    let Ok((soldier_tf, mut controller)) = soldier_query.single_mut() else {
        return;
    };

    if prediction.match_ended || prediction.is_paused || prediction.is_dead || prediction.traversal_active {
        controller.translation = None;
        return;
    }

    let dt = time.delta_secs();
    let grav_y = config.physics.character_gravity_step_y * dt;

    let hero_cfg = config.active_hero(prediction.hero_index);
    let feet_pos = Vec3::new(
        soldier_tf.translation.x,
        soldier_tf.translation.y - config.physics.soldier_capsule_center_y * hero_cfg.hero_scale,
        soldier_tf.translation.z,
    );
    let water_sample = sample_water_surface_at(&config, feet_pos);
    let hero_move_mult = prediction.latest_move_speed_multiplier.unwrap_or(hero_cfg.move_speed_multiplier);
    let shallow_mult = hero_cfg.shallow_water_speed_multiplier;
    let deep_mult = hero_cfg.deep_water_speed_multiplier;
    let speed_mult = if let Some((water_y, _)) = water_sample {
        let depth = (water_y - feet_pos.y).max(0.0);
        if depth >= config.physics.deep_water_swim_depth {
            hero_move_mult * deep_mult
        } else {
            hero_move_mult * shallow_mult
        }
    } else {
        hero_move_mult
    };
    let flow_drift = water_sample
        .map(|(_, flow)| Vec3::new(flow.x, 0.0, flow.y) * 0.35 * dt)
        .unwrap_or(Vec3::ZERO);

    let Some(ref input) = prediction.latest_input else {
        controller.translation = Some(Vec3::new(flow_drift.x, grav_y, flow_drift.z));
        return;
    };

    let flat_input = Vec3::new(input.move_input.x, 0.0, input.move_input.z);
    if flat_input != Vec3::ZERO {
        let speed = match prediction.effective_stance {
            Stance::Run => config.soldier.run_speed,
            Stance::Walk => config.soldier.walk_speed,
            Stance::Crouch => config.soldier.crouch_speed,
        } * speed_mult;
        let delta = flat_input.normalize() * speed * dt + flow_drift;
        controller.translation = Some(Vec3::new(delta.x, grav_y, delta.z));
    } else {
        controller.translation = Some(Vec3::new(flow_drift.x, grav_y, flow_drift.z));
    }
}

fn shared_client_react_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    mut shell_query: Query<(Entity, &mut PhysicsShellCasing)>,
    mut particle_query: Query<(Entity, &mut Transform, &mut PhysicsParticle)>,
) {
    for (entity, mut shell) in &mut shell_query {
        shell.lifetime.tick(time.delta());
        if shell.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }

    for (entity, mut tf, mut particle) in &mut particle_query {
        particle.lifetime.tick(time.delta());
        if particle.scale_decay {
            let rem = 1.0 - particle.lifetime.fraction();
            tf.scale = Vec3::splat(rem.max(0.05));
        }
        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}


struct ResolvedCreepStats<'a> {
    collider_radius: f32,
    hitbox_height: f32,
    aim_chest_height: f32,
    muzzle_offset_y: f32,
    muzzle_forward: f32,
    move_speed: f32,
    fire_rate_secs: f32,
    vision_range: f32,
    attack_range: f32,
    bullet_damage: f32,
    projectile_speed: f32,
    bullet_max_range: f32,
    armor: f32,
    gold_bounty: u32,
    xp_bounty: u32,
    die_despawn_delay_secs: f32,
    _phantom: std::marker::PhantomData<&'a ()>,
}

fn resolve_creep_stats<'a>(config: &'a GameConfig, creep_type_id: Option<&str>) -> ResolvedCreepStats<'a> {
    if let Some(tid) = creep_type_id {
        let ct = config.neutral_camps.creep_types.get(tid).unwrap_or_else(|| {
            panic!("نوع سرباز کمپ '{}' در config.ron یافت نشد!", tid)
        });
        ResolvedCreepStats {
            collider_radius: ct.collider_radius,
            hitbox_height: ct.hitbox_height,
            aim_chest_height: ct.aim_chest_height,
            muzzle_offset_y: ct.muzzle_offset_y,
            muzzle_forward: ct.muzzle_forward,
            move_speed: ct.move_speed,
            fire_rate_secs: ct.fire_rate_secs,
            vision_range: ct.vision_range,
            attack_range: ct.attack_range,
            bullet_damage: ct.bullet_damage,
            projectile_speed: ct.projectile_speed,
            bullet_max_range: ct.bullet_max_range,
            armor: ct.armor,
            gold_bounty: ct.gold_bounty,
            xp_bounty: ct.xp_bounty,
            die_despawn_delay_secs: ct.die_despawn_delay_secs,
            _phantom: std::marker::PhantomData,
        }
    } else {
        let c = &config.creeps;
        ResolvedCreepStats {
            collider_radius: c.collider_radius,
            hitbox_height: c.hitbox_height,
            aim_chest_height: c.aim_chest_height,
            muzzle_offset_y: c.muzzle_offset_y,
            muzzle_forward: c.muzzle_forward,
            move_speed: c.move_speed,
            fire_rate_secs: c.fire_rate_secs,
            vision_range: c.vision_range,
            attack_range: c.attack_range,
            bullet_damage: c.bullet_damage,
            projectile_speed: c.projectile_speed,
            bullet_max_range: c.bullet_max_range,
            armor: c.armor,
            gold_bounty: c.gold_bounty,
            xp_bounty: c.xp_bounty,
            die_despawn_delay_secs: c.die_despawn_delay_secs,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// محاسبه ارتقای زمان‌محور سربازان لاین (هر ۱۰ دقیقه تعداد، هر ۷ دقیقه ویژگی‌ها)
/// بازگشت: (hp_bonus, armor_bonus, damage_bonus, fire_rate_mult)
pub fn lane_creep_gold_bounty(
    config: &GameConfig,
    match_elapsed_secs: f32,
    is_lane_creep: bool,
    base_gold: u32,
) -> u32 {
    if !is_lane_creep {
        return base_gold;
    }
    let steps = (match_elapsed_secs.max(0.0) / config.creeps.stat_upgrade_interval_secs).floor() as u32;
    base_gold + steps.saturating_mul(config.creeps.gold_upgrade_per_step)
}

pub fn lane_creep_time_scaling(
    config: &GameConfig,
    match_elapsed_secs: f32,
    is_lane_creep: bool,
) -> (f32, f32, f32, f32) {
    if !is_lane_creep {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let steps = (match_elapsed_secs.max(0.0) / config.creeps.stat_upgrade_interval_secs).floor() as i32;
    if steps <= 0 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let s = steps as f32;
    let hp_bonus = s * config.creeps.hp_upgrade_per_step;
    let armor_bonus = s * config.creeps.armor_upgrade_per_step;
    let damage_bonus = s * config.creeps.damage_upgrade_per_step;
    let fire_rate_mult = config.creeps.fire_rate_multiplier_per_step.powi(steps);
    (hp_bonus, armor_bonus, damage_bonus, fire_rate_mult)
}

/// محاسبه تعداد سربازان هر موج در لاین بر اساس زمان بازی
pub fn lane_creep_wave_count(config: &GameConfig, match_elapsed_secs: f32) -> usize {
    let steps = (match_elapsed_secs.max(0.0) / config.creeps.count_upgrade_interval_secs).floor() as usize;
    (config.creeps.creeps_per_wave + steps * config.creeps.count_upgrade_amount)
        .min(config.creeps.max_creeps_per_wave)
}

/// محاسبه باف‌های سربازان یک لاین بر اساس باراکس‌های نابودشده حریف
pub fn lane_creep_buffs(
    config: &GameConfig,
    barracks: &[BarracksState],
    creep_team: TeamId,
    lane: LaneId,
    is_lane_creep: bool,
) -> (f32, f32, f32, f32, f32) {
    if !is_lane_creep || creep_team == TeamId::Neutral {
        return (1.0, 1.0, 0.0, 1.0, 1.0);
    }
    let melee_dead = barracks.iter().any(|b| {
        b.is_dead && b.team != creep_team && b.lane == lane && b.kind == BarracksKind::Melee
    });
    let ranged_dead = barracks.iter().any(|b| {
        b.is_dead && b.team != creep_team && b.lane == lane && b.kind == BarracksKind::Ranged
    });
    let bb = &config.map.barracks_buffs;
    let hp_mult = if melee_dead { bb.melee_destroyed_hp_multiplier } else { 1.0 };
    let scale_mult = if melee_dead { bb.melee_destroyed_scale_multiplier } else { 1.0 };
    let armor_bonus = if melee_dead { bb.melee_destroyed_armor_bonus } else { 0.0 };
    let dmg_mult = if ranged_dead { bb.ranged_destroyed_damage_multiplier } else { 1.0 };
    let fire_rate_mult = if ranged_dead { bb.ranged_destroyed_fire_rate_multiplier } else { 1.0 };
    (hp_mult, scale_mult, armor_bonus, dmg_mult, fire_rate_mult)
}

/// بررسی دقیق قرارگیری سرباز روی سطح ایمن (فلات بیرون از حاشیه دره، یا داخل مستطیل دقیق پل/لاین با حاشیه امن)
/// بدون تخصیص حافظه (O(1)) تا هیچ سربازی از لبه پل یا پرتگاه به رودخانه سقوط نکند.
#[inline]
fn is_creep_safe_ground_xz(config: &GameConfig, x: f32, z: f32, margin: f32) -> bool {
    if !config.map.canyon.enabled {
        return true;
    }
    // ۱. اگر با حاشیه امن بیرون از لبه پرتگاه دره باشد، روی فلات امن است
    if canyon_edge_distance(&config.map.canyon, x, z) <= -margin {
        return true;
    }
    // ۲. بررسی قرارگیری ایمن روی پل مرکزی
    let cb = &config.map.center_bridge;
    let cb_tf = cb.model_transform.to_transform();
    let cb_rad = (cb.outer_radius * cb_tf.scale.x.max(cb_tf.scale.z) - margin).max(0.5);
    let dx = x - cb_tf.translation.x;
    let dz = z - cb_tf.translation.z;
    if dx * dx + dz * dz <= cb_rad * cb_rad {
        return true;
    }
    // ۳. بررسی قرارگیری ایمن داخل مستطیل دقیق (OBB) هر سگمنت لاین + مفاصل داخلی بین سگمنت‌ها
    let p_xz = Vec2::new(x, z);
    for lane in &config.map.lanes {
        let half_w = (lane.width * 0.5 - margin).max(0.5);
        let half_w_sq = half_w * half_w;
        let wp_count = lane.waypoints.len();
        for seg_idx in 0..wp_count.saturating_sub(1) {
            let a_xz = Vec2::new(lane.waypoints[seg_idx].0, lane.waypoints[seg_idx].2);
            let b_xz = Vec2::new(lane.waypoints[seg_idx + 1].0, lane.waypoints[seg_idx + 1].2);
            let ab = b_xz - a_xz;
            let len = ab.length();
            if len > 1e-4 {
                let u = ab / len;
                let rel = p_xz - a_xz;
                let proj = rel.dot(u);
                if proj >= 0.0 && proj <= len {
                    let perp_sq = (rel - u * proj).length_squared();
                    if perp_sq <= half_w_sq {
                        return true;
                    }
                }
            }
        }
        // پوشش نرم مفاصل داخلی تغییر زاویه لاین
        for wp_idx in 1..wp_count.saturating_sub(1) {
            let w_xz = Vec2::new(lane.waypoints[wp_idx].0, lane.waypoints[wp_idx].2);
            if p_xz.distance_squared(w_xz) <= half_w_sq {
                return true;
            }
        }
    }
    false
}

/// بررسی سریع (۶ نمونه بدون تخصیص حافظه) برای تشخیص عبور خط مستقیم بین دو نقطه از روی رودخانه/پرتگاه
#[inline]
fn segment_crosses_river_barrier(
    config: &GameConfig,
    from_xz: Vec2,
    to_xz: Vec2,
    margin: f32,
) -> bool {
    if !config.map.canyon.enabled {
        return false;
    }
    const SAMPLES: usize = 6;
    for i in 1..SAMPLES {
        let t = (i as f32) / (SAMPLES as f32);
        let p = from_xz.lerp(to_xz, t);
        if !is_creep_safe_ground_xz(config, p.x, p.y, margin) {
            return true;
        }
    }
    false
}

/// یافتن نزدیک‌ترین نقطه و سگمنت روی لاین برای بازیابی هوشمند مسیر (مشابه Dota 2)
#[inline]
fn nearest_point_on_lane_xz(lane: &LanePathDef, p_xz: Vec2) -> (Vec2, usize, f32) {
    let mut best_pt = p_xz;
    let mut best_seg = 0usize;
    let mut best_dist_sq = f32::MAX;
    for seg_idx in 0..lane.waypoints.len().saturating_sub(1) {
        let a_xz = Vec2::new(lane.waypoints[seg_idx].0, lane.waypoints[seg_idx].2);
        let b_xz = Vec2::new(lane.waypoints[seg_idx + 1].0, lane.waypoints[seg_idx + 1].2);
        let ab = b_xz - a_xz;
        let len_sq = ab.length_squared();
        let t = if len_sq > 1e-5 {
            ((p_xz - a_xz).dot(ab) / len_sq).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let closest = a_xz + ab * t;
        let dsq = p_xz.distance_squared(closest);
        if dsq < best_dist_sq {
            best_dist_sq = dsq;
            best_pt = closest;
            best_seg = seg_idx;
        }
    }
    (best_pt, best_seg, best_dist_sq.sqrt())
}

/// استخراج پارامترهای جعبه چرخیده (OBB) یک مانع برای مسیریابی و جلوگیری از عبور سربازان از روی موانع
#[inline]
fn creep_obstacle_obb_2d(obs: &ObstacleDef) -> Option<(Vec2, Vec2, Quat, Quat, f32, f32)> {
    if obs.size.0 <= 1e-4 || obs.size.1 <= 1e-4 || obs.size.2 <= 1e-4 {
        return None;
    }
    let loc = obs.model_transform.location;
    let scl = obs.model_transform.scale;
    let hx = obs.size.0 * 0.5 * scl.0.abs();
    let hy = obs.size.1 * 0.5 * scl.1.abs();
    let hz = obs.size.2 * 0.5 * scl.2.abs();
    if hx <= 1e-4 || hy <= 1e-4 || hz <= 1e-4 {
        return None;
    }
    let rot_y = obs.model_transform.rotation.1.to_radians();
    let rot = Quat::from_rotation_y(rot_y);
    let inv_rot = Quat::from_rotation_y(-rot_y);
    Some((
        Vec2::new(loc.0, loc.2),
        Vec2::new(hx, hz),
        rot,
        inv_rot,
        loc.1 - hy,
        loc.1 + hy,
    ))
}

/// بررسی برخورد نقطه (x, y, z) سرباز با هر یک از موانع نقشه (با در نظر گرفتن ارتفاع و شعاع بدنه سرباز)
#[inline]
fn is_creep_blocked_by_obstacle(
    config: &GameConfig,
    x: f32,
    y: f32,
    z: f32,
    hitbox_height: f32,
    clearance_radius: f32,
) -> bool {
    let r_sq = clearance_radius * clearance_radius;
    for obs in &config.obstacles {
        let Some((center, half, _rot, inv_rot, min_y, max_y)) = creep_obstacle_obb_2d(obs) else {
            continue;
        };
        if y + hitbox_height < min_y || y > max_y + 0.25 {
            continue;
        }
        let local = inv_rot * Vec3::new(x - center.x, 0.0, z - center.y);
        let cx = local.x.clamp(-half.x, half.x);
        let cz = local.z.clamp(-half.y, half.y);
        let dx = local.x - cx;
        let dz = local.z - cz;
        if dx * dx + dz * dz < r_sq {
            return true;
        }
    }
    false
}

/// بررسی ایمن‌بودن نقطه برای حرکت سرباز (هم بیرون از پرتگاه رودخانه و هم بیرون از تمام موانع مسیر)
#[inline]
fn is_creep_walkable_xz(
    config: &GameConfig,
    x: f32,
    y: f32,
    z: f32,
    hitbox_height: f32,
    river_margin: f32,
    obs_radius: f32,
) -> bool {
    is_creep_safe_ground_xz(config, x, z, river_margin)
        && !is_creep_blocked_by_obstacle(config, x, y, z, hitbox_height, obs_radius)
}

/// هدایت پیش‌نگرانه (Corner Steering) برای دور زدن نرم موانع روی مسیر لاین بدون گیر کردن یا نوسان چپ‌و‌راست
fn steer_dir_around_obstacles(
    config: &GameConfig,
    creep_id: u64,
    cur_pos: Vec3,
    desired_dir: Vec3,
    goal_xz_opt: Option<Vec2>,
    hitbox_height: f32,
    river_margin: f32,
    obs_radius: f32,
) -> Vec3 {
    let dir2 = Vec2::new(desired_dir.x, desired_dir.z).normalize_or_zero();
    if dir2.length_squared() < 1e-6 {
        return desired_dir;
    }

    let lookahead = config.creeps.obstacle_avoid_lookahead_dist;
    let cur_xz = Vec2::new(cur_pos.x, cur_pos.z);
    let target_xz = goal_xz_opt.unwrap_or(cur_xz + dir2 * (lookahead * 2.0));

    // یافتن نزدیک‌ترین مانعی که مسیر پیش‌رو را در فاصله lookahead مسدود کرده است
    let mut blocking_obs: Option<(f32, Vec2, Vec2, Quat, Quat)> = None;
    const SAMPLES: usize = 6;
    for obs in &config.obstacles {
        let Some((center, half, rot, inv_rot, min_y, max_y)) = creep_obstacle_obb_2d(obs) else {
            continue;
        };
        if cur_pos.y + hitbox_height < min_y || cur_pos.y > max_y + 0.25 {
            continue;
        }
        // غربالگری سریع فاصله افقی
        let max_r = half.length() + obs_radius + lookahead;
        if cur_xz.distance_squared(center) > max_r * max_r {
            continue;
        }
        let r_sq = obs_radius * obs_radius;
        for s in 1..=SAMPLES {
            let d = lookahead * (s as f32 / SAMPLES as f32);
            let p = cur_xz + dir2 * d;
            let local = inv_rot * Vec3::new(p.x - center.x, 0.0, p.y - center.y);
            let cx = local.x.clamp(-half.x, half.x);
            let cz = local.z.clamp(-half.y, half.y);
            let dx = local.x - cx;
            let dz = local.z - cz;
            if dx * dx + dz * dz < r_sq {
                if blocking_obs.map_or(true, |(best_d, ..)| d < best_d) {
                    blocking_obs = Some((d, center, half, rot, inv_rot));
                }
                break;
            }
        }
    }

    let Some((hit_dist, center, half, rot, _inv_rot)) = blocking_obs else {
        return desired_dir;
    };

    let corner_pad = obs_radius + 0.45;
    let local_corners = [
        Vec3::new(half.x + corner_pad, 0.0, half.y + corner_pad),
        Vec3::new(half.x + corner_pad, 0.0, -(half.y + corner_pad)),
        Vec3::new(-(half.x + corner_pad), 0.0, half.y + corner_pad),
        Vec3::new(-(half.x + corner_pad), 0.0, -(half.y + corner_pad)),
    ];

    let tie_sign = if (creep_id & 1) == 0 { 1.0_f32 } else { -1.0_f32 };
    let perp_dir = Vec2::new(-dir2.y, dir2.x);
    let mut best_corner: Option<(Vec2, f32)> = None;

    for lc in local_corners {
        let wc3 = Vec3::new(center.x, 0.0, center.y) + rot * lc;
        let wc = Vec2::new(wc3.x, wc3.z);
        if !is_creep_walkable_xz(
            config,
            wc.x,
            cur_pos.y,
            wc.y,
            hitbox_height,
            river_margin,
            obs_radius * 0.9,
        ) {
            continue;
        }
        let to_c = wc - cur_xz;
        let dist_to_c = to_c.length();
        if dist_to_c < 0.25 {
            continue;
        }
        // جلوگیری از انتخاب گوشه‌ای که کاملاً پشت سر سرباز است
        if (to_c / dist_to_c).dot(dir2) < -0.35 {
            continue;
        }
        let side_bias = if perp_dir.dot(to_c / dist_to_c) * tie_sign >= 0.0 {
            -0.25
        } else {
            0.25
        };
        let score = dist_to_c + wc.distance(target_xz) + side_bias;
        if best_corner.map_or(true, |(_, bs)| score < bs) {
            best_corner = Some((wc, score));
        }
    }

    if let Some((corner, _)) = best_corner {
        let to_corner = (corner - cur_xz).normalize_or_zero();
        if to_corner.length_squared() > 1e-6 {
            let urgency = (1.0 - (hit_dist / lookahead).clamp(0.0, 1.0)) * 0.65 + 0.35;
            let blended = (dir2 + to_corner * (config.creeps.obstacle_avoid_steer_strength * urgency))
                .normalize_or_zero();
            if blended.length_squared() > 1e-6 {
                return Vec3::new(blended.x, 0.0, blended.y);
            }
        }
    }

    desired_dir
}

/// برطرف‌کردن قطعی هرگونه نفوذ سرباز به داخل OBB موانع پس از حرکت یا هل‌دادن فیزیکی
fn resolve_creep_obstacle_penetration(
    config: &GameConfig,
    cur_pos: Vec3,
    hitbox_height: f32,
    river_margin: f32,
    obs_radius: f32,
) -> (f32, f32) {
    let mut px = cur_pos.x;
    let mut pz = cur_pos.z;
    let r_sq = obs_radius * obs_radius;

    for obs in &config.obstacles {
        let Some((center, half, rot, inv_rot, min_y, max_y)) = creep_obstacle_obb_2d(obs) else {
            continue;
        };
        if cur_pos.y + hitbox_height < min_y || cur_pos.y > max_y + 0.25 {
            continue;
        }
        let local = inv_rot * Vec3::new(px - center.x, 0.0, pz - center.y);
        let cx = local.x.clamp(-half.x, half.x);
        let cz = local.z.clamp(-half.y, half.y);
        let dx = local.x - cx;
        let dz = local.z - cz;
        let d_sq = dx * dx + dz * dz;
        if d_sq < r_sq {
            let new_local = if d_sq > 1e-6 {
                let d = d_sq.sqrt();
                Vec3::new(cx + (dx / d) * obs_radius, 0.0, cz + (dz / d) * obs_radius)
            } else {
                // مرکز سرباز داخل جعبه است: خروج از نزدیک‌ترین وجه
                let pen_x = half.x - local.x.abs();
                let pen_z = half.y - local.z.abs();
                if pen_x < pen_z {
                    let s = if local.x >= 0.0 { 1.0 } else { -1.0 };
                    Vec3::new(s * (half.x + obs_radius), 0.0, local.z)
                } else {
                    let s = if local.z >= 0.0 { 1.0 } else { -1.0 };
                    Vec3::new(local.x, 0.0, s * (half.y + obs_radius))
                }
            };
            let world_out = Vec3::new(center.x, 0.0, center.y) + rot * new_local;
            if is_creep_safe_ground_xz(config, world_out.x, world_out.z, river_margin) {
                px = world_out.x;
                pz = world_out.z;
            }
        }
    }
    (px, pz)
}

/// هدایت هوشمند و سبک (Zero-Allocation) برای لغزش روان روی لبه‌های پرتگاه و دور زدن موانع مسیر بدون گیر کردن
#[inline]
fn steer_creep_step_avoiding_river(
    config: &GameConfig,
    cur_x: f32,
    cur_y: f32,
    cur_z: f32,
    desired_dir: Vec3,
    step_dist: f32,
    hitbox_height: f32,
    margin: f32,
    obs_radius: f32,
) -> (f32, f32, Vec3, bool) {
    let dir2 = Vec2::new(desired_dir.x, desired_dir.z).normalize_or_zero();
    if dir2.length_squared() < 1e-6 || step_dist <= 1e-6 {
        return (cur_x, cur_z, desired_dir, false);
    }

    let lookahead = config.creeps.cliff_lookahead_dist.max(step_dist * 2.0);
    let direct_x = cur_x + dir2.x * step_dist;
    let direct_z = cur_z + dir2.y * step_dist;
    let direct_step_ok =
        is_creep_walkable_xz(config, direct_x, cur_y, direct_z, hitbox_height, margin, obs_radius);
    let direct_look_ok = is_creep_walkable_xz(
        config,
        cur_x + dir2.x * lookahead,
        cur_y,
        cur_z + dir2.y * lookahead,
        hitbox_height,
        margin,
        obs_radius,
    );

    // مسیر سریع: در تیک‌هایی که مانع یا پرتگاهی جلو نیست تنها با ۲ چک ساده برمی‌گردد
    if direct_step_ok && direct_look_ok {
        return (direct_x, direct_z, Vec3::new(dir2.x, 0.0, dir2.y), true);
    }

    // جستجوی زاویه‌ای سبک برای لغزش نرم در امتداد لبه پرتگاه، پل یا مانع (تا ±۱۳۵ درجه)
    const ANGLES_DEG: [f32; 14] = [
        18.0, -18.0, 36.0, -36.0, 55.0, -55.0, 75.0, -75.0, 95.0, -95.0, 115.0, -115.0, 135.0,
        -135.0,
    ];

    let mut best_score = if direct_step_ok { 0.35 } else { f32::NEG_INFINITY };
    let mut best_dir = dir2;
    let mut found = direct_step_ok;

    for &deg in &ANGLES_DEG {
        let rad = deg.to_radians();
        let (s, c) = rad.sin_cos();
        let cand = Vec2::new(dir2.x * c - dir2.y * s, dir2.x * s + dir2.y * c);
        let nx = cur_x + cand.x * step_dist;
        let nz = cur_z + cand.y * step_dist;
        if !is_creep_walkable_xz(config, nx, cur_y, nz, hitbox_height, margin, obs_radius) {
            continue;
        }
        let look_ok = is_creep_walkable_xz(
            config,
            cur_x + cand.x * lookahead,
            cur_y,
            cur_z + cand.y * lookahead,
            hitbox_height,
            margin,
            obs_radius,
        );
        let half_look_ok = is_creep_walkable_xz(
            config,
            cur_x + cand.x * (lookahead * 0.5),
            cur_y,
            cur_z + cand.y * (lookahead * 0.5),
            hitbox_height,
            margin,
            obs_radius,
        );
        let align = cand.dot(dir2);
        let score = align + if look_ok { 0.65 } else if half_look_ok { 0.25 } else { -0.25 };
        if score > best_score {
            best_score = score;
            best_dir = cand;
            found = true;
        }
    }

    if found {
        let nx = cur_x + best_dir.x * step_dist;
        let nz = cur_z + best_dir.y * step_dist;
        (nx, nz, Vec3::new(best_dir.x, 0.0, best_dir.y), true)
    } else {
        (cur_x, cur_z, desired_dir, false)
    }
}

/// محافظ ساده برای هل‌دادن‌های فیزیکی (تاور و جداسازی سربازان) تا سرباز به داخل دره یا مانع پرتاب نشود
#[inline]
fn move_creep_xz_with_river_barrier(
    config: &GameConfig,
    cur_x: f32,
    cur_y: f32,
    cur_z: f32,
    next_x: f32,
    next_z: f32,
    hitbox_height: f32,
    margin: f32,
    obs_radius: f32,
) -> (f32, f32, bool) {
    if is_creep_walkable_xz(config, next_x, cur_y, next_z, hitbox_height, margin, obs_radius) {
        return (next_x, next_z, true);
    }
    let delta = Vec2::new(next_x - cur_x, next_z - cur_z);
    let dist = delta.length();
    if dist <= 1e-5 {
        return (cur_x, cur_z, false);
    }
    let (sx, sz, _, ok) = steer_creep_step_avoiding_river(
        config,
        cur_x,
        cur_y,
        cur_z,
        Vec3::new(delta.x / dist, 0.0, delta.y / dist),
        dist,
        hitbox_height,
        margin,
        obs_radius,
    );
    (sx, sz, ok)
}

fn step_lane_creeps_system(
    commands: &mut Commands,
    dt: f32,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    world: &mut AuthoritativeWorldState,
    target_query: &Query<&PhysicsTarget>,
    obstacle_query: &Query<(&PhysicsObstacle, &Transform), Without<PhysicsSoldier>>,
    elev_query: &Query<&ElevationSurface>,
) {
    world.match_elapsed_secs += dt;

    // ۱. تولید موج سربازان در هر ۳ لاین پس از دقیقه‌ی ۱ بازی و هر ۳۰ ثانیه
    if world.match_elapsed_secs >= world.next_wave_time_secs {
        world.next_wave_time_secs += config.creeps.wave_interval_secs;
        for lane_def in &config.map.lanes {
            if lane_def.waypoints.len() < 2 {
                continue;
            }
            for &team in &[TeamId::Dawn, TeamId::Dusk] {
                let (start_wp_idx, next_wp_idx) = match team {
                    TeamId::Dawn => (0, 1),
                    TeamId::Dusk => (lane_def.waypoints.len() - 1, lane_def.waypoints.len() - 2),
                    TeamId::Neutral => continue,
                };
                let wp0 = lane_def.waypoints[start_wp_idx];
                let wp1 = lane_def.waypoints[next_wp_idx];
                let p0 = Vec3::new(wp0.0, wp0.1, wp0.2);
                let p1 = Vec3::new(wp1.0, wp1.1, wp1.2);
                let dir = Vec3::new(p1.x - p0.x, 0.0, p1.z - p0.z).normalize_or_zero();
                let side = dir.cross(Vec3::Y).normalize_or_zero();

                let count = lane_creep_wave_count(config, world.match_elapsed_secs);
                let (time_hp_bonus, _, _, _) =
                    lane_creep_time_scaling(config, world.match_elapsed_secs, true);
                for i in 0..count {
                    let row = (i / 2) as f32;
                    let col = if i % 2 == 0 { -0.5 } else { 0.5 };
                    let spawn_pos = p0
                        - dir * (row * config.creeps.wave_spawn_spacing)
                        + side * (col * config.creeps.wave_spawn_spacing);
                    let id = world.next_creep_id;
                    world.next_creep_id += 1;
                    let (hp_mult, _, _, _, _) = lane_creep_buffs(
                        config,
                        &world.barracks,
                        team,
                        lane_def.lane,
                        true,
                    );
                    let buffed_hp = (config.creeps.max_hp + time_hp_bonus) * hp_mult;
                    world.creeps.push(LaneCreepState {
                        id,
                        team,
                        creep_type_id: None,
                        camp_index: None,
                        is_aggroed: true,
                        aggro_target_slot: None,
                        hero_aggro_target_slot: None,
                        hero_aggro_remaining_secs: 0.0,
                        hero_aggro_cooldown_secs: 0.0,
                        home_pos: spawn_pos,
                        lane: lane_def.lane,
                        waypoint_index: next_wp_idx,
                        pos: spawn_pos,
                        facing_dir: dir,
                        hp: buffed_hp,
                        max_hp: buffed_hp,
                        fire_cooldown: 0.0,
                        fire_anim_timer: 0.0,
                        anim_state: CreepAnimState::Run,
                        is_dead: false,
                        despawn_timer: 0.0,
                        hero_block_hesitation_timer: 0.0,
                        hero_block_cooldown_timer: 0.0,
                        hero_block_steer_side: 0.0,
                    });
                }
            }
        }
    }

    // ۱-ب. تولید و استک شدن سربازان ۳۰ کمپ جنگلی از دقیقه‌ی ۱ بازی و هر ۶۰ ثانیه (در صورت خالی بودن شعاع کمپ)
    if world.match_elapsed_secs >= world.next_camp_spawn_time_secs {
        world.next_camp_spawn_time_secs += config.neutral_camps.respawn_interval_secs;
        for (camp_idx, camp) in config.neutral_camps.camps.iter().enumerate() {
            let camp_center = Vec3::new(camp.center.0, camp.center.1, camp.center.2);
            let box_r = camp.spawn_box_radius;

            let hero_blocking = world
                .players
                .iter()
                .any(|p| !p.is_dead && Vec2::new(p.soldier_pos.x - camp_center.x, p.soldier_pos.z - camp_center.z).length() <= box_r);
            let creep_blocking = world
                .creeps
                .iter()
                .any(|c| !c.is_dead && Vec2::new(c.pos.x - camp_center.x, c.pos.z - camp_center.z).length() <= box_r);

            if hero_blocking || creep_blocking {
                continue;
            }

            let count = camp.creep_type_ids.len();
            let face_dir = if camp.team_side == TeamId::Dawn {
                Vec3::new(1.0, 0.0, 0.0)
            } else {
                Vec3::new(-1.0, 0.0, 0.0)
            };
            let side = face_dir.cross(Vec3::Y).normalize_or_zero();

            for (i, tid) in camp.creep_type_ids.iter().enumerate() {
                let ct = &config.neutral_camps.creep_types[tid];
                let offset_idx = (i as f32) - ((count.saturating_sub(1) as f32) * 0.5);
                let spawn_pos = camp_center + side * (offset_idx * config.neutral_camps.spawn_spacing);
                let id = world.next_creep_id;
                world.next_creep_id += 1;
                world.creeps.push(LaneCreepState {
                    id,
                    team: TeamId::Neutral,
                    creep_type_id: Some(tid.clone()),
                    camp_index: Some(camp_idx),
                    is_aggroed: false,
                    aggro_target_slot: None,
                    hero_aggro_target_slot: None,
                    hero_aggro_remaining_secs: 0.0,
                    hero_aggro_cooldown_secs: 0.0,
                    home_pos: spawn_pos,
                    lane: LaneId::Mid,
                    waypoint_index: 0,
                    pos: spawn_pos,
                    facing_dir: face_dir,
                    hp: ct.max_hp,
                    max_hp: ct.max_hp,
                    fire_cooldown: 0.0,
                    fire_anim_timer: 0.0,
                    anim_state: CreepAnimState::Idle,
                    is_dead: false,
                    despawn_timer: 0.0,
                    hero_block_hesitation_timer: 0.0,
                    hero_block_cooldown_timer: 0.0,
                    hero_block_steer_side: 0.0,
                });
            }
        }
    }

    // ۲. به‌روزرسانی تایمر مرگ و حذف سربازان مرده پس از اتمام انیمیشن die
    for c in &mut world.creeps {
        if c.is_dead {
            c.despawn_timer -= dt;
        }
    }
    world.creeps.retain(|c| !c.is_dead || c.despawn_timer > 0.0);

    // ۳. جمع‌آوری لیست اهداف بالقوه (سربازان زنده و هیروهای زنده)
    let creep_snap: Vec<(u64, TeamId, Vec3, bool, bool)> = world
        .creeps
        .iter()
        .map(|c| (c.id, c.team, c.pos, c.is_dead, c.is_aggroed))
        .collect();
    let hero_snap: Vec<(usize, TeamId, Vec3, Stance, f32, bool, bool, bool, bool, bool, bool)> = world
        .players
        .iter()
        .map(|p| {
            let hidden_invis = p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret;
            let water_vis_dawn = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dawn,
                config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            let water_vis_dusk = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dusk,
                config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            (
                p.slot_index,
                p.hero_team,
                p.soldier_pos,
                p.effective_stance,
                p.hero_scale,
                p.is_dead || hidden_invis,
                water_vis_dawn,
                water_vis_dusk,
                p.attack_reveal_to_dawn_remaining > 0.0,
                p.attack_reveal_to_dusk_remaining > 0.0,
                p.attack_reveal_to_neutral_remaining > 0.0,
            )
        })
        .collect();
    let hero_body_snap: Vec<(TeamId, Vec3, f32)> = world
        .players
        .iter()
        .filter(|p| !p.is_dead)
        .map(|p| (p.hero_team, p.soldier_pos, config.soldier.player_radius * p.hero_scale))
        .collect();
    let courier_snap: Vec<(TeamId, Vec3, bool)> = world
        .players
        .iter()
        .map(|p| (p.hero_team, p.courier_pos, p.courier_is_dead))
        .collect();
    let turret_snap: Vec<(TeamId, Vec3, f32, f32, bool)> = world
        .turrets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let td = &config.map.turrets[i];
            let s = td.model_transform.scale_vec();
            let world_r = td.base_radius * s.x.max(s.z);
            (t.team, t.pos, world_r, t.height, t.is_dead)
        })
        .collect();
    let barracks_snap: Vec<(TeamId, Vec3, f32, f32, bool, bool)> = world
        .barracks
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let bd = &config.map.barracks[i];
            let s = bd.model_transform.scale_vec();
            let half_r = (bd.size.0 * s.x).max(bd.size.2 * s.z) * 0.5;
            (b.team, b.pos, half_r, b.height, b.is_dead, b.is_invulnerable)
        })
        .collect();
    let ancient_snap: Vec<(TeamId, Vec3, f32, f32, bool, bool)> = world
        .ancients
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let ad = &config.map.ancients[i];
            let s = ad.model_transform.scale_vec();
            let half_r = (ad.base_size.0 * s.x).max(ad.base_size.2 * s.z) * 0.5;
            (a.team, a.pos, half_r, a.height, a.is_dead, a.is_invulnerable)
        })
        .collect();

    let query_groups = soldier_collision_groups();
    let c_len = world.creeps.len();

    for idx in 0..c_len {
        if world.creeps[idx].is_dead {
            world.creeps[idx].anim_state = CreepAnimState::Die;
            continue;
        }

        world.creeps[idx].fire_cooldown = (world.creeps[idx].fire_cooldown - dt).max(0.0);
        world.creeps[idx].fire_anim_timer = (world.creeps[idx].fire_anim_timer - dt).max(0.0);
        world.creeps[idx].hero_block_hesitation_timer =
            (world.creeps[idx].hero_block_hesitation_timer - dt).max(0.0);
        world.creeps[idx].hero_block_cooldown_timer =
            (world.creeps[idx].hero_block_cooldown_timer - dt).max(0.0);
        world.creeps[idx].hero_aggro_remaining_secs =
            (world.creeps[idx].hero_aggro_remaining_secs - dt).max(0.0);
        world.creeps[idx].hero_aggro_cooldown_secs =
            (world.creeps[idx].hero_aggro_cooldown_secs - dt).max(0.0);
        if world.creeps[idx].hero_aggro_remaining_secs <= 0.0 {
            world.creeps[idx].hero_aggro_target_slot = None;
        }

        let my_id = world.creeps[idx].id;
        let my_team = world.creeps[idx].team;
        let my_pos = world.creeps[idx].pos;
        let prev_safe_pos = my_pos;
        let my_home = world.creeps[idx].home_pos;
        let my_lane = world.creeps[idx].lane;
        let c_stats = resolve_creep_stats(config, world.creeps[idx].creep_type_id.as_deref());
        let safety_margin = c_stats.collider_radius + config.creeps.river_barrier_margin;
        let obs_radius = c_stats.collider_radius + config.creeps.obstacle_avoid_margin;
        let is_neutral = my_team == TeamId::Neutral;
        let dist_to_home = my_pos.distance(my_home);
        let leashed_back = is_neutral && dist_to_home > config.neutral_camps.aggro_leash_radius;
        if leashed_back {
            world.creeps[idx].is_aggroed = false;
            world.creeps[idx].aggro_target_slot = None;
        }
        if config.gameplay_rules.neutral_camps_regen_on_leash
            && is_neutral
            && !world.creeps[idx].is_aggroed
            && dist_to_home > 1.5
            && world.creeps[idx].hp < world.creeps[idx].max_hp
        {
            let regen = world.creeps[idx].max_hp * config.neutral_camps.leash_regen_pct_per_sec * dt;
            world.creeps[idx].hp = (world.creeps[idx].hp + regen).min(world.creeps[idx].max_hp);
        }
        let can_attack_now = !is_neutral || (world.creeps[idx].is_aggroed && !leashed_back);

        // الف) یافتن نزدیک‌ترین دشمن (سرباز یا هیرو) با رعایت کامل مه جنگ، برتری ارتفاع و خط دید موانع
        let eff_creep_vision = c_stats.vision_range
            * config.effective_vision_multiplier(
                world.match_elapsed_secs,
                config.day_night.night_creep_vision_multiplier,
            );
        // (primary_aim_pos, upper_aim_pos, dist, is_structure)
        let mut best_target: Option<(Vec3, Vec3, f32, bool)> = None;
        let mut hero_aggro_locked = false;

        // اولویت ۰ (قانون Creep Aggro مشابه Dota 2): اگر هیروی دشمن اخیراً به هیروی هم‌تیمی شلیک کرده باشد،
        // سربازهای لاین موقتاً اولویت حمله را روی آن هیرو می‌گذارند (تا زمان مرگ، نامرئی‌شدن، شیرجه در آب یا فرار خارج از شعاع تعقیب)
        if !is_neutral {
            if let Some(aggro_slot) = world.creeps[idx].hero_aggro_target_slot {
                let mut valid_aggro = false;
                for &(h_slot, h_team, h_pos, h_stance, h_scale, h_dead, h_water_vis_dawn, h_water_vis_dusk, _, _, _) in &hero_snap {
                    if h_slot != aggro_slot {
                        continue;
                    }
                    let water_visible = match my_team {
                        TeamId::Dawn => h_water_vis_dawn,
                        TeamId::Dusk => h_water_vis_dusk,
                        TeamId::Neutral => false,
                    };
                    if h_team == my_team || h_dead || !water_visible {
                        break;
                    }
                    let d = my_pos.distance(h_pos);
                    if d <= config.combat.creep_hero_aggro_max_chase_dist {
                        let h_vis_h = SharedFogOfWar::hero_visible_height(config, h_stance, h_scale);
                        let aim_y = if h_stance == Stance::Crouch {
                            config.combat.hero_crouch_aim_chest_height * h_scale
                        } else {
                            config.combat.hero_stand_aim_chest_height * h_scale
                        };
                        let upper_y = (h_vis_h - 0.10).max(aim_y);
                        best_target = Some((h_pos + Vec3::Y * aim_y, h_pos + Vec3::Y * upper_y, d, false));
                        hero_aggro_locked = true;
                        valid_aggro = true;
                    }
                    break;
                }
                if !valid_aggro {
                    world.creeps[idx].hero_aggro_target_slot = None;
                    world.creeps[idx].hero_aggro_remaining_secs = 0.0;
                }
            }
        }

        if can_attack_now && !hero_aggro_locked {
            let vision_sq = eff_creep_vision * eff_creep_vision;
            let creep_eye_h = c_stats.hitbox_height * 0.85;
            for &(other_id, other_team, other_pos, other_dead, other_aggroed) in &creep_snap {
                if other_id == my_id || other_team == my_team || other_dead {
                    continue;
                }
                // سرباز لاین به کمپ حمله نمی‌کند مگر اینکه کمپ در حالت تهاجمی (Aggro) باشد
                if !is_neutral && other_team == TeamId::Neutral && !other_aggroed {
                    continue;
                }
                if my_pos.distance_squared(other_pos) > vision_sq {
                    continue;
                }
                let d = my_pos.distance(other_pos);
                if d <= eff_creep_vision
                    && best_target.map_or(true, |(_, _, bd, _)| d < bd)
                    && can_observer_see_target_3d(
                        physics_world,
                        config,
                        my_pos,
                        creep_eye_h,
                        other_pos,
                        c_stats.hitbox_height,
                        eff_creep_vision,
                        0.0,
                    )
                {
                    let primary = other_pos + Vec3::Y * c_stats.aim_chest_height;
                    let upper = other_pos
                        + Vec3::Y * (c_stats.hitbox_height - 0.12).max(c_stats.aim_chest_height);
                    best_target = Some((primary, upper, d, false));
                }
            }
            let my_aggro_slot = world.creeps[idx].aggro_target_slot;
            for &(h_slot, h_team, h_pos, h_stance, h_scale, h_dead, h_water_vis_dawn, h_water_vis_dusk, h_rev_dawn, h_rev_dusk, h_rev_neutral) in &hero_snap {
                let water_visible = match my_team {
                    TeamId::Dawn => h_water_vis_dawn,
                    TeamId::Dusk => h_water_vis_dusk,
                    TeamId::Neutral => !is_pos_in_deep_water(config, h_pos),
                };
                if h_team == my_team || h_dead || !water_visible {
                    continue;
                }
                let d = my_pos.distance(h_pos);
                let h_vis_h = SharedFogOfWar::hero_visible_height(config, h_stance, h_scale);
                let attack_revealed = match my_team {
                    TeamId::Dawn => h_rev_dawn,
                    TeamId::Dusk => h_rev_dusk,
                    TeamId::Neutral => h_rev_neutral || my_aggro_slot == Some(h_slot),
                };
                // اگر هیرو به این تیم یا کمپ شلیک کرده باشد، مکان او لو می‌رود و کل کمپ/سربازان او را تعقیب می‌کنند
                let can_detect = if is_neutral && attack_revealed {
                    let max_chase = config.neutral_camps.aggro_leash_radius + eff_creep_vision;
                    d <= max_chase
                } else if attack_revealed && d <= eff_creep_vision * 1.5 {
                    true
                } else {
                    d <= eff_creep_vision
                        && can_observer_see_target_3d(
                            physics_world,
                            config,
                            my_pos,
                            creep_eye_h,
                            h_pos,
                            h_vis_h,
                            eff_creep_vision,
                            0.0,
                        )
                };
                if can_detect && best_target.map_or(true, |(_, _, bd, _)| d < bd) {
                    let aim_y = if h_stance == Stance::Crouch {
                        config.combat.hero_crouch_aim_chest_height * h_scale
                    } else {
                        config.combat.hero_stand_aim_chest_height * h_scale
                    };
                    let upper_y = (h_vis_h - 0.10).max(aim_y);
                    best_target = Some((h_pos + Vec3::Y * aim_y, h_pos + Vec3::Y * upper_y, d, false));
                }
            }
            if !is_neutral {
                for &(t_team, t_pos, t_rad, t_height, t_dead) in &turret_snap {
                    if t_team == my_team || t_dead {
                        continue;
                    }
                    let edge_dist = (my_pos.distance(t_pos) - t_rad).max(0.0);
                    if edge_dist <= eff_creep_vision
                        && best_target.map_or(true, |(_, _, bd, _)| edge_dist < bd)
                    {
                        let to_tur = Vec3::new(my_pos.x - t_pos.x, 0.0, my_pos.z - t_pos.z).normalize_or_zero();
                        let tur_ground_y = sample_terrain_top_y(config, t_pos.x, t_pos.z).max(t_pos.y);
                        let tur_edge_pos = Vec3::new(t_pos.x, tur_ground_y, t_pos.z) + to_tur * (t_rad + 0.45);
                        if can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            tur_edge_pos,
                            c_stats.aim_chest_height,
                            eff_creep_vision,
                            0.0,
                        ) {
                            let p = Vec3::new(t_pos.x, t_pos.y + t_height * 0.5, t_pos.z);
                            best_target = Some((p, p, edge_dist, true));
                        }
                    }
                }
            }
        }
            if !is_neutral && !hero_aggro_locked {
                for &(b_team, b_pos, b_rad, b_height, b_dead, b_invuln) in &barracks_snap {
                    if b_team == my_team || b_dead || b_invuln {
                        continue;
                    }
                    let edge_dist = (my_pos.distance(b_pos) - b_rad).max(0.0);
                    if edge_dist <= eff_creep_vision
                        && best_target.map_or(true, |(_, _, bd, _)| edge_dist < bd)
                    {
                        let to_bar = Vec3::new(my_pos.x - b_pos.x, 0.0, my_pos.z - b_pos.z).normalize_or_zero();
                        let bar_ground_y = sample_terrain_top_y(config, b_pos.x, b_pos.z).max(b_pos.y);
                        let bar_edge_pos = Vec3::new(b_pos.x, bar_ground_y, b_pos.z) + to_bar * (b_rad * 1.45 + 0.6);
                        if can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            bar_edge_pos,
                            c_stats.aim_chest_height,
                            eff_creep_vision,
                            0.0,
                        ) {
                            let p = Vec3::new(b_pos.x, b_pos.y + b_height * 0.5, b_pos.z);
                            best_target = Some((p, p, edge_dist, true));
                        }
                    }
                }
            }
            if !is_neutral && !hero_aggro_locked {
                for &(cour_team, cour_pos, cour_dead) in &courier_snap {
                    if cour_team == my_team || cour_dead {
                        continue;
                    }
                    let d_xz = Vec2::new(my_pos.x, my_pos.z).distance(Vec2::new(cour_pos.x, cour_pos.z));
                    if d_xz <= eff_creep_vision
                        && best_target.map_or(true, |(_, _, bd, _)| d_xz < bd)
                        && can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            cour_pos,
                            c_stats.aim_chest_height,
                            eff_creep_vision,
                            0.0,
                        )
                    {
                        best_target = Some((cour_pos, cour_pos, d_xz, false));
                    }
                }
            }
            if !is_neutral && !hero_aggro_locked {
                for &(a_team, a_pos, a_rad, _a_height, a_dead, a_invuln) in &ancient_snap {
                    if a_team == my_team || a_dead || a_invuln {
                        continue;
                    }
                    let edge_dist = (my_pos.distance(a_pos) - a_rad).max(0.0);
                    if edge_dist <= eff_creep_vision
                        && best_target.map_or(true, |(_, _, bd, _)| edge_dist < bd)
                    {
                        let to_anc = Vec3::new(my_pos.x - a_pos.x, 0.0, my_pos.z - a_pos.z).normalize_or_zero();
                        let anc_ground_y = sample_terrain_top_y(config, a_pos.x, a_pos.z).max(a_pos.y);
                        let anc_edge_pos = Vec3::new(a_pos.x, anc_ground_y, a_pos.z) + to_anc * (a_rad * 1.45 + 0.6);
                        if can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            anc_edge_pos,
                            c_stats.aim_chest_height,
                            eff_creep_vision,
                            0.0,
                        ) {
                            let p = Vec3::new(a_pos.x, a_pos.y + 1.5, a_pos.z);
                            best_target = Some((p, p, edge_dist, true));
                        }
                    }
                }
            }
        if is_neutral && best_target.is_none() && my_pos.distance(my_home) <= 0.5 {
            world.creeps[idx].is_aggroed = false;
            world.creeps[idx].aggro_target_slot = None;
        }

        let mut moved = false;
        let mut fired_or_aiming = false;

        if let Some((target_pos, upper_target_pos, dist, target_is_structure)) = best_target {
            let to_target = Vec3::new(target_pos.x - my_pos.x, 0.0, target_pos.z - my_pos.z);
            if to_target.length_squared() > 0.0001 {
                world.creeps[idx].facing_dir = to_target.normalize();
            }

            let muzzle_pos = my_pos
                + Vec3::Y * c_stats.muzzle_offset_y
                + world.creeps[idx].facing_dir * c_stats.muzzle_forward;

            // بررسی خط آتش واقعی لوله تفنگ: اگر شلیک به سینه به سنگر می‌خورد، به بالاتنه‌ی آشکار هدف نشانه می‌گیرد
            // و اگر لوله تفنگ همچنان پشت سنگر گیر باشد، سرباز به جای شلیک بی‌اثر به سنگر، آن را دور می‌زند
            let clear_aim_opt = if dist <= c_stats.attack_range {
                if target_is_structure {
                    Some(target_pos)
                } else {
                    let is_shot_clear = |aim_pt: Vec3| -> bool {
                        let v = aim_pt - muzzle_pos;
                        let len = v.length();
                        if len <= 1e-4 {
                            return true;
                        }
                        physics_world
                            .cast_ray(muzzle_pos, v / len, len, query_groups, |ent| {
                                target_query.get(ent).is_err()
                            })
                            .is_none()
                    };
                    if is_shot_clear(target_pos) {
                        Some(target_pos)
                    } else if is_shot_clear(upper_target_pos) {
                        Some(upper_target_pos)
                    } else {
                        None
                    }
                }
            } else {
                None
            };

            if let Some(aim_target) = clear_aim_opt {
                fired_or_aiming = true;
                if world.creeps[idx].fire_cooldown <= 0.0 {
                    let (_, _, time_dmg_bonus, time_fr_mult) = lane_creep_time_scaling(
                        config,
                        world.match_elapsed_secs,
                        !is_neutral,
                    );
                    let (_, _, _, dmg_mult, fire_rate_mult) = lane_creep_buffs(
                        config,
                        &world.barracks,
                        my_team,
                        my_lane,
                        !is_neutral,
                    );
                    let eff_fire_rate = (c_stats.fire_rate_secs * time_fr_mult * fire_rate_mult)
                        .max(config.creeps.min_fire_rate_secs);
                    world.creeps[idx].fire_cooldown = eff_fire_rate;
                    world.creeps[idx].fire_anim_timer =
                        (eff_fire_rate * 0.45).min(0.35);

                    let shot_dir = (aim_target - muzzle_pos).normalize_or_zero();
                    let b_id = world.next_entity_id;
                    world.next_entity_id += 1;

                    commands.spawn((
                        PhysicsBullet {
                            id: b_id,
                            owner_slot: None,
                            owner_creep_id: Some(world.creeps[idx].id),
                            owner_turret_index: None,
                            owner_team: my_team,
                            vfx_source: BulletVfxSource::Creep,
                            velocity: shot_dir * c_stats.projectile_speed,
                            damage: (c_stats.bullet_damage + time_dmg_bonus) * dmg_mult,
                            remaining_range: c_stats.bullet_max_range,
                            gravity: 0.0,
                            aoe_radius: 0.0,
                            toxic_pool_duration_secs: 0.0,
                            toxic_pool_dps: 0.0,
                            toxic_slow_ratio: 0.0,
                            piercing: false,
                            hit_entity_keys: Vec::new(),
                        },
                        Transform::from_translation(muzzle_pos),
                    ));
                    world.pending_fx.push(GameplayFxEvent::ShotFired {
                        muzzle_pos,
                        shot_dir,
                        source: ShotAudioSource::Creep,
                        trauma_add: 0.0,
                        trauma_max: config.combat.shot_fire_trauma_max,
                    });
                }
            } else {
                let step_dist = c_stats.move_speed
                    * creep_toxic_slow_at(&world.toxic_pools, world.creeps[idx].pos, world.creeps[idx].team)
                    * dt;
                let cur_pos = world.creeps[idx].pos;
                let avoid_dir = steer_dir_around_obstacles(
                    config,
                    my_id,
                    cur_pos,
                    world.creeps[idx].facing_dir,
                    Some(Vec2::new(target_pos.x, target_pos.z)),
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                let (nx, nz, steered_dir, did_move) = steer_creep_step_avoiding_river(
                    config,
                    cur_pos.x,
                    cur_pos.y,
                    cur_pos.z,
                    avoid_dir,
                    step_dist,
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                world.creeps[idx].pos.x = nx;
                world.creeps[idx].pos.z = nz;
                if did_move {
                    world.creeps[idx].facing_dir = steered_dir;
                }
                moved = did_move;
            }
        } else if is_neutral {
            // سرباز کمپ اگر دشمنی نباشد به جایگاه کمپ خودش برمی‌گردد و منتظر می‌ماند
            let to_home = Vec3::new(my_home.x - my_pos.x, 0.0, my_home.z - my_pos.z);
            if to_home.length() > 0.35 {
                let dir = to_home.normalize();
                let step_dist = c_stats.move_speed
                    * creep_toxic_slow_at(&world.toxic_pools, world.creeps[idx].pos, world.creeps[idx].team)
                    * dt;
                let cur_pos = world.creeps[idx].pos;
                let avoid_dir = steer_dir_around_obstacles(
                    config,
                    my_id,
                    cur_pos,
                    dir,
                    Some(Vec2::new(my_home.x, my_home.z)),
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                let (nx, nz, steered_dir, did_move) = steer_creep_step_avoiding_river(
                    config,
                    cur_pos.x,
                    cur_pos.y,
                    cur_pos.z,
                    avoid_dir,
                    step_dist,
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                world.creeps[idx].pos.x = nx;
                world.creeps[idx].pos.z = nz;
                world.creeps[idx].facing_dir = steered_dir;
                moved = did_move;
            }
        } else {
            // ب) حرکت در امتداد Waypointهای لاین و پس از آن پیشروی مستقیم به سمت ساختمان اصلی (Ancient) دشمن
            let mut move_target_opt: Option<(Vec3, f32)> = None;
            if world.creeps[idx].waypoint_index == usize::MAX {
                if let Some(&(_a_team, a_pos, a_rad, _a_h, _a_dead, _a_invuln)) = ancient_snap
                    .iter()
                    .find(|&&(a_team, _, _, _, a_dead, _)| a_team != my_team && !a_dead)
                {
                    let to_anc = Vec3::new(a_pos.x - my_pos.x, 0.0, a_pos.z - my_pos.z);
                    let stop_dist = a_rad * 1.45 + c_stats.collider_radius + 1.0;
                    if to_anc.length() > stop_dist {
                        move_target_opt = Some((to_anc, to_anc.length()));
                    }
                }
            } else if let Some(lane_def) = config.map.lanes.iter().find(|l| l.lane == my_lane) {
                let wp_len = lane_def.waypoints.len();
                let mut wp_idx = world.creeps[idx]
                    .waypoint_index
                    .min(wp_len.saturating_sub(1));
                let wp = lane_def.waypoints[wp_idx];
                let my_xz = Vec2::new(my_pos.x, my_pos.z);
                let wp_xz = Vec2::new(wp.0, wp.2);

                // بازیابی هوشمند مسیر لاین (مشابه Dota 2): اگر سرباز به خارج از لاین کشیده شده یا خط مستقیم به Waypoint از رودخانه عبور کند،
                // ابتدا به نزدیک‌ترین نقطه قابل‌دسترس لاین در سمت خودش برمی‌گردد و سپس روی پل لاین پیشروی می‌کند.
                let (nearest_pt, nearest_seg, dist_from_lane) =
                    nearest_point_on_lane_xz(lane_def, my_xz);
                let direct_blocked =
                    segment_crosses_river_barrier(config, my_xz, wp_xz, safety_margin);

                if direct_blocked || dist_from_lane > config.creeps.lane_rejoin_max_deviation {
                    if !segment_crosses_river_barrier(config, my_xz, nearest_pt, safety_margin)
                        && dist_from_lane > lane_def.width * 0.40
                    {
                        let rejoined_wp = match my_team {
                            TeamId::Dawn => (nearest_seg + 1).min(wp_len.saturating_sub(1)),
                            TeamId::Dusk => nearest_seg,
                            TeamId::Neutral => wp_idx,
                        };
                        world.creeps[idx].waypoint_index = rejoined_wp;
                        let to_rejoin = Vec3::new(nearest_pt.x - my_pos.x, 0.0, nearest_pt.y - my_pos.z);
                        let d_rejoin = to_rejoin.length();
                        if d_rejoin > 0.1 {
                            move_target_opt = Some((to_rejoin, d_rejoin));
                        }
                    } else if direct_blocked {
                        // یافتن نزدیک‌ترین Waypoint قابل‌دسترس بدون عبور از رودخانه تا سرباز از روی پل عبور کند
                        let mut best_reach_wp: Option<(usize, f32)> = None;
                        for (wi, &(wx, _wy, wz)) in lane_def.waypoints.iter().enumerate() {
                            let cand_xz = Vec2::new(wx, wz);
                            if !segment_crosses_river_barrier(config, my_xz, cand_xz, safety_margin) {
                                let d = my_xz.distance(cand_xz);
                                if best_reach_wp.map_or(true, |(_, bd)| d < bd) {
                                    best_reach_wp = Some((wi, d));
                                }
                            }
                        }
                        if let Some((best_wi, _)) = best_reach_wp {
                            wp_idx = best_wi;
                            world.creeps[idx].waypoint_index = best_wi;
                        }
                    }
                }

                if move_target_opt.is_none() {
                    let wp = lane_def.waypoints[wp_idx];
                    let wp_pos = Vec3::new(wp.0, wp.1, wp.2);
                    let to_wp = Vec3::new(wp_pos.x - my_pos.x, 0.0, wp_pos.z - my_pos.z);
                    let dist_wp = to_wp.length();

                    // اگر Waypoint روی مرکز یک تاور یا مانع باشد، شعاع رسیدن به Waypoint بزرگ‌تر می‌شود تا سرباز پشت آن گیر نکند
                    let mut reach_dist = config.creeps.waypoint_reach_dist;
                    for &(_t_team, t_pos, t_rad, _t_h, t_dead) in &turret_snap {
                        if !t_dead && Vec2::new(wp_pos.x - t_pos.x, wp_pos.z - t_pos.z).length() <= t_rad + 2.0 {
                            reach_dist = reach_dist.max(t_rad + c_stats.collider_radius + 1.5);
                        }
                    }
                    for obs in &config.obstacles {
                        let Some((center, half, _rot, inv_rot, min_y, max_y)) =
                            creep_obstacle_obb_2d(obs)
                        else {
                            continue;
                        };
                        if wp_pos.y + c_stats.hitbox_height < min_y || wp_pos.y > max_y + 0.5 {
                            continue;
                        }
                        let local = inv_rot * Vec3::new(wp_pos.x - center.x, 0.0, wp_pos.z - center.y);
                        if local.x.abs() <= half.x + obs_radius && local.z.abs() <= half.y + obs_radius {
                            reach_dist = reach_dist.max(half.length() + obs_radius + 1.2);
                        }
                    }

                    if dist_wp <= reach_dist {
                        match my_team {
                            TeamId::Dawn => {
                                if wp_idx + 1 < wp_len {
                                    world.creeps[idx].waypoint_index = wp_idx + 1;
                                } else {
                                    world.creeps[idx].waypoint_index = usize::MAX;
                                }
                            }
                            TeamId::Dusk => {
                                if wp_idx > 0 {
                                    world.creeps[idx].waypoint_index = wp_idx - 1;
                                } else {
                                    world.creeps[idx].waypoint_index = usize::MAX;
                                }
                            }
                            TeamId::Neutral => {}
                        }
                    } else {
                        move_target_opt = Some((to_wp, dist_wp));
                    }
                }
            }

            if let Some((to_wp, dist_wp)) = move_target_opt {
                if dist_wp > 0.01 {
                    let mut dir = to_wp / dist_wp;
                    // هدایت نرم به طرفین هنگام نزدیک شدن به بدنه تاور
                    for &(_t_team, t_pos, t_rad, _t_h, t_dead) in &turret_snap {
                        if t_dead || (my_pos.y - t_pos.y).abs() > 3.0 {
                            continue;
                        }
                        let to_tur = Vec2::new(t_pos.x - my_pos.x, t_pos.z - my_pos.z);
                        let dist_t = to_tur.length();
                        let avoid_r = t_rad + c_stats.collider_radius + 1.2;
                        if dist_t < avoid_r && dist_t > 0.001 {
                            let fwd2 = Vec2::new(dir.x, dir.z);
                            if to_tur.normalize().dot(fwd2) > 0.25 {
                                let perp = Vec2::new(-to_tur.y, to_tur.x).normalize();
                                let side_sign = if perp.dot(fwd2) >= 0.0 { 1.0 } else { -1.0 };
                                let steer = (fwd2 + perp * (side_sign * 1.35)).normalize();
                                dir = Vec3::new(steer.x, 0.0, steer.y);
                            }
                        }
                    }
                    // مکانیک مهارتی Creep Blocking (مشابه Dota 2):
                    // اگر هیرو (به‌ویژه هیروی خودی) دقیقاً جلوی مسیر سرباز قرار گیرد، سرباز مکث بسیار کوتاهی کرده
                    // و سپس هوشمندانه از سمت بازتر دور هیرو می‌چرخد و عبور می‌کند.
                    let mut blocked_hesitating = false;
                    let mut saw_blocking_hero = false;
                    let fwd2 = Vec2::new(dir.x, dir.z).normalize_or_zero();
                    let mut best_steer_vec = Vec2::ZERO;

                    for &(h_team, h_pos, h_rad) in &hero_body_snap {
                        if (my_pos.y - h_pos.y).abs() > 1.8 {
                            continue;
                        }
                        let to_hero = Vec2::new(h_pos.x - my_pos.x, h_pos.z - my_pos.z);
                        let dist_h = to_hero.length();
                        let min_d = h_rad + c_stats.collider_radius;
                        let detect_r = min_d + config.creeps.hero_block_detect_dist;
                        if dist_h < detect_r && dist_h > 0.001 {
                            let to_hero_dir = to_hero / dist_h;
                            let front_dot = to_hero_dir.dot(fwd2);
                            if front_dot > 0.15 {
                                saw_blocking_hero = true;
                                // فقط اگر هیروی خودی دقیقاً در مخروط جلویی سرباز ایستاده باشد، مکث کوتاه واکنش فعال می‌شود
                                let head_on_dist = min_d + config.creeps.hero_block_detect_dist * 0.65;
                                if h_team == my_team
                                    && front_dot >= config.creeps.hero_block_front_cone_dot
                                    && dist_h <= head_on_dist
                                {
                                    if world.creeps[idx].hero_block_hesitation_timer <= 0.0
                                        && world.creeps[idx].hero_block_cooldown_timer <= 0.0
                                    {
                                        world.creeps[idx].hero_block_hesitation_timer =
                                            config.creeps.hero_block_hesitation_secs;
                                        world.creeps[idx].hero_block_cooldown_timer =
                                            config.creeps.hero_block_hesitation_secs
                                                + config.creeps.hero_block_hesitation_cooldown_secs;
                                    }
                                    if world.creeps[idx].hero_block_hesitation_timer > 0.0 {
                                        blocked_hesitating = true;
                                    }
                                }

                                // انتخاب هوشمند سمت دور زدن هیرو (بدون پرش چپ و راست و بدون رفتن سمت پرتگاه)
                                let perp = Vec2::new(-to_hero_dir.y, to_hero_dir.x);
                                let cross = fwd2.x * to_hero_dir.y - fwd2.y * to_hero_dir.x;
                                let mut side_sign = if world.creeps[idx].hero_block_steer_side != 0.0 {
                                    world.creeps[idx].hero_block_steer_side
                                } else if cross.abs() > 0.06 {
                                    if cross > 0.0 { -1.0 } else { 1.0 }
                                } else if (my_id & 1) == 0 {
                                    1.0
                                } else {
                                    -1.0
                                };
                                // اگر سمت انتخاب‌شده به پرتگاه رودخانه می‌رسد، از سمت دیگر دور می‌زند
                                let probe_side = Vec2::new(my_pos.x, my_pos.z) + perp * (side_sign * (min_d + 0.6));
                                if !is_creep_safe_ground_xz(config, probe_side.x, probe_side.y, safety_margin) {
                                    side_sign = -side_sign;
                                }
                                world.creeps[idx].hero_block_steer_side = side_sign;

                                let prox = ((detect_r - dist_h) / config.creeps.hero_block_detect_dist.max(0.05))
                                    .clamp(0.0, 1.0);
                                best_steer_vec += perp * (side_sign * prox * config.creeps.hero_block_steer_strength);
                            }
                        }
                    }

                    if !saw_blocking_hero {
                        world.creeps[idx].hero_block_steer_side = 0.0;
                    }

                    if blocked_hesitating {
                        world.creeps[idx].facing_dir = dir;
                        moved = false;
                    } else {
                        if best_steer_vec.length_squared() > 0.0001 {
                            let steered2 = (fwd2 + best_steer_vec).normalize_or_zero();
                            if steered2.length_squared() > 0.0001 {
                                dir = Vec3::new(steered2.x, 0.0, steered2.y);
                            }
                        }
                        let step_dist = c_stats.move_speed
                            * creep_toxic_slow_at(&world.toxic_pools, world.creeps[idx].pos, world.creeps[idx].team)
                            * dt;
                        let cur_pos = world.creeps[idx].pos;
                        let goal_xz = Vec2::new(my_pos.x + to_wp.x, my_pos.z + to_wp.z);
                        let avoid_dir = steer_dir_around_obstacles(
                            config,
                            my_id,
                            cur_pos,
                            dir,
                            Some(goal_xz),
                            c_stats.hitbox_height,
                            safety_margin,
                            obs_radius,
                        );
                        let (nx, nz, steered_dir, did_move) = steer_creep_step_avoiding_river(
                            config,
                            cur_pos.x,
                            cur_pos.y,
                            cur_pos.z,
                            avoid_dir,
                            step_dist,
                            c_stats.hitbox_height,
                            safety_margin,
                            obs_radius,
                        );
                        world.creeps[idx].pos.x = nx;
                        world.creeps[idx].pos.z = nz;
                        world.creeps[idx].facing_dir = steered_dir;
                        moved = did_move;
                    }
                }
            }
        }

        // ج) هم‌ترازی ارتفاع سرباز با سطح واقعی زمین/رمپ/پل و بستر دره عمیق
        let cx = world.creeps[idx].pos.x;
        let cz = world.creeps[idx].pos.z;
        let canyon_h = canyon_sample_height(&config.map.canyon, cx, cz);
        let inside_canyon = config.map.canyon.enabled && canyon_h < -0.15;
        let ray_origin = Vec3::new(cx, world.creeps[idx].pos.y + 3.0, cz);
        let hit_opt = physics_world.cast_ray(
            ray_origin,
            Vec3::NEG_Y,
            18.0,
            query_groups,
            |ent| {
                if target_query.get(ent).is_ok() || obstacle_query.get(ent).is_ok() {
                    return false;
                }
                if inside_canyon {
                    if let Ok(elev) = elev_query.get(ent) {
                        if elev.elevation_tier <= 0 {
                            return false;
                        }
                    }
                }
                true
            },
        );
        let min_safe_ground_y = if config.map.canyon.enabled {
            config.map.canyon.water_y + 0.5
        } else {
            f32::NEG_INFINITY
        };
        if let Some((_ent, hit)) = hit_opt {
            if hit.normal.y > 0.5 && hit.point.y >= min_safe_ground_y {
                world.creeps[idx].pos.y = hit.point.y;
            } else if inside_canyon {
                // جلوگیری قطعی از سقوط به بستر رودخانه یا پرواز در هوا
                world.creeps[idx].pos = prev_safe_pos;
            }
        } else if inside_canyon {
            // هرگز سرباز را به داخل آب نمی‌اندازیم؛ به آخرین موقعیت امن روی لبه یا پل برمی‌گردانیم
            world.creeps[idx].pos = prev_safe_pos;
        }

        // جلوگیری از ورود و گیر افتادن سربازان پشت پایه تاورها (به همراه لغزش مماسی دور تاور)
        for &(_t_team, t_pos, t_rad, _t_h, t_dead) in &turret_snap {
            if t_dead || (world.creeps[idx].pos.y - t_pos.y).abs() > 3.0 {
                continue;
            }
            let min_d = t_rad + c_stats.collider_radius + 0.15;
            let delta = Vec2::new(world.creeps[idx].pos.x - t_pos.x, world.creeps[idx].pos.z - t_pos.z);
            let dist = delta.length();
            if dist < min_d {
                let push = if dist > 0.0001 { delta / dist } else { Vec2::new(0.0, 1.0) };
                let (px, pz, _) = move_creep_xz_with_river_barrier(
                    config,
                    world.creeps[idx].pos.x,
                    world.creeps[idx].pos.y,
                    world.creeps[idx].pos.z,
                    t_pos.x + push.x * min_d,
                    t_pos.z + push.y * min_d,
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                world.creeps[idx].pos.x = px;
                world.creeps[idx].pos.z = pz;
                if moved {
                    let f = Vec2::new(world.creeps[idx].facing_dir.x, world.creeps[idx].facing_dir.z);
                    let t1 = Vec2::new(-push.y, push.x);
                    let tangent = if t1.dot(f) >= 0.0 { t1 } else { -t1 };
                    let slide = tangent * c_stats.move_speed * creep_toxic_slow_at(&world.toxic_pools, world.creeps[idx].pos, world.creeps[idx].team) * dt * 0.85;
                    let (sx, sz, _) = move_creep_xz_with_river_barrier(
                        config,
                        world.creeps[idx].pos.x,
                        world.creeps[idx].pos.y,
                        world.creeps[idx].pos.z,
                        world.creeps[idx].pos.x + slide.x,
                        world.creeps[idx].pos.z + slide.y,
                        c_stats.hitbox_height,
                        safety_margin,
                        obs_radius,
                    );
                    world.creeps[idx].pos.x = sx;
                    world.creeps[idx].pos.z = sz;
                }
            }
        }

        // جلوگیری از نفوذ سرباز به داخل بدن هیرو (در صورت تماس، سرباز روی محیط هیرو می‌لغزد و هیرو را هل نمی‌دهد)
        for &(_h_team, h_pos, h_rad) in &hero_body_snap {
            if (world.creeps[idx].pos.y - h_pos.y).abs() > 1.8 {
                continue;
            }
            let min_d = h_rad + c_stats.collider_radius;
            let delta = Vec2::new(world.creeps[idx].pos.x - h_pos.x, world.creeps[idx].pos.z - h_pos.z);
            let dist = delta.length();
            if dist < min_d {
                let push = if dist > 0.0001 {
                    delta / dist
                } else {
                    let f = Vec2::new(world.creeps[idx].facing_dir.x, world.creeps[idx].facing_dir.z).normalize_or_zero();
                    if f.length_squared() > 0.01 { -f } else { Vec2::new(1.0, 0.0) }
                };
                let (cx, cz, _) = move_creep_xz_with_river_barrier(
                    config,
                    world.creeps[idx].pos.x,
                    world.creeps[idx].pos.y,
                    world.creeps[idx].pos.z,
                    h_pos.x + push.x * min_d,
                    h_pos.z + push.y * min_d,
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                world.creeps[idx].pos.x = cx;
                world.creeps[idx].pos.z = cz;
            }
        }

        // جلوگیری قطعی از نفوذ سرباز به داخل موانع مسیر (OBB Push-Out)
        let (ox, oz) = resolve_creep_obstacle_penetration(
            config,
            world.creeps[idx].pos,
            c_stats.hitbox_height,
            safety_margin,
            obs_radius,
        );
        world.creeps[idx].pos.x = ox;
        world.creeps[idx].pos.z = oz;

        world.creeps[idx].anim_state = if fired_or_aiming {
            if is_neutral || world.creeps[idx].fire_anim_timer > 0.0 {
                CreepAnimState::Fire
            } else {
                CreepAnimState::Idle
            }
        } else if moved {
            CreepAnimState::Run
        } else {
            CreepAnimState::Idle
        };
    }

    // ۴. حل برخورد فیزیکی سربازان با یکدیگر (عدم تداخل فیزیکی سربازها)
    let min_creep_dist = config.creeps.collider_radius * 2.0;
    for i in 0..c_len {
        if world.creeps[i].is_dead {
            continue;
        }
        for j in (i + 1)..c_len {
            if world.creeps[j].is_dead {
                continue;
            }
            if (world.creeps[i].pos.y - world.creeps[j].pos.y).abs() > 1.8 {
                continue;
            }
            let delta = Vec2::new(
                world.creeps[j].pos.x - world.creeps[i].pos.x,
                world.creeps[j].pos.z - world.creeps[i].pos.z,
            );
            let dist = delta.length();
            if dist < min_creep_dist {
                let push_dir = if dist > 0.0001 {
                    delta / dist
                } else {
                    Vec2::new(1.0, 0.0)
                };
                let sep_margin = config.creeps.collider_radius + config.creeps.river_barrier_margin;
                let sep_obs_r = config.creeps.collider_radius + config.creeps.obstacle_avoid_margin;
                let sep_h = config.creeps.hitbox_height;
                let half_push = (min_creep_dist - dist) * 0.5;
                let (nix, niz, _) = move_creep_xz_with_river_barrier(
                    config,
                    world.creeps[i].pos.x,
                    world.creeps[i].pos.y,
                    world.creeps[i].pos.z,
                    world.creeps[i].pos.x - push_dir.x * half_push,
                    world.creeps[i].pos.z - push_dir.y * half_push,
                    sep_h,
                    sep_margin,
                    sep_obs_r,
                );
                world.creeps[i].pos.x = nix;
                world.creeps[i].pos.z = niz;

                let (njx, njz, _) = move_creep_xz_with_river_barrier(
                    config,
                    world.creeps[j].pos.x,
                    world.creeps[j].pos.y,
                    world.creeps[j].pos.z,
                    world.creeps[j].pos.x + push_dir.x * half_push,
                    world.creeps[j].pos.z + push_dir.y * half_push,
                    sep_h,
                    sep_margin,
                    sep_obs_r,
                );
                world.creeps[j].pos.x = njx;
                world.creeps[j].pos.z = njz;
            }
        }
    }

    // ۵. تضمین عدم نفوذ سرباز به داخل هیرو و موانع پس از جداسازی سربازان از یکدیگر
    for i in 0..c_len {
        if world.creeps[i].is_dead {
            continue;
        }
        let c_stats = resolve_creep_stats(config, world.creeps[i].creep_type_id.as_deref());
        let safety_margin = c_stats.collider_radius + config.creeps.river_barrier_margin;
        let obs_radius = c_stats.collider_radius + config.creeps.obstacle_avoid_margin;
        for &(_h_team, h_pos, h_rad) in &hero_body_snap {
            if (world.creeps[i].pos.y - h_pos.y).abs() > 1.8 {
                continue;
            }
            let min_d = h_rad + c_stats.collider_radius;
            let delta = Vec2::new(world.creeps[i].pos.x - h_pos.x, world.creeps[i].pos.z - h_pos.z);
            let dist = delta.length();
            if dist < min_d {
                let push = if dist > 0.0001 { delta / dist } else { Vec2::new(1.0, 0.0) };
                let (cx, cz, _) = move_creep_xz_with_river_barrier(
                    config,
                    world.creeps[i].pos.x,
                    world.creeps[i].pos.y,
                    world.creeps[i].pos.z,
                    h_pos.x + push.x * min_d,
                    h_pos.z + push.y * min_d,
                    c_stats.hitbox_height,
                    safety_margin,
                    obs_radius,
                );
                world.creeps[i].pos.x = cx;
                world.creeps[i].pos.z = cz;
            }
        }
        let (ox, oz) = resolve_creep_obstacle_penetration(
            config,
            world.creeps[i].pos,
            c_stats.hitbox_height,
            safety_margin,
            obs_radius,
        );
        world.creeps[i].pos.x = ox;
        world.creeps[i].pos.z = oz;
    }
}


fn step_turrets_system(
    commands: &mut Commands,
    dt: f32,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    world: &mut AuthoritativeWorldState,
) {
    let turret_status: Vec<(TeamId, LaneId, u8, bool)> = world
        .turrets
        .iter()
        .map(|t| (t.team, t.lane, t.tier, t.is_dead))
        .collect();

    let creep_snap: Vec<(TeamId, Vec3, bool, bool)> = world
        .creeps
        .iter()
        .map(|c| (c.team, c.pos, c.is_dead, c.camp_index.is_none()))
        .collect();

    let hero_snap: Vec<(TeamId, Vec3, Stance, f32, bool, bool, bool, f32, bool, bool)> = world
        .players
        .iter()
        .map(|p| {
            let hidden_invis = p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret;
            let water_vis_dawn = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dawn,
                config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            let water_vis_dusk = is_swimming_hero_revealed_to_team(
                &world.players,
                p.slot_index,
                TeamId::Dusk,
                config,
                world.match_elapsed_secs,
                world.current_weather,
            );
            (
                p.hero_team,
                p.soldier_pos,
                p.effective_stance,
                p.hero_scale,
                p.is_dead || hidden_invis,
                water_vis_dawn,
                water_vis_dusk,
                p.last_attacked_enemy_hero_at,
                p.attack_reveal_to_dawn_remaining > 0.0,
                p.attack_reveal_to_dusk_remaining > 0.0,
            )
        })
        .collect();

    let courier_snap: Vec<(TeamId, Vec3, bool)> = world
        .players
        .iter()
        .map(|p| (p.hero_team, p.courier_pos, p.courier_is_dead))
        .collect();
    let backdoor_r = config.combat.turret_backdoor_creep_radius;
    let spread_rad = config.combat.turret_bullet_spread_rad;

    for idx in 0..world.turrets.len() {
        if world.turrets[idx].is_dead {
            world.turrets[idx].aim_target_pos = None;
            continue;
        }

        let my_team = world.turrets[idx].team;
        let my_lane = world.turrets[idx].lane;
        let my_tier = world.turrets[idx].tier;
        let my_pos = world.turrets[idx].pos;
        let t_def = &config.map.turrets[idx];
        let t_scale = t_def.model_transform.scale_vec();
        let t_world_radius = t_def.base_radius * t_scale.x.max(t_scale.z);
        let t_world_height = world.turrets[idx].height;

        // ۱. اولویت تاورها مطابق قوانین دقیق Dota 2:
        // - برای Tier 1, 2, 3: تا زمانی که تاور رده پایین‌تر همان لاین زنده است، ضدضربه باقی می‌ماند.
        // - برای Tier 4 (دوقلوهای جلوی Ancient): به محض نابودی حداقل یک تاور Tier 3 در هر لاینی (Top/Mid/Bot) آسیب‌پذیر می‌شوند.
        let prev_tier_alive = if my_tier >= 4 {
            !turret_status
                .iter()
                .any(|&(tm, _ln, tr, dead)| tm == my_team && tr == 3 && dead)
        } else {
            turret_status.iter().any(|&(tm, ln, tr, dead)| {
                tm == my_team && ln == my_lane && tr < my_tier && !dead
            })
        };

        // ۲. محافظت Backdoor: دشمن برای ضربه زدن به تاور حتماً باید سرباز لاین زنده در محدوده تاور داشته باشد
        let has_enemy_lane_creeps = if t_def.backdoor_protection_requires_creeps {
            creep_snap.iter().any(|&(c_team, c_pos, c_dead, is_lane_creep)| {
                !c_dead && is_lane_creep && c_team != my_team && c_team != TeamId::Neutral && c_pos.distance(my_pos) <= backdoor_r
            })
        } else {
            true
        };

        world.turrets[idx].is_invulnerable = prev_tier_alive || !has_enemy_lane_creeps;
        world.turrets[idx].fire_cooldown = (world.turrets[idx].fire_cooldown - dt).max(0.0);

        // ۳. انتخاب هدف تاور مطابق قوانین MOBA:
        // اولویت ۰: هیروی دشمنی که اخیراً به هیروی هم‌تیمی زیر تاور آسیب زده (Tower-Dive Aggro)
        let aggro_window = config.combat.turret_hero_aggro_window_secs;
        let mut best_aggro_hero: Option<(Vec3, f32)> = None;
        let turret_eye_h = config.map.fow_stand_eye_height;
        let eff_turret_vision = t_def.vision_range
            * config.effective_vision_multiplier(
                world.match_elapsed_secs,
                config.day_night.night_turret_vision_multiplier,
            );
        if config.gameplay_rules.turret_prioritize_hero_on_ally_attack {
        for &(h_team, h_pos, h_stance, h_scale, h_dead, h_water_vis_dawn, h_water_vis_dusk, h_last_atk_hero, h_rev_dawn, h_rev_dusk) in &hero_snap {
            let water_visible = match my_team {
                TeamId::Dawn => h_water_vis_dawn,
                TeamId::Dusk => h_water_vis_dusk,
                TeamId::Neutral => false,
            };
            if h_team == my_team
                || h_dead
                || !water_visible
                || (world.match_elapsed_secs - h_last_atk_hero) > aggro_window
            {
                continue;
            }
            let attack_revealed = match my_team {
                TeamId::Dawn => h_rev_dawn,
                TeamId::Dusk => h_rev_dusk,
                TeamId::Neutral => false,
            };
            let d = my_pos.distance(h_pos);
            let to_h = Vec3::new(h_pos.x - my_pos.x, 0.0, h_pos.z - my_pos.z).normalize_or_zero();
            let turret_ground_y = sample_terrain_top_y(config, my_pos.x, my_pos.z).max(my_pos.y);
            let ray_start = Vec3::new(my_pos.x, turret_ground_y, my_pos.z) + to_h * (t_world_radius + 0.45);
            let h_vis_h = SharedFogOfWar::hero_visible_height(config, h_stance, h_scale);
            if d <= t_def.attack_range
                && best_aggro_hero.map_or(true, |(_, bd)| d < bd)
                && (attack_revealed
                    || can_observer_see_target_3d(
                        physics_world,
                        config,
                        ray_start,
                        turret_eye_h,
                        h_pos,
                        h_vis_h,
                        eff_turret_vision,
                        0.0,
                    ))
            {
                let aim_y = if h_stance == Stance::Crouch {
                    config.combat.hero_crouch_aim_chest_height * h_scale
                } else {
                    config.combat.hero_stand_aim_chest_height * h_scale
                };
                best_aggro_hero = Some((h_pos + Vec3::Y * aim_y, d));
            }
        }
        }

        // اولویت ۱: نزدیک‌ترین سرباز دشمن در برد، سپس هیروی دشمن
        let mut best_creep: Option<(Vec3, f32)> = None;
        for &(c_team, c_pos, c_dead, is_lane_creep) in &creep_snap {
            if c_dead || !is_lane_creep || c_team == my_team || c_team == TeamId::Neutral {
                continue;
            }
            let d = my_pos.distance(c_pos);
            let to_c = Vec3::new(c_pos.x - my_pos.x, 0.0, c_pos.z - my_pos.z).normalize_or_zero();
            let turret_ground_y = sample_terrain_top_y(config, my_pos.x, my_pos.z).max(my_pos.y);
            let ray_start = Vec3::new(my_pos.x, turret_ground_y, my_pos.z) + to_c * (t_world_radius + 0.45);
            if d <= t_def.attack_range
                && best_creep.map_or(true, |(_, bd)| d < bd)
                && can_observer_see_target_3d(
                    physics_world,
                    config,
                    ray_start,
                    turret_eye_h,
                    c_pos,
                    config.creeps.hitbox_height,
                    eff_turret_vision,
                    0.0,
                )
            {
                best_creep = Some((c_pos + Vec3::Y * config.creeps.aim_chest_height, d));
            }
        }

        let mut best_hero: Option<(Vec3, f32)> = None;
        if best_aggro_hero.is_none() && best_creep.is_none() {
            for &(h_team, h_pos, h_stance, h_scale, h_dead, h_water_vis_dawn, h_water_vis_dusk, _, h_rev_dawn, h_rev_dusk) in &hero_snap {
                let water_visible = match my_team {
                    TeamId::Dawn => h_water_vis_dawn,
                    TeamId::Dusk => h_water_vis_dusk,
                    TeamId::Neutral => false,
                };
                if h_team == my_team || h_dead || !water_visible {
                    continue;
                }
                let attack_revealed = match my_team {
                    TeamId::Dawn => h_rev_dawn,
                    TeamId::Dusk => h_rev_dusk,
                    TeamId::Neutral => false,
                };
                let d = my_pos.distance(h_pos);
                let to_h = Vec3::new(h_pos.x - my_pos.x, 0.0, h_pos.z - my_pos.z).normalize_or_zero();
                let turret_ground_y = sample_terrain_top_y(config, my_pos.x, my_pos.z).max(my_pos.y);
                let ray_start = Vec3::new(my_pos.x, turret_ground_y, my_pos.z) + to_h * (t_world_radius + 0.45);
                let h_vis_h = SharedFogOfWar::hero_visible_height(config, h_stance, h_scale);
                if d <= t_def.attack_range
                    && best_hero.map_or(true, |(_, bd)| d < bd)
                    && (attack_revealed
                        || can_observer_see_target_3d(
                            physics_world,
                            config,
                            ray_start,
                            turret_eye_h,
                            h_pos,
                            h_vis_h,
                            eff_turret_vision,
                            0.0,
                        ))
                {
                    let aim_y = if h_stance == Stance::Crouch {
                        config.combat.hero_crouch_aim_chest_height * h_scale
                    } else {
                        config.combat.hero_stand_aim_chest_height * h_scale
                    };
                    best_hero = Some((h_pos + Vec3::Y * aim_y, d));
                }
            }
        }

        let mut best_courier: Option<(Vec3, f32)> = None;
        if best_creep.is_none() && best_hero.is_none() {
            for &(c_team, c_pos, c_dead) in &courier_snap {
                if c_team == my_team || c_dead {
                    continue;
                }
                let d_xz = Vec2::new(my_pos.x, my_pos.z).distance(Vec2::new(c_pos.x, c_pos.z));
                if d_xz <= t_def.attack_range && best_courier.map_or(true, |(_, bd)| d_xz < bd) {
                    best_courier = Some((c_pos, d_xz));
                }
            }
        }
        let chosen_target = best_aggro_hero.or(best_creep).or(best_hero).or(best_courier).map(|(p, _)| p);
        world.turrets[idx].aim_target_pos = chosen_target;

        // ۴. شلیک رگباری متناوب دولول
        if let Some(target_pos) = chosen_target {
            if world.turrets[idx].fire_cooldown <= 0.0 {
                world.turrets[idx].fire_cooldown = t_def.fire_rate_secs;
                world.turrets[idx].barrel_toggle = !world.turrets[idx].barrel_toggle;
                let barrel_left = world.turrets[idx].barrel_toggle;

                let muzzle_origin = Vec3::new(my_pos.x, my_pos.y.max(0.0) + t_world_height.max(3.5) * 0.72, my_pos.z);
                let base_dir = (target_pos - muzzle_origin).normalize_or_zero();
                let rx = (world.next_rand() - 0.5) * 2.0 * spread_rad;
                let ry = (world.next_rand() - 0.5) * 2.0 * spread_rad * 0.5;
                let shot_dir = (Quat::from_euler(EulerRot::YXZ, rx, ry, 0.0) * base_dir).normalize_or_zero();

                let b_id = world.next_entity_id;
                world.next_entity_id += 1;
                commands.spawn((
                    PhysicsBullet {
                        id: b_id,
                        owner_slot: None,
                        owner_creep_id: None,
                        owner_turret_index: Some(idx),
                        owner_team: my_team,
                        vfx_source: BulletVfxSource::Turret,
                        velocity: shot_dir * t_def.projectile_speed,
                        damage: t_def.bullet_damage,
                        remaining_range: t_def.bullet_max_range,
                        gravity: 0.0,
                        aoe_radius: 0.0,
                        toxic_pool_duration_secs: 0.0,
                        toxic_pool_dps: 0.0,
                        toxic_slow_ratio: 0.0,
                        piercing: false,
                        hit_entity_keys: Vec::new(),
                    },
                    Transform::from_translation(muzzle_origin + shot_dir * (t_world_radius + 0.55)),
                ));

                world.pending_fx.push(GameplayFxEvent::TurretShotFired {
                    turret_index: idx,
                    barrel_left,
                    target_pos,
                    shot_dir,
                });
            }
        }
    }

    // ۵. شلیک واقعی و بصری چشمه پایگاه به دشمنان متجاوز (قابل کنترل از gameplay_rules.fountain_attacks_enemies)
    if config.gameplay_rules.fountain_attacks_enemies {
        if world.fountain_fire_cooldowns.len() != config.map.fountains.len() {
            world.fountain_fire_cooldowns.resize(config.map.fountains.len(), 0.0);
        }
        for (f_idx, ftn) in config.map.fountains.iter().enumerate() {
            if ftn.team == TeamId::Neutral {
                continue;
            }
            world.fountain_fire_cooldowns[f_idx] = (world.fountain_fire_cooldowns[f_idx] - dt).max(0.0);
            if world.fountain_fire_cooldowns[f_idx] > 0.0 {
                continue;
            }
            let ftn_pos = ftn.model_transform.location_vec();
            let mut best_target: Option<(Vec3, f32)> = None;
            for p in &world.players {
                if p.is_dead || p.hero_team == ftn.team || p.hero_team == TeamId::Neutral {
                    continue;
                }
                let hidden_invis = p.invisibility_remaining > 0.0 && !p.is_revealed_by_turret;
                if hidden_invis {
                    continue;
                }
                let d_xz = Vec2::new(p.soldier_pos.x - ftn_pos.x, p.soldier_pos.z - ftn_pos.z).length();
                if d_xz <= ftn.heal_radius && best_target.map_or(true, |(_, bd)| d_xz < bd) {
                    best_target = Some((p.soldier_pos + Vec3::Y * config.combat.hero_stand_aim_chest_height, d_xz));
                }
            }
            if let Some((target_pos, _)) = best_target {
                world.fountain_fire_cooldowns[f_idx] = ftn.fire_rate_secs;
                let muzzle_origin = ftn_pos + Vec3::Y * (ftn.rim_height + 2.4);
                let shot_dir = (target_pos - muzzle_origin).normalize_or_zero();
                if shot_dir != Vec3::ZERO {
                    let b_id = world.next_entity_id;
                    world.next_entity_id += 1;
                    let bolt_dmg = (ftn.enemy_damage_per_sec * ftn.fire_rate_secs).max(1.0);
                    commands.spawn((
                        PhysicsBullet {
                            id: b_id,
                            owner_slot: None,
                            owner_creep_id: None,
                            owner_turret_index: None,
                            owner_team: ftn.team,
                            vfx_source: BulletVfxSource::Turret,
                            velocity: shot_dir * ftn.projectile_speed,
                            damage: bolt_dmg,
                            remaining_range: ftn.heal_radius + 10.0,
                            gravity: 0.0,
                            aoe_radius: 0.0,
                            toxic_pool_duration_secs: 0.0,
                            toxic_pool_dps: 0.0,
                            toxic_slow_ratio: 0.0,
                            piercing: false,
                            hit_entity_keys: Vec::new(),
                        },
                        Transform::from_translation(muzzle_origin + shot_dir * 0.6),
                    ));
                    world.pending_fx.push(GameplayFxEvent::ShotFired {
                        muzzle_pos: muzzle_origin,
                        shot_dir,
                        source: ShotAudioSource::Creep,
                        trauma_add: 0.0,
                        trauma_max: 0.0,
                    });
                }
            }
        }
    }
}

fn step_barracks_system(
    config: &GameConfig,
    world: &mut AuthoritativeWorldState,
) {
    let turret_t3_dead: Vec<(TeamId, LaneId, bool)> = world
        .turrets
        .iter()
        .filter(|t| t.tier == 3)
        .map(|t| (t.team, t.lane, t.is_dead))
        .collect();

    let creep_snap: Vec<(TeamId, Vec3, bool, bool)> = world
        .creeps
        .iter()
        .map(|c| (c.team, c.pos, c.is_dead, c.camp_index.is_none()))
        .collect();

    let backdoor_r = config.combat.turret_backdoor_creep_radius;

    for idx in 0..world.barracks.len() {
        if world.barracks[idx].is_dead {
            world.barracks[idx].is_invulnerable = true;
            continue;
        }
        let my_team = world.barracks[idx].team;
        let my_lane = world.barracks[idx].lane;
        let my_pos = world.barracks[idx].pos;
        let b_def = &config.map.barracks[idx];

        let t3_alive = turret_t3_dead
            .iter()
            .any(|&(tm, ln, dead)| tm == my_team && ln == my_lane && !dead);

        let has_enemy_lane_creeps = if b_def.backdoor_protection_requires_creeps {
            creep_snap.iter().any(|&(c_team, c_pos, c_dead, is_lane_creep)| {
                !c_dead
                    && is_lane_creep
                    && c_team != my_team
                    && c_team != TeamId::Neutral
                    && c_pos.distance(my_pos) <= backdoor_r
            })
        } else {
            true
        };

        world.barracks[idx].is_invulnerable = t3_alive || !has_enemy_lane_creeps;
    }
}

fn step_ancients_system(
    config: &GameConfig,
    world: &mut AuthoritativeWorldState,
) {
    let turret_t3_status: Vec<(TeamId, bool)> = world
        .turrets
        .iter()
        .filter(|t| t.tier == 3)
        .map(|t| (t.team, t.is_dead))
        .collect();

    let turret_t4_status: Vec<(TeamId, bool)> = world
        .turrets
        .iter()
        .filter(|t| t.tier == 4)
        .map(|t| (t.team, t.is_dead))
        .collect();

    let creep_snap: Vec<(TeamId, Vec3, bool, bool)> = world
        .creeps
        .iter()
        .map(|c| (c.team, c.pos, c.is_dead, c.camp_index.is_none()))
        .collect();

    let backdoor_r = config.combat.turret_backdoor_creep_radius;

    for idx in 0..world.ancients.len() {
        if world.ancients[idx].is_dead {
            world.ancients[idx].is_invulnerable = true;
            continue;
        }
        let my_team = world.ancients[idx].team;
        let my_pos = world.ancients[idx].pos;
        let a_def = &config.map.ancients[idx];

        let any_t3_dead = turret_t3_status
            .iter()
            .any(|&(tm, dead)| tm == my_team && dead);

        // مطابق Dota 2: تا زمانی که هر یک از ۲ تاور Tier 4 زنده باشد، ساختمان Ancient ضدضربه است
        let any_t4_alive = turret_t4_status
            .iter()
            .any(|&(tm, dead)| tm == my_team && !dead);

        let has_enemy_lane_creeps = if a_def.backdoor_protection_requires_creeps {
            creep_snap.iter().any(|&(c_team, c_pos, c_dead, is_lane_creep)| {
                !c_dead
                    && is_lane_creep
                    && c_team != my_team
                    && c_team != TeamId::Neutral
                    && c_pos.distance(my_pos) <= backdoor_r
            })
        } else {
            true
        };

        world.ancients[idx].is_invulnerable = !any_t3_dead || any_t4_alive || !has_enemy_lane_creeps;
    }
}

impl AuthoritativeWorldState {
    pub fn pick_next_weather(&mut self, config: &GameConfig) -> (WeatherKind, f32) {
        let w = &config.weather;
        let candidates = match self.current_weather {
            WeatherKind::Clear => [
                (WeatherKind::Rainy, w.rainy_weight),
                (WeatherKind::Snowy, w.snowy_weight),
            ],
            WeatherKind::Rainy => [
                (WeatherKind::Clear, w.clear_weight),
                (WeatherKind::Snowy, w.snowy_weight),
            ],
            WeatherKind::Snowy => [
                (WeatherKind::Clear, w.clear_weight),
                (WeatherKind::Rainy, w.rainy_weight),
            ],
        };
        let total_w = (candidates[0].1 + candidates[1].1).max(0.0001);
        let next_w = if self.next_rand() * total_w <= candidates[0].1 {
            candidates[0].0
        } else {
            candidates[1].0
        };
        let (min_d, max_d) = config.weather_duration_bounds(next_w);
        let dur = min_d + (max_d - min_d).max(0.0) * self.next_rand();
        (next_w, dur)
    }
}

pub fn step_weather_and_environment_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_cfg: Res<crate::shop_shared::ItemsConfig>,
    match_state: Option<Res<AuthoritativeMatchState>>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    if match_state
        .as_ref()
        .is_some_and(|m| m.phase != MatchPhase::InGame)
    {
        return;
    }
    if world.pause_phase != PausePhase::Running || world.winning_team.is_some() {
        return;
    }

    let dt = time.delta_secs();
    if world.match_elapsed_secs < config.weather.weather_cycle_start_secs {
        world.current_weather = config.weather.initial_weather;
        world.weather_remaining_secs = 0.0;
    } else {
        world.weather_remaining_secs -= dt;
        if world.weather_remaining_secs <= 0.0 {
            let (next_w, next_dur) = world.pick_next_weather(&config);
            world.current_weather = next_w;
            world.weather_remaining_secs = next_dur;
            if next_w == WeatherKind::Rainy {
                let min_l = config.weather.lightning_min_interval_secs;
                let max_l = config.weather.lightning_max_interval_secs;
                world.lightning_cooldown_secs =
                    min_l + (max_l - min_l).max(0.0) * world.next_rand();
            }
        }

        if world.current_weather == WeatherKind::Rainy {
            world.lightning_cooldown_secs -= dt;
            if world.lightning_cooldown_secs <= 0.0 {
                let intensity = 0.85 + 0.30 * world.next_rand();
                world.pending_fx.push(GameplayFxEvent::LightningStrike { intensity });
                let min_l = config.weather.lightning_min_interval_secs;
                let max_l = config.weather.lightning_max_interval_secs;
                world.lightning_cooldown_secs =
                    min_l + (max_l - min_l).max(0.0) * world.next_rand();
            }
        }
    }

    let is_night = config.is_night_at(world.match_elapsed_secs);
    let cur_weather = world.current_weather;
    let first_init = !world.env_stats_initialized && !world.players.is_empty();
    if first_init || is_night != world.last_env_is_night || cur_weather != world.last_env_weather {
        world.env_stats_initialized = true;
        world.last_env_is_night = is_night;
        world.last_env_weather = cur_weather;
        for p in &mut world.players {
            p.active_is_night = is_night;
            p.active_weather = cur_weather;
            crate::shop_shared::recompute_player_slot_stats(p, &config, &items_cfg);
            if first_init && !p.is_dead {
                p.hero_hp = p.hero_max_hp;
                p.hero_stamina = p.hero_max_stamina;
            p.stamina_exhausted = false;
                p.beam_charge = p.hero_weapon.beam_max_charge;
            }
        }
    }
}