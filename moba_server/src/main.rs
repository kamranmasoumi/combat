use bevy::prelude::*;
use moba_shared::{
    AuthoritativeWorldState, ClientInputCommand, GameConfig, MobaPhysicsSet,
    MobaSharedPhysicsPlugin, PhysicsRole,
};
use std::net::{SocketAddr, UdpSocket};

#[derive(Resource)]
struct ServerNetwork {
    socket: UdpSocket,
    last_client_addr: Option<SocketAddr>,
    recv_buf: Vec<u8>,
}

fn main() {
    let config = GameConfig::load_from_file("config.ron");
    let tick_hz = config.network.server_tick_rate_hz;
    let substeps = config.physics.substeps;

    let socket = UdpSocket::bind(&config.network.server_bind_addr).unwrap_or_else(|e| {
        panic!(
            "خطای بحرانی سرور: امکان بایند روی پورت '{}' وجود ندارد: {}",
            config.network.server_bind_addr, e
        )
    });
    socket
        .set_nonblocking(true)
        .expect("خطای بحرانی: تنظیم حالت non-blocking سوکت سرور شکست خورد");

    println!(
        "=== سرور Authoritative (Rapier 3D) روی {} با نرخ {} هرتز فعال شد ===",
        config.network.server_bind_addr, tick_hz
    );

    let world_state = AuthoritativeWorldState::new_from_config(&config);
    let recv_buf_len = config.network.socket_recv_buffer_bytes;

    App::new()
        .add_plugins(MinimalPlugins)
        .insert_resource(config)
        .insert_resource(world_state)
        .insert_resource(ServerNetwork {
            socket,
            last_client_addr: None,
            recv_buf: vec![0u8; recv_buf_len],
        })
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
    mut net: ResMut<ServerNetwork>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let net = &mut *net;
    loop {
        match net.socket.recv_from(&mut net.recv_buf) {
            Ok((len, src_addr)) => {
                let cmd: ClientInputCommand = bincode::deserialize(&net.recv_buf[..len])
                    .expect("خطای بحرانی سرور: پکت ورودی کلاینت معتبر نیست");
                net.last_client_addr = Some(src_addr);
                if let Some(ref mut existing) = world.active_input {
                    existing.accumulate_from(cmd);
                } else {
                    world.active_input = Some(cmd);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => panic!("خطای بحرانی شبکه در دریافت پکت کلاینت: {}", e),
        }
    }
}

fn server_send_snapshot_system(
    net: Res<ServerNetwork>,
    mut world: ResMut<AuthoritativeWorldState>,
) {
    let Some(client_addr) = net.last_client_addr else {
        return;
    };
    if world.active_input.is_none() {
        return;
    }

    let snapshot = world.build_snapshot();
    let payload =
        bincode::serialize(&snapshot).expect("خطای بحرانی سرور: سریالایز کردن Snapshot شکست خورد");
    net.socket
        .send_to(&payload, client_addr)
        .expect("خطای بحرانی سرور: ارسال Snapshot به کلاینت شکست خورد");
}