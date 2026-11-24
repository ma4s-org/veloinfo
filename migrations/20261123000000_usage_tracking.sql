-- Statistiques d'usage internes (remplace Matomo).
--
-- L'objectif est de savoir où les gens regardent la carte et pendant combien
-- de temps, sans conserver de donnée personnelle : pas d'adresse IP, pas de
-- position GPS, pas d'identifiant persistant entre les sessions.

create table usage_session (
    id uuid primary key,
    started_at timestamptz not null default now(),
    last_seen timestamptz not null default now(),
    -- Dernière zone consultée sur la carte (centre de la carte, pas l'utilisateur).
    last_lat double precision,
    last_lng double precision,
    last_zoom double precision,
    path text,
    referrer text,
    language text,
    device text,
    browser text,
    os text,
    screen_width integer,
    screen_height integer,
    page_views integer not null default 0,
    events integer not null default 0,
    constraint usage_session_lat_range
        check (last_lat is null or last_lat between -90 and 90),
    constraint usage_session_lng_range
        check (last_lng is null or last_lng between -180 and 180),
    constraint usage_session_zoom_range
        check (last_zoom is null or last_zoom between 0 and 24)
);

create index usage_session_last_seen_idx
    on usage_session (last_seen desc);

create table usage_event (
    id bigserial primary key,
    session_id uuid not null references usage_session(id) on delete cascade,
    created_at timestamptz not null default now(),
    kind text not null,
    action text,
    path text,
    lat double precision,
    lng double precision,
    zoom double precision
);

create index usage_event_session_idx
    on usage_event (session_id, created_at desc);
create index usage_event_created_at_idx
    on usage_event (created_at desc);
create index usage_event_kind_created_at_idx
    on usage_event (kind, created_at desc);
