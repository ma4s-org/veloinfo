console.log("Enregistrement du service worker");

// Configuration du cache
const CACHE_VERSION = 'v1';
const APP_CACHE = `app-cache-${CACHE_VERSION}`;
const KNOWN_CACHES = [APP_CACHE];

// Constantes
const CACHE_DURATION = 12 * 60 * 60 * 1000; // 12 heures en millisecondes

// Event d'installation : active immédiatement le nouveau SW sans attendre les clients
self.addEventListener('install', function(event) {
    self.skipWaiting();
});

// Event d'activation : nettoie les anciens caches et prend contrôle des clients
self.addEventListener('activate', function(event) {
    event.waitUntil(
        // Récupère tous les noms de caches
        caches.keys().then(cacheNames =>
            // Supprime les caches qui ne sont pas dans KNOWN_CACHES (anciens caches)
            Promise.all(
                cacheNames
                    .filter(name => !KNOWN_CACHES.includes(name))
                    .map(name => caches.delete(name))
            )
        ).then(() => self.clients.claim()) // Prend contrôle de tous les clients
    );
});


// Event de fetch : intercepte toutes les requêtes réseau
self.addEventListener('fetch', function(event) {
    // On ne traite que les requêtes GET (le cache ne supporte pas POST, PUT, etc.)
    if (event.request.method !== 'GET') {
        return;
    }

    event.respondWith(
        (async () => {
            const cache = await caches.open(APP_CACHE);
            const cachedResponse = await cache.match(event.request);
            const now = Date.now();

            // Étape 1 : Vérifier si le cache existe et n'a pas expiré (12 heures)
            if (cachedResponse) {
                const cachedDateStr = cachedResponse.headers.get('sw-cache-date');
                const cachedDate = cachedDateStr ? parseInt(cachedDateStr) : null;
                // Si le cache a un timestamp valide et a moins de 12 heures, le retourner
                if (cachedDate && !isNaN(cachedDate) && (now - cachedDate < CACHE_DURATION)) {
                    return cachedResponse;
                }
            }

            // Étape 2 : Tentative de récupération depuis le réseau
            try {
                const networkResponse = await fetch(event.request);

                // Si la réponse est valide, la mettre en cache
                if (networkResponse && networkResponse.ok) {
                    // Mise en cache asynchrone (ne bloque pas la réponse)
                    updateCache(cache, event.request, networkResponse.clone(), now)
                        .catch(err => console.warn("Échec mise en cache:", err));
                    return networkResponse;
                }

                return networkResponse;
            } catch (err) {
                // Étape 3 : Fallback en cas d'erreur réseau
                // Retourne le cache (même expiré) plutôt qu'une erreur qui avorte la requête
                console.error("Erreur Fetch SW:", err);
                if (cachedResponse) {
                    return cachedResponse;
                }
                // Ne retourner une erreur que si vraiment aucun cache n'existe
                return Response.error();
            }
        })()
    );
});
/**
 * Met en cache une réponse avec timestamp pour suivi d'expiration
 * @param {Cache} cache - Instance du cache
 * @param {Request} request - Requête originale
 * @param {Response} response - Réponse à mettre en cache
 * @param {number} timestamp - Timestamp actuel (Date.now())
 */
async function updateCache(cache, request, response, timestamp) {
    // Ne met en cache que les réponses valides
    if (!response || !response.ok) {
        return response;
    }

    // Ne pas mettre en cache les réponses vides (204, 304, etc.)
    if (response.status === 204 || response.status === 304) {
        return response;
    }

    try {
        // Ajoute un header personnalisé avec la date de mise en cache
        const headers = new Headers(response.headers);
        headers.append('sw-cache-date', timestamp.toString());

        // Clone la réponse en blob pour éviter les consommations multiples
        const body = await response.clone().blob();

        // Enregistre la réponse avec le header de date
        await cache.put(request, new Response(body, {
            status: response.status,
            statusText: response.statusText,
            headers: headers
        }));
    } catch (err) {
        console.warn("Erreur lors de la mise en cache:", err);
    }

    return response;
}
