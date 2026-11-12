use bevy::animation::AnimatedBy;
use bevy::asset::RenderAssetUsages;
use bevy::gltf::GltfMaterialName;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::math::Affine2;
use bevy::shader::ShaderRef;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::gltf::Gltf;
use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, GlobalAmbientLight, NotShadowCaster};
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::window::{PresentMode, PrimaryWindow, WindowResolution};
use moba_shared::{
    ancient_physics_bundle, center_bridge_physics_bundle, create_shared_renet_client,
    grenade_physics_bundle, ground_physics_bundle, lane_segment_physics_bundle,
    mountain_physics_bundle, obstacle_physics_bundle, orient_soldier_towards,
    particle_physics_bundle, ramp_physics_bundle, reconcile_soldier_physics_pos,
    sample_water_surface_at, shell_casing_physics_bundle, target_physics_bundle,
    terrain_platform_physics_bundle, tree_physics_bundle, turret_physics_bundle,
    ClientInputCommand, ClientPhysicsPredictionState, ModelTransformConfig, SharedFogOfWar, FOW_GRID_H, FOW_GRID_W, ColliderDisabled, GameConfig,
    GameplayFxEvent, MapObjectId, MobaSharedPhysicsPlugin, PhysicsRole, PhysicsSoldier,
    ServerWorldSnapshot, TeamId, UpperBodyAction, Velocity, CHANNEL_RELIABLE_FX,
    CHANNEL_UNRELIABLE_STATE,
};
use renet::RenetClient;
use renet_netcode::NetcodeClientTransport;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::path::Path;
use std::time::Duration;

fn main() {
    let config = GameConfig::load_from_file("config.ron");
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

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: config.window.title.clone(),
                resolution: WindowResolution::new(config.window.width, config.window.height),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(DirectionalLightShadowMap { size: 2048 })
        .insert_resource(config)
        .insert_resource(ClientNetwork { client, transport })
        .insert_resource(render_state)
        .insert_resource(ClientPhysicsPredictionState::new())
        .insert_resource(WeaponAttachmentStatus {
            attached: false,
            hand_entity: None,
        })
        .insert_resource(tuner)
        .insert_resource(CameraShake { trauma: 0.0 })
        .insert_resource(CameraControlState {
            free_look_target: None,
            is_pointer_over_minimap: false,
            is_dragging_minimap: false,
        })
        .add_plugins(MaterialPlugin::<WaterSurfaceMaterial>::default())
        .add_plugins(MobaSharedPhysicsPlugin {
            role: PhysicsRole::Client,
            tick_rate_hz: tick_hz,
            substeps,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(FixedUpdate, client_flush_input_to_server_system)
        .add_systems(
            Update,
            (
                load_gltf_and_build_graph,
                attach_split_body_animation_graphs,
                attach_weapon_to_left_hand,
                weapon_live_calibration_system,
                client_sample_input_system,
                client_receive_snapshot_and_sync_system,
                target_visual_reaction_system,
                environment_vfx_and_water_animation_system,
                camera_follow_system,
                soldier_animation_player_system,
                laser_sight_gizmo_system,
                gameplay_fx_system,
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
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            align_weapon_horizontally_system.before(TransformSystems::Propagate),
        )
        .run();
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
struct MobaCamera;

#[derive(Component)]
struct WeaponRoot;

#[derive(Component)]
struct MuzzlePoint;

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

#[derive(Component)]
struct NetworkBulletVisual {
    server_id: u64,
}

#[derive(Component)]
struct NetworkGrenadeVisual {
    server_id: u64,
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
struct SoldierGltfHandle(Handle<Gltf>);

#[derive(Resource)]
struct SoldierAnimationAssets {
    graph_handle: Handle<AnimationGraph>,
    nodes: HashMap<String, AnimationNodeIndex>,
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
}

#[derive(Resource)]
struct ClientRenderState {
    soldier_pos: Vec3,
    soldier_facing_dir: Vec3,
    aim_world_dir: Vec3,
    aim_hit_pos: Vec3,
    laser_hit_pos: Vec3,
    current_locomotion: String,
    previous_lower_anim: String,
    previous_upper_anim: String,
    upper_action: UpperBodyAction,
    traversal_active: bool,
    traversal_anim_name: String,
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
            current_locomotion: "idle".to_string(),
            previous_lower_anim: String::new(),
            previous_upper_anim: String::new(),
            upper_action: UpperBodyAction::None,
            traversal_active: false,
            traversal_anim_name: String::new(),
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

    for (seg_idx, seg) in config.map.river_segments.iter().enumerate() {
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
    for anc in &config.map.ancients {
        let bs = Vec3::new(anc.base_size.0, anc.base_size.1, anc.base_size.2);
        let c = anc.color_rgb;
        let ce = anc.core_emissive_rgb;
        let light_col = if anc.team == TeamId::Dawn {
            Color::srgb(0.18, 0.95, 0.68)
        } else {
            Color::srgb(1.0, 0.25, 0.18)
        };

        let mut anc_cmds = commands.spawn((
            ancient_physics_bundle(&config, anc),
            Visibility::Visible,
        ));

        anc_cmds.insert(VisionOccludable { is_faded: false, half_extents: Vec3::new(bs.x * 0.5, anc.core_height * 0.6, bs.z * 0.5) });
        let anc_ent = anc_cmds.id();
        if custom_glb_exists(&anc.model_path) {
            anc_cmds.with_children(|parent| {
                parent.spawn(glb_model_bundle(
                    &asset_server,
                    &anc.model_path,
                    &anc.model_transform,
                ));
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
            turret_physics_bundle(&config, tur),
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

    let gltf_handle = asset_server.load(&config.animations.model_path);
    commands.insert_resource(SoldierGltfHandle(gltf_handle));
}

// -----------------------------------------------------------------------------
// سیستم پویایی محیط، جریان آب، آبشارها، حلقه‌های موج آب و تعامل ورود بازیکن به آب
// -----------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn environment_vfx_and_water_animation_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    mut render_state: ResMut<ClientRenderState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    let dt = time.delta_secs();

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

fn load_gltf_and_build_graph(
    mut commands: Commands,
    config: Res<GameConfig>,
    gltf_handle: Option<Res<SoldierGltfHandle>>,
    gltf_assets: Res<Assets<Gltf>>,
    clip_assets: Res<Assets<AnimationClip>>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    assets_ready: Option<Res<SoldierAnimationAssets>>,
) {
    if assets_ready.is_some() {
        return;
    }

    let Some(handle) = gltf_handle else { return };
    let Some(gltf) = gltf_assets.get(&handle.0) else { return };

    let mut graph = AnimationGraph::new();
    let mut nodes = HashMap::new();

    for (name, clip_handle) in &gltf.named_animations {
        let clip = clip_assets.get(clip_handle).unwrap_or_else(|| {
            panic!(
                "خطای بحرانی: کلیپ انیمیشن '{}' هنوز در Assets<AnimationClip> بارگذاری نشده است!",
                name
            )
        });
        let node_index = graph.add_clip(clip_handle.clone(), 1.0, graph.root);
        nodes.insert(name.to_string(), node_index);
        println!(
            "انیمیشن بارگذاری شد: '{}' (مدت‌زمان GLB: {:.3}s)",
            name,
            clip.duration()
        );
    }

    for expected_anim in config.animations.clip_durations.keys() {
        assert!(
            nodes.contains_key(expected_anim),
            "خطای بحرانی: انیمیشن '{}' که در config.ron تعریف شده در فایل '{}' وجود ندارد!",
            expected_anim,
            config.animations.model_path
        );
    }

    let graph_handle = animation_graphs.add(graph);
    commands.insert_resource(SoldierAnimationAssets {
        graph_handle,
        nodes,
    });

    let spawn_pos = Vec3::new(
        config.soldier.spawn_pos.0,
        config.soldier.spawn_pos.1,
        config.soldier.spawn_pos.2,
    );
    commands.spawn((
        WorldAssetRoot(gltf.scenes[0].clone()),
        SoldierMarker,
        Transform::from_translation(spawn_pos),
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
    player_query: Query<Entity, (With<AnimationPlayer>, Without<AnimationGraphHandle>)>,
    mut target_query: Query<(Entity, &mut AnimatedBy)>,
    names_query: Query<&Name>,
    parents_query: Query<&ChildOf>,
    anim_assets: Option<Res<SoldierAnimationAssets>>,
) {
    let Some(anim_assets) = anim_assets else { return };

    for lower_player_entity in &player_query {
        commands.entity(lower_player_entity).insert((
            AnimationGraphHandle(anim_assets.graph_handle.clone()),
            AnimationTransitions::new(),
            LowerBodyAnimationPlayer,
        ));

        let upper_player_entity = commands
            .spawn((
                AnimationPlayer::default(),
                AnimationGraphHandle(anim_assets.graph_handle.clone()),
                AnimationTransitions::new(),
                UpperBodyAnimationPlayer,
            ))
            .id();

        for (bone_entity, mut animated_by) in &mut target_query {
            if animated_by.0 == lower_player_entity
                && is_upper_body_in_hierarchy(bone_entity, &names_query, &parents_query)
            {
                animated_by.0 = upper_player_entity;
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status: ResMut<WeaponAttachmentStatus>,
    tuner: Res<WeaponTuner>,
    hand_query: Query<(Entity, &Name)>,
) {
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
            let body_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.14, 0.15, 0.18),
                metallic: 0.85,
                perceptual_roughness: 0.35,
                ..default()
            });
            let accent_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.24, 0.26, 0.3),
                metallic: 0.65,
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
                        Transform {
                            translation: tuner.offset,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::splat(tuner.scale),
                        },
                        Visibility::Visible,
                    ))
                    .with_children(|weapon| {
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
    keyboard: Res<ButtonInput<KeyCode>>,
    mut tuner: ResMut<WeaponTuner>,
) {
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

fn client_sample_input_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    camera_control: Res<CameraControlState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MobaCamera>>,
    muzzle_query: Query<&GlobalTransform, With<MuzzlePoint>>,
    mut prediction: ResMut<ClientPhysicsPredictionState>,
) {
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

    let cmd = ClientInputCommand {
        move_input: move_vec,
        shift_held: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        toggle_crouch_just_pressed: keyboard.just_pressed(KeyCode::ControlLeft)
            || keyboard.just_pressed(KeyCode::ControlRight),
        reload_just_pressed: keyboard.just_pressed(KeyCode::KeyR),
        grenade_just_pressed: keyboard.just_pressed(KeyCode::KeyF),
        space_just_pressed: keyboard.just_pressed(KeyCode::Space),
        kill_toggle_just_pressed: keyboard.just_pressed(KeyCode::KeyX),
        fire_just_pressed: !camera_control.is_pointer_over_minimap
            && !camera_control.is_dragging_minimap
            && mouse_button.just_pressed(MouseButton::Left),
        fire_held: !camera_control.is_pointer_over_minimap
            && !camera_control.is_dragging_minimap
            && mouse_button.pressed(MouseButton::Left),
        aim_ray_origin,
        aim_ray_dir,
        has_aim_ray,
        client_muzzle_world_pos,
    };

    if let Some(ref mut existing) = prediction.latest_input {
        existing.accumulate_from(cmd);
    } else {
        prediction.latest_input = Some(cmd);
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn client_receive_snapshot_and_sync_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    mut net: ResMut<ClientNetwork>,
    mut render_state: ResMut<ClientRenderState>,
    mut prediction: ResMut<ClientPhysicsPredictionState>,
    mut shake: ResMut<CameraShake>,
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
        (Entity, &NetworkBulletVisual, &mut Transform),
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
        render_state.current_locomotion = snap.current_locomotion;
        render_state.upper_action = snap.upper_action;
        render_state.traversal_active = snap.traversal_active;
        render_state.traversal_anim_name = snap.traversal_anim_name;
        render_state.is_dead = snap.is_dead;
        render_state.death_anim_name = snap.death_anim_name;
        render_state.in_water = snap.in_water;
        if let Some(ref mut fow_state) = fow {
            fow_state.shared.apply_visible_mask(&snap.fog_visible_mask);
        }

        prediction.traversal_active = snap.traversal_active;
        prediction.is_dead = snap.is_dead;

        if let Ok(mut soldier_phys_tf) = soldier_physics_query.single_mut() {
            reconcile_soldier_physics_pos(&mut soldier_phys_tf, snap.soldier_pos, &config);
        }

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

        for (entity, vis, mut tf) in &mut bullet_query {
            if let Some(b_snap) = bullet_map.get(&vis.server_id) {
                let dir = b_snap.velocity.normalize_or_zero();
                tf.translation = b_snap.translation;
                tf.rotation = Quat::from_rotation_arc(Vec3::Y, dir);
                seen_bullets.insert(vis.server_id);
            } else {
                commands.entity(entity).despawn();
            }
        }

        for b_snap in &snap.bullets {
            if !seen_bullets.contains(&b_snap.id) {
                let dir = b_snap.velocity.normalize_or_zero();
                let rot = Quat::from_rotation_arc(Vec3::Y, dir);
                commands.spawn((
                    Mesh3d(meshes.add(Cone {
                        radius: 0.028,
                        height: 0.42,
                    })),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(1.0, 0.92, 0.45),
                        emissive: LinearRgba::new(14.0, 8.5, 1.2, 1.0),
                        unlit: true,
                        ..default()
                    })),
                    Transform {
                        translation: b_snap.translation,
                        rotation: rot,
                        scale: Vec3::ONE,
                    },
                    NetworkBulletVisual {
                        server_id: b_snap.id,
                    },
                ));
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
                commands.spawn((
                    Mesh3d(meshes.add(Sphere::new(config.physics.grenade_collider_radius))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.2, 0.38, 0.18),
                        metallic: 0.6,
                        perceptual_roughness: 0.4,
                        ..default()
                    })),
                    NetworkGrenadeVisual {
                        server_id: g_snap.id,
                    },
                    grenade_physics_bundle(
                        &config,
                        g_snap.id,
                        g_snap.translation,
                        g_snap.rotation,
                        g_snap.linear_velocity,
                        g_snap.angular_velocity,
                    ),
                ));
            }
        }
    }

    for fx in all_fx_events {
        spawn_fx_from_server_event(
            &mut commands,
            &config,
            &mut render_state,
            &mut shake,
            &mut meshes,
            &mut materials,
            fx,
        );
    }

    if let (Ok(soldier_phys_tf), Ok(mut soldier_vis_tf)) = (
        soldier_physics_query.single(),
        soldier_visual_query.single_mut(),
    ) {
        let target_feet_pos = Vec3::new(
            soldier_phys_tf.translation.x,
            soldier_phys_tf.translation.y - config.physics.soldier_capsule_center_y,
            soldier_phys_tf.translation.z,
        );
        let smooth_y = soldier_vis_tf
            .translation
            .y
            .lerp(target_feet_pos.y, config.physics.position_reconcile_lerp_factor);
        let smoothed_pos = Vec3::new(target_feet_pos.x, smooth_y, target_feet_pos.z);
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
    event: GameplayFxEvent,
) {
    match event {
        GameplayFxEvent::ShotFired {
            muzzle_pos,
            shot_dir,
            trauma_add,
            trauma_max,
        } => {
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
                PointLight {
                    color: Color::srgb(1.0, 0.78, 0.25),
                    intensity: 85_000.0,
                    range: 5.5,
                    shadow_maps_enabled: false,
                    ..default()
                },
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
            trauma_add,
            trauma_max,
        } => {
            let fire_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.55, 0.1),
                emissive: LinearRgba::new(16.0, 6.0, 0.8, 1.0),
                unlit: true,
                ..default()
            });

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

fn laser_sight_gizmo_system(
    mut gizmos: Gizmos,
    config: Res<GameConfig>,
    muzzle_query: Query<&GlobalTransform, With<MuzzlePoint>>,
    trail_query: Query<&BulletTrail>,
    render_state: Res<ClientRenderState>,
) {
    for trail in &trail_query {
        let alpha = 1.0 - trail.timer.fraction();
        let dir = (trail.end - trail.start).normalize_or_zero();
        let right = dir.cross(Vec3::Y).normalize_or_zero() * 0.012;

        let core_color = Color::srgba(1.0, 0.92, 0.45, alpha);
        let glow_color = Color::srgba(1.0, 0.55, 0.1, alpha * 0.65);

        gizmos.line(trail.start, trail.end, core_color);
        gizmos.line(trail.start + right, trail.end + right, glow_color);
        gizmos.line(trail.start - right, trail.end - right, glow_color);
    }

    if render_state.is_dead
        || render_state.traversal_active
        || render_state.current_locomotion == "swimming"
    {
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
    let laser_red = Color::srgb(1.0, 0.05, 0.05);
    let laser_core = Color::srgb(1.0, 0.35, 0.35);

    gizmos.line(muzzle_pos, laser_end, laser_red);
    gizmos.line(
        muzzle_pos + Vec3::Y * 0.004,
        laser_end + Vec3::Y * 0.004,
        laser_core,
    );

    gizmos.sphere(
        Isometry3d::from_translation(laser_end),
        0.045,
        Color::srgb(1.0, 0.1, 0.1),
    );

    let r = 0.28;
    let gap = 0.10;
    gizmos.line(laser_end + Vec3::X * gap, laser_end + Vec3::X * r, laser_red);
    gizmos.line(laser_end - Vec3::X * gap, laser_end - Vec3::X * r, laser_red);
    gizmos.line(laser_end + Vec3::Z * gap, laser_end + Vec3::Z * r, laser_red);
    gizmos.line(laser_end - Vec3::Z * gap, laser_end - Vec3::Z * r, laser_red);
}

fn gameplay_fx_system(
    mut commands: Commands,
    time: Res<Time>,
    mut flash_query: Query<(Entity, &mut MuzzleFlash)>,
    mut trail_query: Query<(Entity, &mut BulletTrail)>,
) {
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

fn camera_follow_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut shake: ResMut<CameraShake>,
    mut render_state: ResMut<ClientRenderState>,
    camera_control: Res<CameraControlState>,
    soldier_query: Query<&Transform, (With<SoldierMarker>, Without<MobaCamera>)>,
    mut camera_query: Query<&mut Transform, With<MobaCamera>>,
) {
    let Ok(soldier_tf) = soldier_query.single() else { return };
    let Ok(mut camera_tf) = camera_query.single_mut() else { return };

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

    let camera_offset = Vec3::new(
        config.camera.offset.0,
        config.camera.offset.1,
        config.camera.offset.2,
    );
    let focus_pos = camera_control
        .free_look_target
        .unwrap_or(soldier_tf.translation);
    let target_pos = focus_pos + camera_offset + shake_offset;

    camera_tf.translation = camera_tf
        .translation
        .lerp(target_pos, config.camera.follow_lerp_speed * dt);
    camera_tf.look_at(
        focus_pos + shake_offset * config.camera.look_at_shake_factor,
        Vec3::Y,
    );
}

fn soldier_animation_player_system(
    config: Res<GameConfig>,
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
            lower_trans.play(&mut lower_player, node, blend).replay();
            upper_trans.play(&mut upper_player, node, blend).replay();
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

    if render_state.current_locomotion != render_state.previous_lower_anim {
        lower_trans
            .play(&mut lower_player, lower_node, loco_blend)
            .repeat();
        render_state.previous_lower_anim = render_state.current_locomotion.clone();
    }

    match render_state.upper_action {
        UpperBodyAction::TossGrenade => {
            if render_state.previous_upper_anim != "toss_grenade" {
                let node = anim_assets.require_node("toss_grenade");
                upper_trans
                    .play(
                        &mut upper_player,
                        node,
                        Duration::from_millis(config.animations.grenade_blend_ms),
                    )
                    .replay();
                render_state.previous_upper_anim = "toss_grenade".to_string();
            }
        }
        UpperBodyAction::Reloading => {
            if render_state.previous_upper_anim != "reloading" {
                let node = anim_assets.require_node("reloading");
                upper_trans
                    .play(
                        &mut upper_player,
                        node,
                        Duration::from_millis(config.animations.reload_blend_ms),
                    )
                    .replay();
                render_state.previous_upper_anim = "reloading".to_string();
            }
        }
        UpperBodyAction::FiringRifle => {
            if render_state.previous_upper_anim != "firing_rifle" {
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
            if render_state.previous_upper_anim != render_state.current_locomotion {
                let lower_seek = lower_player
                    .animation(lower_node)
                    .expect("خطای بحرانی: انیمیشن پایین‌تنه فعال نیست!")
                    .seek_time();

                let active = upper_trans
                    .play(&mut upper_player, lower_node, loco_blend)
                    .repeat();
                active.seek_to(lower_seek);
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
    config: Res<GameConfig>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    render_state: Res<ClientRenderState>,
    mut camera_control: ResMut<CameraControlState>,
) {
    let wasd_pressed = keyboard.pressed(KeyCode::KeyW)
        || keyboard.pressed(KeyCode::KeyA)
        || keyboard.pressed(KeyCode::KeyS)
        || keyboard.pressed(KeyCode::KeyD);

    if keyboard.just_pressed(KeyCode::KeyY)
        || keyboard.just_pressed(KeyCode::Home)
        || (wasd_pressed && !camera_control.is_dragging_minimap)
    {
        camera_control.free_look_target = None;
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

    if inside && mouse_button.just_pressed(MouseButton::Left) {
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
}

fn minimap_overlay_update_system(
    config: Res<GameConfig>,
    render_state: Res<ClientRenderState>,
    camera_control: Res<CameraControlState>,
    soldier_query: Query<&Transform, With<SoldierMarker>>,
    target_query: Query<&VisualTarget>,
    mut soldier_dot_query: Query<
        &mut Node,
        (
            With<MinimapSoldierDot>,
            Without<MinimapTargetDot>,
            Without<MinimapCameraBox>,
        ),
    >,
    mut target_dot_query: Query<
        (&MinimapTargetDot, &mut Node, &mut Visibility),
        (Without<MinimapSoldierDot>, Without<MinimapCameraBox>),
    >,
    mut cam_box_query: Query<
        &mut Node,
        (
            With<MinimapCameraBox>,
            Without<MinimapSoldierDot>,
            Without<MinimapTargetDot>,
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
    let hero_team = config
        .heroes
        .first()
        .map(|h| h.team)
        .unwrap_or(TeamId::Dawn);
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