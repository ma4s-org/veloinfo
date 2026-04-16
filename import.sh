#!/usr/bin/bash
set -e
set -o pipefail

# ==============================================================================
# CONFIGURATION
# ==============================================================================
PSQL_CMD="psql -h db -U postgres -d carte -v ON_ERROR_STOP=1"

# URLs régionales
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
NEWYORK_NORTH_FILE="new-york-north.osm.pbf"
ONTARIO_EAST_FILE="ontario-east.osm.pbf"
MERGED_FILE="regions.osm.pbf"

# ==============================================================================
# 1. TÉLÉCHARGEMENT ET PRÉPARATION DES DONNÉES
# ==============================================================================
echo "--- 1. Téléchargement et Fusion des données ---"

# On télécharge seulement si le fichier n'est pas déjà là
[ ! -f "$QUEBEC_FILE" ] && wget "$QUEBEC_URL" -O "$QUEBEC_FILE"
[ ! -f "$ONTARIO_FILE" ] && wget "$ONTARIO_URL" -O "$ONTARIO_FILE"
[ ! -f "$NB_FILE" ] && wget "$NB_URL" -O "$NB_FILE"
[ ! -f "$MAINE_FILE" ] && wget "$MAINE_URL" -O "$MAINE_FILE"
[ ! -f "$VERMONT_FILE" ] && wget "$VERMONT_URL" -O "$VERMONT_FILE"
[ ! -f "$NEWYORK_FILE" ] && wget "$NEWYORK_URL" -O "$NEWYORK_FILE"

if [ ! -f "$MERGED_FILE" ]; then
    echo "   -> Extraction Est Ontario (Longitude > -76.55)..."
    osmium extract --bbox -76.5555,41.0,-74.0,57.0 "$ONTARIO_FILE" -o "$ONTARIO_EAST_FILE"
    
    echo "   -> Extraction Nord New York..."
    osmium extract --bbox -79.0,43.0,-71.0,45.5 "$NEWYORK_FILE" -o "$NEWYORK_NORTH_FILE"

    echo "   -> Fusion de tous les fichiers (Merge)..."
    osmium merge "$QUEBEC_FILE" "$ONTARIO_EAST_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NEWYORK_NORTH_FILE" -o "$MERGED_FILE"
    
    # Nettoyage espace disque immédiat
    rm -f "$QUEBEC_FILE" "$ONTARIO_FILE" "$ONTARIO_EAST_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NEWYORK_FILE" "$NEWYORK_NORTH_FILE"
fi

# ==============================================================================
# 2. IMPORTATION OSM2PGSQL
# ==============================================================================
echo "--- 2. Lancement de osm2pgsql (Mode Slim) ---"
$PSQL_CMD -c "CREATE SCHEMA IF NOT EXISTS import;"
osm2pgsql --cache 2000 --slim --drop -H db -U postgres -d carte -O flex -S import.lua --schema import "$MERGED_FILE"

# ==============================================================================
# 3. ÉLÉVATION SRTM
# ==============================================================================
echo "--- 3. Vérification des données SRTM ---"
SRTM_EXISTS=$($PSQL_CMD -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'srtm_elevation');" | xargs)
if [ "$SRTM_EXISTS" != "t" ]; then
    /app/import_srtm.sh
fi

# ==============================================================================
# 4. CALCULS GÉOSPATIAUX (MODE PRODUCTION - STABLE EN RAM)
# ==============================================================================
echo "--- 4. Calculs géospatiaux (Villes + Edge) ---"

$PSQL_CMD <<EOF
-- Réglages de session pour la prod (Évite de swapper sur le disque)
SET search_path = import, public;
SET synchronous_commit = off;
SET work_mem = '1GB'; 
SET maintenance_work_mem = '2GB';

-- 4a. Optimisation des villes (Subdivide 256)
DROP TABLE IF EXISTS import.city_subdivided CASCADE;
CREATE TABLE import.city_subdivided AS 
WITH exploded AS (
    SELECT name, (ST_Dump(ST_MakeValid(geom))).geom as g FROM public.city
),
simplified AS (
    SELECT name, ST_SimplifyPreserveTopology(g, 0.0001) as g 
    FROM exploded 
    WHERE ST_GeometryType(g) = 'ST_Polygon'
)
SELECT name, ST_Subdivide(ST_MakeValid(g), 256) as geom FROM simplified;
CREATE INDEX ON import.city_subdivided USING GIST (geom);
ANALYZE import.city_subdivided;

-- 4b. Enveloppe SRTM pour le court-circuit (Évite de chercher hors zone)
DROP TABLE IF EXISTS import.srtm_boundary;
CREATE TABLE import.srtm_boundary AS 
SELECT ST_SetSRID(ST_Extent(ST_ConvexHull(rast)), 4326) as geom FROM public.srtm_elevation;

-- 4c. Création de la table EDGE (Table physique au lieu de vue pour le volume)
CREATE SEQUENCE IF NOT EXISTS edge_id;
DROP TABLE IF EXISTS import.edge CASCADE;
CREATE TABLE import.edge (
    id bigint PRIMARY KEY,
    way_id bigint,
    tags jsonb,
    geom geometry(LineString, 4326),
    x1 double precision, y1 double precision,
    x2 double precision, y2 double precision,
    city_name text,
    in_bicycle_route boolean,
    elevation_start smallint,
    elevation_end smallint
);

-- 4d. INSERTION MASSIVE (Optimisée)
INSERT INTO import.edge
WITH transformed_ways AS (
    -- On transforme toute la route en 4326 une seule fois
    SELECT way_id, tags, in_bicycle_route, ST_Transform(geom, 4326) as geom_4326
    FROM import.all_way
),
segments AS (
    SELECT
        tw.way_id, tw.tags, tw.in_bicycle_route,
        ST_PointN(segment.geom, 1) as pt1,
        ST_PointN(segment.geom, 2) as pt2,
        segment.geom as segment_geom
    FROM transformed_ways tw
    CROSS JOIN LATERAL ST_DumpSegments(tw.geom_4326) as segment
),
bounds AS (SELECT geom FROM import.srtm_boundary LIMIT 1)
SELECT
    nextval('edge_id'),
    s.way_id, s.tags, s.segment_geom,
    ST_X(s.pt1), ST_Y(s.pt1),
    ST_X(s.pt2), ST_Y(s.pt2),
    c.name,
    s.in_bicycle_route,
    -- ÉLÉVATION START
    CASE 
        WHEN ST_Intersects(s.pt1, (SELECT geom FROM bounds)) 
        THEN (SELECT ST_Value(rast, 1, s.pt1) FROM public.srtm_elevation WHERE ST_Intersects(rast, s.pt1) LIMIT 1)
        ELSE NULL 
    END,
    -- ÉLÉVATION END
    CASE 
        WHEN ST_Intersects(s.pt2, (SELECT geom FROM bounds)) 
        THEN (SELECT ST_Value(rast, 1, s.pt2) FROM public.srtm_elevation WHERE ST_Intersects(rast, s.pt2) LIMIT 1)
        ELSE NULL 
    END
FROM segments s
LEFT JOIN import.city_subdivided c ON ST_Intersects(s.pt1, c.geom);

-- 4e. Indexation finale (la totale)
CREATE INDEX ON import.edge USING GIST (geom); -- Spatial
CREATE INDEX ON import.edge (way_id);          -- Lien OSM
CREATE INDEX ON import.edge (source);          -- Routage
CREATE INDEX ON import.edge (target);          -- Routage
CREATE INDEX ON import.edge (city_name);       -- Filtres géographiques
ANALYZE import.edge;
EOF

# ==============================================================================
# 5. VUES DE RECHERCHE ET FINALISATION
# ==============================================================================
echo "--- 5. Création des vues de recherche et migration finale ---"
$PSQL_CMD <<EOF
SET search_path = import, public;

-- Vues matérialisées pour la recherche (moins lourdes que edge)
CREATE MATERIALIZED VIEW IF NOT EXISTS address_range AS
    SELECT a.geom, a.odd_even, an1.city, an1.street, an1.housenumber as start, an2.housenumber as end,
           (to_tsvector('simple', unaccent(coalesce(an1.street, '') || ' ' || coalesce(an1.city, '')))) as tsvector
    FROM address a
    JOIN address_node an1 ON a.housenumber1 = an1.node_id
    JOIN address_node an2 ON a.housenumber2 = an2.node_id
    UNION
    SELECT geom, CASE WHEN MOD(housenumber, 2) = 0 THEN 'even' ELSE 'odd' END as odd_even,
           city, street, housenumber as start, housenumber as end,
           (to_tsvector('simple', unaccent(coalesce(street, '') || ' ' || coalesce(city, '')))) as tsvector
    FROM address_node an;
CREATE INDEX ON address_range USING GIN (tsvector);
CREATE INDEX ON address_range USING GIST (geom);

CREATE MATERIALIZED VIEW IF NOT EXISTS name_query AS
    SELECT name, geom, tags, to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector FROM name
    UNION SELECT name, ST_Centroid(geom), tags, to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector FROM building WHERE name IS NOT NULL
    UNION SELECT name, ST_Centroid(geom), tags, to_tsvector('simple', unaccent(name)) as tsvector FROM landcover WHERE name IS NOT NULL;
CREATE INDEX ON name_query USING GIN (tsvector);
CREATE INDEX ON name_query USING GIST (geom);

-- MIGRATION ATOMIQUE DES TABLES VERS PUBLIC
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

# Nettoyage
$PSQL_CMD -c "DROP SCHEMA IF EXISTS import CASCADE;"
rm -f "$MERGED_FILE"

echo "🚀 IMPORTATION TERMINÉE AVEC SUCCÈS !"
