#!/usr/bin/env bash
# Importe les sessions historiques de Matomo (MariaDB) dans les tables
# `usage_session` / `usage_event` de Vélo info.
#
# Le script lit les visites Matomo (matomo_log_visit), pas les adresses IP ni
# les identifiants visiteurs. La position importée est celle que Matomo a
# déduite de l'IP (ville approximative) ; le zoom est inconnu.
#
# Variables :
#   MATOMO_DB_HOST        défaut : 127.0.0.1
#   MATOMO_DB_PORT        défaut : 3306
#   MATOMO_DB_NAME        défaut : matomo
#   MATOMO_DB_USER        défaut : matomo
#   MATOMO_DB_PASSWORD    défaut : matomo
#   MATOMO_TABLE_PREFIX   défaut : matomo_
#   MATOMO_SINCE          optionnel, date YYYY-MM-DD (début de l'import)
#   MATOMO_EXPORT_FILE    optionnel, importe un TSV déjà exporté (pas de MySQL)
#   VELOINFO_BIN          optionnel, binaire veloinfo déjà compilé
#
# Exemples :
#   ./import_matomo.sh
#   MATOMO_SINCE=2024-01-01 ./import_matomo.sh
#   MATOMO_EXPORT_FILE=sessions.tsv ./import_matomo.sh
#
# Matomo ne fait plus partie de la stack Docker. Le script s'adresse donc à une
# base MariaDB/MySQL externe via le client `mysql`, ou à un export TSV existant.
set -euo pipefail

MATOMO_DB_HOST="${MATOMO_DB_HOST:-127.0.0.1}"
MATOMO_DB_PORT="${MATOMO_DB_PORT:-3306}"
MATOMO_DB_NAME="${MATOMO_DB_NAME:-matomo}"
MATOMO_DB_USER="${MATOMO_DB_USER:-matomo}"
MATOMO_DB_PASSWORD="${MATOMO_DB_PASSWORD:-matomo}"
MATOMO_TABLE_PREFIX="${MATOMO_TABLE_PREFIX:-matomo_}"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

tsv="${MATOMO_EXPORT_FILE:-$tmp_dir/sessions.tsv}"

if [[ -z "${MATOMO_EXPORT_FILE:-}" ]]; then
    since_filter=""
    if [[ -n "${MATOMO_SINCE:-}" ]]; then
        if [[ ! "$MATOMO_SINCE" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]]; then
            echo "MATOMO_SINCE doit être au format YYYY-MM-DD." >&2
            exit 1
        fi
        since_filter="where v.visit_first_action_time >= '${MATOMO_SINCE} 00:00:00'"
    fi

    prefix="$MATOMO_TABLE_PREFIX"
    query="
select
    v.idvisit,
    date_format(v.visit_first_action_time, '%Y-%m-%d %H:%i:%s'),
    date_format(v.visit_last_action_time, '%Y-%m-%d %H:%i:%s'),
    coalesce(cast(v.location_latitude as char), ''),
    coalesce(cast(v.location_longitude as char), ''),
    coalesce(v.visit_total_actions, 0),
    coalesce(v.visit_total_time, 0),
    replace(replace(replace(coalesce(v.config_os, ''), char(9), ' '), char(10), ' '), char(13), ' '),
    replace(replace(replace(coalesce(v.config_browser_name, ''), char(9), ' '), char(10), ' '), char(13), ' '),
    coalesce(v.config_device_type, 0),
    replace(replace(replace(coalesce(v.location_browser_lang, ''), char(9), ' '), char(10), ' '), char(13), ' '),
    coalesce(cast(substring_index(v.config_resolution, 'x', 1) as unsigned), 0),
    coalesce(cast(substring_index(v.config_resolution, 'x', -1) as unsigned), 0),
    replace(replace(replace(coalesce(v.referer_name, ''), char(9), ' '), char(10), ' '), char(13), ' '),
    replace(replace(replace(coalesce(la.name, ''), char(9), ' '), char(10), ' '), char(13), ' '),
    (
        select count(*)
        from ${prefix}log_link_visit_action lva
        where lva.idvisit = v.idvisit
          and lva.idaction_url is not null
    )
from ${prefix}log_visit v
left join ${prefix}log_action la on la.idaction = v.visit_exit_idaction_url
${since_filter}
order by v.idvisit
"

    echo "Export des sessions Matomo vers $tsv ..." >&2
    if command -v mysql >/dev/null 2>&1; then
        MYSQL_PWD="$MATOMO_DB_PASSWORD" mysql \
            --host="$MATOMO_DB_HOST" \
            --port="$MATOMO_DB_PORT" \
            --user="$MATOMO_DB_USER" \
            --database="$MATOMO_DB_NAME" \
            --batch \
            --raw \
            --skip-column-names \
            --default-character-set=utf8mb4 \
            --execute="$query" > "$tsv"
    else
        echo "Le client 'mysql' est introuvable. Exportez le TSV puis utilisez MATOMO_EXPORT_FILE." >&2
        exit 1
    fi
fi

if [[ ! -s "$tsv" ]]; then
    echo "Le fichier d'export est vide : $tsv" >&2
    exit 1
fi

echo "Import dans Veloinfo ..." >&2
if [[ -n "${VELOINFO_BIN:-}" ]]; then
    "$VELOINFO_BIN" --import-matomo "$tsv"
else
    cargo run -- --import-matomo "$tsv"
fi
