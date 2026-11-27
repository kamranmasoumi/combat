use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::prelude::*;
use moba_shared::{
    create_shared_renet_server, parse_server_cli_args, step_couriers_system,
    step_shadow_blade_system, step_teleports_system, step_weather_and_environment_system,
    try_activate_item_ability, try_activate_shadow_blade, try_buy_cart, try_buy_item, try_cancel_teleport,
    try_courier_deliver, try_courier_move_to, try_courier_return_to_fountain, try_cycle_item_mode,
    try_sell_courier_item, try_sell_item, try_start_teleport, try_swap_courier_slots, try_swap_inventory_slots, try_swap_stash_slots,
    try_buy_recipe, try_move_courier_item_to_stash, try_move_hero_item_to_stash,
    try_move_stash_item_to_courier, try_move_stash_item_to_hero, try_sell_stash_item,
    try_take_all_stash_to_hero, try_transfer_courier_item_to_hero,
    try_transfer_hero_item_to_courier, AuthoritativeMatchState,
    AuthoritativeWorldState, ClientInputCommand, ClientLobbyMessage, CustomPhysicsWorld,
    GameConfig, GameplayFxEvent, ItemsConfig, MatchPhase, MobaPhysicsSet, MobaSharedPhysicsPlugin,
    PhysicsRole, PhysicsSoldier, ServerLobbyPacket, ServerMatchMessage, ServerWorldSnapshot,
    SpectatorFogMode, SpectatorPlaybackSnapshot, TeamId,
    CHANNEL_RELIABLE_FX, CHANNEL_RELIABLE_LOBBY, CHANNEL_UNRELIABLE_STATE,
};
use renet::{ClientId, RenetServer, ServerEvent};
use renet_netcode::NetcodeServerTransport;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Resource)]
struct ServerNetwork {
    server: RenetServer,
    transport: NetcodeServerTransport,
}

#[derive(Resource)]
struct LobbyBroadcastTimer(Timer);

/// بافرهای قابل استفاده مجدد برای جلوگیری از تخصیص حافظه تکراری در هر تیک ۶۰ هرتز سرور
#[derive(Resource, Default)]
struct ServerScratchBuffers {
    client_ids: Vec<ClientId>,
    serialize_buf: Vec<u8>,
}

#[derive(Clone, Debug)]
struct VerifiedTicketInfo {
    user_id: String,
    username: String,
    ticket_id: String,
    slot_index: usize,
}

/// وضعیت احراز هویت تیکت Redis و گزارش نتیجه به بکاند جنگو
#[derive(Resource, Default)]
struct ServerShutdownTracker {
    post_match_elapsed: f32,
    empty_room_elapsed: f32,
    ever_had_client: bool,
}

#[derive(Clone, Debug)]
struct SpectatorSession {
    #[allow(dead_code)]
    user_id: String,
    username: String,
    fog_mode: SpectatorFogMode,
    playback_speed: f32,
    is_paused: bool,
    playback_time_secs: f32,
    following_live: bool,
    delay_secs: f32,
    last_sent_frame_time: f32,
    last_sent_fog_mode: SpectatorFogMode,
    last_sent_paused: bool,
    last_sent_speed: f32,
    last_sent_viewers: usize,
    heartbeat_accum: f32,
}

struct RecordedSpectatorFrame {
    match_time_secs: f32,
    base_snapshot: ServerWorldSnapshot,
    dawn_fog_mask: Vec<u8>,
    dusk_fog_mask: Vec<u8>,
}

enum SourceTvCommand {
    RecordFrame {
        match_time_secs: f32,
        base_snapshot: ServerWorldSnapshot,
        dawn_fog_mask: Vec<u8>,
        dusk_fog_mask: Vec<u8>,
    },
    SpectatorJoin {
        client_id: ClientId,
        session: SpectatorSession,
    },
    SpectatorLeave {
        client_id: ClientId,
    },
    SetFogMode {
        client_id: ClientId,
        mode: SpectatorFogMode,
    },
    SetSpeed {
        client_id: ClientId,
        speed: f32,
    },
    TogglePause {
        client_id: ClientId,
    },
    SeekTo {
        client_id: ClientId,
        target_time_secs: f32,
    },
    JumpLive {
        client_id: ClientId,
    },
    StepBroadcast {
        dt: f32,
        now_match_t: f32,
    },
}

#[derive(Resource)]
struct SourceTvRelay {
    cmd_tx: std::sync::mpsc::SyncSender<SourceTvCommand>,
    out_rx: std::sync::Mutex<std::sync::mpsc::Receiver<(ClientId, Vec<u8>)>>,
    record_accum: f32,
    step_accum: f32,
}

impl SourceTvRelay {
    fn spawn(config: GameConfig) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel::<SourceTvCommand>(64);
        let (out_tx, out_rx) = std::sync::mpsc::sync_channel::<(ClientId, Vec<u8>)>(256);

        let _ = std::thread::Builder::new()
            .name("sourcetv-relay".to_string())
            .spawn(move || {
                let mut frames: VecDeque<RecordedSpectatorFrame> = VecDeque::new();
                let mut spectators: HashMap<ClientId, SpectatorSession> = HashMap::new();
                let mut serialize_buf: Vec<u8> = Vec::with_capacity(65536);

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        SourceTvCommand::RecordFrame {
                            match_time_secs,
                            base_snapshot,
                            dawn_fog_mask,
                            dusk_fog_mask,
                        } => {
                            frames.push_back(RecordedSpectatorFrame {
                                match_time_secs,
                                base_snapshot,
                                dawn_fog_mask,
                                dusk_fog_mask,
                            });
                            let min_keep_time = match_time_secs - config.spectator.max_history_secs;
                            while frames.len() > 2
                                && frames.front().is_some_and(|f| f.match_time_secs < min_keep_time)
                            {
                                frames.pop_front();
                            }
                        }
                        SourceTvCommand::SpectatorJoin { client_id, session } => {
                            spectators.insert(client_id, session);
                        }
                        SourceTvCommand::SpectatorLeave { client_id } => {
                            spectators.remove(&client_id);
                        }
                        SourceTvCommand::SetFogMode { client_id, mode } => {
                            if let Some(s) = spectators.get_mut(&client_id) {
                                s.fog_mode = mode;
                            }
                        }
                        SourceTvCommand::SetSpeed { client_id, speed } => {
                            if let Some(s) = spectators.get_mut(&client_id) {
                                s.playback_speed = speed;
                                if speed < 1.0 {
                                    s.following_live = false;
                                }
                            }
                        }
                        SourceTvCommand::TogglePause { client_id } => {
                            if let Some(s) = spectators.get_mut(&client_id) {
                                s.is_paused = !s.is_paused;
                                if s.is_paused {
                                    s.following_live = false;
                                }
                            }
                        }
                        SourceTvCommand::SeekTo {
                            client_id,
                            target_time_secs,
                        } => {
                            if let Some(s) = spectators.get_mut(&client_id) {
                                s.playback_time_secs = target_time_secs.max(0.0);
                                s.following_live = false;
                            }
                        }
                        SourceTvCommand::JumpLive { client_id } => {
                            if let Some(s) = spectators.get_mut(&client_id) {
                                s.following_live = true;
                                s.is_paused = false;
                                s.playback_speed = 1.0;
                            }
                        }
                        SourceTvCommand::StepBroadcast { dt, now_match_t } => {
                            if spectators.is_empty() || frames.is_empty() {
                                continue;
                            }
                            let min_t = frames.front().map(|f| f.match_time_secs).unwrap_or(0.0);
                            let spec_count = spectators.len();
                            let mut packet_cache: HashMap<(usize, u8, bool, u32), Vec<u8>> = HashMap::new();

                            for (&spec_cid, spec) in spectators.iter_mut() {
                                let max_live_t = (now_match_t - spec.delay_secs).max(min_t);
                                if spec.following_live && !spec.is_paused {
                                    spec.playback_time_secs = max_live_t;
                                } else if !spec.is_paused {
                                    spec.playback_time_secs =
                                        (spec.playback_time_secs + dt * spec.playback_speed)
                                            .clamp(min_t, max_live_t);
                                    if spec.playback_time_secs >= max_live_t - 0.05
                                        && spec.playback_speed >= 1.0
                                    {
                                        spec.following_live = true;
                                    }
                                } else {
                                    spec.playback_time_secs =
                                        spec.playback_time_secs.clamp(min_t, max_live_t);
                                }

                                let pos = frames
                                    .partition_point(|f| f.match_time_secs <= spec.playback_time_secs);
                                let frame_idx = pos.saturating_sub(1).min(frames.len() - 1);
                                let frame = &frames[frame_idx];

                                spec.heartbeat_accum += dt;
                                let frame_changed =
                                    (frame.match_time_secs - spec.last_sent_frame_time).abs() > 0.001;
                                let state_changed = spec.fog_mode != spec.last_sent_fog_mode
                                    || spec.is_paused != spec.last_sent_paused
                                    || (spec.playback_speed - spec.last_sent_speed).abs() > 0.001
                                    || spec_count != spec.last_sent_viewers;
                                if !frame_changed && !state_changed && spec.heartbeat_accum < 0.5 {
                                    continue;
                                }
                                spec.last_sent_frame_time = frame.match_time_secs;
                                spec.last_sent_fog_mode = spec.fog_mode;
                                spec.last_sent_paused = spec.is_paused;
                                spec.last_sent_speed = spec.playback_speed;
                                spec.last_sent_viewers = spec_count;
                                spec.heartbeat_accum = 0.0;

                                let fog_key: u8 = match spec.fog_mode {
                                    SpectatorFogMode::AllVision => 0,
                                    SpectatorFogMode::TeamDawn => 1,
                                    SpectatorFogMode::TeamDusk => 2,
                                };
                                let cache_key = (
                                    frame_idx,
                                    fog_key,
                                    spec.is_paused,
                                    (spec.playback_speed * 100.0).round() as u32,
                                );

                                let bytes = if let Some(cached) = packet_cache.get(&cache_key) {
                                    cached.clone()
                                } else {
                                    let mut snap = frame.base_snapshot.clone();
                                    AuthoritativeWorldState::apply_spectator_fog_mode(
                                        &mut snap,
                                        &config,
                                        spec.fog_mode,
                                        &frame.dawn_fog_mask,
                                        &frame.dusk_fog_mask,
                                    );
                                    snap.spectator_state = Some(SpectatorPlaybackSnapshot {
                                        fog_mode: spec.fog_mode,
                                        playback_speed: spec.playback_speed,
                                        is_paused: spec.is_paused,
                                        current_time_secs: frame.match_time_secs,
                                        min_time_secs: min_t,
                                        max_live_time_secs: max_live_t,
                                        delay_secs: spec.delay_secs,
                                        spectator_count: spec_count,
                                    });
                                    serialize_buf.clear();
                                    if bincode::serialize_into(&mut serialize_buf, &snap).is_err() {
                                        continue;
                                    }
                                    let out = serialize_buf.clone();
                                    packet_cache.insert(cache_key, out.clone());
                                    out
                                };

                                let _ = out_tx.try_send((spec_cid, bytes));
                            }
                        }
                    }
                }
            });

        Self {
            cmd_tx,
            out_rx: std::sync::Mutex::new(out_rx),
            record_accum: 0.0,
            step_accum: 0.0,
        }
    }
}

#[derive(Resource)]
struct ServerAuthState {
    room_id: Option<String>,
    port: u16,
    pending_auth: HashMap<ClientId, Instant>,
    verified_clients: HashMap<ClientId, VerifiedTicketInfo>,
    verified_spectators: HashMap<ClientId, SpectatorSession>,
    result_reported: bool,
}

fn main() {
    let (cli_port, room_id_opt, required_players) = parse_server_cli_args();

    let mut config = GameConfig::load_from_file("config.ron");
    let items_config = ItemsConfig::load_from_file("items.ron");
    let ip = config
        .network
        .server_bind_addr
        .split(':')
        .next()
        .expect("آدرس سرور در config.ron نامعتبر است");
    config.network.server_bind_addr = format!("{}:{}", ip, cli_port);
    let pub_ip = config
        .network
        .server_public_addr
        .split(':')
        .next()
        .expect("server_public_addr در config.ron نامعتبر است");
    config.network.server_public_addr = format!("{}:{}", pub_ip, cli_port);
    config.network.max_clients = required_players + config.spectator.max_spectators_per_match;

    let tick_hz = config.network.server_tick_rate_hz;
    let substeps = config.physics.substeps;

    let (server, transport) = create_shared_renet_server(&config.network);
    let world_state = AuthoritativeWorldState::new_from_config(&config);
    let match_state = AuthoritativeMatchState::new(required_players);

    println!(
        "سرور MOBA روی {} (Room: {:?}) با ظرفیت {} بازیکن آماده اتصال است...",
        config.network.server_bind_addr, room_id_opt, required_players
    );

    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            Duration::from_secs_f64(1.0 / tick_hz),
        )))
        .add_plugins(MobaSharedPhysicsPlugin {
            role: PhysicsRole::Server,
            tick_rate_hz: tick_hz,
            substeps,
        })
        .insert_resource(SourceTvRelay::spawn(config.clone()))
        .insert_resource(config)
        .insert_resource(items_config)
        .insert_resource(world_state)
        .insert_resource(match_state)
        .insert_resource(ServerAuthState {
            room_id: room_id_opt,
            port: cli_port,
            pending_auth: HashMap::new(),
            verified_clients: HashMap::new(),
            verified_spectators: HashMap::new(),
            result_reported: false,
        })
        .insert_resource(LobbyBroadcastTimer(Timer::from_seconds(
            0.2,
            TimerMode::Repeating,
        )))
        .insert_resource(ServerNetwork { server, transport })
        .init_resource::<ServerScratchBuffers>()
        .init_resource::<ServerShutdownTracker>()
        .add_systems(
            FixedUpdate,
            (
                server_network_receive_system.before(MobaPhysicsSet::Drive),
                server_match_tick_system.before(MobaPhysicsSet::Drive),
                step_couriers_system
                    .after(MobaPhysicsSet::Drive)
                    .before(server_send_snapshot_system),
                step_teleports_system
                    .after(MobaPhysicsSet::Drive)
                    .before(server_send_snapshot_system),
                step_shadow_blade_system
                    .after(MobaPhysicsSet::Drive)
                    .before(server_send_snapshot_system),
                step_weather_and_environment_system
                    .after(MobaPhysicsSet::Drive)
                    .before(server_send_snapshot_system),
                server_send_snapshot_system.after(MobaPhysicsSet::React),
            ),
        )
        .run();
}

// =============================================================================
// توابع بدون وابستگی خارجی برای استعلام تیکت از Redis (RESP2) و وبهوک جنگو
// =============================================================================

#[derive(Debug)]
struct RedisTicketPayload {
    room_id: String,
    user_id: String,
    username: String,
    role: String,
    is_custom_lobby: bool,
}

fn verify_ticket_in_redis(redis_addr: &str, ticket_id: &str) -> Option<RedisTicketPayload> {
    if ticket_id.is_empty()
        || ticket_id.len() > 128
        || ticket_id.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }

    let mut stream = TcpStream::connect(redis_addr).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1200)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1200)));

    let key = format!("ticket:{}", ticket_id);
    let cmd = format!("*2\r\n$3\r\nGET\r\n${}\r\n{}\r\n", key.len(), key);
    stream.write_all(cmd.as_bytes()).ok()?;

    let mut reader = BufReader::new(stream);
    let mut header = String::new();
    reader.read_line(&mut header).ok()?;
    let header_trim = header.trim();
    if !header_trim.starts_with('$') || header_trim == "$-1" {
        return None;
    }
    let len: usize = header_trim[1..].parse().ok()?;
    if len == 0 || len > 8192 {
        return None;
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).ok()?;
    let json_str = String::from_utf8(buf).ok()?;

    let room_id = extract_json_str(&json_str, "room_id")?;
    let user_id = extract_json_str(&json_str, "user_id")?;
    let username = extract_json_str(&json_str, "username").unwrap_or_else(|| user_id.clone());
    let role = extract_json_str(&json_str, "role").unwrap_or_else(|| "player".to_string());
    let is_custom_lobby = extract_json_str(&json_str, "is_custom_lobby")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    Some(RedisTicketPayload {
        room_id,
        user_id,
        username,
        role,
        is_custom_lobby,
    })
}

fn extract_json_str(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let idx = json.find(&pattern)?;
    let rest = &json[idx + pattern.len()..];
    let colon = rest.find(':')?;
    let after_colon = rest[colon + 1..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let inner = &after_colon[1..];
    let end_quote = inner.find('"')?;
    Some(inner[..end_quote].to_string())
}

fn post_json_to_django_async(base_http_url: &str, path: &str, json_body: String) {
    let url = base_http_url.trim_end_matches('/').to_string();
    let path = path.to_string();
    thread::spawn(move || {
        let addr = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("https://"))
            .unwrap_or(&url);
        let host_port = addr.split('/').next().unwrap_or(addr);
        let webhook_secret = std::env::var("INTERNAL_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "simorgh-internal-webhook-secret-2026".to_string());
        let req = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nX-Internal-Secret: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            path,
            host_port,
            webhook_secret,
            json_body.len(),
            json_body
        );

        for attempt in 1..=3 {
            match TcpStream::connect(host_port) {
                Ok(mut stream) => {
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    if let Err(e) = stream.write_all(req.as_bytes()).and_then(|_| stream.flush()) {
                        eprintln!(
                            "[DJANGO HTTP ERROR] {} (attempt {}/3) write failed: {:?}",
                            path, attempt, e
                        );
                    } else {
                        let mut resp = String::new();
                        let _ = stream.read_to_string(&mut resp);
                        let status_line = resp.lines().next().unwrap_or("");
                        if status_line.contains(" 200 ") || status_line.contains(" 201 ") {
                            return;
                        }
                        eprintln!(
                            "[DJANGO HTTP ERROR] {} (attempt {}/3) unexpected response: {}",
                            path, attempt, status_line
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[DJANGO CONNECT ERROR] {} -> {} (attempt {}/3): {:?}",
                        host_port, path, attempt, e
                    );
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out
}

fn serialize_final_items_json(inv: &[moba_shared::InventoryItemState]) -> String {
    let mut out = String::from("[");
    let mut sorted: Vec<(usize, &str)> = inv
        .iter()
        .filter(|it| !it.item_id.trim().is_empty())
        .map(|it| (it.slot, it.item_id.as_str()))
        .collect();
    sorted.sort_by_key(|(slot, _)| *slot);
    for (i, (slot, item_id)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"slot\":{},\"item_id\":\"{}\"}}",
            slot,
            escape_json_str(item_id)
        ));
    }
    out.push(']');
    out
}

fn resolve_hero_id_1based(config: &GameConfig, hero_str_id: &str) -> Option<usize> {
    config
        .heroes
        .iter()
        .position(|h| h.id == hero_str_id)
        .map(|idx| idx + 1)
}

#[allow(clippy::too_many_arguments)]
fn notify_django_player_state(
    config: &GameConfig,
    room_id: &str,
    port: u16,
    user_id: &str,
    ticket_id: &str,
    slot_index: usize,
    status: &str,
    hero_id_override: Option<usize>,
    live_player: Option<&moba_shared::PlayerSlotState>,
) {
    let mut body = format!(
        "{{\"room_id\":\"{}\",\"port\":{},\"user_id\":\"{}\",\"ticket_id\":\"{}\",\"slot_index\":{},\"status\":\"{}\"",
        escape_json_str(room_id),
        port,
        escape_json_str(user_id),
        escape_json_str(ticket_id),
        slot_index,
        escape_json_str(status)
    );

    if let Some(p) = live_player {
        let hid = hero_id_override.unwrap_or(p.hero_index + 1);
        let items_json = serialize_final_items_json(&p.inventory);
        body.push_str(&format!(
            ",\"hero_id\":{},\"level\":{},\"kills\":{},\"deaths\":{},\"assists\":{},\"last_hits\":{},\"denies\":{},\"gold_coins\":{},\"final_items\":{}",
            hid,
            p.hero_level,
            p.kills,
            p.deaths,
            p.assists,
            p.last_hits,
            p.denies,
            p.hero_gold,
            items_json
        ));
    } else if let Some(hid) = hero_id_override {
        body.push_str(&format!(",\"hero_id\":{}", hid));
    }

    body.push('}');
    post_json_to_django_async(
        &config.network.backend_http_url,
        "/api/v1/match/player-state/",
        body,
    );
}

fn notify_django_spectator_count(
    config: &GameConfig,
    room_id: &str,
    port: u16,
    spectator_count: usize,
) {
    let body = format!(
        "{{\"room_id\":\"{}\",\"port\":{},\"status\":\"spectator_count\",\"spectator_count\":{}}}",
        escape_json_str(room_id),
        port,
        spectator_count
    );
    post_json_to_django_async(
        &config.network.backend_http_url,
        "/api/v1/match/player-state/",
        body,
    );
}

fn send_initial_configs_to_spectator(
    client_id: ClientId,
    config: &GameConfig,
    items_config: &ItemsConfig,
    net_server: &mut RenetServer,
    match_state: &AuthoritativeMatchState,
) {
    let mut safe_cfg = config.clone();
    safe_cfg.network.private_key = [0; 32];
    let sync_pkt = ServerLobbyPacket::SyncConfigs {
        game_config: safe_cfg,
        items_config: items_config.clone(),
    };
    if let Ok(bytes) = bincode::serialize(&sync_pkt) {
        net_server.send_message(client_id, CHANNEL_RELIABLE_LOBBY, bytes);
    }
    let msg = ServerLobbyPacket::MatchState(ServerMatchMessage {
        your_slot: usize::MAX,
        state: match_state.snapshot(),
    });
    if let Ok(bytes) = bincode::serialize(&msg) {
        net_server.send_message(client_id, CHANNEL_RELIABLE_LOBBY, bytes);
    }
}

fn send_initial_configs_and_sync_world(
    client_id: ClientId,
    config: &GameConfig,
    items_config: &ItemsConfig,
    net_server: &mut RenetServer,
    match_state: &AuthoritativeMatchState,
    world: &mut AuthoritativeWorldState,
    slot_idx: usize,
) {
    let mut safe_cfg = config.clone();
    safe_cfg.network.private_key = [0; 32];
    let sync_pkt = ServerLobbyPacket::SyncConfigs {
        game_config: safe_cfg,
        items_config: items_config.clone(),
    };
    if let Ok(bytes) = bincode::serialize(&sync_pkt) {
        net_server.send_message(client_id, CHANNEL_RELIABLE_LOBBY, bytes);
    }
    if match_state.phase == MatchPhase::InGame {
        world.set_slot_connected(slot_idx, true, config);
    }
}

#[allow(clippy::too_many_arguments)]
fn server_network_receive_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    physics_world: Res<CustomPhysicsWorld>,
    mut net: ResMut<ServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut auth_state: ResMut<ServerAuthState>,
    tv_relay: Res<SourceTvRelay>,
    mut scratch: ResMut<ServerScratchBuffers>,
) {
    let net_inner = &mut *net;
    net_inner.server.update(time.delta());
    if let Err(e) = net_inner.transport.update(time.delta(), &mut net_inner.server) {
        eprintln!("خطای transport سرور: {:?}", e);
    }

    while let Some(event) = net_inner.server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                if auth_state.room_id.is_some() {
                    auth_state.pending_auth.insert(client_id, Instant::now());
                    println!(
                        "[AUTH PENDING] کلاینت {} متصل شد؛ در انتظار تیکت Redis...",
                        client_id
                    );
                } else if match_state.on_client_connected(client_id, &config) {
                    if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                        send_initial_configs_and_sync_world(
                            client_id,
                            &config,
                            &items_config,
                            &mut net_inner.server,
                            &match_state,
                            &mut world,
                            slot_idx,
                        );
                    }
                    println!(
                        "بازیکن {} متصل شد ({}/{}).",
                        client_id,
                        match_state.connected_count(),
                        match_state.required_players
                    );
                } else {
                    println!("اتصال بازیکن {} رد شد (ظرفیت تکمیل یا تکراری).", client_id);
                }
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!("بازیکن {} قطع شد: {:?}", client_id, reason);
                auth_state.pending_auth.remove(&client_id);
                if let Some(vinfo) = auth_state.verified_clients.remove(&client_id) {
                    if let Some(ref rid) = auth_state.room_id {
                        let live_player = if match_state.phase == MatchPhase::InGame {
                            world.players.iter().find(|p| p.slot_index == vinfo.slot_index)
                        } else {
                            None
                        };
                        let lobby_hero_id = match_state
                            .players
                            .get(vinfo.slot_index)
                            .and_then(|lp| lp.selected_hero_id.as_deref())
                            .and_then(|hid| resolve_hero_id_1based(&config, hid));
                        notify_django_player_state(
                            &config,
                            rid,
                            auth_state.port,
                            &vinfo.user_id,
                            &vinfo.ticket_id,
                            vinfo.slot_index,
                            "disconnected",
                            lobby_hero_id,
                            live_player,
                        );
                    }
                }
                if let Some(spec) = auth_state.verified_spectators.remove(&client_id) {
                    let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::SpectatorLeave { client_id });
                    println!(
                        "[SPECTATOR LEFT] تماشاگر {} ({}) خارج شد (باقی‌مانده: {}).",
                        spec.username,
                        spec.user_id,
                        auth_state.verified_spectators.len()
                    );
                    if let Some(ref rid) = auth_state.room_id {
                        notify_django_spectator_count(
                            &config,
                            rid,
                            auth_state.port,
                            auth_state.verified_spectators.len(),
                        );
                    }
                }
                if let Some(slot_idx) = match_state.on_client_disconnected(client_id) {
                    if match_state.phase == MatchPhase::InGame {
                        world.set_slot_connected(slot_idx, false, &config);
                    }
                }
            }
        }
    }

    if auth_state.room_id.is_some() && !auth_state.pending_auth.is_empty() {
        let now = Instant::now();
        let expired: Vec<ClientId> = auth_state
            .pending_auth
            .iter()
            .filter(|(_, ts)| now.duration_since(**ts) > Duration::from_secs(10))
            .map(|(cid, _)| *cid)
            .collect();
        for cid in expired {
            auth_state.pending_auth.remove(&cid);
            net_inner.server.disconnect(cid);
            println!("[AUTH TIMEOUT] کلاینت {} بدون تیکت معتبر قطع شد.", cid);
        }
    }

    scratch.client_ids.clear();
    scratch.client_ids.extend(net_inner.server.clients_id());
    for &client_id in &scratch.client_ids {
        let is_spectator = auth_state.verified_spectators.contains_key(&client_id);
        let is_authenticated = auth_state.room_id.is_none()
            || auth_state.verified_clients.contains_key(&client_id)
            || is_spectator;

        while let Some(bytes) = net_inner
            .server
            .receive_message(client_id, CHANNEL_RELIABLE_LOBBY)
        {
            if let Ok(msg) = bincode::deserialize::<ClientLobbyMessage>(&bytes) {
                if is_spectator
                    && !matches!(
                        msg,
                        ClientLobbyMessage::SpectatorSetFogMode { .. }
                            | ClientLobbyMessage::SpectatorSetSpeed { .. }
                            | ClientLobbyMessage::SpectatorTogglePause
                            | ClientLobbyMessage::SpectatorSeekTo { .. }
                            | ClientLobbyMessage::SpectatorJumpLive
                    )
                {
                    continue;
                }
                match msg {
                    ClientLobbyMessage::Authenticate { ticket_id } => {
                        if let Some(ref expected_room) = auth_state.room_id.clone() {
                            if auth_state.verified_clients.contains_key(&client_id)
                                || auth_state.verified_spectators.contains_key(&client_id)
                            {
                                continue;
                            }
                            let ticket_opt =
                                verify_ticket_in_redis(&config.network.redis_addr, &ticket_id);
                            let room_matches = ticket_opt.as_ref().is_some_and(|t| {
                                t.room_id == *expected_room
                                    || t.room_id.replace('_', "-") == expected_room.replace('_', "-")
                            });

                            if let (true, Some(t)) = (room_matches, ticket_opt) {
                                if t.role.eq_ignore_ascii_case("spectator") {
                                    if auth_state.verified_spectators.len()
                                        >= config.spectator.max_spectators_per_match
                                    {
                                        println!(
                                            "[SPECTATOR REJECTED] ظرفیت تماشاگران اتاق {} تکمیل است.",
                                            expected_room
                                        );
                                        auth_state.pending_auth.remove(&client_id);
                                        net_inner.server.disconnect(client_id);
                                        continue;
                                    }
                                    let delay_secs = if t.is_custom_lobby {
                                        config.spectator.custom_lobby_live_delay_secs
                                    } else {
                                        config.spectator.ranked_live_delay_secs
                                    };
                                    let initial_t = (world.match_elapsed_secs - delay_secs).max(0.0);
                                    auth_state.pending_auth.remove(&client_id);
                                    let new_session = SpectatorSession {
                                        user_id: t.user_id.clone(),
                                        username: t.username.clone(),
                                        fog_mode: SpectatorFogMode::AllVision,
                                        playback_speed: 1.0,
                                        is_paused: false,
                                        playback_time_secs: initial_t,
                                        following_live: true,
                                        delay_secs,
                                        last_sent_frame_time: -1.0,
                                        last_sent_fog_mode: SpectatorFogMode::AllVision,
                                        last_sent_paused: false,
                                        last_sent_speed: 1.0,
                                        last_sent_viewers: 0,
                                        heartbeat_accum: 1.0,
                                    };
                                    auth_state.verified_spectators.insert(client_id, new_session.clone());
                                    let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::SpectatorJoin {
                                        client_id,
                                        session: new_session,
                                    });
                                    send_initial_configs_to_spectator(
                                        client_id,
                                        &config,
                                        &items_config,
                                        &mut net_inner.server,
                                        &match_state,
                                    );
                                    notify_django_spectator_count(
                                        &config,
                                        expected_room,
                                        auth_state.port,
                                        auth_state.verified_spectators.len(),
                                    );
                                    println!(
                                        "[SPECTATOR OK] تماشاگر {} ({}) متصل شد (جمع: {}, تاخیر: {:.0}s).",
                                        t.username,
                                        t.user_id,
                                        auth_state.verified_spectators.len(),
                                        delay_secs
                                    );
                                    continue;
                                }
                                let stale_cids: Vec<ClientId> = auth_state
                                    .verified_clients
                                    .iter()
                                    .filter(|(&cid, info)| cid != client_id && info.user_id == t.user_id)
                                    .map(|(&cid, _)| cid)
                                    .collect();
                                for old_cid in stale_cids {
                                    auth_state.verified_clients.remove(&old_cid);
                                    if let Some(old_slot) = match_state.on_client_disconnected(old_cid) {
                                        if match_state.phase == MatchPhase::InGame {
                                            world.set_slot_connected(old_slot, false, &config);
                                        }
                                    }
                                    net_inner.server.disconnect(old_cid);
                                    println!(
                                        "[AUTH RECONNECT] نشست قبلی {} برای بازیکن {} جایگزین شد.",
                                        old_cid, t.username
                                    );
                                }
                                if let Some(slot_idx) = match_state.on_client_connected_with_user(
                                    client_id,
                                    Some(&t.user_id),
                                    &config,
                                ) {
                                    auth_state.pending_auth.remove(&client_id);
                                    auth_state.verified_clients.insert(
                                        client_id,
                                        VerifiedTicketInfo {
                                            user_id: t.user_id.clone(),
                                            username: t.username.clone(),
                                            ticket_id: ticket_id.clone(),
                                            slot_index: slot_idx,
                                        },
                                    );
                                    send_initial_configs_and_sync_world(
                                        client_id,
                                        &config,
                                        &items_config,
                                        &mut net_inner.server,
                                        &match_state,
                                        &mut world,
                                        slot_idx,
                                    );
                                    let live_player = if match_state.phase == MatchPhase::InGame {
                                        world.players.iter().find(|p| p.slot_index == slot_idx)
                                    } else {
                                        None
                                    };
                                    let lobby_hero_id = match_state
                                        .players
                                        .get(slot_idx)
                                        .and_then(|lp| lp.selected_hero_id.as_deref())
                                        .and_then(|hid| resolve_hero_id_1based(&config, hid));
                                    notify_django_player_state(
                                        &config,
                                        expected_room,
                                        auth_state.port,
                                        &t.user_id,
                                        &ticket_id,
                                        slot_idx,
                                        "connected",
                                        lobby_hero_id,
                                        live_player,
                                    );
                                    println!(
                                        "[AUTH OK] بازیکن {} ({}) با تیکت معتبر در جایگاه {} متصل شد ({}/{}).",
                                        t.username,
                                        t.user_id,
                                        slot_idx,
                                        match_state.connected_count(),
                                        match_state.required_players
                                    );
                                } else {
                                    println!(
                                        "[AUTH REJECTED] ظرفیت پر یا ورود تکراری برای {}",
                                        t.user_id
                                    );
                                    auth_state.pending_auth.remove(&client_id);
                                    net_inner.server.disconnect(client_id);
                                }
                            } else {
                                println!(
                                    "[AUTH REJECTED] تیکت نامعتبر یا متعلق به اتاق دیگر از کلاینت {}",
                                    client_id
                                );
                                auth_state.pending_auth.remove(&client_id);
                                net_inner.server.disconnect(client_id);
                            }
                        }
                    }
                    _ if !is_authenticated => {
                        continue;
                    }
                    ClientLobbyMessage::SelectHero { hero_id } => {
                        if match_state.try_select_hero(client_id, &hero_id, &config) {
                            println!("بازیکن {} هیروی '{}' را انتخاب کرد.", client_id, hero_id);
                            if let (Some(ref rid), Some(vinfo), Some(hid_1based)) = (
                                &auth_state.room_id,
                                auth_state.verified_clients.get(&client_id),
                                resolve_hero_id_1based(&config, &hero_id),
                            ) {
                                notify_django_player_state(
                                    &config,
                                    rid,
                                    auth_state.port,
                                    &vinfo.user_id,
                                    &vinfo.ticket_id,
                                    vinfo.slot_index,
                                    "connected",
                                    Some(hid_1based),
                                    None,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::ReportMapLoaded => {
                        if match_state.on_map_loaded(client_id) {
                            println!("کلاینت {} آبجکتهای نقشه را لود کرد.", client_id);
                        }
                    }
                    ClientLobbyMessage::SendChat { all_chat, text } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                world.push_chat_message(slot_idx, all_chat, &text, &config);
                            }
                        }
                    }
                    ClientLobbyMessage::SendPing { world_pos } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                world.push_ping(slot_idx, world_pos, &config);
                            }
                        }
                    }
                    ClientLobbyMessage::TogglePause => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                world.toggle_pause(slot_idx, &config);
                            }
                        }
                    }
                    ClientLobbyMessage::BuyItem { item_id } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_buy_item(&mut world, slot_idx, &item_id, &config, &items_config);
                            }
                        }
                    }
                    ClientLobbyMessage::BuyCart { item_ids } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_buy_cart(&mut world, slot_idx, &item_ids, &config, &items_config);
                            }
                        }
                    }
                    ClientLobbyMessage::SellItem { inv_index } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_sell_item(&mut world, slot_idx, inv_index, &config, &items_config);
                            }
                        }
                    }
                    ClientLobbyMessage::CycleItemMode { inv_index } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_cycle_item_mode(
                                    &mut world,
                                    slot_idx,
                                    inv_index,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::SwapInventorySlots { from_slot, to_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_swap_inventory_slots(
                                    &mut world,
                                    slot_idx,
                                    from_slot,
                                    to_slot,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::SellCourierItem { inv_index } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_sell_courier_item(
                                    &mut world,
                                    slot_idx,
                                    inv_index,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::CourierDeliver => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_courier_deliver(&mut world, slot_idx, &config, &items_config);
                            }
                        }
                    }
                    ClientLobbyMessage::CourierReturnToFountain => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_courier_return_to_fountain(&mut world, slot_idx);
                            }
                        }
                    }
                    ClientLobbyMessage::CourierMoveTo { world_pos } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_courier_move_to(&mut world, slot_idx, world_pos, &config);
                            }
                        }
                    }
                    ClientLobbyMessage::TransferHeroItemToCourier { hero_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_transfer_hero_item_to_courier(
                                    &mut world,
                                    slot_idx,
                                    hero_slot,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::TransferCourierItemToHero { courier_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_transfer_courier_item_to_hero(
                                    &mut world,
                                    slot_idx,
                                    courier_slot,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::BuyRecipe { item_id } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_buy_recipe(&mut world, slot_idx, &item_id, &config, &items_config);
                            }
                        }
                    }
                    ClientLobbyMessage::MoveHeroItemToStash { hero_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_move_hero_item_to_stash(
                                    &mut world,
                                    slot_idx,
                                    hero_slot,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::MoveStashItemToHero { stash_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_move_stash_item_to_hero(
                                    &mut world,
                                    slot_idx,
                                    stash_slot,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::MoveCourierItemToStash { courier_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_move_courier_item_to_stash(
                                    &mut world,
                                    slot_idx,
                                    courier_slot,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::MoveStashItemToCourier { stash_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_move_stash_item_to_courier(
                                    &mut world,
                                    slot_idx,
                                    stash_slot,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::SellStashItem { stash_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_sell_stash_item(&mut world, slot_idx, stash_slot, &items_config);
                            }
                        }
                    }
                    ClientLobbyMessage::TakeAllStashToHero => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_take_all_stash_to_hero(
                                    &mut world,
                                    slot_idx,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::StartTeleport {
                        inv_index,
                        target_pos,
                        from_minimap,
                    } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_start_teleport(
                                    &mut world,
                                    slot_idx,
                                    inv_index,
                                    target_pos,
                                    from_minimap,
                                    &config,
                                    &items_config,
                                    &physics_world,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::CancelTeleport => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_cancel_teleport(&mut world, slot_idx);
                            }
                        }
                    }
                    ClientLobbyMessage::ActivateShadowBlade { inv_index } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_activate_shadow_blade(
                                    &mut world,
                                    slot_idx,
                                    inv_index,
                                    &items_config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::ActivateItem { inv_index } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_activate_item_ability(
                                    &mut world,
                                    slot_idx,
                                    inv_index,
                                    &config,
                                    &items_config,
                                );
                            }
                        }
                    }
                    // دستورات کنترل پخش و مه جنگ مخصوص تماشاگر (Server-Authoritative)
                    ClientLobbyMessage::SpectatorSetFogMode { mode } => {
                        if let Some(spec) = auth_state.verified_spectators.get_mut(&client_id) {
                            spec.fog_mode = mode;
                            let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::SetFogMode { client_id, mode });
                        }
                    }
                    ClientLobbyMessage::SpectatorSetSpeed { speed } => {
                        if let Some(spec) = auth_state.verified_spectators.get_mut(&client_id) {
                            let valid_speed = config
                                .spectator
                                .playback_speeds
                                .iter()
                                .copied()
                                .min_by(|a, b| {
                                    (*a - speed)
                                        .abs()
                                        .partial_cmp(&(*b - speed).abs())
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                })
                                .unwrap_or(1.0);
                            spec.playback_speed = valid_speed;
                            if valid_speed < 1.0 {
                                spec.following_live = false;
                            }
                            let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::SetSpeed {
                                client_id,
                                speed: valid_speed,
                            });
                        }
                    }
                    ClientLobbyMessage::SpectatorTogglePause => {
                        if let Some(spec) = auth_state.verified_spectators.get_mut(&client_id) {
                            spec.is_paused = !spec.is_paused;
                            if spec.is_paused {
                                spec.following_live = false;
                            }
                            let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::TogglePause { client_id });
                        }
                    }
                    ClientLobbyMessage::SpectatorSeekTo { target_time_secs } => {
                        if let Some(spec) = auth_state.verified_spectators.get_mut(&client_id) {
                            spec.playback_time_secs = target_time_secs.max(0.0);
                            spec.following_live = false;
                            let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::SeekTo {
                                client_id,
                                target_time_secs,
                            });
                        }
                    }
                    ClientLobbyMessage::SpectatorJumpLive => {
                        if let Some(spec) = auth_state.verified_spectators.get_mut(&client_id) {
                            spec.is_paused = false;
                            spec.playback_speed = 1.0;
                            spec.following_live = true;
                            let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::JumpLive { client_id });
                        }
                    }
                    // امنیت سرور آنلاین: دستورات Demo Hero هرگز روی سرور اصلی اجرا نمی‌شوند
                    ClientLobbyMessage::DemoCommand(_) => {}
                    ClientLobbyMessage::SwapCourierSlots { from_slot, to_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_swap_courier_slots(
                                    &mut world,
                                    slot_idx,
                                    from_slot,
                                    to_slot,
                                    &config,
                                );
                            }
                        }
                    }
                    ClientLobbyMessage::SwapStashSlots { from_slot, to_slot } => {
                        if match_state.phase == MatchPhase::InGame {
                            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                                try_swap_stash_slots(
                                    &mut world,
                                    slot_idx,
                                    from_slot,
                                    to_slot,
                                    &items_config,
                                );
                            }
                        }
                    }
                }
            }
        }

        while let Some(bytes) = net_inner
            .server
            .receive_message(client_id, CHANNEL_UNRELIABLE_STATE)
        {
            if !is_authenticated || is_spectator || match_state.phase != MatchPhase::InGame {
                continue;
            }
            if let Some(slot_idx) = match_state.slot_for_client(client_id) {
                if let Ok(cmd) = bincode::deserialize::<ClientInputCommand>(&bytes) {
                    world.set_slot_input(slot_idx, cmd);
                }
            }
        }
    }
}

fn server_match_tick_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut auth_state: ResMut<ServerAuthState>,
    mut shutdown: ResMut<ServerShutdownTracker>,
    mut app_exit: MessageWriter<AppExit>,
    existing_soldiers: Query<Entity, With<PhysicsSoldier>>,
) {
    if match_state.tick(time.delta_secs(), &config).is_some() {
        let old_ents: Vec<Entity> = existing_soldiers.iter().collect();
        world.init_multiplayer_players(&config, &match_state, &mut commands, &old_ents);
        if let Some(ref rid) = auth_state.room_id {
            for vinfo in auth_state.verified_clients.values() {
                if let Some(live_p) = world.players.iter().find(|p| p.slot_index == vinfo.slot_index) {
                    notify_django_player_state(
                        &config,
                        rid,
                        auth_state.port,
                        &vinfo.user_id,
                        &vinfo.ticket_id,
                        vinfo.slot_index,
                        "connected",
                        Some(live_p.hero_index + 1),
                        Some(live_p),
                    );
                }
            }
        }
        println!(
            "همه {} بازیکن وارد زمین بازی شدند (تیم Dawn در غرب و تیم Dusk در شرق)!",
            world.players.len()
        );
    }

    if !auth_state.result_reported {
        if let Some(winner) = world.winning_team {
            auth_state.result_reported = true;
            if let Some(ref rid) = auth_state.room_id {
                let winner_team_num = match winner {
                    TeamId::Dawn => 1,
                    TeamId::Dusk => 2,
                    TeamId::Neutral => 0,
                };
                let duration_sec = world.match_elapsed_secs.round() as i32;
                let mut players_json = String::from("[");
                for (i, p) in world.players.iter().enumerate() {
                    if i > 0 {
                        players_json.push(',');
                    }
                    let uid = match_state
                        .players
                        .get(p.slot_index)
                        .and_then(|lp| lp.user_id.as_deref())
                        .unwrap_or("");
                    let items_json = serialize_final_items_json(&p.inventory);
                    players_json.push_str(&format!(
                        "{{\"user_id\":\"{}\",\"slot_index\":{},\"hero_id\":{},\"level\":{},\"kills\":{},\"deaths\":{},\"assists\":{},\"last_hits\":{},\"denies\":{},\"gold_coins\":{},\"final_items\":{}}}",
                        escape_json_str(uid),
                        p.slot_index,
                        p.hero_index + 1,
                        p.hero_level,
                        p.kills,
                        p.deaths,
                        p.assists,
                        p.last_hits,
                        p.denies,
                        p.hero_gold,
                        items_json
                    ));
                }
                players_json.push(']');
                let payload = format!(
                    "{{\"room_id\":\"{}\",\"port\":{},\"winner_team\":{},\"duration_sec\":{},\"players\":{}}}",
                    rid, auth_state.port, winner_team_num, duration_sec, players_json
                );
                post_json_to_django_async(
                    &config.network.backend_http_url,
                    "/api/v1/match/report-result/",
                    payload,
                );
                println!(
                    "[MATCH RESULT REPORTED] Room: {} | Winner: {} | Duration: {}s",
                    rid, winner_team_num, duration_sec
                );
            }
        }
    }

    let dt = time.delta_secs();
    if match_state.connected_count() > 0 {
        shutdown.ever_had_client = true;
        shutdown.empty_room_elapsed = 0.0;
    } else if shutdown.ever_had_client || auth_state.room_id.is_some() {
        shutdown.empty_room_elapsed += dt;
        if shutdown.empty_room_elapsed >= config.network.empty_room_shutdown_secs {
            println!(
                "[SERVER SHUTDOWN] هیچ بازیکنی به مدت {:.0} ثانیه متصل نیست؛ خروج خودکار برای آزادسازی پورت {}.",
                shutdown.empty_room_elapsed, auth_state.port
            );
            app_exit.write(AppExit::Success);
            return;
        }
    }

    if world.winning_team.is_some() {
        shutdown.post_match_elapsed += dt;
        if shutdown.post_match_elapsed >= config.network.post_match_shutdown_delay_secs {
            println!(
                "[SERVER SHUTDOWN] مسابقه به پایان رسید؛ خروج خودکار پس از {:.0} ثانیه برای آزادسازی پورت {}.",
                shutdown.post_match_elapsed, auth_state.port
            );
            app_exit.write(AppExit::Success);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn server_send_snapshot_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    mut net: ResMut<ServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut auth_state: ResMut<ServerAuthState>,
    mut tv_relay: ResMut<SourceTvRelay>,
    mut lobby_timer: ResMut<LobbyBroadcastTimer>,
    mut scratch: ResMut<ServerScratchBuffers>,
) {
    lobby_timer.0.tick(time.delta());
    let should_send_lobby =
        match_state.dirty || (match_state.phase != MatchPhase::InGame && lobby_timer.0.just_finished());

    if should_send_lobby {
        let snap = match_state.snapshot();
        for p in &match_state.players {
            if let (true, Some(cid)) = (p.connected, p.client_id) {
                let msg = ServerLobbyPacket::MatchState(ServerMatchMessage {
                    your_slot: p.slot_index,
                    state: snap.clone(),
                });
                scratch.serialize_buf.clear();
                if bincode::serialize_into(&mut scratch.serialize_buf, &msg).is_ok() {
                    net.server.send_message(
                        cid,
                        CHANNEL_RELIABLE_LOBBY,
                        scratch.serialize_buf.clone(),
                    );
                }
            }
        }
        for &spec_cid in auth_state.verified_spectators.keys() {
            let msg = ServerLobbyPacket::MatchState(ServerMatchMessage {
                your_slot: usize::MAX,
                state: snap.clone(),
            });
            scratch.serialize_buf.clear();
            if bincode::serialize_into(&mut scratch.serialize_buf, &msg).is_ok() {
                net.server.send_message(
                    spec_cid,
                    CHANNEL_RELIABLE_LOBBY,
                    scratch.serialize_buf.clone(),
                );
            }
        }
        match_state.dirty = false;
    }

    if match_state.phase == MatchPhase::InGame {
        for p in &match_state.players {
            if let (true, Some(cid)) = (p.connected, p.client_id) {
                let snap = world.build_snapshot_for_slot(&config, &items_config, p.slot_index);
                scratch.serialize_buf.clear();
                if bincode::serialize_into(&mut scratch.serialize_buf, &snap).is_ok() {
                    net.server.send_message(
                        cid,
                        CHANNEL_UNRELIABLE_STATE,
                        scratch.serialize_buf.clone(),
                    );
                }
            }
        }

        if !world.pending_fx.is_empty() || !world.pending_private_fx.is_empty() {
            let fx_batch: Vec<GameplayFxEvent> = world.pending_fx.drain(..).collect();
            let private_fx_batch: Vec<(usize, GameplayFxEvent)> =
                world.pending_private_fx.drain(..).collect();
            for p in &match_state.players {
                if let (true, Some(cid)) = (p.connected, p.client_id) {
                    let mut filtered_fx =
                        world.filter_fx_for_slot(&config, p.slot_index, &fx_batch);
                    for (target_slot, ev) in &private_fx_batch {
                        if *target_slot == p.slot_index {
                            filtered_fx.push(ev.clone());
                        }
                    }
                    if filtered_fx.is_empty() {
                        continue;
                    }
                    scratch.serialize_buf.clear();
                    if bincode::serialize_into(&mut scratch.serialize_buf, &filtered_fx).is_ok() {
                        net.server.send_message(
                            cid,
                            CHANNEL_RELIABLE_FX,
                            scratch.serialize_buf.clone(),
                        );
                    }
                }
            }
        }
    }

    if match_state.phase == MatchPhase::InGame {
        let dt = time.delta_secs();
        let record_interval = 1.0 / config.spectator.snapshot_record_hz;
        tv_relay.record_accum += dt;
        if tv_relay.record_accum >= record_interval {
            tv_relay.record_accum = 0.0;
            let base_snapshot = world.build_spectator_snapshot(
                &config,
                &items_config,
                SpectatorFogMode::AllVision,
            );
            let dawn_fog_mask = world.pack_team_fog_mask(TeamId::Dawn);
            let dusk_fog_mask = world.pack_team_fog_mask(TeamId::Dusk);
            let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::RecordFrame {
                match_time_secs: world.match_elapsed_secs,
                base_snapshot,
                dawn_fog_mask,
                dusk_fog_mask,
            });
        }

        if !auth_state.verified_spectators.is_empty() {
            tv_relay.step_accum += dt;
            let step_interval = (1.0 / config.spectator.snapshot_record_hz).min(0.05);
            if tv_relay.step_accum >= step_interval {
                let step_dt = tv_relay.step_accum;
                tv_relay.step_accum = 0.0;
                let _ = tv_relay.cmd_tx.try_send(SourceTvCommand::StepBroadcast {
                    dt: step_dt,
                    now_match_t: world.match_elapsed_secs,
                });
            }
            if let Ok(rx) = tv_relay.out_rx.try_lock() {
                while let Ok((spec_cid, bytes)) = rx.try_recv() {
                    if auth_state.verified_spectators.contains_key(&spec_cid) {
                        net.server.send_message(spec_cid, CHANNEL_UNRELIABLE_STATE, bytes);
                    }
                }
            }
        }
    }

    let net_inner = &mut *net;
    net_inner.transport.send_packets(&mut net_inner.server);
}