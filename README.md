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


# Fédivers (ActivityPub)

Vélo info expose un compte ActivityPub qui rend les commentaires disponibles
sur le Fédivers (Mastodon, GoToSocial, Pleroma, etc.).

* Compte : `@veloinfo@<domaine>` (en production : `@veloinfo@veloinfo.quebec`)
* WebFinger : `/.well-known/webfinger`
* Acteur : `/ap/actor`
* Boîte de réception : `/ap/actor/inbox` et `/ap/inbox` (shared inbox)
* Notes : `/ap/comment/{id}`
* Vignette cartographique : `/report/{id}/map.png`
* Fil d'une contribution : `/ap/report/{id}`
* Outbox : `/ap/actor/outbox`
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
| `FEDIVERSE_VERIFY_SIGNATURES` | `true` | Vérifie les signatures HTTP des activités entrantes. À ne désactiver qu'en développement local. |
| `FEDIVERSE_RENDER_URL` | `http://127.0.0.1:3000` | URL interne utilisée par Chromium pour charger la page de rendu de la carte. |
| `CHROMIUM_BIN` | détection automatique | Chemin explicite vers le binaire Chromium/Chrome. |
| `MAP_CACHE_DIR` | `$TMPDIR/veloinfo-maps` | Répertoire de cache des vignettes générées. |

La clé RSA du compte est générée au premier démarrage et conservée dans la
table `ap_keys`. Les abonnés sont stockés dans `ap_followers`, les acteurs
distants dans `ap_remote_actors` et les livraisons en attente dans
`ap_delivery_queue` (avec relances automatiques).

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