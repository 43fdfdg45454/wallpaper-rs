FROM docker.io/library/rust:1.99-alpine@sha256:3ea887915f261f273d6a24efdfdb92665dee12199923e57870453644fe4ae5c9 AS build
WORKDIR /src
ARG WPRS_VERSION
ARG BUILDKIT_SBOM_SCAN_STAGE=true
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p wprs-server --features oidc \
    && install -D -m 0755 target/release/wprs-server /out/wprs-server \
    && install -d -m 0700 /out/data

FROM gcr.io/distroless/static-debian13:nonroot@sha256:d62067879de890a1def9da0e7766b7d20810a5e5555cef8506b752437d7a3e88
LABEL org.opencontainers.image.title="wprs-server" \
      org.opencontainers.image.description="Self-hosted wallpaper-rs server" \
      org.opencontainers.image.licenses="GPL-3.0-or-later" \
      org.opencontainers.image.source="https://github.com/43fdfdg45454/wallpaper-rs"
COPY --from=build /out/wprs-server /usr/local/bin/wprs-server
COPY --from=build --chown=65532:65532 /out/data /data
ENV WPRS_DATA=/data WPRS_LISTEN=0.0.0.0:8080
USER 65532:65532
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 CMD ["/usr/local/bin/wprs-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/wprs-server"]
CMD ["serve"]
