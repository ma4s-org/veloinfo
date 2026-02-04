#!/usr/bin/bash
rm -f quebec-latest.osm.pbf
wget https://download.geofabrik.de/north-america/canada/quebec-latest.osm.pbf 
osm2pgsql --cache 4000 --drop -H db -U postgres -d carte -O flex -S import.lua quebec-latest.osm.pbf


psql -h db -U postgres -d carte -c "
                     CREATE EXTENSION IF NOT EXISTS postgis;
                     CREATE EXTENSION IF NOT EXISTS unaccent;"

# Check if SRTM elevation data has been imported
echo "Checking SRTM elevation data..."
SRTM_EXISTS=$(psql -h db -U postgres -d carte -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'srtm_elevation_polygons');" 2>/dev/null | xargs)

if [ "$SRTM_EXISTS" = "t" ]; then
    SRTM_COUNT=$(psql -h db -U postgres -d carte -t -c "SELECT COUNT(*) FROM srtm_elevation_polygons;" 2>/dev/null | xargs)
    if [ "$SRTM_COUNT" -gt 0 ]; then
        echo "✓ SRTM elevation data already imported ($SRTM_COUNT polygons)"
    else
        echo "SRTM table exists but is empty, importing..."
        /app/import_srtm.sh
    fi
else
    echo "SRTM elevation data not found, importing..."
    /app/import_srtm.sh
fi

echo ""
echo "Recreating materialized views to apply any changes."
psql -h db -U postgres -d carte -c "
                                drop materialized view if exists last_cycleway_score cascade;
                                CREATE MATERIALIZED VIEW last_cycleway_score
                                AS
                                    SELECT *
                                        FROM (
                                            SELECT c.*, cs.score,
                                            ROW_NUMBER() OVER (PARTITION BY c.way_id ORDER BY cs.created_at DESC) as rn
                                            FROM cyclability_score cs 
                                            JOIN cycleway_way c ON c.way_id = ANY(cs.way_ids)
                                        ) t
                                    WHERE t.rn = 1;
                                CREATE UNIQUE INDEX last_cycleway_score_way_id_idx ON last_cycleway_score(way_id);

                                drop materialized view if exists edge;
                                drop materialized view if exists _all_way_edge;
                                drop sequence if exists edge_id;
                                CREATE SEQUENCE edge_id;
                                create materialized view _all_way_edge as
                                    select 
                                        aw.way_id, 
                                        nodes, 
                                        geom,
                                        aw.name,
                                        aw.tags,
                                        in_bicycle_route
                                    from all_way aw;       
                                create index _all_way_edge_way_id_idx on _all_way_edge (way_id);

                                CREATE MATERIALIZED VIEW edge 
                                AS SELECT  
                                    nextval('edge_id') as id,
                                    awe.nodes[(segment).path[1]] as source,
                                    awe.nodes[(segment).path[1]+1] as target,
                                    st_x(st_transform(ST_PointN((segment).geom, 1), 4326)) as x1,
                                    st_y(st_transform(ST_PointN((segment).geom, 1), 4326)) as y1,
                                    st_x(st_transform(ST_PointN((segment).geom, 2), 4326)) as x2,
                                    st_y(st_transform(ST_PointN((segment).geom, 2), 4326)) as y2,
                                    awe.way_id,
                                    awe.tags,
                                    (segment).geom,
                                    c.name as city_name,
                                    in_bicycle_route,
                                    (SELECT elevation FROM srtm_elevation_polygons 
                                     WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint(
                                        st_x(st_transform(ST_PointN((segment).geom, 1), 4326)),
                                        st_y(st_transform(ST_PointN((segment).geom, 1), 4326))
                                     ), 4326)) LIMIT 1) as elevation_start,
                                    (SELECT elevation FROM srtm_elevation_polygons 
                                     WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint(
                                        st_x(st_transform(ST_PointN((segment).geom, 2), 4326)),
                                        st_y(st_transform(ST_PointN((segment).geom, 2), 4326))
                                     ), 4326)) LIMIT 1) as elevation_end
                                from _all_way_edge awe
                                CROSS JOIN LATERAL ST_DumpSegments(awe.geom) as segment
                                left join city c on ST_Within((segment).geom, c.geom)
                                where awe.nodes[(segment).path[1]+1] is not null;       

                                CREATE INDEX edge_way_id_idx ON edge(way_id);
                                CREATE INDEX edge_geom_idx ON edge using gist(geom);
                                CREATE UNIQUE INDEX edge_id_idx ON edge(id);
                                CREATE INDEX edge_source_idx ON public.edge ("source");
                                CREATE INDEX edge_target_idx ON public.edge ("target");
                                CREATE INDEX edge_city_name_idx ON public.edge (city_name);

                                drop materialized view if exists address_range;
                                create materialized view address_range as
                                    select 
                                        a.geom,
                                        a.odd_even,
                                        an1.city,
                                        an1.street,
                                        an1.housenumber as start,
                                        an2.housenumber as end,
                                        (to_tsvector('simple', unaccent(coalesce(an1.street, '') || ' ' || coalesce(an1.city, '')))) as tsvector
                                    from address a
                                    join address_node an1 on a.housenumber1 = an1.node_id
                                    join address_node an2 on a.housenumber2 = an2.node_id
                                    union
                                    select 
                                        geom,
                                        CASE
                                            WHEN MOD(housenumber, 2) = 0 THEN 'even'
                                            ELSE 'odd'
                                        END as odd_even,
                                        city,
                                        street,
                                        housenumber as start,
                                        housenumber as end,
                                        (to_tsvector('simple', unaccent(coalesce(street, '') || ' ' || coalesce(city, '')))) as tsvector
                                from address_node an;

                                CREATE INDEX textsearch_idx ON address_range USING GIN (tsvector);
                                CREATE INDEX address_range_geom_idx ON address_range using gist(geom);

                                drop materialized view if exists name_query;
                                create materialized view name_query as
                                    select 
                                        name,
                                        geom,
                                        tags,
                                        to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector
                                    from name
                                    union
                                    select
                                        name,
                                        ST_Centroid(geom),
                                        tags,
                                        to_tsvector('simple', unaccent(coalesce(name, ''))) as tsvector
                                    from building
                                    where name is not null
                                    union
                                    select
                                        name,
                                        ST_Centroid(geom),
                                        tags,
                                        to_tsvector('simple', unaccent(name)) as tsvector
                                        from landcover
                                        where name is not null;

                                CREATE INDEX name_query_textsearch_idx ON name_query USING GIN (tsvector);
                                CREATE INDEX name_query_geom_idx ON name_query using gist(geom);
                                "