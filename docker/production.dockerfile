FROM ghcr.io/flibusta-apps/base_docker_images:3.12-poetry-buildtime AS build-image

WORKDIR /root/poetry
COPY pyproject.toml poetry.lock /root/poetry/

ENV VENV_PATH=/opt/venv

RUN poetry export --without-hashes > requirements.txt \
    && . /opt/venv/bin/activate \
    && pip install -r requirements.txt --no-cache-dir


FROM ghcr.io/flibusta-apps/base_docker_images:3.12-postgres-runtime AS runtime-image

WORKDIR /app

ENV VENV_PATH=/opt/venv
ENV PATH="$VENV_PATH/bin:$PATH"

COPY --from=build-image $VENV_PATH $VENV_PATH
COPY ./fastapi_file_server/ /app/
COPY ./scripts/* /root/

EXPOSE 8080

CMD bash /root/start_production.sh
