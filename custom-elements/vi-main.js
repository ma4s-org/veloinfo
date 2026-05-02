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

            // Charger le segment panel autour de ce point
            const loadSegment = async () => {
                const response = await fetch(`/segment_panel_lng_lat/${pointLng}/${pointLat}`);
                const jsonData = await response.json();
                const segment_panel = new SegmentPanel(jsonData);
                this.querySelector("#info").innerHTML = ``;
                this.querySelector("#info").appendChild(segment_panel);
            };
            if (this.map.loaded()) {
                loadSegment();
            } else {
                this.map.once('load', () => loadSegment());
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
            if (
                document.querySelector('vi-change-start') ||
                document.getElementById("info_panel_up") ||
                document.getElementById("info_panel_down") ||
                document.getElementById("segment_panel") ||
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
     * FLUX DE LA MÉTHODE:
     * 
     * 1. MODE "CHANGER LE DÉPART" (lignes ~471-478)
     *    Condition: vi-change-start présent && this.changeStartDestination existe
     *    Actions:
     *      - Crée start_marker (rouge) aux nouvelles coordonnées du clic
     *      - Crée end_marker (bleu) à this.changeStartDestination
     *      - Nettoie this.changeStartDestination = null
     *      - Lance this.route() pour recalculer l'itinéraire
     *      - RETURN (fin prématurée)
     * 
     * 2. DÉTECTION DE PISTE CYCLABLE (lignes ~483-489)
     *    - queryRenderedFeatures avec boîte 20x20px
     *    - Couches: ['cycleway', 'designated', 'shared_lane']
     *    - Vérifie si segment_panel est déjà ouvert (existingSegmentPanel)
     * 
     * 3. CLIC SUR PISTE CYCLABLE (lignes ~491-514)
     *    3a. SI segment_panel déjà ouvert:
     *        → this.selectBigger(event) — agrandit la sélection
     *        → RETURN (fin prématurée)
     *    3b. SINON (premier segment):
     *        → start_marker (ORANGE) au début du segment (depuis geom_json)
     *        → end_marker (BLEU) au point cliqué
     *        → Requête HTTP: /segment_panel_lng_lat/{lng}/{lat}
     *        → Affiche vi-segment-panel avec les données
     *        → Met à jour l'URL: ?point_lng=...&point_lat=...
     * 
     * 4. CLIC DANS LE VIDE (lignes ~516-543)
     *    4a. SI segment_panel ouvert:
     *        → this.clear() — supprime les marqueurs de sélection
     *    4b. Crée ou déplace end_marker (BLEU) au clic
     *    4c. Vide la source "selected" (LineString vide)
     *    4d. Recherche itérative d'un nom de lieu (1000px par 10px)
     *        Couches: ['name', 'Road network', 'City labels',
     *                 'Town labels', 'Village labels']
     *        → S'arrête dès qu'un f.properties.name est trouvé
     *    4e. Affiche vi-point-panel avec le nom trouvé (ou "" si rien)
     * 
     * RÉSUMÉ DES CAS D'USAGE:
     * ┌──────────────────────────────────────────────────────────────────────┐
     * │ Cas                    │ Action                                      │
     * ├──────────────────────────────────────────────────────────────────────┤
     * │ Changer point départ   │ vi-change-start + destination → route()     │
     * │ 1er clic sur piste     → start_marker (ORANGE) + end_marker (BLEU)   │
     * │                        → + segment_panel                             │
     * │ 2ème clic sur piste      → selectBigger() (agrandir sélection)         │
     * │ Clic dans le vide      → end_marker (BLEU) + point_panel             │
     * │ Clic (segment ouvert)  → clear() + end_marker (BLEU) + point_panel   │
     * │ Bouton Itinéraire      → start_marker (ORANGE→ROUGE GPS) + BLEU      │
     * └──────────────────────────────────────────────────────────────────────┘
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
            // Mode sélection du nouveau point de départ
            if (document.querySelector('vi-change-start') && this.changeStartDestination) {
                // Créer les marqueurs avec les nouvelles coordonnées
                this.setMarkers(event.lngLat.lng, event.lngLat.lat,
                    this.changeStartDestination.lng, this.changeStartDestination.lat);
                this.changeStartDestination = null;
                this.route();
                return;
            }
            
            // Clic : créer ou déplacer le point d'arrivée (BLEU) SEULEMENT si pas sur une piste
            // Vérifier d'abord si on clique sur une piste cyclable
            const width = 20;
            const features = this.map.queryRenderedFeatures(
                [
                    [event.point.x - width / 2, event.point.y - width / 2],
                    [event.point.x + width / 2, event.point.y + width / 2]
                ], { layers: ['cycleway', 'designated', 'shared_lane'] }
            );
            
            // Vérifier si un segment_panel est déjà ouvert (pour agrandir la sélection)
            const existingSegmentPanel = document.getElementById("segment_panel");
            
            if (features.length) {
                // Sur une piste : afficher segment_panel + marqueur BLEU pour agrandir sélection
                if (existingSegmentPanel) {
                    // Déjà un segment de sélectionné → agrandir la sélection
                    this.selectBigger(event);
                    return;
                }
                
                // Premier segment sélectionné → récupérer le début du segment pour start_marker
                const response = await fetch(`/segment_panel_lng_lat/${event.lngLat.lng}/${event.lngLat.lat}`);
                const jsonData = await response.json();
                
                // Créer start_marker (ROUGE) au début du segment et end_marker (BLEU) au clic
                if (jsonData.geom_json) {
                    const geom = JSON.parse(jsonData.geom_json);
                    if (geom && geom[0] && geom[0][0]) {
                        // start_marker au début du segment (geom = [[[lng,lat],...]])
                        const startCoord = geom[0][0];
                        if (this.start_marker) {
                            this.start_marker.setLngLat(startCoord);
                        } else {
                            this.start_marker = new maplibregl.Marker({ color: MARKER_COLORS.SEGMENT_START })
                                .setLngLat(startCoord)
                                .addTo(this.map);
                        }
                    }
                }
                
                // end_marker au point cliqué
                if (this.end_marker) {
                    this.end_marker.setLngLat([event.lngLat.lng, event.lngLat.lat]);
                } else {
                    this.end_marker = new maplibregl.Marker({ color: MARKER_COLORS.END })
                        .setLngLat([event.lngLat.lng, event.lngLat.lat])
                        .addTo(this.map);
                }
                
                const segment_panel = new SegmentPanel(jsonData);
                this.querySelector("#info").innerHTML = ``;
                this.querySelector("#info").appendChild(segment_panel);
                this.updateSegmentUrl(event.lngLat.lng, event.lngLat.lat);
            } else {
                // Dans le vide : si segment_panel ouvert, on enlève les marqueurs
                if (existingSegmentPanel) {
                    this.clear();
                }
                
                // Créer ou déplacer le point BLEU
                if (this.end_marker) {
                    this.end_marker.setLngLat([event.lngLat.lng, event.lngLat.lat]);
                } else {
                    this.end_marker = new maplibregl.Marker({ color: MARKER_COLORS.END })
                        .setLngLat([event.lngLat.lng, event.lngLat.lat])
                        .addTo(this.map);
                }
                
                const selected = this.map.getSource("selected");
                if (selected) {
                    selected.setData({
                        type: "Feature",
                        properties: {},
                        geometry: { type: "LineString", coordinates: [] }
                    });
                }
                
                // Recherche du nom le plus proche pour point_panel
                let name = "";
                for (let i = 0; i < 1000; i += 10) {
                    const features = this.map.queryRenderedFeatures(
                        [
                            [event.point.x - i, event.point.y - i],
                            [event.point.x + i, event.point.y + i]
                        ], { layers: ['name', 'Road network', 'City labels', 'Town labels', 'Village labels'] }
                    );
                    for (const f of features) {
                        if (f.properties.name) {
                            name = f.properties.name;
                            break;
                        }
                    }
                    if (name) break;
                }
                
                const point_panel = document.createElement("vi-point-panel");
                point_panel.panel_id = "point_panel";
                point_panel.coords = { lng: event.lngLat.lng, lat: event.lngLat.lat, name };
                this.querySelector("#info").innerHTML = ``;
                this.querySelector("#info").appendChild(point_panel);
            }
            
        } finally {
            // Délai basé sur le zoom pour éviter les conflits
            const zoom = this.map.getZoom();
            const delay = zoom < 10 ? 500 : 300; // Plus long à faible zoom
            setTimeout(() => {
                this._isSelecting = false;
            }, delay);
        }
    }

    async selectBigger(event, destinationLngLat = null) {
        if (this.end_marker) this.end_marker.remove();

        // Utiliser la destination fournie ou l'event
        const destLng = destinationLngLat ? destinationLngLat.lng : event.lngLat.lng;
        const destLat = destinationLngLat ? destinationLngLat.lat : event.lngLat.lat;

        this.end_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([destLng, destLat]).addTo(this.map);

        const r = await fetch(`/segment_panel_bigger/${this.start_marker.getLngLat().lng}/${this.start_marker.getLngLat().lat}/${destLng}/${destLat}`);
        const jsonData = await r.json();
        const segment_panel = new SegmentPanel(jsonData);
        this.querySelector("#info").innerHTML = ``;
        this.querySelector("#info").appendChild(segment_panel);
        // Mettre à jour l'URL avec la destination choisie
        this.updateSegmentUrl(destLng, destLat);
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
        [
            "selected_safe",
            "selected_fast",
            "searched_route",
            "selected"
        ].forEach(layer => {
            if (this.map.getLayer(layer)) this.map.removeLayer(layer);
            if (this.map.getSource(layer)) this.map.removeSource(layer);
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

