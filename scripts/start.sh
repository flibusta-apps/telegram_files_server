#! /usr/bin/env sh

export $(/env.sh)

cd /app
mkdir -p prometheus

granian --interface asgi --host 0.0.0.0 --port 8080 --loop uvloop main:app
