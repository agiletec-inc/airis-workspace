FROM node:22-alpine

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      build-essential ca-certificates git curl openssh-client \
      python3 pkg-config tini \
      libnspr4 libnss3 libdbus-1-3 libatk1.0-0 libatk-bridge2.0-0 \
      libcups2 libxkbcommon0 libatspi2.0-0 libxcomposite1 libxdamage1 \
      libxfixes3 libxrandr2 libgbm1 libasound2 \
      libdrm2 libxshmfence1 libxcb1 libpango-1.0-0 libcairo2 \
      libglib2.0-0 && \
    rm -rf /var/lib/apt/lists/* && \
    corepack enable

RUN set -eux; \
    if ! id -u app >/dev/null 2>&1; then \
      useradd -m -s /bin/bash app; \
    fi; \
    chown -R app:app /home/app

RUN mkdir -p \
      /app/node_modules \
      /app/.pnpm \
      /app/.next \
      /app/dist \
      /app/build \
      /app/out \
      /app/.swc \
      /app/.cache \
      /app/.turbo \
      /pnpm/store && \
    chown -R app:app /app /pnpm

ENV PNPM_HOME=/pnpm
ENV PNPM_STORE_DIR=/pnpm/store

WORKDIR /app

COPY --chown=app:app . .

USER app
RUN pnpm install --frozen-lockfile

ENTRYPOINT ["tini","--"]
