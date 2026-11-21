# track-record web UI: the built Vite app served by nginx, which also
# proxies /ui/api to the backend's UI port. Behind the ingress/auth proxy
# the UI and its API share one origin, so the app uses relative URLs.
#
# Build from frontend/:
#   docker build --tag track-record-frontend frontend
#
# TR_UI_API_UPSTREAM is the backend UI port (default http://server:8081).
FROM node:24-trixie-slim AS build
WORKDIR /src
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY . .
RUN npm run build

FROM nginxinc/nginx-unprivileged:1-alpine
LABEL org.opencontainers.image.title="track-record-frontend" \
      org.opencontainers.image.description="Track Record web UI" \
      org.opencontainers.image.source="https://github.com/sraesch/track-record"
ENV TR_UI_API_UPSTREAM=http://server:8081
# Only TR_ variables are substituted, so nginx's own $uri etc. stay intact.
ENV NGINX_ENVSUBST_FILTER=^TR_
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=build /src/dist /usr/share/nginx/html
EXPOSE 8080
