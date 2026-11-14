-- ActivityPub / Fédivers
--
-- Permet d'exposer les commentaires de Vélo info comme des messages
-- ActivityPub, de gérer les abonnements (Follow) et de recevoir les
-- réponses envoyées depuis Mastodon ou tout autre logiciel du Fédivers.

-- Clé RSA du compte ActivityPub. Une seule ligne (id = 1).
CREATE TABLE IF NOT EXISTS ap_keys (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    private_key_pem TEXT NOT NULL,
    public_key_pem TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Abonnés du compte @veloinfo@...
CREATE TABLE IF NOT EXISTS ap_followers (
    actor_id TEXT PRIMARY KEY,
    inbox TEXT NOT NULL,
    shared_inbox TEXT,
    display_name TEXT,
    follow_activity_id TEXT,
    followed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE ap_followers
    ADD COLUMN IF NOT EXISTS follow_activity_id TEXT;

-- Cache des acteurs distants (clé publique, inbox, nom).
CREATE TABLE IF NOT EXISTS ap_remote_actors (
    id TEXT PRIMARY KEY,
    inbox TEXT NOT NULL,
    shared_inbox TEXT,
    public_key_id TEXT,
    public_key_pem TEXT NOT NULL,
    display_name TEXT,
    username TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- File d'attente de livraison vers les inbox distantes.
CREATE TABLE IF NOT EXISTS ap_delivery_queue (
    id BIGSERIAL PRIMARY KEY,
    activity_id TEXT NOT NULL,
    inbox TEXT NOT NULL,
    payload TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (activity_id, inbox)
);

CREATE INDEX IF NOT EXISTS idx_ap_delivery_queue_due
    ON ap_delivery_queue (next_attempt_at);

-- Identifiants ActivityPub des commentaires.
-- Les commentaires locaux ont un identifiant calculé à partir de leur id ;
-- les commentaires distants conservent ici l'URL de leur Note d'origine.
ALTER TABLE report_comment
    ADD COLUMN IF NOT EXISTS ap_id TEXT;

ALTER TABLE report_comment
    ADD COLUMN IF NOT EXISTS ap_remote_actor_id TEXT;

ALTER TABLE report_comment
    ADD COLUMN IF NOT EXISTS ap_remote_actor_name TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_report_comment_ap_id
    ON report_comment (ap_id)
    WHERE ap_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_report_comment_remote_actor
    ON report_comment (ap_remote_actor_id)
    WHERE ap_remote_actor_id IS NOT NULL;
