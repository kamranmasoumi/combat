from django.urls import path

from .consumers import (
    CoordinatorConsumer,
    LobbyConsumer,
    MatchmakingConsumer,
    SocialPartyConsumer,
)

websocket_urlpatterns = [
    path("ws/matchmaking/", MatchmakingConsumer.as_asgi()),
    path("ws/lobby/", LobbyConsumer.as_asgi()),
    path("ws/social/", SocialPartyConsumer.as_asgi()),
    path("ws/coordinator/", CoordinatorConsumer.as_asgi()),
]