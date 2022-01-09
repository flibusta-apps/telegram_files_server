from io import BytesIO
from typing import Optional

from fastapi import UploadFile

from telegram_files_storage import AiogramFilesStorage, TelethonFilesStorage

from app.models import UploadedFile, UploadBackends
from app.services.storages import StoragesContainer


class FileUploader:
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

    def __init__(self, file: UploadFile, caption: Optional[str] = None) -> None:
        self.file = file
        self.caption = caption

        self.upload_data: Optional[dict] = None
        self.upload_backend: Optional[UploadBackends] = None

    async def _upload(self) -> bool:
        if not self.AIOGRAM_STORAGES and not self.TELETHON_STORAGES:
            raise ValueError("Files storage not exist!")

        if await self._upload_via_aiogram():
            return True

        return await self._upload_via_telethon()

    async def _upload_via_aiogram(self) -> bool:
        if not self.AIOGRAM_STORAGES:
            return False

        data = await self.file.read()

        if isinstance(data, str):
            data = data.encode()

        if len(data) > 50 * 1000 * 1000:
            return False

        bytes_io = BytesIO(data)
        bytes_io.name = self.file.filename

        storage = self.get_aiogram_storage()

        self.upload_data = await storage.upload(bytes_io, self.caption)  # type: ignore
        self.upload_backend = UploadBackends.aiogram

        return True

    async def _upload_via_telethon(self) -> bool:
        if not self.TELETHON_STORAGES:
            return False

        data = await self.file.read()

        if isinstance(data, str):
            data = data.encode()

        bytes_io = BytesIO(data)
        bytes_io.name = self.file.filename

        storage = self.get_telethon_storage()

        self.upload_data = await storage.upload(
            bytes_io, caption=self.caption
        )  # type: ignore
        self.upload_backend = UploadBackends.telethon

        return True

    async def _save_to_db(self) -> UploadedFile:
        return await UploadedFile.objects.create(
            backend=self.upload_backend,
            data=self.upload_data,
        )

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
    async def upload(
        cls, file: UploadFile, caption: Optional[str] = None
    ) -> Optional[UploadedFile]:
        uploader = cls(file, caption)
        upload_result = await uploader._upload()

        if not upload_result:
            return None

        return await uploader._save_to_db()
