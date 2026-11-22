# track-record backend service (REST API on two ports, MCP, jobs).
#
# Build from the repository root, because the build embeds the list of
# database migrations (db/migrations):
#   docker build --file backend/Dockerfile --tag track-record-server .
#
# Multi-platform (linux/amd64 and linux/arm64, ADR 0014): the build stage
# always runs on the build machine's own platform and cross-compiles for the
# target, so no platform is built under emulation.
#   docker buildx build --platform linux/amd64,linux/arm64 --file backend/Dockerfile .
FROM --platform=$BUILDPLATFORM rust:1-trixie AS build
ARG BUILDARCH
ARG TARGETARCH
# The C compiler and linker for each target. They name the native toolchain
# when the target is the build platform (the gcc package provides both names).
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
# A cross toolchain only for another platform. The only C code is ring
# (rustls), which needs the target's gcc and libc headers.
RUN set -eu; \
    case "$TARGETARCH" in \
        amd64) triple=x86_64-unknown-linux-gnu; gcc=gcc-x86-64-linux-gnu ;; \
        arm64) triple=aarch64-unknown-linux-gnu; gcc=gcc-aarch64-linux-gnu ;; \
        *) echo "unsupported target architecture: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    echo "$triple" > /rust-target; \
    if [ "$TARGETARCH" != "$BUILDARCH" ]; then \
        apt-get update; \
        apt-get install --yes --no-install-recommends "$gcc" "libc6-dev-$TARGETARCH-cross"; \
        rm -rf /var/lib/apt/lists/*; \
        rustup target add "$triple"; \
    fi
WORKDIR /src
COPY db/migrations db/migrations
COPY backend backend
RUN cd backend \
    && target=$(cat /rust-target) \
    && cargo build --release --locked --package track-record-server --target "$target" \
    && cp "target/$target/release/track-record-server" /track-record-server

# Runtime: distroless, matching the trixie build stage (same glibc). The
# binary needs only libc and libgcc_s; TLS is rustls with built-in root
# certificates. No shell or package manager, which keeps the scanner findings
# of a full Debian base (util-linux, perl, sqlite, ...) out of the image.
FROM gcr.io/distroless/cc-debian13:nonroot
LABEL org.opencontainers.image.title="track-record-server" \
      org.opencontainers.image.description="Track Record backend: REST API, MCP server and jobs" \
      org.opencontainers.image.source="https://github.com/sraesch/track-record"
COPY --from=build /track-record-server /usr/local/bin/track-record-server
# The image's nonroot user, numeric so that Kubernetes runAsNonRoot can check it.
USER 65532:65532
# 8080: machine API (/api, /mcp, tokens). 8081: UI API (behind the auth proxy).
EXPOSE 8080 8081
ENTRYPOINT ["track-record-server"]
CMD ["serve"]
