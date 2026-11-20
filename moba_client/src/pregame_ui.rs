use crate::backend_client::sanitize_ascii_msg;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::app::AppExit;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button as UiButton;
use bevy::window::{CursorOptions, PrimaryWindow};
use std::f32::consts::TAU;

use crate::backend_client::{
    clear_persisted_session, load_persisted_session, ActiveMatchInfo, BackendAsyncEvent,
    BackendClient, LobbyDetail, LobbySummary, MatchHistoryResponse, PlayerProfile,
};

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
pub enum FocusedInput {
    None,
    Email,
    Otp,
    LobbyTitle,
    LobbyPassword,
    JoinPassword,
    LobbyChat,
}

#[derive(Resource, Default)]
pub struct PendingMatchConnection(pub Option<ActiveMatchInfo>);

#[derive(Resource)]
pub struct PreGameSessionState {
    pub access_token: Option<String>,
    pub profile: Option<PlayerProfile>,
    pub active_match: Option<ActiveMatchInfo>,
    pub tab: PreGameTab,
    pub focused: FocusedInput,
    pub email_input: String,
    pub otp_input: String,
    pub is_loading: bool,
    pub loading_label: String,
    pub toast_message: String,
    pub toast_is_error: bool,
    pub toast_timer: f32,
    // Matchmaking state
    pub queue_players: usize,
    pub queue_required: usize,
    pub server_starting: bool,
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
}

impl Default for PreGameSessionState {
    fn default() -> Self {
        Self {
            access_token: None,
            profile: None,
            active_match: None,
            tab: PreGameTab::AuthEmail,
            focused: FocusedInput::Email,
            email_input: String::new(),
            otp_input: String::new(),
            is_loading: false,
            loading_label: String::new(),
            toast_message: String::new(),
            toast_is_error: false,
            toast_timer: 0.0,
            queue_players: 0,
            queue_required: 2,
            server_starting: false,
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

#[derive(Component)]
struct PreGameEntity;

#[derive(Component)]
struct PreGameHiddenHud(Visibility);

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
enum PreGameBtnAction {
    RequestOtp,
    VerifyOtp,
    BackToEmail,
    SwitchTab(PreGameTab),
    StartFindMatch,
    CancelFindMatch,
    ReconnectActiveMatch,
    AbandonActiveMatch,
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
            .add_systems(OnEnter(AppScreenState::PreGame), on_enter_pregame)
            .add_systems(OnExit(AppScreenState::PreGame), on_exit_pregame)
            .add_systems(
                Update,
                (
                    poll_backend_events_system,
                    handle_pregame_keyboard_system,
                    handle_pregame_buttons_system,
                    animate_pregame_loaders_system,
                    rebuild_pregame_ui_system,
                )
                    .chain()
                    .run_if(in_state(AppScreenState::PreGame)),
            );
    }
}

fn on_enter_pregame(
    mut commands: Commands,
    backend: Res<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut cursor_q: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut hud_roots: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>, Without<PreGameEntity>)>,
) {
    for mut copt in &mut cursor_q {
        copt.visible = true;
    }
    for (ent, mut vis) in &mut hud_roots {
        commands.entity(ent).insert(PreGameHiddenHud(*vis));
        *vis = Visibility::Hidden;
    }
    // دوربین ۲بعدی سبک اختصاصی منوی پیش از بازی (فقط در زمان PreGame فعال است)
    commands.spawn((
        Camera2d,
        Camera {
            order: 10,
            ..default()
        },
        PreGameEntity,
    ));

    // Layered Jungle, River & Mystic Pond atmospheric backdrop
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(22.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.022, 0.085, 0.095)),
            PreGameEntity,
        ))
        .with_children(|root| {
            // Upper emerald jungle canopy aura
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(46.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.24, 0.15, 0.42)),
            ));
            // Lower turquoise river & pond water reflection band
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(36.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.03, 0.28, 0.31, 0.45)),
            ));
            // Mystic horizon river mist band
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(62.0),
                    left: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.24, 0.84, 0.72, 0.16)),
            ));
            // Floating jungle fireflies
            for i in 0..14 {
                let fx = 6.0 + ((i * 19) % 88) as f32;
                let fy = 10.0 + ((i * 23) % 78) as f32;
                let col = if i % 3 == 0 {
                    Color::srgba(0.98, 0.82, 0.30, 0.65)
                } else {
                    Color::srgba(0.25, 0.92, 0.68, 0.55)
                };
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(fx),
                        top: Val::Percent(fy),
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(col),
                    JungleFirefly {
                        base_x_pct: fx,
                        base_y_pct: fy,
                        phase: i as f32 * 0.9,
                        speed: 0.7 + (i % 4) as f32 * 0.25,
                    },
                ));
            }
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(14.0),
                    ..default()
                },
                PreGameDynamicRoot,
            ));
        });

    // بازیابی خودکار نشست ذخیره‌شده
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
        backend.restore_session(tok);
    }
    state.ui_dirty = true;
}

fn on_exit_pregame(
    mut commands: Commands,
    query: Query<Entity, With<PreGameEntity>>,
    mut hidden_huds: Query<(Entity, &mut Visibility, &PreGameHiddenHud)>,
) {
    for ent in &query {
        commands.entity(ent).despawn();
    }
    for (ent, mut vis, saved) in &mut hidden_huds {
        *vis = saved.0;
        commands.entity(ent).remove::<PreGameHiddenHud>();
    }
}

// =============================================================================
// دریافت رویدادهای غیرمسدودکننده بک‌اند و بروزرسانی وضعیت
// =============================================================================

fn poll_backend_events_system(
    time: Res<Time>,
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut pending_conn: ResMut<PendingMatchConnection>,
    mut next_screen: ResMut<NextState<AppScreenState>>,
) {
    if state.toast_timer > 0.0 {
        state.toast_timer = (state.toast_timer - time.delta_secs()).max(0.0);
        if state.toast_timer <= 0.0 {
            state.toast_message.clear();
            state.ui_dirty = true;
        }
    }

    let events = backend.poll_events();
    if events.is_empty() {
        return;
    }

    for ev in events {
        state.ui_dirty = true;
        match ev {
            BackendAsyncEvent::OtpRequested { email } => {
                state.is_loading = false;
                state.email_input = email;
                state.tab = PreGameTab::AuthOtp;
                state.focused = FocusedInput::Otp;
                set_toast(&mut state, "6-digit verification code sent", false);
            }
            BackendAsyncEvent::OtpVerified {
                access_token,
                refresh_token: _,
                profile,
                active_match,
                is_new_player: _,
            } => {
                state.is_loading = false;
                state.access_token = Some(access_token);
                state.profile = Some(profile);
                state.active_match = active_match;
                state.otp_input.clear();
                state.tab = PreGameTab::Hub;
                state.focused = FocusedInput::None;
            }
            BackendAsyncEvent::SessionRestored {
                access_token,
                profile,
                active_match,
            } => {
                state.is_loading = false;
                state.access_token = Some(access_token);
                state.profile = Some(profile);
                state.active_match = active_match;
                state.tab = PreGameTab::Hub;
                state.focused = FocusedInput::None;
            }
            BackendAsyncEvent::SessionInvalid => {
                state.is_loading = false;
                state.access_token = None;
                state.profile = None;
                state.tab = PreGameTab::AuthEmail;
                state.focused = FocusedInput::Email;
            }
            BackendAsyncEvent::ActiveMatchChecked(am) => {
                state.active_match = am;
            }
            BackendAsyncEvent::MatchDisconnected(am) => {
                state.is_loading = false;
                state.active_match = am;
            }
            BackendAsyncEvent::MatchReconnected(am) => {
                state.is_loading = false;
                if let Some(info) = am {
                    pending_conn.0 = Some(info);
                    next_screen.set(AppScreenState::InGame);
                } else {
                    state.active_match = None;
                    set_toast(&mut state, "No active match found", true);
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
                set_toast(&mut state, "Match abandoned (-25 MMR)", true);
            }
            BackendAsyncEvent::HistoryLoaded(hist) => {
                state.is_loading = false;
                state.history = Some(hist);
                state.selected_history_idx = 0;
            }
            BackendAsyncEvent::HttpError(err) => {
                state.is_loading = false;
                set_toast(&mut state, &err, true);
            }
            BackendAsyncEvent::QueueStatus {
                players_in_queue,
                required_players,
            } => {
                state.queue_players = players_in_queue;
                state.queue_required = required_players;
                state.server_starting = false;
                state.tab = PreGameTab::FindingMatch;
            }
            BackendAsyncEvent::StartingGameServer {
                room_id: _,
                port: _,
                players_in_queue,
                required_players,
            } => {
                state.queue_players = players_in_queue;
                state.queue_required = required_players;
                state.server_starting = true;
                state.tab = PreGameTab::FindingMatch;
            }
            BackendAsyncEvent::MatchFound(info) => {
                state.is_loading = false;
                state.server_starting = false;
                state.queue_players = 0;
                state.tab = PreGameTab::Hub;
                state.confirm_modal = None;
                backend.disconnect_lobby_ws();
                pending_conn.0 = Some(info);
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
                set_toast(&mut state, &message, true);
            }
            BackendAsyncEvent::MatchmakingClosed => {
                if state.tab == PreGameTab::FindingMatch && !state.server_starting {
                    state.tab = PreGameTab::Hub;
                }
            }
            BackendAsyncEvent::LobbyList { lobbies, message } => {
                state.is_loading = false;
                state.lobbies = lobbies;
                if state.tab != PreGameTab::LobbyRoom {
                    state.tab = PreGameTab::LobbyBrowser;
                }
                if !message.is_empty() {
                    set_toast(&mut state, &message, false);
                }
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
            }
            BackendAsyncEvent::LobbyStarting { lobby, message } => {
                state.current_lobby = Some(lobby);
                state.server_starting = true;
                state.tab = PreGameTab::LobbyRoom;
                if !message.is_empty() {
                    set_toast(&mut state, &message, false);
                }
            }
            BackendAsyncEvent::KickedFromLobby { lobbies, message }
            | BackendAsyncEvent::LeftLobby { lobbies, message } => {
                state.current_lobby = None;
                state.lobbies = lobbies;
                state.tab = PreGameTab::LobbyBrowser;
                if !message.is_empty() {
                    set_toast(&mut state, &message, true);
                }
            }
            BackendAsyncEvent::LobbyClosed { message } => {
                state.current_lobby = None;
                state.tab = PreGameTab::Hub;
                backend.disconnect_lobby_ws();
                if !message.is_empty() {
                    set_toast(&mut state, &message, true);
                }
            }
            BackendAsyncEvent::LobbyError(msg) => {
                state.is_loading = false;
                set_toast(&mut state, &msg, true);
            }
            BackendAsyncEvent::LobbyWsClosed => {
                if matches!(state.tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom) {
                    state.current_lobby = None;
                    state.tab = PreGameTab::Hub;
                }
            }
        }
    }
}

fn set_toast(state: &mut PreGameSessionState, msg: &str, is_error: bool) {
    state.toast_message = msg.to_string();
    state.toast_is_error = is_error;
    state.toast_timer = 4.0;
}

// =============================================================================
// ورودی کیبورد برای فیلدهای ایمیل، OTP، لابی و چت
// =============================================================================

fn handle_pregame_keyboard_system(
    mut key_ev: MessageReader<KeyboardInput>,
    backend: Res<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
) {
    if state.focused == FocusedInput::None {
        return;
    }

    for ev in key_ev.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => {
                let target = active_input_mut(&mut state);
                if target.pop().is_some() {
                    state.ui_dirty = true;
                }
            }
            Key::Enter => {
                match state.focused {
                    FocusedInput::Email => {
                        let email = state.email_input.trim().to_string();
                        if !email.is_empty() && !state.is_loading {
                            state.is_loading = true;
                            state.loading_label = "SENDING CODE".to_string();
                            state.ui_dirty = true;
                            backend.request_otp(email);
                        }
                    }
                    FocusedInput::Otp => {
                        let email = state.email_input.trim().to_string();
                        let otp = state.otp_input.trim().to_string();
                        if otp.len() == 6 && !state.is_loading {
                            state.is_loading = true;
                            state.loading_label = "VERIFYING".to_string();
                            state.ui_dirty = true;
                            backend.verify_otp(email, otp);
                        }
                    }
                    FocusedInput::LobbyChat => {
                        let txt = state.lobby_chat_input.trim().to_string();
                        if !txt.is_empty() {
                            backend.send_lobby_action(serde_json::json!({
                                "action": "chat_message",
                                "text": txt,
                            }));
                            state.lobby_chat_input.clear();
                            state.ui_dirty = true;
                        }
                    }
                    FocusedInput::JoinPassword => {
                        if let Some(ref lid) = state.join_target_lobby_id.clone() {
                            backend.send_lobby_action(serde_json::json!({
                                "action": "join_lobby",
                                "lobby_id": lid,
                                "password": state.join_password_input.trim(),
                            }));
                        }
                    }
                    _ => {}
                }
            }
            Key::Character(s) => {
                let max_len = match state.focused {
                    FocusedInput::Otp => 6,
                    FocusedInput::Email => 64,
                    FocusedInput::LobbyTitle => 36,
                    FocusedInput::LobbyPassword | FocusedInput::JoinPassword => 24,
                    FocusedInput::LobbyChat => 120,
                    FocusedInput::None => 0,
                };
                let otp_only = state.focused == FocusedInput::Otp;
                let mut pushed = false;
                {
                    let target = active_input_mut(&mut state);
                    for ch in s.chars() {
                        if ch.is_control() {
                            continue;
                        }
                        if otp_only && !ch.is_ascii_digit() {
                            continue;
                        }
                        if target.chars().count() < max_len {
                            target.push(ch);
                            pushed = true;
                        }
                    }
                }
                if pushed {
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
        FocusedInput::Otp => &mut state.otp_input,
        FocusedInput::LobbyTitle => &mut state.create_title,
        FocusedInput::LobbyPassword => &mut state.create_password,
        FocusedInput::JoinPassword => &mut state.join_password_input,
        FocusedInput::LobbyChat => &mut state.lobby_chat_input,
        FocusedInput::None => &mut state.email_input,
    }
}

// =============================================================================
// تعامل دکمه‌ها
// =============================================================================

fn handle_pregame_buttons_system(
    mut backend: ResMut<BackendClient>,
    mut state: ResMut<PreGameSessionState>,
    mut pending_conn: ResMut<PendingMatchConnection>,
    mut next_screen: ResMut<NextState<AppScreenState>>,
    mut exit_ev: MessageWriter<AppExit>,
    mut btn_q: Query<(&Hovered, Has<Pressed>, &PreGameBtnAction, &mut BackgroundColor), Or<(Changed<Hovered>, Added<Pressed>)>>,
) {
    for (hovered, pressed, action, mut bg) in &mut btn_q {
        if hovered.get() && !pressed {
            bg.0 = bg.0.lighter(0.08);
        }
        if !pressed {
            continue;
        }
        state.ui_dirty = true;
        match action {
            PreGameBtnAction::FocusField(f) => {
                state.focused = *f;
            }
            PreGameBtnAction::RequestOtp => {
                let email = state.email_input.trim().to_string();
                if email.contains('@') && !state.is_loading {
                    state.is_loading = true;
                    state.loading_label = "SENDING CODE".to_string();
                    backend.request_otp(email);
                } else {
                    set_toast(&mut state, "Enter a valid email address", true);
                }
            }
            PreGameBtnAction::VerifyOtp => {
                let email = state.email_input.trim().to_string();
                let otp = state.otp_input.trim().to_string();
                if otp.len() == 6 && !state.is_loading {
                    state.is_loading = true;
                    state.loading_label = "VERIFYING".to_string();
                    backend.verify_otp(email, otp);
                } else {
                    set_toast(&mut state, "Enter the full 6-digit code", true);
                }
            }
            PreGameBtnAction::BackToEmail => {
                state.tab = PreGameTab::AuthEmail;
                state.focused = FocusedInput::Email;
            }
            PreGameBtnAction::SwitchTab(tab) => {
                state.tab = *tab;
                state.focused = FocusedInput::None;
                match tab {
                    PreGameTab::LobbyBrowser => {
                        if let Some(tok) = state.access_token.clone() {
                            state.is_loading = true;
                            state.loading_label = "LOADING LOBBIES".to_string();
                            backend.connect_lobby_ws(tok.clone());
                        }
                    }
                    PreGameTab::MatchHistory => {
                        backend.disconnect_lobby_ws();
                        if let Some(tok) = state.access_token.clone() {
                            state.is_loading = true;
                            state.loading_label = "LOADING HISTORY".to_string();
                            backend.fetch_match_history(tok.clone());
                        }
                    }
                    PreGameTab::Hub => {
                        backend.disconnect_lobby_ws();
                        if let Some(tok) = state.access_token.clone() {
                            backend.fetch_active_match(tok.clone());
                        }
                    }
                    _ => {}
                }
            }
            PreGameBtnAction::StartFindMatch => {
                if let Some(tok) = state.access_token.clone() {
                    state.queue_players = 1;
                    state.queue_required = 2;
                    state.server_starting = false;
                    state.tab = PreGameTab::FindingMatch;
                    backend.start_matchmaking(tok.clone());
                }
            }
            PreGameBtnAction::CancelFindMatch => {
                backend.stop_matchmaking();
                state.server_starting = false;
                state.tab = PreGameTab::Hub;
            }
            PreGameBtnAction::ReconnectActiveMatch => {
                if let Some(tok) = state.access_token.clone() {
                    state.is_loading = true;
                    state.loading_label = "RECONNECTING".to_string();
                    backend.reconnect_match(tok.clone());
                } else if let Some(info) = state.active_match.clone() {
                    pending_conn.0 = Some(info);
                    next_screen.set(AppScreenState::InGame);
                }
            }
            PreGameBtnAction::AbandonActiveMatch => {
                if let Some(tok) = state.access_token.clone() {
                    state.is_loading = true;
                    state.loading_label = "ABANDONING".to_string();
                    backend.abandon_match(tok.clone());
                }
            }
            PreGameBtnAction::RefreshLobbies => {
                backend.send_lobby_action(serde_json::json!({ "action": "list_lobbies" }));
            }
            PreGameBtnAction::SelectLobbyCapacity(cap) => {
                state.create_capacity = *cap;
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
                if *has_password {
                    state.join_target_lobby_id = Some(lobby_id.clone());
                    state.join_password_input.clear();
                    state.focused = FocusedInput::JoinPassword;
                } else {
                    backend.send_lobby_action(serde_json::json!({
                        "action": "join_lobby",
                        "lobby_id": lobby_id,
                        "password": "",
                    }));
                }
            }
            PreGameBtnAction::ConfirmJoinLobby => {
                if let Some(ref lid) = state.join_target_lobby_id.clone() {
                    backend.send_lobby_action(serde_json::json!({
                        "action": "join_lobby",
                        "lobby_id": lid,
                        "password": state.join_password_input.trim(),
                    }));
                }
            }
            PreGameBtnAction::CancelJoinModal => {
                state.join_target_lobby_id = None;
                state.join_password_input.clear();
                state.focused = FocusedInput::None;
            }
            PreGameBtnAction::SwitchLobbySlot {
                team_id,
                slot_index,
            } => {
                backend.send_lobby_action(serde_json::json!({
                    "action": "switch_team",
                    "team_id": team_id,
                    "slot_index": slot_index,
                }));
            }
            PreGameBtnAction::KickLobbyPlayer(uid) => {
                backend.send_lobby_action(serde_json::json!({
                    "action": "kick_player",
                    "target_user_id": uid,
                }));
            }
            PreGameBtnAction::SendLobbyChat => {
                let txt = state.lobby_chat_input.trim().to_string();
                if !txt.is_empty() {
                    backend.send_lobby_action(serde_json::json!({
                        "action": "chat_message",
                        "text": txt,
                    }));
                    state.lobby_chat_input.clear();
                }
            }
            PreGameBtnAction::StartLobbyMatch => {
                backend.send_lobby_action(serde_json::json!({ "action": "start_match" }));
            }
            PreGameBtnAction::LeaveLobby => {
                backend.send_lobby_action(serde_json::json!({ "action": "leave_lobby" }));
            }
            PreGameBtnAction::SelectHistoryMatch(idx) => {
                state.selected_history_idx = *idx;
            }
            PreGameBtnAction::RequestLogoutConfirm => {
                state.confirm_modal = Some(ConfirmAction::Logout);
            }
            PreGameBtnAction::RequestQuitConfirm => {
                state.confirm_modal = Some(ConfirmAction::QuitApp);
            }
            PreGameBtnAction::ConfirmModalCancel => {
                state.confirm_modal = None;
            }
            PreGameBtnAction::ConfirmModalAccept => {
                match state.confirm_modal {
                    Some(ConfirmAction::Logout) => {
                        clear_persisted_session();
                        backend.stop_matchmaking();
                        backend.disconnect_lobby_ws();
                        *state = PreGameSessionState::default();
                    }
                    Some(ConfirmAction::QuitApp) => {
                        backend.stop_matchmaking();
                        backend.disconnect_lobby_ws();
                        exit_ev.write(AppExit::Success);
                    }
                    None => {}
                }
            }
        }
    }
}

// =============================================================================
// انیمیشن لودینگ‌های مفهومی (نقاط مداری و نوارهای پالسی)
// =============================================================================

fn animate_pregame_loaders_system(
    time: Res<Time>,
    mut dot_q: Query<(&mut Node, &mut BackgroundColor, &ConceptualOrbitDot)>,
    mut pulse_q: Query<(&mut BackgroundColor, &PulseGlowNode), (Without<ConceptualOrbitDot>, Without<JungleFirefly>)>,
    mut firefly_q: Query<(&mut Node, &mut BackgroundColor, &JungleFirefly), Without<ConceptualOrbitDot>>,
) {
    let t = time.elapsed_secs();
    for (mut node, mut bg, dot) in &mut dot_q {
        let angle = t * 3.4 + dot.phase;
        node.left = Val::Px(dot.radius + angle.cos() * dot.radius - 5.0);
        node.top = Val::Px(dot.radius + angle.sin() * dot.radius - 5.0);
        let alpha = 0.45 + 0.55 * (angle.sin() * 0.5 + 0.5);
        bg.0 = bg.0.with_alpha(alpha);
    }
    for (mut bg, p) in &mut pulse_q {
        let a = 0.72 + 0.28 * (t * p.speed).sin();
        bg.0 = bg.0.with_alpha(a);
    }
    for (mut node, mut bg, ff) in &mut firefly_q {
        let dx = (t * ff.speed + ff.phase).sin() * 1.4;
        let dy = (t * ff.speed * 0.8 + ff.phase * 1.3).cos() * 1.8;
        node.left = Val::Percent(ff.base_x_pct + dx);
        node.top = Val::Percent(ff.base_y_pct + dy);
        let alpha = 0.25 + 0.55 * ((t * ff.speed * 1.5 + ff.phase).sin() * 0.5 + 0.5);
        bg.0 = bg.0.with_alpha(alpha);
    }
}

// =============================================================================
// رندر کامل رابط کاربری پیش از بازی (آیکون‌محور، کم‌نوشته، طراحی Simorgh MOBA)
// =============================================================================

fn rebuild_pregame_ui_system(
    mut commands: Commands,
    mut state: ResMut<PreGameSessionState>,
    root_q: Query<Entity, With<PreGameDynamicRoot>>,
    children_q: Query<&Children>,
) {
    if !state.ui_dirty {
        return;
    }
    let Ok(root_ent) = root_q.single() else {
        return;
    };
    state.ui_dirty = false;

    if let Ok(children) = children_q.get(root_ent) {
        for c in children.iter() {
            commands.entity(c).despawn();
        }
    }

    commands.entity(root_ent).with_children(|root| {
        // نوار بالای ثابت پس از ورود به حساب
        if let Some(ref profile) = state.profile {
            spawn_top_nav_bar(root, profile, state.tab);
        }

        // محتوای اصلی هر صفحه
        match state.tab {
            PreGameTab::AuthEmail | PreGameTab::AuthOtp => spawn_auth_screen(root, &state),
            PreGameTab::Hub => spawn_hub_screen(root, &state),
            PreGameTab::FindingMatch => spawn_finding_match_screen(root, &state),
            PreGameTab::LobbyBrowser => spawn_lobby_browser_screen(root, &state),
            PreGameTab::LobbyRoom => spawn_lobby_room_screen(root, &state),
            PreGameTab::MatchHistory => spawn_history_screen(root, &state),
        }

        if let Some(confirm) = state.confirm_modal {
            let (title, yes_label, accent) = match confirm {
                ConfirmAction::Logout => ("SIGN OUT OF ACCOUNT?", "LOGOUT", Color::srgb(0.98, 0.62, 0.22)),
                ConfirmAction::QuitApp => ("QUIT SIMORGH ARENA?", "QUIT GAME", Color::srgb(0.92, 0.26, 0.30)),
            };
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.03, 0.04, 0.78)),
                GlobalZIndex(300),
            ))
            .with_children(|backdrop| {
                backdrop
                    .spawn((
                        Node {
                            width: Val::Px(380.0),
                            padding: UiRect::all(Val::Px(26.0)),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(18.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(16.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.98)),
                        BorderColor::all(accent),
                    ))
                    .with_children(|box_node| {
                        box_node.spawn((
                            Text::new(title),
                            TextFont { font_size: FontSize::Px(18.0), ..default() },
                            TextColor(Color::WHITE),
                        ));
                        box_node.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(14.0),
                            ..default()
                        })
                        .with_children(|btns| {
                            spawn_action_button(btns, yes_label, accent, PreGameBtnAction::ConfirmModalAccept);
                            spawn_action_button(btns, "CANCEL", Color::srgb(0.35, 0.48, 0.56), PreGameBtnAction::ConfirmModalCancel);
                        });
                    });
            });
        }

        if !state.toast_message.is_empty() {
            let (bg, border) = if state.toast_is_error {
                (Color::srgba(0.28, 0.06, 0.08, 0.96), Color::srgb(0.95, 0.30, 0.32))
            } else {
                (Color::srgba(0.06, 0.22, 0.16, 0.96), Color::srgb(0.24, 0.88, 0.58))
            };
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(bg),
                BorderColor::all(border),
            ))
            .with_children(|t| {
                t.spawn((
                    Text::new(sanitize_ascii_msg(&state.toast_message, "Notice")),
                    TextFont { font_size: FontSize::Px(15.0), ..default() },
                    TextColor(Color::WHITE),
                ));
            });
        }
    });
}

fn spawn_top_nav_bar(parent: &mut ChildSpawnerCommands, profile: &PlayerProfile, active_tab: PreGameTab) {
    let tier_col = rank_tier_color(&profile.rank_tier);
    parent
        .spawn((
            Node {
                width: Val::Percent(94.0),
                max_width: Val::Px(1180.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(12.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
            BorderColor::all(Color::srgba(0.25, 0.35, 0.50, 0.6)),
        ))
        .with_children(|bar| {
            // سمت چپ: نشان رنک + نام کاربری + MMR
            bar.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|left| {
                // آیکون سپر رنگی رنک
                left.spawn((
                    Node {
                        width: Val::Px(38.0),
                        height: Val::Px(38.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(tier_col.with_alpha(0.18)),
                    BorderColor::all(tier_col),
                ))
                .with_children(|b| {
                    b.spawn((
                        Text::new("S"),
                        TextFont { font_size: FontSize::Px(18.0), ..default() },
                        TextColor(tier_col),
                    ));
                });

                left.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|info| {
                    info.spawn((
                        Text::new(profile.username.clone()),
                        TextFont { font_size: FontSize::Px(17.0), ..default() },
                        TextColor(Color::WHITE),
                    ));
                    info.spawn((
                        Text::new(format!("{}  -  {} MMR", profile.rank_tier, profile.mmr)),
                        TextFont { font_size: FontSize::Px(12.0), ..default() },
                        TextColor(tier_col),
                    ));
                });
            });

            // سمت راست: دکمه‌های آیکون‌محور ناوبری
            bar.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|nav| {
                spawn_nav_pill(nav, "PLAY", Color::srgb(0.18, 0.85, 0.58), active_tab == PreGameTab::Hub, PreGameBtnAction::SwitchTab(PreGameTab::Hub));
                spawn_nav_pill(nav, "LOBBY", Color::srgb(0.32, 0.72, 0.98), matches!(active_tab, PreGameTab::LobbyBrowser | PreGameTab::LobbyRoom), PreGameBtnAction::SwitchTab(PreGameTab::LobbyBrowser));
                spawn_nav_pill(nav, "HISTORY", Color::srgb(0.98, 0.78, 0.26), active_tab == PreGameTab::MatchHistory, PreGameBtnAction::SwitchTab(PreGameTab::MatchHistory));
                spawn_nav_pill(nav, "LOGOUT", Color::srgb(0.98, 0.62, 0.22), false, PreGameBtnAction::RequestLogoutConfirm);
                spawn_nav_pill(nav, "QUIT", Color::srgb(0.92, 0.26, 0.30), false, PreGameBtnAction::RequestQuitConfirm);
            });
        });
}

fn spawn_nav_pill(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    accent: Color,
    is_active: bool,
    action: PreGameBtnAction,
) {
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(if is_active { accent.with_alpha(0.22) } else { Color::srgba(0.08, 0.12, 0.18, 0.9) }),
            BorderColor::all(if is_active { accent } else { accent.with_alpha(0.35) }),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Node {
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(accent),
            ));
            b.spawn((
                Text::new(label),
                TextFont { font_size: FontSize::Px(13.5), ..default() },
                TextColor(if is_active { Color::WHITE } else { Color::srgb(0.82, 0.88, 0.95) }),
            ));
        });
}

fn spawn_auth_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let gold = Color::srgb(0.98, 0.80, 0.26);
    parent
        .spawn((
            Node {
                width: Val::Px(420.0),
                padding: UiRect::all(Val::Px(32.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(18.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(18.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
            BorderColor::all(gold.with_alpha(0.65)),
        ))
        .with_children(|card| {
            // نشان سیمرغ
            card.spawn((
                Node {
                    width: Val::Px(64.0),
                    height: Val::Px(64.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(2.5)),
                    border_radius: BorderRadius::all(Val::Px(32.0)),
                    ..default()
                },
                BackgroundColor(gold.with_alpha(0.14)),
                BorderColor::all(gold),
            ))
            .with_children(|em| {
                em.spawn((
                    Text::new("S"),
                    TextFont { font_size: FontSize::Px(30.0), ..default() },
                    TextColor(gold),
                ));
            });

            card.spawn((
                Text::new("SIMORGH ARENA"),
                TextFont { font_size: FontSize::Px(24.0), ..default() },
                TextColor(Color::WHITE),
            ));

            if state.is_loading {
                spawn_conceptual_loader(card, &state.loading_label, gold);
            } else if state.tab == PreGameTab::AuthEmail {
                spawn_input_box(
                    card,
                    "EMAIL",
                    &state.email_input,
                    "player@example.com",
                    state.focused == FocusedInput::Email,
                    PreGameBtnAction::FocusField(FocusedInput::Email),
                );
                spawn_action_button(card, "SEND OTP", Color::srgb(0.18, 0.82, 0.56), PreGameBtnAction::RequestOtp);
            } else {
                card.spawn((
                    Text::new(state.email_input.clone()),
                    TextFont { font_size: FontSize::Px(13.0), ..default() },
                    TextColor(Color::srgb(0.65, 0.78, 0.92)),
                ));
                spawn_input_box(
                    card,
                    "6-DIGIT CODE",
                    &state.otp_input,
                    "- - - - - -",
                    state.focused == FocusedInput::Otp,
                    PreGameBtnAction::FocusField(FocusedInput::Otp),
                );
                spawn_action_button(card, "VERIFY & ENTER", gold, PreGameBtnAction::VerifyOtp);
                spawn_action_button(card, "CHANGE EMAIL", Color::srgb(0.35, 0.42, 0.55), PreGameBtnAction::BackToEmail);
            }
            if !state.is_loading {
                spawn_action_button(card, "QUIT GAME", Color::srgb(0.85, 0.26, 0.30), PreGameBtnAction::RequestQuitConfirm);
            }
        });
}

fn spawn_hub_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let gold = Color::srgb(0.98, 0.80, 0.26);
    let emerald = Color::srgb(0.16, 0.86, 0.58);
    let crimson = Color::srgb(0.95, 0.28, 0.30);

    parent
        .spawn(Node {
            width: Val::Percent(92.0),
            max_width: Val::Px(900.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(22.0),
            ..default()
        })
        .with_children(|hub| {
            // بنر بازی فعال (Reconnect / Abandon مشابه Dota 2)
            if let Some(ref am) = state.active_match {
                hub.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(22.0), Val::Px(16.0)),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(14.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.22, 0.12, 0.04, 0.95)),
                    BorderColor::all(gold),
                ))
                .with_children(|banner| {
                    banner.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(12.0),
                        ..default()
                    })
                    .with_children(|l| {
                        l.spawn((
                            Node {
                                width: Val::Px(14.0),
                                height: Val::Px(14.0),
                                border_radius: BorderRadius::all(Val::Px(7.0)),
                                ..default()
                            },
                            BackgroundColor(gold),
                            PulseGlowNode { speed: 4.0 },
                        ));
                        l.spawn((
                            Text::new(format!("ACTIVE MATCH  -  {}", am.room_id)),
                            TextFont { font_size: FontSize::Px(16.0), ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });

                    banner.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|btns| {
                        spawn_action_button(btns, "RECONNECT", emerald, PreGameBtnAction::ReconnectActiveMatch);
                        spawn_action_button(btns, "ABANDON (-25)", crimson, PreGameBtnAction::AbandonActiveMatch);
                    });
                });
            }

            // کارت اصلی شروع نبرد
            hub.spawn((
                Node {
                    width: Val::Px(460.0),
                    padding: UiRect::all(Val::Px(34.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(20.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(20.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
                BorderColor::all(emerald.with_alpha(0.7)),
            ))
            .with_children(|card| {
                if state.is_loading {
                    spawn_conceptual_loader(card, &state.loading_label, emerald);
                } else {
                    if let Some(ref p) = state.profile {
                        card.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(16.0),
                            ..default()
                        })
                        .with_children(|stats| {
                            spawn_mini_stat_chip(stats, "WINS", &p.wins.to_string(), emerald);
                            spawn_mini_stat_chip(stats, "LOSSES", &p.losses.to_string(), crimson);
                            spawn_mini_stat_chip(stats, "MMR", &p.mmr.to_string(), gold);
                        });
                    }

                    if state.active_match.is_none() {
                        spawn_action_button(card, "FIND MATCH", emerald, PreGameBtnAction::StartFindMatch);
                    }
                    spawn_action_button(card, "CUSTOM LOBBIES", Color::srgb(0.32, 0.72, 0.98), PreGameBtnAction::SwitchTab(PreGameTab::LobbyBrowser));
                }
            });
        });
}

fn spawn_finding_match_screen(parent: &mut ChildSpawnerCommands, state: &PreGameSessionState) {
    let accent = if state.server_starting {
        Color::srgb(0.98, 0.80, 0.26)
    } else {
        Color::srgb(0.24, 0.86, 0.98)
    };
    let label = if state.server_starting {
        "BOOTING ARENA SERVER"
    } else {
        "FINDING MATCH"
    };

    parent
        .spawn((
            Node {
                width: Val::Px(440.0),
                padding: UiRect::all(Val::Px(34.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(22.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.105, 0.115, 0.90)),
            BorderColor::all(accent),
        ))
        .with_children(|card| {
            spawn_conceptual_loader(card, label, accent);

            // نشانگر بصری جایگاه‌های پرشده در صف (Pips)
            card.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|pips| {
                let total = state.queue_required.max(1).min(10);
                for i in 0..total {
                    let filled = i < state.queue_players;
                    pips.spawn((
                        Node {
                            width: Val::Px(22.0),
                            height: Val::Px(22.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(11.0)),
                            ..default()
                        },
                        BackgroundColor(if filled { accent } else { Color::srgba(0.12, 0.16, 0.24, 0.8) }),
                        BorderColor::all(accent.with_alpha(0.8)),
                    ));
                }
            });

            card.spawn((
                Text::new(format!("{} / {} READY", state.queue_players, state.queue_required)),
                TextFont { font_size: FontSize::Px(16.0), ..default() },
                TextColor(Color::WHITE),
            ));

            if !state.server_starting {
                spawn_action_button(card, "CANCEL", Color::srgb(0.92, 0.28, 0.30), PreGameBtnAction::CancelFindMatch);
            }
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
                        spawn_nav_pill(
                            caps,
                            &cap.to_string(),
                            gold,
                            active,
                            PreGameBtnAction::SelectLobbyCapacity(cap),
                        );
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