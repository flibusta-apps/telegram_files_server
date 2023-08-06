from app.serializers import UploadBackend
from app.services.storages import BotStorage, StoragesContainer, UserStorage


class FileDownloader:
    _bot_storage_index = 0
    _user_storage_index = 0

    @classmethod
    @property
    def bot_storages(cls) -> list[BotStorage]:
        return StoragesContainer.BOT_STORAGES

    @classmethod
    @property
    def user_storages(cls) -> list[UserStorage]:
        return StoragesContainer.USER_STORAGES

    @classmethod
    def get_bot_storage(cls) -> BotStorage:
        if not cls.bot_storages:
            raise ValueError("Aiogram storage not exist!")

        bot_storages: list[BotStorage] = cls.bot_storages  # type: ignore

        cls._bot_storage_index = (cls._bot_storage_index + 1) % len(bot_storages)

        return bot_storages[cls._bot_storage_index]

    @classmethod
    def get_user_storage(cls) -> UserStorage:
        if not cls.user_storages:
            raise ValueError("Telethon storage not exists!")

        user_storages: list[UserStorage] = cls.user_storages  # type: ignore

        cls._user_storage_index = (cls._user_storage_index + 1) % len(user_storages)

        return user_storages[cls._user_storage_index]

    @classmethod
    async def _download_via(cls, message_id: int, storage_type: UploadBackend):
        if storage_type == UploadBackend.bot:
            storage = cls.get_bot_storage()
        else:
            storage = cls.get_user_storage()

        return await storage.download(message_id)

    @classmethod
    async def download_by_message_id(cls, message_id: int):
        if not cls.bot_storages and not cls.user_storages:
            raise ValueError("Files storage not exist!")

        if (data := await cls._download_via(message_id, UploadBackend.bot)) is not None:
            return data

        return await cls._download_via(message_id, UploadBackend.user)
