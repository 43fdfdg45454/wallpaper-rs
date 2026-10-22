FROM docker.io/library/rust:1.99-alpine AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p wprs-server --features oidc \
    && install -D -m 0755 target/release/wprs-server /out/wprs-server \
    && install -d -m 0700 /out/data

FROM gcr.io/distroless/static-debian13:nonroot
LABEL org.opencontainers.image.title="wprs-server" \
      org.opencontainers.image.description="Self-hosted wallpaper-rs server" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
COPY --from=build /out/wprs-server /usr/local/bin/wprs-server
COPY --from=build --chown=65532:65532 /out/data /data
ENV WPRS_DATA=/data WPRS_LISTEN=0.0.0.0:8080
USER 65532:65532
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 CMD ["/usr/local/bin/wprs-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/wprs-server"]
CMD ["serve"]
