#!/usr/bin/bash
set -e
set -o pipefail

# Configuration
PSQL_CMD="psql -h db -U postgres -d carte -v ON_ERROR_STOP=1"
# URLs régionales pour Canada + USA nord-est
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
MERGED_FILE="regions.osm.pbf"

# 1. Téléchargement des données
echo "Téléchargement des données OSM (Canada + USA nord-est)..."
rm -f "$QUEBEC_FILE" "$ONTARIO_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NEWYORK_FILE" "$NEWYORK_NORTH_FILE" "$MERGED_FILE"
wget -q "$QUEBEC_URL" -O "$QUEBEC_FILE"
wget -q "$ONTARIO_URL" -O "$ONTARIO_FILE"
wget -q "$NB_URL" -O "$NB_FILE"
wget -q "$MAINE_URL" -O "$MAINE_FILE"
wget -q "$VERMONT_URL" -O "$VERMONT_FILE"
wget -q "$NEWYORK_URL" -O "$NEWYORK_FILE"

# Extraire le nord de New York (latitude > 43°N, env. frontière Québec)
echo "Extraction du nord de New York..."
osmium extract --bbox -79.0,43.0,-71.0,45.5 "$NEWYORK_FILE" -o "$NEWYORK_NORTH_FILE"

# Fusion de tous les fichiers avec osmium
echo "Fusion des données avec osmium..."
osmium merge "$QUEBEC_FILE" "$ONTARIO_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NEWYORK_NORTH_FILE" -o "$MERGED_FILE"
rm -f "$QUEBEC_FILE" "$ONTARIO_FILE" "$NB_FILE" "$MAINE_FILE" "$VERMONT_FILE" "$NEWYORK_FILE" "$NEWYORK_NORTH_FILE"
PBF_FILE="$MERGED_FILE"

# 2. Préparation du schéma temporaire
echo "Préparation du schéma temporaire 'import'..."
$PSQL_CMD -c "DROP SCHEMA IF EXISTS import CASCADE; CREATE SCHEMA import;"

# 3. Importation optimisée avec osm2pgsql
# --slim : Utilise la DB pour les données temporaires (évite les fichiers binaires géants)
echo "Lancement de osm2pgsql (Mode Slim)..."
osm2pgsql --cache 2000 --slim --drop -H db -U postgres -d carte -O flex -S import.lua --schema import "$PBF_FILE"

# Nettoyage
rm -f "$PBF_FILE"

# S'assurer que les extensions sont là
$PSQL_CMD -c "CREATE EXTENSION IF NOT EXISTS postgis; CREATE EXTENSION IF NOT EXISTS unaccent;"

# 4. Vérification des données SRTM
echo "Vérification des données d'élévation SRTM..."
SRTM_EXISTS=$($PSQL_CMD -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'srtm_elevation_polygons');" | xargs)

if [ "$SRTM_EXISTS" != "t" ]; then
    echo "Données SRTM absentes, importation..."
    /app/import_srtm.sh
fi

# 5. Création des vues matérialisées dans le schéma 'import'
echo "Création des vues matérialisées dans le schéma 'import'..."
$PSQL_CMD <<EOF
    SET search_path = import, public;

    CREATE MATERIALIZED VIEW last_cycleway_score AS
        SELECT * FROM (
            SELECT c.*, cs.score,
            ROW_NUMBER() OVER (PARTITION BY c.way_id ORDER BY cs.created_at DESC) as rn
            FROM public.cyclability_score cs 
            JOIN cycleway_way c ON c.way_id = ANY(cs.way_ids)
        ) t WHERE t.rn = 1;
    CREATE UNIQUE INDEX last_cycleway_score_way_id_idx ON last_cycleway_score(way_id);

    CREATE SEQUENCE edge_id;

    CREATE MATERIALIZED VIEW _all_way_edge AS
        SELECT way_id, nodes, geom, name, tags, in_bicycle_route FROM all_way;       
    CREATE INDEX _all_way_edge_way_id_idx ON _all_way_edge (way_id);

    CREATE MATERIALIZED VIEW edge AS
    SELECT
        nextval('edge_id') as id,
        awe.nodes[(segment).path[1]] as source,
        awe.nodes[(segment).path[1]+1] as target,
        st_x(st_transform(ST_PointN((segment).geom, 1), 4326)) as x1,
        st_y(st_transform(ST_PointN((segment).geom, 1), 4326)) as y1,
        st_x(st_transform(ST_PointN((segment).geom, 2), 4326)) as x2,
        st_y(st_transform(ST_PointN((segment).geom, 2), 4326)) as y2,
        awe.way_id, awe.tags, (segment).geom, c.name as city_name, in_bicycle_route,
        (SELECT elevation FROM public.srtm_elevation_polygons
         WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint(
            st_x(st_transform(ST_PointN((segment).geom, 1), 4326)),
            st_y(st_transform(ST_PointN((segment).geom, 1), 4326))
         ), 4326)) LIMIT 1) as elevation_start,
        (SELECT elevation FROM public.srtm_elevation_polygons
         WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint(
            st_x(st_transform(ST_PointN((segment).geom, 2), 4326)),
            st_y(st_transform(ST_PointN((segment).geom, 2), 4326))
         ), 4326)) LIMIT 1) as elevation_end
    FROM _all_way_edge awe
    CROSS JOIN LATERAL ST_DumpSegments(awe.geom) as segment
    LEFT JOIN city c ON ST_Within((segment).geom, c.geom)
    WHERE awe.nodes[(segment).path[1]+1] IS NOT NULL;

    CREATE INDEX edge_way_id_idx ON edge(way_id);
    CREATE INDEX edge_geom_idx ON edge using gist(geom);
    CREATE UNIQUE INDEX edge_id_idx ON edge(id);
    CREATE INDEX edge_source_idx ON edge (source);
    CREATE INDEX edge_target_idx ON edge (target);
    CREATE INDEX edge_city_name_idx ON edge (city_name);

    CREATE MATERIALIZED VIEW address_range AS
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

    CREATE INDEX textsearch_idx ON address_range USING GIN (tsvector);
    CREATE INDEX address_range_geom_idx ON address_range using gist(geom);

    CREATE MATERIALIZED VIEW name_query AS
        SELECT name, geom, tags, to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector FROM name
        UNION SELECT name, ST_Centroid(geom), tags, to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector FROM building WHERE name IS NOT NULL
        UNION SELECT name, ST_Centroid(geom), tags, to_tsvector('simple', unaccent(name)) as tsvector FROM landcover WHERE name IS NOT NULL;

    CREATE INDEX name_query_textsearch_idx ON name_query USING GIN (tsvector);
    CREATE INDEX name_query_geom_idx ON name_query using gist(geom);
EOF

# 6. Échange atomique des schémas
echo "Échange des tables vers le schéma public..."
$PSQL_CMD <<EOF
BEGIN;
DO \$\$ 
DECLARE 
    r RECORD;
BEGIN
    FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = 'import') LOOP
        EXECUTE 'DROP TABLE IF EXISTS public.' || r.tablename || ' CASCADE';
        EXECUTE 'ALTER TABLE import.' || r.tablename || ' SET SCHEMA public';
    END LOOP;
    FOR r IN (SELECT matviewname FROM pg_matviews WHERE schemaname = 'import') LOOP
        EXECUTE 'DROP MATERIALIZED VIEW IF EXISTS public.' || r.matviewname || ' CASCADE';
        EXECUTE 'ALTER MATERIALIZED VIEW import.' || r.matviewname || ' SET SCHEMA public';
    END LOOP;
    FOR r IN (SELECT relname FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'import' AND c.relkind = 'S') LOOP
        EXECUTE 'DROP SEQUENCE IF EXISTS public.' || r.relname || ' CASCADE';
        EXECUTE 'ALTER SEQUENCE import.' || r.relname || ' SET SCHEMA public';
    END LOOP;
END \$\$;
COMMIT;
EOF

echo "Nettoyage..."
$PSQL_CMD -c "DROP SCHEMA IF EXISTS import CASCADE;"

echo "Importation terminée avec succès !"
