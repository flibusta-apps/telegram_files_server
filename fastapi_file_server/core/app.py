import sentry_sdk
from fastapi import FastAPI
from fastapi.responses import ORJSONResponse
from prometheus_fastapi_instrumentator import Instrumentator

from app.on_start import on_start
from app.views import healthcheck_router, router
from core.config import env_config
from core.db import database

sentry_sdk.init(
    env_config.SENTRY_DSN,
)


def start_app() -> FastAPI:
    app = FastAPI(default_response_class=ORJSONResponse)

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

    Instrumentator().instrument(app).expose(app, include_in_schema=True)

    return app
