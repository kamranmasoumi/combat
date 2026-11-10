use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_rapier3d::prelude::*;
use renet::{ChannelConfig, ConnectionConfig, RenetClient, RenetServer, SendType};
use renet_netcode::{
    ClientAuthentication, ConnectToken, NetcodeClientTransport, NetcodeServerTransport,
    ServerAuthentication, ServerConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// -----------------------------------------------------------------------------
// ۱. ساختارهای کانفیگ (بدون هیچ Default یا serde(default))
// -----------------------------------------------------------------------------

use std::collections::HashSet;

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
// ۱-ب. ساختارهای کامل نقشه MOBA + Tactical Shooter (بدون هیچ مقدار پیش‌فرض)
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
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub rotation_euler_deg: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RiverSegmentDef {
    pub id: String,
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub yaw_deg: f32,
    pub color_rgb: (f32, f32, f32),
    pub emissive_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WaterfallDef {
    pub id: String,
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
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub yaw_deg: f32,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TreeDef {
    pub id: String,
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
    pub bullets: Vec<BulletSnapshot>,
    pub grenades: Vec<GrenadeSnapshot>,
    pub targets: Vec<TargetSnapshot>,
    pub fx_events: Vec<GameplayFxEvent>,
}

// -----------------------------------------------------------------------------
// ۳. گروه‌های برخورد، کامپوننت‌ها و پلاگین مشترک Rapier 0.37
// -----------------------------------------------------------------------------

pub const GROUP_WORLD: Group = Group::GROUP_1;
pub const GROUP_SOLDIER: Group = Group::GROUP_2;
pub const GROUP_GRENADE: Group = Group::GROUP_3;
pub const GROUP_FX: Group = Group::GROUP_4;

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

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobaPhysicsSet {
    InputIngress,
    Drive,
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

        let fixed_dt = (1.0 / self.tick_rate_hz) as f32;

        app.insert_resource(Time::<Fixed>::from_hz(self.tick_rate_hz))
            .insert_resource(TimestepMode::Fixed {
                dt: fixed_dt,
                substeps: self.substeps,
            })
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule())
            .configure_sets(
                FixedUpdate,
                (
                    MobaPhysicsSet::InputIngress.before(MobaPhysicsSet::Drive),
                    MobaPhysicsSet::Drive.before(PhysicsSet::SyncBackend),
                    MobaPhysicsSet::React.after(PhysicsSet::Writeback),
                    MobaPhysicsSet::NetworkEgress.after(MobaPhysicsSet::React),
                ),
            )
            .add_systems(
                Startup,
                (
                    configure_rapier_context_system,
                    spawn_shared_physics_actors_system,
                )
                    .chain(),
            );

        match self.role {
            PhysicsRole::Server => {
                app.add_systems(Startup, spawn_server_environment_colliders_system)
                    .add_systems(
                        FixedUpdate,
                        shared_authoritative_drive_system.in_set(MobaPhysicsSet::Drive),
                    )
                    .add_systems(
                        FixedUpdate,
                        shared_authoritative_react_system.in_set(MobaPhysicsSet::React),
                    );
            }
            PhysicsRole::Client => {
                app.add_systems(
                    FixedUpdate,
                    shared_client_drive_system.in_set(MobaPhysicsSet::Drive),
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
// ۴. باندل‌های ساخت کولایدر و اجسام فیزیکی مشترک نقشه (استفاده در سرور و کلاینت)
// -----------------------------------------------------------------------------

pub fn ground_physics_bundle(
    config: &GameConfig,
) -> (
    PhysicsGround,
    ElevationSurface,
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(hx, hy, hz),
        Friction {
            coefficient: config.physics.ground_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.ground_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(half.x, half.y, half.z),
        Friction {
            coefficient: config.physics.ground_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.ground_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(half.x, half.y, half.z),
        Friction {
            coefficient: config.physics.ground_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.ground_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cylinder(half_h, bridge.outer_radius),
        Friction {
            coefficient: config.physics.ground_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.ground_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(width * 0.5, thickness * 0.5, len_3d * 0.5),
        Friction {
            coefficient: config.physics.ground_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.ground_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(half.x, half.y, half.z),
        Friction {
            coefficient: config.physics.obstacle_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.obstacle_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cylinder(half_h, tree.trunk_radius),
        Friction {
            coefficient: config.physics.obstacle_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.obstacle_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cylinder(half_h, turret.base_radius),
        Friction {
            coefficient: config.physics.obstacle_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.obstacle_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(half.x, half.y, half.z),
        Friction {
            coefficient: config.physics.obstacle_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.obstacle_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(half.x, half.y, half.z),
        Friction {
            coefficient: config.physics.obstacle_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.obstacle_restitution,
            combine_rule: CoefficientCombineRule::Average,
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
    RigidBody,
    Collider,
    Friction,
    Restitution,
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
        RigidBody::Fixed,
        Collider::cuboid(half.x, half.y, half.z),
        Friction {
            coefficient: config.physics.target_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Restitution {
            coefficient: config.physics.target_restitution,
            combine_rule: CoefficientCombineRule::Average,
        },
        world_collision_groups(),
        Transform::from_translation(pos),
    )
}

pub fn soldier_physics_bundle(
    config: &GameConfig,
) -> (
    PhysicsSoldier,
    RigidBody,
    Collider,
    CollisionGroups,
    KinematicCharacterController,
    Transform,
) {
    let spawn_pos = Vec3::new(
        config.soldier.spawn_pos.0,
        config.soldier.spawn_pos.1 + config.physics.soldier_capsule_center_y,
        config.soldier.spawn_pos.2,
    );
    (
        PhysicsSoldier,
        RigidBody::KinematicPositionBased,
        Collider::capsule_y(
            config.physics.soldier_capsule_half_segment,
            config.soldier.player_radius,
        ),
        soldier_collision_groups(),
        KinematicCharacterController {
            up: Vec3::Y,
            offset: CharacterLength::Absolute(config.physics.character_skin_offset),
            slide: true,
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(config.physics.character_autostep_max_height),
                min_width: CharacterLength::Absolute(config.physics.character_autostep_min_width),
                include_dynamic_bodies: false,
            }),
            max_slope_climb_angle: config.physics.character_max_slope_climb_deg.to_radians(),
            min_slope_slide_angle: config.physics.character_min_slope_slide_deg.to_radians(),
            apply_impulse_to_dynamic_bodies: true,
            snap_to_ground: Some(CharacterLength::Absolute(
                config.physics.character_snap_to_ground,
            )),
            normal_nudge_factor: config.physics.character_normal_nudge_factor,
            filter_flags: QueryFilterFlags::empty(),
            filter_groups: Some(soldier_collision_groups()),
            exclude_colliders: EntityHashSet::new(),
            exclude_rigid_bodies: EntityHashSet::new(),
            filter_predicate: None,
            custom_shape: None,
            custom_mass: None,
            translation: None,
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
    RigidBody,
    Collider,
    Restitution,
    Friction,
    Damping,
    GravityScale,
    Velocity,
    Ccd,
    CollisionGroups,
    Transform,
) {
    (
        PhysicsGrenade {
            id,
            remaining_fuse: config.combat.grenade_fuse_secs,
        },
        RigidBody::Dynamic,
        Collider::ball(config.physics.grenade_collider_radius),
        Restitution {
            coefficient: config.physics.grenade_restitution,
            combine_rule: CoefficientCombineRule::Max,
        },
        Friction {
            coefficient: config.physics.grenade_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Damping {
            linear_damping: config.physics.grenade_linear_damping,
            angular_damping: config.physics.grenade_angular_damping,
        },
        GravityScale(config.physics.grenade_gravity_scale),
        Velocity {
            linear: linear_velocity,
            angular: angular_velocity,
        },
        Ccd::enabled(),
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
    RigidBody,
    Collider,
    Restitution,
    Friction,
    Damping,
    GravityScale,
    Velocity,
    CollisionGroups,
    Transform,
) {
    (
        PhysicsShellCasing {
            lifetime: Timer::from_seconds(config.fx.shell_lifetime_secs, TimerMode::Once),
        },
        RigidBody::Dynamic,
        Collider::cylinder(
            config.physics.shell_collider_half_height,
            config.physics.shell_collider_radius,
        ),
        Restitution {
            coefficient: config.physics.shell_restitution,
            combine_rule: CoefficientCombineRule::Max,
        },
        Friction {
            coefficient: config.physics.shell_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Damping {
            linear_damping: config.physics.shell_linear_damping,
            angular_damping: config.physics.shell_angular_damping,
        },
        GravityScale(config.physics.shell_gravity_scale),
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
    RigidBody,
    Collider,
    Restitution,
    Friction,
    Damping,
    GravityScale,
    Velocity,
    CollisionGroups,
    Transform,
) {
    (
        PhysicsParticle {
            lifetime: Timer::from_seconds(lifetime_secs, TimerMode::Once),
            scale_decay,
        },
        RigidBody::Dynamic,
        Collider::ball(config.physics.particle_collider_radius),
        Restitution {
            coefficient: config.physics.particle_restitution,
            combine_rule: CoefficientCombineRule::Max,
        },
        Friction {
            coefficient: config.physics.particle_friction,
            combine_rule: CoefficientCombineRule::Average,
        },
        Damping {
            linear_damping: config.physics.particle_linear_damping,
            angular_damping: config.physics.particle_angular_damping,
        },
        GravityScale(config.physics.particle_gravity_scale),
        Velocity {
            linear: linear_velocity,
            angular: Vec3::ZERO,
        },
        fx_collision_groups(),
        Transform::from_translation(translation),
    )
}

/// قانون برتری دید ارتفاعی (Dota 2 High-Ground Vision Rule):
/// اگر ناظر در ارتفاع پایین‌تر از هدف باشد (اختلاف بیش از `high_ground_occlusion_min_diff`)،
/// دید مستقیم به سطح بالاتر مسدود است؛ اما اگر ناظر روی High-Ground باشد، شعاع دید او به اندازه
/// `high_ground_vision_bonus` افزایش می‌یابد.
pub fn can_observer_see_point(
    rapier_context: &RapierContext,
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

    let filter = QueryFilter::new().groups(CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD));
    rapier_context
        .cast_ray(eye_origin, to_target / dist, dist, true, filter)
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
// ۶. توابع کمکی جهت‌گیری، لیزر و راه‌اندازی Rapier (مشترک بین سرور و کلاینت)
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

pub fn compute_laser_hit_with_rapier<F>(
    rapier_context: &RapierContext,
    muzzle_pos: Vec3,
    raw_target: Vec3,
    dead_target_predicate: F,
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
    let pred = |entity: Entity, _collider: &bevy_rapier3d::rapier::geometry::Collider| {
        dead_target_predicate(entity)
    };
    let filter = QueryFilter::new()
        .groups(CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD))
        .predicate(&pred);

    if let Some((_entity, toi)) = rapier_context.cast_ray(muzzle_pos, dir, max_dist, true, filter) {
        muzzle_pos + dir * toi
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

fn configure_rapier_context_system(
    config: Res<GameConfig>,
    mut rapier_configs: Query<&mut RapierConfiguration, With<DefaultRapierContext>>,
) {
    let mut rapier_cfg = rapier_configs
        .single_mut()
        .expect("خطای بحرانی: DefaultRapierContext در صحنه یافت نشد");
    rapier_cfg.gravity = Vec3::new(0.0, config.physics.gravity_y, 0.0);
    rapier_cfg.force_update_from_transform_changes = false;
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

fn shared_authoritative_drive_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    rapier_contexts: ReadRapierContext,
    mut world: ResMut<AuthoritativeWorldState>,
    mut soldier_query: Query<
        (&mut Transform, &mut KinematicCharacterController),
        With<PhysicsSoldier>,
    >,
    obstacle_query: Query<&PhysicsObstacle>,
    target_query: Query<&PhysicsTarget>,
) {
    let Some(input) = world.active_input.clone() else {
        return;
    };

    let dt = time.delta_secs();
    let rapier_context = rapier_contexts
        .single()
        .expect("خطای بحرانی: RapierContext معتبر یافت نشد");
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

        // ۷. سیستم پرش، غلت و عبور از مانع با پرتو و کپسول Rapier
        step_traversal_with_rapier(
            &mut world,
            &input,
            &config,
            &rapier_context,
            &obstacle_query,
            &target_query,
            &mut soldier_tf,
            &mut controller,
            dt,
        );

        // ۸. جابجایی فیزیکی ۳بعدی کاراکتر روی رمپ‌ها، پل مرکزی و سطوح ارتفاعی با KinematicCharacterController
        if !world.traversal.is_active {
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

    // ۹. شلیک گلوله و پرتاب نارنجک در دنیای Rapier
    step_shooting_and_grenade_spawn(&mut commands, &mut world, &input, &config, dt);
}

#[allow(clippy::too_many_arguments)]
fn step_traversal_with_rapier(
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    config: &GameConfig,
    rapier_context: &RapierContext,
    obstacle_query: &Query<&PhysicsObstacle>,
    target_query: &Query<&PhysicsTarget>,
    soldier_tf: &mut Transform,
    controller: &mut KinematicCharacterController,
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
            let dust_pos =
                world.soldier_pos - world.traversal.travel_dir * 0.4 + Vec3::new(0.0, 0.08, 0.0);
            world
                .pending_fx
                .push(GameplayFxEvent::SlideDust { pos: dust_pos });
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
    let vault_pred = |entity: Entity, _collider: &bevy_rapier3d::rapier::geometry::Collider| {
        obstacle_query
            .get(entity)
            .map(|obs| obs.can_vault)
            .unwrap_or(false)
    };
    let vault_filter = QueryFilter::new()
        .groups(CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD))
        .predicate(&vault_pred);

    let crossing_obstacle = rapier_context
        .cast_ray(
            ray_origin,
            travel_dir,
            config.soldier.vault_check_distance,
            true,
            vault_filter,
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

    let capsule_shape = Collider::capsule_y(
        config.physics.soldier_capsule_half_segment,
        config.soldier.player_radius,
    );
    let shape_pos = Vec3::new(
        world.soldier_pos.x,
        world.soldier_pos.y + config.physics.soldier_capsule_center_y,
        world.soldier_pos.z,
    );
    let cast_pred = |entity: Entity, _collider: &bevy_rapier3d::rapier::geometry::Collider| {
        if let Ok(t) = target_query.get(entity) {
            !world.targets[t.index].is_dead
        } else {
            obstacle_query.contains(entity)
        }
    };
    let cast_filter = QueryFilter::new()
        .groups(CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD))
        .predicate(&cast_pred);

    let safe_dist = if let Some((_entity, hit)) = rapier_context.cast_shape(
        shape_pos,
        Quat::IDENTITY,
        travel_dir,
        &capsule_shape,
        ShapeCastOptions {
            max_time_of_impact: distance,
            target_distance: config.soldier.free_traversal_wall_padding,
            stop_at_penetration: true,
            compute_impact_geometry_on_penetration: true,
        },
        cast_filter,
    ) {
        hit.time_of_impact.max(0.0)
    } else {
        distance
    };

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
    rapier_contexts: ReadRapierContext,
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
    let rapier_context = rapier_contexts
        .single()
        .expect("خطای بحرانی: RapierContext در گام React یافت نشد");

    // ۱. خواندن موقعیت ۳بعدی نهایی کپسول سرباز (شامل ارتفاع دقیق روی رمپ، پل یا High-Ground)
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

    // ۲. بروزرسانی نارنجک‌های دینامیک Rapier و بررسی انفجار
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

        world.pending_fx.push(GameplayFxEvent::GrenadeExploded {
            pos,
            trauma_add: config.combat.grenade_explosion_trauma_add,
            trauma_max: config.combat.grenade_explosion_trauma_max,
        });
    }

    // ۳. حرکت گلوله‌ها و تشخیص برخورد پیوسته با Raycast در Rapier
    world.bullets_snapshot.clear();

    for (entity, mut bullet, mut tf) in &mut bullet_query {
        let old_pos = tf.translation;
        let step_vec = bullet.velocity * dt;
        let step_dist = step_vec.length();
        let step_dir = step_vec.normalize_or_zero();
        let new_pos = old_pos + step_vec;
        bullet.remaining_lifetime -= dt;

        let bullet_pred =
            |hit_ent: Entity, _collider: &bevy_rapier3d::rapier::geometry::Collider| {
                if let Some(&t_idx) = target_idx_by_entity.get(&hit_ent) {
                    !world.targets[t_idx].is_dead
                } else {
                    true
                }
            };
        let filter = QueryFilter::new()
            .groups(CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD))
            .predicate(&bullet_pred);

        let ray_hit = if step_dist > f32::EPSILON {
            rapier_context
                .cast_ray_and_get_normal(old_pos, step_dir, step_dist, true, filter)
                .map(|(hit_ent, intersection)| (hit_ent, intersection.point))
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

    // ۴. ریکاوری و فعال‌سازی مجدد کولایدر اهداف تیراندازی در Rapier
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

    // ۵. محاسبه نقطه برخورد لیزر با Raycast در Rapier (پرتو افقی از ارتفاع لوله سلاح؛ اگر به دیوار/صخره High-Ground بخورد مسدود می‌شود)
    let muzzle_pos = resolve_muzzle_pos(&world, &input, &config);
    let raw_target = Vec3::new(world.aim_hit_pos.x, muzzle_pos.y, world.aim_hit_pos.z);
    let dead_flags: Vec<bool> = world.targets.iter().map(|t| t.is_dead).collect();

    world.laser_hit_pos = compute_laser_hit_with_rapier(
        &rapier_context,
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
    mut soldier_query: Query<&mut KinematicCharacterController, With<PhysicsSoldier>>,
) {
    let Some(prediction) = prediction else { return };
    let Ok(mut controller) = soldier_query.single_mut() else {
        return;
    };

    if prediction.is_dead || prediction.traversal_active {
        controller.translation = None;
        return;
    }

    let dt = time.delta_secs();
    let grav_y = config.physics.character_gravity_step_y * dt;

    let Some(ref input) = prediction.latest_input else {
        controller.translation = Some(Vec3::new(0.0, grav_y, 0.0));
        return;
    };

    let flat_input = Vec3::new(input.move_input.x, 0.0, input.move_input.z);
    if flat_input != Vec3::ZERO {
        let speed = match prediction.effective_stance {
            Stance::Run => config.soldier.run_speed,
            Stance::Walk => config.soldier.walk_speed,
            Stance::Crouch => config.soldier.crouch_speed,
        };
        let delta = flat_input.normalize() * speed * dt;
        controller.translation = Some(Vec3::new(delta.x, grav_y, delta.z));
    } else {
        controller.translation = Some(Vec3::new(0.0, grav_y, 0.0));
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