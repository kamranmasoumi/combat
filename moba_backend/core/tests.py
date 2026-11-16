import json
from unittest.mock import patch

from channels.testing import WebsocketCommunicator
from django.test import Client, TransactionTestCase, override_settings

from config.asgi import application
from core.models import GameMatchRoom, MatchParticipant, OtpVerification, Player


class AuthAndMatchmakingE2ETests(TransactionTestCase):
    def setUp(self):
        self.client = Client()

    def _register_and_login_player(self, email: str) -> dict:
        resp1 = self.client.post(
            "/api/v1/auth/request-otp/",
            data=json.dumps({"email": email}),
            content_type="application/json",
        )
        self.assertEqual(resp1.status_code, 200)

        otp_obj = OtpVerification.objects.filter(email=email, is_used=False).latest(
            "created_at"
        )
        resp2 = self.client.post(
            "/api/v1/auth/verify-otp/",
            data=json.dumps({"email": email, "otp_code": otp_obj.otp_code}),
            content_type="application/json",
        )
        self.assertEqual(resp2.status_code, 200)
        data = resp2.json()
        self.assertIn("access_token", data)
        self.assertTrue(data["user_id"].startswith("usr_"))
        return data

    def test_otp_auth_and_profile_verification(self):
        p1 = self._register_and_login_player("rostam@simorgh.ir")
        self.assertEqual(p1["username"], "rostam")
        self.assertEqual(Player.objects.count(), 1)

        me_resp = self.client.get(
            "/api/v1/auth/me/",
            HTTP_AUTHORIZATION=f"Bearer {p1['access_token']}",
        )
        self.assertEqual(me_resp.status_code, 200)
        me_data = me_resp.json()
        self.assertEqual(me_data["email"], "rostam@simorgh.ir")
        self.assertEqual(me_data["mmr"], 1000)
        self.assertIsNone(me_data["active_match"])

    @override_settings(MATCHMAKING_PLAYERS_PER_MATCH=2)
    async def test_websocket_matchmaking_and_active_match_reconnect(self):
        from asgiref.sync import sync_to_async

        p1 = await sync_to_async(self._register_and_login_player)("p1@simorgh.ir")
        p2 = await sync_to_async(self._register_and_login_player)("p2@simorgh.ir")

        with patch(
            "core.consumers.spawn_game_server_process", return_value=99999
        ) as mock_spawn, patch("core.consumers.store_ticket_in_redis") as mock_redis:
            comm1 = WebsocketCommunicator(
                application, f"/ws/matchmaking/?token={p1['access_token']}"
            )
            connected1, _ = await comm1.connect()
            self.assertTrue(connected1)

            msg1 = await comm1.receive_json_from()
            self.assertEqual(msg1["event"], "queued")
            self.assertEqual(msg1["players_in_queue"], 1)
            self.assertEqual(msg1["required_players"], 2)

            comm2 = WebsocketCommunicator(
                application, f"/ws/matchmaking/?token={p2['access_token']}"
            )
            connected2, _ = await comm2.connect()
            self.assertTrue(connected2)

            start1 = await comm1.receive_json_from(timeout=5)
            start2 = await comm2.receive_json_from(timeout=5)
            self.assertEqual(start1["event"], "starting_server")
            self.assertEqual(start2["event"], "starting_server")

            match1 = await comm1.receive_json_from(timeout=5)
            match2 = await comm2.receive_json_from(timeout=5)

            self.assertEqual(match1["event"], "match_found")
            self.assertEqual(match2["event"], "match_found")
            self.assertEqual(match1["room_id"], match2["room_id"])
            self.assertEqual(match1["port"], match2["port"])
            self.assertNotEqual(match1["ticket_id"], match2["ticket_id"])
            self.assertTrue(match1["game_link"].startswith("moba://"))

            mock_spawn.assert_called_once_with(
                match1["port"], match1["room_id"], 2
            )
            self.assertEqual(mock_redis.call_count, 2)

            room_count = await sync_to_async(GameMatchRoom.objects.count)()
            part_count = await sync_to_async(MatchParticipant.objects.count)()
            self.assertEqual(room_count, 1)
            self.assertEqual(part_count, 2)

            await comm1.disconnect()
            await comm2.disconnect()

        # بررسی تشخیص بازی فعال از طریق API ریکانکت وقتی پورت UDP باز است
        with patch("core.consumers.is_udp_port_bound", return_value=True), patch(
            "core.consumers.store_ticket_in_redis"
        ):
            active_resp = await sync_to_async(self.client.get)(
                "/api/v1/match/active/",
                HTTP_AUTHORIZATION=f"Bearer {p1['access_token']}",
            )
            self.assertEqual(active_resp.status_code, 200)
            active_json = active_resp.json()
            self.assertTrue(active_json["has_active_match"])
            self.assertEqual(active_json["active_match"]["room_id"], match1["room_id"])
            self.assertEqual(active_json["active_match"]["ticket_id"], match1["ticket_id"])
            self.assertEqual(
                active_json["active_match"]["participant_status"],
                MatchParticipant.STATUS_CONNECTED,
            )

            # مرحله ۱ خروج دو مرحله‌ای (Disconnect): بازیکن دیسکانکت می‌کند اما هنوز می‌تواند Reconnect کند
            disc_resp = await sync_to_async(self.client.post)(
                "/api/v1/match/disconnect/",
                HTTP_AUTHORIZATION=f"Bearer {p1['access_token']}",
            )
            self.assertEqual(disc_resp.status_code, 200)
            disc_json = disc_resp.json()
            self.assertEqual(disc_json["disconnected_matches"], 1)
            self.assertIsNotNone(disc_json["active_match"])
            self.assertEqual(
                disc_json["active_match"]["participant_status"],
                MatchParticipant.STATUS_DISCONNECTED,
            )

            # مرحله ۲ خروج دو مرحله‌ای (Abandon): بازیکن از منوی اصلی Abandon می‌زند
            with patch("core.consumers.delete_ticket_from_redis"):
                aban_resp = await sync_to_async(self.client.post)(
                    "/api/v1/match/abandon/",
                    HTTP_AUTHORIZATION=f"Bearer {p1['access_token']}",
                )
                self.assertEqual(aban_resp.status_code, 200)
                aban_json = aban_resp.json()
                self.assertEqual(aban_json["abandoned_matches"], 1)
                self.assertEqual(aban_json["mmr"], 975)

            # پس از Abandon، دیگر بازی فعال برای p1 وجود ندارد ولی برای p2 همچنان فعال است
            active_after_aban = await sync_to_async(self.client.get)(
                "/api/v1/match/active/",
                HTTP_AUTHORIZATION=f"Bearer {p1['access_token']}",
            )
            self.assertFalse(active_after_aban.json()["has_active_match"])

            active_p2 = await sync_to_async(self.client.get)(
                "/api/v1/match/active/",
                HTTP_AUTHORIZATION=f"Bearer {p2['access_token']}",
            )
            self.assertTrue(active_p2.json()["has_active_match"])

            # ثبت نتیجه نهایی بازی توسط سرور (POST /api/v1/match/report-result/)
            with patch("core.consumers.delete_ticket_from_redis"):
                report_resp = await sync_to_async(self.client.post)(
                    "/api/v1/match/report-result/",
                    data=json.dumps(
                        {
                            "room_id": match1["room_id"],
                            "winner_team": 2,
                            "duration_sec": 845,
                            "players": [
                                {
                                    "user_id": p1["user_id"],
                                    "slot_index": 0,
                                    "team_id": 1,
                                    "hero_id": 1,
                                    "level": 8,
                                    "kills": 2,
                                    "deaths": 5,
                                    "assists": 1,
                                    "last_hits": 34,
                                    "denies": 3,
                                    "gold_coins": 1200,
                                },
                                {
                                    "user_id": p2["user_id"],
                                    "slot_index": 1,
                                    "team_id": 2,
                                    "hero_id": 3,
                                    "level": 14,
                                    "kills": 9,
                                    "deaths": 1,
                                    "assists": 4,
                                    "last_hits": 95,
                                    "denies": 12,
                                    "gold_coins": 5400,
                                },
                            ],
                        }
                    ),
                    content_type="application/json",
                )
                self.assertEqual(report_resp.status_code, 200)
                rep_json = report_resp.json()
                self.assertEqual(rep_json["status"], "ok")
                self.assertEqual(rep_json["winner_team"], 2)
