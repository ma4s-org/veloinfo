FROM rust:1.81.0 as base
WORKDIR /app

RUN apt-get update && apt-get install -y \
    software-properties-common

RUN apt-get update && apt-get install -y \
    fish \
    rustfmt \
    osm2pgsql \
    nodejs \
    npm \
    cmake make libclang-dev libssl-dev pkg-config 

RUN cd
RUN git clone https://github.com/strukturag/libheif.git
RUN cd libheif && git checkout tags/v1.18.1 -b v1.18.1
run cd 
RUN mkdir build
RUN cd build && cmake --preset=release ../libheif && make && make install
RUN install -d tailwindcss
        
FROM base as dev

RUN chsh -s $(which fish)
    
RUN cargo install cargo-watch
RUN cargo install sqlx-cli --no-default-features --features postgres
RUN rustup component add rustfmt

RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD cargo watch -x run

FROM base as build

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
COPY --from=build /usr/local/lib/libheif /usr/local/lib/libheif
RUN echo "db:5432:carte:postgres:postgres" >> /root/.pgpass
RUN chmod 0600 /root/.pgpass

CMD /app/veloinfo
