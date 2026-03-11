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
COPY apps/corporate-site/package.json apps/corporate-site/
COPY libs/ libs/
RUN corepack install
RUN pnpm install --frozen-lockfile --filter @workspace/corporate-site...

# ============ Builder ============
FROM deps AS builder

COPY apps/corporate-site/ apps/corporate-site/
COPY libs/ libs/
WORKDIR /app/apps/corporate-site
RUN pnpm build

# ============ Production ============
FROM nginx:alpine AS production

COPY apps/corporate-site/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/apps/corporate-site/dist /usr/share/nginx/html

EXPOSE 3000
CMD ["nginx", "-g", "daemon off;"]
