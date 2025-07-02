FROM postgres:17.5

LABEL maintainer="PostGIS Project - https://postgis.net"

RUN apt-get update \
    && apt-get install -y \
    postgresql-postgis \
    postgresql-pgrouting

#RUN ln -s /usr/lib/postgresql/13/lib/libpgrouting-3.6.so /usr/lib/postgresql/13/lib/libpgrouting-3.2.so

RUN mkdir -p /docker-entrypoint-initdb.d
COPY ./initdb-postgis.sh /docker-entrypoint-initdb.d/10_postgis.sh`