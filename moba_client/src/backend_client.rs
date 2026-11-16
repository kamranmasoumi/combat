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
    pub is_me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct MatchHistoryEntry {
    pub room_id: String,
    pub is_custom_lobby: bool,
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

// =============================================================================
// رویدادهای غیرمسدودکننده (Async Results -> Bevy Main Thread)
// =============================================================================

#[derive(Debug, Clone)]
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
    MatchmakingClosed,
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
    LobbyWsClosed,
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
        }
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

    pub fn request_otp(&self, email: String) {
        let url = format!("{}/api/v1/auth/request-otp/", self.http_base);
        let tx = self.tx.clone();
        thread::spawn(move || {
            let body = serde_json::json!({ "email": email });
            match ureq::post(&url)
                .timeout(Duration::from_secs(8))
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
        thread::spawn(move || {
            let body = serde_json::json!({ "email": email, "otp_code": otp_code });
            match ureq::post(&url)
                .timeout(Duration::from_secs(10))
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
                            wins: 0,
                            losses: 0,
                            abandoned_count: 0,
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
        thread::spawn(move || {
            match ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(6))
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
                    Err(_) => {
                        let _ = tx.send(BackendAsyncEvent::SessionInvalid);
                    }
                },
                Err(_) => {
                    clear_persisted_session();
                    let _ = tx.send(BackendAsyncEvent::SessionInvalid);
                }
            }
        });
    }

    pub fn fetch_active_match(&self, access_token: String) {
        let url = format!("{}/api/v1/match/active/", self.http_base);
        let tx = self.tx.clone();
        thread::spawn(move || {
            if let Ok(resp) = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(6))
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
        thread::spawn(move || {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(6))
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
        thread::spawn(move || {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(6))
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
        thread::spawn(move || {
            match ureq::post(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(8))
                .send_string("{}")
            {
                Ok(resp) => {
                    if let Ok(v) = resp.into_json::<serde_json::Value>() {
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
                }
                Err(e) => {
                    let _ = tx.send(BackendAsyncEvent::HttpError(extract_ureq_error(e)));
                }
            }
        });
    }

    pub fn fetch_match_history(&self, access_token: String) {
        let url = format!("{}/api/v1/match/history/", self.http_base);
        let tx = self.tx.clone();
        thread::spawn(move || {
            match ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(8))
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
                        let _ = tx.send(BackendAsyncEvent::MatchmakingClosed);
                        return;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => {
                        let _ = tx.send(BackendAsyncEvent::MatchmakingClosed);
                        return;
                    }
                }
            }
        });
    }

    pub fn stop_matchmaking(&mut self) {
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
                        let _ = tx.send(BackendAsyncEvent::LobbyWsClosed);
                        return;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => {
                        let _ = tx.send(BackendAsyncEvent::LobbyWsClosed);
                        return;
                    }
                }
            }
        });
    }

    pub fn disconnect_lobby_ws(&mut self) {
        if let Some(tx) = self.lobby_ws_tx.take() {
            let _ = tx.send("__CLOSE__".to_string());
        }
    }

    pub fn send_lobby_action(&self, payload: serde_json::Value) {
        if let Some(ref tx) = self.lobby_ws_tx {
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
        team_id: 1,
        slot_index: 0,
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