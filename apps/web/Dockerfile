FROM node:24-bookworm-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
COPY apps ./apps
COPY packages ./packages
RUN npm ci && npm run build --workspace @devstore/web

FROM node:24-bookworm-slim AS runtime
WORKDIR /app
ENV HOST=0.0.0.0
ENV PORT=3000
ENV NITRO_HOST=0.0.0.0
ENV NITRO_PORT=3000
ENV NUXT_TELEMETRY_DISABLED=1
COPY --from=build --chown=node:node /app/apps/web/.output ./apps/web/.output
USER node
EXPOSE 3000
CMD ["node", "apps/web/.output/server/index.mjs"]
