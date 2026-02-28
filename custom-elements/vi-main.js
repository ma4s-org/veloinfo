if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/pub/service-worker.js", { scope: '/' });
}

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
import { registerViMain, unregisterViMain } from '/custom-elements/vi-context.js';

const html = String.raw;

// Constantes de couleurs
const MARKER_COLORS = {
    START: "#f00",  // Rouge pour le départ
    END: "#00f"     // Bleu pour l'arrivée
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
            minZoom: 8
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

    async select(event) {
        // Mode sélection du nouveau point de départ
        if (document.querySelector('vi-change-start') && this.changeStartDestination) {
            // Créer les marqueurs avec les nouvelles coordonnées
            this.setMarkers(event.lngLat.lng, event.lngLat.lat,
                this.changeStartDestination.lng, this.changeStartDestination.lat);

            // Nettoyer la destination
            this.changeStartDestination = null;

            this.route();
            return;
        }

        if (this.start_marker && this.end_marker) {
            this.clear();
        }


        if (this.start_marker && this.map.getLayer("selected")) {
            this.selectBigger(event);
            return;
        }

        if (this.start_marker) this.start_marker.remove();
        // Premier clic = destination (bleu), le départ viendra de la géolocalisation
        this.start_marker = new maplibregl.Marker({ color: "#00f" }).setLngLat([event.lngLat.lng, event.lngLat.lat]).addTo(this.map);

        const width = 20;
        let features = this.map.queryRenderedFeatures(
            [
                [event.point.x - width / 2, event.point.y - width / 2],
                [event.point.x + width / 2, event.point.y + width / 2]
            ], { layers: ['cycleway', 'designated', 'shared_lane'] }
        );
        if (features.length) {
            const response = await fetch(`/segment_panel_lng_lat/${event.lngLat.lng}/${event.lngLat.lat}`);
            const jsonData = await response.json();
            const segment_panel = new SegmentPanel(jsonData);
            this.querySelector("#info").innerHTML = ``;
            this.querySelector("#info").appendChild(segment_panel);
            // Mettre à jour l'URL avec le point sélectionné pour partage
            this.updateSegmentUrl(event.lngLat.lng, event.lngLat.lat);
        } else {
            const selected = this.map.getSource("selected");
            if (selected) {
                selected.setData({
                    type: "Feature",
                    properties: {},
                    geometry: { type: "LineString", coordinates: [] }
                });
            }

            // Recherche du nom le plus proche
            let name = "";
            for (let i = 0; i < 1000; i += 10) {
                features = this.map.queryRenderedFeatures(
                    [
                        [event.point.x - i, event.point.y - i],
                        [event.point.x + i, event.point.y + i]
                    ], { layers: ['name', 'Road network', 'City labels', 'Town labels', 'Village labels'] }
                );
                for (const f of features) {
                    if (f.properties.name) {
                        name = f.properties.name;
                        i = 1000;
                        break;
                    }
                }
                if (name) break;
            }

            const pointPanel = new PointPanel(name);
            this.querySelector("#info").innerHTML = ``;
            this.querySelector("#info").appendChild(pointPanel);
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
        const data = await (await fetch("/info_panel/down")).json();
        this.querySelector("#info").innerHTML = `<vi-info></vi-info>`;
        this.querySelector('vi-info').data = data;
    }

    async route() {
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

