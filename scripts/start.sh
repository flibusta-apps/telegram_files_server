#! /usr/bin/env sh

/env.sh > ./.env
. ./.env
rm ./.env

cd /app
mkdir -p prometheus

granian --interface asgi --host 0.0.0.0 --port 8080 --loop uvloop main:app
