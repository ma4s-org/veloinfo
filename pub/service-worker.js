// SW veloinfo: Proxy transparent (pas de cache)
// Intercepte les requêtes /martin/* et les forward directement au réseau
// Utile pour: monitoring, analytics, offline detection, futures features

self.addEventListener('install', (event) => {
    self.skipWaiting();
});

self.addEventListener('activate', (event) => {
    event.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);
    
    // Intercepte SEULEMENT les tuiles Martin en GET
    if (url.origin !== location.origin || 
        !url.pathname.startsWith('/martin/') || 
        event.request.method !== 'GET') {
        return;
    }
    
    // Forward direct au réseau - aucun cache, aucune modification
    event.respondWith(fetch(event.request));
});
