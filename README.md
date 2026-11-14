# install
Do 'docker-compose up'

When everything is there connect to the dev container. 
with VSCode install the remote development plugin.

Do 'Attach to running container'

# Import the data
run import.sh

when it is done reload the martin server: 
docker-compose restart martin

look at the map from localhost:3000

# Travaux routiers

Les travaux en cours sont importés au démarrage de l'application puis chaque
nuit à 1 h (heure de Montréal), et recalés sur le réseau routier
OpenStreetMap. Seuls les travaux actifs sont conservés : les entraves
planifiées ou à venir sont ignorées.

| Source | Données | Remarque |
| --- | --- | --- |
| Montréal | WFS `entraves-polygonales` | Une seule requête GeoJSON remplace l'ancien parcours de tuiles vectorielles. Les zones sont découpées sur les rues et les pistes cyclables OSM (`transportation` + `all_way`) pour un affichage en lignes ; les zones hors rue restent des polygones. |
| Westmount | `InfoTravauxPublic*`, état « en cours » | |
| Québec | Flux WZDX `event_status = active` | |
| Laval | Service ArcGIS `ing/Obstruction_14_jours` | Couches « rues barrées » et « entrave partielle » ; l'opération `identify` est utilisée car `query` ne renvoie pas les géométries. Données de la Ville de Laval (CC-BY 4.0). |
| Lévis | Service ArcGIS `TravauxInfra` | Couches « pavage » et « infrastructure linéaire en cours » ; les travaux ponctuels ne sont pas encore importés. Données de la Ville de Lévis. |
| Dorval | ArcGIS `Entraves2410_Vue` | Entraves ponctuelles actives. |
| Hampstead | ArcGIS `Reseau_Infotravaux_2023_Hampstead_L` | Tronçons de rue avec `Travaux <> 0`. |
| Mont-Royal | API publique Opatech `public/get_projects` | Tronçons des projets en cours. |
| Dollard-des-Ormeaux | Variable `actMapData` de la carte interactive | Points des travaux `en-cours`, hors parcs. |
| Gatineau | API publique Opatech `public/get_projects` | Tronçons des projets en cours. |
| Sherbrooke | ArcGIS `Travaux/FeatureServer/0` | Travaux en cours. |
| Longueuil | ArcGIS `Gestion_des_entraves_Diffusion/FeatureServer/1` | Zones d'entraves découpées sur le réseau OSM. |
| Trois-Rivières | GeoJSON « Avis de grands travaux » | Zones découpées sur le réseau OSM. |
| Saguenay | GeoJSON « Chantiers - 511 » | Tronçons en cours. |
| Repentigny | API Open511 `/api/events/` | Événements actifs. |
| Drummondville | ArcGIS `Travaux_2023/FeatureServer/1` | Tronçons avec `EtatTravaux = 1`. |

Les autres municipalités de l'île (Baie-D'Urfé, Beaconsfield, Côte-Saint-Luc,
Kirkland, Montréal-Est, Montréal-Ouest, Pointe-Claire,
Sainte-Anne-de-Bellevue, Senneville et L'Île-Dorval) ne publient pas de
géométrie exploitable pour l'instant ; elles restent à intégrer si une source
machine est publiée.

La couche des travaux n'est affichée qu'à partir du zoom 11 pour éviter de
surcharger la carte de loin.

En cas d'échec d'import d'une source, une Note est publiée sur le Fédivers
(au plus une fois par six heures pour la même liste de sources). Elle reste
consultable à `/ap/import-error/{id}`.

L'import peut être lancé manuellement :

```sh
cargo run -- --fetch-road-work [montreal|westmount|quebec|laval|levis|dorval|hampstead|montroyal|ddo|gatineau|sherbrooke|longueuil|trois-rivieres|saguenay|repentigny|drummondville]
```


# Fédivers (ActivityPub)

Vélo info expose un compte ActivityPub qui rend les commentaires disponibles
sur le Fédivers (Mastodon, GoToSocial, Pleroma, etc.).

* Compte principal : `@veloinfo@<domaine>` (en production : `@veloinfo@veloinfo.quebec`)
* Compte d'alertes : `@veloinfo-alertes@<domaine>` (nom configurable)
* WebFinger : `/.well-known/webfinger` (les deux comptes)
* Acteur principal : `/ap/actor`
* Acteur d'alertes : `/ap/alert-actor`
* Boîte de réception : `/ap/actor/inbox` et `/ap/inbox` (shared inbox)
* Boîte de réception des alertes : `/ap/alert-actor/inbox`
* Notes : `/ap/comment/{id}`
* Notes d'erreur d'import : `/ap/import-error/{id}`
* Vignette cartographique : `/report/{id}/map.png`
* Fil d'une contribution : `/ap/report/{id}`
* Outbox : `/ap/actor/outbox`
* Outbox des alertes : `/ap/alert-actor/outbox`
* Page d'explication : `/fediverse`

Chaque commentaire local est publié comme une `Note` ActivityPub. Les abonnés
reçoivent les nouveaux commentaires, et les réponses envoyées depuis le
Fédivers sont enregistrées dans `report_comment` puis affichées comme les
autres réponses. Les réponses locales à un message distant sont également
livrées à l'auteur de ce message.

Les requêtes GET sortantes vers les autres instances sont signées (HTTP
Signatures), ce qui est indispensable pour les instances en mode
`authorized fetch`, dont `mastodon.social`. Lors d'un nouvel abonnement, les
20 derniers commentaires sont aussi renvoyés à l'abonné pour amorcer son fil.

Chaque Note joint une vignette de la carte (`/report/{id}/map.png`) générée
à partir du réseau de rues et de la géométrie de la contribution, ainsi que
la photo du commentaire lorsqu'il y en a une. La vue couvre 400 mètres de
large, et s'élargit seulement si la contribution est plus grande. L'attribution
OpenStreetMap est incluse dans le texte du message.

La vignette est capturée par **Chromium headless** à partir d'une page
MapLibre dédiée (`/report/{id}/map-render`), ce qui donne le rendu détaillé
de la carte (rues, pistes cyclables, bâtiments, hydrographie, noms de lieux).
Un rendu Rust simplifié sert de repli si Chromium n'est pas disponible.
L'image générée est mise en cache dans `MAP_CACHE_DIR` pour éviter de
relancer Chromium à chaque requête.

## Configuration

Les variables suivantes sont optionnelles :

| Variable | Défaut | Description |
| --- | --- | --- |
| `VELOINFO_URL` | `http://localhost:3000` | URL publique du site. Elle doit être en HTTPS en production. |
| `FEDIVERSE_USERNAME` | `veloinfo` | Nom du compte (`acct:nom@domaine`). |
| `FEDIVERSE_NAME` | `Vélo info` | Nom affiché de l'acteur. |
| `FEDIVERSE_SUMMARY` | présentation par défaut | Biographie de l'acteur. |
| `FEDIVERSE_ALERT_USERNAME` | `veloinfo-alertes` | Nom du compte séparé pour les alertes et erreurs d'import. |
| `FEDIVERSE_ALERT_NAME` | `Alertes Vélo info` | Nom affiché du compte d'alertes. |
| `FEDIVERSE_ALERT_SUMMARY` | présentation par défaut | Biographie du compte d'alertes. |
| `FEDIVERSE_VERIFY_SIGNATURES` | `true` | Vérifie les signatures HTTP des activités entrantes. À ne désactiver qu'en développement local. |
| `FEDIVERSE_RENDER_URL` | `http://127.0.0.1:3000` | URL interne utilisée par Chromium pour charger la page de rendu de la carte. |
| `CHROMIUM_BIN` | détection automatique | Chemin explicite vers le binaire Chromium/Chrome. |
| `MAP_CACHE_DIR` | `$TMPDIR/veloinfo-maps` | Répertoire de cache des vignettes générées. |

La clé RSA du compte principal est générée au premier démarrage et conservée
dans `ap_keys`. Le compte d'alertes a sa propre clé (`ap_alert_keys`) et ses
propres abonnés (`ap_alert_followers`). Les acteurs distants sont dans
`ap_remote_actors` et les livraisons en attente dans `ap_delivery_queue`
(avec relances automatiques et signature par la clé du compte concerné).

En production, le site doit être joignable publiquement en HTTPS et le
reverse proxy doit transmettre `/.well-known/webfinger` ainsi que `/ap/*` à
l'application sans réécrire le chemin ni l'en-tête `Host` : les signatures
HTTP des activités entrantes en dépendent.

Attention : `veloinfo.ca` redirige actuellement vers `veloinfo.quebec` pour
la page d'accueil, mais pas pour `/.well-known/webfinger`. Le compte doit
donc être recherché sous `@veloinfo@veloinfo.quebec`. Pour que l'adresse en
`.ca` fonctionne aussi dans la recherche du Fédivers, il faut rediriger
`https://veloinfo.ca/.well-known/webfinger` vers
`https://veloinfo.quebec/.well-known/webfinger` (et idéalement `/ap/*`) au
niveau du proxy ou de Cloudflare.

# License
This project is licensed under either of

Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option