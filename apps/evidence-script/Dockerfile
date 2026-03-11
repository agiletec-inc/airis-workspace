# ============ Development ============
FROM node:24-bookworm AS dev

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      git \
      curl \
      openssh-client \
      python3 \
      pkg-config \
      tini && \
    rm -rf /var/lib/apt/lists/*

# Ensure dedicated app user exists
RUN set -eux; \
    if ! id -u app >/dev/null 2>&1; then \
      useradd -m -s /bin/bash app; \
    fi; \
    chown -R app:app /home/app

# Pre-create common build directories to prevent root-owned creation by Docker volumes
RUN mkdir -p /app/{node_modules,.next,dist,build,out,.swc,.cache,.turbo} && \
    chown -R app:app /app

WORKDIR /app
USER app

ENTRYPOINT ["tini","--"]
CMD ["sleep","infinity"]

# ============ Dependencies ============
FROM node:24-bookworm AS deps

RUN corepack enable
WORKDIR /app

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml .npmrc* ./
COPY apps/evidence-script/package.json apps/evidence-script/
COPY libs/ libs/
RUN corepack install
RUN pnpm install --frozen-lockfile --filter evidence-script...

# ============ Builder ============
FROM deps AS builder

COPY apps/evidence-script/ apps/evidence-script/
COPY libs/ libs/
WORKDIR /app/apps/evidence-script
RUN pnpm build

# ============ Production ============
FROM node:24-slim AS production

RUN useradd -m app
WORKDIR /app

COPY --from=builder /app/apps/evidence-script/.next/standalone ./
COPY --from=builder /app/apps/evidence-script/.next/static ./.next/static
COPY --from=builder /app/apps/evidence-script/public ./public

USER app
EXPOSE 3000
ENV PORT=3000 HOSTNAME="0.0.0.0"
CMD ["node", "server.js"]
