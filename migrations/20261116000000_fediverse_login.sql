-- Connexion des utilisateurs avec leur propre compte Fédivers.
--
-- Vélo info utilise l'API Mastodon-compatible (OAuth) de l'instance de
-- l'utilisateur pour publier ses signalements sur son compte. Les
-- identifiants d'application sont mémorisés par instance, et le jeton
-- d'accès est conservé pour chaque utilisateur local.

-- Applications OAuth enregistrées sur les instances distantes.
CREATE TABLE IF NOT EXISTS fediverse_apps (
    instance TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Compte Fédivers relié à un utilisateur local (cookie uuid ou Keycloak).
CREATE TABLE IF NOT EXISTS fediverse_accounts (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    instance TEXT NOT NULL,
    account_id TEXT NOT NULL,
    acct TEXT NOT NULL,
    username TEXT,
    display_name TEXT,
    avatar_url TEXT,
    profile_url TEXT,
    access_token TEXT NOT NULL,
    scope TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Un même compte distant ne peut être relié qu'à un seul utilisateur local.
CREATE UNIQUE INDEX IF NOT EXISTS idx_fediverse_accounts_remote
    ON fediverse_accounts (instance, account_id);

CREATE INDEX IF NOT EXISTS idx_fediverse_accounts_acct
    ON fediverse_accounts (acct);

-- Journal des publications de signalements sur les comptes personnels.
-- La contrainte d'unicité rend la publication idempotente.
CREATE TABLE IF NOT EXISTS fediverse_publications (
    id BIGSERIAL PRIMARY KEY,
    report_id INTEGER NOT NULL REFERENCES report(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    remote_status_id TEXT,
    remote_status_url TEXT,
    state TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (report_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_fediverse_publications_state
    ON fediverse_publications (state, updated_at);
