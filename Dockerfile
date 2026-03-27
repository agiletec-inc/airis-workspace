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

# Step 1: Copy lockfile + workspace manifests (cache-efficient — only changes when deps change)
COPY pnpm-lock.yaml pnpm-workspace.yaml .npmrc* package.json ./
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm install --frozen-lockfile

# Step 2: Copy full source (changes on every code edit, but deps are cached above)
COPY . .
RUN chown -R app:app /app

# Fix named volume permissions at container start (volumes mount as root)
# setpriv is available in util-linux (included in node:*-bookworm images)
RUN set -e && \
    echo '#!/bin/sh' > /usr/local/bin/entrypoint.sh && \
    echo 'DIRS="node_modules .pnpm .next dist build out .swc .cache .turbo"' >> /usr/local/bin/entrypoint.sh && \
    echo 'for d in $DIRS; do' >> /usr/local/bin/entrypoint.sh && \
    echo '  find /app -maxdepth 5 -name "$d" -type d ! -user app -exec chown -R app:app '"'"'{}'"'"' + 2>/dev/null' >> /usr/local/bin/entrypoint.sh && \
    echo 'done' >> /usr/local/bin/entrypoint.sh && \
    echo 'exec setpriv --reuid=app --regid=app --init-groups -- "$@"' >> /usr/local/bin/entrypoint.sh && \
    chmod +x /usr/local/bin/entrypoint.sh
ENTRYPOINT ["tini","--","entrypoint.sh"]
