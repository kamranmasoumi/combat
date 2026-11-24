use crate::shop_shared::{
    recompute_player_slot_stats, try_activate_item_ability, InventoryItemState, ItemsConfig,
};
use crate::{
    grant_xp_to_player_slot, ActiveWeaponSlot, AuthoritativeMatchState, AuthoritativeWorldState,
    ClientInputCommand, GameConfig, LobbyPlayerInfo, Stance, TeamId, WeaponKind, WeatherKind,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// =============================================================================
// مدل‌های داده و اجرای دستورات Demo Hero (چند-هیرویی با کنترل کامل، گلد بی‌نهایت و حفظ دستورات)
// =============================================================================

pub const DEMO_INFINITE_GOLD: u32 = 99_999;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DummyBehavior {
    #[default]
    Idle,
    AttackPlayer,
    SwimInWater,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DemoTeleportPreset {
    MidLane,
    CanyonWater,
    RoshanPit,
    EnemyTower,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DemoCommand {
    LevelUp { levels: u32 },
    MaxLevel,
    AddGold { amount: u32 },
    RefreshHero,
    RefreshCooldownsOnly,
    RefreshHealthOnly,
    SetFreeCooldowns(bool),
    /// تغییر هیروی اسلاتی که الان در کنترل بازیکن است
    SwitchOwnHero { hero_id: String },
    /// اسپاون یک هیروی جدید در تیم دلخواه (Dawn یا Dusk) تا سقف ۱۰ هیرو (۵v۵)
    SpawnHero { team: TeamId, hero_id: String },
    /// اسپاون یک هیروی Dummy دشمن درست روبه‌روی هیروی تحت کنترل
    SpawnDummy { hero_id: String },
    /// انتقال کامل کنترل بازیکن (حرکت، تیراندازی، شاپ، دوربین و HUD) به اسلات مشخص‌شده
    ControlSlot { slot_index: usize },
    /// جابه‌جایی چرخشی کنترل به هیروی بعدی در زمین
    CycleControlSlot,
    /// حذف هیروی تحت کنترل (در صورتی که اسلات ۰ نباشد)
    RemoveControlledHero,
    /// حذف هیروهای اضافی و بازگشت به حالت پایه ۱v۱
    RemoveExtraHeroes,
    /// بازنشانی کامل هیروی تحت کنترل (لول ۱، بدون آیتم، جان و مهمات کامل)
    ResetControlledHero,
    /// فعال/غیرفعال‌سازی آسیب‌ناپذیری (Invulnerable) برای هیروی تحت کنترل
    ToggleInvulnerable,
    /// جابه‌جایی تیم هیروی تحت کنترل (Dawn <-> Dusk)
    SwapControlledTeam,
    /// ثبت یا لغو دستور حرکت پایدار برای هر اسلات (با تعویض کنترل لغو نمی‌شود)
    SetSlotMoveTarget { slot_index: usize, target: Option<Vec3> },
    /// فعال/غیرفعال‌سازی اسپاون سربازان لاین
    SetCreepsEnabled(bool),
    /// فعال/غیرفعال‌سازی تاورها
    SetTowersEnabled(bool),
    /// توقف موقت / ادامه شبیه‌سازی دمو
    SetPaused(bool),
    /// احضار همه هیروهای غیرکنترلی کنار هیروی فعلی
    RespawnDummiesNearPlayer,
    SetDummyBehavior(DummyBehavior),
    GiveOtherHeroesItem { item_id: String },
    ClearOtherHeroesItems,
    ForceOtherHeroesActivateItem { inv_index: usize },
    SetWeather(WeatherKind),
    SetNightTime(bool),
    SetRevealFullMap(bool),
    SpawnCreepWave,
    ClearCreeps,
    RespawnAllTurrets,
    TeleportSelf(DemoTeleportPreset),
}

#[derive(Clone, Debug)]
pub struct DemoSharedFlags {
    pub free_cooldowns: bool,
    pub dummy_behavior: DummyBehavior,
    pub controlled_slot: usize,
    pub paused: bool,
    pub creeps_enabled: bool,
    pub towers_enabled: bool,
    pub slot_invulnerable: HashSet<usize>,
    pub slot_move_targets: HashMap<usize, Vec3>,
}

impl Default for DemoSharedFlags {
    fn default() -> Self {
        Self {
            free_cooldowns: false,
            dummy_behavior: DummyBehavior::Idle,
            controlled_slot: 0,
            paused: false,
            creeps_enabled: true,
            towers_enabled: true,
            slot_invulnerable: HashSet::new(),
            slot_move_targets: HashMap::new(),
        }
    }
}

pub fn find_canyon_water_point(config: &GameConfig) -> Vec3 {
    let c = &config.map.canyon;
    Vec3::new(
        (c.start_corner.0 + c.end_corner.0) * 0.5,
        c.water_y,
        (c.start_corner.1 + c.end_corner.1) * 0.5,
    )
}

pub fn step_demo_dummy_ai_and_rules(
    config: &GameConfig,
    world: &mut AuthoritativeWorldState,
    flags: &mut DemoSharedFlags,
) {
    // گلد بی‌نهایت برای تمام هیروها در حالت Demo Hero
    for p in &mut world.players {
        if p.hero_gold < DEMO_INFINITE_GOLD {
            p.hero_gold = DEMO_INFINITE_GOLD;
        }
    }

    // اعمال آسیب‌ناپذیری (INVULN) روی اسلات‌های انتخاب‌شده
    for &inv_slot in &flags.slot_invulnerable {
        if let Some(p) = world.players.get_mut(inv_slot) {
            p.is_dead = false;
            p.respawn_remaining = 0.0;
            p.hero_hp = p.hero_max_hp;
        }
    }

    // اگر سربازان غیرفعال باشند، زمان موج بعدی را عقب بینداز
    if !flags.creeps_enabled {
        world.next_wave_time_secs = world.match_elapsed_secs + 9999.0;
    }

    let ctrl = flags.controlled_slot.min(world.players.len().saturating_sub(1));

    if flags.free_cooldowns {
        if let Some(pc) = world.players.get_mut(ctrl) {
            pc.teleport_cooldown_remaining = 0.0;
            pc.shadow_blade_cooldown_remaining = 0.0;
            pc.reflect_armor_cooldown_remaining = 0.0;
            pc.bullet_immunity_cooldown_remaining = 0.0;
            pc.null_pulse_cooldown_remaining = 0.0;
            pc.sonar_reveal_cooldown_remaining = 0.0;
            pc.kinetic_dash_cooldown_remaining = 0.0;
            pc.grenade_charges = pc.hero_grenade.max_charges;
            pc.grenade_recharge_remaining = 0.0;
            pc.hero_stamina = pc.hero_max_stamina;
        }
    }

    if world.players.len() < 2 {
        return;
    }

    let ctrl_pos = world.players[ctrl].soldier_pos;
    let ctrl_dead = world.players[ctrl].is_dead;
    let ctrl_team = world.players[ctrl].hero_team;
    let water_target = find_canyon_water_point(config);

    let mut arrived_slots: Vec<usize> = Vec::new();
    let total_players = world.players.len();
    for slot_idx in 0..total_players {
        if slot_idx == ctrl {
            continue;
        }
        if world.players[slot_idx].is_dead {
            world.players[slot_idx].respawn_remaining =
                world.players[slot_idx].respawn_remaining.min(1.5);
            continue;
        }

        let d_pos = world.players[slot_idx].soldier_pos;
        let d_team = world.players[slot_idx].hero_team;
        let max_r = world.players[slot_idx].hero_weapon.max_range;

        let mut cmd = ClientInputCommand {
            move_input: Vec3::ZERO,
            shift_held: false,
            toggle_crouch_just_pressed: false,
            reload_just_pressed: false,
            grenade_just_pressed: false,
            space_just_pressed: false,
            space_held: false,
            vertical_aim_held: false,
            switch_weapon_just_pressed: false,
            fire_just_pressed: false,
            fire_held: false,
            aim_ray_origin: d_pos + Vec3::Y * 1.2,
            aim_ray_dir: Vec3::NEG_X,
            has_aim_ray: false,
            client_muzzle_world_pos: None,
        };

        // اولویت ۱: اگر بازیکن قبلاً به این هیرو دستور حرکت (کلیک روی زمین/مینی‌مپ) داده، تا رسیدن به مقصد ادامه دهد
        let mut executing_player_order = false;
        if let Some(&move_target) = flags.slot_move_targets.get(&slot_idx) {
            let flat = Vec3::new(move_target.x - d_pos.x, 0.0, move_target.z - d_pos.z);
            if flat.length() <= 0.55 {
                arrived_slots.push(slot_idx);
            } else {
                let dir = flat.normalize_or_zero();
                cmd.move_input = dir;
                cmd.aim_ray_origin = d_pos + Vec3::Y * 1.2;
                cmd.aim_ray_dir = dir;
                cmd.has_aim_ray = true;
                executing_player_order = true;
            }
        }

        if !executing_player_order {
            match flags.dummy_behavior {
                DummyBehavior::Idle => {}
                DummyBehavior::AttackPlayer => {
                    // فقط هیروهای تیم دشمن به هیروی تحت کنترل حمله می‌کنند
                    if !ctrl_dead && d_team != ctrl_team {
                        let target_pt = ctrl_pos + Vec3::Y * 0.9;
                        let ray_origin = d_pos + Vec3::Y * 1.2;
                        let to_ctrl = target_pt - ray_origin;
                        let dist = to_ctrl.length();
                        if dist > 0.05 {
                            cmd.aim_ray_origin = ray_origin;
                            cmd.aim_ray_dir = to_ctrl / dist;
                            cmd.has_aim_ray = true;
                        }
                        if dist <= max_r * 0.95 {
                            cmd.fire_held = true;
                            cmd.fire_just_pressed = world.tick % 6 == 0;
                        } else {
                            let flat = Vec3::new(ctrl_pos.x - d_pos.x, 0.0, ctrl_pos.z - d_pos.z)
                                .normalize_or_zero();
                            cmd.move_input = flat;
                        }
                    }
                }
                DummyBehavior::SwimInWater => {
                    if d_team != ctrl_team {
                        let flat =
                            Vec3::new(water_target.x - d_pos.x, 0.0, water_target.z - d_pos.z);
                        if flat.length() > 1.8 {
                            let dir = flat.normalize_or_zero();
                            cmd.move_input = dir;
                            cmd.aim_ray_origin = d_pos + Vec3::Y * 1.2;
                            cmd.aim_ray_dir = dir;
                            cmd.has_aim_ray = true;
                        }
                    }
                }
            }
        }

        world.set_slot_input(slot_idx, cmd);
    }

    for s in arrived_slots {
        flags.slot_move_targets.remove(&s);
    }
}

/// تعویض کامل و بی‌باگ هیرو روی یک اسلات (بازنشانی کامل سلاح، انیمیشن، ریلود، اشعه و شناسه هیرو)
pub fn switch_slot_hero(
    world: &mut AuthoritativeWorldState,
    match_state: &mut AuthoritativeMatchState,
    slot_idx: usize,
    hero_id: &str,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) {
    let Some(new_idx) = config.heroes.iter().position(|h| h.id == hero_id) else {
        return;
    };
    let hero = config.active_hero(new_idx).clone();
    if let Some(lp) = match_state.players.get_mut(slot_idx) {
        lp.selected_hero_id = Some(hero.id.clone());
        match_state.dirty = true;
    }
    let Some(p) = world.players.get_mut(slot_idx) else {
        return;
    };
    p.hero_index = new_idx;
    p.hero_id = hero.id.clone();
    p.hero_display_name = hero.display_name.clone();
    p.hero_model_path = hero.model_path.clone();
    p.hero_scale = hero.hero_scale;
    p.hero_weapon = hero.weapon.clone();
    p.hero_pistol = hero.pistol.clone();
    p.hero_grenade = hero.grenade;
    p.active_weapon = ActiveWeaponSlot::Primary;
    // بازنشانی کامل وضعیت شلیک/ریلود/اشعه/پارکور برای جلوگیری از گیر کردن یا لگ هنگام تعویض هیرو
    p.fire_cooldown_remaining = 0.0;
    p.firing_anim_remaining = None;
    p.reload_remaining = None;
    p.active_beam_request = None;
    p.grenade_elapsed_and_duration = None;
    p.grenade_spawned_for_throw = false;
    p.traversal.is_active = false;
    p.wall_climb_mountain_index = None;
    p.base_stance = Stance::Run;
    p.effective_stance = Stance::Run;

    recompute_player_slot_stats(p, config, items_cfg);
    p.hero_hp = p.hero_max_hp;
    p.hero_stamina = p.hero_max_stamina;
    p.beam_charge = p.hero_weapon.beam_max_charge;
    p.ammo_in_mag = if p.hero_weapon.weapon_kind == WeaponKind::ElectricBeam
        || p.hero_weapon.weapon_kind == WeaponKind::FlameBeam
    {
        p.beam_charge.ceil() as u32
    } else {
        p.hero_weapon.magazine_size
    };
    p.other_weapon_ammo_in_mag = p.hero_pistol.magazine_size;
    p.grenade_charges = p.hero_grenade.max_charges;
}

pub fn execute_demo_command(
    cmd: DemoCommand,
    world: &mut AuthoritativeWorldState,
    match_state: &mut AuthoritativeMatchState,
    flags: &mut DemoSharedFlags,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
    commands: &mut Commands,
) {
    let ctrl = flags.controlled_slot.min(world.players.len().saturating_sub(1));

    match cmd {
        DemoCommand::LevelUp { levels } => {
            if let Some(pc) = world.players.get_mut(ctrl) {
                let max_lv = config.combat.xp_thresholds_per_level.len().min(25) as u32;
                let target = (pc.hero_level + levels).min(max_lv);
                while pc.hero_level < target {
                    let idx = (pc.hero_level as usize)
                        .min(config.combat.xp_thresholds_per_level.len() - 1);
                    let threshold = config.combat.xp_thresholds_per_level[idx];
                    let xp_needed = threshold.saturating_sub(pc.hero_xp).max(1);
                    grant_xp_to_player_slot(
                        pc,
                        xp_needed,
                        config,
                        items_cfg,
                        &mut world.pending_private_fx,
                    );
                }
            }
        }
        DemoCommand::MaxLevel => {
            if let Some(pc) = world.players.get_mut(ctrl) {
                let max_lv = config.combat.xp_thresholds_per_level.len().min(25) as u32;
                while pc.hero_level < max_lv {
                    let idx = (pc.hero_level as usize)
                        .min(config.combat.xp_thresholds_per_level.len() - 1);
                    let threshold = config.combat.xp_thresholds_per_level[idx];
                    let xp_needed = threshold.saturating_sub(pc.hero_xp).max(1);
                    grant_xp_to_player_slot(
                        pc,
                        xp_needed,
                        config,
                        items_cfg,
                        &mut world.pending_private_fx,
                    );
                }
            }
        }
        DemoCommand::AddGold { amount } => {
            if let Some(pc) = world.players.get_mut(ctrl) {
                pc.hero_gold = pc.hero_gold.saturating_add(amount).max(DEMO_INFINITE_GOLD);
            }
        }
        DemoCommand::RefreshHero => {
            if let Some(pc) = world.players.get_mut(ctrl) {
                pc.is_dead = false;
                pc.respawn_remaining = 0.0;
                pc.hero_hp = pc.hero_max_hp;
                pc.hero_stamina = pc.hero_max_stamina;
                pc.teleport_cooldown_remaining = 0.0;
                pc.shadow_blade_cooldown_remaining = 0.0;
                pc.reflect_armor_cooldown_remaining = 0.0;
                pc.bullet_immunity_cooldown_remaining = 0.0;
                pc.null_pulse_cooldown_remaining = 0.0;
                pc.sonar_reveal_cooldown_remaining = 0.0;
                pc.kinetic_dash_cooldown_remaining = 0.0;
                pc.cryo_debuff_remaining = 0.0;
                pc.cryo_slow_ratio = 0.0;
                pc.cryo_heal_reduction_ratio = 0.0;
                pc.null_mute_remaining = 0.0;
                pc.flame_armor_debuff_remaining = 0.0;
                pc.electric_fire_rate_slow_remaining = 0.0;
                pc.grenade_charges = pc.hero_grenade.max_charges;
                pc.grenade_recharge_remaining = 0.0;
                pc.beam_charge = pc.hero_weapon.beam_max_charge;
                pc.ammo_in_mag = pc.hero_weapon.magazine_size;
            }
        }
        DemoCommand::RefreshCooldownsOnly => {
            for pc in &mut world.players {
                pc.teleport_cooldown_remaining = 0.0;
                pc.shadow_blade_cooldown_remaining = 0.0;
                pc.reflect_armor_cooldown_remaining = 0.0;
                pc.bullet_immunity_cooldown_remaining = 0.0;
                pc.null_pulse_cooldown_remaining = 0.0;
                pc.sonar_reveal_cooldown_remaining = 0.0;
                pc.kinetic_dash_cooldown_remaining = 0.0;
                pc.grenade_charges = pc.hero_grenade.max_charges;
                pc.grenade_recharge_remaining = 0.0;
                pc.beam_charge = pc.hero_weapon.beam_max_charge;
                pc.ammo_in_mag = pc.hero_weapon.magazine_size;
            }
        }
        DemoCommand::RefreshHealthOnly => {
            for pc in &mut world.players {
                pc.is_dead = false;
                pc.respawn_remaining = 0.0;
                pc.hero_hp = pc.hero_max_hp;
                pc.hero_stamina = pc.hero_max_stamina;
                pc.cryo_debuff_remaining = 0.0;
                pc.cryo_slow_ratio = 0.0;
                pc.cryo_heal_reduction_ratio = 0.0;
                pc.null_mute_remaining = 0.0;
                pc.flame_armor_debuff_remaining = 0.0;
                pc.electric_fire_rate_slow_remaining = 0.0;
            }
        }
        DemoCommand::SetFreeCooldowns(enabled) => {
            flags.free_cooldowns = enabled;
        }
        DemoCommand::SwitchOwnHero { hero_id } => {
            switch_slot_hero(world, match_state, ctrl, &hero_id, config, items_cfg);
        }
        DemoCommand::SpawnHero { team, hero_id } => {
            if world.players.len() >= 10 {
                return;
            }
            let valid_hero_id = if config.heroes.iter().any(|h| h.id == hero_id) {
                hero_id
            } else {
                config.heroes[0].id.clone()
            };
            let new_slot = world.players.len();
            let base_pos = world
                .players
                .get(ctrl)
                .map(|p| p.soldier_pos)
                .unwrap_or(Vec3::new(0.0, 0.2, 0.0));
            let angle = (new_slot as f32) * 1.15;
            let dist = if team == world.players[ctrl].hero_team {
                5.5
            } else {
                10.0
            };
            let spawn_pos = Vec3::new(
                base_pos.x + angle.cos() * dist,
                base_pos.y,
                base_pos.z + angle.sin() * dist,
            );
            let lp = LobbyPlayerInfo {
                slot_index: new_slot,
                client_id: None,
                user_id: Some(format!("demo_hero_{}", new_slot)),
                team,
                connected: true,
                selected_hero_id: Some(valid_hero_id),
                map_loaded: true,
            };
            match_state.players.push(lp.clone());
            match_state.required_players = match_state.players.len();
            match_state.dirty = true;
            world.spawn_single_player_slot(config, &lp, spawn_pos, commands);
            if let Some(new_p) = world.players.get_mut(new_slot) {
                new_p.hero_gold = DEMO_INFINITE_GOLD;
                new_p.is_disconnected = false;
            }
        }
        DemoCommand::SpawnDummy { hero_id } => {
            if world.players.len() >= 10 {
                return;
            }
            let valid_hero_id = if config.heroes.iter().any(|h| h.id == hero_id) {
                hero_id
            } else {
                config.heroes[0].id.clone()
            };
            let new_slot = world.players.len();
            let base_pos = world
                .players
                .get(ctrl)
                .map(|p| p.soldier_pos)
                .unwrap_or(Vec3::new(0.0, 0.2, 0.0));
            let fwd = world
                .players
                .get(ctrl)
                .map(|p| Vec3::new(p.aim_world_dir.x, 0.0, p.aim_world_dir.z).normalize_or_zero())
                .unwrap_or(Vec3::X);
            let dir = if fwd.length_squared() > 0.01 {
                fwd
            } else {
                Vec3::X
            };
            let spawn_pos = base_pos + dir * 8.5;
            let enemy_team = if world.players[ctrl].hero_team == TeamId::Dawn {
                TeamId::Dusk
            } else {
                TeamId::Dawn
            };
            let lp = LobbyPlayerInfo {
                slot_index: new_slot,
                client_id: None,
                user_id: Some(format!("demo_dummy_{}", new_slot)),
                team: enemy_team,
                connected: true,
                selected_hero_id: Some(valid_hero_id),
                map_loaded: true,
            };
            match_state.players.push(lp.clone());
            match_state.required_players = match_state.players.len();
            match_state.dirty = true;
            world.spawn_single_player_slot(config, &lp, spawn_pos, commands);
            if let Some(new_p) = world.players.get_mut(new_slot) {
                new_p.hero_gold = DEMO_INFINITE_GOLD;
                new_p.is_disconnected = false;
            }
        }
        DemoCommand::ControlSlot { slot_index } => {
            if slot_index < world.players.len() {
                flags.controlled_slot = slot_index;
                match_state.dirty = true;
            }
        }
        DemoCommand::CycleControlSlot => {
            if !world.players.is_empty() {
                flags.controlled_slot = (flags.controlled_slot + 1) % world.players.len();
                match_state.dirty = true;
            }
        }
        DemoCommand::RemoveControlledHero => {
            // پاک‌سازی دقیق فیزیک در demo_client.rs انجام می‌شود؛ اینجا فقط اگر اسلات > 0 بود حذف می‌کنیم
            if ctrl > 0 && ctrl < world.players.len() {
                world.players.remove(ctrl);
                if ctrl < match_state.players.len() {
                    match_state.players.remove(ctrl);
                }
                for (idx, lp) in match_state.players.iter_mut().enumerate() {
                    lp.slot_index = idx;
                }
                match_state.required_players = match_state.players.len();
                match_state.dirty = true;

                // بازنگاشت کلیدهای slot_invulnerable و slot_move_targets بعد از حذف اسلات میانی
                let mut new_inv = HashSet::new();
                for &s in &flags.slot_invulnerable {
                    if s < ctrl {
                        new_inv.insert(s);
                    } else if s > ctrl {
                        new_inv.insert(s - 1);
                    }
                }
                flags.slot_invulnerable = new_inv;

                let mut new_targets = HashMap::new();
                for (&s, &pos) in &flags.slot_move_targets {
                    if s < ctrl {
                        new_targets.insert(s, pos);
                    } else if s > ctrl {
                        new_targets.insert(s - 1, pos);
                    }
                }
                flags.slot_move_targets = new_targets;
                flags.controlled_slot = 0;
            }
        }
        DemoCommand::RemoveExtraHeroes => {
            flags.controlled_slot = 0;
            if world.players.len() > 2 {
                world.players.truncate(2);
                match_state.players.truncate(2);
                match_state.required_players = match_state.players.len();
                match_state.dirty = true;
            }
            flags.slot_invulnerable.retain(|&s| s < 2);
            flags.slot_move_targets.retain(|&s, _| s < 2);
        }
        DemoCommand::ResetControlledHero => {
            if let Some(pc) = world.players.get_mut(ctrl) {
                pc.hero_level = 1;
                pc.hero_xp = 0;
                pc.inventory.clear();
                pc.stash.clear();
                pc.hero_gold = DEMO_INFINITE_GOLD;
                pc.is_dead = false;
                pc.respawn_remaining = 0.0;
                pc.soldier_pos = pc.spawn_pos;
                pc.traversal.is_active = false;
                pc.wall_climb_mountain_index = None;
                pc.teleport_cooldown_remaining = 0.0;
                pc.shadow_blade_cooldown_remaining = 0.0;
                pc.reflect_armor_cooldown_remaining = 0.0;
                pc.bullet_immunity_cooldown_remaining = 0.0;
                pc.null_pulse_cooldown_remaining = 0.0;
                pc.sonar_reveal_cooldown_remaining = 0.0;
                pc.kinetic_dash_cooldown_remaining = 0.0;
                pc.invisibility_remaining = 0.0;
                pc.reflect_armor_remaining = 0.0;
                pc.bullet_immunity_remaining = 0.0;
                pc.cryo_debuff_remaining = 0.0;
                pc.null_mute_remaining = 0.0;
                recompute_player_slot_stats(pc, config, items_cfg);
                pc.hero_hp = pc.hero_max_hp;
                pc.hero_stamina = pc.hero_max_stamina;
                pc.beam_charge = pc.hero_weapon.beam_max_charge;
                pc.ammo_in_mag = pc.hero_weapon.magazine_size;
            }
            flags.slot_move_targets.remove(&ctrl);
            flags.slot_invulnerable.remove(&ctrl);
        }
        DemoCommand::ToggleInvulnerable => {
            if flags.slot_invulnerable.contains(&ctrl) {
                flags.slot_invulnerable.remove(&ctrl);
            } else {
                flags.slot_invulnerable.insert(ctrl);
                if let Some(pc) = world.players.get_mut(ctrl) {
                    pc.is_dead = false;
                    pc.respawn_remaining = 0.0;
                    pc.hero_hp = pc.hero_max_hp;
                }
            }
        }
        DemoCommand::SwapControlledTeam => {
            if let Some(pc) = world.players.get_mut(ctrl) {
                let new_team = if pc.hero_team == TeamId::Dawn {
                    TeamId::Dusk
                } else {
                    TeamId::Dawn
                };
                pc.hero_team = new_team;
                if let Some(lp) = match_state.players.get_mut(ctrl) {
                    lp.team = new_team;
                }
                match_state.dirty = true;
            }
        }
        DemoCommand::SetSlotMoveTarget { slot_index, target } => {
            if let Some(pos) = target {
                flags.slot_move_targets.insert(slot_index, pos);
            } else {
                flags.slot_move_targets.remove(&slot_index);
            }
        }
        DemoCommand::SetCreepsEnabled(enabled) => {
            flags.creeps_enabled = enabled;
            if enabled {
                world.next_wave_time_secs = world.match_elapsed_secs;
            } else {
                world.creeps.clear();
                world.next_wave_time_secs = world.match_elapsed_secs + 9999.0;
            }
        }
        DemoCommand::SetTowersEnabled(enabled) => {
            flags.towers_enabled = enabled;
            for t in &mut world.turrets {
                if enabled {
                    t.hp = t.max_hp;
                    t.is_dead = false;
                    t.fire_cooldown = 0.0;
                } else {
                    t.hp = 0.0;
                    t.is_dead = true;
                }
            }
        }
        DemoCommand::SetPaused(paused) => {
            flags.paused = paused;
        }
        DemoCommand::RespawnDummiesNearPlayer => {
            if world.players.len() < 2 {
                return;
            }
            let base_pos = world.players[ctrl].soldier_pos;
            let fwd = Vec3::new(
                world.players[ctrl].aim_world_dir.x,
                0.0,
                world.players[ctrl].aim_world_dir.z,
            )
            .normalize_or_zero();
            let dir = if fwd.length_squared() > 0.01 {
                fwd
            } else {
                Vec3::X
            };
            let right = Vec3::new(-dir.z, 0.0, dir.x);
            let mut idx_offset = 0_i32;
            for i in 0..world.players.len() {
                if i == ctrl {
                    continue;
                }
                let lateral = ((idx_offset % 3) - 1) as f32 * 3.5;
                let depth = 8.5 + (idx_offset / 3) as f32 * 3.5;
                let spawn_pos = base_pos + dir * depth + right * lateral;
                let d = &mut world.players[i];
                d.is_dead = false;
                d.respawn_remaining = 0.0;
                d.soldier_pos = spawn_pos;
                d.hero_hp = d.hero_max_hp;
                d.hero_stamina = d.hero_max_stamina;
                d.invisibility_remaining = 0.0;
                d.bullet_immunity_remaining = 0.0;
                d.reflect_armor_remaining = 0.0;
                d.cryo_debuff_remaining = 0.0;
                d.null_mute_remaining = 0.0;
                d.sonar_revealed_remaining = 0.0;
                flags.slot_move_targets.remove(&i);
                idx_offset += 1;
            }
        }
        DemoCommand::SetDummyBehavior(b) => {
            flags.dummy_behavior = b;
        }
        DemoCommand::GiveOtherHeroesItem { item_id } => {
            if items_cfg.get_item(&item_id).is_some() {
                for i in 0..world.players.len() {
                    if i == ctrl {
                        continue;
                    }
                    let d = &mut world.players[i];
                    if d.inventory.len() < items_cfg.max_inventory_slots {
                        let used: Vec<usize> = d.inventory.iter().map(|it| it.slot).collect();
                        if let Some(free_slot) =
                            (0..items_cfg.max_inventory_slots).find(|s| !used.contains(s))
                        {
                            d.inventory.push(InventoryItemState {
                                item_id: item_id.clone(),
                                active_mode: 0,
                                slot: free_slot,
                                count: 1,
                            });
                            recompute_player_slot_stats(d, config, items_cfg);
                            d.hero_hp = d.hero_max_hp;
                        }
                    }
                }
            }
        }
        DemoCommand::ClearOtherHeroesItems => {
            for i in 0..world.players.len() {
                if i == ctrl {
                    continue;
                }
                let d = &mut world.players[i];
                d.inventory.clear();
                recompute_player_slot_stats(d, config, items_cfg);
            }
        }
        DemoCommand::ForceOtherHeroesActivateItem { inv_index } => {
            let total = world.players.len();
            for i in 0..total {
                if i == ctrl {
                    continue;
                }
                world.players[i].shadow_blade_cooldown_remaining = 0.0;
                world.players[i].reflect_armor_cooldown_remaining = 0.0;
                world.players[i].bullet_immunity_cooldown_remaining = 0.0;
                world.players[i].null_pulse_cooldown_remaining = 0.0;
                world.players[i].sonar_reveal_cooldown_remaining = 0.0;
                world.players[i].kinetic_dash_cooldown_remaining = 0.0;
                world.players[i].null_mute_remaining = 0.0;
                try_activate_item_ability(world, i, inv_index, config, items_cfg);
            }
        }
        DemoCommand::SetWeather(w) => {
            world.current_weather = w;
            world.weather_remaining_secs = 9999.0;
            world.env_stats_initialized = false;
            if w == WeatherKind::Rainy {
                world.lightning_cooldown_secs =
                    config.weather.lightning_min_interval_secs.min(4.0);
            }
        }
        DemoCommand::SetNightTime(want_night) => {
            let phase_dur = config.day_night.phase_duration_secs.max(1.0);
            let cycle = (world.match_elapsed_secs / phase_dur).floor() as u32;
            let is_currently_night = cycle % 2 == 1;
            if is_currently_night != want_night {
                world.match_elapsed_secs = ((cycle + 1) as f32) * phase_dur + 1.0;
                world.env_stats_initialized = false;
            }
        }
        DemoCommand::SetRevealFullMap(reveal) => {
            world.demo_reveal_full_map = reveal;
        }
        DemoCommand::SpawnCreepWave => {
            flags.creeps_enabled = true;
            world.next_wave_time_secs = world.match_elapsed_secs;
        }
        DemoCommand::ClearCreeps => {
            world.creeps.clear();
        }
        DemoCommand::RespawnAllTurrets => {
            flags.towers_enabled = true;
            for t in &mut world.turrets {
                t.hp = t.max_hp;
                t.is_dead = false;
                t.fire_cooldown = 0.0;
            }
        }
        DemoCommand::TeleportSelf(preset) => {
            let target = match preset {
                DemoTeleportPreset::MidLane => Vec3::new(-4.0, 0.2, 0.0),
                DemoTeleportPreset::CanyonWater => find_canyon_water_point(config),
                DemoTeleportPreset::RoshanPit => {
                    let p = config.map.roshan_pit.model_transform.location_vec();
                    Vec3::new(p.x, p.y + 0.5, p.z)
                }
                DemoTeleportPreset::EnemyTower => world
                    .turrets
                    .iter()
                    .find(|t| t.team == TeamId::Dusk && !t.is_dead)
                    .map(|t| t.pos + Vec3::new(-10.0, 0.2, 0.0))
                    .unwrap_or(Vec3::new(35.0, 0.2, 0.0)),
            };
            if let Some(pc) = world.players.get_mut(ctrl) {
                pc.soldier_pos = target;
                pc.traversal.is_active = false;
                pc.wall_climb_mountain_index = None;
            }
            flags.slot_move_targets.remove(&ctrl);
        }
    }
}