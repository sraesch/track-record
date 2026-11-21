# track-record backend service (REST API on two ports, MCP, jobs).
#
# Build from the repository root, because the build embeds the list of
# database migrations (db/migrations):
#   docker build --file backend/Dockerfile --tag track-record-server .
FROM rust:1-trixie AS build
WORKDIR /src
COPY db/migrations db/migrations
COPY backend backend
RUN cd backend && cargo build --release --locked --package track-record-server

FROM debian:trixie-slim
LABEL org.opencontainers.image.title="track-record-server" \
      org.opencontainers.image.description="Track Record backend: REST API, MCP server and jobs" \
      org.opencontainers.image.source="https://github.com/sraesch/track-record"
RUN useradd --system --uid 10001 --no-create-home track-record
COPY --from=build /src/backend/target/release/track-record-server /usr/local/bin/track-record-server
USER 10001
# 8080: machine API (/api, /mcp, tokens). 8081: UI API (behind the auth proxy).
EXPOSE 8080 8081
ENTRYPOINT ["track-record-server"]
CMD ["serve"]
