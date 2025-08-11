FROM rust:latest as base
WORKDIR /app

RUN apt-get update && apt-get install -y \
    software-properties-common \
    fish \
    rustfmt \
    osm2pgsql osmium-tool pyosmium \
    nodejs \
    npm \
    gdal-bin \
    cmake make libclang-dev libssl-dev pkg-config 

RUN git clone https://github.com/strukturag/libheif.git
RUN cd libheif && git checkout tags/v1.18.1 -b v1.18.1

RUN mkdir build
RUN cd build && cmake --preset=release ../libheif && make && make install
RUN npm i -g esbuild

FROM base as dev

RUN chsh -s $(which fish)

RUN rustup component add rust-analyzer
RUN cargo install cargo-watch
RUN cargo install cargo-edit
RUN cargo install sqlx-cli --no-default-features --features postgres
RUN rustup component add rustfmt

RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD npm install; cargo watch -x run --ignore tiles --ignore dist

FROM base as build

COPY . .
RUN mkdir -p /app/dist
RUN npm i 
RUN esbuild --bundle index.js --outfile=dist/index.js --format=esm
RUN cargo build --release

FROM debian as prod

RUN apt-get update && apt-get install -y \
    osm2pgsql \
    osmium-tool \
    gdal-bin \
    wget

WORKDIR /app
COPY --from=build /app/target/release/veloinfo /app/veloinfo
COPY --from=build /app/migrations /app/migrations
COPY --from=build /app/pub /app/pub
COPY --from=build /app/dist /app/dist
COPY --from=build /app/node_modules /app/node_modules
COPY --from=build /app/import.sh /app/import.sh
COPY --from=build /app/import.lua /app/import.lua
COPY --from=build /usr/local/lib/libheif /usr/local/lib/libheif
RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD /app/veloinfo
