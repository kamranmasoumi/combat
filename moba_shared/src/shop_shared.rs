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

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct ItemsConfig {
    pub max_inventory_slots: usize,
    pub sell_refund_ratio: f32,
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
        assert!(
            (0.0..=1.0).contains(&self.sell_refund_ratio),
            "sell_refund_ratio باید بین 0.0 و 1.0 باشد"
        );
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
        self.get_item(item_id)
            .map(|it| ((it.cost as f32) * self.sell_refund_ratio).round() as u32)
            .unwrap_or(0)
    }

    /// محاسبه هزینه باقی‌مانده خرید یک آیتم با کسر قیمت کامپوننت‌های موجود در اینونتوری بازیکن
    pub fn effective_purchase_cost(
        &self,
        item_id: &str,
        inventory: &[InventoryItemState],
    ) -> Option<(u32, Vec<usize>)> {
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

    /// باز کردن بازگشتی درخت ساخت یک آیتم برای سبد خرید با حذف آیتم‌هایی که در اینونتوری موجودند
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
        out.push(item.id.clone());
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

/// خرید آیتم به صورت کاملاً Server-Authoritative (در چشمه به اینونتوری هیرو و خارج از چشمه به Courier)
pub fn try_buy_item(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    item_id: &str,
    config: &GameConfig,
    items_cfg: &ItemsConfig,
) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    let hero_cfg = config.active_hero(p.hero_index);
    let in_fountain = !p.is_dead && fountain_regen_bonus_for_pos(config, p.hero_team, p.soldier_pos) > 0.0;

    let Some(item_def) = items_cfg.get_item(item_id) else {
        return false;
    };

    if in_fountain {
        // اگر آیتم مخصوص اسلات دایره‌ای تلپورت است
        if item_def.dedicated_tp_slot {
            if p.hero_gold >= item_def.cost {
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
            }
            return false;
        }

        // اگر آیتم معمولی استک‌پذیر است و قبلاً در اینونتوری وجود دارد
        if item_def.stackable {
            if let Some(existing) = p.inventory.iter_mut().find(|it| it.item_id == item_id) {
                if p.hero_gold >= item_def.cost {
                    p.hero_gold -= item_def.cost;
                    existing.count = existing.count.saturating_add(1);
                    return true;
                }
                return false;
            }
        }

        if let Some((cost_to_pay, mut consumed_indices)) =
            items_cfg.effective_purchase_cost(item_id, &p.inventory)
        {
            let final_slot_count = p.inventory.len() - consumed_indices.len() + 1;
            if p.hero_gold >= cost_to_pay && final_slot_count <= items_cfg.max_inventory_slots {
                p.hero_gold -= cost_to_pay;
                consumed_indices.sort_unstable_by(|a, b| b.cmp(a));
                for idx in consumed_indices {
                    p.inventory.remove(idx);
                }
                let free_slot = (0..items_cfg.max_inventory_slots)
                    .find(|s| !p.inventory.iter().any(|it| it.slot == *s))
                    .unwrap_or(0);
                p.inventory.push(InventoryItemState {
                    item_id: item_id.to_string(),
                    active_mode: 0,
                    slot: free_slot,
                    count: 1,
                });
                recompute_player_slot_stats(p, config, items_cfg);
                return true;
            }
        }
    }

    // خرید از راه دور یا زمانی که اینونتوری هیرو پر است -> قرارگیری در اینونتوری Courier
    if item_def.stackable {
        if let Some(existing) = p.courier_inventory.iter_mut().find(|it| it.item_id == item_id) {
            if p.hero_gold >= item_def.cost {
                p.hero_gold -= item_def.cost;
                existing.count = existing.count.saturating_add(1);
                return true;
            }
            return false;
        }
    }

    let courier_max_slots = hero_cfg.courier.inventory_slots;
    let Some((cost_to_pay, mut consumed_indices)) =
        items_cfg.effective_purchase_cost(item_id, &p.courier_inventory)
    else {
        return false;
    };
    if p.hero_gold < cost_to_pay {
        return false;
    }
    let final_courier_count = p.courier_inventory.len() - consumed_indices.len() + 1;
    if final_courier_count > courier_max_slots {
        return false;
    }

    p.hero_gold -= cost_to_pay;
    consumed_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in consumed_indices {
        p.courier_inventory.remove(idx);
    }
    let free_slot = (0..courier_max_slots)
        .find(|s| !p.courier_inventory.iter().any(|it| it.slot == *s))
        .unwrap_or(0);
    p.courier_inventory.push(InventoryItemState {
        item_id: item_id.to_string(),
        active_mode: 0,
        slot: free_slot,
        count: 1,
    });
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

pub fn try_courier_deliver(world: &mut AuthoritativeWorldState, slot_index: usize) -> bool {
    let Some(p) = world.players.get_mut(slot_index) else {
        return false;
    };
    if p.courier_is_dead {
        return false;
    }
    p.courier_order = CourierOrder::DeliverToHero;
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
            CourierOrder::ReturnToFountain => Some(Vec3::new(p.spawn_pos.x, fly_y, p.spawn_pos.z)),
            CourierOrder::MoveTo => Some(Vec3::new(p.courier_target_pos.x, fly_y, p.courier_target_pos.z)),
            CourierOrder::DeliverToHero => {
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
                    let mut remaining = Vec::new();
                    for mut c_item in p.courier_inventory.drain(..) {
                        let def_opt = items_cfg.get_item(&c_item.item_id);
                        let is_tp = def_opt.is_some_and(|d| d.dedicated_tp_slot);
                        let is_stack = def_opt.is_some_and(|d| d.stackable);
                        if is_tp {
                            if let Some(ref mut tp_item) = p.tp_slot {
                                if tp_item.item_id == c_item.item_id && is_stack {
                                    tp_item.count = tp_item.count.saturating_add(c_item.count);
                                    transferred_any = true;
                                } else {
                                    remaining.push(c_item);
                                }
                            } else {
                                c_item.slot = 0;
                                p.tp_slot = Some(c_item);
                                transferred_any = true;
                            }
                        } else if is_stack {
                            if let Some(existing) = p.inventory.iter_mut().find(|it| it.item_id == c_item.item_id) {
                                existing.count = existing.count.saturating_add(c_item.count);
                                transferred_any = true;
                            } else if let Some(free_slot) = (0..items_cfg.max_inventory_slots)
                                .find(|s| !p.inventory.iter().any(|it| it.slot == *s))
                            {
                                c_item.slot = free_slot;
                                p.inventory.push(c_item);
                                transferred_any = true;
                            } else {
                                remaining.push(c_item);
                            }
                        } else if let Some(free_slot) = (0..items_cfg.max_inventory_slots)
                            .find(|s| !p.inventory.iter().any(|it| it.slot == *s))
                        {
                            c_item.slot = free_slot;
                            p.inventory.push(c_item);
                            transferred_any = true;
                        } else {
                            remaining.push(c_item);
                        }
                    }
                    p.courier_inventory = remaining;
                    if transferred_any {
                        recompute_player_slot_stats(p, &config, &items_cfg);
                    }
                    p.courier_order = CourierOrder::ReturnToFountain;
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