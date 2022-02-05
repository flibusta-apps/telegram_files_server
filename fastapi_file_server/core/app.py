from fastapi import FastAPI

from app.on_start import on_start
from app.views import router, healthcheck_router
from core.db import database


def start_app() -> FastAPI:
    app = FastAPI()

    app.state.database = database

    app.include_router(router)
    app.include_router(healthcheck_router)

    @app.on_event("startup")
    async def startup() -> None:
        database_ = app.state.database
        if not database_.is_connected:
            await database_.connect()

        await on_start()

    @app.on_event("shutdown")
    async def shutdown() -> None:
        database_ = app.state.database
        if database_.is_connected:
            await database_.disconnect()

    return app
