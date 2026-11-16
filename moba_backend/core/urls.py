from django.urls import path

from . import views

urlpatterns = [
    path("auth/request-otp/", views.request_otp_view, name="request_otp"),
    path("auth/verify-otp/", views.verify_otp_view, name="verify_otp"),
    path("auth/me/", views.profile_view, name="profile_me"),
    path("match/active/", views.active_match_view, name="active_match"),
    path("match/disconnect/", views.disconnect_match_view, name="disconnect_match"),
    path("match/reconnect/", views.reconnect_match_view, name="reconnect_match"),
    path("match/player-state/", views.update_player_connection_state_view, name="update_player_connection_state"),
    path("match/abandon/", views.abandon_match_view, name="abandon_match"),
    path("match/report-result/", views.report_match_result_view, name="report_match_result"),
    path("match/history/", views.match_history_view, name="match-history")
]
