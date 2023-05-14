from typing import Optional

from fastapi import APIRouter, Depends, File, Form, HTTPException, UploadFile, status
from fastapi.responses import StreamingResponse

from app.depends import check_token
from app.models import UploadedFile as UploadedFileDB
from app.serializers import CreateUploadedFile, UploadedFile
from app.services.file_downloader import FileDownloader
from app.services.file_uploader import FileUploader


router = APIRouter(
    prefix="/api/v1/files", dependencies=[Depends(check_token)], tags=["files"]
)


@router.get("/", response_model=list[UploadedFile])
async def get_files():
    return await UploadedFileDB.objects.all()


@router.get(
    "/{file_id}",
    response_model=UploadedFile,
    responses={
        404: {},
    },
)
async def get_file(file_id: int):
    uploaded_file = await UploadedFileDB.objects.get_or_none(id=file_id)

    if not uploaded_file:
        raise HTTPException(status.HTTP_404_NOT_FOUND)

    return uploaded_file


@router.post("/", response_model=UploadedFile)
async def create_file(data: CreateUploadedFile):
    return await UploadedFileDB.objects.create(**data.dict())


@router.post("/upload/", response_model=UploadedFile)
async def upload_file(file: UploadFile = File({}), caption: Optional[str] = Form({})):
    return await FileUploader.upload(file, caption=caption)


@router.get("/download_by_message/{chat_id}/{message_id}")
async def download_by_message(chat_id: str, message_id: int):
    data = await FileDownloader.download_by_message_id(message_id)

    if data is None:
        raise HTTPException(status.HTTP_400_BAD_REQUEST)

    return StreamingResponse(data)


@router.delete("/{file_id}", response_model=UploadedFile, responses={400: {}})
async def delete_file(file_id: int):
    uploaded_file = await UploadedFileDB.objects.get_or_none(id=file_id)

    if not uploaded_file:
        raise HTTPException(status.HTTP_400_BAD_REQUEST)

    await uploaded_file.delete()
    return uploaded_file


healthcheck_router = APIRouter(tags=["healthcheck"])


@healthcheck_router.get("/healthcheck")
async def healthcheck():
    return "Ok"
