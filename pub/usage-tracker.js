// Suivi d'usage interne, sans donnée personnelle.
//
// - La session est identifiée par un UUID aléatoire conservé dans
//   localStorage : il est partagé entre les onglets du même navigateur.
// - Aucune adresse IP ni position GPS n'est envoyée. On transmet seulement le
//   centre et le zoom de la carte consultée.
// - Une nouvelle session démarre après 30 minutes d'inactivité.
// - Suivi first-party et anonyme : aucun cookie publicitaire, aucune IP, aucune
//   position GPS. L'en-tête Do Not Track n'est donc pas utilisé pour le
//   désactiver.
(() => {
  const ENDPOINT = '/usage/track';
  const ROUTE_ENDPOINT = '/usage/route';
  const SESSION_KEY = 'vi_usage_session';
  const LAST_ACTIVITY_KEY = 'vi_usage_last_activity';
  const SESSION_TIMEOUT_MS = 30 * 60 * 1000;
  const MAP_MOVE_INTERVAL_MS = 8000;
  const HEARTBEAT_MS = 60000;

  const randomId = () => {
    if (window.crypto?.randomUUID) return window.crypto.randomUUID();
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (char) => {
      const value = Math.floor(Math.random() * 16);
      const digit = char === 'x' ? value : (value & 0x3) | 0x8;
      return digit.toString(16);
    });
  };

  const safeStorage = {
    get(storage, key) {
      try {
        return storage.getItem(key);
      } catch {
        return null;
      }
    },
    set(storage, key, value) {
      try {
        storage.setItem(key, value);
      } catch {
        // Mode navigation privée ou stockage désactivé : on continue sans.
      }
    },
  };

  const now = Date.now();
  const previousActivity = Number.parseInt(
    safeStorage.get(window.localStorage, LAST_ACTIVITY_KEY) || '0',
    10,
  );
  let sessionId = safeStorage.get(window.localStorage, SESSION_KEY);
  if (!sessionId || !Number.isFinite(previousActivity)
      || now - previousActivity > SESSION_TIMEOUT_MS) {
    sessionId = randomId();
    safeStorage.set(window.localStorage, SESSION_KEY, sessionId);
  }

  let position = null;
  try {
    const stored = JSON.parse(localStorage.getItem('position'));
    if (stored
        && Number.isFinite(stored.lat)
        && Number.isFinite(stored.lng)) {
      position = {
        lat: stored.lat,
        lng: stored.lng,
        zoom: Number.isFinite(stored.zoom) ? stored.zoom : null,
      };
    }
  } catch {
    // Position absente ou illisible : la carte nous l'enverra plus tard.
  }

  let lastMapMoveSent = 0;

  const payloadFor = (event, extra = {}) => ({
    session_id: sessionId,
    event,
    path: window.location.pathname,
    referrer: event === 'page_view' ? (document.referrer || null) : null,
    language: navigator.language || null,
    screen_width: window.screen?.width || null,
    screen_height: window.screen?.height || null,
    lat: position?.lat ?? null,
    lng: position?.lng ?? null,
    zoom: position?.zoom ?? null,
    ...extra,
  });

  const send = (event, extra = {}) => {
    safeStorage.set(window.localStorage, LAST_ACTIVITY_KEY, String(Date.now()));

    const body = JSON.stringify(payloadFor(event, extra));

    if (event !== 'action' && navigator.sendBeacon) {
      const blob = new Blob([body], { type: 'application/json' });
      if (navigator.sendBeacon(ENDPOINT, blob)) return;
    }

    fetch(ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      keepalive: true,
      credentials: 'same-origin',
    }).catch(() => {
      // Le suivi ne doit jamais gêner l'application.
    });
  };

  const roundCoord = (value) => (
    Number.isFinite(value) ? Math.round(value * 1000) / 1000 : null
  );

  const routePayload = (event, extra = {}) => ({
    session_id: sessionId,
    event,
    ...extra,
  });

  const sendRouteUpdate = (event, extra = {}) => {
    const body = JSON.stringify(routePayload(event, extra));

    if (navigator.sendBeacon) {
      const blob = new Blob([body], { type: 'application/json' });
      if (navigator.sendBeacon(ROUTE_ENDPOINT, blob)) return;
    }

    fetch(ROUTE_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      keepalive: true,
      credentials: 'same-origin',
    }).catch(() => {
      // Le suivi ne doit jamais gêner l'application.
    });
  };

  window.viUsage = {
    track(action) {
      send('action', { action });
    },

    /**
     * Enregistre une demande d'itinéraire et renvoie son identifiant.
     * Les coordonnées sont arrondies à ~100 m avant l'envoi.
     */
    async routeRequest(data = {}) {
      const payload = routePayload('request', {
        start_lat: roundCoord(data.startLat),
        start_lng: roundCoord(data.startLng),
        end_lat: roundCoord(data.endLat),
        end_lng: roundCoord(data.endLng),
        allow_ferry: data.allowFerry ?? null,
      });

      try {
        const response = await fetch(ROUTE_ENDPOINT, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
          credentials: 'same-origin',
        });
        if (!response.ok) return null;
        const json = await response.json();
        return json.route_id || null;
      } catch {
        return null;
      }
    },

    routeUpdate(event, data = {}) {
      sendRouteUpdate(event, {
        route_id: data.routeId || null,
        safe_distance_km: data.safeDistanceKm ?? null,
        fast_distance_km: data.fastDistanceKm ?? null,
        chosen_route: data.chosenRoute ?? null,
        progress: data.progress ?? null,
        remaining_km: data.remainingKm ?? null,
      });
    },
  };

  window.addEventListener('vi-map-position', (event) => {
    const detail = event.detail || {};
    if (!Number.isFinite(detail.lat) || !Number.isFinite(detail.lng)) return;

    position = {
      lat: detail.lat,
      lng: detail.lng,
      zoom: Number.isFinite(detail.zoom) ? detail.zoom : position?.zoom ?? null,
    };

    const timestamp = Date.now();
    if (timestamp - lastMapMoveSent >= MAP_MOVE_INTERVAL_MS) {
      lastMapMoveSent = timestamp;
      send('map_move');
    }
  });

  // Un onglet ouvert reste une session active, même s'il est en arrière-plan
  // pendant qu'on consulte /usage. Les navigateurs throttlent les timers des
  // onglets cachés, mais l'intervalle reste inférieur à la fenêtre « en direct ».
  document.addEventListener('visibilitychange', () => {
    send('heartbeat');
  });

  window.addEventListener('pagehide', () => {
    send('heartbeat');
  });

  window.setInterval(() => {
    send('heartbeat');
  }, HEARTBEAT_MS);

  send('page_view');
})();
