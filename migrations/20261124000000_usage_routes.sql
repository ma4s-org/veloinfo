-- Suivi des itinéraires demandés et de leur suivi.
--
-- Les coordonnées sont arrondies côté navigateur (~100 m) avant l'envoi :
-- on garde une zone de départ/arrivée, pas la position GPS exacte.

create table usage_route (
    id uuid primary key,
    session_id uuid not null references usage_session(id) on delete cascade,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    start_lat double precision not null,
    start_lng double precision not null,
    end_lat double precision not null,
    end_lng double precision not null,
    allow_ferry boolean,
    safe_distance_km double precision,
    fast_distance_km double precision,
    chosen_route text,
    status text not null default 'requested',
    progress double precision not null default 0,
    remaining_km double precision,
    followed_at timestamptz,
    completed_at timestamptz,
    last_progress_at timestamptz,
    constraint usage_route_start_lat_range
        check (start_lat between -90 and 90),
    constraint usage_route_start_lng_range
        check (start_lng between -180 and 180),
    constraint usage_route_end_lat_range
        check (end_lat between -90 and 90),
    constraint usage_route_end_lng_range
        check (end_lng between -180 and 180),
    constraint usage_route_progress_range
        check (progress between 0 and 1),
    constraint usage_route_status_values
        check (status in ('requested', 'following', 'completed', 'abandoned')),
    constraint usage_route_chosen_values
        check (chosen_route is null or chosen_route in ('safe', 'fast'))
);

create index usage_route_created_at_idx
    on usage_route (created_at desc);
create index usage_route_session_idx
    on usage_route (session_id);
create index usage_route_status_idx
    on usage_route (status);
