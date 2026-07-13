// SW veloinfo: Cache offline-first pour tuiles Martin + ressources statiques
// Intercepte /martin/*, /report/*, /bike_path, et ressources statiques
// Retourne depuis cache immédiatement, refresh en background, 204 en cas d'erreur

const CACHE_NAME = 'veloinfo-v3';
const STATIC_CACHE = 'veloinfo-static-v3';

// Ressources à precacher à l'installation
const PRECACHE_ASSETS = [
    '/',
    '/index.html',
    '/custom-elements/vi-main.js',
    '/pub/veloinfo.css',
    '/node_modules/maplibre-gl/dist/maplibre-gl.js',
    '/node_modules/maplibre-gl/dist/maplibre-gl.css',
    '/pub/webmanifest.json',
    '/style.json'
];

self.addEventListener('install', (event) => {
    self.skipWaiting();
    // Precache des ressources statiques critiques
    event.waitUntil(
        caches.open(STATIC_CACHE).then((cache) => {
            return cache.addAll(PRECACHE_ASSETS).catch(err => {
                // Certains assets peuvent échouer (ex: node_modules manquant en prod)
                console.warn('Precache failed (some assets may be unavailable):', err);
            });
        })
    );
});

self.addEventListener('activate', (event) => {
    // Nettoyer les anciens caches
    event.waitUntil(
        caches.keys().then(keys => Promise.all(
            keys.filter(k => k !== CACHE_NAME && k !== STATIC_CACHE)
                .map(k => caches.delete(k))
        )).then(() => self.clients.claim())
    );
});

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    
    // Seulement GET et même origine
    if (url.origin !== location.origin || 
        event.request.method !== 'GET') {
        return;
    }
    
    // Ressources statiques: network-first avec fallback cache (offline support)
    const isStatic = PRECACHE_ASSETS.some(asset => 
        url.pathname === asset || url.pathname.startsWith('/custom-elements/') ||
        url.pathname.startsWith('/pub/') || url.pathname.startsWith('/node_modules/')
    );
    
    if (isStatic) {
        event.respondWith(
            caches.open(STATIC_CACHE).then(async (cache) => {
                try {
                    // Network first: toujours récupérer la dernière version
                    const response = await fetch(event.request);
                    if (response.ok) {
                        cache.put(event.request, response.clone());
                    }
                    return response;
                } catch (err) {
                    // Hors ligne: fallback sur le cache
                    const cached = await cache.match(event.request);
                    if (cached) return cached;
                    return new Response(null, { status: 404, statusText: 'Not Found' });
                }
            })
        );
        return;
    }
    
    // Tout le reste: réseau direct (API, etc.)
});
