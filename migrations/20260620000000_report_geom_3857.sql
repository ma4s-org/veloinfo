-- Stocker le geom de report en SRID 3857 (Web Mercator) pour correspondre à edge.geom
ALTER TABLE report ALTER COLUMN geom TYPE geometry(Geometry, 3857) USING ST_Transform(geom, 3857);