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

// -----------------------------------------------------------------------------
// ۱. ساختارهای کانفیگ (بدون هیچ Default یا serde(default))
// -----------------------------------------------------------------------------

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct GameConfig {
    pub network: NetworkConfig,
    pub window: WindowConfig,
    pub camera: CameraConfig,
    pub lighting: LightingConfig,
    pub weapon_tuner: WeaponTunerConfig,
    pub physics: PhysicsConfig,
    pub soldier: SoldierConfig,
    pub combat: CombatConfig,
    pub fx: FxConfig,
    pub animations: AnimationConfig,
    pub map: MapConfig,
    pub obstacles: Vec<ObstacleDef>,
    pub targets: Vec<TargetDef>,
}

impl GameConfig {
    pub fn load_from_file(path: &str) -> Self {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("خطای بحرانی: فایل کانفیگ '{}' خوانده نشد: {}", path, e));
        let cfg = ron::from_str::<GameConfig>(&content).unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی: ساختار فایل کانفیگ '{}' نامعتبر است: {}",
                path, e
            )
        });
        cfg.validate_unique_map_ids();
        cfg
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
        check_id(&self.map.center_bridge.id, "map.center_bridge");
        for item in &self.map.fountains {
            check_id(&item.id, "map.fountains");
        }
        for item in &self.map.ancients {
            check_id(&item.id, "map.ancients");
        }
        for item in &self.map.turrets {
            check_id(&item.id, "map.turrets");
        }
        for item in &self.map.mountains {
            check_id(&item.id, "map.mountains");
        }
        for item in &self.map.trees {
            check_id(&item.id, "map.trees");
        }
        for item in &self.map.lanes {
            check_id(&item.id, "map.lanes");
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkConfig {
    pub server_bind_addr: String,
    pub client_bind_addr: String,
    pub server_tick_rate_hz: f64,
    pub protocol_id: u64,
    pub max_clients: usize,
    pub client_id: u64,
    pub connect_token_expire_secs: u64,
    pub connect_token_timeout_secs: i32,
    pub available_bytes_per_tick: u64,
    pub channel_max_memory_bytes: usize,
    pub reliable_resend_time_ms: u64,
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

    let server_config = ServerConfig {
        current_time,
        max_clients: net.max_clients,
        protocol_id: net.protocol_id,
        public_addresses: vec![server_addr],
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
    let server_addr: SocketAddr = net.server_bind_addr.parse().unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی کلاینت: آدرس سرور '{}' معتبر نیست: {}",
            net.server_bind_addr, e
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LightingConfig {
    pub directional_illuminance: f32,
    pub directional_pos: (f32, f32, f32),
    pub directional_shadow_depth_bias: f32,
    pub directional_shadow_normal_bias: f32,
    pub ambient_color_rgb: (f32, f32, f32),
    pub ambient_brightness: f32,
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SoldierConfig {
    pub spawn_pos: (f32, f32, f32),
    pub initial_aim_dir: (f32, f32, f32),
    pub initial_aim_hit_pos: (f32, f32, f32),
    pub aim_plane_height_offset: f32,
    pub player_radius: f32,
    pub run_speed: f32,
    pub walk_speed: f32,
    pub crouch_speed: f32,
    pub running_slide_speed: f32,
    pub dive_roll_speed: f32,
    pub quick_roll_speed: f32,
    pub vault_check_distance: f32,
    pub vault_travel_distance: f32,
    pub vault_ray_height: f32,
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
    pub grenade_fuse_secs: f32,
    pub grenade_target_kill_radius: f32,
    pub grenade_soldier_kill_radius: f32,
    pub grenade_explosion_trauma_add: f32,
    pub grenade_explosion_trauma_max: f32,
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
pub struct MapConfig {
    pub map_dimensions: (f32, f32),
    pub base_vision_range: f32,
    pub high_ground_vision_bonus: f32,
    pub high_ground_occlusion_min_diff: f32,
    pub terrains: Vec<TerrainPlatformDef>,
    pub ramps: Vec<RampDef>,
    pub river_segments: Vec<RiverSegmentDef>,
    pub waterfalls: Vec<WaterfallDef>,
    pub center_bridge: CenterBridgeDef,
    pub fountains: Vec<FountainDef>,
    pub ancients: Vec<AncientDef>,
    pub turrets: Vec<TurretDef>,
    pub mountains: Vec<MountainDef>,
    pub trees: Vec<TreeDef>,
    pub lanes: Vec<LanePathDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TerrainPlatformDef {
    pub id: String,
    pub model_path: String,
    pub team: TeamId,
    pub elevation_tier: i32,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RampDef {
    pub id: String,
    pub model_path: String,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub rotation_euler_deg: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RiverSegmentDef {
    pub id: String,
    pub model_path: String,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub yaw_deg: f32,
    pub flow_dir: (f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub emissive_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaterfallDef {
    pub id: String,
    pub model_path: String,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub yaw_deg: f32,
    pub water_color_rgb: (f32, f32, f32),
    pub foam_emissive_rgb: (f32, f32, f32),
    pub mist_particle_interval_secs: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CenterBridgeDef {
    pub id: String,
    pub model_path: String,
    pub center_pos: (f32, f32, f32),
    pub outer_radius: f32,
    pub deck_thickness: f32,
    pub pillar_radius: f32,
    pub pillar_height: f32,
    pub elevation_tier: i32,
    pub color_rgb: (f32, f32, f32),
    pub accent_color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FountainDef {
    pub id: String,
    pub model_path: String,
    pub team: TeamId,
    pub pos: (f32, f32, f32),
    pub outer_radius: f32,
    pub inner_radius: f32,
    pub rim_height: f32,
    pub heal_radius: f32,
    pub heal_per_sec: f32,
    pub stone_color_rgb: (f32, f32, f32),
    pub water_emissive_rgb: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AncientDef {
    pub id: String,
    pub model_path: String,
    pub team: TeamId,
    pub pos: (f32, f32, f32),
    pub base_size: (f32, f32, f32),
    pub core_radius: f32,
    pub core_height: f32,
    pub max_hp: f32,
    pub color_rgb: (f32, f32, f32),
    pub core_emissive_rgb: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TurretDef {
    pub id: String,
    pub model_path: String,
    pub team: TeamId,
    pub lane: LaneId,
    pub tier: u8,
    pub pos: (f32, f32, f32),
    pub base_radius: f32,
    pub height: f32,
    pub head_radius: f32,
    pub max_hp: f32,
    pub attack_range: f32,
    pub color_rgb: (f32, f32, f32),
    pub crystal_emissive_rgb: (f32, f32, f32),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MountainDef {
    pub id: String,
    pub model_path: String,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub yaw_deg: f32,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TreeDef {
    pub id: String,
    pub model_path: String,
    pub tree_size: TreeSize,
    pub pos: (f32, f32, f32),
    pub trunk_radius: f32,
    pub trunk_height: f32,
    pub canopy_radius: f32,
    pub canopy_height: f32,
    pub trunk_color_rgb: (f32, f32, f32),
    pub canopy_color_rgb: (f32, f32, f32),
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ObstacleDef {
    pub id: String,
    pub model_path: String,
    pub kind: CoverKind,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub yaw_deg: f32,
    pub can_vault: bool,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetDef {
    pub id: String,
    pub model_path: String,
    pub pos: (f32, f32, f32),
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
    RunningDiveRoll,
    QuickRollToRun,
    ObstacleVault,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClientInputCommand {
    pub move_input: Vec3,
    pub shift_held: bool,
    pub toggle_crouch_just_pressed: bool,
    pub reload_just_pressed: bool,
    pub grenade_just_pressed: bool,
    pub space_just_pressed: bool,
    pub kill_toggle_just_pressed: bool,
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
        self.aim_ray_origin = newer.aim_ray_origin;
        self.aim_ray_dir = newer.aim_ray_dir;
        self.has_aim_ray = newer.has_aim_ray;
        self.client_muzzle_world_pos = newer.client_muzzle_world_pos;

        self.toggle_crouch_just_pressed |= newer.toggle_crouch_just_pressed;
        self.reload_just_pressed |= newer.reload_just_pressed;
        self.grenade_just_pressed |= newer.grenade_just_pressed;
        self.space_just_pressed |= newer.space_just_pressed;
        self.kill_toggle_just_pressed |= newer.kill_toggle_just_pressed;
        self.fire_just_pressed |= newer.fire_just_pressed;
    }

    pub fn consume_just_pressed(&mut self) {
        self.toggle_crouch_just_pressed = false;
        self.reload_just_pressed = false;
        self.grenade_just_pressed = false;
        self.space_just_pressed = false;
        self.kill_toggle_just_pressed = false;
        self.fire_just_pressed = false;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum GameplayFxEvent {
    ShotFired {
        muzzle_pos: Vec3,
        shot_dir: Vec3,
        trauma_add: f32,
        trauma_max: f32,
    },
    BulletTrailSegment {
        start: Vec3,
        end: Vec3,
    },
    BulletImpact {
        impact_pos: Vec3,
        bullet_dir: Vec3,
        target_destroyed: bool,
        trauma_add: f32,
        trauma_max: f32,
    },
    GrenadeExploded {
        pos: Vec3,
        trauma_add: f32,
        trauma_max: f32,
    },
    SlideDust {
        pos: Vec3,
    },
    WaterSplash {
        pos: Vec3,
        intensity: f32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BulletSnapshot {
    pub id: u64,
    pub translation: Vec3,
    pub velocity: Vec3,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GrenadeSnapshot {
    pub id: u64,
    pub translation: Vec3,
    pub rotation: Quat,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetSnapshot {
    pub index: usize,
    pub hp: f32,
    pub max_hp: f32,
    pub hit_flash_timer: f32,
    pub is_dead: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerWorldSnapshot {
    pub tick: u64,
    pub soldier_pos: Vec3,
    pub soldier_facing_dir: Vec3,
    pub aim_world_dir: Vec3,
    pub aim_hit_pos: Vec3,
    pub laser_hit_pos: Vec3,
    pub current_locomotion: String,
    pub upper_action: UpperBodyAction,
    pub traversal_active: bool,
    pub traversal_anim_name: String,
    pub is_dead: bool,
    pub death_anim_name: String,
    pub in_water: bool,
    pub bullets: Vec<BulletSnapshot>,
    pub grenades: Vec<GrenadeSnapshot>,
    pub targets: Vec<TargetSnapshot>,
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

/// بررسی قرار گرفتن یک نقطه ۳بعدی درون برکه‌ها و رودخانه‌های نقشه و برگرداندن ارتفاع سطح آب و بردار جریان
pub fn sample_water_surface_at(config: &GameConfig, world_pos: Vec3) -> Option<(f32, Vec2)> {
    for seg in &config.map.river_segments {
        let seg_pos = Vec3::new(seg.pos.0, seg.pos.1, seg.pos.2);
        let half = Vec3::new(seg.size.0 * 0.5, seg.size.1 * 0.5, seg.size.2 * 0.5);
        let water_top_y = seg_pos.y + half.y;

        if world_pos.y < water_top_y - 1.35 || world_pos.y > water_top_y + 0.45 {
            continue;
        }

        let inv_rot = Quat::from_rotation_y(-seg.yaw_deg.to_radians());
        let local = inv_rot * (world_pos - seg_pos);
        // استفاده از شکل بیضوی/نرم در صفحه افقی برای طبیعی‌تر شدن مرز برکه‌ها
        let nx = local.x / half.x.max(0.1);
        let nz = local.z / half.z.max(0.1);
        if nx * nx + nz * nz <= 1.05 {
            let flow = Vec2::new(seg.flow_dir.0, seg.flow_dir.1).normalize_or_zero();
            return Some((water_top_y, flow));
        }
    }
    None
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
}

/// دنیای فیزیک اختصاصی که لیست کولایدرهای ثابت فعال را برای پرتوافکنی و برخوردها نگه می‌دارد
#[derive(Resource, Default, Clone, Debug)]
pub struct CustomPhysicsWorld {
    pub static_colliders: Vec<StaticColliderEntry>,
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
                    closest_toi = hit.toi;
                    best_hit = Some((entry.entity, hit));
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
                        let step_origin = Vec3::new(
                            center.x,
                            feet_y + autostep_max_h + 0.15,
                            center.z,
                        );
                        let mut stepped_up = false;
                        if let Some(top_hit) = ray_intersect_single_entry(
                            step_origin,
                            Vec3::NEG_Y,
                            autostep_max_h + 0.20,
                            entry,
                        ) {
                            if top_hit.normal.y >= max_slope_cos
                                && top_hit.point.y >= feet_y - 0.02
                                && top_hit.point.y <= feet_y + autostep_max_h
                            {
                                center.y = top_hit.point.y + center_y_offset;
                                stepped_up = true;
                            }
                        }

                        if !stepped_up {
                            let horiz_normal =
                                Vec3::new(wall_hit.normal.x, 0.0, wall_hit.normal.z)
                                    .normalize_or_zero();
                            if horiz_normal != Vec3::ZERO {
                                center += horiz_normal * (wall_hit.penetration + nudge);
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

        for offset in &footprint_offsets {
            let ray_origin = Vec3::new(
                center.x + offset.x,
                feet_y + ray_start_offset,
                center.z + offset.y,
            );
            for entry in &self.static_colliders {
                if entry.disabled || !soldier_groups.interacts_with(entry.groups) {
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

        // ۳. اعمال گرانش و چسبیدن نرم به سطح رمپ، زمین یا کف برکه‌ها
        let mut grounded = false;
        let new_feet_y = if let Some(ground_y) = highest_walkable_y {
            let free_fall_feet_y = feet_y + desired_delta.y;
            if free_fall_feet_y <= ground_y + snap_dist {
                grounded = true;
                ground_y
            } else {
                free_fall_feet_y
            }
        } else {
            (feet_y + desired_delta.y).max(-1.6)
        };

        center.y = new_feet_y + center_y_offset;

        // ۴. اصلاح نهایی نفوذ افقی و عمودی در ارتفاع جدید
        for entry in &self.static_colliders {
            if entry.disabled || !soldier_groups.interacts_with(entry.groups) {
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
                        if oy <= -half_seg * 0.5 && contact.penetration > 0.0 {
                            center.y += contact.normal.y * contact.penetration;
                            grounded = true;
                        }
                    } else {
                        let horiz_normal = Vec3::new(contact.normal.x, 0.0, contact.normal.z)
                            .normalize_or_zero();
                        if horiz_normal != Vec3::ZERO {
                            center += horiz_normal * (contact.penetration + nudge);
                        }
                    }
                }
            }
        }

        (center, grounded)
    }
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

#[derive(Component)]
pub struct PhysicsObstacle {
    pub id: String,
    pub can_vault: bool,
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
    pub team: TeamId,
    pub lane: LaneId,
    pub tier: u8,
}

#[derive(Component)]
pub struct PhysicsAncient {
    pub id: String,
    pub team: TeamId,
}

#[derive(Component)]
pub struct PhysicsFountain {
    pub id: String,
    pub team: TeamId,
}

#[derive(Component)]
pub struct PhysicsSoldier;

#[derive(Component)]
pub struct PhysicsGrenade {
    pub id: u64,
    pub remaining_fuse: f32,
}

#[derive(Component)]
pub struct PhysicsBullet {
    pub id: u64,
    pub velocity: Vec3,
    pub remaining_lifetime: f32,
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
    let pos = Vec3::new(plat.pos.0, plat.pos.1, plat.pos.2);
    let half = Vec3::new(plat.size.0 * 0.5, plat.size.1 * 0.5, plat.size.2 * 0.5);
    (
        MapObjectId(plat.id.clone()),
        ElevationSurface {
            elevation_tier: plat.elevation_tier,
            top_y: pos.y + half.y,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        Transform::from_translation(pos),
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
    let pos = Vec3::new(ramp.pos.0, ramp.pos.1, ramp.pos.2);
    let half = Vec3::new(ramp.size.0 * 0.5, ramp.size.1 * 0.5, ramp.size.2 * 0.5);
    let rot = Quat::from_euler(
        EulerRot::YXZ,
        ramp.rotation_euler_deg.1.to_radians(),
        ramp.rotation_euler_deg.0.to_radians(),
        ramp.rotation_euler_deg.2.to_radians(),
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
        Transform {
            translation: pos,
            rotation: rot,
            scale: Vec3::ONE,
        },
    )
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
    let pos = Vec3::new(
        bridge.center_pos.0,
        bridge.center_pos.1,
        bridge.center_pos.2,
    );
    let half_h = bridge.deck_thickness * 0.5;
    (
        MapObjectId(bridge.id.clone()),
        ElevationSurface {
            elevation_tier: bridge.elevation_tier,
            top_y: pos.y + half_h,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(half_h, bridge.outer_radius),
        SurfaceMaterial {
            friction: config.physics.ground_friction,
            restitution: config.physics.ground_restitution,
        },
        world_collision_groups(),
        Transform::from_translation(pos),
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
    let pos = Vec3::new(mtn.pos.0, mtn.pos.1, mtn.pos.2);
    let half = Vec3::new(mtn.size.0 * 0.5, mtn.size.1 * 0.5, mtn.size.2 * 0.5);
    let rot = Quat::from_rotation_y(mtn.yaw_deg.to_radians());
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
        Transform {
            translation: pos,
            rotation: rot,
            scale: Vec3::ONE,
        },
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
    let half_h = tree.trunk_height * 0.5;
    let pos = Vec3::new(tree.pos.0, tree.pos.1 + half_h, tree.pos.2);
    (
        MapObjectId(tree.id.clone()),
        PhysicsTree {
            id: tree.id.clone(),
        },
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(half_h, tree.trunk_radius),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        Transform::from_translation(pos),
    )
}

pub fn turret_physics_bundle(
    config: &GameConfig,
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
    let half_h = turret.height * 0.5;
    let pos = Vec3::new(turret.pos.0, turret.pos.1 + half_h, turret.pos.2);
    (
        MapObjectId(turret.id.clone()),
        PhysicsTurret {
            id: turret.id.clone(),
            team: turret.team,
            lane: turret.lane,
            tier: turret.tier,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cylinder(half_h, turret.base_radius),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        Transform::from_translation(pos),
    )
}

pub fn ancient_physics_bundle(
    config: &GameConfig,
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
    let half = Vec3::new(
        ancient.base_size.0 * 0.5,
        ancient.base_size.1 * 0.5,
        ancient.base_size.2 * 0.5,
    );
    let pos = Vec3::new(ancient.pos.0, ancient.pos.1 + half.y, ancient.pos.2);
    (
        MapObjectId(ancient.id.clone()),
        PhysicsAncient {
            id: ancient.id.clone(),
            team: ancient.team,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        Transform::from_translation(pos),
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
    let pos = Vec3::new(obs.pos.0, obs.pos.1, obs.pos.2);
    let half = Vec3::new(obs.size.0 * 0.5, obs.size.1 * 0.5, obs.size.2 * 0.5);
    let rot = Quat::from_rotation_y(obs.yaw_deg.to_radians());
    (
        MapObjectId(obs.id.clone()),
        PhysicsObstacle {
            id: obs.id.clone(),
            can_vault: obs.can_vault,
        },
        CustomRigidBody::Fixed,
        CustomCollider::cuboid(half.x, half.y, half.z),
        SurfaceMaterial {
            friction: config.physics.obstacle_friction,
            restitution: config.physics.obstacle_restitution,
        },
        world_collision_groups(),
        Transform {
            translation: pos,
            rotation: rot,
            scale: Vec3::ONE,
        },
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
    let pos = Vec3::new(target.pos.0, target.pos.1, target.pos.2);
    let half = Vec3::new(
        target.size.0 * 0.5,
        target.size.1 * 0.5,
        target.size.2 * 0.5,
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
        Transform::from_translation(pos),
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
    let spawn_pos = Vec3::new(
        config.soldier.spawn_pos.0,
        config.soldier.spawn_pos.1 + config.physics.soldier_capsule_center_y,
        config.soldier.spawn_pos.2,
    );
    (
        PhysicsSoldier,
        CustomRigidBody::KinematicPositionBased,
        CustomCollider::capsule_y(
            config.physics.soldier_capsule_half_segment,
            config.soldier.player_radius,
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
            remaining_fuse: config.combat.grenade_fuse_secs,
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

/// قانون برتری دید ارتفاعی (Dota 2 High-Ground Vision Rule) با استفاده از موتور فیزیک اختصاصی
pub fn can_observer_see_point(
    physics_world: &CustomPhysicsWorld,
    config: &GameConfig,
    observer_feet_pos: Vec3,
    target_feet_pos: Vec3,
    eye_height: f32,
) -> bool {
    let height_diff = target_feet_pos.y - observer_feet_pos.y;
    if height_diff >= config.map.high_ground_occlusion_min_diff {
        return false;
    }

    let effective_range = if observer_feet_pos.y - target_feet_pos.y
        >= config.map.high_ground_occlusion_min_diff
    {
        config.map.base_vision_range + config.map.high_ground_vision_bonus
    } else {
        config.map.base_vision_range
    };

    let eye_origin = observer_feet_pos + Vec3::Y * eye_height;
    let target_point = target_feet_pos + Vec3::Y * eye_height;
    let to_target = target_point - eye_origin;
    let dist = to_target.length();
    if dist > effective_range {
        return false;
    }
    if dist <= f32::EPSILON {
        return true;
    }

    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);
    physics_world
        .cast_ray(eye_origin, to_target / dist, dist, query_groups, |_| true)
        .is_none()
}

// -----------------------------------------------------------------------------
// ۵. وضعیت کامل شبیه‌سازی (درون moba_shared برای جلوگیری از بازنویسی)
// -----------------------------------------------------------------------------

pub struct SharedTarget {
    pub max_hp: f32,
    pub hp: f32,
    pub hit_flash_timer: f32,
    pub base_pos: Vec3,
    pub is_dead: bool,
    pub respawn_remaining: f32,
    pub respawn_duration: f32,
}

pub struct SharedTraversalState {
    pub is_active: bool,
    pub kind: TraversalKind,
    pub anim_name: String,
    pub elapsed: f32,
    pub duration: f32,
    pub start_pos: Vec3,
    pub target_pos: Vec3,
    pub travel_dir: Vec3,
}

#[derive(Resource)]
pub struct ClientPhysicsPredictionState {
    pub latest_input: Option<ClientInputCommand>,
    pub traversal_active: bool,
    pub effective_stance: Stance,
    pub is_dead: bool,
}

impl ClientPhysicsPredictionState {
    pub fn new() -> Self {
        Self {
            latest_input: None,
            traversal_active: false,
            effective_stance: Stance::Run,
            is_dead: false,
        }
    }
}

#[derive(Resource)]
pub struct AuthoritativeWorldState {
    pub tick: u64,
    pub rng_seed: u32,
    pub next_entity_id: u64,
    pub active_input: Option<ClientInputCommand>,
    pub soldier_pos: Vec3,
    pub soldier_facing_dir: Vec3,
    pub base_stance: Stance,
    pub effective_stance: Stance,
    pub current_locomotion: String,
    pub aim_world_dir: Vec3,
    pub aim_hit_pos: Vec3,
    pub laser_hit_pos: Vec3,
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
    pub in_water: bool,
    pub water_splash_cooldown: f32,
    pub traversal: SharedTraversalState,
    pub targets: Vec<SharedTarget>,
    pub bullets_snapshot: Vec<BulletSnapshot>,
    pub grenades_snapshot: Vec<GrenadeSnapshot>,
    pub pending_fx: Vec<GameplayFxEvent>,
}

impl AuthoritativeWorldState {
    pub fn new_from_config(config: &GameConfig) -> Self {
        let mut targets = Vec::new();
        for t in &config.targets {
            let pos = Vec3::new(t.pos.0, t.pos.1, t.pos.2);
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
            in_water: false,
            water_splash_cooldown: 0.0,
            traversal: SharedTraversalState {
                is_active: false,
                kind: TraversalKind::RunningJump,
                anim_name: String::new(),
                elapsed: 0.0,
                duration: 0.0,
                start_pos: spawn_pos,
                target_pos: spawn_pos,
                travel_dir: initial_aim_dir,
            },
            targets,
            bullets_snapshot: Vec::new(),
            grenades_snapshot: Vec::new(),
            pending_fx: Vec::new(),
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

    pub fn build_snapshot(&mut self) -> ServerWorldSnapshot {
        self.tick += 1;
        ServerWorldSnapshot {
            tick: self.tick,
            soldier_pos: self.soldier_pos,
            soldier_facing_dir: self.soldier_facing_dir,
            aim_world_dir: self.aim_world_dir,
            aim_hit_pos: self.aim_hit_pos,
            laser_hit_pos: self.laser_hit_pos,
            current_locomotion: self.current_locomotion.clone(),
            upper_action: self.upper_body_action(),
            traversal_active: self.traversal.is_active,
            traversal_anim_name: self.traversal.anim_name.clone(),
            is_dead: self.is_dead,
            death_anim_name: self.death_anim_name.clone(),
            in_water: self.in_water,
            bullets: self.bullets_snapshot.clone(),
            grenades: self.grenades_snapshot.clone(),
            targets: self
                .targets
                .iter()
                .enumerate()
                .map(|(index, t)| TargetSnapshot {
                    index,
                    hp: t.hp,
                    max_hp: t.max_hp,
                    hit_flash_timer: t.hit_flash_timer,
                    is_dead: t.is_dead,
                })
                .collect(),
            fx_events: Vec::new(),
        }
    }

    pub fn take_fx_events(&mut self) -> Vec<GameplayFxEvent> {
        std::mem::take(&mut self.pending_fx)
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
    config: &GameConfig,
) {
    let auth_capsule_pos = Vec3::new(
        authoritative_feet_pos.x,
        authoritative_feet_pos.y + config.physics.soldier_capsule_center_y,
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

fn trigger_random_death(world: &mut AuthoritativeWorldState, config: &GameConfig) {
    let count = config.soldier.death_animations.len();
    assert!(count > 0, "لیست death_animations در کانفیگ خالی است!");
    let idx = ((world.next_rand() * count as f32) as usize).min(count - 1);
    let chosen = config.soldier.death_animations[idx].clone();
    let duration = config.exact_anim_duration(&chosen);

    world.is_dead = true;
    world.death_anim_name = chosen;
    world.respawn_remaining = duration + config.soldier.death_extra_respawn_delay_secs;
    world.reload_remaining = None;
    world.grenade_elapsed_and_duration = None;
    world.firing_anim_remaining = None;
}

fn resolve_muzzle_pos(
    world: &AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
) -> Vec3 {
    if let Some(muzzle_pos) = input.client_muzzle_world_pos {
        muzzle_pos
    } else {
        world.soldier_pos
            + Vec3::new(0.0, config.combat.fallback_muzzle_offset_y, 0.0)
            + world.aim_world_dir * config.combat.fallback_muzzle_forward
    }
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

    for lane in &config.map.lanes {
        for seg_idx in 0..lane.waypoints.len().saturating_sub(1) {
            let a = lane.waypoints[seg_idx];
            let b = lane.waypoints[seg_idx + 1];
            let start = Vec3::new(a.0, a.1, a.2);
            let end = Vec3::new(b.0, b.1, b.2);
            if start.distance(end) > 0.1 {
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

    for mtn in &config.map.mountains {
        commands.spawn(mountain_physics_bundle(&config, mtn));
    }

    for tree in &config.map.trees {
        commands.spawn(tree_physics_bundle(&config, tree));
    }

    for turret in &config.map.turrets {
        commands.spawn(turret_physics_bundle(&config, turret));
    }

    for ancient in &config.map.ancients {
        commands.spawn(ancient_physics_bundle(&config, ancient));
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
    mut physics_world: ResMut<CustomPhysicsWorld>,
    static_query: Query<(
        Entity,
        &Transform,
        &CustomRigidBody,
        &CustomCollider,
        &SurfaceMaterial,
        &CollisionGroups,
        Has<ColliderDisabled>,
    )>,
) {
    physics_world.static_colliders.clear();
    for (entity, tf, body, collider, mat, groups, disabled) in &static_query {
        if *body != CustomRigidBody::Fixed {
            continue;
        }
        let rot = tf.rotation.normalize();
        physics_world.static_colliders.push(StaticColliderEntry {
            entity,
            translation: tf.translation,
            rotation: rot,
            inv_rotation: rot.inverse(),
            collider: *collider,
            bounding_radius: collider.bounding_sphere_radius(),
            material: *mat,
            groups: *groups,
            disabled,
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
    physics_world: Res<CustomPhysicsWorld>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut soldier_query: Query<
        (&mut Transform, &mut CustomCharacterController),
        With<PhysicsSoldier>,
    >,
    obstacle_query: Query<&PhysicsObstacle>,
    target_query: Query<&PhysicsTarget>,
) {
    let Some(input) = world.active_input.clone() else {
        return;
    };

    let dt = time.delta_secs();
    let (mut soldier_tf, mut controller) = soldier_query
        .single_mut()
        .expect("خطای بحرانی: موجودیت PhysicsSoldier یافت نشد");

    // ۱. بررسی کلید X (تست مرگ) و تایمر زنده شدن مجدد در چشمه پایگاه
    if input.kill_toggle_just_pressed {
        if world.is_dead {
            world.is_dead = false;
            world.base_stance = Stance::Run;
        } else {
            world.traversal.is_active = false;
            trigger_random_death(&mut world, &config);
        }
    }

    if world.is_dead {
        controller.translation = None;
        world.respawn_remaining -= dt;
        if world.respawn_remaining <= 0.0 {
            world.is_dead = false;
            world.base_stance = Stance::Run;
            let spawn_pos = Vec3::new(
                config.soldier.spawn_pos.0,
                config.soldier.spawn_pos.1,
                config.soldier.spawn_pos.2,
            );
            world.soldier_pos = spawn_pos;
            soldier_tf.translation =
                spawn_pos + Vec3::Y * config.physics.soldier_capsule_center_y;
        }
    } else {
        // ۲. آپدیت تایمرهای بالاتنه
        if let Some(rem) = &mut world.reload_remaining {
            *rem -= dt;
            if *rem <= 0.0 {
                world.reload_remaining = None;
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
        {
            let dur = config.exact_anim_duration("reloading");
            world.reload_remaining = Some(dur);
            world.firing_anim_remaining = None;
        }

        if input.grenade_just_pressed
            && world.grenade_elapsed_and_duration.is_none()
            && !world.traversal.is_active
        {
            let dur = config.exact_anim_duration("toss_grenade");
            world.grenade_elapsed_and_duration = Some((0.0, dur));
            world.grenade_spawned_for_throw = false;
            world.firing_anim_remaining = None;
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

        // ۵. محاسبه جهت هدف‌گیری با ماوس (Aim Ray) با در نظر گرفتن ارتفاع فعلی کاراکتر
        if !world.traversal.is_active
            && input.has_aim_ray
            && input.aim_ray_dir.y.abs() > config.physics.aim_ray_min_dir_y_epsilon
        {
            let aim_plane_y = world.soldier_pos.y + config.soldier.aim_plane_height_offset;
            let t = (aim_plane_y - input.aim_ray_origin.y) / input.aim_ray_dir.y;
            if t > 0.0 {
                let hit = input.aim_ray_origin + input.aim_ray_dir * t;
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
        let flat_input = Vec3::new(input.move_input.x, 0.0, input.move_input.z);
        world.is_moving = flat_input != Vec3::ZERO;
        world.move_input = flat_input;

        if world.is_moving {
            let move_norm = world.move_input.normalize();
            let forward = world.aim_world_dir;
            let right = forward.cross(Vec3::Y).normalize();
            let local_forward = move_norm.dot(forward);
            let local_right = move_norm.dot(right);
            let angle_deg = local_right.atan2(local_forward).to_degrees();
            let dir_suffix = angle_to_direction_8(angle_deg);

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

        // ۷. سیستم پرش، غلت و عبور از مانع با پرتو و کپسول فیزیک اختصاصی
        step_traversal_custom(
            &mut world,
            &input,
            &config,
            &physics_world,
            &obstacle_query,
            &target_query,
            &mut soldier_tf,
            &mut controller,
            dt,
        );

        // ۸. جابجایی فیزیکی ۳بعدی کاراکتر همراه با مقاومت طبیعی آب و رانش جریان برکه‌ها
        let water_sample = sample_water_surface_at(&config, world.soldier_pos);
        let was_in_water = world.in_water;
        world.in_water = water_sample.is_some();
        world.water_splash_cooldown = (world.water_splash_cooldown - dt).max(0.0);

        if let Some((water_y, flow_dir)) = water_sample {
            // استخراج نقطه پاشش قبل از push کردن برای رفع خطای Borrow Checker
            let splash_pos = Vec3::new(world.soldier_pos.x, water_y, world.soldier_pos.z);

            if !was_in_water {
                world.pending_fx.push(GameplayFxEvent::WaterSplash {
                    pos: splash_pos,
                    intensity: 1.35,
                });
                world.water_splash_cooldown = config.physics.water_splash_interval_secs;
            } else if world.is_moving && world.water_splash_cooldown <= 0.0 {
                world.pending_fx.push(GameplayFxEvent::WaterSplash {
                    pos: splash_pos,
                    intensity: 0.85,
                });
                world.water_splash_cooldown = config.physics.water_splash_interval_secs;
            }

            if !world.traversal.is_active {
                let grav_y = config.physics.character_gravity_step_y * dt;
                let flow_drift = Vec3::new(flow_dir.x, 0.0, flow_dir.y) * 0.35 * dt;
                if world.is_moving {
                    let base_speed = match world.effective_stance {
                        Stance::Run => config.soldier.run_speed,
                        Stance::Walk => config.soldier.walk_speed,
                        Stance::Crouch => config.soldier.crouch_speed,
                    };
                    let water_speed = base_speed * config.physics.water_move_speed_multiplier;
                    let desired_delta =
                        world.move_input.normalize() * water_speed * dt + flow_drift;
                    controller.translation =
                        Some(Vec3::new(desired_delta.x, grav_y, desired_delta.z));
                } else {
                    controller.translation = Some(Vec3::new(flow_drift.x, grav_y, flow_drift.z));
                }
            }
        } else if !world.traversal.is_active {
            let grav_y = config.physics.character_gravity_step_y * dt;
            if world.is_moving {
                let speed = match world.effective_stance {
                    Stance::Run => config.soldier.run_speed,
                    Stance::Walk => config.soldier.walk_speed,
                    Stance::Crouch => config.soldier.crouch_speed,
                };
                let desired_delta = world.move_input.normalize() * speed * dt;
                controller.translation = Some(Vec3::new(desired_delta.x, grav_y, desired_delta.z));
            } else {
                controller.translation = Some(Vec3::new(0.0, grav_y, 0.0));
            }
        }
    }

    // ۹. شلیک گلوله و پرتاب نارنجک
    step_shooting_and_grenade_spawn(&mut commands, &mut world, &input, &config, dt);
}

#[allow(clippy::too_many_arguments)]
fn step_traversal_custom(
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    obstacle_query: &Query<&PhysicsObstacle>,
    target_query: &Query<&PhysicsTarget>,
    soldier_tf: &mut Transform,
    controller: &mut CustomCharacterController,
    dt: f32,
) {
    if world.traversal.is_active {
        world.traversal.elapsed += dt;
        let frac = (world.traversal.elapsed / world.traversal.duration).clamp(0.0, 1.0);
        let current_pos = world
            .traversal
            .start_pos
            .lerp(world.traversal.target_pos, frac);
        world.soldier_facing_dir = world.traversal.travel_dir;

        if world.traversal.kind == TraversalKind::ObstacleVault {
            controller.translation = None;
            soldier_tf.translation = Vec3::new(
                current_pos.x,
                current_pos.y + config.physics.soldier_capsule_center_y,
                current_pos.z,
            );
            world.soldier_pos = current_pos;
        } else {
            let step_delta = current_pos - world.soldier_pos;
            let grav_y = config.physics.character_gravity_step_y * dt;
            controller.translation = Some(Vec3::new(step_delta.x, grav_y, step_delta.z));
        }

        if world.traversal.kind == TraversalKind::RunningJump {
            let current_soldier_pos = world.soldier_pos;
            if let Some((water_y, _)) = sample_water_surface_at(config, current_soldier_pos) {
                let splash_pos = Vec3::new(current_soldier_pos.x, water_y, current_soldier_pos.z);
                world.pending_fx.push(GameplayFxEvent::WaterSplash {
                    pos: splash_pos,
                    intensity: 1.15,
                });
            } else {
                let dust_pos = current_soldier_pos - world.traversal.travel_dir * 0.4
                    + Vec3::new(0.0, 0.08, 0.0);
                world
                    .pending_fx
                    .push(GameplayFxEvent::SlideDust { pos: dust_pos });
            }
        }

        if world.traversal.elapsed >= world.traversal.duration {
            world.traversal.is_active = false;
            controller.translation = None;
            if world.traversal.kind == TraversalKind::QuickRollToRun {
                world.base_stance = Stance::Run;
                world.effective_stance = Stance::Run;
            }
        }
        return;
    }

    if !input.space_just_pressed {
        return;
    }

    let travel_dir = if world.is_moving {
        world.move_input.normalize()
    } else {
        world.aim_world_dir
    };

    let ray_origin = world.soldier_pos + Vec3::Y * config.soldier.vault_ray_height;
    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    let crossing_obstacle = physics_world
        .cast_ray(
            ray_origin,
            travel_dir,
            config.soldier.vault_check_distance,
            query_groups,
            |entity| {
                obstacle_query
                    .get(entity)
                    .map(|obs| obs.can_vault)
                    .unwrap_or(false)
            },
        )
        .is_some();

    if crossing_obstacle {
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
        world.traversal.target_pos = Vec3::new(
            world.soldier_pos.x + travel_dir.x * config.soldier.vault_travel_distance,
            world.soldier_pos.y,
            world.soldier_pos.z + travel_dir.z * config.soldier.vault_travel_distance,
        );
        world.traversal.travel_dir = travel_dir;
        world.soldier_facing_dir = travel_dir;
        return;
    }

    let (kind, anim_name, speed) = match world.effective_stance {
        Stance::Run => (
            TraversalKind::RunningJump,
            "running_slide",
            config.soldier.running_slide_speed,
        ),
        Stance::Walk => (
            TraversalKind::RunningDiveRoll,
            "running_dive_roll",
            config.soldier.dive_roll_speed,
        ),
        Stance::Crouch => (
            TraversalKind::QuickRollToRun,
            "quick_roll_to_run",
            config.soldier.quick_roll_speed,
        ),
    };

    let exact_duration = config.exact_anim_duration(anim_name);
    let distance = speed * exact_duration;

    let shape_pos = Vec3::new(
        world.soldier_pos.x,
        world.soldier_pos.y + config.physics.soldier_capsule_center_y,
        world.soldier_pos.z,
    );

    let safe_dist = physics_world.sweep_capsule_horizontal(
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
                obstacle_query.contains(entity)
            }
        },
    );

    world.traversal.is_active = true;
    world.traversal.kind = kind;
    world.traversal.anim_name = anim_name.to_string();
    world.traversal.elapsed = 0.0;
    world.traversal.duration = exact_duration;
    world.traversal.start_pos = world.soldier_pos;
    world.traversal.target_pos = Vec3::new(
        world.soldier_pos.x + travel_dir.x * safe_dist,
        world.soldier_pos.y,
        world.soldier_pos.z + travel_dir.z * safe_dist,
    );
    world.traversal.travel_dir = travel_dir;
    world.soldier_facing_dir = travel_dir;
}

fn step_shooting_and_grenade_spawn(
    commands: &mut Commands,
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    dt: f32,
) {
    world.fire_cooldown_remaining = (world.fire_cooldown_remaining - dt).max(0.0);

    if world.is_dead || world.traversal.is_active {
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
                spawn_pos,
                Quat::IDENTITY,
                lin_vel,
                ang_vel,
            ));
        }
        return;
    }

    if world.reload_remaining.is_some() {
        return;
    }

    let wants_fire =
        input.fire_just_pressed || (input.fire_held && world.fire_cooldown_remaining <= 0.0);

    if wants_fire {
        world.fire_cooldown_remaining = config.combat.fire_cooldown_secs;
        let dur = config.exact_anim_duration("firing_rifle");
        world.firing_anim_remaining = Some(dur);

        let muzzle_pos = resolve_muzzle_pos(world, input, config);
        let to_aim = Vec3::new(
            world.aim_hit_pos.x - muzzle_pos.x,
            0.0,
            world.aim_hit_pos.z - muzzle_pos.z,
        );
        let shot_dir = if to_aim.length_squared() > config.physics.muzzle_aim_min_dist_sq {
            to_aim.normalize()
        } else {
            world.aim_world_dir
        };

        let spread = (world.next_rand() - 0.5) * config.combat.bullet_spread_rad;
        let final_dir = (Quat::from_rotation_y(spread) * shot_dir).normalize();

        let id = world.next_entity_id;
        world.next_entity_id += 1;
        commands.spawn((
            PhysicsBullet {
                id,
                velocity: final_dir * config.combat.bullet_speed,
                remaining_lifetime: config.combat.bullet_lifetime_secs,
            },
            Transform::from_translation(muzzle_pos),
        ));

        world.pending_fx.push(GameplayFxEvent::ShotFired {
            muzzle_pos,
            shot_dir: final_dir,
            trauma_add: config.combat.shot_fire_trauma_add,
            trauma_max: config.combat.shot_fire_trauma_max,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn shared_authoritative_react_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    physics_world: Res<CustomPhysicsWorld>,
    mut world: ResMut<AuthoritativeWorldState>,
    soldier_query: Query<&Transform, With<PhysicsSoldier>>,
    mut bullet_query: Query<
        (Entity, &mut PhysicsBullet, &mut Transform),
        (Without<PhysicsSoldier>, Without<PhysicsGrenade>),
    >,
    mut grenade_query: Query<
        (Entity, &mut PhysicsGrenade, &Transform, &Velocity),
        (Without<PhysicsSoldier>, Without<PhysicsBullet>),
    >,
    target_query: Query<(Entity, &PhysicsTarget)>,
) {
    let Some(input) = world.active_input.clone() else {
        return;
    };
    let dt = time.delta_secs();

    let soldier_tf = soldier_query
        .single()
        .expect("خطای بحرانی: موجودیت PhysicsSoldier یافت نشد");
    if world.traversal.kind != TraversalKind::ObstacleVault || !world.traversal.is_active {
        world.soldier_pos = Vec3::new(
            soldier_tf.translation.x,
            soldier_tf.translation.y - config.physics.soldier_capsule_center_y,
            soldier_tf.translation.z,
        );
    }

    let mut target_entity_by_idx = HashMap::new();
    let mut target_idx_by_entity = HashMap::new();
    for (entity, p_target) in &target_query {
        target_entity_by_idx.insert(p_target.index, entity);
        target_idx_by_entity.insert(entity, p_target.index);
    }

    let mut exploded_positions = Vec::new();
    world.grenades_snapshot.clear();

    for (entity, mut grenade, tf, vel) in &mut grenade_query {
        grenade.remaining_fuse -= dt;
        if grenade.remaining_fuse <= 0.0 {
            exploded_positions.push(tf.translation);
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

    for pos in exploded_positions {
        for (idx, target) in world.targets.iter_mut().enumerate() {
            if !target.is_dead
                && target.base_pos.distance(pos) < config.combat.grenade_target_kill_radius
            {
                target.hp = 0.0;
                target.is_dead = true;
                target.respawn_remaining = target.respawn_duration;
                if let Some(&t_ent) = target_entity_by_idx.get(&idx) {
                    commands.entity(t_ent).insert(ColliderDisabled);
                }
            }
        }

        if !world.is_dead
            && world.soldier_pos.distance(pos) < config.combat.grenade_soldier_kill_radius
        {
            trigger_random_death(&mut world, &config);
        }

        if let Some((water_y, _)) = sample_water_surface_at(&config, pos) {
            world.pending_fx.push(GameplayFxEvent::WaterSplash {
                pos: Vec3::new(pos.x, water_y, pos.z),
                intensity: 2.2,
            });
        }

        world.pending_fx.push(GameplayFxEvent::GrenadeExploded {
            pos,
            trauma_add: config.combat.grenade_explosion_trauma_add,
            trauma_max: config.combat.grenade_explosion_trauma_max,
        });
    }

    world.bullets_snapshot.clear();
    let bullet_query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    for (entity, mut bullet, mut tf) in &mut bullet_query {
        let old_pos = tf.translation;
        let step_vec = bullet.velocity * dt;
        let step_dist = step_vec.length();
        let step_dir = step_vec.normalize_or_zero();
        let new_pos = old_pos + step_vec;
        bullet.remaining_lifetime -= dt;

        let ray_hit = if step_dist > f32::EPSILON {
            physics_world
                .cast_ray(old_pos, step_dir, step_dist, bullet_query_groups, |hit_ent| {
                    if let Some(&t_idx) = target_idx_by_entity.get(&hit_ent) {
                        !world.targets[t_idx].is_dead
                    } else {
                        true
                    }
                })
                .map(|(hit_ent, hit)| (hit_ent, hit.point))
        } else {
            None
        };

        if let Some((hit_entity, impact_pos)) = ray_hit {
            let mut target_destroyed = false;
            if let Some(&t_idx) = target_idx_by_entity.get(&hit_entity) {
                let target = &mut world.targets[t_idx];
                target.hp -= config.combat.bullet_damage;
                target.hit_flash_timer = config.fx.target_hit_flash_secs;
                if target.hp <= 0.0 && !target.is_dead {
                    target.is_dead = true;
                    target.respawn_remaining = target.respawn_duration;
                    target_destroyed = true;
                    commands.entity(hit_entity).insert(ColliderDisabled);
                }
            }

            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
            });
            world.pending_fx.push(GameplayFxEvent::BulletImpact {
                impact_pos,
                bullet_dir: step_dir,
                target_destroyed,
                trauma_add: config.combat.target_destroy_trauma_add,
                trauma_max: config.combat.target_destroy_trauma_max,
            });
            commands.entity(entity).despawn();
        } else {
            tf.translation = new_pos;
            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: new_pos,
            });

            if bullet.remaining_lifetime <= 0.0 {
                commands.entity(entity).despawn();
            } else {
                world.bullets_snapshot.push(BulletSnapshot {
                    id: bullet.id,
                    translation: tf.translation,
                    velocity: bullet.velocity,
                });
            }
        }
    }

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

    let muzzle_pos = resolve_muzzle_pos(&world, &input, &config);
    let raw_target = Vec3::new(world.aim_hit_pos.x, muzzle_pos.y, world.aim_hit_pos.z);
    let dead_flags: Vec<bool> = world.targets.iter().map(|t| t.is_dead).collect();

    world.laser_hit_pos = compute_laser_hit(
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

    if let Some(ref mut active_input) = world.active_input {
        active_input.consume_just_pressed();
    }
}

fn shared_client_drive_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    prediction: Option<Res<ClientPhysicsPredictionState>>,
    mut soldier_query: Query<(&Transform, &mut CustomCharacterController), With<PhysicsSoldier>>,
) {
    let Some(prediction) = prediction else { return };
    let Ok((soldier_tf, mut controller)) = soldier_query.single_mut() else {
        return;
    };

    if prediction.is_dead || prediction.traversal_active {
        controller.translation = None;
        return;
    }

    let dt = time.delta_secs();
    let grav_y = config.physics.character_gravity_step_y * dt;

    let feet_pos = Vec3::new(
        soldier_tf.translation.x,
        soldier_tf.translation.y - config.physics.soldier_capsule_center_y,
        soldier_tf.translation.z,
    );
    let water_sample = sample_water_surface_at(&config, feet_pos);
    let speed_mult = if water_sample.is_some() {
        config.physics.water_move_speed_multiplier
    } else {
        1.0
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