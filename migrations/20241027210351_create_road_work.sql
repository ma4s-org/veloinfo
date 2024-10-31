CREATE TABLE public.road_work (
    id serial PRIMARY KEY,
    geom public.geometry(polygon, 3857),
    start_date date,
    end_date date
);

CREATE INDEX road_work_geom_idx ON public.road_work using gist(geom);