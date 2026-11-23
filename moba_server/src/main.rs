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
    PhysicsRole, PhysicsSoldier, ServerLobbyPacket, ServerMatchMessage, TeamId,
    CHANNEL_RELIABLE_FX, CHANNEL_RELIABLE_LOBBY, CHANNEL_UNRELIABLE_STATE,
};
use renet::{ClientId, RenetServer, ServerEvent};
use renet_netcode::NetcodeServerTransport;
use std::collections::HashMap;
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

#[derive(Resource)]
struct ServerAuthState {
    room_id: Option<String>,
    port: u16,
    pending_auth: HashMap<ClientId, Instant>,
    verified_clients: HashMap<ClientId, VerifiedTicketInfo>,
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
    config.network.max_clients = required_players;

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
        .insert_resource(config)
        .insert_resource(items_config)
        .insert_resource(world_state)
        .insert_resource(match_state)
        .insert_resource(ServerAuthState {
            room_id: room_id_opt,
            port: cli_port,
            pending_auth: HashMap::new(),
            verified_clients: HashMap::new(),
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
    Some(RedisTicketPayload {
        room_id,
        user_id,
        username,
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

fn notify_django_player_state(
    config: &GameConfig,
    room_id: &str,
    port: u16,
    user_id: &str,
    ticket_id: &str,
    slot_index: usize,
    status: &str,
) {
    let body = format!(
        "{{\"room_id\":\"{}\",\"port\":{},\"user_id\":\"{}\",\"ticket_id\":\"{}\",\"slot_index\":{},\"status\":\"{}\"}}",
        room_id, port, user_id, ticket_id, slot_index, status
    );
    post_json_to_django_async(
        &config.network.backend_http_url,
        "/api/v1/match/player-state/",
        body,
    );
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
                        notify_django_player_state(
                            &config,
                            rid,
                            auth_state.port,
                            &vinfo.user_id,
                            &vinfo.ticket_id,
                            vinfo.slot_index,
                            "disconnected",
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
        let is_authenticated = auth_state.room_id.is_none()
            || auth_state.verified_clients.contains_key(&client_id);

        while let Some(bytes) = net_inner
            .server
            .receive_message(client_id, CHANNEL_RELIABLE_LOBBY)
        {
            if let Ok(msg) = bincode::deserialize::<ClientLobbyMessage>(&bytes) {
                match msg {
                    ClientLobbyMessage::Authenticate { ticket_id } => {
                        if let Some(ref expected_room) = auth_state.room_id.clone() {
                            if auth_state.verified_clients.contains_key(&client_id) {
                                continue;
                            }
                            let ticket_opt =
                                verify_ticket_in_redis(&config.network.redis_addr, &ticket_id);
                            let room_matches = ticket_opt.as_ref().is_some_and(|t| {
                                t.room_id == *expected_room
                                    || t.room_id.replace('_', "-") == expected_room.replace('_', "-")
                            });

                            if let (true, Some(t)) = (room_matches, ticket_opt) {
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
                                    notify_django_player_state(
                                        &config,
                                        expected_room,
                                        auth_state.port,
                                        &t.user_id,
                                        &ticket_id,
                                        slot_idx,
                                        "connected",
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
            if !is_authenticated || match_state.phase != MatchPhase::InGame {
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
                    players_json.push_str(&format!(
                        "{{\"user_id\":\"{}\",\"slot_index\":{},\"hero_id\":{},\"level\":{},\"kills\":{},\"deaths\":{},\"assists\":{},\"last_hits\":{},\"denies\":{},\"gold_coins\":{}}}",
                        uid,
                        p.slot_index,
                        p.hero_index + 1,
                        p.hero_level,
                        p.kills,
                        p.deaths,
                        p.assists,
                        p.last_hits,
                        p.denies,
                        p.hero_gold
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

fn server_send_snapshot_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    mut net: ResMut<ServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
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

    let net_inner = &mut *net;
    net_inner.transport.send_packets(&mut net_inner.server);
}