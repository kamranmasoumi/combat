use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use bevy::transform::TransformPlugin;
use bevy_rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

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
    pub obstacles: Vec<ObstacleDef>,
    pub targets: Vec<TargetDef>,
}

impl GameConfig {
    pub fn load_from_file(path: &str) -> Self {
        let content = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("خطای بحرانی: فایل کانفیگ '{}' خوانده نشد: {}", path, e));
        ron::from_str::<GameConfig>(&content).unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی: ساختار فایل کانفیگ '{}' نامعتبر است: {}",
                path, e
            )
        })
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkConfig {
    pub server_bind_addr: String,
    pub client_bind_addr: String,
    pub server_tick_rate_hz: f64,
    pub socket_recv_buffer_bytes: usize,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ObstacleDef {
    pub pos: (f32, f32, f32),
    pub size: (f32, f32, f32),
    pub can_vault: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TargetDef {
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
    CollisionGroups::new(
        GROUP_WORLD,
        GROUP_SOLDIER | GROUP_GRENADE | GROUP_FX | GROUP_WORLD,
    )
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
pub struct PhysicsGround;

#[derive(Component)]
pub struct PhysicsObstacle {
    pub can_vault: bool,
}

#[derive(Component)]
pub struct PhysicsTarget {
    pub index: usize,
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
// ۴. باندل‌های ساخت کولایدر و اجسام فیزیکی مشترک (استفاده در سرور و کلاینت)
// -----------------------------------------------------------------------------

pub fn ground_physics_bundle(
    config: &GameConfig,
) -> (
    PhysicsGround,
    RigidBody,
    Collider,
    Friction,
    Restitution,
    CollisionGroups,
    Transform,
) {
    let hx = config.lighting.ground_size.0 * 0.5;
    let hy = config.physics.ground_half_thickness;
    let hz = config.lighting.ground_size.1 * 0.5;
    (
        PhysicsGround,
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
        Transform::from_xyz(0.0, -hy, 0.0),
    )
}

pub fn obstacle_physics_bundle(
    config: &GameConfig,
    obs: &ObstacleDef,
) -> (
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
    (
        PhysicsObstacle {
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
        Transform::from_translation(pos),
    )
}

pub fn target_physics_bundle(
    config: &GameConfig,
    index: usize,
    target: &TargetDef,
) -> (
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
        PhysicsTarget { index },
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
            fx_events: std::mem::take(&mut self.pending_fx),
        }
    }
}

// -----------------------------------------------------------------------------
// ۶. توابع کمکی جهت‌گیری، لیزر و پرتوهای Rapier (مشترک بین سرور و کلاینت)
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

// -----------------------------------------------------------------------------
// ۷. سیستم‌های راه‌اندازی و گام‌های شبیه‌سازی Rapier (Drive و React)
// -----------------------------------------------------------------------------

fn configure_rapier_context_system(
    config: Res<GameConfig>,
    mut rapier_configs: Query<&mut RapierConfiguration, With<DefaultRapierContext>>,
) {
    let mut rapier_cfg = rapier_configs
        .single_mut()
        .expect("خطای بحرانی: DefaultRapierContext در صحنه یافت نشد");
    rapier_cfg.gravity = Vec3::new(0.0, config.physics.gravity_y, 0.0);
    rapier_cfg.force_update_from_transform_changes = true;
}

fn spawn_shared_physics_actors_system(mut commands: Commands, config: Res<GameConfig>) {
    commands.spawn(soldier_physics_bundle(&config));
}

fn spawn_server_environment_colliders_system(mut commands: Commands, config: Res<GameConfig>) {
    commands.spawn(ground_physics_bundle(&config));

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

    // ۱. بررسی کلید X (تست مرگ) و تایمر زنده شدن مجدد
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

        // ۵. محاسبه جهت هدف‌گیری با ماوس (Aim Ray)
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

        // ۸. جابجایی فیزیکی کاراکتر با KinematicCharacterController در Rapier
        if !world.traversal.is_active {
            if world.is_moving {
                let speed = match world.effective_stance {
                    Stance::Run => config.soldier.run_speed,
                    Stance::Walk => config.soldier.walk_speed,
                    Stance::Crouch => config.soldier.crouch_speed,
                };
                let desired_delta = world.move_input.normalize() * speed * dt;
                controller.translation = Some(Vec3::new(desired_delta.x, 0.0, desired_delta.z));
            } else {
                controller.translation = None;
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
        let mut current_pos = world
            .traversal
            .start_pos
            .lerp(world.traversal.target_pos, frac);
        current_pos.y = config.soldier.spawn_pos.1;
        world.soldier_facing_dir = world.traversal.travel_dir;

        if world.traversal.kind == TraversalKind::ObstacleVault {
            // در حالت پرش از روی مانع کوتاه، کپسول مستقیماً از روی مانع عبور می‌کند
            controller.translation = None;
            soldier_tf.translation = Vec3::new(
                current_pos.x,
                current_pos.y + config.physics.soldier_capsule_center_y,
                current_pos.z,
            );
            world.soldier_pos = current_pos;
        } else {
            // در حالت سر خوردن یا غلتیدن، حرکت از طریق KinematicCharacterController اعمال می‌شود
            let step_delta = current_pos - world.soldier_pos;
            controller.translation = Some(Vec3::new(step_delta.x, 0.0, step_delta.z));
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

    // بررسی وجود مانع قابل پرش (Vault) با Raycast در Rapier
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
            config.soldier.spawn_pos.1,
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

    // بررسی مسیر غلت/سر خوردن با ShapeCast کپسول در Rapier
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
        config.soldier.spawn_pos.1,
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

    // ۱. خواندن موقعیت نهایی کپسول سرباز پس از حل برخوردهای Rapier
    let soldier_tf = soldier_query
        .single()
        .expect("خطای بحرانی: موجودیت PhysicsSoldier یافت نشد");
    if world.traversal.kind != TraversalKind::ObstacleVault || !world.traversal.is_active {
        world.soldier_pos = Vec3::new(
            soldier_tf.translation.x,
            config.soldier.spawn_pos.1,
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

    // ۵. محاسبه نقطه برخورد لیزر با Raycast در Rapier
    let muzzle_pos = resolve_muzzle_pos(&world, &input, &config);
    let raw_target = Vec3::new(world.aim_hit_pos.x, muzzle_pos.y, world.aim_hit_pos.z);
    let dead_flags: Vec<bool> = world.targets.iter().map(|t| t.is_dead).collect();

    world.laser_hit_pos = compute_laser_hit_with_rapier(
        &rapier_context,
        muzzle_pos,
        raw_target,
        &|hit_ent| {
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

// -----------------------------------------------------------------------------
// ۸. سیستم‌های پیش‌بینی و اجرای فیزیک Rapier در سمت کلاینت (درون moba_shared)
// -----------------------------------------------------------------------------

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

    let Some(ref input) = prediction.latest_input else {
        controller.translation = None;
        return;
    };

    let flat_input = Vec3::new(input.move_input.x, 0.0, input.move_input.z);
    if flat_input != Vec3::ZERO {
        let speed = match prediction.effective_stance {
            Stance::Run => config.soldier.run_speed,
            Stance::Walk => config.soldier.walk_speed,
            Stance::Crouch => config.soldier.crouch_speed,
        };
        let delta = flat_input.normalize() * speed * time.delta_secs();
        controller.translation = Some(Vec3::new(delta.x, 0.0, delta.z));
    } else {
        controller.translation = None;
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