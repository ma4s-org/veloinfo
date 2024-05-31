FROM rust:1.78 as dev

RUN apt-get update && apt-get install -y \
    fish \
    rustfmt \
    osm2pgsql \
    nodejs \
    npm \
    libheif-dev libheif1 pkg-config \
    llvm-dev libclang-dev clang

ENV PKG_CONFIG_PATH /usr/lib/x86_64-linux-gnu/pkgconfig
RUN chsh -s $(which fish)

RUN install -d tailwindcss
    
RUN cargo install cargo-watch
RUN cargo install sqlx-cli --no-default-features --features postgres
RUN rustup component add rustfmt

RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

WORKDIR /app
CMD cargo watch -x run

FROM rust as build
WORKDIR /app

RUN apt-get update && apt-get install -y \
    fish \
    rustfmt \
    osm2pgsql \
    nodejs npm  

RUN install -d tailwindcss

COPY . .
RUN cargo build --release

FROM debian as prod

RUN apt-get update && apt-get install -y \
    osm2pgsql \
    wget

WORKDIR /app
COPY --from=build /app/target/release/veloinfo /app/veloinfo
COPY --from=build /app/migrations /app/migrations
COPY --from=build /app/pub /app/pub
COPY --from=build /app/import.sh /app/import.sh
COPY --from=build /app/import.lua /app/import.lua
RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD /app/veloinfo
