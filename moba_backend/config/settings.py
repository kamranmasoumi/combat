import os
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent

# ریشه اصلی پروژه سیمرغ (جایی که Cargo.toml و moba_server قرار دارند)
WORKSPACE_ROOT = Path(
    os.environ.get("SIMORGH_WORKSPACE_ROOT", str(BASE_DIR.parent))
).resolve()

SECRET_KEY = os.environ.get(
    "DJANGO_SECRET_KEY",
    "simorgh-moba-custom-secret-key-2026-no-drf-no-auth-contrib",
)

DEBUG = os.environ.get("DJANGO_DEBUG", "1") in ("1", "true", "True")

ALLOWED_HOSTS = ["*"]

INSTALLED_APPS = [
    "daphne",
    "django.contrib.admin",
    "django.contrib.auth",
    "django.contrib.contenttypes",
    "django.contrib.sessions",
    "django.contrib.messages",
    "django.contrib.staticfiles",
    "channels",
    "core.apps.CoreConfig",
]

MIDDLEWARE = [
    "django.middleware.security.SecurityMiddleware",
    "django.contrib.sessions.middleware.SessionMiddleware",
    "django.middleware.common.CommonMiddleware",
    "django.middleware.csrf.CsrfViewMiddleware",
    "django.contrib.auth.middleware.AuthenticationMiddleware",
    "django.contrib.messages.middleware.MessageMiddleware",
    "django.middleware.clickjacking.XFrameOptionsMiddleware",
]

ROOT_URLCONF = "config.urls"

TEMPLATES = [
    {
        "BACKEND": "django.template.backends.django.DjangoTemplates",
        "DIRS": [],
        "APP_DIRS": True,
        "OPTIONS": {
            "context_processors": [
                "django.template.context_processors.debug",
                "django.template.context_processors.request",
                "django.contrib.auth.context_processors.auth",
                "django.contrib.messages.context_processors.messages",
            ],
        },
    },
]

ASGI_APPLICATION = "config.asgi.application"
WSGI_APPLICATION = "config.wsgi.application"

# تنظیمات دیتابیس PostgreSQL
DATABASES = {
    "default": {
        "ENGINE": "django.db.backends.postgresql",
        "NAME": os.environ.get("POSTGRES_DB", "simorgh_moba"),
        "USER": os.environ.get("POSTGRES_USER", "postgres"),
        "PASSWORD": os.environ.get("POSTGRES_PASSWORD", "postgres"),
        "HOST": os.environ.get("POSTGRES_HOST", "127.0.0.1"),
        "PORT": os.environ.get("POSTGRES_PORT", "5432"),
    }
}

# آدرس Redis (هم برای تیکت‌های سرور بازی Rust و هم در صورت نیاز برای Channels)
REDIS_URL = os.environ.get("REDIS_URL", "redis://172.21.13.50:6379/0")
REDIS_FALLBACK_URL = os.environ.get("REDIS_FALLBACK_URL", "redis://127.0.0.1:6379/0")

# برای مدیریت وب‌سوکت‌ها در یک پروسه واحد از InMemoryChannelLayer استفاده می‌شود
# (یا در صورت تنظیم USE_REDIS_CHANNEL_LAYER=1 از RedisChannelLayer استفاده می‌کند)
if os.environ.get("USE_REDIS_CHANNEL_LAYER", "0") == "1":
    CHANNEL_LAYERS = {
        "default": {
            "BACKEND": "channels_redis.core.RedisChannelLayer",
            "CONFIG": {
                "hosts": [REDIS_URL],
            },
        },
    }
else:
    CHANNEL_LAYERS = {
        "default": {
            "BACKEND": "channels.layers.InMemoryChannelLayer",
        },
    }

# تنظیمات Matchmaking و اجرای سرویس systemd سرور بازی
MATCHMAKING_PLAYERS_PER_MATCH = int(os.environ.get("MATCHMAKING_PLAYERS_PER_MATCH", "1"))
GAME_SERVER_HOST = os.environ.get("GAME_SERVER_HOST", "127.0.0.1")
GAME_SERVER_PORT_START = int(os.environ.get("GAME_SERVER_PORT_START", "4433"))
GAME_SERVER_PORT_END = int(os.environ.get("GAME_SERVER_PORT_END", "4550"))
GAME_SERVER_SERVICE_NAME = os.environ.get("GAME_SERVER_SERVICE_NAME", "moba-server")

LANGUAGE_CODE = "fa-ir"
TIME_ZONE = "Asia/Tehran"
USE_I18N = True
USE_TZ = True

STATIC_URL = "static/"
STATIC_ROOT = BASE_DIR / "staticfiles"

DEFAULT_AUTO_FIELD = "django.db.models.BigAutoField"
