use bevy::animation::AnimatedBy;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfMaterialName;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::math::Affine2;
use bevy::shader::ShaderRef;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::gltf::Gltf;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight, NotShadowCaster};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::{CursorOptions, MonitorSelection, PresentMode, PrimaryWindow, WindowMode, WindowResolution};
use moba_shared::{
    ancient_physics_bundle, barracks_physics_bundle, center_bridge_physics_bundle, create_shared_renet_client,
    grenade_physics_bundle, ground_physics_bundle, lane_segment_physics_bundle,
    mountain_physics_bundle, obstacle_physics_bundle, orient_soldier_towards,
    particle_physics_bundle, ramp_physics_bundle, reconcile_soldier_physics_pos,
    shell_casing_physics_bundle, target_physics_bundle,
    terrain_platform_physics_bundle, tree_physics_bundle, turret_physics_bundle,
    apply_client_cli_overrides, ClientInputCommand, ClientLobbyMessage, CourierOrder, CourierSnapshot, CreepAnimState, CreepSnapshot,
    ClientPhysicsPredictionState, HeroSnapshot, MatchPhase, MatchStateSnapshot,
    ActiveWeaponSlot, AncientSnapshot, BarracksKind, BarracksSnapshot, ChatMessageEntry, ChatMessageKind, CustomPhysicsWorld, PausePhase, PauseSnapshot, soldier_collision_groups, ModelTransformConfig, RemotePlayerSnapshot, ServerLobbyPacket, ServerMatchMessage, TopBarHeroInfo, TurretSnapshot, SharedFogOfWar, WeaponKind, CHANNEL_RELIABLE_LOBBY,
    FOW_GRID_H, FOW_GRID_W, ColliderDisabled, GameConfig,
    GameplayFxEvent, ItemsConfig, MapObjectId, MobaSharedPhysicsPlugin, PhysicsRole, PhysicsSoldier,
    ScoreboardHeroInfo, ServerWorldSnapshot, Stance, TeamId, UpperBodyAction, Velocity, CHANNEL_RELIABLE_FX,
    CHANNEL_UNRELIABLE_STATE,
};
use renet::RenetClient;
use renet_netcode::NetcodeClientTransport;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::path::Path;
use std::time::Duration;

mod shop_client;
use shop_client::{spawn_item_icon_badge, CourierControlState, ItemIconCache, SettingsUiState, ShopContextSnapshot, ShopOutgoingQueue, ShopPlugin, ShopUiState};


#[cfg(not(debug_assertions))]
fn load_client_config() -> (GameConfig, bool) {
    if Path::new("config.ron").is_file() {
        (GameConfig::load_from_file("config.ron"), true)
    } else if Path::new("../../config.ron").is_file() {
        (GameConfig::load_from_file("../../config.ron"), true)
    } else {
        (
            GameConfig::load_from_str(include_str!("../../config.ron"), "embedded:config.ron"),
            false,
        )
    }
}

#[cfg(debug_assertions)]
fn load_client_config() -> (GameConfig, bool) {
    if Path::new("config.ron").is_file() {
        (GameConfig::load_from_file("config.ron"), true)
    } else if Path::new("../../config.ron").is_file() {
        (GameConfig::load_from_file("../../config.ron"), true)
    } else {
        panic!("خطای بحرانی: فایل config.ron در مسیر لوکال پیدا نشد!");
    }
}

#[cfg(not(debug_assertions))]
fn load_client_items_config() -> (ItemsConfig, bool) {
    if Path::new("items.ron").is_file() {
        (ItemsConfig::load_from_file("items.ron"), true)
    } else if Path::new("../../items.ron").is_file() {
        (ItemsConfig::load_from_file("../../items.ron"), true)
    } else {
        (
            ItemsConfig::load_from_str(include_str!("../../items.ron"), "embedded:items.ron"),
            false,
        )
    }
}

#[cfg(debug_assertions)]
fn load_client_items_config() -> (ItemsConfig, bool) {
    if Path::new("items.ron").is_file() {
        (ItemsConfig::load_from_file("items.ron"), true)
    } else if Path::new("../../items.ron").is_file() {
        (ItemsConfig::load_from_file("../../items.ron"), true)
    } else {
        panic!("خطای بحرانی: فایل items.ron در مسیر لوکال پیدا نشد!");
    }
}

fn main() {
    let (mut config, has_local_config) = load_client_config();
    let (items_config, has_local_items) = load_client_items_config();
    if let Some(build_addr) = option_env!("MOBA_SERVER_ADDR") {
        if !build_addr.trim().is_empty() {
            config.network.server_public_addr = build_addr.trim().to_string();
            config.network.server_bind_addr = build_addr.trim().to_string();
        }
    }
    if std::env::args().nth(2).is_none() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(101);
        config.network.client_id = (nanos % 900_000_000) + 1_000;
    }
    apply_client_cli_overrides(&mut config.network);
    let tick_hz = config.network.server_tick_rate_hz;
    let substeps = config.physics.substeps;

    let (client, transport) = create_shared_renet_client(&config.network);

    let tuner = WeaponTuner {
        offset: Vec3::new(
            config.weapon_tuner.initial_offset.0,
            config.weapon_tuner.initial_offset.1,
            config.weapon_tuner.initial_offset.2,
        ),
        rotation: Vec3::new(
            config.weapon_tuner.initial_rotation_deg.0,
            config.weapon_tuner.initial_rotation_deg.1,
            config.weapon_tuner.initial_rotation_deg.2,
        ),
        scale: config.weapon_tuner.initial_scale,
    };

    let render_state = ClientRenderState::new_from_config(&config);
    let initial_hero_snapshot = {
        let h = config.active_hero(0);
        HeroSnapshot {
            hero_id: h.id.clone(),
            display_name: h.display_name.clone(),
            team: h.team,
            gold: h.starting_gold,
            level: 1,
            xp: 0,
            xp_into_level: 0,
            xp_for_next_level: config.combat.xp_thresholds_per_level[1],
            xp_gain_radius: h.xp_gain_radius,
            hp: h.max_hp,
            max_hp: h.max_hp,
            hp_regen_per_sec: h.hp_regen_per_sec,
            in_fountain: false,
            stamina: h.max_stamina,
            max_stamina: h.max_stamina,
            stamina_regen_per_sec: h.stamina_regen_per_sec,
            armor: h.armor,
            hero_scale: h.hero_scale,
            move_speed: config.soldier.run_speed * h.move_speed_multiplier,
            vision_range: h.vision_range,
            active_weapon: ActiveWeaponSlot::Primary,
            weapon_kind: h.weapon.weapon_kind,
            ammo_in_mag: h.weapon.magazine_size,
            magazine_size: h.weapon.magazine_size,
            fire_rate_secs: h.weapon.fire_rate_secs,
            reload_duration_secs: h.weapon.reload_duration_secs,
            weapon_damage: h.weapon.damage,
            projectile_speed: h.weapon.projectile_speed,
            weapon_max_range: h.weapon.max_range,
            grenade_charges: h.grenade.max_charges,
            grenade_max_charges: h.grenade.max_charges,
            grenade_recharge_remaining: 0.0,
            grenade_throw_duration_secs: h.grenade.throw_duration_secs,
            last_hits: 0,
            denies: 0,
            assists: 0,
            kills: 0,
            deaths: 0,
            respawn_remaining: 0.0,
            inventory: Vec::new(),
            courier_hp: h.courier.max_hp,
            courier_max_hp: h.courier.max_hp,
            courier_is_dead: false,
            courier_respawn_remaining: 0.0,
            courier_order: CourierOrder::Idle,
            courier_in_fountain: true,
            courier_inventory: Vec::new(),
        }
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window.title.clone(),
                resolution: WindowResolution::new(config.window.width, config.window.height),
                present_mode: PresentMode::AutoNoVsync,
                mode: WindowMode::BorderlessFullscreen(MonitorSelection::Current),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(DirectionalLightShadowMap { size: 2048 })
        .insert_resource(config)
        .insert_resource(items_config)
        .insert_resource(LocalConfigOverride {
            has_local_config,
            has_local_items,
        })
        .add_plugins(ShopPlugin)
        .insert_resource(ClientNetwork { client, transport })
        .insert_resource(render_state)
        .insert_resource(ClientPhysicsPredictionState::new())
        .insert_resource(WeaponAttachmentStatus {
            attached: false,
            hand_entity: None,
        })
        .insert_resource(tuner)
        .insert_resource(CameraShake { trauma: 0.0 })
        .insert_resource(LatestHeroSnapshot(initial_hero_snapshot))
        .insert_resource(LatestRemotePlayers::default())
        .insert_resource(LatestCreeps::default())
        .insert_resource(LatestCouriers::default())
        .insert_resource(InspectedUnit::default())
        .insert_resource(HeroClickMoveTarget::default())
        .insert_resource(LatestTopBarState::default())
        .insert_resource(LatestScoreboard::default())
        .insert_resource(LatestTurrets::default())
        .insert_resource(LatestBarracks::default())
        .insert_resource(LatestAncients::default())
        .insert_resource(LatestMatchResult::default())
        .insert_resource(ChatUiState::default())
        .insert_resource(ClientPingState::default())
        .insert_resource(LatestPauseState::default())
        .insert_resource(ClientTelemetryState::default())
        .insert_resource(FixedSoldierTransformHistory::default())
        .insert_resource(HeroModelAnimCache::default())
        .insert_resource(ClientMatchFlow {
            your_slot: None,
            snapshot: None,
            preview_hero_index: 0,
            spawned_preview_hero_index: None,
            map_loaded_reported: false,
            prev_phase: None,
        })
        .insert_resource(CameraControlState {
            free_look_target: None,
            camera_unlocked: false,
            current_cursor_lead: Vec3::ZERO,
            is_pointer_over_minimap: false,
            is_dragging_minimap: false,
        })
        .add_plugins(MaterialPlugin::<WaterSurfaceMaterial>::default())
        .insert_resource(bevy::winit::WinitSettings::game())
        .insert_resource(Time::<Fixed>::from_hz(60.0))
        .add_plugins(MobaSharedPhysicsPlugin {
            role: PhysicsRole::Client,
            tick_rate_hz: tick_hz,
            substeps,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(PreUpdate, client_sample_input_system)
        .add_systems(
            FixedUpdate,
            (
                client_flush_input_to_server_system,
                client_reconcile_fixed_system.after(moba_shared::MobaPhysicsSet::Simulate),
                record_soldier_fixed_transform_system.after(client_reconcile_fixed_system),
            ),
        )
        .add_systems(
            PostUpdate,
            compensate_upper_body_hips_twist_system
                .after(bevy::app::AnimationSystems)
                .before(bevy::transform::TransformSystems::Propagate),
        )
        .add_systems(
            Update,
            (
                load_gltf_and_build_graph,
                attach_split_body_animation_graphs,
                attach_weapon_to_left_hand,
                weapon_live_calibration_system,
                client_receive_snapshot_and_sync_system,
                target_visual_reaction_system,
                environment_vfx_and_water_animation_system,
                camera_follow_system,
                soldier_animation_player_system,
                laser_sight_gizmo_system,
                cursor_reticle_system,
                gameplay_fx_system,
                level_up_vfx_system,
                floating_world_popup_system,
                death_screen_overlay_system,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                update_map_object_labels_system,
                minimap_interaction_system,
                minimap_overlay_update_system,
                fog_of_war_update_system,
                occlusion_transparency_system,
                optimize_tree_shadows_system,
                water_shader_update_system,
                waterfall_mist_particle_system,
                animate_glb_waterfall_materials_system,
                update_hero_hud_system,
                update_hero_stamina_hud_system,
                client_lobby_sync_system,
                hero_preview_system,
                match_overlay_ui_system,
                sync_remote_players_system,
                update_overhead_hero_bars_system,
            )
                .chain()
                .after(camera_follow_system),
        )
        .add_systems(
            Update,
            (sync_lane_creeps_system, update_overhead_creep_bars_system, update_top_match_hud_system, sync_turrets_system, update_overhead_turret_bars_system, sync_barracks_system, update_overhead_barracks_bars_system, sync_ancients_system, update_overhead_ancients_bars_system, end_of_match_overlay_system, client_chat_input_system, update_chat_ui_system, client_ping_input_system, update_ping_visuals_system, client_pause_input_system, update_pause_overlay_ui_system, update_telemetry_hud_system, sync_shop_context_and_network_system)
                .chain()
                .after(sync_remote_players_system),
        )
        .add_systems(Update, (adaptive_ui_performance_mode_system, sync_couriers_system, update_overhead_courier_bars_system, courier_manual_move_input_system, hover_ally_highlight_system, animate_move_click_arrows_system, update_scoreboard_ui_system))
        .add_systems(
            PostUpdate,
            align_weapon_horizontally_system.before(TransformSystems::Propagate),
        )
        .run();
}

#[derive(Resource)]
struct LocalConfigOverride {
    has_local_config: bool,
    has_local_items: bool,
}

#[derive(Resource)]
struct ClientNetwork {
    client: RenetClient,
    transport: NetcodeClientTransport,
}

#[derive(Resource)]
struct WeaponTuner {
    offset: Vec3,
    rotation: Vec3,
    scale: f32,
}

// -----------------------------------------------------------------------------
// استیت‌ها، منابع و کامپوننت‌های محیطی و VFX
// -----------------------------------------------------------------------------

#[derive(Component)]
struct SoldierMarker;

#[derive(Component)]
struct LowerBodyAnimationPlayer;

#[derive(Component)]
struct UpperBodyAnimationPlayer;

#[derive(Component)]
struct SoldierHipsBone;

#[derive(Component)]
struct SoldierSpineRootBone;

#[derive(Component)]
struct MobaCamera;

#[derive(Component)]
struct WeaponRoot;

#[derive(Component)]
struct AttachedWeaponKind(WeaponKind);

#[derive(Component)]
struct MuzzlePoint;

#[derive(Component)]
struct LaserBeamCore;

#[derive(Component)]
struct LaserBeamGlow;

#[derive(Component)]
struct LaserPulseBead {
    phase: f32,
}

#[derive(Component)]
struct LaserImpactDot;

#[derive(Component)]
struct LaserImpactRing;

#[derive(Component)]
struct ReticleRoot {
    current_spread_px: f32,
}

#[derive(Component)]
struct ReticleOuterRing;

#[derive(Component)]
struct ReticleInnerRing;

#[derive(Component)]
struct ReticleCenterDot;

#[derive(Component)]
enum ReticleTickDir {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Component)]
struct MapObjectLabel {
    target_entity: Entity,
    world_offset: Vec3,
}

#[derive(Component)]
struct VisualTarget {
    index: usize,
    max_hp: f32,
    hp: f32,
    hit_flash_timer: f32,
    base_pos: Vec3,
    base_scale: Vec3,
    base_color: Color,
    is_dead: bool,
    is_server_visible: bool,
}

#[derive(Resource, Clone)]
struct SharedFxAssets {
    bullet_mesh: Handle<Mesh>,
    bullet_mat: Handle<StandardMaterial>,
    muzzle_flash_mesh: Handle<Mesh>,
    muzzle_flash_mat: Handle<StandardMaterial>,
    shell_mesh: Handle<Mesh>,
    shell_mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct NetworkBulletVisual {
    server_id: u64,
    visual_offset: Vec3,
    visual_offset_blend: f32,
}

#[derive(Component)]
struct NetworkGrenadeVisual {
    server_id: u64,
    spawn_time: f32,
}

#[derive(Component)]
struct GrenadeBlinkLed {
    mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct ExplosionSmokeParticle {
    vel: Vec3,
    timer: Timer,
    start_scale: f32,
    end_scale: f32,
}

#[derive(Component)]
struct BulletTrail {
    start: Vec3,
    end: Vec3,
    timer: Timer,
}

#[derive(Component)]
struct MuzzleFlash {
    timer: Timer,
}

/// کامپوننت انیمیشن جریان سطح آب و رگه‌های کف روی رودخانه و برکه‌ها
#[derive(Component)]
struct FlowingWaterStreak {
    origin: Vec3,
    travel_vec: Vec3,
    cycle_secs: f32,
    phase: f32,
}

/// کامپوننت انیمیشن ریزش آبشار
#[derive(Component)]
struct CascadingWaterfallSheet {
    top_pos: Vec3,
    drop_height: f32,
    speed: f32,
    phase: f32,
}

/// کامپوننت حلقه‌های موج آب (هم در پای آبشارها و هم هنگام راه رفتن بازیکن در آب)
#[derive(ShaderType, Debug, Clone)]
struct WaterUniforms {
    base_color: Vec4,
    foam_color: Vec4,
    flow_and_params: Vec4,
    player_ripple: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct WaterSurfaceMaterial {
    #[uniform(0)]
    uniforms: WaterUniforms,
    #[texture(1)]
    #[sampler(2)]
    foam_tex: Handle<Image>,
}

impl Material for WaterSurfaceMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/water.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/water.wesl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Component)]
struct WaterfallMistEmitter {
    base_pos: Vec3,
    width: f32,
    timer: Timer,
}

#[derive(Component)]
struct WaterfallMistParticle {
    vel: Vec3,
    timer: Timer,
    start_scale: f32,
    end_scale: f32,
}

#[derive(Resource)]
struct SharedWaterFoamTexture(Handle<Image>);

#[derive(Component)]
struct GlbAnimatedWaterTag {
    is_waterfall: bool,
}

#[derive(Component)]
struct WaterRippleRing {
    timer: Timer,
    start_scale: f32,
    end_scale: f32,
}

/// کامپوننت چرخش و شناور بودن کریستال‌های باستانی و برج‌ها
#[derive(Component)]
struct AnimatedCrystalCore {
    base_y: f32,
    bob_amp: f32,
    bob_speed: f32,
    rot_speed: f32,
    phase: f32,
}

/// کامپوننت ذرات معلق جادویی، شب‌تاب‌های جنگل و مه دره
#[derive(Component)]
struct AmbientFloatingMote {
    base_pos: Vec3,
    drift_amp: Vec3,
    freq: f32,
    phase: f32,
}

#[derive(Resource)]
struct WeaponAttachmentStatus {
    attached: bool,
    hand_entity: Option<Entity>,
}

#[derive(Resource)]
struct CameraShake {
    trauma: f32,
}

const MINIMAP_MARGIN_PX: f32 = 16.0;
const MINIMAP_WIDTH_PX: f32 = 272.0;
const MINIMAP_HEIGHT_PX: f32 = 192.0;
const MINIMAP_TEX_W: u32 = 544;
const MINIMAP_TEX_H: u32 = 384;

#[derive(Resource)]
struct CameraControlState {
    free_look_target: Option<Vec3>,
    camera_unlocked: bool,
    current_cursor_lead: Vec3,
    is_pointer_over_minimap: bool,
    is_dragging_minimap: bool,
}

#[derive(Component)]
struct MinimapSoldierDot;

#[derive(Component)]
struct MinimapTargetDot {
    index: usize,
}

#[derive(Component)]
struct MinimapCameraBox;

#[derive(Component)]
struct MinimapRoot;

#[derive(Component)]
struct MinimapCreepDot {
    creep_id: u64,
}

#[derive(Resource)]
struct LatestHeroSnapshot(HeroSnapshot);

#[derive(Component)]
struct LevelUpVfx {
    timer: Timer,
    is_ring: bool,
}

#[derive(Component)]
struct HeroHudHeaderText;

#[derive(Component)]
struct HeroHudGoldText;

#[derive(Component)]
struct HeroHudKdaText;

#[derive(Component)]
struct HeroHudLevelText;

#[derive(Component)]
struct HeroHudXpLabelText;

#[derive(Component)]
struct HeroHudXpSegment {
    index: usize,
}

#[derive(Component)]
struct HeroHpBarFill;

#[derive(Component)]
struct HeroHpBarText;

#[derive(Component)]
struct HeroStaminaBarFill;

#[derive(Component)]
struct HeroStaminaBarText;

#[derive(Component)]
struct HeroStatDmgText;

#[derive(Component)]
struct HeroStatRateText;

#[derive(Component)]
struct HeroStatRangeText;

#[derive(Component)]
struct HeroStatArmorText;

#[derive(Component)]
struct HeroStatSpeedText;

#[derive(Component)]
struct HeroStatVisionText;

#[derive(Component)]
struct HeroStatAmmoText;

#[derive(Component)]
struct HeroStatGrenadeText;

#[derive(Component)]
struct TreeVisualMarker {
    shadows_optimized: bool,
}

#[derive(Component)]
struct VisionOccludable {
    half_extents: Vec3,
    is_faded: bool,
}

#[derive(Component)]
struct OccludedMaterialState {
    original_handle: Handle<StandardMaterial>,
    original_alpha: f32,
    current_alpha: f32,
}

#[derive(Resource)]
struct FogOfWarState {
    shared: SharedFogOfWar,
    alpha_grid: Vec<f32>,
    fog_image: Handle<Image>,
}

#[derive(Resource)]
struct SoldierGltfHandle {
    model_path: String,
    handle: Handle<Gltf>,
}

#[derive(Resource)]
struct SoldierAnimationAssets {
    model_path: String,
    graph_handle: Handle<AnimationGraph>,
    nodes: HashMap<String, AnimationNodeIndex>,
    durations: HashMap<String, f32>,
}

impl SoldierAnimationAssets {
    fn require_node(&self, anim_name: &str) -> AnimationNodeIndex {
        *self.nodes.get(anim_name).unwrap_or_else(|| {
            panic!(
                "خطای بحرانی: کلیپ انیمیشن '{}' درون فایل GLB پیدا نشد!",
                anim_name
            )
        })
    }

    fn get_or_fallback(&self, anim_name: &str, fallback: &str) -> AnimationNodeIndex {
        if let Some(&node) = self.nodes.get(anim_name) {
            node
        } else {
            self.require_node(fallback)
        }
    }

    fn clip_duration(&self, anim_name: &str) -> Option<f32> {
        self.durations.get(anim_name).copied()
    }
}

#[derive(Resource)]
struct ClientRenderState {
    soldier_pos: Vec3,
    soldier_facing_dir: Vec3,
    aim_world_dir: Vec3,
    aim_hit_pos: Vec3,
    laser_hit_pos: Vec3,
    aim_locked_on_enemy: bool,
    hovered_creep_id: Option<u64>,
    hovered_turret_index: Option<usize>,
    hovered_barracks_index: Option<usize>,
    hovered_ancient_index: Option<usize>,
    current_locomotion: String,
    previous_lower_anim: String,
    previous_upper_anim: String,
    neutral_hips_yaw: Option<f32>,
    upper_action: UpperBodyAction,
    traversal_active: bool,
    traversal_anim_name: String,
    traversal_duration: f32,
    is_dead: bool,
    death_anim_name: String,
    in_water: bool,
    local_wade_ripple_timer: f32,
    fx_rng_seed: u32,
}

impl ClientRenderState {
    fn new_from_config(config: &GameConfig) -> Self {
        let spawn_pos = Vec3::new(
            config.soldier.spawn_pos.0,
            config.soldier.spawn_pos.1,
            config.soldier.spawn_pos.2,
        );
        let aim_dir = Vec3::new(
            config.soldier.initial_aim_dir.0,
            config.soldier.initial_aim_dir.1,
            config.soldier.initial_aim_dir.2,
        )
        .normalize();
        let aim_hit = Vec3::new(
            config.soldier.initial_aim_hit_pos.0,
            config.soldier.initial_aim_hit_pos.1,
            config.soldier.initial_aim_hit_pos.2,
        );
        Self {
            soldier_pos: spawn_pos,
            soldier_facing_dir: aim_dir,
            aim_world_dir: aim_dir,
            aim_hit_pos: aim_hit,
            laser_hit_pos: aim_hit,
            aim_locked_on_enemy: false,
            hovered_creep_id: None,
            hovered_turret_index: None,
            hovered_barracks_index: None,
            hovered_ancient_index: None,
            current_locomotion: "idle".to_string(),
            previous_lower_anim: String::new(),
            previous_upper_anim: String::new(),
            neutral_hips_yaw: None,
            upper_action: UpperBodyAction::None,
            traversal_active: false,
            traversal_anim_name: String::new(),
            traversal_duration: 0.0,
            is_dead: false,
            death_anim_name: String::new(),
            in_water: false,
            local_wade_ripple_timer: 0.0,
            fx_rng_seed: 123456789,
        }
    }

    fn next_fx_rand(&mut self) -> f32 {
        self.fx_rng_seed = self
            .fx_rng_seed
            .wrapping_mul(1664525)
            .wrapping_add(1013904223);
        (self.fx_rng_seed as f32) / (u32::MAX as f32)
    }
}

/// بررسی وجود فایل مدل ۳بعدی `.glb` در پوشه `assets`؛ اگر فایل وجود داشته باشد به‌صورت خودکار جایگزین مش پیش‌فرض Bevy می‌شود
fn custom_glb_exists(model_path: &str) -> bool {
    if model_path.trim().is_empty() {
        return false;
    }
    Path::new("assets").join(model_path).is_file() || Path::new(model_path).is_file()
}

fn model_transform_to_bevy(t: &ModelTransformConfig) -> Transform {
    Transform {
        translation: Vec3::new(t.location.0, t.location.1, t.location.2),
        rotation: Quat::from_euler(
            EulerRot::YXZ,
            t.rotation.1.to_radians(),
            t.rotation.0.to_radians(),
            t.rotation.2.to_radians(),
        ),
        scale: Vec3::new(t.scale.0, t.scale.1, t.scale.2),
    }
}

fn glb_model_bundle(
    asset_server: &AssetServer,
    model_path: &str,
    _model_transform: &ModelTransformConfig,
) -> (WorldAssetRoot, Transform, Visibility) {
    let scene_handle = asset_server.load(format!("{}#Scene0", model_path));
    (
        WorldAssetRoot(scene_handle),
        Transform::IDENTITY,
        Visibility::Visible,
    )
}

fn spawn_map_object_label(
    commands: &mut Commands,
    show_labels: bool,
    target_entity: Entity,
    label_text: &str,
    y_offset: f32,
) {
    if !show_labels {
        return;
    }
    commands.spawn((
        MapObjectLabel {
            target_entity,
            world_offset: Vec3::new(0.0, y_offset, 0.0),
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            padding: UiRect::axes(px(5.0), px(2.0)),
            border_radius: BorderRadius::all(px(4.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.07, 0.10, 0.75)),
        Visibility::Hidden,
        children![(
            Text::new(label_text),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.97, 1.0)),
        )],
    ));
}

// -----------------------------------------------------------------------------
// راه‌اندازی صحنه، نورپردازی سایه‌دار، برکه‌های طبیعی، آبشارها و جنگل انبوه
// -----------------------------------------------------------------------------

fn setup_scene(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut water_materials: ResMut<Assets<WaterSurfaceMaterial>>,
) {
    let spawn_pos = Vec3::new(
        config.soldier.spawn_pos.0,
        config.soldier.spawn_pos.1,
        config.soldier.spawn_pos.2,
    );
    let cam_offset = Vec3::new(
        config.camera.offset.0,
        config.camera.offset.1,
        config.camera.offset.2,
    );
    commands.spawn((
        Camera3d::default(),
        MobaCamera,
        DistanceFog {
            color: Color::srgba(0.68, 0.76, 0.84, 1.0),
            directional_light_color: Color::srgba(1.0, 0.92, 0.78, 0.45),
            directional_light_exponent: 24.0,
            falloff: FogFalloff::Linear {
                start: 85.0,
                end: 290.0,
            },
        },
        Transform::from_translation(spawn_pos + cam_offset).looking_at(spawn_pos, Vec3::Y),
    ));

    // خورشید طلایی-کوهستانی همراه با سایه‌های واقعی ۴ آبشاری (Cascaded Shadow Maps) و Contact Shadows
    let light_pos = Vec3::new(
        config.lighting.directional_pos.0,
        config.lighting.directional_pos.1,
        config.lighting.directional_pos.2,
    );
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: config.lighting.directional_illuminance,
            shadow_maps_enabled: true,
            contact_shadows_enabled: false,
            affects_lightmapped_mesh_diffuse: true,
            shadow_depth_bias: config.lighting.directional_shadow_depth_bias,
            shadow_normal_bias: config.lighting.directional_shadow_normal_bias,
            ..default()
        },
        Transform::from_translation(light_pos).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            minimum_distance: 0.5,
            maximum_distance: 130.0,
            first_cascade_far_bound: 26.0,
            overlap_proportion: 0.15,
        }
        .build(),
    ));

    let amb_c = config.lighting.ambient_color_rgb;
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(amb_c.0, amb_c.1, amb_c.2),
        brightness: config.lighting.ambient_brightness,
        affects_lightmapped_meshes: true,
    });

    // ۱. بستر سنگی عمیق و دریای ابر و مه پیرامون جزیره معلق (مطابق تصویر مرجع)
    let g_c = config.lighting.ground_color_rgb;
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(config.map.map_dimensions.0 * 1.35, config.map.map_dimensions.1 * 1.35),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(g_c.0, g_c.1, g_c.2),
            perceptual_roughness: config.lighting.ground_roughness,
            ..default()
        })),
        Transform::from_xyz(0.0, -5.5, 0.0),
    ));
    commands.spawn(ground_physics_bundle(&config));

    // لایه ابر و مه دره در اطراف لبه‌های پرتگاه نقشه
    let cloud_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.82, 0.87, 0.92, 0.52),
        emissive: LinearRgba::new(0.18, 0.22, 0.26, 0.52),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for &(cx, cz, rad) in &[
        (-155.0, -95.0, 42.0),
        (-155.0, 95.0, 42.0),
        (155.0, -95.0, 42.0),
        (155.0, 95.0, 42.0),
        (0.0, -118.0, 48.0),
        (0.0, 118.0, 48.0),
        (-95.0, -115.0, 38.0),
        (95.0, -115.0, 38.0),
        (-95.0, 115.0, 38.0),
        (95.0, 115.0, 38.0),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(rad, 1.2))),
            MeshMaterial3d(cloud_mat.clone()),
            Transform::from_xyz(cx, -3.8, cz),
            AmbientFloatingMote {
                base_pos: Vec3::new(cx, -3.8, cz),
                drift_amp: Vec3::new(2.5, 0.35, 2.0),
                freq: 0.35,
                phase: (cx + cz) * 0.05,
            },
        ));
    }

    // ۲. سطوح ارتفاعی زمین، حوضچه‌های برکه و پایگاه‌های دایره‌ای غرب (Dawn) و شرق (Dusk)
    for plat in &config.map.terrains {
        let size = Vec3::new(plat.size.0, plat.size.1, plat.size.2);
        let c = plat.color_rgb;
        let mut ent_cmds = commands.spawn((
            terrain_platform_physics_bundle(&config, plat),
            Visibility::Visible,
        ));
        let ent = ent_cmds.id();

        if custom_glb_exists(&plat.model_path) {
            ent_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &plat.model_path,
                    &plat.model_transform,
                ));
            });
        } else {
            ent_cmds.insert((
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(c.0, c.1, c.2),
                    perceptual_roughness: plat.roughness,
                    ..default()
                })),
            ));

            // اگر پلتفرم پایگاه اصلی غرب یا شرق است، حلقه‌های سنگی تزئینی و پرچم‌های تیم را اضافه می‌کنیم
            if plat.elevation_tier == 2 {
                let trim_color = if plat.team == TeamId::Dawn {
                    Color::srgb(0.22, 0.62, 0.52)
                } else {
                    Color::srgb(0.72, 0.24, 0.22)
                };
                let banner_emissive = if plat.team == TeamId::Dawn {
                    LinearRgba::new(0.15, 1.8, 1.2, 1.0)
                } else {
                    LinearRgba::new(2.2, 0.25, 0.15, 1.0)
                };
                ent_cmds.with_children(|parent| {
                    parent.spawn((
                        Mesh3d(meshes.add(Cylinder::new(23.5, 0.18))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.52, 0.51, 0.48),
                            perceptual_roughness: 0.72,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, size.y * 0.5 + 0.05, 0.0),
                    ));
                    parent.spawn((
                        Mesh3d(meshes.add(Torus::new(18.5, 19.6))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: trim_color,
                            emissive: banner_emissive * 0.35,
                            perceptual_roughness: 0.45,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, size.y * 0.5 + 0.12, 0.0),
                    ));
                    // ستون‌ها و پرچم‌های بلند پایگاه
                    for &bz in &[-18.0_f32, 18.0_f32] {
                        parent.spawn((
                            Mesh3d(meshes.add(Cuboid::new(0.4, 5.2, 1.6))),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: trim_color,
                                emissive: banner_emissive,
                                perceptual_roughness: 0.6,
                                ..default()
                            })),
                            Transform::from_xyz(0.0, size.y * 0.5 + 3.2, bz),
                        ));
                    }
                });
            }
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, ent, &plat.id, size.y * 0.5 + 0.8);
    }

    // ۳. رمپ‌ها و پلکان‌های سنگی
    for ramp in &config.map.ramps {
        let size = Vec3::new(ramp.size.0, ramp.size.1, ramp.size.2);
        let c = ramp.color_rgb;
        let mut ent_cmds = commands.spawn((
            ramp_physics_bundle(&config, ramp),
            Visibility::Visible,
        ));
        let ent = ent_cmds.id();
        if custom_glb_exists(&ramp.model_path) {
            ent_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &ramp.model_path,
                    &ramp.model_transform,
                ));
            });
        } else {
            ent_cmds.insert((
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(c.0, c.1, c.2),
                    perceptual_roughness: ramp.roughness,
                    ..default()
                })),
            ));
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, ent, &ramp.id, size.y * 0.5 + 0.8);
    }

    // ۴. برکه‌های فیروزه‌ای طبیعی، جویبارهای جنگلی و جریان پویا (River Segments & Lagoons)
    let streak_mesh = meshes.add(Cuboid::new(0.45, 0.03, 2.2));
    let streak_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.98, 1.0, 0.65),
        emissive: LinearRgba::new(0.65, 1.55, 1.95, 0.65),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let shore_rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.36, 0.39, 0.35),
        perceptual_roughness: 0.86,
        ..default()
    });

    let foam_noise_handle = images.add(bake_water_foam_noise_texture());
    commands.insert_resource(SharedWaterFoamTexture(foam_noise_handle.clone()));
    let subdivided_water_mesh = meshes.add(
        Plane3d::default()
            .mesh()
            .size(1.0, 1.0)
            .subdivisions(20),
    );

    for seg in &config.map.river_segments {
        let tf = seg.model_transform.to_transform();
        let pos = tf.translation;
        let size = Vec3::new(
            seg.size.0 * tf.scale.x,
            seg.size.1 * tf.scale.y,
            seg.size.2 * tf.scale.z,
        );
        let c = seg.color_rgb;
        let em = seg.emissive_rgb;
        let flow_n = Vec2::new(seg.flow_dir.0, seg.flow_dir.1).normalize_or_zero();

        let mut ent_cmds = commands.spawn((
            MapObjectId(seg.id.clone()),
            tf,
            Visibility::Visible,
        ));

        let seg_ent = ent_cmds.id();
        if custom_glb_exists(&seg.model_path) {
            ent_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &seg.model_path,
                    &seg.model_transform,
                ));
            });
        } else {
            let water_mat = water_materials.add(WaterSurfaceMaterial {
                uniforms: WaterUniforms {
                    base_color: Vec4::new(c.0, c.1, c.2, 0.76),
                    foam_color: Vec4::new(
                        (c.0 + 0.55 + em.0 * 0.25).min(1.0),
                        (c.1 + 0.42 + em.1 * 0.25).min(1.0),
                        (c.2 + 0.35 + em.2 * 0.25).min(1.0),
                        1.0,
                    ),
                    flow_and_params: Vec4::new(flow_n.x, flow_n.y, 0.0, 0.0),
                    player_ripple: Vec4::new(0.0, 0.0, 0.0, 0.14),
                },
                foam_tex: foam_noise_handle.clone(),
            });
            ent_cmds.with_children(|parent| {
                parent.spawn((
                    Mesh3d(subdivided_water_mesh.clone()),
                    MeshMaterial3d(water_mat),
                    Transform::from_xyz(0.0, size.y * 0.45, 0.0)
                        .with_scale(Vec3::new(size.x * 1.12, 1.0, size.z * 1.12)),
                ));
            });
        }

        spawn_map_object_label(&mut commands, config.map.show_object_labels, seg_ent, &seg.id, size.y * 0.5 + 0.8);

        // نور ملایم فیروزه‌ای روی برکه‌های اصلی و حوضچه مرکزی
        commands.spawn((
            PointLight {
                color: Color::srgb(0.20, 0.88, 0.95),
                intensity: 42_000.0,
                range: size.x.max(size.z) * 0.95,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(pos + Vec3::Y * 1.4),
        ));


    }

    // ۵. آبشارهای طبیعی چندطبقه همراه با ریزش متحرک آب، کف درخشان و مه آبشار
    let cascade_drop_mesh = meshes.add(Cuboid::new(0.75, 1.1, 0.18));
    let cascade_drop_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.97, 1.0, 0.82),
        emissive: LinearRgba::new(1.1, 2.1, 2.8, 0.82),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    for wf in &config.map.waterfalls {
        let tf = wf.model_transform.to_transform();
        let size = Vec3::new(wf.size.0, wf.size.1, wf.size.2);
        let c = wf.water_color_rgb;
        let em = wf.foam_emissive_rgb;

        let mut ent_cmds = commands.spawn((
            MapObjectId(wf.id.clone()),
            tf,
            Visibility::Visible,
        ));

        ent_cmds.insert(VisionOccludable { is_faded: false, half_extents: size * 0.5 });
        let wf_ent = ent_cmds.id();
        if custom_glb_exists(&wf.model_path) {
            ent_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &wf.model_path,
                    &wf.model_transform,
                ));
            });
        } else {
            let wf_mat = water_materials.add(WaterSurfaceMaterial {
                uniforms: WaterUniforms {
                    base_color: Vec4::new(c.0, c.1, c.2, 0.84),
                    foam_color: Vec4::new(
                        (0.85 + em.0 * 0.15).min(1.0),
                        (0.95 + em.1 * 0.10).min(1.0),
                        1.0,
                        1.0,
                    ),
                    flow_and_params: Vec4::new(0.0, 1.0, 0.0, 1.0),
                    player_ripple: Vec4::new(0.0, 0.0, 0.0, 0.22),
                },
                foam_tex: foam_noise_handle.clone(),
            });
            let tall_h = size.y * 1.65;
            let lift_y = (tall_h - size.y) * 0.5;
            ent_cmds.with_children(|parent| {
                parent.spawn((
                    Mesh3d(subdivided_water_mesh.clone()),
                    MeshMaterial3d(wf_mat),
                    Transform::from_xyz(0.0, lift_y, 0.0)
                        .with_rotation(Quat::from_rotation_x(PI * 0.5))
                        .with_scale(Vec3::new(size.x, 1.0, tall_h)),
                ));
            });
        }

        // سیستم ذرات مه در پای هر آبشار
        let mist_pos = tf.translation - Vec3::Y * (size.y * 0.45 * tf.scale.y);
        commands.spawn(WaterfallMistEmitter {
            base_pos: mist_pos,
            width: size.x * tf.scale.x,
            timer: Timer::from_seconds(wf.mist_particle_interval_secs.max(0.05), TimerMode::Repeating),
        });
        spawn_map_object_label(&mut commands, config.map.show_object_labels, wf_ent, &wf.id, size.y * 0.5 + 0.8);
    }

    // ۶. میدان باستانی دایره‌ای مرکز نقشه (Center Ruined Colosseum Nexus)
    let bridge = &config.map.center_bridge;
    let bc = bridge.color_rgb;
    let ac = bridge.accent_color_rgb;
    let mut bridge_cmds = commands.spawn((
        center_bridge_physics_bundle(&config, bridge),
        Visibility::Visible,
    ));

    let bridge_ent = bridge_cmds.id();
    if custom_glb_exists(&bridge.model_path) {
        bridge_cmds.with_children(|parent| {
            parent.spawn(glb_model_bundle(
                &asset_server,
                &bridge.model_path,
                &bridge.model_transform,
            ));
        });
    } else {
        bridge_cmds
            .insert((
                Mesh3d(meshes.add(Cylinder::new(bridge.outer_radius, bridge.deck_thickness))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(bc.0, bc.1, bc.2),
                    perceptual_roughness: bridge.roughness,
                    ..default()
                })),
            ))
            .with_children(|parent| {
                // حلقه‌های سنگی حکاکی‌شده دور حوضچه مرکزی
                parent.spawn((
                    Mesh3d(meshes.add(Torus::new(5.0, 5.8))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(ac.0, ac.1, ac.2),
                        perceptual_roughness: 0.65,
                        ..default()
                    })),
                    Transform::from_xyz(0.0, bridge.deck_thickness * 0.5 + 0.08, 0.0),
                ));
                parent.spawn((
                    Mesh3d(meshes.add(Torus::new(11.5, 12.2))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.25, 0.72, 0.75),
                        emissive: LinearRgba::new(0.2, 1.4, 1.6, 1.0),
                        perceptual_roughness: 0.35,
                        ..default()
                    })),
                    Transform::from_xyz(0.0, bridge.deck_thickness * 0.5 + 0.04, 0.0),
                ));
            });
    }

    spawn_map_object_label(&mut commands, config.map.show_object_labels, bridge_ent, &bridge.id, bridge.deck_thickness * 0.5 + 1.2);

    // ۷. مسیرهای سنگ‌فرش ۳ لاین اصلی (Top, Mid, Bot)
    for lane in &config.map.lanes {
        let lc = lane.color_rgb;
        let lane_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(lc.0, lc.1, lc.2),
            perceptual_roughness: lane.roughness,
            ..default()
        });
        let curb_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(lc.0 * 0.82, lc.1 * 0.82, lc.2 * 0.82),
            perceptual_roughness: 0.9,
            ..default()
        });
        for seg_idx in 0..lane.waypoints.len().saturating_sub(1) {
            let a = lane.waypoints[seg_idx];
            let b = lane.waypoints[seg_idx + 1];
            let start = Vec3::new(a.0, a.1, a.2);
            let end = Vec3::new(b.0, b.1, b.2);
            let len_3d = start.distance(end);
            if len_3d > 0.1 {
                commands
                    .spawn((
                        Mesh3d(meshes.add(Cuboid::new(lane.width, lane.thickness, len_3d))),
                        MeshMaterial3d(lane_mat.clone()),
                        lane_segment_physics_bundle(
                            &config,
                            format!("{}_seg_{}", lane.id, seg_idx),
                            start,
                            end,
                            lane.width,
                            lane.thickness,
                        ),
                        Visibility::Visible,
                    ))
                    .with_children(|parent| {
                        // لبه‌های سنگی دو طرف جاده (Curbs)
                        for &side in &[-1.0_f32, 1.0_f32] {
                            parent.spawn((
                                Mesh3d(meshes.add(Cuboid::new(
                                    0.45,
                                    lane.thickness * 1.18,
                                    len_3d,
                                ))),
                                MeshMaterial3d(curb_mat.clone()),
                                Transform::from_xyz(side * (lane.width * 0.5), 0.02, 0.0),
                            ));
                        }
                    });
            }
        }
    }

    // ۸. چشمه‌های مقدس پایگاه‌ها (Fountains)
    for ftn in &config.map.fountains {
        let mut tf = ftn.model_transform.to_transform();
        tf.translation.y += ftn.rim_height * 0.5 * tf.scale.y;
        let sc = ftn.stone_color_rgb;
        let we = ftn.water_emissive_rgb;
        let mut ftn_cmds = commands.spawn((
            MapObjectId(ftn.id.clone()),
            tf,
            Visibility::Visible,
        ));
        let ftn_ent = ftn_cmds.id();
        if custom_glb_exists(&ftn.model_path) {
            ftn_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &ftn.model_path,
                    &ftn.model_transform,
                ));
            });
        } else {
            ftn_cmds
                .insert((
                    Mesh3d(meshes.add(Cylinder::new(ftn.outer_radius, ftn.rim_height))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(sc.0, sc.1, sc.2),
                        perceptual_roughness: 0.65,
                        ..default()
                    })),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Mesh3d(meshes.add(Cylinder::new(
                            ftn.inner_radius,
                            ftn.rim_height * 1.12,
                        ))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.2, 0.85, 0.95),
                            emissive: LinearRgba::new(we.0, we.1, we.2, 1.0),
                            unlit: true,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, 0.05, 0.0),
                    ));
                });
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, ftn_ent, &ftn.id, ftn.rim_height * 0.5 + 1.0);
    }

    // ۹. کریستال‌های باستانی هر دو تیم (Ancients: زمردی در غرب، یاقوتی در شرق)
    for (anc_idx, anc) in config.map.ancients.iter().enumerate() {
        let bs = Vec3::new(anc.base_size.0, anc.base_size.1, anc.base_size.2);
        let c = anc.color_rgb;
        let ce = anc.core_emissive_rgb;
        let light_col = if anc.team == TeamId::Dawn {
            Color::srgb(0.18, 0.95, 0.68)
        } else {
            Color::srgb(1.0, 0.25, 0.18)
        };

        let mut anc_cmds = commands.spawn((
            ancient_physics_bundle(&config, anc_idx, anc),
            AncientVisualRoot {
                index: anc_idx,
                is_dead: false,
                is_highlighted: false,
            },
            Visibility::Visible,
        ));

        anc_cmds.insert(VisionOccludable { is_faded: false, half_extents: Vec3::new(bs.x * 0.5, anc.core_height * 0.6, bs.z * 0.5) });
        let anc_ent = anc_cmds.id();
        if custom_glb_exists(&anc.model_path) {
            anc_cmds.with_children(|parent| {
                let mut glb = glb_model_bundle(
                    &asset_server,
                    &anc.model_path,
                    &anc.model_transform,
                );
                glb.1.translation.y = -anc.core_height * 0.5;
                parent.spawn(glb);
            });
        } else {
            anc_cmds
                .insert((
                    Mesh3d(meshes.add(Cylinder::new(bs.x * 0.55, bs.y))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(c.0, c.1, c.2),
                        metallic: 0.35,
                        perceptual_roughness: 0.45,
                        ..default()
                    })),
                ))
                .with_children(|parent| {
                    // پله دوم محراب سنگی
                    parent.spawn((
                        Mesh3d(meshes.add(Cylinder::new(bs.x * 0.38, bs.y * 0.65))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(c.0 * 1.15, c.1 * 1.15, c.2 * 1.15),
                            perceptual_roughness: 0.4,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, bs.y * 0.45, 0.0),
                    ));

                    // منشور کریستال عظیم مرکزی (در حال چرخش و شناور بودن)
                    let base_crystal_y = bs.y * 0.5 + anc.core_height * 0.52;
                    parent
                        .spawn((
                            Mesh3d(meshes.add(Cone {
                                radius: anc.core_radius,
                                height: anc.core_height,
                            })),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: light_col,
                                emissive: LinearRgba::new(ce.0, ce.1, ce.2, 1.0),
                                perceptual_roughness: 0.1,
                                metallic: 0.2,
                                ..default()
                            })),
                            Transform::from_xyz(0.0, base_crystal_y, 0.0),
                            AnimatedCrystalCore {
                                base_y: base_crystal_y,
                                bob_amp: 0.32,
                                bob_speed: 1.8,
                                rot_speed: 0.9,
                                phase: 0.0,
                            },
                        ))
                        .with_children(|crystal| {
                            // مخروط معکوس پایینی برای شکل منشوری کامل کریستال
                            crystal.spawn((
                                Mesh3d(meshes.add(Cone {
                                    radius: anc.core_radius * 0.92,
                                    height: anc.core_height * 0.45,
                                })),
                                MeshMaterial3d(materials.add(StandardMaterial {
                                    base_color: light_col,
                                    emissive: LinearRgba::new(ce.0, ce.1, ce.2, 1.0),
                                    unlit: true,
                                    ..default()
                                })),
                                Transform::from_xyz(0.0, -anc.core_height * 0.35, 0.0)
                                    .with_rotation(Quat::from_rotation_x(PI)),
                            ));
                        });

                    // نور نقطه‌ای قدرتمند کریستال Ancient
                    parent.spawn((
                        PointLight {
                            color: light_col,
                            intensity: 320_000.0,
                            range: 28.0,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_xyz(0.0, base_crystal_y + 1.5, 0.0),
                    ));
                });
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, anc_ent, &anc.id, anc.base_size.1 * 0.5 + anc.core_height + 1.0);
    }

    // ۱۰. ۱۸ برج دفاعی (Turrets) در هر ۳ لاین مطابق تصویر مرجع
    for (t_idx, tur) in config.map.turrets.iter().enumerate() {
        let tc = tur.color_rgb;
        let ce = tur.crystal_emissive_rgb;
        let mut tur_cmds = commands.spawn((
            turret_physics_bundle(&config, t_idx, tur),
            TurretVisualRoot {
                index: t_idx,
                is_dead: false,
                is_highlighted: false,
            },
            Visibility::Visible,
        ));

        tur_cmds.insert(VisionOccludable { is_faded: false, half_extents: Vec3::new(tur.base_radius * 1.4, tur.height * 0.55, tur.base_radius * 1.4) });
        let tur_ent = tur_cmds.id();
        if custom_glb_exists(&tur.model_path) {
            tur_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &tur.model_path,
                    &tur.model_transform,
                ));
            });
        } else {
            tur_cmds
                .insert((
                    Mesh3d(meshes.add(Cylinder::new(tur.base_radius, tur.height))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(tc.0, tc.1, tc.2),
                        metallic: 0.35,
                        perceptual_roughness: 0.55,
                        ..default()
                    })),
                ))
                .with_children(|parent| {
                    // پایه سنگی پهن‌تر برج
                    parent.spawn((
                        Mesh3d(meshes.add(Cylinder::new(
                            tur.base_radius * 1.32,
                            tur.height * 0.28,
                        ))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(tc.0 * 0.85, tc.1 * 0.85, tc.2 * 0.85),
                            perceptual_roughness: 0.75,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, -tur.height * 0.36, 0.0),
                    ));
                    // تاج بالایی برج
                    parent.spawn((
                        Mesh3d(meshes.add(Torus::new(
                            tur.base_radius * 0.85,
                            tur.base_radius * 1.18,
                        ))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(tc.0 * 1.1, tc.1 * 1.1, tc.2 * 1.1),
                            perceptual_roughness: 0.5,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, tur.height * 0.46, 0.0),
                    ));
                    // گوی کریستالی شناور بالای برج
                    let crystal_y = tur.height * 0.5 + tur.head_radius * 1.05;
                    parent.spawn((
                        Mesh3d(meshes.add(Sphere::new(tur.head_radius))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::WHITE,
                            emissive: LinearRgba::new(ce.0, ce.1, ce.2, 1.0),
                            unlit: true,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, crystal_y, 0.0),
                        AnimatedCrystalCore {
                            base_y: crystal_y,
                            bob_amp: 0.16,
                            bob_speed: 2.4,
                            rot_speed: 1.5,
                            phase: (t_idx as f32) * 0.5,
                        },
                    ));
                });
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, tur_ent, &tur.id, tur.height * 0.5 + tur.head_radius * 2.0 + 1.0);
    }

    // ۱۰-ب. ۱۲ باراکس (۲ ساختمان پشت هر تاور Tier 3 در هر لاین)
    for (b_idx, bar) in config.map.barracks.iter().enumerate() {
        let bs = Vec3::new(bar.size.0, bar.size.1, bar.size.2);
        let bc = bar.color_rgb;
        let ae = bar.accent_emissive_rgb;
        let mut bar_cmds = commands.spawn((
            barracks_physics_bundle(&config, b_idx, bar),
            BarracksVisualRoot {
                index: b_idx,
                is_dead: false,
                is_highlighted: false,
            },
            Visibility::Visible,
        ));
        bar_cmds.insert(VisionOccludable {
            is_faded: false,
            half_extents: bs * 0.5,
        });
        let bar_ent = bar_cmds.id();
        if custom_glb_exists(&bar.model_path) {
            bar_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &bar.model_path,
                    &bar.model_transform,
                ));
            });
        } else {
            let accent_col = if bar.team == TeamId::Dawn {
                Color::srgb(0.20, 0.92, 0.72)
            } else {
                Color::srgb(0.98, 0.30, 0.24)
            };
            bar_cmds
                .insert((
                    Mesh3d(meshes.add(Cuboid::new(bs.x, bs.y, bs.z))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(bc.0, bc.1, bc.2),
                        metallic: 0.25,
                        perceptual_roughness: 0.65,
                        ..default()
                    })),
                ))
                .with_children(|parent| {
                    // سقف/تاج متمایز برای باراکس Melee (زره/سلامت) و Ranged (قدرت آتش)
                    if bar.kind == BarracksKind::Melee {
                        parent.spawn((
                            Mesh3d(meshes.add(Cuboid::new(bs.x * 0.72, bs.y * 0.32, bs.z * 0.72))),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: accent_col,
                                emissive: LinearRgba::new(ae.0, ae.1, ae.2, 1.0),
                                perceptual_roughness: 0.35,
                                ..default()
                            })),
                            Transform::from_xyz(0.0, bs.y * 0.62, 0.0),
                        ));
                    } else {
                        parent.spawn((
                            Mesh3d(meshes.add(Cone {
                                radius: bs.x * 0.38,
                                height: bs.y * 0.55,
                            })),
                            MeshMaterial3d(materials.add(StandardMaterial {
                                base_color: accent_col,
                                emissive: LinearRgba::new(ae.0, ae.1, ae.2, 1.0),
                                perceptual_roughness: 0.30,
                                ..default()
                            })),
                            Transform::from_xyz(0.0, bs.y * 0.72, 0.0),
                        ));
                    }
                });
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, bar_ent, &bar.id, bs.y * 0.5 + 1.2);
    }

    // ۱۱. کوه‌ها، صخره‌های چندلایه خزه‌دار و باروهای سنگی دور نقشه
    for mtn in &config.map.mountains {
        let size = Vec3::new(mtn.size.0, mtn.size.1, mtn.size.2);
        let mc = mtn.color_rgb;
        let mut mtn_cmds = commands.spawn((
            mountain_physics_bundle(&config, mtn),
            Visibility::Visible,
        ));

        mtn_cmds.insert(VisionOccludable { is_faded: false, half_extents: size * 0.5 });
        let mtn_ent = mtn_cmds.id();
        if custom_glb_exists(&mtn.model_path) {
            mtn_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &mtn.model_path,
                    &mtn.model_transform,
                ));
            });
        } else {
            mtn_cmds
                .insert((
                    Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(mc.0, mc.1, mc.2),
                        perceptual_roughness: mtn.roughness,
                        ..default()
                    })),
                ))
                .with_children(|parent| {
                    // لایه صخره‌ای پله‌ای و پوشش خزه سبز روی قله صخره‌ها
                    parent.spawn((
                        Mesh3d(meshes.add(Cuboid::new(
                            size.x * 0.76,
                            size.y * 0.28,
                            size.z * 0.76,
                        ))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(mc.0 * 0.92, mc.1 * 0.92, mc.2 * 0.92),
                            perceptual_roughness: 0.92,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, size.y * 0.52, 0.0),
                    ));
                    parent.spawn((
                        Mesh3d(meshes.add(Cuboid::new(
                            size.x * 0.72,
                            0.24,
                            size.z * 0.72,
                        ))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgb(0.24, 0.36, 0.20),
                            perceptual_roughness: 0.94,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, size.y * 0.66, 0.0),
                    ));
                });
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, mtn_ent, &mtn.id, size.y * 0.5 + 1.5);
    }

    // ۱۲. جنگل انبوه درختان کاج چندطبقه (۸۸ درخت در سراسر برکه‌ها و مسیرهای جنگلی)
    for tree in &config.map.trees {
        let tc = tree.trunk_color_rgb;
        let cc = tree.canopy_color_rgb;
        let mut tree_cmds = commands.spawn((
            tree_physics_bundle(&config, tree),
            Visibility::Visible,
        ));

        tree_cmds.insert(VisionOccludable { is_faded: false, half_extents: Vec3::new(tree.canopy_radius, (tree.trunk_height + tree.canopy_height) * 0.5, tree.canopy_radius) });
        tree_cmds.insert(TreeVisualMarker { shadows_optimized: false });
        let tree_ent = tree_cmds.id();
        if custom_glb_exists(&tree.model_path) {
            tree_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &tree.model_path,
                    &tree.model_transform,
                ));
            });
        } else {
            let tier1_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(cc.0 * 0.88, cc.1 * 0.88, cc.2 * 0.88),
                perceptual_roughness: 0.88,
                ..default()
            });
            let tier2_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(cc.0, cc.1, cc.2),
                perceptual_roughness: 0.85,
                ..default()
            });
            let tier3_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(
                    (cc.0 * 1.12).min(1.0),
                    (cc.1 * 1.12).min(1.0),
                    (cc.2 * 1.12).min(1.0),
                ),
                perceptual_roughness: 0.82,
                ..default()
            });

            tree_cmds
                .insert((
                    Mesh3d(meshes.add(Cylinder::new(tree.trunk_radius, tree.trunk_height))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(tc.0, tc.1, tc.2),
                        perceptual_roughness: 0.92,
                        ..default()
                    })),
                ))
                .with_children(|parent| {
                    // ۳ طبقه مخروطی شاخ‌وبرگ کاج برای ظاهر طبیعی و پرپشت
                    parent.spawn((
                        Mesh3d(meshes.add(Cone {
                            radius: tree.canopy_radius * 1.05,
                            height: tree.canopy_height * 0.52,
                        })),
                        MeshMaterial3d(tier1_mat),
                        Transform::from_xyz(
                            0.0,
                            tree.trunk_height * 0.42 + tree.canopy_height * 0.18,
                            0.0,
                        ),
                    ));
                    parent.spawn((
                        Mesh3d(meshes.add(Cone {
                            radius: tree.canopy_radius * 0.82,
                            height: tree.canopy_height * 0.48,
                        })),
                        MeshMaterial3d(tier2_mat),
                        Transform::from_xyz(
                            0.0,
                            tree.trunk_height * 0.42 + tree.canopy_height * 0.46,
                            0.0,
                        ),
                    ));
                    parent.spawn((
                        Mesh3d(meshes.add(Cone {
                            radius: tree.canopy_radius * 0.56,
                            height: tree.canopy_height * 0.42,
                        })),
                        MeshMaterial3d(tier3_mat),
                        Transform::from_xyz(
                            0.0,
                            tree.trunk_height * 0.42 + tree.canopy_height * 0.72,
                            0.0,
                        ),
                    ));
                });
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, tree_ent, &tree.id, tree.trunk_height * 0.5 + tree.canopy_height + 0.8);
    }

    // ۱۳. خرابه‌های باستانی، دیوارهای شکسته و سنگرهای تاکتیکی (Obstacles)
    for obs in &config.obstacles {
        let size = Vec3::new(obs.size.0, obs.size.1, obs.size.2);
        let oc = obs.color_rgb;
        let mut obs_cmds = commands.spawn((
            obstacle_physics_bundle(&config, obs),
            Visibility::Visible,
        ));

        obs_cmds.insert(VisionOccludable { is_faded: false, half_extents: size * 0.5 });
        let obs_ent = obs_cmds.id();
        if custom_glb_exists(&obs.model_path) {
            obs_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &obs.model_path,
                    &obs.model_transform,
                ));
            });
        } else {
            obs_cmds.insert((
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(oc.0, oc.1, oc.2),
                    perceptual_roughness: obs.roughness,
                    ..default()
                })),
            ));
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, obs_ent, &obs.id, size.y * 0.5 + 0.8);
    }

    // ۱۴. اهداف تیراندازی
    for (index, t) in config.targets.iter().enumerate() {
        let pos = t.model_transform.location_vec();
        let size = Vec3::new(t.size.0, t.size.1, t.size.2);
        let color = Color::srgb(t.color_rgb.0, t.color_rgb.1, t.color_rgb.2);
        let mut target_cmds = commands.spawn((
            Visibility::Visible,
            VisualTarget {
                index,
                max_hp: t.max_hp,
                hp: t.max_hp,
                hit_flash_timer: 0.0,
                base_pos: pos,
                base_scale: t.model_transform.scale_vec(),
                base_color: color,
                is_dead: false,
                is_server_visible: true,
            },
            target_physics_bundle(&config, index, t),
        ));

        let target_ent = target_cmds.id();
        if custom_glb_exists(&t.model_path) {
            target_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &t.model_path,
                    &t.model_transform,
                ));
            });
        } else {
            target_cmds.insert((
                Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color,
                    perceptual_roughness: t.roughness,
                    metallic: t.metallic,
                    ..default()
                })),
            ));
        }
        spawn_map_object_label(&mut commands, config.map.show_object_labels, target_ent, &t.id, size.y * 0.5 + 0.8);
    }

    // ۱۵. ذرات معلق جادویی و شب‌تاب‌های درخشان بالای برکه‌ها و جنگل (Ambient VFX Motes)
    let firefly_mesh = meshes.add(Sphere::new(0.09));
    let cyan_mote_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 1.0, 0.9),
        emissive: LinearRgba::new(1.5, 6.5, 5.5, 1.0),
        unlit: true,
        ..default()
    });
    let gold_mote_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.4),
        emissive: LinearRgba::new(5.5, 4.2, 0.8, 1.0),
        unlit: true,
        ..default()
    });

    for i in 0..36 {
        let angle = (i as f32) * 0.52;
        let r = 12.0 + ((i % 7) as f32) * 11.5;
        let base = Vec3::new(
            angle.cos() * r * 1.15,
            1.4 + ((i % 4) as f32) * 0.65,
            angle.sin() * r * 0.78,
        );
        commands.spawn((
            Mesh3d(firefly_mesh.clone()),
            MeshMaterial3d(if i % 2 == 0 {
                cyan_mote_mat.clone()
            } else {
                gold_mote_mat.clone()
            }),
            Transform::from_translation(base),
            AmbientFloatingMote {
                base_pos: base,
                drift_amp: Vec3::new(1.2, 0.55, 1.2),
                freq: 1.1 + (i % 5) as f32 * 0.22,
                phase: i as f32 * 0.7,
            },
        ));
    }

    // ۱۶. ساخت سیستم Fog of War (مه جنگ Dota 2 با برتری ارتفاع و موانع دید)
    let fow_state = init_fog_of_war(&config, &mut images);
    let fog_handle = fow_state.fog_image.clone();
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::default()
                    .mesh()
                    .size(config.map.map_dimensions.0, config.map.map_dimensions.1),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(fog_handle.clone()),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 2.75, 0.0),
    ));
    commands.insert_resource(fow_state);

    // ۱۷. ساخت تکسچر دقیق مینی‌مپ از تمام مسیرها و آبجکت‌های نقشه و نمایش آن در پایین سمت راست
    let minimap_image = bake_minimap_image(&config);
    let minimap_handle = images.add(minimap_image);
    spawn_minimap_ui(&mut commands, &config, minimap_handle, fog_handle);
    spawn_hero_hud_ui(&mut commands);
    spawn_top_match_hud_ui(&mut commands);
    spawn_scoreboard_ui(&mut commands, &config);
    spawn_death_screen_overlay(&mut commands);
    spawn_end_of_match_overlay(&mut commands);
    spawn_chat_ui(&mut commands);
    spawn_pause_overlay_ui(&mut commands);
    spawn_telemetry_hud_ui(&mut commands);
    spawn_match_overlay_ui(&mut commands, &config);

    let shared_fx = SharedFxAssets {
        bullet_mesh: meshes.add(Cone {
            radius: 0.028,
            height: 0.42,
        }),
        bullet_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.92, 0.45),
            emissive: LinearRgba::new(14.0, 8.5, 1.2, 1.0),
            unlit: true,
            ..default()
        }),
        muzzle_flash_mesh: meshes.add(Cone {
            radius: 0.07,
            height: 0.28,
        }),
        muzzle_flash_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.2),
            emissive: LinearRgba::new(16.0, 9.0, 1.5, 1.0),
            unlit: true,
            ..default()
        }),
        shell_mesh: meshes.add(Cylinder::new(0.014, 0.055)),
        shell_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.72, 0.28),
            metallic: 0.9,
            perceptual_roughness: 0.25,
            ..default()
        }),
    };
    for _ in 0..64 {
        commands.spawn((
            Mesh3d(shared_fx.bullet_mesh.clone()),
            MeshMaterial3d(shared_fx.bullet_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
            NetworkBulletVisual {
                server_id: 0,
                visual_offset: Vec3::ZERO,
                visual_offset_blend: 0.0,
            },
        ));
    }
    commands.insert_resource(shared_fx);
    spawn_laser_and_reticle_visuals(
        &mut commands,
        &config,
        &mut meshes,
        &mut materials,
    );

    let gltf_handle = asset_server.load(&config.animations.model_path);
    commands.insert_resource(SoldierGltfHandle {
        model_path: config.animations.model_path.clone(),
        handle: gltf_handle,
    });
    let mut creep_anim_cache = CreepModelAnimCache::default();
    creep_anim_cache.register(
        "lane_dawn",
        &config.creeps.dawn_model_path,
        &config.creeps.anim_run,
        &config.creeps.anim_idle,
        &config.creeps.anim_fire,
        &config.creeps.anim_die,
        &asset_server,
    );
    creep_anim_cache.register(
        "lane_dusk",
        &config.creeps.dusk_model_path,
        &config.creeps.anim_run,
        &config.creeps.anim_idle,
        &config.creeps.anim_fire,
        &config.creeps.anim_die,
        &asset_server,
    );
    for (tid, ct) in &config.neutral_camps.creep_types {
        creep_anim_cache.register(
            tid,
            &ct.model_path,
            &ct.anim_run,
            &ct.anim_idle,
            &ct.anim_fire,
            &ct.anim_die,
            &asset_server,
        );
    }
    commands.insert_resource(creep_anim_cache);
    let dawn_tur_path = config
        .map
        .turrets
        .iter()
        .find(|t| t.team == TeamId::Dawn)
        .map(|t| t.model_path.clone())
        .unwrap_or_else(|| config.creeps.dawn_model_path.clone());
    let dusk_tur_path = config
        .map
        .turrets
        .iter()
        .find(|t| t.team == TeamId::Dusk)
        .map(|t| t.model_path.clone())
        .unwrap_or_else(|| dawn_tur_path.clone());
    commands.insert_resource(TurretGltfHandles {
        dawn: asset_server.load(&dawn_tur_path),
        dusk: asset_server.load(&dusk_tur_path),
    });
}

// -----------------------------------------------------------------------------
// سیستم پویایی محیط، جریان آب، آبشارها، حلقه‌های موج آب و تعامل ورود بازیکن به آب
// -----------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn environment_vfx_and_water_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    mut streak_query: Query<(&mut Transform, &FlowingWaterStreak), Without<CascadingWaterfallSheet>>,
    mut waterfall_query: Query<
        (&mut Transform, &CascadingWaterfallSheet),
        Without<FlowingWaterStreak>,
    >,
    mut crystal_query: Query<
        (&mut Transform, &AnimatedCrystalCore),
        (Without<FlowingWaterStreak>, Without<CascadingWaterfallSheet>),
    >,
    mut mote_query: Query<
        (&mut Transform, &AmbientFloatingMote),
        (
            Without<FlowingWaterStreak>,
            Without<CascadingWaterfallSheet>,
            Without<AnimatedCrystalCore>,
        ),
    >,
    mut ripple_query: Query<
        (Entity, &mut Transform, &mut WaterRippleRing),
        (
            Without<FlowingWaterStreak>,
            Without<CascadingWaterfallSheet>,
            Without<AnimatedCrystalCore>,
            Without<AmbientFloatingMote>,
        ),
    >,
) {
    let t = time.elapsed_secs();

    // ۱. حرکت پیوسته رگه‌های جریان آب روی برکه‌ها و جویبارها
    for (mut tf, streak) in &mut streak_query {
        let u = ((t / streak.cycle_secs) + streak.phase).fract();
        tf.translation = streak.origin + streak.travel_vec * u;
        let pulse = (u * PI).sin().max(0.15);
        tf.scale = Vec3::new(1.0, 1.0, 0.6 + pulse * 0.7);
    }

    // ۲. ریزش پیوسته ورقه‌های آبشارها
    for (mut tf, sheet) in &mut waterfall_query {
        let u = (t * sheet.speed + sheet.phase).fract();
        tf.translation = sheet.top_pos - Vec3::Y * (u * sheet.drop_height);
    }

    // ۳. چرخش و شناور بودن کریستال‌های Ancient و برج‌های دفاعی
    for (mut tf, crystal) in &mut crystal_query {
        tf.translation.y =
            crystal.base_y + (t * crystal.bob_speed + crystal.phase).sin() * crystal.bob_amp;
        tf.rotation = Quat::from_rotation_y(t * crystal.rot_speed + crystal.phase);
    }

    // ۴. حرکت نرم ذرات معلق جادویی و ابرهای اطراف دره
    for (mut tf, mote) in &mut mote_query {
        let phase = t * mote.freq + mote.phase;
        tf.translation = mote.base_pos
            + Vec3::new(
                phase.sin() * mote.drift_amp.x,
                (phase * 1.3).cos() * mote.drift_amp.y,
                (phase * 0.85).cos() * mote.drift_amp.z,
            );
    }

    // ۵. گسترش و محو شدن حلقه‌های موج سطح آب (Water Ripple Rings)
    for (entity, mut tf, mut ripple) in &mut ripple_query {
        ripple.timer.tick(time.delta());
        let frac = ripple.timer.fraction();
        let s = ripple.start_scale.lerp(ripple.end_scale, frac);
        tf.scale = Vec3::new(s, (1.0 - frac).max(0.05), s);
        if ripple.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }


}

fn spawn_water_ripple_and_droplets(
    commands: &mut Commands,
    config: &GameConfig,
    render_state: &mut ClientRenderState,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    water_pos: Vec3,
    intensity: f32,
) {
    // فقط ذرات ظریف پاشش آب (بدون حلقه‌ها و دایره‌های مزاحم دور کاراکتر)
    if intensity >= 0.9 {
        let drop_count = ((intensity * 9.0) as u32).clamp(6, 18);
        let drop_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.88, 0.92, 0.55),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.15,
            ..default()
        });
        let drop_mesh = meshes.add(Sphere::new(0.045));

        for _ in 0..drop_count {
            let rx = render_state.next_fx_rand();
            let ry = render_state.next_fx_rand();
            let rz = render_state.next_fx_rand();
            let vel = Vec3::new(
                (rx - 0.5) * 3.0 * intensity,
                (1.8 + ry * 2.2) * intensity,
                (rz - 0.5) * 3.0 * intensity,
            );
            commands.spawn((
                Mesh3d(drop_mesh.clone()),
                MeshMaterial3d(drop_mat.clone()),
                particle_physics_bundle(config, water_pos + Vec3::Y * 0.08, vel, 0.38, true),
            ));
        }
    }
}

// -----------------------------------------------------------------------------
// بارگذاری انیمیشن‌ها همراه با بررسی وجود تمام کلیپ‌های تعریف‌شده در کانفیگ
// -----------------------------------------------------------------------------

fn resolve_local_selected_hero<'a>(
    config: &'a GameConfig,
    match_flow: &ClientMatchFlow,
    hero_hud: &LatestHeroSnapshot,
) -> Option<&'a moba_shared::HeroDef> {
    if let (Some(slot), Some(snap)) = (match_flow.your_slot, match_flow.snapshot.as_ref()) {
        if let Some(lp) = snap.players.get(slot) {
            if let Some(ref hid) = lp.selected_hero_id {
                if let Some(h) = config.heroes.iter().find(|h| &h.id == hid) {
                    return Some(h);
                }
            }
        }
    }
    if match_flow.is_in_game() {
        if let Some(h) = config.heroes.iter().find(|h| h.id == hero_hud.0.hero_id) {
            return Some(h);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn load_gltf_and_build_graph(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    match_flow: Res<ClientMatchFlow>,
    hero_hud: Res<LatestHeroSnapshot>,
    mut render_state: ResMut<ClientRenderState>,
    mut weapon_status: ResMut<WeaponAttachmentStatus>,
    mut gltf_handle: Option<ResMut<SoldierGltfHandle>>,
    gltf_assets: Res<Assets<Gltf>>,
    clip_assets: Res<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    assets_ready: Option<Res<SoldierAnimationAssets>>,
    old_soldier_q: Query<Entity, With<SoldierMarker>>,
    old_upper_q: Query<Entity, With<UpperBodyAnimationPlayer>>,
) {
    let selected_hero_opt = resolve_local_selected_hero(&config, &match_flow, &hero_hud);
    let desired_model_path = selected_hero_opt
        .map(|h| h.model_path.clone())
        .unwrap_or_else(|| config.animations.model_path.clone());
    let desired_scale = selected_hero_opt
        .map(|h| h.hero_scale)
        .unwrap_or_else(|| config.active_hero(0).hero_scale);

    let Some(ref mut handle_res) = gltf_handle else { return };
    if handle_res.model_path != desired_model_path {
        handle_res.model_path = desired_model_path.clone();
        handle_res.handle = asset_server.load(&desired_model_path);
    }

    if let Some(ref ready) = assets_ready {
        if ready.model_path == desired_model_path && !old_soldier_q.is_empty() {
            return;
        }
    }

    let Some(gltf) = gltf_assets.get(&handle_res.handle) else { return };

    let mut graph = AnimationGraph::new();
    let mut nodes = HashMap::new();
    let mut durations = HashMap::new();

    for (name, clip_handle) in &gltf.named_animations {
        let Some(clip) = clip_assets.get(clip_handle) else {
            return;
        };
        let node_index = graph.add_clip(clip_handle.clone(), 1.0, graph.root);
        nodes.insert(name.to_string(), node_index);
        durations.insert(name.to_string(), clip.duration());
        println!(
            "انیمیشن بارگذاری شد ({}): '{}' (مدت‌زمان GLB: {:.3}s)",
            desired_model_path,
            name,
            clip.duration()
        );
    }

    for expected_anim in config.animations.clip_durations.keys() {
        assert!(
            nodes.contains_key(expected_anim),
            "خطای بحرانی: انیمیشن '{}' که در config.ron تعریف شده در فایل '{}' وجود ندارد!",
            expected_anim,
            desired_model_path
        );
    }

    for ent in &old_soldier_q {
        commands.entity(ent).despawn();
    }
    for ent in &old_upper_q {
        commands.entity(ent).despawn();
    }
    weapon_status.attached = false;
    weapon_status.hand_entity = None;
    render_state.previous_lower_anim.clear();
    render_state.previous_upper_anim.clear();
    render_state.neutral_hips_yaw = None;

    let graph_handle = animation_graphs.add(graph);
    commands.insert_resource(SoldierAnimationAssets {
        model_path: desired_model_path,
        graph_handle,
        nodes,
        durations,
    });

    let spawn_pos = render_state.soldier_pos;
    commands.spawn((
        WorldAssetRoot(gltf.scenes[0].clone()),
        SoldierMarker,
        Transform::from_translation(spawn_pos).with_scale(Vec3::splat(desired_scale)),
    ));
}

fn is_upper_body_bone_name(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("spine")
        || n.contains("chest")
        || n.contains("torso")
        || n.contains("rib")
        || n.contains("neck")
        || n.contains("head")
        || n.contains("shoulder")
        || n.contains("clavicle")
        || n.contains("collar")
        || n.contains("arm")
        || n.contains("forearm")
        || n.contains("elbow")
        || n.contains("hand")
        || n.contains("wrist")
        || n.contains("palm")
        || n.contains("finger")
        || n.contains("thumb")
        || n.contains("index")
        || n.contains("middle")
        || n.contains("ring")
        || n.contains("pinky")
}

fn is_upper_body_in_hierarchy(
    entity: Entity,
    names: &Query<&Name>,
    parents: &Query<&ChildOf>,
) -> bool {
    let mut curr = entity;
    for _ in 0..32 {
        if let Ok(name) = names.get(curr) {
            let n = name.as_str().to_lowercase();
            if n.contains("hips")
                || n.contains("pelvis")
                || n.contains("upleg")
                || n.contains("thigh")
                || n.contains("calf")
                || n.contains("foot")
            {
                return false;
            }
            if is_upper_body_bone_name(name.as_str()) {
                return true;
            }
        }
        if let Ok(child_of) = parents.get(curr) {
            curr = child_of.parent();
        } else {
            break;
        }
    }
    false
}

fn attach_split_body_animation_graphs(
    mut commands: Commands,
    all_players_query: Query<(Entity, Option<&AnimationGraphHandle>, Option<&LowerBodyAnimationPlayer>), With<AnimationPlayer>>,
    soldier_marker_query: Query<Entity, With<SoldierMarker>>,
    upper_player_query: Query<Entity, With<UpperBodyAnimationPlayer>>,
    mut target_query: Query<(Entity, &mut AnimatedBy)>,
    names_query: Query<&Name>,
    parents_query: Query<&ChildOf>,
    anim_assets: Option<Res<SoldierAnimationAssets>>,
) {
    let Some(anim_assets) = anim_assets else { return };

    let mut lower_ent_opt: Option<Entity> = None;
    for (p_ent, graph_opt, lower_opt) in &all_players_query {
        let mut cur = p_ent;
        let mut belongs_to_soldier = soldier_marker_query.contains(cur);
        while !belongs_to_soldier {
            if let Ok(parent) = parents_query.get(cur) {
                cur = parent.parent();
                if soldier_marker_query.contains(cur) {
                    belongs_to_soldier = true;
                    break;
                }
            } else {
                break;
            }
        }
        if !belongs_to_soldier {
            continue;
        }
        lower_ent_opt = Some(p_ent);
        if lower_opt.is_none() || graph_opt.map_or(true, |g| g.0 != anim_assets.graph_handle) {
            commands.entity(p_ent).insert((
                AnimationGraphHandle(anim_assets.graph_handle.clone()),
                AnimationTransitions::new(),
                LowerBodyAnimationPlayer,
            ));
        }
        break;
    }

    let Some(lower_player_entity) = lower_ent_opt else { return };

    let upper_player_entity = if let Ok(existing_upper) = upper_player_query.single() {
        existing_upper
    } else {
        commands
            .spawn((
                AnimationPlayer::default(),
                AnimationGraphHandle(anim_assets.graph_handle.clone()),
                AnimationTransitions::new(),
                UpperBodyAnimationPlayer,
            ))
            .id()
    };

    // اصلاح خودکار استخوان‌های بالاتنه حتی اگر با تأخیر لود شده باشند
    for (bone_entity, mut animated_by) in &mut target_query {
        if animated_by.0 == lower_player_entity {
            if let Ok(bname) = names_query.get(bone_entity) {
                let ln = bname.as_str().to_lowercase();
                if ln.contains("hips") || ln.contains("pelvis") {
                    commands.entity(bone_entity).insert(SoldierHipsBone);
                }
            }
            if is_upper_body_in_hierarchy(bone_entity, &names_query, &parents_query) {
                animated_by.0 = upper_player_entity;
                let parent_is_upper = parents_query
                    .get(bone_entity)
                    .ok()
                    .map(|pr| is_upper_body_in_hierarchy(pr.parent(), &names_query, &parents_query))
                    .unwrap_or(false);
                if !parent_is_upper {
                    commands.entity(bone_entity).insert(SoldierSpineRootBone);
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// ساخت اسلحه افقی با Bevy و تراز در راستای دست و هدف
// -----------------------------------------------------------------------------

fn attach_weapon_to_left_hand(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    hero_state: Res<LatestHeroSnapshot>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status: ResMut<WeaponAttachmentStatus>,
    tuner: Res<WeaponTuner>,
    soldier_marker_query: Query<(), With<SoldierMarker>>,
    parents_query: Query<&ChildOf>,
    hand_query: Query<(Entity, &Name)>,
    existing_weapon_q: Query<(Entity, &AttachedWeaponKind), With<WeaponRoot>>,
) {
    if !match_flow.is_in_game() {
        return;
    }
    for (w_ent, attached_kind) in &existing_weapon_q {
        if attached_kind.0 != hero_state.0.weapon_kind {
            commands.entity(w_ent).despawn();
            status.attached = false;
        }
    }
    if status.attached {
        return;
    }

    for (entity, name) in &hand_query {
        let n = name.as_str().to_lowercase();
        let is_left =
            n.contains("left") || n.contains("_l") || n.contains(".l") || n.contains("l_");
        let is_hand = n.contains("hand") || n.contains("wrist") || n.contains("palm");
        let is_finger = n.contains("thumb")
            || n.contains("index")
            || n.contains("middle")
            || n.contains("ring")
            || n.contains("pinky")
            || n.contains("finger")
            || n.contains("twist");

        if is_hand && is_left && !is_finger {
            // اطمینان از اینکه دست متعلق به SoldierMarker اصلی است نه مدل پیش‌نمایش
            let mut cur = entity;
            let mut belongs_to_soldier = soldier_marker_query.contains(cur);
            while !belongs_to_soldier {
                if let Ok(parent) = parents_query.get(cur) {
                    cur = parent.parent();
                    if soldier_marker_query.contains(cur) {
                        belongs_to_soldier = true;
                        break;
                    }
                } else {
                    break;
                }
            }
            if !belongs_to_soldier {
                continue;
            }

            let weapon_kind = hero_state.0.weapon_kind;
            let body_mat = materials.add(StandardMaterial {
                base_color: match weapon_kind {
                    WeaponKind::AutomaticRifle => Color::srgb(0.14, 0.15, 0.18),
                    WeaponKind::Sniper => Color::srgb(0.16, 0.22, 0.18),
                    WeaponKind::Shotgun => Color::srgb(0.18, 0.16, 0.14),
                    WeaponKind::Throwable => Color::srgb(0.24, 0.16, 0.12),
                    WeaponKind::Pistol => Color::srgb(0.18, 0.20, 0.24),
                },
                metallic: 0.85,
                perceptual_roughness: 0.35,
                ..default()
            });
            let accent_mat = materials.add(StandardMaterial {
                base_color: match weapon_kind {
                    WeaponKind::AutomaticRifle => Color::srgb(0.24, 0.26, 0.30),
                    WeaponKind::Sniper => Color::srgb(0.28, 0.34, 0.30),
                    WeaponKind::Shotgun => Color::srgb(0.46, 0.28, 0.14),
                    WeaponKind::Throwable => Color::srgb(0.85, 0.42, 0.10),
                    WeaponKind::Pistol => Color::srgb(0.55, 0.60, 0.68),
                },
                metallic: if weapon_kind == WeaponKind::Shotgun { 0.25 } else { 0.65 },
                perceptual_roughness: 0.45,
                ..default()
            });
            let laser_box_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.1, 0.1),
                emissive: LinearRgba::new(5.0, 0.1, 0.1, 1.0),
                ..default()
            });
            let m_off = config.weapon_tuner.muzzle_local_offset;

            commands.entity(entity).with_children(|parent| {
                parent
                    .spawn((
                        WeaponRoot,
                        AttachedWeaponKind(weapon_kind),
                        Transform {
                            translation: tuner.offset,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::splat(tuner.scale),
                        },
                        Visibility::Visible,
                    ))
                    .with_children(|weapon| {
                        match weapon_kind {
                            WeaponKind::AutomaticRifle => {
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.055, 0.10, 0.44))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.03, -0.12),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.048, 0.075, 0.30))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, 0.035, -0.42),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.013, 0.40))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.045, -0.60)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.019, 0.07))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, 0.045, -0.80)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.048, 0.085, 0.22))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, 0.02, 0.18),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.04, 0.19, 0.072))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, -0.09, -0.24)
                                        .with_rotation(Quat::from_rotation_x(0.22)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.042, 0.13, 0.055))),
                                    MeshMaterial3d(body_mat),
                                    Transform::from_xyz(0.0, -0.05, -0.02)
                                        .with_rotation(Quat::from_rotation_x(-0.25)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.044, 0.05, 0.10))),
                                    MeshMaterial3d(accent_mat),
                                    Transform::from_xyz(0.0, 0.10, -0.14),
                                ));
                            }
                            WeaponKind::Sniper => {
                                // بدنه کشیده اسنایپر + لوله بلند سایلنسردار + دوربین بزرگ
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.052, 0.09, 0.56))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.03, -0.16),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.014, 0.78))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.048, -0.78)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.024, 0.22))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, 0.048, -1.16)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                // دوربین اسنایپر
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.032, 0.34))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, 0.115, -0.18)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.050, 0.095, 0.28))),
                                    MeshMaterial3d(body_mat),
                                    Transform::from_xyz(0.0, 0.02, 0.22),
                                ));
                            }
                            WeaponKind::Shotgun => {
                                // شات‌گان سنگین با دو لوله ضخیم و دستگیره پمپی چوبی/برنزی
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.072, 0.11, 0.42))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.03, -0.10),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.024, 0.52))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.055, -0.48)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.020, 0.46))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.015, -0.46)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                // دستگیره پمپی و قنداق چوبی
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.068, 0.065, 0.24))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, 0.005, -0.44),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.058, 0.095, 0.26))),
                                    MeshMaterial3d(accent_mat),
                                    Transform::from_xyz(0.0, 0.015, 0.20),
                                ));
                            }
                            WeaponKind::Throwable => {
                                // نارنجک‌انداز سنگین با خشاب استوانه‌ای چرخان و لوله پهن
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.08, 0.12, 0.36))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.03, -0.08),
                                ));
                                // خشاب درام چرخان نارنجی/فلزی
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.095, 0.15))),
                                    MeshMaterial3d(accent_mat.clone()),
                                    Transform::from_xyz(0.0, -0.02, -0.22)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                // لوله پهن پرتاب‌کننده
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.042, 0.38))),
                                    MeshMaterial3d(body_mat),
                                    Transform::from_xyz(0.0, 0.04, -0.44)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.056, 0.09, 0.22))),
                                    MeshMaterial3d(accent_mat),
                                    Transform::from_xyz(0.0, 0.02, 0.18),
                                ));
                            }
                            WeaponKind::Pistol => {
                                // بدنه و اسلاید جمع‌وجور کلت
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.040, 0.065, 0.24))),
                                    MeshMaterial3d(body_mat.clone()),
                                    Transform::from_xyz(0.0, 0.04, -0.08),
                                ));
                                // قبضه پیستول
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cuboid::new(0.036, 0.13, 0.055))),
                                    MeshMaterial3d(accent_mat),
                                    Transform::from_xyz(0.0, -0.04, 0.01)
                                        .with_rotation(Quat::from_rotation_x(-0.22)),
                                ));
                                // لوله کوتاه کلت
                                weapon.spawn((
                                    Mesh3d(meshes.add(Cylinder::new(0.012, 0.12))),
                                    MeshMaterial3d(body_mat),
                                    Transform::from_xyz(0.0, 0.045, -0.24)
                                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                                ));
                            }
                        }

                        weapon.spawn((
                            Mesh3d(meshes.add(Cuboid::new(0.028, 0.028, 0.08))),
                            MeshMaterial3d(laser_box_mat),
                            Transform::from_xyz(0.0, 0.0, -0.50),
                        ));
                        weapon.spawn((
                            MuzzlePoint,
                            Transform::from_xyz(m_off.0, m_off.1, m_off.2),
                            Visibility::Visible,
                        ));
                    });
            });

            status.attached = true;
            status.hand_entity = Some(entity);
            break;
        }
    }
}

fn align_weapon_horizontally_system(
    status: Res<WeaponAttachmentStatus>,
    tuner: Res<WeaponTuner>,
    soldier_query: Query<&Transform, (With<SoldierMarker>, Without<WeaponRoot>)>,
    global_transforms: Query<&GlobalTransform>,
    mut weapon_query: Query<&mut Transform, With<WeaponRoot>>,
) {
    let Some(hand_entity) = status.hand_entity else { return };
    let Ok(soldier_tf) = soldier_query.single() else { return };
    let Ok(hand_gtf) = global_transforms.get(hand_entity) else { return };
    let Ok(mut weapon_tf) = weapon_query.single_mut() else { return };

    let hero_forward =
        Vec3::new(soldier_tf.back().x, 0.0, soldier_tf.back().z).normalize_or_zero();
    if hero_forward.length_squared() < 0.001 {
        return;
    }

    let desired_world_rot = Transform::IDENTITY
        .looking_to(hero_forward, Vec3::Y)
        .rotation;

    let (_, hand_world_rot, _) = hand_gtf.to_scale_rotation_translation();
    let user_rot_offset = Quat::from_euler(
        EulerRot::YXZ,
        tuner.rotation.y.to_radians(),
        tuner.rotation.x.to_radians(),
        tuner.rotation.z.to_radians(),
    );

    weapon_tf.translation = tuner.offset;
    weapon_tf.rotation = hand_world_rot.inverse() * desired_world_rot * user_rot_offset;
    weapon_tf.scale = Vec3::splat(tuner.scale);
}

fn weapon_live_calibration_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    chat_state: Res<ChatUiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tuner: ResMut<WeaponTuner>,
) {
    if chat_state.is_open {
        return;
    }
    let dt = time.delta_secs();

    let move_speed = config.weapon_tuner.calibration_move_speed * dt;
    let rot_speed = config.weapon_tuner.calibration_rot_speed_deg * dt;

    if keyboard.pressed(KeyCode::KeyI) {
        tuner.offset.z -= move_speed;
    }
    if keyboard.pressed(KeyCode::KeyK) {
        tuner.offset.z += move_speed;
    }
    if keyboard.pressed(KeyCode::KeyJ) {
        tuner.offset.x -= move_speed;
    }
    if keyboard.pressed(KeyCode::KeyL) {
        tuner.offset.x += move_speed;
    }
    if keyboard.pressed(KeyCode::KeyU) {
        tuner.offset.y += move_speed;
    }
    if keyboard.pressed(KeyCode::KeyO) {
        tuner.offset.y -= move_speed;
    }

    if keyboard.pressed(KeyCode::ArrowLeft) {
        tuner.rotation.y += rot_speed;
    }
    if keyboard.pressed(KeyCode::ArrowRight) {
        tuner.rotation.y -= rot_speed;
    }
    if keyboard.pressed(KeyCode::ArrowUp) {
        tuner.rotation.x += rot_speed;
    }
    if keyboard.pressed(KeyCode::ArrowDown) {
        tuner.rotation.x -= rot_speed;
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        println!(
            "Offset: ({:.3}, {:.3}, {:.3}), Rotation: ({:.1}, {:.1}, {:.1})",
            tuner.offset.x,
            tuner.offset.y,
            tuner.offset.z,
            tuner.rotation.x,
            tuner.rotation.y,
            tuner.rotation.z
        );
    }
}

// -----------------------------------------------------------------------------
// ارسال ورودی به سرور و دریافت وضعیت Authoritative از سرور
// -----------------------------------------------------------------------------

#[derive(SystemParam)]
struct SampleInputExtraParams<'w> {
    render_state: Res<'w, ClientRenderState>,
    courier_ctrl: ResMut<'w, CourierControlState>,
    inspected: ResMut<'w, InspectedUnit>,
    hero_move_target: ResMut<'w, HeroClickMoveTarget>,
    prediction: ResMut<'w, ClientPhysicsPredictionState>,
}

fn client_sample_input_system(
    match_flow: Res<ClientMatchFlow>,
    match_result: Res<LatestMatchResult>,
    pause_state: Res<LatestPauseState>,
    chat_state: Res<ChatUiState>,
    shop_ui: Res<ShopUiState>,
    settings_ui: Res<SettingsUiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    camera_control: Res<CameraControlState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    muzzle_query: Query<&GlobalTransform, With<MuzzlePoint>>,
    mut ex: SampleInputExtraParams,
) {
    if !match_flow.is_in_game() || match_result.0.is_some() {
        return;
    }
    if chat_state.is_open || pause_state.0.phase != PausePhase::Running {
        ex.prediction.latest_input = Some(ClientInputCommand {
            move_input: Vec3::ZERO,
            shift_held: false,
            toggle_crouch_just_pressed: false,
            reload_just_pressed: false,
            grenade_just_pressed: false,
            space_just_pressed: false,
            space_held: false,
            switch_weapon_just_pressed: false,
            fire_just_pressed: false,
            fire_held: false,
            aim_ray_origin: Vec3::ZERO,
            aim_ray_dir: Vec3::NEG_Z,
            has_aim_ray: false,
            client_muzzle_world_pos: None,
        });
        return;
    }
    let mut move_vec = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        move_vec.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        move_vec.z += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        move_vec.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        move_vec.x += 1.0;
    }
    if move_vec.length_squared() > 0.01 {
        ex.hero_move_target.0 = None;
    } else if let Some(target) = ex.hero_move_target.0 {
        let delta = Vec3::new(
            target.x - ex.render_state.soldier_pos.x,
            0.0,
            target.z - ex.render_state.soldier_pos.z,
        );
        if delta.length() <= 0.45 {
            ex.hero_move_target.0 = None;
        } else {
            move_vec = delta.normalize();
        }
    }

    let mut aim_ray_origin = Vec3::ZERO;
    let mut aim_ray_dir = Vec3::NEG_Z;
    let mut has_aim_ray = false;

    if let (Ok(window), Ok((camera, camera_tf))) = (window_query.single(), camera_query.single()) {
        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(ray) = camera.viewport_to_world(camera_tf, cursor_pos) {
                aim_ray_origin = ray.origin;
                aim_ray_dir = *ray.direction;
                has_aim_ray = true;
            }
        }
    }

    let client_muzzle_world_pos = muzzle_query.single().ok().map(|gtf| gtf.translation());

    let alt_held = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    let hero_action_taken = move_vec.length_squared() > 0.01
        || keyboard.just_pressed(KeyCode::Space)
        || keyboard.just_pressed(KeyCode::KeyR)
        || keyboard.just_pressed(KeyCode::KeyE)
        || keyboard.just_pressed(KeyCode::KeyF)
        || (!alt_held
            && !shop_ui.is_open
            && !settings_ui.is_open
            && !camera_control.is_pointer_over_minimap
            && !camera_control.is_dragging_minimap
            && mouse_button.just_pressed(MouseButton::Left));
    if hero_action_taken && !shop_ui.is_open && !settings_ui.is_open {
        ex.courier_ctrl.selected = false;
        if *ex.inspected == InspectedUnit::OwnCourier {
            *ex.inspected = InspectedUnit::OwnHero;
        }
    }
    let cmd = ClientInputCommand {
        move_input: move_vec,
        shift_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        toggle_crouch_just_pressed: keyboard.just_pressed(KeyCode::ControlLeft)
            || keyboard.just_pressed(KeyCode::ControlRight),
        reload_just_pressed: keyboard.just_pressed(KeyCode::KeyR),
        grenade_just_pressed: keyboard.just_pressed(KeyCode::KeyF),
        space_just_pressed: keyboard.just_pressed(KeyCode::Space),
        space_held: keyboard.pressed(KeyCode::Space),
        switch_weapon_just_pressed: keyboard.just_pressed(KeyCode::KeyE),
        fire_just_pressed: !alt_held
            && !shop_ui.is_open
            && !settings_ui.is_open
            && !camera_control.is_pointer_over_minimap
            && !camera_control.is_dragging_minimap
            && mouse_button.just_pressed(MouseButton::Left),
        fire_held: !alt_held
            && !shop_ui.is_open
            && !settings_ui.is_open
            && !camera_control.is_pointer_over_minimap
            && !camera_control.is_dragging_minimap
            && mouse_button.pressed(MouseButton::Left),
        aim_ray_origin,
        aim_ray_dir,
        has_aim_ray,
        client_muzzle_world_pos,
    };

    if cmd.toggle_crouch_just_pressed {
        ex.prediction.base_stance = if ex.prediction.base_stance == Stance::Crouch {
            Stance::Run
        } else {
            Stance::Crouch
        };
    }
    ex.prediction.effective_stance = if cmd.shift_held {
        Stance::Walk
    } else {
        ex.prediction.base_stance
    };

    if let Some(ref mut existing) = ex.prediction.latest_input {
        existing.accumulate_from(cmd);
    } else {
        ex.prediction.latest_input = Some(cmd);
    }
}

fn client_flush_input_to_server_system(
    mut net: ResMut<ClientNetwork>,
    mut prediction: ResMut<ClientPhysicsPredictionState>,
) {
    let net = &mut *net;
    if net.client.is_connected() {
        if let Some(ref mut cmd) = prediction.latest_input {
            let bytes = bincode::serialize(cmd)
                .expect("خطای بحرانی کلاینت: سریالایز کردن ورودی شکست خورد");
            net.client.send_message(CHANNEL_UNRELIABLE_STATE, bytes);
            cmd.consume_just_pressed();
        }
    }
    let _ = net.transport.send_packets(&mut net.client);
}

#[derive(SystemParam)]
struct TurretFxResources<'w, 's> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    turret_gun_q: Query<'w, 's, &'static mut TurretGunBone>,
    turret_muzzle_q: Query<'w, 's, (&'static TurretMuzzlePoint, &'static GlobalTransform)>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn client_receive_snapshot_and_sync_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    mut net: ResMut<ClientNetwork>,
    mut render_state: ResMut<ClientRenderState>,
    mut prediction: ResMut<ClientPhysicsPredictionState>,
    mut shake: ResMut<CameraShake>,
    mut hud_sync: SnapshotHudSyncParams,
    mut fow: Option<ResMut<FogOfWarState>>,
    mut soldier_visual_query: Query<
        &mut Transform,
        (
            With<SoldierMarker>,
            Without<PhysicsSoldier>,
            Without<NetworkBulletVisual>,
            Without<NetworkGrenadeVisual>,
        ),
    >,
    mut soldier_physics_query: Query<
        &mut Transform,
        (
            With<PhysicsSoldier>,
            Without<SoldierMarker>,
            Without<NetworkBulletVisual>,
            Without<NetworkGrenadeVisual>,
        ),
    >,
    mut bullet_query: Query<
        (Entity, &mut NetworkBulletVisual, &mut Transform, &mut Visibility),
        (
            Without<SoldierMarker>,
            Without<PhysicsSoldier>,
            Without<NetworkGrenadeVisual>,
        ),
    >,
    mut grenade_query: Query<
        (Entity, &NetworkGrenadeVisual, &mut Transform, &mut Velocity),
        (
            Without<SoldierMarker>,
            Without<PhysicsSoldier>,
            Without<NetworkBulletVisual>,
        ),
    >,
    mut target_query: Query<(Entity, &mut VisualTarget)>,
    mut fx_res: TurretFxResources,
) {
    let delta = time.delta();
    let net = &mut *net;

    net.client.update(delta);
    if let Err(e) = net.transport.update(delta, &mut net.client) {
        panic!("خطای بحرانی ترنسپورت Renet در کلاینت: {}", e);
    }

    let mut latest_snapshot: Option<ServerWorldSnapshot> = None;
    let mut all_fx_events: Vec<GameplayFxEvent> = Vec::new();

    if net.client.is_connected() {
        while let Some(message) = net.client.receive_message(CHANNEL_RELIABLE_FX) {
            let mut batch: Vec<GameplayFxEvent> = bincode::deserialize(&message)
                .expect("خطای بحرانی کلاینت: پکت رویدادهای FX نامعتبر است");
            all_fx_events.append(&mut batch);
        }

        while let Some(message) = net.client.receive_message(CHANNEL_UNRELIABLE_STATE) {
            let snap: ServerWorldSnapshot = bincode::deserialize(&message)
                .expect("خطای بحرانی کلاینت: پکت Snapshot دریافتی از سرور نامعتبر است");
            latest_snapshot = Some(snap);
        }
    }

    if let Some(snap) = latest_snapshot {
        render_state.soldier_pos = snap.soldier_pos;
        render_state.soldier_facing_dir = snap.soldier_facing_dir;
        render_state.aim_world_dir = snap.aim_world_dir;
        render_state.aim_hit_pos = snap.aim_hit_pos;
        render_state.laser_hit_pos = snap.laser_hit_pos;
        render_state.aim_locked_on_enemy = snap.aim_locked_on_enemy;
        render_state.hovered_creep_id = snap.hovered_creep_id;
        render_state.hovered_turret_index = snap.hovered_turret_index;
        render_state.hovered_barracks_index = snap.hovered_barracks_index;
        render_state.hovered_ancient_index = snap.hovered_ancient_index;
        render_state.current_locomotion = snap.current_locomotion;
        render_state.upper_action = snap.upper_action;
        render_state.traversal_active = snap.traversal_active;
        render_state.traversal_anim_name = snap.traversal_anim_name;
        render_state.traversal_duration = snap.traversal_duration;
        render_state.is_dead = snap.is_dead;
        render_state.death_anim_name = snap.death_anim_name;
        render_state.in_water = snap.in_water;
        hud_sync.hero_hud.0 = snap.hero.clone();
        hud_sync.remote_players.0 = snap.other_players.clone();
        hud_sync.creeps.0 = snap.creeps.clone();
        hud_sync.couriers.0 = snap.couriers.clone();
        hud_sync.top_bar.match_elapsed_secs = snap.match_elapsed_secs;
        hud_sync.top_bar.dawn_total_kills = snap.dawn_total_kills;
        hud_sync.top_bar.dusk_total_kills = snap.dusk_total_kills;
        hud_sync.top_bar.heroes = snap.top_bar_heroes.clone();
        hud_sync.scoreboard.0 = snap.scoreboard_heroes.clone();
        hud_sync.turrets.0 = snap.turrets.clone();
        hud_sync.barracks.0 = snap.barracks.clone();
        hud_sync.ancients.0 = snap.ancients.clone();
        hud_sync.match_result.0 = snap.winning_team;
        for msg in snap.recent_chat {
            if hud_sync.chat_state.seen_ids.insert(msg.id) {
                hud_sync.chat_state.messages.push(ClientChatItem {
                    entry: msg,
                    fade_remaining: config.chat.message_fade_secs,
                });
                let max_vis = config.chat.max_visible_messages;
                if hud_sync.chat_state.messages.len() > max_vis {
                    let drain_n = hud_sync.chat_state.messages.len() - max_vis;
                    hud_sync.chat_state.messages.drain(0..drain_n);
                }
                hud_sync.chat_state.dirty = true;
            }
        }
        let minimap_ent_opt = hud_sync.minimap_root_q.single().ok();
        for pg in snap.recent_pings {
            if hud_sync.ping_state.seen_ids.insert(pg.id) {
                spawn_team_ping_visuals(
                    &mut commands,
                    &config,
                    &mut fx_res.meshes,
                    &mut fx_res.materials,
                    minimap_ent_opt,
                    pg.world_pos,
                    &pg.sender_name,
                );
            }
        }
        if let Some(ref mut fow_state) = fow {
            fow_state.shared.apply_visible_mask(&snap.fog_visible_mask);
        }

        prediction.traversal_active = snap.traversal_active;
        prediction.is_dead = snap.is_dead;
        prediction.match_ended = snap.winning_team.is_some();
        prediction.is_paused = snap.pause.phase != PausePhase::Running;
        if let Some(h_idx) = config.heroes.iter().position(|h| h.id == snap.hero.hero_id) {
            prediction.hero_index = h_idx;
        }
        prediction.latest_hero_scale = snap.hero.hero_scale;
        prediction.latest_move_speed_multiplier = Some(snap.hero.move_speed / config.soldier.run_speed.max(0.1));
        prediction.latest_authoritative_feet_pos = Some(snap.soldier_pos);
        hud_sync.pause_state.0 = snap.pause.clone();
        if let Some(prev_tick) = hud_sync.telemetry.last_server_tick {
            if snap.tick > prev_tick {
                let delta = snap.tick - prev_tick;
                let step = match hud_sync.telemetry.min_tick_step {
                    Some(s) => s.min(delta),
                    None => delta,
                };
                hud_sync.telemetry.min_tick_step = Some(step.max(1));
                let step_val = hud_sync.telemetry.min_tick_step.unwrap_or(1).max(1);
                let expected = (delta / step_val).max(1);
                hud_sync.telemetry.in_expected_window += expected;
                hud_sync.telemetry.in_lost_window += expected.saturating_sub(1);
            }
        }
        hud_sync.telemetry.last_server_tick = Some(snap.tick);

        // آشتی‌سازی فیزیکی در FixedUpdate (client_reconcile_fixed_system) انجام می‌شود تا نوسان 60Hz/128Hz رخ ندهد

        for (entity, mut v_target) in &mut target_query {
            let s_target = &snap.targets[v_target.index];
            let was_dead = v_target.is_dead;
            v_target.hp = s_target.hp;
            v_target.max_hp = s_target.max_hp;
            v_target.hit_flash_timer = s_target.hit_flash_timer;
            v_target.is_dead = s_target.is_dead;
            v_target.is_server_visible = s_target.is_visible;

            if !was_dead && s_target.is_dead {
                commands.entity(entity).insert(ColliderDisabled);
            } else if was_dead && !s_target.is_dead {
                commands.entity(entity).remove::<ColliderDisabled>();
            }
        }

        let bullet_map: HashMap<u64, _> = snap.bullets.iter().map(|b| (b.id, b)).collect();
        let mut seen_bullets = HashSet::new();

        for (_entity, mut vis, mut tf, mut b_vis) in &mut bullet_query {
            if vis.server_id == 0 {
                continue;
            }
            if let Some(b_snap) = bullet_map.get(&vis.server_id) {
                let dir = b_snap.velocity.normalize_or_zero();
                if vis.visual_offset_blend > 0.0 {
                    vis.visual_offset_blend =
                        (vis.visual_offset_blend - time.delta_secs() * 14.0).max(0.0);
                }
                tf.translation = b_snap.translation + vis.visual_offset * vis.visual_offset_blend;
                tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
                *b_vis = Visibility::Visible;
                seen_bullets.insert(vis.server_id);
            } else {
                vis.server_id = 0;
                vis.visual_offset = Vec3::ZERO;
                vis.visual_offset_blend = 0.0;
                *b_vis = Visibility::Hidden;
            }
        }

        let live_muzzle_opt = hud_sync.muzzle_query.single().ok().map(|gtf| gtf.translation());
        for b_snap in &snap.bullets {
            if !seen_bullets.contains(&b_snap.id) {
                let dir = b_snap.velocity.normalize_or_zero();
                let rot = Quat::from_rotation_arc(Vec3::Y, dir);
                let (vis_off, vis_blend) = if let Some(muzzle) = live_muzzle_opt {
                    let diff = muzzle - b_snap.translation;
                    if diff.length_squared() <= 5.0 * 5.0 {
                        (diff, 1.0)
                    } else {
                        (Vec3::ZERO, 0.0)
                    }
                } else {
                    (Vec3::ZERO, 0.0)
                };
                let mut reused = false;
                for (_e, mut vis, mut tf, mut b_vis) in &mut bullet_query {
                    if vis.server_id == 0 {
                        vis.server_id = b_snap.id;
                        vis.visual_offset = vis_off;
                        vis.visual_offset_blend = vis_blend;
                        tf.translation = b_snap.translation + vis_off * vis_blend;
                        tf.rotation = rot;
                        *b_vis = Visibility::Visible;
                        reused = true;
                        break;
                    }
                }
                if !reused {
                    if let Some(ref sfx) = hud_sync.shared_fx {
                        commands.spawn((
                            Mesh3d(sfx.bullet_mesh.clone()),
                            MeshMaterial3d(sfx.bullet_mat.clone()),
                            Transform {
                                translation: b_snap.translation + vis_off * vis_blend,
                                rotation: rot,
                                scale: Vec3::ONE,
                            },
                            Visibility::Visible,
                            NotShadowCaster,
                            NetworkBulletVisual {
                                server_id: b_snap.id,
                                visual_offset: vis_off,
                                visual_offset_blend: vis_blend,
                            },
                        ));
                    }
                }
            }
        }

        let grenade_map: HashMap<u64, _> = snap.grenades.iter().map(|g| (g.id, g)).collect();
        let mut seen_grenades = HashSet::new();

        for (entity, vis, mut tf, mut vel) in &mut grenade_query {
            if let Some(g_snap) = grenade_map.get(&vis.server_id) {
                tf.translation = g_snap.translation;
                tf.rotation = g_snap.rotation;
                vel.linear = g_snap.linear_velocity;
                vel.angular = g_snap.angular_velocity;
                seen_grenades.insert(vis.server_id);
            } else {
                commands.entity(entity).despawn();
            }
        }

        for g_snap in &snap.grenades {
            if !seen_grenades.contains(&g_snap.id) {
                let led_mat = fx_res.materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.12, 0.08),
                    emissive: LinearRgba::new(14.0, 0.6, 0.4, 1.0),
                    unlit: true,
                    ..default()
                });
                commands
                    .spawn((
                        Mesh3d(fx_res.meshes.add(Sphere::new(config.physics.grenade_collider_radius))),
                        MeshMaterial3d(fx_res.materials.add(StandardMaterial {
                            base_color: Color::srgb(0.2, 0.38, 0.18),
                            metallic: 0.6,
                            perceptual_roughness: 0.4,
                            ..default()
                        })),
                        NetworkGrenadeVisual {
                            server_id: g_snap.id,
                            spawn_time: time.elapsed_secs(),
                        },
                        grenade_physics_bundle(
                            &config,
                            g_snap.id,
                            0,
                            TeamId::Dawn,
                            1.5,
                            0.0,
                            1.0,
                            g_snap.translation,
                            g_snap.rotation,
                            g_snap.linear_velocity,
                            g_snap.angular_velocity,
                        ),
                    ))
                    .with_children(|g| {
                        g.spawn((
                            GrenadeBlinkLed { mat: led_mat.clone() },
                            Mesh3d(fx_res.meshes.add(Sphere::new(config.physics.grenade_collider_radius * 0.45))),
                            MeshMaterial3d(led_mat),
                            Transform::from_xyz(0.0, config.physics.grenade_collider_radius * 0.9, 0.0),
                            PointLight {
                                color: Color::srgb(1.0, 0.10, 0.08),
                                intensity: 95_000.0,
                                range: 5.5,
                                shadow_maps_enabled: false,
                                ..default()
                            },
                            NotShadowCaster,
                        ));
                    });
            }
        }
    }

    let fx_live_muzzle = hud_sync.muzzle_query.single().ok().map(|gtf| gtf.translation());
    for fx in all_fx_events {
        spawn_fx_from_server_event(
            &mut commands,
            &config,
            &mut render_state,
            &mut shake,
            &mut fx_res.meshes,
            &mut fx_res.materials,
            &mut fx_res.turret_gun_q,
            &fx_res.turret_muzzle_q,
            fx_live_muzzle,
            fx,
        );
    }

    if let (Ok(soldier_phys_tf), Ok(mut soldier_vis_tf)) = (
        soldier_physics_query.single(),
        soldier_visual_query.single_mut(),
    ) {
        let hero_scale = hud_sync.hero_hud.0.hero_scale;
        let target_feet_pos = Vec3::new(
            soldier_phys_tf.translation.x,
            soldier_phys_tf.translation.y - config.physics.soldier_capsule_center_y * hero_scale,
            soldier_phys_tf.translation.z,
        );
        let interp_pos = if hud_sync.fixed_history.initialized {
            if hud_sync.fixed_history.curr_feet.distance(target_feet_pos)
                > config.physics.position_reconcile_snap_dist
            {
                hud_sync.fixed_history.prev_feet = target_feet_pos;
                hud_sync.fixed_history.curr_feet = target_feet_pos;
                target_feet_pos
            } else {
                let alpha = hud_sync.fixed_time.overstep_fraction();
                hud_sync
                    .fixed_history
                    .prev_feet
                    .lerp(hud_sync.fixed_history.curr_feet, alpha)
            }
        } else {
            target_feet_pos
        };
        let y_alpha = (1.0 - (-22.0 * time.delta_secs()).exp()).clamp(0.0, 1.0);
        let smooth_y = soldier_vis_tf.translation.y.lerp(interp_pos.y, y_alpha);
        let smoothed_pos = Vec3::new(interp_pos.x, smooth_y, interp_pos.z);
        render_state.soldier_pos = smoothed_pos;
        soldier_vis_tf.translation = smoothed_pos;
        orient_soldier_towards(
            &mut soldier_vis_tf,
            render_state.soldier_facing_dir,
            config.physics.orient_min_dir_sq,
        );
    }
}

fn spawn_fx_from_server_event(
    commands: &mut Commands,
    config: &GameConfig,
    render_state: &mut ClientRenderState,
    shake: &mut CameraShake,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    turret_gun_q: &mut Query<&mut TurretGunBone>,
    turret_muzzle_q: &Query<(&TurretMuzzlePoint, &GlobalTransform)>,
    live_muzzle_opt: Option<Vec3>,
    event: GameplayFxEvent,
) {
    match event {
        GameplayFxEvent::ShotFired {
            muzzle_pos,
            shot_dir,
            trauma_add,
            trauma_max,
        } => {
            let muzzle_pos = match live_muzzle_opt {
                Some(lm) if (lm - muzzle_pos).length_squared() <= 5.0 * 5.0 => lm,
                _ => muzzle_pos,
            };
            let bullet_rot = Quat::from_rotation_arc(Vec3::Y, shot_dir);
            commands.spawn((
                Mesh3d(meshes.add(Cone {
                    radius: 0.07,
                    height: 0.28,
                })),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.8, 0.2),
                    emissive: LinearRgba::new(16.0, 9.0, 1.5, 1.0),
                    unlit: true,
                    ..default()
                })),
                NotShadowCaster,
                Transform {
                    translation: muzzle_pos + shot_dir * 0.1,
                    rotation: bullet_rot,
                    scale: Vec3::ONE,
                },
                MuzzleFlash {
                    timer: Timer::from_seconds(
                        config.fx.muzzle_flash_lifetime_secs,
                        TimerMode::Once,
                    ),
                },
            ));

            let right_dir = shot_dir.cross(Vec3::Y).normalize_or_zero();
            let r1 = render_state.next_fx_rand();
            let r2 = render_state.next_fx_rand();
            let rx = render_state.next_fx_rand();
            let ry = render_state.next_fx_rand();
            let rz = render_state.next_fx_rand();

            let shell_pos = muzzle_pos - shot_dir * 0.25;
            let shell_lin_vel =
                right_dir * (2.2 + r1 * 1.5) + Vec3::Y * (2.4 + r2 * 1.2) - shot_dir * 0.6;
            let shell_ang_vel = Vec3::new(rx * 18.0, ry * 18.0, rz * 18.0);

            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(
                    config.physics.shell_collider_radius,
                    config.physics.shell_collider_half_height * 2.0,
                ))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.95, 0.78, 0.25),
                    metallic: 0.9,
                    perceptual_roughness: 0.25,
                    ..default()
                })),
                shell_casing_physics_bundle(config, shell_pos, shell_lin_vel, shell_ang_vel),
            ));

            shake.trauma = (shake.trauma + trauma_add).min(trauma_max);
        }
        GameplayFxEvent::BulletTrailSegment { start, end } => {
            commands.spawn(BulletTrail {
                start,
                end,
                timer: Timer::from_seconds(config.fx.trail_lifetime_secs, TimerMode::Once),
            });
        }
        GameplayFxEvent::BulletImpact {
            impact_pos,
            bullet_dir,
            target_destroyed,
            trauma_add,
            trauma_max,
        } => {
            let spark_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.75, 0.2),
                emissive: LinearRgba::new(12.0, 5.5, 0.5, 1.0),
                unlit: true,
                ..default()
            });
            let spark_count = if target_destroyed {
                config.fx.hit_spark_count_destroyed
            } else {
                config.fx.hit_spark_count_normal
            };
            let speed = if target_destroyed {
                config.fx.hit_spark_speed_destroyed
            } else {
                config.fx.hit_spark_speed_normal
            };

            for _ in 0..spark_count {
                let rx = render_state.next_fx_rand();
                let ry = render_state.next_fx_rand();
                let rz = render_state.next_fx_rand();
                let spread = Vec3::new(
                    (rx - 0.5) * speed,
                    ry * speed * 0.9 + 1.2,
                    (rz - 0.5) * speed,
                ) - bullet_dir * 2.5;

                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.03, 0.03, 0.12))),
                    MeshMaterial3d(spark_mat.clone()),
                    particle_physics_bundle(
                        config,
                        impact_pos,
                        spread,
                        config.fx.hit_spark_lifetime_secs,
                        true,
                    ),
                ));
            }

            if target_destroyed {
                shake.trauma = (shake.trauma + trauma_add).min(trauma_max);
            }
        }
        GameplayFxEvent::GrenadeExploded {
            pos,
            aoe_radius,
            trauma_add,
            trauma_max,
        } => {
            let fire_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.55, 0.1),
                emissive: LinearRgba::new(16.0, 6.0, 0.8, 1.0),
                unlit: true,
                ..default()
            });
            let smoke_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.32, 0.30, 0.28, 0.62),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.95,
                ..default()
            });
            let dust_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.52, 0.46, 0.38, 0.52),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.98,
                ..default()
            });
            let smoke_mesh = meshes.add(Sphere::new(0.45));
            let cloud_count = ((aoe_radius * 3.0) as u32).clamp(12, 26);
            for c_i in 0..cloud_count {
                let rx = render_state.next_fx_rand() - 0.5;
                let rz = render_state.next_fx_rand() - 0.5;
                let ry = render_state.next_fx_rand();
                let spread_r = aoe_radius * 0.55;
                let spawn_p = pos + Vec3::new(rx * spread_r, 0.25 + ry * 0.45, rz * spread_r);
                let is_dust = c_i % 3 == 0;
                commands.spawn((
                    Mesh3d(smoke_mesh.clone()),
                    MeshMaterial3d(if is_dust { dust_mat.clone() } else { smoke_mat.clone() }),
                    Transform::from_translation(spawn_p).with_scale(Vec3::splat(0.7)),
                    ExplosionSmokeParticle {
                        vel: Vec3::new(rx * 2.4, 1.8 + ry * 2.2, rz * 2.4),
                        timer: Timer::from_seconds(1.5 + ry * 0.7, TimerMode::Once),
                        start_scale: 0.65,
                        end_scale: (aoe_radius * 0.42).clamp(1.6, 3.4),
                    },
                    NotShadowCaster,
                ));
            }

            for _ in 0..config.fx.explosion_particle_count {
                let rx = render_state.next_fx_rand();
                let ry = render_state.next_fx_rand();
                let rz = render_state.next_fx_rand();
                let vel = Vec3::new((rx - 0.5) * 14.0, ry * 9.0 + 2.0, (rz - 0.5) * 14.0);

                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.09))),
                    MeshMaterial3d(fire_mat.clone()),
                    particle_physics_bundle(
                        config,
                        pos,
                        vel,
                        config.fx.explosion_particle_lifetime_secs,
                        true,
                    ),
                ));
            }

            commands.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.65, 0.2),
                    intensity: 250_000.0,
                    range: 12.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(pos + Vec3::Y * 0.5),
                MuzzleFlash {
                    timer: Timer::from_seconds(
                        config.fx.explosion_flash_lifetime_secs,
                        TimerMode::Once,
                    ),
                },
            ));

            shake.trauma = (shake.trauma + trauma_add).min(trauma_max);
        }
        GameplayFxEvent::SlideDust { pos } => {
            let rx = render_state.next_fx_rand();
            let ry = render_state.next_fx_rand();
            let rz = render_state.next_fx_rand();
            let vel = Vec3::new((rx - 0.5) * 1.5, 0.8 + ry * 1.2, (rz - 0.5) * 1.5);

            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.08, 0.08, 0.08))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.5, 0.45),
                    ..default()
                })),
                particle_physics_bundle(
                    config,
                    pos,
                    vel,
                    config.fx.slide_dust_lifetime_secs,
                    true,
                ),
            ));
        }
        GameplayFxEvent::WaterSplash { pos, intensity } => {
            spawn_water_ripple_and_droplets(
                commands,
                config,
                render_state,
                meshes,
                materials,
                pos + Vec3::Y * 0.03,
                intensity,
            );
        }
        GameplayFxEvent::HeroLeveledUp { pos, new_level: _ } => {
            let dur = config.fx.level_up_vfx_duration_secs;
            let pillar_mat = materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.86, 0.25, 0.42),
                emissive: LinearRgba::new(6.5, 4.8, 0.8, 0.42),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            });
            commands.spawn((
                LevelUpVfx {
                    timer: Timer::from_seconds(dur, TimerMode::Once),
                    is_ring: false,
                },
                Mesh3d(meshes.add(Cylinder::new(0.85, 6.8))),
                MeshMaterial3d(pillar_mat.clone()),
                Transform::from_translation(pos + Vec3::Y * 3.4),
                PointLight {
                    color: Color::srgb(1.0, 0.85, 0.28),
                    intensity: 420_000.0,
                    range: 12.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                NotShadowCaster,
            ));
            commands.spawn((
                LevelUpVfx {
                    timer: Timer::from_seconds(dur, TimerMode::Once),
                    is_ring: true,
                },
                Mesh3d(meshes.add(Torus::new(0.95, 1.15))),
                MeshMaterial3d(pillar_mat),
                Transform::from_translation(pos + Vec3::Y * 0.15),
                NotShadowCaster,
            ));

            let spark_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.90, 0.35),
                emissive: LinearRgba::new(10.0, 7.2, 1.2, 1.0),
                unlit: true,
                ..default()
            });
            let spark_mesh = meshes.add(Sphere::new(0.06));
            for i in 0..config.fx.level_up_particle_count {
                let angle = (i as f32) * (std::f32::consts::TAU / config.fx.level_up_particle_count as f32);
                let r = 0.55 + render_state.next_fx_rand() * 0.55;
                let sp = pos + Vec3::new(angle.cos() * r, 0.25, angle.sin() * r);
                let vel = Vec3::new(angle.cos() * 1.4, 3.8 + render_state.next_fx_rand() * 2.5, angle.sin() * 1.4);
                commands.spawn((
                    Mesh3d(spark_mesh.clone()),
                    MeshMaterial3d(spark_mat.clone()),
                    particle_physics_bundle(config, sp, vel, dur * 0.85, true),
                ));
            }
        }
        GameplayFxEvent::LastHitGold { pos, gold } => {
            let coin_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.84, 0.16),
                emissive: LinearRgba::new(5.5, 4.0, 0.4, 1.0),
                metallic: 0.85,
                perceptual_roughness: 0.25,
                ..default()
            });
            let coin_mesh = meshes.add(Cylinder::new(0.11, 0.03));
            for i in 0..config.fx.coin_burst_count {
                let angle = (i as f32) * (std::f32::consts::TAU / config.fx.coin_burst_count.max(1) as f32)
                    + render_state.next_fx_rand() * 0.4;
                let spd = 2.2 + render_state.next_fx_rand() * 2.0;
                let vel = Vec3::new(angle.cos() * spd, 3.6 + render_state.next_fx_rand() * 2.2, angle.sin() * spd);
                commands.spawn((
                    Mesh3d(coin_mesh.clone()),
                    MeshMaterial3d(coin_mat.clone()),
                    particle_physics_bundle(
                        config,
                        pos + Vec3::Y * 1.1,
                        vel,
                        config.fx.coin_lifetime_secs,
                        true,
                    ),
                ));
            }

            commands
                .spawn((
                    FloatingWorldPopup {
                        world_pos: pos + Vec3::Y * 1.75,
                        timer: Timer::from_seconds(config.fx.floating_popup_duration_secs, TimerMode::Once),
                    },
                    Pickable::IGNORE,
                    GlobalZIndex(180),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        padding: UiRect::axes(px(7.0), px(3.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.09, 0.02, 0.78)),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(format!("+{}g", gold)),
                        TextFont {
                            font_size: FontSize::Px(17.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.88, 0.22)),
                    ));
                });
        }
        GameplayFxEvent::TurretShotFired {
            turret_index,
            barrel_left,
            target_pos: _,
            shot_dir,
        } => {
            let jitter = (render_state.next_fx_rand() - 0.5) * 2.0 * config.fx.turret_recoil_jitter_rad;
            for mut gun in turret_gun_q.iter_mut() {
                if gun.turret_index == turret_index {
                    gun.current_recoil = config.fx.turret_recoil_offset;
                    gun.recoil_jitter_z = jitter;
                }
            }
            let mut muzzle_world = None;
            for (m, gtf) in turret_muzzle_q.iter() {
                if m.turret_index == turret_index && m.is_left == barrel_left {
                    muzzle_world = Some(gtf.translation());
                    break;
                }
            }
            let flash_pos = muzzle_world.unwrap_or_else(|| {
                let t_def = &config.map.turrets[turret_index];
                t_def.model_transform.location_vec() + Vec3::Y * (t_def.height * 0.72)
            });
            let rot = Quat::from_rotation_arc(Vec3::Y, shot_dir.normalize_or_zero());
            commands.spawn((
                Mesh3d(meshes.add(Cone {
                    radius: 0.14,
                    height: 0.45,
                })),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.85, 0.25),
                    emissive: LinearRgba::new(18.0, 10.0, 1.8, 1.0),
                    unlit: true,
                    ..default()
                })),
                PointLight {
                    color: Color::srgb(1.0, 0.82, 0.30),
                    intensity: 140_000.0,
                    range: 8.5,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform {
                    translation: flash_pos,
                    rotation: rot,
                    scale: Vec3::ONE,
                },
                MuzzleFlash {
                    timer: Timer::from_seconds(
                        config.fx.muzzle_flash_lifetime_secs,
                        TimerMode::Once,
                    ),
                },
            ));
        }
        GameplayFxEvent::CreepDenied { pos } => {
            commands
                .spawn((
                    FloatingWorldPopup {
                        world_pos: pos + Vec3::Y * 1.85,
                        timer: Timer::from_seconds(config.fx.floating_popup_duration_secs, TimerMode::Once),
                    },
                    Pickable::IGNORE,
                    GlobalZIndex(180),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        width: px(28.0),
                        height: px(28.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.35, 0.95, 1.0)),
                    BackgroundColor(Color::srgba(0.04, 0.14, 0.20, 0.88)),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new("!"),
                        TextFont {
                            font_size: FontSize::Px(19.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.98, 1.0)),
                    ));
                });
        }
    }
}

// -----------------------------------------------------------------------------
// واکنش بصری اهداف، لیزر، افکت‌ها، دوربین و پخش انیمیشن‌ها
// -----------------------------------------------------------------------------

fn target_visual_reaction_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut render_state: ResMut<ClientRenderState>,
    mut target_query: Query<(
        &mut Transform,
        &mut Visibility,
        Option<&MeshMaterial3d<StandardMaterial>>,
        &VisualTarget,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    let lerp_speed = config.fx.target_recover_lerp_speed;

    for (mut tf, mut vis, mat_handle_opt, target) in &mut target_query {
        if target.is_dead || !target.is_server_visible {
            *vis = Visibility::Hidden;
            continue;
        }

        *vis = Visibility::Visible;
        if target.hit_flash_timer > 0.0 {
            let rx = render_state.next_fx_rand();
            let rz = render_state.next_fx_rand();
            let jitter = Vec3::new((rx - 0.5) * 0.08, 0.0, (rz - 0.5) * 0.08);
            tf.translation = target.base_pos + jitter;
            tf.scale = Vec3::new(1.06, 0.94, 1.06);

            if let Some(mat_handle) = mat_handle_opt {
                if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                    mat.emissive = LinearRgba::new(6.0, 1.5, 0.5, 1.0);
                }
            }
        } else {
            tf.translation = tf.translation.lerp(target.base_pos, lerp_speed * dt);
            tf.scale = tf.scale.lerp(target.base_scale, lerp_speed * dt);

            if let Some(mat_handle) = mat_handle_opt {
                if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                    let health_ratio = (target.hp / target.max_hp).clamp(0.0, 1.0);
                    mat.base_color = target
                        .base_color
                        .mix(&Color::srgb(0.25, 0.08, 0.08), 1.0 - health_ratio);
                    mat.emissive = LinearRgba::BLACK;
                }
            }
        }
    }
}

fn spawn_laser_and_reticle_visuals(
    commands: &mut Commands,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let lc = &config.laser;
    let cyl_mesh = meshes.add(Cylinder::new(1.0, 1.0));

    let core_a = lc.core_color_rgba.3;
    let core_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(
            lc.core_color_rgba.0,
            lc.core_color_rgba.1,
            lc.core_color_rgba.2,
            core_a,
        ),
        emissive: LinearRgba::new(
            lc.core_color_rgba.0 * lc.emissive_strength * core_a,
            lc.core_color_rgba.1 * lc.emissive_strength * core_a * 0.5,
            lc.core_color_rgba.2 * lc.emissive_strength * core_a * 0.5,
            core_a,
        ),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let glow_a = lc.glow_color_rgba.3;
    let glow_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(
            lc.glow_color_rgba.0,
            lc.glow_color_rgba.1,
            lc.glow_color_rgba.2,
            glow_a,
        ),
        emissive: LinearRgba::new(
            lc.glow_color_rgba.0 * lc.emissive_strength * glow_a,
            lc.glow_color_rgba.1 * lc.emissive_strength * glow_a,
            lc.glow_color_rgba.2 * lc.emissive_strength * glow_a,
            glow_a,
        ),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    let bead_a = lc.bead_alpha;
    let bead_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(
            lc.core_color_rgba.0,
            lc.core_color_rgba.1,
            lc.core_color_rgba.2,
            bead_a,
        ),
        emissive: LinearRgba::new(
            lc.core_color_rgba.0 * lc.emissive_strength * bead_a,
            lc.core_color_rgba.1 * lc.emissive_strength * bead_a * 0.5,
            lc.core_color_rgba.2 * lc.emissive_strength * bead_a * 0.5,
            bead_a,
        ),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands.spawn((
        LaserBeamCore,
        Mesh3d(cyl_mesh.clone()),
        MeshMaterial3d(core_mat.clone()),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    commands.spawn((
        LaserBeamGlow,
        Mesh3d(cyl_mesh.clone()),
        MeshMaterial3d(glow_mat),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    for i in 0..3 {
        commands.spawn((
            LaserPulseBead {
                phase: (i as f32) / 3.0,
            },
            Mesh3d(cyl_mesh.clone()),
            MeshMaterial3d(bead_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            NotShadowCaster,
        ));
    }

    let dot_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(
            lc.core_color_rgba.0,
            lc.core_color_rgba.1,
            lc.core_color_rgba.2,
            1.0,
        ),
        emissive: LinearRgba::new(
            lc.glow_color_rgba.0 * lc.emissive_strength,
            lc.glow_color_rgba.1 * lc.emissive_strength,
            lc.glow_color_rgba.2 * lc.emissive_strength,
            1.0,
        ),
        unlit: true,
        ..default()
    });
    commands.spawn((
        LaserImpactDot,
        Mesh3d(meshes.add(Sphere::new(lc.impact_dot_radius))),
        MeshMaterial3d(dot_mat.clone()),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    commands.spawn((
        LaserImpactRing,
        Mesh3d(meshes.add(Torus::new(
            lc.impact_ring_inner_radius,
            lc.impact_ring_outer_radius,
        ))),
        MeshMaterial3d(dot_mat),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    // نشانه گر دایره ای زیبا و تاکتیکی موس
    let rc = &config.cursor_reticle;
    let norm_col = Color::srgba(
        rc.normal_color_rgba.0,
        rc.normal_color_rgba.1,
        rc.normal_color_rgba.2,
        rc.normal_color_rgba.3,
    );
    let container_size = rc.outer_diameter_px + rc.fire_expand_px + rc.tick_length_px * 2.0 + 16.0;

    commands
        .spawn((
            ReticleRoot {
                current_spread_px: 0.0,
            },
            Pickable::IGNORE,
            GlobalZIndex(200),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: px(container_size),
                height: px(container_size),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                ReticleOuterRing,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(rc.outer_diameter_px),
                    height: px(rc.outer_diameter_px),
                    border: UiRect::all(px(rc.outer_border_px)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(norm_col),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.10)),
            ));

            root.spawn((
                ReticleInnerRing,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(rc.inner_diameter_px),
                    height: px(rc.inner_diameter_px),
                    border: UiRect::all(px(rc.inner_border_px)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(norm_col),
                BackgroundColor(Color::NONE),
            ));

            root.spawn((
                ReticleCenterDot,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(rc.center_dot_px),
                    height: px(rc.center_dot_px),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(norm_col),
            ));

            for (dir, w, h) in [
                (ReticleTickDir::Top, rc.tick_thickness_px, rc.tick_length_px),
                (ReticleTickDir::Bottom, rc.tick_thickness_px, rc.tick_length_px),
                (ReticleTickDir::Left, rc.tick_length_px, rc.tick_thickness_px),
                (ReticleTickDir::Right, rc.tick_length_px, rc.tick_thickness_px),
            ] {
                root.spawn((
                    dir,
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(w),
                        height: px(h),
                        border_radius: BorderRadius::all(px(1.0)),
                        ..default()
                    },
                    BackgroundColor(norm_col),
                ));
            }
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn laser_sight_gizmo_system(
    mut gizmos: Gizmos,
    time: Res<Time>,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    muzzle_query: Query<&GlobalTransform, With<MuzzlePoint>>,
    trail_query: Query<&BulletTrail>,
    render_state: Res<ClientRenderState>,
    mut core_query: Query<
        (&mut Transform, &mut Visibility),
        (
            With<LaserBeamCore>,
            Without<LaserBeamGlow>,
            Without<LaserPulseBead>,
            Without<LaserImpactDot>,
            Without<LaserImpactRing>,
        ),
    >,
    mut glow_query: Query<
        (&mut Transform, &mut Visibility),
        (
            With<LaserBeamGlow>,
            Without<LaserBeamCore>,
            Without<LaserPulseBead>,
            Without<LaserImpactDot>,
            Without<LaserImpactRing>,
        ),
    >,
    mut bead_query: Query<
        (&LaserPulseBead, &mut Transform, &mut Visibility),
        (
            Without<LaserBeamCore>,
            Without<LaserBeamGlow>,
            Without<LaserImpactDot>,
            Without<LaserImpactRing>,
        ),
    >,
    mut dot_query: Query<
        (&mut Transform, &mut Visibility),
        (
            With<LaserImpactDot>,
            Without<LaserBeamCore>,
            Without<LaserBeamGlow>,
            Without<LaserPulseBead>,
            Without<LaserImpactRing>,
        ),
    >,
    mut ring_query: Query<
        (&mut Transform, &mut Visibility),
        (
            With<LaserImpactRing>,
            Without<LaserBeamCore>,
            Without<LaserBeamGlow>,
            Without<LaserPulseBead>,
            Without<LaserImpactDot>,
        ),
    >,
) {
    for trail in &trail_query {
        let alpha = 1.0 - trail.timer.fraction();
        let dir = (trail.end - trail.start).normalize_or_zero();
        let right = dir.cross(Vec3::Y).normalize_or_zero() * 0.014;

        let core_color = Color::srgba(1.0, 0.94, 0.55, alpha);
        let glow_color = Color::srgba(1.0, 0.55, 0.1, alpha * 0.65);

        gizmos.line(trail.start, trail.end, core_color);
        gizmos.line(trail.start + right, trail.end + right, glow_color);
        gizmos.line(trail.start - right, trail.end - right, glow_color);
    }

    let hide_laser = !match_flow.is_in_game()
        || render_state.is_dead
        || render_state.traversal_active
        || render_state.current_locomotion == "swimming";

    if hide_laser {
        for (_, mut vis) in &mut core_query {
            *vis = Visibility::Hidden;
        }
        for (_, mut vis) in &mut glow_query {
            *vis = Visibility::Hidden;
        }
        for (_, _, mut vis) in &mut bead_query {
            *vis = Visibility::Hidden;
        }
        for (_, mut vis) in &mut dot_query {
            *vis = Visibility::Hidden;
        }
        for (_, mut vis) in &mut ring_query {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let muzzle_pos = if let Ok(muzzle_gtf) = muzzle_query.single() {
        muzzle_gtf.translation()
    } else {
        render_state.soldier_pos
            + Vec3::new(0.0, config.combat.fallback_muzzle_offset_y, 0.0)
            + render_state.aim_world_dir * config.combat.fallback_muzzle_forward
    };

    let laser_end = render_state.laser_hit_pos;
    let delta = laser_end - muzzle_pos;
    let beam_len = delta.length();
    if beam_len <= 0.05 {
        return;
    }
    let beam_dir = delta / beam_len;
    let beam_rot = Quat::from_rotation_arc(Vec3::Y, beam_dir);
    let mid_pos = (muzzle_pos + laser_end) * 0.5;
    let t = time.elapsed_secs();

    let base_r = config.laser.beam_radius;
    let shimmer = 0.85 + 0.15 * (t * config.laser.pulse_speed).sin();
    let core_r = base_r * config.laser.beam_core_ratio * shimmer;
    let glow_r = base_r * (0.90 + 0.18 * (t * config.laser.pulse_speed * 0.7).cos());

    if let Ok((mut core_tf, mut core_vis)) = core_query.single_mut() {
        core_tf.translation = mid_pos;
        core_tf.rotation = beam_rot;
        core_tf.scale = Vec3::new(core_r, beam_len, core_r);
        *core_vis = Visibility::Visible;
    }

    if let Ok((mut glow_tf, mut glow_vis)) = glow_query.single_mut() {
        glow_tf.translation = mid_pos;
        glow_tf.rotation = beam_rot;
        glow_tf.scale = Vec3::new(glow_r, beam_len, glow_r);
        *glow_vis = Visibility::Visible;
    }

    let bead_len = (beam_len * 0.08).clamp(0.25, 0.85);
    let bead_r = base_r * 0.52;
    let speed_norm = config.laser.pulse_speed / beam_len.max(1.0);
    for (bead, mut b_tf, mut b_vis) in &mut bead_query {
        let u = (t * speed_norm + bead.phase).fract();
        b_tf.translation = muzzle_pos.lerp(laser_end, u);
        b_tf.rotation = beam_rot;
        b_tf.scale = Vec3::new(bead_r, bead_len, bead_r);
        *b_vis = Visibility::Visible;
    }

    let pulse = 0.85 + 0.25 * (t * config.laser.impact_pulse_speed).sin();
    if let Ok((mut dot_tf, mut dot_vis)) = dot_query.single_mut() {
        dot_tf.translation = laser_end;
        dot_tf.scale = Vec3::splat(pulse);
        *dot_vis = Visibility::Visible;
    }

    if let Ok((mut ring_tf, mut ring_vis)) = ring_query.single_mut() {
        ring_tf.translation = laser_end;
        ring_tf.rotation = Quat::from_rotation_y(t * 2.5);
        ring_tf.scale = Vec3::splat(0.85 + 0.30 * (t * config.laser.impact_pulse_speed).cos());
        *ring_vis = Visibility::Visible;
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn cursor_reticle_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    render_state: Res<ClientRenderState>,
    shake: Res<CameraShake>,
    shop_ui: Res<ShopUiState>,
    settings_ui: Res<SettingsUiState>,
    mut window_query: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut root_query: Query<(&mut ReticleRoot, &mut Node, &mut Visibility)>,
    mut outer_query: Query<
        (&mut Node, &mut BorderColor),
        (
            With<ReticleOuterRing>,
            Without<ReticleRoot>,
            Without<ReticleInnerRing>,
            Without<ReticleCenterDot>,
            Without<ReticleTickDir>,
        ),
    >,
    mut inner_query: Query<
        &mut BorderColor,
        (
            With<ReticleInnerRing>,
            Without<ReticleOuterRing>,
            Without<ReticleCenterDot>,
            Without<ReticleTickDir>,
        ),
    >,
    mut dot_query: Query<
        &mut BackgroundColor,
        (
            With<ReticleCenterDot>,
            Without<ReticleOuterRing>,
            Without<ReticleInnerRing>,
            Without<ReticleTickDir>,
        ),
    >,
    mut tick_query: Query<
        (&ReticleTickDir, &mut Node, &mut BackgroundColor),
        (
            Without<ReticleRoot>,
            Without<ReticleOuterRing>,
            Without<ReticleInnerRing>,
            Without<ReticleCenterDot>,
        ),
    >,
) {
    let Ok((window, mut cursor_options)) = window_query.single_mut() else {
        return;
    };
    let in_game = match_flow.is_in_game();

    if config.cursor_reticle.hide_os_cursor_in_game {
        let desired_visible = !in_game || shop_ui.is_open || settings_ui.is_open;
        if cursor_options.visible != desired_visible {
            cursor_options.visible = desired_visible;
        }
    }

    let Ok((mut reticle, mut root_node, mut root_vis)) = root_query.single_mut() else {
        return;
    };

    if !in_game || shop_ui.is_open || settings_ui.is_open {
        if *root_vis != Visibility::Hidden {
            *root_vis = Visibility::Hidden;
        }
        return;
    }

    let Some(cursor_pos) = window.cursor_position() else {
        if *root_vis != Visibility::Hidden {
            *root_vis = Visibility::Hidden;
        }
        return;
    };

    if *root_vis != Visibility::Visible {
        *root_vis = Visibility::Visible;
    }
    let rc = &config.cursor_reticle;
    let container_size = rc.outer_diameter_px + rc.fire_expand_px + rc.tick_length_px * 2.0 + 16.0;
    root_node.left = px(cursor_pos.x - container_size * 0.5);
    root_node.top = px(cursor_pos.y - container_size * 0.5);

    let dt = time.delta_secs();
    let target_spread = if render_state.upper_action == UpperBodyAction::FiringRifle {
        rc.fire_expand_px
    } else {
        (shake.trauma * rc.fire_expand_px * 1.4).clamp(0.0, rc.fire_expand_px)
    };
    reticle.current_spread_px = reticle
        .current_spread_px
        + (target_spread - reticle.current_spread_px) * (rc.expand_recover_speed * dt).clamp(0.0, 1.0);

    let active_rgba = if render_state.upper_action == UpperBodyAction::Reloading {
        rc.reload_color_rgba
    } else if render_state.aim_locked_on_enemy {
        rc.locked_color_rgba
    } else {
        rc.normal_color_rgba
    };
    let active_col = Color::srgba(active_rgba.0, active_rgba.1, active_rgba.2, active_rgba.3);

    let outer_d = rc.outer_diameter_px + reticle.current_spread_px;
    if let Ok((mut outer_node, mut outer_border)) = outer_query.single_mut() {
        outer_node.width = px(outer_d);
        outer_node.height = px(outer_d);
        *outer_border = BorderColor::all(active_col);
    }

    if let Ok(mut inner_border) = inner_query.single_mut() {
        *inner_border = BorderColor::all(active_col);
    }

    if let Ok(mut dot_bg) = dot_query.single_mut() {
        *dot_bg = BackgroundColor(active_col);
    }

    let center = container_size * 0.5;
    let outer_r = outer_d * 0.5;
    for (dir, mut tick_node, mut tick_bg) in &mut tick_query {
        *tick_bg = BackgroundColor(active_col);
        match dir {
            ReticleTickDir::Top => {
                tick_node.left = px(center - rc.tick_thickness_px * 0.5);
                tick_node.top = px(center - outer_r - rc.tick_length_px * 0.65);
            }
            ReticleTickDir::Bottom => {
                tick_node.left = px(center - rc.tick_thickness_px * 0.5);
                tick_node.top = px(center + outer_r - rc.tick_length_px * 0.35);
            }
            ReticleTickDir::Left => {
                tick_node.left = px(center - outer_r - rc.tick_length_px * 0.65);
                tick_node.top = px(center - rc.tick_thickness_px * 0.5);
            }
            ReticleTickDir::Right => {
                tick_node.left = px(center + outer_r - rc.tick_length_px * 0.35);
                tick_node.top = px(center - rc.tick_thickness_px * 0.5);
            }
        }
    }
}

fn gameplay_fx_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flash_query: Query<(Entity, &mut MuzzleFlash)>,
    mut trail_query: Query<(Entity, &mut BulletTrail)>,
    mut smoke_query: Query<(Entity, &mut Transform, &mut ExplosionSmokeParticle)>,
    mut led_query: Query<(&GrenadeBlinkLed, &mut PointLight)>,
) {
    let t_secs = time.elapsed_secs();
    let dt = time.delta_secs();
    let blink_on = (t_secs * 16.0).sin() > 0.0;
    for (led, mut light) in &mut led_query {
        light.intensity = if blink_on { 120_000.0 } else { 0.0 };
        if let Some(mut mat) = materials.get_mut(&led.mat) {
            mat.emissive = if blink_on {
                LinearRgba::new(16.0, 0.8, 0.4, 1.0)
            } else {
                LinearRgba::new(0.8, 0.05, 0.05, 1.0)
            };
        }
    }
    for (entity, mut tf, mut sp) in &mut smoke_query {
        sp.timer.tick(time.delta());
        tf.translation += sp.vel * dt;
        sp.vel *= (1.0 - 1.4 * dt).max(0.1);
        let frac = sp.timer.fraction();
        let s = sp.start_scale.lerp(sp.end_scale, frac) * (1.0 - (frac - 0.70).max(0.0) * 3.0).max(0.05);
        tf.scale = Vec3::splat(s);
        if sp.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
    for (entity, mut flash) in &mut flash_query {
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }

    for (entity, mut trail) in &mut trail_query {
        trail.timer.tick(time.delta());
        if trail.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn camera_follow_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    pause_state: Res<LatestPauseState>,
    chat_state: Res<ChatUiState>,
    mut shake: ResMut<CameraShake>,
    mut render_state: ResMut<ClientRenderState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    shop_ui: Res<ShopUiState>,
    mut camera_control: ResMut<CameraControlState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    soldier_query: Query<&Transform, (With<SoldierMarker>, Without<MobaCamera>)>,
    mut camera_query: Query<&mut Transform, With<MobaCamera>>,
    mut last_mid_cursor: Local<Option<Vec2>>,
) {
    if !match_flow.is_in_game() {
        return;
    }
    let Ok(soldier_tf) = soldier_query.single() else { return };
    let Ok(mut camera_tf) = camera_query.single_mut() else { return };

    if !chat_state.is_open && !shop_ui.is_open && keyboard.just_pressed(KeyCode::KeyC) {
        camera_control.camera_unlocked = !camera_control.camera_unlocked;
        if camera_control.camera_unlocked {
            if camera_control.free_look_target.is_none() {
                camera_control.free_look_target = Some(soldier_tf.translation);
            }
        } else {
            camera_control.free_look_target = None;
        }
    }

    if !chat_state.is_open && !shop_ui.is_open && mouse_button.pressed(MouseButton::Middle) {
        if let Ok(window) = window_query.single() {
            if let Some(cursor) = window.cursor_position() {
                if let Some(prev) = *last_mid_cursor {
                    let delta = cursor - prev;
                    if delta.length_squared() > 0.01 {
                        let base = camera_control
                            .free_look_target
                            .unwrap_or(soldier_tf.translation);
                        let pan_speed = 0.065;
                        let half_x = config.map.map_dimensions.0 * 0.5;
                        let half_z = config.map.map_dimensions.1 * 0.5;
                        let next_x = (base.x - delta.x * pan_speed).clamp(-half_x, half_x);
                        let next_z = (base.z - delta.y * pan_speed).clamp(-half_z, half_z);
                        camera_control.free_look_target = Some(Vec3::new(next_x, base.y, next_z));
                    }
                }
                *last_mid_cursor = Some(cursor);
            }
        }
    } else {
        *last_mid_cursor = None;
    }

    let dt = time.delta_secs();
    shake.trauma = (shake.trauma - config.camera.trauma_decay_rate * dt).max(0.0);
    let shake_power = shake.trauma * shake.trauma;

    let rx = render_state.next_fx_rand();
    let ry = render_state.next_fx_rand();
    let rz = render_state.next_fx_rand();
    let amp = config.camera.shake_amplitude;
    let shake_offset = Vec3::new(
        (rx - 0.5) * amp.0 * shake_power,
        (ry - 0.5) * amp.1 * shake_power,
        (rz - 0.5) * amp.2 * shake_power,
    );

    let mut desired_lead = Vec3::ZERO;
    if camera_control.free_look_target.is_none()
        && !camera_control.is_pointer_over_minimap
        && !camera_control.is_dragging_minimap
        && !chat_state.is_open
        && pause_state.0.phase == PausePhase::Running
    {
        if let Ok(window) = window_query.single() {
            if let Some(cursor) = window.cursor_position() {
                let win_w = window.width().max(1.0);
                let win_h = window.height().max(1.0);
                let nx = ((cursor.x / win_w) - 0.5) * 2.0;
                let ny = ((cursor.y / win_h) - 0.5) * 2.0;
                let vec2 = Vec2::new(nx, ny);
                let mag = vec2.length().min(1.0);
                let dz = config.camera.cursor_lead_deadzone_ratio;
                if mag > dz && mag > 1e-4 {
                    let t = ((mag - dz) / (1.0 - dz)).clamp(0.0, 1.0);
                    let smooth = t * t * (3.0 - 2.0 * t);
                    let dir = vec2 / mag;
                    desired_lead = Vec3::new(
                        dir.x * smooth * config.camera.cursor_lead_max_distance_x,
                        0.0,
                        dir.y * smooth * config.camera.cursor_lead_max_distance_z,
                    );
                }
            }
        }
    }

    let lead_alpha = (1.0 - (-config.camera.cursor_lead_lerp_speed * dt).exp()).clamp(0.0, 1.0);
    camera_control.current_cursor_lead =
        camera_control.current_cursor_lead.lerp(desired_lead, lead_alpha);

    let camera_offset = Vec3::new(
        config.camera.offset.0,
        config.camera.offset.1,
        config.camera.offset.2,
    );
    let focus_pos = camera_control
        .free_look_target
        .unwrap_or(soldier_tf.translation + camera_control.current_cursor_lead);
    let target_pos = focus_pos + camera_offset + shake_offset;

    let cam_alpha = (1.0 - (-config.camera.follow_lerp_speed * dt).exp()).clamp(0.0, 1.0);
    camera_tf.translation = camera_tf.translation.lerp(target_pos, cam_alpha);
    camera_tf.look_at(
        focus_pos + shake_offset * config.camera.look_at_shake_factor,
        Vec3::Y,
    );
}

fn soldier_animation_player_system(
    config: Res<GameConfig>,
    hero_hud: Res<LatestHeroSnapshot>,
    mut render_state: ResMut<ClientRenderState>,
    anim_assets: Option<Res<SoldierAnimationAssets>>,
    mut lower_query: Query<
        (&mut AnimationPlayer, &mut AnimationTransitions),
        (With<LowerBodyAnimationPlayer>, Without<UpperBodyAnimationPlayer>),
    >,
    mut upper_query: Query<
        (&mut AnimationPlayer, &mut AnimationTransitions),
        (With<UpperBodyAnimationPlayer>, Without<LowerBodyAnimationPlayer>),
    >,
) {
    let Some(anim_assets) = anim_assets else { return };
    let Ok((mut lower_player, mut lower_trans)) = lower_query.single_mut() else { return };
    let Ok((mut upper_player, mut upper_trans)) = upper_query.single_mut() else { return };

    if render_state.is_dead {
        let die_key = render_state.death_anim_name.clone();
        if render_state.previous_lower_anim != die_key {
            let node = anim_assets.require_node(&die_key);
            let blend = Duration::from_millis(config.animations.death_blend_ms);
            lower_trans.play(&mut lower_player, node, blend).replay();
            upper_trans.play(&mut upper_player, node, blend).replay();
            render_state.previous_lower_anim = die_key.clone();
            render_state.previous_upper_anim = die_key;
        }
        return;
    }

    if render_state.traversal_active {
        let trav_key = render_state.traversal_anim_name.clone();
        if render_state.previous_lower_anim != trav_key {
            let node = anim_assets.require_node(&trav_key);
            let blend = Duration::from_millis(config.animations.traversal_blend_ms);
            let server_dur = render_state.traversal_duration.max(0.05);
            let clip_dur = anim_assets.clip_duration(&trav_key).unwrap_or(server_dur);
            let speed = (clip_dur / server_dur).max(0.1);
            let l_act = lower_trans.play(&mut lower_player, node, blend);
            l_act.set_speed(speed);
            l_act.replay();
            let u_act = upper_trans.play(&mut upper_player, node, blend);
            u_act.set_speed(speed);
            u_act.replay();
            render_state.previous_lower_anim = trav_key.clone();
            render_state.previous_upper_anim = trav_key;
        }
        return;
    }

    let fallback_anim = if render_state.current_locomotion == "swimming" {
        "walk_forward"
    } else {
        "idle"
    };
    let lower_node = anim_assets.get_or_fallback(&render_state.current_locomotion, fallback_anim);
    let loco_blend = Duration::from_millis(config.animations.locomotion_blend_ms);

    if render_state.current_locomotion != render_state.previous_lower_anim
        || !lower_player.is_playing_animation(lower_node)
    {
        lower_trans
            .play(&mut lower_player, lower_node, loco_blend)
            .repeat();
        render_state.previous_lower_anim = render_state.current_locomotion.clone();
    }

    match render_state.upper_action {
        UpperBodyAction::TossGrenade => {
            if render_state.previous_upper_anim != "toss_grenade" {
                let node = anim_assets.require_node("toss_grenade");
                let throw_dur = hero_hud.0.grenade_throw_duration_secs.max(0.05);
                let clip_dur = anim_assets.clip_duration("toss_grenade").unwrap_or(throw_dur);
                let speed = (clip_dur / throw_dur).max(0.1);
                let active = upper_trans.play(
                    &mut upper_player,
                    node,
                    Duration::from_millis(config.animations.grenade_blend_ms),
                );
                active.set_speed(speed);
                active.replay();
                render_state.previous_upper_anim = "toss_grenade".to_string();
            }
        }
        UpperBodyAction::Reloading => {
            if render_state.previous_upper_anim != "reloading" {
                let node = anim_assets.require_node("reloading");
                let base_dur = config.exact_anim_duration("reloading");
                let speed = (base_dur / hero_hud.0.reload_duration_secs.max(0.1)).max(0.5);
                let active = upper_trans.play(
                    &mut upper_player,
                    node,
                    Duration::from_millis(config.animations.reload_blend_ms),
                );
                active.set_speed(speed);
                active.replay();
                render_state.previous_upper_anim = "reloading".to_string();
            }
        }
        UpperBodyAction::FiringRifle => {
            if hero_hud.0.active_weapon == ActiveWeaponSlot::Pistol {
                let pistol_node = anim_assets.require_node("pistol_run");
                if render_state.previous_upper_anim != "pistol_run"
                    || !upper_player.is_playing_animation(pistol_node)
                {
                    let node = anim_assets.require_node("pistol_run");
                    upper_trans
                        .play(&mut upper_player, node, loco_blend)
                        .repeat();
                    render_state.previous_upper_anim = "pistol_run".to_string();
                }
            } else if render_state.previous_upper_anim != "firing_rifle" {
                let node = anim_assets.require_node("firing_rifle");
                upper_trans
                    .play(
                        &mut upper_player,
                        node,
                        Duration::from_millis(config.animations.firing_blend_ms),
                    )
                    .replay();
                render_state.previous_upper_anim = "firing_rifle".to_string();
            }
        }
        UpperBodyAction::None => {
            let in_deep_water_anim = render_state.current_locomotion == "swimming"
                || render_state.current_locomotion == "treading_water";
            if hero_hud.0.active_weapon == ActiveWeaponSlot::Pistol && !in_deep_water_anim {
                let node = anim_assets.require_node("pistol_run");
                if render_state.previous_upper_anim != "pistol_run"
                    || !upper_player.is_playing_animation(node)
                {
                    upper_trans
                        .play(&mut upper_player, node, loco_blend)
                        .repeat();
                    render_state.previous_upper_anim = "pistol_run".to_string();
                }
            } else if render_state.previous_upper_anim != render_state.current_locomotion
                || !upper_player.is_playing_animation(lower_node)
            {
                let lower_seek_opt = lower_player.animation(lower_node).map(|a| a.seek_time());
                let active = upper_trans
                    .play(&mut upper_player, lower_node, loco_blend)
                    .repeat();
                if let Some(lower_seek) = lower_seek_opt {
                    active.seek_to(lower_seek);
                }
                render_state.previous_upper_anim = render_state.current_locomotion.clone();
            }
        }
    }
}


fn update_map_object_labels_system(
    config: Res<GameConfig>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    target_transforms: Query<&GlobalTransform>,
    mut label_query: Query<(&MapObjectLabel, &mut Node, &mut Visibility)>,
) {
    if !config.map.show_object_labels {
        for (_, _, mut vis) in &mut label_query {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Ok((camera, camera_gtf)) = camera_query.single() else {
        return;
    };

    let win_w = window.width();
    let win_h = window.height();

    for (label, mut node, mut vis) in &mut label_query {
        let Ok(target_gtf) = target_transforms.get(label.target_entity) else {
            *vis = Visibility::Hidden;
            continue;
        };

        let world_pos = target_gtf.translation() + label.world_offset;
        if let Ok(viewport_pos) = camera.world_to_viewport(camera_gtf, world_pos) {
            if viewport_pos.x >= -80.0
                && viewport_pos.x <= win_w + 80.0
                && viewport_pos.y >= -40.0
                && viewport_pos.y <= win_h + 40.0
            {
                node.left = px(viewport_pos.x);
                node.top = px(viewport_pos.y);
                *vis = Visibility::Visible;
            } else {
                *vis = Visibility::Hidden;
            }
        } else {
            *vis = Visibility::Hidden;
        }
    }
}


// -----------------------------------------------------------------------------
// سیستم مینی‌مپ دقیق (پایین سمت راست) + کلیک و درگ برای انتقال دوربین
// -----------------------------------------------------------------------------

fn world_to_minimap_uv(config: &GameConfig, world_x: f32, world_z: f32) -> Vec2 {
    let map_w = config.map.map_dimensions.0.max(1.0);
    let map_h = config.map.map_dimensions.1.max(1.0);
    Vec2::new(
        (world_x / map_w + 0.5).clamp(0.0, 1.0),
        (world_z / map_h + 0.5).clamp(0.0, 1.0),
    )
}

fn spawn_minimap_ui(commands: &mut Commands, config: &GameConfig, minimap_handle: Handle<Image>, fog_handle: Handle<Image>) {
    let mut target_dots = Vec::new();
    for index in 0..config.targets.len() {
        target_dots.push(index);
    }

    commands
        .spawn((
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                right: px(MINIMAP_MARGIN_PX),
                bottom: px(MINIMAP_MARGIN_PX),
                width: px(MINIMAP_WIDTH_PX),
                height: px(MINIMAP_HEIGHT_PX),
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::all(px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(Color::srgba(0.65, 0.72, 0.80, 0.85)),
            BackgroundColor(Color::srgba(0.06, 0.08, 0.11, 0.94)),
        ))
        .with_children(|parent| {
            // تصویر پس‌زمینه رندر شده از تمام مسیرها، رودها، کوه‌ها، درخت‌ها و سازه‌های نقشه
            parent.spawn((
                ImageNode::new(minimap_handle),
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    ..default()
                },
            ));

            // لایه زنده Fog of War روی مینی‌مپ
            parent.spawn((
                ImageNode::new(fog_handle),
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    ..default()
                },
            ));

            // کادر محدوده دید فعلی دوربین
            parent.spawn((
                MinimapCameraBox,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    width: px(28.0),
                    height: px(20.0),
                    border: UiRect::all(px(1.5)),
                    ..default()
                },
                BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.80)),
                BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.05)),
            ));

            // نشانگر زنده اهداف تیراندازی
            for idx in target_dots {
                parent.spawn((
                    MinimapTargetDot { index: idx },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        width: px(6.0),
                        height: px(6.0),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                    BackgroundColor(Color::srgb(1.0, 0.65, 0.15)),
                    Visibility::Visible,
                ));
            }

            // نشانگر زنده بازیکن (سرباز)
            parent.spawn((
                MinimapSoldierDot,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(0.0),
                    width: px(9.0),
                    height: px(9.0),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(Color::BLACK),
                BackgroundColor(Color::srgb(1.0, 0.96, 0.35)),
            ));

            // راهنمای بازگشت دوربین به بازیکن
            parent.spawn((
                Text::new("Y: Center Cam"),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgba(0.92, 0.95, 1.0, 0.78)),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(6.0),
                    top: px(4.0),
                    ..default()
                },
            ));
        });
}

fn minimap_interaction_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<GameConfig>,
    chat_state: Res<ChatUiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    render_state: Res<ClientRenderState>,
    courier_ctrl: Res<CourierControlState>,
    mut inspected: ResMut<InspectedUnit>,
    mut hero_move_target: ResMut<HeroClickMoveTarget>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut camera_control: ResMut<CameraControlState>,
) {
    if !chat_state.is_open {
        let wasd_pressed = keyboard.pressed(KeyCode::KeyW)
            || keyboard.pressed(KeyCode::KeyA)
            || keyboard.pressed(KeyCode::KeyS)
            || keyboard.pressed(KeyCode::KeyD);

        if keyboard.just_pressed(KeyCode::KeyY) || keyboard.just_pressed(KeyCode::Home) {
            camera_control.camera_unlocked = false;
            camera_control.free_look_target = None;
        } else if !camera_control.camera_unlocked && wasd_pressed && !camera_control.is_dragging_minimap {
            camera_control.free_look_target = None;
        }
    }

    let Ok(window) = window_query.single() else {
        camera_control.is_pointer_over_minimap = false;
        camera_control.is_dragging_minimap = false;
        return;
    };

    let Some(cursor) = window.cursor_position() else {
        camera_control.is_pointer_over_minimap = false;
        if !mouse_button.pressed(MouseButton::Left) {
            camera_control.is_dragging_minimap = false;
        }
        return;
    };

    let min_x = window.width() - MINIMAP_MARGIN_PX - MINIMAP_WIDTH_PX;
    let max_x = window.width() - MINIMAP_MARGIN_PX;
    let min_y = window.height() - MINIMAP_MARGIN_PX - MINIMAP_HEIGHT_PX;
    let max_y = window.height() - MINIMAP_MARGIN_PX;

    let inside = cursor.x >= min_x && cursor.x <= max_x && cursor.y >= min_y && cursor.y <= max_y;
    camera_control.is_pointer_over_minimap = inside;

    let alt_held = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if inside && !alt_held && mouse_button.just_pressed(MouseButton::Left) {
        camera_control.is_dragging_minimap = true;
    }
    if !mouse_button.pressed(MouseButton::Left) {
        camera_control.is_dragging_minimap = false;
    }

    if camera_control.is_dragging_minimap {
        let u = ((cursor.x - min_x) / MINIMAP_WIDTH_PX).clamp(0.0, 1.0);
        let v = ((cursor.y - min_y) / MINIMAP_HEIGHT_PX).clamp(0.0, 1.0);
        let world_x = (u - 0.5) * config.map.map_dimensions.0;
        let world_z = (v - 0.5) * config.map.map_dimensions.1;
        camera_control.free_look_target = Some(Vec3::new(world_x, render_state.soldier_pos.y, world_z));
    }

    if inside && !chat_state.is_open && mouse_button.just_pressed(MouseButton::Right) {
        let u = ((cursor.x - min_x) / MINIMAP_WIDTH_PX).clamp(0.0, 1.0);
        let v = ((cursor.y - min_y) / MINIMAP_HEIGHT_PX).clamp(0.0, 1.0);
        let world_x = (u - 0.5) * config.map.map_dimensions.0;
        let world_z = (v - 0.5) * config.map.map_dimensions.1;
        let target_pos = Vec3::new(world_x, 0.0, world_z);
        spawn_green_move_arrows(&mut commands, &mut meshes, &mut materials, target_pos);
        if courier_ctrl.selected {
            outgoing.0.push(ClientLobbyMessage::CourierMoveTo { world_pos: target_pos });
        } else {
            *inspected = InspectedUnit::OwnHero;
            hero_move_target.0 = Some(target_pos);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn minimap_overlay_update_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    render_state: Res<ClientRenderState>,
    hero_hud: Res<LatestHeroSnapshot>,
    latest_creeps: Res<LatestCreeps>,
    latest_couriers: Res<LatestCouriers>,
    match_flow: Res<ClientMatchFlow>,
    camera_control: Res<CameraControlState>,
    soldier_query: Query<&Transform, With<SoldierMarker>>,
    target_query: Query<&VisualTarget>,
    minimap_root_q: Query<Entity, With<MinimapRoot>>,
    mut soldier_dot_query: Query<
        &mut Node,
        (
            With<MinimapSoldierDot>,
            Without<MinimapTargetDot>,
            Without<MinimapCameraBox>,
            Without<MinimapCreepDot>,
            Without<MinimapCourierDot>,
        ),
    >,
    mut target_dot_query: Query<
        (&MinimapTargetDot, &mut Node, &mut Visibility),
        (
            Without<MinimapSoldierDot>,
            Without<MinimapCameraBox>,
            Without<MinimapCreepDot>,
            Without<MinimapCourierDot>,
        ),
    >,
    mut cam_box_query: Query<
        &mut Node,
        (
            With<MinimapCameraBox>,
            Without<MinimapSoldierDot>,
            Without<MinimapTargetDot>,
            Without<MinimapCreepDot>,
            Without<MinimapCourierDot>,
        ),
    >,
    mut creep_dot_query: Query<
        (Entity, &MinimapCreepDot, &mut Node, &mut BackgroundColor),
        (
            Without<MinimapSoldierDot>,
            Without<MinimapTargetDot>,
            Without<MinimapCameraBox>,
            Without<MinimapCourierDot>,
        ),
    >,
    mut courier_dot_query: Query<
        (Entity, &MinimapCourierDot, &mut Node, &mut BackgroundColor),
        (
            Without<MinimapSoldierDot>,
            Without<MinimapTargetDot>,
            Without<MinimapCameraBox>,
            Without<MinimapCreepDot>,
        ),
    >,
) {
    let soldier_pos = soldier_query
        .single()
        .map(|tf| tf.translation)
        .unwrap_or(render_state.soldier_pos);

    if let Ok(mut dot_node) = soldier_dot_query.single_mut() {
        let uv = world_to_minimap_uv(&config, soldier_pos.x, soldier_pos.z);
        dot_node.left = px(uv.x * MINIMAP_WIDTH_PX - 4.5);
        dot_node.top = px(uv.y * MINIMAP_HEIGHT_PX - 4.5);
    }

    let focus = camera_control.free_look_target.unwrap_or(soldier_pos);
    if let Ok(mut cam_node) = cam_box_query.single_mut() {
        let uv = world_to_minimap_uv(&config, focus.x, focus.z);
        let box_w = 28.0;
        let box_h = 20.0;
        cam_node.left = px((uv.x * MINIMAP_WIDTH_PX - box_w * 0.5).clamp(0.0, MINIMAP_WIDTH_PX - box_w));
        cam_node.top = px((uv.y * MINIMAP_HEIGHT_PX - box_h * 0.5).clamp(0.0, MINIMAP_HEIGHT_PX - box_h));
    }

    for (dot, mut node, mut vis) in &mut target_dot_query {
        if let Some(vt) = target_query.iter().find(|t| t.index == dot.index) {
            if vt.is_dead || !vt.is_server_visible {
                *vis = Visibility::Hidden;
            } else {
                *vis = Visibility::Visible;
                let uv = world_to_minimap_uv(&config, vt.base_pos.x, vt.base_pos.z);
                node.left = px(uv.x * MINIMAP_WIDTH_PX - 3.0);
                node.top = px(uv.y * MINIMAP_HEIGHT_PX - 3.0);
            }
        }
    }

    // نمایش زنده سربازان لاین (سبز خودی / قرمز دشمن) و کمپ‌های قابل‌مشاهده روی مینی‌مپ
    let my_team = hero_hud.0.team;
    let alive_creeps: HashMap<u64, &CreepSnapshot> = latest_creeps
        .0
        .iter()
        .filter(|c| !c.is_dead && c.is_visible)
        .map(|c| (c.id, c))
        .collect();

    let mut existing_dots = HashSet::new();
    for (ent, dot, mut node, mut bg) in &mut creep_dot_query {
        let Some(&cs) = alive_creeps.get(&dot.creep_id) else {
            commands.entity(ent).despawn();
            continue;
        };
        existing_dots.insert(cs.id);
        let uv = world_to_minimap_uv(&config, cs.pos.x, cs.pos.z);
        node.left = px(uv.x * MINIMAP_WIDTH_PX - 2.2);
        node.top = px(uv.y * MINIMAP_HEIGHT_PX - 2.2);
        *bg = if cs.team == my_team {
            Color::srgb(0.20, 0.92, 0.38).into()
        } else if cs.team == TeamId::Neutral {
            Color::srgb(0.98, 0.62, 0.16).into()
        } else {
            Color::srgb(0.95, 0.22, 0.22).into()
        };
    }

    if let Ok(root_ent) = minimap_root_q.single() {
        for cs in alive_creeps.values() {
            if existing_dots.contains(&cs.id) {
                continue;
            }
            let uv = world_to_minimap_uv(&config, cs.pos.x, cs.pos.z);
            let dot_col = if cs.team == my_team {
                Color::srgb(0.20, 0.92, 0.38)
            } else if cs.team == TeamId::Neutral {
                Color::srgb(0.98, 0.62, 0.16)
            } else {
                Color::srgb(0.95, 0.22, 0.22)
            };
            commands.entity(root_ent).with_children(|parent| {
                parent.spawn((
                    MinimapCreepDot { creep_id: cs.id },
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(uv.x * MINIMAP_WIDTH_PX - 2.2),
                        top: px(uv.y * MINIMAP_HEIGHT_PX - 2.2),
                        width: px(4.5),
                        height: px(4.5),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(dot_col),
                ));
            });
        }
    }

    // نمایش Courierهای زنده و قابل‌مشاهده روی مینی‌مپ
    let my_slot = match_flow.your_slot;
    let alive_couriers: HashMap<usize, &CourierSnapshot> = latest_couriers
        .0
        .iter()
        .filter(|c| !c.is_dead && c.is_visible)
        .map(|c| (c.owner_slot, c))
        .collect();
    let mut existing_cour_dots = HashSet::new();
    for (ent, cdot, mut node, mut bg) in &mut courier_dot_query {
        let Some(&cs) = alive_couriers.get(&cdot.owner_slot) else {
            commands.entity(ent).despawn();
            continue;
        };
        existing_cour_dots.insert(cdot.owner_slot);
        let uv = world_to_minimap_uv(&config, cs.pos.x, cs.pos.z);
        node.left = px(uv.x * MINIMAP_WIDTH_PX - 3.5);
        node.top = px(uv.y * MINIMAP_HEIGHT_PX - 3.5);
        bg.0 = if Some(cs.owner_slot) == my_slot {
            Color::srgb(0.20, 0.92, 1.0)
        } else if cs.team == my_team {
            Color::srgb(0.25, 0.92, 0.45)
        } else {
            Color::srgb(0.98, 0.25, 0.25)
        };
    }
    if let Ok(root_ent) = minimap_root_q.single() {
        for (&owner_slot, &cs) in &alive_couriers {
            if existing_cour_dots.contains(&owner_slot) {
                continue;
            }
            let uv = world_to_minimap_uv(&config, cs.pos.x, cs.pos.z);
            let col = if Some(owner_slot) == my_slot {
                Color::srgb(0.20, 0.92, 1.0)
            } else if cs.team == my_team {
                Color::srgb(0.25, 0.92, 0.45)
            } else {
                Color::srgb(0.98, 0.25, 0.25)
            };
            commands.entity(root_ent).with_children(|p| {
                p.spawn((
                    MinimapCourierDot { owner_slot },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(uv.x * MINIMAP_WIDTH_PX - 3.5),
                        top: px(uv.y * MINIMAP_HEIGHT_PX - 3.5),
                        width: px(7.0),
                        height: px(7.0),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(2.0)),
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                    BackgroundColor(col),
                ));
            });
        }
    }
}


fn bake_minimap_image(config: &GameConfig) -> Image {
    let w = MINIMAP_TEX_W as usize;
    let h = MINIMAP_TEX_H as usize;
    let mut pixels = vec![0u8; w * h * 4];

    let map_w = config.map.map_dimensions.0.max(1.0);
    let map_h = config.map.map_dimensions.1.max(1.0);
    let px_per_unit_x = (w as f32) / map_w;
    let px_per_unit_z = (h as f32) / map_h;

    let to_px = |wx: f32, wz: f32| -> (f32, f32) {
        (
            (wx / map_w + 0.5) * (w as f32),
            (wz / map_h + 0.5) * (h as f32),
        )
    };

    let put_pixel = |buf: &mut [u8], x: i32, y: i32, rgba: [u8; 4]| {
        if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
            let idx = ((y as usize) * w + (x as usize)) * 4;
            let a = rgba[3] as f32 / 255.0;
            let inv_a = 1.0 - a;
            buf[idx] = (buf[idx] as f32 * inv_a + rgba[0] as f32 * a).round() as u8;
            buf[idx + 1] = (buf[idx + 1] as f32 * inv_a + rgba[1] as f32 * a).round() as u8;
            buf[idx + 2] = (buf[idx + 2] as f32 * inv_a + rgba[2] as f32 * a).round() as u8;
            buf[idx + 3] = 255;
        }
    };

    // پس‌زمینه دره و ابرهای حاشیه نقشه
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            pixels[idx] = 20;
            pixels[idx + 1] = 24;
            pixels[idx + 2] = 30;
            pixels[idx + 3] = 255;
        }
    }

    let rgb_to_u8 = |c: (f32, f32, f32), boost: f32, alpha: u8| -> [u8; 4] {
        [
            ((c.0 * boost).clamp(0.0, 1.0) * 255.0) as u8,
            ((c.1 * boost).clamp(0.0, 1.0) * 255.0) as u8,
            ((c.2 * boost).clamp(0.0, 1.0) * 255.0) as u8,
            alpha,
        ]
    };

    let draw_rotated_rect = |buf: &mut [u8], cx: f32, cz: f32, sx: f32, sz: f32, yaw_deg: f32, rgba: [u8; 4]| {
        let (pcx, pcz) = to_px(cx, cz);
        let hx = (sx * 0.5) * px_per_unit_x;
        let hz = (sz * 0.5) * px_per_unit_z;
        let bound = (hx * hx + hz * hz).sqrt().ceil() as i32 + 1;
        let rad = yaw_deg.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        let min_x = (pcx as i32 - bound).max(0);
        let max_x = (pcx as i32 + bound).min(w as i32 - 1);
        let min_y = (pcz as i32 - bound).max(0);
        let max_y = (pcz as i32 + bound).min(h as i32 - 1);

        for py in min_y..=max_y {
            for px_i in min_x..=max_x {
                let dx = (px_i as f32 + 0.5) - pcx;
                let dy = (py as f32 + 0.5) - pcz;
                let lx = dx * cos_a - dy * sin_a;
                let ly = dx * sin_a + dy * cos_a;
                if lx.abs() <= hx && ly.abs() <= hz {
                    put_pixel(buf, px_i, py, rgba);
                }
            }
        }
    };

    let draw_rotated_ellipse = |buf: &mut [u8], cx: f32, cz: f32, sx: f32, sz: f32, yaw_deg: f32, rgba: [u8; 4]| {
        let (pcx, pcz) = to_px(cx, cz);
        let hx = (sx * 0.5 * px_per_unit_x).max(1.0);
        let hz = (sz * 0.5 * px_per_unit_z).max(1.0);
        let bound = hx.max(hz).ceil() as i32 + 1;
        let rad = yaw_deg.to_radians();
        let cos_a = rad.cos();
        let sin_a = rad.sin();

        let min_x = (pcx as i32 - bound).max(0);
        let max_x = (pcx as i32 + bound).min(w as i32 - 1);
        let min_y = (pcz as i32 - bound).max(0);
        let max_y = (pcz as i32 + bound).min(h as i32 - 1);

        for py in min_y..=max_y {
            for px_i in min_x..=max_x {
                let dx = (px_i as f32 + 0.5) - pcx;
                let dy = (py as f32 + 0.5) - pcz;
                let lx = (dx * cos_a - dy * sin_a) / hx;
                let ly = (dx * sin_a + dy * cos_a) / hz;
                if lx * lx + ly * ly <= 1.0 {
                    put_pixel(buf, px_i, py, rgba);
                }
            }
        }
    };

    let draw_circle = |buf: &mut [u8], cx: f32, cz: f32, radius_px: f32, rgba: [u8; 4]| {
        let (pcx, pcz) = to_px(cx, cz);
        let r = radius_px.max(1.0);
        let ri = r.ceil() as i32 + 1;
        for py in (pcz as i32 - ri)..=(pcz as i32 + ri) {
            for px_i in (pcx as i32 - ri)..=(pcx as i32 + ri) {
                let dx = (px_i as f32 + 0.5) - pcx;
                let dy = (py as f32 + 0.5) - pcz;
                if dx * dx + dy * dy <= r * r {
                    put_pixel(buf, px_i, py, rgba);
                }
            }
        }
    };

    let draw_segment = |buf: &mut [u8], ax: f32, az: f32, bx: f32, bz: f32, half_width_px: f32, rgba: [u8; 4]| {
        let (pax, paz) = to_px(ax, az);
        let (pbx, pbz) = to_px(bx, bz);
        let hw = half_width_px.max(1.0);
        let pad = hw.ceil() as i32 + 1;
        let min_x = (pax.min(pbx) as i32 - pad).max(0);
        let max_x = (pax.max(pbx) as i32 + pad).min(w as i32 - 1);
        let min_y = (paz.min(pbz) as i32 - pad).max(0);
        let max_y = (paz.max(pbz) as i32 + pad).min(h as i32 - 1);

        let abx = pbx - pax;
        let aby = pbz - paz;
        let ab_len_sq = abx * abx + aby * aby;

        for py in min_y..=max_y {
            for px_i in min_x..=max_x {
                let ppx = px_i as f32 + 0.5;
                let ppy = py as f32 + 0.5;
                let t = if ab_len_sq > 1e-4 {
                    (((ppx - pax) * abx + (ppy - paz) * aby) / ab_len_sq).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let cx = pax + abx * t;
                let cy = paz + aby * t;
                let dx = ppx - cx;
                let dy = ppy - cy;
                if dx * dx + dy * dy <= hw * hw {
                    put_pixel(buf, px_i, py, rgba);
                }
            }
        }
    };

    // ۱. پلتفرم‌های زمین و پایگاه‌ها
    for plat in &config.map.terrains {
        let boost = if plat.elevation_tier >= 2 { 1.35 } else { 1.15 };
        let col = rgb_to_u8(plat.color_rgb, boost, 255);
        let loc = plat.model_transform.location;
        let scl = plat.model_transform.scale;
        draw_rotated_rect(&mut pixels, loc.0, loc.2, plat.size.0 * scl.0, plat.size.2 * scl.2, plat.model_transform.rotation.1, col);
    }

    // ۲. برکه‌ها و جویبارها
    for seg in &config.map.river_segments {
        let col = rgb_to_u8(seg.color_rgb, 1.45, 235);
        let loc = seg.model_transform.location;
        let scl = seg.model_transform.scale;
        draw_rotated_ellipse(&mut pixels, loc.0, loc.2, seg.size.0 * scl.0, seg.size.2 * scl.2, seg.model_transform.rotation.1, col);
    }

    // ۳. مسیرهای ۳ لاین اصلی (Top, Mid, Bot)
    for lane in &config.map.lanes {
        let border_col = rgb_to_u8(lane.color_rgb, 0.72, 255);
        let road_col = rgb_to_u8(lane.color_rgb, 1.28, 255);
        let hw = lane.width * 0.5 * px_per_unit_x;
        for seg_idx in 0..lane.waypoints.len().saturating_sub(1) {
            let a = lane.waypoints[seg_idx];
            let b = lane.waypoints[seg_idx + 1];
            draw_segment(&mut pixels, a.0, a.2, b.0, b.2, hw + 1.2, border_col);
        }
        for seg_idx in 0..lane.waypoints.len().saturating_sub(1) {
            let a = lane.waypoints[seg_idx];
            let b = lane.waypoints[seg_idx + 1];
            draw_segment(&mut pixels, a.0, a.2, b.0, b.2, hw, road_col);
        }
    }

    // ۴. رمپ‌ها و پلکان‌های سنگی
    for ramp in &config.map.ramps {
        let col = rgb_to_u8(ramp.color_rgb, 1.35, 255);
        let loc = ramp.model_transform.location;
        let scl = ramp.model_transform.scale;
        draw_rotated_rect(
            &mut pixels,
            loc.0,
            loc.2,
            ramp.size.0 * scl.0,
            ramp.size.2 * scl.2,
            ramp.model_transform.rotation.1,
            col,
        );
    }

    // ۵. میدان باستانی مرکز نقشه
    let bridge = &config.map.center_bridge;
    let bridge_loc = bridge.model_transform.location;
    let bridge_s = bridge.model_transform.scale.0.max(bridge.model_transform.scale.2);
    let bridge_r = bridge.outer_radius * bridge_s * px_per_unit_x;
    draw_circle(
        &mut pixels,
        bridge_loc.0,
        bridge_loc.2,
        bridge_r + 1.5,
        [60, 58, 52, 255],
    );
    draw_circle(
        &mut pixels,
        bridge_loc.0,
        bridge_loc.2,
        bridge_r,
        rgb_to_u8(bridge.color_rgb, 1.25, 255),
    );
    draw_circle(
        &mut pixels,
        bridge_loc.0,
        bridge_loc.2,
        4.6 * px_per_unit_x,
        [55, 215, 235, 255],
    );

    // ۶. آبشارها
    for wf in &config.map.waterfalls {
        let loc = wf.model_transform.location;
        let scl = wf.model_transform.scale;
        draw_rotated_rect(
            &mut pixels,
            loc.0,
            loc.2,
            wf.size.0 * scl.0,
            (wf.size.2 * scl.2).max(2.0),
            wf.model_transform.rotation.1,
            [140, 240, 255, 245],
        );
    }

    // ۷. کوه‌ها و دیوارهای سنگی
    for mtn in &config.map.mountains {
        let loc = mtn.model_transform.location;
        let scl = mtn.model_transform.scale;
        let sx = mtn.size.0 * scl.0;
        let sz = mtn.size.2 * scl.2;
        draw_rotated_rect(
            &mut pixels,
            loc.0,
            loc.2,
            sx + 1.5,
            sz + 1.5,
            mtn.model_transform.rotation.1,
            [32, 34, 38, 255],
        );
        draw_rotated_rect(
            &mut pixels,
            loc.0,
            loc.2,
            sx,
            sz,
            mtn.model_transform.rotation.1,
            rgb_to_u8(mtn.color_rgb, 1.25, 255),
        );
    }

    // ۸. موانع تاکتیکی و خرابه‌ها (Obstacles)
    for obs in &config.obstacles {
        let loc = obs.model_transform.location;
        let scl = obs.model_transform.scale;
        draw_rotated_rect(
            &mut pixels,
            loc.0,
            loc.2,
            (obs.size.0 * scl.0).max(2.0),
            (obs.size.2 * scl.2).max(2.0),
            obs.model_transform.rotation.1,
            rgb_to_u8(obs.color_rgb, 1.4, 255),
        );
    }

    // ۹. درختان جنگل
    for tree in &config.map.trees {
        let loc = tree.model_transform.location;
        let ts = tree.model_transform.scale.0.max(tree.model_transform.scale.2);
        let tr = (tree.canopy_radius * ts * px_per_unit_x).max(2.2);
        draw_circle(&mut pixels, loc.0, loc.2, tr + 0.9, [18, 32, 18, 240]);
        draw_circle(
            &mut pixels,
            loc.0,
            loc.2,
            tr,
            rgb_to_u8(tree.canopy_color_rgb, 1.35, 255),
        );
    }

    // ۱۰. چشمه‌های پایگاه (Fountains)
    for ftn in &config.map.fountains {
        let loc = ftn.model_transform.location;
        let fs = ftn.model_transform.scale.0.max(ftn.model_transform.scale.2);
        let fr = (ftn.outer_radius * fs * px_per_unit_x).max(4.0);
        let col = if ftn.team == TeamId::Dawn {
            [50, 235, 195, 255]
        } else {
            [245, 85, 70, 255]
        };
        draw_circle(&mut pixels, loc.0, loc.2, fr + 1.2, [255, 255, 255, 210]);
        draw_circle(&mut pixels, loc.0, loc.2, fr, col);
    }

    // ۱۰-ب. باراکس‌ها (Barracks)
    for bar in &config.map.barracks {
        let loc = bar.model_transform.location;
        let col = if bar.team == TeamId::Dawn {
            [40, 210, 165, 255]
        } else {
            [235, 70, 60, 255]
        };
        draw_rotated_rect(&mut pixels, loc.0, loc.2, bar.size.0 + 1.4, bar.size.2 + 1.4, 0.0, [15, 18, 22, 255]);
        draw_rotated_rect(&mut pixels, loc.0, loc.2, bar.size.0, bar.size.2, 0.0, col);
    }

    // ۱۱. برج‌های دفاعی (Turrets)
    for tur in &config.map.turrets {
        let loc = tur.model_transform.location;
        let col = if tur.team == TeamId::Dawn {
            [45, 235, 175, 255]
        } else {
            [245, 75, 65, 255]
        };
        draw_circle(&mut pixels, loc.0, loc.2, 4.5, [15, 18, 22, 255]);
        draw_circle(&mut pixels, loc.0, loc.2, 3.3, col);
        draw_circle(&mut pixels, loc.0, loc.2, 1.3, [255, 255, 255, 255]);
    }

    // ۱۱-ب. جایگاه ۳۰ کمپ جنگلی (Neutral Camps)
    for camp in &config.neutral_camps.camps {
        draw_circle(&mut pixels, camp.center.0, camp.center.2, 4.0, [18, 16, 12, 235]);
        draw_circle(&mut pixels, camp.center.0, camp.center.2, 2.7, [245, 185, 48, 255]);
    }

    // ۱۲. ساختمان‌های باستانی (Ancients)
    for anc in &config.map.ancients {
        let loc = anc.model_transform.location;
        let col = if anc.team == TeamId::Dawn {
            [40, 255, 185, 255]
        } else {
            [255, 65, 55, 255]
        };
        draw_circle(&mut pixels, loc.0, loc.2, 8.2, [15, 18, 22, 255]);
        draw_circle(&mut pixels, loc.0, loc.2, 6.6, col);
        draw_circle(&mut pixels, loc.0, loc.2, 2.8, [255, 255, 255, 255]);
    }

    Image::new(
        Extent3d {
            width: MINIMAP_TEX_W,
            height: MINIMAP_TEX_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}


// -----------------------------------------------------------------------------
// سیستم Fog of War (شبیه Dota 2) و شفاف‌سازی آبجکت‌های مسدودکننده دید دوربین
// -----------------------------------------------------------------------------

fn init_fog_of_war(config: &GameConfig, images: &mut Assets<Image>) -> FogOfWarState {
    let hero_team = config.active_hero(0).team;
    let shared = SharedFogOfWar::from_config(config, hero_team);
    let total = FOW_GRID_W * FOW_GRID_H;

    let mut img = Image::new_fill(
        Extent3d {
            width: FOW_GRID_W as u32,
            height: FOW_GRID_H as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[10, 16, 28, 220],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::linear();
    let fog_image = images.add(img);

    FogOfWarState {
        shared,
        alpha_grid: vec![0.86; total],
        fog_image,
    }
}

fn fog_of_war_update_system(
    time: Res<Time>,
    mut fow: ResMut<FogOfWarState>,
    mut images: ResMut<Assets<Image>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    let lerp_rate = (10.0 * dt).clamp(0.05, 1.0);
    let fog_handle = fow.fog_image.clone();
    let Some(mut img) = images.get_mut(&fog_handle) else {
        return;
    };
    let Some(ref mut data) = img.data else {
        return;
    };

    for gz in 0..FOW_GRID_H {
        for gx in 0..FOW_GRID_W {
            let idx = gz * FOW_GRID_W + gx;
            let is_vis = fow.shared.visible_now[idx];
            let is_exp = fow.shared.explored[idx];
            let target_alpha = if is_vis {
                0.0
            } else if is_exp {
                0.55
            } else {
                0.86
            };

            fow.alpha_grid[idx] =
                fow.alpha_grid[idx] + (target_alpha - fow.alpha_grid[idx]) * lerp_rate;
            let shimmer =
                ((gx as f32 * 0.18 + t * 0.9).sin() * (gz as f32 * 0.18 - t * 0.7).cos()) * 0.025;
            let a = if target_alpha == 0.0 && fow.alpha_grid[idx] < 0.04 {
                0.0
            } else {
                (fow.alpha_grid[idx] + shimmer).clamp(0.0, 0.92)
            };

            let p = idx * 4;
            if is_exp {
                data[p] = 12;
                data[p + 1] = 20;
                data[p + 2] = 36;
            } else {
                data[p] = 6;
                data[p + 1] = 10;
                data[p + 2] = 18;
            }
            data[p + 3] = (a * 255.0) as u8;
        }
    }
}

fn segment_intersects_aabb(seg_a: Vec3, seg_b: Vec3, center: Vec3, half: Vec3, rot: Quat) -> bool {
    let inv_rot = rot.inverse();
    let la = inv_rot * (seg_a - center);
    let lb = inv_rot * (seg_b - center);
    let dir = lb - la;

    let mut t_min = 0.0_f32;
    let mut t_max = 1.0_f32;

    for axis in 0..3 {
        let o = la[axis];
        let d = dir[axis];
        let h = half[axis];
        if d.abs() < 1e-5 {
            if o < -h || o > h {
                return false;
            }
        } else {
            let inv_d = 1.0 / d;
            let mut t1 = (-h - o) * inv_d;
            let mut t2 = (h - o) * inv_d;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            if t_min > t_max {
                return false;
            }
        }
    }
    true
}

fn optimize_tree_shadows_system(
    mut commands: Commands,
    mut tree_query: Query<(Entity, &mut TreeVisualMarker)>,
    children_query: Query<&Children>,
    mesh_query: Query<(), With<Mesh3d>>,
) {
    for (tree_ent, mut marker) in &mut tree_query {
        if marker.shadows_optimized {
            continue;
        }
        let mut found_mesh = false;
        if mesh_query.contains(tree_ent) {
            commands.entity(tree_ent).insert(NotShadowCaster);
            found_mesh = true;
        }
        for desc in children_query.iter_descendants(tree_ent) {
            if mesh_query.contains(desc) {
                commands.entity(desc).insert(NotShadowCaster);
                found_mesh = true;
            }
        }
        if found_mesh {
            marker.shadows_optimized = true;
        }
    }
}

fn occlusion_transparency_system(
    mut commands: Commands,
    time: Res<Time>,
    render_state: Res<ClientRenderState>,
    camera_query: Query<&GlobalTransform, With<MobaCamera>>,
    mut occludable_query: Query<(Entity, &GlobalTransform, &mut VisionOccludable)>,
    children_query: Query<&Children>,
    mut mat_mesh_query: Query<(
        &mut MeshMaterial3d<StandardMaterial>,
        Option<&mut OccludedMaterialState>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(cam_gtf) = camera_query.single() else {
        return;
    };
    let cam_pos = cam_gtf.translation();
    let hero_focus = render_state.soldier_pos + Vec3::Y * 1.1;
    let dt = time.delta_secs();
    let fade_speed = (10.0 * dt).clamp(0.05, 1.0);

    for (root_entity, root_gtf, mut occ) in &mut occludable_query {
        let (scale, rot, center) = root_gtf.to_scale_rotation_translation();
        let padded_half = Vec3::new(
            occ.half_extents.x * scale.x.abs() + 1.2,
            occ.half_extents.y * scale.y.abs() + 0.6,
            occ.half_extents.z * scale.z.abs() + 1.2,
        );

        let is_blocking = segment_intersects_aabb(cam_pos, hero_focus, center, padded_half, rot);
        if !is_blocking && !occ.is_faded {
            continue;
        }

        let target_alpha = if is_blocking { 0.22_f32 } else { 1.0_f32 };
        let mut any_still_faded = false;

        let mut process_entity = |ent: Entity| {
            let Ok((mut mat_comp, state_opt)) = mat_mesh_query.get_mut(ent) else {
                return;
            };

            if let Some(mut state) = state_opt {
                let goal = if is_blocking { 0.22 } else { state.original_alpha };
                state.current_alpha = state.current_alpha + (goal - state.current_alpha) * fade_speed;

                if !is_blocking && (state.current_alpha - state.original_alpha).abs() < 0.025 {
                    // بازگردانی Handle اصلی اشتراکی برای حفظ کامل GPU Instancing
                    mat_comp.0 = state.original_handle.clone();
                    commands.entity(ent).remove::<OccludedMaterialState>();
                } else {
                    any_still_faded = true;
                    if let Some(mut mat) = materials.get_mut(&mat_comp.0) {
                        mat.base_color.set_alpha(state.current_alpha);
                        mat.alpha_mode = AlphaMode::Blend;
                    }
                }
            } else if is_blocking {
                if let Some(orig_mat) = materials.get(&mat_comp.0).cloned() {
                    let orig_handle = mat_comp.0.clone();
                    let orig_alpha = orig_mat.base_color.alpha();
                    let mut cloned = orig_mat;
                    let new_alpha = orig_alpha + (target_alpha - orig_alpha) * fade_speed;
                    cloned.base_color.set_alpha(new_alpha);
                    cloned.alpha_mode = AlphaMode::Blend;
                    let new_handle = materials.add(cloned);
                    mat_comp.0 = new_handle;
                    commands.entity(ent).insert(OccludedMaterialState {
                        original_handle: orig_handle,
                        original_alpha: orig_alpha,
                        current_alpha: new_alpha,
                    });
                    any_still_faded = true;
                }
            }
        };

        process_entity(root_entity);
        for desc in children_query.iter_descendants(root_entity) {
            process_entity(desc);
        }

        occ.is_faded = is_blocking || any_still_faded;
    }
}


// -----------------------------------------------------------------------------
// سیستم‌های شیدر آب (WESL)، مه آبشار و انیمیشن آبشارهای درون فایل GLB
// -----------------------------------------------------------------------------

fn bake_water_foam_noise_texture() -> Image {
    let size = 256_usize;
    let mut data = vec![0u8; size * size * 4];

    for y in 0..size {
        for x in 0..size {
            let u = (x as f32) / (size as f32);
            let v = (y as f32) / (size as f32);
            let a = u * std::f32::consts::TAU;
            let b = v * std::f32::consts::TAU;

            let w1 = (a.sin() * b.cos() + (a * 2.0 + 1.1).cos() * (b * 2.0 - 0.7).sin()) * 0.25 + 0.5;
            let w2 = ((a * 3.0 - b * 2.0).sin() * (b * 3.0 + a).cos()) * 0.5 + 0.5;
            let w3 = ((a * 5.0 + b * 4.0).sin() * 0.5 + 0.5).powi(2);

            let r = (w1 * 0.65 + w3 * 0.35).clamp(0.0, 1.0);
            let g = (w2 * 0.65 + w3 * 0.35).clamp(0.0, 1.0);
            let bl = ((r + g) * 0.5).clamp(0.0, 1.0);

            let idx = (y * size + x) * 4;
            data[idx] = (r * 255.0) as u8;
            data[idx + 1] = (g * 255.0) as u8;
            data[idx + 2] = (bl * 255.0) as u8;
            data[idx + 3] = 255;
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: size as u32,
            height: size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    img
}

fn water_shader_update_system(
    time: Res<Time>,
    camera_query: Query<&GlobalTransform, With<MobaCamera>>,
    mut water_materials: ResMut<Assets<WaterSurfaceMaterial>>,
) {
    let t = time.elapsed_secs();
    let cam_pos = camera_query
        .single()
        .map(|gtf| gtf.translation())
        .unwrap_or(Vec3::new(0.0, 23.0, 17.5));
    for (_, mat) in water_materials.iter_mut() {
        mat.uniforms.flow_and_params.z = t;
        mat.uniforms.player_ripple.x = cam_pos.x;
        mat.uniforms.player_ripple.y = cam_pos.y;
        mat.uniforms.player_ripple.z = cam_pos.z;
    }
}

fn waterfall_mist_particle_system(
    mut commands: Commands,
    time: Res<Time>,
    mut render_state: ResMut<ClientRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut emitter_query: Query<&mut WaterfallMistEmitter>,
    mut mist_query: Query<(Entity, &mut Transform, &mut WaterfallMistParticle)>,
    mut cached_assets: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    let (mist_mesh, mist_mat) = cached_assets.get_or_insert_with(|| {
        let m = meshes.add(Sphere::new(0.45));
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.88, 0.97, 1.0, 0.42),
            emissive: LinearRgba::new(0.65, 1.25, 1.55, 0.42),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        (m, mat)
    });

    let dt = time.delta_secs();

    for mut emitter in &mut emitter_query {
        emitter.timer.tick(time.delta());
        if emitter.timer.just_finished() {
            for _ in 0..2 {
                let rx = render_state.next_fx_rand() - 0.5;
                let rz = render_state.next_fx_rand() - 0.5;
                let ry = render_state.next_fx_rand();
                let spawn_p = emitter.base_pos + Vec3::new(rx * emitter.width * 0.85, 0.2, rz * 1.6);
                let vel = Vec3::new(rx * 1.8, 1.4 + ry * 1.4, rz * 1.8);
                commands.spawn((
                    Mesh3d(mist_mesh.clone()),
                    MeshMaterial3d(mist_mat.clone()),
                    Transform::from_translation(spawn_p).with_scale(Vec3::splat(0.75)),
                    WaterfallMistParticle {
                        vel,
                        timer: Timer::from_seconds(1.45, TimerMode::Once),
                        start_scale: 0.75,
                        end_scale: 2.8,
                    },
                ));
            }
        }
    }

    for (entity, mut tf, mut p) in &mut mist_query {
        p.timer.tick(time.delta());
        tf.translation += p.vel * dt;
        p.vel *= (1.0 - 0.9 * dt).max(0.1);
        let frac = p.timer.fraction();
        let s = p.start_scale.lerp(p.end_scale, frac) * (1.0 - (frac - 0.65).max(0.0) * 2.4).max(0.05);
        tf.scale = Vec3::splat(s);
        if p.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

fn ensure_repeat_sampler(img: &mut Image) {
    let desc = match &mut img.sampler {
        ImageSampler::Descriptor(d) => d,
        ImageSampler::Default => {
            img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
            match &mut img.sampler {
                ImageSampler::Descriptor(d) => d,
                _ => return,
            }
        }
    };
    desc.address_mode_u = ImageAddressMode::Repeat;
    desc.address_mode_v = ImageAddressMode::Repeat;
}

/// انیمیت خودکار آبشارها و آب‌های عمیق درون مدل‌های `.glb` با حفظ کامل متریال و تکسچر اصلی مدل
fn animate_glb_waterfall_materials_system(
    mut commands: Commands,
    time: Res<Time>,
    mut candidate_query: Query<
        (Entity, &mut MeshMaterial3d<StandardMaterial>, Option<&Name>, Option<&GltfMaterialName>),
        Without<GlbAnimatedWaterTag>,
    >,
    tagged_query: Query<(&MeshMaterial3d<StandardMaterial>, &GlbAnimatedWaterTag)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut logged_names: Local<HashSet<String>>,
) {
    for (ent, mut mat_comp, name_opt, gltf_mat_opt) in &mut candidate_query {
        let debug_key = format!(
            "node={:?} | mat={:?}",
            name_opt.map(|n| n.as_str()),
            gltf_mat_opt.map(|m| m.0.as_str())
        );
        if logged_names.insert(debug_key.clone()) {
            println!("مش/متریال GLB: {}", debug_key);
        }

        let name_str = name_opt.map(|n| n.as_str().to_lowercase()).unwrap_or_default();
        let mat_str = gltf_mat_opt.map(|m| m.0.to_lowercase()).unwrap_or_default();

        let is_waterfall = name_str.contains("waterfall")
            || name_str.contains("cascade")
            || name_str.contains("abshar")
            || mat_str.contains("waterfall")
            || mat_str.contains("cascade")
            || mat_str.contains("abshar");

        let is_deep_water = !is_waterfall
            && (name_str.contains("water")
                || name_str.contains("river")
                || name_str.contains("pond")
                || name_str.contains("pool")
                || name_str.contains("lake")
                || mat_str.contains("water")
                || mat_str.contains("river")
                || mat_str.contains("pond")
                || mat_str.contains("pool")
                || mat_str.contains("lake"));

        if is_waterfall || is_deep_water {
            if let Some(cloned_mat) = materials.get(&mat_comp.0).cloned() {
                mat_comp.0 = materials.add(cloned_mat);
            }
            if let Some(mat) = materials.get(&mat_comp.0) {
                if let Some(ref tex_h) = mat.base_color_texture {
                    if let Some(mut img) = images.get_mut(tex_h) {
                        ensure_repeat_sampler(&mut img);
                    }
                }
                if let Some(ref norm_h) = mat.normal_map_texture {
                    if let Some(mut img) = images.get_mut(norm_h) {
                        ensure_repeat_sampler(&mut img);
                    }
                }
            }
            commands
                .entity(ent)
                .insert(GlbAnimatedWaterTag { is_waterfall });
        }
    }

    let t = time.elapsed_secs();
    for (mat_comp, tag) in &tagged_query {
        if let Some(mut mat) = materials.get_mut(&mat_comp.0) {
            if tag.is_waterfall {
                let offset_y = -(t * 1.25).fract();
                mat.uv_transform = Affine2::from_translation(Vec2::new(0.0, offset_y));
            } else {
                let offset_x = (t * 0.14).fract();
                let offset_y = (t * 0.10).fract();
                mat.uv_transform = Affine2::from_translation(Vec2::new(offset_x, offset_y));
            }
        }
    }
}


// -----------------------------------------------------------------------------
// رابط کاربری نمایش تمام پارامترهای هیرو و اسلحه (Server-Authoritative HUD)
// -----------------------------------------------------------------------------

const XP_RING_SEGMENTS: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeroStatKind {
    Dmg,
    Rate,
    Range,
    Armor,
    Speed,
    Vision,
    Ammo,
    Grenade,
}

#[derive(Component)]
struct HeroStatBonusText(HeroStatKind);

fn spawn_stat_chip(
    row: &mut ChildSpawnerCommands,
    icon_label: &str,
    icon_color: Color,
    marker: impl Component,
    kind: HeroStatKind,
) {
    row.spawn((
        Node {
            padding: UiRect::axes(px(7.0), px(4.0)),
            column_gap: px(4.0),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1.0)),
            border_radius: BorderRadius::all(px(5.0)),
            ..default()
        },
        BorderColor::all(Color::srgba(0.28, 0.34, 0.42, 0.55)),
        BackgroundColor(Color::srgba(0.08, 0.11, 0.16, 0.85)),
    ))
    .with_children(|chip| {
        chip.spawn((
            Node {
                width: px(16.0),
                height: px(16.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(icon_color),
        ))
        .with_children(|ic| {
            ic.spawn((
                Text::new(icon_label),
                TextFont {
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgb(0.05, 0.06, 0.08)),
            ));
        });

        chip.spawn((
            marker,
            Text::new("--"),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.95, 0.99)),
        ));

        chip.spawn((
            HeroStatBonusText(kind),
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(11.0),
                ..default()
            },
            TextColor(Color::srgb(0.28, 0.95, 0.52)),
        ));
    });
}

fn spawn_hero_hud_ui(commands: &mut Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(16.0),
                bottom: px(16.0),
                padding: UiRect::all(px(12.0)),
                flex_direction: FlexDirection::Row,
                column_gap: px(14.0),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(10.0)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.42, 0.58, 0.72, 0.78)),
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.90)),
        ))
        .with_children(|root| {
            // ستون چپ: دایره توخالی نمایش سطح (Level) + حلقه XP دور دایره
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4.0),
                    ..default()
                },
            ))
            .with_children(|lvl_col| {
                let ring_box = 86.0_f32;
                let center_xy = ring_box * 0.5;
                let arc_r = 37.0_f32;
                let seg_size = 6.5_f32;

                lvl_col
                    .spawn(Node {
                        width: px(ring_box),
                        height: px(ring_box),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|ring| {
                        // سگمنت‌های دایره‌ای XP دور دایره سطح
                        for i in 0..XP_RING_SEGMENTS {
                            let angle = -std::f32::consts::FRAC_PI_2
                                + (i as f32) * (std::f32::consts::TAU / XP_RING_SEGMENTS as f32);
                            let sx = center_xy + angle.cos() * arc_r - seg_size * 0.5;
                            let sy = center_xy + angle.sin() * arc_r - seg_size * 0.5;
                            ring.spawn((
                                HeroHudXpSegment { index: i },
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(sx),
                                    top: px(sy),
                                    width: px(seg_size),
                                    height: px(seg_size),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.20, 0.24, 0.30, 0.65)),
                            ));
                        }

                        // دایره توخالی مرکزی با عدد سطح
                        ring.spawn((
                            Node {
                                width: px(56.0),
                                height: px(56.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(px(2.5)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.95, 0.78, 0.24)),
                            BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.92)),
                        ))
                        .with_children(|inner| {
                            inner.spawn((
                                HeroHudLevelText,
                                Text::new("1"),
                                TextFont {
                                    font_size: FontSize::Px(22.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.88, 0.36)),
                            ));
                        });
                    });

                lvl_col.spawn((
                    HeroHudXpLabelText,
                    Text::new("XP 0/240"),
                    TextFont {
                        font_size: FontSize::Px(10.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.78, 0.45)),
                ));
            });

            // ستون راست: نام هیرو، گلد، KDA، نوار سلامتی و کارت‌های آمار آیکون‌دار
            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(7.0),
                width: px(425.0),
                ..default()
            })
            .with_children(|info| {
                // ردیف ۱: نام هیرو + سکه گلد + آمار KDA/LH/DN
                info.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|top| {
                    top.spawn((
                        HeroHudHeaderText,
                        Text::new("Hero"),
                        TextFont {
                            font_size: FontSize::Px(14.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.98, 1.0)),
                    ));

                    top.spawn((
                        Node {
                            padding: UiRect::axes(px(8.0), px(3.0)),
                            column_gap: px(5.0),
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::all(px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.28, 0.22, 0.06, 0.85)),
                    ))
                    .with_children(|g| {
                        g.spawn((
                            Node {
                                width: px(11.0),
                                height: px(11.0),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgb(1.0, 0.82, 0.16)),
                        ));
                        g.spawn((
                            HeroHudGoldText,
                            Text::new("600"),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.88, 0.35)),
                        ));
                    });
                });

                // ردیف ۲: نوار سلامتی (HP) با متن دقیق داخل نوار
                info.spawn((
                    Node {
                        width: percent(100),
                        height: px(18.0),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(4.0)),
                        overflow: Overflow::clip(),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.22, 0.28, 0.35, 0.9)),
                    BackgroundColor(Color::srgb(0.10, 0.12, 0.15)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        HeroHpBarFill,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            top: px(0.0),
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.16, 0.78, 0.38)),
                    ));
                    bar.spawn((
                        HeroHpBarText,
                        Text::new("650 / 650 (+2.5/s)"),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                // ردیف ۲-ب: نوار آبی Stamina زیر نوار سلامتی
                info.spawn((
                    Node {
                        width: percent(100),
                        height: px(14.0),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(4.0)),
                        overflow: Overflow::clip(),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.18, 0.34, 0.52, 0.9)),
                    BackgroundColor(Color::srgb(0.07, 0.10, 0.16)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        HeroStaminaBarFill,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            top: px(0.0),
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.58, 0.96)),
                    ));
                    bar.spawn((
                        HeroStaminaBarText,
                        Text::new("STAMINA 100 / 100 (+15.0/s)"),
                        TextFont {
                            font_size: FontSize::Px(9.5),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                // ردیف ۳: آمار KDA و مهمات اسلحه
                info.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|mid| {
                    mid.spawn((
                        HeroHudKdaText,
                        Text::new("K 0 / D 0 / A 0  |  LH 0  DN 0"),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.76, 0.84, 0.92)),
                    ));

                    mid.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(6.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|chips| {
                        spawn_stat_chip(chips, "A", Color::srgb(0.98, 0.80, 0.22), HeroStatAmmoText, HeroStatKind::Ammo);
                        spawn_stat_chip(chips, "G", Color::srgb(1.0, 0.45, 0.14), HeroStatGrenadeText, HeroStatKind::Grenade);
                    });
                });

                // ردیف ۴: ۶ کارت آمار با آیکون رنگی مفهومی + نمایش +-number باف/دیباف آیتم‌ها
                info.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(6.0),
                    row_gap: px(6.0),
                    ..default()
                })
                .with_children(|grid| {
                    spawn_stat_chip(grid, "D", Color::srgb(0.95, 0.30, 0.28), HeroStatDmgText, HeroStatKind::Dmg);
                    spawn_stat_chip(grid, "F", Color::srgb(0.98, 0.58, 0.18), HeroStatRateText, HeroStatKind::Rate);
                    spawn_stat_chip(grid, "R", Color::srgb(0.24, 0.86, 0.94), HeroStatRangeText, HeroStatKind::Range);
                    spawn_stat_chip(grid, "S", Color::srgb(0.36, 0.64, 0.98), HeroStatArmorText, HeroStatKind::Armor);
                    spawn_stat_chip(grid, "M", Color::srgb(0.26, 0.88, 0.52), HeroStatSpeedText, HeroStatKind::Speed);
                    spawn_stat_chip(grid, "V", Color::srgb(0.76, 0.48, 0.98), HeroStatVisionText, HeroStatKind::Vision);
                });
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[derive(SystemParam)]
struct HeroHudExtraStatQueries<'w, 's> {
    dmg_q: Query<'w, 's, &'static mut Text, (With<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    rate_q: Query<'w, 's, &'static mut Text, (With<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    range_q: Query<'w, 's, &'static mut Text, (With<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    armor_q: Query<'w, 's, &'static mut Text, (With<HeroStatArmorText>, Without<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    speed_q: Query<'w, 's, &'static mut Text, (With<HeroStatSpeedText>, Without<HeroStatArmorText>, Without<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    vision_q: Query<'w, 's, &'static mut Text, (With<HeroStatVisionText>, Without<HeroStatSpeedText>, Without<HeroStatArmorText>, Without<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    ammo_q: Query<'w, 's, &'static mut Text, (With<HeroStatAmmoText>, Without<HeroStatGrenadeText>, Without<HeroStatVisionText>, Without<HeroStatSpeedText>, Without<HeroStatArmorText>, Without<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    grenade_q: Query<'w, 's, &'static mut Text, (With<HeroStatGrenadeText>, Without<HeroStatAmmoText>, Without<HeroStatVisionText>, Without<HeroStatSpeedText>, Without<HeroStatArmorText>, Without<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
    bonus_q: Query<'w, 's, (&'static HeroStatBonusText, &'static mut Text, &'static mut TextColor), (Without<HeroStatGrenadeText>, Without<HeroStatAmmoText>, Without<HeroStatVisionText>, Without<HeroStatSpeedText>, Without<HeroStatArmorText>, Without<HeroStatRangeText>, Without<HeroStatRateText>, Without<HeroStatDmgText>, Without<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
}

fn update_hero_hud_system(
    config: Res<GameConfig>,
    items_cfg: Res<ItemsConfig>,
    hero_state: Res<LatestHeroSnapshot>,
    inspect_p: HudInspectParams,
    mut extra_stats: HeroHudExtraStatQueries,
    mut hp_bar_query: Query<(&mut Node, &mut BackgroundColor), With<HeroHpBarFill>>,
    mut xp_seg_query: Query<(&HeroHudXpSegment, &mut BackgroundColor), Without<HeroHpBarFill>>,
    mut header_q: Query<&mut Text, With<HeroHudHeaderText>>,
    mut gold_q: Query<&mut Text, (With<HeroHudGoldText>, Without<HeroHudHeaderText>)>,
    mut kda_q: Query<&mut Text, (With<HeroHudKdaText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>)>,
    mut lvl_q: Query<&mut Text, (With<HeroHudLevelText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>)>,
    mut xp_lbl_q: Query<&mut Text, (With<HeroHudXpLabelText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>)>,
    mut hp_txt_q: Query<&mut Text, (With<HeroHpBarText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>, Without<HeroHudXpLabelText>)>,
) {
    let h = &hero_state.0;
    let clear_bonuses = |extra: &mut HeroHudExtraStatQueries| {
        for (_, mut bt, _) in &mut extra.bonus_q {
            **bt = String::new();
        }
    };
    let set_unit_extra_stats = |extra: &mut HeroHudExtraStatQueries,
                                lvl_q: &mut Query<&mut Text, (With<HeroHudLevelText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>)>,
                                xp_q: &mut Query<&mut Text, (With<HeroHudXpLabelText>, Without<HeroHudHeaderText>, Without<HeroHudGoldText>, Without<HeroHudKdaText>, Without<HeroHudLevelText>)>,
                                lvl_str: &str,
                                xp_str: &str,
                                dmg: f32,
                                rate: f32,
                                rng: f32,
                                arm: f32,
                                spd: f32,
                                vis: f32,
                                ammo: &str| {
        if let Ok(mut t) = lvl_q.single_mut() { **t = lvl_str.to_string(); }
        if let Ok(mut t) = xp_q.single_mut() { **t = xp_str.to_string(); }
        if let Ok(mut t) = extra.dmg_q.single_mut() { **t = format!("DMG {:.0}", dmg); }
        if let Ok(mut t) = extra.rate_q.single_mut() { **t = format!("RATE {:.2}s", rate); }
        if let Ok(mut t) = extra.range_q.single_mut() { **t = format!("RNG {:.0}m", rng); }
        if let Ok(mut t) = extra.armor_q.single_mut() { **t = format!("ARM {:.1}", arm); }
        if let Ok(mut t) = extra.speed_q.single_mut() { **t = format!("SPD {:.1}", spd); }
        if let Ok(mut t) = extra.vision_q.single_mut() { **t = format!("VIS {:.0}m", vis); }
        if let Ok(mut t) = extra.ammo_q.single_mut() { **t = ammo.to_string(); }
        if let Ok(mut t) = extra.grenade_q.single_mut() { **t = "N/A".to_string(); }
    };

    let show_own_courier = inspect_p.courier_ctrl.selected || *inspect_p.inspected == InspectedUnit::OwnCourier;
    if show_own_courier {
        let my_hero_cfg = config.heroes.iter().find(|hd| hd.id == h.hero_id).unwrap_or(config.active_hero(0));
        let pct = if h.courier_is_dead || h.courier_max_hp <= 0.0 {
            0.0
        } else {
            ((h.courier_hp / h.courier_max_hp) * 100.0).clamp(0.0, 100.0)
        };
        if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
            bar_node.width = percent(pct);
            *bar_col = Color::srgb(0.28, 0.86, 1.0).into();
        }
        if let Ok(mut t) = header_q.single_mut() {
            **t = format!("DRONE COURIER [{:?}]", h.team);
        }
        if let Ok(mut t) = hp_txt_q.single_mut() {
            **t = if h.courier_is_dead {
                format!("DEAD - RESPAWN IN {:.0}s", h.courier_respawn_remaining.ceil())
            } else {
                format!("{:.0} / {:.0} [{:?}]", h.courier_hp, h.courier_max_hp, h.courier_order)
            };
        }
        if let Ok(mut t) = kda_q.single_mut() {
            **t = format!("STASH {}/6  |  RMB: Fly  |  [F3] Deliver  |  [F1] Hero", h.courier_inventory.len());
        }
        clear_bonuses(&mut extra_stats);
        set_unit_extra_stats(
            &mut extra_stats,
            &mut lvl_q,
            &mut xp_lbl_q,
            "DRN",
            "FLYING",
            0.0,
            0.0,
            0.0,
            0.0,
            my_hero_cfg.courier.move_speed,
            my_hero_cfg.courier.vision_range,
            "N/A",
        );
        return;
    }

    match *inspect_p.inspected {
        InspectedUnit::EnemyCourier(owner_slot) => {
            if let Some(c) = inspect_p.latest_couriers.0.iter().find(|c| c.owner_slot == owner_slot) {
                let pct = if c.is_dead || c.max_hp <= 0.0 { 0.0 } else { ((c.hp / c.max_hp) * 100.0).clamp(0.0, 100.0) };
                if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
                    bar_node.width = percent(pct);
                    *bar_col = Color::srgb(0.95, 0.30, 0.30).into();
                }
                if let Ok(mut t) = header_q.single_mut() { **t = format!("ENEMY COURIER [{:?}]", c.team); }
                if let Ok(mut t) = hp_txt_q.single_mut() { **t = format!("{:.0} / {:.0}", c.hp, c.max_hp); }
                if let Ok(mut t) = kda_q.single_mut() { **t = "Enemy Flying Courier".to_string(); }
                clear_bonuses(&mut extra_stats);
                set_unit_extra_stats(&mut extra_stats, &mut lvl_q, &mut xp_lbl_q, "DRN", "ENEMY", 0.0, 0.0, 0.0, 0.0, 14.0, 6.0, "N/A");
                return;
            }
        }
        InspectedUnit::RemoteHero(slot) => {
            if let Some(rp) = inspect_p.latest_remotes.0.iter().find(|r| r.slot_index == slot) {
                let pct = if rp.is_dead || rp.max_hp <= 0.0 { 0.0 } else { ((rp.hp / rp.max_hp) * 100.0).clamp(0.0, 100.0) };
                if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
                    bar_node.width = percent(pct);
                    *bar_col = if rp.team == h.team { Color::srgb(0.16, 0.78, 0.38).into() } else { Color::srgb(0.95, 0.30, 0.30).into() };
                }
                if let Ok(mut t) = header_q.single_mut() { **t = format!("{} [{:?}]", rp.display_name, rp.team); }
                if let Ok(mut t) = hp_txt_q.single_mut() { **t = format!("{:.0} / {:.0}", rp.hp, rp.max_hp); }
                if let Ok(mut t) = kda_q.single_mut() { **t = format!("Weapon: {:?}", rp.weapon_kind); }
                clear_bonuses(&mut extra_stats);
                let vis_r = config.heroes.iter().find(|hd| hd.id == rp.hero_id).map(|hd| hd.vision_range).unwrap_or(36.0);
                set_unit_extra_stats(
                    &mut extra_stats,
                    &mut lvl_q,
                    &mut xp_lbl_q,
                    &format!("{}", rp.level),
                    "HERO",
                    rp.weapon_damage,
                    rp.fire_rate_secs,
                    rp.weapon_max_range,
                    rp.armor,
                    rp.move_speed,
                    vis_r,
                    &format!("{}/{}", rp.ammo_in_mag, rp.magazine_size),
                );
                return;
            }
        }
        InspectedUnit::Creep(cid) => {
            if let Some(cr) = inspect_p.latest_creeps.0.iter().find(|c| c.id == cid) {
                let pct = if cr.is_dead || cr.max_hp <= 0.0 { 0.0 } else { ((cr.hp / cr.max_hp) * 100.0).clamp(0.0, 100.0) };
                if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
                    bar_node.width = percent(pct);
                    *bar_col = if cr.team == h.team { Color::srgb(0.16, 0.78, 0.38).into() } else { Color::srgb(0.95, 0.30, 0.30).into() };
                }
                if let Ok(mut t) = header_q.single_mut() { **t = format!("CREEP [{:?}]", cr.team); }
                if let Ok(mut t) = hp_txt_q.single_mut() { **t = format!("{:.0} / {:.0}", cr.hp, cr.max_hp); }
                if let Ok(mut t) = kda_q.single_mut() { **t = cr.creep_type_id.clone().unwrap_or_else(|| "Lane Creep".to_string()); }
                clear_bonuses(&mut extra_stats);
                set_unit_extra_stats(
                    &mut extra_stats,
                    &mut lvl_q,
                    &mut xp_lbl_q,
                    "CRP",
                    "UNIT",
                    cr.damage,
                    cr.fire_rate_secs,
                    cr.attack_range,
                    cr.armor,
                    cr.move_speed,
                    config.creeps.vision_range,
                    "INF",
                );
                return;
            }
        }
        InspectedUnit::Turret(t_idx) => {
            if let (Some(ts), Some(td)) = (inspect_p.latest_turrets.0.get(t_idx), config.map.turrets.get(t_idx)) {
                let pct = if ts.is_dead || ts.max_hp <= 0.0 { 0.0 } else { ((ts.hp / ts.max_hp) * 100.0).clamp(0.0, 100.0) };
                if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
                    bar_node.width = percent(pct);
                    *bar_col = if ts.team == h.team { Color::srgb(0.16, 0.78, 0.38).into() } else { Color::srgb(0.95, 0.30, 0.30).into() };
                }
                if let Ok(mut t) = header_q.single_mut() { **t = format!("TIER {} TURRET [{:?}]", td.tier, ts.team); }
                if let Ok(mut t) = hp_txt_q.single_mut() {
                    **t = if ts.is_invulnerable { format!("{:.0} / {:.0} [INVULNERABLE]", ts.hp, ts.max_hp) } else { format!("{:.0} / {:.0}", ts.hp, ts.max_hp) };
                }
                if let Ok(mut t) = kda_q.single_mut() { **t = format!("Lane: {:?}  |  Tower Structure", td.lane); }
                clear_bonuses(&mut extra_stats);
                set_unit_extra_stats(&mut extra_stats, &mut lvl_q, &mut xp_lbl_q, &format!("T{}", td.tier), "TOWER", td.bullet_damage, td.fire_rate_secs, td.attack_range, td.armor, 0.0, td.vision_range, "INF");
                return;
            }
        }
        InspectedUnit::Barracks(b_idx) => {
            if let (Some(bs), Some(bd)) = (inspect_p.latest_barracks.0.get(b_idx), config.map.barracks.get(b_idx)) {
                let pct = if bs.is_dead || bs.max_hp <= 0.0 { 0.0 } else { ((bs.hp / bs.max_hp) * 100.0).clamp(0.0, 100.0) };
                if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
                    bar_node.width = percent(pct);
                    *bar_col = if bs.team == h.team { Color::srgb(0.16, 0.78, 0.38).into() } else { Color::srgb(0.95, 0.30, 0.30).into() };
                }
                if let Ok(mut t) = header_q.single_mut() { **t = format!("{:?} BARRACKS [{:?}]", bd.kind, bs.team); }
                if let Ok(mut t) = hp_txt_q.single_mut() {
                    **t = if bs.is_invulnerable { format!("{:.0} / {:.0} [INVULNERABLE]", bs.hp, bs.max_hp) } else { format!("{:.0} / {:.0}", bs.hp, bs.max_hp) };
                }
                if let Ok(mut t) = kda_q.single_mut() { **t = format!("Lane: {:?}  |  Barracks Structure", bd.lane); }
                clear_bonuses(&mut extra_stats);
                set_unit_extra_stats(&mut extra_stats, &mut lvl_q, &mut xp_lbl_q, "BRK", "BLDG", 0.0, 0.0, 0.0, bd.armor, 0.0, config.map.base_vision_range, "N/A");
                return;
            }
        }
        InspectedUnit::Ancient(a_idx) => {
            if let (Some(ans), Some(ad)) = (inspect_p.latest_ancients.0.get(a_idx), config.map.ancients.get(a_idx)) {
                let pct = if ans.is_dead || ans.max_hp <= 0.0 { 0.0 } else { ((ans.hp / ans.max_hp) * 100.0).clamp(0.0, 100.0) };
                if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
                    bar_node.width = percent(pct);
                    *bar_col = if ans.team == h.team { Color::srgb(0.16, 0.78, 0.38).into() } else { Color::srgb(0.95, 0.30, 0.30).into() };
                }
                if let Ok(mut t) = header_q.single_mut() { **t = format!("ANCIENT CORE [{:?}]", ans.team); }
                if let Ok(mut t) = hp_txt_q.single_mut() {
                    **t = if ans.is_invulnerable { format!("{:.0} / {:.0} [INVULNERABLE]", ans.hp, ans.max_hp) } else { format!("{:.0} / {:.0}", ans.hp, ans.max_hp) };
                }
                if let Ok(mut t) = kda_q.single_mut() { **t = "Main Base Objective".to_string(); }
                clear_bonuses(&mut extra_stats);
                set_unit_extra_stats(&mut extra_stats, &mut lvl_q, &mut xp_lbl_q, "ANC", "CORE", 0.0, 0.0, 0.0, ad.armor, 0.0, config.map.base_vision_range, "N/A");
                return;
            }
        }
        InspectedUnit::OwnHero | InspectedUnit::OwnCourier => {}
    }

    let hp_pct = ((h.hp / h.max_hp.max(1.0)) * 100.0).clamp(0.0, 100.0);

    if let Ok((mut bar_node, mut bar_col)) = hp_bar_query.single_mut() {
        bar_node.width = percent(hp_pct);
        *bar_col = if hp_pct > 50.0 {
            Color::srgb(0.16, 0.78, 0.38).into()
        } else if hp_pct > 25.0 {
            Color::srgb(0.88, 0.65, 0.15).into()
        } else {
            Color::srgb(0.88, 0.20, 0.20).into()
        };
    }

    let xp_frac = if h.level >= 25 || h.xp_for_next_level == 0 {
        1.0
    } else {
        (h.xp_into_level as f32 / h.xp_for_next_level as f32).clamp(0.0, 1.0)
    };
    let lit_count = (xp_frac * XP_RING_SEGMENTS as f32).round() as usize;
    let lit_col = Color::srgb(1.0, 0.84, 0.24);
    let dim_col = Color::srgba(0.20, 0.24, 0.30, 0.65);
    for (seg, mut bg) in &mut xp_seg_query {
        *bg = if seg.index < lit_count {
            lit_col.into()
        } else {
            dim_col.into()
        };
    }

    if let Ok(mut t) = header_q.single_mut() {
        **t = format!("{} [{:?}]", h.display_name, h.team);
    }
    if let Ok(mut t) = gold_q.single_mut() {
        **t = format!("{}", h.gold);
    }
    if let Ok(mut t) = kda_q.single_mut() {
        **t = format!("K {} / D {} / A {}   |   LH {}  DN {}", h.kills, h.deaths, h.assists, h.last_hits, h.denies);
    }
    if let Ok(mut t) = lvl_q.single_mut() {
        **t = format!("{}", h.level);
    }
    if let Ok(mut t) = xp_lbl_q.single_mut() {
        **t = if h.level >= 25 {
            "MAX LVL".to_string()
        } else {
            format!("XP {}/{}", h.xp_into_level, h.xp_for_next_level)
        };
    }
    if let Ok(mut t) = hp_txt_q.single_mut() {
        if h.respawn_remaining > 0.0 {
            **t = format!("DEAD - RESPAWN IN {}s", h.respawn_remaining.ceil() as u32);
        } else if h.in_fountain {
            **t = format!("{:.0} / {:.0} (+{:.1}/s) [FOUNTAIN]", h.hp, h.max_hp, h.hp_regen_per_sec);
        } else {
            **t = format!("{:.0} / {:.0} (+{:.1}/s)", h.hp, h.max_hp, h.hp_regen_per_sec);
        }
    }
    let mods = items_cfg.sum_inventory_modifiers(&h.inventory);
    let is_pistol = h.active_weapon == ActiveWeaponSlot::Pistol;
    let item_dmg = if is_pistol { mods.pistol_damage_bonus } else { mods.primary_damage_bonus };
    let item_rate_pct = if is_pistol { mods.pistol_fire_rate_pct } else { mods.primary_fire_rate_pct };
    let item_rng = if is_pistol { mods.pistol_range_bonus } else { mods.primary_range_bonus };
    let item_arm = mods.armor_bonus;
    let item_spd = config.soldier.run_speed * mods.move_speed_pct;
    let item_mag = if is_pistol { mods.pistol_magazine_bonus } else { mods.primary_magazine_bonus };

    let base_dmg = h.weapon_damage - item_dmg;
    let base_rng = h.weapon_max_range - item_rng;
    let base_arm = h.armor - item_arm;
    let base_spd = h.move_speed - item_spd;
    let base_mag = ((h.magazine_size as i32) - item_mag).max(1);

    if let Ok(mut t) = extra_stats.dmg_q.single_mut() {
        **t = format!("DMG {:.0}", base_dmg);
    }
    if let Ok(mut t) = extra_stats.rate_q.single_mut() {
        **t = format!("Rate {:.2}s | Rld {:.2}s", h.fire_rate_secs, h.reload_duration_secs);
    }
    if let Ok(mut t) = extra_stats.range_q.single_mut() {
        **t = format!("Rng {:.0}m", base_rng);
    }
    if let Ok(mut t) = extra_stats.armor_q.single_mut() {
        **t = format!("Arm {:.1}", base_arm);
    }
    if let Ok(mut t) = extra_stats.speed_q.single_mut() {
        **t = format!("Spd {:.1}", base_spd);
    }
    if let Ok(mut t) = extra_stats.vision_q.single_mut() {
        **t = format!("Vis {:.0}m", h.vision_range);
    }
    if let Ok(mut t) = extra_stats.ammo_q.single_mut() {
        let tag = if is_pistol { "PISTOL" } else { "RIFLE" };
        **t = format!("[E] {} {}/{}", tag, h.ammo_in_mag, base_mag);
    }
    if let Ok(mut t) = extra_stats.grenade_q.single_mut() {
        if h.grenade_charges < h.grenade_max_charges && h.grenade_recharge_remaining > 0.0 {
            **t = format!(
                "[F] GRN {}/{} ({}s)",
                h.grenade_charges,
                h.grenade_max_charges,
                h.grenade_recharge_remaining.ceil() as u32
            );
        } else {
            **t = format!("[F] GRN {}/{}", h.grenade_charges, h.grenade_max_charges);
        }
    }

    let green = Color::srgb(0.26, 0.96, 0.50);
    let red = Color::srgb(0.98, 0.34, 0.34);
    for (bkind, mut bt, mut bc) in &mut extra_stats.bonus_q {
        match bkind.0 {
            HeroStatKind::Dmg => {
                if item_dmg > 0.01 {
                    **bt = format!("+{:.0}", item_dmg);
                    *bc = TextColor(green);
                } else if item_dmg < -0.01 {
                    **bt = format!("{:.0}", item_dmg);
                    *bc = TextColor(red);
                } else {
                    bt.clear();
                }
            }
            HeroStatKind::Rate => {
                let pct = item_rate_pct * 100.0;
                if pct > 0.1 {
                    **bt = format!("+{:.0}%", pct);
                    *bc = TextColor(green);
                } else if pct < -0.1 {
                    **bt = format!("{:.0}%", pct);
                    *bc = TextColor(red);
                } else {
                    bt.clear();
                }
            }
            HeroStatKind::Range => {
                if item_rng > 0.05 {
                    **bt = format!("+{:.0}m", item_rng);
                    *bc = TextColor(green);
                } else if item_rng < -0.05 {
                    **bt = format!("{:.0}m", item_rng);
                    *bc = TextColor(red);
                } else {
                    bt.clear();
                }
            }
            HeroStatKind::Armor => {
                if item_arm > 0.05 {
                    **bt = format!("+{:.1}", item_arm);
                    *bc = TextColor(green);
                } else if item_arm < -0.05 {
                    **bt = format!("{:.1}", item_arm);
                    *bc = TextColor(red);
                } else {
                    bt.clear();
                }
            }
            HeroStatKind::Speed => {
                if item_spd > 0.05 {
                    **bt = format!("+{:.1}", item_spd);
                    *bc = TextColor(green);
                } else if item_spd < -0.05 {
                    **bt = format!("{:.1}", item_spd);
                    *bc = TextColor(red);
                } else {
                    bt.clear();
                }
            }
            HeroStatKind::Ammo => {
                if item_mag > 0 {
                    **bt = format!("+{}", item_mag);
                    *bc = TextColor(green);
                } else if item_mag < 0 {
                    **bt = format!("{}", item_mag);
                    *bc = TextColor(red);
                } else {
                    bt.clear();
                }
            }
            HeroStatKind::Vision | HeroStatKind::Grenade => {
                bt.clear();
            }
        }
    }
}

fn level_up_vfx_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut LevelUpVfx)>,
) {
    for (ent, mut tf, mut vfx) in &mut q {
        vfx.timer.tick(time.delta());
        let frac = vfx.timer.fraction();
        if vfx.is_ring {
            let s = 1.0 + frac * 2.2;
            tf.scale = Vec3::new(s, (1.0 - frac).max(0.05), s);
        } else {
            let s = (1.0 - frac).max(0.05);
            tf.scale = Vec3::new(s, 1.0 + frac * 0.4, s);
        }
        if vfx.timer.is_finished() {
            commands.entity(ent).despawn();
        }
    }
}


// -----------------------------------------------------------------------------
// سیستم ۴ فازی کلاینت: اتصال -> انتخاب هیرو (۳۰ ثانیه + GLB Idle) -> صفحه نبرد -> بازی
// -----------------------------------------------------------------------------

#[derive(Resource)]
struct ClientMatchFlow {
    your_slot: Option<usize>,
    snapshot: Option<MatchStateSnapshot>,
    preview_hero_index: usize,
    spawned_preview_hero_index: Option<usize>,
    map_loaded_reported: bool,
    prev_phase: Option<MatchPhase>,
}

impl ClientMatchFlow {
    fn current_phase(&self) -> MatchPhase {
        self.snapshot
            .as_ref()
            .map(|s| s.phase)
            .unwrap_or(MatchPhase::WaitingForPlayers)
    }

    fn is_in_game(&self) -> bool {
        self.current_phase() == MatchPhase::InGame
    }
}

#[derive(Component)]
struct MatchOverlayRoot;

#[derive(Component)]
struct MatchPhaseHeaderText;

#[derive(Component)]
struct MatchStatusBodyText;

#[derive(Component)]
struct HeroPickRowRoot;

#[derive(Component)]
struct HeroPickButton {
    hero_index: usize,
}

#[derive(Component)]
struct HeroLockInButton;

#[derive(Component)]
struct HeroPreviewRoot;

#[derive(Component)]
struct HeroPreviewAnimStarted;

#[derive(Component)]
struct RemotePlayerVisual {
    slot_index: usize,
    last_lower: String,
    last_upper: String,
}

#[derive(Component)]
struct RemoteLowerAnimPlayer {
    slot_index: usize,
}

#[derive(Component)]
struct RemoteUpperAnimPlayer {
    slot_index: usize,
}

#[derive(Component)]
struct RemoteWeaponAttached;

#[derive(Resource, Default)]
struct LatestRemotePlayers(Vec<RemotePlayerSnapshot>);

#[derive(Resource, Default)]
struct LatestCreeps(Vec<CreepSnapshot>);

#[derive(Resource, Default)]
struct LatestCouriers(Vec<CourierSnapshot>);

struct CachedHeroAnimGraph {
    graph_handle: Handle<AnimationGraph>,
    nodes: HashMap<String, AnimationNodeIndex>,
    durations: HashMap<String, f32>,
}

impl CachedHeroAnimGraph {
    fn get_or_fallback(&self, anim_name: &str, fallback: &str) -> AnimationNodeIndex {
        if let Some(&node) = self.nodes.get(anim_name) {
            return node;
        }
        if let Some(&fb) = self.nodes.get(fallback) {
            return fb;
        }
        *self.nodes.values().next().unwrap_or_else(|| {
            panic!("خطای بحرانی: گراف انیمیشن هیرو هیچ کلیپی ندارد!")
        })
    }
}

#[derive(Resource, Default)]
struct HeroModelAnimCache {
    gltf_handles: HashMap<String, Handle<Gltf>>,
    graphs: HashMap<String, CachedHeroAnimGraph>,
}

impl HeroModelAnimCache {
    fn ensure_loaded<'a>(
        &'a mut self,
        model_path: &str,
        asset_server: &AssetServer,
        gltf_assets: &Assets<Gltf>,
        clip_assets: &Assets<AnimationClip>,
        animation_graphs: &mut Assets<AnimationGraph>,
    ) -> Option<&'a CachedHeroAnimGraph> {
        if !self.graphs.contains_key(model_path) {
            let handle = self
                .gltf_handles
                .entry(model_path.to_string())
                .or_insert_with(|| asset_server.load(model_path.to_string()))
                .clone();
            if let Some(gltf) = gltf_assets.get(&handle) {
                let mut graph = AnimationGraph::new();
                let mut nodes = HashMap::new();
                let mut durations = HashMap::new();
                for (name, clip_h) in &gltf.named_animations {
                    let Some(clip) = clip_assets.get(clip_h) else {
                        return None;
                    };
                    let idx = graph.add_clip(clip_h.clone(), 1.0, graph.root);
                    nodes.insert(name.to_string(), idx);
                    durations.insert(name.to_string(), clip.duration());
                }
                let graph_handle = animation_graphs.add(graph);
                self.graphs.insert(
                    model_path.to_string(),
                    CachedHeroAnimGraph {
                        graph_handle,
                        nodes,
                        durations,
                    },
                );
            }
        }
        self.graphs.get(model_path)
    }
}

struct TeamCreepAnimGraph {
    graph_handle: Handle<AnimationGraph>,
    run_node: AnimationNodeIndex,
    idle_node: AnimationNodeIndex,
    fire_node: AnimationNodeIndex,
    die_node: AnimationNodeIndex,
}

struct CreepAnimSpec {
    model_path: String,
    anim_run: String,
    anim_idle: String,
    anim_fire: String,
    anim_die: String,
}

#[derive(Resource, Default)]
struct CreepModelAnimCache {
    specs: HashMap<String, CreepAnimSpec>,
    gltf_handles: HashMap<String, Handle<Gltf>>,
    graphs: HashMap<String, TeamCreepAnimGraph>,
}

impl CreepModelAnimCache {
    fn register(
        &mut self,
        key: &str,
        model_path: &str,
        anim_run: &str,
        anim_idle: &str,
        anim_fire: &str,
        anim_die: &str,
        asset_server: &AssetServer,
    ) {
        self.specs.insert(
            key.to_string(),
            CreepAnimSpec {
                model_path: model_path.to_string(),
                anim_run: anim_run.to_string(),
                anim_idle: anim_idle.to_string(),
                anim_fire: anim_fire.to_string(),
                anim_die: anim_die.to_string(),
            },
        );
        self.gltf_handles
            .entry(model_path.to_string())
            .or_insert_with(|| asset_server.load(model_path.to_string()));
    }

    fn build_pending(
        &mut self,
        gltf_assets: &Assets<Gltf>,
        animation_graphs: &mut Assets<AnimationGraph>,
    ) {
        for (key, spec) in &self.specs {
            if self.graphs.contains_key(key) {
                continue;
            }
            let Some(handle) = self.gltf_handles.get(&spec.model_path) else { continue };
            let Some(gltf) = gltf_assets.get(handle) else { continue };

            let mut graph = AnimationGraph::new();
            let mut nodes = HashMap::new();
            for (name, clip_h) in &gltf.named_animations {
                let node_idx = graph.add_clip(clip_h.clone(), 1.0, graph.root);
                nodes.insert(name.to_string(), node_idx);
            }
            let find_node = |target: &str, fallbacks: &[&str]| -> AnimationNodeIndex {
                if let Some(&idx) = nodes.get(target) {
                    return idx;
                }
                for &fb in fallbacks {
                    if let Some(&idx) = nodes.get(fb) {
                        return idx;
                    }
                }
                *nodes.values().next().unwrap_or_else(|| {
                    panic!("فایل GLB سرباز '{}' هیچ انیمیشنی ندارد!", spec.model_path)
                })
            };
            let run_node = find_node(&spec.anim_run, &["run", "run_forward", "walk_forward"]);
            let idle_node = find_node(&spec.anim_idle, &["idle", "fire"]);
            let fire_node = find_node(&spec.anim_fire, &["fire", "firing_rifle"]);
            let die_node = find_node(&spec.anim_die, &["die", "die_1", "die_2"]);
            let graph_handle = animation_graphs.add(graph);
            self.graphs.insert(
                key.clone(),
                TeamCreepAnimGraph {
                    graph_handle,
                    run_node,
                    idle_node,
                    fire_node,
                    die_node,
                },
            );
        }
    }
}

#[derive(Resource, Default)]
struct LatestTopBarState {
    match_elapsed_secs: f32,
    dawn_total_kills: u32,
    dusk_total_kills: u32,
    heroes: Vec<TopBarHeroInfo>,
}

#[derive(Resource, Default)]
struct LatestTurrets(Vec<TurretSnapshot>);

#[derive(Resource, Default)]
struct LatestBarracks(Vec<BarracksSnapshot>);

#[derive(Resource, Default)]
struct LatestAncients(Vec<AncientSnapshot>);

#[derive(Resource, Default)]
struct LatestMatchResult(Option<TeamId>);

#[derive(SystemParam)]
struct SnapshotHudSyncParams<'w, 's> {
    hero_hud: ResMut<'w, LatestHeroSnapshot>,
    remote_players: ResMut<'w, LatestRemotePlayers>,
    creeps: ResMut<'w, LatestCreeps>,
    couriers: ResMut<'w, LatestCouriers>,
    top_bar: ResMut<'w, LatestTopBarState>,
    scoreboard: ResMut<'w, LatestScoreboard>,
    turrets: ResMut<'w, LatestTurrets>,
    barracks: ResMut<'w, LatestBarracks>,
    ancients: ResMut<'w, LatestAncients>,
    match_result: ResMut<'w, LatestMatchResult>,
    chat_state: ResMut<'w, ChatUiState>,
    ping_state: ResMut<'w, ClientPingState>,
    minimap_root_q: Query<'w, 's, Entity, With<MinimapRoot>>,
    pause_state: ResMut<'w, LatestPauseState>,
    muzzle_query: Query<'w, 's, &'static GlobalTransform, With<MuzzlePoint>>,
    telemetry: ResMut<'w, ClientTelemetryState>,
    shared_fx: Option<Res<'w, SharedFxAssets>>,
    fixed_time: Res<'w, Time<Fixed>>,
    fixed_history: ResMut<'w, FixedSoldierTransformHistory>,
}

fn spawn_match_overlay_ui(commands: &mut Commands, config: &GameConfig) {
    commands
        .spawn((
            MatchOverlayRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(28.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.08, 0.90)),
        ))
        .with_children(|root| {
            root.spawn((
                MatchPhaseHeaderText,
                Text::new("Connecting to MOBA Server..."),
                TextFont {
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.86, 0.45)),
            ));

            root.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                column_gap: px(24.0),
                ..default()
            })
            .with_children(|mid| {
                mid.spawn((
                    HeroPickRowRoot,
                    Node {
                        width: px(420.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(8.0),
                        display: Display::None,
                        ..default()
                    },
                ))
                .with_children(|list| {
                    for (idx, hero) in config.heroes.iter().enumerate() {
                        list.spawn((
                            Interaction::default(),
                            HeroPickButton { hero_index: idx },
                            Node {
                                width: percent(100),
                                padding: UiRect::all(px(10.0)),
                                border: UiRect::all(px(1.5)),
                                border_radius: BorderRadius::all(px(6.0)),
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.45, 0.65, 0.80, 0.8)),
                            BackgroundColor(Color::srgba(0.08, 0.12, 0.18, 0.95)),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(format!(
                                    "{} [{:?}] | {:?} | HP:{:.0} DMG:{:.0} RNG:{:.0}m",
                                    hero.display_name,
                                    hero.team,
                                    hero.weapon.weapon_kind,
                                    hero.max_hp,
                                    hero.weapon.damage,
                                    hero.weapon.max_range
                                )),
                                TextFont {
                                    font_size: FontSize::Px(13.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                            ));
                        });
                    }

                    list.spawn((
                        Interaction::default(),
                        HeroLockInButton,
                        Node {
                            width: percent(100),
                            margin: UiRect::top(px(8.0)),
                            padding: UiRect::all(px(12.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(2.0)),
                            border_radius: BorderRadius::all(px(6.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.25, 0.90, 0.55)),
                        BackgroundColor(Color::srgb(0.10, 0.42, 0.25)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("LOCK IN HERO"),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                });

                mid.spawn((
                    MatchStatusBodyText,
                    Text::new("Waiting for server state..."),
                    TextFont {
                        font_size: FontSize::Px(16.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.92, 0.96)),
                ));
            });

            root.spawn((
                Text::new("MOBA Tactical Shooter - Server Authoritative Draft & Match Flow"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgba(0.65, 0.72, 0.80, 0.8)),
            ));
        });
}

fn client_lobby_sync_system(
    mut config: ResMut<GameConfig>,
    mut items_config: ResMut<ItemsConfig>,
    local_override: Res<LocalConfigOverride>,
    mut shop_ui: ResMut<ShopUiState>,
    hero_hud: Res<LatestHeroSnapshot>,
    mut net: ResMut<ClientNetwork>,
    mut match_flow: ResMut<ClientMatchFlow>,
    anim_assets: Option<Res<SoldierAnimationAssets>>,
    soldier_loaded: Query<(), With<SoldierMarker>>,
) {
    while let Some(bytes) = net.client.receive_message(CHANNEL_RELIABLE_LOBBY) {
        if let Ok(pkt) = bincode::deserialize::<ServerLobbyPacket>(&bytes) {
            match pkt {
                ServerLobbyPacket::MatchState(msg) => {
                    match_flow.your_slot = Some(msg.your_slot);
                    match_flow.snapshot = Some(msg.state);
                }
                ServerLobbyPacket::SyncConfigs {
                    game_config: mut srv_cfg,
                    items_config: srv_items,
                } => {
                    if !local_override.has_local_config {
                        srv_cfg.network.server_bind_addr = config.network.server_bind_addr.clone();
                        srv_cfg.network.server_public_addr = config.network.server_public_addr.clone();
                        srv_cfg.network.client_bind_addr = config.network.client_bind_addr.clone();
                        srv_cfg.network.client_id = config.network.client_id;
                        srv_cfg.network.private_key = config.network.private_key;
                        *config = srv_cfg;
                    }
                    if !local_override.has_local_items {
                        *items_config = srv_items;
                        shop_ui.dirty = true;
                    }
                }
            }
        }
    }

    let desired_model = resolve_local_selected_hero(&config, &match_flow, &hero_hud)
        .map(|h| h.model_path.as_str())
        .unwrap_or(config.animations.model_path.as_str());
    let hero_model_ready = anim_assets
        .as_ref()
        .is_some_and(|a| a.model_path == desired_model)
        && !soldier_loaded.is_empty();

    if match_flow.current_phase() == MatchPhase::BattleLoading
        && !match_flow.map_loaded_reported
        && hero_model_ready
    {
        if let Ok(bytes) = bincode::serialize(&ClientLobbyMessage::ReportMapLoaded) {
            net.client.send_message(CHANNEL_RELIABLE_LOBBY, bytes);
            match_flow.map_loaded_reported = true;
        }
    }
}

fn hero_preview_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    gltf_assets: Res<Assets<Gltf>>,
    clip_assets: Res<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut hero_anim_cache: ResMut<HeroModelAnimCache>,
    mut match_flow: ResMut<ClientMatchFlow>,
    mut camera_query: Query<&mut Transform, With<MobaCamera>>,
    mut preview_query: Query<(Entity, &mut Transform), (With<HeroPreviewRoot>, Without<MobaCamera>)>,
    children_query: Query<&Children>,
    mut anim_player_query: Query<(Entity, &mut AnimationPlayer, Option<&AnimationGraphHandle>)>,
) {
    let phase = match_flow.current_phase();
    if phase != MatchPhase::HeroSelect {
        for (ent, _) in &preview_query {
            commands.entity(ent).despawn();
        }
        match_flow.spawned_preview_hero_index = None;
        return;
    }

    let preview_origin = Vec3::new(0.0, 2.4, 0.0);
    if let Ok(mut cam_tf) = camera_query.single_mut() {
        let cam_pos = preview_origin + Vec3::new(0.0, 1.8, 4.4);
        *cam_tf = Transform::from_translation(cam_pos).looking_at(preview_origin + Vec3::Y * 0.9, Vec3::Y);
    }

    let hero = config.active_hero(match_flow.preview_hero_index);
    if match_flow.spawned_preview_hero_index != Some(match_flow.preview_hero_index) {
        for (ent, _) in &preview_query {
            commands.entity(ent).despawn();
        }
        let gltf_scene = asset_server.load(format!("{}#Scene0", hero.model_path));
        commands.spawn((
            HeroPreviewRoot,
            WorldAssetRoot(gltf_scene),
            Transform::from_translation(preview_origin)
                .with_scale(Vec3::splat(hero.hero_scale)),
        ));
        match_flow.spawned_preview_hero_index = Some(match_flow.preview_hero_index);
        return;
    }

    let cached_opt = hero_anim_cache.ensure_loaded(
        &hero.model_path,
        &asset_server,
        &gltf_assets,
        &clip_assets,
        &mut animation_graphs,
    );

    for (root_ent, mut tf) in &mut preview_query {
        tf.rotate_y(0.55 * time.delta_secs());
        if let Some(assets) = cached_opt {
            let idle_node = assets.get_or_fallback("idle", "walk_forward");
            for desc in children_query.iter_descendants(root_ent) {
                if let Ok((p_ent, mut player, graph_opt)) = anim_player_query.get_mut(desc) {
                    if graph_opt.map_or(true, |g| g.0 != assets.graph_handle) {
                        commands
                            .entity(p_ent)
                            .insert((AnimationGraphHandle(assets.graph_handle.clone()), HeroPreviewAnimStarted));
                    }
                    if !player.is_playing_animation(idle_node) {
                        player.play(idle_node).repeat();
                    }
                }
            }
        }
    }
}

fn match_overlay_ui_system(
    config: Res<GameConfig>,
    mut net: ResMut<ClientNetwork>,
    mut match_flow: ResMut<ClientMatchFlow>,
    mut overlay_query: Query<(&mut Node, &mut BackgroundColor), With<MatchOverlayRoot>>,
    mut header_query: Query<&mut Text, With<MatchPhaseHeaderText>>,
    mut body_query: Query<&mut Text, (With<MatchStatusBodyText>, Without<MatchPhaseHeaderText>)>,
    mut pick_row_query: Query<&mut Node, (With<HeroPickRowRoot>, Without<MatchOverlayRoot>)>,
    mut hero_btn_query: Query<
        (&Interaction, &HeroPickButton, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, Without<MatchOverlayRoot>, Without<HeroLockInButton>),
    >,
    lock_btn_query: Query<&Interaction, (Changed<Interaction>, With<HeroLockInButton>)>,
) {
    let phase = match_flow.current_phase();

    if let Ok((mut root_node, mut root_bg)) = overlay_query.single_mut() {
        if phase == MatchPhase::InGame {
            if root_node.display != Display::None {
                root_node.display = Display::None;
            }
            return;
        }
        if root_node.display != Display::Flex {
            root_node.display = Display::Flex;
        }
        let target_bg = match phase {
            MatchPhase::HeroSelect => Color::srgba(0.03, 0.05, 0.08, 0.35),
            _ => Color::srgba(0.03, 0.05, 0.08, 0.92),
        };
        if root_bg.0 != target_bg {
            *root_bg = BackgroundColor(target_bg);
        }
    }

    if let Ok(mut row_node) = pick_row_query.single_mut() {
        let desired_row = if phase == MatchPhase::HeroSelect {
            Display::Flex
        } else {
            Display::None
        };
        if row_node.display != desired_row {
            row_node.display = desired_row;
        }
    }

    for (interaction, btn, mut bg, mut border) in &mut hero_btn_query {
        if *interaction == Interaction::Pressed {
            match_flow.preview_hero_index = btn.hero_index;
            *bg = BackgroundColor(Color::srgba(0.16, 0.28, 0.42, 0.95));
            *border = BorderColor::all(Color::srgb(0.95, 0.82, 0.30));
        }
    }

    if let Ok(interaction) = lock_btn_query.single() {
        if *interaction == Interaction::Pressed && phase == MatchPhase::HeroSelect {
            let hero = config.active_hero(match_flow.preview_hero_index);
            if let Ok(bytes) = bincode::serialize(&ClientLobbyMessage::SelectHero {
                hero_id: hero.id.clone(),
            }) {
                net.client.send_message(CHANNEL_RELIABLE_LOBBY, bytes);
            }
        }
    }

    let Some(snap) = &match_flow.snapshot else {
        return;
    };

    if let Ok(mut header) = header_query.single_mut() {
        let next_header = match snap.phase {
            MatchPhase::WaitingForPlayers => format!(
                "PHASE 1: WAITING FOR PLAYERS ({} / {})",
                snap.connected_count, snap.required_players
            ),
            MatchPhase::HeroSelect => {
                let is_my_turn = snap.current_pick_slot == match_flow.your_slot;
                let turn_txt = if is_my_turn {
                    "YOUR TURN TO PICK!".to_string()
                } else if let Some(s) = snap.current_pick_slot {
                    format!("Slot {} Picking...", s + 1)
                } else {
                    "Draft Complete".to_string()
                };
                format!(
                    "PHASE 2: HERO SELECT  |  {}  |  Time: {:.0}s",
                    turn_txt, snap.pick_time_remaining
                )
            }
            MatchPhase::BattleLoading => {
                "PHASE 3: BATTLE LOADING (Preparing Arena...)".to_string()
            }
            MatchPhase::InGame => String::new(),
        };
        if **header != next_header {
            **header = next_header;
        }
    }

    if let Ok(mut body) = body_query.single_mut() {
        let mut lines = Vec::new();
        match snap.phase {
            MatchPhase::WaitingForPlayers => {
                lines.push("Player Lobby Status:".to_string());
                for p in &snap.players {
                    let status = if p.connected {
                        format!("CONNECTED (ID: {})", p.client_id.unwrap_or(0))
                    } else {
                        "WAITING...".to_string()
                    };
                    let me = if Some(p.slot_index) == match_flow.your_slot {
                        " <-- YOU"
                    } else {
                        ""
                    };
                    lines.push(format!("  Slot {} [{:?}] : {}{}", p.slot_index + 1, p.team, status, me));
                }
            }
            MatchPhase::HeroSelect => {
                let preview_h = config.active_hero(match_flow.preview_hero_index);
                let taken = snap.taken_hero_ids.contains(&preview_h.id);
                lines.push(format!(
                    "Previewing: {} [{}]",
                    preview_h.display_name,
                    if taken { "TAKEN" } else { "AVAILABLE" }
                ));
                lines.push(format!(
                    "Weapon: {:?} | Mag: {} | DMG: {:.0} | Rate: {:.2}s | Range: {:.0}m",
                    preview_h.weapon.weapon_kind,
                    preview_h.weapon.magazine_size,
                    preview_h.weapon.damage,
                    preview_h.weapon.fire_rate_secs,
                    preview_h.weapon.max_range
                ));
                lines.push(format!(
                    "HP: {:.0} (+{:.1}/s) | Armor: {:.1} | Vision: {:.0}m | Gold: {}",
                    preview_h.max_hp,
                    preview_h.hp_regen_per_sec,
                    preview_h.armor,
                    preview_h.vision_range,
                    preview_h.starting_gold
                ));
                lines.push(String::new());
                lines.push("Draft Order & Picks:".to_string());
                for (step, &slot_idx) in snap.draft_order.iter().enumerate() {
                    let p = &snap.players[slot_idx];
                    let marker = if step == snap.current_draft_step { ">> " } else { "   " };
                    let pick_name = p
                        .selected_hero_id
                        .as_deref()
                        .unwrap_or("...");
                    lines.push(format!(
                        "{}Slot {} [{:?}] -> {}",
                        marker,
                        slot_idx + 1,
                        p.team,
                        pick_name
                    ));
                }
            }
            MatchPhase::BattleLoading => {
                lines.push("TEAM DAWN  vs  TEAM DUSK".to_string());
                lines.push("----------------------------------------".to_string());
                for p in &snap.players {
                    let hero_name = p
                        .selected_hero_id
                        .as_deref()
                        .and_then(|id| config.heroes.iter().find(|h| h.id == id))
                        .map(|h| h.display_name.as_str())
                        .unwrap_or("Unknown");
                    let load_txt = if p.map_loaded { "READY" } else { "LOADING..." };
                    lines.push(format!(
                        "  [{:?}] Slot {} -> {} ({})",
                        p.team,
                        p.slot_index + 1,
                        hero_name,
                        load_txt
                    ));
                }
            }
            MatchPhase::InGame => {}
        }
        let next_body = lines.join("
");
        if **body != next_body {
            **body = next_body;
        }
    }
}

fn adaptive_ui_performance_mode_system(
    match_flow: Res<ClientMatchFlow>,
    shop_ui: Res<ShopUiState>,
    settings_ui: Res<SettingsUiState>,
    mut winit_settings: ResMut<bevy::winit::WinitSettings>,
    mut window_query: Query<&mut Window, With<PrimaryWindow>>,
    mut last_ui_mode: Local<Option<bool>>,
) {
    let ui_page_active = !match_flow.is_in_game() || shop_ui.is_open || settings_ui.is_open;
    if *last_ui_mode == Some(ui_page_active) {
        return;
    }
    *last_ui_mode = Some(ui_page_active);

    if ui_page_active {
        *winit_settings = bevy::winit::WinitSettings {
            focused_mode: bevy::winit::UpdateMode::reactive_low_power(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            ),
            unfocused_mode: bevy::winit::UpdateMode::reactive_low_power(
                std::time::Duration::from_secs_f64(1.0 / 30.0),
            ),
        };
        if let Ok(mut window) = window_query.single_mut() {
            if window.present_mode != PresentMode::AutoVsync {
                window.present_mode = PresentMode::AutoVsync;
            }
        }
    } else {
        *winit_settings = bevy::winit::WinitSettings::game();
        if let Ok(mut window) = window_query.single_mut() {
            if window.present_mode != PresentMode::AutoNoVsync {
                window.present_mode = PresentMode::AutoNoVsync;
            }
        }
    }
}


// -----------------------------------------------------------------------------
// همگام‌سازی کامل سایر بازیکنان آنلاین (انیمیشن بالاتنه/پایین‌تنه + سلاح در دست)
// -----------------------------------------------------------------------------

#[derive(SystemParam)]
struct RemoteAssetsParams<'w> {
    config: Res<'w, GameConfig>,
    asset_server: Res<'w, AssetServer>,
    gltf_assets: Res<'w, Assets<Gltf>>,
    clip_assets: Res<'w, Assets<AnimationClip>>,
    animation_graphs: ResMut<'w, Assets<AnimationGraph>>,
    hero_anim_cache: ResMut<'w, HeroModelAnimCache>,
    match_flow: Res<'w, ClientMatchFlow>,
    remote_players: Res<'w, LatestRemotePlayers>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

#[derive(SystemParam)]
struct RemoteHierarchyParams<'w, 's> {
    children_query: Query<'w, 's, &'static Children>,
    names_query: Query<'w, 's, &'static Name>,
    parents_query: Query<'w, 's, &'static ChildOf>,
    target_query: Query<'w, 's, (Entity, &'static mut AnimatedBy)>,
    anim_player_check_query: Query<'w, 's, (Option<&'static AnimationGraphHandle>, Option<&'static RemoteLowerAnimPlayer>), With<AnimationPlayer>>,
    hand_query: Query<'w, 's, (Entity, &'static Name), Without<RemoteWeaponAttached>>,
}

fn sync_remote_players_system(
    mut commands: Commands,
    time: Res<Time>,
    mut res: RemoteAssetsParams,
    mut hier: RemoteHierarchyParams,
    mut existing_query: Query<(Entity, &mut RemotePlayerVisual, &mut Transform, &mut Visibility)>,
    mut lower_query: Query<
        (&RemoteLowerAnimPlayer, &mut AnimationPlayer, &mut AnimationTransitions),
        Without<RemoteUpperAnimPlayer>,
    >,
    mut upper_query: Query<
        (&RemoteUpperAnimPlayer, &mut AnimationPlayer, &mut AnimationTransitions),
        Without<RemoteLowerAnimPlayer>,
    >,
) {
    if !res.match_flow.is_in_game() {
        return;
    }

    let dt = time.delta_secs();

    // Server Authority: اگر مختصات بازیکن دشمن در Snapshot نباشد (در مه جنگ است)، مدل آن را حذف کن
    for (root_ent, vis_comp, _, _) in &existing_query {
        if !res.remote_players.0.iter().any(|rp| rp.slot_index == vis_comp.slot_index) {
            commands.entity(root_ent).despawn();
        }
    }

    for rp in &res.remote_players.0 {
        let mut root_opt: Option<Entity> = None;

        for (root_ent, mut vis_comp, mut tf, mut vis) in &mut existing_query {
            if vis_comp.slot_index == rp.slot_index {
                root_opt = Some(root_ent);
                tf.translation = tf.translation.lerp(rp.pos, (18.0 * dt).clamp(0.0, 1.0));
                tf.scale = Vec3::splat(rp.hero_scale);

                let flat_dir = Vec3::new(rp.facing_dir.x, 0.0, rp.facing_dir.z);
                if flat_dir.length_squared() > 0.0001 {
                    let target_rot = Transform::IDENTITY
                        .looking_to(-flat_dir.normalize(), Vec3::Y)
                        .rotation;
                    tf.rotation = tf.rotation.slerp(target_rot, (16.0 * dt).clamp(0.0, 1.0));
                }

                *vis = if rp.is_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };

                // ۱. اتصال گراف انیمیشن Split-Body به مدل بازیکن ریموت (خوداصلاح‌شونده بر اساس model_path واقعی هیرو)
                let cached_opt = res.hero_anim_cache.ensure_loaded(
                    &rp.model_path,
                    &res.asset_server,
                    &res.gltf_assets,
                    &res.clip_assets,
                    &mut res.animation_graphs,
                );
                if let Some(assets) = cached_opt {
                    let mut existing_upper_ent = upper_query
                        .iter()
                        .find(|(up, _, _)| up.slot_index == rp.slot_index)
                        .map(|_| {
                            hier.children_query
                                .iter_descendants(root_ent)
                                .find(|&d| upper_query.get(d).is_ok())
                        })
                        .flatten();

                    let mut lower_player_ent_opt: Option<Entity> = None;

                    for desc in hier.children_query.iter_descendants(root_ent) {
                        if let Ok((graph_opt, lower_opt)) = hier.anim_player_check_query.get(desc) {
                            if upper_query.get(desc).is_ok() {
                                continue;
                            }
                            lower_player_ent_opt = Some(desc);
                            if lower_opt.is_none()
                                || graph_opt.map_or(true, |g| g.0 != assets.graph_handle)
                            {
                                commands.entity(desc).insert((
                                    AnimationGraphHandle(assets.graph_handle.clone()),
                                    AnimationTransitions::new(),
                                    RemoteLowerAnimPlayer {
                                        slot_index: rp.slot_index,
                                    },
                                ));
                            }
                        }

                        // اتصال سلاح بازیکن ریموت به دست چپش
                        if let Ok((h_ent, h_name)) = hier.hand_query.get(desc) {
                            let n = h_name.as_str().to_lowercase();
                            let is_left = n.contains("left")
                                || n.contains("_l")
                                || n.contains(".l")
                                || n.contains("l_");
                            let is_hand =
                                n.contains("hand") || n.contains("wrist") || n.contains("palm");
                            let is_finger = n.contains("thumb")
                                || n.contains("index")
                                || n.contains("middle")
                                || n.contains("ring")
                                || n.contains("pinky")
                                || n.contains("finger")
                                || n.contains("twist");
                            if is_hand && is_left && !is_finger {
                                let w_mat = res.materials.add(StandardMaterial {
                                    base_color: match rp.weapon_kind {
                                        WeaponKind::AutomaticRifle => Color::srgb(0.14, 0.15, 0.18),
                                        WeaponKind::Sniper => Color::srgb(0.16, 0.22, 0.18),
                                        WeaponKind::Shotgun => Color::srgb(0.35, 0.22, 0.14),
                                        WeaponKind::Throwable => Color::srgb(0.80, 0.38, 0.10),
                                        WeaponKind::Pistol => Color::srgb(0.24, 0.26, 0.30),
                                    },
                                    metallic: 0.75,
                                    perceptual_roughness: 0.35,
                                    ..default()
                                });
                                commands.entity(h_ent).insert(RemoteWeaponAttached).with_children(|parent| {
                                    parent.spawn((
                                        Mesh3d(res.meshes.add(Cuboid::new(0.06, 0.09, 0.52))),
                                        MeshMaterial3d(w_mat),
                                        Transform::from_xyz(0.0, 0.03, -0.22)
                                            .with_scale(Vec3::splat(res.config.weapon_tuner.initial_scale)),
                                    ));
                                });
                            }
                        }
                    }

                    if let Some(lower_desc) = lower_player_ent_opt {
                        let upper_ent = if let Some(u) = existing_upper_ent {
                            u
                        } else {
                            let u = commands
                                .spawn((
                                    AnimationPlayer::default(),
                                    AnimationGraphHandle(assets.graph_handle.clone()),
                                    AnimationTransitions::new(),
                                    RemoteUpperAnimPlayer {
                                        slot_index: rp.slot_index,
                                    },
                                ))
                                .id();
                            commands.entity(root_ent).add_child(u);
                            existing_upper_ent = Some(u);
                            u
                        };
                        let _ = existing_upper_ent;

                        for (bone_ent, mut animated_by) in &mut hier.target_query {
                            if animated_by.0 == lower_desc
                                && is_upper_body_in_hierarchy(bone_ent, &hier.names_query, &hier.parents_query)
                            {
                                animated_by.0 = upper_ent;
                            }
                        }
                    }

                    // ۲. پخش تمام انیمیشن‌های بالاتنه و پایین‌تنه با AnimationTransitions و خوداصلاحی در صورت توقف
                    let desired_lower = if rp.is_dead {
                        rp.death_anim.clone()
                    } else if let Some(ref trav) = rp.traversal_anim {
                        trav.clone()
                    } else {
                        rp.current_locomotion.clone()
                    };

                    let desired_upper = if rp.is_dead {
                        rp.death_anim.clone()
                    } else if let Some(ref trav) = rp.traversal_anim {
                        trav.clone()
                    } else if let Some(ref up) = rp.upper_anim {
                        up.clone()
                    } else {
                        rp.current_locomotion.clone()
                    };

                    let is_looping_lower = !rp.is_dead && rp.traversal_anim.is_none();
                    let lower_node = assets.get_or_fallback(&desired_lower, "idle");
                    for (lp, mut player, mut trans) in &mut lower_query {
                        if lp.slot_index == rp.slot_index {
                            if vis_comp.last_lower != desired_lower
                                || (is_looping_lower && !player.is_playing_animation(lower_node))
                            {
                                let active = trans.play(&mut player, lower_node, Duration::from_millis(120));
                                if is_looping_lower {
                                    active.repeat();
                                }
                                vis_comp.last_lower = desired_lower.clone();
                            }
                        }
                    }

                    let is_looping_upper = !rp.is_dead
                        && rp.traversal_anim.is_none()
                        && (rp.upper_anim.is_none() || rp.upper_anim.as_deref() == Some("pistol_run"));
                    let upper_node = assets.get_or_fallback(&desired_upper, "idle");
                    for (up, mut player, mut trans) in &mut upper_query {
                        if up.slot_index == rp.slot_index {
                            if vis_comp.last_upper != desired_upper
                                || (is_looping_upper && !player.is_playing_animation(upper_node))
                            {
                                let active = trans.play(&mut player, upper_node, Duration::from_millis(90));
                                if desired_upper == "toss_grenade" {
                                    let throw_dur = rp.grenade_throw_duration_secs.max(0.05);
                                    let clip_dur = assets.durations.get("toss_grenade").copied().unwrap_or(throw_dur);
                                    let speed = (clip_dur / throw_dur).max(0.1);
                                    active.set_speed(speed);
                                }
                                if is_looping_upper {
                                    active.repeat();
                                }
                                vis_comp.last_upper = desired_upper.clone();
                            }
                        }
                    }
                }
                break;
            }
        }

        if root_opt.is_none() {
            let gltf_scene = res.asset_server.load(format!("{}#Scene0", rp.model_path));
            commands.spawn((
                RemotePlayerVisual {
                    slot_index: rp.slot_index,
                    last_lower: String::new(),
                    last_upper: String::new(),
                },
                WorldAssetRoot(gltf_scene),
                Transform::from_translation(rp.pos).with_scale(Vec3::splat(rp.hero_scale)),
                if rp.is_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ));
        }
    }
}


// -----------------------------------------------------------------------------
// نوار سلامتی (سبز برای خودی / قرمز برای دشمن) + آیکون فشنگ‌های جداگانه بالای سر هر هیرو
// -----------------------------------------------------------------------------

#[derive(Component)]
struct OverheadHeroBarRoot {
    key_id: usize,
    ammo_total: u32,
    grenade_total: u32,
}

#[derive(Component)]
struct OverheadHpFill {
    key_id: usize,
}

#[derive(Component)]
struct OverheadStaminaFill {
    key_id: usize,
}

#[derive(Component)]
struct OverheadBulletPip {
    key_id: usize,
    bullet_index: u32,
}

#[derive(Component)]
struct OverheadGrenadePip {
    key_id: usize,
    grenade_index: u32,
}

#[derive(SystemParam)]
struct OverheadBarQueries<'w, 's> {
    root_q: Query<'w, 's, (Entity, &'static OverheadHeroBarRoot, &'static mut Node)>,
    hp_q: Query<'w, 's, (&'static OverheadHpFill, &'static mut Node, &'static mut BackgroundColor), (Without<OverheadHeroBarRoot>, Without<OverheadStaminaFill>)>,
    stamina_q: Query<'w, 's, (&'static OverheadStaminaFill, &'static mut Node), (Without<OverheadHeroBarRoot>, Without<OverheadHpFill>)>,
    pip_q: Query<'w, 's, (&'static OverheadBulletPip, &'static mut BackgroundColor), (Without<OverheadHeroBarRoot>, Without<OverheadHpFill>, Without<OverheadStaminaFill>, Without<OverheadGrenadePip>)>,
    gpip_q: Query<'w, 's, (&'static OverheadGrenadePip, &'static mut BackgroundColor), (Without<OverheadHeroBarRoot>, Without<OverheadHpFill>, Without<OverheadStaminaFill>, Without<OverheadBulletPip>)>,
}

fn spawn_overhead_bar(commands: &mut Commands, key_id: usize, ammo_total: u32, grenade_total: u32) {
    let total = ammo_total.max(1);
    let g_total = grenade_total.max(1);
    let pip_w = ((136.0 / total as f32) - 1.5).clamp(2.5, 10.0);

    commands
        .spawn((
            OverheadHeroBarRoot { key_id, ammo_total: total, grenade_total: g_total },
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: px(140.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(3.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_children(|root| {
            // نوار سلامتی (سبز برای خودی و هم‌تیمی، قرمز برای دشمن)
            root.spawn((
                Node {
                    width: percent(100),
                    height: px(7.5),
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(2.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.05, 0.06, 0.08, 0.85)),
                BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.78)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    OverheadHpFill { key_id },
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.16, 0.82, 0.35)),
                ));
            });

            // نوار آبی Stamina زیر نوار سلامتی بالای سر هیرو
            root.spawn((
                Node {
                    width: percent(100),
                    height: px(4.5),
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(2.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.05, 0.08, 0.14, 0.85)),
                BackgroundColor(Color::srgba(0.06, 0.08, 0.14, 0.78)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    OverheadStaminaFill { key_id },
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.14, 0.58, 0.96)),
                ));
            });

            // ردیف آیکون فشنگ‌ها زیر نوار سلامتی (زرد = باقی‌مانده، مشکی = مصرف‌شده)
            root.spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: px(1.5),
                    row_gap: px(1.5),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|row| {
                for b_idx in 0..total {
                    row.spawn((
                        OverheadBulletPip {
                            key_id,
                            bullet_index: b_idx,
                        },
                        Node {
                            width: px(pip_w),
                            height: px(8.0),
                            border_radius: BorderRadius::top(px(1.5)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.98, 0.80, 0.16)),
                    ));
                }
            });

            // ردیف آیکون نارنجک‌ها زیر نوار فشنگ‌ها (نارنجی = آماده، مشکی = در حال شارژ)
            root.spawn((
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(3.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|grow| {
                for g_idx in 0..g_total {
                    grow.spawn((
                        OverheadGrenadePip {
                            key_id,
                            grenade_index: g_idx,
                        },
                        Node {
                            width: px(12.0),
                            height: px(7.0),
                            border_radius: BorderRadius::all(px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(1.0, 0.48, 0.14)),
                    ));
                }
            });
        });
}

fn update_overhead_hero_bars_system(
    mut commands: Commands,
    match_flow: Res<ClientMatchFlow>,
    render_state: Res<ClientRenderState>,
    hero_hud: Res<LatestHeroSnapshot>,
    remote_players: Res<LatestRemotePlayers>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut bars: OverheadBarQueries,
) {
    if !match_flow.is_in_game() {
        for (_, _, mut node) in &mut bars.root_q {
            node.display = Display::None;
        }
        return;
    }

    let Ok((camera, cam_gtf)) = camera_query.single() else {
        return;
    };

    let my_team = hero_hud.0.team;

    struct BarTarget {
        key_id: usize,
        world_head: Vec3,
        visible: bool,
        is_ally: bool,
        hp: f32,
        max_hp: f32,
        stamina: f32,
        max_stamina: f32,
        ammo_rem: u32,
        ammo_total: u32,
        grenade_rem: u32,
        grenade_total: u32,
    }

    let mut targets: Vec<BarTarget> = Vec::with_capacity(1 + remote_players.0.len());

    // ۱. خود هیروی بازیکن (همیشه سبز، بالاتر از سر بر اساس hero_scale)
    let h = &hero_hud.0;
    targets.push(BarTarget {
        key_id: usize::MAX,
        world_head: render_state.soldier_pos + Vec3::Y * (3.15 * h.hero_scale),
        visible: !render_state.is_dead,
        is_ally: true,
        hp: h.hp,
        max_hp: h.max_hp,
        stamina: h.stamina,
        max_stamina: h.max_stamina,
        ammo_rem: h.ammo_in_mag,
        ammo_total: h.magazine_size,
        grenade_rem: h.grenade_charges,
        grenade_total: h.grenade_max_charges,
    });

    // ۲. سایر هیروهای بازی (هم‌تیمی‌ها سبز، دشمنان قرمز)
    for rp in &remote_players.0 {
        targets.push(BarTarget {
            key_id: rp.slot_index,
            world_head: rp.pos + Vec3::Y * (3.15 * rp.hero_scale),
            visible: rp.is_visible && !rp.is_dead,
            is_ally: rp.team == my_team,
            hp: rp.hp,
            max_hp: rp.max_hp,
            stamina: rp.stamina,
            max_stamina: rp.max_stamina,
            ammo_rem: rp.ammo_in_mag,
            ammo_total: rp.magazine_size,
            grenade_rem: rp.grenade_charges,
            grenade_total: rp.grenade_max_charges,
        });
    }

    for (_, root, mut node) in &mut bars.root_q {
        if !targets.iter().any(|t| t.key_id == root.key_id) {
            node.display = Display::None;
        }
    }

    for tgt in &targets {
        let mut existing_ent: Option<(Entity, u32, u32)> = None;
        for (ent, root, _) in &bars.root_q {
            if root.key_id == tgt.key_id {
                existing_ent = Some((ent, root.ammo_total, root.grenade_total));
                break;
            }
        }
        match existing_ent {
            None => {
                spawn_overhead_bar(&mut commands, tgt.key_id, tgt.ammo_total, tgt.grenade_total);
                continue;
            }
            Some((ent, cur_total, cur_gtotal)) if cur_total != tgt.ammo_total.max(1) || cur_gtotal != tgt.grenade_total.max(1) => {
                commands.entity(ent).despawn();
                spawn_overhead_bar(&mut commands, tgt.key_id, tgt.ammo_total, tgt.grenade_total);
                continue;
            }
            _ => {}
        }

        let screen_opt = if tgt.visible {
            camera.world_to_viewport(cam_gtf, tgt.world_head).ok()
        } else {
            None
        };

        for (_, root, mut node) in &mut bars.root_q {
            if root.key_id == tgt.key_id {
                if let Some(sp) = screen_opt {
                    node.display = Display::Flex;
                    node.left = px(sp.x - 70.0);
                    node.top = px(sp.y - 38.0);
                } else {
                    node.display = Display::None;
                }
            }
        }

        let hp_pct = ((tgt.hp / tgt.max_hp.max(1.0)) * 100.0).clamp(0.0, 100.0);
        let hp_col = if tgt.is_ally {
            Color::srgb(0.16, 0.82, 0.35)
        } else {
            Color::srgb(0.88, 0.18, 0.18)
        };
        for (fill, mut fnode, mut fbg) in &mut bars.hp_q {
            if fill.key_id == tgt.key_id {
                fnode.width = percent(hp_pct);
                *fbg = hp_col.into();
            }
        }

        let stamina_pct = ((tgt.stamina / tgt.max_stamina.max(1.0)) * 100.0).clamp(0.0, 100.0);
        for (sfill, mut snode) in &mut bars.stamina_q {
            if sfill.key_id == tgt.key_id {
                snode.width = percent(stamina_pct);
            }
        }

        let yellow_bullet = Color::srgb(0.98, 0.80, 0.16);
        let used_black = Color::srgb(0.06, 0.06, 0.08);
        for (pip, mut pbg) in &mut bars.pip_q {
            if pip.key_id == tgt.key_id {
                *pbg = if pip.bullet_index < tgt.ammo_rem {
                    yellow_bullet.into()
                } else {
                    used_black.into()
                };
            }
        }

        let orange_grenade = Color::srgb(1.0, 0.48, 0.14);
        for (gpip, mut gbg) in &mut bars.gpip_q {
            if gpip.key_id == tgt.key_id {
                *gbg = if gpip.grenade_index < tgt.grenade_rem {
                    orange_grenade.into()
                } else {
                    used_black.into()
                };
            }
        }
    }
}

// -----------------------------------------------------------------------------
// همگام‌سازی بصری سربازان لاین (GLB + انیمیشن‌های run/idle/fire/die + هایلایت قرمز + نوار سلامتی)
// -----------------------------------------------------------------------------

#[derive(Component)]
struct CreepVisual {
    creep_id: u64,
    last_anim: Option<CreepAnimState>,
    is_highlighted: bool,
}

#[derive(Component)]
struct CreepAnimPlayer {
    creep_id: u64,
}

#[derive(Component)]
struct CreepSavedMaterial(Handle<StandardMaterial>);

#[derive(Component)]
struct OverheadCreepBarRoot {
    creep_id: u64,
}

#[derive(Component)]
struct OverheadCreepHpFill {
    creep_id: u64,
}

#[derive(SystemParam)]
struct CreepAssetParams<'w> {
    config: Res<'w, GameConfig>,
    asset_server: Res<'w, AssetServer>,
    match_flow: Res<'w, ClientMatchFlow>,
    render_state: Res<'w, ClientRenderState>,
    latest_creeps: Res<'w, LatestCreeps>,
    gltf_assets: Res<'w, Assets<Gltf>>,
    animation_graphs: ResMut<'w, Assets<AnimationGraph>>,
    creep_anim_cache: ResMut<'w, CreepModelAnimCache>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

#[allow(clippy::too_many_arguments)]
fn sync_lane_creeps_system(
    mut commands: Commands,
    time: Res<Time>,
    mut res: CreepAssetParams,
    children_query: Query<&Children>,
    player_check_query: Query<(Option<&CreepAnimPlayer>, Option<&AnimationGraphHandle>), With<AnimationPlayer>>,
    mut anim_player_query: Query<(&CreepAnimPlayer, &mut AnimationPlayer, &mut AnimationTransitions)>,
    mut existing_query: Query<(Entity, &mut CreepVisual, &mut Transform, &mut Visibility)>,
    mut mat_mesh_query: Query<(
        &mut MeshMaterial3d<StandardMaterial>,
        Option<&CreepSavedMaterial>,
    )>,
) {
    // ۱. ساخت تدریجی گراف انیمیشن برای هر مدل سرباز (تیم Dawn، تیم Dusk و تمام انواع سربازان کمپ‌های جنگلی)
    res.creep_anim_cache
        .build_pending(&res.gltf_assets, &mut res.animation_graphs);

    if !res.match_flow.is_in_game() {
        return;
    }

    let dt = time.delta_secs();
    let creep_map: HashMap<u64, &CreepSnapshot> =
        res.latest_creeps.0.iter().map(|c| (c.id, c)).collect();
    let mut seen_ids = HashSet::new();

    for (root_ent, mut vis_comp, mut tf, mut vis) in &mut existing_query {
        let Some(&cs) = creep_map.get(&vis_comp.creep_id) else {
            commands.entity(root_ent).despawn();
            continue;
        };
        seen_ids.insert(cs.id);

        tf.translation = tf.translation.lerp(cs.pos, (16.0 * dt).clamp(0.0, 1.0));
        let c_scale = cs
            .creep_type_id
            .as_deref()
            .and_then(|tid| res.config.neutral_camps.creep_types.get(tid))
            .map(|ct| ct.scale)
            .unwrap_or(res.config.creeps.scale)
            * cs.scale_multiplier;
        tf.scale = Vec3::splat(c_scale);

        let flat_dir = Vec3::new(cs.facing_dir.x, 0.0, cs.facing_dir.z);
        if flat_dir.length_squared() > 0.0001 {
            let target_rot = Transform::IDENTITY
                .looking_to(-flat_dir.normalize(), Vec3::Y)
                .rotation;
            tf.rotation = tf.rotation.slerp(target_rot, (14.0 * dt).clamp(0.0, 1.0));
        }

        *vis = if cs.is_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        // ۲. اتصال و خوداصلاحی گراف انیمیشن برای سربازان لاین و کمپ‌های جنگلی
        let anim_key: &str = if let Some(ref tid) = cs.creep_type_id {
            tid.as_str()
        } else if cs.team == TeamId::Dusk {
            "lane_dusk"
        } else {
            "lane_dawn"
        };

        if let Some(team_anim) = res.creep_anim_cache.graphs.get(anim_key) {
            for desc in children_query.iter_descendants(root_ent) {
                if let Ok((cp_opt, graph_opt)) = player_check_query.get(desc) {
                    let needs_bind = cp_opt.is_none()
                        || graph_opt.map_or(true, |g| g.0 != team_anim.graph_handle);
                    if needs_bind {
                        commands.entity(desc).insert((
                            AnimationGraphHandle(team_anim.graph_handle.clone()),
                            AnimationTransitions::new(),
                            CreepAnimPlayer { creep_id: cs.id },
                        ));
                    }
                }
            }

            let target_node = match cs.anim_state {
                CreepAnimState::Run => team_anim.run_node,
                CreepAnimState::Idle => team_anim.idle_node,
                CreepAnimState::Fire => team_anim.fire_node,
                CreepAnimState::Die => team_anim.die_node,
            };
            let is_looping = cs.anim_state != CreepAnimState::Die;
            let mut played = false;
            for (cp, mut player, mut trans) in &mut anim_player_query {
                if cp.creep_id == cs.id {
                    let needs_play = vis_comp.last_anim != Some(cs.anim_state)
                        || (is_looping && !player.is_playing_animation(target_node));
                    if needs_play {
                        let active = trans.play(&mut player, target_node, Duration::from_millis(100));
                        if is_looping {
                            active.repeat();
                        }
                        played = true;
                    }
                }
            }
            if played {
                vis_comp.last_anim = Some(cs.anim_state);
            }
        }

        // هایلایت قرمز ملایم روی سرباز هدف هنگام قرار گرفتن نشانه‌گر موس روی آن
        let should_highlight = !cs.is_dead && res.render_state.hovered_creep_id == Some(cs.id);
        if vis_comp.is_highlighted != should_highlight {
            let tint = res.config.creeps.hovered_tint_emissive_rgb;
            let mut apply_tint = |ent: Entity| {
                let Ok((mut mat_comp, saved_opt)) = mat_mesh_query.get_mut(ent) else {
                    return;
                };
                if should_highlight {
                    if saved_opt.is_none() {
                        let orig_h = mat_comp.0.clone();
                        if let Some(mut cloned) = res.materials.get(&orig_h).cloned() {
                            cloned.emissive = LinearRgba::new(tint.0, tint.1, tint.2, 1.0);
                            cloned.base_color = cloned.base_color.mix(&Color::srgb(1.0, 0.35, 0.30), 0.35);
                            mat_comp.0 = res.materials.add(cloned);
                            commands.entity(ent).insert(CreepSavedMaterial(orig_h));
                        }
                    }
                } else if let Some(saved) = saved_opt {
                    mat_comp.0 = saved.0.clone();
                    commands.entity(ent).remove::<CreepSavedMaterial>();
                }
            };
            apply_tint(root_ent);
            for desc in children_query.iter_descendants(root_ent) {
                apply_tint(desc);
            }
            vis_comp.is_highlighted = should_highlight;
        }
    }

    // اسپاون مدل GLB سربازان جدید
    for cs in &res.latest_creeps.0 {
        if seen_ids.contains(&cs.id) {
            continue;
        }
        let (c_model, c_scale) = cs
            .creep_type_id
            .as_deref()
            .and_then(|tid| res.config.neutral_camps.creep_types.get(tid))
            .map(|ct| (ct.model_path.as_str(), ct.scale))
            .unwrap_or_else(|| {
                (
                    res.config.creeps.model_path_for_team(cs.team),
                    res.config.creeps.scale,
                )
            });
        let scene_h = res.asset_server.load(format!("{}#Scene0", c_model));
        commands.spawn((
            CreepVisual {
                creep_id: cs.id,
                last_anim: None,
                is_highlighted: false,
            },
            WorldAssetRoot(scene_h),
            Transform::from_translation(cs.pos).with_scale(Vec3::splat(c_scale * cs.scale_multiplier)),
            if cs.is_visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ));
    }
}

#[allow(clippy::type_complexity)]
fn update_overhead_creep_bars_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    hero_hud: Res<LatestHeroSnapshot>,
    latest_creeps: Res<LatestCreeps>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut bar_root_q: Query<(Entity, &OverheadCreepBarRoot, &mut Node)>,
    mut bar_fill_q: Query<
        (&OverheadCreepHpFill, &mut Node, &mut BackgroundColor),
        Without<OverheadCreepBarRoot>,
    >,
) {
    if !match_flow.is_in_game() {
        for (_, _, mut node) in &mut bar_root_q {
            node.display = Display::None;
        }
        return;
    }

    let Ok((camera, cam_gtf)) = camera_query.single() else {
        return;
    };

    let my_team = hero_hud.0.team;
    let creep_map: HashMap<u64, &CreepSnapshot> =
        latest_creeps.0.iter().map(|c| (c.id, c)).collect();
    let mut existing_bars: HashMap<u64, Entity> = HashMap::new();

    for (ent, root, mut node) in &mut bar_root_q {
        let Some(&cs) = creep_map.get(&root.creep_id) else {
            commands.entity(ent).despawn();
            continue;
        };
        existing_bars.insert(root.creep_id, ent);

        if cs.is_dead || !cs.is_visible {
            node.display = Display::None;
            continue;
        }

        let c_height = cs
            .creep_type_id
            .as_deref()
            .and_then(|tid| config.neutral_camps.creep_types.get(tid))
            .map(|ct| ct.hitbox_height)
            .unwrap_or(config.creeps.hitbox_height);
        let world_head = cs.pos + Vec3::Y * (c_height + 0.45);
        if let Ok(sp) = camera.world_to_viewport(cam_gtf, world_head) {
            node.display = Display::Flex;
            node.left = px(sp.x - 28.0);
            node.top = px(sp.y - 12.0);
        } else {
            node.display = Display::None;
        }
    }

    for cs in &latest_creeps.0 {
        if !existing_bars.contains_key(&cs.id) && !cs.is_dead {
            let cid = cs.id;
            commands
                .spawn((
                    OverheadCreepBarRoot { creep_id: cid },
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        width: px(56.0),
                        height: px(6.5),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(2.0)),
                        overflow: Overflow::clip(),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.04, 0.05, 0.07, 0.9)),
                    BackgroundColor(Color::srgba(0.08, 0.09, 0.12, 0.80)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        OverheadCreepHpFill { creep_id: cid },
                        Pickable::IGNORE,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.84, 0.38)),
                    ));
                });
        }
    }

    for (fill, mut fnode, mut fbg) in &mut bar_fill_q {
        if let Some(&cs) = creep_map.get(&fill.creep_id) {
            let pct = ((cs.hp / cs.max_hp.max(1.0)) * 100.0).clamp(0.0, 100.0);
            fnode.width = percent(pct);
            *fbg = if cs.team == my_team {
                if cs.is_deniable {
                    Color::srgb(0.95, 0.78, 0.18).into()
                } else {
                    Color::srgb(0.18, 0.84, 0.38).into()
                }
            } else if cs.team == TeamId::Neutral {
                Color::srgb(0.96, 0.56, 0.14).into()
            } else {
                Color::srgb(0.90, 0.20, 0.20).into()
            };
        }
    }
}


// -----------------------------------------------------------------------------
// نوار بالای وسط صفحه (مشابه Dota 2): زمان بازی + عکس هیروهای دو تیم Dawn و Dusk
// -----------------------------------------------------------------------------

#[derive(Component)]
struct TopMatchHudRoot;

#[derive(Component)]
struct TopMatchClockText;

#[derive(Component)]
struct TopDawnKillsText;

#[derive(Component)]
struct TopDuskKillsText;

#[derive(Component)]
struct TopDawnPortraitsRow;

#[derive(Component)]
struct TopDuskPortraitsRow;

#[derive(Resource, Default)]
struct TopBarPopulatedSignature(Vec<(usize, String, bool)>);

fn spawn_top_match_hud_ui(commands: &mut Commands) {
    commands.init_resource::<TopBarPopulatedSignature>();
    commands
        .spawn((
            TopMatchHudRoot,
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(8.0),
                width: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                column_gap: px(10.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|root| {
            // تیم غرب (Dawn - زمردی)
            root.spawn((
                TopDawnPortraitsRow,
                Pickable::IGNORE,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));

            // تعداد کل کیل‌های تیم Dawn (چپ زمان بازی)
            root.spawn((
                Pickable::IGNORE,
                Node {
                    padding: UiRect::axes(px(11.0), px(7.0)),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.20, 0.85, 0.56)),
                BackgroundColor(Color::srgba(0.05, 0.16, 0.12, 0.92)),
            ))
            .with_children(|k| {
                k.spawn((
                    TopDawnKillsText,
                    Text::new("0"),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.32, 0.98, 0.65)),
                ));
            });

            // ساعت مرکزی بازی (MM:SS)
            root.spawn((
                Pickable::IGNORE,
                Node {
                    padding: UiRect::axes(px(16.0), px(7.0)),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::srgba(0.65, 0.72, 0.80, 0.85)),
                BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.92)),
            ))
            .with_children(|clock| {
                clock.spawn((
                    TopMatchClockText,
                    Text::new("00:00"),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.94, 0.76)),
                ));
            });

            // تعداد کل کیل‌های تیم Dusk (راست زمان بازی)
            root.spawn((
                Pickable::IGNORE,
                Node {
                    padding: UiRect::axes(px(11.0), px(7.0)),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.92, 0.28, 0.24)),
                BackgroundColor(Color::srgba(0.20, 0.06, 0.07, 0.92)),
            ))
            .with_children(|k| {
                k.spawn((
                    TopDuskKillsText,
                    Text::new("0"),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.36, 0.32)),
                ));
            });

            // تیم شرق (Dusk - یاقوتی)
            root.spawn((
                TopDuskPortraitsRow,
                Pickable::IGNORE,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
        });
}

fn hero_initials(name: &str) -> String {
    let mut out = String::new();
    for word in name.split_whitespace().take(2) {
        if let Some(ch) = word.chars().next() {
            out.push(ch.to_ascii_uppercase());
        }
    }
    if out.is_empty() {
        "H".to_string()
    } else {
        out
    }
}

fn spawn_portrait_card(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    hero: &TopBarHeroInfo,
) {
    let border_col = if hero.is_disconnected {
        Color::srgb(0.84, 0.20, 0.20)
    } else if hero.is_dead {
        Color::srgb(0.35, 0.35, 0.38)
    } else if hero.team == TeamId::Dawn {
        Color::srgb(0.20, 0.85, 0.56)
    } else {
        Color::srgb(0.92, 0.28, 0.24)
    };
    let bg_col = if hero.is_disconnected {
        Color::srgba(0.08, 0.08, 0.10, 0.72)
    } else if hero.is_dead {
        Color::srgba(0.10, 0.10, 0.12, 0.85)
    } else if hero.team == TeamId::Dawn {
        Color::srgba(0.08, 0.24, 0.18, 0.92)
    } else {
        Color::srgba(0.26, 0.09, 0.09, 0.92)
    };

    parent
        .spawn((
            Pickable::IGNORE,
            Node {
                width: px(50.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                ..default()
            },
        ))
        .with_children(|card| {
            card.spawn((
                Pickable::IGNORE,
                Node {
                    width: px(50.0),
                    height: px(36.0),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(4.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(border_col),
                BackgroundColor(bg_col),
            ))
            .with_children(|frame| {
                if custom_glb_exists(&hero.portrait_path) {
                    frame.spawn((
                        ImageNode::new(asset_server.load(&hero.portrait_path)),
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                    ));
                    if hero.is_dead && hero.respawn_remaining > 0.0 {
                        frame.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: percent(100),
                                height: percent(100),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.05, 0.07, 0.72)),
                        ))
                        .with_children(|ov| {
                            ov.spawn((
                                Text::new(format!("{}", hero.respawn_remaining.ceil() as u32)),
                                TextFont {
                                    font_size: FontSize::Px(16.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.35, 0.32)),
                            ));
                        });
                    }
                } else {
                    frame.spawn((
                        Text::new(if hero.is_dead && hero.respawn_remaining > 0.0 {
                            format!("{}", hero.respawn_remaining.ceil() as u32)
                        } else {
                            hero_initials(&hero.display_name)
                        }),
                        TextFont {
                            font_size: FontSize::Px(if hero.is_dead { 16.0 } else { 14.0 }),
                            ..default()
                        },
                        TextColor(if hero.is_disconnected {
                            Color::srgba(0.55, 0.55, 0.58, 0.70)
                        } else if hero.is_dead {
                            Color::srgb(1.0, 0.35, 0.32)
                        } else {
                            Color::WHITE
                        }),
                    ));
                }
                if hero.is_disconnected {
                    frame.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: percent(100),
                            height: percent(100),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.62)),
                    ))
                    .with_children(|dc| {
                        // خط قرمز افقی روی کارت بازیکن دیسکانکت‌شده
                        dc.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: px(0.0),
                                width: percent(100),
                                height: px(3.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.98, 0.20, 0.20)),
                        ));
                    });
                }
            });

            // نوار سلامتی کوچک زیر عکس هم‌تیمی‌ها
            if let Some(hp_r) = hero.hp_ratio {
                card.spawn((
                    Pickable::IGNORE,
                    Node {
                        width: px(50.0),
                        height: px(4.5),
                        border_radius: BorderRadius::all(px(2.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.12, 0.14)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: percent((hp_r * 100.0).clamp(0.0, 100.0)),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.84, 0.38)),
                    ));
                });
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn update_top_match_hud_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    match_flow: Res<ClientMatchFlow>,
    top_bar: Res<LatestTopBarState>,
    mut sig_cache: ResMut<TopBarPopulatedSignature>,
    mut root_q: Query<&mut Node, With<TopMatchHudRoot>>,
    mut clock_q: Query<&mut Text, (With<TopMatchClockText>, Without<TopDawnKillsText>, Without<TopDuskKillsText>)>,
    mut dawn_kills_q: Query<&mut Text, (With<TopDawnKillsText>, Without<TopMatchClockText>, Without<TopDuskKillsText>)>,
    mut dusk_kills_q: Query<&mut Text, (With<TopDuskKillsText>, Without<TopMatchClockText>, Without<TopDawnKillsText>)>,
    dawn_row_q: Query<(Entity, Option<&Children>), With<TopDawnPortraitsRow>>,
    dusk_row_q: Query<(Entity, Option<&Children>), With<TopDuskPortraitsRow>>,
) {
    let Ok(mut root_node) = root_q.single_mut() else {
        return;
    };
    if !match_flow.is_in_game() {
        root_node.display = Display::None;
        return;
    }
    root_node.display = Display::Flex;

    let total_secs = top_bar.match_elapsed_secs.max(0.0) as u32;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if let Ok(mut clock_txt) = clock_q.single_mut() {
        **clock_txt = format!("{:02}:{:02}", mins, secs);
    }
    if let Ok(mut dk_txt) = dawn_kills_q.single_mut() {
        **dk_txt = format!("{}", top_bar.dawn_total_kills);
    }
    if let Ok(mut ek_txt) = dusk_kills_q.single_mut() {
        **ek_txt = format!("{}", top_bar.dusk_total_kills);
    }

    let new_sig: Vec<(usize, String, bool)> = top_bar
        .heroes
        .iter()
        .map(|h| {
            let hp_bucket = h.hp_ratio.map(|r| (r * 20.0).round() as i32).unwrap_or(-1);
            let resp_sec = if h.is_dead { h.respawn_remaining.ceil() as i32 } else { 0 };
            (h.slot_index, format!("{}_{}_{}_{}", h.hero_id, hp_bucket, resp_sec, h.is_disconnected), h.is_dead)
        })
        .collect();

    if sig_cache.0 == new_sig {
        return;
    }
    sig_cache.0 = new_sig;

    if let Ok((dawn_ent, children_opt)) = dawn_row_q.single() {
        if let Some(children) = children_opt {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        commands.entity(dawn_ent).with_children(|parent| {
            for h in top_bar.heroes.iter().filter(|h| h.team == TeamId::Dawn) {
                spawn_portrait_card(parent, &asset_server, h);
            }
        });
    }

    if let Ok((dusk_ent, children_opt)) = dusk_row_q.single() {
        if let Some(children) = children_opt {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        commands.entity(dusk_ent).with_children(|parent| {
            for h in top_bar.heroes.iter().filter(|h| h.team == TeamId::Dusk) {
                spawn_portrait_card(parent, &asset_server, h);
            }
        });
    }
}


// -----------------------------------------------------------------------------
// سیستم پاپ‌آپ سکه گلد / علامت تعجب دیناید + افکت صفحه سیاه‌وسفید هنگام مرگ
// -----------------------------------------------------------------------------

#[derive(Component)]
struct FloatingWorldPopup {
    world_pos: Vec3,
    timer: Timer,
}

#[derive(Component)]
struct DeathScreenOverlay;

#[derive(Component)]
struct DeathScreenCountdownText;

fn spawn_death_screen_overlay(commands: &mut Commands) {
    commands
        .spawn((
            DeathScreenOverlay,
            Pickable::IGNORE,
            GlobalZIndex(140),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                padding: UiRect::top(px(88.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.09, 0.11, 0.60)),
        ))
        .with_children(|root| {
            root.spawn((
                Pickable::IGNORE,
                Node {
                    padding: UiRect::axes(px(22.0), px(10.0)),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(8.0)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.85, 0.24, 0.22, 0.85)),
                BackgroundColor(Color::srgba(0.06, 0.06, 0.08, 0.88)),
            ))
            .with_children(|badge| {
                badge.spawn((
                    DeathScreenCountdownText,
                    Text::new("YOU ARE DEAD - RESPAWNING IN 12s"),
                    TextFont {
                        font_size: FontSize::Px(18.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.35, 0.32)),
                ));
            });
        });
}

fn floating_world_popup_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut popup_q: Query<(Entity, &mut FloatingWorldPopup, &mut Node, &mut Visibility)>,
) {
    let Ok((camera, cam_gtf)) = camera_query.single() else {
        return;
    };
    let rise_px = config.fx.floating_popup_rise_px;

    for (ent, mut popup, mut node, mut vis) in &mut popup_q {
        popup.timer.tick(time.delta());
        if popup.timer.is_finished() {
            commands.entity(ent).despawn();
            continue;
        }
        let frac = popup.timer.fraction();
        if let Ok(sp) = camera.world_to_viewport(cam_gtf, popup.world_pos) {
            *vis = Visibility::Visible;
            node.left = px(sp.x - 16.0);
            node.top = px(sp.y - 16.0 - frac * rise_px);
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

fn death_screen_overlay_system(
    match_flow: Res<ClientMatchFlow>,
    render_state: Res<ClientRenderState>,
    hero_hud: Res<LatestHeroSnapshot>,
    mut overlay_q: Query<&mut Node, With<DeathScreenOverlay>>,
    mut txt_q: Query<&mut Text, With<DeathScreenCountdownText>>,
    mut fog_q: Query<&mut DistanceFog, With<MobaCamera>>,
) {
    let is_dead_in_game = match_flow.is_in_game() && render_state.is_dead;

    if let Ok(mut node) = overlay_q.single_mut() {
        node.display = if is_dead_in_game {
            Display::Flex
        } else {
            Display::None
        };
    }

    if is_dead_in_game {
        if let Ok(mut txt) = txt_q.single_mut() {
            let secs = hero_hud.0.respawn_remaining.ceil() as u32;
            **txt = format!("YOU ARE DEAD  |  RESPAWNING IN {}s", secs);
        }
    }

    if let Ok(mut fog) = fog_q.single_mut() {
        if is_dead_in_game {
            fog.color = Color::srgba(0.24, 0.25, 0.27, 1.0);
            fog.directional_light_color = Color::srgba(0.32, 0.33, 0.35, 0.35);
        } else {
            fog.color = Color::srgba(0.68, 0.76, 0.84, 1.0);
            fog.directional_light_color = Color::srgba(1.0, 0.92, 0.78, 0.45);
        }
    }
}


// -----------------------------------------------------------------------------
// سیستم تاورهای دولول (Rigged Bones: table_4 + gun_0 + MuzzleLeft/Right + Recoil + نوار سلامتی)
// -----------------------------------------------------------------------------

#[derive(Component)]
struct TurretVisualRoot {
    index: usize,
    is_dead: bool,
    is_highlighted: bool,
}

#[derive(Component)]
struct TurretRigBound;

#[derive(Component)]
struct TurretYawBone {
    turret_index: usize,
    base_quat: Quat,
}

#[derive(Component)]
struct TurretGunBone {
    turret_index: usize,
    base_quat: Quat,
    base_pos: Vec3,
    current_recoil: f32,
    recoil_jitter_z: f32,
}

#[derive(Component)]
struct TurretMuzzlePoint {
    turret_index: usize,
    is_left: bool,
}

#[derive(Component)]
struct TurretSavedMaterial(Handle<StandardMaterial>);

#[derive(Component)]
struct OverheadTurretBarRoot {
    turret_index: usize,
}

#[derive(Component)]
struct OverheadTurretHpFill {
    turret_index: usize,
}

#[derive(Component)]
struct OverheadTurretShieldBadge {
    turret_index: usize,
}

#[derive(Resource)]
struct TurretGltfHandles {
    dawn: Handle<Gltf>,
    dusk: Handle<Gltf>,
}

#[derive(Resource)]
struct TurretAnimationAssets {
    dawn_graph: Handle<AnimationGraph>,
    dawn_idle: Option<AnimationNodeIndex>,
    dusk_graph: Handle<AnimationGraph>,
    dusk_idle: Option<AnimationNodeIndex>,
}

#[derive(Component)]
struct TurretAnimPlayer {
    turret_index: usize,
    playing_idle: bool,
}

#[derive(SystemParam)]
struct TurretRigQueries<'w, 's> {
    children_query: Query<'w, 's, &'static Children>,
    names_query: Query<'w, 's, &'static Name>,
    parents_query: Query<'w, 's, &'static ChildOf>,
    global_tfs: Query<'w, 's, &'static GlobalTransform>,
    anim_players: Query<'w, 's, (&'static mut AnimationPlayer, Option<&'static mut AnimationTransitions>, Option<&'static mut TurretAnimPlayer>)>,
    turret_gltf_handles: Option<Res<'w, TurretGltfHandles>>,
    gltf_assets: Res<'w, Assets<Gltf>>,
    animation_graphs: ResMut<'w, Assets<AnimationGraph>>,
    turret_anim_assets: Option<Res<'w, TurretAnimationAssets>>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_turrets_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    render_state: Res<ClientRenderState>,
    latest_turrets: Res<LatestTurrets>,
    mut rig: TurretRigQueries,
    mut root_q: Query<(Entity, &mut TurretVisualRoot, Option<&TurretRigBound>)>,
    mut bone_tf_q: Query<&mut Transform>,
    mut yaw_q: Query<(Entity, &TurretYawBone)>,
    mut gun_q: Query<(Entity, &mut TurretGunBone)>,
    mut mat_mesh_q: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&TurretSavedMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ساخت گراف انیمیشن Idle برای تاورهای Dawn و Dusk
    if rig.turret_anim_assets.is_none() {
        if let Some(ref handles) = rig.turret_gltf_handles {
            if let (Some(dawn_gltf), Some(dusk_gltf)) = (
                rig.gltf_assets.get(&handles.dawn),
                rig.gltf_assets.get(&handles.dusk),
            ) {
                let build_idle_graph = |gltf: &Gltf, graphs: &mut Assets<AnimationGraph>| {
                    let mut graph = AnimationGraph::new();
                    let mut exact_node = None;
                    let mut fallback_node = None;
                    let mut all_names: Vec<String> = Vec::new();
                    for (name, clip_h) in &gltf.named_animations {
                        all_names.push(name.to_string());
                        let idx = graph.add_clip(clip_h.clone(), 1.0, graph.root);
                        if name.as_ref() == config.fx.turret_idle_anim {
                            exact_node = Some(idx);
                        } else if fallback_node.is_none()
                            && name.eq_ignore_ascii_case(&config.fx.turret_idle_anim)
                        {
                            fallback_node = Some(idx);
                        }
                    }
                    if exact_node.is_none() && fallback_node.is_none() {
                        if let Some((_n, clip_h)) = gltf.named_animations.iter().next() {
                            fallback_node = Some(graph.add_clip(clip_h.clone(), 1.0, graph.root));
                        }
                    }
                    info!("Turret GLB animations: {:?} (selected: {})", all_names, config.fx.turret_idle_anim);
                    (graphs.add(graph), exact_node.or(fallback_node))
                };
                let (dawn_graph, dawn_idle) = build_idle_graph(dawn_gltf, &mut rig.animation_graphs);
                let (dusk_graph, dusk_idle) = build_idle_graph(dusk_gltf, &mut rig.animation_graphs);
                commands.insert_resource(TurretAnimationAssets {
                    dawn_graph,
                    dawn_idle,
                    dusk_graph,
                    dusk_idle,
                });
            }
        }
    }
    if !match_flow.is_in_game() {
        return;
    }

    let dt = time.delta_secs();
    let aim_slerp = (config.fx.turret_aim_slerp_speed * dt).clamp(0.0, 1.0);
    let recoil_recover = (config.fx.turret_recoil_recover_speed * dt).clamp(0.0, 1.0);
    let m_left = config.fx.turret_muzzle_left_offset;
    let m_right = config.fx.turret_muzzle_right_offset;

    for (root_ent, mut root_comp, bound_opt) in &mut root_q {
        let Some(ts) = latest_turrets.0.get(root_comp.index) else {
            continue;
        };

        // ۱. حذف کامل تاور نابودشده از صحنه بازی
        if ts.is_dead {
            root_comp.is_dead = true;
            commands.entity(root_ent).despawn();
            continue;
        }

        // ۲. اتصال گراف انیمیشن Idle و استخوان‌های table_4 و gun_0
        if let Some(ref anim_assets) = rig.turret_anim_assets {
            let (g_handle, idle_opt) = if ts.team == TeamId::Dawn {
                (&anim_assets.dawn_graph, anim_assets.dawn_idle)
            } else {
                (&anim_assets.dusk_graph, anim_assets.dusk_idle)
            };
            for desc in rig.children_query.iter_descendants(root_ent) {
                if let Ok((mut ap, trans_opt, tap_opt)) = rig.anim_players.get_mut(desc) {
                    if tap_opt.is_none() {
                        let mut trans = AnimationTransitions::new();
                        if let Some(idle_node) = idle_opt {
                            trans.play(&mut ap, idle_node, Duration::ZERO).repeat();
                        }
                        commands.entity(desc).insert((
                            AnimationGraphHandle(g_handle.clone()),
                            trans,
                            TurretAnimPlayer {
                                turret_index: root_comp.index,
                                playing_idle: true,
                            },
                        ));
                    } else if let (Some(mut trans), Some(mut tap)) = (trans_opt, tap_opt) {
                        let should_idle = ts.aim_target_pos.is_none();
                        if should_idle && !tap.playing_idle {
                            if let Some(idle_node) = idle_opt {
                                trans.play(&mut ap, idle_node, Duration::from_millis(180)).repeat();
                            }
                            tap.playing_idle = true;
                        } else if !should_idle && tap.playing_idle {
                            ap.stop_all();
                            tap.playing_idle = false;
                        }
                    }
                }
            }
        }

        if bound_opt.is_none() {
            let mut found_yaw = false;
            let mut found_gun = false;
            for desc in rig.children_query.iter_descendants(root_ent) {
                let Ok(name) = rig.names_query.get(desc) else {
                    continue;
                };
                if name.as_str() == "table_4" {
                    if let Ok(tf) = bone_tf_q.get(desc) {
                        commands.entity(desc).insert(TurretYawBone {
                            turret_index: root_comp.index,
                            base_quat: tf.rotation,
                        });
                        found_yaw = true;
                    }
                } else if name.as_str() == "gun_0" {
                    if let Ok(tf) = bone_tf_q.get(desc) {
                        commands
                            .entity(desc)
                            .insert(TurretGunBone {
                                turret_index: root_comp.index,
                                base_quat: tf.rotation,
                                base_pos: tf.translation,
                                current_recoil: 0.0,
                                recoil_jitter_z: 0.0,
                            })
                            .with_children(|gun_children| {
                                gun_children.spawn((
                                    TurretMuzzlePoint {
                                        turret_index: root_comp.index,
                                        is_left: true,
                                    },
                                    Transform::from_xyz(m_left.0, m_left.1, m_left.2),
                                    Visibility::Visible,
                                ));
                                gun_children.spawn((
                                    TurretMuzzlePoint {
                                        turret_index: root_comp.index,
                                        is_left: false,
                                    },
                                    Transform::from_xyz(m_right.0, m_right.1, m_right.2),
                                    Visibility::Visible,
                                ));
                            });
                        found_gun = true;
                    }
                }
            }
            if found_yaw && found_gun {
                commands.entity(root_ent).insert(TurretRigBound);
            }
        }

        // ۳. هایلایت قرمز هنگام قرار گرفتن نشانه‌گر موس روی تاور دشمن
        let should_highlight = !ts.is_dead && render_state.hovered_turret_index == Some(root_comp.index);
        if root_comp.is_highlighted != should_highlight {
            let tint = config.fx.turret_hovered_tint_emissive_rgb;
            let mut apply_tint = |ent: Entity| {
                let Ok((mut mat_comp, saved_opt)) = mat_mesh_q.get_mut(ent) else {
                    return;
                };
                if should_highlight {
                    if saved_opt.is_none() {
                        let orig_h = mat_comp.0.clone();
                        if let Some(mut cloned) = materials.get(&orig_h).cloned() {
                            cloned.emissive = LinearRgba::new(tint.0, tint.1, tint.2, 1.0);
                            cloned.base_color = cloned.base_color.mix(&Color::srgb(1.0, 0.35, 0.30), 0.30);
                            mat_comp.0 = materials.add(cloned);
                            commands.entity(ent).insert(TurretSavedMaterial(orig_h));
                        }
                    }
                } else if let Some(saved) = saved_opt {
                    mat_comp.0 = saved.0.clone();
                    commands.entity(ent).remove::<TurretSavedMaterial>();
                }
            };
            apply_tint(root_ent);
            for desc in rig.children_query.iter_descendants(root_ent) {
                apply_tint(desc);
            }
            root_comp.is_highlighted = should_highlight;
        }
    }

    // ۴. چرخش افقی دیسک (table_4) فقط در زمان هدف‌گیری تا انیمیشن Idle آزادانه اجرا شود
    for (yaw_ent, yaw_bone) in &mut yaw_q {
        let Some(ts) = latest_turrets.0.get(yaw_bone.turret_index) else {
            continue;
        };
        if ts.is_dead {
            continue;
        }
        let Some(target_pos) = ts.aim_target_pos else {
            continue;
        };
        let table_world_pos = rig
            .global_tfs
            .get(yaw_ent)
            .map(|g| g.translation())
            .unwrap_or(Vec3::ZERO);
        let dx = target_pos.x - table_world_pos.x;
        let dz = target_pos.z - table_world_pos.z;

        let parent_world_rot = rig
            .parents_query
            .get(yaw_ent)
            .ok()
            .and_then(|c| rig.global_tfs.get(c.parent()).ok())
            .map(|g| g.to_scale_rotation_translation().1)
            .unwrap_or(Quat::IDENTITY);
        let local_dir = parent_world_rot.inverse() * Vec3::new(dx, 0.0, dz);
        let yaw_angle = (-local_dir.z).atan2(local_dir.x) + config.fx.turret_yaw_offset_rad;
        let local_vertical_axis = (parent_world_rot.inverse() * Vec3::Y).normalize_or_zero();
        let target_rot = if local_vertical_axis.length_squared() > 0.0001 {
            Quat::from_axis_angle(local_vertical_axis, yaw_angle) * yaw_bone.base_quat
        } else {
            yaw_bone.base_quat
        };

        if let Ok(mut tf) = bone_tf_q.get_mut(yaw_ent) {
            tf.rotation = tf.rotation.slerp(target_rot, aim_slerp);
        }
    }

    // ۵. چرخش عمودی لوله (gun_0) و لگد (Recoil) فقط در زمان هدف‌گیری
    for (gun_ent, mut gun_bone) in &mut gun_q {
        let Some(ts) = latest_turrets.0.get(gun_bone.turret_index) else {
            continue;
        };
        if ts.is_dead {
            continue;
        }
        gun_bone.current_recoil = gun_bone.current_recoil.lerp(0.0, recoil_recover);
        gun_bone.recoil_jitter_z = gun_bone.recoil_jitter_z.lerp(0.0, recoil_recover);

        let Some(target_pos) = ts.aim_target_pos else {
            if gun_bone.current_recoil > 0.01 {
                if let Ok(mut tf) = bone_tf_q.get_mut(gun_ent) {
                    tf.translation.y = gun_bone.base_pos.y - gun_bone.current_recoil;
                }
            }
            continue;
        };

        let gun_world_pos = rig
            .global_tfs
            .get(gun_ent)
            .map(|g| g.translation())
            .unwrap_or(Vec3::ZERO);
        let dx = target_pos.x - gun_world_pos.x;
        let dy = target_pos.y - gun_world_pos.y;
        let dz = target_pos.z - gun_world_pos.z;
        let pitch_angle = dy.atan2(dx.hypot(dz));
        let target_rot =
            gun_bone.base_quat * Quat::from_axis_angle(Vec3::Z, pitch_angle + gun_bone.recoil_jitter_z);

        if let Ok(mut tf) = bone_tf_q.get_mut(gun_ent) {
            tf.rotation = tf.rotation.slerp(target_rot, aim_slerp);
            tf.translation.y = gun_bone.base_pos.y - gun_bone.current_recoil;
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_overhead_turret_bars_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    hero_hud: Res<LatestHeroSnapshot>,
    latest_turrets: Res<LatestTurrets>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut bar_root_q: Query<(Entity, &OverheadTurretBarRoot, &mut Node, &mut BorderColor)>,
    mut bar_fill_q: Query<
        (&OverheadTurretHpFill, &mut Node, &mut BackgroundColor),
        Without<OverheadTurretBarRoot>,
    >,
    shield_q: Query<&OverheadTurretShieldBadge>,
) {
    if !match_flow.is_in_game() {
        for (_, _, mut node, _) in &mut bar_root_q {
            node.display = Display::None;
        }
        return;
    }

    let Ok((camera, cam_gtf)) = camera_query.single() else {
        return;
    };
    let my_team = hero_hud.0.team;

    for ts in &latest_turrets.0 {
        let mut found_ent = None;
        for (ent, root, _, _) in &bar_root_q {
            if root.turret_index == ts.index {
                found_ent = Some(ent);
                break;
            }
        }
        if found_ent.is_none() && !ts.is_dead {
            let tidx = ts.index;
            commands
                .spawn((
                    OverheadTurretBarRoot { turret_index: tidx },
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        width: px(96.0),
                        height: px(9.5),
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::all(px(3.0)),
                        overflow: Overflow::clip(),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.10, 0.12, 0.16, 0.9)),
                    BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.85)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        OverheadTurretHpFill { turret_index: tidx },
                        Pickable::IGNORE,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.16, 0.82, 0.36)),
                    ));
                });
        }
    }
    let _ = shield_q.iter().count();

    for (_, root, mut node, mut border) in &mut bar_root_q {
        let Some(ts) = latest_turrets.0.get(root.turret_index) else {
            node.display = Display::None;
            continue;
        };
        if ts.is_dead {
            node.display = Display::None;
            continue;
        }
        let t_def = &config.map.turrets[ts.index];
        let scaled_h = t_def.height * t_def.model_transform.scale_vec().y;
        let world_head = t_def.model_transform.location_vec() + Vec3::Y * (scaled_h.max(3.5) + 1.2);
        if let Ok(sp) = camera.world_to_viewport(cam_gtf, world_head) {
            node.display = Display::Flex;
            node.left = px(sp.x - 48.0);
            node.top = px(sp.y - 16.0);
            *border = if ts.is_invulnerable {
                BorderColor::all(Color::srgb(0.45, 0.78, 1.0))
            } else {
                BorderColor::all(Color::srgba(0.08, 0.10, 0.14, 0.92))
            };
        } else {
            node.display = Display::None;
        }
    }

    for (fill, mut fnode, mut fbg) in &mut bar_fill_q {
        if let Some(ts) = latest_turrets.0.get(fill.turret_index) {
            let pct = ((ts.hp / ts.max_hp.max(1.0)) * 100.0).clamp(0.0, 100.0);
            fnode.width = percent(pct);
            *fbg = if ts.team == my_team {
                Color::srgb(0.16, 0.82, 0.36).into()
            } else {
                Color::srgb(0.92, 0.22, 0.20).into()
            };
        }
    }
}

// -----------------------------------------------------------------------------
// سیستم باراکس‌ها (هایلایت دشمن + حذف پس از تخریب + نوار سلامتی بالای ساختمان)
// -----------------------------------------------------------------------------

#[derive(Component)]
struct BarracksVisualRoot {
    index: usize,
    is_dead: bool,
    is_highlighted: bool,
}

#[derive(Component)]
struct BarracksSavedMaterial(Handle<StandardMaterial>);

#[derive(Component)]
struct OverheadBarracksBarRoot {
    barracks_index: usize,
}

#[derive(Component)]
struct OverheadBarracksHpFill {
    barracks_index: usize,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_barracks_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    render_state: Res<ClientRenderState>,
    latest_barracks: Res<LatestBarracks>,
    children_query: Query<&Children>,
    mut root_q: Query<(Entity, &mut BarracksVisualRoot)>,
    mut mat_mesh_q: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&BarracksSavedMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !match_flow.is_in_game() {
        return;
    }

    for (root_ent, mut root_comp) in &mut root_q {
        let Some(bs) = latest_barracks.0.get(root_comp.index) else {
            continue;
        };
        if bs.is_dead {
            root_comp.is_dead = true;
            commands.entity(root_ent).despawn();
            continue;
        }

        let should_highlight = !bs.is_dead && render_state.hovered_barracks_index == Some(root_comp.index);
        if root_comp.is_highlighted != should_highlight {
            let tint = config.fx.turret_hovered_tint_emissive_rgb;
            let mut apply_tint = |ent: Entity| {
                let Ok((mut mat_comp, saved_opt)) = mat_mesh_q.get_mut(ent) else {
                    return;
                };
                if should_highlight {
                    if saved_opt.is_none() {
                        let orig_h = mat_comp.0.clone();
                        if let Some(mut cloned) = materials.get(&orig_h).cloned() {
                            cloned.emissive = LinearRgba::new(tint.0, tint.1, tint.2, 1.0);
                            cloned.base_color = cloned.base_color.mix(&Color::srgb(1.0, 0.35, 0.30), 0.30);
                            mat_comp.0 = materials.add(cloned);
                            commands.entity(ent).insert(BarracksSavedMaterial(orig_h));
                        }
                    }
                } else if let Some(saved) = saved_opt {
                    mat_comp.0 = saved.0.clone();
                    commands.entity(ent).remove::<BarracksSavedMaterial>();
                }
            };
            apply_tint(root_ent);
            for desc in children_query.iter_descendants(root_ent) {
                apply_tint(desc);
            }
            root_comp.is_highlighted = should_highlight;
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_overhead_barracks_bars_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    hero_hud: Res<LatestHeroSnapshot>,
    latest_barracks: Res<LatestBarracks>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut bar_root_q: Query<(Entity, &OverheadBarracksBarRoot, &mut Node, &mut BorderColor)>,
    mut bar_fill_q: Query<
        (&OverheadBarracksHpFill, &mut Node, &mut BackgroundColor),
        Without<OverheadBarracksBarRoot>,
    >,
) {
    if !match_flow.is_in_game() {
        for (_, _, mut node, _) in &mut bar_root_q {
            node.display = Display::None;
        }
        return;
    }

    let Ok((camera, cam_gtf)) = camera_query.single() else {
        return;
    };
    let my_team = hero_hud.0.team;

    for bs in &latest_barracks.0 {
        let mut found_ent = None;
        for (ent, root, _, _) in &bar_root_q {
            if root.barracks_index == bs.index {
                found_ent = Some(ent);
                break;
            }
        }
        if found_ent.is_none() && !bs.is_dead {
            let bidx = bs.index;
            commands
                .spawn((
                    OverheadBarracksBarRoot { barracks_index: bidx },
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        width: px(92.0),
                        height: px(9.0),
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::all(px(3.0)),
                        overflow: Overflow::clip(),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.10, 0.12, 0.16, 0.9)),
                    BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.85)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        OverheadBarracksHpFill { barracks_index: bidx },
                        Pickable::IGNORE,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.16, 0.82, 0.36)),
                    ));
                });
        }
    }

    for (_, root, mut node, mut border) in &mut bar_root_q {
        let Some(bs) = latest_barracks.0.get(root.barracks_index) else {
            node.display = Display::None;
            continue;
        };
        if bs.is_dead {
            node.display = Display::None;
            continue;
        }
        let b_def = &config.map.barracks[bs.index];
        let scaled_h = b_def.size.1 * b_def.model_transform.scale_vec().y;
        let world_head = b_def.model_transform.location_vec() + Vec3::Y * (scaled_h + 1.4);
        if let Ok(sp) = camera.world_to_viewport(cam_gtf, world_head) {
            node.display = Display::Flex;
            node.left = px(sp.x - 46.0);
            node.top = px(sp.y - 16.0);
            *border = if bs.is_invulnerable {
                BorderColor::all(Color::srgb(0.45, 0.78, 1.0))
            } else {
                BorderColor::all(Color::srgba(0.08, 0.10, 0.14, 0.92))
            };
        } else {
            node.display = Display::None;
        }
    }

    for (fill, mut fnode, mut fbg) in &mut bar_fill_q {
        if let Some(bs) = latest_barracks.0.get(fill.barracks_index) {
            let pct = ((bs.hp / bs.max_hp.max(1.0)) * 100.0).clamp(0.0, 100.0);
            fnode.width = percent(pct);
            *fbg = if bs.team == my_team {
                Color::srgb(0.16, 0.82, 0.36).into()
            } else {
                Color::srgb(0.92, 0.22, 0.20).into()
            };
        }
    }
}


// -----------------------------------------------------------------------------
// سیستم ساختمان اصلی (Ancient) و صفحه افکت‌پرداز پایان بازی (Victory / Defeat)
// -----------------------------------------------------------------------------

#[derive(Component)]
struct AncientVisualRoot {
    index: usize,
    is_dead: bool,
    is_highlighted: bool,
}

#[derive(Component)]
struct AncientSavedMaterial(Handle<StandardMaterial>);

#[derive(Component)]
struct OverheadAncientBarRoot {
    ancient_index: usize,
}

#[derive(Component)]
struct OverheadAncientHpFill {
    ancient_index: usize,
}

#[derive(Component)]
struct EndOfMatchOverlayRoot;

#[derive(Component)]
struct EndOfMatchBannerCard;

#[derive(Component)]
struct EndOfMatchTitleText;

#[derive(Component)]
struct EndOfMatchSubtitleText;

#[derive(Component)]
struct EndOfMatchStatsText;

fn spawn_end_of_match_overlay(commands: &mut Commands) {
    commands
        .spawn((
            EndOfMatchOverlayRoot,
            Pickable::IGNORE,
            GlobalZIndex(250),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.72)),
        ))
        .with_children(|root| {
            root.spawn((
                EndOfMatchBannerCard,
                Pickable::IGNORE,
                Node {
                    width: px(540.0),
                    padding: UiRect::all(px(32.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(14.0),
                    border: UiRect::all(px(3.0)),
                    border_radius: BorderRadius::all(px(16.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.95, 0.80, 0.25)),
                BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.94)),
            ))
            .with_children(|card| {
                card.spawn((
                    EndOfMatchTitleText,
                    Text::new("VICTORY"),
                    TextFont {
                        font_size: FontSize::Px(54.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.25, 0.98, 0.65)),
                ));
                card.spawn((
                    EndOfMatchSubtitleText,
                    Text::new("TEAM DAWN VICTORY"),
                    TextFont {
                        font_size: FontSize::Px(20.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.92, 0.95, 1.0)),
                ));
                card.spawn((
                    EndOfMatchStatsText,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.86, 0.92)),
                ));
            });
        });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn sync_ancients_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    render_state: Res<ClientRenderState>,
    latest_ancients: Res<LatestAncients>,
    children_query: Query<&Children>,
    mut root_q: Query<(Entity, &mut AncientVisualRoot)>,
    mut mat_mesh_q: Query<(&mut MeshMaterial3d<StandardMaterial>, Option<&AncientSavedMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !match_flow.is_in_game() {
        return;
    }

    for (root_ent, mut root_comp) in &mut root_q {
        let Some(asnap) = latest_ancients.0.get(root_comp.index) else {
            continue;
        };
        if asnap.is_dead {
            root_comp.is_dead = true;
            commands.entity(root_ent).despawn();
            continue;
        }

        let should_highlight = !asnap.is_dead && render_state.hovered_ancient_index == Some(root_comp.index);
        if root_comp.is_highlighted != should_highlight {
            let tint = config.fx.turret_hovered_tint_emissive_rgb;
            let mut apply_tint = |ent: Entity| {
                let Ok((mut mat_comp, saved_opt)) = mat_mesh_q.get_mut(ent) else {
                    return;
                };
                if should_highlight {
                    if saved_opt.is_none() {
                        let orig_h = mat_comp.0.clone();
                        if let Some(mut cloned) = materials.get(&orig_h).cloned() {
                            cloned.emissive = LinearRgba::new(tint.0, tint.1, tint.2, 1.0);
                            cloned.base_color = cloned.base_color.mix(&Color::srgb(1.0, 0.35, 0.30), 0.30);
                            mat_comp.0 = materials.add(cloned);
                            commands.entity(ent).insert(AncientSavedMaterial(orig_h));
                        }
                    }
                } else if let Some(saved) = saved_opt {
                    mat_comp.0 = saved.0.clone();
                    commands.entity(ent).remove::<AncientSavedMaterial>();
                }
            };
            apply_tint(root_ent);
            for desc in children_query.iter_descendants(root_ent) {
                apply_tint(desc);
            }
            root_comp.is_highlighted = should_highlight;
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_overhead_ancients_bars_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    hero_hud: Res<LatestHeroSnapshot>,
    latest_ancients: Res<LatestAncients>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut bar_root_q: Query<(Entity, &OverheadAncientBarRoot, &mut Node, &mut BorderColor)>,
    mut bar_fill_q: Query<
        (&OverheadAncientHpFill, &mut Node, &mut BackgroundColor),
        Without<OverheadAncientBarRoot>,
    >,
) {
    if !match_flow.is_in_game() {
        for (_, _, mut node, _) in &mut bar_root_q {
            node.display = Display::None;
        }
        return;
    }

    let Ok((camera, cam_gtf)) = camera_query.single() else {
        return;
    };
    let my_team = hero_hud.0.team;

    for asnap in &latest_ancients.0 {
        let mut found_ent = None;
        for (ent, root, _, _) in &bar_root_q {
            if root.ancient_index == asnap.index {
                found_ent = Some(ent);
                break;
            }
        }
        if found_ent.is_none() && !asnap.is_dead {
            let aidx = asnap.index;
            commands
                .spawn((
                    OverheadAncientBarRoot { ancient_index: aidx },
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0.0),
                        top: px(0.0),
                        width: px(136.0),
                        height: px(12.0),
                        border: UiRect::all(px(2.0)),
                        border_radius: BorderRadius::all(px(4.0)),
                        overflow: Overflow::clip(),
                        display: Display::None,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.10, 0.12, 0.16, 0.95)),
                    BackgroundColor(Color::srgba(0.06, 0.07, 0.10, 0.88)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        OverheadAncientHpFill { ancient_index: aidx },
                        Pickable::IGNORE,
                        Node {
                            width: percent(100),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.16, 0.82, 0.36)),
                    ));
                });
        }
    }

    for (_, root, mut node, mut border) in &mut bar_root_q {
        let Some(asnap) = latest_ancients.0.get(root.ancient_index) else {
            node.display = Display::None;
            continue;
        };
        if asnap.is_dead {
            node.display = Display::None;
            continue;
        }
        let a_def = &config.map.ancients[asnap.index];
        let scaled_h = (a_def.base_size.1 + a_def.core_height * 0.30) * a_def.model_transform.scale_vec().y;
        let world_head = a_def.model_transform.location_vec() + Vec3::Y * scaled_h;
        if let Ok(sp) = camera.world_to_viewport(cam_gtf, world_head) {
            node.display = Display::Flex;
            node.left = px(sp.x - 68.0);
            node.top = px(sp.y - 18.0);
            *border = if asnap.is_invulnerable {
                BorderColor::all(Color::srgb(0.45, 0.78, 1.0))
            } else {
                BorderColor::all(Color::srgba(0.95, 0.82, 0.30, 0.95))
            };
        } else {
            node.display = Display::None;
        }
    }

    for (fill, mut fnode, mut fbg) in &mut bar_fill_q {
        if let Some(asnap) = latest_ancients.0.get(fill.ancient_index) {
            let pct = ((asnap.hp / asnap.max_hp.max(1.0)) * 100.0).clamp(0.0, 100.0);
            fnode.width = percent(pct);
            *fbg = if asnap.team == my_team {
                Color::srgb(0.16, 0.82, 0.36).into()
            } else {
                Color::srgb(0.92, 0.22, 0.20).into()
            };
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn end_of_match_overlay_system(
    time: Res<Time>,
    match_flow: Res<ClientMatchFlow>,
    match_result: Res<LatestMatchResult>,
    hero_hud: Res<LatestHeroSnapshot>,
    top_bar: Res<LatestTopBarState>,
    mut root_q: Query<(&mut Node, &mut BackgroundColor), With<EndOfMatchOverlayRoot>>,
    mut card_q: Query<(&mut BorderColor, &mut BackgroundColor), (With<EndOfMatchBannerCard>, Without<EndOfMatchOverlayRoot>)>,
    mut title_q: Query<(&mut Text, &mut TextColor), With<EndOfMatchTitleText>>,
    mut sub_q: Query<&mut Text, (With<EndOfMatchSubtitleText>, Without<EndOfMatchTitleText>)>,
    mut stats_q: Query<&mut Text, (With<EndOfMatchStatsText>, Without<EndOfMatchTitleText>, Without<EndOfMatchSubtitleText>)>,
) {
    let Ok((mut root_node, mut root_bg)) = root_q.single_mut() else {
        return;
    };
    let Some(winner) = match_result.0 else {
        root_node.display = Display::None;
        return;
    };
    if !match_flow.is_in_game() {
        root_node.display = Display::None;
        return;
    }

    root_node.display = Display::Flex;
    let is_victory = winner == hero_hud.0.team;
    let pulse = (time.elapsed_secs() * 3.2).sin() * 0.5 + 0.5;
    *root_bg = BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.68 + pulse * 0.10));

    if let Ok((mut border, mut card_bg)) = card_q.single_mut() {
        if is_victory {
            *border = BorderColor::all(Color::srgb(0.25 + pulse * 0.45, 0.95, 0.55));
            *card_bg = BackgroundColor(Color::srgba(0.04, 0.14, 0.10, 0.95));
        } else {
            *border = BorderColor::all(Color::srgb(0.95, 0.20 + pulse * 0.25, 0.20));
            *card_bg = BackgroundColor(Color::srgba(0.16, 0.04, 0.05, 0.95));
        }
    }

    if let Ok((mut t, mut col)) = title_q.single_mut() {
        if is_victory {
            **t = "VICTORY".to_string();
            *col = TextColor(Color::srgb(0.30, 0.98, 0.65));
        } else {
            **t = "DEFEAT".to_string();
            *col = TextColor(Color::srgb(1.0, 0.30, 0.28));
        }
    }

    if let Ok(mut sub) = sub_q.single_mut() {
        **sub = format!("{:?} TEAM WINS — ENEMY ANCIENT DESTROYED", winner);
    }

    if let Ok(mut stats) = stats_q.single_mut() {
        let total_secs = top_bar.match_elapsed_secs.max(0.0) as u32;
        let h = &hero_hud.0;
        **stats = format!(
            "Time {:02}:{:02}  |  LVL {}  |  K {} / D {} / A {}  |  LH {}  DN {}  |  Gold {}",
            total_secs / 60,
            total_secs % 60,
            h.level,
            h.kills,
            h.deaths,
            h.assists,
            h.last_hits,
            h.denies,
            h.gold
        );
    }
}


// -----------------------------------------------------------------------------
// سیستم چت مینیمال درون بازی (مشابه Dota 2: Enter = چت تیمی، Shift+Enter = چت عمومی)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct ClientChatItem {
    entry: ChatMessageEntry,
    fade_remaining: f32,
}

#[derive(Resource, Default)]
struct ChatUiState {
    is_open: bool,
    just_opened: bool,
    all_chat: bool,
    draft_text: String,
    messages: Vec<ClientChatItem>,
    seen_ids: HashSet<u64>,
    dirty: bool,
}

#[derive(Component)]
struct ChatHudRoot;

#[derive(Component)]
struct ChatMessagesColumn;

#[derive(Component)]
struct ChatInputRow;

#[derive(Component)]
struct ChatModeBadgeText;

#[derive(Component)]
struct ChatDraftText;

fn spawn_chat_ui(commands: &mut Commands) {
    commands
        .spawn((
            ChatHudRoot,
            Pickable::IGNORE,
            GlobalZIndex(210),
            Node {
                position_type: PositionType::Absolute,
                left: px(16.0),
                bottom: px(208.0),
                width: px(450.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(6.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                ChatMessagesColumn,
                Pickable::IGNORE,
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    ..default()
                },
            ));

            root.spawn((
                ChatInputRow,
                Pickable::IGNORE,
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(10.0), px(7.0)),
                    column_gap: px(8.0),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(6.0)),
                    display: Display::None,
                    ..default()
                },
                BorderColor::all(Color::srgba(0.22, 0.78, 0.65, 0.90)),
                BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.90)),
            ))
            .with_children(|row| {
                row.spawn((
                    ChatModeBadgeText,
                    Text::new("[TEAM]"),
                    TextFont {
                        font_size: FontSize::Px(12.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.24, 0.92, 0.70)),
                ));

                row.spawn((
                    ChatDraftText,
                    Text::new("|"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.97, 1.0)),
                ));
            });
        });
}

fn client_chat_input_system(
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    mut net: ResMut<ClientNetwork>,
    mut chat_state: ResMut<ChatUiState>,
) {
    if !match_flow.is_in_game() {
        key_events.clear();
        if chat_state.is_open {
            chat_state.is_open = false;
            chat_state.draft_text.clear();
            chat_state.dirty = true;
        }
        return;
    }

    if !chat_state.is_open {
        key_events.clear();
        if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
            chat_state.is_open = true;
            chat_state.just_opened = true;
            chat_state.all_chat =
                keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
            chat_state.draft_text.clear();
            chat_state.dirty = true;
        }
        return;
    }

    if chat_state.just_opened {
        chat_state.just_opened = false;
        key_events.clear();
        return;
    }

    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match ev.logical_key {
            Key::Escape => {
                chat_state.is_open = false;
                chat_state.draft_text.clear();
                chat_state.dirty = true;
                break;
            }
            Key::Enter => {
                let text = chat_state.draft_text.trim().to_string();
                if !text.is_empty() {
                    if let Ok(bytes) = bincode::serialize(&ClientLobbyMessage::SendChat {
                        all_chat: chat_state.all_chat,
                        text,
                    }) {
                        net.client.send_message(CHANNEL_RELIABLE_LOBBY, bytes);
                    }
                }
                chat_state.is_open = false;
                chat_state.draft_text.clear();
                chat_state.dirty = true;
                break;
            }
            Key::Backspace => {
                if chat_state.draft_text.pop().is_some() {
                    chat_state.dirty = true;
                }
            }
            Key::Space => {
                if chat_state.draft_text.chars().count() < config.chat.max_message_len {
                    chat_state.draft_text.push(' ');
                    chat_state.dirty = true;
                }
            }
            Key::Character(ref s) => {
                let mut added = false;
                for ch in s.chars() {
                    if ch.is_control() {
                        continue;
                    }
                    if chat_state.draft_text.chars().count() >= config.chat.max_message_len {
                        break;
                    }
                    chat_state.draft_text.push(ch);
                    added = true;
                }
                if added {
                    chat_state.dirty = true;
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_chat_ui_system(
    mut commands: Commands,
    time: Res<Time>,
    match_flow: Res<ClientMatchFlow>,
    mut chat_state: ResMut<ChatUiState>,
    mut root_q: Query<&mut Node, With<ChatHudRoot>>,
    mut input_row_q: Query<
        (&mut Node, &mut BorderColor),
        (With<ChatInputRow>, Without<ChatHudRoot>),
    >,
    mut badge_q: Query<(&mut Text, &mut TextColor), With<ChatModeBadgeText>>,
    mut draft_q: Query<
        (&mut Text, &mut TextColor),
        (With<ChatDraftText>, Without<ChatModeBadgeText>),
    >,
    msg_col_q: Query<(Entity, Option<&Children>), With<ChatMessagesColumn>>,
) {
    let Ok(mut root_node) = root_q.single_mut() else {
        return;
    };
    if !match_flow.is_in_game() {
        root_node.display = Display::None;
        return;
    }
    root_node.display = Display::Flex;

    let dt = time.delta_secs();
    if !chat_state.is_open {
        let mut faded_any = false;
        for item in &mut chat_state.messages {
            if item.fade_remaining > 0.0 {
                item.fade_remaining = (item.fade_remaining - dt).max(0.0);
                if item.fade_remaining <= 0.0 {
                    faded_any = true;
                }
            }
        }
        if faded_any {
            chat_state.dirty = true;
        }
    }

    if let Ok((mut row_node, mut row_border)) = input_row_q.single_mut() {
        row_node.display = if chat_state.is_open {
            Display::Flex
        } else {
            Display::None
        };
        *row_border = if chat_state.all_chat {
            BorderColor::all(Color::srgba(0.96, 0.72, 0.20, 0.92))
        } else {
            BorderColor::all(Color::srgba(0.22, 0.84, 0.65, 0.92))
        };
    }

    if chat_state.is_open {
        if let Ok((mut badge_txt, mut badge_col)) = badge_q.single_mut() {
            if chat_state.all_chat {
                **badge_txt = "[ALL]".to_string();
                *badge_col = TextColor(Color::srgb(0.98, 0.76, 0.24));
            } else {
                **badge_txt = "[TEAM]".to_string();
                *badge_col = TextColor(Color::srgb(0.26, 0.92, 0.70));
            }
        }
        if let Ok((mut draft_txt, mut draft_col)) = draft_q.single_mut() {
            let cursor = if (time.elapsed_secs() * 2.4).sin() >= 0.0 {
                "|"
            } else {
                ""
            };
            if chat_state.draft_text.is_empty() {
                **draft_txt = format!("{} Type message... (Enter: Send, Esc: Cancel)", cursor);
                *draft_col = TextColor(Color::srgba(0.68, 0.74, 0.82, 0.75));
            } else {
                **draft_txt = format!("{}{}", chat_state.draft_text, cursor);
                *draft_col = TextColor(Color::srgb(0.96, 0.98, 1.0));
            }
        }
    }

    if !chat_state.dirty {
        return;
    }
    chat_state.dirty = false;

    if let Ok((col_ent, children_opt)) = msg_col_q.single() {
        if let Some(children) = children_opt {
            for child in children.iter() {
                commands.entity(child).despawn();
            }
        }
        commands.entity(col_ent).with_children(|col| {
            for item in &chat_state.messages {
                if !chat_state.is_open && item.fade_remaining <= 0.0 {
                    continue;
                }
                let e = &item.entry;
                match e.kind {
                    ChatMessageKind::SystemDisconnect => {
                        col.spawn((
                            Pickable::IGNORE,
                            Node {
                                padding: UiRect::axes(px(9.0), px(4.0)),
                                column_gap: px(6.0),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(px(5.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.24, 0.04, 0.05, 0.82)),
                        ))
                        .with_children(|line| {
                            line.spawn((
                                Text::new(&e.text),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.32, 0.32)),
                            ));
                        });
                    }
                    ChatMessageKind::SystemReconnect => {
                        col.spawn((
                            Pickable::IGNORE,
                            Node {
                                padding: UiRect::axes(px(9.0), px(4.0)),
                                column_gap: px(6.0),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(px(5.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.04, 0.22, 0.09, 0.82)),
                        ))
                        .with_children(|line| {
                            line.spawn((
                                Text::new(&e.text),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.28, 0.98, 0.50)),
                            ));
                        });
                    }
                    ChatMessageKind::PlayerTeam | ChatMessageKind::PlayerAll => {
                        let scope_tag = if e.all_chat { "[ALL]" } else { "[TEAM]" };
                        let scope_col = if e.all_chat {
                            Color::srgb(0.98, 0.76, 0.24)
                        } else {
                            Color::srgb(0.26, 0.92, 0.70)
                        };
                        let name_col = if e.sender_team == TeamId::Dawn {
                            Color::srgb(0.32, 0.95, 0.65)
                        } else {
                            Color::srgb(0.96, 0.36, 0.32)
                        };

                        col.spawn((
                            Pickable::IGNORE,
                            Node {
                                padding: UiRect::axes(px(9.0), px(4.0)),
                                column_gap: px(5.0),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(px(5.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.68)),
                        ))
                        .with_children(|line| {
                            line.spawn((
                                Text::new(scope_tag),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(scope_col),
                            ));
                            line.spawn((
                                Text::new(format!("{}:", e.sender_name)),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(name_col),
                            ));
                            line.spawn((
                                Text::new(&e.text),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.94, 0.96, 0.99)),
                            ));
                        });
                    }
                }
            }
        });
    }
}


// -----------------------------------------------------------------------------
// سیستم پینگ تیمی (Alt + Left Click روی دنیای ۳بعدی یا مینی‌مپ)
// -----------------------------------------------------------------------------

#[derive(Resource, Default)]
struct ClientPingState {
    seen_ids: HashSet<u64>,
}

#[derive(Component)]
struct WorldPingRing {
    timer: Timer,
    base_radius: f32,
}

#[derive(Component)]
struct WorldPingBeam {
    timer: Timer,
    base_pos: Vec3,
    beam_height: f32,
}

#[derive(Component)]
struct WorldPingBadge {
    world_pos: Vec3,
    timer: Timer,
}

#[derive(Component)]
struct MinimapPingDot {
    uv: Vec2,
    timer: Timer,
}

fn spawn_team_ping_visuals(
    commands: &mut Commands,
    config: &GameConfig,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    minimap_ent_opt: Option<Entity>,
    world_pos: Vec3,
    sender_name: &str,
) {
    let dur = config.ping.duration_secs;
    let ring_r = config.ping.world_ring_radius;
    let beam_h = config.ping.beam_height;

    let ping_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.20, 0.96, 0.86, 0.92),
        emissive: LinearRgba::new(1.5, 9.0, 8.0, 0.92),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands.spawn((
        WorldPingRing {
            timer: Timer::from_seconds(dur, TimerMode::Once),
            base_radius: ring_r,
        },
        Mesh3d(meshes.add(Torus::new(0.78, 1.0))),
        MeshMaterial3d(ping_mat.clone()),
        Transform::from_translation(world_pos + Vec3::Y * 0.14)
            .with_scale(Vec3::splat(ring_r * 0.2)),
        NotShadowCaster,
    ));

    commands.spawn((
        WorldPingBeam {
            timer: Timer::from_seconds(dur, TimerMode::Once),
            base_pos: world_pos,
            beam_height: beam_h,
        },
        Mesh3d(meshes.add(Cylinder::new(0.16, beam_h))),
        MeshMaterial3d(ping_mat),
        Transform::from_translation(world_pos + Vec3::Y * (beam_h * 0.5)),
        PointLight {
            color: Color::srgb(0.22, 0.96, 0.88),
            intensity: 260_000.0,
            range: 12.0,
            shadow_maps_enabled: false,
            ..default()
        },
        NotShadowCaster,
    ));

    commands
        .spawn((
            WorldPingBadge {
                world_pos: world_pos + Vec3::Y * 2.6,
                timer: Timer::from_seconds(dur, TimerMode::Once),
            },
            Pickable::IGNORE,
            GlobalZIndex(205),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                padding: UiRect::axes(px(9.0), px(4.0)),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.22, 0.96, 0.88)),
            BackgroundColor(Color::srgba(0.03, 0.12, 0.14, 0.88)),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(format!("! {}", sender_name)),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 1.0, 0.92)),
            ));
        });

    if let Some(minimap_ent) = minimap_ent_opt {
        let uv = world_to_minimap_uv(config, world_pos.x, world_pos.z);
        commands.entity(minimap_ent).with_children(|parent| {
            parent.spawn((
                MinimapPingDot {
                    uv,
                    timer: Timer::from_seconds(dur, TimerMode::Once),
                },
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(uv.x * MINIMAP_WIDTH_PX - 10.0),
                    top: px(uv.y * MINIMAP_HEIGHT_PX - 10.0),
                    width: px(20.0),
                    height: px(20.0),
                    border: UiRect::all(px(2.5)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.22, 0.98, 0.88)),
                BackgroundColor(Color::srgba(0.22, 0.98, 0.88, 0.35)),
            ));
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn client_ping_input_system(
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    match_result: Res<LatestMatchResult>,
    chat_state: Res<ChatUiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    physics_world: Res<CustomPhysicsWorld>,
    render_state: Res<ClientRenderState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut net: ResMut<ClientNetwork>,
) {
    if !match_flow.is_in_game() || match_result.0.is_some() || chat_state.is_open {
        return;
    }

    let alt_held = keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight);
    if !alt_held || !mouse_button.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let min_x = window.width() - MINIMAP_MARGIN_PX - MINIMAP_WIDTH_PX;
    let max_x = window.width() - MINIMAP_MARGIN_PX;
    let min_y = window.height() - MINIMAP_MARGIN_PX - MINIMAP_HEIGHT_PX;
    let max_y = window.height() - MINIMAP_MARGIN_PX;

    let mut ping_world_pos: Option<Vec3> = None;

    if cursor.x >= min_x && cursor.x <= max_x && cursor.y >= min_y && cursor.y <= max_y {
        let u = ((cursor.x - min_x) / MINIMAP_WIDTH_PX).clamp(0.0, 1.0);
        let v = ((cursor.y - min_y) / MINIMAP_HEIGHT_PX).clamp(0.0, 1.0);
        let wx = (u - 0.5) * config.map.map_dimensions.0;
        let wz = (v - 0.5) * config.map.map_dimensions.1;
        let ground_y = physics_world
            .cast_ray(
                Vec3::new(wx, 25.0, wz),
                Vec3::NEG_Y,
                50.0,
                soldier_collision_groups(),
                |_| true,
            )
            .map(|(_, hit)| hit.point.y)
            .unwrap_or(0.0);
        ping_world_pos = Some(Vec3::new(wx, ground_y, wz));
    } else if let Ok((camera, cam_gtf)) = camera_query.single() {
        if let Ok(ray) = camera.viewport_to_world(cam_gtf, cursor) {
            if let Some((_, hit)) = physics_world.cast_ray(
                ray.origin,
                *ray.direction,
                350.0,
                soldier_collision_groups(),
                |_| true,
            ) {
                ping_world_pos = Some(hit.point);
            } else if ray.direction.y.abs() > 1e-4 {
                let t = (render_state.soldier_pos.y - ray.origin.y) / ray.direction.y;
                if t > 0.0 {
                    ping_world_pos = Some(ray.origin + *ray.direction * t);
                }
            }
        }
    }

    if let Some(world_pos) = ping_world_pos {
        if let Ok(bytes) = bincode::serialize(&ClientLobbyMessage::SendPing { world_pos }) {
            net.client.send_message(CHANNEL_RELIABLE_LOBBY, bytes);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_ping_visuals_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut ring_q: Query<(Entity, &mut Transform, &mut WorldPingRing)>,
    mut beam_q: Query<
        (Entity, &mut Transform, &mut WorldPingBeam),
        Without<WorldPingRing>,
    >,
    mut badge_q: Query<(Entity, &mut WorldPingBadge, &mut Node, &mut Visibility)>,
    mut mini_q: Query<
        (Entity, &mut MinimapPingDot, &mut Node),
        Without<WorldPingBadge>,
    >,
) {
    let dt = time.delta();
    let t = time.elapsed_secs();
    let pulse_spd = config.ping.minimap_pulse_speed;

    for (ent, mut tf, mut ring) in &mut ring_q {
        ring.timer.tick(dt);
        let frac = ring.timer.fraction();
        let pulse = ((frac * 3.0).fract()).powf(0.65);
        let s = ring.base_radius * (0.25 + pulse * 0.95);
        tf.scale = Vec3::new(s, (1.0 - frac).max(0.15), s);
        if ring.timer.is_finished() {
            commands.entity(ent).despawn();
        }
    }

    for (ent, mut tf, mut beam) in &mut beam_q {
        beam.timer.tick(dt);
        let rem = (1.0 - beam.timer.fraction()).max(0.05);
        tf.translation = beam.base_pos + Vec3::Y * (beam.beam_height * 0.5);
        tf.scale = Vec3::new(rem, 1.0, rem);
        if beam.timer.is_finished() {
            commands.entity(ent).despawn();
        }
    }

    let cam_opt = camera_query.single().ok();
    for (ent, mut badge, mut node, mut vis) in &mut badge_q {
        badge.timer.tick(dt);
        if badge.timer.is_finished() {
            commands.entity(ent).despawn();
            continue;
        }
        if let Some((camera, cam_gtf)) = cam_opt {
            let bob = (t * 4.5).sin() * 0.25;
            if let Ok(sp) = camera.world_to_viewport(cam_gtf, badge.world_pos + Vec3::Y * bob) {
                *vis = Visibility::Visible;
                node.left = px(sp.x - 28.0);
                node.top = px(sp.y - 14.0);
            } else {
                *vis = Visibility::Hidden;
            }
        }
    }

    for (ent, mut dot, mut node) in &mut mini_q {
        dot.timer.tick(dt);
        if dot.timer.is_finished() {
            commands.entity(ent).despawn();
            continue;
        }
        let pulse = (t * pulse_spd).sin() * 0.5 + 0.5;
        let size = 10.0 + pulse * 14.0;
        node.width = px(size);
        node.height = px(size);
        node.left = px(dot.uv.x * MINIMAP_WIDTH_PX - size * 0.5);
        node.top = px(dot.uv.y * MINIMAP_HEIGHT_PX - size * 0.5);
    }
}


// -----------------------------------------------------------------------------
// سیستم توقف بازی (Pause / Unpause با F9 — طراحی مینیمال)
// -----------------------------------------------------------------------------

#[derive(Resource)]
struct LatestPauseState(PauseSnapshot);

impl Default for LatestPauseState {
    fn default() -> Self {
        Self(PauseSnapshot {
            phase: PausePhase::Running,
            paused_by_team: None,
            paused_by_name: String::new(),
            is_disconnect_pause: false,
            lock_remaining_secs: 0.0,
            countdown_remaining_secs: 0.0,
            your_team_pauses_left: 0,
            your_team_time_left_secs: 0.0,
            your_player_cooldown_secs: 0.0,
            can_you_unpause: false,
        })
    }
}

#[derive(Component)]
struct PauseOverlayRoot;

#[derive(Component)]
struct PauseBannerBox;

#[derive(Component)]
struct PauseTitleText;

#[derive(Component)]
struct PauseMetaText;

#[derive(Component)]
struct PauseCountdownText;

fn spawn_pause_overlay_ui(commands: &mut Commands) {
    commands
        .spawn((
            PauseOverlayRoot,
            Pickable::IGNORE,
            GlobalZIndex(235),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexStart,
                padding: UiRect::top(px(74.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.38)),
        ))
        .with_children(|root| {
            root.spawn((
                PauseBannerBox,
                Pickable::IGNORE,
                Node {
                    padding: UiRect::axes(px(22.0), px(10.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4.0),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(10.0)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.95, 0.78, 0.24, 0.90)),
                BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.92)),
            ))
            .with_children(|card| {
                card.spawn((
                    PauseTitleText,
                    Text::new("PAUSED"),
                    TextFont {
                        font_size: FontSize::Px(22.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.84, 0.30)),
                ));
                card.spawn((
                    PauseMetaText,
                    Text::new(""),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.90, 0.96)),
                ));
            });

            root.spawn((
                PauseCountdownText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(68.0),
                    ..default()
                },
                TextColor(Color::srgb(0.98, 0.88, 0.32)),
                Node {
                    position_type: PositionType::Absolute,
                    top: px(170.0),
                    display: Display::None,
                    ..default()
                },
            ));
        });
}

fn client_pause_input_system(
    match_flow: Res<ClientMatchFlow>,
    match_result: Res<LatestMatchResult>,
    chat_state: Res<ChatUiState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut net: ResMut<ClientNetwork>,
) {
    if !match_flow.is_in_game() || match_result.0.is_some() || chat_state.is_open {
        return;
    }
    if keyboard.just_pressed(KeyCode::F9) {
        if let Ok(bytes) = bincode::serialize(&ClientLobbyMessage::TogglePause) {
            net.client.send_message(CHANNEL_RELIABLE_LOBBY, bytes);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_pause_overlay_ui_system(
    match_flow: Res<ClientMatchFlow>,
    match_result: Res<LatestMatchResult>,
    pause_state: Res<LatestPauseState>,
    mut root_q: Query<(&mut Node, &mut BackgroundColor), With<PauseOverlayRoot>>,
    mut banner_q: Query<
        &mut Node,
        (With<PauseBannerBox>, Without<PauseOverlayRoot>, Without<PauseCountdownText>),
    >,
    mut title_q: Query<&mut Text, With<PauseTitleText>>,
    mut meta_q: Query<&mut Text, (With<PauseMetaText>, Without<PauseTitleText>)>,
    mut cd_q: Query<
        (&mut Node, &mut Text),
        (
            With<PauseCountdownText>,
            Without<PauseOverlayRoot>,
            Without<PauseBannerBox>,
            Without<PauseTitleText>,
            Without<PauseMetaText>,
        ),
    >,
) {
    if !pause_state.is_changed() && !match_flow.is_changed() && !match_result.is_changed() {
        return;
    }
    let Ok((mut root_node, mut root_bg)) = root_q.single_mut() else {
        return;
    };
    let ps = &pause_state.0;
    if !match_flow.is_in_game() || match_result.0.is_some() || ps.phase == PausePhase::Running {
        root_node.display = Display::None;
        return;
    }

    root_node.display = Display::Flex;
    match ps.phase {
        PausePhase::Running => {}
        PausePhase::Paused => {
            *root_bg = BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.42));
            if let Ok(mut bn) = banner_q.single_mut() {
                bn.display = Display::Flex;
            }
            if let Ok((mut cdn, _)) = cd_q.single_mut() {
                cdn.display = Display::None;
            }
            if let Ok(mut t) = title_q.single_mut() {
                **t = if ps.is_disconnect_pause {
                    "PAUSED (DISCONNECT)".to_string()
                } else {
                    "PAUSED".to_string()
                };
            }
            if let Ok(mut m) = meta_q.single_mut() {
                let bank_s = ps.your_team_time_left_secs.ceil() as u32;
                let action_hint = if ps.can_you_unpause {
                    "[F9] Unpause".to_string()
                } else {
                    format!("Locked {}s", ps.lock_remaining_secs.ceil() as u32)
                };
                **m = format!(
                    "{}  •  Pauses: {}  •  Bank: {:02}:{:02}  •  {}",
                    ps.paused_by_name,
                    ps.your_team_pauses_left,
                    bank_s / 60,
                    bank_s % 60,
                    action_hint
                );
            }
        }
        PausePhase::UnpauseCountdown => {
            *root_bg = BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.22));
            if let Ok(mut bn) = banner_q.single_mut() {
                bn.display = Display::None;
            }
            if let Ok((mut cdn, mut cdt)) = cd_q.single_mut() {
                cdn.display = Display::Flex;
                let sec = ps.countdown_remaining_secs.ceil().max(1.0) as u32;
                **cdt = format!("{}", sec);
            }
        }
    }
}


fn update_hero_stamina_hud_system(
    hero_state: Res<LatestHeroSnapshot>,
    mut fill_q: Query<(&mut Node, &mut BackgroundColor), With<HeroStaminaBarFill>>,
    mut txt_q: Query<&mut Text, With<HeroStaminaBarText>>,
) {
    if !hero_state.is_changed() {
        return;
    }
    let h = &hero_state.0;
    let pct = ((h.stamina / h.max_stamina.max(1.0)) * 100.0).clamp(0.0, 100.0);
    if let Ok((mut node, mut bg)) = fill_q.single_mut() {
        node.width = percent(pct);
        *bg = if pct >= 30.0 {
            Color::srgb(0.14, 0.58, 0.96).into()
        } else {
            Color::srgb(0.10, 0.36, 0.72).into()
        };
    }
    if let Ok(mut t) = txt_q.single_mut() {
        **t = format!(
            "STAMINA  {:.0} / {:.0} (+{:.1}/s)",
            h.stamina, h.max_stamina, h.stamina_regen_per_sec
        );
    }
}


fn compensate_upper_body_hips_twist_system(
    match_flow: Res<ClientMatchFlow>,
    mut render_state: ResMut<ClientRenderState>,
    hips_q: Query<&Transform, (With<SoldierHipsBone>, Without<SoldierSpineRootBone>)>,
    mut spine_q: Query<&mut Transform, (With<SoldierSpineRootBone>, Without<SoldierHipsBone>)>,
) {
    if !match_flow.is_in_game() {
        return;
    }
    let Ok(hips_tf) = hips_q.single() else {
        return;
    };
    let Ok(mut spine_tf) = spine_q.single_mut() else {
        return;
    };

    let (hips_yaw, _, _) = hips_tf.rotation.to_euler(EulerRot::YXZ);
    if render_state.current_locomotion == "idle" && render_state.previous_upper_anim == "idle" {
        render_state.neutral_hips_yaw = Some(hips_yaw);
        return;
    }

    if !render_state.traversal_active
        && render_state.previous_upper_anim != render_state.current_locomotion
    {
        let neutral = render_state.neutral_hips_yaw.unwrap_or(0.0);
        let mut delta_yaw = hips_yaw - neutral;
        while delta_yaw > std::f32::consts::PI {
            delta_yaw -= std::f32::consts::TAU;
        }
        while delta_yaw < -std::f32::consts::PI {
            delta_yaw += std::f32::consts::TAU;
        }
        spine_tf.rotation = Quat::from_rotation_y(-delta_yaw) * spine_tf.rotation;
    }
}


// -----------------------------------------------------------------------------
// نمایشگر مینیمال آمار شبکه و فریم‌ریت در بالا سمت راست (FPS / Ping / Packet Loss)
// -----------------------------------------------------------------------------

#[derive(Resource)]
struct ClientTelemetryState {
    fps_smoothed: f32,
    last_server_tick: Option<u64>,
    min_tick_step: Option<u64>,
    in_expected_window: u64,
    in_lost_window: u64,
    loss_sample_timer: f32,
    packet_loss_in_pct: f32,
    refresh_timer: f32,
}

impl Default for ClientTelemetryState {
    fn default() -> Self {
        Self {
            fps_smoothed: 60.0,
            last_server_tick: None,
            min_tick_step: None,
            in_expected_window: 0,
            in_lost_window: 0,
            loss_sample_timer: 0.0,
            packet_loss_in_pct: 0.0,
            refresh_timer: 0.0,
        }
    }
}

#[derive(Component)]
struct TelemetryTopRowText;

#[derive(Component)]
struct TelemetryLossRowText;

fn spawn_telemetry_hud_ui(commands: &mut Commands) {
    commands
        .spawn((
            Pickable::IGNORE,
            GlobalZIndex(210),
            Node {
                position_type: PositionType::Absolute,
                top: px(10.0),
                right: px(14.0),
                padding: UiRect::axes(px(10.0), px(5.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(2.0),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BorderColor::all(Color::srgba(0.28, 0.36, 0.48, 0.50)),
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.74)),
        ))
        .with_children(|box_node| {
            box_node.spawn((
                TelemetryTopRowText,
                Text::new("FPS 60  |  PING 0ms"),
                TextFont {
                    font_size: FontSize::Px(11.5),
                    ..default()
                },
                TextColor(Color::srgb(0.86, 0.92, 0.98)),
            ));
            box_node.spawn((
                TelemetryLossRowText,
                Text::new("IN 0.0%  |  OUT 0.0%"),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.74, 0.84, 0.94)),
            ));
        });
}

fn update_telemetry_hud_system(
    time: Res<Time>,
    net: Res<ClientNetwork>,
    mut telemetry: ResMut<ClientTelemetryState>,
    mut top_q: Query<(&mut Text, &mut TextColor), With<TelemetryTopRowText>>,
    mut loss_q: Query<
        (&mut Text, &mut TextColor),
        (With<TelemetryLossRowText>, Without<TelemetryTopRowText>),
    >,
) {
    let dt = time.delta_secs();
    if dt > 1e-4 {
        let inst_fps = (1.0 / dt).clamp(1.0, 999.0);
        telemetry.fps_smoothed = telemetry.fps_smoothed * 0.92 + inst_fps * 0.08;
    }

    telemetry.loss_sample_timer += dt;
    if telemetry.loss_sample_timer >= 2.0 {
        telemetry.loss_sample_timer = 0.0;
        if telemetry.in_expected_window > 0 {
            let raw = (telemetry.in_lost_window as f32 / telemetry.in_expected_window as f32) * 100.0;
            telemetry.packet_loss_in_pct = (telemetry.packet_loss_in_pct * 0.4 + raw * 0.6).clamp(0.0, 100.0);
            telemetry.in_expected_window = 0;
            telemetry.in_lost_window = 0;
        } else {
            telemetry.packet_loss_in_pct = 0.0;
        }
    }

    telemetry.refresh_timer += dt;
    if telemetry.refresh_timer < 0.40 {
        return;
    }
    telemetry.refresh_timer = 0.0;

    let ping_ms = (net.client.rtt() * 1000.0).max(0.0) as u32;
    let loss_out_pct = (net.client.packet_loss() * 100.0).clamp(0.0, 100.0) as f32;
    let fps = telemetry.fps_smoothed.round() as u32;
    let loss_in = telemetry.packet_loss_in_pct;

    if let Ok((mut text, mut color)) = top_q.single_mut() {
        **text = format!("FPS {}  |  PING {}ms", fps, ping_ms);
        *color = if ping_ms >= 160 || fps < 30 {
            TextColor(Color::srgb(0.98, 0.40, 0.36))
        } else if ping_ms >= 95 || fps < 50 {
            TextColor(Color::srgb(0.96, 0.82, 0.34))
        } else {
            TextColor(Color::srgb(0.86, 0.92, 0.98))
        };
    }

    if let Ok((mut text, mut color)) = loss_q.single_mut() {
        **text = format!("IN {:.1}%  |  OUT {:.1}%", loss_in, loss_out_pct);
        *color = if loss_in >= 5.0 || loss_out_pct >= 5.0 {
            TextColor(Color::srgb(0.98, 0.40, 0.36))
        } else if loss_in >= 2.0 || loss_out_pct >= 2.0 {
            TextColor(Color::srgb(0.96, 0.82, 0.34))
        } else {
            TextColor(Color::srgb(0.74, 0.84, 0.94))
        };
    }
}


#[derive(Resource, Default)]
struct FixedSoldierTransformHistory {
    prev_feet: Vec3,
    curr_feet: Vec3,
    initialized: bool,
}

fn client_reconcile_fixed_system(
    config: Res<GameConfig>,
    prediction: Res<ClientPhysicsPredictionState>,
    mut soldier_phys_q: Query<&mut Transform, With<PhysicsSoldier>>,
) {
    let Some(auth_feet) = prediction.latest_authoritative_feet_pos else {
        return;
    };
    if let Ok(mut phys_tf) = soldier_phys_q.single_mut() {
        reconcile_soldier_physics_pos(
            &mut phys_tf,
            auth_feet,
            prediction.latest_hero_scale,
            &config,
        );
    }
}

fn record_soldier_fixed_transform_system(
    config: Res<GameConfig>,
    hero_hud: Res<LatestHeroSnapshot>,
    mut history: ResMut<FixedSoldierTransformHistory>,
    soldier_phys_q: Query<&Transform, With<PhysicsSoldier>>,
) {
    let Ok(phys_tf) = soldier_phys_q.single() else {
        return;
    };
    let hero_scale = hero_hud.0.hero_scale;
    let feet = Vec3::new(
        phys_tf.translation.x,
        phys_tf.translation.y - config.physics.soldier_capsule_center_y * hero_scale,
        phys_tf.translation.z,
    );
    if !history.initialized {
        history.prev_feet = feet;
        history.curr_feet = feet;
        history.initialized = true;
    } else {
        history.prev_feet = history.curr_feet;
        history.curr_feet = feet;
    }
}

fn sync_shop_context_and_network_system(
    match_flow: Res<ClientMatchFlow>,
    chat_state: Res<ChatUiState>,
    hero_hud: Res<LatestHeroSnapshot>,
    render_state: Res<ClientRenderState>,
    inspected: Res<InspectedUnit>,
    latest_remotes: Res<LatestRemotePlayers>,
    latest_couriers: Res<LatestCouriers>,
    mut ctx: ResMut<ShopContextSnapshot>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut net: ResMut<ClientNetwork>,
) {
    let in_game = match_flow.is_in_game();
    let (insp_inv, insp_ro) = match *inspected {
        InspectedUnit::OwnHero | InspectedUnit::OwnCourier => (None, false),
        InspectedUnit::RemoteHero(slot) => (
            latest_remotes.0.iter().find(|r| r.slot_index == slot).map(|r| r.inventory.clone()),
            true,
        ),
        InspectedUnit::EnemyCourier(slot) => (
            latest_couriers.0.iter().find(|c| c.owner_slot == slot).map(|c| c.inventory.clone()),
            true,
        ),
        InspectedUnit::Creep(_) | InspectedUnit::Turret(_) | InspectedUnit::Barracks(_) | InspectedUnit::Ancient(_) => {
            (Some(Vec::new()), true)
        }
    };
    if ctx.in_game != in_game
        || ctx.chat_open != chat_state.is_open
        || ctx.in_fountain != hero_hud.0.in_fountain
        || ctx.is_dead != render_state.is_dead
        || ctx.gold != hero_hud.0.gold
        || ctx.inventory != hero_hud.0.inventory
        || (ctx.courier_hp - hero_hud.0.courier_hp).abs() > 0.1
        || (ctx.courier_max_hp - hero_hud.0.courier_max_hp).abs() > 0.1
        || ctx.courier_is_dead != hero_hud.0.courier_is_dead
        || (ctx.courier_respawn_remaining - hero_hud.0.courier_respawn_remaining).abs() > 0.2
        || ctx.courier_order != hero_hud.0.courier_order
        || ctx.courier_in_fountain != hero_hud.0.courier_in_fountain
        || ctx.courier_inventory != hero_hud.0.courier_inventory
        || ctx.inspected_inventory != insp_inv
        || ctx.inspected_is_read_only != insp_ro
    {
        ctx.in_game = in_game;
        ctx.chat_open = chat_state.is_open;
        ctx.in_fountain = hero_hud.0.in_fountain;
        ctx.is_dead = render_state.is_dead;
        ctx.gold = hero_hud.0.gold;
        ctx.inventory = hero_hud.0.inventory.clone();
        ctx.courier_hp = hero_hud.0.courier_hp;
        ctx.courier_max_hp = hero_hud.0.courier_max_hp;
        ctx.courier_is_dead = hero_hud.0.courier_is_dead;
        ctx.courier_respawn_remaining = hero_hud.0.courier_respawn_remaining;
        ctx.courier_order = hero_hud.0.courier_order;
        ctx.courier_in_fountain = hero_hud.0.courier_in_fountain;
        ctx.courier_inventory = hero_hud.0.courier_inventory.clone();
        ctx.inspected_inventory = insp_inv;
        ctx.inspected_is_read_only = insp_ro;
    }

    if net.client.is_connected() && !outgoing.0.is_empty() {
        for msg in outgoing.0.drain(..) {
            if let Ok(bytes) = bincode::serialize(&msg) {
                net.client.send_message(CHANNEL_RELIABLE_LOBBY, bytes);
            }
        }
    }
}


#[derive(Component)]
struct CourierVisualEntity {
    owner_slot: usize,
    model_path: String,
}

#[derive(Component)]
struct CourierOverheadBar {
    owner_slot: usize,
}

#[derive(Component)]
struct CourierOverheadFill {
    owner_slot: usize,
}

#[allow(clippy::too_many_arguments)]
fn sync_couriers_system(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    latest_couriers: Res<LatestCouriers>,
    mut existing_q: Query<(Entity, &CourierVisualEntity, &mut Transform, &mut Visibility)>,
) {
    let t_secs = time.elapsed_secs();
    let mut seen = std::collections::HashSet::new();

    for c in &latest_couriers.0 {
        seen.insert(c.owner_slot);
        let mut found = false;
        for (ent, vis_comp, mut tf, mut vis) in &mut existing_q {
            if vis_comp.owner_slot == c.owner_slot {
                if vis_comp.model_path != c.model_path {
                    commands.entity(ent).despawn();
                    continue;
                }
                found = true;
                let desired_vis = if c.is_visible && !c.is_dead {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *vis != desired_vis {
                    *vis = desired_vis;
                }
                let hover_offset = (t_secs * 4.2 + c.owner_slot as f32).sin() * 0.18;
                let target_pos = c.pos + Vec3::Y * hover_offset;
                let alpha = 1.0 - (-18.0 * time.delta_secs()).exp();
                tf.translation = tf.translation.lerp(target_pos, alpha);
                if c.facing_dir.length_squared() > 0.01 {
                    let look_target = tf.translation + Vec3::new(c.facing_dir.x, 0.0, c.facing_dir.z);
                    tf.look_at(look_target, Vec3::Y);
                }
                break;
            }
        }

        if !found {
            let team_color = if c.team == TeamId::Dawn {
                Color::srgb(0.25, 0.85, 1.0)
            } else {
                Color::srgb(1.0, 0.42, 0.25)
            };
            let _ = (&mut meshes, &mut materials, team_color);
            commands.spawn((
                CourierVisualEntity {
                    owner_slot: c.owner_slot,
                    model_path: c.model_path.clone(),
                },
                WorldAssetRoot(
                    asset_server.load(GltfAssetLabel::Scene(0).from_asset(c.model_path.clone())),
                ),
                Transform::from_translation(c.pos).with_scale(Vec3::splat(c.scale)),
                if c.is_visible && !c.is_dead {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ));
        }
    }

    for (ent, vis_comp, _, _) in &existing_q {
        if !seen.contains(&vis_comp.owner_slot) {
            commands.entity(ent).despawn();
        }
    }
}

fn update_overhead_courier_bars_system(
    mut commands: Commands,
    match_flow: Res<ClientMatchFlow>,
    latest_couriers: Res<LatestCouriers>,
    cam_q: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut bar_q: Query<(Entity, &CourierOverheadBar, &mut Node)>,
    mut fill_q: Query<(&CourierOverheadFill, &mut Node, &mut BackgroundColor), Without<CourierOverheadBar>>,
) {
    let Ok((camera, cam_gt)) = cam_q.single() else {
        return;
    };
    let my_slot = match_flow.your_slot;
    let my_team = match_flow
        .your_slot
        .and_then(|s| match_flow.snapshot.as_ref().and_then(|sn| sn.players.get(s)))
        .map(|p| p.team);

    let mut active_slots = std::collections::HashSet::new();

    for c in &latest_couriers.0 {
        if !c.is_visible || c.is_dead {
            continue;
        }
        let world_pos = c.pos + Vec3::Y * 1.25;
        let Ok(screen_pos) = camera.world_to_viewport(cam_gt, world_pos) else {
            continue;
        };
        active_slots.insert(c.owner_slot);

        let bar_w = 54.0;
        let ratio = (c.hp / c.max_hp.max(1.0)).clamp(0.0, 1.0);
        let fill_color = if Some(c.owner_slot) == my_slot {
            Color::srgb(0.30, 0.92, 1.0)
        } else if Some(c.team) == my_team {
            Color::srgb(0.28, 0.90, 0.45)
        } else {
            Color::srgb(0.96, 0.30, 0.30)
        };

        let mut exists = false;
        for (_, bar, mut node) in &mut bar_q {
            if bar.owner_slot == c.owner_slot {
                exists = true;
                node.left = px(screen_pos.x - bar_w * 0.5);
                node.top = px(screen_pos.y - 10.0);
                break;
            }
        }
        for (fill, mut fnode, mut fbg) in &mut fill_q {
            if fill.owner_slot == c.owner_slot {
                fnode.width = percent(ratio * 100.0);
                fbg.0 = fill_color;
            }
        }

        if !exists {
            commands
                .spawn((
                    CourierOverheadBar { owner_slot: c.owner_slot },
                    Pickable::IGNORE,
                    GlobalZIndex(120),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(screen_pos.x - bar_w * 0.5),
                        top: px(screen_pos.y - 10.0),
                        width: px(bar_w),
                        height: px(7.0),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(2.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.05, 0.08, 0.12, 0.95)),
                    BackgroundColor(Color::srgba(0.05, 0.07, 0.10, 0.85)),
                ))
                .with_children(|p| {
                    p.spawn((
                        CourierOverheadFill { owner_slot: c.owner_slot },
                        Pickable::IGNORE,
                        Node {
                            width: percent(ratio * 100.0),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(fill_color),
                    ));
                });
        }
    }

    for (ent, bar, _) in &bar_q {
        if !active_slots.contains(&bar.owner_slot) {
            commands.entity(ent).despawn();
        }
    }
}

#[derive(Component)]
struct MinimapCourierDot {
    owner_slot: usize,
}

#[derive(Resource, Clone, PartialEq, Eq, Debug, Default)]
enum InspectedUnit {
    #[default]
    OwnHero,
    OwnCourier,
    RemoteHero(usize),
    EnemyCourier(usize),
    Creep(u64),
    Turret(usize),
    Barracks(usize),
    Ancient(usize),
}

#[derive(Resource, Default)]
struct HeroClickMoveTarget(Option<Vec3>);

#[derive(SystemParam)]
struct HudInspectParams<'w> {
    courier_ctrl: Res<'w, CourierControlState>,
    inspected: Res<'w, InspectedUnit>,
    latest_couriers: Res<'w, LatestCouriers>,
    latest_remotes: Res<'w, LatestRemotePlayers>,
    latest_creeps: Res<'w, LatestCreeps>,
    latest_turrets: Res<'w, LatestTurrets>,
    latest_barracks: Res<'w, LatestBarracks>,
    latest_ancients: Res<'w, LatestAncients>,
}

fn ray_point_distance(ray_origin: Vec3, ray_dir: Vec3, point: Vec3) -> (f32, f32) {
    let to_p = point - ray_origin;
    let t = to_p.dot(ray_dir).max(0.0);
    let closest = ray_origin + ray_dir * t;
    (closest.distance(point), t)
}

#[derive(Component)]
struct MoveClickArrow {
    center: Vec3,
    dir: Vec3,
    elapsed: f32,
    duration: f32,
}

fn spawn_green_move_arrows(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
) {
    let arrow_mesh = meshes.add(Cuboid::new(0.22, 0.08, 0.55));
    let arrow_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 1.0, 0.42),
        emissive: LinearRgba::rgb(0.30, 3.2, 0.85),
        unlit: true,
        ..default()
    });
    for i in 0..3 {
        let angle = (i as f32) * (std::f32::consts::TAU / 3.0);
        let dir = Vec3::new(angle.cos(), 0.0, angle.sin());
        let start_pos = center + dir * 1.15 + Vec3::Y * 0.16;
        let mut tf = Transform::from_translation(start_pos);
        tf.look_at(center + Vec3::Y * 0.16, Vec3::Y);
        commands.spawn((
            MoveClickArrow {
                center: center + Vec3::Y * 0.16,
                dir,
                elapsed: 0.0,
                duration: 0.42,
            },
            Mesh3d(arrow_mesh.clone()),
            MeshMaterial3d(arrow_mat.clone()),
            tf,
        ));
    }
}

fn animate_move_click_arrows_system(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut MoveClickArrow, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (ent, mut arrow, mut tf) in &mut q {
        arrow.elapsed += dt;
        let p = (arrow.elapsed / arrow.duration).clamp(0.0, 1.0);
        if p >= 1.0 {
            commands.entity(ent).despawn();
            continue;
        }
        let dist = 1.15 * (1.0 - p);
        tf.translation = arrow.center + arrow.dir * dist;
        tf.scale = Vec3::splat(1.0 - p * 0.65);
    }
}

#[derive(Component)]
struct AllyHoverRing;

#[allow(clippy::too_many_arguments)]
fn hover_ally_highlight_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    hero_hud: Res<LatestHeroSnapshot>,
    latest_couriers: Res<LatestCouriers>,
    latest_remotes: Res<LatestRemotePlayers>,
    latest_creeps: Res<LatestCreeps>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut ring_q: Query<(&mut Transform, &mut Visibility), With<AllyHoverRing>>,
) {
    if !match_flow.is_in_game() {
        for (_, mut v) in &mut ring_q {
            *v = Visibility::Hidden;
        }
        return;
    }
    let my_team = hero_hud.0.team;
    let hovered_pos: Option<(Vec3, f32)> = (|| {
        let window = window_q.single().ok()?;
        let cursor_pos = window.cursor_position()?;
        let (camera, cam_gt) = cam_q.single().ok()?;
        let ray = camera.viewport_to_world(cam_gt, cursor_pos).ok()?;
        let rd = *ray.direction;

        for c in &latest_couriers.0 {
            if c.is_visible && !c.is_dead && c.team == my_team && ray_point_distance(ray.origin, rd, c.pos).0 <= 2.2 {
                return Some((Vec3::new(c.pos.x, c.pos.y - 0.3, c.pos.z), 1.3));
            }
        }
        for rp in &latest_remotes.0 {
            if rp.is_visible && !rp.is_dead && rp.team == my_team && ray_point_distance(ray.origin, rd, rp.pos + Vec3::Y * 1.0).0 <= 1.25 {
                return Some((rp.pos + Vec3::Y * 0.12, 1.1));
            }
        }
        for cr in &latest_creeps.0 {
            if cr.is_visible && !cr.is_dead && cr.team == my_team && ray_point_distance(ray.origin, rd, cr.pos + Vec3::Y * 0.8).0 <= 1.15 {
                return Some((cr.pos + Vec3::Y * 0.10, 0.9));
            }
        }
        for td in &config.map.turrets {
            if td.team == my_team && ray_point_distance(ray.origin, rd, td.model_transform.location_vec() + Vec3::Y * 2.5).0 <= 3.2 {
                return Some((td.model_transform.location_vec() + Vec3::Y * 0.15, 2.8));
            }
        }
        for bd in &config.map.barracks {
            if bd.team == my_team && ray_point_distance(ray.origin, rd, bd.model_transform.location_vec() + Vec3::Y * 1.8).0 <= 3.5 {
                return Some((bd.model_transform.location_vec() + Vec3::Y * 0.15, 3.2));
            }
        }
        for ad in &config.map.ancients {
            if ad.team == my_team && ray_point_distance(ray.origin, rd, ad.model_transform.location_vec() + Vec3::Y * 2.5).0 <= 4.5 {
                return Some((ad.model_transform.location_vec() + Vec3::Y * 0.15, 4.2));
            }
        }
        None
    })();

    if let Ok((mut tf, mut vis)) = ring_q.single_mut() {
        if let Some((pos, scale)) = hovered_pos {
            tf.translation = pos;
            tf.scale = Vec3::new(scale, 1.0, scale);
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    } else {
        let mesh = meshes.add(Torus::new(0.85, 1.0));
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.95, 0.72),
            emissive: LinearRgba::rgb(0.35, 2.4, 1.6),
            unlit: true,
            ..default()
        });
        commands.spawn((
            AllyHoverRing,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
}

#[derive(SystemParam)]
struct ManualMoveQueries<'w> {
    latest_couriers: Res<'w, LatestCouriers>,
    latest_remotes: Res<'w, LatestRemotePlayers>,
    latest_creeps: Res<'w, LatestCreeps>,
    latest_turrets: Res<'w, LatestTurrets>,
    latest_barracks: Res<'w, LatestBarracks>,
    latest_ancients: Res<'w, LatestAncients>,
}

#[derive(SystemParam)]
struct ManualMoveUiGuards<'w> {
    shop_ui: Res<'w, ShopUiState>,
    settings_ui: Res<'w, SettingsUiState>,
    chat_state: Res<'w, ChatUiState>,
    camera_control: Res<'w, CameraControlState>,
    render_state: Res<'w, ClientRenderState>,
    physics_world: Res<'w, CustomPhysicsWorld>,
}

#[allow(clippy::too_many_arguments)]
fn courier_manual_move_input_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<GameConfig>,
    match_flow: Res<ClientMatchFlow>,
    guards: ManualMoveUiGuards,
    q: ManualMoveQueries,
    mut courier_ctrl: ResMut<CourierControlState>,
    mut inspected: ResMut<InspectedUnit>,
    mut hero_move_target: ResMut<HeroClickMoveTarget>,
    mouse: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
) {
    if !match_flow.is_in_game()
        || guards.shop_ui.is_open
        || guards.settings_ui.is_open
        || guards.chat_state.is_open
        || guards.camera_control.is_pointer_over_minimap
        || guards.camera_control.is_dragging_minimap
    {
        return;
    }
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Ok(window) = window_q.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok((camera, cam_gt)) = cam_q.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(cam_gt, cursor_pos) else {
        return;
    };
    let ray_dir = *ray.direction;
    let my_slot = match_flow.your_slot;

    for c in &q.latest_couriers.0 {
        if !c.is_visible || c.is_dead {
            continue;
        }
        if ray_point_distance(ray.origin, ray_dir, c.pos).0 <= 2.2 {
            if Some(c.owner_slot) == my_slot {
                courier_ctrl.selected = true;
                *inspected = InspectedUnit::OwnCourier;
            } else {
                *inspected = InspectedUnit::EnemyCourier(c.owner_slot);
            }
            return;
        }
    }

    if !courier_ctrl.selected {
        for rp in &q.latest_remotes.0 {
            if rp.is_visible && ray_point_distance(ray.origin, ray_dir, rp.pos + Vec3::Y * 1.0).0 <= 1.25 {
                *inspected = InspectedUnit::RemoteHero(rp.slot_index);
                return;
            }
        }
        for cr in &q.latest_creeps.0 {
            if cr.is_visible && !cr.is_dead && ray_point_distance(ray.origin, ray_dir, cr.pos + Vec3::Y * 0.8).0 <= 1.15 {
                *inspected = InspectedUnit::Creep(cr.id);
                return;
            }
        }
        for (idx, td) in config.map.turrets.iter().enumerate() {
            if q.latest_turrets.0.get(idx).is_some_and(|t| !t.is_dead)
                && ray_point_distance(ray.origin, ray_dir, td.model_transform.location_vec() + Vec3::Y * 2.5).0 <= 3.2
            {
                *inspected = InspectedUnit::Turret(idx);
                return;
            }
        }
        for (idx, bd) in config.map.barracks.iter().enumerate() {
            if q.latest_barracks.0.get(idx).is_some_and(|b| !b.is_dead)
                && ray_point_distance(ray.origin, ray_dir, bd.model_transform.location_vec() + Vec3::Y * 1.8).0 <= 3.5
            {
                *inspected = InspectedUnit::Barracks(idx);
                return;
            }
        }
        for (idx, ad) in config.map.ancients.iter().enumerate() {
            if q.latest_ancients.0.get(idx).is_some_and(|a| !a.is_dead)
                && ray_point_distance(ray.origin, ray_dir, ad.model_transform.location_vec() + Vec3::Y * 2.5).0 <= 4.5
            {
                *inspected = InspectedUnit::Ancient(idx);
                return;
            }
        }
    }

    let terrain_hit = guards
        .physics_world
        .cast_ray(
            ray.origin,
            ray_dir,
            400.0,
            soldier_collision_groups(),
            |_| true,
        )
        .map(|(_, hit)| hit.point)
        .or_else(|| {
            if ray_dir.y.abs() > 1e-4 {
                let t = (guards.render_state.soldier_pos.y - ray.origin.y) / ray_dir.y;
                (t > 0.0).then(|| ray.origin + ray_dir * t)
            } else {
                None
            }
        });

    if let Some(hit_world) = terrain_hit {
        spawn_green_move_arrows(&mut commands, &mut meshes, &mut materials, hit_world);
        if courier_ctrl.selected {
            outgoing.0.push(ClientLobbyMessage::CourierMoveTo { world_pos: hit_world });
        } else {
            *inspected = InspectedUnit::OwnHero;
            hero_move_target.0 = Some(hit_world);
        }
    }
}

// -----------------------------------------------------------------------------
// پنل جدول امتیازات (Scoreboard با نگه داشتن کلید ` در سمت چپ صفحه)
// -----------------------------------------------------------------------------

#[derive(Resource, Default)]
struct LatestScoreboard(Vec<ScoreboardHeroInfo>);

#[derive(Component)]
struct ScoreboardRoot;

#[derive(Component)]
struct ScoreboardClockText;

#[derive(Component)]
struct ScoreboardTablesContainer;

#[derive(Resource, Default)]
struct ScoreboardRenderCache {
    was_open: bool,
    last_snapshot: Vec<ScoreboardHeroInfo>,
}

fn spawn_scoreboard_ui(commands: &mut Commands, config: &GameConfig) {
    commands.init_resource::<ScoreboardRenderCache>();
    let sb = &config.scoreboard;

    commands
        .spawn((
            ScoreboardRoot,
            Pickable::IGNORE,
            GlobalZIndex(220),
            Node {
                position_type: PositionType::Absolute,
                left: px(sb.panel_left_px),
                top: px(sb.panel_top_px),
                width: px(sb.panel_width_px),
                padding: UiRect::all(px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: px(10.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(10.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.42, 0.58, 0.72, 0.85)),
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.94)),
        ))
        .with_children(|root| {
            // هدر بالای پنل
            root.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(px(4.0)),
                ..default()
            })
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new("MATCH SCOREBOARD  [`]"),
                    TextFont {
                        font_size: FontSize::Px(14.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.86, 0.45)),
                ));
                hdr.spawn((
                    ScoreboardClockText,
                    Text::new("00:00"),
                    TextFont {
                        font_size: FontSize::Px(13.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.86, 0.92, 0.98)),
                ));
            });

            // کانتینر دو جدول زیر هم (Dawn و Dusk)
            root.spawn((
                ScoreboardTablesContainer,
                Pickable::IGNORE,
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(10.0),
                    ..default()
                },
            ));
        });
}

fn spawn_scoreboard_team_table(
    parent: &mut ChildSpawnerCommands,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
    asset_server: &AssetServer,
    icon_cache: &mut ItemIconCache,
    team: TeamId,
    is_ally_team: bool,
    heroes: &[&ScoreboardHeroInfo],
) {
    let sb = &config.scoreboard;
    let (team_title, accent_col, header_bg, table_border) = match team {
        TeamId::Dawn => (
            if is_ally_team { "TEAM DAWN  [ALLIES]" } else { "TEAM DAWN  [ENEMIES]" },
            Color::srgb(0.20, 0.85, 0.56),
            Color::srgba(0.05, 0.18, 0.13, 0.92),
            Color::srgba(0.20, 0.72, 0.48, 0.60),
        ),
        TeamId::Dusk => (
            if is_ally_team { "TEAM DUSK  [ALLIES]" } else { "TEAM DUSK  [ENEMIES]" },
            Color::srgb(0.92, 0.28, 0.24),
            Color::srgba(0.20, 0.06, 0.07, 0.92),
            Color::srgba(0.82, 0.26, 0.24, 0.60),
        ),
        TeamId::Neutral => (
            "NEUTRAL",
            Color::srgb(0.70, 0.75, 0.80),
            Color::srgba(0.10, 0.12, 0.16, 0.92),
            Color::srgba(0.40, 0.45, 0.52, 0.60),
        ),
    };

    parent
        .spawn((
            Pickable::IGNORE,
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(4.0),
                padding: UiRect::all(px(8.0)),
                border: UiRect::all(px(1.0)),
                border_radius: BorderRadius::all(px(8.0)),
                ..default()
            },
            BorderColor::all(table_border),
            BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.88)),
        ))
        .with_children(|table| {
            // هدر تیم + عنوان ستون‌ها
            table
                .spawn((
                    Node {
                        width: percent(100),
                        padding: UiRect::axes(px(8.0), px(5.0)),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: px(8.0),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BackgroundColor(header_bg),
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        Node {
                            width: px(185.0),
                            ..default()
                        },
                    ))
                    .with_children(|c| {
                        c.spawn((
                            Text::new(team_title),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(accent_col),
                        ));
                    });

                    hdr.spawn(Node {
                        width: px(88.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((
                            Text::new(if is_ally_team { "LVL / XP" } else { "LVL" }),
                            TextFont {
                                font_size: FontSize::Px(10.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.75, 0.82, 0.90)),
                        ));
                    });

                    hdr.spawn(Node {
                        width: px(86.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn((
                            Text::new("K / D / A"),
                            TextFont {
                                font_size: FontSize::Px(10.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.75, 0.82, 0.90)),
                        ));
                    });

                    if is_ally_team {
                        hdr.spawn(Node {
                            width: px(74.0),
                            justify_content: JustifyContent::Center,
                            ..default()
                        })
                        .with_children(|c| {
                            c.spawn((
                                Text::new("GOLD"),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.86, 0.32)),
                            ));
                        });

                        hdr.spawn(Node {
                            flex_grow: 1.0,
                            justify_content: JustifyContent::FlexStart,
                            ..default()
                        })
                        .with_children(|c| {
                            c.spawn((
                                Text::new("ITEMS"),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.75, 0.82, 0.90)),
                            ));
                        });
                    }
                });

            if heroes.is_empty() {
                table.spawn((
                    Text::new("No players in this team"),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgba(0.60, 0.66, 0.74, 0.75)),
                ));
                return;
            }

            // ردیف هر هیرو
            for h in heroes {
                let row_bg = if h.is_local_player {
                    Color::srgba(0.11, 0.16, 0.24, 0.95)
                } else {
                    Color::srgba(0.07, 0.09, 0.14, 0.86)
                };
                let row_border = if h.is_local_player {
                    Color::srgba(0.95, 0.78, 0.24, 0.72)
                } else {
                    Color::srgba(0.22, 0.28, 0.36, 0.45)
                };

                table
                    .spawn((
                        Node {
                            width: percent(100),
                            min_height: px(sb.row_height_px),
                            padding: UiRect::axes(px(8.0), px(4.0)),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(8.0),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(6.0)),
                            ..default()
                        },
                        BorderColor::all(row_border),
                        BackgroundColor(row_bg),
                    ))
                    .with_children(|row| {
                        // ستون ۱: عکس و نام هیرو
                        row.spawn(Node {
                            width: px(185.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: px(8.0),
                            ..default()
                        })
                        .with_children(|hero_col| {
                            let p_border = if h.is_disconnected {
                                Color::srgb(0.84, 0.20, 0.20)
                            } else if h.is_dead {
                                Color::srgb(0.40, 0.40, 0.44)
                            } else {
                                accent_col
                            };
                            hero_col
                                .spawn((
                                    Node {
                                        width: px(sb.portrait_width_px),
                                        height: px(sb.portrait_height_px),
                                        border: UiRect::all(px(1.5)),
                                        border_radius: BorderRadius::all(px(4.0)),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        overflow: Overflow::clip(),
                                        ..default()
                                    },
                                    BorderColor::all(p_border),
                                    BackgroundColor(Color::srgba(0.08, 0.11, 0.16, 0.95)),
                                ))
                                .with_children(|frame| {
                                    if custom_glb_exists(&h.portrait_path) {
                                        frame.spawn((
                                            ImageNode::new(asset_server.load(&h.portrait_path)),
                                            Node {
                                                width: percent(100),
                                                height: percent(100),
                                                ..default()
                                            },
                                        ));
                                    } else {
                                        frame.spawn((
                                            Text::new(hero_initials(&h.display_name)),
                                            TextFont {
                                                font_size: FontSize::Px(12.0),
                                                ..default()
                                            },
                                            TextColor(Color::WHITE),
                                        ));
                                    }
                                    if h.is_dead && h.respawn_remaining > 0.0 {
                                        frame.spawn((
                                            Node {
                                                position_type: PositionType::Absolute,
                                                width: percent(100),
                                                height: percent(100),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 0.74)),
                                        ))
                                        .with_children(|ov| {
                                            ov.spawn((
                                                Text::new(format!("{}", h.respawn_remaining.ceil() as u32)),
                                                TextFont {
                                                    font_size: FontSize::Px(12.5),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(1.0, 0.35, 0.32)),
                                            ));
                                        });
                                    }
                                    if h.is_disconnected {
                                        frame.spawn((
                                            Node {
                                                position_type: PositionType::Absolute,
                                                left: px(0.0),
                                                width: percent(100),
                                                height: px(2.5),
                                                ..default()
                                            },
                                            BackgroundColor(Color::srgb(0.98, 0.20, 0.20)),
                                        ));
                                    }
                                });

                            hero_col
                                .spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(1.0),
                                    ..default()
                                })
                                .with_children(|nm| {
                                    let name_col = if h.is_disconnected {
                                        Color::srgba(0.60, 0.62, 0.66, 0.85)
                                    } else if h.is_local_player {
                                        Color::srgb(1.0, 0.90, 0.42)
                                    } else {
                                        Color::srgb(0.94, 0.96, 1.0)
                                    };
                                    nm.spawn((
                                        Text::new(&h.display_name),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(name_col),
                                    ));
                                    if h.is_local_player || h.is_disconnected {
                                        let sub = if h.is_disconnected {
                                            "DISCONNECTED"
                                        } else {
                                            "YOU"
                                        };
                                        let sub_col = if h.is_disconnected {
                                            Color::srgb(0.96, 0.34, 0.34)
                                        } else {
                                            Color::srgb(0.45, 0.90, 1.0)
                                        };
                                        nm.spawn((
                                            Text::new(sub),
                                            TextFont {
                                                font_size: FontSize::Px(9.0),
                                                ..default()
                                            },
                                            TextColor(sub_col),
                                        ));
                                    }
                                });
                        });

                        // ستون ۲: کپسول سطح و XP (برای هم‌تیمی‌ها دقیقاً مشابه کپسول پایین سمت چپ؛ برای دشمن فقط سطح فعلی)
                        row.spawn(Node {
                            width: px(88.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(2.0),
                            ..default()
                        })
                        .with_children(|lvl_col| {
                            if let Some(ref xp) = h.xp_info {
                                let ring_box = sb.level_ring_size_px;
                                let center_xy = ring_box * 0.5;
                                let arc_r = ring_box * 0.42;
                                let seg_size = (ring_box * 0.085).max(3.0);
                                let inner_d = ring_box * 0.62;
                                let seg_count = sb.xp_ring_segments;
                                let xp_frac = if h.level >= 25 || xp.xp_for_next_level == 0 {
                                    1.0
                                } else {
                                    (xp.xp_into_level as f32 / xp.xp_for_next_level as f32).clamp(0.0, 1.0)
                                };
                                let lit_count = (xp_frac * seg_count as f32).round() as usize;
                                let lit_col = Color::srgb(1.0, 0.84, 0.24);
                                let dim_col = Color::srgba(0.20, 0.24, 0.30, 0.65);

                                lvl_col
                                    .spawn(Node {
                                        width: px(ring_box),
                                        height: px(ring_box),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    })
                                    .with_children(|ring| {
                                        for i in 0..seg_count {
                                            let angle = -std::f32::consts::FRAC_PI_2
                                                + (i as f32) * (std::f32::consts::TAU / seg_count as f32);
                                            let sx = center_xy + angle.cos() * arc_r - seg_size * 0.5;
                                            let sy = center_xy + angle.sin() * arc_r - seg_size * 0.5;
                                            ring.spawn((
                                                Node {
                                                    position_type: PositionType::Absolute,
                                                    left: px(sx),
                                                    top: px(sy),
                                                    width: px(seg_size),
                                                    height: px(seg_size),
                                                    border_radius: BorderRadius::MAX,
                                                    ..default()
                                                },
                                                BackgroundColor(if i < lit_count { lit_col } else { dim_col }),
                                            ));
                                        }

                                        ring.spawn((
                                            Node {
                                                width: px(inner_d),
                                                height: px(inner_d),
                                                justify_content: JustifyContent::Center,
                                                align_items: AlignItems::Center,
                                                border: UiRect::all(px(1.8)),
                                                border_radius: BorderRadius::MAX,
                                                ..default()
                                            },
                                            BorderColor::all(Color::srgb(0.95, 0.78, 0.24)),
                                            BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.94)),
                                        ))
                                        .with_children(|inner| {
                                            inner.spawn((
                                                Text::new(format!("{}", h.level)),
                                                TextFont {
                                                    font_size: FontSize::Px(13.0),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(1.0, 0.88, 0.36)),
                                            ));
                                        });
                                    });

                                let xp_txt = if h.level >= 25 || xp.xp_for_next_level == 0 {
                                    "MAX LVL".to_string()
                                } else {
                                    format!("XP {}/{}", xp.xp_into_level, xp.xp_for_next_level)
                                };
                                lvl_col.spawn((
                                    Text::new(xp_txt),
                                    TextFont {
                                        font_size: FontSize::Px(9.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.85, 0.78, 0.45)),
                                ));
                            } else {
                                // برای دشمن: فقط سطح فعلی هیرو (بدون حلقه و مقدار XP)
                                let badge_d = sb.level_ring_size_px * 0.68;
                                lvl_col
                                    .spawn((
                                        Node {
                                            width: px(badge_d),
                                            height: px(badge_d),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border: UiRect::all(px(1.8)),
                                            border_radius: BorderRadius::MAX,
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgba(0.72, 0.76, 0.84, 0.85)),
                                        BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.94)),
                                    ))
                                    .with_children(|inner| {
                                        inner.spawn((
                                            Text::new(format!("{}", h.level)),
                                            TextFont {
                                                font_size: FontSize::Px(13.5),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.92, 0.95, 1.0)),
                                        ));
                                    });
                            }
                        });

                        // ستون ۳: K / D / A با رنگ‌بندی مفهومی
                        row.spawn(Node {
                            width: px(86.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            column_gap: px(4.0),
                            ..default()
                        })
                        .with_children(|kda| {
                            kda.spawn((
                                Text::new(format!("{}", h.kills)),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.32, 0.96, 0.56)),
                            ));
                            kda.spawn((
                                Text::new("/"),
                                TextFont {
                                    font_size: FontSize::Px(11.5),
                                    ..default()
                                },
                                TextColor(Color::srgba(0.55, 0.62, 0.70, 0.80)),
                            ));
                            kda.spawn((
                                Text::new(format!("{}", h.deaths)),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.36, 0.36)),
                            ));
                            kda.spawn((
                                Text::new("/"),
                                TextFont {
                                    font_size: FontSize::Px(11.5),
                                    ..default()
                                },
                                TextColor(Color::srgba(0.55, 0.62, 0.70, 0.80)),
                            ));
                            kda.spawn((
                                Text::new(format!("{}", h.assists)),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.45, 0.86, 1.0)),
                            ));
                        });

                        // ستون ۴ و ۵: گلد و آیتم‌ها (فقط برای هم‌تیمی‌ها — سرور برای دشمن None می‌فرستد)
                        if is_ally_team {
                            let gold_val = h.gold.unwrap_or(0);
                            row.spawn((
                                Node {
                                    width: px(74.0),
                                    padding: UiRect::axes(px(7.0), px(3.0)),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    column_gap: px(5.0),
                                    border_radius: BorderRadius::all(px(5.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.24, 0.19, 0.05, 0.82)),
                            ))
                            .with_children(|g| {
                                g.spawn((
                                    Node {
                                        width: px(9.5),
                                        height: px(9.5),
                                        border_radius: BorderRadius::MAX,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(1.0, 0.82, 0.16)),
                                ));
                                g.spawn((
                                    Text::new(format!("{}", gold_val)),
                                    TextFont {
                                        font_size: FontSize::Px(12.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(1.0, 0.88, 0.35)),
                                ));
                            });

                            row.spawn(Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: px(4.0),
                                ..default()
                            })
                            .with_children(|items_row| {
                                let inv_slice = h.inventory.as_deref().unwrap_or(&[]);
                                for slot_idx in 0..sb.max_item_slots {
                                    if let Some(inv_item) = inv_slice.iter().find(|it| it.slot == slot_idx) {
                                        if let Some(item_def) = items_cfg.get_item(&inv_item.item_id) {
                                            items_row
                                                .spawn(Node {
                                                    width: px(sb.item_icon_size_px),
                                                    height: px(sb.item_icon_size_px),
                                                    justify_content: JustifyContent::Center,
                                                    align_items: AlignItems::Center,
                                                    ..default()
                                                })
                                                .with_children(|slot_node| {
                                                    spawn_item_icon_badge(
                                                        slot_node,
                                                        asset_server,
                                                        icon_cache,
                                                        item_def,
                                                        sb.item_icon_size_px,
                                                        8.5,
                                                    );
                                                    if !item_def.modes.is_empty() {
                                                        let m_idx = inv_item.active_mode.min(item_def.modes.len() - 1);
                                                        let mc = item_def.modes[m_idx].color_rgb;
                                                        slot_node.spawn((
                                                            Node {
                                                                position_type: PositionType::Absolute,
                                                                right: px(2.0),
                                                                bottom: px(2.0),
                                                                width: px(6.0),
                                                                height: px(6.0),
                                                                border_radius: BorderRadius::MAX,
                                                                ..default()
                                                            },
                                                            BackgroundColor(Color::srgb(mc.0, mc.1, mc.2)),
                                                        ));
                                                    }
                                                });
                                            continue;
                                        }
                                    }
                                    items_row.spawn((
                                        Node {
                                            width: px(sb.item_icon_size_px),
                                            height: px(sb.item_icon_size_px),
                                            border: UiRect::all(px(1.0)),
                                            border_radius: BorderRadius::all(px(5.0)),
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgba(0.24, 0.30, 0.38, 0.55)),
                                        BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.72)),
                                    ));
                                }
                            });
                        }
                    });
            }
        });
}

#[derive(SystemParam)]
struct ScoreboardUiParams<'w, 's> {
    config: Res<'w, GameConfig>,
    items_cfg: Res<'w, ItemsConfig>,
    asset_server: Res<'w, AssetServer>,
    icon_cache: ResMut<'w, ItemIconCache>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    match_flow: Res<'w, ClientMatchFlow>,
    chat_state: Res<'w, ChatUiState>,
    shop_ui: Res<'w, ShopUiState>,
    settings_ui: Res<'w, SettingsUiState>,
    hero_hud: Res<'w, LatestHeroSnapshot>,
    top_bar: Res<'w, LatestTopBarState>,
    scoreboard: Res<'w, LatestScoreboard>,
    cache: ResMut<'w, ScoreboardRenderCache>,
    root_q: Query<'w, 's, &'static mut Node, With<ScoreboardRoot>>,
    clock_q: Query<'w, 's, &'static mut Text, With<ScoreboardClockText>>,
    container_q: Query<'w, 's, (Entity, Option<&'static Children>), With<ScoreboardTablesContainer>>,
}

fn update_scoreboard_ui_system(
    mut commands: Commands,
    mut p: ScoreboardUiParams,
) {
    let Ok(mut root_node) = p.root_q.single_mut() else {
        return;
    };

    let can_open = p.match_flow.is_in_game()
        && !p.chat_state.is_open
        && !p.shop_ui.search_focused
        && p.settings_ui.listening_slot.is_none();
    let is_open = can_open && p.keyboard.pressed(KeyCode::Backquote);

    if !is_open {
        if root_node.display != Display::None {
            root_node.display = Display::None;
        }
        p.cache.was_open = false;
        return;
    }

    if root_node.display != Display::Flex {
        root_node.display = Display::Flex;
    }

    let total_secs = p.top_bar.match_elapsed_secs.max(0.0) as u32;
    if let Ok(mut clock_txt) = p.clock_q.single_mut() {
        **clock_txt = format!("{:02}:{:02}", total_secs / 60, total_secs % 60);
    }

    if p.cache.was_open && p.cache.last_snapshot == p.scoreboard.0 {
        return;
    }
    p.cache.was_open = true;
    p.cache.last_snapshot = p.scoreboard.0.clone();

    let my_team = p.hero_hud.0.team;
    let dawn_heroes: Vec<&ScoreboardHeroInfo> = p.scoreboard
        .0
        .iter()
        .filter(|h| h.team == TeamId::Dawn)
        .collect();
    let dusk_heroes: Vec<&ScoreboardHeroInfo> = p.scoreboard
        .0
        .iter()
        .filter(|h| h.team == TeamId::Dusk)
        .collect();

    if let Ok((cont_ent, ch_opt)) = p.container_q.single() {
        if let Some(ch) = ch_opt {
            for child in ch.iter() {
                commands.entity(child).despawn();
            }
        }
        commands.entity(cont_ent).with_children(|parent| {
            spawn_scoreboard_team_table(
                parent,
                &p.config,
                &p.items_cfg,
                &p.asset_server,
                &mut p.icon_cache,
                TeamId::Dawn,
                my_team == TeamId::Dawn,
                &dawn_heroes,
            );
            spawn_scoreboard_team_table(
                parent,
                &p.config,
                &p.items_cfg,
                &p.asset_server,
                &mut p.icon_cache,
                TeamId::Dusk,
                my_team == TeamId::Dusk,
                &dusk_heroes,
            );
        });
    }
}