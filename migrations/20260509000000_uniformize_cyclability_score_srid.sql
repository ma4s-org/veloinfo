-- Uniformiser les géométries en SRID 4326 (standard PostGIS)
-- Les segments 1-2 ont été sauvegardés avec SRID 3857 mais coordonnées 4326 (mal étiquetés)
-- Les segments 3+ ont été sauvegardés correctement en SRID 4326

-- Corriger les segments mal étiquetés (SRID 3857 mais coordonnées 4326)
-- On détecte ceux où les X sont dans la plage -180 à 180 (donc 4326, pas 3857)
-- Utilise ST_StartPoint() car geom est un LINESTRING, pas un POINT
UPDATE cyclability_score 
SET geom = ST_SetSRID(geom, 4326)
WHERE ST_SRID(geom) = 3857 
  AND ST_X(ST_StartPoint(geom)) BETWEEN -180 AND 180;

-- Créer un index GIST sur geom (déjà existant, mais on s'assure qu'il est à jour)
DROP INDEX IF EXISTS idx_cyclability_score_geom;
CREATE INDEX idx_cyclability_score_geom ON cyclability_score USING GIST (geom);

-- Vérification : toutes les géométries devraient être en 4326 maintenant
-- SELECT id, ST_SRID(geom) as srid FROM cyclability_score;
