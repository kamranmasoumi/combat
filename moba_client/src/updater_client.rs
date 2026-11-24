use crate::pregame_ui::{AppScreenState, PreGameSessionState};
use bevy::app::AppExit;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Button as UiButton;
use moba_shared::GameConfig;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tungstenite::{connect, Message};
use ureq::serde::{Deserialize, Serialize};

// =============================================================================
// سیستم جامع Game Coordinator + آپدیت خودکار تفاضلی کلاینت (مشابه Dota 2)
// - اتصال وب‌سوکت زنده /ws/coordinator/ (هم در منو و هم بی‌صدا در حین بازی)
// - بدون هیچ‌گونه اختلال در نبردهای رنک فعال (InGame یا دارای ActiveMatch برای Reconnect)
// - دانلود تفاضلی (Delta) با بررسی SHA-256، نوار پیشرفت ۴ مرحله‌ای، سرعت MB/s و ETA
// - جایگزینی اتمیک (Staging -> Rename running exe -> Spawn new exe -> Exit)
// =============================================================================

pub const COMPILED_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMPILED_CLIENT_BUILD: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
struct LocalBuildMarker {
    version: String,
    build_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PatchManifestFile {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct PatchInfoDto {
    pub version: String,
    pub build_number: u32,
    pub min_supported_build: u32,
    pub title: String,
    #[serde(default)]
    pub release_notes: String,
    pub base_download_url: String,
    #[serde(default)]
    pub total_size_bytes: u64,
    #[serde(default)]
    pub file_count: usize,
    #[serde(default = "default_true")]
    pub is_mandatory: bool,
    #[serde(default)]
    pub files: Vec<PatchManifestFile>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "ureq::serde")]
pub struct CoordinatorPayloadDto {
    #[serde(default)]
    pub event: String,
    #[serde(default = "default_online")]
    pub mode: String,
    #[serde(default)]
    pub block_matchmaking: bool,
    #[serde(default)]
    pub banner_message: String,
    #[serde(default)]
    pub latest_patch: Option<PatchInfoDto>,
}

fn default_online() -> String {
    "online".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStage {
    Idle,
    FetchingManifest,
    ScanningLocalFiles,
    DownloadingDelta,
    ApplyingPatch,
    CompletedRestarting,
    CompletedNoRestart,
    Failed,
}

#[derive(Debug, Clone)]
enum UpdaterWorkerEvent {
    CoordinatorSync(CoordinatorPayloadDto),
    StageChanged {
        stage: UpdateStage,
        headline: String,
        detail: String,
    },
    ScanProgress {
        checked: usize,
        total: usize,
        current_file: String,
    },
    DownloadPlan {
        files_to_download: usize,
        total_manifest_files: usize,
        bytes_to_download: u64,
    },
    DownloadProgress {
        downloaded_bytes: u64,
        total_bytes: u64,
        completed_files: usize,
        total_files: usize,
        current_file: String,
        speed_mbps: f32,
        eta_secs: u32,
    },
    Finished {
        new_version: String,
        new_build: u32,
        needs_restart: bool,
    },
    Error(String),
}

#[derive(Resource)]
pub struct ClientUpdaterState {
    pub local_version: String,
    pub local_build: u32,
    pub coordinator_mode: String,
    pub block_matchmaking: bool,
    pub banner_message: String,
    pub latest_patch: Option<PatchInfoDto>,
    pub update_dismissed_for_build: Option<u32>,
    pub stage: UpdateStage,
    pub stage_headline: String,
    pub stage_detail: String,
    pub current_file: String,
    pub files_done: usize,
    pub files_total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub speed_mbps: f32,
    pub eta_secs: u32,
    pub error_message: Option<String>,
    pub deferred_in_game_notice: bool,
    pub is_pointer_over_overlay: bool,
    pub ui_dirty: bool,
    pub last_ui_refresh_secs: f32,
    rx: Mutex<Receiver<UpdaterWorkerEvent>>,
    tx: Sender<UpdaterWorkerEvent>,
    http_base_url: String,
    ws_base_url: String,
    ws_started: bool,
    worker_busy: Arc<Mutex<bool>>,
}

impl ClientUpdaterState {
    pub fn new(http_base_url: String, ws_base_url: String) -> Self {
        let (local_version, local_build) = read_local_build_marker();
        let (tx, rx) = mpsc::channel();
        Self {
            local_version,
            local_build,
            coordinator_mode: "online".to_string(),
            block_matchmaking: false,
            banner_message: String::new(),
            latest_patch: None,
            update_dismissed_for_build: None,
            stage: UpdateStage::Idle,
            stage_headline: String::new(),
            stage_detail: String::new(),
            current_file: String::new(),
            files_done: 0,
            files_total: 0,
            bytes_done: 0,
            bytes_total: 0,
            speed_mbps: 0.0,
            eta_secs: 0,
            error_message: None,
            deferred_in_game_notice: false,
            is_pointer_over_overlay: false,
            ui_dirty: true,
            last_ui_refresh_secs: 0.0,
            rx: Mutex::new(rx),
            tx,
            http_base_url: http_base_url.trim_end_matches('/').to_string(),
            ws_base_url: ws_base_url.trim_end_matches('/').to_string(),
            ws_started: false,
            worker_busy: Arc::new(Mutex::new(false)),
        }
    }

    pub fn has_newer_patch(&self) -> bool {
        self.latest_patch
            .as_ref()
            .is_some_and(|p| p.build_number > self.local_build)
    }

    pub fn is_mandatory_update_pending(&self) -> bool {
        self.latest_patch.as_ref().is_some_and(|p| {
            p.build_number > self.local_build
                && (p.is_mandatory || self.local_build < p.min_supported_build)
        })
    }

    pub fn should_block_new_matchmaking(&self) -> bool {
        self.block_matchmaking
            || self.coordinator_mode != "online"
            || self.is_mandatory_update_pending()
            || self.is_updating_active()
    }

    pub fn is_updating_active(&self) -> bool {
        matches!(
            self.stage,
            UpdateStage::FetchingManifest
                | UpdateStage::ScanningLocalFiles
                | UpdateStage::DownloadingDelta
                | UpdateStage::ApplyingPatch
                | UpdateStage::CompletedRestarting
        )
    }

    pub fn start_update_download(&mut self) {
        let Some(patch) = self.latest_patch.clone() else {
            return;
        };
        {
            let mut busy = self.worker_busy.lock().unwrap();
            if *busy {
                return;
            }
            *busy = true;
        }
        self.stage = UpdateStage::FetchingManifest;
        self.stage_headline = format!("PREPARING PATCH v{}", patch.version);
        self.stage_detail = "Connecting to patch server & downloading file manifest...".to_string();
        self.error_message = None;
        self.files_done = 0;
        self.files_total = patch.file_count;
        self.bytes_done = 0;
        self.bytes_total = patch.total_size_bytes;
        self.speed_mbps = 0.0;
        self.eta_secs = 0;
        self.ui_dirty = true;

        let tx = self.tx.clone();
        let http_base = self.http_base_url.clone();
        let busy_flag = Arc::clone(&self.worker_busy);
        thread::spawn(move || {
            let result = run_delta_patch_worker(&http_base, &patch, &tx);
            if let Err(err) = result {
                let _ = tx.send(UpdaterWorkerEvent::Error(err));
            }
            if let Ok(mut b) = busy_flag.lock() {
                *b = false;
            }
        });
    }
}

pub struct ClientUpdaterPlugin;

impl Plugin for ClientUpdaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_client_updater_system)
            .add_systems(
                Update,
                (
                    poll_updater_events_system,
                    updater_overlay_interaction_system,
                    rebuild_updater_ui_system,
                    animate_coordinator_banner_system,
                )
                    .chain(),
            );
    }
}

#[derive(Component)]
struct CoordinatorBannerRoot;

#[derive(Component)]
struct CoordinatorPulseDot;

#[derive(Component)]
struct CoordinatorScannerFill;

#[derive(Component)]
struct CoordinatorLoadingText;

#[derive(Component)]
struct VersionWatermarkRoot;

#[derive(Component)]
struct UpdaterModalRoot;

#[derive(Component)]
struct UpdaterInteractiveArea;

#[derive(Component, Clone)]
enum UpdaterBtnAction {
    StartUpdate,
    DismissOptionalUpdate,
    RetryUpdate,
    DismissCompleted,
}

fn init_client_updater_system(mut commands: Commands, config: Res<GameConfig>) {
    // پاک‌سازی فایل موقت نسخه قبلی (moba_client.exe.old) و پوشه .update_staging پس از ری‌استارت موفق
    cleanup_previous_update_artifacts();

    let mut state = ClientUpdaterState::new(
        config.network.backend_http_url.clone(),
        config.network.backend_ws_url.clone(),
    );
    state.ws_started = true;

    let tx_http = state.tx.clone();
    let http_url = format!("{}/api/v1/client/patch/latest/", state.http_base_url);
    thread::spawn(move || {
        if let Ok(resp) = ureq::get(&http_url).timeout(Duration::from_secs(6)).call() {
            if let Ok(dto) = resp.into_json::<CoordinatorPayloadDto>() {
                let _ = tx_http.send(UpdaterWorkerEvent::CoordinatorSync(dto));
            }
        }
    });

    // وب‌سوکت پایدار /ws/coordinator/ با اتصال مجدد خودکار (زنده در منو و بی‌صدا در حین نبرد)
    let tx_ws = state.tx.clone();
    let ws_url = format!("{}/ws/coordinator/", state.ws_base_url);
    thread::spawn(move || loop {
        match connect(ws_url.as_str()) {
            Ok((mut socket, _)) => {
                let mut last_ping = Instant::now();
                loop {
                    if last_ping.elapsed() >= Duration::from_secs(25) {
                        let _ = socket.send(Message::Text(r#"{"action":"ping"}"#.into()));
                        last_ping = Instant::now();
                    }
                    match socket.read() {
                        Ok(Message::Text(txt)) => {
                            if let Ok(dto) = ureq::serde_json::from_str::<CoordinatorPayloadDto>(&txt)
                            {
                                if dto.event != "pong" {
                                    let _ = tx_ws.send(UpdaterWorkerEvent::CoordinatorSync(dto));
                                }
                            }
                        }
                        Ok(Message::Ping(p)) => {
                            let _ = socket.send(Message::Pong(p));
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }
            }
            Err(_) => {}
        }
        thread::sleep(Duration::from_secs(5));
    });

    commands.insert_resource(state);
}

fn poll_updater_events_system(
    time: Res<Time>,
    screen: Res<State<AppScreenState>>,
    mut updater: ResMut<ClientUpdaterState>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let in_game = *screen.get() == AppScreenState::InGame;
    let events: Vec<UpdaterWorkerEvent> = {
        let Ok(rx) = updater.rx.lock() else {
            return;
        };
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    };

    if events.is_empty() {
        // اگر در حال بازگشت از InGame به PreGame هستیم و آپدیت معوق داشتیم، UI را تازه کن
        if !in_game && updater.deferred_in_game_notice {
            updater.deferred_in_game_notice = false;
            updater.ui_dirty = true;
        }
        return;
    }

    let now_s = time.elapsed_secs();
    for ev in events {
        match ev {
            UpdaterWorkerEvent::CoordinatorSync(dto) => {
                updater.coordinator_mode = dto.mode;
                updater.block_matchmaking = dto.block_matchmaking;
                updater.banner_message = dto.banner_message;
                if let Some(patch) = dto.latest_patch {
                    let is_new = patch.build_number > updater.local_build;
                    updater.latest_patch = Some(patch);
                    if is_new && in_game {
                        // در میانه نبرد رنک هیچ‌گونه پاپ‌آپ یا دانلودی اجرا نمی‌شود
                        updater.deferred_in_game_notice = true;
                    }
                }
                updater.ui_dirty = true;
            }
            UpdaterWorkerEvent::StageChanged {
                stage,
                headline,
                detail,
            } => {
                updater.stage = stage;
                updater.stage_headline = headline;
                updater.stage_detail = detail;
                updater.ui_dirty = true;
            }
            UpdaterWorkerEvent::ScanProgress {
                checked,
                total,
                current_file,
            } => {
                updater.stage = UpdateStage::ScanningLocalFiles;
                updater.files_done = checked;
                updater.files_total = total;
                updater.current_file = current_file;
                if now_s - updater.last_ui_refresh_secs >= 0.08 {
                    updater.last_ui_refresh_secs = now_s;
                    updater.ui_dirty = true;
                }
            }
            UpdaterWorkerEvent::DownloadPlan {
                files_to_download,
                total_manifest_files,
                bytes_to_download,
            } => {
                updater.stage = UpdateStage::DownloadingDelta;
                updater.files_done = 0;
                updater.files_total = files_to_download;
                updater.bytes_done = 0;
                updater.bytes_total = bytes_to_download;
                updater.stage_headline = "DOWNLOADING DELTA PATCH".to_string();
                updater.stage_detail = format!(
                    "Only {} of {} files changed ({:.2} MB to download)",
                    files_to_download,
                    total_manifest_files,
                    bytes_to_download as f64 / (1024.0 * 1024.0)
                );
                updater.ui_dirty = true;
            }
            UpdaterWorkerEvent::DownloadProgress {
                downloaded_bytes,
                total_bytes,
                completed_files,
                total_files,
                current_file,
                speed_mbps,
                eta_secs,
            } => {
                updater.stage = UpdateStage::DownloadingDelta;
                updater.bytes_done = downloaded_bytes;
                updater.bytes_total = total_bytes;
                updater.files_done = completed_files;
                updater.files_total = total_files;
                updater.current_file = current_file;
                updater.speed_mbps = speed_mbps;
                updater.eta_secs = eta_secs;
                if now_s - updater.last_ui_refresh_secs >= 0.08 {
                    updater.last_ui_refresh_secs = now_s;
                    updater.ui_dirty = true;
                }
            }
            UpdaterWorkerEvent::Finished {
                new_version,
                new_build,
                needs_restart,
            } => {
                updater.local_version = new_version.clone();
                updater.local_build = new_build;
                updater.error_message = None;
                updater.ui_dirty = true;
                if needs_restart {
                    updater.stage = UpdateStage::CompletedRestarting;
                    updater.stage_headline = format!("UPDATED TO v{} — RESTARTING...", new_version);
                    updater.stage_detail =
                        "Launching updated client binary cleanly...".to_string();
                    app_exit.write(AppExit::Success);
                } else {
                    updater.stage = UpdateStage::CompletedNoRestart;
                    updater.stage_headline = format!("PATCH v{} INSTALLED!", new_version);
                    updater.stage_detail =
                        "All assets verified and updated. Ready to play!".to_string();
                }
            }
            UpdaterWorkerEvent::Error(msg) => {
                updater.stage = UpdateStage::Failed;
                updater.error_message = Some(msg.clone());
                updater.stage_headline = "UPDATE FAILED".to_string();
                updater.stage_detail = msg;
                updater.ui_dirty = true;
            }
        }
    }
}

fn updater_overlay_interaction_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut updater: ResMut<ClientUpdaterState>,
    area_q: Query<&Hovered, (With<UpdaterInteractiveArea>, Without<UiButton>)>,
    mut btn_q: Query<(&Hovered, &UpdaterBtnAction, &mut BorderColor), With<UiButton>>,
) {
    let clicked = mouse.just_pressed(MouseButton::Left);
    let mut triggered: Option<UpdaterBtnAction> = None;
    let mut any_btn_hovered = false;

    for (hovered, action, mut border) in &mut btn_q {
        if hovered.get() {
            any_btn_hovered = true;
            *border = BorderColor::all(Color::srgb(0.96, 0.84, 0.42));
            if clicked && triggered.is_none() {
                triggered = Some(action.clone());
            }
        } else {
            *border = BorderColor::all(Color::srgba(0.34, 0.44, 0.56, 0.85));
        }
    }

    updater.is_pointer_over_overlay = any_btn_hovered || area_q.iter().any(|h| h.get());

    let Some(act) = triggered else {
        return;
    };

    match act {
        UpdaterBtnAction::StartUpdate | UpdaterBtnAction::RetryUpdate => {
            updater.start_update_download();
        }
        UpdaterBtnAction::DismissOptionalUpdate => {
            if let Some(p) = &updater.latest_patch {
                updater.update_dismissed_for_build = Some(p.build_number);
            }
            updater.ui_dirty = true;
        }
        UpdaterBtnAction::DismissCompleted => {
            updater.stage = UpdateStage::Idle;
            updater.ui_dirty = true;
        }
    }
}

fn animate_coordinator_banner_system(
    time: Res<Time>,
    mut dot_q: Query<&mut BackgroundColor, With<CoordinatorPulseDot>>,
    mut scan_q: Query<&mut Node, With<CoordinatorScannerFill>>,
    mut txt_q: Query<&mut Text, With<CoordinatorLoadingText>>,
) {
    let t = time.elapsed_secs();
    let pulse = 0.45 + 0.55 * (t * 4.5).sin().abs();
    for mut bg in &mut dot_q {
        bg.0 = Color::srgba(1.0, 0.36 + 0.45 * pulse, 0.34 + 0.40 * pulse, 0.55 + 0.45 * pulse);
    }
    // حرکت رفت‌وبرگشتی نرم نوار اسکنر لودینگ مفهومی (بین ۰ تا ۷۲ درصد عرض)
    let sweep = 0.5 - 0.5 * (t * 2.4).cos();
    for mut node in &mut scan_q {
        node.left = Val::Percent(sweep * 72.0);
    }
    let dots = match ((t * 2.2) as usize) % 4 {
        0 => "SYNCING WITH GAME COORDINATOR",
        1 => "SYNCING WITH GAME COORDINATOR.",
        2 => "SYNCING WITH GAME COORDINATOR..",
        _ => "SYNCING WITH GAME COORDINATOR...",
    };
    for mut txt in &mut txt_q {
        if txt.0 != dots {
            txt.0 = dots.to_string();
        }
    }
}

fn rebuild_updater_ui_system(
    mut commands: Commands,
    screen: Res<State<AppScreenState>>,
    pregame: Option<Res<PreGameSessionState>>,
    mut updater: ResMut<ClientUpdaterState>,
    banner_q: Query<Entity, With<CoordinatorBannerRoot>>,
    watermark_q: Query<Entity, With<VersionWatermarkRoot>>,
    modal_q: Query<Entity, With<UpdaterModalRoot>>,
) {
    let in_game = *screen.get() == AppScreenState::InGame;
    let has_active_ranked_match = pregame
        .as_ref()
        .is_some_and(|pg| pg.active_match.is_some());

    if in_game {
        // در میانه نبرد هیچ‌گونه مودال مسدودکننده‌ای نمایش داده نمی‌شود
        for e in &modal_q {
            commands.entity(e).despawn();
        }
        for e in &banner_q {
            commands.entity(e).despawn();
        }
        for e in &watermark_q {
            commands.entity(e).despawn();
        }
        return;
    }

    if !updater.ui_dirty && !watermark_q.is_empty() {
        return;
    }
    updater.ui_dirty = false;

    for e in &banner_q {
        commands.entity(e).despawn();
    }
    for e in &watermark_q {
        commands.entity(e).despawn();
    }
    for e in &modal_q {
        commands.entity(e).despawn();
    }

    // ۰. نمایش ظریف نسخه فعلی اپلیکیشن و شماره پچ در گوشه پایین سمت چپ
    let mut ver_label = format!("v{}  •  Patch #{}", updater.local_version, updater.local_build);
    if updater.coordinator_mode != "online" {
        ver_label.push_str("  •  COORDINATOR UPDATING");
    } else if let Some(ref p) = updater.latest_patch {
        if p.build_number > updater.local_build {
            ver_label.push_str(&format!("  •  Update v{} (#{}) ready", p.version, p.build_number));
        }
    }
    commands
        .spawn((
            VersionWatermarkRoot,
            Pickable::IGNORE,
            GlobalZIndex(250),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Px(14.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.74)),
            BorderColor::all(Color::srgba(0.28, 0.34, 0.42, 0.55)),
        ))
        .with_children(|wm| {
            wm.spawn((
                Text::new(ver_label),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(if updater.coordinator_mode != "online" {
                    Color::srgb(0.98, 0.52, 0.48)
                } else {
                    Color::srgba(0.76, 0.82, 0.88, 0.88)
                }),
            ));
        });

    // ۱. بنر قرمز چشم‌گیر Game Coordinator با لودینگ مفهومی انیمیشنی (مشابه Dota 2)
    let show_coord_banner = updater.coordinator_mode != "online"
        || updater.block_matchmaking
        || (updater.has_newer_patch() && has_active_ranked_match);
    if show_coord_banner {
        let is_coord_red = updater.coordinator_mode != "online" || updater.block_matchmaking;
        let (bg_col, border_col, badge_bg, badge_txt, headline_msg, show_loader) = if is_coord_red {
            let badge = if updater.coordinator_mode == "maintenance" {
                "MAINTENANCE"
            } else {
                "COORDINATOR UPDATING"
            };
            let msg = if updater.banner_message.is_empty() {
                "Game Coordinator is currently updating — Matchmaking is paused. Active ranked matches are unaffected."
                    .to_string()
            } else {
                updater.banner_message.clone()
            };
            (
                Color::srgba(0.42, 0.06, 0.08, 0.96),
                Color::srgb(0.96, 0.28, 0.26),
                Color::srgb(0.76, 0.14, 0.16),
                badge,
                msg,
                true,
            )
        } else {
            let v = updater
                .latest_patch
                .as_ref()
                .map(|p| p.version.as_str())
                .unwrap_or("new");
            (
                Color::srgba(0.38, 0.22, 0.06, 0.96),
                Color::srgb(0.96, 0.72, 0.24),
                Color::srgb(0.68, 0.42, 0.08),
                "PATCH READY",
                format!(
                    "Patch v{} is ready — Reconnect and finish your active ranked match first; update will install afterward.",
                    v
                ),
                false,
            )
        };

        commands
            .spawn((
                CoordinatorBannerRoot,
                GlobalZIndex(265),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(10.0),
                    left: Val::Percent(16.0),
                    right: Val::Percent(16.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(10.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(7.0),
                    border: UiRect::all(Val::Px(1.8)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(bg_col),
                BorderColor::all(border_col),
            ))
            .with_children(|banner| {
                // ردیف بالا: نقطه چشمک‌زن + برچسب وضعیت قرمز + متن اصلی
                banner
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|row| {
                        if show_loader {
                            row.spawn((
                                CoordinatorPulseDot,
                                Node {
                                    width: Val::Px(10.0),
                                    height: Val::Px(10.0),
                                    border_radius: BorderRadius::MAX,
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(1.0, 0.42, 0.38)),
                            ));
                        }
                        row.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(badge_bg),
                        ))
                        .with_children(|bdg| {
                            bdg.spawn((
                                Text::new(badge_txt),
                                TextFont {
                                    font_size: FontSize::Px(11.0),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                        row.spawn((
                            Text::new(headline_msg),
                            TextFont {
                                font_size: FontSize::Px(13.0),
                                ..default()
                            },
                            TextColor(Color::srgb(1.0, 0.94, 0.94)),
                        ));
                    });

                // ردیف پایین (در حالت Coordinator Updating): لودینگ مفهومی با نوار اسکنر متحرک
                if show_loader {
                    banner
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(10.0),
                            ..default()
                        })
                        .with_children(|lrow| {
                            lrow.spawn((
                                CoordinatorLoadingText,
                                Text::new("SYNCING WITH GAME COORDINATOR..."),
                                TextFont {
                                    font_size: FontSize::Px(10.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(1.0, 0.78, 0.76)),
                            ));
                            lrow.spawn((
                                Node {
                                    flex_grow: 1.0,
                                    height: Val::Px(6.0),
                                    overflow: Overflow::clip(),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.16, 0.02, 0.03, 0.90)),
                            ))
                            .with_children(|track| {
                                track.spawn((
                                    CoordinatorScannerFill,
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: Val::Percent(0.0),
                                        top: Val::Px(0.0),
                                        width: Val::Percent(28.0),
                                        height: Val::Percent(100.0),
                                        border_radius: BorderRadius::all(Val::Px(3.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgb(1.0, 0.38, 0.34)),
                                ));
                            });
                        });
                }
            });
    }

    // ۲. مودال آپدیت و نوار پیشرفت مفهومی (فقط وقتی بازیکن نبرد فعال برای Reconnect ندارد)
    let dismissed = updater
        .latest_patch
        .as_ref()
        .is_some_and(|p| updater.update_dismissed_for_build == Some(p.build_number));
    let show_modal = !has_active_ranked_match
        && (updater.stage != UpdateStage::Idle
            || (updater.has_newer_patch()
                && (updater.is_mandatory_update_pending() || !dismissed)));

    if !show_modal {
        return;
    }

    let Some(patch) = updater.latest_patch.clone() else {
        if updater.stage == UpdateStage::Idle {
            return;
        }
        return;
    };

    commands
        .spawn((
            UpdaterModalRoot,
            UpdaterInteractiveArea,
            Hovered::default(),
            GlobalZIndex(280),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 0.78)),
        ))
        .with_children(|backdrop| {
            backdrop
                .spawn((
                    UpdaterInteractiveArea,
                    Hovered::default(),
                    Node {
                        width: Val::Px(540.0),
                        padding: UiRect::all(Val::Px(22.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.10, 0.13, 0.98)),
                    BorderColor::all(Color::srgb(0.28, 0.56, 0.82)),
                ))
                .with_children(|card| {
                    // تیتر و نشان نسخه
                    card.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|hdr| {
                        hdr.spawn((
                            Text::new(if updater.is_updating_active() {
                                "CLIENT UPDATE IN PROGRESS"
                            } else {
                                "NEW CLIENT PATCH AVAILABLE"
                            }),
                            TextFont {
                                font_size: FontSize::Px(17.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.96, 0.98, 1.0)),
                        ));
                        hdr.spawn((
                            Text::new(format!(
                                "v{} (#{})  ->  v{} (#{})",
                                updater.local_version,
                                updater.local_build,
                                patch.version,
                                patch.build_number
                            )),
                            TextFont {
                                font_size: FontSize::Px(12.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.45, 0.82, 0.98)),
                        ));
                    });

                    // عنوان پچ
                    card.spawn((
                        Text::new(patch.title.clone()),
                        TextFont {
                            font_size: FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.84, 0.42)),
                    ));

                    // نمایش ۴ مرحله مفهومی هنگام دانلود/نصب
                    if updater.stage != UpdateStage::Idle {
                        let active_step = match updater.stage {
                            UpdateStage::FetchingManifest => 1,
                            UpdateStage::ScanningLocalFiles => 2,
                            UpdateStage::DownloadingDelta => 3,
                            UpdateStage::ApplyingPatch
                            | UpdateStage::CompletedRestarting
                            | UpdateStage::CompletedNoRestart => 4,
                            _ => 0,
                        };
                        card.spawn(Node {
                            width: Val::Percent(100.0),
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|steps| {
                            for (idx, lbl) in [
                                (1, "1. MANIFEST"),
                                (2, "2. VERIFY"),
                                (3, "3. DOWNLOAD"),
                                (4, "4. APPLY"),
                            ] {
                                let bg = if active_step == idx {
                                    Color::srgb(0.18, 0.48, 0.72)
                                } else if active_step > idx {
                                    Color::srgb(0.16, 0.42, 0.24)
                                } else {
                                    Color::srgb(0.14, 0.17, 0.21)
                                };
                                steps
                                    .spawn((
                                        Node {
                                            flex_grow: 1.0,
                                            flex_basis: Val::Px(0.0),
                                            height: Val::Px(26.0),
                                            justify_content: JustifyContent::Center,
                                            align_items: AlignItems::Center,
                                            border_radius: BorderRadius::all(Val::Px(4.0)),
                                            ..default()
                                        },
                                        BackgroundColor(bg),
                                    ))
                                    .with_children(|s| {
                                        s.spawn((
                                            Text::new(lbl),
                                            TextFont {
                                                font_size: FontSize::Px(10.5),
                                                ..default()
                                            },
                                            TextColor(Color::WHITE),
                                        ));
                                    });
                            }
                        });

                        // نوار پیشرفت (Progress Bar)
                        let pct = match updater.stage {
                            UpdateStage::FetchingManifest => 0.08_f32,
                            UpdateStage::ScanningLocalFiles => {
                                if updater.files_total > 0 {
                                    (updater.files_done as f32 / updater.files_total as f32)
                                        .clamp(0.05, 1.0)
                                } else {
                                    0.2
                                }
                            }
                            UpdateStage::DownloadingDelta => {
                                if updater.bytes_total > 0 {
                                    (updater.bytes_done as f32 / updater.bytes_total as f32)
                                        .clamp(0.0, 1.0)
                                } else {
                                    0.5
                                }
                            }
                            UpdateStage::ApplyingPatch
                            | UpdateStage::CompletedRestarting
                            | UpdateStage::CompletedNoRestart => 1.0,
                            _ => 0.0,
                        };

                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(18.0),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.04, 0.06, 0.08)),
                            BorderColor::all(Color::srgb(0.26, 0.34, 0.44)),
                        ))
                        .with_children(|track| {
                            track.spawn((
                                Node {
                                    width: Val::Percent((pct * 100.0).clamp(0.0, 100.0)),
                                    height: Val::Percent(100.0),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                BackgroundColor(if updater.stage == UpdateStage::Failed {
                                    Color::srgb(0.78, 0.24, 0.22)
                                } else if matches!(
                                    updater.stage,
                                    UpdateStage::CompletedRestarting
                                        | UpdateStage::CompletedNoRestart
                                ) {
                                    Color::srgb(0.22, 0.68, 0.34)
                                } else {
                                    Color::srgb(0.22, 0.64, 0.94)
                                }),
                            ));
                        });

                        // جزئیات زنده دانلود (فایل جاری، حجم، سرعت MB/s و زمان باقی‌مانده)
                        card.spawn((
                            Text::new(updater.stage_headline.clone()),
                            TextFont {
                                font_size: FontSize::Px(13.0),
                                ..default()
                            },
                            TextColor(Color::srgb(0.95, 0.97, 0.99)),
                        ));

                        let telemetry = if updater.stage == UpdateStage::DownloadingDelta {
                            format!(
                                "{:.2} / {:.2} MB  •  {:.1} MB/s  •  ETA {}s  •  File {}/{}: {}",
                                updater.bytes_done as f64 / (1024.0 * 1024.0),
                                updater.bytes_total as f64 / (1024.0 * 1024.0),
                                updater.speed_mbps,
                                updater.eta_secs,
                                updater.files_done.min(updater.files_total),
                                updater.files_total,
                                updater.current_file
                            )
                        } else if updater.stage == UpdateStage::ScanningLocalFiles {
                            format!(
                                "Checking SHA-256 ({}/{}): {}",
                                updater.files_done, updater.files_total, updater.current_file
                            )
                        } else {
                            updater.stage_detail.clone()
                        };
                        card.spawn((
                            Text::new(telemetry),
                            TextFont {
                                font_size: FontSize::Px(11.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.74, 0.80, 0.88)),
                        ));
                    } else {
                        // نمایش توضیحات پچ قبل از شروع دانلود
                        let notes = if patch.release_notes.trim().is_empty() {
                            "Delta update will download only changed files in moba_client.exe and assets/."
                                .to_string()
                        } else {
                            patch.release_notes.clone()
                        };
                        card.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(10.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.05, 0.07, 0.09, 0.92)),
                            BorderColor::all(Color::srgba(0.24, 0.30, 0.38, 0.80)),
                        ))
                        .with_children(|nbox| {
                            nbox.spawn((
                                Text::new(notes),
                                TextFont {
                                    font_size: FontSize::Px(12.5),
                                    ..default()
                                },
                                TextColor(Color::srgb(0.84, 0.89, 0.94)),
                            ));
                        });
                    }

                    // دکمه‌های اقدام
                    card.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(10.0),
                        justify_content: JustifyContent::End,
                        ..default()
                    })
                    .with_children(|actions| {
                        match updater.stage {
                            UpdateStage::Idle => {
                                if !updater.is_mandatory_update_pending() {
                                    spawn_updater_btn(
                                        actions,
                                        "LATER",
                                        UpdaterBtnAction::DismissOptionalUpdate,
                                        Color::srgb(0.24, 0.28, 0.34),
                                    );
                                }
                                spawn_updater_btn(
                                    actions,
                                    "UPDATE NOW",
                                    UpdaterBtnAction::StartUpdate,
                                    Color::srgb(0.18, 0.52, 0.30),
                                );
                            }
                            UpdateStage::Failed => {
                                spawn_updater_btn(
                                    actions,
                                    "RETRY UPDATE",
                                    UpdaterBtnAction::RetryUpdate,
                                    Color::srgb(0.68, 0.32, 0.16),
                                );
                            }
                            UpdateStage::CompletedNoRestart => {
                                spawn_updater_btn(
                                    actions,
                                    "CONTINUE",
                                    UpdaterBtnAction::DismissCompleted,
                                    Color::srgb(0.18, 0.52, 0.30),
                                );
                            }
                            _ => {}
                        }
                    });
                });
        });
}

fn spawn_updater_btn(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: UpdaterBtnAction,
    bg: Color,
) {
    parent
        .spawn((
            UiButton,
            Hovered::default(),
            Node {
                min_width: Val::Px(132.0),
                height: Val::Px(38.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(bg),
            BorderColor::all(Color::srgba(0.34, 0.44, 0.56, 0.85)),
            action,
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

// =============================================================================
// موتور دانلود تفاضلی (Delta Patching)، جایگزینی اتمیک و SHA-256 خالص Rust
// =============================================================================

fn app_root_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_local_build_marker() -> (String, u32) {
    let marker_path = app_root_dir().join("client_build.json");
    if let Ok(raw) = fs::read_to_string(&marker_path) {
        if let Ok(m) = ureq::serde_json::from_str::<LocalBuildMarker>(&raw) {
            if m.build_number >= COMPILED_CLIENT_BUILD {
                return (m.version, m.build_number);
            }
        }
    }
    (COMPILED_CLIENT_VERSION.to_string(), COMPILED_CLIENT_BUILD)
}

fn write_local_build_marker(version: &str, build_number: u32) {
    let marker_path = app_root_dir().join("client_build.json");
    let marker = LocalBuildMarker {
        version: version.to_string(),
        build_number,
    };
    if let Ok(json_str) = ureq::serde_json::to_string_pretty(&marker) {
        let _ = fs::write(marker_path, json_str);
    }
}

fn cleanup_previous_update_artifacts() {
    let root = app_root_dir();
    let old_exe = root.join("moba_client.exe.old");
    if old_exe.exists() {
        let _ = fs::remove_file(&old_exe);
    }
    let staging = root.join(".update_staging");
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
}

fn resolve_download_url(http_base: &str, base_download_url: &str, rel_path: &str) -> String {
    let base = if base_download_url.starts_with("http://") || base_download_url.starts_with("https://")
    {
        base_download_url.trim_end_matches('/').to_string()
    } else {
        format!(
            "{}/{}",
            http_base.trim_end_matches('/'),
            base_download_url.trim_matches('/')
        )
    };
    format!("{}/{}", base, rel_path.trim_start_matches('/'))
}

fn run_delta_patch_worker(
    http_base: &str,
    patch_summary: &PatchInfoDto,
    tx: &Sender<UpdaterWorkerEvent>,
) -> Result<(), String> {
    let _ = tx.send(UpdaterWorkerEvent::StageChanged {
        stage: UpdateStage::FetchingManifest,
        headline: format!("FETCHING MANIFEST v{}", patch_summary.version),
        detail: "Retrieving file hashes from server...".to_string(),
    });

    let manifest_url = format!(
        "{}/api/v1/client/patch/manifest/{}/",
        http_base.trim_end_matches('/'),
        patch_summary.version
    );
    let manifest: PatchInfoDto = ureq::get(&manifest_url)
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|e| format!("Failed to fetch manifest: {}", e))?
        .into_json()
        .map_err(|e| format!("Invalid manifest JSON: {}", e))?;

    if manifest.files.is_empty() {
        return Err("Patch manifest contains no files.".to_string());
    }

    let root = app_root_dir();
    let total_manifest_files = manifest.files.len();
    let mut to_download: Vec<PatchManifestFile> = Vec::new();
    let mut manifest_asset_paths: HashSet<String> = HashSet::new();

    for (idx, f) in manifest.files.iter().enumerate() {
        // محافظت امنیتی در برابر Path Traversal
        if f.path.contains("..") || f.path.starts_with('/') || f.path.starts_with('\\') {
            return Err(format!("Unsafe path in manifest: {}", f.path));
        }
        if f.path.starts_with("assets/") {
            manifest_asset_paths.insert(f.path.clone());
        }
        let _ = tx.send(UpdaterWorkerEvent::ScanProgress {
            checked: idx + 1,
            total: total_manifest_files,
            current_file: f.path.clone(),
        });

        let local_path = root.join(&f.path);
        let matches = local_path.is_file()
            && sha256_file_hex(&local_path)
                .map(|h| h.eq_ignore_ascii_case(&f.sha256))
                .unwrap_or(false);
        if !matches {
            to_download.push(f.clone());
        }
    }

    let total_bytes: u64 = to_download.iter().map(|f| f.size).sum();
    let total_dl_files = to_download.len();
    let _ = tx.send(UpdaterWorkerEvent::DownloadPlan {
        files_to_download: total_dl_files,
        total_manifest_files,
        bytes_to_download: total_bytes,
    });

    let staging_dir = root.join(".update_staging");
    if staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Cannot create staging directory: {}", e))?;

    let dl_start = Instant::now();
    let mut downloaded_bytes: u64 = 0;

    for (file_idx, f) in to_download.iter().enumerate() {
        let url = resolve_download_url(http_base, &manifest.base_download_url, &f.path);
        let staged_path = staging_dir.join(&f.path);
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed creating dir for {}: {}", f.path, e))?;
        }

        let resp = ureq::get(&url)
            .timeout(Duration::from_secs(120))
            .call()
            .map_err(|e| format!("Download failed for {}: {}", f.path, e))?;
        let mut reader = resp.into_reader();
        let mut out_file = fs::File::create(&staged_path)
            .map_err(|e| format!("Failed creating staged file {}: {}", f.path, e))?;

        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("Network read error on {}: {}", f.path, e))?;
            if n == 0 {
                break;
            }
            out_file
                .write_all(&buf[..n])
                .map_err(|e| format!("Disk write error on {}: {}", f.path, e))?;
            downloaded_bytes += n as u64;

            let elapsed = dl_start.elapsed().as_secs_f32().max(0.05);
            let bytes_per_sec = downloaded_bytes as f32 / elapsed;
            let speed_mbps = bytes_per_sec / (1024.0 * 1024.0);
            let remaining_bytes = total_bytes.saturating_sub(downloaded_bytes);
            let eta_secs = if bytes_per_sec > 1.0 {
                (remaining_bytes as f32 / bytes_per_sec).ceil() as u32
            } else {
                0
            };
            let _ = tx.send(UpdaterWorkerEvent::DownloadProgress {
                downloaded_bytes,
                total_bytes,
                completed_files: file_idx,
                total_files: total_dl_files,
                current_file: f.path.clone(),
                speed_mbps,
                eta_secs,
            });
        }
        drop(out_file);

        // بررسی قطعی یکپارچگی فایل دانلودشده با SHA-256
        let actual_hash = sha256_file_hex(&staged_path)
            .map_err(|e| format!("Failed hashing {}: {}", f.path, e))?;
        if !actual_hash.eq_ignore_ascii_case(&f.sha256) {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(format!(
                "SHA-256 mismatch for {} (corrupted download). Please retry.",
                f.path
            ));
        }
    }

    // مرحله ۴: اعمال اتمیک فایل‌ها (اول assets/ و سپس جایگزینی ایمن exe در صورت تغییر)
    let _ = tx.send(UpdaterWorkerEvent::StageChanged {
        stage: UpdateStage::ApplyingPatch,
        headline: "VERIFYING & APPLYING PATCH...".to_string(),
        detail: "Moving verified files into place safely...".to_string(),
    });

    let mut exe_updated = false;
    for f in &to_download {
        let staged = staging_dir.join(&f.path);
        let dest = root.join(&f.path);
        if f.path.eq_ignore_ascii_case("moba_client.exe") {
            exe_updated = true;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed creating target dir for {}: {}", f.path, e))?;
        }
        if dest.exists() {
            let _ = fs::remove_file(&dest);
        }
        fs::rename(&staged, &dest)
            .or_else(|_| fs::copy(&staged, &dest).map(|_| ()))
            .map_err(|e| format!("Failed installing {}: {}", f.path, e))?;
    }

    // جایگزینی ایمن moba_client.exe در ویندوز با ترفند Rename-Running-Binary
    if exe_updated {
        let current_exe = std::env::current_exe()
            .map_err(|e| format!("Cannot resolve current exe path: {}", e))?;
        let old_exe = root.join("moba_client.exe.old");
        let staged_exe = staging_dir.join("moba_client.exe");
        if old_exe.exists() {
            let _ = fs::remove_file(&old_exe);
        }
        fs::rename(&current_exe, &old_exe)
            .map_err(|e| format!("Failed renaming running binary: {}", e))?;
        if let Err(e) = fs::rename(&staged_exe, &current_exe) {
            // Rollback فوری در صورت خطا
            let _ = fs::rename(&old_exe, &current_exe);
            return Err(format!("Failed swapping new binary (rolled back): {}", e));
        }
    }

    write_local_build_marker(&manifest.version, manifest.build_number);
    let _ = fs::remove_dir_all(&staging_dir);

    let needs_restart = exe_updated || !to_download.is_empty();
    if needs_restart {
        if let Ok(exe_path) = std::env::current_exe() {
            let _ = std::process::Command::new(exe_path).spawn();
        }
    }

    let _ = tx.send(UpdaterWorkerEvent::Finished {
        new_version: manifest.version,
        new_build: manifest.build_number,
        needs_restart,
    });
    Ok(())
}

// =============================================================================
// پیاده‌سازی خالص و استاندارد FIPS 180-4 SHA-256 (بدون نیاز به هیچ crate اضافی)
// =============================================================================

fn sha256_file_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = PureSha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize_hex())
}

struct PureSha256 {
    state: [u32; 8],
    bit_len: u64,
    buffer: [u8; 64],
    buf_len: usize,
}

impl PureSha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            bit_len: 0,
            buffer: [0u8; 64],
            buf_len: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.bit_len = self.bit_len.wrapping_add((data.len() as u64) * 8);
        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buffer[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buffer;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(Self::K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    fn finalize_hex(mut self) -> String {
        let bit_len = self.bit_len;
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;
        if self.buf_len > 56 {
            for b in &mut self.buffer[self.buf_len..64] {
                *b = 0;
            }
            let block = self.buffer;
            self.compress(&block);
            self.buf_len = 0;
        }
        for b in &mut self.buffer[self.buf_len..56] {
            *b = 0;
        }
        self.buffer[56..64].copy_from_slice(&bit_len.to_be_bytes());
        let block = self.buffer;
        self.compress(&block);

        let mut out = String::with_capacity(64);
        for word in self.state {
            use std::fmt::Write as _;
            let _ = write!(&mut out, "{:08x}", word);
        }
        out
    }
}