use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::PrimaryWindow;
use moba_shared::{
    ClientLobbyMessage, InventoryItemState, ItemCategory, ItemDef, ItemsConfig,
};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShopCategoryFilter {
    All,
    PrimaryWeapon,
    Pistol,
    Mobility,
    Defense,
    Upgrades,
}

impl ShopCategoryFilter {
    fn all() -> &'static [Self] {
        &[
            Self::All,
            Self::PrimaryWeapon,
            Self::Pistol,
            Self::Mobility,
            Self::Defense,
            Self::Upgrades,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::PrimaryWeapon => "Primary",
            Self::Pistol => "Pistol",
            Self::Mobility => "Mobility",
            Self::Defense => "Defense",
            Self::Upgrades => "Upgrades",
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

#[derive(Resource, Default)]
pub struct ShopContextSnapshot {
    pub in_game: bool,
    pub chat_open: bool,
    pub in_fountain: bool,
    pub is_dead: bool,
    pub gold: u32,
    pub inventory: Vec<InventoryItemState>,
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
}

#[derive(Resource, Default)]
pub struct InventoryShortcutsState {
    pub bindings: [Option<ShortcutInput>; 6],
}

#[derive(Resource, Default)]
pub struct SettingsUiState {
    pub is_open: bool,
    pub listening_slot: Option<usize>,
    pub just_started_listening: bool,
    pub status_message: Option<(String, bool)>,
    pub dirty: bool,
}

#[derive(Resource, Default)]
struct InventoryDragState {
    pressed_slot: Option<usize>,
    press_cursor: Vec2,
    dragging_slot: Option<usize>,
}

#[derive(Resource)]
pub struct ShopUiState {
    pub is_open: bool,
    pub active_category: ShopCategoryFilter,
    pub selected_item_id: String,
    pub cart: Vec<String>,
    pub search_query: String,
    pub search_focused: bool,
    pub pending_sell_slot: Option<usize>,
    pub prev_inventory: Vec<InventoryItemState>,
    pub dirty: bool,
}

impl Default for ShopUiState {
    fn default() -> Self {
        Self {
            is_open: false,
            active_category: ShopCategoryFilter::All,
            selected_item_id: String::new(),
            cart: Vec::new(),
            search_query: String::new(),
            search_focused: false,
            pending_sell_slot: None,
            prev_inventory: Vec::new(),
            dirty: true,
        }
    }
}

#[derive(Resource, Default)]
struct ItemIconCache(std::collections::HashMap<String, Option<Handle<Image>>>);

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
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    sell_confirm_modal_system,
                    settings_modal_interaction_system,
                    rebuild_settings_modal_system,
                    inventory_shortcut_trigger_system,
                    hud_inventory_drag_and_drop_system,
                )
                    .chain(),
            );
    }
}

fn spawn_item_icon_badge(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    icon_cache: &mut ItemIconCache,
    item: &ItemDef,
    size_px: f32,
    font_px: f32,
) {
    let c = item.icon_color_rgb;
    let bg = Color::srgb(c.0 * 0.32, c.1 * 0.32, c.2 * 0.32);
    let border = Color::srgb(c.0, c.1, c.2);

    parent
        .spawn((
            Pickable::IGNORE,
            Node {
                width: px(size_px),
                height: px(size_px),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(border),
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

// -----------------------------------------------------------------------------
// مارکرهای رابط کاربری شاپ و اینونتوری
// -----------------------------------------------------------------------------

#[derive(Component)]
struct HudInventoryRoot;

#[derive(Component)]
struct HudInventorySlotsRow;

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
struct ShopScrollablePanel;

#[derive(Component)]
struct ShopCatalogGrid;

#[derive(Component)]
struct ShopSelectItemBtn(String);

#[derive(Component)]
struct ShopInspectorColumn;

#[derive(Component)]
struct ShopBuyNowBtn(String);

#[derive(Component)]
struct ShopAddToCartBtn(String);

#[derive(Component)]
struct ShopCartAndInvColumn;

#[derive(Component)]
struct ShopRemoveCartBtn(usize);

#[derive(Component)]
struct ShopBuyCartBtn;

#[derive(Component)]
struct ShopClearCartBtn;

#[derive(Component)]
struct ShopSellInvBtn(usize);

#[derive(Component)]
struct ShopCycleModeBtn(usize);

#[derive(Component)]
struct SellConfirmModalRoot;

#[derive(Component)]
struct SellConfirmCardContent;

#[derive(Component)]
struct SellConfirmAcceptBtn(usize);

#[derive(Component)]
struct SellConfirmCancelBtn;

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
struct SettingsStatusText;

#[derive(Component)]
struct DragGhostRoot;

#[derive(Component)]
struct DragGhostLabelText;

fn spawn_shop_and_inventory_ui(mut commands: Commands) {
    // ۱. نوار ۶ اسلاته اینونتوری + دکمه [B] SHOP در پایین صفحه
    commands
        .spawn((
            HudInventoryRoot,
            GlobalZIndex(190),
            Node {
                position_type: PositionType::Absolute,
                left: px(565.0),
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
                Interaction::default(),
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
                    Text::new("[B] SHOP"),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.88, 0.36)),
                ));
            });
        });

    // ۲. پنجره جامع شاپ در وسط صفحه
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
                    Node {
                        width: px(1110.0),
                        height: px(670.0),
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
                                    font_size: FontSize::Px(19.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.95, 0.88, 0.55)),
                            ));
                            title_col.spawn((
                                Text::new("Shift+Click: Set Cart  |  Shift+Ctrl+Click: Append to Cart"),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.62, 0.74, 0.88)),
                            ));
                        });

                        hdr.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: px(12.0),
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
                                        Text::new("IN FOUNTAIN"),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
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
                                            font_size: FontSize::Px(14.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(1.0, 0.88, 0.30)),
                                    ));
                                });

                            right
                                .spawn((
                                    Interaction::default(),
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

                    // بدنه ۳ ستونه شاپ
                    win.spawn(Node {
                        width: percent(100),
                        height: px(585.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(12.0),
                        ..default()
                    })
                    .with_children(|body| {
                        // ستون چپ: نوار جستجو + تب‌های دسته‌بندی + شبکه اسکرول‌پذیر آیتم‌ها
                        body.spawn(Node {
                            width: px(406.0),
                            height: percent(100),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(7.0),
                            ..default()
                        })
                        .with_children(|left| {
                            // نوار سرچ آیتم با تایپ
                            left.spawn(Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Row,
                                column_gap: px(6.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|srow| {
                                srow.spawn((
                                    Interaction::default(),
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
                                        Text::new("Search items... (Click to type)"),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgba(0.60, 0.68, 0.78, 0.85)),
                                    ));
                                });

                                srow.spawn((
                                    Interaction::default(),
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
                                    row_gap: px(5.0),
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
                                    height: px(492.0),
                                    padding: UiRect::all(px(8.0)),
                                    flex_direction: FlexDirection::Row,
                                    flex_wrap: FlexWrap::Wrap,
                                    align_content: AlignContent::FlexStart,
                                    column_gap: px(6.0),
                                    row_gap: px(6.0),
                                    border: UiRect::all(px(1.0)),
                                    border_radius: BorderRadius::all(px(8.0)),
                                    overflow: Overflow::scroll_y(),
                                    ..default()
                                },
                                BorderColor::all(Color::srgba(0.25, 0.32, 0.42, 0.7)),
                                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.85)),
                            ));
                        });

                        // ستون وسط: جزئیات آیتم + باف/دیباف + ساختار درختی ساخت (Recipe Tree)
                        body.spawn((
                            ShopInspectorColumn,
                            ShopScrollablePanel,
                            RelativeCursorPosition::default(),
                            ScrollPosition::default(),
                            Node {
                                width: px(370.0),
                                height: percent(100),
                                padding: UiRect::all(px(12.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(8.0),
                                border: UiRect::all(px(1.0)),
                                border_radius: BorderRadius::all(px(8.0)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.28, 0.38, 0.50, 0.75)),
                            BackgroundColor(Color::srgba(0.06, 0.09, 0.14, 0.90)),
                        ));

                        // ستون راست: سبد خرید + اینونتوری فعلی (فروش / تغییر حالت)
                        body.spawn((
                            ShopCartAndInvColumn,
                            ShopScrollablePanel,
                            RelativeCursorPosition::default(),
                            ScrollPosition::default(),
                            Node {
                                width: px(278.0),
                                height: percent(100),
                                padding: UiRect::all(px(10.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: px(8.0),
                                border: UiRect::all(px(1.0)),
                                border_radius: BorderRadius::all(px(8.0)),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(0.32, 0.42, 0.52, 0.75)),
                            BackgroundColor(Color::srgba(0.05, 0.08, 0.12, 0.90)),
                        ));
                    });
                });
        });

    // ۳. دکمه تنظیمات (بالا سمت چپ صفحه)
    commands
        .spawn((
            Interaction::default(),
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
                            Interaction::default(),
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

    // ۵. پنجره تاییدیه فروش آیتم
    commands
        .spawn((
            SellConfirmModalRoot,
            GlobalZIndex(260),
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
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.74)),
        ))
        .with_children(|backdrop| {
            backdrop.spawn((
                SellConfirmCardContent,
                Node {
                    width: px(380.0),
                    padding: UiRect::all(px(18.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12.0),
                    border: UiRect::all(px(2.0)),
                    border_radius: BorderRadius::all(px(12.0)),
                    ..default()
                },
                BorderColor::all(Color::srgb(0.95, 0.65, 0.25)),
                BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.98)),
            ));
        });

    // ۶. شبح درگ و دراپ آیتم در اینونتوری
    commands
        .spawn((
            DragGhostRoot,
            Pickable::IGNORE,
            GlobalZIndex(270),
            Node {
                position_type: PositionType::Absolute,
                width: px(46.0),
                height: px(46.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2.0)),
                border_radius: BorderRadius::all(px(6.0)),
                display: Display::None,
                ..default()
            },
            BorderColor::all(Color::srgb(1.0, 0.88, 0.35)),
            BackgroundColor(Color::srgba(0.12, 0.16, 0.24, 0.92)),
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
        });
}

#[derive(SystemParam)]
struct ShopInteractionParams<'w, 's> {
    toggle_btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<HudShopToggleBtn>)>,
    close_btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ShopCloseBtn>)>,
    search_box_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ShopSearchBoxBtn>)>,
    search_clr_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ShopSearchClearBtn>)>,
    cat_btn_q: Query<'w, 's, (&'static Interaction, &'static ShopCategoryBtn), Changed<Interaction>>,
    select_btn_q: Query<'w, 's, (&'static Interaction, &'static ShopSelectItemBtn), Changed<Interaction>>,
    buy_now_q: Query<'w, 's, (&'static Interaction, &'static ShopBuyNowBtn), Changed<Interaction>>,
    add_cart_q: Query<'w, 's, (&'static Interaction, &'static ShopAddToCartBtn), Changed<Interaction>>,
    rem_cart_q: Query<'w, 's, (&'static Interaction, &'static ShopRemoveCartBtn), Changed<Interaction>>,
    buy_cart_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ShopBuyCartBtn>)>,
    clear_cart_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<ShopClearCartBtn>)>,
    sell_inv_q: Query<'w, 's, (&'static Interaction, &'static ShopSellInvBtn), Changed<Interaction>>,
    cycle_mode_q: Query<'w, 's, (&'static Interaction, &'static ShopCycleModeBtn), Changed<Interaction>>,
    modal_q: Query<'w, 's, &'static mut Node, With<ShopModalRoot>>,
    hud_root_q: Query<'w, 's, &'static mut Node, (With<HudInventoryRoot>, Without<ShopModalRoot>)>,
    catalog_scroll_q: Query<'w, 's, &'static mut ScrollPosition, With<ShopCatalogGrid>>,
}

fn add_item_expanded_to_cart(
    items_cfg: &ItemsConfig,
    item_id: &str,
    inventory: &[InventoryItemState],
    cart: &mut Vec<String>,
    override_cart: bool,
) {
    if override_cart {
        cart.clear();
    }
    let mut sim_inv = inventory.to_vec();
    for existing in cart.iter() {
        sim_inv.push(InventoryItemState {
            item_id: existing.clone(),
            active_mode: 0,
            slot: 0,
        });
    }
    let expanded = items_cfg.expand_missing_recipe_items(item_id, &sim_inv);
    cart.extend(expanded);
}

fn prune_newly_bought_from_cart(
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
            let mut to_remove = cnt - old_cnt;
            while to_remove > 0 {
                if let Some(pos) = cart.iter().position(|c| c.as_str() == *id) {
                    cart.remove(pos);
                    to_remove -= 1;
                } else {
                    break;
                }
            }
        }
    }
}

fn shop_hotkey_and_button_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut p: ShopInteractionParams,
) {
    if ui_state.selected_item_id.is_empty() {
        if let Some(first) = items_cfg.items.first() {
            ui_state.selected_item_id = first.id.clone();
            ui_state.dirty = true;
        }
    }

    if ui_state.prev_inventory != ctx.inventory {
        let prev = ui_state.prev_inventory.clone();
        prune_newly_bought_from_cart(&prev, &ctx.inventory, &mut ui_state.cart);
        ui_state.prev_inventory = ctx.inventory.clone();
        ui_state.dirty = true;
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
        }
    }

    for inter in &p.toggle_btn_q {
        if *inter == Interaction::Pressed {
            ui_state.is_open = !ui_state.is_open;
            ui_state.search_focused = false;
            ui_state.dirty = true;
        }
    }
    for inter in &p.close_btn_q {
        if *inter == Interaction::Pressed {
            ui_state.is_open = false;
            ui_state.search_focused = false;
            ui_state.dirty = true;
        }
    }

    for inter in &p.search_box_q {
        if *inter == Interaction::Pressed {
            ui_state.search_focused = true;
            ui_state.dirty = true;
        }
    }
    for inter in &p.search_clr_q {
        if *inter == Interaction::Pressed {
            ui_state.search_query.clear();
            ui_state.search_focused = false;
            if let Ok(mut sp) = p.catalog_scroll_q.single_mut() {
                sp.y = 0.0;
            }
            ui_state.dirty = true;
        }
    }

    for (inter, cat_btn) in &p.cat_btn_q {
        if *inter == Interaction::Pressed {
            ui_state.active_category = cat_btn.0;
            ui_state.search_focused = false;
            if let Ok(mut sp) = p.catalog_scroll_q.single_mut() {
                sp.y = 0.0;
            }
            ui_state.dirty = true;
        }
    }

    let shift_held = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let ctrl_held =
        keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);

    for (inter, sel_btn) in &p.select_btn_q {
        if *inter == Interaction::Pressed {
            ui_state.search_focused = false;
            ui_state.selected_item_id = sel_btn.0.clone();
            if shift_held {
                let inv_clone = ctx.inventory.clone();
                add_item_expanded_to_cart(
                    &items_cfg,
                    &sel_btn.0,
                    &inv_clone,
                    &mut ui_state.cart,
                    !ctrl_held,
                );
            }
            ui_state.dirty = true;
        }
    }

    for (inter, buy_btn) in &p.buy_now_q {
        if *inter == Interaction::Pressed {
            ui_state.search_focused = false;
            outgoing.0.push(ClientLobbyMessage::BuyItem {
                item_id: buy_btn.0.clone(),
            });
            ui_state.dirty = true;
        }
    }

    for (inter, add_btn) in &p.add_cart_q {
        if *inter == Interaction::Pressed {
            ui_state.search_focused = false;
            let inv_clone = ctx.inventory.clone();
            add_item_expanded_to_cart(
                &items_cfg,
                &add_btn.0,
                &inv_clone,
                &mut ui_state.cart,
                false,
            );
            ui_state.dirty = true;
        }
    }

    for (inter, rem_btn) in &p.rem_cart_q {
        if *inter == Interaction::Pressed && rem_btn.0 < ui_state.cart.len() {
            ui_state.cart.remove(rem_btn.0);
            ui_state.dirty = true;
        }
    }

    for inter in &p.buy_cart_q {
        if *inter == Interaction::Pressed && !ui_state.cart.is_empty() {
            outgoing.0.push(ClientLobbyMessage::BuyCart {
                item_ids: ui_state.cart.clone(),
            });
            ui_state.cart.clear();
            ui_state.dirty = true;
        }
    }

    for inter in &p.clear_cart_q {
        if *inter == Interaction::Pressed && !ui_state.cart.is_empty() {
            ui_state.cart.clear();
            ui_state.dirty = true;
        }
    }

    for (inter, sell_btn) in &p.sell_inv_q {
        if *inter == Interaction::Pressed {
            ui_state.pending_sell_slot = Some(sell_btn.0);
            ui_state.dirty = true;
        }
    }

    for (inter, cyc_btn) in &p.cycle_mode_q {
        if *inter == Interaction::Pressed {
            outgoing.0.push(ClientLobbyMessage::CycleItemMode {
                inv_index: cyc_btn.0,
            });
            ui_state.dirty = true;
        }
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
                    if ch.is_control() || ui_state.search_query.chars().count() >= 32 {
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
        if rel_cursor.normalized.is_some_and(|p| (-0.5..=0.5).contains(&p.x) && (-0.5..=0.5).contains(&p.y)) {
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn rebuild_shop_modal_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    mut gold_q: Query<&mut Text, With<ShopHeaderGoldText>>,
    mut ftn_badge_q: Query<(&mut BorderColor, &mut BackgroundColor), With<ShopFountainBadge>>,
    mut ftn_txt_q: Query<(&mut Text, &mut TextColor), (With<ShopFountainBadgeText>, Without<ShopHeaderGoldText>)>,
    mut search_box_q: Query<&mut BorderColor, (With<ShopSearchBoxBtn>, Without<ShopFountainBadge>)>,
    mut search_txt_q: Query<(&mut Text, &mut TextColor), (With<ShopSearchText>, Without<ShopHeaderGoldText>, Without<ShopFountainBadgeText>)>,
    tabs_q: Query<(Entity, Option<&Children>), With<ShopCategoryTabsRow>>,
    grid_q: Query<(Entity, Option<&Children>), With<ShopCatalogGrid>>,
    insp_q: Query<(Entity, Option<&Children>), With<ShopInspectorColumn>>,
    cart_q: Query<(Entity, Option<&Children>), With<ShopCartAndInvColumn>>,
) {
    if !ui_state.is_open {
        return;
    }
    let header_dirty = ctx.is_changed();
    if !ui_state.dirty && !header_dirty {
        return;
    }
    let full_rebuild = ui_state.dirty;
    if ui_state.dirty {
        ui_state.bypass_change_detection().dirty = false;
    }

    if let Ok(mut g_txt) = gold_q.single_mut() {
        **g_txt = format!("GOLD: {}", ctx.gold);
    }
    if let Ok((mut b_col, mut bg_col)) = ftn_badge_q.single_mut() {
        if ctx.in_fountain && !ctx.is_dead {
            *b_col = BorderColor::all(Color::srgb(0.24, 0.88, 0.48));
            *bg_col = BackgroundColor(Color::srgba(0.06, 0.24, 0.12, 0.9));
        } else {
            *b_col = BorderColor::all(Color::srgb(0.95, 0.65, 0.18));
            *bg_col = BackgroundColor(Color::srgba(0.26, 0.16, 0.04, 0.9));
        }
    }
    if let Ok((mut f_txt, mut f_col)) = ftn_txt_q.single_mut() {
        if ctx.is_dead {
            **f_txt = "DEAD - BROWSE / QUEUE ONLY".to_string();
            *f_col = TextColor(Color::srgb(0.98, 0.35, 0.32));
        } else if ctx.in_fountain {
            **f_txt = "IN FOUNTAIN - READY TO BUY & SELL".to_string();
            *f_col = TextColor(Color::srgb(0.35, 0.98, 0.60));
        } else {
            **f_txt = "OUTSIDE FOUNTAIN - BROWSE & CART ONLY".to_string();
            *f_col = TextColor(Color::srgb(0.98, 0.78, 0.28));
        }
    }

    if let Ok(mut sb_border) = search_box_q.single_mut() {
        *sb_border = BorderColor::all(if ui_state.search_focused {
            Color::srgb(0.96, 0.82, 0.28)
        } else {
            Color::srgba(0.35, 0.48, 0.62, 0.8)
        });
    }
    if let Ok((mut s_txt, mut s_col)) = search_txt_q.single_mut() {
        if ui_state.search_query.is_empty() {
            **s_txt = if ui_state.search_focused {
                "Type item name... |".to_string()
            } else {
                "Search items... (Click to type)".to_string()
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

    // ۱. بازسازی تب‌های دسته‌بندی
    if let Ok((tabs_ent, ch)) = tabs_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(tabs_ent).with_children(|row| {
            for &cat in ShopCategoryFilter::all() {
                let active = ui_state.active_category == cat;
                row.spawn((
                    Interaction::default(),
                    ShopCategoryBtn(cat),
                    Node {
                        padding: UiRect::axes(px(9.0), px(5.0)),
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
                        Text::new(cat.label()),
                        TextFont {
                            font_size: FontSize::Px(11.5),
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

    // ۲. بازسازی شبکه کاتالوگ آیتم‌ها با فیلتر دسته‌بندی و جستجو
    let query_lower = ui_state.search_query.trim().to_lowercase();
    if let Ok((grid_ent, ch)) = grid_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(grid_ent).with_children(|grid| {
            for item in &items_cfg.items {
                if !ui_state.active_category.matches(item.category) {
                    continue;
                }
                if !query_lower.is_empty()
                    && !item.name.to_lowercase().contains(&query_lower)
                    && !item.short_code.to_lowercase().contains(&query_lower)
                    && !item.description.to_lowercase().contains(&query_lower)
                {
                    continue;
                }
                let is_sel = ui_state.selected_item_id == item.id;
                let owned_count = ctx.inventory.iter().filter(|i| i.item_id == item.id).count();
                let (eff_cost, consumed) = items_cfg
                    .effective_purchase_cost(&item.id, &ctx.inventory)
                    .unwrap_or((item.cost, Vec::new()));
                let affordable = ctx.gold >= eff_cost;

                grid.spawn((
                    Interaction::default(),
                    ShopSelectItemBtn(item.id.clone()),
                    Node {
                        width: px(184.0),
                        padding: UiRect::all(px(7.0)),
                        flex_direction: FlexDirection::Row,
                        column_gap: px(8.0),
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(if is_sel { 2.0 } else { 1.0 })),
                        border_radius: BorderRadius::all(px(7.0)),
                        ..default()
                    },
                    BorderColor::all(if is_sel {
                        Color::srgb(0.98, 0.82, 0.26)
                    } else if owned_count > 0 {
                        Color::srgba(0.24, 0.85, 0.48, 0.75)
                    } else {
                        Color::srgba(0.26, 0.34, 0.44, 0.75)
                    }),
                    BackgroundColor(if is_sel {
                        Color::srgba(0.16, 0.20, 0.28, 0.96)
                    } else {
                        Color::srgba(0.08, 0.11, 0.16, 0.92)
                    }),
                ))
                .with_children(|card| {
                    spawn_item_icon_badge(card, &asset_server, &mut icon_cache, item, 38.0, 12.0);
                    card.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2.0),
                        ..default()
                    })
                    .with_children(|info| {
                        info.spawn((
                            Text::new(&item.name),
                            TextFont {
                                font_size: FontSize::Px(11.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.94, 0.96, 1.0)),
                        ));
                        let price_txt = if !consumed.is_empty() {
                            format!("{}g (Craft)", eff_cost)
                        } else {
                            format!("{}g  [T{}]", eff_cost, item.tier)
                        };
                        info.spawn((
                            Text::new(price_txt),
                            TextFont {
                                font_size: FontSize::Px(10.5),
                                ..default()
                            },
                            TextColor(if affordable {
                                Color::srgb(1.0, 0.84, 0.28)
                            } else {
                                Color::srgb(0.75, 0.42, 0.40)
                            }),
                        ));
                    });
                });
            }
        });
    }

    // ۳. بازسازی ستون وسط (جزئیات آیتم، باف/دیباف و ساختار درختی با آیکون/عکس)
    if let Ok((insp_ent, ch)) = insp_q.single() {
        clear_children(&mut commands, ch);
        if let Some(item) = items_cfg.get_item(&ui_state.selected_item_id) {
            let (eff_cost, consumed) = items_cfg
                .effective_purchase_cost(&item.id, &ctx.inventory)
                .unwrap_or((item.cost, Vec::new()));
            let can_buy = ctx.in_fountain
                && !ctx.is_dead
                && ctx.gold >= eff_cost
                && (ctx.inventory.len() - consumed.len() + 1) <= items_cfg.max_inventory_slots;

            commands.entity(insp_ent).with_children(|col| {
                // هدر آیتم انتخاب‌شده
                col.spawn(Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    spawn_item_icon_badge(hdr, &asset_server, &mut icon_cache, item, 52.0, 15.0);
                    hdr.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2.0),
                        ..default()
                    })
                    .with_children(|meta| {
                        meta.spawn((
                            Text::new(&item.name),
                            TextFont {
                                font_size: FontSize::Px(16.0),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        meta.spawn((
                            Text::new(format!(
                                "{}  |  Tier {}  |  Cost: {}g  (Sell: {}g)",
                                item.category.label(),
                                item.tier,
                                item.cost,
                                items_cfg.sell_price(&item.id)
                            )),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.95, 0.82, 0.35)),
                        ));
                    });
                });

                col.spawn((
                    Text::new(&item.description),
                    TextFont {
                        font_size: FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.80, 0.86, 0.94)),
                ));

                // باف‌ها و دیباف‌های پایه
                let (buffs, debuffs) = item.stats.split_buffs_and_debuffs();
                if !buffs.is_empty() || !debuffs.is_empty() {
                    col.spawn((
                        Text::new("STATS & EFFECTS:"),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.70, 0.82, 0.95)),
                    ));
                    for b in buffs {
                        col.spawn((
                            Text::new(format!("  {}", b)),
                            TextFont {
                                font_size: FontSize::Px(11.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.28, 0.95, 0.52)),
                        ));
                    }
                    for d in debuffs {
                        col.spawn((
                            Text::new(format!("  {}  [DEBUFF]", d)),
                            TextFont {
                                font_size: FontSize::Px(11.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.34, 0.34)),
                        ));
                    }
                }

                // حالت‌های مختلف آیتم
                if !item.modes.is_empty() {
                    col.spawn((
                        Text::new("SWITCHABLE MODES (Click in Inventory):"),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.80, 0.32)),
                    ));
                    for m in &item.modes {
                        let (mb, md) = m.stats.split_buffs_and_debuffs();
                        let mc = m.color_rgb;
                        col.spawn((
                            Text::new(format!("  [{}] {}", m.name, mb.join(", "))),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(mc.0, mc.1, mc.2)),
                        ));
                        if !md.is_empty() {
                            col.spawn((
                                Text::new(format!("      Debuff: {}", md.join(", "))),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.95, 0.38, 0.38)),
                            ));
                        }
                    }
                }

                // ساختار درختی ساخت: Built From (با آیکون/عکس هر کامپوننت)
                if !item.recipe.is_empty() {
                    col.spawn((
                        Text::new("BUILT FROM (RECIPE TREE):"),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 0.85, 0.98)),
                    ));
                    col.spawn(Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(6.0),
                        row_gap: px(6.0),
                        ..default()
                    })
                    .with_children(|row| {
                        let mut used = vec![false; ctx.inventory.len()];
                        for comp_id in &item.recipe {
                            let owned = ctx
                                .inventory
                                .iter()
                                .enumerate()
                                .position(|(i, inv)| !used[i] && &inv.item_id == comp_id)
                                .map(|i| {
                                    used[i] = true;
                                    true
                                })
                                .unwrap_or(false);
                            if let Some(comp) = items_cfg.get_item(comp_id) {
                                row.spawn((
                                    Interaction::default(),
                                    ShopSelectItemBtn(comp.id.clone()),
                                    Node {
                                        padding: UiRect::axes(px(7.0), px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: px(6.0),
                                        border: UiRect::all(px(1.0)),
                                        border_radius: BorderRadius::all(px(6.0)),
                                        ..default()
                                    },
                                    BorderColor::all(if owned {
                                        Color::srgb(0.24, 0.88, 0.48)
                                    } else {
                                        Color::srgba(0.45, 0.55, 0.68, 0.8)
                                    }),
                                    BackgroundColor(if owned {
                                        Color::srgba(0.06, 0.22, 0.12, 0.9)
                                    } else {
                                        Color::srgba(0.10, 0.14, 0.20, 0.9)
                                    }),
                                ))
                                .with_children(|chip| {
                                    spawn_item_icon_badge(chip, &asset_server, &mut icon_cache, comp, 26.0, 9.0);
                                    let txt = if owned {
                                        format!("{} [OWNED]", comp.name)
                                    } else {
                                        format!("{} ({}g)", comp.name, comp.cost)
                                    };
                                    chip.spawn((
                                        Text::new(txt),
                                        TextFont {
                                            font_size: FontSize::Px(10.5),
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                });
                            }
                        }
                    });
                }

                // ساختار درختی ساخت: Builds Into (با آیکون/عکس)
                let upgrades = items_cfg.builds_into(&item.id);
                if !upgrades.is_empty() {
                    col.spawn((
                        Text::new("BUILDS INTO:"),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.82, 0.35)),
                    ));
                    col.spawn(Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(6.0),
                        row_gap: px(6.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for upg in upgrades {
                            row.spawn((
                                Interaction::default(),
                                ShopSelectItemBtn(upg.id.clone()),
                                Node {
                                    padding: UiRect::axes(px(7.0), px(5.0)),
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    column_gap: px(6.0),
                                    border: UiRect::all(px(1.0)),
                                    border_radius: BorderRadius::all(px(6.0)),
                                    ..default()
                                },
                                BorderColor::all(Color::srgba(0.85, 0.70, 0.25, 0.85)),
                                BackgroundColor(Color::srgba(0.18, 0.15, 0.06, 0.9)),
                            ))
                            .with_children(|chip| {
                                spawn_item_icon_badge(chip, &asset_server, &mut icon_cache, upg, 24.0, 9.0);
                                chip.spawn((
                                    Text::new(format!("{} ({}g)", upg.name, upg.cost)),
                                    TextFont {
                                        font_size: FontSize::Px(10.5),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(1.0, 0.90, 0.50)),
                                ));
                            });
                        }
                    });
                }

                // دکمه‌های خرید مستقیم و افزودن به سبد خرید
                col.spawn(Node {
                    width: percent(100),
                    margin: UiRect::top(px(8.0)),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(8.0),
                    ..default()
                })
                .with_children(|btns| {
                    btns.spawn((
                        Interaction::default(),
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
                        let lbl = if !consumed.is_empty() {
                            format!("CRAFT NOW ({}g)", eff_cost)
                        } else {
                            format!("BUY NOW ({}g)", eff_cost)
                        };
                        b.spawn((
                            Text::new(lbl),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(if can_buy {
                                Color::WHITE
                            } else {
                                Color::srgba(0.65, 0.65, 0.68, 0.8)
                            }),
                        ));
                    });

                    btns.spawn((
                        Interaction::default(),
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
                            Text::new("+ CART"),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                });
            });
        }
    }

    // ۴. بازسازی ستون راست (سبد خرید + اینونتوری و فروش)
    if let Ok((cart_ent, ch)) = cart_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(cart_ent).with_children(|col| {
            col.spawn((
                Text::new(format!("SHOPPING CART ({})", ui_state.cart.len())),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.85, 1.0)),
            ));

            let mut total_cart_cost = 0_u32;
            let mut sim_inv = ctx.inventory.clone();
            for (c_idx, cid) in ui_state.cart.iter().enumerate() {
                if let Some(item) = items_cfg.get_item(cid) {
                    let (c_cost, consumed) = items_cfg
                        .effective_purchase_cost(cid, &sim_inv)
                        .unwrap_or((item.cost, Vec::new()));
                    total_cart_cost = total_cart_cost.saturating_add(c_cost);
                    let mut rem_sorted = consumed;
                    rem_sorted.sort_unstable_by(|a, b| b.cmp(a));
                    for idx in rem_sorted {
                        if idx < sim_inv.len() {
                            sim_inv.remove(idx);
                        }
                    }
                    sim_inv.push(InventoryItemState {
                        item_id: cid.clone(),
                        active_mode: 0,
                        slot: 0,
                    });

                    col.spawn((
                        Node {
                            width: percent(100),
                            padding: UiRect::axes(px(7.0), px(4.0)),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            column_gap: px(6.0),
                            border_radius: BorderRadius::all(px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.12, 0.18, 0.9)),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Interaction::default(),
                            ShopSelectItemBtn(item.id.clone()),
                            Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: px(6.0),
                                ..default()
                            },
                        ))
                        .with_children(|lbl| {
                            spawn_item_icon_badge(lbl, &asset_server, &mut icon_cache, item, 22.0, 8.5);
                            lbl.spawn((
                                Text::new(format!("{} ({}g)", item.name, c_cost)),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                        row.spawn((
                            Interaction::default(),
                            ShopRemoveCartBtn(c_idx),
                            Node {
                                padding: UiRect::axes(px(6.0), px(2.0)),
                                border_radius: BorderRadius::all(px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.35, 0.08, 0.08, 0.9)),
                        ))
                        .with_children(|x| {
                            x.spawn((
                                Text::new("X"),
                                TextFont {
                                    font_size: FontSize::Px(10.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                    });
                }
            }

            let can_buy_cart = ctx.in_fountain
                && !ctx.is_dead
                && !ui_state.cart.is_empty()
                && ctx.gold >= total_cart_cost
                && sim_inv.len() <= items_cfg.max_inventory_slots;

            col.spawn((
                Text::new(format!("Cart Total: {}g", total_cart_cost)),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.30)),
            ));

            col.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: px(6.0),
                ..default()
            })
            .with_children(|btns| {
                btns.spawn((
                    Interaction::default(),
                    ShopBuyCartBtn,
                    Node {
                        flex_grow: 1.0,
                        padding: UiRect::axes(px(8.0), px(6.0)),
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BorderColor::all(if can_buy_cart {
                        Color::srgb(0.24, 0.92, 0.52)
                    } else {
                        Color::srgba(0.4, 0.4, 0.45, 0.6)
                    }),
                    BackgroundColor(if can_buy_cart {
                        Color::srgba(0.08, 0.34, 0.16, 0.95)
                    } else {
                        Color::srgba(0.12, 0.12, 0.14, 0.8)
                    }),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("BUY CART"),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                btns.spawn((
                    Interaction::default(),
                    ShopClearCartBtn,
                    Node {
                        padding: UiRect::axes(px(8.0), px(6.0)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(5.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.55, 0.35, 0.35, 0.7)),
                    BackgroundColor(Color::srgba(0.20, 0.08, 0.08, 0.85)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("CLEAR"),
                        TextFont {
                            font_size: FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });

            // بخش اینونتوری و فروش آیتم‌ها
            col.spawn((
                Text::new(format!(
                    "YOUR INVENTORY ({}/{})",
                    ctx.inventory.len(),
                    items_cfg.max_inventory_slots
                )),
                TextFont {
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.85, 0.45)),
            ));

            let mut sorted_inv: Vec<&InventoryItemState> = ctx.inventory.iter().collect();
            sorted_inv.sort_by_key(|it| it.slot);
            for inv_item in sorted_inv {
                let inv_idx = inv_item.slot;
                if let Some(item) = items_cfg.get_item(&inv_item.item_id) {
                    let refund = items_cfg.sell_price(&item.id);
                    col.spawn((
                        Node {
                            width: percent(100),
                            padding: UiRect::all(px(6.0)),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            column_gap: px(5.0),
                            border: UiRect::all(px(1.0)),
                            border_radius: BorderRadius::all(px(5.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.28, 0.38, 0.48, 0.8)),
                        BackgroundColor(Color::srgba(0.08, 0.11, 0.15, 0.9)),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Interaction::default(),
                            ShopSelectItemBtn(item.id.clone()),
                            Node {
                                flex_grow: 1.0,
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: px(5.0),
                                ..default()
                            },
                        ))
                        .with_children(|lbl| {
                            spawn_item_icon_badge(lbl, &asset_server, &mut icon_cache, item, 22.0, 8.5);
                            lbl.spawn((
                                Text::new(&item.name),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });

                        if !item.modes.is_empty() {
                            let m_idx = inv_item.active_mode.min(item.modes.len() - 1);
                            let m_name = &item.modes[m_idx].name;
                            row.spawn((
                                Interaction::default(),
                                ShopCycleModeBtn(inv_idx),
                                Node {
                                    padding: UiRect::axes(px(5.0), px(2.0)),
                                    border_radius: BorderRadius::all(px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.18, 0.22, 0.40, 0.95)),
                            ))
                            .with_children(|mb| {
                                mb.spawn((
                                    Text::new(m_name),
                                    TextFont {
                                        font_size: FontSize::Px(10.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.75, 0.88, 1.0)),
                                ));
                            });
                        }

                        if ctx.in_fountain && !ctx.is_dead {
                            row.spawn((
                                Interaction::default(),
                                ShopSellInvBtn(inv_idx),
                                Node {
                                    padding: UiRect::axes(px(5.0), px(2.0)),
                                    border_radius: BorderRadius::all(px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.34, 0.16, 0.06, 0.95)),
                            ))
                            .with_children(|sb| {
                                sb.spawn((
                                    Text::new(format!("+{}g", refund)),
                                    TextFont {
                                        font_size: FontSize::Px(10.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(1.0, 0.85, 0.30)),
                                ));
                            });
                        }
                    });
                }
            }
        });
    }
}

fn rebuild_hud_inventory_bar_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    ui_state: Res<ShopUiState>,
    shortcuts: Res<InventoryShortcutsState>,
    slots_q: Query<(Entity, Option<&Children>), With<HudInventorySlotsRow>>,
    mut toggle_txt_q: Query<&mut Text, With<HudShopToggleText>>,
    mut last_inv: Local<Option<Vec<InventoryItemState>>>,
    mut last_cart_len: Local<usize>,
    mut last_bindings: Local<Option<[Option<ShortcutInput>; 6]>>,
) {
    if !ctx.in_game {
        return;
    }
    let cart_changed = *last_cart_len != ui_state.cart.len();
    let inv_changed = last_inv.as_ref() != Some(&ctx.inventory);
    let bind_changed = last_bindings.as_ref() != Some(&shortcuts.bindings);
    if !cart_changed && !inv_changed && !bind_changed {
        return;
    }
    *last_cart_len = ui_state.cart.len();
    *last_inv = Some(ctx.inventory.clone());
    *last_bindings = Some(shortcuts.bindings);

    if let Ok(mut t) = toggle_txt_q.single_mut() {
        if ui_state.cart.is_empty() {
            **t = "[B] SHOP".to_string();
        } else {
            **t = format!("[B] SHOP ({})", ui_state.cart.len());
        }
    }

    if let Ok((row_ent, ch)) = slots_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(row_ent).with_children(|row| {
            for idx in 0..items_cfg.max_inventory_slots {
                let key_lbl = shortcuts
                    .bindings
                    .get(idx)
                    .and_then(|b| *b)
                    .map(|b| b.label());
                if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == idx) {
                    if let Some(item) = items_cfg.get_item(&inv_item.item_id) {
                        row.spawn((
                            Interaction::default(),
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
                    Interaction::default(),
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
}


fn sell_confirm_modal_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut ui_state: ResMut<ShopUiState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    accept_q: Query<(&Interaction, &SellConfirmAcceptBtn), Changed<Interaction>>,
    cancel_q: Query<&Interaction, (Changed<Interaction>, With<SellConfirmCancelBtn>)>,
    mut modal_q: Query<&mut Node, With<SellConfirmModalRoot>>,
    card_q: Query<(Entity, Option<&Children>), With<SellConfirmCardContent>>,
    mut last_pending: Local<Option<usize>>,
) {
    for (inter, btn) in &accept_q {
        if *inter == Interaction::Pressed {
            outgoing.0.push(ClientLobbyMessage::SellItem { inv_index: btn.0 });
            ui_state.pending_sell_slot = None;
            ui_state.dirty = true;
        }
    }
    for inter in &cancel_q {
        if *inter == Interaction::Pressed {
            ui_state.pending_sell_slot = None;
        }
    }

    if !ctx.in_game || !ui_state.is_open {
        ui_state.pending_sell_slot = None;
    } else if let Some(slot) = ui_state.pending_sell_slot {
        if !ctx.inventory.iter().any(|it| it.slot == slot) {
            ui_state.pending_sell_slot = None;
        }
    }

    let desired_display = if ui_state.pending_sell_slot.is_some() {
        Display::Flex
    } else {
        Display::None
    };
    if let Ok(mut m_node) = modal_q.single_mut() {
        if m_node.display != desired_display {
            m_node.display = desired_display;
        }
    }

    if *last_pending == ui_state.pending_sell_slot {
        return;
    }
    *last_pending = ui_state.pending_sell_slot;

    let Some(slot) = ui_state.pending_sell_slot else {
        return;
    };
    let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == slot) else {
        return;
    };
    let Some(item) = items_cfg.get_item(&inv_item.item_id) else {
        return;
    };
    let refund = items_cfg.sell_price(&item.id);
    let pct = (items_cfg.sell_refund_ratio * 100.0).round() as u32;

    if let Ok((card_ent, ch)) = card_q.single() {
        clear_children(&mut commands, ch);
        commands.entity(card_ent).with_children(|card| {
            card.spawn((
                Text::new("CONFIRM ITEM SALE"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.78, 0.30)),
            ));

            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: px(10.0),
                ..default()
            })
            .with_children(|row| {
                spawn_item_icon_badge(row, &asset_server, &mut icon_cache, item, 42.0, 11.0);
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2.0),
                    ..default()
                })
                .with_children(|info| {
                    info.spawn((
                        Text::new(&item.name),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    info.spawn((
                        Text::new(format!("Original Cost: {}g  ->  Refund ({}%): +{}g", item.cost, pct, refund)),
                        TextFont {
                            font_size: FontSize::Px(11.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.45, 0.95, 0.55)),
                    ));
                });
            });

            card.spawn(Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(10.0),
                ..default()
            })
            .with_children(|btns| {
                btns.spawn((
                    Interaction::default(),
                    SellConfirmCancelBtn,
                    Node {
                        padding: UiRect::axes(px(14.0), px(8.0)),
                        border: UiRect::all(px(1.0)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.55, 0.60, 0.68)),
                    BackgroundColor(Color::srgba(0.14, 0.17, 0.22, 0.95)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("CANCEL"),
                        TextFont {
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });

                btns.spawn((
                    Interaction::default(),
                    SellConfirmAcceptBtn(slot),
                    Node {
                        padding: UiRect::axes(px(14.0), px(8.0)),
                        border: UiRect::all(px(1.5)),
                        border_radius: BorderRadius::all(px(6.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.95, 0.70, 0.25)),
                    BackgroundColor(Color::srgba(0.38, 0.22, 0.06, 0.96)),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new(format!("CONFIRM SELL (+{}g)", refund)),
                        TextFont {
                            font_size: FontSize::Px(12.0),
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.88, 0.35)),
                    ));
                });
            });
        });
    }
}

#[derive(SystemParam)]
struct SettingsInteractionParams<'w, 's> {
    toggle_btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<SettingsToggleBtn>)>,
    close_btn_q: Query<'w, 's, &'static Interaction, (Changed<Interaction>, With<SettingsCloseBtn>)>,
    bind_btn_q: Query<'w, 's, (&'static Interaction, &'static SettingsSlotBindBtn), Changed<Interaction>>,
    clear_btn_q: Query<'w, 's, (&'static Interaction, &'static SettingsSlotClearBtn), Changed<Interaction>>,
    toggle_node_q: Query<'w, 's, &'static mut Node, With<SettingsToggleBtn>>,
    modal_node_q: Query<'w, 's, &'static mut Node, (With<SettingsModalRoot>, Without<SettingsToggleBtn>)>,
}

fn settings_modal_interaction_system(
    ctx: Res<ShopContextSnapshot>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut settings_ui: ResMut<SettingsUiState>,
    mut shortcuts: ResMut<InventoryShortcutsState>,
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
        if let Ok(mut mn) = p.modal_node_q.single_mut() {
            if mn.display != Display::None {
                mn.display = Display::None;
            }
        }
        return;
    }

    for inter in &p.toggle_btn_q {
        if *inter == Interaction::Pressed {
            settings_ui.is_open = !settings_ui.is_open;
            settings_ui.listening_slot = None;
            settings_ui.status_message = None;
            settings_ui.dirty = true;
        }
    }
    for inter in &p.close_btn_q {
        if *inter == Interaction::Pressed {
            settings_ui.is_open = false;
            settings_ui.listening_slot = None;
            settings_ui.dirty = true;
        }
    }

    if settings_ui.is_open {
        for (inter, clr) in &p.clear_btn_q {
            if *inter == Interaction::Pressed {
                shortcuts.bindings[clr.0] = None;
                if settings_ui.listening_slot == Some(clr.0) {
                    settings_ui.listening_slot = None;
                }
                settings_ui.status_message = Some((format!("Slot {} shortcut cleared.", clr.0 + 1), false));
                settings_ui.dirty = true;
            }
        }

        for (inter, bbtn) in &p.bind_btn_q {
            if *inter == Interaction::Pressed {
                settings_ui.listening_slot = Some(bbtn.0);
                settings_ui.just_started_listening = true;
                settings_ui.status_message = Some((
                    format!("Listening for Slot {} — press ANY key or mouse button (ESC to cancel)...", bbtn.0 + 1),
                    false,
                ));
                settings_ui.dirty = true;
            }
        }

        if let Some(target_slot) = settings_ui.listening_slot {
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
                    if let Some(dup_idx) = shortcuts
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

fn rebuild_settings_modal_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut icon_cache: ResMut<ItemIconCache>,
    items_cfg: Res<ItemsConfig>,
    ctx: Res<ShopContextSnapshot>,
    mut settings_ui: ResMut<SettingsUiState>,
    shortcuts: Res<InventoryShortcutsState>,
    container_q: Query<(Entity, Option<&Children>), With<SettingsSlotsContainer>>,
    mut status_q: Query<(&mut Text, &mut TextColor), With<SettingsStatusText>>,
) {
    if !settings_ui.is_open || !settings_ui.dirty {
        return;
    }
    settings_ui.dirty = false;

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
                        Interaction::default(),
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
                            Interaction::default(),
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

fn inventory_shortcut_trigger_system(
    ctx: Res<ShopContextSnapshot>,
    ui_state: Res<ShopUiState>,
    settings_ui: Res<SettingsUiState>,
    shortcuts: Res<InventoryShortcutsState>,
    items_cfg: Res<ItemsConfig>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
) {
    if !ctx.in_game || ui_state.is_open || settings_ui.is_open || ctx.chat_open {
        return;
    }
    for (slot_idx, binding) in shortcuts.bindings.iter().enumerate() {
        if let Some(input) = binding {
            if input.just_pressed(&keys, &mouse) {
                if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == slot_idx) {
                    if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                        if !def.modes.is_empty() {
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

fn hud_inventory_drag_and_drop_system(
    ctx: Res<ShopContextSnapshot>,
    items_cfg: Res<ItemsConfig>,
    mouse: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    slot_q: Query<(&HudInventorySlotBtn, &RelativeCursorPosition)>,
    mut drag_state: ResMut<InventoryDragState>,
    mut ui_state: ResMut<ShopUiState>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut ghost_q: Query<(&mut Node, &mut Visibility), With<DragGhostRoot>>,
    mut ghost_txt_q: Query<&mut Text, With<DragGhostLabelText>>,
) {
    if !ctx.in_game {
        drag_state.pressed_slot = None;
        drag_state.dragging_slot = None;
        if let Ok((mut gn, _)) = ghost_q.single_mut() {
            if gn.display != Display::None {
                gn.display = Display::None;
            }
        }
        return;
    }

    let cursor_pos = window_q.single().ok().and_then(|w| w.cursor_position());
    let hovered_slot = slot_q.iter().find_map(|(btn, rel)| {
        rel.normalized
            .is_some_and(|p| (-0.5..=0.5).contains(&p.x) && (-0.5..=0.5).contains(&p.y))
            .then_some(btn.inv_index)
    });

    if mouse.just_pressed(MouseButton::Left) {
        if let (Some(slot_idx), Some(cpos)) = (hovered_slot, cursor_pos) {
            if ctx.inventory.iter().any(|it| it.slot == slot_idx) {
                drag_state.pressed_slot = Some(slot_idx);
                drag_state.press_cursor = cpos;
                drag_state.dragging_slot = None;
            }
        }
    }

    if mouse.pressed(MouseButton::Left) {
        if let (Some(from_slot), Some(cpos)) = (drag_state.pressed_slot, cursor_pos) {
            if drag_state.dragging_slot.is_none() && cpos.distance(drag_state.press_cursor) > 6.0 {
                drag_state.dragging_slot = Some(from_slot);
                if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == from_slot) {
                    if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                        if let Ok(mut gt) = ghost_txt_q.single_mut() {
                            **gt = def.short_code.clone();
                        }
                    }
                }
            }
            if drag_state.dragging_slot.is_some() {
                if let Ok((mut gn, mut gv)) = ghost_q.single_mut() {
                    gn.display = Display::Flex;
                    gn.left = px(cpos.x - 23.0);
                    gn.top = px(cpos.y - 23.0);
                    if *gv != Visibility::Visible {
                        *gv = Visibility::Visible;
                    }
                }
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        if let Some(from_slot) = drag_state.dragging_slot.take() {
            if let Some(to_slot) = hovered_slot {
                if to_slot != from_slot {
                    outgoing.0.push(ClientLobbyMessage::SwapInventorySlots {
                        from_slot,
                        to_slot,
                    });
                }
            }
        } else if let Some(clicked_slot) = drag_state.pressed_slot {
            if hovered_slot == Some(clicked_slot) {
                if let Some(inv_item) = ctx.inventory.iter().find(|it| it.slot == clicked_slot) {
                    if let Some(def) = items_cfg.get_item(&inv_item.item_id) {
                        if !def.modes.is_empty() {
                            outgoing.0.push(ClientLobbyMessage::CycleItemMode {
                                inv_index: clicked_slot,
                            });
                        } else {
                            ui_state.selected_item_id = def.id.clone();
                            ui_state.is_open = true;
                            ui_state.dirty = true;
                        }
                    }
                }
            }
        }
        drag_state.pressed_slot = None;
        if let Ok((mut gn, _)) = ghost_q.single_mut() {
            if gn.display != Display::None {
                gn.display = Display::None;
            }
        }
    }
}