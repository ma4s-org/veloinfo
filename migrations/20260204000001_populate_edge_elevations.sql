-- Optional migration to add elevation support to edges
-- This migration assumes the edge materialized view already has elevation columns
-- (added via import.sh or manually)

-- Helper function to refresh edge materialized view with elevation data populated
-- Call this after SRTM data is loaded to populate the elevation columns
CREATE OR REPLACE FUNCTION refresh_edge_with_elevations() RETURNS void AS $$
BEGIN
    -- Refresh the edge materialized view
    -- This will recreate it, so we need to ensure the definition includes elevation lookups
    RAISE NOTICE 'Starting edge refresh with elevation data...';
    
    -- Drop and recreate edge materialized view with elevation data
    DROP MATERIALIZED VIEW IF EXISTS edge CASCADE;
    
    CREATE MATERIALIZED VIEW edge AS
      SELECT nextval('edge_id'::regclass) AS id,
        awe.nodes[segment.path[1]] AS source,
        awe.nodes[segment.path[1] + 1] AS target,
        st_x(st_transform(st_pointn(segment.geom, 1), 4326)) AS x1,
        st_y(st_transform(st_pointn(segment.geom, 1), 4326)) AS y1,
        st_x(st_transform(st_pointn(segment.geom, 2), 4326)) AS x2,
        st_y(st_transform(st_pointn(segment.geom, 2), 4326)) AS y2,
        awe.way_id,
        awe.tags,
        segment.geom,
        c.name AS city_name,
        awe.in_bicycle_route,
        (SELECT ST_Value(rast, ST_SetSRID(ST_MakePoint(
            st_x(st_transform(st_pointn(segment.geom, 1), 4326)),
            st_y(st_transform(st_pointn(segment.geom, 1), 4326))
        ), 4326)) FROM srtm_elevation 
         WHERE ST_Intersects(rast, ST_SetSRID(ST_MakePoint(
            st_x(st_transform(st_pointn(segment.geom, 1), 4326)),
            st_y(st_transform(st_pointn(segment.geom, 1), 4326))
         ), 4326)) LIMIT 1) AS elevation_start,
        (SELECT ST_Value(rast, ST_SetSRID(ST_MakePoint(
            st_x(st_transform(st_pointn(segment.geom, 2), 4326)),
            st_y(st_transform(st_pointn(segment.geom, 2), 4326))
        ), 4326)) FROM srtm_elevation 
         WHERE ST_Intersects(rast, ST_SetSRID(ST_MakePoint(
            st_x(st_transform(st_pointn(segment.geom, 2), 4326)),
            st_y(st_transform(st_pointn(segment.geom, 2), 4326))
         ), 4326)) LIMIT 1) AS elevation_end
      FROM _all_way_edge awe
        CROSS JOIN LATERAL st_dumpsegments(awe.geom) segment(path, geom)
        LEFT JOIN city c ON st_within(segment.geom, c.geom)
      WHERE awe.nodes[segment.path[1] + 1] IS NOT NULL;
    
    -- Recreate indexes
    CREATE UNIQUE INDEX edge_id_idx ON edge(id);
    CREATE INDEX edge_source_idx ON edge(source);
    CREATE INDEX edge_target_idx ON edge(target);
    CREATE INDEX edge_geom_idx ON edge USING GIST(geom);
    CREATE INDEX edge_way_id_idx ON edge(way_id);
    CREATE INDEX edge_city_name_idx ON edge(city_name);
    CREATE INDEX edge_elevation_idx ON edge(elevation_start, elevation_end)
      WHERE elevation_start IS NOT NULL AND elevation_end IS NOT NULL;
    
    RAISE NOTICE 'Edge refresh completed with elevation data.';
END;
$$ LANGUAGE plpgsql;
