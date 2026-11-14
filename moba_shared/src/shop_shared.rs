use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;

use crate::{
    fountain_regen_bonus_for_pos, ActiveWeaponSlot, AuthoritativeWorldState, CourierOrder, GameConfig,
    PlayerSlotState,
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

        (buffs, debuffs)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemModeDef {
    pub name: String,
    pub color_rgb: (f32, f32, f32),
    pub stats: ItemStatsModifier,
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
    pub stats: ItemStatsModifier,
    pub modes: Vec<ItemModeDef>,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct ItemsConfig {
    pub max_inventory_slots: usize,
    pub sell_refund_ratio: f32,
    pub items: Vec<ItemDef>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct InventoryItemState {
    pub item_id: String,
    pub active_mode: usize,
    pub slot: usize,
}

impl ItemsConfig {
    pub fn load_from_file(path: &str) -> Self {
        let content = fs::read_to_string(path).unwrap_or_else(|e| {
            panic!("خطای بحرانی: فایل کانفیگ آیتم‌ها '{}' خوانده نشد: {}", path, e)
        });
        let cfg = ron::from_str::<ItemsConfig>(&content).unwrap_or_else(|e| {
            panic!(
                "خطای بحرانی: ساختار فایل کانفیگ آیتم‌ها '{}' نامعتبر است: {}",
                path, e
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

    let mut total_mod = ItemStatsModifier::zero();
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

    if in_fountain {
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
                });
                recompute_player_slot_stats(p, config, items_cfg);
                return true;
            }
        }
    }

    // خرید از راه دور یا زمانی که اینونتوری هیرو پر است -> قرارگیری در اینونتوری Courier
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
    let Some(pos) = p.inventory.iter().position(|it| it.slot == inv_index) else {
        return false;
    };
    let removed = p.inventory.remove(pos);
    let refund = items_cfg.sell_price(&removed.item_id);
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
                        if let Some(free_slot) = (0..items_cfg.max_inventory_slots)
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