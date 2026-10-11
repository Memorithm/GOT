FROM python:3.12-alpine
WORKDIR /app
COPY --chown=65534:65534 agent.py /app/agent.py
RUN chmod 0444 /app/agent.py
