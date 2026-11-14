-- Notes ActivityPub publiées lorsqu'une ou plusieurs sources de travaux
-- routiers n'ont pas pu être importées. Le contenu est conservé pour que la
-- Note reste consultable à son URL /ap/import-error/{id}.
CREATE TABLE IF NOT EXISTS ap_import_error (
    id serial PRIMARY KEY,
    failures text NOT NULL,
    content text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS ap_import_error_created_at_idx
    ON ap_import_error (created_at DESC);
