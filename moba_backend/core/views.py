import json
import random
from datetime import timedelta
from typing import Optional

from django.http import HttpRequest, JsonResponse
from django.utils import timezone
from django.views.decorators.csrf import csrf_exempt
from django.views.decorators.http import require_GET, require_POST

from .consumers import (
    abandon_active_matches_for_player,
    disconnect_active_match_for_player,
    get_active_match_for_player,
    reconnect_active_match_for_player,
    record_match_result_from_server,
    update_player_connection_state_from_server,
)
from .models import (
    GameMatchRoom,
    MatchParticipant,
    OtpVerification,
    Player,
    PlayerSessionToken,
)


def _parse_json_body(request: HttpRequest) -> dict:
    try:
        raw = request.body.decode("utf-8") if request.body else "{}"
        return json.loads(raw)
    except Exception:
        return {}


def _extract_bearer_token(request: HttpRequest) -> str:
    auth_header = request.headers.get("Authorization", "")
    if auth_header.startswith("Bearer "):
        return auth_header[7:].strip()
    return request.GET.get("token", "").strip()


def _resolve_authenticated_player(request: HttpRequest) -> Optional[Player]:
    token_str = _extract_bearer_token(request)
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


@csrf_exempt
@require_POST
def request_otp_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/auth/request-otp/
    دریافت ایمیل کاربر، ساخت کد ۶ رقمی OTP در دیتابیس و نمایش آن در لاگ سرور
    """
    data = _parse_json_body(request)
    email = str(data.get("email", "")).strip().lower()

    if not email or "@" not in email or len(email) < 5:
        return JsonResponse(
            {"error": "ایمیل واردشده معتبر نیست"}, status=400
        )

    # باطل کردن کدهای قبلی استفاده‌نشده این ایمیل
    OtpVerification.objects.filter(email=email, is_used=False).update(is_used=True)

    otp_code = f"{random.randint(100000, 999999)}"
    expires_at = timezone.now() + timedelta(minutes=5)

    OtpVerification.objects.create(
        email=email,
        otp_code=otp_code,
        is_used=False,
        expires_at=expires_at,
    )

    print(
        f"\n========================================\n"
        f"[AUTH OTP] Email: {email} | Code: {otp_code}\n"
        f"========================================\n",
        flush=True,
    )

    return JsonResponse(
        {
            "status": "ok",
            "message": "کد تایید ارسال شد",
            "email": email,
        },
        status=200,
    )


@csrf_exempt
@require_POST
def verify_otp_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/auth/verify-otp/
    بررسی کد ۶ رقمی، ثبت‌نام خودکار بازیکن در صورت جدید بودن، یا ورود بازیکن قبلی
    و صدور توکن نشست (access_token & refresh_token) + بررسی بازی فعال
    """
    data = _parse_json_body(request)
    email = str(data.get("email", "")).strip().lower()
    otp_code = str(data.get("otp_code", "")).strip()

    if not email or len(otp_code) != 6:
        return JsonResponse(
            {"error": "ایمیل یا کد ۶ رقمی نامعتبر است"}, status=400
        )

    otp_record = (
        OtpVerification.objects.filter(
            email=email, otp_code=otp_code, is_used=False
        )
        .order_by("-created_at")
        .first()
    )

    if not otp_record or not otp_record.is_valid():
        return JsonResponse(
            {"error": "کد واردشده نادرست یا منقضی شده است"}, status=400
        )

    otp_record.is_used = True
    otp_record.save(update_fields=["is_used"])

    default_username = email.split("@")[0][:32] or "player"
    player, created = Player.objects.get_or_create(
        email=email,
        defaults={
            "username": default_username,
            "mmr": 1000,
            "rank_tier": "Simorgh Initiate",
        },
    )

    if player.is_banned:
        return JsonResponse(
            {"error": "حساب کاربری شما مسدود شده است"}, status=403
        )

    player.last_login_at = timezone.now()
    player.save(update_fields=["last_login_at"])

    session_token = PlayerSessionToken.create_for_player(player)
    client_ip = request.META.get("REMOTE_ADDR", "")
    active_match = get_active_match_for_player(player, client_ip=client_ip)

    action_str = "REGISTER+LOGIN" if created else "LOGIN"
    print(
        f"[AUTH SUCCESS] {action_str}: {player.username} ({player.user_id}) | MMR: {player.mmr} | ActiveMatch: {bool(active_match)}",
        flush=True,
    )

    return JsonResponse(
        {
            "access_token": session_token.access_token,
            "refresh_token": session_token.refresh_token,
            "user_id": player.user_id,
            "username": player.username,
            "email": player.email,
            "mmr": player.mmr,
            "rank_tier": player.rank_tier,
            "is_new_player": created,
            "active_match": active_match,
        },
        status=200,
    )


@require_GET
def profile_view(request: HttpRequest) -> JsonResponse:
    """
    GET /api/v1/auth/me/
    اعتبارسنجی توکن فعلی بازیکن هنگام باز کردن بازی و بازگرداندن اطلاعات پروفایل، رنک و بازی فعال (در صورت وجود)
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "توکن نامعتبر یا منقضی است"}, status=401)

    client_ip = request.META.get("REMOTE_ADDR", "")
    active_match = get_active_match_for_player(player, client_ip=client_ip)
    return JsonResponse(
        {
            "user_id": player.user_id,
            "username": player.username,
            "email": player.email,
            "mmr": player.mmr,
            "rank_tier": player.rank_tier,
            "wins": player.wins,
            "losses": player.losses,
            "abandoned_count": player.abandoned_count,
            "active_match": active_match,
        },
        status=200,
    )


@require_GET
def active_match_view(request: HttpRequest) -> JsonResponse:
    """
    GET /api/v1/match/active/
    بررسی وجود بازی فعال برای بازیکن جهت نمایش دکمه Reconnect و Abandon در منوی اصلی کلاینت
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "توکن نامعتبر یا منقضی است"}, status=401)

    client_ip = request.META.get("REMOTE_ADDR", "")
    active_match = get_active_match_for_player(player, client_ip=client_ip)
    return JsonResponse(
        {
            "has_active_match": active_match is not None,
            "active_match": active_match,
        },
        status=200,
    )


@csrf_exempt
@require_POST
def disconnect_match_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/match/disconnect/
    مرحله اول خروج دو مرحله‌ای (مشابه Dota 2):
    بازیکن از داخل بازی صرفاً دیسکانکت می‌کند و به منوی اصلی بازمی‌گردد بدون اینکه بازی را Abandon کند.
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "توکن نامعتبر یا منقضی است"}, status=401)

    disconnected_count = disconnect_active_match_for_player(player)
    client_ip = request.META.get("REMOTE_ADDR", "")
    active_match = get_active_match_for_player(player, client_ip=client_ip)
    return JsonResponse(
        {
            "status": "ok",
            "disconnected_matches": disconnected_count,
            "active_match": active_match,
        },
        status=200,
    )


@csrf_exempt
@require_POST
def reconnect_match_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/match/reconnect/
    هنگامی که بازیکن روی دکمه Reconnect کلیک می‌کند یا ارتباط UDP او مجدداً با سرور لوپ بازی برقرار می‌شود،
    وضعیت او در دیتابیس از `disconnected` به `connected` بازمی‌گردد.
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "توکن نامعتبر یا منقضی است"}, status=401)

    reconnected_count = reconnect_active_match_for_player(player)
    client_ip = request.META.get("REMOTE_ADDR", "")
    active_match = get_active_match_for_player(player, client_ip=client_ip)
    return JsonResponse(
        {
            "status": "ok",
            "reconnected_matches": reconnected_count,
            "active_match": active_match,
        },
        status=200,
    )


@csrf_exempt
@require_POST
def update_player_connection_state_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/match/player-state/
    وب‌هوک داخلی فراخوانی‌شده توسط `moba_server` هر زمان که ارتباط UDP بازیکن در لوپ بازی
    برقرار/ریکانکت (`connected`) یا قطع (`disconnected`) می‌شود.
    """
    data = _parse_json_body(request)
    if not data.get("room_id") and not data.get("port"):
        return JsonResponse({"error": "فیلد room_id یا port الزامی است"}, status=400)

    result = update_player_connection_state_from_server(data)
    status_code = 200 if result.get("status") in ("ok", "ignored") else 404
    return JsonResponse(result, status=status_code)


@csrf_exempt
@require_POST
def abandon_match_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/match/abandon/
    مرحله دوم خروج دو مرحله‌ای (مشابه Dota 2 - Abandon):
    بازیکن در منوی اصلی گزینه «ترک نهایی نبرد (Abandon)» را انتخاب می‌کند.
    وضعیت او در بازی به `abandoned` تغییر کرده و دیگر امکان Reconnect به آن بازی را ندارد.
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "توکن نامعتبر یا منقضی است"}, status=401)

    result = abandon_active_matches_for_player(player)
    return JsonResponse(
        {
            "status": "ok",
            "closed_matches": result.get("abandoned_matches", 0),
            **result,
        },
        status=200,
    )


@csrf_exempt
@require_POST
def report_match_result_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/match/report-result/
    وب‌هوک داخلی فراخوانی‌شده توسط `moba_server` در پایان نبرد برای ذخیره نتیجه بازی،
    آمار بازیکنان (K/D/A, Hero, Level, Gold)، بروزرسانی MMR/Rank و بستن اتاق بازی.
    """
    data = _parse_json_body(request)
    if not data.get("room_id"):
        return JsonResponse({"error": "فیلد room_id الزامی است"}, status=400)

    result = record_match_result_from_server(data)
    status_code = 200 if result.get("status") == "ok" else 404
    return JsonResponse(result, status=status_code)


@require_GET
def match_history_view(request: HttpRequest) -> JsonResponse:
    """
    GET /api/v1/match/history/
    دریافت خلاصه کارنامه بازیکن و لیست کامل تاریخچه نبردها به همراه جدول بازیکنان هر مسابقه
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "توکن نامعتبر یا منقضی است"}, status=401)

    participations = list(
        MatchParticipant.objects.select_related("match_room")
        .filter(player=player)
        .exclude(match_room__status=GameMatchRoom.STATUS_WAITING)
        .order_by("-joined_at")[:60]
    )

    room_ids = [p.match_room_id for p in participations]
    all_room_participants = (
        MatchParticipant.objects.select_related("player")
        .filter(match_room_id__in=room_ids)
        .order_by("team_id", "slot_index")
    )

    participants_by_room = {}
    for rp in all_room_participants:
        participants_by_room.setdefault(rp.match_room_id, []).append(rp)

    matches_list = []
    total_kills = 0
    total_deaths = 0
    total_assists = 0

    for part in participations:
        room = part.match_room
        total_kills += part.kills
        total_deaths += part.deaths
        total_assists += part.assists

        if part.status == MatchParticipant.STATUS_ABANDONED:
            outcome = "abandoned"
        elif room.status == GameMatchRoom.STATUS_RUNNING:
            outcome = "running"
        elif room.winner_team in (1, 2):
            outcome = "win" if part.team_id == room.winner_team else "loss"
        else:
            outcome = "canceled"

        timestamp_dt = room.finished_at or part.left_at or room.created_at
        played_at_str = timezone.localtime(timestamp_dt).strftime("%Y/%m/%d - %H:%M")

        room_players = []
        for rp in participants_by_room.get(room.id, []):
            room_players.append(
                {
                    "user_id": rp.player.user_id,
                    "username": rp.player.username,
                    "team_id": rp.team_id,
                    "slot_index": rp.slot_index,
                    "hero_id": rp.hero_id,
                    "level": rp.level,
                    "kills": rp.kills,
                    "deaths": rp.deaths,
                    "assists": rp.assists,
                    "last_hits": rp.last_hits,
                    "denies": rp.denies,
                    "gold_coins": rp.gold_coins,
                    "status": rp.status,
                    "is_me": rp.player_id == player.id,
                }
            )

        matches_list.append(
            {
                "room_id": room.room_id,
                "is_custom_lobby": getattr(room, "is_custom_lobby", False),
                "outcome": outcome,
                "team_id": part.team_id,
                "winner_team": room.winner_team or 0,
                "hero_id": part.hero_id,
                "level": part.level,
                "kills": part.kills,
                "deaths": part.deaths,
                "assists": part.assists,
                "last_hits": part.last_hits,
                "denies": part.denies,
                "gold_coins": part.gold_coins,
                "mmr_delta": part.mmr_delta,
                "duration_sec": room.duration_sec,
                "played_at": played_at_str,
                "players": room_players,
            }
        )

    avg_kda = round(
        (total_kills + total_assists) / max(1, total_deaths), 2
    )

    return JsonResponse(
        {
            "summary": {
                "username": player.username,
                "mmr": player.mmr,
                "rank_tier": player.rank_tier,
                "wins": player.wins,
                "losses": player.losses,
                "abandoned_count": player.abandoned_count,
                "total_matches": player.total_matches,
                "win_rate_percent": player.win_rate_percent,
                "total_kills": total_kills,
                "total_deaths": total_deaths,
                "total_assists": total_assists,
                "avg_kda": avg_kda,
            },
            "matches": matches_list,
        },
        status=200,
    )
