FROM rust:1-bookworm@sha256:77fac8b98f9f46062bb680b6d25d5bcaabfc400143952ebc572e924bcbedc3fa AS builder

WORKDIR /app

COPY . .

RUN cargo build --release --locked --bin telegram_files_server


FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818

RUN apt-get update \
    && apt-get install -y openssl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN update-ca-certificates
RUN groupadd --gid 101 app \
    && useradd --uid 101 --gid 101 --no-create-home --shell /usr/sbin/nologin app

COPY ./scripts/*.sh /
RUN chmod +x /*.sh

WORKDIR /app

COPY --from=builder /app/target/release/telegram_files_server /usr/local/bin
# Match the telegram-bot-api user (aiogram/telegram-bot-api) that owns the shared volume.
USER 101:101
ENTRYPOINT ["/start.sh"]
