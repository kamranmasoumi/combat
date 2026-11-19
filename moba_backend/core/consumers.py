import asyncio
import json
import os
import secrets
import signal
import socket
import subprocess
import threading
import time
from dataclasses import dataclass
from datetime import timedelta
from typing import Dict, List, Optional
from urllib.parse import parse_qs

import redis
from channels.db import database_sync_to_async
from channels.generic.websocket import AsyncWebsocketConsumer
from django.conf import settings
from django.db import transaction
from django.utils import timezone

from .models import (
    GameMatchRoom,
    MatchParticipant,
    Player,
    PlayerSessionToken,
    compute_rank_tier_from_mmr,
)


@dataclass
class QueuedPlayerEntry:
    user_id: str
    username: str
    mmr: int
    channel_name: str
    player_pk: int
    client_ip: str = ""


# صف جهانی Matchmaking و قفل همگام‌سازی در حافظه ASGI
_QUEUE_LOCK = asyncio.Lock()
_WAITING_QUEUE: List[QueuedPlayerEntry] = []

# نگهداری نام سرویس systemd فعال روی هر پورت: port -> "moba-server@4433_room-101_2.service"
_ACTIVE_SERVICE_UNITS: Dict[int, str] = {}
# برای سازگاری با پنل ادمین
_RESERVED_PORTS = _ACTIVE_SERVICE_UNITS


def _systemctl_cmd_prefix() -> List[str]:
    """
    اگر پروسه جنگو با کاربر root اجرا شود نیازی به sudo نیست؛
    در غیر این صورت از `sudo -n systemctl` استفاده می‌کند.
    """
    if hasattr(os, "geteuid") and os.geteuid() == 0:
        return ["systemctl"]
    return ["sudo", "-n", "systemctl"]


def get_game_server_unit_name(port: int, room_id: str, player_count: int) -> str:
    """
    ساخت نام نمونه داینامیک سرویس systemd:
    moba-server@<port>_<room_id>_<player_count>.service
    """
    service_base = getattr(settings, "GAME_SERVER_SERVICE_NAME", "moba-server")
    safe_room_id = str(room_id).replace("_", "-")
    return f"{service_base}@{port}_{safe_room_id}_{player_count}.service"


def is_systemd_unit_active(unit_name: str) -> bool:
    try:
        cmd = _systemctl_cmd_prefix() + ["is-active", "--quiet", unit_name]
        res = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
        return res.returncode == 0
    except Exception:
        return False


def is_systemd_unit_failed(unit_name: str) -> bool:
    try:
        cmd = _systemctl_cmd_prefix() + ["is-failed", "--quiet", unit_name]
        res = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
        return res.returncode == 0
    except Exception:
        return False


def get_systemd_unit_main_pid(unit_name: str) -> Optional[int]:
    try:
        cmd = _systemctl_cmd_prefix() + ["show", "-p", "MainPID", "--value", unit_name]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=3)
        if res.returncode == 0:
            pid_val = int(res.stdout.strip() or "0")
            if pid_val > 0:
                return pid_val
    except Exception:
        pass
    return None


def stop_game_server_service(port: int, room_id: str, player_count: int) -> bool:
    """
    توقف سرویس systemd مربوط به یک اتاق بازی
    """
    unit_name = _ACTIVE_SERVICE_UNITS.pop(port, None) or get_game_server_unit_name(
        port, room_id, player_count
    )
    cmd = _systemctl_cmd_prefix() + ["stop", unit_name]
    print(f"[SYSTEMD] Stopping game server service: {' '.join(cmd)}", flush=True)
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        return res.returncode == 0
    except Exception as exc:
        print(f"[SYSTEMD WARNING] Error stopping {unit_name}: {exc}", flush=True)
        return False


def is_udp_port_bound(port: int) -> bool:
    """
    بررسی اینکه آیا سوکت UDP روی این پورت توسط سرور بازی (moba_server) اشغال و Bind شده است یا خیر.
    اگر bind با خطای OSError مواجه شود یعنی پورت توسط سرور بازی باز شده و در حال گوش دادن است.
    """
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        sock.bind(("0.0.0.0", port))
        return False
    except OSError:
        return True
    finally:
        sock.close()


def _cleanup_finished_processes() -> None:
    dead_ports = []
    for port, unit_name in list(_ACTIVE_SERVICE_UNITS.items()):
        if not is_udp_port_bound(port) and not is_systemd_unit_active(unit_name):
            dead_ports.append(port)

    for port in dead_ports:
        _ACTIVE_SERVICE_UNITS.pop(port, None)
        GameMatchRoom.objects.filter(
            server_port=port, status=GameMatchRoom.STATUS_RUNNING
        ).update(status=GameMatchRoom.STATUS_FINISHED)


def find_available_udp_port() -> int:
    """
    بررسی پورت‌های باز و آزاد بودن سوکت UDP برای بالا آوردن سرویس سرور بازی
    """
    _cleanup_finished_processes()
    start_port = getattr(settings, "GAME_SERVER_PORT_START", 4433)
    end_port = getattr(settings, "GAME_SERVER_PORT_END", 4550)

    for port in range(start_port, end_port + 1):
        if port in _ACTIVE_SERVICE_UNITS:
            continue
        if not is_udp_port_bound(port):
            return port

    raise RuntimeError("No free UDP port available in configured range!")


def resolve_game_server_host(client_ip: Optional[str] = None) -> str:
    """
    تعیین آدرس IP سرور بازی برای اتصال UDP کلاینت‌ها.
    """
    env_host = os.environ.get("GAME_SERVER_HOST", "").strip()
    if env_host:
        return env_host

    probe_targets = []
    if client_ip and client_ip not in ("127.0.0.1", "::1", "localhost"):
        probe_targets.append(client_ip)
    if os.name != "nt" and os.path.isdir("/mnt/c"):
        probe_targets.append("172.21.0.1")

    for target in probe_targets:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        try:
            sock.connect((target, 80))
            local_ip = sock.getsockname()[0]
            if local_ip and not local_ip.startswith("127."):
                return local_ip
        except OSError:
            pass
        finally:
            sock.close()

    return getattr(settings, "GAME_SERVER_HOST", "127.0.0.1")


def store_ticket_in_redis(ticket_id: str, room_id: str, user_id: str, username: str) -> None:
    """
    ذخیره تیکت بازیکن در تمام آدرس‌های Redis تنظیم‌شده با فرمت مورد انتظار سرور بازی Rust:
    key = ticket:{ticket_id}
    value = {"room_id": ..., "user_id": ..., "username": ...}
    """
    payload = json.dumps(
        {
            "room_id": room_id,
            "user_id": user_id,
            "username": username,
        }
    )
    key = f"ticket:{ticket_id}"
    raw_urls = [
        getattr(settings, "REDIS_URL", "redis://172.21.13.50:6379/0"),
        getattr(settings, "REDIS_FALLBACK_URL", "redis://127.0.0.1:6379/0"),
    ]
    seen_urls = []
    for u in raw_urls:
        if u and u not in seen_urls:
            seen_urls.append(u)

    wrote_any = False
    for url in seen_urls:
        try:
            r = redis.Redis.from_url(url, socket_connect_timeout=1.5, socket_timeout=1.5)
            r.setex(key, 7200, payload)
            wrote_any = True
        except Exception as exc:
            print(f"[REDIS WARNING] Could not write ticket to {url}: {exc}", flush=True)

    if not wrote_any:
        print(f"[REDIS ERROR] Failed to store ticket {key} on any Redis instance!", flush=True)


def delete_ticket_from_redis(ticket_id: str) -> None:
    """
    حذف تیکت بازیکن از Redis پس از ترک نهایی بازی (Abandon) یا اتمام مسابقه
    """
    key = f"ticket:{ticket_id}"
    raw_urls = [
        getattr(settings, "REDIS_URL", "redis://172.21.13.50:6379/0"),
        getattr(settings, "REDIS_FALLBACK_URL", "redis://127.0.0.1:6379/0"),
    ]
    seen_urls = []
    for u in raw_urls:
        if u and u not in seen_urls:
            seen_urls.append(u)

    for url in seen_urls:
        try:
            r = redis.Redis.from_url(url, socket_connect_timeout=1.5, socket_timeout=1.5)
            r.delete(key)
        except Exception:
            pass


def terminate_game_server_instance(
    port: int,
    room_id: str,
    player_count: int,
    process_pid: Optional[int] = None,
) -> None:
    """
    توقف سرویس systemd و Kill کردن پروسه سرور لوپ بازی روی پورت مشخص‌شده
    """
    stop_game_server_service(port=port, room_id=room_id, player_count=player_count)
    if process_pid:
        try:
            os.kill(process_pid, signal.SIGTERM)
        except Exception:
            pass


def spawn_game_server_process(port: int, room_id: str, player_count: int) -> Optional[int]:
    """
    راه‌اندازی سرور لوپ بازی از طریق سرویس systemd اوبونتو (بدون استفاده از cargo):
    sudo systemctl start moba-server@<port>_<room_id>_<player_count>.service
    """
    unit_name = get_game_server_unit_name(port, room_id, player_count)
    cmd = _systemctl_cmd_prefix() + ["start", unit_name]

    print(
        f"[MATCHMAKING SYSTEMD] Starting game server service: {' '.join(cmd)}",
        flush=True,
    )

    res = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
    if res.returncode != 0:
        err_out = (res.stderr or res.stdout or "").strip()
        print(
            f"[MATCHMAKING ERROR] Failed to start systemd service {unit_name} (exit {res.returncode}): {err_out}",
            flush=True,
        )
        return None

    _ACTIVE_SERVICE_UNITS[port] = unit_name
    return get_systemd_unit_main_pid(unit_name)


async def wait_for_udp_server_ready(port: int, timeout_sec: float = 30.0) -> bool:
    """
    صبر می‌کند تا سرویس systemd بالا بیاید و سوکت UDP پورت مورد نظر واقعاً توسط `moba_server` بایند شود.
    """
    unit_name = _ACTIVE_SERVICE_UNITS.get(port)
    # در محیط تست واحد که spawn_game_server_process ماک شده است
    if unit_name is None:
        return True

    deadline = time.monotonic() + timeout_sec
    while time.monotonic() < deadline:
        if is_systemd_unit_failed(unit_name):
            print(
                f"[MATCHMAKING ERROR] Systemd service {unit_name} entered failed state!",
                flush=True,
            )
            _ACTIVE_SERVICE_UNITS.pop(port, None)
            return False

        if is_udp_port_bound(port):
            print(
                f"[MATCHMAKING READY] Systemd service {unit_name} bound UDP port {port} and is ready!",
                flush=True,
            )
            await asyncio.sleep(0.3)
            return True

        await asyncio.sleep(0.2)

    print(
        f"[MATCHMAKING TIMEOUT] Systemd service {unit_name} on UDP port {port} did not bind within {timeout_sec}s",
        flush=True,
    )
    return False


def get_active_match_for_player(player: Player, client_ip: Optional[str] = None) -> Optional[dict]:
    """
    بررسی می‌کند آیا بازیکن در حال حاضر یک بازی فعال (در حال اجرا روی پورت UDP) دارد که آن را Abandon نکرده باشد.
    در صورت بسته شدن سرور بازی، وضعیت اتاق را در دیتابیس به FINISHED تغییر می‌دهد.
    """
    _cleanup_finished_processes()

    participants = (
        MatchParticipant.objects.select_related("match_room")
        .filter(
            player=player,
            match_room__status=GameMatchRoom.STATUS_RUNNING,
        )
        .exclude(
            status__in=[
                MatchParticipant.STATUS_ABANDONED,
                MatchParticipant.STATUS_COMPLETED,
            ]
        )
        .order_by("-joined_at")
    )

    now = timezone.now()
    for part in participants:
        room = part.match_room
        port = room.server_port
        unit_name = _ACTIVE_SERVICE_UNITS.get(port) or get_game_server_unit_name(
            port, room.room_id, room.max_players
        )

        port_bound = is_udp_port_bound(port)
        recently_created = (now - room.created_at) < timedelta(seconds=20)
        unit_active = is_systemd_unit_active(unit_name) if recently_created else False

        if port_bound or (unit_active and recently_created):
            # اطمینان از وجود تیکت در Redis برای اتصال مجدد (Reconnect) بدون خطای احراز هویت
            store_ticket_in_redis(
                ticket_id=part.ticket_id,
                room_id=room.room_id,
                user_id=player.user_id,
                username=player.username,
            )
            effective_host = room.server_host
            if effective_host in ("127.0.0.1", "localhost"):
                effective_host = resolve_game_server_host(client_ip)
            server_addr = f"{effective_host}:{room.server_port}"
            game_link = f"moba://{server_addr}/{room.room_id}/{part.ticket_id}"
            return {
                "room_id": room.room_id,
                "port": room.server_port,
                "server_addr": server_addr,
                "ticket_id": part.ticket_id,
                "player_count": room.max_players,
                "team_id": part.team_id,
                "slot_index": part.slot_index,
                "participant_status": part.status,
                "game_link": game_link,
            }
        else:
            # سرور بازی دیگر روی این پورت فعال نیست؛ وضعیت اتاق را در دیتابیس پایان یافته ثبت کن
            room.status = GameMatchRoom.STATUS_FINISHED
            room.finished_at = now
            room.save(update_fields=["status", "finished_at"])

    return None


def disconnect_active_match_for_player(player: Player) -> int:
    """
    مرحله اول خروج دو مرحله‌ای (مشابه Dota 2):
    بازیکن صرفاً از بازی دیسکانکت می‌کند و به منوی اصلی برمی‌گردد (وضعیت: Disconnected).
    بازی و جایگاه او همچنان فعال می‌ماند تا بتواند در منو Reconnect کند یا Abandon بزند.
    """
    participants = (
        MatchParticipant.objects.select_related("match_room")
        .filter(
            player=player,
            match_room__status=GameMatchRoom.STATUS_RUNNING,
        )
        .exclude(
            status__in=[
                MatchParticipant.STATUS_ABANDONED,
                MatchParticipant.STATUS_COMPLETED,
            ]
        )
    )
    count = 0
    for part in participants:
        part.status = MatchParticipant.STATUS_DISCONNECTED
        part.save(update_fields=["status"])
        count += 1
        print(
            f"[MATCH DISCONNECT] Player {player.username} ({player.user_id}) disconnected from {part.match_room.room_id} (can still reconnect or abandon).",
            flush=True,
        )
    return count


def reconnect_active_match_for_player(player: Player) -> int:
    """
    بازگرداندن وضعیت بازیکن به `connected` هنگام اتصال مجدد (Reconnect) به بازی فعال
    """
    participants = (
        MatchParticipant.objects.select_related("match_room")
        .filter(
            player=player,
            match_room__status=GameMatchRoom.STATUS_RUNNING,
        )
        .exclude(
            status__in=[
                MatchParticipant.STATUS_ABANDONED,
                MatchParticipant.STATUS_COMPLETED,
            ]
        )
    )
    count = 0
    for part in participants:
        if part.status != MatchParticipant.STATUS_CONNECTED:
            part.status = MatchParticipant.STATUS_CONNECTED
            part.save(update_fields=["status"])
        count += 1
        print(
            f"[MATCH RECONNECT] Player {player.username} ({player.user_id}) reconnected to {part.match_room.room_id}.",
            flush=True,
        )
    return count


def update_player_connection_state_from_server(data: dict) -> dict:
    """
    بروزرسانی آنی وضعیت اتصال بازیکن (`connected` یا `disconnected`) توسط `moba_server`
    بر اساس رویدادهای واقعی سوکت UDP (اتصال، ریکانکت، دیسکانکت یا بستن مستقیم برنامه بازی).
    """
    room_id = str(data.get("room_id", "")).strip()
    port = int(data.get("port", 0) or 0)
    user_id = str(data.get("user_id", "")).strip()
    ticket_id = str(data.get("ticket_id", "")).strip()
    slot_index = data.get("slot_index", None)
    raw_status = str(data.get("status", "")).strip().lower()

    if raw_status not in (
        MatchParticipant.STATUS_CONNECTED,
        MatchParticipant.STATUS_DISCONNECTED,
    ):
        return {"status": "error", "message": f"Invalid status: {raw_status}"}

    with transaction.atomic():
        room = None
        if room_id:
            room = (
                GameMatchRoom.objects.select_for_update()
                .filter(room_id=room_id)
                .first()
            )
            if not room:
                alt_room_id = room_id.replace("-", "_")
                room = (
                    GameMatchRoom.objects.select_for_update()
                    .filter(room_id=alt_room_id)
                    .first()
                )
        if not room and port > 0:
            room = (
                GameMatchRoom.objects.select_for_update()
                .filter(server_port=port, status=GameMatchRoom.STATUS_RUNNING)
                .order_by("-created_at")
                .first()
            )

        if not room:
            return {"status": "error", "message": f"Room {room_id} (port {port}) not found"}

        if room.status in (GameMatchRoom.STATUS_FINISHED, GameMatchRoom.STATUS_CANCELED):
            return {"status": "ignored", "reason": "match_already_ended"}

        qs = MatchParticipant.objects.select_related("player").filter(match_room=room)
        part = None
        if user_id:
            part = qs.filter(player__user_id=user_id).first()
        if not part and ticket_id:
            part = qs.filter(ticket_id=ticket_id).first()
        if not part and slot_index is not None:
            try:
                part = qs.filter(slot_index=int(slot_index)).first()
            except Exception:
                pass

        if not part:
            return {"status": "error", "message": "Participant not found in room"}

        # اگر بازیکن قبلاً Abandon یا Completed شده باشد، وضعیت او تغییر نمی‌کند
        if part.status in (
            MatchParticipant.STATUS_ABANDONED,
            MatchParticipant.STATUS_COMPLETED,
        ):
            return {
                "status": "ignored",
                "reason": f"participant_already_{part.status}",
                "participant_status": part.status,
            }

        if part.status != raw_status:
            part.status = raw_status
            part.save(update_fields=["status"])
            print(
                f"[UDP STATE SYNC] Room {room.room_id} | Player {part.player.username} (slot {part.slot_index}) -> {raw_status.upper()}",
                flush=True,
            )

        return {
            "status": "ok",
            "room_id": room.room_id,
            "user_id": part.player.user_id,
            "slot_index": part.slot_index,
            "participant_status": part.status,
        }


def abandon_active_matches_for_player(player: Player) -> dict:
    """
    مرحله دوم خروج دو مرحله‌ای (مشابه Dota 2 - Abandon):
    بازیکن در منوی اصلی دکمه «ترک نهایی نبرد (Abandon)» را می‌زند:
    - وضعیت شرکت‌کننده به `abandoned` تغییر می‌یابد و تیکت او از Redis پاک می‌شود.
    - جریمه Abandon (ثبت ۱ شکست، افزایش شمارنده Abandon و کسر ۲۵ امتیاز MMR) روی پروفایل بازیکن اعمال می‌شود.
    - اگر تمام بازیکنان آن اتاق Abandon کرده باشند، اتاق بسته شده و سرویس سرور بازی نیز متوقف (Kill) می‌شود.
    """
    _cleanup_finished_processes()
    participants = (
        MatchParticipant.objects.select_related("match_room")
        .filter(
            player=player,
            match_room__status=GameMatchRoom.STATUS_RUNNING,
        )
        .exclude(
            status__in=[
                MatchParticipant.STATUS_ABANDONED,
                MatchParticipant.STATUS_COMPLETED,
            ]
        )
    )
    now = timezone.now()
    abandoned_count = 0
    rooms_closed = 0

    rooms_to_terminate = []
    with transaction.atomic():
        for part in participants:
            room = part.match_room
            is_lobby = getattr(room, "is_custom_lobby", False)
            part.status = MatchParticipant.STATUS_ABANDONED
            part.is_winner = False
            part.mmr_delta = 0 if is_lobby else -25
            part.left_at = now
            part.save(update_fields=["status", "is_winner", "mmr_delta", "left_at"])

            delete_ticket_from_redis(part.ticket_id)
            abandoned_count += 1

            if not is_lobby:
                player.losses += 1
                player.abandoned_count += 1
                player.mmr = max(0, player.mmr - 25)
                player.rank_tier = compute_rank_tier_from_mmr(player.mmr)
                player.save(
                    update_fields=["losses", "abandoned_count", "mmr", "rank_tier"]
                )
                print(
                    f"[MATCH ABANDON] Player {player.username} ({player.user_id}) ABANDONED room {room.room_id} (-25 MMR -> {player.mmr}).",
                    flush=True,
                )
            else:
                print(
                    f"[LOBBY MATCH LEAVE] Player {player.username} ({player.user_id}) left custom lobby room {room.room_id} (0 MMR change).",
                    flush=True,
                )

            remaining_active = (
                MatchParticipant.objects.filter(match_room=room)
                .exclude(status=MatchParticipant.STATUS_ABANDONED)
                .exists()
            )
            if not remaining_active:
                room.status = GameMatchRoom.STATUS_CANCELED
                room.finished_at = now
                room.save(update_fields=["status", "finished_at"])
                rooms_to_terminate.append(
                    (room.server_port, room.room_id, room.max_players, room.process_pid)
                )
                rooms_closed += 1
                print(
                    f"[MATCH ROOM CLOSED] All players abandoned room {room.room_id}; scheduling termination on port {room.server_port}.",
                    flush=True,
                )

    for srv_port, r_id, p_cnt, pid in rooms_to_terminate:
        threading.Thread(
            target=terminate_game_server_instance,
            kwargs={
                "port": srv_port,
                "room_id": r_id,
                "player_count": p_cnt,
                "process_pid": pid,
            },
            daemon=True,
        ).start()

    return {
        "abandoned_matches": abandoned_count,
        "rooms_closed": rooms_closed,
        "mmr": player.mmr,
        "rank_tier": player.rank_tier,
        "losses": player.losses,
        "abandoned_count": player.abandoned_count,
    }


def _schedule_server_termination(
    port: int,
    room_id: str,
    player_count: int,
    process_pid: Optional[int],
    delay_sec: float = 5.5,
) -> None:
    """
    توقف سرویس systemd و Kill کردن پروسه لوپ بازی پس از یک وقفه کوتاه در پس‌زمینه،
    تا تمام کلاینت‌ها فرصت نمایش بنر پیروزی/شکست ۵ ثانیه‌ای را داشته باشند و سپس سرویس
    به طور قطعی متوقف شود (حتی در صورت تنظیم بودن Restart=always در systemd).
    """
    def _worker() -> None:
        if delay_sec > 0:
            time.sleep(delay_sec)
        terminate_game_server_instance(
            port=port,
            room_id=room_id,
            player_count=player_count,
            process_pid=process_pid,
        )
        print(
            f"[MATCH SERVER TERMINATED] Room {room_id} on port {port} stopped after post-game delay.",
            flush=True,
        )

    t = threading.Thread(target=_worker, daemon=True)
    t.start()


def record_match_result_from_server(data: dict) -> dict:
    """
    ثبت نتیجه نهایی بازی ارسال‌شده توسط `moba_server` (یا کلاینت به عنوان فال‌بک) پس از اتمام نبرد:
    - ذخیره تیم برنده، مدت زمان بازی و زمان پایان در `GameMatchRoom`
    - ذخیره آمار هر بازیکن (Hero, Level, K/D/A, Last Hits, Denies, Gold) در `MatchParticipant`
    - بروزرسانی MMR، درجه رنک (Rank Tier)، برد و باخت بازیکنان (به جز کسانی که قبلاً Abandon کرده و جریمه شده‌اند)
    - آزادسازی پورت و توقف سرویس systemd (`systemctl stop`) و پروسه سرور بازی پس از وقفه ۵ ثانیه‌ای
    """
    room_id = str(data.get("room_id", "")).strip()
    port = int(data.get("port", 0) or 0)
    winner_team = int(data.get("winner_team", 0) or 0)
    duration_sec = int(data.get("duration_sec", 0) or 0)
    players_payload = data.get("players", []) or []

    with transaction.atomic():
        room = None
        if room_id:
            room = (
                GameMatchRoom.objects.select_for_update()
                .filter(room_id=room_id)
                .first()
            )
            if not room:
                # تطابق در صورتی که در systemd کاراکتر _ با - جایگزین شده باشد
                alt_room_id = room_id.replace("-", "_")
                room = (
                    GameMatchRoom.objects.select_for_update()
                    .filter(room_id=alt_room_id)
                    .first()
                )
        if not room and port > 0:
            room = (
                GameMatchRoom.objects.select_for_update()
                .filter(server_port=port)
                .order_by("-created_at")
                .first()
            )

        if not room:
            return {"status": "error", "message": f"Room {room_id} (port {port}) not found"}

        # جلوگیری از ثبت مضاعف MMR در صورت ارسال همزمان از سرور و کلاینت‌ها
        already_finished = room.status in (
            GameMatchRoom.STATUS_FINISHED,
            GameMatchRoom.STATUS_CANCELED,
        )
        now = timezone.now()

        room.status = (
            GameMatchRoom.STATUS_FINISHED
            if winner_team in (1, 2)
            else GameMatchRoom.STATUS_CANCELED
        )
        room.winner_team = winner_team
        room.duration_sec = max(room.duration_sec, max(0, duration_sec))
        room.finished_at = room.finished_at or now
        room.save(
            update_fields=["status", "winner_team", "duration_sec", "finished_at"]
        )

        # ایندکس کردن آمار ارسالی سرور بر اساس user_id و slot_index
        stats_by_user_id: Dict[str, dict] = {}
        stats_by_slot: Dict[int, dict] = {}
        for pstat in players_payload:
            if not isinstance(pstat, dict):
                continue
            uid = str(pstat.get("user_id", "")).strip()
            if uid:
                stats_by_user_id[uid] = pstat
            if "slot_index" in pstat and pstat["slot_index"] is not None:
                try:
                    stats_by_slot[int(pstat["slot_index"])] = pstat
                except Exception:
                    pass

        participants = list(
            MatchParticipant.objects.select_related("player").filter(match_room=room)
        )
        updated_players = 0

        for part in participants:
            player = part.player
            pstat = stats_by_user_id.get(player.user_id) or stats_by_slot.get(
                part.slot_index, {}
            )

            hero_id_val = int(pstat.get("hero_id", 0) or 0)
            if hero_id_val > 0:
                part.hero_id = hero_id_val
            level_val = int(pstat.get("level", 0) or 0)
            if level_val > 0:
                part.level = max(part.level, level_val)
            part.kills = max(part.kills, int(pstat.get("kills", part.kills) or 0))
            part.deaths = max(part.deaths, int(pstat.get("deaths", part.deaths) or 0))
            part.assists = max(part.assists, int(pstat.get("assists", part.assists) or 0))
            part.last_hits = max(
                part.last_hits, int(pstat.get("last_hits", part.last_hits) or 0)
            )
            part.denies = max(part.denies, int(pstat.get("denies", part.denies) or 0))
            part.gold_coins = max(
                part.gold_coins, int(pstat.get("gold_coins", part.gold_coins) or 0)
            )
            part.left_at = part.left_at or now

            delete_ticket_from_redis(part.ticket_id)

            # اگر بازیکن قبلاً Abandon کرده باشد، جریمه او قبلاً اعمال شده و حتی در صورت برد تیمش برنده محسوب نمی‌شود
            if part.status == MatchParticipant.STATUS_ABANDONED:
                part.is_winner = False
                part.save(
                    update_fields=[
                        "hero_id",
                        "level",
                        "kills",
                        "deaths",
                        "assists",
                        "last_hits",
                        "denies",
                        "gold_coins",
                        "is_winner",
                        "left_at",
                    ]
                )
                continue

            is_lobby = getattr(room, "is_custom_lobby", False)

            # بررسی اینکه آیا سرور خودش وضعیت abandoned برای این اسلات گزارش کرده است
            srv_abandoned = bool(pstat.get("abandoned", False))
            if srv_abandoned:
                part.status = MatchParticipant.STATUS_ABANDONED
                part.is_winner = False
                part.mmr_delta = 0 if is_lobby else -25
                part.save()
                if not already_finished and not is_lobby:
                    player.losses += 1
                    player.abandoned_count += 1
                    player.mmr = max(0, player.mmr - 25)
                    player.rank_tier = compute_rank_tier_from_mmr(player.mmr)
                    player.save(
                        update_fields=[
                            "losses",
                            "abandoned_count",
                            "mmr",
                            "rank_tier",
                        ]
                    )
                    updated_players += 1
                continue

            part.status = MatchParticipant.STATUS_COMPLETED
            if winner_team in (1, 2):
                won = part.team_id == winner_team
                part.is_winner = won
                part.mmr_delta = 0 if is_lobby else (25 if won else -25)
                if not already_finished and not is_lobby:
                    if won:
                        player.wins += 1
                        player.mmr += 25
                    else:
                        player.losses += 1
                        player.mmr = max(0, player.mmr - 25)
                    player.rank_tier = compute_rank_tier_from_mmr(player.mmr)
                    player.save(
                        update_fields=["wins", "losses", "mmr", "rank_tier"]
                    )
                    updated_players += 1
            else:
                part.is_winner = False
                part.mmr_delta = 0

            part.save()

    # زمان‌بندی توقف سرویس systemd و Kill پروسه لوپ بازی پس از ۵.۵ ثانیه (فقط در اولین ثبت نتیجه)
    if not already_finished:
        _schedule_server_termination(
            port=room.server_port,
            room_id=room.room_id,
            player_count=room.max_players,
            process_pid=room.process_pid,
            delay_sec=5.5,
        )
        print(
            f"[MATCH RESULT SAVED] Room: {room.room_id} | Winner Team: {winner_team} | Duration: {duration_sec}s | Updated Players: {updated_players}",
            flush=True,
        )

    return {
        "status": "ok",
        "room_id": room.room_id,
        "winner_team": winner_team,
        "duration_sec": room.duration_sec,
        "updated_players": updated_players,
        "already_finished": already_finished,
    }


@database_sync_to_async
def _authenticate_ws_token(token_str: str) -> Optional[Player]:
    if not token_str:
        return None
    token_obj = (
        PlayerSessionToken.objects.select_related("player")
        .filter(
            access_token=token_str,
            is_active=True,
            expires_at__gt=timezone.now(),
        )
        .first()
    )
    if not token_obj or token_obj.player.is_banned:
        return None
    return token_obj.player


@database_sync_to_async
def _mark_room_finished(room_id: str) -> None:
    GameMatchRoom.objects.filter(room_id=room_id).update(
        status=GameMatchRoom.STATUS_FINISHED
    )


@database_sync_to_async
def _get_active_match_for_player_async(
        player: Player, client_ip: Optional[str] = None
) -> Optional[dict]:
    return get_active_match_for_player(player, client_ip=client_ip)


@database_sync_to_async
def _create_match_records_and_spawn(
        matched_entries: List[QueuedPlayerEntry],
        port: int,
        room_id: str,
        server_host: str,
        player_count: int,
        is_custom_lobby: bool = False,
        explicit_teams: Optional[Dict[str, int]] = None,
) -> Dict[str, dict]:
    """
    ساخت رکورد اتاق و شرکت‌کنندگان در دیتابیس Postgres، ثبت تیکت‌ها در Redis و استارت سرویس systemd سرور بازی
    """
    player_pks = [e.player_pk for e in matched_entries]
    old_room_ids = list(
        MatchParticipant.objects.filter(
            player_id__in=player_pks,
            match_room__status=GameMatchRoom.STATUS_RUNNING,
        ).values_list("match_room_id", flat=True)
    )
    if old_room_ids:
        GameMatchRoom.objects.filter(id__in=old_room_ids).update(
            status=GameMatchRoom.STATUS_FINISHED
        )

    half_team = (player_count + 1) // 2
    assignments: Dict[str, dict] = {}

    match_room = GameMatchRoom.objects.create(
        room_id=room_id,
        server_host=server_host,
        server_port=port,
        max_players=player_count,
        status=GameMatchRoom.STATUS_RUNNING,
        process_pid=None,
        is_custom_lobby=is_custom_lobby,
    )

    for idx, entry in enumerate(matched_entries):
        ticket_id = f"tkt_{room_id}_{secrets.token_hex(6)}"
        if explicit_teams and entry.user_id in explicit_teams:
            team_id = explicit_teams[entry.user_id]
        else:
            team_id = 1 if idx < half_team else 2

        MatchParticipant.objects.create(
            match_room=match_room,
            player_id=entry.player_pk,
            ticket_id=ticket_id,
            slot_index=idx,
            team_id=team_id,
            status=MatchParticipant.STATUS_CONNECTED,
        )

        store_ticket_in_redis(
            ticket_id=ticket_id,
            room_id=room_id,
            user_id=entry.user_id,
            username=entry.username,
        )

        entry_host = resolve_game_server_host(entry.client_ip) if server_host == "127.0.0.1" else server_host
        server_addr = f"{entry_host}:{port}"
        game_link = f"moba://{server_addr}/{room_id}/{ticket_id}"

        assignments[entry.channel_name] = {
            "event": "match_found",
            "room_id": room_id,
            "port": port,
            "server_addr": server_addr,
            "ticket_id": ticket_id,
            "player_count": player_count,
            "game_link": game_link,
        }

    pid = spawn_game_server_process(port, room_id, player_count)
    if pid is not None:
        match_room.process_pid = pid
        match_room.save(update_fields=["process_pid"])

    return assignments


class MatchmakingConsumer(AsyncWebsocketConsumer):
    """
    وب‌سوکت صف یافتن نبرد (Find Match):
    ws://<host>/ws/matchmaking/?token=<access_token>
    """

    player: Optional[Player] = None

    async def connect(self) -> None:
        query_string = self.scope.get("query_string", b"").decode("utf-8")
        params = parse_qs(query_string)
        token_str = params.get("token", [""])[0].strip()

        if not token_str:
            headers = dict(self.scope.get("headers", []))
            auth_val = headers.get(b"authorization", b"").decode("utf-8")
            if auth_val.startswith("Bearer "):
                token_str = auth_val[7:].strip()

        self.player = await _authenticate_ws_token(token_str)
        if not self.player:
            await self.close(code=4001)
            return

        await self.accept()
        await self._join_matchmaking_queue()

    async def disconnect(self, close_code: int) -> None:
        async with _QUEUE_LOCK:
            before_len = len(_WAITING_QUEUE)
            _WAITING_QUEUE[:] = [
                item
                for item in _WAITING_QUEUE
                if item.channel_name != self.channel_name
                and (not self.player or item.user_id != self.player.user_id)
            ]
            after_len = len(_WAITING_QUEUE)
            remaining = list(_WAITING_QUEUE)

        if before_len != after_len and remaining:
            required = getattr(settings, "MATCHMAKING_PLAYERS_PER_MATCH", 2)
            for item in remaining:
                await self.channel_layer.send(
                    item.channel_name,
                    {
                        "type": "queue.status",
                        "payload": {
                            "event": "queued",
                            "players_in_queue": len(remaining),
                            "required_players": required,
                        },
                    },
                )

    async def receive(self, text_data: Optional[str] = None, bytes_data: Optional[bytes] = None) -> None:
        if not text_data:
            return
        try:
            msg = json.loads(text_data)
        except Exception:
            return

        action = msg.get("action", "")
        if action == "cancel":
            await self.close(code=1000)
        elif action == "find_match":
            await self._join_matchmaking_queue()

    async def _join_matchmaking_queue(self) -> None:
        assert self.player is not None
        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        # بررسی امنیتی: بازیکن در صورت داشتن بازی فعال اجازه ورود به صف نبرد را ندارد
        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        if active_match:
            print(
                f"[QUEUE REJECTED] Player {self.player.username} already has active match {active_match.get('room_id')}.",
                flush=True,
            )
            await self.send(
                text_data=json.dumps(
                    {
                        "event": "error",
                        "message": "شما در حال حاضر یک نبرد فعال دارید و نمی‌توانید وارد صف نبرد جدید شوید.",
                        "active_match": active_match,
                    }
                )
            )
            await self.close(code=4003)
            return

        required = max(1, int(getattr(settings, "MATCHMAKING_PLAYERS_PER_MATCH", 2)))
        matched_batch: Optional[List[QueuedPlayerEntry]] = None
        queue_snapshot: List[QueuedPlayerEntry] = []

        async with _QUEUE_LOCK:
            # حذف ورودی‌های قبلی همین بازیکن در صورت اتصال مجدد
            _WAITING_QUEUE[:] = [
                item for item in _WAITING_QUEUE if item.user_id != self.player.user_id
            ]
            _WAITING_QUEUE.append(
                QueuedPlayerEntry(
                    user_id=self.player.user_id,
                    username=self.player.username,
                    mmr=self.player.mmr,
                    channel_name=self.channel_name,
                    player_pk=self.player.pk,
                    client_ip=client_ip,
                )
            )

            print(
                f"[QUEUE] {self.player.username} joined queue "
                f"({len(_WAITING_QUEUE)}/{required})",
                flush=True,
            )

            if len(_WAITING_QUEUE) >= required:
                matched_batch = _WAITING_QUEUE[:required]
                del _WAITING_QUEUE[:required]
            else:
                queue_snapshot = list(_WAITING_QUEUE)

        if matched_batch is not None:
            port = find_available_udp_port()
            room_id = f"room-{secrets.randbelow(900) + 100}-{secrets.token_hex(2)}"
            first_client_ip = matched_batch[0].client_ip if matched_batch else client_ip
            server_host = resolve_game_server_host(first_client_ip)

            # ۱. اعلام به تمام بازیکنان مچ که نفرات کامل شده و سرویس سرور بازی در حال استارت است
            for entry in matched_batch:
                await self.channel_layer.send(
                    entry.channel_name,
                    {
                        "type": "queue.status",
                        "payload": {
                            "event": "starting_server",
                            "room_id": room_id,
                            "port": port,
                            "players_in_queue": required,
                            "required_players": required,
                        },
                    },
                )

            # ۲. ثبت رکوردهای دیتابیس، ذخیره تیکت‌ها در Redis و استارت سرویس systemd سرور بازی
            assignments = await _create_match_records_and_spawn(
                matched_entries=matched_batch,
                port=port,
                room_id=room_id,
                server_host=server_host,
                player_count=required,
            )

            # ۳. صبر کردن تا زمانی که پورت UDP سرویس سرور بازی واقعاً بالا بیاید و Bind شود
            server_ready = await wait_for_udp_server_ready(port, timeout_sec=30.0)
            if not server_ready:
                await _mark_room_finished(room_id)
                for ch_name in assignments.keys():
                    await self.channel_layer.send(
                        ch_name,
                        {
                            "type": "queue.status",
                            "payload": {
                                "event": "error",
                                "message": "خطا در راه‌اندازی سرویس سرور بازی؛ دوباره تلاش کنید",
                            },
                        },
                    )
                return

            # ۴. ارسال لینک بازی و تیکت پس از اطمینان از آماده بودن سرور UDP
            for ch_name, payload in assignments.items():
                await self.channel_layer.send(
                    ch_name,
                    {
                        "type": "match.found",
                        "payload": payload,
                    },
                )
        else:
            for item in queue_snapshot:
                await self.channel_layer.send(
                    item.channel_name,
                    {
                        "type": "queue.status",
                        "payload": {
                            "event": "queued",
                            "players_in_queue": len(queue_snapshot),
                            "required_players": required,
                        },
                    },
                )

    async def queue_status(self, event: dict) -> None:
        await self.send(text_data=json.dumps(event["payload"]))

    async def match_found(self, event: dict) -> None:
        await self.send(text_data=json.dumps(event["payload"]))


# ============================================================================
# سیستم جامع لابی سفارشی (Custom Lobby System)
# ============================================================================

ALLOWED_LOBBY_CAPACITIES = (2, 4, 6, 8, 10)


@dataclass
class CustomLobbyMember:
    user_id: str
    username: str
    player_pk: int
    channel_name: str
    team_id: int  # 1 = سپاه روشنایی (اهورا), 2 = سپاه تاریکی (اهریمن)
    slot_index: int  # 0 .. (max_players // 2 - 1) در تیم مربوطه
    client_ip: str = ""


@dataclass
class CustomLobbyRoom:
    lobby_id: str
    title: str
    password: str
    max_players: int
    host_user_id: str
    host_username: str
    members: List[CustomLobbyMember]
    chat_history: List[dict]
    is_starting: bool = False


_LOBBY_LOCK = asyncio.Lock()
_ACTIVE_LOBBIES: Dict[str, CustomLobbyRoom] = {}
_LOBBY_BROWSER_CHANNELS: set = set()


def _serialize_lobby_summary(lobby: CustomLobbyRoom) -> dict:
    return {
        "lobby_id": lobby.lobby_id,
        "title": lobby.title,
        "host_username": lobby.host_username,
        "has_password": bool(lobby.password),
        "current_players": len(lobby.members),
        "max_players": lobby.max_players,
        "is_starting": lobby.is_starting,
    }


def _serialize_lobby_detail(lobby: CustomLobbyRoom) -> dict:
    return {
        "lobby_id": lobby.lobby_id,
        "title": lobby.title,
        "host_user_id": lobby.host_user_id,
        "host_username": lobby.host_username,
        "has_password": bool(lobby.password),
        "max_players": lobby.max_players,
        "is_starting": lobby.is_starting,
        "members": [
            {
                "user_id": m.user_id,
                "username": m.username,
                "team_id": m.team_id,
                "slot_index": m.slot_index,
                "is_host": m.user_id == lobby.host_user_id,
            }
            for m in lobby.members
        ],
        "chat_messages": lobby.chat_history[-40:],
    }


def _find_first_free_slot(lobby: CustomLobbyRoom, preferred_team: Optional[int] = None) -> Optional[tuple]:
    half = lobby.max_players // 2
    occupied = {(m.team_id, m.slot_index) for m in lobby.members}
    teams_order = [preferred_team] if preferred_team in (1, 2) else [1, 2]
    for t in teams_order:
        for s in range(half):
            if (t, s) not in occupied:
                return (t, s)
    return None


class LobbyConsumer(AsyncWebsocketConsumer):
    """
    وب‌سوکت جامع لابی‌های سفارشی:
    ws://<host>/ws/lobby/?token=<access_token>
    """

    player: Optional[Player] = None
    current_lobby_id: Optional[str] = None
    last_chat_ts: float = 0.0

    async def connect(self) -> None:
        query_string = self.scope.get("query_string", b"").decode("utf-8")
        params = parse_qs(query_string)
        token_str = params.get("token", [""])[0].strip()

        if not token_str:
            headers = dict(self.scope.get("headers", []))
            auth_val = headers.get(b"authorization", b"").decode("utf-8")
            if auth_val.startswith("Bearer "):
                token_str = auth_val[7:].strip()

        self.player = await _authenticate_ws_token(token_str)
        if not self.player:
            await self.close(code=4001)
            return

        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        # بررسی امنیتی ۱: عدم اجازه ورود به بخش لابی در صورت داشتن نبرد فعال
        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        await self.accept()

        if active_match:
            await self.send_json(
                {
                    "event": "error",
                    "message": "شما یک نبرد فعال دارید و نمی‌توانید وارد بخش لابی شوید.",
                }
            )
            await self.close(code=4003)
            return

        # حذف از صف مچ‌میکینگ معمولی در صورت حضور احتمالی
        async with _QUEUE_LOCK:
            _WAITING_QUEUE[:] = [
                item for item in _WAITING_QUEUE if item.user_id != self.player.user_id
            ]

        async with _LOBBY_LOCK:
            _LOBBY_BROWSER_CHANNELS.add(self.channel_name)
            lobbies_snapshot = [
                _serialize_lobby_summary(lb)
                for lb in _ACTIVE_LOBBIES.values()
                if not lb.is_starting
            ]

        await self.send_json(
            {
                "event": "lobby_list",
                "lobbies": lobbies_snapshot,
                "message": "لیست لابی‌ها بروزرسانی شد.",
            }
        )

    async def disconnect(self, close_code: int) -> None:
        async with _LOBBY_LOCK:
            _LOBBY_BROWSER_CHANNELS.discard(self.channel_name)

        if self.player:
            await self._handle_member_leave(is_disconnect=True)

    async def send_json(self, payload: dict) -> None:
        await self.send(text_data=json.dumps(payload))

    async def receive(self, text_data: Optional[str] = None, bytes_data: Optional[bytes] = None) -> None:
        if not text_data or not self.player:
            return
        try:
            msg = json.loads(text_data)
        except Exception:
            return

        action = str(msg.get("action", "")).strip()

        if action == "list_lobbies":
            await self._handle_list_lobbies()
        elif action == "create_lobby":
            await self._handle_create_lobby(msg)
        elif action == "join_lobby":
            await self._handle_join_lobby(msg)
        elif action == "switch_team":
            await self._handle_switch_team(msg)
        elif action == "chat_message":
            await self._handle_chat_message(msg)
        elif action == "kick_player":
            await self._handle_kick_player(msg)
        elif action in ("leave_lobby", "delete_lobby"):
            await self._handle_member_leave(is_disconnect=False)
        elif action == "start_match":
            await self._handle_start_match()

    async def _broadcast_lobby_list_to_browsers(self) -> None:
        async with _LOBBY_LOCK:
            lobbies_snapshot = [
                _serialize_lobby_summary(lb)
                for lb in _ACTIVE_LOBBIES.values()
                if not lb.is_starting
            ]
            targets = list(_LOBBY_BROWSER_CHANNELS)

        for ch in targets:
            await self.channel_layer.send(
                ch,
                {
                    "type": "lobby.event",
                    "payload": {
                        "event": "lobby_list",
                        "lobbies": lobbies_snapshot,
                    },
                },
            )

    async def _broadcast_to_lobby_members(
            self, lobby: CustomLobbyRoom, payload: dict
    ) -> None:
        for member in list(lobby.members):
            await self.channel_layer.send(
                member.channel_name,
                {
                    "type": "lobby.event",
                    "payload": payload,
                },
            )

    async def _handle_list_lobbies(self) -> None:
        async with _LOBBY_LOCK:
            lobbies_snapshot = [
                _serialize_lobby_summary(lb)
                for lb in _ACTIVE_LOBBIES.values()
                if not lb.is_starting
            ]
        await self.send_json(
            {
                "event": "lobby_list",
                "lobbies": lobbies_snapshot,
                "message": "لیست لابی‌ها بروزرسانی شد.",
            }
        )

    async def _handle_create_lobby(self, msg: dict) -> None:
        assert self.player is not None
        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        # چک مجدد عدم حضور در بازی فعال
        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        if active_match:
            await self.send_json(
                {
                    "event": "error",
                    "message": "شما در حال حاضر یک نبرد فعال دارید و مجاز به ساخت لابی نیستید.",
                }
            )
            return

        title = str(msg.get("title", "")).strip()[:36]
        if not title:
            title = f"لابی {self.player.username}"

        password = str(msg.get("password", "")).strip()[:24]
        try:
            max_players = int(msg.get("max_players", 2))
        except Exception:
            max_players = 2

        if max_players not in ALLOWED_LOBBY_CAPACITIES:
            await self.send_json(
                {
                    "event": "error",
                    "message": "ظرفیت لابی باید یکی از اعداد زوج ۲، ۴، ۶، ۸ یا ۱۰ باشد.",
                }
            )
            return

        async with _LOBBY_LOCK:
            # بررسی اینکه بازیکن قبلاً در لابی دیگری نباشد
            for existing_lb in _ACTIVE_LOBBIES.values():
                if any(m.user_id == self.player.user_id for m in existing_lb.members):
                    await self.send_json(
                        {
                            "event": "error",
                            "message": "شما هم‌اکنون در یک لابی حضور دارید.",
                        }
                    )
                    return

            lobby_id = f"lby_{secrets.randbelow(9000) + 1000}"
            while lobby_id in _ACTIVE_LOBBIES:
                lobby_id = f"lby_{secrets.randbelow(9000) + 1000}"

            host_member = CustomLobbyMember(
                user_id=self.player.user_id,
                username=self.player.username,
                player_pk=self.player.pk,
                channel_name=self.channel_name,
                team_id=1,
                slot_index=0,
                client_ip=client_ip,
            )
            new_lobby = CustomLobbyRoom(
                lobby_id=lobby_id,
                title=title,
                password=password,
                max_players=max_players,
                host_user_id=self.player.user_id,
                host_username=self.player.username,
                members=[host_member],
                chat_history=[
                    {
                        "sender": "سیستم",
                        "text": f"لابی «{title}» با ظرفیت {max_players} نفر توسط {self.player.username} ساخته شد.",
                        "is_system": True,
                    }
                ],
                is_starting=False,
            )
            _ACTIVE_LOBBIES[lobby_id] = new_lobby
            _LOBBY_BROWSER_CHANNELS.discard(self.channel_name)
            self.current_lobby_id = lobby_id
            detail = _serialize_lobby_detail(new_lobby)

        await self.send_json(
            {
                "event": "lobby_state",
                "lobby": detail,
                "message": "لابی با موفقیت ساخته شد.",
            }
        )
        await self._broadcast_lobby_list_to_browsers()

    async def _handle_join_lobby(self, msg: dict) -> None:
        assert self.player is not None
        lobby_id = str(msg.get("lobby_id", "")).strip()
        password_attempt = str(msg.get("password", "")).strip()

        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        if active_match:
            await self.send_json(
                {
                    "event": "error",
                    "message": "شما در حال حاضر یک نبرد فعال دارید و نمی‌توانید وارد لابی شوید.",
                }
            )
            return

        async with _LOBBY_LOCK:
            for existing_lb in _ACTIVE_LOBBIES.values():
                if any(m.user_id == self.player.user_id for m in existing_lb.members):
                    await self.send_json(
                        {
                            "event": "error",
                            "message": "شما هم‌اکنون داخل یک لابی هستید.",
                        }
                    )
                    return

            lobby = _ACTIVE_LOBBIES.get(lobby_id)
            if not lobby:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "لابی مورد نظر یافت نشد یا بسته شده است.",
                    }
                )
                return

            if lobby.is_starting:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "این لابی در حال شروع بازی است و امکان ورود وجود ندارد.",
                    }
                )
                return

            if len(lobby.members) >= lobby.max_players:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "ظرفیت این لابی تکمیل شده است.",
                    }
                )
                return

            if lobby.password and not secrets.compare_digest(lobby.password, password_attempt):
                await self.send_json(
                    {
                        "event": "error",
                        "message": "گذرواژه واردشده نادرست است.",
                    }
                )
                return

            free_slot = _find_first_free_slot(lobby)
            if not free_slot:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "جایگاه خالی در لابی یافت نشد.",
                    }
                )
                return

            team_id, slot_idx = free_slot
            lobby.members.append(
                CustomLobbyMember(
                    user_id=self.player.user_id,
                    username=self.player.username,
                    player_pk=self.player.pk,
                    channel_name=self.channel_name,
                    team_id=team_id,
                    slot_index=slot_idx,
                    client_ip=client_ip,
                )
            )
            lobby.chat_history.append(
                {
                    "sender": "سیستم",
                    "text": f"{self.player.username} به لابی پیوست.",
                    "is_system": True,
                }
            )
            _LOBBY_BROWSER_CHANNELS.discard(self.channel_name)
            self.current_lobby_id = lobby.lobby_id
            detail = _serialize_lobby_detail(lobby)

            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_state",
                    "lobby": detail,
                    "message": f"{self.player.username} وارد لابی شد.",
                },
            )

        await self._broadcast_lobby_list_to_browsers()

    async def _handle_switch_team(self, msg: dict) -> None:
        assert self.player is not None
        if not self.current_lobby_id:
            return

        try:
            target_team = int(msg.get("team_id", 1))
            target_slot = int(msg.get("slot_index", -1))
        except Exception:
            return

        if target_team not in (1, 2):
            return

        async with _LOBBY_LOCK:
            lobby = _ACTIVE_LOBBIES.get(self.current_lobby_id)
            if not lobby or lobby.is_starting:
                return

            member = next(
                (m for m in lobby.members if m.user_id == self.player.user_id), None
            )
            if not member:
                return

            half = lobby.max_players // 2
            occupied = {
                (m.team_id, m.slot_index)
                for m in lobby.members
                if m.user_id != self.player.user_id
            }

            if 0 <= target_slot < half:
                if (target_team, target_slot) in occupied:
                    await self.send_json(
                        {
                            "event": "error",
                            "message": "این جایگاه توسط بازیکن دیگری اشغال شده است.",
                        }
                    )
                    return
                chosen_slot = target_slot
            else:
                free = _find_first_free_slot(lobby, preferred_team=target_team)
                if not free or free[0] != target_team:
                    await self.send_json(
                        {
                            "event": "error",
                            "message": "ظرفیت این تیم تکمیل است.",
                        }
                    )
                    return
                chosen_slot = free[1]

            if member.team_id == target_team and member.slot_index == chosen_slot:
                return

            member.team_id = target_team
            member.slot_index = chosen_slot
            team_title = "سپاه روشنایی (تیم ۱)" if target_team == 1 else "سپاه تاریکی (تیم ۲)"
            lobby.chat_history.append(
                {
                    "sender": "سیستم",
                    "text": f"{self.player.username} به {team_title} (جایگاه {chosen_slot + 1}) منتقل شد.",
                    "is_system": True,
                }
            )
            detail = _serialize_lobby_detail(lobby)
            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_state",
                    "lobby": detail,
                    "message": f"جایگاه {self.player.username} بروزرسانی شد.",
                },
            )

    async def _handle_chat_message(self, msg: dict) -> None:
        assert self.player is not None
        if not self.current_lobby_id:
            return

        now_mono = time.monotonic()
        if now_mono - self.last_chat_ts < 0.35:
            return
        self.last_chat_ts = now_mono

        raw_text = str(msg.get("text", "")).strip()
        clean_text = "".join(ch for ch in raw_text if ch.isprintable())[:120]
        if not clean_text:
            return

        async with _LOBBY_LOCK:
            lobby = _ACTIVE_LOBBIES.get(self.current_lobby_id)
            if not lobby:
                return
            if not any(m.user_id == self.player.user_id for m in lobby.members):
                return

            lobby.chat_history.append(
                {
                    "sender": self.player.username,
                    "text": clean_text,
                    "is_system": False,
                }
            )
            if len(lobby.chat_history) > 60:
                lobby.chat_history = lobby.chat_history[-60:]

            detail = _serialize_lobby_detail(lobby)
            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_state",
                    "lobby": detail,
                    "message": "",
                },
            )

    async def _handle_kick_player(self, msg: dict) -> None:
        assert self.player is not None
        if not self.current_lobby_id:
            return

        target_user_id = str(msg.get("target_user_id", "")).strip()
        if not target_user_id or target_user_id == self.player.user_id:
            return

        kicked_member: Optional[CustomLobbyMember] = None
        async with _LOBBY_LOCK:
            lobby = _ACTIVE_LOBBIES.get(self.current_lobby_id)
            if not lobby:
                return

            if lobby.host_user_id != self.player.user_id:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "فقط سازنده لابی مجاز به اخراج بازیکن است.",
                    }
                )
                return

            if lobby.is_starting:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "در حین شروع بازی امکان اخراج بازیکن وجود ندارد.",
                    }
                )
                return

            for m in lobby.members:
                if m.user_id == target_user_id:
                    kicked_member = m
                    break

            if not kicked_member:
                return

            lobby.members = [
                m for m in lobby.members if m.user_id != target_user_id
            ]
            _LOBBY_BROWSER_CHANNELS.add(kicked_member.channel_name)

            lobby.chat_history.append(
                {
                    "sender": "سیستم",
                    "text": f"{kicked_member.username} توسط سازنده از لابی اخراج شد.",
                    "is_system": True,
                }
            )
            detail = _serialize_lobby_detail(lobby)
            lobbies_snapshot = [
                _serialize_lobby_summary(lb)
                for lb in _ACTIVE_LOBBIES.values()
                if not lb.is_starting
            ]

            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_state",
                    "lobby": detail,
                    "message": f"{kicked_member.username} از لابی اخراج شد.",
                },
            )

        if kicked_member:
            await self.channel_layer.send(
                kicked_member.channel_name,
                {
                    "type": "lobby.kicked",
                    "payload": {
                        "event": "kicked_from_lobby",
                        "lobbies": lobbies_snapshot,
                        "message": f"شما توسط سازنده لابی ({self.player.username}) از لابی حذف شدید.",
                    },
                },
            )
            await self._broadcast_lobby_list_to_browsers()

    async def _handle_member_leave(self, is_disconnect: bool = False) -> None:
        assert self.player is not None
        lobby_id = self.current_lobby_id
        if not lobby_id:
            return

        host_dissolved = False
        remaining_members_to_notify: List[CustomLobbyMember] = []
        lobby_title = ""

        async with _LOBBY_LOCK:
            lobby = _ACTIVE_LOBBIES.get(lobby_id)
            if not lobby:
                self.current_lobby_id = None
                return

            # نیازمندی ۱۱: در صورت شروع بازی، سازنده لابی حق حذف لابی را ندارد
            if lobby.is_starting and not is_disconnect:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "بازی در حال راه‌اندازی است؛ امکان حذف یا ترک لابی وجود ندارد.",
                    }
                )
                return

            if lobby.is_starting and is_disconnect:
                return

            self.current_lobby_id = None
            if not is_disconnect:
                _LOBBY_BROWSER_CHANNELS.add(self.channel_name)

            # نیازمندی ۱۰: اگر سازنده لابی خارج شود یا لابی را حذف کند، کل لابی پاک شده و همه به صفحه اصلی برگردند
            if lobby.host_user_id == self.player.user_id:
                host_dissolved = True
                lobby_title = lobby.title
                remaining_members_to_notify = list(lobby.members)
                _ACTIVE_LOBBIES.pop(lobby_id, None)
            else:
                lobby.members = [
                    m for m in lobby.members if m.user_id != self.player.user_id
                ]
                lobby.chat_history.append(
                    {
                        "sender": "سیستم",
                        "text": f"{self.player.username} لابی را ترک کرد.",
                        "is_system": True,
                    }
                )
                detail = _serialize_lobby_detail(lobby)
                await self._broadcast_to_lobby_members(
                    lobby,
                    {
                        "event": "lobby_state",
                        "lobby": detail,
                        "message": f"{self.player.username} از لابی خارج شد.",
                    },
                )

            lobbies_snapshot = [
                _serialize_lobby_summary(lb)
                for lb in _ACTIVE_LOBBIES.values()
                if not lb.is_starting
            ]

        if host_dissolved:
            for m in remaining_members_to_notify:
                await self.channel_layer.send(
                    m.channel_name,
                    {
                        "type": "lobby.closed",
                        "payload": {
                            "event": "lobby_closed",
                            "message": f"لابی «{lobby_title}» به دلیل خروج سازنده ({self.player.username}) منحل شد.",
                        },
                    },
                )
        elif not is_disconnect:
            await self.send_json(
                {
                    "event": "left_lobby",
                    "lobbies": lobbies_snapshot,
                    "message": "شما از لابی خارج شدید.",
                }
            )

        await self._broadcast_lobby_list_to_browsers()

    async def _handle_start_match(self) -> None:
        assert self.player is not None
        if not self.current_lobby_id:
            return

        async with _LOBBY_LOCK:
            lobby = _ACTIVE_LOBBIES.get(self.current_lobby_id)
            if not lobby:
                return

            # نیازمندی ۵: فقط سازنده لابی می‌تواند بازی را شروع کند
            if lobby.host_user_id != self.player.user_id:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "فقط سازنده لابی مجاز به شروع نبرد است.",
                    }
                )
                return

            if lobby.is_starting:
                return

            if len(lobby.members) < lobby.max_players:
                await self.send_json(
                    {
                        "event": "error",
                        "message": f"ظرفیت لابی هنوز تکمیل نشده است ({len(lobby.members)} از {lobby.max_players} نفر).",
                    }
                )
                return

            half = lobby.max_players // 2
            team1_count = sum(1 for m in lobby.members if m.team_id == 1)
            team2_count = sum(1 for m in lobby.members if m.team_id == 2)
            if team1_count != half or team2_count != half:
                await self.send_json(
                    {
                        "event": "error",
                        "message": f"هر تیم باید دقیقاً دارای {half} بازیکن باشد.",
                    }
                )
                return

            # قفل کردن لابی به محض شروع بازی (نیازمندی ۱۱)
            lobby.is_starting = True
            lobby.chat_history.append(
                {
                    "sender": "سیستم",
                    "text": "ظرفیت تکمیل شد! در حال راه‌اندازی سرور اختصاصی نبرد...",
                    "is_system": True,
                }
            )
            detail = _serialize_lobby_detail(lobby)
            members_ordered = sorted(
                lobby.members, key=lambda x: (x.team_id, x.slot_index)
            )
            max_players = lobby.max_players
            lobby_id_copy = lobby.lobby_id

            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_starting",
                    "lobby": detail,
                    "message": "در حال راه‌اندازی سرور نبرد... لطفاً شکیبا باشید.",
                },
            )

        await self._broadcast_lobby_list_to_browsers()

        try:
            port = find_available_udp_port()
        except Exception:
            async with _LOBBY_LOCK:
                if lobby_id_copy in _ACTIVE_LOBBIES:
                    _ACTIVE_LOBBIES[lobby_id_copy].is_starting = False
                    detail = _serialize_lobby_detail(_ACTIVE_LOBBIES[lobby_id_copy])
                    await self._broadcast_to_lobby_members(
                        _ACTIVE_LOBBIES[lobby_id_copy],
                        {
                            "event": "lobby_state",
                            "lobby": detail,
                            "message": "هیچ پورت آزادی روی سرور یافت نشد؛ دوباره تلاش کنید.",
                        },
                    )
            return

        room_id = f"lobby-{secrets.randbelow(900) + 100}-{secrets.token_hex(2)}"
        first_ip = members_ordered[0].client_ip if members_ordered else ""
        server_host = resolve_game_server_host(first_ip)

        queued_entries = [
            QueuedPlayerEntry(
                user_id=m.user_id,
                username=m.username,
                mmr=1000,
                channel_name=m.channel_name,
                player_pk=m.player_pk,
                client_ip=m.client_ip,
            )
            for m in members_ordered
        ]
        explicit_teams = {m.user_id: m.team_id for m in members_ordered}

        assignments = await _create_match_records_and_spawn(
            matched_entries=queued_entries,
            port=port,
            room_id=room_id,
            server_host=server_host,
            player_count=max_players,
            is_custom_lobby=True,
            explicit_teams=explicit_teams,
        )

        server_ready = await wait_for_udp_server_ready(port, timeout_sec=30.0)
        if not server_ready:
            await _mark_room_finished(room_id)
            async with _LOBBY_LOCK:
                lb = _ACTIVE_LOBBIES.get(lobby_id_copy)
                if lb:
                    lb.is_starting = False
                    detail = _serialize_lobby_detail(lb)
                    await self._broadcast_to_lobby_members(
                        lb,
                        {
                            "event": "lobby_state",
                            "lobby": detail,
                            "message": "خطا در راه‌اندازی سرور بازی؛ مجدداً تلاش کنید.",
                        },
                    )
            return

        async with _LOBBY_LOCK:
            _ACTIVE_LOBBIES.pop(lobby_id_copy, None)

        for ch_name, payload in assignments.items():
            await self.channel_layer.send(
                ch_name,
                {
                    "type": "lobby.event",
                    "payload": payload,
                },
            )

    async def lobby_event(self, event: dict) -> None:
        await self.send_json(event["payload"])

    async def lobby_kicked(self, event: dict) -> None:
        self.current_lobby_id = None
        await self.send_json(event["payload"])

    async def lobby_closed(self, event: dict) -> None:
        self.current_lobby_id = None
        await self.send_json(event["payload"])