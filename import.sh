#!/usr/bin/bash
rm quebec-latest.osm.pbf
wget https://download.geofabrik.de/north-america/canada/quebec-latest.osm.pbf -O quebec-latest.osm.pbf

osm2pgsql -H db -U postgres -d carte -O flex -S import.lua quebec-latest.osm.pbf

psql -h db -U postgres -d carte -c "
                                    drop materialized view if exists bike_path;
                                    CREATE MATERIALIZED VIEW bike_path AS
                                        SELECT way_id,
                                                name,
                                                geom,
                                                source,
                                                target,
                                                kind,
                                                tags,
                                                nodes,
                                                case
                                                    when score is null then -1
                                                    else score
                                                end as score
                                            FROM (
                                                SELECT c.*, cs.score,
                                                ROW_NUMBER() OVER (PARTITION BY c.way_id ORDER BY cs.created_at DESC) as rn
                                                FROM cycleway_way c 
                                                LEFT JOIN cyclability_score cs ON c.way_id = ANY(cs.way_ids)
                                            ) t
                                        WHERE t.rn = 1;
                                    CREATE UNIQUE INDEX bike_path_way_id_idx ON bike_path(way_id);
                                    CREATE INDEX edge_geom_gist ON bike_path USING gist(geom);

                                    CREATE MATERIALIZED VIEW bike_path_far AS
                                        SELECT way_id,
                                                name,
                                                geom,
                                                source,
                                                target,
                                                kind,
                                                tags,
                                                nodes,
                                                case
                                                    when score is null then -1
                                                    else score
                                                end as score
                                            FROM (
                                                SELECT c.*, cs.score,
                                                ROW_NUMBER() OVER (PARTITION BY c.way_id ORDER BY cs.created_at DESC) as rn
                                                FROM cycleway_way_far c 
                                                LEFT JOIN cyclability_score cs ON c.way_id = ANY(cs.way_ids)
                                            ) t
                                        WHERE t.rn = 1;
                                    CREATE UNIQUE INDEX bike_path_far_way_id_idx ON bike_path_far(way_id);
                                    CREATE INDEX edge_geom_far_gist ON bike_path_far USING gist(geom);

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
                                    
                                    drop materialized view if exists _all_way_edge;
                                    drop sequence if exists edge_id;
                                    CREATE SEQUENCE edge_id;
                                    create materialized view _all_way_edge as
                                        select 
                                            nextval('edge_id')  as id,
                                            aw.way_id, 
                                            unnest(nodes) as node,
                                            nodes, 
                                            ST_DumpSegments(geom) as segment,
                                            aw.name,
                                            aw.tags
                                        from all_way aw;       
                                    create unique index _all_way_edge_id_idx on _all_way_edge (id);
                                    create index _all_way_edge_way_id_idx on _all_way_edge (way_id);

                                    drop materialized view if exists edge;
                                    CREATE MATERIALIZED VIEW edge 
                                    AS SELECT  
                                        id,
                                        node as source,
                                        awe.nodes[(segment).path[1]+1] as target,
                                        st_x(st_transform(ST_PointN((segment).geom, 1), 4326)) as x1,
                                        st_y(st_transform(ST_PointN((segment).geom, 1), 4326)) as y1,
                                        st_x(st_transform(ST_PointN((segment).geom, 2), 4326)) as x2,
                                        st_y(st_transform(ST_PointN((segment).geom, 2), 4326)) as y2,
                                        awe.way_id,
                                        awe.tags,
                                        score,
                                        (segment).geom
                                    from _all_way_edge awe
                                    left join  last_cycleway_score cs on cs.way_id = awe.way_id
                                    where awe.nodes[(segment).path[1]+1] is not null;       

                                    CREATE INDEX edge_way_id_idx ON edge(way_id);
                                    CREATE INDEX edge_geom_idx ON edge using gist(geom);
                                    CREATE UNIQUE INDEX edge_id_idx ON edge(id);
                                    CREATE INDEX edge_source_idx ON public.edge ("source");
                                    CREATE INDEX edge_target_idx ON public.edge ("target");

                                    drop materialized view if exists address_range;
                                    create materialized view address_range as
                                        select 
                                        	a.geom,
                                            a.odd_even,
                                        	an1.city,
                                        	an1.street,
                                        	an1.housenumber as start,
                                        	an2.housenumber as end,
                                        	(to_tsvector('french', coalesce(an1.street, '') || ' ' || coalesce(an1.city, ''))) as tsvector
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
                                        	(to_tsvector('french', coalesce(street, '') || ' ' || coalesce(city, ''))) as tsvector
                                        from address_node an;

                                    CREATE INDEX textsearch_idx ON address_range USING GIN (tsvector);
                                    CREATE INDEX address_range_geom_idx ON address_range using gist(geom);

                                    drop materialized view if exists name_query;
                                    create materialized view name_query as
                                        select 
                                            name,
                                        	geom,
                                            tags,
                                        	to_tsvector('french', name) as tsvector
                                        from name
                                        union
                                        select
                                            name,
                                            ST_Centroid(geom),
                                            tags,
                                            to_tsvector('french', name) as tsvector
                                        from building
                                        where name is not null
                                        union
                                        select
                                            name,
                                            ST_Centroid(geom),
                                            tags,
                                            to_tsvector('french', name) as tsvector
                                        from landcover
                                        where name is not null;

                                    CREATE INDEX name_query_textsearch_idx ON name_query USING GIN (tsvector);
                                    CREATE INDEX name_query_geom_idx ON name_query using gist(geom);
                                    "