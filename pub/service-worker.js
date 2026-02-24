const CACHE_VERSION = 'v1';
const DATA_CACHE = `data-cache-${CACHE_VERSION}`;
const STATIC_CACHE = `static-cache-${CACHE_VERSION}`;
const KNOWN_CACHES = [DATA_CACHE, STATIC_CACHE];
const ONE_WEEK = 7 * 24 * 60 * 60 * 1000;
const dataPattern = /\/\d+\/\d+\/\d+$/;

self.addEventListener('install', function(event) {
    self.skipWaiting();
});

self.addEventListener('activate', function(event) {
    event.waitUntil(
        caches.keys().then(cacheNames =>
            Promise.all(
                cacheNames
                    .filter(name => !KNOWN_CACHES.includes(name))
                    .map(name => caches.delete(name))
            )
        ).then(() => self.clients.claim())
    );
});


self.addEventListener('fetch', function(event) {
    const url = new URL(event.request.url);

    if (dataPattern.test(url.pathname)) {
        event.respondWith(
            (async () => {
                const cache = await caches.open(DATA_CACHE);
                const cachedResponse = await cache.match(event.request);
                const now = Date.now();

                // 1. Stratégie Cache-First avec vérification de date
                if (cachedResponse) {
                    const cachedDate = parseInt(cachedResponse.headers.get('sw-cache-date'));
                    if (cachedDate && (now - cachedDate < ONE_WEEK)) {
                        return cachedResponse;
                    }
                }

                // 2. Tentative réseau sécurisée
                try {
                    const networkResponse = await fetch(event.request);
                    
                    // On ne met en cache QUE si la réponse est valide
                    if (networkResponse && networkResponse.ok) {
                        // On utilise updateCache mais on s'assure qu'il ne bloque pas
                        // en retournant la réponse originale immédiatement
                        updateCache(cache, event.request, networkResponse.clone(), now)
                            .catch(err => console.warn("Échec mise en cache:", err));
                        return networkResponse;
                    }
                    
                    return networkResponse;
                } catch (err) {
                    // 3. Fallback ultime : si le réseau lâche (ou reload brutal), 
                    // on rend le vieux cache s'il existe, sinon on laisse l'erreur
                    console.error("Erreur Fetch SW:", err);
                    return cachedResponse || Response.error(); 
                }
            })()
        );
        return;
    }

    // Le reste de ton code (statiques, etc.)
});
// Fonction utilitaire pour cloner et dater la mise en cache
async function updateCache(cache, request, response, timestamp) {
    if (!response || !response.ok) {
        return response; 
    }

    const headers = new Headers(response.headers);
    headers.append('sw-cache-date', timestamp.toString());

    const body = await response.clone().blob();
    await cache.put(request, new Response(body, {
        status: response.status,
        statusText: response.statusText,
        headers: headers
    }));

    return response;
}
