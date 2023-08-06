from typing import Optional

from pydantic import BaseModel
from pydantic_settings import BaseSettings


BotToken = str
TelethonSessionName = str


class TelethonConfig(BaseModel):
    APP_ID: int
    API_HASH: str


class EnvConfig(BaseSettings):
    API_KEY: str

    TELEGRAM_CHAT_ID: int

    BOT_TOKENS: Optional[list[BotToken]]

    TELETHON_APP_CONFIG: Optional[TelethonConfig]
    TELETHON_SESSIONS: Optional[list[TelethonSessionName]]

    SENTRY_DSN: str


env_config = EnvConfig()
