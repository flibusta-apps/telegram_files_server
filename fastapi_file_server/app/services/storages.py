from telegram_files_storage import AiogramFilesStorage, TelethonFilesStorage

from core.config import env_config


class StoragesContainer:
    AIOGRAM_STORAGES: list[AiogramFilesStorage] = []
    TELETHON_STORAGES: list[TelethonFilesStorage] = []

    @classmethod
    async def prepare(cls):
        if env_config.BOT_TOKENS:
            cls.AIOGRAM_STORAGES: list[AiogramFilesStorage] = [
                AiogramFilesStorage(env_config.TELEGRAM_CHAT_ID, token)
                for token in env_config.BOT_TOKENS
            ]

        if env_config.TELETHON_APP_CONFIG and env_config.TELETHON_SESSIONS:
            cls.TELETHON_STORAGES: list[TelethonFilesStorage] = [
                TelethonFilesStorage(
                    env_config.TELEGRAM_CHAT_ID,
                    env_config.TELETHON_APP_CONFIG.APP_ID,
                    env_config.TELETHON_APP_CONFIG.API_HASH,
                    session,
                )
                for session in env_config.TELETHON_SESSIONS
            ]

        for storage in [*cls.AIOGRAM_STORAGES, *cls.TELETHON_STORAGES]:
            await storage.prepare()
