import enum
from typing import TypedDict

from pydantic import BaseModel


class UploadBackend(enum.StrEnum):
    bot = "bot"
    user = "user"


class Data(TypedDict):
    chat_id: str | int
    message_id: int


class UploadedFile(BaseModel):
    backend: UploadBackend
    data: Data
