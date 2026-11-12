use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use moba_shared::{
    create_shared_renet_server, parse_server_cli_args, AuthoritativeMatchState,
    AuthoritativeWorldState, ClientInputCommand, ClientLobbyMessage, GameConfig, GameplayFxEvent,
    MatchPhase, MobaPhysicsSet, MobaSharedPhysicsPlugin, PhysicsRole, PhysicsSoldier,
    ServerMatchMessage, CHANNEL_RELIABLE_FX, CHANNEL_RELIABLE_LOBBY, CHANNEL_UNRELIABLE_STATE,
};
use renet::{RenetServer, ServerEvent};
use renet_netcode::NetcodeServerTransport;
use std::time::Duration;

#[derive(Resource)]
struct ServerNetwork {
    server: RenetServer,
    transport: NetcodeServerTransport,
}

#[derive(Resource)]
struct LobbyBroadcastTimer(Timer);

fn main() {
    let (cli_port, required_players) = parse_server_cli_args();

    let mut config = GameConfig::load_from_file("config.ron");
    let ip = config
        .network
        .server_bind_addr
        .split(':')
        .next()
        .expect("آدرس سرور در config.ron نامعتبر است");
    config.network.server_bind_addr = format!("{}:{}", ip, cli_port);
    config.network.max_clients = required_players;

    let tick_hz = config.network.server_tick_rate_hz;
    let substeps = config.physics.substeps;

    let (server, transport) = create_shared_renet_server(&config.network);
    let world_state = AuthoritativeWorldState::new_from_config(&config);
    let match_state = AuthoritativeMatchState::new(required_players);

    println!(
        "سرور MOBA روی {} با ظرفیت {} بازیکن آماده اتصال است...",
        config.network.server_bind_addr, required_players
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
        .insert_resource(world_state)
        .insert_resource(match_state)
        .insert_resource(LobbyBroadcastTimer(Timer::from_seconds(
            0.2,
            TimerMode::Repeating,
        )))
        .insert_resource(ServerNetwork { server, transport })
        .add_systems(
            FixedUpdate,
            (
                server_network_receive_system.before(MobaPhysicsSet::Drive),
                server_match_tick_system.before(MobaPhysicsSet::Drive),
                server_send_snapshot_system.after(MobaPhysicsSet::React),
            ),
        )
        .run();
}

fn server_network_receive_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    mut net: ResMut<ServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let net_inner = &mut *net;
    net_inner.server.update(time.delta());
    if let Err(e) = net_inner.transport.update(time.delta(), &mut net_inner.server) {
        eprintln!("خطای transport سرور: {:?}", e);
    }

    while let Some(event) = net_inner.server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                if match_state.on_client_connected(client_id, &config) {
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
                match_state.on_client_disconnected(client_id);
            }
        }
    }

    let client_ids = net_inner.server.clients_id();
    for client_id in client_ids {
        while let Some(bytes) = net_inner
            .server
            .receive_message(client_id, CHANNEL_RELIABLE_LOBBY)
        {
            if let Ok(msg) = bincode::deserialize::<ClientLobbyMessage>(&bytes) {
                match msg {
                    ClientLobbyMessage::SelectHero { hero_id } => {
                        if match_state.try_select_hero(client_id, &hero_id, &config) {
                            println!("بازیکن {} هیروی '{}' را انتخاب کرد.", client_id, hero_id);
                        }
                    }
                    ClientLobbyMessage::ReportMapLoaded => {
                        if match_state.on_map_loaded(client_id) {
                            println!("کلاینت {} آبجکت‌های نقشه را لود کرد.", client_id);
                        }
                    }
                }
            }
        }

        // ورودی هر بازیکن فقط به اسلات خودش در دنیای بازی اعمال می‌شود (عدم تداخل بازیکن‌ها)
        while let Some(bytes) = net_inner
            .server
            .receive_message(client_id, CHANNEL_UNRELIABLE_STATE)
        {
            if match_state.phase != MatchPhase::InGame {
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
}

fn server_send_snapshot_system(
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    mut net: ResMut<ServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut lobby_timer: ResMut<LobbyBroadcastTimer>,
) {
    lobby_timer.0.tick(time.delta());
    let should_send_lobby =
        match_state.dirty || (match_state.phase != MatchPhase::InGame && lobby_timer.0.just_finished());

    if should_send_lobby {
        let snap = match_state.snapshot();
        for p in &match_state.players {
            if let (true, Some(cid)) = (p.connected, p.client_id) {
                let msg = ServerMatchMessage {
                    your_slot: p.slot_index,
                    state: snap.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&msg) {
                    net.server.send_message(cid, CHANNEL_RELIABLE_LOBBY, bytes);
                }
            }
        }
        match_state.dirty = false;
    }

    if match_state.phase == MatchPhase::InGame {
        // ارسال Snapshot اختصاصی برای هر بازیکن بر اساس جایگاه، هیرو و مه جنگ تیم خودش
        for p in &match_state.players {
            if let (true, Some(cid)) = (p.connected, p.client_id) {
                let snap = world.build_snapshot_for_slot(&config, p.slot_index);
                if let Ok(snap_bytes) = bincode::serialize(&snap) {
                    net.server
                        .send_message(cid, CHANNEL_UNRELIABLE_STATE, snap_bytes);
                }
            }
        }

        if !world.pending_fx.is_empty() {
            let fx_batch: Vec<GameplayFxEvent> = world.pending_fx.drain(..).collect();
            if let Ok(fx_bytes) = bincode::serialize(&fx_batch) {
                net.server
                    .broadcast_message(CHANNEL_RELIABLE_FX, fx_bytes);
            }
        }
    }

    let net_inner = &mut *net;
    net_inner.transport.send_packets(&mut net_inner.server);
}