use crate::WeatherKind;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

use crate::{
    fountain_regen_bonus_for_pos, sample_camera_focus_y, soldier_collision_groups,
    ActiveTeleportState, ActiveWeaponSlot, AncientSnapshot, AncientState, AuthoritativeWorldState,
    BarracksSnapshot, BarracksState, CourierOrder, CustomPhysicsWorld, GameConfig, GameplayFxEvent,
    PhysicsSoldier, PlayerSlotState, TeamId, TurretSnapshot, TurretState,
};

pub const RECIPE_ID_PREFIX: &str = "recipe:";

pub fn recipe_item_id(target_item_id: &str) -> String {
    format!("{}{}", RECIPE_ID_PREFIX, target_item_id)
}

pub fn parse_recipe_target(item_id: &str) -> Option<&str> {
    item_id.strip_prefix(RECIPE_ID_PREFIX)
}

pub fn is_recipe_item_id(item_id: &str) -> bool {
    item_id.starts_with(RECIPE_ID_PREFIX)
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemCategory {
    PrimaryWeapon,
    Pistol,
    Mobility,
    Defense,
    Upgrades,
}

impl ItemCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PrimaryWeapon => "Primary",
            Self::Pistol => "Pistol",
            Self::Mobility => "Mobility",
            Self::Defense => "Defense",
            Self::Upgrades => "Upgrades",
        }
    }
}

/// تمام ۱۶ ویژگی قابل تقویت/تضعیف هیرو و سلاح‌ها (باف با عدد مثبت، دیباف با عدد منفی)

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct HeroEnvironmentStatModifier {
    pub move_speed_pct: f32,
    pub primary_fire_rate_pct: f32,
    pub primary_damage_bonus: f32,
    pub primary_reload_speed_pct: f32,
    pub primary_magazine_bonus: i32,
    pub primary_range_bonus: f32,
    pub pistol_fire_rate_pct: f32,
    pub pistol_damage_bonus: f32,
    pub pistol_reload_speed_pct: f32,
    pub pistol_magazine_bonus: i32,
    pub pistol_range_bonus: f32,
    pub hp_regen_bonus: f32,
    pub stamina_regen_bonus: f32,
    pub max_hp_bonus: f32,
    pub max_stamina_bonus: f32,
    pub armor_bonus: f32,
    pub grenade_damage_bonus: f32,
    pub grenade_aoe_bonus: f32,
    pub grenade_max_charges_bonus: i32,
    pub grenade_recharge_speed_pct: f32,
    pub vision_range_bonus: f32,
}

impl HeroEnvironmentStatModifier {
    pub fn zero() -> Self {
        Self {
            move_speed_pct: 0.0,
            primary_fire_rate_pct: 0.0,
            primary_damage_bonus: 0.0,
            primary_reload_speed_pct: 0.0,
            primary_magazine_bonus: 0,
            primary_range_bonus: 0.0,
            pistol_fire_rate_pct: 0.0,
            pistol_damage_bonus: 0.0,
            pistol_reload_speed_pct: 0.0,
            pistol_magazine_bonus: 0,
            pistol_range_bonus: 0.0,
            hp_regen_bonus: 0.0,
            stamina_regen_bonus: 0.0,
            max_hp_bonus: 0.0,
            max_stamina_bonus: 0.0,
            armor_bonus: 0.0,
            grenade_damage_bonus: 0.0,
            grenade_aoe_bonus: 0.0,
            grenade_max_charges_bonus: 0,
            grenade_recharge_speed_pct: 0.0,
            vision_range_bonus: 0.0,
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.move_speed_pct += other.move_speed_pct;
        self.primary_fire_rate_pct += other.primary_fire_rate_pct;
        self.primary_damage_bonus += other.primary_damage_bonus;
        self.primary_reload_speed_pct += other.primary_reload_speed_pct;
        self.primary_magazine_bonus += other.primary_magazine_bonus;
        self.primary_range_bonus += other.primary_range_bonus;
        self.pistol_fire_rate_pct += other.pistol_fire_rate_pct;
        self.pistol_damage_bonus += other.pistol_damage_bonus;
        self.pistol_reload_speed_pct += other.pistol_reload_speed_pct;
        self.pistol_magazine_bonus += other.pistol_magazine_bonus;
        self.pistol_range_bonus += other.pistol_range_bonus;
        self.hp_regen_bonus += other.hp_regen_bonus;
        self.stamina_regen_bonus += other.stamina_regen_bonus;
        self.max_hp_bonus += other.max_hp_bonus;
        self.max_stamina_bonus += other.max_stamina_bonus;
        self.armor_bonus += other.armor_bonus;
        self.grenade_damage_bonus += other.grenade_damage_bonus;
        self.grenade_aoe_bonus += other.grenade_aoe_bonus;
        self.grenade_max_charges_bonus += other.grenade_max_charges_bonus;
        self.grenade_recharge_speed_pct += other.grenade_recharge_speed_pct;
        self.vision_range_bonus += other.vision_range_bonus;
    }

    pub fn to_item_stats_modifier(&self) -> ItemStatsModifier {
        ItemStatsModifier {
            move_speed_pct: self.move_speed_pct,
            primary_fire_rate_pct: self.primary_fire_rate_pct,
            primary_damage_bonus: self.primary_damage_bonus,
            primary_reload_speed_pct: self.primary_reload_speed_pct,
            primary_magazine_bonus: self.primary_magazine_bonus,
            primary_range_bonus: self.primary_range_bonus,
            pistol_fire_rate_pct: self.pistol_fire_rate_pct,
            pistol_damage_bonus: self.pistol_damage_bonus,
            pistol_reload_speed_pct: self.pistol_reload_speed_pct,
            pistol_magazine_bonus: self.pistol_magazine_bonus,
            pistol_range_bonus: self.pistol_range_bonus,
            hp_regen_bonus: self.hp_regen_bonus,
            stamina_regen_bonus: self.stamina_regen_bonus,
            max_hp_bonus: self.max_hp_bonus,
            max_stamina_bonus: self.max_stamina_bonus,
            armor_bonus: self.armor_bonus,
            grenade_damage_bonus: self.grenade_damage_bonus,
            grenade_aoe_bonus: self.grenade_aoe_bonus,
            grenade_max_charges_bonus: self.grenade_max_charges_bonus,
            grenade_recharge_speed_pct: self.grenade_recharge_speed_pct,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroEnvironmentModifiers {
    pub day: HeroEnvironmentStatModifier,
    pub night: HeroEnvironmentStatModifier,
    pub clear: HeroEnvironmentStatModifier,
    pub rainy: HeroEnvironmentStatModifier,
    pub snowy: HeroEnvironmentStatModifier,
}

impl HeroEnvironmentModifiers {
    pub fn combined(&self, is_night: bool, weather: WeatherKind) -> HeroEnvironmentStatModifier {
        let mut out = if is_night { self.night } else { self.day };
        let w = match weather {
            WeatherKind::Clear => &self.clear,
            WeatherKind::Rainy => &self.rainy,
            WeatherKind::Snowy => &self.snowy,
        };
        out.add(w);
        out
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct ItemStatsModifier {
    pub move_speed_pct: f32,
    pub primary_fire_rate_pct: f32,
    pub primary_damage_bonus: f32,
    pub primary_reload_speed_pct: f32,
    pub primary_magazine_bonus: i32,
    pub primary_range_bonus: f32,
    pub pistol_fire_rate_pct: f32,
    pub pistol_damage_bonus: f32,
    pub pistol_reload_speed_pct: f32,
    pub pistol_magazine_bonus: i32,
    pub pistol_range_bonus: f32,
    pub hp_regen_bonus: f32,
    pub stamina_regen_bonus: f32,
    pub max_hp_bonus: f32,
    pub max_stamina_bonus: f32,
    pub armor_bonus: f32,
    pub grenade_damage_bonus: f32,
    pub grenade_aoe_bonus: f32,
    pub grenade_max_charges_bonus: i32,
    pub grenade_recharge_speed_pct: f32,
}

impl ItemStatsModifier {
    pub fn zero() -> Self {
        Self {
            move_speed_pct: 0.0,
            primary_fire_rate_pct: 0.0,
            primary_damage_bonus: 0.0,
            primary_reload_speed_pct: 0.0,
            primary_magazine_bonus: 0,
            primary_range_bonus: 0.0,
            pistol_fire_rate_pct: 0.0,
            pistol_damage_bonus: 0.0,
            pistol_reload_speed_pct: 0.0,
            pistol_magazine_bonus: 0,
            pistol_range_bonus: 0.0,
            hp_regen_bonus: 0.0,
            stamina_regen_bonus: 0.0,
            max_hp_bonus: 0.0,
            max_stamina_bonus: 0.0,
            armor_bonus: 0.0,
            grenade_damage_bonus: 0.0,
            grenade_aoe_bonus: 0.0,
            grenade_max_charges_bonus: 0,
            grenade_recharge_speed_pct: 0.0,
        }
    }

    pub fn add(&mut self, other: &Self) {
        self.move_speed_pct += other.move_speed_pct;
        self.primary_fire_rate_pct += other.primary_fire_rate_pct;
        self.primary_damage_bonus += other.primary_damage_bonus;
        self.primary_reload_speed_pct += other.primary_reload_speed_pct;
        self.primary_magazine_bonus += other.primary_magazine_bonus;
        self.primary_range_bonus += other.primary_range_bonus;
        self.pistol_fire_rate_pct += other.pistol_fire_rate_pct;
        self.pistol_damage_bonus += other.pistol_damage_bonus;
        self.pistol_reload_speed_pct += other.pistol_reload_speed_pct;
        self.pistol_magazine_bonus += other.pistol_magazine_bonus;
        self.pistol_range_bonus += other.pistol_range_bonus;
        self.hp_regen_bonus += other.hp_regen_bonus;
        self.stamina_regen_bonus += other.stamina_regen_bonus;
        self.max_hp_bonus += other.max_hp_bonus;
        self.max_stamina_bonus += other.max_stamina_bonus;
        self.armor_bonus += other.armor_bonus;
        self.grenade_damage_bonus += other.grenade_damage_bonus;
        self.grenade_aoe_bonus += other.grenade_aoe_bonus;
        self.grenade_max_charges_bonus += other.grenade_max_charges_bonus;
        self.grenade_recharge_speed_pct += other.grenade_recharge_speed_pct;
    }

    pub fn split_buffs_and_debuffs(&self) -> (Vec<String>, Vec<String>) {
        let mut buffs = Vec::new();
        let mut debuffs = Vec::new();

        fn push_pct(buffs: &mut Vec<String>, debuffs: &mut Vec<String>, val: f32, label: &str) {
            if val > 0.0001 {
                buffs.push(format!("+{:.0}% {}", val * 100.0, label));
            } else if val < -0.0001 {
                debuffs.push(format!("{:.0}% {}", val * 100.0, label));
            }
        }
        fn push_f32(
            buffs: &mut Vec<String>,
            debuffs: &mut Vec<String>,
            val: f32,
            label: &str,
            suffix: &str,
        ) {
            if val > 0.0001 {
                buffs.push(format!("+{:.1}{} {}", val, suffix, label));
            } else if val < -0.0001 {
                debuffs.push(format!("{:.1}{} {}", val, suffix, label));
            }
        }
        fn push_i32(buffs: &mut Vec<String>, debuffs: &mut Vec<String>, val: i32, label: &str) {
            if val > 0 {
                buffs.push(format!("+{} {}", val, label));
            } else if val < 0 {
                debuffs.push(format!("{} {}", val, label));
            }
        }

        push_pct(&mut buffs, &mut debuffs, self.move_speed_pct, "Move Speed");
        push_pct(&mut buffs, &mut debuffs, self.primary_fire_rate_pct, "Primary Fire Rate");
        push_f32(&mut buffs, &mut debuffs, self.primary_damage_bonus, "Primary Damage", "");
        push_pct(&mut buffs, &mut debuffs, self.primary_reload_speed_pct, "Primary Reload Speed");
        push_i32(&mut buffs, &mut debuffs, self.primary_magazine_bonus, "Primary Magazine");
        push_f32(&mut buffs, &mut debuffs, self.primary_range_bonus, "Primary Range", "m");

        push_pct(&mut buffs, &mut debuffs, self.pistol_fire_rate_pct, "Pistol Fire Rate");
        push_f32(&mut buffs, &mut debuffs, self.pistol_damage_bonus, "Pistol Damage", "");
        push_pct(&mut buffs, &mut debuffs, self.pistol_reload_speed_pct, "Pistol Reload Speed");
        push_i32(&mut buffs, &mut debuffs, self.pistol_magazine_bonus, "Pistol Magazine");
        push_f32(&mut buffs, &mut debuffs, self.pistol_range_bonus, "Pistol Range", "m");

        push_f32(&mut buffs, &mut debuffs, self.max_hp_bonus, "Max Health", "");
        push_f32(&mut buffs, &mut debuffs, self.hp_regen_bonus, "Health Regen", "/s");
        push_f32(&mut buffs, &mut debuffs, self.max_stamina_bonus, "Max Stamina", "");
        push_f32(&mut buffs, &mut debuffs, self.stamina_regen_bonus, "Stamina Regen", "/s");
        push_f32(&mut buffs, &mut debuffs, self.armor_bonus, "Armor", "");
        push_f32(&mut buffs, &mut debuffs, self.grenade_damage_bonus, "Grenade Damage", "");
        push_f32(&mut buffs, &mut debuffs, self.grenade_aoe_bonus, "Grenade AOE", "m");
        push_i32(&mut buffs, &mut debuffs, self.grenade_max_charges_bonus, "Grenade Max Charges");
        push_pct(&mut buffs, &mut debuffs, self.grenade_recharge_speed_pct, "Grenade Recharge Speed");

        (buffs, debuffs)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemModeDef {
    pub name: String,
    pub color_rgb: (f32, f32, f32),
    pub stats: ItemStatsModifier,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemActiveAbility {
    None,
    Teleport,
    ShadowBlade,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemDef {
    pub id: String,
    pub name: String,
    pub short_code: String,
    pub icon_path: String,
    pub icon_color_rgb: (f32, f32, f32),
    pub category: ItemCategory,
    pub tier: u8,
    pub cost: u32,
    pub description: String,
    pub recipe: Vec<String>,
    pub stackable: bool,
    pub dedicated_tp_slot: bool,
    pub active_ability: ItemActiveAbility,
    pub stats: ItemStatsModifier,
    pub modes: Vec<ItemModeDef>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TeleportConfig {
    pub item_id: String,
    pub shortcut_key: String,
    pub cooldown_secs: f32,
    pub channel_duration_secs: f32,
    pub building_teleport_radius: f32,
    pub min_building_clearance: f32,
    pub max_click_snap_distance: f32,
    pub cancel_on_left_click: bool,
    pub cancel_on_move: bool,
    pub consume_on_use: bool,
    pub portal_radius: f32,
    pub portal_height: f32,
    pub portal_core_color_rgb: (f32, f32, f32),
    pub portal_edge_color_rgb: (f32, f32, f32),
    pub portal_bloom_intensity: f32,
    pub preview_valid_color_rgb: (f32, f32, f32),
    pub preview_invalid_color_rgb: (f32, f32, f32),
    pub illusion_color_rgba: (f32, f32, f32, f32),
    pub illusion_emissive_rgb: (f32, f32, f32),
    pub loop_sound: String,
    pub loop_volume: f32,
    pub complete_origin_sound: String,
    pub complete_origin_volume: f32,
    pub complete_dest_sound: String,
    pub complete_dest_volume: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShadowBladeConfig {
    pub item_id: String,
    pub duration_secs: f32,
    pub cooldown_secs: f32,
    pub invis_move_speed_bonus_pct: f32,
    pub turret_true_sight_radius: f32,
    pub water_glass_color_rgba: (f32, f32, f32, f32),
    pub water_glass_emissive_rgb: (f32, f32, f32),
    pub water_glass_metallic: f32,
    pub water_glass_roughness: f32,
    pub buff_bar_color_rgb: (f32, f32, f32),
    pub activate_sound: String,
    pub activate_volume: f32,
    pub end_sound: String,
    pub end_volume: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ShopUiConfig {
    pub shop_panel_width_px: f32,
    pub shop_panel_height_px: f32,
    pub catalog_column_width_px: f32,
    pub inspector_column_width_px: f32,
    pub tooltip_width_px: f32,
    pub drag_threshold_px: f32,
    pub courier_world_drop_radius: f32,
    pub hero_world_drop_radius: f32,
    pub hero_to_courier_transfer_radius: f32,
    pub pyramid_node_size_px: f32,
    pub pyramid_connector_thickness_px: f32,
    pub shop_error_toast_secs: f32,
    pub hud_inventory_left_px: f32,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct ItemsConfig {
    pub max_inventory_slots: usize,
    pub max_stash_slots: usize,
    pub sell_refund_ratio: f32,
    pub shop_ui: ShopUiConfig,
    pub teleport: TeleportConfig,
    pub shadow_blade: ShadowBladeConfig,
    pub items: Vec<ItemDef>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct InventoryItemState {
    pub item_id: String,
    pub active_mode: usize,
    pub slot: usize,
    pub count: u32,
}

impl ItemsConfig {
    pub fn load_from_file(path: &str) -> Self {
        let content = fs::read_to_string(path).unwrap_or_else(|e| {
            panic!("خطای بحرانی: فایل کانفیگ آیتم‌ها '{}' خوانده نشد: {}", path, e)
        });
        Self::load_from_str(&content, path)
    }

    pub fn load_from_str(content: &str, label: &str) -> Self {
        let cfg = ron::from_str::<ItemsConfig>(content).unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی: ساختار فایل کانفیگ آیتم‌ها '{}' نامعتبر است: {}",
                label, e
            )
        });
        cfg.validate();
        cfg
    }

    pub fn validate(&self) {
        assert!(self.max_inventory_slots > 0, "max_inventory_slots باید بزرگ‌تر از صفر باشد");
        assert!(self.max_stash_slots > 0, "max_stash_slots باید بزرگ‌تر از صفر باشد");
        assert!(
            (0.0..=1.0).contains(&self.sell_refund_ratio),
            "sell_refund_ratio باید بین 0.0 و 1.0 باشد"
        );
        let sui = &self.shop_ui;
        assert!(sui.shop_panel_width_px > 400.0, "shop_ui.shop_panel_width_px نامعتبر است");
        assert!(sui.shop_panel_height_px > 300.0, "shop_ui.shop_panel_height_px نامعتبر است");
        assert!(sui.catalog_column_width_px > 200.0, "shop_ui.catalog_column_width_px نامعتبر است");
        assert!(sui.inspector_column_width_px > 200.0, "shop_ui.inspector_column_width_px نامعتبر است");
        assert!(sui.tooltip_width_px > 120.0, "shop_ui.tooltip_width_px نامعتبر است");
        assert!(sui.drag_threshold_px > 0.0, "shop_ui.drag_threshold_px باید مثبت باشد");
        assert!(sui.courier_world_drop_radius > 0.0, "shop_ui.courier_world_drop_radius باید مثبت باشد");
        assert!(sui.hero_world_drop_radius > 0.0, "shop_ui.hero_world_drop_radius باید مثبت باشد");
        assert!(sui.hero_to_courier_transfer_radius > 0.0, "shop_ui.hero_to_courier_transfer_radius باید مثبت باشد");
        assert!(sui.pyramid_node_size_px > 20.0, "shop_ui.pyramid_node_size_px نامعتبر است");
        assert!(sui.pyramid_connector_thickness_px > 0.0, "shop_ui.pyramid_connector_thickness_px باید مثبت باشد");
        assert!(sui.shop_error_toast_secs > 0.0, "shop_ui.shop_error_toast_secs باید مثبت باشد");
        assert!(sui.hud_inventory_left_px >= 0.0, "shop_ui.hud_inventory_left_px نامعتبر است");
        assert!(!self.items.is_empty(), "لیست items در items.ron نمی‌تواند خالی باشد");
        let tp = &self.teleport;
        assert!(!tp.item_id.trim().is_empty(), "teleport.item_id در items.ron خالی است");
        assert!(!tp.shortcut_key.trim().is_empty(), "teleport.shortcut_key در items.ron خالی است");
        assert!(tp.cooldown_secs >= 0.0, "teleport.cooldown_secs نامعتبر است");
        assert!(tp.channel_duration_secs > 0.0, "teleport.channel_duration_secs باید مثبت باشد");
        assert!(tp.building_teleport_radius > 0.0, "teleport.building_teleport_radius باید مثبت باشد");
        assert!(tp.min_building_clearance >= 0.0, "teleport.min_building_clearance نامعتبر است");
        assert!(tp.max_click_snap_distance > 0.0, "teleport.max_click_snap_distance باید مثبت باشد");
        assert!(tp.portal_radius > 0.0, "teleport.portal_radius باید مثبت باشد");
        assert!(tp.portal_height > 0.0, "teleport.portal_height باید مثبت باشد");
        assert!(tp.portal_bloom_intensity >= 0.0, "teleport.portal_bloom_intensity نامعتبر است");
        assert!(!tp.loop_sound.trim().is_empty(), "teleport.loop_sound خالی است");
        assert!((0.0..=2.0).contains(&tp.loop_volume), "teleport.loop_volume نامعتبر است");
        assert!(!tp.complete_origin_sound.trim().is_empty(), "teleport.complete_origin_sound خالی است");
        assert!((0.0..=2.0).contains(&tp.complete_origin_volume), "teleport.complete_origin_volume نامعتبر است");
        assert!(!tp.complete_dest_sound.trim().is_empty(), "teleport.complete_dest_sound خالی است");
        assert!((0.0..=2.0).contains(&tp.complete_dest_volume), "teleport.complete_dest_volume نامعتبر است");
        let sb = &self.shadow_blade;
        assert!(!sb.item_id.trim().is_empty(), "shadow_blade.item_id خالی است");
        assert!(sb.duration_secs > 0.0, "shadow_blade.duration_secs باید مثبت باشد");
        assert!(sb.cooldown_secs >= 0.0, "shadow_blade.cooldown_secs نامعتبر است");
        assert!(sb.turret_true_sight_radius > 0.0, "shadow_blade.turret_true_sight_radius باید مثبت باشد");
        assert!(!sb.activate_sound.trim().is_empty(), "shadow_blade.activate_sound خالی است");
        assert!((0.0..=2.0).contains(&sb.activate_volume), "shadow_blade.activate_volume نامعتبر است");
        assert!(!sb.end_sound.trim().is_empty(), "shadow_blade.end_sound خالی است");
        assert!((0.0..=2.0).contains(&sb.end_volume), "shadow_blade.end_volume نامعتبر است");

        let mut seen = HashSet::new();
        let mut cost_map = HashMap::new();
        for item in &self.items {
            assert!(!item.id.trim().is_empty(), "شناسه آیتم نمی‌تواند خالی باشد");
            assert!(
                seen.insert(item.id.clone()),
                "شناسه آیتم '{}' تکراری است!",
                item.id
            );
            assert!(!item.name.trim().is_empty(), "نام آیتم '{}' خالی است", item.id);
            assert!(!item.short_code.trim().is_empty(), "short_code آیتم '{}' خالی است", item.id);
            assert!(item.cost > 0, "قیمت آیتم '{}' باید بزرگ‌تر از صفر باشد", item.id);
            cost_map.insert(item.id.as_str(), item.cost);
        }

        for item in &self.items {
            let mut comp_sum = 0_u32;
            for comp_id in &item.recipe {
                assert!(
                    comp_id != &item.id,
                    "آیتم '{}' نمی‌تواند در recipe خودش باشد!",
                    item.id
                );
                let comp_cost = cost_map.get(comp_id.as_str()).unwrap_or_else(|| {
                    panic!(
                        "کامپوننت '{}' در recipe آیتم '{}' در items.ron تعریف نشده است!",
                        comp_id, item.id
                    )
                });
                comp_sum = comp_sum.saturating_add(*comp_cost);
            }
            assert!(
                item.cost >= comp_sum,
                "قیمت کل آیتم '{}' ({}) نباید کمتر از مجموع قیمت کامپوننت‌هایش ({}) باشد!",
                item.id,
                item.cost,
                comp_sum
            );
        }
    }

    pub fn get_item(&self, id: &str) -> Option<&ItemDef> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn builds_into(&self, id: &str) -> Vec<&ItemDef> {
        self.items
            .iter()
            .filter(|i| i.recipe.iter().any(|r| r == id))
            .collect()
    }

    pub fn sell_price(&self, item_id: &str) -> u32 {
        if let Some(target_id) = parse_recipe_target(item_id) {
            let fee = self.recipe_fee(target_id);
            return ((fee as f32) * self.sell_refund_ratio).round() as u32;
        }
        self.get_item(item_id)
            .map(|it| ((it.cost as f32) * self.sell_refund_ratio).round() as u32)
            .unwrap_or(0)
    }

    /// محاسبه هزینه باقی‌مانده خرید یک آیتم با کسر قیمت کامپوننت‌ها و اسکرول Recipe موجود در اینونتوری
    pub fn effective_purchase_cost(
        &self,
        item_id: &str,
        inventory: &[InventoryItemState],
    ) -> Option<(u32, Vec<usize>)> {
        if let Some(target_id) = parse_recipe_target(item_id) {
            let fee = self.recipe_fee(target_id);
            return (fee > 0).then_some((fee, Vec::new()));
        }
        let item = self.get_item(item_id)?;
        let mut consumed_indices = Vec::new();
        let mut used = vec![false; inventory.len()];
        let mut owned_value = 0_u32;

        for comp_id in &item.recipe {
            if let Some((idx, _)) = inventory
                .iter()
                .enumerate()
                .find(|(idx, inv)| !used[*idx] && &inv.item_id == comp_id)
            {
                used[idx] = true;
                consumed_indices.push(idx);
                if let Some(comp) = self.get_item(comp_id) {
                    owned_value = owned_value.saturating_add(comp.cost);
                }
            }
        }

        let rec_fee = self.recipe_fee(item_id);
        if rec_fee > 0 {
            let rec_id = recipe_item_id(item_id);
            if let Some((idx, _)) = inventory
                .iter()
                .enumerate()
                .find(|(idx, inv)| !used[*idx] && inv.item_id == rec_id)
            {
                used[idx] = true;
                consumed_indices.push(idx);
                owned_value = owned_value.saturating_add(rec_fee);
            }
        }

        let remaining_cost = item.cost.saturating_sub(owned_value);
        Some((remaining_cost, consumed_indices))
    }

    /// مجموع باف‌ها و دیباف‌های تمام آیتم‌های موجود در اینونتوری (با در نظر گرفتن حالت فعال)
    pub fn sum_inventory_modifiers(&self, inventory: &[InventoryItemState]) -> ItemStatsModifier {
        let mut total = ItemStatsModifier::zero();
        for inv in inventory {
            if let Some(item) = self.get_item(&inv.item_id) {
                total.add(&item.stats);
                if !item.modes.is_empty() {
                    let m_idx = inv.active_mode.min(item.modes.len() - 1);
                    total.add(&item.modes[m_idx].stats);
                }
            }
        }
        total
    }

    /// هزینه اسکرول/ترکیب آیتم (تفاوت قیمت کل آیتم با مجموع قیمت کامپوننت‌های مستقیم آن)
    pub fn recipe_fee(&self, item_id: &str) -> u32 {
        let Some(item) = self.get_item(item_id) else {
            return 0;
        };
        if item.recipe.is_empty() {
            return 0;
        }
        let comp_sum: u32 = item
            .recipe
            .iter()
            .filter_map(|cid| self.get_item(cid).map(|c| c.cost))
            .sum();
        item.cost.saturating_sub(comp_sum)
    }

    /// باز کردن بازگشتی درخت ساخت یک آیتم برای سبد خرید با حذف آیتم‌هایی که در اینونتوری موجودند
    /// اگر آیتم ترکیبی هزینه ساخت (Recipe Fee) داشته باشد، خود آیتم به عنوان مرحله نهایی ترکیب در انتها اضافه می‌شود.
    pub fn expand_missing_recipe_items(
        &self,
        item_id: &str,
        inventory: &[InventoryItemState],
    ) -> Vec<String> {
        let mut available: Vec<String> = inventory.iter().map(|i| i.item_id.clone()).collect();
        let mut out = Vec::new();
        self.expand_recursive(item_id, &mut available, &mut out);
        out
    }

    fn expand_recursive(
        &self,
        item_id: &str,
        available: &mut Vec<String>,
        out: &mut Vec<String>,
    ) {
        let Some(item) = self.get_item(item_id) else {
            return;
        };
        if item.recipe.is_empty() {
            out.push(item.id.clone());
            return;
        }
        for comp_id in &item.recipe {
            if let Some(pos) = available.iter().position(|id| id == comp_id) {
                available.remove(pos);
            } else {
                self.expand_recursive(comp_id, available, out);
            }
        }
        if self.recipe_fee(item_id) > 0 {
            let rec_id = recipe_item_id(item_id);
            if let Some(pos) = available.iter().position(|id| id == &rec_id) {
                available.remove(pos);
            } else {
                out.push(rec_id);
            }
        }
    }
}

/// بازمحاسبه کامل تمام ۱۶ ویژگی هیرو و سلاح‌ها بر اساس هیروی پایه + سطح + آیتم‌های اینونتوری
pub fn recompute_player_slot_stats(
    p: &mut PlayerSlotState,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) {
    let base = config.active_hero(p.hero_index);
    let levels_gained = p.hero_level.saturating_sub(1) as f32;
    let growth = &base.level_growth;

    let mut total_mod = base
        .environment
        .combined(p.active_is_night, p.active_weather)
        .to_item_stats_modifier();
    for inv in &p.inventory {
        if let Some(item) = items_cfg.get_item(&inv.item_id) {
            total_mod.add(&item.stats);
            if !item.modes.is_empty() {
                let m_idx = inv.active_mode.min(item.modes.len() - 1);
                total_mod.add(&item.modes[m_idx].stats);
            }
        }
    }

    let prev_max_hp = p.hero_max_hp.max(1.0);
    let prev_max_stamina = p.hero_max_stamina.max(1.0);
    let hp_ratio = (p.hero_hp / prev_max_hp).clamp(0.0, 1.0);
    let stamina_ratio = (p.hero_stamina / prev_max_stamina).clamp(0.0, 1.0);

    // ویژگی‌های هیرو
    p.hero_move_speed_multiplier = (base.move_speed_multiplier + total_mod.move_speed_pct).max(0.25);
    p.hero_max_hp =
        (base.max_hp + growth.max_hp_per_level * levels_gained + total_mod.max_hp_bonus).max(50.0);
    p.hero_hp = if p.is_dead {
        0.0
    } else {
        (hp_ratio * p.hero_max_hp).clamp(1.0, p.hero_max_hp)
    };
    p.hero_hp_regen_per_sec =
        (base.hp_regen_per_sec + growth.hp_regen_per_level * levels_gained + total_mod.hp_regen_bonus)
            .max(0.0);

    p.hero_max_stamina = (base.max_stamina + total_mod.max_stamina_bonus).max(10.0);
    p.hero_stamina = (stamina_ratio * p.hero_max_stamina).clamp(0.0, p.hero_max_stamina);
    p.hero_stamina_regen_per_sec = (base.stamina_regen_per_sec
        + growth.stamina_regen_per_level * levels_gained
        + total_mod.stamina_regen_bonus)
        .max(1.0);
    p.hero_armor = base.armor + growth.armor_per_level * levels_gained + total_mod.armor_bonus;

    // اسلحه اصلی (Primary)
    let base_p_rate =
        (base.weapon.fire_rate_secs - growth.fire_rate_reduction_per_level * levels_gained).max(0.01);
    let base_p_reload =
        (base.weapon.reload_duration_secs - growth.reload_time_reduction_per_level * levels_gained)
            .max(0.05);
    let p_rate_mult = (1.0 + total_mod.primary_fire_rate_pct).max(0.20);
    let p_reload_mult = (1.0 + total_mod.primary_reload_speed_pct).max(0.20);

    p.hero_weapon.fire_rate_secs =
        (base_p_rate / p_rate_mult).max(config.combat.min_fire_rate_secs);
    p.hero_weapon.reload_duration_secs =
        (base_p_reload / p_reload_mult).max(config.combat.min_reload_duration_secs);
    p.hero_weapon.damage = (base.weapon.damage
        + growth.damage_per_level * levels_gained
        + total_mod.primary_damage_bonus)
        .max(1.0);
    p.hero_weapon.magazine_size =
        ((base.weapon.magazine_size as i32) + total_mod.primary_magazine_bonus).max(1) as u32;
    p.hero_weapon.max_range = (base.weapon.max_range
        + growth.max_range_per_level * levels_gained
        + total_mod.primary_range_bonus)
        .max(2.0);

    // اسلحه دوم (Pistol)
    let pis_rate_mult = (1.0 + total_mod.pistol_fire_rate_pct).max(0.20);
    let pis_reload_mult = (1.0 + total_mod.pistol_reload_speed_pct).max(0.20);

    p.hero_pistol.fire_rate_secs =
        (base.pistol.fire_rate_secs / pis_rate_mult).max(config.combat.min_fire_rate_secs);
    p.hero_pistol.reload_duration_secs =
        (base.pistol.reload_duration_secs / pis_reload_mult).max(config.combat.min_reload_duration_secs);
    p.hero_pistol.damage = (base.pistol.damage + total_mod.pistol_damage_bonus).max(1.0);
    p.hero_pistol.magazine_size =
        ((base.pistol.magazine_size as i32) + total_mod.pistol_magazine_bonus).max(1) as u32;
    p.hero_pistol.max_range = (base.pistol.max_range + total_mod.pistol_range_bonus).max(2.0);

    match p.active_weapon {
        ActiveWeaponSlot::Primary => {
            p.ammo_in_mag = p.ammo_in_mag.min(p.hero_weapon.magazine_size);
            p.other_weapon_ammo_in_mag = p.other_weapon_ammo_in_mag.min(p.hero_pistol.magazine_size);
        }
        ActiveWeaponSlot::Pistol => {
            p.ammo_in_mag = p.ammo_in_mag.min(p.hero_pistol.magazine_size);
            p.other_weapon_ammo_in_mag = p.other_weapon_ammo_in_mag.min(p.hero_weapon.magazine_size);
        }
    }

    // تقویت نارنجک‌ها با آیتم‌ها
    let base_g = base.grenade;
    p.hero_grenade.damage = (base_g.damage + total_mod.grenade_damage_bonus).max(1.0);
    p.hero_grenade.aoe_radius = (base_g.aoe_radius + total_mod.grenade_aoe_bonus).max(0.5);
    p.hero_grenade.max_charges =
        ((base_g.max_charges as i32) + total_mod.grenade_max_charges_bonus).max(1) as u32;
    let g_recharge_mult = (1.0 + total_mod.grenade_recharge_speed_pct).max(0.25);
    p.hero_grenade.recharge_secs = (base_g.recharge_secs / g_recharge_mult).max(1.0);
    p.grenade_charges = p.grenade_charges.min(p.hero_grenade.max_charges);
    if p.grenade_charges < p.hero_grenade.max_charges && p.grenade_recharge_remaining <= 0.0 {
        p.grenade_recharge_remaining = p.hero_grenade.recharge_secs;
    }
}

/// ترکیب خودکار آیتم‌های کامل‌شده در یک اینونتوری (زمانی که تمام اجزای دستور ساخت و بدون هزینه اضافه موجود باشند)
pub fn auto_combine_items(
    inventory: &mut Vec<InventoryItemState>,
    max_slots: usize,
    items_cfg: &ItemsConfig,
) -> bool {
    let mut crafted_any = false;
    loop {
        let mut found_craft: Option<(String, Vec<usize>)> = None;
        for item_def in &items_cfg.items {
            if item_def.recipe.is_empty() || item_def.dedicated_tp_slot {
                continue;
            }
            let expected_consumed =
                item_def.recipe.len() + usize::from(items_cfg.recipe_fee(&item_def.id) > 0);
            if let Some((remaining_cost, consumed_indices)) =
                items_cfg.effective_purchase_cost(&item_def.id, inventory)
            {
                if remaining_cost == 0 && consumed_indices.len() == expected_consumed {
                    found_craft = Some((item_def.id.clone(), consumed_indices));
                    break;
                }
            }
        }
        let Some((crafted_id, mut consumed_indices)) = found_craft else {
            break;
        };
        let preferred_slot = consumed_indices
            .iter()
            .filter_map(|&idx| inventory.get(idx).map(|it| it.slot))
            .min()
            .unwrap_or(0);
        consumed_indices.sort_unstable_by(|a, b| b.cmp(a));
        for idx in consumed_indices {
            inventory.remove(idx);
        }
        let free_slot = if preferred_slot < max_slots
            && !inventory.iter().any(|it| it.slot == preferred_slot)
        {
            preferred_slot
        } else {
            (0..max_slots)
                .find(|s| !inventory.iter().any(|it| it.slot == *s))
                .unwrap_or(0)
        };
        inventory.push(InventoryItemState {
            item_id: crafted_id,
            active_mode: 0,
            slot: free_slot,
            count: 1,
        });
        crafted_any = true;
    }
    crafted_any
}

/// ثبت پیام وضعیت/خطای شاپ برای پخش صدا و نمایش به بازیکن
pub fn set_shop_notice(p: &mut PlayerSlotState, text: impl Into<String>, is_error: bool) {
    p.shop_notice_seq = p.shop_notice_seq.wrapping_add(1);
    p.shop_notice_text = text.into();
    p.shop_notice_is_error = is_error;
}

/// تلاش برای قرار دادن یک آیتم (یا اسکرول Recipe) در یک اینونتوری مشخص (با پشتیبانی استک و ترکیب خودکار حتی در حالت پر)
fn try_insert_into_inventory(
    inv: &mut Vec<InventoryItemState>,
    max_slots: usize,
    item_id: &str,
    count: u32,
    items_cfg: &ItemsConfig,
) -> bool {
    let is_stack = items_cfg
        .get_item(item_id)
        .is_some_and(|d| d.stackable);
    if is_stack {
        if let Some(existing) = inv.iter_mut().find(|it| it.item_id == item_id) {
            existing.count = existing.count.saturating_add(count);
            return true;
        }
    }
    if let Some(free_slot) = (0..max_slots).find(|s| !inv.iter().any(|it| it.slot == *s)) {
        inv.push(InventoryItemState {
            item_id: item_id.to_string(),
            active_mode: 0,
            slot: free_slot,
            count,
        });
        auto_combine_items(inv, max_slots, items_cfg);
        return true;
    }
    // اگر اینونتوری پر است، بررسی کن آیا ورود این آیتم باعث ترکیب خودکار و آزاد شدن اسلات می‌شود
    let mut test_inv = inv.clone();
    test_inv.push(InventoryItemState {
        item_id: item_id.to_string(),
        active_mode: 0,
        slot: max_slots,
        count,
    });
    if auto_combine_items(&mut test_inv, max_slots, items_cfg) && test_inv.len() <= max_slots {
        *inv = test_inv;
        return true;
    }
    false
}

/// محاسبه خرید ترکیبی از یک اینونتوری اصلی + اختیاری Stash (در صورت حضور واحد در چشمه)
fn try_craft_buy_into_target(
    hero_gold: &mut u32,
    target_inv: &mut Vec<InventoryItemState>,
    target_max_slots: usize,
    mut stash_opt: Option<&mut Vec<InventoryItemState>>,
    item_id: &str,
    items_cfg: &ItemsConfig,
) -> Result<bool, &'static str> {
    let Some(item_def) = items_cfg.get_item(item_id) else {
        return Err("INVALID ITEM");
    };

    if item_def.stackable {
        if let Some(existing) = target_inv.iter_mut().find(|it| it.item_id == item_id) {
            if *hero_gold < item_def.cost {
                return Err("NOT ENOUGH GOLD");
            }
            *hero_gold -= item_def.cost;
            existing.count = existing.count.saturating_add(1);
            return Ok(true);
        }
    }

    let mut target_used = vec![false; target_inv.len()];
    let mut stash_used = stash_opt
        .as_deref()
        .map(|s| vec![false; s.len()])
        .unwrap_or_default();
    let mut target_consumed = Vec::new();
    let mut stash_consumed = Vec::new();
    let mut owned_val = 0_u32;

    if !item_def.recipe.is_empty() {
        for comp_id in &item_def.recipe {
            if let Some((idx, _)) = target_inv
                .iter()
                .enumerate()
                .find(|(idx, inv)| !target_used[*idx] && &inv.item_id == comp_id)
            {
                target_used[idx] = true;
                target_consumed.push(idx);
                if let Some(c) = items_cfg.get_item(comp_id) {
                    owned_val = owned_val.saturating_add(c.cost);
                }
            } else if let Some(ref stash) = stash_opt {
                if let Some((idx, _)) = stash
                    .iter()
                    .enumerate()
                    .find(|(idx, inv)| !stash_used[*idx] && &inv.item_id == comp_id)
                {
                    stash_used[idx] = true;
                    stash_consumed.push(idx);
                    if let Some(c) = items_cfg.get_item(comp_id) {
                        owned_val = owned_val.saturating_add(c.cost);
                    }
                }
            }
        }
        let rec_fee = items_cfg.recipe_fee(item_id);
        if rec_fee > 0 {
            let rec_id = recipe_item_id(item_id);
            if let Some((idx, _)) = target_inv
                .iter()
                .enumerate()
                .find(|(idx, inv)| !target_used[*idx] && inv.item_id == rec_id)
            {
                target_used[idx] = true;
                target_consumed.push(idx);
                owned_val = owned_val.saturating_add(rec_fee);
            } else if let Some(ref stash) = stash_opt {
                if let Some((idx, _)) = stash
                    .iter()
                    .enumerate()
                    .find(|(idx, inv)| !stash_used[*idx] && inv.item_id == rec_id)
                {
                    stash_used[idx] = true;
                    stash_consumed.push(idx);
                    owned_val = owned_val.saturating_add(rec_fee);
                }
            }
        }
    }

    let cost_to_pay = item_def.cost.saturating_sub(owned_val);
    if *hero_gold < cost_to_pay {
        return Err("NOT ENOUGH GOLD");
    }

    let final_slots = target_inv.len() - target_consumed.len() + 1;
    if final_slots > target_max_slots {
        // بررسی اینکه آیا اضافه شدن آیتم باعث ترکیب خودکار ثانویه و آزاد شدن اسلات می‌شود
        let mut test_inv = target_inv.clone();
        let mut tc_sorted = target_consumed.clone();
        tc_sorted.sort_unstable_by(|a, b| b.cmp(a));
        for idx in tc_sorted {
            test_inv.remove(idx);
        }
        test_inv.push(InventoryItemState {
            item_id: item_id.to_string(),
            active_mode: 0,
            slot: target_max_slots,
            count: 1,
        });
        if auto_combine_items(&mut test_inv, target_max_slots, items_cfg)
            && test_inv.len() <= target_max_slots
        {
            *hero_gold -= cost_to_pay;
            *target_inv = test_inv;
            if let Some(stash) = stash_opt.as_deref_mut() {
                stash_consumed.sort_unstable_by(|a, b| b.cmp(a));
                for idx in stash_consumed {
                    stash.remove(idx);
                }
            }
            return Ok(true);
        }
        return Ok(false); // اینونتوری هدف پر است -> اجازه fallback به Stash
    }

    let preferred_slot = target_consumed
        .iter()
        .filter_map(|&idx| target_inv.get(idx).map(|it| it.slot))
        .min();
    *hero_gold -= cost_to_pay;
    target_consumed.sort_unstable_by(|a, b| b.cmp(a));
    for idx in target_consumed {
        target_inv.remove(idx);
    }
    if let Some(stash) = stash_opt.as_deref_mut() {
        stash_consumed.sort_unstable_by(|a, b| b.cmp(a));
        for idx in stash_consumed {
            stash.remove(idx);
        }
    }
    let free_slot = preferred_slot
        .filter(|s| !target_inv.iter().any(|it| it.slot == *s))
        .or_else(|| (0..target_max_slots).find(|s| !target_inv.iter().any(|it| it.slot == *s)))
        .unwrap_or(0);
    target_inv.push(InventoryItemState {
        item_id: item_id.to_string(),
        active_mode: 0,
        slot: free_slot,
        count: 1,
    });
    auto_combine_items(target_inv, target_max_slots, items_cfg);
    Ok(true)
}

/// خرید فقط اسکرول Recipe یک آیتم ترکیبی (به‌عنوان یک آیتم جدا با هزینه صرفاً recipe_fee)
pub fn try_buy_recipe(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    target_item_id: &str,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    let fee = items_cfg.recipe_fee(target_item_id);
    if fee == 0 {
        return false;
    }
    if p.hero_gold < fee {
        set_shop_notice(p, "NOT ENOUGH GOLD FOR RECIPE", true);
        return false;
    }

    let hero_cfg = config.active_hero(p.hero_index);
    let courier_max_slots = hero_cfg.courier.inventory_slots;
    let hero_in_shop =
        !p.is_dead && fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) > 0.0;
    let courier_in_shop = !p.courier_is_dead
        && fountain_regen_bonus_for_pos(config, p.hero_team, p.courier_pos) > 0.0;
    let rec_id = recipe_item_id(target_item_id);

    // قانون ۱: هیرو در محدوده شاپ -> اول هیرو، اگر پر بود Stash
    if hero_in_shop {
        if try_insert_into_inventory(
            &mut p.inventory,
            items_cfg.max_inventory_slots,
            &rec_id,
            1,
            items_cfg,
        ) {
            p.hero_gold -= fee;
            recompute_player_slot_stats(p, config, items_cfg);
            return true;
        }
        if try_insert_into_inventory(
            &mut p.stash,
            items_cfg.max_stash_slots,
            &rec_id,
            1,
            items_cfg,
        ) {
            p.hero_gold -= fee;
            return true;
        }
        set_shop_notice(p, "HERO & STASH FULL - CANNOT BUY", true);
        return false;
    }

    // قانون ۲: هیرو خارج از شاپ، کوریر در محدوده شاپ -> اول کوریر، اگر پر بود Stash
    if courier_in_shop {
        if try_insert_into_inventory(
            &mut p.courier_inventory,
            courier_max_slots,
            &rec_id,
            1,
            items_cfg,
        ) {
            p.hero_gold -= fee;
            return true;
        }
        if try_insert_into_inventory(
            &mut p.stash,
            items_cfg.max_stash_slots,
            &rec_id,
            1,
            items_cfg,
        ) {
            p.hero_gold -= fee;
            return true;
        }
        set_shop_notice(p, "COURIER & STASH FULL - CANNOT BUY", true);
        return false;
    }

    // قانون ۳: هر دو خارج از شاپ -> خرید به Stash
    if try_insert_into_inventory(
        &mut p.stash,
        items_cfg.max_stash_slots,
        &rec_id,
        1,
        items_cfg,
    ) {
        p.hero_gold -= fee;
        return true;
    }
    set_shop_notice(p, "STASH FULL - CANNOT BUY", true);
    false
}

/// خرید آیتم با مدیریت کامل ۳ اینونتوری (Hero / Courier / Stash) به صورت Server-Authoritative
pub fn try_buy_item(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    item_id: &str,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    if let Some(target_id) = parse_recipe_target(item_id) {
        return try_buy_recipe(world, slot_index, target_id, config, items_cfg);
    }

    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    let Some(item_def) = items_cfg.get_item(item_id) else {
        return false;
    };
    let hero_cfg = config.active_hero(p.hero_index);
    let courier_max_slots = hero_cfg.courier.inventory_slots;
    let hero_in_shop =
        !p.is_dead && fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) > 0.0;
    let courier_in_shop = !p.courier_is_dead
        && fountain_regen_bonus_for_pos(config, p.hero_team, p.courier_pos) > 0.0;

    // قانون ۱: هیرو در محدوده شاپ -> اول هیرو، اگر پر بود Stash
    if hero_in_shop {
        if item_def.dedicated_tp_slot {
            if p.hero_gold < item_def.cost {
                set_shop_notice(p, "NOT ENOUGH GOLD", true);
                return false;
            }
            if let Some(ref mut tp_item) = p.tp_slot {
                if tp_item.item_id == item_id && item_def.stackable {
                    p.hero_gold -= item_def.cost;
                    tp_item.count = tp_item.count.saturating_add(1);
                    return true;
                }
            } else {
                p.hero_gold -= item_def.cost;
                p.tp_slot = Some(InventoryItemState {
                    item_id: item_id.to_string(),
                    active_mode: 0,
                    slot: 0,
                    count: 1,
                });
                return true;
            }
        } else {
            match try_craft_buy_into_target(
                &mut p.hero_gold,
                &mut p.inventory,
                items_cfg.max_inventory_slots,
                Some(&mut p.stash),
                item_id,
                items_cfg,
            ) {
                Ok(true) => {
                    recompute_player_slot_stats(p, config, items_cfg);
                    return true;
                }
                Err(msg) => {
                    set_shop_notice(p, msg, true);
                    return false;
                }
                Ok(false) => {} // اینونتوری هیرو پر است -> ورود به Stash
            }
        }

        // هیرو پر بود -> تلاش برای قرار دادن در Stash
        match try_craft_buy_into_target(
            &mut p.hero_gold,
            &mut p.stash,
            items_cfg.max_stash_slots,
            None,
            item_id,
            items_cfg,
        ) {
            Ok(true) => return true,
            Err(msg) => {
                set_shop_notice(p, msg, true);
                return false;
            }
            Ok(false) => {
                set_shop_notice(p, "HERO & STASH FULL - CANNOT BUY", true);
                return false;
            }
        }
    }

    // قانون ۲: هیرو خارج از شاپ، کوریر داخل شاپ -> اول کوریر، اگر پر بود Stash
    if courier_in_shop {
        match try_craft_buy_into_target(
            &mut p.hero_gold,
            &mut p.courier_inventory,
            courier_max_slots,
            Some(&mut p.stash),
            item_id,
            items_cfg,
        ) {
            Ok(true) => return true,
            Err(msg) => {
                set_shop_notice(p, msg, true);
                return false;
            }
            Ok(false) => {} // کوریر پر است -> ورود به Stash
        }

        match try_craft_buy_into_target(
            &mut p.hero_gold,
            &mut p.stash,
            items_cfg.max_stash_slots,
            None,
            item_id,
            items_cfg,
        ) {
            Ok(true) => return true,
            Err(msg) => {
                set_shop_notice(p, msg, true);
                return false;
            }
            Ok(false) => {
                set_shop_notice(p, "COURIER & STASH FULL - CANNOT BUY", true);
                return false;
            }
        }
    }

    // قانون ۳: هم هیرو و هم کوریر خارج از شاپ -> خرید مستقیم به Stash
    match try_craft_buy_into_target(
        &mut p.hero_gold,
        &mut p.stash,
        items_cfg.max_stash_slots,
        None,
        item_id,
        items_cfg,
    ) {
        Ok(true) => true,
        Err(msg) => {
            set_shop_notice(p, msg, true);
            false
        }
        Ok(false) => {
            set_shop_notice(p, "STASH FULL - CANNOT BUY", true);
            false
        }
    }
}

/// بارگیری خودکار آیتم‌های داخل Stash به ترتیب اسلات در اینونتوری کوریر (تا سقف ظرفیت کوریر)
pub fn pull_stash_into_courier(
    p: &mut PlayerSlotState,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    if p.courier_is_dead || p.stash.is_empty() {
        return false;
    }
    let courier_max_slots = config.active_hero(p.hero_index).courier.inventory_slots;
    p.stash.sort_by_key(|it| it.slot);
    let mut moved_any = false;
    let mut remaining_stash = Vec::new();

    for st_item in p.stash.drain(..) {
        if try_insert_into_inventory(
            &mut p.courier_inventory,
            courier_max_slots,
            &st_item.item_id,
            st_item.count,
            items_cfg,
        ) {
            moved_any = true;
        } else {
            remaining_stash.push(st_item);
        }
    }
    p.stash = remaining_stash;
    moved_any
}

/// انتقال دستی آیتم از اینونتوری هیرو به Stash (فقط وقتی هیرو زنده و داخل چشمه است)
pub fn try_move_hero_item_to_stash(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    hero_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead || fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) <= 0.0 {
        set_shop_notice(p, "MUST BE IN SHOP TO MOVE TO STASH", true);
        return false;
    }
    let Some(pos) = p.inventory.iter().position(|it| it.slot == hero_slot) else {
        return false;
    };
    let item = p.inventory[pos].clone();
    if try_insert_into_inventory(
        &mut p.stash,
        items_cfg.max_stash_slots,
        &item.item_id,
        item.count,
        items_cfg,
    ) {
        p.inventory.remove(pos);
        recompute_player_slot_stats(p, config, items_cfg);
        return true;
    }
    set_shop_notice(p, "STASH IS FULL", true);
    false
}

/// انتقال دستی آیتم از Stash به اینونتوری هیرو (فقط وقتی هیرو زنده و داخل چشمه است)
pub fn try_move_stash_item_to_hero(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    stash_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead || fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) <= 0.0 {
        set_shop_notice(p, "MUST BE IN SHOP TO TAKE FROM STASH", true);
        return false;
    }
    let Some(pos) = p.stash.iter().position(|it| it.slot == stash_slot) else {
        return false;
    };
    let item = p.stash[pos].clone();
    let is_tp = items_cfg
        .get_item(&item.item_id)
        .is_some_and(|d| d.dedicated_tp_slot);
    if is_tp {
        if let Some(ref mut tp) = p.tp_slot {
            if tp.item_id == item.item_id {
                tp.count = tp.count.saturating_add(item.count);
                p.stash.remove(pos);
                return true;
            }
        } else {
            p.tp_slot = Some(InventoryItemState {
                item_id: item.item_id,
                active_mode: 0,
                slot: 0,
                count: item.count,
            });
            p.stash.remove(pos);
            return true;
        }
    }
    if try_insert_into_inventory(
        &mut p.inventory,
        items_cfg.max_inventory_slots,
        &item.item_id,
        item.count,
        items_cfg,
    ) {
        p.stash.remove(pos);
        recompute_player_slot_stats(p, config, items_cfg);
        return true;
    }
    set_shop_notice(p, "HERO INVENTORY IS FULL", true);
    false
}

/// انتقال همه آیتم‌های قابل‌جاگیری از Stash به هیرو (وقتی هیرو در چشمه است)
pub fn try_take_all_stash_to_hero(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead
        || p.stash.is_empty()
        || fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) <= 0.0
    {
        return false;
    }
    p.stash.sort_by_key(|it| it.slot);
    let mut moved_any = false;
    let mut remaining = Vec::new();
    for st_item in p.stash.drain(..) {
        let is_tp = items_cfg
            .get_item(&st_item.item_id)
            .is_some_and(|d| d.dedicated_tp_slot);
        let mut accepted = false;
        if is_tp {
            if let Some(ref mut tp) = p.tp_slot {
                if tp.item_id == st_item.item_id {
                    tp.count = tp.count.saturating_add(st_item.count);
                    accepted = true;
                }
            } else {
                p.tp_slot = Some(InventoryItemState {
                    item_id: st_item.item_id.clone(),
                    active_mode: 0,
                    slot: 0,
                    count: st_item.count,
                });
                accepted = true;
            }
        }
        if !accepted {
            accepted = try_insert_into_inventory(
                &mut p.inventory,
                items_cfg.max_inventory_slots,
                &st_item.item_id,
                st_item.count,
                items_cfg,
            );
        }
        if accepted {
            moved_any = true;
        } else {
            remaining.push(st_item);
        }
    }
    p.stash = remaining;
    if moved_any {
        recompute_player_slot_stats(p, config, items_cfg);
    } else {
        set_shop_notice(p, "HERO INVENTORY IS FULL", true);
    }
    moved_any
}

/// انتقال دستی آیتم از کوریر به Stash (فقط وقتی کوریر زنده و داخل چشمه است)
pub fn try_move_courier_item_to_stash(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    courier_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead
        || fountain_regen_bonus_for_pos(config, p.hero_team, p.courier_pos) <= 0.0
    {
        set_shop_notice(p, "COURIER MUST BE IN SHOP FOR STASH", true);
        return false;
    }
    let Some(pos) = p
        .courier_inventory
        .iter()
        .position(|it| it.slot == courier_slot)
    else {
        return false;
    };
    let item = p.courier_inventory[pos].clone();
    if try_insert_into_inventory(
        &mut p.stash,
        items_cfg.max_stash_slots,
        &item.item_id,
        item.count,
        items_cfg,
    ) {
        p.courier_inventory.remove(pos);
        return true;
    }
    set_shop_notice(p, "STASH IS FULL", true);
    false
}

/// انتقال دستی آیتم از Stash به کوریر (فقط وقتی کوریر زنده و داخل چشمه است)
pub fn try_move_stash_item_to_courier(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    stash_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead
        || fountain_regen_bonus_for_pos(config, p.hero_team, p.courier_pos) <= 0.0
    {
        set_shop_notice(p, "COURIER MUST BE IN SHOP FOR STASH", true);
        return false;
    }
    let courier_max_slots = config.active_hero(p.hero_index).courier.inventory_slots;
    let Some(pos) = p.stash.iter().position(|it| it.slot == stash_slot) else {
        return false;
    };
    let item = p.stash[pos].clone();
    if try_insert_into_inventory(
        &mut p.courier_inventory,
        courier_max_slots,
        &item.item_id,
        item.count,
        items_cfg,
    ) {
        p.stash.remove(pos);
        return true;
    }
    set_shop_notice(p, "COURIER INVENTORY IS FULL", true);
    false
}

/// فروش آیتم مستقیم از Stash (چون Stash همیشه داخل چشمه است)
pub fn try_sell_stash_item(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    stash_slot: usize,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    let Some(pos) = p.stash.iter().position(|it| it.slot == stash_slot) else {
        return false;
    };
    let refund = items_cfg.sell_price(&p.stash[pos].item_id);
    if p.stash[pos].count > 1 {
        p.stash[pos].count -= 1;
    } else {
        p.stash.remove(pos);
    }
    p.hero_gold = p.hero_gold.saturating_add(refund);
    true
}

/// خرید یکجای سبد خرید به صورت Server-Authoritative
pub fn try_buy_cart(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    item_ids: &[String],
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    if item_ids.is_empty() {
        return false;
    }
    let mut bought_any = false;
    for item_id in item_ids {
        if try_buy_item(world, slot_index, item_id, config, items_cfg) {
            bought_any = true;
        }
    }
    bought_any
}

/// فروش آیتم از اینونتوری (فقط در محدوده چشمه تیم خودی)
pub fn try_sell_item(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    inv_index: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead || fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) <= 0.0 {
        return false;
    }
    if inv_index == usize::MAX {
        let Some(ref mut tp_item) = p.tp_slot else {
            return false;
        };
        let refund = items_cfg.sell_price(&tp_item.item_id);
        if tp_item.count > 1 {
            tp_item.count -= 1;
        } else {
            p.tp_slot = None;
        }
        p.hero_gold = p.hero_gold.saturating_add(refund);
        return true;
    }
    let Some(pos) = p.inventory.iter().position(|it| it.slot == inv_index) else {
        return false;
    };
    let refund = items_cfg.sell_price(&p.inventory[pos].item_id);
    if p.inventory[pos].count > 1 {
        p.inventory[pos].count -= 1;
    } else {
        p.inventory.remove(pos);
    }
    p.hero_gold = p.hero_gold.saturating_add(refund);
    recompute_player_slot_stats(p, config, items_cfg);
    true
}

/// تغییر حالت آیتم‌های چندحالته (مثلاً Adaptive Combat Rig یا Duelist Dual-Core)
pub fn try_cycle_item_mode(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    inv_index: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead {
        return false;
    }
    let Some(inv_item) = p.inventory.iter_mut().find(|it| it.slot == inv_index) else {
        return false;
    };
    let Some(item_def) = items_cfg.get_item(&inv_item.item_id) else {
        return false;
    };
    if item_def.modes.is_empty() {
        return false;
    }
    inv_item.active_mode = (inv_item.active_mode + 1) % item_def.modes.len();
    recompute_player_slot_stats(p, config, items_cfg);
    true
}

/// جابجایی یا تعویض جای آیتم‌ها در خانه‌های اینونتوری (Drag & Drop)
pub fn try_swap_inventory_slots(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    from_slot: usize,
    to_slot: usize,
    items_cfg: &ItemsConfig,
) -> bool {
    if from_slot >= items_cfg.max_inventory_slots
        || to_slot >= items_cfg.max_inventory_slots
        || from_slot == to_slot
    {
        return false;
    }
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    let has_any = p
        .inventory
        .iter()
        .any(|it| it.slot == from_slot || it.slot == to_slot);
    if !has_any {
        return false;
    }
    for it in &mut p.inventory {
        if it.slot == from_slot {
            it.slot = to_slot;
        } else if it.slot == to_slot {
            it.slot = from_slot;
        }
    }
    true
}


/// فروش آیتم از اینونتوری Courier (وقتی Courier زنده و داخل چشمه است)
pub fn try_sell_courier_item(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    inv_index: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead
        || fountain_regen_bonus_for_pos(config, p.hero_team, p.courier_pos) <= 0.0
    {
        return false;
    }
    let Some(pos) = p.courier_inventory.iter().position(|it| it.slot == inv_index) else {
        return false;
    };
    let removed = p.courier_inventory.remove(pos);
    let refund = items_cfg.sell_price(&removed.item_id);
    p.hero_gold = p.hero_gold.saturating_add(refund);
    true
}

pub fn try_courier_deliver(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead {
        return false;
    }
    let courier_in_shop =
        fountain_regen_bonus_for_pos(config, p.hero_team, p.courier_pos) > 0.0;
    if courier_in_shop {
        pull_stash_into_courier(p, config, items_cfg);
        p.courier_order = CourierOrder::DeliverToHero;
    } else if p.courier_inventory.is_empty() && !p.stash.is_empty() {
        p.courier_order = CourierOrder::PickupStashThenDeliver;
    } else {
        p.courier_order = CourierOrder::DeliverToHero;
    }
    true
}

pub fn try_courier_return_to_fountain(world: &mut AuthoritativeWorldState, slot_index: usize) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead {
        return false;
    }
    p.courier_order = CourierOrder::ReturnToFountain;
    true
}

pub fn try_courier_move_to(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    world_pos: Vec3,
    config: &GameConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead {
        return false;
    }
    let h_cfg = config.active_hero(p.hero_index);
    let half_x = config.map.map_dimensions.0 * 0.5;
    let half_z = config.map.map_dimensions.1 * 0.5;
    let clamped_x = world_pos.x.clamp(-half_x, half_x);
    let clamped_z = world_pos.z.clamp(-half_z, half_z);
    p.courier_target_pos = Vec3::new(clamped_x, h_cfg.courier.fly_height, clamped_z);
    p.courier_order = CourierOrder::MoveTo;
    true
}

/// انتقال یک آیتم مشخص از هیرو به کوریر (هیرو به سمت کوریر حرکت می‌کند و پس از رسیدن تحویل می‌دهد)
pub fn try_transfer_hero_item_to_courier(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    hero_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead || p.courier_is_dead || p.active_teleport.is_some() {
        return false;
    }
    let has_item = if hero_slot == usize::MAX {
        p.tp_slot.as_ref().is_some_and(|it| it.count > 0)
    } else {
        p.inventory.iter().any(|it| it.slot == hero_slot)
    };
    if !has_item {
        return false;
    }

    let dist_xz = Vec2::new(
        p.courier_pos.x - p.soldier_pos.x,
        p.courier_pos.z - p.soldier_pos.z,
    )
    .length();
    if dist_xz <= items_cfg.shop_ui.hero_to_courier_transfer_radius {
        p.pending_hero_to_courier_slot = None;
        return execute_hero_slot_to_courier_transfer(p, hero_slot, config, items_cfg);
    }

    p.pending_hero_to_courier_slot = Some(hero_slot);
    true
}

/// انتقال یک آیتم مشخص از کوریر به هیرو (کوریر به سمت هیرو پرواز می‌کند و فقط همین آیتم را تحویل می‌دهد)
pub fn try_transfer_courier_item_to_hero(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    courier_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead || p.courier_is_dead {
        return false;
    }
    if !p.courier_inventory.iter().any(|it| it.slot == courier_slot) {
        return false;
    }

    let h_cfg = config.active_hero(p.hero_index);
    let dist_xz = Vec2::new(
        p.soldier_pos.x - p.courier_pos.x,
        p.soldier_pos.z - p.courier_pos.z,
    )
    .length();
    if dist_xz <= h_cfg.courier.deliver_radius {
        let transferred = execute_courier_slot_to_hero_transfer(p, courier_slot, config, items_cfg);
        if !transferred {
            set_shop_notice(p, "HERO INVENTORY FULL", true);
        }
        p.courier_order = CourierOrder::Idle;
        return transferred;
    }

    p.courier_order = CourierOrder::DeliverSlotToHero { courier_slot };
    true
}

/// اجرای واقعی تحویل یک اسلات از هیرو به کوریر با مدیریت کامل حالت‌های پر بودن اینونتوری کوریر
pub fn execute_hero_slot_to_courier_transfer(
    p: &mut PlayerSlotState,
    hero_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    if p.is_dead || p.courier_is_dead {
        return false;
    }
    let h_cfg = config.active_hero(p.hero_index);
    let courier_max_slots = h_cfg.courier.inventory_slots;

    // استخراج موقت آیتم از اسلات هیرو (بدون حذف دائمی تا زمانی که پذیرش در کوریر قطعی شود)
    let source_item = if hero_slot == usize::MAX {
        let Some(ref tp) = p.tp_slot else {
            return false;
        };
        tp.clone()
    } else {
        let Some(it) = p.inventory.iter().find(|it| it.slot == hero_slot) else {
            return false;
        };
        it.clone()
    };

    let def_opt = items_cfg.get_item(&source_item.item_id);
    let is_stack = def_opt.is_some_and(|d| d.stackable);

    // حالت ۱: آیتم استک‌پذیر است و کوریر قبلاً از همین نوع دارد -> ادغام استک
    if is_stack {
        if let Some(existing) = p
            .courier_inventory
            .iter_mut()
            .find(|it| it.item_id == source_item.item_id)
        {
            existing.count = existing.count.saturating_add(source_item.count);
            if hero_slot == usize::MAX {
                p.tp_slot = None;
            } else if let Some(pos) = p.inventory.iter().position(|it| it.slot == hero_slot) {
                p.inventory.remove(pos);
                recompute_player_slot_stats(p, config, items_cfg);
            }
            return true;
        }
    }

    // حالت ۲: کوریر اسلات خالی دارد -> انتقال مستقیم + ترکیب خودکار
    if let Some(free_slot) = (0..courier_max_slots)
        .find(|s| !p.courier_inventory.iter().any(|it| it.slot == *s))
    {
        let mut moved = source_item;
        moved.slot = free_slot;
        p.courier_inventory.push(moved);
        if hero_slot == usize::MAX {
            p.tp_slot = None;
        } else if let Some(pos) = p.inventory.iter().position(|it| it.slot == hero_slot) {
            p.inventory.remove(pos);
            recompute_player_slot_stats(p, config, items_cfg);
        }
        auto_combine_items(&mut p.courier_inventory, courier_max_slots, items_cfg);
        return true;
    }

    // حالت ۳: اینونتوری کوریر پر است (۶/۶) اما اضافه شدن این آیتم باعث ترکیب خودکار می‌شود
    let mut test_cour = p.courier_inventory.clone();
    let mut moved_test = source_item.clone();
    moved_test.slot = courier_max_slots;
    test_cour.push(moved_test);
    if auto_combine_items(&mut test_cour, courier_max_slots, items_cfg)
        && test_cour.len() <= courier_max_slots
    {
        p.courier_inventory = test_cour;
        if hero_slot == usize::MAX {
            p.tp_slot = None;
        } else if let Some(pos) = p.inventory.iter().position(|it| it.slot == hero_slot) {
            p.inventory.remove(pos);
            recompute_player_slot_stats(p, config, items_cfg);
        }
        return true;
    }

    // حالت ۴: اینونتوری کوریر پر است و قابل ترکیب نیست -> اگر از اسلات معمولی هیرو آمده، با آخرین آیتم غیر-TP کوریر جابجا (Swap) کن تا آیتمی گم نشود
    if hero_slot != usize::MAX {
        if let Some(swap_pos) = p.courier_inventory.iter().rposition(|cit| {
            items_cfg
                .get_item(&cit.item_id)
                .is_none_or(|d| !d.dedicated_tp_slot)
        }) {
            if let Some(hero_pos) = p.inventory.iter().position(|it| it.slot == hero_slot) {
                let mut cour_item = p.courier_inventory.remove(swap_pos);
                let cour_slot_num = cour_item.slot;
                let mut hero_item = p.inventory.remove(hero_pos);
                hero_item.slot = cour_slot_num;
                cour_item.slot = hero_slot;
                p.courier_inventory.push(hero_item);
                p.inventory.push(cour_item);
                auto_combine_items(&mut p.courier_inventory, courier_max_slots, items_cfg);
                auto_combine_items(&mut p.inventory, items_cfg.max_inventory_slots, items_cfg);
                recompute_player_slot_stats(p, config, items_cfg);
                return true;
            }
        }
    }

    // اگر اسلات TP بود و کوریر کاملاً پر و غیرقابل ادغام بود -> لغو امن بدون از دست رفتن آیتم
    false
}

/// اجرای واقعی تحویل یک اسلات مشخص از کوریر به هیرو با مدیریت کامل حالت‌های پر بودن اینونتوری هیرو
pub fn execute_courier_slot_to_hero_transfer(
    p: &mut PlayerSlotState,
    courier_slot: usize,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    if p.is_dead || p.courier_is_dead {
        return false;
    }
    let Some(cour_pos) = p
        .courier_inventory
        .iter()
        .position(|it| it.slot == courier_slot)
    else {
        return false;
    };
    let mut c_item = p.courier_inventory[cour_pos].clone();
    let def_opt = items_cfg.get_item(&c_item.item_id);
    let is_tp = def_opt.is_some_and(|d| d.dedicated_tp_slot);
    let is_stack = def_opt.is_some_and(|d| d.stackable);

    // حالت ۱: آیتم مخصوص اسلات تلپورت
    if is_tp {
        if let Some(ref mut tp_item) = p.tp_slot {
            if tp_item.item_id == c_item.item_id && is_stack {
                tp_item.count = tp_item.count.saturating_add(c_item.count);
                p.courier_inventory.remove(cour_pos);
                return true;
            }
            return false;
        } else {
            c_item.slot = 0;
            p.tp_slot = Some(c_item);
            p.courier_inventory.remove(cour_pos);
            return true;
        }
    }

    // حالت ۲: استک‌پذیر و موجود در هیرو
    if is_stack {
        if let Some(existing) = p.inventory.iter_mut().find(|it| it.item_id == c_item.item_id) {
            existing.count = existing.count.saturating_add(c_item.count);
            p.courier_inventory.remove(cour_pos);
            return true;
        }
    }

    // حالت ۳: هیرو اسلات خالی دارد
    if let Some(free_slot) = (0..items_cfg.max_inventory_slots)
        .find(|s| !p.inventory.iter().any(|it| it.slot == *s))
    {
        c_item.slot = free_slot;
        p.inventory.push(c_item);
        p.courier_inventory.remove(cour_pos);
        auto_combine_items(&mut p.inventory, items_cfg.max_inventory_slots, items_cfg);
        recompute_player_slot_stats(p, config, items_cfg);
        return true;
    }

    // حالت ۴: هیرو ۶/۶ پر است اما این آیتم باعث ترکیب خودکار در هیرو می‌شود
    let mut test_inv = p.inventory.clone();
    let mut test_item = c_item.clone();
    test_item.slot = items_cfg.max_inventory_slots;
    test_inv.push(test_item);
    if auto_combine_items(&mut test_inv, items_cfg.max_inventory_slots, items_cfg)
        && test_inv.len() <= items_cfg.max_inventory_slots
    {
        p.inventory = test_inv;
        p.courier_inventory.remove(cour_pos);
        recompute_player_slot_stats(p, config, items_cfg);
        return true;
    }

    // حالت ۵: هیرو ۶/۶ پر است و قابل ترکیب نیست -> آیتم به صورت امن در کوریر باقی می‌ماند
    false
}

/// شبیه‌سازی حرکت، تحویل آیتم و ریسپاون Courierها (پرواز در ارتفاع بالا بدون برخورد با موانع)
pub fn step_couriers_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_cfg: Res<ItemsConfig>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    for p in &mut world.players {
        let h_cfg = config.active_hero(p.hero_index);
        let fly_y = h_cfg.courier.fly_height;

        if p.is_dead || p.courier_is_dead || p.active_teleport.is_some() {
            p.pending_hero_to_courier_slot = None;
        } else if let Some(hero_slot) = p.pending_hero_to_courier_slot {
            let has_item = if hero_slot == usize::MAX {
                p.tp_slot.as_ref().is_some_and(|it| it.count > 0)
            } else {
                p.inventory.iter().any(|it| it.slot == hero_slot)
            };
            if !has_item {
                p.pending_hero_to_courier_slot = None;
            } else {
                let dist_xz = Vec2::new(
                    p.courier_pos.x - p.soldier_pos.x,
                    p.courier_pos.z - p.soldier_pos.z,
                )
                .length();
                if dist_xz <= items_cfg.shop_ui.hero_to_courier_transfer_radius {
                    p.pending_hero_to_courier_slot = None;
                    execute_hero_slot_to_courier_transfer(p, hero_slot, &config, &items_cfg);
                }
            }
        }

        if p.courier_is_dead {
            p.courier_respawn_remaining -= dt;
            if p.courier_respawn_remaining <= 0.0 {
                p.courier_is_dead = false;
                p.courier_respawn_remaining = 0.0;
                p.courier_hp = p.courier_max_hp;
                p.courier_pos = Vec3::new(p.spawn_pos.x, fly_y, p.spawn_pos.z);
                p.courier_order = CourierOrder::Idle;
            }
            continue;
        }

        let target_xz = match p.courier_order {
            CourierOrder::Idle => None,
            CourierOrder::ReturnToFountain | CourierOrder::PickupStashThenDeliver => {
                Some(Vec3::new(p.spawn_pos.x, fly_y, p.spawn_pos.z))
            }
            CourierOrder::MoveTo => Some(Vec3::new(p.courier_target_pos.x, fly_y, p.courier_target_pos.z)),
            CourierOrder::DeliverToHero | CourierOrder::DeliverSlotToHero { .. } => {
                if p.is_dead {
                    p.courier_order = CourierOrder::ReturnToFountain;
                    Some(Vec3::new(p.spawn_pos.x, fly_y, p.spawn_pos.z))
                } else {
                    Some(Vec3::new(p.soldier_pos.x, fly_y, p.soldier_pos.z))
                }
            }
        };

        if let Some(target) = target_xz {
            let delta = Vec3::new(target.x - p.courier_pos.x, 0.0, target.z - p.courier_pos.z);
            let dist = delta.length();
            let step = h_cfg.courier.move_speed * dt;

            if dist > 0.05 {
                let dir = delta / dist;
                p.courier_facing_dir = dir;
                if dist <= step {
                    p.courier_pos.x = target.x;
                    p.courier_pos.z = target.z;
                } else {
                    p.courier_pos.x += dir.x * step;
                    p.courier_pos.z += dir.z * step;
                }
            }
            p.courier_pos.y = fly_y;

            let new_dist = Vec3::new(target.x - p.courier_pos.x, 0.0, target.z - p.courier_pos.z).length();
            match p.courier_order {
                CourierOrder::DeliverToHero if !p.is_dead && new_dist <= h_cfg.courier.deliver_radius => {
                    let mut transferred_any = false;
                    loop {
                        let mut pass_transferred = false;
                        let mut remaining = Vec::new();
                        for mut c_item in p.courier_inventory.drain(..) {
                            let def_opt = items_cfg.get_item(&c_item.item_id);
                            let is_tp = def_opt.is_some_and(|d| d.dedicated_tp_slot);
                            let is_stack = def_opt.is_some_and(|d| d.stackable);
                            if is_tp {
                                if let Some(ref mut tp_item) = p.tp_slot {
                                    if tp_item.item_id == c_item.item_id && is_stack {
                                        tp_item.count = tp_item.count.saturating_add(c_item.count);
                                        pass_transferred = true;
                                    } else {
                                        remaining.push(c_item);
                                    }
                                } else {
                                    c_item.slot = 0;
                                    p.tp_slot = Some(c_item);
                                    pass_transferred = true;
                                }
                            } else if is_stack {
                                if let Some(existing) = p.inventory.iter_mut().find(|it| it.item_id == c_item.item_id) {
                                    existing.count = existing.count.saturating_add(c_item.count);
                                    pass_transferred = true;
                                } else if let Some(free_slot) = (0..items_cfg.max_inventory_slots)
                                    .find(|s| !p.inventory.iter().any(|it| it.slot == *s))
                                {
                                    c_item.slot = free_slot;
                                    p.inventory.push(c_item);
                                    pass_transferred = true;
                                } else {
                                    remaining.push(c_item);
                                }
                            } else if let Some(free_slot) = (0..items_cfg.max_inventory_slots)
                                .find(|s| !p.inventory.iter().any(|it| it.slot == *s))
                            {
                                c_item.slot = free_slot;
                                p.inventory.push(c_item);
                                pass_transferred = true;
                            } else {
                                // اگر اینونتوری ۶/۶ پر است اما این آیتم با اجزای داخل اینونتوری ترکیب می‌شود
                                // نکته حیاتی: هرگز c_item.slot را تغییر ندهیم تا در صورت عدم ترکیب، آیتم در اسلات کوریر مخفی نشود
                                let mut test_inv = p.inventory.clone();
                                let mut test_item = c_item.clone();
                                test_item.slot = items_cfg.max_inventory_slots;
                                test_inv.push(test_item);
                                if auto_combine_items(&mut test_inv, items_cfg.max_inventory_slots, &items_cfg)
                                    && test_inv.len() <= items_cfg.max_inventory_slots
                                {
                                    p.inventory = test_inv;
                                    pass_transferred = true;
                                } else {
                                    remaining.push(c_item);
                                }
                            }
                        }
                        p.courier_inventory = remaining;
                        if pass_transferred {
                            transferred_any = true;
                        }
                        if !pass_transferred || p.courier_inventory.is_empty() {
                            break;
                        }
                    }
                    let crafted_any = auto_combine_items(
                        &mut p.inventory,
                        items_cfg.max_inventory_slots,
                        &items_cfg,
                    );
                    if transferred_any || crafted_any {
                        recompute_player_slot_stats(p, &config, &items_cfg);
                    }
                    if !p.courier_inventory.is_empty() {
                        set_shop_notice(p, "HERO INVENTORY FULL - COURIER RETURNING", true);
                    }
                    p.courier_order = CourierOrder::ReturnToFountain;
                }
                CourierOrder::DeliverSlotToHero { courier_slot }
                    if !p.is_dead && new_dist <= h_cfg.courier.deliver_radius =>
                {
                    let transferred = execute_courier_slot_to_hero_transfer(
                        p,
                        courier_slot,
                        &config,
                        &items_cfg,
                    );
                    if transferred {
                        p.courier_order = CourierOrder::ReturnToFountain;
                    } else {
                        // اینونتوری هیرو ۶/۶ پر است و قابل ترکیب نیست: کوریر با آیتم به پایگاه برمی‌گردد تا آیتم محفوظ بماند
                        set_shop_notice(p, "HERO INVENTORY FULL - COURIER RETURNING", true);
                        p.courier_order = CourierOrder::ReturnToFountain;
                    }
                }
                CourierOrder::PickupStashThenDeliver
                    if new_dist <= h_cfg.courier.fountain_radius =>
                {
                    pull_stash_into_courier(p, &config, &items_cfg);
                    if !p.courier_inventory.is_empty() {
                        p.courier_order = CourierOrder::DeliverToHero;
                    } else {
                        p.courier_order = CourierOrder::Idle;
                    }
                }
                CourierOrder::ReturnToFountain if new_dist <= h_cfg.courier.fountain_radius => {
                    p.courier_order = CourierOrder::Idle;
                }
                CourierOrder::MoveTo if new_dist <= 0.6 => {
                    p.courier_order = CourierOrder::Idle;
                }
                _ => {}
            }
        } else {
            p.courier_pos.y = fly_y;
        }
    }
}

/// پیدا کردن نقطه فرود معتبر تلپورت در شعاع مشخصی از آبجکت‌های تیم خودی (تاورها، باراکس، انشنت، چشمه)
pub fn resolve_teleport_destination(
    config: &GameConfig,
    items_cfg: &ItemsConfig,
    turrets: &[TurretState],
    barracks: &[BarracksState],
    ancients: &[AncientState],
    team: TeamId,
    requested_pos: Vec3,
    from_minimap: bool,
) -> Option<Vec3> {
    if team == TeamId::Neutral {
        return None;
    }

    let mut best: Option<(f32, Vec3, f32, bool)> = None;

    for t in turrets {
        if t.team != team || t.is_dead {
            continue;
        }
        let t_def = &config.map.turrets[t.index];
        let s = t_def.model_transform.scale_vec();
        let rad = t_def.base_radius * s.x.max(s.z);
        let d = Vec2::new(requested_pos.x - t.pos.x, requested_pos.z - t.pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, t.pos, rad, false));
        }
    }

    for b in barracks {
        if b.team != team || b.is_dead {
            continue;
        }
        let b_def = &config.map.barracks[b.index];
        let s = b_def.model_transform.scale_vec();
        let rad = 0.5 * (b_def.size.0 * s.x).max(b_def.size.2 * s.z);
        let d = Vec2::new(requested_pos.x - b.pos.x, requested_pos.z - b.pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, b.pos, rad, false));
        }
    }

    for a in ancients {
        if a.team != team || a.is_dead {
            continue;
        }
        let a_def = &config.map.ancients[a.index];
        let s = a_def.model_transform.scale_vec();
        let rad = 0.5 * (a_def.base_size.0 * s.x).max(a_def.base_size.2 * s.z);
        let d = Vec2::new(requested_pos.x - a.pos.x, requested_pos.z - a.pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, a.pos, rad, false));
        }
    }

    for ftn in &config.map.fountains {
        if ftn.team != team {
            continue;
        }
        let f_pos = ftn.model_transform.location_vec();
        let d = Vec2::new(requested_pos.x - f_pos.x, requested_pos.z - f_pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, f_pos, 0.0, true));
        }
    }

    let (dist_xz, b_pos, b_rad, is_fountain) = best?;
    if !from_minimap && dist_xz > items_cfg.teleport.max_click_snap_distance {
        return None;
    }

    let raw_dir = Vec2::new(requested_pos.x - b_pos.x, requested_pos.z - b_pos.z);
    let dir_xz = if raw_dir.length_squared() > 1e-4 {
        raw_dir.normalize()
    } else {
        let to_center = Vec2::new(-b_pos.x, -b_pos.z);
        if to_center.length_squared() > 1e-4 {
            to_center.normalize()
        } else {
            Vec2::X
        }
    };

    let min_clear = if is_fountain {
        0.0
    } else {
        items_cfg
            .teleport
            .min_building_clearance
            .max(b_rad + config.soldier.player_radius + 0.35)
    };
    let max_r = items_cfg.teleport.building_teleport_radius.max(min_clear);
    let clamped_r = dist_xz.clamp(min_clear, max_r);
    let final_x = b_pos.x + dir_xz.x * clamped_r;
    let final_z = b_pos.z + dir_xz.y * clamped_r;
    let final_y = sample_camera_focus_y(config, final_x, final_z);

    Some(Vec3::new(final_x, final_y, final_z))
}


/// نسخه مشترک محاسبه مقصد تلپورت برای پیش‌نمایش کلاینت از روی Snapshotها (بدون تکرار منطق)
pub fn resolve_teleport_destination_from_snapshots(
    config: &GameConfig,
    items_cfg: &ItemsConfig,
    turrets: &[TurretSnapshot],
    barracks: &[BarracksSnapshot],
    ancients: &[AncientSnapshot],
    team: TeamId,
    requested_pos: Vec3,
    from_minimap: bool,
) -> Option<Vec3> {
    if team == TeamId::Neutral {
        return None;
    }

    let mut best: Option<(f32, Vec3, f32, bool)> = None;

    for t in turrets {
        if t.team != team || t.is_dead {
            continue;
        }
        let Some(t_def) = config.map.turrets.get(t.index) else {
            continue;
        };
        let t_pos = t_def.model_transform.location_vec();
        let s = t_def.model_transform.scale_vec();
        let rad = t_def.base_radius * s.x.max(s.z);
        let d = Vec2::new(requested_pos.x - t_pos.x, requested_pos.z - t_pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, t_pos, rad, false));
        }
    }

    for b in barracks {
        if b.team != team || b.is_dead {
            continue;
        }
        let Some(b_def) = config.map.barracks.get(b.index) else {
            continue;
        };
        let b_pos = b_def.model_transform.location_vec();
        let s = b_def.model_transform.scale_vec();
        let rad = 0.5 * (b_def.size.0 * s.x).max(b_def.size.2 * s.z);
        let d = Vec2::new(requested_pos.x - b_pos.x, requested_pos.z - b_pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, b_pos, rad, false));
        }
    }

    for a in ancients {
        if a.team != team || a.is_dead {
            continue;
        }
        let Some(a_def) = config.map.ancients.get(a.index) else {
            continue;
        };
        let a_pos = a_def.model_transform.location_vec();
        let s = a_def.model_transform.scale_vec();
        let rad = 0.5 * (a_def.base_size.0 * s.x).max(a_def.base_size.2 * s.z);
        let d = Vec2::new(requested_pos.x - a_pos.x, requested_pos.z - a_pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, a_pos, rad, false));
        }
    }

    for ftn in &config.map.fountains {
        if ftn.team != team {
            continue;
        }
        let f_pos = ftn.model_transform.location_vec();
        let d = Vec2::new(requested_pos.x - f_pos.x, requested_pos.z - f_pos.z).length();
        if best.map_or(true, |(bd, _, _, _)| d < bd) {
            best = Some((d, f_pos, 0.0, true));
        }
    }

    let (dist_xz, b_pos, b_rad, is_fountain) = best?;
    if !from_minimap && dist_xz > items_cfg.teleport.max_click_snap_distance {
        return None;
    }

    let raw_dir = Vec2::new(requested_pos.x - b_pos.x, requested_pos.z - b_pos.z);
    let dir_xz = if raw_dir.length_squared() > 1e-4 {
        raw_dir.normalize()
    } else {
        let to_center = Vec2::new(-b_pos.x, -b_pos.z);
        if to_center.length_squared() > 1e-4 {
            to_center.normalize()
        } else {
            Vec2::X
        }
    };

    let min_clear = if is_fountain {
        0.0
    } else {
        items_cfg
            .teleport
            .min_building_clearance
            .max(b_rad + config.soldier.player_radius + 0.35)
    };
    let max_r = items_cfg.teleport.building_teleport_radius.max(min_clear);
    let clamped_r = dist_xz.clamp(min_clear, max_r);
    let final_x = b_pos.x + dir_xz.x * clamped_r;
    let final_z = b_pos.z + dir_xz.y * clamped_r;
    let final_y = sample_camera_focus_y(config, final_x, final_z);

    Some(Vec3::new(final_x, final_y, final_z))
}

/// شروع تلپورت توسط سرور (بررسی مالکیت آیتم، کولدان ۶۰ ثانیه و اعتبار مقصد)
pub fn try_start_teleport(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    inv_index: usize,
    target_pos: Vec3,
    from_minimap: bool,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
    physics_world: &CustomPhysicsWorld,
) -> bool {
    if world.winning_team.is_some() {
        return false;
    }
    let Some(p) = world.players.get(slot_index) else {
        return false;
    };
    if p.is_dead || p.traversal.is_active || p.wall_climb_mountain_index.is_some() {
        return false;
    }
    if p.teleport_cooldown_remaining > 0.0 || p.active_teleport.is_some() {
        return false;
    }
    let tp_item_opt = p
        .tp_slot
        .as_ref()
        .filter(|it| it.count > 0)
        .or_else(|| p.inventory.iter().find(|it| it.slot == inv_index && it.count > 0));
    let Some(inv_item) = tp_item_opt else {
        return false;
    };
    let Some(item_def) = items_cfg.get_item(&inv_item.item_id) else {
        return false;
    };
    if item_def.active_ability != ItemActiveAbility::Teleport {
        return false;
    }

    let Some(mut dest_pos) = resolve_teleport_destination(
        config,
        items_cfg,
        &world.turrets,
        &world.barracks,
        &world.ancients,
        p.hero_team,
        target_pos,
        from_minimap,
    ) else {
        return false;
    };

    if let Some((_, hit)) = physics_world.cast_ray(
        Vec3::new(dest_pos.x, dest_pos.y + 12.0, dest_pos.z),
        Vec3::NEG_Y,
        24.0,
        soldier_collision_groups(),
        |_| true,
    ) {
        if hit.normal.y > 0.5 {
            dest_pos.y = hit.point.y;
        }
    }

    let origin_pos = p.soldier_pos;
    let facing = Vec3::new(dest_pos.x - origin_pos.x, 0.0, dest_pos.z - origin_pos.z)
        .normalize_or_zero();
    let facing_dir = if facing == Vec3::ZERO {
        p.soldier_facing_dir
    } else {
        facing
    };

    if let Some(p_mut) = world.players.get_mut(slot_index) {
        p_mut.active_teleport = Some(ActiveTeleportState {
            inv_index,
            origin_pos,
            dest_pos,
            facing_dir,
            elapsed_secs: 0.0,
            total_secs: items_cfg.teleport.channel_duration_secs,
            cancel_on_left_click: items_cfg.teleport.cancel_on_left_click,
            cancel_on_move: items_cfg.teleport.cancel_on_move,
        });
        p_mut.reload_remaining = None;
        p_mut.firing_anim_remaining = None;
        p_mut.active_beam_request = None;
    }

    world.pending_fx.push(GameplayFxEvent::TeleportStarted {
        slot_index,
        origin_pos,
        dest_pos,
    });
    true
}

/// لغو تلپورت فعال (مثلاً با کلیک چپ یا تیراندازی)
pub fn try_cancel_teleport(world: &mut AuthoritativeWorldState, slot_index: usize) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    let Some(tp) = p.active_teleport.take() else {
        return false;
    };
    world.pending_fx.push(GameplayFxEvent::TeleportCanceled {
        slot_index,
        origin_pos: tp.origin_pos,
    });
    true
}

/// به‌روزرسانی تایمر کولدان و چنلینگ تلپورت در سرور (۶۰ هرتز)
pub fn step_teleports_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_cfg: Res<ItemsConfig>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut soldier_q: Query<(&PhysicsSoldier, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let mut completed_slots: Vec<(usize, Vec3, Vec3, usize)> = Vec::new();

    for p in &mut world.players {
        if p.teleport_cooldown_remaining > 0.0 {
            p.teleport_cooldown_remaining = (p.teleport_cooldown_remaining - dt).max(0.0);
        }

        if let Some(ref mut tp) = p.active_teleport {
            tp.elapsed_secs += dt;
            if tp.elapsed_secs >= tp.total_secs {
                let origin_pos = tp.origin_pos;
                let dest_pos = tp.dest_pos;
                let facing_dir = tp.facing_dir;
                let inv_idx = tp.inv_index;
                p.active_teleport = None;
                p.teleport_cooldown_remaining = items_cfg.teleport.cooldown_secs;
                p.soldier_pos = dest_pos;
                p.soldier_facing_dir = facing_dir;

                if items_cfg.teleport.consume_on_use {
                    if let Some(ref mut tp_item) = p.tp_slot {
                        if tp_item.count > 1 {
                            tp_item.count -= 1;
                        } else {
                            p.tp_slot = None;
                        }
                    } else if let Some(pos) = p.inventory.iter().position(|it| it.slot == inv_idx) {
                        if p.inventory[pos].count > 1 {
                            p.inventory[pos].count -= 1;
                        } else {
                            p.inventory.remove(pos);
                            recompute_player_slot_stats(p, &config, &items_cfg);
                        }
                    }
                }
                completed_slots.push((p.slot_index, origin_pos, dest_pos, inv_idx));
            }
        }
    }

    for (slot_idx, origin_pos, dest_pos, _) in completed_slots {
        for (ps, mut tf) in &mut soldier_q {
            if ps.slot_index == slot_idx {
                let scale = world.players.get(slot_idx).map(|p| p.hero_scale).unwrap_or(1.0);
                tf.translation = dest_pos
                    + Vec3::Y * (config.physics.soldier_capsule_center_y * scale);
                break;
            }
        }
        world.pending_fx.push(GameplayFxEvent::TeleportCompleted {
            slot_index: slot_idx,
            origin_pos,
            dest_pos,
        });
    }
}


/// فعال‌سازی آیتم Shadow Blade توسط سرور (غیب شدن هیرو + بونوس سرعت)
pub fn try_activate_shadow_blade(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    inv_index: usize,
    items_cfg: &ItemsConfig,
) -> bool {
    if world.winning_team.is_some() {
        return false;
    }
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.is_dead || p.shadow_blade_cooldown_remaining > 0.0 || p.invisibility_remaining > 0.0 {
        return false;
    }
    let Some(inv_item) = p.inventory.iter().find(|it| it.slot == inv_index) else {
        return false;
    };
    let Some(def) = items_cfg.get_item(&inv_item.item_id) else {
        return false;
    };
    if def.active_ability != ItemActiveAbility::ShadowBlade {
        return false;
    }
    let sb = &items_cfg.shadow_blade;
    p.invisibility_remaining = sb.duration_secs;
    p.invisibility_total_secs = sb.duration_secs;
    p.invis_move_speed_bonus_pct = sb.invis_move_speed_bonus_pct;
    p.shadow_blade_cooldown_remaining = sb.cooldown_secs;
    let pos = p.soldier_pos;
    world.pending_fx.push(GameplayFxEvent::ShadowBladeActivated { slot_index, pos });
    true
}

/// به‌روزرسانی تایمر غیبی، کولدان Shadow Blade و تشخیص دید واقعی (True Sight) توسط Turretهای دشمن
pub fn step_shadow_blade_system(
    time: Res<Time<Fixed>>,
    items_cfg: Res<ItemsConfig>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || world.winning_team.is_some() {
        return;
    }
    let true_sight_r = items_cfg.shadow_blade.turret_true_sight_radius;
    let mut ended_slots: Vec<(usize, Vec3)> = Vec::new();
    let turrets_snapshot: Vec<(TeamId, Vec3, bool)> = world
        .turrets
        .iter()
        .map(|t| (t.team, t.pos, t.is_dead))
        .collect();

    for p in &mut world.players {
        if p.shadow_blade_cooldown_remaining > 0.0 {
            p.shadow_blade_cooldown_remaining = (p.shadow_blade_cooldown_remaining - dt).max(0.0);
        }
        if p.invisibility_remaining > 0.0 {
            p.is_revealed_by_turret = turrets_snapshot.iter().any(|&(t_team, t_pos, t_dead)| {
                !t_dead
                    && t_team != p.hero_team
                    && Vec2::new(p.soldier_pos.x - t_pos.x, p.soldier_pos.z - t_pos.z).length()
                        <= true_sight_r
            });
            p.invisibility_remaining = (p.invisibility_remaining - dt).max(0.0);
            if p.invisibility_remaining <= 0.0 {
                p.invisibility_total_secs = 0.0;
                p.invis_move_speed_bonus_pct = 0.0;
                p.is_revealed_by_turret = false;
                ended_slots.push((p.slot_index, p.soldier_pos));
            }
        } else {
            p.is_revealed_by_turret = false;
        }
    }

    for (slot_index, pos) in ended_slots {
        world
            .pending_fx
            .push(GameplayFxEvent::ShadowBladeEnded { slot_index, pos });
    }
}