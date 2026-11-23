import asyncio
import contextvars
import json
import os
import secrets
import signal
import socket
import subprocess
import threading
import time
from dataclasses import asdict, dataclass
from datetime import timedelta
from typing import Any, Callable, Dict, List, Optional
from urllib.parse import parse_qs

import redis
from asgiref.sync import async_to_sync
from channels.db import database_sync_to_async
from channels.layers import get_channel_layer
from django.db.models import Q
from channels.generic.websocket import AsyncWebsocketConsumer
from django.conf import settings
from django.db import transaction
from django.utils import timezone

from .models import (
    Friendship,
    GameMatchRoom,
    MatchParticipant,
    Player,
    PlayerSessionToken,
    compute_rank_tier_from_mmr,
)


_REDIS_CLIENT_LOCK = threading.Lock()
_REDIS_CLIENT_CACHE: Optional[redis.Redis] = None


def _get_shared_redis() -> redis.Redis:
    global _REDIS_CLIENT_CACHE
    with _REDIS_CLIENT_LOCK:
        if _REDIS_CLIENT_CACHE is not None:
            try:
                _REDIS_CLIENT_CACHE.ping()
                return _REDIS_CLIENT_CACHE
            except Exception:
                _REDIS_CLIENT_CACHE = None

        client = redis.Redis.from_url(
            settings.REDIS_URL,
            decode_responses=True,
            socket_connect_timeout=2.0,
            socket_timeout=2.0,
        )
        client.ping()
        _REDIS_CLIENT_CACHE = client
        return client


class _RedisHashStore:
    """
    ذخیره‌سازی توزیع‌شده ساختارها در Redis Hash با کش تراکنشی در محدوده هر Lock
    تا هر تغییری روی آبجکت‌ها در پایان قفل به‌صورت خودکار در Redis ذخیره شود.
    """

    def __init__(self, redis_key: str, from_dict: Callable[[dict], Any]):
        self.redis_key = redis_key
        self._from_dict = from_dict
        self._tx_var: contextvars.ContextVar[Optional[Dict[str, Any]]] = contextvars.ContextVar(
            f"tx_{redis_key}", default=None
        )
        self._deleted_var: contextvars.ContextVar[Optional[set]] = contextvars.ContextVar(
            f"del_{redis_key}", default=None
        )

    async def begin_tx_async(self) -> tuple:
        t1 = self._tx_var.set({})
        t2 = self._deleted_var.set(set())
        return (t1, t2)

    async def commit_and_end_tx_async(self, tokens: tuple) -> None:
        cache = self._tx_var.get()
        deleted = self._deleted_var.get()
        self._tx_var.reset(tokens[0])
        self._deleted_var.reset(tokens[1])
        del_list = list(deleted) if deleted else []
        mapping = {k: json.dumps(asdict(v)) for k, v in cache.items()} if cache else {}
        if not del_list and not mapping:
            return

        def _flush():
            r = _get_shared_redis()
            if del_list:
                r.hdel(self.redis_key, *del_list)
            if mapping:
                r.hset(self.redis_key, mapping=mapping)

        try:
            await asyncio.to_thread(_flush)
        except Exception as exc:
            print(f"[REDIS STORE ERROR] {self.redis_key}: {exc}", flush=True)

    def _load_one(self, key: str) -> Any:
        raw = _get_shared_redis().hget(self.redis_key, key)
        if not raw:
            return None
        return self._from_dict(json.loads(raw))

    def get(self, key: str, default: Any = None) -> Any:
        cache = self._tx_var.get()
        deleted = self._deleted_var.get()
        if deleted is not None and key in deleted:
            return default
        if cache is not None and key in cache:
            return cache[key]
        obj = self._load_one(key)
        if obj is None:
            return default
        if cache is not None:
            cache[key] = obj
        return obj

    def mget(self, keys: List[str]) -> Dict[str, Any]:
        if not keys:
            return {}
        raw_vals = _get_shared_redis().hmget(self.redis_key, keys)
        out: Dict[str, Any] = {}
        for k, raw in zip(keys, raw_vals):
            if raw:
                out[k] = self._from_dict(json.loads(raw))
        return out

    def __getitem__(self, key: str) -> Any:
        obj = self.get(key)
        if obj is None:
            raise KeyError(key)
        return obj

    def __setitem__(self, key: str, value: Any) -> None:
        cache = self._tx_var.get()
        deleted = self._deleted_var.get()
        if cache is not None:
            cache[key] = value
            if deleted is not None:
                deleted.discard(key)
        else:
            _get_shared_redis().hset(self.redis_key, key, json.dumps(asdict(value)))

    def pop(self, key: str, default: Any = None) -> Any:
        cache = self._tx_var.get()
        deleted = self._deleted_var.get()
        val = self.get(key, default)
        if cache is not None:
            cache.pop(key, None)
            if deleted is not None:
                deleted.add(key)
        else:
            _get_shared_redis().hdel(self.redis_key, key)
        return val

    def __contains__(self, key: str) -> bool:
        return self.get(key) is not None

    def values(self) -> List[Any]:
        return [v for _, v in self.items()]

    def items(self) -> List[tuple]:
        cache = self._tx_var.get()
        deleted = self._deleted_var.get() or set()
        raw_map = _get_shared_redis().hgetall(self.redis_key)
        out: List[tuple] = []
        seen = set()
        for k, raw in raw_map.items():
            if k in deleted:
                continue
            seen.add(k)
            if cache is not None and k in cache:
                out.append((k, cache[k]))
            else:
                obj = self._from_dict(json.loads(raw))
                if cache is not None:
                    cache[k] = obj
                out.append((k, obj))
        if cache is not None:
            for k, v in cache.items():
                if k not in seen and k not in deleted:
                    out.append((k, v))
        return out


class _RedisChannelSet:
    """
    مجموعه کانال‌های مرورگر لابی در Redis Set برای پخش همگانی بین تمام Instanceها
    """

    def __init__(self, redis_key: str = "simorgh:lobby_browsers"):
        self.redis_key = redis_key

    def add(self, channel_name: str) -> None:
        if channel_name:
            _get_shared_redis().sadd(self.redis_key, channel_name)

    def discard(self, channel_name: str) -> None:
        if channel_name:
            _get_shared_redis().srem(self.redis_key, channel_name)

    def __iter__(self):
        return iter(_get_shared_redis().smembers(self.redis_key))


class _RedisQueueList:
    """
    صف توزیع‌شده Matchmaking در Redis (ذخیره‌شده زیر کلید JSON با همگام‌سازی در Lock)
    """

    def __init__(self, redis_key: str, from_dict: Callable[[dict], Any]):
        self.redis_key = redis_key
        self._from_dict = from_dict
        self._tx_var: contextvars.ContextVar[Optional[List[Any]]] = contextvars.ContextVar(
            f"tx_q_{redis_key}", default=None
        )

    async def begin_tx_async(self) -> contextvars.Token:
        def _load():
            raw = _get_shared_redis().get(self.redis_key)
            return [self._from_dict(d) for d in json.loads(raw)] if raw else []

        items = await asyncio.to_thread(_load)
        return self._tx_var.set(items)

    async def commit_and_end_tx_async(self, token: contextvars.Token) -> None:
        items = self._tx_var.get()
        self._tx_var.reset(token)
        if items is None:
            return
        payload = json.dumps([asdict(x) for x in items])
        try:
            await asyncio.to_thread(lambda: _get_shared_redis().set(self.redis_key, payload))
        except Exception as exc:
            print(f"[REDIS QUEUE ERROR] {self.redis_key}: {exc}", flush=True)

    def _items(self) -> List[Any]:
        items = self._tx_var.get()
        if items is not None:
            return items
        raw = _get_shared_redis().get(self.redis_key)
        return [self._from_dict(d) for d in json.loads(raw)] if raw else []

    def __len__(self) -> int:
        return len(self._items())

    def __iter__(self):
        return iter(self._items())

    def __getitem__(self, idx):
        return self._items()[idx]

    def __setitem__(self, idx, value) -> None:
        items = self._tx_var.get()
        if items is not None:
            items[idx] = value
        else:
            cur = self._items()
            cur[idx] = value
            _get_shared_redis().set(self.redis_key, json.dumps([asdict(x) for x in cur]))

    def append(self, value: Any) -> None:
        items = self._tx_var.get()
        if items is not None:
            items.append(value)
        else:
            cur = self._items()
            cur.append(value)
            _get_shared_redis().set(self.redis_key, json.dumps([asdict(x) for x in cur]))


class _RedisPortDict:
    """
    نگهداری توزیع‌شده پورت‌های رزروشده سرور بازی در Redis Hash (port -> unit_name)
    """

    def __init__(self, redis_key: str = "simorgh:active_ports"):
        self.redis_key = redis_key

    def get(self, port: int, default: Any = None) -> Any:
        val = _get_shared_redis().hget(self.redis_key, str(port))
        return val if val is not None else default

    def __getitem__(self, port: int) -> str:
        val = _get_shared_redis().hget(self.redis_key, str(port))
        if val is None:
            raise KeyError(port)
        return val

    def __setitem__(self, port: int, unit_name: str) -> None:
        _get_shared_redis().hset(self.redis_key, str(port), unit_name)

    def __delitem__(self, port: int) -> None:
        removed = _get_shared_redis().hdel(self.redis_key, str(port))
        if not removed:
            raise KeyError(port)

    def __len__(self) -> int:
        return int(_get_shared_redis().hlen(self.redis_key))

    def keys(self) -> List[int]:
        return [int(k) for k in _get_shared_redis().hkeys(self.redis_key)]

    def values(self) -> List[str]:
        return list(_get_shared_redis().hvals(self.redis_key))

    def __contains__(self, port: object) -> bool:
        try:
            return bool(_get_shared_redis().hexists(self.redis_key, str(int(port))))  # type: ignore[arg-type]
        except Exception:
            return False

    def pop(self, port: int, default: Any = None) -> Any:
        r = _get_shared_redis()
        key_s = str(port)
        val = r.hget(self.redis_key, key_s)
        if val is None:
            return default
        r.hdel(self.redis_key, key_s)
        return val

    def items(self) -> List[tuple]:
        raw_map = _get_shared_redis().hgetall(self.redis_key)
        return [(int(k), v) for k, v in raw_map.items()]


class _RedisLockManager:
    """
    قفل توزیع‌شده غیرمسدودکننده Redis بین تمام پروسه‌ها و سرورهای بک‌اند + همگام‌سازی خودکار استورهای داخل قفل
    """

    _RELEASE_LUA = (
        "if redis.call('get', KEYS[1]) == ARGV[1] then "
        "return redis.call('del', KEYS[1]) else return 0 end"
    )

    def __init__(self, lock_key: str, stores: List[Any], ttl_ms: int = 8000):
        self.lock_key = lock_key
        self.stores = stores
        self.ttl_ms = ttl_ms
        self._token_var: contextvars.ContextVar[Optional[str]] = contextvars.ContextVar(
            f"lock_tok_{lock_key}", default=None
        )
        self._store_tokens_var: contextvars.ContextVar[Optional[List[tuple]]] = contextvars.ContextVar(
            f"lock_st_{lock_key}", default=None
        )

    def _acquire_sync(self, token: str) -> bool:
        return bool(_get_shared_redis().set(self.lock_key, token, nx=True, px=self.ttl_ms))

    def _release_sync(self, token: str) -> None:
        try:
            _get_shared_redis().eval(self._RELEASE_LUA, 1, self.lock_key, token)
        except Exception:
            pass

    async def __aenter__(self):
        token = secrets.token_hex(12)
        deadline = time.monotonic() + 10.0
        while time.monotonic() < deadline:
            if await asyncio.to_thread(self._acquire_sync, token):
                break
            await asyncio.sleep(0.008)
        self._token_var.set(token)
        st_tokens = []
        for st in self.stores:
            st_tokens.append((st, await st.begin_tx_async()))
        self._store_tokens_var.set(st_tokens)
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        st_tokens = self._store_tokens_var.get() or []
        for st, tok in reversed(st_tokens):
            await st.commit_and_end_tx_async(tok)
        self._store_tokens_var.set(None)
        token = self._token_var.get()
        self._token_var.set(None)
        if token:
            await asyncio.to_thread(self._release_sync, token)


@dataclass
class QueuedPlayerEntry:
    user_id: str
    username: str
    mmr: int
    channel_name: str
    player_pk: int
    client_ip: str = ""
    party_group_id: str = ""

    @classmethod
    def from_dict(cls, d: dict) -> "QueuedPlayerEntry":
        return cls(
            user_id=str(d.get("user_id", "")),
            username=str(d.get("username", "")),
            mmr=int(d.get("mmr", 1000)),
            channel_name=str(d.get("channel_name", "")),
            player_pk=int(d.get("player_pk", 0)),
            client_ip=str(d.get("client_ip", "")),
            party_group_id=str(d.get("party_group_id", "")),
        )


# صف جهانی Matchmaking و قفل همگام‌سازی توزیع‌شده در Redis
_WAITING_QUEUE = _RedisQueueList("simorgh:mm_queue", QueuedPlayerEntry.from_dict)
_QUEUE_LOCK = _RedisLockManager("simorgh:lock:mm_queue", [_WAITING_QUEUE])

# نگهداری نام سرویس systemd فعال روی هر پورت: port -> "moba-server@4433_room-101_2.service"
_ACTIVE_SERVICE_UNITS = _RedisPortDict("simorgh:active_ports")
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

    now = timezone.now()
    for port in dead_ports:
        _ACTIVE_SERVICE_UNITS.pop(port, None)
        dead_room_ids = list(
            GameMatchRoom.objects.filter(
                server_port=port, status=GameMatchRoom.STATUS_RUNNING
            ).values_list("id", flat=True)
        )
        if dead_room_ids:
            GameMatchRoom.objects.filter(id__in=dead_room_ids).update(
                status=GameMatchRoom.STATUS_FINISHED,
                finished_at=now,
            )
            MatchParticipant.objects.filter(
                match_room_id__in=dead_room_ids,
                status__in=[
                    MatchParticipant.STATUS_ASSIGNED,
                    MatchParticipant.STATUS_CONNECTED,
                    MatchParticipant.STATUS_DISCONNECTED,
                ],
            ).update(status=MatchParticipant.STATUS_COMPLETED, left_at=now)


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


_find_available_udp_port_async = database_sync_to_async(find_available_udp_port)


def resolve_game_server_host(client_ip: Optional[str] = None) -> str:
    """
    تعیین آدرس IP سرور بازی از روی متغیر GAME_SERVER_HOST در settings.py.
    """
    return str(settings.GAME_SERVER_HOST).strip()


def store_ticket_in_redis(ticket_id: str, room_id: str, user_id: str, username: str) -> None:
    """
    ذخیره تیکت بازیکن در Redis با فرمت مورد انتظار سرور بازی Rust:
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
    try:
        _get_shared_redis().setex(f"ticket:{ticket_id}", 7200, payload)
    except Exception as exc:
        print(f"[REDIS ERROR] Failed to store ticket ticket:{ticket_id}: {exc}", flush=True)


def delete_ticket_from_redis(ticket_id: str) -> None:
    """
    حذف تیکت بازیکن از Redis پس از ترک نهایی بازی (Abandon) یا اتمام مسابقه
    """
    try:
        _get_shared_redis().delete(f"ticket:{ticket_id}")
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
        if await asyncio.to_thread(is_systemd_unit_failed, unit_name):
            print(
                f"[MATCHMAKING ERROR] Systemd service {unit_name} entered failed state!",
                flush=True,
            )
            _ACTIVE_SERVICE_UNITS.pop(port, None)
            return False

        if await asyncio.to_thread(is_udp_port_bound, port):
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
        if room.finished_at is not None or room.winner_team in (1, 2):
            if room.status == GameMatchRoom.STATUS_RUNNING:
                room.status = GameMatchRoom.STATUS_FINISHED
                room.finished_at = room.finished_at or now
                room.save(update_fields=["status", "finished_at"])
            if part.status not in (
                MatchParticipant.STATUS_ABANDONED,
                MatchParticipant.STATUS_COMPLETED,
            ):
                part.status = MatchParticipant.STATUS_COMPLETED
                part.left_at = part.left_at or now
                part.save(update_fields=["status", "left_at"])
            continue
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
            # سرور بازی دیگر روی این پورت فعال نیست؛ وضعیت اتاق و بازیکنان را در دیتابیس پایان یافته ثبت کن
            room.status = GameMatchRoom.STATUS_FINISHED
            room.finished_at = now
            room.save(update_fields=["status", "finished_at"])
            MatchParticipant.objects.filter(
                match_room=room,
                status__in=[
                    MatchParticipant.STATUS_ASSIGNED,
                    MatchParticipant.STATUS_CONNECTED,
                    MatchParticipant.STATUS_DISCONNECTED,
                ],
            ).update(status=MatchParticipant.STATUS_COMPLETED, left_at=now)

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

    # برگرداندن وضعیت آنلاین بازیکنان از in_match به online در Redis
    try:
        for part in participants:
            usess = _ONLINE_USERS.get(part.player.user_id)
            if usess and usess.status == "in_match":
                usess.status = "online"
                _ONLINE_USERS[part.player.user_id] = usess
    except Exception:
        pass

    # زمان‌بندی توقف سرویس systemd و Kill پروسه لوپ بازی پس از ۱۰.۵ ثانیه (متناسب با افزایش ۴ ثانیه‌ای شمارش معکوس پایان بازی)
    if not already_finished:
        _schedule_server_termination(
            port=room.server_port,
            room_id=room.room_id,
            player_count=room.max_players,
            process_pid=room.process_pid,
            delay_sec=10.5,
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
            party_group_id=entry.party_group_id,
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
            "team_id": team_id,
            "slot_index": idx,
            "game_link": game_link,
        }

    pid = spawn_game_server_process(port, room_id, player_count)
    if pid is not None:
        match_room.process_pid = pid
        match_room.save(update_fields=["process_pid"])

    return assignments


def _pop_matched_batch_preserving_parties(
    queue: List[QueuedPlayerEntry], required: int
) -> Optional[List[QueuedPlayerEntry]]:
    """
    انتخاب دقیقاً `required` بازیکن از صف بدون نصف‌کردن هیچ گروه پارتی (party_group_id).
    """
    if len(queue) < required:
        return None

    groups_order: List[str] = []
    groups_map: Dict[str, List[QueuedPlayerEntry]] = {}
    for entry in queue:
        key = entry.party_group_id if entry.party_group_id else f"solo:{entry.user_id}"
        if key not in groups_map:
            groups_order.append(key)
            groups_map[key] = []
        groups_map[key].append(entry)

    chosen_keys: List[str] = []

    def dfs(start_idx: int, remaining: int) -> bool:
        if remaining == 0:
            return True
        for i in range(start_idx, len(groups_order)):
            gkey = groups_order[i]
            gsize = len(groups_map[gkey])
            if gsize <= remaining:
                chosen_keys.append(gkey)
                if dfs(i + 1, remaining - gsize):
                    return True
                chosen_keys.pop()
        return False

    if not dfs(0, required):
        return None

    chosen_set = set(chosen_keys)
    matched: List[QueuedPlayerEntry] = []
    remaining_queue: List[QueuedPlayerEntry] = []
    for entry in queue:
        key = entry.party_group_id if entry.party_group_id else f"solo:{entry.user_id}"
        if key in chosen_set:
            matched.append(entry)
        else:
            remaining_queue.append(entry)
    queue[:] = remaining_queue
    return matched


@database_sync_to_async
def _find_party_member_in_active_match(player_pks: List[int]) -> Optional[str]:
    part = (
        MatchParticipant.objects.select_related("player", "match_room")
        .filter(
            player_id__in=player_pks,
            match_room__status=GameMatchRoom.STATUS_RUNNING,
            status__in=[
                MatchParticipant.STATUS_CONNECTED,
                MatchParticipant.STATUS_DISCONNECTED,
            ],
        )
        .first()
    )
    return part.player.username if part else None


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
                        "message": "You already have an active match and cannot join a new queue.",
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

            matched_batch = _pop_matched_batch_preserving_parties(_WAITING_QUEUE, required)
            if matched_batch is None:
                queue_snapshot = list(_WAITING_QUEUE)

        if matched_batch is not None:
            try:
                port = await _find_available_udp_port_async()
            except Exception:
                for entry in matched_batch:
                    await self.channel_layer.send(
                        entry.channel_name,
                        {
                            "type": "queue.status",
                            "payload": {
                                "event": "error",
                                "message": "No free game server port available; please try again.",
                            },
                        },
                    )
                return
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
                                "message": "Failed to start the game server; please try again.",
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

    @classmethod
    def from_dict(cls, d: dict) -> "CustomLobbyMember":
        return cls(
            user_id=str(d.get("user_id", "")),
            username=str(d.get("username", "")),
            player_pk=int(d.get("player_pk", 0)),
            channel_name=str(d.get("channel_name", "")),
            team_id=int(d.get("team_id", 1)),
            slot_index=int(d.get("slot_index", 0)),
            client_ip=str(d.get("client_ip", "")),
        )


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

    @classmethod
    def from_dict(cls, d: dict) -> "CustomLobbyRoom":
        return cls(
            lobby_id=str(d.get("lobby_id", "")),
            title=str(d.get("title", "")),
            password=str(d.get("password", "")),
            max_players=int(d.get("max_players", 2)),
            host_user_id=str(d.get("host_user_id", "")),
            host_username=str(d.get("host_username", "")),
            members=[CustomLobbyMember.from_dict(m) for m in d.get("members", [])],
            chat_history=list(d.get("chat_history", [])),
            is_starting=bool(d.get("is_starting", False)),
        )


_ACTIVE_LOBBIES = _RedisHashStore("simorgh:active_lobbies", CustomLobbyRoom.from_dict)
_LOBBY_LOCK = _RedisLockManager("simorgh:lock:lobbies", [_ACTIVE_LOBBIES])
_LOBBY_BROWSER_CHANNELS = _RedisChannelSet("simorgh:lobby_browsers")


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
                    "message": "You have an active match and cannot enter custom lobbies.",
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
                "message": "Lobby list updated.",
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
        elif action == "update_settings":
            await self._handle_update_settings(msg)

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
                "message": "Lobby list updated.",
            }
        )

    async def _handle_create_lobby(self, msg: dict) -> None:
        assert self.player is not None
        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        async with _SOCIAL_LOCK:
            usess = _ONLINE_USERS.get(self.player.user_id)
            in_party_now = False
            if usess and usess.party_id:
                p_obj = _ACTIVE_PARTIES.get(usess.party_id)
                if p_obj and len(p_obj.members) >= 2:
                    in_party_now = True
                else:
                    old_pid = usess.party_id
                    if p_obj:
                        _ACTIVE_PARTIES.pop(old_pid, None)
                    for iid, inv in list(_PENDING_PARTY_INVITES.items()):
                        if inv.party_id == old_pid:
                            _PENDING_PARTY_INVITES.pop(iid, None)
                    usess.party_id = None
                    if usess.status == "in_party":
                        usess.status = "online"
        if in_party_now:
            await self.send_json(
                {
                    "event": "error",
                    "message": "Leave your active party before creating a custom lobby.",
                }
            )
            return

        # چک مجدد عدم حضور در بازی فعال
        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        if active_match:
            await self.send_json(
                {
                    "event": "error",
                    "message": "You have an active match and cannot create a lobby.",
                }
            )
            return

        title = str(msg.get("title", "")).strip()[:36]
        if not title:
            title = f"{self.player.username}'s Room"

        password = str(msg.get("password", "")).strip()[:24]
        try:
            max_players = int(msg.get("max_players", 2))
        except Exception:
            max_players = 2

        if max_players not in ALLOWED_LOBBY_CAPACITIES:
            await self.send_json(
                {
                    "event": "error",
                    "message": "Lobby capacity must be 2, 4, 6, 8, or 10 players.",
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
                            "message": "You are already inside a lobby.",
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
                        "sender": "SYSTEM",
                        "text": f"Lobby '{title}' ({max_players} players) created by {self.player.username}.",
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
                "message": "Lobby created successfully.",
            }
        )
        await self._broadcast_lobby_list_to_browsers()

    async def _handle_join_lobby(self, msg: dict) -> None:
        assert self.player is not None
        lobby_id = str(msg.get("lobby_id", "")).strip()
        password_attempt = str(msg.get("password", "")).strip()

        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        async with _SOCIAL_LOCK:
            usess = _ONLINE_USERS.get(self.player.user_id)
            in_party_now = False
            if usess and usess.party_id:
                p_obj = _ACTIVE_PARTIES.get(usess.party_id)
                if p_obj and len(p_obj.members) >= 2:
                    in_party_now = True
                else:
                    old_pid = usess.party_id
                    if p_obj:
                        _ACTIVE_PARTIES.pop(old_pid, None)
                    for iid, inv in list(_PENDING_PARTY_INVITES.items()):
                        if inv.party_id == old_pid:
                            _PENDING_PARTY_INVITES.pop(iid, None)
                    usess.party_id = None
                    if usess.status == "in_party":
                        usess.status = "online"
        if in_party_now:
            await self.send_json(
                {
                    "event": "error",
                    "message": "Leave your active party before joining a custom lobby.",
                }
            )
            return

        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        if active_match:
            await self.send_json(
                {
                    "event": "error",
                    "message": "You have an active match and cannot join a lobby.",
                }
            )
            return

        async with _LOBBY_LOCK:
            for existing_lb in _ACTIVE_LOBBIES.values():
                if any(m.user_id == self.player.user_id for m in existing_lb.members):
                    await self.send_json(
                        {
                            "event": "error",
                            "message": "You are already in a lobby.",
                        }
                    )
                    return

            lobby = _ACTIVE_LOBBIES.get(lobby_id)
            if not lobby:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "Lobby not found or has been closed.",
                    }
                )
                return

            if lobby.is_starting:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "This lobby is already starting the match.",
                    }
                )
                return

            if len(lobby.members) >= lobby.max_players:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "This lobby is already full.",
                    }
                )
                return

            if lobby.password and not secrets.compare_digest(lobby.password, password_attempt):
                await self.send_json(
                    {
                        "event": "error",
                        "message": "Incorrect lobby password.",
                    }
                )
                return

            free_slot = _find_first_free_slot(lobby)
            if not free_slot:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "No free slot available in this lobby.",
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
                    "sender": "SYSTEM",
                    "text": f"{self.player.username} joined the lobby.",
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
                    "message": f"{self.player.username} entered the lobby.",
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
                            "message": "This slot is already occupied by another player.",
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
                            "message": "This team is already full.",
                        }
                    )
                    return
                chosen_slot = free[1]

            if member.team_id == target_team and member.slot_index == chosen_slot:
                return

            member.team_id = target_team
            member.slot_index = chosen_slot
            team_title = "Team Alpha" if target_team == 1 else "Team Omega"
            lobby.chat_history.append(
                {
                    "sender": "SYSTEM",
                    "text": f"{self.player.username} moved to {team_title} (Slot {chosen_slot + 1}).",
                    "is_system": True,
                }
            )
            detail = _serialize_lobby_detail(lobby)
            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_state",
                    "lobby": detail,
                    "message": f"{self.player.username}'s slot updated.",
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
                        "message": "Only the lobby host can kick players.",
                    }
                )
                return

            if lobby.is_starting:
                await self.send_json(
                    {
                        "event": "error",
                        "message": "Cannot kick players while the match is starting.",
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
                    "sender": "SYSTEM",
                    "text": f"{kicked_member.username} was kicked from the lobby by the host.",
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
                    "message": f"{kicked_member.username} was kicked from the lobby.",
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
                        "message": f"You were removed from the lobby by {self.player.username}.",
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
                        "message": "Match is starting; cannot leave or close the lobby.",
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
                        "sender": "SYSTEM",
                        "text": f"{self.player.username} left the lobby.",
                        "is_system": True,
                    }
                )
                detail = _serialize_lobby_detail(lobby)
                await self._broadcast_to_lobby_members(
                    lobby,
                    {
                        "event": "lobby_state",
                        "lobby": detail,
                        "message": f"{self.player.username} left the lobby.",
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
                            "message": f"Lobby '{lobby_title}' was disbanded because host ({self.player.username}) left.",
                        },
                    },
                )
        elif not is_disconnect:
            await self.send_json(
                {
                    "event": "left_lobby",
                    "lobbies": lobbies_snapshot,
                    "message": "You left the lobby.",
                }
            )

        await self._broadcast_lobby_list_to_browsers()

    async def _handle_update_settings(self, msg: dict) -> None:
        assert self.player is not None
        if not self.current_lobby_id:
            return
        async with _LOBBY_LOCK:
            lobby = _ACTIVE_LOBBIES.get(self.current_lobby_id)
            if not lobby or lobby.host_user_id != self.player.user_id or lobby.is_starting:
                return
            new_title = str(msg.get("title", "")).strip()[:36]
            if new_title:
                lobby.title = new_title
            if msg.get("clear_password"):
                lobby.password = ""
            elif "password" in msg:
                new_pw = str(msg.get("password", "")).strip()[:24]
                if new_pw:
                    lobby.password = new_pw
            try:
                new_cap = int(msg.get("max_players", lobby.max_players))
            except Exception:
                new_cap = lobby.max_players
            if new_cap in ALLOWED_LOBBY_CAPACITIES and new_cap >= len(lobby.members):
                lobby.max_players = new_cap
                half = new_cap // 2
                # ابتدا اسلات اعضای خارج از محدوده جدید را خالی کن تا هنگام جابه‌جایی برخورد رخ ندهد
                displaced = [m for m in lobby.members if m.slot_index >= half]
                for m in displaced:
                    m.slot_index = -1
                for m in displaced:
                    free = _find_first_free_slot(lobby)
                    if free:
                        m.team_id, m.slot_index = free
            lobby.chat_history.append(
                {
                    "sender": "SYSTEM",
                    "text": f"Lobby settings updated by {self.player.username}.",
                    "is_system": True,
                }
            )
            detail = _serialize_lobby_detail(lobby)
            await self._broadcast_to_lobby_members(
                lobby,
                {
                    "event": "lobby_state",
                    "lobby": detail,
                    "message": "Lobby settings updated.",
                },
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
                        "message": "Only the lobby host can start the match.",
                    }
                )
                return

            if lobby.is_starting:
                return

            if len(lobby.members) < lobby.max_players:
                await self.send_json(
                    {
                        "event": "error",
                        "message": f"Lobby is not full yet ({len(lobby.members)}/{lobby.max_players} players).",
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
                        "message": f"Each team must have exactly {half} players.",
                    }
                )
                return

            # قفل کردن لابی به محض شروع بازی (نیازمندی ۱۱)
            lobby.is_starting = True
            lobby.chat_history.append(
                {
                    "sender": "SYSTEM",
                    "text": "Lobby full! Starting dedicated match server...",
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
                    "message": "Starting match server... Please wait.",
                },
            )

        await self._broadcast_lobby_list_to_browsers()

        try:
            port = await _find_available_udp_port_async()
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
                            "message": "No free server port available; please try again.",
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
                            "message": "Failed to start game server; please try again.",
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


# ============================================================================
# سیستم جامع دوستان، وضعیت آنلاین (Presence)، پارتی (Party) و نوتیفیکیشن زنده
# ============================================================================

PARTY_DEFAULT_MAX_SIZE = int(getattr(settings, "PARTY_DEFAULT_MAX_SIZE", 5))
PARTY_INVITE_TTL_SECS = float(getattr(settings, "PARTY_INVITE_TTL_SECS", 60.0))


@dataclass
class OnlineUserSession:
    user_id: str
    username: str
    mmr: int
    rank_tier: str
    player_pk: int
    channel_name: str
    client_ip: str = ""
    status: str = "online"  # online | in_party | in_queue | in_match
    party_id: Optional[str] = None

    @classmethod
    def from_dict(cls, d: dict) -> "OnlineUserSession":
        return cls(
            user_id=str(d.get("user_id", "")),
            username=str(d.get("username", "")),
            mmr=int(d.get("mmr", 1000)),
            rank_tier=str(d.get("rank_tier", "Simorgh Initiate")),
            player_pk=int(d.get("player_pk", 0)),
            channel_name=str(d.get("channel_name", "")),
            client_ip=str(d.get("client_ip", "")),
            status=str(d.get("status", "online")),
            party_id=d.get("party_id"),
        )


@dataclass
class PartyMember:
    user_id: str
    username: str
    mmr: int
    rank_tier: str
    player_pk: int
    channel_name: str
    client_ip: str = ""
    is_ready: bool = False

    @classmethod
    def from_dict(cls, d: dict) -> "PartyMember":
        return cls(
            user_id=str(d.get("user_id", "")),
            username=str(d.get("username", "")),
            mmr=int(d.get("mmr", 1000)),
            rank_tier=str(d.get("rank_tier", "Simorgh Initiate")),
            player_pk=int(d.get("player_pk", 0)),
            channel_name=str(d.get("channel_name", "")),
            client_ip=str(d.get("client_ip", "")),
            is_ready=bool(d.get("is_ready", False)),
        )


@dataclass
class PartyRoom:
    party_id: str
    leader_user_id: str
    game_mode: str
    max_size: int
    members: List[PartyMember]
    chat_history: List[dict]
    is_queuing: bool = False

    @classmethod
    def from_dict(cls, d: dict) -> "PartyRoom":
        return cls(
            party_id=str(d.get("party_id", "")),
            leader_user_id=str(d.get("leader_user_id", "")),
            game_mode=str(d.get("game_mode", "")),
            max_size=int(d.get("max_size", 5)),
            members=[PartyMember.from_dict(m) for m in d.get("members", [])],
            chat_history=list(d.get("chat_history", [])),
            is_queuing=bool(d.get("is_queuing", False)),
        )


@dataclass
class PartyInvite:
    invite_id: str
    party_id: str
    from_user_id: str
    from_username: str
    to_user_id: str
    game_mode: str
    created_at: float
    expires_at: float

    @classmethod
    def from_dict(cls, d: dict) -> "PartyInvite":
        return cls(
            invite_id=str(d.get("invite_id", "")),
            party_id=str(d.get("party_id", "")),
            from_user_id=str(d.get("from_user_id", "")),
            from_username=str(d.get("from_username", "")),
            to_user_id=str(d.get("to_user_id", "")),
            game_mode=str(d.get("game_mode", "")),
            created_at=float(d.get("created_at", 0.0)),
            expires_at=float(d.get("expires_at", 0.0)),
        )


_ONLINE_USERS = _RedisHashStore("simorgh:online_users", OnlineUserSession.from_dict)
_ACTIVE_PARTIES = _RedisHashStore("simorgh:active_parties", PartyRoom.from_dict)
_PENDING_PARTY_INVITES = _RedisHashStore("simorgh:party_invites", PartyInvite.from_dict)
_SOCIAL_LOCK = _RedisLockManager(
    "simorgh:lock:social",
    [_ONLINE_USERS, _ACTIVE_PARTIES, _PENDING_PARTY_INVITES],
)


def get_player_presence_snapshot(user_id: str) -> dict:
    """
    استعلام سریع وضعیت آنلاین بازیکن برای استفاده در ویوهای HTTP (بدون قفل مسدودکننده)
    """
    sess = _ONLINE_USERS.get(user_id)
    if not sess:
        return {"online": False, "status": "offline", "party_id": None}
    return {
        "online": True,
        "status": sess.status,
        "party_id": sess.party_id,
    }


def push_social_event_to_user_sync(user_id: str, payload: dict) -> bool:
    """
    ارسال آنی رویداد/نوتیفیکیشن وب‌سوکت از داخل ویوهای HTTP جنگو به بازیکن آنلاین
    """
    sess = _ONLINE_USERS.get(user_id)
    if not sess or not sess.channel_name:
        return False
    layer = get_channel_layer()
    if layer is None:
        return False
    try:
        async_to_sync(layer.send)(
            sess.channel_name,
            {
                "type": "social.event",
                "payload": payload,
            },
        )
        return True
    except Exception as exc:
        print(f"[SOCIAL PUSH WARNING] Failed pushing to {user_id}: {exc}", flush=True)
        return False


def _serialize_party_state(party: PartyRoom) -> dict:
    return {
        "party_id": party.party_id,
        "leader_user_id": party.leader_user_id,
        "game_mode": party.game_mode,
        "max_size": party.max_size,
        "is_queuing": party.is_queuing,
        "members": [
            {
                "user_id": m.user_id,
                "username": m.username,
                "mmr": m.mmr,
                "rank_tier": m.rank_tier,
                "is_leader": m.user_id == party.leader_user_id,
                "is_ready": m.is_ready,
            }
            for m in party.members
        ],
        "chat_messages": party.chat_history[-35:],
    }


def _serialize_party_invite(inv: PartyInvite) -> dict:
    remaining = max(0, int(inv.expires_at - time.time()))
    return {
        "invite_id": inv.invite_id,
        "party_id": inv.party_id,
        "from_user_id": inv.from_user_id,
        "from_username": inv.from_username,
        "game_mode": inv.game_mode,
        "expires_in_secs": remaining,
    }


def _prune_expired_invites() -> None:
    now = time.time()
    expired = [
        iid for iid, inv in _PENDING_PARTY_INVITES.items() if inv.expires_at <= now
    ]
    for iid in expired:
        _PENDING_PARTY_INVITES.pop(iid, None)


def build_friends_payload_for_player(player: Player) -> dict:
    """
    ساخت لیست کامل دوستان، درخواست‌های ورودی و خروجی همراه با وضعیت آنلاین لحظه‌ای
    """
    friendships = list(
        Friendship.objects.select_related("requester", "addressee").filter(
            Q(requester=player) | Q(addressee=player)
        )
    )

    other_ids = [
        (f.addressee.user_id if f.requester_id == player.id else f.requester.user_id)
        for f in friendships
    ]
    online_map = _ONLINE_USERS.mget(other_ids)

    friends = []
    incoming = []
    outgoing = []

    for f in friendships:
        other = f.addressee if f.requester_id == player.id else f.requester
        sess = online_map.get(other.user_id)
        if sess:
            eff_in_party = False
            eff_status = sess.status
            if sess.party_id:
                p_obj = _ACTIVE_PARTIES.get(sess.party_id)
                if p_obj and len(p_obj.members) >= 2:
                    eff_in_party = True
                elif eff_status == "in_party":
                    eff_status = "online"
            pres = {"online": True, "status": eff_status, "in_party": eff_in_party}
        else:
            pres = {"online": False, "status": "offline", "in_party": False}
        if f.status == Friendship.STATUS_ACCEPTED:
            friends.append(
                {
                    "user_id": other.user_id,
                    "username": other.username,
                    "mmr": other.mmr,
                    "rank_tier": other.rank_tier,
                    "online": pres["online"],
                    "status": pres["status"],
                    "in_party": pres["in_party"],
                }
            )
        elif f.status == Friendship.STATUS_PENDING:
            item = {
                "request_id": f.id,
                "user_id": other.user_id,
                "username": other.username,
                "mmr": other.mmr,
                "rank_tier": other.rank_tier,
                "online": pres["online"],
            }
            if f.addressee_id == player.id:
                incoming.append(item)
            else:
                outgoing.append(item)

    # مرتب‌سازی: ابتدا آنلاین‌ها و سپس بر اساس MMR
    friends.sort(key=lambda x: (not x["online"], -x["mmr"], x["username"].lower()))
    return {
        "friends": friends,
        "incoming_requests": incoming,
        "outgoing_requests": outgoing,
    }


@database_sync_to_async
def _build_friends_payload_async(player: Player) -> dict:
    return build_friends_payload_for_player(player)


@database_sync_to_async
def _get_friend_user_ids_async(player_pk: int) -> List[str]:
    qs = Friendship.objects.select_related("requester", "addressee").filter(
        Q(requester_id=player_pk) | Q(addressee_id=player_pk),
        status=Friendship.STATUS_ACCEPTED,
    )
    out: List[str] = []
    for f in qs:
        other = f.addressee if f.requester_id == player_pk else f.requester
        out.append(other.user_id)
    return out


class SocialPartyConsumer(AsyncWebsocketConsumer):
    """
    وب‌سوکت جامع دوستان، وضعیت آنلاین، پارتی (تا ۵ نفر)، نوتیفیکیشن‌های زنده و صف گروهی:
    ws://<host>/ws/social/?token=<access_token>
    """

    player: Optional[Player] = None
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

        await self.accept()

        party_payload = None
        pending_invites_payload = []

        async with _SOCIAL_LOCK:
            _prune_expired_invites()
            existing_party_id = None
            prev = _ONLINE_USERS.get(self.player.user_id)
            if prev and prev.party_id and prev.party_id in _ACTIVE_PARTIES:
                existing_party_id = prev.party_id

            _ONLINE_USERS[self.player.user_id] = OnlineUserSession(
                user_id=self.player.user_id,
                username=self.player.username,
                mmr=self.player.mmr,
                rank_tier=self.player.rank_tier,
                player_pk=self.player.pk,
                channel_name=self.channel_name,
                client_ip=client_ip,
                status="in_party" if existing_party_id else "online",
                party_id=existing_party_id,
            )

            if existing_party_id:
                party = _ACTIVE_PARTIES.get(existing_party_id)
                if party:
                    for m in party.members:
                        if m.user_id == self.player.user_id:
                            m.channel_name = self.channel_name
                            m.client_ip = client_ip
                            break
                    party_payload = _serialize_party_state(party)

            for inv in _PENDING_PARTY_INVITES.values():
                if inv.to_user_id == self.player.user_id:
                    pending_invites_payload.append(_serialize_party_invite(inv))

        friends_data = await _build_friends_payload_async(self.player)
        await self.send_json(
            {
                "event": "social_snapshot",
                "friends": friends_data["friends"],
                "incoming_requests": friends_data["incoming_requests"],
                "outgoing_requests": friends_data["outgoing_requests"],
                "party": party_payload,
                "pending_invites": pending_invites_payload,
            }
        )

        await self._broadcast_presence_to_friends(
            online=True,
            status="in_party" if party_payload else "online",
            in_party=party_payload is not None,
        )

    async def disconnect(self, close_code: int) -> None:
        if not self.player:
            return

        uid = self.player.user_id
        party_to_leave: Optional[str] = None
        became_offline = False

        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(uid)
            if sess and sess.channel_name == self.channel_name:
                party_to_leave = sess.party_id
                _ONLINE_USERS.pop(uid, None)
                became_offline = True

        if party_to_leave:
            await self._remove_user_from_party(uid, reason="disconnected")

        if became_offline:
            await self._broadcast_presence_to_friends(
                online=False, status="offline", in_party=False
            )

    async def send_json(self, payload: dict) -> None:
        await self.send(text_data=json.dumps(payload))

    async def _broadcast_presence_for_user(
        self,
        player_pk: int,
        user_id: str,
        username: str,
        online: bool,
        status: str,
        in_party: bool,
    ) -> None:
        friend_ids = await _get_friend_user_ids_async(player_pk)
        if not friend_ids:
            return
        targets = []
        online_friends = await asyncio.to_thread(_ONLINE_USERS.mget, friend_ids)
        for fid in friend_ids:
            fsess = online_friends.get(fid)
            if fsess and fsess.channel_name:
                targets.append(fsess.channel_name)

        payload = {
            "event": "friend_presence",
            "user_id": user_id,
            "username": username,
            "online": online,
            "status": status,
            "in_party": in_party,
        }
        for ch in targets:
            await self.channel_layer.send(
                ch,
                {
                    "type": "social.event",
                    "payload": payload,
                },
            )

    async def _broadcast_presence_to_friends(
        self, online: bool, status: str, in_party: bool
    ) -> None:
        if not self.player:
            return
        await self._broadcast_presence_for_user(
            self.player.pk,
            self.player.user_id,
            self.player.username,
            online=online,
            status=status,
            in_party=in_party,
        )

    async def _broadcast_to_party(self, party: PartyRoom, payload: dict) -> None:
        for m in list(party.members):
            if m.channel_name:
                await self.channel_layer.send(
                    m.channel_name,
                    {
                        "type": "social.event",
                        "payload": payload,
                    },
                )

    async def receive(
        self, text_data: Optional[str] = None, bytes_data: Optional[bytes] = None
    ) -> None:
        if not text_data or not self.player:
            return
        try:
            msg = json.loads(text_data)
        except Exception:
            return

        action = str(msg.get("action", "")).strip()
        if action == "refresh_social":
            await self._handle_refresh_social()
        elif action == "invite_to_party":
            await self._handle_invite_to_party(msg)
        elif action == "cancel_party_invite":
            await self._handle_cancel_party_invite(msg)
        elif action == "respond_party_invite":
            await self._handle_respond_party_invite(msg)
        elif action == "toggle_ready":
            await self._handle_toggle_ready()
        elif action == "set_game_mode":
            await self._handle_set_game_mode(msg)
        elif action == "kick_party_member":
            await self._handle_kick_party_member(msg)
        elif action == "promote_leader":
            await self._handle_promote_leader(msg)
        elif action == "leave_party":
            await self._remove_user_from_party(self.player.user_id, reason="left")
        elif action == "party_chat":
            await self._handle_party_chat(msg)
        elif action == "start_party_matchmaking":
            await self._handle_start_party_matchmaking()
        elif action == "cancel_party_matchmaking":
            await self._handle_cancel_party_matchmaking()
        elif action == "invite_to_lobby":
            await self._handle_invite_to_lobby(msg)

    async def _handle_refresh_social(self) -> None:
        assert self.player is not None
        party_payload = None
        pending_invites_payload = []
        async with _SOCIAL_LOCK:
            _prune_expired_invites()
            sess = _ONLINE_USERS.get(self.player.user_id)
            if sess and sess.party_id and sess.party_id in _ACTIVE_PARTIES:
                p_obj = _ACTIVE_PARTIES[sess.party_id]
                if len(p_obj.members) >= 2:
                    party_payload = _serialize_party_state(p_obj)
            for inv in _PENDING_PARTY_INVITES.values():
                if inv.to_user_id == self.player.user_id:
                    pending_invites_payload.append(_serialize_party_invite(inv))
        friends_data = await _build_friends_payload_async(self.player)
        await self.send_json(
            {
                "event": "social_snapshot",
                "friends": friends_data["friends"],
                "incoming_requests": friends_data["incoming_requests"],
                "outgoing_requests": friends_data["outgoing_requests"],
                "party": party_payload,
                "pending_invites": pending_invites_payload,
            }
        )

    async def _handle_invite_to_party(self, msg: dict) -> None:
        assert self.player is not None
        target_uid = str(msg.get("target_user_id", "")).strip()
        requested_mode = str(msg.get("game_mode", "5v5 JUNGLE SKIRMISH")).strip()[:40]
        if not target_uid or target_uid == self.player.user_id:
            return

        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        active_match = await _get_active_match_for_player_async(
            self.player, client_ip=client_ip
        )
        if active_match:
            await self.send_json(
                {
                    "event": "notification",
                    "kind": "error",
                    "title": "ACTIVE MATCH",
                    "message": "Finish or abandon your active match before forming a party.",
                }
            )
            return

        target_channel = None
        invite_payload = None
        party_state_payload = None

        async with _SOCIAL_LOCK:
            _prune_expired_invites()
            me_sess = _ONLINE_USERS.get(self.player.user_id)
            target_sess = _ONLINE_USERS.get(target_uid)
            if not me_sess:
                return
            if not target_sess:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "PLAYER OFFLINE",
                        "message": "Target player is currently offline.",
                    }
                )
                return

            # ایجاد خودکار پارتی در صورتی که بازیکن هنوز در پارتی نباشد
            party = _ACTIVE_PARTIES.get(me_sess.party_id) if me_sess.party_id else None
            if party is None:
                party_id = f"pty_{secrets.randbelow(90000) + 10000}"
                while party_id in _ACTIVE_PARTIES:
                    party_id = f"pty_{secrets.randbelow(90000) + 10000}"
                party = PartyRoom(
                    party_id=party_id,
                    leader_user_id=self.player.user_id,
                    game_mode=requested_mode or "5v5 JUNGLE SKIRMISH",
                    max_size=PARTY_DEFAULT_MAX_SIZE,
                    members=[
                        PartyMember(
                            user_id=self.player.user_id,
                            username=self.player.username,
                            mmr=self.player.mmr,
                            rank_tier=self.player.rank_tier,
                            player_pk=self.player.pk,
                            channel_name=self.channel_name,
                            client_ip=client_ip,
                            is_ready=False,
                        )
                    ],
                    chat_history=[
                        {
                            "sender": "SYSTEM",
                            "text": f"{self.player.username} formed a squad.",
                            "is_system": True,
                        }
                    ],
                )
                _ACTIVE_PARTIES[party_id] = party
                me_sess.party_id = party_id

            if party.leader_user_id != self.player.user_id:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "LEADER ONLY",
                        "message": "Only the party leader can invite players.",
                    }
                )
                return

            if party.is_queuing:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "IN QUEUE",
                        "message": "Cancel matchmaking before inviting more squadmates.",
                    }
                )
                return

            if len(party.members) >= party.max_size:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "PARTY FULL",
                        "message": f"Squad is full ({party.max_size}/{party.max_size}).",
                    }
                )
                return

            if any(m.user_id == target_uid for m in party.members):
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "info",
                        "title": "ALREADY IN SQUAD",
                        "message": f"{target_sess.username} is already in your party.",
                    }
                )
                return

            # حذف دعوت‌های تکراری قبلی همین پارتی به همین کاربر
            for iid, inv in list(_PENDING_PARTY_INVITES.items()):
                if inv.party_id == party.party_id and inv.to_user_id == target_uid:
                    _PENDING_PARTY_INVITES.pop(iid, None)

            now = time.time()
            invite_id = f"inv_{secrets.token_hex(6)}"
            invite = PartyInvite(
                invite_id=invite_id,
                party_id=party.party_id,
                from_user_id=self.player.user_id,
                from_username=self.player.username,
                to_user_id=target_uid,
                game_mode=party.game_mode,
                created_at=now,
                expires_at=now + PARTY_INVITE_TTL_SECS,
            )
            _PENDING_PARTY_INVITES[invite_id] = invite
            target_channel = target_sess.channel_name
            invite_payload = _serialize_party_invite(invite)
            party_state_payload = _serialize_party_state(party)

        if party_state_payload:
            if len(party_state_payload.get("members", [])) >= 2:
                await self.send_json(
                    {
                        "event": "party_state",
                        "party": party_state_payload,
                    }
                )
            await self.send_json(
                {
                    "event": "notification",
                    "kind": "info",
                    "title": "INVITE SENT",
                    "message": f"Squad invite sent to {target_sess.username}.",
                }
            )

        if target_channel and invite_payload:
            await self.channel_layer.send(
                target_channel,
                {
                    "type": "social.event",
                    "payload": {
                        "event": "party_invite_received",
                        "invite": invite_payload,
                        "title": "PARTY INVITE",
                        "message": f"{self.player.username} invited you to join their squad ({invite_payload['game_mode']}).",
                    },
                },
            )

    async def _handle_cancel_party_invite(self, msg: dict) -> None:
        assert self.player is not None
        target_uid = str(msg.get("target_user_id", "")).strip()
        if not target_uid:
            return

        canceled_inv_ids: List[str] = []
        target_channel: Optional[str] = None
        disbanded_solo = False

        async with _SOCIAL_LOCK:
            _prune_expired_invites()
            me_sess = _ONLINE_USERS.get(self.player.user_id)
            target_sess = _ONLINE_USERS.get(target_uid)
            if target_sess:
                target_channel = target_sess.channel_name

            for iid, inv in list(_PENDING_PARTY_INVITES.items()):
                if inv.from_user_id == self.player.user_id and inv.to_user_id == target_uid:
                    _PENDING_PARTY_INVITES.pop(iid, None)
                    canceled_inv_ids.append(iid)

            if me_sess and me_sess.party_id:
                party = _ACTIVE_PARTIES.get(me_sess.party_id)
                if party and len(party.members) <= 1:
                    has_other_invites = any(
                        inv.party_id == party.party_id for inv in _PENDING_PARTY_INVITES.values()
                    )
                    if not has_other_invites:
                        _ACTIVE_PARTIES.pop(party.party_id, None)
                        me_sess.party_id = None
                        if me_sess.status == "in_party":
                            me_sess.status = "online"
                        disbanded_solo = True

        if target_channel:
            for iid in canceled_inv_ids:
                await self.channel_layer.send(
                    target_channel,
                    {
                        "type": "social.event",
                        "payload": {
                            "event": "party_invite_cleared",
                            "invite_id": iid,
                        },
                    },
                )

        if canceled_inv_ids:
            await self.send_json(
                {
                    "event": "party_invite_result",
                    "target_user_id": target_uid,
                    "accepted": False,
                    "message": "Party invite canceled.",
                }
            )
        if disbanded_solo:
            await self._broadcast_presence_to_friends(
                online=True, status="online", in_party=False
            )

    async def _handle_respond_party_invite(self, msg: dict) -> None:
        assert self.player is not None
        invite_id = str(msg.get("invite_id", "")).strip()
        accept = bool(msg.get("accept", False))

        client_info = self.scope.get("client")
        client_ip = client_info[0] if client_info and len(client_info) > 0 else ""

        if accept:
            active_match = await _get_active_match_for_player_async(
                self.player, client_ip=client_ip
            )
            if active_match:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "ACTIVE MATCH",
                        "message": "Cannot join a party while you have an active match.",
                    }
                )
                return

            async with _LOBBY_LOCK:
                in_custom_lobby = any(
                    any(m.user_id == self.player.user_id for m in lb.members)
                    for lb in _ACTIVE_LOBBIES.values()
                )
            if in_custom_lobby:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "IN CUSTOM LOBBY",
                        "message": "Leave your Custom Lobby before joining a party.",
                    }
                )
                return

            async with _QUEUE_LOCK:
                in_solo_queue = any(q.user_id == self.player.user_id for q in _WAITING_QUEUE)
            if in_solo_queue:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "IN MATCHMAKING",
                        "message": "Cancel matchmaking before joining a party.",
                    }
                )
                return

        inviter_channel = None
        joined_party_members: List[PartyMember] = []
        joined_party_detail: Optional[dict] = None
        old_party_id_to_leave: Optional[str] = None

        async with _SOCIAL_LOCK:
            _prune_expired_invites()
            inv = _PENDING_PARTY_INVITES.pop(invite_id, None)
            if not inv or inv.to_user_id != self.player.user_id:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "INVITE EXPIRED",
                        "message": "This party invite has expired or is no longer valid.",
                    }
                )
                return

            inviter_sess = _ONLINE_USERS.get(inv.from_user_id)
            if inviter_sess:
                inviter_channel = inviter_sess.channel_name

            if not accept:
                inv_party = _ACTIVE_PARTIES.get(inv.party_id)
                if inv_party and len(inv_party.members) <= 1:
                    has_more = any(
                        p_inv.party_id == inv.party_id
                        for p_inv in _PENDING_PARTY_INVITES.values()
                    )
                    if not has_more:
                        _ACTIVE_PARTIES.pop(inv.party_id, None)
                        if inviter_sess and inviter_sess.party_id == inv.party_id:
                            inviter_sess.party_id = None
                            if inviter_sess.status == "in_party":
                                inviter_sess.status = "online"
                await self.send_json(
                    {
                        "event": "party_invite_cleared",
                        "invite_id": invite_id,
                    }
                )
            else:
                party = _ACTIVE_PARTIES.get(inv.party_id)
                if not party:
                    await self.send_json(
                        {
                            "event": "notification",
                            "kind": "error",
                            "title": "PARTY CLOSED",
                            "message": "The party no longer exists.",
                        }
                    )
                    return

                if party.is_queuing:
                    await self.send_json(
                        {
                            "event": "notification",
                            "kind": "error",
                            "title": "PARTY IN QUEUE",
                            "message": "This party is already searching for a match.",
                        }
                    )
                    return

                if len(party.members) >= party.max_size:
                    await self.send_json(
                        {
                            "event": "notification",
                            "kind": "error",
                            "title": "PARTY FULL",
                            "message": "The squad is already full.",
                        }
                    )
                    return

                me_sess = _ONLINE_USERS.get(self.player.user_id)
                if me_sess and me_sess.party_id and me_sess.party_id != party.party_id:
                    old_party_id_to_leave = me_sess.party_id

        if old_party_id_to_leave:
            await self._remove_user_from_party(self.player.user_id, reason="switched")

        if not accept:
            if inviter_channel:
                await self.channel_layer.send(
                    inviter_channel,
                    {
                        "type": "social.event",
                        "payload": {
                            "event": "party_invite_result",
                            "target_user_id": self.player.user_id,
                            "accepted": False,
                            "message": f"{self.player.username} declined your party invite.",
                        },
                    },
                )
            return

        async with _SOCIAL_LOCK:
            party = _ACTIVE_PARTIES.get(inv.party_id)
            if not party or len(party.members) >= party.max_size:
                return
            if not any(m.user_id == self.player.user_id for m in party.members):
                party.members.append(
                    PartyMember(
                        user_id=self.player.user_id,
                        username=self.player.username,
                        mmr=self.player.mmr,
                        rank_tier=self.player.rank_tier,
                        player_pk=self.player.pk,
                        channel_name=self.channel_name,
                        client_ip=client_ip,
                        is_ready=False,
                    )
                )
                party.chat_history.append(
                    {
                        "sender": "SYSTEM",
                        "text": f"{self.player.username} joined the squad.",
                        "is_system": True,
                    }
                )
            for m in party.members:
                ms = _ONLINE_USERS.get(m.user_id)
                if ms:
                    ms.party_id = party.party_id
                    ms.status = "in_party"

            joined_party_members = list(party.members)
            joined_party_detail = _serialize_party_state(party)

        if joined_party_detail:
            for m in joined_party_members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {
                                "event": "party_state",
                                "party": joined_party_detail,
                            },
                        },
                    )
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {
                                "event": "notification",
                                "kind": "party_joined",
                                "title": "SQUAD UPDATED",
                                "message": f"{self.player.username} joined the party ({len(joined_party_members)}/{joined_party_detail['max_size']}).",
                            },
                        },
                    )
            for m in joined_party_members:
                await self._broadcast_presence_for_user(
                    m.player_pk,
                    m.user_id,
                    m.username,
                    online=True,
                    status="in_party",
                    in_party=True,
                )

    async def _handle_toggle_ready(self) -> None:
        assert self.player is not None
        detail = None
        members = []
        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party or party.is_queuing:
                return
            for m in party.members:
                if m.user_id == self.player.user_id:
                    m.is_ready = not m.is_ready
                    break
            detail = _serialize_party_state(party)
            members = list(party.members)

        if detail:
            for m in members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": detail},
                        },
                    )

    async def _handle_set_game_mode(self, msg: dict) -> None:
        assert self.player is not None
        mode = str(msg.get("game_mode", "")).strip()[:40]
        if not mode:
            return
        detail = None
        members = []
        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party or party.leader_user_id != self.player.user_id or party.is_queuing:
                return
            party.game_mode = mode
            party.chat_history.append(
                {
                    "sender": "SYSTEM",
                    "text": f"Mode changed to {mode}.",
                    "is_system": True,
                }
            )
            detail = _serialize_party_state(party)
            members = list(party.members)

        if detail:
            for m in members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": detail},
                        },
                    )

    async def _handle_kick_party_member(self, msg: dict) -> None:
        assert self.player is not None
        target_uid = str(msg.get("target_user_id", "")).strip()
        if not target_uid or target_uid == self.player.user_id:
            return

        kicked_member: Optional[PartyMember] = None
        detail = None
        remaining_members: List[PartyMember] = []
        disbanded_sole_member: Optional[PartyMember] = None
        cleared_invites: List[tuple] = []

        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party or party.leader_user_id != self.player.user_id or party.is_queuing:
                return
            for m in party.members:
                if m.user_id == target_uid:
                    kicked_member = m
                    break
            if not kicked_member:
                return
            party.members = [m for m in party.members if m.user_id != target_uid]
            ksess = _ONLINE_USERS.get(target_uid)
            if ksess:
                ksess.party_id = None
                ksess.status = "online"

            if len(party.members) <= 1:
                _ACTIVE_PARTIES.pop(party.party_id, None)
                for iid, inv in list(_PENDING_PARTY_INVITES.items()):
                    if inv.party_id == party.party_id:
                        _PENDING_PARTY_INVITES.pop(iid, None)
                        ts = _ONLINE_USERS.get(inv.to_user_id)
                        if ts and ts.channel_name:
                            cleared_invites.append((ts.channel_name, iid))
                if len(party.members) == 1:
                    disbanded_sole_member = party.members[0]
                    sole_sess = _ONLINE_USERS.get(disbanded_sole_member.user_id)
                    if sole_sess:
                        sole_sess.party_id = None
                        sole_sess.status = "online"
            else:
                party.chat_history.append(
                    {
                        "sender": "SYSTEM",
                        "text": f"{kicked_member.username} was removed from the squad.",
                        "is_system": True,
                    }
                )
                detail = _serialize_party_state(party)
                remaining_members = list(party.members)

        for ch, iid in cleared_invites:
            await self.channel_layer.send(
                ch,
                {
                    "type": "social.event",
                    "payload": {"event": "party_invite_cleared", "invite_id": iid},
                },
            )

        if kicked_member:
            if kicked_member.channel_name:
                await self.channel_layer.send(
                    kicked_member.channel_name,
                    {
                        "type": "social.event",
                        "payload": {
                            "event": "party_kicked",
                            "title": "REMOVED FROM PARTY",
                            "message": f"You were removed from the squad by {self.player.username}.",
                        },
                    },
                )
            await self._broadcast_presence_for_user(
                kicked_member.player_pk,
                kicked_member.user_id,
                kicked_member.username,
                online=True,
                status="online",
                in_party=False,
            )

        if disbanded_sole_member:
            if disbanded_sole_member.channel_name:
                await self.channel_layer.send(
                    disbanded_sole_member.channel_name,
                    {
                        "type": "social.event",
                        "payload": {
                            "event": "party_left",
                            "message": f"{kicked_member.username} was removed. Party disbanded.",
                        },
                    },
                )
            await self._broadcast_presence_for_user(
                disbanded_sole_member.player_pk,
                disbanded_sole_member.user_id,
                disbanded_sole_member.username,
                online=True,
                status="online",
                in_party=False,
            )
        elif detail:
            for m in remaining_members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": detail},
                        },
                    )

    async def _handle_promote_leader(self, msg: dict) -> None:
        assert self.player is not None
        target_uid = str(msg.get("target_user_id", "")).strip()
        if not target_uid or target_uid == self.player.user_id:
            return
        detail = None
        members = []
        new_leader_name = ""
        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party or party.leader_user_id != self.player.user_id:
                return
            target_m = next((m for m in party.members if m.user_id == target_uid), None)
            if not target_m:
                return
            party.leader_user_id = target_uid
            new_leader_name = target_m.username
            party.chat_history.append(
                {
                    "sender": "SYSTEM",
                    "text": f"{new_leader_name} is now the squad leader.",
                    "is_system": True,
                }
            )
            detail = _serialize_party_state(party)
            members = list(party.members)

        if detail:
            for m in members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": detail},
                        },
                    )
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {
                                "event": "notification",
                                "kind": "info",
                                "title": "NEW SQUAD LEADER",
                                "message": f"{new_leader_name} is now the party leader.",
                            },
                        },
                    )

    async def _handle_party_chat(self, msg: dict) -> None:
        assert self.player is not None
        now = time.monotonic()
        if now - self.last_chat_ts < 0.3:
            return
        self.last_chat_ts = now

        raw = str(msg.get("text", "")).strip()
        clean = "".join(ch for ch in raw if ch.isprintable())[:140]
        if not clean:
            return

        detail = None
        members = []
        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party:
                return
            party.chat_history.append(
                {
                    "sender": self.player.username,
                    "text": clean,
                    "is_system": False,
                }
            )
            if len(party.chat_history) > 50:
                party.chat_history = party.chat_history[-50:]
            detail = _serialize_party_state(party)
            members = list(party.members)

        if detail:
            for m in members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": detail},
                        },
                    )

    async def _remove_user_from_party(self, user_id: str, reason: str = "left") -> None:
        leaver_channel = None
        leaver_name = user_id
        leaver_pk: Optional[int] = None
        remaining_members: List[PartyMember] = []
        updated_detail = None
        disbanded_sole_member: Optional[PartyMember] = None
        cleared_invites: List[tuple] = []
        canceled_queue_channels: List[str] = []

        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(user_id)
            party_id = sess.party_id if sess else None
            if not party_id:
                # جستجوی ایمن در صورت دیسکانکت
                for pid, p in list(_ACTIVE_PARTIES.items()):
                    if any(m.user_id == user_id for m in p.members):
                        party_id = pid
                        break
            if not party_id:
                return

            party = _ACTIVE_PARTIES.get(party_id)
            if not party:
                if sess:
                    sess.party_id = None
                    sess.status = "online"
                return

            for m in party.members:
                if m.user_id == user_id:
                    leaver_channel = m.channel_name
                    leaver_name = m.username
                    leaver_pk = m.player_pk
                    break

            was_queuing = party.is_queuing
            party.members = [m for m in party.members if m.user_id != user_id]
            if sess:
                sess.party_id = None
                sess.status = "online"

            if was_queuing:
                party.is_queuing = False
                canceled_queue_channels = [m.channel_name for m in party.members if m.channel_name]
                # پاک‌سازی صف مچ‌میکینگ برای اعضای پارتی
                member_ids = {m.user_id for m in party.members} | {user_id}
                async with _QUEUE_LOCK:
                    _WAITING_QUEUE[:] = [
                        q
                        for q in _WAITING_QUEUE
                        if q.user_id not in member_ids and q.party_group_id != party_id
                    ]

            if len(party.members) <= 1:
                _ACTIVE_PARTIES.pop(party_id, None)
                for iid, inv in list(_PENDING_PARTY_INVITES.items()):
                    if inv.party_id == party_id:
                        _PENDING_PARTY_INVITES.pop(iid, None)
                        ts = _ONLINE_USERS.get(inv.to_user_id)
                        if ts and ts.channel_name:
                            cleared_invites.append((ts.channel_name, iid))
                if len(party.members) == 1:
                    disbanded_sole_member = party.members[0]
                    sole_sess = _ONLINE_USERS.get(disbanded_sole_member.user_id)
                    if sole_sess:
                        sole_sess.party_id = None
                        sole_sess.status = "online"
            else:
                if party.leader_user_id == user_id:
                    party.leader_user_id = party.members[0].user_id
                    party.chat_history.append(
                        {
                            "sender": "SYSTEM",
                            "text": f"{leaver_name} left. {party.members[0].username} is now leader.",
                            "is_system": True,
                        }
                    )
                else:
                    party.chat_history.append(
                        {
                            "sender": "SYSTEM",
                            "text": f"{leaver_name} left the squad.",
                            "is_system": True,
                        }
                    )
                for m in party.members:
                    ms = _ONLINE_USERS.get(m.user_id)
                    if ms:
                        ms.status = "in_party"
                updated_detail = _serialize_party_state(party)
                remaining_members = list(party.members)

        for ch, iid in cleared_invites:
            await self.channel_layer.send(
                ch,
                {
                    "type": "social.event",
                    "payload": {"event": "party_invite_cleared", "invite_id": iid},
                },
            )

        if reason == "left" and leaver_channel:
            await self.channel_layer.send(
                leaver_channel,
                {
                    "type": "social.event",
                    "payload": {
                        "event": "party_left",
                        "message": "You left the party.",
                    },
                },
            )
            if leaver_pk is not None:
                await self._broadcast_presence_for_user(
                    leaver_pk,
                    user_id,
                    leaver_name,
                    online=True,
                    status="online",
                    in_party=False,
                )

        for ch in canceled_queue_channels:
            await self.channel_layer.send(
                ch,
                {
                    "type": "social.event",
                    "payload": {
                        "event": "party_queue_canceled",
                        "message": f"Matchmaking canceled because {leaver_name} left the squad.",
                    },
                },
            )

        if disbanded_sole_member:
            if disbanded_sole_member.channel_name:
                await self.channel_layer.send(
                    disbanded_sole_member.channel_name,
                    {
                        "type": "social.event",
                        "payload": {
                            "event": "party_left",
                            "message": f"{leaver_name} left. Party disbanded.",
                        },
                    },
                )
            await self._broadcast_presence_for_user(
                disbanded_sole_member.player_pk,
                disbanded_sole_member.user_id,
                disbanded_sole_member.username,
                online=True,
                status="online",
                in_party=False,
            )
        elif updated_detail:
            for m in remaining_members:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": updated_detail},
                        },
                    )
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {
                                "event": "notification",
                                "kind": "party_left",
                                "title": "SQUAD MEMBER LEFT",
                                "message": f"{leaver_name} left the party.",
                            },
                        },
                    )

    async def _handle_start_party_matchmaking(self) -> None:
        assert self.player is not None
        party_members_copy: List[PartyMember] = []
        party_id_copy: Optional[str] = None
        party_detail: Optional[dict] = None

        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "NO ACTIVE PARTY",
                        "message": "You are not currently in a party.",
                    }
                )
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party:
                return
            if party.leader_user_id != self.player.user_id:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "LEADER ONLY",
                        "message": "Only the party leader can start matchmaking.",
                    }
                )
                return
            if party.is_queuing:
                return

            not_ready = [m.username for m in party.members if len(party.members) > 1 and not m.is_ready]
            if not_ready:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "SQUAD NOT READY",
                        "message": f"Waiting for ready check: {', '.join(not_ready)}",
                    }
                )
                return

            member_pks = [m.player_pk for m in party.members]

        busy_member_name = await _find_party_member_in_active_match(member_pks)
        if busy_member_name:
            await self.send_json(
                {
                    "event": "notification",
                    "kind": "error",
                    "title": "ACTIVE MATCH",
                    "message": f"{busy_member_name} already has an active match in progress.",
                }
            )
            return

        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party or party.leader_user_id != self.player.user_id or party.is_queuing:
                return
            party.is_queuing = True
            party_id_copy = party.party_id
            party_members_copy = list(party.members)
            for m in party.members:
                ms = _ONLINE_USERS.get(m.user_id)
                if ms:
                    ms.status = "in_queue"
            party_detail = _serialize_party_state(party)

        required = max(
            len(party_members_copy),
            int(getattr(settings, "MATCHMAKING_PLAYERS_PER_MATCH", 2)),
        )

        # اعلام ورود کل پارتی به صف مچ‌میکینگ
        if party_detail:
            for m in party_members_copy:
                if m.channel_name:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": party_detail},
                        },
                    )

        matched_batch: Optional[List[QueuedPlayerEntry]] = None
        queue_snapshot: List[QueuedPlayerEntry] = []
        party_uids = {m.user_id for m in party_members_copy}

        async with _QUEUE_LOCK:
            _WAITING_QUEUE[:] = [
                q for q in _WAITING_QUEUE if q.user_id not in party_uids
            ]
            for m in party_members_copy:
                _WAITING_QUEUE.append(
                    QueuedPlayerEntry(
                        user_id=m.user_id,
                        username=m.username,
                        mmr=m.mmr,
                        channel_name=m.channel_name,
                        player_pk=m.player_pk,
                        client_ip=m.client_ip,
                        party_group_id=party_id_copy or "",
                    )
                )

            matched_batch = _pop_matched_batch_preserving_parties(_WAITING_QUEUE, required)
            if matched_batch is None:
                queue_snapshot = list(_WAITING_QUEUE)

        if matched_batch is not None:
            try:
                port = await _find_available_udp_port_async()
            except Exception:
                p_groups = {e.party_group_id for e in matched_batch if e.party_group_id}
                async with _SOCIAL_LOCK:
                    for pid_g in p_groups:
                        p_obj = _ACTIVE_PARTIES.get(pid_g)
                        if p_obj:
                            p_obj.is_queuing = False
                            for pm in p_obj.members:
                                ps = _ONLINE_USERS.get(pm.user_id)
                                if ps:
                                    ps.status = "in_party"
                            p_state = _serialize_party_state(p_obj)
                            for pm in p_obj.members:
                                if pm.channel_name:
                                    await self.channel_layer.send(
                                        pm.channel_name,
                                        {
                                            "type": "social.event",
                                            "payload": {"event": "party_state", "party": p_state},
                                        },
                                    )
                for entry in matched_batch:
                    await self.channel_layer.send(
                        entry.channel_name,
                        {
                            "type": "social.event",
                            "payload": {
                                "event": "notification",
                                "kind": "error",
                                "title": "SERVER BUSY",
                                "message": "No free game server port available; please try again.",
                            },
                        },
                    )
                return
            room_id = f"room-{secrets.randbelow(900) + 100}-{secrets.token_hex(2)}"
            first_ip = matched_batch[0].client_ip if matched_batch else ""
            server_host = resolve_game_server_host(first_ip)

            # قرار دادن هر گروه پارتی در یک تیم واحد (هیچ‌گاه اعضای یک پارتی مقابل هم قرار نگیرند)
            half_team = (required + 1) // 2
            explicit_teams: Dict[str, int] = {}
            groups: Dict[str, List[QueuedPlayerEntry]] = {}
            for entry in matched_batch:
                gkey = entry.party_group_id or f"solo:{entry.user_id}"
                groups.setdefault(gkey, []).append(entry)
            ordered_groups = sorted(groups.values(), key=lambda g: -len(g))
            t1_count = 0
            t2_count = 0
            for grp in ordered_groups:
                if t1_count + len(grp) <= half_team:
                    chosen_team = 1
                    t1_count += len(grp)
                elif t2_count + len(grp) <= half_team:
                    chosen_team = 2
                    t2_count += len(grp)
                elif t1_count <= t2_count:
                    chosen_team = 1
                    t1_count += len(grp)
                else:
                    chosen_team = 2
                    t2_count += len(grp)
                for entry in grp:
                    explicit_teams[entry.user_id] = chosen_team

            for entry in matched_batch:
                await self.channel_layer.send(
                    entry.channel_name,
                    {
                        "type": "social.event" if entry.user_id in party_uids else "queue.status",
                        "payload": {
                            "event": "starting_server",
                            "room_id": room_id,
                            "port": port,
                            "players_in_queue": required,
                            "required_players": required,
                        },
                    },
                )

            assignments = await _create_match_records_and_spawn(
                matched_entries=matched_batch,
                port=port,
                room_id=room_id,
                server_host=server_host,
                player_count=required,
                explicit_teams=explicit_teams,
            )

            server_ready = await wait_for_udp_server_ready(port, timeout_sec=30.0)
            async with _SOCIAL_LOCK:
                if party_id_copy and party_id_copy in _ACTIVE_PARTIES:
                    _ACTIVE_PARTIES[party_id_copy].is_queuing = False
                for m in party_members_copy:
                    ms = _ONLINE_USERS.get(m.user_id)
                    if ms:
                        ms.status = "in_match" if server_ready else "in_party"

            if not server_ready:
                await _mark_room_finished(room_id)
                for entry in matched_batch:
                    await self.channel_layer.send(
                        entry.channel_name,
                        {
                            "type": "social.event" if entry.user_id in party_uids else "queue.status",
                            "payload": {
                                "event": "error",
                                "message": "Failed to launch game server; please try again.",
                            },
                        },
                    )
                return

            for entry in matched_batch:
                payload = assignments.get(entry.channel_name)
                if not payload:
                    continue
                await self.channel_layer.send(
                    entry.channel_name,
                    {
                        "type": "social.event" if entry.user_id in party_uids else "match.found",
                        "payload": payload,
                    },
                )
        else:
            for item in queue_snapshot:
                await self.channel_layer.send(
                    item.channel_name,
                    {
                        "type": "social.event" if item.user_id in party_uids else "queue.status",
                        "payload": {
                            "event": "queued",
                            "players_in_queue": len(queue_snapshot),
                            "required_players": required,
                        },
                    },
                )

    async def _handle_cancel_party_matchmaking(self) -> None:
        assert self.player is not None
        members: List[PartyMember] = []
        detail = None
        async with _SOCIAL_LOCK:
            sess = _ONLINE_USERS.get(self.player.user_id)
            if not sess or not sess.party_id:
                return
            party = _ACTIVE_PARTIES.get(sess.party_id)
            if not party:
                return
            party.is_queuing = False
            members = list(party.members)
            member_uids = {m.user_id for m in members}
            for m in members:
                ms = _ONLINE_USERS.get(m.user_id)
                if ms:
                    ms.status = "in_party"
            detail = _serialize_party_state(party)

        async with _QUEUE_LOCK:
            _WAITING_QUEUE[:] = [
                q for q in _WAITING_QUEUE if q.user_id not in member_uids
            ]

        for m in members:
            if m.channel_name:
                if detail:
                    await self.channel_layer.send(
                        m.channel_name,
                        {
                            "type": "social.event",
                            "payload": {"event": "party_state", "party": detail},
                        },
                    )
                await self.channel_layer.send(
                    m.channel_name,
                    {
                        "type": "social.event",
                        "payload": {
                            "event": "party_queue_canceled",
                            "message": f"Matchmaking canceled by {self.player.username}.",
                        },
                    },
                )

    async def _handle_invite_to_lobby(self, msg: dict) -> None:
        assert self.player is not None
        target_uid = str(msg.get("target_user_id", "")).strip()
        lobby_id = str(msg.get("lobby_id", "")).strip()
        lobby_title = str(msg.get("lobby_title", "")).strip()[:36]
        if not target_uid or not lobby_id or target_uid == self.player.user_id:
            return
        async with _LOBBY_LOCK:
            lb_obj = _ACTIVE_LOBBIES.get(lobby_id)
            if not lb_obj or lb_obj.is_starting:
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "LOBBY UNAVAILABLE",
                        "message": "This lobby is no longer open for invites.",
                    }
                )
                return
            if not any(m.user_id == self.player.user_id for m in lb_obj.members):
                await self.send_json(
                    {
                        "event": "notification",
                        "kind": "error",
                        "title": "NOT IN LOBBY",
                        "message": "You must be inside the lobby to invite players.",
                    }
                )
                return
            has_password = bool(lb_obj.password)
            auth_title = lb_obj.title
        async with _SOCIAL_LOCK:
            target_sess = _ONLINE_USERS.get(target_uid)
        if not target_sess or not target_sess.channel_name:
            await self.send_json(
                {
                    "event": "notification",
                    "kind": "error",
                    "title": "PLAYER OFFLINE",
                    "message": "Target player is currently offline.",
                }
            )
            return
        await self.channel_layer.send(
            target_sess.channel_name,
            {
                "type": "social.event",
                "payload": {
                    "event": "lobby_invite_received",
                    "lobby_id": lobby_id,
                    "lobby_title": auth_title,
                    "has_password": has_password,
                    "from_user_id": self.player.user_id,
                    "from_username": self.player.username,
                },
            },
        )
        await self.send_json(
            {
                "event": "notification",
                "kind": "info",
                "title": "LOBBY INVITE SENT",
                "message": f"Invited {target_sess.username} to lobby.",
            }
        )

    async def social_event(self, event: dict) -> None:
        await self.send_json(event["payload"])

    async def queue_status(self, event: dict) -> None:
        await self.send_json(event["payload"])

    async def match_found(self, event: dict) -> None:
        await self.send_json(event["payload"])