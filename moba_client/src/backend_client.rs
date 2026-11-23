use bevy::prelude::*;
use ureq::serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

// =============================================================================
// مدل‌های داده منطبق بر پاسخ‌های HTTP و WebSocket جنگو
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct ActiveMatchInfo {
    pub room_id: String,
    pub port: u16,
    pub server_addr: String,
    pub ticket_id: String,
    pub player_count: usize,
    pub team_id: i32,
    pub slot_index: usize,
    pub participant_status: String,
    pub game_link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PlayerProfile {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub mmr: i32,
    pub rank_tier: String,
    #[serde(default)]
    pub wins: i32,
    #[serde(default)]
    pub losses: i32,
    #[serde(default)]
    pub abandoned_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct VerifyOtpResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub mmr: i32,
    pub rank_tier: String,
    #[serde(default)]
    pub wins: i32,
    #[serde(default)]
    pub losses: i32,
    #[serde(default)]
    pub abandoned_count: i32,
    pub is_new_player: bool,
    pub active_match: Option<ActiveMatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct ProfileMeResponse {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub mmr: i32,
    pub rank_tier: String,
    pub wins: i32,
    pub losses: i32,
    pub abandoned_count: i32,
    pub active_match: Option<ActiveMatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct MatchHistorySummary {
    pub username: String,
    pub mmr: i32,
    pub rank_tier: String,
    pub wins: i32,
    pub losses: i32,
    pub abandoned_count: i32,
    pub total_matches: i32,
    pub win_rate_percent: f32,
    pub total_kills: i32,
    pub total_deaths: i32,
    pub total_assists: i32,
    pub avg_kda: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct MatchHistoryPlayerRow {
    pub user_id: String,
    pub username: String,
    pub team_id: i32,
    pub slot_index: usize,
    pub hero_id: i32,
    pub level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub last_hits: i32,
    pub denies: i32,
    pub gold_coins: i32,
    pub status: String,
    #[serde(default)]
    pub party_group_id: String,
    pub is_me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct MatchHistoryEntry {
    pub room_id: String,
    pub is_custom_lobby: bool,
    #[serde(default)]
    pub is_party_match: bool,
    #[serde(default)]
    pub party_group_id: String,
    pub outcome: String,
    pub team_id: i32,
    pub winner_team: i32,
    pub hero_id: i32,
    pub level: i32,
    pub kills: i32,
    pub deaths: i32,
    pub assists: i32,
    pub last_hits: i32,
    pub denies: i32,
    pub gold_coins: i32,
    pub mmr_delta: i32,
    pub duration_sec: i32,
    pub played_at: String,
    pub players: Vec<MatchHistoryPlayerRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct MatchHistoryResponse {
    pub summary: MatchHistorySummary,
    pub matches: Vec<MatchHistoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct LobbySummary {
    pub lobby_id: String,
    pub title: String,
    pub host_username: String,
    pub has_password: bool,
    pub current_players: usize,
    pub max_players: usize,
    pub is_starting: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct LobbyMemberInfo {
    pub user_id: String,
    pub username: String,
    pub team_id: i32,
    pub slot_index: usize,
    pub is_host: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct LobbyChatMsg {
    pub sender: String,
    pub text: String,
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct LobbyDetail {
    pub lobby_id: String,
    pub title: String,
    pub host_user_id: String,
    pub host_username: String,
    pub has_password: bool,
    pub max_players: usize,
    pub is_starting: bool,
    pub members: Vec<LobbyMemberInfo>,
    pub chat_messages: Vec<LobbyChatMsg>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct FriendEntry {
    pub user_id: String,
    pub username: String,
    pub mmr: i32,
    pub rank_tier: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub in_party: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct FriendRequestEntry {
    pub request_id: i64,
    pub user_id: String,
    pub username: String,
    pub mmr: i32,
    pub rank_tier: String,
    #[serde(default)]
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PlayerSearchResult {
    pub user_id: String,
    pub username: String,
    pub mmr: i32,
    pub rank_tier: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub relationship: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct FriendsSnapshot {
    #[serde(default)]
    pub friends: Vec<FriendEntry>,
    #[serde(default)]
    pub incoming_requests: Vec<FriendRequestEntry>,
    #[serde(default)]
    pub outgoing_requests: Vec<FriendRequestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PartyMemberInfo {
    pub user_id: String,
    pub username: String,
    pub mmr: i32,
    pub rank_tier: String,
    #[serde(default)]
    pub is_leader: bool,
    #[serde(default)]
    pub is_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PartyChatMsg {
    pub sender: String,
    pub text: String,
    #[serde(default)]
    pub is_system: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PartyState {
    pub party_id: String,
    pub leader_user_id: String,
    pub game_mode: String,
    pub max_size: usize,
    #[serde(default)]
    pub is_queuing: bool,
    #[serde(default)]
    pub members: Vec<PartyMemberInfo>,
    #[serde(default)]
    pub chat_messages: Vec<PartyChatMsg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct LobbyInviteInfo {
    pub lobby_id: String,
    pub lobby_title: String,
    #[serde(default)]
    pub has_password: bool,
    pub from_user_id: String,
    pub from_username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PartyInviteInfo {
    pub invite_id: String,
    pub party_id: String,
    pub from_user_id: String,
    pub from_username: String,
    pub game_mode: String,
    #[serde(default)]
    pub expires_in_secs: u64,
}

// =============================================================================
// رویدادهای غیرمسدودکننده (Async Results -> Bevy Main Thread)
// =============================================================================

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum BackendAsyncEvent {
    OtpRequested {
        email: String,
    },
    OtpVerified {
        access_token: String,
        refresh_token: String,
        profile: PlayerProfile,
        active_match: Option<ActiveMatchInfo>,
        is_new_player: bool,
    },
    SessionRestored {
        access_token: String,
        profile: PlayerProfile,
        active_match: Option<ActiveMatchInfo>,
    },
    SessionInvalid,
    ActiveMatchChecked(Option<ActiveMatchInfo>),
    MatchDisconnected(Option<ActiveMatchInfo>),
    MatchReconnected(Option<ActiveMatchInfo>),
    MatchAbandoned {
        mmr: i32,
        rank_tier: String,
        losses: i32,
        abandoned_count: i32,
    },
    HistoryLoaded(MatchHistoryResponse),
    HttpError(String),
    // WebSocket Matchmaking Events
    QueueStatus {
        players_in_queue: usize,
        required_players: usize,
    },
    StartingGameServer {
        room_id: String,
        port: u16,
        players_in_queue: usize,
        required_players: usize,
    },
    MatchFound(ActiveMatchInfo),
    MatchmakingError {
        message: String,
        active_match: Option<ActiveMatchInfo>,
    },
    MatchmakingClosed(u64),
    // WebSocket Custom Lobby Events
    LobbyList {
        lobbies: Vec<LobbySummary>,
        message: String,
    },
    LobbyState {
        lobby: LobbyDetail,
        message: String,
    },
    LobbyStarting {
        lobby: LobbyDetail,
        message: String,
    },
    KickedFromLobby {
        lobbies: Vec<LobbySummary>,
        message: String,
    },
    LobbyClosed {
        message: String,
    },
    LeftLobby {
        lobbies: Vec<LobbySummary>,
        message: String,
    },
    LobbyError(String),
    LobbyWsClosed(u64),
    // Friends HTTP & Social/Party WebSocket Events
    FriendsLoaded(FriendsSnapshot),
    FriendSearchResults(Vec<PlayerSearchResult>),
    FriendActionDone {
        message: String,
        snapshot: FriendsSnapshot,
    },
    FriendActionError(String),
    FriendPresenceUpdated {
        user_id: String,
        username: String,
        online: bool,
        status: String,
        in_party: bool,
    },
    SocialSnapshot {
        friends: FriendsSnapshot,
        party: Option<PartyState>,
        pending_invites: Vec<PartyInviteInfo>,
    },
    PartyStateUpdated(PartyState),
    PartyCleared {
        reason: String,
    },
    PartyInviteReceived {
        invite: PartyInviteInfo,
        title: String,
        message: String,
    },
    PartyInviteCleared(String),
    LobbyInviteReceived(LobbyInviteInfo),
    PartyInviteResult {
        target_user_id: String,
        accepted: bool,
        message: String,
    },
    SocialNotification {
        kind: String,
        title: String,
        message: String,
    },
    PartyQueueCanceled(String),
    SocialWsClosed(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
struct PersistedSession {
    access_token: String,
    refresh_token: String,
    email: String,
}

fn session_file_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("SimorghMoba");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("session.json")
}

pub fn save_persisted_session(access_token: &str, refresh_token: &str, email: &str) {
    let data = PersistedSession {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        email: email.to_string(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&data) {
        let _ = std::fs::write(session_file_path(), json);
    }
}

pub fn load_persisted_session() -> Option<(String, String, String)> {
    let raw = std::fs::read_to_string(session_file_path()).ok()?;
    let data: PersistedSession = serde_json::from_str(&raw).ok()?;
    if data.access_token.trim().is_empty() {
        return None;
    }
    Some((data.access_token, data.refresh_token, data.email))
}

pub fn clear_persisted_session() {
    let _ = std::fs::remove_file(session_file_path());
}

// =============================================================================
// Resource مرکزی کلاینت بک‌اند
// =============================================================================

#[derive(Resource)]
pub struct BackendClient {
    pub http_base: String,
    pub ws_base: String,
    pub tx: Sender<BackendAsyncEvent>,
    pub rx: Arc<Mutex<Receiver<BackendAsyncEvent>>>,
    mm_ws_tx: Option<Sender<String>>,
    lobby_ws_tx: Option<Sender<String>>,
    social_ws_tx: Option<Sender<String>>,
    mm_conn_id: u64,
    lobby_conn_id: u64,
    social_conn_id: u64,
    http_timeout_secs: u64,
    ws_ping_interval_secs: u64,
    ws_poll_sleep_ms: u64,
}

impl BackendClient {
    pub fn new(http_base: String, ws_base: String) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            http_base: http_base.trim_end_matches('/').to_string(),
            ws_base: ws_base.trim_end_matches('/').to_string(),
            tx,
            rx: Arc::new(Mutex::new(rx)),
            mm_ws_tx: None,
            lobby_ws_tx: None,
            social_ws_tx: None,
            mm_conn_id: 0,
            lobby_conn_id: 0,
            social_conn_id: 0,
            http_timeout_secs: 0,
            ws_ping_interval_secs: 0,
            ws_poll_sleep_ms: 0,
        }
    }

    pub fn configure_network(
        &mut self,
        http_timeout_secs: u64,
        ws_ping_interval_secs: u64,
        ws_poll_sleep_ms: u64,
    ) {
        assert!(http_timeout_secs > 0, "main_menu.http_timeout_secs must be > 0");
        assert!(ws_ping_interval_secs > 0, "main_menu.ws_ping_interval_secs must be > 0");
        assert!(ws_poll_sleep_ms > 0, "main_menu.ws_poll_sleep_ms must be > 0");
        self.http_timeout_secs = http_timeout_secs;
        self.ws_ping_interval_secs = ws_ping_interval_secs;
        self.ws_poll_sleep_ms = ws_poll_sleep_ms;
    }

    fn http_timeout(&self) -> Duration {
        assert!(
            self.http_timeout_secs > 0,
            "BackendClient::configure_network must be called from GameConfig.main_menu before HTTP requests"
        );
        Duration::from_secs(self.http_timeout_secs)
    }

    fn ws_ping_interval(&self) -> Duration {
        assert!(
            self.ws_ping_interval_secs > 0,
            "BackendClient::configure_network must be called from GameConfig.main_menu before WebSocket connections"
        );
        Duration::from_secs(self.ws_ping_interval_secs)
    }

    fn ws_poll_sleep(&self) -> Duration {
        assert!(
            self.ws_poll_sleep_ms > 0,
            "BackendClient::configure_network must be called from GameConfig.main_menu before WebSocket connections"
        );
        Duration::from_millis(self.ws_poll_sleep_ms)
    }

    pub fn active_mm_conn_id(&self) -> u64 {
        self.mm_conn_id
    }

    pub fn active_lobby_conn_id(&self) -> u64 {
        self.lobby_conn_id
    }

    pub fn active_social_conn_id(&self) -> u64 {
        self.social_conn_id
    }

    pub fn poll_events(&self) -> Vec<BackendAsyncEvent> {
        let mut out = Vec::new();
        if let Ok(rx) = self.rx.lock() {
            while let Ok(ev) = rx.try_recv() {
                out.push(ev);
            }
        }
        out
    }

    // -------------------------------------------------------------------------
    // سرویس‌های HTTP
    // -------------------------------------------------------------------------

    pub fn request_otp(&self, email: String, mode: String, username: String) {
        let url = format!("{}/api/v1/auth/request-otp/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let body = serde_json::json!({
                "email": email,
                "mode": mode,
                "username": username
            });
            match ureq::post(&url)
                .timeout(timeout)
                .send_json(body)
            {
                Ok(_) => {
                    let _ = tx.send(BackendAsyncEvent::OtpRequested { email });
                }
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn verify_otp(&self, email: String, otp_code: String) {
        let url = format!("{}/api/v1/auth/verify-otp/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let body = serde_json::json!({ "email": email, "otp_code": otp_code });
            match ureq::post(&url)
                .timeout(timeout)
                .send_json(body)
            {
                Ok(resp) => match resp.into_json::<VerifyOtpResponse>() {
                    Ok(v) => {
                        save_persisted_session(&v.access_token, &v.refresh_token, &v.email);
                        let profile = PlayerProfile {
                            user_id: v.user_id,
                            username: v.username,
                            email: v.email,
                            mmr: v.mmr,
                            rank_tier: v.rank_tier,
                            wins: v.wins,
                            losses: v.losses,
                            abandoned_count: v.abandoned_count,
                        };
                        let _ = tx.send(BackendAsyncEvent::OtpVerified {
                            access_token: v.access_token,
                            refresh_token: v.refresh_token,
                            profile,
                            active_match: v.active_match,
                            is_new_player: v.is_new_player,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::HttpError(format!(
                            "Invalid server response: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn restore_session(&self, access_token: String) {
        let url = format!("{}/api/v1/auth/me/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            match ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .call()
            {
                Ok(resp) => match resp.into_json::<ProfileMeResponse>() {
                    Ok(me) => {
                        let profile = PlayerProfile {
                            user_id: me.user_id,
                            username: me.username,
                            email: me.email,
                            mmr: me.mmr,
                            rank_tier: me.rank_tier,
                            wins: me.wins,
                            losses: me.losses,
                            abandoned_count: me.abandoned_count,
                        };
                        let _ = tx.send(BackendAsyncEvent::SessionRestored {
                            access_token,
                            profile,
                            active_match: me.active_match,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::HttpError(format!(
                            "Failed to parse profile response: {}",
                            e
                        )));
                    }
                },
                Err(ureq::Error::Status(401 | 403, _)) => {
                    clear_persisted_session();
                    let _ = tx.send(BackendAsyncEvent::SessionInvalid);
                }
                Err(e) => {
                    // Network/server outage: keep session.json intact so player is not logged out permanently.
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn fetch_active_match(&self, access_token: String) {
        let url = format!("{}/api/v1/match/active/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            if let Ok(resp) = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .call()
            {
                if let Ok(v) = resp.into_json::<serde_json::Value>() {
                    let am = v
                        .get("active_match")
                        .and_then(|x| serde_json::from_value::<Option<ActiveMatchInfo>>(x.clone()).ok())
                        .flatten();
                    let _ = tx.send(BackendAsyncEvent::ActiveMatchChecked(am));
                }
            }
        });
    }

    pub fn disconnect_match(&self, access_token: String) {
        let url = format!("{}/api/v1/match/disconnect/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .send_string("{}")
            {
                Ok(resp) => {
                    let am = resp
                        .into_json::<serde_json::Value>()
                        .ok()
                        .and_then(|v| {
                            v.get("active_match")
                                .and_then(|x| serde_json::from_value::<Option<ActiveMatchInfo>>(x.clone()).ok())
                                .flatten()
                        });
                    let _ = tx.send(BackendAsyncEvent::MatchDisconnected(am));
                }
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn reconnect_match(&self, access_token: String) {
        let url = format!("{}/api/v1/match/reconnect/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .send_string("{}")
            {
                Ok(resp) => {
                    let am = resp
                        .into_json::<serde_json::Value>()
                        .ok()
                        .and_then(|v| {
                            v.get("active_match")
                                .and_then(|x| serde_json::from_value::<Option<ActiveMatchInfo>>(x.clone()).ok())
                                .flatten()
                        });
                    let _ = tx.send(BackendAsyncEvent::MatchReconnected(am));
                }
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn abandon_match(&self, access_token: String) {
        let url = format!("{}/api/v1/match/abandon/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .send_string("{}")
            {
                Ok(resp) => match resp.into_json::<serde_json::Value>() {
                    Ok(v) => {
                        let mmr = v.get("mmr").and_then(|x| x.as_i64()).unwrap_or(1000) as i32;
                        let rank_tier = v
                            .get("rank_tier")
                            .and_then(|x| x.as_str())
                            .unwrap_or("Simorgh Initiate")
                            .to_string();
                        let losses = v.get("losses").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                        let abandoned_count = v
                            .get("abandoned_count")
                            .and_then(|x| x.as_i64())
                            .unwrap_or(0) as i32;
                        let _ = tx.send(BackendAsyncEvent::MatchAbandoned {
                            mmr,
                            rank_tier,
                            losses,
                            abandoned_count,
                        });
                    }
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::HttpError(format!(
                            "Invalid abandon response: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn fetch_match_history(&self, access_token: String) {
        let url = format!("{}/api/v1/match/history/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            match ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .call()
            {
                Ok(resp) => match resp.into_json::<MatchHistoryResponse>() {
                    Ok(hist) => {
                        let _ = tx.send(BackendAsyncEvent::HistoryLoaded(hist));
                    }
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::HttpError(format!(
                            "Failed to load match history: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    // -------------------------------------------------------------------------
    // وب‌سوکت Matchmaking (/ws/matchmaking/)
    // -------------------------------------------------------------------------

    pub fn start_matchmaking(&mut self, access_token: String) {
        self.stop_matchmaking();
        self.mm_conn_id = self.mm_conn_id.wrapping_add(1);
        let conn_id = self.mm_conn_id;
        let poll_sleep = self.ws_poll_sleep();
        let ws_url = format!("{}/ws/matchmaking/?token={}", self.ws_base, access_token);
        let tx = self.tx.clone();
        let (out_tx, out_rx) = mpsc::channel::<String>();
        self.mm_ws_tx = Some(out_tx);

        thread::spawn(move || {
            let Ok((mut socket, _)) = connect(ws_url.as_str()) else {
                let _ = tx.send(BackendAsyncEvent::MatchmakingError {
                    message: "Could not connect to matchmaking server.".to_string(),
                    active_match: None,
                });
                return;
            };
            set_ws_nonblocking(&mut socket);

            loop {
                while let Ok(cmd) = out_rx.try_recv() {
                    if cmd == "__CLOSE__" {
                        let _ = socket.close(None);
                        return;
                    }
                    let _ = socket.send(Message::Text(cmd.into()));
                }

                match socket.read() {
                    Ok(Message::Text(txt)) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                            let ev = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
                            match ev {
                                "queued" => {
                                    let p = v.get("players_in_queue").and_then(|x| x.as_u64()).unwrap_or(1) as usize;
                                    let r = v.get("required_players").and_then(|x| x.as_u64()).unwrap_or(2) as usize;
                                    let _ = tx.send(BackendAsyncEvent::QueueStatus {
                                        players_in_queue: p,
                                        required_players: r,
                                    });
                                }
                                "starting_server" => {
                                    let room_id = v.get("room_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    let port = v.get("port").and_then(|x| x.as_u64()).unwrap_or(4433) as u16;
                                    let p = v.get("players_in_queue").and_then(|x| x.as_u64()).unwrap_or(2) as usize;
                                    let r = v.get("required_players").and_then(|x| x.as_u64()).unwrap_or(2) as usize;
                                    let _ = tx.send(BackendAsyncEvent::StartingGameServer {
                                        room_id,
                                        port,
                                        players_in_queue: p,
                                        required_players: r,
                                    });
                                }
                                "match_found" => {
                                    if let Some(am) = parse_match_found_payload(&v) {
                                        let _ = tx.send(BackendAsyncEvent::MatchFound(am));
                                        let _ = socket.close(None);
                                        return;
                                    }
                                }
                                "error" => {
                                    let msg = v.get("message").and_then(|x| x.as_str()).map(|s| sanitize_ascii_msg(s, "Matchmaking error")).unwrap_or_else(|| "Matchmaking error".to_string());
                                    let am = v
                                        .get("active_match")
                                        .and_then(|x| serde_json::from_value::<ActiveMatchInfo>(x.clone()).ok());
                                    let _ = tx.send(BackendAsyncEvent::MatchmakingError {
                                        message: msg,
                                        active_match: am,
                                    });
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = tx.send(BackendAsyncEvent::MatchmakingClosed(conn_id));
                        return;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(poll_sleep);
                    }
                    Err(_) => {
                        let _ = tx.send(BackendAsyncEvent::MatchmakingClosed(conn_id));
                        return;
                    }
                }
            }
        });
    }

    pub fn stop_matchmaking(&mut self) {
        self.mm_conn_id = self.mm_conn_id.wrapping_add(1);
        if let Some(tx) = self.mm_ws_tx.take() {
            let _ = tx.send(serde_json::json!({"action": "cancel"}).to_string());
            let _ = tx.send("__CLOSE__".to_string());
        }
    }

    // -------------------------------------------------------------------------
    // وب‌سوکت لابی سفارشی (/ws/lobby/)
    // -------------------------------------------------------------------------

    pub fn connect_lobby_ws(&mut self, access_token: String) {
        self.disconnect_lobby_ws();
        self.lobby_conn_id = self.lobby_conn_id.wrapping_add(1);
        let conn_id = self.lobby_conn_id;
        let poll_sleep = self.ws_poll_sleep();
        let ws_url = format!("{}/ws/lobby/?token={}", self.ws_base, access_token);
        let tx = self.tx.clone();
        let (out_tx, out_rx) = mpsc::channel::<String>();
        self.lobby_ws_tx = Some(out_tx);

        thread::spawn(move || {
            let Ok((mut socket, _)) = connect(ws_url.as_str()) else {
                let _ = tx.send(BackendAsyncEvent::LobbyError(
                    "Could not connect to lobby server.".to_string(),
                ));
                return;
            };
            set_ws_nonblocking(&mut socket);

            loop {
                while let Ok(cmd) = out_rx.try_recv() {
                    if cmd == "__CLOSE__" {
                        let _ = socket.close(None);
                        return;
                    }
                    let _ = socket.send(Message::Text(cmd.into()));
                }

                match socket.read() {
                    Ok(Message::Text(txt)) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                            let ev = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
                            let msg_str = v.get("message").and_then(|x| x.as_str()).map(|s| sanitize_ascii_msg(s, "")).unwrap_or_default();
                            match ev {
                                "lobby_list" => {
                                    let lobbies = v
                                        .get("lobbies")
                                        .and_then(|x| serde_json::from_value::<Vec<LobbySummary>>(x.clone()).ok())
                                        .unwrap_or_default();
                                    let _ = tx.send(BackendAsyncEvent::LobbyList {
                                        lobbies,
                                        message: msg_str,
                                    });
                                }
                                "lobby_state" => {
                                    if let Some(lobby) = v
                                        .get("lobby")
                                        .and_then(|x| serde_json::from_value::<LobbyDetail>(x.clone()).ok())
                                    {
                                        let _ = tx.send(BackendAsyncEvent::LobbyState {
                                            lobby,
                                            message: msg_str,
                                        });
                                    }
                                }
                                "lobby_starting" => {
                                    if let Some(lobby) = v
                                        .get("lobby")
                                        .and_then(|x| serde_json::from_value::<LobbyDetail>(x.clone()).ok())
                                    {
                                        let _ = tx.send(BackendAsyncEvent::LobbyStarting {
                                            lobby,
                                            message: msg_str,
                                        });
                                    }
                                }
                                "match_found" => {
                                    if let Some(am) = parse_match_found_payload(&v) {
                                        let _ = tx.send(BackendAsyncEvent::MatchFound(am));
                                        let _ = socket.close(None);
                                        return;
                                    }
                                }
                                "kicked_from_lobby" => {
                                    let lobbies = v
                                        .get("lobbies")
                                        .and_then(|x| serde_json::from_value::<Vec<LobbySummary>>(x.clone()).ok())
                                        .unwrap_or_default();
                                    let _ = tx.send(BackendAsyncEvent::KickedFromLobby {
                                        lobbies,
                                        message: msg_str,
                                    });
                                }
                                "lobby_closed" => {
                                    let _ = tx.send(BackendAsyncEvent::LobbyClosed { message: msg_str });
                                }
                                "left_lobby" => {
                                    let lobbies = v
                                        .get("lobbies")
                                        .and_then(|x| serde_json::from_value::<Vec<LobbySummary>>(x.clone()).ok())
                                        .unwrap_or_default();
                                    let _ = tx.send(BackendAsyncEvent::LeftLobby {
                                        lobbies,
                                        message: msg_str,
                                    });
                                }
                                "error" => {
                                    let _ = tx.send(BackendAsyncEvent::LobbyError(msg_str));
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = tx.send(BackendAsyncEvent::LobbyWsClosed(conn_id));
                        return;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(poll_sleep);
                    }
                    Err(_) => {
                        let _ = tx.send(BackendAsyncEvent::LobbyWsClosed(conn_id));
                        return;
                    }
                }
            }
        });
    }

    pub fn disconnect_lobby_ws(&mut self) {
        self.lobby_conn_id = self.lobby_conn_id.wrapping_add(1);
        if let Some(tx) = self.lobby_ws_tx.take() {
            let _ = tx.send("__CLOSE__".to_string());
        }
    }

    pub fn send_lobby_action(&self, payload: serde_json::Value) {
        if let Some(ref tx) = self.lobby_ws_tx {
            let _ = tx.send(payload.to_string());
        }
    }

    // -------------------------------------------------------------------------
    // سرویس‌های HTTP دوستان (Friends REST API)
    // -------------------------------------------------------------------------

    pub fn fetch_friends(&self, access_token: String) {
        let url = format!("{}/api/v1/friends/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let resp = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .call();
            match resp {
                Ok(r) => match r.into_json::<FriendsSnapshot>() {
                    Ok(snap) => {
                        let _ = tx.send(BackendAsyncEvent::FriendsLoaded(snap));
                    }
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                            "Failed to parse friends: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::FriendActionError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn search_players(&self, access_token: String, query: String) {
        let encoded: String = query
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if encoded.len() < 2 {
            let _ = self.tx.send(BackendAsyncEvent::FriendSearchResults(Vec::new()));
            return;
        }
        let url = format!("{}/api/v1/friends/search/?q={}", self.http_base, encoded);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let resp = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(timeout)
                .call();
            match resp {
                Ok(r) => match r.into_string() {
                    Ok(body) => match serde_json::from_str::<serde_json::Value>(&body) {
                        Ok(v) => {
                            let results: Vec<PlayerSearchResult> = v
                                .get("results")
                                .and_then(|x| serde_json::from_value(x.clone()).ok())
                                .unwrap_or_default();
                            let _ = tx.send(BackendAsyncEvent::FriendSearchResults(results));
                        }
                        Err(e) => {
                            let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                                "Invalid search response: {}",
                                e
                            )));
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                            "Search error: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::FriendActionError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn send_friend_request(&self, access_token: String, target_user_id: String) {
        let url = format!("{}/api/v1/friends/request/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let body = serde_json::json!({ "target_user_id": target_user_id }).to_string();
            let resp = ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .set("Content-Type", "application/json")
                .timeout(timeout)
                .send_string(&body);
            match resp {
                Ok(r) => match r.into_string() {
                    Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(v) => {
                            let msg = v
                                .get("message")
                                .and_then(|x| x.as_str())
                                .map(|s| sanitize_ascii_msg(s, "Friend request sent."))
                                .unwrap_or_else(|| "Friend request sent.".to_string());
                            let snap: FriendsSnapshot =
                                serde_json::from_value(v).unwrap_or_default();
                            let _ = tx.send(BackendAsyncEvent::FriendActionDone {
                                message: msg,
                                snapshot: snap,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                                "Invalid friend request response: {}",
                                e
                            )));
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                            "Friend request read error: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::FriendActionError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn respond_friend_request(&self, access_token: String, request_id: i64, accept: bool) {
        let url = format!("{}/api/v1/friends/respond/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let body = serde_json::json!({
                "request_id": request_id,
                "accept": accept
            })
            .to_string();
            let resp = ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .set("Content-Type", "application/json")
                .timeout(timeout)
                .send_string(&body);
            match resp {
                Ok(r) => match r.into_string() {
                    Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(v) => {
                            let msg = v
                                .get("message")
                                .and_then(|x| x.as_str())
                                .map(|s| sanitize_ascii_msg(s, "Friend request updated."))
                                .unwrap_or_else(|| "Friend request updated.".to_string());
                            let snap: FriendsSnapshot =
                                serde_json::from_value(v).unwrap_or_default();
                            let _ = tx.send(BackendAsyncEvent::FriendActionDone {
                                message: msg,
                                snapshot: snap,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                                "Invalid friend response payload: {}",
                                e
                            )));
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                            "Friend response read error: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::FriendActionError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn remove_friend(&self, access_token: String, friend_user_id: String) {
        let url = format!("{}/api/v1/friends/remove/", self.http_base);
        let tx = self.tx.clone();
        let timeout = self.http_timeout();
        thread::spawn(move || {
            let body = serde_json::json!({ "friend_user_id": friend_user_id }).to_string();
            let resp = ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .set("Content-Type", "application/json")
                .timeout(timeout)
                .send_string(&body);
            match resp {
                Ok(r) => match r.into_string() {
                    Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(v) => {
                            let msg = v
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("Friends list updated.")
                                .to_string();
                            let snap: FriendsSnapshot =
                                serde_json::from_value(v).unwrap_or_default();
                            let _ = tx.send(BackendAsyncEvent::FriendActionDone {
                                message: msg,
                                snapshot: snap,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                                "Invalid remove friend response: {}",
                                e
                            )));
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(BackendAsyncEvent::FriendActionError(format!(
                            "Remove friend read error: {}",
                            e
                        )));
                    }
                },
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::FriendActionError(extract_ureq_error(e)));
                }
            }
        });
    }

    // -------------------------------------------------------------------------
    // وب‌سوکت دائمی دوستان، حضور آنلاین و پارتی (/ws/social/)
    // -------------------------------------------------------------------------

    pub fn is_social_ws_connected(&self) -> bool {
        self.social_ws_tx.is_some()
    }

    pub fn connect_social_ws(&mut self, access_token: String) {
        if self.social_ws_tx.is_some() {
            return;
        }
        self.social_conn_id = self.social_conn_id.wrapping_add(1);
        let conn_id = self.social_conn_id;
        let ping_interval = self.ws_ping_interval();
        let poll_sleep = self.ws_poll_sleep();
        let ws_url = format!("{}/ws/social/?token={}", self.ws_base, access_token);
        let tx = self.tx.clone();
        let (cmd_tx, cmd_rx) = mpsc::channel::<String>();
        self.social_ws_tx = Some(cmd_tx);

        thread::spawn(move || {
            let Ok((mut socket, _)) = tungstenite::connect(ws_url.as_str()) else {
                let _ = tx.send(BackendAsyncEvent::SocialWsClosed(conn_id));
                return;
            };
            set_ws_nonblocking(&mut socket);
            let mut last_ping = std::time::Instant::now();

            loop {
                if last_ping.elapsed() >= ping_interval {
                    last_ping = std::time::Instant::now();
                    if socket.send(Message::Ping(Vec::new().into())).is_err() {
                        let _ = tx.send(BackendAsyncEvent::SocialWsClosed(conn_id));
                        break;
                    }
                }
                while let Ok(cmd) = cmd_rx.try_recv() {
                    if cmd == "__CLOSE__" {
                        let _ = socket.close(None);
                        return;
                    }
                    let _ = socket.send(Message::Text(cmd.into()));
                }

                match socket.read() {
                    Ok(Message::Text(txt)) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(txt.as_str()) {
                            let ev = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
                            match ev {
                                "social_snapshot" => {
                                    let friends: FriendsSnapshot =
                                        serde_json::from_value(v.clone()).unwrap_or_default();
                                    let party: Option<PartyState> = v
                                        .get("party")
                                        .and_then(|x| {
                                            if x.is_null() {
                                                None
                                            } else {
                                                serde_json::from_value(x.clone()).ok()
                                            }
                                        });
                                    let pending_invites: Vec<PartyInviteInfo> = v
                                        .get("pending_invites")
                                        .and_then(|x| serde_json::from_value(x.clone()).ok())
                                        .unwrap_or_default();
                                    let _ = tx.send(BackendAsyncEvent::SocialSnapshot {
                                        friends,
                                        party,
                                        pending_invites,
                                    });
                                }
                                "friends_updated" => {
                                    let snap: FriendsSnapshot =
                                        serde_json::from_value(v).unwrap_or_default();
                                    let _ = tx.send(BackendAsyncEvent::FriendsLoaded(snap));
                                }
                                "friend_presence" => {
                                    let user_id = v
                                        .get("user_id")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let username = v
                                        .get("username")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let online =
                                        v.get("online").and_then(|x| x.as_bool()).unwrap_or(false);
                                    let status = v
                                        .get("status")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("offline")
                                        .to_string();
                                    let in_party = v
                                        .get("in_party")
                                        .and_then(|x| x.as_bool())
                                        .unwrap_or(false);
                                    let _ = tx.send(BackendAsyncEvent::FriendPresenceUpdated {
                                        user_id,
                                        username,
                                        online,
                                        status,
                                        in_party,
                                    });
                                }
                                "party_state" => {
                                    if let Some(pv) = v.get("party") {
                                        if let Ok(ps) =
                                            serde_json::from_value::<PartyState>(pv.clone())
                                        {
                                            let _ =
                                                tx.send(BackendAsyncEvent::PartyStateUpdated(ps));
                                        }
                                    }
                                }
                                "party_left" | "party_kicked" => {
                                    let msg = v
                                        .get("message")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("Left party.")
                                        .to_string();
                                    let _ =
                                        tx.send(BackendAsyncEvent::PartyCleared { reason: msg });
                                }
                                "party_invite_received" => {
                                    if let Some(iv) = v.get("invite") {
                                        if let Ok(inv) =
                                            serde_json::from_value::<PartyInviteInfo>(iv.clone())
                                        {
                                            let title = v
                                                .get("title")
                                                .and_then(|x| x.as_str())
                                                .unwrap_or("PARTY INVITE")
                                                .to_string();
                                            let message = v
                                                .get("message")
                                                .and_then(|x| x.as_str())
                                                .unwrap_or("Squad invite received.")
                                                .to_string();
                                            let _ =
                                                tx.send(BackendAsyncEvent::PartyInviteReceived {
                                                    invite: inv,
                                                    title,
                                                    message,
                                                });
                                        }
                                    }
                                }
                                "party_invite_result" => {
                                    let target_user_id = v
                                        .get("target_user_id")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let accepted = v
                                        .get("accepted")
                                        .and_then(|x| x.as_bool())
                                        .unwrap_or(false);
                                    let message = v
                                        .get("message")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("Invite response received.")
                                        .to_string();
                                    let _ = tx.send(BackendAsyncEvent::PartyInviteResult {
                                        target_user_id,
                                        accepted,
                                        message,
                                    });
                                }
                                "lobby_invite_received" => {
                                    if let Ok(inv) = serde_json::from_value::<LobbyInviteInfo>(v.clone()) {
                                        let _ = tx.send(BackendAsyncEvent::LobbyInviteReceived(inv));
                                    }
                                }
                                "party_invite_cleared" => {
                                    let iid = v
                                        .get("invite_id")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let _ = tx.send(BackendAsyncEvent::PartyInviteCleared(iid));
                                }
                                "notification" => {
                                    let kind = v
                                        .get("kind")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("info")
                                        .to_string();
                                    let title = v
                                        .get("title")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("NOTICE")
                                        .to_string();
                                    let message = v
                                        .get("message")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let _ = tx.send(BackendAsyncEvent::SocialNotification {
                                        kind,
                                        title,
                                        message,
                                    });
                                }
                                "party_queue_canceled" => {
                                    let msg = v
                                        .get("message")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("Party matchmaking canceled.")
                                        .to_string();
                                    let _ = tx.send(BackendAsyncEvent::PartyQueueCanceled(msg));
                                }
                                // Party Matchmaking events routed through /ws/social/
                                "queued" => {
                                    let p_in = v
                                        .get("players_in_queue")
                                        .and_then(|x| x.as_u64())
                                        .unwrap_or(1)
                                        as usize;
                                    let req = v
                                        .get("required_players")
                                        .and_then(|x| x.as_u64())
                                        .unwrap_or(2)
                                        as usize;
                                    let _ = tx.send(BackendAsyncEvent::QueueStatus {
                                        players_in_queue: p_in,
                                        required_players: req,
                                    });
                                }
                                "starting_server" => {
                                    let room_id = v
                                        .get("room_id")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let port =
                                        v.get("port").and_then(|x| x.as_u64()).unwrap_or(0) as u16;
                                    let p_in = v
                                        .get("players_in_queue")
                                        .and_then(|x| x.as_u64())
                                        .unwrap_or(2)
                                        as usize;
                                    let req = v
                                        .get("required_players")
                                        .and_then(|x| x.as_u64())
                                        .unwrap_or(2)
                                        as usize;
                                    let _ = tx.send(BackendAsyncEvent::StartingGameServer {
                                        room_id,
                                        port,
                                        players_in_queue: p_in,
                                        required_players: req,
                                    });
                                }
                                "match_found" => {
                                    if let Some(info) = parse_match_found_payload(&v) {
                                        let _ = tx.send(BackendAsyncEvent::MatchFound(info));
                                    }
                                }
                                "error" => {
                                    let msg = v
                                        .get("message")
                                        .and_then(|x| x.as_str())
                                        .unwrap_or("Social error.")
                                        .to_string();
                                    let _ = tx.send(BackendAsyncEvent::SocialNotification {
                                        kind: "error".to_string(),
                                        title: "ERROR".to_string(),
                                        message: msg,
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        let _ = tx.send(BackendAsyncEvent::SocialWsClosed(conn_id));
                        break;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(ref e))
                        if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        thread::sleep(poll_sleep);
                    }
                    Err(_) => {
                        let _ = tx.send(BackendAsyncEvent::SocialWsClosed(conn_id));
                        break;
                    }
                }
            }
        });
    }

    pub fn disconnect_social_ws(&mut self) {
        self.social_conn_id = self.social_conn_id.wrapping_add(1);
        if let Some(tx) = self.social_ws_tx.take() {
            let _ = tx.send("__CLOSE__".to_string());
        }
    }

    pub fn send_social_action(&self, payload: serde_json::Value) {
        if let Some(ref tx) = self.social_ws_tx {
            let _ = tx.send(payload.to_string());
        }
    }
}

fn parse_match_found_payload(v: &serde_json::Value) -> Option<ActiveMatchInfo> {
    let room_id = v.get("room_id")?.as_str()?.to_string();
    let port = v.get("port")?.as_u64()? as u16;
    let server_addr = v.get("server_addr")?.as_str()?.to_string();
    let ticket_id = v.get("ticket_id")?.as_str()?.to_string();
    let player_count = v.get("player_count").and_then(|x| x.as_u64()).unwrap_or(2) as usize;
    let team_id = v.get("team_id").and_then(|x| x.as_i64()).unwrap_or(1) as i32;
    let slot_index = v.get("slot_index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
    let game_link = v
        .get("game_link")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Some(ActiveMatchInfo {
        room_id,
        port,
        server_addr,
        ticket_id,
        player_count,
        team_id,
        slot_index,
        participant_status: "connected".to_string(),
        game_link,
    })
}

fn set_ws_nonblocking(socket: &mut WebSocket<MaybeTlsStream<std::net::TcpStream>>) {
    match socket.get_mut() {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_nonblocking(true);
        }
        _ => {}
    }
}

pub fn sanitize_ascii_msg(raw: &str, fallback: &str) -> String {
    let ascii: String = raw
        .chars()
        .map(|c| if c.is_ascii() && !c.is_control() { c } else { ' ' })
        .collect();
    let compact: String = ascii.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() >= 3 && compact.chars().any(|c| c.is_ascii_alphanumeric()) {
        compact
    } else {
        fallback.to_string()
    }
}

fn extract_ureq_error(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(_, resp) => {
            if let Ok(v) = resp.into_json::<serde_json::Value>() {
                if let Some(msg) = v.get("error").and_then(|x| x.as_str()) {
                    return sanitize_ascii_msg(msg, "Request failed.");
                }
            }
            "Request failed.".to_string()
        }
        ureq::Error::Transport(t) => format!("Server connection error: {}", t),
    }
}