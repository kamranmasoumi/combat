use crate::backend_client::{
    clear_persisted_session, load_persisted_session, sanitize_ascii_msg, ActiveMatchInfo,
    BackendAsyncEvent, BackendClient, FriendsSnapshot, LobbyDetail, LobbySummary,
    MatchHistoryResponse, PartyInviteInfo, PartyState, PlayerProfile, PlayerSearchResult,
};
use bevy::animation::RepeatAnimation;
use bevy::app::AppExit;
use bevy::gltf::Gltf;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::light::AmbientLight;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button as UiButton;
use bevy::window::{CursorOptions, PrimaryWindow};
use moba_shared::GameConfig;
use std::collections::HashSet;
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
    pub social_syncing: bool,
    pub social_reconnect_timer: f32,
    pub pending_sent_invites: HashSet<String>,
    pub party: Option<PartyState>,
    pub pending_party_invites: Vec<PartyInviteInfo>,
    pub party_chat_open: bool,
    pub party_chat_input: String,
    // Custom Lobby state
    pub lobbies: Vec<LobbySummary>,
    pub current_lobby: Option<LobbyDetail>,
    pub create_title: String,
    pub create_password: String,
    pub create_capacity: usize,
    pub join_target_lobby_id: Option<String>,
    pub join_password_input: String,
    pub lobby_chat_input: String,
    // Match History state
    pub history: Option<MatchHistoryResponse>,
    pub selected_history_idx: usize,
    pub confirm_modal: Option<ConfirmAction>,
    pub ui_dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Logout,
    QuitApp,
    AbandonMatch,
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
            social_syncing: false,
            social_reconnect_timer: 0.0,
            pending_sent_invites: HashSet::new(),
            party: None,
            pending_party_invites: Vec::new(),
            party_chat_open: false,
            party_chat_input: String::new(),
            lobbies: Vec::new(),
            current_lobby: None,
            create_title: String::new(),
            create_password: String::new(),
            create_capacity: 2,
            join_target_lobby_id: None,
            join_password_input: String::new(),
            lobby_chat_input: String::new(),
            history: None,
            selected_history_idx: 0,
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

#[derive(Component)]
struct PreGameEntity;

#[derive(Component)]
struct PreGameHiddenHud(Visibility);

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
    SendFriendRequest(String),
    RespondFriendRequest { request_id: i64, accept: bool },
    RemoveFriend(String),
    InviteToParty(String),
    RespondPartyInvite { invite_id: String, accept: bool },
    TogglePartyReady,
    KickPartyMember(String),
    PromotePartyLeader(String),
    LeaveParty,
    TogglePartyChat,
    SendPartyChat,
    DismissNotification(u64),
    // Custom Lobby actions
    RefreshLobbies,
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
    RequestLogoutConfirm,
    RequestQuitConfirm,
    ConfirmModalAccept,
    ConfirmModalCancel,
    FocusField(FocusedInput),
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
            .add_systems(OnEnter(AppScreenState::PreGame), on_enter_pregame)
            .add_systems(OnExit(AppScreenState::PreGame), on_exit_pregame)
            .add_systems(
                Update,
                (
                    poll_backend_events_system,
                    handle_pregame_keyboard_system,
                    handle_pregame_buttons_system,
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
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut showcase: ResMut<MenuShowcaseAssets>,
    mut cursor_q: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut old_cams_q: Query<&mut Camera, (With<Camera3d>, Without<PreGameEntity>)>,
    mut hud_roots: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>, Without<PreGameEntity>)>,
) {
    for mut copt in &mut cursor_q {
        copt.visible = true;
    }
    for mut cam in &mut old_cams_q {
        cam.is_active = false;
    }
    for (ent, mut vis) in &mut hud_roots {
        commands.entity(ent).insert(PreGameHiddenHud(*vis));
        *vis = Visibility::Hidden;
    }

    let mm = &config.main_menu;
    showcase.gltf_handle = Some(asset_server.load(mm.soldier_model_path.clone()));
    showcase.idle_graph = None;
    showcase.idle_node = None;
    showcase.spawned_squad_count = 0;
    showcase.showcase_visible = false;

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
    mut hidden_huds: Query<(Entity, &mut Visibility, &PreGameHiddenHud)>,
) {
    backend.disconnect_social_ws();
    showcase.spawned_squad_count = 0;
    showcase.idle_graph = None;
    showcase.idle_node = None;
    for ent in &query {
        commands.entity(ent).despawn();
    }
    for (ent, mut vis, saved) in &mut hidden_huds {
        *vis = saved.0;
        commands.entity(ent).remove::<PreGameHiddenHud>();
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
        commands.entity(ent).despawn();
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

    // اتصال مجدد خودکار وب‌سوکت Social/Party در صورت قطع شدن
    if let Some(tok) = state.access_token.clone() {
        if !backend.is_social_ws_connected() {
            state.social_reconnect_timer -= dt;
            if state.social_reconnect_timer <= 0.0 {
                state.social_reconnect_timer = 2.0;
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
                if let Some(ref p) = party {
                    for m in &p.members {
                        state.pending_sent_invites.remove(&m.user_id);
                    }
                }
                state.party = party;
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
                for m in &ps.members {
                    state.pending_sent_invites.remove(&m.user_id);
                }
                if ps.is_queuing && state.tab == PreGameTab::Hub {
                    state.tab = PreGameTab::FindingMatch;
                }
                state.party = Some(ps);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::PartyCleared { reason } => {
                state.party = None;
                state.party_chat_open = false;
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
            BackendAsyncEvent::MatchmakingClosed => {
                if state.tab == PreGameTab::FindingMatch && !state.server_starting {
                    state.tab = PreGameTab::Hub;
                    state.ui_dirty = true;
                }
            }
            BackendAsyncEvent::LobbyList { lobbies, message } => {
                state.is_loading = false;
                state.lobbies = lobbies;
                if !message.is_empty() && state.tab == PreGameTab::LobbyBrowser {
                    set_toast(&mut state, &message, false);
                }
                state.ui_dirty = true;
            }
            BackendAsyncEvent::LobbyState { lobby, message } => {
                state.is_loading = false;
                state.current_lobby = Some(lobby);
                state.join_target_lobby_id = None;
                state.join_password_input.clear();
                state.tab = PreGameTab::LobbyRoom;
                if !message.is_empty() {
                    set_toast(&mut state, &message, false);
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
                set_toast(&mut state, &message, false);
                state.ui_dirty = true;
            }
            BackendAsyncEvent::LobbyError(err) => {
                state.is_loading = false;
                push_notification(&mut state, "error", "LOBBY ERROR", &err, notif_dur);
            }
            BackendAsyncEvent::LobbyWsClosed => {
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
                }
            }
            BackendAsyncEvent::SocialWsClosed => {
                backend.disconnect_social_ws();
                state.social_syncing = false;
                state.social_reconnect_timer = 1.5;
                state.ui_dirty = true;
            }
        }
    }
}

fn set_toast(state: &mut PreGameSessionState, msg: &str, is_error: bool) {
    state.toast_message = sanitize_ascii_msg(
        msg,
        if is_error { "Operation failed." } else { "Updated." },
    );
    state.toast_is_error = is_error;
    state.toast_timer = 5.0;
    state.ui_dirty = true;
}

// =============================================================================
// ورودی صفحه‌کلید
// =============================================================================

fn handle_pregame_keyboard_system(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    config: Res<GameConfig>,
    backend: Res<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
) {
    if state.confirm_modal.is_some() {
        if keys.just_pressed(KeyCode::Escape) {
            state.confirm_modal = None;
            state.ui_dirty = true;
        }
        return;
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
                    FocusedInput::FriendSearch => 32,
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
                    backend.send_lobby_action(serde_json::json!({
                        "action": "join_lobby",
                        "lobby_id": lid,
                        "password": state.join_password_input.trim()
                    }));
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
        FocusedInput::PartyChat => &mut state.party_chat_input,
        FocusedInput::None => &mut state.email_input,
    }
}

// =============================================================================
// پردازش کلیک دکمه‌های منو
// =============================================================================

fn handle_pregame_buttons_system(
    config: Res<GameConfig>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut app_exit: MessageWriter<AppExit>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut btn_q: Query<(&Hovered, Has<Pressed>, &PreGameBtnAction, &mut BorderColor), With<UiButton>>,
) {
    let mut triggered: Option<PreGameBtnAction> = None;
    let clicked_this_frame = mouse.just_pressed(MouseButton::Left);
    for (hovered, pressed, action, mut border) in &mut btn_q {
        if pressed {
            *border = BorderColor::all(Color::srgb(0.98, 0.84, 0.36));
        } else if hovered.get() {
            *border = BorderColor::all(Color::srgb(0.32, 0.96, 0.84));
        }
        if clicked_this_frame && hovered.get() && triggered.is_none() {
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
            if let Some(ref p) = state.party {
                let my_uid = state
                    .profile
                    .as_ref()
                    .map(|pr| pr.user_id.as_str())
                    .unwrap_or("");
                if p.leader_user_id != my_uid {
                    push_notification(
                        &mut state,
                        "info",
                        "PARTY LEADER ONLY",
                        "Only the squad leader can start matchmaking.",
                        notif_dur,
                    );
                    return;
                }
                state.server_starting = false;
                backend.send_social_action(serde_json::json!({
                    "action": "start_party_matchmaking"
                }));
            } else {
                state.tab = PreGameTab::FindingMatch;
                state.queue_players = 1;
                state.queue_required = 2;
                state.server_starting = false;
                state.ui_dirty = true;
                backend.start_matchmaking(tok);
            }
        }
        PreGameBtnAction::CancelFindMatch => {
            if state.party.is_some() {
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
        PreGameBtnAction::RemoveFriend(uid) => {
            if let Some(tok) = state.access_token.clone() {
                state.is_loading = true;
                state.loading_label = "UPDATING FRIENDS LIST".to_string();
                state.ui_dirty = true;
                backend.remove_friend(tok, uid);
            }
        }
        PreGameBtnAction::InviteToParty(uid) => {
            if state.pending_sent_invites.contains(&uid) {
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
            state.pending_sent_invites.insert(uid.clone());
            state.ui_dirty = true;
            let mode = config
                .main_menu
                .game_modes
                .first()
                .cloned()
                .unwrap_or_else(|| "5v5 JUNGLE SKIRMISH".to_string());
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
        PreGameBtnAction::CreateLobby => {
            backend.send_lobby_action(serde_json::json!({
                "action": "create_lobby",
                "title": state.create_title.trim(),
                "password": state.create_password.trim(),
                "max_players": state.create_capacity,
            }));
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
                backend.send_lobby_action(serde_json::json!({
                    "action": "join_lobby",
                    "lobby_id": lid,
                    "password": state.join_password_input.trim()
                }));
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
            backend.send_lobby_action(serde_json::json!({
                "action": "switch_team",
                "team_id": team_id,
                "slot_index": slot_index
            }));
        }
        PreGameBtnAction::KickLobbyPlayer(uid) => {
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
                    state.friends = FriendsSnapshot::default();
                    state.friend_search_results.clear();
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
    backend: Res<BackendClient>,
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
            commands.entity(ch).despawn();
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
                spawn_top_nav_bar(parent, profile, &state, backend.is_social_ws_connected());
            }
        }

        match state.tab {
            PreGameTab::AuthEmail | PreGameTab::AuthOtp => spawn_auth_screen(parent, &state),
            PreGameTab::Hub => spawn_hub_screen(parent, &state, &config),
            PreGameTab::FindingMatch => spawn_finding_match_screen(parent, &state, &config),
            PreGameTab::LobbyBrowser => spawn_lobby_browser_screen(parent, &state),
            PreGameTab::LobbyRoom => spawn_lobby_room_screen(parent, &state),
            PreGameTab::MatchHistory => spawn_history_screen(parent, &state),
        }

        if !state.pending_party_invites.is_empty() || !state.notifications.is_empty() {
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

        if let Some(confirm) = state.confirm_modal {
            let (title, body, ok_label) = match confirm {
                ConfirmAction::Logout => (
                    "CONFIRM LOG OUT",
                    "Are you sure you want to log out of your account?",
                    "LOG OUT",
                ),
                ConfirmAction::QuitApp => (
                    "EXIT GAME",
                    "Are you sure you want to close the game client?",
                    "EXIT GAME",
                ),
                ConfirmAction::AbandonMatch => (
                    "ABANDON ACTIVE MATCH?",
                    "Abandoning deducts -25 MMR, records a loss, and prevents reconnecting to this match.",
                    "ABANDON MATCH",
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
                            Text::new(body),
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
    ws_online: bool,
) {
    let level_est = (profile.wins + profile.losses + 1).max(1);
    let mmr_progress_pct = ((profile.mmr % 500) as f32 / 500.0 * 100.0).clamp(8.0, 100.0);

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
    let in_party = state.party.is_some();
    let show_party_controls = party_members_count >= 2;
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
                                            spawn_sized_button(
                                                acts,
                                                "CHAT",
                                                PreGameBtnAction::TogglePartyChat,
                                                Color::srgb(0.08, 0.24, 0.28),
                                                Color::srgb(0.30, 0.80, 0.82),
                                                46.0,
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
                                                        cell.spawn((
                                                            UiButton,
                                                            Hovered::default(),
                                                            Node {
                                                                padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
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
                                                                Text::new("KICK"),
                                                                TextFont {
                                                                    font_size: FontSize::Px(8.5),
                                                                    ..default()
                                                                },
                                                                TextColor(Color::WHITE),
                                                            ));
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
            // پایین وسط: کارت RECENT MATCHES
            // =================================================================
            stage
                .spawn((
                    Node {
                        width: Val::Px(256.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.082, 0.098, 0.88)),
                    BorderColor::all(Color::srgba(0.24, 0.64, 0.62, 0.70)),
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
                        for m in matches_slice.iter().take(3) {
                            let (lbl, row_bg, lbl_col) = match m.outcome.as_str() {
                                "win" => (
                                    "VICTORY",
                                    Color::srgba(0.07, 0.28, 0.18, 0.82),
                                    Color::srgb(0.32, 0.96, 0.56),
                                ),
                                "loss" => (
                                    "DEFEAT",
                                    Color::srgba(0.32, 0.09, 0.10, 0.82),
                                    Color::srgb(0.98, 0.42, 0.42),
                                ),
                                "abandoned" => (
                                    "ABANDON",
                                    Color::srgba(0.30, 0.20, 0.06, 0.82),
                                    Color::srgb(0.98, 0.78, 0.34),
                                ),
                                _ => (
                                    "LIVE",
                                    Color::srgba(0.08, 0.24, 0.30, 0.82),
                                    Color::srgb(0.40, 0.90, 0.98),
                                ),
                            };
                            let party_tag = if m.is_party_match { " [P]" } else { "" };
                            recent
                                .spawn((
                                    UiButton,
                                    Hovered::default(),
                                    Node {
                                        width: Val::Percent(100.0),
                                        padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                                        flex_direction: FlexDirection::Row,
                                        justify_content: JustifyContent::SpaceBetween,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(1.0)),
                                        border_radius: BorderRadius::all(Val::Px(3.0)),
                                        ..default()
                                    },
                                    BackgroundColor(row_bg),
                                    BorderColor::all(Color::srgba(0.28, 0.50, 0.52, 0.35)),
                                    PreGameBtnAction::SwitchTab(PreGameTab::MatchHistory),
                                ))
                                .with_children(|r| {
                                    r.spawn((
                                        Text::new(format!("{}{}", lbl, party_tag)),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(lbl_col),
                                    ));
                                    r.spawn((
                                        Text::new(format!(
                                            "{} / {} / {}",
                                            m.kills, m.deaths, m.assists
                                        )),
                                        TextFont {
                                            font_size: FontSize::Px(12.0),
                                            ..default()
                                        },
                                        TextColor(Color::srgb(0.92, 0.96, 0.96)),
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
                            PreGameBtnAction::SwitchTab(PreGameTab::LobbyBrowser),
                            Color::srgba(0.04, 0.24, 0.22, 0.92),
                            Color::srgb(0.24, 0.92, 0.76),
                            218.0,
                            62.0,
                        );
                        let find_lbl = if show_party_controls {
                            if is_party_leader {
                                format!("FIND MATCH ({})", party_members_count)
                            } else {
                                "SQUAD READY".to_string()
                            }
                        } else {
                            "FIND MATCH".to_string()
                        };
                        spawn_sized_button(
                            btns,
                            &find_lbl,
                            PreGameBtnAction::StartFindMatch,
                            Color::srgba(0.68, 0.48, 0.10, 0.96),
                            Color::srgb(1.0, 0.86, 0.36),
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

            // جستجوی بازیکن
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(6.0),
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
                    spawn_sized_button(
                        search_row,
                        if state.friend_searching { "..." } else { "FIND" },
                        PreGameBtnAction::SearchPlayers,
                        Color::srgb(0.10, 0.40, 0.36),
                        Color::srgb(0.30, 0.94, 0.82),
                        58.0,
                        36.0,
                    );
                });

            // نتایج جستجو
            if !state.friend_search_results.is_empty() {
                for res in state.friend_search_results.iter().take(3) {
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
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
                            } else {
                                let badge = match res.relationship.as_str() {
                                    "friend" => "FRIEND",
                                    "outgoing_pending" => "PENDING",
                                    "incoming_pending" => "INCOMING",
                                    _ => "ADDED",
                                };
                                row.spawn((
                                    Text::new(badge),
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

            // درخواست‌های دوستی ورودی
            if !state.friends.incoming_requests.is_empty() {
                panel.spawn((
                    Text::new(format!(
                        "REQUESTS ({})",
                        state.friends.incoming_requests.len()
                    )),
                    TextFont {
                        font_size: FontSize::Px(10.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.82, 0.34)),
                ));
                for req in state.friends.incoming_requests.iter().take(3) {
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
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

            // لیست دوستان همراه با وضعیت زنده و دکمه INVITE / INVITING...
            if state.friends.friends.is_empty() {
                panel.spawn((
                    Text::new("No friends yet. Search by username above!"),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.60, 0.74, 0.74)),
                ));
            } else {
                for f in state.friends.friends.iter().take(7) {
                    let dot_col = if !f.online {
                        Color::srgb(0.42, 0.48, 0.50)
                    } else if f.status == "in_party" || f.status == "in_queue" {
                        Color::srgb(0.96, 0.78, 0.28)
                    } else {
                        Color::srgb(0.24, 0.96, 0.58)
                    };
                    let already_in_my_party = state
                        .party
                        .as_ref()
                        .map(|p| p.members.iter().any(|m| m.user_id == f.user_id))
                        .unwrap_or(false);
                    let is_inviting = state.pending_sent_invites.contains(&f.user_id);

                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
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
                                            f.status.to_uppercase(),
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
                                    ))
                                    .with_children(|invb| {
                                        invb.spawn((
                                            Text::new("INVITING.."),
                                            TextFont {
                                                font_size: FontSize::Px(9.5),
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.98, 0.88, 0.45)),
                                        ));
                                    });
                                } else if f.online {
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
                                spawn_sized_button(
                                    acts,
                                    "X",
                                    PreGameBtnAction::RemoveFriend(f.user_id.clone()),
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

fn spawn_lobby_browser_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let cyan = Color::srgb(0.32, 0.72, 0.98);
    let emerald = Color::srgb(0.16, 0.86, 0.58);
    let gold = Color::srgb(0.98, 0.80, 0.26);

    parent
        .spawn(Node {
            width: Val::Percent(94.0),
            max_width: Val::Px(1120.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(20.0),
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|row| {
            // ستون چپ: لیست لابی‌های فعال
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    padding: UiRect::all(Val::Px(20.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(12.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
                BorderColor::all(cyan.with_alpha(0.55)),
            ))
            .with_children(|left| {
                left.spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("ACTIVE LOBBIES"),
                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    spawn_action_button(hdr, "REFRESH", cyan, PreGameBtnAction::RefreshLobbies);
                });

                if state.lobbies.is_empty() {
                    left.spawn((
                        Text::new("NO LOBBIES YET"),
                        TextFont { font_size: FontSize::Px(14.0), ..default() },
                        TextColor(Color::srgb(0.55, 0.65, 0.78)),
                    ));
                } else {
                    for lb in &state.lobbies {
                        left.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.12, 0.19, 0.92)),
                            BorderColor::all(Color::srgba(0.30, 0.42, 0.60, 0.5)),
                        ))
                        .with_children(|item| {
                            item.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(10.0),
                                ..default()
                            })
                            .with_children(|info| {
                                if lb.has_password {
                                    info.spawn((
                                        Node {
                                            width: Val::Px(12.0),
                                            height: Val::Px(12.0),
                                            border_radius: BorderRadius::all(Val::Px(3.0)),
                                            ..default()
                                        },
                                        BackgroundColor(gold),
                                    ));
                                }
                                info.spawn((
                                    Text::new(format!("{}  ({}/{})", lb.title, lb.current_players, lb.max_players)),
                                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });

                            spawn_action_button(
                                item,
                                "JOIN",
                                emerald,
                                PreGameBtnAction::OpenJoinLobby {
                                    lobby_id: lb.lobby_id.clone(),
                                    has_password: lb.has_password,
                                },
                            );
                        });
                    }
                }

                // مودال ورود با رمز عبور
                if state.join_target_lobby_id.is_some() {
                    left.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::all(Val::Px(14.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(10.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(12.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.10, 0.14, 0.22, 0.98)),
                        BorderColor::all(gold),
                    ))
                    .with_children(|modal| {
                        spawn_input_box(
                            modal,
                            "LOBBY PASSWORD",
                            &state.join_password_input,
                            "------",
                            state.focused == FocusedInput::JoinPassword,
                            PreGameBtnAction::FocusField(FocusedInput::JoinPassword),
                        );
                        modal.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|btns| {
                            spawn_action_button(btns, "ENTER", emerald, PreGameBtnAction::ConfirmJoinLobby);
                            spawn_action_button(btns, "CANCEL", Color::srgb(0.85, 0.28, 0.30), PreGameBtnAction::CancelJoinModal);
                        });
                    });
                }
            });

            // ستون راست: ساخت لابی جدید
            row.spawn((
                Node {
                    width: Val::Px(340.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(14.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(16.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
                BorderColor::all(emerald.with_alpha(0.55)),
            ))
            .with_children(|right| {
                right.spawn((
                    Text::new("CREATE LOBBY"),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::WHITE),
                ));
                spawn_input_box(
                    right,
                    "TITLE",
                    &state.create_title,
                    "Simorgh Room",
                    state.focused == FocusedInput::LobbyTitle,
                    PreGameBtnAction::FocusField(FocusedInput::LobbyTitle),
                );
                spawn_input_box(
                    right,
                    "PASSWORD (OPT)",
                    &state.create_password,
                    "Optional",
                    state.focused == FocusedInput::LobbyPassword,
                    PreGameBtnAction::FocusField(FocusedInput::LobbyPassword),
                );
                right.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|caps| {
                    for &cap in &[2usize, 4, 6, 8, 10] {
                        let active = state.create_capacity == cap;
                        spawn_nav_pill(caps, &cap.to_string(), active, PreGameBtnAction::SelectLobbyCapacity(cap));
                    }
                });
                spawn_action_button(right, "CREATE", emerald, PreGameBtnAction::CreateLobby);
            });
        });
}

fn spawn_lobby_room_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let Some(ref lobby) = state.current_lobby else {
        return;
    };
    let my_uid = state
        .profile
        .as_ref()
        .map(|p| p.user_id.as_str())
        .unwrap_or("");
    let is_host = lobby.host_user_id == my_uid;
    let dawn_col = Color::srgb(0.16, 0.85, 0.62);
    let dusk_col = Color::srgb(0.95, 0.28, 0.30);
    let gold = Color::srgb(0.98, 0.80, 0.26);
    let half = lobby.max_players / 2;

    parent
        .spawn(Node {
            width: Val::Percent(94.0),
            max_width: Val::Px(1140.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|wrap| {
            // هدر اتاق لابی
            wrap.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
                BorderColor::all(gold.with_alpha(0.55)),
            ))
            .with_children(|hdr| {
                hdr.spawn((
                    Text::new(format!("{}  -  {}/{}", lobby.title, lobby.members.len(), lobby.max_players)),
                    TextFont { font_size: FontSize::Px(18.0), ..default() },
                    TextColor(Color::WHITE),
                ));
                hdr.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|btns| {
                    if state.server_starting || lobby.is_starting {
                        spawn_conceptual_loader(btns, "STARTING SERVER", gold);
                    } else {
                        if is_host {
                            spawn_action_button(btns, "START MATCH", dawn_col, PreGameBtnAction::StartLobbyMatch);
                        }
                        spawn_action_button(btns, "LEAVE", dusk_col, PreGameBtnAction::LeaveLobby);
                    }
                });
            });

            // دو تیم در کنار هم + چت
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|cols| {
                spawn_team_slots_panel(cols, "DAWN (TEAM 1)", 1, half, lobby, is_host, my_uid, dawn_col);
                spawn_team_slots_panel(cols, "DUSK (TEAM 2)", 2, half, lobby, is_host, my_uid, dusk_col);

                // پنل چت لابی
                cols.spawn((
                    Node {
                        width: Val::Px(320.0),
                        padding: UiRect::all(Val::Px(14.0)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: Val::Px(10.0),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
                    BorderColor::all(Color::srgba(0.32, 0.44, 0.62, 0.55)),
                ))
                .with_children(|chat| {
                    chat.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|msgs| {
                        let start = lobby.chat_messages.len().saturating_sub(10);
                        for m in &lobby.chat_messages[start..] {
                            let col = if m.is_system { gold } else { Color::srgb(0.86, 0.92, 0.98) };
                            msgs.spawn((
                                Text::new(format!("{}: {}", m.sender, m.text)),
                                TextFont { font_size: FontSize::Px(12.5), ..default() },
                                TextColor(col),
                            ));
                        }
                    });

                    spawn_input_box(
                        chat,
                        "CHAT",
                        &state.lobby_chat_input,
                        "Message...",
                        state.focused == FocusedInput::LobbyChat,
                        PreGameBtnAction::FocusField(FocusedInput::LobbyChat),
                    );
                    spawn_action_button(chat, "SEND", Color::srgb(0.32, 0.72, 0.98), PreGameBtnAction::SendLobbyChat);
                });
            });
        });
}

fn spawn_team_slots_panel(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    team_id: i32,
    half: usize,
    lobby: &LobbyDetail,
    is_host: bool,
    my_uid: &str,
    accent: Color,
) {
    parent
        .spawn((
            Node {
                flex_grow: 1.0,
                padding: UiRect::all(Val::Px(16.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
            BorderColor::all(accent.with_alpha(0.6)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(title),
                TextFont { font_size: FontSize::Px(16.0), ..default() },
                TextColor(accent),
            ));

            for slot_idx in 0..half {
                let occupant = lobby
                    .members
                    .iter()
                    .find(|m| m.team_id == team_id && m.slot_index == slot_idx);
                if let Some(m) = occupant {
                    panel
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(9.0)),
                                ..default()
                            },
                            BackgroundColor(accent.with_alpha(0.14)),
                            BorderColor::all(accent.with_alpha(0.6)),
                        ))
                        .with_children(|row| {
                            let tag = if m.is_host { "* " } else { "" };
                            row.spawn((
                                Text::new(format!("{}{}", tag, m.username)),
                                TextFont { font_size: FontSize::Px(14.5), ..default() },
                                TextColor(Color::WHITE),
                            ));
                            if is_host && m.user_id != my_uid {
                                spawn_action_button(
                                    row,
                                    "KICK",
                                    Color::srgb(0.90, 0.25, 0.28),
                                    PreGameBtnAction::KickLobbyPlayer(m.user_id.clone()),
                                );
                            }
                        });
                } else {
                    panel
                        .spawn((
                            UiButton,
            Hovered::default(),
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(9.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.08, 0.11, 0.17, 0.7)),
                            BorderColor::all(Color::srgba(0.30, 0.38, 0.52, 0.45)),
                            PreGameBtnAction::SwitchLobbySlot {
                                team_id,
                                slot_index: slot_idx,
                            },
                        ))
                        .with_children(|slot| {
                            slot.spawn((
                                Text::new(format!("+ SLOT {}", slot_idx + 1)),
                                TextFont { font_size: FontSize::Px(13.5), ..default() },
                                TextColor(Color::srgb(0.55, 0.65, 0.78)),
                            ));
                        });
                }
            }
        });
}

fn spawn_history_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let gold = Color::srgb(0.98, 0.80, 0.26);
    let emerald = Color::srgb(0.16, 0.86, 0.58);
    let crimson = Color::srgb(0.95, 0.28, 0.30);

    if state.is_loading {
        spawn_conceptual_loader(parent, &state.loading_label, gold);
        return;
    }

    let Some(ref hist) = state.history else {
        return;
    };

    parent
        .spawn(Node {
            width: Val::Percent(94.0),
            max_width: Val::Px(1140.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|wrap| {
            // نوار خلاصه آمار کلی
            wrap.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|sum| {
                spawn_mini_stat_chip(sum, "WIN RATE", &format!("{:.1}%", hist.summary.win_rate_percent), emerald);
                spawn_mini_stat_chip(sum, "AVG KDA", &format!("{:.2}", hist.summary.avg_kda), gold);
                spawn_mini_stat_chip(sum, "MATCHES", &hist.summary.total_matches.to_string(), Color::srgb(0.32, 0.72, 0.98));
                spawn_mini_stat_chip(sum, "ABANDONS", &hist.summary.abandoned_count.to_string(), crimson);
            });

            // لیست نبردها + جدول جزئیات نبرد انتخابی
            wrap.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|cols| {
                cols.spawn(Node {
                    width: Val::Px(460.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|list| {
                    for (idx, m) in hist.matches.iter().take(8).enumerate() {
                        let out_col = match m.outcome.as_str() {
                            "win" => emerald,
                            "loss" => crimson,
                            "abandoned" => gold,
                            _ => Color::srgb(0.55, 0.65, 0.78),
                        };
                        let selected = idx == state.selected_history_idx;
                        list.spawn((
                            UiButton,
            Hovered::default(),
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                                border_radius: BorderRadius::all(Val::Px(10.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.06, 0.09, 0.15, 0.95)),
                            BorderColor::all(if selected { gold } else { out_col.with_alpha(0.55) }),
                            PreGameBtnAction::SelectHistoryMatch(idx),
                        ))
                        .with_children(|row| {
                            row.spawn((
                                Text::new(format!("{}  -  {}/{}/{}", m.outcome.to_uppercase(), m.kills, m.deaths, m.assists)),
                                TextFont { font_size: FontSize::Px(14.0), ..default() },
                                TextColor(out_col),
                            ));
                            row.spawn((
                                Text::new(format!("{:+} MMR", m.mmr_delta)),
                                TextFont { font_size: FontSize::Px(13.0), ..default() },
                                TextColor(Color::WHITE),
                            ));
                        });
                    }
                });

                // جدول بازیکنان نبرد انتخابی
                if let Some(sel) = hist.matches.get(state.selected_history_idx) {
                    cols.spawn((
                        Node {
                            flex_grow: 1.0,
                            padding: UiRect::all(Val::Px(16.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(14.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
                        BorderColor::all(gold.with_alpha(0.5)),
                    ))
                    .with_children(|sb| {
                        sb.spawn((
                            Text::new(format!("SCOREBOARD  -  {}  -  {}s", sel.played_at, sel.duration_sec)),
                            TextFont { font_size: FontSize::Px(14.5), ..default() },
                            TextColor(gold),
                        ));
                        for rp in &sel.players {
                            let t_col = if rp.team_id == 1 { emerald } else { crimson };
                            sb.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                    justify_content: JustifyContent::SpaceBetween,
                                    border_radius: BorderRadius::all(Val::Px(6.0)),
                                    ..default()
                                },
                                BackgroundColor(if rp.is_me { t_col.with_alpha(0.18) } else { Color::srgba(0.08, 0.11, 0.17, 0.7) }),
                            ))
                            .with_children(|r| {
                                r.spawn((
                                    Text::new(format!("{} (Lv.{})", rp.username, rp.level)),
                                    TextFont { font_size: FontSize::Px(13.5), ..default() },
                                    TextColor(t_col),
                                ));
                                r.spawn((
                                    Text::new(format!("{}/{}/{}  -  LH {}  -  {}g", rp.kills, rp.deaths, rp.assists, rp.last_hits, rp.gold_coins)),
                                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                                    TextColor(Color::WHITE),
                                ));
                            });
                        }
                    });
                }
            });
        });
}

// =============================================================================
// ویجت‌های کمکی
// =============================================================================

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

fn spawn_input_box(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    value: &str,
    placeholder: &str,
    is_focused: bool,
    action: PreGameBtnAction,
) {
    let border_col = if is_focused {
        Color::srgb(0.98, 0.80, 0.26)
    } else {
        Color::srgba(0.35, 0.48, 0.68, 0.6)
    };
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.09, 0.95)),
            BorderColor::all(border_col),
            action,
        ))
        .with_children(|f| {
            f.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(border_col),
            ));
            let display_txt = if value.is_empty() {
                placeholder.to_string()
            } else if is_focused {
                format!("{}|", value)
            } else {
                value.to_string()
            };
            f.spawn((
                Text::new(display_txt),
                TextFont { font_size: FontSize::Px(15.5), ..default() },
                TextColor(if value.is_empty() {
                    Color::srgb(0.45, 0.52, 0.64)
                } else {
                    Color::WHITE
                }),
            ));
        });
}

fn spawn_action_button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    color: Color,
    action: PreGameBtnAction,
) {
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                padding: UiRect::axes(Val::Px(22.0), Val::Px(11.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(color.with_alpha(0.22)),
            BorderColor::all(color),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(14.5), ..default() },
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_mini_stat_chip(parent: &mut ChildSpawnerCommands, label: &str, val: &str, color: Color) {
    parent
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(color.with_alpha(0.12)),
            BorderColor::all(color.with_alpha(0.6)),
        ))
        .with_children(|c| {
            c.spawn((
                Text::new(val),
                TextFont { font_size: FontSize::Px(17.0), ..default() },
                TextColor(Color::WHITE),
            ));
            c.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(color),
            ));
        });
}

#[allow(dead_code)]
fn rank_tier_color(tier: &str) -> Color {
    if tier.contains("Immortal") {
        Color::srgb(0.98, 0.35, 0.45)
    } else if tier.contains("Champion") {
        Color::srgb(0.82, 0.46, 0.98)
    } else if tier.contains("Golden") {
        Color::srgb(0.98, 0.80, 0.26)
    } else if tier.contains("Silver") {
        Color::srgb(0.75, 0.84, 0.95)
    } else if tier.contains("Initiate") {
        Color::srgb(0.24, 0.86, 0.62)
    } else {
        Color::srgb(0.78, 0.52, 0.32)
    }
}