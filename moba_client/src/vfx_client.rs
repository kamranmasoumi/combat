use bevy::light::NotShadowCaster;
use bevy::ecs::system::SystemParam;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy_hanabi::prelude::{
    AccelModifier, Attribute, ColorOverLifetimeModifier, EffectAsset, ExprWriter, HanabiPlugin,
    LinearDragModifier, OrientMode, OrientModifier, ParticleEffect, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SizeOverLifetimeModifier,
    SpawnerSettings,
};
use moba_shared::{particle_physics_bundle, BulletVfxSource, GameConfig, ImpactSurface, WeaponKind, WeaponVfxProfile, WeatherKind};
use std::collections::HashMap;

// -----------------------------------------------------------------------------
// شیدرهای اختصاصی WESL: آب، تیرهای منحصر‌به‌فرد هیروها، خط تیر، شعله و موج شوک
// -----------------------------------------------------------------------------

#[derive(ShaderType, Debug, Clone)]
pub struct WaterUniforms {
    pub base_color: Vec4,
    pub foam_color: Vec4,
    pub flow_and_params: Vec4,
    pub camera_pos: Vec4,
    pub player_ripple: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct WaterSurfaceMaterial {
    #[uniform(0)]
    pub uniforms: WaterUniforms,
    #[texture(1)]
    #[sampler(2)]
    pub foam_tex: Handle<Image>,
}

impl Material for WaterSurfaceMaterial {
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

#[derive(ShaderType, Debug, Clone)]
pub struct BulletShaderUniforms {
    pub core_color: Vec4,
    pub edge_color: Vec4,
    pub params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct VanguardBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for VanguardBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_vanguard_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct FrostBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for FrostBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_frost_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct IronBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for IronBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_iron_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct PyroBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for PyroBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_pyro_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct VenomBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for VenomBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_venom_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct ElectricBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for ElectricBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_electric_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Debug, Clone)]
pub struct FlameBeamUniforms {
    pub core_color: Vec4,
    pub edge_color: Vec4,
    pub camera_pos: Vec4,
    pub params: Vec4,
}

#[derive(ShaderType, Debug, Clone)]
pub struct ToxicPoolUniforms {
    pub core_color: Vec4,
    pub edge_color: Vec4,
    pub params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct ToxicPoolMaterial {
    #[uniform(0)]
    pub uniforms: ToxicPoolUniforms,
}

impl Material for ToxicPoolMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_toxic_pool.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct FlameBeamMaterial {
    #[uniform(0)]
    pub uniforms: FlameBeamUniforms,
}

impl Material for FlameBeamMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_flame_beam.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct FlameBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for FlameBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_flame_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct BladeBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for BladeBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_blade_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct PistolBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for PistolBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_pistol_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct KineticBulletMaterial {
    #[uniform(0)]
    pub uniforms: BulletShaderUniforms,
}

impl Material for KineticBulletMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_kinetic_bullet.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Debug, Clone)]
struct BulletTrailUniforms {
    core_color: Vec4,
    wave_color: Vec4,
    params: Vec4,
    style_params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct BulletTrailMaterial {
    #[uniform(0)]
    uniforms: BulletTrailUniforms,
}

impl Material for BulletTrailMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_bullet_trail.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Debug, Clone)]
struct MuzzleFlashUniforms {
    core_color: Vec4,
    edge_color: Vec4,
    params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct MuzzleFlashMaterial {
    #[uniform(0)]
    uniforms: MuzzleFlashUniforms,
}

impl Material for MuzzleFlashMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_muzzle_flash.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Debug, Clone)]
struct ShockwaveUniforms {
    core_color: Vec4,
    edge_color: Vec4,
    params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct ShockwaveRingMaterial {
    #[uniform(0)]
    uniforms: ShockwaveUniforms,
}

impl Material for ShockwaveRingMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_shockwave_ring.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Debug, Clone)]
struct BloodSplatterUniforms {
    core_color: Vec4,
    edge_color: Vec4,
    params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct BloodSplatterMaterial {
    #[uniform(0)]
    uniforms: BloodSplatterUniforms,
}

impl Material for BloodSplatterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_blood_splatter.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(ShaderType, Debug, Clone)]
struct ExplosionFireballUniforms {
    core_color: Vec4,
    edge_color: Vec4,
    params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct ExplosionFireballMaterial {
    #[uniform(0)]
    uniforms: ExplosionFireballUniforms,
}

impl Material for ExplosionFireballMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_explosion_fireball.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(ShaderType, Debug, Clone)]
struct WaterExplosionUniforms {
    core_color: Vec4,
    foam_color: Vec4,
    params: Vec4,
}

#[derive(ShaderType, Debug, Clone)]
pub struct TeleportPortalUniforms {
    pub core_color: Vec4,
    pub edge_color: Vec4,
    pub params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct TeleportPortalMaterial {
    #[uniform(0)]
    pub uniforms: TeleportPortalUniforms,
}

impl Material for TeleportPortalMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_teleport_portal.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct WaterExplosionMaterial {
    #[uniform(0)]
    uniforms: WaterExplosionUniforms,
}

impl Material for WaterExplosionMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_water_explosion.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

// -----------------------------------------------------------------------------
// رجیستری متریال و مش تیرهای اختصاصی هر هیرو و سلاح از روی config.ron
// -----------------------------------------------------------------------------

#[derive(Clone)]
pub enum BulletMaterialHandle {
    Vanguard(Handle<VanguardBulletMaterial>),
    Frost(Handle<FrostBulletMaterial>),
    Iron(Handle<IronBulletMaterial>),
    Pyro(Handle<PyroBulletMaterial>),
    Venom(Handle<VenomBulletMaterial>),
    Electric(Handle<ElectricBulletMaterial>),
    Flame(Handle<FlameBulletMaterial>),
    Blade(Handle<BladeBulletMaterial>),
    Pistol(Handle<PistolBulletMaterial>),
    Kinetic(Handle<KineticBulletMaterial>),
}

#[derive(Clone)]
pub struct CachedBulletVisualAsset {
    pub mesh: Handle<Mesh>,
    pub material: BulletMaterialHandle,
}

#[derive(Resource, Default)]
pub struct BulletVfxRegistry {
    pub entries: HashMap<BulletVfxSource, CachedBulletVisualAsset>,
}

impl BulletVfxRegistry {
    pub fn spawn_bullet_entity(
        &self,
        commands: &mut Commands,
        source: &BulletVfxSource,
        transform: Transform,
        marker: impl Component,
    ) -> Entity {
        let asset = self.entries.get(source).unwrap_or_else(|| {
            panic!(
                "خطای بحرانی VFX: پروفایل تیر برای منبع '{:?}' در BulletVfxRegistry یافت نشد!",
                source
            )
        });
        let mut ent = commands.spawn((
            Mesh3d(asset.mesh.clone()),
            transform,
            Visibility::Visible,
            NotShadowCaster,
            marker,
        ));
        match &asset.material {
            BulletMaterialHandle::Vanguard(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Frost(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Iron(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Pyro(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Venom(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Electric(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Flame(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Blade(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Pistol(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
            BulletMaterialHandle::Kinetic(h) => {
                ent.insert(MeshMaterial3d(h.clone()));
            }
        }
        ent.id()
    }
}

// -----------------------------------------------------------------------------
// رویدادها و منابع GPU VFX (Hanabi + WESL)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum ClientVfxEvent {
    MuzzleSmoke {
        pos: Vec3,
        dir: Vec3,
        vfx_source: BulletVfxSource,
    },
    MagneticTrail {
        start: Vec3,
        end: Vec3,
        vfx_source: BulletVfxSource,
    },
    BulletImpact {
        pos: Vec3,
        bullet_dir: Vec3,
        surface: ImpactSurface,
        vfx_source: BulletVfxSource,
    },
    GrenadeExploded {
        pos: Vec3,
        aoe_radius: f32,
        in_water: bool,
        water_y: f32,
        weapon_source: Option<BulletVfxSource>,
    },
    SpawnPondAmbient {
        pos: Vec3,
    },
    SpawnWaterfallSpray {
        pos: Vec3,
    },
    WaterSplash {
        pos: Vec3,
        intensity: f32,
    },
    FlameStreamStep {
        start: Vec3,
        end: Vec3,
        hit_any: bool,
    },
}

#[derive(Resource, Default)]
pub struct VfxEventQueue(pub Vec<ClientVfxEvent>);

#[derive(Resource, Default)]
struct VfxMaterialPool {
    free_trail_mats: Vec<Handle<BulletTrailMaterial>>,
    free_flash_mats: Vec<Handle<MuzzleFlashMaterial>>,
    free_ring_mats: Vec<Handle<ShockwaveRingMaterial>>,
    free_blood_mats: Vec<Handle<BloodSplatterMaterial>>,
}

const MAX_POOLED_TRAIL_MATS: usize = 72;
const MAX_POOLED_FLASH_MATS: usize = 32;
const MAX_POOLED_RING_MATS: usize = 32;
const MAX_POOLED_BLOOD_MATS: usize = 48;
const MAX_ACTIVE_TRAILS: usize = 72;
const MAX_ACTIVE_RINGS: usize = 24;
const MAX_ACTIVE_HANABI: usize = 36;
const MAX_ACTIVE_BLOOD_DECALS: usize = 40;

#[derive(Resource)]
struct HanabiEffectHandles {
    muzzle_smoke: Handle<EffectAsset>,
    blood_burst: Handle<EffectAsset>,
    stone_dust: Handle<EffectAsset>,
    metal_sparks: Handle<EffectAsset>,
    water_splash: Handle<EffectAsset>,
    pond_ambient_bubbles: Handle<EffectAsset>,
    waterfall_dense_spray: Handle<EffectAsset>,
    explosion_sparks: Handle<EffectAsset>,
    explosion_smoke: Handle<EffectAsset>,
    water_explosion_spray: Handle<EffectAsset>,
    flame_beam_embers: Handle<EffectAsset>,
    flame_beam_smoke: Handle<EffectAsset>,
    toxic_splash_sparks: Handle<EffectAsset>,
    trail_mesh: Handle<Mesh>,
    muzzle_flash_mesh: Handle<Mesh>,
    shockwave_ring_mesh: Handle<Mesh>,
    blood_decal_mesh: Handle<Mesh>,
    blood_air_droplet_mesh: Handle<Mesh>,
    blood_air_droplet_mat: Handle<StandardMaterial>,
    fireball_sphere_mesh: Handle<Mesh>,
    water_geyser_mesh: Handle<Mesh>,
}

#[derive(Component)]
struct OneShotVfxDespawn {
    timer: Timer,
}

#[derive(Component)]
struct OneShotLightFade {
    timer: Timer,
    initial_intensity: f32,
}

#[derive(Component)]
struct BulletTrailInstance {
    timer: Timer,
    mat_handle: Handle<BulletTrailMaterial>,
}

#[derive(Component)]
struct MuzzleFlashInstance {
    timer: Timer,
    mat_handle: Handle<MuzzleFlashMaterial>,
}

#[derive(Component)]
struct ShockwaveRingInstance {
    timer: Timer,
    start_scale: f32,
    end_scale: f32,
    mat_handle: Handle<ShockwaveRingMaterial>,
}

#[derive(Component)]
struct BloodGroundDecal {
    timer: Timer,
    mat_handle: Handle<BloodSplatterMaterial>,
}

#[derive(Component)]
struct ExplosionFireballInstance {
    timer: Timer,
    start_scale: f32,
    end_scale: f32,
    mat_handle: Handle<ExplosionFireballMaterial>,
}

#[derive(Component)]
struct WaterExplosionInstance {
    timer: Timer,
    base_y: f32,
    max_height: f32,
    radius_scale: f32,
    mat_handle: Handle<WaterExplosionMaterial>,
}

pub struct MobaVfxPlugin;

impl Plugin for MobaVfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_plugins(MaterialPlugin::<WaterSurfaceMaterial>::default())
            .add_plugins(MaterialPlugin::<VanguardBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<FrostBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<IronBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<PyroBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<VenomBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<ElectricBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<FlameBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<FlameBeamMaterial>::default())
            .add_plugins(MaterialPlugin::<ToxicPoolMaterial>::default())
            .add_plugins(MaterialPlugin::<BladeBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<PistolBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<KineticBulletMaterial>::default())
            .add_plugins(MaterialPlugin::<BulletTrailMaterial>::default())
            .add_plugins(MaterialPlugin::<MuzzleFlashMaterial>::default())
            .add_plugins(MaterialPlugin::<ShockwaveRingMaterial>::default())
            .add_plugins(MaterialPlugin::<BloodSplatterMaterial>::default())
            .add_plugins(MaterialPlugin::<ExplosionFireballMaterial>::default())
            .add_plugins(MaterialPlugin::<WaterExplosionMaterial>::default())
            .add_plugins(MaterialPlugin::<TeleportPortalMaterial>::default())
            .add_plugins(MaterialPlugin::<WeatherRainMaterial>::default())
            .add_plugins(MaterialPlugin::<WeatherSnowMaterial>::default())
            .add_plugins(MaterialPlugin::<TacticalAuraMaterial>::default())
            .add_plugins(MaterialPlugin::<AutumnLeafMaterial>::default())
            .add_plugins(MaterialPlugin::<ExtendedMaterial<StandardMaterial, SnowAccumulationExtension>>::default())
            .init_resource::<VfxEventQueue>()
            .init_resource::<TacticalAuraSyncState>()
            .init_resource::<BiomeWalkersState>()
            .init_resource::<SnowAccumulationState>()
            .init_resource::<VfxMaterialPool>()
            .init_resource::<BulletVfxRegistry>()
            .init_resource::<WeatherVfxState>()
            .add_systems(
                Startup,
                (
                    init_hanabi_vfx_assets_system,
                    init_bullet_vfx_registry_system,
                    init_river_biome_foliage_system,
                ),
            )
            .add_systems(
                OnExit(crate::pregame_ui::AppScreenState::InGame),
                cleanup_ingame_vfx_on_exit_system,
            )
            .add_systems(
                Update,
                (
                    process_vfx_events_system,
                    update_vfx_lifetimes_system,
                    update_weather_vfx_system,
                    update_snow_accumulation_system,
                    update_tactical_auras_vfx_system,
                    update_autumn_leaves_scatter_system,
                )
                    .chain(),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn register_bullet_profile(
    registry: &mut BulletVfxRegistry,
    source: BulletVfxSource,
    profile: &WeaponVfxProfile,
    meshes: &mut Assets<Mesh>,
    vanguard_mats: &mut Assets<VanguardBulletMaterial>,
    frost_mats: &mut Assets<FrostBulletMaterial>,
    iron_mats: &mut Assets<IronBulletMaterial>,
    pyro_mats: &mut Assets<PyroBulletMaterial>,
    venom_mats: &mut Assets<VenomBulletMaterial>,
    electric_mats: &mut Assets<ElectricBulletMaterial>,
    flame_mats: &mut Assets<FlameBulletMaterial>,
    blade_mats: &mut Assets<BladeBulletMaterial>,
    pistol_mats: &mut Assets<PistolBulletMaterial>,
    kinetic_mats: &mut Assets<KineticBulletMaterial>,
) {
    let uniforms = BulletShaderUniforms {
        core_color: Vec4::new(
            profile.core_color_rgb.0,
            profile.core_color_rgb.1,
            profile.core_color_rgb.2,
            1.0,
        ),
        edge_color: Vec4::new(
            profile.edge_color_rgb.0,
            profile.edge_color_rgb.1,
            profile.edge_color_rgb.2,
            1.0,
        ),
        params: Vec4::new(0.0, profile.bloom_intensity, profile.pulse_speed, 0.0),
    };

    let mesh = if matches!(
        source,
        BulletVfxSource::Hero {
            weapon_kind: moba_shared::WeaponKind::SpinningBlade,
            ..
        }
    ) {
        meshes.add(Cylinder::new(
            profile.projectile_radius,
            profile.projectile_length,
        ))
    } else {
        meshes.add(Capsule3d::new(
            profile.projectile_radius,
            profile.projectile_length,
        ))
    };

    let material = match profile.shader_path.as_str() {
        "shaders/vfx_vanguard_bullet.wesl" => {
            BulletMaterialHandle::Vanguard(vanguard_mats.add(VanguardBulletMaterial { uniforms }))
        }
        "shaders/vfx_frost_bullet.wesl" => {
            BulletMaterialHandle::Frost(frost_mats.add(FrostBulletMaterial { uniforms }))
        }
        "shaders/vfx_iron_bullet.wesl" => {
            BulletMaterialHandle::Iron(iron_mats.add(IronBulletMaterial { uniforms }))
        }
        "shaders/vfx_pyro_bullet.wesl" => {
            BulletMaterialHandle::Pyro(pyro_mats.add(PyroBulletMaterial { uniforms }))
        }
        "shaders/vfx_venom_bullet.wesl" => {
            BulletMaterialHandle::Venom(venom_mats.add(VenomBulletMaterial { uniforms }))
        }
        "shaders/vfx_electric_bullet.wesl" => {
            BulletMaterialHandle::Electric(electric_mats.add(ElectricBulletMaterial { uniforms }))
        }
        "shaders/vfx_flame_bullet.wesl" => {
            BulletMaterialHandle::Flame(flame_mats.add(FlameBulletMaterial { uniforms }))
        }
        "shaders/vfx_blade_bullet.wesl" => {
            BulletMaterialHandle::Blade(blade_mats.add(BladeBulletMaterial { uniforms }))
        }
        "shaders/vfx_pistol_bullet.wesl" => {
            BulletMaterialHandle::Pistol(pistol_mats.add(PistolBulletMaterial { uniforms }))
        }
        "shaders/vfx_kinetic_bullet.wesl" => {
            BulletMaterialHandle::Kinetic(kinetic_mats.add(KineticBulletMaterial { uniforms }))
        }
        other => panic!(
            "خطای بحرانی VFX: فایل شیدر '{}' در config.ron شناخته شده نیست!",
            other
        ),
    };

    registry
        .entries
        .insert(source, CachedBulletVisualAsset { mesh, material });
}

#[allow(clippy::too_many_arguments)]
fn init_bullet_vfx_registry_system(
    config: Res<GameConfig>,
    mut registry: ResMut<BulletVfxRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut vanguard_mats: ResMut<Assets<VanguardBulletMaterial>>,
    mut frost_mats: ResMut<Assets<FrostBulletMaterial>>,
    mut iron_mats: ResMut<Assets<IronBulletMaterial>>,
    mut pyro_mats: ResMut<Assets<PyroBulletMaterial>>,
    mut venom_mats: ResMut<Assets<VenomBulletMaterial>>,
    mut electric_mats: ResMut<Assets<ElectricBulletMaterial>>,
    mut flame_mats: ResMut<Assets<FlameBulletMaterial>>,
    mut blade_mats: ResMut<Assets<BladeBulletMaterial>>,
    mut pistol_mats: ResMut<Assets<PistolBulletMaterial>>,
    mut kinetic_mats: ResMut<Assets<KineticBulletMaterial>>,
) {
    for hero in &config.heroes {
        register_bullet_profile(
            &mut registry,
            BulletVfxSource::Hero {
                hero_id: hero.id.clone(),
                weapon_kind: hero.weapon.weapon_kind,
            },
            &hero.bullet_vfx.primary,
            &mut meshes,
            &mut vanguard_mats,
            &mut frost_mats,
            &mut iron_mats,
            &mut pyro_mats,
        &mut venom_mats,
        &mut electric_mats,
        &mut flame_mats,
        &mut blade_mats,
            &mut pistol_mats,
            &mut kinetic_mats,
        );
        register_bullet_profile(
            &mut registry,
            BulletVfxSource::Hero {
                hero_id: hero.id.clone(),
                weapon_kind: WeaponKind::Pistol,
            },
            &hero.bullet_vfx.pistol,
            &mut meshes,
            &mut vanguard_mats,
            &mut frost_mats,
            &mut iron_mats,
            &mut pyro_mats,
        &mut venom_mats,
        &mut electric_mats,
        &mut flame_mats,
        &mut blade_mats,
            &mut pistol_mats,
            &mut kinetic_mats,
        );
    }

    register_bullet_profile(
        &mut registry,
        BulletVfxSource::Creep,
        &config.fx.creep_bullet_vfx,
        &mut meshes,
        &mut vanguard_mats,
        &mut frost_mats,
        &mut iron_mats,
        &mut pyro_mats,
        &mut venom_mats,
        &mut electric_mats,
        &mut flame_mats,
        &mut blade_mats,
        &mut pistol_mats,
        &mut kinetic_mats,
    );

    register_bullet_profile(
        &mut registry,
        BulletVfxSource::Turret,
        &config.fx.turret_bullet_vfx,
        &mut meshes,
        &mut vanguard_mats,
        &mut frost_mats,
        &mut iron_mats,
        &mut pyro_mats,
        &mut venom_mats,
        &mut electric_mats,
        &mut flame_mats,
        &mut blade_mats,
        &mut pistol_mats,
        &mut kinetic_mats,
    );
}

fn build_burst_effect(
    name: &'static str,
    capacity: u32,
    count: f32,
    spawn_radius: f32,
    speed_min: f32,
    speed_max: f32,
    life_min: f32,
    life_max: f32,
    accel: Vec3,
    drag: f32,
    colors: &[(f32, Vec4)],
    sizes: &[(f32, Vec3)],
) -> EffectAsset {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(spawn_radius).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(speed_min).uniform(writer.lit(speed_max)).expr(),
    };
    let init_life = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(life_min).uniform(writer.lit(life_max)).expr(),
    );
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0_f32).expr());

    let update_accel = AccelModifier::new(writer.lit(accel).expr());
    let update_drag = LinearDragModifier::new(writer.lit(drag).expr());

    let mut color_grad: bevy_hanabi::Gradient<Vec4> = bevy_hanabi::Gradient::new();
    for &(k, v) in colors {
        color_grad.add_key(k, v);
    }
    let mut size_grad: bevy_hanabi::Gradient<Vec3> = bevy_hanabi::Gradient::new();
    for &(k, v) in sizes {
        size_grad.add_key(k, v);
    }

    let spawner = SpawnerSettings::once(count.into());
    let module = writer.finish();

    EffectAsset::new(capacity, spawner, module)
        .with_name(name)
        .init(init_pos)
        .init(init_vel)
        .init(init_life)
        .init(init_age)
        .update(update_accel)
        .update(update_drag)
        .render(ColorOverLifetimeModifier::new(color_grad))
        .render(SizeOverLifetimeModifier {
            gradient: size_grad,
            screen_space_size: false,
        })
        .render(OrientModifier {
            mode: OrientMode::ParallelCameraDepthPlane,
            rotation: None,
        })
}

fn init_hanabi_vfx_assets_system(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ۱. دود سریع سر لوله تفنگ (Muzzle Gunsmoke)
    let muzzle_smoke = effects.add(build_burst_effect(
        "muzzle_smoke",
        4096,
        12.0,
        0.04,
        1.6,
        3.6,
        0.22,
        0.50,
        Vec3::new(0.0, 1.5, 0.0),
        4.5,
        &[
            (0.0, Vec4::new(1.8, 1.4, 0.9, 0.55)),
            (0.25, Vec4::new(0.65, 0.65, 0.68, 0.35)),
            (1.0, Vec4::new(0.45, 0.46, 0.50, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.06)),
            (0.5, Vec3::splat(0.18)),
            (1.0, Vec3::splat(0.32)),
        ],
    ));

    // ۲. پاشش خون (برخورد به دشمنان و سربازان)
    let blood_burst = effects.add(build_burst_effect(
        "blood_burst",
        8192,
        24.0,
        0.08,
        2.2,
        5.8,
        0.28,
        0.58,
        Vec3::new(0.0, -14.0, 0.0),
        2.0,
        &[
            (0.0, Vec4::new(2.2, 0.08, 0.08, 0.95)),
            (0.5, Vec4::new(0.95, 0.03, 0.03, 0.85)),
            (1.0, Vec4::new(0.35, 0.01, 0.01, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.09)),
            (0.4, Vec3::splat(0.06)),
            (1.0, Vec3::splat(0.02)),
        ],
    ));

    // ۳. گرد و خاک نرم (برخورد به سنگ، دیوار و کوه)
    let stone_dust = effects.add(build_burst_effect(
        "stone_dust",
        8192,
        18.0,
        0.10,
        1.0,
        2.8,
        0.35,
        0.75,
        Vec3::new(0.0, 0.9, 0.0),
        4.0,
        &[
            (0.0, Vec4::new(0.85, 0.78, 0.66, 0.65)),
            (0.5, Vec4::new(0.62, 0.58, 0.52, 0.35)),
            (1.0, Vec4::new(0.48, 0.46, 0.42, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.08)),
            (0.4, Vec3::splat(0.24)),
            (1.0, Vec3::splat(0.42)),
        ],
    ));

    // ۴. جرقه و ذرات فلزی درخشان (برخورد به اجسام فلزی مثل Turret)
    let metal_sparks = effects.add(build_burst_effect(
        "metal_sparks",
        8192,
        26.0,
        0.06,
        3.5,
        9.0,
        0.18,
        0.42,
        Vec3::new(0.0, -16.0, 0.0),
        1.5,
        &[
            (0.0, Vec4::new(12.0, 7.2, 1.4, 1.0)),
            (0.5, Vec4::new(6.0, 2.2, 0.25, 0.9)),
            (1.0, Vec4::new(1.5, 0.3, 0.05, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.065)),
            (0.5, Vec3::splat(0.045)),
            (1.0, Vec3::splat(0.01)),
        ],
    ));

    // ۵. پاشش قطرات آب (برخورد به آب)
    let water_splash = effects.add(build_burst_effect(
        "water_splash",
        8192,
        22.0,
        0.10,
        2.0,
        5.2,
        0.28,
        0.55,
        Vec3::new(0.0, -12.0, 0.0),
        1.8,
        &[
            (0.0, Vec4::new(1.4, 2.4, 3.0, 0.85)),
            (0.5, Vec4::new(0.75, 0.95, 1.05, 0.55)),
            (1.0, Vec4::new(0.65, 0.90, 1.0, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.075)),
            (0.5, Vec3::splat(0.09)),
            (1.0, Vec3::splat(0.03)),
        ],
    ));

    let explosion_sparks = effects.add(build_burst_effect(
        "explosion_sparks",
        8192,
        48.0,
        0.35,
        6.0,
        16.0,
        0.28,
        0.68,
        Vec3::new(0.0, -14.0, 0.0),
        1.4,
        &[
            (0.0, Vec4::new(18.0, 10.0, 2.2, 1.0)),
            (0.45, Vec4::new(10.0, 3.2, 0.4, 0.95)),
            (1.0, Vec4::new(2.0, 0.3, 0.05, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.11)),
            (0.5, Vec3::splat(0.07)),
            (1.0, Vec3::splat(0.015)),
        ],
    ));

    let explosion_smoke = effects.add(build_burst_effect(
        "explosion_smoke",
        4096,
        24.0,
        0.55,
        1.8,
        5.2,
        0.65,
        1.45,
        Vec3::new(0.0, 2.8, 0.0),
        2.6,
        &[
            (0.0, Vec4::new(1.4, 0.55, 0.15, 0.45)),
            (0.28, Vec4::new(0.24, 0.22, 0.21, 0.42)),
            (1.0, Vec4::new(0.16, 0.16, 0.17, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.32)),
            (0.45, Vec3::splat(0.85)),
            (1.0, Vec3::splat(1.35)),
        ],
    ));

    let water_explosion_spray = effects.add(build_burst_effect(
        "water_explosion_spray",
        8192,
        64.0,
        0.45,
        5.5,
        14.5,
        0.45,
        1.05,
        Vec3::new(0.0, -15.5, 0.0),
        1.2,
        &[
            (0.0, Vec4::new(1.8, 3.2, 3.8, 0.95)),
            (0.45, Vec4::new(0.85, 1.35, 1.55, 0.72)),
            (1.0, Vec4::new(0.75, 0.96, 1.0, 0.0)),
        ],
        &[
            (0.0, Vec3::splat(0.14)),
            (0.45, Vec3::splat(0.22)),
            (1.0, Vec3::splat(0.05)),
        ],
    ));

    let trail_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    let muzzle_flash_mesh = meshes.add(Cone {
        radius: 0.35,
        height: 1.0,
    });
    let shockwave_ring_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    let blood_decal_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    let blood_air_droplet_mesh = meshes.add(Sphere::new(0.026));
    let blood_air_droplet_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.015, 0.015),
        perceptual_roughness: 0.18,
        metallic: 0.05,
        ..default()
    });
    let fireball_sphere_mesh = meshes.add(Sphere::new(1.0));
    let water_geyser_mesh = meshes.add(Cone {
        radius: 1.0,
        height: 1.0,
    });

    // ۶. حباب‌ها و جرقه‌های سطح برکه (سبک Crash Bandicoot با Hanabi 0.20)
    let pond_writer = ExprWriter::new();
    let p_init_pos = SetPositionSphereModifier {
        center: pond_writer.lit(Vec3::ZERO).expr(),
        radius: pond_writer.lit(7.5_f32).expr(),
        dimension: ShapeDimension::Volume,
    };
    let p_init_vel = SetVelocitySphereModifier {
        center: pond_writer.lit(Vec3::ZERO).expr(),
        speed: pond_writer.lit(0.25_f32).uniform(pond_writer.lit(0.75_f32)).expr(),
    };
    let p_init_life = SetAttributeModifier::new(
        Attribute::LIFETIME,
        pond_writer.lit(1.2_f32).uniform(pond_writer.lit(2.4_f32)).expr(),
    );
    let p_init_age = SetAttributeModifier::new(Attribute::AGE, pond_writer.lit(0.0_f32).expr());
    let p_accel = AccelModifier::new(pond_writer.lit(Vec3::new(0.0, 0.85, 0.0)).expr());
    let p_drag = LinearDragModifier::new(pond_writer.lit(1.4_f32).expr());

    let mut pond_color_grad: bevy_hanabi::Gradient<Vec4> = bevy_hanabi::Gradient::new();
    pond_color_grad.add_key(0.0, Vec4::new(0.55, 1.85, 1.55, 0.0));
    pond_color_grad.add_key(0.25, Vec4::new(0.65, 2.10, 1.75, 0.78));
    pond_color_grad.add_key(0.80, Vec4::new(0.85, 1.95, 1.65, 0.45));
    pond_color_grad.add_key(1.0, Vec4::new(0.90, 2.00, 1.80, 0.0));

    let mut pond_size_grad: bevy_hanabi::Gradient<Vec3> = bevy_hanabi::Gradient::new();
    pond_size_grad.add_key(0.0, Vec3::splat(0.04));
    pond_size_grad.add_key(0.45, Vec3::splat(0.11));
    pond_size_grad.add_key(1.0, Vec3::splat(0.02));

    let pond_ambient_bubbles = effects.add(
        EffectAsset::new(512, SpawnerSettings::rate(3.5_f32.into()), pond_writer.finish())
            .with_name("pond_ambient_bubbles")
            .init(p_init_pos)
            .init(p_init_vel)
            .init(p_init_life)
            .init(p_init_age)
            .update(p_accel)
            .update(p_drag)
            .render(ColorOverLifetimeModifier::new(pond_color_grad))
            .render(SizeOverLifetimeModifier {
                gradient: pond_size_grad,
                screen_space_size: false,
            })
            .render(OrientModifier {
                mode: OrientMode::ParallelCameraDepthPlane,
                rotation: None,
            }),
    );

    // ۷. افکت متراکم پاشش و اسپری آب در پای آبشار (Dense Water Spray Effect)
    let spray_writer = ExprWriter::new();
    let s_init_pos = SetPositionSphereModifier {
        center: spray_writer.lit(Vec3::ZERO).expr(),
        radius: spray_writer.lit(14.0_f32).expr(),
        dimension: ShapeDimension::Volume,
    };
    let s_init_vel = SetVelocitySphereModifier {
        center: spray_writer.lit(Vec3::new(0.0, -3.5, 0.0)).expr(),
        speed: spray_writer.lit(4.5_f32).uniform(spray_writer.lit(11.5_f32)).expr(),
    };
    let s_init_life = SetAttributeModifier::new(
        Attribute::LIFETIME,
        spray_writer.lit(0.45_f32).uniform(spray_writer.lit(1.05_f32)).expr(),
    );
    let s_init_age = SetAttributeModifier::new(Attribute::AGE, spray_writer.lit(0.0_f32).expr());
    let s_accel = AccelModifier::new(spray_writer.lit(Vec3::new(0.0, -11.5, 0.0)).expr());
    let s_drag = LinearDragModifier::new(spray_writer.lit(1.6_f32).expr());

    let mut spray_color_grad: bevy_hanabi::Gradient<Vec4> = bevy_hanabi::Gradient::new();
    spray_color_grad.add_key(0.0, Vec4::new(0.88, 1.0, 0.99, 0.92));
    spray_color_grad.add_key(0.35, Vec4::new(0.75, 0.96, 0.96, 0.78));
    spray_color_grad.add_key(0.75, Vec4::new(0.65, 0.92, 0.95, 0.35));
    spray_color_grad.add_key(1.0, Vec4::new(0.80, 0.98, 1.0, 0.0));

    let mut spray_size_grad: bevy_hanabi::Gradient<Vec3> = bevy_hanabi::Gradient::new();
    spray_size_grad.add_key(0.0, Vec3::splat(0.14));
    spray_size_grad.add_key(0.35, Vec3::splat(0.28));
    spray_size_grad.add_key(1.0, Vec3::splat(0.06));

    let waterfall_dense_spray = effects.add(
        EffectAsset::new(2048, SpawnerSettings::rate(140.0_f32.into()), spray_writer.finish())
            .with_name("waterfall_dense_spray")
            .init(s_init_pos)
            .init(s_init_vel)
            .init(s_init_life)
            .init(s_init_age)
            .update(s_accel)
            .update(s_drag)
            .render(ColorOverLifetimeModifier::new(spray_color_grad))
            .render(SizeOverLifetimeModifier {
                gradient: spray_size_grad,
                screen_space_size: false,
            })
            .render(OrientModifier {
                mode: OrientMode::ParallelCameraDepthPlane,
                rotation: None,
            }),
    );

    commands.insert_resource(HanabiEffectHandles {
        muzzle_smoke,
        blood_burst,
        stone_dust,
        metal_sparks,
        water_splash,
        pond_ambient_bubbles,
        waterfall_dense_spray,
        explosion_sparks,
        explosion_smoke,
        water_explosion_spray,
        flame_beam_embers: effects.add(build_burst_effect(
            "flame_beam_embers",
            4096,
            10.0,
            0.14,
            2.2,
            5.4,
            0.22,
            0.48,
            Vec3::new(0.0, 4.5, 0.0),
            2.8,
            &[
                (0.0, Vec4::new(7.5, 2.4, 0.25, 1.0)),
                (0.45, Vec4::new(4.5, 0.9, 0.08, 0.9)),
                (1.0, Vec4::new(1.8, 0.15, 0.0, 0.0)),
            ],
            &[
                (0.0, Vec3::new(0.04, 0.09, 0.04)),
                (0.5, Vec3::new(0.03, 0.07, 0.03)),
                (1.0, Vec3::splat(0.01)),
            ],
        )),
        flame_beam_smoke: effects.add(build_burst_effect(
            "flame_beam_smoke",
            4096,
            6.0,
            0.28,
            0.9,
            2.2,
            0.35,
            0.75,
            Vec3::new(0.0, 3.2, 0.0),
            3.5,
            &[
                (0.0, Vec4::new(0.32, 0.14, 0.06, 0.42)),
                (0.35, Vec4::new(0.10, 0.08, 0.07, 0.34)),
                (1.0, Vec4::new(0.04, 0.04, 0.04, 0.0)),
            ],
            &[
                (0.0, Vec3::splat(0.14)),
                (0.5, Vec3::splat(0.34)),
                (1.0, Vec3::splat(0.58)),
            ],
        )),
        toxic_splash_sparks: effects.add(build_burst_effect(
            "toxic_splash_sparks",
            4096,
            22.0,
            0.18,
            2.4,
            6.5,
            0.28,
            0.62,
            Vec3::new(0.0, -11.0, 0.0),
            2.2,
            &[
                (0.0, Vec4::new(2.2, 6.5, 0.6, 1.0)),
                (0.45, Vec4::new(0.6, 3.8, 0.2, 0.9)),
                (1.0, Vec4::new(0.15, 0.9, 0.05, 0.0)),
            ],
            &[
                (0.0, Vec3::splat(0.11)),
                (0.5, Vec3::splat(0.08)),
                (1.0, Vec3::splat(0.02)),
            ],
        )),
        trail_mesh,
        muzzle_flash_mesh,
        shockwave_ring_mesh,
        blood_decal_mesh,
        blood_air_droplet_mesh,
        blood_air_droplet_mat,
        fireball_sphere_mesh,
        water_geyser_mesh,
    });
}

#[derive(SystemParam)]
struct VfxProcessMaterials<'w, 's> {
    trail_materials: ResMut<'w, Assets<BulletTrailMaterial>>,
    flash_materials: ResMut<'w, Assets<MuzzleFlashMaterial>>,
    shockwave_materials: ResMut<'w, Assets<ShockwaveRingMaterial>>,
    blood_materials: ResMut<'w, Assets<BloodSplatterMaterial>>,
    fireball_materials: ResMut<'w, Assets<ExplosionFireballMaterial>>,
    water_exp_materials: ResMut<'w, Assets<WaterExplosionMaterial>>,
    cam_q: Query<'w, 's, &'static Transform, With<Camera3d>>,
    active_lights_q: Query<'w, 's, (), With<OneShotLightFade>>,
}

fn next_vfx_rand(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    (*seed as f32) / (u32::MAX as f32)
}

#[derive(SystemParam)]
struct VfxActiveCounts<'w, 's> {
    one_shot_q: Query<'w, 's, (), With<OneShotVfxDespawn>>,
    trail_q: Query<'w, 's, (), With<BulletTrailInstance>>,
    ring_q: Query<'w, 's, (), With<ShockwaveRingInstance>>,
    decal_q: Query<'w, 's, (), With<BloodGroundDecal>>,
}

fn process_vfx_events_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    handles: Option<Res<HanabiEffectHandles>>,
    mut queue: ResMut<VfxEventQueue>,
    mut pool: ResMut<VfxMaterialPool>,
    active_counts: VfxActiveCounts,
    mut mats: VfxProcessMaterials,
    mut rng_seed: Local<u32>,
) {
    if *rng_seed == 0 {
        *rng_seed = 0xCAFE_BABE;
    }
    if queue.0.is_empty() {
        return;
    }
    let Some(handles) = handles else {
        queue.0.clear();
        return;
    };
    let t = time.elapsed_secs();
    let cam_pos = mats.cam_q.iter().next().map(|tf| tf.translation);
    let mut active_lights = mats.active_lights_q.iter().count();
    let mut active_hanabi = active_counts.one_shot_q.iter().count();
    let mut active_trails = active_counts.trail_q.iter().count();
    let mut active_rings = active_counts.ring_q.iter().count();
    let mut active_decals = active_counts.decal_q.iter().count();
    const MAX_ACTIVE_VFX_LIGHTS: usize = 10;
    const MAX_VFX_CAMERA_DIST_SQ: f32 = 85.0 * 85.0;
    let is_near_camera = |p: Vec3| -> bool {
        cam_pos.map_or(true, |cp| cp.distance_squared(p) <= MAX_VFX_CAMERA_DIST_SQ)
    };

    for ev in queue.0.drain(..) {
        match ev {
            ClientVfxEvent::MuzzleSmoke {
                pos,
                dir,
                vfx_source,
            } => {
                if !is_near_camera(pos) {
                    continue;
                }
                let profile = config.resolve_bullet_vfx(&vfx_source);
                let forward = dir.normalize_or_zero();
                let spawn_pos = pos + forward * 0.16;

                let is_hero_shot = matches!(vfx_source, BulletVfxSource::Hero { .. });
                // لایه ۱: دود سر لوله تفنگ (Hanabi با سقف همزمانی برای جلوگیری از دراپ در درگیری شلوغ)
                if active_hanabi < MAX_ACTIVE_HANABI && (is_hero_shot || active_hanabi < MAX_ACTIVE_HANABI / 2) {
                    active_hanabi += 1;
                    commands.spawn((
                        ParticleEffect::new(handles.muzzle_smoke.clone()),
                        Transform::from_translation(spawn_pos),
                        Visibility::default(),
                        OneShotVfxDespawn {
                            timer: Timer::from_seconds(0.65, TimerMode::Once),
                        },
                    ));
                }

                // لایه ۲: شعله ستاره‌ای سر لوله با شیدر WESL اختصاصی (بازیافت از Pool)
                let flash_rot = Quat::from_rotation_arc(Vec3::Y, forward);
                let flash_uniforms = MuzzleFlashUniforms {
                    core_color: Vec4::new(
                        profile.core_color_rgb.0,
                        profile.core_color_rgb.1,
                        profile.core_color_rgb.2,
                        1.0,
                    ),
                    edge_color: Vec4::new(
                        profile.edge_color_rgb.0,
                        profile.edge_color_rgb.1,
                        profile.edge_color_rgb.2,
                        1.0,
                    ),
                    params: Vec4::new(0.0, profile.bloom_intensity, t.fract(), 0.0),
                };
                let flash_mat = if let Some(reused) = pool.free_flash_mats.pop() {
                    if let Some(mut existing) = mats.flash_materials.get_mut(&reused) {
                        existing.uniforms = flash_uniforms;
                    }
                    reused
                } else {
                    mats.flash_materials.add(MuzzleFlashMaterial {
                        uniforms: flash_uniforms,
                    })
                };
                commands.spawn((
                    Mesh3d(handles.muzzle_flash_mesh.clone()),
                    MeshMaterial3d(flash_mat.clone()),
                    Transform {
                        translation: pos + forward * (profile.muzzle_flash_scale * 0.55),
                        rotation: flash_rot,
                        scale: Vec3::splat(profile.muzzle_flash_scale),
                    },
                    NotShadowCaster,
                    MuzzleFlashInstance {
                        timer: Timer::from_seconds(
                            profile.muzzle_flash_lifetime_secs,
                            TimerMode::Once,
                        ),
                        mat_handle: flash_mat,
                    },
                ));

                // لایه ۳: حلقه موج شوک سر لوله (بازیافت از Pool + سقف همزمانی)
                if active_rings < MAX_ACTIVE_RINGS && (is_hero_shot || active_rings < MAX_ACTIVE_RINGS / 2) {
                    active_rings += 1;
                    let ring_uniforms = ShockwaveUniforms {
                        core_color: Vec4::new(
                            profile.core_color_rgb.0,
                            profile.core_color_rgb.1,
                            profile.core_color_rgb.2,
                            1.0,
                        ),
                        edge_color: Vec4::new(
                            profile.edge_color_rgb.0,
                            profile.edge_color_rgb.1,
                            profile.edge_color_rgb.2,
                            1.0,
                        ),
                        params: Vec4::new(0.0, profile.bloom_intensity * 0.75, 8.0, 0.0),
                    };
                    let ring_mat = if let Some(reused) = pool.free_ring_mats.pop() {
                        if let Some(mut existing) = mats.shockwave_materials.get_mut(&reused) {
                            existing.uniforms = ring_uniforms;
                        }
                        reused
                    } else {
                        mats.shockwave_materials.add(ShockwaveRingMaterial {
                            uniforms: ring_uniforms,
                        })
                    };
                    commands.spawn((
                        Mesh3d(handles.shockwave_ring_mesh.clone()),
                        MeshMaterial3d(ring_mat.clone()),
                        Transform {
                            translation: pos + forward * 0.14,
                            rotation: flash_rot,
                            scale: Vec3::splat(profile.muzzle_ring_start_scale),
                        },
                        NotShadowCaster,
                        ShockwaveRingInstance {
                            timer: Timer::from_seconds(
                                profile.muzzle_flash_lifetime_secs * 1.4,
                                TimerMode::Once,
                            ),
                            start_scale: profile.muzzle_ring_start_scale,
                            end_scale: profile.muzzle_ring_end_scale,
                            mat_handle: ring_mat,
                        },
                    ));
                }

                // لایه ۴: نور دینامیک HDR سر لوله تفنگ (فقط برای هیروها و با سقف همزمانی برای حفظ FPS)
                if matches!(vfx_source, BulletVfxSource::Hero { .. })
                    && profile.muzzle_light_intensity > 0.0
                    && active_lights < MAX_ACTIVE_VFX_LIGHTS
                {
                    active_lights += 1;
                    commands.spawn((
                        PointLight {
                            color: Color::srgb(
                                profile.core_color_rgb.0,
                                profile.core_color_rgb.1,
                                profile.core_color_rgb.2,
                            ),
                            intensity: profile.muzzle_light_intensity,
                            range: profile.muzzle_light_range,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(pos + forward * 0.25),
                        OneShotLightFade {
                            timer: Timer::from_seconds(
                                profile.muzzle_flash_lifetime_secs * 1.35,
                                TimerMode::Once,
                            ),
                            initial_intensity: profile.muzzle_light_intensity,
                        },
                    ));
                }
            }
            ClientVfxEvent::MagneticTrail {
                start,
                end,
                vfx_source,
            } => {
                if !is_near_camera(start) && !is_near_camera(end) {
                    continue;
                }
                let delta = end - start;
                let len = delta.length();
                if len <= 0.08 {
                    continue;
                }
                let profile = config.resolve_bullet_vfx(&vfx_source);
                let dir = delta / len;
                let mid = (start + end) * 0.5;
                let right = dir.cross(Vec3::Y).normalize_or_zero();
                let rot = if right.length_squared() > 0.001 {
                    let up = right.cross(dir).normalize_or_zero();
                    Quat::from_mat3(&Mat3::from_cols(dir, up, right))
                } else {
                    Quat::IDENTITY
                };

                let is_hero_trail = matches!(vfx_source, BulletVfxSource::Hero { .. });
                if active_trails >= MAX_ACTIVE_TRAILS || (!is_hero_trail && active_trails >= MAX_ACTIVE_TRAILS * 2 / 3) {
                    continue;
                }
                active_trails += 1;
                let trail_uniforms = BulletTrailUniforms {
                    core_color: Vec4::new(
                        profile.core_color_rgb.0,
                        profile.core_color_rgb.1,
                        profile.core_color_rgb.2,
                        1.0,
                    ),
                    wave_color: Vec4::new(
                        profile.edge_color_rgb.0,
                        profile.edge_color_rgb.1,
                        profile.edge_color_rgb.2,
                        1.0,
                    ),
                    params: Vec4::new(0.0, t, profile.trail_wave_freq, profile.trail_wave_amp),
                    style_params: Vec4::new(
                        profile.bloom_intensity * 0.55,
                        profile.trail_style_mode,
                        0.0,
                        0.0,
                    ),
                };
                let mat_handle = if let Some(reused) = pool.free_trail_mats.pop() {
                    if let Some(mut existing) = mats.trail_materials.get_mut(&reused) {
                        existing.uniforms = trail_uniforms;
                    }
                    reused
                } else {
                    mats.trail_materials.add(BulletTrailMaterial {
                        uniforms: trail_uniforms,
                    })
                };

                commands.spawn((
                    Mesh3d(handles.trail_mesh.clone()),
                    MeshMaterial3d(mat_handle.clone()),
                    Transform {
                        translation: mid,
                        rotation: rot,
                        scale: Vec3::new(len, 1.0, profile.trail_width),
                    },
                    NotShadowCaster,
                    BulletTrailInstance {
                        timer: Timer::from_seconds(profile.trail_lifetime_secs, TimerMode::Once),
                        mat_handle,
                    },
                ));
            }
            ClientVfxEvent::BulletImpact {
                pos,
                bullet_dir,
                surface,
                vfx_source,
            } => {
                if !is_near_camera(pos) {
                    continue;
                }
                let profile = config.resolve_bullet_vfx(&vfx_source);
                let (effect_h, lifetime) = match surface {
                    ImpactSurface::Character => (handles.blood_burst.clone(), 0.70),
                    ImpactSurface::StoneWall | ImpactSurface::Building | ImpactSurface::Tree => {
                        (handles.stone_dust.clone(), 0.85)
                    }
                    ImpactSurface::Turret => (handles.metal_sparks.clone(), 0.55),
                    ImpactSurface::Water => (handles.water_splash.clone(), 0.65),
                };

                let is_hero_impact = matches!(vfx_source, BulletVfxSource::Hero { .. });
                if active_hanabi < MAX_ACTIVE_HANABI && (is_hero_impact || active_hanabi < MAX_ACTIVE_HANABI * 2 / 3) {
                    active_hanabi += 1;
                    commands.spawn((
                        ParticleEffect::new(effect_h),
                        Transform::from_translation(pos),
                        Visibility::default(),
                        OneShotVfxDespawn {
                            timer: Timer::from_seconds(lifetime, TimerMode::Once),
                        },
                    ));
                }

                // حلقه موج برخورد با رنگ و بلوم اختصاصی هیرو (بازیافت از Pool)
                if active_rings < MAX_ACTIVE_RINGS && (is_hero_impact || active_rings < MAX_ACTIVE_RINGS / 2) {
                    active_rings += 1;
                    let imp_ring_uniforms = ShockwaveUniforms {
                        core_color: Vec4::new(
                            profile.core_color_rgb.0,
                            profile.core_color_rgb.1,
                            profile.core_color_rgb.2,
                            1.0,
                        ),
                        edge_color: Vec4::new(
                            profile.edge_color_rgb.0,
                            profile.edge_color_rgb.1,
                            profile.edge_color_rgb.2,
                            1.0,
                        ),
                        params: Vec4::new(0.0, profile.bloom_intensity * 0.85, 10.0, 0.0),
                    };
                    let imp_ring_mat = if let Some(reused) = pool.free_ring_mats.pop() {
                        if let Some(mut existing) = mats.shockwave_materials.get_mut(&reused) {
                            existing.uniforms = imp_ring_uniforms;
                        }
                        reused
                    } else {
                        mats.shockwave_materials.add(ShockwaveRingMaterial {
                            uniforms: imp_ring_uniforms,
                        })
                    };
                    commands.spawn((
                        Mesh3d(handles.shockwave_ring_mesh.clone()),
                        MeshMaterial3d(imp_ring_mat.clone()),
                        Transform::from_translation(pos + Vec3::Y * 0.06)
                            .with_scale(Vec3::splat(0.15)),
                        NotShadowCaster,
                        ShockwaveRingInstance {
                            timer: Timer::from_seconds(
                                profile.impact_ring_lifetime_secs,
                                TimerMode::Once,
                            ),
                            start_scale: 0.15,
                            end_scale: profile.impact_ring_end_scale,
                            mat_handle: imp_ring_mat,
                        },
                    ));
                }

                // نور لحظه‌ای برخورد با بلوم سینماتیک (فقط برای هیروها و با سقف همزمانی)
                if matches!(vfx_source, BulletVfxSource::Hero { .. })
                    && profile.impact_light_intensity > 0.0
                    && active_lights < MAX_ACTIVE_VFX_LIGHTS
                {
                    active_lights += 1;
                    commands.spawn((
                        PointLight {
                            color: Color::srgb(
                                profile.edge_color_rgb.0,
                                profile.edge_color_rgb.1,
                                profile.edge_color_rgb.2,
                            ),
                            intensity: profile.impact_light_intensity,
                            range: profile.impact_light_range,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(pos + Vec3::Y * 0.15),
                        OneShotLightFade {
                            timer: Timer::from_seconds(
                                profile.impact_ring_lifetime_secs,
                                TimerMode::Once,
                            ),
                            initial_intensity: profile.impact_light_intensity,
                        },
                    ));
                }

                // پاشش واقعی و نامنظم خون در هوا و روی زمین با شیدر WESL
                if surface == ImpactSurface::Character {
                    let bs = &config.fx.blood_splatter_vfx;
                    let dir_flat = Vec3::new(bullet_dir.x, 0.0, bullet_dir.z).normalize_or_zero();
                    let base_dir = if dir_flat == Vec3::ZERO { Vec3::Z } else { dir_flat };

                    // ۱. ذرات فیزیکی خون در هوا (جهت‌دار همراه با پراکندگی تصادفی)
                    for _ in 0..bs.air_droplet_count {
                        let r1 = next_vfx_rand(&mut rng_seed);
                        let r2 = next_vfx_rand(&mut rng_seed);
                        let r3 = next_vfx_rand(&mut rng_seed);
                        let yaw_spread = (r1 - 0.5) * 2.2;
                        let spray_dir = (Quat::from_rotation_y(yaw_spread) * base_dir).normalize();
                        let speed = 2.4 + r2 * 4.8;
                        let vel = spray_dir * speed + Vec3::Y * (1.2 + r3 * 2.8);
                        commands.spawn((
                            Mesh3d(handles.blood_air_droplet_mesh.clone()),
                            MeshMaterial3d(handles.blood_air_droplet_mat.clone()),
                            NotShadowCaster,
                            particle_physics_bundle(
                                &config,
                                pos + Vec3::new((r1 - 0.5) * 0.14, (r2 - 0.5) * 0.14, (r3 - 0.5) * 0.14),
                                vel,
                                0.42 + r2 * 0.25,
                                true,
                            ),
                        ));
                    }

                    // ۲. لکه‌های ارگانیک و نامنظم خون روی زمین با شیدر vfx_blood_splatter.wesl
                    let span = (bs.decal_count_max - bs.decal_count_min + 1).max(1);
                    let count = bs.decal_count_min + ((next_vfx_rand(&mut rng_seed) * span as f32) as u32).min(span - 1);
                    let ground_y = (pos.y - 0.85).max(0.03);
                    for i in 0..count {
                        let r_ang = next_vfx_rand(&mut rng_seed) * std::f32::consts::TAU;
                        let r_dist = next_vfx_rand(&mut rng_seed) * bs.decal_scatter_radius;
                        let r_rot = next_vfx_rand(&mut rng_seed) * std::f32::consts::TAU;
                        let r_scl = bs.decal_min_scale + next_vfx_rand(&mut rng_seed) * (bs.decal_max_scale - bs.decal_min_scale);
                        let r_stretch = 1.15 + next_vfx_rand(&mut rng_seed) * 0.70;
                        let r_seed = next_vfx_rand(&mut rng_seed) * 100.0;

                        let offset = base_dir * (0.25 + r_dist * 0.85)
                            + Vec3::new(r_ang.cos() * r_dist * 0.65, 0.004 * (i as f32 + 1.0), r_ang.sin() * r_dist * 0.65);

                        if active_decals >= MAX_ACTIVE_BLOOD_DECALS {
                            break;
                        }
                        active_decals += 1;
                        let blood_uniforms = BloodSplatterUniforms {
                            core_color: Vec4::new(bs.core_color_rgb.0, bs.core_color_rgb.1, bs.core_color_rgb.2, 1.0),
                            edge_color: Vec4::new(bs.edge_color_rgb.0, bs.edge_color_rgb.1, bs.edge_color_rgb.2, 1.0),
                            params: Vec4::new(0.0, r_seed, r_stretch, bs.gloss_strength),
                        };
                        let blood_mat = if let Some(reused) = pool.free_blood_mats.pop() {
                            if let Some(mut existing) = mats.blood_materials.get_mut(&reused) {
                                existing.uniforms = blood_uniforms;
                            }
                            reused
                        } else {
                            mats.blood_materials.add(BloodSplatterMaterial {
                                uniforms: blood_uniforms,
                            })
                        };
                        commands.spawn((
                            Mesh3d(handles.blood_decal_mesh.clone()),
                            MeshMaterial3d(blood_mat.clone()),
                            Transform::from_translation(Vec3::new(pos.x, ground_y, pos.z) + offset)
                                .with_rotation(Quat::from_rotation_y(r_rot))
                                .with_scale(Vec3::new(r_scl * r_stretch, 1.0, r_scl)),
                            NotShadowCaster,
                            BloodGroundDecal {
                                timer: Timer::from_seconds(bs.decal_lifetime_secs, TimerMode::Once),
                                mat_handle: blood_mat,
                            },
                        ));
                    }
                }
            }
            ClientVfxEvent::GrenadeExploded {
                pos,
                aoe_radius,
                in_water,
                water_y,
                weapon_source,
            } => {
                let ge = &config.fx.grenade_explosion_vfx;
                let seed = next_vfx_rand(&mut rng_seed) * 50.0;

                if in_water {
                    let surface_pos = Vec3::new(pos.x, water_y, pos.z);
                    // ۱. فواره آب با شیدر vfx_water_explosion.wesl
                    let w_mat = mats.water_exp_materials.add(WaterExplosionMaterial {
                        uniforms: WaterExplosionUniforms {
                            core_color: Vec4::new(ge.water_core_rgb.0, ge.water_core_rgb.1, ge.water_core_rgb.2, 1.0),
                            foam_color: Vec4::new(ge.water_foam_rgb.0, ge.water_foam_rgb.1, ge.water_foam_rgb.2, 1.0),
                            params: Vec4::new(0.0, ge.water_bloom_intensity, seed, 0.0),
                        },
                    });
                    commands.spawn((
                        Mesh3d(handles.water_geyser_mesh.clone()),
                        MeshMaterial3d(w_mat.clone()),
                        Transform::from_translation(surface_pos + Vec3::Y * 0.5)
                            .with_scale(Vec3::new(aoe_radius * 0.38, 0.8, aoe_radius * 0.38)),
                        NotShadowCaster,
                        WaterExplosionInstance {
                            timer: Timer::from_seconds(ge.water_geyser_duration_secs, TimerMode::Once),
                            base_y: water_y,
                            max_height: ge.water_geyser_height,
                            radius_scale: aoe_radius * 0.42,
                            mat_handle: w_mat,
                        },
                    ));

                    // ۲. حلقه موج انفجار روی سطح آب
                    let ring_mat = mats.shockwave_materials.add(ShockwaveRingMaterial {
                        uniforms: ShockwaveUniforms {
                            core_color: Vec4::new(ge.water_foam_rgb.0, ge.water_foam_rgb.1, ge.water_foam_rgb.2, 1.0),
                            edge_color: Vec4::new(ge.water_core_rgb.0, ge.water_core_rgb.1, ge.water_core_rgb.2, 1.0),
                            params: Vec4::new(0.0, ge.water_bloom_intensity * 0.7, 12.0, 0.0),
                        },
                    });
                    commands.spawn((
                        Mesh3d(handles.shockwave_ring_mesh.clone()),
                        MeshMaterial3d(ring_mat.clone()),
                        Transform::from_translation(surface_pos + Vec3::Y * 0.08)
                            .with_scale(Vec3::splat(0.35)),
                        NotShadowCaster,
                        ShockwaveRingInstance {
                            timer: Timer::from_seconds(ge.shockwave_duration_secs * 1.35, TimerMode::Once),
                            start_scale: 0.35,
                            end_scale: aoe_radius * 1.35,
                            mat_handle: ring_mat,
                        },
                    ));

                    // ۳. ذرات اسپری متراکم آب (Hanabi)
                    commands.spawn((
                        ParticleEffect::new(handles.water_explosion_spray.clone()),
                        Transform::from_translation(surface_pos + Vec3::Y * 0.45),
                        Visibility::default(),
                        OneShotVfxDespawn {
                            timer: Timer::from_seconds(1.25, TimerMode::Once),
                        },
                    ));

                    // ۴. نور لحظه‌ای فیروزه‌ای انفجار در آب
                    commands.spawn((
                        PointLight {
                            color: Color::srgb(ge.water_core_rgb.0, ge.water_core_rgb.1, ge.water_core_rgb.2),
                            intensity: 260_000.0,
                            range: aoe_radius * 2.5,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(surface_pos + Vec3::Y * 1.2),
                        OneShotLightFade {
                            timer: Timer::from_seconds(0.32, TimerMode::Once),
                            initial_intensity: 260_000.0,
                        },
                    ));
                } else {
                    let is_venom = matches!(
                        weapon_source,
                        Some(BulletVfxSource::Hero { ref hero_id, .. }) if hero_id == "hero_venom_alchemist"
                    );
                    let (core_rgb, edge_rgb, bloom_val) = if let Some(ref src) = weapon_source {
                        let prof = config.resolve_bullet_vfx(src);
                        (prof.core_color_rgb, prof.edge_color_rgb, prof.bloom_intensity)
                    } else {
                        (ge.fireball_core_rgb, ge.fireball_edge_rgb, ge.fireball_bloom_intensity)
                    };

                    // ۱. گنبد انفجار یا پاشش AOE با رنگ اختصاصی سلاح یا نارنجک
                    let fb_mat = mats.fireball_materials.add(ExplosionFireballMaterial {
                        uniforms: ExplosionFireballUniforms {
                            core_color: Vec4::new(core_rgb.0, core_rgb.1, core_rgb.2, 1.0),
                            edge_color: Vec4::new(edge_rgb.0, edge_rgb.1, edge_rgb.2, 1.0),
                            params: Vec4::new(0.0, bloom_val, seed, 0.0),
                        },
                    });
                    commands.spawn((
                        Mesh3d(handles.fireball_sphere_mesh.clone()),
                        MeshMaterial3d(fb_mat.clone()),
                        Transform::from_translation(pos + Vec3::Y * 0.35)
                            .with_scale(Vec3::splat(0.35)),
                        NotShadowCaster,
                        ExplosionFireballInstance {
                            timer: Timer::from_seconds(ge.fireball_duration_secs, TimerMode::Once),
                            start_scale: 0.35,
                            end_scale: aoe_radius * if is_venom { 0.55 } else { 0.68 },
                            mat_handle: fb_mat,
                        },
                    ));

                    // ۲. حلقه موج شوک روی زمین
                    let ring_mat = mats.shockwave_materials.add(ShockwaveRingMaterial {
                        uniforms: ShockwaveUniforms {
                            core_color: Vec4::new(core_rgb.0, core_rgb.1, core_rgb.2, 1.0),
                            edge_color: Vec4::new(edge_rgb.0, edge_rgb.1, edge_rgb.2, 1.0),
                            params: Vec4::new(0.0, bloom_val * 0.85, 14.0, 0.0),
                        },
                    });
                    commands.spawn((
                        Mesh3d(handles.shockwave_ring_mesh.clone()),
                        MeshMaterial3d(ring_mat.clone()),
                        Transform::from_translation(pos + Vec3::Y * 0.12)
                            .with_scale(Vec3::splat(0.30)),
                        NotShadowCaster,
                        ShockwaveRingInstance {
                            timer: Timer::from_seconds(ge.shockwave_duration_secs, TimerMode::Once),
                            start_scale: 0.30,
                            end_scale: aoe_radius * 1.25,
                            mat_handle: ring_mat,
                        },
                    ));

                    // ۳. ذرات Hanabi متناسب با نوع انفجار (پاشش اسیدی یا جرقه و دود آتش)
                    if is_venom {
                        commands.spawn((
                            ParticleEffect::new(handles.toxic_splash_sparks.clone()),
                            Transform::from_translation(pos + Vec3::Y * 0.25),
                            Visibility::default(),
                            OneShotVfxDespawn {
                                timer: Timer::from_seconds(0.85, TimerMode::Once),
                            },
                        ));
                    } else {
                        commands.spawn((
                            ParticleEffect::new(handles.explosion_sparks.clone()),
                            Transform::from_translation(pos + Vec3::Y * 0.25),
                            Visibility::default(),
                            OneShotVfxDespawn {
                                timer: Timer::from_seconds(0.85, TimerMode::Once),
                            },
                        ));
                        commands.spawn((
                            ParticleEffect::new(handles.explosion_smoke.clone()),
                            Transform::from_translation(pos + Vec3::Y * 0.35),
                            Visibility::default(),
                            OneShotVfxDespawn {
                                timer: Timer::from_seconds(1.65, TimerMode::Once),
                            },
                        ));
                    }

                    // ۴. نور لحظه‌ای متناسب با رنگ افکت
                    commands.spawn((
                        PointLight {
                            color: Color::srgb(core_rgb.0, core_rgb.1, core_rgb.2),
                            intensity: if is_venom { 220_000.0 } else { 340_000.0 },
                            range: aoe_radius * 2.8,
                            shadow_maps_enabled: false,
                            ..default()
                        },
                        Transform::from_translation(pos + Vec3::Y * 0.8),
                        OneShotLightFade {
                            timer: Timer::from_seconds(0.28, TimerMode::Once),
                            initial_intensity: if is_venom { 220_000.0 } else { 340_000.0 },
                        },
                    ));
                }
            }
            ClientVfxEvent::SpawnPondAmbient { pos } => {
                commands.spawn((
                    ParticleEffect::new(handles.pond_ambient_bubbles.clone()),
                    Transform::from_translation(pos),
                    Visibility::default(),
                ));
            }
            ClientVfxEvent::FlameStreamStep { start, end, hit_any } => {
                if !is_near_camera(start) && !is_near_camera(end) {
                    continue;
                }
                let mid_pos = start.lerp(end, 0.42);
                if active_hanabi < MAX_ACTIVE_HANABI {
                    active_hanabi += 1;
                    commands.spawn((
                        ParticleEffect::new(handles.flame_beam_embers.clone()),
                        Transform::from_translation(mid_pos),
                        Visibility::default(),
                        OneShotVfxDespawn {
                            timer: Timer::from_seconds(0.55, TimerMode::Once),
                        },
                    ));
                }
                if active_hanabi < MAX_ACTIVE_HANABI {
                    active_hanabi += 1;
                    commands.spawn((
                        ParticleEffect::new(handles.flame_beam_smoke.clone()),
                        Transform::from_translation(start.lerp(end, 0.72)),
                        Visibility::default(),
                        OneShotVfxDespawn {
                            timer: Timer::from_seconds(0.85, TimerMode::Once),
                        },
                    ));
                }
                if hit_any && active_hanabi < MAX_ACTIVE_HANABI {
                    active_hanabi += 1;
                    commands.spawn((
                        ParticleEffect::new(handles.flame_beam_embers.clone()),
                        Transform::from_translation(end),
                        Visibility::default(),
                        OneShotVfxDespawn {
                            timer: Timer::from_seconds(0.55, TimerMode::Once),
                        },
                    ));
                }
            }
            ClientVfxEvent::WaterSplash { pos, intensity } => {
                if !is_near_camera(pos) {
                    continue;
                }
                let ge = &config.fx.grenade_explosion_vfx;
                let seed = next_vfx_rand(&mut rng_seed) * 50.0;
                let scale = intensity.clamp(1.0, 3.0);

                // ۱. فواره و تاج آب با شیدر vfx_water_explosion.wesl
                let w_mat = mats.water_exp_materials.add(WaterExplosionMaterial {
                    uniforms: WaterExplosionUniforms {
                        core_color: Vec4::new(ge.water_core_rgb.0, ge.water_core_rgb.1, ge.water_core_rgb.2, 1.0),
                        foam_color: Vec4::new(ge.water_foam_rgb.0, ge.water_foam_rgb.1, ge.water_foam_rgb.2, 1.0),
                        params: Vec4::new(0.0, ge.water_bloom_intensity * 0.72, seed, 0.0),
                    },
                });
                commands.spawn((
                    Mesh3d(handles.water_geyser_mesh.clone()),
                    MeshMaterial3d(w_mat.clone()),
                    Transform::from_translation(pos + Vec3::Y * 0.25)
                        .with_scale(Vec3::new(0.75 * scale, 0.45, 0.75 * scale)),
                    NotShadowCaster,
                    WaterExplosionInstance {
                        timer: Timer::from_seconds(0.52, TimerMode::Once),
                        base_y: pos.y,
                        max_height: 1.55 * scale,
                        radius_scale: 0.82 * scale,
                        mat_handle: w_mat,
                    },
                ));

                // ۲. حلقه موج پاشش روی سطح آب (vfx_shockwave_ring.wesl)
                let ring_mat = mats.shockwave_materials.add(ShockwaveRingMaterial {
                    uniforms: ShockwaveUniforms {
                        core_color: Vec4::new(ge.water_foam_rgb.0, ge.water_foam_rgb.1, ge.water_foam_rgb.2, 1.0),
                        edge_color: Vec4::new(ge.water_core_rgb.0, ge.water_core_rgb.1, ge.water_core_rgb.2, 1.0),
                        params: Vec4::new(0.0, ge.water_bloom_intensity * 0.55, 10.0, 0.0),
                    },
                });
                commands.spawn((
                    Mesh3d(handles.shockwave_ring_mesh.clone()),
                    MeshMaterial3d(ring_mat.clone()),
                    Transform::from_translation(pos + Vec3::Y * 0.06)
                        .with_scale(Vec3::splat(0.35)),
                    NotShadowCaster,
                    ShockwaveRingInstance {
                        timer: Timer::from_seconds(0.68, TimerMode::Once),
                        start_scale: 0.35,
                        end_scale: 2.6 * scale,
                        mat_handle: ring_mat,
                    },
                ));

                // ۳. ذرات اسپری پاشش آب به اطراف (Hanabi)
                commands.spawn((
                    ParticleEffect::new(handles.water_explosion_spray.clone()),
                    Transform::from_translation(pos + Vec3::Y * 0.22),
                    Visibility::default(),
                    OneShotVfxDespawn {
                        timer: Timer::from_seconds(1.05, TimerMode::Once),
                    },
                ));
            }
            ClientVfxEvent::SpawnWaterfallSpray { pos } => {
                commands.spawn((
                    ParticleEffect::new(handles.waterfall_dense_spray.clone()),
                    Transform::from_translation(pos),
                    Visibility::default(),
                ));
            }
        }
    }
}

#[derive(SystemParam)]
struct VfxLifetimeMaterials<'w> {
    vanguard_mats: ResMut<'w, Assets<VanguardBulletMaterial>>,
    frost_mats: ResMut<'w, Assets<FrostBulletMaterial>>,
    iron_mats: ResMut<'w, Assets<IronBulletMaterial>>,
    pyro_mats: ResMut<'w, Assets<PyroBulletMaterial>>,
    pistol_mats: ResMut<'w, Assets<PistolBulletMaterial>>,
    kinetic_mats: ResMut<'w, Assets<KineticBulletMaterial>>,
    trail_materials: ResMut<'w, Assets<BulletTrailMaterial>>,
    flash_materials: ResMut<'w, Assets<MuzzleFlashMaterial>>,
    shockwave_materials: ResMut<'w, Assets<ShockwaveRingMaterial>>,
    blood_materials: ResMut<'w, Assets<BloodSplatterMaterial>>,
    fireball_materials: ResMut<'w, Assets<ExplosionFireballMaterial>>,
    water_exp_materials: ResMut<'w, Assets<WaterExplosionMaterial>>,
    teleport_portal_materials: ResMut<'w, Assets<TeleportPortalMaterial>>,
}

#[derive(SystemParam)]
struct VfxLifetimeQueries<'w, 's> {
    one_shot_q: Query<'w, 's, (Entity, &'static mut OneShotVfxDespawn)>,
    light_q: Query<'w, 's, (Entity, &'static mut PointLight, &'static mut OneShotLightFade)>,
    trail_q: Query<'w, 's, (Entity, &'static mut BulletTrailInstance)>,
    flash_q: Query<'w, 's, (Entity, &'static mut MuzzleFlashInstance)>,
    ring_q: Query<'w, 's, (Entity, &'static mut Transform, &'static mut ShockwaveRingInstance), (Without<BloodGroundDecal>, Without<ExplosionFireballInstance>, Without<WaterExplosionInstance>)>,
    fireball_q: Query<'w, 's, (Entity, &'static mut Transform, &'static mut ExplosionFireballInstance), (Without<BloodGroundDecal>, Without<ShockwaveRingInstance>, Without<WaterExplosionInstance>)>,
    water_exp_q: Query<'w, 's, (Entity, &'static mut Transform, &'static mut WaterExplosionInstance), (Without<BloodGroundDecal>, Without<ShockwaveRingInstance>, Without<ExplosionFireballInstance>)>,
    decal_q: Query<'w, 's, (Entity, &'static mut BloodGroundDecal)>,
}

fn cleanup_ingame_vfx_on_exit_system(
    mut commands: Commands,
    mut queue: ResMut<VfxEventQueue>,
    q: VfxLifetimeQueries,
) {
    queue.0.clear();
    let mut to_despawn = std::collections::HashSet::new();
    for (ent, _) in &q.one_shot_q {
        to_despawn.insert(ent);
    }
    for (ent, _, _) in &q.light_q {
        to_despawn.insert(ent);
    }
    for (ent, _) in &q.trail_q {
        to_despawn.insert(ent);
    }
    for (ent, _) in &q.flash_q {
        to_despawn.insert(ent);
    }
    for (ent, _, _) in &q.ring_q {
        to_despawn.insert(ent);
    }
    for (ent, _, _) in &q.fireball_q {
        to_despawn.insert(ent);
    }
    for (ent, _, _) in &q.water_exp_q {
        to_despawn.insert(ent);
    }
    for (ent, _) in &q.decal_q {
        to_despawn.insert(ent);
    }
    for ent in to_despawn {
        commands.entity(ent).despawn();
    }
}

fn update_vfx_lifetimes_system(
    mut commands: Commands,
    time: Res<Time>,
    world_opt: Option<Res<moba_shared::AuthoritativeWorldState>>,
    mut pool: ResMut<VfxMaterialPool>,
    mut mats: VfxLifetimeMaterials,
    mut q: VfxLifetimeQueries,
) {
    if world_opt
        .as_ref()
        .is_some_and(|w| w.pause_phase != moba_shared::PausePhase::Running)
    {
        return;
    }
    let dt = time.delta();
    let t = time.elapsed_secs();

    for (_, mat) in mats.vanguard_mats.iter_mut() {
        mat.uniforms.params.x = t;
    }
    for (_, mat) in mats.frost_mats.iter_mut() {
        mat.uniforms.params.x = t;
    }
    for (_, mat) in mats.iron_mats.iter_mut() {
        mat.uniforms.params.x = t;
    }
    for (_, mat) in mats.pyro_mats.iter_mut() {
        mat.uniforms.params.x = t;
    }
    for (_, mat) in mats.pistol_mats.iter_mut() {
        mat.uniforms.params.x = t;
    }
    for (_, mat) in mats.kinetic_mats.iter_mut() {
        mat.uniforms.params.x = t;
    }
    for (_, mat) in mats.teleport_portal_materials.iter_mut() {
        mat.uniforms.params.x = t;
    }

    for (ent, mut os) in &mut q.one_shot_q {
        os.timer.tick(dt);
        if os.timer.is_finished() {
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut light, mut fade) in &mut q.light_q {
        fade.timer.tick(dt);
        let rem = (1.0 - fade.timer.fraction()).clamp(0.0, 1.0);
        light.intensity = fade.initial_intensity * rem * rem;
        if fade.timer.is_finished() {
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut tr) in &mut q.trail_q {
        tr.timer.tick(dt);
        let frac = tr.timer.fraction();
        if let Some(mut mat) = mats.trail_materials.get_mut(&tr.mat_handle) {
            mat.uniforms.params.x = frac;
            mat.uniforms.params.y = t;
        }
        if tr.timer.is_finished() {
            if pool.free_trail_mats.len() < MAX_POOLED_TRAIL_MATS {
                pool.free_trail_mats.push(tr.mat_handle.clone());
            }
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut fl) in &mut q.flash_q {
        fl.timer.tick(dt);
        let frac = fl.timer.fraction();
        if let Some(mut mat) = mats.flash_materials.get_mut(&fl.mat_handle) {
            mat.uniforms.params.x = frac;
        }
        if fl.timer.is_finished() {
            if pool.free_flash_mats.len() < MAX_POOLED_FLASH_MATS {
                pool.free_flash_mats.push(fl.mat_handle.clone());
            }
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut tf, mut ring) in &mut q.ring_q {
        ring.timer.tick(dt);
        let frac = ring.timer.fraction();
        let ease = 1.0 - (1.0 - frac).powi(2);
        let s = ring.start_scale.lerp(ring.end_scale, ease);
        tf.scale = Vec3::splat(s);
        if let Some(mut mat) = mats.shockwave_materials.get_mut(&ring.mat_handle) {
            mat.uniforms.params.x = frac;
        }
        if ring.timer.is_finished() {
            if pool.free_ring_mats.len() < MAX_POOLED_RING_MATS {
                pool.free_ring_mats.push(ring.mat_handle.clone());
            }
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut tf, mut fb) in &mut q.fireball_q {
        fb.timer.tick(dt);
        let frac = fb.timer.fraction();
        let ease = 1.0 - (1.0 - frac).powi(3);
        let s = fb.start_scale.lerp(fb.end_scale, ease);
        tf.scale = Vec3::splat(s);
        if let Some(mut mat) = mats.fireball_materials.get_mut(&fb.mat_handle) {
            mat.uniforms.params.x = frac;
        }
        if fb.timer.is_finished() {
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut tf, mut we) in &mut q.water_exp_q {
        we.timer.tick(dt);
        let frac = we.timer.fraction();
        let rise = (frac * std::f32::consts::PI).sin().clamp(0.05, 1.0);
        let h = we.max_height * rise;
        tf.translation.y = we.base_y + h * 0.5;
        tf.scale = Vec3::new(we.radius_scale * (0.7 + frac * 0.6), h, we.radius_scale * (0.7 + frac * 0.6));
        if let Some(mut mat) = mats.water_exp_materials.get_mut(&we.mat_handle) {
            mat.uniforms.params.x = frac;
        }
        if we.timer.is_finished() {
            safe_despawn(&mut commands, ent);
        }
    }

    for (ent, mut dec) in &mut q.decal_q {
        dec.timer.tick(dt);
        let frac = dec.timer.fraction();
        if let Some(mut mat) = mats.blood_materials.get_mut(&dec.mat_handle) {
            mat.uniforms.params.x = frac;
        }
        if dec.timer.is_finished() {
            if pool.free_blood_mats.len() < MAX_POOLED_BLOOD_MATS {
                pool.free_blood_mats.push(dec.mat_handle.clone());
            }
            safe_despawn(&mut commands, ent);
        }
    }
}

// -----------------------------------------------------------------------------
// ساخت مش سه‌بعدی دره عمیق پرپیچ‌وخم (شمال‌غرب به جنوب‌شرق) و سطح رودخانه
// -----------------------------------------------------------------------------

pub fn build_canyon_terrain_and_river_meshes(
    canyon: &moba_shared::CanyonConfig,
    biome: &moba_shared::MapBiomeConfig,
) -> (Mesh, Mesh) {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let nx: usize = canyon.grid_resolution.0.max(32) as usize;
    let nz: usize = canyon.grid_resolution.1.max(32) as usize;
    let min_x = -180.0_f32;
    let max_x = 180.0_f32;
    let min_z = -120.0_f32;
    let max_z = 120.0_f32;
    let dx = (max_x - min_x) / (nx as f32);
    let dz = (max_z - min_z) / (nz as f32);

    let cols = nx + 1;
    let rows = nz + 1;
    let v_count = cols * rows;

    // پیش‌محاسبه فاصله دقیق لبه دره (canyon_edge_distance > 0 یعنی داخل دره) در تمام گره‌های شبکه
    let mut edge_dists: Vec<f32> = Vec::with_capacity(v_count);
    let mut grid_heights: Vec<f32> = Vec::with_capacity(v_count);
    for iz in 0..rows {
        let z = min_z + (iz as f32) * dz;
        for ix in 0..cols {
            let x = min_x + (ix as f32) * dx;
            edge_dists.push(moba_shared::canyon_edge_distance(canyon, x, z));
            grid_heights.push(moba_shared::canyon_sample_height(canyon, x, z));
        }
    }

    let sample_ground_color = |x: f32, z: f32| -> [f32; 4] {
        let signed_side = moba_shared::canyon_river_side_signed(canyon, x, z);
        let autumn_t = ((-signed_side / biome.river_transition_width) * 0.5 + 0.5).clamp(0.0, 1.0);
        let n_scale = biome.ground_noise_scale;
        let patch_n = biome_fbm_2d(x * n_scale, z * n_scale);
        let detail_n = biome_fbm_2d(x * n_scale * 2.7 + 19.3, z * n_scale * 2.7 - 11.7);

        let lp = biome.left_ground_primary_rgb;
        let ls = biome.left_ground_secondary_rgb;
        let lh = biome.left_ground_highlight_rgb;
        let left_r = lp.0 * (1.0 - patch_n) + ls.0 * patch_n;
        let left_g = lp.1 * (1.0 - patch_n) + ls.1 * patch_n;
        let left_b = lp.2 * (1.0 - patch_n) + ls.2 * patch_n;
        let hl_w = (detail_n - 0.52).max(0.0) * 1.45;
        let left_rgb = (
            (left_r + (lh.0 - left_r) * hl_w).clamp(0.0, 1.0),
            (left_g + (lh.1 - left_g) * hl_w).clamp(0.0, 1.0),
            (left_b + (lh.2 - left_b) * hl_w).clamp(0.0, 1.0),
        );

        let rp = biome.right_ground_primary_rgb;
        let rs = biome.right_ground_secondary_rgb;
        let rh = biome.right_ground_highlight_rgb;
        let right_r = rp.0 * (1.0 - patch_n) + rs.0 * patch_n;
        let right_g = rp.1 * (1.0 - patch_n) + rs.1 * patch_n;
        let right_b = rp.2 * (1.0 - patch_n) + rs.2 * patch_n;
        let right_rgb = (
            (right_r + (rh.0 - right_r) * hl_w).clamp(0.0, 1.0),
            (right_g + (rh.1 - right_g) * hl_w).clamp(0.0, 1.0),
            (right_b + (rh.2 - right_b) * hl_w).clamp(0.0, 1.0),
        );

        Color::srgb(
            left_rgb.0 * (1.0 - autumn_t) + right_rgb.0 * autumn_t,
            left_rgb.1 * (1.0 - autumn_t) + right_rgb.1 * autumn_t,
            left_rgb.2 * (1.0 - autumn_t) + right_rgb.2 * autumn_t,
        )
        .to_linear()
        .to_f32_array()
    };

    let sample_cliff_color = |x: f32, y: f32, z: f32| -> [f32; 4] {
        let signed_side = moba_shared::canyon_river_side_signed(canyon, x, z);
        let autumn_t = ((-signed_side / biome.river_transition_width) * 0.5 + 0.5).clamp(0.0, 1.0);
        let rock_t = ((-y) / canyon.water_y.abs().max(1.0)).clamp(0.0, 1.0);
        let warm_shift = autumn_t * 0.06;
        let detail = biome_fbm_2d(x * 0.14 + y * 0.18, z * 0.14 - y * 0.12) * 0.07 - 0.035;
        Color::srgb(
            (0.39 - 0.15 * rock_t + warm_shift + detail).clamp(0.0, 1.0),
            (0.36 - 0.11 * rock_t - warm_shift * 0.25 + detail).clamp(0.0, 1.0),
            (0.31 - 0.06 * rock_t - warm_shift * 0.5 + detail).clamp(0.0, 1.0),
        )
        .to_linear()
        .to_f32_array()
    };

    let bed_color = Color::srgb(0.14, 0.26, 0.24).to_linear().to_f32_array();
    let sample_bed_y = |x: f32, z: f32| -> f32 {
        let bed_ripple = 0.25 * (x * 0.22).sin() * (z * 0.22).cos();
        canyon.floor_depth + bed_ripple
    };

    // یافتن نقطه دقیق منحنی صفر (canyon_edge_distance == 0.0) روی هر یال با ۴ مرحله Bisection
    let find_exact_contour_point = |a: Vec2, da: f32, b: Vec2, db: f32| -> Vec2 {
        let mut lo_p = a;
        let mut hi_p = b;
        let mut lo_d = da;
        let mut hi_d = db;
        // گام اول: درون‌یابی خطی
        let denom = hi_d - lo_d;
        let t0 = if denom.abs() > 1e-6 {
            (-lo_d / denom).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let mut mid_p = lo_p.lerp(hi_p, t0);
        for _ in 0..4 {
            let md = moba_shared::canyon_edge_distance(canyon, mid_p.x, mid_p.y);
            if (lo_d <= 0.0) == (md <= 0.0) {
                lo_p = mid_p;
                lo_d = md;
            } else {
                hi_p = mid_p;
                hi_d = md;
            }
            mid_p = (lo_p + hi_p) * 0.5;
        }
        mid_p
    };

    // محاسبه نرمال نرم و پیوسته دیواره دره رو به داخل رودخانه (بدون شکستگی پله‌ای)
    let sample_cliff_normal = |x: f32, z: f32| -> [f32; 3] {
        let g_eps = 0.40_f32;
        let gx = moba_shared::canyon_edge_distance(canyon, x + g_eps, z)
            - moba_shared::canyon_edge_distance(canyon, x - g_eps, z);
        let gz = moba_shared::canyon_edge_distance(canyon, x, z + g_eps)
            - moba_shared::canyon_edge_distance(canyon, x, z - g_eps);
        let n = Vec3::new(gx, 0.14, gz).normalize_or_zero();
        if n == Vec3::ZERO {
            [0.0, 1.0, 0.0]
        } else {
            [n.x, n.y, n.z]
        }
    };

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(v_count * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(v_count * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(v_count * 2);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(v_count * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(nx * nz * 8);

    let push_vert = |positions: &mut Vec<[f32; 3]>,
                     normals: &mut Vec<[f32; 3]>,
                     uvs: &mut Vec<[f32; 2]>,
                     colors: &mut Vec<[f32; 4]>,
                     pos: [f32; 3],
                     norm: [f32; 3],
                     col: [f32; 4]|
     -> u32 {
        let idx = positions.len() as u32;
        positions.push(pos);
        normals.push(norm);
        let u = ((pos[0] - min_x) / (max_x - min_x)).clamp(0.0, 1.0);
        let v = ((pos[2] - min_z) / (max_z - min_z)).clamp(0.0, 1.0);
        uvs.push([u, v]);
        colors.push(col);
        idx
    };

    // مثلث‌بندی دقیق یک سلول مرزی با Marching Squares (برش دقیق منحنی لبه دره + ساخت دیواره منحنی صاف)
    let emit_cell_exact = |positions: &mut Vec<[f32; 3]>,
                           normals: &mut Vec<[f32; 3]>,
                           uvs: &mut Vec<[f32; 2]>,
                           colors: &mut Vec<[f32; 4]>,
                           indices: &mut Vec<u32>,
                           x0: f32,
                           z0: f32,
                           x1: f32,
                           z1: f32| {
        // ترتیب CCW از دید بالا (+Y): (x0,z0) -> (x0,z1) -> (x1,z1) -> (x1,z0)
        let c = [
            Vec2::new(x0, z0),
            Vec2::new(x0, z1),
            Vec2::new(x1, z1),
            Vec2::new(x1, z0),
        ];
        let d = [
            moba_shared::canyon_edge_distance(canyon, c[0].x, c[0].y),
            moba_shared::canyon_edge_distance(canyon, c[1].x, c[1].y),
            moba_shared::canyon_edge_distance(canyon, c[2].x, c[2].y),
            moba_shared::canyon_edge_distance(canyon, c[3].x, c[3].y),
        ];
        let inside = [d[0] > 0.0, d[1] > 0.0, d[2] > 0.0, d[3] > 0.0];
        let in_count = inside.iter().filter(|&&b| b).count();

        if in_count == 0 {
            // سلول کاملاً روی فلات بالا (y = 0)
            let v0 = push_vert(positions, normals, uvs, colors, [c[0].x, 0.0, c[0].y], [0.0, 1.0, 0.0], sample_ground_color(c[0].x, c[0].y));
            let v1 = push_vert(positions, normals, uvs, colors, [c[1].x, 0.0, c[1].y], [0.0, 1.0, 0.0], sample_ground_color(c[1].x, c[1].y));
            let v2 = push_vert(positions, normals, uvs, colors, [c[2].x, 0.0, c[2].y], [0.0, 1.0, 0.0], sample_ground_color(c[2].x, c[2].y));
            let v3 = push_vert(positions, normals, uvs, colors, [c[3].x, 0.0, c[3].y], [0.0, 1.0, 0.0], sample_ground_color(c[3].x, c[3].y));
            indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
            return;
        }
        if in_count == 4 {
            // سلول کاملاً در بستر دره
            let v0 = push_vert(positions, normals, uvs, colors, [c[0].x, sample_bed_y(c[0].x, c[0].y), c[0].y], [0.0, 1.0, 0.0], bed_color);
            let v1 = push_vert(positions, normals, uvs, colors, [c[1].x, sample_bed_y(c[1].x, c[1].y), c[1].y], [0.0, 1.0, 0.0], bed_color);
            let v2 = push_vert(positions, normals, uvs, colors, [c[2].x, sample_bed_y(c[2].x, c[2].y), c[2].y], [0.0, 1.0, 0.0], bed_color);
            let v3 = push_vert(positions, normals, uvs, colors, [c[3].x, sample_bed_y(c[3].x, c[3].y), c[3].y], [0.0, 1.0, 0.0], bed_color);
            indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
            return;
        }

        // جمع‌آوری چندضلعی بیرون (فلات) و درون (بستر دره) و نقاط عبور دقیق منحنی
        let mut out_poly: Vec<Vec2> = Vec::with_capacity(6);
        let mut in_poly: Vec<Vec2> = Vec::with_capacity(6);
        let mut crossings: Vec<Vec2> = Vec::with_capacity(4);

        for i in 0..4 {
            let j = (i + 1) % 4;
            if !inside[i] {
                out_poly.push(c[i]);
            } else {
                in_poly.push(c[i]);
            }
            if inside[i] != inside[j] {
                let cp = find_exact_contour_point(c[i], d[i], c[j], d[j]);
                out_poly.push(cp);
                in_poly.push(cp);
                crossings.push(cp);
            }
        }


        // ۱. مثلث‌بندی بخش فلات بالای سلول (دقیقاً بریده‌شده روی منحنی گرد لبه دره)
        if out_poly.len() >= 3 {
            let mut ov: Vec<u32> = Vec::with_capacity(out_poly.len());
            for p in &out_poly {
                ov.push(push_vert(
                    positions,
                    normals,
                    uvs,
                    colors,
                    [p.x, 0.0, p.y],
                    [0.0, 1.0, 0.0],
                    sample_ground_color(p.x, p.y),
                ));
            }
            for k in 1..(ov.len() - 1) {
                indices.extend_from_slice(&[ov[0], ov[k], ov[k + 1]]);
            }
        }

        // ۲. مثلث‌بندی بخش بستر رودخانه سلول
        if in_poly.len() >= 3 {
            let mut iv: Vec<u32> = Vec::with_capacity(in_poly.len());
            for p in &in_poly {
                iv.push(push_vert(
                    positions,
                    normals,
                    uvs,
                    colors,
                    [p.x, sample_bed_y(p.x, p.y), p.y],
                    [0.0, 1.0, 0.0],
                    bed_color,
                ));
            }
            for k in 1..(iv.len() - 1) {
                indices.extend_from_slice(&[iv[0], iv[k], iv[k + 1]]);
            }
        }

        // ۳. ساخت دیواره عمودی صاف و منحنی دقیقاً منطبق بر منحنی فیزیکی لبه دره (بدون هیچ پله‌پله شدن)
        if crossings.len() >= 2 {
            let mut pa = crossings[0];
            let mut pb = crossings[1];
            let mid = (pa + pb) * 0.5;
            let mid_n = Vec3::from_array(sample_cliff_normal(mid.x, mid.y));
            let top_a = Vec3::new(pa.x, 0.0, pa.y);
            let top_b = Vec3::new(pb.x, 0.0, pb.y);
            let bot_a = Vec3::new(pa.x, sample_bed_y(pa.x, pa.y), pa.y);
            // اطمینان قطعی از جهت روبه‌داخل رودخانه (CCW) تا هیچ دیواره‌ای پشت‌ورو نشود
            if (bot_a - top_a).cross(top_b - top_a).dot(mid_n) < 0.0 {
                std::mem::swap(&mut pa, &mut pb);
            }

            let na = sample_cliff_normal(pa.x, pa.y);
            let nb = sample_cliff_normal(pb.x, pb.y);
            let y_mid = canyon.water_y.clamp(canyon.floor_depth + 1.0, -1.0);
            let y_ba = sample_bed_y(pa.x, pa.y);
            let y_bb = sample_bed_y(pb.x, pb.y);

            // دو نوار عمودی (بالای آب + زیر آب) برای سایه‌زنی و گرادیان سنگی طبیعی
            let ta = push_vert(positions, normals, uvs, colors, [pa.x, 0.0, pa.y], na, sample_cliff_color(pa.x, 0.0, pa.y));
            let tb = push_vert(positions, normals, uvs, colors, [pb.x, 0.0, pb.y], nb, sample_cliff_color(pb.x, 0.0, pb.y));
            let ma = push_vert(positions, normals, uvs, colors, [pa.x, y_mid, pa.y], na, sample_cliff_color(pa.x, y_mid, pa.y));
            let mb = push_vert(positions, normals, uvs, colors, [pb.x, y_mid, pb.y], nb, sample_cliff_color(pb.x, y_mid, pb.y));
            let ba = push_vert(positions, normals, uvs, colors, [pa.x, y_ba, pa.y], na, bed_color);
            let bb = push_vert(positions, normals, uvs, colors, [pb.x, y_bb, pb.y], nb, bed_color);

            indices.extend_from_slice(&[ta, ma, tb, tb, ma, mb]);
            indices.extend_from_slice(&[ma, ba, mb, mb, ba, bb]);
        }
    };

    // اشتراک‌گذاری رئوس برای سلول‌های کاملاً غیرمرزی جهت سبکی حافظه + برش دقیق منحنی برای سلول‌های مرزی دره
    let mut flat_vert_map = vec![u32::MAX; v_count];
    for iz in 0..nz {
        let z0 = min_z + (iz as f32) * dz;
        let z1 = z0 + dz;
        for ix in 0..nx {
            let x0 = min_x + (ix as f32) * dx;
            let x1 = x0 + dx;
            let idx00 = iz * cols + ix;
            let idx10 = idx00 + 1;
            let idx01 = (iz + 1) * cols + ix;
            let idx11 = idx01 + 1;

            let in00 = edge_dists[idx00] > 0.0;
            let in10 = edge_dists[idx10] > 0.0;
            let in01 = edge_dists[idx01] > 0.0;
            let in11 = edge_dists[idx11] > 0.0;

            if in00 == in10 && in10 == in01 && in01 == in11 {
                // سلول همگن (کاملاً فلات یا کاملاً کف رودخانه) -> اشتراک رئوس سریع
                for &vidx in &[idx00, idx10, idx01, idx11] {
                    if flat_vert_map[vidx] == u32::MAX {
                        let vx = min_x + ((vidx % cols) as f32) * dx;
                        let vz = min_z + ((vidx / cols) as f32) * dz;
                        let vy = grid_heights[vidx];
                        let col = if edge_dists[vidx] > 0.0 {
                            bed_color
                        } else {
                            sample_ground_color(vx, vz)
                        };
                        flat_vert_map[vidx] = push_vert(
                            &mut positions,
                            &mut normals,
                            &mut uvs,
                            &mut colors,
                            [vx, vy, vz],
                            [0.0, 1.0, 0.0],
                            col,
                        );
                    }
                }
                let v00 = flat_vert_map[idx00];
                let v10 = flat_vert_map[idx10];
                let v01 = flat_vert_map[idx01];
                let v11 = flat_vert_map[idx11];
                indices.extend_from_slice(&[v00, v01, v10, v10, v01, v11]);
            } else {
                // سلول مرزی لبه دره: زیرتقسیم ۲×۲ برای منحنی کاملاً گرد و یکدست و رفع کامل حالت زینی
                let xm = (x0 + x1) * 0.5;
                let zm = (z0 + z1) * 0.5;
                emit_cell_exact(&mut positions, &mut normals, &mut uvs, &mut colors, &mut indices, x0, z0, xm, zm);
                emit_cell_exact(&mut positions, &mut normals, &mut uvs, &mut colors, &mut indices, xm, z0, x1, zm);
                emit_cell_exact(&mut positions, &mut normals, &mut uvs, &mut colors, &mut indices, x0, zm, xm, z1);
                emit_cell_exact(&mut positions, &mut normals, &mut uvs, &mut colors, &mut indices, xm, zm, x1, z1);
            }
        }
    }

    let mut terrain_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    terrain_mesh.insert_indices(Indices::U32(indices));

    let water_y = canyon.water_y;
    let mut w_positions: Vec<[f32; 3]> = Vec::new();
    let mut w_normals: Vec<[f32; 3]> = Vec::new();
    let mut w_uvs: Vec<[f32; 2]> = Vec::new();
    let mut w_indices: Vec<u32> = Vec::new();
    let mut w_vert_map = vec![u32::MAX; v_count];

    for iz in 0..nz {
        for ix in 0..nx {
            let idx00 = iz * cols + ix;
            let idx10 = idx00 + 1;
            let idx01 = (iz + 1) * cols + ix;
            let idx11 = idx01 + 1;
            let min_h = grid_heights[idx00]
                .min(grid_heights[idx10])
                .min(grid_heights[idx01])
                .min(grid_heights[idx11]);
            if min_h >= water_y + 0.40 {
                continue;
            }
            for &vidx in &[idx00, idx10, idx01, idx11] {
                if w_vert_map[vidx] == u32::MAX {
                    let vx = min_x + ((vidx % cols) as f32) * dx;
                    let vz = min_z + ((vidx / cols) as f32) * dz;
                    let depth_norm = (moba_shared::canyon_edge_distance(canyon, vx, vz) / 10.0).clamp(0.0, 1.0);
                    w_vert_map[vidx] = w_positions.len() as u32;
                    w_positions.push([vx, water_y, vz]);
                    w_normals.push([0.0, 1.0, 0.0]);
                    w_uvs.push([depth_norm, 0.0]);
                }
            }
            let w00 = w_vert_map[idx00];
            let w10 = w_vert_map[idx10];
            let w01 = w_vert_map[idx01];
            let w11 = w_vert_map[idx11];
            w_indices.extend_from_slice(&[w00, w01, w10, w10, w01, w11]);
        }
    }

    let mut river_mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    river_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, w_positions);
    river_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, w_normals);
    river_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, w_uvs);
    river_mesh.insert_indices(Indices::U32(w_indices));

    (terrain_mesh, river_mesh)
}

/// ساخت مش سه‌بعدی منحنی و طبیعی برای آبشار (دارای قوس ریزش از لبه کوه و موج‌دار بودن عرضی)
pub fn build_curved_waterfall_mesh() -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let cols: usize = 36;
    let rows: usize = 24;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((cols + 1) * (rows + 1) * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((cols + 1) * (rows + 1) * 2);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((cols + 1) * (rows + 1) * 2);
    let mut indices: Vec<u32> = Vec::with_capacity(cols * rows * 12);

    for side in 0..2 {
        let base_idx = positions.len() as u32;
        let z_off = if side == 0 { 0.04 } else { -0.04 };
        let n_sign = if side == 0 { 1.0_f32 } else { -1.0_f32 };

        for iy in 0..=rows {
            let v = (iy as f32) / (rows as f32);
            let y = 0.5 - v;
            let arc_z = -0.50 + 1.05 * (1.0 - (1.0 - v).powi(2));
            let dz_dv = 2.10 * (1.0 - v);

            for ix in 0..=cols {
                let u = (ix as f32) / (cols as f32);
                let x = u - 0.5;
                let rock_wave = (0.14 * (x * 8.5).sin() + 0.07 * (x * 19.0).cos()) * (1.0 - v * 0.45);
                let z = arc_z + rock_wave + z_off;

                let n = Vec3::new(0.0, dz_dv, 1.0).normalize() * n_sign;
                positions.push([x, y, z]);
                normals.push([n.x, n.y, n.z]);
                uvs.push([u, v]);
            }
        }

        let stride = (cols + 1) as u32;
        for iy in 0..rows {
            for ix in 0..cols {
                let i00 = base_idx + (iy as u32) * stride + (ix as u32);
                let i10 = i00 + 1;
                let i01 = i00 + stride;
                let i11 = i01 + 1;
                if side == 0 {
                    indices.extend_from_slice(&[i00, i01, i10, i10, i01, i11]);
                } else {
                    indices.extend_from_slice(&[i00, i10, i01, i10, i11, i01]);
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[derive(ShaderType, Debug, Clone)]
pub struct WeatherShaderUniforms {
    pub color: Vec4,
    pub params: Vec4,
    pub shape: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct WeatherRainMaterial {
    #[uniform(0)]
    pub uniforms: WeatherShaderUniforms,
}

impl Material for WeatherRainMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/weather_rain.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct WeatherSnowMaterial {
    #[uniform(0)]
    pub uniforms: WeatherShaderUniforms,
}

impl Material for WeatherSnowMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/weather_snow.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Resource, Default)]
pub struct WeatherVfxState {
    pub active_weather: Option<WeatherKind>,
    pub camera_focus: Vec3,
}

#[derive(Component)]
struct WeatherRainMesh {
    fade: f32,
}

#[derive(Component)]
struct WeatherRainGroundMesh;

fn build_weather_terrain_splash_mesh(canyon: &moba_shared::CanyonConfig) -> Mesh {
    let nx: usize = 96;
    let nz: usize = 72;
    let min_x = -160.0_f32;
    let max_x = 160.0_f32;
    let min_z = -110.0_f32;
    let max_z = 110.0_f32;
    let dx = (max_x - min_x) / (nx as f32);
    let dz = (max_z - min_z) / (nz as f32);

    let cols = nx + 1;
    let rows = nz + 1;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(cols * rows);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(cols * rows);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(cols * rows);
    let mut heights: Vec<f32> = Vec::with_capacity(cols * rows);

    for iz in 0..rows {
        let z = min_z + (iz as f32) * dz;
        for ix in 0..cols {
            let x = min_x + (ix as f32) * dx;
            let y = moba_shared::canyon_sample_height(canyon, x, z);
            heights.push(y);
            positions.push([x, y + 0.05, z]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push([0.5, 0.5]);
        }
    }

    let water_cutoff = canyon.water_y + 0.18;
    let mut indices: Vec<u32> = Vec::new();
    for iz in 0..nz {
        for ix in 0..nx {
            let i00 = iz * cols + ix;
            let i10 = i00 + 1;
            let i01 = i00 + cols;
            let i11 = i01 + 1;
            if heights[i00] > water_cutoff
                && heights[i10] > water_cutoff
                && heights[i01] > water_cutoff
                && heights[i11] > water_cutoff
            {
                indices.extend_from_slice(&[
                    i00 as u32, i01 as u32, i10 as u32,
                    i10 as u32, i01 as u32, i11 as u32,
                ]);
            }
        }
    }

    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

#[derive(Component)]
struct WeatherSnowMesh {
    fade: f32,
}

fn build_weather_curtain_mesh(width: f32, height: f32, include_ground_splashes: bool) -> Mesh {
    // چند صفحه مورب رو به دوربین ایزومتریک (با دو رویه برای دید کامل)
    let hw = width * 0.5;
    let hh = height * 0.5;
    let offsets = [
        Vec3::new(0.0, 9.5, 4.0),
        Vec3::new(0.0, 12.5, -2.5),
    ];
    let tilt_rad: f32 = -0.58;
    let rot = Quat::from_rotation_x(tilt_rad);

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for (layer_idx, off) in offsets.iter().enumerate() {
        let base = positions.len() as u32;
        let corners = [
            Vec3::new(-hw, -hh, 0.0),
            Vec3::new(hw, -hh, 0.0),
            Vec3::new(hw, hh, 0.0),
            Vec3::new(-hw, hh, 0.0),
        ];
        let u_shift = (layer_idx as f32) * 0.31;
        let layer_uvs = [
            [0.0 + u_shift, 0.0],
            [1.0 + u_shift, 0.0],
            [1.0 + u_shift, 1.0],
            [0.0 + u_shift, 1.0],
        ];
        let n = (rot * Vec3::Z).to_array();
        for i in 0..4 {
            let p = *off + rot * corners[i];
            positions.push(p.to_array());
            normals.push(n);
            uvs.push(layer_uvs[i]);
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    if include_ground_splashes {
        let base = positions.len() as u32;
        let gy = 0.08_f32;
        let gw = width * 0.48;
        let gd = height * 0.65;
        let g_corners = [
            [-gw, gy, -gd],
            [gw, gy, -gd],
            [gw, gy, gd],
            [-gw, gy, gd],
        ];
        let g_uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        for i in 0..4 {
            positions.push(g_corners[i]);
            normals.push([0.0, 1.0, 0.0]);
            uvs.push(g_uvs[i]);
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

fn update_weather_vfx_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    weather_state: Res<WeatherVfxState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut rain_mats: ResMut<Assets<WeatherRainMaterial>>,
    mut snow_mats: ResMut<Assets<WeatherSnowMaterial>>,
    mut rain_q: Query<(Entity, &mut WeatherRainMesh, &MeshMaterial3d<WeatherRainMaterial>, &mut Transform), (Without<WeatherSnowMesh>, Without<WeatherRainGroundMesh>)>,
    rain_ground_q: Query<Entity, (With<WeatherRainGroundMesh>, Without<WeatherRainMesh>, Without<WeatherSnowMesh>)>,
    mut snow_q: Query<(Entity, &mut WeatherSnowMesh, &MeshMaterial3d<WeatherSnowMaterial>, &mut Transform), Without<WeatherRainMesh>>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let w = &config.weather;
    let fade_rate = dt / w.transition_duration_secs.max(0.1);
    let focus = Vec3::new(weather_state.camera_focus.x, 0.0, weather_state.camera_focus.z);

    let want_rain = weather_state.active_weather == Some(WeatherKind::Rainy);
    if want_rain && rain_q.is_empty() {
        let mesh = meshes.add(build_weather_curtain_mesh(54.0, 32.0, false));
        let c = w.rain_drop_color_rgba;
        let mat = rain_mats.add(WeatherRainMaterial {
            uniforms: WeatherShaderUniforms {
                color: Vec4::new(c.0, c.1, c.2, c.3),
                params: Vec4::new(t, w.rain_fall_speed, w.rain_wind_tilt, w.rain_density),
                shape: Vec4::new(w.rain_streak_width, w.rain_streak_length, 0.0, 1.37),
            },
        });
        commands.spawn((
            WeatherRainMesh { fade: 0.0 },
            Mesh3d(mesh),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(focus),
            NotShadowCaster,
        ));
        if rain_ground_q.is_empty() {
            let g_mesh = meshes.add(build_weather_terrain_splash_mesh(&config.map.canyon));
            commands.spawn((
                WeatherRainGroundMesh,
                Mesh3d(g_mesh),
                MeshMaterial3d(mat),
                Transform::IDENTITY,
                NotShadowCaster,
            ));
        }
    }
    for (ent, mut rm, mat_h, mut tf) in &mut rain_q {
        tf.translation = focus;
        if want_rain {
            rm.fade = (rm.fade + fade_rate).min(1.0);
        } else {
            rm.fade = (rm.fade - fade_rate).max(0.0);
            if rm.fade <= 0.001 {
                safe_despawn(&mut commands, ent);
                for ge in &rain_ground_q {
                    safe_despawn(&mut commands, ge);
                }
                continue;
            }
        }
        if let Some(mut mat) = rain_mats.get_mut(&mat_h.0) {
            mat.uniforms.params.x = t;
            mat.uniforms.shape.z = rm.fade;
        }
    }

    let want_snow = weather_state.active_weather == Some(WeatherKind::Snowy);
    if want_snow && snow_q.is_empty() {
        let mesh = meshes.add(build_weather_curtain_mesh(56.0, 32.0, false));
        let c = w.snow_flake_color_rgba;
        let mat = snow_mats.add(WeatherSnowMaterial {
            uniforms: WeatherShaderUniforms {
                color: Vec4::new(c.0, c.1, c.2, c.3),
                params: Vec4::new(t, w.snow_fall_speed, w.snow_sway_amp, w.snow_density),
                shape: Vec4::new(w.snow_flake_size, w.snow_flake_softness, 0.0, 2.71),
            },
        });
        commands.spawn((
            WeatherSnowMesh { fade: 0.0 },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(focus),
            NotShadowCaster,
        ));
    }
    for (ent, mut sm, mat_h, mut tf) in &mut snow_q {
        tf.translation = focus;
        if want_snow {
            sm.fade = (sm.fade + fade_rate).min(1.0);
        } else {
            sm.fade = (sm.fade - fade_rate).max(0.0);
            if sm.fade <= 0.001 {
                safe_despawn(&mut commands, ent);
                continue;
            }
        }
        if let Some(mut mat) = snow_mats.get_mut(&mat_h.0) {
            mat.uniforms.params.x = t;
            mat.uniforms.shape.z = sm.fade;
        }
    }
}

#[inline]
fn safe_despawn(commands: &mut Commands, entity: Entity) {
    commands.queue(move |world: &mut World| {
        if let Ok(em) = world.get_entity_mut(entity) {
            em.despawn();
        }
    });
}
// =============================================================================
// سیستم افکت‌های سه‌بعدی WESL برای تمام آیتم‌های تاکتیکی (vfx_tactical_aura.wesl)
// =============================================================================

#[derive(ShaderType, Debug, Clone)]
pub struct TacticalAuraUniforms {
    pub primary_color: Vec4,
    pub secondary_color: Vec4,
    pub params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct TacticalAuraMaterial {
    #[uniform(0)]
    pub uniforms: TacticalAuraUniforms,
}

impl Material for TacticalAuraMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/vfx_tactical_aura.wesl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Clone, Debug)]
pub struct HeroTacticalAuraSnapshot {
    pub slot_index: usize,
    pub pos: Vec3,
    pub is_dead: bool,
    pub is_reflect_armor_active: bool,
    pub is_bullet_immune: bool,
    pub is_cryo_debuffed: bool,
    pub is_null_muted: bool,
    pub is_sonar_revealed: bool,
}

#[derive(Resource, Default)]
pub struct TacticalAuraSyncState {
    pub heroes: Vec<HeroTacticalAuraSnapshot>,
    pub pending_reflect_pulses: Vec<usize>,
    pub pending_lifesteal_heals: Vec<Vec3>,
    pub pending_tactical_pulses: Vec<(Vec3, f32, (f32, f32, f32))>,
}

#[derive(Component)]
struct HeroPersistentAuraVfx {
    slot_index: usize,
    kind: u8, // 0=ReflectDome, 1=PhaseDome, 3=CryoRing, 4=NullRing, 5=SonarRing
    hit_pulse: f32,
}

#[derive(Component)]
struct TransientTacticalAuraVfx {
    elapsed: f32,
    duration: f32,
    mode: f32,
    start_scale: f32,
    end_scale: f32,
    rise_speed: f32,
}

#[allow(clippy::too_many_arguments)]
fn update_tactical_auras_vfx_system(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut aura_mats: ResMut<Assets<TacticalAuraMaterial>>,
    mut sync: ResMut<TacticalAuraSyncState>,
    mut persistent_q: Query<
        (
            Entity,
            &mut HeroPersistentAuraVfx,
            &mut Transform,
            &MeshMaterial3d<TacticalAuraMaterial>,
        ),
        Without<TransientTacticalAuraVfx>,
    >,
    mut transient_q: Query<
        (
            Entity,
            &mut TransientTacticalAuraVfx,
            &mut Transform,
            &MeshMaterial3d<TacticalAuraMaterial>,
        ),
        Without<HeroPersistentAuraVfx>,
    >,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    // ۱. اعمال پمپاژ لحظه‌ای شیلد مغناطیسی Aegis Mirror هنگام برخورد تیر
    let reflect_pulses: Vec<usize> = sync.pending_reflect_pulses.drain(..).collect();
    for (_, mut aura, _, _) in &mut persistent_q {
        if aura.kind == 0 && reflect_pulses.contains(&aura.slot_index) {
            aura.hit_pulse = 1.0;
        }
    }

    // ۲. افکت سبز صعودی Lifesteal (مشابه Dota 2)
    let heal_positions: Vec<Vec3> = sync.pending_lifesteal_heals.drain(..).collect();
    for pos in heal_positions {
        let mesh = meshes.add(Cylinder::new(0.68, 1.75));
        let mat = aura_mats.add(TacticalAuraMaterial {
            uniforms: TacticalAuraUniforms {
                primary_color: Vec4::new(0.14, 0.96, 0.36, 1.0),
                secondary_color: Vec4::new(0.68, 1.0, 0.72, 1.0),
                params: Vec4::new(t, 1.0, 0.0, 2.0),
            },
        });
        commands.spawn((
            TransientTacticalAuraVfx {
                elapsed: 0.0,
                duration: 0.52,
                mode: 2.0,
                start_scale: 0.92,
                end_scale: 1.18,
                rise_speed: 0.75,
            },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos + Vec3::Y * 0.95),
            NotShadowCaster,
        ));
    }

    // ۳. موج‌های انفجاری WESL برای NullPulse / SonarReveal / KineticDash
    let pulse_events: Vec<(Vec3, f32, (f32, f32, f32))> =
        sync.pending_tactical_pulses.drain(..).collect();
    for (pos, radius, col) in pulse_events {
        let mesh = meshes.add(Plane3d::default().mesh().size(2.0, 2.0));
        let mat = aura_mats.add(TacticalAuraMaterial {
            uniforms: TacticalAuraUniforms {
                primary_color: Vec4::new(col.0, col.1, col.2, 1.0),
                secondary_color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                params: Vec4::new(t, 1.0, 0.0, 6.0),
            },
        });
        commands.spawn((
            TransientTacticalAuraVfx {
                elapsed: 0.0,
                duration: 0.58,
                mode: 6.0,
                start_scale: 0.8,
                end_scale: radius.max(2.0),
                rise_speed: 0.0,
            },
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos + Vec3::Y * 0.14),
            NotShadowCaster,
        ));
    }

    // ۴. همگام‌سازی شیلدها و حلقه‌های پایدار روی تمام هیروها (لوکال و ریموت)
    let mut existing_keys: Vec<(usize, u8)> = Vec::new();
    for (ent, mut aura, mut tf, mat_h) in &mut persistent_q {
        let hero_opt = sync.heroes.iter().find(|h| h.slot_index == aura.slot_index);
        let still_active = hero_opt.is_some_and(|h| {
            !h.is_dead
                && match aura.kind {
                    0 => h.is_reflect_armor_active,
                    1 => h.is_bullet_immune,
                    3 => h.is_cryo_debuffed,
                    4 => h.is_null_muted,
                    5 => h.is_sonar_revealed,
                    _ => false,
                }
        });
        if !still_active {
            safe_despawn(&mut commands, ent);
            continue;
        }
        existing_keys.push((aura.slot_index, aura.kind));
        let hpos = hero_opt.unwrap().pos;
        aura.hit_pulse = (aura.hit_pulse - dt * 5.8).max(0.0);

        match aura.kind {
            0 => {
                // شیلد مغناطیسی Aegis Mirror + پمپاژ لحظه‌ای هنگام برخورد تیر
                let pump = 1.0 + aura.hit_pulse * 0.24;
                tf.translation = hpos + Vec3::Y * 1.05;
                tf.scale = Vec3::splat(pump);
                tf.rotation = Quat::from_rotation_y(t * 1.4);
            }
            1 => {
                // گنبد طلایی Quantum Phase Shield
                tf.translation = hpos + Vec3::Y * 1.05;
                tf.scale = Vec3::splat(1.0 + (t * 4.5).sin() * 0.025);
                tf.rotation = Quat::from_rotation_y(-t * 1.1);
            }
            3 => {
                tf.translation = hpos + Vec3::Y * 0.10;
            }
            4 => {
                tf.translation = hpos + Vec3::Y * 1.15;
            }
            5 => {
                tf.translation = hpos + Vec3::Y * 0.14;
            }
            _ => {}
        }

        if let Some(mut mat) = aura_mats.get_mut(&mat_h.0) {
            mat.uniforms.params.x = t;
            mat.uniforms.params.y = 1.0;
            mat.uniforms.params.z = aura.hit_pulse;
        }
    }

    for h in &sync.heroes {
        if h.is_dead {
            continue;
        }
        let specs: [(u8, bool, Vec4, Vec4, Vec3, bool); 5] = [
            (
                0,
                h.is_reflect_armor_active,
                Vec4::new(1.0, 0.44, 0.12, 1.0),
                Vec4::new(1.0, 0.88, 0.48, 1.0),
                Vec3::Y * 1.05,
                true,
            ),
            (
                1,
                h.is_bullet_immune,
                Vec4::new(1.0, 0.86, 0.24, 1.0),
                Vec4::new(1.0, 0.99, 0.88, 1.0),
                Vec3::Y * 1.05,
                true,
            ),
            (
                3,
                h.is_cryo_debuffed,
                Vec4::new(0.22, 0.84, 1.0, 1.0),
                Vec4::new(0.82, 0.98, 1.0, 1.0),
                Vec3::Y * 0.10,
                false,
            ),
            (
                4,
                h.is_null_muted,
                Vec4::new(0.96, 0.22, 0.88, 1.0),
                Vec4::new(1.0, 0.72, 0.98, 1.0),
                Vec3::Y * 1.15,
                false,
            ),
            (
                5,
                h.is_sonar_revealed,
                Vec4::new(1.0, 0.80, 0.20, 1.0),
                Vec4::new(1.0, 0.96, 0.72, 1.0),
                Vec3::Y * 0.14,
                false,
            ),
        ];

        for (kind, active, c1, c2, offset, is_sphere) in specs {
            if !active || existing_keys.contains(&(h.slot_index, kind)) {
                continue;
            }
            let mesh = if is_sphere {
                let rad = if kind == 1 { 1.24 } else { 1.16 };
                meshes.add(Sphere::new(rad).mesh().uv(24, 14))
            } else {
                meshes.add(Plane3d::default().mesh().size(2.15, 2.15))
            };
            let mat = aura_mats.add(TacticalAuraMaterial {
                uniforms: TacticalAuraUniforms {
                    primary_color: c1,
                    secondary_color: c2,
                    params: Vec4::new(t, 1.0, 0.0, kind as f32),
                },
            });
            commands.spawn((
                HeroPersistentAuraVfx {
                    slot_index: h.slot_index,
                    kind,
                    hit_pulse: 0.0,
                },
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_translation(h.pos + offset),
                NotShadowCaster,
            ));
        }
    }

    // ۵. به‌روزرسانی عمر افکت‌های گذرا (لایف‌استیل سبز و موج‌های EMP/Sonar)
    for (ent, mut tr, mut tf, mat_h) in &mut transient_q {
        tr.elapsed += dt;
        let frac = (tr.elapsed / tr.duration.max(0.01)).clamp(0.0, 1.0);
        if frac >= 1.0 {
            safe_despawn(&mut commands, ent);
            continue;
        }
        let fade = 1.0 - frac;
        let cur_scale = tr.start_scale + (tr.end_scale - tr.start_scale) * frac;
        if tr.mode > 5.5 {
            tf.scale = Vec3::new(cur_scale, 1.0, cur_scale);
        } else {
            tf.scale = Vec3::splat(cur_scale);
            tf.translation.y += tr.rise_speed * dt;
        }
        if let Some(mut mat) = aura_mats.get_mut(&mat_h.0) {
            mat.uniforms.params.x = t;
            mat.uniforms.params.y = fade;
        }
    }
}

// =============================================================================
// سیستم بیوم دو نیمه نقشه (چمنزار متحرک سمت چپ + برگ‌های پاییزی تعاملی سمت راست)
// =============================================================================

#[inline]
fn biome_hash21(x: f32, z: f32) -> f32 {
    let h = (x * 127.1 + z * 311.7).sin() * 43758.5453;
    h - h.floor()
}

fn biome_value_noise_2d(x: f32, z: f32) -> f32 {
    let ix = x.floor();
    let iz = z.floor();
    let fx = x - ix;
    let fz = z - iz;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);
    let a = biome_hash21(ix, iz);
    let b = biome_hash21(ix + 1.0, iz);
    let c = biome_hash21(ix, iz + 1.0);
    let d = biome_hash21(ix + 1.0, iz + 1.0);
    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    ab + (cd - ab) * uz
}

fn biome_fbm_2d(x: f32, z: f32) -> f32 {
    let mut sum = 0.0_f32;
    let mut amp = 0.5_f32;
    let mut px = x;
    let mut pz = z;
    for _ in 0..3 {
        sum += amp * biome_value_noise_2d(px, pz);
        px = px * 2.04 + 13.7;
        pz = pz * 2.04 - 7.9;
        amp *= 0.5;
    }
    sum
}

struct SplitMix64(u64);

impl SplitMix64 {
    #[inline]
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_add(0x9E37_79B9_7F4A_7C15))
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[inline]
    fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32)
    }

    #[inline]
    fn range_f32(&mut self, min_v: f32, max_v: f32) -> f32 {
        min_v + (max_v - min_v) * self.next_f32()
    }
}

fn dist_sq_point_to_segment_2d(px: f32, pz: f32, ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let abx = bx - ax;
    let abz = bz - az;
    let len_sq = abx * abx + abz * abz;
    if len_sq <= 1e-6 {
        let dx = px - ax;
        let dz = pz - az;
        return dx * dx + dz * dz;
    }
    let t = (((px - ax) * abx + (pz - az) * abz) / len_sq).clamp(0.0, 1.0);
    let cx = ax + abx * t;
    let cz = az + abz * t;
    let dx = px - cx;
    let dz = pz - cz;
    dx * dx + dz * dz
}

fn is_point_clear_for_ground_foliage(
    config: &moba_shared::GameConfig,
    x: f32,
    z: f32,
    want_left: bool,
    min_river_edge_margin: f32,
    lane_clear_margin: f32,
    base_clear_margin: f32,
    tree_clear_radius: f32,
) -> bool {
    let canyon = &config.map.canyon;
    if moba_shared::is_left_of_river(canyon, x, z) != want_left {
        return false;
    }
    if moba_shared::canyon_edge_distance(canyon, x, z) > -min_river_edge_margin {
        return false;
    }

    // خارج از پل مرکزی دایره‌ای
    let br = &config.map.center_bridge;
    let br_s = br.model_transform.scale.0.max(br.model_transform.scale.2);
    let br_r = br.outer_radius * br_s + 1.5;
    let bdx = x - br.model_transform.location.0;
    let bdz = z - br.model_transform.location.2;
    if bdx * bdx + bdz * bdz < br_r * br_r {
        return false;
    }

    // خارج از پایگاه‌های ارتفاعی (Dawn / Dusk High Ground)
    for plat in &config.map.terrains {
        if plat.elevation_tier >= 2 {
            let loc = plat.model_transform.location;
            let hx = plat.size.0 * 0.5 + base_clear_margin;
            let hz = plat.size.2 * 0.5 + base_clear_margin;
            if (x - loc.0).abs() <= hx && (z - loc.2).abs() <= hz {
                return false;
            }
        }
    }

    // خارج از جاده لاین‌ها (Top / Mid / Bot)
    for lane in &config.map.lanes {
        let half_w = lane.width * 0.5 + lane_clear_margin;
        let half_w_sq = half_w * half_w;
        for seg in 0..lane.waypoints.len().saturating_sub(1) {
            let a = lane.waypoints[seg];
            let b = lane.waypoints[seg + 1];
            if dist_sq_point_to_segment_2d(x, z, a.0, a.2, b.0, b.2) < half_w_sq {
                return false;
            }
        }
    }

    // در صورت نیاز، فاصله از تنه درختان
    if tree_clear_radius > 0.0 {
        let tr_sq = tree_clear_radius * tree_clear_radius;
        for tree in &config.map.trees {
            let tl = tree.model_transform.location;
            let dx = x - tl.0;
            let dz = z - tl.2;
            if dx * dx + dz * dz < tr_sq {
                return false;
            }
        }
    }

    true
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct AutumnLeafUniforms {
    pub palette_0: Vec4,
    pub palette_1: Vec4,
    pub palette_2: Vec4,
    pub palette_3: Vec4,
    pub palette_4: Vec4,
    pub params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct AutumnLeafMaterial {
    #[uniform(0)]
    pub uniforms: AutumnLeafUniforms,
}

impl Material for AutumnLeafMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/autumn_leaf.wesl".into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Mask(0.5)
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BiomeWalkerSnapshot {
    pub id: u64,
    pub pos: Vec3,
}

#[derive(Resource, Default)]
pub struct BiomeWalkersState {
    pub walkers: Vec<BiomeWalkerSnapshot>,
}

struct AutumnLeafParticle {
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    ang_vel: Vec3,
    size: f32,
    rest_y: f32,
    phase: f32,
    cell: (i32, i32),
    is_active: bool,
}

#[derive(Resource)]
struct AutumnLeavesFieldState {
    mesh_handle: Handle<Mesh>,
    leaves: Vec<AutumnLeafParticle>,
    grid: HashMap<(i32, i32), Vec<usize>>,
    active_indices: Vec<usize>,
    prev_walker_pos: HashMap<u64, Vec3>,
    canyon: moba_shared::CanyonConfig,
    cfg: moba_shared::AutumnLeavesConfig,
    map_half_extents: (f32, f32),
}

#[inline]
fn leaf_grid_cell(x: f32, z: f32, cell_size: f32) -> (i32, i32) {
    ((x / cell_size).floor() as i32, (z / cell_size).floor() as i32)
}

#[inline]
fn write_leaf_quad_vertices(
    leaf: &AutumnLeafParticle,
    positions: &mut [[f32; 3]],
    normals: &mut [[f32; 3]],
    base_v: usize,
) {
    let rot = Quat::from_euler(EulerRot::YXZ, leaf.yaw, leaf.pitch, leaf.roll);
    let half = leaf.size * 0.5;
    let curl = leaf.size * 0.08;

    // کواد مربعی با انحنای ملایم برای نمایش کامل سیلوت شیدر برگ واقعی (افرا، بلوط و دندانه‌دار)
    let local = [
        Vec3::new(-half, curl, -half), // پایین-چپ (u=0, v=0)
        Vec3::new(half, curl, -half),  // پایین-راست (u=1, v=0)
        Vec3::new(half, curl, half),   // بالا-راست (u=1, v=1)
        Vec3::new(-half, curl, half),  // بالا-چپ (u=0, v=1)
    ];
    let n = (rot * Vec3::Y).normalize_or_zero();
    let n_arr = [n.x, n.y, n.z];
    for i in 0..4 {
        let wp = leaf.pos + rot * local[i];
        positions[base_v + i] = [wp.x, wp.y, wp.z];
        normals[base_v + i] = n_arr;
    }
}

fn init_river_biome_foliage_system(
    mut commands: Commands,
    config: Res<moba_shared::GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut leaf_materials: ResMut<Assets<AutumnLeafMaterial>>,
) {
    let biome = &config.map.biome;

    // برگ‌های پاییزی تعاملی سمت راست رودخانه با شیدر واقعی برگ (یک Draw Call دینامیک)
    if biome.autumn_leaves.enabled {
        use bevy::asset::RenderAssetUsages;
        use bevy::mesh::{Indices, PrimitiveTopology};

        let lcfg = &biome.autumn_leaves;
        let half_x = config.map.map_dimensions.0 * 0.5 - 6.0;
        let half_z = config.map.map_dimensions.1 * 0.5 - 6.0;
        let target_leaves = lcfg.leaf_count as usize;
        let mut rng = SplitMix64::new(lcfg.seed);

        let right_trees: Vec<(f32, f32)> = config
            .map
            .trees
            .iter()
            .filter_map(|t| {
                let loc = t.model_transform.location;
                if !moba_shared::is_left_of_river(&config.map.canyon, loc.0, loc.2) {
                    Some((loc.0, loc.2))
                } else {
                    None
                }
            })
            .collect();

        let mut leaves: Vec<AutumnLeafParticle> = Vec::with_capacity(target_leaves);
        let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let mut positions: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0]; target_leaves * 4];
        let mut normals: Vec<[f32; 3]> = vec![[0.0, 1.0, 0.0]; target_leaves * 4];
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(target_leaves * 4);
        let mut indices: Vec<u32> = Vec::with_capacity(target_leaves * 12);

        let max_attempts = target_leaves * 20;
        let mut attempts = 0usize;

        while leaves.len() < target_leaves && attempts < max_attempts {
            attempts += 1;
            // حدود ۳۵٪ برگ‌ها اطراف درختان پاییزی (tree4) و ۶۵٪ در سراسر زمین سمت راست پخش می‌شوند
            let (lx, lz) = if !right_trees.is_empty() && rng.next_f32() < 0.35 {
                let tidx = (rng.next_u64() as usize) % right_trees.len();
                let (tx, tz) = right_trees[tidx];
                let ang = rng.range_f32(0.0, std::f32::consts::TAU);
                let rad = rng.range_f32(1.8, 11.5);
                (
                    (tx + ang.cos() * rad).clamp(-half_x, half_x),
                    (tz + ang.sin() * rad).clamp(-half_z, half_z),
                )
            } else {
                (rng.range_f32(-half_x, half_x), rng.range_f32(-half_z, half_z))
            };

            if !is_point_clear_for_ground_foliage(
                &config,
                lx,
                lz,
                false,
                lcfg.min_river_edge_margin,
                lcfg.lane_clear_margin,
                lcfg.base_clear_margin,
                0.0,
            ) {
                continue;
            }

            let idx = leaves.len();
            let rest_y = lcfg.ground_y_offset + rng.range_f32(0.0, 0.035);
            let cell = leaf_grid_cell(lx, lz, lcfg.spatial_cell_size);
            let leaf = AutumnLeafParticle {
                pos: Vec3::new(lx, rest_y, lz),
                vel: Vec3::ZERO,
                yaw: rng.range_f32(0.0, std::f32::consts::TAU),
                pitch: rng.range_f32(-0.08, 0.08),
                roll: rng.range_f32(-0.08, 0.08),
                ang_vel: Vec3::ZERO,
                size: rng.range_f32(lcfg.leaf_size_min, lcfg.leaf_size_max),
                rest_y,
                phase: rng.range_f32(0.0, std::f32::consts::TAU),
                cell,
                is_active: false,
            };

            let base_v = idx * 4;
            write_leaf_quad_vertices(&leaf, &mut positions, &mut normals, base_v);

            // کدگذاری نوع برگ (0=افرا، 1=بلوط، 2=دندانه‌دار) و اندیس رنگ پالت (0..4) در بخش صحیح UV
            let species_offset = (idx % 3) as f32;
            let color_offset = (idx % lcfg.palette_rgb.len().min(5)) as f32;
            uvs.push([species_offset + 0.001, color_offset + 0.001]);
            uvs.push([species_offset + 0.999, color_offset + 0.001]);
            uvs.push([species_offset + 0.999, color_offset + 0.999]);
            uvs.push([species_offset + 0.001, color_offset + 0.999]);

            let v0 = base_v as u32;
            indices.extend_from_slice(&[
                v0, v0 + 1, v0 + 2,
                v0, v0 + 2, v0 + 3,
                v0, v0 + 2, v0 + 1,
                v0, v0 + 3, v0 + 2,
            ]);

            grid.entry(cell).or_default().push(idx);
            leaves.push(leaf);
        }

        let actual_verts = leaves.len() * 4;
        positions.truncate(actual_verts);
        normals.truncate(actual_verts);

        let mut leaf_mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        leaf_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        leaf_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        leaf_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        leaf_mesh.insert_indices(Indices::U32(indices));

        let pal_vec4 = |i: usize| -> Vec4 {
            let c = lcfg.palette_rgb[i % lcfg.palette_rgb.len()];
            let lin = Color::srgb(c.0, c.1, c.2).to_linear();
            Vec4::new(lin.red, lin.green, lin.blue, 1.0)
        };

        let mesh_handle = meshes.add(leaf_mesh);
        let leaf_mat = leaf_materials.add(AutumnLeafMaterial {
            uniforms: AutumnLeafUniforms {
                palette_0: pal_vec4(0),
                palette_1: pal_vec4(1),
                palette_2: pal_vec4(2),
                palette_3: pal_vec4(3),
                palette_4: pal_vec4(4),
                params: Vec4::new(lcfg.roughness, lcfg.vein_contrast, lcfg.edge_darkening, 0.0),
            },
        });

        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            MeshMaterial3d(leaf_mat),
            Transform::IDENTITY,
            Visibility::Visible,
            bevy::light::NotShadowCaster,
        ));

        commands.insert_resource(AutumnLeavesFieldState {
            mesh_handle,
            leaves,
            grid,
            active_indices: Vec::new(),
            prev_walker_pos: HashMap::new(),
            canyon: config.map.canyon.clone(),
            cfg: lcfg.clone(),
            map_half_extents: (half_x, half_z),
        });
    }
}

fn update_autumn_leaves_scatter_system(
    time: Res<Time>,
    world_opt: Option<Res<moba_shared::AuthoritativeWorldState>>,
    walkers: Res<BiomeWalkersState>,
    field_opt: Option<ResMut<AutumnLeavesFieldState>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    if world_opt
        .as_ref()
        .is_some_and(|w| w.pause_phase != moba_shared::PausePhase::Running)
    {
        return;
    }
    let Some(mut field) = field_opt else {
        return;
    };
    let dt = time.delta_secs().clamp(0.0001, 0.05);
    let elapsed = time.elapsed_secs();

    let cell_size = field.cfg.spatial_cell_size;
    let scatter_r = field.cfg.scatter_radius;
    let scatter_r_sq = scatter_r * scatter_r;
    let min_speed = field.cfg.min_walker_speed;
    let cell_span = (scatter_r / cell_size).ceil() as i32;

    // ۱. تشخیص حرکت قهرمان‌ها و کریپ‌ها و اعمال ضربه پخش‌کننده به برگ‌های اطراف
    for w in &walkers.walkers {
        let prev = field.prev_walker_pos.insert(w.id, w.pos);
        let Some(prev_pos) = prev else {
            continue;
        };
        let delta = Vec2::new(w.pos.x - prev_pos.x, w.pos.z - prev_pos.z);
        let dist_moved = delta.length();
        // جلوگیری از انفجار برگ هنگام تلپورت
        if dist_moved > 12.0 {
            continue;
        }
        let speed = dist_moved / dt;
        if speed < min_speed {
            continue;
        }
        // فقط اگر در نیمه راست (پاییزی) یا نزدیک ساحل راست است بررسی کن
        if moba_shared::canyon_river_side_signed(&field.canyon, w.pos.x, w.pos.z) > scatter_r + 2.0 {
            continue;
        }

        let move_dir = delta / dist_moved.max(1e-4);
        let speed_factor = (speed / 8.0).clamp(0.40, 1.85);
        let (cx, cz) = leaf_grid_cell(w.pos.x, w.pos.z, cell_size);

        let mut candidate_leaves: Vec<usize> = Vec::new();
        for gx in (cx - cell_span)..=(cx + cell_span) {
            for gz in (cz - cell_span)..=(cz + cell_span) {
                if let Some(bucket) = field.grid.get(&(gx, gz)) {
                    candidate_leaves.extend_from_slice(bucket);
                }
            }
        }

        let out_imp = field.cfg.scatter_outward_impulse;
        let up_imp = field.cfg.scatter_upward_impulse;
        let swirl_imp = field.cfg.scatter_swirl_impulse;
        let spin_spd = field.cfg.scatter_spin_speed;

        for leaf_idx in candidate_leaves {
            let leaf = &mut field.leaves[leaf_idx];
            let dx = leaf.pos.x - w.pos.x;
            let dz = leaf.pos.z - w.pos.z;
            let d_sq = dx * dx + dz * dz;
            if d_sq > scatter_r_sq {
                continue;
            }
            let d = d_sq.sqrt();
            let falloff = (1.0 - d / scatter_r).clamp(0.0, 1.0);
            let strength = falloff * speed_factor;

            let away = if d > 0.08 {
                Vec2::new(dx / d, dz / d)
            } else {
                Vec2::new(leaf.phase.cos(), leaf.phase.sin())
            };
            let swirl_sign = if leaf_idx % 2 == 0 { 1.0 } else { -1.0 };
            let tangent = Vec2::new(-away.y * swirl_sign, away.x * swirl_sign);

            let jitter = 0.70 + 0.60 * ((leaf.phase * 3.17).sin() * 0.5 + 0.5);
            leaf.vel.x += (away.x * out_imp + tangent.x * swirl_imp + move_dir.x * 1.15) * strength * dt * 14.0;
            leaf.vel.z += (away.y * out_imp + tangent.y * swirl_imp + move_dir.y * 1.15) * strength * dt * 14.0;
            leaf.vel.y = leaf.vel.y.max(up_imp * strength * jitter);

            leaf.ang_vel.x = (leaf.phase.sin() * spin_spd) * strength;
            leaf.ang_vel.y = (swirl_sign * spin_spd * 0.85) * strength;
            leaf.ang_vel.z = (leaf.phase.cos() * spin_spd) * strength;

            if !leaf.is_active {
                leaf.is_active = true;
                field.active_indices.push(leaf_idx);
            }
        }
    }

    if field.active_indices.is_empty() {
        return;
    }

    // ۲. شبیه‌سازی فیزیک سبک فقط برای برگ‌های فعال و به‌روزرسانی مستقیم ورتکس‌های مش
    let gravity = field.cfg.gravity;
    let air_drag_factor = (-field.cfg.air_drag * dt).exp();
    let ground_fric_factor = (-field.cfg.ground_friction * dt).exp();
    let flutter_amp = field.cfg.flutter_amplitude;
    let flutter_freq = field.cfg.flutter_frequency;
    let min_river_margin = field.cfg.min_river_edge_margin;
    let (half_x, half_z) = field.map_half_extents;
    let canyon = field.canyon.clone();

    let mut moved_cells: Vec<(usize, (i32, i32), (i32, i32))> = Vec::new();
    let mut updated_leaves: Vec<usize> = Vec::with_capacity(field.active_indices.len());
    let mut still_active: Vec<usize> = Vec::with_capacity(field.active_indices.len());

    let active_snapshot = std::mem::take(&mut field.active_indices);
    for leaf_idx in active_snapshot {
        let leaf = &mut field.leaves[leaf_idx];
        let airborne = leaf.pos.y > leaf.rest_y + 0.015 || leaf.vel.y > 0.01;

        if airborne {
            leaf.vel.y -= gravity * dt;
            leaf.vel.x *= air_drag_factor;
            leaf.vel.z *= air_drag_factor;
            let flutter = (elapsed * flutter_freq + leaf.phase).sin() * flutter_amp;
            leaf.pos.x += (leaf.vel.x + flutter * 0.35) * dt;
            leaf.pos.z += (leaf.vel.z + (elapsed * flutter_freq * 0.8 + leaf.phase).cos() * flutter_amp * 0.35) * dt;
            leaf.pos.y += leaf.vel.y * dt;

            leaf.yaw += leaf.ang_vel.y * dt;
            leaf.pitch += leaf.ang_vel.x * dt;
            leaf.roll += leaf.ang_vel.z * dt;
            leaf.ang_vel *= air_drag_factor;
        } else {
            leaf.vel.x *= ground_fric_factor;
            leaf.vel.z *= ground_fric_factor;
            leaf.pos.x += leaf.vel.x * dt;
            leaf.pos.z += leaf.vel.z * dt;
            leaf.pitch *= ground_fric_factor;
            leaf.roll *= ground_fric_factor;
        }

        // نگه داشتن برگ در محدوده زمین سمت راست (جلوگیری از سقوط داخل دره)
        leaf.pos.x = leaf.pos.x.clamp(-half_x, half_x);
        leaf.pos.z = leaf.pos.z.clamp(-half_z, half_z);
        if moba_shared::canyon_edge_distance(&canyon, leaf.pos.x, leaf.pos.z) > -min_river_margin {
            leaf.vel.x = -leaf.vel.x * 0.35;
            leaf.vel.z = -leaf.vel.z * 0.35;
        }

        if leaf.pos.y <= leaf.rest_y {
            leaf.pos.y = leaf.rest_y;
            if leaf.vel.y < 0.0 {
                leaf.vel.y = 0.0;
            }
        }

        let new_cell = leaf_grid_cell(leaf.pos.x, leaf.pos.z, cell_size);
        if new_cell != leaf.cell {
            moved_cells.push((leaf_idx, leaf.cell, new_cell));
            leaf.cell = new_cell;
        }

        updated_leaves.push(leaf_idx);

        let horiz_speed_sq = leaf.vel.x * leaf.vel.x + leaf.vel.z * leaf.vel.z;
        let settled = leaf.pos.y <= leaf.rest_y + 0.005
            && leaf.vel.y <= 0.01
            && horiz_speed_sq < 0.008
            && leaf.pitch.abs() < 0.09
            && leaf.roll.abs() < 0.09;

        if settled {
            leaf.pos.y = leaf.rest_y;
            leaf.vel = Vec3::ZERO;
            leaf.ang_vel = Vec3::ZERO;
            leaf.is_active = false;
        } else {
            still_active.push(leaf_idx);
        }
    }
    field.active_indices = still_active;

    for (leaf_idx, old_cell, new_cell) in moved_cells {
        if let Some(bucket) = field.grid.get_mut(&old_cell) {
            if let Some(pos) = bucket.iter().position(|&i| i == leaf_idx) {
                bucket.swap_remove(pos);
            }
        }
        field.grid.entry(new_cell).or_default().push(leaf_idx);
    }

    if let Some(mut mesh) = meshes.get_mut(&field.mesh_handle) {
        let mut pos_copy = if let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        {
            p.clone()
        } else {
            return;
        };
        let mut norm_copy = if let Some(bevy::mesh::VertexAttributeValues::Float32x3(n)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        {
            n.clone()
        } else {
            return;
        };

        for leaf_idx in updated_leaves {
            write_leaf_quad_vertices(&field.leaves[leaf_idx], &mut pos_copy, &mut norm_copy, leaf_idx * 4);
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos_copy);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, norm_copy);
    }
}


// =============================================================================
// سیستم پویا نشستن برف روی زمین، درختان، کوه‌ها و ساختمان‌ها (ExtendedMaterial)
// =============================================================================

#[derive(Component)]
pub struct SnowSurfaceRoot;

#[derive(Component)]
pub struct SnowConvertedMaterial {
    pub original_handle: Handle<StandardMaterial>,
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct SnowAccumulationUniforms {
    pub snow_color: Vec4,
    pub snow_shadow_tint: Vec4,
    pub night_glow_color: Vec4,
    pub snow_params: Vec4,
    pub detail_params: Vec4,
    pub fx_params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct SnowAccumulationExtension {
    #[uniform(100)]
    pub uniforms: SnowAccumulationUniforms,
}

impl MaterialExtension for SnowAccumulationExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/snow_accumulation.wesl".into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/snow_accumulation.wesl".into()
    }
}

pub type SnowExtendedMaterial = ExtendedMaterial<StandardMaterial, SnowAccumulationExtension>;

#[derive(Resource, Default)]
pub struct SnowAccumulationState {
    pub snow_amount: f32,
    pub night_blend: f32,
    last_pushed_snow: f32,
    last_pushed_night: f32,
    converted_map: HashMap<AssetId<StandardMaterial>, Handle<SnowExtendedMaterial>>,
}

fn build_snow_uniforms(config: &moba_shared::GameConfig, snow_amount: f32, night_blend: f32) -> SnowAccumulationUniforms {
    let s = &config.weather.snow_accumulation;
    let sc = Color::srgb(s.snow_color_rgb.0, s.snow_color_rgb.1, s.snow_color_rgb.2).to_linear();
    let st = Color::srgb(s.snow_shadow_tint_rgb.0, s.snow_shadow_tint_rgb.1, s.snow_shadow_tint_rgb.2).to_linear();
    let ng = Color::srgb(s.night_snow_glow_rgb.0, s.night_snow_glow_rgb.1, s.night_snow_glow_rgb.2).to_linear();
    let water_excl_y = config.map.canyon.water_y + s.water_exclusion_margin;

    SnowAccumulationUniforms {
        snow_color: Vec4::new(sc.red, sc.green, sc.blue, 1.0),
        snow_shadow_tint: Vec4::new(st.red, st.green, st.blue, 1.0),
        night_glow_color: Vec4::new(ng.red, ng.green, ng.blue, 1.0),
        snow_params: Vec4::new(snow_amount, s.slope_threshold, s.slope_softness, s.ground_coverage_boost),
        detail_params: Vec4::new(s.noise_scale, s.noise_strength, s.snow_roughness, night_blend),
        fx_params: Vec4::new(s.sparkle_strength, s.sparkle_scale, water_excl_y, s.night_snow_glow_strength),
    }
}

#[allow(clippy::too_many_arguments)]
fn update_snow_accumulation_system(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<moba_shared::GameConfig>,
    world_opt: Option<Res<moba_shared::AuthoritativeWorldState>>,
    weather_state: Res<WeatherVfxState>,
    mut snow_state: ResMut<SnowAccumulationState>,
    std_materials: Res<Assets<StandardMaterial>>,
    mut snow_materials: ResMut<Assets<SnowExtendedMaterial>>,
    mut leaf_materials: ResMut<Assets<AutumnLeafMaterial>>,
    roots_q: Query<Entity, With<SnowSurfaceRoot>>,
    children_q: Query<&Children>,
    std_mesh_q: Query<&MeshMaterial3d<StandardMaterial>, Without<SnowConvertedMaterial>>,
    converted_q: Query<(Entity, &SnowConvertedMaterial)>,
) {
    if world_opt
        .as_ref()
        .is_some_and(|w| w.pause_phase != moba_shared::PausePhase::Running)
    {
        return;
    }

    let s_cfg = &config.weather.snow_accumulation;
    if !s_cfg.enabled {
        return;
    }

    let dt = time.delta_secs().clamp(0.0, 0.1);
    let want_snow = weather_state.active_weather == Some(WeatherKind::Snowy);
    if want_snow {
        let rate = dt / s_cfg.accumulate_duration_secs.max(0.1);
        snow_state.snow_amount = (snow_state.snow_amount + rate).min(1.0);
    } else {
        let rate = dt / s_cfg.melt_duration_secs.max(0.1);
        snow_state.snow_amount = (snow_state.snow_amount - rate).max(0.0);
    }

    let snow_amt = snow_state.snow_amount;
    let night_b = snow_state.night_blend;

    // به‌روزرسانی میزان نشستن برف روی برگ‌های پاییزی سمت راست
    if (snow_amt - snow_state.last_pushed_snow).abs() > 0.0005 {
        for (_, mat) in leaf_materials.iter_mut() {
            mat.uniforms.params.w = snow_amt;
        }
    }

    // وقتی برف کاملاً آب شد، متریال اصلی StandardMaterial را برمی‌گردانیم
    if snow_amt <= 0.001 {
        if !converted_q.is_empty() {
            for (ent, conv) in &converted_q {
                commands
                    .entity(ent)
                    .remove::<MeshMaterial3d<SnowExtendedMaterial>>()
                    .remove::<SnowConvertedMaterial>()
                    .insert(MeshMaterial3d(conv.original_handle.clone()));
            }
        }
        snow_state.last_pushed_snow = 0.0;
        return;
    }

    let uniforms = build_snow_uniforms(&config, snow_amt, night_b);

    // تبدیل متریال آبجکت‌های محیطی (زمین، درختان، کوه‌ها، برج‌ها و ساختمان‌ها) با حفظ کامل GPU Instancing
    let mut to_convert: Vec<(Entity, Handle<StandardMaterial>)> = Vec::new();
    for root_ent in &roots_q {
        if let Ok(mat_comp) = std_mesh_q.get(root_ent) {
            to_convert.push((root_ent, mat_comp.0.clone()));
        }
        for desc in children_q.iter_descendants(root_ent) {
            if let Ok(mat_comp) = std_mesh_q.get(desc) {
                to_convert.push((desc, mat_comp.0.clone()));
            }
        }
    }

    for (ent, orig_handle) in to_convert {
        let asset_id = orig_handle.id();
        let ext_handle = if let Some(cached) = snow_state.converted_map.get(&asset_id) {
            cached.clone()
        } else if let Some(base_mat) = std_materials.get(&orig_handle) {
            let new_ext = snow_materials.add(ExtendedMaterial {
                base: base_mat.clone(),
                extension: SnowAccumulationExtension { uniforms },
            });
            snow_state.converted_map.insert(asset_id, new_ext.clone());
            new_ext
        } else {
            continue;
        };

        commands
            .entity(ent)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((
                MeshMaterial3d(ext_handle),
                SnowConvertedMaterial {
                    original_handle: orig_handle,
                },
            ));
    }

    // به‌روزرسانی Uniform برف و روشنایی شب روی تمام متریال‌های کش‌شده (بسیار سبک؛ حدود ۲۰ متریال)
    if (snow_amt - snow_state.last_pushed_snow).abs() > 0.0005
        || (night_b - snow_state.last_pushed_night).abs() > 0.0005
    {
        for (_, mat) in snow_materials.iter_mut() {
            mat.extension.uniforms = uniforms;
        }
        snow_state.last_pushed_snow = snow_amt;
        snow_state.last_pushed_night = night_b;
    }
}