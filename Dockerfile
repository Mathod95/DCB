# Image de production de DevCardBattle : binaire unique + cartes et règles du repo.
# Les cartes sont en lecture seule (DCB_EDITOR=0) : on les crée en local, on push,
# et la CI publie une nouvelle image.

# cargo-chef : les dépendances sont compilées dans une couche à part, gardée en cache par la CI
# (cache GitHub Actions) tant que Cargo.lock ne change pas ; seul le code du projet se recompile.
FROM rust:1.97-slim-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --locked -p dcb-server --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY migrations migrations
COPY migrations-app migrations-app
COPY rules rules
RUN cargo build --release --locked -p dcb-server \
 && cp target/release/dcb-server /usr/local/bin/dcb-server

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app dcb \
 && mkdir -p /data && chown dcb /data
WORKDIR /app
COPY --from=build /usr/local/bin/dcb-server /usr/local/bin/dcb-server
# Les cartes appartiennent à l'utilisateur de l'image : en mode démo, l'éditeur y écrit (dans
# le conteneur, hors volume), et ces cartes disparaissent au déploiement suivant.
COPY --chown=10001 cards cards
COPY rules rules

# Commit de l'image, affiché par la pastille de préprod (DCB_CHANNEL, donné au conteneur).
ARG DCB_COMMIT=""
ENV DCB_COMMIT=$DCB_COMMIT
ENV DCB_BIND=0.0.0.0:3000 \
    DCB_EDITOR=0 \
    DCB_SANDBOX=1 \
    DCB_SECURE_COOKIES=1 \
    DATABASE_URL=sqlite:///data/cache.db \
    DCB_APP_DATABASE_URL=sqlite:///data/app.db \
    RUST_LOG=dcb_server=info,tower_http=info

USER dcb
VOLUME /data
EXPOSE 3000
ENTRYPOINT ["/usr/bin/tini", "--", "dcb-server"]
