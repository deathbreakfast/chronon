#!/bin/bash
set -euo pipefail
dnf install -y docker
systemctl enable --now docker
usermod -aG docker ec2-user

mkdir -p /opt/chronon
cat > /opt/chronon/docker-compose.data.yml <<'COMPOSE'
services:
  postgres:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: chronon
      POSTGRES_PASSWORD: chronon
      POSTGRES_DB: chronon
    ports:
      - "5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data
  redis:
    image: redis:7-alpine
    restart: unless-stopped
    ports:
      - "6379:6379"
volumes:
  pgdata:
COMPOSE

cd /opt/chronon
docker compose -f docker-compose.data.yml up -d
