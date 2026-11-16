import os
import signal
from datetime import timedelta

from django.contrib import admin, messages
from django.db.models import Count, Q
from django.http import HttpRequest
from django.utils import timezone
from django.utils.html import format_html

from .consumers import (
    get_game_server_unit_name,
    is_udp_port_bound,
    stop_game_server_service,
    store_ticket_in_redis,
)
from .models import (
    GameMatchRoom,
    MatchParticipant,
    OtpVerification,
    Player,
    PlayerSessionToken,
    compute_rank_tier_from_mmr,
)

# عنوان‌های اختصاصی پنل مدیریت جنگو
admin.site.site_header = "پنل مدیریت سرورهای سیمرغ (Simorgh MOBA Admin)"
admin.site.site_title = "مدیریت بازی سیمرغ"
admin.site.index_title = "داشبورد مدیریت بازیکنان، رنکینگ، نشست‌ها و اتاق‌های نبرد"


# ============================================================================
# ۱. جداول Inline برای مشاهده سریع زیرمجموعه‌ها در صفحات جزئیات
# ============================================================================


class PlayerSessionTokenInline(admin.TabularInline):
    model = PlayerSessionToken
    extra = 0
    fields = ("access_token", "refresh_token", "is_active", "created_at", "expires_at")
    readonly_fields = ("created_at",)
    show_change_link = True
    verbose_name = "توکن نشست"
    verbose_name_plural = "توکن‌های نشست این بازیکن"


class PlayerMatchParticipationInline(admin.TabularInline):
    model = MatchParticipant
    extra = 0
    fields = (
        "match_room",
        "team_id",
        "slot_index",
        "status",
        "hero_id",
        "level",
        "kills",
        "deaths",
        "assists",
        "mmr_delta",
        "is_winner",
        "ticket_id",
        "joined_at",
    )
    readonly_fields = ("joined_at",)
    autocomplete_fields = ("match_room",)
    show_change_link = True
    verbose_name = "مشارکت در مسابقه"
    verbose_name_plural = "تاریخچه مسابقات این بازیکن"


class RoomParticipantsInline(admin.TabularInline):
    model = MatchParticipant
    extra = 0
    fields = (
        "player",
        "team_id",
        "slot_index",
        "status",
        "hero_id",
        "level",
        "kills",
        "deaths",
        "assists",
        "last_hits",
        "denies",
        "gold_coins",
        "mmr_delta",
        "is_winner",
        "ticket_id",
        "joined_at",
    )
    readonly_fields = ("joined_at",)
    autocomplete_fields = ("player",)
    show_change_link = True
    verbose_name = "بازیکن حاضر در این اتاق"
    verbose_name_plural = "بازیکنان حاضر در این اتاق نبرد"


# ============================================================================
# ۲. پنل مدیریت بازیکنان و رنکینگ (PlayerAdmin)
# ============================================================================


@admin.register(Player)
class PlayerAdmin(admin.ModelAdmin):
    list_display = (
        "id",
        "user_id",
        "username",
        "email",
        "mmr",
        "rank_tier",
        "mmr_rank_badge",
        "wins",
        "losses",
        "abandoned_count",
        "win_rate_badge",
        "matches_played_count",
        "is_banned",
        "ban_status_badge",
        "last_login_at",
        "created_at",
    )
    list_display_links = ("id", "user_id", "username")
    list_editable = ("mmr", "rank_tier", "wins", "losses", "abandoned_count", "is_banned")
    search_fields = ("user_id", "username", "email", "rank_tier")
    list_filter = ("is_banned", "rank_tier", "created_at", "last_login_at")
    readonly_fields = ("created_at", "last_login_at", "win_rate_badge")
    ordering = ("-mmr", "-created_at")
    list_per_page = 30
    inlines = [PlayerMatchParticipationInline, PlayerSessionTokenInline]

    fieldsets = (
        (
            "اطلاعات هویتی بازیکن",
            {
                "fields": ("user_id", "username", "email", "is_banned"),
            },
        ),
        (
            "رنکینگ و آمار نبردها (MMR & Match Stats)",
            {
                "fields": (
                    "mmr",
                    "rank_tier",
                    "wins",
                    "losses",
                    "abandoned_count",
                    "win_rate_badge",
                ),
            },
        ),
        (
            "اطلاعات زمانی",
            {
                "fields": ("created_at", "last_login_at"),
            },
        ),
    )

    actions = [
        "action_ban_players",
        "action_unban_players",
        "action_add_25_mmr",
        "action_subtract_25_mmr",
        "action_reset_mmr_1000",
        "action_revoke_player_tokens",
    ]

    def get_queryset(self, request: HttpRequest):
        qs = super().get_queryset(request)
        return qs.annotate(_matches_count=Count("match_participations"))

    @admin.display(description="نشان رنک و MMR", ordering="mmr")
    def mmr_rank_badge(self, obj: Player) -> str:
        color = "#3b82f6"
        if obj.mmr >= 2000:
            color = "#f59e0b"
        elif obj.mmr >= 1500:
            color = "#8b5cf6"
        elif obj.mmr < 1000:
            color = "#6b7280"
        return format_html(
            '<span style="background:{}; color:#fff; padding:3px 8px; border-radius:6px; font-weight:bold; font-size:12px;">{} ({})</span>',
            color,
            obj.rank_tier,
            obj.mmr,
        )

    @admin.display(description="نرخ پیروزی (Win Rate)")
    def win_rate_badge(self, obj: Player) -> str:
        total = obj.wins + obj.losses
        if total == 0:
            return "بدون بازی (0%)"
        pct = round((obj.wins * 100.0) / total, 1)
        color = "#10b981" if pct >= 50.0 else "#ef4444"
        return format_html(
            '<strong style="color:{};">{}% ({} از {})</strong>',
            color,
            pct,
            obj.wins,
            total,
        )

    @admin.display(description="مچ‌های ثبت‌شده", ordering="_matches_count")
    def matches_played_count(self, obj: Player) -> int:
        return getattr(obj, "_matches_count", 0)

    @admin.display(description="وضعیت حساب", ordering="is_banned")
    def ban_status_badge(self, obj: Player) -> str:
        if obj.is_banned:
            return format_html(
                '<span style="background:#dc2626; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">مسدود (BANNED)</span>'
            )
        return format_html(
            '<span style="background:#16a34a; color:#fff; padding:2px 8px; border-radius:4px;">فعال</span>'
        )

    @admin.action(description="🚫 مسدود کردن (Ban) بازیکنان انتخاب‌شده و قطع نشست آن‌ها")
    def action_ban_players(self, request: HttpRequest, queryset):
        updated = queryset.update(is_banned=True)
        PlayerSessionToken.objects.filter(player__in=queryset).update(is_active=False)
        self.message_user(
            request,
            f"{updated} بازیکن مسدود شدند و توکن‌های نشست آن‌ها باطل گردید.",
            messages.WARNING,
        )

    @admin.action(description="✅ رفع مسدودیت (Unban) بازیکنان انتخاب‌شده")
    def action_unban_players(self, request: HttpRequest, queryset):
        updated = queryset.update(is_banned=False)
        self.message_user(
            request, f"مسدودیت {updated} بازیکن برطرف شد.", messages.SUCCESS
        )

    @admin.action(description="⬆️ افزایش ۲۵ واحدی MMR (+25 MMR) و بروزرسانی Rank Tier")
    def action_add_25_mmr(self, request: HttpRequest, queryset):
        count = 0
        for p in queryset:
            p.mmr += 25
            p.rank_tier = compute_rank_tier_from_mmr(p.mmr)
            p.save(update_fields=["mmr", "rank_tier"])
            count += 1
        self.message_user(
            request, f"به {count} بازیکن ۲۵ امتیاز MMR اضافه شد.", messages.SUCCESS
        )

    @admin.action(description="⬇️ کاهش ۲۵ واحدی MMR (-25 MMR) و بروزرسانی Rank Tier")
    def action_subtract_25_mmr(self, request: HttpRequest, queryset):
        count = 0
        for p in queryset:
            p.mmr = max(0, p.mmr - 25)
            p.rank_tier = compute_rank_tier_from_mmr(p.mmr)
            p.save(update_fields=["mmr", "rank_tier"])
            count += 1
        self.message_user(
            request, f"از {count} بازیکن ۲۵ امتیاز MMR کسر شد.", messages.SUCCESS
        )

    @admin.action(description="🔄 بازنشانی امتیاز MMR به ۱۰۰۰ (پیش‌فرض)")
    def action_reset_mmr_1000(self, request: HttpRequest, queryset):
        updated = queryset.update(mmr=1000, rank_tier="Simorgh Initiate")
        self.message_user(
            request,
            f"امتیاز MMR برای {updated} بازیکن به ۱۰۰۰ بازنشانی شد.",
            messages.SUCCESS,
        )

    @admin.action(description="🔑 باطل کردن تمام توکن‌های ورود بازیکنان انتخاب‌شده (Force Logout)")
    def action_revoke_player_tokens(self, request: HttpRequest, queryset):
        revoked = PlayerSessionToken.objects.filter(
            player__in=queryset, is_active=True
        ).update(is_active=False)
        self.message_user(
            request,
            f"{revoked} توکن فعال باطل شد.",
            messages.INFO,
        )


# ============================================================================
# ۳. پنل مدیریت اتاق‌های نبرد و سرورهای بازی (GameMatchRoomAdmin)
# ============================================================================


@admin.register(GameMatchRoom)
class GameMatchRoomAdmin(admin.ModelAdmin):
    list_display = (
        "id",
        "room_id",
        "status",
        "status_badge",
        "winner_badge",
        "duration_display",
        "live_udp_socket_badge",
        "server_host",
        "server_port",
        "max_players",
        "participants_summary",
        "process_pid",
        "created_at",
        "finished_at",
    )
    list_display_links = ("id", "room_id")
    list_editable = ("status",)
    search_fields = (
        "room_id",
        "server_host",
        "server_port",
        "process_pid",
        "participants__player__username",
        "participants__player__email",
        "participants__ticket_id",
    )
    list_filter = ("status", "winner_team", "max_players", "server_host", "created_at")
    readonly_fields = ("created_at", "live_udp_socket_badge")
    ordering = ("-created_at",)
    list_per_page = 30
    inlines = [RoomParticipantsInline]

    actions = [
        "action_force_stop_and_finish_rooms",
        "action_refresh_room_tickets_in_redis",
        "action_sync_room_status_with_udp_ports",
    ]

    def get_queryset(self, request: HttpRequest):
        qs = super().get_queryset(request)
        return qs.annotate(_part_count=Count("participants"))

    @admin.display(description="وضعیت اتاق", ordering="status")
    def status_badge(self, obj: GameMatchRoom) -> str:
        if obj.status == GameMatchRoom.STATUS_RUNNING:
            return format_html(
                '<span style="background:#0284c7; color:#fff; padding:3px 8px; border-radius:5px; font-weight:bold;">🔥 در حال اجرا (Running)</span>'
            )
        if obj.status == GameMatchRoom.STATUS_WAITING:
            return format_html(
                '<span style="background:#d97706; color:#fff; padding:3px 8px; border-radius:5px;">⏳ در انتظار</span>'
            )
        if obj.status == GameMatchRoom.STATUS_CANCELED:
            return format_html(
                '<span style="background:#dc2626; color:#fff; padding:3px 8px; border-radius:5px;">🚫 لغوشده / Abandon</span>'
            )
        return format_html(
            '<span style="background:#4b5563; color:#fff; padding:3px 8px; border-radius:5px;">🏁 پایان‌یافته</span>'
        )

    @admin.display(description="تیم پیروز", ordering="winner_team")
    def winner_badge(self, obj: GameMatchRoom) -> str:
        if obj.winner_team == 1:
            return format_html(
                '<span style="background:#0891b2; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">🏆 تیم ۱ (روشنایی)</span>'
            )
        if obj.winner_team == 2:
            return format_html(
                '<span style="background:#e11d48; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">🏆 تیم ۲ (تاریکی)</span>'
            )
        return "—"

    @admin.display(description="مدت نبرد", ordering="duration_sec")
    def duration_display(self, obj: GameMatchRoom) -> str:
        if not obj.duration_sec:
            return "—"
        mins = obj.duration_sec // 60
        secs = obj.duration_sec % 60
        return f"{mins:02d}:{secs:02d}"

    @admin.display(description="وضعیت زنده پورت UDP")
    def live_udp_socket_badge(self, obj: GameMatchRoom) -> str:
        bound = is_udp_port_bound(obj.server_port)
        if bound:
            return format_html(
                '<span style="color:#16a34a; font-weight:bold;">🟢 پورت {} فعال (Listening)</span>',
                obj.server_port,
            )
        return format_html(
            '<span style="color:#9ca3af;">⚪ پورت {} آزاد (بسته)</span>',
            obj.server_port,
        )

    @admin.display(description="بازیکنان اتاق", ordering="_part_count")
    def participants_summary(self, obj: GameMatchRoom) -> str:
        cnt = getattr(obj, "_part_count", 0)
        return f"{cnt} از {obj.max_players} بازیکن"

    @admin.action(description="🛑 توقف سرویس systemd سرور بازی (systemctl stop) و ثبت پایان اتاق")
    def action_force_stop_and_finish_rooms(self, request: HttpRequest, queryset):
        stopped = 0
        for room in queryset:
            stop_game_server_service(
                port=room.server_port,
                room_id=room.room_id,
                player_count=room.max_players,
            )
            if room.process_pid:
                try:
                    os.kill(room.process_pid, signal.SIGTERM)
                except Exception:
                    pass
            room.status = GameMatchRoom.STATUS_FINISHED
            room.save(update_fields=["status"])
            stopped += 1

        self.message_user(
            request,
            f"سرویس systemd برای {stopped} اتاق نبرد متوقف شد و وضعیت آن‌ها به پایان‌یافته (Finished) تغییر یافت.",
            messages.WARNING,
        )

    @admin.action(description="🔄 بازنویسی و تمدید تیکت‌های Redis برای بازیکنان اتاق‌های انتخاب‌شده")
    def action_refresh_room_tickets_in_redis(self, request: HttpRequest, queryset):
        ticket_count = 0
        participants = MatchParticipant.objects.select_related(
            "match_room", "player"
        ).filter(match_room__in=queryset)
        for part in participants:
            store_ticket_in_redis(
                ticket_id=part.ticket_id,
                room_id=part.match_room.room_id,
                user_id=part.player.user_id,
                username=part.player.username,
            )
            ticket_count += 1
        self.message_user(
            request,
            f"{ticket_count} تیکت بازیکن در Redis با موفقیت بازنویسی و به مدت ۲ ساعت تمدید شد.",
            messages.SUCCESS,
        )

    @admin.action(description="🔍 بررسی زنده پورت‌های UDP و همگام‌سازی خودکار وضعیت اتاق‌ها")
    def action_sync_room_status_with_udp_ports(self, request: HttpRequest, queryset):
        closed_count = 0
        for room in queryset.filter(status=GameMatchRoom.STATUS_RUNNING):
            if not is_udp_port_bound(room.server_port):
                room.status = GameMatchRoom.STATUS_FINISHED
                room.save(update_fields=["status"])
                closed_count += 1
        self.message_user(
            request,
            f"وضعیت پورت‌ها بررسی شد؛ {closed_count} اتاق که سرور آن‌ها بسته شده بود به وضعیت Finished تغییر یافت.",
            messages.INFO,
        )


# ============================================================================
# ۴. پنل مدیریت شرکت‌کنندگان و تیکت‌های نبرد (MatchParticipantAdmin)
# ============================================================================


@admin.register(MatchParticipant)
class MatchParticipantAdmin(admin.ModelAdmin):
    list_display = (
        "id",
        "match_room",
        "player",
        "participant_status_badge",
        "team_badge",
        "slot_index",
        "hero_id",
        "level",
        "kda_summary",
        "cs_summary",
        "gold_coins",
        "mmr_delta_badge",
        "ticket_id",
        "joined_at",
    )
    list_display_links = ("id", "ticket_id")
    search_fields = (
        "ticket_id",
        "match_room__room_id",
        "player__username",
        "player__email",
        "player__user_id",
    )
    list_filter = (
        "status",
        "is_winner",
        "team_id",
        "hero_id",
        "match_room__status",
        "joined_at",
    )
    autocomplete_fields = ("match_room", "player")
    readonly_fields = ("joined_at",)
    ordering = ("-joined_at",)
    list_per_page = 30

    actions = ["action_repush_tickets_to_redis"]

    @admin.display(description="وضعیت بازیکن", ordering="status")
    def participant_status_badge(self, obj: MatchParticipant) -> str:
        if obj.status == MatchParticipant.STATUS_CONNECTED:
            return format_html(
                '<span style="background:#16a34a; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">🟢 متصل (Connected)</span>'
            )
        if obj.status == MatchParticipant.STATUS_DISCONNECTED:
            return format_html(
                '<span style="background:#d97706; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">🟡 دیسکانکت موقت (Disconnected)</span>'
            )
        if obj.status == MatchParticipant.STATUS_ABANDONED:
            return format_html(
                '<span style="background:#dc2626; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">🚫 ترک بازی (Abandoned)</span>'
            )
        if obj.status == MatchParticipant.STATUS_COMPLETED:
            return format_html(
                '<span style="background:#2563eb; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">🏁 تکمیل‌شده (Completed)</span>'
            )
        return obj.get_status_display()

    @admin.display(description="تیم", ordering="team_id")
    def team_badge(self, obj: MatchParticipant) -> str:
        if obj.team_id == 1:
            return format_html(
                '<span style="background:#0891b2; color:#fff; padding:2px 8px; border-radius:4px;">تیم ۱ (روشنایی)</span>'
            )
        return format_html(
            '<span style="background:#e11d48; color:#fff; padding:2px 8px; border-radius:4px;">تیم ۲ (تاریکی)</span>'
        )

    @admin.display(description="K / D / A")
    def kda_summary(self, obj: MatchParticipant) -> str:
        return f"{obj.kills} / {obj.deaths} / {obj.assists}"

    @admin.display(description="LH / DN")
    def cs_summary(self, obj: MatchParticipant) -> str:
        return f"{obj.last_hits} / {obj.denies}"

    @admin.display(description="تغییر MMR", ordering="mmr_delta")
    def mmr_delta_badge(self, obj: MatchParticipant) -> str:
        if obj.mmr_delta > 0:
            return format_html(
                '<strong style="color:#16a34a;">+{}</strong>', obj.mmr_delta
            )
        if obj.mmr_delta < 0:
            return format_html(
                '<strong style="color:#dc2626;">{}</strong>', obj.mmr_delta
            )
        return "0"

    @admin.action(description="🔄 ثبت و تمدید مجدد تیکت‌های انتخاب‌شده در Redis")
    def action_repush_tickets_to_redis(self, request: HttpRequest, queryset):
        count = 0
        for part in queryset.select_related("match_room", "player"):
            store_ticket_in_redis(
                ticket_id=part.ticket_id,
                room_id=part.match_room.room_id,
                user_id=part.player.user_id,
                username=part.player.username,
            )
            count += 1
        self.message_user(
            request,
            f"{count} تیکت با موفقیت در Redis ثبت و تمدید شد.",
            messages.SUCCESS,
        )


# ============================================================================
# ۵. پنل مدیریت توکن‌های نشست بازیکنان (PlayerSessionTokenAdmin)
# ============================================================================


@admin.register(PlayerSessionToken)
class PlayerSessionTokenAdmin(admin.ModelAdmin):
    list_display = (
        "id",
        "player",
        "short_access_token",
        "is_active",
        "token_status_badge",
        "created_at",
        "expires_at",
    )
    list_display_links = ("id", "short_access_token")
    list_editable = ("is_active",)
    search_fields = (
        "player__username",
        "player__email",
        "player__user_id",
        "access_token",
        "refresh_token",
    )
    list_filter = ("is_active", "created_at", "expires_at")
    autocomplete_fields = ("player",)
    readonly_fields = ("created_at",)
    ordering = ("-created_at",)
    list_per_page = 30

    actions = ["action_revoke_tokens", "action_extend_tokens_30d"]

    @admin.display(description="توکن دسترسی (Access Token)")
    def short_access_token(self, obj: PlayerSessionToken) -> str:
        return f"{obj.access_token[:24]}..."

    @admin.display(description="اعتبار نشست")
    def token_status_badge(self, obj: PlayerSessionToken) -> str:
        now = timezone.now()
        if not obj.is_active:
            return format_html(
                '<span style="color:#dc2626; font-weight:bold;">❌ باطل‌شده</span>'
            )
        if obj.expires_at <= now:
            return format_html(
                '<span style="color:#d97706; font-weight:bold;">⌛ منقضی‌شده</span>'
            )
        return format_html(
            '<span style="color:#16a34a; font-weight:bold;">✅ معتبر و فعال</span>'
        )

    @admin.action(description="🚫 غیرفعال و باطل کردن توکن‌های انتخاب‌شده")
    def action_revoke_tokens(self, request: HttpRequest, queryset):
        updated = queryset.update(is_active=False)
        self.message_user(
            request, f"{updated} توکن نشست غیرفعال شد.", messages.WARNING
        )

    @admin.action(description="⏳ تمدید اعتبار توکن‌های انتخاب‌شده به مدت ۳۰ روز")
    def action_extend_tokens_30d(self, request: HttpRequest, queryset):
        new_exp = timezone.now() + timedelta(days=30)
        updated = queryset.update(is_active=True, expires_at=new_exp)
        self.message_user(
            request,
            f"اعتبار {updated} توکن نشست تا ۳۰ روز آینده تمدید شد.",
            messages.SUCCESS,
        )


# ============================================================================
# ۶. پنل مدیریت کدهای یک‌بارمصرف تایید ایمیل (OtpVerificationAdmin)
# ============================================================================


@admin.register(OtpVerification)
class OtpVerificationAdmin(admin.ModelAdmin):
    list_display = (
        "id",
        "email",
        "otp_code",
        "is_used",
        "otp_validity_badge",
        "created_at",
        "expires_at",
    )
    list_display_links = ("id", "email", "otp_code")
    search_fields = ("email", "otp_code")
    list_filter = ("is_used", "created_at", "expires_at")
    readonly_fields = ("created_at",)
    ordering = ("-created_at",)
    list_per_page = 30

    actions = ["action_mark_otp_used"]

    @admin.display(description="وضعیت اعتبار کد")
    def otp_validity_badge(self, obj: OtpVerification) -> str:
        if obj.is_used:
            return format_html(
                '<span style="color:#6b7280;">استفاده‌شده / باطل</span>'
            )
        if timezone.now() > obj.expires_at:
            return format_html(
                '<span style="color:#ef4444;">منقضی‌شده</span>'
            )
        return format_html(
            '<span style="background:#16a34a; color:#fff; padding:2px 8px; border-radius:4px; font-weight:bold;">آماده استفاده ({})</span>',
            obj.otp_code,
        )

    @admin.action(description="🚫 باطل کردن کدهای OTP انتخاب‌شده")
    def action_mark_otp_used(self, request: HttpRequest, queryset):
        updated = queryset.update(is_used=True)
        self.message_user(
            request, f"{updated} کد OTP باطل شد.", messages.INFO
        )
