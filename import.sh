#!/usr/bin/bash
set -e
set -o pipefail

# ==============================================================================
# 1. CONFIGURATION ET TÉLÉCHARGEMENT
# ==============================================================================
PSQL_CMD="psql -h db -U postgres -d carte -v ON_ERROR_STOP=1"

QUEBEC_URL="https://download.geofabrik.de/north-america/canada/quebec-latest.osm.pbf"
ONTARIO_URL="https://download.geofabrik.de/north-america/canada/ontario-latest.osm.pbf"
NB_URL="https://download.geofabrik.de/north-america/canada/new-brunswick-latest.osm.pbf"
MAINE_URL="https://download.geofabrik.de/north-america/us/maine-latest.osm.pbf"
VERMONT_URL="https://download.geofabrik.de/north-america/us/vermont-latest.osm.pbf"
NEWYORK_URL="https://download.geofabrik.de/north-america/us/new-york-latest.osm.pbf"

QUEBEC_FILE="quebec-latest.osm.pbf"
ONTARIO_FILE="ontario-latest.osm.pbf"
NB_FILE="new-brunswick-latest.osm.pbf"
MAINE_FILE="maine-latest.osm.pbf"
VERMONT_FILE="vermont-latest.osm.pbf"
NEWYORK_FILE="new-york-latest.osm.pbf"
NY_NORTH_FILE="new-york-north.osm.pbf"
ONT_EAST_FILE="ontario-east.osm.pbf"
MERGED_FILE="regions.osm.pbf"

echo "--- Étape 1 : Téléchargement des données OSM ---"
rm -f "$MERGED_FILE"
wget "$QUEBEC_URL" -O "$QUEBEC_FILE"
wget "$ONTARIO_URL" -O "$ONTARIO_FILE"
wget "$NB_URL" -O "$NB_FILE"
wget "$MAINE_URL" -O "$MAINE_FILE"
wget "$VERMONT_URL" -O "$VERMONT_FILE"
wget "$NEWYORK_URL" -O "$NEWYORK_FILE"

echo "   -> Extractions géographiques ciblées..."
osmium extract --bbox -76.55,41.0,-74.0,57.0 "$ONTARIO_FILE" -o "$ONT_EAST_FILE"
osmium extract --bbox -79.0,43.0,-71.0,45.5 "$NEWYORK_FILE" -o "$NY_NORTH_FILE"

echo "   -> Fusion Osmium..."
osmium merge "$QUEBEC_FILE" "$ONT_EAST_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NY_NORTH_FILE" -o "$MERGED_FILE"

rm -f "$QUEBEC_FILE" "$ONTARIO_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NEWYORK_FILE" "$NY_NORTH_FILE" "$ONT_EAST_FILE"

# ==============================================================================
# 2. IMPORTATION OSM2PGSQL (STAGING)
# ==============================================================================
echo "--- Étape 2 : Importation osm2pgsql (Schéma import) ---"
$PSQL_CMD -c "CREATE SCHEMA IF NOT EXISTS import;"
osm2pgsql --cache 2000 --slim --drop -H db -U postgres -d carte -O flex -S import.lua --schema import "$MERGED_FILE"
rm -f "$MERGED_FILE"

# ==============================================================================
# 3. CALCULS GÉOSPATIAUX (TURBO MODE FULL 3857)
# ==============================================================================
echo "--- Étape 3 : Traitements SQL (Zéro ST_Transform) ---"

$PSQL_CMD <<EOF
SET search_path = import, public;
SET synchronous_commit = off;

-- A. Villes subdivisées et converties en 3857
DROP TABLE IF EXISTS import.city_subdivided CASCADE;
CREATE TABLE import.city_subdivided AS 
SELECT name, ST_Transform(ST_Subdivide(ST_MakeValid(geom), 256), 3857) as geom 
FROM public.city;
CREATE INDEX ON import.city_subdivided USING GIST (geom);

-- B. Créer le garde-fou 'bounds' en 3857 (basé sur tes polygones SRTM 3857)
DROP TABLE IF EXISTS import.srtm_boundary;
CREATE TABLE import.srtm_boundary AS 
SELECT ST_SetSRID(ST_Extent(geom), 3857) as geom 
FROM public.srtm_elevation_polygons;

-- C. Structure EDGE (Tout en 3857)
CREATE SEQUENCE IF NOT EXISTS edge_id;
DROP TABLE IF EXISTS import.edge CASCADE;
CREATE TABLE import.edge (
    id bigint PRIMARY KEY,
    source bigint, target bigint,
    x1 double precision, y1 double precision,
    x2 double precision, y2 double precision,
    way_id bigint, tags jsonb, geom geometry(LineString, 3857),
    city_name text, in_bicycle_route boolean,
    elevation_start smallint, elevation_end smallint
);

-- D. INSERTION MASSIVE (Performance Maximale)
INSERT INTO import.edge
WITH segments AS (
    SELECT 
        aw.way_id, aw.tags, aw.in_bicycle_route,
        aw.nodes[(segment.path)[1]] as s_id,
        aw.nodes[(segment.path)[1] + 1] as t_id,
        ST_PointN(segment.geom, 1) as p1,
        ST_PointN(segment.geom, 2) as p2,
        segment.geom as s_geom 
    FROM import.all_way aw
    CROSS JOIN LATERAL ST_DumpSegments(aw.geom) as segment
),
bounds AS (SELECT geom FROM import.srtm_boundary LIMIT 1)
SELECT
    nextval('edge_id'), s.s_id, s.t_id,
    ST_X(s.p1), ST_Y(s.p1), 
    ST_X(s.p2), ST_Y(s.p2),
    s.way_id, s.tags, s.s_geom,
    c.name, s.in_bicycle_route,
    r1.elevation::smallint, r2.elevation::smallint
FROM segments s
-- Jointure ville (3857 vs 3857)
LEFT JOIN import.city_subdivided c ON ST_Intersects(s.p1, c.geom)
-- Jointure SRTM (3857 vs 3857) avec court-circuit
LEFT JOIN LATERAL (
    SELECT elevation FROM public.srtm_elevation_polygons 
    WHERE ST_Intersects(s.p1, (SELECT geom FROM bounds))
    AND ST_Covers(geom, s.p1) 
    LIMIT 1
) r1 ON true
LEFT JOIN LATERAL (
    SELECT elevation FROM public.srtm_elevation_polygons 
    WHERE ST_Intersects(s.p2, (SELECT geom FROM bounds))
    AND ST_Covers(geom, s.p2) 
    LIMIT 1
) r2 ON true;

-- E. Indexation et Statistiques
CREATE INDEX ON import.edge USING GIST (geom);
CREATE INDEX ON import.edge (source);
CREATE INDEX ON import.edge (target);
CREATE INDEX ON import.edge (city_name);
CREATE INDEX ON import.edge (way_id);
ANALYZE import.edge;

-- F. Vues de recherche
CREATE MATERIALIZED VIEW import.address_range AS
    SELECT a.geom, a.odd_even, an1.city, an1.street, an1.housenumber as start, an2.housenumber as end,
           (to_tsvector('simple', unaccent(coalesce(an1.street, '') || ' ' || coalesce(an1.city, '')))) as tsvector
    FROM import.address a
    JOIN import.address_node an1 ON a.housenumber1 = an1.node_id
    JOIN import.address_node an2 ON a.housenumber2 = an2.node_id;

CREATE MATERIALIZED VIEW import.name_query AS
    SELECT name, geom, tags, to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector FROM import.name;

CREATE INDEX ON import.address_range USING GIN (tsvector);
CREATE INDEX ON import.name_query USING GIN (tsvector);
EOF

# ==============================================================================
# 4. ÉCHANGE ATOMIQUE ET OCÉANS
# ==============================================================================
echo "--- Étape 4 : Bascule vers Public et Océans ---"

$PSQL_CMD <<EOF
BEGIN;
DO \$\$ 
DECLARE r RECORD;
BEGIN
    FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = 'import') LOOP
        EXECUTE 'DROP TABLE IF EXISTS public.' || quote_ident(r.tablename) || ' CASCADE';
        EXECUTE 'ALTER TABLE import.' || quote_ident(r.tablename) || ' SET SCHEMA public';
    END LOOP;
    FOR r IN (SELECT matviewname FROM pg_matviews WHERE schemaname = 'import') LOOP
        EXECUTE 'DROP MATERIALIZED VIEW IF EXISTS public.' || quote_ident(r.matviewname) || ' CASCADE';
        EXECUTE 'ALTER MATERIALIZED VIEW import.' || quote_ident(r.matviewname) || ' SET SCHEMA public';
    END LOOP;
    FOR r IN (SELECT relname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'import' AND c.relkind = 'S') LOOP
        EXECUTE 'DROP SEQUENCE IF EXISTS public.' || quote_ident(r.relname) || ' CASCADE';
        EXECUTE 'ALTER SEQUENCE import.' || quote_ident(r.relname) || ' SET SCHEMA public';
    END LOOP;
END \$\$;
COMMIT;
EOF

# Import des océans
OCEAN_EXISTS=$($PSQL_CMD -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'ocean');" | xargs)
if [ "$OCEAN_EXISTS" != "t" ]; then
    echo "Importation des océans..."
    wget "https://osmdata.openstreetmap.de/download/water-polygons-split-4326.zip" -O ocean.zip
    unzip -q ocean.zip
    shp2pgsql -s 4326 water-polygons-split-4326/water_polygons.shp ocean | $PSQL_CMD
    $PSQL_CMD -c "CREATE INDEX ON ocean USING GIST (geom);"
    rm -rf water-polygons-split-4326 ocean.zip
fi

echo "🚀 Importation terminée avec succès !"
