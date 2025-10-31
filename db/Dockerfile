FROM postgres:17.5

LABEL maintainer="PostGIS Project - https://postgis.net"

RUN apt-get update \
    && apt-get install -y \
    postgresql-17-postgis \
    postgresql-17-pgrouting

RUN mkdir -p /docker-entrypoint-initdb.d
COPY ./initdb-postgis.sh /docker-entrypoint-initdb.d/10_postgis.sh