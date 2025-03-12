# Building Stage
ARG RUST_VERSION=1.85
FROM rust:${RUST_VERSION}-slim-bookworm AS build
WORKDIR /app
COPY LICENSE LICENSE

RUN --mount=type=bind,source=doc,target=doc \
    --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=crates,target=crates \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    <<EOF
set -e
apt-get -y update && apt-get install -y --no-install-recommends pkg-config libssl-dev
cargo build --locked --release
cp target/release/bifrost /bifrost
EOF


# Final Stage
FROM debian:bookworm-slim AS final

COPY --from=build /bifrost /app/bifrost

WORKDIR /app

CMD ["/app/bifrost"]
