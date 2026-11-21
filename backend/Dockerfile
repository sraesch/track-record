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

# Runtime: distroless, matching the trixie build stage (same glibc). The
# binary needs only libc and libgcc_s; TLS is rustls with built-in root
# certificates. No shell or package manager, which keeps the scanner findings
# of a full Debian base (util-linux, perl, sqlite, ...) out of the image.
FROM gcr.io/distroless/cc-debian13:nonroot
LABEL org.opencontainers.image.title="track-record-server" \
      org.opencontainers.image.description="Track Record backend: REST API, MCP server and jobs" \
      org.opencontainers.image.source="https://github.com/sraesch/track-record"
COPY --from=build /src/backend/target/release/track-record-server /usr/local/bin/track-record-server
# The image's nonroot user, numeric so that Kubernetes runAsNonRoot can check it.
USER 65532:65532
# 8080: machine API (/api, /mcp, tokens). 8081: UI API (behind the auth proxy).
EXPOSE 8080 8081
ENTRYPOINT ["track-record-server"]
CMD ["serve"]
