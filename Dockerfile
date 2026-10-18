# Image de production de DevCardBattle : binaire unique + cartes et règles du repo.
# Les cartes sont en lecture seule (DCB_EDITOR=0) : on les crée en local, on push,
# et la CI publie une nouvelle image.

FROM rust:1.97-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY migrations migrations
COPY migrations-app migrations-app
COPY rules rules
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p dcb-server \
 && cp target/release/dcb-server /usr/local/bin/dcb-server

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app dcb \
 && mkdir -p /data && chown dcb /data
WORKDIR /app
COPY --from=build /usr/local/bin/dcb-server /usr/local/bin/dcb-server
COPY cards cards
COPY rules rules

ENV DCB_BIND=0.0.0.0:3000 \
    DCB_EDITOR=0 \
    DCB_SECURE_COOKIES=1 \
    DATABASE_URL=sqlite:///data/cache.db \
    DCB_APP_DATABASE_URL=sqlite:///data/app.db \
    RUST_LOG=dcb_server=info,tower_http=info

USER dcb
VOLUME /data
EXPOSE 3000
ENTRYPOINT ["/usr/bin/tini", "--", "dcb-server"]
