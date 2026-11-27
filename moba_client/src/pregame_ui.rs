use crate::backend_client::{
    clear_persisted_session, load_persisted_session, sanitize_ascii_msg, ActiveMatchInfo,
    BackendAsyncEvent, BackendClient, FriendsSnapshot, LobbyDetail, LobbySummary,
    LobbyInviteInfo, MatchHistoryItemSlot, MatchHistoryResponse, PartyInviteInfo, PartyState, PlayerProfile,
    PlayerSearchResult,
};
use bevy::animation::RepeatAnimation;
use bevy::app::AppExit;
use bevy::gltf::Gltf;
use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::input::ButtonState;
use bevy::light::AmbientLight;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureDimension, TextureFormat};
use bevy::ui::{Pressed, RelativeCursorPosition};
use bevy::ui_widgets::Button as UiButton;
use bevy::window::{CursorOptions, PrimaryWindow};
use crate::demo_client::PendingDemoLaunch;
use moba_shared::{parse_recipe_target, GameConfig, HeroDef, ItemsConfig};
use std::collections::{HashMap, HashSet};
use std::f32::consts::TAU;

// =============================================================================
// وضعیت کلی برنامه (PreGame Launcher vs InGame Loop)
// =============================================================================

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppScreenState {
    #[default]
    PreGame,
    InGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreGameTab {
    AuthEmail,
    AuthOtp,
    Hub,
    FindingMatch,
    LobbyBrowser,
    LobbyRoom,
    MatchHistory,
    DemoHero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryFilter {
    All,
    Ranked,
    Party,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Login,
    Register,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedInput {
    None,
    Email,
    Username,
    Otp,
    LobbyTitle,
    LobbyPassword,
    JoinPassword,
    LobbyChat,
    LobbySearch,
    FriendSearch,
    PartyChat,
}

#[derive(Debug, Clone)]
pub struct MenuNotification {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub message: String,
    pub timer: f32,
    pub max_secs: f32,
}

#[derive(Resource, Default)]
pub struct PendingMatchConnection(pub Option<ActiveMatchInfo>);

#[derive(Resource)]
pub struct PreGameSessionState {
    pub access_token: Option<String>,
    pub profile: Option<PlayerProfile>,
    pub active_match: Option<ActiveMatchInfo>,
    pub tab: PreGameTab,
    pub auth_mode: AuthMode,
    pub focused: FocusedInput,
    pub email_input: String,
    pub username_input: String,
    pub otp_input: String,
    pub is_loading: bool,
    pub loading_label: String,
    pub toast_message: String,
    pub toast_is_error: bool,
    pub toast_timer: f32,
    // Interactive Notification Stack
    pub notifications: Vec<MenuNotification>,
    pub next_notif_id: u64,
    // Matchmaking state
    pub queue_players: usize,
    pub queue_required: usize,
    pub server_starting: bool,
    pub selected_mode_idx: usize,
    // Friends & Party Social state
    pub friends: FriendsSnapshot,
    pub friend_search_input: String,
    pub friend_search_results: Vec<PlayerSearchResult>,
    pub friend_searching: bool,
    pub friend_search_executed: bool,
    pub social_syncing: bool,
    pub social_reconnect_timer: f32,
    pub pending_sent_invites: HashMap<String, f32>,
    pub party: Option<PartyState>,
    pub pending_party_invites: Vec<PartyInviteInfo>,
    pub party_chat_open: bool,
    pub party_chat_unread: usize,
    pub party_chat_input: String,
    // Custom Lobby state
    pub lobbies: Vec<LobbySummary>,
    pub current_lobby: Option<LobbyDetail>,
    pub create_lobby_modal_open: bool,
    pub edit_lobby_settings_open: bool,
    pub create_password_visible: bool,
    pub pending_create_lobby: Option<(String, String, usize)>,
    pub pending_join_lobby: Option<(String, String)>,
    pub lobby_invite_modal_open: bool,
    pub pending_lobby_invites: Vec<LobbyInviteInfo>,
    pub create_title: String,
    pub create_password: String,
    pub create_capacity: usize,
    pub join_target_lobby_id: Option<String>,
    pub join_password_input: String,
    pub lobby_chat_input: String,
    pub lobby_search_input: String,
    pub lobby_hide_full_locked: bool,
    pub selected_lobby_id: Option<String>,
    // Match History state
    pub history: Option<MatchHistoryResponse>,
    pub selected_history_idx: usize,
    pub history_filter: HistoryFilter,
    pub confirm_modal: Option<ConfirmAction>,
    pub ui_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmAction {
    Logout,
    QuitApp,
    AbandonMatch,
    RemoveFriend { user_id: String, username: String },
}

impl Default for PreGameSessionState {
    fn default() -> Self {
        Self {
            access_token: None,
            profile: None,
            active_match: None,
            tab: PreGameTab::AuthEmail,
            auth_mode: AuthMode::Login,
            focused: FocusedInput::Email,
            email_input: String::new(),
            username_input: String::new(),
            otp_input: String::new(),
            is_loading: false,
            loading_label: String::new(),
            toast_message: String::new(),
            toast_is_error: false,
            toast_timer: 0.0,
            notifications: Vec::new(),
            next_notif_id: 1,
            queue_players: 0,
            queue_required: 2,
            server_starting: false,
            selected_mode_idx: 0,
            friends: FriendsSnapshot::default(),
            friend_search_input: String::new(),
            friend_search_results: Vec::new(),
            friend_searching: false,
            friend_search_executed: false,
            social_syncing: false,
            social_reconnect_timer: 0.0,
            pending_sent_invites: HashMap::new(),
            party: None,
            pending_party_invites: Vec::new(),
            party_chat_open: false,
            party_chat_unread: 0,
            party_chat_input: String::new(),
            lobbies: Vec::new(),
            current_lobby: None,
            create_lobby_modal_open: false,
            edit_lobby_settings_open: false,
            create_password_visible: false,
            pending_create_lobby: None,
            pending_join_lobby: None,
            lobby_invite_modal_open: false,
            pending_lobby_invites: Vec::new(),
            create_title: String::new(),
            create_password: String::new(),
            create_capacity: 2,
            join_target_lobby_id: None,
            join_password_input: String::new(),
            lobby_chat_input: String::new(),
            lobby_search_input: String::new(),
            lobby_hide_full_locked: false,
            selected_lobby_id: None,
            history: None,
            selected_history_idx: 0,
            history_filter: HistoryFilter::All,
            confirm_modal: None,
            ui_dirty: true,
        }
    }
}

#[derive(Resource, Default)]
struct MenuShowcaseAssets {
    gltf_handle: Option<Handle<Gltf>>,
    idle_graph: Option<Handle<AnimationGraph>>,
    idle_node: Option<AnimationNodeIndex>,
    spawned_squad_count: usize,
    showcase_visible: bool,
}

#[derive(Resource, Default)]
struct MenuHeroPortraitCache {
    handles: HashMap<String, Handle<Image>>,
    mipmapped: HashSet<String>,
}

#[derive(Resource, Default)]
struct MenuItemIconCache {
    handles: HashMap<String, Handle<Image>>,
    mipmapped: HashSet<String>,
}

#[derive(Component)]
struct PreGameEntity;

#[derive(Component)]
struct PreGameHiddenHud(Visibility);

#[derive(Component)]
struct PreGameHiddenWorld(Visibility);

#[derive(Component)]
struct PreGameDeactivatedCamera;

#[derive(Component, Clone, Copy)]
struct PreGameBtnBaseBorder(BorderColor);

#[derive(Component)]
struct PreGameScrollableList;

#[derive(Component)]
struct MenuBackdropPlane;

#[derive(Component)]
struct MenuShowcaseCamera;

#[derive(Component)]
struct MenuSoldierSlot {
    slot_idx: usize,
    anim_bound: bool,
}

#[derive(Component)]
struct JungleFirefly {
    base_x_pct: f32,
    base_y_pct: f32,
    phase: f32,
    speed: f32,
}

#[derive(Component)]
struct PreGameDynamicRoot;

#[derive(Component)]
struct ConceptualOrbitDot {
    phase: f32,
    radius: f32,
}

#[derive(Component)]
struct PulseGlowNode {
    speed: f32,
}

#[derive(Component, Clone)]
#[allow(dead_code)]
enum PreGameBtnAction {
    SwitchAuthMode(AuthMode),
    RequestOtp,
    VerifyOtp,
    BackToEmail,
    SwitchTab(PreGameTab),
    StartFindMatch,
    CancelFindMatch,
    ReconnectActiveMatch,
    RequestAbandonConfirm,
    // Social / Friends / Party actions
    RefreshFriends,
    SearchPlayers,
    ClearFriendSearch,
    SendFriendRequest(String),
    RespondFriendRequest { request_id: i64, accept: bool },
    CancelFriendRequest(String),
    RequestRemoveFriendConfirm { user_id: String, username: String },
    RemoveFriend(String),
    InviteToParty(String),
    CancelPartyInvite(String),
    RespondPartyInvite { invite_id: String, accept: bool },
    TogglePartyReady,
    KickPartyMember(String),
    PromotePartyLeader(String),
    LeaveParty,
    TogglePartyChat,
    SendPartyChat,
    DismissNotification(u64),
    // Custom Lobby actions
    OpenCreateLobbyModal,
    CloseLobbyModal,
    ToggleCreatePasswordVisibility,
    OpenLobbySettingsModal,
    SaveLobbySettings,
    SwapLobbyTeam,
    OpenLobbyInviteModal,
    CloseLobbyInviteModal,
    SendLobbyInvite(String),
    RespondLobbyInvite { lobby_id: String, accept: bool },
    RefreshLobbies,
    ToggleHideFullLobbies,
    SelectLobbyRow(String),
    JoinSelectedLobby,
    SelectLobbyCapacity(usize),
    CreateLobby,
    OpenJoinLobby { lobby_id: String, has_password: bool },
    ConfirmJoinLobby,
    CancelJoinModal,
    SwitchLobbySlot { team_id: i32, slot_index: usize },
    KickLobbyPlayer(String),
    SendLobbyChat,
    StartLobbyMatch,
    LeaveLobby,
    SelectHistoryMatch(usize),
    SetHistoryFilter(HistoryFilter),
    RequestLogoutConfirm,
    RequestQuitConfirm,
    ConfirmModalAccept,
    ConfirmModalCancel,
    FocusField(FocusedInput),
    LaunchDemoHero(String),
}

// =============================================================================
// پلاگین PreGame
// =============================================================================

pub struct PreGamePlugin;

impl Plugin for PreGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreGameSessionState>()
            .init_resource::<PendingMatchConnection>()
            .init_resource::<MenuShowcaseAssets>()
            .init_resource::<MenuHeroPortraitCache>()
            .init_resource::<MenuItemIconCache>()
            .add_systems(OnEnter(AppScreenState::PreGame), on_enter_pregame)
            .add_systems(OnExit(AppScreenState::PreGame), on_exit_pregame)
            .add_systems(
                Update,
                (
                    prepare_hero_portrait_mipmaps_system,
                    prepare_item_icon_mipmaps_system,
                    poll_backend_events_system,
                    handle_pregame_keyboard_system,
                    handle_pregame_buttons_system,
                    handle_pregame_scroll_system,
                    update_menu_backdrop_system,
                    sync_menu_party_soldiers_system,
                    attach_menu_soldier_anim_system,
                    animate_pregame_loaders_system,
                    rebuild_pregame_ui_system,
                )
                    .chain()
                    .run_if(in_state(AppScreenState::PreGame)),
            );
    }
}

fn push_notification(
    state: &mut PreGameSessionState,
    kind: &str,
    title: &str,
    message: &str,
    duration_secs: f32,
) {
    let id = state.next_notif_id;
    state.next_notif_id += 1;
    let clean_title = sanitize_ascii_msg(title, "NOTICE");
    let clean_msg = sanitize_ascii_msg(message, "Action completed.");
    state.notifications.push(MenuNotification {
        id,
        kind: kind.to_string(),
        title: clean_title,
        message: clean_msg,
        timer: duration_secs,
        max_secs: duration_secs.max(1.0),
    });
    if state.notifications.len() > 4 {
        state.notifications.remove(0);
    }
    state.ui_dirty = true;
}

fn is_in_active_squad(state: &PreGameSessionState) -> bool {
    state.party.as_ref().map(|p| p.members.len() >= 2).unwrap_or(false)
}

fn sync_search_results_with_friends(state: &mut PreGameSessionState) {
    for res in &mut state.friend_search_results {
        if state.friends.friends.iter().any(|f| f.user_id == res.user_id) {
            res.relationship = "friend".to_string();
        } else if state
            .friends
            .outgoing_requests
            .iter()
            .any(|r| r.user_id == res.user_id)
        {
            res.relationship = "outgoing_pending".to_string();
        } else if state
            .friends
            .incoming_requests
            .iter()
            .any(|r| r.user_id == res.user_id)
        {
            res.relationship = "incoming_pending".to_string();
        }
    }
}

fn validate_client_username(u: &str) -> Result<(), &'static str> {
    let u = u.trim();
    if u.len() < 3 || u.len() > 24 {
        return Err("Username must be 3-24 characters.");
    }
    let mut chars = u.chars();
    let Some(first) = chars.next() else {
        return Err("Username cannot be empty.");
    };
    if !first.is_ascii_alphabetic() {
        return Err("Username must start with an English letter.");
    }
    if !u
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("Use only letters, digits, _ or -.");
    }
    Ok(())
}

fn on_enter_pregame(
    mut commands: Commands,
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut showcase: ResMut<MenuShowcaseAssets>,
    mut hero_portraits: ResMut<MenuHeroPortraitCache>,
    mut item_icons: ResMut<MenuItemIconCache>,
    mut cursor_q: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut old_cams_q: Query<(Entity, &mut Camera), (With<Camera3d>, Without<PreGameEntity>)>,
    mut hud_roots: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>, Without<PreGameEntity>)>,
    mut world_roots: Query<
        (Entity, &mut Visibility),
        (Without<Node>, Without<ChildOf>, Without<PreGameEntity>, Without<PreGameHiddenHud>),
    >,
) {
    let mm = &config.main_menu;
    backend.configure_network(
        mm.http_timeout_secs,
        mm.ws_ping_interval_secs,
        mm.ws_poll_sleep_ms,
    );
    state.queue_required = mm.default_queue_required_players;
    if !mm.lobby_capacities.contains(&state.create_capacity) {
        state.create_capacity = mm.lobby_capacities[0];
    }

    for mut copt in &mut cursor_q {
        copt.visible = true;
    }
    for (cam_ent, mut cam) in &mut old_cams_q {
        if cam.is_active {
            commands.entity(cam_ent).insert(PreGameDeactivatedCamera);
            cam.is_active = false;
        }
    }
    for (ent, mut vis) in &mut hud_roots {
        commands.entity(ent).insert(PreGameHiddenHud(*vis));
        *vis = Visibility::Hidden;
    }
    for (ent, mut vis) in &mut world_roots {
        commands.entity(ent).insert(PreGameHiddenWorld(*vis));
        *vis = Visibility::Hidden;
    }
    showcase.gltf_handle = Some(asset_server.load(mm.soldier_model_path.clone()));
    showcase.idle_graph = None;
    showcase.idle_node = None;
    showcase.spawned_squad_count = 0;
    showcase.showcase_visible = false;

    let assets_dir = crate::client_assets_dir();
    let aniso = mm.hero_icon_anisotropy_clamp;
    let use_mipmaps = mm.hero_icon_generate_mipmaps;
    for hero in &config.heroes {
        let full_path = assets_dir.join(&hero.portrait_path);
        assert!(
            full_path.is_file(),
            "Hero portrait file missing for '{}' ({}): expected '{}'",
            hero.id,
            hero.display_name,
            full_path.display()
        );
        if !hero_portraits.handles.contains_key(&hero.portrait_path) {
            let handle: Handle<Image> = asset_server
                .load_builder()
                .with_settings(move |s: &mut ImageLoaderSettings| {
                    s.is_srgb = true;
                    s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::ClampToEdge,
                        address_mode_v: ImageAddressMode::ClampToEdge,
                        mag_filter: ImageFilterMode::Linear,
                        min_filter: ImageFilterMode::Linear,
                        mipmap_filter: if use_mipmaps {
                            ImageFilterMode::Linear
                        } else {
                            ImageFilterMode::Nearest
                        },
                        anisotropy_clamp: aniso,
                        ..default()
                    });
                })
                .load(hero.portrait_path.clone());
            hero_portraits
                .handles
                .insert(hero.portrait_path.clone(), handle);
        }
    }

    for item in &items_config.items {
        let icon_path = item.icon_path.trim();
        if icon_path.is_empty() || item_icons.handles.contains_key(icon_path) {
            continue;
        }
        let full_path = assets_dir.join(icon_path);
        if full_path.is_file() {
            let handle: Handle<Image> = asset_server
                .load_builder()
                .with_settings(move |s: &mut ImageLoaderSettings| {
                    s.is_srgb = true;
                    s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: ImageAddressMode::ClampToEdge,
                        address_mode_v: ImageAddressMode::ClampToEdge,
                        mag_filter: ImageFilterMode::Linear,
                        min_filter: ImageFilterMode::Linear,
                        mipmap_filter: if use_mipmaps {
                            ImageFilterMode::Linear
                        } else {
                            ImageFilterMode::Nearest
                        },
                        anisotropy_clamp: aniso,
                        ..default()
                    });
                })
                .load(icon_path.to_string());
            item_icons.handles.insert(icon_path.to_string(), handle);
        }
    }

    let cam_pos = Vec3::from_array(mm.camera_position);
    let cam_look = Vec3::from_array(mm.camera_look_at);
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: 10,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.018, 0.055, 0.062)),
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: mm.camera_fov_deg.to_radians(),
            ..default()
        }),
        AmbientLight {
            color: Color::srgb(0.75, 0.90, 0.86),
            brightness: mm.ambient_light_brightness,
            ..default()
        },
        Transform::from_translation(cam_pos).looking_at(cam_look, Vec3::Y),
        MenuShowcaseCamera,
        PreGameEntity,
    ));

    let key_dir = Vec3::from_array(mm.key_light_direction).normalize_or_zero();
    let key_rot = if key_dir.length_squared() > 0.001 {
        Transform::from_translation(Vec3::ZERO)
            .looking_to(key_dir, Vec3::Y)
            .rotation
    } else {
        Quat::IDENTITY
    };
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: mm.key_light_illuminance,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(key_rot),
        PreGameEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.28, 0.92, 0.76),
            intensity: mm.rim_light_intensity,
            range: 24.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(Vec3::from_array(mm.rim_light_position)),
        PreGameEntity,
    ));

    let bg_tex: Handle<Image> = asset_server.load(mm.background_image_path.clone());
    let bg_mesh = meshes.add(Rectangle::new(1.0, 1.0));
    let bg_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(bg_tex),
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(bg_mesh),
        MeshMaterial3d(bg_mat),
        Transform::from_xyz(0.0, 0.0, -8.0),
        MenuBackdropPlane,
        PreGameEntity,
    ));

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Stretch,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            PreGameEntity,
        ))
        .with_children(|root| {
            for i in 0..10 {
                let fx = 8.0 + ((i * 23) % 84) as f32;
                let fy = 14.0 + ((i * 19) % 68) as f32;
                let col = if i % 2 == 0 {
                    Color::srgba(0.98, 0.84, 0.34, 0.55)
                } else {
                    Color::srgba(0.26, 0.94, 0.72, 0.45)
                };
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(fx),
                        top: Val::Percent(fy),
                        width: Val::Px(4.0),
                        height: Val::Px(4.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(col),
                    JungleFirefly {
                        base_x_pct: fx,
                        base_y_pct: fy,
                        phase: i as f32 * 1.1,
                        speed: 0.55 + (i % 4) as f32 * 0.22,
                    },
                ));
            }
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Stretch,
                    ..default()
                },
                PreGameDynamicRoot,
            ));
        });

    if state.access_token.is_none() {
        if let Some((token, _refresh, email)) = load_persisted_session() {
            state.email_input = email;
            state.is_loading = true;
            state.loading_label = "RESTORING SESSION".to_string();
            backend.restore_session(token);
        }
    } else if let Some(tok) = state.access_token.clone() {
        state.tab = PreGameTab::Hub;
        state.active_match = None;
        state.is_loading = false;
        state.server_starting = false;
        state.queue_players = 0;
        state.confirm_modal = None;
        backend.restore_session(tok.clone());
        backend.connect_social_ws(tok.clone());
        backend.fetch_friends(tok.clone());
        backend.fetch_match_history(tok);
    }
    state.ui_dirty = true;
}

fn on_exit_pregame(
    mut commands: Commands,
    mut backend: ResMut<BackendClient>,
    mut showcase: ResMut<MenuShowcaseAssets>,
    query: Query<Entity, With<PreGameEntity>>,
    mut hidden_huds: Query<(Entity, &mut Visibility, &PreGameHiddenHud), Without<PreGameHiddenWorld>>,
    mut hidden_worlds: Query<(Entity, &mut Visibility, &PreGameHiddenWorld), Without<PreGameHiddenHud>>,
    mut deactivated_cams: Query<(Entity, &mut Camera), With<PreGameDeactivatedCamera>>,
) {
    backend.disconnect_social_ws();
    showcase.spawned_squad_count = 0;
    showcase.idle_graph = None;
    showcase.idle_node = None;
    for ent in &query {
        safe_despawn(&mut commands, ent);
    }
    for (ent, mut vis, saved) in &mut hidden_huds {
        *vis = saved.0;
        commands.entity(ent).remove::<PreGameHiddenHud>();
    }
    for (ent, mut vis, saved) in &mut hidden_worlds {
        *vis = saved.0;
        commands.entity(ent).remove::<PreGameHiddenWorld>();
    }
    for (cam_ent, mut cam) in &mut deactivated_cams {
        cam.is_active = true;
        commands.entity(cam_ent).remove::<PreGameDeactivatedCamera>();
    }
}

fn update_menu_backdrop_system(
    config: Res<GameConfig>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cam_q: Query<&Transform, (With<MenuShowcaseCamera>, Without<MenuBackdropPlane>)>,
    mut bg_q: Query<&mut Transform, With<MenuBackdropPlane>>,
) {
    let Ok(win) = windows.single() else { return };
    let Ok(cam_tf) = cam_q.single() else { return };
    let Ok(mut bg_tf) = bg_q.single_mut() else { return };

    let aspect = (win.width() / win.height().max(1.0)).max(0.5);
    let dist = 12.0_f32;
    let fov_rad = config.main_menu.camera_fov_deg.to_radians();
    let frustum_h = 2.0 * dist * (fov_rad * 0.5).tan() * 1.04;
    let frustum_w = frustum_h * aspect;

    let forward = cam_tf.forward();
    bg_tf.translation = cam_tf.translation + *forward * dist;
    bg_tf.rotation = cam_tf.rotation;
    bg_tf.scale = Vec3::new(frustum_w, frustum_h, 1.0);
}

fn sync_menu_party_soldiers_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    state: Res<PreGameSessionState>,
    mut showcase: ResMut<MenuShowcaseAssets>,
    existing_q: Query<(Entity, &MenuSoldierSlot, &mut Visibility)>,
) {
    let should_show = matches!(state.tab, PreGameTab::Hub | PreGameTab::FindingMatch)
        && state.profile.is_some();

    let party_count = state
        .party
        .as_ref()
        .map(|p| p.members.len())
        .unwrap_or(1)
        .clamp(1, config.main_menu.party_max_size);

    let target_count = if should_show { party_count } else { 0 };

    if showcase.spawned_squad_count == target_count && showcase.showcase_visible == should_show {
        return;
    }

    for (ent, _, _) in &existing_q {
        safe_despawn(&mut commands, ent);
    }

    if target_count > 0 {
        let scene_path = format!("{}#Scene0", config.main_menu.soldier_model_path);
        for idx in 0..target_count {
            let slot_cfg = &config.main_menu.squad_slots[idx];
            let scene_handle = asset_server.load(scene_path.clone());
            commands.spawn((
                WorldAssetRoot(scene_handle),
                Transform {
                    translation: Vec3::from_array(slot_cfg.position),
                    rotation: Quat::from_rotation_y(slot_cfg.yaw_deg.to_radians()),
                    scale: Vec3::splat(slot_cfg.scale),
                },
                Visibility::Visible,
                MenuSoldierSlot {
                    slot_idx: idx,
                    anim_bound: false,
                },
                PreGameEntity,
            ));
        }
    }

    showcase.spawned_squad_count = target_count;
    showcase.showcase_visible = should_show;
}

fn attach_menu_soldier_anim_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    gltf_assets: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut showcase: ResMut<MenuShowcaseAssets>,
    mut slots_q: Query<(Entity, &mut MenuSoldierSlot)>,
    children_q: Query<&Children>,
    mut player_q: Query<&mut AnimationPlayer>,
) {
    if showcase.idle_graph.is_none() {
        let Some(ref handle) = showcase.gltf_handle else { return };
        let Some(gltf) = gltf_assets.get(handle) else { return };
        let anim_name = config.main_menu.soldier_idle_animation.as_str();
        let clip_handle = gltf
            .named_animations
            .get(anim_name)
            .cloned()
            .or_else(|| gltf.animations.first().cloned());
        let Some(clip_handle) = clip_handle else { return };

        let mut graph = AnimationGraph::new();
        let node = graph.add_clip(clip_handle, 1.0, graph.root);
        showcase.idle_graph = Some(graphs.add(graph));
        showcase.idle_node = Some(node);
    }

    let (Some(ref graph_handle), Some(idle_node)) = (&showcase.idle_graph, showcase.idle_node)
    else {
        return;
    };

    for (root_ent, mut slot) in &mut slots_q {
        if slot.anim_bound {
            continue;
        }
        let mut stack = vec![root_ent];
        let mut bound_any = false;
        while let Some(curr) = stack.pop() {
            if let Ok(mut player) = player_q.get_mut(curr) {
                commands
                    .entity(curr)
                    .insert(AnimationGraphHandle(graph_handle.clone()));
                let active = player.play(idle_node);
                active.set_repeat(RepeatAnimation::Forever);
                active.seek_to(slot.slot_idx as f32 * 0.37);
                bound_any = true;
            }
            if let Ok(children) = children_q.get(curr) {
                for ch in children.iter() {
                    stack.push(ch);
                }
            }
        }
        if bound_any {
            slot.anim_bound = true;
        }
    }
}

#[inline]
fn srgb_u8_to_linear(v: u8) -> f32 {
    let s = (v as f32) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn linear_to_srgb_u8(l: f32) -> u8 {
    let clamped = l.clamp(0.0, 1.0);
    let s = if clamped <= 0.0031308 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

fn generate_rgba8_mip_chain(
    base_pixels: &[u8],
    base_w: u32,
    base_h: u32,
    is_srgb: bool,
) -> (Vec<u8>, u32) {
    let mut all_data = Vec::with_capacity(base_pixels.len() * 4 / 3 + 64);
    all_data.extend_from_slice(base_pixels);
    let mut prev = base_pixels.to_vec();
    let mut w = base_w;
    let mut h = base_h;
    let mut mip_count = 1u32;

    while w > 1 || h > 1 {
        let next_w = (w / 2).max(1);
        let next_h = (h / 2).max(1);
        let mut next = vec![0u8; (next_w as usize) * (next_h as usize) * 4];

        for ny in 0..next_h {
            let sy0 = (ny * 2).min(h - 1);
            let sy1 = (ny * 2 + 1).min(h - 1);
            for nx in 0..next_w {
                let sx0 = (nx * 2).min(w - 1);
                let sx1 = (nx * 2 + 1).min(w - 1);
                let coords = [(sx0, sy0), (sx1, sy0), (sx0, sy1), (sx1, sy1)];

                let mut sum_r = 0.0_f32;
                let mut sum_g = 0.0_f32;
                let mut sum_b = 0.0_f32;
                let mut sum_a = 0.0_f32;
                let mut alpha_weight = 0.0_f32;

                for (sx, sy) in coords {
                    let idx = ((sy as usize) * (w as usize) + (sx as usize)) * 4;
                    let a = (prev[idx + 3] as f32) / 255.0;
                    let (r, g, b) = if is_srgb {
                        (
                            srgb_u8_to_linear(prev[idx]),
                            srgb_u8_to_linear(prev[idx + 1]),
                            srgb_u8_to_linear(prev[idx + 2]),
                        )
                    } else {
                        (
                            prev[idx] as f32 / 255.0,
                            prev[idx + 1] as f32 / 255.0,
                            prev[idx + 2] as f32 / 255.0,
                        )
                    };
                    sum_r += r * a;
                    sum_g += g * a;
                    sum_b += b * a;
                    sum_a += a;
                    alpha_weight += a;
                }

                let dst = ((ny as usize) * (next_w as usize) + (nx as usize)) * 4;
                let out_a = sum_a * 0.25;
                if alpha_weight > 1e-5 {
                    let inv_w = 1.0 / alpha_weight;
                    let r = sum_r * inv_w;
                    let g = sum_g * inv_w;
                    let b = sum_b * inv_w;
                    if is_srgb {
                        next[dst] = linear_to_srgb_u8(r);
                        next[dst + 1] = linear_to_srgb_u8(g);
                        next[dst + 2] = linear_to_srgb_u8(b);
                    } else {
                        next[dst] = (r * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                        next[dst + 1] = (g * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                        next[dst + 2] = (b * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
                    }
                }
                next[dst + 3] = (out_a * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
            }
        }

        all_data.extend_from_slice(&next);
        prev = next;
        w = next_w;
        h = next_h;
        mip_count += 1;
    }

    (all_data, mip_count)
}

fn prepare_hero_portrait_mipmaps_system(
    config: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<MenuHeroPortraitCache>,
) {
    if !config.main_menu.hero_icon_generate_mipmaps {
        return;
    }
    if cache.mipmapped.len() >= cache.handles.len() {
        return;
    }

    let pending: Vec<(String, Handle<Image>)> = cache
        .handles
        .iter()
        .filter(|(path, _)| !cache.mipmapped.contains(path.as_str()))
        .map(|(p, h)| (p.clone(), h.clone()))
        .collect();

    let aniso = config.main_menu.hero_icon_anisotropy_clamp;
    for (path, handle) in pending {
        let Some(mut img) = images.get_mut(&handle) else {
            continue;
        };
        let is_srgb = match img.texture_descriptor.format {
            TextureFormat::Rgba8UnormSrgb => true,
            TextureFormat::Rgba8Unorm => false,
            _ => {
                cache.mipmapped.insert(path);
                continue;
            }
        };
        if img.texture_descriptor.dimension != TextureDimension::D2
            || img.texture_descriptor.size.depth_or_array_layers != 1
            || img.texture_descriptor.mip_level_count > 1
        {
            cache.mipmapped.insert(path);
            continue;
        }
        let w = img.texture_descriptor.size.width;
        let h = img.texture_descriptor.size.height;
        let expected_bytes = (w as usize) * (h as usize) * 4;
        if w <= 1 || h <= 1 {
            cache.mipmapped.insert(path);
            continue;
        }
        let Some(base_data) = img.data.as_ref() else {
            cache.mipmapped.insert(path);
            continue;
        };
        if base_data.len() != expected_bytes {
            cache.mipmapped.insert(path);
            continue;
        }

        let (mip_bytes, mip_levels) = generate_rgba8_mip_chain(base_data, w, h, is_srgb);
        img.data = Some(mip_bytes);
        img.texture_descriptor.mip_level_count = mip_levels;
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: aniso,
            lod_min_clamp: 0.0,
            lod_max_clamp: (mip_levels.saturating_sub(1)) as f32,
            ..default()
        });
        cache.mipmapped.insert(path);
    }
}

fn prepare_item_icon_mipmaps_system(
    config: Res<GameConfig>,
    mut images: ResMut<Assets<Image>>,
    mut cache: ResMut<MenuItemIconCache>,
) {
    if !config.main_menu.hero_icon_generate_mipmaps {
        return;
    }
    if cache.mipmapped.len() >= cache.handles.len() {
        return;
    }

    let pending: Vec<(String, Handle<Image>)> = cache
        .handles
        .iter()
        .filter(|(path, _)| !cache.mipmapped.contains(path.as_str()))
        .map(|(p, h)| (p.clone(), h.clone()))
        .collect();

    let aniso = config.main_menu.hero_icon_anisotropy_clamp;
    for (path, handle) in pending {
        let Some(mut img) = images.get_mut(&handle) else {
            continue;
        };
        let is_srgb = match img.texture_descriptor.format {
            TextureFormat::Rgba8UnormSrgb => true,
            TextureFormat::Rgba8Unorm => false,
            _ => {
                cache.mipmapped.insert(path);
                continue;
            }
        };
        if img.texture_descriptor.dimension != TextureDimension::D2
            || img.texture_descriptor.size.depth_or_array_layers != 1
            || img.texture_descriptor.mip_level_count > 1
        {
            cache.mipmapped.insert(path);
            continue;
        }
        let w = img.texture_descriptor.size.width;
        let h = img.texture_descriptor.size.height;
        let expected_bytes = (w as usize) * (h as usize) * 4;
        if w <= 1 || h <= 1 {
            cache.mipmapped.insert(path);
            continue;
        }
        let Some(base_data) = img.data.as_ref() else {
            cache.mipmapped.insert(path);
            continue;
        };
        if base_data.len() != expected_bytes {
            cache.mipmapped.insert(path);
            continue;
        }

        let (mip_bytes, mip_levels) = generate_rgba8_mip_chain(base_data, w, h, is_srgb);
        img.data = Some(mip_bytes);
        img.texture_descriptor.mip_level_count = mip_levels;
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            anisotropy_clamp: aniso,
            lod_min_clamp: 0.0,
            lod_max_clamp: (mip_levels.saturating_sub(1)) as f32,
            ..default()
        });
        cache.mipmapped.insert(path);
    }
}

// =============================================================================
// دریافت رویدادهای غیرمسدودکننده بک‌اند و اتصال مجدد خودکار وب‌سوکت دوستان/پارتی
// =============================================================================

fn poll_backend_events_system(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut pending_conn: ResMut<PendingMatchConnection>,
    mut next_screen: ResMut<NextState<AppScreenState>>,
) {
    let dt = time.delta_secs();
    if state.toast_timer > 0.0 {
        state.toast_timer = (state.toast_timer - dt).max(0.0);
        if state.toast_timer == 0.0 && !state.toast_message.is_empty() {
            state.toast_message.clear();
            state.ui_dirty = true;
        }
    }

    // شمارش معکوس و پاک‌سازی خودکار دعوت‌های ارسالی منقضی‌شده
    if !state.pending_sent_invites.is_empty() {
        let before_inv = state.pending_sent_invites.len();
        state.pending_sent_invites.retain(|_, rem| {
            *rem -= dt;
            *rem > 0.0
        });
        if state.pending_sent_invites.len() != before_inv {
            state.ui_dirty = true;
        }
    }

    // اتصال مجدد خودکار وب‌سوکت Social/Party در صورت قطع شدن
    if let Some(tok) = state.access_token.clone() {
        if !backend.is_social_ws_connected() {
            state.social_reconnect_timer -= dt;
            if state.social_reconnect_timer <= 0.0 {
                state.social_reconnect_timer = config.main_menu.social_reconnect_interval_secs;
                backend.connect_social_ws(tok.clone());
                backend.fetch_friends(tok);
            }
        }
    }

    let before_n = state.notifications.len();
    for n in &mut state.notifications {
        n.timer = (n.timer - dt).max(0.0);
    }
    state.notifications.retain(|n| n.timer > 0.0);
    if state.notifications.len() != before_n {
        state.ui_dirty = true;
    }

    let notif_dur = config.main_menu.notification_duration_secs;
    let toast_dur = config.main_menu.toast_duration_secs;
    let events = backend.poll_events();
    if events.is_empty() {
        return;
    }

    for ev in events {
        match ev {
            BackendAsyncEvent::OtpRequested { email } => {
                state.is_loading = false;
                state.email_input = email;
                state.otp_input.clear();
                state.tab = PreGameTab::AuthOtp;
                state.focused = FocusedInput::Otp;
                push_notification(
                    &mut state,
                    "info",
                    "OTP SENT",
                    "6-digit verification code sent. Check server console.",
                    notif_dur,
                );
            }
            BackendAsyncEvent::OtpVerified {
                access_token,
                refresh_token: _,
                profile,
                active_match,
                is_new_player,
            } => {
                state.is_loading = false;
                state.access_token = Some(access_token.clone());
                state.profile = Some(profile.clone());
                state.active_match = active_match;
                state.tab = PreGameTab::Hub;
                state.focused = FocusedInput::None;
                backend.connect_social_ws(access_token.clone());
                backend.fetch_friends(access_token.clone());
                backend.fetch_match_history(access_token);
                let msg = if is_new_player {
                    format!("Account registered! Welcome, {}!", profile.username)
                } else {
                    format!("Welcome back, {}!", profile.username)
                };
                push_notification(&mut state, "party_joined", "SIGNED IN", &msg, notif_dur);
            }
            BackendAsyncEvent::SessionRestored {
                access_token,
                profile,
                active_match,
            } => {
                state.is_loading = false;
                state.access_token = Some(access_token.clone());
                state.profile = Some(profile);
                state.active_match = active_match;
                if matches!(state.tab, PreGameTab::AuthEmail | PreGameTab::AuthOtp) {
                    state.tab = PreGameTab::Hub;
                }
                state.focused = FocusedInput::None;
                backend.connect_social_ws(access_token.clone());
                backend.fetch_friends(access_token.clone());
                backend.fetch_match_history(access_token);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::SessionInvalid => {
                state.is_loading = false;
                state.access_token = None;
                state.profile = None;
                state.active_match = None;
                state.party = None;
                state.pending_party_invites.clear();
                state.pending_sent_invites.clear();
                backend.disconnect_social_ws();
                clear_persisted_session();
                state.tab = PreGameTab::AuthEmail;
                state.focused = FocusedInput::Email;
                push_notification(
                    &mut state,
                    "error",
                    "SESSION EXPIRED",
                    "Please sign in again.",
                    notif_dur,
                );
            }
            BackendAsyncEvent::ActiveMatchChecked(am)
            | BackendAsyncEvent::MatchDisconnected(am) => {
                state.is_loading = false;
                state.active_match = am;
                state.ui_dirty = true;
            }
            BackendAsyncEvent::MatchReconnected(am) => {
                state.is_loading = false;
                if let Some(info) = am {
                    state.active_match = Some(info.clone());
                    pending_conn.0 = Some(info);
                    next_screen.set(AppScreenState::InGame);
                } else {
                    state.active_match = None;
                    push_notification(
                        &mut state,
                        "error",
                        "MATCH ENDED",
                        "Active match is no longer running.",
                        notif_dur,
                    );
                }
            }
            BackendAsyncEvent::MatchAbandoned {
                mmr,
                rank_tier,
                losses,
                abandoned_count,
            } => {
                state.is_loading = false;
                state.active_match = None;
                if let Some(ref mut p) = state.profile {
                    p.mmr = mmr;
                    p.rank_tier = rank_tier;
                    p.losses = losses;
                    p.abandoned_count = abandoned_count;
                }
                if let Some(tok) = state.access_token.clone() {
                    backend.fetch_match_history(tok);
                }
                push_notification(
                    &mut state,
                    "info",
                    "MATCH ABANDONED",
                    "You left the match. You may queue again.",
                    notif_dur,
                );
            }
            BackendAsyncEvent::HistoryLoaded(hist) => {
                state.is_loading = false;
                if let Some(ref mut p) = state.profile {
                    p.mmr = hist.summary.mmr;
                    p.rank_tier = hist.summary.rank_tier.clone();
                    p.wins = hist.summary.wins;
                    p.losses = hist.summary.losses;
                    p.abandoned_count = hist.summary.abandoned_count;
                }
                state.history = Some(hist);
                state.selected_history_idx = 0;
                state.ui_dirty = true;
            }
            BackendAsyncEvent::HttpError(err) => {
                state.is_loading = false;
                push_notification(&mut state, "error", "REQUEST FAILED", &err, notif_dur);
            }
            // Social / Friends / Party events
            BackendAsyncEvent::SocialSnapshot {
                friends,
                party,
                pending_invites,
            } => {
                state.social_syncing = false;
                state.friends = friends;
                sync_search_results_with_friends(&mut state);
                let normalized_party = party.filter(|p| p.members.len() >= 2);
                if let Some(ref p) = normalized_party {
                    for m in &p.members {
                        state.pending_sent_invites.remove(&m.user_id);
                    }
                } else {
                    state.party_chat_open = false;
                    state.party_chat_unread = 0;
                }
                state.party = normalized_party;
                state.pending_party_invites = pending_invites;
                state.ui_dirty = true;
            }
            BackendAsyncEvent::FriendsLoaded(snap) => {
                state.social_syncing = false;
                state.friends = snap;
                sync_search_results_with_friends(&mut state);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::FriendSearchResults(res) => {
                state.friend_searching = false;
                state.friend_search_executed = true;
                state.friend_search_results = res;
                sync_search_results_with_friends(&mut state);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::FriendActionDone { message, snapshot } => {
                state.is_loading = false;
                state.friends = snapshot;
                sync_search_results_with_friends(&mut state);
                push_notification(&mut state, "friend_accepted", "FRIENDS", &message, notif_dur);
            }
            BackendAsyncEvent::FriendActionError(err) => {
                state.is_loading = false;
                state.friend_searching = false;
                state.social_syncing = false;
                push_notification(&mut state, "error", "FRIENDS ERROR", &err, notif_dur);
            }
            BackendAsyncEvent::FriendPresenceUpdated {
                user_id,
                username,
                online,
                status,
                in_party,
            } => {
                if !online {
                    state.pending_sent_invites.remove(&user_id);
                }
                let mut changed = false;
                for f in &mut state.friends.friends {
                    if f.user_id == user_id {
                        let was_online = f.online;
                        f.online = online;
                        f.status = status.clone();
                        f.in_party = in_party;
                        changed = true;
                        if !was_online && online {
                            push_notification(
                                &mut state,
                                "info",
                                "FRIEND ONLINE",
                                &format!("{} is now online.", username),
                                notif_dur * 0.75,
                            );
                        }
                        break;
                    }
                }
                if changed {
                    state.friends.friends.sort_by(|a, b| {
                        b.online
                            .cmp(&a.online)
                            .then_with(|| b.mmr.cmp(&a.mmr))
                            .then_with(|| a.username.cmp(&b.username))
                    });
                    state.ui_dirty = true;
                } else if let Some(tok) = state.access_token.clone() {
                    backend.fetch_friends(tok);
                }
            }
            BackendAsyncEvent::PartyStateUpdated(ps) => {
                if ps.members.len() < 2 {
                    state.party = None;
                    state.party_chat_open = false;
                    state.party_chat_unread = 0;
                    state.ui_dirty = true;
                } else {
                    let prev_chat_len = state
                        .party
                        .as_ref()
                        .map(|old| old.chat_messages.len())
                        .unwrap_or(0);
                    let new_chat_len = ps.chat_messages.len();
                    if new_chat_len > prev_chat_len && !state.party_chat_open {
                        let non_sys_delta = ps.chat_messages[prev_chat_len..]
                            .iter()
                            .filter(|m| !m.is_system)
                            .count();
                        state.party_chat_unread = state.party_chat_unread.saturating_add(non_sys_delta);
                    }
                    for m in &ps.members {
                        state.pending_sent_invites.remove(&m.user_id);
                    }
                    if ps.is_queuing && state.tab == PreGameTab::Hub {
                        state.tab = PreGameTab::FindingMatch;
                    }
                    state.party = Some(ps);
                    state.ui_dirty = true;
                }
            }
            BackendAsyncEvent::PartyCleared { reason } => {
                state.party = None;
                state.party_chat_open = false;
                state.party_chat_unread = 0;
                state.pending_sent_invites.clear();
                if state.tab == PreGameTab::FindingMatch {
                    state.tab = PreGameTab::Hub;
                }
                push_notification(&mut state, "party_left", "PARTY", &reason, notif_dur);
            }
            BackendAsyncEvent::PartyInviteReceived {
                invite,
                title,
                message,
            } => {
                state
                    .pending_party_invites
                    .retain(|i| i.invite_id != invite.invite_id);
                state.pending_party_invites.push(invite);
                push_notification(&mut state, "party_joined", &title, &message, notif_dur);
            }
            BackendAsyncEvent::PartyInviteCleared(iid) => {
                state.pending_party_invites.retain(|i| i.invite_id != iid);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::PartyInviteResult {
                target_user_id,
                accepted: _,
                message,
            } => {
                state.pending_sent_invites.remove(&target_user_id);
                push_notification(&mut state, "info", "INVITE RESPONSE", &message, notif_dur);
            }
            BackendAsyncEvent::SocialNotification {
                kind,
                title,
                message,
            } => {
                if kind == "error" {
                    state.pending_sent_invites.clear();
                }
                push_notification(&mut state, &kind, &title, &message, notif_dur);
            }
            BackendAsyncEvent::PartyQueueCanceled(msg) => {
                state.server_starting = false;
                state.queue_players = 0;
                if state.tab == PreGameTab::FindingMatch {
                    state.tab = PreGameTab::Hub;
                }
                push_notification(&mut state, "info", "QUEUE CANCELED", &msg, notif_dur);
            }
            BackendAsyncEvent::QueueStatus {
                players_in_queue,
                required_players,
            } => {
                state.tab = PreGameTab::FindingMatch;
                state.queue_players = players_in_queue;
                state.queue_required = required_players;
                state.server_starting = false;
                state.ui_dirty = true;
            }
            BackendAsyncEvent::StartingGameServer {
                room_id: _,
                port: _,
                players_in_queue,
                required_players,
            } => {
                state.tab = PreGameTab::FindingMatch;
                state.queue_players = players_in_queue;
                state.queue_required = required_players;
                state.server_starting = true;
                push_notification(
                    &mut state,
                    "party_joined",
                    "MATCH READY",
                    "Launching dedicated tactical server...",
                    notif_dur,
                );
            }
            BackendAsyncEvent::MatchFound(info) => {
                state.is_loading = false;
                state.server_starting = false;
                state.active_match = Some(info.clone());
                pending_conn.0 = Some(info);
                backend.stop_matchmaking();
                backend.disconnect_lobby_ws();
                next_screen.set(AppScreenState::InGame);
            }
            BackendAsyncEvent::MatchmakingError {
                message,
                active_match,
            } => {
                state.is_loading = false;
                state.server_starting = false;
                if active_match.is_some() {
                    state.active_match = active_match;
                }
                state.tab = PreGameTab::Hub;
                push_notification(&mut state, "error", "MATCHMAKING ERROR", &message, notif_dur);
            }
            BackendAsyncEvent::MatchmakingClosed(conn_id) => {
                if conn_id != backend.active_mm_conn_id() {
                    continue;
                }
                if state.tab == PreGameTab::FindingMatch && !state.server_starting {
                    state.tab = PreGameTab::Hub;
                    state.ui_dirty = true;
                }
            }
            BackendAsyncEvent::LobbyInviteReceived(inv) => {
                state
                    .pending_lobby_invites
                    .retain(|x| x.lobby_id != inv.lobby_id);
                let title = format!("LOBBY INVITE // {}", inv.from_username.to_uppercase());
                let msg = format!("{} invited you to {}", inv.from_username, inv.lobby_title);
                state.pending_lobby_invites.push(inv);
                push_notification(&mut state, "party_joined", &title, &msg, notif_dur);
            }
            BackendAsyncEvent::LobbyList { lobbies, message } => {
                state.is_loading = false;
                state.lobbies = lobbies;
                if let Some((title, pw, cap)) = state.pending_create_lobby.take() {
                    state.is_loading = true;
                    state.loading_label = "CREATING CUSTOM LOBBY".to_string();
                    backend.send_lobby_action(serde_json::json!({
                        "action": "create_lobby",
                        "title": title,
                        "password": pw,
                        "max_players": cap,
                    }));
                } else if let Some((lid, pw)) = state.pending_join_lobby.take() {
                    state.is_loading = true;
                    state.loading_label = "JOINING LOBBY".to_string();
                    backend.send_lobby_action(serde_json::json!({
                        "action": "join_lobby",
                        "lobby_id": lid,
                        "password": pw,
                    }));
                } else if !message.is_empty() && state.tab == PreGameTab::LobbyBrowser {
                    set_toast(&mut state, &message, false, toast_dur);
                }
                state.ui_dirty = true;
            }
            BackendAsyncEvent::LobbyState { lobby, message } => {
                state.is_loading = false;
                state.create_lobby_modal_open = false;
                state.edit_lobby_settings_open = false;
                state.lobby_invite_modal_open = false;
                state.current_lobby = Some(lobby);
                state.join_target_lobby_id = None;
                state.join_password_input.clear();
                state.tab = PreGameTab::LobbyRoom;
                if !message.is_empty() {
                    set_toast(&mut state, &message, false, toast_dur);
                }
                state.ui_dirty = true;
            }
            BackendAsyncEvent::LobbyStarting { lobby, message } => {
                state.current_lobby = Some(lobby);
                state.tab = PreGameTab::LobbyRoom;
                push_notification(&mut state, "party_joined", "LOBBY STARTING", &message, notif_dur);
            }
            BackendAsyncEvent::KickedFromLobby { lobbies, message } => {
                state.current_lobby = None;
                state.lobbies = lobbies;
                state.tab = PreGameTab::LobbyBrowser;
                push_notification(&mut state, "error", "REMOVED FROM LOBBY", &message, notif_dur);
            }
            BackendAsyncEvent::LobbyClosed { message } => {
                state.current_lobby = None;
                state.tab = PreGameTab::Hub;
                backend.disconnect_lobby_ws();
                push_notification(&mut state, "error", "LOBBY CLOSED", &message, notif_dur);
            }
            BackendAsyncEvent::LeftLobby { lobbies, message } => {
                state.current_lobby = None;
                state.lobbies = lobbies;
                state.tab = PreGameTab::LobbyBrowser;
                set_toast(&mut state, &message, false, toast_dur);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::LobbyError(err) => {
                state.is_loading = false;
                state.server_starting = false;
                state.pending_create_lobby = None;
                state.pending_join_lobby = None;
                push_notification(&mut state, "error", "LOBBY ERROR", &err, notif_dur);
            }
            BackendAsyncEvent::LobbyWsClosed(conn_id) => {
                if conn_id != backend.active_lobby_conn_id() {
                    continue;
                }
                state.is_loading = false;
                state.server_starting = false;
                state.pending_create_lobby = None;
                state.pending_join_lobby = None;
                if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom) {
                    state.current_lobby = None;
                    state.tab = PreGameTab::Hub;
                    push_notification(
                        &mut state,
                        "error",
                        "LOBBY DISCONNECTED",
                        "Connection to lobby server closed.",
                        notif_dur,
                    );
                } else {
                    state.ui_dirty = true;
                }
            }
            BackendAsyncEvent::SocialWsClosed(conn_id) => {
                if conn_id != backend.active_social_conn_id() {
                    continue;
                }
                backend.disconnect_social_ws();
                state.social_syncing = false;
                state.social_reconnect_timer = config.main_menu.social_reconnect_interval_secs;
                state.ui_dirty = true;
            }
        }
    }
}

fn set_toast(state: &mut PreGameSessionState, msg: &str, is_error: bool, duration_secs: f32) {
    state.toast_message = sanitize_ascii_msg(
        msg,
        if is_error { "Operation failed." } else { "Updated." },
    );
    state.toast_is_error = is_error;
    state.toast_timer = duration_secs;
    state.ui_dirty = true;
}

// =============================================================================
// ورودی صفحه‌کلید
// =============================================================================

fn handle_pregame_keyboard_system(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<GameConfig>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
) {
    if state.is_loading {
        key_events.clear();
        return;
    }

    if state.confirm_modal.is_some() {
        if keys.just_pressed(KeyCode::Escape) {
            state.confirm_modal = None;
            state.ui_dirty = true;
        }
        key_events.clear();
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        if state.create_lobby_modal_open || state.edit_lobby_settings_open {
            state.create_lobby_modal_open = false;
            state.edit_lobby_settings_open = false;
            state.focused = FocusedInput::None;
            state.ui_dirty = true;
            return;
        }
        if state.lobby_invite_modal_open {
            state.lobby_invite_modal_open = false;
            state.ui_dirty = true;
            return;
        }
    }
    if keys.just_pressed(KeyCode::Escape) && state.join_target_lobby_id.is_some() {
        state.join_target_lobby_id = None;
        state.focused = FocusedInput::None;
        state.ui_dirty = true;
        return;
    }

    if state.focused == FocusedInput::None {
        return;
    }

    let mut submit_pressed = false;
    for ev in key_events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => {
                let target = active_input_mut(&mut state);
                target.pop();
                state.ui_dirty = true;
            }
            Key::Tab => {
                let next_focus = match state.focused {
                    FocusedInput::Username if state.auth_mode == AuthMode::Register => Some(FocusedInput::Email),
                    FocusedInput::Email if state.auth_mode == AuthMode::Register => Some(FocusedInput::Username),
                    FocusedInput::LobbyTitle => Some(FocusedInput::LobbyPassword),
                    FocusedInput::LobbyPassword => Some(FocusedInput::LobbyTitle),
                    _ => None,
                };
                if let Some(nf) = next_focus {
                    state.focused = nf;
                    state.ui_dirty = true;
                }
            }
            Key::Enter => {
                submit_pressed = true;
            }
            Key::Space => {
                if matches!(
                    state.focused,
                    FocusedInput::LobbyTitle | FocusedInput::LobbyChat | FocusedInput::PartyChat
                ) {
                    let target = active_input_mut(&mut state);
                    if target.len() < 96 {
                        target.push(' ');
                        state.ui_dirty = true;
                    }
                }
            }
            Key::Character(s) => {
                let max_len = match state.focused {
                    FocusedInput::Otp => 6,
                    FocusedInput::Email => 72,
                    FocusedInput::Username => 24,
                    FocusedInput::LobbyTitle => 32,
                    FocusedInput::LobbyPassword | FocusedInput::JoinPassword => 24,
                    FocusedInput::LobbyChat | FocusedInput::PartyChat => 110,
                    FocusedInput::FriendSearch | FocusedInput::LobbySearch => 32,
                    FocusedInput::None => 0,
                };
                let only_digits = state.focused == FocusedInput::Otp;
                let mut changed = false;
                {
                    let target = active_input_mut(&mut state);
                    for ch in s.chars() {
                        if target.len() >= max_len {
                            break;
                        }
                        if only_digits {
                            if ch.is_ascii_digit() {
                                target.push(ch);
                                changed = true;
                            }
                        } else if ch.is_ascii_graphic() {
                            target.push(ch);
                            changed = true;
                        }
                    }
                }
                if changed {
                    state.ui_dirty = true;
                }
            }
            _ => {}
        }
    }

    if submit_pressed {
        let notif_dur = config.main_menu.notification_duration_secs;
        match state.focused {
            FocusedInput::Email | FocusedInput::Username => {
                let email = state.email_input.trim().to_string();
                let username = state.username_input.trim().to_string();
                if state.auth_mode == AuthMode::Register {
                    if let Err(err) = validate_client_username(&username) {
                        push_notification(&mut state, "error", "INVALID USERNAME", err, notif_dur);
                        return;
                    }
                }
                if email.contains('@') && email.len() >= 5 {
                    let mode_str = if state.auth_mode == AuthMode::Register {
                        "register"
                    } else {
                        "login"
                    };
                    state.is_loading = true;
                    state.loading_label = "SENDING OTP CODE".to_string();
                    state.ui_dirty = true;
                    backend.request_otp(email, mode_str.to_string(), username);
                }
            }
            FocusedInput::Otp => {
                let email = state.email_input.trim().to_string();
                let otp = state.otp_input.trim().to_string();
                if otp.len() == 6 {
                    state.is_loading = true;
                    state.loading_label = "VERIFYING CODE".to_string();
                    state.ui_dirty = true;
                    backend.verify_otp(email, otp);
                }
            }
            FocusedInput::LobbyChat => {
                let txt = state.lobby_chat_input.trim().to_string();
                if !txt.is_empty() {
                    backend.send_lobby_action(serde_json::json!({
                        "action": "chat_message",
                        "text": txt
                    }));
                    state.lobby_chat_input.clear();
                    state.ui_dirty = true;
                }
            }
            FocusedInput::PartyChat => {
                let txt = state.party_chat_input.trim().to_string();
                if !txt.is_empty() {
                    backend.send_social_action(serde_json::json!({
                        "action": "party_chat",
                        "text": txt
                    }));
                    state.party_chat_input.clear();
                    state.ui_dirty = true;
                }
            }
            FocusedInput::FriendSearch => {
                let q = state.friend_search_input.trim().to_string();
                if q.len() >= 2 {
                    if let Some(tok) = state.access_token.clone() {
                        state.friend_searching = true;
                        state.ui_dirty = true;
                        backend.search_players(tok, q);
                    }
                }
            }
            FocusedInput::JoinPassword => {
                if let Some(lid) = state.join_target_lobby_id.clone() {
                    let pw = state.join_password_input.trim().to_string();
                    state.is_loading = true;
                    state.loading_label = "JOINING CUSTOM LOBBY".to_string();
                    if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom) {
                        backend.send_lobby_action(serde_json::json!({
                            "action": "join_lobby",
                            "lobby_id": lid,
                            "password": pw
                        }));
                    } else if let Some(tok) = state.access_token.clone() {
                        state.pending_join_lobby = Some((lid, pw));
                        backend.connect_lobby_ws(tok);
                    }
                    state.ui_dirty = true;
                }
            }
            _ => {}
        }
    }
}

fn active_input_mut(state: &mut PreGameSessionState) -> &mut String {
    match state.focused {
        FocusedInput::Email => &mut state.email_input,
        FocusedInput::Username => &mut state.username_input,
        FocusedInput::Otp => &mut state.otp_input,
        FocusedInput::LobbyTitle => &mut state.create_title,
        FocusedInput::LobbyPassword => &mut state.create_password,
        FocusedInput::JoinPassword => &mut state.join_password_input,
        FocusedInput::LobbyChat => &mut state.lobby_chat_input,
        FocusedInput::FriendSearch => &mut state.friend_search_input,
        FocusedInput::LobbySearch => &mut state.lobby_search_input,
        FocusedInput::PartyChat => &mut state.party_chat_input,
        FocusedInput::None => &mut state.email_input,
    }
}

// =============================================================================
// پردازش کلیک دکمه‌های منو
// =============================================================================

fn is_action_allowed_for_active_modal(state: &PreGameSessionState, action: &PreGameBtnAction) -> bool {
    if state.is_loading {
        return false;
    }
    if state.confirm_modal.is_some() {
        return matches!(
            action,
            PreGameBtnAction::ConfirmModalAccept | PreGameBtnAction::ConfirmModalCancel
        );
    }
    if state.create_lobby_modal_open || state.edit_lobby_settings_open {
        return matches!(
            action,
            PreGameBtnAction::FocusField(FocusedInput::LobbyTitle | FocusedInput::LobbyPassword)
                | PreGameBtnAction::SelectLobbyCapacity(_)
                | PreGameBtnAction::ToggleCreatePasswordVisibility
                | PreGameBtnAction::CreateLobby
                | PreGameBtnAction::SaveLobbySettings
                | PreGameBtnAction::CloseLobbyModal
        );
    }
    if state.join_target_lobby_id.is_some() {
        return matches!(
            action,
            PreGameBtnAction::FocusField(FocusedInput::JoinPassword)
                | PreGameBtnAction::ConfirmJoinLobby
                | PreGameBtnAction::CancelJoinModal
        );
    }
    if state.lobby_invite_modal_open {
        return matches!(
            action,
            PreGameBtnAction::SendLobbyInvite(_) | PreGameBtnAction::CloseLobbyInviteModal
        );
    }
    true
}

fn handle_pregame_scroll_system(
    mut wheel_events: MessageReader<MouseWheel>,
    state: Res<PreGameSessionState>,
    mut scroll_q: Query<
        (&mut ScrollPosition, &ComputedNode, &RelativeCursorPosition),
        With<PreGameScrollableList>,
    >,
) {
    if state.is_loading || state.confirm_modal.is_some() {
        wheel_events.clear();
        return;
    }
    let mut delta_y = 0.0_f32;
    for ev in wheel_events.read() {
        let step = match ev.unit {
            MouseScrollUnit::Line => ev.y * 32.0,
            MouseScrollUnit::Pixel => ev.y,
        };
        delta_y += step;
    }
    if delta_y.abs() < 0.01 {
        return;
    }
    for (mut scroll, computed, rel_cursor) in &mut scroll_q {
        if let Some(pos) = rel_cursor.normalized {
            if pos.x >= 0.0 && pos.x <= 1.0 && pos.y >= 0.0 && pos.y <= 1.0 {
                let max_y = (computed.content_size().y - computed.size().y).max(0.0);
                scroll.y = (scroll.y - delta_y).clamp(0.0, max_y);
            }
        }
    }
}

fn handle_pregame_buttons_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut pending_demo: ResMut<PendingDemoLaunch>,
    updater: Option<Res<crate::updater_client::ClientUpdaterState>>,
    mut app_exit: MessageWriter<AppExit>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut btn_q: Query<
        (
            Entity,
            &Hovered,
            Has<Pressed>,
            &PreGameBtnAction,
            &mut BorderColor,
            Option<&PreGameBtnBaseBorder>,
        ),
        With<UiButton>,
    >,
) {
    if updater.as_ref().is_some_and(|u| u.is_pointer_over_overlay) {
        return;
    }
    let mut triggered: Option<PreGameBtnAction> = None;
    let clicked_this_frame = mouse.just_pressed(MouseButton::Left);
    for (ent, hovered, pressed, action, mut border, base_border) in &mut btn_q {
        let saved_base = if let Some(b) = base_border {
            b.0
        } else {
            let current = *border;
            commands.entity(ent).insert(PreGameBtnBaseBorder(current));
            current
        };
        let allowed = is_action_allowed_for_active_modal(&state, action);
        if allowed && pressed {
            *border = BorderColor::all(Color::srgb(0.98, 0.84, 0.36));
        } else if allowed && hovered.get() {
            *border = BorderColor::all(Color::srgb(0.32, 0.96, 0.84));
        } else {
            *border = saved_base;
        }
        if clicked_this_frame && allowed && hovered.get() && triggered.is_none() {
            triggered = Some(action.clone());
        }
    }

    let Some(action) = triggered else { return };
    let notif_dur = config.main_menu.notification_duration_secs;

    match action {
        PreGameBtnAction::FocusField(f) => {
            state.focused = f;
            state.ui_dirty = true;
        }
        PreGameBtnAction::LaunchDemoHero(hero_id) => {
            if state.active_match.is_some() {
                push_notification(
                    &mut state,
                    "error",
                    "ACTIVE MATCH",
                    "Reconnect or abandon your active match before entering Demo Hero.",
                    notif_dur,
                );
                return;
            }
            backend.stop_matchmaking();
            backend.disconnect_lobby_ws();
            pending_demo.0 = Some(hero_id);
        }
        PreGameBtnAction::SwitchAuthMode(mode) => {
            state.auth_mode = mode;
            state.focused = if mode == AuthMode::Register {
                FocusedInput::Username
            } else {
                FocusedInput::Email
            };
            state.ui_dirty = true;
        }
        PreGameBtnAction::RequestOtp => {
            let email = state.email_input.trim().to_string();
            let username = state.username_input.trim().to_string();
            if state.auth_mode == AuthMode::Register {
                if let Err(err) = validate_client_username(&username) {
                    push_notification(&mut state, "error", "INVALID USERNAME", err, notif_dur);
                    return;
                }
            }
            if !email.contains('@') || email.len() < 5 {
                push_notification(
                    &mut state,
                    "error",
                    "INVALID EMAIL",
                    "Please enter a valid email address.",
                    notif_dur,
                );
            } else {
                let mode_str = if state.auth_mode == AuthMode::Register {
                    "register"
                } else {
                    "login"
                };
                state.is_loading = true;
                state.loading_label = "SENDING OTP CODE".to_string();
                state.ui_dirty = true;
                backend.request_otp(email, mode_str.to_string(), username);
            }
        }
        PreGameBtnAction::VerifyOtp => {
            let email = state.email_input.trim().to_string();
            let otp = state.otp_input.trim().to_string();
            if otp.len() != 6 {
                push_notification(
                    &mut state,
                    "error",
                    "INVALID OTP",
                    "Enter the 6-digit verification code.",
                    notif_dur,
                );
            } else {
                state.is_loading = true;
                state.loading_label = "VERIFYING CODE".to_string();
                state.ui_dirty = true;
                backend.verify_otp(email, otp);
            }
        }
        PreGameBtnAction::BackToEmail => {
            state.tab = PreGameTab::AuthEmail;
            state.focused = if state.auth_mode == AuthMode::Register {
                FocusedInput::Username
            } else {
                FocusedInput::Email
            };
            state.ui_dirty = true;
        }
        PreGameBtnAction::SwitchTab(target_tab) => {
            let Some(tok) = state.access_token.clone() else { return };
            if state.tab == PreGameTab::FindingMatch {
                if state.party.is_some() {
                    backend.send_social_action(serde_json::json!({
                        "action": "cancel_party_matchmaking"
                    }));
                } else {
                    backend.stop_matchmaking();
                }
            }
            if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom)
                && !matches!(target_tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom)
            {
                backend.disconnect_lobby_ws();
                state.current_lobby = None;
            }
            match target_tab {
                PreGameTab::LobbyBrowser => {
                    if state.active_match.is_some() {
                        push_notification(
                            &mut state,
                            "error",
                            "ACTIVE MATCH",
                            "Finish or abandon your active match before entering lobbies.",
                            notif_dur,
                        );
                        return;
                    }
                    if is_in_active_squad(&state) {
                        push_notification(
                            &mut state,
                            "error",
                            "LEAVE PARTY FIRST",
                            "Leave your active squad before entering Custom Lobbies.",
                            notif_dur,
                        );
                        return;
                    }
                    if state.party.is_some() {
                        state.party = None;
                        state.party_chat_open = false;
                        state.party_chat_unread = 0;
                        backend.send_social_action(serde_json::json!({ "action": "leave_party" }));
                    }
                    state.tab = PreGameTab::LobbyBrowser;
                    state.is_loading = true;
                    state.loading_label = "CONNECTING TO LOBBY SERVER".to_string();
                    backend.connect_lobby_ws(tok);
                }
                PreGameTab::MatchHistory => {
                    state.tab = PreGameTab::MatchHistory;
                    state.is_loading = true;
                    state.loading_label = "LOADING MATCH HISTORY".to_string();
                    backend.fetch_match_history(tok);
                }
                _ => {
                    state.tab = target_tab;
                    backend.fetch_active_match(tok.clone());
                    backend.fetch_friends(tok.clone());
                    if state.history.is_none() {
                        backend.fetch_match_history(tok);
                    }
                }
            }
            state.ui_dirty = true;
        }
        PreGameBtnAction::StartFindMatch => {
            if let Some(ref upd) = updater {
                if upd.should_block_new_matchmaking() {
                    let msg = if upd.is_mandatory_update_pending() {
                        "A mandatory client patch is available. Please update before matchmaking."
                    } else if !upd.banner_message.is_empty() {
                        upd.banner_message.as_str()
                    } else {
                        "Game Coordinator is currently updating. Matchmaking is paused."
                    };
                    push_notification(&mut state, "error", "GAME COORDINATOR", msg, notif_dur);
                    return;
                }
            }
            let Some(tok) = state.access_token.clone() else { return };
            if state.active_match.is_some() {
                push_notification(
                    &mut state,
                    "error",
                    "ACTIVE MATCH",
                    "Reconnect or abandon your active match first.",
                    notif_dur,
                );
                return;
            }
            if is_in_active_squad(&state) {
                let is_leader = state
                    .party
                    .as_ref()
                    .map(|p| {
                        let my_uid = state
                            .profile
                            .as_ref()
                            .map(|pr| pr.user_id.as_str())
                            .unwrap_or("");
                        p.leader_user_id == my_uid
                    })
                    .unwrap_or(false);
                if !is_leader {
                    backend.send_social_action(serde_json::json!({ "action": "toggle_ready" }));
                    return;
                }
                state.server_starting = false;
                backend.send_social_action(serde_json::json!({
                    "action": "start_party_matchmaking"
                }));
            } else {
                if state.party.is_some() {
                    state.party = None;
                    state.party_chat_open = false;
                    state.party_chat_unread = 0;
                    backend.send_social_action(serde_json::json!({ "action": "leave_party" }));
                }
                state.tab = PreGameTab::FindingMatch;
                state.queue_players = 1;
                state.queue_required = config.main_menu.default_queue_required_players;
                state.server_starting = false;
                state.ui_dirty = true;
                backend.start_matchmaking(tok);
            }
        }
        PreGameBtnAction::CancelFindMatch => {
            if is_in_active_squad(&state) {
                backend.send_social_action(serde_json::json!({
                    "action": "cancel_party_matchmaking"
                }));
            } else {
                backend.stop_matchmaking();
                state.tab = PreGameTab::Hub;
                state.server_starting = false;
                state.queue_players = 0;
                state.ui_dirty = true;
            }
        }
        PreGameBtnAction::ReconnectActiveMatch => {
            let Some(tok) = state.access_token.clone() else { return };
            state.is_loading = true;
            state.loading_label = "RECONNECTING TO BATTLE".to_string();
            state.ui_dirty = true;
            backend.reconnect_match(tok);
        }
        PreGameBtnAction::RequestAbandonConfirm => {
            state.confirm_modal = Some(ConfirmAction::AbandonMatch);
            state.ui_dirty = true;
        }
        // Friends & Party Actions
        PreGameBtnAction::RefreshFriends => {
            if let Some(tok) = state.access_token.clone() {
                state.social_syncing = true;
                state.ui_dirty = true;
                backend.fetch_friends(tok.clone());
                if !backend.is_social_ws_connected() {
                    backend.connect_social_ws(tok);
                } else {
                    backend.send_social_action(serde_json::json!({ "action": "refresh_social" }));
                }
            }
        }
        PreGameBtnAction::SearchPlayers => {
            let q = state.friend_search_input.trim().to_string();
            if q.len() < 2 {
                push_notification(
                    &mut state,
                    "error",
                    "SEARCH TOO SHORT",
                    "Type at least 2 characters to search players.",
                    notif_dur,
                );
            } else if let Some(tok) = state.access_token.clone() {
                state.friend_searching = true;
                state.ui_dirty = true;
                backend.search_players(tok, q);
            }
        }
        PreGameBtnAction::SendFriendRequest(uid) => {
            if let Some(tok) = state.access_token.clone() {
                // بلافاصله دکمه + ADD را در UI به PENDING تغییر بده تا دوباره کلیک نشود
                for res in &mut state.friend_search_results {
                    if res.user_id == uid {
                        res.relationship = "outgoing_pending".to_string();
                    }
                }
                state.is_loading = true;
                state.loading_label = "SENDING FRIEND REQUEST".to_string();
                state.ui_dirty = true;
                backend.send_friend_request(tok, uid);
            }
        }
        PreGameBtnAction::RespondFriendRequest { request_id, accept } => {
            if let Some(tok) = state.access_token.clone() {
                state.is_loading = true;
                state.loading_label = if accept {
                    "ACCEPTING FRIEND".to_string()
                } else {
                    "DECLINING REQUEST".to_string()
                };
                state.ui_dirty = true;
                backend.respond_friend_request(tok, request_id, accept);
            }
        }
        PreGameBtnAction::ClearFriendSearch => {
            state.friend_search_input.clear();
            state.friend_search_results.clear();
            state.friend_search_executed = false;
            if state.focused == FocusedInput::FriendSearch {
                state.focused = FocusedInput::None;
            }
            state.ui_dirty = true;
        }
        PreGameBtnAction::CancelFriendRequest(uid) => {
            if let Some(tok) = state.access_token.clone() {
                for res in &mut state.friend_search_results {
                    if res.user_id == uid {
                        res.relationship = "none".to_string();
                    }
                }
                state.is_loading = true;
                state.loading_label = "CANCELING FRIEND REQUEST".to_string();
                state.ui_dirty = true;
                backend.remove_friend(tok, uid);
            }
        }
        PreGameBtnAction::RequestRemoveFriendConfirm { user_id, username } => {
            state.confirm_modal = Some(ConfirmAction::RemoveFriend { user_id, username });
            state.ui_dirty = true;
        }
        PreGameBtnAction::RemoveFriend(uid) => {
            if let Some(tok) = state.access_token.clone() {
                state.is_loading = true;
                state.loading_label = "UPDATING FRIENDS LIST".to_string();
                state.ui_dirty = true;
                backend.remove_friend(tok, uid);
            }
        }
        PreGameBtnAction::CancelPartyInvite(uid) => {
            state.pending_sent_invites.remove(&uid);
            backend.send_social_action(serde_json::json!({
                "action": "cancel_party_invite",
                "target_user_id": uid
            }));
            state.ui_dirty = true;
        }
        PreGameBtnAction::InviteToParty(uid) => {
            if state.pending_sent_invites.contains_key(&uid) {
                return;
            }
            if state.friends.friends.iter().any(|f| f.user_id == uid && f.in_party) {
                push_notification(
                    &mut state,
                    "info",
                    "ALREADY IN PARTY",
                    "This friend is already in another squad.",
                    notif_dur,
                );
                return;
            }
            if let Some(tok) = state.access_token.clone() {
                if !backend.is_social_ws_connected() {
                    backend.connect_social_ws(tok);
                    push_notification(
                        &mut state,
                        "info",
                        "RECONNECTING",
                        "Reconnecting to party server... Try inviting again in a moment.",
                        notif_dur,
                    );
                    return;
                }
            }
            state
                .pending_sent_invites
                .insert(uid.clone(), config.main_menu.party_invite_timeout_secs);
            state.ui_dirty = true;
            let mode_idx = state
                .selected_mode_idx
                .min(config.main_menu.game_modes.len().saturating_sub(1));
            let mode = config.main_menu.game_modes[mode_idx].clone();
            backend.send_social_action(serde_json::json!({
                "action": "invite_to_party",
                "target_user_id": uid,
                "game_mode": mode
            }));
        }
        PreGameBtnAction::RespondPartyInvite { invite_id, accept } => {
            state
                .pending_party_invites
                .retain(|i| i.invite_id != invite_id);
            backend.send_social_action(serde_json::json!({
                "action": "respond_party_invite",
                "invite_id": invite_id,
                "accept": accept
            }));
            state.ui_dirty = true;
        }
        PreGameBtnAction::TogglePartyReady => {
            backend.send_social_action(serde_json::json!({ "action": "toggle_ready" }));
        }
        PreGameBtnAction::KickPartyMember(uid) => {
            backend.send_social_action(serde_json::json!({
                "action": "kick_party_member",
                "target_user_id": uid
            }));
        }
        PreGameBtnAction::PromotePartyLeader(uid) => {
            backend.send_social_action(serde_json::json!({
                "action": "promote_leader",
                "target_user_id": uid
            }));
        }
        PreGameBtnAction::LeaveParty => {
            backend.send_social_action(serde_json::json!({ "action": "leave_party" }));
        }
        PreGameBtnAction::TogglePartyChat => {
            state.party_chat_open = !state.party_chat_open;
            if state.party_chat_open {
                state.party_chat_unread = 0;
                state.focused = FocusedInput::PartyChat;
            } else if state.focused == FocusedInput::PartyChat {
                state.focused = FocusedInput::None;
            }
            state.ui_dirty = true;
        }
        PreGameBtnAction::SendPartyChat => {
            let txt = state.party_chat_input.trim().to_string();
            if !txt.is_empty() {
                backend.send_social_action(serde_json::json!({
                    "action": "party_chat",
                    "text": txt
                }));
                state.party_chat_input.clear();
                state.ui_dirty = true;
            }
        }
        PreGameBtnAction::DismissNotification(nid) => {
            state.notifications.retain(|n| n.id != nid);
            state.ui_dirty = true;
        }
        // Custom Lobby Actions
        PreGameBtnAction::RefreshLobbies => {
            backend.send_lobby_action(serde_json::json!({ "action": "list_lobbies" }));
        }
        PreGameBtnAction::SelectLobbyCapacity(cap) => {
            state.create_capacity = cap;
            state.ui_dirty = true;
        }
        PreGameBtnAction::OpenCreateLobbyModal => {
            if state.active_match.is_some() {
                push_notification(
                    &mut state,
                    "error",
                    "ACTIVE MATCH",
                    "Finish or abandon your active match before creating a lobby.",
                    notif_dur,
                );
                return;
            }
            if is_in_active_squad(&state) {
                push_notification(
                    &mut state,
                    "error",
                    "LEAVE PARTY FIRST",
                    "Leave your active squad before creating a Custom Lobby.",
                    notif_dur,
                );
                return;
            }
            if state.party.is_some() {
                state.party = None;
                state.party_chat_open = false;
                state.party_chat_unread = 0;
                backend.send_social_action(serde_json::json!({ "action": "leave_party" }));
            }
            if state.create_title.trim().is_empty() {
                let uname = state
                    .profile
                    .as_ref()
                    .map(|p| p.username.as_str())
                    .unwrap_or("SOLDIER");
                state.create_title = format!("{}'s Room", uname);
            }
            state.create_lobby_modal_open = true;
            state.edit_lobby_settings_open = false;
            state.focused = FocusedInput::LobbyTitle;
            state.ui_dirty = true;
        }
        PreGameBtnAction::CloseLobbyModal => {
            state.create_lobby_modal_open = false;
            state.edit_lobby_settings_open = false;
            state.focused = FocusedInput::None;
            state.ui_dirty = true;
        }
        PreGameBtnAction::ToggleCreatePasswordVisibility => {
            state.create_password_visible = !state.create_password_visible;
            state.ui_dirty = true;
        }
        PreGameBtnAction::OpenLobbySettingsModal => {
            if let Some((title, cap, is_starting)) = state
                .current_lobby
                .as_ref()
                .map(|lb| (lb.title.clone(), lb.max_players, lb.is_starting))
            {
                if is_starting {
                    return;
                }
                state.create_title = title;
                state.create_password.clear();
                state.create_capacity = cap;
                state.edit_lobby_settings_open = true;
                state.focused = FocusedInput::LobbyTitle;
                state.ui_dirty = true;
            }
        }
        PreGameBtnAction::SaveLobbySettings => {
            if state.current_lobby.as_ref().is_some_and(|lb| lb.is_starting) {
                return;
            }
            state.is_loading = true;
            state.loading_label = "SAVING LOBBY SETTINGS".to_string();
            let mut payload = serde_json::json!({
                "action": "update_settings",
                "title": state.create_title.trim(),
                "max_players": state.create_capacity,
            });
            let pw = state.create_password.trim();
            if !pw.is_empty() {
                payload["password"] = serde_json::Value::String(pw.to_string());
            }
            backend.send_lobby_action(payload);
            state.ui_dirty = true;
        }
        PreGameBtnAction::SwapLobbyTeam => {
            if let Some(ref lb) = state.current_lobby {
                if lb.is_starting {
                    return;
                }
                let my_uid = state
                    .profile
                    .as_ref()
                    .map(|p| p.user_id.as_str())
                    .unwrap_or("");
                if let Some(me) = lb.members.iter().find(|m| m.user_id == my_uid) {
                    let target_team = if me.team_id == 1 { 2 } else { 1 };
                    backend.send_lobby_action(serde_json::json!({
                        "action": "switch_team",
                        "team_id": target_team,
                        "slot_index": -1
                    }));
                }
            }
        }
        PreGameBtnAction::OpenLobbyInviteModal => {
            if let Some(tok) = state.access_token.clone() {
                backend.fetch_friends(tok);
            }
            state.lobby_invite_modal_open = true;
            state.ui_dirty = true;
        }
        PreGameBtnAction::CloseLobbyInviteModal => {
            state.lobby_invite_modal_open = false;
            state.ui_dirty = true;
        }
        PreGameBtnAction::SendLobbyInvite(target_uid) => {
            if let Some(ref lb) = state.current_lobby {
                backend.send_social_action(serde_json::json!({
                    "action": "invite_to_lobby",
                    "target_user_id": target_uid,
                    "lobby_id": lb.lobby_id,
                    "lobby_title": lb.title
                }));
                state.lobby_invite_modal_open = false;
                state.ui_dirty = true;
            }
        }
        PreGameBtnAction::RespondLobbyInvite { lobby_id, accept } => {
            let needs_pw = state
                .pending_lobby_invites
                .iter()
                .find(|x| x.lobby_id == lobby_id)
                .map(|x| x.has_password)
                .unwrap_or(false);
            state
                .pending_lobby_invites
                .retain(|x| x.lobby_id != lobby_id);
            if accept {
                if state.party.is_some() {
                    push_notification(
                        &mut state,
                        "error",
                        "LEAVE PARTY FIRST",
                        "Leave your active squad before joining a Custom Lobby.",
                        notif_dur,
                    );
                    state.ui_dirty = true;
                    return;
                }
                if needs_pw {
                    state.join_target_lobby_id = Some(lobby_id);
                    state.join_password_input.clear();
                    state.focused = FocusedInput::JoinPassword;
                } else if let Some(tok) = state.access_token.clone() {
                    state.is_loading = true;
                    state.loading_label = "JOINING CUSTOM LOBBY".to_string();
                    if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom) {
                        backend.send_lobby_action(serde_json::json!({
                            "action": "join_lobby",
                            "lobby_id": lobby_id,
                            "password": ""
                        }));
                    } else {
                        state.pending_join_lobby = Some((lobby_id, String::new()));
                        backend.connect_lobby_ws(tok);
                    }
                }
            }
            state.ui_dirty = true;
        }
        PreGameBtnAction::ToggleHideFullLobbies => {
            state.lobby_hide_full_locked = !state.lobby_hide_full_locked;
            state.ui_dirty = true;
        }
        PreGameBtnAction::SelectLobbyRow(lid) => {
            state.selected_lobby_id = Some(lid);
            state.ui_dirty = true;
        }
        PreGameBtnAction::JoinSelectedLobby => {
            let Some(ref sel_id) = state.selected_lobby_id else {
                push_notification(
                    &mut state,
                    "info",
                    "SELECT A LOBBY",
                    "Click a lobby row first to join.",
                    notif_dur,
                );
                return;
            };
            if let Some(lb) = state.lobbies.iter().find(|l| &l.lobby_id == sel_id).cloned() {
                if lb.current_players >= lb.max_players {
                    push_notification(
                        &mut state,
                        "error",
                        "LOBBY FULL",
                        "This lobby is already full.",
                        notif_dur,
                    );
                    return;
                }
                if lb.has_password {
                    state.join_target_lobby_id = Some(lb.lobby_id);
                    state.join_password_input.clear();
                    state.focused = FocusedInput::JoinPassword;
                    state.ui_dirty = true;
                } else {
                    state.is_loading = true;
                    state.loading_label = "JOINING CUSTOM LOBBY".to_string();
                    backend.send_lobby_action(serde_json::json!({
                        "action": "join_lobby",
                        "lobby_id": lb.lobby_id,
                        "password": ""
                    }));
                    state.ui_dirty = true;
                }
            }
        }
        PreGameBtnAction::CreateLobby => {
            let Some(tok) = state.access_token.clone() else { return };
            let title = state.create_title.trim().to_string();
            let pw = state.create_password.trim().to_string();
            let cap = state.create_capacity;
            state.is_loading = true;
            state.loading_label = "CREATING CUSTOM LOBBY".to_string();
            if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom) {
                backend.send_lobby_action(serde_json::json!({
                    "action": "create_lobby",
                    "title": title,
                    "password": pw,
                    "max_players": cap,
                }));
            } else {
                state.pending_create_lobby = Some((title, pw, cap));
                backend.connect_lobby_ws(tok);
            }
            state.ui_dirty = true;
        }
        PreGameBtnAction::OpenJoinLobby {
            lobby_id,
            has_password,
        } => {
            if has_password {
                state.join_target_lobby_id = Some(lobby_id);
                state.join_password_input.clear();
                state.focused = FocusedInput::JoinPassword;
                state.ui_dirty = true;
            } else {
                backend.send_lobby_action(serde_json::json!({
                    "action": "join_lobby",
                    "lobby_id": lobby_id,
                    "password": ""
                }));
            }
        }
        PreGameBtnAction::ConfirmJoinLobby => {
            if let Some(lid) = state.join_target_lobby_id.clone() {
                let pw = state.join_password_input.trim().to_string();
                state.is_loading = true;
                state.loading_label = "JOINING CUSTOM LOBBY".to_string();
                if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom) {
                    backend.send_lobby_action(serde_json::json!({
                        "action": "join_lobby",
                        "lobby_id": lid,
                        "password": pw
                    }));
                } else if let Some(tok) = state.access_token.clone() {
                    state.pending_join_lobby = Some((lid, pw));
                    backend.connect_lobby_ws(tok);
                }
                state.ui_dirty = true;
            }
        }
        PreGameBtnAction::CancelJoinModal => {
            state.join_target_lobby_id = None;
            state.focused = FocusedInput::None;
            state.ui_dirty = true;
        }
        PreGameBtnAction::SwitchLobbySlot {
            team_id,
            slot_index,
        } => {
            if state.current_lobby.as_ref().is_some_and(|lb| lb.is_starting) {
                return;
            }
            backend.send_lobby_action(serde_json::json!({
                "action": "switch_team",
                "team_id": team_id,
                "slot_index": slot_index
            }));
        }
        PreGameBtnAction::KickLobbyPlayer(uid) => {
            if state.current_lobby.as_ref().is_some_and(|lb| lb.is_starting) {
                return;
            }
            backend.send_lobby_action(serde_json::json!({
                "action": "kick_player",
                "target_user_id": uid
            }));
        }
        PreGameBtnAction::SendLobbyChat => {
            let txt = state.lobby_chat_input.trim().to_string();
            if !txt.is_empty() {
                backend.send_lobby_action(serde_json::json!({
                    "action": "chat_message",
                    "text": txt
                }));
                state.lobby_chat_input.clear();
                state.ui_dirty = true;
            }
        }
        PreGameBtnAction::StartLobbyMatch => {
            backend.send_lobby_action(serde_json::json!({ "action": "start_match" }));
        }
        PreGameBtnAction::LeaveLobby => {
            backend.send_lobby_action(serde_json::json!({ "action": "leave_lobby" }));
        }
        PreGameBtnAction::SelectHistoryMatch(idx) => {
            state.selected_history_idx = idx;
            state.ui_dirty = true;
        }
        PreGameBtnAction::SetHistoryFilter(f) => {
            state.history_filter = f;
            state.selected_history_idx = 0;
            state.ui_dirty = true;
        }
        PreGameBtnAction::RequestLogoutConfirm => {
            state.confirm_modal = Some(ConfirmAction::Logout);
            state.ui_dirty = true;
        }
        PreGameBtnAction::RequestQuitConfirm => {
            state.confirm_modal = Some(ConfirmAction::QuitApp);
            state.ui_dirty = true;
        }
        PreGameBtnAction::ConfirmModalCancel => {
            state.confirm_modal = None;
            state.ui_dirty = true;
        }
        PreGameBtnAction::ConfirmModalAccept => {
            let act = state.confirm_modal.take();
            match act {
                Some(ConfirmAction::Logout) => {
                    backend.stop_matchmaking();
                    backend.disconnect_lobby_ws();
                    backend.disconnect_social_ws();
                    clear_persisted_session();
                    state.access_token = None;
                    state.profile = None;
                    state.active_match = None;
                    state.party = None;
                    state.party_chat_open = false;
                    state.party_chat_unread = 0;
                    state.friends = FriendsSnapshot::default();
                    state.friend_search_results.clear();
                    state.friend_search_executed = false;
                    state.pending_party_invites.clear();
                    state.pending_sent_invites.clear();
                    state.current_lobby = None;
                    state.history = None;
                    state.tab = PreGameTab::AuthEmail;
                    state.auth_mode = AuthMode::Login;
                    state.focused = FocusedInput::Email;
                    push_notification(
                        &mut state,
                        "info",
                        "LOGGED OUT",
                        "You have been signed out.",
                        notif_dur,
                    );
                }
                Some(ConfirmAction::QuitApp) => {
                    backend.stop_matchmaking();
                    backend.disconnect_lobby_ws();
                    backend.disconnect_social_ws();
                    app_exit.write(AppExit::Success);
                }
                Some(ConfirmAction::AbandonMatch) => {
                    if let Some(tok) = state.access_token.clone() {
                        state.is_loading = true;
                        state.loading_label = "ABANDONING MATCH".to_string();
                        backend.abandon_match(tok);
                    }
                }
                Some(ConfirmAction::RemoveFriend { user_id, username: _ }) => {
                    if let Some(tok) = state.access_token.clone() {
                        state.is_loading = true;
                        state.loading_label = "REMOVING FRIEND".to_string();
                        backend.remove_friend(tok, user_id);
                    }
                }
                None => {}
            }
            state.ui_dirty = true;
        }
    }
}

// =============================================================================
// انیمیشن لودرهای مفهومی و ذرات جنگل
// =============================================================================

fn animate_pregame_loaders_system(
    time: Res<Time>,
    mut orbit_q: Query<(&ConceptualOrbitDot, &mut Node), Without<JungleFirefly>>,
    mut pulse_q: Query<(&PulseGlowNode, &mut BorderColor)>,
    mut firefly_q: Query<(&JungleFirefly, &mut Node, &mut BackgroundColor)>,
) {
    let t = time.elapsed_secs();
    for (dot, mut node) in &mut orbit_q {
        let ang = t * 2.8 + dot.phase;
        let cx = 36.0 + ang.cos() * dot.radius;
        let cy = 36.0 + ang.sin() * dot.radius;
        node.left = Val::Px(cx);
        node.top = Val::Px(cy);
    }
    for (pulse, mut border) in &mut pulse_q {
        let a = 0.35 + 0.55 * (0.5 + 0.5 * (t * pulse.speed).sin());
        *border = BorderColor::all(Color::srgba(0.24, 0.92, 0.78, a));
    }
    for (ff, mut node, mut bg) in &mut firefly_q {
        let dx = (t * ff.speed + ff.phase).sin() * 1.4;
        let dy = (t * ff.speed * 0.7 + ff.phase * 1.3).cos() * 1.1;
        node.left = Val::Percent((ff.base_x_pct + dx).clamp(2.0, 96.0));
        node.top = Val::Percent((ff.base_y_pct + dy).clamp(4.0, 92.0));
        let alpha = 0.20 + 0.50 * (0.5 + 0.5 * (t * 2.1 + ff.phase).sin());
        bg.0 = bg.0.with_alpha(alpha);
    }
}

// =============================================================================
// بازسازی تمیز رابط کاربری PreGame هنگام تغییر وضعیت
// =============================================================================

fn rebuild_pregame_ui_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    backend: Res<BackendClient>,
    hero_portraits: Res<MenuHeroPortraitCache>,
    item_icons: Res<MenuItemIconCache>,
    mut state: ResMut<PreGameSessionState>,
    root_q: Query<(Entity, Option<&Children>), With<PreGameDynamicRoot>>,
) {
    if !state.ui_dirty {
        return;
    }
    state.ui_dirty = false;

    let Ok((root_ent, children)) = root_q.single() else {
        return;
    };
    if let Some(ch_list) = children {
        for ch in ch_list.iter() {
            safe_despawn(&mut commands, ch);
        }
    }

    let is_auth = matches!(state.tab, PreGameTab::AuthEmail | PreGameTab::AuthOtp);
    let has_dim_backdrop = !matches!(state.tab, PreGameTab::Hub | PreGameTab::FindingMatch);

    commands.entity(root_ent).with_children(|parent| {
        if has_dim_backdrop {
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.015, 0.045, 0.055, 0.84)),
            ));
        }

        if let Some(ref profile) = state.profile {
            if !is_auth {
                spawn_top_nav_bar(parent, profile, &state, &config, backend.is_social_ws_connected());
            }
        }

        match state.tab {
            PreGameTab::AuthEmail | PreGameTab::AuthOtp => spawn_auth_screen(parent, &state),
            PreGameTab::Hub => spawn_hub_screen(parent, &state, &config, &hero_portraits),
            PreGameTab::FindingMatch => spawn_finding_match_screen(parent, &state, &config),
            PreGameTab::LobbyBrowser => spawn_lobby_browser_screen(parent, &state, &config),
            PreGameTab::LobbyRoom => spawn_lobby_room_screen(parent, &state, &config),
            PreGameTab::MatchHistory => spawn_history_screen(
                parent,
                &state,
                &config,
                &items_config,
                &hero_portraits,
                &item_icons,
            ),
            PreGameTab::DemoHero => spawn_demo_hero_screen(parent, &config, &hero_portraits),
        }

        if state.create_lobby_modal_open || state.edit_lobby_settings_open {
            spawn_create_or_edit_lobby_modal(parent, &state, &config);
        }
        if state.join_target_lobby_id.is_some() {
            spawn_join_password_modal(parent, &state);
        }
        if state.lobby_invite_modal_open {
            spawn_lobby_invite_modal(parent, &state);
        }
        if !state.pending_party_invites.is_empty()
            || !state.pending_lobby_invites.is_empty()
            || !state.notifications.is_empty()
        {
            spawn_notification_stack(parent, &state);
        }

        if !state.toast_message.is_empty() {
            let (bg, border, txt_col) = if state.toast_is_error {
                (
                    Color::srgba(0.32, 0.06, 0.08, 0.94),
                    Color::srgb(0.95, 0.34, 0.36),
                    Color::srgb(1.0, 0.86, 0.86),
                )
            } else {
                (
                    Color::srgba(0.04, 0.16, 0.18, 0.94),
                    Color::srgb(0.24, 0.88, 0.76),
                    Color::srgb(0.86, 1.0, 0.96),
                )
            };
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(14.0),
                        left: Val::Percent(35.0),
                        width: Val::Percent(30.0),
                        padding: UiRect::axes(Val::Px(18.0), Val::Px(9.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(bg),
                    BorderColor::all(border),
                ))
                .with_children(|t| {
                    t.spawn((
                        Text::new(state.toast_message.clone()),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(txt_col),
                    ));
                });
        }

        if state.is_loading {
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.01, 0.03, 0.04, 0.78)),
                ))
                .with_children(|ov| {
                    ov.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(34.0), Val::Px(26.0)),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(14.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.03, 0.10, 0.12, 0.96)),
                        BorderColor::all(Color::srgb(0.22, 0.88, 0.76)),
                        PulseGlowNode { speed: 3.4 },
                    ))
                    .with_children(|card| {
                        spawn_conceptual_loader(
                            card,
                            &state.loading_label,
                            Color::srgb(0.28, 0.96, 0.84),
                        );
                    });
                });
        }

        if let Some(ref confirm) = state.confirm_modal {
            let (title, body_text, ok_label): (&str, String, &str) = match confirm {
                ConfirmAction::Logout => (
                    "CONFIRM LOG OUT",
                    "Are you sure you want to log out of your account?".to_string(),
                    "LOG OUT",
                ),
                ConfirmAction::QuitApp => (
                    "EXIT GAME",
                    "Are you sure you want to close the game client?".to_string(),
                    "EXIT GAME",
                ),
                ConfirmAction::AbandonMatch => (
                    "ABANDON ACTIVE MATCH?",
                    format!(
                        "Abandoning deducts -{} MMR, records a loss, and prevents reconnecting to this match.",
                        config.main_menu.abandon_mmr_penalty
                    ),
                    "ABANDON MATCH",
                ),
                ConfirmAction::RemoveFriend { username, .. } => (
                    "REMOVE FRIEND?",
                    format!(
                        "Are you sure you want to remove '{}' from your friends list?",
                        username
                    ),
                    "REMOVE",
                ),
            };
            parent
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.01, 0.03, 0.04, 0.78)),
                ))
                .with_children(|ov| {
                    ov.spawn((
                        Node {
                            width: Val::Px(440.0),
                            padding: UiRect::all(Val::Px(24.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(14.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.03, 0.09, 0.11, 0.98)),
                        BorderColor::all(Color::srgb(0.92, 0.34, 0.36)),
                    ))
                    .with_children(|modal| {
                        modal.spawn((
                            Text::new(title),
                            TextFont {
                                font_size: FontSize::Px(19.0),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.78, 0.78)),
                        ));

                        modal.spawn((
                            Text::new(body_text),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.84, 0.92, 0.94)),
                        ));
                        modal
                            .spawn(Node {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::FlexEnd,
                                column_gap: Val::Px(12.0),
                                margin: UiRect::top(Val::Px(6.0)),
                                ..default()
                            })
                            .with_children(|row| {
                                spawn_sized_button(
                                    row,
                                    "CANCEL",
                                    PreGameBtnAction::ConfirmModalCancel,
                                    Color::srgb(0.14, 0.22, 0.26),
                                    Color::srgb(0.45, 0.65, 0.70),
                                    110.0,
                                    38.0,
                                );
                                spawn_sized_button(
                                    row,
                                    ok_label,
                                    PreGameBtnAction::ConfirmModalAccept,
                                    Color::srgb(0.64, 0.16, 0.18),
                                    Color::srgb(0.98, 0.44, 0.44),
                                    148.0,
                                    38.0,
                                );
                            });
                    });
                });
        }
    });
}

// =============================================================================
// نوار ناوبری بالای صفحه
// =============================================================================

fn spawn_top_nav_bar(
    parent: &mut ChildSpawnerCommands,
    profile: &PlayerProfile,
    state: &PreGameSessionState,
    config: &GameConfig,
    ws_online: bool,
) {
    let level_est = (profile.wins + profile.losses + 1).max(1);
    let step = config.main_menu.mmr_tier_bar_step;
    let mmr_progress_pct = (profile.mmr.rem_euclid(step) as f32 / step as f32 * 100.0).clamp(8.0, 100.0);

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(64.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                border: UiRect::bottom(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.068, 0.082, 0.92)),
            BorderColor::all(Color::srgba(0.20, 0.72, 0.64, 0.55)),
        ))
        .with_children(|bar| {
            bar.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|left| {
                left.spawn((
                    Node {
                        width: Val::Px(46.0),
                        height: Val::Px(46.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.22, 0.24)),
                    BorderColor::all(Color::srgb(0.32, 0.88, 0.76)),
                ))
                .with_children(|av| {
                    let initial = profile
                        .username
                        .chars()
                        .next()
                        .unwrap_or('P')
                        .to_ascii_uppercase();
                    av.spawn((
                        Text::new(initial.to_string()),
                        TextFont {
                            font_size: FontSize::Px(22.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.98, 0.86, 0.42)),
                    ));
                });

                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|info| {
                    info.spawn((
                        Text::new(profile.username.to_uppercase()),
                        TextFont {
                            font_size: FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.98, 0.97)),
                    ));
                    info.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|sub| {
                        sub.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.24, 0.18, 0.05, 0.92)),
                            BorderColor::all(Color::srgb(0.92, 0.74, 0.28)),
                        ))
                        .with_children(|lv| {
                            lv.spawn((
                                Text::new(format!("LV {}", level_est)),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.86, 0.42)),
                            ));
                        });
                        sub.spawn((
                            Node {
                                width: Val::Px(96.0),
                                height: Val::Px(6.0),
                                border_radius: BorderRadius::all(Val::Px(2.0)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.10, 0.20, 0.22, 0.95)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                Node {
                                    width: Val::Percent(mmr_progress_pct),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.90, 0.76, 0.26)),
                            ));
                        });
                    });
                });
            });

            bar.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|tabs| {
                spawn_nav_pill(
                    tabs,
                    "PLAY",
                    matches!(state.tab, PreGameTab::Hub | PreGameTab::FindingMatch),
                    PreGameBtnAction::SwitchTab(PreGameTab::Hub),
                );
                spawn_nav_pill(
                    tabs,
                    "LOBBY",
                    matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom),
                    PreGameBtnAction::SwitchTab(PreGameTab::LobbyBrowser),
                );
                spawn_nav_pill(
                    tabs,
                    "MATCH HISTORY",
                    state.tab == PreGameTab::MatchHistory,
                    PreGameBtnAction::SwitchTab(PreGameTab::MatchHistory),
                );
                spawn_nav_pill(
                    tabs,
                    "DEMO HERO",
                    state.tab == PreGameTab::DemoHero,
                    PreGameBtnAction::SwitchTab(PreGameTab::DemoHero),
                );
            });

            bar.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|right| {
                // نشانگر وضعیت اتصال زنده وب‌سوکت اجتماعی
                right
                    .spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(5.0),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.03, 0.11, 0.13, 0.90)),
                        BorderColor::all(if ws_online {
                            Color::srgb(0.24, 0.92, 0.58)
                        } else {
                            Color::srgb(0.96, 0.66, 0.22)
                        }),
                    ))
                    .with_children(|ws| {
                        ws.spawn((
                            Node {
                                width: Val::Px(7.0),
                                height: Val::Px(7.0),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(if ws_online {
                                Color::srgb(0.24, 0.96, 0.58)
                            } else {
                                Color::srgb(0.98, 0.68, 0.24)
                            }),
                        ));
                        ws.spawn((
                            Text::new(if ws_online { "LIVE" } else { "RECONNECTING" }),
                            TextFont {
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(if ws_online {
                                Color::srgb(0.76, 0.98, 0.86)
                            } else {
                                Color::srgb(0.98, 0.82, 0.44)
                            }),
                        ));
                    });

                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        margin: UiRect::right(Val::Px(6.0)),
                        ..default()
                    })
                    .with_children(|m| {
                        m.spawn((
                            Text::new("MMR"),
                            TextFont {
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.60, 0.74, 0.72)),
                        ));
                        m.spawn((
                            Text::new(format!("{}", profile.mmr)),
                            TextFont {
                                font_size: FontSize::Px(15.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.82, 0.30)),
                        ));
                    });

                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        margin: UiRect::right(Val::Px(8.0)),
                        ..default()
                    })
                    .with_children(|w| {
                        w.spawn((
                            Text::new("RECORD"),
                            TextFont {
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.60, 0.74, 0.72)),
                        ));
                        w.spawn((
                            Text::new(format!("{}W / {}L", profile.wins, profile.losses)),
                            TextFont {
                                font_size: FontSize::Px(14.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.38, 0.86, 0.98)),
                        ));
                    });

                spawn_sized_button(
                    right,
                    "LOG OUT",
                    PreGameBtnAction::RequestLogoutConfirm,
                    Color::srgba(0.36, 0.11, 0.12, 0.92),
                    Color::srgb(0.76, 0.28, 0.30),
                    96.0,
                    34.0,
                );
                spawn_sized_button(
                    right,
                    "EXIT GAME",
                    PreGameBtnAction::RequestQuitConfirm,
                    Color::srgba(0.28, 0.08, 0.09, 0.95),
                    Color::srgb(0.66, 0.22, 0.24),
                    104.0,
                    34.0,
                );
            });
        });
}

fn spawn_nav_pill(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    active: bool,
    action: PreGameBtnAction,
) {
    let (bg, border, txt) = if active {
        (
            Color::srgba(0.08, 0.34, 0.34, 0.92),
            Color::srgb(0.30, 0.95, 0.84),
            Color::srgb(0.96, 1.0, 0.98),
        )
    } else {
        (Color::NONE, Color::NONE, Color::srgb(0.68, 0.80, 0.80))
    };
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                border: UiRect::bottom(Val::Px(if active { 2.5 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(txt),
            ));
        });
}

// =============================================================================
// صفحه ورود و ثبت‌نام مجزا (Login vs Register با اعتبارسنجی نام کاربری)
// =============================================================================

fn spawn_auth_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let is_reg = state.auth_mode == AuthMode::Register;
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|center| {
            center
                .spawn((
                    Node {
                        width: Val::Px(460.0),
                        padding: UiRect::all(Val::Px(28.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: Val::Px(14.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.085, 0.10, 0.96)),
                    BorderColor::all(Color::srgb(0.24, 0.82, 0.72)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("JUNGLE SKIRMISH // TACTICAL MOBA"),
                        TextFont {
                            font_size: FontSize::Px(21.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.98, 0.84, 0.36)),
                    ));

                    if state.tab == PreGameTab::AuthEmail {
                        // تب‌های مجزای LOG IN و REGISTER
                        card.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|tabs| {
                            spawn_sized_button(
                                tabs,
                                "LOG IN",
                                PreGameBtnAction::SwitchAuthMode(AuthMode::Login),
                                if !is_reg {
                                    Color::srgb(0.09, 0.42, 0.38)
                                } else {
                                    Color::srgb(0.05, 0.14, 0.16)
                                },
                                if !is_reg {
                                    Color::srgb(0.30, 0.96, 0.84)
                                } else {
                                    Color::srgba(0.26, 0.52, 0.52, 0.55)
                                },
                                196.0,
                                36.0,
                            );
                            spawn_sized_button(
                                tabs,
                                "REGISTER",
                                PreGameBtnAction::SwitchAuthMode(AuthMode::Register),
                                if is_reg {
                                    Color::srgb(0.58, 0.40, 0.08)
                                } else {
                                    Color::srgb(0.05, 0.14, 0.16)
                                },
                                if is_reg {
                                    Color::srgb(0.98, 0.84, 0.34)
                                } else {
                                    Color::srgba(0.26, 0.52, 0.52, 0.55)
                                },
                                196.0,
                                36.0,
                            );
                        });

                        card.spawn((
                            Text::new(if is_reg {
                                "Choose a unique soldier username (3-24 chars) and your email."
                            } else {
                                "Sign in with your registered email address."
                            }),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.76, 0.88, 0.88)),
                        ));

                        if is_reg {
                            spawn_menu_input(
                                card,
                                "SOLDIER USERNAME",
                                &state.username_input,
                                "e.g. Jungle_Raider",
                                state.focused == FocusedInput::Username,
                                FocusedInput::Username,
                                false,
                            );
                            if !state.username_input.is_empty() {
                                match validate_client_username(&state.username_input) {
                                    Ok(()) => {
                                        card.spawn((
                                            Text::new("Username format is valid."),
                                            TextFont {
                                                font_size: FontSize::Px(11.0),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.32, 0.94, 0.62)),
                                        ));
                                    }
                                    Err(e) => {
                                        card.spawn((
                                            Text::new(e),
                                            TextFont {
                                                font_size: FontSize::Px(11.0),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.98, 0.46, 0.44)),
                                        ));
                                    }
                                }
                            }
                        }

                        spawn_menu_input(
                            card,
                            "EMAIL ADDRESS",
                            &state.email_input,
                            "soldier@example.com",
                            state.focused == FocusedInput::Email,
                            FocusedInput::Email,
                            false,
                        );

                        spawn_sized_button(
                            card,
                            if is_reg {
                                "CREATE ACCOUNT & SEND OTP"
                            } else {
                                "SEND LOGIN OTP"
                            },
                            PreGameBtnAction::RequestOtp,
                            if is_reg {
                                Color::srgb(0.62, 0.44, 0.10)
                            } else {
                                Color::srgb(0.09, 0.44, 0.38)
                            },
                            if is_reg {
                                Color::srgb(1.0, 0.86, 0.36)
                            } else {
                                Color::srgb(0.30, 0.96, 0.82)
                            },
                            404.0,
                            46.0,
                        );
                    } else {
                        card.spawn((
                            Text::new(format!(
                                "Enter the 6-digit OTP code sent for {}.",
                                state.email_input
                            )),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.76, 0.88, 0.88)),
                        ));
                        spawn_menu_input(
                            card,
                            "6-DIGIT OTP CODE",
                            &state.otp_input,
                            "123456",
                            state.focused == FocusedInput::Otp,
                            FocusedInput::Otp,
                            false,
                        );
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_sized_button(
                                row,
                                "BACK",
                                PreGameBtnAction::BackToEmail,
                                Color::srgb(0.14, 0.22, 0.25),
                                Color::srgb(0.45, 0.64, 0.68),
                                120.0,
                                44.0,
                            );
                            spawn_sized_button(
                                row,
                                "VERIFY & ENTER",
                                PreGameBtnAction::VerifyOtp,
                                Color::srgb(0.68, 0.48, 0.10),
                                Color::srgb(1.0, 0.84, 0.34),
                                270.0,
                                44.0,
                            );
                        });
                    }
                });
        });
}

// =============================================================================
// صفحه اصلی منو (Main Menu Hub):
// - ستون چپ: پنل همیشه‌باز FRIENDS بالای کارت PARTY
// - پایین وسط-چپ: کارت RECENT MATCHES
// - پایین راست: CREATE LOBBY و FIND MATCH (بدون دکمه انتخاب نوع بازی)
// =============================================================================

fn spawn_hub_screen(
    parent: &mut ChildSpawnerCommands,
    state: &PreGameSessionState,
    config: &GameConfig,
    hero_portraits: &MenuHeroPortraitCache,
) {
    let my_uid = state
        .profile
        .as_ref()
        .map(|p| p.user_id.as_str())
        .unwrap_or("");
    let my_name = state
        .profile
        .as_ref()
        .map(|p| p.username.as_str())
        .unwrap_or("SOLDIER");
    let max_party = config.main_menu.party_max_size;
    let party_members_count = state.party.as_ref().map(|p| p.members.len()).unwrap_or(1);
    let in_party = is_in_active_squad(state);
    let show_party_controls = in_party;
    let is_party_leader = state
        .party
        .as_ref()
        .map(|p| p.leader_user_id == my_uid)
        .unwrap_or(true);
    let am_i_ready = state
        .party
        .as_ref()
        .and_then(|p| p.members.iter().find(|m| m.user_id == my_uid))
        .map(|m| m.is_ready)
        .unwrap_or(false);

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::all(Val::Px(20.0)),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|stage| {
            // =================================================================
            // ستون چپ: پنل همیشه‌باز FRIENDS (بالا) + کارت PARTY (پایین)
            // =================================================================
            stage
                .spawn(Node {
                    width: Val::Px(326.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Stretch,
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|left_col| {
                    // ۱. پنل همیشه‌باز دوستان بالای کارت اسکواد
                    spawn_left_friends_panel(left_col, state);

                    // چت سریع پارتی در صورت باز بودن
                    if show_party_controls && state.party_chat_open {
                        if let Some(ref p) = state.party {
                            spawn_party_chat_box(left_col, p, state);
                        }
                    }

                    // ۲. کارت PARTY (۵ اسلات افقی)
                    left_col
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(12.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(10.0),
                                border: UiRect::all(Val::Px(1.5)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.025, 0.082, 0.098, 0.88)),
                            BorderColor::all(Color::srgba(0.24, 0.64, 0.62, 0.70)),
                        ))
                        .with_children(|party_card| {
                            party_card
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    align_items: AlignItems::Center,
                                    ..default()
                                })
                                .with_children(|hdr| {
                                    hdr.spawn((
                                        Text::new(format!(
                                            "PARTY  {}/{}",
                                            party_members_count, max_party
                                        )),
                                        TextFont {
                                            font_size: FontSize::Px(13.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.84, 0.94, 0.92)),
                                    ));
                                    // فقط وقتی بیش از ۱ نفر در اسکواد باشد دکمه‌های Ready/Chat/Leave نمایش داده می‌شوند
                                    if show_party_controls {
                                        hdr.spawn(Node {
                                            flex_direction: FlexDirection::Row,
                                            column_gap: Val::Px(5.0),
                                            ..default()
                                        })
                                        .with_children(|acts| {
                                            spawn_sized_button(
                                                acts,
                                                if am_i_ready { "READY" } else { "NOT READY" },
                                                PreGameBtnAction::TogglePartyReady,
                                                if am_i_ready {
                                                    Color::srgb(0.10, 0.46, 0.26)
                                                } else {
                                                    Color::srgb(0.68, 0.36, 0.06)
                                                },
                                                if am_i_ready {
                                                    Color::srgb(0.30, 0.96, 0.58)
                                                } else {
                                                    Color::srgb(0.98, 0.66, 0.20)
                                                },
                                                78.0,
                                                22.0,
                                            );
                                            let chat_lbl = if state.party_chat_unread > 0 {
                                                format!("CHAT({})", state.party_chat_unread)
                                            } else {
                                                "CHAT".to_string()
                                            };
                                            spawn_sized_button(
                                                acts,
                                                &chat_lbl,
                                                PreGameBtnAction::TogglePartyChat,
                                                if state.party_chat_unread > 0 {
                                                    Color::srgb(0.42, 0.30, 0.06)
                                                } else {
                                                    Color::srgb(0.08, 0.24, 0.28)
                                                },
                                                if state.party_chat_unread > 0 {
                                                    Color::srgb(1.0, 0.84, 0.30)
                                                } else {
                                                    Color::srgb(0.30, 0.80, 0.82)
                                                },
                                                if state.party_chat_unread > 0 { 62.0 } else { 46.0 },
                                                22.0,
                                            );
                                            spawn_sized_button(
                                                acts,
                                                "LEAVE",
                                                PreGameBtnAction::LeaveParty,
                                                Color::srgb(0.42, 0.12, 0.14),
                                                Color::srgb(0.92, 0.36, 0.38),
                                                50.0,
                                                22.0,
                                            );
                                        });
                                    }
                                });

                            party_card
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    column_gap: Val::Px(6.0),
                                    ..default()
                                })
                                .with_children(|slots_row| {
                                    for idx in 0..max_party {
                                        let member_opt = state
                                            .party
                                            .as_ref()
                                            .and_then(|p| p.members.get(idx));
                                        if let Some(m) = member_opt {
                                            let is_self = m.user_id == my_uid;
                                            let border_col = if m.is_leader {
                                                Color::srgb(0.96, 0.80, 0.30)
                                            } else if m.is_ready {
                                                Color::srgb(0.28, 0.92, 0.60)
                                            } else {
                                                Color::srgb(0.96, 0.56, 0.16)
                                            };
                                            slots_row
                                                .spawn((
                                                    Node {
                                                        width: Val::Px(54.0),
                                                        height: Val::Px(60.0),
                                                        flex_direction: FlexDirection::Column,
                                                        justify_content: JustifyContent::SpaceBetween,
                                                        align_items: AlignItems::Center,
                                                        padding: UiRect::all(Val::Px(4.0)),
                                                        border: UiRect::all(Val::Px(1.5)),
                                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                                        ..default()
                                                    },
                                                    BackgroundColor(Color::srgba(0.06, 0.18, 0.21, 0.95)),
                                                    BorderColor::all(border_col),
                                                ))
                                                .with_children(|cell| {
                                                    let badge = if m.is_leader {
                                                        if m.is_ready || !show_party_controls {
                                                            "LDR"
                                                        } else {
                                                            "WAIT"
                                                        }
                                                    } else if m.is_ready {
                                                        "RDY"
                                                    } else {
                                                        "WAIT"
                                                    };
                                                    cell.spawn((
                                                        Text::new(badge),
                                                        TextFont {
                                                            font_size: FontSize::Px(9.0),
                                                            ..default()
                                                        },
                                                        TextColor(border_col),
                                                    ));
                                                    let ch = m
                                                        .username
                                                        .chars()
                                                        .next()
                                                        .unwrap_or('S')
                                                        .to_ascii_uppercase();
                                                    cell.spawn((
                                                        Text::new(ch.to_string()),
                                                        TextFont {
                                                            font_size: FontSize::Px(18.0),
                                                            ..default()
                                                        },
                                                        TextColor(Color::WHITE),
                                                    ));
                                                    if is_party_leader && !is_self {
                                                        cell.spawn(Node {
                                                            flex_direction: FlexDirection::Row,
                                                            column_gap: Val::Px(2.0),
                                                            ..default()
                                                        })
                                                        .with_children(|macts| {
                                                            macts.spawn((
                                                                UiButton,
                                                                Hovered::default(),
                                                                Node {
                                                                    padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                                                                    border: UiRect::all(Val::Px(1.0)),
                                                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                                                    ..default()
                                                                },
                                                                BackgroundColor(Color::srgb(0.40, 0.30, 0.06)),
                                                                BorderColor::all(Color::srgb(0.96, 0.80, 0.30)),
                                                                PreGameBtnAction::PromotePartyLeader(m.user_id.clone()),
                                                            ))
                                                            .with_children(|pb| {
                                                                pb.spawn((
                                                                    Text::new("^"),
                                                                    TextFont {
                                                                        font_size: FontSize::Px(8.5),
                                                                        ..default()
                                                                    },
                                                                    TextColor(Color::srgb(1.0, 0.90, 0.45)),
                                                                ));
                                                            });
                                                            macts.spawn((
                                                                UiButton,
                                                                Hovered::default(),
                                                                Node {
                                                                    padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                                                                    border: UiRect::all(Val::Px(1.0)),
                                                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                                                    ..default()
                                                                },
                                                                BackgroundColor(Color::srgb(0.48, 0.12, 0.14)),
                                                                BorderColor::all(Color::srgb(0.95, 0.36, 0.38)),
                                                                PreGameBtnAction::KickPartyMember(m.user_id.clone()),
                                                            ))
                                                            .with_children(|kb| {
                                                                kb.spawn((
                                                                    Text::new("X"),
                                                                    TextFont {
                                                                        font_size: FontSize::Px(8.5),
                                                                        ..default()
                                                                    },
                                                                    TextColor(Color::WHITE),
                                                                ));
                                                            });
                                                        });
                                                    } else {
                                                        let short: String =
                                                            m.username.chars().take(6).collect();
                                                        cell.spawn((
                                                            Text::new(short),
                                                            TextFont {
                                                                font_size: FontSize::Px(9.0),
                                                                ..default()
                                                            },
                                                            TextColor(Color::srgb(0.78, 0.90, 0.90)),
                                                        ));
                                                    }
                                                });
                                        } else if idx == 0 && !in_party {
                                            slots_row
                                                .spawn((
                                                    Node {
                                                        width: Val::Px(54.0),
                                                        height: Val::Px(60.0),
                                                        flex_direction: FlexDirection::Column,
                                                        justify_content: JustifyContent::SpaceBetween,
                                                        align_items: AlignItems::Center,
                                                        padding: UiRect::all(Val::Px(4.0)),
                                                        border: UiRect::all(Val::Px(1.5)),
                                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                                        ..default()
                                                    },
                                                    BackgroundColor(Color::srgba(0.08, 0.22, 0.25, 0.96)),
                                                    BorderColor::all(Color::srgb(0.96, 0.80, 0.30)),
                                                ))
                                                .with_children(|cell| {
                                                    cell.spawn((
                                                        Text::new("YOU"),
                                                        TextFont {
                                                            font_size: FontSize::Px(9.5),
                                                            ..default()
                                                        },
                                                        TextColor(Color::srgb(0.96, 0.82, 0.34)),
                                                    ));
                                                    let ch = my_name
                                                        .chars()
                                                        .next()
                                                        .unwrap_or('S')
                                                        .to_ascii_uppercase();
                                                    cell.spawn((
                                                        Text::new(ch.to_string()),
                                                        TextFont {
                                                            font_size: FontSize::Px(20.0),
                                                            ..default()
                                                        },
                                                        TextColor(Color::WHITE),
                                                    ));
                                                    let short: String =
                                                        my_name.chars().take(6).collect();
                                                    cell.spawn((
                                                        Text::new(short),
                                                        TextFont {
                                                            font_size: FontSize::Px(9.0),
                                                            ..default()
                                                        },
                                                        TextColor(Color::srgb(0.84, 0.94, 0.94)),
                                                    ));
                                                });
                                        } else {
                                            slots_row
                                                .spawn((
                                                    UiButton,
                                                    Hovered::default(),
                                                    Node {
                                                        width: Val::Px(54.0),
                                                        height: Val::Px(60.0),
                                                        justify_content: JustifyContent::Center,
                                                        align_items: AlignItems::Center,
                                                        border: UiRect::all(Val::Px(1.5)),
                                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                                        ..default()
                                                    },
                                                    BackgroundColor(Color::srgba(0.04, 0.12, 0.14, 0.82)),
                                                    BorderColor::all(Color::srgba(0.26, 0.62, 0.60, 0.55)),
                                                    PreGameBtnAction::FocusField(FocusedInput::FriendSearch),
                                                ))
                                                .with_children(|plus| {
                                                    plus.spawn((
                                                        Text::new("+"),
                                                        TextFont {
                                                            font_size: FontSize::Px(26.0),
                                                            ..default()
                                                        },
                                                        TextColor(Color::srgb(0.55, 0.84, 0.82)),
                                                    ));
                                                });
                                        }
                                    }
                                });
                        });
                });

            // =================================================================
            // پایین وسط: کارت RECENT MATCHES (با آیکون هیرو)
            // =================================================================
            stage
                .spawn((
                    Node {
                        width: Val::Px(316.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.082, 0.098, 0.90)),
                    BorderColor::all(Color::srgba(0.24, 0.64, 0.62, 0.72)),
                ))
                .with_children(|recent| {
                    recent.spawn((
                        Text::new("RECENT MATCHES"),
                        TextFont {
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.84, 0.94, 0.92)),
                    ));

                    let matches_slice = state
                        .history
                        .as_ref()
                        .map(|h| h.matches.as_slice())
                        .unwrap_or(&[]);

                    if matches_slice.is_empty() {
                        recent.spawn((
                            Text::new("No recent battles yet."),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.56, 0.70, 0.70)),
                        ));
                    } else {
                        let icon_sz = config.main_menu.recent_match_icon_size_px;
                        let icon_rad = config.main_menu.hero_icon_border_radius_px;
                        for m in matches_slice.iter().take(3) {
                            let (lbl, row_bg, lbl_col) = match m.outcome.as_str() {
                                "win" => (
                                    "VICTORY",
                                    Color::srgba(0.06, 0.24, 0.16, 0.86),
                                    Color::srgb(0.32, 0.96, 0.56),
                                ),
                                "loss" => (
                                    "DEFEAT",
                                    Color::srgba(0.28, 0.08, 0.09, 0.86),
                                    Color::srgb(0.98, 0.42, 0.42),
                                ),
                                "abandoned" => (
                                    "ABANDON",
                                    Color::srgba(0.28, 0.18, 0.05, 0.86),
                                    Color::srgb(0.98, 0.78, 0.34),
                                ),
                                _ => (
                                    "LIVE",
                                    Color::srgba(0.07, 0.22, 0.28, 0.86),
                                    Color::srgb(0.40, 0.90, 0.98),
                                ),
                            };
                            let hero_def = resolve_hero_def(config, m.hero_id);
                            let hero_name = resolve_hero_display_name(config, m.hero_id);
                            let party_tag = if m.is_party_match { " [P]" } else { "" };
                            recent
                                .spawn((
                                    UiButton,
                                    Hovered::default(),
                                    Node {
                                        width: Val::Percent(100.0),
                                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(8.0),
                                        border: UiRect {
                                            left: Val::Px(3.0),
                                            right: Val::Px(1.0),
                                            top: Val::Px(1.0),
                                            bottom: Val::Px(1.0),
                                        },
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(row_bg),
                                    BorderColor::all(lbl_col.with_alpha(0.55)),
                                    PreGameBtnAction::SwitchTab(PreGameTab::MatchHistory),
                                ))
                                .with_children(|r| {
                                    r.spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(8.0),
                                        ..default()
                                    })
                                    .with_children(|left| {
                                        spawn_hero_portrait_frame(
                                            left,
                                            hero_def,
                                            hero_portraits,
                                            icon_sz,
                                            icon_rad,
                                            lbl_col.with_alpha(0.85),
                                            1.5,
                                            None,
                                        );
                                        left.spawn(Node {
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(1.0),
                                            ..default()
                                        })
                                        .with_children(|meta| {
                                            meta.spawn((
                                                Text::new(hero_name),
                                                TextFont {
                                                    font_size: FontSize::Px(11.5),
                                                    ..default()
                                                },
                                                TextColor(Color::WHITE),
                                            ));
                                            meta.spawn((
                                                Text::new(format!("{}{}", lbl, party_tag)),
                                                TextFont {
                                                    font_size: FontSize::Px(10.0),
                                                    ..default()
                                                },
                                                TextColor(lbl_col),
                                            ));
                                        });
                                    });

                                    r.spawn((
                                        Text::new(format!(
                                            "{}/{}/{}",
                                            m.kills, m.deaths, m.assists
                                        )),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.94, 0.98, 0.98)),
                                    ));
                                });
                        }
                    }
                });

            // =================================================================
            // پایین راست: فقط CREATE LOBBY و FIND MATCH (یا ABANDON و RECONNECT)
            // =================================================================
            stage
                .spawn(Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|btns| {
                    if state.active_match.is_some() {
                        spawn_sized_button(
                            btns,
                            "ABANDON",
                            PreGameBtnAction::RequestAbandonConfirm,
                            Color::srgba(0.54, 0.12, 0.14, 0.95),
                            Color::srgb(0.98, 0.42, 0.42),
                            175.0,
                            62.0,
                        );
                        spawn_sized_button(
                            btns,
                            "RECONNECT",
                            PreGameBtnAction::ReconnectActiveMatch,
                            Color::srgba(0.08, 0.42, 0.34, 0.95),
                            Color::srgb(0.32, 0.98, 0.78),
                            230.0,
                            62.0,
                        );
                    } else {
                        spawn_sized_button(
                            btns,
                            "CREATE LOBBY",
                            PreGameBtnAction::OpenCreateLobbyModal,
                            Color::srgba(0.04, 0.24, 0.22, 0.92),
                            Color::srgb(0.24, 0.92, 0.76),
                            218.0,
                            62.0,
                        );
                        let (find_lbl, find_act, find_bg, find_bdr) = if show_party_controls {
                            if is_party_leader {
                                let ready_cnt = state
                                    .party
                                    .as_ref()
                                    .map(|p| p.members.iter().filter(|m| m.is_ready).count())
                                    .unwrap_or(0);
                                if ready_cnt < party_members_count {
                                    (
                                        format!("WAIT READY ({}/{})", ready_cnt, party_members_count),
                                        PreGameBtnAction::StartFindMatch,
                                        Color::srgba(0.44, 0.30, 0.08, 0.92),
                                        Color::srgb(0.94, 0.72, 0.28),
                                    )
                                } else {
                                    (
                                        format!("FIND MATCH ({})", party_members_count),
                                        PreGameBtnAction::StartFindMatch,
                                        Color::srgba(0.68, 0.48, 0.10, 0.96),
                                        Color::srgb(1.0, 0.86, 0.36),
                                    )
                                }
                            } else if am_i_ready {
                                (
                                    "CANCEL READY".to_string(),
                                    PreGameBtnAction::TogglePartyReady,
                                    Color::srgba(0.10, 0.46, 0.28, 0.96),
                                    Color::srgb(0.32, 0.98, 0.62),
                                )
                            } else {
                                (
                                    "READY UP".to_string(),
                                    PreGameBtnAction::TogglePartyReady,
                                    Color::srgba(0.66, 0.38, 0.08, 0.96),
                                    Color::srgb(1.0, 0.74, 0.26),
                                )
                            }
                        } else {
                            (
                                "FIND MATCH".to_string(),
                                PreGameBtnAction::StartFindMatch,
                                Color::srgba(0.68, 0.48, 0.10, 0.96),
                                Color::srgb(1.0, 0.86, 0.36),
                            )
                        };
                        spawn_sized_button(
                            btns,
                            &find_lbl,
                            find_act,
                            find_bg,
                            find_bdr,
                            248.0,
                            62.0,
                        );
                    }
                });
        });
}

// =============================================================================
// پنل همیشه‌باز دوستان و جستجوی بازیکن در سمت چپ بالای کارت PARTY
// =============================================================================

fn spawn_left_friends_panel(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let online_cnt = state.friends.friends.iter().filter(|f| f.online).count();
    let my_uid = state
        .profile
        .as_ref()
        .map(|p| p.user_id.as_str())
        .unwrap_or("");
    let in_squad = is_in_active_squad(state);
    let is_party_leader = state
        .party
        .as_ref()
        .map(|p| p.leader_user_id == my_uid)
        .unwrap_or(true);
    let party_is_queuing = state
        .party
        .as_ref()
        .map(|p| p.is_queuing)
        .unwrap_or(false);
    let party_is_full = state
        .party
        .as_ref()
        .map(|p| p.members.len() >= p.max_size)
        .unwrap_or(false);

    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.072, 0.088, 0.88)),
            BorderColor::all(Color::srgba(0.24, 0.72, 0.68, 0.70)),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new(format!(
                            "FRIENDS ({} ONLINE / {})",
                            online_cnt,
                            state.friends.friends.len()
                        )),
                        TextFont {
                            font_size: FontSize::Px(12.5),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.86, 0.42)),
                    ));
                    spawn_sized_button(
                        hdr,
                        if state.social_syncing { "SYNCING.." } else { "SYNC" },
                        PreGameBtnAction::RefreshFriends,
                        Color::srgb(0.08, 0.26, 0.30),
                        Color::srgb(0.32, 0.84, 0.86),
                        68.0,
                        24.0,
                    );
                });

            // جستجوی بازیکن + دکمه پاک‌سازی جستجو
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|search_row| {
                    spawn_menu_input(
                        search_row,
                        "",
                        &state.friend_search_input,
                        "Search username...",
                        state.focused == FocusedInput::FriendSearch,
                        FocusedInput::FriendSearch,
                        false,
                    );
                    if !state.friend_search_input.is_empty()
                        || !state.friend_search_results.is_empty()
                        || state.friend_search_executed
                    {
                        spawn_sized_button(
                            search_row,
                            "X",
                            PreGameBtnAction::ClearFriendSearch,
                            Color::srgb(0.28, 0.14, 0.16),
                            Color::srgb(0.84, 0.38, 0.40),
                            28.0,
                            36.0,
                        );
                    }
                    spawn_sized_button(
                        search_row,
                        if state.friend_searching { "..." } else { "FIND" },
                        PreGameBtnAction::SearchPlayers,
                        Color::srgb(0.10, 0.40, 0.36),
                        Color::srgb(0.30, 0.94, 0.82),
                        52.0,
                        36.0,
                    );
                });

            // ناحیه اسکرول‌پذیر یکپارچه برای نتایج جستجو، درخواست‌های ورودی، درخواست‌های ارسالی و لیست دوستان
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                    RelativeCursorPosition::default(),
                    PreGameScrollableList,
                ))
                .with_children(|scroll| {
                    // ۱. نتایج جستجو
                    if state.friend_search_executed && state.friend_search_results.is_empty() {
                        scroll.spawn((
                            Text::new("No players found matching your search."),
                            TextFont {
                                font_size: FontSize::Px(11.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.82, 0.64, 0.44)),
                        ));
                    } else if !state.friend_search_results.is_empty() {
                        scroll.spawn((
                            Text::new(format!(
                                "SEARCH RESULTS ({})",
                                state.friend_search_results.len()
                            )),
                            TextFont {
                                font_size: FontSize::Px(10.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.45, 0.92, 0.86)),
                        ));
                        for res in state.friend_search_results.iter().take(6) {
                            scroll
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_shrink: 0.0,
                                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.06, 0.16, 0.19, 0.92)),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new(format!("{} ({})", res.username, res.mmr)),
                                        TextFont {
                                            font_size: FontSize::Px(11.5),
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                    if res.relationship == "none" || res.relationship == "declined" {
                                        spawn_sized_button(
                                            row,
                                            "+ ADD",
                                            PreGameBtnAction::SendFriendRequest(res.user_id.clone()),
                                            Color::srgb(0.10, 0.42, 0.28),
                                            Color::srgb(0.32, 0.95, 0.62),
                                            58.0,
                                            22.0,
                                        );
                                    } else if res.relationship == "outgoing_pending" {
                                        spawn_sized_button(
                                            row,
                                            "CANCEL",
                                            PreGameBtnAction::CancelFriendRequest(res.user_id.clone()),
                                            Color::srgb(0.44, 0.22, 0.10),
                                            Color::srgb(0.96, 0.56, 0.28),
                                            62.0,
                                            22.0,
                                        );
                                    } else if res.relationship == "incoming_pending" {
                                        if let Some(inc) = state
                                            .friends
                                            .incoming_requests
                                            .iter()
                                            .find(|r| r.user_id == res.user_id)
                                        {
                                            spawn_sized_button(
                                                row,
                                                "ACCEPT",
                                                PreGameBtnAction::RespondFriendRequest {
                                                    request_id: inc.request_id,
                                                    accept: true,
                                                },
                                                Color::srgb(0.10, 0.46, 0.28),
                                                Color::srgb(0.32, 0.96, 0.62),
                                                62.0,
                                                22.0,
                                            );
                                        } else {
                                            row.spawn((
                                                Text::new("INCOMING"),
                                                TextFont {
                                                    font_size: FontSize::Px(10.0),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.98, 0.82, 0.34)),
                                            ));
                                        }
                                    } else {
                                        row.spawn((
                                            Text::new("FRIEND"),
                                            TextFont {
                                                font_size: FontSize::Px(10.0),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.58, 0.92, 0.72)),
                                        ));
                                    }
                                });
                        }
                    }

                    // ۲. درخواست‌های دوستی ورودی
                    if !state.friends.incoming_requests.is_empty() {
                        scroll.spawn((
                            Text::new(format!(
                                "INCOMING REQUESTS ({})",
                                state.friends.incoming_requests.len()
                            )),
                            TextFont {
                                font_size: FontSize::Px(10.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.82, 0.34)),
                        ));
                        for req in &state.friends.incoming_requests {
                            scroll
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_shrink: 0.0,
                                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(1.0)),
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.18, 0.15, 0.05, 0.90)),
                                    BorderColor::all(Color::srgb(0.92, 0.74, 0.26)),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new(req.username.clone()),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(Color::WHITE),
                                    ));
                                    row.spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        column_gap: Val::Px(4.0),
                                        ..default()
                                    })
                                    .with_children(|btns| {
                                        spawn_sized_button(
                                            btns,
                                            "OK",
                                            PreGameBtnAction::RespondFriendRequest {
                                                request_id: req.request_id,
                                                accept: true,
                                            },
                                            Color::srgb(0.10, 0.46, 0.28),
                                            Color::srgb(0.32, 0.96, 0.62),
                                            36.0,
                                            22.0,
                                        );
                                        spawn_sized_button(
                                            btns,
                                            "NO",
                                            PreGameBtnAction::RespondFriendRequest {
                                                request_id: req.request_id,
                                                accept: false,
                                            },
                                            Color::srgb(0.46, 0.14, 0.14),
                                            Color::srgb(0.94, 0.38, 0.38),
                                            36.0,
                                            22.0,
                                        );
                                    });
                                });
                        }
                    }

                    // ۳. درخواست‌های دوستی ارسالی (با قابلیت لغو درخواست)
                    if !state.friends.outgoing_requests.is_empty() {
                        scroll.spawn((
                            Text::new(format!(
                                "SENT REQUESTS ({})",
                                state.friends.outgoing_requests.len()
                            )),
                            TextFont {
                                font_size: FontSize::Px(10.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.72, 0.86, 0.96)),
                        ));
                        for req in &state.friends.outgoing_requests {
                            scroll
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_shrink: 0.0,
                                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(1.0)),
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.06, 0.14, 0.20, 0.88)),
                                    BorderColor::all(Color::srgba(0.36, 0.64, 0.84, 0.65)),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Text::new(req.username.clone()),
                                        TextFont {
                                            font_size: FontSize::Px(11.5),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.88, 0.94, 0.98)),
                                    ));
                                    spawn_sized_button(
                                        row,
                                        "CANCEL",
                                        PreGameBtnAction::CancelFriendRequest(req.user_id.clone()),
                                        Color::srgb(0.44, 0.18, 0.12),
                                        Color::srgb(0.96, 0.48, 0.34),
                                        62.0,
                                        22.0,
                                    );
                                });
                        }
                    }

                    // ۴. لیست دوستان همراه با وضعیت زنده، دعوت/لغو دعوت پارتی و تایید حذف دوست
                    if state.friends.friends.is_empty() {
                        scroll.spawn((
                            Text::new("No friends yet. Search by username above!"),
                            TextFont {
                                font_size: FontSize::Px(11.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.60, 0.74, 0.74)),
                        ));
                    } else {
                        for f in &state.friends.friends {
                            let dot_col = if !f.online {
                                Color::srgb(0.42, 0.48, 0.50)
                            } else if f.status == "in_party"
                                || f.status == "in_queue"
                                || f.status == "in_match"
                                || f.status == "in_game"
                            {
                                Color::srgb(0.96, 0.78, 0.28)
                            } else {
                                Color::srgb(0.24, 0.96, 0.58)
                            };
                            let already_in_my_party = state
                                .party
                                .as_ref()
                                .map(|p| p.members.iter().any(|m| m.user_id == f.user_id))
                                .unwrap_or(false);
                            let is_inviting = state.pending_sent_invites.contains_key(&f.user_id);

                            scroll
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_shrink: 0.0,
                                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.045, 0.13, 0.155, 0.90)),
                                ))
                                .with_children(|row| {
                                    row.spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(7.0),
                                        ..default()
                                    })
                                    .with_children(|info| {
                                        info.spawn((
                                            Node {
                                                width: Val::Px(8.0),
                                                height: Val::Px(8.0),
                                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                                ..default()
                                            },
                                            BackgroundColor(dot_col),
                                        ));
                                        info.spawn(Node {
                                            flex_direction: FlexDirection::Column,
                                            ..default()
                                        })
                                        .with_children(|col| {
                                            col.spawn((
                                                Text::new(f.username.clone()),
                                                TextFont {
                                                    font_size: FontSize::Px(12.5),
                                                    ..default()
                                                },
                                                TextColor(if f.online {
                                                    Color::WHITE
                                                } else {
                                                    Color::srgb(0.60, 0.68, 0.70)
                                                }),
                                            ));
                                            col.spawn((
                                                Text::new(format!(
                                                    "{} | {} MMR",
                                                    f.status.replace('_', " ").to_uppercase(),
                                                    f.mmr
                                                )),
                                                TextFont {
                                                    font_size: FontSize::Px(9.5),
                                                    ..default()
                                                },
                                                TextColor(dot_col),
                                            ));
                                        });
                                    });

                                    row.spawn(Node {
                                        flex_direction: FlexDirection::Row,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(4.0),
                                        ..default()
                                    })
                                    .with_children(|acts| {
                                        if already_in_my_party {
                                            acts.spawn((
                                                Text::new("IN SQUAD"),
                                                TextFont {
                                                    font_size: FontSize::Px(9.5),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.45, 0.94, 0.72)),
                                            ));
                                        } else if is_inviting {
                                            acts.spawn((
                                                UiButton,
                                                Hovered::default(),
                                                Node {
                                                    width: Val::Px(68.0),
                                                    height: Val::Px(24.0),
                                                    justify_content: JustifyContent::Center,
                                                    align_items: AlignItems::Center,
                                                    border: UiRect::all(Val::Px(1.0)),
                                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(Color::srgba(0.32, 0.24, 0.06, 0.95)),
                                                BorderColor::all(Color::srgb(0.98, 0.80, 0.28)),
                                                PulseGlowNode { speed: 5.0 },
                                                PreGameBtnAction::CancelPartyInvite(f.user_id.clone()),
                                            ))
                                            .with_children(|invb| {
                                                invb.spawn((
                                                    Text::new("CANCEL"),
                                                    TextFont {
                                                        font_size: FontSize::Px(9.5),
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.98, 0.88, 0.45)),
                                                ));
                                            });
                                        } else if f.online && (f.status == "in_match" || f.status == "in_game") {
                                            acts.spawn((
                                                Text::new("IN MATCH"),
                                                TextFont {
                                                    font_size: FontSize::Px(9.5),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.96, 0.78, 0.28)),
                                            ));
                                        } else if f.online && f.status == "in_queue" {
                                            acts.spawn((
                                                Text::new("IN QUEUE"),
                                                TextFont {
                                                    font_size: FontSize::Px(9.5),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.96, 0.78, 0.28)),
                                            ));
                                        } else if f.online && f.in_party {
                                            acts.spawn((
                                                Text::new("IN PARTY"),
                                                TextFont {
                                                    font_size: FontSize::Px(9.5),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.96, 0.78, 0.28)),
                                            ));
                                        } else if f.online {
                                            if party_is_full {
                                                acts.spawn((
                                                    Text::new("FULL"),
                                                    TextFont {
                                                        font_size: FontSize::Px(9.5),
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.68, 0.74, 0.74)),
                                                ));
                                            } else if in_squad && (!is_party_leader || party_is_queuing) {
                                                acts.spawn((
                                                    Text::new(if party_is_queuing { "QUEUING" } else { "ONLINE" }),
                                                    TextFont {
                                                        font_size: FontSize::Px(9.5),
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.54, 0.82, 0.78)),
                                                ));
                                            } else {
                                                spawn_sized_button(
                                                    acts,
                                                    "INVITE",
                                                    PreGameBtnAction::InviteToParty(f.user_id.clone()),
                                                    Color::srgb(0.10, 0.44, 0.34),
                                                    Color::srgb(0.30, 0.96, 0.76),
                                                    56.0,
                                                    24.0,
                                                );
                                            }
                                        }
                                        spawn_sized_button(
                                            acts,
                                            "X",
                                            PreGameBtnAction::RequestRemoveFriendConfirm {
                                                user_id: f.user_id.clone(),
                                                username: f.username.clone(),
                                            },
                                            Color::srgb(0.32, 0.12, 0.14),
                                            Color::srgb(0.76, 0.30, 0.32),
                                            24.0,
                                            24.0,
                                        );
                                    });
                                });
                        }
                    }
                });
        });
}

// =============================================================================
// باکس چت سریع پارتی
// =============================================================================

fn spawn_party_chat_box(
    parent: &mut ChildSpawnerCommands,
    party: &PartyState,
    state: &PreGameSessionState,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.075, 0.09, 0.94)),
            BorderColor::all(Color::srgb(0.24, 0.78, 0.74)),
        ))
        .with_children(|chat| {
            chat.spawn((
                Text::new("PARTY CHAT"),
                TextFont {
                    font_size: FontSize::Px(11.5),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.92, 0.86)),
            ));
            for msg in party.chat_messages.iter().rev().take(4).rev() {
                let col = if msg.is_system {
                    Color::srgb(0.95, 0.82, 0.36)
                } else {
                    Color::srgb(0.88, 0.96, 0.96)
                };
                chat.spawn((
                    Text::new(format!("{}: {}", msg.sender, msg.text)),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(col),
                ));
            }
            chat.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            })
            .with_children(|row| {
                spawn_menu_input(
                    row,
                    "",
                    &state.party_chat_input,
                    "Type party message...",
                    state.focused == FocusedInput::PartyChat,
                    FocusedInput::PartyChat,
                    false,
                );
                spawn_sized_button(
                    row,
                    "SEND",
                    PreGameBtnAction::SendPartyChat,
                    Color::srgb(0.10, 0.40, 0.38),
                    Color::srgb(0.32, 0.94, 0.84),
                    60.0,
                    36.0,
                );
            });
        });
}

// =============================================================================
// پشته نوتیفیکیشن‌های مفهومی و دعوت‌نامه‌های تعاملی پارتی
// =============================================================================

fn spawn_notification_stack(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    parent
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(76.0),
            right: Val::Px(18.0),
            width: Val::Px(332.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|stack| {
            for linv in state.pending_lobby_invites.iter().take(2) {
                stack
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.14, 0.18, 0.96)),
                        BorderColor::all(Color::srgb(0.32, 0.94, 0.84)),
                        PulseGlowNode { speed: 4.0 },
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(format!("LOBBY INVITE // {}", linv.from_username.to_uppercase())),
                            TextFont { font_size: FontSize::Px(12.5), ..default() },
                            TextColor(Color::srgb(0.38, 0.96, 0.86)),
                        ));
                        card.spawn((
                            Text::new(format!("Room: {}", linv.lobby_title)),
                            TextFont { font_size: FontSize::Px(12.0), ..default() },
                            TextColor(Color::srgb(0.90, 0.96, 0.96)),
                        ));
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_sized_button(
                                row,
                                "JOIN LOBBY",
                                PreGameBtnAction::RespondLobbyInvite {
                                    lobby_id: linv.lobby_id.clone(),
                                    accept: true,
                                },
                                Color::srgb(0.10, 0.48, 0.30),
                                Color::srgb(0.32, 0.98, 0.64),
                                116.0,
                                28.0,
                            );
                            spawn_sized_button(
                                row,
                                "DECLINE",
                                PreGameBtnAction::RespondLobbyInvite {
                                    lobby_id: linv.lobby_id.clone(),
                                    accept: false,
                                },
                                Color::srgb(0.46, 0.14, 0.16),
                                Color::srgb(0.96, 0.38, 0.40),
                                104.0,
                                28.0,
                            );
                        });
                    });
            }
            for inv in state.pending_party_invites.iter().take(2) {
                stack
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.04, 0.15, 0.16, 0.96)),
                        BorderColor::all(Color::srgb(0.98, 0.82, 0.32)),
                        PulseGlowNode { speed: 4.0 },
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Text::new(format!("PARTY INVITE // {}", inv.from_username.to_uppercase())),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.98, 0.86, 0.40)),
                        ));
                        card.spawn((
                            Text::new(format!("{} invited you to join their squad.", inv.from_username)),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.88, 0.96, 0.96)),
                        ));
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_sized_button(
                                row,
                                "ACCEPT",
                                PreGameBtnAction::RespondPartyInvite {
                                    invite_id: inv.invite_id.clone(),
                                    accept: true,
                                },
                                Color::srgb(0.10, 0.48, 0.30),
                                Color::srgb(0.32, 0.98, 0.64),
                                110.0,
                                28.0,
                            );
                            spawn_sized_button(
                                row,
                                "DECLINE",
                                PreGameBtnAction::RespondPartyInvite {
                                    invite_id: inv.invite_id.clone(),
                                    accept: false,
                                },
                                Color::srgb(0.46, 0.14, 0.16),
                                Color::srgb(0.96, 0.38, 0.40),
                                110.0,
                                28.0,
                            );
                        });
                    });
            }

            for n in state.notifications.iter().rev().take(3) {
                let (accent, bg) = match n.kind.as_str() {
                    "error" => (
                        Color::srgb(0.96, 0.36, 0.38),
                        Color::srgba(0.24, 0.06, 0.08, 0.95),
                    ),
                    "party_joined" | "friend_accepted" => (
                        Color::srgb(0.28, 0.96, 0.68),
                        Color::srgba(0.04, 0.16, 0.15, 0.95),
                    ),
                    "friend_request" => (
                        Color::srgb(0.98, 0.82, 0.32),
                        Color::srgba(0.18, 0.14, 0.04, 0.95),
                    ),
                    _ => (
                        Color::srgb(0.32, 0.84, 0.96),
                        Color::srgba(0.03, 0.12, 0.16, 0.95),
                    ),
                };
                let pct = ((n.timer / n.max_secs) * 100.0).clamp(0.0, 100.0);

                stack
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(10.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(5.0),
                            border: UiRect::left(Val::Px(4.0)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor::all(accent),
                    ))
                    .with_children(|card| {
                        card.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new(n.title.clone()),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(accent),
                            ));
                            hdr.spawn((
                                UiButton,
                                Hovered::default(),
                                Node {
                                    padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                                BorderColor::all(Color::NONE),
                                PreGameBtnAction::DismissNotification(n.id),
                            ))
                            .with_children(|x| {
                                x.spawn((
                                    Text::new("x"),
                                    TextFont {
                                        font_size: FontSize::Px(11.0),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(0.75, 0.82, 0.84)),
                                ));
                            });
                        });
                        card.spawn((
                            Text::new(n.message.clone()),
                            TextFont {
                                font_size: FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.92, 0.96, 0.96)),
                        ));
                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(3.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.10)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Node {
                                    width: Val::Percent(pct),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(accent),
                            ));
                        });
                    });
            }
        });
}

// =============================================================================
// صفحه جستجوی نبرد
// =============================================================================

fn spawn_finding_match_screen(
    parent: &mut ChildSpawnerCommands,
    state: &PreGameSessionState,
    _config: &GameConfig,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::all(Val::Px(24.0)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::FlexEnd,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|stage| {
            stage
                .spawn((
                    Node {
                        width: Val::Px(440.0),
                        padding: UiRect::axes(Val::Px(28.0), Val::Px(22.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.088, 0.105, 0.93)),
                    BorderColor::all(Color::srgb(0.28, 0.94, 0.80)),
                    PulseGlowNode { speed: 3.6 },
                ))
                .with_children(|card| {
                    let title = if state.server_starting {
                        "LAUNCHING DEDICATED SERVER..."
                    } else {
                        "SEARCHING FOR OPPONENTS"
                    };
                    let col = if state.server_starting {
                        Color::srgb(0.98, 0.84, 0.32)
                    } else {
                        Color::srgb(0.28, 0.96, 0.82)
                    };
                    spawn_conceptual_loader(card, title, col);

                    card.spawn((
                        Text::new(format!(
                            "PLAYERS READY: {} / {}",
                            state.queue_players, state.queue_required
                        )),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.90, 0.98, 0.96)),
                    ));

                    if !state.server_starting {
                        spawn_sized_button(
                            card,
                            "CANCEL QUEUE",
                            PreGameBtnAction::CancelFindMatch,
                            Color::srgb(0.50, 0.14, 0.16),
                            Color::srgb(0.96, 0.40, 0.42),
                            180.0,
                            40.0,
                        );
                    }
                });
        });
}

fn spawn_sized_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: PreGameBtnAction,
    bg: Color,
    border: Color,
    width_px: f32,
    height_px: f32,
) {
    let font_sz = if height_px >= 56.0 {
        17.0
    } else if height_px <= 26.0 {
        10.5
    } else {
        13.5
    };
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                width: Val::Px(width_px),
                height: Val::Px(height_px),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(border),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(font_sz),
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.99, 0.98)),
            ));
        });
}

fn spawn_menu_input(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    placeholder: &str,
    focused: bool,
    focus_target: FocusedInput,
    is_password: bool,
) {
    let shown = if value.is_empty() {
        placeholder.to_string()
    } else if is_password {
        "*".repeat(value.len())
    } else {
        value.to_string()
    };
    let text_col = if value.is_empty() {
        Color::srgb(0.52, 0.66, 0.66)
    } else {
        Color::srgb(0.95, 0.99, 0.98)
    };
    let border_col = if focused {
        Color::srgb(0.30, 0.96, 0.82)
    } else {
        Color::srgba(0.26, 0.56, 0.58, 0.70)
    };
    parent
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|col| {
            if !label.is_empty() {
                col.spawn((
                    Text::new(label),
                    TextFont {
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.68, 0.86, 0.84)),
                ));
            }
            col.spawn((
                UiButton,
                Hovered::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(36.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.06, 0.08, 0.95)),
                BorderColor::all(border_col),
                PreGameBtnAction::FocusField(focus_target),
            ))
            .with_children(|box_n| {
                let cursor_suffix = if focused { "|" } else { "" };
                box_n.spawn((
                    Text::new(format!("{}{}", shown, cursor_suffix)),
                    TextFont {
                        font_size: FontSize::Px(13.0),
                        ..default()
                    },
                    TextColor(text_col),
                ));
            });
        });
}

fn spawn_join_password_modal(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let Some(ref target_lid) = state.join_target_lobby_id else {
        return;
    };
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.03, 0.04, 0.76)),
        ))
        .with_children(|ov| {
            ov.spawn((
                Node {
                    width: Val::Px(420.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.105, 0.98)),
                BorderColor::all(Color::srgb(0.98, 0.82, 0.32)),
            ))
            .with_children(|modal| {
                modal.spawn((
                    Text::new(format!("PRIVATE LOBBY // #{}", target_lid)),
                    TextFont { font_size: FontSize::Px(17.0), ..default() },
                    TextColor(Color::srgb(0.98, 0.86, 0.40)),
                ));
                modal.spawn((
                    Text::new("This custom lobby is password-protected. Enter the password to join."),
                    TextFont { font_size: FontSize::Px(12.5), ..default() },
                    TextColor(Color::srgb(0.80, 0.90, 0.90)),
                ));
                spawn_menu_input(
                    modal,
                    "LOBBY PASSWORD",
                    &state.join_password_input,
                    "Enter password...",
                    state.focused == FocusedInput::JoinPassword,
                    FocusedInput::JoinPassword,
                    true,
                );
                modal
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|btns| {
                        spawn_sized_button(
                            btns,
                            "CANCEL",
                            PreGameBtnAction::CancelJoinModal,
                            Color::srgb(0.12, 0.20, 0.24),
                            Color::srgb(0.42, 0.62, 0.66),
                            175.0,
                            40.0,
                        );
                        spawn_sized_button(
                            btns,
                            "JOIN LOBBY",
                            PreGameBtnAction::ConfirmJoinLobby,
                            Color::srgba(0.66, 0.48, 0.10, 0.96),
                            Color::srgb(1.0, 0.86, 0.36),
                            190.0,
                            40.0,
                        );
                    });
            });
        });
}

fn spawn_lobby_browser_screen(
    parent: &mut ChildSpawnerCommands,
    state: &PreGameSessionState,
    config: &GameConfig,
) {
    let gold = Color::srgb(0.98, 0.82, 0.30);
    let q_lower = state.lobby_search_input.trim().to_ascii_lowercase();
    let filtered: Vec<&LobbySummary> = state
        .lobbies
        .iter()
        .filter(|lb| {
            if state.lobby_hide_full_locked
                && (lb.current_players >= lb.max_players || lb.has_password)
            {
                return false;
            }
            if !q_lower.is_empty() {
                let title_m = lb.title.to_ascii_lowercase().contains(&q_lower);
                let id_m = lb.lobby_id.to_ascii_lowercase().contains(&q_lower);
                let host_m = lb.host_username.to_ascii_lowercase().contains(&q_lower);
                return title_m || id_m || host_m;
            }
            true
        })
        .collect();

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::axes(Val::Px(24.0), Val::Px(16.0)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|wrap| {
            // ۱. نوار هدر CUSTOM LOBBIES + جستجو + فیلتر + REFRESH
            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.102, 0.92)),
                BorderColor::all(Color::srgba(0.26, 0.72, 0.70, 0.65)),
            ))
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new("CUSTOM LOBBIES"),
                    TextFont { font_size: FontSize::Px(20.0), ..default() },
                    TextColor(Color::WHITE),
                ));

                hdr.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(12.0),
                    ..default()
                })
                .with_children(|controls| {
                    controls
                        .spawn(Node {
                            width: Val::Px(250.0),
                            ..default()
                        })
                        .with_children(|sw| {
                            spawn_menu_input(
                                sw,
                                "",
                                &state.lobby_search_input,
                                "Search lobby name or ID...",
                                state.focused == FocusedInput::LobbySearch,
                                FocusedInput::LobbySearch,
                                false,
                            );
                        });

                    spawn_sized_button(
                        controls,
                        if state.lobby_hide_full_locked {
                            "[x] HIDE FULL / LOCKED"
                        } else {
                            "[ ] HIDE FULL / LOCKED"
                        },
                        PreGameBtnAction::ToggleHideFullLobbies,
                        if state.lobby_hide_full_locked {
                            Color::srgb(0.08, 0.30, 0.28)
                        } else {
                            Color::srgba(0.04, 0.12, 0.14, 0.90)
                        },
                        Color::srgba(0.32, 0.74, 0.74, 0.70),
                        168.0,
                        36.0,
                    );

                    spawn_sized_button(
                        controls,
                        "REFRESH",
                        PreGameBtnAction::RefreshLobbies,
                        Color::srgb(0.07, 0.24, 0.26),
                        Color::srgb(0.30, 0.86, 0.80),
                        104.0,
                        36.0,
                    );
                });
            });

            // ۲. جدول لیست لابی‌ها
            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    padding: UiRect::all(Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.070, 0.084, 0.88)),
                BorderColor::all(Color::srgba(0.24, 0.62, 0.60, 0.55)),
            ))
            .with_children(|table| {
                // هدر ستون‌ها
                table
                    .spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.28, 0.68, 0.66, 0.45)),
                    ))
                    .with_children(|th| {
                        for (label, pct) in [
                            ("LOBBY NAME", 28.0),
                            ("HOST", 16.0),
                            ("GAME MODE", 20.0),
                            ("MAP", 18.0),
                            ("PLAYERS", 10.0),
                            ("STATUS", 8.0),
                        ] {
                            th.spawn(Node {
                                width: Val::Percent(pct),
                                ..default()
                            })
                            .with_children(|c| {
                                c.spawn((
                                    Text::new(label),
                                    TextFont { font_size: FontSize::Px(11.5), ..default() },
                                    TextColor(Color::srgb(0.62, 0.82, 0.80)),
                                ));
                            });
                        }
                    });

                if filtered.is_empty() {
                    table
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|empty| {
                            empty.spawn((
                                Text::new("No custom lobbies available. Click CREATE LOBBY below to host a match!"),
                                TextFont { font_size: FontSize::Px(14.0), ..default() },
                                TextColor(Color::srgb(0.68, 0.82, 0.82)),
                            ));
                        });
                } else {
                    table
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollPosition::default(),
                            RelativeCursorPosition::default(),
                            PreGameScrollableList,
                        ))
                        .with_children(|rows_scroll| {
                    for lb in &filtered {
                        let is_full = lb.current_players >= lb.max_players;
                        let is_selected = state
                            .selected_lobby_id
                            .as_ref()
                            .is_some_and(|s| s == &lb.lobby_id);
                        let half = (lb.max_players / 2).max(1);

                        let (row_bg, row_border) = if is_selected {
                            (
                                Color::srgba(0.18, 0.16, 0.06, 0.92),
                                gold,
                            )
                        } else if is_full {
                            (
                                Color::srgba(0.03, 0.08, 0.10, 0.72),
                                Color::srgba(0.22, 0.36, 0.38, 0.40),
                            )
                        } else {
                            (
                                Color::srgba(0.035, 0.11, 0.13, 0.88),
                                Color::srgba(0.24, 0.56, 0.56, 0.45),
                            )
                        };

                        let txt_col = if is_selected {
                            gold
                        } else if is_full {
                            Color::srgb(0.55, 0.64, 0.66)
                        } else {
                            Color::srgb(0.92, 0.97, 0.96)
                        };

                        rows_scroll
                            .spawn((
                                UiButton,
                                Hovered::default(),
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(46.0),
                                    flex_shrink: 0.0,
                                    padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(if is_selected { 1.5 } else { 1.0 })),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(row_bg),
                                BorderColor::all(row_border),
                                PreGameBtnAction::SelectLobbyRow(lb.lobby_id.clone()),
                            ))
                            .with_children(|row| {
                                let lock_tag = if lb.has_password { " [LOCK]" } else { "" };
                                let cells = [
                                    (format!("{}{}", lb.title, lock_tag), 28.0, txt_col),
                                    (lb.host_username.clone(), 16.0, txt_col),
                                    (format!("{}v{} JUNGLE SKIRMISH", half, half), 20.0, txt_col),
                                    (config.main_menu.default_map_name.clone(), 18.0, txt_col),
                                    (
                                        format!("{} / {}", lb.current_players, lb.max_players),
                                        10.0,
                                        txt_col,
                                    ),
                                    (
                                        if is_full { "FULL" } else { "OPEN" }.to_string(),
                                        8.0,
                                        if is_full {
                                            Color::srgb(0.96, 0.38, 0.40)
                                        } else {
                                            Color::srgb(0.32, 0.96, 0.58)
                                        },
                                    ),
                                ];
                                for (val, pct, col) in cells {
                                    row.spawn(Node {
                                        width: Val::Percent(pct),
                                        ..default()
                                    })
                                    .with_children(|c| {
                                        c.spawn((
                                            Text::new(val),
                                            TextFont { font_size: FontSize::Px(13.0), ..default() },
                                            TextColor(col),
                                        ));
                                    });
                                }
                            });
                    }
                    });
                }
            });

            // ۳. نوار پایینی: BACK TO MENU | CREATE LOBBY + JOIN LOBBY
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|bottom| {
                spawn_sized_button(
                    bottom,
                    "BACK TO MENU",
                    PreGameBtnAction::SwitchTab(PreGameTab::Hub),
                    Color::srgba(0.05, 0.14, 0.16, 0.92),
                    Color::srgba(0.36, 0.62, 0.64, 0.75),
                    190.0,
                    54.0,
                );

                bottom
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|right| {
                        spawn_sized_button(
                            right,
                            "CREATE LOBBY",
                            PreGameBtnAction::OpenCreateLobbyModal,
                            Color::srgba(0.04, 0.24, 0.22, 0.92),
                            Color::srgb(0.24, 0.92, 0.76),
                            210.0,
                            56.0,
                        );
                        spawn_sized_button(
                            right,
                            "JOIN LOBBY",
                            PreGameBtnAction::JoinSelectedLobby,
                            Color::srgba(0.68, 0.48, 0.10, 0.96),
                            Color::srgb(1.0, 0.86, 0.36),
                            210.0,
                            56.0,
                        );
                    });
            });
        });
}

fn spawn_create_or_edit_lobby_modal(
    parent: &mut ChildSpawnerCommands,
    state: &PreGameSessionState,
    config: &GameConfig,
) {
    let is_edit = state.edit_lobby_settings_open;
    let title_txt = if is_edit { "LOBBY SETTINGS" } else { "CREATE CUSTOM LOBBY" };
    let submit_txt = if is_edit { "SAVE SETTINGS" } else { "CREATE LOBBY" };
    let submit_act = if is_edit {
        PreGameBtnAction::SaveLobbySettings
    } else {
        PreGameBtnAction::CreateLobby
    };
    let half = state.create_capacity / 2;

    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.03, 0.04, 0.72)),
        ))
        .with_children(|ov| {
            ov.spawn((
                Node {
                    width: Val::Px(460.0),
                    padding: UiRect::all(Val::Px(24.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.105, 0.97)),
                BorderColor::all(Color::srgb(0.24, 0.76, 0.72)),
            ))
            .with_children(|modal| {
                // Header + X close
                modal
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::bottom(Val::Px(8.0)),
                        border: UiRect::bottom(Val::Px(1.0)),
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn((
                            Text::new(title_txt),
                            TextFont { font_size: FontSize::Px(20.0), ..default() },
                            TextColor(Color::srgb(0.95, 0.99, 0.98)),
                        ));
                        spawn_sized_button(
                            hdr,
                            "X",
                            PreGameBtnAction::CloseLobbyModal,
                            Color::srgba(0.10, 0.20, 0.24, 0.90),
                            Color::srgb(0.40, 0.72, 0.72),
                            30.0,
                            28.0,
                        );
                    });

                // LOBBY NAME
                spawn_menu_input(
                    modal,
                    "LOBBY NAME",
                    &state.create_title,
                    "Enter room name...",
                    state.focused == FocusedInput::LobbyTitle,
                    FocusedInput::LobbyTitle,
                    false,
                );

                // LOBBY PASSWORD (Optional) with SHOW/HIDE toggle
                modal
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexEnd,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|pw_row| {
                        spawn_menu_input(
                            pw_row,
                            "LOBBY PASSWORD (Optional)",
                            &state.create_password,
                            "Leave empty for open lobby",
                            state.focused == FocusedInput::LobbyPassword,
                            FocusedInput::LobbyPassword,
                            !state.create_password_visible,
                        );
                        spawn_sized_button(
                            pw_row,
                            if state.create_password_visible { "HIDE" } else { "SHOW" },
                            PreGameBtnAction::ToggleCreatePasswordVisibility,
                            Color::srgb(0.06, 0.18, 0.22),
                            Color::srgb(0.28, 0.74, 0.74),
                            62.0,
                            36.0,
                        );
                    });

                // FORMAT / CAPACITY
                modal
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|cap_col| {
                        cap_col.spawn((
                            Text::new(format!("FORMAT ({}v{} JUNGLE SKIRMISH)", half, half)),
                            TextFont { font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgb(0.68, 0.86, 0.84)),
                        ));
                        cap_col
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                column_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|row| {
                                for &cap in &config.main_menu.lobby_capacities {
                                    let sel = state.create_capacity == cap;
                                    let h = cap / 2;
                                    spawn_sized_button(
                                        row,
                                        &format!("{}v{}", h, h),
                                        PreGameBtnAction::SelectLobbyCapacity(cap),
                                        if sel {
                                            Color::srgb(0.09, 0.42, 0.36)
                                        } else {
                                            Color::srgb(0.04, 0.12, 0.15)
                                        },
                                        if sel {
                                            Color::srgb(0.32, 0.96, 0.82)
                                        } else {
                                            Color::srgba(0.24, 0.50, 0.52, 0.65)
                                        },
                                        76.0,
                                        34.0,
                                    );
                                }
                            });
                    });

                // MAP
                modal
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|mcol| {
                        mcol.spawn((
                            Text::new("MAP"),
                            TextFont { font_size: FontSize::Px(11.0), ..default() },
                            TextColor(Color::srgb(0.68, 0.86, 0.84)),
                        ));
                        mcol.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(36.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.07, 0.09, 0.92)),
                            BorderColor::all(Color::srgba(0.24, 0.56, 0.58, 0.65)),
                        ))
                        .with_children(|b| {
                            b.spawn((
                                Text::new(config.main_menu.default_map_name.clone()),
                                TextFont { font_size: FontSize::Px(13.0), ..default() },
                                TextColor(Color::srgb(0.88, 0.96, 0.95)),
                            ));
                        });
                    });

                // Footer: CANCEL + CREATE LOBBY
                modal
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(12.0),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    })
                    .with_children(|btns| {
                        spawn_sized_button(
                            btns,
                            "CANCEL",
                            PreGameBtnAction::CloseLobbyModal,
                            Color::srgba(0.06, 0.16, 0.19, 0.94),
                            Color::srgb(0.32, 0.58, 0.62),
                            198.0,
                            44.0,
                        );
                        spawn_sized_button(
                            btns,
                            submit_txt,
                            submit_act,
                            Color::srgba(0.66, 0.48, 0.10, 0.96),
                            Color::srgb(1.0, 0.86, 0.36),
                            198.0,
                            44.0,
                        );
                    });
            });
        });
}

fn spawn_lobby_invite_modal(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let online_friends: Vec<_> = state
        .friends
        .friends
        .iter()
        .filter(|f| f.online)
        .collect();
    parent
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.03, 0.04, 0.72)),
        ))
        .with_children(|ov| {
            ov.spawn((
                Node {
                    width: Val::Px(380.0),
                    padding: UiRect::all(Val::Px(20.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.105, 0.97)),
                BorderColor::all(Color::srgb(0.28, 0.88, 0.78)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("INVITE FRIEND TO LOBBY"),
                        TextFont { font_size: FontSize::Px(15.0), ..default() },
                        TextColor(Color::srgb(0.98, 0.86, 0.40)),
                    ));
                    spawn_sized_button(
                        hdr,
                        "X",
                        PreGameBtnAction::CloseLobbyInviteModal,
                        Color::srgb(0.40, 0.12, 0.14),
                        Color::srgb(0.92, 0.36, 0.38),
                        28.0,
                        26.0,
                    );
                });

                if online_friends.is_empty() {
                    card.spawn((
                        Text::new("No online friends available to invite right now."),
                        TextFont { font_size: FontSize::Px(12.5), ..default() },
                        TextColor(Color::srgb(0.68, 0.80, 0.80)),
                    ));
                } else {
                    for f in online_friends.iter().take(7) {
                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.14, 0.17, 0.92)),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new(format!("{} ({} MMR)", f.username, f.mmr)),
                                TextFont { font_size: FontSize::Px(13.0), ..default() },
                                TextColor(Color::WHITE),
                            ));
                            spawn_sized_button(
                                row,
                                "INVITE",
                                PreGameBtnAction::SendLobbyInvite(f.user_id.clone()),
                                Color::srgb(0.10, 0.44, 0.34),
                                Color::srgb(0.30, 0.96, 0.76),
                                72.0,
                                26.0,
                            );
                        });
                    }
                }
            });
        });
}

fn spawn_lobby_room_screen(
    parent: &mut ChildSpawnerCommands,
    state: &PreGameSessionState,
    config: &GameConfig,
) {
    let Some(ref lobby) = state.current_lobby else {
        return;
    };
    let my_uid = state
        .profile
        .as_ref()
        .map(|p| p.user_id.as_str())
        .unwrap_or("");
    let is_host = lobby.host_user_id == my_uid;
    let alpha_col = Color::srgb(0.24, 0.92, 0.74);
    let omega_col = Color::srgb(0.96, 0.68, 0.24);
    let gold = Color::srgb(0.98, 0.82, 0.30);
    let half = (lobby.max_players / 2).max(1);

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            padding: UiRect::axes(Val::Px(22.0), Val::Px(14.0)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|wrap| {
            // ۱. نوار هدر اتاق لابی (دقیقاً مطابق طرح مرجع)
            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.102, 0.92)),
                BorderColor::all(Color::srgba(0.26, 0.72, 0.70, 0.65)),
            ))
            .with_children(|hdr| {
                // عنوان + وضعیت قفل
                hdr.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|title_grp| {
                    title_grp.spawn((
                        Text::new(lobby.title.clone()),
                        TextFont { font_size: FontSize::Px(19.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    if lobby.has_password {
                        title_grp.spawn((
                            Text::new("[PRIVATE]"),
                            TextFont { font_size: FontSize::Px(11.5), ..default() },
                            TextColor(gold),
                        ));
                    }
                });

                // متادیتای وسط: LOBBY ID | GAME MODE | MAP
                hdr.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(28.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|meta| {
                    for (k, v) in [
                        ("LOBBY ID", format!("#{}", lobby.lobby_id)),
                        ("GAME MODE", format!("{}v{} JUNGLE SKIRMISH", half, half)),
                        ("MAP", config.main_menu.default_map_name.clone()),
                    ] {
                        meta.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|col| {
                            col.spawn((
                                Text::new(k),
                                TextFont { font_size: FontSize::Px(10.0), ..default() },
                                TextColor(Color::srgb(0.60, 0.78, 0.76)),
                            ));
                            col.spawn((
                                Text::new(v),
                                TextFont { font_size: FontSize::Px(13.0), ..default() },
                                TextColor(Color::srgb(0.94, 0.98, 0.98)),
                            ));
                        });
                    }
                });

                spawn_sized_button(
                    hdr,
                    "LEAVE LOBBY",
                    PreGameBtnAction::LeaveLobby,
                    Color::srgba(0.46, 0.12, 0.14, 0.94),
                    Color::srgb(0.94, 0.36, 0.38),
                    132.0,
                    36.0,
                );
            });

            // ۲. بخش میانی: TEAM ALPHA  <-- VS + SWAP TEAMS -->  TEAM OMEGA
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(22.0),
                ..default()
            })
            .with_children(|arena| {
                spawn_team_slots_panel(
                    arena,
                    "TEAM ALPHA",
                    1,
                    half,
                    lobby,
                    is_host,
                    my_uid,
                    alpha_col,
                );

                // ستون وسط: نشان VS + دکمه SWAP TEAMS
                arena
                    .spawn(Node {
                        width: Val::Px(116.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        ..default()
                    })
                    .with_children(|mid| {
                        mid.spawn((
                            Node {
                                width: Val::Px(74.0),
                                height: Val::Px(74.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(2.5)),
                                border_radius: BorderRadius::all(Val::Px(12.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.12, 0.14, 0.94)),
                            BorderColor::all(gold),
                        ))
                        .with_children(|vs| {
                            vs.spawn((
                                Text::new("VS"),
                                TextFont { font_size: FontSize::Px(28.0), ..default() },
                                TextColor(Color::srgb(0.98, 0.94, 0.82)),
                            ));
                        });
                        spawn_sized_button(
                            mid,
                            "SWAP TEAMS",
                            PreGameBtnAction::SwapLobbyTeam,
                            Color::srgba(0.05, 0.18, 0.20, 0.92),
                            Color::srgb(0.32, 0.84, 0.82),
                            114.0,
                            38.0,
                        );
                    });

                spawn_team_slots_panel(
                    arena,
                    "TEAM OMEGA",
                    2,
                    half,
                    lobby,
                    is_host,
                    my_uid,
                    omega_col,
                );
            });

            // ۳. بخش پایینی: چپ (LOBBY CHAT) | راست (LOBBY SETTINGS + START MATCH)
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|bottom| {
                // LOBBY CHAT
                bottom
                    .spawn((
                        Node {
                            width: Val::Px(440.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.025, 0.085, 0.10, 0.92)),
                        BorderColor::all(Color::srgba(0.26, 0.72, 0.70, 0.60)),
                    ))
                    .with_children(|chat| {
                        chat.spawn((
                            Text::new("LOBBY CHAT"),
                            TextFont { font_size: FontSize::Px(12.5), ..default() },
                            TextColor(Color::srgb(0.84, 0.95, 0.94)),
                        ));
                        let start = lobby.chat_messages.len().saturating_sub(5);
                        for m in &lobby.chat_messages[start..] {
                            let col = if m.is_system {
                                gold
                            } else {
                                Color::srgb(0.88, 0.96, 0.96)
                            };
                            chat.spawn((
                                Text::new(format!("{}: {}", m.sender, m.text)),
                                TextFont { font_size: FontSize::Px(11.5), ..default() },
                                TextColor(col),
                            ));
                        }
                        chat.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::FlexEnd,
                            column_gap: Val::Px(8.0),
                            margin: UiRect::top(Val::Px(4.0)),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_menu_input(
                                row,
                                "",
                                &state.lobby_chat_input,
                                "Type a message...",
                                state.focused == FocusedInput::LobbyChat,
                                FocusedInput::LobbyChat,
                                false,
                            );
                            spawn_sized_button(
                                row,
                                "SEND",
                                PreGameBtnAction::SendLobbyChat,
                                Color::srgb(0.09, 0.34, 0.36),
                                Color::srgb(0.32, 0.88, 0.84),
                                72.0,
                                36.0,
                            );
                        });
                    });

                // راست: LOBBY SETTINGS + START MATCH
                bottom
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::FlexEnd,
                        column_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|btns| {
                        if state.server_starting || lobby.is_starting {
                            spawn_conceptual_loader(btns, "STARTING SERVER", gold);
                        } else if is_host {
                            spawn_sized_button(
                                btns,
                                "LOBBY SETTINGS",
                                PreGameBtnAction::OpenLobbySettingsModal,
                                Color::srgba(0.04, 0.22, 0.20, 0.92),
                                Color::srgb(0.24, 0.92, 0.76),
                                200.0,
                                58.0,
                            );
                            spawn_sized_button(
                                btns,
                                "START MATCH",
                                PreGameBtnAction::StartLobbyMatch,
                                Color::srgba(0.68, 0.48, 0.10, 0.96),
                                Color::srgb(1.0, 0.86, 0.36),
                                232.0,
                                58.0,
                            );
                        } else {
                            btns.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(22.0), Val::Px(16.0)),
                                    border: UiRect::all(Val::Px(1.5)),
                                    border_radius: BorderRadius::all(Val::Px(5.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.03, 0.10, 0.12, 0.90)),
                                BorderColor::all(Color::srgba(0.32, 0.76, 0.74, 0.65)),
                            ))
                            .with_children(|w| {
                                w.spawn((
                                    Text::new("WAITING FOR HOST TO START..."),
                                    TextFont { font_size: FontSize::Px(14.0), ..default() },
                                    TextColor(gold),
                                ));
                            });
                        }
                    });
            });
        });
}

fn spawn_team_slots_panel(
    parent: &mut ChildSpawnerCommands,
    team_name: &str,
    team_id: i32,
    slots_per_team: usize,
    lobby: &LobbyDetail,
    is_host: bool,
    my_uid: &str,
    accent: Color,
) {
    parent
        .spawn((
            Node {
                width: Val::Px(400.0),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.082, 0.098, 0.90)),
            BorderColor::all(accent.with_alpha(0.70)),
        ))
        .with_children(|panel| {
            // هدر تیم
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        border: UiRect::bottom(Val::Px(1.5)),
                        ..default()
                    },
                    BorderColor::all(accent.with_alpha(0.60)),
                ))
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new(team_name),
                        TextFont { font_size: FontSize::Px(15.0), ..default() },
                        TextColor(accent),
                    ));
                    hdr.spawn((
                        Text::new(format!("{}v{}", slots_per_team, slots_per_team)),
                        TextFont { font_size: FontSize::Px(13.0), ..default() },
                        TextColor(Color::srgb(0.84, 0.92, 0.92)),
                    ));
                });

            // اسلات‌های بازیکنان
            for slot_idx in 0..slots_per_team {
                let occupant = lobby
                    .members
                    .iter()
                    .find(|m| m.team_id == team_id && m.slot_index == slot_idx);

                if let Some(m) = occupant {
                    let is_me = m.user_id == my_uid;
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(46.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.5)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.16, 0.18, 0.94)),
                            BorderColor::all(if is_me {
                                Color::srgb(0.32, 0.96, 0.82)
                            } else {
                                accent.with_alpha(0.75)
                            }),
                        ))
                        .with_children(|row| {
                            row.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(8.0),
                                ..default()
                            })
                            .with_children(|left| {
                                let tag = if m.is_host { "HOST" } else { "PLR" };
                                left.spawn((
                                    Text::new(tag),
                                    TextFont { font_size: FontSize::Px(10.5), ..default() },
                                    TextColor(if m.is_host {
                                        Color::srgb(0.98, 0.84, 0.34)
                                    } else {
                                        Color::srgb(0.65, 0.82, 0.82)
                                    }),
                                ));
                                left.spawn((
                                    Text::new(m.username.clone()),
                                    TextFont { font_size: FontSize::Px(13.5), ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });

                            row.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|right| {
                                right
                                    .spawn((
                                        Node {
                                            padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            border_radius: BorderRadius::all(Val::Px(3.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.08, 0.36, 0.22, 0.92)),
                                        BorderColor::all(Color::srgb(0.28, 0.94, 0.60)),
                                    ))
                                    .with_children(|rdy| {
                                        rdy.spawn((
                                            Text::new("READY"),
                                            TextFont { font_size: FontSize::Px(10.5), ..default() },
                                            TextColor(Color::srgb(0.76, 1.0, 0.84)),
                                        ));
                                    });
                                if is_host && !is_me {
                                    spawn_sized_button(
                                        right,
                                        "KICK",
                                        PreGameBtnAction::KickLobbyPlayer(m.user_id.clone()),
                                        Color::srgb(0.48, 0.12, 0.14),
                                        Color::srgb(0.94, 0.36, 0.38),
                                        48.0,
                                        24.0,
                                    );
                                }
                            });
                        });
                } else {
                    // اسلات خالی: + JOIN SLOT و دکمه INVITE
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(46.0),
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.03, 0.09, 0.11, 0.84)),
                            BorderColor::all(Color::srgba(0.24, 0.48, 0.50, 0.45)),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                UiButton,
                                Hovered::default(),
                                Node {
                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                                BorderColor::all(Color::NONE),
                                PreGameBtnAction::SwitchLobbySlot {
                                    team_id,
                                    slot_index: slot_idx,
                                },
                            ))
                            .with_children(|jb| {
                                jb.spawn((
                                    Text::new("+ JOIN SLOT"),
                                    TextFont { font_size: FontSize::Px(12.5), ..default() },
                                    TextColor(Color::srgb(0.74, 0.88, 0.88)),
                                ));
                            });
                            spawn_sized_button(
                                row,
                                "INVITE",
                                PreGameBtnAction::OpenLobbyInviteModal,
                                Color::srgba(0.07, 0.22, 0.25, 0.92),
                                Color::srgb(0.28, 0.72, 0.74),
                                76.0,
                                28.0,
                            );
                        });
                }
            }
        });
}

fn resolve_hero_def<'a>(config: &'a GameConfig, hero_id: i32) -> Option<&'a HeroDef> {
    if hero_id > 0 {
        config.heroes.get((hero_id - 1) as usize)
    } else {
        None
    }
}

fn resolve_hero_display_name(config: &GameConfig, hero_id: i32) -> String {
    match resolve_hero_def(config, hero_id) {
        Some(h) => h.display_name.to_uppercase(),
        None => "UNASSIGNED".to_string(),
    }
}

fn compute_kda_ratio(kills: i32, deaths: i32, assists: i32) -> f32 {
    let denom = deaths.max(1) as f32;
    (kills + assists) as f32 / denom
}

#[allow(clippy::too_many_arguments)]
fn spawn_hero_portrait_frame(
    parent: &mut ChildSpawnerCommands,
    hero: Option<&HeroDef>,
    portraits: &MenuHeroPortraitCache,
    size_px: f32,
    border_radius_px: f32,
    border_color: Color,
    border_width_px: f32,
    level_badge: Option<i32>,
) {
    let clamped_radius = border_radius_px.min(size_px * 0.5);
    let inner_radius = (clamped_radius - border_width_px).max(0.0);

    parent
        .spawn((
            Node {
                width: Val::Px(size_px),
                height: Val::Px(size_px),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(border_width_px)),
                border_radius: BorderRadius::all(Val::Px(clamped_radius)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.06, 0.08, 0.96)),
            BorderColor::all(border_color),
        ))
        .with_children(|frame| {
            if let Some(h) = hero {
                let handle = portraits
                    .handles
                    .get(&h.portrait_path)
                    .unwrap_or_else(|| {
                        panic!(
                            "Hero portrait handle missing in MenuHeroPortraitCache for '{}' ({})",
                            h.id, h.portrait_path
                        )
                    })
                    .clone();
                frame.spawn((
                    ImageNode::new(handle),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        border_radius: BorderRadius::all(Val::Px(inner_radius)),
                        ..default()
                    },
                ));
            } else {
                frame.spawn((
                    Text::new("?"),
                    TextFont {
                        font_size: FontSize::Px((size_px * 0.44).clamp(11.0, 28.0)),
                        ..default()
                    },
                    TextColor(Color::srgb(0.56, 0.72, 0.74)),
                ));
            }

            if let Some(lvl) = level_badge {
                if lvl > 0 {
                    frame
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                right: Val::Px(2.0),
                                bottom: Val::Px(2.0),
                                padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.02, 0.05, 0.07, 0.92)),
                            BorderColor::all(border_color.with_alpha(0.85)),
                        ))
                        .with_children(|badge| {
                            badge.spawn((
                                Text::new(format!("L{}", lvl)),
                                TextFont {
                                    font_size: FontSize::Px(9.0),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.98, 0.88, 0.42)),
                            ));
                        });
                }
            }
        });
}

fn spawn_kda_colored_row(
    parent: &mut ChildSpawnerCommands,
    kills: i32,
    deaths: i32,
    assists: i32,
    font_size: f32,
) {
    let emerald = Color::srgb(0.32, 0.96, 0.62);
    let crimson = Color::srgb(0.98, 0.42, 0.42);
    let teal = Color::srgb(0.42, 0.90, 0.96);
    let slash = Color::srgb(0.52, 0.66, 0.68);

    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Text::new(format!("{}", kills)),
                TextFont { font_size: FontSize::Px(font_size), ..default() },
                TextColor(emerald),
            ));
            r.spawn((
                Text::new("/"),
                TextFont { font_size: FontSize::Px(font_size - 1.0), ..default() },
                TextColor(slash),
            ));
            r.spawn((
                Text::new(format!("{}", deaths)),
                TextFont { font_size: FontSize::Px(font_size), ..default() },
                TextColor(crimson),
            ));
            r.spawn((
                Text::new("/"),
                TextFont { font_size: FontSize::Px(font_size - 1.0), ..default() },
                TextColor(slash),
            ));
            r.spawn((
                Text::new(format!("{}", assists)),
                TextFont { font_size: FontSize::Px(font_size), ..default() },
                TextColor(teal),
            ));
        });
}

#[allow(clippy::too_many_arguments)]
fn spawn_inventory_slots_strip(
    parent: &mut ChildSpawnerCommands,
    items_config: &ItemsConfig,
    item_icons: &MenuItemIconCache,
    final_items: &[MatchHistoryItemSlot],
    slot_size_px: f32,
    border_radius_px: f32,
    gap_px: f32,
    show_slot_numbers: bool,
) {
    let slot_count = items_config.max_inventory_slots;
    let clamped_rad = border_radius_px.min(slot_size_px * 0.4);
    let inner_rad = (clamped_rad - 1.2).max(0.0);

    parent
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(if show_slot_numbers { 5.0 } else { 3.0 })),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap_px),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(clamped_rad + 2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.044, 0.058, 0.92)),
            BorderColor::all(Color::srgba(0.22, 0.46, 0.50, 0.50)),
        ))
        .with_children(|tray| {
            for idx in 0..slot_count {
                let entry = final_items.iter().find(|it| it.slot == idx);
                if let Some(slot_entry) = entry {
                    if parse_recipe_target(&slot_entry.item_id).is_some() {
                        tray.spawn((
                            Node {
                                width: Val::Px(slot_size_px),
                                height: Val::Px(slot_size_px),
                                flex_shrink: 0.0,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.3)),
                                border_radius: BorderRadius::all(Val::Px(clamped_rad)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.24, 0.18, 0.06, 0.95)),
                            BorderColor::all(Color::srgb(0.95, 0.78, 0.30)),
                        ))
                        .with_children(|slot| {
                            slot.spawn((
                                Text::new("REC"),
                                TextFont {
                                    font_size: FontSize::Px((slot_size_px * 0.28).clamp(7.5, 11.5)),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.88, 0.44)),
                            ));
                        });
                        continue;
                    }

                    if let Some(item) = items_config.get_item(&slot_entry.item_id) {
                        let c = item.icon_color_rgb;
                        let bg = Color::srgb(c.0 * 0.24, c.1 * 0.24, c.2 * 0.24);
                        let bdr = Color::srgb(
                            (c.0 * 0.85 + 0.15).min(1.0),
                            (c.1 * 0.85 + 0.15).min(1.0),
                            (c.2 * 0.85 + 0.15).min(1.0),
                        );
                        tray.spawn((
                            Node {
                                width: Val::Px(slot_size_px),
                                height: Val::Px(slot_size_px),
                                flex_shrink: 0.0,
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.3)),
                                border_radius: BorderRadius::all(Val::Px(clamped_rad)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(bg),
                            BorderColor::all(bdr),
                        ))
                        .with_children(|slot| {
                            if let Some(icon_h) = item_icons.handles.get(item.icon_path.trim()) {
                                slot.spawn((
                                    ImageNode::new(icon_h.clone()),
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        border_radius: BorderRadius::all(Val::Px(inner_rad)),
                                        ..default()
                                    },
                                ));
                            } else {
                                slot.spawn((
                                    Text::new(&item.short_code),
                                    TextFont {
                                        font_size: FontSize::Px(
                                            (slot_size_px * 0.30).clamp(8.0, 12.0),
                                        ),
                                        ..default()
                                    },
                                    TextColor(Color::srgb(
                                        (c.0 + 0.35).min(1.0),
                                        (c.1 + 0.35).min(1.0),
                                        (c.2 + 0.35).min(1.0),
                                    )),
                                ));
                            }

                            if show_slot_numbers {
                                slot.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        top: Val::Px(1.0),
                                        left: Val::Px(2.0),
                                        padding: UiRect::axes(Val::Px(3.0), Val::Px(0.5)),
                                        border_radius: BorderRadius::all(Val::Px(2.5)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.86)),
                                ))
                                .with_children(|kb| {
                                    kb.spawn((
                                        Text::new(format!("{}", idx + 1)),
                                        TextFont {
                                            font_size: FontSize::Px(8.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.95, 0.85, 0.40)),
                                    ));
                                });
                            }
                        });
                        continue;
                    }
                }

                // اسلات خالی مشابه طراحی اینونتوری داخل بازی
                tray.spawn((
                    Node {
                        width: Val::Px(slot_size_px),
                        height: Val::Px(slot_size_px),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(clamped_rad)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.055, 0.080, 0.115, 0.82)),
                    BorderColor::all(Color::srgba(0.24, 0.32, 0.40, 0.48)),
                ))
                .with_children(|empty| {
                    empty.spawn((
                        Text::new(format!("{}", idx + 1)),
                        TextFont {
                            font_size: FontSize::Px((slot_size_px * 0.30).clamp(8.0, 10.5)),
                            ..default()
                        },
                        TextColor(Color::srgba(0.55, 0.68, 0.78, 0.42)),
                    ));
                });
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn spawn_history_team_block(
    parent: &mut ChildSpawnerCommands,
    config: &GameConfig,
    items_config: &ItemsConfig,
    hero_portraits: &MenuHeroPortraitCache,
    item_icons: &MenuItemIconCache,
    team_name: &str,
    team_color: Color,
    is_winner: bool,
    players: &[&crate::backend_client::MatchHistoryPlayerRow],
    party_tags: &HashMap<String, usize>,
) {
    let gold = Color::srgb(0.98, 0.82, 0.30);
    let mm = &config.main_menu;
    let icon_sz = mm.history_player_row_icon_size_px;
    let icon_rad = mm.hero_icon_border_radius_px;

    let total_kills: i32 = players.iter().map(|p| p.kills).sum();
    let total_gold: i32 = players.iter().map(|p| p.gold_coins).sum();

    // هدر تیم
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                border: UiRect {
                    left: Val::Px(3.5),
                    bottom: Val::Px(1.0),
                    ..default()
                },
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(team_color.with_alpha(0.14)),
            BorderColor::all(team_color.with_alpha(0.75)),
        ))
        .with_children(|hdr| {
            hdr.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|left| {
                left.spawn((
                    Text::new(team_name),
                    TextFont { font_size: FontSize::Px(11.5), ..default() },
                    TextColor(team_color),
                ));
                if is_winner {
                    left.spawn((
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(1.5)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(gold.with_alpha(0.20)),
                        BorderColor::all(gold.with_alpha(0.85)),
                    ))
                    .with_children(|w| {
                        w.spawn((
                            Text::new("WINNER"),
                            TextFont { font_size: FontSize::Px(9.0), ..default() },
                            TextColor(gold),
                        ));
                    });
                }
            });

            hdr.spawn((
                Text::new(format!("{} KILLS  |  {}G", total_kills, total_gold)),
                TextFont { font_size: FontSize::Px(10.5), ..default() },
                TextColor(Color::srgb(0.84, 0.92, 0.92)),
            ));
        });

    if players.is_empty() {
        parent.spawn((
            Text::new("No players recorded for this team."),
            TextFont { font_size: FontSize::Px(10.5), ..default() },
            TextColor(Color::srgb(0.56, 0.70, 0.72)),
        ));
        return;
    }

    for p in players {
        let hero_def = resolve_hero_def(config, p.hero_id);
        let hero_lbl = resolve_hero_display_name(config, p.hero_id);
        let p_tag = if !p.party_group_id.is_empty() {
            format!(" [P{}]", party_tags.get(&p.party_group_id).copied().unwrap_or(1))
        } else {
            String::new()
        };
        let you_tag = if p.is_me { " [YOU]" } else { "" };
        let frame_border = if p.is_me {
            gold
        } else {
            team_color.with_alpha(0.80)
        };

        parent
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_shrink: 0.0,
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(5.0),
                    border: UiRect::all(Val::Px(if p.is_me { 1.2 } else { 1.0 })),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(if p.is_me {
                    Color::srgba(0.11, 0.25, 0.25, 0.94)
                } else {
                    Color::srgba(0.032, 0.095, 0.115, 0.84)
                }),
                BorderColor::all(if p.is_me {
                    gold.with_alpha(0.78)
                } else {
                    Color::srgba(0.22, 0.48, 0.50, 0.35)
                }),
            ))
            .with_children(|prow| {
                // ردیف بالا: آیکون هیرو + نام بازیکن/هیرو | آمار K/D/A + CS + Gold
                prow.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|top_row| {
                    top_row
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|left| {
                            spawn_hero_portrait_frame(
                                left,
                                hero_def,
                                hero_portraits,
                                icon_sz,
                                icon_rad,
                                frame_border,
                                1.5,
                                None,
                            );
                            left.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(1.0),
                                ..default()
                            })
                            .with_children(|info| {
                                info.spawn((
                                    Text::new(format!("{}{}{}", p.username, you_tag, p_tag)),
                                    TextFont { font_size: FontSize::Px(11.5), ..default() },
                                    TextColor(if p.is_me { gold } else { Color::WHITE }),
                                ));
                                info.spawn((
                                    Text::new(format!("{}  |  LVL {}", hero_lbl, p.level)),
                                    TextFont { font_size: FontSize::Px(9.5), ..default() },
                                    TextColor(team_color.with_alpha(0.88)),
                                ));
                            });
                        });

                    top_row
                        .spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|stats| {
                            spawn_kda_colored_row(stats, p.kills, p.deaths, p.assists, 11.5);
                            stats.spawn((
                                Text::new(format!("{}/{} CS", p.last_hits, p.denies)),
                                TextFont { font_size: FontSize::Px(10.0), ..default() },
                                TextColor(Color::srgb(0.76, 0.88, 0.88)),
                            ));
                            stats.spawn((
                                Text::new(format!("{}G", p.gold_coins)),
                                TextFont { font_size: FontSize::Px(11.0), ..default() },
                                TextColor(gold),
                            ));
                        });
                });

                // ردیف پایین: نوار اینونتوری نهایی بازیکن (مشابه اسلات‌های HUD داخل بازی)
                prow.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|inv_row| {
                    inv_row.spawn((
                        Text::new("BUILD"),
                        TextFont { font_size: FontSize::Px(8.5), ..default() },
                        TextColor(Color::srgb(0.52, 0.72, 0.74)),
                    ));
                    spawn_inventory_slots_strip(
                        inv_row,
                        items_config,
                        item_icons,
                        &p.final_items,
                        mm.history_player_item_slot_size_px,
                        mm.history_item_slot_border_radius_px,
                        mm.history_item_slot_gap_px,
                        false,
                    );
                });
            });
    }
}

fn spawn_history_screen(
    parent: &mut ChildSpawnerCommands,
    state: &PreGameSessionState,
    config: &GameConfig,
    items_config: &ItemsConfig,
    hero_portraits: &MenuHeroPortraitCache,
    item_icons: &MenuItemIconCache,
) {
    let gold = Color::srgb(0.98, 0.82, 0.30);
    let emerald = Color::srgb(0.28, 0.96, 0.60);
    let crimson = Color::srgb(0.96, 0.38, 0.38);
    let amber = Color::srgb(0.98, 0.64, 0.28);
    let cyan = Color::srgb(0.36, 0.88, 0.98);

    let Some(ref hist) = state.history else {
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|c| {
                spawn_conceptual_loader(c, "LOADING MATCH HISTORY", Color::srgb(0.32, 0.94, 0.84));
            });
        return;
    };

    let s = &hist.summary;
    let filtered_matches: Vec<&crate::backend_client::MatchHistoryEntry> = hist
        .matches
        .iter()
        .filter(|m| match state.history_filter {
            HistoryFilter::All => true,
            HistoryFilter::Ranked => !m.is_custom_lobby,
            HistoryFilter::Party => m.is_party_match,
            HistoryFilter::Custom => m.is_custom_lobby,
        })
        .collect();

    let sel_idx = if filtered_matches.is_empty() {
        0
    } else {
        state.selected_history_idx.min(filtered_matches.len() - 1)
    };
    let selected_match = filtered_matches.get(sel_idx).copied();

    let mm = &config.main_menu;

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(22.0), Val::Px(14.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|wrap| {
            // =================================================================
            // ۱. نوار بالای CAREER SUMMARY
            // =================================================================
            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.102, 0.94)),
                BorderColor::all(Color::srgba(0.26, 0.72, 0.70, 0.68)),
            ))
            .with_children(|hdr| {
                hdr.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|title_col| {
                    title_col.spawn((
                        Text::new("CAREER SUMMARY"),
                        TextFont { font_size: FontSize::Px(19.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    title_col.spawn((
                        Text::new(format!(
                            "{}W  -  {}L  -  {}A",
                            s.wins, s.losses, s.abandoned_count
                        )),
                        TextFont { font_size: FontSize::Px(10.5), ..default() },
                        TextColor(Color::srgb(0.64, 0.84, 0.82)),
                    ));
                });

                hdr.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(26.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|metrics| {
                    let avg_kda_str = format!(
                        "{:.2} KDA ({}/{}/{})",
                        s.avg_kda, s.total_kills, s.total_deaths, s.total_assists
                    );
                    for (lbl, val, col) in [
                        ("TOTAL MATCHES", format!("{}", s.total_matches), Color::WHITE),
                        ("WIN RATE", format!("{:.1}%", s.win_rate_percent), emerald),
                        ("AVG K/D/A", avg_kda_str, Color::srgb(0.86, 0.96, 0.95)),
                        ("MMR / RANK", format!("{} ({})", s.mmr, s.rank_tier), gold),
                    ] {
                        metrics
                            .spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(1.0),
                                ..default()
                            })
                            .with_children(|mcol| {
                                mcol.spawn((
                                    Text::new(lbl),
                                    TextFont { font_size: FontSize::Px(9.5), ..default() },
                                    TextColor(Color::srgb(0.60, 0.78, 0.76)),
                                ));
                                mcol.spawn((
                                    Text::new(val),
                                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                                    TextColor(col),
                                ));
                            });
                    }
                });

                hdr.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|filters| {
                    for (fl, fl_lbl) in [
                        (HistoryFilter::All, "ALL"),
                        (HistoryFilter::Ranked, "RANKED"),
                        (HistoryFilter::Party, "PARTY"),
                        (HistoryFilter::Custom, "CUSTOM"),
                    ] {
                        let active = state.history_filter == fl;
                        spawn_sized_button(
                            filters,
                            fl_lbl,
                            PreGameBtnAction::SetHistoryFilter(fl),
                            if active {
                                Color::srgb(0.09, 0.40, 0.36)
                            } else {
                                Color::srgba(0.04, 0.12, 0.14, 0.90)
                            },
                            if active {
                                Color::srgb(0.32, 0.96, 0.84)
                            } else {
                                Color::srgba(0.24, 0.50, 0.52, 0.60)
                            },
                            74.0,
                            30.0,
                        );
                    }
                });
            });

            // =================================================================
            // ۲. بخش دو ستونه: چپ MATCH LOG (53%) | راست MATCH DETAILS (47%)
            // =================================================================
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(14.0),
                ..default()
            })
            .with_children(|cols| {
                // ستون چپ: فهرست مسابقات با آیکون هیرو + نوار آیتم‌ها
                cols.spawn((
                    Node {
                        width: Val::Percent(53.0),
                        height: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(10.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.070, 0.084, 0.90)),
                    BorderColor::all(Color::srgba(0.24, 0.62, 0.60, 0.58)),
                ))
                .with_children(|log| {
                    // هدر ستون‌ها
                    log.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgba(0.28, 0.68, 0.66, 0.45)),
                    ))
                    .with_children(|th| {
                        for (lbl, pct) in [
                            ("HERO & RESULT", 40.0),
                            ("FINAL ITEMS", 28.0),
                            ("K / D / A", 16.0),
                            ("TIME", 16.0),
                        ] {
                            th.spawn(Node { width: Val::Percent(pct), ..default() })
                                .with_children(|c| {
                                    c.spawn((
                                        Text::new(lbl),
                                        TextFont { font_size: FontSize::Px(10.5), ..default() },
                                        TextColor(Color::srgb(0.62, 0.82, 0.80)),
                                    ));
                                });
                        }
                    });

                    if filtered_matches.is_empty() {
                        log.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_grow: 1.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|e| {
                            e.spawn((
                                Text::new("No matches recorded in this category yet."),
                                TextFont { font_size: FontSize::Px(14.0), ..default() },
                                TextColor(Color::srgb(0.66, 0.80, 0.80)),
                            ));
                        });
                    } else {
                        log.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                overflow: Overflow::scroll_y(),
                                ..default()
                            },
                            ScrollPosition::default(),
                            RelativeCursorPosition::default(),
                            PreGameScrollableList,
                        ))
                        .with_children(|hist_scroll| {
                            let row_icon_sz = mm.history_log_icon_size_px;
                            let row_height = (row_icon_sz + 20.0).max(64.0);
                            for (idx, m) in filtered_matches.iter().enumerate() {
                                let is_sel = idx == sel_idx;
                                let (outcome_lbl, accent) = match m.outcome.as_str() {
                                    "win" => ("VICTORY", emerald),
                                    "loss" => ("DEFEAT", crimson),
                                    "abandoned" => ("ABANDON", Color::srgb(0.98, 0.74, 0.24)),
                                    "running" => ("LIVE", cyan),
                                    _ => ("CANCELED", Color::srgb(0.60, 0.66, 0.68)),
                                };
                                let hero_def = resolve_hero_def(config, m.hero_id);
                                let hero_name = resolve_hero_display_name(config, m.hero_id);
                                let mode_sub = if m.is_custom_lobby {
                                    "CUSTOM LOBBY".to_string()
                                } else if m.is_party_match {
                                    format!("PARTY ({:+} MMR)", m.mmr_delta)
                                } else {
                                    format!("RANKED ({:+} MMR)", m.mmr_delta)
                                };
                                let dur_str = format!(
                                    "{:02}:{:02}",
                                    m.duration_sec / 60,
                                    m.duration_sec % 60
                                );
                                let kda_ratio = compute_kda_ratio(m.kills, m.deaths, m.assists);

                                hist_scroll
                                    .spawn((
                                        UiButton,
                                        Hovered::default(),
                                        Node {
                                            width: Val::Percent(100.0),
                                            flex_shrink: 0.0,
                                            height: Val::Px(row_height),
                                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                            flex_direction: FlexDirection::Row,
                                            align_items: AlignItems::Center,
                                            border: UiRect {
                                                left: Val::Px(4.5),
                                                right: Val::Px(if is_sel { 1.5 } else { 1.0 }),
                                                top: Val::Px(if is_sel { 1.5 } else { 1.0 }),
                                                bottom: Val::Px(if is_sel { 1.5 } else { 1.0 }),
                                            },
                                            border_radius: BorderRadius::all(Val::Px(5.0)),
                                            ..default()
                                        },
                                        BackgroundColor(if is_sel {
                                            Color::srgba(0.15, 0.14, 0.05, 0.94)
                                        } else {
                                            Color::srgba(0.032, 0.102, 0.122, 0.88)
                                        }),
                                        BorderColor::all(if is_sel {
                                            gold
                                        } else {
                                            accent.with_alpha(0.68)
                                        }),
                                        PreGameBtnAction::SelectHistoryMatch(idx),
                                    ))
                                    .with_children(|row| {
                                        // ستون ۱: آیکون هیرو + نام هیرو + نتیجه و مود
                                        row.spawn(Node {
                                            width: Val::Percent(40.0),
                                            flex_direction: FlexDirection::Row,
                                            align_items: AlignItems::Center,
                                            column_gap: Val::Px(8.0),
                                            ..default()
                                        })
                                        .with_children(|hero_cell| {
                                            spawn_hero_portrait_frame(
                                                hero_cell,
                                                hero_def,
                                                hero_portraits,
                                                row_icon_sz,
                                                mm.hero_icon_border_radius_px,
                                                if is_sel { gold } else { accent },
                                                1.8,
                                                Some(m.level),
                                            );
                                            hero_cell
                                                .spawn(Node {
                                                    flex_direction: FlexDirection::Column,
                                                    row_gap: Val::Px(3.0),
                                                    ..default()
                                                })
                                                .with_children(|meta| {
                                                    meta.spawn(Node {
                                                        flex_direction: FlexDirection::Row,
                                                        align_items: AlignItems::Center,
                                                        column_gap: Val::Px(5.0),
                                                        ..default()
                                                    })
                                                    .with_children(|top_line| {
                                                        top_line.spawn((
                                                            Text::new(hero_name),
                                                            TextFont {
                                                                font_size: FontSize::Px(12.5),
                                                                ..default()
                                                            },
                                                            TextColor(Color::WHITE),
                                                        ));
                                                        top_line
                                                            .spawn((
                                                                Node {
                                                                    padding: UiRect::axes(
                                                                        Val::Px(4.5),
                                                                        Val::Px(1.0),
                                                                    ),
                                                                    border: UiRect::all(Val::Px(1.0)),
                                                                    border_radius: BorderRadius::all(
                                                                        Val::Px(3.0),
                                                                    ),
                                                                    ..default()
                                                                },
                                                                BackgroundColor(
                                                                    accent.with_alpha(0.16),
                                                                ),
                                                                BorderColor::all(
                                                                    accent.with_alpha(0.80),
                                                                ),
                                                            ))
                                                            .with_children(|pill| {
                                                                pill.spawn((
                                                                    Text::new(outcome_lbl),
                                                                    TextFont {
                                                                        font_size: FontSize::Px(8.5),
                                                                        ..default()
                                                                    },
                                                                    TextColor(accent),
                                                                ));
                                                            });
                                                    });
                                                    meta.spawn((
                                                        Text::new(mode_sub),
                                                        TextFont {
                                                            font_size: FontSize::Px(9.5),
                                                            ..default()
                                                        },
                                                        TextColor(Color::srgb(0.68, 0.82, 0.80)),
                                                    ));
                                                });
                                        });

                                        // ستون ۲: نوار آیتم‌های نهایی (مشابه اینونتوری)
                                        row.spawn(Node {
                                            width: Val::Percent(28.0),
                                            align_items: AlignItems::Center,
                                            ..default()
                                        })
                                        .with_children(|items_cell| {
                                            spawn_inventory_slots_strip(
                                                items_cell,
                                                items_config,
                                                item_icons,
                                                &m.final_items,
                                                mm.history_log_item_slot_size_px,
                                                mm.history_item_slot_border_radius_px,
                                                mm.history_item_slot_gap_px,
                                                false,
                                            );
                                        });

                                        // ستون ۳: K/D/A رنگی + نسبت KDA
                                        row.spawn(Node {
                                            width: Val::Percent(16.0),
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(2.0),
                                            ..default()
                                        })
                                        .with_children(|c| {
                                            spawn_kda_colored_row(
                                                c, m.kills, m.deaths, m.assists, 12.5,
                                            );
                                            c.spawn((
                                                Text::new(format!(
                                                    "{:.2} KDA | {}G",
                                                    kda_ratio, m.gold_coins
                                                )),
                                                TextFont {
                                                    font_size: FontSize::Px(9.0),
                                                    ..default()
                                                },
                                                TextColor(gold),
                                            ));
                                        });

                                        // ستون ۴: مدت زمان و تاریخ
                                        row.spawn(Node {
                                            width: Val::Percent(16.0),
                                            flex_direction: FlexDirection::Column,
                                            row_gap: Val::Px(2.0),
                                            ..default()
                                        })
                                        .with_children(|c| {
                                            c.spawn((
                                                Text::new(dur_str),
                                                TextFont {
                                                    font_size: FontSize::Px(11.5),
                                                    ..default()
                                                },
                                                TextColor(Color::srgb(0.88, 0.95, 0.95)),
                                            ));
                                            c.spawn((
                                                Text::new(m.played_at.clone()),
                                                TextFont {
                                                    font_size: FontSize::Px(9.5),
                                                    ..default()
                                                },
                                                TextColor(if is_sel {
                                                    gold
                                                } else {
                                                    Color::srgb(0.68, 0.80, 0.80)
                                                }),
                                            ));
                                        });
                                    });
                            }
                        });
                    }
                });

                // ستون راست: MATCH DETAILS با بنر هیرو + اینونتوری کامل + جدول دو تیم
                cols.spawn(Node {
                    width: Val::Percent(47.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::SpaceBetween,
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|right_col| {
                    right_col
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                flex_grow: 1.0,
                                min_height: Val::Px(0.0),
                                padding: UiRect::all(Val::Px(14.0)),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(10.0),
                                border: UiRect::all(Val::Px(1.5)),
                                border_radius: BorderRadius::all(Val::Px(6.0)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.025, 0.082, 0.098, 0.94)),
                            BorderColor::all(Color::srgba(0.26, 0.72, 0.70, 0.68)),
                        ))
                        .with_children(|det| {
                            if let Some(m) = selected_match {
                                let (o_lbl, o_col) = match m.outcome.as_str() {
                                    "win" => ("VICTORY", emerald),
                                    "loss" => ("DEFEAT", crimson),
                                    "abandoned" => ("ABANDONED", Color::srgb(0.98, 0.74, 0.24)),
                                    "running" => ("LIVE MATCH", cyan),
                                    _ => ("CANCELED", Color::srgb(0.64, 0.72, 0.74)),
                                };
                                let dur_str = format!(
                                    "{:02}:{:02}",
                                    m.duration_sec / 60,
                                    m.duration_sec % 60
                                );
                                let mode_lbl = if m.is_custom_lobby {
                                    "CUSTOM LOBBY"
                                } else if m.is_party_match {
                                    "PARTY RANKED"
                                } else {
                                    "SOLO RANKED"
                                };
                                let hero_def = resolve_hero_def(config, m.hero_id);
                                let hero_name = resolve_hero_display_name(config, m.hero_id);
                                let kda_ratio = compute_kda_ratio(m.kills, m.deaths, m.assists);

                                // بنر ویژه هیرو انتخاب‌شده
                                det.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_shrink: 0.0,
                                        padding: UiRect::all(Val::Px(11.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(12.0),
                                        border: UiRect {
                                            left: Val::Px(4.0),
                                            right: Val::Px(1.0),
                                            top: Val::Px(1.0),
                                            bottom: Val::Px(1.0),
                                        },
                                        border_radius: BorderRadius::all(Val::Px(6.0)),
                                        ..default()
                                    },
                                    BackgroundColor(o_col.with_alpha(0.11)),
                                    BorderColor::all(o_col.with_alpha(0.65)),
                                ))
                                .with_children(|banner| {
                                    banner
                                        .spawn(Node {
                                            flex_direction: FlexDirection::Row,
                                            align_items: AlignItems::Center,
                                            column_gap: Val::Px(12.0),
                                            ..default()
                                        })
                                        .with_children(|left| {
                                            spawn_hero_portrait_frame(
                                                left,
                                                hero_def,
                                                hero_portraits,
                                                mm.history_detail_hero_icon_size_px,
                                                mm.hero_icon_border_radius_px,
                                                o_col,
                                                2.2,
                                                Some(m.level),
                                            );
                                            left.spawn(Node {
                                                flex_direction: FlexDirection::Column,
                                                row_gap: Val::Px(3.0),
                                                ..default()
                                            })
                                            .with_children(|txt| {
                                                txt.spawn((
                                                    Text::new(format!("{}  //  {}", o_lbl, mode_lbl)),
                                                    TextFont {
                                                        font_size: FontSize::Px(10.5),
                                                        ..default()
                                                    },
                                                    TextColor(o_col),
                                                ));
                                                txt.spawn((
                                                    Text::new(hero_name),
                                                    TextFont {
                                                        font_size: FontSize::Px(18.0),
                                                        ..default()
                                                    },
                                                    TextColor(Color::WHITE),
                                                ));
                                                txt.spawn((
                                                    Text::new(format!(
                                                        "{}  |  {}  |  {}",
                                                        mm.default_map_name, dur_str, m.played_at
                                                    )),
                                                    TextFont {
                                                        font_size: FontSize::Px(9.5),
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.70, 0.84, 0.84)),
                                                ));
                                            });
                                        });

                                    banner
                                        .spawn(Node {
                                            flex_direction: FlexDirection::Column,
                                            align_items: AlignItems::FlexEnd,
                                            row_gap: Val::Px(2.0),
                                            ..default()
                                        })
                                        .with_children(|kda_box| {
                                            spawn_kda_colored_row(
                                                kda_box, m.kills, m.deaths, m.assists, 17.5,
                                            );
                                            kda_box.spawn((
                                                Text::new(format!("{:.2} KDA", kda_ratio)),
                                                TextFont {
                                                    font_size: FontSize::Px(10.5),
                                                    ..default()
                                                },
                                                TextColor(gold),
                                            ));
                                        });
                                });

                                // پنل FINAL INVENTORY مشابه طراحی اینونتوری داخل بازی
                                let equipped_names: Vec<String> = m
                                    .final_items
                                    .iter()
                                    .filter_map(|it| {
                                        if let Some(target) = parse_recipe_target(&it.item_id) {
                                            Some(format!("RECIPE: {}", target.to_uppercase()))
                                        } else {
                                            items_config
                                                .get_item(&it.item_id)
                                                .map(|d| d.name.to_uppercase())
                                        }
                                    })
                                    .collect();
                                let items_summary = if equipped_names.is_empty() {
                                    "No items equipped in final inventory.".to_string()
                                } else {
                                    equipped_names.join("  |  ")
                                };

                                det.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_shrink: 0.0,
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(6.0),
                                        border: UiRect::all(Val::Px(1.0)),
                                        border_radius: BorderRadius::all(Val::Px(5.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.022, 0.068, 0.086, 0.94)),
                                    BorderColor::all(Color::srgba(0.24, 0.58, 0.60, 0.52)),
                                ))
                                .with_children(|inv_panel| {
                                    inv_panel
                                        .spawn(Node {
                                            width: Val::Percent(100.0),
                                            flex_direction: FlexDirection::Row,
                                            justify_content: JustifyContent::SpaceBetween,
                                            align_items: AlignItems::Center,
                                            ..default()
                                        })
                                        .with_children(|row| {
                                            row.spawn(Node {
                                                flex_direction: FlexDirection::Column,
                                                row_gap: Val::Px(2.0),
                                                ..default()
                                            })
                                            .with_children(|lbl_col| {
                                                lbl_col.spawn((
                                                    Text::new("FINAL INVENTORY"),
                                                    TextFont {
                                                        font_size: FontSize::Px(10.5),
                                                        ..default()
                                                    },
                                                    TextColor(gold),
                                                ));
                                                lbl_col.spawn((
                                                    Text::new(format!(
                                                        "{} / {} SLOTS",
                                                        equipped_names.len(),
                                                        items_config.max_inventory_slots
                                                    )),
                                                    TextFont {
                                                        font_size: FontSize::Px(9.0),
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.62, 0.80, 0.80)),
                                                ));
                                            });

                                            spawn_inventory_slots_strip(
                                                row,
                                                items_config,
                                                item_icons,
                                                &m.final_items,
                                                mm.history_detail_item_slot_size_px,
                                                mm.history_item_slot_border_radius_px,
                                                mm.history_item_slot_gap_px,
                                                true,
                                            );
                                        });

                                    inv_panel.spawn((
                                        Text::new(items_summary),
                                        TextFont { font_size: FontSize::Px(9.0), ..default() },
                                        TextColor(Color::srgb(0.72, 0.86, 0.86)),
                                    ));
                                });

                                // ردیف ۴ کارت آمار کلیدی نبرد
                                let mmr_str = if m.is_custom_lobby {
                                    "0 (CUSTOM)".to_string()
                                } else {
                                    format!("{:+} MMR", m.mmr_delta)
                                };
                                let mmr_col = if m.mmr_delta > 0 {
                                    emerald
                                } else if m.mmr_delta < 0 {
                                    crimson
                                } else {
                                    Color::srgb(0.75, 0.84, 0.84)
                                };
                                det.spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_shrink: 0.0,
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                })
                                .with_children(|strip| {
                                    for (k, v, c) in [
                                        ("GOLD EARNED", format!("{}G", m.gold_coins), gold),
                                        (
                                            "CS / DENIES",
                                            format!("{} / {}", m.last_hits, m.denies),
                                            Color::WHITE,
                                        ),
                                        ("HERO LEVEL", format!("LVL {}", m.level), cyan),
                                        ("MMR CHANGE", mmr_str, mmr_col),
                                    ] {
                                        strip
                                            .spawn((
                                                Node {
                                                    flex_grow: 1.0,
                                                    flex_basis: Val::Px(0.0),
                                                    padding: UiRect::axes(
                                                        Val::Px(8.0),
                                                        Val::Px(6.0),
                                                    ),
                                                    flex_direction: FlexDirection::Column,
                                                    row_gap: Val::Px(2.0),
                                                    border: UiRect::all(Val::Px(1.0)),
                                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(Color::srgba(
                                                    0.038, 0.122, 0.145, 0.92,
                                                )),
                                                BorderColor::all(Color::srgba(
                                                    0.24, 0.56, 0.56, 0.48,
                                                )),
                                            ))
                                            .with_children(|cell| {
                                                cell.spawn((
                                                    Text::new(k),
                                                    TextFont {
                                                        font_size: FontSize::Px(9.0),
                                                        ..default()
                                                    },
                                                    TextColor(Color::srgb(0.62, 0.80, 0.78)),
                                                ));
                                                cell.spawn((
                                                    Text::new(v),
                                                    TextFont {
                                                        font_size: FontSize::Px(13.0),
                                                        ..default()
                                                    },
                                                    TextColor(c),
                                                ));
                                            });
                                    }
                                });

                                // جدول بازیکنان تفکیک‌شده بر اساس تیم (قابل اسکرول)
                                let mut party_tags: HashMap<String, usize> = HashMap::new();
                                for p in &m.players {
                                    if !p.party_group_id.is_empty()
                                        && !party_tags.contains_key(&p.party_group_id)
                                    {
                                        let next_id = party_tags.len() + 1;
                                        party_tags.insert(p.party_group_id.clone(), next_id);
                                    }
                                }

                                let mut alpha_players: Vec<&crate::backend_client::MatchHistoryPlayerRow> =
                                    m.players.iter().filter(|p| p.team_id == 1).collect();
                                alpha_players.sort_by_key(|p| p.slot_index);

                                let mut omega_players: Vec<&crate::backend_client::MatchHistoryPlayerRow> =
                                    m.players.iter().filter(|p| p.team_id != 1).collect();
                                omega_players.sort_by_key(|p| p.slot_index);

                                det.spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        flex_grow: 1.0,
                                        min_height: Val::Px(0.0),
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(6.0),
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                    ScrollPosition::default(),
                                    RelativeCursorPosition::default(),
                                    PreGameScrollableList,
                                ))
                                .with_children(|roster| {
                                    spawn_history_team_block(
                                        roster,
                                        config,
                                        items_config,
                                        hero_portraits,
                                        item_icons,
                                        "TEAM ALPHA",
                                        emerald,
                                        m.winner_team == 1,
                                        &alpha_players,
                                        &party_tags,
                                    );
                                    spawn_history_team_block(
                                        roster,
                                        config,
                                        items_config,
                                        hero_portraits,
                                        item_icons,
                                        "TEAM OMEGA",
                                        amber,
                                        m.winner_team == 2,
                                        &omega_players,
                                        &party_tags,
                                    );
                                });
                            } else {
                                det.spawn((
                                    Text::new("MATCH DETAILS"),
                                    TextFont { font_size: FontSize::Px(17.0), ..default() },
                                    TextColor(Color::WHITE),
                                ));
                                det.spawn((
                                    Text::new("Select a match from the log to inspect details."),
                                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                                    TextColor(Color::srgb(0.68, 0.80, 0.80)),
                                ));
                            }
                        });

                    // دکمه BACK TO MENU در پایین راست
                    right_col
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::FlexEnd,
                            ..default()
                        })
                        .with_children(|b| {
                            spawn_sized_button(
                                b,
                                "BACK TO MENU",
                                PreGameBtnAction::SwitchTab(PreGameTab::Hub),
                                Color::srgba(0.05, 0.16, 0.18, 0.94),
                                Color::srgb(0.30, 0.86, 0.78),
                                210.0,
                                48.0,
                            );
                        });
                });
            });
        });
}

fn spawn_conceptual_loader(parent: &mut ChildSpawnerCommands, label: &str, color: Color) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|box_col| {
            let radius = 24.0;
            box_col
                .spawn(Node {
                    width: Val::Px(radius * 2.0),
                    height: Val::Px(radius * 2.0),
                    ..default()
                })
                .with_children(|orbit| {
                    for i in 0..6 {
                        orbit.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(10.0),
                                height: Val::Px(10.0),
                                border_radius: BorderRadius::all(Val::Px(5.0)),
                                ..default()
                            },
                            BackgroundColor(color),
                            ConceptualOrbitDot {
                                phase: (i as f32) * (TAU / 6.0),
                                radius,
                            },
                        ));
                    }
                });

            box_col.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(15.0), ..default() },
                TextColor(color),
            ));
        });
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
// صفحه انتخاب هیرو برای ورود به محیط Demo Hero (در دسترس همه بازیکنان از منوی اصلی)
// =============================================================================

fn spawn_demo_hero_screen(
    parent: &mut ChildSpawnerCommands,
    config: &GameConfig,
    hero_portraits: &MenuHeroPortraitCache,
) {
    let gold = Color::srgb(0.98, 0.84, 0.34);
    let teal = Color::srgb(0.28, 0.94, 0.82);
    let mm = &config.main_menu;

    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(26.0), Val::Px(18.0)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::SpaceBetween,
            row_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|wrap| {
            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.025, 0.088, 0.105, 0.92)),
                BorderColor::all(Color::srgba(0.26, 0.78, 0.74, 0.70)),
            ))
            .with_children(|hdr| {
                hdr.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Text::new("DEMO HERO // TACTICAL SANDBOX"),
                        TextFont { font_size: FontSize::Px(20.0), ..default() },
                        TextColor(gold),
                    ));
                    col.spawn((
                        Text::new("Test any hero, weapon, item counter, weather, day/night, turrets, creeps, and enemy dummies locally with zero latency."),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(Color::srgb(0.76, 0.90, 0.90)),
                    ));
                });

                spawn_sized_button(
                    hdr,
                    "BACK TO MENU",
                    PreGameBtnAction::SwitchTab(PreGameTab::Hub),
                    Color::srgba(0.06, 0.16, 0.19, 0.94),
                    Color::srgb(0.32, 0.78, 0.76),
                    156.0,
                    40.0,
                );
            });

            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    align_content: AlignContent::FlexStart,
                    column_gap: Val::Px(14.0),
                    row_gap: Val::Px(14.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
                RelativeCursorPosition::default(),
                PreGameScrollableList,
            ))
            .with_children(|grid| {
                for hero in &config.heroes {
                    grid.spawn((
                        Node {
                            width: Val::Px(312.0),
                            padding: UiRect::all(Val::Px(14.0)),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::SpaceBetween,
                            row_gap: Val::Px(12.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.025, 0.082, 0.098, 0.92)),
                        BorderColor::all(Color::srgba(0.26, 0.68, 0.68, 0.65)),
                    ))
                    .with_children(|card| {
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(12.0),
                            ..default()
                        })
                        .with_children(|header| {
                            spawn_hero_portrait_frame(
                                header,
                                Some(hero),
                                hero_portraits,
                                mm.demo_hero_icon_size_px,
                                mm.hero_icon_border_radius_px,
                                teal.with_alpha(0.85),
                                1.8,
                                None,
                            );
                            header
                                .spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|info| {
                                    info.spawn((
                                        Text::new(hero.display_name.to_uppercase()),
                                        TextFont { font_size: FontSize::Px(15.5), ..default() },
                                        TextColor(Color::WHITE),
                                    ));
                                    info.spawn((
                                        Text::new(format!("WEAPON: {:?}", hero.weapon.weapon_kind)),
                                        TextFont { font_size: FontSize::Px(11.0), ..default() },
                                        TextColor(teal),
                                    ));
                                    info.spawn((
                                        Text::new(format!(
                                            "HP {:.0}  |  ARMOR {:.1}  |  DMG {:.0}",
                                            hero.max_hp, hero.armor, hero.weapon.damage
                                        )),
                                        TextFont { font_size: FontSize::Px(10.5), ..default() },
                                        TextColor(Color::srgb(0.82, 0.92, 0.92)),
                                    ));
                                });
                        });

                        spawn_sized_button(
                            card,
                            "LAUNCH DEMO HERO",
                            PreGameBtnAction::LaunchDemoHero(hero.id.clone()),
                            Color::srgba(0.64, 0.46, 0.10, 0.95),
                            gold,
                            282.0,
                            38.0,
                        );
                    });
                }
            });
        });
}