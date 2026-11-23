use crate::backend_client::ActiveMatchInfo;
use crate::pregame_ui::{AppScreenState, PendingMatchConnection};
use crate::shop_client::ShopOutgoingQueue;
use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Button as UiButton;
use moba_shared::{
    create_shared_renet_server, execute_demo_command, step_couriers_system,
    step_demo_dummy_ai_and_rules, step_shadow_blade_system, step_teleports_system,
    step_weather_and_environment_system, try_activate_item_ability, try_activate_shadow_blade,
    try_buy_cart, try_buy_item, try_buy_recipe, try_cancel_teleport, try_courier_deliver,
    try_courier_move_to, try_courier_return_to_fountain, try_cycle_item_mode,
    try_move_courier_item_to_stash, try_move_hero_item_to_stash, try_move_stash_item_to_courier,
    try_move_stash_item_to_hero, try_sell_courier_item, try_sell_item, try_sell_stash_item,
    try_start_teleport, try_swap_courier_slots, try_swap_inventory_slots, try_swap_stash_slots,
    try_take_all_stash_to_hero, try_transfer_courier_item_to_hero,
    try_transfer_hero_item_to_courier, AuthoritativeMatchState, AuthoritativeWorldState,
    ClientInputCommand, ClientLobbyMessage, CustomPhysicsWorld, DemoCommand, DemoSharedFlags,
    DemoTeleportPreset, DummyBehavior, GameConfig, GameplayFxEvent, ItemsConfig, LobbyPlayerInfo,
    MatchPhase, MobaPhysicsSet, MobaSharedPhysicsPlugin, PhysicsRole, PhysicsSoldier,
    ServerLobbyPacket, ServerMatchMessage, TeamId, WeatherKind, CHANNEL_RELIABLE_FX,
    CHANNEL_RELIABLE_LOBBY, CHANNEL_UNRELIABLE_STATE,
};
use renet::{ClientId, RenetServer, ServerEvent};
use renet_netcode::NetcodeServerTransport;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

// =============================================================================
// سرور لوکال تعبیه‌شده Demo Hero (پشتیبانی کامل از چند هیرو در هر دو تیم و سوئیچ کنترل کامل)
// =============================================================================

pub struct EmbeddedDemoServerHandle {
    pub addr: String,
    pub port: u16,
    stop_flag: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
}

impl EmbeddedDemoServerHandle {
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(jh) = self.join_handle.take() {
            let _ = jh.join();
        }
    }
}

impl Drop for EmbeddedDemoServerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Resource)]
struct DemoServerNetwork {
    server: RenetServer,
    transport: NetcodeServerTransport,
}

#[derive(Resource)]
struct DemoServerStopFlag(Arc<AtomicBool>);

#[derive(Resource, Default)]
struct DemoServerRuntime {
    flags: DemoSharedFlags,
    initialized: bool,
    owner_client_id: Option<ClientId>,
    serialize_buf: Vec<u8>,
}

fn spawn_embedded_demo_server(
    mut config: GameConfig,
    items_config: ItemsConfig,
    initial_hero_id: String,
) -> EmbeddedDemoServerHandle {
    let port = UdpSocket::bind("127.0.0.1:0")
        .and_then(|s| s.local_addr())
        .map(|a| a.port())
        .expect("خطای بحرانی: پورت آزاد UDP روی 127.0.0.1 برای Demo Hero یافت نشد");
    let addr = format!("127.0.0.1:{}", port);
    config.network.server_bind_addr = addr.clone();
    config.network.server_public_addr = addr.clone();
    config.network.max_clients = 2;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop_flag);

    let join_handle = thread::spawn(move || {
        run_embedded_demo_server_loop(config, items_config, initial_hero_id, thread_stop);
    });

    thread::sleep(Duration::from_millis(60));

    EmbeddedDemoServerHandle {
        addr,
        port,
        stop_flag,
        join_handle: Some(join_handle),
    }
}

fn run_embedded_demo_server_loop(
    config: GameConfig,
    items_config: ItemsConfig,
    initial_hero_id: String,
    stop_flag: Arc<AtomicBool>,
) {
    let tick_hz = config.network.server_tick_rate_hz;
    let substeps = config.physics.substeps;
    let (server, transport) = create_shared_renet_server(&config.network);
    let world_state = AuthoritativeWorldState::new_from_config(&config);

    let player_hero_id = if config.heroes.iter().any(|h| h.id == initial_hero_id) {
        initial_hero_id
    } else {
        config.heroes[0].id.clone()
    };
    let dummy_hero_id = config
        .heroes
        .iter()
        .find(|h| h.id != player_hero_id)
        .unwrap_or(&config.heroes[0])
        .id
        .clone();

    let mut match_state = AuthoritativeMatchState::new(2);
    match_state.phase = MatchPhase::InGame;
    match_state.players = vec![
        LobbyPlayerInfo {
            slot_index: 0,
            client_id: None,
            user_id: Some("demo_hero_0".to_string()),
            team: TeamId::Dawn,
            connected: false,
            selected_hero_id: Some(player_hero_id),
            map_loaded: true,
        },
        LobbyPlayerInfo {
            slot_index: 1,
            client_id: None,
            user_id: Some("demo_hero_1".to_string()),
            team: TeamId::Dusk,
            connected: true,
            selected_hero_id: Some(dummy_hero_id),
            map_loaded: true,
        },
    ];
    match_state.dirty = true;

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
        .insert_resource(DemoServerNetwork { server, transport })
        .insert_resource(DemoServerStopFlag(stop_flag))
        .init_resource::<DemoServerRuntime>()
        .add_systems(
            FixedUpdate,
            (
                demo_server_init_and_receive_system.before(MobaPhysicsSet::Drive),
                demo_server_ai_tick_system.before(MobaPhysicsSet::Drive),
                step_couriers_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system),
                step_teleports_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system),
                step_shadow_blade_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system),
                step_weather_and_environment_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system),
                demo_server_send_snapshot_system.after(MobaPhysicsSet::React),
            ),
        )
        .run();
}

#[allow(clippy::too_many_arguments)]
fn demo_server_init_and_receive_system(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    physics_world: Res<CustomPhysicsWorld>,
    stop_flag: Res<DemoServerStopFlag>,
    mut app_exit: MessageWriter<AppExit>,
    mut net: ResMut<DemoServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut demo: ResMut<DemoServerRuntime>,
    existing_soldiers: Query<(Entity, &PhysicsSoldier)>,
) {
    if stop_flag.0.load(Ordering::SeqCst) {
        app_exit.write(AppExit::Success);
        return;
    }

    if !demo.initialized {
        demo.initialized = true;
        let old_ents: Vec<Entity> = existing_soldiers.iter().map(|(e, _)| e).collect();
        world.init_multiplayer_players(&config, &match_state, &mut commands, &old_ents);
        if world.players.len() >= 2 {
            world.players[0].hero_gold = 10_000;
            world.players[1].hero_gold = 10_000;
            let mid_pos = Vec3::new(-6.0, 0.2, 0.0);
            let dummy_pos = Vec3::new(8.0, 0.2, 0.0);
            world.players[0].soldier_pos = mid_pos;
            world.players[0].spawn_pos = mid_pos;
            world.players[1].soldier_pos = dummy_pos;
            world.players[1].spawn_pos = dummy_pos;
            world.players[1].is_disconnected = false;
        }
    }

    let net_inner = &mut *net;
    net_inner.server.update(time.delta());
    let _ = net_inner.transport.update(time.delta(), &mut net_inner.server);

    while let Some(event) = net_inner.server.get_event() {
        match event {
            ServerEvent::ClientConnected { client_id } => {
                demo.owner_client_id = Some(client_id);
                if let Some(p0) = match_state.players.get_mut(0) {
                    p0.client_id = Some(client_id);
                    p0.connected = true;
                    p0.map_loaded = true;
                }
                match_state.dirty = true;
                world.set_slot_connected(0, true, &config);

                let sync_pkt = ServerLobbyPacket::SyncConfigs {
                    game_config: config.clone(),
                    items_config: items_config.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&sync_pkt) {
                    net_inner
                        .server
                        .send_message(client_id, CHANNEL_RELIABLE_LOBBY, bytes);
                }
                let ctrl = demo
                    .flags
                    .controlled_slot
                    .min(world.players.len().saturating_sub(1));
                let match_pkt = ServerLobbyPacket::MatchState(ServerMatchMessage {
                    your_slot: ctrl,
                    state: match_state.snapshot(),
                });
                if let Ok(bytes) = bincode::serialize(&match_pkt) {
                    net_inner
                        .server
                        .send_message(client_id, CHANNEL_RELIABLE_LOBBY, bytes);
                }
            }
            ServerEvent::ClientDisconnected { client_id, .. } => {
                if demo.owner_client_id == Some(client_id) {
                    demo.owner_client_id = None;
                    stop_flag.0.store(true, Ordering::SeqCst);
                    app_exit.write(AppExit::Success);
                    return;
                }
            }
        }
    }

    let Some(client_id) = demo.owner_client_id else {
        return;
    };

    while let Some(bytes) = net_inner
        .server
        .receive_message(client_id, CHANNEL_RELIABLE_LOBBY)
    {
        if let Ok(msg) = bincode::deserialize::<ClientLobbyMessage>(&bytes) {
            let ctrl = demo
                .flags
                .controlled_slot
                .min(world.players.len().saturating_sub(1));
            match msg {
                ClientLobbyMessage::DemoCommand(cmd) => {
                    let is_remove_extra = matches!(cmd, DemoCommand::RemoveExtraHeroes);
                    execute_demo_command(
                        cmd,
                        &mut world,
                        &mut match_state,
                        &mut demo.flags,
                        &config,
                        &items_config,
                        &mut commands,
                    );
                    if is_remove_extra {
                        for (ent, ps) in &existing_soldiers {
                            if ps.slot_index >= 2 {
                                commands.entity(ent).despawn();
                            }
                        }
                    }
                }
                ClientLobbyMessage::BuyItem { item_id } => {
                    try_buy_item(&mut world, ctrl, &item_id, &config, &items_config);
                }
                ClientLobbyMessage::BuyRecipe { item_id } => {
                    try_buy_recipe(&mut world, ctrl, &item_id, &config, &items_config);
                }
                ClientLobbyMessage::BuyCart { item_ids } => {
                    try_buy_cart(&mut world, ctrl, &item_ids, &config, &items_config);
                }
                ClientLobbyMessage::SellItem { inv_index } => {
                    try_sell_item(&mut world, ctrl, inv_index, &config, &items_config);
                }
                ClientLobbyMessage::CycleItemMode { inv_index } => {
                    try_cycle_item_mode(&mut world, ctrl, inv_index, &config, &items_config);
                }
                ClientLobbyMessage::SwapInventorySlots { from_slot, to_slot } => {
                    try_swap_inventory_slots(&mut world, ctrl, from_slot, to_slot, &items_config);
                }
                ClientLobbyMessage::SwapCourierSlots { from_slot, to_slot } => {
                    try_swap_courier_slots(&mut world, ctrl, from_slot, to_slot, &config);
                }
                ClientLobbyMessage::SwapStashSlots { from_slot, to_slot } => {
                    try_swap_stash_slots(&mut world, ctrl, from_slot, to_slot, &items_config);
                }
                ClientLobbyMessage::SellCourierItem { inv_index } => {
                    try_sell_courier_item(&mut world, ctrl, inv_index, &config, &items_config);
                }
                ClientLobbyMessage::CourierDeliver => {
                    try_courier_deliver(&mut world, ctrl, &config, &items_config);
                }
                ClientLobbyMessage::CourierReturnToFountain => {
                    try_courier_return_to_fountain(&mut world, ctrl);
                }
                ClientLobbyMessage::CourierMoveTo { world_pos } => {
                    try_courier_move_to(&mut world, ctrl, world_pos, &config);
                }
                ClientLobbyMessage::TransferHeroItemToCourier { hero_slot } => {
                    try_transfer_hero_item_to_courier(
                        &mut world,
                        ctrl,
                        hero_slot,
                        &config,
                        &items_config,
                    );
                }
                ClientLobbyMessage::TransferCourierItemToHero { courier_slot } => {
                    try_transfer_courier_item_to_hero(
                        &mut world,
                        ctrl,
                        courier_slot,
                        &config,
                        &items_config,
                    );
                }
                ClientLobbyMessage::MoveHeroItemToStash { hero_slot } => {
                    try_move_hero_item_to_stash(
                        &mut world,
                        ctrl,
                        hero_slot,
                        &config,
                        &items_config,
                    );
                }
                ClientLobbyMessage::MoveStashItemToHero { stash_slot } => {
                    try_move_stash_item_to_hero(
                        &mut world,
                        ctrl,
                        stash_slot,
                        &config,
                        &items_config,
                    );
                }
                ClientLobbyMessage::MoveCourierItemToStash { courier_slot } => {
                    try_move_courier_item_to_stash(
                        &mut world,
                        ctrl,
                        courier_slot,
                        &config,
                        &items_config,
                    );
                }
                ClientLobbyMessage::MoveStashItemToCourier { stash_slot } => {
                    try_move_stash_item_to_courier(
                        &mut world,
                        ctrl,
                        stash_slot,
                        &config,
                        &items_config,
                    );
                }
                ClientLobbyMessage::SellStashItem { stash_slot } => {
                    try_sell_stash_item(&mut world, ctrl, stash_slot, &items_config);
                }
                ClientLobbyMessage::TakeAllStashToHero => {
                    try_take_all_stash_to_hero(&mut world, ctrl, &config, &items_config);
                }
                ClientLobbyMessage::StartTeleport {
                    inv_index,
                    target_pos,
                    from_minimap,
                } => {
                    try_start_teleport(
                        &mut world,
                        ctrl,
                        inv_index,
                        target_pos,
                        from_minimap,
                        &config,
                        &items_config,
                        &physics_world,
                    );
                }
                ClientLobbyMessage::CancelTeleport => {
                    try_cancel_teleport(&mut world, ctrl);
                }
                ClientLobbyMessage::ActivateShadowBlade { inv_index } => {
                    try_activate_shadow_blade(&mut world, ctrl, inv_index, &items_config);
                }
                ClientLobbyMessage::ActivateItem { inv_index } => {
                    try_activate_item_ability(&mut world, ctrl, inv_index, &config, &items_config);
                }
                _ => {}
            }
        }
    }

    let ctrl = demo
        .flags
        .controlled_slot
        .min(world.players.len().saturating_sub(1));
    while let Some(bytes) = net_inner
        .server
        .receive_message(client_id, CHANNEL_UNRELIABLE_STATE)
    {
        if let Ok(cmd) = bincode::deserialize::<ClientInputCommand>(&bytes) {
            world.set_slot_input(ctrl, cmd);
        }
    }
}

fn demo_server_ai_tick_system(
    config: Res<GameConfig>,
    mut world: ResMut<AuthoritativeWorldState>,
    demo: Res<DemoServerRuntime>,
) {
    step_demo_dummy_ai_and_rules(&config, &mut world, &demo.flags);
}

fn demo_server_send_snapshot_system(
    config: Res<GameConfig>,
    items_config: Res<ItemsConfig>,
    mut net: ResMut<DemoServerNetwork>,
    mut match_state: ResMut<AuthoritativeMatchState>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut demo: ResMut<DemoServerRuntime>,
) {
    let Some(cid) = demo.owner_client_id else {
        return;
    };
    let ctrl = demo
        .flags
        .controlled_slot
        .min(world.players.len().saturating_sub(1));

    if match_state.dirty {
        let msg = ServerLobbyPacket::MatchState(ServerMatchMessage {
            your_slot: ctrl,
            state: match_state.snapshot(),
        });
        demo.serialize_buf.clear();
        if bincode::serialize_into(&mut demo.serialize_buf, &msg).is_ok() {
            net.server
                .send_message(cid, CHANNEL_RELIABLE_LOBBY, demo.serialize_buf.clone());
        }
        match_state.dirty = false;
    }

    let snap = world.build_snapshot_for_slot(&config, &items_config, ctrl);
    demo.serialize_buf.clear();
    if bincode::serialize_into(&mut demo.serialize_buf, &snap).is_ok() {
        net.server
            .send_message(cid, CHANNEL_UNRELIABLE_STATE, demo.serialize_buf.clone());
    }

    if !world.pending_fx.is_empty() || !world.pending_private_fx.is_empty() {
        let fx_batch: Vec<GameplayFxEvent> = world.pending_fx.drain(..).collect();
        let private_fx_batch: Vec<(usize, GameplayFxEvent)> =
            world.pending_private_fx.drain(..).collect();
        let mut filtered_fx = world.filter_fx_for_slot(&config, ctrl, &fx_batch);
        for (target_slot, ev) in private_fx_batch {
            if target_slot == ctrl {
                filtered_fx.push(ev);
            }
        }
        if !filtered_fx.is_empty() {
            demo.serialize_buf.clear();
            if bincode::serialize_into(&mut demo.serialize_buf, &filtered_fx).is_ok() {
                net.server
                    .send_message(cid, CHANNEL_RELIABLE_FX, demo.serialize_buf.clone());
            }
        }
    }

    let net_inner = &mut *net;
    net_inner.transport.send_packets(&mut net_inner.server);
}

// =============================================================================
// منابع و رابط کاربری مستقل Demo Hero در کلاینت
// =============================================================================

#[derive(Resource, Default)]
pub struct PendingDemoLaunch(pub Option<String>);

#[derive(Clone, Debug)]
pub struct DemoSlotEntry {
    pub slot_index: usize,
    pub team: TeamId,
    pub hero_name: String,
}

#[derive(Resource)]
pub struct ActiveDemoSession {
    pub handle: Option<EmbeddedDemoServerHandle>,
    pub is_active: bool,
    pub panel_open: bool,
    pub free_cooldowns: bool,
    pub reveal_full_map: bool,
    pub is_night: bool,
    pub weather: WeatherKind,
    pub dummy_behavior: DummyBehavior,
    pub controlled_slot: usize,
    pub slots: Vec<DemoSlotEntry>,
    pub spawn_hero_idx: usize,
    pub dummy_item_idx: usize,
    pub ui_dirty: bool,
}

impl Default for ActiveDemoSession {
    fn default() -> Self {
        Self {
            handle: None,
            is_active: false,
            panel_open: true,
            free_cooldowns: false,
            reveal_full_map: false,
            is_night: false,
            weather: WeatherKind::Clear,
            dummy_behavior: DummyBehavior::Idle,
            controlled_slot: 0,
            slots: Vec::new(),
            spawn_hero_idx: 0,
            dummy_item_idx: 0,
            ui_dirty: true,
        }
    }
}

#[derive(Component)]
struct DemoPanelRoot;

#[derive(Component, Clone)]
enum DemoBtnAction {
    TogglePanel,
    ControlSlot(usize),
    CycleControlSlot,
    NextSpawnHero,
    SpawnHero(TeamId),
    RemoveExtraHeroes,
    ChangeControlledHero,
    LevelUp,
    MaxLevel,
    AddGold,
    RefreshHero,
    ToggleFreeCooldowns,
    RespawnDummiesNearPlayer,
    CycleDummyBehavior,
    NextDummyItem,
    GiveOtherHeroesSelectedItem,
    ForceOtherHeroesUseSlot0,
    ClearOtherHeroesItems,
    SetWeather(WeatherKind),
    ToggleDayNight,
    ToggleRevealMap,
    SpawnCreepWave,
    ClearCreeps,
    RespawnAllTurrets,
    Teleport(DemoTeleportPreset),
}

const DEMO_TEST_ITEMS: &[(&str, &str)] = &[
    ("shadow_blade", "Shadow Blade"),
    ("quantum_phase_shield", "Phase Shield"),
    ("aegis_mirror_harness", "Mirror Harness"),
    ("mirage_phase_cloak", "Mirage Cloak"),
    ("ballistic_truelock", "True-Lock Core"),
    ("cryo_stasis_injector", "Cryo Injector"),
    ("valkyrie_crit_capacitor", "Crit Capacitor"),
    ("null_vector_disruptor", "Null Disruptor"),
    ("deep_scan_sonar", "Sonar Array"),
    ("vector_thruster_pack", "Thruster Pack"),
];

pub struct DemoHeroClientPlugin;

impl Plugin for DemoHeroClientPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingDemoLaunch>()
            .init_resource::<ActiveDemoSession>()
            .add_systems(
                Update,
                launch_embedded_demo_system.run_if(in_state(AppScreenState::PreGame)),
            )
            .add_systems(OnExit(AppScreenState::InGame), cleanup_demo_session_system)
            .add_systems(
                Update,
                (
                    demo_panel_hotkey_and_click_system,
                    rebuild_demo_panel_ui_system,
                )
                    .chain()
                    .run_if(in_state(AppScreenState::InGame)),
            );
    }
}

fn launch_embedded_demo_system(
    config: Res<GameConfig>,
    items_cfg: Res<ItemsConfig>,
    mut pending_demo: ResMut<PendingDemoLaunch>,
    mut pending_conn: ResMut<PendingMatchConnection>,
    mut session: ResMut<ActiveDemoSession>,
    mut next_screen: ResMut<NextState<AppScreenState>>,
) {
    let Some(hero_id) = pending_demo.0.take() else {
        return;
    };

    if let Some(mut old) = session.handle.take() {
        old.stop();
    }

    let handle = spawn_embedded_demo_server(config.clone(), items_cfg.clone(), hero_id.clone());
    let addr = handle.addr.clone();
    let port = handle.port;

    let own_idx = config
        .heroes
        .iter()
        .position(|h| h.id == hero_id)
        .unwrap_or(0);
    let dummy_idx = if config.heroes.len() > 1 {
        (own_idx + 1) % config.heroes.len()
    } else {
        0
    };
    let own_name = config.heroes[own_idx].display_name.clone();
    let dummy_name = config.heroes[dummy_idx].display_name.clone();

    *session = ActiveDemoSession {
        handle: Some(handle),
        is_active: true,
        panel_open: true,
        free_cooldowns: false,
        reveal_full_map: false,
        is_night: false,
        weather: WeatherKind::Clear,
        dummy_behavior: DummyBehavior::Idle,
        controlled_slot: 0,
        slots: vec![
            DemoSlotEntry {
                slot_index: 0,
                team: TeamId::Dawn,
                hero_name: own_name,
            },
            DemoSlotEntry {
                slot_index: 1,
                team: TeamId::Dusk,
                hero_name: dummy_name,
            },
        ],
        spawn_hero_idx: dummy_idx,
        dummy_item_idx: 0,
        ui_dirty: true,
    };

    pending_conn.0 = Some(ActiveMatchInfo {
        room_id: "demo_hero_local".to_string(),
        port,
        server_addr: addr,
        ticket_id: "DEMO_LOCAL".to_string(),
        player_count: 2,
        team_id: 1,
        slot_index: 0,
        participant_status: "connected".to_string(),
        game_link: String::new(),
    });

    next_screen.set(AppScreenState::InGame);
}

fn cleanup_demo_session_system(
    mut commands: Commands,
    mut session: ResMut<ActiveDemoSession>,
    panel_q: Query<Entity, With<DemoPanelRoot>>,
) {
    for ent in &panel_q {
        commands.entity(ent).despawn();
    }
    if let Some(mut h) = session.handle.take() {
        h.stop();
    }
    session.is_active = false;
    session.ui_dirty = true;
}

fn demo_panel_hotkey_and_click_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    config: Res<GameConfig>,
    mut session: ResMut<ActiveDemoSession>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut btn_q: Query<(&Hovered, &DemoBtnAction, &mut BorderColor), With<UiButton>>,
) {
    if !session.is_active {
        return;
    }

    if keys.just_pressed(KeyCode::F8) {
        session.panel_open = !session.panel_open;
        session.ui_dirty = true;
    }

    if keys.just_pressed(KeyCode::F7) && !session.slots.is_empty() {
        session.controlled_slot = (session.controlled_slot + 1) % session.slots.len();
        session.ui_dirty = true;
        outgoing
            .0
            .push(ClientLobbyMessage::DemoCommand(DemoCommand::CycleControlSlot));
    }

    let clicked = mouse.just_pressed(MouseButton::Left);
    let mut triggered: Option<DemoBtnAction> = None;
    for (hovered, action, mut border) in &mut btn_q {
        if hovered.get() {
            *border = BorderColor::all(Color::srgb(0.98, 0.84, 0.34));
            if clicked && triggered.is_none() {
                triggered = Some(action.clone());
            }
        } else {
            *border = BorderColor::all(Color::srgba(0.28, 0.72, 0.72, 0.65));
        }
    }

    let Some(act) = triggered else {
        return;
    };

    match act {
        DemoBtnAction::TogglePanel => {
            session.panel_open = !session.panel_open;
            session.ui_dirty = true;
        }
        DemoBtnAction::ControlSlot(slot_index) => {
            if slot_index < session.slots.len() {
                session.controlled_slot = slot_index;
                session.ui_dirty = true;
                outgoing.0.push(ClientLobbyMessage::DemoCommand(
                    DemoCommand::ControlSlot { slot_index },
                ));
            }
        }
        DemoBtnAction::CycleControlSlot => {
            if !session.slots.is_empty() {
                session.controlled_slot = (session.controlled_slot + 1) % session.slots.len();
                session.ui_dirty = true;
                outgoing
                    .0
                    .push(ClientLobbyMessage::DemoCommand(DemoCommand::CycleControlSlot));
            }
        }
        DemoBtnAction::NextSpawnHero => {
            if !config.heroes.is_empty() {
                session.spawn_hero_idx = (session.spawn_hero_idx + 1) % config.heroes.len();
                session.ui_dirty = true;
            }
        }
        DemoBtnAction::SpawnHero(team) => {
            if session.slots.len() < 10 && !config.heroes.is_empty() {
                let h = &config.heroes[session.spawn_hero_idx % config.heroes.len()];
                let new_slot = session.slots.len();
                session.slots.push(DemoSlotEntry {
                    slot_index: new_slot,
                    team,
                    hero_name: h.display_name.clone(),
                });
                session.ui_dirty = true;
                outgoing.0.push(ClientLobbyMessage::DemoCommand(
                    DemoCommand::SpawnHero {
                        team,
                        hero_id: h.id.clone(),
                    },
                ));
            }
        }
        DemoBtnAction::RemoveExtraHeroes => {
            session.controlled_slot = 0;
            if session.slots.len() > 2 {
                session.slots.truncate(2);
            }
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::RemoveExtraHeroes,
            ));
        }
        DemoBtnAction::ChangeControlledHero => {
            if !config.heroes.is_empty() {
                let h = &config.heroes[session.spawn_hero_idx % config.heroes.len()];
                let ctrl = session.controlled_slot;
                if let Some(entry) = session.slots.get_mut(ctrl) {
                    entry.hero_name = h.display_name.clone();
                }
                session.ui_dirty = true;
                outgoing.0.push(ClientLobbyMessage::DemoCommand(
                    DemoCommand::SwitchOwnHero {
                        hero_id: h.id.clone(),
                    },
                ));
            }
        }
        DemoBtnAction::LevelUp => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::LevelUp {
                    levels: 1,
                }));
        }
        DemoBtnAction::MaxLevel => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::MaxLevel));
        }
        DemoBtnAction::AddGold => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::AddGold {
                    amount: 10_000,
                }));
        }
        DemoBtnAction::RefreshHero => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::RefreshHero));
        }
        DemoBtnAction::ToggleFreeCooldowns => {
            session.free_cooldowns = !session.free_cooldowns;
            session.ui_dirty = true;
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SetFreeCooldowns(
                    session.free_cooldowns,
                )));
        }
        DemoBtnAction::RespawnDummiesNearPlayer => {
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::RespawnDummiesNearPlayer,
            ));
        }
        DemoBtnAction::CycleDummyBehavior => {
            session.dummy_behavior = match session.dummy_behavior {
                DummyBehavior::Idle => DummyBehavior::AttackPlayer,
                DummyBehavior::AttackPlayer => DummyBehavior::SwimInWater,
                DummyBehavior::SwimInWater => DummyBehavior::Idle,
            };
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SetDummyBehavior(session.dummy_behavior),
            ));
        }
        DemoBtnAction::NextDummyItem => {
            session.dummy_item_idx = (session.dummy_item_idx + 1) % DEMO_TEST_ITEMS.len();
            session.ui_dirty = true;
        }
        DemoBtnAction::GiveOtherHeroesSelectedItem => {
            let (item_id, _) = DEMO_TEST_ITEMS[session.dummy_item_idx % DEMO_TEST_ITEMS.len()];
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::GiveOtherHeroesItem {
                    item_id: item_id.to_string(),
                },
            ));
        }
        DemoBtnAction::ForceOtherHeroesUseSlot0 => {
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::ForceOtherHeroesActivateItem { inv_index: 0 },
            ));
        }
        DemoBtnAction::ClearOtherHeroesItems => {
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::ClearOtherHeroesItems,
            ));
        }
        DemoBtnAction::SetWeather(w) => {
            session.weather = w;
            session.ui_dirty = true;
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SetWeather(w)));
        }
        DemoBtnAction::ToggleDayNight => {
            session.is_night = !session.is_night;
            session.ui_dirty = true;
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SetNightTime(
                    session.is_night,
                )));
        }
        DemoBtnAction::ToggleRevealMap => {
            session.reveal_full_map = !session.reveal_full_map;
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SetRevealFullMap(session.reveal_full_map),
            ));
        }
        DemoBtnAction::SpawnCreepWave => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SpawnCreepWave));
        }
        DemoBtnAction::ClearCreeps => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::ClearCreeps));
        }
        DemoBtnAction::RespawnAllTurrets => {
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::RespawnAllTurrets,
            ));
        }
        DemoBtnAction::Teleport(preset) => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::TeleportSelf(
                    preset,
                )));
        }
    }
}

fn rebuild_demo_panel_ui_system(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut session: ResMut<ActiveDemoSession>,
    existing_q: Query<Entity, With<DemoPanelRoot>>,
) {
    if !session.is_active {
        return;
    }
    if !session.ui_dirty && !existing_q.is_empty() {
        return;
    }
    session.ui_dirty = false;

    for ent in &existing_q {
        commands.entity(ent).despawn();
    }

    let spawn_hero_name = config
        .heroes
        .get(session.spawn_hero_idx)
        .map(|h| h.display_name.as_str())
        .unwrap_or("Hero");
    let (_, dummy_item_label) = DEMO_TEST_ITEMS[session.dummy_item_idx % DEMO_TEST_ITEMS.len()];
    let ai_label = match session.dummy_behavior {
        DummyBehavior::Idle => "AI: IDLE",
        DummyBehavior::AttackPlayer => "AI: ATTACK ME",
        DummyBehavior::SwimInWater => "AI: SWIM WATER",
    };

    commands
        .spawn((
            DemoPanelRoot,
            GlobalZIndex(220),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(64.0),
                left: Val::Px(12.0),
                width: Val::Px(if session.panel_open { 260.0 } else { 148.0 }),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.07, 0.09, 0.92)),
            BorderColor::all(Color::srgb(0.26, 0.88, 0.78)),
        ))
        .with_children(|root| {
            spawn_demo_btn(
                root,
                if session.panel_open {
                    "DEMO CONTROLS [F8] -"
                } else {
                    "DEMO [F8] +"
                },
                DemoBtnAction::TogglePanel,
                Color::srgb(0.08, 0.30, 0.28),
                true,
            );

            if !session.panel_open {
                return;
            }

            // بخش ۱: انتخاب و کنترل مستقیم تک‌تک هیروهای حاضر در زمین
            spawn_section_label(
                root,
                &format!("CONTROL HERO [F7] ({}/10)", session.slots.len()),
            );
            spawn_demo_btn(
                root,
                "SWITCH CONTROL TO NEXT [F7]",
                DemoBtnAction::CycleControlSlot,
                Color::srgb(0.36, 0.26, 0.06),
                true,
            );
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|grid| {
                for entry in &session.slots {
                    let is_ctrl = entry.slot_index == session.controlled_slot;
                    let team_tag = if entry.team == TeamId::Dawn { "A" } else { "E" };
                    let short_name: String = entry.hero_name.chars().take(7).collect();
                    let lbl = format!("#{} {} {}", entry.slot_index, team_tag, short_name);
                    let bg = if is_ctrl {
                        Color::srgb(0.14, 0.48, 0.30)
                    } else if entry.team == TeamId::Dawn {
                        Color::srgb(0.06, 0.20, 0.28)
                    } else {
                        Color::srgb(0.30, 0.10, 0.12)
                    };
                    grid.spawn((
                        UiButton,
                        Hovered::default(),
                        Node {
                            width: Val::Px(118.0),
                            height: Val::Px(22.0),
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(if is_ctrl { 1.5 } else { 1.0 })),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(bg),
                        BorderColor::all(if is_ctrl {
                            Color::srgb(0.98, 0.86, 0.34)
                        } else {
                            Color::srgba(0.28, 0.72, 0.72, 0.65)
                        }),
                        DemoBtnAction::ControlSlot(entry.slot_index),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(lbl),
                            TextFont {
                                font_size: FontSize::Px(9.5),
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                }
            });

            // بخش ۲: اسپاون هیروهای جدید در هر دو تیم و تعویض هیروی فعلی
            spawn_section_label(root, "SPAWN & CHANGE HERO");
            spawn_demo_btn(
                root,
                &format!("SELECT: {}  >", spawn_hero_name),
                DemoBtnAction::NextSpawnHero,
                Color::srgb(0.08, 0.20, 0.28),
                true,
            );
            spawn_btn_row(
                root,
                ("+ SPAWN ALLY", DemoBtnAction::SpawnHero(TeamId::Dawn), Color::srgb(0.07, 0.32, 0.26)),
                ("+ SPAWN ENEMY", DemoBtnAction::SpawnHero(TeamId::Dusk), Color::srgb(0.36, 0.12, 0.14)),
            );
            spawn_btn_row(
                root,
                ("CHANGE MY HERO", DemoBtnAction::ChangeControlledHero, Color::srgb(0.14, 0.24, 0.34)),
                ("RESET 1v1", DemoBtnAction::RemoveExtraHeroes, Color::srgb(0.26, 0.12, 0.14)),
            );

            // بخش ۳: ارتقا و وضعیت هیروی تحت کنترل
            spawn_section_label(root, "ACTIVE HERO STATS");
            spawn_btn_row(
                root,
                ("+1 LVL", DemoBtnAction::LevelUp, Color::srgb(0.07, 0.22, 0.26)),
                ("MAX LVL", DemoBtnAction::MaxLevel, Color::srgb(0.07, 0.22, 0.26)),
            );
            spawn_btn_row(
                root,
                ("+10K GOLD", DemoBtnAction::AddGold, Color::srgb(0.36, 0.26, 0.06)),
                ("REFRESH", DemoBtnAction::RefreshHero, Color::srgb(0.08, 0.34, 0.22)),
            );
            spawn_demo_btn(
                root,
                if session.free_cooldowns {
                    "FREE COOLDOWNS: ON"
                } else {
                    "FREE COOLDOWNS: OFF"
                },
                DemoBtnAction::ToggleFreeCooldowns,
                if session.free_cooldowns {
                    Color::srgb(0.10, 0.44, 0.28)
                } else {
                    Color::srgb(0.06, 0.16, 0.20)
                },
                true,
            );

            // بخش ۴: کنترل رفتار و آیتم سایر هیروها
            spawn_section_label(root, "OTHER HEROES (AI & ITEMS)");
            spawn_btn_row(
                root,
                ("BRING HERE", DemoBtnAction::RespawnDummiesNearPlayer, Color::srgb(0.28, 0.14, 0.14)),
                (ai_label, DemoBtnAction::CycleDummyBehavior, Color::srgb(0.34, 0.18, 0.10)),
            );
            spawn_demo_btn(
                root,
                &format!("ITEM: {}  >", dummy_item_label),
                DemoBtnAction::NextDummyItem,
                Color::srgb(0.12, 0.16, 0.24),
                true,
            );
            spawn_btn_row(
                root,
                ("GIVE ALL", DemoBtnAction::GiveOtherHeroesSelectedItem, Color::srgb(0.10, 0.32, 0.26)),
                ("USE SLOT 1", DemoBtnAction::ForceOtherHeroesUseSlot0, Color::srgb(0.42, 0.22, 0.08)),
            );
            spawn_demo_btn(
                root,
                "CLEAR OTHER ITEMS",
                DemoBtnAction::ClearOtherHeroesItems,
                Color::srgb(0.24, 0.10, 0.12),
                true,
            );

            // بخش ۵: محیط، آب‌وهوا، شب‌وروز و نقشه
            spawn_section_label(root, "ENVIRONMENT & MAP");
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|row| {
                for (lbl, w) in [
                    ("CLEAR", WeatherKind::Clear),
                    ("RAIN", WeatherKind::Rainy),
                    ("SNOW", WeatherKind::Snowy),
                ] {
                    let active = session.weather == w;
                    spawn_demo_btn(
                        row,
                        lbl,
                        DemoBtnAction::SetWeather(w),
                        if active {
                            Color::srgb(0.12, 0.42, 0.38)
                        } else {
                            Color::srgb(0.05, 0.14, 0.18)
                        },
                        false,
                    );
                }
            });
            spawn_btn_row(
                root,
                (
                    if session.is_night { "TIME: NIGHT" } else { "TIME: DAY" },
                    DemoBtnAction::ToggleDayNight,
                    Color::srgb(0.10, 0.20, 0.34),
                ),
                (
                    if session.reveal_full_map { "FOW: OFF" } else { "FOW: ON" },
                    DemoBtnAction::ToggleRevealMap,
                    Color::srgb(0.16, 0.22, 0.30),
                ),
            );
            spawn_btn_row(
                root,
                ("SPAWN CREEPS", DemoBtnAction::SpawnCreepWave, Color::srgb(0.08, 0.26, 0.22)),
                ("CLEAR CREEPS", DemoBtnAction::ClearCreeps, Color::srgb(0.22, 0.14, 0.14)),
            );
            spawn_demo_btn(
                root,
                "RESPAWN ALL TURRETS",
                DemoBtnAction::RespawnAllTurrets,
                Color::srgb(0.12, 0.24, 0.28),
                true,
            );

            // بخش ۶: تلپورت سریع
            spawn_section_label(root, "QUICK TELEPORT");
            spawn_btn_row(
                root,
                ("MID LANE", DemoBtnAction::Teleport(DemoTeleportPreset::MidLane), Color::srgb(0.08, 0.20, 0.24)),
                ("WATER", DemoBtnAction::Teleport(DemoTeleportPreset::CanyonWater), Color::srgb(0.06, 0.26, 0.34)),
            );
            spawn_btn_row(
                root,
                ("ROSHAN PIT", DemoBtnAction::Teleport(DemoTeleportPreset::RoshanPit), Color::srgb(0.20, 0.14, 0.26)),
                ("ENEMY T1", DemoBtnAction::Teleport(DemoTeleportPreset::EnemyTower), Color::srgb(0.28, 0.14, 0.14)),
            );
        });
}

fn spawn_section_label(parent: &mut ChildSpawnerCommands, title: &str) {
    parent.spawn((
        Text::new(title),
        TextFont {
            font_size: FontSize::Px(9.5),
            ..default()
        },
        TextColor(Color::srgb(0.96, 0.84, 0.38)),
    ));
}

fn spawn_btn_row(
    parent: &mut ChildSpawnerCommands,
    left: (&str, DemoBtnAction, Color),
    right: (&str, DemoBtnAction, Color),
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            spawn_demo_btn(row, left.0, left.1, left.2, false);
            spawn_demo_btn(row, right.0, right.1, right.2, false);
        });
}

fn spawn_demo_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: DemoBtnAction,
    bg: Color,
    full_width: bool,
) {
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                width: if full_width {
                    Val::Percent(100.0)
                } else {
                    Val::Auto
                },
                flex_grow: if full_width { 0.0 } else { 1.0 },
                height: Val::Px(22.0),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgba(0.28, 0.72, 0.72, 0.65)),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(10.0),
                    ..default()
                },
                TextColor(Color::srgb(0.94, 0.98, 0.98)),
            ));
        });
}