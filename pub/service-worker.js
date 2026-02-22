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

    // SCÉNARIO 1 : Les tuiles (Cache-First, TTL 1 semaine) — toutes origines
    if (dataPattern.test(url.pathname)) {
        event.respondWith(
            caches.open(DATA_CACHE).then(cache =>
                cache.match(event.request).then(function(cachedResponse) {
                    const now = Date.now();

                    if (cachedResponse) {
                        const cachedDate = parseInt(cachedResponse.headers.get('sw-cache-date'));
                        if (cachedDate && (now - cachedDate < ONE_WEEK)) {
                            return cachedResponse;
                        }
                    }

                    return fetch(event.request)
                        .then(networkResponse => updateCache(cache, event.request, networkResponse, now))
                        .catch(() => cachedResponse || Promise.reject('Hors-ligne et pas de cache'));
                })
            )
        );
        return;
    }

    // Ignorer les requêtes cross-origin pour le reste
    if (url.origin !== self.location.origin) return;

    // SCÉNARIO 2 : Tout le reste (Network-First avec mise en cache et fallback)
    event.respondWith(
        fetch(event.request)
            .then(function(networkResponse) {
                if (networkResponse.ok) {
                    const responseClone = networkResponse.clone();
                    caches.open(STATIC_CACHE).then(cache => cache.put(event.request, responseClone));
                }
                return networkResponse;
            })
            .catch(function() {
                return caches.match(event.request);
            })
    );
});

// Fonction utilitaire pour cloner et dater la mise en cache
async function updateCache(cache, request, response, timestamp) {
    const headers = new Headers(response.headers);
    headers.append('sw-cache-date', timestamp.toString());
    headers.set('Access-Control-Allow-Origin', '*');

    const body = await response.clone().blob();
    await cache.put(request, new Response(body, {
        status: response.status,
        statusText: response.statusText,
        headers: headers
    }));

    return response;
}
