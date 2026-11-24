-- Distingue les sessions collectées par le nouveau suivi (`web`) de celles
-- importées depuis l'ancienne base Matomo (`matomo`).
--
-- `legacy_ref` conserve l'identifiant `idvisit` de Matomo et permet de rendre
-- l'import idempotent (la même visite ne sera pas insérée deux fois).

alter table usage_session
    add column source text not null default 'web',
    add column legacy_ref text;

create unique index usage_session_legacy_ref_idx
    on usage_session (source, legacy_ref)
    where legacy_ref is not null;
