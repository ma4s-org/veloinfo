ALTER TABLE public.road_work
    ALTER COLUMN geom TYPE geometry(geometry, 3857)
    USING geom;