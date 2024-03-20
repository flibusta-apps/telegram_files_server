cd /app

rm -rf prometheus
mkdir prometheus

granian --interface asgi --host 0.0.0.0 --port 8080 --loop uvloop main:app
