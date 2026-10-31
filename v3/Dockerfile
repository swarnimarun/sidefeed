FROM node:24-bookworm-slim

ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=8787 \
    SIDEFEED_DB=/data/sidefeed.sqlite

WORKDIR /app
# The application uses Node built-ins only; no dependency installation is needed.
COPY --chown=node:node package.json ./
COPY --chown=node:node server ./server
COPY --chown=node:node web ./web
RUN mkdir -p /data && chown node:node /data && chmod 700 /data

USER node
EXPOSE 8787
VOLUME ["/data"]
CMD ["node", "server/index.mjs"]
