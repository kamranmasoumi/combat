from django.db import migrations, models


class Migration(migrations.Migration):

    dependencies = [
        ("core", "0001_initial"),
    ]

    operations = [
        migrations.AddField(
            model_name="player",
            name="abandoned_count",
            field=models.IntegerField(
                default=0, verbose_name="تعداد ترک بازی (Abandon)"
            ),
        ),
        migrations.AddField(
            model_name="gamematchroom",
            name="winner_team",
            field=models.IntegerField(
                blank=True,
                choices=[
                    (0, "نامشخص / بدون برنده"),
                    (1, "تیم ۱ — سپاه روشنایی (اهورا)"),
                    (2, "تیم ۲ — سپاه تاریکی (اهریمن)"),
                ],
                null=True,
                verbose_name="تیم پیروز",
            ),
        ),
        migrations.AddField(
            model_name="gamematchroom",
            name="duration_sec",
            field=models.IntegerField(
                default=0, verbose_name="مدت زمان نبرد (ثانیه)"
            ),
        ),
        migrations.AddField(
            model_name="gamematchroom",
            name="finished_at",
            field=models.DateTimeField(
                blank=True, null=True, verbose_name="زمان پایان اتاق"
            ),
        ),
        migrations.AlterField(
            model_name="gamematchroom",
            name="status",
            field=models.CharField(
                choices=[
                    ("waiting", "در انتظار بازیکنان (Waiting)"),
                    ("running", "در حال اجرا (Running)"),
                    ("finished", "پایان‌یافته (Finished)"),
                    ("canceled", "لغوشده / ترک‌شده (Canceled)"),
                ],
                default="running",
                max_length=24,
                verbose_name="وضعیت مسابقه",
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="status",
            field=models.CharField(
                choices=[
                    ("assigned", "تخصیص‌یافته به اتاق (Assigned)"),
                    ("connected", "متصل در بازی (Connected)"),
                    (
                        "disconnected",
                        "دیسکانکت موقت — قابل اتصال مجدد (Disconnected)",
                    ),
                    ("abandoned", "ترک نهایی بازی (Abandoned)"),
                    ("completed", "پایان کامل بازی (Completed)"),
                ],
                db_index=True,
                default="connected",
                max_length=24,
                verbose_name="وضعیت بازیکن در بازی",
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="hero_id",
            field=models.IntegerField(
                default=0, verbose_name="شناسه قهرمان (Hero ID)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="level",
            field=models.IntegerField(
                default=1, verbose_name="سطح قهرمان (Level)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="kills",
            field=models.IntegerField(
                default=0, verbose_name="کشتن‌ها (Kills)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="deaths",
            field=models.IntegerField(
                default=0, verbose_name="مرگ‌ها (Deaths)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="assists",
            field=models.IntegerField(
                default=0, verbose_name="کمک‌ها (Assists)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="last_hits",
            field=models.IntegerField(
                default=0, verbose_name="ضربه آخر (Last Hits)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="denies",
            field=models.IntegerField(
                default=0, verbose_name="دینای (Denies)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="gold_coins",
            field=models.IntegerField(
                default=0, verbose_name="طلای نهایی (Gold)"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="is_winner",
            field=models.BooleanField(
                default=False, verbose_name="برنده مسابقه"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="mmr_delta",
            field=models.IntegerField(
                default=0, verbose_name="تغییر امتیاز MMR"
            ),
        ),
        migrations.AddField(
            model_name="matchparticipant",
            name="left_at",
            field=models.DateTimeField(
                blank=True, null=True, verbose_name="زمان خروج / پایان"
            ),
        ),
    ]
