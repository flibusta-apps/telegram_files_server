cd /app

rm -rf prometheus
mkdir prometheus

uvicorn main:app --host 0.0.0.0 --port 8080 --loop uvloop
