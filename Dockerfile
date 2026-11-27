# Container image for pixel-factor-service. Build with `make image`.
#
# Both base images are pinned by digest; Dependabot proposes updates.

FROM rust:1.100.0-slim-trixie@sha256:3b199fd6db7d709274df56565b8f4003f9e30d7bfcff9d53148c86053b7855e2 AS build
WORKDIR /src
COPY . .
# Read at compile time and reported by GET /version.
ARG PF_BUILD_COMMIT=unknown
ENV PF_BUILD_COMMIT=${PF_BUILD_COMMIT}
RUN cargo build --release --locked -p pixel-factor-service \
    && mkdir -p /out/data \
    && cp target/release/pixel-factor-service /out/

# Distroless: libc and CA certificates only, no shell, no package manager.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:29ee70762214fe8190cea04ef1c0a84a8365879e047ce8f911ae5a5ab2355fa2
ARG PF_BUILD_COMMIT=unknown
LABEL org.opencontainers.image.title="pixel-factor-service" \
      org.opencontainers.image.source="https://github.com/Jodre11/pixel-factor" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      org.opencontainers.image.revision="${PF_BUILD_COMMIT}"
COPY --from=build /out/pixel-factor-service /usr/local/bin/pixel-factor-service
COPY --from=build --chown=65532:65532 /out/data /data
USER 65532:65532
ENV PF_LISTEN=0.0.0.0:8080 \
    PF_DB_PATH=/data/pixel-factor.db \
    PF_LOG_FORMAT=json \
    RUST_LOG=info
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --retries=3 --start-period=30s --start-interval=1s \
    CMD ["/usr/local/bin/pixel-factor-service", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/pixel-factor-service"]
