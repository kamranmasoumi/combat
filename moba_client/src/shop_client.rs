use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::picking::hover::Hovered;
use bevy::ui::{Pressed, RelativeCursorPosition};
use bevy::ui_widgets::Button as UiButton;
use bevy::window::PrimaryWindow;
use crate::audio_client::{AudioCategory, AudioEventQueue, AudioSettingsState, ClientAudioEvent};
use moba_shared::{
    parse_recipe_target, recipe_item_id, ActiveBuffSnapshot, ClientLobbyMessage, CourierOrder,
    GameConfig, InventoryItemState, ItemActiveAbility, ItemCategory, ItemDef, ItemsConfig,
};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShopCategoryFilter {
    All,
    PrimaryWeapon,
    Pistol,
    Defense,
    Mobility,
    Upgrades,
}

impl ShopCategoryFilter {
    fn all() -> &'static [Self] {
        &[
            Self::All,
            Self::PrimaryWeapon,
            Self::Pistol,
            Self::Defense,
            Self::Mobility,
            Self::Upgrades,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::PrimaryWeapon => "Rifle",
            Self::Pistol => "Pistol",
            Self::Defense => "Defense",
            Self::Mobility => "Mobility",
            Self::Upgrades => "Tactical",
        }
    }

    fn matches(&self, cat: ItemCategory) -> bool {
        match self {
            Self::All => true,
            Self::PrimaryWeapon => cat == ItemCategory::PrimaryWeapon,
            Self::Pistol => cat == ItemCategory::Pistol,
            Self::Mobility => cat == ItemCategory::Mobility,
            Self::Defense => cat == ItemCategory::Defense,
            Self::Upgrades => cat == ItemCategory::Upgrades,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShopTierFilter {
    #[default]
    All,
    BasicOnly,
    UpgradesOnly,
}

impl ShopTierFilter {
    fn all() -> &'static [Self] {
        &[Self::All, Self::BasicOnly, Self::UpgradesOnly]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::All => "All Tiers",
            Self::BasicOnly => "Basic",
            Self::UpgradesOnly => "Upgrades",
        }
    }

    fn matches(&self, item: &ItemDef) -> bool {
        match self {
            Self::All => true,
            Self::BasicOnly => item.recipe.is_empty(),
            Self::UpgradesOnly => !item.recipe.is_empty(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldDropTarget {
    Courier,
    Hero,
}

#[derive(Resource, Default)]
pub struct InventoryDragDropBridge {
    pub is_dragging: bool,
    pub drag_source_is_courier: bool,
    pub dragging_slot: Option<usize>,
    pub hovered_world_target: Option<WorldDropTarget>,
    pub clear_hero_move_target: bool,
}

#[derive(Resource, Default)]
pub struct CourierControlState {
    pub selected: bool,
}

#[derive(Resource, Default)]
pub struct TeleportPreviewState {
    pub active: bool,
    pub wait_left_release_after_confirm: bool,
    pub valid_dest: Option<Vec3>,
    pub cursor_world_pos: Option<Vec3>,
    pub from_minimap: bool,
    pub suppress_fire_this_frame: bool,
}

#[derive(Resource, Default)]
pub struct ShopContextSnapshot {
    pub in_game: bool,
    pub chat_open: bool,
    pub in_fountain: bool,
    pub is_dead: bool,
    pub gold: u32,
    pub inventory: Vec<InventoryItemState>,
    pub tp_slot: Option<InventoryItemState>,
    pub teleport_cooldown_remaining: f32,
    pub is_teleporting: bool,
    pub teleport_progress: f32,
    pub shadow_blade_cooldown_remaining: f32,
    pub is_invisible: bool,
    pub active_buffs: Vec<ActiveBuffSnapshot>,
    pub courier_hp: f32,
    pub courier_max_hp: f32,
    pub courier_is_dead: bool,
    pub courier_respawn_remaining: f32,
    pub courier_order: CourierOrder,
    pub courier_in_fountain: bool,
    pub courier_inventory: Vec<InventoryItemState>,
    pub stash: Vec<InventoryItemState>,
    pub shop_notice_seq: u64,
    pub shop_notice_text: String,
    pub shop_notice_is_error: bool,
    pub inspected_inventory: Option<Vec<InventoryItemState>>,
    pub inspected_is_read_only: bool,
}

#[derive(Resource, Default)]
pub struct ShopOutgoingQueue(pub Vec<ClientLobbyMessage>);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShortcutInput {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl ShortcutInput {
    pub fn label(&self) -> String {
        match self {
            ShortcutInput::Mouse(MouseButton::Left) => "M1".to_string(),
            ShortcutInput::Mouse(MouseButton::Right) => "M2".to_string(),
            ShortcutInput::Mouse(MouseButton::Middle) => "M3".to_string(),
            ShortcutInput::Mouse(MouseButton::Back) => "M4".to_string(),
            ShortcutInput::Mouse(MouseButton::Forward) => "M5".to_string(),
            ShortcutInput::Mouse(MouseButton::Other(n)) => format!("M{}", n),
            ShortcutInput::Key(k) => {
                let raw = format!("{:?}", k);
                if let Some(s) = raw.strip_prefix("Key") {
                    s.to_string()
                } else if let Some(s) = raw.strip_prefix("Digit") {
                    s.to_string()
                } else if let Some(s) = raw.strip_prefix("Numpad") {
                    format!("NUM{}", s)
                } else {
                    raw.to_uppercase()
                }
            }
        }
    }

    pub fn just_pressed(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        match self {
            ShortcutInput::Key(k) => keys.just_pressed(*k),
            ShortcutInput::Mouse(m) => mouse.just_pressed(*m),
        }
    }

    pub fn pressed(
        &self,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
    ) -> bool {
        match self {
            ShortcutInput::Key(k) => keys.pressed(*k),
            ShortcutInput::Mouse(m) => mouse.pressed(*m),
        }
    }

    pub fn from_config_str(raw: &str) -> Self {
        let up = raw.trim().to_ascii_uppercase();
        match up.as_str() {
            "M1" => ShortcutInput::Mouse(MouseButton::Left),
            "M2" => ShortcutInput::Mouse(MouseButton::Right),
            "M3" => ShortcutInput::Mouse(MouseButton::Middle),
            "M4" => ShortcutInput::Mouse(MouseButton::Back),
            "M5" => ShortcutInput::Mouse(MouseButton::Forward),
            "SPACE" => ShortcutInput::Key(KeyCode::Space),
            "SHIFT" | "LSHIFT" => ShortcutInput::Key(KeyCode::ShiftLeft),
            "RSHIFT" => ShortcutInput::Key(KeyCode::ShiftRight),
            "CTRL" | "LCTRL" => ShortcutInput::Key(KeyCode::ControlLeft),
            "RCTRL" => ShortcutInput::Key(KeyCode::ControlRight),
            "ALT" | "LALT" => ShortcutInput::Key(KeyCode::AltLeft),
            "RALT" => ShortcutInput::Key(KeyCode::AltRight),
            "TAB" => ShortcutInput::Key(KeyCode::Tab),
            "A" => ShortcutInput::Key(KeyCode::KeyA),
            "B" => ShortcutInput::Key(KeyCode::KeyB),
            "C" => ShortcutInput::Key(KeyCode::KeyC),
            "D" => ShortcutInput::Key(KeyCode::KeyD),
            "E" => ShortcutInput::Key(KeyCode::KeyE),
            "F" => ShortcutInput::Key(KeyCode::KeyF),
            "G" => ShortcutInput::Key(KeyCode::KeyG),
            "H" => ShortcutInput::Key(KeyCode::KeyH),
            "I" => ShortcutInput::Key(KeyCode::KeyI),
            "J" => ShortcutInput::Key(KeyCode::KeyJ),
            "K" => ShortcutInput::Key(KeyCode::KeyK),
            "L" => ShortcutInput::Key(KeyCode::KeyL),
            "M" => ShortcutInput::Key(KeyCode::KeyM),
            "N" => ShortcutInput::Key(KeyCode::KeyN),
            "O" => ShortcutInput::Key(KeyCode::KeyO),
            "P" => ShortcutInput::Key(KeyCode::KeyP),
            "Q" => ShortcutInput::Key(KeyCode::KeyQ),
            "R" => ShortcutInput::Key(KeyCode::KeyR),
            "S" => ShortcutInput::Key(KeyCode::KeyS),
            "T" => ShortcutInput::Key(KeyCode::KeyT),
            "U" => ShortcutInput::Key(KeyCode::KeyU),
            "V" => ShortcutInput::Key(KeyCode::KeyV),
            "W" => ShortcutInput::Key(KeyCode::KeyW),
            "X" => ShortcutInput::Key(KeyCode::KeyX),
            "Y" => ShortcutInput::Key(KeyCode::KeyY),
            "Z" => ShortcutInput::Key(KeyCode::KeyZ),
            _ => panic!(
                "خطای بحرانی کانفیگ: کلید '{}' در config.ron معتبر نیست!",
                raw
            ),
        }
    }
}

#[derive(Resource, Default)]
pub struct InventoryShortcutsState {
    pub bindings: [Option<ShortcutInput>; 6],
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct VerticalAimKeyState {
    pub binding: ShortcutInput,
}

#[derive(Resource, Default)]
pub struct SettingsUiState {
    pub is_open: bool,
    pub listening_slot: Option<usize>,
    pub listening_vertical_aim: bool,
    pub just_started_listening: bool,
    pub status_message: Option<(String, bool)>,
    pub dirty: bool,
}

#[derive(Resource, Default)]
struct InventoryDragState {
    pressed_slot: Option<usize>,
    pressed_is_courier: bool,
    press_cursor: Vec2,
    dragging_slot: Option<usize>,
    dragging_is_courier: bool,
}

#[derive(Resource)]
pub struct ShopUiState {
    pub is_open: bool,
    pub active_category: ShopCategoryFilter,
    pub tier_filter: ShopTierFilter,
    pub only_affordable: bool,
    pub selected_item_id: String,
    pub cart: Vec<String>,
    pub search_query: String,
    pub search_focused: bool,
    pub pending_sell_slot: Option<usize>,
    pub pending_sell_is_courier: bool,
    pub pending_sell_is_stash: bool,
    pub prev_inventory: Vec<InventoryItemState>,
    pub prev_courier_inventory: Vec<InventoryItemState>,
    pub prev_stash: Vec<InventoryItemState>,
    pub last_notice_seq: u64,
    pub notice_text: String,
    pub notice_is_error: bool,
    pub notice_timer: f32,
    pub is_pointer_over_hud_ui: bool,
    pub is_dragging_item: bool,
    pub suppress_world_click_this_frame: bool,
    pub dirty: bool,
}

impl Default for ShopUiState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_category: ShopCategoryFilter::All,
            tier_filter: ShopTierFilter::All,
            only_affordable: false,
            selected_item_id: String::new(),
            cart: Vec::new(),
            search_query: String::new(),
            search_focused: false,
            pending_sell_slot: None,
            pending_sell_is_courier: false,
            pending_sell_is_stash: false,
            prev_inventory: Vec::new(),
            prev_courier_inventory: Vec::new(),
            prev_stash: Vec::new(),
            last_notice_seq: 0,
            notice_text: String::new(),
            notice_is_error: false,
            notice_timer: 0.0,
            is_pointer_over_hud_ui: false,
            is_dragging_item: false,
            suppress_world_click_this_frame: false,
            dirty: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct ItemIconCache(std::collections::HashMap<String, Option<Handle<Image>>>);

impl ItemIconCache {
    fn get_or_load(&mut self, icon_path: &str, asset_server: &AssetServer) -> Option<Handle<Image>> {
        if icon_path.trim().is_empty() {
            return None;
        }
        if let Some(cached) = self.0.get(icon_path) {
            return cached.clone();
        }
        let exists = Path::new("assets").join(icon_path).is_file() || Path::new(icon_path).is_file();
        let handle = if exists {
            Some(asset_server.load(icon_path.to_string()))
        } else {
            None
        };
        self.0.insert(icon_path.to_string(), handle.clone());
        handle
    }
}

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopContextSnapshot>()
            .init_resource::<ShopOutgoingQueue>()
            .init_resource::<ShopUiState>()
            .init_resource::<InventoryShortcutsState>()
            .init_resource::<SettingsUiState>()
            .init_resource::<InventoryDragState>()
            .init_resource::<InventoryDragDropBridge>()
            .init_resource::<CourierControlState>()
            .init_resource::<TeleportPreviewState>()
            .init_resource::<ItemIconCache>()
            .add_systems(Startup, spawn_shop_and_inventory_ui)
            .add_systems(
                Update,
                (
                    shop_hotkey_and_button_system,
                    shop_search_input_system,
                    shop_scroll_system,
                    rebuild_shop_modal_system,
                    rebuild_hud_inventory_bar_system,
                    hud_cart_interaction_system,
                    shop_hover_tooltip_system,
                    update_hud_buffs_bar_system,
                    update_shop_notice_toast_system,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    sell_confirm_modal_system,
                    settings_modal_interaction_system,
                    audio_mixer_slider_interaction_system,
                    rebuild_settings_modal_system,
                    inventory_shortcut_trigger_system,
                    hud_inventory_drag_and_drop_system,
                    courier_hud_and_hotkey_system,
                )
                    .chain(),
            );
    }
}

pub fn spawn_item_icon_badge(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    icon_cache: &mut ItemIconCache,
    item: &ItemDef,
    size_px: f32,
    font_px: f32,
) {
    let c = item.icon_color_rgb;
    let bg = Color::srgb(c.0 * 0.25, c.1 * 0.25, c.2 * 0.25);

    parent
        .spawn((
            Pickable::IGNORE,
            Node {
                width: px(size_px),
                height: px(size_px),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(bg),
        ))
        .with_children(|box_node| {
            if let Some(icon_h) = icon_cache.get_or_load(&item.icon_path, asset_server) {
                box_node.spawn((
                    ImageNode::new(icon_h),
                    Node {
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                ));
            } else {
                box_node.spawn((
                    Text::new(&item.short_code),
                    TextFont {
                        font_size: FontSize::Px(font_px),
                        ..default()
                    },
                    TextColor(Color::srgb(
                        (c.0 + 0.35).min(1.0),
                        (c.1 + 0.35).min(1.0),
                        (c.2 + 0.35).min(1.0),
                    )),
                ));
            }
        });
}

fn spawn_recipe_scroll_badge(
    parent: &mut ChildSpawnerCommands,
    size_px: f32,
    font_px: f32,
) {
    parent
        .spawn((
            Pickable::IGNORE,
            Node {
                width: px(size_px),
                height: px(size_px),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(4.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.98, 0.80, 0.34)),
            BackgroundColor(Color::srgba(0.32, 0.22, 0.06, 0.96)),
        ))
        .with_children(|box_node| {
            box_node.spawn((
                Pickable::IGNORE,
                Text::new("REC"),
                TextFont {
                    font_size: FontSize::Px(font_px),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.92, 0.56)),
            ));
        });
}

// -----------------------------------------------------------------------------
// مارکرهای رابط کاربری شاپ و اینونتوری
// -----------------------------------------------------------------------------

#[derive(Component)]
struct HudInventoryRoot;

#[derive(Component)]
struct HudInventorySlotsRow;

#[derive(Component)]
struct HudCartBarRoot;

#[derive(Component)]
struct HudCartSlotsRow;

#[derive(Component)]
struct HudCartSlotBtn {
    cart_index: usize,
    item_id: String,
}

#[derive(Component)]
struct HudCartRemoveBtn(usize);

#[derive(Component)]
struct HudCartClearBtn;

#[derive(Component)]
struct ShopPanelCard;

#[derive(Component)]
struct ShopHoverTooltipRoot;

#[derive(Component)]
struct HudTpSlotBtn;

#[derive(Component)]
struct HudBuffsContainer;

#[derive(Component)]
struct HudShopToggleBtn;

#[derive(Component)]
struct HudShopToggleText;

#[derive(Component)]
struct HudInventorySlotBtn {
    inv_index: usize,
}

#[derive(Component)]
struct ShopModalRoot;

#[derive(Component)]
struct ShopHeaderGoldText;

#[derive(Component)]
struct ShopFountainBadge;

#[derive(Component)]
struct ShopFountainBadgeText;

#[derive(Component)]
struct ShopCloseBtn;

#[derive(Component)]
struct ShopSearchBoxBtn;

#[derive(Component)]
struct ShopSearchText;

#[derive(Component)]
struct ShopSearchClearBtn;

#[derive(Component)]
struct ShopCategoryTabsRow;

#[derive(Component)]
struct ShopCategoryBtn(ShopCategoryFilter);

#[derive(Component)]
struct ShopQuickFilterRow;

#[derive(Component)]
struct ShopTierFilterBtn(ShopTierFilter);

#[derive(Component)]
struct ShopAffordableToggleBtn;

#[derive(Component)]
struct ShopScrollablePanel;

#[derive(Component)]
struct ShopCatalogGrid;

#[derive(Component)]
struct ShopSelectItemBtn(String);

#[derive(Component)]
struct ShopRecipeNodeBtn(String);

#[derive(Component)]
struct ShopInspectorColumn;

#[derive(Component)]
struct ShopBuyNowBtn(String);

#[derive(Component)]
struct ShopAddToCartBtn(String);

#[derive(Component)]
struct SellConfirmModalRoot;

#[derive(Component)]
struct SellConfirmCardContent;

#[derive(Component)]
struct SellConfirmAcceptBtn {
    slot: usize,
    is_courier: bool,
    is_stash: bool,
}

#[derive(Component)]
struct ContextMoveToStashBtn {
    slot: usize,
    is_courier: bool,
}

#[derive(Component)]
struct ContextTakeFromStashBtn {
    stash_slot: usize,
    to_courier: bool,
}

#[derive(Component)]
struct HudStashBarRoot;

#[derive(Component)]
struct HudStashSlotsRow;

#[derive(Component)]
struct HudStashSlotBtn {
    stash_index: usize,
}

#[derive(Component)]
struct HudStashTakeAllBtn;

#[derive(Component)]
struct HudShopNoticeRoot;

#[derive(Component)]
struct HudShopNoticeText;

#[derive(Component)]
struct SettingsToggleBtn;

#[derive(Component)]
struct SettingsModalRoot;

#[derive(Component)]
struct SettingsCloseBtn;

#[derive(Component)]
struct SettingsSlotsContainer;

#[derive(Component)]
struct SettingsSlotBindBtn(usize);

#[derive(Component)]
struct SettingsSlotClearBtn(usize);

#[derive(Component)]
struct SettingsVerticalAimContainer;

#[derive(Component)]
struct SettingsVerticalAimBindBtn;

#[derive(Component)]
struct SettingsVerticalAimResetBtn;

#[derive(Component)]
struct SettingsAudioContainer;

#[derive(Component)]
struct AudioSliderTrack {
    channel: Option<usize>,
}

#[derive(Component)]
struct AudioChannelMuteBtn {
    channel: Option<usize>,
}

#[derive(Component)]
struct AudioResetBtn;

#[derive(Component)]
struct SettingsStatusText;

#[derive(Component)]
struct DragGhostRoot;

#[derive(Component)]
struct DragGhostLabelText;

#[derive(Component)]
struct DragGhostTargetText;

#[derive(Component)]
struct HudCourierDeliverBtn;

#[derive(Component)]
struct HudCourierReturnBtn;

#[derive(Component)]
struct HudCourierDeadBanner;

#[derive(Component)]
struct HudCourierDeadText;

#[derive(Component)]
struct HudCourierSelectBtn;

#[derive(Component)]
struct HudCourierSelectBtnText;

fn is_cursor_inside(rel: &RelativeCursorPosition) -> bool {
    rel.normalized
        .is_some_and(|p| (-0.5..=0.5).contains(&p.x) && (-0.5..=0.5).contains(&p.y))
}

fn effective_cost_for_display(items_cfg: &ItemsConfig, item_id: &str, ctx: &ShopContextSnapshot) -> (u32, usize) {
    if let Some(target_id) = parse_recipe_target(item_id) {
        return (items_cfg.recipe_fee(target_id), 0);
    }
    let hero_in_shop = ctx.in_fountain && !ctx.is_dead;
    let courier_in_shop = ctx.courier_in_fountain && !ctx.courier_is_dead;
    let mut combined = Vec::new();
    if hero_in_shop {
        combined.extend(ctx.inventory.iter().cloned());
        combined.extend(ctx.stash.iter().cloned());
    } else if courier_in_shop {
        combined.extend(ctx.courier_inventory.iter().cloned());
        combined.extend(ctx.stash.iter().cloned());
    } else {
        combined.extend(ctx.stash.iter().cloned());
    }
    items_cfg
        .effective_purchase_cost(item_id, &combined)
        .map_or_else(
            || (items_cfg.get_item(item_id).map_or(0, |d| d.cost), 0),
            |(c, consumed)| (c, consumed.len()),
        )
}

fn item_matches_search(item: &ItemDef, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }
    for token in query_lower.split_whitespace() {
        let mut matched = item.name.to_lowercase().contains(token)
            || item.short_code.to_lowercase().contains(token)
            || item.id.to_lowercase().contains(token)
            || item.description.to_lowercase().contains(token)
            || item.category.label().to_lowercase().contains(token);

        if !matched {
            let s = &item.stats;
            let has_speed = s.move_speed_pct.abs() > 0.0001;
            let has_dmg = s.primary_damage_bonus.abs() > 0.0001 || s.pistol_damage_bonus.abs() > 0.0001;
            let has_rate = s.primary_fire_rate_pct.abs() > 0.0001 || s.pistol_fire_rate_pct.abs() > 0.0001;
            let has_reload = s.primary_reload_speed_pct.abs() > 0.0001 || s.pistol_reload_speed_pct.abs() > 0.0001;
            let has_mag = s.primary_magazine_bonus != 0 || s.pistol_magazine_bonus != 0;
            let has_range = s.primary_range_bonus.abs() > 0.0001 || s.pistol_range_bonus.abs() > 0.0001;
            let has_hp = s.max_hp_bonus.abs() > 0.0001;
            let has_regen = s.hp_regen_bonus.abs() > 0.0001;
            let has_armor = s.armor_bonus.abs() > 0.0001;
            let has_stam = s.max_stamina_bonus.abs() > 0.0001 || s.stamina_regen_bonus.abs() > 0.0001;
            let has_gren = s.grenade_damage_bonus.abs() > 0.0001
                || s.grenade_aoe_bonus.abs() > 0.0001
                || s.grenade_max_charges_bonus != 0
                || s.grenade_recharge_speed_pct.abs() > 0.0001;
            let has_invis = item.active_ability == ItemActiveAbility::ShadowBlade;
            let has_tp = item.active_ability == ItemActiveAbility::Teleport;
            let has_modes = !item.modes.is_empty();

            matched = (token == "speed" || token == "move" || token == "boot" || token == "boots") && has_speed
                || (token == "dmg" || token == "damage" || token == "attack") && has_dmg
                || (token == "rate" || token == "fire" || token == "dps" || token == "as") && has_rate
                || (token == "reload") && has_reload
                || (token == "mag" || token == "ammo" || token == "clip") && has_mag
                || (token == "range" || token == "scope" || token == "sniper") && has_range
                || (token == "hp" || token == "health" || token == "tank" || token == "vit") && has_hp
                || (token == "regen" || token == "heal" || token == "sustain") && has_regen
                || (token == "armor" || token == "def" || token == "defense") && has_armor
                || (token == "stamina" || token == "sprint") && has_stam
                || (token == "grenade" || token == "bomb" || token == "aoe" || token == "blast") && has_gren
                || (token == "invis" || token == "stealth" || token == "shadow") && has_invis
                || (token == "tp" || token == "teleport" || token == "scroll") && has_tp
                || (token == "mode" || token == "toggle" || token == "switch") && has_modes
                || (token == "active") && (has_invis || has_tp || has_modes)
                || (token == "rifle" || token == "primary") && item.category == ItemCategory::PrimaryWeapon
                || (token == "pistol" || token == "sidearm") && item.category == ItemCategory::Pistol;
        }
        if !matched {
            return false;
        }
    }
    true
}

fn spawn_shop_and_inventory_ui(
    mut commands: Commands,
    config: Res<GameConfig>,
    items_cfg: Res<ItemsConfig>,
) {
    commands.insert_resource(VerticalAimKeyState {
        binding: ShortcutInput::from_config_str(&config.soldier.vertical_aim_key),
    });
    let sui = &items_cfg.shop_ui;

    // ۰. نوار نمایشگر مدت‌زمان باف‌ها و آیتم‌های فعال بالای سبد خرید و اینونتوری
    commands.spawn((
        HudBuffsContainer,
        Pickable::IGNORE,
        GlobalZIndex(190),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(208.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: px(5.0),
            display: Display::None,
            ..default()
        },
    ));

    // ۰.۲ بنر خطا/هشدار شاپ (مثلاً پر بودن اینونتوری‌ها یا کمبود طلا)
    commands
        .spawn((
            HudShopNoticeRoot,
            Pickable::IGNORE,
            GlobalZIndex(280),
            Node {
                position_type: PositionType::Absolute,
                left: px(sui.hud_inventory_left_px),
                bottom: px(210.0),
                padding: UiRect::axes(px(14.0), px(7.0)),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(8.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgb(0.95, 0.30, 0.30)),
            BackgroundColor(Color::srgba(0.32, 0.06, 0.06, 0.96)),
        ))
        .with_children(|n| {
            n.spawn((
                HudShopNoticeText,
                Pickable::IGNORE,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.85)),
            ));
        });

    // ۰.۴ نوار Stash (۶ اسلاته در چشمه، نمایش بالای سبد خرید سریع)
    commands
        .spawn((
            HudStashBarRoot,
            RelativeCursorPosition::default(),
            GlobalZIndex(190),
            Node {
                position_type: PositionType::Absolute,
                left: px(sui.hud_inventory_left_px),
                bottom: px(152.0),
                padding: UiRect::axes(px(8.0), px(5.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(8.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.92, 0.72, 0.25, 0.80)),
            BackgroundColor(Color::srgba(0.06, 0.06, 0.09, 0.92)),
        ))
        .with_children(|bar| {
            bar.spawn((
                Pickable::IGNORE,
                Text::new("STASH"),
                TextFont {
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.38)),
            ));
            bar.spawn((
                HudStashSlotsRow,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(5.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            bar.spawn((
                UiButton,
                Hovered::default(),
                HudStashTakeAllBtn,
                Node {
                    padding: UiRect::axes(px(7.0), px(5.0)),
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(5.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.24, 0.88, 0.52)),
                BackgroundColor(Color::srgba(0.06, 0.24, 0.14, 0.94)),
            ))
            .with_children(|b| {
                b.spawn((
                    Pickable::IGNORE,
                    Text::new("TAKE ALL"),
                    TextFont {
                        font_size: FontSize::Px(9.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.45, 0.98, 0.68)),
                ));
            });
        });

    // ۰.۵ نوار سبد خرید سریع بالای اینونتوری در HUD
    commands
        .spawn((
            HudCartBarRoot,
            RelativeCursorPosition::default(),
            GlobalZIndex(190),
            Node {
                position_type: PositionType::Absolute,
                left: px(sui.hud_inventory_left_px),
                bottom: px(88.0),
                padding: UiRect::axes(px(8.0), px(5.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(8.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.35, 0.45, 0.60, 0.72)),
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.90)),
        ))
        .with_children(|bar| {
            bar.spawn((
                HudCartSlotsRow,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(5.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
        });

    // ۱. نوار ۶ اسلاته اینونتوری + اسلات TP + دکمه [B] SHOP + دکمه‌های سریع Courier
    commands
        .spawn((
            HudInventoryRoot,
            RelativeCursorPosition::default(),
            GlobalZIndex(190),
            Node {
                position_type: PositionType::Absolute,
                left: px(sui.hud_inventory_left_px),
                bottom: px(16.0),
                padding: UiRect::all(px(8.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(10.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.42, 0.58, 0.72, 0.78)),
            BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.90)),
        ))
        .with_children(|root| {
            root.spawn((
                HudInventorySlotsRow,
                Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(6.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));

            root.spawn((
                UiButton,
            Hovered::default(),
                RelativeCursorPosition::default(),
                HudTpSlotBtn,
                Node {
                    width: px(46.0),
                    height: px(46.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(2.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BorderColor::all(Color::srgba(0.28, 0.72, 0.95, 0.75)),
                BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.90)),
            ));

            root.spawn((
                UiButton,
            Hovered::default(),
                RelativeCursorPosition::default(),
                HudShopToggleBtn,
                Node {
                    padding: UiRect::axes(px(12.0), px(12.0)),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BorderColor::all(Color::srgb(0.95, 0.78, 0.24)),
                BackgroundColor(Color::srgba(0.24, 0.19, 0.05, 0.92)),
            ))
            .with_children(|btn| {
                btn.spawn((
                    HudShopToggleText,
                    Pickable::IGNORE,
                    Text::new("[B] SHOP"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.88, 0.36)),
                ));
            });

            root.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(4.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    UiButton,
            Hovered::default(),
                    HudCourierDeliverBtn,
                    Node {
                        padding: UiRect::axes(px(8.0), px(4.0)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.95, 0.78, 0.25)),
                    BackgroundColor(Color::srgba(0.24, 0.18, 0.05, 0.94)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Pickable::IGNORE,
                        Text::new("[F3] DELIVER"),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.88, 0.36)),
                    ));
                });

                col.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        UiButton,
            Hovered::default(),
                        HudCourierReturnBtn,
                        Node {
                            padding: UiRect::axes(px(6.0), px(3.0)),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(4.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.45, 0.62, 0.80)),
                        BackgroundColor(Color::srgba(0.10, 0.15, 0.22, 0.94)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Pickable::IGNORE,
                            Text::new("[F4] HOME"),
                            TextFont {
                                font_size: FontSize::Px(9.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.82, 0.90, 1.0)),
                        ));
                    });

                    row.spawn((
                        UiButton,
            Hovered::default(),
                        RelativeCursorPosition::default(),
                        HudCourierSelectBtn,
                        Node {
                            padding: UiRect::axes(px(6.0), px(3.0)),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(4.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.35, 0.75, 0.92)),
                        BackgroundColor(Color::srgba(0.08, 0.16, 0.24, 0.94)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            HudCourierSelectBtnText,
                            Pickable::IGNORE,
                            Text::new("[F2] DRONE"),
                            TextFont {
                                font_size: FontSize::Px(9.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.60, 0.92, 1.0)),
                        ));
                    });
                });
            });
        });

    // ۱.۵ بنر قرمز Cooldown مرگ Courier
    commands
        .spawn((
            HudCourierDeadBanner,
            Pickable::IGNORE,
            GlobalZIndex(195),
            Node {
                position_type: PositionType::Absolute,
                left: px(sui.hud_inventory_left_px),
                bottom: px(212.0),
                padding: UiRect::axes(px(12.0), px(6.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(8.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(8.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgb(0.95, 0.25, 0.25)),
            BackgroundColor(Color::srgba(0.32, 0.05, 0.05, 0.94)),
        ))
        .with_children(|banner| {
            banner
                .spawn((
                    Node {
                        width: px(22.0),
                        height: px(16.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::all(px(4.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(1.0, 0.45, 0.45)),
                    BackgroundColor(Color::srgba(0.55, 0.10, 0.10, 0.95)),
                ))
                .with_children(|icon| {
                    icon.spawn((
                        Text::new("DRN"),
                        TextFont {
                            font_size: FontSize::Px(8.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.85, 0.85)),
                    ));
                });

            banner.spawn((
                HudCourierDeadText,
                Text::new("COURIER DEAD - RESPAWNS IN 0s"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.42, 0.42)),
            ));
        });

    // ۲. پنجره ۲ ستونه شاپ (بدون ستون سبد خرید سمت راست)
    let body_h = (sui.shop_panel_height_px - 88.0).max(260.0);
    let catalog_h = (body_h - 104.0).max(160.0);
    commands
        .spawn((
            ShopModalRoot,
            GlobalZIndex(225),
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
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.68)),
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    ShopPanelCard,
                    RelativeCursorPosition::default(),
                    Node {
                        width: px(sui.shop_panel_width_px),
                        height: px(sui.shop_panel_height_px),
                        padding: UiRect::all(px(16.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10.0),
                        border: UiRect::all(px(2.0)),
                        border_radius: BorderRadius::all(px(12.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.45, 0.62, 0.78, 0.88)),
                    BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.97)),
                ))
                .with_children(|win| {
                    // هدر شاپ
                    win.spawn(Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(2.0),
                            ..default()
                        })
                        .with_children(|title_col| {
                            title_col.spawn((
                                Text::new("ARMORY & ITEM SHOP  [B / Esc]"),
                                TextFont {
                                    font_size: FontSize::Px(18.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.95, 0.88, 0.55)),
                            ));
                            title_col.spawn((
                                Text::new("Right-Click: Quick Buy  |  Shift+Click: Queue Quick-Buy  |  /: Search"),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.62, 0.74, 0.88)),
                            ));
                        });

                        hdr.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: px(10.0),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|right| {
                            right
                                .spawn((
                                    ShopFountainBadge,
                                    Node {
                                        padding: UiRect::axes(px(10.0), px(5.0)),
                                        border: UiRect::all(px(1.0)),
                                        border_radius: BorderRadius::all(px(6.0)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgb(0.24, 0.88, 0.48)),
                                    BackgroundColor(Color::srgba(0.06, 0.24, 0.12, 0.9)),
                                ))
                                .with_children(|b| {
                                    b.spawn((
                                        ShopFountainBadgeText,
                                        Text::new("HERO IN SHOP"),
                                        TextFont {
                                            font_size: FontSize::Px(11.5),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.35, 0.98, 0.60)),
                                    ));
                                });

                            right
                                .spawn((
                                    Node {
                                        padding: UiRect::axes(px(12.0), px(5.0)),
                                        border_radius: BorderRadius::all(px(6.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.28, 0.22, 0.05, 0.9)),
                                ))
                                .with_children(|g| {
                                    g.spawn((
                                        ShopHeaderGoldText,
                                        Text::new("GOLD: 600"),
                                        TextFont {
                                            font_size: FontSize::Px(13.5),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(1.0, 0.88, 0.30)),
                                    ));
                                });

                            right
                                .spawn((
                                    UiButton,
            Hovered::default(),
                                    RelativeCursorPosition::default(),
                                    ShopCloseBtn,
                                    Node {
                                        padding: UiRect::axes(px(10.0), px(5.0)),
                                        border: UiRect::all(px(1.0)),
                                        border_radius: BorderRadius::all(px(6.0)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgb(0.88, 0.28, 0.26)),
                                    BackgroundColor(Color::srgba(0.28, 0.07, 0.07, 0.9)),
                                ))
                                .with_children(|c| {
                                    c.spawn((
                                        Pickable::IGNORE,
                                        Text::new("X"),
                                        TextFont {
                                            font_size: FontSize::Px(13.0),
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                        });
                    });

                    // بدنه ۲ ستونه تمیز شاپ: چپ = کاتالوگ هوشمند، راست = درخت هرمی آیتم
                    win.spawn(Node {
                        width: percent(100),
                        height: px(body_h),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(12.0),
                        ..default()
                    })
                    .with_children(|body| {
                        // ستون چپ: سرچ هوشمند + دسته‌بندی ساده + فیلتر سریع + کاتالوگ گروه‌بندی‌شده
                        body.spawn(Node {
                            width: px(sui.catalog_column_width_px),
                            height: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(6.0),
                            ..default()
                        })
                        .with_children(|left| {
                            left.spawn(Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Row,
                                column_gap: px(6.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|srow| {
                                srow.spawn((
                                    UiButton,
            Hovered::default(),
                                    ShopSearchBoxBtn,
                                    Node {
                                        flex_grow: 1.0,
                                        padding: UiRect::axes(px(10.0), px(6.0)),
                                        border: UiRect::all(px(1.5)),
                                        border_radius: BorderRadius::all(px(6.0)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgba(0.35, 0.48, 0.62, 0.8)),
                                    BackgroundColor(Color::srgba(0.07, 0.10, 0.15, 0.95)),
                                ))
                                .with_children(|sb| {
                                    sb.spawn((
                                        ShopSearchText,
                                        Pickable::IGNORE,
                                        Text::new("Search name or stat (armor, speed, regen...)"),
                                        TextFont {
                                            font_size: FontSize::Px(11.5),
                                            ..default()
                                        },
                                        TextColor(Color::srgba(0.60, 0.68, 0.78, 0.85)),
                                    ));
                                });

                                srow.spawn((
                                    UiButton,
            Hovered::default(),
                                    ShopSearchClearBtn,
                                    Node {
                                        padding: UiRect::axes(px(9.0), px(6.0)),
                                        border: UiRect::all(px(1.0)),
                                        border_radius: BorderRadius::all(px(6.0)),
                                        ..default()
                                    },
                                    BorderColor::all(Color::srgba(0.45, 0.35, 0.35, 0.75)),
                                    BackgroundColor(Color::srgba(0.18, 0.08, 0.09, 0.9)),
                                ))
                                .with_children(|cb| {
                                    cb.spawn((
                                        Pickable::IGNORE,
                                        Text::new("CLR"),
                                        TextFont {
                                            font_size: FontSize::Px(11.0),
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                            });

                            left.spawn((
                                ShopCategoryTabsRow,
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Row,
                                    flex_wrap: FlexWrap::Wrap,
                                    column_gap: px(5.0),
                                    row_gap: px(4.0),
                                    ..default()
                                },
                            ));

                            left.spawn((
                                ShopQuickFilterRow,
                                Node {
                                    width: percent(100),
                                    flex_direction: FlexDirection::Row,
                                    flex_wrap: FlexWrap::Wrap,
                                    align_items: AlignItems::Center,
                                    column_gap: px(5.0),
                                    row_gap: px(4.0),
                                    ..default()
                                },
                            ));

                            left.spawn((
                                ShopCatalogGrid,
                                ShopScrollablePanel,
                                RelativeCursorPosition::default(),
                                ScrollPosition::default(),
                                Node {
                                    width: percent(100),
                                    height: px(catalog_h),
                                    padding: UiRect::all(px(8.0)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(8.0),
                                    border: UiRect::all(px(1.0)),
                                    border_radius: BorderRadius::all(px(8.0)),
                                    overflow: Overflow::scroll_y(),
                                    ..default()
                                },
                                BorderColor::all(Color::srgba(0.25, 0.32, 0.42, 0.7)),
                                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.85)),
                            ));
                        });

                        // ستون راست: درخت هرمی ساخت (Dota 2 Pyramid) + دکمه‌های خرید
                        body.spawn((
                            ShopInspectorColumn,
                            ShopScrollablePanel,
                            RelativeCursorPosition::default(),
                            ScrollPosition::default(),
                            Node {
                                width: px(sui.inspector_column_width_px),
                                height: percent(100),
                                padding: UiRect::all(px(12.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(10.0),
                                border: UiRect::all(px(1.0)),
                                border_radius: BorderRadius::all(px(8.0)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.28, 0.38, 0.50, 0.75)),
                            BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.90)),
                        ));
                    });
                });
        });

    // ۳. دکمه تنظیمات (بالا سمت چپ صفحه)
    commands
        .spawn((
            UiButton,
            Hovered::default(),
            RelativeCursorPosition::default(),
            SettingsToggleBtn,
            GlobalZIndex(210),
            Node {
                position_type: PositionType::Absolute,
                top: px(12.0),
                left: px(12.0),
                padding: UiRect::axes(px(10.0), px(7.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(7.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(8.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgba(0.45, 0.65, 0.85, 0.85)),
            BackgroundColor(Color::srgba(0.05, 0.08, 0.13, 0.92)),
        ))
        .with_children(|btn| {
            btn.spawn((
                Pickable::IGNORE,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2.5),
                    ..default()
                },
            ))
            .with_children(|bars| {
                for w in [14.0, 10.0, 14.0] {
                    bars.spawn((
                        Node {
                            width: px(w),
                            height: px(2.0),
                            border_radius: BorderRadius::all(px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.85, 0.92, 1.0)),
                    ));
                }
            });
            btn.spawn((
                Pickable::IGNORE,
                Text::new("KEYS"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
            ));
        });

    // ۴. پنجره تنظیمات شورتکات‌های اینونتوری
    commands
        .spawn((
            SettingsModalRoot,
            GlobalZIndex(255),
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
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.72)),
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    Node {
                        width: px(600.0),
                        padding: UiRect::all(px(18.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(14.0),
                        border: UiRect::all(px(2.0)),
                        border_radius: BorderRadius::all(px(12.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.45, 0.68, 0.92)),
                    BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.98)),
                ))
                .with_children(|card| {
                    card.spawn(Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn((
                            Text::new("INVENTORY SHORTCUTS"),
                            TextFont {
                                font_size: FontSize::Px(18.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.95, 0.85, 0.40)),
                        ));
                        hdr.spawn((
                            UiButton,
            Hovered::default(),
                            SettingsCloseBtn,
                            Node {
                                padding: UiRect::axes(px(12.0), px(6.0)),
                                border: UiRect::all(px(1.0)),
                                border_radius: BorderRadius::all(px(6.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgb(0.85, 0.35, 0.35)),
                            BackgroundColor(Color::srgba(0.32, 0.08, 0.08, 0.95)),
                        ))
                        .with_children(|cbtn| {
                            cbtn.spawn((
                                Text::new("[X] CLOSE"),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    });

                    card.spawn((
                        Text::new("Click any slot below, then press ANY keyboard key or mouse button to bind it. Duplicates are not allowed."),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.82, 0.90)),
                    ));

                    card.spawn((
                        SettingsSlotsContainer,
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(8.0),
                            ..default()
                        },
                    ));

                    card.spawn((
                        SettingsVerticalAimContainer,
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(px(10.0)),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(8.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.28, 0.40, 0.54, 0.75)),
                        BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.90)),
                    ));

                    card.spawn((
                        Text::new("ADVANCED AUDIO MIXER"),
                        TextFont {
                            font_size: FontSize::Px(15.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.88, 1.0)),
                    ));

                    card.spawn((
                        SettingsAudioContainer,
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(7.0),
                            padding: UiRect::all(px(10.0)),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(8.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.28, 0.40, 0.54, 0.75)),
                        BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.90)),
                    ));

                    card.spawn((
                        SettingsStatusText,
                        Text::new("Select a slot to assign a shortcut."),
                        TextFont {
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.70, 0.85, 0.95)),
                    ));
                });
        });

    // ۵. منوی شناور روی آیتم اینونتوری (فروش درجا برای هیرو یا کوریر)
    commands
        .spawn((
            SellConfirmModalRoot,
            RelativeCursorPosition::default(),
            GlobalZIndex(260),
            Node {
                position_type: PositionType::Absolute,
                left: px(sui.hud_inventory_left_px),
                bottom: px(76.0),
                display: Display::None,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                SellConfirmCardContent,
                Node {
                    min_width: px(180.0),
                    padding: UiRect::all(px(8.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(6.0),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::all(px(8.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.85, 0.68, 0.24)),
                BackgroundColor(Color::srgba(0.05, 0.07, 0.11, 0.97)),
            ));
        });

    // ۶. کارت تولتیپ هاور آیتم (شاپ + درخت هرمی + اینونتوری هیرو/کوریر + سبد خرید)
    commands.spawn((
        ShopHoverTooltipRoot,
        Pickable::IGNORE,
        GlobalZIndex(270),
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            top: px(0.0),
            width: px(sui.tooltip_width_px),
            padding: UiRect::all(px(12.0)),
            flex_direction: FlexDirection::Column,
            row_gap: px(5.0),
            border: UiRect::all(px(1.5)),
            border_radius: BorderRadius::all(px(8.0)),
            display: Display::None,
            ..default()
        },
        BorderColor::all(Color::srgb(0.32, 0.44, 0.58)),
        BackgroundColor(Color::srgba(0.04, 0.06, 0.10, 0.98)),
    ));

    // ۷. شبح درگ و دراپ آیتم (پشتیبانی از جابجایی اسلات و انتقال هیرو <-> کوریر)
    commands
        .spawn((
            DragGhostRoot,
            Pickable::IGNORE,
            GlobalZIndex(275),
            Node {
                position_type: PositionType::Absolute,
                min_width: px(50.0),
                height: px(46.0),
                padding: UiRect::axes(px(6.0), px(4.0)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::all(px(6.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgb(1.0, 0.88, 0.35)),
            BackgroundColor(Color::srgba(0.10, 0.14, 0.22, 0.94)),
        ))
        .with_children(|g| {
            g.spawn((
                DragGhostLabelText,
                Pickable::IGNORE,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            g.spawn((
                DragGhostTargetText,
                Pickable::IGNORE,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(8.5),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.95, 1.0)),
            ));
        });
}

#[derive(SystemParam)]
struct ShopInteractionParams<'w, 's> {
    close_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<ShopCloseBtn>, Without<ShopPanelCard>, Without<HudShopToggleBtn>)>,
    search_box_q: Query<'w, 's, Entity, (Added<Pressed>, With<ShopSearchBoxBtn>)>,
    search_clr_q: Query<'w, 's, Entity, (Added<Pressed>, With<ShopSearchClearBtn>)>,
    cat_btn_q: Query<'w, 's, &'static ShopCategoryBtn, Added<Pressed>>,
    tier_btn_q: Query<'w, 's, &'static ShopTierFilterBtn, Added<Pressed>>,
    afford_btn_q: Query<'w, 's, Entity, (Added<Pressed>, With<ShopAffordableToggleBtn>)>,
    select_btn_q: Query<'w, 's, &'static ShopSelectItemBtn, Added<Pressed>>,
    select_hover_q: Query<'w, 's, (&'static Hovered, &'static ShopSelectItemBtn)>,
    recipe_btn_q: Query<'w, 's, &'static ShopRecipeNodeBtn, Added<Pressed>>,
    buy_now_q: Query<'w, 's, &'static ShopBuyNowBtn, Added<Pressed>>,
    add_cart_q: Query<'w, 's, &'static ShopAddToCartBtn, Added<Pressed>>,
    modal_q: Query<'w, 's, &'static mut Node, With<ShopModalRoot>>,
    panel_rel_q: Query<'w, 's, &'static RelativeCursorPosition, With<ShopPanelCard>>,
    toggle_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudShopToggleBtn>, Without<ShopPanelCard>)>,
    hud_root_q: Query<'w, 's, &'static mut Node, (With<HudInventoryRoot>, Without<ShopModalRoot>)>,
    catalog_scroll_q: Query<'w, 's, &'static mut ScrollPosition, With<ShopCatalogGrid>>,
}

fn add_item_expanded_to_cart(
    items_cfg: &ItemsConfig,
    item_id: &str,
    ctx: &ShopContextSnapshot,
    cart: &mut Vec<String>,
    override_cart: bool,
) {
    if override_cart {
        cart.clear();
    }
    let mut sim_inv = ctx.inventory.clone();
    sim_inv.extend(ctx.courier_inventory.iter().cloned());
    sim_inv.extend(ctx.stash.iter().cloned());
    for existing in cart.iter() {
        sim_inv.push(InventoryItemState {
            item_id: existing.clone(),
            active_mode: 0,
            slot: 0,
            count: 1,
        });
    }
    let expanded = items_cfg.expand_missing_recipe_items(item_id, &sim_inv);
    cart.extend(expanded);
}

fn prune_newly_bought_from_cart(
    items_cfg: &ItemsConfig,
    prev_inv: &[InventoryItemState],
    curr_inv: &[InventoryItemState],
    cart: &mut Vec<String>,
) {
    let mut prev_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for it in prev_inv {
        *prev_counts.entry(it.item_id.as_str()).or_insert(0) += 1;
    }
    let mut curr_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for it in curr_inv {
        *curr_counts.entry(it.item_id.as_str()).or_insert(0) += 1;
    }
    for (id, &cnt) in &curr_counts {
        let old_cnt = prev_counts.get(id).copied().unwrap_or(0);
        if cnt > old_cnt {
            let gained = cnt - old_cnt;
            for _ in 0..gained {
                if let Some(pos) = cart.iter().position(|c| c.as_str() == *id) {
                    cart.remove(pos);
                }
                // حتی اگر خود آیتم ترکیبی در سبد نبود (بلکه recipe:<id> و کامپوننت‌ها در سبد بودند)، آن‌ها را پاک کن
                if let Some(def) = items_cfg.get_item(id) {
                    if !def.recipe.is_empty() {
                        let rec_id = recipe_item_id(id);
                        if let Some(rpos) = cart.iter().position(|c| c == &rec_id) {
                            cart.remove(rpos);
                        }
                        for comp_id in &def.recipe {
                            if let Some(cpos) = cart.iter().position(|c| c == comp_id) {
                                cart.remove(cpos);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn try_send_buy_item(
    item_id: &str,
    items_cfg: &ItemsConfig,
    ctx: &ShopContextSnapshot,
    outgoing: &mut ShopOutgoingQueue,
    audio_q: &mut AudioEventQueue,
) -> bool {
    if let Some(target_id) = parse_recipe_target(item_id) {
        return try_send_buy_recipe(target_id, items_cfg, ctx, outgoing, audio_q);
    }
    let (eff_cost, _) = effective_cost_for_display(items_cfg, item_id, ctx);
    if ctx.gold < eff_cost {
        audio_q.0.push(ClientAudioEvent::NotEnoughGold);
        return false;
    }
    outgoing.0.push(ClientLobbyMessage::BuyItem {
        item_id: item_id.to_string(),
    });
    true
}

fn try_send_buy_recipe(
    target_item_id: &str,
    items_cfg: &ItemsConfig,
    ctx: &ShopContextSnapshot,
    outgoing: &mut ShopOutgoingQueue,
    audio_q: &mut AudioEventQueue,
) -> bool {
    let fee = items_cfg.recipe_fee(target_item_id);
    if fee == 0 || ctx.gold < fee {
        audio_q.0.push(ClientAudioEvent::NotEnoughGold);
        return false;
    }
    outgoing.0.push(ClientLobbyMessage::BuyRecipe {
        item_id: target_item_id.to_string(),
    });
    true
}

fn shop_hotkey_and_button_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut audio_q: ResMut<AudioEventQueue>,
    mut prev_inv_ready: Local<bool>,
    mut prev_in_fountain: Local<Option<bool>>,
    mut p: ShopInteractionParams,
) {
    let was_open = ui_state.is_open;

    if ui_state.selected_item_id.is_empty() {
        if let Some(first) = items_cfg.items.first() {
            ui_state.selected_item_id = first.id.clone();
            ui_state.dirty = true;
        }
    }

    if !ctx.in_game {
        *prev_inv_ready = false;
        *prev_in_fountain = None;
    } else if !ctx.is_dead {
        if let Some(was_in) = *prev_in_fountain {
            if !was_in && ctx.in_fountain {
                audio_q.0.push(ClientAudioEvent::ShopEnter);
            } else if was_in && !ctx.in_fountain {
                audio_q.0.push(ClientAudioEvent::ShopExit);
            }
        }
        *prev_in_fountain = Some(ctx.in_fountain);
    }

    if ui_state.prev_inventory != ctx.inventory
        || ui_state.prev_courier_inventory != ctx.courier_inventory
        || ui_state.prev_stash != ctx.stash
    {
        let mut prev_all = ui_state.prev_inventory.clone();
        prev_all.extend(ui_state.prev_courier_inventory.iter().cloned());
        prev_all.extend(ui_state.prev_stash.iter().cloned());
        let mut curr_all = ctx.inventory.clone();
        curr_all.extend(ctx.courier_inventory.iter().cloned());
        curr_all.extend(ctx.stash.iter().cloned());

        prune_newly_bought_from_cart(&items_cfg, &prev_all, &curr_all, &mut ui_state.cart);

        if ctx.in_game && *prev_inv_ready {
            let mut crafted_now = false;
            let mut gained_any_item = false;
            for item in &curr_all {
                let new_cnt: u32 = curr_all
                    .iter()
                    .filter(|x| x.item_id == item.item_id)
                    .map(|x| x.count)
                    .sum();
                let old_cnt: u32 = prev_all
                    .iter()
                    .filter(|x| x.item_id == item.item_id)
                    .map(|x| x.count)
                    .sum();
                if new_cnt > old_cnt {
                    gained_any_item = true;
                    if let Some(def) = items_cfg.get_item(&item.item_id) {
                        if !def.recipe.is_empty() {
                            crafted_now = true;
                            break;
                        }
                    }
                }
            }
            if crafted_now {
                audio_q.0.push(ClientAudioEvent::ShopCombine);
            } else if gained_any_item {
                audio_q.0.push(ClientAudioEvent::ShopBuy);
            }
        }
        if ctx.in_game {
            *prev_inv_ready = true;
        }
        ui_state.prev_inventory = ctx.inventory.clone();
        ui_state.prev_courier_inventory = ctx.courier_inventory.clone();
        ui_state.prev_stash = ctx.stash.clone();
        ui_state.dirty = true;
    } else if ctx.in_game {
        *prev_inv_ready = true;
    }

    if let Ok(mut hud_node) = p.hud_root_q.single_mut() {
        let desired_hud = if ctx.in_game { Display::Flex } else { Display::None };
        if hud_node.display != desired_hud {
            hud_node.display = desired_hud;
        }
    }

    if !ctx.in_game {
        if ui_state.is_open {
            ui_state.is_open = false;
            ui_state.search_focused = false;
        }
        if ui_state.is_open != was_open {
            audio_q.0.push(if ui_state.is_open {
                ClientAudioEvent::ShopOpen
            } else {
                ClientAudioEvent::ShopClose
            });
        }
        if let Ok(mut m_node) = p.modal_q.single_mut() {
            if m_node.display != Display::None {
                m_node.display = Display::None;
            }
        }
        return;
    }

    if !ctx.chat_open && !ui_state.search_focused {
        if keyboard.just_pressed(KeyCode::KeyB) {
            ui_state.is_open = !ui_state.is_open;
            if !ui_state.is_open {
                ui_state.search_focused = false;
            }
            ui_state.dirty = true;
        } else if ui_state.is_open && keyboard.just_pressed(KeyCode::Escape) {
            ui_state.is_open = false;
            ui_state.search_focused = false;
            ui_state.dirty = true;
        } else if ui_state.is_open && keyboard.just_pressed(KeyCode::Slash) {
            ui_state.search_focused = true;
            ui_state.dirty = true;
        }
    }

    let mut handled_shop_toggle_click = false;
    if mouse.just_pressed(MouseButton::Left) {
        if p.toggle_rel_q.single().is_ok_and(is_cursor_inside) {
            ui_state.is_open = !ui_state.is_open;
            ui_state.search_focused = false;
            ui_state.suppress_world_click_this_frame = true;
            ui_state.dirty = true;
            handled_shop_toggle_click = true;
        } else if ui_state.is_open && p.close_rel_q.single().is_ok_and(is_cursor_inside) {
            ui_state.is_open = false;
            ui_state.search_focused = false;
            ui_state.suppress_world_click_this_frame = true;
            ui_state.dirty = true;
            handled_shop_toggle_click = true;
        }
    }

    for _ in &p.search_box_q {
        ui_state.search_focused = true;
        ui_state.dirty = true;
    }
    for _ in &p.search_clr_q {
        ui_state.search_query.clear();
        ui_state.search_focused = false;
        if let Ok(mut sp) = p.catalog_scroll_q.single_mut() {
            sp.y = 0.0;
        }
        ui_state.dirty = true;
    }

    for cat_btn in &p.cat_btn_q {
        ui_state.active_category = cat_btn.0;
        ui_state.search_focused = false;
        if let Ok(mut sp) = p.catalog_scroll_q.single_mut() {
            sp.y = 0.0;
        }
        ui_state.dirty = true;
    }

    for tier_btn in &p.tier_btn_q {
        ui_state.tier_filter = tier_btn.0;
        ui_state.search_focused = false;
        if let Ok(mut sp) = p.catalog_scroll_q.single_mut() {
            sp.y = 0.0;
        }
        ui_state.dirty = true;
    }

    for _ in &p.afford_btn_q {
        ui_state.only_affordable = !ui_state.only_affordable;
        ui_state.search_focused = false;
        if let Ok(mut sp) = p.catalog_scroll_q.single_mut() {
            sp.y = 0.0;
        }
        ui_state.dirty = true;
    }

    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl_held =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    for sel_btn in &p.select_btn_q {
        ui_state.search_focused = false;
        ui_state.selected_item_id = sel_btn.0.clone();
        if shift_held {
            add_item_expanded_to_cart(&items_cfg, &sel_btn.0, &ctx, &mut ui_state.cart, !ctrl_held);
            audio_q.0.push(ClientAudioEvent::ShopAddCart);
        }
        ui_state.dirty = true;
    }

    // کلیک راست روی هر آیتم داخل شاپ -> خرید سریع (Quick Buy مثل Dota 2)
    if ui_state.is_open && mouse.just_pressed(MouseButton::Right) {
        if let Some(hovered_id) = p
            .select_hover_q
            .iter()
            .find_map(|(hovered, btn)| hovered.get().then(|| btn.0.clone()))
        {
            if try_send_buy_item(&hovered_id, &items_cfg, &ctx, &mut outgoing, &mut audio_q) {
                ui_state.dirty = true;
            }
        }
    }

    // کلیک روی نود اسکرول Recipe در هرم -> خرید صرفاً اسکرول Recipe با قیمت recipe_fee
    for rec_btn in &p.recipe_btn_q {
        ui_state.search_focused = false;
        if try_send_buy_recipe(&rec_btn.0, &items_cfg, &ctx, &mut outgoing, &mut audio_q) {
            ui_state.dirty = true;
        }
    }

    for buy_btn in &p.buy_now_q {
        ui_state.search_focused = false;
        if try_send_buy_item(&buy_btn.0, &items_cfg, &ctx, &mut outgoing, &mut audio_q) {
            ui_state.dirty = true;
        }
    }

    for add_btn in &p.add_cart_q {
        ui_state.search_focused = false;
        add_item_expanded_to_cart(&items_cfg, &add_btn.0, &ctx, &mut ui_state.cart, !ctrl_held);
        audio_q.0.push(ClientAudioEvent::ShopAddCart);
        ui_state.dirty = true;
    }

    if was_open
        && ui_state.is_open
        && !handled_shop_toggle_click
        && (mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right))
    {
        let over_panel = p.panel_rel_q.single().is_ok_and(is_cursor_inside);
        let over_toggle = p.toggle_rel_q.single().is_ok_and(is_cursor_inside);
        if !over_panel && !over_toggle {
            ui_state.is_open = false;
            ui_state.search_focused = false;
            ui_state.suppress_world_click_this_frame = true;
        }
    }

    if ui_state.is_open != was_open {
        audio_q.0.push(if ui_state.is_open {
            ClientAudioEvent::ShopOpen
        } else {
            ClientAudioEvent::ShopClose
        });
    }

    if let Ok(mut m_node) = p.modal_q.single_mut() {
        let desired_modal = if ui_state.is_open { Display::Flex } else { Display::None };
        if m_node.display != desired_modal {
            m_node.display = desired_modal;
            if ui_state.is_open {
                ui_state.dirty = true;
            }
        }
    }
}

fn shop_search_input_system(
    mut key_events: MessageReader<KeyboardInput>,
    mut ui_state: ResMut<ShopUiState>,
    mut catalog_scroll_q: Query<&mut ScrollPosition, With<ShopCatalogGrid>>,
) {
    if !ui_state.is_open || !ui_state.search_focused {
        key_events.clear();
        return;
    }

    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match ev.logical_key {
            Key::Escape | Key::Enter => {
                ui_state.search_focused = false;
                ui_state.dirty = true;
                break;
            }
            Key::Backspace => {
                if ui_state.search_query.pop().is_some() {
                    if let Ok(mut sp) = catalog_scroll_q.single_mut() {
                        sp.y = 0.0;
                    }
                    ui_state.dirty = true;
                }
            }
            Key::Space => {
                if ui_state.search_query.chars().count() < 32 {
                    ui_state.search_query.push(' ');
                    if let Ok(mut sp) = catalog_scroll_q.single_mut() {
                        sp.y = 0.0;
                    }
                    ui_state.dirty = true;
                }
            }
            Key::Character(ref s) => {
                let mut changed = false;
                for ch in s.chars() {
                    if ch.is_control() || ch == '/' || ui_state.search_query.chars().count() >= 32 {
                        continue;
                    }
                    ui_state.search_query.push(ch);
                    changed = true;
                }
                if changed {
                    if let Ok(mut sp) = catalog_scroll_q.single_mut() {
                        sp.y = 0.0;
                    }
                    ui_state.dirty = true;
                }
            }
            _ => {}
        }
    }
}

fn shop_scroll_system(
    mut wheel_events: MessageReader<MouseWheel>,
    ui_state: Res<ShopUiState>,
    mut scroll_q: Query<
        (&mut ScrollPosition, &ComputedNode, &RelativeCursorPosition),
        With<ShopScrollablePanel>,
    >,
) {
    if !ui_state.is_open {
        wheel_events.clear();
        return;
    }

    let mut delta_y = 0.0_f32;
    for ev in wheel_events.read() {
        let step = match ev.unit {
            MouseScrollUnit::Line => ev.y * 28.0,
            MouseScrollUnit::Pixel => ev.y,
        };
        delta_y += step;
    }

    if delta_y.abs() < 0.01 {
        return;
    }

    for (mut scroll, computed, rel_cursor) in &mut scroll_q {
        if is_cursor_inside(rel_cursor) {
            let max_y = (computed.content_size().y - computed.size().y).max(0.0);
            scroll.y = (scroll.y - delta_y).clamp(0.0, max_y);
        }
    }
}

fn clear_children(commands: &mut Commands, children_opt: Option<&Children>) {
    if let Some(children) = children_opt {
        for c in children.iter() {
            commands.entity(c).despawn();
        }
    }
}

/// نود گرافیکی هرمی برای یک آیتم در درخت ساخت Dota 2 (حداقل متن، آیکون + قیمت/تیک مالکیت، جزئیات در تولتیپ هاور)
#[allow(clippy::too_many_arguments)]
fn spawn_pyramid_item_tile(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    icon_cache: &mut ItemIconCache,
    item: &ItemDef,
    icon_size: f32,
    is_root: bool,
    owned: bool,
    display_cost: u32,
    affordable: bool,
) {
    let border_col = if is_root {
        Color::srgb(0.98, 0.82, 0.28)
    } else if owned {
        Color::srgb(0.24, 0.90, 0.48)
    } else if affordable {
        Color::srgba(0.42, 0.56, 0.72, 0.88)
    } else {
        Color::srgba(0.30, 0.34, 0.42, 0.72)
    };
    let bg_col = if owned {
        Color::srgba(0.06, 0.24, 0.12, 0.95)
    } else if is_root {
        Color::srgba(0.16, 0.15, 0.08, 0.96)
    } else {
        Color::srgba(0.08, 0.11, 0.16, 0.94)
    };

    parent
        .spawn((
            UiButton,
            Hovered::default(),
            ShopSelectItemBtn(item.id.clone()),
            Node {
                padding: UiRect::axes(px(5.0), px(4.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                border: UiRect::all(px(if is_root { 2.0 } else { 1.5 })),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BorderColor::all(border_col),
            BackgroundColor(bg_col),
        ))
        .with_children(|tile| {
            spawn_item_icon_badge(
                tile,
                asset_server,
                icon_cache,
                item,
                icon_size,
                (icon_size * 0.26).clamp(8.0, 13.0),
            );
            let badge_txt = if owned {
                "OWNED".to_string()
            } else {
                format!("{}g", display_cost)
            };
            let badge_col = if owned {
                Color::srgb(0.32, 0.98, 0.58)
            } else if affordable {
                Color::srgb(1.0, 0.86, 0.32)
            } else {
                Color::srgb(0.68, 0.44, 0.42)
            };
            tile.spawn((
                Pickable::IGNORE,
                Text::new(badge_txt),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(badge_col),
            ));
        });
}

/// نود گرافیکی اسکرول Recipe برای نمایش شفاف هزینه ترکیب (Recipe Fee) در هرم
fn spawn_pyramid_recipe_tile(
    parent: &mut ChildSpawnerCommands,
    parent_item_id: &str,
    fee: u32,
    icon_size: f32,
    affordable: bool,
) {
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            ShopRecipeNodeBtn(parent_item_id.to_string()),
            Node {
                padding: UiRect::axes(px(5.0), px(4.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0),
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.95, 0.72, 0.24)),
            BackgroundColor(Color::srgba(0.22, 0.16, 0.05, 0.95)),
        ))
        .with_children(|tile| {
            tile.spawn((
                Pickable::IGNORE,
                Node {
                    width: px(icon_size),
                    height: px(icon_size),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(4.0)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.98, 0.82, 0.38, 0.85)),
                BackgroundColor(Color::srgba(0.34, 0.24, 0.08, 0.95)),
            ))
            .with_children(|b| {
                b.spawn((
                    Pickable::IGNORE,
                    Text::new("REC"),
                    TextFont {
                        font_size: FontSize::Px((icon_size * 0.27).clamp(8.0, 12.0)),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.92, 0.56)),
                ));
            });
            tile.spawn((
                Pickable::IGNORE,
                Text::new(format!("{}g", fee)),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(if affordable {
                    Color::srgb(1.0, 0.88, 0.36)
                } else {
                    Color::srgb(0.70, 0.45, 0.42)
                }),
            ));
        });
}

/// رندر خط‌کشی انشعاب افقی/عمودی بالای هر فرزند در هرم
fn spawn_branch_connector_header(
    parent: &mut ChildSpawnerCommands,
    idx: usize,
    total: usize,
    line_px: f32,
    line_col: Color,
) {
    if total > 1 {
        parent
            .spawn((
                Pickable::IGNORE,
                Node {
                    width: percent(100),
                    height: px(line_px),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
            ))
            .with_children(|bar| {
                bar.spawn((
                    Pickable::IGNORE,
                    Node {
                        width: percent(50),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(if idx == 0 { Color::NONE } else { line_col }),
                ));
                bar.spawn((
                    Pickable::IGNORE,
                    Node {
                        width: percent(50),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(if idx + 1 == total { Color::NONE } else { line_col }),
                ));
            });
    }
    parent.spawn((
        Pickable::IGNORE,
        Node {
            width: px(line_px),
            height: px(10.0),
            ..default()
        },
        BackgroundColor(line_col),
    ));
}

#[derive(SystemParam)]
struct ShopRebuildQueries<'w, 's> {
    gold_q: Query<'w, 's, &'static mut Text, With<ShopHeaderGoldText>>,
    ftn_badge_q: Query<'w, 's, (&'static mut BorderColor, &'static mut BackgroundColor), With<ShopFountainBadge>>,
    ftn_txt_q: Query<'w, 's, (&'static mut Text, &'static mut TextColor), (With<ShopFountainBadgeText>, Without<ShopHeaderGoldText>)>,
    search_box_q: Query<'w, 's, &'static mut BorderColor, (With<ShopSearchBoxBtn>, Without<ShopFountainBadge>)>,
    search_txt_q: Query<'w, 's, (&'static mut Text, &'static mut TextColor), (With<ShopSearchText>, Without<ShopHeaderGoldText>, Without<ShopFountainBadgeText>)>,
    tabs_q: Query<'w, 's, (Entity, Option<&'static Children>), With<ShopCategoryTabsRow>>,
    quick_q: Query<'w, 's, (Entity, Option<&'static Children>), With<ShopQuickFilterRow>>,
    grid_q: Query<'w, 's, (Entity, Option<&'static Children>), With<ShopCatalogGrid>>,
    insp_q: Query<'w, 's, (Entity, Option<&'static Children>), With<ShopInspectorColumn>>,
}

#[allow(clippy::too_many_arguments)]
fn spawn_catalog_section(
    grid: &mut ChildSpawnerCommands,
    title: &str,
    items: &[&ItemDef],
    asset_server: &AssetServer,
    icon_cache: &mut ItemIconCache,
    items_cfg: &ItemsConfig,
    ctx: &ShopContextSnapshot,
    selected_item_id: &str,
) {
    if items.is_empty() {
        return;
    }
    grid.spawn((
        Pickable::IGNORE,
        Text::new(format!("{} ({})", title, items.len())),
        TextFont {
            font_size: FontSize::Px(10.5),
            ..default()
        },
        TextColor(Color::srgb(0.66, 0.78, 0.92)),
    ));

    grid.spawn(Node {
        width: percent(100),
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_content: AlignContent::FlexStart,
        column_gap: px(6.0),
        row_gap: px(6.0),
        ..default()
    })
    .with_children(|row| {
        for item in items {
            let is_sel = selected_item_id == item.id;
            let owned_count = ctx.inventory.iter().filter(|i| i.item_id == item.id).count()
                + ctx.courier_inventory.iter().filter(|i| i.item_id == item.id).count();
            let (eff_cost, consumed_cnt) = effective_cost_for_display(items_cfg, &item.id, ctx);
            let affordable = ctx.gold >= eff_cost;

            row.spawn((
                UiButton,
            Hovered::default(),
                ShopSelectItemBtn(item.id.clone()),
                Node {
                    width: px(62.0),
                    padding: UiRect::axes(px(4.0), px(5.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(3.0),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(if is_sel { 2.0 } else { 1.0 })),
                    border_radius: BorderRadius::all(px(6.0)),
                    ..default()
                },
                BorderColor::all(if is_sel {
                    Color::srgb(0.98, 0.82, 0.26)
                } else if owned_count > 0 {
                    Color::srgba(0.24, 0.85, 0.48, 0.80)
                } else {
                    Color::srgba(0.22, 0.28, 0.38, 0.65)
                }),
                BackgroundColor(if is_sel {
                    Color::srgba(0.18, 0.22, 0.30, 0.96)
                } else {
                    Color::srgba(0.08, 0.11, 0.16, 0.92)
                }),
            ))
            .with_children(|card| {
                spawn_item_icon_badge(card, asset_server, icon_cache, item, 42.0, 11.0);
                let price_txt = if consumed_cnt > 0 {
                    format!("{}*", eff_cost)
                } else {
                    format!("{}", eff_cost)
                };
                card.spawn((
                    Pickable::IGNORE,
                    Text::new(price_txt),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(if affordable {
                        Color::srgb(1.0, 0.84, 0.28)
                    } else {
                        Color::srgb(0.65, 0.42, 0.40)
                    }),
                ));
            });
        }
    });
}

fn rebuild_shop_modal_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    mut q: ShopRebuildQueries,
) {
    if !ui_state.is_open {
        return;
    }
    let header_dirty = ctx.is_changed();
    if !ui_state.dirty && !header_dirty {
        return;
    }
    let full_rebuild = ui_state.dirty || header_dirty;
    if ui_state.dirty {
        ui_state.bypass_change_detection().dirty = false;
    }

    if let Ok(mut g_txt) = q.gold_q.single_mut() {
        **g_txt = format!("GOLD: {}", ctx.gold);
    }

    let hero_in_shop = ctx.in_fountain && !ctx.is_dead;
    let courier_in_shop = ctx.courier_in_fountain && !ctx.courier_is_dead;
    if let Ok((mut b_col, mut bg_col)) = q.ftn_badge_q.single_mut() {
        if hero_in_shop {
            *b_col = BorderColor::all(Color::srgb(0.24, 0.88, 0.48));
            *bg_col = BackgroundColor(Color::srgba(0.06, 0.24, 0.12, 0.9));
        } else if courier_in_shop {
            *b_col = BorderColor::all(Color::srgb(0.32, 0.78, 0.98));
            *bg_col = BackgroundColor(Color::srgba(0.06, 0.18, 0.28, 0.9));
        } else {
            *b_col = BorderColor::all(Color::srgb(0.95, 0.76, 0.26));
            *bg_col = BackgroundColor(Color::srgba(0.24, 0.18, 0.05, 0.9));
        }
    }
    if let Ok((mut f_txt, mut f_col)) = q.ftn_txt_q.single_mut() {
        if hero_in_shop {
            **f_txt = "HERO IN SHOP -> HERO / STASH".to_string();
            *f_col = TextColor(Color::srgb(0.35, 0.98, 0.60));
        } else if courier_in_shop {
            **f_txt = "COURIER IN SHOP -> DRONE / STASH".to_string();
            *f_col = TextColor(Color::srgb(0.45, 0.90, 1.0));
        } else {
            **f_txt = "REMOTE BUY -> STASH".to_string();
            *f_col = TextColor(Color::srgb(1.0, 0.88, 0.38));
        }
    }

    if let Ok(mut sb_border) = q.search_box_q.single_mut() {
        *sb_border = BorderColor::all(if ui_state.search_focused {
            Color::srgb(0.96, 0.82, 0.28)
        } else {
            Color::srgba(0.35, 0.48, 0.62, 0.8)
        });
    }
    if let Ok((mut s_txt, mut s_col)) = q.search_txt_q.single_mut() {
        if ui_state.search_query.is_empty() {
            **s_txt = if ui_state.search_focused {
                "Type name or stat... |".to_string()
            } else {
                "Search name or stat (armor, speed, regen...)".to_string()
            };
            *s_col = TextColor(Color::srgba(0.60, 0.68, 0.78, 0.85));
        } else {
            **s_txt = if ui_state.search_focused {
                format!("{}|", ui_state.search_query)
            } else {
                ui_state.search_query.clone()
            };
            *s_col = TextColor(Color::WHITE);
        }
    }

    if !full_rebuild {
        return;
    }

    // ۱. تب‌های دسته‌بندی ساده
    if let Ok((tabs_ent, ch)) = q.tabs_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(tabs_ent).with_children(|row| {
            for &cat in ShopCategoryFilter::all() {
                let active = ui_state.active_category == cat;
                row.spawn((
                    UiButton,
            Hovered::default(),
                    ShopCategoryBtn(cat),
                    Node {
                        padding: UiRect::axes(px(9.0), px(4.5)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BorderColor::all(if active {
                        Color::srgb(0.95, 0.80, 0.28)
                    } else {
                        Color::srgba(0.32, 0.40, 0.52, 0.7)
                    }),
                    BackgroundColor(if active {
                        Color::srgba(0.26, 0.22, 0.08, 0.95)
                    } else {
                        Color::srgba(0.08, 0.11, 0.16, 0.9)
                    }),
                ))
                .with_children(|b| {
                    b.spawn((
                        Pickable::IGNORE,
                        Text::new(cat.label()),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(if active {
                            Color::srgb(1.0, 0.90, 0.40)
                        } else {
                            Color::srgb(0.84, 0.90, 0.96)
                        }),
                    ));
                });
            }
        });
    }

    // ۱.۵ فیلترهای سریع (Basic / Upgrades / Affordable)
    if let Ok((quick_ent, ch)) = q.quick_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(quick_ent).with_children(|row| {
            for &tf in ShopTierFilter::all() {
                let active = ui_state.tier_filter == tf;
                row.spawn((
                    UiButton,
            Hovered::default(),
                    ShopTierFilterBtn(tf),
                    Node {
                        padding: UiRect::axes(px(8.0), px(3.0)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(4.0)),
                        ..default()
                    },
                    BorderColor::all(if active {
                        Color::srgb(0.40, 0.82, 1.0)
                    } else {
                        Color::srgba(0.26, 0.34, 0.44, 0.7)
                    }),
                    BackgroundColor(if active {
                        Color::srgba(0.08, 0.20, 0.32, 0.95)
                    } else {
                        Color::srgba(0.06, 0.09, 0.14, 0.88)
                    }),
                ))
                .with_children(|b| {
                    b.spawn((
                        Pickable::IGNORE,
                        Text::new(tf.label()),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(if active {
                            Color::srgb(0.65, 0.94, 1.0)
                        } else {
                            Color::srgb(0.75, 0.82, 0.90)
                        }),
                    ));
                });
            }

            let aff = ui_state.only_affordable;
            row.spawn((
                UiButton,
            Hovered::default(),
                ShopAffordableToggleBtn,
                Node {
                    padding: UiRect::axes(px(8.0), px(3.0)),
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(4.0)),
                    ..default()
                },
                BorderColor::all(if aff {
                    Color::srgb(0.24, 0.92, 0.52)
                } else {
                    Color::srgba(0.26, 0.34, 0.44, 0.7)
                }),
                BackgroundColor(if aff {
                    Color::srgba(0.06, 0.26, 0.14, 0.95)
                } else {
                    Color::srgba(0.06, 0.09, 0.14, 0.88)
                }),
            ))
            .with_children(|b| {
                b.spawn((
                    Pickable::IGNORE,
                    Text::new(if aff { "Affordable: ON" } else { "Affordable" }),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(if aff {
                        Color::srgb(0.40, 0.98, 0.62)
                    } else {
                        Color::srgb(0.75, 0.82, 0.90)
                    }),
                ));
            });
        });
    }

    // ۲. بازسازی کاتالوگ گروه‌بندی‌شده (Basic و Upgrades مرتب بر اساس قیمت)
    let query_lower = ui_state.search_query.trim().to_lowercase();
    if let Ok((grid_ent, ch)) = q.grid_q.single() {
        clear_children(&mut commands, ch);
        let mut basics: Vec<&ItemDef> = Vec::new();
        let mut upgrades: Vec<&ItemDef> = Vec::new();
        for item in &items_cfg.items {
            if !ui_state.active_category.matches(item.category) {
                continue;
            }
            if !ui_state.tier_filter.matches(item) {
                continue;
            }
            if !item_matches_search(item, &query_lower) {
                continue;
            }
            if ui_state.only_affordable {
                let (eff_cost, _) = effective_cost_for_display(&items_cfg, &item.id, &ctx);
                if ctx.gold < eff_cost {
                    continue;
                }
            }
            if item.recipe.is_empty() {
                basics.push(item);
            } else {
                upgrades.push(item);
            }
        }
        basics.sort_by_key(|it| it.cost);
        upgrades.sort_by_key(|it| (it.tier, it.cost));

        commands.entity(grid_ent).with_children(|grid| {
            if basics.is_empty() && upgrades.is_empty() {
                grid.spawn((
                    Pickable::IGNORE,
                    Text::new("No matching items. Try clearing search or filters."),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(Color::srgba(0.65, 0.72, 0.82, 0.85)),
                ));
            } else {
                spawn_catalog_section(
                    grid,
                    "BASIC ITEMS",
                    &basics,
                    &asset_server,
                    &mut icon_cache,
                    &items_cfg,
                    &ctx,
                    &ui_state.selected_item_id,
                );
                spawn_catalog_section(
                    grid,
                    "UPGRADES",
                    &upgrades,
                    &asset_server,
                    &mut icon_cache,
                    &items_cfg,
                    &ctx,
                    &ui_state.selected_item_id,
                );
            }
        });
    }

    // ۳. بازسازی ستون راست: درخت هرمی Dota 2 (حداقل متن، توضیحات کامل در پاپ‌آپ هاور)
    if let Ok((insp_ent, ch)) = q.insp_q.single() {
        clear_children(&mut commands, ch);
        if let Some(item) = items_cfg.get_item(&ui_state.selected_item_id) {
            let sui = &items_cfg.shop_ui;
            let line_px = sui.pyramid_connector_thickness_px;
            let node_px = sui.pyramid_node_size_px;
            let line_col = Color::srgba(0.55, 0.70, 0.88, 0.78);
            let upg_line_col = Color::srgba(0.92, 0.76, 0.30, 0.72);

            let (eff_cost, consumed_cnt) = effective_cost_for_display(&items_cfg, &item.id, &ctx);
            let can_buy = ctx.gold >= eff_cost;

            let mut owned_pool: Vec<String> = ctx.inventory.iter().map(|i| i.item_id.clone()).collect();
            owned_pool.extend(ctx.courier_inventory.iter().map(|i| i.item_id.clone()));
            owned_pool.extend(ctx.stash.iter().map(|i| i.item_id.clone()));

            commands.entity(insp_ent).with_children(|col| {
                // هدر فشرده آیتم انتخاب‌شده
                col.spawn((
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::bottom(px(6.0)),
                        border: UiRect::bottom(px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.24, 0.34, 0.46, 0.65)),
                ))
                .with_children(|hdr| {
                    hdr.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|left| {
                        spawn_item_icon_badge(left, &asset_server, &mut icon_cache, item, 40.0, 12.0);
                        left.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(1.0),
                            ..default()
                        })
                        .with_children(|meta| {
                            meta.spawn((
                                Text::new(&item.name),
                                TextFont {
                                    font_size: FontSize::Px(15.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                            meta.spawn((
                                Text::new(format!(
                                    "{} | T{} | {}g  (Hover nodes for stats)",
                                    item.category.label(),
                                    item.tier,
                                    item.cost
                                )),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.88, 0.80, 0.45)),
                            ));
                        });
                    });
                });

                // ناحیه هرم Dota 2
                col.spawn(Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(4.0), px(8.0)),
                    ..default()
                })
                .with_children(|pyr| {
                    // ردیف بالایی: Builds Into
                    let upgrades = items_cfg.builds_into(&item.id);
                    if !upgrades.is_empty() {
                        pyr.spawn((
                            Pickable::IGNORE,
                            Text::new("BUILDS INTO"),
                            TextFont {
                                font_size: FontSize::Px(9.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.78, 0.36)),
                        ));
                        pyr.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::Center,
                            column_gap: px(6.0),
                            row_gap: px(4.0),
                            margin: UiRect::top(px(4.0)),
                            ..default()
                        })
                        .with_children(|urow| {
                            for upg in &upgrades {
                                let (u_cost, _) = effective_cost_for_display(&items_cfg, &upg.id, &ctx);
                                let u_owned = owned_pool.iter().any(|x| x == &upg.id);
                                spawn_pyramid_item_tile(
                                    urow,
                                    &asset_server,
                                    &mut icon_cache,
                                    upg,
                                    (node_px * 0.74).max(28.0),
                                    false,
                                    u_owned,
                                    u_cost,
                                    ctx.gold >= u_cost,
                                );
                            }
                        });
                        pyr.spawn((
                            Pickable::IGNORE,
                            Node {
                                width: px(line_px),
                                height: px(14.0),
                                ..default()
                            },
                            BackgroundColor(upg_line_col),
                        ));
                    }

                    // راس هرم: آیتم اصلی انتخاب‌شده
                    let root_owned = owned_pool.iter().any(|x| x == &item.id);
                    spawn_pyramid_item_tile(
                        pyr,
                        &asset_server,
                        &mut icon_cache,
                        item,
                        node_px,
                        true,
                        root_owned,
                        eff_cost,
                        ctx.gold >= eff_cost,
                    );

                    // شاخه‌های پایین هرم: کامپوننت‌های مورد نیاز + اسکرول Recipe
                    let rec_fee = items_cfg.recipe_fee(&item.id);
                    if !item.recipe.is_empty() {
                        pyr.spawn((
                            Pickable::IGNORE,
                            Node {
                                width: px(line_px),
                                height: px(14.0),
                                ..default()
                            },
                            BackgroundColor(line_col),
                        ));

                        let total_branches = item.recipe.len() + usize::from(rec_fee > 0);
                        let child_icon = if total_branches >= 4 {
                            (node_px * 0.80).max(32.0)
                        } else {
                            (node_px * 0.88).max(36.0)
                        };
                        let sub_icon = (node_px * 0.62).max(26.0);

                        pyr.spawn(Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::FlexStart,
                            ..default()
                        })
                        .with_children(|branch_row| {
                            let mut used_pool = owned_pool.clone();

                            for (b_idx, comp_id) in item.recipe.iter().enumerate() {
                                let comp_owned = if let Some(pos) = used_pool.iter().position(|x| x == comp_id) {
                                    used_pool.remove(pos);
                                    true
                                } else {
                                    false
                                };

                                branch_row
                                    .spawn(Node {
                                        flex_grow: 1.0,
                                        flex_basis: px(0.0),
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    })
                                    .with_children(|bcol| {
                                        spawn_branch_connector_header(
                                            bcol,
                                            b_idx,
                                            total_branches,
                                            line_px,
                                            line_col,
                                        );
                                        if let Some(comp) = items_cfg.get_item(comp_id) {
                                            let (c_cost, _) =
                                                effective_cost_for_display(&items_cfg, &comp.id, &ctx);
                                            spawn_pyramid_item_tile(
                                                bcol,
                                                &asset_server,
                                                &mut icon_cache,
                                                comp,
                                                child_icon,
                                                false,
                                                comp_owned,
                                                c_cost,
                                                ctx.gold >= c_cost,
                                            );

                                            // سطح دوم هرم (اگر خود کامپوننت هم ترکیبی باشد)
                                            if !comp.recipe.is_empty() && !comp_owned {
                                                let sub_fee = items_cfg.recipe_fee(&comp.id);
                                                let sub_total = comp.recipe.len() + usize::from(sub_fee > 0);
                                                bcol.spawn((
                                                    Pickable::IGNORE,
                                                    Node {
                                                        width: px(line_px),
                                                        height: px(10.0),
                                                        ..default()
                                                    },
                                                    BackgroundColor(line_col),
                                                ));
                                                bcol.spawn(Node {
                                                    width: percent(100),
                                                    flex_direction: FlexDirection::Row,
                                                    justify_content: JustifyContent::Center,
                                                    align_items: AlignItems::FlexStart,
                                                    ..default()
                                                })
                                                .with_children(|srow| {
                                                    for (s_idx, sub_id) in comp.recipe.iter().enumerate() {
                                                        let sub_owned = if let Some(p) = used_pool.iter().position(|x| x == sub_id) {
                                                            used_pool.remove(p);
                                                            true
                                                        } else {
                                                            false
                                                        };
                                                        srow.spawn(Node {
                                                            flex_grow: 1.0,
                                                            flex_basis: px(0.0),
                                                            flex_direction: FlexDirection::Column,
                                                            align_items: AlignItems::Center,
                                                            ..default()
                                                        })
                                                        .with_children(|scol| {
                                                            spawn_branch_connector_header(
                                                                scol,
                                                                s_idx,
                                                                sub_total,
                                                                line_px,
                                                                line_col,
                                                            );
                                                            if let Some(sub_def) = items_cfg.get_item(sub_id) {
                                                                spawn_pyramid_item_tile(
                                                                    scol,
                                                                    &asset_server,
                                                                    &mut icon_cache,
                                                                    sub_def,
                                                                    sub_icon,
                                                                    false,
                                                                    sub_owned,
                                                                    sub_def.cost,
                                                                    ctx.gold >= sub_def.cost,
                                                                );
                                                            }
                                                        });
                                                    }
                                                    if sub_fee > 0 {
                                                        srow.spawn(Node {
                                                            flex_grow: 1.0,
                                                            flex_basis: px(0.0),
                                                            flex_direction: FlexDirection::Column,
                                                            align_items: AlignItems::Center,
                                                            ..default()
                                                        })
                                                        .with_children(|scol| {
                                                            spawn_branch_connector_header(
                                                                scol,
                                                                comp.recipe.len(),
                                                                sub_total,
                                                                line_px,
                                                                line_col,
                                                            );
                                                            spawn_pyramid_recipe_tile(
                                                                scol,
                                                                &comp.id,
                                                                sub_fee,
                                                                sub_icon,
                                                                ctx.gold >= sub_fee,
                                                            );
                                                        });
                                                    }
                                                });
                                            }
                                        }
                                    });
                            }

                            if rec_fee > 0 {
                                branch_row
                                    .spawn(Node {
                                        flex_grow: 1.0,
                                        flex_basis: px(0.0),
                                        flex_direction: FlexDirection::Column,
                                        align_items: AlignItems::Center,
                                        ..default()
                                    })
                                    .with_children(|bcol| {
                                        spawn_branch_connector_header(
                                            bcol,
                                            item.recipe.len(),
                                            total_branches,
                                            line_px,
                                            line_col,
                                        );
                                        spawn_pyramid_recipe_tile(
                                            bcol,
                                            &item.id,
                                            rec_fee,
                                            child_icon,
                                            ctx.gold >= rec_fee,
                                        );
                                    });
                            }
                        });
                    } else {
                        pyr.spawn((
                            Pickable::IGNORE,
                            Node {
                                margin: UiRect::top(px(10.0)),
                                ..default()
                            },
                        ))
                        .with_children(|n| {
                            n.spawn((
                                Pickable::IGNORE,
                                Text::new("BASE COMPONENT  (Hover icon for stats)"),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::srgba(0.62, 0.72, 0.84, 0.80)),
                            ));
                        });
                    }
                });

                // دکمه‌های خرید مستقیم و افزودن به نوار سریع HUD
                col.spawn(Node {
                    width: percent(100),
                    margin: UiRect::top(px(6.0)),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    ..default()
                })
                .with_children(|btns| {
                    btns.spawn((
                        UiButton,
            Hovered::default(),
                        ShopBuyNowBtn(item.id.clone()),
                        Node {
                            flex_grow: 1.0,
                            padding: UiRect::axes(px(10.0), px(9.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1.5)),
                            border_radius: BorderRadius::all(px(6.0)),
                            ..default()
                        },
                        BorderColor::all(if can_buy {
                            Color::srgb(0.24, 0.92, 0.52)
                        } else {
                            Color::srgba(0.45, 0.45, 0.48, 0.6)
                        }),
                        BackgroundColor(if can_buy {
                            Color::srgba(0.08, 0.36, 0.18, 0.95)
                        } else {
                            Color::srgba(0.12, 0.12, 0.14, 0.85)
                        }),
                    ))
                    .with_children(|b| {
                        let lbl = if consumed_cnt > 0 {
                            format!("CRAFT NOW ({}g)", eff_cost)
                        } else {
                            format!("BUY NOW ({}g)", eff_cost)
                        };
                        b.spawn((
                            Pickable::IGNORE,
                            Text::new(lbl),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(if can_buy {
                                Color::WHITE
                            } else {
                                Color::srgba(0.65, 0.65, 0.68, 0.85)
                            }),
                        ));
                    });

                    btns.spawn((
                        UiButton,
            Hovered::default(),
                        ShopAddToCartBtn(item.id.clone()),
                        Node {
                            padding: UiRect::axes(px(12.0), px(9.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1.5)),
                            border_radius: BorderRadius::all(px(6.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.32, 0.72, 0.96)),
                        BackgroundColor(Color::srgba(0.08, 0.20, 0.34, 0.95)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Pickable::IGNORE,
                            Text::new("+ QUICK BUY"),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                });
            });
        }
    }
}

#[derive(Default)]
struct HudBarRebuildCache {
    last_inv: Option<Vec<InventoryItemState>>,
    last_stash: Option<Vec<InventoryItemState>>,
    last_tp_slot: Option<Option<InventoryItemState>>,
    last_cart: Vec<String>,
    last_cart_afford_mask: u64,
    last_in_shop_mask: u8,
    last_bindings: Option<[Option<ShortcutInput>; 6]>,
    last_cour_sel: Option<bool>,
    last_tp_cd: u32,
    last_tp_preview: bool,
    last_is_tp: bool,
    last_sb_cd: u32,
    last_is_invis: bool,
}

#[derive(SystemParam)]
struct HudBarQueries<'w, 's> {
    slots_q: Query<'w, 's, (Entity, Option<&'static Children>), With<HudInventorySlotsRow>>,
    cart_bar_q: Query<'w, 's, &'static mut Node, (With<HudCartBarRoot>, Without<HudStashBarRoot>, Without<HudStashTakeAllBtn>)>,
    cart_slots_q: Query<'w, 's, (Entity, Option<&'static Children>), With<HudCartSlotsRow>>,
    stash_bar_q: Query<'w, 's, &'static mut Node, (With<HudStashBarRoot>, Without<HudCartBarRoot>, Without<HudStashTakeAllBtn>)>,
    stash_slots_q: Query<'w, 's, (Entity, Option<&'static Children>), With<HudStashSlotsRow>>,
    stash_take_btn_q: Query<'w, 's, &'static mut Node, (With<HudStashTakeAllBtn>, Without<HudCartBarRoot>, Without<HudStashBarRoot>)>,
    tp_slot_q: Query<'w, 's, (Entity, Option<&'static Children>, &'static mut BorderColor), (With<HudTpSlotBtn>, Without<HudInventoryRoot>)>,
    toggle_txt_q: Query<'w, 's, &'static mut Text, With<HudShopToggleText>>,
    root_border_q: Query<'w, 's, &'static mut BorderColor, (With<HudInventoryRoot>, Without<HudTpSlotBtn>)>,
}

fn rebuild_hud_inventory_bar_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    ui_state: Res<ShopUiState>,
    shortcuts: Res<InventoryShortcutsState>,
    courier_ctrl: Res<CourierControlState>,
    teleport_preview: Res<TeleportPreviewState>,
    mut hq: HudBarQueries,
    mut cache: Local<HudBarRebuildCache>,
) {
    if !ctx.in_game {
        if let Ok(mut cb_node) = hq.cart_bar_q.single_mut() {
            if cb_node.display != Display::None {
                cb_node.display = Display::None;
            }
        }
        if let Ok(mut sb_node) = hq.stash_bar_q.single_mut() {
            if sb_node.display != Display::None {
                sb_node.display = Display::None;
            }
        }
        return;
    }
    let active_inv = if let Some(ref insp_inv) = ctx.inspected_inventory {
        insp_inv
    } else if courier_ctrl.selected {
        &ctx.courier_inventory
    } else {
        &ctx.inventory
    };
    let in_shop_mask = u8::from(ctx.in_fountain && !ctx.is_dead)
        | (u8::from(ctx.courier_in_fountain && !ctx.courier_is_dead) << 1);

    let mut afford_mask = 0_u64;
    for (ci, cid) in ui_state.cart.iter().take(64).enumerate() {
        let (eff_cost, _) = effective_cost_for_display(&items_cfg, cid, &ctx);
        let can_buy = ctx.gold >= eff_cost;
        if can_buy {
            afford_mask |= 1_u64 << ci;
        }
    }
    let cart_changed = cache.last_cart != ui_state.cart
        || cache.last_cart_afford_mask != afford_mask
        || cache.last_in_shop_mask != in_shop_mask;
    let inv_changed = cache.last_inv.as_ref() != Some(active_inv);
    let stash_changed = cache.last_stash.as_ref() != Some(&ctx.stash);
    let tp_slot_changed = cache.last_tp_slot.as_ref() != Some(&ctx.tp_slot);
    let bind_changed = cache.last_bindings.as_ref() != Some(&shortcuts.bindings);
    let sel_changed = cache.last_cour_sel != Some(courier_ctrl.selected);
    let tp_cd_ceil = ctx.teleport_cooldown_remaining.ceil().max(0.0) as u32;
    let sb_cd_ceil = ctx.shadow_blade_cooldown_remaining.ceil().max(0.0) as u32;
    let tp_changed = cache.last_tp_cd != tp_cd_ceil
        || cache.last_tp_preview != teleport_preview.active
        || cache.last_is_tp != ctx.is_teleporting
        || cache.last_sb_cd != sb_cd_ceil
        || cache.last_is_invis != ctx.is_invisible;
    if !cart_changed && !inv_changed && !stash_changed && !tp_slot_changed && !bind_changed && !sel_changed && !tp_changed {
        return;
    }
    cache.last_cart = ui_state.cart.clone();
    cache.last_cart_afford_mask = afford_mask;
    cache.last_in_shop_mask = in_shop_mask;
    cache.last_inv = Some(active_inv.clone());
    cache.last_stash = Some(ctx.stash.clone());
    cache.last_tp_slot = Some(ctx.tp_slot.clone());
    cache.last_bindings = Some(shortcuts.bindings);
    cache.last_cour_sel = Some(courier_ctrl.selected);
    cache.last_tp_cd = tp_cd_ceil;
    cache.last_tp_preview = teleport_preview.active;
    cache.last_is_tp = ctx.is_teleporting;
    cache.last_sb_cd = sb_cd_ceil;
    cache.last_is_invis = ctx.is_invisible;

    if let Ok(mut bc) = hq.root_border_q.single_mut() {
        *bc = if courier_ctrl.selected {
            BorderColor::all(Color::srgb(0.28, 0.88, 1.0))
        } else {
            BorderColor::all(Color::srgba(0.42, 0.58, 0.72, 0.78))
        };
    }

    if let Ok(mut t) = hq.toggle_txt_q.single_mut() {
        if ui_state.cart.is_empty() {
            **t = "[B] SHOP".to_string();
        } else {
            **t = format!("[B] SHOP ({})", ui_state.cart.len());
        }
    }

    if let Ok(mut cb_node) = hq.cart_bar_q.single_mut() {
        let desired_cb = if ui_state.cart.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
        if cb_node.display != desired_cb {
            cb_node.display = desired_cb;
        }
    }
    if let Ok((cart_row_ent, cart_ch)) = hq.cart_slots_q.single() {
        clear_children(&mut commands, cart_ch);
        if !ui_state.cart.is_empty() {
            commands.entity(cart_row_ent).with_children(|crow| {
                for (c_idx, cid) in ui_state.cart.iter().enumerate() {
                    let is_recipe_scroll = parse_recipe_target(cid).is_some();
                    let (eff_cost, _) = effective_cost_for_display(&items_cfg, cid, &ctx);
                    let valid_entry = if let Some(tid) = parse_recipe_target(cid) {
                        items_cfg.recipe_fee(tid) > 0
                    } else {
                        items_cfg.get_item(cid).is_some()
                    };
                    if !valid_entry {
                        continue;
                    }
                    let can_buy = ctx.gold >= eff_cost;
                    let border_col = if can_buy {
                        Color::srgb(0.18, 0.92, 0.42)
                    } else if is_recipe_scroll {
                        Color::srgba(0.92, 0.72, 0.25, 0.80)
                    } else {
                        Color::srgba(0.35, 0.42, 0.55, 0.65)
                    };
                    let bg_col = if can_buy {
                        Color::srgba(0.06, 0.22, 0.12, 0.92)
                    } else if is_recipe_scroll {
                        Color::srgba(0.18, 0.13, 0.04, 0.92)
                    } else {
                        Color::srgba(0.04, 0.06, 0.10, 0.88)
                    };
                    crow.spawn((
                        UiButton,
                        Hovered::default(),
                        RelativeCursorPosition::default(),
                        HudCartSlotBtn {
                            cart_index: c_idx,
                            item_id: cid.clone(),
                        },
                        Node {
                            width: px(46.0),
                            height: px(46.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(2.0)),
                            border_radius: BorderRadius::all(px(6.0)),
                            ..default()
                        },
                        BorderColor::all(border_col),
                        BackgroundColor(bg_col),
                    ))
                    .with_children(|slot_node| {
                        if is_recipe_scroll {
                            spawn_recipe_scroll_badge(slot_node, 30.0, 9.5);
                        } else if let Some(item) = items_cfg.get_item(cid) {
                            spawn_item_icon_badge(
                                slot_node,
                                &asset_server,
                                &mut icon_cache,
                                item,
                                32.0,
                                10.0,
                            );
                        }
                        slot_node.spawn((
                            Pickable::IGNORE,
                            Text::new(format!("{}g", eff_cost)),
                            TextFont {
                                font_size: FontSize::Px(8.5),
                                ..default()
                            },
                            TextColor(if can_buy {
                                Color::srgb(1.0, 0.88, 0.35)
                            } else {
                                Color::srgb(0.75, 0.56, 0.52)
                            }),
                        ));
                        slot_node
                            .spawn((
                                UiButton,
                                Hovered::default(),
                                HudCartRemoveBtn(c_idx),
                                Node {
                                    position_type: PositionType::Absolute,
                                    top: px(-4.0),
                                    right: px(-4.0),
                                    width: px(14.0),
                                    height: px(14.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.65, 0.14, 0.16, 0.96)),
                            ))
                            .with_children(|xb| {
                                xb.spawn((
                                    Pickable::IGNORE,
                                    Text::new("x"),
                                    TextFont {
                                        font_size: FontSize::Px(9.5),
                                        ..default()
                                    },
                                    TextColor(Color::WHITE),
                                ));
                            });
                    });
                }
                crow.spawn((
                    UiButton,
            Hovered::default(),
                    HudCartClearBtn,
                    Node {
                        padding: UiRect::axes(px(7.0), px(5.0)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.75, 0.25, 0.25, 0.85)),
                    BackgroundColor(Color::srgba(0.28, 0.08, 0.09, 0.92)),
                ))
                .with_children(|cb| {
                    cb.spawn((
                        Pickable::IGNORE,
                        Text::new("CLEAR"),
                        TextFont {
                            font_size: FontSize::Px(9.5),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.72, 0.72)),
                    ));
                });
            });
        }
    }

    // نوار Stash (همیشه در طول بازی نمایش داده می‌شود)
    let show_stash = ctx.in_game;
    if let Ok(mut sb_node) = hq.stash_bar_q.single_mut() {
        let desired = if show_stash { Display::Flex } else { Display::None };
        if sb_node.display != desired {
            sb_node.display = desired;
        }
    }
    if let Ok(mut take_node) = hq.stash_take_btn_q.single_mut() {
        let desired = if !ctx.stash.is_empty() && ctx.in_fountain && !ctx.is_dead {
            Display::Flex
        } else {
            Display::None
        };
        if take_node.display != desired {
            take_node.display = desired;
        }
    }
    if let Ok((stash_row_ent, stash_ch)) = hq.stash_slots_q.single() {
        clear_children(&mut commands, stash_ch);
        if show_stash {
            commands.entity(stash_row_ent).with_children(|srow| {
                for s_idx in 0..items_cfg.max_stash_slots {
                    if let Some(st_item) = ctx.stash.iter().find(|it| it.slot == s_idx) {
                        srow.spawn((
                            UiButton,
                            Hovered::default(),
                            RelativeCursorPosition::default(),
                            HudStashSlotBtn { stash_index: s_idx },
                            Node {
                                width: px(40.0),
                                height: px(40.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(px(1.5)),
                                border_radius: BorderRadius::all(px(5.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.95, 0.78, 0.28, 0.85)),
                            BackgroundColor(Color::srgba(0.10, 0.11, 0.16, 0.92)),
                        ))
                        .with_children(|slot| {
                            if parse_recipe_target(&st_item.item_id).is_some() {
                                spawn_recipe_scroll_badge(slot, 34.0, 10.0);
                            } else if let Some(item) = items_cfg.get_item(&st_item.item_id) {
                                spawn_item_icon_badge(slot, &asset_server, &mut icon_cache, item, 36.0, 10.5);
                            }
                            if st_item.count > 1 {
                                slot.spawn((
                                    Pickable::IGNORE,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        bottom: px(1.0),
                                        right: px(2.0),
                                        padding: UiRect::axes(px(3.0), px(0.5)),
                                        border_radius: BorderRadius::all(px(3.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.90)),
                                ))
                                .with_children(|cnt| {
                                    cnt.spawn((
                                        Text::new(format!("x{}", st_item.count)),
                                        TextFont { font_size: FontSize::Px(8.5), ..default() },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                            }
                        });
                    } else {
                        srow.spawn((
                            UiButton,
                            Hovered::default(),
                            RelativeCursorPosition::default(),
                            HudStashSlotBtn { stash_index: s_idx },
                            Node {
                                width: px(40.0),
                                height: px(40.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(px(1.0)),
                                border_radius: BorderRadius::all(px(5.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.30, 0.34, 0.42, 0.50)),
                            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.72)),
                        ));
                    }
                }
            });
        }
    }

    if let Ok((row_ent, ch)) = hq.slots_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(row_ent).with_children(|row| {
            for idx in 0..items_cfg.max_inventory_slots {
                let key_lbl = if courier_ctrl.selected {
                    None
                } else {
                    shortcuts.bindings.get(idx).and_then(|b| *b).map(|b| b.label())
                };
                if let Some(inv_item) = active_inv.iter().find(|it| it.slot == idx) {
                    if parse_recipe_target(&inv_item.item_id).is_some() {
                        row.spawn((
                            UiButton,
                            Hovered::default(),
                            RelativeCursorPosition::default(),
                            HudInventorySlotBtn { inv_index: idx },
                            Node {
                                width: px(46.0),
                                height: px(46.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|slot| {
                            spawn_recipe_scroll_badge(slot, 42.0, 11.5);
                        });
                        continue;
                    }
                    if let Some(item) = items_cfg.get_item(&inv_item.item_id) {
                        row.spawn((
                            UiButton,
            Hovered::default(),
                            RelativeCursorPosition::default(),
                            HudInventorySlotBtn { inv_index: idx },
                            Node {
                                width: px(46.0),
                                height: px(46.0),
                                flex_direction: FlexDirection::Column,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..default()
                            },
                        ))
                        .with_children(|slot| {
                            spawn_item_icon_badge(slot, &asset_server, &mut icon_cache, item, 44.0, 11.5);
                            if let Some(kl) = &key_lbl {
                                slot.spawn((
                                    Pickable::IGNORE,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: px(1.0),
                                        left: px(2.0),
                                        padding: UiRect::axes(px(3.0), px(0.5)),
                                        border_radius: BorderRadius::all(px(3.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.85)),
                                ))
                                .with_children(|kb| {
                                    kb.spawn((
                                        Text::new(kl),
                                        TextFont {
                                            font_size: FontSize::Px(8.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.95, 0.85, 0.40)),
                                    ));
                                });
                            }
                            if item.active_ability == ItemActiveAbility::ShadowBlade
                                && !ctx.inspected_is_read_only
                                && !courier_ctrl.selected
                            {
                                if ctx.is_invisible {
                                    slot.spawn((
                                        Pickable::IGNORE,
                                        Node {
                                            position_type: PositionType::Absolute,
                                            width: px(46.0),
                                            height: px(46.0),
                                            border: UiRect::all(px(2.0)),
                                            border_radius: BorderRadius::all(px(6.0)),
                                            ..default()
                                        },
                                        BorderColor::all(Color::srgb(0.45, 0.78, 1.0)),
                                        BackgroundColor(Color::srgba(0.25, 0.55, 0.95, 0.18)),
                                    ));
                                }
                                if sb_cd_ceil > 0 {
                                    slot.spawn((
                                        Pickable::IGNORE,
                                        Node {
                                            position_type: PositionType::Absolute,
                                            width: px(44.0),
                                            height: px(44.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border_radius: BorderRadius::all(px(5.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.78)),
                                    ))
                                    .with_children(|cd| {
                                        cd.spawn((
                                            Text::new(format!("{}s", sb_cd_ceil)),
                                            TextFont {
                                                font_size: FontSize::Px(13.0),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.95, 0.85, 0.35)),
                                        ));
                                    });
                                }
                            }
                            if inv_item.count > 1 {
                                slot.spawn((
                                    Pickable::IGNORE,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        bottom: px(1.0),
                                        right: px(2.0),
                                        padding: UiRect::axes(px(3.0), px(0.5)),
                                        border_radius: BorderRadius::all(px(3.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.88)),
                                ))
                                .with_children(|cnt| {
                                    cnt.spawn((
                                        Text::new(format!("x{}", inv_item.count)),
                                        TextFont {
                                            font_size: FontSize::Px(9.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.95, 0.95, 0.98)),
                                    ));
                                });
                            }
                            if !item.modes.is_empty() {
                                let m_idx = inv_item.active_mode.min(item.modes.len() - 1);
                                let mc = item.modes[m_idx].color_rgb;
                                slot.spawn((
                                    Pickable::IGNORE,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        bottom: px(1.0),
                                        padding: UiRect::axes(px(3.0), px(0.5)),
                                        border_radius: BorderRadius::all(px(3.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.88)),
                                ))
                                .with_children(|tag| {
                                    tag.spawn((
                                        Text::new(&item.modes[m_idx].name),
                                        TextFont {
                                            font_size: FontSize::Px(8.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(mc.0, mc.1, mc.2)),
                                    ));
                                });
                            }
                        });
                        continue;
                    }
                }
                row.spawn((
                    UiButton,
            Hovered::default(),
                    RelativeCursorPosition::default(),
                    HudInventorySlotBtn { inv_index: idx },
                    Node {
                        width: px(46.0),
                        height: px(46.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.24, 0.30, 0.38, 0.55)),
                    BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.75)),
                ))
                .with_children(|empty| {
                    let lbl = key_lbl.unwrap_or_else(|| format!("{}", idx + 1));
                    empty.spawn((
                        Pickable::IGNORE,
                        Text::new(lbl),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(Color::srgba(0.60, 0.70, 0.80, 0.70)),
                    ));
                });
            }
        });
    }

    if let Ok((tp_ent, tp_ch, mut tp_border)) = hq.tp_slot_q.single_mut() {
        clear_children(&mut commands, tp_ch);
        let is_active = teleport_preview.active || ctx.is_teleporting;
        *tp_border = if is_active {
            BorderColor::all(Color::srgb(0.20, 0.95, 1.0))
        } else if ctx.tp_slot.is_some() {
            BorderColor::all(Color::srgb(0.32, 0.72, 0.95))
        } else {
            BorderColor::all(Color::srgba(0.24, 0.32, 0.42, 0.55))
        };
        let key_lbl = items_cfg.teleport.shortcut_key.to_ascii_uppercase();
        commands.entity(tp_ent).with_children(|tp_btn| {
            if let Some(ref tp_item) = ctx.tp_slot {
                if let Some(def) = items_cfg.get_item(&tp_item.item_id) {
                    spawn_item_icon_badge(tp_btn, &asset_server, &mut icon_cache, def, 34.0, 10.5);
                }
                tp_btn
                    .spawn((
                        Pickable::IGNORE,
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: px(1.0),
                            right: px(2.0),
                            padding: UiRect::axes(px(4.0), px(0.5)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.90)),
                    ))
                    .with_children(|cnt| {
                        cnt.spawn((
                            Text::new(format!("{}", tp_item.count)),
                            TextFont {
                                font_size: FontSize::Px(9.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.95, 0.98, 1.0)),
                        ));
                    });
            } else {
                tp_btn.spawn((
                    Pickable::IGNORE,
                    Text::new("TP"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgba(0.55, 0.70, 0.85, 0.65)),
                ));
            }
            tp_btn
                .spawn((
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        top: px(-2.0),
                        left: px(1.0),
                        padding: UiRect::axes(px(3.0), px(0.5)),
                        border_radius: BorderRadius::all(px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.88)),
                ))
                .with_children(|kb| {
                    kb.spawn((
                        Text::new(key_lbl),
                        TextFont {
                            font_size: FontSize::Px(8.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.85, 0.40)),
                    ));
                });
            if tp_cd_ceil > 0 {
                tp_btn
                    .spawn((
                        Pickable::IGNORE,
                        Node {
                            position_type: PositionType::Absolute,
                            width: px(42.0),
                            height: px(42.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.80)),
                    ))
                    .with_children(|cd| {
                        cd.spawn((
                            Text::new(format!("{}s", tp_cd_ceil)),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.95, 0.85, 0.35)),
                        ));
                    });
            }
        });
    }
}

#[derive(SystemParam)]
struct SellMenuQueries<'w, 's> {
    accept_q: Query<'w, 's, &'static SellConfirmAcceptBtn, Added<Pressed>>,
    move_stash_q: Query<'w, 's, &'static ContextMoveToStashBtn, Added<Pressed>>,
    take_stash_q: Query<'w, 's, &'static ContextTakeFromStashBtn, Added<Pressed>>,
    modal_q: Query<'w, 's, (&'static mut Node, &'static RelativeCursorPosition), With<SellConfirmModalRoot>>,
    card_q: Query<'w, 's, (Entity, Option<&'static Children>), With<SellConfirmCardContent>>,
}

fn sell_confirm_modal_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<ShopUiState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut audio_q: ResMut<AudioEventQueue>,
    mut sq: SellMenuQueries,
    mut last_state: Local<(Option<usize>, bool, bool, bool, bool, bool, bool, usize)>,
) {
    for btn in &sq.accept_q {
        if btn.is_stash {
            audio_q.0.push(ClientAudioEvent::ShopSell);
            outgoing
                .0
                .push(ClientLobbyMessage::SellStashItem { stash_slot: btn.slot });
        } else if btn.is_courier {
            if ctx.courier_in_fountain && !ctx.courier_is_dead {
                audio_q.0.push(ClientAudioEvent::ShopSell);
                outgoing
                    .0
                    .push(ClientLobbyMessage::SellCourierItem { inv_index: btn.slot });
            }
        } else if ctx.in_fountain && !ctx.is_dead {
            audio_q.0.push(ClientAudioEvent::ShopSell);
            outgoing
                .0
                .push(ClientLobbyMessage::SellItem { inv_index: btn.slot });
        }
        ui_state.pending_sell_slot = None;
        ui_state.suppress_world_click_this_frame = true;
        ui_state.dirty = true;
    }

    for btn in &sq.move_stash_q {
        if btn.is_courier {
            outgoing.0.push(ClientLobbyMessage::MoveCourierItemToStash {
                courier_slot: btn.slot,
            });
        } else {
            outgoing.0.push(ClientLobbyMessage::MoveHeroItemToStash {
                hero_slot: btn.slot,
            });
        }
        ui_state.pending_sell_slot = None;
        ui_state.suppress_world_click_this_frame = true;
        ui_state.dirty = true;
    }

    for btn in &sq.take_stash_q {
        if btn.to_courier {
            outgoing.0.push(ClientLobbyMessage::MoveStashItemToCourier {
                stash_slot: btn.stash_slot,
            });
        } else {
            outgoing.0.push(ClientLobbyMessage::MoveStashItemToHero {
                stash_slot: btn.stash_slot,
            });
        }
        ui_state.pending_sell_slot = None;
        ui_state.suppress_world_click_this_frame = true;
        ui_state.dirty = true;
    }

    if !ctx.in_game {
        ui_state.pending_sell_slot = None;
    } else if let Some(slot) = ui_state.pending_sell_slot {
        let still_exists = if ui_state.pending_sell_is_stash {
            ctx.stash.iter().any(|it| it.slot == slot)
        } else if ui_state.pending_sell_is_courier {
            ctx.courier_inventory.iter().any(|it| it.slot == slot)
        } else {
            ctx.inventory.iter().any(|it| it.slot == slot)
        };
        if !still_exists {
            ui_state.pending_sell_slot = None;
        } else if keys.just_pressed(KeyCode::Escape) {
            ui_state.pending_sell_slot = None;
        } else if mouse.just_pressed(MouseButton::Left) {
            let over_menu = sq.modal_q.single().is_ok_and(|(_, rel)| is_cursor_inside(rel));
            if !over_menu {
                ui_state.pending_sell_slot = None;
                ui_state.suppress_world_click_this_frame = true;
            }
        }
    }

    let desired_display = if ui_state.pending_sell_slot.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok((mut m_node, _)) = sq.modal_q.single_mut() {
        if m_node.display != desired_display {
            m_node.display = desired_display;
        }
        if let Some(slot) = ui_state.pending_sell_slot {
            m_node.left = px(items_cfg.shop_ui.hud_inventory_left_px + (slot as f32) * 52.0);
            m_node.bottom = px(if ui_state.pending_sell_is_stash { 196.0 } else { 76.0 });
        }
    }

    let cur_key = (
        ui_state.pending_sell_slot,
        ui_state.pending_sell_is_courier,
        ui_state.pending_sell_is_stash,
        ctx.in_fountain,
        ctx.is_dead,
        ctx.courier_in_fountain,
        ctx.courier_is_dead,
        ctx.stash.len(),
    );
    if *last_state == cur_key {
        return;
    }
    *last_state = cur_key;

    let Some(slot) = ui_state.pending_sell_slot else {
        return;
    };
    let is_courier = ui_state.pending_sell_is_courier;
    let is_stash = ui_state.pending_sell_is_stash;
    let inv_item = if is_stash {
        ctx.stash.iter().find(|it| it.slot == slot)
    } else if is_courier {
        ctx.courier_inventory.iter().find(|it| it.slot == slot)
    } else {
        ctx.inventory.iter().find(|it| it.slot == slot)
    };
    let Some(inv_item) = inv_item else {
        return;
    };

    let display_name = if let Some(tid) = parse_recipe_target(&inv_item.item_id) {
        let base_name = items_cfg.get_item(tid).map_or(tid, |d| d.name.as_str());
        format!("{} Recipe", base_name)
    } else if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
        def.name.clone()
    } else {
        return;
    };
    let refund = items_cfg.sell_price(&inv_item.item_id);
    let hero_in_shop = ctx.in_fountain && !ctx.is_dead;
    let courier_in_shop = ctx.courier_in_fountain && !ctx.courier_is_dead;
    let can_sell = if is_stash {
        true
    } else if is_courier {
        courier_in_shop
    } else {
        hero_in_shop
    };
    let stash_has_room = ctx.stash.len() < items_cfg.max_stash_slots;

    if let Ok((card_ent, ch)) = sq.card_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(card_ent).with_children(|card| {
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                ..default()
            })
            .with_children(|row| {
                if parse_recipe_target(&inv_item.item_id).is_some() {
                    spawn_recipe_scroll_badge(row, 22.0, 8.5);
                } else if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                    spawn_item_icon_badge(row, &asset_server, &mut icon_cache, def, 22.0, 9.0);
                }
                row.spawn((
                    Text::new(display_name),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });

            // دکمه انتقال بین اینونتوری و Stash
            if is_stash {
                if hero_in_shop {
                    card.spawn((
                        UiButton,
                        Hovered::default(),
                        ContextTakeFromStashBtn {
                            stash_slot: slot,
                            to_courier: false,
                        },
                        Node {
                            width: percent(100),
                            padding: UiRect::axes(px(10.0), px(5.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(5.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.24, 0.88, 0.52)),
                        BackgroundColor(Color::srgba(0.06, 0.24, 0.14, 0.95)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Pickable::IGNORE,
                            Text::new("MOVE TO HERO"),
                            TextFont { font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgb(0.45, 0.98, 0.65)),
                        ));
                    });
                }
                if courier_in_shop {
                    card.spawn((
                        UiButton,
                        Hovered::default(),
                        ContextTakeFromStashBtn {
                            stash_slot: slot,
                            to_courier: true,
                        },
                        Node {
                            width: percent(100),
                            padding: UiRect::axes(px(10.0), px(5.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(5.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.32, 0.82, 0.98)),
                        BackgroundColor(Color::srgba(0.06, 0.18, 0.28, 0.95)),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Pickable::IGNORE,
                            Text::new("MOVE TO COURIER"),
                            TextFont { font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgb(0.55, 0.92, 1.0)),
                        ));
                    });
                }
            } else if can_sell && stash_has_room {
                card.spawn((
                    UiButton,
                    Hovered::default(),
                    ContextMoveToStashBtn { slot, is_courier },
                    Node {
                        width: percent(100),
                        padding: UiRect::axes(px(10.0), px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.35, 0.78, 0.96)),
                    BackgroundColor(Color::srgba(0.06, 0.18, 0.28, 0.95)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Pickable::IGNORE,
                        Text::new("MOVE TO STASH"),
                        TextFont { font_size: FontSize::Px(11.0), ..default() },
                        TextColor(Color::srgb(0.60, 0.92, 1.0)),
                    ));
                });
            }

            if can_sell {
                card.spawn((
                    UiButton,
                    Hovered::default(),
                    SellConfirmAcceptBtn {
                        slot,
                        is_courier,
                        is_stash,
                    },
                    Node {
                        width: percent(100),
                        padding: UiRect::axes(px(10.0), px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.95, 0.72, 0.25)),
                    BackgroundColor(Color::srgba(0.32, 0.22, 0.05, 0.95)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Pickable::IGNORE,
                        Text::new(format!("SELL (+{}g)", refund)),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.88, 0.36)),
                    ));
                });
            } else {
                card.spawn((
                    Node {
                        width: percent(100),
                        padding: UiRect::axes(px(8.0), px(6.0)),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(6.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.45, 0.48, 0.54, 0.65)),
                    BackgroundColor(Color::srgba(0.12, 0.14, 0.18, 0.90)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Node {
                            padding: UiRect::axes(px(4.0), px(1.0)),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(3.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.85, 0.45, 0.35)),
                        BackgroundColor(Color::srgba(0.32, 0.10, 0.10, 0.90)),
                    ))
                    .with_children(|lk| {
                        lk.spawn((
                            Text::new("LOCK"),
                            TextFont {
                                font_size: FontSize::Px(8.5),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.62, 0.52)),
                        ));
                    });
                    b.spawn((
                        Text::new(format!("SELL +{}g (SHOP)", refund)),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::srgba(0.62, 0.66, 0.72, 0.85)),
                    ));
                });
            }
        });
    }
}

#[derive(SystemParam)]
struct SettingsInteractionParams<'w, 's> {
    toggle_btn_q: Query<'w, 's, Entity, (Added<Pressed>, With<SettingsToggleBtn>)>,
    close_btn_q: Query<'w, 's, Entity, (Added<Pressed>, With<SettingsCloseBtn>)>,
    bind_btn_q: Query<'w, 's, &'static SettingsSlotBindBtn, Added<Pressed>>,
    clear_btn_q: Query<'w, 's, &'static SettingsSlotClearBtn, Added<Pressed>>,
    vaim_bind_q: Query<'w, 's, Entity, (Added<Pressed>, With<SettingsVerticalAimBindBtn>)>,
    vaim_reset_q: Query<'w, 's, Entity, (Added<Pressed>, With<SettingsVerticalAimResetBtn>)>,
    toggle_node_q: Query<'w, 's, &'static mut Node, With<SettingsToggleBtn>>,
    modal_node_q: Query<'w, 's, &'static mut Node, (With<SettingsModalRoot>, Without<SettingsToggleBtn>)>,
}

fn settings_modal_interaction_system(
    config: Res<GameConfig>,
    ctx: Res<ShopContextSnapshot>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut settings_ui: ResMut<SettingsUiState>,
    mut shortcuts: ResMut<InventoryShortcutsState>,
    mut vertical_aim_key: ResMut<VerticalAimKeyState>,
    mut p: SettingsInteractionParams,
) {
    if let Ok(mut tn) = p.toggle_node_q.single_mut() {
        let desired = if ctx.in_game { Display::Flex } else { Display::None };
        if tn.display != desired {
            tn.display = desired;
        }
    }

    if !ctx.in_game {
        settings_ui.is_open = false;
        settings_ui.listening_slot = None;
        settings_ui.listening_vertical_aim = false;
        if let Ok(mut mn) = p.modal_node_q.single_mut() {
            if mn.display != Display::None {
                mn.display = Display::None;
            }
        }
        return;
    }

    for _ in &p.toggle_btn_q {
        settings_ui.is_open = !settings_ui.is_open;
        settings_ui.listening_slot = None;
        settings_ui.listening_vertical_aim = false;
        settings_ui.status_message = None;
        settings_ui.dirty = true;
    }
    for _ in &p.close_btn_q {
        settings_ui.is_open = false;
        settings_ui.listening_slot = None;
        settings_ui.listening_vertical_aim = false;
        settings_ui.dirty = true;
    }

    if settings_ui.is_open {
        for clr in &p.clear_btn_q {
            shortcuts.bindings[clr.0] = None;
            if settings_ui.listening_slot == Some(clr.0) {
                settings_ui.listening_slot = None;
            }
            settings_ui.status_message = Some((format!("Slot {} shortcut cleared.", clr.0 + 1), false));
            settings_ui.dirty = true;
        }

        for bbtn in &p.bind_btn_q {
            settings_ui.listening_slot = Some(bbtn.0);
            settings_ui.listening_vertical_aim = false;
            settings_ui.just_started_listening = true;
            settings_ui.status_message = Some((
                format!("Listening for Slot {} - press ANY key or mouse button (ESC to cancel)...", bbtn.0 + 1),
                false,
            ));
            settings_ui.dirty = true;
        }

        for _ in &p.vaim_bind_q {
            settings_ui.listening_vertical_aim = true;
            settings_ui.listening_slot = None;
            settings_ui.just_started_listening = true;
            settings_ui.status_message = Some((
                "Listening for 3D Elevation Aim - press ANY key or mouse button (ESC to cancel)...".to_string(),
                false,
            ));
            settings_ui.dirty = true;
        }

        for _ in &p.vaim_reset_q {
            let def_key = ShortcutInput::from_config_str(&config.soldier.vertical_aim_key);
            if let Some(dup_idx) = shortcuts.bindings.iter().position(|b| *b == Some(def_key)) {
                settings_ui.status_message = Some((
                    format!("Cannot reset: [{}] is bound to Slot {}!", def_key.label(), dup_idx + 1),
                    true,
                ));
            } else {
                vertical_aim_key.binding = def_key;
                settings_ui.listening_vertical_aim = false;
                settings_ui.status_message = Some((
                    format!("3D Elevation Aim reset to [{}].", def_key.label()),
                    false,
                ));
            }
            settings_ui.dirty = true;
        }

        if settings_ui.listening_vertical_aim {
            if settings_ui.just_started_listening {
                settings_ui.just_started_listening = false;
            } else if keys.just_pressed(KeyCode::Escape) {
                settings_ui.listening_vertical_aim = false;
                settings_ui.status_message = Some(("Binding cancelled.".to_string(), false));
                settings_ui.dirty = true;
            } else {
                let captured = keys
                    .get_just_pressed()
                    .next()
                    .copied()
                    .map(ShortcutInput::Key)
                    .or_else(|| {
                        mouse
                            .get_just_pressed()
                            .next()
                            .copied()
                            .map(ShortcutInput::Mouse)
                    });

                if let Some(input) = captured {
                    if let Some(dup_idx) = shortcuts
                        .bindings
                        .iter()
                        .position(|b| *b == Some(input))
                    {
                        settings_ui.status_message = Some((
                            format!("Duplicate blocked: [{}] is already bound to Slot {}!", input.label(), dup_idx + 1),
                            true,
                        ));
                    } else {
                        vertical_aim_key.binding = input;
                        settings_ui.listening_vertical_aim = false;
                        settings_ui.status_message = Some((
                            format!("3D Elevation Aim bound to [{}].", input.label()),
                            false,
                        ));
                    }
                    settings_ui.dirty = true;
                }
            }
        } else if let Some(target_slot) = settings_ui.listening_slot {
            if settings_ui.just_started_listening {
                settings_ui.just_started_listening = false;
            } else if keys.just_pressed(KeyCode::Escape) {
                settings_ui.listening_slot = None;
                settings_ui.status_message = Some(("Binding cancelled.".to_string(), false));
                settings_ui.dirty = true;
            } else {
                let captured = keys
                    .get_just_pressed()
                    .next()
                    .copied()
                    .map(ShortcutInput::Key)
                    .or_else(|| {
                        mouse
                            .get_just_pressed()
                            .next()
                            .copied()
                            .map(ShortcutInput::Mouse)
                    });

                if let Some(input) = captured {
                    if vertical_aim_key.binding == input {
                        settings_ui.status_message = Some((
                            format!("Duplicate blocked: [{}] is already bound to 3D Elevation Aim!", input.label()),
                            true,
                        ));
                    } else if let Some(dup_idx) = shortcuts
                        .bindings
                        .iter()
                        .enumerate()
                        .find_map(|(i, b)| (i != target_slot && *b == Some(input)).then_some(i))
                    {
                        settings_ui.status_message = Some((
                            format!("Duplicate blocked: [{}] is already bound to Slot {}!", input.label(), dup_idx + 1),
                            true,
                        ));
                    } else {
                        shortcuts.bindings[target_slot] = Some(input);
                        settings_ui.listening_slot = None;
                        settings_ui.status_message = Some((
                            format!("Slot {} bound to [{}].", target_slot + 1, input.label()),
                            false,
                        ));
                    }
                    settings_ui.dirty = true;
                }
            }
        } else if keys.just_pressed(KeyCode::Escape) {
            settings_ui.is_open = false;
            settings_ui.dirty = true;
        }
    }

    if let Ok(mut mn) = p.modal_node_q.single_mut() {
        let desired = if settings_ui.is_open { Display::Flex } else { Display::None };
        if mn.display != desired {
            mn.display = desired;
            if settings_ui.is_open {
                settings_ui.dirty = true;
            }
        }
    }
}


fn audio_mixer_slider_interaction_system(
    config: Res<GameConfig>,
    settings_ui: Res<SettingsUiState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut audio_settings: ResMut<AudioSettingsState>,
    slider_q: Query<(&AudioSliderTrack, &RelativeCursorPosition)>,
    mute_q: Query<&AudioChannelMuteBtn, Added<Pressed>>,
    reset_q: Query<Entity, (Added<Pressed>, With<AudioResetBtn>)>,
) {
    if !settings_ui.is_open {
        return;
    }

    for _ in &reset_q {
        audio_settings.reset_from_config(&config.audio);
    }

    for mbtn in &mute_q {
        match mbtn.channel {
            None => audio_settings.master_muted = !audio_settings.master_muted,
            Some(idx) if idx < 7 => audio_settings.channel_muted[idx] = !audio_settings.channel_muted[idx],
            _ => {}
        }
        audio_settings.dirty = true;
    }

    if mouse.pressed(MouseButton::Left) {
        for (track, rel) in &slider_q {
            if let Some(pos) = rel.normalized {
                if (-0.52..=0.52).contains(&pos.x) && (-0.55..=0.55).contains(&pos.y) {
                    let val = (pos.x + 0.5).clamp(0.0, 1.0);
                    match track.channel {
                        None => {
                            if (audio_settings.master_volume - val).abs() > 0.005 {
                                audio_settings.master_volume = val;
                                audio_settings.dirty = true;
                            }
                        }
                        Some(idx) if idx < 7 => {
                            if (audio_settings.channel_volumes[idx] - val).abs() > 0.005 {
                                audio_settings.channel_volumes[idx] = val;
                                audio_settings.dirty = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn spawn_audio_slider_row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    channel: Option<usize>,
    value: f32,
    muted: bool,
    accent_col: Color,
) {
    let pct = (value * 100.0).round() as u32;
    parent
        .spawn(Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: px(155.0),
                ..default()
            })
            .with_children(|lbl| {
                lbl.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(if muted {
                        Color::srgba(0.55, 0.58, 0.65, 0.7)
                    } else {
                        Color::srgb(0.88, 0.94, 1.0)
                    }),
                ));
            });

            row.spawn((
                UiButton,
            Hovered::default(),
                AudioChannelMuteBtn { channel },
                Node {
                    width: px(44.0),
                    padding: UiRect::axes(px(4.0), px(3.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(4.0)),
                    ..default()
                },
                BorderColor::all(if muted {
                    Color::srgb(0.95, 0.30, 0.30)
                } else {
                    Color::srgba(0.35, 0.48, 0.62, 0.75)
                }),
                BackgroundColor(if muted {
                    Color::srgba(0.34, 0.08, 0.08, 0.92)
                } else {
                    Color::srgba(0.10, 0.14, 0.20, 0.92)
                }),
            ))
            .with_children(|mb| {
                mb.spawn((
                    Pickable::IGNORE,
                    Text::new(if muted { "MUTED" } else { "ON" }),
                    TextFont {
                        font_size: FontSize::Px(9.5),
                        ..default()
                    },
                    TextColor(if muted {
                        Color::srgb(1.0, 0.45, 0.45)
                    } else {
                        Color::srgb(0.45, 0.95, 0.65)
                    }),
                ));
            });

            // نوار اسلایدر قابل کلیک و درگ
            row.spawn((
                AudioSliderTrack { channel },
                RelativeCursorPosition::default(),
                Node {
                    flex_grow: 1.0,
                    height: px(16.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::FlexStart,
                    border: UiRect::all(px(1.0)),
                    border_radius: BorderRadius::all(px(5.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.28, 0.38, 0.52, 0.85)),
                BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.95)),
            ))
            .with_children(|track| {
                track.spawn((
                    Pickable::IGNORE,
                    Node {
                        width: percent((value * 100.0).clamp(0.0, 100.0)),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(if muted {
                        Color::srgba(0.45, 0.45, 0.48, 0.55)
                    } else {
                        accent_col
                    }),
                ));
            });

            row.spawn(Node {
                width: px(42.0),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            })
            .with_children(|pct_box| {
                pct_box.spawn((
                    Text::new(format!("{}%", pct)),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.92, 1.0)),
                ));
            });
        });
}

fn rebuild_settings_modal_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut settings_ui: ResMut<SettingsUiState>,
    mut audio_settings: ResMut<AudioSettingsState>,
    shortcuts: Res<InventoryShortcutsState>,
    vertical_aim_key: Res<VerticalAimKeyState>,
    container_q: Query<(Entity, Option<&Children>), With<SettingsSlotsContainer>>,
    vaim_cont_q: Query<(Entity, Option<&Children>), With<SettingsVerticalAimContainer>>,
    audio_cont_q: Query<(Entity, Option<&Children>), With<SettingsAudioContainer>>,
    mut status_q: Query<(&mut Text, &mut TextColor), With<SettingsStatusText>>,
) {
    if !settings_ui.is_open || (!settings_ui.dirty && !audio_settings.dirty) {
        return;
    }
    let rebuild_slots = settings_ui.dirty;
    let rebuild_audio = settings_ui.dirty || audio_settings.dirty;
    settings_ui.dirty = false;
    audio_settings.dirty = false;

    if let Ok((mut st_txt, mut st_col)) = status_q.single_mut() {
        if let Some((msg, is_err)) = &settings_ui.status_message {
            **st_txt = msg.clone();
            st_col.0 = if *is_err {
                Color::srgb(1.0, 0.42, 0.42)
            } else {
                Color::srgb(0.55, 0.92, 0.65)
            };
        } else {
            **st_txt = "Select a slot to assign a shortcut.".to_string();
            st_col.0 = Color::srgb(0.70, 0.85, 0.95);
        }
    }

    if rebuild_audio {
        if let Ok((ac_ent, ac_ch)) = audio_cont_q.single() {
            clear_children(&mut commands, ac_ch);
            commands.entity(ac_ent).with_children(|ac| {
                spawn_audio_slider_row(
                    ac,
                    "MASTER VOLUME",
                    None,
                    audio_settings.master_volume,
                    audio_settings.master_muted,
                    Color::srgb(0.98, 0.80, 0.24),
                );
                for cat in AudioCategory::ALL {
                    let idx = cat.index();
                    spawn_audio_slider_row(
                        ac,
                        cat.label(),
                        Some(idx),
                        audio_settings.channel_volumes[idx],
                        audio_settings.channel_muted[idx],
                        Color::srgb(0.24, 0.78, 0.96),
                    );
                }
                ac.spawn(Node {
                    width: percent(100),
                    justify_content: JustifyContent::FlexEnd,
                    margin: UiRect::top(px(4.0)),
                    ..default()
                })
                .with_children(|rrow| {
                    rrow.spawn((
                        UiButton,
            Hovered::default(),
                        AudioResetBtn,
                        Node {
                            padding: UiRect::axes(px(10.0), px(4.0)),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(5.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.45, 0.65, 0.85, 0.8)),
                        BackgroundColor(Color::srgba(0.10, 0.16, 0.24, 0.95)),
                    ))
                    .with_children(|rb| {
                        rb.spawn((
                            Pickable::IGNORE,
                            Text::new("RESET AUDIO TO CONFIG"),
                            TextFont {
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.85, 0.94, 1.0)),
                        ));
                    });
                });
            });
        }
    }

    if rebuild_slots {
    if let Ok((vaim_ent, vaim_ch)) = vaim_cont_q.single() {
        clear_children(&mut commands, vaim_ch);
        let is_listening = settings_ui.listening_vertical_aim;
        let key_lbl = if is_listening {
            "PRESS KEY...".to_string()
        } else {
            vertical_aim_key.binding.label()
        };
        commands.entity(vaim_ent).with_children(|row| {
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(2.0),
                ..default()
            })
            .with_children(|info| {
                info.spawn((
                    Text::new("3D ELEVATION AIM (HOLD)"),
                    TextFont {
                        font_size: FontSize::Px(12.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.88, 0.55)),
                ));
                info.spawn((
                    Text::new("Hold to shoot at exact 3D cursor height; release for straight horizontal shots."),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.68, 0.78, 0.88)),
                ));
            });

            row.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(6.0),
                ..default()
            })
            .with_children(|btns| {
                btns.spawn((
                    UiButton,
            Hovered::default(),
                    SettingsVerticalAimBindBtn,
                    Node {
                        min_width: px(78.0),
                        padding: UiRect::axes(px(10.0), px(6.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BorderColor::all(if is_listening {
                        Color::srgb(1.0, 0.82, 0.25)
                    } else {
                        Color::srgb(0.38, 0.72, 0.95)
                    }),
                    BackgroundColor(if is_listening {
                        Color::srgba(0.28, 0.22, 0.06, 0.95)
                    } else {
                        Color::srgba(0.08, 0.12, 0.18, 0.95)
                    }),
                ))
                .with_children(|b| {
                    b.spawn((
                        Pickable::IGNORE,
                        Text::new(key_lbl),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(if is_listening {
                            Color::srgb(1.0, 0.88, 0.35)
                        } else {
                            Color::srgb(0.45, 0.92, 1.0)
                        }),
                    ));
                });

                btns.spawn((
                    UiButton,
            Hovered::default(),
                    SettingsVerticalAimResetBtn,
                    Node {
                        padding: UiRect::axes(px(8.0), px(6.0)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.45, 0.60, 0.75, 0.75)),
                    BackgroundColor(Color::srgba(0.10, 0.15, 0.22, 0.92)),
                ))
                .with_children(|rb| {
                    rb.spawn((
                        Pickable::IGNORE,
                        Text::new("RESET"),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.85, 0.92, 1.0)),
                    ));
                });
            });
        });
    }
    if let Ok((cont_ent, ch)) = container_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(cont_ent).with_children(|row| {
            for idx in 0..items_cfg.max_inventory_slots.min(6) {
                let is_listening = settings_ui.listening_slot == Some(idx);
                let bound = shortcuts.bindings[idx];
                let border_c = if is_listening {
                    Color::srgb(1.0, 0.82, 0.25)
                } else if bound.is_some() {
                    Color::srgb(0.38, 0.72, 0.95)
                } else {
                    Color::srgba(0.32, 0.42, 0.52, 0.75)
                };

                row.spawn(Node {
                    width: px(84.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(5.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        UiButton,
            Hovered::default(),
                        SettingsSlotBindBtn(idx),
                        Node {
                            width: px(84.0),
                            height: px(92.0),
                            padding: UiRect::all(px(6.0)),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1.5)),
                            border_radius: BorderRadius::all(px(8.0)),
                            ..default()
                        },
                        BorderColor::all(border_c),
                        BackgroundColor(if is_listening {
                            Color::srgba(0.28, 0.22, 0.06, 0.95)
                        } else {
                            Color::srgba(0.08, 0.11, 0.16, 0.95)
                        }),
                    ))
                    .with_children(|slot| {
                        slot.spawn((
                            Pickable::IGNORE,
                            Text::new(format!("SLOT {}", idx + 1)),
                            TextFont {
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.72, 0.80, 0.90)),
                        ));

                        if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == idx) {
                            if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                                spawn_item_icon_badge(slot, &asset_server, &mut icon_cache, def, 34.0, 9.5);
                            }
                        } else {
                            slot.spawn((
                                Pickable::IGNORE,
                                Node {
                                    width: px(34.0),
                                    height: px(34.0),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(px(1.0)),
                                    border_radius: BorderRadius::all(px(5.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgba(0.28, 0.35, 0.42, 0.5)),
                                BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.8)),
                            ));
                        }

                        let key_txt = if is_listening {
                            "PRESS...".to_string()
                        } else if let Some(b) = bound {
                            b.label()
                        } else {
                            "NONE".to_string()
                        };
                        slot.spawn((
                            Pickable::IGNORE,
                            Text::new(key_txt),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(if is_listening {
                                Color::srgb(1.0, 0.88, 0.35)
                            } else if bound.is_some() {
                                Color::srgb(0.45, 0.92, 1.0)
                            } else {
                                Color::srgb(0.55, 0.62, 0.70)
                            }),
                        ));
                    });

                    if bound.is_some() {
                        col.spawn((
                            UiButton,
            Hovered::default(),
                            SettingsSlotClearBtn(idx),
                            Node {
                                padding: UiRect::axes(px(8.0), px(2.0)),
                                border: UiRect::all(px(1.0)),
                                border_radius: BorderRadius::all(px(4.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.75, 0.35, 0.35, 0.8)),
                            BackgroundColor(Color::srgba(0.25, 0.08, 0.08, 0.90)),
                        ))
                        .with_children(|cb| {
                            cb.spawn((
                                Pickable::IGNORE,
                                Text::new("CLEAR"),
                                TextFont {
                                    font_size: FontSize::Px(9.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.75, 0.75)),
                            ));
                        });
                    }
                });
            }
        });
    }
    }
}

fn inventory_shortcut_trigger_system(
    ctx: Res<ShopContextSnapshot>,
    ui_state: Res<ShopUiState>,
    settings_ui: Res<SettingsUiState>,
    shortcuts: Res<InventoryShortcutsState>,
    items_cfg: Res<ItemsConfig>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut teleport_preview: ResMut<TeleportPreviewState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
) {
    if !ctx.in_game || ui_state.is_open || settings_ui.is_open || ctx.chat_open {
        teleport_preview.active = false;
        return;
    }
    if ctx.is_dead
        || ctx.is_teleporting
        || ctx.teleport_cooldown_remaining > 0.0
        || ctx.tp_slot.is_none()
    {
        teleport_preview.active = false;
    }

    let tp_key = ShortcutInput::from_config_str(&items_cfg.teleport.shortcut_key);
    if tp_key.just_pressed(&keys, &mouse)
        && ctx.tp_slot.as_ref().is_some_and(|it| it.count > 0)
        && !ctx.is_dead
        && !ctx.is_teleporting
        && ctx.teleport_cooldown_remaining <= 0.0
    {
        teleport_preview.active = !teleport_preview.active;
    }

    for (slot_idx, binding) in shortcuts.bindings.iter().enumerate() {
        if let Some(input) = binding {
            if input.just_pressed(&keys, &mouse) {
                if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == slot_idx) {
                    if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                        if def.active_ability == ItemActiveAbility::ShadowBlade {
                            if !ctx.is_dead
                                && !ctx.is_invisible
                                && ctx.shadow_blade_cooldown_remaining <= 0.0
                            {
                                outgoing.0.push(ClientLobbyMessage::ActivateShadowBlade {
                                    inv_index: slot_idx,
                                });
                            }
                        } else if !def.modes.is_empty() {
                            outgoing.0.push(ClientLobbyMessage::CycleItemMode {
                                inv_index: slot_idx,
                            });
                        }
                    }
                }
            }
        }
    }
}

#[derive(SystemParam)]
struct HudDragQueries<'w, 's> {
    window_q: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    slot_q: Query<'w, 's, (&'static HudInventorySlotBtn, &'static RelativeCursorPosition)>,
    tp_btn_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudTpSlotBtn>, Without<HudInventorySlotBtn>)>,
    hud_inv_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudInventoryRoot>, Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>)>,
    hud_cart_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudCartBarRoot>, Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudInventoryRoot>)>,
    hud_stash_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudStashBarRoot>, Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudInventoryRoot>, Without<HudCartBarRoot>)>,
    stash_slot_q: Query<'w, 's, (&'static HudStashSlotBtn, &'static RelativeCursorPosition), (Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudInventoryRoot>, Without<HudCartBarRoot>, Without<HudStashBarRoot>)>,
    stash_take_all_q: Query<'w, 's, Entity, (Added<Pressed>, With<HudStashTakeAllBtn>)>,
    sell_menu_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<SellConfirmModalRoot>, Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudInventoryRoot>, Without<HudCartBarRoot>, Without<HudStashBarRoot>, Without<HudStashSlotBtn>)>,
    settings_btn_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<SettingsToggleBtn>, Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudInventoryRoot>, Without<HudCartBarRoot>, Without<HudStashBarRoot>, Without<HudStashSlotBtn>, Without<SellConfirmModalRoot>)>,
    drone_btn_rel_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudCourierSelectBtn>, Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudInventoryRoot>, Without<HudCartBarRoot>, Without<HudStashBarRoot>, Without<HudStashSlotBtn>, Without<SellConfirmModalRoot>, Without<SettingsToggleBtn>)>,
    ghost_q: Query<'w, 's, (&'static mut Node, &'static mut Visibility, &'static mut BorderColor), With<DragGhostRoot>>,
    ghost_txt_q: Query<'w, 's, &'static mut Text, (With<DragGhostLabelText>, Without<DragGhostTargetText>)>,
    ghost_target_q: Query<'w, 's, (&'static mut Text, &'static mut TextColor), (With<DragGhostTargetText>, Without<DragGhostLabelText>)>,
}

fn hud_inventory_drag_and_drop_system(
    ctx: Res<ShopContextSnapshot>,
    items_cfg: Res<ItemsConfig>,
    courier_ctrl: Res<CourierControlState>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut drag_state: ResMut<InventoryDragState>,
    mut drag_bridge: ResMut<InventoryDragDropBridge>,
    mut ui_state: ResMut<ShopUiState>,
    mut teleport_preview: ResMut<TeleportPreviewState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut q: HudDragQueries,
) {
    if !ctx.in_game {
        drag_state.pressed_slot = None;
        drag_state.dragging_slot = None;
        drag_bridge.is_dragging = false;
        drag_bridge.dragging_slot = None;
        drag_bridge.hovered_world_target = None;
        ui_state.is_pointer_over_hud_ui = false;
        ui_state.is_dragging_item = false;
        if let Ok((mut gn, _, _)) = q.ghost_q.single_mut() {
            if gn.display != Display::None {
                gn.display = Display::None;
            }
        }
        return;
    }

    let over_stash_bar = q.hud_stash_rel_q.single().is_ok_and(is_cursor_inside);
    let over_hud = q.hud_inv_rel_q.single().is_ok_and(is_cursor_inside)
        || (!ui_state.cart.is_empty() && q.hud_cart_rel_q.single().is_ok_and(is_cursor_inside))
        || over_stash_bar
        || (ui_state.pending_sell_slot.is_some()
            && q.sell_menu_rel_q.single().is_ok_and(is_cursor_inside))
        || q.settings_btn_rel_q.single().is_ok_and(is_cursor_inside);
    ui_state.is_pointer_over_hud_ui = over_hud;

    for _ in &q.stash_take_all_q {
        outgoing.0.push(ClientLobbyMessage::TakeAllStashToHero);
        ui_state.suppress_world_click_this_frame = true;
    }

    if ctx.inspected_is_read_only {
        drag_state.pressed_slot = None;
        drag_state.dragging_slot = None;
        drag_bridge.is_dragging = false;
        drag_bridge.dragging_slot = None;
        ui_state.is_dragging_item = false;
        if let Ok((mut gn, _, _)) = q.ghost_q.single_mut() {
            if gn.display != Display::None {
                gn.display = Display::None;
            }
        }
        return;
    }

    let cursor_pos = q.window_q.single().ok().and_then(|w| w.cursor_position());
    let hovered_slot = q
        .slot_q
        .iter()
        .find_map(|(btn, rel)| is_cursor_inside(rel).then_some(btn.inv_index));
    let hovered_tp = q.tp_btn_q.single().is_ok_and(is_cursor_inside);
    let hovered_drone_btn = q.drone_btn_rel_q.single().is_ok_and(is_cursor_inside);

    // کلیک چپ روی اسلات TP (اگر درگ نشود تلپورت فعال می‌شود؛ همچنین قابل درگ به سمت کوریر است)
    if mouse.just_pressed(MouseButton::Left) && hovered_tp {
        if let Some(cpos) = cursor_pos {
            if ctx.tp_slot.as_ref().is_some_and(|it| it.count > 0) {
                drag_state.pressed_slot = Some(usize::MAX);
                drag_state.pressed_is_courier = false;
                drag_state.press_cursor = cpos;
                drag_state.dragging_slot = None;
                ui_state.suppress_world_click_this_frame = true;
            }
        }
    }

    let hovered_stash_slot = if !ctx.stash.is_empty() {
        q.stash_slot_q
            .iter()
            .find_map(|(btn, rel)| is_cursor_inside(rel).then_some(btn.stash_index))
    } else {
        None
    };

    // کلیک راست روی اسلات‌های اینونتوری هیرو، کوریر یا Stash -> منوی فروش / انتقال به Stash
    if mouse.just_pressed(MouseButton::Right) {
        if let Some(st_idx) = hovered_stash_slot {
            if ctx.stash.iter().any(|it| it.slot == st_idx) {
                ui_state.pending_sell_slot = Some(st_idx);
                ui_state.pending_sell_is_courier = false;
                ui_state.pending_sell_is_stash = true;
                ui_state.suppress_world_click_this_frame = true;
                ui_state.dirty = true;
            }
            return;
        }
        if let Some(slot_idx) = hovered_slot {
            let is_cour = courier_ctrl.selected;
            let has_item = if is_cour {
                ctx.courier_inventory.iter().any(|it| it.slot == slot_idx)
            } else {
                ctx.inventory.iter().any(|it| it.slot == slot_idx)
            };
            if has_item {
                ui_state.pending_sell_slot = Some(slot_idx);
                ui_state.pending_sell_is_courier = is_cour;
                ui_state.pending_sell_is_stash = false;
                ui_state.suppress_world_click_this_frame = true;
                ui_state.dirty = true;
            }
            return;
        }
    }

    // کلیک چپ روی اسلات Stash -> انتقال سریع به هیرو (یا کوریر در صورت انتخاب)
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(st_idx) = hovered_stash_slot {
            if ctx.stash.iter().any(|it| it.slot == st_idx) {
                if courier_ctrl.selected {
                    outgoing
                        .0
                        .push(ClientLobbyMessage::MoveStashItemToCourier { stash_slot: st_idx });
                } else {
                    outgoing
                        .0
                        .push(ClientLobbyMessage::MoveStashItemToHero { stash_slot: st_idx });
                }
                ui_state.suppress_world_click_this_frame = true;
                return;
            }
        }
    }

    // شروع فشردن کلیک چپ روی اسلات اینونتوری (هم هیرو و هم کوریر)
    if mouse.just_pressed(MouseButton::Left) && !hovered_tp {
        if let (Some(slot_idx), Some(cpos)) = (hovered_slot, cursor_pos) {
            let is_cour = courier_ctrl.selected;
            let has_item = if is_cour {
                ctx.courier_inventory.iter().any(|it| it.slot == slot_idx)
            } else {
                ctx.inventory.iter().any(|it| it.slot == slot_idx)
            };
            if has_item {
                drag_state.pressed_slot = Some(slot_idx);
                drag_state.pressed_is_courier = is_cour;
                drag_state.press_cursor = cpos;
                drag_state.dragging_slot = None;
                ui_state.suppress_world_click_this_frame = true;
            }
        }
    }

    if mouse.pressed(MouseButton::Left) {
        if let (Some(from_slot), Some(cpos)) = (drag_state.pressed_slot, cursor_pos) {
            let threshold = items_cfg.shop_ui.drag_threshold_px;
            if drag_state.dragging_slot.is_none() && cpos.distance(drag_state.press_cursor) > threshold {
                drag_state.dragging_slot = Some(from_slot);
                drag_state.dragging_is_courier = drag_state.pressed_is_courier;
                let item_id_opt = if from_slot == usize::MAX {
                    ctx.tp_slot.as_ref().map(|it| it.item_id.as_str())
                } else if drag_state.dragging_is_courier {
                    ctx.courier_inventory
                        .iter()
                        .find(|it| it.slot == from_slot)
                        .map(|it| it.item_id.as_str())
                } else {
                    ctx.inventory
                        .iter()
                        .find(|it| it.slot == from_slot)
                        .map(|it| it.item_id.as_str())
                };
                if let Some(raw_id) = item_id_opt {
                    let lbl = if parse_recipe_target(raw_id).is_some() {
                        "REC".to_string()
                    } else if let Some(def) = items_cfg.get_item(raw_id) {
                        def.short_code.clone()
                    } else {
                        String::new()
                    };
                    if let Ok(mut gt) = q.ghost_txt_q.single_mut() {
                        **gt = lbl;
                    }
                }
            }

            if drag_state.dragging_slot.is_some() {
                let effective_target = if !drag_state.dragging_is_courier && hovered_drone_btn {
                    Some(WorldDropTarget::Courier)
                } else {
                    drag_bridge.hovered_world_target
                };

                if let Ok((mut gn, mut gv, mut gbc)) = q.ghost_q.single_mut() {
                    gn.display = Display::Flex;
                    gn.left = px(cpos.x - 26.0);
                    gn.top = px(cpos.y - 23.0);
                    if *gv != Visibility::Visible {
                        *gv = Visibility::Visible;
                    }
                    *gbc = match effective_target {
                        Some(WorldDropTarget::Courier) => BorderColor::all(Color::srgb(0.28, 0.90, 1.0)),
                        Some(WorldDropTarget::Hero) => BorderColor::all(Color::srgb(0.26, 0.95, 0.52)),
                        None => BorderColor::all(Color::srgb(1.0, 0.88, 0.35)),
                    };
                }
                if let Ok((mut tt, mut tc)) = q.ghost_target_q.single_mut() {
                    match effective_target {
                        Some(WorldDropTarget::Courier) => {
                            **tt = "-> COURIER".to_string();
                            *tc = TextColor(Color::srgb(0.35, 0.94, 1.0));
                        }
                        Some(WorldDropTarget::Hero) => {
                            **tt = "-> HERO".to_string();
                            *tc = TextColor(Color::srgb(0.32, 0.98, 0.56));
                        }
                        None => {
                            **tt = if drag_state.dragging_is_courier {
                                "DROP ON HERO".to_string()
                            } else {
                                "DROP ON DRONE".to_string()
                            };
                            *tc = TextColor(Color::srgba(0.75, 0.82, 0.90, 0.85));
                        }
                    }
                }
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        if let Some(from_slot) = drag_state.dragging_slot.take() {
            let from_courier = drag_state.dragging_is_courier;
            ui_state.suppress_world_click_this_frame = true;

            if over_stash_bar && from_slot != usize::MAX {
                if from_courier {
                    outgoing
                        .0
                        .push(ClientLobbyMessage::MoveCourierItemToStash { courier_slot: from_slot });
                } else {
                    outgoing
                        .0
                        .push(ClientLobbyMessage::MoveHeroItemToStash { hero_slot: from_slot });
                }
            } else if !from_courier
                && (drag_bridge.hovered_world_target == Some(WorldDropTarget::Courier)
                    || hovered_drone_btn)
            {
                outgoing.0.push(ClientLobbyMessage::TransferHeroItemToCourier {
                    hero_slot: from_slot,
                });
                drag_bridge.clear_hero_move_target = true;
            } else if from_courier
                && drag_bridge.hovered_world_target == Some(WorldDropTarget::Hero)
            {
                outgoing.0.push(ClientLobbyMessage::TransferCourierItemToHero {
                    courier_slot: from_slot,
                });
            } else if !from_courier && from_slot != usize::MAX {
                if let Some(to_slot) = hovered_slot {
                    if to_slot != from_slot {
                        outgoing.0.push(ClientLobbyMessage::SwapInventorySlots {
                            from_slot,
                            to_slot,
                        });
                    }
                }
            }
        } else if let Some(clicked_slot) = drag_state.pressed_slot {
            ui_state.suppress_world_click_this_frame = true;
            if clicked_slot == usize::MAX && hovered_tp {
                if ctx.tp_slot.as_ref().is_some_and(|it| it.count > 0)
                    && !ctx.is_dead
                    && !ctx.is_teleporting
                    && ctx.teleport_cooldown_remaining <= 0.0
                {
                    teleport_preview.active = !teleport_preview.active;
                    teleport_preview.wait_left_release_after_confirm = true;
                    teleport_preview.suppress_fire_this_frame = true;
                }
            } else if !drag_state.pressed_is_courier && hovered_slot == Some(clicked_slot) {
                if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == clicked_slot) {
                    if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                        if def.active_ability == ItemActiveAbility::ShadowBlade {
                            if !ctx.is_dead
                                && !ctx.is_invisible
                                && ctx.shadow_blade_cooldown_remaining <= 0.0
                            {
                                outgoing.0.push(ClientLobbyMessage::ActivateShadowBlade {
                                    inv_index: clicked_slot,
                                });
                                teleport_preview.wait_left_release_after_confirm = true;
                                teleport_preview.suppress_fire_this_frame = true;
                            }
                        } else if !def.modes.is_empty() {
                            outgoing.0.push(ClientLobbyMessage::CycleItemMode {
                                inv_index: clicked_slot,
                            });
                        }
                        // باگ ۳ رفع شد: کلیک چپ روی آیتم پسیو در اینونتوری دیگر شاپ را باز نمی‌کند!
                    }
                }
            }
        }
        drag_state.pressed_slot = None;
        if let Ok((mut gn, _, _)) = q.ghost_q.single_mut() {
            if gn.display != Display::None {
                gn.display = Display::None;
            }
        }
    }

    let is_dragging_now = drag_state.dragging_slot.is_some();
    drag_bridge.is_dragging = is_dragging_now;
    drag_bridge.drag_source_is_courier = drag_state.dragging_is_courier;
    drag_bridge.dragging_slot = drag_state.dragging_slot;
    if !is_dragging_now {
        drag_bridge.hovered_world_target = None;
    }
    ui_state.is_dragging_item = drag_state.pressed_slot.is_some() || is_dragging_now;
}

#[derive(SystemParam)]
struct CourierHudParams<'w, 's> {
    deliver_btn_q: Query<'w, 's, Entity, (Added<Pressed>, With<HudCourierDeliverBtn>)>,
    return_btn_q: Query<'w, 's, Entity, (Added<Pressed>, With<HudCourierReturnBtn>)>,
    select_btn_q: Query<'w, 's, Entity, (Added<Pressed>, With<HudCourierSelectBtn>)>,
    select_txt_q: Query<'w, 's, &'static mut Text, With<HudCourierSelectBtnText>>,
    dead_banner_q: Query<'w, 's, &'static mut Node, With<HudCourierDeadBanner>>,
    dead_txt_q: Query<'w, 's, &'static mut Text, (With<HudCourierDeadText>, Without<HudCourierSelectBtnText>)>,
}

fn courier_hud_and_hotkey_system(
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    settings_ui: Res<SettingsUiState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut courier_ctrl: ResMut<CourierControlState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut p: CourierHudParams,
) {
    if !ctx.in_game {
        courier_ctrl.selected = false;
        return;
    }

    for _ in &p.deliver_btn_q {
        outgoing.0.push(ClientLobbyMessage::CourierDeliver);
        ui_state.suppress_world_click_this_frame = true;
    }
    for _ in &p.return_btn_q {
        outgoing.0.push(ClientLobbyMessage::CourierReturnToFountain);
        ui_state.suppress_world_click_this_frame = true;
    }
    for _ in &p.select_btn_q {
        courier_ctrl.selected = !courier_ctrl.selected;
        ui_state.suppress_world_click_this_frame = true;
    }

    if !ui_state.is_open && !settings_ui.is_open && !ctx.chat_open {
        if keys.just_pressed(KeyCode::F3) {
            outgoing.0.push(ClientLobbyMessage::CourierDeliver);
        }
        if keys.just_pressed(KeyCode::F4) {
            outgoing.0.push(ClientLobbyMessage::CourierReturnToFountain);
        }
        if keys.just_pressed(KeyCode::F2) {
            courier_ctrl.selected = true;
        }
        if keys.just_pressed(KeyCode::F1) {
            courier_ctrl.selected = false;
        }
    }

    if let Ok(mut st) = p.select_txt_q.single_mut() {
        let desired = if courier_ctrl.selected {
            "[F1] HERO"
        } else {
            "[F2] DRONE"
        };
        if **st != desired {
            **st = desired.to_string();
        }
    }

    if let Ok(mut db_node) = p.dead_banner_q.single_mut() {
        let desired_disp = if ctx.in_game && ctx.courier_is_dead {
            Display::Flex
        } else {
            Display::None
        };
        if db_node.display != desired_disp {
            db_node.display = desired_disp;
        }
    }
    if ctx.courier_is_dead {
        if let Ok(mut dt) = p.dead_txt_q.single_mut() {
            let txt = format!(
                "COURIER DEAD  |  RESPAWNS IN {:.0}s",
                ctx.courier_respawn_remaining.ceil()
            );
            if **dt != txt {
                **dt = txt;
            }
        }
    }
}

fn update_hud_buffs_bar_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut container_q: Query<(Entity, &mut Node, Option<&Children>), With<HudBuffsContainer>>,
    mut last_buffs: Local<Vec<ActiveBuffSnapshot>>,
    mut last_in_game: Local<bool>,
) {
    let Ok((ent, mut node, children_opt)) = container_q.single_mut() else {
        return;
    };
    if !ctx.in_game || ctx.active_buffs.is_empty() {
        if node.display != Display::None {
            node.display = Display::None;
            clear_children(&mut commands, children_opt);
            last_buffs.clear();
        }
        *last_in_game = ctx.in_game;
        return;
    }

    if *last_in_game == ctx.in_game && *last_buffs == ctx.active_buffs {
        return;
    }
    *last_in_game = ctx.in_game;
    *last_buffs = ctx.active_buffs.clone();

    node.display = Display::Flex;
    clear_children(&mut commands, children_opt);

    commands.entity(ent).with_children(|col| {
        for buff in &ctx.active_buffs {
            let ratio = (buff.remaining_secs / buff.total_secs.max(0.01)).clamp(0.0, 1.0);
            let bar_col = Color::srgb(buff.color_rgb.0, buff.color_rgb.1, buff.color_rgb.2);
            col.spawn((
                Pickable::IGNORE,
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: px(6.0),
                    padding: UiRect::axes(px(6.0), px(3.0)),
                    border_radius: BorderRadius::all(px(14.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.88)),
            ))
            .with_children(|row| {
                row.spawn((
                    Pickable::IGNORE,
                    Node {
                        width: px(28.0),
                        height: px(28.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::MAX,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(bar_col),
                    BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.95)),
                ))
                .with_children(|circle| {
                    if let Some(def) = items_cfg.get_item(&buff.item_id) {
                        spawn_item_icon_badge(circle, &asset_server, &mut icon_cache, def, 24.0, 9.0);
                    }
                });

                row.spawn((
                    Pickable::IGNORE,
                    Node {
                        width: px(164.0),
                        height: px(18.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.35, 0.48, 0.62, 0.65)),
                    BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.92)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Pickable::IGNORE,
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(0.0),
                            top: px(0.0),
                            width: percent(ratio * 100.0),
                            height: percent(100),
                            ..default()
                        },
                        BackgroundColor(bar_col.with_alpha(0.62)),
                    ));
                    bar.spawn((
                        Pickable::IGNORE,
                        Text::new(format!("{}  {:.1}s", buff.label, buff.remaining_secs)),
                        TextFont {
                            font_size: FontSize::Px(10.5),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        }
    });
}

fn hud_cart_interaction_system(
    ctx: Res<ShopContextSnapshot>,
    items_cfg: Res<ItemsConfig>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ui_state: ResMut<ShopUiState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut audio_q: ResMut<AudioEventQueue>,
    cart_btn_q: Query<(&HudCartSlotBtn, &RelativeCursorPosition)>,
    rem_btn_q: Query<&HudCartRemoveBtn, Added<Pressed>>,
    clear_btn_q: Query<Entity, (Added<Pressed>, With<HudCartClearBtn>)>,
) {
    if !ctx.in_game {
        return;
    }
    for rem in &rem_btn_q {
        if rem.0 < ui_state.cart.len() {
            ui_state.cart.remove(rem.0);
            ui_state.suppress_world_click_this_frame = true;
            ui_state.dirty = true;
            return;
        }
    }
    for _ in &clear_btn_q {
        if !ui_state.cart.is_empty() {
            ui_state.cart.clear();
            ui_state.suppress_world_click_this_frame = true;
            ui_state.dirty = true;
            return;
        }
    }
    if ui_state.cart.is_empty() || !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let hovered = cart_btn_q
        .iter()
        .find_map(|(btn, rel)| is_cursor_inside(rel).then_some((btn.cart_index, btn.item_id.clone())));
    let Some((_cart_idx, item_id)) = hovered else {
        return;
    };
    ui_state.suppress_world_click_this_frame = true;
    if try_send_buy_item(&item_id, &items_cfg, &ctx, &mut outgoing, &mut audio_q) {
        ui_state.dirty = true;
    }
}

#[derive(PartialEq, Eq, Clone)]
enum TooltipTarget {
    Item {
        item_id: String,
        active_mode: Option<usize>,
        from_hud: bool,
    },
    RecipeScroll {
        parent_item_id: String,
    },
}

#[derive(SystemParam)]
struct TooltipQueries<'w, 's> {
    window_q: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    shop_btn_q: Query<'w, 's, (&'static Hovered, &'static ShopSelectItemBtn)>,
    recipe_btn_q: Query<'w, 's, (&'static Hovered, &'static ShopRecipeNodeBtn)>,
    hud_slot_q: Query<'w, 's, (&'static HudInventorySlotBtn, &'static RelativeCursorPosition)>,
    hud_tp_q: Query<'w, 's, &'static RelativeCursorPosition, (With<HudTpSlotBtn>, Without<HudInventorySlotBtn>)>,
    hud_cart_q: Query<'w, 's, (&'static HudCartSlotBtn, &'static RelativeCursorPosition), (Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>)>,
    hud_stash_q: Query<'w, 's, (&'static HudStashSlotBtn, &'static RelativeCursorPosition), (Without<HudInventorySlotBtn>, Without<HudTpSlotBtn>, Without<HudCartSlotBtn>)>,
    tip_q: Query<'w, 's, (Entity, &'static mut Node, Option<&'static Children>), With<ShopHoverTooltipRoot>>,
}

fn shop_hover_tooltip_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    ui_state: Res<ShopUiState>,
    courier_ctrl: Res<CourierControlState>,
    mut q: TooltipQueries,
    mut last_target: Local<Option<TooltipTarget>>,
) {
    let Ok((tip_ent, mut tip_node, tip_ch)) = q.tip_q.single_mut() else {
        return;
    };
    if !ctx.in_game || ui_state.is_dragging_item {
        tip_node.display = Display::None;
        *last_target = None;
        return;
    }

    let mut target: Option<TooltipTarget> = None;

    // ۱. هاور داخل پنجره شاپ (کاتالوگ و هرم)
    if ui_state.is_open {
        target = q
            .shop_btn_q
            .iter()
            .find_map(|(hovered, btn)| {
                hovered.get().then(|| TooltipTarget::Item {
                    item_id: btn.0.clone(),
                    active_mode: None,
                    from_hud: false,
                })
            })
            .or_else(|| {
                q.recipe_btn_q.iter().find_map(|(hovered, btn)| {
                    hovered.get().then(|| TooltipTarget::RecipeScroll {
                        parent_item_id: btn.0.clone(),
                    })
                })
            });
    }

    // ۲. هاور روی اینونتوری HUD (هیرو / کوریر / واحد اینسپکت‌شده)، اسلات TP و سبد خرید سریع
    if target.is_none() && ui_state.pending_sell_slot.is_none() {
        if let Some(slot_idx) = q
            .hud_slot_q
            .iter()
            .find_map(|(btn, rel)| is_cursor_inside(rel).then_some(btn.inv_index))
        {
            let active_inv = if let Some(ref insp_inv) = ctx.inspected_inventory {
                insp_inv
            } else if courier_ctrl.selected {
                &ctx.courier_inventory
            } else {
                &ctx.inventory
            };
            if let Some(inv_item) = active_inv.iter().find(|it| it.slot == slot_idx) {
                target = Some(if let Some(tid) = parse_recipe_target(&inv_item.item_id) {
                    TooltipTarget::RecipeScroll {
                        parent_item_id: tid.to_string(),
                    }
                } else {
                    TooltipTarget::Item {
                        item_id: inv_item.item_id.clone(),
                        active_mode: Some(inv_item.active_mode),
                        from_hud: true,
                    }
                });
            }
        } else if q.hud_tp_q.single().is_ok_and(is_cursor_inside) {
            if let Some(ref tp) = ctx.tp_slot {
                target = Some(TooltipTarget::Item {
                    item_id: tp.item_id.clone(),
                    active_mode: None,
                    from_hud: true,
                });
            }
        } else if !ctx.stash.is_empty()
            && q.hud_stash_q.iter().any(|(_, rel)| is_cursor_inside(rel))
        {
            if let Some(st_idx) = q
                .hud_stash_q
                .iter()
                .find_map(|(btn, rel)| is_cursor_inside(rel).then_some(btn.stash_index))
            {
                if let Some(st_item) = ctx.stash.iter().find(|it| it.slot == st_idx) {
                    target = Some(if let Some(tid) = parse_recipe_target(&st_item.item_id) {
                        TooltipTarget::RecipeScroll {
                            parent_item_id: tid.to_string(),
                        }
                    } else {
                        TooltipTarget::Item {
                            item_id: st_item.item_id.clone(),
                            active_mode: Some(st_item.active_mode),
                            from_hud: true,
                        }
                    });
                }
            }
        } else if !ui_state.cart.is_empty() {
            if let Some(cid) = q
                .hud_cart_q
                .iter()
                .find_map(|(btn, rel)| is_cursor_inside(rel).then(|| btn.item_id.clone()))
            {
                target = Some(if let Some(tid) = parse_recipe_target(&cid) {
                    TooltipTarget::RecipeScroll {
                        parent_item_id: tid.to_string(),
                    }
                } else {
                    TooltipTarget::Item {
                        item_id: cid,
                        active_mode: None,
                        from_hud: true,
                    }
                });
            }
        }
    }

    let Some(ref cur_target) = target else {
        tip_node.display = Display::None;
        *last_target = None;
        return;
    };

    if let Ok(win) = q.window_q.single() {
        if let Some(cpos) = win.cursor_position() {
            let tip_w = items_cfg.shop_ui.tooltip_width_px;
            let max_x = (win.width() - tip_w - 16.0).max(12.0);
            let left_x = if cpos.x + 18.0 > max_x {
                (cpos.x - tip_w - 14.0).max(12.0)
            } else {
                (cpos.x + 18.0).min(max_x)
            };
            tip_node.left = px(left_x);

            if cpos.y > win.height() * 0.55 {
                tip_node.top = Val::Auto;
                tip_node.bottom = px((win.height() - cpos.y + 18.0).max(16.0));
            } else {
                tip_node.bottom = Val::Auto;
                let max_y = (win.height() - 240.0).max(12.0);
                tip_node.top = px((cpos.y + 14.0).min(max_y));
            }
        }
    }

    tip_node.display = Display::Flex;
    if last_target.as_ref() == Some(cur_target) {
        return;
    }
    *last_target = Some(cur_target.clone());
    clear_children(&mut commands, tip_ch);

    match cur_target {
        TooltipTarget::RecipeScroll { parent_item_id } => {
            let Some(item) = items_cfg.get_item(parent_item_id) else {
                return;
            };
            let fee = items_cfg.recipe_fee(parent_item_id);
            commands.entity(tip_ent).with_children(|tip| {
                tip.spawn((
                    Text::new(format!("{} - Recipe", item.name)),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(Color::srgb(1.0, 0.88, 0.42)),
                ));
                tip.spawn((
                    Text::new(format!("Combine Fee: {} Gold  |  Total Item Cost: {} Gold", fee, item.cost)),
                    TextFont { font_size: FontSize::Px(11.0), ..default() },
                    TextColor(Color::srgb(0.95, 0.82, 0.32)),
                ));
                tip.spawn((
                    Text::new("Click to pay the remaining combine cost once you own the required components (or buy the full item now)."),
                    TextFont { font_size: FontSize::Px(10.5), ..default() },
                    TextColor(Color::srgb(0.76, 0.82, 0.90)),
                ));
            });
        }
        TooltipTarget::Item {
            item_id,
            active_mode,
            from_hud,
        } => {
            let Some(item) = items_cfg.get_item(item_id) else {
                return;
            };
            let (eff_cost, _) = effective_cost_for_display(&items_cfg, item_id, &ctx);
            let sell_val = items_cfg.sell_price(item_id);
            let rec_fee = items_cfg.recipe_fee(item_id);

            commands.entity(tip_ent).with_children(|tip| {
                tip.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    spawn_item_icon_badge(hdr, &asset_server, &mut icon_cache, item, 34.0, 10.5);
                    hdr.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(1.0),
                        ..default()
                    })
                    .with_children(|t| {
                        t.spawn((
                            Text::new(&item.name),
                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                            TextColor(Color::WHITE),
                        ));
                        let sub = if *from_hud {
                            format!("Tier {} | Cost: {}g | Sell: {}g", item.tier, item.cost, sell_val)
                        } else if rec_fee > 0 {
                            format!("{} Gold (Recipe: {}g) | Sell: {}g", eff_cost, rec_fee, sell_val)
                        } else {
                            format!("{} Gold | Sell: {}g", eff_cost, sell_val)
                        };
                        t.spawn((
                            Text::new(sub),
                            TextFont { font_size: FontSize::Px(10.5), ..default() },
                            TextColor(Color::srgb(1.0, 0.84, 0.28)),
                        ));
                    });
                });

                let (buffs, debuffs) = item.stats.split_buffs_and_debuffs();
                for line in buffs {
                    tip.spawn((
                        Text::new(line),
                        TextFont { font_size: FontSize::Px(11.0), ..default() },
                        TextColor(Color::srgb(0.42, 0.94, 0.62)),
                    ));
                }
                for line in debuffs {
                    tip.spawn((
                        Text::new(format!("{} [DEBUFF]", line)),
                        TextFont { font_size: FontSize::Px(11.0), ..default() },
                        TextColor(Color::srgb(0.96, 0.40, 0.38)),
                    ));
                }

                if !item.modes.is_empty() {
                    for (m_i, m) in item.modes.iter().enumerate() {
                        let (mb, md) = m.stats.split_buffs_and_debuffs();
                        let mc = m.color_rgb;
                        let prefix = if *active_mode == Some(m_i) { "> " } else { "  " };
                        tip.spawn((
                            Text::new(format!("{}[{}] {}", prefix, m.name, mb.join(", "))),
                            TextFont { font_size: FontSize::Px(10.5), ..default() },
                            TextColor(Color::srgb(mc.0, mc.1, mc.2)),
                        ));
                        if !md.is_empty() {
                            tip.spawn((
                                Text::new(format!("    Debuff: {}", md.join(", "))),
                                TextFont { font_size: FontSize::Px(10.0), ..default() },
                                TextColor(Color::srgb(0.95, 0.42, 0.40)),
                            ));
                        }
                    }
                }

                if !item.description.is_empty() {
                    tip.spawn((
                        Text::new(&item.description),
                        TextFont { font_size: FontSize::Px(10.5), ..default() },
                        TextColor(Color::srgb(0.76, 0.82, 0.90)),
                    ));
                }

                if *from_hud {
                    let hint = if courier_ctrl.selected {
                        "Right-Click: Sell (in shop) | Drag onto Hero: Deliver item"
                    } else {
                        "Right-Click: Sell (in shop) | Drag onto Courier: Hand over"
                    };
                    tip.spawn((
                        Text::new(hint),
                        TextFont { font_size: FontSize::Px(9.5), ..default() },
                        TextColor(Color::srgb(0.56, 0.72, 0.88)),
                    ));
                }
            });
        }
    }
}


fn update_shop_notice_toast_system(
    time: Res<Time>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    mut audio_q: ResMut<AudioEventQueue>,
    mut root_q: Query<(&mut Node, &mut BorderColor, &mut BackgroundColor), With<HudShopNoticeRoot>>,
    mut txt_q: Query<(&mut Text, &mut TextColor), With<HudShopNoticeText>>,
) {
    if !ctx.in_game {
        ui_state.notice_timer = 0.0;
        if let Ok((mut n, _, _)) = root_q.single_mut() {
            if n.display != Display::None {
                n.display = Display::None;
            }
        }
        return;
    }

    if ctx.shop_notice_seq != ui_state.last_notice_seq && !ctx.shop_notice_text.is_empty() {
        ui_state.last_notice_seq = ctx.shop_notice_seq;
        ui_state.notice_text = ctx.shop_notice_text.clone();
        ui_state.notice_is_error = ctx.shop_notice_is_error;
        ui_state.notice_timer = items_cfg.shop_ui.shop_error_toast_secs;
        if ctx.shop_notice_is_error {
            if ctx.shop_notice_text.contains("FULL") {
                audio_q.0.push(ClientAudioEvent::InventoryFull);
            } else {
                audio_q.0.push(ClientAudioEvent::NotEnoughGold);
            }
        }
        if let Ok((mut txt, mut col)) = txt_q.single_mut() {
            **txt = ui_state.notice_text.clone();
            *col = TextColor(if ui_state.notice_is_error {
                Color::srgb(1.0, 0.85, 0.85)
            } else {
                Color::srgb(0.65, 1.0, 0.80)
            });
        }
        if let Ok((_, mut bc, mut bg)) = root_q.single_mut() {
            if ui_state.notice_is_error {
                *bc = BorderColor::all(Color::srgb(0.95, 0.30, 0.30));
                *bg = BackgroundColor(Color::srgba(0.32, 0.06, 0.06, 0.96));
            } else {
                *bc = BorderColor::all(Color::srgb(0.24, 0.90, 0.52));
                *bg = BackgroundColor(Color::srgba(0.06, 0.24, 0.14, 0.96));
            }
        }
    }

    if ui_state.notice_timer > 0.0 {
        ui_state.notice_timer = (ui_state.notice_timer - time.delta_secs()).max(0.0);
        if let Ok((mut n, _, _)) = root_q.single_mut() {
            if n.display != Display::Flex {
                n.display = Display::Flex;
            }
        }
    } else if let Ok((mut n, _, _)) = root_q.single_mut() {
        if n.display != Display::None {
            n.display = Display::None;
        }
    }
}