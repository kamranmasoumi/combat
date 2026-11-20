import secrets
import uuid

from django.db import models
from django.utils import timezone


def generate_player_uid() -> str:
    return f"usr_{uuid.uuid4().hex[:12]}"


def compute_rank_tier_from_mmr(mmr: int) -> str:
    if mmr >= 2500:
        return "Simorgh Immortal"
    if mmr >= 2000:
        return "Pahlavan Champion"
    if mmr >= 1600:
        return "Golden Griffin"
    if mmr >= 1300:
        return "Silver Vanguard"
    if mmr >= 1000:
        return "Simorgh Initiate"
    return "Bronze Recruit"


class Player(models.Model):
    """
    جدول اختصاصی بازیکنان و سیستم رنک (کاملاً مستقل در جدول moba_players)
    """

    id = models.BigAutoField(primary_key=True)
    user_id = models.CharField(
        "شناسه یکتای بازیکن",
        max_length=64,
        unique=True,
        default=generate_player_uid,
        db_index=True,
    )
    email = models.EmailField(
        "ایمیل بازیکن", max_length=128, unique=True, db_index=True
    )
    username = models.CharField(
        "نام کاربری (پهلوان)", max_length=64, db_index=True
    )
    mmr = models.IntegerField("امتیاز رنک (MMR)", default=1000)
    rank_tier = models.CharField(
        "درجه رنک (Rank Tier)", max_length=32, default="Simorgh Initiate"
    )
    wins = models.IntegerField("تعداد پیروزی‌ها", default=0)
    losses = models.IntegerField("تعداد شکست‌ها", default=0)
    abandoned_count = models.IntegerField(
        "تعداد ترک بازی (Abandon)", default=0
    )
    is_banned = models.BooleanField("مسدود شده (Ban)", default=False)
    created_at = models.DateTimeField("تاریخ ثبت‌نام", auto_now_add=True)
    last_login_at = models.DateTimeField(
        "آخرین ورود به بازی", null=True, blank=True
    )

    class Meta:
        db_table = "moba_players"
        verbose_name = "بازیکن"
        verbose_name_plural = "۱. بازیکنان و رنکینگ (Players)"
        ordering = ["-mmr", "-created_at"]

    def __str__(self) -> str:
        return f"{self.username} ({self.email}) — MMR: {self.mmr}"

    @property
    def total_matches(self) -> int:
        return self.wins + self.losses

    @property
    def win_rate_percent(self) -> float:
        total = self.total_matches
        if total <= 0:
            return 0.0
        return round((self.wins * 100.0) / total, 1)


class OtpVerification(models.Model):
    """
    جدول اختصاصی کدهای یک‌بارمصرف (OTP) برای ورود و ثبت‌نام یکپارچه
    """

    id = models.BigAutoField(primary_key=True)
    email = models.EmailField("ایمیل", max_length=128, db_index=True)
    otp_code = models.CharField("کد ۶ رقمی OTP", max_length=6)
    username = models.CharField("نام کاربری انتخابی در ثبت‌نام", max_length=64, blank=True, default="")
    is_used = models.BooleanField("استفاده‌شده", default=False)
    created_at = models.DateTimeField("زمان صدور", auto_now_add=True)
    expires_at = models.DateTimeField("زمان انقضا")

    class Meta:
        db_table = "moba_otp_verifications"
        verbose_name = "کد تایید یک‌بارمصرف (OTP)"
        verbose_name_plural = "۵. کدهای تایید یک‌بارمصرف (OTP Codes)"
        ordering = ["-created_at"]

    def __str__(self) -> str:
        return f"{self.email} -> {self.otp_code}"

    def is_valid(self) -> bool:
        return not self.is_used and timezone.now() <= self.expires_at


class PlayerSessionToken(models.Model):
    """
    جدول اختصاصی نشست و توکن‌های احراز هویت بازیکن در منوی بازی و وب‌سوکت
    """

    id = models.BigAutoField(primary_key=True)
    player = models.ForeignKey(
        Player,
        verbose_name="بازیکن",
        on_delete=models.CASCADE,
        related_name="tokens",
    )
    access_token = models.CharField(
        "توکن دسترسی (Access Token)", max_length=128, unique=True, db_index=True
    )
    refresh_token = models.CharField(
        "توکن بازیابی (Refresh Token)",
        max_length=128,
        unique=True,
        db_index=True,
    )
    is_active = models.BooleanField("فعال", default=True)
    created_at = models.DateTimeField("زمان ایجاد", auto_now_add=True)
    expires_at = models.DateTimeField("زمان انقضا")

    class Meta:
        db_table = "moba_player_tokens"
        verbose_name = "توکن نشست بازیکن"
        verbose_name_plural = "۴. توکن‌ها و نشست‌های بازیکنان (Session Tokens)"
        ordering = ["-created_at"]

    def __str__(self) -> str:
        return f"{self.player.username} ({self.access_token[:18]}...)"

    @classmethod
    def create_for_player(cls, player: Player) -> "PlayerSessionToken":
        access = f"simorgh_at_{secrets.token_urlsafe(32)}"
        refresh = f"simorgh_rt_{secrets.token_urlsafe(32)}"
        return cls.objects.create(
            player=player,
            access_token=access,
            refresh_token=refresh,
            is_active=True,
            expires_at=timezone.now() + timezone.timedelta(days=30),
        )


class GameMatchRoom(models.Model):
    """
    جدول اختصاصی اتاق‌های مسابقه ساخته‌شده توسط Matchmaking
    """

    STATUS_WAITING = "waiting"
    STATUS_RUNNING = "running"
    STATUS_FINISHED = "finished"
    STATUS_CANCELED = "canceled"

    STATUS_CHOICES = [
        (STATUS_WAITING, "در انتظار بازیکنان (Waiting)"),
        (STATUS_RUNNING, "در حال اجرا (Running)"),
        (STATUS_FINISHED, "پایان‌یافته (Finished)"),
        (STATUS_CANCELED, "لغوشده / ترک‌شده (Canceled)"),
    ]

    WINNER_CHOICES = [
        (0, "نامشخص / بدون برنده"),
        (1, "تیم ۱ — سپاه روشنایی (اهورا)"),
        (2, "تیم ۲ — سپاه تاریکی (اهریمن)"),
    ]

    id = models.BigAutoField(primary_key=True)
    room_id = models.CharField(
        "شناسه اتاق (Room ID)", max_length=64, unique=True, db_index=True
    )
    server_host = models.CharField(
        "آدرس IP سرور", max_length=64, default="127.0.0.1"
    )
    server_port = models.IntegerField("پورت UDP سرور")
    max_players = models.IntegerField("ظرفیت کل بازیکنان", default=2)
    status = models.CharField(
        "وضعیت مسابقه",
        max_length=24,
        choices=STATUS_CHOICES,
        default=STATUS_RUNNING,
    )
    winner_team = models.IntegerField(
        "تیم پیروز",
        choices=WINNER_CHOICES,
        null=True,
        blank=True,
    )
    duration_sec = models.IntegerField(
        "مدت زمان نبرد (ثانیه)", default=0
    )
    process_pid = models.IntegerField(
        "شناسه پروسه سیستم‌عامل (PID)", null=True, blank=True
    )
    is_custom_lobby = models.BooleanField(
        "لابی سفارشی (بدون امتیاز و رنک)", default=False
    )
    created_at = models.DateTimeField("زمان شروع اتاق", auto_now_add=True)
    finished_at = models.DateTimeField(
        "زمان پایان اتاق", null=True, blank=True
    )

    class Meta:
        db_table = "moba_match_rooms"
        verbose_name = "اتاق نبرد (Match Room)"
        verbose_name_plural = "۲. اتاق‌های نبرد و سرورهای بازی (Match Rooms)"
        ordering = ["-created_at"]

    def __str__(self) -> str:
        return f"{self.room_id} ({self.server_host}:{self.server_port}) [{self.status}]"

    @property
    def server_addr(self) -> str:
        return f"{self.server_host}:{self.server_port}"


class MatchParticipant(models.Model):
    """
    جدول اختصاصی بازیکنان حاضر در هر اتاق مسابقه، وضعیت اتصال/ترک بازی و آمار نهایی نبرد
    """

    TEAM_CHOICES = [
        (1, "تیم ۱ — سپاه روشنایی (اهورا)"),
        (2, "تیم ۲ — سپاه تاریکی (اهریمن)"),
    ]

    STATUS_ASSIGNED = "assigned"
    STATUS_CONNECTED = "connected"
    STATUS_DISCONNECTED = "disconnected"
    STATUS_ABANDONED = "abandoned"
    STATUS_COMPLETED = "completed"

    PARTICIPANT_STATUS_CHOICES = [
        (STATUS_ASSIGNED, "تخصیص‌یافته به اتاق (Assigned)"),
        (STATUS_CONNECTED, "متصل در بازی (Connected)"),
        (STATUS_DISCONNECTED, "دیسکانکت موقت — قابل اتصال مجدد (Disconnected)"),
        (STATUS_ABANDONED, "ترک نهایی بازی (Abandoned)"),
        (STATUS_COMPLETED, "پایان کامل بازی (Completed)"),
    ]

    id = models.BigAutoField(primary_key=True)
    match_room = models.ForeignKey(
        GameMatchRoom,
        verbose_name="اتاق نبرد",
        on_delete=models.CASCADE,
        related_name="participants",
    )
    player = models.ForeignKey(
        Player,
        verbose_name="بازیکن",
        on_delete=models.CASCADE,
        related_name="match_participations",
    )
    ticket_id = models.CharField(
        "تیکت اتصال UDP (Ticket ID)", max_length=64, unique=True, db_index=True
    )
    slot_index = models.IntegerField("شماره جایگاه (Slot)", default=0)
    team_id = models.IntegerField(
        "تیم", choices=TEAM_CHOICES, default=1
    )
    status = models.CharField(
        "وضعیت بازیکن در بازی",
        max_length=24,
        choices=PARTICIPANT_STATUS_CHOICES,
        default=STATUS_CONNECTED,
        db_index=True,
    )
    hero_id = models.IntegerField("شناسه قهرمان (Hero ID)", default=0)
    level = models.IntegerField("سطح قهرمان (Level)", default=1)
    kills = models.IntegerField("کشتن‌ها (Kills)", default=0)
    deaths = models.IntegerField("مرگ‌ها (Deaths)", default=0)
    assists = models.IntegerField("کمک‌ها (Assists)", default=0)
    last_hits = models.IntegerField("ضربه آخر (Last Hits)", default=0)
    denies = models.IntegerField("دینای (Denies)", default=0)
    gold_coins = models.IntegerField("طلای نهایی (Gold)", default=0)
    is_winner = models.BooleanField("برنده مسابقه", default=False)
    mmr_delta = models.IntegerField("تغییر امتیاز MMR", default=0)
    party_group_id = models.CharField("شناسه گروه پارتی", max_length=64, blank=True, default="", db_index=True)
    joined_at = models.DateTimeField("زمان ورود به مچ", auto_now_add=True)
    left_at = models.DateTimeField(
        "زمان خروج / پایان", null=True, blank=True
    )

    class Meta:
        db_table = "moba_match_participants"
        verbose_name = "شرکت‌کننده نبرد"
        verbose_name_plural = "۳. بازیکنان حاضر در نبردها (Match Participants)"
        ordering = ["-joined_at"]

    def __str__(self) -> str:
        return f"{self.player.username} در {self.match_room.room_id} (تیم {self.team_id} - وضعیت: {self.status})"


class Friendship(models.Model):
    """
    جدول اختصاصی دوستی‌ها و درخواست‌های دوستی بین بازیکنان
    """

    STATUS_PENDING = "pending"
    STATUS_ACCEPTED = "accepted"
    STATUS_DECLINED = "declined"
    STATUS_BLOCKED = "blocked"

    STATUS_CHOICES = [
        (STATUS_PENDING, "در انتظار تایید (Pending)"),
        (STATUS_ACCEPTED, "تایید شده (Accepted)"),
        (STATUS_DECLINED, "رد شده (Declined)"),
        (STATUS_BLOCKED, "مسدود شده (Blocked)"),
    ]

    id = models.BigAutoField(primary_key=True)
    requester = models.ForeignKey(
        Player,
        verbose_name="درخواست‌دهنده",
        on_delete=models.CASCADE,
        related_name="sent_friend_requests",
    )
    addressee = models.ForeignKey(
        Player,
        verbose_name="دریافت‌کننده",
        on_delete=models.CASCADE,
        related_name="received_friend_requests",
    )
    status = models.CharField(
        "وضعیت دوستی",
        max_length=16,
        choices=STATUS_CHOICES,
        default=STATUS_PENDING,
        db_index=True,
    )
    created_at = models.DateTimeField("زمان ایجاد", auto_now_add=True)
    updated_at = models.DateTimeField("آخرین بروزرسانی", auto_now=True)

    class Meta:
        db_table = "moba_friendships"
        verbose_name = "رابطه دوستی"
        verbose_name_plural = "۶. دوستان و درخواست‌های دوستی (Friendships)"
        ordering = ["-updated_at"]
        constraints = [
            models.UniqueConstraint(
                fields=["requester", "addressee"],
                name="unique_moba_friendship_pair",
            )
        ]

    def __str__(self) -> str:
        return f"{self.requester.username} -> {self.addressee.username} [{self.status}]"