from io import BytesIO
from typing import Optional

from telegram_files_storage import AiogramFilesStorage, TelethonFilesStorage

from app.services.storages import StoragesContainer


class FileDownloader:
    _aiogram_storage_index = 0
    _telethon_storage_index = 0

    @classmethod
    @property
    def AIOGRAM_STORAGES(cls) -> list[AiogramFilesStorage]:
        return StoragesContainer.AIOGRAM_STORAGES

    @classmethod
    @property
    def TELETHON_STORAGES(cls) -> list[TelethonFilesStorage]:
        return StoragesContainer.TELETHON_STORAGES

    @classmethod
    def get_aiogram_storage(cls) -> AiogramFilesStorage:
        if not cls.AIOGRAM_STORAGES:
            raise ValueError("Aiogram storage not exist!")

        cls._aiogram_storage_index = (cls._aiogram_storage_index + 1) % len(
            cls.AIOGRAM_STORAGES
        )

        return cls.AIOGRAM_STORAGES[cls._aiogram_storage_index]

    @classmethod
    def get_telethon_storage(cls) -> TelethonFilesStorage:
        if not cls.TELETHON_STORAGES:
            raise ValueError("Telethon storage not exists!")

        cls._telethon_storage_index = (cls._telethon_storage_index + 1) % len(
            cls.TELETHON_STORAGES
        )

        return cls.TELETHON_STORAGES[cls._telethon_storage_index]

    @classmethod
    async def download_by_file_id(cls, file_id: str) -> Optional[BytesIO]:
        if not cls.AIOGRAM_STORAGES:
            return None

        storage = cls.get_aiogram_storage()

        return await storage.download(file_id)

    @classmethod
    async def download_by_message_id(cls, message_id: int) -> Optional[BytesIO]:
        if not cls.TELETHON_STORAGES:
            return None

        storage = cls.get_telethon_storage()

        return await storage.download(message_id)
