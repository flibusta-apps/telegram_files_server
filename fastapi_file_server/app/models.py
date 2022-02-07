from datetime import datetime
from enum import Enum

import ormar

from core.db import metadata, database


class BaseMeta(ormar.ModelMeta):
    metadata = metadata
    database = database


class UploadBackends(str, Enum):
    bot = "bot"
    user = "user"


class UploadedFile(ormar.Model):
    class Meta(BaseMeta):
        tablename = "uploaded_files"

    id = ormar.BigInteger(primary_key=True, nullable=False)
    backend = ormar.String(max_length=16, choices=list(UploadBackends))
    data = ormar.JSON()
    upload_time = ormar.DateTime(timezone=True, default=datetime.now)
