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

En production, le service Martin doit démarrer avec le fichier
`martin/config.yaml` (`--config /config.yaml`). Ce fichier active le cache des
tuiles (une heure) et fixe les bornes de la source `road_work`. Les URL des
tuiles de travaux contiennent une version (`?v=<date>-<max_id>-<count>`) qui
change à chaque import : le navigateur et le CDN renouvellent donc leurs tuiles
automatiquement. Martin 1.6 ne tient pas compte de cette version dans sa
propre clé de cache ; après un import manuel, ses tuiles restent au plus une
heure en mémoire (ou redémarrer Martin pour les voir immédiatement).

En cas d'échec d'import d'une source, une Note est publiée sur le Fédivers
(au plus une fois par six heures pour la même liste de sources). Elle reste
consultable à `/ap/import-error/{id}`.

L'import peut être lancé manuellement :

```sh
cargo run -- --fetch-road-work [montreal|westmount|quebec|laval|levis|dorval|hampstead|montroyal|ddo|gatineau|sherbrooke|longueuil|trois-rivieres|saguenay|repentigny|drummondville]
```


# Statistiques d'usage

Matomo n'est plus nécessaire : le suivi est intégré à l'application et le
tableau de bord est disponible publiquement à `/usage`.

Le navigateur envoie une activité à `/usage/track`. Le suivi reste volontairement
minimal et sans donnée personnelle :

* identifiant de session aléatoire conservé dans `localStorage`, partagé entre
  les onglets du même navigateur ; une nouvelle session démarre après 30
  minutes d'inactivité ;
* centre et zoom de la carte consultée, jamais la position GPS ;
* page visitée, langue, taille d'écran et type d'appareil déduit du
  `User-Agent` (stocké sous forme de libellé, pas en clair).

Ni l'adresse IP, ni un identifiant persistant entre les visites ne sont
enregistrés. Le suivi est first-party et anonyme : il ne dépose pas de cookie
publicitaire et n'utilise pas l'en-tête `Do Not Track`.

Le tableau de bord `/usage` se rafraîchit toutes les 15 secondes. Il affiche la
carte des dernières zones consultées, les sessions en direct (activité dans les
cinq dernières minutes), les pages vues et les actions des 24 heures, un
graphique du trafic par jour, semaine ou mois, ainsi que le détail de chaque
session (durée, appareil, navigateur, langue, zone et référent). Les données sont exposées en JSON à `/usage/sessions` et conservées
dans les tables `usage_session` et `usage_event`.

Les itinéraires demandés sont listés dans le tableau de bord. Au survol d'une
ligne, leur tracé est recalculé à la demande et affiché dans un aperçu
cartographique (`/usage/routes/{id}/geometry`). Ce calcul est mis en cache en
mémoire et ne compte pas comme une nouvelle demande d'itinéraire.

## Importer l'historique Matomo

Les visites déjà enregistrées par Matomo peuvent être importées :

```sh
./import_matomo.sh
```

Matomo ne fait plus partie de la stack Docker. Le script s'adresse à une base
MariaDB/MySQL externe via le client `mysql` (variables `MATOMO_DB_*`, préfixe
`MATOMO_TABLE_PREFIX`, défaut `matomo_`), ou à un export TSV déjà produit avec
`MATOMO_EXPORT_FILE=sessions.tsv`. On peut limiter la période avec
`MATOMO_SINCE=AAAA-MM-JJ`.

Différences avec le suivi intégré :

* la position des sessions importées vient du centre de carte présent dans
  l'URL Matomo (`lat`, `lng`, `zoom`) lorsqu'il existe ; sinon, elle est
  déduite par Matomo à partir de l'IP (ville approximative) et le zoom est
  inconnu. Si aucune des deux n'est disponible, la session est importée sans
  position et n'apparaît pas sur la carte ;
* les pages vues d'une session sont importées sous forme de total, pas
  d'événement individuel ;
* ni l'adresse IP ni l'identifiant visiteur de Matomo ne sont lus.

En production, l'export peut aussi être transmis directement au binaire déjà
compilé :

```sh
docker compose exec -T veloinfo /app/veloinfo --import-matomo - < sessions.tsv
```

Les sessions importées portent `source = 'matomo'` et sont signalées comme
telles dans le détail. L'import est idempotent : relancer le script ne crée pas
de doublon. Les conteneurs Matomo ont été retirés de `docker-compose.yaml` ; les
volumes `matomo` et `matomo-db` peuvent être conservés comme sauvegarde, puis
supprimés manuellement quand ils ne sont plus nécessaires.

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
* Connexion personnelle : formulaire POST `/fediverse/connect`, retour sur `/fediverse/callback`
* État de connexion : `/fediverse/me`, déconnexion : `/fediverse/disconnect`

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

## Connexion avec son propre compte Fédivers

En plus du compte principal, chaque utilisateur peut relier son propre compte
Mastodon, GoToSocial, Pleroma ou Akkoma. Le menu de la carte propose
« Se connecter avec le Fédivers » : l'utilisateur saisit le domaine de son
instance, autorise Vélo info sur celle-ci, puis les nouveaux signalements sont
publiés automatiquement sur son compte (texte, lien, vignette de la carte et
photo éventuelle).

La connexion utilise l'API Mastodon-compatible en OAuth 2.0. Vélo info ne
reçoit jamais le mot de passe : il conserve seulement un jeton d'accès, stocké
dans `fediverse_accounts`. Ce jeton est sensible : l'accès à PostgreSQL doit
être restreint. Les signalements déjà publiés sont suivis dans
`fediverse_publications` ; en cas d'échec réseau, un worker retente la
publication jusqu'à cinq fois. Les commentaires et réponses continuent d'être
fédérés par le compte principal `@veloinfo@<domaine>` afin que les réponses
envoyées depuis le Fédivers restent visibles sur la carte.

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
| `FEDIVERSE_VISIBILITY` | `public` | Visibilité des statuts publiés sur les comptes personnels (`public`, `unlisted`, `private`). |
| `FEDIVERSE_RENDER_URL` | `http://127.0.0.1:3000` | URL interne utilisée par Chromium pour charger la page de rendu de la carte. |
| `CHROMIUM_BIN` | détection automatique | Chemin explicite vers le binaire Chromium/Chrome. |
| `MAP_CACHE_DIR` | `$TMPDIR/veloinfo-maps` | Répertoire de cache des vignettes générées. |

La clé RSA du compte principal est générée au premier démarrage et conservée
dans `ap_keys`. Le compte d'alertes a sa propre clé (`ap_alert_keys`) et ses
propres abonnés (`ap_alert_followers`). Les acteurs distants sont dans
`ap_remote_actors` et les livraisons en attente dans `ap_delivery_queue`
(avec relances automatiques et signature par la clé du compte concerné).
Les applications OAuth enregistrées sur les instances sont dans
`fediverse_apps`, les comptes personnels reliés dans `fediverse_accounts` et
les publications de signalements dans `fediverse_publications`.

En production, le site doit être joignable publiquement en HTTPS et le
reverse proxy doit transmettre `/.well-known/webfinger` ainsi que `/ap/*` et
`/fediverse/*` à l'application sans réécrire le chemin ni l'en-tête `Host` :
les signatures HTTP des activités entrantes en dépendent.

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