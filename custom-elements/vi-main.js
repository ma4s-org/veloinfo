import '/custom-elements/vi-follow-panel.js';
import '/custom-elements/vi-route-panel.js';
import RouteSearching from '/custom-elements/vi-route-searching.js';
import '/custom-elements/vi-search-input.js';
import '/custom-elements/vi-menu.js';
import '/custom-elements/vi-install-ios.js';
import '/custom-elements/vi-install-android.js';
import '/custom-elements/vi-mobilizon-events.js';
import '/custom-elements/vi-route-define.js';
import SegmentPanel from '/custom-elements/vi-segment-panel.js';
import PointPanel from '/custom-elements/vi-point-panel.js';
import '/custom-elements/vi-change-start.js';
import '/custom-elements/vi-info.js';
import ViLayers from './vi-layers.js';
import ViInfo from './vi-info.js';
import { registerViMain } from '/custom-elements/vi-context.js';

const html = String.raw;

// Constantes de couleurs
const MARKER_COLORS = {
    START: "#f00",      // Rouge pour le départ d'itinéraire (GPS)
    END: "#00f",        // Bleu pour l'arrivée / point cliqué
    SEGMENT_START: "#ffa500"  // Orange pour le début du segment sélectionné
};

const LAYERS = {
    SAFE: "selected_safe",
    FAST: "selected_fast",
    SELECTED: "selected",
    SEARCHED: "searched_route"
};

// Images de sprite accessibles à la demande
const SPRITE_IMAGES = {
    'bike-parking': '/pub/bicycle-parking.png',
    'drinking-water': '/pub/drinking_water.png',
    'bike-shop': '/pub/bike_shop.png',
    'bicycle_repair_station': '/pub/bicycle_repair_station.png',
    'bixi': '/pub/bixi.png',
    'snow': '/pub/snow-margin.png',
    'oneway': '/pub/oneway.png'
};

class ViMain extends HTMLElement {
    constructor() {
        super();
        this._loadingImages = new Set();
        this._isReporting = false;  // Mode "signaler" activé
        this._reportingSegment = null;  // Segment en cours de signalement
        this._segmentStart = null;  // Point de départ du segment actuel (pour modifier la fin)
        this.soundEnabled = document.cookie.includes('sound_enabled=1');  // Annonces vocales depuis le cookie
        this.innerHTML = html`
            <div id="map">
                <a rel="me" href="https://mastodon.social/@MartinNHamel"></a>
                <vi-search-input id="search"></vi-search-input>
                <div id="info"></div>
                <vi-menu></vi-menu>
                <div id="buttons"
                    style="position: absolute; top:142px; right:6px; padding: 4px; z-index: 10">
                    <div id="layers_button"
                        style="border-radius: 0.375rem; border-width: 1px; border-color: rgb(209 213 219); cursor: pointer;">
                        <img style="width: 29px; height: 29px; background-color: rgb(255 255 255); border-radius: 0.375rem; align-self: center;" src="/pub/layers.png">
                    </div>
                    <div id="snow_button"
                        style="border-radius: 0.375rem; border-width: 1px; border-color: rgb(209 213 219); margin-top: 4px;
                               display: flex; justify-content: center; align-items: center; background-color: white;
                               width: 31px; height: 31px; cursor: pointer;">
                        <img style="width: 24px; height: 24px;" src="/pub/snow.png">
                    </div>
                    <div id="sound_button"
                        style="border-radius: 0.375rem; border-width: 1px; border-color: rgb(209 213 219); margin-top: 4px;
                               display: none; justify-content: center; align-items: center; background-color: white;
                               width: 31px; height: 31px; cursor: pointer;">
                        <span id="sound_icon" style="font-size: 18px;">
                            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#999" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon>
                                <line x1="23" y1="9" x2="17" y2="15"></line>
                                <line x1="17" y1="9" x2="23" y2="15"></line>
                            </svg>
                        </span>
                    </div>
                    <div id="speed_container"
                        style="justify-content: center; align-items: center; width: 31px; height: 31px;
                                background-color: white; margin-top: 4px; padding: 4px; border-radius: 0.375rem;
                                border-width: 1px; border-color: rgb(209 213 219); display: none;">
                        <div id="speed_value"
                            style="font-size: 1.2em; font-weight: bold; justify-content: center; align-items: center;">
                            0
                        </div>
                    </div>
                </div>
                <vi-mobilizon-events></vi-mobilizon-events>
            </div>
            <md-dialog id="city_snow_dialog">
                <div slot="headline" id="headline">Neige au sol à <span class="city_name" style="font-weight: bold;"></span></div>
                <div slot="content">
                    <p>Indiquez si vous avez de la neige au sol dans les endroits non déneigés</p>
                </div>
                <div slot="actions" style="display: flex; gap: 8px; justify-content: center;">
                    <md-filled-button id="snow_yes">Neige</md-filled-button>
                    <md-filled-button id="snow_no">Pas de neige</md-filled-button>
                </div>
            </md-dialog>
        `;
    }

    connectedCallback() {
        registerViMain(this);
        this.addMap();

        // Mettre à jour l'icône son selon l'état du cookie
        if (this.soundEnabled) {
            this.querySelector('#sound_icon').innerHTML = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#1a73e8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`;
        }

        this.querySelector('#layers_button').addEventListener('click', () => {
            const info = this.querySelector("#info");
            let viLayers = new ViLayers(this);
            info.innerHTML = ``;
            info.appendChild(viLayers);
        });

        this.querySelector('#snow_button').addEventListener('click', async () => {
            const map = this.map;
            const centerPoint = map.project(map.getCenter());
            const dialog = this.querySelector('#city_snow_dialog');
            const features = map.queryRenderedFeatures(centerPoint, { layers: ['city'] });
            if (!features.length) return;
            const cityName = features[0].properties.name;
            dialog.querySelector('.city_name').textContent = cityName;
            dialog.show();

            const updateSnow = async (snow) => {
                this.querySelector("#snow_yes").disabled = true;
                this.querySelector("#snow_no").disabled = true;
                await fetch('/city_snow_edit', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ name: cityName, snow })
                });
                dialog.close();
                // on vide les caches
                const cacheNames = await caches.keys();
                Promise.all(
                    cacheNames.map(name => caches.delete(name))
                );
                this.querySelector("#snow_yes").disabled = false;
                this.querySelector("#snow_no").disabled = false;
                map.getSource('city_snow').setUrl(`${window.location.origin}/city_snow?t=${Date.now()}`);
                map.getSource('bike_path').setUrl(`${window.location.origin}/bike_path?t=${Date.now()}`);
            };

            this.querySelector('#snow_yes').onclick = () => updateSnow(true);
            this.querySelector('#snow_no').onclick = () => updateSnow(false);
        });

        this.querySelector('#sound_button').addEventListener('click', () => {
            this.soundEnabled = !this.soundEnabled;
            document.cookie = `sound_enabled=${this.soundEnabled ? '1' : '0'};max-age=31536000;path=/;samesite=strict`;
            let iconSvg = this.soundEnabled
                ? `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#1a73e8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><path d="M19.07 4.93a10 10 0 0 1 0 14.14M15.54 8.46a5 5 0 0 1 0 7.07"></path></svg>`
                : `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#999" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"></polygon><line x1="23" y1="9" x2="17" y2="15"></line><line x1="17" y1="9" x2="23" y2="15"></line></svg>`;
            this.querySelector('#sound_icon').innerHTML = iconSvg;
            if (this.soundEnabled && 'speechSynthesis' in window) {
                let utterance = new SpeechSynthesisUtterance('Annonces vocales activées');
                utterance.lang = 'fr-FR';
                speechSynthesis.speak(utterance);
            }
        });

        // Vérifier si des paramètres de route sont dans l'URL
        this.checkRouteParams();
    }

    checkRouteParams() {
        const params = new URLSearchParams(window.location.search);
        if (params.has("start_lng") && params.has("start_lat") && params.has("end_lng") && params.has("end_lat")) {
            const startLng = parseFloat(params.get("start_lng"));
            const startLat = parseFloat(params.get("start_lat"));
            const endLng = parseFloat(params.get("end_lng"));
            const endLat = parseFloat(params.get("end_lat"));

            // Créer les marqueurs
            this.setMarkers(startLng, startLat, endLng, endLat);

            // Attendre que la carte soit chargée avant de calculer la route
            if (this.map.loaded()) {
                this.route();
            } else {
                this.map.once('load', () => {
                    this.route();
                });
            }
        } else if (params.has("point_lng") && params.has("point_lat")) {
            const pointLng = parseFloat(params.get("point_lng"));
            const pointLat = parseFloat(params.get("point_lat"));

            // Afficher le marqueur du point recherché
            if (this.start_marker) this.start_marker.remove();
            this.start_marker = new maplibregl.Marker({ color: "#00f" })
                .setLngLat([pointLng, pointLat])
                .addTo(this.map);
            // Marquer ce marqueur comme temporaire (créé via URL params)
            this.start_marker.getElement().setAttribute('data-temp', 'true');

            // Afficher le point panel (itinéraire, signaler, annuler)
            const loadPointPanel = async () => {
                const response = await fetch(`/point_panel_lng_lat/${pointLng}/${pointLat}`);
                const json = await response.json();
                const pointPanel = new PointPanel(json.name, { lng: pointLng, lat: pointLat });
                this.querySelector("#info").innerHTML = ``;
                this.querySelector("#info").appendChild(pointPanel);
            };
            if (this.map.loaded()) {
                loadPointPanel();
            } else {
                this.map.once('load', () => loadPointPanel());
            }
        }
    }

    addMap() {
        const position = JSON.parse(localStorage.getItem("position"));
        let lng = position?.lng ?? -73.39899762303611;
        let lat = position?.lat ?? 45.921066117828786;
        let zoom = position?.zoom ?? 6;
        const params = new URLSearchParams(window.location.search);
        if (params.has("lat") && params.has("lng") && params.has("zoom")) {
            lat = parseFloat(params.get("lat"));
            lng = parseFloat(params.get("lng"));
            zoom = parseFloat(params.get("zoom"));
        }

        // Affichage de la vitesse
        navigator.geolocation.watchPosition((position) => {
            const speed = position.coords.speed ? position.coords.speed * 3.6 : 0;
            const speedValue = document.getElementById("speed_value");
            if (speedValue) {
                speedValue.textContent = speed.toFixed(0);
                speedValue.parentElement.style.display = (speed === 0 || speed == null) ? "none" : "flex";
            }
        });

        this.map = new maplibregl.Map({
            container: 'map',
            style: '/style.json',
            center: [lng, lat],
            zoom: zoom,
            minZoom: 5
        });

        // Fournir les images manquantes à la demande pour éviter les warnings
        this.map.on('styleimagemissing', async (e) => {
            const id = e.id;
            const url = SPRITE_IMAGES[id];
            if (!url) return;
            // Avoid duplicate loads while previous addImage is in-flight
            if (this.map.hasImage(id) || this._loadingImages.has(id)) return;
            this._loadingImages.add(id);
            try {
                const res = await this.map.loadImage(url);
                if (!this.map.hasImage(id)) {
                    this.map.addImage(id, res.data);
                }
            } catch (_) {
                // ignore
            } finally {
                this._loadingImages.delete(id);
            }
        });

        // Les images seront ajoutées dynamiquement via styleimagemissing

        this.isGeolocateActive = false;
        this.map.addControl(new maplibregl.NavigationControl());
        this.geolocate = new maplibregl.GeolocateControl({
            fitBoundsOptions: { maxZoom: 16.5 },
            positionOptions: { enableHighAccuracy: true },
            trackUserLocation: true
        });
        this.geolocate.on('trackuserlocationstart', () => { this.isGeolocateActive = true; });
        this.geolocate.on('trackuserlocationend', () => { this.isGeolocateActive = false; });
        this.geolocate.on('error', () => { this.isGeolocateActive = false; });
        this.map.addControl(this.geolocate);
        this.map.addControl(new maplibregl.AttributionControl({
            compact: true
        }));

        this.map.on("load", () => {
            setTimeout(() => {
                const layers = JSON.parse(localStorage.getItem("layers"));
                ["bixi", "bike_parking", "bike_shop", "drinking_water", "bicycle_repair_station"].forEach(layer => {
                    const visible = !layers || !layers[layer] || layers[layer] !== "none";
                    this.map.setLayoutProperty(layer, 'visibility', visible ? 'visible' : 'none');
                });
            }, 1000);
            const params = new URLSearchParams(window.location.search);
            if (!params.has("start_lng")) {
                this.infoPanelUp();
            }
        });

        this.map.on("click", (event) => {
            // Nettoyer le message temporaire si présent (pour le mode signalement)
            if (this.temp_message) {
                this.temp_message.remove();
                this.temp_message = null;
            }
            
            if (
                document.querySelector('vi-change-start') ||
                document.getElementById("info_panel_up") ||
                document.getElementById("info_panel_down") ||
                document.querySelector('vi-segment-panel') ||
                document.getElementById("layers") ||
                document.getElementById("point_panel")
            ) {
                this.select(event);
            }
        });

        let timeout = null;
        this.map.on("move", () => {
            if (timeout) clearTimeout(timeout);
            timeout = setTimeout(() => {
                const map = this.map;
                // If a route or segment panel is present, preserve params; otherwise reset to position-only
                const hasRoutePanel = !!this.querySelector('vi-route-panel');
                const hasSegmentPanel = !!this.querySelector('vi-segment-panel');
                if (hasRoutePanel || hasSegmentPanel) {
                    const url = new URL(window.location);
                    url.searchParams.set('lat', map.getCenter().lat);
                    url.searchParams.set('lng', map.getCenter().lng);
                    url.searchParams.set('zoom', map.getZoom());
                    window.history.replaceState({}, '', url);
                } else {
                    window.history.replaceState(
                        {},
                        '',
                        `/?lat=${map.getCenter().lat}&lng=${map.getCenter().lng}&zoom=${map.getZoom()}`
                    );
                }
                localStorage.setItem("position", JSON.stringify({
                    lng: map.getCenter().lng,
                    lat: map.getCenter().lat,
                    zoom: map.getZoom()
                }));
                if (this.querySelector('#info_panel_up')) {
                    this.infoPanelUp();
                }
            }, 1000);
        });

        // Démarre une géolocalisation en arrière-plan pour garder la position disponible
        if (navigator.geolocation) {
            const geoSuccess = (position) => {
                const coords = {
                    lat: position.coords.latitude,
                    lng: position.coords.longitude,
                    accuracy: position.coords.accuracy,
                    speed: position.coords.speed
                };
                // Stocke la dernière position pour y accéder quand nécessaire
                this._lastPosition = coords;

                // Met à jour l'affichage de la vitesse si présent (sinon rien n'est affiché)
                const speed = position.coords.speed ? position.coords.speed * 3.6 : 0;
                const speedValue = document.getElementById("speed_value");
                if (speedValue) {
                    speedValue.textContent = speed.toFixed(0);
                    speedValue.parentElement.style.display = (speed === 0 || speed == null) ? "none" : "flex";
                }
            };

            const geoError = (err) => {
                console.warn("Geolocation error:", err);
            };

            // Options : haute précision, cache court
            this._geoWatchId = navigator.geolocation.watchPosition(
                geoSuccess,
                geoError,
                { enableHighAccuracy: true, maximumAge: 5000, timeout: 10000 }
            );
        }
    }

    // Méthode helper pour nettoyer les layers et sources
    cleanupLayers(layerNames = [LAYERS.SAFE, LAYERS.FAST, LAYERS.SELECTED]) {
        layerNames.forEach(layer => {
            if (this.map.getLayer(layer)) {
                this.map.removeLayer(layer);
            }
            if (this.map.getSource(layer)) {
                this.map.removeSource(layer);
            }
        });
    }

    // Méthode helper pour gérer les marqueurs
    setMarkers(startLng, startLat, endLng, endLat) {
        if (this.start_marker) this.start_marker.remove();
        this.start_marker = new maplibregl.Marker({ color: MARKER_COLORS.START })
            .setLngLat([startLng, startLat])
            .addTo(this.map);

        if (this.end_marker) this.end_marker.remove();
        this.end_marker = new maplibregl.Marker({ color: MARKER_COLORS.END })
            .setLngLat([endLng, endLat])
            .addTo(this.map);
    }

    // Méthode helper pour mettre à jour l'URL avec les coordonnées de route
    updateRouteUrl(startLng, startLat, endLng, endLat) {
        const url = new URL(window.location);
        url.searchParams.set('start_lng', startLng.toFixed(6));
        url.searchParams.set('start_lat', startLat.toFixed(6));
        url.searchParams.set('end_lng', endLng.toFixed(6));
        url.searchParams.set('end_lat', endLat.toFixed(6));
        window.history.replaceState({}, '', url);
    }

    // Méthode helper pour mettre à jour l'URL avec le point sélectionné (segment panel)
    updateSegmentUrl(pointLng, pointLat) {
        const url = new URL(window.location);
        url.searchParams.set('point_lng', parseFloat(pointLng).toFixed(6));
        url.searchParams.set('point_lat', parseFloat(pointLat).toFixed(6));
        window.history.replaceState({}, '', url);
    }

    async infoPanelUp() {
        const bounds = this.map.getBounds();
        const r = await fetch(`/info_panel/up/${bounds._sw.lng}/${bounds._sw.lat}/${bounds._ne.lng}/${bounds._ne.lat}`);
        const json = await r.json();
        // Si pas de contributions, afficher info_panel_down (replié)
        if (!json.contributions || json.contributions.length === 0) {
            json.arrow = null;
        }
        let viInfo = new ViInfo(json);
        this.querySelector("#info").innerHTML = ``;
        this.querySelector('#info').appendChild(viInfo);
    }

    /**
     * Gère le clic sur la carte pour la sélection de points et segments.
     * 
     * Déclenché par: this.map.on("click", ...) ligne 266
     * Condition: seulement si un panneau est ouvert (vi-change-start, info_panel_*,
     *            segment_panel, layers, point_panel)
     * 
     * COULEURS DES MARQUEURS:
     *   - ORANGE (#ffa500) = début du segment sélectionné
     *   - ROUGE (#f00)     = départ d'itinéraire (GPS)
     *   - BLEU (#00f)      = destination / point cliqué
     * 
     * NOUVEAU FONCTIONNEMENT SELECT (2 clics):
     * 
     * 1. MODE "CHANGER LE DÉPART" (lignes ~474-480)
     *    Condition: vi-change-start présent && this.changeStartDestination existe
     *    Actions:
     *      - Crée start_marker (rouge) aux nouvelles coordonnées du clic
     *      - Crée end_marker (bleu) à this.changeStartDestination
     *      - Nettoie this.changeStartDestination = null
     *      - Lance this.route() pour recalculer l'itinéraire
     *      - RETURN (fin prématurée)
     * 
     * 2. DÉTECTION DE PISTE CYCLABLE (lignes ~484-490)
     *    - queryRenderedFeatures avec boîte 20x20px
     *    - Couches: ['cycleway', 'designated', 'shared_lane']
     *    - Vérifie si segment_panel est déjà ouvert (existingSegmentPanel)
     * 
     * 3. CLIC SUR PISTE CYCLABLE - NOUVEAU FONCTIONNEMENT:
     *    3a. SI _firstClick existe DÉJÀ (2ème clic):
     *        → Appel à /segment_between/{startLng}/{startLat}/{endLng}/{endLat}
     *        → Le serveur retourne un geom qui couvre les deux points
     *        → Affiche vi-segment-panel avec les données
     *        → Réinitialise _firstClick = null
     *        → RETURN
     *    3b. SINON (1er clic):
     *        → Place end_marker (BLEU) au point cliqué
     *        → Stocke _firstClick = { lng, lat }
     *        → NE FAIT AUCUNE SÉLECTION DE WAY
     * 
     * 4. CLIC DANS LE VIDE (lignes ~517-566)
     *    4a. SI segment_panel ouvert:
     *        → this.clear() — supprime les marqueurs de sélection
     *    4b. Réinitialise _firstClick = null
     *    4c. Crée ou déplace end_marker (BLEU) au clic
     *    4d. Vide la source "selected" (LineString vide)
     *    4e. Recherche itérative d'un nom de lieu
     *    4f. Affiche vi-point-panel avec le nom trouvé
     * 
     * @param {Object} event - Événement de clic MapLibre
     * @param {Object} event.lngLat - Coordonnées géographiques du clic
     * @param {Object} event.point - Coordonnées pixels du clic (x, y)
     */
    async select(event) {
        // Éviter les exécutions multiples du même clic
        if (this._isSelecting) {
            return;
        }
        this._isSelecting = true;
        
        try {
            // Nettoyer le marqueur bleu temporaire (chargé via URL params) si présent
            // Ce marqueur est temporaire et doit être supprimé au premier clic
            if (this.start_marker && this.start_marker.getElement()?.getAttribute('data-temp') === 'true') {
                this.start_marker.remove();
                this.start_marker = null;
            }
            
            // Mode sélection du nouveau point de départ
            if (document.querySelector('vi-change-start') && this.changeStartDestination) {
                // Créer les marqueurs avec les nouvelles coordonnées
                this.setMarkers(event.lngLat.lng, event.lngLat.lat,
                    this.changeStartDestination.lng, this.changeStartDestination.lat);
                this.changeStartDestination = null;
                this.route();
                return;
            }
            
            // Vérifier si un segment_panel est déjà ouvert
            const existingSegmentPanel = document.querySelector('vi-segment-panel');
            
            // Vérifier si un point_panel est déjà ouvert
            const existingPointPanel = document.getElementById("point_panel");
            
            // Mode signalement OU segment existant : un seul clic pour mettre à jour la fin du segment
            if (existingSegmentPanel) {
                // Déterminer le point de départ
                let startLng, startLat;
                if (this._isReporting && this._reportingSegment) {
                    // Mode signalement : utiliser le point sauvegardé
                    startLng = this._reportingSegment.startLng;
                    startLat = this._reportingSegment.startLat;
                } else if (this._segmentStart) {
                    // Segment normal : utiliser le point de départ du segment actuel
                    startLng = this._segmentStart.lng;
                    startLat = this._segmentStart.lat;
                } else {
                    // Pas de point de départ connu, laisser le comportement normal
                }
                
                if (startLng !== undefined && startLat !== undefined) {
                    const endLng = event.lngLat.lng;
                    const endLat = event.lngLat.lat;
                    
                    // Appel au serveur pour recalculer la géométrie du segment avec la nouvelle fin
                    // Le serveur connaît la forme réelle de la rue (pas juste un buffer simplifié)
                    const response = await fetch(`/segment_between/${startLng}/${startLat}/${endLng}/${endLat}`);
                    const jsonData = await response.json();
                    
                    // Mettre à jour la source avec la nouvelle géométrie du serveur
                    if (jsonData.geom_json && this.map.getSource("selected")) {
                        this.map.getSource("selected").setData(JSON.parse(jsonData.geom_json));
                    }
                    
                    // Mettre à jour le segment-panel avec la nouvelle géométrie
                    if (existingSegmentPanel && jsonData.geom_json) {
                        existingSegmentPanel.updateSegment({
                            geom_json: jsonData.geom_json,
                            startLng, startLat, endLng, endLat
                        });
                    }
                    
                    // Mettre à jour l'URL
                    this.updateSegmentUrl(endLng, endLat);
                    
                    // Réinitialiser le mode signalement si actif
                    if (this._isReporting) {
                        this._isReporting = false;
                        this._reportingSegment = null;
                    }
                    
                    // Mettre à jour le point de départ du segment pour les prochains clics
                    this._segmentStart = { lng: startLng, lat: startLat };
                    
                    // Réinitialiser le premier clic
                    this._firstClick = null;
                    return;
                }
            }
            
            // Si un point_panel est ouvert (et pas en mode signalement), on ne crée pas de segment
            if (existingPointPanel && !this._isReporting) {
                // Mettre à jour le marker bleu
                if (this.end_marker) {
                    this.end_marker.setLngLat([event.lngLat.lng, event.lngLat.lat]);
                } else {
                    this.end_marker = new maplibregl.Marker({ color: MARKER_COLORS.END })
                        .setLngLat([event.lngLat.lng, event.lngLat.lat])
                        .addTo(this.map);
                }
                
                // Mettre à jour l'URL
                this.updateSegmentUrl(event.lngLat.lng, event.lngLat.lat);
                
                // Réinitialiser _firstClick pour éviter de créer un segment au prochain clic
                this._firstClick = null;
                return;
            }
            
            // 2ème clic : on a déjà un premier point → créer un segment avec buffer 10m
            if (this._firstClick) {
                    // 2ème clic : on a déjà un premier point → créer un segment avec buffer 10m
                    const startLng = this._firstClick.lng;
                    const startLat = this._firstClick.lat;
                    const endLng = event.lngLat.lng;
                    const endLat = event.lngLat.lat;
                    
                    // Vérifier si les deux points sont identiques (ou très proches) → créer un cercle
                    const distance = Math.sqrt(Math.pow(endLng - startLng, 2) + Math.pow(endLat - startLat, 2));
                    const isSamePoint = distance < 0.0001; // ~10 mètres
                    
                    // Fonction pour créer un buffer avec extrémités arrondies
                    // 10 mètres pour modifications (garde la largeur du segment existant)
                    // 0.3 mètres pour le cercle initial en mode signalement (premier clic seulement)
                    const bufferMeters = 10;
                    const createBuffer = (lng1, lat1, lng2, lat2, bufferMeters = 10, isCircle = false) => {
                        // Conversion mètres → degrés
                        const avgLat = (lat1 + lat2) / 2;
                        const latDeg = bufferMeters / 111320;
                        const lngDeg = bufferMeters / (111320 * Math.cos(avgLat * Math.PI / 180));
                        
                        // Si c'est un cercle (points identiques), pas besoin de calculs de direction
                        if (isCircle) {
                            const points = 32;
                            const coords = [];
                            for (let i = 0; i < points; i++) {
                                const angle = (2 * Math.PI * i) / points;
                                coords.push([
                                    lng1 + lngDeg * Math.cos(angle),
                                    lat1 + latDeg * Math.sin(angle)
                                ]);
                            }
                            coords.push(coords[0]); // Fermer
                            return {
                                type: "Polygon",
                                coordinates: [coords]
                            };
                        }
                        
                        // Vecteurs de base
                        const dx = lng2 - lng1;
                        const dy = lat2 - lat1;
                        const length = Math.sqrt(dx * dx + dy * dy);
                        
                        // Vecteurs unitaires
                        const dirX = dx / length;   // direction de la ligne
                        const dirY = dy / length;
                        const perpX = -dy / length; // perpendiculaire (90° gauche)
                        const perpY = dx / length;
                        
                        const arcPoints = 8;
                        const coords = [];
                        
                        // 1. Ligne droite côté supérieur (de start à end)
                        coords.push([lng1 + perpX * lngDeg, lat1 + perpY * latDeg]);
                        coords.push([lng2 + perpX * lngDeg, lat2 + perpY * latDeg]);
                        
                        // 2. Demi-cercle à la FIN (de +perp à -perp, dans le sens de la ligne)
                        for (let i = 1; i < arcPoints; i++) {
                            const angle = (Math.PI * i / arcPoints); // 0 à PI
                            // cos(angle)*perp + sin(angle)*dir donne l'arc vers l'avant
                            coords.push([
                                lng2 + lngDeg * Math.cos(angle) * perpX + lngDeg * Math.sin(angle) * dirX,
                                lat2 + latDeg * Math.cos(angle) * perpY + latDeg * Math.sin(angle) * dirY
                            ]);
                        }
                        
                        // 3. Ligne droite côté inférieur (de end à start)
                        coords.push([lng2 - perpX * lngDeg, lat2 - perpY * latDeg]);
                        coords.push([lng1 - perpX * lngDeg, lat1 - perpY * latDeg]);
                        
                        // 4. Demi-cercle au DÉPART (de -perp à +perp, sens opposé à la ligne)
                        for (let i = 1; i < arcPoints; i++) {
                            const angle = (Math.PI * i / arcPoints); // 0 à PI
                            // -cos(angle)*perp - sin(angle)*dir donne l'arc vers l'arrière
                            coords.push([
                                lng1 - lngDeg * Math.cos(angle) * perpX - lngDeg * Math.sin(angle) * dirX,
                                lat1 - lngDeg * Math.cos(angle) * perpY - lngDeg * Math.sin(angle) * dirY
                            ]);
                        }
                        
                        coords.push(coords[0]); // Fermer
                        
                        return {
                            type: "Polygon",
                            coordinates: [coords]
                        };
                    };
                    
                    const polygon = createBuffer(startLng, startLat, endLng, endLat, bufferMeters, isSamePoint);
                    
                    // Ajouter la source et le layer de sélection avec extrémités arrondies
                    // En mode signalement, utiliser le rouge (couleur des reports), sinon le bleu (sélection)
                    const selectedColor = this._isReporting ? "#ff0000" : "#0000ff";
                    if (this.map.getSource("selected")) {
                        this.map.getSource("selected").setData(polygon);
                    } else {
                        this.map.addSource("selected", {
                            type: "geojson",
                            data: polygon
                        });
                    }
                    
                    // Layer de remplissage
                    if (!this.map.getLayer("selected")) {
                        this.map.addLayer({
                            id: "selected",
                            type: "fill",
                            source: "selected",
                            paint: {
                                "fill-color": selectedColor,
                                "fill-opacity": 0.3,
                                "fill-antialias": true
                            }
                        });
                    } else {
                        this.map.setPaintProperty("selected", "fill-color", selectedColor);
                    }
                    
                    // Layer de contour
                    if (!this.map.getLayer("selected-outline")) {
                        this.map.addLayer({
                            id: "selected-outline",
                            type: "line",
                            source: "selected",
                            paint: {
                                "line-color": selectedColor,
                                "line-width": 2
                            }
                        });
                    } else {
                        this.map.setPaintProperty("selected-outline", "line-color", selectedColor);
                    }
                    
                    // Pas de zoom automatique - l'utilisateur garde le contrôle de la vue
                    
                    // Récupérer le nom utilisateur depuis le cookie uuid
                    let userNameResp = await fetch('/user_name', { credentials: 'same-origin' });
                    let userNameData = await userNameResp.json();

                    // Données pour le segment panel
                    const jsonData = {
                        way_ids: "",
                        score_circle: { score: -1 },
                        segment_name: this._isReporting ? "Segment à signaler" : "Segment sélectionné",
                        score_selector: "",
                        comment: "",
                        edit: this._isReporting,
                        is_reporting: this._isReporting,
                        photo_ids: [],
                        geom_json: JSON.stringify(polygon),
                        fit_bounds: false,
                        user_name: userNameData.user_name || "",
                        martin_url: `${window.location.origin}/martin`
                    };
                    
                    const segment_panel = new SegmentPanel(jsonData);
                    this.querySelector("#info").innerHTML = ``;
                    this.querySelector("#info").appendChild(segment_panel);
                    
                    // Mettre à jour l'URL avec la destination choisie
                    this.updateSegmentUrl(endLng, endLat);
                    
                    // Réinitialiser le mode signalement
                    this._isReporting = false;
                    this._reportingSegment = null;
                    
                    // Mettre à jour le point de départ du segment pour les prochains clics
                    this._segmentStart = { lng: startLng, lat: startLat };
                    
                    // Réinitialiser le premier clic
                    this._firstClick = null;
                    return;
            }
            
            // 1er clic : stocker le point et placer le marqueur BLEU (destination)
            this._firstClick = { lng: event.lngLat.lng, lat: event.lngLat.lat };
            
            // end_marker au point cliqué
            if (this.end_marker) {
                this.end_marker.setLngLat([event.lngLat.lng, event.lngLat.lat]);
            } else {
                this.end_marker = new maplibregl.Marker({ color: MARKER_COLORS.END })
                    .setLngLat([event.lngLat.lng, event.lngLat.lat])
                    .addTo(this.map);
            }
            
            // Afficher un point_panel
            const point_panel = document.createElement("vi-point-panel");
            point_panel.panel_id = "point_panel";
            point_panel.coords = { lng: event.lngLat.lng, lat: event.lngLat.lat, name: "" };
            point_panel.on_cycleway = false;
            this.querySelector("#info").innerHTML = ``;
            this.querySelector("#info").appendChild(point_panel);
            
            // Mettre à jour l'URL
            this.updateSegmentUrl(event.lngLat.lng, event.lngLat.lat);
            
        } finally {
            // Délai basé sur le zoom pour éviter les conflits
            const zoom = this.map.getZoom();
            const delay = zoom < 10 ? 500 : 300; // Plus long à faible zoom
            setTimeout(() => {
                this._isSelecting = false;
            }, delay);
        }
    }

    async clear() {
        if (this.start_marker) {
            this.start_marker.remove();
            this.start_marker = null;
        }
        if (this.end_marker) {
            this.end_marker.remove();
            this.end_marker = null;
        }
        this._firstClick = null;
        this._segmentStart = null;
        this._isReporting = false;
        this._reportingSegment = null;
        [
            "selected_safe",
            "selected_fast",
            "searched_route",
            "selected",
            "selected-outline"
        ].forEach(layer => {
            if (this.map.getLayer(layer)) this.map.removeLayer(layer);
        });
        [
            "selected_safe",
            "selected_fast",
            "searched_route",
            "selected",
            "selected-outline"
        ].forEach(source => {
            if (this.map.getSource(source)) this.map.removeSource(source);
        });


        // Affiche le panneau d'info
        let viInfo = new ViInfo(null);
        this.querySelector("#info").innerHTML = ``;
        this.querySelector('#info').appendChild(viInfo);
    }

    async route() {
        // Vérifier qu'on a une destination (end_marker)
        if (!this.end_marker) {
            console.warn("route() : pas de destination (end_marker)");
            return;
        }
        
        // Quand on clique "Itinéraire", on utilise TOUJOURS la GPS comme départ
        // On supprime le marqueur de segment (ORANGE) s'il existe
        if (this.start_marker) {
            this.start_marker.remove();
            this.start_marker = null;
        }
        
        // Créer le marqueur de départ ROUGE depuis la géolocalisation
        const position = this.getLastPosition();
        if (!position) {
            // Pas de GPS → afficher RouteSearching quand même, il gérera la demande GPS
            // ou proposera "entrer votre position manuellement"
            const info = document.getElementById("info");
            const routeSearching = new RouteSearching(this);
            info.innerHTML = ``;
            info.appendChild(routeSearching);
            return;
        }
        
        this.start_marker = new maplibregl.Marker({ color: MARKER_COLORS.START })
            .setLngLat([position.lng, position.lat])
            .addTo(this.map);
        
        // Centrer la carte pour voir les deux marqueurs
        const bounds = new maplibregl.LngLatBounds();
        bounds.extend([position.lng, position.lat]);
        bounds.extend([this.end_marker.getLngLat().lng, this.end_marker.getLngLat().lat]);
        this.map.fitBounds(bounds, { padding: 50 });
        
        const info = document.getElementById("info");
        const routeSearching = new RouteSearching(this);
        info.innerHTML = ``;
        info.appendChild(routeSearching);
    }


    fitBounds(geom) {
        return geom.reduce(
            ([min, max], coord) => [
                [Math.min(coord[0], min[0]), Math.min(coord[1], min[1])],
                [Math.max(coord[0], max[0]), Math.max(coord[1], max[1])]
            ],
            [[Infinity, Infinity], [-Infinity, -Infinity]]
        );
    }

    calculateBearing(lon1, lat1, lon2, lat2) {
        lon1 *= Math.PI / 180.0;
        lat1 *= Math.PI / 180.0;
        lon2 *= Math.PI / 180.0;
        lat2 *= Math.PI / 180.0;
        const y = Math.sin(lon2 - lon1) * Math.cos(lat2);
        const x = Math.cos(lat1) * Math.sin(lat2) - Math.sin(lat1) * Math.cos(lat2) * Math.cos(lon2 - lon1);
        let bearing = Math.atan2(y, x) * (180 / Math.PI);
        return (bearing + 360) % 360;
    }

    calculateDistance(lat1, lon1, lat2, lon2) {
        const R = 6371;
        const φ1 = lat1 * Math.PI / 180;
        const φ2 = lat2 * Math.PI / 180;
        const Δφ = (lat2 - lat1) * Math.PI / 180;
        const Δλ = (lon2 - lon1) * Math.PI / 180;
        const a = Math.sin(Δφ / 2) ** 2 +
            Math.cos(φ1) * Math.cos(φ2) *
            Math.sin(Δλ / 2) ** 2;
        const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
        return R * c;
    }

    calculateTotalDistance(coordinates, index = 0) {
        if (!coordinates || coordinates.length === 0) return 0;
        this.distanceCache = this.distanceCache || {};
        if (index in this.distanceCache) return this.distanceCache[index];

        let totalDistance = 0;
        for (let i = index; i < coordinates.length - 1; i++) {
            totalDistance += this.calculateDistance(
                coordinates[i][1], coordinates[i][0],
                coordinates[i + 1][1], coordinates[i + 1][0]
            );
        }
        this.distanceCache[index] = totalDistance;
        return totalDistance;
    }

    clearDistanceCache() {
        this.distanceCache = {};
    }

    getLastPosition() {
        return this._lastPosition || null;
    }

    stopBackgroundGeolocation() {
        if (this._geoWatchId != null && navigator.geolocation) {
            navigator.geolocation.clearWatch(this._geoWatchId);
            this._geoWatchId = null;
        }
    }
}

customElements.define('vi-main', ViMain);

