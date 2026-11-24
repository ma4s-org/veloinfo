FROM rust:latest as base
WORKDIR /app

RUN apt-get update && apt-get install -y \
    fish hx \
    rustfmt \
    osm2pgsql osmium-tool pyosmium \
    curl ca-certificates gnupg \
    gdal-bin \
    unzip \
    libheif-dev \
    cmake make libclang-dev libssl-dev pkg-config 

# maplibre-gl 6.7 tire une dépendance qui exige Node >= 22.
# Debian trixie ne fournit que Node 20 : on installe Node 22 via NodeSource.
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*

FROM base as dev

# Chromium sert à générer les vignettes cartographiques des messages
# ActivityPub en mode headless.
RUN apt-get update && apt-get install -y \
    chromium \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

RUN chsh -s $(which fish)

RUN rustup component add rust-analyzer
RUN cargo install cargo-watch
RUN cargo install cargo-edit
RUN cargo install sqlx-cli --no-default-features --features postgres
RUN rustup component add rustfmt
RUN npm i typescript-language-server -g
RUN npm i vscode-html-languageservice -g

RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD npm install; cargo watch -x run \
    --ignore "tiles/*" \
    --ignore "dist/*" \
    --ignore "*.osm.pbf" \
    --ignore "nodes.bin" \
    --ignore "package-lock.json" \
    --ignore ".aider*" \
    --ignore ".jj/*" \
    --ignore ".vscode/*" \
    --ignore "lock/*" \
    --ignore "well-known/*" \
    --ignore "images/*" \
    --ignore "target/*"

FROM base as build

COPY . .
RUN mkdir -p /app/dist

# Cache npm (les paquets déjà téléchargés sont réutilisés).
RUN --mount=type=cache,target=/root/.npm,sharing=locked npm i

# Cache Cargo entre les déploiements : les dépendances déjà compilées ne sont
# pas reconstruites à chaque modification du code. Le binaire est copié hors
# du cache, car un montage de cache n'est pas conservé dans l'image.
RUN --mount=type=cache,target=/app/target,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    cargo build --release \
    && cp target/release/veloinfo /tmp/veloinfo

FROM debian as prod

RUN apt-get update && apt-get install -y \
    osm2pgsql \
    osmium-tool \
    gdal-bin \
    libheif-dev \
    wget unzip \
    ca-certificates \
    chromium \
    fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=build /tmp/veloinfo /app/veloinfo
COPY --from=build /app/migrations /app/migrations
COPY --from=build /app/pub /app/pub
COPY --from=build /app/node_modules /app/node_modules
COPY --from=build /app/well-known /app/well-known
COPY --from=build /app/custom-elements /app/custom-elements
COPY --from=build /app/import.sh /app/import.sh
COPY --from=build /app/import_srtm.sh /app/import_srtm.sh
COPY --from=build /app/import.lua /app/import.lua
RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD /app/veloinfo
