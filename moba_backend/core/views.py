import hmac
import json
import re
import secrets
from datetime import timedelta
from typing import Optional

from django.conf import settings
from django.db import transaction
from django.db.models import Q
from django.http import HttpRequest, JsonResponse
from django.utils import timezone
from django.views.decorators.csrf import csrf_exempt
from django.views.decorators.http import require_GET, require_POST

from .consumers import (
    abandon_active_matches_for_player,
    build_friends_payload_for_player,
    get_player_presence_snapshot,
    push_social_event_to_user_sync,
    disconnect_active_match_for_player,
    get_active_match_for_player,
    reconnect_active_match_for_player,
    record_match_result_from_server,
    update_player_connection_state_from_server,
)
from .models import (
    Friendship,
    GameMatchRoom,
    MatchParticipant,
    OtpVerification,
    Player,
    PlayerSessionToken,
)


def _is_internal_webhook_authorized(request: HttpRequest) -> bool:
    configured_secret = str(getattr(settings, "INTERNAL_WEBHOOK_SECRET", "") or "").strip()
    if configured_secret:
        provided = str(request.headers.get("X-Internal-Secret", "") or "").strip()
        return bool(provided) and hmac.compare_digest(provided, configured_secret)
    remote_addr = str(request.META.get("REMOTE_ADDR", "") or "").strip()
    return remote_addr in ("127.0.0.1", "::1", "localhost")


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


USERNAME_REGEX = re.compile(r"^[A-Za-z][A-Za-z0-9_-]{2,23}$")


def _validate_username(username: str) -> Optional[str]:
    if len(username) < 3 or len(username) > 24:
        return "Username must be between 3 and 24 characters."
    if not USERNAME_REGEX.match(username):
        return "Username must start with a letter and use only letters, digits, _ or -."
    if Player.objects.filter(username__iexact=username).exists():
        return "This username is already taken."
    return None


@csrf_exempt
@require_POST
def request_otp_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/auth/request-otp/
    پشتیبانی از دو حالت مجزای Login و Register همراه با اعتبارسنجی نام کاربری در ثبت‌نام
    """
    data = _parse_json_body(request)
    email = str(data.get("email", "")).strip().lower()
    mode = str(data.get("mode", "login")).strip().lower()
    username = str(data.get("username", "")).strip()

    if not email or "@" not in email or "." not in email.split("@")[-1] or len(email) < 5:
        return JsonResponse(
            {"error": "Please enter a valid email address."}, status=400
        )

    existing_player = Player.objects.filter(email=email).first()
    if mode == "register":
        if existing_player:
            return JsonResponse(
                {"error": "An account with this email already exists. Please use Log In."},
                status=400,
            )
        err = _validate_username(username)
        if err:
            return JsonResponse({"error": err}, status=400)
    else:
        if not existing_player:
            return JsonResponse(
                {"error": "No account found for this email. Please Register first."},
                status=404,
            )
        if existing_player.is_banned:
            return JsonResponse(
                {"error": "Your account has been suspended."}, status=403
            )
        username = ""

    cooldown_secs = int(getattr(settings, "OTP_COOLDOWN_SECS", 15))
    recent_cutoff = timezone.now() - timedelta(seconds=cooldown_secs)
    if OtpVerification.objects.filter(email=email, created_at__gt=recent_cutoff).exists():
        return JsonResponse(
            {"error": f"Please wait {cooldown_secs} seconds before requesting another OTP."},
            status=429,
        )

    # باطل کردن کدهای قبلی استفاده‌نشده این ایمیل
    OtpVerification.objects.filter(email=email, is_used=False).update(is_used=True)

    otp_code = f"{secrets.randbelow(900000) + 100000}"
    expires_at = timezone.now() + timedelta(minutes=int(getattr(settings, "OTP_TTL_MINUTES", 5)))

    OtpVerification.objects.create(
        email=email,
        otp_code=otp_code,
        username=username,
        is_used=False,
        expires_at=expires_at,
    )

    print(
        f"\n========================================\n"
        f"[AUTH OTP] Mode: {mode.upper()} | Email: {email} | Username: {username or '-'} | Code: {otp_code}\n"
        f"========================================\n",
        flush=True,
    )

    return JsonResponse(
        {
            "status": "ok",
            "message": "OTP code sent.",
            "email": email,
            "mode": mode,
        },
        status=200,
    )


@csrf_exempt
@require_POST
def verify_otp_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/auth/verify-otp/
    بررسی کد ۶ رقمی برای ورود (Login) یا ثبت‌نام با نام کاربری انتخابی (Register)
    """
    data = _parse_json_body(request)
    email = str(data.get("email", "")).strip().lower()
    otp_code = str(data.get("otp_code", "")).strip()

    if not email or len(otp_code) != 6:
        return JsonResponse(
            {"error": "Invalid email or 6-digit OTP code."}, status=400
        )

    with transaction.atomic():
        otp_record = (
            OtpVerification.objects.select_for_update()
            .filter(email=email, otp_code=otp_code, is_used=False)
            .order_by("-created_at")
            .first()
        )

        if not otp_record or not otp_record.is_valid():
            return JsonResponse(
                {"error": "Invalid or expired OTP code."}, status=400
            )

        chosen_username = (otp_record.username or "").strip()
        existing_player = Player.objects.select_for_update().filter(email=email).first()
        if chosen_username:
            if existing_player:
                return JsonResponse(
                    {"error": "Account already exists. Please log in."}, status=400
                )
            err = _validate_username(chosen_username)
            if err:
                return JsonResponse({"error": err}, status=400)
            player = Player.objects.create(
                email=email,
                username=chosen_username,
                mmr=1000,
                rank_tier="Simorgh Initiate",
            )
            created = True
        else:
            if not existing_player:
                return JsonResponse(
                    {"error": "Account not found. Please register first."}, status=404
                )
            player = existing_player
            created = False

        otp_record.is_used = True
        otp_record.save(update_fields=["is_used"])

        if player.is_banned:
            return JsonResponse(
                {"error": "Your account has been suspended."}, status=403
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
            "wins": player.wins,
            "losses": player.losses,
            "abandoned_count": player.abandoned_count,
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
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

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
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

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
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

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
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

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
    if not _is_internal_webhook_authorized(request):
        return JsonResponse({"error": "Forbidden internal webhook."}, status=403)
    data = _parse_json_body(request)
    if not data.get("room_id") and not data.get("port"):
        return JsonResponse({"error": "room_id or port is required."}, status=400)

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
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

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
    if not _is_internal_webhook_authorized(request):
        return JsonResponse({"error": "Forbidden internal webhook."}, status=403)
    data = _parse_json_body(request)
    if not data.get("room_id"):
        return JsonResponse({"error": "room_id is required."}, status=400)

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
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

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
                    "party_group_id": getattr(rp, "party_group_id", ""),
                    "is_me": rp.player_id == player.id,
                }
            )

        matches_list.append(
            {
                "room_id": room.room_id,
                "is_custom_lobby": getattr(room, "is_custom_lobby", False),
                "is_party_match": bool(getattr(part, "party_group_id", "")),
                "party_group_id": getattr(part, "party_group_id", ""),
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


# ============================================================================
# اندپوینت‌های HTTP سیستم دوست‌یابی و جستجوی بازیکنان (Friends REST API)
# ============================================================================


@require_GET
def friends_list_view(request: HttpRequest) -> JsonResponse:
    """
    GET /api/v1/friends/
    دریافت لیست دوستان (با وضعیت آنلاین زنده)، درخواست‌های ورودی و درخواست‌های خروجی
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

    payload = build_friends_payload_for_player(player)
    return JsonResponse(payload, status=200)


@require_GET
def friends_search_view(request: HttpRequest) -> JsonResponse:
    """
    GET /api/v1/friends/search/?q=<username_or_user_id>
    جستجوی بازیکنان برای ارسال درخواست دوستی
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

    q = str(request.GET.get("q", "")).strip()
    if len(q) < 2:
        return JsonResponse({"results": []}, status=200)

    candidates = list(
        Player.objects.filter(
            Q(username__icontains=q) | Q(user_id__icontains=q),
            is_banned=False,
        )
        .exclude(id=player.id)
        .order_by("-mmr")[:15]
    )

    candidate_ids = [c.id for c in candidates]
    existing = Friendship.objects.filter(
        (Q(requester=player) & Q(addressee_id__in=candidate_ids))
        | (Q(addressee=player) & Q(requester_id__in=candidate_ids))
    )
    rel_map = {}
    for f in existing:
        other_id = f.addressee_id if f.requester_id == player.id else f.requester_id
        if f.status == Friendship.STATUS_ACCEPTED:
            rel_map[other_id] = "friend"
        elif f.status == Friendship.STATUS_PENDING:
            rel_map[other_id] = (
                "outgoing_pending" if f.requester_id == player.id else "incoming_pending"
            )
        else:
            rel_map[other_id] = f.status

    results = []
    for c in candidates:
        pres = get_player_presence_snapshot(c.user_id)
        results.append(
            {
                "user_id": c.user_id,
                "username": c.username,
                "mmr": c.mmr,
                "rank_tier": c.rank_tier,
                "online": pres["online"],
                "relationship": rel_map.get(c.id, "none"),
            }
        )

    return JsonResponse({"results": results}, status=200)


@csrf_exempt
@require_POST
def send_friend_request_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/friends/request/
    body: {"target_user_id": "..."} یا {"username": "..."}
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

    data = _parse_json_body(request)
    target_uid = str(data.get("target_user_id", "")).strip()
    target_username = str(data.get("username", "")).strip()

    target = None
    if target_uid:
        target = Player.objects.filter(user_id=target_uid, is_banned=False).first()
    elif target_username:
        target = Player.objects.filter(username__iexact=target_username, is_banned=False).first()

    if not target:
        return JsonResponse({"error": "Player not found"}, status=404)
    if target.id == player.id:
        return JsonResponse({"error": "Cannot add yourself"}, status=400)

    existing = Friendship.objects.filter(
        (Q(requester=player) & Q(addressee=target))
        | (Q(requester=target) & Q(addressee=player))
    ).first()

    if existing:
        if existing.status == Friendship.STATUS_BLOCKED:
            return JsonResponse({"error": "Unable to send friend request."}, status=403)
        if existing.status == Friendship.STATUS_ACCEPTED:
            return JsonResponse({"error": "Already friends"}, status=400)
        if existing.status == Friendship.STATUS_PENDING:
            if existing.requester_id == target.id:
                # طرف مقابل قبلاً درخواست داده بود -> تایید خودکار
                existing.status = Friendship.STATUS_ACCEPTED
                existing.save(update_fields=["status", "updated_at"])
                push_social_event_to_user_sync(
                    target.user_id,
                    {
                        "event": "notification",
                        "kind": "friend_accepted",
                        "title": "FRIEND ACCEPTED",
                        "message": f"{player.username} accepted your friend request.",
                    },
                )
                push_social_event_to_user_sync(
                    target.user_id,
                    {"event": "friends_updated", **build_friends_payload_for_player(target)},
                )
                return JsonResponse(
                    {
                        "status": "accepted",
                        "message": f"You are now friends with {target.username}.",
                        **build_friends_payload_for_player(player),
                    },
                    status=200,
                )
            return JsonResponse({"error": "Friend request already pending"}, status=400)
        # درخواست قبلی رد شده بود -> ارسال مجدد
        existing.requester = player
        existing.addressee = target
        existing.status = Friendship.STATUS_PENDING
        existing.save(update_fields=["requester", "addressee", "status", "updated_at"])
        fr_obj = existing
    else:
        fr_obj = Friendship.objects.create(
            requester=player,
            addressee=target,
            status=Friendship.STATUS_PENDING,
        )

    push_social_event_to_user_sync(
        target.user_id,
        {
            "event": "notification",
            "kind": "friend_request",
            "title": "FRIEND REQUEST",
            "message": f"{player.username} sent you a friend request.",
            "request_id": fr_obj.id,
        },
    )
    push_social_event_to_user_sync(
        target.user_id,
        {"event": "friends_updated", **build_friends_payload_for_player(target)},
    )

    return JsonResponse(
        {
            "status": "pending",
            "message": f"Friend request sent to {target.username}.",
            **build_friends_payload_for_player(player),
        },
        status=200,
    )


@csrf_exempt
@require_POST
def respond_friend_request_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/friends/respond/
    body: {"request_id": 12, "accept": true}
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

    data = _parse_json_body(request)
    try:
        req_id = int(data.get("request_id", 0))
    except Exception:
        req_id = 0
    accept = bool(data.get("accept", False))

    fr = (
        Friendship.objects.select_related("requester", "addressee")
        .filter(id=req_id, addressee=player, status=Friendship.STATUS_PENDING)
        .first()
    )
    if not fr:
        return JsonResponse({"error": "Friend request not found"}, status=404)

    fr.status = Friendship.STATUS_ACCEPTED if accept else Friendship.STATUS_DECLINED
    fr.save(update_fields=["status", "updated_at"])

    if accept:
        push_social_event_to_user_sync(
            fr.requester.user_id,
            {
                "event": "notification",
                "kind": "friend_accepted",
                "title": "NEW FRIEND",
                "message": f"{player.username} accepted your friend request.",
            },
        )
    push_social_event_to_user_sync(
        fr.requester.user_id,
        {"event": "friends_updated", **build_friends_payload_for_player(fr.requester)},
    )

    return JsonResponse(
        {
            "status": fr.status,
            "message": (
                f"You are now friends with {fr.requester.username}."
                if accept
                else f"Declined request from {fr.requester.username}."
            ),
            **build_friends_payload_for_player(player),
        },
        status=200,
    )


@csrf_exempt
@require_POST
def remove_friend_view(request: HttpRequest) -> JsonResponse:
    """
    POST /api/v1/friends/remove/
    body: {"friend_user_id": "..."}
    """
    player = _resolve_authenticated_player(request)
    if not player:
        return JsonResponse({"error": "Invalid or expired session token."}, status=401)

    data = _parse_json_body(request)
    friend_uid = str(data.get("friend_user_id", "")).strip()
    if not friend_uid:
        return JsonResponse({"error": "friend_user_id is required"}, status=400)

    other = Player.objects.filter(user_id=friend_uid).first()
    if not other:
        return JsonResponse({"error": "Player not found"}, status=404)

    deleted_count, _ = Friendship.objects.filter(
        (Q(requester=player) & Q(addressee=other))
        | (Q(requester=other) & Q(addressee=player))
    ).delete()

    if deleted_count > 0:
        push_social_event_to_user_sync(
            other.user_id,
            {"event": "friends_updated", **build_friends_payload_for_player(other)},
        )

    return JsonResponse(
        {
            "status": "ok",
            "removed": deleted_count > 0,
            **build_friends_payload_for_player(player),
        },
        status=200,
    )