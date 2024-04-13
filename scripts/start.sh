#! /usr/bin/env sh

cd /app
mkdir -p prometheus

/env.sh > ./.env

granian --interface asgi --host 0.0.0.0 --port 8080 --loop uvloop main:app
