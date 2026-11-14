use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy_hanabi::prelude::{
    AccelModifier, Attribute, ColorOverLifetimeModifier, EffectAsset, ExprWriter, HanabiPlugin,
    LinearDragModifier, OrientMode, OrientModifier, ParticleEffect, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SizeOverLifetimeModifier,
    SpawnerSettings,
};
use moba_shared::ImpactSurface;

// -----------------------------------------------------------------------------
// شیدرهای اختصاصی WESL: آب بهینه و خط تیر موج مغناطیسی
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
struct MagneticTrailUniforms {
    core_color: Vec4,
    wave_color: Vec4,
    params: Vec4,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
struct MagneticTrailMaterial {
    #[uniform(0)]
    uniforms: MagneticTrailUniforms,
}

impl Material for MagneticTrailMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/magnetic_trail.wesl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

// -----------------------------------------------------------------------------
// رویدادها و منابع GPU VFX (Hanabi)
// -----------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum ClientVfxEvent {
    MuzzleSmoke { pos: Vec3, dir: Vec3 },
    MagneticTrail { start: Vec3, end: Vec3 },
    BulletImpact { pos: Vec3, surface: ImpactSurface },
    SpawnPondAmbient { pos: Vec3 },
}

#[derive(Resource, Default)]
pub struct VfxEventQueue(pub Vec<ClientVfxEvent>);

#[derive(Resource)]
struct HanabiEffectHandles {
    muzzle_smoke: Handle<EffectAsset>,
    blood_burst: Handle<EffectAsset>,
    stone_dust: Handle<EffectAsset>,
    metal_sparks: Handle<EffectAsset>,
    water_splash: Handle<EffectAsset>,
    pond_ambient_bubbles: Handle<EffectAsset>,
    trail_mesh: Handle<Mesh>,
    blood_decal_mesh: Handle<Mesh>,
    blood_decal_mat: Handle<StandardMaterial>,
}

#[derive(Component)]
struct OneShotVfxDespawn {
    timer: Timer,
}

#[derive(Component)]
struct MagneticTrailInstance {
    timer: Timer,
    mat_handle: Handle<MagneticTrailMaterial>,
}

#[derive(Component)]
struct BloodGroundDecal {
    timer: Timer,
}

pub struct MobaVfxPlugin;

impl Plugin for MobaVfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_plugins(MaterialPlugin::<WaterSurfaceMaterial>::default())
            .add_plugins(MaterialPlugin::<MagneticTrailMaterial>::default())
            .init_resource::<VfxEventQueue>()
            .add_systems(Startup, init_hanabi_vfx_assets_system)
            .add_systems(
                Update,
                (
                    process_vfx_events_system,
                    update_vfx_lifetimes_system,
                )
                    .chain(),
            );
    }
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
        10.0,
        0.04,
        1.4,
        3.2,
        0.22,
        0.48,
        Vec3::new(0.0, 1.4, 0.0),
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
        22.0,
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
        16.0,
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
        24.0,
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
        20.0,
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

    let trail_mesh = meshes.add(Plane3d::default().mesh().size(1.0, 1.0));
    let blood_decal_mesh = meshes.add(Circle::new(0.42));
    let blood_decal_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.48, 0.02, 0.02, 0.72),
        perceptual_roughness: 0.35,
        alpha_mode: AlphaMode::Blend,
        ..default()
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

    commands.insert_resource(HanabiEffectHandles {
        muzzle_smoke,
        blood_burst,
        stone_dust,
        metal_sparks,
        water_splash,
        pond_ambient_bubbles,
        trail_mesh,
        blood_decal_mesh,
        blood_decal_mat,
    });
}

fn process_vfx_events_system(
    mut commands: Commands,
    time: Res<Time>,
    handles: Option<Res<HanabiEffectHandles>>,
    mut queue: ResMut<VfxEventQueue>,
    mut trail_materials: ResMut<Assets<MagneticTrailMaterial>>,
) {
    if queue.0.is_empty() {
        return;
    }
    let Some(handles) = handles else {
        queue.0.clear();
        return;
    };
    let t = time.elapsed_secs();

    for ev in queue.0.drain(..) {
        match ev {
            ClientVfxEvent::MuzzleSmoke { pos, dir } => {
                let forward = dir.normalize_or_zero();
                let spawn_pos = pos + forward * 0.18;
                commands.spawn((
                    ParticleEffect::new(handles.muzzle_smoke.clone()),
                    Transform::from_translation(spawn_pos),
                    Visibility::default(),
                    OneShotVfxDespawn {
                        timer: Timer::from_seconds(0.65, TimerMode::Once),
                    },
                ));
            }
            ClientVfxEvent::MagneticTrail { start, end } => {
                let delta = end - start;
                let len = delta.length();
                if len <= 0.08 {
                    continue;
                }
                let dir = delta / len;
                let mid = (start + end) * 0.5;
                // جهت‌دهی صفحه خط تیر به سمت بالا و راستای تیر
                let right = dir.cross(Vec3::Y).normalize_or_zero();
                let rot = if right.length_squared() > 0.001 {
                    let up = right.cross(dir).normalize_or_zero();
                    Quat::from_mat3(&Mat3::from_cols(dir, up, right))
                } else {
                    Quat::IDENTITY
                };

                let mat_handle = trail_materials.add(MagneticTrailMaterial {
                    uniforms: MagneticTrailUniforms {
                        core_color: Vec4::new(1.0, 0.92, 0.52, 1.0),
                        wave_color: Vec4::new(0.22, 0.86, 1.0, 1.0),
                        params: Vec4::new(0.0, t, 18.0, 5.2),
                    },
                });

                commands.spawn((
                    Mesh3d(handles.trail_mesh.clone()),
                    MeshMaterial3d(mat_handle.clone()),
                    Transform {
                        translation: mid,
                        rotation: rot,
                        scale: Vec3::new(len, 1.0, 0.24),
                    },
                    NotShadowCaster,
                    MagneticTrailInstance {
                        timer: Timer::from_seconds(0.18, TimerMode::Once),
                        mat_handle,
                    },
                ));
            }
            ClientVfxEvent::BulletImpact { pos, surface } => {
                let (effect_h, lifetime) = match surface {
                    ImpactSurface::Character => (handles.blood_burst.clone(), 0.70),
                    ImpactSurface::StoneWall | ImpactSurface::Building => (handles.stone_dust.clone(), 0.85),
                    ImpactSurface::Turret => (handles.metal_sparks.clone(), 0.55),
                    ImpactSurface::Water => (handles.water_splash.clone(), 0.65),
                };

                commands.spawn((
                    ParticleEffect::new(effect_h),
                    Transform::from_translation(pos),
                    Visibility::default(),
                    OneShotVfxDespawn {
                        timer: Timer::from_seconds(lifetime, TimerMode::Once),
                    },
                ));

                // لکه خون روی زمین زیر محل برخورد به دشمن
                if surface == ImpactSurface::Character {
                    let ground_pos = Vec3::new(pos.x, (pos.y - 0.85).max(0.03), pos.z);
                    commands.spawn((
                        Mesh3d(handles.blood_decal_mesh.clone()),
                        MeshMaterial3d(handles.blood_decal_mat.clone()),
                        Transform::from_translation(ground_pos)
                            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
                        NotShadowCaster,
                        BloodGroundDecal {
                            timer: Timer::from_seconds(3.2, TimerMode::Once),
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
        }
    }
}

fn update_vfx_lifetimes_system(
    mut commands: Commands,
    time: Res<Time>,
    mut trail_materials: ResMut<Assets<MagneticTrailMaterial>>,
    mut one_shot_q: Query<(Entity, &mut OneShotVfxDespawn)>,
    mut trail_q: Query<(Entity, &mut MagneticTrailInstance)>,
    mut decal_q: Query<(Entity, &mut Transform, &mut BloodGroundDecal)>,
) {
    let dt = time.delta();
    let t = time.elapsed_secs();

    for (ent, mut os) in &mut one_shot_q {
        os.timer.tick(dt);
        if os.timer.is_finished() {
            commands.entity(ent).despawn();
        }
    }

    for (ent, mut tr) in &mut trail_q {
        tr.timer.tick(dt);
        let frac = tr.timer.fraction();
        if let Some(mut mat) = trail_materials.get_mut(&tr.mat_handle) {
            mat.uniforms.params.x = frac;
            mat.uniforms.params.y = t;
        }
        if tr.timer.is_finished() {
            commands.entity(ent).despawn();
        }
    }

    for (ent, mut tf, mut dec) in &mut decal_q {
        dec.timer.tick(dt);
        let frac = dec.timer.fraction();
        if frac > 0.7 {
            let s = ((1.0 - frac) / 0.3).max(0.05);
            tf.scale = Vec3::splat(s);
        }
        if dec.timer.is_finished() {
            commands.entity(ent).despawn();
        }
    }
}