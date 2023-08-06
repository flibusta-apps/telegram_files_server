from fastapi import FastAPI
from fastapi.responses import ORJSONResponse

from prometheus_fastapi_instrumentator import Instrumentator
import sentry_sdk

from app.on_start import on_start
from app.views import router
from core.config import env_config


sentry_sdk.init(
    env_config.SENTRY_DSN,
)


def start_app() -> FastAPI:
    app = FastAPI(default_response_class=ORJSONResponse)

    app.include_router(router)

    @app.on_event("startup")
    async def startup() -> None:
        await on_start()

    Instrumentator(
        should_ignore_untemplated=True,
        excluded_handlers=["/docs", "/metrics", "/healthcheck"],
    ).instrument(app).expose(app, include_in_schema=True)

    return app
