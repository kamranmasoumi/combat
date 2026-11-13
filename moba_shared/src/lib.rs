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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatConfig {
    pub max_message_len: usize,
    pub max_visible_messages: usize,
    pub message_fade_secs: f32,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct GameConfig {
    pub pause: PauseConfig,
    pub ping: PingConfig,
    pub chat: ChatConfig,
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
        let cfg = ron::from_str::<GameConfig>(&content).unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی: ساختار فایل کانفیگ '{}' نامعتبر است: {}",
                path, e
            )
        });
        cfg.validate_unique_map_ids();
        cfg.validate_fountains();
        cfg.validate_barracks();
        cfg.validate_ancients();
        cfg.validate_chat();
        cfg.validate_ping();
        cfg.validate_pause();
        cfg.validate_camera();
        cfg.validate_heroes();
        cfg.validate_creeps();
        cfg.validate_neutral_camps();
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
            assert!(hero.max_stamina > 0.0, "max_stamina هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.stamina_regen_per_sec > 0.0, "stamina_regen_per_sec هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.slide_stamina_cost > 0.0, "slide_stamina_cost هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.jump_over_stamina_cost > 0.0, "jump_over_stamina_cost هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.level_growth.stamina_regen_per_level >= 0.0, "stamina_regen_per_level هیرو '{}' نمی‌تواند منفی باشد", hero.id);
            assert!(hero.hero_scale > 0.0, "hero_scale هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.xp_gain_radius > 0.0, "xp_gain_radius هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.magazine_size > 0, "magazine_size اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.fire_rate_secs > 0.0, "fire_rate_secs اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.reload_duration_secs > 0.0, "reload_duration_secs اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.projectile_speed > 0.0, "projectile_speed اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.max_range > 0.0, "max_range اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
            assert!(hero.weapon.pellets_per_shot > 0, "pellets_per_shot اسلحه هیرو '{}' باید بزرگ‌تر از صفر باشد", hero.id);
        }
    }

    pub fn validate_neutral_camps(&self) {
        let nc = &self.neutral_camps;
        assert!(nc.respawn_interval_secs > 0.0, "respawn_interval_secs کمپ‌ها باید بزرگ‌تر از صفر باشد");
        assert!(nc.aggro_leash_radius > 0.0, "aggro_leash_radius کمپ‌ها باید بزرگ‌تر از صفر باشد");
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
        assert!(!c.model_path.trim().is_empty(), "model_path سربازان لاین نمی‌تواند خالی باشد");
        assert!(c.scale > 0.0, "scale سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.collider_radius > 0.0, "collider_radius سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.hitbox_height > 0.0, "hitbox_height سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.wave_interval_secs > 0.0, "wave_interval_secs باید بزرگ‌تر از صفر باشد");
        assert!(c.creeps_per_wave > 0, "creeps_per_wave باید حداقل ۱ باشد");
        assert!(c.max_hp > 0.0, "max_hp سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.fire_rate_secs > 0.0, "fire_rate_secs سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.projectile_speed > 0.0, "projectile_speed سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(c.bullet_max_range > 0.0, "bullet_max_range سربازان لاین باید بزرگ‌تر از صفر باشد");
        assert!(
            (0.0..=1.0).contains(&c.deny_hp_threshold_ratio),
            "deny_hp_threshold_ratio باید بین 0 و 1 باشد"
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
    }

    pub fn validate_pause(&self) {
        assert!(self.pause.max_pauses_per_team > 0, "pause.max_pauses_per_team باید بزرگ‌تر از صفر باشد");
        assert!(self.pause.team_time_bank_secs > 0.0, "pause.team_time_bank_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.pause.player_cooldown_secs >= 0.0, "pause.player_cooldown_secs نمی‌تواند منفی باشد");
        assert!(self.pause.manual_unpause_lock_secs >= 0.0, "pause.manual_unpause_lock_secs نمی‌تواند منفی باشد");
        assert!(self.pause.disconnect_unpause_lock_secs >= 0.0, "pause.disconnect_unpause_lock_secs نمی‌تواند منفی باشد");
        assert!(self.pause.unpause_countdown_secs > 0.0, "pause.unpause_countdown_secs باید بزرگ‌تر از صفر باشد");
    }

    pub fn validate_ping(&self) {
        assert!(self.ping.duration_secs > 0.0, "ping.duration_secs باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.max_active_pings > 0, "ping.max_active_pings باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.world_ring_radius > 0.0, "ping.world_ring_radius باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.beam_height > 0.0, "ping.beam_height باید بزرگ‌تر از صفر باشد");
        assert!(self.ping.minimap_pulse_speed > 0.0, "ping.minimap_pulse_speed باید بزرگ‌تر از صفر باشد");
    }

    pub fn validate_chat(&self) {
        assert!(self.chat.max_message_len > 0, "chat.max_message_len باید بزرگ‌تر از صفر باشد");
        assert!(self.chat.max_visible_messages > 0, "chat.max_visible_messages باید بزرگ‌تر از صفر باشد");
        assert!(self.chat.message_fade_secs > 0.0, "chat.message_fade_secs باید بزرگ‌تر از صفر باشد");
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

/// خواندن اجباری آرگومان‌های خط فرمان سرور: `<port> <required_players>` (بدون مقدار پیش‌فرض)
pub fn parse_server_cli_args() -> (u16, usize) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        panic!(
            "خطای بحرانی: پورت سرور و تعداد نفرات مجاز باید به کامند ارسال شوند! مثال:\n  cargo run -p moba_server -- 5000 2"
        );
    }
    let port: u16 = args[1].parse().unwrap_or_else(|e| {
        panic!("خطای بحرانی: پورت '{}' معتبر نیست: {}", args[1], e)
    });
    assert!(port > 0, "خطای بحرانی: شماره پورت باید بزرگ‌تر از صفر باشد");
    let required_players: usize = args[2].parse().unwrap_or_else(|e| {
        panic!("خطای بحرانی: تعداد نفرات '{}' معتبر نیست: {}", args[2], e)
    });
    assert!(
        required_players >= 1,
        "خطای بحرانی: تعداد نفرات مجاز باید حداقل ۱ نفر باشد"
    );
    (port, required_players)
}

/// خواندن اختیاری پورت و client_id برای کلاینت جهت اتصال چند بازیکن هم‌زمان: `<port> <client_id>`
pub fn apply_client_cli_overrides(net: &mut NetworkConfig) {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 {
        let port: u16 = args[1].parse().unwrap_or_else(|e| {
            panic!("خطای بحرانی کلاینت: پورت '{}' معتبر نیست: {}", args[1], e)
        });
        let ip = net
            .server_bind_addr
            .split(':')
            .next()
            .expect("آدرس سرور نامعتبر است");
        net.server_bind_addr = format!("{}:{}", ip, port);
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
    pub cursor_lead_max_distance_x: f32,
    pub cursor_lead_max_distance_z: f32,
    pub cursor_lead_deadzone_ratio: f32,
    pub cursor_lead_lerp_speed: f32,
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

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WeaponKind {
    AutomaticRifle,
    Sniper,
    Shotgun,
    Throwable,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
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
    pub armor: f32,
    pub hero_scale: f32,
    pub move_speed_multiplier: f32,
    pub shallow_water_speed_multiplier: f32,
    pub deep_water_speed_multiplier: f32,
    pub vision_range: f32,
    pub high_ground_vision_bonus: f32,
    pub weapon: HeroWeaponDef,
    pub level_growth: HeroLevelGrowth,
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
    pub jump_over_speed: f32,
    pub jump_over_clearance_height: f32,
    pub space_hold_threshold_secs: f32,
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
    pub model_path: String,
    pub scale: f32,
    pub collider_radius: f32,
    pub hitbox_height: f32,
    pub aim_chest_height: f32,
    pub muzzle_offset_y: f32,
    pub muzzle_forward: f32,
    pub first_wave_delay_secs: f32,
    pub wave_interval_secs: f32,
    pub creeps_per_wave: usize,
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
    pub hovered_tint_emissive_rgb: (f32, f32, f32),
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
    pub creep_types: HashMap<String, NeutralCreepTypeDef>,
    pub camps: Vec<NeutralCampDef>,
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
pub struct MapConfig {
    pub map_dimensions: (f32, f32),
    pub show_object_labels: bool,
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
    pub max_hp: f32,
    pub armor: f32,
    pub backdoor_protection_requires_creeps: bool,
    pub color_rgb: (f32, f32, f32),
    pub core_emissive_rgb: (f32, f32, f32),
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MountainDef {
    pub id: String,
    pub model_path: String,
    pub model_transform: ModelTransformConfig,
    pub size: (f32, f32, f32),
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
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
    pub model_transform: ModelTransformConfig,
    pub kind: CoverKind,
    pub size: (f32, f32, f32),
    pub can_vault: bool,
    pub color_rgb: (f32, f32, f32),
    pub roughness: f32,
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
        self.space_held = newer.space_held;
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
    HeroLeveledUp {
        pos: Vec3,
        new_level: u32,
    },
    LastHitGold {
        pos: Vec3,
        gold: u32,
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
    SelectHero { hero_id: String },
    ReportMapLoaded,
    SendChat { all_chat: bool, text: String },
    SendPing { world_pos: Vec3 },
    TogglePause,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerMatchMessage {
    pub your_slot: usize,
    pub state: MatchStateSnapshot,
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
        if self.slot_for_client(client_id).is_some() {
            return false;
        }
        // اول بررسی کن آیا این بازیکن قبلاً در یکی از اسلات‌ها بوده و دیسکانکت شده است (Reconnect)
        let reconnect_idx = self
            .players
            .iter()
            .position(|p| !p.connected && p.client_id == Some(client_id));
        let target_idx = reconnect_idx.or_else(|| {
            self.players
                .iter()
                .position(|p| !p.connected && p.client_id.is_none())
        }).or_else(|| {
            self.players.iter().position(|p| !p.connected)
        });
        let Some(idx) = target_idx else {
            return false;
        };
        let slot = &mut self.players[idx];
        slot.connected = true;
        slot.client_id = Some(client_id);
        if self.phase == MatchPhase::InGame {
            slot.map_loaded = true;
        }
        self.dirty = true;

        if self.phase == MatchPhase::WaitingForPlayers
            && self.connected_count() == self.required_players
        {
            self.start_hero_select(config);
        }
        true
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
                if all_loaded && self.connected_count() > 0 && self.battle_loading_timer >= 2.5 {
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
    pub weapon_kind: WeaponKind,
    pub ammo_in_mag: u32,
    pub magazine_size: u32,
    pub fire_rate_secs: f32,
    pub reload_duration_secs: f32,
    pub weapon_damage: f32,
    pub projectile_speed: f32,
    pub weapon_max_range: f32,
    pub last_hits: u32,
    pub denies: u32,
    pub assists: u32,
    pub kills: u32,
    pub deaths: u32,
    pub respawn_remaining: f32,
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
    pub ammo_in_mag: u32,
    pub magazine_size: u32,
    pub is_visible: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerWorldSnapshot {
    pub tick: u64,
    pub match_elapsed_secs: f32,
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
    pub is_dead: bool,
    pub death_anim_name: String,
    pub in_water: bool,
    pub hero: HeroSnapshot,
    pub other_players: Vec<RemotePlayerSnapshot>,
    pub top_bar_heroes: Vec<TopBarHeroInfo>,
    pub fog_visible_mask: Vec<u8>,
    pub bullets: Vec<BulletSnapshot>,
    pub grenades: Vec<GrenadeSnapshot>,
    pub targets: Vec<TargetSnapshot>,
    pub creeps: Vec<CreepSnapshot>,
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

/// بررسی قرار گرفتن یک نقطه ۳بعدی درون برکه‌ها و رودخانه‌های نقشه و برگرداندن ارتفاع سطح آب و بردار جریان
pub fn sample_water_surface_at(config: &GameConfig, world_pos: Vec3) -> Option<(f32, Vec2)> {
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
                        let horiz_normal = Vec3::new(wall_hit.normal.x, 0.0, wall_hit.normal.z)
                            .normalize_or_zero();
                        let mut stepped_up = false;

                        // اگر برخورد در پایین پا و زیر سقف Autostep باشد، هم زیر مرکز و هم کمی جلوتر روی لبه مانع تست می‌کنیم
                        if wall_hit.contact_point.y <= feet_y + autostep_max_h + 0.08 {
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

                        if !stepped_up && horiz_normal != Vec3::ZERO {
                            center += horiz_normal * (wall_hit.penetration + nudge);
                            any_wall_push = true;
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

        // ۳. اعمال گرانش و چسبیدن نرم به سطح رمپ، زمین یا کف شیب‌دار برکه‌ها (عمیق‌تر در نزدیکی آبشار)
        let mut adjusted_walkable_y = highest_walkable_y;
        if let Some(gy) = adjusted_walkable_y {
            let probe_pos = Vec3::new(center.x, gy, center.z);
            if sample_water_surface_at(config, probe_pos).is_some() {
                let extra_depth = sample_waterfall_depth_offset(config, center.x, center.z);
                adjusted_walkable_y = Some((gy - extra_depth).max(-1.6));
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
                        let cur_feet_y = center.y - center_y_offset;
                        // از درزهای ناچیز کف زمین (اختلاف ارتفاع زیر 15 سانتی‌متر در کره پایینی) صرف‌نظر کن تا کاراکتر گیر نکند
                        if oy < 0.0 && contact.contact_point.y <= cur_feet_y + autostep_max_h {
                            continue;
                        }
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
}

#[derive(Component)]
pub struct PhysicsBullet {
    pub id: u64,
    pub owner_slot: Option<usize>,
    pub owner_team: TeamId,
    pub velocity: Vec3,
    pub damage: f32,
    pub remaining_range: f32,
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
    let z_off = (team_slot_offset as f32) * 2.6;
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
        CustomCharacterController::default(),
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
            owner_slot: 0,
            owner_team: TeamId::Dawn,
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

/// قانون برتری دید ارتفاعی و خط دید موانع (Dota 2 High-Ground + Line-of-Sight) با برد دلخواه
pub fn can_observer_see_point_with_range(
    physics_world: &CustomPhysicsWorld,
    config: &GameConfig,
    observer_feet_pos: Vec3,
    target_feet_pos: Vec3,
    eye_height: f32,
    base_range: f32,
    high_ground_bonus: f32,
) -> bool {
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
    pub blocker: Vec<bool>,
    pub explored: Vec<bool>,
    pub visible_now: Vec<bool>,
    pub allied_sources: Vec<(Vec3, f32)>,
}

impl SharedFogOfWar {
    pub fn from_config(config: &GameConfig, hero_team: TeamId) -> Self {
        let total = FOW_GRID_W * FOW_GRID_H;
        let mut terrain_y = vec![0.0_f32; total];
        let mut blocker = vec![false; total];

        for gz in 0..FOW_GRID_H {
            for gx in 0..FOW_GRID_W {
                let (wx, wz) = Self::cell_to_world(config, gx, gz);
                let idx = gz * FOW_GRID_W + gx;

                let mut best_y = -1.6_f32;
                for plat in &config.map.terrains {
                    let loc = plat.model_transform.location;
                    let scl = plat.model_transform.scale;
                    let hx = plat.size.0 * 0.5 * scl.0.abs();
                    let hz = plat.size.2 * 0.5 * scl.2.abs();
                    if (wx - loc.0).abs() <= hx && (wz - loc.2).abs() <= hz {
                        let top_y = loc.1 + plat.size.1 * 0.5 * scl.1.abs();
                        best_y = best_y.max(top_y);
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
                    best_y = best_y.max(
                        b_loc.1 + bridge.deck_thickness * 0.5 * bridge.model_transform.scale.1.abs(),
                    );
                }

                terrain_y[idx] = best_y;

                let mut blocked = false;
                for mtn in &config.map.mountains {
                    let loc = mtn.model_transform.location;
                    let scl = mtn.model_transform.scale;
                    let hx = mtn.size.0 * 0.5 * scl.0.abs();
                    let hz = mtn.size.2 * 0.5 * scl.2.abs();
                    let inv_rot =
                        Quat::from_rotation_y(-mtn.model_transform.rotation.1.to_radians());
                    let local = inv_rot * Vec3::new(wx - loc.0, 0.0, wz - loc.2);
                    if local.x.abs() <= hx && local.z.abs() <= hz {
                        blocked = true;
                        break;
                    }
                }
                if !blocked {
                    for obs in &config.obstacles {
                        if obs.can_vault {
                            continue;
                        }
                        let loc = obs.model_transform.location;
                        let scl = obs.model_transform.scale;
                        let hx = obs.size.0 * 0.5 * scl.0.abs();
                        let hz = obs.size.2 * 0.5 * scl.2.abs();
                        let inv_rot =
                            Quat::from_rotation_y(-obs.model_transform.rotation.1.to_radians());
                        let local = inv_rot * Vec3::new(wx - loc.0, 0.0, wz - loc.2);
                        if local.x.abs() <= hx && local.z.abs() <= hz {
                            blocked = true;
                            break;
                        }
                    }
                }
                if !blocked {
                    for tree in &config.map.trees {
                        let loc = tree.model_transform.location;
                        let ts = tree
                            .model_transform
                            .scale
                            .0
                            .abs()
                            .max(tree.model_transform.scale.2.abs());
                        let r = (tree.trunk_radius * 1.45 * ts).max(0.85);
                        let tdx = wx - loc.0;
                        let tdz = wz - loc.2;
                        if tdx * tdx + tdz * tdz <= r * r {
                            blocked = true;
                            break;
                        }
                    }
                }
                blocker[idx] = blocked;
            }
        }

        let mut allied_sources = Vec::new();
        for ftn in &config.map.fountains {
            if ftn.team == hero_team {
                let loc = ftn.model_transform.location;
                allied_sources.push((Vec3::new(loc.0, loc.1, loc.2), 24.0));
            }
        }
        for anc in &config.map.ancients {
            if anc.team == hero_team {
                let loc = anc.model_transform.location;
                allied_sources.push((Vec3::new(loc.0, loc.1, loc.2), 26.0));
            }
        }
        for tur in &config.map.turrets {
            if tur.team == hero_team {
                let loc = tur.model_transform.location;
                allied_sources.push((Vec3::new(loc.0, loc.1, loc.2), tur.attack_range + 4.0));
            }
        }
        for bar in &config.map.barracks {
            if bar.team == hero_team {
                let loc = bar.model_transform.location;
                allied_sources.push((Vec3::new(loc.0, loc.1, loc.2), 20.0));
            }
        }

        Self {
            terrain_y,
            blocker,
            explored: vec![false; total],
            visible_now: vec![false; total],
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
    pub fn is_world_point_visible(&self, config: &GameConfig, wx: f32, wz: f32) -> bool {
        if let Some((gx, gz)) = Self::world_to_cell(config, wx, wz) {
            self.visible_now[gz * FOW_GRID_W + gx]
        } else {
            false
        }
    }

    fn reveal_from_source(
        &mut self,
        config: &GameConfig,
        origin: Vec3,
        vision_range: f32,
        ray_count: usize,
    ) {
        let origin_y = Self::world_to_cell(config, origin.x, origin.z)
            .map(|(gx, gz)| self.terrain_y[gz * FOW_GRID_W + gx])
            .unwrap_or(origin.y);

        let step_size = 1.5_f32;
        let steps = (vision_range / step_size).ceil() as usize;

        for r_i in 0..ray_count {
            let angle = (r_i as f32) * (std::f32::consts::PI * 2.0 / ray_count as f32);
            let dx = angle.cos();
            let dz = angle.sin();

            for s_i in 0..=steps {
                let dist = s_i as f32 * step_size;
                if dist > vision_range {
                    break;
                }
                let wx = origin.x + dx * dist;
                let wz = origin.z + dz * dist;
                let Some((gx, gz)) = Self::world_to_cell(config, wx, wz) else {
                    break;
                };
                let idx = gz * FOW_GRID_W + gx;

                let cell_y = self.terrain_y[idx];
                if cell_y - origin_y >= config.map.high_ground_occlusion_min_diff {
                    break;
                }

                self.visible_now[idx] = true;
                self.explored[idx] = true;

                if self.blocker[idx] && dist > 1.8 {
                    break;
                }
            }
        }
    }

    pub fn update_for_hero(&mut self, config: &GameConfig, hero_pos: Vec3, hero: &HeroDef, allied_creeps: &[Vec3]) {
        self.visible_now.fill(false);

        let hero_ground_y = Self::world_to_cell(config, hero_pos.x, hero_pos.z)
            .map(|(gx, gz)| self.terrain_y[gz * FOW_GRID_W + gx])
            .unwrap_or(hero_pos.y);

        let hero_range = hero.vision_range
            + if hero_ground_y >= config.map.high_ground_occlusion_min_diff {
                hero.high_ground_vision_bonus
            } else {
                0.0
            };

        self.reveal_from_source(config, hero_pos, hero_range, 240);

        let allied = self.allied_sources.clone();
        for (src_pos, src_range) in allied {
            self.reveal_from_source(config, src_pos, src_range, 72);
        }
        for &c_pos in allied_creeps {
            self.reveal_from_source(config, c_pos, config.creeps.vision_range, 64);
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
}

#[derive(Resource)]
pub struct ClientPhysicsPredictionState {
    pub latest_input: Option<ClientInputCommand>,
    pub traversal_active: bool,
    pub effective_stance: Stance,
    pub is_dead: bool,
    pub match_ended: bool,
    pub is_paused: bool,
}

impl ClientPhysicsPredictionState {
    pub fn new() -> Self {
        Self {
            latest_input: None,
            traversal_active: false,
            effective_stance: Stance::Run,
            is_dead: false,
            match_ended: false,
            is_paused: false,
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
    pub space_hold_secs: Option<f32>,
    pub hero_armor: f32,
    pub hero_scale: f32,
    pub hero_move_speed_multiplier: f32,
    pub hero_shallow_water_speed_multiplier: f32,
    pub hero_deep_water_speed_multiplier: f32,
    pub hero_vision_range: f32,
    pub hero_weapon: HeroWeaponDef,
    pub ammo_in_mag: u32,
    pub current_move_speed: f32,
    pub last_hits: u32,
    pub denies: u32,
    pub assists: u32,
    pub kills: u32,
    pub deaths: u32,
    pub fog_of_war: SharedFogOfWar,
    pub in_water: bool,
    pub water_splash_cooldown: f32,
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
    pub space_hold_secs: Option<f32>,
    pub hero_armor: f32,
    pub hero_scale: f32,
    pub hero_move_speed_multiplier: f32,
    pub hero_shallow_water_speed_multiplier: f32,
    pub hero_deep_water_speed_multiplier: f32,
    pub hero_vision_range: f32,
    pub hero_weapon: HeroWeaponDef,
    pub ammo_in_mag: u32,
    pub current_move_speed: f32,
    pub last_hits: u32,
    pub denies: u32,
    pub assists: u32,
    pub kills: u32,
    pub deaths: u32,
    pub fog_of_war: SharedFogOfWar,
    pub in_water: bool,
    pub water_splash_cooldown: f32,
    pub traversal: SharedTraversalState,
    pub targets: Vec<SharedTarget>,
    pub targets_visible: Vec<bool>,
    pub creeps: Vec<LaneCreepState>,
    pub turrets: Vec<TurretState>,
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
    pub next_wave_time_secs: f32,
    pub next_camp_spawn_time_secs: f32,
    pub next_creep_id: u64,
    pub bullets_snapshot: Vec<BulletSnapshot>,
    pub grenades_snapshot: Vec<GrenadeSnapshot>,
    pub pending_fx: Vec<GameplayFxEvent>,
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
            space_hold_secs: None,
            hero_armor: hero.armor,
            hero_scale: hero.hero_scale,
            hero_move_speed_multiplier: hero.move_speed_multiplier,
            hero_shallow_water_speed_multiplier: hero.shallow_water_speed_multiplier,
            hero_deep_water_speed_multiplier: hero.deep_water_speed_multiplier,
            hero_vision_range: hero.vision_range,
            ammo_in_mag: hero.weapon.magazine_size,
            hero_weapon: hero.weapon,
            current_move_speed: config.soldier.run_speed * hero.move_speed_multiplier,
            last_hits: 0,
            denies: 0,
            assists: 0,
            kills: 0,
            deaths: 0,
            fog_of_war,
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
            targets_visible: vec![true; targets.len()],
            targets,
            creeps: Vec::new(),
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
            next_wave_time_secs: config.creeps.first_wave_delay_secs,
            next_camp_spawn_time_secs: config.neutral_camps.first_spawn_delay_secs,
            next_creep_id: 1,
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
        std::mem::swap(&mut self.space_hold_secs, &mut p.space_hold_secs);
        std::mem::swap(&mut self.hero_armor, &mut p.hero_armor);
        std::mem::swap(&mut self.hero_scale, &mut p.hero_scale);
        std::mem::swap(&mut self.hero_move_speed_multiplier, &mut p.hero_move_speed_multiplier);
        std::mem::swap(&mut self.hero_shallow_water_speed_multiplier, &mut p.hero_shallow_water_speed_multiplier);
        std::mem::swap(&mut self.hero_deep_water_speed_multiplier, &mut p.hero_deep_water_speed_multiplier);
        std::mem::swap(&mut self.hero_vision_range, &mut p.hero_vision_range);
        std::mem::swap(&mut self.hero_weapon, &mut p.hero_weapon);
        std::mem::swap(&mut self.ammo_in_mag, &mut p.ammo_in_mag);
        std::mem::swap(&mut self.current_move_speed, &mut p.current_move_speed);
        std::mem::swap(&mut self.last_hits, &mut p.last_hits);
        std::mem::swap(&mut self.denies, &mut p.denies);
        std::mem::swap(&mut self.assists, &mut p.assists);
        std::mem::swap(&mut self.kills, &mut p.kills);
        std::mem::swap(&mut self.deaths, &mut p.deaths);
        std::mem::swap(&mut self.fog_of_war, &mut p.fog_of_war);
        std::mem::swap(&mut self.in_water, &mut p.in_water);
        std::mem::swap(&mut self.water_splash_cooldown, &mut p.water_splash_cooldown);
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
                let any_disconnected = self.players.iter().any(|p| p.is_disconnected);
                let (team, name, player_cd) = if let Some(p) = self.players.get(slot_index) {
                    (p.hero_team, p.hero_display_name.clone(), p.pause_cooldown_remaining)
                } else {
                    (self.hero_team, self.hero_display_name.clone(), 0.0)
                };

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
        let (sender_name, sender_team) = if let Some(p) = self.players.get(slot_index) {
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
        let (sender_name, sender_team) = if let Some(p) = self.players.get(slot_index) {
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

    pub fn set_slot_input(&mut self, slot_index: usize, cmd: ClientInputCommand) {
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
                space_hold_secs: None,
                hero_armor: hero.armor,
                hero_scale: hero.hero_scale,
                hero_move_speed_multiplier: hero.move_speed_multiplier,
                hero_shallow_water_speed_multiplier: hero.shallow_water_speed_multiplier,
                hero_deep_water_speed_multiplier: hero.deep_water_speed_multiplier,
                hero_vision_range: hero.vision_range,
                ammo_in_mag: hero.weapon.magazine_size,
                hero_weapon: hero.weapon,
                current_move_speed: config.soldier.run_speed * hero.move_speed_multiplier,
                last_hits: 0,
                denies: 0,
                assists: 0,
                kills: 0,
                deaths: 0,
                fog_of_war: SharedFogOfWar::from_config(config, lp.team),
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
                    travel_dir: facing_dir,
                },
            });
        }
    }

    pub fn build_snapshot_for_slot(&mut self, config: &GameConfig, slot_index: usize) -> ServerWorldSnapshot {
        if self.players.is_empty() {
            return self.build_snapshot(config);
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

        self.swap_slot_in(slot_index);
        let mut snap = self.build_snapshot(config);
        let mut others = Vec::with_capacity(self.players.len().saturating_sub(1));
        for p in &self.players {

            if p.slot_index == slot_index {
                continue;
            }
            let is_vis = p.hero_team == self.hero_team
                || self.fog_of_war.is_world_point_visible(config, p.soldier_pos.x, p.soldier_pos.z);
            // Server Authority: اگر دشمن در مه جنگ باشد، مختصات و اطلاعات او اصلاً به کلاینت ارسال نمی‌شود!
            if !is_vis {
                continue;
            }
            let trav_anim = if p.traversal.is_active {
                Some(p.traversal.anim_name.clone())
            } else {
                None
            };
            let upper_anim = if p.grenade_elapsed_and_duration.is_some() {
                Some("toss_grenade".to_string())
            } else if p.reload_remaining.is_some() {
                Some("reloading".to_string())
            } else if p.firing_anim_remaining.is_some() {
                Some("firing_rifle".to_string())
            } else {
                None
            };
            others.push(RemotePlayerSnapshot {
                slot_index: p.slot_index,
                team: p.hero_team,
                hero_id: p.hero_id.clone(),
                display_name: p.hero_display_name.clone(),
                model_path: p.hero_model_path.clone(),
                hero_scale: p.hero_scale,
                weapon_kind: p.hero_weapon.weapon_kind,
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
                ammo_in_mag: p.ammo_in_mag,
                magazine_size: p.hero_weapon.magazine_size,
                is_visible: true,
            });
        }
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
                let is_vis = c.team == self.hero_team
                    || self.fog_of_war.is_world_point_visible(config, c.pos.x, c.pos.z);
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
                    is_visible: true,
                })
            })
            .collect();
        self.swap_slot_in(slot_index);
        snap
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
        self.space_hold_secs = None;
        self.hero_armor = hero.armor;
        self.hero_scale = hero.hero_scale;
        self.hero_move_speed_multiplier = hero.move_speed_multiplier;
        self.hero_shallow_water_speed_multiplier = hero.shallow_water_speed_multiplier;
        self.hero_deep_water_speed_multiplier = hero.deep_water_speed_multiplier;
        self.hero_vision_range = hero.vision_range;
        self.ammo_in_mag = hero.weapon.magazine_size;
        self.hero_weapon = hero.weapon;
        self.current_move_speed = config.soldier.run_speed * hero.move_speed_multiplier;
        self.fog_of_war = SharedFogOfWar::from_config(config, hero.team);
    }

    pub fn build_snapshot(&mut self, config: &GameConfig) -> ServerWorldSnapshot {
        self.tick += 1;
        ServerWorldSnapshot {
            tick: self.tick,
            match_elapsed_secs: self.match_elapsed_secs,
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
                    self.hero_hp_regen_per_sec + fountain_bonus
                },
                in_fountain: !self.is_dead
                    && fountain_regen_bonus_for_pos(config, self.hero_team, self.soldier_pos) > 0.0,
                stamina: self.hero_stamina,
                max_stamina: self.hero_max_stamina,
                stamina_regen_per_sec: self.hero_stamina_regen_per_sec,
                armor: self.hero_armor,
                hero_scale: self.hero_scale,
                move_speed: self.current_move_speed,
                vision_range: self.hero_vision_range,
                weapon_kind: self.hero_weapon.weapon_kind,
                ammo_in_mag: self.ammo_in_mag,
                magazine_size: self.hero_weapon.magazine_size,
                fire_rate_secs: self.hero_weapon.fire_rate_secs,
                reload_duration_secs: self.hero_weapon.reload_duration_secs,
                weapon_damage: self.hero_weapon.damage,
                projectile_speed: self.hero_weapon.projectile_speed,
                weapon_max_range: self.hero_weapon.max_range,
                last_hits: self.last_hits,
                denies: self.denies,
                assists: self.assists,
                kills: self.kills,
                deaths: self.deaths,
                respawn_remaining: if self.is_dead { self.respawn_remaining.max(0.0) } else { 0.0 },
            },
            other_players: Vec::new(),
            top_bar_heroes: Vec::new(),
            fog_visible_mask: self.fog_of_war.pack_visible_mask(),
            bullets: self.bullets_snapshot.clone(),
            grenades: self.grenades_snapshot.clone(),
            creeps: Vec::new(),
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


/// محاسبه بونوس بازسازی سلامتی در چشمه پایگاه تیم خودی بر اساس config.ron
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
        if pos.distance(ftn_pos) <= ftn.heal_radius {
            best_bonus = best_bonus.max(ftn.heal_per_sec);
        }
    }
    best_bonus
}

pub fn grant_xp_to_player_slot(
    p: &mut PlayerSlotState,
    amount: u32,
    config: &GameConfig,
    pending_fx: &mut Vec<GameplayFxEvent>,
) {
    if amount == 0 || p.hero_level >= 25 {
        return;
    }
    p.hero_xp = p.hero_xp.saturating_add(amount);
    let growth = &config.active_hero(p.hero_index).level_growth;

    while p.hero_level < 25 {
        let next_idx = p.hero_level as usize;
        if p.hero_xp >= config.combat.xp_thresholds_per_level[next_idx] {
            p.hero_level += 1;
            p.hero_weapon.fire_rate_secs = (p.hero_weapon.fire_rate_secs
                - growth.fire_rate_reduction_per_level)
                .max(config.combat.min_fire_rate_secs);
            p.hero_weapon.reload_duration_secs = (p.hero_weapon.reload_duration_secs
                - growth.reload_time_reduction_per_level)
                .max(config.combat.min_reload_duration_secs);
            p.hero_weapon.damage += growth.damage_per_level;
            p.hero_weapon.max_range += growth.max_range_per_level;
            p.hero_max_hp += growth.max_hp_per_level;
            p.hero_hp = (p.hero_hp + growth.max_hp_per_level).min(p.hero_max_hp);
            p.hero_hp_regen_per_sec += growth.hp_regen_per_level;
            p.hero_stamina_regen_per_sec += growth.stamina_regen_per_level;
            p.hero_armor += growth.armor_per_level;

            pending_fx.push(GameplayFxEvent::HeroLeveledUp {
                pos: p.soldier_pos,
                new_level: p.hero_level,
            });
        } else {
            break;
        }
    }
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
    world.reload_remaining = None;
    world.grenade_elapsed_and_duration = None;
    world.firing_anim_remaining = None;
    world.space_hold_secs = None;
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
    match_state: Option<Res<AuthoritativeMatchState>>,
    physics_world: Res<CustomPhysicsWorld>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut soldier_query: Query<(&PhysicsSoldier, &mut Transform, &mut CustomCharacterController)>,
    obstacle_query: Query<&PhysicsObstacle>,
    target_query: Query<&PhysicsTarget>,
    turret_query: Query<&PhysicsTurret>,
    barracks_query: Query<&PhysicsBarracks>,
    ancient_query: Query<&PhysicsAncient>,
) {
    if match_state
        .as_ref()
        .is_some_and(|m| m.phase != MatchPhase::InGame)
    {
        return;
    }
    let dt = time.delta_secs();
    world.step_pause_system(dt, &config);
    if world.winning_team.is_some() || world.pause_phase != PausePhase::Running {
        for (_, _, mut controller) in &mut soldier_query {
            controller.translation = None;
        }
        return;
    }
    let hero_aim_targets: Vec<(usize, TeamId, Vec3, f32, Stance, bool)> = world
        .players
        .iter()
        .map(|p| {
            (
                p.slot_index,
                p.hero_team,
                p.soldier_pos,
                p.hero_scale,
                p.effective_stance,
                p.is_dead,
            )
        })
        .collect();

    for (soldier_marker, mut soldier_tf, mut controller) in &mut soldier_query {
        let slot_idx = soldier_marker.slot_index;
        let has_slots = !world.players.is_empty();
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
        if let Some(input) = world.active_input.clone() {
            shared_drive_single_player(
                &mut commands,
                dt,
                &config,
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
                &hero_aim_targets,
            );
        }
        if has_slots {
            world.swap_slot_in(slot_idx);
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

    // حل برخورد فیزیکی هیروها با سربازان لاین (مانع فیزیکی کامل، عدم عبور از داخل سربازها)
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
}

fn shared_drive_single_player(
    commands: &mut Commands,
    dt: f32,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    world: &mut AuthoritativeWorldState,
    input: &ClientInputCommand,
    soldier_tf: &mut Transform,
    controller: &mut CustomCharacterController,
    obstacle_query: &Query<&PhysicsObstacle>,
    target_query: &Query<&PhysicsTarget>,
    turret_query: &Query<&PhysicsTurret>,
    barracks_query: &Query<&PhysicsBarracks>,
    ancient_query: &Query<&PhysicsAncient>,
    hero_aim_targets: &[(usize, TeamId, Vec3, f32, Stance, bool)],
) {

    // ۱. بررسی کلید X (تست مرگ) و تایمر زنده شدن مجدد در چشمه پایگاه
    if input.kill_toggle_just_pressed {
        if world.is_dead {
            world.is_dead = false;
            world.base_stance = Stance::Run;
        } else {
            world.traversal.is_active = false;
            trigger_random_death(world, config);
        }
    }

    if world.is_dead {
        controller.translation = None;
        world.respawn_remaining -= dt;
        if world.respawn_remaining <= 0.0 {
            world.is_dead = false;
            world.hero_hp = world.hero_max_hp;
            world.hero_stamina = world.hero_max_stamina;
            world.space_hold_secs = None;
            world.ammo_in_mag = world.hero_weapon.magazine_size;
            world.base_stance = Stance::Run;
            let spawn_pos = world.active_spawn_pos;
            world.soldier_pos = spawn_pos;
            soldier_tf.translation =
                spawn_pos + Vec3::Y * config.physics.soldier_capsule_center_y;
        }
    } else {
        // ۲. بازسازی سلامتی در ثانیه (HP Regen پایه + بونوس چشمه تیم خودی) و آپدیت تایمرهای بالاتنه
        let fountain_bonus = fountain_regen_bonus_for_pos(config, world.hero_team, world.soldier_pos);
        let effective_hp_regen = world.hero_hp_regen_per_sec + fountain_bonus;
        world.hero_hp = (world.hero_hp + effective_hp_regen * dt).min(world.hero_max_hp);
        world.hero_stamina = (world.hero_stamina + world.hero_stamina_regen_per_sec * dt).min(world.hero_max_stamina);

        if let Some(rem) = &mut world.reload_remaining {
            *rem -= dt;
            if *rem <= 0.0 {
                world.reload_remaining = None;
                world.ammo_in_mag = world.hero_weapon.magazine_size;
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
            world.reload_remaining = Some(world.hero_weapon.reload_duration_secs);
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

        // ۵. هدف‌گیری ۳بعدی دقیق روی سطوح مختلف و قفل نرم روی هیروی دشمن (ایستاده، نشسته یا پشت سنگر)
        if !world.traversal.is_active
            && input.has_aim_ray
            && input.aim_ray_dir.y.abs() > config.physics.aim_ray_min_dir_y_epsilon
        {
            let ray_dir = input.aim_ray_dir.normalize();
            let max_aim_dist = config.combat.aim_lock_ray_max_distance;
            let aim_groups = soldier_collision_groups();

            // الف) ابتدا برخورد اشعه ماوس با هندسه واقعی نقشه (پلتفرم‌ها، دیوارها، رمپ‌ها و اهداف) را پیدا کن
            let world_hit_opt = physics_world.cast_ray(
                input.aim_ray_origin,
                ray_dir,
                max_aim_dist,
                aim_groups,
                |ent| {
                    if let Ok(t) = target_query.get(ent) {
                        !world.targets[t.index].is_dead
                    } else {
                        true
                    }
                },
            );
            let world_toi = world_hit_opt.as_ref().map(|(_, h)| h.toi).unwrap_or(f32::MAX);

            // ب) بررسی برخورد تحلیلی اشعه ماوس با کپسول هیروهای دشمن قابل مشاهده (فقط اگر قبل از مانع به هیرو بخورد)
            let mut best_enemy_hit: Option<(f32, Vec3)> = None;
            for &(e_slot, e_team, e_pos, e_scale, e_stance, e_dead) in hero_aim_targets {
                if e_slot == world.active_slot || e_team == world.hero_team || e_dead {
                    continue;
                }
                if !world.fog_of_war.is_world_point_visible(config, e_pos.x, e_pos.z) {
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
                    if toi <= world_toi + 0.05
                        && best_enemy_hit.map_or(true, |(prev_toi, _)| toi < prev_toi)
                    {
                        let chest_h = if e_stance == Stance::Crouch {
                            config.combat.hero_crouch_aim_chest_height * e_scale
                        } else {
                            config.combat.hero_stand_aim_chest_height * e_scale
                        };
                        best_enemy_hit = Some((toi, e_pos + Vec3::Y * chest_h));
                    }
                }
            }

            // ج) بررسی برخورد اشعه موس با سربازان لاین و کمپ (سرباز دشمن/کمپ یا سرباز خودی قابل دیناید)
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
                if is_enemy && !world.fog_of_war.is_world_point_visible(config, c.pos.x, c.pos.z) {
                    continue;
                }
                let c_stats = resolve_creep_stats(config, c.creep_type_id.as_deref());
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
                    let prev_best_toi = best_creep_hit.map(|(_, t, _)| t).unwrap_or(world_toi + 0.05);
                    if toi <= world_toi + 0.05 && toi < prev_best_toi {
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

            let hero_wins_aim = match (best_enemy_hit, best_creep_hit) {
                (Some((h_t, _)), Some((_, c_t, _))) => h_t <= c_t,
                (Some(_), None) => true,
                _ => false,
            };

            if hero_wins_aim {
                resolved_hit = Some(best_enemy_hit.unwrap().1);
                locked_enemy = true;
            } else if let Some((c_id, _, c_chest_pos)) = best_creep_hit {
                resolved_hit = Some(c_chest_pos);
                locked_enemy = true;
                hovered_creep = Some(c_id);
            } else if let Some((hit_ent, hit)) = world_hit_opt {
                if let Ok(pt) = turret_query.get(hit_ent) {
                    if pt.team != world.hero_team && !world.turrets[pt.index].is_dead {
                        resolved_hit = Some(hit.point);
                        locked_enemy = true;
                        hovered_turret = Some(pt.index);
                    } else {
                        resolved_hit = Some(hit.point);
                    }
                } else if let Ok(pb) = barracks_query.get(hit_ent) {
                    if pb.team != world.hero_team && !world.barracks[pb.index].is_dead {
                        resolved_hit = Some(hit.point);
                        locked_enemy = true;
                        hovered_barracks = Some(pb.index);
                    } else {
                        resolved_hit = Some(hit.point);
                    }
                } else if let Ok(pa) = ancient_query.get(hit_ent) {
                    if pa.team != world.hero_team && !world.ancients[pa.index].is_dead {
                        resolved_hit = Some(hit.point);
                        locked_enemy = true;
                        hovered_ancient = Some(pa.index);
                    } else {
                        resolved_hit = Some(hit.point);
                    }
                } else if target_query.get(hit_ent).is_ok() {
                    resolved_hit = Some(hit.point);
                    locked_enemy = true;
                } else if hit.normal.y > 0.7 {
                    // تقاطع خود اشعه دوربین با صفحه ارتفاعی هم‌تراز سینه روی همان سطح زمین
                    // تا انتهای خط لیزر دقیقاً روی پیکسل مرکز نشانه‌گر موس منطبق بماند
                    let elevated_plane_y = hit.point.y + config.soldier.aim_plane_height_offset;
                    let t_elevated = (elevated_plane_y - input.aim_ray_origin.y) / ray_dir.y;
                    if t_elevated > 0.0 {
                        resolved_hit = Some(input.aim_ray_origin + ray_dir * t_elevated);
                    } else {
                        resolved_hit = Some(hit.point);
                    }
                } else {
                    // برخورد با دیواره عمودی، صخره یا مانع: خود نقطه برخورد روی اشعه موس است
                    resolved_hit = Some(hit.point);
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
            world,
            input,
            config,
            physics_world,
            obstacle_query,
            target_query,
            soldier_tf,
            controller,
            dt,
        );

        // ۸. جابجایی فیزیکی ۳بعدی کاراکتر همراه با مقاومت طبیعی آب و رانش جریان برکه‌ها
        let water_sample = sample_water_surface_at(config, world.soldier_pos);
        let was_in_water = world.in_water;
        world.in_water = water_sample.is_some();
        world.water_splash_cooldown = (world.water_splash_cooldown - dt).max(0.0);

        if let Some((water_y, flow_dir)) = water_sample {
            let water_depth = (water_y - world.soldier_pos.y).max(0.0);
            let is_deep_water = water_depth >= 0.85;

            let hero_cfg = config.active_hero(world.hero_index);
            let hero_move_mult = hero_cfg.move_speed_multiplier;
            let shallow_mult = hero_cfg.shallow_water_speed_multiplier;
            let deep_mult = hero_cfg.deep_water_speed_multiplier;

            if is_deep_water && !world.traversal.is_active {
                if world.is_moving {
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
                    } * hero_move_mult;
                    let depth_mult = if is_deep_water {
                        deep_mult
                    } else {
                        shallow_mult
                    };
                    let water_speed = base_speed * depth_mult;
                    world.current_move_speed = water_speed;
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
                let hero_move_mult = config.active_hero(world.hero_index).move_speed_multiplier;
                let speed = match world.effective_stance {
                    Stance::Run => config.soldier.run_speed,
                    Stance::Walk => config.soldier.walk_speed,
                    Stance::Crouch => config.soldier.crouch_speed,
                } * hero_move_mult;
                world.current_move_speed = speed;
                let desired_delta = world.move_input.normalize() * speed * dt;
                controller.translation = Some(Vec3::new(desired_delta.x, grav_y, desired_delta.z));
            } else {
                controller.translation = Some(Vec3::new(0.0, grav_y, 0.0));
            }
        }
    }

    // ۹. شلیک گلوله و پرتاب نارنجک
    step_shooting_and_grenade_spawn(commands, world, input, config, dt);
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

        if world.traversal.kind == TraversalKind::ObstacleVault
            || world.traversal.kind == TraversalKind::JumpOver
        {
            controller.translation = None;
            let ground_y = if world.traversal.kind == TraversalKind::JumpOver {
                let ray_orig = Vec3::new(current_pos.x, current_pos.y + 4.0, current_pos.z);
                physics_world
                    .cast_ray(
                        ray_orig,
                        Vec3::NEG_Y,
                        10.0,
                        soldier_collision_groups(),
                        |ent| !obstacle_query.contains(ent) && !target_query.contains(ent),
                    )
                    .map(|(_, hit)| hit.point.y)
                    .unwrap_or(current_pos.y)
            } else {
                current_pos.y
            };
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

    if let Some((water_y, _)) = sample_water_surface_at(config, world.soldier_pos) {
        if (water_y - world.soldier_pos.y) >= 0.85 {
            world.space_hold_secs = None;
            return;
        }
    }

    let travel_dir = if world.is_moving {
        world.move_input.normalize()
    } else {
        world.aim_world_dir
    };

    let ray_origin = world.soldier_pos + Vec3::Y * config.soldier.vault_ray_height;
    let query_groups = CollisionGroups::new(GROUP_SOLDIER, GROUP_WORLD);

    let check_vault = || {
        physics_world
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
            .is_some()
    };

    let (kind, anim_name, speed, stamina_cost) = if world.effective_stance == Stance::Run {
        if input.space_just_pressed && world.space_hold_secs.is_none() {
            if check_vault() {
                if world.hero_stamina < world.hero_slide_stamina_cost {
                    return;
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
                world.traversal.target_pos = Vec3::new(
                    world.soldier_pos.x + travel_dir.x * config.soldier.vault_travel_distance,
                    world.soldier_pos.y,
                    world.soldier_pos.z + travel_dir.z * config.soldier.vault_travel_distance,
                );
                world.traversal.travel_dir = travel_dir;
                world.soldier_facing_dir = travel_dir;
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
        if check_vault() {
            if world.hero_stamina < world.hero_slide_stamina_cost {
                return;
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
            world.traversal.target_pos = Vec3::new(
                world.soldier_pos.x + travel_dir.x * config.soldier.vault_travel_distance,
                world.soldier_pos.y,
                world.soldier_pos.z + travel_dir.z * config.soldier.vault_travel_distance,
            );
            world.traversal.travel_dir = travel_dir;
            world.soldier_facing_dir = travel_dir;
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

    let exact_duration = config.exact_anim_duration(anim_name);
    let distance = speed * exact_duration;

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
                true
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
        (input.fire_just_pressed || input.fire_held) && world.fire_cooldown_remaining <= 0.0;

    if wants_fire {
        if world.ammo_in_mag == 0 {
            world.reload_remaining = Some(world.hero_weapon.reload_duration_secs);
            world.firing_anim_remaining = None;
            return;
        }

        world.ammo_in_mag -= 1;
        world.fire_cooldown_remaining = world.hero_weapon.fire_rate_secs;
        let dur = config.exact_anim_duration("firing_rifle");
        world.firing_anim_remaining = Some(dur);

        let muzzle_pos = resolve_muzzle_pos(world, input, config);
        let to_aim = world.aim_hit_pos - muzzle_pos;
        let shot_dir = if to_aim.length_squared() > config.physics.muzzle_aim_min_dist_sq {
            let mut dir = to_aim.normalize();
            // محدود کردن زاویه عمودی از روی کانفیگ تا تیر بین سطوح بالا و پایین روان حرکت کند
            let max_sin = config.combat.max_vertical_aim_pitch_sin;
            dir.y = dir.y.clamp(-max_sin, max_sin);
            dir.normalize()
        } else {
            world.aim_world_dir
        };

        let pellets = world.hero_weapon.pellets_per_shot;
        let crouch_accuracy_mult = if world.effective_stance == Stance::Crouch {
            0.60
        } else {
            1.0
        };
        let spread_rad = world.hero_weapon.spread_rad * crouch_accuracy_mult;
        let proj_speed = world.hero_weapon.projectile_speed;
        let dmg = world.hero_weapon.damage;
        let max_range = world.hero_weapon.max_range;

        let mut first_dir = shot_dir;
        for p_idx in 0..pellets {
            let spread = (world.next_rand() - 0.5) * spread_rad;
            let final_dir = (Quat::from_rotation_y(spread) * shot_dir).normalize();
            if p_idx == 0 {
                first_dir = final_dir;
            }
            let id = world.next_entity_id;
            world.next_entity_id += 1;
            commands.spawn((
                PhysicsBullet {
                    id,
                    owner_slot: Some(world.active_slot),
                    owner_team: world.hero_team,
                    velocity: final_dir * proj_speed,
                    damage: dmg,
                    remaining_range: max_range,
                },
                Transform::from_translation(muzzle_pos),
            ));
        }

        world.pending_fx.push(GameplayFxEvent::ShotFired {
            muzzle_pos,
            shot_dir: first_dir,
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
    let mut turret_idx_by_entity = HashMap::new();
    for (entity, p_turret) in &turret_query {
        turret_idx_by_entity.insert(entity, p_turret.index);
    }
    let mut barracks_idx_by_entity = HashMap::new();
    for (entity, p_barrack) in &barracks_query {
        barracks_idx_by_entity.insert(entity, p_barrack.index);
    }
    let mut ancient_idx_by_entity = HashMap::new();
    for (entity, p_anc) in &ancient_query {
        ancient_idx_by_entity.insert(entity, p_anc.index);
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
        let world_inner = &mut *world;
        for (idx, target) in world_inner.targets.iter_mut().enumerate() {
            if !target.is_dead
                && target.base_pos.distance(pos) < config.combat.grenade_target_kill_radius
            {
                target.hp = 0.0;
                target.is_dead = true;
                target.respawn_remaining = target.respawn_duration;
                let t_pos = target.base_pos;
                if let Some(&t_ent) = target_entity_by_idx.get(&idx) {
                    commands.entity(t_ent).insert(ColliderDisabled);
                }
                world_inner.last_hits += 1;
                world_inner.hero_gold += 35;
                if world_inner.soldier_pos.distance(t_pos) <= world_inner.hero_xp_gain_radius {
                    world_inner.hero_xp += 45;
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
        bullet.remaining_range -= step_dist;

        let mut best_hero_hit: Option<(usize, f32)> = None;
        let mut best_creep_hit: Option<(usize, f32)> = None;
        if step_dist > f32::EPSILON {
            for (p_idx, p) in world.players.iter().enumerate() {
                if Some(p.slot_index) == bullet.owner_slot || p.hero_team == bullet.owner_team || p.is_dead {
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
            let impact_pos = old_pos + step_dir * h_toi;
            let armor = world.players[enemy_idx].hero_armor;
            let dmg_mult = dota2_armor_multiplier(armor);
            let effective_dmg = (bullet.damage * dmg_mult).max(1.0);

            world.players[enemy_idx].hero_hp =
                (world.players[enemy_idx].hero_hp - effective_dmg).max(0.0);

            let enemy_killed = world.players[enemy_idx].hero_hp <= 0.0
                && !world.players[enemy_idx].is_dead;
            if enemy_killed {
                world.swap_slot_in(enemy_idx);
                trigger_random_death(&mut world, &config);
                world.swap_slot_in(enemy_idx);

                if let Some(owner_slot) = bullet.owner_slot {
                    let world_inner = &mut *world;
                    if let Some(attacker) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                        attacker.kills += 1;
                        attacker.hero_gold += 225;
                        grant_xp_to_player_slot(attacker, 150, &config, &mut world_inner.pending_fx);
                    }
                }
            }

            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
            });
            world.pending_fx.push(GameplayFxEvent::BulletImpact {
                impact_pos,
                bullet_dir: step_dir,
                target_destroyed: enemy_killed,
                trauma_add: config.combat.shot_fire_trauma_add,
                trauma_max: config.combat.shot_fire_trauma_max,
            });
            commands.entity(entity).despawn();
            continue;
        }

        let creep_wins = match (best_creep_hit, ray_hit.as_ref()) {
            (Some((_, c_toi)), Some((_, _, s_toi))) => c_toi <= *s_toi,
            (Some(_), None) => true,
            _ => false,
        };

        if creep_wins {
            let (c_idx, c_toi) = best_creep_hit.unwrap();
            let impact_pos = old_pos + step_dir * c_toi;
            let c_stats = resolve_creep_stats(&config, world.creeps[c_idx].creep_type_id.as_deref());
            let (_, _, armor_bonus, _, _) = lane_creep_buffs(
                &config,
                &world.barracks,
                world.creeps[c_idx].team,
                world.creeps[c_idx].lane,
                world.creeps[c_idx].camp_index.is_none(),
            );
            let dmg_mult = dota2_armor_multiplier(c_stats.armor + armor_bonus);
            let effective_dmg = (bullet.damage * dmg_mult).max(1.0);
            world.creeps[c_idx].hp = (world.creeps[c_idx].hp - effective_dmg).max(0.0);
            if let Some(hit_camp_idx) = world.creeps[c_idx].camp_index {
                for c in &mut world.creeps {
                    if c.camp_index == Some(hit_camp_idx) && !c.is_dead {
                        c.is_aggroed = true;
                    }
                }
            }

            let creep_killed = world.creeps[c_idx].hp <= 0.0 && !world.creeps[c_idx].is_dead;
            if creep_killed {
                world.creeps[c_idx].is_dead = true;
                world.creeps[c_idx].anim_state = CreepAnimState::Die;
                world.creeps[c_idx].despawn_timer = c_stats.die_despawn_delay_secs;
                let creep_pos = world.creeps[c_idx].pos;
                let creep_team = world.creeps[c_idx].team;
                let gold_bounty = c_stats.gold_bounty;
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
                                &mut world_inner.pending_fx,
                            );
                        }
                    }
                } else {
                    if let Some(owner_slot) = bullet.owner_slot {
                        let world_inner = &mut *world;
                        if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                            killer.last_hits += 1;
                            killer.hero_gold += gold_bounty;
                            world_inner.pending_fx.push(GameplayFxEvent::LastHitGold {
                                pos: creep_pos,
                                gold: gold_bounty,
                            });
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
                                && p.soldier_pos.distance(creep_pos) <= p.hero_xp_gain_radius
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
                                &mut world_inner.pending_fx,
                            );
                        }
                    }
                }
            }

            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: impact_pos,
            });
            world.pending_fx.push(GameplayFxEvent::BulletImpact {
                impact_pos,
                bullet_dir: step_dir,
                target_destroyed: creep_killed,
                trauma_add: config.combat.shot_fire_trauma_add * 0.5,
                trauma_max: config.combat.shot_fire_trauma_max,
            });
            commands.entity(entity).despawn();
            continue;
        }

        if let Some((hit_entity, impact_pos, _)) = ray_hit {
            let mut target_destroyed = false;
            if let Some(&tur_idx) = turret_idx_by_entity.get(&hit_entity) {
                let turret_team = world.turrets[tur_idx].team;
                let turret_invuln = world.turrets[tur_idx].is_invulnerable;
                let turret_dead = world.turrets[tur_idx].is_dead;
                if bullet.owner_team != turret_team && !turret_invuln && !turret_dead {
                    let dmg_mult = dota2_armor_multiplier(world.turrets[tur_idx].armor);
                    let eff_dmg = (bullet.damage * dmg_mult).max(1.0);
                    world.turrets[tur_idx].hp = (world.turrets[tur_idx].hp - eff_dmg).max(0.0);
                    if world.turrets[tur_idx].hp <= 0.0 && !world.turrets[tur_idx].is_dead {
                        world.turrets[tur_idx].is_dead = true;
                        world.turrets[tur_idx].aim_target_pos = None;
                        target_destroyed = true;
                        commands.entity(hit_entity).insert(ColliderDisabled);
                        let tur_pos = world.turrets[tur_idx].pos;
                        let gold_b = config.map.turrets[tur_idx].gold_bounty;
                        let xp_b = config.map.turrets[tur_idx].xp_bounty;
                        if let Some(owner_slot) = bullet.owner_slot {
                            let world_inner = &mut *world;
                            if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                                killer.last_hits += 1;
                                killer.hero_gold += gold_b;
                                world_inner.pending_fx.push(GameplayFxEvent::LastHitGold {
                                    pos: tur_pos + Vec3::Y * 2.5,
                                    gold: gold_b,
                                });
                            }
                        }
                        let killer_team = bullet.owner_team;
                        let eligible: Vec<usize> = world
                            .players
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| !p.is_dead && p.hero_team == killer_team && p.soldier_pos.distance(tur_pos) <= p.hero_xp_gain_radius)
                            .map(|(i, _)| i)
                            .collect();
                        if !eligible.is_empty() && xp_b > 0 {
                            let share = xp_b / (eligible.len() as u32);
                            let world_inner = &mut *world;
                            for idx in eligible {
                                grant_xp_to_player_slot(&mut world_inner.players[idx], share, &config, &mut world_inner.pending_fx);
                            }
                        }
                    }
                }
            } else if let Some(&bar_idx) = barracks_idx_by_entity.get(&hit_entity) {
                let bar_team = world.barracks[bar_idx].team;
                let bar_lane = world.barracks[bar_idx].lane;
                let bar_kind = world.barracks[bar_idx].kind;
                let bar_invuln = world.barracks[bar_idx].is_invulnerable;
                let bar_dead = world.barracks[bar_idx].is_dead;
                if bullet.owner_team != bar_team && !bar_invuln && !bar_dead {
                    let dmg_mult = dota2_armor_multiplier(world.barracks[bar_idx].armor);
                    let eff_dmg = (bullet.damage * dmg_mult).max(1.0);
                    world.barracks[bar_idx].hp = (world.barracks[bar_idx].hp - eff_dmg).max(0.0);
                    if world.barracks[bar_idx].hp <= 0.0 && !world.barracks[bar_idx].is_dead {
                        world.barracks[bar_idx].is_dead = true;
                        target_destroyed = true;
                        commands.entity(hit_entity).insert(ColliderDisabled);
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
                        if let Some(owner_slot) = bullet.owner_slot {
                            let world_inner = &mut *world;
                            if let Some(killer) = world_inner.players.iter_mut().find(|p| p.slot_index == owner_slot) {
                                killer.last_hits += 1;
                                killer.hero_gold += gold_b;
                                world_inner.pending_fx.push(GameplayFxEvent::LastHitGold {
                                    pos: bar_pos + Vec3::Y * 2.5,
                                    gold: gold_b,
                                });
                            }
                        }
                        let killer_team = bullet.owner_team;
                        let eligible: Vec<usize> = world
                            .players
                            .iter()
                            .enumerate()
                            .filter(|(_, p)| !p.is_dead && p.hero_team == killer_team && p.soldier_pos.distance(bar_pos) <= p.hero_xp_gain_radius)
                            .map(|(i, _)| i)
                            .collect();
                        if !eligible.is_empty() && xp_b > 0 {
                            let share = xp_b / (eligible.len() as u32);
                            let world_inner = &mut *world;
                            for idx in eligible {
                                grant_xp_to_player_slot(&mut world_inner.players[idx], share, &config, &mut world_inner.pending_fx);
                            }
                        }
                    }
                }
            } else if let Some(&anc_idx) = ancient_idx_by_entity.get(&hit_entity) {
                let anc_team = world.ancients[anc_idx].team;
                let anc_invuln = world.ancients[anc_idx].is_invulnerable;
                let anc_dead = world.ancients[anc_idx].is_dead;
                if bullet.owner_team != anc_team && !anc_invuln && !anc_dead {
                    let dmg_mult = dota2_armor_multiplier(world.ancients[anc_idx].armor);
                    let eff_dmg = (bullet.damage * dmg_mult).max(1.0);
                    world.ancients[anc_idx].hp = (world.ancients[anc_idx].hp - eff_dmg).max(0.0);
                    if world.ancients[anc_idx].hp <= 0.0 && !world.ancients[anc_idx].is_dead {
                        world.ancients[anc_idx].is_dead = true;
                        target_destroyed = true;
                        commands.entity(hit_entity).insert(ColliderDisabled);
                        let anc_pos = world.ancients[anc_idx].pos;
                        let winner = if anc_team == TeamId::Dawn {
                            TeamId::Dusk
                        } else {
                            TeamId::Dawn
                        };
                        world.winning_team = Some(winner);
                        world.pending_fx.push(GameplayFxEvent::GrenadeExploded {
                            pos: anc_pos + Vec3::Y * 3.0,
                            trauma_add: config.combat.grenade_explosion_trauma_add,
                            trauma_max: config.combat.grenade_explosion_trauma_max,
                        });
                    }
                }
            } else if let Some(&t_idx) = target_idx_by_entity.get(&hit_entity) {
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
                                grant_xp_to_player_slot(attacker, 45, &config, &mut world_inner.pending_fx);
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
            // بررسی برخورد گلوله با سطح آب برکه‌ها و رودخانه‌ها
            let mid_pos = (old_pos + new_pos) * 0.5;
            if let Some((water_y, _)) = sample_water_surface_at(&config, mid_pos) {
                let crossed_water = (old_pos.y >= water_y - 0.15 && new_pos.y <= water_y + 0.15)
                    || (mid_pos.y - water_y).abs() <= 0.35;
                if crossed_water {
                    world.pending_fx.push(GameplayFxEvent::WaterSplash {
                        pos: Vec3::new(mid_pos.x, water_y, mid_pos.z),
                        intensity: 1.15,
                    });
                }
            }

            tf.translation = new_pos;
            world.pending_fx.push(GameplayFxEvent::BulletTrailSegment {
                start: old_pos,
                end: new_pos,
            });

            if bullet.remaining_range <= 0.0 {
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

    let dead_flags: Vec<bool> = world.targets.iter().map(|t| t.is_dead).collect();
    let laser_hero_targets: Vec<(usize, TeamId, Vec3, f32, Stance, bool)> = world
        .players
        .iter()
        .map(|p| {
            (
                p.slot_index,
                p.hero_team,
                p.soldier_pos,
                p.hero_scale,
                p.effective_stance,
                p.is_dead,
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
            let muzzle_pos = resolve_muzzle_pos(&world, &input, &config);
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
                for &(e_slot, e_team, e_pos, e_scale, e_stance, e_dead) in &laser_hero_targets {
                    if e_slot == world.active_slot || e_team == world.hero_team || e_dead {
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
            world
                .fog_of_war
                .update_for_hero(&config, hero_pos, hero, &allied_creep_positions);
        }
        if let Some(ref mut active_input) = world.active_input {
            active_input.consume_just_pressed();
        }
        if has_slots {
            world.swap_slot_in(slot_idx);
        }
    }

    for idx in 0..world.targets.len() {
        let t_pos = world.targets[idx].base_pos;
        if world.players.is_empty() {
            world.targets_visible[idx] =
                world.fog_of_war.is_world_point_visible(&config, t_pos.x, t_pos.z);
        } else {
            world.targets_visible[idx] = true;
        }
    }

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

    if prediction.match_ended || prediction.is_paused || prediction.is_dead || prediction.traversal_active {
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
    let hero_cfg = config.active_hero(0);
    let hero_move_mult = hero_cfg.move_speed_multiplier;
    let shallow_mult = hero_cfg.shallow_water_speed_multiplier;
    let deep_mult = hero_cfg.deep_water_speed_multiplier;
    let speed_mult = if let Some((water_y, _)) = water_sample {
        let depth = (water_y - feet_pos.y).max(0.0);
        if depth >= 0.85 {
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

fn step_lane_creeps_system(
    commands: &mut Commands,
    dt: f32,
    config: &GameConfig,
    physics_world: &CustomPhysicsWorld,
    world: &mut AuthoritativeWorldState,
    target_query: &Query<&PhysicsTarget>,
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

                let count = config.creeps.creeps_per_wave;
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
                    let buffed_hp = config.creeps.max_hp * hp_mult;
                    world.creeps.push(LaneCreepState {
                        id,
                        team,
                        creep_type_id: None,
                        camp_index: None,
                        is_aggroed: true,
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
    let hero_snap: Vec<(TeamId, Vec3, bool)> = world
        .players
        .iter()
        .map(|p| (p.hero_team, p.soldier_pos, p.is_dead))
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

        let my_id = world.creeps[idx].id;
        let my_team = world.creeps[idx].team;
        let my_pos = world.creeps[idx].pos;
        let my_home = world.creeps[idx].home_pos;
        let my_lane = world.creeps[idx].lane;
        let c_stats = resolve_creep_stats(config, world.creeps[idx].creep_type_id.as_deref());
        let is_neutral = my_team == TeamId::Neutral;
        let leashed_back = is_neutral && my_pos.distance(my_home) > config.neutral_camps.aggro_leash_radius;
        if leashed_back {
            world.creeps[idx].is_aggroed = false;
        }
        let can_attack_now = !is_neutral || (world.creeps[idx].is_aggroed && !leashed_back);

        // الف) یافتن نزدیک‌ترین دشمن (سرباز یا هیرو) با رعایت کامل مه جنگ، برتری ارتفاع و خط دید موانع
        let mut best_target: Option<(Vec3, f32)> = None;
        if can_attack_now {
            for &(other_id, other_team, other_pos, other_dead, other_aggroed) in &creep_snap {
                if other_id == my_id || other_team == my_team || other_dead {
                    continue;
                }
                // سرباز لاین به کمپ حمله نمی‌کند مگر اینکه کمپ در حالت تهاجمی (Aggro) باشد
                if !is_neutral && other_team == TeamId::Neutral && !other_aggroed {
                    continue;
                }
                let d = my_pos.distance(other_pos);
                if d <= c_stats.vision_range
                    && best_target.map_or(true, |(_, bd)| d < bd)
                    && can_observer_see_point_with_range(
                        physics_world,
                        config,
                        my_pos,
                        other_pos,
                        c_stats.aim_chest_height,
                        c_stats.vision_range,
                        0.0,
                    )
                {
                    best_target = Some((other_pos + Vec3::Y * c_stats.aim_chest_height, d));
                }
            }
            for &(h_team, h_pos, h_dead) in &hero_snap {
                if h_team == my_team || h_dead {
                    continue;
                }
                let d = my_pos.distance(h_pos);
                if d <= c_stats.vision_range
                    && best_target.map_or(true, |(_, bd)| d < bd)
                    && can_observer_see_point_with_range(
                        physics_world,
                        config,
                        my_pos,
                        h_pos,
                        c_stats.aim_chest_height,
                        c_stats.vision_range,
                        0.0,
                    )
                {
                    best_target = Some((h_pos + Vec3::Y * c_stats.aim_chest_height, d));
                }
            }
            if !is_neutral {
                for &(t_team, t_pos, t_rad, t_height, t_dead) in &turret_snap {
                    if t_team == my_team || t_dead {
                        continue;
                    }
                    let edge_dist = (my_pos.distance(t_pos) - t_rad).max(0.0);
                    if edge_dist <= c_stats.vision_range
                        && best_target.map_or(true, |(_, bd)| edge_dist < bd)
                    {
                        let to_tur = Vec3::new(my_pos.x - t_pos.x, 0.0, my_pos.z - t_pos.z).normalize_or_zero();
                        let tur_edge_pos = Vec3::new(t_pos.x, my_pos.y, t_pos.z) + to_tur * (t_rad + 0.45);
                        if can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            tur_edge_pos,
                            c_stats.aim_chest_height,
                            c_stats.vision_range,
                            0.0,
                        ) {
                            best_target = Some((Vec3::new(t_pos.x, t_pos.y + t_height * 0.5, t_pos.z), edge_dist));
                        }
                    }
                }
            }
        }
            if !is_neutral {
                for &(b_team, b_pos, b_rad, b_height, b_dead, b_invuln) in &barracks_snap {
                    if b_team == my_team || b_dead || b_invuln {
                        continue;
                    }
                    let edge_dist = (my_pos.distance(b_pos) - b_rad).max(0.0);
                    if edge_dist <= c_stats.vision_range
                        && best_target.map_or(true, |(_, bd)| edge_dist < bd)
                    {
                        let to_bar = Vec3::new(my_pos.x - b_pos.x, 0.0, my_pos.z - b_pos.z).normalize_or_zero();
                        let bar_edge_pos = Vec3::new(b_pos.x, my_pos.y, b_pos.z) + to_bar * (b_rad * 1.45 + 0.6);
                        if can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            bar_edge_pos,
                            c_stats.aim_chest_height,
                            c_stats.vision_range,
                            0.0,
                        ) {
                            best_target = Some((Vec3::new(b_pos.x, b_pos.y + b_height * 0.5, b_pos.z), edge_dist));
                        }
                    }
                }
            }
            if !is_neutral {
                for &(a_team, a_pos, a_rad, _a_height, a_dead, a_invuln) in &ancient_snap {
                    if a_team == my_team || a_dead || a_invuln {
                        continue;
                    }
                    let edge_dist = (my_pos.distance(a_pos) - a_rad).max(0.0);
                    if edge_dist <= c_stats.vision_range
                        && best_target.map_or(true, |(_, bd)| edge_dist < bd)
                    {
                        let to_anc = Vec3::new(my_pos.x - a_pos.x, 0.0, my_pos.z - a_pos.z).normalize_or_zero();
                        let anc_edge_pos = Vec3::new(a_pos.x, my_pos.y, a_pos.z) + to_anc * (a_rad * 1.45 + 0.6);
                        if can_observer_see_point_with_range(
                            physics_world,
                            config,
                            my_pos,
                            anc_edge_pos,
                            c_stats.aim_chest_height,
                            c_stats.vision_range,
                            0.0,
                        ) {
                            best_target = Some((Vec3::new(a_pos.x, a_pos.y + 1.5, a_pos.z), edge_dist));
                        }
                    }
                }
            }
        if is_neutral && best_target.is_none() && my_pos.distance(my_home) <= 0.5 {
            world.creeps[idx].is_aggroed = false;
        }

        let mut moved = false;
        let mut fired_or_aiming = false;

        if let Some((target_pos, dist)) = best_target {
            let to_target = Vec3::new(target_pos.x - my_pos.x, 0.0, target_pos.z - my_pos.z);
            if to_target.length_squared() > 0.0001 {
                world.creeps[idx].facing_dir = to_target.normalize();
            }

            if dist <= c_stats.attack_range {
                fired_or_aiming = true;
                if world.creeps[idx].fire_cooldown <= 0.0 {
                    let (_, _, _, dmg_mult, fire_rate_mult) = lane_creep_buffs(
                        config,
                        &world.barracks,
                        my_team,
                        my_lane,
                        !is_neutral,
                    );
                    let eff_fire_rate = c_stats.fire_rate_secs * fire_rate_mult;
                    world.creeps[idx].fire_cooldown = eff_fire_rate;
                    world.creeps[idx].fire_anim_timer =
                        (eff_fire_rate * 0.45).min(0.35);

                    let muzzle_pos = my_pos
                        + Vec3::Y * c_stats.muzzle_offset_y
                        + world.creeps[idx].facing_dir * c_stats.muzzle_forward;
                    let aim_target = target_pos;
                    let shot_dir = (aim_target - muzzle_pos).normalize_or_zero();
                    let b_id = world.next_entity_id;
                    world.next_entity_id += 1;

                    commands.spawn((
                        PhysicsBullet {
                            id: b_id,
                            owner_slot: None,
                            owner_team: my_team,
                            velocity: shot_dir * c_stats.projectile_speed,
                            damage: c_stats.bullet_damage * dmg_mult,
                            remaining_range: c_stats.bullet_max_range,
                        },
                        Transform::from_translation(muzzle_pos),
                    ));
                    world.pending_fx.push(GameplayFxEvent::ShotFired {
                        muzzle_pos,
                        shot_dir,
                        trauma_add: 0.0,
                        trauma_max: config.combat.shot_fire_trauma_max,
                    });
                }
            } else {
                let step = world.creeps[idx].facing_dir * c_stats.move_speed * dt;
                world.creeps[idx].pos.x += step.x;
                world.creeps[idx].pos.z += step.z;
                moved = true;
            }
        } else if is_neutral {
            // سرباز کمپ اگر دشمنی نباشد به جایگاه کمپ خودش برمی‌گردد و منتظر می‌ماند
            let to_home = Vec3::new(my_home.x - my_pos.x, 0.0, my_home.z - my_pos.z);
            if to_home.length() > 0.35 {
                let dir = to_home.normalize();
                world.creeps[idx].facing_dir = dir;
                let step = dir * c_stats.move_speed * dt;
                world.creeps[idx].pos.x += step.x;
                world.creeps[idx].pos.z += step.z;
                moved = true;
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
                let wp_idx = world.creeps[idx]
                    .waypoint_index
                    .min(lane_def.waypoints.len().saturating_sub(1));
                let wp = lane_def.waypoints[wp_idx];
                let wp_pos = Vec3::new(wp.0, wp.1, wp.2);
                let to_wp = Vec3::new(wp_pos.x - my_pos.x, 0.0, wp_pos.z - my_pos.z);
                let dist_wp = to_wp.length();

                // اگر Waypoint دقیقاً روی مرکز یک تاور باشد، شعاع رسیدن به Waypoint از شعاع تاور بزرگ‌تر می‌شود تا سرباز پشت تاور گیر نکند
                let mut reach_dist = config.creeps.waypoint_reach_dist;
                for &(_t_team, t_pos, t_rad, _t_h, t_dead) in &turret_snap {
                    if !t_dead && Vec2::new(wp_pos.x - t_pos.x, wp_pos.z - t_pos.z).length() <= t_rad + 2.0 {
                        reach_dist = reach_dist.max(t_rad + c_stats.collider_radius + 1.5);
                    }
                }

                if dist_wp <= reach_dist {
                    match my_team {
                        TeamId::Dawn => {
                            if wp_idx + 1 < lane_def.waypoints.len() {
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
                    world.creeps[idx].facing_dir = dir;
                    let step = dir * c_stats.move_speed * dt;
                    world.creeps[idx].pos.x += step.x;
                    world.creeps[idx].pos.z += step.z;
                    moved = true;
                }
            }
        }

        // ج) هم‌ترازی ارتفاع سرباز با سطح واقعی زمین/رمپ/پلتفرم
        let ray_origin = Vec3::new(
            world.creeps[idx].pos.x,
            world.creeps[idx].pos.y + 3.0,
            world.creeps[idx].pos.z,
        );
        if let Some((_ent, hit)) = physics_world.cast_ray(
            ray_origin,
            Vec3::NEG_Y,
            12.0,
            query_groups,
            |ent| target_query.get(ent).is_err(),
        ) {
            if hit.normal.y > 0.5 {
                world.creeps[idx].pos.y = hit.point.y;
            }
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
                world.creeps[idx].pos.x = t_pos.x + push.x * min_d;
                world.creeps[idx].pos.z = t_pos.z + push.y * min_d;
                if moved {
                    let f = Vec2::new(world.creeps[idx].facing_dir.x, world.creeps[idx].facing_dir.z);
                    let t1 = Vec2::new(-push.y, push.x);
                    let tangent = if t1.dot(f) >= 0.0 { t1 } else { -t1 };
                    let slide = tangent * c_stats.move_speed * dt * 0.85;
                    world.creeps[idx].pos.x += slide.x;
                    world.creeps[idx].pos.z += slide.y;
                }
            }
        }

        world.creeps[idx].anim_state = if fired_or_aiming {
            if world.creeps[idx].fire_anim_timer > 0.0 {
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
                let half_push = (min_creep_dist - dist) * 0.5;
                world.creeps[i].pos.x -= push_dir.x * half_push;
                world.creeps[i].pos.z -= push_dir.y * half_push;
                world.creeps[j].pos.x += push_dir.x * half_push;
                world.creeps[j].pos.z += push_dir.y * half_push;
            }
        }
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

    let hero_snap: Vec<(TeamId, Vec3, bool)> = world
        .players
        .iter()
        .map(|p| (p.hero_team, p.soldier_pos, p.is_dead))
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

        // ۱. اولویت تاورها (Tier 1 -> Tier 2 -> Tier 3): اگر تاور جلویی هنوز زنده است، این تاور ضدضربه است
        let prev_tier_alive = turret_status.iter().any(|&(tm, ln, tr, dead)| {
            tm == my_team && ln == my_lane && tr < my_tier && !dead
        });

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

        // ۳. انتخاب هدف تاور: اولویت با نزدیک‌ترین سرباز دشمن در برد، سپس هیروی دشمن
        let mut best_creep: Option<(Vec3, f32)> = None;
        for &(c_team, c_pos, c_dead, is_lane_creep) in &creep_snap {
            if c_dead || !is_lane_creep || c_team == my_team || c_team == TeamId::Neutral {
                continue;
            }
            let d = my_pos.distance(c_pos);
            let to_c = Vec3::new(c_pos.x - my_pos.x, 0.0, c_pos.z - my_pos.z).normalize_or_zero();
            let ray_start = Vec3::new(my_pos.x, c_pos.y, my_pos.z) + to_c * (t_world_radius + 0.45);
            if d <= t_def.attack_range
                && best_creep.map_or(true, |(_, bd)| d < bd)
                && can_observer_see_point_with_range(
                    physics_world,
                    config,
                    ray_start,
                    c_pos,
                    1.15,
                    t_def.vision_range,
                    0.0,
                )
            {
                best_creep = Some((c_pos + Vec3::Y * config.creeps.aim_chest_height, d));
            }
        }

        let mut best_hero: Option<(Vec3, f32)> = None;
        if best_creep.is_none() {
            for &(h_team, h_pos, h_dead) in &hero_snap {
                if h_team == my_team || h_dead {
                    continue;
                }
                let d = my_pos.distance(h_pos);
                let to_h = Vec3::new(h_pos.x - my_pos.x, 0.0, h_pos.z - my_pos.z).normalize_or_zero();
                let ray_start = Vec3::new(my_pos.x, h_pos.y, my_pos.z) + to_h * (t_world_radius + 0.45);
                if d <= t_def.attack_range
                    && best_hero.map_or(true, |(_, bd)| d < bd)
                    && can_observer_see_point_with_range(
                        physics_world,
                        config,
                        ray_start,
                        h_pos,
                        1.15,
                        t_def.vision_range,
                        0.0,
                    )
                {
                    best_hero = Some((h_pos + Vec3::Y * config.combat.hero_stand_aim_chest_height, d));
                }
            }
        }

        let chosen_target = best_creep.or(best_hero).map(|(p, _)| p);
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
                        owner_team: my_team,
                        velocity: shot_dir * t_def.projectile_speed,
                        damage: t_def.bullet_damage,
                        remaining_range: t_def.bullet_max_range,
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

        world.ancients[idx].is_invulnerable = !any_t3_dead || !has_enemy_lane_creeps;
    }
}