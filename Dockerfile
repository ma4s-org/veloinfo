FROM rust:latest as base
WORKDIR /app

RUN apt-get update && apt-get install -y \
    fish hx \
    rustfmt \
    osm2pgsql osmium-tool pyosmium \
    nodejs \
    npm \
    gdal-bin \
    unzip \
    cmake make libclang-dev libssl-dev pkg-config 

RUN git clone https://github.com/strukturag/libheif.git
RUN cd libheif && git checkout tags/v1.18.1 -b v1.18.1

RUN mkdir build
RUN cd build && cmake --preset=release ../libheif && make && make install
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall jj-cli --no-confirm

FROM base as dev

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

CMD npm install; cargo watch -x run --ignore tiles --ignore dist

FROM base as build

COPY . .
RUN mkdir -p /app/dist
RUN npm i 
RUN cargo build --release

FROM debian as prod

RUN apt-get update && apt-get install -y \
    osm2pgsql \
    osmium-tool \
    gdal-bin \
    wget unzip

WORKDIR /app
COPY --from=build /app/target/release/veloinfo /app/veloinfo
COPY --from=build /app/migrations /app/migrations
COPY --from=build /app/pub /app/pub
COPY --from=build /app/node_modules /app/node_modules
COPY --from=build /app/custom-elements /app/custom-elements
COPY --from=build /app/import.sh /app/import.sh
COPY --from=build /app/import_srtm.sh /app/import_srtm.sh
COPY --from=build /app/import.lua /app/import.lua
COPY --from=build /usr/local/lib/libheif /usr/local/lib/libheif
RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD /app/veloinfo
