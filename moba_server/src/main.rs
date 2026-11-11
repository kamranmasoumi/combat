use bevy::prelude::*;
use moba_shared::{
    create_shared_renet_server, AuthoritativeWorldState, ClientInputCommand, GameConfig,
    MobaPhysicsSet, MobaSharedPhysicsPlugin, PhysicsRole, CHANNEL_RELIABLE_FX,
    CHANNEL_UNRELIABLE_STATE,
};
use renet::{RenetServer, ServerEvent};
use renet_netcode::NetcodeServerTransport;

#[derive(Resource)]
struct ServerRenetResources {
    server: RenetServer,
    transport: NetcodeServerTransport,
}

fn main() {
    let config = GameConfig::load_from_file("config.ron");
    let tick_hz = config.network.server_tick_rate_hz;
    let substeps = config.physics.substeps;

    let (server, transport) = create_shared_renet_server(&config.network);

    println!(
        "=== سرور Authoritative (Renet + Custom 3D Physics) روی {} با نرخ {} هرتز فعال شد ===",
        config.network.server_bind_addr, tick_hz
    );

    let world_state = AuthoritativeWorldState::new_from_config(&config);

    App::new()
        .add_plugins(MinimalPlugins)
        .insert_resource(config)
        .insert_resource(world_state)
        .insert_resource(ServerRenetResources { server, transport })
        .add_plugins(MobaSharedPhysicsPlugin {
            role: PhysicsRole::Server,
            tick_rate_hz: tick_hz,
            substeps,
        })
        .add_systems(
            FixedUpdate,
            server_receive_input_system.in_set(MobaPhysicsSet::InputIngress),
        )
        .add_systems(
            FixedUpdate,
            server_send_snapshot_system.in_set(MobaPhysicsSet::NetworkEgress),
        )
        .run();
}

fn server_receive_input_system(
    time: Res<Time<Fixed>>,
    mut net: ResMut<ServerRenetResources>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let delta = time.delta();
    let net = &mut *net;

    net.server.update(delta);
    if let Err(e) = net.transport.update(delta, &mut net.server) {
        panic!("خطای بحرانی ترنسپورت Renet در سرور: {}", e);
    }

    while let Some(event) = net.server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                println!(">>> کلاینت با شناسه {} به سرور متصل شد.", client_id);
            }
            ServerEvent::ClientDisconnected { client_id, reason } => {
                println!(
                    ">>> کلاینت {} قطع شد (دلیل: {}).",
                    client_id, reason
                );
            }
        }
    }

    for client_id in net.server.clients_id() {
        while let Some(message) = net
            .server
            .receive_message(client_id, CHANNEL_UNRELIABLE_STATE)
        {
            let cmd: ClientInputCommand = bincode::deserialize(&message)
                .expect("خطای بحرانی سرور: پکت ورودی کلاینت معتبر نیست");
            if let Some(ref mut existing) = world.active_input {
                existing.accumulate_from(cmd);
            } else {
                world.active_input = Some(cmd);
            }
        }
    }
}

fn server_send_snapshot_system(
    mut net: ResMut<ServerRenetResources>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let net = &mut *net;

    if net.server.clients_id().is_empty() || world.active_input.is_none() {
        net.transport.send_packets(&mut net.server);
        return;
    }

    let fx_events = world.take_fx_events();
    if !fx_events.is_empty() {
        let fx_payload = bincode::serialize(&fx_events)
            .expect("خطای بحرانی سرور: سریالایز کردن رویدادهای FX شکست خورد");
        net.server
            .broadcast_message(CHANNEL_RELIABLE_FX, fx_payload);
    }

    let snapshot = world.build_snapshot();
    let snap_payload =
        bincode::serialize(&snapshot).expect("خطای بحرانی سرور: سریالایز کردن Snapshot شکست خورد");
    net.server
        .broadcast_message(CHANNEL_UNRELIABLE_STATE, snap_payload);

    net.transport.send_packets(&mut net.server);
}