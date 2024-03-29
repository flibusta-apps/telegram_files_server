from typing import Annotated, Optional

from fastapi import APIRouter, Depends, Form, HTTPException, UploadFile, status
from fastapi.responses import StreamingResponse

from app.depends import check_token
from app.serializers import UploadedFile
from app.services.file_downloader import FileDownloader
from app.services.file_uploader import FileUploader


router = APIRouter(
    prefix="/api/v1/files", dependencies=[Depends(check_token)], tags=["files"]
)


@router.post("/upload/", response_model=UploadedFile)
async def upload_file(
    file: UploadFile,
    file_size: Annotated[int, Form()],
    filename: Annotated[str, Form()],
    caption: Annotated[Optional[str], Form()],
):
    return await FileUploader.upload(file, filename, file_size, caption=caption)


@router.get("/download_by_message/{chat_id}/{message_id}")
async def download_by_message(chat_id: str, message_id: int):
    data = await FileDownloader.download_by_message_id(message_id)

    if data is None:
        raise HTTPException(status.HTTP_400_BAD_REQUEST)

    return StreamingResponse(data)
