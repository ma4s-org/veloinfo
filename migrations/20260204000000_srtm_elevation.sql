-- Create raster table for SRTM elevation data
CREATE TABLE IF NOT EXISTS srtm_elevation (
    id SERIAL PRIMARY KEY,
    rast RASTER
);

-- Create spatial index on raster
CREATE INDEX IF NOT EXISTS srtm_elevation_rast_idx 
    ON srtm_elevation USING GIST (ST_ConvexHull(rast));

-- Create a function to get elevation at a specific point
CREATE OR REPLACE FUNCTION get_elevation_at_point(
    lon FLOAT8,
    lat FLOAT8
) RETURNS FLOAT8 AS $$
DECLARE
    elevation FLOAT8;
BEGIN
    -- Prefer polygon table if it exists
    IF to_regclass('public.srtm_elevation_polygons') IS NOT NULL THEN
        SELECT elevation INTO elevation
        FROM srtm_elevation_polygons
        WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint(lon, lat), 4326))
        LIMIT 1;
    ELSIF to_regclass('public.srtm_elevation_points') IS NOT NULL THEN
        SELECT elevation INTO elevation
        FROM srtm_elevation_points
        WHERE ST_DWithin(geom, ST_SetSRID(ST_MakePoint(lon, lat), 4326), 0)
        LIMIT 1;
    ELSE
        SELECT ST_Value(rast, ST_SetSRID(ST_MakePoint(lon, lat), 4326)) INTO elevation
        FROM srtm_elevation
        WHERE ST_Intersects(rast, ST_SetSRID(ST_MakePoint(lon, lat), 4326))
        LIMIT 1;
    END IF;

    RETURN elevation;
END;
$$ LANGUAGE plpgsql STABLE;

-- Create a function to get elevation values along a line
CREATE OR REPLACE FUNCTION get_elevation_profile(
    line_geom GEOMETRY
) RETURNS TABLE(distance FLOAT8, elevation FLOAT8) AS $$
DECLARE
    point_geom GEOMETRY;
    total_distance FLOAT8 := 0;
BEGIN
    FOR i IN 1 .. ST_NumPoints(line_geom) LOOP
        point_geom := ST_PointN(line_geom, i);
        
        IF i > 1 THEN
            total_distance := total_distance + ST_Distance(ST_PointN(line_geom, i-1), point_geom);
        END IF;

        IF to_regclass('public.srtm_elevation_polygons') IS NOT NULL THEN
            SELECT total_distance, elevation INTO distance, elevation
            FROM srtm_elevation_polygons
            WHERE ST_Contains(geom, point_geom)
            LIMIT 1;
        ELSIF to_regclass('public.srtm_elevation_points') IS NOT NULL THEN
            SELECT total_distance, elevation INTO distance, elevation
            FROM srtm_elevation_points
            WHERE ST_DWithin(geom, point_geom, 0)
            LIMIT 1;
        ELSE
            SELECT total_distance, ST_Value(rast, point_geom) INTO distance, elevation
            FROM srtm_elevation
            WHERE ST_Intersects(rast, point_geom)
            LIMIT 1;
        END IF;
        
        RETURN NEXT;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- Function to calculate slope between two points
CREATE OR REPLACE FUNCTION calculate_slope(
    ele1 FLOAT8,
    ele2 FLOAT8,
    distance_m FLOAT8
) RETURNS FLOAT8 AS $$
BEGIN
    IF distance_m <= 0 THEN
        RETURN 0;
    END IF;
    RETURN ((ele2 - ele1) / distance_m) * 100; -- percentage
END;
$$ LANGUAGE plpgsql IMMUTABLE;
