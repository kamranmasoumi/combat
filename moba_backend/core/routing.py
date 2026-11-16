from django.urls import path

from .consumers import MatchmakingConsumer, LobbyConsumer

websocket_urlpatterns = [
    path("ws/matchmaking/", MatchmakingConsumer.as_asgi()),
    path("ws/lobby/", LobbyConsumer.as_asgi())
]
