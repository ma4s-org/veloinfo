-- Distingue l'origine des travaux (Montréal, Westmount, ...) pour pouvoir
-- rafraîchir une source sans effacer les données des autres.
ALTER TABLE public.road_work ADD COLUMN source text;

-- Les lignes existantes proviennent toutes de l'import de Montréal.
UPDATE public.road_work SET source = 'montreal' WHERE source IS NULL;

ALTER TABLE public.road_work ALTER COLUMN source SET NOT NULL;

CREATE INDEX road_work_source_idx ON public.road_work (source);
