-- Migration pour supporter les segments personnalisés (cercle/capsule)
-- way_ids est supprimé, geom devient la source de vérité

-- 1. Supprimer l'index sur way_ids s'il existe
DROP INDEX IF EXISTS idx_way_ids;

-- 2. Supprimer la colonne way_ids
ALTER TABLE cyclability_score DROP COLUMN IF EXISTS way_ids;

-- 3. S'assurer que geom est NOT NULL
ALTER TABLE cyclability_score ALTER COLUMN geom SET NOT NULL;

-- 4. Créer l'index GIST sur geom pour les recherches spatiales
CREATE INDEX IF NOT EXISTS idx_cyclability_score_geom ON cyclability_score USING GIST (geom);
