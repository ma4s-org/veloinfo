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
                                            aw.tags,
                                            case
                                                when tags->>'bicycle' = 'no' then 1 / 0.0001
                                                when tags->>'highway' = 'steps' then 1 / 0.05
                                                when tags->>'bicycle' = 'discouraged' then 1 / 0.1
                                                when tags->>'bicycle' = 'dismount' then 1 / 0.3
                                                when tags->>'highway' = 'cycleway' then 1 / 1
                                                when tags->>'cycleway' = 'track' then 1 / 0.9
                                                when tags->>'cycleway:both' = 'track' then 1 / 0.9
                                                when tags->>'cycleway:left' = 'track' then 1 / 0.9
                                                when tags->>'cycleway:right' = 'track' then 1 / 0.9
                                                when tags->>'cycleway' = 'lane' then 1 / 0.8
                                                when tags->>'cycleway:both' = 'lane' then 1 / 0.8
                                                when tags->>'cycleway:left' = 'lane' then 1 / 0.8
                                                when tags->>'cycleway:right' = 'lane' then 1 / 0.8
                                                when tags->>'cycleway' = 'shared_lane' then 1 / 0.7
                                                when tags->>'cycleway:both' = 'shared_lane' then 1 / 0.7
                                                when tags->>'cycleway:left' = 'shared_lane' then 1 / 0.7
                                                when tags->>'cycleway:right' = 'shared_lane' then 1 / 0.7
                                                when tags->>'highway' = 'path' and tags->>'bicycle' = 'yes' then 1 / 0.6
                                                when tags->>'highway' = 'footway' and tags->>'bicycle' = 'yes' then 1 / 0.6
                                                when tags->>'highway' = 'residential' then 1 / 0.6
                                                when tags->>'highway' = 'unclassified' then 1 / 0.6
                                                when tags->>'bicycle' = 'designated' then 1 / 0.5
                                                when tags->>'highway' = 'tertiary' then 1 / 0.45
                                                when tags->>'highway' = 'tertiary_link' then 1 / 0.45
                                                when tags->>'highway' = 'footway' and tags->>'footway' = 'crossing' then 1 / 0.3
                                                when tags->>'highway' = 'secondary' then 1 / 0.3
                                                when tags->>'highway' = 'secondary_link' then 1 / 0.3
                                                when tags->>'bicycle' = 'yes' then 1 / 0.3
                                                when tags->>'highway' = 'service' then 1 / 0.3
                                                when tags->>'highway' = 'path' then 1 / 0.3
                                                when tags->>'cycleway' = 'separate' then 1 / 0.2
                                                when tags->>'cycleway:both' = 'separate' then 1 / 0.2
                                                when tags->>'cycleway:left' = 'separate' then 1 / 0.2
                                                when tags->>'cycleway:right' = 'separate' then 1 / 0.2
                                                when tags->>'highway' = 'primary' then 1 / 0.1
                                                when tags->>'highway' = 'trunk' then 1 / 0.1
                                                when tags->>'highway' = 'footway' then 1 / 0.1
                                                when tags->>'highway' = 'proposed' then 1 / 0.001
                                                when tags->>'highway' is not null then 1 / 0.01
                                                else 1 / 0.25
                                            end as cost_road
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
                                        score,
                                        (segment).geom,
                                        cost_road,
                                        st_length((segment).geom) *
                                        CASE
                                            WHEN score IS NULL THEN 
                                                cost_road
                                            WHEN score = 0 THEN 1 / 0.001
                                            ELSE cost_road * (1 / score)
                                        END as cost,
                                        st_length((segment).geom) *
                                        CASE
                                            when awe.tags->>'oneway:bicycle' = 'no' and score is not null and score != 0 then cost_road * (1 / score)
                                            when awe.tags->>'oneway:bicycle' = 'no' then cost_road
                                            when awe.tags->>'oneway' = 'no' and score is not null and score != 0 then cost_road * (1 / score)
                                            when awe.tags->>'oneway:bicycle' = 'yes' then 1 / 0.001
                                            when awe.tags->>'oneway' = 'yes' then 1 / 0.001
                                            WHEN score IS NULL THEN
                                                cost_road
                                            WHEN score = 0 THEN 1 / 0.001
                                            ELSE cost_road * (1 / score)
                                        END as reverse_cost
                                    from _all_way_edge awe
                                    left join  last_cycleway_score cs on cs.way_id = awe.way_id
                                    where awe.nodes[(segment).path[1]+1] is not null;       

                                    CREATE INDEX edge_way_id_idx ON edge(way_id);
                                    CREATE INDEX edge_geom_idx ON edge using gist(geom);
                                    CREATE UNIQUE INDEX edge_id_idx ON edge(id);

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