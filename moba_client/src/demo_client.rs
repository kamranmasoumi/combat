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
    step_weather_and_environment_system, team_fountain_pos, try_activate_item_ability,
    try_activate_shadow_blade, try_buy_cart, try_buy_item, try_buy_recipe, try_cancel_teleport,
    try_courier_deliver, try_courier_move_to, try_courier_return_to_fountain, try_cycle_item_mode,
    try_move_courier_item_to_stash, try_move_hero_item_to_stash, try_move_stash_item_to_courier,
    try_move_stash_item_to_hero, try_sell_courier_item, try_sell_item, try_sell_stash_item,
    try_start_teleport, try_swap_courier_slots, try_swap_inventory_slots, try_swap_stash_slots,
    try_take_all_stash_to_hero, try_transfer_courier_item_to_hero,
    try_transfer_hero_item_to_courier, AuthoritativeMatchState, AuthoritativeWorldState,
    ClientInputCommand, ClientLobbyMessage, CustomPhysicsWorld, DemoCommand, DemoSharedFlags,
    DemoTeleportPreset, DummyBehavior, GameConfig, GameplayFxEvent, ItemsConfig, LobbyPlayerInfo,
    MatchPhase, MobaPhysicsSet, MobaSharedPhysicsPlugin, PhysicsRole, PhysicsSoldier,
    ServerLobbyPacket, ServerMatchMessage, TeamId, WeatherKind, CHANNEL_RELIABLE_FX,
    CHANNEL_RELIABLE_LOBBY, CHANNEL_UNRELIABLE_STATE, DEMO_INFINITE_GOLD,
};
use renet::{ClientId, RenetServer, ServerEvent};
use renet_netcode::NetcodeServerTransport;
use std::collections::{HashMap, HashSet};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

// =============================================================================
// سرور لوکال تعبیه‌شده Demo Hero (گلد بی‌نهایت، خرید از هرجا، آب‌وهوای فوری و حفظ دستورات)
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

fn demo_server_not_paused(demo: Res<DemoServerRuntime>) -> bool {
    !demo.flags.paused
}

/// اجرای تراکنش شاپ در حالت Demo Hero به طوری که خرید/فروش از هر نقطه نقشه مستقیماً به اینونتوری هیرو اعمال شود
fn with_demo_fountain_access<F>(
    world: &mut AuthoritativeWorldState,
    slot_index: usize,
    config: &GameConfig,
    f: F,
) where
    F: FnOnce(&mut AuthoritativeWorldState),
{
    let saved_pos = world.players.get(slot_index).map(|p| (p.soldier_pos, p.hero_team));
    if let Some((_, team)) = saved_pos {
        if let Some(fountain_pos) = team_fountain_pos(config, team) {
            if let Some(p) = world.players.get_mut(slot_index) {
                p.soldier_pos = fountain_pos;
            }
        }
    }
    f(world);
    if let Some((orig_pos, _)) = saved_pos {
        if let Some(p) = world.players.get_mut(slot_index) {
            p.soldier_pos = orig_pos;
        }
    }
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
    // اجازه تغییر فوری آب‌وهوا در Demo Hero از ثانیه صفر (بدون نیاز به انتظار تا دقیقه ۱۵)
    config.weather.weather_cycle_start_secs = 0.0;

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
        .configure_sets(
            FixedUpdate,
            (
                MobaPhysicsSet::Drive,
                MobaPhysicsSet::Simulate,
                MobaPhysicsSet::React,
            )
                .run_if(demo_server_not_paused),
        )
        .add_systems(
            FixedUpdate,
            (
                demo_server_init_and_receive_system.before(MobaPhysicsSet::Drive),
                demo_server_ai_tick_system
                    .before(MobaPhysicsSet::Drive)
                    .run_if(demo_server_not_paused),
                step_couriers_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system)
                    .run_if(demo_server_not_paused),
                step_teleports_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system)
                    .run_if(demo_server_not_paused),
                step_shadow_blade_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system)
                    .run_if(demo_server_not_paused),
                step_weather_and_environment_system
                    .after(MobaPhysicsSet::Drive)
                    .before(demo_server_send_snapshot_system)
                    .run_if(demo_server_not_paused),
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
    mut existing_soldiers: Query<(Entity, &mut PhysicsSoldier)>,
) {
    if stop_flag.0.load(Ordering::SeqCst) {
        app_exit.write(AppExit::Success);
        return;
    }

    if !demo.initialized {
        demo.initialized = true;
        let old_ents: Vec<Entity> = existing_soldiers.iter().map(|(e, _)| e).collect();
        world.init_multiplayer_players(&config, &match_state, &mut commands, &old_ents);
        // شروع دمو با هوای صاف و ثابت تا زمان تغییر دستی توسط بازیکن
        world.current_weather = WeatherKind::Clear;
        world.weather_remaining_secs = 9999.0;
        if world.players.len() >= 2 {
            world.players[0].hero_gold = DEMO_INFINITE_GOLD;
            world.players[1].hero_gold = DEMO_INFINITE_GOLD;
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
                    let is_remove_ctrl = matches!(cmd, DemoCommand::RemoveControlledHero);
                    let removed_slot = if is_remove_ctrl && ctrl > 0 {
                        Some(ctrl)
                    } else {
                        None
                    };
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
                    } else if let Some(rem) = removed_slot {
                        for (ent, mut ps) in &mut existing_soldiers {
                            if ps.slot_index == rem {
                                commands.entity(ent).despawn();
                            } else if ps.slot_index > rem {
                                ps.slot_index -= 1;
                            }
                        }
                    }
                }
                ClientLobbyMessage::BuyItem { item_id } => {
                    with_demo_fountain_access(&mut world, ctrl, &config, |w| {
                        try_buy_item(w, ctrl, &item_id, &config, &items_config);
                    });
                }
                ClientLobbyMessage::BuyRecipe { item_id } => {
                    with_demo_fountain_access(&mut world, ctrl, &config, |w| {
                        try_buy_recipe(w, ctrl, &item_id, &config, &items_config);
                    });
                }
                ClientLobbyMessage::BuyCart { item_ids } => {
                    with_demo_fountain_access(&mut world, ctrl, &config, |w| {
                        try_buy_cart(w, ctrl, &item_ids, &config, &items_config);
                    });
                }
                ClientLobbyMessage::SellItem { inv_index } => {
                    with_demo_fountain_access(&mut world, ctrl, &config, |w| {
                        try_sell_item(w, ctrl, inv_index, &config, &items_config);
                    });
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
                    with_demo_fountain_access(&mut world, ctrl, &config, |w| {
                        try_move_stash_item_to_hero(w, ctrl, stash_slot, &config, &items_config);
                    });
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
                    with_demo_fountain_access(&mut world, ctrl, &config, |w| {
                        try_take_all_stash_to_hero(w, ctrl, &config, &items_config);
                    });
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
            if !demo.flags.paused {
                world.set_slot_input(ctrl, cmd);
            }
        }
    }
}

fn demo_server_ai_tick_system(
    config: Res<GameConfig>,
    mut world: ResMut<AuthoritativeWorldState>,
    mut demo: ResMut<DemoServerRuntime>,
) {
    step_demo_dummy_ai_and_rules(&config, &mut world, &mut demo.flags);
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
// منابع و رابط کاربری مدرن Demo Hero (بزرگ، خوانا، وسط‌چین عمودی سمت چپ مشابه Dota 2)
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
    pub is_pointer_over_panel: bool,
    pub spawn_dropdown_open: bool,
    pub global_more_open: bool,
    pub free_cooldowns: bool,
    pub creeps_enabled: bool,
    pub towers_enabled: bool,
    pub reveal_full_map: bool,
    pub is_night: bool,
    pub paused: bool,
    pub weather: WeatherKind,
    pub dummy_behavior: DummyBehavior,
    pub controlled_slot: usize,
    pub slots: Vec<DemoSlotEntry>,
    pub invulnerable_slots: HashSet<usize>,
    pub slot_move_targets: HashMap<usize, Vec3>,
    pub pending_hero_move_target_sync: Option<Option<Vec3>>,
    pub spawn_hero_idx: usize,
    pub ui_dirty: bool,
}

impl Default for ActiveDemoSession {
    fn default() -> Self {
        Self {
            handle: None,
            is_active: false,
            panel_open: true,
            is_pointer_over_panel: false,
            spawn_dropdown_open: false,
            global_more_open: false,
            free_cooldowns: false,
            creeps_enabled: true,
            towers_enabled: true,
            reveal_full_map: false,
            is_night: false,
            paused: false,
            weather: WeatherKind::Clear,
            dummy_behavior: DummyBehavior::Idle,
            controlled_slot: 0,
            slots: Vec::new(),
            invulnerable_slots: HashSet::new(),
            slot_move_targets: HashMap::new(),
            pending_hero_move_target_sync: None,
            spawn_hero_idx: 0,
            ui_dirty: true,
        }
    }
}

impl ActiveDemoSession {
    /// انتقال کنترل به اسلات دلخواه با حفظ دستور حرکت هیروی قبلی و بازیابی دستور حرکت هیروی جدید
    pub fn switch_control_to_slot(
        &mut self,
        target_slot: usize,
        outgoing: &mut ShopOutgoingQueue,
    ) -> bool {
        if !self.is_active || target_slot >= self.slots.len() || target_slot == self.controlled_slot
        {
            return false;
        }
        self.controlled_slot = target_slot;
        self.pending_hero_move_target_sync =
            Some(self.slot_move_targets.get(&target_slot).copied());
        self.ui_dirty = true;
        outgoing
            .0
            .push(ClientLobbyMessage::DemoCommand(DemoCommand::ControlSlot {
                slot_index: target_slot,
            }));
        true
    }
}

#[derive(Component)]
struct DemoPanelRoot;

#[derive(Component)]
struct DemoPanelInteractiveArea;

#[derive(Component, Clone)]
enum DemoBtnAction {
    TogglePanel,
    ToggleSpawnDropdown,
    SelectSpawnHero(usize),
    ToggleGlobalMore,
    CycleControlSlot,
    SpawnHero(TeamId),
    SpawnDummy,
    ChangeControlledHero,
    LevelUp,
    MaxLevel,
    ToggleInvulnerable,
    RefreshHero,
    SwapControlledTeam,
    ResetControlledHero,
    RemoveControlledHero,
    ToggleFreeCooldowns,
    ToggleCreeps,
    ToggleTowers,
    ToggleDayNight,
    RefreshCooldownsOnly,
    RefreshHealthOnly,
    SetWeather(WeatherKind),
    ToggleRevealMap,
    RespawnDummiesNearPlayer,
    CycleDummyBehavior,
    SpawnCreepWave,
    RespawnAllTurrets,
    Teleport(DemoTeleportPreset),
    TogglePause,
    QuitDemo,
}

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
        is_pointer_over_panel: false,
        spawn_dropdown_open: false,
        global_more_open: false,
        free_cooldowns: false,
        creeps_enabled: true,
        towers_enabled: true,
        reveal_full_map: false,
        is_night: false,
        paused: false,
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
        invulnerable_slots: HashSet::new(),
        slot_move_targets: HashMap::new(),
        pending_hero_move_target_sync: None,
        spawn_hero_idx: own_idx,
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
    session.is_pointer_over_panel = false;
    session.slot_move_targets.clear();
    session.invulnerable_slots.clear();
    session.pending_hero_move_target_sync = None;
    session.ui_dirty = true;
}

#[allow(clippy::too_many_arguments)]
fn demo_panel_hotkey_and_click_system(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    config: Res<GameConfig>,
    mut session: ResMut<ActiveDemoSession>,
    mut outgoing: ResMut<ShopOutgoingQueue>,
    mut next_screen: ResMut<NextState<AppScreenState>>,
    panel_area_q: Query<&Hovered, (With<DemoPanelInteractiveArea>, Without<UiButton>)>,
    mut btn_q: Query<(&Hovered, &DemoBtnAction, &mut BorderColor), With<UiButton>>,
) {
    if !session.is_active {
        session.is_pointer_over_panel = false;
        return;
    }

    if keys.just_pressed(KeyCode::F8) {
        session.panel_open = !session.panel_open;
        session.ui_dirty = true;
    }

    if keys.just_pressed(KeyCode::F7) && !session.slots.is_empty() {
        let next_slot = (session.controlled_slot + 1) % session.slots.len();
        session.switch_control_to_slot(next_slot, &mut outgoing);
    }

    let clicked = mouse.just_pressed(MouseButton::Left);
    let mut triggered: Option<DemoBtnAction> = None;
    let mut any_btn_hovered = false;
    for (hovered, action, mut border) in &mut btn_q {
        if hovered.get() {
            any_btn_hovered = true;
            *border = BorderColor::all(Color::srgba(0.76, 0.84, 0.94, 0.92));
            if clicked && triggered.is_none() {
                triggered = Some(action.clone());
            }
        } else {
            *border = BorderColor::all(Color::srgba(0.28, 0.33, 0.40, 0.72));
        }
    }

    session.is_pointer_over_panel =
        any_btn_hovered || panel_area_q.iter().any(|h| h.get());

    let Some(act) = triggered else {
        return;
    };

    match act {
        DemoBtnAction::TogglePanel => {
            session.panel_open = !session.panel_open;
            session.ui_dirty = true;
        }
        DemoBtnAction::ToggleSpawnDropdown => {
            session.spawn_dropdown_open = !session.spawn_dropdown_open;
            session.ui_dirty = true;
        }
        DemoBtnAction::SelectSpawnHero(idx) => {
            if idx < config.heroes.len() {
                session.spawn_hero_idx = idx;
                session.spawn_dropdown_open = false;
                session.ui_dirty = true;
            }
        }
        DemoBtnAction::ToggleGlobalMore => {
            session.global_more_open = !session.global_more_open;
            session.ui_dirty = true;
        }
        DemoBtnAction::CycleControlSlot => {
            if !session.slots.is_empty() {
                let next_slot = (session.controlled_slot + 1) % session.slots.len();
                session.switch_control_to_slot(next_slot, &mut outgoing);
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
        DemoBtnAction::SpawnDummy => {
            if session.slots.len() < 10 && !config.heroes.is_empty() {
                let h = &config.heroes[session.spawn_hero_idx % config.heroes.len()];
                let ctrl_team = session
                    .slots
                    .get(session.controlled_slot)
                    .map(|s| s.team)
                    .unwrap_or(TeamId::Dawn);
                let enemy_team = if ctrl_team == TeamId::Dawn {
                    TeamId::Dusk
                } else {
                    TeamId::Dawn
                };
                let new_slot = session.slots.len();
                session.slots.push(DemoSlotEntry {
                    slot_index: new_slot,
                    team: enemy_team,
                    hero_name: h.display_name.clone(),
                });
                session.ui_dirty = true;
                outgoing.0.push(ClientLobbyMessage::DemoCommand(
                    DemoCommand::SpawnDummy {
                        hero_id: h.id.clone(),
                    },
                ));
            }
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
        DemoBtnAction::ToggleInvulnerable => {
            let ctrl = session.controlled_slot;
            if session.invulnerable_slots.contains(&ctrl) {
                session.invulnerable_slots.remove(&ctrl);
            } else {
                session.invulnerable_slots.insert(ctrl);
            }
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::ToggleInvulnerable,
            ));
        }
        DemoBtnAction::RefreshHero => {
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::RefreshHero));
        }
        DemoBtnAction::SwapControlledTeam => {
            let ctrl = session.controlled_slot;
            if let Some(entry) = session.slots.get_mut(ctrl) {
                entry.team = if entry.team == TeamId::Dawn {
                    TeamId::Dusk
                } else {
                    TeamId::Dawn
                };
            }
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SwapControlledTeam,
            ));
        }
        DemoBtnAction::ResetControlledHero => {
            let ctrl = session.controlled_slot;
            session.invulnerable_slots.remove(&ctrl);
            session.slot_move_targets.remove(&ctrl);
            session.pending_hero_move_target_sync = Some(None);
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::ResetControlledHero,
            ));
        }
        DemoBtnAction::RemoveControlledHero => {
            let ctrl = session.controlled_slot;
            if ctrl > 0 && ctrl < session.slots.len() {
                session.slots.remove(ctrl);
                for (i, s) in session.slots.iter_mut().enumerate() {
                    s.slot_index = i;
                }
                let mut new_inv = HashSet::new();
                for &s in &session.invulnerable_slots {
                    if s < ctrl {
                        new_inv.insert(s);
                    } else if s > ctrl {
                        new_inv.insert(s - 1);
                    }
                }
                session.invulnerable_slots = new_inv;
                let mut new_targets = HashMap::new();
                for (&s, &pos) in &session.slot_move_targets {
                    if s < ctrl {
                        new_targets.insert(s, pos);
                    } else if s > ctrl {
                        new_targets.insert(s - 1, pos);
                    }
                }
                session.slot_move_targets = new_targets;
                session.controlled_slot = 0;
                session.pending_hero_move_target_sync =
                    Some(session.slot_move_targets.get(&0).copied());
                session.ui_dirty = true;
                outgoing.0.push(ClientLobbyMessage::DemoCommand(
                    DemoCommand::RemoveControlledHero,
                ));
            } else if session.slots.len() > 2 {
                session.controlled_slot = 0;
                session.slots.truncate(2);
                session.invulnerable_slots.retain(|&s| s < 2);
                session.slot_move_targets.retain(|&s, _| s < 2);
                session.pending_hero_move_target_sync =
                    Some(session.slot_move_targets.get(&0).copied());
                session.ui_dirty = true;
                outgoing.0.push(ClientLobbyMessage::DemoCommand(
                    DemoCommand::RemoveExtraHeroes,
                ));
            }
        }
        DemoBtnAction::ToggleFreeCooldowns => {
            session.free_cooldowns = !session.free_cooldowns;
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SetFreeCooldowns(session.free_cooldowns),
            ));
        }
        DemoBtnAction::ToggleCreeps => {
            session.creeps_enabled = !session.creeps_enabled;
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SetCreepsEnabled(session.creeps_enabled),
            ));
        }
        DemoBtnAction::ToggleTowers => {
            session.towers_enabled = !session.towers_enabled;
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SetTowersEnabled(session.towers_enabled),
            ));
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
        DemoBtnAction::RefreshCooldownsOnly => {
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::RefreshCooldownsOnly,
            ));
        }
        DemoBtnAction::RefreshHealthOnly => {
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::RefreshHealthOnly,
            ));
        }
        DemoBtnAction::SetWeather(w) => {
            session.weather = w;
            session.ui_dirty = true;
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SetWeather(w)));
        }
        DemoBtnAction::ToggleRevealMap => {
            session.reveal_full_map = !session.reveal_full_map;
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::SetRevealFullMap(session.reveal_full_map),
            ));
        }
        DemoBtnAction::RespawnDummiesNearPlayer => {
            let ctrl = session.controlled_slot;
            session.slot_move_targets.retain(|&s, _| s == ctrl);
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
        DemoBtnAction::SpawnCreepWave => {
            session.creeps_enabled = true;
            session.ui_dirty = true;
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SpawnCreepWave));
        }
        DemoBtnAction::RespawnAllTurrets => {
            session.towers_enabled = true;
            session.ui_dirty = true;
            outgoing.0.push(ClientLobbyMessage::DemoCommand(
                DemoCommand::RespawnAllTurrets,
            ));
        }
        DemoBtnAction::Teleport(preset) => {
            let ctrl = session.controlled_slot;
            session.slot_move_targets.remove(&ctrl);
            session.pending_hero_move_target_sync = Some(None);
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::TeleportSelf(
                    preset,
                )));
        }
        DemoBtnAction::TogglePause => {
            session.paused = !session.paused;
            session.ui_dirty = true;
            outgoing
                .0
                .push(ClientLobbyMessage::DemoCommand(DemoCommand::SetPaused(
                    session.paused,
                )));
        }
        DemoBtnAction::QuitDemo => {
            next_screen.set(AppScreenState::PreGame);
        }
    }
}

// =============================================================================
// رنگ‌ها و المان‌های رابط کاربری بزرگ و خوانا مشابه Dota 2 Demo Hero
// =============================================================================

const COL_PANEL_BG: Color = Color::srgba(0.095, 0.112, 0.135, 0.95);
const COL_PANEL_BORDER: Color = Color::srgba(0.24, 0.28, 0.34, 0.92);
const COL_DIVIDER: Color = Color::srgba(0.24, 0.28, 0.34, 0.55);
const COL_BTN_SLATE: Color = Color::srgb(0.24, 0.28, 0.33);
const COL_BTN_DARK: Color = Color::srgb(0.14, 0.17, 0.21);
const COL_BTN_ACTIVE: Color = Color::srgb(0.16, 0.42, 0.52);
const COL_BTN_ALLY: Color = Color::srgb(0.22, 0.46, 0.24);
const COL_BTN_ENEMY: Color = Color::srgb(0.55, 0.17, 0.17);
const COL_BTN_QUIT: Color = Color::srgb(0.84, 0.30, 0.24);
const COL_BTN_PAUSE_ON: Color = Color::srgb(0.62, 0.44, 0.12);
const COL_TEXT_HEADER: Color = Color::srgb(0.95, 0.97, 0.99);
const COL_TEXT_SUB: Color = Color::srgb(0.78, 0.83, 0.89);

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
        .map(|h| h.display_name.to_uppercase())
        .unwrap_or_else(|| "HERO".to_string());
    let ai_short = match session.dummy_behavior {
        DummyBehavior::Idle => "AI: IDLE",
        DummyBehavior::AttackPlayer => "AI: ATK",
        DummyBehavior::SwimInWater => "AI: SWIM",
    };
    let ctrl_invuln = session.invulnerable_slots.contains(&session.controlled_slot);
    let ctrl_badge = session
        .slots
        .get(session.controlled_slot)
        .map(|s| {
            let tag = if s.team == TeamId::Dawn { "ALLY" } else { "ENEMY" };
            let short: String = s.hero_name.chars().take(9).collect();
            format!("P{} {} • {}", s.slot_index, tag, short)
        })
        .unwrap_or_else(|| "P0 ALLY".to_string());
    let ctrl_team_is_dawn = session
        .slots
        .get(session.controlled_slot)
        .map(|s| s.team == TeamId::Dawn)
        .unwrap_or(true);

    // کانتینر تمام‌ارتفاع سمت چپ (Pickable::IGNORE) برای وسط‌چین عمودی دقیق پنل
    commands
        .spawn((
            DemoPanelRoot,
            Pickable::IGNORE,
            GlobalZIndex(220),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|outer| {
            // بدنه اصلی پنل (در صورت باز بودن)
            if session.panel_open {
                outer
                    .spawn((
                        DemoPanelInteractiveArea,
                        Hovered::default(),
                        Node {
                            width: Val::Px(322.0),
                            padding: UiRect::all(Val::Px(14.0)),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(9.0),
                            border: UiRect::new(
                                Val::Px(0.0),
                                Val::Px(1.5),
                                Val::Px(1.5),
                                Val::Px(1.5),
                            ),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(COL_PANEL_BG),
                        BorderColor::all(COL_PANEL_BORDER),
                    ))
                    .with_children(|root| {
                        // =====================================================
                        // ۱. بخش SPAWN (مشابه Dota 2، بدون بخش اضافی دادن آیتم)
                        // =====================================================
                        root.spawn((
                            Text::new("SPAWN"),
                            TextFont {
                                font_size: FontSize::Px(14.5),
                                ..default()
                            },
                            TextColor(COL_TEXT_HEADER),
                        ));

                        // باکس انتخاب هیرو (Dropdown بزرگ و خوانا)
                        root.spawn((
                            UiButton,
                            Hovered::default(),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(40.0),
                                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.06, 0.08, 0.10)),
                            BorderColor::all(Color::srgba(0.28, 0.33, 0.40, 0.85)),
                            DemoBtnAction::ToggleSpawnDropdown,
                        ))
                        .with_children(|box_el| {
                            box_el.spawn((
                                Text::new(spawn_hero_name),
                                TextFont {
                                    font_size: FontSize::Px(14.5),
                                    ..default()
                                },
                                TextColor(COL_TEXT_HEADER),
                            ));
                            box_el.spawn((
                                Text::new(if session.spawn_dropdown_open { "^" } else { "v" }),
                                TextFont {
                                    font_size: FontSize::Px(13.5),
                                    ..default()
                                },
                                TextColor(COL_TEXT_SUB),
                            ));
                        });

                        // لیست بازشونده انتخاب سریع هیرو
                        if session.spawn_dropdown_open {
                            root.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(3.0),
                                    padding: UiRect::all(Val::Px(5.0)),
                                    border: UiRect::all(Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.05, 0.07, 0.09)),
                                BorderColor::all(COL_PANEL_BORDER),
                            ))
                            .with_children(|dd| {
                                for (idx, h) in config.heroes.iter().enumerate() {
                                    let is_sel = idx == session.spawn_hero_idx;
                                    spawn_dota_btn(
                                        dd,
                                        &h.display_name.to_uppercase(),
                                        DemoBtnAction::SelectSpawnHero(idx),
                                        if is_sel { COL_BTN_ACTIVE } else { COL_BTN_DARK },
                                        true,
                                        30.0,
                                        12.5,
                                    );
                                }
                            });
                        }

                        // دکمه تمام‌عرض CHANGE MY HERO
                        spawn_dota_btn(
                            root,
                            "CHANGE MY HERO",
                            DemoBtnAction::ChangeControlledHero,
                            COL_BTN_SLATE,
                            true,
                            36.0,
                            13.0,
                        );

                        // ردیف سه‌تایی +ALLY | +ENEMY | +DUMMY
                        spawn_row_3(
                            root,
                            ("+ALLY", DemoBtnAction::SpawnHero(TeamId::Dawn), COL_BTN_ALLY),
                            ("+ENEMY", DemoBtnAction::SpawnHero(TeamId::Dusk), COL_BTN_ENEMY),
                            ("+DUMMY", DemoBtnAction::SpawnDummy, COL_BTN_SLATE),
                            35.0,
                            12.5,
                        );

                        spawn_divider(root);

                        // =====================================================
                        // ۲. بخش UNITS (گرید ۲ + ۳ + ۳ مشابه Dota 2)
                        // =====================================================
                        root.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|hdr| {
                            hdr.spawn((
                                Text::new("UNITS"),
                                TextFont {
                                    font_size: FontSize::Px(14.5),
                                    ..default()
                                },
                                TextColor(COL_TEXT_HEADER),
                            ));
                            hdr.spawn((
                                UiButton,
                                Hovered::default(),
                                Node {
                                    height: Val::Px(24.0),
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(if ctrl_team_is_dawn {
                                    Color::srgba(0.14, 0.34, 0.20, 0.92)
                                } else {
                                    Color::srgba(0.42, 0.14, 0.16, 0.92)
                                }),
                                BorderColor::all(Color::srgba(0.40, 0.48, 0.56, 0.78)),
                                DemoBtnAction::CycleControlSlot,
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new(ctrl_badge),
                                    TextFont {
                                        font_size: FontSize::Px(11.0),
                                        ..default()
                                    },
                                    TextColor(COL_TEXT_HEADER),
                                ));
                            });
                        });

                        spawn_row_2(
                            root,
                            ("LVL UP", DemoBtnAction::LevelUp, COL_BTN_SLATE),
                            ("LVL MAX", DemoBtnAction::MaxLevel, COL_BTN_SLATE),
                            34.0,
                            12.5,
                        );
                        spawn_row_3(
                            root,
                            (
                                "INVULN",
                                DemoBtnAction::ToggleInvulnerable,
                                if ctrl_invuln { COL_BTN_ACTIVE } else { COL_BTN_SLATE },
                            ),
                            ("REFRESH", DemoBtnAction::RefreshHero, COL_BTN_SLATE),
                            (ai_short, DemoBtnAction::CycleDummyBehavior, COL_BTN_SLATE),
                            34.0,
                            12.0,
                        );
                        spawn_row_3(
                            root,
                            ("SWAP TEAM", DemoBtnAction::SwapControlledTeam, COL_BTN_SLATE),
                            ("RESET", DemoBtnAction::ResetControlledHero, COL_BTN_SLATE),
                            ("REMOVE", DemoBtnAction::RemoveControlledHero, COL_BTN_SLATE),
                            34.0,
                            11.5,
                        );

                        spawn_divider(root);

                        // =====================================================
                        // ۳. بخش GLOBAL (چک‌باکس‌های ۲×۲ + REFRESH + آب‌وهوا + MORE)
                        // =====================================================
                        spawn_section_header_with_more(
                            root,
                            "GLOBAL",
                            if session.global_more_open {
                                "LESS ^"
                            } else {
                                "MORE >"
                            },
                            DemoBtnAction::ToggleGlobalMore,
                        );

                        root.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_dota_checkbox(
                                row,
                                "Free Spells",
                                session.free_cooldowns,
                                DemoBtnAction::ToggleFreeCooldowns,
                            );
                            spawn_dota_checkbox(
                                row,
                                "Creeps",
                                session.creeps_enabled,
                                DemoBtnAction::ToggleCreeps,
                            );
                        });
                        root.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(8.0),
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_dota_checkbox(
                                row,
                                "Towers",
                                session.towers_enabled,
                                DemoBtnAction::ToggleTowers,
                            );
                            spawn_dota_checkbox(
                                row,
                                "Night Time",
                                session.is_night,
                                DemoBtnAction::ToggleDayNight,
                            );
                        });

                        // ردیف REFRESH: COOLDOWNS | HEALTH
                        root.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Text::new("REFRESH:"),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(COL_TEXT_SUB),
                            ));
                            spawn_dota_btn(
                                row,
                                "COOLDOWNS",
                                DemoBtnAction::RefreshCooldownsOnly,
                                COL_BTN_SLATE,
                                false,
                                32.0,
                                11.5,
                            );
                            spawn_dota_btn(
                                row,
                                "HEALTH",
                                DemoBtnAction::RefreshHealthOnly,
                                COL_BTN_SLATE,
                                false,
                                32.0,
                                11.5,
                            );
                        });

                        // ردیف اختصاصی تغییر فوری آب‌وهوا (CLEAR | RAIN | SNOW)
                        root.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(5.0),
                            ..default()
                        })
                        .with_children(|row| {
                            row.spawn((
                                Text::new("WEATHER:"),
                                TextFont {
                                    font_size: FontSize::Px(12.0),
                                    ..default()
                                },
                                TextColor(COL_TEXT_SUB),
                            ));
                            for (lbl, w) in [
                                ("CLEAR", WeatherKind::Clear),
                                ("RAIN", WeatherKind::Rainy),
                                ("SNOW", WeatherKind::Snowy),
                            ] {
                                let active = session.weather == w;
                                spawn_dota_btn(
                                    row,
                                    lbl,
                                    DemoBtnAction::SetWeather(w),
                                    if active { COL_BTN_ACTIVE } else { COL_BTN_DARK },
                                    false,
                                    32.0,
                                    11.5,
                                );
                            }
                        });

                        // کشوی اختیاری MORE برای تلپورت سریع، FoW، احضار هیروها و کنترل موج/تاور
                        if session.global_more_open {
                            root.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    padding: UiRect::all(Val::Px(8.0)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(6.0),
                                    border: UiRect::all(Val::Px(1.0)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.06, 0.08, 0.10, 0.90)),
                                BorderColor::all(COL_DIVIDER),
                            ))
                            .with_children(|more| {
                                spawn_dota_btn(
                                    more,
                                    "BRING OTHER HEROES HERE",
                                    DemoBtnAction::RespawnDummiesNearPlayer,
                                    COL_BTN_DARK,
                                    true,
                                    30.0,
                                    11.5,
                                );
                                spawn_row_2(
                                    more,
                                    (
                                        if session.reveal_full_map {
                                            "FOW: REVEALED"
                                        } else {
                                            "FOW: NORMAL"
                                        },
                                        DemoBtnAction::ToggleRevealMap,
                                        if session.reveal_full_map {
                                            COL_BTN_ACTIVE
                                        } else {
                                            COL_BTN_DARK
                                        },
                                    ),
                                    ("SPAWN WAVE", DemoBtnAction::SpawnCreepWave, COL_BTN_DARK),
                                    30.0,
                                    11.5,
                                );
                                spawn_dota_btn(
                                    more,
                                    "RESPAWN ALL TURRETS",
                                    DemoBtnAction::RespawnAllTurrets,
                                    COL_BTN_DARK,
                                    true,
                                    30.0,
                                    11.5,
                                );
                                spawn_row_2(
                                    more,
                                    ("TP: MID", DemoBtnAction::Teleport(DemoTeleportPreset::MidLane), COL_BTN_DARK),
                                    ("TP: WATER", DemoBtnAction::Teleport(DemoTeleportPreset::CanyonWater), COL_BTN_DARK),
                                    30.0,
                                    11.5,
                                );
                                spawn_row_2(
                                    more,
                                    ("TP: ROSHAN", DemoBtnAction::Teleport(DemoTeleportPreset::RoshanPit), COL_BTN_DARK),
                                    ("TP: ENEMY T1", DemoBtnAction::Teleport(DemoTeleportPreset::EnemyTower), COL_BTN_DARK),
                                    30.0,
                                    11.5,
                                );
                            });
                        }

                        spawn_divider(root);

                        // =====================================================
                        // ۴. نوار پایینی PAUSE | QUIT (مشابه Dota 2)
                        // =====================================================
                        spawn_row_2(
                            root,
                            (
                                if session.paused { "RESUME" } else { "PAUSE" },
                                DemoBtnAction::TogglePause,
                                if session.paused {
                                    COL_BTN_PAUSE_ON
                                } else {
                                    COL_BTN_SLATE
                                },
                            ),
                            ("QUIT", DemoBtnAction::QuitDemo, COL_BTN_QUIT),
                            38.0,
                            13.5,
                        );
                    });
            }

            // زبانه جمع‌شونده سمت راست پنل (< / >)
            outer
                .spawn((
                    UiButton,
                    Hovered::default(),
                    Node {
                        width: Val::Px(28.0),
                        height: Val::Px(104.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::new(
                            Val::Px(0.0),
                            Val::Px(1.5),
                            Val::Px(1.5),
                            Val::Px(1.5),
                        ),
                        border_radius: BorderRadius::all(Val::Px(5.0)),
                        ..default()
                    },
                    BackgroundColor(COL_PANEL_BG),
                    BorderColor::all(COL_PANEL_BORDER),
                    DemoBtnAction::TogglePanel,
                ))
                .with_children(|tab| {
                    tab.spawn((
                        Text::new(if session.panel_open { "<" } else { ">" }),
                        TextFont {
                            font_size: FontSize::Px(18.0),
                            ..default()
                        },
                        TextColor(COL_TEXT_HEADER),
                    ));
                });
        });
}

fn spawn_divider(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(1.0),
            margin: UiRect::vertical(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(COL_DIVIDER),
    ));
}

fn spawn_section_header_with_more(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    more_label: &str,
    more_action: DemoBtnAction,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(title),
                TextFont {
                    font_size: FontSize::Px(14.5),
                    ..default()
                },
                TextColor(COL_TEXT_HEADER),
            ));
            row.spawn((
                UiButton,
                Hovered::default(),
                Node {
                    height: Val::Px(25.0),
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(COL_BTN_DARK),
                BorderColor::all(Color::srgba(0.28, 0.33, 0.40, 0.72)),
                more_action,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(more_label),
                    TextFont {
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(COL_TEXT_SUB),
                ));
            });
        });
}

fn spawn_dota_checkbox(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    checked: bool,
    action: DemoBtnAction,
) {
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                height: Val::Px(28.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            BorderColor::all(Color::srgba(0.28, 0.33, 0.40, 0.0)),
            action,
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(18.0),
                    height: Val::Px(18.0),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(if checked {
                    Color::srgb(0.76, 0.93, 0.98)
                } else {
                    Color::srgb(0.05, 0.06, 0.08)
                }),
                BorderColor::all(if checked {
                    Color::srgb(0.45, 0.62, 0.92)
                } else {
                    Color::srgb(0.32, 0.36, 0.42)
                }),
            ));
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(13.5),
                    ..default()
                },
                TextColor(COL_TEXT_SUB),
            ));
        });
}

fn spawn_row_2(
    parent: &mut ChildSpawnerCommands,
    left: (&str, DemoBtnAction, Color),
    right: (&str, DemoBtnAction, Color),
    height: f32,
    font_px: f32,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            spawn_dota_btn(row, left.0, left.1, left.2, false, height, font_px);
            spawn_dota_btn(row, right.0, right.1, right.2, false, height, font_px);
        });
}

fn spawn_row_3(
    parent: &mut ChildSpawnerCommands,
    a: (&str, DemoBtnAction, Color),
    b: (&str, DemoBtnAction, Color),
    c: (&str, DemoBtnAction, Color),
    height: f32,
    font_px: f32,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            spawn_dota_btn(row, a.0, a.1, a.2, false, height, font_px);
            spawn_dota_btn(row, b.0, b.1, b.2, false, height, font_px);
            spawn_dota_btn(row, c.0, c.1, c.2, false, height, font_px);
        });
}

fn spawn_dota_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: DemoBtnAction,
    bg: Color,
    full_width: bool,
    height: f32,
    font_px: f32,
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
                flex_basis: if full_width { Val::Auto } else { Val::Px(0.0) },
                height: Val::Px(height),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgba(0.28, 0.33, 0.40, 0.72)),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(font_px),
                    ..default()
                },
                TextColor(COL_TEXT_HEADER),
            ));
        });
}