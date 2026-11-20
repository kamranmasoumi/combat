from django.urls import path

from .consumers import LobbyConsumer, MatchmakingConsumer, SocialPartyConsumer

websocket_urlpatterns = [
    path("ws/matchmaking/", MatchmakingConsumer.as_asgi()),
    path("ws/lobby/", LobbyConsumer.as_asgi()),
    path("ws/social/", SocialPartyConsumer.as_asgi()),
]