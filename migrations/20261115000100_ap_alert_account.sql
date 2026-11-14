-- Compte ActivityPub séparé pour les rapports d'import et alertes techniques.
CREATE TABLE IF NOT EXISTS ap_alert_keys (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    private_key_pem TEXT NOT NULL,
    public_key_pem TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS ap_alert_followers (
    actor_id TEXT PRIMARY KEY,
    inbox TEXT NOT NULL,
    shared_inbox TEXT,
    display_name TEXT,
    follow_activity_id TEXT,
    followed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- La file de livraison doit savoir avec quelle clé signer l'activité.
ALTER TABLE ap_delivery_queue
    ADD COLUMN IF NOT EXISTS account TEXT NOT NULL DEFAULT 'main';

CREATE INDEX IF NOT EXISTS idx_ap_delivery_queue_account_due
    ON ap_delivery_queue (account, next_attempt_at);
